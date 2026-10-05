# Explicit live-reference language profile

## ADDED Requirements

### Requirement: K-LSLOT-01
r[K-LSLOT-01]

`Live-Slot-Design` is a separate opt-in profile. Ordinary quotation `[ n ]` and checked definitions MUST retain their original immutable resolved dependencies, captures and identity forever; slot publication MUST NOT redirect them or change ordinary `run`. Only an independently checked immutable generic caller in this profile MAY contain the typed `slot.invoke` instruction. Its recipe and ProgramValueId describe the generic operation and ordered interface but MUST NOT capture or encode a slot ID, source name, selected target/version or credential. Its reflection MUST remain inert rather than resolve a current target. This profile does not change K-SCOPE-02, K-DEF-01/02 or `Live-Wasm-Draft`'s exclusion of slots from its prepared code.

#### Scenario: LSLOT-01 immutable capture versus explicit live call

- GIVEN the saved `[ n ]`, generic caller and versioned publication in LSLOT-01
- WHEN both callers receive input 20 after publication
- THEN the saved Program yields 21 and the explicitly live caller yields 22 without changing either caller's identity

### Requirement: K-LSLOT-02
r[K-LSLOT-02]

`slot.invoke` MUST consume or borrow an explicitly supplied host-issued invocation-input `LiveRef<I,O,E>` against the exact ordered `I -- O ! E` target interface; it MUST NOT perform dynamic source-name lookup. The checker MUST assign dispatch the conservative effect `E ∪ {live.dispatch}` and reject incompatible stack/resource shapes or an unavailable contract. `LiveRef` MUST be non-`Data`, non-`Capture`, nonserializable, unforgeable by guest value/bytes/ID conversion, and scoped to the root invocation: no quotation capture, aggregate storage, export, return, or escape beyond that root. Slot IDs and references designate a lookup, never authorize invocation, publication or actual host effects. A nested dispatch MAY borrow a valid input ref but MUST use the root's pinned registry snapshot.

For nested calls, the root host MUST supply every required typed ref (including transitive refs) as separate ordered invocation inputs. A checked caller or selected target MAY borrow/pass through an already supplied ref as an explicit ordered argument to another checked target, with its borrow scoped within that root. Neither a callee nor a caller MAY manufacture a missing ref, embed its slot identity in a recipe, resolve a name to obtain one, capture one in a Program, or return one as a result. The checker MUST account for the exact nested ordered ref-bearing input interface and all transitive `live.dispatch` effects; receipt of a ref never changes the current host authorization boundary.

#### Scenario: LSLOT-06 input-only reference boundary

- GIVEN every capture, aggregate, export, serialization, forgery and lifetime variant in LSLOT-06
- WHEN static and invocation admission check them
- THEN only the host-issued input borrowed by generic checked dispatch is eligible

### Requirement: H-LSLOT-01
r[H-LSLOT-01]

Host publication MUST independently admit a selected immutable candidate without executing its body, verify exact ordered input/output and nominal resource/schema identity and effect inclusion within the slot ceiling, and check current publication authorization. A compare-and-swap MUST include the expected global registry epoch (not `Live-Wasm-Draft` namespace/source generation) and exact slot ID, incarnation and generation; success atomically installs one new persistent registry map and advances the global epoch and slot generation monotonically. Any successful registry update, including an unrelated-slot update or delete/recreate, advances the global epoch; recreation uses a new incarnation. A stale writer MUST fail even if a later target repeats an earlier ProgramValueId (no ABA). Rollback MUST undergo fresh authorization, candidate admission and applicable evidence validation, then a new CAS/epoch; it never edits pinned maps, frames, saved Programs or actual effects. Retained old versions MUST be accounted for under quota before publication, with exhaustion refusing the new publication. No candidate execution, implicit resource/state migration, or publication on partial admission is permitted.

An obsolete persistent map MUST retain its versions while it is reachable from any active root or in-flight frame. Independently saved exact Programs MUST retain their own exact code even after that map is retired. Only after a map is no longer current or pinned, and a code version's final frame/program owner has released it, MAY that code version be retired and its retention charge reclaimed. Publication capacity MUST be decided atomically against versions reachable *after* a proposed commit; when the old current map has no other pins or saved owners, replacing its only version under quota one MAY reclaim it at commit and admit a fresh candidate. Reclamation MUST NOT revoke a surviving owner.

#### Scenario: LSLOT-04 globally serialized publication

- GIVEN same-slot writers, an unrelated-slot race, delete/recreate and rollback variants in LSLOT-04
- WHEN each tries epoch-and-slot-incarnation/generation CAS
- THEN stale proposals refuse, and only freshly admitted authorized rollback publishes a new epoch
