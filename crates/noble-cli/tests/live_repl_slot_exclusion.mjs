import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { spawn } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

// LSLOT-09's guarded `Live-Wasm-Draft` profile: the checked `slot.invoke`
// caller is refused at admission with generation zero, while an explicit
// `:reload` still rebinds current names and a saved quotation keeps its old
// capture. Ordinary capture therefore does not become slot selection.

const binary = process.argv[2];
assert.ok(binary, 'pass the checked noble CLI binary');
const sha256 = value => createHash('sha256').update(value).digest('hex');
const binarySha256 = sha256(fs.readFileSync(binary));
const evidenceDirectory = process.env.NOBLE_SLOT_EVIDENCE_DIR;
if (evidenceDirectory) fs.mkdirSync(evidenceDirectory, { recursive: true, mode: 0o700 });
const record = row => {
  if (evidenceDirectory) fs.appendFileSync(path.join(evidenceDirectory, 'LSLOT09-repl.jsonl'),
    JSON.stringify(row) + '\n', { flag: 'a', mode: 0o600 });
};
const home = os.homedir();
const homeStat = fs.statSync(home);
assert.equal(homeStat.uid, process.getuid(), 'selected authority home must be owned by the operator');
assert.equal(homeStat.mode & 0o022, 0, 'authority home cannot be group or world writable');
const directory = fs.mkdtempSync(path.join(home, '.noble-lslot09-repl-'));
const sourceFile = path.join(directory, 'selected.noble');
const slotSource = 'slot.invoke\n';
const oldSource = 'def n [ 1 + ]\n', newSource = 'def n [ 2 + ]\n';
const slotSourceSha256 = sha256(slotSource);

function session(initial) {
  fs.writeFileSync(sourceFile, initial);
  const child = spawn(binary, ['live', 'repl', '--source', sourceFile, '--engine', 'v8'],
    { stdio: ['pipe', 'pipe', 'pipe'] });
  let pending = '', stderr = '', rows = [], receive, exitCode;
  child.stderr.on('data', bytes => {
    stderr += bytes.toString();
    record({ kind: 'stderr', text: bytes.toString() });
  });
  child.on('exit', code => { exitCode = code; });
  child.stdout.on('data', bytes => {
    pending += bytes.toString();
    for (let end; (end = pending.indexOf('\n')) >= 0;) {
      const report = JSON.parse(pending.slice(0, end));
      record({ kind: 'cli', report });
      pending = pending.slice(end + 1);
      if (receive) { const resolve = receive; receive = undefined; resolve(report); }
      else rows.push(report);
    }
  });
  const next = () => rows.length ? Promise.resolve(rows.shift()) : new Promise((resolve, reject) => {
    const timeout = setTimeout(() => reject(Error(`live child timed out: ${stderr}`)), 20000);
    receive = report => { clearTimeout(timeout); resolve(report); };
  });
  const submit = async command => {
    record({ kind: 'operator', command });
    child.stdin.write(`${command}\n`);
    return next();
  };
  const end = async () => {
    child.stdin.end();
    const code = child.exitCode ?? await new Promise(resolve => child.once('exit', resolve));
    record({ kind: 'exit', code, stderr, pending });
    return { code, stderr, pending };
  };
  const abort = async () => {
    child.stdin.end();
    child.kill();
    if (child.exitCode === null) await new Promise(resolve => child.once('exit', resolve));
  };
  return { next, submit, end, abort, exitCode: () => exitCode };
}

let summary;
try {
  // 1. Guarded Live-Wasm-Draft: the slot caller is refused at admission.
  const slotSession = session(slotSource);
  const refused = await slotSession.next();
  assert.equal(refused.schema, 'noble-live-report/v1');
  assert.equal(refused.stage, 'admission');
  assert.equal(refused.outcome, 'reload-refused');
  assert.match(refused.diagnostic, /slot\.invoke requires the opt-in live-slot source profile/);
  assert.equal(refused.generation, 0, 'a refused candidate installs no live generation');
  assert.deepEqual([refused.guest_requests, refused.protected_operations], [0, 0]);
  assert.deepEqual(refused.stack, []);
  const slotExit = await slotSession.end();
  assert.equal(slotExit.code, 2, slotExit.stderr);
  assert.equal(slotExit.pending, '', 'a refused candidate emits exactly one report');

  // 2. Control: explicit reload rebinds n while the saved quotation keeps the
  //    old capture. The live source generations are recorded for the registry
  //    epoch separation in LSLOT-02.
  const control = session(oldSource);
  const initial = await control.next();
  assert.equal(initial.generation, 1);
  const saved = await control.submit('[ n ]');
  const savedGeneration = saved.generation;
  assert.equal(saved.stack[0].type, 'Program');
  fs.writeFileSync(sourceFile, newSource);
  const reloaded = await control.submit(`:reload ${sourceFile}`);
  const reload = {
    generation: reloaded.generation,
    installed_generation: reloaded.installed_code_span?.generation,
    stack: reloaded.stack,
  };
  assert.deepEqual(reloaded.stack, saved.stack, 'reload must not rewrite a saved quotation');
  assert.equal(reload.installed_generation, reload.generation + 1);
  const invoked = await control.submit('20 swap run');
  assert.deepEqual(invoked.stack.map(cell => cell.value), ['21'],
    'the saved quotation keeps the old capture after the reload');
  assert.deepEqual([invoked.guest_requests, invoked.protected_operations], [0, 0]);
  const controlExit = await control.end();
  assert.equal(controlExit.code, 0, controlExit.stderr);

  summary = {
    case: 'LSLOT-09', gate: 'guarded-live-draft-refuses-checked-slot-caller',
    selected_binary_sha256: binarySha256, slot_source_sha256: slotSourceSha256,
    refusal: { profile: 'guarded Live-Wasm-Draft', stage: refused.stage,
      outcome: refused.outcome, diagnostic: refused.diagnostic, generation: refused.generation,
      exit: slotExit.code, guest_requests: refused.guest_requests,
      protected_operations: refused.protected_operations },
    control: { initial_generation: initial.generation, saved_generation: savedGeneration,
      reload, saved_program: saved.stack[0], stack: invoked.stack,
      source_generations: [initial.generation, savedGeneration, reload.generation,
        invoked.generation] },
    guest_requests: 0, protected_operations: 0,
  };
  if (evidenceDirectory) fs.writeFileSync(path.join(evidenceDirectory, 'LSLOT09-repl-summary.json'),
    JSON.stringify(summary, null, 2), { mode: 0o600 });
  console.log(JSON.stringify({ summary: 'LSLOT09-repl-summary.json', ...summary }));
} finally {
  fs.rmSync(directory, { recursive: true, force: true });
}
