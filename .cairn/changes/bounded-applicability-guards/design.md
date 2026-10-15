## Context

This is a design-only change for issue #8. Nothing below is implemented, executed or proved by this change.

**Baseline re-verified at `44329dc`.** The issue cites `0ea10d3`. `git diff 0ea10d3 44329dc` is empty for every path cited here (`.cairn/specs/program-contracts/spec.md`, `crates/noble-contracts/src/companion/`, `crates/noble-cli/src/core/companions/`, `crates/noble-cli/src/build.rs`, `proofs/mc2/`, `specs/verification/obligations.json`), so the issue's links have not drifted. One unrelated stale link exists: program-contracts §8.3 points to `README.md#mc2-companion-sessions-and-proof-required-builds`, but that anchor exists in neither the `0ea10d3` nor the `44329dc` README.

What exists today:

- **Specification.** Under VC-LOGIC-02 (`program-contracts/spec.md:119`), an evidence validator proves the implication only. Applicability for an actual input needs a separate proof, a checked decidable guard, or an environmental premise that policy accepts explicitly. VC-USE-01 (`:1024`) selects established caller proofs and "explicit, total, pure scalar/structural applicability guards", each with a proved link, specified cost and fail-closed behavior. VC-ADMIT-05 (`:644`) makes guard code executable behavior, not erasable metadata. §8.3 (`:759`) requires an installed guard for the exact contract and restoration of tail/identity on refusal. It also states that erasure does not make conditional correctness unconditional. Obligation **PO-21** ("Applicability checks establish the exact precondition before certified invocation", route `lean-and-aeneas-guard-correspondence`) is `open` in `specs/verification/obligations.json`.
- **Recognizer.** `templates_of` (`crates/noble-contracts/src/companion/guard.rs:48`) returns nothing unless the checked input interface is exactly `[I64]`. `recognize` (`:70`) inspects only the root of `requires`. It accepts exactly three shapes: `Lt(in0, i64::MAX)`, `Eq(in0, k)` in either operand order, and `Not(Eq(in0, i64::MIN))`. Every other shape yields no template, and the registry then refuses `UnsupportedGuardTemplate` (`guard.rs:139`). `accept.rs:68` stores the recognized templates in `ContractEntry.guards` (`registry/mod.rs:68`). Derived (compose/instantiate) contracts get `guards: Vec::new()` (`rules/derive.rs:161`), so composed certified programs currently have no guard (control: `crates/noble-cli/src/core/companions/controls/composition.rs:71`).
- **Wrappers.** `wrapper_source` (`guard.rs:171`) renders `[ dup K = [ drop 1 inl ] [ [ SUBJECT ] run inr ] if ]`, with the branches swapped for `EqI64Literal`. `invocation_wrapper` (`guard.rs:205`) renders `[ swap dup K = [ drop drop 1 inl ] [ swap run inr ] if ]` over `[argument, Program]`. On rejection the wrapper consumes the argument (and the live Program) and returns `inl (I64 1)`. On acceptance it runs the subject on the untouched stack and tags the top result `inr`.
- **Compiled invocation path.** `noble companions … invoke INDEX ARG` → `op_invoke` (`crates/noble-cli/src/core/companions/operations/invocation.rs:17`). The steps are:
  1. `certified_at` projects the live companion. Then `bound_companion` (`session/binding.rs:132`) runs `Core::correspond`, re-observes the live subject, runs `Core::bind`, and compares the issued identity.
  2. `evidence_guard_templates` returns the first template; it is stale-checked against policy/revision.
  3. `invocation_wrapper` is called, and `"{wrapper} run"` is compiled and executed in the session engine with typed injections `[I64, Program]` (`invocation.rs:40-50`).
  4. The frame is restored, and identity is re-checked (`:53`).

  The argument comes from the CLI only as an `I64` literal (`reporting/inputs.rs:1`); only I64/Program/companion injections exist. `noble build --require-proof` writes the same invocation wrapper as a separate `wrapper.noble` product next to `artifact.wasm` (`build.rs:170,202`; `build/artifacts.rs:30`). It is not compiled into the released artifact.
