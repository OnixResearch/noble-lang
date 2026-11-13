#!/usr/bin/env node
/**
 * Runtime observations for the VC-LIVE-03 relocation controls. The ordinary CLI proves an
 * all-old root emits once and a sequentially published mixed map can emit twice. A deliberately
 * test-only CLI build reads the registry's current map at each dispatch: holding the root after
 * selecting the old caller, then publishing both compatible new slots, duplicates the caller-to-
 * callee relocation and omits its reverse. The runner keeps those implementation observations
 * separate from the Lean model and makes no model/runtime correspondence claim.
 */
import assert from 'node:assert/strict';
import { cli, expectOutcome, selectedAuthority } from './live_slot_resource_ref_cli.mjs';

const QUOTE = String.fromCharCode(34);
const REQUEST = 'relocated';
const RESPONSE = 'emitted:relocated';
const LEAF_REF = 'LiveRef<I64,I64,test.emit>';
const CALLER_REF = `LiveRef<I64+${LEAF_REF},I64,live.dispatch+test.emit>`;
const REF = 'LiveRef<I64,I64,live.dispatch+test.emit>';
const CEILING = ['live.dispatch', 'test.emit'];
const emitDefinition = name => `def ${name} [ drop ${QUOTE}${REQUEST}${QUOTE} test.emit drop 21 ]`;

const sources = {
  defTraced: emitDefinition('tracedLeaf'),
  emitting: 'tracedLeaf',
  inert: 'drop 21',
  callerEmits: `${QUOTE}${REQUEST}${QUOTE} test.emit drop slot.invoke`,
  callerDelegates: 'slot.invoke',
  root: 'slot.invoke',
};
const authoritySources = {
  defTraced: sources.defTraced,
  leafEmitting: sources.emitting,
  leafInert: sources.inert,
  callerEmits: sources.callerEmits,
  callerDelegates: sources.callerDelegates,
  root: sources.root,
};
const slots = [
  { slotId: 'E', input: ['I64'], output: ['I64'], effectCeiling: ['test.emit'], proofRequired: false },
  { slotId: 'A', input: ['I64', LEAF_REF], output: ['I64'],
    effectCeiling: CEILING, proofRequired: false },
];
const grants = slots.flatMap(slot =>
  ['publish', 'dispatch', 'effect'].map(operation =>
    ({ operation, slotId: slot.slotId, allowed: true })));
const invoke = {
  operation: 'invoke', id: 'root', inputs: [{ kind: 'i64', value: '20' }],
  refs: [{ position: 1, ordinal: 0, slot: 'E' }, { position: 2, ordinal: 1, slot: 'A' }],
};

const distinguished = trace => trace.filter(row =>
  row.operation === 'effect' && row.request === REQUEST && row.response === RESPONSE);
const summarize = root => ({
  epoch: root.epoch,
  stack: root.stack,
  dispatches: root.request_trace.filter(row => row.operation === 'dispatch').map(row => ({
    slotId: row.slotId, programValueId: row.programValueId, definitionId: row.definitionId,
    policyEpoch: row.policyInput.rootEpoch,
  })),
  effects: distinguished(root.request_trace).map(row => ({
    slotId: row.policyInput.slotId, programValueId: row.programValueId,
    definitionId: row.definitionId, request: row.request, response: row.response,
    allowed: row.policyAllowed, policyEpoch: row.policyInput.rootEpoch,
  })),
});

const install = async (session, id, source, inputs, selectedName) => expectOutcome(
  await session.issue({ operation: 'install', id, source, inputs,
    ...(selectedName === undefined ? {} : { selected_name: selectedName }) }), 'installed');

const prepare = async (binary, name) => {
  const session = await cli(binary, name, selectedAuthority({
    sources: authoritySources, slots, grants, effects: CEILING, quota: 16,
  }));
  try {
    expectOutcome(await session.configured, 'configured');
    expectOutcome(await session.issue({ operation: 'define', id: 'defTraced', name: 'tracedLeaf',
      source: sources.defTraced, inputs: [] }), 'definition-retained');
    await install(session, 'leafEmitting', sources.emitting, ['I64'], 'tracedLeaf');
    await install(session, 'leafInert', sources.inert, ['I64']);
    await install(session, 'callerEmits', sources.callerEmits, ['I64', LEAF_REF]);
    await install(session, 'callerDelegates', sources.callerDelegates, ['I64', LEAF_REF]);
    await install(session, 'root', sources.root, ['I64', LEAF_REF, CALLER_REF]);
    return session;
  } catch (error) {
    await session.abort();
    throw error;
  }
};

