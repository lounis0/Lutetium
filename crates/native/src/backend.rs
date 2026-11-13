//! Native backend: typed IR -> Cranelift -> COFF object.
//!
//! The input is the same `IrModule` the bytecode backend consumes — there is
//! no second frontend. Every Lutetium function becomes a Cranelift function;
//! the module entry point is emitted as the exported symbol `main` so the
//! MinGW C runtime startup can call it.
//!
//! Value representation:
//! * `int` -> i64, `float` -> f64, `bool` -> i8 (0/1),
//! * `str` -> i64 pointer to a `[len: u64][bytes..]` header (constant strings
//!   live in read-only data; concatenation allocates through the runtime),
//! * `unit` -> no value.
//!
//! Checked arithmetic: division/modulo guard against zero and `+ - *` guard
//! against overflow, branching to `lut_rt_panic_*` runtime functions — the
//! same observable behavior as the bytecode VM (E5002/E5005).

use cranelift_codegen::ir::condcodes::{FloatCC, IntCC};
use cranelift_codegen::ir::types;
use cranelift_codegen::ir::{AbiParam, InstBuilder, TrapCode, Type as IrType, UserFuncName, Value};
use cranelift_codegen::isa;
use cranelift_codegen::isa::CallConv;
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext, Variable};
use cranelift_module::{Linkage, Module};
use cranelift_object::{ObjectBuilder, ObjectModule};

use lu_compiler::ir::{BinOp, Const, IrFunction, IrInstr, IrModule, IrTerm, Ty, UnOp};

/// Runtime function names imported by generated code.
const RT_PRINT_I64: &str = "lut_rt_print_i64";
const RT_PRINT_F64: &str = "lut_rt_print_f64";
const RT_PRINT_BOOL: &str = "lut_rt_print_bool";
const RT_PRINT_STR: &str = "lut_rt_print_str";
const RT_CONCAT: &str = "lut_rt_concat";
const RT_STR_EQ: &str = "lut_rt_str_eq";
const RT_PANIC_DIV0: &str = "lut_rt_panic_div0";
const RT_PANIC_OVF: &str = "lut_rt_panic_overflow";

#[derive(Debug)]
pub struct NativeError(pub String);

impl std::fmt::Display for NativeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[E6002] native compilation failed: {}", self.0)
    }
}

fn cl_type(ty: Ty) -> IrType {
    match ty {
        Ty::Int => types::I64,
        Ty::Float => types::F64,
        Ty::Bool => types::I8,
        Ty::Str => types::I64, // pointer to [len: u64][bytes]
        Ty::Unit => types::I8, // never carries a value
        // Heap/container and opaque Win32 handles are pointer-sized values.
        Ty::List | Ty::Map | Ty::Window | Ty::Widget => types::I64,
    }
}

/// Rejects modules using features the native backend does not support yet:
/// list/map values and built-in calls. Scalar programs stay native;
/// everything else runs on the bytecode VM until heap-reference support
/// lands in the runtime. Failing loudly here beats silently miscompiling.
fn check_supported(ir: &IrModule) -> Result<(), NativeError> {
    for f in &ir.funcs {
        let unsupported_ty = |t: &Ty| matches!(t, Ty::List | Ty::Map);
        if f.params.iter().any(unsupported_ty)
            || unsupported_ty(&f.ret)
            || f.reg_tys.iter().any(unsupported_ty)
        {
            return Err(NativeError(format!(
                "function `{}` uses list or map values, which the native backend does not support yet; run with the bytecode VM instead",
                f.name
            )));
        }
        for b in &f.blocks {
            for instr in &b.instrs {
                match instr {
                    IrInstr::Builtin { kind, .. } => {
                        return Err(NativeError(format!(
                            "function `{}` uses the built-in `{}`, which the native backend does not support yet; run with the bytecode VM instead",
                            f.name,
                            kind.name()
                        )));
                    }
                    IrInstr::Print {
                        ty: Ty::List | Ty::Map,
                        ..
                    } => {
                        return Err(NativeError(format!(
                            "function `{}` prints a list or map, which the native backend does not support yet; run with the bytecode VM instead",
                            f.name
                        )));
                    }
                    _ => {}
                }
            }
        }
    }
    Ok(())
}

fn bin_int_cc(op: BinOp) -> IntCC {
    match op {
        BinOp::Eq => IntCC::Equal,
        BinOp::Ne => IntCC::NotEqual,
        BinOp::Lt => IntCC::SignedLessThan,
        BinOp::Le => IntCC::SignedLessThanOrEqual,
        BinOp::Gt => IntCC::SignedGreaterThan,
        BinOp::Ge => IntCC::SignedGreaterThanOrEqual,
        _ => unreachable!("non-comparison op"),
    }
}

