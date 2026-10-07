# Compiler-derived source inventory

Tasks 2.3 and 2.4 established the inventory mechanism. The `source-coverage` check collects the compiler-derived inventory and compares it with a reviewed classification.

The classification retains the **bounded DXM1 declared-module renewal dated
2026-09-28**, bound to its historical 32-unit observation, plus a narrow
candidate file review described below. The earlier observation and its pass
do not rebind to the additional candidate paths.
[Its JSON export](../policy/source-inventory.json) records conservative
accounting; the [DXM1 assurance](declared-modules-v1/assurance.json)
passed `source-coverage`, boundary/deny-all, policy and quality checks
for that earlier source, not the current isolated selection candidate. The
previous [M6 quality archive](m6/quality-archive.json) retains its separately
bound historical observation. Neither quality gate nor compiler
classification is universal refinement or runtime acceptance.
Classification, quality, independent comparison and milestone acceptance are
separate observations. Later source changes require fresh bound receipts, not
reuse of this pass. Historical M4, MC2, M5, M6 and M7 evidence retains its original
scope, counts and bindings.

The historical collector ceiling was 34 units/shards: 32 previously expected
workspace units plus two acyclic-control units. An earlier `570ed67` source
candidate used a 41-unit/shard ceiling for 39 units and two controls. The
isolated selection candidate's pinned offline Cargo plan selects 54 workspace
units; its capacity is exactly 54 units/shards, with no spare allowance.
Historical acyclic controls measured 56 on another source, but the selected
candidate fixture currently duplicates `[dev-dependencies]`, so it cannot
justify a 56-unit claim here. A prior complete 54-unit collection on a
different, non-ancestor source measured 18,075 spans and 16,646 calls in
the CLI bin+test shard with a 32,768 scratch cap. That is not a candidate
measurement. On the separately sealed candidate source, the CLI bin+test
shard actually reached its 16,384-span cap with 15,421 call facts and at
least 541 distinct referenced spans missing. The collector completed Cargo
but rejected this partial shard: 53 of 54 selected units had valid shards,
1,110 references were invalid, and **no source inventory was produced**.
The raw partial artifact and invocation are retained under
`/home/brittonr/.cache/noble-candidate-source-inventory-resolved-plan-corrected-20261007/artifacts/`.
For the subsequent bounded candidate re-collection, only `max_spans` was
raised to 32,768; calls and constructors remained at 16,384. Expansions
remained bounded at 8,192, a shard at 32 MiB and one canonical IR artifact
at 128 MiB. This cap change enabled the complete 54/54-unit compiler
architecture observation described below. The earlier 53/54 partial run
remains separate; complete collection does not imply architecture, source
or native acceptance. Octet's 1,535 compiler lint findings were warning-only;
the preserved overall check exited 1 because architecture was blocked by 1,501
policy findings (1,494 required compiler unknowns and 7 forbidden role edges).
The later composition-root provider classification (for seven observed
CLI edges) does not alter this receipt or discharge any required compiler
unknown; it does not enforce an exclusive `noble-cli` caller.

The selected patched Octet executable initially collected 39/39 complete units
on unchanged `570ed67` source using **disposable scratch-only** limits of 48
units/shards. Renewed actual selected-app runs under the staged 41-unit
candidate policy collected 39/39 complete workspace units and 41/41 complete
acyclic-positive units/shards, with zero collection issues in either run;
their fresh 12-artifact bundles replayed valid with zero diagnostics. Both
checks exited 1: the normal run had 712 compiler unknowns, two forbidden
role edges involving `anyhow`/`wasmtime`, and a canonical `Rules.lean` warning;
the acyclic positive had 385 compiler unknowns. After the selected Octet
source's diagnostic-path provenance correction, fresh COW runs against the same
staged 41-unit policy again collected complete 39/39 and 41/41 units and
shards with zero collection issues. Both exited 1 with the same red finding
counts, and the normal run retained the original `Rules.lean` warning twice.
Both corrected 12-artifact bundles replayed valid without diagnostics; replay
does not change their blocked architecture dispositions.
Direct pure comparison of the corrected normal IR with the reviewed
source and native policies still rejects source subject omission/staleness,
test and macro classification, dependency closure and the two external
production dependencies. The corrected acyclic reverse-dependency fixture
also collected 41/41 units and shards, then added exactly one forbidden
kernel-to-CLI role edge above its 385 compiler unknowns; its bundle replayed
valid. Neither result revises the checked-in source classification.
The reviewed source comparator rejected the candidate: 7,162 unique
production paths against 5,816 reviewed, including 1,437 new and 91 stale
paths; test subjects, macro origins and dependency closure also need explicit
review. Complete collection is not an architecture, source or native
approval. Do not copy the 32-unit source dispositions onto new paths or infer
acceptance from complete collection. Current `source-coverage` is not a PASS.

This is a named-scope accounting control, not milestone acceptance, an architecture-clean receipt, or a refinement proof.

## Partial 54-unit candidate file review (2026-10-07)

The unchanged raw compiler IR from the isolated candidate's complete
54/54-unit collection had zero collection issues. Octet reported 1,535
warning-only lint findings, while its blocked architecture phase had
1,501 policy findings (1,494 required compiler unknowns and 7 forbidden
role edges), making the overall check exit 1. That complete compiler
observation yielded 8,421 production item facts / 8,248 distinct paths;
it is an inventory input, not an architecture/source/native acceptance.
Against the historical reviewed set, 2,537 observed paths were
unreviewed across 108 source files, while **105 previously reviewed paths
were stale**. The two debts are independent. The complete
package/file/compiler-item-kind triage and the separate stale-path/category
list are retained in
`/home/brittonr/.cache/noble-candidate-source-inventory-review-20261007/triage.json`;
the input IR remains in
`/home/brittonr/.cache/noble-candidate-source-inventory-span-cap-20261007/artifacts/collector/compiler-architecture-ir.json`.
Compiler item kinds are *not* proposed `body` / `generated` / `structural`
classifications. Unreviewed package counts are 99 `noble-kernel`, 1,040
`noble-contracts`, 1,179 `noble-cli`, and 219 `noble-wasm`; the compiler
kinds are 1,370 `other`, 689 `function`, 239 `implementation`, 86 `struct`,
67 `constant`, 40 `enum`, 38 `module`, 5 `type-alias`, and 3 `static`.
The stale set is separate: `noble-cli` 54 body; `noble-contracts` 41 body
and 1 generated; `noble-kernel` 6 body and 1 structural; `noble-wasm`
2 body.

Only `crates/noble-cli/src/core/live_slot_origin.rs` was classified in this
step. Its 123 observed item facts resolve to **120 exact production paths,
zero test-only paths**: 101 body paths retain `open` extraction and refinement
obligations; 18 declarations/imports are structural; and one
compiler-generated test-harness item has no independent body obligation.
Two authored `#[test]` function paths also have
compiler-generated registration constants; the shared paths retain their
authored body obligations. Their separately classified compiler-generated
wrapper closures retain inherited open obligations, but are not substitutes
for the function bodies. `#[cfg(test)]` did not exempt these items: the
observed bin+test unit has a production role. No other file or
`test_subjects` row was added.

