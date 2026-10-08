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
On Linux, `noble live watch --source /absolute/math.noble --engine v8` now
handles a cooperative completed save: `fsync` the same-directory
`math.noble.tmp`, then atomically rename it onto the selected file. The host
pairs the rename events, reads a bounded directory-pinned regular single-link
snapshot, checks its bytes and identity again after Wasm staging, and refuses
a candidate superseded by a later complete save. Only a committed current
generation receives a success ACK with `watch_sequence`, pinned
`source_sha256`, `watch_save_protocol`, and bounded `source_freshness`.
The old rename-only path was removed; this checked watch is not a no-op alias.
Rename provenance alone cannot authenticate later inode bytes: unpaired
renames, in-place partial writes, symlink escapes, hardlinks and changed
candidates do not earn an ACK, while writable mappings or aliases can change
bytes without a selected-directory event. The selected-byte execution gate
refuses observed partial writes and external `MAP_SHARED` drift with zero
guest requests; finite checks do not prove bytes immutable after ACK or
through every in-flight guest step. Explicit `:reload` remains available;
its ACK also identifies pinned committed bytes rather than promising that
a concurrently rewritten pathname still contains them. Unselected or
non-granted effectful definitions/submissions, incompatible replacements,
unstable files and exhausted rebuild budgets fail closed before candidate
worker staging. An effectful old Program requires current host authority,
and typed values remain retained. Failed post-invocation guest admission
retains the completed invocation's stack and unused grant.

Scoped real-child tests exercise selected declared-module watch snapshots,
Core code-span retirement, an old active Wasm frame completing before the
queued reload ACK, and a checked compiled candidate A discarded after a
second fsynced atomic rename B; partial in-place and separate external
`MAP_SHARED` controls refuse before guest execution. Indirect
self-rebind/cycle refusal was also exercised in scoped live-child checks.
These are not complete automatic H-LIVE-01/LIVE-05 acceptance: general
effectful watch, full adversarial in-flight source-change controls,
complete source-bound LIVE-02, every LIVE-05 variant, interpreter execution,
scenario receipts and source-bound proof/assurance remain open. LIVE-06
current-host-policy revocation also remains open; no general revoke API is
established by the selected runtime tests.
The canonical LIVE case states remain `absent`/`not-run`/proof `open`; none of the
implementation or acceptance tasks below is complete.

- [ ] [serial] Implement and test in-process Wasm encoder/verifier with independently accepted source, exact selected ABI and isolated staging. r[BE-LIVE-01] r[S-LIVE-01]
- [ ] [serial] Implement resident live CLI/reload/watch, complete dependent rebuild, atomic safepoint publication, stable typed stack and bounded code retirement. r[K-LIVE-01] r[H-LIVE-01] r[H-LIVE-02] r[H-LIVE-03] r[DX-LIVE-01]
- [ ] [serial] Exercise every LIVE case against actual source, selected VM and hostile controls with source-bound receipts and separately classify proof/assumptions before acceptance. r[V-LIVE-01] r[EV-LIVE-01]
- [ ] [serial] Complete host-granted guest self-edit with compiler-bound originating identity, effectful generation observation, typed pure Program proposal during actual Wasm invocation, bounded enqueue, post-return independent admission and one-use generation CAS; exercise LIVE-09/10 including retained-old-Program denial, rearm, traps, effects and guest/file provenance. Keep both cases absent/not-run until executed. r[K-LIVE-02] r[H-LIVE-04]