fn bin_float_cc(op: BinOp) -> FloatCC {
    match op {
        BinOp::Eq => FloatCC::Equal,
        BinOp::Ne => FloatCC::NotEqual,
        BinOp::Lt => FloatCC::LessThan,
        BinOp::Le => FloatCC::LessThanOrEqual,
        BinOp::Gt => FloatCC::GreaterThan,
        BinOp::Ge => FloatCC::GreaterThanOrEqual,
        _ => unreachable!("non-comparison op"),
    }
}

pub struct Backend<'m> {
    module: ObjectModule,
    func_ids: Vec<cranelift_module::FuncId>,
    func_sigs: Vec<cranelift_codegen::ir::Signature>,
    rt_ids: std::collections::HashMap<&'static str, cranelift_module::FuncId>,
    ir: &'m IrModule,
}

impl<'m> Backend<'m> {
    pub fn new(
        isa: std::sync::Arc<dyn isa::TargetIsa>,
        ir: &'m IrModule,
    ) -> Result<Self, NativeError> {
        check_supported(ir)?;
        let builder = ObjectBuilder::new(
            isa,
            "lutetium".to_string(),
            Box::new(|_libcall| {
                // Our code does not use Cranelift libcalls.
                String::new()
            }),
        )
        .map_err(|e| NativeError(e.to_string()))?;
        let mut module = ObjectModule::new(builder);

        let mut rt_ids = std::collections::HashMap::new();
        let rt_sigs: &[(&'static str, &[IrType], &[IrType])] = &[
            (RT_PRINT_I64, &[types::I64], &[]),
            (RT_PRINT_F64, &[types::F64], &[]),
            (RT_PRINT_BOOL, &[types::I32], &[]),
            (RT_PRINT_STR, &[types::I64], &[]),
            (RT_CONCAT, &[types::I64, types::I64], &[types::I64]),
            (RT_STR_EQ, &[types::I64, types::I64], &[types::I8]),
            (RT_PANIC_DIV0, &[], &[]),
            (RT_PANIC_OVF, &[types::I32], &[]),
        ];
        for (name, params, results) in rt_sigs {
            let mut sig = module.make_signature();
            sig.call_conv = CallConv::WindowsFastcall;
            sig.params = params.iter().map(|t| AbiParam::new(*t)).collect();
            sig.returns = results.iter().map(|t| AbiParam::new(*t)).collect();
            let id = module
                .declare_function(name, Linkage::Import, &sig)
                .map_err(|e| NativeError(e.to_string()))?;
            rt_ids.insert(*name, id);
        }

        let mut func_ids = Vec::with_capacity(ir.funcs.len());
        let mut func_sigs = Vec::with_capacity(ir.funcs.len());
        for (i, f) in ir.funcs.iter().enumerate() {
            let is_entry = i == ir.entry;
            let mut sig = module.make_signature();
            sig.call_conv = CallConv::WindowsFastcall;
            for p in &f.params {
                sig.params.push(AbiParam::new(cl_type(*p)));
            }
            if is_entry {
                // `int main(void)` for the C runtime startup
                sig.returns.push(AbiParam::new(types::I32));
            } else if f.ret != Ty::Unit {
                sig.returns.push(AbiParam::new(cl_type(f.ret)));
            }
            let symbol = if is_entry { "main" } else { f.name.as_str() };
            let id = module
                .declare_function(symbol, Linkage::Export, &sig)
                .map_err(|e| NativeError(e.to_string()))?;
            func_ids.push(id);
            func_sigs.push(sig);
        }

        Ok(Backend {
            module,
            func_ids,
            func_sigs,
            rt_ids,
            ir,
        })
    }

    pub fn translate(mut self) -> Result<Self, NativeError> {
        let mut ctx = FunctionBuilderContext::new();
        for (i, f) in self.ir.funcs.iter().enumerate() {
            let is_entry = i == self.ir.entry;
            let sig = self.func_sigs[i].clone();
            let mut func = cranelift_codegen::ir::Function::with_name_signature(
                UserFuncName::user(0, i as u32),
                sig,
            );
            let mut tr = FnTranslator {
                f,
                is_entry,
                blocks: Vec::new(),
            };
            tr.translate(
                &mut func,
                &mut ctx,
                &mut self.module,
                &self.rt_ids,
                &self.func_ids,
            )
            .map_err(NativeError)?;
            let mut cctx = cranelift_codegen::Context::for_function(func);
            self.module
                .define_function(self.func_ids[i], &mut cctx)
                .map_err(|e| NativeError(e.to_string()))?;
        }
        Ok(self)
    }

    pub fn finish(self) -> Result<Vec<u8>, NativeError> {
        let product = self.module.finish();
        product
            .object
            .write()
            .map_err(|e| NativeError(e.to_string()))
    }
}

struct FnTranslator<'a> {
    f: &'a IrFunction,
    is_entry: bool,
    blocks: Vec<cranelift_codegen::ir::Block>,
}

