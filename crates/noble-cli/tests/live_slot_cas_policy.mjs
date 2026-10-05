import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { createHash } from 'node:crypto';
import { closeSync, mkdirSync, mkdtempSync, openSync, readFileSync, rmSync, writeFileSync, writeSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const binary = process.argv[2];
assert.ok(binary, 'pass the selected real noble binary');
const authority = fileURLToPath(new URL('./live_slot_cas_policy_authority.json', import.meta.url));
const manifest = JSON.parse(readFileSync(authority, 'utf8'));
const source = {
  callAB: 'slot.invoke', callEmit: 'slot.invoke',
  A1: '1 +', A1fresh: '1 +', A2: '2 +', A3: '3 +',
  Ainc3: '1 +', Ainc4: '1 +', Ainc5: '1 +',
  Ainc6: '1 +', Ainc7: '1 +', Ainc8: '1 +',
  B1: '1 +', B1fresh: '1 +', B2: '2 +',
  emit1: '"touched" test.emit drop 1 +',
  emit2: '"touched-v2" test.emit drop 2 +',
};
for (const selected of manifest.sources) assert.equal(
  selected.sha256, createHash('sha256').update(source[selected.id]).digest('hex'),
  `authority source bytes for ${selected.id}`);
const evidenceDir = process.env.NOBLE_SLOT_EVIDENCE_DIR;
const artifactDir = evidenceDir ? resolve(evidenceDir) : mkdtempSync(join(tmpdir(), 'noble-lslot-cas-'));
if (evidenceDir) mkdirSync(artifactDir, { recursive: true, mode: 0o700 });
else process.once('exit', () => rmSync(artifactDir, { recursive: true, force: true }));
const input = value => ({ kind: 'i64', value: String(value) });
const cas = (slot, id, epoch, incarnation = null, generation = null, operation = 'publish') =>
  ({ operation, slot, id, expected_epoch: String(epoch),
    expected_incarnation: incarnation === null ? null : String(incarnation),
    expected_generation: generation === null ? null : String(generation) });

function session(name) {
  const path = join(artifactDir, `${name}.jsonl`);
  const fd = openSync(path, 'wx', 0o600);
  // The selected host requires an anonymous pipe with one independent writer.
  const child = spawn('/bin/bash', ['-c', 'cat | exec "$1" live slot --engine v8 --authority "$2"',
    'sh', binary, authority], { stdio: ['pipe', 'pipe', 'pipe'] });
  let buffer = '', stderr = '', waiter;
  const queue = [];
  let exitCode;
  child.stderr.on('data', bytes => { stderr += bytes.toString(); });
  child.on('exit', code => { exitCode = code; });
  child.stdout.on('data', bytes => {
    buffer += bytes.toString();
    for (let end; (end = buffer.indexOf('\n')) >= 0;) {
      const line = buffer.slice(0, end);
      buffer = buffer.slice(end + 1);
      writeSync(fd, `${line}\n`);
      const row = JSON.parse(line);
      if (waiter) { const current = waiter; waiter = null; current(row); }
      else queue.push(row);
    }
  });
  async function next() {
    if (queue.length) return queue.shift();
    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => { waiter = null; reject(Error(`CLI receipt timeout in ${name}: ${stderr}`)); }, 45000);
      waiter = row => { clearTimeout(timeout); resolve(row); };
    });
  }
  async function issue(request) {
    child.stdin.write(`${JSON.stringify(request)}\n`);
    return next();
  }
  async function end() {
    child.stdin.end();
    if (exitCode === undefined) await new Promise(resolve => child.once('exit', resolve));
    closeSync(fd);
    writeFileSync(join(artifactDir, `${name}.stderr.txt`), stderr, { mode: 0o600 });
    assert.ok(exitCode === 0 || exitCode === 2, `CLI exited ${exitCode}: ${stderr}`);
  }
  return { name, child, next, issue, end, path };
}

