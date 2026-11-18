# M2 proof evidence: gate, theorem inventory, disclosures (2026-09-18)

Scope: the proof-milestone closeout of the `m2-checker-feasibility` change —
task 4.5 (the proof-required gate) and the evidence records of tasks 6.1/6.2 —
over the proof commits `2d3cbe7`..`92b0081`. Fragment label: **M2 fragment
v0**. Every command output below was produced this session against the
worktree's proof tree exactly as committed at `92b0081`; nothing in `proofs/`
was modified (the gate runs read-only over the proof root).

## 1. The proof-required gate (task 4.5)

The gate is the checked-in control `verification/m2-proof-gate.sh`. Exact
command, run from the repository root with the pinned Lean first on PATH:

```console
$ PATH=/nix/store/hbxwgcbpkyqllqzq7cidhnvhgb60jb9q-noble-lean-release-4.31.0/bin:$PATH \
    verification/m2-proof-gate.sh
```

Prerequisites: `proofs/m2` with its lake package cache populated
(`.lake/packages`, built once by `lake update` or by the extraction toolrun
recorded in [the probe ladder](m2-extraction-probe.md)); the pinned Lean
4.31.0 release on PATH; nothing else. The gate may be pointed at another
proof root (`verification/m2-proof-gate.sh <root>` or `M2_PROOF_ROOT`), which
is how the refusal matrix runs mutated scratch copies.

Checks, in order (first failure exits non-zero with the named check):

| Check | Defends against (design.md gate list) |
|---|---|
| `TOOLCHAIN` | a proof run on an unpinned Lean (must be the 4.31.0 release) |
| `BUILD` (`lake build NobleM2 NobleM2.Refinement NobleKernel`) | a proof-required claim without a building theorem target: a missing function, a deleted external implementation, an unfinished required refinement |
| `NO-SORRY` (`grep -rnw sorry`, all `.lean` outside `.lake`) | a proof hole anywhere in the proof tree — `lake build` alone exits 0 on a `sorry` warning, this scan does not |
| `GENERATED-KERNEL` | a missing/empty/hand-replaced generated module: `NobleKernel.lean` must carry the Aeneas generation header and `import NobleKernel.Funs`, and `Funs.lean` must define `acceptance.check` |
| `REFINEMENT-SUBJECT` | a refinement restated about a substitute: `Refinement.lean` and `Embed.lean` must import the generated module itself (`import NobleKernel` as a whole line) |
| `REQUIRED-THEOREMS` | a missing or hole-carrying required theorem, and a substituted proof subject: an elaborated Lean checker asserts every milestone theorem exists, that its axioms satisfy the policy below, and — for the refinement family — that the theorem's *type* mentions the generated constant `noble_kernel.acceptance.check` (semantic subject binding, so keeping the import while calling a hand-written copy still rejects) |
| `EXTERNAL-MODELS` | an unexplained external model: no `axiom` declarations may remain in `FunsExternal.lean`/`TypesExternal.lean` (the Aeneas split-file templates are axiom stubs; the checked-in file replaces them with implementations) |

Axiom policy enforced by `REQUIRED-THEOREMS`: the reference-model theorems
(`applyScheme_ok`, `check_soundness`, `check_total`,
`foldBody_work_bounded`), the composition lemma `accepted_via_refinement`,
and the extracted checker itself may use only Lean's trifecta
`{propext, Classical.choice, Quot.sound}`; the evaluation-closed theorems
may additionally use the per-declaration `native_decide` axioms Lean 4.31
emits (name suffix `._native.native_decide.ax_1_1`). `sorryAx` and anything
else reject. This is the programmatic form of `#print axioms` for every
required theorem at once.

### Passing run (2026-09-18, trimmed to the check lines; the full axiom
inventory the gate prints is reproduced in §3)

