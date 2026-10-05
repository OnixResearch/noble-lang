import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const binary = process.argv[2];
assert.ok(binary, 'pass the checked noble CLI binary path');
const authority = fileURLToPath(new URL('./live_slot_replay_failure_authority.json', import.meta.url));
// The CLI deliberately refuses file-backed stdin: cat owns the sole writer of
// an anonymous pipe for this separate opt-in worker.
const child = spawn('/bin/bash', ['-c', 'cat | exec "$1" live slot --engine v8 --authority "$2"',
  'sh', binary, authority], { stdio: ['pipe', 'pipe', 'pipe'] });
let buffer = '';
const rows = [];
let waiting;
let stderr = '';
child.stderr.on('data', bytes => { stderr += bytes.toString(); });
child.stdout.on('data', bytes => {
  buffer += bytes.toString();
  for (let end; (end = buffer.indexOf('\n')) >= 0;) {
    const row = JSON.parse(buffer.slice(0, end));
    buffer = buffer.slice(end + 1);
    if (waiting) { const resolve = waiting; waiting = undefined; resolve(row); }
    else rows.push(row);
  }
});
const next = () => rows.length ? Promise.resolve(rows.shift()) : new Promise((resolve, reject) => {
  const timeout = setTimeout(() => reject(Error(`CLI receipt timeout: ${stderr}`)), 15000);
  waiting = row => { clearTimeout(timeout); resolve(row); };
});
const issue = async request => {
  child.stdin.write(JSON.stringify(request) + '\n');
  return next();
};
const expect = (row, outcome) => {
  assert.equal(row.outcome, outcome, JSON.stringify(row));
  return row;
};
try {
  expect(await next(), 'configured');
  expect(await issue({ operation: 'install', id: 'caller', source: 'slot.invoke',
    inputs: ['I64', 'LiveRef<I64,Unit+I64,test.emit>'] }), 'installed');
  expect(await issue({ operation: 'define', id: 'definition', name: 'replay',
    source: 'def replay [ "first" test.emit drop 1 = [ "second" test.emit ] [ "different" test.emit ] if 42 ]',
    inputs: [] }), 'definition-retained');
  expect(await issue({ operation: 'install', id: 'named', source: 'replay',
    inputs: ['I64'], selected_name: 'replay' }), 'installed');
  expect(await issue({ operation: 'publish', slot: 'emitter', id: 'named',
    program_index: 0, expected_epoch: '0' }), 'published');
  const refs = [{ position: 1, ordinal: 0, slot: 'emitter' }];
  const recorded = expect(await issue({ operation: 'invoke', id: 'caller',
    inputs: [{ kind: 'i64', value: '1' }], refs, record: true }), 'executed');
  assert.equal(recorded.protected_operations, 2);
  assert.deepEqual(recorded.stack, [{ kind: 3, value: '0' }, { kind: 1, value: '42' }]);
  assert.deepEqual(recorded.request_trace.filter(row => row.operation === 'effect')
    .map(row => row.request), ['first', 'second']);
  expect(await issue({ operation: 'policy',
    grant: { operation: 'effect', slotId: 'emitter', allowed: false } }), 'policy-updated');
  const replay = await issue({ operation: 'replay', token: recorded.replay_token,
    inputs: [{ kind: 'i64', value: '0' }], refs,
    expected_trace: recorded.request_trace, expected_stack: recorded.stack });
  assert.ok(['replay-diverged', 'replay-refused'].includes(replay.outcome), JSON.stringify(replay));
  assert.equal(replay.actual?.outcome, 'execution-failed', JSON.stringify(replay));
  assert.equal(replay.actual.protected_operations, 0,
    'the virtually performed first effect must not count as a real protected operation on failure');
  assert.deepEqual(replay.actual.request_trace.filter(row => row.operation === 'effect')
    .map(row => row.request), ['first']);
  assert.equal(replay.actual.request_trace.find(row => row.operation === 'effect')?.outcome,
    'performed');
  child.stdin.end();
  const exit = child.exitCode === null
    ? await new Promise(resolve => child.once('exit', resolve)) : child.exitCode;
  assert.equal(exit, 2, stderr);
  console.log('checked conditional replay: first response scripted, second diverged, no real replay effect');
} catch (error) {
  child.stdin.end();
  child.kill();
  throw error;
}