impl<'a> FnTranslator<'a> {
    fn var(&self, reg: u16) -> Variable {
        Variable::from_u32(reg as u32)
    }

    fn translate(
        &mut self,
        func: &mut cranelift_codegen::ir::Function,
        ctx: &mut FunctionBuilderContext,
        module: &mut ObjectModule,
        rt: &std::collections::HashMap<&'static str, cranelift_module::FuncId>,
        func_ids: &[cranelift_module::FuncId],
    ) -> Result<(), String> {
        let mut builder = FunctionBuilder::new(func, ctx);

        // One eblock per IR block, in IR order.
        for _ in 0..self.f.blocks.len() {
            let b = builder.create_block();
            self.blocks.push(b);
        }
        let entry = self.blocks[0];
        builder.switch_to_block(entry);
        builder.append_block_params_for_function_params(entry);
        builder.seal_block(entry);

        // Declare one variable per register with its exact type.
        for (r, ty) in self.f.reg_tys.iter().enumerate() {
            let v = self.var(r as u16);
            builder.declare_var(v, cl_type(*ty));
        }

        // Bind function parameters to their variables.
        let params: Vec<Value> = builder.block_params(entry).to_vec();
        for (i, p) in params.iter().enumerate() {
            builder.def_var(self.var(i as u16), *p);
        }

        for (bi, block) in self.f.blocks.iter().enumerate() {
            builder.switch_to_block(self.blocks[bi]);
            for instr in &block.instrs {
                self.instr(&mut builder, instr, module, rt, func_ids)?;
            }
            match &block.term {
                Some(IrTerm::Jmp(b)) => {
                    builder.ins().jump(self.blocks[*b], &[]);
                }
                Some(IrTerm::Br { cond, then, els }) => {
                    let c = self.load(&mut builder, *cond);
                    builder
                        .ins()
                        .brif(c, self.blocks[*then], &[], self.blocks[*els], &[]);
                }
                Some(IrTerm::Ret(r)) => {
                    if self.is_entry {
                        let zero = builder.ins().iconst(types::I32, 0);
                        builder.ins().return_(&[zero]);
                    } else if self.f.ret == Ty::Unit {
                        builder.ins().return_(&[]);
                    } else {
                        let v = self.load(&mut builder, *r);
                        builder.ins().return_(&[v]);
                    }
                }
                Some(IrTerm::RetUnit) => {
                    if self.is_entry {
                        let zero = builder.ins().iconst(types::I32, 0);
                        builder.ins().return_(&[zero]);
                    } else {
                        builder.ins().return_(&[]);
                    }
                }
                None => {
                    // unreachable code after break/continue: safe terminator
                    builder.ins().trap(TrapCode::user(1).unwrap());
                }
            }
        }
        builder.seal_all_blocks();
        builder.finalize();
        Ok(())
    }

    fn load(&self, builder: &mut FunctionBuilder, reg: u16) -> Value {
        builder.use_var(self.var(reg))
    }

    fn store(&mut self, builder: &mut FunctionBuilder, reg: u16, val: Value) {
        builder.def_var(self.var(reg), val);
    }

