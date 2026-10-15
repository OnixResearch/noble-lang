## Context

Issue #12 asks for a design and conformance plan for bounded implicit-argument elaboration over Noble's explicit source proof core. This is design only. It selects no concrete syntax, changes no code and claims no evidence. Goal display and producer helpers belong to #6 (`proof-automation-diagnostics`, committed alongside this change). Law bundles belong to #7.

### Explicit core at `44329dc` (grounded in source)

The issue links are pinned to `0ea10d3`. Between that revision and `44329dc` there is one commit. It leaves `.cairn/changes/intrinsic-noble-proofs/`, `crates/noble-contracts/src/intrinsic*`, `crates/noble-cli/src/workflow/` and `verification/intrinsic-proofs/` unchanged. It adds 76 calculator and nominal-signature test lines at line 124 of `crates/noble-contracts/tests/declared/cases.rs`, which shifts the cited test lines. It also changes calculator and declared-module signature code. All line numbers here refer to `44329dc`.

- **Propositions** are `Pi(level, Sort, body)` or `Eq(Code, level, level)`. Sorts are `Type0` codes, `Value(Code)`, or `Premise(Eq …)`. Codes are `Unit | Bool | I64 | Text | Variable(level) | Pair | Sum | List` (`crates/noble-contracts/src/intrinsic.rs:224-229`).
- **Equality operands** must be bound variable *names*. No value expression language exists (`intrinsic.rs:679-695`).
- **Proof forms** (`proof_walk`, `intrinsic.rs:864-1127`):
  - `(use NAME)` infers the premise or the accepted proof's proposition, shifted to the current context (`shifted_prop`).
  - `(apply F ARG)` is **binary**. It requires the head's outer `Pi` binder level to equal the current context depth (`apply binder scope mismatch`). ARG is parsed as a code for `Type0`, must name a bound value of *exactly* the binder's code for `Value` (`apply value has wrong code`), or is checked against the premise. The binder is then removed by capture-avoiding `replace_prop`, which rebases later levels, including inside `Pair`/`Sum`/`List` codes.
  - `(refl x)` infers `Eq(code, i, i)`.
  - `(intro (n S) body)` requires `S` to equal the goal's binder sort exactly.
  - `(subst (motive (v C) EqProp) e p)` takes an explicit motive that must be an `Eq` proposition.
- **Definitional equality** in the checker is *structural identity* of propositions after level rebasing (`AfterInfer`, `intrinsic.rs:1060-1070`). There is no normalization of codes, which have no computation, and no `Eq` symmetry or rewriting.
- **Contract proofs** are fixed reviewed-template recognizers. The revision-1 `exec-literal N` must equal the literal of the exact subject's first slice (`intrinsic.rs:1315`). The Lean term is host-generated, and the claim is `export_lean` of the contract, independent of the proof text (`crates/noble-cli/src/workflow/intrinsic.rs:404-416`).
- **Lexing.** Logical atoms admit `_`, `?` and other punctuation bytes (`crates/noble-contracts/src/source/lexer.rs:207-223`). Binder names are arbitrary atoms (`atom(&binding[0])`). A bare `_` or `?` placeholder would therefore be indistinguishable from a legal binder name. The illustrative placeholder below is a nullary *form*, `(infer)`. Binder names are atoms, so it can never collide with one. At `44329dc` it is refused in both argument roles: as a code it gives `not a predicative Type0 code`, as a value `expected logical atom`.
- **Accepted fixtures.** `(intro (x (List I64)) (apply (apply (use alias.poly-refl) (List I64)) x))` (`verification/intrinsic-proofs/fixtures.mjs:42-44`). `crates/noble-contracts/tests/declared/cases.rs:1087-1121` checks `(apply (apply (apply (apply (use base) x) (refl x)) I64) z)` with claim `(∀ (v0 : El .i64), (∀ (v1 : El (.pair (.list .i64) .i64)), v1 = v1))` and refuses the same term with a `Bool` sibling `w`.

## Decisions

### Decision: Inference fragment `implicit-args/v1`

**Choice:** Elaboration is an untrusted pass that rewrites an abbreviated proof term into the explicit core above. It admits exactly three abbreviations. The concrete spelling is illustrative and not selected.

