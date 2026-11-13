#!/usr/bin/env node
/**
 * VC-LIVE-03 caller→callee relocation witness, at implementation level: two interface-identical
 * versions of the same caller slot move one distinguished event out of the caller and into the
 * callee.
 *
 * The earlier recon stalled on "the caller cannot emit". It cannot emit *in a root frame*: the
 * effect import performs the effect on behalf of the frame's selected target
 * (`crates/noble-cli/src/core/runtime/slot-engine.mjs:359-364` ->
 * `crates/noble-cli/src/core/runtime/slot-registry.mjs:808-813`, where `requestEffect` requires
 * `root.selectedHandles.get(handle)` and `selectedHandles` is filled only by `dispatch`), and the
 * guest's emitted slot site sets its pre-select failure code 5 before it ever selects
 * (`crates/noble-wasm/src/source/emit/live.rs:27-30`). A *dispatched* slot program has such a
 * target, so a caller version that both emits and dispatches is expressible: slot `A` holds
 * `"relocated" test.emit drop slot.invoke`.
 *
 *   old pair: A emits and then invokes an inert E  => dispatch A, effect A, dispatch E
 *   new pair: A only invokes; E's version emits     => dispatch A, dispatch E, effect E
 *
 * Both pairs use the same slot contracts and observe the same stack result; the distinguished
 * row's selected slot and program identity move from the caller's version to the callee's, and
 * each effect row names exactly the program its performing frame selected. A third pair with both
 * versions inert is the omission control.
 */
import assert from 'node:assert/strict';
import { cli, expectOutcome, selectedAuthority } from './live_slot_resource_ref_cli.mjs';

const QUOTE = String.fromCharCode(34);
const emitDefinition = name => `def ${name} [ drop ${QUOTE}relocated${QUOTE} test.emit drop 21 ]`;
const LEAF_REF = 'LiveRef<I64,I64,test.emit>';
const CALLER_REF = `LiveRef<I64+${LEAF_REF},I64,live.dispatch+test.emit>`;
const REQUEST = 'relocated';
const RESPONSE = 'emitted:relocated';

const sources = {
  defTraced: emitDefinition('tracedLeaf'),
  emitting: 'tracedLeaf',
  inert: 'drop 21',
  callerEmits: `${QUOTE}relocated${QUOTE} test.emit drop slot.invoke`,
  callerDelegates: 'slot.invoke',
  root: 'slot.invoke',
};
// The authority binds one source digest per define and per install identity, independently.
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
    effectCeiling: ['live.dispatch', 'test.emit'], proofRequired: false },
];
const grants = slots.flatMap(slot =>
  ['publish', 'dispatch', 'effect'].map(operation =>
    ({ operation, slotId: slot.slotId, allowed: true })));

const distinguished = trace => trace.filter(row =>
  row.operation === 'effect' && row.request === REQUEST && row.response === RESPONSE);
const operations = trace => trace.map(row => `${row.operation}:${row.slotId}`);

const install = async (session, id, source, inputs, selected) => expectOutcome(
  await session.issue({ operation: 'install', id, source, inputs,
    ...(selected === undefined ? {} : { selected_name: selected }) }), 'installed');

