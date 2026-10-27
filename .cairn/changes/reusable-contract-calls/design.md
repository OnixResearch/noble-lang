# Reusable contract-based named calls and explicit law bundles: design

## Context

Issue #7 asks for a bounded way to reuse independently checked callee contracts and reusable lemmas instead of re-proving inlined bodies. Its comment adds an Arend-inspired law-bundle representation. Issue #11 (`resource-safe-wiring-laws`) is its first consumer. This change is a design and evidence plan only. It changes no code, executes no scenario and promotes no proof. r[PREUSE-EVID-01]

The issue links are pinned to `0ea10d3`. This design was verified against `ffcce53`: origin/main `44329dc` plus the format and vendor re-pin commit. The #7 links carry no line anchors. `named_v2.rs` grew from 495 to 847 lines through `rustfmt`; its checks and messages are otherwise the same. `.cairn/changes/intrinsic-noble-proofs/tasks.md` is unchanged. The issue mentions "README sections on MC1/MC2 and named-v2", but `README.md` has no such sections at either commit. The relevant content is the two table rows at `README.md` L527-528, the Verification section at L548-572, and the generated `specs/PROGRAM-CONTRACTS.md` §8.1/§8.3.

### Baseline facts this design builds on

| Fact | Source (ffcce53) |
|---|---|
| `contract 2`/`proof 2 ... for` binds the accepted subject definition body. It reconstructs the named-call graph from immutable source, with distinct authenticated occurrences and fresh specialization rows. | `.cairn/specs/program-contracts/spec.md` VC-INTRINSIC-01 (L1484-1491); `.cairn/changes/intrinsic-noble-proofs/design.md` L50-58 |
| The named-v2 checker is one hard-coded profile. `verify` requires `revision == 2`, a pure `I64 -- I64` subject, exactly three use rows and exactly two source modules (L390-402). Callee bodies have one or two nodes (`checked`, L55). The ensures must be `true` / `x+2` (`selected_assertion`, L794-808). The Lean statement hard-codes callee bodies `[.lit (.i64 1), .word 4]` (L747). | `crates/noble-contracts/src/intrinsic/named_v2.rs` |
| The reviewed rule re-derives each callee from its body. `twoStepResult`/`twoStepClaim` take `bodyA : entryA.body = [.lit (.i64 1), .word 4]` as premises. `Step.call` runs the callee body directly. | `proofs/mc1/NobleContracts/NamedV2.lean` L193-256; `NamedSubject.lean` L143-144 |
| A contract theorem cannot be a `use` dependency: "contract theorem dependency is not in the pure fragment". Contract proofs return no reusable dependency. | `crates/noble-contracts/src/intrinsic.rs` L2133, L1970 |
| The logical `Proposition` is `Pi(usize, Sort, Proposition)` or `Eq(Code, usize, usize)`. Equality relates only bound variables. | `intrinsic.rs` L346-349 |
| A published proof is a `ProofDependency` record: `name, module, version, source, declaration, exported, dependencies, model_revision, checker_revision`. The CLI refuses a node whose stored revisions differ from the current pins, with code `intrinsic-stale-proof`. | `intrinsic.rs` L104-116; `crates/noble-cli/src/workflow/intrinsic.rs` L156-180, L281-322 |
| `(use alias.name)` is resolved against the current session alias map. Named-call origins instead use a frozen alias snapshot. | `.../register/logical.rs` L593-619; `.../register/logical/origin.rs` L19-61 |
| Typed sequencing needs explicit `bridge` and `join` premises. Exported claims quantify the preserved bottom tail. | `proofs/mc1/NobleContracts/Composition.lean` L28-37; `Rules.lean` L370-387; `Obligation.lean` L21-29 |
| `NamedV1.lift_run` lifts MC1 code into the named model. No NamedV1 `exec_append` or frame lemma exists. | `NamedSubject.lean` L348; grep of `proofs/mc1` |

### Design-time observations (not evidence records)

These were run once with the prebuilt `noble-44329dc-release` binary (SHA-256 `e1fb70bac358e7ffe1a064295f985448b89899c2289d8a3d514cf0cdfc708127`) on throwaway files outside the repository. They show baseline behavior only. They are not scenario executions, conformance evidence or proofs. Every scenario in §9 remains `absent/not-run/open/unassessed`.

- **O7-1.** `verify-module` on a three-call `pipeline@1` caller using `contract 2` with ensures `x+12` returns `intrinsic-contract-invalid`: "Check: selected named rule requires true and exact wrapping +2 postcondition". The same caller with ensures `x+2` returns "named v2 needs a pure unary subject and one original imported module".
- **O7-2.** `verify-module` on the §8 leaf modules `steps@1` and `offsets@2` (`contract 1`/`proof 1` for `[ 1 + ]` and `[ 10 + ]`) reaches stage `independent-recheck`. It then stops with `missing-tool` because no `NOBLE_BWRAP` sandbox is configured. A variant with a mismatched literal is refused earlier, at `acceptance`: "literal rule does not match first subject slice". The leaf source proofs therefore pass the bounded Noble source checker. No Lean recheck ran.
- **O7-3.** The §8 lemma module `arith@1`, whose `Eq` relates arithmetic terms, is refused at `acceptance` with "expected logical atom".
- **O7-4.** A definitions-only version of the §8 pipeline links and runs in `session --framed --declared-modules`:

  | Input | Result |
  |---|---|
  | `5 p.add12` | `17` |
  | `9223372036854775800 p.add12` | `-9223372036854775804` (wrapping) |
  | `[ p.add12 ] reflect` | recipe `[{"invoke":"definition:3"}]`; the named body is not inlined |

## Decisions

### Decision 1: Profile and bounded vocabulary

**Choice:** Add an opt-in `Named-Contract-Calls-v3` profile with additive declarations:

- `contract 3 Name [ subject QualifiedDefinition input [ Name Type ... ] output [ Name Type ... ] requires [ LogicalTerm ] ensures [ LogicalTerm ] ]`
- `proof 3 Name for QualifiedContract3 [ ProofTerm ]` only

The fields, order, binder grammar and MC1 logical vocabulary are identical to revisions 1 and 2. There is no colon form, no `params` field, and no cross-revision `for`. `members.rs` `revision` (L130) would accept `3` only under the profile. r[PREUSE-GRAMMAR-01]

