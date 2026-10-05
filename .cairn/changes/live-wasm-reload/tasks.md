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
Automatic `noble live watch SOURCE` is unavailable. `IN_MOVED_TO` gives
pathname provenance but cannot authenticate bytes at rename time: writable
mappings or external hardlinks can mutate a renamed inode without a selected
basename content notification. The unsound automatic path was removed, not
left as a no-op alias. Explicit `:reload` remains the host-controlled path;
its ACK names pinned committed source bytes, not an assertion that a
concurrently rewritten path still contains those bytes. A bounded
postpublication path check reports its observation separately. Source reads
pin a directory descriptor and reject symlink final components. Unselected
or non-granted effectful definitions/submissions, incompatible
replacements, unstable files and exhausted rebuild budgets fail closed before
candidate worker staging. An effectful old Program requires current host
authority, and typed values remain retained. Failed post-invocation guest
admission retains the completed invocation's stack and unused grant.

Indirect self-rebind/cycle refusal was exercised in scoped live-child checks;
general proof and canonical LIVE case acceptance remain open. This is not the
complete automatic H-LIVE-01/LIVE-05 watch profile: declared modules,
effectful watch, full in-flight source-change controls, code retirement,
complete source-bound LIVE-02 acceptance, interpreter execution, complete
scenario receipts and
source-bound proof/assurance remain absent.
The canonical LIVE case states remain `absent`/`not-run`/proof `open`; none of the
implementation or acceptance tasks below is complete.

- [ ] [serial] Implement and test in-process Wasm encoder/verifier with independently accepted source, exact selected ABI and isolated staging. r[BE-LIVE-01] r[S-LIVE-01]
- [ ] [serial] Implement resident live CLI/reload/watch, complete dependent rebuild, atomic safepoint publication, stable typed stack and bounded code retirement. r[K-LIVE-01] r[H-LIVE-01] r[H-LIVE-02] r[H-LIVE-03] r[DX-LIVE-01]
- [ ] [serial] Exercise every LIVE case against actual source, selected VM and hostile controls with source-bound receipts and separately classify proof/assumptions before acceptance. r[V-LIVE-01] r[EV-LIVE-01]
- [ ] [serial] Complete host-granted guest self-edit with compiler-bound originating identity, effectful generation observation, typed pure Program proposal during actual Wasm invocation, bounded enqueue, post-return independent admission and one-use generation CAS; exercise LIVE-09/10 including retained-old-Program denial, rearm, traps, effects and guest/file provenance. Keep both cases absent/not-run until executed. r[K-LIVE-02] r[H-LIVE-04]
