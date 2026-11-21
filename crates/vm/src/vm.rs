//! Register-machine bytecode interpreter.
//!
//! Execution model:
//! * one register file per call frame, sized `nregs`,
//! * registers are pre-filled with `Value::Uninit` so that reading a register
//!   the verifier could not prove written is always a clean runtime error,
//! * integer arithmetic is checked (overflow and division by zero are runtime
//!   errors, never wraps/panics),
//! * call depth is capped.

use crate::bytecode::{op, BcConst, BcFunction, BuiltinKind, BytecodeModule};
use crate::heap::{GcStats, Heap, Obj};
use std::rc::Rc;

const MAX_CALL_DEPTH: usize = 1024;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Uninit,
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(Rc<str>),
    /// GC-managed list (handle into the VM heap).
    List(u32),
    /// GC-managed map (handle into the VM heap).
    Map(u32),
    Unit,
}

impl Value {
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Uninit => "uninitialized",
            Value::Int(_) => "int",
            Value::Float(_) => "float",
            Value::Bool(_) => "bool",
            Value::Str(_) => "str",
            Value::List(_) => "list",
            Value::Map(_) => "map",
            Value::Unit => "unit",
        }
    }

    /// The GC handle if this value is a traced container.
    pub fn gc_ref(&self) -> Option<u32> {
        match self {
            Value::List(r) | Value::Map(r) => Some(*r),
            _ => None,
        }
    }
}

/// How a value prints via the `print` builtin.
pub fn format_value(v: &Value) -> String {
    match v {
        Value::Uninit => "<uninitialized>".to_string(),
        Value::Int(i) => i.to_string(),
        Value::Float(f) => {
            let s = format!("{}", f);
            if s.contains('.')
                || s.contains('e')
                || s.contains("inf")
                || s.contains("NaN")
                || s.contains("nan")
            {
                s
            } else {
                s + ".0"
            }
        }
        Value::Bool(b) => b.to_string(),
        Value::Str(s) => s.to_string(),
        Value::List(r) => {
            // `r` is a heap handle; formatting lists requires heap access and
            // is done by the VM's list-printing helper, not here.
            format!("<list {}>", r)
        }
        Value::Map(r) => format!("<map {}>", r),
        Value::Unit => "()".to_string(),
    }
}

/// Deep list equality: same length, element-wise equality with the usual
/// int/float numeric widening. `depth` guards against hand-crafted bytecode
/// building arbitrarily nested lists (the compiler rejects nesting); real
/// programs never recurse here.
fn list_deep_eq(a: &Value, b: &Value, heap: &mut Heap, depth: u32) -> Result<bool, String> {
    const MAX_DEPTH: u32 = 64;
    if depth > MAX_DEPTH {
        return Err("list comparison nested too deeply".to_string());
    }
    let (la, lb) = match (a, b) {
        (Value::List(ra), Value::List(rb)) => match (heap.get(*ra), heap.get(*rb)) {
            (Obj::List(x), Obj::List(y)) => (x.clone(), y.clone()),
            _ => unreachable!("List value must point at a list object"),
        },
        _ => unreachable!("list_deep_eq called on non-list values"),
    };
    if la.len() != lb.len() {
        return Ok(false);
    }
    for (x, y) in la.iter().zip(lb.iter()) {
        let eq = match (x, y) {
            (Value::Int(x), Value::Int(y)) => x == y,
            (Value::Float(x), Value::Float(y)) => x == y,
            // int/float element pairs compare numerically (implicit widening).
            (Value::Int(x), Value::Float(y)) | (Value::Float(y), Value::Int(x)) => {
                (*x as f64) == *y
            }
            (Value::Bool(x), Value::Bool(y)) => x == y,
            (Value::Str(x), Value::Str(y)) => x == y,
            (Value::List(_), Value::List(_)) => return list_deep_eq(x, y, heap, depth + 1),
            // Different element types (or unit/uninit) are simply unequal;
            // the checker rejects such comparisons before they execute.
            _ => false,
        };
        if !eq {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Formatting with GC containers rendered (`[1, 2, 3]`).
fn format_with_heap(v: &Value, heap: &Heap) -> String {
    match v {
        Value::List(r) => {
            let items = match heap.get(*r) {
                Obj::List(items) => items.clone(),
                _ => unreachable!(),
            };
            let parts: Vec<String> = items.iter().map(|i| format_with_heap(i, heap)).collect();
            format!("[{}]", parts.join(", "))
        }
        Value::Map(r) => {
            let pairs = match heap.get(*r) {
                Obj::Map(pairs) => pairs.clone(),
                _ => unreachable!(),
            };
            let parts: Vec<String> = pairs
                .iter()
                .map(|(k, val)| {
                    format!(
                        "{}: {}",
                        format_with_heap(k, heap),
                        format_with_heap(val, heap)
                    )
                })
                .collect();
            format!("{{{}}}", parts.join(", "))
        }
        other => format_value(other),
    }
}

#[derive(Debug, Clone)]
pub struct VmError {
    pub code: &'static str,
    pub message: String,
    pub func: String,
    pub offset: usize,
}

impl VmError {
    fn new(code: &'static str, func: &str, offset: usize, message: String) -> VmError {
        VmError {
            code,
            message,
            func: func.to_string(),
            offset,
        }
    }
}

impl std::fmt::Display for VmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{}] {} (in `{}`, offset {})",
            self.code, self.message, self.func, self.offset
        )
    }
}

