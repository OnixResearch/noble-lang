#!/usr/bin/env node
/**
 * VC-LIVE-03 two-selection control: one root selects the same leaf slot twice, the runtime holds
 * it at the second `live_select`, and a compare-and-set republication of that leaf commits between
 * the root's two selections. On the pinned runtime both selections come from the root's own map,
 * so the root performs exactly the events its *pinned* leaf version performs — twice for an old
 * version that emits, zero times for an old version that does not — no matter what the new version
 * does. A runtime that resolved each dispatch from the registry's current map instead would mix
 * the two versions inside one root and perform exactly one event either way: an omission when the
 * pinned version emitted, an extra event when only the new version does.
 *
 * The lane measures both publication directions (emitting old / inert old) so that the omitted and
 * the extra event are both observable, and asserts counts plus the definition each selection
 * names rather than a trace shape.
 *
 * Usage: node live_slot_two_selections.mjs ABSOLUTE_PREBUILT_CLI [emitting-old|inert-old]
 * The optional third argument restricts the run to one publication direction so an out-of-tree
 * mutant can be measured per direction; without it both directions run.
 */
import assert from 'node:assert/strict';
import { cli, expectOutcome, selectedAuthority } from './live_slot_resource_ref_cli.mjs';

const QUOTE = String.fromCharCode(34);
const emitDefinition = name => `def ${name} [ drop ${QUOTE}relocated${QUOTE} test.emit drop 21 ]`;
const LEAF_REF = 'LiveRef<I64,I64,test.emit>';
const DISPATCHER_REF = `LiveRef<I64+${LEAF_REF},I64,live.dispatch+test.emit>`;
const REQUEST = 'relocated';
const RESPONSE = 'emitted:relocated';

const sources = {
  defTraced: emitDefinition('tracedLeaf'),
  defTracedNext: emitDefinition('tracedLeafNext'),
  defInert: 'def inertLeaf [ drop 21 ]',
  dispatcher: 'slot.invoke slot.invoke',
  caller: 'slot.invoke',
};

// An `install` selects a retained declaration by name, so its source text is that name; the
// authority binds one digest per install identity (and per define identity) independently.
const authoritySources = (oldDefinition, nextDefinition) => ({
  defTraced: sources.defTraced,
  defTracedNext: sources.defTracedNext,
  defInert: sources.defInert,
  leafOld: oldDefinition,
  leafNext: nextDefinition,
  dispatcher: sources.dispatcher,
  caller: sources.caller,
});

const slots = [
  { slotId: 'E', input: ['I64'], output: ['I64'], effectCeiling: ['test.emit'], proofRequired: false },
  { slotId: 'A', input: ['I64', LEAF_REF], output: ['I64'],
    effectCeiling: ['live.dispatch', 'test.emit'], proofRequired: false },
];
const grants = slots.flatMap(slot =>
  (slot.slotId === 'E' ? ['publish', 'dispatch', 'effect'] : ['publish', 'dispatch'])
    .map(operation => ({ operation, slotId: slot.slotId, allowed: true })));

const leafSelections = trace => trace.filter(row =>
  row.operation === 'dispatch' && row.slotId === 'E');
const distinguished = trace => trace.filter(row =>
  row.operation === 'effect' && row.request === REQUEST && row.response === RESPONSE);
const unique = values => [...new Set(values)];
const counts = trace => ({
  leafSelections: leafSelections(trace).length,
  effects: distinguished(trace).length,
});
const rowEpochs = trace => unique(trace.map(row => row.policyInput?.rootEpoch));

const invoke = {
  operation: 'invoke', id: 'caller', inputs: [{ kind: 'i64', value: '20' }],
  refs: [{ position: 1, ordinal: 0, slot: 'E' }, { position: 2, ordinal: 1, slot: 'A' }],
};

const install = async (session, id, source, inputs, selected) => expectOutcome(
  await session.issue({ operation: 'install', id, source, inputs,
    ...(selected === undefined ? {} : { selected_name: selected }) }), 'installed');

