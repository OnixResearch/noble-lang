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

An isolated LIVE-05 watch slice derived from private source `2cc259596eef`
passed two focused offline real-child controls (one default, one with
`live-test-barrier`). A paired rename to canonical missing-import bytes
`import missing@1 as m` refused at the Core lexer before any declared-module
lookup, with zero guest requests and unchanged old generation/stack; the
last-ACKed-byte gate refused execution until the original selected bytes
were restored, after which old `addone` returned `21`. In the feature-only
control, an accepted `20 addone` paused before top-level commit while a
fsynced paired rename selected the replacement: old `21` completed before
watch sequence 1 ACKed generation 2, then the new definition returned `22`.
Neither control is a canonical LIVE-05 case receipt or proof promotion.
The private scoped receipt at
`$HOME/.cache/noble-live05-host-seal-20261009-scoped-receipt.json` binds the
resulting source commit and bounded logs. The path is not an integrity
binding; its SHA-256 is reported separately because it cannot be embedded
in the commit it binds. Full module/ABI artifact provenance and canonical
proof remain open.

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
The later feature-only FIFO tests stop inside an actual old compiled Wasm
guest frame before a queued reload ACK, and after checked candidate A
compilation before its staging reply, allowing a second complete fsynced
rename B to supersede A. The worker reports 55 default and 57
feature-enabled live tests passing, with separate external `MAP_SHARED`
source-drift refusal. Isolated LIVE06 commit
`a35194da1776747e8478414d04a106162aca7733` later passed 55 default
and 59 feature-enabled live tests. Its two A-active actual guest controls
compare the same A source and selected alias: allow yielded one request and
one protected operation; feature-only host revocation before the old Wasm
request changed binding allowed and active slot true-to-false, yielding one
request, zero protected operations and a terminal trap. A separate B load
did not replace the stale A adapter. The scoped receipt is
`$HOME/.cache/noble-mlive-private-20261007/tmp/live06-a35194d-scoped-receipt.json`;
child stdout/stderr were not saved separately from Rust harness summaries.
That earlier combined graft had not run a guest. In the separate 2026-10-08
isolated Core-bridge cutover, the first focused real-guest test initially
exposed an empty first CodeSpan rather than a backend-owned interval; the
corrected instance-only receipt path then passed. The first complete scoped
`cargo test --locked --offline -j2 -p noble-cli --test live_repl -- --test-threads=2`
run passed 60/62: one test still pinned incidental retained-budget diagnostic
wording, and one real old-Program invocation failed because a zero-length
guest interval falsely intersected a reused nonempty interval. After deleting
that wording assertion, preserving the refusal/unchanged-namespace checks,
and enforcing nonempty intervals on both sides of the overlap test, the same
actual V8 child suite passed 62/62. Four focused `core::live::tests::` unit
cases passed under
`cargo test --locked --offline -j2 -p noble-cli --bin noble core::live::tests:: -- --nocapture`
on test-harness SHA-256
`4435689893eadd3189116126564a31bc86232eeab0835083bd84bed830964a93`,
including first-byte-malformed Wasm stage refusal with generation zero,
unchanged stack, zero host requests and protected operations, followed by
real checked `21` guest execution and instance registration in that same worker.
The first unit fixture wrongly mixed an ordinary frontend with a live
compiler and was corrected to the actual live frontend before that passing
run. A separate direct `noble live repl --engine v8 --source PATH` run used
the selected file at one path: saved `20 [ addone ]` kept its old body (21)
after the file changed from `[ 1 + ]` to `[ 2 + ]`, a new invocation returned
22, reimport of the original bytes restored 21 for new calls, and a real
`code-retired` receipt appeared; each observed step had zero guest requests
and protected operations, with a clean CLI exit. That executed CLI SHA-256
is `fe6f6a83e0b1952d671b31aa4cde2925c04d181bfcb7fac82541549a6d45a2b2`;
Node 24.13.0 binary SHA-256 is
`ad2d5c03bb247d3690f8baf48b2aac572fe0ed5252b4c5bdcfa7987a169b8f3b`.
The LIVE06 donor's 263-file selected source SHA map was
`policy/tool-selection-files.ncl` SHA-256
`4a8bc45c448002dd3f2f3bf285848b832d1583500a08be685aa025f35c5cdb17`;
every entry matched that donor checkout's actual bytes and its then-pinned
Nickel export matched `policy/tool-selection.json`. The separate successor's
270-file Nix/NCL/JSON selection is source-rehashed only, with no new Nickel
export. Neither source selection is by itself
a whole-workspace source-bound assurance gate or a substitute for an immutable
CLI/guest execution receipt.
The later isolated LIVE06 production cutover removed the hardcoded
feature-only policy command and added host-stdin
`:revoke-effect MODULE@VERSION ADAPTER EFFECT` against an allowed manifest
slot and checked immutable root. A read-only independent source review found
no surviving authority bypass and rechecked the actual `test.clock`
before/after bound-guest case and corrected receipt names before runtime.
Pinned nightly-2026-03-21/GCC15/Node 24.13.0 offline V8 live REPL suites
passed 66/66 default and 69/69 feature-enabled tests. In a separate direct
default-CLI child, a typed old A `Program` survived B import, the exact A
binding and active slot revoked true-to-false without a guest request, B ran
through `emit-b` with one request/one protected operation, and the saved old
A trapped through `emit-a` with one request/zero protected operations and a
poisoned session. Foreign and repeated host selections refused; the
real-child tests also exercise a prior unrevoked A allow, monotone denial
after later reimport, and scripted `test.clock` allowed-before/denied-after.
The direct default CLI SHA-256 was
`bb738f38f887c9f08cf40c727af9b0bdb47198e37a85c1f940031ae614cf6676`;
raw direct-child reports and stderr are under
`$HOME/.cache/noble-live06-policy-cutover-20261008-yD7w-tmp/direct-live06-lIcbkl/`.
These scoped controls are not
complete H-LIVE-01/LIVE-05 watch acceptance, complete source-bound LIVE-02,
every LIVE-05 variant or complete canonical LIVE-06 acceptance; effectful/general
transitions, general LIVE-10 cancellation/retirement, interpreter execution,
canonical scenario receipts and source-bound proof remain open.
The canonical LIVE case states remain `absent`/`not-run`/proof `open`; none of the
implementation or acceptance tasks below is complete.

