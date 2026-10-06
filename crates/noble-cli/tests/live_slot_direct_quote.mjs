import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { spawn } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

const binary = process.argv[2];
assert.ok(binary, 'pass the selected noble CLI binary');
const sources = { definition: 'def q [ quote ]', quoted: 'q', runner: 'swap [ run ] dip +' };
const digest = source => createHash('sha256').update(source).digest('hex');
const home = os.homedir();
assert.equal(fs.statSync(home).mode & 0o022, 0, 'authority home must not be writable by others');
const directory = fs.mkdtempSync(path.join(home, '.noble-live-slot-quote-'));
const authority = path.join(directory, 'authority.json');
const evidencePath = process.env.NOBLE_SLOT_QUOTE_EVIDENCE;
const record = row => {
  if (evidencePath) fs.appendFileSync(evidencePath, JSON.stringify(row) + '\n', { mode: 0o600 });
};
fs.writeFileSync(authority, JSON.stringify({
  owner: 'quote-owner-host', quota: 1, effects: [], resources: [], slots: [],
  grants: [{ operation: 'discard', slotId: 'quoted', allowed: true },
    { operation: 'discard', slotId: 'runner', allowed: true }],
  sources: Object.entries(sources).map(([id, source]) => ({ id, sha256: digest(source) })),
}), { flag: 'wx', mode: 0o600 });
record({ kind: 'authority', bytes: fs.readFileSync(authority, 'utf8'),
  binary_sha256: digest(fs.readFileSync(binary)) });
// The CLI accepts operator control only from an anonymous pipe.
const child = spawn('/bin/bash', ['-c', 'cat | exec "$1" live slot --engine v8 --authority "$2"',
  'sh', binary, authority], { stdio: ['pipe', 'pipe', 'pipe'] });
let pending = '', stderr = '', rows = [], receive;
child.stderr.on('data', bytes => { stderr += bytes.toString(); record({ kind: 'stderr', text: bytes.toString() }); });
child.stdout.on('data', bytes => {
  pending += bytes.toString();
  for (let end; (end = pending.indexOf('\n')) >= 0;) {
    const row = JSON.parse(pending.slice(0, end));
    pending = pending.slice(end + 1);
    record({ kind: 'cli', row });
    if (receive) { const resolve = receive; receive = undefined; resolve(row); }
    else rows.push(row);
  }
});
const next = () => rows.length ? Promise.resolve(rows.shift()) : new Promise((resolve, reject) => {
  const timeout = setTimeout(() => reject(Error(`selected CLI timed out: ${stderr}`)), 20000);
  receive = row => { clearTimeout(timeout); resolve(row); };
});
const issue = async request => {
  record({ kind: 'operator', request });
  child.stdin.write(JSON.stringify(request) + '\n');
  return next();
};
const expected = (row, outcome) => { assert.equal(row.outcome, outcome, JSON.stringify(row)); return row; };
const use = owner => ({ operation: 'invoke', id: 'runner', inputs: [
  { kind: 'i64', value: '20' }, { kind: 'program', owner },
], refs: [] });
try {
  expected(await next(), 'configured');
  expected(await issue({ operation: 'define', id: 'definition', name: 'q',
    source: sources.definition, inputs: [] }), 'definition-retained');
  expected(await issue({ operation: 'install', id: 'quoted', source: sources.quoted,
    selected_name: 'q', inputs: ['I64'] }), 'installed');
  expected(await issue({ operation: 'install', id: 'runner', source: sources.runner,
    inputs: ['I64', 'Program<empty,I64,pure>'] }), 'installed');
  const quoted = expected(await issue({ operation: 'invoke', id: 'quoted',
    inputs: [{ kind: 'i64', value: '5' }], refs: [] }), 'executed');
  assert.equal(quoted.guest_requests, 0);
  assert.equal(quoted.protected_operations, 0);
  assert.equal(quoted.stack.length, 1);
  assert.equal(quoted.stack[0].kind, 4);
  assert.equal(quoted.stack[0].source_id, null);
  assert.equal(quoted.stack[0].program_index, null);
  assert.deepEqual(quoted.stack[0].capture_values, [{ type: 'I64', value: '5' }]);
  const owner = quoted.stack[0].owner;
  const run = expected(await issue(use(owner)), 'executed');
  assert.deepEqual(run.stack, [{ kind: 1, value: '25' }]);
  assert.equal(run.guest_requests, 0);
  assert.equal(run.protected_operations, 0);
  const wrong = expected(await issue(use('program-999999')), 'refused');
  assert.equal(wrong.guest_requests, 0);
  assert.equal(wrong.protected_operations, 0);
  expected(await issue({ operation: 'release-program', owner }), 'program-released');
  const stale = expected(await issue(use(owner)), 'refused');
  assert.equal(stale.guest_requests, 0);
  assert.equal(stale.protected_operations, 0);
  const discarded = expected(await issue({ operation: 'discard', id: 'quoted' }), 'discarded');
  assert.equal(discarded.retire_code_spans.length, 1,
    'released dynamic output must leave no owner of its installed code span');
  child.stdin.end();
  const exit = child.exitCode ?? await new Promise(resolve => child.once('exit', resolve));
  assert.equal(exit, 0, stderr);
  console.log('checked direct quote retained, ran with 20 to yield 25, released and retired');
} catch (error) {
  child.stdin.end();
  child.kill();
  throw error;
} finally {
  fs.rmSync(directory, { recursive: true });
}
