# Compiler-derived source inventory

Tasks 2.3 and 2.4 established the inventory mechanism. The `source-coverage` check collects the compiler-derived inventory and compares it with a reviewed classification.

The checked-in classification is the **M3 Wasm-feasibility source renewal dated 2026-09-20**, retained under `verification/m3-wasm/assurance.tar.gz:octet`. That run passed the published 73-rule deny-all hook and the architecture gate with zero findings. The fresh compiler inventory also passed the independent source-coverage comparator. These are separate observations; classification itself is not a quality or proof receipt.

This is a named-scope accounting control, not milestone acceptance, an architecture-clean receipt, or a refinement proof.

## Commands

The existing collection and comparison interfaces are:

```sh
source-inventory collect --root DIR --selection FILE --artifact-dir DIR
source-inventory check --root DIR --selection FILE --inventory FILE --policy FILE --artifact-dir DIR
```

The check runs both:

```sh
nix --option min-free 0 --option build-dir /nix/var/nix/builds --builders '' \
  build .#checks.x86_64-linux.source-coverage -L --out-link "$EVIDENCE/source-coverage"
nix run .#source-inventory -- collect …
nix run .#source-inventory -- check …
```

## Ownership and mechanisms

[The reviewed policy](../policy/source-inventory.ncl) owns the expected classification.
Its JSON export is checked, not refreshed, by the `policy` check.
`nix/source-inventory-derive.nix` derives the inventory from collector artifacts and performs no I/O.
`nix/source-inventory.nix` is the pure comparator over reviewed expectations and compiler-derived data.
`nix/source-inventory-app.nix` is the thin execution and file adapter.
Octet owns the compiler facts, coverage report, and Cargo graph.

The inventory comes from `cargo-octet check` on the real workspace with all targets and features.
Nothing in the inventory is hand-written: units, subjects, macro origins, dependency edges, and the compiler coverage report are compiler-derived.
The reviewed classifications are project-owned policy, not compiler certifications.

The policy is not an isolated historical M1 fixture. `flake.nix` still compares it with the current workspace compiler inventory and current architecture scope. Its authoritative scope includes `noble-kernel`, `noble-contracts`, `noble-wasm`, and `noble-cli`; the CLI executable's compiler namespace is `noble`. The eight owned WAT assets have separately bound runtime evidence, not Rust extraction coverage.

## Classification

Each compiler-derived production subject is reviewed as one of three categories.

| Category | Meaning | Disposition |
|---|---|---|
| `body` | A Noble-authored function or named constant initializer observed in a production-role unit | `extracted`, `modeled`, `excepted`, or `open` |
| `generated` | A compiler-generated derive, harness, or nested closure item with no independent Noble body obligation | `generated` |
| `structural` | A declaration, implementation block, module, macro, type, or import marker; associated bodies are accounted separately | `structural` |

Bodies additionally carry a refinement status: `proved` or `open`.
Only bodies enter the extraction, modeling, exception, proof, and open counts.

The reviewed observation contains 549 bodies, 512 generated paths, and 243 structural paths: 1,304 unique production paths covering 1,337 compiler item facts. A path shared by an authored body and generated items retains the body obligation. The difference between item and path counts also includes compiler observations of different item kinds under one qualified path.

All 549 body dispositions and refinements remain `open` in this inventory. It does not import or discharge the separately bound kernel, MC1, or Wasm-emitter extraction and proof evidence. Zero bodies here are classified as extracted, modeled, excepted, or proved; that accounting limit is not a claim that the separate proof lanes have no evidence.

Tests, macro origins, dependencies, tools, and future components are classified separately.
Test-labelled subjects keep the test role and cannot discharge a production obligation.
Only observations exclusively owned by test-role units enter the separate test set. The collector retains production roles for library and binary test configurations; a test body in such a unit cannot obtain an exemption merely through its name. Generated closure accounting likewise does not exclude its effects or required unknowns from architecture evaluation.