Supported subjects are closed, nonrecursive, resource-free and pure definitions. Their accepted body is a flat ordered sequence of the following elements:

- `I64`/`Bool` literals
- pure builtins `+ - * = dup drop swap`
- named calls (`alias.word`, `Module@V.word`, or a local word) that resolve to a definition with an accepted revision-1 or revision-3 contract proof

Call interfaces use `I64` and `Bool` only, with at most four inputs and four outputs. Assertions use the MC1 subset `true false not and eq lt le` over `(in x)`, `(out y)`, literals and wrapping `add sub mul`. Excluded are `maps`, `param`, `definition`, pair/sum/list terms, quotations, `run`, `compose`, `dip`, `case`, `if`, `reflect`, effects, resources and Program-typed slots. Excluded forms are **refused as Unsupported, never inlined**.

Reusable lemmas need one logical extension: `(Eq I64 T U)` where `T` and `U` are bounded scalar terms (variables, `I64` literals, `add`/`sub`/`mul`). Today `Eq` relates only variables (O7-3). Arithmetic laws enter only as new reviewed rule identities with exact Lean declarations, never as axioms:

| Rule identity | Lean declaration |
|---|---|
| `bv-add-assoc` | `BitVec.add_assoc` (already used inside `NamedV2.twoStepResult`) |
| `bv-add-comm` | `BitVec.add_comm` |
| `bv-mul-add` | `BitVec.mul_add` |

Bounds are charged to the existing shared `Meter`/`ProofBudgets` (VC-INTRINSIC-05):

| Bound | Limit |
|---|---|
| Call occurrences per subject | 16 |
| Distinct callee proofs | 8 |
| Proof-DAG depth | 4 |
| Lemma uses | 16 |
| Nodes per assertion | 256 |
| Symbolic stack depth | 16 |
| Generated Lean statement | the existing `limits.work`/`limits.bytes*128` bound (`named_v2.rs` L760-766) |

**Rationale:** This is the smallest generalization that gives several reusable callees, multi-step composition and real lemma reuse. It stays inside the reviewed NamedV1 normal-return model and the MC1 predicate vocabulary. Program-valued operands belong to #11, and resources to #3.

### Decision 2: Exact subject and dependency envelope

**Choice:** Every caller proof carries an envelope. The consumer rebuilds it independently. Producer-supplied fields are never trusted. Digests are integrity indexes under VC-ID-03, not authority. r[PREUSE-ENVELOPE-01]

| Record | Fields (all compared exactly) | Existing anchor |
|---|---|---|
| `SubjectRef` | module name, version, full source bytes (SHA-256 kept only as an index); definition name, lexical ordinal, definition span and bytes; accepted candidate body (format, `SEMANTIC_REVISION`, nodes, Inst); complete ordered interface `in -- out ! {}` | Lean `NamedV1.Provenance` (L16-23) plus `Entry` body and interface |
| `ContractRef` | owning module identity, contract name, revision (1 or 3), `SubjectRef`, ordered binders, canonical `requires`/`ensures` forms, claim kind `normal-return-pc`, regenerated claim text (index digest) | `ContractGoal` (`intrinsic.rs` L80) |
| `ProofRef` | module identity, proof name, revision, declaration bytes and span, `for` `ContractRef`, exported flag, `model_revision`, `checker_revision`, rule-library revision, accepting policy, ordered `dependencies : [ProofRef]`, assumptions | extends `ProofDependency` (`intrinsic.rs` L104-116) |
| `CallOccurrence` | source-order ordinal `k`; caller `SubjectRef`; source node; exact full-token span (`alias.word`, not just `alias`); spelled scope (local, `Module@V`, or alias); frozen import-snapshot entry; resolved callee `SubjectRef`; fresh specialization slot; instantiated interface; callee `ProofRef`; regenerated expected callee claim | `NamedUseOrigin` (`intrinsic.rs` L39); `verify_named_call_origin` (`source/declared.rs` L214-282) |
| `LemmaUse` | site span, pure revision-1 proposition `ProofRef`, explicit ordered arguments (Type0 codes or bounded scalar terms), instantiated statement | `use`/`apply` (`intrinsic.rs` L1404-1460) |
| `Envelope` | caller `ProofRef` and `ContractRef`, `[CallOccurrence]`, `[LemmaUse]`, closure (topologically ranked `ProofRef` set), assumption union, model/checker/rule revisions, consumer policy | new |

The binding rules are as follows.

1. An occurrence's callee `SubjectRef` must equal both of these: the definition resolved through the **frozen** import snapshot, and the subject of the callee proof's `for` contract. Display names, import aliases and claimed hashes grant nothing.
2. Repeated occurrences of one callee share its `ProofRef`. Each still has its own occurrence identity, span, ordinal and specialization slot.
3. The closure is finite and acyclic, with ranks strictly descending. This mirrors `no_recursive_named_call` (`NamedSubject.lean` L295) and the `cyclic proof dependency` refusal (`intrinsic.rs` L2037).
4. Callee assumptions join the caller's assumptions. They must be a subset of the consumer-selected allowed set (VC-EVIDENCE-01). The first profile allows none beyond the reviewed library.
5. `use` resolution inside revision-3 proofs uses the same frozen snapshot as call resolution. It does not use the latest session alias.

**Rationale:** These are the bindings VC-ID-02, VC-EVIDENCE-02 and VC-VALUE-03 require: subject, captures and instantiation, dependencies, semantic context, claim and assumptions. The design reuses existing records instead of creating a parallel identity.

### Decision 3: Contract-based call rule

**Choice:** Revision-3 checking is a deterministic symbolic execution. The checker owns the symbolic state; the proof term supplies only bridges, joins and lemma uses. r[PREUSE-CALL-01]

The state is `σ = (τ, ⟨t1 … tn⟩, Φ, G)`:

- `τ` is the opaque bottom tail.
- `⟨t1 … tn⟩` is the complete ordered symbolic stack, with the top last.
- `Φ` is the set of facts.
- `G` is the set of fresh ghost names.

Every step updates the state as follows.

