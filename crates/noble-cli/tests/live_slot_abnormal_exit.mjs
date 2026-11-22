import assert from 'node:assert/strict';
import { SlotRegistry } from '../src/core/runtime/slot-registry.mjs';
import { cli, expectOutcome, selectedAuthority } from './live_slot_resource_ref_cli.mjs';

// LREF-CASE-01 (`.cairn/changes/live-reference-lifetimes`, r[LREF-EXIT-01], owner decision 6): an
// abnormal root exit releases the root's map pin and both frame pins exactly once, issues no saved
// owner and rolls back no effect. A quota-1 registry holds slot B at epoch 1; root R runs caller A,
// which dispatches B through `slot.invoke`, so R holds two frame pins (A's root frame and B's).
//
//   save-time (continue variant): B-v1 performs `test.emit`, then returns a Program larger than
//     the host's observation bound, so saving R's output fails after the guest settled. The host's
//     release is exact and the session continues: a freshly authorized B-v2 publishes at epoch 2
//     (the quota-1 registry admits it only because R's map pin was released), B-v1 retires, and a
//     new root selects B-v2.
//   quota (poison variant): B-v1 exhausts its step quota. A recorded guest failure is terminal for
//     the shared guest state, so the host releases R's pins, shows a ledger with no pin left, and
//     poisons the session.
//   abandoned frames (in process): a root aborted with both frames still entered. The host itself
//     releases both frame pins, exactly once, and the root's map pin.
//
// Not executed through the shipped CLI: host abort (`test.abort` is not installable in the
// live-slot profile) and worker cancellation (the shipped binary has no fault model for it; a
// failed checkpoint poisons the session).
const binary = process.argv[2];
const ref = 'LiveRef<I64,Program<empty,I64,pure>,test.emit>';
const sources = {
  caller: 'slot.invoke',
  oversized: `"b1" test.emit drop quote ${Array(24).fill('[ 1 + ] compose').join(' ')}`,
  // 2^17 identity leaves exceed the 100000-step root quota.
  spinning: `[ 0 drop ] ${Array(17).fill('dup compose').join(' ')} run quote`,
  fresh: 'quote',
};
const slots = [{ slotId: 'B', input: ['I64'], output: ['Program<empty,I64,pure>'],
  effectCeiling: ['test.emit'], proofRequired: false }];
const grants = ['publish', 'dispatch', 'effect'].map(operation =>
  ({ operation, slotId: 'B', allowed: true }));
const root = { operation: 'invoke', id: 'caller', inputs: [{ kind: 'i64', value: '7' }],
  refs: [{ position: 1, ordinal: 0, slot: 'B' }] };
const exitRow = row => ({ outcome: row.outcome, diagnostic: row.diagnostic,
  request_trace: row.request_trace.map(item => [item.operation, item.outcome]),
  protected_operations: row.protected_operations, saved_owner_tokens_issued: row.stack?.length ?? 0,
  frame_pins: row.frame_pins, exit_release: row.exit_release, session_state: row.session_state });
// Every reply is printed before it is asserted on, so a receipt can quote what a failing CLI did.
const observe = (variant, label, row) => {
  console.log(JSON.stringify({ variant, observation: label, outcome: row.outcome,
    diagnostic: row.diagnostic ?? null, epoch: row.epoch ?? null,
    request_trace: row.request_trace?.map(item => [item.operation, item.outcome]) ?? null,
    frame_pins: row.frame_pins ?? null, exit_release: row.exit_release ?? null,
    session_state: row.session_state ?? null, retire_code_spans: row.retire_code_spans ?? null }));
  return row;
};

async function session(variant, body) {
  const live = await cli(binary, `LREF-CASE-01-${variant}`, selectedAuthority({ sources, quota: 1,
    effects: ['test.emit', 'live.dispatch'], slots, grants }));
  const issue = async (request, outcome) => expectOutcome(await live.issue(request), outcome);
  let complete = false;
  try {
    expectOutcome(await live.configured, 'configured');
    await issue({ operation: 'install', id: 'caller', source: sources.caller, inputs: ['I64', ref] },
      'installed');
    const target = variant === 'quota' ? 'spinning' : 'oversized';
    const installed = await issue({ operation: 'install', id: target, source: sources[target],
      inputs: ['I64'] }, 'installed');
    await issue({ operation: 'candidate', id: target }, 'candidate-staged');
    const first = await issue({ operation: 'publish', slot: 'B', id: target, expected_epoch: '0',
      expected_incarnation: null, expected_generation: null }, 'published');
    assert.equal(first.epoch, '1');
    const result = await body({ live, issue, b1: installed });
    complete = true;
    return result;
  } finally { if (!complete) await live.abort(); }
}

