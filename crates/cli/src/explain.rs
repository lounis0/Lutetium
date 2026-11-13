//! Static explanations for the stable error codes, shared with
//! `ERROR_CODES.json` at the repository root.

pub struct Explained {
    pub code: &'static str,
    pub title: &'static str,
    pub body: &'static str,
    pub fix: &'static str,
}

const EXPLANATIONS: &[Explained] = &[
    Explained {
        code: "E1001",
        title: "lexical error",
        body: "The lexer found a character or token it cannot understand: an unexpected symbol, an unknown string escape, or a string literal that is never closed.",
        fix: "Check the highlighted characters. Strings must close on the same line with `\"` and only support \\n \\t \\r \\0 \\\\ \\\" escapes.",
    },
    Explained {
        code: "E1101",
        title: "integer literal overflow",
        body: "An integer literal does not fit in the 64-bit signed `int` type.",
        fix: "Use a smaller value, or write the literal as a float (e.g. `1.0e19` is not supported yet; use `9223372036854775807` as the maximum).",
    },
    Explained {
        code: "E2001",
        title: "syntax error",
        body: "The parser found a token it did not expect at this position.",
        fix: "Compare the highlighted line with the grammar in docs/LANGUAGE_SPEC.md; check for a missing brace, colon or comma.",
    },
    Explained {
        code: "E2002",
        title: "expected an expression",
        body: "An expression was required here (after `=`, inside `(...)`, as a condition, ...).",
        fix: "Write a literal, a variable, a call, or a parenthesized expression at the highlighted position.",
    },
    Explained {
        code: "E2005",
        title: "method calls are not supported yet",
        body: "Lutetium 0.1 has no method-call syntax (`value.method()`); it is planned for the language-expansion milestone.",
        fix: "Use free functions for now.",
    },
    Explained {
        code: "E2006",
        title: "if used as a value needs an else branch",
        body: "When an `if` appears in value position (for example on the right side of `=` or inside an argument), both branches must exist and produce compatible types, because the expression evaluates to a value.",
        fix: "Add `else { ... }`, or use the statement form when no value is needed.",
    },
    Explained {
        code: "E2101",
        title: "duplicate definition",
        body: "Two functions share the same name (or a function is named `print`, which is built in).",
        fix: "Remove or rename one of the definitions.",
    },
    Explained {
        code: "E2102",
        title: "missing `main`",
        body: "Every runnable Lutetium program needs an entry point named `main` that takes no parameters.",
        fix: "Add `fn main() { ... }`.",
    },
    Explained {
        code: "E3001",
        title: "type mismatch",
        body: "Two types that cannot be combined met each other: an initializer, an assignment, an argument, or a return value has the wrong type. The only implicit conversion is `int` -> `float`; there is no silent narrowing or precision loss.",
        fix: "Add an explicit conversion where needed, or fix the declared type.",
    },
    Explained {
        code: "E3002",
        title: "cannot infer / conflicting return type",
        body: "A function without `-> type` returned values of different types, or its return type could not be inferred (typically because it is recursive).",
        fix: "Annotate the return type, e.g. `fn f(x: int) -> int { ... }`.",
    },
    Explained {
        code: "E3003",
        title: "missing return",
        body: "A function that returns a value must end with a `return` statement.",
        fix: "Add a `return` as the last statement of the function body.",
    },
    Explained {
        code: "E3004",
        title: "unknown type",
        body: "A type annotation names something that is not a built-in type.",
        fix: "Use one of: int, float, bool, str, unit.",
    },
    Explained {
        code: "E3005",
        title: "unknown variable",
        body: "The variable is not defined in any enclosing scope at this point.",
        fix: "Define it with `let` before use, or fix the spelling.",
    },
    Explained {
        code: "E3006",
        title: "unknown function",
        body: "No function with this name exists in the module.",
        fix: "Check the spelling, or define the function with `fn`.",
    },
    Explained {
        code: "E3007",
        title: "argument count mismatch",
        body: "A call passes a different number of arguments than the function declares.",
        fix: "Match the call site to the definition's parameter list.",
    },
    Explained {
        code: "E3008",
        title: "assignment to immutable variable",
        body: "Variables are immutable by default in Lutetium.",
        fix: "Declare the variable with `let mut` if it needs to change.",
    },
    Explained {
        code: "E3009",
        title: "condition must be bool",
        body: "`if` and `while` conditions must have type `bool`; numbers are not truthy/falsy in Lutetium.",
        fix: "Write an explicit comparison, e.g. `while i < 10 { ... }`.",
    },
    Explained {
        code: "E3010",
        title: "invalid operands",
        body: "The operator is not defined for these operand types. `+` works on two numbers or two strings; arithmetic needs two ints or two floats.",
        fix: "Make both operands the same numeric type, or use `+` only to concatenate two strings.",
    },
    Explained {
        code: "E3011",
        title: "duplicate binding in scope",
        body: "Two `let` bindings with the same name exist in the same block.",
        fix: "Rename one variable, or remove the earlier binding.",
    },
    Explained {
        code: "E3012",
        title: "`break` outside a loop",
        body: "`break` only makes sense inside `while`.",
        fix: "Move the `break` into a loop body.",
    },
    Explained {
        code: "E3013",
        title: "`continue` outside a loop",
        body: "`continue` only makes sense inside `while` or `for`.",
        fix: "Move the `continue` into a loop body.",
    },
    Explained {
        code: "E3014",
        title: "`return` outside a function",
        body: "Top-level (implicit entry) statements run before `main`; they cannot return.",
        fix: "Move the `return` into a function.",
    },
    Explained {
        code: "E3015",
        title: "unknown method",
        body: "Method calls (`value.name(args)`) are only defined for the built-in methods: lists support `len`, `push`, `pop`, `get`, `set`; strings support `len`, `upper`, `lower`, `trim`, `contains`, `replace`, `split`; maps support `len`, `get`, `set`.",
        fix: "Use one of the built-in methods for the receiver's type, or define a free function with `fn`.",
    },
    Explained {
        code: "E3016",
        title: "map literal too large",
        body: "A map literal lowers to a single `BUILTIN map_new` instruction whose operand count is stored in one byte, so a literal may have at most 127 `key: value` entries.",
        fix: "Split the map literal across several maps, or build larger maps with `set` in a loop.",
    },
    Explained {
        code: "E4001",
        title: "invalid bytecode file",
        body: "The file is not a Lutetium bytecode module: bad magic, unsupported format version, invalid UTF-8 or a broken constant/function table.",
        fix: "Rebuild it from source with `lutetium build <file.lu>`.",
    },
    Explained {
        code: "E4002",
        title: "bytecode verification failed",
        body: "The module failed structural verification: unknown opcode, out-of-range operand, call/arity mismatch, register read before write, or control flow falling off the end. Lutetium never executes unverified bytecode.",
        fix: "Recompile from source; if you wrote the bytecode by hand, fix it to conform to docs/BYTECODE_FORMAT.md.",
    },
    Explained {
        code: "E4003",
        title: "truncated bytecode",
        body: "The file ends in the middle of a value or instruction.",
        fix: "Rebuild the module from source.",
    },
    Explained {
        code: "E5001",
        title: "call stack overflow",
        body: "Recursion exceeded the runtime's call-depth limit (1024 frames).",
        fix: "Reduce the recursion depth or rewrite the function iteratively.",
    },
    Explained {
        code: "E5002",
        title: "integer division or modulo by zero",
        body: "The right operand of `/` or `%` on ints was zero.",
        fix: "Check the divisor before dividing.",
    },
    Explained {
        code: "E5003",
        title: "corrupted bytecode at runtime",
        body: "The VM met an invalid opcode or a constant of the wrong kind. This cannot happen for verified modules.",
        fix: "Recompile from source.",
    },
    Explained {
        code: "E5004",
        title: "runtime type error",
        body: "An instruction received a value of an unexpected type (for example reading a register that was never written).",
        fix: "Recompile from source; report it if the source type-checked.",
    },
    Explained {
        code: "E5005",
        title: "integer overflow",
        body: "An arithmetic operation on ints overflowed the 64-bit signed range. Lutetium never wraps silently.",
        fix: "Use smaller operands, or floats when values may exceed the int range.",
    },
    Explained {
        code: "E6001",
        title: "native backend not available",
        body: "`lutetium build --target native` requires the Cranelift-based native backend, planned for milestone 2.",
        fix: "Use `--target bytecode` for now.",
    },
];

pub fn explain_code(code: &str) -> Option<&'static Explained> {
    EXPLANATIONS.iter().find(|e| e.code == code)
}
