//! Bytecode verifier.
//!
//! Nothing is executed before it is verified. The verifier proves, per
//! function, that:
//!
//! * every opcode is known and correctly shaped,
//! * constant indices, function indices, arities and jump targets are in range,
//! * every register is written before it is read on the paths the verifier
//!   explores (dataflow fixpoint over reachable states),
//! * control flow cannot fall off the end of the code.
//!
//! Guarantee chain: the verifier rejects reads of registers that are never
//! written on any explored path. Path-dependent cases (written on some paths
//! only) cannot be rejected statically without a must-analysis, so the VM
//! additionally initializes all registers to an "uninitialized" value and
//! raises a clean runtime error if one is ever read. The compiler front end
//! never emits such code.
//!
//! Parameters count as pre-defined registers (`r0..r{arity-1}`).

use crate::bytecode::{op_shape, BytecodeModule, OpShape};
use std::collections::{BTreeSet, HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct VerifyError {
    pub func: String,
    pub offset: usize,
    pub message: String,
}

impl std::fmt::Display for VerifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[E4002] verification failed in `{}` at offset {}: {}",
            self.func, self.offset, self.message
        )
    }
}

/// Control-flow continuation of a decoded instruction.
enum Next {
    /// fall through to `offset`
    Linear(usize),
    /// unconditional jump to `offset`
    Jump(usize),
    /// conditional branch: `target` when false, `linear` when true
    Branch { target: usize, linear: usize },
    /// RET / RET_UNIT
    End,
}

struct R<'a> {
    code: &'a [u8],
    p: usize,
}

impl<'a> R<'a> {
    fn u8(&mut self) -> Result<u8, String> {
        if self.p >= self.code.len() {
            return Err("truncated instruction".into());
        }
        let v = self.code[self.p];
        self.p += 1;
        Ok(v)
    }

    fn u16(&mut self) -> Result<u16, String> {
        if self.p + 2 > self.code.len() {
            return Err("truncated instruction".into());
        }
        let v = u16::from_le_bytes([self.code[self.p], self.code[self.p + 1]]);
        self.p += 2;
        Ok(v)
    }

    fn u32(&mut self) -> Result<u32, String> {
        if self.p + 4 > self.code.len() {
            return Err("truncated instruction".into());
        }
        let b: [u8; 4] = self.code[self.p..self.p + 4].try_into().unwrap();
        self.p += 4;
        Ok(u32::from_le_bytes(b))
    }

    fn i32(&mut self) -> Result<i32, String> {
        Ok(self.u32()? as i32)
    }
}

type Decoded = (Vec<u16>, Option<u16>, Next);