With this one file reviewed, the pure comparator over the *same* derived
inventory reports 5,936 reviewed production paths, 2,417 still omitted
(down by exactly 120), and the same 105 stale paths. Its result at
`/home/brittonr/.cache/noble-candidate-source-inventory-review-20261007/source-coverage-corrected.json`
preceded the dependency-edge review below. That historical result remained
`valid=false`: `subject-omission`, `subject-stale`,
`test-subject-classification`, `macro-classification`,
`dependency-closure`, and `reviewed-scope-partition` were present. This is
partial classification, not source-coverage acceptance, architecture
clearance, extraction, proof, or a fresh compiler collection.

## Candidate dependency-edge review (2026-10-07)

The unchanged complete 54/54-unit inventory's Cargo graph observes fourteen
production edges: the seven previously reviewed workspace-internal edges and
exactly seven direct external **normal** edges from `noble-cli` to `anyhow`,
`blake3`, `libc`, `serde_json`, `wasmparser`, `wasmtime`, and `wat`. The
candidate's `Cargo.lock` lists these seven as CLI dependencies; the reviewed
policy now names precisely these caller/callee/kind tuples. The historical
seven-edge DXM1 and M6–M8 receipts remain bound to their earlier sources;
the candidate's fourteen-edge accounting does not rebind those receipts.
The raw inventory SHA-256 is
`53fcdbc15aa0d7a865f8f41cde84437af7666a21bac623c9f54adb36d153d801`;
its source compiler IR SHA-256 is
`cd6808f09f8f50b2bc07390cb1cbb3397124bc0cfadd4430d19b4cf32e791f20`.

Pure comparison of this policy against the unchanged complete candidate
inventory removes only `dependency-closure` from the earlier six diagnostics.
It remains `valid=false` with 2,417 omitted production paths and 105 stale
reviewed paths, plus `test-subject-classification`, `macro-classification`,
and `reviewed-scope-partition`. These external edges are accounted for, not
approved as safe dependencies or proof of architecture/native acceptance.
Changing one reviewed edge kind from `normal` to `build` restores precisely
`dependency-closure` alongside the five remaining diagnostics.
The actual app gates the source and native comparators behind the blocked
Octet architecture phase; this pure result is no full-gate pass or fresh
compiler collection.

## Candidate macro-origin review (2026-10-07)

The same unchanged 54/54-unit compiler inventory observes **62 distinct
macro origins**, against 32 historical reviewed names. The 30 new names
below were checked against their IR expansion callsite spans and the named
definitions/uses in the candidate source or pinned tool/dependency source.
They are individually added to the reviewed name set; none is a macro
execution, effect, safety, extraction, or proof approval. In particular this
does not excuse any of Octet's 1,494 required compiler unknowns.

| Newly reviewed origin | Definition/callsite and remaining boundary |
|---|---|
| `alloc::format` | Selected Rust `alloc/src/macros.rs`: formats via `fmt::format`; observed in `noble-contracts` intrinsic, named-v2, program, signatures and editor source; allocation and rendered-text semantics remain open. |
| `anyhow::__anyhow` | Pinned anyhow 1.0.100 `src/macros.rs`: hidden error construction used by `bail!`/`ensure!` in CLI component hosts; not a host authorization witness. |
| `anyhow::__fallback_ensure` | Pinned anyhow `src/ensure.rs`: conditional early `Err` (possibly formatting); observed in CLI authorized-fs, callback-owner and quota hosts; does not prove their guards. |
| `anyhow::anyhow` | Pinned anyhow `src/macros.rs`: creates an error from format text or an error expression; observed in CLI component hosts, including invocation-identity failures; backtrace/environment assumptions remain external. |
| `anyhow::bail` | Pinned anyhow `src/macros.rs`: returns `Err(__anyhow!(…))`; observed in CLI component hosts; a refusal message is not independent admission evidence. |
| `core::const_format_args` | Selected Rust `core/src/macros/mod.rs` compiler built-in; IR sites are test assertions in `noble-contracts` source/parsing/signatures. |
| `core::format_args` | Selected Rust compiler formatting built-in; observed with `alloc::format` in `noble-contracts` intrinsic/proof text and related source files; formatting semantics remain external. |
| `core::panic::panic_2021` | Selected Rust panic expansion; IR sites are `noble-contracts` test assertions/panic branches, not a waiver for failure or a proof of no panic. |
| `core::prelude::v1::test` | Compiler test-harness attribute at `noble-contracts` and `noble-wasm` `#[test]` sites; test-labelled source in production-role test configurations still retains its source obligations. |
| `core::write` | Selected Rust `core/src/macros/mod.rs` dispatches `write_fmt`; observed at `noble-contracts/src/intrinsic.rs` bounded Lean-text writer, not a proof of formatting or allocation bounds. |
| `intrinsic::advance` | `noble-contracts/src/intrinsic.rs`: returns updated meter and propagates failure; observed in the bounded intrinsic interpreter. |
| `intrinsic::lean_text::append` | Same file: checked byte cap before `String::push_str` in Lean-text assembly. |
| `intrinsic::lean_text::push` | Same file: checked traversal-stack frame insertion in Lean-text assembly. |
| `intrinsic::parsed::{closure#0}::push_frame` | Same file: local parsed-form closure's bounded frame insertion. |
| `intrinsic::parsed::{closure#0}::push_value` | Same file: local parsed-form closure's bounded rewritten-value insertion. |
| `intrinsic::proof_walk::{closure#0}::finish_apply` | Same file: charges proof work/size and constructs a substituted proposition and rendered text; not a proof of its resulting term. |
| `intrinsic::proof_walk::{closure#0}::schedule` | Same file: local proof-walk closure's bounded frame insertion. |
| `intrinsic::proof_walk::{closure#0}::take_checked` | Same file: local proof-walk closure accepts only a checked completed value. |
| `intrinsic::proof_walk::{closure#0}::take_inferred` | Same file: local proof-walk closure accepts only an inferred completed value. |
| `intrinsic::push_rewrite_frame` | Same file: delegates bounded rewrite-frame insertion to `work_push!`. |
| `intrinsic::push_rewritten` | Same file: delegates bounded rewritten-value insertion to `work_push!`. |
| `intrinsic::take_code` | Same file: pops only the expected rewritten code variant or refuses. |
| `intrinsic::take_prop` | Same file: pops only the expected rewritten proposition variant or refuses. |
| `intrinsic::take_sort` | Same file: pops only the expected rewritten sort variant or refuses. |
| `intrinsic::work_pop` | Same file: local inline/spill stack pop and count adjustment; authored control flow remains an open source-body obligation. |
| `intrinsic::work_push` | Same file: checks stack count/limit before inline or spill insertion; authored control flow remains an open source-body obligation. |
| `serde_json::json_internal` | Pinned serde_json 1.0.151 `src/macros.rs`: hidden `json!` expansion for CLI live reports, slot/origin records and test fixtures. It builds allocated values and the generic Serialize arm calls `to_value(…).unwrap()`; name accounting does not prove a panic-free or trustworthy report. |
| `std::default::Default` | Compiler derive observed on CLI live-slot `ControlState`; generated initializer accounting does not validate live-slot state transitions. |
| `std::writeln` | Selected Rust `core/src/macros/mod.rs` newline formatting; observed on CLI live-slot output pipe, whose write/flush failure is handled separately. |
| `words::subst::finish::attempt_segment` | `noble-kernel/src/words/subst/finish.rs`: locally checks a substituted segment and returns the owned walk on error; kernel source behavior remains open. |

