# Program Contracts and Verification Evidence

Document: SPEC-V002  
Revision: 0.1.0-draft.1  
Date: 2026-09-09  
Status: Optional verification-layer specification; not a source-syntax or wire-format specification  
Depends on: [SPEC-V001](VERIFICATION.md), [IMPL-V001](VERIFICATION-TOOLCHAIN.md)

## 1. Scope

A program's ordinary type describes its stack interface and latent host-operation bound. A behavioral contract states a stronger property of an exact program under explicit assumptions. These are different claims.

**VC-SCOPE-01.** Behavioral verification SHALL be optional. An ordinary well-typed program MUST remain callable without an attached behavioral proof unless an explicit host admission policy requires additional evidence. Such a policy MUST NOT be confused with the core typing relation.

**VC-SCOPE-02.** Contracts and proofs SHALL initially live in separate Lean modules and tool metadata. This revision adds no Noble assertion syntax, dependent `Program` parameters, automatic SMT invocation, or runtime proof objects. Mathematical notation below is not Noble source.

The selected proof logic is Lean 4 over the reviewed Noble model. Verus verifies Rust implementation contracts; it is not an automatically available prover for arbitrary Noble source.

## 2. Contract meanings

### 2.1 Preconditions and normal-return postconditions

Let a logical configuration contain the current typed stack, modeled host state, ownership obligations, and accumulated event history. Immutable ghost parameters can retain initial inputs and state snapshots for relational specifications.

Define the intended partial-correctness judgment:

```text
PC(p, P, Q) :=
  for every well-formed initial configuration c satisfying P,
  every modeled normal execution of p from c ends in a configuration satisfying Q.
```

**VC-LOGIC-01.** A postcondition under `PC` is conditional on normal return. It MUST NOT be described as proving termination, absence of allowed abnormal termination, or host availability. Ordinary `Result` errors are normal returns, so their alternatives must be included in `Q` where the program can return them.

For example, a filesystem result contract must not state properties only of the successful byte payload while ignoring an error alternative that also returns the directory handle.

**VC-LOGIC-02.** Preconditions MUST remain visible to callers and admission policy. The evidence validator proves the implication; it does not establish that a particular invocation's inputs satisfy the precondition. Applicability requires a separate proof, a checked decidable guard, or a trusted environmental premise explicitly accepted by policy.

A false precondition makes a partial-correctness claim vacuous. The system must not advertise such a claim as useful correctness for unconstrained inputs.

### 2.2 Safety and total correctness

A prefix-safety claim constrains every finite reachable execution prefix, including those that do not return normally. A total-correctness claim establishes normal termination and its postcondition under stated environment and execution assumptions.

**VC-LOGIC-03.** Reports MUST distinguish `partial-correctness`, `prefix-safety`, and `total-correctness`. A termination argument MUST include the relevant recursion, loops, called programs, and host-response assumptions. A pure effect set is not that argument.

**VC-LOGIC-04.** Claims abstracting from quotas or allocation failure MUST say so. A totality result in the ideal semantics does not imply successful completion under arbitrary finite memory, cancellation, execution budgets, or unresponsive hosts.

**VC-LOGIC-05.** Host-state and trace contracts MUST identify the modeled host operations and the assumptions on their responses. No contract may infer real-world availability, authorization, confidentiality, or exactly-once remote effects solely from a `Program` type or effect set.

## 3. Compositional rules

### 3.1 Sequencing

For compatible stack interfaces and the same semantic/host model, the contract logic SHALL establish:

```text
PC(p, P, Q)    PC(q, Q, R)
--------------------------------
PC(compose(p, q), P, R)
```

Predicates are over the full logical configurations described above. Shared ghost constants and world assumptions must remain in scope; resource obligations passed between programs must match. The definition of normal sequencing ensures that `q` does not start after `p` aborts.

**VC-COMPOSE-01.** Contract composition MUST require a proved intermediate implication when the first postcondition and second precondition differ. Matching stack types alone does not discharge that implication. A proof of partial correctness MUST NOT silently acquire the strength of prefix safety or total correctness during composition.