- **Initial state.** The stack holds the input binders `⟨x1 … xm⟩`. `Φ = {requires(x⃗)}`.
- **Primitive steps.** A literal pushes a constant. `+`, `-` and `*` push the wrapping term. `=` pushes a Bool term. `dup`, `drop` and `swap` permute or copy terms of `Data` scalar sort. Each step is justified by its NamedV1 `Step` constructor, the analog of the `Rules.lean` `exec_*` lemmas.
- **Call `CALL(k)`.** Let callee `k` have interface `I1 … Ip -- O1 … Oq`, `requires P_k` and `ensures Q_k`.
  1. The top `p` stack terms `a⃗` must have exactly the sorts `I1 … Ip` in order. This is the complete ordered interface: there is no suffix or count matching, and reordering requires explicit `swap`.
  2. The proof supplies a bridge `β_k : Φ ⊢ P_k[in ↦ a⃗]`.
  3. The checker pops `a⃗`.
  4. For each output `j`: if `Q_k` has a top-level conjunct `(eq (out y_j) U)` with no `out` in `U`, the checker pushes `U[in ↦ a⃗]`. Otherwise it pushes a fresh ghost `g_{k,j}`.
  5. Finally `Φ := Φ ∪ {Q_k[in ↦ a⃗, out ↦ r⃗]}`.
  
  The tail `τ` and every stack term below `a⃗` are unchanged. This is justified because `exportedNamedClaim` quantifies over `tail` (NamedV2.lean L20-28).
- **Final state.** The stack must have exactly the caller's output sorts. The proof supplies a join `γ : Φ ⊢ ensures[in ↦ x⃗, out ↦ final terms]`.

Substitution is capture-avoiding by construction:

- Callee binders are local and α-renamed through the existing de Bruijn `Proposition` representation.
- Ghosts are named by (occurrence ordinal, output index) and cannot collide with caller `in` names.
- Binder rebasing follows the existing fix in `intrinsic-noble-proofs/design.md` L66.

The intermediate snapshot (`in` terms), the ghost set and `Φ` persist across all steps. Nothing is dropped except through explicit `drop` of a term.

Strength stays at the original level. The resulting claim is normal-return partial correctness, which is the weakest component kind. It never implies termination, finite-prefix safety, quota success or Wasm correspondence (VC-COMPOSE-01, K-ERROR-03).

Callee opacity does not hide anything the consumer must see. The proof sees only the callee contract. Independently of the proof, the consumer:

- re-accepts each callee definition with the kernel and derives its effect bound, which must be `{}`, and its resource-freedom;
- records each callee's correspondence status, for example a source-bound receipt versus no recheck, and reports the composite as the minimum;
- carries the callee assumptions forward.

New reviewed Lean obligations (none exist today):

| Id | Statement shape |
|---|---|
| T1 `NamedV3.exec_append_iff` | NamedV1 analog of `Rules.exec_append_iff` (L219) |
| T2 `NamedV3.callFrame` | `exportedNamedClaim callee [] P Q` and an entry embedding give: for all `frame args final`, if `args` is typed at `entry.input`, `P args [] []` holds, and `Exec env [.call use] (frame ++ args) final`, then `∃ res, final = frame ++ res`, `res` is typed at `entry.output`, and `Q args res []`. |
| T3 `NamedV3.embed` | Callee-subject environment rows embed into the caller environment under an injective slot renaming that preserves origin, body and interface. Holds₂ is environment-independent for `maps`-free terms. |
| T4 `NamedV3.liftMc1Claim` | MC1 `exportedClaim` for code `c` implies `exportedNamedClaim` for an entry whose body is `liftCode c` (via `NamedV1.lift_run`, L348). |
| T5 `NamedV3.chainClaim` | A list of step certificates, bridges and a join implies `exportedNamedClaim caller [] pre post`. Generalizes `twoStepClaim`. |
| T6 | `Holds₂` evaluation lemmas for the revision-3 vocabulary (analogs of `holds_true`/`holds_eq`, `Expression.lean` L65-85) |

**Rationale:** The rule needs no callee body. That makes callees reusable across callers without re-proof. Intermediate implications stay explicit, matching the shape of `pc_sequence`/`TypedPC.compose`.

### Decision 4: Lemma application is not program invocation

**Choice:** There are two disjoint sorts. Logical judgments `Γ;Δ ⊢ t ⇐ P` range over pure Type0 values. Program call slices `(pc-call k QualifiedProof Bridge)` consume a contract claim about a program subject. r[PREUSE-LEMMA-01]

- `(use Q)` inside a bridge or join may name a pure revision-1 proposition proof or a local fact. Facts in `Φ` are exposed as `Δ` premise binders `f0 f1 …` in insertion order.
- `(pc-call …)` may name only a revision-1 or revision-3 contract proof whose subject equals the resolved occurrence.
- Crossing the two sorts fails with `preuse-sort-mismatch`.

`(apply P arg)` instantiates with an explicit Type0 code or bounded scalar term and capture-avoiding substitution. Instantiation never executes a program and never creates a program claim. A lemma about wrapping `add` says nothing about a definition `inc` until that definition's contract is joined in through `pc-call`.

**Rationale:** VC-BUILD-03 and VC-INTRINSIC-03 forbid deriving program behavior from a type or a self-chosen proposition. Separate sorts make that a checker invariant.

### Decision 5: Bounded checking algorithm and dependency replay

**Choice:** The consumer takes the caller source bytes, the immutable store of previously accepted modules, the import snapshot, and its own policy (allowed assumptions, pinned model/checker/rule revisions, budgets). It then runs these steps. r[PREUSE-CHECK-01]

1. **Parse and resolve.** Use the existing grammar extended for revision 3. Unsupported forms are refused (Unsupported).
2. **Kernel acceptance.** Kernel-accept the caller definition. Refuse any body node outside Decision 1.
3. **Authenticate occurrences.** Use the existing machinery: `origin::trace`, `verify_named_call_origin`, `verify_named_export` (`source/declared.rs` L287-355) and the frozen snapshot. Build `[CallOccurrence]` in source order.
4. **Match proof slices.** Match each `(pc-call k Q β)` to occurrence `k`. The index must equal the ordinal, following the `named_v2.rs` L825 precedent. `Q` must resolve through the snapshot to an exported, accepted contract proof whose subject equals the occurrence callee. Its revisions must equal the pins, as in `validate_dependency_revision`.
5. **Walk the closure.** Walk `ProofRef.dependencies` with rank checking and the following refusals:

   | Condition | Outcome |
   |---|---|
   | Cycle | Invalid |
   | Missing or private dependency | Invalid, `preuse-dependency-missing` |
   | Stale revision | Unsupported, `intrinsic-stale-proof` |
   | Budget exceeded | Exhausted |

