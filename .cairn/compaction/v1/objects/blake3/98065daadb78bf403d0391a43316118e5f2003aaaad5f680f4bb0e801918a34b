# MC1 experimental contract frontend selection

## MODIFIED Requirements

### Requirement: VC-GATE-01
r[VC-GATE-01]

Before profile implementation acceptance, the project MUST select a versioned contract IR, declaration grammar, companion representation, rule encoding, and API interfaces. The design MUST specify eligibility, concrete construction boundaries, failures, budgets, and evidence decoding. Experimental encodings MUST retain their scope labels and MUST NOT claim portable cross-version interoperability.

MC1 selects experimental `contracts-draft/v1`: a bounded s-expression declaration `(contract 1 name (input ...) (output ...) (params ...) (define ...) (program [ ... ]) (requires ...) (ensures ...))`. Parameters and pure nonrecursive logical definitions are optional. Binding references MUST explicitly distinguish initial input, final output, quantified parameter and logical-definition indices; scalar and structural expression operands MUST be typed. The ordered typed expression graph and the actual kernel-accepted candidate MUST be retained as the obligation subject. Program quotation interfaces MUST be explicit when not inferable. Ghost values MUST NOT synthesize executable literals or resources. Unsupported predicates and invalid bindings MUST remain separate diagnostic outcomes.

The selected MC1 semantic model is resource-free, pure normal-return partial correctness with wrapping `I64` arithmetic. It includes scalar/structural predicates, Boolean connectives, preserved stack tails, and universally quantified addition-builder rules. Every partial logical projection MUST have a defined failure interpretation, and undefined assertions MUST NOT become accepted truths. Totality and host protocols are not inferred.

Companion records are immutable descriptions: contract IR/context, inert offered evidence, and an abstract accepted program/evidence pairing whose construction is restricted to independent acceptance or checked derivation. Experimental rule records MUST name a version, exact subjects/claims, prior premise indices, context and assumptions. Construction and import MUST NOT accept supplied certified flags; replay MUST be finite, bounded and acyclic. Composition MUST retain premises and require its intermediate implication. These interfaces specify MC2 implementation obligations; MC1 host proof reports MUST NOT claim Wasm companion, runtime guard or portable-encoding conformance.

MC1 CLI operations are `noble verify CONTRACT [--emit DIR] [--proof FILE | --refutation FILE] [--timeout-ms N]` and `noble explain-proof CONTRACT`. Verification MUST regenerate the independently selected expected claim, isolate proof source execution, independently recheck the exact declaration and transitive assumptions, and retain the seven claim outcomes. Explanation MUST NOT invoke proof code. Accepted application evidence MUST remain separate from implementation refinement and backend correspondence.

#### Scenario: Versioned typed increment export

- GIVEN a version 1 declaration for the actual accepted `[ 1 + ]` program and the unconditional wrapping postcondition with distinct initial/final references
- WHEN the contract frontend and Lean export execute
- THEN the typed IR and exact proposition are retained and an independently checked theorem is required before reporting proved

#### Scenario: Invalid and unsupported declarations remain distinct

- GIVEN an ordinarily valid increment subject with an unbound variable, a predicate type mismatch or an unsupported host predicate
- WHEN optional contract elaboration runs
- THEN the contract is rejected with the corresponding invalid or unsupported diagnostic without weakening its claim or reclassifying the ordinary subject as ill-typed

#### Scenario: No evidence by compiler success

- GIVEN an accepted program and an emitted contract proposition but absent or invalid application proof material
- WHEN verify or explain-proof reports the result
- THEN no application proved result, termination guarantee, Wasm correspondence or certified runtime admission is reported
