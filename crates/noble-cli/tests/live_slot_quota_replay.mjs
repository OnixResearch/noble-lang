import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const binary = process.argv[2];
assert.ok(binary, 'pass the selected noble CLI binary');
const authority = fileURLToPath(new URL('./live_slot_quota_replay_authority.json', import.meta.url));
const fixture = JSON.parse(fs.readFileSync(new URL('../../../specs/conformance/live-reference-cases.json', import.meta.url)));
const case08 = fixture.cases.find(row => row.id === 'LSLOT-08');
assert.ok(case08, 'canonical LSLOT-08 must remain available');
const variants = case08.input.replay_fixture.variants;
const requestedEvidence = process.env.NOBLE_SLOT_EVIDENCE_DIR;
if (requestedEvidence && !path.isAbsolute(requestedEvidence)) {
  throw Error('selected evidence directory must be absolute');
}
const evidenceDir = requestedEvidence ?? fs.mkdtempSync(path.join(os.tmpdir(), 'noble-lslot-quota-'));
if (requestedEvidence) fs.mkdirSync(evidenceDir, { recursive: true, mode: 0o700 });
const selectedSource = Object.freeze({
  caller: 'slot.invoke', keeper: '[ 1 + ]', increment: '2 +', use_saved: 'run',
  definition: 'def traced [ drop "A" test.emit drop "B" test.emit drop 42 ]', named: 'traced',
});
const reportPath = name => path.join(evidenceDir, `${name}.raw.jsonl`);
const selected = (row, outcome) => {
  assert.equal(row.outcome, outcome, JSON.stringify(row));
  return row;
};
const counted = (row, requests = 0, operations = 0) => {
  assert.equal(row.guest_requests, requests, JSON.stringify(row));
  assert.equal(row.protected_operations, operations, JSON.stringify(row));
  return row;
};
const scalar = value => [{ kind: 'i64', value: String(value) }];
const ref = slot => [{ position: 1, ordinal: 0, slot }];

function start(name) {
  const output = reportPath(name);
  const commands = path.join(evidenceDir, `${name}.requests.jsonl`);
  const stderrFile = path.join(evidenceDir, `${name}.stderr.txt`);
  for (const file of [output, commands]) fs.closeSync(fs.openSync(file, 'wx', 0o600));
  const child = spawn('/bin/bash', [
    '-c', 'cat | exec "$1" live slot --engine v8 --authority "$2"',
    'sh', binary, authority,
  ], { stdio: ['pipe', 'pipe', 'pipe'] });
  let buffer = '', stderr = '';
  const rows = [], waiting = [];
  let closed = false;
  child.stdout.on('data', bytes => {
    fs.appendFileSync(output, bytes);
    buffer += bytes.toString();
    for (let end; (end = buffer.indexOf('\n')) >= 0;) {
      const row = JSON.parse(buffer.slice(0, end));
      buffer = buffer.slice(end + 1);
      const next = waiting.shift();
      next ? next.resolve(row) : rows.push(row);
    }
  });
  child.stderr.on('data', bytes => {
    stderr += bytes.toString();
    fs.appendFileSync(stderrFile, bytes, { mode: 0o600 });
  });
  child.on('close', () => {
    closed = true;
    for (const next of waiting.splice(0)) next.reject(Error(`CLI ended before receipt: ${stderr}`));
  });
  const next = () => rows.length ? Promise.resolve(rows.shift()) : new Promise((resolve, reject) => {
    if (closed) return reject(Error(`CLI already exited: ${stderr}`));
    const timeout = setTimeout(() => reject(Error(`CLI receipt timeout (${name}): ${stderr}`)), 15000);
    waiting.push({ resolve: row => { clearTimeout(timeout); resolve(row); }, reject: error => {
      clearTimeout(timeout); reject(error);
    } });
  });
  const write = request => {
    const line = JSON.stringify(request) + '\n';
    fs.appendFileSync(commands, line);
    child.stdin.write(line);
  };
  return {
    output, commands, stderrFile, next, write,
    issue: request => { write(request); return next(); },
    finish: async () => {
      child.stdin.end();
      const code = child.exitCode === null ? await new Promise(resolve => child.once('close', resolve)) : child.exitCode;
      assert.equal(code, 0, `${name}: ${stderr}`);
      assert.equal(buffer, '', `${name}: incomplete JSONL receipt`);
      assert.equal(rows.length, 0, `${name}: unexpected extra CLI receipts`);
    },
    abort: () => child.kill(),
  };
}

