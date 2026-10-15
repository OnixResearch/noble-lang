# Checked proof automation over untrusted producers

## ADDED Requirements

### Requirement: PAUTO-MAP-01
r[PAUTO-MAP-01]

A proof-authoring tool that offers producer assistance MUST derive every obligation it presents from the trusted claim generator for the exact prepared subject: MC1 `export_lean`, intrinsic contract preparation, or pure proposition parsing. It MUST NOT derive an obligation from producer output. Each presented obligation MUST carry the following:

- A stable structural key: module, version, declaration, revision and site path.
- A digest of its exact generated statement.
- The subject identity and source span.
- The complete ordered input, intermediate and output stacks, including the preserved bottom tail.
- Pre/postcondition roots with distinct initial and final namespaces.
- Each intermediate bridge and result-join premise of a composition.
- The selected wrapping `I64` semantics.
- Explicit assumptions and the model, rule and checker revisions.
- The claim kind.

The obligation map is non-executing and is never authority for acceptance.

#### Scenario: PAUTO-CASE-01 obligation map for the increment proof

- GIVEN the CONTRACT-18 `arithmetic@1` source with its bridge site left open as an editor-only proof hole
- WHEN the obligation map is generated
- THEN it lists the bridge site under a stable structural key with the stacks `tail ++ [x]`, `tail ++ [x] ++ [1]`, `tail ++ [x, 1]` and `tail ++ [wrap64(x+1)]`, the exact generated claim digest and source spans, and it reports no proof outcome other than `unknown`

### Requirement: PAUTO-PRODUCER-01
r[PAUTO-PRODUCER-01]

Templates, helpers, tactic/SMT producers, AI suggestions and other metaprograms MUST be untrusted, bounded producers. Their only outputs are candidate explicit proof text or candidate concrete inputs.

- **Acceptance path.** A candidate MUST pass the existing explicit source proof checker and the independently pinned Lean consumer against the separately regenerated exact claim. A producer MUST NOT supply or alter a claim, axiom, reviewed rule, `use` target, acceptance flag, policy or budget. It MUST NOT be invoked from ordinary `quote`/`compose`/`run`, runtime evidence replay or admission.
- **Budgets.** Producer resource use MUST be charged to a pool disjoint from the checker's limits. Producer exhaustion MUST report `unknown` and MUST remain distinct from checker `timeout` or exhaustion, from rejection and from refutation.
- **Accepted evidence.** Accepted evidence MUST record only the explicit checked term and its exact claim, dependencies and revisions. It MUST replay without rerunning the producer.
- **Visible obligations.** A helper expansion MUST keep every remaining obligation visible. Composition MUST retain its real bridge and result-join obligations, and reflexivity MUST NOT discharge a goal that is not structurally definitional. An unsupported expansion MUST remain visibly unaccepted.

#### Scenario: PAUTO-CASE-09 helper expansion parity

- GIVEN an accepted `poly-refl : (Pi (A Type0) (Pi (x A) (Eq A x x)))` and the goal `(Pi (x (List I64)) (Eq (List I64) x x))`
- WHEN helper `intro*` then `exact-use poly-refl` emits an explicit term, and a separately hand-written `(intro (x (List I64)) (refl x))` is submitted
- THEN both are checked independently against byte-identical generated claim digests, the accepted evidence for the helper result contains no helper reference, and replay without the helper succeeds

#### Scenario: PAUTO-CASE-12 omitted bridge rejection

- GIVEN a producer candidate whose `pc-sequence` omits its `bridge` argument
- WHEN the candidate is spliced and checked
- THEN the source checker rejects it at the `pc-sequence` span, no Lean acceptance occurs, and the goal view still shows the missing bridge obligation

### Requirement: PAUTO-CEX-01
r[PAUTO-CEX-01]

A solver model or generated concrete input MUST be reported only as a candidate counterexample until it has been replayed. Three results are distinct:

- A `disproved` outcome requires an independently accepted refutation of the exact claim under the selected Noble semantics.
- A separate replay of the exact accepted subject on the same input under the relevant compiled profile is an observed execution. It is test-class evidence and MUST NOT by itself establish `disproved`.
- An executable counterexample MAY be advertised only when the accepted refutation and the observed execution agree. It MUST retain the input, the output or trap, the source and artifact identities, and the violated predicate.

Disagreement between semantic and compiled replay MUST be reported as a correspondence discrepancy, not resolved. Wrapping `I64` values and signed boundaries MUST be shown explicitly. A vacuous precondition MUST be flagged rather than advertised as useful correctness.

#### Scenario: PAUTO-CASE-03 replayed signed-overflow counterexample

- GIVEN `[ 1 + ]` with `(ensures (lt (in x) (out y)))` and the boundary candidate `x = 9223372036854775807`
- WHEN the refutation template is checked by the Lean consumer and the compiled subject is run once on that input
- THEN the report shows `disproved` with a semantic refutation plus an observed execution returning `-9223372036854775808`, the violated predicate, and the source and artifact identities; a model without accepted refutation stays `unknown`
