# ACCEPTANCE_TESTS.md

How to verify that each shipped feature actually works. Run from the repo
root. All commands must pass with the current tree.

## Full suite (every session, before marking anything VERIFIED)

```powershell
cargo build --workspace
cargo test --workspace     # baseline: 117 tests
cargo build --release
cargo fmt --all -- --check
```

## 0.4 features

### Lists

```lu
# examples/lists.lu or an inline script:
let xs = [1, 2, 3]
xs.push(4)
let n = xs.len()        # 4
let y = xs.get(1)       # 2
xs.set(0, 10)
let p = xs.pop()        # 4
```

* `lutetium run <file>` succeeds and prints expected values.
* Negative: out-of-range index → E5009; mixing element types → E300x
  diagnostic; both covered in `end_to_end.rs`.

### Builtins

* `sqrt(2.0) > 1.4` runs; `abs(-3) == 3` (int, not float);
  `read_text` on a missing file → E5009 at runtime.
* Covered by e2e tests C10 (19 tests) — keep passing.

### GC

```powershell
lutetium run examples\gc_stress.lu --gc-stats
lutetium run examples\gc_stress.lu --gc-stress
```

* Stats line prints real (non-zero where applicable) counters; stress mode
  forces collections; program output identical with and without stress.

### Native gate

```powershell
lutetium build build\native_list.lu --target native
```

* Exits non-zero with E6002 explaining lists/builtins are not yet supported
  natively. Scalar programs still produce working exes
  (11 native-exe integration tests).

### Diagnostics

```powershell
lutetium explain E3015
lutetium check file.lu --diagnostics json
```

* explain prints every registered code; JSON mode is valid JSON.

## Per-task acceptance (added as tasks land)

* **T1 maps** ✅ VERIFIED 2026-11-12: `{name: "Ada"}` + `.field` + `len/get/set`
  positive tests (map_literals_and_field_access, map_get_set_methods,
  maps_flow_through_functions, maps_survive_gc_collections,
  optimizer_preserves_maps); negatives E3010 (unknown field / added key /
  dynamic key / non-string key / `==`), E3001 (nesting, map-in-list,
  list-in-map), E3016 (128 entries), E3015 (unknown map method); GC stress
  with 2000 map allocations; examples/maps.lu runs under the installed 0.4.1
  binary; suite green (149).
* **T2 for-in**: iterate, mutate inside loop, run under --gc-stress; equality
  builtin tested both directions.
* **T3 gc-stats**: CLI test asserts minor_collections > 0 with --gc-stress.
* **T7 optimizer**: level-3 output is semantically equivalent on a
  differential suite; no test regressions.
* **T8 fmt**: formatting examples/*.lu twice yields identical files
  (idempotency); string contents unchanged.
* **T11 install** ✅ VERIFIED 2026-11-12: install.ps1 → global binary 0.4.1,
  `lutetium run` on examples/maps.lu works from the installation directory,
  dist zip rebuilt.