The external crate archives were matched to this candidate's `Cargo.lock`
checksums: anyhow 1.0.100
`a23eb6b1614318a8071c9b2521f36b424b2c83db5eb3a0fead4a6c0809af6e61`
and serde_json 1.0.151
`c841b55ecdae098c80dcae9cf767f6f8a0c2cdb3416bbef72181df4d0fe73f14`.
The selected nightly Rust source identifies the `alloc` and `core` formatting
macros; compiler built-ins/derives and test harness entries remain compiler
trust boundaries. Pure comparison now removes **only** `macro-classification`
from the previous five diagnostics: `valid=false` with `subject-omission`,
`subject-stale`, `test-subject-classification`, and
`reviewed-scope-partition` still present (2,417 missing and 105 stale
production paths). Deleting `serde_json::json_internal` or adding an
unobserved origin to policy restores `macro-classification`; these are
classification refusals, not architecture or native gate results.

## Candidate test-only file review (2026-10-07)

The unchanged complete inventory has 2,872 test-subject item rows resolving
to 2,279 distinct qualified paths. Against the historical 1,333 reviewed
test paths, 1,061 observed names were unreviewed and 115 older names were
stale. Package-level counts cannot simply be added: 20 `editor`/`named_v2`
qualified names occur in both CLI and contracts test crates. This review
instead selects the complete, independently named
`crates/noble-wasm/tests/checked_u64.rs` integration-test file.

Its one `test+test` unit has the **test** role, and its 12 compiler item rows
resolve to exactly 11 previously unreviewed test-only paths. No production
item in this file or production path sharing these names was observed. The
reviewed names are `checked_u64::LIMITS`,
`checked_u64::checked_unsigned_export_requires_its_exact_world_recipe`,
`checked_u64::main`, `checked_u64::main::test`, `checked_u64::std`,
`checked_u64::test`, `checked_u64::{use#0}`, and four
`noble-wasm::closure@crates/noble-wasm/tests/checked_u64.rs` paths at byte
ranges `152:1165`, `447:454`, `560:567`, and `962:969`.
`LIMITS` is an authored test constant. The authored `#[test]` function
checks exact checked-u64 world/recipe correspondence; its generated test
registration constant shares the function's qualified path, accounting for
the twelfth item row rather than a twelfth policy name. The generated harness
wrapper closure does not replace the authored test body. This **test-only**
classification is neither a production exemption nor a proof of the test's
claims: a future production-role observation of the same path would still
require its separate `body` obligation.

The reviewed test-name set is now 1,344 paths; 1,050 observed test paths
remain unreviewed and 115 previously reviewed paths remain stale.
`test-subject-classification` therefore stays red. Pure comparison of the
unchanged inventory remains `valid=false` with precisely
`subject-omission`, `subject-stale`, `test-subject-classification`, and
`reviewed-scope-partition`; the 2,417 missing and 105 stale **production**
paths are untouched. The independent synthetic controls also reject a stale
reviewed test path and a production-role body whose path was already listed
in the test set. The actual app remains blocked before source comparison by
Octet architecture; no collector, source-coverage or runtime PASS is claimed.

## Candidate logical-parser complete-file review (2026-10-07)

With the same sealed 54-unit compiler inventory, the entire
`crates/noble-contracts/src/source/declared/parsing/logic.rs` file contributes
13 observed production item facts resolving to **12 previously unreviewed
unique paths** in package `noble-contracts`. The `parse` function, its import,
and its two compiler-observed authored closures occur in both `lib` and
`lib+test` units. The `#[cfg(test)]` module occurs in the `lib+test` unit,
which has the **domain-core production role**, not the test-only role.
All 12 paths are classified: five `body/open` (parser, two authored
closures, authored test function and inherited test-wrapper closure), six
structural (the parser import, test module and four test imports), and one
generated test-harness item. The authored closure spans `1637:1640` and
`2340:2342` correspond to the `map_err(|_| …)` and
`ok_or_else(|| …)` expressions, respectively. The final
`frames.last().map_or(span, |frame| frame.0)` is authored syntax inside
`parse`; the test's `(0..65).map(|_| LogicalToken { … })` closure at bytes
`4356:4359` is likewise authored syntax inside
`nested_form_preserves_order_and_rejects_excess_depth`. The raw IR has no
separate item fact/path for either closure: both inherit their containing
authored bodies' open obligations, and no synthetic path is added.

The authored `nested_form_preserves_order_and_rejects_excess_depth`
function and generated `#[test]` registration constant share one path:
the authored body keeps its open extraction/refinement obligation.
The generated test wrapper at `3079:4588` inherits that obligation despite
its compiler-expansion provenance. Only `tests::test` is a pure generated
harness path. No derive-generated item or stale reviewed path belongs to
this file; **no stale path was deleted**. Compiler `item_kind=other` did
not determine any of these dispositions.

The pure comparator over the unchanged raw inventory and updated reviewed
policy reports 5,948 reviewed production paths (2,701 body/open,
2,133 generated, 1,114 structural), with **2,405 omitted** (down exactly
12) and **105 stale** reviewed production paths (unchanged). The independent
test-only debt remains 1,050 omitted / 115 stale. The report is retained at
`/home/brittonr/.cache/noble-candidate-source-inventory-review-20261007/source-coverage-logic-file.json`:
`valid=false` with exactly `subject-omission`, `subject-stale`,
`test-subject-classification`, and `reviewed-scope-partition`. The
`source-inventory-controls` flake check evaluated all 38 synthetic controls;
that check is not source, architecture, native-assurance, or refinement
acceptance. No collector or full gate was rerun.

## Candidate source-origin body review (2026-10-07)

