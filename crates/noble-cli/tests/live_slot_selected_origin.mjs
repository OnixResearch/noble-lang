import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { cli, evidenceDirectory, expectOutcome, selectedAuthority } from './live_slot_resource_ref_cli.mjs';

const binary = process.argv[2];
assert.ok(binary, 'pass the selected noble CLI binary');
const definitions = [
  ['variable', 'builder', 'def builder [ quote [ + ] compose ]'],
  ['renamed', 'renamed', 'def renamed [ quote [ + ] compose ]'],
  ['fixed', 'fixed', 'def fixed [ 2 quote [ + ] compose ]'],
  ['fixed3', 'fixed3', 'def fixed3 [ 3 quote [ + ] compose ]'],
  ['computed', 'computed', 'def computed [ 1 1 + quote [ + ] compose ]'],
  ['computed3', 'computed3', 'def computed3 [ 1 2 + quote [ + ] compose ]'],
  ['ambiguous', 'ambiguous', 'def ambiguous [ 1 + quote [ + ] compose ]'],
  ['dual', 'dual', 'def dual [ quote swap quote ]'],
  ['copy', 'copy', 'def copy [ dup quote swap quote ]'],
];
const sources = Object.fromEntries(definitions.flatMap(([id, name, definition]) =>
  [[id, name], [`definition-${id}`, definition]]));
Object.assign(sources, { runner: 'run', misbound: 'fixed', misdigest: 'fixed' });
const slots = ['ordinary-control', 'proof-required'].map(slotId => ({
  slotId, input: ['I64'], output: ['I64'], effectCeiling: [],
  proofRequired: slotId === 'proof-required',
}));
const authority = selectedAuthority({ sources, slots, effects: [], quota: 24,
  grants: slots.map(slot => ({ operation: 'publish', slotId: slot.slotId, allowed: true })) });
const session = await cli(binary, 'LSLOT01-selected-origin-control', authority);
const replies = [];
const issue = async (request, outcome) => {
  const row = expectOutcome(await session.issue(request), outcome);
  replies.push(row);
  if (row.guest_requests !== undefined) assert.equal(row.guest_requests, 0, JSON.stringify(row));
  if (row.protected_operations !== undefined)
    assert.equal(row.protected_operations, 0, JSON.stringify(row));
  return row;
};
const input = value => ({ kind: 'i64', value: String(value) });
const invoke = (id, inputs) => issue({ operation: 'invoke', id, inputs, refs: [] }, 'executed');
const run = async (owner, value, outcome = 'executed') => issue({ operation: 'invoke', id: 'runner',
  inputs: [input(value), { kind: 'program', owner }], refs: [] }, outcome);
