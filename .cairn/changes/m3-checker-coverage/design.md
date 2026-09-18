# M3 checker coverage design

## Context

M2 is complete and integrated (`2ffd19a`): the named `M2 fragment v0` checker exists in `crates/noble-kernel`, every kernel function translates through the pinned Charon/Aeneas pair, the generated module is computable, and the reference model in `proofs/m2` proves soundness (`check_soundness`), termination (`check_total`, `foldBody_work_bounded`), positive coverage (`coverage_positive`), and refinement of the extracted `noble_kernel.acceptance.check` (twelve fixture agreements plus `accepted_via_refinement`) — `verification/m2-scope-review.md` items 2-6, theorem inventory in `verification/m2-proof-evidence.md` section 3.
The proof is nevertheless end-to-end over one entry point: `verification/m2-coverage.json` classifies 474 generated functions as 451 extracted, **1 proved**, 17 modeled, 5 excepted, 0 open.
The verification-toolchain entry point requires the next extension once that pattern exists: "extend extraction and refinement across the kernel" (`specs/VERIFICATION-TOOLCHAIN.md` section 8).
The core-bootstrap specification fixes the checker boundary (B-CHECK-01 through B-CHECK-07 in `.cairn/specs/core-bootstrap/spec.md`), the language specification fixes the declarative judgment and the word contracts (`.cairn/specs/language/spec.md` sections 4.4, 6.3-6.5, 7.1-7.4, 8.1, 11.3), and the verification specification fixes the acceptance-theorem shape (V-CHECK-03), the outcome, termination, and coverage duties (V-CHECK-04/05), and the narrow-label rule (V-MODEL-01, V-GATE-05).

## Goals and Non-goals

M3 delivers the named **bootstrap fragment v1**: the complete B-CHECK-01..07 audit with the partial families scoped in, the complete bootstrap word table with per-word contracts and controls, per-rule and per-word proof depth in a new `proofs/m3` root over the same pinned route, and the extended M2 gates.
M3 does not deliver inference, unification, generalization, recursion support, live resources, evaluation, recipes, Wasm, host boundaries, or whole-kernel verification, and it does not claim the roadmap's Wasm milestone (M3 `wasm-feasibility`, `specs/roadmap.json`) or any conformance-ledger scenario execution.

## Decisions

### Decision: Extend the named fragment by audit, not by redefinition

**Choice:** Fragment v1 is `M2 fragment v0` (types, schemes, node forms, limits, outcomes, and diagnostics per `verification/m2-fragment.md`) plus exactly the families the audit below marks `In`, each with its own named controls and theorem obligations.
Every addition keeps the fragment label `bootstrap fragment v1`; every omission stays enumerated (V-MODEL-01).

**Rationale:** V-GATE-05 requires narrow results to retain their exact subset labels; B-SCOPE-01 fixes Core-Bootstrap as the target subset; the M2 records make the audit factual rather than aspirational.

**B-CHECK-01..07 audit (M2 evidence versus M3 scope)**

