import assert from 'node:assert/strict';
import { SlotRegistry } from '../src/core/runtime/slot-registry.mjs';
import { cli, expectOutcome, selectedAuthority } from './live_slot_resource_ref_cli.mjs';

// OWN-CASE-05 (`.cairn/changes/ownership-lifetime-contract`): "a root that saves a Program, then
// traps before return". OWN-CASE-05 is WITHHELD from promotion: the design's "frame pins are
// released exactly once" is not observable in this profile, because on an abnormal exit the host
// forgets the failed root's frame records and neither releases nor counts the backend frame pins
// (the instance is discarded). Real per-frame release accounting belongs to
// `live-reference-lifetimes` (OWNER DECISION 2026-11-20 decision 6, r[LREF-EXIT-01]).
//
// This lane therefore records only what is measured:
//
// 1. The abnormal exit. The caller dispatches into a target that produces a captured I64 Program
//    and then performs `test.emit`; the effect grant is revoked first, so the guest fails before
//    the root returns. The reply reports no stack (no saved-owner token), the request trace
//    [dispatch allowed, effect denied], the poisoned session state, and the per-root frame-pin
//    ledger of host entry/exit calls. On this policy-denied-effect mechanism the guest unwinds its
//    frames, so the reply reports 2 entered / 2 released / 0 active; that is a count of host calls,
//    not a release measurement, and it does not exercise the abandoned-pin path. The in-process
//    abort below does: it reports the pins it still held (2 / 0 / 2). No `released_by_host`
//    field is emitted.
//
// 2. `test.abort` is not installable in this profile. The live-slot test hosts grant only
//    `test.emit`, and the environment has no `test.abort` row, so a source containing
//    `test.abort` is refused as invalid input (`word test.abort is not available in this
//    environment`). This lane records that refusal as a witness only and never treats it as the
//    case's success condition.
const binary = process.argv[2];

async function abnormalExit() {
  const sources = { caller: 'slot.invoke', target: 'quote "x" test.emit drop' };
  const authority = selectedAuthority({ sources, quota: 24,
    effects: ['test.emit', 'live.dispatch'],
    slots: [{ slotId: 'own', input: ['I64'], output: ['Program<empty,I64,pure>'],
      effectCeiling: ['test.emit'], proofRequired: false }],
    grants: [{ operation: 'publish', slotId: 'own', allowed: true },
      { operation: 'dispatch', slotId: 'own', allowed: true },
      { operation: 'effect', slotId: 'own', allowed: true }] });
  const live = await cli(binary, 'OWN-CASE-05-abnormal-exit', authority);
  let complete = false;
  try {
    expectOutcome(await live.configured, 'configured');
    for (const [id, inputs] of [
      ['caller', ['I64', 'LiveRef<I64,Program<empty,I64,pure>,test.emit>']],
      ['target', ['I64']]]) {
      expectOutcome(await live.issue({ operation: 'install', id, source: sources[id], inputs }),
        'installed');
    }
    expectOutcome(await live.issue({ operation: 'candidate', id: 'target' }), 'candidate-staged');
    expectOutcome(await live.issue({ operation: 'publish', slot: 'own', id: 'target',
      expected_epoch: '0', expected_incarnation: null, expected_generation: null }), 'published');
    expectOutcome(await live.issue({ operation: 'policy',
      grant: { operation: 'effect', slotId: 'own', allowed: false } }), 'policy-updated');
    const refused = await live.issue({ operation: 'invoke', id: 'caller',
      inputs: [{ kind: 'i64', value: '7' }], refs: [{ position: 1, ordinal: 0, slot: 'own' }] });
    assert.equal(refused.outcome, 'execution-failed', JSON.stringify(refused));
    assert.equal(refused.session_state, 'terminated-after-guest-failure');
    assert.equal(refused.stack, undefined, 'a failed root returns no stack and issues no owner');
    assert.deepEqual(refused.request_trace.map(row => [row.operation, row.outcome]),
      [['dispatch', 'allowed'], ['effect', 'denied']]);
    // The reply's ledger is a count of host entry/exit calls, not proof of backend pin release.
    // On this mechanism the guest leaves its frames while unwinding, so the observed triple is
    // 2 entered / 2 released / 0 active; the abandoned-pin abort is exercised only in-process
    // below, and it reports the pins it still holds instead of assigning a release.
    assert.equal(refused.frame_pins.entered, 2, JSON.stringify(refused.frame_pins));
    assert.equal(refused.frame_pins.released + refused.frame_pins.active, 2,
      JSON.stringify(refused.frame_pins));
    // The poisoned host exits instead of answering another operator request; the exit status is the
    // observation that the session did not continue (owner decision 6 selects this branch only
    // because the backend's own frame-pin counter is still not exposed -- see the receipt notes).
    const exit = await live.close();
    assert.equal(exit, 2, 'the failed session exits instead of taking further operator work');
    complete = true;
    return { mechanism: 'policy-denied effect inside a dispatched frame after the Program is produced',
      target_trace: refused.request_trace.map(row => [row.operation, row.outcome]),
      saved_owner_tokens_issued: refused.stack === undefined ? 0 : refused.stack.length,
      frame_pins: { ...refused.frame_pins,
        measurement: 'host entry/exit calls during the run; not proof of backend pin release' },
      frame_pins_unobservable: 'on the abandoned-pin abort path the host neither releases nor '
        + 'counts the failed root\'s frame pins; real per-frame release accounting belongs to '
        + 'live-reference-lifetimes',
      guest_instance: refused.session_state,
      session_state: refused.session_state, post_exit_session: 'exits without another reply',
      guest_requests: refused.guest_requests,
      protected_operations: refused.protected_operations, exit };
  } finally { if (!complete) await live.abort(); }
}