    fn instr(
        &mut self,
        builder: &mut FunctionBuilder,
        instr: &IrInstr,
        module: &mut ObjectModule,
        rt: &std::collections::HashMap<&'static str, cranelift_module::FuncId>,
        func_ids: &[cranelift_module::FuncId],
    ) -> Result<(), String> {
        match instr {
            IrInstr::Const { v, dst } => match v {
                Const::Int(i) => {
                    let x = builder.ins().iconst(types::I64, *i);
                    self.store(builder, *dst, x);
                }
                Const::Float(fl) => {
                    let x = builder.ins().f64const(*fl);
                    self.store(builder, *dst, x);
                }
                Const::Bool(b) => {
                    let x = builder.ins().iconst(types::I8, *b as i64);
                    self.store(builder, *dst, x);
                }
                Const::Str(s) => {
                    let mut bytes = Vec::with_capacity(8 + s.len());
                    bytes.extend_from_slice(&(s.len() as u64).to_le_bytes());
                    bytes.extend_from_slice(s.as_bytes());
                    let data_id = module
                        .declare_anonymous_data(false, false)
                        .map_err(|e| e.to_string())?;
                    let mut desc = cranelift_module::DataDescription::new();
                    desc.set_align(8);
                    desc.define(bytes.into());
                    module
                        .define_data(data_id, &desc)
                        .map_err(|e| e.to_string())?;
                    let gv = module.declare_data_in_func(data_id, builder.func);
                    let ptr = builder.ins().global_value(types::I64, gv);
                    self.store(builder, *dst, ptr);
                }
                Const::Unit => {
                    let x = builder.ins().iconst(types::I8, 0);
                    self.store(builder, *dst, x);
                }
            },
            IrInstr::Move { src, dst } => {
                let v = self.load(builder, *src);
                self.store(builder, *dst, v);
            }
            IrInstr::Builtin { kind, .. } => {
                return Err(format!(
                    "builtin `{}` reached the native backend (internal error: check_supported should have rejected the module)",
                    kind.name()
                ));
            }
            IrInstr::Bin { op, ty, a, b, dst } => {
                let av = self.load(builder, *a);
                let bv = self.load(builder, *b);
                let r = match ty {
                    Ty::List | Ty::Map => {
                        return Err("list/map arithmetic reached the native backend (internal error: check_supported should have rejected the module)".to_string());
                    }
                    Ty::Int => match op {
                        BinOp::Add | BinOp::Sub | BinOp::Mul => {
                            let raw = match op {
                                BinOp::Add => builder.ins().iadd(av, bv),
                                BinOp::Sub => builder.ins().isub(av, bv),
                                _ => builder.ins().imul(av, bv),
                            };
                            let ovf = match op {
                                BinOp::Mul => {
                                    let hi = builder.ins().smulhi(av, bv);
                                    let sign_low = builder.ins().sshr_imm(raw, 63);
                                    builder.ins().icmp(IntCC::NotEqual, hi, sign_low)
                                }
                                _ => {
                                    let aneg = builder.ins().icmp_imm(IntCC::SignedLessThan, av, 0);
                                    let bneg = builder.ins().icmp_imm(IntCC::SignedLessThan, bv, 0);
                                    let rneg =
                                        builder.ins().icmp_imm(IntCC::SignedLessThan, raw, 0);
                                    // Add overflows when operand signs agree and the
                                    // result flips; Sub overflows when they differ
                                    // and the result flips away from the minuend.
                                    let op_match = if *op == BinOp::Add {
                                        builder.ins().icmp(IntCC::Equal, aneg, bneg)
                                    } else {
                                        builder.ins().icmp(IntCC::NotEqual, aneg, bneg)
                                    };
                                    let differs = builder.ins().icmp(IntCC::NotEqual, rneg, aneg);
                                    builder.ins().band(op_match, differs)
                                }
                            };
                            let which = match op {
                                BinOp::Add => 0i64,
                                BinOp::Sub => 1,
                                _ => 2,
                            };
                            self.guard(builder, module, ovf, Some(which), rt)?;
                            raw
                        }
                        BinOp::Div => {
                            self.guard_zero(builder, module, bv, rt)?;
                            builder.ins().sdiv(av, bv)
                        }
                        BinOp::Mod => {
                            self.guard_zero(builder, module, bv, rt)?;
                            builder.ins().srem(av, bv)
                        }
                        _ => builder.ins().icmp(bin_int_cc(*op), av, bv),
                    },
                    Ty::Float => match op {
                        BinOp::Add => builder.ins().fadd(av, bv),
                        BinOp::Sub => builder.ins().fsub(av, bv),
                        BinOp::Mul => builder.ins().fmul(av, bv),
                        BinOp::Div => builder.ins().fdiv(av, bv),
                        _ => builder.ins().fcmp(bin_float_cc(*op), av, bv),
                    },
                    Ty::Str => match op {
                        BinOp::Add => {
                            let fref = module.declare_func_in_func(rt[RT_CONCAT], builder.func);
                            let call = builder.ins().call(fref, &[av, bv]);
                            builder.inst_results(call)[0]
                        }
                        BinOp::Eq | BinOp::Ne => {
                            let fref = module.declare_func_in_func(rt[RT_STR_EQ], builder.func);
                            let call = builder.ins().call(fref, &[av, bv]);
                            let eq = builder.inst_results(call)[0];
                            if *op == BinOp::Ne {
                                builder.ins().bxor_imm(eq, 1)
                            } else {
                                eq
                            }
                        }
                        _ => unreachable!("invalid string op"),
                    },
                    Ty::Bool => match op {
                        BinOp::Eq => builder.ins().icmp(IntCC::Equal, av, bv),
                        _ => builder.ins().icmp(bin_int_cc(*op), av, bv),
                    },
                    Ty::Unit | Ty::Window | Ty::Widget => unreachable!("non-scalar arithmetic"),
                };
                self.store(builder, *dst, r);
            }
            IrInstr::Un { op, ty, a, dst } => {
                let av = self.load(builder, *a);
                let r = match (op, ty) {
                    (UnOp::Not, _) => builder.ins().bxor_imm(av, 1),
                    (UnOp::Neg, Ty::Int) => builder.ins().ineg(av),
                    (UnOp::Neg, Ty::Float) => builder.ins().fneg(av),
                    _ => unreachable!(),
                };
                self.store(builder, *dst, r);
            }
            IrInstr::IntToFloat { a, dst } => {
                let av = self.load(builder, *a);
                let r = builder.ins().fcvt_from_sint(types::F64, av);
                self.store(builder, *dst, r);
            }
            IrInstr::Call { func, args, dst } => {
                let fref = module.declare_func_in_func(func_ids[*func], builder.func);
                let mut argv: Vec<Value> = Vec::with_capacity(args.len());
                for a in args.iter() {
                    argv.push(self.load(builder, *a));
                }
                let call = builder.ins().call(fref, &argv);
                // dst is always written so the register stays defined; for
                // unit-returning callees it holds a dummy.
                let r = if builder.inst_results(call).is_empty() {
                    builder.ins().iconst(types::I8, 0)
                } else {
                    builder.inst_results(call)[0]
                };
                self.store(builder, *dst, r);
            }
            IrInstr::Print { src, ty } => {
                let v = self.load(builder, *src);
                let (name, arg) = match ty {
                    Ty::Int => (RT_PRINT_I64, v),
                    Ty::Float => (RT_PRINT_F64, v),
                    Ty::Bool => {
                        let wide = builder.ins().uextend(types::I32, v);
                        (RT_PRINT_BOOL, wide)
                    }
                    Ty::Str => (RT_PRINT_STR, v),
                    Ty::Unit => (RT_PRINT_I64, v), // unreachable in practice
                    Ty::List | Ty::Map | Ty::Window | Ty::Widget => {
                        return Err("container/GUI handle print is unsupported".to_string());
                    }
                };
                let fref = module.declare_func_in_func(rt[name], builder.func);
                builder.ins().call(fref, &[arg]);
            }
        }
        Ok(())
    }