- **Lean.** `proofs/mc2/NobleCertificates/Guards.lean` proves `guard_sound` (`:299`), `wrapperProgram_exec_iff` (`:188`), `sourceWrapperProgram_refines` (`:234`), `liveWrapperProgram_refines` (`:257`), `guard_correspondence` (`:311`) and the boundary lemma `ltI64Max_boundary` (`:321`) against a handwritten `GuardPrecondition` (`:43`). `Rules.lean:249` (`wrapper_pc`) transports a candidate PC to the wrapper under `guardedPre`, which **still assumes** the candidate's own precondition. `Correspondence.lean:174,196` equates the extracted Rust renderers with the layouts. `MC2Gate.lean:88-93` freezes the template set at exactly three constructors. The source-bound guarded fixture (`SourceReplay.lean:253-280`) is closed by `native_decide` (expanded trust).

**Gap this design targets.** No theorem connects the Rust recognizer's match to the MC1 meaning of the contract's own `requires`, namely `Holds requires [x] [] params ↔ GuardPrecondition t x`. The link is a structural Rust match plus a handwritten Lean definition. Only inputs of type `I64` are guardable. Composition drops guards. No rule says which applicability facts survive stack manipulation. The selected extension below must close the first gap for its own template and specify the others, without implementing anything.

## Decisions

### Decision: Applicability witnesses, not refined input views

**Choice:** An applicability witness is a logical, nonexecuting judgment `Applies(β, v̄)`. It is bound to a contract/subject/revision tuple `β` (see "Exact binding") and to the actual input values `v̄` that the protected body consumes. It is never a guest value, stack type, `Program` parameter or runtime token. A witness is introduced in exactly one of three ways:

1. a caller proof (static);
2. one execution of a selected guard wrapper (dynamic);
3. an environmental premise accepted explicitly by policy, unchanged and not extended here.

Refined input views (`{xs : List a | nonempty}` as a stack type) are rejected for this profile.

**Rationale:** A refined view would change ordinary stack typing and `Program<S,T,e>` interfaces. VC-SCOPE-02 and V-CORE-02 forbid dependent `Program` parameters, general refinement inference and implicit behavioral coercions. A refined view would also have to be propagated (or invalidated) through every stack word, quotation, capture, aggregate and reflection observation, and erased from recipes. A witness instead lives where VC-LOGIC-02 already places applicability: beside the checked claim, in proofs and admission. Ordinary typing, recipes and `DefinitionId`s stay unchanged. Proof-free `run` stays available (VC-SCOPE-01). Dynamic witnesses are consumed by the single wrapped invocation; no refinement value can be stored, duplicated, or replayed onto another input.

### Decision: One bounded first extension: `NonEmptyList`

**Choice:** Add exactly one template, `NonEmptyList`, as a new **versioned template set v2**, while v1 (`LtI64Max`, `NeI64Min`, `EqI64Literal k`) stays frozen, byte for byte, together with its existing gate. `NonEmptyList` is selected only when **all** of the following hold:

- the subject's checked input interface is exactly one value of type `List A`, where `A` is a pure Type0 code (`Unit`, `Bool`, `I64`, `Text`, or `Pair`/`Sum`/`List` of such codes; no `Program`, `Syntax`, resource or effectful type);
- the **root** of the contract's `requires` is exactly `(not (is-nil (in x0)))`, where `x0` is that single input.

The following variants are explicitly **unsupported**, not normalized:

- `(lt 0 (length (in x0)))` and `(not (eq (length (in x0)) 0))`: MC1 `length` is an `I64` modulo 2^64 (§8.1; `proofs/mc1/NobleContracts/Expression.lean` `length`), so they are not equivalent to nonemptiness for every mathematical list length;
- double negations;
- `(is-nil …)` (emptiness);
- any `and`/`or`/`implies` root;
- any predicate that mentions `head`, `tail`, `left`, `right` or `maps`.

