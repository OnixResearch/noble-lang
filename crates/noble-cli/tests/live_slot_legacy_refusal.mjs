import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { spawn, spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

// LSLOT-09: the same checked generic caller that the opt-in live-slot profile
// accepts must be refused by Core-Bootstrap (compile and ordinary session)
// before any guest request, and ordinary quotation capture must not change.
// The guarded Live-Wasm-Draft refusal is the separate `live_repl` lane.

const binary = process.argv[2];
assert.ok(binary, 'pass the checked noble CLI binary');
const fixturePath = fileURLToPath(
  new URL('../../../specs/conformance/live-reference-cases.json', import.meta.url));
const fixture = JSON.parse(fs.readFileSync(fixturePath, 'utf8')).cases
  .find(row => row.id === 'LSLOT-09');
assert.equal(fixture.input.harness, 'legacy-profile-exclusion');
assert.ok(fixture.input.profiles.includes('Core-Bootstrap'));
assert.ok(fixture.input.profiles.includes('ordinary Core session'));
const sha256 = value => createHash('sha256').update(value).digest('hex');
const binarySha256 = sha256(fs.readFileSync(binary));
const evidenceDirectory = process.env.NOBLE_SLOT_EVIDENCE_DIR;
if (evidenceDirectory) fs.mkdirSync(evidenceDirectory, { recursive: true, mode: 0o700 });
const record = row => {
  if (evidenceDirectory) fs.appendFileSync(path.join(evidenceDirectory, 'LSLOT09-legacy.jsonl'),
    JSON.stringify(row) + '\n', { flag: 'a', mode: 0o600 });
};
const home = os.homedir();
const homeStat = fs.statSync(home);
assert.equal(homeStat.uid, process.getuid(), 'selected authority home must be owned by the operator');
assert.equal(homeStat.mode & 0o022, 0, 'authority home cannot be group or world writable');
const directory = fs.mkdtempSync(path.join(home, '.noble-lslot09-'));
const sourceFile = path.join(directory, 'slot-invoke.noble');
// The canonical attempt is a checked caller whose input includes a borrowed
// LiveRef<I64,I64,{}>. `pure` is the empty effect set in this profile.
const callerSource = fixture.input.attempt.includes('slot.invoke') ? 'slot.invoke' : null;
assert.equal(callerSource, 'slot.invoke');
fs.writeFileSync(sourceFile, `${callerSource}\n`, { flag: 'wx', mode: 0o600 });
const sourceSha256 = sha256(fs.readFileSync(sourceFile));

function run(args, input, expectedStatus) {
  const result = spawnSync(binary, args, { input, encoding: 'utf8', timeout: 120000 });
  assert.equal(result.error, undefined, `${args.join(' ')}: ${result.error}`);
  assert.equal(result.signal, null, `${args.join(' ')}: ${result.signal}`);
  let reports = [];
  const text = result.stdout.trim();
  if (text.startsWith('{')) reports = text.split('\n').map(line => JSON.parse(line));
  else if (text.startsWith('[')) reports = JSON.parse(text);
  record({ kind: 'command', args, stdin: input ?? null,
    stdin_sha256: input === undefined ? null : sha256(input),
    source_sha256: sourceSha256, status: result.status, stdout: result.stdout,
    stderr: result.stderr, reports });
  assert.equal(result.status, expectedStatus, `${args.join(' ')}: exit ${result.status}: ${result.stderr}`);
  return { status: result.status, reports, stdout: result.stdout, stderr: result.stderr };
}

let summary;
try {
  // 1. Establish the exact checked caller in the opt-in profile: the same
  //    bytes are installed with a borrowed LiveRef input and reflected.
  const authority = path.join(directory, 'authority.json');
  const sources = { caller: callerSource };
  fs.writeFileSync(authority, JSON.stringify({
    owner: 'legacy-refusal-host', quota: 4, effects: ['live.dispatch'],
    grants: [{ operation: 'publish', slotId: 'increment', allowed: true },
      { operation: 'dispatch', slotId: 'increment', allowed: true }],
    resources: [],
    slots: [{ slotId: 'increment', input: ['I64'], output: ['I64'],
      effectCeiling: [], proofRequired: false }],
    sources: Object.entries(sources).map(([id, source]) => ({ id, sha256: sha256(source) })),
  }), { flag: 'wx', mode: 0o600 });
  record({ kind: 'authority', bytes: fs.readFileSync(authority, 'utf8'),
    selected_binary_sha256: binarySha256 });
  const child = spawn('/bin/sh', ['-c', 'cat | exec "$1" live slot --engine v8 --authority "$2"',
    'sh', binary, authority], { stdio: ['pipe', 'pipe', 'pipe'] });
  let pending = '', stderr = '', rows = [], receive, exitCode;
  child.stderr.on('data', bytes => { stderr += bytes.toString(); });
  child.on('exit', code => { exitCode = code; });
  child.stdout.on('data', bytes => {
    pending += bytes.toString();
    for (let end; (end = pending.indexOf('\n')) >= 0;) {
      const row = JSON.parse(pending.slice(0, end));
      record({ kind: 'cli', row });
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
    record({ kind: 'operator', request });
    child.stdin.write(JSON.stringify(request) + '\n');
    return next();
  };
  const outcome = (row, expected) => { assert.equal(row.outcome, expected, JSON.stringify(row)); return row; };
  const configured = outcome(await next(), 'configured');
  assert.equal(configured.epoch, '0');
  const installed = outcome(await issue({ operation: 'install', id: 'caller',
    source: callerSource, inputs: ['I64', 'LiveRef<I64,I64,pure>'] }), 'installed');
  assert.equal(installed.guest_requests, 0);
  assert.equal(installed.protected_operations, 0);
  const site = outcome(await issue({ operation: 'reflect', id: 'caller', site_id: 0 }), 'reflected');
  assert.equal(site.site.instruction, 'slot.invoke');
  assert.deepEqual(site.site.input, ['I64']);
  assert.deepEqual(site.site.output, ['I64']);
  assert.deepEqual(site.site.effects, ['live.dispatch']);
  assert.equal(JSON.stringify(site.site).includes('slotId'), false);
  child.stdin.end();
  const exit = child.exitCode ?? await new Promise(resolve => child.once('exit', resolve));
  record({ kind: 'exit', code: exit, stderr });
  assert.equal(exit, 0, stderr);
  assert.equal(pending, '', 'incomplete slot report');

  // 2. Two legacy Core entry points refuse the same caller bytes at resolve.
  const compiled = run(['compile', sourceFile], undefined, 4);
  const compile = compiled.reports.at(-1);
  assert.equal(compile.profile, 'Core-Bootstrap');
  assert.equal(compile.outcome, 'unsupported');
  assert.match(compile.diagnostic, /slot\.invoke requires the opt-in live-slot source profile/);
  assert.deepEqual([compile.guest_requests, compile.protected_operations], [0, 0]);
  assert.equal(compile.prior_stack, 'unchanged');
  const sessionRun = run(['session'], `${callerSource}\n`, 4);
  const sessionRefusal = sessionRun.reports.at(-1);
  assert.equal(sessionRefusal.profile, 'Core-Bootstrap');
  assert.equal(sessionRefusal.outcome, 'unsupported');
  assert.match(sessionRefusal.diagnostic, /slot\.invoke requires the opt-in live-slot source profile/);
  assert.deepEqual([sessionRefusal.guest_requests, sessionRefusal.protected_operations], [0, 0]);
  assert.equal(sessionRefusal.prior_session, 'unchanged');

  // 3. The Core control keeps ordinary immutable capture after redefinition.
  const control = run(['session'],
    'def n [ 1 + ]\n[ n ]\ndef n [ 2 + ]\n20 swap run\n', 0);
  const receipts = control.reports;
  assert.deepEqual(receipts.map(row => row.outcome), ['defined', 'normal', 'defined', 'normal']);
  assert.equal(receipts[1].stack[0].type, 'Program');
  assert.deepEqual(receipts[3].stack.map(cell => [cell.type, cell.value]), [['I64', '21']]);
  assert.deepEqual([receipts[3].guest_requests, receipts[3].protected_operations], [0, 0]);

  summary = {
    case: 'LSLOT-09', gate: 'legacy-profiles-refuse-checked-slot-caller',
    selected_binary_sha256: binarySha256, caller_source_sha256: sourceSha256,
    checked_caller: { installed: installed.outcome, site: site.site },
    refusals: [
      { profile: 'Core-Bootstrap', entry: 'compile', status: compiled.status,
        stage: compile.stage, outcome: compile.outcome, diagnostic: compile.diagnostic,
        guest_requests: compile.guest_requests,
        protected_operations: compile.protected_operations },
      { profile: 'ordinary Core session', entry: 'session', status: sessionRun.status,
        stage: sessionRefusal.stage, outcome: sessionRefusal.outcome,
        diagnostic: sessionRefusal.diagnostic,
        guest_requests: sessionRefusal.guest_requests,
        protected_operations: sessionRefusal.protected_operations },
    ],
    control: { profile: 'Core-Bootstrap', saved: receipts[1].stack[0],
      stack: receipts[3].stack, guest_requests: receipts[3].guest_requests,
      protected_operations: receipts[3].protected_operations },
    guest_requests: 0, protected_operations: 0,
  };
  if (evidenceDirectory) fs.writeFileSync(path.join(evidenceDirectory, 'LSLOT09-legacy-summary.json'),
    JSON.stringify(summary, null, 2), { mode: 0o600 });
  console.log(JSON.stringify({ summary: 'LSLOT09-legacy-summary.json', ...summary }));
} finally {
  fs.rmSync(directory, { recursive: true, force: true });
}
