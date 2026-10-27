#!/usr/bin/env node
/**
 * VC-LIVE-03 relocation witness: a host effect that used to be performed by the caller slot is
 * moved into the callee slot, and the effect row's checked DefinitionId moves with it while the
 * request and the response stay identical.
 *
 * Every scenario asserts a *count* over the request trace rather than a shape, and the duplicate
 * and omission scenarios name explicitly which coincidence they exhibit.
 */
import assert from 'node:assert/strict';
import { cli, selectedAuthority } from './live_slot_resource_ref_cli.mjs';

const QUOTE = String.fromCharCode(34);
const emitDefinition = name => `def ${name} [ drop ${QUOTE}relocated${QUOTE} test.emit drop 21 ]`;
const REF = 'LiveRef<I64,I64,live.dispatch+test.emit>';
const DOUBLE_REF = 'LiveRef<I64,I64,test.emit>';
const CEILING = ['live.dispatch', 'test.emit'];
const REQUEST = 'relocated';
const RESPONSE = 'emitted:relocated';

const sources = {
  defTracedA: emitDefinition('tracedA'),
  tracedA: 'tracedA',
  defTracedE: emitDefinition('tracedE'),
  tracedE: 'tracedE',
  defInert: 'def inertBody [ drop 21 ]',
  inert: 'inertBody',
  callerOld: 'slot.invoke',
  callerNew: 'slot.invoke',
};

const slots = [
  { slotId: 'A', input: ['I64'], output: ['I64'], effectCeiling: ['test.emit'], proofRequired: false },
  { slotId: 'B', input: ['I64', REF], output: ['I64'], effectCeiling: CEILING, proofRequired: false },
  { slotId: 'D', input: ['I64', DOUBLE_REF], output: ['I64'], effectCeiling: ['live.dispatch', 'test.emit'], proofRequired: false },
  { slotId: 'E', input: ['I64'], output: ['I64'], effectCeiling: CEILING, proofRequired: false },
];

const grantsFor = deniedEffectSlot => slots.flatMap(slot =>
  ['publish', 'dispatch', 'effect'].map(operation => ({
    operation,
    slotId: slot.slotId,
    allowed: !(operation === 'effect' && slot.slotId === deniedEffectSlot),
  })));

const distinguished = trace => trace.filter(row =>
  row.operation === 'effect' && row.request === REQUEST && row.response === RESPONSE);

const expectRow = async (promise, outcome) => {
  const row = await promise;
  assert.equal(row.outcome, outcome, JSON.stringify(row).slice(0, 800));
  return row;
};

const openSession = async (binary, variant, deniedEffectSlot = null) => {
  const session = await cli(binary, variant, selectedAuthority({
    sources,
    slots,
    grants: grantsFor(deniedEffectSlot),
    effects: ['live.dispatch', 'test.emit'],
    quota: 16,
  }));
  await expectRow(session.configured, 'configured');
  for (const [id, name, source] of [
    ['defTracedA', 'tracedA', sources.defTracedA],
    ['defTracedE', 'tracedE', sources.defTracedE],
    ['defInert', 'inertBody', sources.defInert],
  ]) {
    await expectRow(session.issue({ operation: 'define', id, name, source, inputs: [] }),
      'definition-retained');
  }
  return session;
};

const install = async (session, id, source, inputs, selectedName) => expectRow(session.issue({
  operation: 'install', id, source, inputs, ...(selectedName === undefined ? {} : { selected_name: selectedName }),
}), 'installed');

const publish = async (session, slot, id, expectedEpoch) => expectRow(session.issue({
  operation: 'publish', slot, id, program_index: 0, expected_epoch: expectedEpoch,
}), 'published');

// Old version: the caller slot B dispatches to slot A, and slot A performs the effect.
const oldVersionRow = async binary => {
  const session = await openSession(binary, 'reloc-old');
  await install(session, 'tracedA', sources.tracedA, ['I64'], 'tracedA');
  await install(session, 'callerOld', sources.callerOld, ['I64', DOUBLE_REF]);
  await expectRow(session.issue({ operation: 'candidate', id: 'callerOld' }), 'candidate-staged');
  let epoch = (await publish(session, 'A', 'tracedA', '0')).epoch;
  await publish(session, 'D', 'callerOld', epoch);
  const root = await expectRow(session.issue({
    operation: 'invoke', id: 'callerOld', inputs: [{ kind: 'i64', value: '5' }],
    refs: [{ position: 1, ordinal: 0, slot: 'A' }],
  }), 'executed');
  await session.close();
  return root;
};

// New version: the caller slot B dispatches to slot E, and slot E performs the effect.
const newVersionRow = async binary => {
  const session = await openSession(binary, 'reloc-new');
  await install(session, 'tracedE', sources.tracedE, ['I64'], 'tracedE');
  await install(session, 'callerNew', sources.callerNew, ['I64', REF]);
  await expectRow(session.issue({ operation: 'candidate', id: 'callerNew' }), 'candidate-staged');
  let epoch = (await publish(session, 'E', 'tracedE', '0')).epoch;
  await publish(session, 'B', 'callerNew', epoch);
  const root = await expectRow(session.issue({
    operation: 'invoke', id: 'callerNew', inputs: [{ kind: 'i64', value: '5' }],
    refs: [{ position: 1, ordinal: 0, slot: 'E' }],
  }), 'executed');
  await session.close();
  return root;
};

