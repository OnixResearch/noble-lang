## Context

S-CASE-07 records `wasm_valid:true`, `recipe_matches_artifact:false`, `trusted_build:false` and `translation_validation:false`, expecting `admission/correspondence-reject` with zero guest requests and protected operations. The selected Core-only route `noble admit-artifact WASM --effects CLAIMS_JSON [--source HOST_SOURCE] [--allow-effects ...] [--opt off|on]` already exists. It parses candidate `source`, `digest`, `trusted_correspondence` and `allowed` fields, type-checks them and discards them; it refuses unknown manifest fields. A fresh admission engine checks, in order: fresh session, start section and V8 compilation, reflected import module/name/kind, claimed effects against actual imports, host-allowed effects, presence of host-selected source, and exact final bytes against the host's pinned rebuild. Only a byte-exact match reaches instantiation. S-CASE-16's receipt and rows are bound to S-CASE-16's case and to an older source tree; they are not S-CASE-07 evidence. S-HOST-03's selected-route text belongs to the active `safety-wasm-manifest-admission` delta.

## Decisions

### Decision: Reuse the selected route without production changes

**Choice:** The host invoker supplies `--source` and `--opt`. A pure candidate with a correct empty effect claim passes the validity, import and effect checks, so the correspondence decision alone determines the outcome. Unequal final bytes refuse as `correspondence-reject` before instantiation.

**Rationale:** The canonical case asks for correspondence refusal, which the route already enforces. Changing production code would reopen the S-CASE-16 evidence scope without adding a needed capability.

### Decision: Structural absences, not toggles

**Choice:** Record `trusted_build:false` and `translation_validation:false` as absences. The route has no host flag or candidate field for either. A manifest carrying either name refuses as `invalid-manifest`, and an extra CLI flag refuses as usage `invalid-input`. Candidate `trusted_correspondence:true`, a matching digest and a claimed source are parsed and discarded; the gate shows the same refusal with and without these hints.

**Rationale:** An input that does not exist cannot be toggled. Presenting a false toggle would overstate what is measured.

### Decision: Candidates and differential controls

**Choice:** The canonical candidate is a real `2 2 +` artifact produced by the same fresh CLI. It differs from the `1 2 +` compilation in its code push and its inert recipe atom. The host selects `1 2 +`. Positive controls admit the unmodified `1 2 +` artifact (`I64(3)`) and the same `2 2 +` bytes under their actual source (`I64(4)`). Supplemental refusals cover a valid inert empty custom section `00 01 00` appended to the true artifact. A semantic mutation is included only if the frozen WAT contains exactly one `(call $push_i64 (i64.const 1))`. It changes that code push to `2`, keeps the recipe atom and executes to `I64(4)` in an isolated in-process harness engine, which is an executability oracle, not an admission route. It must refuse under both `1 2 +` and `2 2 +` host sources. Missing host source and an `--opt on` mismatch are labeled non-canonical controls.

**Rationale:** A differential on identical bytes isolates the host-selected recipe as the deciding input. Code/recipe divergence shows that no selected recipe corresponds to a forged artifact.

### Decision: Normative scope in S-TYPE-02 only

**Choice:** Modify S-TYPE-02 and leave S-HOST-03 unchanged.

**Rationale:** The active `safety-wasm-manifest-admission` change holds a MODIFIED S-HOST-03 operation. A second competing MODIFIED operation could be silently reverted by a later sync or archive. The existing S-HOST-03 text already requires host-supplied source, pinned options and byte equality on this route.

### Decision: Evidence and promotion order

**Choice:** Sync the delta and regenerate views before the final gate so that the gate binds the final native safety text. Run the gate before editing case state. Promote only S-CASE-07 evidence[0] after a passing immutable receipt. Later source changes require new chronological replays by the release owner.

**Rationale:** This avoids native drift between the accepted receipt and the promoted specification and keeps every earlier receipt a separate historical source scope.

## Risks / Trade-offs

- Correspondence is byte equality with the host's own pinned toolchain, the same compiler family that produced the positive artifacts. It is not an independent implementation, translation validation or a backend proof; PO-17 and PO-18 remain open.
- The selection is Core-only. Declared-binding artifacts, components, signatures, proof objects and general Wasm loading are out of scope.
- The harness executability oracle uses the engine's internal compilation path in-process. It is not an admission route and says nothing about the mutated artifact's safety.
- Refusal reports construct zero counters. The gate corroborates them with fresh-engine host state (no trace, protected operation, compilation, instance, memory or table change); the observation remains finite.
