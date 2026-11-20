# Review packet: C backend canonical edits, delta and case designs

Prepared for the owner's explicit yes/no and an independent reviewer pass
before any integration. It does not accept the change, does not authorize
`cairn sync --execute`, does not select or admit native execution, and does
not promote any CB case. Canonical C text and all case evidence remain on
HOLD.

Subject under review:

- canonical `.cairn/specs/c-backend/spec.md` at `cairn/wave1`
  (`12b52f9a`), in particular CB-BUILD-01, CB-LIMIT-01, CB-SANDBOX-01 and
  CB-ADMIT-01;
- the eleven `MODIFIED` delta blocks in
  `.cairn/changes/c-backend/specs/c-backend/spec.md` after the
  reconciliation recorded in `reconciliation.md`;
- the aligned designs CB-CASE-06 and CB-CASE-08 in
  `specs/conformance/c-backend-cases.json`.

Revision: this version resolves the independent review record of 2026-11-20
(findings R-01 to R-17 and round-2 notes N-1 to N-6). The delta text changed
for R-01 (`rdtsc` restored), R-06 (limit insertion reworded for post-sync
reading), R-08 and N-5 (input reads refuse symlinks and non-regular or
oversized files) and N-6 (CB-BUILD-01 "not executed" qualifier), and the
preview hashes were refreshed. Packet and reconciliation text changed for
the remaining findings; no canonical text changed.

How the canonical text was reached: `git log -p -- .cairn/specs/c-backend/spec.md`.
The surviving canonical text of CB-BUILD-01 equals the creation commit
`9a2e4092`; the surviving text of CB-LIMIT-01, CB-SANDBOX-01 and CB-ADMIT-01
equals the result of `e76d8469` (with one sentence re-inserted by
`39f1b208`). Intermediate edits in `da465a52`, `5c1d124c`, `77d184eb` and
`39f1b208` were reverted by `27f7fd79` and are not in the reviewed text.

## 1. Direct canonical edits

### 1.1 CB-LIMIT-01 (commit `e76d8469`, survives)

Pre-edit block ended after the first paragraph. Added paragraph:

```diff
@@ r[CB-LIMIT-01] first parent -> e76d8469
+
+A supplied `limits.list_length` is an exact policy value even for a source
+whose values happen to be only I64/Bool. Any profile accepting it MUST retain
+that exact supplied bound in its manifest/build subject and enforce it before
+list growth when lists are supported; a profile that cannot honor the field
+MUST refuse the policy before emission rather than silently strip the bound
+or advertise an unenforced no-op. CB-CASE-01 retains its supplied list bound
+and remains OPEN while the restricted source-only emitter refuses it.
```

History: `da465a52` replaced that last status sentence and appended a
duplicate-member sentence; `39f1b208` re-inserted the status sentence;
`27f7fd79` removed the duplicate-member sentence. The current canonical
therefore equals `e76d8469` byte for byte.

Stale status: the last sentence says the restricted source-only emitter
refuses a supplied list bound. Code inspection shows it accepts, retains and
emits the field (`crates/noble-cli/src/c_backend.rs`: optional
`list_length`, default 255, values above 255 refuse, `#define LIST_LIMIT`,
`manifest.runtime_limits.list_length`, and a pre-`cons` bound check that has
never been executed as native code). The delta keeps the canonical sentence
verbatim and inserts a correction worded to read correctly after a sync: it
names the `e76d8469` state and then the current behaviour. It no longer says
"this first-parent canonical", which would be false once merged. See
`reconciliation.md`. The owner may still prefer a one-sentence direct
canonical correction before sync; that is item 1 "Revise".

### 1.2 CB-SANDBOX-01 (commit `e76d8469`, survives)

First paragraph, two exact clause substitutions inside one existing line:

```diff
@@ r[CB-SANDBOX-01] first paragraph
-"before loading admitted bytes"
+"before candidate instructions"
-"clock and process creation/exec"
+"clock, post-load process creation/exec"
```

The rest of the first paragraph is unchanged. This is a **narrowing**, not a
neutral rewording: every confinement the sentence lists — FD closure, no
ambient credentials or secrets, filesystem/mount confinement, network/clock/
exec blocking and resource bounds — now applies "before candidate
instructions" instead of "before loading admitted bytes", and process
creation/exec blocking is limited to "post-load". Checklist item 2 asks the
owner to accept or reject that relaxation.