// One pair: `callerProgram` is published into slot A, `leafProgram` into slot E, and one root
// dispatches into A, which dispatches into E.
const pair = async (binary, name, callerProgram, leafProgram) => {
  const session = await cli(binary, name, selectedAuthority({
    sources: authoritySources, slots, grants, effects: ['live.dispatch', 'test.emit'], quota: 16,
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
    const bootstrapped = expectOutcome(await session.issue({ operation: 'bootstrap',
      expected_epoch: '0', members: [{ slot: 'E', id: leafProgram }, { slot: 'A', id: callerProgram }] }),
    'bootstrapped');
    const rootSite = expectOutcome(await session.issue({ operation: 'reflect', id: 'root',
      site_id: 0 }), 'reflected').site;
    const root = expectOutcome(await session.issue({
      operation: 'invoke', id: 'root', inputs: [{ kind: 'i64', value: '20' }],
      refs: [{ position: 1, ordinal: 0, slot: 'E' }, { position: 2, ordinal: 1, slot: 'A' }],
    }), 'executed');
    const exit = await session.close();
    return { name, epoch: bootstrapped.epoch, rootSite, exit,
      rootEpoch: root.epoch, stack: root.stack, guestRequests: root.guest_requests,
      operations: operations(root.request_trace),
      effects: distinguished(root.request_trace).map(row => ({
        slotId: row.policyInput.slotId, programValueId: row.programValueId,
        definitionId: row.definitionId, request: row.request, response: row.response,
        rootEpoch: row.policyInput.rootEpoch, allowed: row.policyAllowed,
      })),
      dispatches: root.request_trace.filter(row => row.operation === 'dispatch')
        .map(row => ({ slotId: row.slotId, programValueId: row.programValueId })),
    };
  } catch (error) {
    await session.abort();
    throw error;
  }
};

export const run = async binary => ({
  old: await pair(binary, 'reloc-caller-emits', 'callerEmits', 'leafInert'),
  next: await pair(binary, 'reloc-callee-emits', 'callerDelegates', 'leafEmitting'),
  omission: await pair(binary, 'reloc-both-inert', 'callerDelegates', 'leafInert'),
});

const verify = ({ old, next, omission }) => {
  for (const [name, row, expected] of [
    ['the caller-emitting version', old, ['dispatch:A', 'effect:A', 'dispatch:E']],
    ['the callee-emitting version', next, ['dispatch:A', 'dispatch:E', 'effect:E']],
  ]) {
    assert.deepEqual(row.operations, expected, `${name} must perform its dispatches in this order`);
    assert.equal(row.effects.length, 1, `${name} must perform the distinguished event exactly once`);
    assert.equal(row.effects[0].request, REQUEST);
    assert.equal(row.effects[0].response, RESPONSE);
    assert.equal(row.effects[0].allowed, true);
    assert.equal(row.effects[0].rootEpoch, row.rootEpoch, `${name} performs it on its pinned epoch`);
    // The effect belongs to the frame that selected it: the caller's own version in the old pair,
    // the callee's version in the new one.
    const performingSlot = expected[1].split(':')[1];
    assert.equal(row.effects[0].slotId, performingSlot);
    const selector = row.dispatches.find(dispatch => dispatch.slotId === performingSlot);
    assert.equal(row.effects[0].programValueId, selector.programValueId,
      `${name} must perform the effect on behalf of the program its frame selected`);
    assert.equal(row.guestRequests, 3, `${name} issues one request per protected import`);
    assert.equal(row.exit, 0);
  }
  // Interface compatibility: same slot contracts, same reflected root site, same observed result.
  assert.deepEqual(old.rootSite, next.rootSite, 'both versions keep the same checked caller site');
  assert.deepEqual(old.stack, next.stack, 'both versions observe the same stack result');
  // Relocation: the event moves out of the caller's version and into the callee's.
  const oldDispatch = old.dispatches.find(dispatch => dispatch.slotId === 'A');
  const nextDispatch = next.dispatches.find(dispatch => dispatch.slotId === 'A');
  assert.notEqual(old.effects[0].programValueId, next.effects[0].programValueId,
    'the emitting program must change between the two versions');
  assert.notEqual(oldDispatch.programValueId, nextDispatch.programValueId,
    'the caller slot must hold a different program in each version');
  // Omission control: both versions inert keeps both dispatches and performs nothing.
  assert.deepEqual(omission.operations, ['dispatch:A', 'dispatch:E']);
  assert.deepEqual(omission.effects, []);
  assert.equal(omission.exit, 0);
};

if (process.argv[1]?.endsWith('live_slot_caller_emits.mjs') === true) {
  const measured = await run(process.argv[2]);
  process.stdout.write(`${JSON.stringify(measured)}\n`);
  verify(measured);
}