The dated 2026-09-15 M1 review at tree `3abd178bda9543155c545fdacf6b07f3fd5bbc49` contained 6 bodies, 4 generated paths, and 6 structural paths across 17 compiler items. Those historical counts do not describe the current expected subject set.

## Compiler review observation

The review uses `verification/m3-wasm/assurance.tar.gz:octet` from 2026-09-20. These identifiers bind the observed source set and reviewed architecture policy, not a later source tree:

| Binding | BLAKE3 identity |
|---|---|
| Compiler IR | `55077abfce561242e5c95bff8a31f8332fe7c7ab463058037ed60e433d5933c5` |
| Compiler coverage | `5d28fa4b4378bfe765c930cd88583776724b30cf3d5515e7597dec44cc6d67f4` |
| Collector policy | `dd92d69b6ac0862d5c4834f59f769d09c66c3db74466d394fe9f52bc3051ce46` |
| Cargo graph | `34112df1b861268fc8c0162e49a8d9e420447ced87d0d597e99ae0fe9d3498da` |
| `noble-cli` source | `0641088804e6318613f8f02547f166bb8b3161569c978fdc70352c27b325ed4c` |
| `noble-contracts` source | `3211e4161f66d171cbe6b46de3b1f31d93e98e039b8be1b6475bca028ff162b5` |
| `noble-kernel` source | `1b7efd02dd9345d8d0a428ba1d2659f3b3a7c20dcc7a7e5410d0e51f55f94960` |
| `noble-wasm` source | `fd74372954d1c6b0cd2e8614085d58af1cf4573e66ace96e09f5b3c169f54801` |

Coverage reports all 17 expected units for one target/feature configuration. There are 666 unique exclusively test-role paths across 772 test item facts, 24 macro origins, five production dependency edges, no external Cargo edges, and thirteen selected tools. The three domain-core crates retain their `no_std` policy obligations.

Complete collection alone is not a passing architecture gate. The earlier interim run's required production desugaring unknowns were not waived; this observation was collected after those source corrections. Listing `<builtin>` in the macro-origin set does not waive any required unknown, and the full compiler IR can retain test-only unknown facts without granting them production authority. Gate mode, core capability prohibitions, and empty waiver/exception sets remain unchanged.

The default catalog was insufficient for acceptance. The published deny-all hook enables 73 rules, including off-by-default rules. The accepted source resolves their findings through legal const helpers, closed-enum declarations, bounded module separation, explicit units, and narrowly scoped explanations where heuristic requirements conflict with fallible validation or guarded arithmetic bounds. No assertion padding, new opaque extraction boundary, policy waiver, or catalog weakening was introduced.

The architecture policy names the exact observed CLI effect owners, including compiler-qualified closure identities and filesystem observations through `std::path`. Its manifest was generated with the selected Nickel and `octet-standards` tools before this collector run. The inherited source comparison, policy/tool-selection freshness checks, and strict Octet gate remain separate acceptance controls; this document is not their execution receipt. Closure identities include source byte offsets, so even comment or help-text edits can require renewal. The collection limit is now 24 units/shards to include the new crate's library/test units; all 17 expected units are still mandatory.

## Rejection controls

The comparator rejects, with a named diagnostic:

- incomplete compiler coverage, missing, unexpected, or duplicated units;
- a changed required target, feature set, or default-feature state;
- an undeclared production package or source scope;
- an omitted or stale production subject;
- a subject bound to the wrong package;
- an invalid disposition or refinement status;
- an unclassified or stale test subject, macro origin, dependency edge, or tool;
- an excepted body without a record, an incomplete record, or any kernel exception;
- a future component claimed as verified, or listed as a production subject.

Twenty-five synthetic self-tests cover these rules, including separate target, feature, and default-feature mutations.
They are harness self-tests, not compiler evidence.

## Limits

The reviewed policy is a static expectation. The build never refreshes it.
Renewal is an explicit reviewed action bound to the compiler-derived set at review time.
The inventory covers the selected target and feature configuration only.
Structural and generated categories are accounting, not independent verified bodies.
No entry establishes refinement, native safety, dependency safety, or whole-project verification.