const publishRequest = (slot, id, epoch) => ({
  operation: 'publish', slot, id, program_index: 0, expected_epoch: String(epoch),
  expected_incarnation: '1', expected_generation: '1',
});

// Run a version pair while publishing both replacement slots between the caller's selection and
// its callee's selection. `old` and `next` always each emit once when selected coherently.
const interleaved = async (binary, mode, name, oldCaller, oldLeaf, nextCaller, nextLeaf) => {
  const session = await prepare(binary, name);
  try {
    for (const id of [nextCaller, nextLeaf]) {
      expectOutcome(await session.issue({ operation: 'candidate', id, program_index: 0 }), 'candidate-staged');
    }
    const bootstrapped = expectOutcome(await session.issue({ operation: 'bootstrap', expected_epoch: '0',
      members: [{ slot: 'E', id: oldLeaf }, { slot: 'A', id: oldCaller }] }), 'bootstrapped');
    const before = summarize(expectOutcome(await session.issue(invoke), 'executed'));
    expectOutcome(await session.issue({ operation: 'hold-checkpoint', import: 'live_select',
      occurrence: 2 }), 'checkpoint-hold-selected');
    session.send(invoke);
    const entered = expectOutcome(await session.next(), 'checkpoint-entered');
    session.send(publishRequest('E', nextLeaf, bootstrapped.epoch));
    session.send(publishRequest('A', nextCaller, BigInt(bootstrapped.epoch) + 1n));
    session.send({ operation: 'resume-checkpoint' });
    const leafPublication = expectOutcome(await session.next(), 'published');
    const callerPublication = expectOutcome(await session.next(), 'published');
    const held = summarize(expectOutcome(await session.next(), 'executed'));
    const fresh = summarize(expectOutcome(await session.issue(invoke), 'executed'));
    const exit = await session.close();
    assert.equal(entered.import, 'live_select');
    assert.equal(entered.ordinal, 0, 'the hold is before the callee slot selection');
    assert.equal(leafPublication.epoch, String(BigInt(bootstrapped.epoch) + 1n));
    assert.equal(callerPublication.epoch, String(BigInt(bootstrapped.epoch) + 2n));
    assert.equal(leafPublication.root, entered.root);
    assert.equal(callerPublication.root, entered.root);
    assert.equal(exit, 0);
    return { before, held, fresh, oldCaller, oldLeaf, nextCaller, nextLeaf,
      pinnedEpoch: bootstrapped.epoch, publishedEpoch: callerPublication.epoch, mode };
  } catch (error) {
    await session.abort();
    throw error;
  }
};

// Publish one slot, execute a root on that intermediate map, then publish the other slot. This
// uses the ordinary authority surface and records the compatible mixed pair itself, not a missing
// slot or a failed guest.
const sequentialMixedPair = async binary => {
  const session = await prepare(binary, 'relocation-sequential-mixed-pair');
  try {
    for (const id of ['callerDelegates', 'leafEmitting']) {
      expectOutcome(await session.issue({ operation: 'candidate', id, program_index: 0 }), 'candidate-staged');
    }
    const bootstrapped = expectOutcome(await session.issue({ operation: 'bootstrap', expected_epoch: '0',
      members: [{ slot: 'E', id: 'leafInert' }, { slot: 'A', id: 'callerEmits' }] }), 'bootstrapped');
    const allOld = summarize(expectOutcome(await session.issue(invoke), 'executed'));
    const leafPublication = expectOutcome(await session.issue(publishRequest(
      'E', 'leafEmitting', bootstrapped.epoch)), 'published');
    const mixed = summarize(expectOutcome(await session.issue(invoke), 'executed'));
    const callerPublication = expectOutcome(await session.issue(publishRequest(
      'A', 'callerDelegates', leafPublication.epoch)), 'published');
    const allNew = summarize(expectOutcome(await session.issue(invoke), 'executed'));
    const exit = await session.close();
    assert.equal(exit, 0);
    return { allOld, mixed, allNew, leafPublication: leafPublication.epoch,
      callerPublication: callerPublication.epoch };
  } catch (error) {
    await session.abort();
    throw error;
  }
};

