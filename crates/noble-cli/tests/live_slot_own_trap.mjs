import assert from 'node:assert/strict';
import { SlotRegistry } from '../src/core/runtime/slot-registry.mjs';
import { cli, expectOutcome, selectedAuthority } from './live_slot_resource_ref_cli.mjs';

// OWN-CASE-05 (`.cairn/changes/ownership-lifetime-contract`): a root that produces a Program and
// then traps before returning. The design names `test.abort` as the trap. Two measured facts:
//
// 1. The live-slot source profile cannot bind `test.abort`. The kernel bootstrap table ends at
//    `test.emit` (Definition 22); the ordinary Core source environment appends the `test.abort`
//    row (Definition 23, effect id 1) through `adapt_environment`, but the live-slot session's
//    environment is the kernel environment directly, so `prepare` of any source containing
//    `test.abort` fails with the internal diagnostic "inconsistent frontend arena". The profile's
//    `bootstrap_matches` rule also fixes its effect list to `test.emit` plus `live.dispatch`, so
//    adding the row without the kernel rule is refused as "unknown source host effect" or
//    "kernel rejected a changed module operation or adapter contract". This lane records the
//    refusal; it cannot execute the designed trap.
//
// 2. The host ledger's abnormal-exit path can still be observed directly at the registry layer:
//    a root whose captured map is the last owner of a version, and which has active dispatch
//    frames, is aborted. `abortRoot` clears the frame ledger and retires the map-pinned version
//    exactly once. Backend per-root frame-pin counters are not exposed by any operator request or
//    registry accessor, so their exact release is NOT observed here.
const binary = process.argv[2];
const SOURCE = 'Program<I64,I64,pure>';

async function mechanism() {
  // A slot whose ceiling contains `test.emit` and an authorized effect grant select the trusted
  // test hosts; `test.emit` then prepares, while `test.abort` is refused.
  const sources = { emit: '"x" test.emit drop 1 +', emit2: '"x" test.emit drop 1 +',
    abort: '1 test.abort' };
  const authority = selectedAuthority({ sources, quota: 24,
    effects: ['test.emit', 'test.abort', 'live.dispatch'],
    slots: [{ slotId: 'T', input: ['I64'], output: ['I64'],
      effectCeiling: ['test.emit', 'test.abort'], proofRequired: false }],
    grants: [{ operation: 'effect', slotId: 'T', allowed: true }] });
  const live = await cli(binary, 'OWN-CASE-05-mechanism', authority);
  let complete = false;
  try {
    expectOutcome(await live.configured, 'configured');
    expectOutcome(await live.issue({ operation: 'install', id: 'emit', source: sources.emit,
      inputs: ['I64'] }), 'installed');
    const abort = await live.issue({ operation: 'install', id: 'abort', source: sources.abort,
      inputs: ['I64'] });
    assert.equal(abort.outcome, 'refused', JSON.stringify(abort));
    assert.equal(abort.diagnostic, 'inconsistent frontend arena', JSON.stringify(abort));
    assert.equal(abort.guest_requests, 0, JSON.stringify(abort));
    assert.equal(abort.protected_operations, 0, JSON.stringify(abort));
    // The refusal is a command refusal, not a poisoned session: the same test-hosts profile still
    // admits `test.emit` afterwards.
    expectOutcome(await live.issue({ operation: 'install', id: 'emit2', source: sources.emit2,
      inputs: ['I64'] }), 'installed');
    const exit = await live.close();
    complete = true;
    return { abort_refusal: { outcome: abort.outcome, diagnostic: abort.diagnostic,
      guest_requests: abort.guest_requests }, emit_installed: true, exit };
  } finally { if (!complete) await live.abort(); }
}

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
  registry.abortRoot(root);
  assert.deepEqual(released.filter(row => row === '101:P-B1'), ['101:P-B1'],
    'the aborted root releases its map pin exactly once');
  assert.equal(registry.retainedVersions, 1, 'only the new map version remains retained');
  assert.throws(() => registry.rootEpoch(root), /inactive/,
    'the aborted root is gone; no operation can reach its ledger');
  return { retired_exactly_once: ['101:P-B1'], retained_versions_after: registry.retainedVersions,
    normal_finish_refused: true };
}

const observed = { case: 'OWN-CASE-05', designed_mechanism: await mechanism(), host_ledger: ledger() };
console.log(JSON.stringify(observed));
