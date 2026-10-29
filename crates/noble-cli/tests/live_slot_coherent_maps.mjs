#!/usr/bin/env node
/**
 * VC-LIVE-03 coherence witness: a root pinned on a coherent entry map performs exactly one
 * dispatch and exactly one effect, both rows report the same pinned map epoch, and the order in
 * which the two publications land before the root does not change that.
 *
 * Every scenario asserts a *count* plus the pinned map epoch of both rows, rather than a shape.
 * Retention: a root held before its first selection keeps the map it was pinned on while a
 * compare-and-set publication of a new emitting version lands, so it dispatches to and emits from
 * the old version exactly once, and the next root uses the new version exactly once. A
 * publication that omits the incarnation/generation compare-and-set fields stale-rejects with no
 * root pinned at all, so such a refusal says nothing about retention.
 */
import assert from 'node:assert/strict';
import { cli, selectedAuthority } from './live_slot_resource_ref_cli.mjs';

const QUOTE = String.fromCharCode(34);
const emitDefinition = name => `def ${name} [ drop ${QUOTE}relocated${QUOTE} test.emit drop 21 ]`;
const REF = 'LiveRef<I64,I64,live.dispatch+test.emit>';
const CEILING = ['live.dispatch', 'test.emit'];
const REQUEST = 'relocated';
const RESPONSE = 'emitted:relocated';

const sources = {
  defTracedE: emitDefinition('tracedE'),
  tracedE: 'tracedE',
  defTracedE2: emitDefinition('tracedE2'),
  tracedE2: 'tracedE2',
  defInert: 'def inertBody [ drop 21 ]',
  inert: 'inertBody',
  caller: 'slot.invoke',
};

const slots = [
  { slotId: 'B', input: ['I64', REF], output: ['I64'], effectCeiling: CEILING, proofRequired: false },
  { slotId: 'E', input: ['I64'], output: ['I64'], effectCeiling: CEILING, proofRequired: false },
];

const grants = slots.flatMap(slot =>
  ['publish', 'dispatch', 'effect'].map(operation => ({ operation, slotId: slot.slotId, allowed: true })));

const dispatched = trace => trace.filter(row => row.operation === 'dispatch');
const distinguished = trace => trace.filter(row =>
  row.operation === 'effect' && row.request === REQUEST && row.response === RESPONSE);

const expectRow = async (promise, outcome) => {
  const row = await promise;
  assert.equal(row.outcome, outcome, JSON.stringify(row).slice(0, 800));
  return row;
};

// Both publications land before the root, in either order: caller-then-leaf or leaf-then-caller.
const coherentRoot = async (binary, variant, callerFirst) => {
  const session = await cli(binary, variant, selectedAuthority({
    sources, slots, grants, effects: ['live.dispatch', 'test.emit'], quota: 16,
  }));
  await expectRow(session.configured, 'configured');
  for (const [id, name, source] of [
    ['defTracedE', 'tracedE', sources.defTracedE],
    ['defInert', 'inertBody', sources.defInert],
  ]) {
    await expectRow(session.issue({ operation: 'define', id, name, source, inputs: [] }),
      'definition-retained');
  }
  await expectRow(session.issue({
    operation: 'install', id: 'tracedE', source: sources.tracedE, inputs: ['I64'], selected_name: 'tracedE',
  }), 'installed');
  await expectRow(session.issue({
    operation: 'install', id: 'caller', source: sources.caller, inputs: ['I64', REF],
  }), 'installed');
  await expectRow(session.issue({ operation: 'candidate', id: 'caller' }), 'candidate-staged');

  const publishCaller = async expectedEpoch => expectRow(session.issue({
    operation: 'publish', slot: 'B', id: 'caller', program_index: 0, expected_epoch: expectedEpoch,
  }), 'published');
  const publishLeaf = async expectedEpoch => expectRow(session.issue({
    operation: 'publish', slot: 'E', id: 'tracedE', program_index: 0, expected_epoch: expectedEpoch,
  }), 'published');

  if (callerFirst) {
    await publishLeaf((await publishCaller('0')).epoch);
  } else {
    await publishCaller((await publishLeaf('0')).epoch);
  }

  const root = await expectRow(session.issue({
    operation: 'invoke', id: 'caller', inputs: [{ kind: 'i64', value: '5' }],
    refs: [{ position: 1, ordinal: 0, slot: 'E' }],
  }), 'executed');
  return { session, root };
};