The unchanged complete 54-unit compiler inventory contributes 62 previously
unreviewed production paths across three complete selected Rust files; each
item was checked against its source declaration or exact closure byte span.
All appear in both `lib` and `lib+test` domain-core units. The
`state/proof.rs` file contributes 16 paths: two authored `body/open` functions
(`correspond_checked_target`, `same_candidate`), one structural implementation
declaration and 13 structural import markers. The
`state/register/logical/origin.rs` file contributes 18 paths: authored
`body/open` functions `mismatch`, `imports` and `trace`, 14 authored
`body/open` closures (byte spans `1041:1066`, `1515:1517`, `1885:1887`,
`2478:2485`, `3107:3114`, `4076:4083`, `4441:4443`, `5061:5064`,
`5481:5484`, `7526:7535`, `7724:7731`, `8590:8592`, `8832:8834`,
`9177:9191`), and the structural `PendingUse` record. The
`noble-wasm/src/source/origin.rs` file contributes 28 paths: three authored
`body/open` bound initializers (`MAX_ORIGIN_PROGRAMS`,
`MAX_ORIGIN_OPERATIONS`, `MAX_ORIGIN_DEPTH`), five authored `body/open`
functions (`action`, `programs`, `dynamic`, `returned_outputs`, `build`),
the structural `Value` enum and 19 structural import markers. The closure
spans belong to the named-origin source file, not to similarly named
`logical.rs` or `proof.rs` entries. There are no test-only, `#[cfg(test)]`,
derive-generated, or stale policy paths in these three files.
Thirteen further authored closures have no separate IR item fact or path:
`state/proof.rs` lines 35, 41, 46, 101;
`state/register/logical/origin.rs` lines 26, 42, 60, 74, 95, 105, 191;
and `noble-wasm/src/source/origin.rs` lines 80, 119. Their containing
authored bodies (`correspond_checked_target`, `imports`, `trace`, `programs`
and `returned_outputs`) retain open extraction/refinement obligations;
no invented closure paths are added.

Classification does not discharge any source-bound proof or admit the actual
application. The pure comparator on the unchanged inventory reports 6,010
reviewed production paths (2,728 body/open, 2,133 generated, 1,149
structural), 2,343 omitted and 105 stale; test-only debt remains 1,050
omitted and 115 stale. The four existing diagnostics remain
`subject-omission`, `subject-stale`, `test-subject-classification`, and
`reviewed-scope-partition`. The separate comparator report is retained at
`/home/brittonr/.cache/noble-candidate-source-inventory-review-20261007/source-coverage-origin-body-review.json`;
earlier reports remain unchanged. Actual Octet architecture remains blocked
by compiler unknowns; neither the collector nor the full source/native
acceptance gate is claimed here.

## Candidate logical-binding and namespace complete-file review (2026-10-07)

The same unchanged 54/54 compiler observation contributes **52 unreviewed
production item facts / paths** from
`crates/noble-contracts/src/source/declared/state/register/logical.rs`.
Forty-three are `body/open`: 12 authored functions (`at`, `stage`,
`dependency_bytes`, `pure_type`, `types`, `source_origin`,
`SourceOrigin::definition_source`, `bind_subject`, `resolve_contract`,
`is_current`, `imported_module`, `resolve_uses`), two authored constant
initializers (`TYPE_STACK`, `USE_FRAMES`), and 29 authored closures.
The 29 observed closure byte spans, in source order, are `1425:1434`,
`1539:1548`, `2126:2135`, `2502:2509`, `2572:2579`, `4042:4045`,
`4365:4367`, `5432:5434`, `8348:8357`, `9090:9092`, `10187:10190`,
`10873:10876`, `11070:11073`, `11253:11255`, `12359:12366`,
`13856:13859`, `14125:14127`, `17166:17176`, `17224:17226`,
`17429:17439`, `17487:17489`, `17745:17755`, `17799:17801`,
`17984:18001`, `18444:18452`, `18619:18626`, `18691:18693`,
`19994:20001`, `20726:20728`. Nine paths are structural: the
`Staged`, `Request` and `SourceOrigin` declarations, `SourceOrigin`
implementation, `origin` module declaration, and four function-local
`use` markers. The separately reviewed `logical/origin.rs` paths are not
recounted by this parent module declaration.

The complete `crates/noble-contracts/src/source/declared/state/namespace.rs`
file has **16 observed production item facts / paths**. Nine were already
reviewed (seven methods and the implementation/module declarations).
Its seven newly reviewed `body/open` paths are the authored
`checked_signature` method and six authored error closures at `1297:1299`,
`1602:1605`, `4482:4484`, `5597:5599`, `5813:5816`, `7432:7434`.
Twenty-two additional authored closures have no separate raw IR item path.
In `logical.rs`, `stage` owns those at lines 42, 43 (`.map`) and 93;
`source_origin` at 219; `bind_subject` at 275, 312, 325;
`resolve_contract` at 382, 387, 392; `imported_module` at 411; and
`resolve_uses` at 451 (`.map`), 474, 484, 488, 494. The closure at
`resolve_uses` line 443 has its own observed compiler item and is
already among the 29 recorded paths. In `namespace.rs`, `checked_signature`
owns those at lines 15, 19, 23; `resolve_type` at 113 (`.map`); and
`import` at 203, 214. The containing authored bodies keep their open
obligations; no synthetic paths were added. All 68 observed items have
source-file spans, are non-expansion items in both `lib` and `lib+test` domain-core
units, and none are test-only or derive-generated. The two complete files
add **59** reviewed paths: 50 `body/open`, nine structural and zero
generated. `logical.rs` has no linked stale policy path. Six historical
`namespace.rs` closure paths remain stale: `283:285`, `588:591`,
`1881:1883`, `2996:2998`, `3212:3215`, `4831:4833`. They are included
in the unchanged total of 105 stale production paths, not silently
reclassified or deleted. Their current observed closure offsets are
`1297:1299`, `1602:1605`, `4482:4484`, `5597:5599`, `5813:5816`,
`7432:7434`, respectively; the historical source file changed, so exact
old-to-new source identity requires its own review.

The pure comparator remains `valid=false`: 6,069 reviewed production paths
(2,778 body/open, 2,133 generated, 1,158 structural), 2,284 observed
omissions and 105 stale reviewed paths. Test debt stays at 1,050 omitted
and 115 stale. The four diagnostics remain `subject-omission`,
`subject-stale`, `test-subject-classification`, and
`reviewed-scope-partition`. The separate report is retained at
`/home/brittonr/.cache/noble-candidate-source-inventory-review-20261007/source-coverage-logical-namespace.json`;
historical reports remain unchanged. Classification keeps all authored
bodies open and provides neither source-bound proof nor application
admission. The collector, Octet architecture gate and full source/native
gate were not rerun.

## Candidate named-v2 complete-file test-only review (2026-10-07)

