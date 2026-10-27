## Selected model

A model configuration has bounded roots, bounded slots and a finite publication
sequence. An entry map is an ordered finite list of (slot key, generation)
entries, where a later publication prefixes the map so the newest entry shadows
older ones. A root pins one such map at a publication epoch; pinning is the only
way to give a root a map and nothing in the model replaces the pinned map in
place.

A request through a root produces an ordered guest trace: one dispatch event per
request that the pinned map resolves, carrying the exact selected identity, plus
a commit event. Denials and publication notices are *control* events, kept
separately identifiable rather than interleaved into the guest trace.

## Proven properties

`publish_preserves_other_keys` and `publish_selects_the_published_generation`
characterise one single-slot publication; `pin_is_immutable` and
`root_selections_use_pinned_map` state that a root's selections are its pinned
map's; `frozen_entry_map_reference_agrees` relates execution to a
frozen-entry-map reference for the same request sequence; and
`mixed_map_selects_old_and_new` records that two separate single-slot
publications are not one atomic replacement, so a fresh root pinned between them
selects a mixed pair. `denied_dispatch_produces_no_guest_event` states that
authorization is checked even though the selection is frozen, and
`abnormal_prefix_preserves_committed_events` states that a finite abnormal
prefix keeps committed events rather than implying rollback.

## Snapshot consistency versus coordinated-version coherence

Root-snapshot consistency is what this model proves: a root selects from one
map. Coordinated-version coherence — that the pinned map contains a
behaviorally compatible set for a specific refactoring — is *not* proved here
and cannot follow from interface or effect compatibility alone. The
paper-derived relocation counterexample (move an operation between a caller and
callee so mixed selections execute it twice or not at all) needs the two-version
fixture to exhibit, so `mixed_map_selects_old_and_new` is the model-side half of
that witness and the fixture half stays Phase 2.

## Open correspondence and coverage

No correspondence claim relates this model to the JavaScript runtime in
`crates/noble-cli/src/core/runtime/slot-registry.mjs`: the model's
`publish`/`pinRoot`/`authorize` are the design's vocabulary, not a transcription
of the runtime. Denied dispatch and effects can stop execution before the event
the one-event conclusion would count, so no event-count claim is made under any
authorization outcome. Atomic multi-slot replacement, state or resource
migration, general transactional updating, new contextual-effect inference,
liveness and exactly-once remote effects stay outside this change.
