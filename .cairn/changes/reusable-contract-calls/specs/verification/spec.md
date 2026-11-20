# Reusable-call correspondence obligations

## MODIFIED Requirements

### Requirement: V-INTRINSIC-01
r[V-INTRINSIC-01]

**V-INTRINSIC-01.**

The reviewed Lean 4 reference semantics MUST define the exact `PureTyCode`/`El` interpretation, predicative logical binder/equality rules, typed source proof calculus, captured program subject and partial-correctness propositions, finite rule expansion and proof erasure for `Intrinsic-Proofs-Draft`. The first independent checker target for Noble-source proof declarations MUST be the pinned strict Lean consumer, checking a restricted translated closed term against the independently regenerated exact MC1 obligation with inspected transitive assumptions; it MUST NOT accept producer Lean declarations, unchecked axioms, native proof evaluation, tactics or a Bend2 proof route. `exec_lit_iff`, `exec_add_iff`, `pc_exact`, `pc_sequence`, `exportedClaim_unaryI64`, `exportedClaim_consequence`, `holds_true`, `holds_eq` and `List.append_assoc` MUST be named only when their exact reviewed theorem declarations, premises and model/standard-library revisions are available. Both `pc_sequence` implications MUST be proved from checked exact-result hypotheses; symbolic append-associativity is not definitionally equal by fiat. The conversion of an inner `PC` into exact exported MC1 typed/pre/post `Holds` predicates MUST independently prove typing, output existential and expression evaluation; it MUST NOT assume `PC` and `exportedClaim` are interchangeable. A proof about wrapping `I64` normal returns MUST NOT be reported as termination, runtime Wasm correspondence, host authority or a theorem for arbitrary integers.

The source proof typing/translation-soundness theorem, proof-kernel consistency/normalization and decidability/coverage, independently checked type-code/domain fidelity, exact MC1 source/contract/subject correspondence, proof erasure preservation, and actual Rust parser/checker/translator refinement and backend/artifact correspondence are separate **open** obligations. Lean kernel acceptance of generated output alone MUST NOT close them. Reports MUST identify exact source declaration/version, accepted subject/recipe, expected claim, assumptions, proof-library/model/consumer revisions, evidence class and unresolved boundaries.

For the selected named-call revision, the independently expected `NamedV2.exportedNamedClaim` MUST include `StackTyped` for `before`, empty typed `params` and `after`, `pre before [] params`, preservation of every bottom-stack tail, named `Exec` of the accepted `twice` definition BODY and `Holds₂` for the entire original MC1 `Term` vocabulary. A strict Lean example with `step [ 1 + ]` specialized at two distinct uses and `twice [ d.step d.step ]` can prove the wrapping `x+2` proposition, with only `propext`, `Classical.choice` and `Quot.sound` as axioms. That is a model/fixture result, NOT a theorem about the authentic submitted one-call root, exact source occurrences, no-orphan graph, owner policy, Rust implementation or backend. Independent host source and graph admission MUST establish those premises before any named source proof can be accepted; `NamedV1.Satisfies` or `NamedV1.ExactClosure` alone cannot establish them. No implementation/evidence promotion follows from this specification or model fixture.

A reusable reviewed two-call rule MUST be parameterized by independently accepted actual graph/row and source premises and instantiated at the regenerated exact claim by the strict consumer. The synthetic example with fixed slots 7/8 MUST NOT be reused as authority over actual source. The rule, source-origin authentication and generated exact claim are joint admission prerequisites, not substitutes for each other; no host result is claimed here.

**Reuse existing policy, not a new language backend:** under VT-ROLE-01 and VT-SCOPE-01, actual production Rust module parsing/resolution, source-term checker, typed Noble→Lean proof lowering and admission decisions follow the pinned Charon → Aeneas → Lean implementation-refinement route with source coverage and explicit external models. Charon/Aeneas consume Rust representations; they do not parse Noble source, accept Noble proof terms or provide the independent exact application-theorem kernel check. No new extraction/refinement is claimed by this spec change. Eurydice (Rust→C), Scylla (C→Rust) and Bend2 are not selected proof dependencies.

The reviewed rule identities of the bounded `Named-Contract-Calls-v3` profile are `List.append_assoc` (historical), `List.nil_append`, `List.append_nil`, `BitVec.add_assoc`, `BitVec.add_comm` and `BitVec.mul_add`. Each MUST be an exact Lean core/Std theorem declaration, and the rule set MUST be stated and pinned *by name* in the strict contract gate (`proofs/mc1/ContractGate.lean`) under the same strict axiom list (`propext`, `Classical.choice`, `Quot.sound`); the identities are never axioms and MUST NOT be admitted by widening any module's simp set. Adding, removing or changing a rule identity is a reviewed rule-set change and requires independent review of this list.

Rule-identity edits MUST NOT modify a `MODEL` module or any revision-hashed source, so `model_revision()`, `named_model_revision()`, `checker_revision()` and `named_checker_revision()` stay unchanged and no accepted dependency is staled by a rule-set change; any proof-term rule table belongs outside those inputs (and, while it is inert, outside the extracted contract library) until the revision-3 symbolic checker consumes and enforces it. The revision-3 symbolic checker is the enforcement point: until it exists the table is an inert record. The NamedV3 model lemmas `callFrame`, `embed`, `liftMc1Claim` and `chainClaim` discharge no source correspondence, host or artifact obligation; the archived design's amendment of 2026-11-20 records exactly what each one states.

## ADDED Requirements

### Requirement: PREUSE-EVID-01
r[PREUSE-EVID-01]

Revision-3 reusable-call and law-bundle support MUST record each of the following as an open obligation, separately from finite scenario results. Each obligation is discharged only by its own proof or refinement evidence:

- **Source-to-logical call translation:** each authenticated occurrence maps to exactly one modeled call with exact slot, provenance and instantiation.
- **Claim regeneration:** regenerated claims match the elaborated source contracts.
- **Checked rule application:** the Rust symbolic and bridge checker establishes exactly the reviewed chain-rule premises.
- **Replay soundness:** an accepted envelope yields the exact theorem.
- **Model lemmas:** environment embedding and lifting from MC1 to the named model.
- **Erasure:** recipe, DefinitionId and Wasm are unchanged.
- **Model adequacy.**
- **Executed-artifact correspondence.**

Passing scenarios, a successful Lean compilation, document validation or a bundle record MUST NOT be reported as discharging any of these obligations.