function outcome(row, expected) {
  assert.equal(row.outcome, expected, JSON.stringify(row));
  return row;
}
function observedRefusal(log, row, guestRequests = 0) {
  assert.equal(row.guest_requests, guestRequests, JSON.stringify(row));
  assert.equal(row.protected_operations, 0, JSON.stringify(row));
  log.refusal = { outcome: row.outcome, guest_requests: row.guest_requests,
    protected_operations: row.protected_operations };
}
async function install(s, id, inputs = ['I64']) {
  const installed = outcome(await s.issue({ operation: 'install', id,
    source: source[id], inputs }), 'installed');
  const staged = outcome(await s.issue({ operation: 'candidate', id }), 'candidate-staged');
  for (const row of [installed, staged]) {
    assert.equal(row.guest_requests, 0, JSON.stringify(row));
    assert.equal(row.protected_operations, 0, JSON.stringify(row));
  }
}
const publish = (s, slot, id, epoch, incarnation, generation, operation) =>
  s.issue(cas(slot, id, epoch, incarnation, generation, operation));
async function select(s, slot, expectedEpoch, expectedGeneration, expectedResult, caller = 'callAB') {
  const report = outcome(await s.issue({ operation: 'invoke', id: caller,
    inputs: [input(20)], refs: [{ position: 1, ordinal: 0, slot }] }), 'executed');
  assert.equal(report.epoch, String(expectedEpoch), JSON.stringify(report));
  assert.deepEqual(report.stack, [{ kind: 1, value: String(expectedResult) }], JSON.stringify(report));
  assert.equal(report.guest_requests, caller === 'callEmit' ? 2 : 1);
  assert.equal(report.protected_operations, caller === 'callEmit' ? 1 : 0);
  const dispatches = report.request_trace.filter(row => row.operation === 'dispatch');
  assert.equal(dispatches.length, 1);
  assert.equal(dispatches[0].slotId, slot);
  assert.equal(dispatches[0].generation, String(expectedGeneration));
  assert.equal(dispatches[0].outcome, 'allowed');
  return dispatches[0];
}
async function baseline(s) {
  outcome(await s.next(), 'configured');
  for (const id of ['callAB', 'A1', 'A2', 'A3', 'B1', 'B1fresh', 'B2']) await install(s, id,
    id === 'callAB' ? ['I64', 'LiveRef<I64,I64,pure>'] : ['I64']);
  outcome(await publish(s, 'B', 'B1', 0), 'published');
  outcome(await s.issue({ operation: 'delete', slot: 'B', expected_epoch: '1',
    expected_incarnation: '1', expected_generation: '1' }), 'deleted');
  const b = outcome(await publish(s, 'B', 'B1fresh', 2), 'published');
  assert.deepEqual([b.epoch, b.incarnation, b.generation], ['3', '2', '1']);
  outcome(await publish(s, 'A', 'A1', 3), 'published');
  let epoch = 4;
  for (let incarnation = 2; incarnation <= 7; incarnation++) {
    const removed = outcome(await s.issue({ operation: 'delete', slot: 'A',
      expected_epoch: String(epoch), expected_incarnation: String(incarnation - 1),
      expected_generation: '1' }), 'deleted');
    assert.equal(removed.epoch, String(++epoch));
    const id = incarnation === 2 ? 'A1fresh' : `Ainc${incarnation}`;
    await install(s, id);
    const published = outcome(await publish(s, 'A', id, epoch), 'published');
    assert.deepEqual([published.epoch, published.incarnation, published.generation],
      [String(++epoch), String(incarnation), '1']);
  }
  for (let generation = 2; generation <= 3; generation++) {
    const row = outcome(await publish(s, 'A', 'Ainc7', epoch, 7, generation - 1), 'published');
    assert.deepEqual([row.epoch, row.incarnation, row.generation],
      [String(++epoch), '7', String(generation)]);
  }
  assert.equal(epoch, 18);
  const before = await select(s, 'A', 18, 3, 21);
  assert.equal(before.incarnation, '7');
  const bBefore = await select(s, 'B', 18, 1, 21);
  assert.equal(bBefore.incarnation, '2');
  return { before, bBefore };
}