Conjunction is **not** supported in this extension (see "Checking rules").

**Rationale:** The pure Core vocabulary has `=` on `I64` but no ordered comparison word (`crates/noble-contracts/src/program/words.rs:88-111`). The existing `LtI64Max` is executable only because `x < MAX ⇔ x ≠ MAX` over signed `I64`. A general scalar range guard would therefore need a new comparison primitive, which is a language change outside #8. Nonemptiness is the smallest structural fact that is:

- decidable by an existing total eliminator (`list.case`, K-DATA §7.4, `language/spec.md:1118`);
- total over every well-typed input;
- useful, because it makes the partial MC1 selectors `head`/`tail` defined.

It exercises the "undefined is not true" rule directly. Sum-tag (`is-left`) guards are the obvious second candidate; they are deferred so that only one template set revision is opened at a time.

### Decision: The guard consumes rejected inputs (unchanged rejection schema)

**Choice:** `NonEmptyList` keeps the existing v1 result schema. On rejection the wrapper consumes the argument (and, in the live wrapper, the Program value) and returns `inl (I64 1)`, with zero body requests. On acceptance it runs the unchanged subject on the untouched stack and returns `inr` of its single top result. The rejected list is **not** returned.

**Rationale:** A single `Sum I64 Y` schema keeps the existing `render` left/right check (`invocation.rs:71` onwards) and Lean `rejectionProgram` reusable. The rejected payload is pure data that the caller still holds a value-equal copy of if it chose to `dup` before invoking. Returning resources from a rejected guard belongs to the ownership-aware design in #3. Resource-bearing inputs are out of scope here, because `dup` requires `Data(a)` (`language/spec.md:1011`, K-RES-02).

## Selected grammar

The guardable-precondition grammar matches the **whole** checked `requires` root on the exact input interface. It is not a fragment search.

```text
GuardV1  ::= (lt (in x0) 9223372036854775807)                    -- LtI64Max      ; in: [I64]
           | (not (eq (in x0) -9223372036854775808))             -- NeI64Min      ; either eq order
           | (eq (in x0) k) | (eq k (in x0))                     -- EqI64Literal k; k an I64 literal
GuardV2  ::= GuardV1
           | (not (is-nil (in x0)))                              -- NonEmptyList  ; in: [List A], A ∈ Type0
Witness  ::= static(π)            -- checked caller proof, VC-COMPOSE-01
           | guard(t, β)          -- one execution of wrapper(t) under binding β
           | premise(e, policy)   -- existing VC-LOGIC-02 route, not extended
```

`x0` must be input index 0, and the input interface must have length 1. Spellings are the MC1 surface forms (§8.1). Recognition runs on the typed MC1 IR (`ExprKind`), not on text.

## Checking rules

Write `C = (c, s, ρ)` for an accepted contract `c` (exact statement), subject `s` (identity and op events), and context `ρ` (semantic revision, policy revision, host-contract and environment-fact digests, as retained by `Core`). `Pre_c` is the independently elaborated MC1 precondition. `T(c)` is the recognized template set.

```text
(G-Form)   templates_v2(prepared_c) = [t]    root(requires_c) ≡ canon(t)    in(c) = sig(t)
           ---------------------------------------------------------------------------------
                                    Guardable(C, t)

(G-Intro)  Guardable(C, t)    current(ρ)    bind(C) = s    exec(wrapper(t, s), tail ++ v̄) accepts
           ---------------------------------------------------------------------------------
                               Applies(β(C,t), v̄)    for that single invocation only

(S-Intro)  Γ ⊢ π : PC(p, P₀, Q₁)    Γ ⊢ ι : ∀σ σ'. Q₁ σ σ' ⇒ Pre_c(top_in(c)(σ'))
           ---------------------------------------------------------------------------------
                        Applies_static(C, compose(p, s))       (VC-COMPOSE-01 sequencing)

(A-Elim)   Applies(β, v̄)    PC(s, Pre_c, Post_c)    s runs on exactly tail ++ v̄
           ---------------------------------------------------------------------------------
                               Post_c holds for every normal return

(Erase)    erase(guarded C) = wrapper(t, s)        -- guard code retained (VC-ADMIT-05)
           erase(static C)  = compose(p, s)        -- proof metadata only; no guard inserted
```