const saveTime = await session('save-time', async ({ live, issue, b1 }) => {
  await issue({ operation: 'install', id: 'fresh', source: sources.fresh, inputs: ['I64'] }, 'installed');
  await issue({ operation: 'candidate', id: 'fresh' }, 'candidate-staged');
  // Withdraw publication, so B-v2 is authorized only after R's exit.
  await issue({ operation: 'policy', grant: { operation: 'publish', slotId: 'B', allowed: false } },
    'policy-updated');
  const failed = observe('save-time', 'failed-root', await live.issue(root));
  assert.equal(failed.outcome, 'execution-failed', JSON.stringify(failed));
  assert.match(failed.diagnostic, /checked captured value exceeds host observation bound/);
  assert.equal(failed.stack, undefined, 'a failed root issues no saved owner');
  assert.deepEqual(failed.request_trace.map(item => [item.operation, item.outcome]),
    [['dispatch', 'allowed'], ['effect', 'performed']]);
  assert.equal(failed.request_trace[0].generation, '1', 'R dispatches B-v1');
  assert.equal(failed.protected_operations, 1, 'the performed effect is reported, not rolled back');
  assert.deepEqual(failed.frame_pins, { entered: 2, released: 2, active: 0, released_on_abort: 0 });
  assert.deepEqual(failed.exit_release, { exact: true });
  assert.equal(failed.session_state, 'retained');
  await issue({ operation: 'policy', grant: { operation: 'publish', slotId: 'B', allowed: true } },
    'policy-updated');
  const published = observe('save-time', 'publication', await live.issue({ operation: 'publish',
    slot: 'B', id: 'fresh', expected_epoch: '1', expected_incarnation: '1',
    expected_generation: '1' }));
  expectOutcome(published, 'published');
  assert.equal(published.epoch, '2', 'the quota-1 registry admits B-v2 only without R\'s map pin');
  const retired = published.retire_code_spans.filter(span => span.generation === Number(b1.generation));
  assert.equal(retired.length, 1, 'B-v1 retires at the publication');
  const next = expectOutcome(observe('save-time', 'next-root', await live.issue(root)), 'executed');
  assert.deepEqual(next.request_trace.map(item => [item.operation, item.outcome, item.generation]),
    [['dispatch', 'allowed', '2']], 'a new root selects B-v2');
  assert.deepEqual(next.frame_pins, { entered: 2, released: 2, active: 0 });
  const [owner] = next.stack;
  assert.equal(owner.kind, 4);
  await issue({ operation: 'release-program', owner: owner.owner }, 'program-released');
  const exit = await live.close();
  assert.equal(exit, 0, 'the continued session ends normally');
  return { failed_root: exitRow(failed), publication: { outcome: published.outcome,
    epoch: published.epoch, retired_b1_spans: retired },
  next_root: { outcome: next.outcome, epoch: next.epoch, generation: next.request_trace[0].generation,
    frame_pins: next.frame_pins }, exit };
});

const quota = await session('quota', async ({ live }) => {
  const failed = observe('quota', 'failed-root', await live.issue(root));
  assert.equal(failed.outcome, 'execution-failed', JSON.stringify(failed));
  assert.match(failed.diagnostic, /checked guest execution returned status/);
  assert.equal(failed.stack, undefined, 'a failed root issues no saved owner');
  assert.deepEqual(failed.request_trace.map(item => [item.operation, item.outcome]),
    [['dispatch', 'allowed']]);
  assert.deepEqual(failed.frame_pins, { entered: 2, released: 2, active: 0, released_on_abort: 0 },
    'the ledger shows no pin left before the session is poisoned');
  assert.deepEqual(failed.exit_release, { exact: false,
    reason: 'the guest recorded a failure, which is terminal for the shared guest state' });
  assert.equal(failed.session_state, 'terminated-after-guest-failure');
  const exit = await live.close();
  assert.equal(exit, 2, 'the poisoned session exits instead of taking further operator work');
  return { failed_root: exitRow(failed), exit };
});