const reflect = async session => {
  const row = counted(selected(await session.issue({ operation: 'reflect', id: 'caller', site_id: 0 }), 'reflected'));
  const site = { instruction: 'slot.invoke', siteId: 0, input: ['I64'], output: ['I64'], effects: ['live.dispatch'] };
  assert.deepEqual(row.site, site, 'generic reflection must not select or expose a slot/credential');
  return row.site;
};

async function retention() {
  const session = start('retention');
  try {
    counted(selected(await session.next(), 'configured'));
    counted(selected(await session.issue({ operation: 'install', id: 'caller',
      source: selectedSource.caller, inputs: ['I64', 'LiveRef<I64,I64,pure>'] }), 'installed'));
    const generic = await reflect(session); // Before any slot exists.
    const keeper = counted(selected(await session.issue({ operation: 'install', id: 'keeper',
      source: selectedSource.keeper, inputs: [] }), 'installed'));
    const initial = counted(selected(await session.issue({ operation: 'publish', slot: 'Q',
      id: 'keeper', program_index: 1, expected_epoch: '0' }), 'published'));
    assert.deepEqual([initial.epoch, initial.incarnation, initial.generation], ['1', '1', '1']);
    const saved = counted(selected(await session.issue({ operation: 'invoke', id: 'keeper',
      inputs: [], refs: [] }), 'executed'));
    assert.equal(saved.epoch, '1');
    const [{ owner, source_id: sourceId, program_index: index }] = saved.stack;
    assert.equal(sourceId, 'keeper');
    assert.equal(index, 1);
    assert.match(owner, /^program-[1-9][0-9]*$/);
    assert.deepEqual(await reflect(session), generic);
    counted(selected(await session.issue({ operation: 'install', id: 'use_saved',
      source: selectedSource.use_saved, inputs: ['I64', 'Program<I64,I64,pure>'] }), 'installed'));
    counted(selected(await session.issue({ operation: 'install', id: 'increment',
      source: selectedSource.increment, inputs: ['I64'] }), 'installed'));
    counted(selected(await session.issue({ operation: 'candidate', id: 'increment' }), 'candidate-staged'));
    const attempt = { operation: 'publish', slot: 'Q', id: 'increment', program_index: 0,
      expected_epoch: '1', expected_incarnation: '1', expected_generation: '1' };
    selected(await session.issue({ operation: 'hold-checkpoint', import: 'live_select', occurrence: 1 }),
      'checkpoint-hold-selected');
    session.write({ operation: 'invoke', id: 'caller', inputs: scalar(5), refs: ref('Q') });
    selected(await session.next(), 'checkpoint-entered');
    const refusedWhilePinned = selected(await session.issue(attempt), 'retention-budget-refused');
    assert.equal(refusedWhilePinned.epoch, '1');
    const oldRoot = counted(selected(await session.issue({ operation: 'resume-checkpoint' }), 'executed'), 1);
    assert.deepEqual(oldRoot.stack, [{ kind: 1, value: '6' }]);
    assert.equal(oldRoot.epoch, '1');
    assert.equal(oldRoot.request_trace[0].generation, '1');
    assert.equal(oldRoot.request_trace[0].artifactSha256, keeper.artifact_sha256);
    assert.deepEqual(oldRoot.retire_code_spans, []);
    assert.deepEqual(await reflect(session), generic);
    const refusedAfterRoot = counted(selected(await session.issue(attempt), 'retention-budget-refused'));
    assert.equal(refusedAfterRoot.epoch, '1');
    assert.deepEqual(refusedAfterRoot.retire_code_spans, []);
    assert.deepEqual(await reflect(session), generic);
    const stillCallable = counted(selected(await session.issue({ operation: 'invoke', id: 'use_saved',
      inputs: [...scalar(10), { kind: 'program', owner }], refs: [] }), 'executed'));
    assert.deepEqual(stillCallable.stack, [{ kind: 1, value: '11' }]);
    assert.equal(stillCallable.epoch, '1');
    counted(selected(await session.issue({ operation: 'release-program', owner }), 'program-released'));
    assert.deepEqual(await reflect(session), generic);
    const published = counted(selected(await session.issue(attempt), 'published'));
    assert.deepEqual([published.epoch, published.incarnation, published.generation], ['2', '1', '2']);
    assert.ok(published.retire_code_spans.some(span => span.generation === Number(keeper.generation)),
      'last owner release must permit physical v1 code-span retirement on publication');
    assert.deepEqual(await reflect(session), generic);
    const newRoot = counted(selected(await session.issue({ operation: 'invoke', id: 'caller',
      inputs: scalar(5), refs: ref('Q') }), 'executed'), 1);
    assert.deepEqual(newRoot.stack, [{ kind: 1, value: '7' }]);
    assert.equal(newRoot.epoch, '2');
    assert.equal(newRoot.request_trace[0].generation, '2');
    assert.notEqual(newRoot.request_trace[0].programValueId, oldRoot.request_trace[0].programValueId);
    assert.deepEqual(await reflect(session), generic);
    await session.finish();
    return { owner, oldEpoch: oldRoot.epoch, newEpoch: newRoot.epoch,
      retired: published.retire_code_spans,
      ...(requestedEvidence ? { raw: session.output, requests: session.commands } : {}) };
  } catch (error) { session.abort(); throw error; }
}