- **Formation** requires the canonical whole-root match, the exact input signature and a current context. `canon(NonEmptyList) = Not(IsNil(Input 0))`. Without a template, invocation refuses `unsupported-guard-template`. There is no fallback to an "assumed true" guard.
- **Introduction** happens once per invocation. Its conclusion names the binding and the actual consumed values; it cannot be stored in a companion or aggregate.
- **Elimination** is the only consumer. The witness establishes `Pre_c(v̄)` for the values that the body receives, and the existing `wrapper_pc` (`Rules.lean:249`) then transports the PC claim.
- **Conjunction is not supported.** A future `GuardAnd` would have to recognize the whole root `(and P₁ P₂)` with *every* conjunct a selected template over the *same* single input. It would evaluate every flag before the body starts, reject if any flag rejects, and come with a theorem `Holds (and P₁ P₂) ↔ GP t₁ ∧ GP t₂`. Executing a guard for one conjunct never introduces `Applies` for a contract whose root is a conjunction. Such a contract is `unsupported`.
- **Composition through ordered stack interfaces.** Composition never transfers a guard. A guarded `C₁` composed with `C₂` gives the composite a precondition `Pre_{C₁}` (if the intermediate implication is proved) over `C₁`'s input. The composite is guardable only if *its own* elaborated precondition independently passes `G-Form`. Today derived contracts carry no guards (`rules/derive.rs:161`); this design keeps that refusal and adds no derived-guard rule.
- **Erasure.** Erasing proof metadata from a guarded certified program yields the wrapper program, with the guard intact (`CertifiedInvariant.lean:114-121`). Erasing a static witness leaves the unguarded composed program, which is correct only because `π`/`ι` covered every input. If proof metadata is stripped, a release that required the claim must fall back to a labeled ordinary run. It must never be reported as certified (VC-ADMIT-02, VC-VALUE-05).

## Exact binding

A guard binding is the tuple:

```text
β = ( template-set version (1 | 2),
      template t (including literal k when present),
      contract id + exact_statement (the full independently generated statement,
        not only the 64-bit statement digest, registry/mod.rs:62-64),
      canonical root term canon(t) re-derived from requires_c,
      subject identity + retained op events (`Subject.events`, `companion/mod.rs:234-244`),
      exact input type list and output type list,
      semantic revision, policy revision, host-contract and environment-fact digests )
```

- The binding is recomputed at invocation from the current registry entry and live subject. It is never read from candidate metadata.
- Any change to `requires` creates a new contract (new statement), so an old guard does not apply.
- Any change to the subject or its captures changes identity, and `bind`/`correspond` refuse `MismatchedSubject`.
- A policy or semantic revision change makes the entry stale, and `templates` refuses `StaleContext`.
- A guard for contract `c` with a similar predicate (for example `EqI64Literal 5` versus a contract with `k = 6`, or `NonEmptyList` versus a contract whose root is `(and (not (is-nil (in x0))) …)`) is a different or absent template, not a substitute.
- 64-bit fold digests are indices, not collision-resistant equality evidence. The binding therefore compares the retained exact statement and op events, matching the existing `ContractEntry` comment.

## Correspondence statement

For every template `t` in the selected set, every typed input `v` with `HasType v (sig t)`, every tail and every subject code `s`:

```text
(C1 recognizer)   templates_v2(prepared) = [t]  ⇒  ∀ v params, Holds(requires_prepared, [v], [], params) ↔ GP(t, v)
(C2 guard exec)   Exec(guardProgram t, tail ++ [v], r)  ↔  r = tail ++ [v, Bool(guardRejects t v)]
(C3 soundness)    guardRejects t v = false  ⇒  GP(t, v)                (and conversely: exactness)
(C4 wrapper)      Exec(wrapperProgram t s, tail ++ [v], r) ↔
                    if guardRejects t v then r = tail ++ [inl (I64 1)]
                    else ∃ m. Exec(s, tail ++ [v], m) ∧ Exec([inr], m, r)
(C5 renderers)    parse(wrapper_source(t, ⌜s⌝)) = sourceWrapperProgram t s,
                  parse(invocation_wrapper t)   = liveWrapperProgram t, and both refine (C4)
(C6 lowering)     the managed-Wasm compilation of "{invocation_wrapper t} run" executed with
                  injections [v, Program s] observes (C4) — PO-17/PO-18 scope, OPEN
```

- `GP(NonEmptyList, v) := ∃ a as. v = list (a :: as)`.
- `guardProgram NonEmptyList = [dup, block [true], block [drop, drop, false], list.case]`.
- The **actual compiled invocation path** for `NonEmptyList` is the existing `op_invoke` route above, with two specified changes:
  1. a typed `List A` argument injection taken from an existing session stack slot (not a CLI literal);
  2. the v2 template dispatch.
- The rendered texts are fixed:

  ```text
  source:  [ dup [ true ] [ drop drop false ] list.case [ drop 1 inl ] [ [ SUBJECT ] run inr ] if ]
  live:    [ swap dup [ true ] [ drop drop false ] list.case [ drop drop 1 inl ] [ swap run inr ] if ]
  ```

  In both, the flag is `true` exactly for the empty list. `list.case` runs its first program for `nil` and its second on `head tail` for `cons` (`language/spec.md:1118-1124`). `if` runs its first program for `true`.

Meaning of a successful guard: after `G-Intro`, `Pre_c` holds for **the values that the protected body then consumes**. Those are the same stack cells, because the guard reads a `dup` copy and the accepted branch runs on the original `tail ++ [v]`. Outcomes other than success:

- **Refusal.** The flag is `true`. The result is `inl (I64 1)` with zero candidate-body requests (C4). The session reports `precondition-reject` (as CONTRACT-07 does today).
- **Undefined predicates.** No selected template is undefined on a well-typed input: `is-nil` is total on lists, and `eq`/`lt` are total on `I64`. Any precondition whose evaluation can be undefined (`head`, `tail`, `left`, `right`, `maps`, wrong-typed projections) is outside `GuardV2`. It stays `unsupported` and is never treated as true or as false-meaning-reject. `Holds` counts only `some (bool true)` (`Expression.lean`).
- **Traps.** None of the guard's operations traps on well-typed input. A trap raised by the Wasm runtime during the guard (for example an internal fault) is reported as a trap, not as rejection or acceptance. The body has not started; `body_started=false` is required.
- **Resource bounds.** The guard costs `dup` + `list.case` + `if`. The cost of `dup` on a list in managed Wasm is not audited in this design [INFERENCE: it may be a copy proportional to list size]. The guard runs under the invocation's existing quota meter. Exhaustion during the guard is classified `exhausted` (VC-LOGIC-04) with the body not started, never `precondition-reject`. Cost and limits must be stated per template, as VC-USE-01 requires.
- **Effects.** The wrapper is pure (`! {}`). Rejection occurs before the subject's first instruction, so no guest request, protected operation or host effect can precede it.

## Fact survival

Applicability facts are properties of **values at stack positions inside a proof**, never runtime tokens. A dynamic witness is consumed by its single invocation and does not survive into the caller's continuation.