6. **Regenerate claims.** Regenerate every callee and lemma claim from its immutable source, not from producer text, and compare it with the `ContractRef`.
7. **Check proof terms.** Run Decision 3 symbolically. Check each bridge and join with the existing bounded bidirectional checker, extended by Decision 1. Charge all substitution and normalization.
8. **Emit the Lean obligation module.** It contains the caller `NamedV1.Subject` with fresh rows (the generalization of `statement`, L735-769), the replayed callee theorems, and the regenerated caller claim. The proof lowers to a `NamedV3.chainClaim` instance whose callee arguments are theorem constants.
9. **Replay dependencies.** For every distinct `ProofRef` in the closure, deduplicated and in topological order, regenerate its obligation and re-check its retained proof term. This happens in the same isolated strict Lean session, which allows only `propext`, `Classical.choice` and `Quot.sound` (`crates/noble-cli/src/workflow/verification/named_v2.rs` L84). The first profile has **no** acceptance cache that skips replay. Caching by exact identity would be a later profile, gated by an additional ledger-to-theorem obligation (Decision 10, C4).
10. **Check the final claim and publish.** Strict exact-type-check the caller theorem against the regenerated claim. Publish only through `commit_verified` (`state/prepare.rs` L270), storing the envelope.

Outcomes are distinct and never reported as proved:

| Outcome | Kind |
|---|---|
| accepted | — |
| invalid | `Invalid` |
| unsupported | `Unsupported` |
| exhausted | `Exhausted` |
| internal | `Internal` |
| recheck unavailable | e.g. `missing-tool`, as in O7-2 |

Work is linear in body nodes plus the summed term sizes plus the closure size. Each is bounded by Decision 1, so checking is decidable and bounded.

**Rationale:** Replay keeps the independent-checker boundary of VC-INTRINSIC-03 and VC-LIB-04. The checker never accepts a callee merely because a record says it was accepted.

### Decision 6: Invalidation and diagnostics

**Choice:** Applicability is a function of the complete envelope. r[PREUSE-CHECK-01]

| Change | Effect | Diagnostic |
|---|---|---|
| Caller source bytes | New subject. Old evidence stays bound to the old bytes. | `intrinsic-proof-refused` if the old term no longer checks |
| Callee module version (`offsets@3`) imported by a new caller version | Occurrences resolve differently, so the old envelope is inapplicable. Saved old definitions keep their old evidence. | `preuse-callee-mismatch` |
| Alias rebinding after acceptance | No effect on the accepted, frozen envelope. A new submission resolves anew. Replaying the old envelope under the new snapshot is refused, as in the CONTRACT-26 stale-`@5` precedent. | `preuse-callee-mismatch` |
| Callee contract statement changes under the same name | New `ContractRef`; dependents refuse. | `preuse-callee-claim-changed` |
| Model, checker or rule-library revision | Transitively stale | `intrinsic-stale-proof` |
| Consumer policy (assumptions, budgets) | Applicability is revalidated (VC-VALUE-03). | `preuse-policy-changed` |
| Lemma module | Treated like a callee change | `preuse-dependency-missing` / `preuse-callee-claim-changed` |
| Live-slot replacement | Out of scope; governed by #1 (VC-LSLOT-01) | — |

Each diagnostic carries its kind, source span, occurrence ordinal, the unmet obligation (bridge, join, identity field or budget) and the expected versus actual identity field. A diagnostic never claims the target proposition is false. For example, an unproved bridge is "not established", not "refuted".

### Decision 7: Worked three-stage example

The example has two separately exported callees from different modules (`steps@1.inc`, `offsets@2.add10`), one reusable lemma (`arith@1.add-assoc-wrap`), and a repeated callee occurrence. Source units are separate submissions, as in CONTRACT-26 (`specs/conformance/contract-cases.json` L1223-1225).

Units U1 and U2 use only syntax accepted at `44329dc`. They passed the bounded source checker, stopping at the unavailable Lean recheck (O7-2):

```noble
module steps@1 [
  export inc
  export inc-law
  export inc-correct
  def inc [ 1 + ]
  contract 1 inc-law [ subject inc
    input [ x I64 ] output [ y I64 ]
    requires [ true ]
    ensures [ (eq (out y) (add (in x) 1)) ] ]
  proof 1 inc-correct for inc-law
    [ (export-unary-I64
        (intro (tail Stack) (intro (x I64)
          (pc-sequence (pc-exact (exec-literal 1))
            (pc-exact (exec-add))
            (bridge (by-exact-append-assoc))
            (join (by-exact-result)))))
        (mc1-true-eq-wrap)) ]
]
```

`offsets@2` is the same text with `add10`, `[ 10 + ]`, `(add (in x) 10)` and `(exec-literal 10)`.

U3 is the lemma. It is **proposed and refused today** with "expected logical atom" (O7-3). It needs the Decision 1 `Eq` extension and the reviewed rule `bv-add-assoc`:

```noble
module arith@1 [
  export add-assoc-wrap
  proof 1 add-assoc-wrap :
    [ (Pi (a I64) (Pi (b I64) (Pi (c I64) (Eq I64 (add (add a b) c) (add a (add b c)))))) ]
    [ (intro (a I64) (intro (b I64) (intro (c I64) (bv-add-assoc a b c)))) ]
]
```

U4 consists of three separate import units: `import steps@1 as s`, `import offsets@2 as o` and `import arith@1 as ar`.

U5 is the caller. It is **proposed and unchecked**: `contract 3`, `proof 3`, `pc-call`, `by-true`, `export-named-claim` and term arguments to `apply` do not exist yet. Its `def` line ran as O7-4:

```noble
module pipeline@1 [
  export add12
  def add12 [ s.inc o.add10 s.inc ]
  contract 3 add12-law [ subject add12
    input [ x I64 ] output [ y I64 ]
    requires [ true ]
    ensures [ (eq (out y) (add (in x) 12)) ] ]
  proof 3 add12-correct for add12-law
    [ (export-named-claim
        (pc-call 0 s.inc-correct (by-true))
        (pc-call 1 o.add10-correct (by-true))
        (pc-call 2 s.inc-correct (by-true))
        (join (subst (motive (z I64) (Eq I64 (add (add (add x 1) 10) 1) z))
                (apply (apply (apply (use ar.add-assoc-wrap) x) 1) 11)
                (apply (apply (apply (use ar.add-assoc-wrap) (add x 1)) 10) 1)))) ]
]
```