```console
[m2-gate] PASS TOOLCHAIN (lean 4.31.0)
[m2-gate] PASS BUILD (lake build NobleM2 NobleM2.Refinement NobleKernel)
[m2-gate] PASS NO-SORRY (no 'sorry' outside .lake)
[m2-gate] PASS GENERATED-KERNEL (Aeneas entry point, Funs.lean, acceptance.check)
[m2-gate] PASS REFINEMENT-SUBJECT (Refinement.lean and Embed.lean import NobleKernel)
m2-gate: axioms NobleM2.applyScheme_ok [propext, Quot.sound]
m2-gate: axioms NobleM2.check_soundness [propext, Classical.choice, Quot.sound]
m2-gate: axioms NobleM2.check_total [propext, Classical.choice, Quot.sound]
m2-gate: axioms NobleM2.foldBody_work_bounded [propext, Classical.choice, Quot.sound]
m2-gate: axioms noble_kernel.acceptance.check [propext, Classical.choice, Quot.sound]
m2-gate: axioms NobleM2.Refinement.accepted_via_refinement [propext, Classical.choice, Quot.sound]
m2-gate: subject NobleM2.Refinement.accepted_via_refinement binds noble_kernel.acceptance.check
  … (12 fixture refinement theorems: axioms + subject binding, all bind
    noble_kernel.acceptance.check; each carries only its own
    ._native.native_decide.ax_1_1 besides the trifecta) …
[m2-gate] PASS REQUIRED-THEOREMS (inventory, axiom policy, subject binding)
[m2-gate] PASS EXTERNAL-MODELS (no template axioms in FunsExternal/TypesExternal)
[m2-gate] PASS (all checks) — proof root: …/proofs/m2
```

## 2. Refusal controls (task 4.5, each executed on a scratch copy)

Runner: `verification/m2-proof-gate-refusals.sh` (same PATH prerequisite).
It first re-runs the positive baseline, then four mutated scratch copies of
`proofs/m2` (hard-linked package cache, isolated `.lake/build`), and asserts
each mutation is refused at its named check. A positive baseline that fails
aborts the matrix. Output of the run this session:

```console
== positive baseline ==
[M2-gate lines as in §1]
baseline: gate exit 0 (all checks pass)

== mutation: proof-hole (expected refusal: NO-SORRY) ==
[m2-gate] PASS TOOLCHAIN (lean 4.31.0)
[m2-gate] PASS BUILD (lake build NobleM2 NobleM2.Refinement NobleKernel)
[m2-gate] FAIL NO-SORRY: proof sources contain 'sorry'
…/proof-hole/NobleM2/CheckSoundness.lean:301:  sorry
refused: gate exit 1 at NO-SORRY

== mutation: substituted-subject (expected refusal: REFINEMENT-SUBJECT) ==
[m2-gate] PASS TOOLCHAIN (lean 4.31.0)
[m2-gate] PASS BUILD (lake build NobleM2 NobleM2.Refinement NobleKernel)
[m2-gate] PASS NO-SORRY (no 'sorry' outside .lake)
[m2-gate] PASS GENERATED-KERNEL (Aeneas entry point, Funs.lean, acceptance.check)
[m2-gate] FAIL REFINEMENT-SUBJECT: NobleM2/Refinement.lean does not import the
  generated NobleKernel module (substituted subject)
refused: gate exit 1 at REFINEMENT-SUBJECT

== mutation: unexplained-external (expected refusal: BUILD) ==
[m2-gate] PASS TOOLCHAIN (lean 4.31.0)
[m2-gate] FAIL BUILD: lake build exited 1 (proof-required claims need a building target)
error: NobleKernel/Funs.lean:98:12: Unknown identifier `types.impls.clone_stack`
error: NobleKernel/Funs.lean:100:13: Unknown identifier `types.impls.clone_stack`
error: build failed
refused: gate exit 1 at BUILD

== mutation: unexecuted-coverage (expected refusal: REQUIRED-THEOREMS) ==
[m2-gate] PASS TOOLCHAIN (lean 4.31.0)
[m2-gate] PASS BUILD (lake build NobleM2 NobleM2.Refinement NobleKernel)
[m2-gate] PASS NO-SORRY (no 'sorry' outside .lake)
[m2-gate] PASS GENERATED-KERNEL (Aeneas entry point, Funs.lean, acceptance.check)
[m2-gate] PASS REFINEMENT-SUBJECT (Refinement.lean and Embed.lean import NobleKernel)
[m2-gate] FAIL REQUIRED-THEOREMS: reference-model theorem inventory check exited 1
.m2-gate-a.lean:14:7: error(lean.unknownIdentifier): Unknown constant `NobleM2.check_total`
.m2-gate-a.lean:15:7: error(lean.unknownIdentifier): Unknown constant `NobleM2.foldBody_work_bounded`
.m2-gate-a.lean:17:7: error(lean.unknownIdentifier): Unknown constant `NobleM2.coverage_positive`
.m2-gate-a.lean:18:7: error(lean.unknownIdentifier): Unknown constant `NobleM2.Coverage.accepted`
.m2-gate-a.lean:19:7: error(lean.unknownIdentifier): Unknown constant `NobleM2.Coverage.derives`
refused: gate exit 1 at REQUIRED-THEOREMS

== refusal matrix complete: baseline passes, all four mutations refused ==
```