| Operation on a value `v` carrying a static fact `F(v)` | Survives? | Rule |
|---|---|---|
| `swap`, `dip` (held/moved) | Yes, relocated | Value unchanged; position renamed in the configuration predicate |
| `dup` (Data only) | Yes, on both copies | Immutable pure values; both copies are equal to `v` |
| `drop` | Fact discarded | No value remains |
| `quote`/capture into a Program | Only as a fact about the capture slot | Bound by capture identity; a changed capture changes subject identity (`proofs/mc2/NobleCertificates/Model.lean:114`) |
| `compose(p, s)` | Only through a proved intermediate implication | VC-COMPOSE-01; type compatibility alone proves nothing |
| Capture-family instantiation/substitution | Only for the instantiated slot value | A fact about family parameter `n` does not transfer to another instantiation |
| `cons`, `list.case` destructuring, `pair`/`unpair`, `inl`/`inr`/`case` | No; re-establish | A new value; `NonEmptyList` survives `cons` trivially but must be re-proved, not inherited |
| `+`, `-`, `*` on I64 | No | Wrapping arithmetic: `x < MAX` does not survive `1 +` (`MAX-1 ↦ MAX`); `x ≠ MIN` does not survive `1 -` |
| Changed `requires`, contract, subject, semantic or policy revision | Invalidated | The binding β no longer matches; revalidate (VC-ADMIT-04, VC-VALUE-03) |

**Signed `I64` wrapping is not ℤ.** `GP(LtI64Max, x)` is `x.toInt < 2^63-1` over BitVec 64 (`Guards.lean:43`). It does not imply `x + 1 > x` in ℤ arithmetic on the original value unless the wrapping semantics are applied. `ltI64Max_boundary` (`Guards.lean:321`) pins `MAX + 1 = MIN`. Range-style preconditions over mathematical integers (for example "x + 1 does not overflow in ℤ") are not guardable unless restated as the exact `I64` template.

## Examples

Both examples use the same MC1 contract. The block-interface spelling for `list.case` branches is [INFERENCE]: no existing MC1 fixture exercises `list.case`, so the implementation must confirm it by `noble verify`.

```text
(contract 1 head-of-nonempty
  (input (xs (List I64)))
  (output (y I64))
  (program [ (block () (I64) [ 0 ]) (block (I64 (List I64)) (I64) [ drop ]) list.case ])
  (requires (not (is-nil (in xs))))
  (ensures (eq (out y) (head (in xs)))))
```

Without the precondition the claim is false: for `nil` the body returns `0`, `head nil` is undefined, and `Holds` is not true. With it, the claim is a valid conditional claim.

**Static applicability.** A caller builds the list itself: `[ 5 nil cons ] ∘ subject`.

1. The caller proves `PC([5 nil cons], true, λ_ σ'. σ' = tail ++ [list [5]])`.
2. It proves the intermediate implication `σ' = tail ++ [list [5]] ⇒ Holds (not (is-nil (in xs))) [list [5]]`, which is immediate by `evaluate`.
3. It composes by VC-COMPOSE-01 (`pc_sequence`), obtaining `PC([5 nil cons …list.case], true, y = 5)`.

No guard executes. Erasure leaves exactly `5 nil cons [ 0 ] [ drop ] list.case`. This is checkable today as an MC1/Lean proof over the composed program. A *runtime companion* rule that consumes such a static witness needs the generalized composition of #7 (MC2 `ComposeV1` only composes `IncrementBy` templates). That rule is listed as a coordination dependency, not claimed.

**Runtime guard.** A caller receives `xs` from elsewhere (for example a session slot) and invokes the certified subject with `NonEmptyList`:

- `xs = [7, 8]` → `inr (I64 7)`, subject identity unchanged;
- `xs = []` → `inl (I64 1)`, `candidate_body_started=false`, zero guest requests and protected operations.

The equivalent source wrapper is `[ dup [ true ] [ drop drop false ] list.case [ drop 1 inl ] [ [ [ 0 ] [ drop ] list.case ] run inr ] if ]`.

## Boundary and hostile test plan (unexecuted designs)