function mutatedRequest(variant, recorded, inputs, refs) {
  const request = { operation: 'replay', token: recorded.replay_token, inputs, refs,
    expected_trace: structuredClone(recorded.request_trace),
    expected_stack: structuredClone(recorded.stack),
    expected_identity: structuredClone(recorded.frozen_identity) };
  switch (variant.name) {
    case 'bare-slot-id': delete request.token; break;
    case 'bare-LiveRef': request.expected_identity.slots = []; break;
    case 'wrong-registry-epoch': request.expected_trace[0].rootEpoch = String(variant.epoch); break;
    case 'wrong-slot-incarnation': request.expected_identity.slots[0].incarnation = String(variant.incarnation); break;
    case 'wrong-slot-generation': request.expected_identity.slots[0].generation = String(variant.generation); break;
    case 'substituted-selected-artifact': request.expected_identity.slots[0].artifactSha256 = '0'.repeat(64); break;
    case 'substituted-captures': request.expected_identity.slots[0].captures = variant.captures; break;
    case 'wrong-semantic-context': request.expected_identity.slots[0].semanticContext = variant.semantic_context; break;
    case 'reordered-host-requests': [request.expected_trace[1], request.expected_trace[2]] =
      [request.expected_trace[2], request.expected_trace[1]]; break;
    case 'substituted-host-response': request.expected_trace[1].response = variant.response_for_first_request; break;
    case 'wrong-host-accounting': request.expected_trace[2].protectedOperations = variant.protected_operations; break;
    case 'exact-frozen-admitted-and-ordered': break;
    default: throw Error(`unexpected canonical replay variant: ${variant.name}`);
  }
  return request;
}

