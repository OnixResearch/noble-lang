# Native and dependency assurance

**Tasks 3.3 and 3.4 own this capability.** The Nix `native-assurance` check is task 4.4's.

This is scope accounting and interpreter evidence. It is not soundness, not dependency safety, and not M1 acceptance.

## Commands

```sh
export NIX_CONFIG='min-free = 0
max-free = 0
builders =
sandbox = true
require-sigs = true'
nix run --offline --no-write-lock-file .#native-assurance -- inventory --root "$ROOT" --selection "$ROOT/policy/tool-selection.json" \
    --artifact-dir "$EVIDENCE/inventory"
nix run --offline --no-write-lock-file .#native-assurance -- check --root "$ROOT" --selection "$ROOT/policy/tool-selection.json" \
  --inventory "$EVIDENCE/inventory/inventory.json" --policy "$ROOT/policy/native-assurance.json" \
  --artifact-dir "$EVIDENCE/check"
```

[The reviewed policy](../policy/native-assurance.ncl) owns the expectations; its JSON export is checked, not refreshed, by the `policy` check.

## Inventory

`nix/native-assurance-derive.nix` derives the inventory from compiler and Cargo facts:

| Fact | Source |
|---|---|
| packages, units, target, features | compiler architecture IR units and the reviewed selection |
| owned unsafe sites | compiler `function-safety` facts with `is_unsafe`; vacuous under the current architecture policy (see below) |
| foreign items | compiler item facts with `item_kind = foreign` |
| production dependencies | Cargo graph edges whose callee is outside the selected workspace packages |
| fixture packages | the checksum-bound randomness fixture lock |
| compiler tools | the reviewed tool set |
| foreign libraries | hardcoded empty by the derivation; the comparator check cannot fail and is not a linker-closure audit |

The observed named M1 scope is 6 units, 46 production items, **0** function-safety facts, **0** unsafe sites, **0** foreign items, **0** production dependencies, and 6 fixture-lock packages (1 direct, 5 transitive).

The historical M3 inventory is retained under `verification/m3-wasm/assurance.tar.gz:octet`: 17 units, 2109 item facts across production and test units, zero function-safety/unsafe/foreign facts, and no external production dependencies. Its fixture lock contains eight packages: four then-current workspace packages and the four pinned registry packages. The older M1 counts above describe their original run, not current workspace coverage.
The separate current boundary randomness fixture lock covers the five-package workspace and 75 checksum-bound registry packages; its 110 total package records do not renew either historical inventory.

The immutable selected candidate's 54/54 complete architecture IR derives
54 native inventory units, 11,293 item facts across production and test
units, and zero owned foreign items. Foreign-item absence is real compiler
evidence: the pinned collector records each `extern` block as a `foreign`
item. The derived zero function-safety facts and zero owned unsafe sites are
vacuous. The pinned collector emits a `function-safety` fact only for a
function whose return type is a value-flow-tracked type, and it tracks only
types named by protected outbound effects. All six outbound effects in the
sealed architecture policy are unprotected, so the IR has no such fact among
its 160,618 facts. `is_unsafe` would also cover only `unsafe fn` signatures,
not `unsafe` blocks or implementations. Foreign libraries are hardcoded empty
by the derivation, so that comparator check cannot fail and is not proof:
Wasmtime's build script compiles a C helper that is linked into `noble-cli`.

Its Cargo graph has seven direct external normal edges, all from
`noble-cli`, to `anyhow`, `blake3`, `libc`, `serde_json`, `wasmparser`,
`wasmtime` and `wat`; the selected Cargo.lock resolves them to 1.0.100,
1.8.7, 0.2.189, 1.0.151, 0.243.0, 40.0.2 and 1.243.0. The previous
zero-dependency policy correctly rejected these edges as
`production-dependency-mismatch`. The reviewed policy now records each
edge's caller, name and edge kind, which the comparator matches exactly as
`role->name:edge_kind`. Each record also carries a version and an unproved
safety assumption that the comparator does not compare: changing a recorded
version or emptying an assumption still passes. The versions were compared
independently once against the sealed Cargo.lock, where each matches exactly
one package; no comparator or gate repeats that comparison. The six registry
packages are checksum-bound in Cargo.lock. Wasmtime is a path package with no
Cargo.lock checksum, as are 29 other non-workspace path packages
(13 `cranelift-*`, 2 `pulley-*`, `wasmtime-environ`, 12
`wasmtime-internal-*` and `winch-codegen`); only the tool-selection
Wasmtime source NAR binds their content. This is direct-edge accounting, not
an audit of external unsafe implementations, transitive native dependencies,
linker closure, engine/trap semantics, or source/native safety acceptance.

