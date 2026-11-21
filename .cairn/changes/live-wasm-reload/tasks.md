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

The separate combined watch/harness source `727488acef1297ae52f572a53f87f98e91553fe6`
passed focused offline locked real-child watched lexer refusal, old `21` before
fsynced watch sequence 1 / generation 2 ACK then new `22`, actual one-shot
`test.emit:A` followed by `test.abort` terminal trap, and default/stale/foreign
grant denial. The latter three ran with both private feature flags together.
Source/Wasm/import-byte admission checks passed 1/2/5; a separately built
feature-free CLI refused an injected fixture grant with zero guest requests
and still returned ordinary `21`. A direct combined-feature JSONL attempt
stopped at startup generation 1 / sequence 0 on the driver's incorrect
expected initial SHA-256, before any rename or effect request; no direct
combined effect receipt was obtained or rerun after loss of the exclusive
process slot. The external private scoped receipt at
`$HOME/.cache/noble-live05-combined-20261009-scoped-receipt.json` binds the
source, CLI and bounded test logs without embedding its own hash in this
commit. Selected Nix prerequisites and canonical LIVE-05 case/proof remain
absent/not-run/open.

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

A later isolated acceptance slice added `verification/live-cases/gate.mjs`
and its retained receipt `verification/live-cases/acceptance.json` (SHA-256
`54181eb144b5ef8a99bb79e9a9817648d0c77077d49c626482893a2dbcbcd4c4`). The gate
refuses a dirty source scope, pins each LIVE case's exact
id/profile/kind/requirements/input/expected projection, builds the pinned CLI
and the `live_repl` integration binary from the committed tree, runs the
selected `live_repl` tests, drives real `noble live repl` and
`noble live watch` children for every observed field, and writes one
immutable `acceptance.json` outside the repository before it is copied in.
It was measured on commit `004d24c4` with source revision
`sha256:f4abcb1e43efba0405040ec85d11468516585fbad174aaeace08930a564a7d1a`,
CLI SHA-256
`ca4b197c4b9e36d90b34456e71d5e21c122d496264acd9c8a221595fc9b76fb7` and
`live_repl` test-binary SHA-256
`f6f4867d7d5e659e8eb0a94d2aa74677069fea5cb3ebee518524e44581757be3`, over 34
recorded commands and 75 retained JSON reports. LIVE-01 is promoted
(`implemented`/`passed`/`open`/`explicit`): `20 addone` returned 21 on the old
bytes, a fsynced paired rename to the case's after bytes was followed by a
`reload-committed` ACK at the next generation with zero guest requests and zero
protected operations, and the first post-ACK submission produced the stack
`[21, 22]`; no external assembler or optimizer ran because the reload committed
with PATH holding only failing wasm-tools/wasm-opt/binaryen traps and no trap
marker appeared, and the same build's `noble run` and `noble session` kept
Core-Bootstrap reports with `self.propose` unbound. LIVE-06 is promoted: after
`import ledger@1 as l`, `[ l.send ]`, a committed reload to the case's
`ledger@2` bytes and `import ledger@2 as n`, the host-only
`:revoke-effect ledger@1 emit-a test.emit` changed that exact slot from
allowed/active to denied/inactive with zero guest requests; `n.send` still ran
through `emit-b` with one guest request and one protected operation, the case's
`import ledger@2 as l` rebound the original alias while the retained stack kept
its exact A handle, and `run` on the retained Program trapped with request
identity `ledger@1:test.emit` through `emit-a`, one guest request, zero
protected operations, decision deny/policy-denied and a terminal session.
LIVE-07 is promoted:
`--engine interpreter` produced exactly one `engine-selection` report with
outcome `interpreter-unavailable`, exit 2 and no second report within 300 ms,
while `--engine v8` still reported engine `v8`. The selected real-child tests
`live_01_reload_ack_precedes_fresh_next_top_level_call`, three LIVE-06 tests and
`interpreter_choice_refuses_at_startup_without_v8_fallback` passed in the same
build. LIVE-02, LIVE-03 and LIVE-05 remain absent/not-run. A follow-up slice on
this branch corrected the reported startup stalls and widened the gate.
MEASURED, a default-profile file holding a definition plus
`module count@1 [...]` and a declared binding whose signature differs from its
module (`bind ledger@1 test.emit emit-a Bool -- ! test.emit allow`) already
refuse before EOF with one admission/`reload-refused` report, exit 2 and zero
counters, so the earlier "no report and no exit" observation was a probe
artifact rather than a runtime defect; the tests
`mixed_definition_and_module_source_refuses_at_startup_with_one_report` and
`mismatched_declared_binding_refuses_at_startup_with_one_report` pin both
refusals. The gate now builds the feature-only `live-test-barrier` CLI and test
binary after the default-profile observations, shows the default binary ignores
the barrier environment, and cites the seam-gated real-child tests for LIVE-02's
in-flight old frame, publication safepoint and old generation, LIVE-05's racing
and superseded watch variants, and LIVE-03's changed-file-during-read drift.
LIVE-03's declared variants run on a live definition namespace and module root
(module-only selected file plus the case definition as the first declared
submission): the incompatible-schema replacement is refused before an
opaque-schema comparison by the same-version declaration rule, and the
dependency-cycle replacement is refused as `cyclic local module definitions`,
each with the old definition still callable. MEASURED remaining gaps. LIVE-02's
canonical after-reload script `20 twice swap 20 swap run` is a **case input
defect**, established from the canonical spec and the case's own `expected`
fields and not repaired here: `run : S Program<S,T,e> -- T` (language spec,
line 848) requires exactly `[I64]` below the Program, and the script leaves
`[I64, I64, Program]` (`20 twice` pushes 24, `swap` moves the Program up, the
second `20 swap` adds another I64 without changing the Program's depth), so the
CLI refuses it at check (`expected [] I64 Program<…>; actual [] I64 I64
Program`) and `expected.stack_bottom_to_top [24, 21]` is unreachable; the
variant that executes the retained Program with its typed input still returns
the old body. The case text was not edited. LIVE-03's `host-binding-mismatch`
needs a reload that can carry a binding change, which the single startup
manifest cannot; its `initial_module` is not defective as a session state (the
module-only selected file plus the case definition as the first declared
submission reproduces it), but the single-source harness cannot deliver both at
startup, so the mapping is recorded rather than the input changed. LIVE-05's
`"A" test.emit test.abort` submission is refused at resolve because top-level
effects are unbound and `test.abort` has no Core environment row in this
profile; bindings admit only test.emit/test.clock module operations, and the
refusal is the same class as the missing adapt-environment abort row the
OwnCases slice reports for live-slot sessions. LIVE-04,
LIVE-08, LIVE-09 and LIVE-10 are untouched. No proof, Octet or Lean claim is
made; every promoted case keeps `proof: open`, and task 3 below stays unchecked
because not every LIVE case is exercised.

Batch-1 re-run (2026-11-21): Cairn campaign batch 1 merged later runtime
changes (the OWN round-2 frame-pin ring and abort ledger, the PAA-1 worker
assembly), so the same gate ran again on the batch tree `05bac95a`. Its receipt
[`verification/live-cases/acceptance-2026-11-21-b1.json`](../../../verification/live-cases/acceptance-2026-11-21-b1.json)
(SHA-256 `fd5e4a96ba488365b284bbc66d9bde3170d705918b6f26592e021fd2dc769fd5`;
source revision `sha256:13ac9083a61ee517e44db8b617b72419b3b79d7c5840ff78b0f9b4f72c73843a`,
CLI `be2d6ed6…`, `live_repl` test binary `99c540e7…`, 34 commands, 75 reports)
promotes the same LIVE-01, LIVE-06 and LIVE-07. The case records keep citing
the `004d24c4` receipt; no case state changes.

### Amendment proposal: LIVE-02 `input.after_reload_ack`

The canonical case cannot type-check its own `expected` fields. In the
`Live-Wasm-Draft` profile, `run : S Program<S,T,e> -- T` (language spec, line
848) requires the stack below the Program to be exactly its declared input
`S`. After the retained `[ addone ]` Program, the canonical script
`20 twice swap 20 swap run` goes `[Program]` → `[Program, 24]` → `[24,
Program]` → `[24, Program, 20]` → `[24, 20, Program]`; the Program's declared
input is `[I64]`, so `run` is asked for `[I64]` under a stack holding two I64s
and the checker refuses with `expected [] I64 Program<…>; actual [] I64 I64
Program`. `expected.stack_bottom_to_top [24, 21]` is therefore unreachable, and
the in-flight and immutable-capture fields cannot be observed through this
script as written.

Proposed exact replacement for `input.after_reload_ack` (nothing else in the
case changes):

```json
"after_reload_ack": ["20 twice swap run"]
```

Derivation: `[Program]` (retained old Program) → `20` `[Program, 20]` →
`twice` (the rebuilt dependent, new `addone`) `[Program, 24]` → `swap`
`[24, Program]` → `run` (consumes the Program's exact input `[I64]`) `[24,
21]`, which is the case's declared `stack_bottom_to_top`. This keeps the
case's purpose (a new dependent's result beside the old Program's captured
result) and its in-flight condition unchanged. The case text is not edited
here; the amendment is for the owner.

- [ ] [serial] Implement and test in-process Wasm encoder/verifier with independently accepted source, exact selected ABI and isolated staging. r[BE-LIVE-01] r[S-LIVE-01]
- [ ] [serial] Implement resident live CLI/reload/watch, complete dependent rebuild, atomic safepoint publication, stable typed stack and bounded code retirement. r[K-LIVE-01] r[H-LIVE-01] r[H-LIVE-02] r[H-LIVE-03] r[DX-LIVE-01]
- [ ] [serial] Exercise every LIVE case against actual source, selected VM and hostile controls with source-bound receipts and separately classify proof/assumptions before acceptance. r[V-LIVE-01] r[EV-LIVE-01]
- [ ] [serial] Complete host-granted guest self-edit with compiler-bound originating identity, effectful generation observation, typed pure Program proposal during actual Wasm invocation, bounded enqueue, post-return independent admission and one-use generation CAS; exercise LIVE-09/10 including retained-old-Program denial, rearm, traps, effects and guest/file provenance. Keep both cases absent/not-run until executed. r[K-LIVE-02] r[H-LIVE-04]