    /// Branch on `bad`; if true, call the panic runtime function (which never
    /// returns) and trap; otherwise continue in a fresh block.
    #[allow(clippy::too_many_arguments)]
    fn guard(
        &mut self,
        builder: &mut FunctionBuilder,
        module: &mut ObjectModule,
        bad: Value,
        overflow_op: Option<i64>,
        rt: &std::collections::HashMap<&'static str, cranelift_module::FuncId>,
    ) -> Result<(), String> {
        let cont = builder.create_block();
        let panic_b = builder.create_block();
        builder.ins().brif(bad, panic_b, &[], cont, &[]);
        builder.switch_to_block(panic_b);
        match overflow_op {
            None => {
                let fref = module.declare_func_in_func(rt[RT_PANIC_DIV0], builder.func);
                builder.ins().call(fref, &[]);
            }
            Some(which) => {
                let fref = module.declare_func_in_func(rt[RT_PANIC_OVF], builder.func);
                let w = builder.ins().iconst(types::I32, which);
                builder.ins().call(fref, &[w]);
            }
        }
        builder.ins().trap(TrapCode::user(1).unwrap());
        builder.switch_to_block(cont);
        Ok(())
    }

    fn guard_zero(
        &mut self,
        builder: &mut FunctionBuilder,
        module: &mut ObjectModule,
        b: Value,
        rt: &std::collections::HashMap<&'static str, cranelift_module::FuncId>,
    ) -> Result<(), String> {
        let is_zero = builder.ins().icmp_imm(IntCC::Equal, b, 0);
        self.guard(builder, module, is_zero, None, rt)
    }
}
