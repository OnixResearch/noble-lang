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
enforces the field (`crates/noble-cli/src/c_backend.rs`: optional
`list_length`, default 255, values above 255 refuse, `#define LIST_LIMIT`,
`manifest.runtime_limits.list_length`, `cons` bound check). The delta keeps
the canonical sentence verbatim and inserts a correction rather than
replacing it; see `reconciliation.md`. The owner may prefer a one-sentence
canonical correction at sync time. That choice is item R1 of the checklist.

### 1.2 CB-SANDBOX-01 (commit `e76d8469`, survives)

First paragraph, two exact clause substitutions inside one existing line:

```diff
@@ r[CB-SANDBOX-01] first paragraph
-"before loading admitted bytes"
+"before candidate instructions"
-"clock and process creation/exec"
+"clock, post-load process creation/exec"
```

The rest of the first paragraph is unchanged.

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

History: `da465a52` and `5c1d124c` temporarily extended the clock probes
(vDSO family, direct vvar, `rdtscp`); `27f7fd79` restored the narrower
sentence above. The delta now adds those probes as insertions, so a sync can
no longer drop the canonical wording.

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

No requirement identity or obligation changes with this line.

## 2. Aligned case designs

Both cases remain designs: `state.implementation=absent`,
`state.execution=not-run`, `state.proof=open`, `state.trust=unassessed`,
`evidence=[]`. No receipt exists and no state is promoted.

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
  overflow on append; forged Sum tag. All refuse before dereference.
- Alignment: exercises the exact supplied bound, its manifest retention and
  pre-growth enforcement in the delta's paragraph.

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
- Alignment: exactly the channels the delta adds to the canonical denial
  set, including the TSC-only insufficiency wording.

## 3. Safety obligations created or strengthened

CB-LIMIT-01 addendum:

- retain the exact supplied `limits.list_length` in the build subject and
  manifest; enforce it before list growth; refuse values outside 0..255;
- never silently strip or advertise an unenforced bound;
- reject duplicate JSON object members at every nesting level before policy
  validation.

CB-SANDBOX-01 addendum:

- vDSO clock functions, coarse clocks, direct vvar reads, `rdtsc` and
  `rdtscp` must refuse or be demonstrably contained with no host or elapsed
  time disclosure;
- a TSC-only, offset-TSC, vDSO-only or coarse-clock-only mechanism is not a
  complete barrier; without a reviewed barrier native execution stays
  unavailable;
- the benign `test.emit` control must run under the same confinement.

CB-ADMIT-01 (canonical, unchanged by this delta):

- descriptor-relative normalized opens; reject `.`/`..`, ancestor symlinks,
  magic links, mutable hard links, retargeting and replacement;
- sealed snapshots reused for acceptance, re-emission, compilation, complete
  ELF comparison and launch;
- refusals before effects, never downgraded by a manifest or syntax check.

CB-BUILD-01 (delta disclosure, no canonical edit): the source-only route is
nonadmitted; `--native` and `execute --target c` refuse; no pair of build-key
or manifest digests makes its output native evidence.

## 4. Reconciliation outcome

Seven of eleven delta blocks are byte-identical to canonical. The other four
contain every canonical sentence verbatim plus inserted reviewed text:
CB-BUILD-01 one paragraph, CB-LIMIT-01 two sentences, CB-SANDBOX-01 two
sentences, CB-ADMIT-01 one paragraph. No canonical sentence is deleted or
modified by a sync. The dry-run preview reports `blocked: false` with eleven
`modified` operations; the after-fix `expected_after_hash` is
`11d99f29b0b54218d40ad6804bdde0202a8bcf99711c258c9044536c58434940`.
Details, per-requirement hashes and the exact pre-fix differences are in
`reconciliation.md`.

## 5. Open risks

- R1. The canonical CB-LIMIT-01 status sentence is stale (it says the
  emitter refuses the supplied bound). The delta corrects it additively; a
  later canonical cleanup still needs an owner decision.
- R2. A sync drops the two canonical section headings `## Representation and
  execution` and `## Native trust boundaries`, which sit inside requirement
  blocks and are not regenerated. The integrator must restore them or move
  them out of the blocks before syncing.
- R3. A sync drops the 25 generated scenario designs; `--write-views`
  restores them and checks requirement identities. This must run after any
  legal sync.
- R4. Source-only gaps remain open and disclosed, not fixed: final
  aggregate/Syntax/Program results, `reflect`, typed runtime inputs,
  definitions and host effects refuse; input reads authenticate only the
  final path component; output paths are re-resolved; there is no native
  admission or execution.
- R5. No native runner, compiler pin, mediator or VM lane exists yet. The
  owner has authorized a pinned QEMU TCG lane and sealed hostile native
  testing inside it, but exact QEMU/kernel/image/machine pins remain
  unreviewed, and `--native`/`execute --target c` continue to refuse. No
  native GO follows from this packet.
- R6. All ten CB cases remain absent/not-run/open/unassessed; no evidence
  exists.
- R7. The delta is not synced and the canonical remains on HOLD until a
  separate owner decision.

## 6. Checklist

Please mark one line per item.

1. Canonical CB-LIMIT-01 supplied-limit obligation
   - Accept: the obligation is correct as written and the delta's stale-clause
     correction is sufficient.
   - Reject: the obligation or its status sentence is wrong; state why.
   - Revise: authorize a direct canonical correction of the last sentence
     before sync.

2. Canonical CB-SANDBOX-01 sealed-runner, loader and clock obligations
   - Accept: the obligations are sufficient as written.
   - Reject: a named obligation is insufficient or unsafe; state which.
   - Revise: require additional clock/vvar/TSC, loader or FD obligations.

3. Canonical CB-ADMIT-01 path, snapshot and complete-ELF obligations
   - Accept: the obligations are sufficient as written.
   - Reject: a named obligation is insufficient; state which.
   - Revise: require additional descriptor, snapshot or ELF checks.

4. Delta additions (source-only disclosure, limit correction, clock probes)
   - Accept: all four blocks are canonical plus only the reviewed insertions.
   - Reject: an insertion is inaccurate or overclaims; state which.
   - Revise: require rewritten insertions.

5. Aligned case designs CB-CASE-06 and CB-CASE-08
   - Accept: the designs match the obligations and stay unexecuted.
   - Reject: a case input or expectation is wrong; state which.
   - Revise: require additional controls before any future execution.

6. Integration
   - Accept: authorize the integrator to run `cairn sync --execute` for this
     change after the reviewer pass, then regenerate views and restore the
     two section headings.
   - Reject: keep the canonical and delta on HOLD; state the condition.
   - Revise: authorize sync only after named changes.

## 7. What acceptance does not do

Acceptance of this packet is not native execution permission, not a runner or
compiler pin selection, not a sandbox or VM selection beyond the authorized
pin-and-verify lane, not owner acceptance of the C backend, and not evidence
for any CB case. It only allows the integrator to apply the reviewed delta to
the held canonical text.
