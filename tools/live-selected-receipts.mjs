// Scoped real-guest receipts, not canonical LIVE-01..10 status evidence.
// Run with the pinned Node 24 binary and --noble ABSOLUTE_COMPILED_BINARY.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import readline from 'node:readline';

assert.deepEqual(process.argv.slice(2, 3), ['--noble']);
const binary = process.argv[3];
assert.equal(process.argv.length, 4);
assert.ok(path.isAbsolute(binary) && fs.statSync(binary).isFile());
const binarySha256 = digest(fs.readFileSync(binary));
const deadlineMs = 20_000;
function digest(bytes) { return createHash('sha256').update(bytes).digest('hex'); }
function deadline(promise, label) {
  let timer;
  return Promise.race([promise, new Promise((_, reject) => {
    timer = setTimeout(() => reject(Error(`${label} exceeded ${deadlineMs}ms`)), deadlineMs);
  })]).finally(() => clearTimeout(timer));
}
function fixture(initial) {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'noble-live-selected-receipt-'));
  const source = path.join(directory, 'math.noble');
  fs.writeFileSync(source, initial, { flag: 'wx', mode: 0o600 });
  return { directory, source, replace(text) {
    const temporary = `${source}.tmp`;
    const fd = fs.openSync(temporary, 'wx', 0o600);
    try { fs.writeSync(fd, text); fs.fsyncSync(fd); }
    finally { fs.closeSync(fd); }
    fs.renameSync(temporary, source);
    return digest(text);
  }, close() { fs.rmSync(directory, { recursive: true, force: true }); } };
}
function guest(args) {
  const child = spawn(binary, args, { stdio: ['pipe', 'pipe', 'pipe'] });
  const lines = readline.createInterface({ input: child.stdout })[Symbol.asyncIterator]();
  let stderr = '';
  child.stderr.on('data', chunk => { stderr += chunk; });
  const closed = new Promise(resolve => child.once('close', (code, signal) => resolve({ code, signal })));
  const receipts = [];
  const report = async input => {
    if (input !== null) {
      await deadline(new Promise((resolve, reject) => child.stdin.write(`${input}\n`, error =>
        error ? reject(error) : resolve())), 'live input');
    }
    const line = await deadline(lines.next(), 'live report');
    assert.equal(line.done, false, `live child closed without receipt: ${stderr}`);
    assert.ok(Buffer.byteLength(line.value) <= 65_536);
    const value = JSON.parse(line.value);
    receipts.push({ input, raw_jsonl: line.value, parsed: value });
    return value;
  };
  return { report, receipts, async close(code) {
    child.stdin.end();
    const exit = await deadline(closed, 'live child exit');
    assert.deepEqual(exit, { code, signal: null }, stderr);
    assert.equal(stderr, '');
    return exit;
  }, abort() { if (child.exitCode === null) child.kill(); } };
}
async function caseRun(label, initial, args, scenario) {
  const sample = fixture(initial);
  const live = guest(['live', 'repl', '--source', sample.source, '--engine', 'v8', ...args]);
  try {
    const outcome = await scenario(sample, live);
    const exit = await live.close(outcome.exit);
    console.log(JSON.stringify({ scope: 'selected-live-scoped-not-canonical', case: label,
      binary_sha256: binarySha256, initial_source_sha256: digest(initial),
      receipts: live.receipts, exit, unproven: outcome.unproven ?? [] }));
  } finally { live.abort(); sample.close(); }
}
async function main() {
  await caseRun('LIVE-02-named-transitive-not-in-flight', 'def addone [ 1 + ]', [],
    async (sample, live) => {
      assert.equal((await live.report(null)).source_sha256, digest('def addone [ 1 + ]'));
      assert.equal((await live.report('def twice [ addone addone ]')).outcome, 'defined');
      const saved = await live.report('20 [ addone ]');
      assert.equal(saved.stack[1].type, 'Program');
      const next = sample.replace('def addone [ 2 + ]');
      const ack = await live.report(`:reload ${sample.source}`);
      assert.equal(ack.outcome, 'reload-committed');
      assert.equal(ack.source_sha256, next);
      assert.deepEqual(ack.stack, saved.stack);
      const old = await live.report('run');
      assert.equal(old.stack.at(-1).value, '21');
      const fresh = await live.report('20 twice');
      assert.equal(fresh.stack.at(-1).value, '24');
      return { exit: 0, unproven: ['LIVE-02 active old frame while reload requested'] };
    });
  const bindingsDirectory = fs.mkdtempSync(path.join(os.tmpdir(), 'noble-live-bindings-'));
  try {
    const manifest = path.join(bindingsDirectory, 'bindings.txt');
    fs.writeFileSync(manifest, 'bind ledger@1 test.emit emit-a Text -- ! test.emit allow\n'
      + 'bind ledger@2 test.emit emit-b Text -- ! test.emit allow\n');
    await caseRun('LIVE-06-old-adapter-current-alias-denial',
      'module ledger@1 [ require emit Text -- ! test.emit export send def send [ "A" emit ] ]',
      ['--declared-modules', '--bindings', manifest, '--verify-source-before-execute'],
      async (sample, live) => {
      assert.equal((await live.report(null)).outcome, 'reload-committed');
      assert.equal((await live.report('import ledger@1 as l')).outcome, 'namespace-committed');
      const saved = await live.report('[ l.send ]');
      assert.equal(saved.stack[0].type, 'Program');
      sample.replace('module ledger@2 [ require emit Text -- ! test.emit export send def send [ "B" emit ] ]');
      assert.equal((await live.report(`:reload ${sample.source}`)).outcome, 'reload-committed');
      assert.equal((await live.report('import ledger@2 as l')).outcome, 'namespace-committed');
      const denied = await live.report('run');
      assert.equal(denied.outcome, 'trap');
      assert.equal(denied.request_trace[0].adapter_identity, 'emit-a');
      assert.equal(denied.request_trace[0].decision, 'deny');
        return { exit: 2, unproven: ['LIVE-06 explicit host-revoke command distinct from alias rebind'] };
      });
  } finally { fs.rmSync(bindingsDirectory, { recursive: true, force: true }); }
  const evolve = 'def evolve [ dup 18 - quote [ + ] compose self.generation swap self.propose '
    + 'drop dup 18 - quote [ + ] compose self.generation swap self.propose drop 1 + ]';
  await caseRun('LIVE-10-queued-proposal-terminal-denial', evolve,
    ['--self-edit', 'evolve', '--expect-generation', '1'], async (_sample, live) => {
      assert.equal((await live.report(null)).outcome, 'reload-committed');
      assert.equal((await live.report(null)).outcome, 'grant-selected');
      assert.equal((await live.report('[ evolve ]')).stack[0].type, 'Program');
      const denied = await live.report('20 evolve');
      assert.equal(denied.outcome, 'trap');
      assert.equal(denied.cancelled_proposal_roots_before_cancel, 1);
      assert.equal(denied.cancelled_proposal_roots_remaining, 0);
      assert.equal(denied.cancelled_proposal_root_state_after & 127, 0);
      assert.equal(denied.cancelled_proposal_repeat_release_status, 4);
      assert.equal(denied.cancelled_proposal_unrelated_root_release_status, 4);
      return { exit: 2, unproven: ['LIVE-10 tampered capture/CAS/bytecode refusal matrix'] };
    });
  const work = 'def evolve [ dup 18 - quote [ + ] compose self.generation swap self.propose drop'
    + ' 1 +'.repeat(80) + ' ]';
  await caseRun('LIVE-10-queued-proposal-step-quota', work,
    ['--self-edit', 'evolve', '--expect-generation', '1', '--max-guest-steps', '300'],
    async (_sample, live) => {
      assert.equal((await live.report(null)).outcome, 'reload-committed');
      assert.equal((await live.report(null)).outcome, 'grant-selected');
      const exhausted = await live.report('20 evolve');
      assert.equal(exhausted.outcome, 'runtime-exhausted');
      assert.equal(exhausted.cancelled_proposal_roots_before_cancel, 1);
      assert.equal(exhausted.cancelled_proposal_roots_remaining, 0);
      assert.equal(exhausted.protected_operations, 0);
      return { exit: 5, unproven: ['LIVE-10 other refusal matrix variants'] };
    });
  const selected = fixture('def addone [ 1 + ]');
  const watch = guest(['live', 'watch', '--source', selected.source, '--engine', 'v8']);
  try {
    const initial = await watch.report(null);
    assert.equal(initial.watch_sequence, 0);
    const next = selected.replace('def addone [ 2 + ]');
    const ack = await watch.report(null);
    assert.equal(ack.watch_sequence, 1);
    assert.equal(ack.source_sha256, next);
    assert.equal(ack.source_freshness, 'matched-at-watch-ack-check');
    const ran = await watch.report('20 addone');
    assert.equal(ran.outcome, 'normal');
    assert.equal(ran.source_generation, ack.generation);
    assert.equal(ran.stack.at(-1).value, '22');
    const exit = await watch.close(0);
    console.log(JSON.stringify({ scope: 'selected-live-scoped-not-canonical',
      case: 'LIVE-05-single-paired-save-only', binary_sha256: binarySha256,
      initial_source_sha256: digest('def addone [ 1 + ]'), receipts: watch.receipts, exit,
      unproven: ['LIVE-05 partial write', 'forced supersede during compile',
        'in-flight pre-commit input', 'effectful trap'] }));
  } finally { watch.abort(); selected.close(); }
}
main().catch(error => { console.error(`scoped live selected receipts failed: ${error.stack ?? error}`);
  process.exitCode = 1; });
