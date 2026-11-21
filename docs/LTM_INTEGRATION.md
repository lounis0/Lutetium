# Future `.LTM` integration points

## Status: architecture notes only

No `.LTM` file format, parser, loader, agent runtime, tensor runtime, scheduler, or language API is implemented in this repository. Lutetium source currently uses `.lu`; executable semantics come from the compiler, LTBC bytecode VM, or the explicitly supported Windows native subset. These notes identify boundaries a future design may evaluate; they are not a feature promise and must not be presented as working behavior.

## Existing integration seams

| Existing seam | Relevant code | Future design question (not implemented) |
|---|---|---|
| CLI command and project dispatch | `crates/cli/src/lib.rs`, `crates/cli/src/project.rs` | Would a `.LTM` workspace need its own explicit command or manifest? How would it remain separate from the narrow `lutetium.toml` single-entry project? |
| Source frontend | `crates/syntax`, `crates/compiler/src/pipeline.rs` | Would `.LTM` embed `.lu`, describe external assets, or be a different syntax? Do not pass a new extension into the source parser without a format specification. |
| Typed IR/backend boundary | `crates/compiler/src/ir.rs`, `crates/native/src/backend.rs` | Could a future component produce ordinary typed IR, or is an interpreter boundary needed? This repository has no tensor/agent operations to map today. |
| Bytecode execution | `crates/vm/src/bytecode.rs`, `crates/vm/src/verify.rs`, `crates/vm/src/vm.rs` | Any future executable module format would need versioning, bounded decoding, independent verification, capability policy, and explicit runtime semantics before it could be executed. No current VM sandbox exists. |
| Native runtime | `crates/runtime/src/lib.rs`, `crates/native/src/linker.rs` | Which operations, if any, belong in generated native code versus an external host? Current native output is a narrow Windows x64 GNU subset, not a safe agent sandbox. |
| Diagnostics | `crates/diagnostics/src/lib.rs`, `ERROR_CODES.json`, `crates/cli/src/explain.rs` | Future file formats should retain source spans, stable error codes, and machine-readable diagnostics. |
| Native UI | `crates/ion` | A Rust UI host may display status or results, but it is not an agent runtime, model interface, permission system, or `.LTM` loader. |

## Minimum prerequisites before implementation

1. A written `.LTM` format/version specification, ownership and compatibility policy, and concrete positive/negative examples.
2. A threat model for untrusted inputs, filesystem/network access, process execution, resource limits, cancellation, and isolation. The current VM is explicitly not a security sandbox.
3. A clear distinction between declarative project metadata and executable Lutetium code; no silently executing manifest contents.
4. A parser/validator and bounded resource accounting with malformed-input tests before any loader accepts files.
5. A typed interface proposal for any external model/tool providers, including failure, timeout, permissions, and data lifetime semantics; credentials must not be stored in project source or manifests.
6. Acceptance tests that demonstrate actual behavior, plus updated docs and a separately approved implementation scope.

Until these prerequisites are decided and implemented, `.LTM` should remain documentation-only. Do not invent syntax, sample manifests that imply support, fake runtime shims, or stub CLI commands.
