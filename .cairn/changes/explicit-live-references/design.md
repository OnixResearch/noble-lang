## Context

K-SCOPE-02 and K-DEF-01/02 preserve immutable checked Programs, exact dependencies and an immutable submission namespace. `Live-Wasm-Draft` reload changes only new top-level resolution through an atomic rebuilt namespace; its own slot exclusion remains true. P-ID-06 separates evidence from identity. Issue #1 asks for an explicit late target without weakening any of those rules. r[K-LSLOT-01]

## Decisions

### Decision: Typed generic instruction with invocation-only input

**Choice:** Only `Live-Slot-Design` adds `slot.invoke`. An independently checked, immutable generic caller has an instruction typed against ordered `I -- O ! E`; its explicit invocation input includes a borrowed `LiveRef<I,O,E>` issued by the host. The reference is non-Data/non-Capture, cannot be quoted, stored, returned, exported, serialized or kept past its root invocation. Neither caller recipe nor ProgramValueId embeds a slot ID, name, target, version or credential. The instruction's checked effects are `E ∪ {live.dispatch}`. Ordinary `[ n ]`, saved Programs and their exact resolved dependencies stay untouched. r[K-LSLOT-01] r[K-LSLOT-02]

**Rationale:** The checked caller describes dispatch generically; the selected target is invocation context, never retroactive mutation of the caller or an implicit lookup by source name. Type/effect compatibility is necessary, not host permission.

**Nested inputs:** A root intending A→B→C receives independently host-issued A, B and C typed refs as separate ordered invocation inputs, not from registry names or an A/B recipe. The generic caller borrows A and passes B/C as explicit ordered operands to checked A; A borrows B and passes C to checked B; B borrows C for `slot.invoke`. Each target's exact checked ordered input includes its forwarded borrowed refs, and its effects include each transitive `live.dispatch`. No ref is an output, captured environment or reusable value after root completion. r[K-LSLOT-02]

### Decision: One pinned root registry snapshot, per-dispatch authority

**Choice:** Host root entry atomically pins an immutable persistent map and monotonically increasing global epoch in O(1) map-reference work. Every nested/transitive `slot.invoke` in that root selects from precisely that map, even if concurrent publication installs another map. The host separately authorizes each dispatch against *current* policy and each concrete requested effect, including from pinned old code. A slot ID or possession of `LiveRef`, effect declaration or proof is never authority. The ref ends at root completion; independently pinned registry maps and saved exact Programs may retain code thereafter. r[H-LSLOT-01] r[S-LSLOT-01]

**Rationale:** Parent A entering at epoch 1 continues to call B-v1 even if B-v2 publishes at epoch 2 before its nested call; the next root observes epoch 2. Mixing epochs within one root would make transitive behavior and evidence ambiguous.

**Last-pin accounting:** Current registry maps and their referenced code remain live. Obsolete maps are retired only after their final root/frame pin releases; an old immutable Program independently keeps its exact code reachable until its own last owner releases. Once no current map, in-flight frame or saved exact Program reaches a code version, retire that version and reclaim its retained-code budget. Capacity admission evaluates the post-publication reachable set atomically: while either old owner pins v1, a quota-one v2 replacement refuses; after both release, a newly authorized v2 CAS can retire v1 and publish. r[H-LSLOT-01] r[S-LSLOT-01]

### Decision: Checked transactional publication, not migration

**Choice:** Before any candidate body execution, independently admit its exact immutable ProgramValueId/DefinitionId, ordered stack/resource/schema interface and effects no wider than the slot ceiling; reject incompatible or unavailable contracts. A host-authorized publication compares expected *global epoch plus slot identity/generation* and atomically installs a new immutable map with strictly incremented epoch/generation. Competing stale proposals, including equal old target IDs after a rollback, reject (no ABA). Rollback is a newly authorized CAS and epoch, never mutation of past roots, replayed effects or state/resource migration. Reserve retained old-version capacity before publishing; refuse on exhaustion without invalidating old roots or saved Programs. r[H-LSLOT-01] r[S-LSLOT-01]

**Rationale:** Type safety alone cannot justify publication, and a saved exact Program is not a dynamic slot. Persistent snapshots make concurrent root selection coherent while retention remains bounded.

### Decision: Exact target evidence, inert reflection and reproducible trace

**Choice:** If host policy requests a behavioral claim, separately validate evidence for the *selected* immutable target under P-ID-06: exact ProgramValueId/DefinitionId, captures, instantiated interface, semantic context, claim, assumptions and admitted artifact correspondence. A generic caller proof or old target proof does not transfer. Reflection reports the generic instruction/interface without consulting the registry. The invocation trace records root epoch, slot identity/generation, exact selected target, applicable evidence/policy decisions, actual effects and nested selections. Portable replay requires an explicit frozen admitted registry **and** exact epoch, slot incarnation/generation, selected artifact/captures, semantic context, host-policy inputs and ordered request/response/accounting trace; wrong or missing bindings refuse. A replay-eligible record is not behavioral proof or authorization for new real-world effects. r[VC-LSLOT-01] r[DX-LSLOT-01] r[EV-LSLOT-01]

**Rationale:** A future publication changes selected behavior without changing the caller; only a version-bound execution record can support a behavioral or replay claim.

## Risks / Trade-offs

- The host must implement atomic map+epoch pinning, strict generation CAS and bounded retention; this document does not show runtime or proof feasibility.
- `LiveRef` is an ephemeral host-issued input, not a portable value or session permission. Selection remains unavailable in Core and guarded `Live-Wasm-Draft`, whose existing no-slot rule stays intact. r[B-LSLOT-01]
- Old pinned executions can request real effects after a newer publication. Revocation must be checked at every dispatch/effect; failed publication cannot roll back already executed work. r[S-LSLOT-01]