fn decode(m: &BytecodeModule, fi: usize, ip: usize) -> Result<Decoded, (usize, String)> {
    let f = &m.funcs[fi];
    let code = &f.code;
    if ip >= code.len() {
        return Err((ip, "instruction pointer out of range".into()));
    }
    let op = code[ip];
    let shape = op_shape(op).ok_or((ip, format!("unknown opcode 0x{:02x}", op)))?;
    let mut r = R { code, p: ip + 1 };
    let reg = |v: u16| -> Result<(), String> {
        if v >= f.nregs {
            Err(format!(
                "register r{} out of range (function declares {} register(s))",
                v, f.nregs
            ))
        } else {
            Ok(())
        }
    };

    Ok(match shape {
        OpShape::Const => {
            let ci = r.u32().map_err(|e| (ip, e))? as usize;
            let d = r.u16().map_err(|e| (ip, e))?;
            if ci >= m.consts.len() {
                return Err((ip, format!("constant index {} out of range", ci)));
            }
            reg(d).map_err(|e| (ip, e))?;
            (vec![], Some(d), Next::Linear(r.p))
        }
        OpShape::Unit => {
            let d = r.u16().map_err(|e| (ip, e))?;
            reg(d).map_err(|e| (ip, e))?;
            (vec![], Some(d), Next::Linear(r.p))
        }
        OpShape::Move => {
            let a = r.u16().map_err(|e| (ip, e))?;
            let d = r.u16().map_err(|e| (ip, e))?;
            reg(a).map_err(|e| (ip, e))?;
            reg(d).map_err(|e| (ip, e))?;
            (vec![a], Some(d), Next::Linear(r.p))
        }
        OpShape::Ternary => {
            let a = r.u16().map_err(|e| (ip, e))?;
            let b = r.u16().map_err(|e| (ip, e))?;
            let d = r.u16().map_err(|e| (ip, e))?;
            reg(a).map_err(|e| (ip, e))?;
            reg(b).map_err(|e| (ip, e))?;
            reg(d).map_err(|e| (ip, e))?;
            (vec![a, b], Some(d), Next::Linear(r.p))
        }
        OpShape::Sink => {
            let a = r.u16().map_err(|e| (ip, e))?;
            reg(a).map_err(|e| (ip, e))?;
            (vec![a], None, Next::Linear(r.p))
        }
        OpShape::Binary => {
            let a = r.u16().map_err(|e| (ip, e))?;
            let d = r.u16().map_err(|e| (ip, e))?;
            reg(a).map_err(|e| (ip, e))?;
            reg(d).map_err(|e| (ip, e))?;
            (vec![a], Some(d), Next::Linear(r.p))
        }
        OpShape::Branch => {
            let c = r.u16().map_err(|e| (ip, e))?;
            let t = r.i32().map_err(|e| (ip, e))?;
            reg(c).map_err(|e| (ip, e))?;
            if t < 0 || t as usize >= code.len() {
                return Err((ip, format!("branch target {} out of range", t)));
            }
            (
                vec![c],
                None,
                Next::Branch {
                    target: t as usize,
                    linear: r.p,
                },
            )
        }
        OpShape::Jump => {
            let t = r.i32().map_err(|e| (ip, e))?;
            if t < 0 || t as usize >= code.len() {
                return Err((ip, format!("jump target {} out of range", t)));
            }
            (vec![], None, Next::Jump(t as usize))
        }
        OpShape::Call => {
            let callee = r.u16().map_err(|e| (ip, e))? as usize;
            let argc = r.u8().map_err(|e| (ip, e))? as usize;
            if callee >= m.funcs.len() {
                return Err((ip, format!("function index {} out of range", callee)));
            }
            if argc as u8 != m.funcs[callee].arity {
                return Err((
                    ip,
                    format!(
                        "call passes {} argument(s) but `{}` takes {}",
                        argc, m.funcs[callee].name, m.funcs[callee].arity
                    ),
                ));
            }
            let mut reads = Vec::with_capacity(argc);
            for _ in 0..argc {
                let a = r.u16().map_err(|e| (ip, e))?;
                reg(a).map_err(|e| (ip, e))?;
                reads.push(a);
            }
            let d = r.u16().map_err(|e| (ip, e))?;
            reg(d).map_err(|e| (ip, e))?;
            (reads, Some(d), Next::Linear(r.p))
        }
        OpShape::Ret => {
            let a = r.u16().map_err(|e| (ip, e))?;
            reg(a).map_err(|e| (ip, e))?;
            (vec![a], None, Next::End)
        }
        OpShape::Builtin => {
            let kind = r.u8().map_err(|e| (ip, e))?;
            let d = r.u16().map_err(|e| (ip, e))?;
            let argc = r.u8().map_err(|e| (ip, e))? as usize;
            let kind = crate::bytecode::BuiltinKind::from_u8(kind)
                .ok_or((ip, format!("unknown builtin kind")))?;
            let (want_argc, _) = kind.sig();
            // ListNew takes a variable number of elements; MapNew takes
            // interleaved (key, value) pairs, so its argc must be even.
            let argc_ok = match kind {
                crate::bytecode::BuiltinKind::ListNew => true,
                crate::bytecode::BuiltinKind::MapNew => argc % 2 == 0,
                _ => argc == want_argc,
            };
            if !argc_ok {
                return Err((
                    ip,
                    format!(
                        "builtin `{}` expects {} argument(s), found {}",
                        kind.name(),
                        want_argc,
                        argc
                    ),
                ));
            }
            let mut reads = Vec::with_capacity(argc);
            for _ in 0..argc {
                let a = r.u16().map_err(|e| (ip, e))?;
                reg(a).map_err(|e| (ip, e))?;
                reads.push(a);
            }
            reg(d).map_err(|e| (ip, e))?;
            (reads, Some(d), Next::Linear(r.p))
        }
        OpShape::None => (vec![], None, Next::End),
    })
}

/// Verify every function in the module.
pub fn verify(m: &BytecodeModule) -> Result<(), VerifyError> {
    for fi in 0..m.funcs.len() {
        verify_func(m, fi)?;
    }
    Ok(())
}