The symbolic trace below is what the checker derives, not what the producer supplies. Revision-1 callees enter through T4.

| Step | Occurrence (span is the full token) | Callee `ProofRef` | Stack after | Facts added |
|---|---|---|---|---|
| start | — | — | `⟨x⟩` | `true` |
| 0 | `s.inc`, ordinal 0, fresh slot A | `steps@1.inc-correct` (rev 1, lifted) | `⟨add x 1⟩` | `y0 = x+1` |
| 1 | `o.add10`, ordinal 1, fresh slot B | `offsets@2.add10-correct` (rev 1, lifted) | `⟨add (add x 1) 10⟩` | `y1 = …+10` |
| 2 | `s.inc`, ordinal 2, fresh slot C ≠ A | `steps@1.inc-correct` (same `ProofRef`, new occurrence) | `⟨add (add (add x 1) 10) 1⟩` | `y2 = …+1` |

Every bridge is `true`, so `(by-true)` suffices.

The join goal is `(Eq I64 (add (add (add x 1) 10) 1) (add x 12))`. It is proved by `subst` from two instances of the same lemma:

- `p2 = add-assoc-wrap x 1 11 : Eq (add (add x 1) 11) (add x (add 1 11))`, which normalizes to `… (add x 12)` by bounded closed-literal computation.
- `p1 = add-assoc-wrap (add x 1) 10 1 : Eq L (add (add x 1) (add 10 1))`, which normalizes to `Eq L (add (add x 1) 11)`.

With the motive `M(z) = Eq L z`, `subst` takes `p2 : Eq A B` and the premise `p1 : M(A)` and yields `M(B)`, the goal. `export-named-claim` then rebuilds the typed `Holds₂` boundary, generalizing `named-true-eq-wrap`.

The expected exact claim is `NamedV2.exportedNamedClaim pipelineSubject [] (Holds₂ true) (Holds₂ (eq (output 0) (add (input 0) (i64 12))))`, under BitVec64 wrapping and over every tail. The envelope closure is `{steps@1.inc-correct, offsets@2.add10-correct, arith@1.add-assoc-wrap}`. Each element is replayed once, though `inc-correct` serves two occurrences. Assumptions are none.

Non-claims: no termination, quota success, Wasm/artifact correspondence or owner-frozen law.

### Decision 8: Scenario designs (unexecuted)

Every row has state `implementation: absent`, `execution: not-run`, `proof: open`, `trust: unassessed`, and no evidence. Variants are relative to Decision 7 unless stated. Expected observations for refusals include zero guest requests and zero protected operations (EV-CASE-02).

| ID | Covers | Input | Expected observation |
|---|---|---|---|
| PREUSE-S01 | positive multi-callee | Decision 7 unchanged | Accepted. The exact claim is `y = x+12` (wrapping). The closure has three `ProofRef`s, and `inc-correct` is replayed once. Proof-free and proof-bearing builds give the same `add12` recipe, DefinitionId and Wasm. |
| PREUSE-S02 | intermediate predicate, positive | `guard@1.dec` = `[ 1 - ]` with `requires (lt 0 (in x))` and `ensures (eq (out y) (sub (in x) 1))`. Caller `def dec-inc [ g.dec s.inc ]` requires `(lt 0 (in x))`; bridge `(use f0)`. | Accepted. The bridge is an exact fact match. |
| PREUSE-S03 | intermediate predicate, negative | Caller `def inc-dec [ s.inc g.dec ]` requires `(le 0 (in x))`. The bridge needs `0 < x+1`, which is false at `x = 9223372036854775807` under wrapping. | `preuse-bridge-unproved` ("not established"). No revision-3 rule derives it. Variant S03b, with requires `(lt 0 (add (in x) 1))`, is accepted. |
| PREUSE-S04 | reordered stack slots | `diff@1.sub2 [ - ]` takes `input [ a I64 b I64 ]` and ensures `z = a - b`. Caller `def rsub [ swap d.sub2 ]` claims `z = b - a`. | Accepted. Variant (a): `swap` omitted with the same claim → join refused. Variant (b): a producer-supplied argument binding that maps callee `a` to the wrong slot → refused, because bindings are derived by the checker. |
| PREUSE-S05 | repeated occurrences | (a) only two `pc-call` slices; (b) slice indices 2 and 0 swapped; (c) occurrences 0 and 2 forced into one specialization slot; (d) occurrence 2 bound to `o.add10-correct` | (a) missing slice; (b) "call slice does not match source order"; (c) slot-reuse refusal; (d) `preuse-callee-mismatch` |
| PREUSE-S06 | alias rebinding | After S01, `import steps@2 as s` where `steps@2.inc = [ 2 + ]`. Resubmit identical caller bytes as `pipeline@2`, and separately replay the `pipeline@1` envelope. | Fresh submission: join or claim refusal. Old-envelope replay under the new snapshot: `preuse-callee-mismatch`. The accepted `pipeline@1` evidence is unchanged. |
| PREUSE-S07 | changed callee version | `offsets@3.add10 [ 9 + ]` with claim `+9`. A new caller imports `@3` with the unchanged proof, then with an updated join for `+11`. | Unchanged proof refused. Updated proof accepted for the new subject only. |
| PREUSE-S08 | forged proof | The callee `ProofRef` declaration bytes are altered (literal `2`) while the claimed digest is kept; separately, claim text is substituted. | Regeneration or replay fails, so the caller is refused. The digest grants nothing. |
| PREUSE-S09 | stale proof | Callee accepted under an older named model revision | `intrinsic-stale-proof` (Unsupported) |
| PREUSE-S10 | missing dependency | `ar.add-assoc-wrap` unexported, or `arith@1` not supplied | `preuse-dependency-missing` (cf. "unknown or private imported proof", `logical.rs` L715) |
| PREUSE-S11 | dependency cycle | A forged dependency record with `p → q → p`; separately `def loop [ loop ]` | "cyclic proof dependency"; nonrecursive-subject refusal |
| PREUSE-S12 | sort separation | `(use s.inc-correct)` in a join; `(pc-call 0 ar.add-assoc-wrap …)` | `preuse-sort-mismatch` for both |
| PREUSE-S13 | effectful callee | The callee is the `fx@1.say` emitter (`require emit Text -- ! test.emit`). | Unsupported (`preuse-unsupported-callee`); zero host requests |
| PREUSE-S14 | resource interface | A callee or caller that mentions `Resource<test.counter>` | Unsupported |
| PREUSE-S15 | no inlining fallback | A caller body containing `[ s.inc ] run`, or a `pc-call` slice that supplies `(exec-literal 1) (exec-add)` instead of a callee proof | Unsupported or refused. The body is never inlined. |
| PREUSE-S16 | budgets | 17 occurrences; DAG depth 5; an oversized assertion | Exhausted, distinct from Invalid |
| PREUSE-S17 | strength | The report for S01 | The claim kind is `normal-return-pc`, and the report states no termination or quota claim. |
| PREUSE-S18 | erasure | S01 compiled with optimization off and on, with and without the `contract 3`/`proof 3` members | Identical executable identities |
| PREUSE-S19 | recheck unavailable | S01 with no sandbox configured | `missing-tool`, not accepted (cf. O7-2) |
| PREUSE-S20 | owner law | The candidate weakens ensures to `true` | A valid theorem, but refused by the VC-OWNER-01 owner-frozen comparison |
| PREUSE-S21 | revision crossing | `proof 3` for a `contract 2`; `proof 2` for a `contract 3` | "proof and contract revisions differ" (cf. `logical.rs` L101) |
| PREUSE-S22 | ambiguous lemma | `import arithA@1 as a` and `import arithB@1 as b`, both exporting `add-assoc-wrap`; an unqualified `(use add-assoc-wrap)` | Refused. The qualified form is accepted with the selected identity shown. |

