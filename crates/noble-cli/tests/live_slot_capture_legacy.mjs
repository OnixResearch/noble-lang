import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { spawn } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const binary = process.argv[2];
assert.ok(binary, 'pass the checked noble CLI binary');
const fixturePath = fileURLToPath(new URL('../../../specs/conformance/live-reference-cases.json', import.meta.url));
const fixture = JSON.parse(fs.readFileSync(fixturePath, 'utf8')).cases.find(row => row.id === 'LSLOT-01');
const sources = {
  definition: fixture.input.definition,
  saved: fixture.input.saved_program,
  runner: 'run',
  generic: 'slot.invoke',
  original: 'n',
  replacement: '[ 2 + ]',
};
const home = os.homedir();
const homeStat = fs.statSync(home);
assert.equal(homeStat.uid, process.getuid(), 'selected authority home must be owned by the operator');
assert.equal(homeStat.mode & 0o022, 0, 'authority home cannot be group or world writable');
const directory = fs.mkdtempSync(path.join(home, '.noble-live-slot-capture-'));
const authority = path.join(directory, 'authority.json');
const sha256 = value => createHash('sha256').update(value).digest('hex');
fs.writeFileSync(authority, JSON.stringify({
  owner: 'capture-regression-host', quota: 8, effects: ['live.dispatch'],
  grants: [{ operation: 'publish', slotId: 'increment', allowed: true },
    { operation: 'dispatch', slotId: 'increment', allowed: true }],
  resources: [], slots: [{ slotId: 'increment', input: ['I64'], output: ['I64'], effectCeiling: [], proofRequired: false }],
  sources: Object.entries(sources).map(([id, source]) => ({ id, sha256: sha256(source) })),
}), { flag: 'wx', mode: 0o600 });
// The child reads an anonymous pipe: cat owns its sole input end, never a file-backed stdin.
const child = spawn('/bin/bash', ['-c', 'cat | exec "$1" live slot --engine v8 --authority "$2"',
  'sh', binary, authority], { stdio: ['pipe', 'pipe', 'pipe'] });
let pending = '', stderr = '', rows = [], receive;
child.stderr.on('data', bytes => { stderr += bytes.toString(); });
child.stdout.on('data', bytes => {
  pending += bytes.toString();
  for (let end; (end = pending.indexOf('\n')) >= 0;) {
    const row = JSON.parse(pending.slice(0, end));
    pending = pending.slice(end + 1);
    if (receive) { const resolve = receive; receive = undefined; resolve(row); }
    else rows.push(row);
  }
});
const next = () => rows.length ? Promise.resolve(rows.shift()) : new Promise((resolve, reject) => {
  const timeout = setTimeout(() => reject(Error(`checked CLI timed out: ${stderr}`)), 20000);
  receive = row => { clearTimeout(timeout); resolve(row); };
});
const issue = async request => {
  child.stdin.write(JSON.stringify(request) + '\n');
  return next();
};
const expected = (row, outcome) => { assert.equal(row.outcome, outcome, JSON.stringify(row)); return row; };
const root20 = id => ({ operation: 'invoke', id, inputs: [{ kind: 'i64', value: '20' }],
  refs: [{ position: 1, ordinal: 0, slot: 'increment' }] });
try {
  expected(await next(), 'configured');
  expected(await issue({ operation: 'define', id: 'definition', name: 'n', source: sources.definition, inputs: [] }), 'definition-retained');
  expected(await issue({ operation: 'install', id: 'saved', source: sources.saved, inputs: [] }), 'installed');
  const captured = expected(await issue({ operation: 'invoke', id: 'saved', inputs: [], refs: [] }), 'executed');
  assert.equal(captured.stack[0].kind, 4);
  assert.equal(captured.stack[0].source_id, 'saved');
  const savedOwner = captured.stack[0].owner;
  expected(await issue({ operation: 'install', id: 'runner', source: sources.runner,
    inputs: ['I64', 'Program<I64,I64,pure>'] }), 'installed');
  expected(await issue({ operation: 'install', id: 'generic', source: sources.generic,
    inputs: ['I64', 'LiveRef<I64,I64,pure>'] }), 'installed');
  const original = expected(await issue({ operation: 'install', id: 'original', source: sources.original,
    selected_name: 'n', inputs: ['I64'] }), 'installed');
  const first = expected(await issue({ operation: 'publish', slot: 'increment', id: 'original',
    program_index: 1, expected_epoch: '0' }), 'published');
  assert.equal(first.epoch, '1');
  assert.equal(first.incarnation, '1');
  assert.equal(first.generation, '1');
  const before = expected(await issue({ operation: 'reflect', id: 'generic', site_id: 0 }), 'reflected');
  assert.deepEqual(before.site, { instruction: 'slot.invoke', siteId: 0,
    input: ['I64'], output: ['I64'], effects: ['live.dispatch'] });
  const oldGeneric = expected(await issue(root20('generic')), 'executed');
  assert.deepEqual(oldGeneric.stack, [{ kind: 1, value: '21' }]);
  assert.equal(oldGeneric.request_trace[0].artifactSha256, original.artifact_sha256);
  const replacement = expected(await issue({ operation: 'install', id: 'replacement',
    source: sources.replacement, inputs: [] }), 'installed');
  const second = expected(await issue({ operation: 'publish', slot: 'increment', id: 'replacement',
    program_index: 1, expected_epoch: '1', expected_incarnation: '1', expected_generation: '1' }), 'published');
  assert.equal(second.epoch, String(fixture.expected.new_global_epoch));
  assert.equal(second.incarnation, first.incarnation);
  assert.equal(second.generation, '2');
  const ordinary = expected(await issue({ operation: 'invoke', id: 'runner',
    inputs: [{ kind: 'i64', value: '20' }, { kind: 'program', owner: savedOwner }], refs: [] }), 'executed');
  const latest = expected(await issue(root20('generic')), 'executed');
  const after = expected(await issue({ operation: 'reflect', id: 'generic', site_id: 0 }), 'reflected');
  assert.deepEqual(ordinary.stack, [{ kind: 1, value: String(fixture.expected.saved_result) }]);
  assert.deepEqual(latest.stack, [{ kind: 1, value: String(fixture.expected.generic_result) }]);
  assert.deepEqual(after.site, before.site, 'one installed generic recipe cannot change at publication');
  assert.equal(latest.request_trace[0].artifactSha256, replacement.artifact_sha256);
  assert.notEqual(latest.request_trace[0].artifactSha256, original.artifact_sha256);
  assert.equal(ordinary.guest_requests, 0, 'the saved Program must not dispatch through the slot');
  assert.deepEqual(ordinary.request_trace, []);
  assert.equal(latest.guest_requests, 1);
  assert.equal(latest.protected_operations, 0);
  child.stdin.end();
  const exit = child.exitCode ?? await new Promise(resolve => child.once('exit', resolve));
  assert.equal(exit, 0, stderr);
  console.log('saved checked [ n ] owner remains 21; same opt-in generic selects fresh target 22 at epoch 2');
} catch (error) {
  child.stdin.end();
  child.kill();
  throw error;
} finally {
  fs.rmSync(directory, { recursive: true });
}