async function abortRefusalWitness() {
  // `test.abort` has no row in the live-slot test-host environment; the invalid-input refusal is
  // recorded as a witness, not as this case's success condition.
  const sources = { abort: '1 test.abort' };
  const authority = selectedAuthority({ sources, quota: 24,
    effects: ['test.emit', 'test.abort', 'live.dispatch'],
    slots: [{ slotId: 'T', input: ['I64'], output: ['I64'],
      effectCeiling: ['test.emit', 'test.abort'], proofRequired: false }],
    grants: [{ operation: 'effect', slotId: 'T', allowed: true }] });
  const live = await cli(binary, 'OWN-CASE-05-abort-witness', authority);
  let complete = false;
  try {
    expectOutcome(await live.configured, 'configured');
    const refused = await live.issue({ operation: 'install', id: 'abort', source: sources.abort,
      inputs: ['I64'] });
    assert.equal(refused.outcome, 'refused', JSON.stringify(refused));
    assert.equal(refused.guest_requests, 0, JSON.stringify(refused));
    assert.equal(refused.protected_operations, 0, JSON.stringify(refused));
    const exit = await live.close();
    complete = true;
    return { source: sources.abort, outcome: refused.outcome, stage: refused.stage,
      diagnostic: refused.diagnostic, guest_requests: refused.guest_requests, exit };
  } finally { if (!complete) await live.abort(); }
}

// The in-process ledger half: a root with two entered dispatch frames that is aborted releases the
// map pin exactly once and reports the one frame pin it still held; the ledger then has no live
// root left. It does not claim a frame-pin release.
const digest = 'a'.repeat(64);
const descriptor = (input = ['I64'], output = ['I64'], effectCeiling = []) =>
  ({ input, output, effectCeiling });