### Decision 9: Correspondence obligations (separate from finite tests)

Passing PREUSE-S01 through S22 is finite regression evidence. It never discharges any obligation in the table below. r[PREUSE-EVID-01]

| Id | Obligation | Route | Ledger link |
|---|---|---|---|
| C1 | Each accepted source occurrence maps to exactly one NamedV1 `.call use` with the right slot, provenance and Inst. The graph has no orphan or extra rows. | Aeneas → Lean refinement of `origin::trace`/`verify_named_call_origin` | PO-19, V-INTRINSIC-01 |
| C2 | Regenerated caller and callee claims equal the elaborated source contract semantics (`Holds₂` of the source predicate) | lean-and-aeneas contract export | PO-19 |
| C3 | The Rust symbolic executor and bridge checker establish exactly the premises of `NamedV3.chainClaim`, with capture-avoiding substitution | Aeneas, VC-LIB-04 | PO-20 |
| C4 | An accepted envelope implies a strict Lean theorem of the exact caller claim with every closure member replayed. Any future cache needs its own proof that a ledger lookup implies the theorem. | lean-and-rust | PO-16, PO-20 |
| C5 | T1-T6 (embedding, lifting, frame, chain) are proved in the reviewed library under the gate's axiom list (`proofs/mc1/ContractGate.lean` L36-39) | lean-kernel | PO-15 |
| C6 | Erasure: `proof 3` members leave caller and callee recipe, DefinitionId and Wasm unchanged | lean + implementation test | PO-20, VC-INTRINSIC-04 |
| C7 | NamedV1 semantics is adequate to the language semantics for the revision-3 fragment | lean-kernel | PO-15, V-AUTH-01 |
| C8 | The executed caller and callee artifacts correspond to the checked subjects | #10 | PO-17, PO-18 |