**VC-COMPOSE-02.** Proofs MUST preserve event ordering and explicit ownership transitions. A mathematical set union of effects is not a proof that the actual event traces commute. No generic host-state frame rule is assumed; use requires the appropriate locality/preservation hypotheses for that state model.

### 3.2 Quoting, invocation, and runtime builders

For capturable `v`, executing `quote(v)` pushes that same semantic value and issues no guest host operation. When `v` is a program, it is pushed as a program value, not executed. Finite recipe and interface witnesses remain available as required by SPEC-0001.

**VC-BUILD-01.** Reusable builder theorems MUST quantify over eligible runtime values and compatible runtime program operands. A proof about a literal specialization MUST NOT be reported as covering all values supplied after compilation.

For the builder body:

```text
quote [ + ] compose
```

an intended theorem can state that, for every `I64` capture `n`, the returned program maps a later `I64` input `x` to `wrap64(x + n)` in the ideal pure semantics. Captured data, interface instantiation, and the builder's recipe rules are part of the theorem's subject.

**VC-BUILD-02.** Ordinary `quote`, `compose`, and `call` MUST NOT run a prover. A theorem about a builder family can justify its runtime instances mathematically; an external evidence service MAY later instantiate that theorem or validate structural evidence. Tracking or transporting per-instance certificates is not a requirement of ordinary execution.

**VC-BUILD-03.** A higher-order contract MUST constrain the behavioral properties of program arguments it relies on. `Program<S,T,e>` alone does not imply that an argument increments, sorts, terminates, or preserves a business invariant. Duplicating a program value does not grant independent polymorphic instantiations or missing behavioral assumptions.

## 4. Arithmetic example

For `[ 1 + ]`, the unconditional normal-result claim is:

```text
output = wrap64(input + 1)
```

The stronger inequality `output > input` requires `input < 9223372036854775807` under the baseline's signed `I64` interpretation. At the maximum input, the output is `-9223372036854775808`.

**VC-NUM-01.** Program contracts MUST use Noble's selected arithmetic semantics or prove the preconditions under which mathematical arithmetic agrees with them. A proof over unbounded integers does not automatically apply to `I64`. [B1 §4.4]

An implementation may later provide ordinary checked-arithmetic words returning `Result`; their contracts require those words' own specifications and do not change the meaning of the existing operators.

## 5. Reflection, rewrites, and identity

**VC-REWRITE-01.** Rechecking transformed syntax establishes only the ordinary properties checked by `prepare`. It MUST NOT automatically transfer the original program's behavioral evidence. Evidence for the transformed program requires a fresh proof or an applicable proved transformation rule.

For instance, replacing addition by subtraction can leave the stack signature and empty effect bound unchanged while invalidating an increment contract.

**VC-REWRITE-02.** A transformation claim MUST name its equivalence or preservation relation. Execution-only equivalence, normal-result equivalence, trace preservation, resource behavior, recipe equality, and full contextual equivalence are distinct claims. A backend optimization that retains recipes has a different obligation from a source rewrite that changes program identity.

**VC-ID-01.** Proof evidence and optional behavioral claims MUST be associated separately from `DefinitionId` and `ProgramValueId`. Adding or replacing a proof of unchanged behavior MUST NOT change the program's canonical recipe or identity. Mandatory stack/effect interfaces and semantic dependencies remain part of ordinary identity as already specified.

**VC-ID-02.** Evidence about a closed program MUST bind its captured values, instantiated interfaces, resolved dependencies, builtin semantics, and relevant host contracts. A theorem about a universally quantified family MUST record its quantifiers and any checked instantiation; a function-template identity alone does not identify a particular captured program value.

**VC-ID-03.** A digest is an index or integrity check under the chosen encoding, not proof validity, provenance, or authority. Until Noble's canonical encodings are standardized, all such bindings MUST be versioned experimental representations and must be checked against the actual subject. This specification chooses no stable language hash algorithm.

## 6. Evidence envelope: semantic requirements

The following fields define a conceptual envelope, not a transport format or additional Noble value type:

| Field | Meaning |
|---|---|
| Subject | Exact program/definition and instantiation, or explicitly quantified builder family |
| Semantic context | Language/formal-model revisions, schemas, builtin and host-contract identities |
| Claim | Exact elaborated proposition plus referenced logical definitions |
| Applicability | Preconditions, quantifiers, invocation guards, and environment assumptions |
| Evidence | Proof module/declaration or another explicitly identified evidence object |
| Evidence class | Lean, Aeneas/Lean, Verus, validation, test, review, or assumption |
| Toolchain | Immutable tool/library/configuration identities required to check the evidence |
| Dependencies | Supporting claims, proof assumptions, external models, and bridge status |
| Coverage | What is established and what is explicitly excluded |

**VC-EVIDENCE-01.** An evidence object MUST NOT self-authorize its acceptance policy. The consumer selects the expected claim, allowed assumptions, semantic context, evidence classes, and resource limits independently of untrusted supplied metadata.

**VC-EVIDENCE-02.** Proof verification MUST establish that the theorem matches the intended claim, not only that some theorem is valid. Assumptions and all logically relevant definitions MUST be checked transitively. Renaming a different predicate to the expected display name does not satisfy this requirement.

**VC-EVIDENCE-03.** Evidence dependency records MUST not use circular attestations as justification. Recursive program proofs belong inside the proof logic with appropriate induction/coinduction principles; they are not cycles in an external trust manifest claiming that each component is trusted because the other is.

**VC-EVIDENCE-04.** Proof verification services MUST enforce the isolated-build and independent-recheck requirements of IMPL-V001. Verification of user proof source is not an undocumented step of ordinary Noble `prepare`.

A later evidence-transport specification must select canonical statement/subject encodings, safe export formats, validated decoding, and a secure rejection protocol. This revision defines the binding obligations without claiming those engineering tasks complete.

## 7. Admission and execution are distinct

**VC-ADMIT-01.** Admission SHALL separately evaluate ordinary program validity, optional proof evidence, interface compatibility, compilation/artifact provenance or correspondence, runtime policy, and resource authorization. Satisfying one condition does not waive another.

An intended sequence is:

```text
validate subject and expected interface
  -> validate required evidence against the consumer's claim and assumptions
  -> establish source/recipe-to-executable trust under the selected policy
  -> enforce host policy and authorize concrete resources
  -> invoke the already prepared program
```

Services may reorder independent non-executing checks, but all required conditions must hold before guest execution. Evidence should not cause captured secrets to be published or execution authority to be acquired implicitly.

**VC-ADMIT-02.** Missing evidence, invalid evidence, disallowed assumptions, mismatched subjects, unsupported proof formats, timeout, and internal verifier failure MUST remain distinguishable. Required evidence that is absent or inconclusive MUST fail closed. A permissive host may run ordinary checked programs without optional proofs, but MUST label that choice rather than fabricate a verification result.

**VC-ADMIT-03.** A proof about source or a recipe MUST NOT be used to justify unrelated supplied Wasm. An accepted artifact MUST be tied to that subject through an explicitly trusted build, a verified compilation chain, or appropriate translation validation, each with its real trust boundary reported.

**VC-ADMIT-04.** Cached acceptance results MUST bind the complete applicable subject, claim, assumptions, semantic/toolchain context, and current acceptance-policy revision. A change that affects applicability MUST trigger revalidation. Immutable old evidence may remain valid for its old subject without being acceptable under the current policy.

## 8. First supported contract fragment

**VC-BOOT-01.** The first implementation SHOULD cover pure scalar/structural-data postconditions, arithmetic with explicit wrap semantics, and quotation/builder-family composition. Host trace/resource protocols and termination proofs may follow using the same Lean semantics.

The logic is not claimed to be decidable, complete, or fully automated. Proof search failure does not establish falsity. Optional proof syntax, general user-facing refinement inference, compressed certificates, proof markets, and an in-guest proof kernel remain outside this revision.

The required soundness obligations are PO-15 and PO-16; all corresponding fixtures are currently `not-run`.