The unchanged 54/54 inventory contains two separate `test+test` units:
`noble-contracts` / `crates/noble-contracts/tests/named_v2.rs`
(`9ad7ae351afa654aac0e599d09dcc26f1a27ca181084583830bed90e3d6432c9`)
and `noble-cli` / `crates/noble-cli/tests/named_v2.rs`
(`b68db1d6e5b6663d6be2b52bae9c61cac9c1cfe1a378f3292f2827fec671f72c`).
Both have role `test`, not production; their raw item observations retain
the distinct package and unit identities. The contracts file has 62 raw
item facts, 47 unique paths: 28 body-bearing, 15 structural `use` paths,
four pure harness-generated paths. The CLI file has 47 raw item facts,
39 unique paths: 26 body-bearing, nine structural (seven `use` markers,
the `Fixture` struct/inherent implementation sharing a path, and its
`Drop` implementation), four pure harness-generated paths.

The 11 shared qualified names are `named_v2::main`,
`named_v2::main::test`, `named_v2::std`, `named_v2::test` and
`named_v2::{use#0}` through `named_v2::{use#6}`. Both files agree on
their pure generated/structural classifications, so the policy's
qualified-path set counts them once while inventory retains both
package/unit observations. The union of 109 raw facts is **75 new
unique test-only names**: 54 body-bearing, 17 structural, four
pure generated. Authored constants, helper functions, test functions
and source closures are body-bearing; compiler-generated `#[test]`
wrapper closures inherit each authored test body's obligation.
Generated registration constants sharing authored test paths do not
erase those obligations. This classification grants no production-role
exemption if any of these qualified names appears in a production unit;
none does in this inventory. No prior test-policy path was removed.
Eight additional authored closures have no separate compiler item fact:
contracts `fixture_for` line 18 (`.map`),
`named_v2_rejects_partial_word_wrong_token_and_alias_rebind` line 99
(`.position`), and `named_v2_rejects_wrong_inst_and_unreachable_graph`
lines 114, 115, 122 (`.find`, `.map_err`, `.find`); CLI `copy_lean_tree`
line 74 (`.is_none_or`) and `named_consumer` line 195 (both `.split` and
`.any`). Each inherits its enclosing test-only body obligation, not an
invented path. The separate CLI line 78 compiler unknown is unchanged;
fact-free closure accounting does not classify or excuse that unknown.

The pure comparator remains `valid=false`: production stays at 6,069
reviewed (2,778 body/open, 2,133 generated, 1,158 structural), 2,284
omitted and 105 stale; test policy rises from 1,344 to **1,419**
reviewed names, with **975 omitted and 115 stale**. Its diagnostics
remain `subject-omission`, `subject-stale`,
`test-subject-classification`, `reviewed-scope-partition`; the 105/115
stale paths are not deleted or silently reclassified. The separate
report is
`/home/brittonr/.cache/noble-candidate-source-inventory-review-20261007/source-coverage-named-v2-test-only.json`.
This is source inventory accounting only, not proof, admission,
extraction, or permission to run the collector or full gate.

## Candidate source-proof and named-v2 production complete-file review (2026-10-07)

The complete `crates/noble-contracts/src/source/proof.rs` has 81 observed
production item facts / **80 unique paths** in the `noble-contracts`
`domain-core` `lib` and `lib+test` units. Its 29 `body/open` paths are
25 authored functions (including the selected live-slot definition and
source/quote correspondence checks) and four authored closures observed
at byte spans `3744:3751`, `4130:4137`, `6899:6906`, `12411:12417`.
The **25 structural** paths include five structs, an enum, five authored
implementation blocks, and 15 local `use` markers. The authored
`CheckedDefinitionId` struct and its authored impl have the same
qualified path, so 81 facts do not mean 81 paths. The **26 pure
generated** paths are 17 derived implementations and nine derived
methods from `CheckedDefinitionId`, `SourceProofRefusal`, and
`ProofContext`. They are accounting, not a proof of the authored bodies.

Fifteen additional authored closures in `source/proof.rs` have no
separate raw IR item path: `CheckedSelectedTarget::checked_operation_span`
owns those at lines 77, 82, 87, 89, 113, 119;
`CheckedSelectedTarget::checked_quote_site` at 136, 137, 142, 150,
152, 156; `Session::checked_selected_target` at 246, 248, 253.
Those inherit their enclosing open bodies; do not invent policy paths.
The four observed closure paths carry only their parameter-token spans
(`|index|`, `|bytes|`, `|index|`, `|slot|` respectively), not complete
body spans, so source reconciliation includes their containing methods.

The complete
`crates/noble-cli/src/workflow/verification/named_v2.rs` has **two
observed production item facts / paths**, both `body/open`: authored
`noble::workflow::verification::named_v2::verify` and its
`ok_or_else` closure at `503:505` (line 11, the `||` parameter token).
This is the `noble-cli` `composition-root` `bin` and `bin+test` unit,
not the test-only `tests/named_v2.rs`. Both files are wholly newly
reviewed in their complete production modules; their total is
**82 new paths** (31 body/open, 25 structural, 26 generated).
None of the 105 historical stale production policy paths link to
either file, and no stale path was deleted.

The pure comparator remains `valid=false`: **6,151 reviewed production
paths** (2,809 body/open, 2,159 generated, 1,183 structural), **2,202
observed omissions**, 105 stale; tests remain 1,419 reviewed, 975
omitted, 115 stale. Its four diagnostics remain `subject-omission`,
`subject-stale`, `test-subject-classification`,
`reviewed-scope-partition`. The separate pure-comparator result is
`/home/brittonr/.cache/noble-candidate-source-inventory-review-20261007/source-coverage-proof-named-v2-production.json`.
The retained proof- and target-related source is *not* behaviorally
proved, admitted, extracted, or a claim of a full source/native gate.

## Candidate named-source identity and selected-root complete-file review (2026-10-07)

The complete `crates/noble-contracts/src/intrinsic/named_v2.rs` has
**87 raw production item facts / 87 newly reviewed paths**:
48 `body/open` (20 authored functions and 28 separately observed
authored closures), 39 structural (the `Graph` declaration and 38
`use#0`–`use#37` source import markers), zero generated or constant
facts. The source binds original named-call ownership and alias
resolution, an independently accepted source recipe and exact
two-call graph to a bounded NamedV2 statement; classification does
not itself establish any of those claims. Its closure item spans
generally cover `|parameter|` or `||` rather than their entire bodies;
the `check_contract` closure at `27783:27820` (line 464) includes
`|| -> Result<CheckedProof,Diagnostic>` but not the body. The
function paths and observed closure paths remain independently
`body/open`.

Sixteen further authored closures in that file have no separate IR
item path: `checked` lines 30, 60; `source_call` 136;
`same_derived` 158; `verify` 225, 231, 233, 253, 272, 276, 330,
332, 360, 368; `statement` 428, 431. They inherit their named
enclosing body's open obligations; they do not authorize synthetic
qualified paths. All 87 observed item spans are within the source
file, workspace-origin and not expansion facts. No prior policy path
linked to this file is stale.

