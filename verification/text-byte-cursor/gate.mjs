import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

const [binary, output] = process.argv.slice(2);
if (!binary || !output) throw new Error('usage: node gate.mjs NOBLE_BINARY NEW_OUTPUT_DIRECTORY');
const binaryBytes = fs.readFileSync(binary);
const binarySha256 = crypto.createHash('sha256').update(binaryBytes).digest('hex');
fs.mkdirSync(output);
fs.mkdirSync(path.join(output, 'inputs'));
fs.mkdirSync(path.join(output, 'raw'));
const receipt = {
  schema: 'noble-text-byte-cursor-observation/v1',
  assertion: 'finite compiled managed-Wasm child observations, not source-bound or MA1/CALC acceptance',
  executable: { path: fs.realpathSync(binary), sha256: binarySha256 },
  cases: [],
};

function child(name, args, source) {
  const input = Buffer.from(`${source}\n`);
  fs.writeFileSync(path.join(output, 'inputs', `${name}.noble`), input);
  const run = spawnSync(binary, args, {
    input, encoding: 'utf8', timeout: 180_000, killSignal: 'SIGKILL',
    maxBuffer: 4 * 1024 * 1024,
  });
  assert.equal(run.signal, null, `${name}: child timed out or was killed`);
  if (run.error) throw run.error;
  fs.writeFileSync(path.join(output, 'raw', `${name}.stdout`), run.stdout);
  fs.writeFileSync(path.join(output, 'raw', `${name}.stderr`), run.stderr);
  const reports = run.stdout.trim().split('\n').map(line => JSON.parse(line));
  assert.equal(reports.length, 1, `${name}: expected one compiled child report`);
  receipt.cases.push({ name, args, source, source_sha256: crypto.createHash('sha256').update(input).digest('hex'),
    exit: run.status, report: reports[0] });
  return { exit: run.status, report: reports[0] };
}

const vectors = [
  ['ascii', '"Aé" 0 text.byte', 'Aé', '65'],
  ['utf8-first', '"Aé" 1 text.byte', 'Aé', '195'],
  ['utf8-second', '"Aé" 2 text.byte', 'Aé', '169'],
  ['eof-exact', '"Aé" 3 text.byte', 'Aé', '-1'],
  ['empty-eof', '"" 0 text.byte', '', '-1'],
  ['negative', '"Aé" -1 text.byte', 'Aé', '-2'],
  ['negative-minimum', '"Aé" -9223372036854775808 text.byte', 'Aé', '-2'],
  ['past-end', '"Aé" 4 text.byte', 'Aé', '-3'],
  ['past-end-wrap32', '"Aé" 4294967297 text.byte', 'Aé', '-3'],
  ['past-end-maximum', '"Aé" 9223372036854775807 text.byte', 'Aé', '-3'],
];
for (const optimized of ['off', 'on']) {
  let allocation = null;
  for (const [label, source, text, value] of vectors) {
    const name = `${label}-${optimized}`;
    const { exit, report } = child(name, ['session', '--text-byte-cursor', '--opt', optimized], source);
    assert.equal(exit, 0, `${name}: compiled child exited unsuccessfully`);
    assert.equal(report.schema, 'noble-core-report/v1');
    assert.equal(report.profile, 'Text-Byte-Cursor-v1');
    assert.equal(report.backend, 'managed-linear-memory');
    assert.equal(report.stage, 'wasm');
    assert.equal(report.outcome, 'normal');
    assert.equal(report.status, 0);
    assert.equal(report.native_trap, null);
    assert.equal(report.module.optimized, optimized === 'on');
    assert.deepEqual(report.request_trace, []);
    assert.equal(report.guest_requests, 0);
    assert.equal(report.stack.length, 2);
    assert.equal(report.stack[0].type, 'Text');
    assert.equal(report.stack[0].value, text);
    assert.equal(report.stack[1].type, 'I64');
    assert.equal(report.stack[1].value, value);
    allocation ??= report.metrics.allocated_total_bytes;
    assert.equal(report.metrics.allocated_total_bytes, allocation,
      `${name}: cursor result allocated guest cells based on byte or status`);
  }
  const name = `same-text-sequential-bytes-${optimized}`;
  const sequence = '\"Aé\" dup 0 text.byte swap 1 text.byte swap 2 text.byte swap 3 text.byte';
  const { exit, report } = child(name, ['session', '--text-byte-cursor', '--opt', optimized], sequence);
  assert.equal(exit, 0);
  assert.equal(report.profile, 'Text-Byte-Cursor-v1');
  assert.equal(report.stage, 'wasm');
  assert.equal(report.outcome, 'normal');
  assert.equal(report.native_trap, null);
  assert.equal(report.module.optimized, optimized === 'on');
  assert.equal(report.guest_requests, 0);
  assert.deepEqual(report.request_trace, []);
  assert.deepEqual(report.stack.map(({ type, value }) => ({ type, value })), [
    { type: 'Text', value: 'Aé' },
    { type: 'I64', value: '65' },
    { type: 'I64', value: '195' },
    { type: 'I64', value: '169' },
    { type: 'Text', value: 'Aé' },
    { type: 'I64', value: '-1' },
  ]);
  assert.equal(report.stack[0].handle, report.stack[4].handle,
    'cursor operation replaced the original immutable Text handle');
}
const unbound = child('core-word-unbound', ['session'], '"ab" 0 text.byte');
assert.equal(unbound.exit, 2);
assert.equal(unbound.report.profile, 'Core-Bootstrap');
assert.equal(unbound.report.outcome, 'unbound-word');
assert.equal(unbound.report.prior_stack, 'unchanged');
assert.equal(unbound.report.prior_namespace, 'unchanged');
const wrapping = child('core-I64-wrapping', ['session'], '9223372036854775807 1 +');
assert.equal(wrapping.exit, 0);
assert.equal(wrapping.report.profile, 'Core-Bootstrap');
assert.equal(wrapping.report.outcome, 'normal');
assert.deepEqual(wrapping.report.request_trace, []);
assert.equal(wrapping.report.guest_requests, 0);
assert.deepEqual(wrapping.report.stack.map(({ type, value }) => ({ type, value })),
  [{ type: 'I64', value: '-9223372036854775808' }]);
const invalidProfile = child('declared-combination-refused',
  ['session', '--declared-modules', '--text-byte-cursor'], '"ab" 0 text.byte');
assert.notEqual(invalidProfile.exit, 0);
assert.equal(invalidProfile.report.stage, 'arguments');
assert.equal(invalidProfile.report.profile, 'Core-Bootstrap');
fs.writeFileSync(path.join(output, 'observation.json'), `${JSON.stringify(receipt, null, 2)}\n`);
console.log(JSON.stringify({ outcome: 'observed', binary_sha256: binarySha256,
  cases: receipt.cases.length, output, source_bound: false, calculator_accepted: false }));
