//! Lutetium bytecode: instruction set, binary container and (de)serialization.
//!
//! The binary layout is documented in `docs/BYTECODE_FORMAT.md`.

pub const MAGIC: &[u8; 4] = b"LTBC";
/// Version 2: adds the `List` return-type tag and the BUILTIN opcode.
/// Version 3 adds maps; version 4 adds deep list equality; version 5 adds
/// opaque GUI handle tags and GUI builtin kinds (native target only).
pub const FORMAT_VERSION: u16 = 5;

/// Return-type tags stored per function.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BcTy {
    Int,
    Float,
    Bool,
    Str,
    Unit,
    /// A list value (element types are tracked by the checker, not here).
    List,
    /// A map value (string keys; the value type is tracked by the checker).
    Map,
    /// Opaque native GUI window handle.
    Window,
    /// Opaque native GUI control handle.
    Widget,
}

impl BcTy {
    pub fn tag(self) -> u8 {
        match self {
            BcTy::Int => 1,
            BcTy::Float => 2,
            BcTy::Bool => 3,
            BcTy::Str => 4,
            BcTy::Unit => 5,
            BcTy::List => 6,
            BcTy::Map => 7,
            BcTy::Window => 8,
            BcTy::Widget => 9,
        }
    }

    pub fn from_tag(t: u8) -> Option<BcTy> {
        match t {
            1 => Some(BcTy::Int),
            2 => Some(BcTy::Float),
            3 => Some(BcTy::Bool),
            4 => Some(BcTy::Str),
            5 => Some(BcTy::Unit),
            6 => Some(BcTy::List),
            7 => Some(BcTy::Map),
            8 => Some(BcTy::Window),
            9 => Some(BcTy::Widget),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            BcTy::Int => "int",
            BcTy::Float => "float",
            BcTy::Bool => "bool",
            BcTy::Str => "str",
            BcTy::Unit => "unit",
            BcTy::List => "list",
            BcTy::Map => "map",
            BcTy::Window => "window",
            BcTy::Widget => "widget",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum BcConst {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(String),
    Unit,
}

#[derive(Debug, Clone)]
pub struct BcFunction {
    pub name: String,
    pub arity: u8,
    pub nregs: u16,
    pub ret: BcTy,
    pub code: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct BytecodeModule {
    pub consts: Vec<BcConst>,
    pub funcs: Vec<BcFunction>,
    /// Index into `funcs` of the entry point.
    pub entry: usize,
}

// ---------------------------------------------------------------------------
// Instruction set (register machine: every instruction names its operands)
// ---------------------------------------------------------------------------

pub mod op {
    pub const CONST_INT: u8 = 0x01;
    pub const CONST_FLOAT: u8 = 0x02;
    pub const CONST_BOOL: u8 = 0x03;
    pub const CONST_STR: u8 = 0x04;
    pub const CONST_UNIT: u8 = 0x05;
    pub const MOVE: u8 = 0x06;

    pub const ADD_INT: u8 = 0x08;
    pub const SUB_INT: u8 = 0x09;
    pub const MUL_INT: u8 = 0x0A;
    pub const DIV_INT: u8 = 0x0B;
    pub const MOD_INT: u8 = 0x0C;
    pub const ADD_FLOAT: u8 = 0x0D;
    pub const SUB_FLOAT: u8 = 0x0E;
    pub const MUL_FLOAT: u8 = 0x0F;
    pub const DIV_FLOAT: u8 = 0x10;
    pub const INT_TO_FLOAT: u8 = 0x11;
    pub const CONCAT_STR: u8 = 0x12;

    pub const EQ_INT: u8 = 0x14;
    pub const NE_INT: u8 = 0x15;
    pub const LT_INT: u8 = 0x16;
    pub const LE_INT: u8 = 0x17;
    pub const GT_INT: u8 = 0x18;
    pub const GE_INT: u8 = 0x19;
    pub const EQ_FLOAT: u8 = 0x1A;
    pub const NE_FLOAT: u8 = 0x1B;
    pub const LT_FLOAT: u8 = 0x1C;
    pub const LE_FLOAT: u8 = 0x1D;
    pub const GT_FLOAT: u8 = 0x1E;
    pub const GE_FLOAT: u8 = 0x1F;
    pub const EQ_STR: u8 = 0x20;
    pub const NE_STR: u8 = 0x21;
    pub const EQ_BOOL: u8 = 0x22;
    pub const NE_BOOL: u8 = 0x23;
    pub const NOT: u8 = 0x24;
    pub const NEG_INT: u8 = 0x25;
    pub const NEG_FLOAT: u8 = 0x26;

    pub const JUMP: u8 = 0x27;
    pub const JUMP_IF_FALSE: u8 = 0x28;
    pub const CALL: u8 = 0x29;
    pub const RET: u8 = 0x2A;
    pub const RET_UNIT: u8 = 0x2B;
    pub const PRINT: u8 = 0x2C;
    /// BUILTIN: u8 kind, u16 dst, u8 argc, argc x u16 args.
    /// Kinds and their signatures live in `BuiltinKind`.
    pub const BUILTIN: u8 = 0x2D;
    /// Deep list equality: element-wise comparison with int/float numeric
    /// widening; different lengths are unequal. Version 4 additions,
    /// appended after all earlier opcodes so existing encodings are stable.
    pub const EQ_LIST: u8 = 0x2E;
    pub const NE_LIST: u8 = 0x2F;
}

/// Built-in operations shared by the checker, verifier, VM and native
/// backend. `sig()` documents the argument count and whether the kind writes
/// its destination register (all kinds write a value; unit results write a
/// dummy so the verifier stays uniform).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinKind {
    ListNew,
    ListLen,
    ListPush,
    ListPop,
    ListGet,
    ListSet,
    StrLen,
    StrUpper,
    StrLower,
    StrTrim,
    StrContains,
    StrReplace,
    StrSplit,
    AbsInt,
    AbsFloat,
    MinInt,
    MinFloat,
    MaxInt,
    MaxFloat,
    Sqrt,
    Pow,
    Floor,
    Ceil,
    Round,
    Sin,
    Cos,
    Tan,
    Log,
    ReadText,
    WriteText,
    FileExists,
    /// MapNew: even argc; interleaved (key, value) register pairs.
    MapNew,
    MapGet,
    MapSet,
    GuiWindow,
    GuiLabel,
    GuiInput,
    GuiButton,
    GuiSetText,
    GuiControl,
    GuiGetText,
    GuiGetId,
    GuiOnClick,
    GuiOnKey,
    GuiText,
    GuiRoundRect,
    GuiKeyHandler,
    GuiRun,
    GuiClose,
}

impl BuiltinKind {
    pub fn from_u8(k: u8) -> Option<BuiltinKind> {
        use BuiltinKind::*;
        Some(match k {
            0x01 => ListNew,
            0x02 => ListLen,
            0x03 => ListPush,
            0x04 => ListPop,
            0x05 => ListGet,
            0x06 => ListSet,
            0x10 => StrLen,
            0x11 => StrUpper,
            0x12 => StrLower,
            0x13 => StrTrim,
            0x14 => StrContains,
            0x15 => StrReplace,
            0x16 => StrSplit,
            0x20 => AbsInt,
            0x21 => AbsFloat,
            0x22 => MinInt,
            0x23 => MinFloat,
            0x24 => MaxInt,
            0x25 => MaxFloat,
            0x26 => Sqrt,
            0x27 => Pow,
            0x28 => Floor,
            0x29 => Ceil,
            0x2A => Round,
            0x2B => Sin,
            0x2C => Cos,
            0x2D => Tan,
            0x2E => Log,
            0x30 => ReadText,
            0x31 => WriteText,
            0x32 => FileExists,
            0x40 => MapNew,
            0x41 => MapGet,
            0x42 => MapSet,
            0x50 => GuiWindow,
            0x51 => GuiLabel,
            0x52 => GuiInput,
            0x53 => GuiButton,
            0x54 => GuiSetText,
            0x55 => GuiRun,
            0x56 => GuiClose,
            0x57 => GuiControl,
            0x58 => GuiGetText,
            0x59 => GuiGetId,
            0x5A => GuiOnClick,
            0x5B => GuiOnKey,
            0x5C => GuiText,
            0x5D => GuiRoundRect,
            0x5E => GuiKeyHandler,
            _ => return None,
        })
    }

    pub fn kind_byte(self) -> u8 {
        use BuiltinKind::*;
        match self {
            ListNew => 0x01,
            ListLen => 0x02,
            ListPush => 0x03,
            ListPop => 0x04,
            ListGet => 0x05,
            ListSet => 0x06,
            StrLen => 0x10,
            StrUpper => 0x11,
            StrLower => 0x12,
            StrTrim => 0x13,
            StrContains => 0x14,
            StrReplace => 0x15,
            StrSplit => 0x16,
            AbsInt => 0x20,
            AbsFloat => 0x21,
            MinInt => 0x22,
            MinFloat => 0x23,
            MaxInt => 0x24,
            MaxFloat => 0x25,
            Sqrt => 0x26,
            Pow => 0x27,
            Floor => 0x28,
            Ceil => 0x29,
            Round => 0x2A,
            Sin => 0x2B,
            Cos => 0x2C,
            Tan => 0x2D,
            Log => 0x2E,
            ReadText => 0x30,
            WriteText => 0x31,
            FileExists => 0x32,
            MapNew => 0x40,
            MapGet => 0x41,
            MapSet => 0x42,
            GuiWindow => 0x50,
            GuiLabel => 0x51,
            GuiInput => 0x52,
            GuiButton => 0x53,
            GuiSetText => 0x54,
            GuiRun => 0x55,
            GuiClose => 0x56,
            GuiControl => 0x57,
            GuiGetText => 0x58,
            GuiGetId => 0x59,
            GuiOnClick => 0x5A,
            GuiOnKey => 0x5B,
            GuiText => 0x5C,
            GuiRoundRect => 0x5D,
            GuiKeyHandler => 0x5E,
        }
    }

    /// Human-readable name (errors, disassembly).
    pub fn name(self) -> &'static str {
        use BuiltinKind::*;
        match self {
            ListNew => "list_new",
            ListLen => "list_len",
            ListPush => "list_push",
            ListPop => "list_pop",
            ListGet => "list_get",
            ListSet => "list_set",
            StrLen => "str_len",
            StrUpper => "str_upper",
            StrLower => "str_lower",
            StrTrim => "str_trim",
            StrContains => "str_contains",
            StrReplace => "str_replace",
            StrSplit => "str_split",
            AbsInt => "abs_int",
            AbsFloat => "abs_float",
            MinInt => "min_int",
            MinFloat => "min_float",
            MaxInt => "max_int",
            MaxFloat => "max_float",
            Sqrt => "sqrt",
            Pow => "pow",
            Floor => "floor",
            Ceil => "ceil",
            Round => "round",
            Sin => "sin",
            Cos => "cos",
            Tan => "tan",
            Log => "log",
            ReadText => "read_text",
            WriteText => "write_text",
            FileExists => "file_exists",
            MapNew => "map_new",
            MapGet => "map_get",
            MapSet => "map_set",
            GuiWindow => "gui_window",
            GuiLabel => "gui_label",
            GuiInput => "gui_input",
            GuiButton => "gui_button",
            GuiSetText => "gui_set_text",
            GuiRun => "gui_run",
            GuiClose => "gui_close",
            GuiControl => "gui_control",
            GuiGetText => "gui_get_text",
            GuiGetId => "gui_id",
            GuiOnClick => "gui_on_click",
            GuiOnKey => "gui_on_key",
            GuiText => "gui_text",
            GuiRoundRect => "gui_round_rect",
            GuiKeyHandler => "gui_key_handler",
        }
    }

    /// (argument count, human-readable arg description)
    pub fn sig(self) -> (usize, &'static str) {
        use BuiltinKind::*;
        match self {
            ListNew => (0, "(elements...)"),
            ListLen => (1, "(list)"),
            ListPush => (2, "(list, value)"),
            ListPop => (1, "(list)"),
            ListGet => (2, "(list, index)"),
            ListSet => (3, "(list, index, value)"),
            MapNew => (0, "(key, value, ...)"),
            MapGet => (2, "(map, key)"),
            MapSet => (3, "(map, key, value)"),
            StrLen => (1, "(string)"),
            StrUpper | StrLower | StrTrim => (1, "(string)"),
            StrContains => (2, "(string, needle)"),
            StrReplace => (3, "(string, from, to)"),
            StrSplit => (2, "(string, separator)"),
            AbsInt | AbsFloat | Sqrt | Floor | Ceil | Round | Sin | Cos | Tan | Log => {
                (1, "(number)")
            }
            MinInt | MinFloat | MaxInt | MaxFloat | Pow => (2, "(a, b)"),
            ReadText => (1, "(path)"),
            WriteText => (2, "(path, content)"),
            FileExists => (1, "(path)"),
            GuiWindow => (3, "(title, width, height)"),
            GuiLabel | GuiInput | GuiButton => (7, "(window, id, text, x, y, width, height)"),
            GuiControl => (2, "(window, id)"),
            GuiGetText | GuiGetId => (1, "(widget)"),
            GuiSetText => (2, "(widget, text)"),
            GuiOnClick | GuiOnKey => (2, "(window, callback)"),
            GuiText => (7, "(window, id, text, x, y, width, height)"),
            GuiKeyHandler => (2, "(window, callback)"),
            GuiRoundRect => (8, "(window, id, x, y, width, height, radius, color)"),
            GuiRun | GuiClose => (1, "(window)"),
        }
    }

    /// All builtin function names usable as call syntax (`sqrt(x)`, ...).
    /// Method-style names (`l.push`) are resolved by the checker, not here.
    pub fn function_name(self) -> Option<&'static str> {
        use BuiltinKind::*;
        Some(match self {
            Sqrt => "sqrt",
            Pow => "pow",
            Floor => "floor",
            Ceil => "ceil",
            Round => "round",
            AbsInt => "abs_int",
            AbsFloat => "abs",
            MinInt => "min_int",
            MinFloat => "min",
            MaxInt => "max_int",
            MaxFloat => "max",
            Sin => "sin",
            Cos => "cos",
            Tan => "tan",
            Log => "log",
            ReadText => "read_text",
            WriteText => "write_text",
            FileExists => "file_exists",
            _ => return None,
        })
    }

    /// Reverse of `function_name`: resolve a source-level builtin call name
    /// (`sqrt(x)`, `read_text(p)`, ...) to its kind.
    pub fn function_name_of(name: &str) -> Option<BuiltinKind> {
        use BuiltinKind::*;
        Some(match name {
            "sqrt" => Sqrt,
            "pow" => Pow,
            "floor" => Floor,
            "ceil" => Ceil,
            "round" => Round,
            "abs" => AbsFloat,
            "abs_int" => AbsInt,
            "min" => MinFloat,
            "min_int" => MinInt,
            "max" => MaxFloat,
            "max_int" => MaxInt,
            "sin" => Sin,
            "cos" => Cos,
            "tan" => Tan,
            "log" => Log,
            "read_text" => ReadText,
            "write_text" => WriteText,
            "file_exists" => FileExists,
            "gui_window" => GuiWindow,
            "gui_label" => GuiLabel,
            "gui_input" => GuiInput,
            "gui_button" => GuiButton,
            "gui_set_text" => GuiSetText,
            "gui_run" => GuiRun,
            "gui_close" => GuiClose,
            "gui_control" => GuiControl,
            "gui_get_text" => GuiGetText,
            "gui_id" => GuiGetId,
            "gui_on_click" => GuiOnClick,
            "gui_on_key" => GuiOnKey,
            "gui_text" => GuiText,
            "gui_round_rect" => GuiRoundRect,
            "gui_key_handler" => GuiKeyHandler,
            _ => return None,
        })
    }
}

/// Static operand layout, shared by the verifier, disassembler and CLI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpShape {
    /// const index (u32) + dst register (u16)
    Const,
    /// dst register (u16)
    Unit,
    /// dst register (u16), no write
    Sink,
    /// src register, dst register
    Move,
    /// a, b, dst registers
    Ternary,
    /// a, dst registers
    Binary,
    /// conditional register (u16), target offset (i32)
    Branch,
    /// unconditional target offset (i32)
    Jump,
    /// func index (u16), argc (u8), argc x u16 registers, dst register (u16)
    Call,
    /// builtin kind (u8), dst (u16), argc (u8), argc x u16 registers
    Builtin,
    /// src register
    Ret,
    /// no operands
    None,
}