Rationale (canonical paragraph 2): "a filter denying all `execve` before the
initial load cannot launch the worker, and leaving `execve`/`execveat`
available afterward permits substitution". Compensating canonical
obligations: the loader, dynamic linker or libc and static-link assumptions,
startup syscalls, syscall filter, FD protocol, kernel/version prerequisites
and target ABI are pinned and tested as one profile; permitted loader/setup
operations must not be usable by guest computation to open authority; the
initial load needs a separately reviewed transition to the fail-closed
post-load no-exec/no-writable-executable-mapping filter; unreviewed imports,
loader escapes, direct file/network/process access and host-clock reads are
denied; and if any restriction cannot be enforced, execution is
blocked/nonselected.

Trigger: paragraph 1's "candidate instructions" and paragraph 2's
"untrusted instructions" denote the same boundary — after pinned
loader/startup setup and before the admitted program's own instructions.
The admitted ELF's pinned startup/runtime code therefore runs before the
post-load filter is active; no guest computation may run in that window.
Whether that narrow window is acceptable is the decision in item 2.

Second paragraph, added in full:

```diff
@@ r[CB-SANDBOX-01] first parent -> e76d8469
+
+The native runner MUST select a reviewed immutable derivation and policy
+separate from the proof-only Lean runner and any package merely cached on
+the host. It MUST enumerate and mount the exact sealed executable and
+loader/library closure read-only, with no writable executable path, close
+all but fixed private protocol/output FDs, and clear ambient secrets and
+credentials. Its initial ELF load requires a separately reviewed transition
+to a fail-closed post-load no-exec/no-writable-executable-mapping filter
+before untrusted instructions; a filter denying all `execve` before the
+initial load cannot launch the worker, and leaving `execve`/`execveat`
+available afterward permits substitution. It MUST deny unreviewed imports,
+loader escapes, direct file/network/process access and host-clock reads.
+Seccomp denial alone does NOT stop vDSO clock calls or x86 `rdtsc`; without
+an owner-selected and verified VM/virtualized-clock or equivalent clock/TSC
+and loader barrier, native execution MUST remain unavailable. A parent-owned
+cgroup/process-tree memory, CPU, task, output and monotonic wall deadline
+MUST kill and reap the entire worker tree; signal, crash, timeout or partial
+output MUST NOT count as success or authorize protected operations.
```

History: `da465a52` temporarily replaced the canonical paragraph-1
escape/"terminate with zero" clause with TSC/containment wording and changed
the paragraph-2 seccomp sentence to `rdtsc`/`rdtscp`; `5c1d124c` narrowed
the paragraph-1 clause to namespace/sandbox escape and added the
vDSO/vvar/`rdtscp` hostile-probe sentence; `27f7fd79` reverted both and
restored the canonical sentences above. `fd54c757` is a twin of `e76d8469`
with the same resulting blob. The delta now adds the clock probes as
insertions, so a sync can no longer drop the canonical wording.

### 1.3 CB-ADMIT-01 (commit `e76d8469`, survives)

Second paragraph, added in full:

```diff
@@ r[CB-ADMIT-01] first parent -> e76d8469
+
+For every build and execution request, the verifier MUST open the host-selected
+source, contract, policy, current authority and candidate through bounded,
+lexically absolute normalized paths under trusted descriptor-relative
+anchors. It MUST reject `.`/`..`, ancestor symlinks and magic links,
+unreviewed mutable hard links, directory retargeting and file replacement;
+path ownership or a prior pathname digest is insufficient. It MUST snapshot
+the exact bytes into sealed read-only descriptors and use those same bytes
+for independent kernel acceptance, trusted C re-emission, reproducible
+native compilation, full ELF inspection and comparison, and launch without
+reopening mutable candidate paths. The independent build MUST compare the
+complete ELF, including inert sections, under pinned deterministic flags,
+compiler, linker, sysroot, startup/runtime, ABI and environment. Unsupported
+architecture, interpreter, dynamic import, load segment, relocation or
+executable/writable mapping MUST refuse before effects, not pass because
+a manifest or syntax check succeeded.
```