| Abbreviation | Illustrative notation | Explicit result |
|---|---|---|
| A1. Argument placeholder at a `Type0` or `Value` binder of `apply` | `(apply F (infer))` | `(apply F c)` or `(apply F x)` |
| A2. Explicit-spine application of an exactly resolved head, with an implicit `Type0` prefix | `(app H a1 … an)` | Nested binary `apply` with every `Type0` binder of `H`'s telescope filled. `a1 … an` fill the non-`Type0` binders left to right. Each `ai` is an explicit term or `(infer)` (Value binders only). |
| A3. Structural parameter of a reviewed template rule | `(exec-literal (infer))` | `(exec-literal N)` with `N` from the exact subject slice |

The design does **not** admit the following:

- Inference at a `Premise` binder. Proof witnesses are never inferred.
- `subst` motive inference, which would need higher-order unification.
- Omission of `use` targets, bundle members or theorem names.
- Omission of `intro` binder sorts.
- Under- or over-application in A2. Every non-`Type0` position must be written, possibly as `(infer)`.
- Implicitness declared at the theorem's definition site. Such a declaration would change the declared proposition's identity, so it is out of scope.
- Inference in `contract` declarations, `def` bodies, MC1 `.noble-contract` files, or any ordinary `quote`/`compose`/`run`/`reflect`.

Abbreviated terms are admitted only under an explicit opt-in elaboration revision that is recorded with the module submission. The meaning of revision-1 and revision-2 terms is unchanged, and the explicit core continues to reject the abbreviations.

**Grammar (abstract).**

```text
t   ::= core-term | (app h a…) | (apply t a) | (exec-literal p) | …core forms
h   ::= (use QualifiedProof) | (app h a…) | (apply h a)    # inferable heads only
a   ::= core-argument | (infer)
p   ::= I64-literal | (infer)
```

**Constraint language C.**

```text
c ::= Unit | Bool | I64 | Text | Var ℓ | Pair c c | Sum c c | List c | ?α        (code metas)
v ::= ℓ | ?β:c                                                                    (value metas carry a code)
P ::= Pi ℓ S P | Eq c v v          S ::= Type0 | Value c | Premise P
K ::= c ≐ c | v ≐ v | P ≐ P | n ≐ literal(subject slice k)                      (constraints)
```

**Inference rules.** `d` is the current proof-context depth, Γ the context and Δ the premises. Constraints are generated left to right.

1. **Head.** For `(app (use T) a1…an)` or an `apply` chain, infer `T`'s proposition with `shifted_prop(goal_T, d)`, exactly as the checker does. Walk its telescope. For each `Type0` binder (A2) or each `(infer)` at a `Type0`/`Value` binder (A1), allocate a fresh meta `?k`. Substitute it with the meta-aware analogue of `replace_prop`, which uses the checker's own rebasing. For an explicit argument, substitute it as the checker would.
2. **Value argument.** An explicit value argument `x` at binder `Value(c)` contributes `sort(x) ≐ c` (rule V).
3. **Premise argument.** An explicit premise argument `t` at binder `Premise(P)` is handled in one of two ways:
   - If `t` is inferable (`use`, `refl`, `apply`, or `app`), add `infer(t) ≐ P` (rule Pr).
   - Otherwise, after solving, `P` must be ground and `t` is *checked* against it (rule Pr′).
4. **Expected goal.** In check mode against goal `G`, add `result ≐ G` (rule G). In infer mode (head position), no goal constraint is added.
5. **Template parameter.** For A3, add `?n ≐ literal(subject slice 0)`, where the literal comes from the trusted `prepare_checked_contract` (rule R).
6. **Solving.** Solve by Robinson unification. Rigid-rigid constructor equations decompose. `Pi` equations decompose binder sorts and bodies at the same level. A clash or an occurs-check failure (`?α ≐ List ?α`) is `invalid-constraint`. The only definitional equality used is the checker's structural identity, so nothing is unfolded, normalized, symmetrized or rewritten.
7. **Scope.** A solution for a meta created at depth `d` may mention only levels `< d`. A solution that mentions a binder introduced *inside* the goal or the head telescope is `ill-scoped` (capture). Value metas must be solved by a level whose sort is exactly `Value(c)` for the meta's own code.

