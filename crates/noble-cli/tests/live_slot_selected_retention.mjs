import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { cli, evidenceDirectory, expectOutcome, selectedAuthority } from './live_slot_resource_ref_cli.mjs';

const binary = process.argv[2];
const childIndex = Number(process.argv[3]);
const childEntry = Number(process.argv[4]);
assert.ok(Number.isInteger(childIndex) && childIndex >= 0,
  'select static child index from independently checked compiler metadata');
assert.ok(Number.isInteger(childEntry) && childEntry >= 4,
  'bind observed static child entry to checked compiler metadata');
const sources = { definition: 'def builder [ quote [ 1 + ] compose ]', builder: 'builder',
  v2: '2 +', v2fresh: '2 +', runner: 'run', caller: 'slot.invoke' };
const authority = selectedAuthority({ sources, quota: 1, effects: ['live.dispatch'],
  slots: [{ slotId: 'Q', input: ['I64'], output: ['I64'], effectCeiling: [], proofRequired: false }],
  grants: [{ operation: 'publish', slotId: 'Q', allowed: true },
    { operation: 'dispatch', slotId: 'Q', allowed: true },
    { operation: 'discard', slotId: 'v2', allowed: true }] });
const session = await cli(binary, 'LSLOT08-selected-dynamic-owner-retention', authority);
const input = value => ({ kind: 'i64', value: String(value) });
const refs = [{ position: 1, ordinal: 0, slot: 'Q' }];
const issue = async (request, outcome) => {
  const row = expectOutcome(await session.issue(request), outcome);
  if ('guest_requests' in row) assert.equal(row.guest_requests, 0, JSON.stringify(row));
  if ('protected_operations' in row) assert.equal(row.protected_operations, 0, JSON.stringify(row));
  return row;
};
let complete = false;
try {
  expectOutcome(await session.configured, 'configured');
  await issue({ operation: 'define', id: 'definition', name: 'builder',
    source: sources.definition, inputs: [] }, 'definition-retained');
  const first = await issue({ operation: 'install', id: 'builder',
    source: sources.builder, selected_name: 'builder', inputs: ['I64'] }, 'installed');
  const candidate = await issue({ operation: 'candidate', id: 'builder',
    program_index: childIndex }, 'candidate-staged');
  assert.equal(candidate.program_index, childIndex);
  const initial = await issue({ operation: 'publish', slot: 'Q', id: 'builder',
    program_index: childIndex, expected_epoch: '0' }, 'published');
  assert.deepEqual([initial.epoch, initial.incarnation, initial.generation], ['1', '1', '1']);
  const built = await issue({ operation: 'invoke', id: 'builder',
    inputs: [input(2)], refs: [] }, 'executed');
  const [saved] = built.stack;
  assert.equal(built.stack.length, 1);
  assert.equal(saved.kind, 4);
  assert.equal(saved.source_id, null);
  assert.equal(saved.program_index, null);
  assert.equal(saved.capture_status, 'checked-selected-origin');
  assert.equal(saved.verified_origin.caller_id, 'builder');
  assert.equal(saved.verified_origin.artifact_sha256, first.artifact_sha256);
  assert.equal(saved.verified_origin.stack_position, 0);
  assert.equal(saved.capture_values.find(entry => entry.field === 'cell_a')?.value
    ?.find(cell => cell.kind === 1)?.payload, '2');
  const staticChild = saved.capture_values.find(entry => entry.field === 'cell_b')?.value?.[0];
  assert.equal(staticChild?.kind, 4);
  assert.equal(staticChild.x, childEntry,
    'saved composite must actually contain the published builder static child entry');
  await issue({ operation: 'install', id: 'runner', source: sources.runner,
    inputs: ['Program<empty,I64,pure>'] }, 'installed');
  const run = owner => issue({ operation: 'invoke', id: 'runner',
    inputs: [{ kind: 'program', owner }], refs: [] }, 'executed');
  assert.deepEqual((await run(saved.owner)).stack, [{ kind: 1, value: '3' }]);
  await issue({ operation: 'install', id: 'caller', source: sources.caller,
    inputs: ['I64', 'LiveRef<I64,I64,pure>'] }, 'installed');
  await issue({ operation: 'install', id: 'v2', source: sources.v2,
    inputs: ['I64'] }, 'installed');
  await issue({ operation: 'candidate', id: 'v2' }, 'candidate-staged');
  const attempt = { operation: 'publish', slot: 'Q', id: 'v2', program_index: 0,
    expected_epoch: '1', expected_incarnation: '1', expected_generation: '1' };
  await issue({ operation: 'hold-checkpoint', import: 'live_select', occurrence: 1 },
    'checkpoint-hold-selected');
  session.send({ operation: 'invoke', id: 'caller', inputs: [input(5)], refs });
  expectOutcome(await session.next(), 'checkpoint-entered');
  const held = await issue(attempt, 'retention-budget-refused');
  assert.equal(held.epoch, '1');
  const oldRoot = expectOutcome(await session.issue({ operation: 'resume-checkpoint' }), 'executed');
  assert.deepEqual([oldRoot.guest_requests, oldRoot.protected_operations, oldRoot.epoch], [1, 0, '1']);
  assert.deepEqual(oldRoot.stack, [{ kind: 1, value: '6' }]);
  assert.equal(oldRoot.request_trace[0].artifactSha256, first.artifact_sha256);
  assert.ok(oldRoot.request_trace[0].programValueId.startsWith(
    `artifact:${first.artifact_sha256}:cell:`));
  const afterRoot = await issue(attempt, 'retention-budget-refused');
  assert.equal(afterRoot.epoch, '1');
  assert.deepEqual((await run(saved.owner)).stack, [{ kind: 1, value: '3' }]);
  await issue({ operation: 'discard', id: 'v2' }, 'discarded');
  await issue({ operation: 'release-program', owner: saved.owner }, 'program-released');
  await issue({ operation: 'invoke', id: 'runner',
    inputs: [{ kind: 'program', owner: saved.owner }], refs: [] }, 'refused');
  await issue({ operation: 'install', id: 'v2fresh', source: sources.v2fresh,
    inputs: ['I64'] }, 'installed');
  await issue({ operation: 'candidate', id: 'v2fresh' }, 'candidate-staged');
  const published = await issue({ ...attempt, id: 'v2fresh' }, 'published');
  assert.deepEqual([published.epoch, published.incarnation, published.generation], ['2', '1', '2']);
  assert.ok(published.retire_code_spans.some(span => span.generation === Number(first.generation)),
    'last saved owner release must permit former static child code retirement');
  const newRoot = expectOutcome(await session.issue({ operation: 'invoke', id: 'caller',
    inputs: [input(5)], refs }), 'executed');
  assert.deepEqual([newRoot.guest_requests, newRoot.protected_operations, newRoot.epoch], [1, 0, '2']);
  assert.deepEqual(newRoot.stack, [{ kind: 1, value: '7' }]);
  assert.notEqual(newRoot.request_trace[0].programValueId, oldRoot.request_trace[0].programValueId);
  assert.equal(await session.close(), 0);
  const records = fs.readFileSync(session.log, 'utf8').trim().split('\n').map(JSON.parse);
  assert.deepEqual(records[0].authority, authority);
  assert.equal(records[0].selected_binary_sha256, session.selectedBinarySha256);
  assert.equal(records.at(-1).kind, 'exit');
  assert.equal(records.at(-1).code, 0);
  assert.deepEqual(records.filter(record => record.kind === 'operator').map(record => record.request),
    fs.readFileSync(session.requests, 'utf8').trim().split('\n').map(JSON.parse));
  assert.deepEqual(records.filter(record => record.kind === 'cli').map(record => record.row),
    fs.readFileSync(session.responses, 'utf8').trim().split('\n').map(JSON.parse));
  const summary = { selected_binary_sha256: session.selectedBinarySha256,
    static_child_program_index: childIndex, static_child_entry: childEntry,
    static_child_artifact_sha256: first.artifact_sha256,
    saved_owner: saved.owner, saved_origin: saved.verified_origin,
    old_root: { epoch: oldRoot.epoch, stack: oldRoot.stack,
      guest_requests: oldRoot.guest_requests, protected_operations: oldRoot.protected_operations,
      request_trace: oldRoot.request_trace },
    new_root: { epoch: newRoot.epoch, stack: newRoot.stack,
      guest_requests: newRoot.guest_requests, protected_operations: newRoot.protected_operations,
      request_trace: newRoot.request_trace },
    retire_code_spans: published.retire_code_spans };
  fs.writeFileSync(path.join(evidenceDirectory,
    'LSLOT08-selected-dynamic-owner-retention.summary.json'), JSON.stringify(summary, null, 2) + '\n',
  { flag: 'wx', mode: 0o600 });
  complete = true;
  console.log('source-selected dynamic owner transitively retained published v1 through quota and released before v2 publication');
} finally { if (!complete) await session.abort(); }