The separate graft onto tested returned-identity source
`1d5ac07674389c7a344d187762f991ed18ca1db9` preserves this LIVE06
donor's code and *historical donor-only* 66/66, 69/69 and direct-CLI receipts.
The clean combined source `ac3f60c9820acd0103a017ca8aad8fe17c3a6041` passed
independent static security and selected-byte provenance review before a
separately authorized, offline **bounded** CLI test slot. Its pinned-nightly
default CLI SHA-256 is
`4024a0c6a1cb5b132f15be7beccd0756a899d2aba017e55bf46a490c8804a63b`;
Node 24.13.0 SHA-256 is
`ad2d5c03bb247d3690f8baf48b2aac572fe0ed5252b4c5bdcfa7987a169b8f3b`.
Targeted Rust real-child
`live06_host_revokes_old_effect_before_actual_retained_wasm_request`,
`revoked_declared_adapter_cannot_reauthorize_on_later_import` and
`revoked_declared_clock_denies_remaining_scripted_effect` all passed, not the
complete combined `live_repl` suite. A separate direct Node/V8 test script
SHA-256 `542938a7c3b7f809706ab8c60362e31634fc89103033c3f20e50758f3504f4e0`
rejected noncanonical versions, wrong adapter/effect and extra host terms,
then revoked exact `ledger@1 emit-a test.emit` from allowed/active to
denied/inactive with no guest or protected operation. B still executed with
one request/one protected operation through `emit-b`; after alias rebind, the
saved A `Program` trapped through `emit-a` with one guest/host request,
zero protected operations, no emitted Text and terminal exit 2. Raw JSONL
reports and the script remain under
`/tmp/noble-ac3f-combined-2dFTCMG6/`; the direct receipt
`live06-exact-host-revoke.receipt.json` has SHA-256
`ea233d815daf2c4f54c8aa06eb06be4d5641c6bc8c76cf0d64a9a81606718094`.
The bounded returned-target/epoch, Worker root and Signed source-epoch checks
are specified in `../runtime-program-template-identity/design.md`. No full
combined suite, Nix/Nickel export, new document-validation gate or seal ran.
The `passed` in `specs/VALIDATION.json` is the donor's historical input set
and is **not** current for these edited documents. Canonical LIVE/LSLOT
cases remain absent/not-run/open, with no Octet/Lean proof claim.

- [ ] [serial] Implement and test in-process Wasm encoder/verifier with independently accepted source, exact selected ABI and isolated staging. r[BE-LIVE-01] r[S-LIVE-01]
- [ ] [serial] Implement resident live CLI/reload/watch, complete dependent rebuild, atomic safepoint publication, stable typed stack and bounded code retirement. r[K-LIVE-01] r[H-LIVE-01] r[H-LIVE-02] r[H-LIVE-03] r[DX-LIVE-01]
- [ ] [serial] Exercise every LIVE case against actual source, selected VM and hostile controls with source-bound receipts and separately classify proof/assumptions before acceptance. r[V-LIVE-01] r[EV-LIVE-01]
- [ ] [serial] Complete host-granted guest self-edit with compiler-bound originating identity, effectful generation observation, typed pure Program proposal during actual Wasm invocation, bounded enqueue, post-return independent admission and one-use generation CAS; exercise LIVE-09/10 including retained-old-Program denial, rearm, traps, effects and guest/file provenance. Keep both cases absent/not-run until executed. r[K-LIVE-02] r[H-LIVE-04]
