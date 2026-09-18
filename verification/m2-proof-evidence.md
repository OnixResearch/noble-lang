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
