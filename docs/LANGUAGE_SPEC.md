# LANGUAGE_SPEC.md — Lutetium 0.4

This document describes the implemented subset of Lutetium, version 0.4.
Everything here is implemented and tested. Features not listed (maps in
source syntax, structs, enums, pattern matching, generics, closures, async,
string interpolation, exponent literals) are planned for later milestones and
must not be assumed to exist.

## Program structure and entry points

A module contains any mix of top-level `fn` definitions and executable
statements:

* With no top-level statements, the module must define `main` (E2102).
* With top-level statements, a synthetic `__lutetium_init` entry point is
  generated: it executes the top-level statements in order, then calls `main`
  if one is defined. `main` is optional in that case.
* Top-level statements may use implicit declarations (`name = value`) — the
  first plain assignment to an unknown identifier declares a mutable,
  type-inferred local (E3002 if the type cannot be inferred).
* `return` is not allowed in top-level code (E3014).
* Imported modules do not exist yet; their initialization order will be
  specified together with the module system and will not run arbitrary
  application code.

## Lexical structure

* Comments: `#` to end of line.
* Statements are separated by newlines (or `;`); blocks are brace-delimited.
  There is no significant indentation.
* Identifiers: `[A-Za-z_][A-Za-z0-9_]*`. Keywords: `fn let mut if else while
  for in return break continue true false`.
* Integer literals: digits with `_` separators, 64-bit signed (`int`). Overflow
  in a literal is E1101.
* Float literals: `12.5` (a `.` followed by at least one digit; 64-bit `float`).
* Strings: `"..."`, single line, escapes `\n \t \r \0 \\ \"`.

## Grammar (EBNF)

The authoritative grammar lives in `docs/GRAMMAR.ebnf`. Informally:

```ebnf
module      = { function | statement } ;
function    = "fn" , IDENT , "(" , [ params ] , ")" , [ "->" , type ] , block ;
params      = param , { "," , param } ;
param       = IDENT , [ ":" , type ] ;
statement   = "let" [ "mut" ] IDENT [ ":" type ] "=" expr
            | "if" expr block { "else" ( if_stmt | block ) }
            | "while" expr block
            | "for" IDENT "in" add_expr ( ".." | "..=" ) add_expr block
            | "return" [ expr ]
            | "break" | "continue"
            | IDENT assign_op expr
            | expr ;
if_expr     = "if" expr block "else" ( if_expr | block ) ;   (* value position: else required *)
assign_op   = "=" | "+=" | "-=" | "*=" | "/=" ;
expr        = or_expr ;
or_expr     = and_expr { "||" and_expr } ;
and_expr    = cmp_expr { "&&" cmp_expr } ;
cmp_expr    = add_expr [ ( "==" | "!=" | "<" | "<=" | ">" | ">=" ) add_expr ] ;
add_expr    = mul_expr { ( "+" | "-" ) mul_expr } ;
mul_expr    = unary { ( "*" | "/" | "%" ) unary } ;
unary       = ( "-" | "!" ) unary | postfix ;
postfix     = primary { "[" expr "]" | "." IDENT "(" [ args ] ")" | "." IDENT } ;
primary     = INT | FLOAT | STRING | "true" | "false"
            | IDENT [ "(" [ args ] ")" ]
            | "[" [ args ] "]"            (* list literal *)
            | "{" [ map_entry { "," map_entry } ] "}"   (* map literal *)
            | if_expr
            | "(" expr ")" ;
map_entry   = IDENT ":" expr ;
```

## Types

| Type | Values |
|---|---|
| `int` | 64-bit signed |
| `float` | 64-bit IEEE 754 |
| `bool` | `true` / `false` |
| `str` | immutable UTF-8 string |
| `list` | growable sequence of scalars of one element type |
| `unit` | the type of side-effecting expressions |

The static checker tracks list element types (`list int`, `list float`, ...)
even though the syntax has no list type annotation yet.

## Type rules

* Function parameters may be annotated. Unannotated parameter types are
  inferred monomorphically from all call sites (conflicting call sites →
  E3001; uninferable → E3002). A return type may be omitted, in which case it
  is inferred from `return` statements and expression bodies (recursive
  functions with inferred returns must be annotated — E3002).
* Variables: `let x = ...` (immutable) or `let mut x = ...` or the implicit
  `x = ...` form at statement level (mutable). Re-declaring a name in the same
  block is E3011; shadowing in a nested block is allowed.