const verifyInterleaved = (row, mode, expectedHeldSlots) => {
  assert.equal(row.before.effects.length, 1, 'the coherent old pair emits exactly once');
  assert.deepEqual(row.before.effects.map(effect => effect.slotId), [expectedHeldSlots.old]);
  assert.equal(row.held.epoch, row.pinnedEpoch, 'the active root remains pinned to its old epoch');
  assert.deepEqual(row.held.dispatches.map(dispatch => dispatch.slotId), ['A', 'E']);
  assert.deepEqual(row.held.dispatches.map(dispatch => dispatch.policyEpoch), [row.pinnedEpoch, row.pinnedEpoch]);
  assert.equal(row.held.dispatches[0].programValueId, row.before.dispatches[0].programValueId,
    'the held root already selected its old caller before either publication');
  assert.equal(row.fresh.epoch, row.publishedEpoch, 'the next root pins the two-publication map');
  assert.equal(row.fresh.effects.length, 1, 'the coherent new pair emits exactly once');
  assert.deepEqual(row.fresh.effects.map(effect => effect.slotId), [expectedHeldSlots.next]);
  assert.notEqual(row.fresh.dispatches[0].programValueId, row.before.dispatches[0].programValueId);
  assert.equal(row.held.effects.every(effect => effect.allowed), true);

  if (mode === 'pinned') {
    assert.equal(row.held.effects.length, 1, 'the pinned-map build keeps exactly one old event');
    assert.deepEqual(row.held.effects.map(effect => effect.slotId), [expectedHeldSlots.old]);
    assert.deepEqual(row.held.dispatches.map(dispatch => dispatch.programValueId),
      row.before.dispatches.map(dispatch => dispatch.programValueId));
  } else if (row.name === 'duplicate') {
    assert.equal(row.held.effects.length, 2, 'the current-map control duplicates the event');
    assert.deepEqual(row.held.effects.map(effect => effect.slotId), ['A', 'E']);
    assert.equal(row.held.dispatches[1].programValueId, row.fresh.dispatches[1].programValueId,
      'the broken second selection observes the newly published emitting callee');
  } else {
    assert.equal(row.held.effects.length, 0, 'the current-map control omits the event');
    assert.equal(row.held.dispatches[1].programValueId, row.fresh.dispatches[1].programValueId,
      'the broken second selection observes the newly published inert callee');
  }
};

const verifySequential = row => {
  assert.equal(row.allOld.effects.length, 1);
  assert.deepEqual(row.allOld.effects.map(effect => effect.slotId), ['A']);
  assert.equal(row.mixed.epoch, row.leafPublication,
    'the intermediate root is pinned after the callee publication and before the caller publication');
  assert.equal(row.mixed.effects.length, 2,
    'the mixed compatible pair performs the event in both old caller and new callee');
  assert.deepEqual(row.mixed.effects.map(effect => effect.slotId), ['A', 'E']);
  assert.deepEqual(row.mixed.dispatches.map(dispatch => dispatch.slotId), ['A', 'E']);
  assert.equal(row.mixed.dispatches[0].programValueId, row.allOld.dispatches[0].programValueId,
    'the mixed pair keeps the old caller version');
  assert.notEqual(row.mixed.dispatches[1].programValueId, row.allOld.dispatches[1].programValueId,
    'the mixed pair selects the new callee version');
  assert.equal(row.allNew.epoch, row.callerPublication);
  assert.equal(row.allNew.effects.length, 1, 'the all-new pair emits exactly once');
  assert.deepEqual(row.allNew.effects.map(effect => effect.slotId), ['E']);
  assert.notEqual(row.allNew.dispatches[0].programValueId, row.mixed.dispatches[0].programValueId);
  assert.equal(row.allNew.dispatches[1].programValueId, row.mixed.dispatches[1].programValueId);
};

export const run = async (binary, mode = 'pinned') => {
  assert.ok(mode === 'pinned' || mode === 'current-map', `unknown relocation mode: ${mode}`);
  const sequential = await sequentialMixedPair(binary);
  const duplicate = await interleaved(binary, mode, 'relocation-current-map-duplicate',
    'callerEmits', 'leafInert', 'callerDelegates', 'leafEmitting');
  duplicate.name = 'duplicate';
  const omission = await interleaved(binary, mode, 'relocation-current-map-omission',
    'callerDelegates', 'leafEmitting', 'callerEmits', 'leafInert');
  omission.name = 'omission';
  verifySequential(sequential);
  verifyInterleaved(duplicate, mode, { old: 'A', next: 'E' });
  verifyInterleaved(omission, mode, { old: 'E', next: 'A' });
  return { mode, sequential, duplicate, omission };
};

if (process.argv[1]?.endsWith('live_slot_relocation_controls.mjs') === true) {
  const measured = await run(process.argv[2], process.argv[3] ?? 'pinned');
  process.stdout.write(`${JSON.stringify(measured)}\n`);
}
