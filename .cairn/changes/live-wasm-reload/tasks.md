## Phase 1: Specification (complete; change remains active)

- [x] [serial] Record opt-in Wasm live reload, immutable dependent rebuild and next-top-level freshness without changing old Program semantics. r[K-LIVE-01] r[H-LIVE-02]
- [x] [serial] Design explicit REPL/watch entrypoints, bounded file snapshots, in-process binary validation, staged publication and fail-closed interpreter selection. r[DX-LIVE-01] r[BE-LIVE-01] r[S-LIVE-01]
- [x] [serial] Add unexecuted scenarios and separate roadmap/status entries without promoting historical evidence. r[EV-LIVE-01] r[V-LIVE-01]
- [x] [serial] Validate the spec with pinned native Cairn: after the Nix-backed wrapper unexpectedly attempted shared auto-GC, `/nix/store/bvz6kl88qkfi15n8ml1xr7p22syykggd-cairn-0.1.0/bin/cairn validate --root . --policy /nix/store/jqz083ahq10fdrakci9xzvf7bl4nh075-source/cairn-policy/generated/cairn-policy.json` ran directly with immutable policy SHA-256 `62b9ade2bc8af10b2c68776dba8abb374a6c7331aae7ad8bb41ca938ef100d27`, no Nix invocation, 15 GiB free disk and 33 GiB available memory; returned `valid: true`, empty issues/findings on 13 changes and 41 specs. This validates specification structure, not any LIVE implementation, execution, proof or acceptance. r[EV-LIVE-01] r[V-LIVE-01]
- [x] [serial] Extend this existing active change with the narrowly host-granted self-edit K-LIVE-02/H-LIVE-04 delta, LIVE-09/10 unexecuted designs, source/receipt boundaries and generated views/ledger; direct scoped Cairn proposal/design/tasks gates and local document conversion/checks passed. Those document checks do not promote any LIVE execution or proof state. r[K-LIVE-02] r[H-LIVE-04] r[EV-LIVE-01]

## Phase 2: Implementation underway; acceptance open

Guarded partial `noble live repl` now stages checked Core definitions with an
in-process Wasm byte emitter and publishes explicit selected-file reloads in
the resident Node/V8 arena. Host-selected file reload and direct definition
admission use bounded Core dependency-order rebuild and atomic publication,
exercised for the acyclic `addone→twice→four` chain. A host-admitted guest
proposal with any current effective dependent refuses instead of rebuilding
it; eligible dependent-free candidates can publish. One
source-generation ACK follows the host publication; a frontend-private
successor may have multiple internal commits. Saved old Programs retain
their original identities and dependencies. Exact `:generation` inspection
reports the host publication generation, actual V8 engine and ordered
checked typed stack without file access, worker submission or grant
consumption; it is not full DX-LIVE-01 or accepted LIVE case evidence.
Cooperative `noble live watch --source PATH` admits paired `.tmp` rename
notifications and rechecks bounded selected bytes before publication.
`IN_MOVED_TO` alone cannot authenticate later inode bytes: writable mappings
or external hardlinks can mutate the renamed inode without a selected
basename event. Explicit `:reload` remains available; a watch or reload ACK
names checked committed source bytes, not an assertion that a subsequently
rewritten path still contains those bytes. A bounded postpublication path
check reports its observation separately. Source reads pin a directory
descriptor and reject symlink final components. Unselected
or non-granted effectful definitions/submissions, incompatible
replacements, unstable files and exhausted rebuild budgets fail closed before
candidate worker staging. An effectful old Program requires current host
authority, and typed values remain retained. Failed post-invocation guest
admission retains the completed invocation's stack and unused grant.

Indirect self-rebind/cycle refusal was exercised in scoped live-child checks;
general proof remains open. Integrated source `8b1504b` passed 55 scoped
live CLI tests and five selected real guest traces on CLI SHA-256
`2de6d8ebb2f99c48e370866a9e13892c96fbb8c0af0ec568a7ce4dbb505b4e39`.
They cover selected declared snapshots, Core zero-owner table-span reuse,
one paired watch save and exact queued-proposal root release one-to-zero
after trap/quota. The source manifest SHA-256 is
`33317914fd0f057b9eb45047ac21f052833334ad25910c30fce2154e7c40fd5a`;
the combined scoped guest receipt SHA-256 is
`9376b0d13aa1c0cd2f852925fc9af528821428ee36cb6cdc6044dc6bc7eab5d5`.
Complete H-LIVE-01/LIVE-05 watch, LIVE-02 active in-flight reload,
LIVE-05 forced overlap, LIVE-06 explicit host revoke, effectful/general
transitions, general LIVE-10 cancellation/retirement, interpreter
execution, canonical scenario receipts and source-bound proof remain open.
The canonical LIVE case states remain `absent`/`not-run`/proof `open`; none of the
implementation or acceptance tasks below is complete.

- [ ] [serial] Implement and test in-process Wasm encoder/verifier with independently accepted source, exact selected ABI and isolated staging. r[BE-LIVE-01] r[S-LIVE-01]
- [ ] [serial] Implement resident live CLI/reload/watch, complete dependent rebuild, atomic safepoint publication, stable typed stack and bounded code retirement. r[K-LIVE-01] r[H-LIVE-01] r[H-LIVE-02] r[H-LIVE-03] r[DX-LIVE-01]
- [ ] [serial] Exercise every LIVE case against actual source, selected VM and hostile controls with source-bound receipts and separately classify proof/assumptions before acceptance. r[V-LIVE-01] r[EV-LIVE-01]
- [ ] [serial] Complete host-granted guest self-edit with compiler-bound originating identity, effectful generation observation, typed pure Program proposal during actual Wasm invocation, bounded enqueue, post-return independent admission and one-use generation CAS; exercise LIVE-09/10 including retained-old-Program denial, rearm, traps, effects and guest/file provenance. Keep both cases absent/not-run until executed. r[K-LIVE-02] r[H-LIVE-04]