pub fn op_shape(op: u8) -> Option<OpShape> {
    use op::*;
    Some(match op {
        CONST_INT | CONST_FLOAT | CONST_BOOL | CONST_STR => OpShape::Const,
        CONST_UNIT => OpShape::Unit,
        MOVE => OpShape::Move,
        ADD_INT | SUB_INT | MUL_INT | DIV_INT | MOD_INT | ADD_FLOAT | SUB_FLOAT | MUL_FLOAT
        | DIV_FLOAT | CONCAT_STR | EQ_INT | NE_INT | LT_INT | LE_INT | GT_INT | GE_INT
        | EQ_FLOAT | NE_FLOAT | LT_FLOAT | LE_FLOAT | GT_FLOAT | GE_FLOAT | EQ_STR | NE_STR
        | EQ_BOOL | NE_BOOL | EQ_LIST | NE_LIST => OpShape::Ternary,
        INT_TO_FLOAT | NOT | NEG_INT | NEG_FLOAT => OpShape::Binary,
        PRINT => OpShape::Sink,
        JUMP_IF_FALSE => OpShape::Branch,
        JUMP => OpShape::Jump,
        CALL => OpShape::Call,
        BUILTIN => OpShape::Builtin,
        RET => OpShape::Ret,
        RET_UNIT => OpShape::None,
        _ => return None,
    })
}