* The only implicit conversion is `int` -> `float` (widen on demand in
  arithmetic, comparisons, assignment, calls and list elements). There is no
  narrowing and no silent precision loss.
* Conditions of `if`/`while` must be `bool` (E3009).
* Operators:
  * `+ - * / %` on two ints -> int; on two floats -> float; `+` on two strings
    concatenates. Everything else is E3010.
  * `== !=` on any two values of the same type; `< <= > >=` on two ints or two
    floats. Results are `bool`.
  * `&& ||` are short-circuiting (the right operand is not evaluated when the
    result is decided); `!` negates a bool.
  * Indexing `list[int]` yields the element type; the index must be `int`
    (E3010).
* A function returning a value must end with a `return` statement (E3003) —
  except expression-bodied functions, whose trailing expression is the
  implicit return.
* `main` exists, takes no parameters and returns `unit` (E2102).
* `print(expr)` is a built-in that prints one value and a newline.

### Lists

* Literals: `[a, b, c]`. All elements must have one type; `int` elements widen
  when a `float` element appears. Nested lists are rejected (E3001). Empty
  literals cannot infer their element type (E3002).
* Methods (receiver is the first argument of the underlying builtin):
  * `list`: `len() -> int`, `push(v) -> unit`, `pop() -> element`,
    `get(i) -> element`, `set(i, v) -> unit`.
  * `str`: `len() -> int`, `upper() -> str`, `lower() -> str`,
    `trim() -> str`, `contains(s) -> bool`, `replace(a, b) -> str`,
    `split(sep) -> list str`.
* Unknown methods are E3015; wrong arity is E3007; wrong argument types are
  E3001. `pop` on an empty list is a runtime error (E5009).
* Lists are heap objects with reference semantics: `lst.push(1)` mutates every
  alias of `lst`.

### Maps

* Literals: `{name: "Ada", score: 95}`. Keys are identifiers, read as string
  keys; a later duplicate key overwrites the earlier one. Values are per-key
  typed scalars (`int`/`float`/`bool`/`str`) — mixed types are fine; nested
  lists/maps as values are rejected (E3001). At most 127 entries (E3016).
  An empty literal `{}` is a valid map with no fields.
* Record semantics: the key set is fixed at literal construction. Field
  access `m.key` (compile-time checked, unknown field → E3010) and
  `m.get("key")` (literal key required) yield the field's value type;
  `m.set("key", v)` overwrites an existing key (checked against the field's
  type, with the usual `int` -> `float` widening) and cannot add new keys
  (E3010); `m.len() -> int`. Dynamic keys (`m.get(k)` with a variable `k`)
  are E3010.
* Maps are heap objects with reference semantics, like lists; mutating a map
  is visible through every alias.
* In expression position a `{` always starts a map literal; blocks only
  appear after `fn`/`if`/`while`/`for`/`else`.

### Built-in free functions

`sqrt pow floor ceil round log sin cos tan` (float arguments and results),
`abs min max` (overloaded: all-int arguments select the int variants and
return int, otherwise float), `read_text(path) -> str`, `write_text(path,
content) -> unit`, `file_exists(path) -> bool`. Unknown names are E3006;
wrong arity is E3007.

## Runtime semantics

* Integer arithmetic is checked: division/modulo by zero raises E5002;
  overflow raises E5005. Float division follows IEEE 754 (inf/NaN).
* Integer division truncates toward zero; `%` takes the sign of the dividend.
* Deep recursion (1024 frames) raises E5001.
* Failed built-ins raise E5009 with the kind name: list index out of bounds,
  `pop` from an empty list, `map_get` on an absent key, `read_text` on a
  missing file, type mismatches. Negative list indices count from the end
  (`lst.get(-1)` is the last element).
* `print` formatting: ints as digits, floats with a decimal point (e.g. `2.0`),
  bools as `true`/`false`, strings raw, lists as `[1, 2, 3]` and maps as
  `{k: v}` (recursively).
* Memory: list and map objects live in a precise generational tracing GC heap
  (`crates/vm/src/heap.rs`); unreachable objects are reclaimed automatically
  (see `docs/RUNTIME_ARCHITECTURE.md`).

## Execution

`lutetium run` compiles to bytecode, verifies it (never executes unverified
bytecode), and executes on the register-machine VM. `lutetium build --target
bytecode` writes `build/<name>.lbc`. `lutetium build --target native`
compiles through the Cranelift backend to a standalone Windows `.exe` —
native mode supports scalars (int/float/bool/str) and rejects lists, maps and
built-in calls with E6002 until their runtime support lands.
