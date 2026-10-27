#!/usr/bin/env node
/**
 * VC-LIVE-03 coherence witness: a root pinned on a coherent entry map performs exactly one
 * dispatch and exactly one effect, both rows report the same pinned map epoch, and the order in
 * which the two publications land before the root does not change that.
 *
 * Every scenario asserts a *count* plus the pinned map epoch of both rows, rather than a shape.
 * An already-pinned root retains its map: a publication that tries to land after the pin is
 * refused, and a fresh root afterwards still reports one coherent pair, never a duplicate.
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

  // Retention: with a root already pinned, the publication that tries to land afterwards is
  // refused, and a fresh root still sees one coherent pair rather than a duplicate.
  await expectRow(callerFirstSession.issue({ operation: 'candidate', id: 'tracedE' }),
    'candidate-staged');
  const pinnedEpoch = distinguished(callerFirstRoot.request_trace)[0].policyInput.rootEpoch;
  const refusedPublication = await callerFirstSession.issue({
    operation: 'publish', slot: 'E', id: 'tracedE', program_index: 0, expected_epoch: pinnedEpoch,
  });
  assert.notEqual(refusedPublication.outcome, 'published',
    'a publication must not land into the map of an already-pinned root');
  const freshRoot = await expectRow(callerFirstSession.issue({
    operation: 'invoke', id: 'caller', inputs: [{ kind: 'i64', value: '5' }],
    refs: [{ position: 1, ordinal: 0, slot: 'E' }],
  }), 'executed');
  assert.deepEqual(counts(freshRoot), { dispatches: 1, effects: 1 },
    'the refused publication must not duplicate or omit the effect');
  assert.equal(dispatched(freshRoot.request_trace)[0].policyInput.rootEpoch, pinnedEpoch,
    'the retained map keeps the epoch the earlier root was pinned on');
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
    refusedPublication: refusedPublication.outcome,
    freshRootCounts: counts(freshRoot),
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