Every row has state `absent / not-run / open / unassessed` with empty evidence. The IDs are design-local (`GUARD-CASE-*`); an integrator would register them in `specs/conformance/contract-cases.json`. "Refuse" means a typed refusal at the stated stage with `candidate_body_started=false`, zero guest requests and zero protected operations.

| Case | Input | Expected observation |
|---|---|---|
| GUARD-CASE-01 MIN/MAX | v1 `LtI64Max` at `MAX`, `MAX-1`, `MIN`; `NeI64Min` at `MIN`, `MIN+1`, `MAX` | reject at `MAX` / accept `MAX-1` (→ `inr MAX`) / accept `MIN`; reject `MIN` / accept `MIN+1` / accept `MAX`; v1 behavior and wrapper bytes unchanged under the v2 set |
| GUARD-CASE-02 wrong literal | contract `(eq (in x) 5)`; arguments 5, 6, −5; a `EqI64Literal 6` binding substituted for the contract's own | accept 5, reject 6 and −5; substituted binding refuses `MismatchedContext`/`StaleContext` before execution, never "accepts 6" |
| GUARD-CASE-03 empty/nonempty | `head-of-nonempty` with `[]`, `[MIN]`, `[7, 8]`, `[[]]`-shaped `List (List I64)` element | `[]` → `inl 1`, body not started; others → `inr head`; tail below the argument preserved; `List (List I64)` accepted (outer list nonempty) |
| GUARD-CASE-04 undefined selectors | contracts with `requires` `(lt 0 (head (in xs)))`, `(is-left (in s))`, `(eq (left (in s)) 1)` | `unsupported-guard-template` at preparation; invocation never treated as true or as `precondition-reject` |
| GUARD-CASE-05 partial precondition | `requires (and (not (is-nil (in xs))) (lt 0 (head (in xs))))`; also `(and (lt (in x) MAX) (not (eq (in x) MIN)))` | `unsupported` (no conjunction); executing the `NonEmptyList`/`LtI64Max` guard for one conjunct does not yield `Applies` |
| GUARD-CASE-06 near-miss shapes | `(lt 0 (length (in xs)))`, `(not (eq (length (in xs)) 0))`, `(not (not (not (is-nil (in xs)))))`, `(is-nil (in xs))`, two-input `[List I64, I64]` | each `unsupported`; length forms explicitly cite modulo-2^64 `length` |
| GUARD-CASE-07 changed predicate | admit `head-of-nonempty`; re-admit with ensures unchanged but `requires true`; also with `requires (not (is-nil (in xs)))` reproduced under a new policy revision | first: new contract, no guard, invocation refuses; second: old entry `StaleContext` until re-admission |
| GUARD-CASE-08 changed subject | bound companion whose live subject is replaced by `[ [ 1 ] [ drop ] list.case ]` (same interface); changed capture in a family | `MismatchedSubject` before guard execution; identity unchanged on refusal |
| GUARD-CASE-09 erased proof metadata | strip evidence from a certified guarded program; strip static proof from the composed caller | guarded: wrapper (with guard) still executes identically, no certified claim reported after projection; static: ordinary labeled run, release blocked when proof required |
| GUARD-CASE-10 composition | compose two guarded `LtI64Max` evidences; compose `[5 nil cons]` evidence with `head-of-nonempty` evidence | first: composite has no guard → `unsupported-guard-template` (existing control preserved); second: `unsupported` until #7 rule exists; never inherits a guard |
| GUARD-CASE-11 quota and trap | guard executed with a fuel/memory quota that expires inside `dup`/`list.case` on a long list; injected engine trap inside guard | `exhausted` / `trap`, body not started, distinguishable from `precondition-reject`; frame restored |
| GUARD-CASE-12 wrong-typed injection | `NonEmptyList` binding invoked with an `I64` argument, or with a `List Program` slot | refusal at injection typing (`stack-type`), no execution |

## Evidence ledger