The complete `crates/noble-contracts/src/source/preparation.rs` has
**nine raw production facts / nine paths**. Five previous reviewed
paths (`Session` implementation, `prepare`, `environment`, `retained`,
`declaration`) remain unchanged. Its **four new `body/open` paths**
are `prepare_signed` and the observed authored closures at
`2151:2157` (line 48, selected-root lookup), `4697:4707`
(line 111, binding-key clone), `5666:5669` (line 135,
environment error). Authored closures at line 37 (`.any`) inside
`prepare_signed` and line 185 (`.and_then`) inside `declaration`
have no separate compiler item and inherit those open bodies.
The earlier reviewed `declaration` computes its source definition
identity; this review makes no new authority or proof claim.

Both complete files belong to `noble-contracts` `domain-core` `lib`
and `lib+test` units, not test-only units. Together they add **91
reviewed production paths**: 52 `body/open`, 39 structural, zero
generated. Exactly one historical stale path is linked to
`source/preparation.rs`:
`noble-contracts::closure@crates/noble-contracts/src/source/preparation.rs:3386:3396`.
Those old offsets now slice `vec::Vec::`, not an observed current
closure; this is not proof of old-to-new identity. Retain that path
among the existing **105 stale production paths** and delete none.

The pure comparator remains `valid=false`: **6,242 reviewed
production paths** (2,861 body/open, 2,159 generated, 1,222
structural), **2,111 observed omissions**, 105 stale. Test debt
remains 1,419 reviewed, 975 omitted, 115 stale. Its four diagnostics
remain `subject-omission`, `subject-stale`,
`test-subject-classification`, `reviewed-scope-partition`. The
separate result is
`/home/brittonr/.cache/noble-candidate-source-inventory-review-20261007/source-coverage-intrinsic-named-v2-preparation.json`.
No collector, full source/native gate, architecture admission,
extraction or refinement proof is implied.

## Current handwritten Module equality correction (2026-10-07)

The current `crates/noble-contracts/src/source/declared.rs` has a
hand-written `impl PartialEq for Module` at lines 82–83. The unchanged
54-unit compiler IR records its implementation at byte span
`2298:4206` (`from_expansion=false`) and authored `eq` function at
`2330:4204` (`from_expansion=false`). Earlier current policy incorrectly
carried both as generated from the old `#[derive(PartialEq)]`. Only
those **two existing** production paths change classification:
the impl is structural/not-applicable, and `eq` is `body/open/open`.
The old derive and its historical receipt confer no proof on the
present hand-written equality, which compares source-owned fields
and published host results. The separate historical
`StructuralPartialEq` stale path stays among the 105; none of the
other stale entries or 86 mapped-but-unreviewed current closures was
deleted, aliased, or waived.

The pure comparator remains `valid=false`: **6,242 reviewed
production paths**, now **2,862 body/open, 2,157 generated, 1,223
structural**, 2,111 observed omissions and 105 stale. Tests remain
1,419 reviewed, 975 omitted and 115 stale; diagnostics are
`subject-omission`, `subject-stale`,
`test-subject-classification`, `reviewed-scope-partition`. The
pure-comparator report is
`/home/brittonr/.cache/noble-candidate-source-inventory-review-20261007/source-coverage-module-partialeq-correction.json`.
At the time of this correction, the v1 inventory comparator checked
subject names and reviewed categories but did **not** carry raw IR
`from_expansion`; the correction rests on independently inspected
source and compiler spans, not a category-proof claim by that old
comparator or a name-only contract test. The v2 cutover below adds
bounded expansion-provenance rejection, not full classification proof.

## Compiler expansion provenance at the source-inventory boundary (2026-10-07)

The pure inventory derivation now emits `noble-source-inventory/v2`.
For **each** compiler item fact it joins `fact.span_id` to the raw
compiler IR span and preserves both `span_id` and boolean
`from_expansion` in that item's inventory row. Missing references or
nonboolean expansion values reject derivation rather than defaulting.
The derivation also rejects missing expansion attributes, any duplicate
span ID (even a byte-identical repeated row), duplicate fact IDs, and
item facts with no observing unit. Without that guard, `listToAttrs`
keeps the first occurrence of a duplicate span ID, potentially
shadowing a later conflicting authored record behind an earlier
forged expansion; `all []` could also classify an unowned item as
test-only. None of those cases occurs in the frozen
selected IR: 70,612 span IDs and 160,618 fact IDs are unique, and all
11,293 item facts have unit membership and a boolean-bearing span.
The comparator rejects v1 inventories and missing/nonboolean
provenance in either production or test rows; it groups **all**
production observations by qualified path, including duplicate paths
from different units. A reviewed `generated` path with an observed
hand-authored item is rejected as `generated-authored-subject`, even
if another observation of that path came from an expansion. An empty
observed group is rejected by the existing `subject-stale` diagnostic
instead; the retained historical `StructuralPartialEq` stale derive
cannot become an accepted current generated subject. A true expansion
flag permits generated *accounting*, not proof of its classification,
extraction, refinement, or behavior.

The unchanged selected 54-unit raw IR (`cd6808f09f8f50b2bc07390cb1cbb3397124bc0cfadd4430d19b4cf32e791f20`)
was rederived without a collector run into
`/home/brittonr/.cache/noble-candidate-source-inventory-review-20261007/inventory-v2-module-partialeq.json`.
All 8,421 production and 2,872 test item rows retained their previous
identity, order, and unit memberships, with added per-item span
provenance. The guarded rederivation
`/home/brittonr/.cache/noble-candidate-source-inventory-review-20261007/inventory-v2-guarded.json`
is byte-identical to the first v2 derivation despite the stricter
identity checks. Twelve real production paths have expansion-first,
authored-later observations; two are reviewed as body/open. A synthetic
expansion-first generated mutant must reject, rather than trusting only
the first row. The selected pure comparator also rejects a generated
mutant of those two reviewed real mixed paths; its diagnostic report is
`/home/brittonr/.cache/noble-candidate-source-inventory-review-20261007/source-coverage-v2-mixed-first-mutant.json`.
Pure comparison with the corrected reviewed policy remains
`valid=false` with exactly the four earlier diagnostics:
`subject-omission`, `subject-stale`,
`test-subject-classification`, `reviewed-scope-partition`; there is
**no** `generated-authored-subject` in the baseline. The 6,242
reviewed production paths remain 2,862 body/open, 2,157 generated,
1,223 structural, with 2,111 omissions and 105 stale. Relabeling
exactly the current handwritten `Module` impl and `eq` as generated
adds `generated-authored-subject` in the selected pure comparator.
The independent diagnostic-only baseline and mutant reports are
`/home/brittonr/.cache/noble-candidate-source-inventory-review-20261007/source-coverage-v2-expansion-baseline.json`
and
`/home/brittonr/.cache/noble-candidate-source-inventory-review-20261007/source-coverage-v2-expansion-mutant.json`.

