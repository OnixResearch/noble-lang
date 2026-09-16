# Native and dependency assurance

**Tasks 3.3 and 3.4 own this capability.** The Nix `native-assurance` check is task 4.4's.

This is scope accounting and interpreter evidence. It is not soundness, not dependency safety, and not M1 acceptance.

## Commands

```sh
nix --option min-free 0 --option build-dir /nix/var/nix/builds --builders '' \
  run .#native-assurance -- inventory --root "$ROOT" --selection "$ROOT/policy/tool-selection.json" \
    --artifact-dir "$EVIDENCE/inventory"
nix run .#native-assurance -- check --root "$ROOT" --selection "$ROOT/policy/tool-selection.json" \
  --inventory "$EVIDENCE/inventory/inventory.json" --policy "$ROOT/policy/native-assurance.json" \
  --artifact-dir "$EVIDENCE/check"
```

[The reviewed policy](../policy/native-assurance.ncl) owns the expectations; its JSON export is checked, not refreshed, by the `policy` check.

## Inventory

`nix/native-assurance-derive.nix` derives the inventory from compiler and Cargo facts:

| Fact | Source |
|---|---|
| packages, units, target, features | compiler architecture IR units and the reviewed selection |
| owned unsafe sites | compiler `function-safety` facts with `is_unsafe` |
| foreign items | compiler item facts with `item_kind = foreign` |
| production dependencies | Cargo graph edges whose callee is outside the selected workspace packages |
| fixture packages | the checksum-bound randomness fixture lock |
| compiler tools | the reviewed tool set |
| foreign libraries | none linked |

The observed named M1 scope is 6 units, 46 production items, **0** function-safety facts, **0** unsafe sites, **0** foreign items, **0** production dependencies, and 6 fixture-lock packages (1 direct, 5 transitive).

## Empty owned-unsafe scope

The empty scope is recorded only with positive accounting:

- the workspace manifest forbids `unsafe_code`;
- the compiler reports no function-safety fact;
- the compiler reports no foreign item;
- no owned unsafe site exists.

Each condition is a separate comparator diagnostic, so an empty scope cannot be asserted while accounting is absent. A reviewed unsafe site that the compiler does not observe is also rejected.

## Miri controls

Pinned required configuration: toolchain `nightly-2026-08-18`, target `x86_64-unknown-linux-gnu`, flags `-Zmiri-strict-provenance`, subject `crates/noble-kernel/Cargo.toml`, required test `budget`.

| Control | Result |
|---|---|
| Positive baseline | `cargo miri setup` passed; `cargo miri test` reported 2 passed, 0 failed |
| Negative: strict-provenance violation | A fixture crate performing an integer-to-pointer read fails: `integer-to-pointer casts and ptr::with_exposed_provenance are not supported with -Zmiri-strict-provenance` |
| Retained negative hypothesis | `--target wasm32-unknown-unknown` was **accepted**: Miri prepared a wasm32 sysroot and the tests passed. It is not an unsupported-target control |

Unsupported required configurations are rejected by the reviewed comparator (`miri-unsupported-configuration`, `miri-configuration-mismatch`), not by assuming Miri refuses a target.

## Comparator controls

Twenty-four synthetic self-tests cover scope packages, target and feature drift, missing unsafe-forbid evidence, missing accounting basis, observed unsafe or foreign facts, unreviewed and absent unsafe sites, unclassified dependencies, missing fixture records, unclassified tools and foreign libraries, missing safety arguments, missing target assumptions, and each Miri requirement.

## Limits

Dependency records are not dependency safety. Interpreter results are not refinement. The inventory covers the named scope only.