| Item | Class | Status at `44329dc` | This change |
|---|---|---|---|
| v1 `guard_sound`, `wrapperProgram_exec_iff`, `sourceWrapperProgram_refines`, `liveWrapperProgram_refines`, `guard_correspondence`, `ltI64Max_boundary` | Strict Lean theorems in MC2 gate | Present in `Guards.lean`; PO-21 remains `open` | Reused unchanged |
| v1 renderer equations `wrapper_source_layout`, `invocation_wrapper_layout` | Extracted-Rust (Charon/Aeneas) equations | Present in `Correspondence.lean` | Reused unchanged |
| C1 recognizer link for v1 and `NonEmptyList` | **Theorem obligation** (Lean model + Aeneas over `templates_of`) | Absent | New, open (`GUARD-CORR-01`) |
| C2–C4 for `NonEmptyList` (guard exec, soundness + exactness, wrapper) | Theorem obligation (strict Lean) | Absent | New, open |
| C5 renderer layouts for v2 | Extracted-Rust equations | Absent | New, open |
| C6 Wasm lowering/loading of wrapper | PO-17/PO-18 | Open, not addressed by this design | Remains open; coordinated in #10 |
| `MC2Gate` frozen-set check | Gate script | Exactly 3 constructors | Future v2 gate checks exactly 4 constructors under a distinct set version; v1 gate kept |
| GUARD-CASE-01…12 | Compiled tests on frozen executables | Not designed before; absent | Designed here; not run |
| Host-check observation authenticity, independent Lean checker soundness, session engine/Node/V8, `native_decide` source equations | **Trusted components** | Explicit assumptions (§8.3, VC-GATE-03) | Unchanged; not discharged |
| Recognizer/IR parser correctness beyond C1, MC1 frontend/elaboration (PO-19) | Trusted / open | Open | Unchanged |

Passing tests, matching hashes or successful extraction do not discharge any theorem row.

## Coordination

- **#3**: resource-bearing and borrowed inputs, returning rejected owners, and lifetime refinements are excluded. `dup` requires `Data(a)`.
- **#7**: companion-level static witnesses and any derived-guard rule depend on generalized composition. Named-call subjects need their own G-Form binding.
- **#10**: C6 and the binding of `wrapper.noble` to the admitted artifact are listed in its correspondence matrix.
- **#6**: diagnostics for `unsupported-guard-template` should name the unmatched root shape. No automation may synthesize guards (VC-USE-01).

## Risks / Trade-offs

- Restricting the selection to whole-root matches makes many useful preconditions unsupported. This is deliberate: a broader normalizer would itself need a correspondence theorem.
- `NonEmptyList` depends on unaudited `dup` cost for aggregates. A quota failure inside the guard is reported as exhaustion, not rejection.
- Keeping the `inl (I64 1)` rejection payload discards the rejected list. Callers that need it must `dup` first.
- The v2 template set means MC2 extraction, gate and registry encodings all need a version bump. Silently widening v1 is forbidden.

## Acceptance mapping

| Issue #8 acceptance checkbox | Section |
|---|---|
| Small selected grammar and typing/checking rules, with a rationale for choosing witnesses versus refined input views | "Decision: Applicability witnesses, not refined input views"; "Selected grammar"; "Checking rules" |
| Correspondence statement relating each selected logical predicate to its executable guard, including the actual compiled invocation path | "Correspondence statement" (C1–C6, wrapper texts, `op_invoke` path); "Context" (compiled invocation path) |
| Example where a caller proves applicability statically and an equivalent example using a runtime guard | "Examples" |
| Boundary/hostile test plan covering MIN/MAX, wrong literal, empty/nonempty structural inputs where selected, undefined selectors, partial-precondition checks, changed predicate/subject and erased proof metadata | "Boundary and hostile test plan" (GUARD-CASE-01…12) |
| Clear evidence ledger separating theorem obligations, compiled tests and remaining trusted components | "Evidence ledger" |