const owners = new Set();
function saved(row, position, id, installation, status = 'checked-selected-origin') {
  const program = row.stack[position];
  assert.equal(program.kind, 4);
  assert.equal(program.source_id, null);
  assert.equal(program.program_index, null);
  assert.equal(program.capture_status, status);
  assert.ok(program.owner && !owners.has(program.owner), 'each output owns its independent Program');
  owners.add(program.owner);
  if (status === 'checked-selected-origin') {
    assert.deepEqual(program.verified_origin, {
      artifact_sha256: installation.artifact_sha256, caller_id: id,
      definition_identity: program.verified_origin.definition_identity,
      source_generation: program.verified_origin.source_generation,
      stack_position: position,
    });
    assert.match(program.verified_origin.definition_identity, /^[0-9]+$/);
    assert.match(program.verified_origin.source_generation, /^[0-9]+$/);
  } else assert.equal(program.verified_origin, undefined);
  return program;
}
// A composed Program captures an actual graph; inspect its quoted I64 leaf rather
// than mistaking the non-scalar graph for a direct-quote capture_value.
function composedCapture(program) {
  const cells = program.capture_values.find(entry => entry.field === 'cell_a')?.value;
  assert.ok(Array.isArray(cells));
  const scalars = cells.filter(cell => cell.kind === 1);
  assert.equal(scalars.length, 1);
  return scalars[0].payload;
}
let complete = false;
try {
  expectOutcome(await session.configured, 'configured');
  for (const [id, name, definition] of definitions)
    await issue({ operation: 'define', id: `definition-${id}`, name, source: definition, inputs: [] }, 'definition-retained');
  const installations = {};
  for (const [id, name] of definitions)
    installations[id] = await issue({ operation: 'install', id, source: name,
      selected_name: name, inputs: id === 'dual' ? ['I64', 'I64'] : ['I64'] }, 'installed');
  await issue({ operation: 'install', id: 'misbound', source: 'fixed',
    selected_name: 'builder', inputs: ['I64'] }, 'refused');
  await issue({ operation: 'install', id: 'misdigest', source: 'builder',
    selected_name: 'builder', inputs: ['I64'] }, 'refused');
  installations.runner = await issue({ operation: 'install', id: 'runner', source: 'run',
    inputs: ['I64', 'Program<I64,I64,pure>'] }, 'installed');

  const varying2 = saved(await invoke('variable', [input(2)]), 0, 'variable', installations.variable);
  assert.equal(composedCapture(varying2), '2');
  assert.ok(varying2.target_identity?.definition_id, 'checked anonymous target D is absent');
  assert.deepEqual((await run(varying2.owner, 1)).stack, [{ kind: 1, value: '3' }]);
  const varying3 = saved(await invoke('variable', [input(3)]), 0, 'variable', installations.variable);
  assert.equal(composedCapture(varying3), '3');
  assert.equal(varying2.target_identity.definition_id, varying3.target_identity.definition_id);
  assert.notEqual(varying2.target_identity.program_value_id, varying3.target_identity.program_value_id);
  assert.notEqual(varying2.target_identity.definition_id, varying2.verified_origin.definition_identity);
  assert.notEqual(varying2.target_identity.program_value_id, varying2.verified_origin.definition_identity);
  assert.deepEqual(varying2.target_identity.captures, [{ type: 'I64', value: '2' }]);
  assert.deepEqual(varying3.target_identity.captures, [{ type: 'I64', value: '3' }]);
  assert.deepEqual(varying2.target_identity.interface, {
    input: ['I64'], output: ['I64'], effects: [],
  });
  assert.equal(varying2.target_identity.artifact_sha256, null);
  const renamed = saved(await invoke('renamed', [input(2)]), 0, 'renamed', installations.renamed);
  assert.notEqual(renamed.verified_origin.source_generation,
    varying2.verified_origin.source_generation);
  assert.equal(renamed.target_identity.definition_id, varying2.target_identity.definition_id);
  assert.equal(renamed.target_identity.program_value_id, varying2.target_identity.program_value_id);
  const zero = saved(await invoke('variable', [input(0)]), 0, 'variable', installations.variable);
  const negativeZero = saved(await invoke('variable', [input('-0')]), 0, 'variable', installations.variable);
  assert.equal(composedCapture(negativeZero), '0');
  assert.equal(negativeZero.target_identity.program_value_id, zero.target_identity.program_value_id);
  const noArtifact = await issue({ operation: 'publish', slot: 'ordinary-control',
    owner: varying2.owner, expected_epoch: '0' }, 'refused');
  assert.match(noArtifact.diagnostic, /dynamic saved Program lacks independently checked installed target metadata/);
  const noProof = await issue({ operation: 'publish', slot: 'proof-required',
    owner: varying2.owner, expected_epoch: '0' }, 'refused');
  assert.match(noProof.diagnostic, /proof-required slot lacks independently checked target evidence/);
  assert.deepEqual((await run(varying3.owner, 1)).stack, [{ kind: 1, value: '4' }]);
  const fixed = {};
  for (const id of ['fixed', 'computed', 'fixed3', 'computed3']) {
    const row = await invoke(id, [input(3)]);
    assert.deepEqual(row.stack[0], { kind: 1, value: '3' });
    const program = saved(row, 1, id, installations[id]);
    const literal = id.endsWith('3') ? 3 : 2;
    assert.equal(composedCapture(program), String(literal));
    assert.deepEqual(program.target_identity.captures, [{ type: 'I64', value: String(literal) }]);
    assert.deepEqual((await run(program.owner, 1)).stack, [{ kind: 1, value: String(literal + 1) }]);
    fixed[id] = program.target_identity;
  }
  assert.notEqual(fixed.fixed.definition_id, fixed.fixed3.definition_id);
  assert.notEqual(fixed.computed.definition_id, fixed.computed3.definition_id);
  assert.notEqual(fixed.fixed.definition_id, varying2.target_identity.definition_id);
  assert.notEqual(fixed.fixed3.definition_id, varying3.target_identity.definition_id);
  for (const id of ['fixed', 'computed']) {
    const row = await invoke(id, [input(2)]);
    const sameFixed = saved(row, 1, id, installations[id]);
    assert.equal(composedCapture(sameFixed), '2');
    assert.equal(sameFixed.target_identity.definition_id, fixed[id].definition_id);
    assert.equal(sameFixed.target_identity.program_value_id, fixed[id].program_value_id);
  }
  const dual = await invoke('dual', [input(2), input(3)]);
  const first = saved(dual, 0, 'dual', installations.dual);
  const second = saved(dual, 1, 'dual', installations.dual);
  assert.deepEqual([first.capture_values, second.capture_values],
    [[{ type: 'I64', value: '3' }], [{ type: 'I64', value: '2' }]]);
  assert.equal(first.target_identity, undefined);
  assert.equal(second.target_identity, undefined);
  const copy = await invoke('copy', [input(7)]);
  assert.deepEqual([saved(copy, 0, 'copy', installations.copy).capture_values,
    saved(copy, 1, 'copy', installations.copy).capture_values],
  [[{ type: 'I64', value: '7' }], [{ type: 'I64', value: '7' }]]);
  assert.equal(copy.stack[0].target_identity, undefined);
  assert.equal(copy.stack[1].target_identity, undefined);
  const opaque = saved(await invoke('ambiguous', [input(2)]), 0, 'ambiguous',
    installations.ambiguous, 'backend-observed-source-occurrence-unavailable');
  assert.equal(composedCapture(opaque), '3');
  assert.equal(opaque.target_identity, undefined);
  assert.deepEqual((await run(opaque.owner, 1)).stack, [{ kind: 1, value: '4' }]);
  await run('program-999999', 1, 'refused');
  for (const owner of owners)
    await issue({ operation: 'release-program', owner }, 'program-released');
  await run(varying2.owner, 1, 'refused');
  const exit = await session.close();
  assert.equal(exit, 0);
  const records = fs.readFileSync(session.log, 'utf8').trim().split('\n').map(JSON.parse);
  assert.equal(records[0].kind, 'authority');
  assert.equal(records[0].selected_binary_sha256, session.selectedBinarySha256);
  assert.deepEqual(records[0].authority, authority);
  assert.deepEqual(records.filter(record => record.kind === 'operator').map(record => record.request),
    fs.readFileSync(session.requests, 'utf8').trim().split('\n').map(JSON.parse));
  assert.deepEqual(records.filter(record => record.kind === 'cli').map(record => record.row),
    fs.readFileSync(session.responses, 'utf8').trim().split('\n').map(JSON.parse));
  assert.equal(records.at(-1).kind, 'exit');
  assert.equal(records.at(-1).code, 0);
  fs.writeFileSync(path.join(evidenceDirectory, 'LSLOT01-selected-origin-control.summary.json'),
    JSON.stringify({ selected_binary_sha256: session.selectedBinarySha256,
      owner_count: owners.size, installed_artifacts: Object.fromEntries(
        Object.entries(installations).map(([id, row]) => [id, row.artifact_sha256])),
      guest_requests: replies.reduce((n, row) => n + (row.guest_requests ?? 0), 0),
      protected_operations: replies.reduce((n, row) => n + (row.protected_operations ?? 0), 0) }, null, 2) + '\n',
    { flag: 'wx', mode: 0o600 });
  complete = true;
  console.log('selected source origin and experimental anonymous D/P: varying, fixed, computed, rename, opaque/alias refusal, no artifact/proof and owner lifecycle');
} finally {
  if (!complete) await session.abort();
}