| Requirement | M2 fragment v0 status (evidence) | Fragment v1 scope |
|---|---|---|
| B-CHECK-01 untrusted candidate, independent acceptance against explicit expected interface and environment | Covered: `noble-candidate/v0` request/candidate split, untrusted witnesses, five-way outcomes (`verification/m2-fragment.md` "Candidate and request schema", "Limits and outcomes"; VT-M2-01) | Retained; controls re-run over v1; schema shape unchanged unless a scoped-in rejection needs a field, which bumps the format revision with both revisions controlled |
| B-CHECK-02 exact environment identities; external environment data separately validated; recursive-definition dependencies and user-declared recursive schemas unsupported; bootstrap `List` supported | Partial: the fixed 22-entry table binds exact identities and the `List` schema (`crates/noble-kernel/src/contracts/bootstrap/data.rs` `table()`), but recursion is only silently omitted (`verification/m2-fragment.md` "Omissions"); no named rejection control exists | **In:** explicit non-accepted outcomes with diagnostics for (a) a recursive definition dependency and (b) a user-declared recursive schema in environment data, plus a control that external environment data does not enter checking unvalidated |
| B-CHECK-03 declarative derivation per node, complete ordered stack joins, quotation effect separation, fresh instantiation, duplication shares one interface | Covered at control level (FRAG-WORD/SEQ/QUOTE families, `verification/m2-fragment.md` "Control map"); proof depth is one end-to-end function (`verification/m2-coverage.json`: 1 proved) | **In:** per-rule refinement theorems for LITERAL, WORD, SEQUENCE, and QUOTATION (decision below); a duplication-sharing negative control (a duplicated quotation's second use cannot claim an independent instantiation) |
| B-CHECK-04 eligibility and effect inclusion from derivations; empty bound cannot hide `test.emit`; host denial does not repair | Covered: ELIG-DATA and EFF-INC families with hidden-emit and narrowed-bound negatives (`verification/m2-fragment.md` "Control map"; CORE-15 scenario design) | **In:** the eligibility and inclusion decision paths become separately proved subjects (`eligibility_iff`, `inclusion_iff` below); per-word latent-bound propagation enters the per-word theorem set |
| B-CHECK-05 finite, well-founded candidates and substitutions; rejects cyclic type equations and malformed node references; decreasing measure or fail-closed limit | Partial: work budget with fail-closed accounting, `check_total`, `foldBody_work_bounded` (`verification/m2-proof-evidence.md` section 3); malformed node references rejected in the malformed lane (`verification/m2-acceptance-run.md` "Developer-experience controls"); cyclic type equations cannot arise in v0 (no unification) and no rejection control exists | **In:** named rejection controls for cyclic substitution/type-equation inputs at the v1 boundary (self-referential and mutually referential substitution witnesses); every new v1 path charges work before allocation or traversal (B-CHECK-07 discipline) |
| B-CHECK-06 branch-local types from checked eliminator derivations; complete common output stack and conservative bound; no trusted refinements or implicit union joins; `Sum<Resource<R>,I64>` elimination exposes the payload at its own type and the sum stays non-`Data` | Partial: `if`, `case`, `list.case` are implemented with join controls (FRAG-BRANCH, `verification/m2-fragment.md` "Control map"; fragment-suite branch cases and the DX-01 branch control in `verification/m2-acceptance-run.md`), but the property pool omits `case`/`list.case` (`crates/noble-kernel/tests/property/fit.rs` `WORDS`, 19 entries), the oracle has no arms for them (`crates/noble-kernel/tests/property/table.rs` `word_face`), and no payload-exposure or implicit-union-join negative exists | **In:** all three eliminators in the generator pool and the oracle; per-eliminator derivation proofs (CASE, IF, LISTCASE rules); a `Sum<Resource<R>,I64>` eliminator control exposing the selected payload at its own type while the sum stays non-`Data`; negatives for an advertised branch refinement and an implicit union join |
| B-CHECK-07 request binding, limits, boundary and exhausted cases | Covered: LIM-* and OUT-* families with per-limit boundary and exhausted controls (`verification/m2-fragment.md` "Limits and outcomes"; VT-M2-01) | Retained; any new v1 path (recursion, schema, cycle checks) declares and charges its limits with the same boundary/exhausted control pair |

### Decision: Complete the word table to every bootstrap word

**Choice:** Fragment v1's environment table is exactly the 23 entries below, each with a per-word contract row, at least one positive and one rejection control, a property-pool entry, an independent oracle arm, and a per-word theorem obligation.
`=` is added to the kernel table (`S I64 I64 -- S Bool ! {}`, effect-free) — it is part of the bootstrap numeric vocabulary (`.cairn/specs/language/spec.md` section 4.4, K-NUM-01 paragraph: "`=` in the bootstrap numeric vocabulary compares two `I64` values and returns `Bool`") and is the only bootstrap word missing from the M2 table.

**Rationale:** B-SCOPE-01 items 4-5 fix the word scope ("the bootstrap stack/data/control words, wrapping arithmetic, `quote`, `compose`, `run`, and `reflect`", plus the supplied `test.emit`); the per-word contracts live in the language specification sections cited below.
The M2 harness already proved the value of full-pool differential coverage — it found two real substitution bugs (`verification/m2-acceptance-run.md` "Kernel bug found by the property harness").

**Word table plan**

| Word | Contract source (`.cairn/specs/language/spec.md`) | v0 kernel | v0 pool | v0 oracle arm | v1 duty |
|---|---|---|---|---|---|
| `dup` | 7.1 | yes | yes | yes | keep + theorem |
| `drop` | 7.1 | yes | yes | yes | keep + theorem |
| `swap` | 7.1 | yes | yes | yes | keep + theorem |
| `dip` | 7.1 | yes | yes | yes | keep + theorem |
| `+` `-` `*` | 4.4 (K-NUM-01) | yes | `+` only | yes (shared arm) | pool `-`/`*`; per-word theorems |
| `=` | 4.4 (K-NUM-01 paragraph) | **no** | no | no | add to kernel, pool, oracle, controls, theorems |
| `quote` | 6.5 | yes | yes | yes | keep + theorem |
| `compose` | 6.4 | yes | yes | yes | keep + theorem |
| `run` | 6.3 | yes | yes | yes | keep + theorem |
| `reflect` | 11.3 | yes | yes | yes | keep + theorem |
| `unit` | 7.2 | yes | yes | yes | keep + theorem |
| `pair` | 7.2 | yes | yes | yes | keep + theorem |
| `unpair` | 7.2 | yes | yes | yes | keep + theorem |
| `inl` | 7.2 | yes | yes | yes | keep + theorem |
| `inr` | 7.2 | yes | yes | yes | keep + theorem |
| `case` | 7.2 | yes | **no** | **no** | pool + oracle arm + controls + theorem |
| `if` | 7.3 | yes | yes | yes | keep + theorem |
| `nil` | 7.4 | yes | yes | yes | keep + theorem |
| `cons` | 7.4 | yes | yes | yes | keep + theorem |
| `list.case` | 7.4 | yes | **no** | **no** | pool + oracle arm + controls + theorem |
| `test.emit` | B-SCOPE-01 item 5 | yes | yes | yes | keep + theorem |

(v0 columns verified against `crates/noble-kernel/src/contracts/bootstrap/data.rs` `table()`, `crates/noble-kernel/tests/property/fit.rs` `WORDS`, and `crates/noble-kernel/tests/property/table.rs` `word_face`; the pool's 19 entries additionally include the `test.counter` resource-maker fixture, which stays as a negative-eligibility fixture and is not a language word.)
The per-word scheme contract rows, control fixtures, and doc examples are recorded in `verification/m3-fragment.md`, the fragment v1 definition that supersedes `verification/m2-fragment.md` for v1 while citing it.

### Decision: Acceptance remains witness verification, now with explicit cycle and recursion rejection

**Choice:** Candidates still carry total instantiation witnesses verified against environment schemes (M2's instantiation-verification decision stands).
Fragment v1 adds rejection paths, not inference: environment data containing a recursive definition dependency or a user-declared recursive schema yields the unsupported outcome with a diagnostic naming the identity (B-CHECK-02); a substitution witness that refers to itself, directly or through a chain, yields the invalid outcome (B-CHECK-05), checked by an explicit bounded well-foundedness walk that charges work.
Both rejections issue no candidate-body host request and leave session state unchanged (B-RESULT-02).

**Rationale:** K-CHECK-02/03 keep inference and the solver algorithm as follow-up work; B-CHECK-02/05 demand explicit rejection, not silent scope; B-SCOPE-02 forbids fallback. The M2 malformed lane already rejects dangling and `u32::MAX` node references, so the walk extends an existing discipline.

### Decision: Per-rule and per-word proof depth in a new proof root

**Choice:** `proofs/m3/` reuses the M2 Lake pattern (pinned backend, `NobleKernel` generated module bound to source and inventory identities, `NobleM3` reference modules) and states the fragment v1 theorem set:

1. **Per-word scheme refinement** — for each of the 23 table entries `w`: the extracted instantiation path applied to `w`'s scheme and a well-kinded total witness yields exactly the reference instantiation's interface (extends `applyScheme_ok`, `verification/m2-proof-evidence.md` section 3).
2. **Per-rule soundness** — for each rule family `R` in {EMPTY, LITERAL, WORD, SEQUENCE, QUOTATION, CASE, IF, LISTCASE}: acceptance through `R`'s checker path implies well-formedness and a `TypingDerivation` whose corresponding rule instance appears at that node (V-CHECK-03 shape; refines `foldBody_derives` and `check_soundness` from whole-body to per-rule labels).
3. **Decision-path correspondences** — `eligibility_iff` (the extracted eligibility guard decides the declarative `DataOk`, per constrained word) and `inclusion_iff` (the extracted inclusion decides the declarative subset), extending `dataOkAt_iff` and `subsetOf_subset`; branch-union conservativity lemmas for the three eliminators (K-EFFECT-01/02).
4. **Termination and work** — `check_total_v1` and `foldBody_work_bounded_v1` over fragment v1 including the new rejection walks, with per-word cost positivity (PO-10, V-CHECK-05).
5. **Per-word positive coverage** — `coverage_positive_word` instantiated for every table entry: an accepted, derivable fixture exists per word (extends `coverage_positive` beyond the single arithmetic fixture).
6. **Refinement matrix** — per-word and per-rule representative-input agreements between the extracted `acceptance.check` (and the separately proved decision paths) and the reference `check`, plus rejection agreements for the new B-CHECK-02/05/06 negatives; `accepted_via_refinement` is retained and generalized to the v1 statement.
7. **Duplication and freshness** — a duplicated first-class program value shares one instantiated interface at both the extracted and reference levels (B-CHECK-03, K-CHECK-01).

**Rationale:** PO-11 requires the Aeneas route for the actual checker; VT-AENEAS-04 requires the theorem to bind generated constants; V-CHECK-05 requires coverage and termination per the advertised supported language, which after this change is the full word table. The M2 `proved` count of 1 makes per-rule depth the measurable growth axis: the fragment v1 coverage record must classify every acceptance-path decision function as proved, modeled with disclosure, or excepted with disclosure — never silently open (VT-SCOPE-05).

### Decision: Same route, same bindings, extended gates

**Choice:** Extraction stays the pinned Charon → Aeneas → Lean route with `-split-files` externals and the computable generated module (`verification/m2-scope-review.md` item 2); the proof-required gate, coverage gate, and refusal matrix are **extended in place** (`verification/m3-proof-gate.sh`, `verification/m3-coverage-gate.sh` succeed `verification/m2-proof-gate.sh` and `verification/m2-coverage-gate.sh`): the required-theorem list grows to the v1 set, the coverage classification re-derives from the regenerated inventory, and the refusal matrix adds mutations for a missing per-word theorem, a missing per-word coverage case, a removed eliminator oracle arm, and a candidate-schema revision regression.
No M2 gate is deleted or weakened; the M2 gates keep passing until the v1 gates supersede them at archive time.

**Rationale:** VT-CI-05 requires regeneration and compiler-derived comparison on change; VT-M2-04 established the refusal discipline; forking gates would duplicate controls rather than extend them (the mission's same-route constraint).

### Decision: The property harness and oracle cover the full pool

**Choice:** The generator pool becomes all 23 entries plus the resource-maker fixture; the oracle gains independent arms for `case` and `list.case` written from the documented contracts only (own type mirror, own join logic, no kernel decision code — the M2 oracle discipline in `crates/noble-kernel/tests/property/oracle.rs`'s header comment).
The agreement and malformed lanes keep their recorded seeds, bounds, shrinker, and revisions (DX-PROPERTY-01/02); the doc-example suite gains per-word `noble-check` examples with at least one rejection per constrained word (DX-DOC-01/02); the DX-01 diagnostic controls extend to the new rejection families (DX-DIAG-01, B-DIAG-01).

**Rationale:** The M2 harness found the substitution walk bugs precisely because its pool was broad; the two missing eliminators are the highest-value ungenerated shapes (two consumed program values each, union bounds, payload exposure).

### Decision: Evidence, ledger, and disclosure honesty

**Choice:** `specs/verification/obligations.json` entries for PO-09/PO-10/PO-11 move to the fragment v1 claims only when the v1 theorem targets build with bound evidence; every other obligation stays open.
`specs/STATUS.json` component fields move only with that evidence.
The two M2 disclosures are handled explicitly: the reference model either gains provenance (closing the `eraseProvenance` erasure) or re-discloses it unchanged; the `charon::opaque` excepted set is re-derived from the regenerated inventory and every new entry carries its justification (`verification/m2-scope-review.md` "Explicitly not claimed").

**Rationale:** EV-BIND-01, EV-TIER-01, V-CLAIM-01, V-EVIDENCE-04, VT-AENEAS-05; the M2 pattern of flag-and-record moving together.

### Decision: Octet deny-all compliance carries over unchanged

**Choice:** Every new kernel function follows the M2 catalog discipline (300-line modules, bounded loops, no unwrap/expect/panic, exhaustive matches, value-passing state, explicit capacity); the architecture catalog phase stays at 0 findings; test-file style warnings stay outside the kernel catalog scope as in M2.

**Rationale:** VT-OCTET-01..03 and the M2 style decision; the Aeneas subset and the catalog agree on the iterative value-passing shape.

## Requirement Traceability

The [delta](specs/verification-toolchain/spec.md) contains every new task-linked requirement (VT-M3-01 through VT-M3-04).
The audit table above preserves B-CHECK-01..07; the following map fixes the remaining touchpoints:

| Requirement | Where the change satisfies it |
|---|---|
| B-CHECK-02 | Recursive-definition and recursive-schema rejection controls (task 2.2) |
| B-CHECK-03 | Per-rule refinements and the duplication control (tasks 5.2, 2.3) |
| B-CHECK-05 | Cyclic-substitution rejection and work-charged walks (task 2.2) |
| B-CHECK-06 | Eliminator pool/oracle coverage, payload-exposure and union-join controls, per-eliminator proofs (tasks 2.3, 3.1, 5.2) |
| B-SCOPE-01/02 | The complete word table and explicit unsupported outcomes (tasks 1.2, 2.1) |
| V-CHECK-03/04/05 | The v1 theorem set's soundness, outcome, termination, and coverage families (tasks 5.2-5.4) |
| VT-AENEAS-01/03/04 | Same-route extraction, regeneration, and refinement over generated constants (tasks 4.1-4.3) |
| VT-SCOPE-05, VT-CI-05 | Extended per-function classification and refusal gates (tasks 6.1-6.2) |
| VT-M2-01..04 | Every v0 control re-run over v1; no M2 gate weakened (tasks 6.1, 7.1) |
| DX-PROPERTY-01/02, DX-DOC-01/02, DX-DIAG-01, B-DIAG-01 | Full-pool harness, per-word examples, extended diagnostics (tasks 3.1-3.3) |

## Planned Artifacts

- `crates/noble-kernel/src/`: the `=` contract entry; the recursion/schema/cycle rejection paths; any v1 schema field.
- `crates/noble-kernel/tests/`: extended acceptance, fragment, property (`fit.rs` pool, `table.rs` arms), docexamples, dx01 controls.
- `proofs/m3/`: Lake root, `NobleM3` reference modules (Words v1, Judgment with the three eliminator rules and the rejection judgments, per-rule soundness files, Termination, Coverage, Refinement), proof README.
- `verification/`: `m3-fragment.md`, `m3-extraction-probe.md`, `m3-coverage.json`, `m3-proof-gate.sh` (+ refusals), `m3-coverage-gate.sh` (+ refusals/join tooling succeeded from the M2 pair), `m3-proof-evidence.md`, `m3-acceptance-run.md`, `m3-scope-review.md`.
- `nix/` and `policy/source-inventory.ncl`: extended extraction subject and classification.
- `.pi/m3-*` retained evidence roots outside the worktree.

## Verification Plan

### Planning-package checks

Run the Bun document self-tests (`bun tools/cairn-specs.mjs --self-test`, `bun test tools/cairn-specs.test.mjs tools/check-specs.test.mjs`, `bun tools/check-specs.mjs --self-test`), then the Cairn gates with the explicit worktree root: `validate`, `gate proposal m3-checker-coverage`, `gate design m3-checker-coverage`, `gate tasks m3-checker-coverage`.

### Implementation checks

The workspace exposes, in addition to the M2 checks:

- `check-count` v1 corpus: every per-word positive fixture yields a derivation record; every rejection fixture yields its named outcome and diagnostic.
- `recursion-schema-cycle`: the B-CHECK-02/05 negative matrix (recursive definition dependency, user-declared recursive schema, self- and mutually-referential substitution witnesses) with boundary and exhausted companions.
- `eliminators`: `case`/`list.case`/`if` positive, unequal-join, union-bound, payload-exposure, implicit-union-join, and advertised-refinement controls.
- `property-v1`: the agreement lane over the full pool (recorded seeds, bounds, 0 disagreements) and the malformed lane (0 accepted, 0 panics).
- `docexamples-v1`: per-word executed examples with the illustrative control.
- `extraction-m3`: regeneration under the pinned pair with the record re-bound to source, inventory, selection, and tool identities.
- `proof-m3`: the v1 theorem set builds under the pinned Lean with no `sorry`, the axiom policy enforced, and every refinement subject a generated constant.
- `coverage-m3`: per-function classification of the regenerated inventory with separate extracted/proved/modeled/excepted/open statuses.
- Refusal matrix: the M2 mutations plus a missing per-word theorem, a missing per-word coverage case, a removed oracle arm, and a schema-revision regression each reject.

### Acceptance sequence

Baseline and package gates; fragment v1 definition and control map; kernel extension with tests; harness/oracle/doc extension; extraction re-binding; reference model and the v1 theorem set; gate extension and refusals; full control matrix; evidence records and scope review; then sync, archive, commit, push, and integration.

## Risks / Trade-offs

- **Extraction breakage from new kernel paths.** The rejection walks and the `=` entry must stay inside the confirmed Aeneas subset; the M2 probe-ladder pattern (`verification/m2-extraction-probe.md`) is re-run before proof work, and a failed probe leaves the change active with its diagnostic rather than weakening a gate.
- **New harness finds.** Full-pool generation over the eliminators may surface kernel bugs as it did twice in M2; the budget includes repair, regression controls, and re-extraction, and a find is a result, not a failure of the change.
- **Proof volume.** Per-word and per-rule theorems multiply fixtures and embedding lemmas; per-word lemma files bound file size, and evaluation-closed proofs keep their per-declaration `native_decide` axioms enumerated by the gate (the M2 axiom-policy precedent).
- **Provenance disclosure.** Closing `eraseProvenance` requires reference-model diagnostics; if the cost exceeds the milestone, the erasure is re-disclosed verbatim rather than silently kept.
- **Opaque growth.** Any new `#[charon::opaque]` exception must carry its VT-SCOPE-05 justification; the preferred path is writing the helper in the extractable subset.
- **Shared state.** `specs/STATUS.json` flags and the obligation ledger move only with the v1 evidence; nothing else is touched.
- **Scope creep into inference.** The cycle and recursion families are rejection paths only; any pull toward unification is out of scope by the decision above.

## Rollout and Recovery

Implementation proceeds in task order: audit and fragment v1, kernel extension, harness and DX extension, extraction re-binding, proofs, gates, acceptance.
A failed stage leaves the change active with its exact diagnostic and configuration; no gate is weakened to make progress, and the M2 gates remain the floor until the v1 gates pass.
