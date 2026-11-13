# STANDARD_LIBRARY.md

Lutetium does not have an import system yet; the "standard library" is the
set of built-in free functions and methods the checker and VM resolve
directly. Every entry here is implemented and tested
(`crates/compiler/tests/end_to_end.rs`).

## Built-in free functions

| Name | Signature | Notes |
|---|---|---|
| `sqrt` | `(float) -> float` | |
| `pow` | `(float, float) -> float` | |
| `floor` | `(float) -> float` | |
| `ceil` | `(float) -> float` | |
| `round` | `(float) -> float` | half away from zero |
| `log` | `(float) -> float` | natural logarithm |
| `sin` / `cos` / `tan` | `(float) -> float` | radians |
| `abs` | `(int) -> int` or `(float) -> float` | overloaded |
| `min` | `(int, int) -> int` or `(float, float) -> float` | overloaded |
| `max` | `(int, int) -> int` or `(float, float) -> float` | overloaded |
| `read_text` | `(str) -> str` | whole file; missing file → E5009 |
| `write_text` | `(str, str) -> unit` | overwrites; creates the file |
| `file_exists` | `(str) -> bool` | |

Overloads are selected statically: `abs`/`min`/`max` take the int variants
only when every argument is statically `int`, otherwise the float variants
(`int` widens to `float` implicitly).

Wrong arity → E3007. Wrong argument types → E3001. Unknown names → E3006.

## Built-in methods

### Lists (`[a, b, c]`)

| Method | Signature | Notes |
|---|---|---|
| `len` | `() -> int` | |
| `push` | `(element) -> unit` | grows in place |
| `pop` | `() -> element` | empty list → E5009 |
| `get` | `(int) -> element` | out of bounds → E5009; negative index counts from the end |
| `set` | `(int, element) -> unit` | same index rules |

### Strings

| Method | Signature | Notes |
|---|---|---|
| `len` | `() -> int` | byte length (UTF-8) |
| `upper` | `() -> str` | |
| `lower` | `() -> str` | |
| `trim` | `() -> str` | removes leading/trailing whitespace |
| `contains` | `(str) -> bool` | |
| `replace` | `(str, str) -> str` | all occurrences |
| `split` | `(str) -> list str` | empty separator splits between every character |

### Maps (`{name: "Ada", score: 95}`)

Record maps: the key set is fixed at literal construction; keys are
identifiers read as string keys; values are per-key typed scalars.

| Method | Signature | Notes |
|---|---|---|
| `len` | `() -> int` | |
| `get` | `(literal str) -> field type` | key must be a literal; unknown key → E3010 |
| `set` | `(literal str, value) -> unit` | overwrites an existing key only; value must match the field's type (int → float widens) |

Field access `m.key` is equivalent to `m.get("key")` with compile-time key
checking (unknown field → E3010). Reference semantics like lists.

Unknown method for the receiver's type → E3015.

## Value printing (`print`)

`print(x)` prints one value plus a newline: ints as digits, floats always
with a decimal point (`2.0`), bools as `true`/`false`, strings raw, lists as
`[1, 2, 3]` and maps as `{name: Ada}` (recursively).

## Planned (not implemented — do not use)

`math`, `text`, `collections`, `io`, `fs`, `path`, `time`, `random`,
`system`, `process` modules, the `import` statement, sets, heterogeneous
dynamic-key maps (records above have fixed keys), list/map equality,
nesting (lists of lists/maps). Tracked in `ROADMAP.md` (Phase 4); none of
these names resolve today except the built-ins above.
