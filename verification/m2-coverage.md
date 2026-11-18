# M2 per-function coverage classification gate (task 3.3)

**Historical task 3.3 status: complete for its recorded extraction, not
current-source evidence after #97.** Every function of the recorded M2 kernel
extraction subject is
classified `extracted`, `proved`, `fixture-checked`, `modeled`, `excepted`,
or `open` in the reviewed record `verification/m2-coverage.json`; the gate
`verification/m2-coverage-gate.sh` joins that record with the authoritative
function list derived from the extraction output, enforces consistency in
both directions, and refuses five mutation classes
(`verification/m2-coverage-refusals.sh`). The original run was green with
all five refusals demonstrated; it is not a fresh run for the selected
`1238879f` extractor. Nothing under `proofs/`,
the Rust crate, or the existing gates was modified; only the four new
`verification/` files listed in §7 exist.

Since 2026-11-05 (#88) every citation also carries its claim strength, and
`noble_kernel.acceptance.check` is `fixture-checked`, not `proved`: see §8.
The refusal matrix now has seven mutation classes and one positive control.
For #97 the M2 and M3 coverage JSON records were re-derived against the new
checked-in generated definition lists; that *static* join does not establish
their proof gates, source-to-generated correspondence, or current-Rust
applicability. See [the #97 classification](aeneas-re-extraction/README.md#generated-subjects-and-lane-receipts).
The whole-crate re-extraction of 2026-11-16 (§9) recorded the then-current
generated subject, and that run's record covered it. That subject was
regenerated once more on 2026-11-18; §10 records the subject sync, the
re-derived record and the current gate run (§9 is historical).

## 1. Binding source for the function list

The authoritative list is **the defs of the generated
`proofs/m2/NobleKernel/Funs.lean` plus the reviewed external surface
`NobleKernel/FunsExternal.lean`** — each entry carrying its Rust-side name
(doc header `[rust::path]:` or `rust_fun`/trait-impl attribute) and its
`Source: 'path', lines X:Y-X:Z` span — cross-checked against the
**elaborated environment** (§2, ENV-BINDING).

Why this source binds:

- The LLBC/`translation.json` are intermediate pipeline artifacts. They are
  not checked into the current tree (only stale M1 scratch copies exist
  under `.pi/`), so they cannot bind anything about `proofs/m2` today.
- `Funs.lean`/`FunsExternal.lean` are what `import NobleKernel` actually
  elaborates and what every theorem in `NobleM2.Refinement` is stated
  about. A function that exists only upstream (LLBC) but not here proves
  nothing; a function here but not upstream cannot occur without the
  generated file changing, which the join detects.
- The environment cross-check closes the text↔environment gap: every
  classified constant must exist as a definition *in its generated module*
  in the elaborated environment, and the only unclassified definitions left
  in those modules must be elaborator-generated matchers (96 of them:
  `match_N`/`match_N.splitter`, `_sparseCasesOn_1`, `_unsafe_rec`). A
  record entry naming a constant that does not elaborate, or a def in the
  text the parser misses (keyword-parity guard), both reject.

Scope note: type declarations and the 8 type-alias defs in
`Types.lean`/`TypesExternal.lean` are type-level, carry no Rust function
identity, and stay with the extraction/proof gates; this record owns
*function* declarations only.

## 2. What the gate enforces

| Check | Named failure | Meaning |
|---|---|---|
| C0 SETUP/TOOLCHAIN | `SETUP`, `TOOLCHAIN` | `bun` present (the repo's checks already run on it); pinned Lean 4.31.0 on PATH |
| C1 BUILD | `BUILD` | `lake build NobleM2 NobleM2.Refinement NobleKernel` green (the env checkers elaborate against these oleans) |
| C2 COVERAGE-DERIVE | `COVERAGE-DERIVE`, `PARSE-PARITY` | function list derived from the two generated files; parsed count == top-level `def`/`impl_def` keyword count |
| C3 COVERAGE-JOIN | `UNCLASSIFIED`, `STALE-ENTRY`, `STATUS`, `STATUS-PLACEMENT`, `DRIFT`, `RECORD-FORM`, `CITATION-FORM`, `STRENGTH-STATUS` | every generated function classified exactly once; every non-`open` entry generated; status vocabulary; `extracted`/`proved`/`fixture-checked` live in `Funs.lean`, `modeled`/`excepted` in `FunsExternal.lean`; citations exactly on `proved` and `fixture-checked`, each `{theorem, strength, axioms}`; `proved` only with a strict universal citation, `fixture-checked` only without one; record fields restate the generated facts (kind/rust/file/line/span/loop_body/trait_impl) |
| C4 EXCEPTED-DISCLOSURE | `EXCEPTED-DISCLOSURE` | the `excepted` set is exactly the crate's `#[charon::opaque]` declarations (count and short names, extracted from `crates/noble-kernel/src`) |
| C5 OPEN-SUBJECT | `OPEN-ENTRY`, `OPEN-SUBJECT` | `open` entries may only name subject functions missing from the output, and must exist as Rust functions in the crate; a generated function marked `open` rejects |
| C6 ENV-BINDING/CITATIONS | `CONSTANT-MISSING`, `CONSTANT-KIND`, `MODULE`, `ENV-UNCLASSIFIED`, `CITATION`, `CITATION-BINDING` | two scratch Lean checkers (proof-gate mechanics): every entry exists as a definition in its generated module; residual defs are matchers only; every citation exists, is a theorem, and its *statement mentions the classified constant* (semantic subject binding, as in the proof gate's GATE-B) |
| C7 STRENGTH | `STRENGTH-DRIFT`, `STRENGTH-COUNT` | each checker classifies every citation it verifies from the elaborated statement and its transitive axioms (`verification/coverage-strength.lean`); the record must restate exactly those strengths, and every cited theorem must be classified exactly once |
| C8 SUMMARY | — | per-status counts and per-strength citation counts; 100% classification required |

The two scratch checkers exist because the root module `NobleM2` and
`NobleM2.Refinement` cannot be imported together (both elaborate
`deriving`-generated instances with identical names, e.g.
`NobleM2.UnsupportedKind.ofNat` from `NobleM2.Termination`) — the same
diamond the proof gate solves with GATE_A/GATE_B. Checker A imports
`NobleKernel` + `NobleM2` (constants, residual, non-Refinement citations);
checker B imports `NobleM2.Refinement` (the refinement family). Note that
the `NobleM2` root does **not** transitively import `NobleKernel`, so A
imports it explicitly.

## 3. Classification summary (historical run)

| Status | Count | Population |
|---|---|---|
| `extracted` | 448 | generated defs in `Funs.lean` minus the proved one: crate functions, 41 Aeneas loop-body defs (`*_loop*.body`, bound to their parent loop's span), 129 trait-implementation defs and 18 `impl_def` trait instances, and the `WORK_CAP`/`SEMANTIC_REVISION`/`CANDIDATE_FORMAT`-style const items |
| `proved` | 1 | `noble_kernel.acceptance.check` — cited by the 13 refinement-family theorems (the 12 fixture agreements + `accepted_via_refinement`), each verified to *mention the extracted constant in its statement* |
| `modeled` | 17 | the external std surface implemented in `FunsExternal.lean`: `core::cmp` 1, `core::convert` 2, `core::fmt` 2, `core::num` 1, `core::option` 3, `core::result` 3, `alloc::vec` 5 |
| `excepted` | 5 | the crate's `#[charon::opaque]` disclosures, each with a faithful total model in `FunsExternal.lean`: `types::impls::clone_stack`, `types::impls::debug_stack`, `shapes::impls::clone_parts`, `shapes::impls::debug_parts`, `shapes::impls::debug_slots` |
| `open` | 0 | none — see §5 |
| **total** | **471** | `Funs.lean` 449 + `FunsExternal.lean` 22 |

Coverage: 471/471 = 100% of generated functions classified; the
`Funs.lean` entries span 21 crate source files under
`crates/noble-kernel/src` (plus 2 rustc/core macro/option paths).

These counts and the outputs in §4 and §6 are the 2026-09-18 record.
Later records, including the #97 re-derived map, have different function
counts and count `acceptance.check` as `fixture-checked` (§8). The checked-in
record is the only reviewed count for a new gate run.

## 4. Commands and outputs

Gate (from any directory; pinned Lean first on PATH; `bun` required):

```sh
PATH=/nix/store/hbxwgcbpkyqllqzq7cidhnvhgb60jb9q-noble-lean-release-4.31.0/bin:$PATH \
  verification/m2-coverage-gate.sh
```

Passing output (2026-09-18, run from a foreign cwd):

```
[m2-coverage] PASS SETUP (bun + lean 4.31.0)
[m2-coverage] PASS BUILD (lake build NobleM2 NobleM2.Refinement NobleKernel)
[m2-coverage] PASS COVERAGE-DERIVE (Funs.lean + FunsExternal.lean defs with rust names and source spans; keyword parity holds)
[m2-coverage] PASS COVERAGE-JOIN (total=471 classified=471 extracted=448 proved=1 modeled=17 excepted=5 open=0)
[m2-coverage] PASS COVERAGE-JOIN (record and generated surface agree)
[m2-coverage] PASS EXCEPTED-DISCLOSURE (5 excepted entries == #[charon::opaque] declarations in the crate)
[m2-coverage] PASS OPEN-SUBJECT (0 open entries; every generated function is classified)
info: m2-coverage: ENV-A-PASS 471 classified constants bound to their modules; 96 elaborator matchers tolerated
info: m2-coverage: ENV-B-PASS 13 refinement citations verified (theorem exists, statement mentions the constant)
[m2-coverage] PASS ENV-BINDING (constants bound in the elaborated environment; citations are theorems mentioning their constants)
[m2-coverage] SUMMARY total=471 classified=471 extracted=448 proved=1 modeled=17 excepted=5 open=0
[m2-coverage] PASS (all checks) — subject: /home/brittonr/git/noble-m2-checker/proofs/m2; record: /home/brittonr/git/noble-m2-checker/verification/m2-coverage.json
```

Refusal matrix (same PATH; ~80 s, dominated by rebuilding the mutated
`Funs.lean` in a scratch copy):

```sh
PATH=/nix/store/hbxwgcbpkyqllqzq7cidhnvhgb60jb9q-noble-lean-release-4.31.0/bin:$PATH \
  verification/m2-coverage-refusals.sh
```

The baseline passes first, then five mutated scratch copies (proof-root
copies share `.lake/packages` via hard links with real `.lake/build`;
record copies are mutated with `bun`; the repository tree is never
touched):

```
== mutation: unclassified-def (expected refusal: UNCLASSIFIED) ==
[m2-coverage] FAIL COVERAGE-JOIN: derived functions and reviewed record disagree (see named problems above)
[m2-coverage] FAIL UNCLASSIFIED: noble_kernel.types.impls.coverage_probe (noble_kernel::types::impls::coverage_probe, Funs.lean:8025) is generated but has no record entry
refused at UNCLASSIFIED (gate exit 1)

== mutation: dangling-citation (expected refusal: CITATION) ==
[m2-coverage] FAIL ENV-BINDING: environment/citation check reported violations (see named failures above)
.m2-cov-env-b.lean:20:0: error: m2-coverage: FAIL CITATION: NobleM2.Refinement.refinement_ghost_theorem cited for noble_kernel.acceptance.check does not exist
refused at CITATION (gate exit 1)

== mutation: misbound-citation (expected refusal: CITATION-BINDING) ==
[m2-coverage] FAIL ENV-BINDING: environment/citation check reported violations (see named failures above)
.m2-cov-env-a.lean:31:0: error: m2-coverage: FAIL CITATION-BINDING: NobleM2.check_total does not mention noble_kernel.acceptance.check in its statement
refused at CITATION-BINDING (gate exit 1)

== mutation: stale-entry (expected refusal: STALE-ENTRY) ==
[m2-coverage] FAIL COVERAGE-JOIN: derived functions and reviewed record disagree (see named problems above)
[m2-coverage] FAIL STALE-ENTRY: noble_kernel.types.ghost_function is classified in the record but not generated (stale entry)
refused at STALE-ENTRY (gate exit 1)

== mutation: bad-status (expected refusal: STATUS) ==
[m2-coverage] FAIL COVERAGE-JOIN: derived functions and reviewed record disagree (see named problems above)
[m2-coverage] FAIL STATUS: noble_kernel.types.EffSet.empty: invalid status "reviewed" (must be one of extracted|proved|modeled|excepted|open)
refused at STATUS (gate exit 1)

== all refusals demonstrated ==
```

The two task-required mutations are `unclassified-def` (a plausible
generated function spliced into `Funs.lean` before its final
`end noble_kernel`, keeping the tree building) and `dangling-citation`
(a cited theorem renamed in the record). The other three came free:
`misbound-citation` cites a *real* theorem about a different subject
(`NobleM2.check_total`, the reference model's totality theorem) and is
caught by the statement-mention binding, not by existence; `stale-entry`
and `bad-status` pin the record's shape.

## 5. Hand spot-checks and judgment calls

Spot-checked by hand against the Rust sources (five entries, all exact):

| Record entry | Status | Span in record | Rust source says |
|---|---|---|---|
| `noble_kernel.acceptance.check` | proved | `crates/noble-kernel/src/acceptance.rs 45:0-57:1` | `pub fn check(` at line 45, closing at 57 |
| `noble_kernel.types.EffSet.empty` | extracted | `crates/noble-kernel/src/types.rs 34:4-36:5` | `pub fn empty()` at 34–36 |
| `noble_kernel.words.subst.walk_step` | extracted | `crates/noble-kernel/src/words/subst.rs 86:0-111:1` | `fn walk_step(` at 86 |
| `noble_kernel.types.impls.push_type_program_loop0.body` | extracted | `crates/noble-kernel/src/types/impls.rs 70:4-73:5` | the `while` loop body of `push_type_program`, 70–73 |
| `noble_kernel.acceptance.parts.site` | extracted | `crates/noble-kernel/src/acceptance/parts.rs 24:0-29:1` | `fn site` at 24 |

plus the five `excepted` entries, each hand-verified to sit directly under
a `#[charon::opaque]` attribute (and machine-verified by C4).

Judgment calls, stated honestly:

- **`excepted` vs `modeled` for the five opaque helpers.** They carry both
  a crate-level disclosure (`#[charon::opaque]`, still present in the Rust
  source) *and* a faithful model in `FunsExternal.lean`. They are
  classified `excepted` — the in-crate, attribute-declared exclusion from
  extraction is the salient fact — with the model noted on each entry.
  `modeled` is reserved for the external-to-the-crate std surface, which
  was never an extraction target. Under the alternative reading (count by
  where the implementation lives) the five would move to `modeled`
  (22/0 instead of 17/5); nothing else changes. The gate binds the
  `excepted` set to the actual crate attributes either way.
- **`open` is empty by construction, not by fiat.** The record's key set
  must equal the generated set, and a *generated* function marked `open`
  rejects (`OPEN-ENTRY`); `open` is the reviewed bucket for subject
  functions missing from the extraction output (the M1 inventory's
  pre-extraction state), which today do not exist — the strict toolrun
  probe reports every kernel function translated. Upstream Rust-side
  accounting (the compiler-derived 490 production subjects) stays with
  the source-inventory control; it is a different grain (monomorphization,
  trait impls, closures, and loop bodies map many-to-many across the
  pipeline), and grafting that join onto this gate would trade exactness
  for a fuzzy match.
- **Discrepancy noted, not silently absorbed:** the acceptance run
  ([m2-acceptance-run.md](m2-acceptance-run.md)) lists *six* opaque
  assumptions including `shapes::impls::clone_slots`. The crate declares
  five (`clone_parts`, `clone_slots` is *not* among them), and
  `clone_slots`/`clone_slots_loop`/`…_loop.body` are in fact extracted in
  `Funs.lean` (Aeneas functionalized that loop outside the problematic
  mutual block). This record follows the crate attributes and the
  generated surface: 5 `excepted`. The prose count in the older document
  is stale; fixing that document is outside this task's file scope.
- **Only `acceptance.check` is `proved`.** The 13 refinement-family
  theorems bind the extracted checker end-to-end (outcome-level agreement
  on the twelve fixtures plus the acceptance handoff), which is exactly
  what "proved" means here; they do not constitute per-function lemmas for
  the other 448 extracted functions, and no such lemmas are claimed. The
  reference-model theorems (`applyScheme_ok`, `check_soundness`,
  `check_total`, `coverage_positive`, `Coverage.*`, `Regression.ce_*`)
  bind `NobleM2` reference functions — outside the extraction subject —
  and remain owned by the proof gate
  ([m2-proof-evidence.md](m2-proof-evidence.md)).

## 6. Maintaining the record

`m2-coverage.json` is reviewed data, not regenerated silently. After a
regeneration that changes the generated surface:

```sh
bun verification/m2-coverage-join.mjs emit proofs/m2 > /tmp/skeleton.json
```

produces a fresh skeleton with `status: "REVIEW"`; review sets the
statuses (and citations for `proved`) and replaces the record — the gate's
DRIFT/UNCLASSIFIED/STALE-ENTRY checks force exactly this on any drift, and
refuse a record that merely re-states old facts over a new surface.

When the generated surface changes but the classification does not, the
2026-11-18 re-derivation (§10) instead rebuilds each entry from
`derive(PROOF_ROOT)` — the same parser the gate runs — and carries the
reviewed `status`/`citations`/`note` fields over *by function name*: every
surviving name keeps its reviewed classification, a name that disappeared
must be dropped (the join reports it as `STALE-ENTRY` otherwise), and a new
generated name must be reviewed and classified explicitly. The derived
`kind`/`rust`/`source`/`file`/`line` fields are never carried over, so the
record cannot restate a span the new output no longer has.

## 7. Files added

- `verification/m2-coverage-gate.sh` — the gate (§2).
- `verification/m2-coverage-join.mjs` — the parser/join tool (derive,
  emit, check, leanpayload modes; keyword-parity guard).
- `verification/m2-coverage.json` — the reviewed classification record
  (471 entries; `subject`, `record_version`, per-entry lean/kind/rust/
  file/line/source/loop_body/trait_impl/status/citations/notes).
- `verification/m2-coverage-refusals.sh` — the five-cell refusal matrix.
- `verification/m2-coverage.md` — this evidence record.

Dependencies: pinned Lean 4.31.0 (as every proof gate) and `bun`
(already required by the repository's `noble-documents` check). Scratch
files (`.m2-cov-*`) are written inside the proof root during a run and
removed on exit.

## 8. Claim strength (2026-11-05, #88)

A `proved` label counted in totals must not rest on statements weaker than
readers assume. Every citation is now `{theorem, strength, axioms}`, and the
gate's two Lean checkers compute both fields from the compiled environment
(`verification/coverage-strength.lean`, spliced after their imports), never
from source text:

| Strength | Statement shape |
|---|---|
| `conditional` | some hypothesis is an equation that mentions the classified constant: the theorem assumes the refinement it is cited for |
| `fixture` | otherwise, no universally quantified non-proof input: a closed statement about one request and candidate |
| `universal` | otherwise: it quantifies over inputs with no such hypothesis |

`axioms` is `native` when a native-evaluation axiom (a per-declaration
`native_decide` auxiliary, `Lean.ofReduceBool` or `Lean.trustCompiler`) is in
the transitive axiom set, else `strict`. The join
(`verification/coverage-strength.mjs`) then requires a strict universal
citation for `proved`; anything else is `fixture-checked`, which the totals
never count as proved. Every cited theorem must be classified exactly once
(`STRENGTH-COUNT`), and a record that restates a strength the elaborated
statement does not have refuses (`STRENGTH-DRIFT`).

Current gate run (2026-11-05):

```
[m2-coverage] PASS COVERAGE-JOIN (total=515 classified=515 extracted=491 proved=0 fixture-checked=1 modeled=18 excepted=5 open=0)
[m2-coverage] PASS STRENGTH (citations=13 fixture=12 conditional=1 universal=0 strict=1 native=12 strict_universal=0)
[m2-coverage] NON-CLAIM strength classes describe statement shape only; they do not show that the reference model is correct
```

`noble_kernel.acceptance.check` is therefore `fixture-checked`: its twelve
`refinement_*` citations are closed `native_decide` fixtures, and
`accepted_via_refinement` is conditional on the refinement equation
`project_result (acceptance.check …) = NobleM2.check env req cand`, which no
theorem proves.

Refusal matrix additions, all demonstrated on the current tree:

| Cell | Mutation | Refused at |
|---|---|---|
| `native-fixtures-as-proved` | `acceptance.check` marked `proved` while every citation is a fixture or a conditional handoff | `STRENGTH-STATUS` |
| `conditional-claimed-universal` | `accepted_via_refinement` recorded as `universal` and the function as `proved` | `STRENGTH-DRIFT` (the elaborated statement is conditional) |
| `universal-probe` (positive control) | a synthetic `∀ e : EffId, clone e = ok e` theorem cited for the toy extracted `EffId` clone | accepted: classified universal/strict, `proved=1` |

Strength classes describe statement shape only. They do not show that
`NobleM2.check` is correct, and a `universal` theorem about the reference
model would still need the extracted function in its statement (the existing
`CITATION-BINDING` check) to count for that function.

## 9. Whole-crate re-extraction (#97, 2026-11-16)

**Historical: this section records the 2026-11-16 subject and its counts.
That subject was superseded on 2026-11-18 (the `checked_i64_environment`
closure adapters became the direct helpers `binary`/`unary`); §10 records
the current bytes, the re-derived record and the gate run on them.**

The checked-in `proofs/m2/NobleKernel/{Funs,Types}.lean` are now the
whole-crate extraction of `crates/noble-kernel` (Charon `9e95cf31` with
`--inline-anon-consts`, Aeneas `1238879f`; Rust subject `acf98da0`),
identical to `proofs/m3/NobleKernel`. The record was re-derived from the
emitter skeleton (§6) with these classifications:

| Status | Count | Population |
|---|---|---|
| `extracted` | 2268 | every generated def in `Funs.lean` except `acceptance.check` |
| `fixture-checked` | 1 | `noble_kernel.acceptance.check`: the same 13 citations as before (12 closed fixtures, the conditional `accepted_via_refinement` handoff) |
| `modeled` | 69 | the template's 66 external std functions in `FunsExternal.lean`, plus `Option::map`, `Result::unwrap_or` and `Formatter::debug_c_like_enum_write_str`, kept for the MC1/M3-Wasm `StdModels` that delegate to them (the generated `map`/`unwrap_or` calls resolve to these copies) |
| `excepted` | 5 | the crate's five `#[charon::opaque]` helpers, unchanged |
| `open` | 0 | |
| **total** | **2343** | `Funs.lean` 2269 + `FunsExternal.lean` 74 |

Every citation's axiom class is now `native` (historical: §11 restores
`strict` for `accepted_via_refinement`): the generated checker itself
depends on three literal-size evaluation axioms that the pinned Aeneas
`toStr` emits for string literals of the generated code (see
[the M2 proof evidence §8](m2-proof-evidence.md)), so `accepted_via_refinement`
moved from `conditional`/`strict` to `conditional`/`native`.

Two gate adjustments, both forced by the new subject:

- **Residual definitions.** The generated code has four `mutual … end`
  blocks closed by `partial_fixpoint`, for which Lean elaborates one packed
  fixpoint named after the block's first declaration
  (`types.impls.clone_shape.mutual`, `types.impls.debug_shape.mutual`,
  `contracts.clone_budget.Counter.boxed_type.mutual`,
  `contracts.clone_budget.Counter.boxed_pattern.mutual`). They are not
  textual definitions, so the join cannot classify them. Checker A now
  tolerates a residual `<name>.mutual` only when `<name>` is classified and
  its own definition is built from that packed fixpoint; the new refusal
  cell `borrowed-mutual-suffix` splices an `abbrev
  core.slice.Slice.first.mutual` that the classified `first` does not use,
  and the gate refuses it at `ENV-UNCLASSIFIED`.
- **The `conditional-claimed-universal` cell** now forges the strict axiom
  class as well as the universal strength: with the truthful `native` class
  kept, the join's status rule refused first and the cell no longer reached
  the elaborated-statement check it exists to exercise.

The external models also gained a doc header on every definition (the
parser binds a definition only through its doc comment); no definition
changed.

Gate run (2026-11-16, pinned Lean 4.35.0 and `bun` on PATH):

```
[m2-coverage] PASS COVERAGE-DERIVE (Funs.lean + FunsExternal.lean defs with rust names and source spans; keyword parity holds)
[m2-coverage] PASS COVERAGE-JOIN (total=2343 classified=2343 extracted=2268 proved=0 fixture-checked=1 modeled=69 excepted=5 open=0)
[m2-coverage] PASS EXCEPTED-DISCLOSURE (5 excepted entries == #[charon::opaque] declarations in the crate)
info: m2-coverage: ENV-A-PASS 2343 classified constants bound to their modules; 302 elaborator matchers and 4 partial_fixpoint mutual packings tolerated
info: m2-coverage: ENV-B-PASS 13 refinement citations verified (theorem exists, statement mentions the constant)
[m2-coverage] PASS STRENGTH (citations=13 fixture=12 conditional=1 universal=0 strict=0 native=13 strict_universal=0)
[m2-coverage] PASS (all checks)
```

Refusal matrix (`verification/m2-coverage-refusals.sh`, same PATH), trimmed
to the result lines:

```
refused at UNCLASSIFIED (gate exit 1)            (unclassified-def)
refused at CITATION (gate exit 1)                (dangling-citation)
refused at CITATION-BINDING (gate exit 1)        (misbound-citation)
refused at STALE-ENTRY (gate exit 1)             (stale-entry)
refused at STATUS (gate exit 1)                  (bad-status)
refused at STRENGTH-STATUS (gate exit 1)         (native-fixtures-as-proved)
refused at STRENGTH-DRIFT (gate exit 1)          (conditional-claimed-universal)
refused at ENV-UNCLASSIFIED (gate exit 1)        (borrowed-mutual-suffix)
accepted: the universal probe is classified universal/strict and its function is proved
== all refusals demonstrated ==
```

## 10. Subject re-sync and re-derived record (#97, 2026-11-18)

§9's subject was regenerated once more before this run: the two
`checked_i64_environment` closure adapters became the ordinary helper
functions `binary`/`unary` in `crates/noble-kernel/src/contracts.rs`
(commit `e1a14cb4`), so the generated `Types.lean` lost the two
`@[reducible]` closure aliases and the generated `Funs.lean` lost the two
`Fn`-impl `call`s in favour of `contracts.checked_i64_environment.binary`
(`Funs.lean:40647`) and `.unary` (`:40621`) — the same scheme construction
with the captured tail passed as an ordinary parameter. Only the M3 copy of
the generated output had been updated, so the two lanes had diverged.

**Subject.** `proofs/m2/NobleKernel/Types.lean` and `Funs.lean` are now the
exact bytes of the current `proofs/m3/NobleKernel` files, copied, never
hand-edited:

```
e09e7688131f2bf2fc68fa68453cc86e  proofs/m2/NobleKernel/Types.lean
f27ba9d7370a147f7be2015ca04f2287  proofs/m2/NobleKernel/Funs.lean
```

`FunsExternal.lean` and `TypesExternal.lean` were already identical in both
lanes; `NobleKernel.lean` is the hand-maintained entry point (the M2 copy
carries its regeneration header, the M3 copy is the bare generated entry)
and its generated content — `module` plus `public import NobleKernel.Funs` —
is the same.

**Record.** `m2-coverage.json` was rebuilt from `derive(proofs/m2)` (the
gate's own parser) with the reviewed `status`/`citations`/`note` fields
carried over by function name (§6). Changes, each reviewed:

| Change | Count |
|---|---|
| entries whose restated generated facts moved (647 facts: 640 Lean `line`, 7 Rust `source` span, 3 of them both) | 644 |
| dropped as `STALE-ENTRY`: `…checked_i64_environment.closure{,_1}.Insts.CoreOpsFunctionFnTuplePatternScheme.call` | 2 |
| added as `extracted`: `…checked_i64_environment.{binary,unary}` | 2 |
| status counts after the re-derivation | unchanged: 2,268 `extracted`, 1 `fixture-checked`, 69 `modeled`, 5 `excepted` (2,343 total) |

`noble_kernel.acceptance.check` keeps its thirteen citations unchanged: the
generated `check` statement did not change (only its internal helper calls
did), and the gate re-verified every citation against the new environment.

**Gate run (2026-11-18, commit `589b2d7e`, pinned Lean 4.35.0 and `bun`
first on PATH).** `verification/m2-coverage-gate.sh` (result lines; the
duplicate `COVERAGE-JOIN (record and generated surface agree)` line, the
`NON-CLAIM` note and the `SUMMARY` line are elided — the per-status and
per-strength counts are on the `COVERAGE-JOIN` and `STRENGTH` lines):

```
[m2-coverage] PASS SETUP (bun + lean 4.35.0)
[m2-coverage] PASS BUILD (lake build NobleM2 NobleM2.Refinement NobleKernel)
[m2-coverage] PASS COVERAGE-DERIVE (Funs.lean + FunsExternal.lean defs with rust names and source spans; keyword parity holds)
[m2-coverage] PASS COVERAGE-JOIN (total=2343 classified=2343 extracted=2268 proved=0 fixture-checked=1 modeled=69 excepted=5 open=0)
[m2-coverage] PASS EXCEPTED-DISCLOSURE (5 excepted entries == #[charon::opaque] declarations in the crate)
[m2-coverage] PASS OPEN-SUBJECT (0 open entries; every generated function is classified)
info: m2-coverage: ENV-A-PASS 2343 classified constants bound to their modules; 302 elaborator matchers and 4 partial_fixpoint mutual packings tolerated
info: m2-coverage: ENV-B-PASS 13 refinement citations verified (theorem exists, statement mentions the constant)
[m2-coverage] PASS ENV-BINDING (constants bound in the elaborated environment; citations are theorems mentioning their constants)
[m2-coverage] PASS STRENGTH (citations=13 fixture=12 conditional=1 universal=0 strict=0 native=13 strict_universal=0)
[m2-coverage] PASS (all checks) — subject: …/proofs/m2; record: …/verification/m2-coverage.json
```

`verification/m2-coverage-refusals.sh` (same PATH; result lines, case label
in parentheses):

```
baseline: gate exit 0 (all checks pass)
refused at UNCLASSIFIED (gate exit 1)            (unclassified-def)
refused at CITATION (gate exit 1)                (dangling-citation)
refused at CITATION-BINDING (gate exit 1)        (misbound-citation)
refused at STALE-ENTRY (gate exit 1)             (stale-entry)
refused at STATUS (gate exit 1)                  (bad-status)
refused at STRENGTH-STATUS (gate exit 1)         (native-fixtures-as-proved)
refused at STRENGTH-DRIFT (gate exit 1)          (conditional-claimed-universal)
refused at ENV-UNCLASSIFIED (gate exit 1)        (borrowed-mutual-suffix)
accepted: the universal probe is classified universal/strict and its function is proved
== all refusals demonstrated ==
```

Every cell keeps its semantic target on the new subject: the spliced
`coverage_probe` is refused as `UNCLASSIFIED` at `Funs.lean:49464` (the new
end of the file), the forged strict/universal handoff is still refused by
the elaborated statement (`record says strict … the transitive axiom set is
native`, `record says universal, the elaborated statement is conditional`),
and the positive control still reaches `proved=1 strict_universal=1`. No
cell needed retargeting, so none was deleted or weakened.

Both scripts were re-run on a clean tree with explicit exit capture; each
exited 0 (`EXIT m2-coverage-gate = 0`, `EXIT m2-coverage-refusals = 0`), and
the re-run reproduced the lines quoted above.

## 11. Literal-free checker and re-derived record (#97, 2026-11-18)

The kernel subject was regenerated at Rust commit `12be5375`. Adapter
identities are now compared as byte arrays, and the
`acceptance::parts::join` assertions carry no message, so the generated
checker no longer reaches the three literal-size axioms §9 describes. See
[the M2 proof evidence §10](m2-proof-evidence.md) for the bytes, which are
identical to the M3 lane's. `m2-coverage.json` was rebuilt from
`emit(proofs/m2)` with the reviewed fields carried over by name (§6):

| Change | Count |
|---|---|
| entries whose restated generated facts moved (2,232 facts: 2,101 Lean `line`, 131 Rust `source` span, 113 entries both) | 2,119 |
| added as `extracted`: `contracts.TEST_CODE_EXPORT_IDENTITY_BYTES`, `contracts.TEST_SEND_IDENTITY_BYTES` | 2 |
| dropped as `STALE-ENTRY`: the unused `alloc.string.String.Insts.CoreCmpPartialEqShared0Str.ne` model | 1 |
| status counts after the re-derivation | 2,270 `extracted`, 1 `fixture-checked`, 68 `modeled`, 5 `excepted` (2,344 total) |

`accepted_via_refinement` is computed as a `conditional`/`strict` citation
again, its pre-#97 class. The twelve fixtures stay `native` through their own
`native_decide`. Before the record was re-derived, the gate on the new subject
failed only at `STRENGTH-DRIFT`: "record says native axioms, the transitive
axiom set is strict".

**Gate run (2026-11-18, commit `13d97944`, pinned Lean 4.35.0 and `bun`
first on PATH).** `verification/m2-coverage-gate.sh` (result lines, elided as
in §10):

```
[m2-coverage] PASS SETUP (bun + lean 4.35.0)
[m2-coverage] PASS BUILD (lake build NobleM2 NobleM2.Refinement NobleKernel)
[m2-coverage] PASS COVERAGE-DERIVE (Funs.lean + FunsExternal.lean defs with rust names and source spans; keyword parity holds)
[m2-coverage] PASS COVERAGE-JOIN (total=2344 classified=2344 extracted=2270 proved=0 fixture-checked=1 modeled=68 excepted=5 open=0)
[m2-coverage] PASS EXCEPTED-DISCLOSURE (5 excepted entries == #[charon::opaque] declarations in the crate)
[m2-coverage] PASS OPEN-SUBJECT (0 open entries; every generated function is classified)
info: m2-coverage: ENV-A-PASS 2344 classified constants bound to their modules; 302 elaborator matchers and 4 partial_fixpoint mutual packings tolerated
info: m2-coverage: ENV-B-PASS 13 refinement citations verified (theorem exists, statement mentions the constant)
[m2-coverage] PASS ENV-BINDING (constants bound in the elaborated environment; citations are theorems mentioning their constants)
[m2-coverage] PASS STRENGTH (citations=13 fixture=12 conditional=1 universal=0 strict=1 native=12 strict_universal=0)
[m2-coverage] PASS (all checks) — subject: …/proofs/m2; record: …/verification/m2-coverage.json
```

`verification/m2-coverage-refusals.sh` (same PATH; result lines, case label
in parentheses):

```
baseline: gate exit 0 (all checks pass)
refused at UNCLASSIFIED (gate exit 1)      (unclassified-def)
refused at CITATION (gate exit 1)          (dangling-citation)
refused at CITATION-BINDING (gate exit 1)  (misbound-citation)
refused at STALE-ENTRY (gate exit 1)       (stale-entry)
refused at STATUS (gate exit 1)            (bad-status)
refused at STRENGTH-STATUS (gate exit 1)   (native-fixtures-as-proved)
refused at STRENGTH-DRIFT (gate exit 1)    (conditional-claimed-universal)
refused at ENV-UNCLASSIFIED (gate exit 1)  (borrowed-mutual-suffix)
accepted: the universal probe is classified universal/strict and its function is proved (universal-probe)
== all refusals demonstrated ==
```

Every cell keeps its semantic target. `conditional-claimed-universal` now
refuses only for the universal claim, because the strict-axiom claim it also
forges is true of `accepted_via_refinement` again. The gate ran at commit
`13d97944` and the refusal matrix at `8bbc7975`, which leaves every M2 input
unchanged. Both ran with explicit exit capture, and each exited 0:
`EXIT m2-coverage-gate = 0`, `EXIT m2-coverage-refusals = 0`.