**Uniqueness criterion.** First-order syntactic unification over finite trees has a most general unifier that is unique up to renaming of metas. The elaboration is **unique** iff all of the following hold:

- Unification succeeds.
- The mgu assigns *every* meta a ground, well-scoped term.
- Every premise position holds an explicit term.

Otherwise it is refused with one of these categories:

- `underconstrained`: some meta is unassigned or only bound to another meta.
- `invalid-constraint`: clash or occurs failure.
- `ill-scoped`.
- `unsupported`: premise placeholder, motive, under-application, or an abbreviation outside A1-A3.
- `exhausted`.
- `internal`.

The elaborator never enumerates Γ, Δ, imports or rules to *find* a value. It never applies a heuristic tie-break or a default (for example, an unused `Type0` is never defaulted to `Unit`). Import order and ambient declarations cannot influence it, because the head is an already exactly resolved identity.

**Rationale:** The explicit core has no value terms beyond variables and no computational definitional equality. First-order unification is therefore decidable, terminating and complete for the fragment, and the "single admissible reconstruction" the issue requires coincides with "ground mgu". Proof witnesses are excluded because two proofs of the same proposition can carry different transitive dependencies and assumptions, so selecting one is identity-relevant even when the propositions coincide. r[IMPL-ELAB-01]

### Decision: Explicit fallback at the source span

**Choice:** Every refusal points at the `(infer)` form, or at the `app` head for implicit `Type0` binders, and reports the following:

1. The unresolved parameter: the binder name and position in `T`'s telescope, plus `T`'s exact identity `module@version/name`.
2. The relevant constraints, rendered with spans of their origin (rule V, Pr, G or R).
3. The category.
4. Candidates, *only where useful and only listed*: for a refused premise placeholder, the Δ premises and accepted proofs whose proposition is identical to the required one, each with an identity and span; for `underconstrained` code metas, the text "any PureTyCode; no constraint mentions this binder".
5. A copy-ready explicit replacement with the missing slot shown as an editor-only hole (#6).

Refusal rejects only the opted-in proof submission. It never turns a meta into an assumption, an axiom, an accepted hole or a `sorry`. It is never reported as refutation (VC-TOOL-02 `error`/`unsupported`, never `disproved`).

**Rationale:** The issue requires actionable uncertainty and no silent choice. r[IMPL-ELAB-01]

### Decision: Shared budget model

**Choice:** Elaboration runs inside the same `Meter` and `Limits` as `check_batch_with_budgets`: 65,536 bytes, 16,384 nodes, depth 64 and 2,000,000 work, plus `ProofBudgets` normalization and substitution (`lib.rs:43-50`, `intrinsic.rs:119-126`). Every step charges the shared `work` counter. Sub-caps make the failing stage reportable.

| Stage | Charged to | Proposed cap (design parameter, unmeasured) |
|---|---|---|
| Metas allocated | `nodes` (1 per meta) | ≤ number of telescope binders visited |
| Constraint generation | `work` (size of both sides) | ≤ 4 × telescope size |
| Unification steps, including the occurs check | `normalization` | ≤ `normalization_work` |
| Meta substitution and rebasing | `substitution` | ≤ `substitution_work` |
| Dependency traversal for `use` heads | existing `check_dependency` metering | unchanged |
| Expanded term bytes | `charge` plus the source byte limit (the expansion is rechecked) | the expanded module must still fit `Limits.bytes` |
| Diagnostics | separate output cap | 16 KiB total, 16 constraints, 8 candidates, 1 KiB per rendered term, with an explicit truncation count |

Exhaustion is reported as `DiagnosticKind::Exhausted`, naming the stage. It is never acceptance and never refutation. The elaboration budget is not additive to the checker budget: elaboration plus checking together fit the one request.

**Rationale:** VC-INTRINSIC-05 requires one shared bounded admission request. r[IMPL-TRUST-01]

### Decision: Inspect/copy-expanded form, determinism and versioning

**Choice:** An elaboration record (proposed `noble-elaboration/v1`) is emitted for every abbreviated proof. For each inserted argument it lists:

- The span of the `(infer)` form or `app` head.
- The binder name and position.
- The solved code or level, rendered with its source name.
- The originating constraint spans.

It also carries:

- The exact selected theorem or rule identity (`module@version/name`, model and checker revisions).
- The instantiated claim, rendered by `lean_proposition`/`export_lean`.
- Assumptions (Δ, allowed axioms) and the transitive `use` closure.
- The full explicit term text, the elaborator revision `implicit-args/v1`, and digests of the abbreviated source, the explicit term and the claim.

"Copy expanded" yields the explicit term as ordinary core source, which the author can paste to replace the abbreviation. Fully explicit authoring stays available unchanged.

**Determinism.** The output bytes are a function of four inputs only: the source bytes, the resolved namespace snapshot, the elaborator revision and the limits. Telescope order, constraint order and decomposition order are fixed, and there is no hash-map iteration or clock input. Any change in inference behavior that could alter an expansion or a refusal requires a new elaborator revision. Evidence records the revision. A replay that re-elaborates under the recorded revision must reproduce the stored explicit term byte for byte; otherwise it refuses with `internal/elaboration-nondeterminism`.

**Source mapping.** Each node of the explicit term maps to the abbreviated span that produced it. An inserted node maps to its `(infer)` or `app` span, so checker diagnostics on the expansion point back to the author's text.

**Rationale:** This meets the issue's inspectability and reproducibility requirements. The #6 goal view shows the "inserted arguments" row directly from this record. r[IMPL-EXPOSE-01]

### Decision: Independent acceptance and identity preservation

**Choice:** The elaborator is a producer in the #6 sense. Acceptance consumes only the explicit term:

1. The existing `check_batch_with_budgets` checks it. That check revalidates every inserted code (`not a predicative Type0 code` for anything outside the closed constructor set), every value sort and every substitution.
2. `verify_with_evidence` then rechecks it in the pinned isolated Lean consumer against the claim regenerated independently from the declared proposition or contract.

The claim never comes from the elaborator. Missing witnesses, bridges, joins or unresolved metas cannot become assumptions or accepted holes. Accepted evidence stores the explicit term, so replay needs no elaborator.

Inference stays out of the following:

- Ordinary `quote`/`compose`/`run`.
- Runtime companion replay.
- Proof-required admission.

Inferring a logical `Eq` or applying a behavioral law establishes no Program, recipe or content-hash identity and authorizes no evidence reuse for a different subject (VC-ID-01, VC-REWRITE-01). Resource eligibility, applicability, artifact correspondence and host authorization stay independently enforced (VC-ADMIT-01).

**Rationale:** Trust is unchanged. The elaborator's soundness is not needed for safety; only its *determinism and refusal discipline* are user-visible claims. r[IMPL-TRUST-01]

## Worked abbreviated/expanded pairs

None of the abbreviated terms is accepted by any tool at `44329dc` (§Context). None of the explicit terms below was run through the source checker for this change unless the text cites a test that runs it. The claim strings marked [INFERENCE] are extrapolated from the `lean_proposition` rendering asserted in `cases.rs:1104-1105`.

**P1. Polymorphic reflexivity.** Context: `module logic@1` has accepted, exported `poly-refl : (Pi (A Type0) (Pi (x A) (Eq A x x)))` (the CONTRACT-17 source), imported as `alias`.

```text
abbreviated: (intro (x (List I64)) (app (use alias.poly-refl) x))
expanded   : (intro (x (List I64)) (apply (apply (use alias.poly-refl) (List I64)) x))
```

The derivation runs as follows:

1. Head telescope at `d = 1`: `Pi 1 Type0 (Pi 2 (Value (Var 1)) (Eq (Var 1) 2 2))`.
2. The implicit `A ↦ ?α` gives `Pi 1 (Value ?α) (Eq ?α 1 1)` after rebasing.
3. The explicit `x` (level 0, sort `Value (List I64)`) gives V: `?α ≐ List I64`. Substituting yields `Eq ?α 0 0`.
4. Rule G against the goal `Eq (List I64) 0 0` adds `?α ≐ List I64`, which is consistent.
5. The mgu `{?α ↦ List I64}` is ground and well-scoped, so the elaboration is unique.

The expanded text is identical to the accepted fixture `importedUse` (`fixtures.mjs:42-44`). The claim for both forms comes from the declared proposition `(Pi (x (List I64)) (Eq (List I64) x x))`, so it is the same claim by construction: `(∀ (v0 : El (.list .i64)), v0 = v0)` [INFERENCE rendering].

Variant with a context type code: inside `(intro (B Type0) (intro (y (List B)) …))`, the term `(app (use poly-refl) y)` solves `?α ↦ List (Var 0)`. That solution is well-scoped because `B` has level 0 < d.

**P2. Dependent rebasing with an explicit premise.** This pair uses the source pair from `cases.rs:1087-1105`, whose explicit form is checked by that test.

```text
base    : (Pi (x I64) (Pi (h (Eq I64 x x)) (Pi (A Type0) (Pi (y (Pair (List A) I64)) (Eq (Pair (List A) I64) y y)))))
abbrev. : (intro (x I64) (intro (z (Pair (List I64) I64)) (app (use base) x (refl x) z)))
expanded: (intro (x I64) (intro (z (Pair (List I64) I64)) (apply (apply (apply (apply (use base) x) (refl x)) I64) z)))
```

The derivation runs as follows:

1. `x` gives V: `I64 ≐ I64`.
2. `(refl x)` is inferable and gives Pr: `Eq I64 0 0 ≐ Eq I64 0 0`.
3. The implicit `A ↦ ?α` is substituted *after* the value and premise binders are removed. This is the rebasing case the checker regression covers.
4. `z` gives V: `Pair (List ?α) I64 ≐ Pair (List I64) I64`, so `?α ↦ I64`.

The claim asserted by the test for the explicit term is `(∀ (v0 : El .i64), (∀ (v1 : El (.pair (.list .i64) .i64)), v1 = v1))`. The abbreviated form's claim is the same declared proposition, hence identical.

**P3. Reviewed-rule application (CONTRACT-18 template).**

```text
abbreviated: … (pc-sequence (pc-exact (exec-literal (infer))) (pc-exact (exec-add)) (bridge (by-exact-append-assoc)) (join (by-exact-result))) …
expanded   : … (pc-sequence (pc-exact (exec-literal 1))       (pc-exact (exec-add)) (bridge (by-exact-append-assoc)) (join (by-exact-result))) …
```

Rule R takes `?n ≐ 1` from the literal that `prepare_checked_contract` extracts from the exact `increment [ 1 + ]` subject. The claim for both forms is `MC1Obligation.claim = export_lean(prepare_contract(increment-law))`. It is computed from the contract, never from the term, so the same independently checked claim applies to both. The bridge and join stay explicit: A3 never infers rule *proofs*, only the structural literal.

## Ambiguous, underconstrained and identity-selection examples

| ID | Source (abbreviated) | Outcome | Diagnostic |
|---|---|---|---|
| IMPL-CASE-05 underconstrained `Type0` | `const-refl : (Pi (A Type0) (Pi (B Type0) (Pi (x A) (Eq A x x))))`; `(app (use const-refl) x)` | refused, `underconstrained` | "binder `B` (position 2 of `logic@1/const-refl`) has no constraint; write `(apply (apply (apply (use const-refl) I64) <code>) x)`". `B` is not defaulted to `Unit`. |
| IMPL-CASE-06 two local proofs | Δ has `h1 : (Eq I64 x x)` and `h2 : (Eq I64 x x)`; `twice-refl : (Pi (x I64) (Pi (h (Eq I64 x x)) (Eq I64 x x)))`; `(app (use twice-refl) x (infer))` | refused, `unsupported: premise inference` | Candidates listed, never chosen: `h1` (span), `h2` (span), `logic@1/base` (accepted). The same refusal applies with exactly one candidate, because witnesses carry dependency identity. |
| IMPL-CASE-07 two bundles or imports | `import lawsA@1 as a`, `import lawsB@1 as b`, both exporting `assoc` for the same carrier | `(use assoc)` does not resolve to either import ([INFERENCE] from the qualified `(use a.base)` and `(use alias.poly-refl)` fixtures: imported proofs are referenced by alias-qualified names); `(app (use a.assoc) …)` elaborates with `a`'s exact identity; swapping import order changes nothing | No bundle or instance search exists. A #7 bundle member must be named through its explicit bundle or local parameter. |
| IMPL-CASE-08 meta bound only to meta | `(apply (apply (use poly-refl) (infer)) (infer))` in *infer* mode (head of another `apply`) | refused, `underconstrained` | `?α`, `?β` unassigned; no goal constraint exists in head position. |

**Rationale for a strict refusal policy.** In `implicit-args/v1`, two distinct ground solutions cannot exist (§Inference fragment). Ambiguity in the issue's sense therefore always appears as `underconstrained` or as an identity selection that the fragment never performs. A later fragment that adds definitional unfolding would need a separate uniqueness proof before admitting multiple-solution checks.

## Negative cases

All of these are refused before acceptance. In every case the explicit checker and the Lean consumer are the authorities; the elaborator's refusal is an earlier, friendlier stop.

| ID | Case | Input | Where refused | Expected diagnostic |
|---|---|---|---|---|
| IMPL-CASE-09 | Wrong value or type | P2 with `w : Bool` in place of `z` | Elaboration `invalid-constraint` (`Pair (List ?α) I64 ≐ Bool`); the explicit form is also refused by the checker (cases.rs:1107-1121) | Clash between the binder `y` sort and the `w` sort, with both spans |
| IMPL-CASE-10 | Forged inferred argument | A producer submits the "expanded" form `(apply (apply (use poly-refl) I64) x)` with `x : List I64`, or an elaboration record claiming `A := I64` | Checker: `apply value has wrong code`. Source binding: re-elaboration bytes ≠ stored expansion → `internal/elaboration-nondeterminism` | The record is never trusted; only the explicit term is checked |
| IMPL-CASE-11 | Illegal `Type0` code | Expansion inserts `(Resource test.counter)`, `(Program …)` or `Syntax` for `A` | Checker code parser: `not a predicative Type0 code` | Inference cannot produce these, because solutions are subterms of parsed codes; forged text is refused |
| IMPL-CASE-12 | Variable capture | `T : (Pi (A Type0) (Pi (x A) (Pi (y A) (Eq A y y))))`, `(app (use T) (infer))` checked against the goal `(Pi (w I64) (Eq I64 w w))` | Elaboration: `?α ↦ I64`, but `?β` (for `x`) is unconstrained; matching it to `w` would be `ill-scoped` because `w` is bound inside the goal | `underconstrained` for `x`; a name-matching heuristic is explicitly excluded |
| IMPL-CASE-13 | Incorrect binder rebasing | P2 elaborated with metas substituted *before* the value and premise binders are removed (level off by 2) | Checker re-derivation of the claim mismatches the declared proposition | The regression mirrors the checker's own rebasing test; the elaborator must reuse `replace_prop` |
| IMPL-CASE-14 | Stale theorem dependency | `poly-refl` in `logic@2` gains an extra `Type0` binder; the old expansion is replayed | The recorded identity `logic@1` ≠ the current resolution; `validate_dependency_revision` → `intrinsic-stale-proof` for changed revisions | Old evidence stays bound to `logic@1` only |
| IMPL-CASE-15 | Changed expected claim | P1 with the goal changed to `(Pi (x (List Bool)) …)` and the stored expansion reused | The claim digest differs; the stored explicit term fails the new claim | Re-elaboration produces a new expansion `A := List Bool`; nothing is reused silently |
| IMPL-CASE-16 | Unresolved proof hole | An editor proof hole at an argument or premise position | Never parsed as `.noble`; a hole is not `(infer)`; every admission path rejects it | `incomplete`; never an assumption |

## Determinism, budget and diagnostic-bound tests

| ID | Input | Expected observation |
|---|---|---|
| IMPL-CASE-17 determinism | P1–P3 elaborated 100 times across processes and with dependency modules supplied in a different order | Byte-identical explicit terms and records |
| IMPL-CASE-18 budget exhaustion | A head with 200 nested `Type0` binders (cf. `deepBinders` in `fixtures.mjs:47-51`) and `work`, `normalization` or `substitution` set just below need | `Exhausted` naming the stage; not accepted; not refuted |
| IMPL-CASE-19 bounded diagnostics | A refusal with 1,000 constraints and 50 Δ candidates | ≤ 16 KiB, ≤ 16 constraints, ≤ 8 candidates, explicit truncation count |
| IMPL-CASE-20 no runtime inference | Module with abbreviated proofs, then ordinary `run`/`quote`/`compose` of its definitions | Zero elaborator invocations; executable identity unchanged with and without proofs (VC-INTRINSIC-04) |

The positive pairs are IMPL-CASE-01 (P1), IMPL-CASE-02 (P1 with a context code), IMPL-CASE-03 (P2) and IMPL-CASE-04 (P3). Every IMPL-CASE has state `implementation: absent`, `execution: not-run`, `proof: open`, `trust: unassessed`, and no evidence.

## Evidence plan

The plan keeps three claims apart:

1. **Examples check.** For each positive pair, a source-bound receipt shows that the *explicit* term passes the source checker and the pinned Lean consumer against the independently regenerated claim, and that the abbreviated form elaborates to exactly that term. This is execution evidence about those inputs only (EV-STATE-02).
2. **Elaborator correctness.** Soundness is not required for trust. The user-visible claims are:
   - Determinism.
   - *Refusal completeness*: no silent choice. A reported unique solution is the unique ground mgu.
   - *Fragment completeness*: if A1-A3 constraints have a ground mgu within budget, elaboration succeeds.
   - Termination within budget.

   Evidence for these is a reviewed Lean proof of first-order unification uniqueness over the C term algebra, open, plus Rust refinement of the actual elaborator under the existing VT-ROLE-01 Charon → Aeneas → Lean policy, also open. Property tests are `tested-only` and never close these claims.
3. **Source-to-Lean translation.** Translation correctness stays the existing open V-INTRINSIC-01 obligation. Elaboration neither discharges nor weakens it. Lean kernel acceptance of an expanded term proves that term's claim only.

## Coordination

- **#6.** The goal view's "inserted arguments and selected rule/theorem identities" row is the elaboration record. Helpers `exact-use`/`apply-use` call this elaborator. Both refuse premise inference.
- **#7.** A bundle is selected explicitly at the use site or through a named local parameter. This fragment resolves no bundle and selects no instance. If #7 later admits omitting reconstructible bundle arguments, it must extend this record with the selected bundle identity and keep the refusal policy.
- **#10.** No interaction beyond the rule that inference confers no artifact or recipe identity.

## Risks / Trade-offs

- **The fragment is deliberately small.** It infers no witnesses and no motives, and its spine form does not support partial application. Authors still write premise proofs. This is the price of identity-safe refusal.
- **Placeholder spelling.** `_` and `?` are legal logical atoms and binder names today, so the final placeholder must be a form or a reserved token in a new grammar revision. `(infer)` is illustrative only.
- **Cost of determinism.** Any behavioral fix requires a new elaborator revision, so old records stay reproducible only under their recorded revision.
- **Unmeasured caps.** The proposed budget caps are unmeasured design parameters.

## Acceptance mapping

| Issue #12 checkbox | Section |
|---|---|
| Bounded grammar, inference rules, uniqueness criterion, explicit fallback, shared budget model | Decision: Inference fragment `implicit-args/v1`; Decision: Explicit fallback; Decision: Shared budget model |
| Abbreviated/expanded pair for polymorphic reflexivity and one reviewed-rule application with the same independently checked claim | Worked pairs P1 (and P2), P3 |
| Ambiguous and underconstrained examples, including two local proofs or bundles where identity matters | Ambiguous, underconstrained and identity-selection examples (IMPL-CASE-05 to 08) |
| Negative cases: wrong value/type, forged inferred argument, illegal resource/Program code in `Type0`, capture or incorrect rebasing, stale theorem dependency, changed expected claim, unresolved holes | Negative cases (IMPL-CASE-09 to 16) |
| Determinism, budget exhaustion and bounded diagnostic-output tests | Determinism, budget and diagnostic-bound tests (IMPL-CASE-17 to 19) |
| Evidence plan separating example checking from elaborator correctness and source-to-Lean translation | Evidence plan |