async function replayVariant(variant) {
  const session = start(`replay-${variant.name}`);
  try {
    counted(selected(await session.next(), 'configured'));
    counted(selected(await session.issue({ operation: 'install', id: 'caller',
      source: selectedSource.caller, inputs: ['I64', 'LiveRef<I64,I64,test.emit>'] }), 'installed'));
    selected(await session.issue({ operation: 'define', id: 'definition', name: 'traced',
      source: selectedSource.definition, inputs: [] }), 'definition-retained');
    counted(selected(await session.issue({ operation: 'install', id: 'named',
      source: selectedSource.named, inputs: ['I64'], selected_name: 'traced' }), 'installed'));
    let selectedId = 'named';
    let published = counted(selected(await session.issue({ operation: 'publish', slot: 'emit',
      id: 'named', program_index: 0, expected_epoch: '0' }), 'published'));
    assert.deepEqual([published.epoch, published.incarnation, published.generation], ['1', '1', '1']);
    let epoch = 1;
    for (let incarnation = 1; incarnation <= 3; incarnation++) {
      const deleted = counted(selected(await session.issue({ operation: 'delete', slot: 'emit',
        expected_epoch: String(epoch), expected_incarnation: String(incarnation),
        expected_generation: '1' }), 'deleted'));
      assert.equal(deleted.epoch, String(++epoch));
      selectedId = `named${incarnation + 1}`;
      counted(selected(await session.issue({ operation: 'install', id: selectedId,
        source: selectedSource.named, inputs: ['I64'], selected_name: 'traced' }), 'installed'));
      published = counted(selected(await session.issue({ operation: 'publish', slot: 'emit',
        id: selectedId, program_index: 0, expected_epoch: String(epoch) }), 'published'));
      assert.deepEqual([published.epoch, published.incarnation, published.generation],
        [String(++epoch), String(incarnation + 1), '1']);
    }
    for (let generation = 2; generation <= 3; generation++) {
      published = counted(selected(await session.issue({ operation: 'publish', slot: 'emit',
        id: selectedId, program_index: 0, expected_epoch: String(epoch),
        expected_incarnation: '4', expected_generation: String(generation - 1) }), 'published'));
      assert.deepEqual([published.epoch, published.incarnation, published.generation],
        [String(++epoch), '4', String(generation)]);
    }
    assert.equal(epoch, 9, 'frozen replay must start at reachable incarnation 4/generation 3');
    const inputs = scalar(5), refs = ref('emit');
    const recorded = counted(selected(await session.issue({ operation: 'invoke', id: 'caller',
      inputs, refs, record: true }), 'executed'), 3, 2);
    assert.equal(recorded.epoch, '9');
    assert.match(recorded.replay_token, /^replay-[1-9][0-9]*$/);
    assert.deepEqual(recorded.stack, [{ kind: 1, value: '42' }]);
    assert.deepEqual(recorded.request_trace.map(row => row.operation), ['dispatch', 'effect', 'effect']);
    assert.deepEqual(recorded.request_trace.slice(1).map(row => row.request), ['A', 'B']);
    assert.deepEqual(recorded.request_trace.slice(1).map(row => row.response), ['emitted:A', 'emitted:B']);
    assert.deepEqual(recorded.request_trace.slice(1).map(row => row.protectedOperations), [1, 2]);
    assert.equal(recorded.request_trace[0].rootEpoch, '9');
    assert.ok(recorded.request_trace[0].definitionId, 'named selected target must have checked DefinitionId');
    assert.equal(recorded.frozen_identity.slots[0].definitionId, recorded.request_trace[0].definitionId);
    counted(selected(await session.issue({ operation: 'policy',
      grant: { operation: 'effect', slotId: 'emit', allowed: false } }), 'policy-updated'));
    const outcome = await session.issue(mutatedRequest(variant, recorded, inputs, refs));
    if (variant.name === 'exact-frozen-admitted-and-ordered') {
      counted(selected(outcome, 'replay-matched'), 3, 0);
      assert.equal(outcome.scripted_operations, 2);
      assert.deepEqual(outcome.actual_trace, recorded.request_trace);
      assert.deepEqual(outcome.actual_stack, recorded.stack);
    } else {
      counted(selected(outcome, variant.name === 'bare-slot-id' ? 'refused' : 'replay-diverged'));
      assert.equal(outcome.actual, undefined, 'mismatched receipt must refuse before guest execution');
    }
    await session.finish();
    return { name: variant.name, outcome: outcome.outcome,
      guest_requests: outcome.guest_requests, protected_operations: outcome.protected_operations,
      scripted_operations: outcome.scripted_operations ?? null,
      ...(requestedEvidence ? { raw: session.output, requests: session.commands } : {}) };
  } catch (error) { session.abort(); throw error; }
}