pub fn opcode_name(op: u8) -> Option<&'static str> {
    use op::*;
    Some(match op {
        CONST_INT => "CONST_INT",
        CONST_FLOAT => "CONST_FLOAT",
        CONST_BOOL => "CONST_BOOL",
        CONST_STR => "CONST_STR",
        CONST_UNIT => "CONST_UNIT",
        MOVE => "MOVE",
        ADD_INT => "ADD_INT",
        SUB_INT => "SUB_INT",
        MUL_INT => "MUL_INT",
        DIV_INT => "DIV_INT",
        MOD_INT => "MOD_INT",
        ADD_FLOAT => "ADD_FLOAT",
        SUB_FLOAT => "SUB_FLOAT",
        MUL_FLOAT => "MUL_FLOAT",
        DIV_FLOAT => "DIV_FLOAT",
        INT_TO_FLOAT => "INT_TO_FLOAT",
        CONCAT_STR => "CONCAT_STR",
        EQ_INT => "EQ_INT",
        NE_INT => "NE_INT",
        LT_INT => "LT_INT",
        LE_INT => "LE_INT",
        GT_INT => "GT_INT",
        GE_INT => "GE_INT",
        EQ_FLOAT => "EQ_FLOAT",
        NE_FLOAT => "NE_FLOAT",
        LT_FLOAT => "LT_FLOAT",
        LE_FLOAT => "LE_FLOAT",
        GT_FLOAT => "GT_FLOAT",
        GE_FLOAT => "GE_FLOAT",
        EQ_STR => "EQ_STR",
        NE_STR => "NE_STR",
        EQ_BOOL => "EQ_BOOL",
        NE_BOOL => "NE_BOOL",
        NOT => "NOT",
        NEG_INT => "NEG_INT",
        NEG_FLOAT => "NEG_FLOAT",
        JUMP => "JUMP",
        JUMP_IF_FALSE => "JUMP_IF_FALSE",
        CALL => "CALL",
        BUILTIN => "BUILTIN",
        EQ_LIST => "EQ_LIST",
        NE_LIST => "NE_LIST",
        RET => "RET",
        RET_UNIT => "RET_UNIT",
        PRINT => "PRINT",
        _ => return None,
    })
}