struct Frame {
    func: usize,
    ip: usize,
    ret_dst: Option<u16>,
    ret_ip: usize,
    regs: Vec<Value>,
}

/// Read helpers. The verifier has already proven these are in range; the
/// bounds checks are kept as a defensive measure for VM-internal bugs.
fn rd_u8(code: &[u8], p: &mut usize) -> u8 {
    let v = code[*p];
    *p += 1;
    v
}

fn rd_u16(code: &[u8], p: &mut usize) -> u16 {
    let v = u16::from_le_bytes([code[*p], code[*p + 1]]);
    *p += 2;
    v
}

fn rd_u32(code: &[u8], p: &mut usize) -> u32 {
    let v = u32::from_le_bytes([code[*p], code[*p + 1], code[*p + 2], code[*p + 3]]);
    *p += 4;
    v
}

fn rd_i32(code: &[u8], p: &mut usize) -> i32 {
    rd_u32(code, p) as i32
}

/// VM execution options (GC configuration).
#[derive(Debug, Clone, Copy, Default)]
pub struct VmOptions {
    /// Collect on every allocation (stress testing).
    pub gc_stress: bool,
}

/// Execute the module's entry function. `out` receives each `print` line,
/// newline included. Returns the entry value and final GC statistics.
pub fn run_with_options(
    m: &BytecodeModule,
    mut out: impl FnMut(&str),
    opts: VmOptions,
) -> Result<(Value, GcStats), VmError> {
    let entry = m.entry;
    let ef = &m.funcs[entry];
    let regs = vec![Value::Uninit; ef.nregs as usize];
    let mut frames: Vec<Frame> = vec![Frame {
        func: entry,
        ip: 0,
        ret_dst: None,
        ret_ip: 0,
        regs,
    }];
    let mut heap = Heap::new();
    heap.stress = opts.gc_stress;

    loop {
        let fi = frames.last().unwrap().func;
        let ip0 = frames.last().unwrap().ip;
        let f: &BcFunction = &m.funcs[fi];
        let code = &f.code;
        if ip0 >= code.len() {
            return Err(VmError::new(
                "E5003",
                &f.name,
                ip0,
                "instruction pointer out of range (bytecode not verified?)".into(),
            ));
        }
        let opb = code[ip0];
        let mut p = ip0 + 1;
        let fname = f.name.clone();

        macro_rules! err {
            ($code:expr, $($arg:tt)*) => {
                return Err(VmError::new($code, &fname, ip0, format!($($arg)*)))
            };
        }

        macro_rules! regs {
            () => {
                &mut frames.last_mut().unwrap().regs
            };
        }

        macro_rules! bin_num {
            ($a:expr, $b:expr, $intop:expr, $floatop:expr, $sym:expr) => {{
                let a = frames.last().unwrap().regs[$a].clone();
                let b = frames.last().unwrap().regs[$b].clone();
                let v = match (&a, &b) {
                    (Value::Int(x), Value::Int(y)) => {
                        let f: fn(i64, i64) -> Option<i64> = $intop;
                        match f(*x, *y) {
                            Some(r) => Value::Int(r),
                            None => err!("E5005", "integer {} overflow ({} {} {})", $sym, x, $sym, y),
                        }
                    }
                    (Value::Float(x), Value::Float(y)) => {
                        let g: fn(f64, f64) -> f64 = $floatop;
                        Value::Float(g(*x, *y))
                    }
                    _ => err!(
                        "E5004",
                        "type error: `{}` expects two operands of the same numeric type, found {} and {}",
                        $sym,
                        a.type_name(),
                        b.type_name()
                    ),
                };
                v
            }};
        }

        match opb {
            op::CONST_INT => {
                let ci = rd_u32(code, &mut p) as usize;
                let d = rd_u16(code, &mut p) as usize;
                match m.consts.get(ci) {
                    Some(BcConst::Int(v)) => regs!()[d] = Value::Int(*v),
                    _ => err!("E5003", "constant {} is not an int", ci),
                }
            }
            op::CONST_FLOAT => {
                let ci = rd_u32(code, &mut p) as usize;
                let d = rd_u16(code, &mut p) as usize;
                match m.consts.get(ci) {
                    Some(BcConst::Float(v)) => regs!()[d] = Value::Float(*v),
                    _ => err!("E5003", "constant {} is not a float", ci),
                }
            }
            op::CONST_BOOL => {
                let ci = rd_u32(code, &mut p) as usize;
                let d = rd_u16(code, &mut p) as usize;
                match m.consts.get(ci) {
                    Some(BcConst::Bool(v)) => regs!()[d] = Value::Bool(*v),
                    _ => err!("E5003", "constant {} is not a bool", ci),
                }
            }
            op::CONST_STR => {
                let ci = rd_u32(code, &mut p) as usize;
                let d = rd_u16(code, &mut p) as usize;
                match m.consts.get(ci) {
                    Some(BcConst::Str(s)) => regs!()[d] = Value::Str(Rc::from(s.as_str())),
                    _ => err!("E5003", "constant {} is not a string", ci),
                }
            }
            op::CONST_UNIT => {
                let d = rd_u16(code, &mut p) as usize;
                regs!()[d] = Value::Unit;
            }
            op::MOVE => {
                let a = rd_u16(code, &mut p) as usize;
                let d = rd_u16(code, &mut p) as usize;
                let v = frames.last().unwrap().regs[a].clone();
                check_init(&v).map_err(|msg| VmError::new("E5004", &fname, ip0, msg))?;
                regs!()[d] = v;
            }
            op::ADD_INT => {
                let (a, b, d) = (
                    rd_u16(code, &mut p) as usize,
                    rd_u16(code, &mut p) as usize,
                    rd_u16(code, &mut p) as usize,
                );
                regs!()[d] = bin_num!(a, b, |x, y| x.checked_add(y), |x, y| x + y, "+");
            }
            op::SUB_INT => {
                let (a, b, d) = (
                    rd_u16(code, &mut p) as usize,
                    rd_u16(code, &mut p) as usize,
                    rd_u16(code, &mut p) as usize,
                );
                regs!()[d] = bin_num!(a, b, |x, y| x.checked_sub(y), |x, y| x - y, "-");
            }
            op::MUL_INT => {
                let (a, b, d) = (
                    rd_u16(code, &mut p) as usize,
                    rd_u16(code, &mut p) as usize,
                    rd_u16(code, &mut p) as usize,
                );
                regs!()[d] = bin_num!(a, b, |x, y| x.checked_mul(y), |x, y| x * y, "*");
            }
            op::DIV_INT => {
                let (a, b, d) = (
                    rd_u16(code, &mut p) as usize,
                    rd_u16(code, &mut p) as usize,
                    rd_u16(code, &mut p) as usize,
                );
                let bv = frames.last().unwrap().regs[b].clone();
                if let Value::Int(0) = bv {
                    err!("E5002", "integer division by zero");
                }
                regs!()[d] = bin_num!(a, b, |x, y| x.checked_div(y), |x, y| x / y, "/");
            }
            op::MOD_INT => {
                let (a, b, d) = (
                    rd_u16(code, &mut p) as usize,
                    rd_u16(code, &mut p) as usize,
                    rd_u16(code, &mut p) as usize,
                );
                let bv = frames.last().unwrap().regs[b].clone();
                if let Value::Int(0) = bv {
                    err!("E5002", "integer modulo by zero");
                }
                regs!()[d] = bin_num!(a, b, |x, y| x.checked_rem(y), |x, y| x % y, "%");
            }
            op::ADD_FLOAT => {
                let (a, b, d) = (
                    rd_u16(code, &mut p) as usize,
                    rd_u16(code, &mut p) as usize,
                    rd_u16(code, &mut p) as usize,
                );
                regs!()[d] = bin_num!(a, b, |x, y| x.checked_add(y), |x, y| x + y, "+");
            }
            op::SUB_FLOAT => {
                let (a, b, d) = (
                    rd_u16(code, &mut p) as usize,
                    rd_u16(code, &mut p) as usize,
                    rd_u16(code, &mut p) as usize,
                );
                regs!()[d] = bin_num!(a, b, |x, y| x.checked_sub(y), |x, y| x - y, "-");
            }
            op::MUL_FLOAT => {
                let (a, b, d) = (
                    rd_u16(code, &mut p) as usize,
                    rd_u16(code, &mut p) as usize,
                    rd_u16(code, &mut p) as usize,
                );
                regs!()[d] = bin_num!(a, b, |x, y| x.checked_mul(y), |x, y| x * y, "*");
            }
            op::DIV_FLOAT => {
                let (a, b, d) = (
                    rd_u16(code, &mut p) as usize,
                    rd_u16(code, &mut p) as usize,
                    rd_u16(code, &mut p) as usize,
                );
                regs!()[d] = bin_num!(a, b, |x, y| Some(x / y), |x, y| x / y, "/");
            }
            op::INT_TO_FLOAT => {
                let a = rd_u16(code, &mut p) as usize;
                let d = rd_u16(code, &mut p) as usize;
                let v = frames.last().unwrap().regs[a].clone();
                match v {
                    Value::Int(x) => regs!()[d] = Value::Float(x as f64),
                    other => err!(
                        "E5004",
                        "type error: int-to-float conversion expects an int, found {}",
                        other.type_name()
                    ),
                }
            }
            op::CONCAT_STR => {
                let (a, b, d) = (
                    rd_u16(code, &mut p) as usize,
                    rd_u16(code, &mut p) as usize,
                    rd_u16(code, &mut p) as usize,
                );
                let av = frames.last().unwrap().regs[a].clone();
                let bv = frames.last().unwrap().regs[b].clone();
                match (&av, &bv) {
                    (Value::Str(x), Value::Str(y)) => {
                        let s: String = format!("{}{}", x, y);
                        regs!()[d] = Value::Str(Rc::from(s.as_str()));
                    }
                    _ => err!(
                        "E5004",
                        "type error: string concatenation expects two strings, found {} and {}",
                        av.type_name(),
                        bv.type_name()
                    ),
                }
            }
            op::EQ_INT | op::NE_INT | op::LT_INT | op::LE_INT | op::GT_INT | op::GE_INT => {
                let (a, b, d) = (
                    rd_u16(code, &mut p) as usize,
                    rd_u16(code, &mut p) as usize,
                    rd_u16(code, &mut p) as usize,
                );
                let av = frames.last().unwrap().regs[a].clone();
                let bv = frames.last().unwrap().regs[b].clone();
                let r = match (&av, &bv) {
                    (Value::Int(x), Value::Int(y)) => Value::Bool(match opb {
                        op::EQ_INT => x == y,
                        op::NE_INT => x != y,
                        op::LT_INT => x < y,
                        op::LE_INT => x <= y,
                        op::GT_INT => x > y,
                        _ => x >= y,
                    }),
                    _ => err!(
                        "E5004",
                        "type error: integer comparison expects two ints, found {} and {}",
                        av.type_name(),
                        bv.type_name()
                    ),
                };
                regs!()[d] = r;
            }
            op::EQ_FLOAT
            | op::NE_FLOAT
            | op::LT_FLOAT
            | op::LE_FLOAT
            | op::GT_FLOAT
            | op::GE_FLOAT => {
                let (a, b, d) = (
                    rd_u16(code, &mut p) as usize,
                    rd_u16(code, &mut p) as usize,
                    rd_u16(code, &mut p) as usize,
                );
                let av = frames.last().unwrap().regs[a].clone();
                let bv = frames.last().unwrap().regs[b].clone();
                let r = match (&av, &bv) {
                    (Value::Float(x), Value::Float(y)) => Value::Bool(match opb {
                        op::EQ_FLOAT => x == y,
                        op::NE_FLOAT => x != y,
                        op::LT_FLOAT => x < y,
                        op::LE_FLOAT => x <= y,
                        op::GT_FLOAT => x > y,
                        _ => x >= y,
                    }),
                    _ => err!(
                        "E5004",
                        "type error: float comparison expects two floats, found {} and {}",
                        av.type_name(),
                        bv.type_name()
                    ),
                };
                regs!()[d] = r;
            }
            op::EQ_STR | op::NE_STR => {
                let (a, b, d) = (
                    rd_u16(code, &mut p) as usize,
                    rd_u16(code, &mut p) as usize,
                    rd_u16(code, &mut p) as usize,
                );
                let av = frames.last().unwrap().regs[a].clone();
                let bv = frames.last().unwrap().regs[b].clone();
                match (&av, &bv) {
                    (Value::Str(x), Value::Str(y)) => {
                        let eq = x == y;
                        regs!()[d] = Value::Bool(if opb == op::EQ_STR { eq } else { !eq });
                    }
                    _ => err!(
                        "E5004",
                        "type error: string comparison expects two strings, found {} and {}",
                        av.type_name(),
                        bv.type_name()
                    ),
                }
            }
            op::EQ_BOOL | op::NE_BOOL => {
                let (a, b, d) = (
                    rd_u16(code, &mut p) as usize,
                    rd_u16(code, &mut p) as usize,
                    rd_u16(code, &mut p) as usize,
                );
                let av = frames.last().unwrap().regs[a].clone();
                let bv = frames.last().unwrap().regs[b].clone();
                match (&av, &bv) {
                    (Value::Bool(x), Value::Bool(y)) => {
                        let eq = x == y;
                        regs!()[d] = Value::Bool(if opb == op::EQ_BOOL { eq } else { !eq });
                    }
                    _ => err!(
                        "E5004",
                        "type error: bool comparison expects two bools, found {} and {}",
                        av.type_name(),
                        bv.type_name()
                    ),
                }
            }
            op::EQ_LIST | op::NE_LIST => {
                let (a, b, d) = (
                    rd_u16(code, &mut p) as usize,
                    rd_u16(code, &mut p) as usize,
                    rd_u16(code, &mut p) as usize,
                );
                let av = frames.last().unwrap().regs[a].clone();
                let bv = frames.last().unwrap().regs[b].clone();
                if !matches!(av, Value::List(_)) || !matches!(bv, Value::List(_)) {
                    err!(
                        "E5004",
                        "type error: list comparison expects two lists, found {} and {}",
                        av.type_name(),
                        bv.type_name()
                    );
                }
                let eq = list_deep_eq(&av, &bv, &mut heap, 0)
                    .map_err(|msg| VmError::new("E5009", &fname, ip0, msg))?;
                regs!()[d] = Value::Bool(if opb == op::EQ_LIST { eq } else { !eq });
            }
            op::NOT => {
                let a = rd_u16(code, &mut p) as usize;
                let d = rd_u16(code, &mut p) as usize;
                let v = frames.last().unwrap().regs[a].clone();
                match v {
                    Value::Bool(b) => regs!()[d] = Value::Bool(!b),
                    other => err!(
                        "E5004",
                        "type error: `!` expects a bool, found {}",
                        other.type_name()
                    ),
                }
            }
            op::NEG_INT => {
                let a = rd_u16(code, &mut p) as usize;
                let d = rd_u16(code, &mut p) as usize;
                let v = frames.last().unwrap().regs[a].clone();
                match v {
                    Value::Int(x) => match x.checked_neg() {
                        Some(r) => regs!()[d] = Value::Int(r),
                        None => err!("E5005", "integer negation overflow (-{})", x),
                    },
                    other => err!(
                        "E5004",
                        "type error: unary `-` expects an int or float, found {}",
                        other.type_name()
                    ),
                }
            }
            op::NEG_FLOAT => {
                let a = rd_u16(code, &mut p) as usize;
                let d = rd_u16(code, &mut p) as usize;
                let v = frames.last().unwrap().regs[a].clone();
                match v {
                    Value::Float(x) => regs!()[d] = Value::Float(-x),
                    other => err!(
                        "E5004",
                        "type error: unary `-` expects an int or float, found {}",
                        other.type_name()
                    ),
                }
            }
            op::JUMP => {
                let t = rd_i32(code, &mut p);
                frames.last_mut().unwrap().ip = t as usize;
                continue;
            }
            op::JUMP_IF_FALSE => {
                let c = rd_u16(code, &mut p) as usize;
                let t = rd_i32(code, &mut p);
                let v = frames.last().unwrap().regs[c].clone();
                let go = match v {
                    Value::Bool(b) => !b,
                    other => err!(
                        "E5004",
                        "type error: branch condition must be bool, found {}",
                        other.type_name()
                    ),
                };
                if go {
                    frames.last_mut().unwrap().ip = t as usize;
                    continue;
                }
            }
            op::CALL => {
                let callee = rd_u16(code, &mut p) as usize;
                let argc = rd_u8(code, &mut p) as usize;
                let mut args = Vec::with_capacity(argc);
                for _ in 0..argc {
                    let a = rd_u16(code, &mut p) as usize;
                    let v = frames.last().unwrap().regs[a].clone();
                    check_init(&v).map_err(|msg| VmError::new("E5004", &fname, ip0, msg))?;
                    args.push(v);
                }
                let dst = rd_u16(code, &mut p);
                if frames.len() >= MAX_CALL_DEPTH {
                    err!(
                        "E5001",
                        "call stack overflow (depth limit {})",
                        MAX_CALL_DEPTH
                    );
                }
                let cf = &m.funcs[callee];
                let mut cregs = vec![Value::Uninit; cf.nregs as usize];
                for (i, a) in args.into_iter().enumerate() {
                    cregs[i] = a;
                }
                frames.last_mut().unwrap().ip = p;
                frames.push(Frame {
                    func: callee,
                    ip: 0,
                    ret_dst: Some(dst),
                    ret_ip: p,
                    regs: cregs,
                });
                continue;
            }
            op::RET => {
                let a = rd_u16(code, &mut p) as usize;
                let v = frames.last().unwrap().regs[a].clone();
                check_init(&v).map_err(|msg| VmError::new("E5004", &fname, ip0, msg))?;
                let fr = frames.pop().unwrap();
                match frames.last_mut() {
                    Some(parent) => {
                        parent.regs[fr.ret_dst.unwrap_or(0) as usize] = v;
                        parent.ip = fr.ret_ip;
                    }
                    None => return Ok((v, heap.stats())),
                }
                continue;
            }
            op::RET_UNIT => {
                let fr = frames.pop().unwrap();
                match frames.last_mut() {
                    Some(parent) => {
                        parent.regs[fr.ret_dst.unwrap_or(0) as usize] = Value::Unit;
                        parent.ip = fr.ret_ip;
                    }
                    None => return Ok((Value::Unit, heap.stats())),
                }
                continue;
            }
            op::PRINT => {
                let a = rd_u16(code, &mut p) as usize;
                let v = frames.last().unwrap().regs[a].clone();
                check_init(&v).map_err(|msg| VmError::new("E5004", &fname, ip0, msg))?;
                let mut line = format_with_heap(&v, &heap);
                line.push('\n');
                out(&line);
            }
            op::BUILTIN => {
                let kind_b = rd_u8(code, &mut p);
                let dst = rd_u16(code, &mut p) as usize;
                let argc = rd_u8(code, &mut p) as usize;
                let mut argv: Vec<Value> = Vec::with_capacity(argc);
                for _ in 0..argc {
                    let a = rd_u16(code, &mut p) as usize;
                    let v = frames.last().unwrap().regs[a].clone();
                    check_init(&v).map_err(|msg| VmError::new("E5004", &fname, ip0, msg))?;
                    argv.push(v);
                }
                let kind = BuiltinKind::from_u8(kind_b).ok_or_else(|| {
                    VmError::new(
                        "E5003",
                        &fname,
                        ip0,
                        format!("unknown builtin kind 0x{:02x}", kind_b),
                    )
                })?;
                // Collect before operations that allocate in the GC heap.
                if matches!(
                    kind,
                    BuiltinKind::ListNew | BuiltinKind::MapNew | BuiltinKind::StrSplit
                ) {
                    let mut roots: Vec<&mut Vec<Value>> =
                        frames.iter_mut().map(|fr| &mut fr.regs).collect();
                    heap.maybe_collect(&mut roots);
                }
                let result = exec_builtin(&kind, argv, &mut heap).map_err(|msg| {
                    VmError::new(
                        "E5009",
                        &fname,
                        ip0,
                        format!("builtin `{}`: {}", kind.name(), msg),
                    )
                })?;
                regs!()[dst] = result;
            }
            other => {
                err!(
                    "E5003",
                    "unknown opcode 0x{:02x} (bytecode not verified?)",
                    other
                );
            }
        }

        frames.last_mut().unwrap().ip = p;
    }
}