// In process: root R pins epoch 1 (B-v1) and enters its root frame and B's frame; B-v2 is published
// while R lives, so B-v1 stays retained only through R's map pin. Aborting R with both frames entered
// releases both frame pins from the host ledger and retires B-v1 exactly once.
function abandonedFrames() {
  const digest = 'a'.repeat(64);
  const descriptor = { input: ['I64'], output: ['I64'], effectCeiling: [] };
  const policy = new Map([['publish:B', true], ['dispatch:B', true]]);
  const released = [];
  const registry = new SlotRegistry({
    quota: 2, maxSlots: 128, knownEffects: ['test.emit'],
    authorize: request => policy.get(`${request.operation}:${request.slotId ?? request.nominalId}`) === true,
    performEffect: () => '', previewEffect: () => '',
    retireTarget: (handle, version) => { released.push(`${handle}:${version.programValueId}`); return true; },
    retainProgram: () => true, releaseProgramBackend: () => true, verifyEvidence: () => false,
  });
  const checked = name => ({ programValueId: `P-${name}`, definitionId: `D-${name}`, captures: [],
    interface: descriptor, effects: [], semanticContext: 'C1', recipeSha256: digest, claim: 'Q1',
    assumptions: 'A1', artifactSha256: digest, resourceCatalog: [] });
  const memory = new WebAssembly.Memory({ initial: 20, maximum: 20 });
  const view = new DataView(memory.buffer);
  const frame = (address, parent, handle, pairs) => {
    for (const [offset, value] of [[0, parent], [4, handle], [8, pairs.length], [12, 0]]) {
      view.setUint32(address + offset, value, true);
    }
    pairs.forEach(([position, ordinal], index) => {
      view.setUint32(address + 16 + index * 8, position, true);
      view.setUint32(address + 20 + index * 8, ordinal, true);
    });
  };
  const b1 = registry.admitCandidate(checked('B1'), 101, digest);
  assert.equal(registry.publish({ slotId: 'B', expectedEpoch: 0n, interface: descriptor,
    version: b1 }).outcome, 'published');
  const module = registry.registerModule(digest, [{ site_id: 3, target_input: ['I64'],
    target_output: ['I64'], effect_ceiling: [], selected_ref_logical_position: 1,
    forwarded_source_positions: [], forwarded_target_positions: [] }]);
  const pinned = registry.pinRoot(900, module);
  registry.bindRef(pinned, 1, 7, 'B', descriptor);
  assert.equal(registry.validateBorrowBinding(pinned, module, 1, 7), 1);
  view.setUint32(1050632, 1, true);
  frame(1077248, 0, 900, [[1, 7]]);
  registry.enterFrame(pinned, module, -1, 1077248, 900, memory);
  assert.equal(registry.dispatch(pinned, 7, module, 3).handle, 101);
  frame(1077248 + 64, 1077248, 101, []);
  registry.enterFrame(pinned, module, 3, 1077248 + 64, 101, memory);
  const b2 = registry.admitCandidate(checked('B2'), 102, digest);
  assert.equal(registry.publish({ slotId: 'B', expectedEpoch: 1n, expectedIncarnation: 1n,
    expectedGeneration: 1n, version: b2 }).outcome, 'published');
  assert.deepEqual(released, [], 'R\'s map pin keeps B-v1 retained');
  const pins = registry.abortRoot(pinned);
  assert.deepEqual(pins, { entered: 2, released: 2, active: 0, released_on_abort: 2 },
    'the host releases both entered frame pins itself, exactly once');
  assert.deepEqual(released, ['101:P-B1'], 'releasing R\'s map pin retires B-v1 exactly once');
  assert.equal(registry.retainedVersions, 1, 'only B-v2 stays retained');
  assert.throws(() => registry.abortRoot(pinned), /inactive/, 'R cannot be released twice');
  return { frame_pins: pins, retired: released, retained_versions_after: registry.retainedVersions };
}

console.log(JSON.stringify({ case: 'LREF-CASE-01', save_time: saveTime, quota,
  abandoned_frames: abandonedFrames(), not_executed: {
    host_abort: '`test.abort` is not installable in the live-slot profile',
    worker_cancellation: 'the shipped binary has no fault model for it; a failed checkpoint poisons' } }));