The preserved source-bound Octet collector exited 1. Its 1,535 static-lint
findings are warning-only; the architecture phase was blocked by 1,501 open
policy findings: 1,494 `required-compiler-unknown` and seven
`forbidden-role-edge`. The unknowns comprise 1,225 unsupported `?`
desugarings, 210 `for` desugarings, ten range desugarings and 49 calls without
compiler-resolved definitions; by package, 984 are in `noble-cli`, 441 in
`noble-contracts`, 57 in `noble-wasm` and 12 in `noble-kernel`. The IR retains
3,621 unknown observations in total; these 1,494 were required compiler
facts. The desugaring and unresolved-definition failures are compiler
observation limits, not missing provider declarations. The seven role edges
were exactly the CLI's unclassified direct normal providers listed above.
Architecture coverage was complete (54/54 units), with zero collection
issues.

The subsequently renewed architecture policy classifies those seven
providers for `composition-root` **normal** edges only: `anyhow`, `libc` and
`wasmtime` as infrastructure (`anyhow` may consult ambient backtrace settings);
`blake3`, `serde_json`, `wasmparser` and `wat` as value/digest/parser providers.
All seven **observed** edges originate
in `noble-cli`; the architecture policy cannot enforce that caller package
because `noble-syndicate` also has the `composition-root` role. The exact
native comparator binds observed caller edges; the separate source comparator
now reviews those exact seven CLI edges but remains red for subject omissions,
stale paths, test subjects, macro origins and reviewed scope. A new syndicate
external edge would fail exact source dependency closure. Neither comparator
runs after a blocked Octet architecture gate. This is a role-granular static
permission, not a retroactive modification of the preserved Octet receipt,
an admission of domain-core dependencies, or native safety attestation.
The 1,494 required unknowns remain open; no waiver or protected-effect
mutation discharges them. A pure architecture replay against the preserved
IR/Cargo graph is separately reported, not a fresh compiler collection.

The **pure native comparator passed** against that sealed compiler/Cargo
inventory and the renewed reviewed policy (`valid = true`, no diagnostics).
On those same real facts, removing one observed dependency edge or changing
its `normal` kind to `build` is refused as `production-dependency-mismatch`.
These one-time diagnostic outputs are retained under
`/home/brittonr/.cache/noble-candidate-native-assurance-20261007/`; the
re-run for this prose-only policy revision, including the unenforced version
and assumption mutations, is under
`/home/brittonr/.cache/noble-candidate-native-d1d3-20261007/`. Pure scope
comparison does **not** turn the Octet exit 1 into a collection application
pass, discharge external/native safety assumptions, accept the separately
rejecting source classification, or pass the full gate.

## Empty owned-unsafe scope

Each condition below is a separate comparator diagnostic, but they are not equal evidence:

- the workspace manifest forbids `unsafe_code`. With the `[lints] workspace = true` opt-in in all five member manifests and a successful compiler/Cargo run under that lint, this is the scoped owned-unsafe basis. The derivation checks only the workspace line; the member opt-ins were checked against the sealed manifests;
- the compiler reports no foreign item. This is real compiler evidence;
- the compiler reports no function-safety fact. This is vacuous: the current architecture policy tracks no value-flow types, so no such fact can be emitted;
- no owned unsafe site exists. Sites derive only from function-safety facts, so this is vacuous too.

An empty scope therefore rests on the forbid lint, the member opt-ins and the successful build, not on inventory observation of unsafe code. A reviewed unsafe site that the compiler does not observe is still rejected.

## Miri controls

Pinned required configuration: toolchain `nightly-2026-08-18`, target `x86_64-unknown-linux-gnu`, flags `-Zmiri-strict-provenance`, subject `crates/noble-kernel/Cargo.toml`, required test `budget`.

| Control | Result |
|---|---|
| Positive baseline | `cargo miri setup` passed; `cargo miri test` reported 2 passed, 0 failed |
| Negative: strict-provenance violation | A fixture crate performing an integer-to-pointer read fails: `integer-to-pointer casts and ptr::with_exposed_provenance are not supported with -Zmiri-strict-provenance` |
| Retained negative hypothesis | `--target wasm32-unknown-unknown` was **accepted**: Miri prepared a wasm32 sysroot and the tests passed. It is not an unsupported-target control |

Unsupported required configurations are rejected by the reviewed comparator (`miri-unsupported-configuration`, `miri-configuration-mismatch`), not by assuming Miri refuses a target.

## Comparator controls

Twenty-six synthetic self-tests cover scope packages, target and feature drift, missing unsafe-forbid evidence, missing accounting basis, observed unsafe or foreign facts, unreviewed and absent unsafe sites, unclassified dependencies, missing fixture records, unclassified tools and foreign libraries, missing safety arguments, missing target assumptions, and each Miri requirement.
The function-safety and unsafe-site controls inject synthetic facts that the current collector configuration cannot emit, and no control uses a non-empty reviewed dependency list.

## Limits

Dependency records are not dependency safety. Interpreter results are not refinement. The inventory covers the named scope only.