async function runCase(name, test) {
  const s = session(name);
  const summary = { name, raw: s.path, counters: join(artifactDir, `${name}.counters.json`) };
  try {
    await test(s, summary);
    summary.result = 'pass';
  } catch (error) {
    summary.result = 'failed';
    summary.error = String(error.stack ?? error);
    throw error;
  } finally {
    await s.end();
    writeFileSync(summary.counters, JSON.stringify(summary, null, 2) + '\n', { mode: 0o600 });
    console.log(evidenceDir ? JSON.stringify(summary) : `${name}: ${summary.result}`);
  }
}

await runCase('LSLOT04-same-slot-writers', async (s, log) => {
  const { before } = await baseline(s);
  const first = outcome(await publish(s, 'A', 'A2', 18, 7, 3), 'published');
  assert.deepEqual([first.epoch, first.incarnation, first.generation], ['19', '7', '4']);
  observedRefusal(log, outcome(await publish(s, 'A', 'A3', 18, 7, 3), 'stale-reject'));
  const after = await select(s, 'A', 19, 4, 22);
  assert.notEqual(after.programValueId, before.programValueId);
  log.committed = first; log.after = after;
});
await runCase('LSLOT04-unrelated-slot-race', async (s, log) => {
  const { before, bBefore } = await baseline(s);
  const first = outcome(await publish(s, 'B', 'B2', 18, 2, 1), 'published');
  assert.deepEqual([first.epoch, first.incarnation, first.generation], ['19', '2', '2']);
  observedRefusal(log, outcome(await publish(s, 'A', 'A2', 18, 7, 3), 'stale-reject'));
  const after = await select(s, 'A', 19, 3, 21);
  assert.equal(after.programValueId, before.programValueId);
  const bAfter = await select(s, 'B', 19, 2, 22);
  assert.notEqual(bAfter.programValueId, bBefore.programValueId);
  log.committed = first; log.after = [after, bAfter];
});
await runCase('LSLOT04-delete-recreate-ABA', async (s, log) => {
  const { before } = await baseline(s);
  const deleted = outcome(await s.issue({ operation: 'delete', slot: 'A', expected_epoch: '18',
    expected_incarnation: '7', expected_generation: '3' }), 'deleted');
  assert.equal(deleted.epoch, '19');
  await install(s, 'Ainc8');
  const recreated = outcome(await publish(s, 'A', 'Ainc8', 19), 'published');
  assert.deepEqual([recreated.epoch, recreated.incarnation, recreated.generation], ['20', '8', '1']);
  observedRefusal(log, outcome(await publish(s, 'A', 'A2', 18, 7, 3), 'stale-reject'));
  observedRefusal(log, outcome(await publish(s, 'A', 'A2', 20, 7, 3), 'stale-reject'));
  const after = await select(s, 'A', 20, 1, 21);
  assert.equal(after.incarnation, '8');
  assert.notEqual(after.programValueId, before.programValueId,
    'same visible source must be fresh installed Program identity');
  log.commits = [deleted, recreated]; log.after = after;
});
await runCase('LSLOT04-authorized-rollback', async (s, log) => {
  const { before } = await baseline(s);
  const newer = outcome(await publish(s, 'A', 'A2', 18, 7, 3), 'published');
  assert.deepEqual([newer.epoch, newer.incarnation, newer.generation], ['19', '7', '4']);
  await install(s, 'Ainc8');
  const rollback = outcome(await publish(s, 'A', 'Ainc8', 19, 7, 4, 'rollback'), 'published');
  assert.deepEqual([rollback.epoch, rollback.incarnation, rollback.generation], ['20', '7', '5']);
  const after = await select(s, 'A', 20, 5, 21);
  assert.notEqual(after.programValueId, before.programValueId,
    'freshly admitted same recipe must have a new selected Program identity');
  log.commits = [newer, rollback]; log.after = after;
});
await runCase('LSLOT04-rollback-denied', async (s, log) => {
  await baseline(s);
  outcome(await publish(s, 'A', 'A2', 18, 7, 3), 'published');
  await install(s, 'Ainc8');
  outcome(await s.issue({ operation: 'policy',
    grant: { operation: 'rollback', slotId: 'A', allowed: false } }), 'policy-updated');
  const denied = await publish(s, 'A', 'Ainc8', 19, 7, 4, 'rollback');
  assert.equal(denied.outcome, 'policy-denied', JSON.stringify(denied));
  observedRefusal(log, denied);
  const after = await select(s, 'A', 19, 4, 22);
  log.denied = denied; log.after = after;
});