History: `da465a52` narrowed the first phrase to "seeking CB-ADMIT-01 native
admission" and added a source-only disclosure paragraph; `27f7fd79` restored
the broader "For every build and execution request" wording and removed the
disclosure paragraph. The delta now adds the source-only disclosure; the
canonical obligation is unchanged.

### 1.4 CB-BUILD-01 (no surviving direct edit)

The canonical block equals its creation text at `9a2e4092`. `da465a52` and
`77d184eb` added source-only paragraphs; `27f7fd79` removed them. The only
CB-BUILD-01 change under review is the delta paragraph described in
`reconciliation.md`.

### 1.5 Document status line (commit `e76d8469`, edited again by `27f7fd79`)

```diff
-Status: Proposed C-Backend-Draft contract; implementation, native sandbox, conformance evidence and backend proof absent
+Status: Proposed C-Backend-Draft contract; restricted inert I64/Bool source emission exists, while native compilation/execution, sandbox, conformance evidence and backend proof remain absent
```

No requirement identity or obligation changes with this line. It is
non-normative preamble, so a sync leaves it untouched, and it now
understates the emitter: it says I64/Bool, while the code emits
I64/Bool/Unit/Text results with internal Pair/Sum/List values. Checklist
item 6 asks whether to correct that line separately; no delta edit does so.

## 2. Aligned case designs

Both cases remain designs: `state.implementation=absent`,
`state.execution=not-run`, `state.proof=open`, `state.trust=unassessed`,
`evidence=[]`. No receipt exists and no state is promoted.