// ---------------------------------------------------------------------------
// Binary container
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct BcError {
    pub code: &'static str,
    pub message: String,
}

impl BcError {
    fn new(code: &'static str, message: impl Into<String>) -> BcError {
        BcError {
            code,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for BcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)
    }
}

fn put_u32(w: &mut Vec<u8>, v: u32) {
    w.extend_from_slice(&v.to_le_bytes());
}

fn put_str(w: &mut Vec<u8>, s: &str) {
    put_u32(w, s.len() as u32);
    w.extend_from_slice(s.as_bytes());
}

pub fn serialize(m: &BytecodeModule) -> Vec<u8> {
    let mut w = Vec::new();
    w.extend_from_slice(MAGIC);
    w.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    put_u32(&mut w, m.consts.len() as u32);
    for c in &m.consts {
        match c {
            BcConst::Int(i) => {
                w.push(1);
                w.extend_from_slice(&i.to_le_bytes());
            }
            BcConst::Float(f) => {
                w.push(2);
                w.extend_from_slice(&f.to_le_bytes());
            }
            BcConst::Str(s) => {
                w.push(3);
                put_str(&mut w, s);
            }
            BcConst::Bool(b) => {
                w.push(4);
                w.push(*b as u8);
            }
            BcConst::Unit => w.push(5),
        }
    }
    put_u32(&mut w, m.funcs.len() as u32);
    for f in &m.funcs {
        put_str(&mut w, &f.name);
        w.push(f.arity);
        w.extend_from_slice(&f.nregs.to_le_bytes());
        w.push(f.ret.tag());
        put_u32(&mut w, f.code.len() as u32);
        w.extend_from_slice(&f.code);
    }
    put_str(&mut w, &m.funcs[m.entry].name);
    w
}