What each mutation is (all four are spelled out in the runner and applied
mechanically):

1. **Proof hole (PRF-HOLE).** The `check_soundness` proof body in
   `CheckSoundness.lean` is replaced by `sorry` (theorem statement kept).
   The build still passes — `sorry` is only a warning to `lake` — and the
   gate refuses at `NO-SORRY`. Had the scan been bypassed, the axiom policy
   would refuse the `sorryAx` the theorem then depends on.
2. **Substituted subject (PRF-SUBJECT).** A scratch module
   `NobleM2/NobleKernelStub.lean` defines a hand-written
   `acceptance.check` with the same name and type (delegating body), and
   `Refinement.lean` is rewritten to import it and call
   `noble_kernel_stub.acceptance.check`. The mutated tree **builds green**
   and every refinement theorem still proves — exactly the hand-written
   substitute the design rejects — and the gate refuses at
   `REFINEMENT-SUBJECT` (import binding); the semantic subject-binding check
   in `REQUIRED-THEOREMS` refuses the same attack if only the call is
   rewritten and the import kept.
3. **Unexplained external model.** The `types.impls.clone_stack`
   implementation is deleted from `FunsExternal.lean`. The generated module
   no longer elaborates (`Unknown identifier 'types.impls.clone_stack'`,
   `Funs.lean:98/100`) and the gate refuses at `BUILD`. The complementary
   variant — leaving the Aeneas template's `axiom` in place instead of an
   implementation — is refused by `EXTERNAL-MODELS`.
4. **Unexecuted required coverage case.** `Termination.lean` (the
   `coverage_positive` theorem file) is deleted, its imports are stripped
   from `NobleM2.lean` and `CheckSoundness.lean`, and the `DecidableEq`
   instances it provided are moved into `CheckSoundness.lean` so the tree
   **still builds green**. The gate refuses at `REQUIRED-THEOREMS`: the
   required theorems `check_total`, `foldBody_work_bounded`,
   `coverage_positive` (and the `Coverage` witnesses) are gone.

## 3. Theorem inventory

Axiom sets were enumerated this session with the same mechanism the gate
uses (`Lean.collectAxioms` over the built environment, both import roots).
Excluding the compiler-generated `.inj`/`.injEq`/`.sizeOf_spec`/`.eq_1`/
`.eq_def`/`.brecOn` scaffolding (316 and 284 environment theorems under the
two roots; every one of them is trifecta-clean or carries only its own
`._native.native_decide.ax_1_1` axioms or those of the evaluation-closed
premises it uses), the hand-written theorems are:

**Reference model — declarative layer (`NobleM2/Judgment.lean`)**

| Theorem | Statement (one line) | Axioms |
|---|---|---|
| `union_nil_left` / `union_nil_right` | `{[]} ∪ x = x`, `x ∪ {[]} = x` (empty-bound identities) | ⊆ trifecta |
| `union_empty_left` / `union_empty_right` | the same for the empty set | ⊆ trifecta |
| `union_assoc` | `(a ∪ b) ∪ c = a ∪ (b ∪ c)` — summary associativity, spec §8.2 | ⊆ trifecta |
| `foldl_union` | `heads.foldl (∪) b = b ∪ heads.foldr (∪) {}` — reconciles the fold's left-nested bound with the judgment's right-nested index | ⊆ trifecta |

(private helpers: `cons_congr`, `natStrongInduction`, `union_assoc_aux`.)

**Reference model — soundness (`NobleM2/Soundness.lean`, `CheckSoundness.lean`)**

| Theorem | Statement | Axioms |
|---|---|---|
| `applyScheme_ok` | an accepted scheme application exposes exactly the instantiated interface | propext, Quot.sound |
| `append_nil` | `l ++ [] = l` on stacks | ⊆ trifecta |
| `quotationScheme_none` / `_some` | the quotation scheme instantiates to `none` without its outer binding, and to the node's own interface with it | ⊆ trifecta |
| `dataOkAt_iff` | the eligibility guard decides the declarative `DataOk` side condition | ⊆ trifecta |
| `subsetOf_subset` | a decided effect inclusion is the declarative subset relation | ⊆ trifecta |
| `joinInterface_ok` | a successful join exposes its segment premises (tail match, replacement, bound union) | ⊆ trifecta |
| `foldBody_derives` | every successful fold yields a `Derives` whose effect index is the right-nested union of the charged interfaces | ⊆ trifecta |
| `foldBody_entry_derives` | at a fold's entry (empty incoming bound) the derivation's index is the fold's accumulated bound | ⊆ trifecta |
| `check_soundness` | **acceptance soundness (V-CHECK-03/PO-09):** `check … = accepted checked` ⇒ `WellFormed` ∧ `TypingDerivation` | **trifecta** |
| `Regression.ce_accepted` / `Regression.ce_derives` | the closed counterexample: the segment-consuming literal input is accepted *and* derivable (regression for the frame-join reading) | trifecta + own `native_decide` axiom |

