## Why

S-CASE-07 is still `absent/not-run`. Its canonical Wasm-Draft input is a valid artifact whose recipe does not match it, with forged metadata, no trusted build and no translation validation; it expects `admission/correspondence-reject` with zero guest requests and protected operations. The selected Core-only `noble admit-artifact` route already rebuilds host-selected source and compares exact final bytes, but S-TYPE-02 does not state that finite scope or what the canonical `trusted_build` and `translation_validation` fields mean on this route. Relabeling S-CASE-16's effect-manifest evidence would not measure correspondence. r[S-TYPE-02]

## What Changes

- Scope S-TYPE-02 on the existing selected route: a valid submitted artifact remains candidate data until its exact final bytes equal the host's pinned compilation of independently accepted host-selected source; candidate `source`, `digest`, `trusted_correspondence` and `allowed` fields never establish correspondence or runnability. r[S-TYPE-02]
- Define S-CASE-07's `trusted_build:false` and `translation_validation:false` as structural absences of this route, not toggles. A manifest field with either name is refused as an invalid manifest; neither is inferred from candidate metadata. r[S-TYPE-02]
- Execute a new source-bound gate on a freshly built pinned offline CLI: a real `2 2 +` artifact with forged metadata refuses under host-selected `1 2 +`; the same bytes are admitted under their actual `2 2 +` source (`I64(4)`); the unmodified `1 2 +` artifact executes to `I64(3)`; a valid inert custom-section append and a uniquely targeted executable code mutation also refuse. Missing host source and option mismatch are labeled non-canonical controls. r[S-TYPE-02]

## Impact

- **Files**: this change directory; the native safety specification's S-TYPE-02 and its generated views; `verification/scase07/` gate, immutable receipt and postpromotion check; canonical S-CASE-07 state and evidence[0] only after acceptance; a short status and roadmap record. No production code changes. S-HOST-03's selected-route text, held by the active `safety-wasm-manifest-admission` delta, is not modified.
- **Testing**: Freshly built pinned offline CLI; pinned `wasm-tools` validation and V8 compilation of every candidate; exact admission reports and exit codes; fresh-engine host-state corroboration and raw-install refusal; retained external raw outputs. Backend and loader proofs (PO-17/PO-18) remain open.