struct Reader<'a> {
    b: &'a [u8],
    p: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], BcError> {
        if self.p + n > self.b.len() {
            return Err(BcError::new(
                "E4003",
                "truncated bytecode file (unexpected end of data)",
            ));
        }
        let s = &self.b[self.p..self.p + n];
        self.p += n;
        Ok(s)
    }

    fn u8(&mut self) -> Result<u8, BcError> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, BcError> {
        let b = self.take(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    fn u32(&mut self) -> Result<u32, BcError> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn i64(&mut self) -> Result<i64, BcError> {
        let b = self.take(8)?;
        Ok(i64::from_le_bytes(b.try_into().unwrap()))
    }

    fn f64v(&mut self) -> Result<f64, BcError> {
        let b = self.take(8)?;
        Ok(f64::from_le_bytes(b.try_into().unwrap()))
    }

    fn str(&mut self) -> Result<String, BcError> {
        let len = self.u32()? as usize;
        let b = self.take(len)?;
        String::from_utf8(b.to_vec())
            .map_err(|_| BcError::new("E4001", "bytecode contains invalid UTF-8 in a string"))
    }
}

pub fn deserialize(bytes: &[u8]) -> Result<BytecodeModule, BcError> {
    let mut r = Reader { b: bytes, p: 0 };
    if r.take(4)? != MAGIC {
        return Err(BcError::new(
            "E4001",
            "not a Lutetium bytecode file (bad magic)",
        ));
    }
    let version = r.u16()?;
    if version != FORMAT_VERSION {
        return Err(BcError::new(
            "E4001",
            format!(
                "unsupported bytecode version {} (this runtime supports {})",
                version, FORMAT_VERSION
            ),
        ));
    }

    let nconsts = r.u32()? as usize;
    let mut consts = Vec::with_capacity(nconsts.min(4096));
    for _ in 0..nconsts {
        let tag = r.u8()?;
        let c = match tag {
            1 => BcConst::Int(r.i64()?),
            2 => BcConst::Float(r.f64v()?),
            3 => BcConst::Str(r.str()?),
            4 => BcConst::Bool(r.u8()? != 0),
            5 => BcConst::Unit,
            _ => {
                return Err(BcError::new(
                    "E4001",
                    format!("unknown constant tag {} in bytecode", tag),
                ))
            }
        };
        consts.push(c);
    }

    let nfuncs = r.u32()? as usize;
    let mut funcs = Vec::with_capacity(nfuncs.min(4096));
    for _ in 0..nfuncs {
        let name = r.str()?;
        let arity = r.u8()?;
        let nregs = r.u16()?;
        let ret_tag = r.u8()?;
        let ret = BcTy::from_tag(ret_tag).ok_or_else(|| {
            BcError::new(
                "E4001",
                format!("unknown return type tag {} in bytecode", ret_tag),
            )
        })?;
        let codelen = r.u32()? as usize;
        let code = r.take(codelen)?.to_vec();
        funcs.push(BcFunction {
            name,
            arity,
            nregs,
            ret,
            code,
        });
    }

    let entry_name = r.str()?;
    let entry = funcs
        .iter()
        .position(|f| f.name == entry_name)
        .ok_or_else(|| {
            BcError::new(
                "E4001",
                format!(
                    "bytecode entry point `{}` is missing from the function table",
                    entry_name
                ),
            )
        })?;

    Ok(BytecodeModule {
        consts,
        funcs,
        entry,
    })
}

// ---------------------------------------------------------------------------
// Disassembler (used by `lutetium inspect`)
// ---------------------------------------------------------------------------

fn read_u16(code: &[u8], p: &mut usize) -> u16 {
    let v = u16::from_le_bytes([code[*p], code[*p + 1]]);
    *p += 2;
    v
}

fn read_u32(code: &[u8], p: &mut usize) -> u32 {
    let v = u32::from_le_bytes([code[*p], code[*p + 1], code[*p + 2], code[*p + 3]]);
    *p += 4;
    v
}

fn read_i32(code: &[u8], p: &mut usize) -> i32 {
    read_u32(code, p) as i32
}

/// Disassemble a module to text for `lutetium inspect`.
pub fn disassemble(m: &BytecodeModule) -> String {
    let mut out = String::new();
    let _ = std::fmt::Write::write_fmt(
        &mut out,
        format_args!(
            "module: {} function(s), {} constant(s), entry `{}`\n\n",
            m.funcs.len(),
            m.consts.len(),
            m.funcs[m.entry].name
        ),
    );
    for (i, c) in m.consts.iter().enumerate() {
        let v = match c {
            BcConst::Int(v) => v.to_string(),
            BcConst::Float(v) => v.to_string(),
            BcConst::Bool(v) => v.to_string(),
            BcConst::Str(s) => format!("{:?}", s),
            BcConst::Unit => "unit".to_string(),
        };
        let _ = std::fmt::Write::write_fmt(&mut out, format_args!("  c{} = {}\n", i, v));
    }
    if !m.consts.is_empty() {
        out.push('\n');
    }
    for (fi, f) in m.funcs.iter().enumerate() {
        let code: &[u8] = &f.code;
        let _ = std::fmt::Write::write_fmt(
            &mut out,
            format_args!(
                "fn {} #{} (arity {}, {} register(s), ret {}), {} code bytes\n",
                f.name,
                fi,
                f.arity,
                f.nregs,
                f.ret.name(),
                code.len()
            ),
        );
        let mut p = 0usize;
        while p < code.len() {
            let start = p;
            let op = code[p];
            p += 1;
            let shape = match op_shape(op) {
                Some(s) => s,
                None => {
                    let _ = std::fmt::Write::write_fmt(
                        &mut out,
                        format_args!("  {:04}  <invalid opcode 0x{:02x}>\n", start, op),
                    );
                    break;
                }
            };
            let text = match shape {
                OpShape::Const => {
                    let ci = read_u32(code, &mut p) as usize;
                    let d = read_u16(code, &mut p);
                    let v = match m.consts.get(ci) {
                        Some(BcConst::Int(v)) => v.to_string(),
                        Some(BcConst::Float(v)) => v.to_string(),
                        Some(BcConst::Bool(v)) => v.to_string(),
                        Some(BcConst::Str(s)) => format!("{:?}", s),
                        Some(BcConst::Unit) => "unit".to_string(),
                        None => "<bad index>".to_string(),
                    };
                    format!("{} c{}, r{}  ; {}", opcode_name(op).unwrap(), ci, d, v)
                }
                OpShape::Unit => {
                    let d = read_u16(code, &mut p);
                    format!("{} r{}", opcode_name(op).unwrap(), d)
                }
                OpShape::Move => {
                    let a = read_u16(code, &mut p);
                    let d = read_u16(code, &mut p);
                    format!("{} r{}, r{}", opcode_name(op).unwrap(), a, d)
                }
                OpShape::Ternary => {
                    let a = read_u16(code, &mut p);
                    let b = read_u16(code, &mut p);
                    let d = read_u16(code, &mut p);
                    format!("{} r{}, r{}, r{}", opcode_name(op).unwrap(), a, b, d)
                }
                OpShape::Sink => {
                    let a = read_u16(code, &mut p);
                    format!("{} r{}", opcode_name(op).unwrap(), a)
                }
                OpShape::Binary => {
                    let a = read_u16(code, &mut p);
                    let d = read_u16(code, &mut p);
                    format!("{} r{}, r{}", opcode_name(op).unwrap(), a, d)
                }
                OpShape::Branch => {
                    let c = read_u16(code, &mut p);
                    let t = read_i32(code, &mut p);
                    format!("{} r{}, {:04}", opcode_name(op).unwrap(), c, t)
                }
                OpShape::Jump => {
                    let t = read_i32(code, &mut p);
                    format!("{} {:04}", opcode_name(op).unwrap(), t)
                }
                OpShape::Call => {
                    let fi = read_u16(code, &mut p) as usize;
                    let argc = code[p] as usize;
                    p += 1;
                    let mut args = Vec::new();
                    for _ in 0..argc {
                        args.push(read_u16(code, &mut p));
                    }
                    let d = read_u16(code, &mut p);
                    let name = m.funcs.get(fi).map(|f| f.name.as_str()).unwrap_or("?");
                    let args_s = args
                        .iter()
                        .map(|a| format!("r{}", a))
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!(
                        "{} {}({}) -> r{}",
                        opcode_name(op).unwrap(),
                        name,
                        args_s,
                        d
                    )
                }
                OpShape::Ret => {
                    let a = read_u16(code, &mut p);
                    format!("{} r{}", opcode_name(op).unwrap(), a)
                }
                OpShape::Builtin => {
                    let kind = f.code[p];
                    p += 1;
                    let d = read_u16(code, &mut p);
                    let argc = code[p] as usize;
                    p += 1;
                    let mut args = Vec::new();
                    for _ in 0..argc {
                        args.push(read_u16(code, &mut p));
                    }
                    let kind_name = BuiltinKind::from_u8(kind)
                        .map(|k| k.name().to_string())
                        .unwrap_or(format!("kind 0x{:02x}", kind));
                    let args_s = args
                        .iter()
                        .map(|a| format!("r{}", a))
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!("BUILTIN {} ({}) -> r{}", kind_name, args_s, d)
                }
                OpShape::None => opcode_name(op).unwrap().to_string(),
            };
            let _ =
                std::fmt::Write::write_fmt(&mut out, format_args!("  {:04}  {}\n", start, text));
        }
        out.push('\n');
    }
    out
}