**Bounded C8 follow-on (#52):** the [one-call linking pilot](../../../verification/linking/README.md)
uses the existing same-artifact named-call path, an explicit engine-owned
operand/continuation ABI model, conditional composition rules and finite
source/dependency/final-byte controls. Actual lowering and loading remain
premises; C8 and PO-17/18 remain open. This does not implement revision-3 calls,
promote PREUSE scenarios, or reopen the completed #7 design.

### Decision 10: Migration plan

**Choice:** Additive only.

- Revisions 1 and 2 keep their grammar, checker paths, Lean theorems (`twoStepClaim`) and receipts unchanged: CONTRACT-18, CONTRACT-26 (`verification/intrinsic-named-v2/acceptance.json`), MC1 `.noble-contract` and MC2 companions. `NamedV2.lean` is not edited. New theorems live in a separate `NamedV3` module, imported only by the revision-3 consumer.
- Revision-1 contract proofs become **callable** through T4 without re-proof. They stay non-`use`-able as lemmas (L2133 is unchanged).
- A revision-2 `twice` remains valid under revision 2. A revision-3 restatement is a new proof for the same subject and changes no DefinitionId (VC-ID-01).
- Every unsupported case is refused as Unsupported or Invalid. The checker never inlines a callee body, never infers a missing callee proof, never weakens an ensures, and never falls back to revision 2.
- MC2 evidence-companion admission for revision 3 is a later gate.
- Implementation order (future tasks):
  1. grammar and frozen-snapshot `use`;
  2. Lean T1-T6;
  3. envelope, symbolic checker and replay;
  4. the `Eq`-over-terms extension and reviewed BitVec rules;
  5. scenario registration and execution;
  6. correspondence C1-C8.

### Decision 11: Explicit law bundles

**Choice:** A law bundle is an immutable, explicitly selected *index* over already-accepted identities. It is not a new proof authority, class system or instance search. r[PREUSE-BUNDLE-01]

#### Minimal schema

| Field | Content |
|---|---|
| `identity` | module name and version, bundle name, declaration bytes and span |
| `params` | ordered parameters with sorts: `Type0` code, `Stack`, or program code with an exact ordered interface |
| `operations[]` | name, exact identity (a builtin word id such as `compose` = word 9, or a `SubjectRef`), complete ordered `in -- out ! effects`, recipe |
| `profile` | semantic model revision (e.g. the `named_model_revision()` digest), rule-library revision, checker revision |
| `laws[]` | name; theorem (`ProofRef`, or a reviewed rule naming an exact Lean constant and library revision); instantiated statement over `params`; required explicit arguments; observation relation id; strength; scope flags for normal return, traces, resources, failure/cleanup and quota, each `established` or `not-established`; finite dependency closure |
| `assumptions[]` | explicit, consumer-allowed only |
| `resource_premises[]` | every `Data(a)`/`Capture(a)` or move-only requirement each law needs. These are checked by the kernel at each instantiation, never granted by the bundle. |

The source form is **proposed and unselected**. It is listed only to make the record concrete; a manifest record attached to a module is the alternative:

```text
bundle 1 Name [ params [ (p Code) ... ] operations [ op Name = Identity ... ]
  laws [ law Name : [ Proposition ] by QualifiedProofOrRule observe ObsId scope [ ... ] ... ] ]
```

Selection is explicit, in one of two ways:

- at the use site: `(law Q.bundle-name law-name arg ...)`
- through a named local parameter: `(with-bundle (B Q.bundle-name) ... (law B law-name ...))`

Elaboration resolves selection to immutable operation and theorem identities. Those identities, and never the bundle name, are what enter the envelope (Decision 2).

#### Bounded pure example: `seq@1.code-monoid`

The example is mapped to existing checked rules. It is pure, MC1-v1, normal return only.

| Element | Binding | Status at `44329dc` |
|---|---|---|
| op `compose` | builtin word 9; `Step.compose` produces `.program (p ++ q)` (`Model.lean` L72-73) | existing |
| op `id` | the empty program `[ ]`; `exec_nil_iff` (`Rules.lean` L214) | existing |
| law `seq` | `TypedPC.compose` (`Composition.lean` L28-37), with `pc_sequence` (`Rules.lean` L378) premises `bridge` and `join` | existing, required by `ContractGate.lean` |
| law `assoc` | `List.append_assoc`, already whitelisted (V-INTRINSIC-01; `Rules.lean` L5), with `exec_append_iff` (L219) | existing |
| law `left-id` / `right-id` | `List.nil_append` / `List.append_nil` with `exec_append_iff` | Lean core; **must be added to the reviewed whitelist** |
| observation | `OBS-N`: `Exec` equivalence and `PC` (`Rules.lean` L366) | established in the model |
| recipe scope | P-RECIPE-03 (spec) | specified; PO-08 still open; not proved |
| traces, resources, failure, quota | `not-established`. MC1 has no host operation (`host_unsupported`, `Rules.lean` L178), no resource type and no quota. | — |

#### Worked reuse

A consumer has an accepted claim for the grouping `p ∘ (q ∘ r)` and needs it for `(p ∘ q) ∘ r`. It writes `(law seq.code-monoid assoc p q r)`. The elaborated, inspectable result is:

| Binding | Value |
|---|---|
| operation | `compose` = builtin word 9 (exact id), interfaces `A→B`, `B→C`, `C→D` |
| theorem | `List.append_assoc` at the pinned Lean core revision, via `exec_append_iff` |
| instantiated statement | `Exec ((p ++ q) ++ r) s t ↔ Exec (p ++ (q ++ r)) s t`, where `p q r` are the exact code lists of the resolved subjects |
| profile | MC1-v1 model digest, checker revision |
| observation | `OBS-N` (normal-return) |
| assumptions | none |
| resource premises | none (pure fragment) |

In this model the two code lists are *propositionally equal*. The reuse is therefore justified by equality of the modeled subject, consistent with P-RECIPE-03. It is not justified by "same results". The executed compose tree still differs, so artifact correspondence remains #10's.

#### Negative cases

All refuse before acceptance.

| Case | Input | Diagnostic |
|---|---|---|
| mismatched operation | `def compose2 [ compose ]` substituted for builtin `compose`. It is behaviorally equal but has a different identity. | `preuse-bundle-operation-mismatch` |
| stale theorem or module | Bundle `seq@1` built under model revision M1; current pin is M2 | `intrinsic-stale-proof` |
| missing dependency | `left-id` cited while `List.nil_append` is not whitelisted | `preuse-dependency-missing` |
| incompatible observation | Consumer needs `OBS-T` (trace) or `OBS-Q` (quota); the bundle supplies `OBS-N` | `preuse-observation-mismatch` |
| resource eligibility | A bundle declaring a "copyable"/fanout law over a carrier containing `Resource<…>`; or a law instance at a resource type | Bundle admission recheck refuses; the kernel independently refuses the `dup` instance (`required S Data`, as observed for #11's `fanout`) |
| exact-subject transfer | A law proved for `[ 1 1 + ]` applied to `[ 2 ]` because the results are equal | `preuse-subject-mismatch` (V-OBS-01, P-RECIPE-04) |

#### Ambiguous two-bundle example and import-stability tests

`import seqA@1 as a` and `import seqB@1 as b` both export `code-monoid` for builtin `compose`. `seqA` declares `OBS-N` only. `seqB` declares `OBS-N` plus a recipe scope.

- **Unqualified selection.** `(law code-monoid assoc …)` is refused with `preuse-ambiguous-bundle`, listing both identities. `(law a.code-monoid assoc …)` is accepted, and the report shows `seqA@1`.

The following tests are designed and unexecuted:

| Test | Expected |
|---|---|
| (i) Permute the two import orders. | Byte-identical elaborated identities |
| (ii) Add an unrelated `import misc@1 as m` that exports another bundle named `code-monoid` over a different operation. | The selection is unchanged. |
| (iii) After acceptance, rebind `a` to `seqA@2`. | The accepted evidence is unchanged, because it uses the frozen snapshot. A new submission shows the changed identity and needs fresh checking. |
| (iv) Remove `seqB`. | The previously ambiguous unqualified form is still refused unless the user re-submits. Resolution never silently changes the selected law. |

#### Specialization, projection and invalidation

These are defined before any inheritance.

- **Specialization** instantiates `params` through a checked `apply`. It produces a new derived statement bound to the new subject.
- **Projection** selects one law or operation and yields its exact identity.
- **Invalidation:** a bundle identity changes when any member identity changes, and dependents revalidate (Decision 6).
- **#12 interaction:** if #12 later omits reconstructible bundle arguments, it must expose the selected identities and refuse ambiguity. This matches IMPL-CASE-07 in `implicit-proof-arguments`.

#### Packaging, deriving and correspondence

| Activity | What it produces | Needs |
|---|---|---|
| Packaging existing evidence | A bundle record that indexes accepted identities. It adds no theorem. | Each referenced proof is independently accepted under the pinned profile, and every member identity is exact. |
| Deriving a new theorem | For example, a specialization, or `assoc` used to move a claim to another grouping | A fresh checked proof term against the new exact claim, bound to the new subject (VC-REWRITE-01) |
| Source and compiled-artifact correspondence | Evidence that the executed bytes realize the checked subject | #10 and PO-17/18. Bundles never supply it. |

**Rationale:** Explicit, identity-resolving bundles give the reuse benefit of Arend records without global instance search, trusted law declarations, hierarchy or runtime proof callbacks. Behavioral equality never selects a law or transfers evidence (P-ID-06, V-OBS-01).

## Coordination

- **#1 (live slots):** replacement and stale-dispatch evidence stay under VC-LSLOT-01. Saved old definitions keep old envelopes.
- **#3 (ownership):** resource premises and lifetimes. This profile refuses resources.
- **#6 `proof-automation-diagnostics` (PAUTO-):** producers may emit `pc-call` slices and bridges. Templates come only from reviewed rule identities. Goal display shows envelope identities.
- **#10 `artifact-correspondence-extension`:** C8, and the artifact side of bundle reuse.
- **#11 `resource-safe-wiring-laws`:** the first bundle consumer. It reuses this schema for `wire@1` laws.
- **#12 `implicit-proof-arguments` (IMPL-):** implicit bundle and lemma arguments must elaborate to the explicit identities above and refuse ambiguity.
- **#14 (numeric graph):** unaffected.
- **#15 (math foundation):** the BitVec lemmas (`bv-add-assoc` etc.) and T1-T6 are natural first targets.

## Risks / Trade-offs

- Mandatory dependency replay costs time proportional to the closure. The budgets bound it, but deep libraries may need the later identity cache, which carries obligation C4.
- The first profile proves inequality bridges only by exact match or projection. Many realistic callers will get `bridge-unproved` until reviewed inequality rules exist. This is deliberate: the alternative would be producer search or a trusted solver.
- The `Eq`-over-terms extension enlarges the trusted checker surface and needs its own hostile cases (O7-3 shows it is absent today).
- No theorem in this design has been checked. T1-T6, C1-C8 and the bundle whitelist additions remain open.

## Acceptance mapping

| Issue #7 checkbox | Section |
|---|---|
| Design document with exact subject/dependency envelope, call rule and bounded checking algorithm | Decision 1 (profile and bounds), Decision 2 (envelope), Decision 3 (call rule), Decision 4 (lemma vs call), Decision 5 (algorithm and replay), Decision 6 (invalidation and diagnostics) |
| Worked three-stage example using at least two separately exported callees and one reusable lemma | Decision 7 (`steps@1.inc` ×2, `offsets@2.add10`, `arith@1.add-assoc-wrap`); baseline observations O7-1 to O7-4 |
| Positive and negative scenarios: differing intermediate predicates, reordered stack slots, repeated call occurrences, alias rebinding, changed callee versions, forged/stale proofs, missing dependencies, dependency cycles | Decision 8: PREUSE-S02/S03, S04, S05, S06, S07, S08/S09, S10, S11, plus S01 and S12-S22 |
| Explicit correspondence obligations for source-to-logical calls, checked rule application and erasure, separate from finite tests | Decision 9 (C1-C8, mapped to PO-15/16/17/18/19/20) |
| Migration plan preserving MC1/MC2 and named-v2, with unsupported cases rejected rather than silently inlined or weakened | Decision 10; Decision 1 (refusal list); PREUSE-S15 |
| Comment: minimal bundle schema and one bounded pure composition/identity-law example mapped to existing checked rules | Decision 11 (minimal schema; `seq@1.code-monoid`) |
| Comment: worked reuse with explicit operation, theorem, profile, observation and assumption bindings | Decision 11 (worked reuse) |
| Comment: negative cases for mismatched operations, stale revisions, missing dependencies, incompatible observations and resource eligibility | Decision 11 (negative cases) |
| Comment: ambiguous two-bundle example plus tests that unrelated imports cannot silently change the selected law | Decision 11 (ambiguous example, tests i-iv); PREUSE-S22 |
| Comment: distinction between packaging evidence, deriving a theorem and establishing correspondence | Decision 11 (packaging, deriving and correspondence table) |

### Decision: the exact vocabulary T2–T5 need before their theorems can be stated

Measured blocker (2026-10-27): T2–T5 cannot be written today because the vocabulary below does not
exist in Lean, and adding a second caller/callee boundary definition beside `exportedNamedClaim`
would create a second convention. These are the **exact** shapes to review before any of T2–T5 is
attempted; none of them has been kernel-checked, and none is claimed here.

```lean
-- T3 (embed): callee-subject environment rows embed into a caller environment.
-- The renaming is injective and preserves the whole source record, not just the slot number.
structure Embedding where
  rename : Nat → Nat
  injective : ∀ a b, rename a = rename b → a = b
  preserves_origin : ∀ entry, (lookup callee …).origin = (lookup caller …).origin
  preserves_body : ∀ entry, body is transported unchanged
  preserves_interface : ∀ entry, input/output are transported unchanged

-- T2 (callFrame): an entry embedding plus a callee claim give the caller's frame discipline.
structure EntryEmbedding where
  embedding : Embedding
  entry : NamedV1.Entry
  calleeClaim : NamedV2.exportedNamedClaim callee [] P Q

-- T5 (chainClaim): step certificates, bridges and a join.
structure StepCertificate where
  before : Stack
  after : Stack
  claim : Prop              -- a *pure* logical judgment, never a program invocation
structure Bridge where
  fromState : Stack
  toState : Stack
  obligation : Prop
structure Chain where
  steps : List StepCertificate
  bridges : List Bridge
  join : Stack → Stack → Prop
```

Review questions that must be answered before implementation, in this order:

1. Is `Embedding` over *rows* (as `Embedding` above) or over *programs*, and does origin preservation
   quantify over every row or only the reachable ones? T3 says every row.
2. Does `EntryEmbedding.calleeClaim` take the callee's own types or the caller's frame types? The
   design's T2 statement uses the caller's `frame ++ args` discipline, so the claim's stacks are
   caller stacks.
3. `StepCertificate.claim` is a **pure** proposition; a program call slice must stay a separate sort
   (Decision 4). Confirm that a certificate cannot smuggle a `(pc-call …)` slice in.
4. `Chain.join` is a premise the *caller* supplies, matching `pc_sequence`; confirm no step may
   supply its own join.

Nothing is ticked by this section: tasks 15–19 stay open, and T1/T6 remain the only landed theorems.
