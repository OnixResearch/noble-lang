# Independently admitted runtime Program template identity

## MODIFIED Requirements

### Requirement: P-ID-03
r[P-ID-03]

Different captured data MUST remain distinguishable in portable program-value identity. Function-template identity alone is insufficient.

When an anonymous closed `Program` returned by accepted `quote` and `compose` operations is presented for an identity-bearing boundary, an independently checked admission MUST derive the target's own definition-template `DefinitionId` from that returned program, rather than reuse the source/guest builder's `DefinitionId`, a generic caller's identity, a slot label, a claimed ID, or executable bytes. Ordinary `quote`, `compose`, `run`, and `reflect` retain their existing behavior; they do not require eager identity derivation, source recompilation, proof search, or host authorization merely to construct or execute a program.

Admission MUST reify a bounded immutable snapshot of the actual selected program's VM recipe, captures, complete instantiated ordered interface, effects, resolved builtin/host/dependency identities, and any semantic lexical/scoped recursive owner. It MUST independently validate the actual graph, types, capture eligibility and required owner references against a checked mapping from accepted resolved source `quote`/`compose` sites through compiler lowering to those VM occurrences and their operands. For each operand, checked source-to-operand dataflow MUST establish whether it derives from a varying root input binding or from a fixed source literal or computed source constant; executing `quote` on a value does not by itself prove that the value is a runtime-variable capture. The mapping MUST bind actual operand values and source-binding lineage, not merely site labels or a matching recipe. A reflected recipe or guest-supplied event stream, ID, source name, memory address, signature descriptor, or artifact manifest is not authentication. If the compiler supplies no independently verifiable mapping, or a relevant constant derivation, capture binding, alias, owner or actual VM occurrence cannot be verified unambiguously, identity-bearing admission MUST refuse before publication or an identity-dependent claim.

The definition-template projection MUST normalize `compose` association and preserve quotation boundaries as in P-RECIPE-03/04. It MUST retain fixed source literals and checked input-independent computed source constants as fixed semantic operands, not turn them into capture slots merely because `quote` executes at runtime; a changed fixed operand changes the template identity. Only an independently established varying root-input lineage MAY create an ordered, typed runtime-capture slot. Logical repeated-slot alias equivalence MUST follow checked source-binding lineage: reusing or recomposing the same binding cannot depend on whether VM graph nodes or captured-value heap cells are shared, while independent bindings with equal value bits MUST NOT be merged into one slot. The projection MUST include the resolved body shape, interface/schema/effect witnesses, semantic dependencies required by P-ID-01, and any semantic lexical/scoped recursive owner; changing which resolved lexical recursive binding is referenced changes the template identity, not a rename of that binding or a different incidental VM code owner, handle or pin. Source spans and NodeIds MAY help authenticate a site mapping only when independently tied to accepted source and compiled operations; alone they establish no provenance and, like source names, VM handles, allocation and optimizer layout, MUST NOT determine canonical template identity. This projection does not erase the actual literal from P-RECIPE-04's exact, inert recipe. The same checked varying-input template with captured `I64(2)` versus `I64(3)` has the same target template identity but different `ProgramValueId`; changing fixed operands, slot shape/aliasing, quotation structure, lexical owner or semantic dependencies changes the template identity. A named source definition and an anonymous template have distinct identity domains, not an interchangeable ID supplied by the builder.

The selected target `ProgramValueId` MUST bind this independently derived template identity, its exact ordered typed capture values and instantiated interface/schema/effect and dependency identity, rather than the builder's identity or a template alone. Any installed Wasm candidate MUST separately correspond to that accepted target and its exact capture environment; template equality or a matching compiler artifact label is insufficient. P-ID-04 still leaves canonical ID bytes, hashing and cross-implementation stability open: this amendment chooses structural relations, not digest strings. P-ID-06/07, VC-LSLOT-01 and EV-LSLOT-01 continue to govern applicable proof, replay, host authority and artifact evidence separately; deriving an ID establishes none of them.

#### Scenario: ID-05 captured anonymous template

- GIVEN the accepted `quote [ + ] compose` builder in ID-05 consumes the same checked varying root-input binding once with `I64(2)` and once with `I64(3)`, with independently authenticated source-to-operand mappings and actual returned programs, rather than two changed fixed source literals
- WHEN identity-bearing admission checks each actual returned program and derives its target template and program-value identities
- THEN the definition template is the same while the two program values differ; neither result borrows the builder's identity or obtains a behavioral proof from this identity relation

#### Scenario: Fixed literals and computed constants are not capture slots

- GIVEN `[ 2 quote [ + ] compose ]` versus `[ 3 quote [ + ] compose ]`, independently checked closed computed-source-constant operands `[ 1 1 + quote [ + ] compose ]` versus `[ 1 2 + quote [ + ] compose ]`, and an accepted `[ quote [ + ] compose ]` builder supplied by the same varying root-input binding with `I64(2)` versus `I64(3)`
- WHEN independently checked source-to-operand derivations and the actual VM graph support identity-bearing admission for each
- THEN the fixed literal pair has different target template identities, the computed-constant pair has different target template identities, and only the varying-input pair shares a target template identity while its program-value identities differ; all exact inert reflected recipes retain their actual literal `2` or `3`

#### Scenario: Logical alias, independent bindings, and incidental heap topology

- GIVEN schematic checked source bindings `x = root I64(2)` and independent `y = root I64(2)`, with one target formed by `q = quote(x); compose(q,q)`, a second by `compose(quote(x),quote(x))`, and a third by `compose(quote(x),quote(y))`; these labels describe source-binding provenance, not additional Noble syntax
- WHEN verified source-site and actual-VM-graph mappings establish the same normalized quoted recipe shape while VM code owners, heap-cell sharing and composition adapters vary
- THEN the first two targets have the same logical repeated-slot alias, template identity and program-value identity despite different physical graph sharing; the third has two independent capture slots and a different template and program-value identity despite equal `I64(2)` payloads and literal-bearing recipes

#### Scenario: Lexical owner and unverifiable site mapping

- GIVEN two checked targets with the same body and captures but different required lexical recursive owners, and another pair with the same lexical owner but different VM code-owner handles or pins
- WHEN independent admission verifies the accepted source binding, compiler site-to-operand mapping, actual VM graph and owner, or encounters a missing, truncated, forged or ambiguous mapping
- THEN different lexical owners distinguish target templates, incidental VM code owners/handles/pins do not, and an unverifiable mapping refuses identity-bearing admission without borrowing an asserted target D

These draft vectors refine the existing unexecuted ID-05 relation without adding canonical case IDs or execution evidence, promoting LSLOT-05/08, or asserting that the current compiler emits the required mapping.