fn verify_func(m: &BytecodeModule, fi: usize) -> Result<(), VerifyError> {
    let name = m.funcs[fi].name.clone();
    let err = |offset: usize, message: String| VerifyError {
        func: name.clone(),
        offset,
        message,
    };

    if fi == m.entry && m.funcs[fi].arity != 0 {
        return Err(err(0, "entry function must take no parameters".into()));
    }
    let code_len = m.funcs[fi].code.len();
    if code_len == 0 {
        return Err(err(0, "function has an empty body".into()));
    }

    // Union-merge dataflow fixpoint: inputs[offset] = registers defined on
    // some explored path entering that offset.
    let mut inputs: HashMap<usize, BTreeSet<u16>> = HashMap::new();
    let mut work: Vec<usize> = Vec::new();
    let seed: BTreeSet<u16> = (0..m.funcs[fi].arity as u16).collect();
    inputs.insert(0, seed);
    work.push(0);

    while let Some(start) = work.pop() {
        let mut ip = start;
        let mut state = inputs[&start].clone();
        loop {
            let (reads, write, next) = match decode(m, fi, ip) {
                Ok(v) => v,
                Err((off, msg)) => return Err(err(off, msg)),
            };
            for r in &reads {
                if !state.contains(r) {
                    return Err(err(
                        ip,
                        format!("register r{} is read before it is written", r),
                    ));
                }
            }
            if let Some(d) = write {
                state.insert(d);
            }
            match next {
                Next::End => break,
                Next::Linear(t) => {
                    if t >= code_len {
                        return Err(err(
                            ip,
                            "control flow falls off the end of the function".into(),
                        ));
                    }
                    merge(&mut inputs, &mut work, t, &state);
                    ip = t;
                    state = inputs[&t].clone();
                }
                Next::Jump(t) => {
                    merge(&mut inputs, &mut work, t, &state);
                    break;
                }
                Next::Branch { target, linear } => {
                    merge(&mut inputs, &mut work, target, &state);
                    if linear >= code_len {
                        return Err(err(
                            ip,
                            "control flow falls off the end of the function".into(),
                        ));
                    }
                    merge(&mut inputs, &mut work, linear, &state);
                    ip = linear;
                    state = inputs[&linear].clone();
                }
            }
        }
    }

    // Every jump target must have been visited (i.e. be reachable); report
    // unreachable-but-targeted code as an error rather than skipping it.
    let mut targets: HashSet<usize> = HashSet::new();
    let mut p = 0usize;
    while p < code_len {
        match decode(m, fi, p) {
            Ok((_, _, next)) => {
                let t = match next {
                    Next::Jump(t) => Some(t),
                    Next::Branch { target, .. } => Some(target),
                    _ => None,
                };
                if let Some(t) = t {
                    targets.insert(t);
                }
                p += instr_len(m, fi, p);
            }
            Err((off, msg)) => return Err(err(off, msg)),
        }
    }
    for t in targets {
        if !inputs.contains_key(&t) {
            return Err(err(t, "jump target is unreachable by linear flow".into()));
        }
    }
    Ok(())
}

/// Length of the instruction at `ip` (assumes it decodes cleanly).
fn instr_len(m: &BytecodeModule, fi: usize, ip: usize) -> usize {
    let code = &m.funcs[fi].code;
    let op = code[ip];
    match op_shape(op).unwrap() {
        OpShape::Const => 7,   // op + u32 + u16
        OpShape::Unit => 3,    // op + u16
        OpShape::Move => 5,    // op + u16 + u16
        OpShape::Ternary => 7, // op + 3x u16
        OpShape::Binary => 5,  // op + u16 + u16
        OpShape::Branch => 7,  // op + u16 + i32
        OpShape::Sink => 3,    // op + u16
        OpShape::Jump => 5,    // op + i32
        OpShape::Call => {
            let argc = code[ip + 3] as usize;
            4 + argc * 2 + 2 // op + u16 + u8 + args + dst
        }
        OpShape::Ret => 3, // op + u16
        OpShape::None => 1,
        OpShape::Builtin => {
            let argc = code[ip + 4] as usize;
            5 + argc * 2 // op + kind + dst + argc + args
        }
    }
}

/// Merge `state` into the input set at `offset`; returns true if this is the
/// first visit or the set grew (caller re-enqueues in both cases).
fn merge(
    inputs: &mut HashMap<usize, BTreeSet<u16>>,
    work: &mut Vec<usize>,
    offset: usize,
    state: &BTreeSet<u16>,
) -> bool {
    match inputs.get_mut(&offset) {
        None => {
            inputs.insert(offset, state.clone());
            work.push(offset);
            true
        }
        Some(prev) => {
            let before = prev.len();
            prev.extend(state.iter().copied());
            let grew = prev.len() != before;
            if grew {
                work.push(offset);
            }
            grew
        }
    }
}