async function traceCapacity() {
  const session = start('bounded-trace-retention');
  try {
    counted(selected(await session.next(), 'configured'));
    counted(selected(await session.issue({ operation: 'install', id: 'caller',
      source: selectedSource.caller,
      inputs: ['I64', 'LiveRef<I64,I64,pure>'] }), 'installed'));
    counted(selected(await session.issue({ operation: 'install', id: 'keeper',
      source: selectedSource.keeper, inputs: [] }), 'installed'));
    counted(selected(await session.issue({ operation: 'publish', slot: 'Q',
      id: 'keeper', program_index: 1, expected_epoch: '0' }), 'published'));
    const invoke = { operation: 'invoke', id: 'caller', inputs: scalar(5), refs: ref('Q') };
    let completed = 0;
    for (; completed < 2048; completed++) {
      const result = await session.issue(invoke);
      if (result.outcome === 'trace-capacity-refused') {
        counted(result);
        break;
      }
      counted(selected(result, 'executed'), 1);
      assert.deepEqual(result.stack, [{ kind: 1, value: '6' }]);
      assert.equal(result.request_trace[0].operation, 'dispatch');
    }
    assert.ok(completed >= 128 && completed < 2048,
      'repeated valid dispatches must reach bounded retention before the frame cap');
    const trace = counted(selected(await session.issue({ operation: 'trace' }), 'trace-observed')).trace;
    assert.equal(trace.length, completed, 'trace must retain every successful dispatch');
    assert.equal(trace[0].operation, 'dispatch');
    const before = JSON.stringify(trace);
    assert.ok(Buffer.byteLength(before) <= 2 * 1024 * 1024);
    counted(selected(await session.issue(invoke), 'trace-capacity-refused'));
    assert.equal(JSON.stringify((await session.issue({ operation: 'trace' })).trace), before,
      'refusal must preserve the full exact historical trace without appending or truncating');
    await session.finish();
    return { completed, retained_rows: trace.length, retained_utf8_bytes: Buffer.byteLength(before) };
  } catch (error) { session.abort(); throw error; }
}

const failures = [];
let retained = null, boundedTrace = null;
try { retained = await retention(); }
catch (error) { failures.push({ name: 'retention', error: String(error.stack ?? error) }); }
const replayed = [];
for (const variant of variants) {
  try { replayed.push(await replayVariant(variant)); }
  catch (error) {
    const failure = { name: variant.name, status: 'failed',
      error: String(error.stack ?? error),
      ...(requestedEvidence ? {
        raw: reportPath(`replay-${variant.name}`),
        requests: path.join(evidenceDir, `replay-${variant.name}.requests.jsonl`),
      } : {}) };
    failures.push(failure);
    replayed.push(failure);
  }
}
try { boundedTrace = await traceCapacity(); }
catch (error) { failures.push({ name: 'bounded-trace-retention', error: String(error.stack ?? error) }); }
console.log(JSON.stringify({ retention: retained, replay: replayed, boundedTrace, failures }));
if (failures.length) {
  console.error(`quota/replay failures preserved: ${evidenceDir}`);
  process.exitCode = 1;
} else if (!requestedEvidence) fs.rmSync(evidenceDir, { recursive: true });