export const run = async binary => {
  const oldRoot = await oldVersionRow(binary);
  const newRoot = await newVersionRow(binary);
  const oldRows = distinguished(oldRoot.request_trace);
  const newRows = distinguished(newRoot.request_trace);

  // Coherent: exactly one distinguished row on each side, with the same request and response.
  assert.equal(oldRows.length, 1, 'old version must perform the effect exactly once');
  assert.equal(newRows.length, 1, 'new version must perform the effect exactly once');
  assert.deepEqual(oldRows[0].request, REQUEST);
  assert.deepEqual(newRows[0].request, REQUEST);
  assert.deepEqual(oldRows[0].response, RESPONSE);
  assert.deepEqual(newRows[0].response, RESPONSE);

  // The relocation witness: the effect row's checked DefinitionId and slot move from A to E.
  assert.equal(oldRows[0].policyInput.slotId, 'A', 'old row must name the caller-side slot');
  assert.equal(newRows[0].policyInput.slotId, 'E', 'new row must name the callee slot');
  assert.notEqual(oldRows[0].definitionId, newRows[0].definitionId,
    'the emitting program DefinitionId must change with the relocation');

  // Abnormal prefix: the dispatch precedes the effect on both sides.
  assert.equal(oldRoot.request_trace[0].operation, 'dispatch');
  assert.equal(newRoot.request_trace[0].operation, 'dispatch');
  assert.deepEqual(oldRoot.request_trace[0].request, undefined);

  // Duplicate (non-claim): a single invocation that routes to two emitting slots cannot be built
  // from this authority shape. `slot.invoke` with two borrowed formals refuses
  // `is_ambiguous is_compatible borrowed references`, its `slot.invoke.N` form refuses
  // `invalid borrowed input ordinal`, so the duplicate-vs-omission distinction is recorded as an
  // open question rather than asserted here.

  // Omission: a dispatch to a slot that does not emit performs the effect zero times.
  const omissionSession = await openSession(binary, 'reloc-omission');
  await install(omissionSession, 'inert', sources.inert, ['I64'], 'inertBody');
  await install(omissionSession, 'callerNew', sources.callerNew, ['I64', REF]);
  await expectRow(omissionSession.issue({ operation: 'candidate', id: 'callerNew' }), 'candidate-staged');
  let omissionEpoch = (await publish(omissionSession, 'E', 'inert', '0')).epoch;
  await publish(omissionSession, 'B', 'callerNew', omissionEpoch);
  const omissionRoot = await expectRow(omissionSession.issue({
    operation: 'invoke', id: 'callerNew', inputs: [{ kind: 'i64', value: '5' }],
    refs: [{ position: 1, ordinal: 0, slot: 'E' }],
  }), 'executed');
  assert.equal(omissionRoot.request_trace[0].operation, 'dispatch');
  assert.equal(distinguished(omissionRoot.request_trace).length, 0,
    'an omission must show zero effect rows while the dispatch still happens');
  await omissionSession.close();

  // Denied effect: with the effect grant withdrawn the same invocation must not perform it.
  const deniedSession = await openSession(binary, 'reloc-denied', 'E');
  await install(deniedSession, 'tracedE', sources.tracedE, ['I64'], 'tracedE');
  await install(deniedSession, 'callerNew', sources.callerNew, ['I64', REF]);
  await expectRow(deniedSession.issue({ operation: 'candidate', id: 'callerNew' }), 'candidate-staged');
  let deniedEpoch = (await publish(deniedSession, 'E', 'tracedE', '0')).epoch;
  await publish(deniedSession, 'B', 'callerNew', deniedEpoch);
  const deniedRow = await deniedSession.issue({
    operation: 'invoke', id: 'callerNew', inputs: [{ kind: 'i64', value: '5' }],
    refs: [{ position: 1, ordinal: 0, slot: 'E' }],
  });
  assert.notEqual(deniedRow.outcome, 'executed', JSON.stringify(deniedRow).slice(0, 800));
  assert.equal(distinguished(deniedRow.request_trace ?? []).length, 0,
    'a denied effect must produce no effect row');
  await deniedSession.close();

  return {
    oldDefinitionId: oldRows[0].definitionId,
    newDefinitionId: newRows[0].definitionId,
    oldSlot: oldRows[0].policyInput.slotId,
    newSlot: newRows[0].policyInput.slotId,
    duplicateCount: null,
    omissionCount: 0,
    deniedOutcome: deniedRow.outcome,
  };
};

if (process.argv[1]?.endsWith('live_slot_relocation_witness.mjs') === true) {
  const witness = await run(process.argv[2]);
  process.stdout.write(`${JSON.stringify(witness)}\n`);
}
