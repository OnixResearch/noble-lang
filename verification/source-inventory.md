# Compiler-derived source inventory

Tasks 2.3 and 2.4 established the inventory mechanism. The `source-coverage` check collects the compiler-derived inventory and compares it with a reviewed classification.

The current checked-in classification is the **M6 native-async source renewal dated
2026-09-24**, covering all 25 expected compiler units.
[The current JSON export](../policy/source-inventory.json) records the renewed
conservative accounting. The complete, unchanged 73-rule deny-all run retained
in the [roundtrip-verified M6 quality archive](m6/quality-archive.json) has zero
warnings, errors and architecture findings; the independently checked
`source-coverage` Nix output is valid. These are separate current-source
observations, not a universal refinement or runtime acceptance receipt.
Classification, quality, independent comparison and milestone acceptance are
separate observations. Later source changes require fresh bound receipts, not
reuse of this pass. Historical M4, MC2 and M5 evidence retains its original scope,
counts and bindings.

This is a named-scope accounting control, not milestone acceptance, an architecture-clean receipt, or a refinement proof.

## Commands

The existing collection and comparison interfaces are:

```sh
source-inventory collect --root DIR --selection FILE --artifact-dir DIR
source-inventory check --root DIR --selection FILE --inventory FILE --policy FILE --artifact-dir DIR
```

The Nix source-coverage check uses those collection/comparison interfaces:

```sh
nix --option min-free 0 --option build-dir /nix/var/nix/builds --builders '' \
  build .#checks.x86_64-linux.source-coverage -L --out-link "$EVIDENCE/source-coverage"
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

The policy is not an isolated historical M1 fixture. `flake.nix` still compares it with the current workspace compiler inventory and current architecture scope. Its authoritative scope includes `noble-kernel`, `noble-contracts`, `noble-wasm`, and `noble-cli`; the CLI executable's compiler namespace is `noble`. Independent Wasmtime peers are separate verification-only workspaces. Owned JS/WAT/runtime assets, the canonical ABI, host provenance, native release and external tools require separately bound evidence and trust accounting, not Rust extraction coverage.

## Classification

Each compiler-derived production subject is reviewed as one of three categories.

| Category | Meaning | Disposition |
|---|---|---|
| `body` | A Noble-authored function, closure carrying authored source-body semantics, or named constant initializer observed in a production-role unit | `extracted`, `modeled`, `excepted`, or `open` |
| `generated` | A compiler-generated derive or harness item with no independent Noble body obligation | `generated` |
| `structural` | A declaration, implementation block, module, macro, type, or import marker; associated bodies are accounted separately | `structural` |

Bodies additionally carry a refinement status: `proved` or `open`.
Only bodies enter the extraction, modeling, exception, proof, and open counts.

The complete 25-unit reviewed observation contains 1,939 authored-body paths,
1,707 generated paths and 837 structural paths: 4,483 unique production paths
covering 4,573 compiler
item facts. A path shared by an authored body and generated items retains the
body obligation. The difference between item and path counts also includes
compiler observations of different item kinds under one qualified path.

This renewal restored 195 retained authored closures from `generated` classification
to `body` / `open` / `open`, removed 18 stale source-offset closure entries, and
added 19 new closure entries. Anonymous syntax or a generated wrapper does not
erase authored semantics. The correction restores explicit obligations; it does
not extract, model or prove those bodies, and removing a stale offset is not a
discharge of its semantic obligation.

All 1,939 body dispositions and refinements remain `open` in this inventory.
It does not import or discharge the separately bound kernel, MC1, Wasm-emitter
or M4/MC2/M5 frontend/compiler/companion/resource evidence, or the bounded M6
async extraction and proof observations. Zero bodies here are
classified as extracted, modeled, excepted, or proved; that accounting limit is
not a claim that the separate proof lanes have no evidence.

Tests, macro origins, dependencies, tools, and future components are classified separately.
Test-labelled subjects keep the test role and cannot discharge a production obligation.
Only observations exclusively owned by test-role units enter the separate test set. The collector retains production roles for library and binary test configurations; a test body in such a unit cannot obtain an exemption merely through its name. Generated-item accounting likewise does not exclude effects or required unknowns from architecture evaluation, and authored closures retain open body obligations.

### Historical observations

The dated 2026-09-15 M1 review at tree `3abd178bda9543155c545fdacf6b07f3fd5bbc49` contained 6 bodies, 4 generated paths, and 6 structural paths across 17 compiler items. Those historical counts do not describe the current expected subject set.

The historical M5 synchronous-component renewal dated 2026-09-24 contained
1,620 bodies, 1,635 generated paths and 731 structural paths: 3,986 production
paths across 4,069 compiler item facts in 23 units. Its separate test set contained
948 paths across 1,173 item facts. Its compiler IR was
`cbcae3e9ec7012839f3a1738f7192315f00e35df8eea7586bc8da860a9b71784`,
coverage identity was
`a4b1f589e2a6b5eabdc9a37d3fa97e7ed546674b5a04066eefad396621166113`,
and architecture receipt was
`a6812cfe862898b1e5c93914756261a42b55b2a7c36c3acd67a8b334efb1333d`.
These remain historical M5 counts and bindings, not the current M6 expectation.

## Compiler review observation

The M6 review is bound by the compiler IR, coverage, collector receipt/policy,
toolchain, Cargo graph and per-crate source identities recorded in
[the reviewed policy](../policy/source-inventory.ncl). Its JSON export and the
complete compiler packet retained at `quality.tar.xz.part-*:workspace-octet`
and `quality.tar.xz.part-*:source-inventory/review` are the
machine-comparable authorities for this observation. The compiler IR is
`cb97f0413e10fa95022fc07e4d84b0e17c016f8a08288021417ee54cd7180c7e`,
coverage identity is
`6736e9d9d79601eb4d00e9f6078cc4fbf6f522850641a4f2fb0ac19f1467a6a4`,
collector receipt is
`ba600b581adb46e817e3223d87c2b511aa811fa999cbe06ab630c11d78ea4362`,
and architecture receipt is
`764ac9260e832554be4b2958694ef5f3123ffcfb1c7beea64f54dc7ba178e2e7`.
These identities bind the reviewed observation, not an arbitrary later source
tree. The [production Nix source and outputs](m6/nix-checks.json) and the
independently [verified quality archive](m6/quality-archive.json) retain the
passing comparison and its raw compiler observations. Historical M3/M4/MC2/M5
identifiers are not the current M6 binding, and an archive location alone is
not a passing receipt.

Coverage reports all 25 expected units: six domain-core units (three library and
three library-test configurations), two composition-root units (binary and
binary-test), and seventeen test-role units. The single declared configuration is
`x86_64-unknown-linux-gnu`, `features=[]`, `default_features=true`.
There are 1,044 unique exclusively test-role paths across 1,302 test item facts,
26 macro origins, six production dependency edges, no external Cargo edges,
and thirteen selected tools. Both future components remain `open`, and the
exception set is empty. The three domain-core crates retain their `no_std`
policy obligations.

Complete collection alone is not a passing architecture gate. The earlier interim run's required production desugaring unknowns were not waived; this observation was collected after those source corrections. Listing `<builtin>` in the macro-origin set does not waive any required unknown, and the full compiler IR can retain test-only unknown facts without granting them production authority. Gate mode, core capability prohibitions, and empty waiver/exception sets remain unchanged.

The default catalog was insufficient for acceptance. The published deny-all hook enables 73 rules, including off-by-default rules. The current source resolves their findings through legal const helpers, closed-enum declarations, bounded module separation, explicit units, and narrowly scoped explanations where heuristic requirements conflict with fallible validation or guarded arithmetic bounds. No assertion padding, new opaque extraction boundary, policy waiver, or catalog weakening was introduced.

The architecture policy names the exact observed CLI effect owners, including
compiler-qualified closure identities and filesystem observations through
`std::path`. Its manifest uses the selected Nickel and `octet-standards` tools.
The inherited source comparison, policy/tool-selection freshness checks, and
strict Octet gate remain separate acceptance controls; this document is not
their execution receipt. Closure identities include source byte offsets, so even
comment or help-text edits can require renewal. All 25 expected workspace units
remain mandatory. An acyclic boundary control must additionally retain both
units of its synthetic shell library; collection capacity is not permission to
omit real or synthetic test targets.
The collector's configuration-entry capacity is 2,048; this does not enlarge
the single declared target/feature configuration or waive incomplete collection.

## Extraction and native-async evidence boundary

The actual-source lane separately extracts the whole kernel, frontend and
compiler. The backend imports actual frontend types before its own generated
types; separate audit entrypoints include their transitive project dependencies.
Its reviewed extraction lock and implementation
receipt are the authorities for exact functions, external models, dependencies
and axioms. The shared current lock is `verification/m4/extraction-lock.json`;
historical M4/MC2 receipts retain their original scopes, and each renewal requires
a fresh independent check. M5's resource-transition theorems and component
dependency coverage remain separate from this classification.

M6 adds the async-task kernel, WIT/component frontend and async component
lowering/emission scopes. The [independent fresh
check](../verification/m6/extraction.json) passed against the renewed
[reviewed lock](../verification/m4/extraction-lock.json): 526 source files,
fourteen strict actual-Rust roots for transition, table-decision, classifier
and record-validation correspondence plus qualified lifecycle properties,
all 75 state/event constructor pairs, and 176 M6 refusal controls (228 total
across inherited lanes). The [formal archives](../verification/m6/formal-archives.json)
retain the discovery and check products with full roundtrip integrity.
Native execution, constructor coverage, extracted-body coverage and
refinement remain distinct; none establishes unmodeled host or native runtime
semantics.

The [corrected full MC2 regression](m6/regressions.json) and all thirteen
[declared Nix checks](m6/nix-checks.json) pass. Neither native execution nor
independently checked extraction closes
the inherited proof obligations or turns the 1,939 open authored-body
obligations into universal refinement.

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