Alignment diff `b294f7d7` ("docs(c-backend): stage strengthened unsynced C
safety delta") changed only these two cases in
`specs/conformance/c-backend-cases.json`:

- CB-CASE-06: each quota control's `list_length_limit` became
  `policy.limits.list_length`; `expected` gained
  `manifest_list_length_by_quota_control: [1, 0]`.
- CB-CASE-08: the harness name changed from
  `isolated-test-only-sandbox-runner-direct-syscall-probe` to
  `isolated-test-only-sandbox-runner-direct-syscall-and-clock-bypass-probes`;
  `input.clock_probe_source_design` was added (AT_SYSINFO_EHDR vDSO
  resolution of `__vdso_clock_gettime`, `__vdso_time` and
  `__vdso_gettimeofday` plus the coarse clock, direct vvar mapping reads,
  `rdtsc` `0f31` and `rdtscp` `0f01f9`, with the trusted runner comparing
  repeated guest clock values and deltas against independent host realtime
  and monotonic samples); the attempts grew from 7 to 15 (seven clock probes
  plus `unshare`); `expected` gained `guest_host_clock_observations: 0` and
  `guest_host_elapsed_time_observations: 0`.

### CB-CASE-06 (CB-LIMIT-01, CB-VALUE-01, CB-GATE-01)

- Harness: `native-list-quota-and-test-only-ABI-index-controls`.
- Source: `20 nil [ 1 + ] swap cons [ ] [ drop run ] list.case`, existing
  fixture `CORE-05`.
- Quota controls: policy `limits.list_length=1` expects normal completion
  with stack `[I64 21]`; `limits.list_length=0` expects
  `list-quota-exhausted-before-cons`.
- Expected: `manifest_list_length_by_quota_control: [1, 0]`, exact at-bound
  stack, one-over is exhaustion rather than success, bounded cleanup, zero
  guest requests after failure, native signal is not success.
- Test-only ABI attempts: valid element; out-of-bounds index; `u64` length
  overflow on append; forged Sum tag. The first is a positive control that
  yields the valid element; the other three refuse before dereference.
- Alignment: exercises the exact supplied bound, its manifest retention and
  pre-growth enforcement — obligations from canonical `e76d8469`, not from
  the delta insertion.
- Open design gap: the quota-control policies are partial overlays
  (`{"limits":{"list_length":1}}`). The current decoder would refuse them
  before the limit values matter: the policy also lacks the outer `profile`,
  `target`, `allowed_effects` and four-field `limits` shape. In addition,
  the limits overlay omits `stack_slots`, `steps` and `output_bytes`, which
  decide the expected `normal` / `[I64 21]` outcome. The policies must be
  completed before any execution; checklist item 5 "Revise".

### CB-CASE-08 (CB-SANDBOX-01, CB-GATE-01)

- Harness:
  `isolated-test-only-sandbox-runner-direct-syscall-and-clock-bypass-probes`.
- Positive control: `"A" test.emit` under invocation `I` and an explicit
  grant, expecting request trace `test.emit:A` and one protected operation.
- Hostile attempts: `openat`, `connect`, `clock_gettime`, `execve`, direct
  secret FD read, `clone`, `unshare(CLONE_NEWUSER|CLONE_NEWNS)` and loader
  escape; vDSO `__vdso_clock_gettime`, `__vdso_time`,
  `__vdso_gettimeofday` and `CLOCK_MONOTONIC_COARSE`; direct vvar read;
  `rdtsc` (`0f31`) and `rdtscp` (`0f01f9`).
- Expected: hostile protected operations 0, disclosed bytes 0, host-clock
  and host-elapsed observations 0, secret FDs 0, production hostile ELF not
  admitted, sandbox absent blocks execution, native crash is not success.
- Alignment: the seven clock probes cover exactly the clock channels the
  delta adds (`rdtsc` and `rdtscp` included), while the remaining attempts
  cover the canonical direct-syscall, secret-FD, namespace-escape and
  loader-escape set. The TSC-only insufficiency is stated in the delta.

## 3. Safety obligations

### Canonical CB-LIMIT-01 (paragraph added by `e76d8469`)

- a supplied `limits.list_length` is an exact policy value even for scalar
  sources;
- any accepting profile retains that exact bound in its manifest/build
  subject and enforces it before list growth **when lists are supported**;
- a profile that cannot honour the field refuses the policy before emission
  rather than stripping the bound or advertising an unenforced no-op;
- the status sentence says CB-CASE-01 remains OPEN while the restricted
  emitter refuses the field. That sentence is stale against the current
  code and is disclosed as stale by the delta insertion (item 1).

### Delta CB-LIMIT-01 insertions

- current emitter behaviour, not a normative range: it accepts an exact
  optional `list_length` from 0 to 255 with default 255 and emits a
  pre-`cons` bound check that has not been executed as native code;
- obligation: contract and policy JSON MUST reject duplicate object member
  names at every nesting level, so an earlier supplied bound cannot be
  overwritten by a later value before validation.

### Canonical CB-SANDBOX-01 (paragraph 1 from `9a2e4092`, narrowed by `e76d8469`; paragraph 2 added by `e76d8469`)

- confinement applies **before candidate instructions** (a narrowing from
  the pre-edit "before loading admitted bytes"; see §1.2): close inherited
  FDs except enumerated private request/response protocol FDs, no ambient
  credentials or secrets, confined filesystem/mount view, no network, clock
  or post-load process creation/exec, no unmediated file/syscall authority,
  and memory/CPU/wall bounds;
- the local runtime/shim, launcher/worker, IPC and host mediator are
  distinct trust subjects; the worker shares no mediator address space,
  callable pointers or protected FDs;
- the loader/dynamic-linker/libc or static-link assumptions, startup
  syscalls, syscall filter, FD protocol, kernel/version prerequisites and
  ABI are pinned and tested as one profile; loader/setup operations must not
  let guest computation open authority; otherwise execution is
  blocked/nonselected;
- hostile direct `openat`, `connect`, `clock_gettime`, `execve`,
  inherited-FD and escape attempts refuse or terminate with zero protected
  operations, with the benign authorized `test.emit` control under the same
  confinement; a native crash is not a successful Noble result;
- reviewed immutable runner derivation and policy, separate from the Lean
  proof runner and cached host packages; exact sealed executable and
  loader/library closure mounted read-only with no writable executable path;
  all but fixed private protocol/output FDs closed; ambient secrets and
  credentials cleared;
- initial ELF load followed by a separately reviewed fail-closed post-load
  no-exec/no-writable-executable-mapping filter before untrusted
  instructions; unreviewed imports, loader escapes, direct
  file/network/process access and host-clock reads denied;
- seccomp denial alone does not stop vDSO clock calls or x86 `rdtsc`;
  without an owner-selected and verified VM/virtualized-clock or equivalent
  clock/TSC and loader barrier, native execution stays unavailable;
- a parent-owned cgroup/process-tree memory, CPU, task, output and monotonic
  wall deadline kills and reaps the whole worker tree; signal, crash,
  timeout or partial output is not success and authorizes no protected
  operation.

### Delta CB-SANDBOX-01 insertions

- hostile vDSO `__vdso_clock_gettime`, `__vdso_time`,
  `__vdso_gettimeofday`, coarse-clock, direct-vvar and x86
  `rdtsc`/`rdtscp` attempts likewise MUST refuse or be demonstrably
  contained without host-time or elapsed-time disclosure;
- trapping or virtualizing only the TSC is insufficient, and a vDSO-only or
  coarse-clock-only mechanism is not equivalent;
- the barrier must also cover direct vvar reads and `rdtscp`; a TSC-only or
  offset-TSC mechanism does not satisfy it.

### Canonical CB-ADMIT-01 (added by `e76d8469`, unchanged by this delta)

- descriptor-relative normalized opens; reject `.`/`..`, ancestor symlinks,
  magic links, mutable hard links, retargeting and replacement;
- sealed snapshots reused for acceptance, re-emission, compilation, complete
  ELF comparison and launch;
- refusals before effects, never downgraded by a manifest or syntax check.

The shipped nonselected `noble build --target c` does not meet this MUST: it
accepts relative and `..` paths, follows ancestor symlinks, seals nothing and
re-resolves outputs. Accepting item 3 accepts the obligation, not
conformance.

### CB-BUILD-01 (delta disclosure, no canonical edit)

The source-only route is nonadmitted; `--native` and `execute --target c`
refuse; no pair of build-key or manifest digests makes its output native
evidence.

## 4. Reconciliation outcome

Seven of eleven delta blocks are byte-identical to the canonical requirement
prose (the canonical also carries generated scenario regions). The other four
contain every canonical sentence verbatim plus inserted reviewed text:
CB-BUILD-01 one paragraph, CB-LIMIT-01 three sentences, CB-SANDBOX-01 two
sentences, CB-ADMIT-01 one paragraph. No canonical sentence is deleted or
modified by a sync. The dry-run preview reports `blocked: false` with eleven
`modified` operations. After the review revisions (R-01, R-06, R-08, N-5,
N-6) the preview was re-run: delta hash
`ebb57a7aff2992febb9fd83021a77d5e516cabc48261854f93db445908573c61`,
`expected_after_hash`
`e2b67dc4087dbf73b245e91bf4f525e8091bfbb86ad3c7aa10564435ee7493d2`.
Details, per-requirement hashes and the exact pre-fix differences are in
`reconciliation.md`.

## 5. Open risks

- R1. The canonical CB-LIMIT-01 status sentence is stale (it says the
  emitter refuses the supplied bound). The delta insertion now names the
  `e76d8469` state and the current behaviour so it reads correctly after a
  sync; a direct canonical correction is still available as item 1
  "Revise".
- R2. A sync drops the two canonical section headings, which sit inside
  requirement blocks and are not regenerated: `## Representation and
  execution` ends the CB-IDENTITY-01 block and belongs between CB-IDENTITY-01
  and CB-NUM-01; `## Native trust boundaries` ends the CB-LIMIT-01 block and
  belongs between CB-LIMIT-01 and CB-HOST-01. Restore both after **every**
  sync. Moving them out of the blocks beforehand is not an option: a `## `
  line inside a delta section ends the MODIFIED section and drops later
  blocks.
- R3. A sync drops the 25 generated scenario designs; `--write-views`
  restores them and checks requirement identities. This must run after any
  legal sync.
- R4. Source-only gaps remain open and disclosed, not fixed: final
  aggregate/Syntax/Program results, `reflect`, typed runtime inputs,
  definitions and host effects refuse; inputs refuse only a final-component
  symlink and non-regular or oversized files — no path component is
  authenticated and nothing is sealed; output paths are re-resolved; there
  is no native admission or execution.
- R5. No native runner, compiler pin, mediator or VM lane exists yet. The
  owner has authorized a pinned QEMU TCG lane and sealed hostile native
  testing inside it, but exact QEMU/kernel/image/machine pins remain
  unreviewed, and `--native`/`execute --target c` continue to refuse. No
  native GO follows from this packet.
- R6. All ten CB cases remain absent/not-run/open/unassessed; no evidence
  exists. CB-CASE-06's quota policies are incomplete overlays and must be
  completed before execution (item 5).
- R7. The delta is not synced and the canonical remains on HOLD until a
  separate owner decision.
- R8. Canonical CB-SANDBOX-01 paragraph 1 allows only "explicitly enumerated
  private request/response protocol FDs", while paragraph 2 keeps "fixed
  private protocol/output FDs" open, and the emitted runtime writes results
  to stdout and failures to stderr. Whether output FDs belong to the
  enumerated set is an owner decision (item 2); resolving it needs a
  separately reviewed canonical edit.

## 6. Checklist

Please mark one line per item.

1. Canonical CB-LIMIT-01 supplied-limit obligation
   - Accept: the obligation is correct as written and the reworded delta
     insertion (it names the `e76d8469` state, then current behaviour) is
     sufficient, leaving the stale canonical sentence in place.
   - Reject: the obligation or its status sentence is wrong; state why.
   - Revise: authorize a one-sentence direct canonical correction of the
     stale status sentence before sync (recorded as a reviewed edit).

2. Canonical CB-SANDBOX-01 sealed-runner, loader and clock obligations
   - Accept: the obligations are sufficient as written.
   - Reject: a named obligation is insufficient or unsafe; state which.
   - Revise: require additional clock/vvar/TSC, loader or FD obligations.
   - Accept/Reject the `e76d8469` timing relaxation **and its reading**:
     confinement now starts "before candidate instructions" rather than
     "before loading admitted bytes", and process creation/exec is blocked
     only "post-load". This line records the interpretation that ¶1
     "candidate instructions" and ¶2 "untrusted instructions" name the same
     boundary, after pinned loader/startup setup and before the admitted
     program's own instructions: pinned startup/runtime code may run before
     the post-load filter, but no guest computation may. A later reviewed
     canonical edit could align the two terms. Compensating obligations are
     in §1.2 and §3; the owner's answer should record both the relaxation
     and this reading.
   - Accept/Revise the FD set: ¶1 enumerates request/response protocol FDs,
     ¶2 also keeps output FDs open, and the runtime writes stdout/stderr.

3. Canonical CB-ADMIT-01 path, snapshot and complete-ELF obligations
   - Accept: the obligations are sufficient as written. This accepts the
     obligation, not conformance: the shipped nonselected `noble build
     --target c` does not meet it (relative/`..` paths, ancestor symlinks,
     no sealing, re-resolved outputs).
   - Reject: a named obligation is insufficient; state which.
   - Revise: require additional descriptor, snapshot or ELF checks.

4. Delta additions (source-only disclosure, limit correction, clock probes)
   - Accept: all four blocks are canonical plus only the reviewed insertions.
   - Reject: an insertion is inaccurate or overclaims; state which.
   - Revise: require rewritten insertions.

5. Aligned case designs CB-CASE-06 and CB-CASE-08
   - Accept: the designs match the obligations and stay unexecuted, on the
     condition that CB-CASE-06's partial quota policies are completed before
     execution.
   - Reject: a case input or expectation is wrong; state which.
   - Revise: require additional controls before any future execution.

6. Document status line (non-normative)
   - Accept: leave "restricted inert I64/Bool source emission exists" as is.
   - Reject/Revise: authorize a separate status-line correction to include
     Unit/Text results and internal aggregates.

7. Integration
   - Accept: authorize the integrator to run `cairn sync --execute` for this
     change after the reviewer pass. This is an intentional mid-change sync:
     the change stays active, nothing is archived, and any later delta edit
     needs a fresh reconciliation and preview. After the sync, regenerate
     views and restore the two section headings at the positions in R2.
   - Reject: keep the canonical and delta on HOLD; state the condition.
   - Revise: authorize sync only after named changes.

## 7. What acceptance does not do

Acceptance of this packet is not native execution permission, not a runner or
compiler pin selection, not a sandbox or VM selection beyond the authorized
pin-and-verify lane, not owner acceptance of the C backend, and not evidence
for any CB case. It only allows the integrator to apply the reviewed delta to
the held canonical text.