`source-inventory collect` derives v2 from its own collector artifacts;
the flake `source-coverage` check uses the same derivation. In contrast,
`source-inventory check --inventory FILE` only compares the **supplied**
inventory: it does not independently prove the file came from the selected
IR or exact architecture policy. Its result is diagnostic-only unless
the inventory was rederived and bound to selected compiler artifacts;
neither mode substitutes for a complete source/native gate. The older
archived v1 receipts stay bound to their historical source and are not
silently upgraded or accepted as current v2. The bounded synthetic
control set now has 63 cases (the prior 62 plus the identical-duplicate
span rejection); these complement rather than replace selected
pure derivation and comparison.

## Commands

The existing collection and comparison interfaces are:

```sh
source-inventory collect --root DIR --selection FILE --artifact-dir DIR
source-inventory check --root DIR --selection FILE --inventory FILE --policy FILE --artifact-dir DIR
```

The Nix source-coverage check uses those collection/comparison interfaces:

```sh
export NIX_CONFIG='min-free = 0
max-free = 0
builders =
sandbox = true
require-sigs = true'
nix build --offline --no-write-lock-file .#checks.x86_64-linux.source-coverage \
  -L --out-link "$EVIDENCE/source-coverage"
```

The independent `checks.x86_64-linux.source-inventory-controls` check
evaluates the synthetic comparator controls and writes their names to
`self-tests.txt` without evaluating the compiler inventory or the blocked
`source-coverage` receipt. Passing it does not make source coverage valid:
the actual app still gates comparison behind Octet architecture.

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

The policy is not an isolated historical M1 fixture. `flake.nix` still compares
it with the current workspace compiler inventory and current architecture
scope. Its authoritative scope includes `noble-kernel`, `noble-contracts`,
`noble-wasm`, `noble-cli` and `noble-syndicate`; the CLI executable's compiler
namespace is `noble`. Independent Wasmtime peers are separate
verification-only workspaces. Owned JS/WAT/runtime assets, the canonical ABI,
host provenance, native release and external tools require separately bound
evidence and trust accounting, not Rust extraction coverage.

## Classification

Each compiler-derived production subject is reviewed as one of three categories.

| Category | Meaning | Disposition |
|---|---|---|
| `body` | A Noble-authored function, closure carrying authored source-body semantics, or named constant initializer observed in a production-role unit | `extracted`, `modeled`, `excepted`, or `open` |
| `generated` | A compiler-generated derive or harness item with no independent Noble body obligation | `generated` |
| `structural` | A declaration, implementation block, module, macro, type, or import marker; associated bodies are accounted separately | `structural` |

Bodies additionally carry a refinement status: `proved` or `open`.
Only bodies enter the extraction, modeling, exception, proof, and open counts.

The last compiler-derived DXM1-reviewed 32-unit observation contains 2,595
authored-body paths, 2,131 generated paths and 1,090 structural paths: 5,816
unique production paths covering 5,926 compiler item facts. A path shared by an authored body and
generated items retains the body obligation. The difference between item and
path counts also includes
compiler observations of different item kinds under one qualified path.

The M6 renewal restored 195 retained authored closures from `generated` classification
to `body` / `open` / `open`, removed 18 stale source-offset closure entries, and
added 19 new closure entries. Anonymous syntax or a generated wrapper does not
erase authored semantics. The correction restores explicit obligations; it does
not extract, model or prove those bodies, and removing a stale offset is not a
discharge of its semantic obligation.

All 2,696 currently reviewed body dispositions and refinements remain `open`
in this inventory (the historical 2,595 plus the 101 candidate-file bodies).
It does not import or discharge the separately bound kernel, MC1, Wasm-emitter
or M4/MC2/M5 frontend/compiler/companion/resource evidence, the bounded M6
async extraction, the three pure selected M7 dataspace functions, or bounded
M8 choreography acceptance. Zero
bodies here are
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
These remain historical M5 counts and bindings, not the current DXM1 expectation.

## Compiler review observation

The current DXM1 [22-command, fourteen-gate assurance](declared-modules-v1/assurance.json)
independently passes complete signed/sandbox Nix flake checks over frozen
pre-promotion source, including published Octet deny-all. The compiler
collector and reviewed coverage check all 32 expected and observed units,
with no missing/duplicate identities or approved exceptions, on the selected
`x86_64-unknown-linux-gnu` default-feature configuration. All 2,595
authored production bodies remain open source-inventory refinement obligations;
2,131 generated and 1,090 structural paths are separate accounting, not
proved bodies. The 5,816 unique production paths comprise 5,926 compiler
item facts; 1,333 exclusively test-role paths comprise 1,668 facts. The
compiler IR identity is `d3042d0bced94e4e3b06e15152fbf92f2ac625a642685ddbdfe5ba35e1e4e89c`, coverage identity is
`8d51c4c1f83f7b51b532f9d430416fe7454b51002780cb230a95cdce44930b59`, and reviewed compiler architecture receipt
SHA-256 is `b73ab1c2e60ef4fb0900ab18392dbc52cb09c47a1d910f07aef22e1ee800a3ab`. The separately checked
[completion evidence](declared-modules-v1/evidence.json) records 32 macro
origins, seven production dependency edges, thirteen selected tools and
zero approved exceptions or waivers. Independent source-bound assurance,
scoped runtime tests and inherited strict Lean proofs remain distinct.

### Historical M8 compiler review observation

The historical M8 [seventeen-command assurance](m8/assurance.json)
independently passes full Nix flake checks over frozen pre-promotion source.
Its `source-coverage` output checks all 30 then-expected and observed compiler
units with no missing, unexpected or duplicated identities, valid shards,
one selected `x86_64-unknown-linux-gnu` default-feature configuration,
and 2,087 open authored production bodies. That fresh compiler architecture
IR identity is
`57602668008355ffbf03f0e953303ea1b234a447be97502478127458de719da2`,
the matching unit-coverage identity is
`04b7da4b6083eaf0ffdd434f984734673783f3f9033b096ff9fae107e25b5378`,
and the checked architecture receipt identity is
`6a4e477ddcab68a4d2dbfd8219d42fcc9dc59aa86e1a6b8643c88086e9dab1d7`.
The separately checked [M8 completion record](m8/evidence.json) retains
1,120 exclusively test-role paths across 1,406 test item facts, 27 macro
origins, seven production dependency edges, thirteen selected tools and
zero approved exceptions. Historical diagnostic IDs in reviewed Nickel
comments do not replace fresh complete-Nix observations.

### Historical M7 compiler review observation