const policy = new Map();
const released = [];
const grants = (operation, slotId, allowed) => policy.set(`${operation}:${slotId}`, allowed);
const host = quota => new SlotRegistry({
  quota, maxSlots: 128, knownEffects: ['test.emit'],
  authorize: request => policy.get(`${request.operation}:${request.slotId ?? request.nominalId}`) === true,
  performEffect: (effect, text, version) => `${version.programValueId}:${effect}:${text}`,
  previewEffect: (effect, text) => effect === 'test.emit' ? `ok-${text}` : null,
  retireTarget: (handle, version) => { released.push(`${handle}:${version.programValueId}`); return true; },
  retainProgram: () => true,
  releaseProgramBackend: () => true,
  verifyEvidence: () => false,
});
const checked = name => ({
  programValueId: `P-${name}`, definitionId: `D-${name}`, captures: [],
  interface: descriptor(), effects: [], semanticContext: 'C1',
  recipeSha256: digest, claim: 'Q1', assumptions: 'A1', artifactSha256: digest, resourceCatalog: [],
});
const memory = new WebAssembly.Memory({ initial: 20, maximum: 20 });
const view = new DataView(memory.buffer);
const FRAME = 1077248;
const ROOT_BINDING_COUNT = 1050632;
const frame = (address, parent, handle, pairs) => {
  view.setUint32(address, parent, true);
  view.setUint32(address + 4, handle, true);
  view.setUint32(address + 8, pairs.length, true);
  view.setUint32(address + 12, 0, true);
  pairs.forEach(([position, ordinal], index) => {
    view.setUint32(address + 16 + index * 8, position, true);
    view.setUint32(address + 20 + index * 8, ordinal, true);
  });
};
const site = () => ({ site_id: 3, target_input: ['I64'], target_output: ['I64'],
  effect_ceiling: [], selected_ref_logical_position: 1,
  forwarded_source_positions: [], forwarded_target_positions: [] });

function ledger() {
  const registry = host(4);
  grants('publish', 'B', true);
  grants('dispatch', 'B', true);
  const b1 = registry.admitCandidate(checked('B1'), 101, digest);
  assert.equal(registry.publish({ slotId: 'B', expectedEpoch: 0n, interface: descriptor(),
    version: b1 }).outcome, 'published');
  const module = registry.registerModule(digest, [site()]);
  const contract = descriptor();
  const root = registry.pinRoot(900, module);
  registry.bindRef(root, 1, 7, 'B', contract);
  assert.equal(registry.validateBorrowBinding(root, module, 1, 7), 1);
  view.setUint32(ROOT_BINDING_COUNT, 1, true);
  frame(FRAME, 0, 900, [[1, 7]]);
  registry.enterFrame(root, module, -1, FRAME, 900, memory);
  assert.equal(registry.dispatch(root, 7, module, 3).handle, 101);
  frame(FRAME + 64, FRAME, 101, []);
  registry.enterFrame(root, module, 3, FRAME + 64, 101, memory);
  const b2 = registry.admitCandidate(checked('B2'), 102, digest);
  assert.equal(registry.publish({ slotId: 'B', expectedEpoch: 1n, expectedIncarnation: 1n,
    expectedGeneration: 1n, version: b2 }).outcome, 'published');
  // The aborted root captured the old map: B1 stays retained while the root is live.
  assert.ok(!released.includes('101:P-B1'), 'a live root must pin its captured map version');
  assert.throws(() => registry.finishRoot(root), /still has an active frame/,
    'a normal root finish is refused while the dispatch frames are entered');
  assert.deepEqual(registry.abortRoot(root), { entered: 2, released: 0, active: 2 },
    'the aborted root reports the frame pins it still held instead of assigning a release');
  assert.deepEqual(released.filter(row => row === '101:P-B1'), ['101:P-B1'],
    'the aborted root releases its map pin exactly once');
  assert.equal(registry.retainedVersions, 1, 'only the new map version remains retained');
  assert.throws(() => registry.rootEpoch(root), /inactive/,
    'the aborted root is gone; no operation can reach its ledger');
  return { retired_exactly_once: ['101:P-B1'], retained_versions_after: registry.retainedVersions,
    normal_finish_refused: true, frame_pins: { entered: 2, released: 0, active: 2 } };
}

const observed = { case: 'OWN-CASE-05', abnormal_exit: await abnormalExit(),
  abort_refusal_witness: await abortRefusalWitness(), host_ledger: ledger() };
console.log(JSON.stringify(observed));
