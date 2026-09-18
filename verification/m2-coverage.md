# M2 per-function coverage classification gate (task 3.3)

**Status: complete.** Every function of the M2 kernel extraction subject is
classified `extracted`, `proved`, `modeled`, `excepted`, or `open` in the
reviewed record `verification/m2-coverage.json`; the gate
`verification/m2-coverage-gate.sh` joins that record with the authoritative
function list derived from the extraction output, enforces consistency in
both directions, and refuses five mutation classes
(`verification/m2-coverage-refusals.sh`). Run on the current tree:
baseline green, all five refusals demonstrated. Nothing under `proofs/`,
the Rust crate, or the existing gates was modified; only the four new
`verification/` files listed in §7 exist.

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
| C3 COVERAGE-JOIN | `UNCLASSIFIED`, `STALE-ENTRY`, `STATUS`, `STATUS-PLACEMENT`, `DRIFT`, `RECORD-FORM` | every generated function classified exactly once; every non-`open` entry generated; status vocabulary; `extracted`/`proved` live in `Funs.lean`, `modeled`/`excepted` in `FunsExternal.lean`; citations exactly on `proved`; record fields restate the generated facts (kind/rust/file/line/span/loop_body/trait_impl) |
| C4 EXCEPTED-DISCLOSURE | `EXCEPTED-DISCLOSURE` | the `excepted` set is exactly the crate's `#[charon::opaque]` declarations (count and short names, extracted from `crates/noble-kernel/src`) |
| C5 OPEN-SUBJECT | `OPEN-ENTRY`, `OPEN-SUBJECT` | `open` entries may only name subject functions missing from the output, and must exist as Rust functions in the crate; a generated function marked `open` rejects |
| C6 ENV-BINDING/CITATIONS | `CONSTANT-MISSING`, `CONSTANT-KIND`, `MODULE`, `ENV-UNCLASSIFIED`, `CITATION`, `CITATION-BINDING` | two scratch Lean checkers (proof-gate mechanics): every entry exists as a definition in its generated module; residual defs are matchers only; every citation exists, is a theorem, and its *statement mentions the classified constant* (semantic subject binding, as in the proof gate's GATE-B) |
| C7 SUMMARY | — | per-status counts; 100% classification required |

The two scratch checkers exist because the root module `NobleM2` and
`NobleM2.Refinement` cannot be imported together (both elaborate
`deriving`-generated instances with identical names, e.g.
`NobleM2.UnsupportedKind.ofNat` from `NobleM2.Termination`) — the same
diamond the proof gate solves with GATE_A/GATE_B. Checker A imports
`NobleKernel` + `NobleM2` (constants, residual, non-Refinement citations);
checker B imports `NobleM2.Refinement` (the refinement family). Note that
the `NobleM2` root does **not** transitively import `NobleKernel`, so A
imports it explicitly.

## 3. Classification summary (current tree)

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