The historical M7 [sixteen-command assurance](m7/assurance.json) independently
passed complete Nix flake checks over frozen pre-promotion source. Its
`source-coverage` output reports all 29 expected and observed compiler unit
identities with none missing, unexpected or duplicated, valid shard coverage,
one selected `x86_64-unknown-linux-gnu` default-feature configuration and
2,006 open authored bodies. The fresh compiler architecture IR identity is
`b2c98fe0a17d37b06a7f796730177cbd71e332e6a270e10fe04de23f211586e7`,
the matching unit-coverage identity is
`fb7adb09b9aae311b66ebce774935393129d75b809bf2c0b5d469cbe64df675b`,
and its checked exported architecture policy identity is
`c9e505706e8821ccf700038c36b9405cce3127e48da587b59fae409e6304cd3a`.
The separately checked [M7 completion record](m7/evidence.json) retains 1,076
exclusively test-role paths, 27 macro origins, seven production dependency
edges, thirteen selected tools and no approved exceptions. Historical
pre-capacity diagnostic IDs in comments of the frozen reviewed Nickel
inventory do not replace these fresh complete-Nix observations.

### Historical M6 compiler review observation

The M6 review was bound by the compiler IR, coverage, collector receipt/policy,
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
identifiers are not the historical M6 binding, and an archive location alone
is not a passing receipt.

Historical M6 coverage reported all 25 expected units: six domain-core units
(three library and three library-test configurations), two composition-root
units (binary and binary-test), and seventeen test-role units. The single
declared configuration is
`x86_64-unknown-linux-gnu`, `features=[]`, `default_features=true`.
The historical M6 observation had 1,044 unique exclusively test-role paths
across 1,302 test item facts, 26 macro origins, six production dependency
edges, no external Cargo edges **in that earlier source**, and thirteen
selected tools. The candidate's seven CLI external edges are recorded above.
Both future components remain `open`, and the
exception set is empty. The three domain-core crates retain their `no_std`
policy obligations.

Complete collection alone is not a passing architecture gate. The earlier interim run's required production desugaring unknowns were not waived; this observation was collected after those source corrections. Listing `<builtin>` in the macro-origin set does not waive any required unknown, and the full compiler IR can retain test-only unknown facts without granting them production authority. Gate mode, core capability prohibitions, and empty waiver/exception sets remain unchanged.

The default catalog was insufficient for M6 acceptance. The published deny-all
hook enables 73 rules, including off-by-default rules. That M6 source resolved
its findings through legal const helpers, closed-enum declarations, bounded
module separation, explicit units, and narrowly scoped explanations where
heuristic requirements conflict with fallible validation or guarded arithmetic
bounds. No assertion padding, new opaque extraction boundary, policy waiver,
or catalog weakening was introduced.

The architecture policy names the exact observed CLI effect owners, including
compiler-qualified closure identities and filesystem observations through
`std::path`. Its manifest uses the selected Nickel and `octet-standards` tools.
The inherited source comparison, policy/tool-selection freshness checks, and
strict Octet gate remain separate acceptance controls; this document is not
their execution receipt. Closure identities include source byte offsets, so even
comment or help-text edits can require renewal. All 25 expected M6 workspace
units were mandatory; the current DXM1 renewal requires all 32. An acyclic
boundary control must additionally retain both
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
[reviewed lock](../verification/m4/extraction-lock.json): 526 M6 source files,
fourteen strict actual-Rust roots for transition, table-decision, classifier
and record-validation correspondence plus qualified lifecycle properties,
all 75 state/event constructor pairs, and 176 M6 refusal controls (228 total
across inherited lanes). The [formal archives](../verification/m6/formal-archives.json)
retain the discovery and check products with full roundtrip integrity.
Native execution, constructor coverage, extracted-body coverage and
refinement remain distinct; none establishes unmodeled host or native runtime
semantics.

M7 separately [checks 548 actual Rust source files](m7/extraction.json) under
the reviewed lock. Exactly three compiler-ID-joined transparent pure Rust
dataspace functions at DefIDs 31/32/33 have nine strict Lean
correspondence/refusal/invariant theorems against independent finite
references, and the whole-crate check rejects 37 M7 mutations (265 combined
inherited and M7 controls). This is not table, wire, host, adapter, engine,
full frontend/backend or 2,006-body refinement. The
[M7 assurance](m7/assurance.json) independently passes MC1/MC2/M3/M4/M5/M6
regressions, workspace Rust tests/Clippy and complete published Octet and Nix
quality; the earlier [M6 regression](m6/regressions.json) and
[declared checks](m6/nix-checks.json) retain their own bound scope.

M8 separately [checks 562 reviewed source files](m8/extraction.json)
against the independently renewed shared lock. It preserves M7's exactly
three compiler-ID-joined transparent pure dataspace functions at DefIDs
31/32/33 and all nine strict Lean roots, while refusing 265 inherited and
renewed mutations across prior proof lanes. No new M8 parser, projector,
monitor, adapter, host or engine theorem is claimed. The
[M8 assurance](m8/assurance.json) separately passes seventeen commands
and eleven gates, including fresh M7 seven-case/sixteen-variant regression,
workspace Rust checks, published deny-all and complete Nix flake. All
2,087 then-current authored production bodies remained open inventory
refinement obligations; bounded compiled execution and proof roots did not
discharge them.

DXM1 separately [checks 630 actual reviewed source files](declared-modules-v1/extraction.json)
under the renewed shared [lock](m4/extraction-lock.json). All 268 inherited
and renewed mutation controls refuse the planted defects (34 earlier, 21
M5, 176 M6 and 37 M7); no stale generated Lean source is admitted. Exactly
three compiler-ID-joined pure M7 dataspace functions at DefIDs 31/32/33
retain nine strict Lean roots; there is no new nominal, module, adapter,
host or Wasm-engine theorem. The [DXM1 assurance](declared-modules-v1/assurance.json)
separately passes actual DX-03/08/09 compiled-source acceptance, M4–M8
regressions, workspace Rust/Clippy, complete published Octet deny-all and
full Nix flake. All 2,595 current authored production body obligations remain
open. Anonymous authored closures remain open, including the observed
`nominal/inspection.rs:5810:5817` source-offset body; that reviewed
classification is not a generated-body exception or proof discharge.

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

Thirty-eight synthetic controls cover these rules, including separate target,
feature and default-feature mutations. The external-edge controls accept an
exact reviewed CLI-to-`anyhow` normal edge but reject unreviewed observed or
stale reviewed edges and changed caller/kind. External-edge filtering fails
the positive control; an observed-subset-of-reviewed comparator mutation fails
the stale-edge control. Test-subject controls reject missing and stale paths
and do not allow a test-set entry to excuse an observed production body.
Macro controls reject missing, unexpected, and stale origin names.
These are harness tests, not compiler evidence.

## Limits

The reviewed policy is a static expectation. The build never refreshes it.
Renewal is an explicit reviewed action bound to the compiler-derived set at review time.
The inventory covers the selected target and feature configuration only.
Structural and generated categories are accounting, not independent verified bodies.
No entry establishes refinement, native safety, dependency safety, or whole-project verification.