**Reference model — termination and coverage (`NobleM2/Termination.lean`)**

| Theorem | Statement | Axioms |
|---|---|---|
| `charge_ok` | a successful charge spends exactly its cost (`budget + cost = work`) | ⊆ trifecta |
| `stepFold_ok` | one charged step repays its cost and records exactly one derivation | ⊆ trifecta |
| `schemeCost_pos` / `joinCost_pos` | every charged step costs ≥ 1 work unit | ⊆ trifecta |
| `foldBody_work_bounded` | every successful fold stays within its work budget and strictly spends from it on a nonempty body | **trifecta** |
| `check_total` | the decision function is total (every request/candidate reaches an outcome) | **trifecta** |
| `Coverage.lit41`/`lit1`/`addLook`/`addInst` | the fixture's per-node instantiation premises (by evaluation) | own `native_decide` axioms |
| `Coverage.accepted` | the kernel's arithmetic fixture is accepted with the asserted checked result | trifecta + own `native_decide` |
| `Coverage.derives` | and carries a declarative `Derives` witness at its expected interface | ⊆ trifecta + the four premises' `native_decide` axioms |
| `coverage_positive` | **positive coverage (PO-10):** an accepted fragment input with a `TypingDerivation` exists | trifecta + the `Coverage` `native_decide` axioms |

**Refinement (`NobleM2/Refinement.lean`; subject: the generated
`noble_kernel.acceptance.check`)**

| Theorem | Statement | Axioms | Subject |
|---|---|---|---|
| `refinement_sequence_and_literals_accept_arithmetic` … `refinement_resource_eligibility_rejects_duplication` (12) | for each kernel acceptance fixture: `project_result (acceptance.check (embedEnv …) (embedRequest …) (embedCandidate …)) = NobleM2.check …` (the two diagnostic-provenance fixtures after `eraseProvenance`, see §7) | trifecta + own `native_decide` axiom | binds the generated constant (gate-verified) |
| `accepted_via_refinement` | extracted/reference agreement + extracted `ok (Accepted g)` ⇒ the reference outcome is the acceptance of the projected checked result | **trifecta** | binds the generated constant (gate-verified) |

**Extracted checker** — `noble_kernel.acceptance.check` (generated,
`NobleKernel/Funs.lean`): **trifecta** (`propext, Classical.choice,
Quot.sound`), i.e. the extracted call graph including its fuel-driven
`partial_fixpoint` loops rests on nothing beyond Lean's logical foundation.

**Embedding layer** (`NobleM2/Embed.lean`): the termination side conditions
`sizeOf_vec1_lt_programType`, `sizeOf_vec2_lt_programType` (⊆ trifecta);
everything else is a definition (§6 discloses the modeling choices).

**Fixture agreement harness** (`NobleM2/Fixtures.lean`): twelve `Row`s
comparing the reference outcome with the outcome the Rust tests assert,
checked by `#eval rows.all (·.outcomeEq …)` → `true` (12/12; part of the
repair verification in §4) and superseded as *proof* by the refinement
family above.

## 4. Reference-model repair (what diverged, how verified)