export const run = async binary => {
  // Coherent map, leaf published first.
  const { session: leafFirstSession, root: leafFirstRoot } =
    await coherentRoot(binary, 'coherent-leaf-first', false);
  await leafFirstSession.close();
  // Coherent map, caller published first.
  const { session: callerFirstSession, root: callerFirstRoot } =
    await coherentRoot(binary, 'coherent-caller-first', true);

  const counts = root => ({
    dispatches: dispatched(root.request_trace).length,
    effects: distinguished(root.request_trace).length,
  });
  assert.deepEqual(counts(leafFirstRoot), { dispatches: 1, effects: 1 },
    'a coherent map must dispatch once and perform the effect exactly once');
  assert.deepEqual(counts(callerFirstRoot), { dispatches: 1, effects: 1 },
    'the publication order before the root must not change the counts');

  // Coherence: both rows come from the same pinned map epoch, and the dispatch row names the
  // same checked definition as the effect row.
  for (const root of [leafFirstRoot, callerFirstRoot]) {
    const dispatchRow = dispatched(root.request_trace)[0];
    const effectRow = distinguished(root.request_trace)[0];
    assert.equal(effectRow.policyInput.rootEpoch, dispatchRow.policyInput.rootEpoch,
      'both rows of one root must report the same pinned map epoch');
    assert.equal(effectRow.definitionId, dispatchRow.definitionId,
      'the effect row must name the definition the dispatch selected');
    assert.equal(effectRow.policyInput.slotId, 'E');
    assert.equal(effectRow.request, REQUEST);
    assert.equal(effectRow.response, RESPONSE);
  }

  // Retention: hold a root before its first selection, land a compare-and-set publication of a new
  // emitting version of E, then resume. The held root must dispatch to and emit from the version it
  // was pinned on exactly once; the next root must use the new version exactly once.
  const invokeCaller = { operation: 'invoke', id: 'caller', inputs: [{ kind: 'i64', value: '5' }],
    refs: [{ position: 1, ordinal: 0, slot: 'E' }] };
  const oldEffect = distinguished(callerFirstRoot.request_trace)[0];
  const oldDispatch = dispatched(callerFirstRoot.request_trace)[0];
  const pinnedEpoch = oldEffect.policyInput.rootEpoch;
  assert.equal(typeof oldDispatch.incarnation, 'string', 'the dispatch row names the selected incarnation');
  assert.equal(typeof oldDispatch.generation, 'string', 'the dispatch row names the selected generation');
  await expectRow(callerFirstSession.issue({
    operation: 'define', id: 'defTracedE2', name: 'tracedE2', source: sources.defTracedE2, inputs: [],
  }), 'definition-retained');
  await expectRow(callerFirstSession.issue({
    operation: 'install', id: 'tracedE2', source: sources.tracedE2, inputs: ['I64'], selected_name: 'tracedE2',
  }), 'installed');
  // A publication issued while a root is held needs a candidate staged while idle under E's exact
  // contract; program 0 is the one published into E above.
  await expectRow(callerFirstSession.issue({ operation: 'candidate', id: 'tracedE2', program_index: 0 }),
    'candidate-staged');
  await expectRow(callerFirstSession.issue({
    operation: 'hold-checkpoint', import: 'live_select', occurrence: 1,
  }), 'checkpoint-hold-selected');
  callerFirstSession.send(invokeCaller);
  const entered = await expectRow(callerFirstSession.next(), 'checkpoint-entered');
  callerFirstSession.send({
    operation: 'publish', slot: 'E', id: 'tracedE2', program_index: 0,
    expected_epoch: pinnedEpoch, expected_incarnation: oldDispatch.incarnation,
    expected_generation: oldDispatch.generation,
  });
  // Queue the resume at once: the checkpoint has a deadline, and the router applies the ordered
  // publication and acknowledges it before resuming the guest.
  callerFirstSession.send({ operation: 'resume-checkpoint' });
  const heldPublication = await expectRow(callerFirstSession.next(), 'published');
  assert.equal(heldPublication.root, entered.root, 'the publication must land while this root is held');
  assert.equal(heldPublication.checkpoint, entered.checkpoint);
  assert.notEqual(heldPublication.epoch, pinnedEpoch, 'the publication must advance the registry epoch');
  const heldRoot = await expectRow(callerFirstSession.next(), 'executed');
  assert.deepEqual(counts(heldRoot), { dispatches: 1, effects: 1 },
    'the held root must dispatch once and emit exactly once across the publication');
  for (const row of [...dispatched(heldRoot.request_trace), ...distinguished(heldRoot.request_trace)]) {
    assert.equal(row.policyInput.rootEpoch, pinnedEpoch, 'the held root keeps the map it was pinned on');
    assert.equal(row.definitionId, oldEffect.definitionId,
      'the held root must dispatch to and emit from the version it was pinned on');
  }
  assert.deepEqual(heldRoot.control_events.map(event =>
    [event.operation, event.scope, event.epoch, event.outcome]),
  [['publish', 'E', heldPublication.epoch, 'published']],
  'the held root records the publication that landed during it');
  const freshRoot = await expectRow(callerFirstSession.issue(invokeCaller), 'executed');
  assert.deepEqual(counts(freshRoot), { dispatches: 1, effects: 1 },
    'the next root must dispatch once and emit exactly once');
  const newEffect = distinguished(freshRoot.request_trace)[0];
  for (const row of [...dispatched(freshRoot.request_trace), newEffect]) {
    assert.equal(row.policyInput.rootEpoch, heldPublication.epoch, 'the next root is pinned on the new map');
    assert.equal(row.definitionId, newEffect.definitionId);
  }
  assert.notEqual(newEffect.definitionId, oldEffect.definitionId, 'the next root must use the new version');
  // Control: the request shape without the incarnation/generation compare-and-set fields
  // stale-rejects while no root is pinned, so that refusal cannot witness retention.
  await expectRow(callerFirstSession.issue({ operation: 'candidate', id: 'tracedE2', program_index: 0 }),
    'candidate-staged');
  const withoutCompareAndSet = await expectRow(callerFirstSession.issue({
    operation: 'publish', slot: 'E', id: 'tracedE2', program_index: 0, expected_epoch: heldPublication.epoch,
  }), 'stale-reject');
  await callerFirstSession.close();

  // Sequential-publication control: a root attempted between the two publications cannot execute
  // at all because the slot its reference names holds no published program yet, so the mixed pair
  // is the limit of the coherence premise rather than a root-snapshot defect. After the second
  // publication lands the same root executes coherently on one pinned map.
  const sequentialSession = await cli(binary, 'coherent-sequential', selectedAuthority({
    sources, slots, grants, effects: ['live.dispatch', 'test.emit'], quota: 16,
  }));
  await expectRow(sequentialSession.configured, 'configured');
  await expectRow(sequentialSession.issue({
    operation: 'define', id: 'defTracedE', name: 'tracedE', source: sources.defTracedE, inputs: [],
  }), 'definition-retained');
  await expectRow(sequentialSession.issue({
    operation: 'install', id: 'tracedE', source: sources.tracedE, inputs: ['I64'], selected_name: 'tracedE',
  }), 'installed');
  await expectRow(sequentialSession.issue({
    operation: 'install', id: 'caller', source: sources.caller, inputs: ['I64', REF],
  }), 'installed');
  await expectRow(sequentialSession.issue({ operation: 'candidate', id: 'caller' }), 'candidate-staged');
  await expectRow(sequentialSession.issue({
    operation: 'publish', slot: 'B', id: 'caller', program_index: 0, expected_epoch: '0',
  }), 'published');
  const halfPublished = await sequentialSession.issue({
    operation: 'invoke', id: 'caller', inputs: [{ kind: 'i64', value: '5' }],
    refs: [{ position: 1, ordinal: 0, slot: 'E' }],
  });
  assert.equal(halfPublished.outcome, 'execution-failed', JSON.stringify(halfPublished).slice(0, 800));
  assert.equal(halfPublished.diagnostic, 'Error: root reference has no pinned published slot');
  assert.equal((halfPublished.request_trace ?? []).length, 0,
    'a root attempted between the two publications must produce no trace rows');
  assert.equal(halfPublished.session_state, 'terminated-after-guest-failure',
    'the half-published root terminates the session rather than mixing maps');
  await sequentialSession.close();

  // Omission control: a dispatch to a slot that does not emit keeps the dispatch and produces
  // no effect row, so the count assertion above distinguishes omission from coherence.
  const omissionSession = await cli(binary, 'coherent-omission', selectedAuthority({
    sources, slots, grants, effects: ['live.dispatch', 'test.emit'], quota: 16,
  }));
  await expectRow(omissionSession.configured, 'configured');
  await expectRow(omissionSession.issue({
    operation: 'define', id: 'defInert', name: 'inertBody', source: sources.defInert, inputs: [],
  }), 'definition-retained');
  await expectRow(omissionSession.issue({
    operation: 'install', id: 'inert', source: sources.inert, inputs: ['I64'], selected_name: 'inertBody',
  }), 'installed');
  await expectRow(omissionSession.issue({
    operation: 'install', id: 'caller', source: sources.caller, inputs: ['I64', REF],
  }), 'installed');
  await expectRow(omissionSession.issue({ operation: 'candidate', id: 'caller' }), 'candidate-staged');
  const inertEpoch = (await expectRow(omissionSession.issue({
    operation: 'publish', slot: 'E', id: 'inert', program_index: 0, expected_epoch: '0',
  }), 'published')).epoch;
  await expectRow(omissionSession.issue({
    operation: 'publish', slot: 'B', id: 'caller', program_index: 0, expected_epoch: inertEpoch,
  }), 'published');
  const omissionRoot = await expectRow(omissionSession.issue({
    operation: 'invoke', id: 'caller', inputs: [{ kind: 'i64', value: '5' }],
    refs: [{ position: 1, ordinal: 0, slot: 'E' }],
  }), 'executed');
  assert.deepEqual(counts(omissionRoot), { dispatches: 1, effects: 0 },
    'an omission keeps the dispatch and must show zero effect rows');
  await omissionSession.close();

  return {
    leafFirstCounts: counts(leafFirstRoot),
    callerFirstCounts: counts(callerFirstRoot),
    pinnedEpoch,
    heldPublicationOutcome: heldPublication.outcome,
    publishedEpoch: heldPublication.epoch,
    heldRootCounts: counts(heldRoot),
    heldRootDefinition: oldEffect.definitionId,
    freshRootCounts: counts(freshRoot),
    freshRootDefinition: newEffect.definitionId,
    withoutCompareAndSetOutcome: withoutCompareAndSet.outcome,
    halfPublishedOutcome: halfPublished.outcome,
    halfPublishedRows: (halfPublished.request_trace ?? []).length,
    halfPublishedSessionState: halfPublished.session_state,
    omissionCounts: counts(omissionRoot),
  };
};

if (process.argv[1]?.endsWith('live_slot_coherent_maps.mjs') === true) {
  const witness = await run(process.argv[2]);
  process.stdout.write(`${JSON.stringify(witness)}\n`);
}