/// Execute a builtin operation. `heap` is used for list/map operations.
fn exec_builtin(kind: &BuiltinKind, argv: Vec<Value>, heap: &mut Heap) -> Result<Value, String> {
    use BuiltinKind::*;
    let arg = |i: usize| argv.get(i).cloned().unwrap_or(Value::Unit);
    Ok(match kind {
        ListNew => Value::List(heap.alloc(Obj::List(argv))),
        MapNew => {
            if !argv.len().is_multiple_of(2) {
                return Err("map_new expects (key, value) pairs".into());
            }
            // Later duplicate keys win (same as `set` overwriting).
            let mut pairs: Vec<(Value, Value)> = Vec::with_capacity(argv.len() / 2);
            for kv in argv.chunks(2) {
                let key = match &kv[0] {
                    Value::Str(k) => k.clone(),
                    other => {
                        return Err(format!(
                            "map keys must be strings, found {}",
                            other.type_name()
                        ))
                    }
                };
                match pairs
                    .iter_mut()
                    .find(|(k, _)| matches!(k, Value::Str(s) if s.as_ref() == key.as_ref()))
                {
                    Some(pair) => pair.1 = kv[1].clone(),
                    None => pairs.push((Value::Str(key), kv[1].clone())),
                }
            }
            Value::Map(heap.alloc(Obj::Map(pairs)))
        }
        MapGet => {
            let r = arg(0).gc_ref().ok_or("expected a map")?;
            let key = str_of(arg(1)).map_err(|_| "map keys must be strings".to_string())?;
            match heap.get(r) {
                Obj::Map(m) => m
                    .iter()
                    .find(|(k, _)| matches!(k, Value::Str(s) if s.as_ref() == key.as_ref()))
                    .map(|(_, v)| v.clone())
                    .ok_or_else(|| format!("map has no key `{}`", key))?,
                Obj::List(_) => return Err("expected a map".into()),
            }
        }
        MapSet => {
            let r = arg(0).gc_ref().ok_or("expected a map")?;
            let key = str_of(arg(1)).map_err(|_| "map keys must be strings".to_string())?;
            let v = arg(2);
            match heap.get_mut(r) {
                Obj::Map(m) => {
                    if let Some(pair) = m
                        .iter_mut()
                        .find(|(k, _)| matches!(k, Value::Str(s) if s.as_ref() == key.as_ref()))
                    {
                        pair.1 = v;
                    } else {
                        m.push((Value::Str(Rc::from(key.as_ref())), v));
                    }
                }
                Obj::List(_) => return Err("expected a map".into()),
            }
            heap.write_barrier(r);
            Value::Unit
        }
        ListLen => {
            let r = arg(0).gc_ref().ok_or("expected a list")?;
            Value::Int(match heap.get(r) {
                Obj::List(v) => v.len() as i64,
                Obj::Map(m) => m.len() as i64,
            })
        }
        ListPush => {
            let r = arg(0).gc_ref().ok_or("expected a list")?;
            let v = arg(1);
            match heap.get_mut(r) {
                Obj::List(items) => items.push(v),
                Obj::Map(_) => return Err("expected a list".into()),
            }
            heap.write_barrier(r);
            Value::Unit
        }
        ListPop => {
            let r = arg(0).gc_ref().ok_or("expected a list")?;
            let v = match heap.get_mut(r) {
                Obj::List(items) => items.pop().ok_or("pop from an empty list")?,
                Obj::Map(_) => return Err("expected a list".into()),
            };
            v
        }
        ListGet => {
            let r = arg(0).gc_ref().ok_or("expected a list")?;
            let idx = match arg(1) {
                Value::Int(i) => i,
                other => {
                    return Err(format!(
                        "list index must be int, found {}",
                        other.type_name()
                    ))
                }
            };
            match heap.get(r) {
                Obj::List(items) => {
                    let len = items.len() as i64;
                    let i = if idx < 0 { len + idx } else { idx };
                    if i < 0 || i >= len {
                        return Err(format!("list index {} out of bounds (length {})", idx, len));
                    }
                    items[i as usize].clone()
                }
                Obj::Map(_) => return Err("expected a list".into()),
            }
        }
        ListSet => {
            let r = arg(0).gc_ref().ok_or("expected a list")?;
            let idx = match arg(1) {
                Value::Int(i) => i,
                other => {
                    return Err(format!(
                        "list index must be int, found {}",
                        other.type_name()
                    ))
                }
            };
            let v = arg(2);
            match heap.get_mut(r) {
                Obj::List(items) => {
                    let len = items.len() as i64;
                    let i = if idx < 0 { len + idx } else { idx };
                    if i < 0 || i >= len {
                        return Err(format!("list index {} out of bounds (length {})", idx, len));
                    }
                    items[i as usize] = v;
                }
                Obj::Map(_) => return Err("expected a list".into()),
            }
            heap.write_barrier(r);
            Value::Unit
        }
        StrLen => {
            let s = str_of(arg(0))?;
            Value::Int(s.as_ref().chars().count() as i64)
        }
        StrUpper => Value::Str(Rc::from(str_of(arg(0))?.to_uppercase().as_str())),
        StrLower => Value::Str(Rc::from(str_of(arg(0))?.to_lowercase().as_str())),
        StrTrim => Value::Str(Rc::from(str_of(arg(0))?.trim().to_string().as_str())),
        StrContains => {
            let s = str_of(arg(0))?;
            let needle = str_of(arg(1))?;
            Value::Bool(s.contains(needle.as_ref()))
        }
        StrReplace => {
            let s = str_of(arg(0))?;
            let from = str_of(arg(1))?;
            let to = str_of(arg(2))?;
            Value::Str(Rc::from(s.replace(from.as_ref(), to.as_ref()).as_str()))
        }
        StrSplit => {
            let s = str_of(arg(0))?;
            let sep = str_of(arg(1))?;
            let parts: Vec<Value> = s
                .split(sep.as_ref())
                .map(|p| Value::Str(Rc::from(p)))
                .collect();
            Value::List(heap.alloc(Obj::List(parts)))
        }
        AbsInt => {
            let Value::Int(i) = arg(0) else {
                return Err("expected an int".into());
            };
            Value::Int(i.checked_abs().ok_or("integer overflow in abs")?)
        }
        AbsFloat => {
            let Value::Float(f) = arg(0) else {
                return Err("expected a float".into());
            };
            Value::Float(f.abs())
        }
        MinInt | MaxInt => {
            let (Value::Int(a), Value::Int(b)) = (arg(0), arg(1)) else {
                return Err("expected two ints".into());
            };
            Value::Int(if *kind == MinInt { a.min(b) } else { a.max(b) })
        }
        MinFloat | MaxFloat => {
            let (Value::Float(a), Value::Float(b)) = (arg(0), arg(1)) else {
                return Err("expected two floats".into());
            };
            Value::Float(if *kind == MinFloat {
                a.min(b)
            } else {
                a.max(b)
            })
        }
        Sqrt => Value::Float(float1(&argv)?.sqrt()),
        Pow => {
            let (a, b) = float2(&argv)?;
            Value::Float(a.powf(b))
        }
        Floor => Value::Float(float1(&argv)?.floor()),
        Ceil => Value::Float(float1(&argv)?.ceil()),
        Round => Value::Float(float1(&argv)?.round()),
        Sin => Value::Float(float1(&argv)?.sin()),
        Cos => Value::Float(float1(&argv)?.cos()),
        Tan => Value::Float(float1(&argv)?.tan()),
        Log => Value::Float(float1(&argv)?.ln()),
        ReadText => {
            let path = str_of(arg(0))?;
            match std::fs::read_to_string(path.as_ref()) {
                Ok(s) => Value::Str(Rc::from(s.as_str())),
                Err(e) => return Err(format!("cannot read `{}`: {}", path, e)),
            }
        }
        WriteText => {
            let path = str_of(arg(0))?;
            let content = str_of(arg(1))?;
            match std::fs::write(path.as_ref(), content.as_ref()) {
                Ok(_) => Value::Unit,
                Err(e) => return Err(format!("cannot write `{}`: {}", path, e)),
            }
        }
        FileExists => {
            let path = str_of(arg(0))?;
            Value::Bool(std::path::Path::new(path.as_ref()).exists())
        }
        GuiWindow | GuiLabel | GuiInput | GuiButton | GuiSetText | GuiControl | GuiGetText
        | GuiGetId | GuiOnClick | GuiOnKey | GuiText | GuiRoundRect | GuiKeyHandler | GuiRun
        | GuiClose => {
            return Err("GUI functions are available only in native Windows GUI executables".into())
        }
    })
}

fn str_of(v: Value) -> Result<Rc<str>, String> {
    match v {
        Value::Str(s) => Ok(s),
        other => Err(format!("expected a string, found {}", other.type_name())),
    }
}

fn float1(argv: &[Value]) -> Result<f64, String> {
    match argv.first() {
        Some(Value::Float(f)) => Ok(*f),
        Some(other) => Err(format!("expected a float, found {}", other.type_name())),
        None => Err("missing argument".into()),
    }
}

fn float2(argv: &[Value]) -> Result<(f64, f64), String> {
    match (argv.first(), argv.get(1)) {
        (Some(Value::Float(a)), Some(Value::Float(b))) => Ok((*a, *b)),
        _ => Err("expected two floats".into()),
    }
}

/// Execute with default options (compatibility wrapper).
pub fn run(m: &BytecodeModule, out: impl FnMut(&str)) -> Result<Value, VmError> {
    run_with_options(m, out, VmOptions::default()).map(|(v, _)| v)
}

fn check_init(v: &Value) -> Result<(), String> {
    match v {
        Value::Uninit => Err("read of an uninitialized register (this indicates a verifier gap or corrupted bytecode)".to_string()),
        _ => Ok(()),
    }
}