// One publication direction: `oldKind` is the leaf version the root pins, `nextKind` the version
// the operator commits while the root is held at its second selection.
const measure = async (binary, name, oldDefinition, nextDefinition, heldEffects, nextEffects) => {
  const session = await cli(binary, `two-selections-${name}`, selectedAuthority({
    sources: authoritySources(oldDefinition, nextDefinition),
    slots, grants, effects: ['live.dispatch', 'test.emit'], quota: 16,
  }));
  try {
    expectOutcome(await session.configured, 'configured');
    for (const [id, definition, source] of [
      ['defTraced', 'tracedLeaf', sources.defTraced],
      ['defTracedNext', 'tracedLeafNext', sources.defTracedNext],
      ['defInert', 'inertLeaf', sources.defInert],
    ]) expectOutcome(await session.issue({ operation: 'define', id, name: definition, source, inputs: [] }),
      'definition-retained');
    await install(session, 'leafOld', oldDefinition, ['I64'], oldDefinition);
    await install(session, 'leafNext', nextDefinition, ['I64'], nextDefinition);
    await install(session, 'dispatcher', sources.dispatcher, ['I64', LEAF_REF]);
    await install(session, 'caller', sources.caller, ['I64', LEAF_REF, DISPATCHER_REF]);
    // The replacement must be staged while the slot is idle under its exact contract, before the
    // root is pinned; staging it during the hold expires the eight-second checkpoint deadline.
    // The idle-staged candidate must name the exact Program index the publication will select.
    expectOutcome(await session.issue({ operation: 'candidate', id: 'leafNext', program_index: 0 }),
      'candidate-staged');
    const bootstrapped = expectOutcome(await session.issue({ operation: 'bootstrap',
      expected_epoch: '0', members: [{ slot: 'E', id: 'leafOld' }, { slot: 'A', id: 'dispatcher' }] }),
    'bootstrapped');
    const pinnedEpoch = bootstrapped.epoch;
    expectOutcome(await session.issue({ operation: 'hold-checkpoint', import: 'live_select',
      occurrence: 2 }), 'checkpoint-hold-selected');
    session.send(invoke);
    const entered = expectOutcome(await session.next(), 'checkpoint-entered');
    // Queue the publication and the resume at once: the router applies the ordered publication and
    // acknowledges it before resuming the guest, independently of operator stdout latency.
    session.send({ operation: 'publish', slot: 'E', id: 'leafNext', program_index: 0,
      expected_epoch: pinnedEpoch, expected_incarnation: '1', expected_generation: '1' });
    session.send({ operation: 'resume-checkpoint' });
    const committed = expectOutcome(await session.next(), 'published');
    const held = expectOutcome(await session.next(), 'executed');
    const next = expectOutcome(await session.issue(invoke), 'executed');
    const exit = await session.close();
    return {
      variant: name, pinnedDefinition: oldDefinition, nextDefinition,
      pinnedEpoch, publicationEpoch: committed.epoch,
      entered: { import: entered.import, ordinal: entered.ordinal, site: entered.site },
      committed: { epoch: committed.epoch, root: committed.root === entered.root,
        checkpoint: committed.checkpoint === entered.checkpoint, controlId: committed.control_id },
      heldEpoch: held.epoch,
      heldCounts: counts(held.request_trace),
      heldPrograms: unique(leafSelections(held.request_trace).map(row => row.programValueId)),
      heldDefinitions: unique(leafSelections(held.request_trace).map(row => row.definitionId)),
      heldEffects: unique(distinguished(held.request_trace).map(row => row.policyInput?.slotId)),
      heldRowEpochs: rowEpochs(held.request_trace),
      heldSlotOrder: held.request_trace.filter(row => row.operation === 'dispatch')
        .map(row => row.slotId),
      heldControls: held.control_events.map(event =>
        [event.control_id, event.operation, event.scope, event.epoch, event.outcome]),
      nextEpoch: next.epoch,
      nextCounts: counts(next.request_trace),
      nextPrograms: unique(leafSelections(next.request_trace).map(row => row.programValueId)),
      nextDefinitions: unique(leafSelections(next.request_trace).map(row => row.definitionId)),
      nextEffects: unique(distinguished(next.request_trace).map(row => row.policyInput?.slotId)),
      nextRowEpochs: rowEpochs(next.request_trace),
      expectedHeldEffects: heldEffects,
      expectedNextEffects: nextEffects,
      exit,
    };
  } catch (error) {
    await session.abort();
    throw error;
  }
};