async function emitBaseline(s, slot = 'emit') {
  outcome(await s.next(), 'configured');
  await install(s, 'callEmit', ['I64', 'LiveRef<I64,I64,test.emit>']);
  await install(s, 'emit1');
  await install(s, 'emit2');
  const initial = outcome(await publish(s, slot, 'emit1', 0), 'published');
  assert.deepEqual([initial.epoch, initial.incarnation, initial.generation], ['1', '1', '1']);
  return initial;
}
await runCase('LSLOT07-publication-without-grant', async (s, log) => {
  await emitBaseline(s);
  const before = await select(s, 'emit', 1, 1, 21, 'callEmit');
  outcome(await s.issue({ operation: 'policy',
    grant: { operation: 'publish', slotId: 'emit', allowed: false } }), 'policy-updated');
  const denied = await publish(s, 'emit', 'emit2', 1, 1, 1);
  assert.equal(denied.outcome, 'policy-denied', JSON.stringify(denied));
  observedRefusal(log, denied);
  const after = await select(s, 'emit', 1, 1, 21, 'callEmit');
  assert.equal(after.programValueId, before.programValueId);
  log.denied = denied; log.after = after;
});
await runCase('LSLOT07-static-dispatch-without-grant', async (s, log) => {
  await emitBaseline(s, 'emitDeny');
  const before = outcome(await s.issue({ operation: 'trace' }), 'trace-observed');
  assert.deepEqual(before.trace, []);
  const denied = outcome(await s.issue({ operation: 'invoke', id: 'callEmit',
    inputs: [input(20)], refs: [{ position: 1, ordinal: 0, slot: 'emitDeny' }] }), 'refused');
  observedRefusal(log, denied);
  const after = outcome(await s.issue({ operation: 'trace' }), 'trace-observed');
  assert.deepEqual(after.trace, before.trace, 'denied root never reaches selected guest dispatch');
  const updated = outcome(await publish(s, 'emitDeny', 'emit2', 1, 1, 1), 'published');
  assert.deepEqual([updated.epoch, updated.incarnation, updated.generation], ['2', '1', '2']);
  log.denied = denied; log.mapUnchangedBeforeNextCAS = updated;
});
await runCase('LSLOT07-dispatch-without-grant', async (s, log) => {
  await emitBaseline(s);
  const before = await select(s, 'emit', 1, 1, 21, 'callEmit');
  outcome(await s.issue({ operation: 'policy',
    grant: { operation: 'dispatch', slotId: 'emit', allowed: false } }), 'policy-updated');
  const denied = outcome(await s.issue({ operation: 'invoke', id: 'callEmit',
    inputs: [input(20)], refs: [{ position: 1, ordinal: 0, slot: 'emit' }] }), 'execution-failed');
  assert.deepEqual(denied.request_trace.map(row => [row.operation, row.outcome]),
    [['dispatch', 'policy-denied']]);
  assert.equal(denied.request_trace[0].programValueId, before.programValueId);
  assert.equal(denied.request_trace[0].rootEpoch, '1');
  observedRefusal(log, denied, 1);
  log.denied = denied;
});
await runCase('LSLOT07-old-version-effect-revoked-after-pin', async (s, log) => {
  await emitBaseline(s);
  outcome(await s.issue({ operation: 'hold-checkpoint', import: 'test_emit', occurrence: 1 }),
    'checkpoint-hold-selected');
  s.child.stdin.write(JSON.stringify({ operation: 'invoke', id: 'callEmit',
    inputs: [input(20)], refs: [{ position: 1, ordinal: 0, slot: 'emit' }] }) + '\n');
  const entered = outcome(await s.next(), 'checkpoint-entered');
  assert.equal(entered.import, 'test_emit');
  s.child.stdin.write(JSON.stringify({ operation: 'policy',
    grant: { operation: 'effect', slotId: 'emit', allowed: false } }) + '\n');
  const committed = outcome(await s.next(), 'policy-updated');
  assert.equal(committed.control_id, '1');
  assert.equal(committed.root, entered.root);
  assert.equal(committed.checkpoint, entered.checkpoint);
  s.child.stdin.write(JSON.stringify({ operation: 'resume-checkpoint' }) + '\n');
  const denied = outcome(await s.next(), 'execution-failed');
  assert.deepEqual(denied.request_trace.map(row => [row.operation, row.outcome]),
    [['dispatch', 'allowed'], ['effect', 'denied']]);
  const [dispatch, effect] = denied.request_trace;
  assert.equal(dispatch.rootEpoch, '1');
  assert.equal(dispatch.incarnation, '1');
  assert.equal(dispatch.generation, '1');
  assert.equal(effect.request, 'touched');
  assert.equal(effect.protectedOperations, 0);
  assert.deepEqual(denied.control_events.map(row => [row.control_id, row.outcome]),
    [['1', 'policy-updated']]);
  observedRefusal(log, denied, 2);
  log.checkpoint = entered; log.ack = committed; log.denied = denied;
});
await runCase('LSLOT07-revoked-after-root-pin', async (s, log) => {
  await emitBaseline(s);
  outcome(await s.issue({ operation: 'hold-checkpoint', import: 'live_select', occurrence: 1 }),
    'checkpoint-hold-selected');
  s.child.stdin.write(JSON.stringify({ operation: 'invoke', id: 'callEmit',
    inputs: [input(20)], refs: [{ position: 1, ordinal: 0, slot: 'emit' }] }) + '\n');
  const entered = outcome(await s.next(), 'checkpoint-entered');
  assert.equal(entered.import, 'live_select');
  assert.equal(entered.ordinal, 0);
  assert.ok(Number.isInteger(entered.site), JSON.stringify(entered));
  s.child.stdin.write(JSON.stringify({ operation: 'policy',
    grant: { operation: 'dispatch', slotId: 'emit', allowed: false } }) + '\n');
  const committed = outcome(await s.next(), 'policy-updated');
  assert.equal(committed.control_id, '1');
  assert.equal(committed.root, entered.root);
  assert.equal(committed.checkpoint, entered.checkpoint);
  s.child.stdin.write(JSON.stringify({ operation: 'resume-checkpoint' }) + '\n');
  const denied = outcome(await s.next(), 'execution-failed');
  assert.equal(denied.epoch, undefined);
  assert.equal(denied.protected_operations, 0);
  assert.equal(denied.guest_requests, 1,
    'denied dispatch itself is the sole guest request; target/effect body cannot run');
  assert.deepEqual(denied.request_trace.map(row => [row.operation, row.outcome]),
    [['dispatch', 'policy-denied']]);
  assert.equal(denied.request_trace[0].rootEpoch, '1');
  assert.equal(denied.request_trace[0].incarnation, '1');
  assert.equal(denied.request_trace[0].generation, '1');
  assert.equal(denied.request_trace[0].policyAllowed, false);
  assert.deepEqual(denied.control_events.map(event => [event.control_id, event.outcome]),
    [['1', 'policy-updated']]);
  log.checkpoint = entered; log.ack = committed; log.denied = denied;
  observedRefusal(log, denied, 1);
});
if (evidenceDir) console.log(`CASE_ARTIFACT_DIRECTORY=${artifactDir}`);