The first reference model (`2d3cbe7^`) was written from the fragment
definition and diverged from the kernel on every fixture that exercises a
join or a diagnostic. Commit `2d3cbe7` ("Repair the reference model to
match the kernel's decisions") records the four divergences:

1. **`tailOf` returned the bottom of the stack where the kernel takes the
   top** (spec §4.1: "The stack top is written on the right"), so every
   join with a nonempty expected segment failed.
2. The reference fold carried a **work-conservation guard the kernel does
   not have** (the kernel charges and fails closed on budget exhaustion,
   it does not require exact conservation).
3. **Eligibility failures were mapped to the instantiation-kind
   constraint** instead of the eligibility constraint.
4. Diagnostics were reported **without stacks** (no expected/actual lists).

Verification path: the twelve kernel fixtures were restated as Lean
definitions (`Fixtures.lean`) with an agreement harness (`#eval`, 12/12
`true`), and the agreement was then *proved* against the extracted checker
itself by the twelve refinement theorems of §3 — which this session's gate
run re-verifies (build green, axioms within policy, subject bound to the
generated constant). The repair direction is kernel-as-authoritative for
decisions; the divergence list above is why the reference model is a
*model* of the kernel's decisions, not an independent second opinion.

## 5. Judgment frame-join decision (spec citations)

The declarative judgment's SEQUENCE rule composes a node's instantiated
interface onto the running stack by **segment replacement** — premises
`tailEquals stack a = true` and `replaceTail stack a b = mid` — mirroring
the checker's `joinInterface`, rather than by exact frame equality
(`stack = a`) which the first draft used. The exact-frame reading is
refuted by the retained counterexample regression (`Regression.ce_*`): the
checker accepts a literal whose instantiation consumes only the empty
segment while the running stack is `[bool]`, and that acceptance is
derivable precisely under segment replacement. The citations:

- §4.1: "A stack-tail variable stands for a finite ordered prefix of zero
  or more values. It is not an untyped container and cannot be inspected by
  a word that merely threads it onward." — the joined segment is the
  prefix the node's `stackIn` matches, and "the stack top is written on the
  right" fixes which end.
- §4.2: "A named definition may have a rank-1 type scheme quantifying
  value-type, stack-tail, and effect variables, together with built-in
  eligibility constraints. First-class program values have instantiated
  interfaces." — the node rule demands the *exact* instantiation
  (`NodeDerives`), never a generalization of it.
- §8.1: SEQUENCE (`p : A -- B ! e`, `q : B -- C ! f` ⊢ `p q : A -- C !
  union(e,f)`) and "word invocation instantiates its rank-1 scheme with
  fresh variables before unification"; QUOTATION (`B : A -- B' ! e` ⊢
  `[B] : R -- R Program<A,B',e> ! {}`) — the quotation constructor's
  construction-bound-empty premise.
- §8.2: "Union is associative, commutative, and idempotent as a
  *summary*" — `foldl_union`/`union_assoc` reconcile the fold's left-nested
  accumulation with the judgment's right-nested derivation index at the
  fold's entry.

## 6. Disclosed assumptions

**External implementations (`NobleKernel/FunsExternal.lean`).** Aeneas's
`-split-files` routes every external-to-the-crate definition not covered by
the Aeneas standard library into `FunsExternal_Template.lean` as axiom
stubs; the checked-in file replaces each with a total Lean implementation
mirroring the Rust std semantics on the reachable domain:

- `core::cmp`: `PartialEq<bool>::ne`;
- `core::convert`: `TryFrom<u32, usize>` / `TryFrom<u64, usize>` via the
  guarded `tryFromUScalar` (std's Ok-on-fit/Err-overflow);
- `core::fmt`: `Formatter::debug_c_like_enum_write_str`,
  `Option::<T>::fmt` — **formatter no-ops**: in the Aeneas model
  `core.fmt.Formatter` is opaque and `write_str`/`write_fmt` return the
  formatter unchanged, so a formatting step has no observable effect and
  the written text is not modeled;
- `core::num`: `usize::saturating_mul` (exact product clamped to
  `usize::MAX` — Lean `Nat` does not overflow);
- `core::option`: `Option::map`, `Option::<&T>::copied` (identity: `&T` is
  represented by `T` and `T : Copy`), `Option::clone`;
- `core::result`: `Result::is_err`, `ok`, `unwrap_or`;
- `alloc::vec`: `Vec::truncate`, `as_slice`, `pop`, `is_empty`,
  `Extend::extend` (fails only where Rust would abort on capacity
  overflow);
- **the crate's `#[charon::opaque]` helpers** `types.impls.clone_stack`,
  `shapes.impls.clone_parts` — **clone value-preservation**: the Rust
  bodies deep-copy the slice elements one at a time; in the functional
  model the (terminating) extracted `Clone` impls compute values equal to
  their inputs, so returning the same elements is the same observable
  value — plus `shapes.impls.debug_slots`, `debug_parts`,
  `types.impls.debug_stack` (formatter no-ops as above).

Keeping these implementations in `FunsExternal.lean` (rather than removing
the `#[charon::opaque]` attributes from the Rust crate) keeps the
extraction reproducible: Aeneas cannot functionalize those loops inside the
trait-instance mutual block, which is why the attributes exist. None of the
six carries a checker decision (they are copying and formatting steps).

**Embedding (`NobleM2/Embed.lean`).** `Nat` identifiers embed into `U32`
modulo `2^32` (the identity on every fragment identifier); `Int` payloads
via two's-complement `BitVec.ofInt 64`; lists into `Vec` through `vecOf`,
whose length-overflow fallback (inputs longer than `usize::MAX` —
unreachable for every fragment value) yields the empty vector; the
recursive translations are fuel-parameterized with size-dominating budgets
whose exhausted fallback arms never fire on well-formed inputs.

**Evaluation-closed proofs.** The refinement family, the coverage
witnesses, and the regression close their equations by `native_decide`:
`Scheme.instantiate` runs under well-founded recursion that the kernel's
defeq checker does not unfold, and the extracted call graph runs Aeneas's
fuel-driven `partial_fixpoint` loops. Each such theorem carries only its
own per-declaration `native_decide` axiom besides the trifecta (§3, and
the gate's policy check makes any other axiom a hard failure).

## 7. Caveats

- **The two `eraseProvenance` fixtures.** For
  `hidden_emit_rejects_against_empty_bound` and
  `resource_eligibility_rejects_duplication`, the extracted checker fills
  the failing `node`/`def` provenance fields exactly as the Rust kernel
  does, while the reference `diagnosticOf` records `none` — fields the
  Rust tests never assert. The two refinement theorems are therefore
  stated after `eraseProvenance`, which clears exactly those two fields
  and nothing else. The provenance fields in the reference model remain a
  known gap, kept open in [the scope review](m2-scope-review.md).
- **File sizes vs the 300-line style cap.** `Judgment.lean` (358),
  `Termination.lean` (317), `CheckSoundness.lean` (379), and `Embed.lean`
  (488) exceed the checker style cap. Accepted deviation: the first three
  are irreducible proof case-trees (the three-node-form fold induction,
  the five-way outcome split of the acceptance theorem, the judgment
  laws), and `Embed.lean` is one symmetric embedding/projection layer
  whose split would separate mutually documented translations; the proof
  tree is frozen after acceptance, so the deviation is documented here
  rather than split. `NobleKernel/Funs.lean` (8021 lines) is generated and
  not subject to hand-written style rules.
- **Gate scope.** The gate binds the proof subject at the Lean level
  (generated module presence, import list, constant-level subject binding,
  axiom policy). The extraction-record identity bindings (source blake3,
  inventory, selection, tool revisions) remain the M1-owned `extract`
  machinery recorded in [the probe ladder](m2-extraction-probe.md); the
  two controls compose. Aggregate `m1-controls`-style matrix enumeration
  stays reserved (task 5.2 of M1's plan; the M2 runner covers the four
  named task-4.5 cells).

## 8. Addendum: the whole-crate re-extraction (#97, 2026-11-16)

Sections 1–7 record the 2026-09-18 subject and gate. Since #97 the
checked-in `proofs/m2/NobleKernel/{Funs,Types}.lean` are the whole-crate
extraction of `crates/noble-kernel` by Charon `9e95cf31` (with
`--inline-anon-consts`) and Aeneas `1238879f`, byte-identical to
`proofs/m3/NobleKernel`. Where this addendum and §1–§7 differ, the
addendum describes the current tree.

**External models (supersedes the §6 list).** `NobleKernel/FunsExternal.lean`
defines the template's 71 externals — 66 `rust_fun` functions plus the five
`#[charon::opaque]` helpers — and three more functions the template no
longer lists, and `TypesExternal.lean` its six external types; both files
are shared verbatim with `proofs/m3`. Their headers and per-definition
comments are the review record. Two decisions matter for readers of the
theorems:

- `core::mem::size_of::<T>` returns `fail .undef`. Its value is rustc's
  layout size for `T`, which the Rust source does not determine; the
  generated `Counter::vector::<T>`/`Counter::node::<T>` call it with `T` a
  bound type variable, so no computable per-type model exists, a constant
  would assert one false size, and a classical choice would stop the
  computable generated module compiling. Only
  `Env::calculator_clone_budget` reaches it, and no other kernel function
  calls the clone-budget walk (`acceptance::check` does not). No `ok`
  clone-budget result is derivable in the model, and a partial-correctness
  statement about the clone budget would hold vacuously.
- `Option::map`, `Result::unwrap_or` and
  `Formatter::debug_c_like_enum_write_str` are not in the template: the
  pinned library models the first two and the regenerated kernel no longer
  calls the third. They are kept because the MC1 and M3-Wasm `StdModels`,
  which compile this module from `proofs/m3`, delegate to them by name.
  Inside `noble_kernel` the generated calls to `Option::map` and
  `Result::unwrap_or` therefore resolve to these copies, which have the
  library functions' semantics.

**Embedding (`NobleM2/Embed.lean`).** The extracted environment's twelve
fields that fragment v0 does not model are fixed at the Rust bootstrap
environment's values (`contracts::environment`): resource kinds
`[fixtureResource]`, no nominals, generic variants, live-resource nominals,
owners or bound adapters, every optional profile off, no caller module and
no declared-module admission. `embedEnv`'s docstring states this admission
surface as a premise of every refinement theorem. Kernel-only types
(contract, evidence, certified, live-reference, nominal, generic-nominal)
project to a resource type of kind `2^32 + tag`, outside the `U32` range,
so they never coincide with a projected in-fragment type; the five
kernel-only constraints project to matching reference constructors that
the fragment v0 checker never emits (`NobleM2/Candidate.lean`); the
kernel's live-site list is dropped because the embedded environment
disables live slots. No theorem statement changed.

**Axioms (supersedes the "Extracted checker" paragraph of §3).** The
pinned Aeneas library's
`toStr (s) (h : s.toByteArray.size ≤ U32.max := by decide +native)`
leaves one `<decl>._native.decide.ax_<n>` axiom per string literal of the
generated code. `noble_kernel.acceptance.check` reaches three of them:

```
_private.NobleKernel.Funs.0.noble_kernel.contracts.TEST_CODE_EXPORT_ADAPTER_IDENTITY._native.decide.ax_1
_private.NobleKernel.Funs.0.noble_kernel.contracts.TEST_SEND_ADAPTER_IDENTITY._native.decide.ax_1
_private.NobleKernel.Funs.0.noble_kernel.acceptance.parts.join._native.decide.ax_5
```

each of type `decide ((String.toByteArray "<literal>").size ≤ U32.max) =
true`. The extracted checker, `accepted_via_refinement` and every
`refinement_*` theorem are therefore **no longer trifecta-only**: each also
trusts the compiled evaluation of those literal sizes. The gate's checker B
admits exactly M4Audit's reviewed `nativeStringBound` shape
(`proofs/m4/M4Audit.lean`) owned by `NobleKernel.Funs` or
`NobleKernel.FunsExternal`, and reports those axioms on their own line as
`generated literal-size evaluation (not strict)`; any other axiom keeps the
§1 policy. `verification/m3-proof-gate.sh` carries a byte-identical
matcher. The reference-model theorems of checker A are unchanged:
trifecta only, plus their own `native_decide` axiom for the three
evaluation-closed ones.

**Refusal controls.** `verification/m2-proof-gate-refusals.sh` gains two
cells, each required to be refused at `REQUIRED-THEOREMS` *for its probe
axiom*:

- `foreign-native-type`: a `decide +native` on a `Nat` bound inside the
  external model `Shared0Slice.Insts.CoreCmpPartialEqArray.eq`, which
  `acceptance.check` reaches — owner and name have the generated shape, the
  type does not;
- `foreign-owner-literal`: a `toStr` literal inside `embedEnv` — the exact
  literal-size shape, owned by `NobleM2.Embed`.

**Commands and results (2026-11-16, pinned Lean 4.35.0 first on PATH).**
`lake build NobleM2 NobleM2.Refinement NobleKernel` in `proofs/m2`: build
completed successfully; all twelve refinement theorems close by
`native_decide` on the regenerated checker, and the fixture harness of
`NobleM2/Fixtures.lean` reports 12/12. The only build warnings are the
pre-existing `Shapes.lean` unused `termination_by` and the unused
`checked` binder of `accepted_via_refinement`, both already in the Lean
4.31 build logs.

```
$ verification/m2-proof-gate-refusals.sh
== positive baseline ==
[m2-gate] PASS (all checks)
baseline: gate exit 0 (all checks pass)
refused: gate exit 1 at NO-SORRY                (proof-hole)
refused: gate exit 1 at REFINEMENT-SUBJECT      (substituted-subject)
refused: gate exit 1 at BUILD                   (unexplained-external)
refused: gate exit 1 at REQUIRED-THEOREMS       (unexecuted-coverage)
refused for the probe axiom: *CoreCmpPartialEqArray.eq._native.decide.ax_*
refused: gate exit 1 at REQUIRED-THEOREMS       (foreign-native-type)
refused for the probe axiom: *embedEnv._native.decide.ax_*
refused: gate exit 1 at REQUIRED-THEOREMS       (foreign-owner-literal)
== refusal matrix complete: baseline passes, all six mutations refused ==
```

(Trimmed to the result lines; the per-cell case labels in parentheses are
added here.) In the two new cells the gate names exactly the probe
axiom as forbidden —
`_private.NobleKernel.FunsExternal.0.noble_kernel.Shared0Slice.Insts.CoreCmpPartialEqArray.eq._native.decide.ax_1`
and `NobleM2.embedEnv._native.decide.ax_1` — while the three generated
literal-size axioms stay on the separate "not strict" line.

## 9. Subject re-sync (#97, 2026-11-18)

**§8 is historical for the subject it describes.** After it was written the
kernel extraction was regenerated once more (the two `checked_i64_environment`
closure adapters became the ordinary helpers `binary`/`unary` in
`crates/noble-kernel/src/contracts.rs`, commit `e1a14cb4`), and the checked-in
M2 subject still carried the earlier bytes. `proofs/m2/NobleKernel/{Types,
Funs}.lean` are now byte-identical to the current `proofs/m3/NobleKernel`
files (`md5 e09e7688131f2bf2fc68fa68453cc86e` and
`f27ba9d7370a147f7be2015ca04f2287`), and the two `*External.lean` external
models were already identical. The library entry point
`proofs/m2/NobleKernel.lean` keeps its hand-written regeneration header
(the M3 copy is the bare generated entry); its generated content — `module`
plus `public import NobleKernel.Funs` — is the same, and both files are
hand-maintained, not compared by byte. Nothing else in the proof tree
changed.

What this does and does not establish: the subject is the same generated code
the M3 lane carries, and the gates below re-verify the theorems against it.
It is not a fresh extraction run; the extraction receipt, its subject commit
and the source inventory belong to the lanes that own them.

The change is inside the generated definitions, not in any theorem
statement: `contracts.checked_i64_environment` now calls
`contracts.checked_i64_environment.binary`/`.unary` (plain generated defs
with Rust names and source spans) where it used to call the two
`Fn`-impl `call`s of the closure adapters. `noble_kernel.acceptance.check`'s
type is unchanged, so every refinement theorem still states the same
equation; all twelve `refinement_*` theorems still close by
`native_decide`, and the three generated literal-size axioms of §8 are
unchanged (`TEST_CODE_EXPORT_ADAPTER_IDENTITY`, `TEST_SEND_ADAPTER_IDENTITY`,
`acceptance.parts.join`).

**Gate run (2026-11-18, commit `589b2d7e`, pinned Lean 4.35.0 first on
PATH).** Commands:
`lake build NobleM2 NobleM2.Refinement NobleKernel` (inside the gate),
`verification/m2-proof-gate.sh`, `verification/m2-proof-gate-refusals.sh`.

```
[m2-gate] PASS TOOLCHAIN (lean 4.35.0)
[m2-gate] PASS BUILD (lake build NobleM2 NobleM2.Refinement NobleKernel)
[m2-gate] PASS NO-SORRY (no 'sorry' outside .lake)
[m2-gate] PASS GENERATED-KERNEL (Aeneas entry point, Funs.lean, acceptance.check)
[m2-gate] PASS REFINEMENT-SUBJECT (Refinement.lean and Embed.lean import NobleKernel)
[m2-gate] PASS REQUIRED-THEOREMS (inventory, axiom policy, subject binding)
[m2-gate] PASS EXTERNAL-MODELS (no template axioms in FunsExternal/TypesExternal)
[m2-gate] PASS (all checks) — proof root: …/proofs/m2
```

The axiom lines for `noble_kernel.acceptance.check` and the twelve
`refinement_*` theorems are the §8 set (trifecta + the declaration's own
`native_decide.ax_1_1` + the three literal-size axioms, reported separately
as `generated literal-size evaluation (not strict)`), and every refinement
theorem logs `subject … binds noble_kernel.acceptance.check`.

```
$ verification/m2-proof-gate-refusals.sh
baseline: gate exit 0 (all checks pass)
refused: gate exit 1 at NO-SORRY                (proof-hole)
refused: gate exit 1 at REFINEMENT-SUBJECT      (substituted-subject)
refused: gate exit 1 at BUILD                   (unexplained-external)
refused: gate exit 1 at REQUIRED-THEOREMS       (unexecuted-coverage)
refused for the probe axiom: *CoreCmpPartialEqArray.eq._native.decide.ax_*
refused: gate exit 1 at REQUIRED-THEOREMS       (foreign-native-type)
refused for the probe axiom: *embedEnv._native.decide.ax_*
refused: gate exit 1 at REQUIRED-THEOREMS       (foreign-owner-literal)
== refusal matrix complete: baseline passes, all six mutations refused ==
```

Both scripts above were re-run on a clean tree with explicit exit capture —
`EXIT m2-proof-gate = 0`, `EXIT m2-proof-gate-refusals = 0` — and the re-run
reproduced the quoted lines.