const verify = measured => {
  const { pinnedEpoch, publicationEpoch, heldCounts, nextCounts } = measured;
  assert.equal(measured.entered.import, 'live_select');
  assert.equal(measured.entered.ordinal, 0, 'the held selection is the leaf reference, not the caller');
  assert.equal(measured.committed.epoch !== pinnedEpoch, true,
    'the publication must advance the registry epoch while the root is held');
  assert.equal(measured.committed.root, true, 'the publication must land while this root is held');
  assert.equal(measured.committed.checkpoint, true);

  // Coherent pinned map: two leaf selections, and the held root performs exactly the events its
  // pinned version performs - twice when it emits, zero times when it is inert.
  assert.equal(heldCounts.leafSelections, 2,
    'one root must select the same leaf slot twice');
  assert.equal(measured.heldPrograms.length, 1,
    'both selections of one root must name the same pinned Program');
  assert.deepEqual(measured.heldEffects, heldCounts.effects === 0 ? []
    : ['E'], 'every effect row must name the selected leaf slot');
  assert.equal(heldCounts.effects, measured.expectedHeldEffects,
    `the held root must perform the pinned version's event exactly ${measured.expectedHeldEffects} times`);
  assert.equal(measured.heldEpoch, pinnedEpoch, 'the held root keeps the map epoch it pinned');
  assert.deepEqual(measured.heldRowEpochs, [pinnedEpoch],
    'every trace row of the held root must report the pinned epoch');
  assert.deepEqual(measured.heldSlotOrder, ['A', 'E', 'E'],
    'the root dispatches to the dispatcher and then to the leaf twice');
  assert.deepEqual(measured.heldControls, [['1', 'publish', 'E', publicationEpoch, 'published']],
    'the held root records the publication that landed during it');

  // The next root is pinned on the new map and performs exactly the new version's events.
  assert.equal(nextCounts.leafSelections, 2);
  assert.equal(measured.nextPrograms.length, 1);
  assert.notEqual(measured.nextPrograms[0], measured.heldPrograms[0],
    'the next root must select the republished version');
  assert.equal(nextCounts.effects, measured.expectedNextEffects,
    `the next root must perform the new version's event exactly ${measured.expectedNextEffects} times`);
  assert.equal(measured.nextEpoch, publicationEpoch, 'the next root pins the published epoch');
  assert.deepEqual(measured.nextRowEpochs, [publicationEpoch],
    'every trace row of the next root must report its own pinned epoch');
  assert.equal(measured.exit, 0, 'the session must close cleanly');
};

export const run = async (binary, only = null) => {
  const directions = [
    // name, pinned definition, republished definition, pinned-version events, new-version events
    ['emitting-old', 'tracedLeaf', 'inertLeaf', 2, 0],
    ['inert-old', 'inertLeaf', 'tracedLeafNext', 0, 2],
  ].filter(([name]) => only === null || only === name);
  assert.ok(directions.length, `unknown publication direction ${only}`);
  const measured = [];
  for (const [name, oldDefinition, nextDefinition, heldEffects, nextEffects] of directions) {
    measured.push(await measure(binary, name, oldDefinition, nextDefinition, heldEffects,
      nextEffects));
  }
  return { measured, summary: { variants: measured } };
};

if (process.argv[1]?.endsWith('live_slot_two_selections.mjs') === true) {
  const { measured, summary } = await run(process.argv[2], process.argv[3] ?? null);
  // Report the measured counts before asserting, so an out-of-tree mutant's own numbers are visible
  // even though the lane fails.
  process.stdout.write(`${JSON.stringify(summary)}\n`);
  for (const row of measured) verify(row);
}
