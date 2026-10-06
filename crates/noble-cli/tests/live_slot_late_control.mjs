import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { cli, evidenceDirectory, expectOutcome, selectedAuthority } from './live_slot_resource_ref_cli.mjs';

const binary = process.argv[2];
assert.ok(binary, 'pass the selected noble CLI binary');
const definition = 'def builder [ quote [ + ] compose ]';
const slot = 'ordinary-control';
const authority = selectedAuthority({
  sources: { 'definition-builder': definition, builder: 'builder', target: '1 +', caller: 'slot.invoke' },
  slots: [{ slotId: slot, input: ['I64'], output: ['I64'], effectCeiling: [], proofRequired: false }],
  effects: ['live.dispatch'],
  grants: [{ operation: 'publish', slotId: slot, allowed: true },
    { operation: 'dispatch', slotId: slot, allowed: true }],
});
const session = await cli(binary, 'LSLOT01-late-control', authority);
const input = value => ({ kind: 'i64', value: String(value) });
const issue = async (request, outcome) => expectOutcome(await session.issue(request), outcome);
let complete = false;
try {
  expectOutcome(await session.configured, 'configured');
  await issue({ operation: 'define', id: 'definition-builder', name: 'builder',
    source: definition, inputs: [] }, 'definition-retained');
  for (const [id, source, inputs] of [
    ['builder', 'builder', ['I64']], ['target', '1 +', ['I64']],
    ['caller', 'slot.invoke', ['I64', 'LiveRef<I64,I64,pure>']],
  ]) await issue({ operation: 'install', id, source, inputs,
    ...(id === 'builder' ? { selected_name: 'builder' } : {}) }, 'installed');
  await issue({ operation: 'candidate', id: 'target' }, 'candidate-staged');

  // The reader waits until invocation has started before consuming the next
  // operator line. This selected root has no protected checkpoint, but does
  // return a Program requiring a second post-root inspection roundtrip.
  session.send({ operation: 'invoke', id: 'builder', inputs: [input(2)], refs: [] });
  session.send({ operation: 'policy', grant: {
    operation: 'publish', slotId: slot, allowed: false,
  } });
  const root = expectOutcome(await session.next(), 'executed');
  assert.equal(root.guest_requests, 0);
  assert.equal(root.protected_operations, 0);
  assert.equal(root.epoch, '0');
  assert.ok(root.stack[0].target_identity?.definition_id);
  const late = expectOutcome(await session.next(), 'refused');
  assert.equal(late.stage, 'operator-control');
  assert.equal(late.when, 'too-late');
  assert.equal(late.scope, 'publish:ordinary-control');
  assert.match(late.diagnostic, /control was not applied/);
  assert.match(late.control_id, /^[0-9]+$/);

  const published = await issue({ operation: 'publish', slot, id: 'target',
    expected_epoch: '0' }, 'published');
  assert.equal(published.epoch, '1', 'late policy must not revoke the publication grant or advance epoch');
  const dispatched = await issue({ operation: 'invoke', id: 'caller', inputs: [input(2)],
    refs: [{ position: 1, ordinal: 0, slot }] }, 'executed');
  assert.equal(dispatched.epoch, '1');
  assert.deepEqual(dispatched.stack, [{ kind: 1, value: '3' }]);
  assert.equal(dispatched.guest_requests, 1);
  assert.equal(dispatched.protected_operations, 0);
  assert.equal(dispatched.request_trace.filter(row => row.operation === 'dispatch').length, 1);
  const afterCheckpoint = await issue({ operation: 'publish', slot, id: 'target',
    expected_epoch: '1', expected_incarnation: '1', expected_generation: '1' }, 'published');
  assert.equal(afterCheckpoint.epoch, '2', 'cancelled control must not leak into the next root');
  await issue({ operation: 'release-program', owner: root.stack[0].owner }, 'program-released');
  assert.equal(await session.close(), 0);
  fs.writeFileSync(path.join(evidenceDirectory, 'LSLOT01-late-control.summary.json'),
    JSON.stringify({ late, published_epoch: published.epoch,
      next_root: { epoch: dispatched.epoch, stack: dispatched.stack,
        guest_requests: dispatched.guest_requests, protected_operations: dispatched.protected_operations },
      after_checkpoint_epoch: afterCheckpoint.epoch,
      selected_binary_sha256: session.selectedBinarySha256 }, null, 2) + '\n',
    { flag: 'wx', mode: 0o600 });
  complete = true;
  console.log('late in-flight policy refused, session live, first and post-checkpoint publications unaffected');
} finally {
  if (!complete) await session.abort();
}
