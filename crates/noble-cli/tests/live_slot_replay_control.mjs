import assert from 'node:assert/strict';
import { cli, selectedAuthority } from './live_slot_resource_ref_cli.mjs';

const binary = process.argv[2];
assert.ok(binary, 'pass the selected noble CLI binary');
const QUOTE = String.fromCharCode(34);
const definition = `def traced [ drop ${QUOTE}A${QUOTE} test.emit drop 21 ]`;
const effects = ['live.dispatch', 'test.emit'];
const slots = [
  { slotId: 'S1', input: ['I64'], output: ['I64'], effectCeiling: effects, proofRequired: false },
  { slotId: 'B', input: ['I64', 'LiveRef<I64,I64,live.dispatch+test.emit>'], output: ['I64'],
    effectCeiling: effects, proofRequired: false },
];
const grants = ['publish', 'dispatch', 'effect'].flatMap(operation =>
  slots.map(slot => ({ operation, slotId: slot.slotId, allowed: true })));
const session = await cli(binary, 'LSLOT-replay-control', selectedAuthority({
  sources: { definition, named: 'traced', caller: 'slot.invoke' }, slots, grants, effects, quota: 8,
}));
const expectRow = async (promise, outcome) => {
  const row = await promise;
  assert.equal(row.outcome, outcome, JSON.stringify(row).slice(0, 700));
  return row;
};
const refs = [{ position: 1, ordinal: 0, slot: 'S1' }];
try {
  await expectRow(session.configured, 'configured');
  await expectRow(session.issue({ operation: 'define', id: 'definition', name: 'traced',
    source: definition, inputs: [] }), 'definition-retained');
  await expectRow(session.issue({ operation: 'install', id: 'named', source: 'traced',
    inputs: ['I64'], selected_name: 'traced' }), 'installed');
  await expectRow(session.issue({ operation: 'candidate', id: 'named' }), 'candidate-staged');
  await expectRow(session.issue({ operation: 'install', id: 'caller', source: 'slot.invoke',
    inputs: ['I64', 'LiveRef<I64,I64,live.dispatch+test.emit>'] }), 'installed');
  await expectRow(session.issue({ operation: 'candidate', id: 'caller' }), 'candidate-staged');
  const leaf = await expectRow(session.issue({ operation: 'publish', slot: 'S1', id: 'named',
    program_index: 0, expected_epoch: '0' }), 'published');
  await expectRow(session.issue({ operation: 'publish', slot: 'B', id: 'caller',
    program_index: 0, expected_epoch: leaf.epoch }), 'published');
  const recorded = await expectRow(session.issue({ operation: 'invoke', id: 'caller',
    inputs: [{ kind: 'i64', value: '5' }], refs, record: true }), 'executed');
  assert.ok(recorded.replay_token, 'the recorded root must retain a frozen token');
  assert.deepEqual(recorded.request_trace.map(row => row.operation), ['dispatch', 'effect']);
  // Hold the first protected checkpoint of the replay so the operator change is applied inside
  // the replay rather than racing it.
  await expectRow(session.issue({ operation: 'hold-checkpoint', import: 'test_emit',
    occurrence: 1 }), 'checkpoint-hold-selected');
  session.send({ operation: 'replay', token: recorded.replay_token,
    inputs: [{ kind: 'i64', value: '5' }], refs,
    expected_trace: structuredClone(recorded.request_trace),
    expected_stack: structuredClone(recorded.stack),
    expected_identity: structuredClone(recorded.frozen_identity) });
  const entered = await expectRow(session.next(), 'checkpoint-entered');
  assert.equal(entered.import, 'test_emit');
  // The change is a revocation of the effect grant the recorded root used.
  session.send({ operation: 'policy', grant: { operation: 'effect', slotId: 'S1', allowed: false } });
  const committed = await expectRow(session.next(), 'policy-updated');
  assert.equal(committed.root, entered.root);
  assert.equal(committed.checkpoint, entered.checkpoint);
  session.send({ operation: 'resume-checkpoint' });
  const replay = await expectRow(session.next(), 'replay-matched');
  assert.deepEqual(replay.actual_trace, recorded.request_trace);
  assert.equal(replay.scripted_operations, 1);
  // The change committed during the replay keeps its receipt: the host reads it from the replay
  // reply, so the session survives and keeps answering.
  session.send({ operation: 'trace' });
  let observed = null;
  for (let pass = 0; pass < 3 && observed === null; pass += 1) {
    const row = await session.next();
    if (row.outcome === 'trace-observed') observed = row;
    else assert.equal(row.stage, 'operator-control', JSON.stringify(row).slice(0, 400));
  }
  assert.ok(observed, 'the session must still answer after a control committed during replay');
  assert.equal(await session.close(), 0,
    'a change applied during a replay must not poison the session');
  console.log(JSON.stringify({ replay: replay.outcome, committed:
    { control_id: committed.control_id, outcome: committed.outcome, root: committed.root,
      checkpoint: committed.checkpoint } }));
} catch (error) {
  await session.abort();
  throw error;
}
