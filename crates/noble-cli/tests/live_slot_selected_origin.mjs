import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { cli, digest, evidenceDirectory, expectOutcome, selectedAuthority } from './live_slot_resource_ref_cli.mjs';

const binary = process.argv[2];
assert.ok(binary, 'pass the selected noble CLI binary');
const definitions = [
  ['variable', 'builder', 'def builder [ quote [ + ] compose ]'],
  ['subtract', 'subtract', 'def subtract [ quote [ - ] compose ]'],
  ['renamed', 'renamed', 'def renamed [ quote [ + ] compose ]'],
  ['fixed', 'fixed', 'def fixed [ 2 quote [ + ] compose ]'],
  ['fixed3', 'fixed3', 'def fixed3 [ 3 quote [ + ] compose ]'],
  ['computed', 'computed', 'def computed [ 1 1 + quote [ + ] compose ]'],
  ['computed3', 'computed3', 'def computed3 [ 1 2 + quote [ + ] compose ]'],
  ['ambiguous', 'ambiguous', 'def ambiguous [ 1 + quote [ + ] compose ]'],
  ['dual', 'dual', 'def dual [ quote swap quote ]'],
  ['copy', 'copy', 'def copy [ dup quote swap quote ]'],
  ['aliased', 'aliased', 'def aliased [ drop dup quote swap quote compose [ + + + ] compose ]'],
  ['renamed-alias', 'otheralias', 'def otheralias [ swap swap drop dup quote swap quote compose [ + + + ] compose ]'],
  ['independent', 'independent', 'def independent [ quote swap quote compose [ + + + ] compose ]'],
  ['fixed-pair', 'fixedpair', 'def fixedpair [ drop 2 quote 2 quote compose [ + + + ] compose ]'],
  ['shared-program', 'sharedprogram', 'def sharedprogram [ quote [ + ] compose let q q q compose [ 0 + ] compose ]'],
  ['repeat-root', 'repeatroot', 'def repeatroot [ dup quote [ + ] compose swap quote [ + ] compose compose [ 0 + ] compose ]'],
  ['nested-program', 'nestedprogram', 'def nestedprogram [ quote let q q quote [ run + ] compose ]'],
];
const sources = Object.fromEntries(definitions.flatMap(([id, name, definition]) =>
  [[id, name], [`definition-${id}`, definition]]));
Object.assign(sources, { runner: 'run', runner2: 'run',
  generic: 'slot.invoke', misbound: 'fixed', misdigest: 'fixed' });
const slots = ['ordinary-control', 'proof-required', 'alias-target',
  'alias-proof-required'].map(slotId => ({
  slotId, input: slotId.startsWith('alias-') ? ['I64', 'I64'] : ['I64'],
  output: ['I64'], effectCeiling: [],
  proofRequired: slotId.endsWith('proof-required'),
}));
const authority = selectedAuthority({ sources, slots, effects: ['live.dispatch'], quota: 40,
  grants: [...slots.map(slot => ({ operation: 'publish', slotId: slot.slotId, allowed: true })),
    { operation: 'dispatch', slotId: 'alias-target', allowed: true },
    { operation: 'discard', slotId: 'variable', allowed: true },
    { operation: 'discard', slotId: 'aliased', allowed: true }] });
const session = await cli(binary, 'LSLOT01-selected-origin-control', authority);
const replies = [];
const issue = async (request, outcome, guestRequests = 0) => {
  const row = expectOutcome(await session.issue(request), outcome);
  replies.push(row);
  if (row.guest_requests !== undefined)
    assert.equal(row.guest_requests, guestRequests, JSON.stringify(row));
  if (row.protected_operations !== undefined)
    assert.equal(row.protected_operations, 0, JSON.stringify(row));
  return row;
};
const input = value => ({ kind: 'i64', value: String(value) });
const invoke = (id, inputs) => issue({ operation: 'invoke', id, inputs, refs: [] }, 'executed');
const run = async (owner, value, outcome = 'executed') => issue({ operation: 'invoke', id: 'runner',
  inputs: [input(value), { kind: 'program', owner }], refs: [] }, outcome);
const run2 = async (owner, left, right) => issue({ operation: 'invoke', id: 'runner2',
  inputs: [input(left), input(right), { kind: 'program', owner }], refs: [] }, 'executed');
const owners = new Set();
function manifestDigest(descriptor) {
  const canonical = JSON.stringify(descriptor, (_, value) =>
    value !== null && typeof value === 'object' && !Array.isArray(value)
      ? Object.fromEntries(Object.keys(value).sort().map(key => [key, value[key]]))
      : value);
  return digest(Buffer.concat([
    Buffer.from('noble live-slot composite code donors v1\0'),
    Buffer.from(canonical),
  ]));
}
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
function composedQuoteCaptures(program) {
  const graph = program.capture_values.find(entry => entry.field === 'cell_a')?.value;
  assert.ok(Array.isArray(graph));
  assert.equal(graph[0].kind, 4);
  return ['a', 'b'].map(edge => {
    const quotation = graph[graph[0][edge]];
    assert.equal(quotation.kind, 4);
    const scalar = graph[quotation.a];
    assert.equal(scalar.kind, 1);
    return scalar.payload;
  });
}
let complete = false;
try {
  expectOutcome(await session.configured, 'configured');
  for (const [id, name, definition] of definitions)
    await issue({ operation: 'define', id: `definition-${id}`, name, source: definition, inputs: [] }, 'definition-retained');
  const installations = {};
  for (const [id, name] of definitions)
    installations[id] = await issue({ operation: 'install', id, source: name,
      selected_name: name,
      inputs: ['dual', 'aliased', 'renamed-alias', 'independent', 'fixed-pair'].includes(id)
        ? ['I64', 'I64'] : ['I64'] }, 'installed');
  await issue({ operation: 'install', id: 'misbound', source: 'fixed',
    selected_name: 'builder', inputs: ['I64'] }, 'refused');
  await issue({ operation: 'install', id: 'misdigest', source: 'builder',
    selected_name: 'builder', inputs: ['I64'] }, 'refused');
  installations.runner = await issue({ operation: 'install', id: 'runner', source: 'run',
    inputs: ['I64', 'Program<I64,I64,pure>'] }, 'installed');
  installations.runner2 = await issue({ operation: 'install', id: 'runner2', source: 'run',
    inputs: ['I64', 'I64', 'Program<I64+I64,I64,pure>'] }, 'installed');
  installations.generic = await issue({ operation: 'install', id: 'generic',
    source: 'slot.invoke', inputs: ['I64', 'I64',
      'LiveRef<I64+I64,I64,pure>'] }, 'installed');
  const attemptedRebind = await issue({ operation: 'define',
    id: 'definition-renamed-alias', name: 'aliased',
    source: sources['definition-renamed-alias'], inputs: [] }, 'refused');
  assert.match(attemptedRebind.diagnostic, /selected declaration name is already retained/);

  const varying2 = saved(await invoke('variable', [input(2)]), 0, 'variable', installations.variable);
  assert.equal(composedCapture(varying2), '2');
  assert.ok(varying2.target_identity?.definition_id, 'checked anonymous target D is absent');
  assert.match(varying2.target_identity.definition_id,
    /^anonymous-template-experimental-v3-sha256:[a-f0-9]{64}$/);
  assert.match(varying2.target_identity.program_value_id,
    /^anonymous-program-experimental-v3-sha256:[a-f0-9]{64}$/);
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
  const unaryManifest = varying2.target_identity.artifact_manifest;
  assert.equal(varying2.target_identity.artifact_format,
    'noble-live-slot-composite-code-donors/v1');
  assert.equal(unaryManifest.owner.token, varying2.owner);
  assert.equal(unaryManifest.identity.definition_id, varying2.target_identity.definition_id);
  assert.equal(unaryManifest.identity.program_value_id, varying2.target_identity.program_value_id);
  assert.equal(unaryManifest.donors.native.artifact_sha256,
    installations.variable.artifact_sha256);
  assert.equal(unaryManifest.donors.static.artifact_sha256,
    installations.variable.artifact_sha256);
  assert.equal(varying2.target_identity.artifact_sha256, manifestDigest(unaryManifest));
  const renamed = saved(await invoke('renamed', [input(2)]), 0, 'renamed', installations.renamed);
  assert.notEqual(renamed.verified_origin.source_generation,
    varying2.verified_origin.source_generation);
  assert.equal(renamed.target_identity.definition_id, varying2.target_identity.definition_id);
  assert.equal(renamed.target_identity.program_value_id, varying2.target_identity.program_value_id);
  const zero = saved(await invoke('variable', [input(0)]), 0, 'variable', installations.variable);
  const negativeZero = saved(await invoke('variable', [input('-0')]), 0, 'variable', installations.variable);
  assert.equal(composedCapture(negativeZero), '0');
  assert.equal(negativeZero.target_identity.program_value_id, zero.target_identity.program_value_id);
  const unaryPublished = await issue({ operation: 'publish', slot: 'ordinary-control',
    owner: varying2.owner, expected_epoch: '0' }, 'published');
  assert.equal(unaryPublished.epoch, '1',
    'one checked returned quote/compose Program can be published under its exact donor graph');
  const noProof = await issue({ operation: 'publish', slot: 'proof-required',
    owner: varying2.owner, expected_epoch: '0' }, 'refused');
  assert.match(noProof.diagnostic, /proof-required slot lacks independently checked target evidence/);
  assert.deepEqual((await run(varying3.owner, 1)).stack, [{ kind: 1, value: '4' }]);
  const subtract = saved(await invoke('subtract', [input(2)]), 0, 'subtract',
    installations.subtract);
  assert.equal(composedCapture(subtract), '2');
  assert.ok(subtract.target_identity,
    'pure subtraction must acquire its own checked template, not the addition template');
  assert.notEqual(subtract.target_identity.definition_id, varying2.target_identity.definition_id);
  assert.deepEqual((await run(subtract.owner, 9)).stack, [{ kind: 1, value: '7' }]);
  const fixed = {};
  const fixedOwners = {};
  for (const id of ['fixed', 'computed', 'fixed3', 'computed3']) {
    const row = await invoke(id, [input(3)]);
    assert.deepEqual(row.stack[0], { kind: 1, value: '3' });
    const program = saved(row, 1, id, installations[id]);
    const literal = id.endsWith('3') ? 3 : 2;
    assert.equal(composedCapture(program), String(literal));
    assert.deepEqual(program.target_identity.captures, [],
      'fixed constant is bound inside the semantic definition, not a root capture');
    assert.deepEqual((await run(program.owner, 1)).stack, [{ kind: 1, value: String(literal + 1) }]);
    fixed[id] = program.target_identity;
    fixedOwners[id] = program.owner;
  }
  assert.notEqual(fixed.fixed.definition_id, fixed.computed.definition_id,
    'literal 2 and computed 1+1 bind distinct checked source-operation templates');
  assert.notEqual(fixed.fixed.program_value_id, fixed.computed.program_value_id);
  assert.notEqual(fixed.fixed.definition_id, fixed.fixed3.definition_id);
  assert.notEqual(fixed.computed.definition_id, fixed.computed3.definition_id);
  assert.notEqual(fixed.fixed.definition_id, varying2.target_identity.definition_id);
  assert.notEqual(fixed.fixed3.definition_id, varying3.target_identity.definition_id);
  assert.deepEqual((await run(fixedOwners.fixed, 1)).stack, [{ kind: 1, value: '3' }],
    'old saved Program must retain its original captured constant after newer source installation');
  assert.deepEqual((await run(fixedOwners.fixed3, 1)).stack, [{ kind: 1, value: '4' }]);
  for (const id of ['fixed', 'computed']) {
    const row = await invoke(id, [input(2)]);
    const sameFixed = saved(row, 1, id, installations[id]);
    assert.equal(composedCapture(sameFixed), '2');
    assert.equal(sameFixed.target_identity.definition_id, fixed[id].definition_id);
    assert.equal(sameFixed.target_identity.program_value_id, fixed[id].program_value_id);
  }
  const dual = await invoke('dual', [input(2), input(3)]);
  const first = saved(dual, 0, 'dual', installations.dual,
    'backend-observed-source-occurrence-unavailable');
  const second = saved(dual, 1, 'dual', installations.dual,
    'backend-observed-source-occurrence-unavailable');
  assert.deepEqual([first.capture_values, second.capture_values],
    [[{ type: 'I64', value: '3' }], [{ type: 'I64', value: '2' }]]);
  assert.equal(first.target_identity, undefined);
  assert.equal(second.target_identity, undefined);
  const copy = await invoke('copy', [input(7)]);
  assert.deepEqual([saved(copy, 0, 'copy', installations.copy,
    'backend-observed-source-occurrence-unavailable').capture_values,
    saved(copy, 1, 'copy', installations.copy,
      'backend-observed-source-occurrence-unavailable').capture_values],
  [[{ type: 'I64', value: '7' }], [{ type: 'I64', value: '7' }]]);
  assert.equal(copy.stack[0].target_identity, undefined);
  assert.equal(copy.stack[1].target_identity, undefined);
  const sharedProgram = saved(await invoke('shared-program', [input(2)]), 0,
    'shared-program', installations['shared-program']);
  const repeatedRoot = saved(await invoke('repeat-root', [input(2)]), 0,
    'repeat-root', installations['repeat-root']);
  assert.deepEqual((await run(sharedProgram.owner, 1)).stack, [{ kind: 1, value: '5' }]);
  assert.deepEqual((await run(repeatedRoot.owner, 1)).stack, [{ kind: 1, value: '5' }]);
  const sharedChildren = sharedProgram.capture_values.find(entry => entry.field === 'cell_a')?.value;
  const freshChildren = repeatedRoot.capture_values.find(entry => entry.field === 'cell_a')?.value;
  assert.equal(sharedChildren?.[0]?.a, sharedChildren?.[0]?.b,
    'let q repeats the exact immutable VM Program cell');
  assert.notEqual(freshChildren?.[0]?.a, freshChildren?.[0]?.b,
    'two quote executions create distinct backend Program cells');
  assert.equal(sharedProgram.target_identity.definition_id,
    repeatedRoot.target_identity.definition_id,
    'physical Program sharing must not change normalized root-binding template');
  assert.equal(sharedProgram.target_identity.program_value_id,
    repeatedRoot.target_identity.program_value_id);
  assert.deepEqual(sharedProgram.target_identity.captures, [{ type: 'I64', value: '2' }]);
  assert.deepEqual(repeatedRoot.target_identity.captures,
    [{ type: 'I64', value: '2' }]);
  assert.deepEqual(sharedProgram.target_identity.artifact_manifest.donors.native.entries,
    [1, 2, 3], 'checked lexical dip requires pinned native entry 3');
  const nestedProgram = saved(await invoke('nested-program', [input(2)]), 0,
    'nested-program', installations['nested-program']);
  assert.deepEqual((await run(nestedProgram.owner, 1)).stack, [{ kind: 1, value: '3' }]);
  const nestedChanged = saved(await invoke('nested-program', [input(3)]), 0,
    'nested-program', installations['nested-program']);
  assert.equal(nestedProgram.target_identity.definition_id,
    nestedChanged.target_identity.definition_id,
    'quote(Program) retains nested root binder class across varying captures');
  assert.notEqual(nestedProgram.target_identity.program_value_id,
    nestedChanged.target_identity.program_value_id);
  assert.deepEqual((await run(nestedChanged.owner, 1)).stack, [{ kind: 1, value: '4' }]);
  const same = saved(await invoke('aliased', [input(2), input(2)]), 0,
    'aliased', installations.aliased);
  const renamedAlias = saved(await invoke('renamed-alias', [input(2), input(2)]), 0,
    'renamed-alias', installations['renamed-alias']);
  const distinct = saved(await invoke('independent', [input(2), input(2)]), 0,
    'independent', installations.independent);
  for (const program of [same, renamedAlias, distinct])
    assert.deepEqual(composedQuoteCaptures(program), ['2', '2']);
  const sameIdentity = same.target_identity;
  const distinctIdentity = distinct.target_identity;
  assert.match(sameIdentity.definition_id, /^anonymous-template-experimental-v3-sha256:[a-f0-9]{64}$/);
  assert.match(sameIdentity.program_value_id, /^anonymous-program-experimental-v3-sha256:[a-f0-9]{64}$/);
  assert.notEqual(same.verified_origin.definition_identity,
    renamedAlias.verified_origin.definition_identity,
    'renamed alias fixtures must really use distinct checked builder owners');
  assert.notEqual(installations.aliased.artifact_sha256,
    installations['renamed-alias'].artifact_sha256,
    'alias equivalence must hold across distinct installed caller artifacts');
  assert.equal(sameIdentity.definition_id, renamedAlias.target_identity.definition_id,
    'renaming and different source sites preserve the same logical root binding');
  assert.equal(sameIdentity.program_value_id, renamedAlias.target_identity.program_value_id);
  assert.notEqual(sameIdentity.definition_id, distinctIdentity.definition_id,
    'two equal-valued independent source bindings are not one logical alias');
  assert.notEqual(sameIdentity.program_value_id, distinctIdentity.program_value_id);
  assert.deepEqual(sameIdentity.captures, [{ type: 'I64', value: '2' }]);
  assert.deepEqual(distinctIdentity.captures,
    [{ type: 'I64', value: '2' }, { type: 'I64', value: '2' }]);
  assert.deepEqual(sameIdentity.interface,
    { input: ['I64', 'I64'], output: ['I64'], effects: [] });
  assert.match(sameIdentity.artifact_sha256, /^[0-9a-f]{64}$/);
  assert.equal(sameIdentity.artifact_format, 'noble-live-slot-composite-code-donors/v1');
  assert.equal(sameIdentity.artifact_manifest.donors.native.artifact_sha256,
    installations.variable.artifact_sha256);
  assert.equal(sameIdentity.artifact_manifest.donors.static.artifact_sha256,
    installations.aliased.artifact_sha256);
  assert.deepEqual(sameIdentity.artifact_manifest.donors.native.entries, [1, 2]);
  assert.ok(sameIdentity.artifact_manifest.donors.static.entries.length >= 3,
    'the exact caller code donor must include its checked static add instructions');
  assert.equal(sameIdentity.artifact_manifest.owner.token, same.owner);
  assert.notEqual(sameIdentity.artifact_sha256, installations.aliased.artifact_sha256,
    'the returned executable is the two-donor graph bundle, not caller Wasm');
  assert.notEqual(sameIdentity.artifact_sha256, renamedAlias.target_identity.artifact_sha256,
    'distinct installed static code donors retain separate manifest identities');
  await issue({ operation: 'discard', id: 'variable' }, 'discard-refused');
  await issue({ operation: 'discard', id: 'aliased' }, 'discard-refused');
  assert.deepEqual((await run2(same.owner, 1, 4)).stack, [{ kind: 1, value: '9' }]);
  assert.deepEqual((await run2(distinct.owner, 1, 4)).stack, [{ kind: 1, value: '9' }]);
  const changed = saved(await invoke('independent', [input(2), input(3)]), 0,
    'independent', installations.independent);
  assert.deepEqual(composedQuoteCaptures(changed), ['3', '2']);
  assert.equal(changed.target_identity.definition_id, distinctIdentity.definition_id);
  assert.notEqual(changed.target_identity.program_value_id, distinctIdentity.program_value_id);
  assert.deepEqual(changed.target_identity.captures,
    [{ type: 'I64', value: '3' }, { type: 'I64', value: '2' }]);
  assert.equal(changed.target_identity.artifact_manifest.donors.static.artifact_sha256,
    installations.independent.artifact_sha256);
  assert.notEqual(changed.target_identity.artifact_sha256,
    distinct.target_identity.artifact_sha256,
    'changing captured graph data changes the exact returned manifest');
  assert.deepEqual((await run2(changed.owner, 1, 4)).stack, [{ kind: 1, value: '10' }]);
  const fixedPair = saved(await invoke('fixed-pair', [input(2), input(2)]), 1,
    'fixed-pair', installations['fixed-pair']);
  assert.ok(fixedPair.target_identity,
    'two independently checked fixed Program captures have their own constructed identity');
  assert.notEqual(fixedPair.target_identity.definition_id, sameIdentity.definition_id,
    'two equal fixed literals are not aliased root bindings');
  assert.deepEqual((await run2(fixedPair.owner, 1, 4)).stack, [{ kind: 1, value: '9' }]);
  const assertedManifest = await issue({ operation: 'publish', slot: 'alias-target',
    owner: same.owner, expected_epoch: '0',
    target_manifest: { artifact_sha256: installations.aliased.artifact_sha256 } }, 'refused');
  assert.match(assertedManifest.diagnostic, /unknown slot command or authority field/,
    'operator cannot supply a builder-hash target artifact or manifest');
  const fixedPairPublication = await issue({ operation: 'publish', slot: 'alias-proof-required',
    owner: fixedPair.owner, expected_epoch: '0' }, 'refused');
  assert.match(fixedPairPublication.diagnostic,
    /proof-required slot lacks independently checked target evidence/);
  const firstPublish = await issue({ operation: 'publish', slot: 'alias-target',
    owner: same.owner, expected_epoch: unaryPublished.epoch }, 'published');
  assert.equal(firstPublish.epoch, '2');
  const refs = [{ position: 2, ordinal: 0, slot: 'alias-target' }];
  const recorded = await issue({ operation: 'invoke', id: 'generic',
    inputs: [input(1), input(4)], refs, record: true }, 'executed', 1);
  assert.deepEqual(recorded.stack, [{ kind: 1, value: '9' }]);
  assert.equal(recorded.frozen_identity.slots[0].artifactSha256,
    sameIdentity.artifact_sha256);
  assert.equal(recorded.frozen_identity.slots[0].programValueId,
    sameIdentity.program_value_id);
  assert.deepEqual(recorded.frozen_identity.slots[0].captures, ['I64:2']);
  assert.equal(recorded.frozen_identity.slots[0].recipeSha256,
    sameIdentity.artifact_manifest.recipe_sha256);
  const replayRequest = { operation: 'replay', token: recorded.replay_token,
    inputs: [input(1), input(4)], refs,
    expected_identity: structuredClone(recorded.frozen_identity),
    expected_trace: structuredClone(recorded.request_trace),
    expected_stack: structuredClone(recorded.stack) };
  const secondCapture = saved(await invoke('aliased', [input(3), input(2)]), 0,
    'aliased', installations.aliased);
  assert.equal(secondCapture.target_identity.definition_id, sameIdentity.definition_id);
  assert.notEqual(secondCapture.target_identity.program_value_id,
    sameIdentity.program_value_id);
  const secondPublish = await issue({ operation: 'publish', slot: 'alias-target',
    owner: secondCapture.owner, expected_epoch: firstPublish.epoch,
    expected_incarnation: firstPublish.incarnation,
    expected_generation: firstPublish.generation }, 'published');
  assert.deepEqual([secondPublish.epoch, secondPublish.incarnation, secondPublish.generation],
    ['3', '1', '2']);
  const staleEpoch = await issue({ operation: 'publish', slot: 'alias-target',
    owner: same.owner, expected_epoch: firstPublish.epoch,
    expected_incarnation: secondPublish.incarnation,
    expected_generation: secondPublish.generation }, 'stale-reject');
  assert.equal(staleEpoch.epoch, secondPublish.epoch,
    'old observed global epoch must not replace the current saved returned target');
  await issue({ operation: 'release-program', owner: same.owner }, 'program-released');
  const fresh = await issue({ operation: 'invoke', id: 'generic',
    inputs: [input(1), input(4)], refs }, 'executed', 1);
  assert.equal(fresh.epoch, secondPublish.epoch);
  assert.equal(fresh.request_trace.find(row => row.operation === 'dispatch')?.generation,
    secondPublish.generation);
  assert.deepEqual(fresh.stack, [{ kind: 1, value: '11' }]);
  const replaced = structuredClone(replayRequest);
  replaced.expected_identity.slots[0].programValueId =
    secondCapture.target_identity.program_value_id;
  replaced.expected_identity.slots[0].artifactSha256 =
    secondCapture.target_identity.artifact_sha256;
  const rejectedReplay = await issue(replaced, 'replay-diverged');
  assert.equal(rejectedReplay.guest_requests, 0);
  assert.equal(rejectedReplay.protected_operations, 0);
  const pinnedReplay = await issue(replayRequest, 'replay-matched', 1);
  assert.deepEqual(pinnedReplay.actual_stack, recorded.stack);
  await issue({ operation: 'release-replay', token: recorded.replay_token }, 'replay-released');
  const aliasedNoProof = await issue({ operation: 'publish', slot: 'alias-proof-required',
    owner: secondCapture.owner, expected_epoch: secondPublish.epoch }, 'refused');
  assert.match(aliasedNoProof.diagnostic, /proof-required slot lacks independently checked target evidence/);
  const opaque = saved(await invoke('ambiguous', [input(2)]), 0, 'ambiguous',
    installations.ambiguous, 'backend-observed-source-occurrence-unavailable');
  assert.equal(composedCapture(opaque), '3');
  assert.equal(opaque.target_identity, undefined);
  assert.deepEqual((await run(opaque.owner, 1)).stack, [{ kind: 1, value: '4' }]);
  await run('program-999999', 1, 'refused');
  for (const owner of owners)
    if (owner !== same.owner)
      await issue({ operation: 'release-program', owner }, 'program-released');
  await run(varying2.owner, 1, 'refused');
  const staleOwner = await issue({ operation: 'publish', slot: 'alias-target',
    owner: same.owner, expected_epoch: secondPublish.epoch,
    expected_incarnation: secondPublish.incarnation,
    expected_generation: secondPublish.generation }, 'refused');
  assert.match(staleOwner.diagnostic, /unknown saved checked Program owner/);
  const exit = await session.close();
  assert.equal(exit, 0);
  const records = fs.readFileSync(session.log, 'utf8').trim().split('\n').map(JSON.parse);
  assert.equal(records[0].kind, 'authority');
  assert.equal(records[0].selected_binary_sha256, session.selectedBinarySha256);
  assert.equal(records.at(-1).kind, 'exit');
  assert.equal(records.at(-1).code, 0);
  fs.writeFileSync(path.join(evidenceDirectory, 'LSLOT01-selected-origin-control.summary.json'),
    JSON.stringify({ selected_binary_sha256: session.selectedBinarySha256,
      owner_count: owners.size, installed_artifacts: Object.fromEntries(
        Object.entries(installations).map(([id, row]) => [id, row.artifact_sha256])),
      returned_target_manifest_sha256: sameIdentity.artifact_sha256,
      published_epoch: secondPublish.epoch,
      guest_requests: replies.reduce((n, row) => n + (row.guest_requests ?? 0), 0),
      protected_operations: replies.reduce((n, row) => n + (row.protected_operations ?? 0), 0) }, null, 2) + '\n',
    { flag: 'wx', mode: 0o600 });
  complete = true;
} finally {
  if (!complete) await session.abort();
}

// Live-slot `define` deliberately forbids replacing the same name. Exercise
// the permitted old/new path through distinct selected source IDs/names:
// the returned semantic target stays equal while its installed donor changes,
// and the already saved old Program retains its own old code owner.
const reload = await cli(binary, 'LSLOT01-selected-distinct-source-reload', authority);
let reloadComplete = false;
try {
  expectOutcome(await reload.configured, 'configured');
  for (const [id, name] of [['aliased', 'aliased'], ['renamed-alias', 'otheralias']])
    expectOutcome(await reload.issue({ operation: 'define', id: `definition-${id}`,
      name, source: sources[`definition-${id}`], inputs: [] }), 'definition-retained');
  const oldInstall = expectOutcome(await reload.issue({ operation: 'install',
    id: 'aliased', source: 'aliased', selected_name: 'aliased',
    inputs: ['I64', 'I64'] }), 'installed');
  const newInstall = expectOutcome(await reload.issue({ operation: 'install',
    id: 'renamed-alias', source: 'otheralias', selected_name: 'otheralias',
    inputs: ['I64', 'I64'] }), 'installed');
  expectOutcome(await reload.issue({ operation: 'install', id: 'runner2',
    source: 'run', inputs: ['I64', 'I64', 'Program<I64+I64,I64,pure>'] }), 'installed');
  const old = expectOutcome(await reload.issue({ operation: 'invoke', id: 'aliased',
    inputs: [input(2), input(2)], refs: [] }), 'executed').stack[0];
  const newer = expectOutcome(await reload.issue({ operation: 'invoke', id: 'renamed-alias',
    inputs: [input(2), input(2)], refs: [] }), 'executed').stack[0];
  assert.equal(old.target_identity.definition_id, newer.target_identity.definition_id);
  assert.equal(old.target_identity.program_value_id, newer.target_identity.program_value_id);
  assert.notEqual(old.target_identity.artifact_sha256, newer.target_identity.artifact_sha256);
  assert.equal(old.target_identity.artifact_manifest.donors.static.artifact_sha256,
    oldInstall.artifact_sha256);
  assert.equal(newer.target_identity.artifact_manifest.donors.static.artifact_sha256,
    newInstall.artifact_sha256);
  const oldPublished = expectOutcome(await reload.issue({ operation: 'publish',
    slot: 'alias-target', owner: old.owner, expected_epoch: '0' }), 'published');
  assert.equal(oldPublished.epoch, '1');
  const newPublished = expectOutcome(await reload.issue({ operation: 'publish',
    slot: 'alias-target', owner: newer.owner, expected_epoch: '1',
    expected_incarnation: '1', expected_generation: '1' }), 'published');
  assert.equal(newPublished.epoch, '2');
  const savedRun = owner => reload.issue({ operation: 'invoke', id: 'runner2',
    inputs: [input(1), input(4), { kind: 'program', owner }], refs: [] });
  assert.deepEqual(expectOutcome(await savedRun(old.owner), 'executed').stack,
    [{ kind: 1, value: '9' }], 'old saved Program changed after compatible newer source publication');
  assert.deepEqual(expectOutcome(await savedRun(newer.owner), 'executed').stack,
    [{ kind: 1, value: '9' }]);
  expectOutcome(await reload.issue({ operation: 'discard', id: 'aliased' }), 'discard-refused');
  expectOutcome(await reload.issue({ operation: 'release-program', owner: old.owner }),
    'program-released');
  expectOutcome(await reload.issue({ operation: 'release-program', owner: newer.owner }),
    'program-released');
  expectOutcome(await savedRun(old.owner), 'refused');
  assert.equal(await reload.close(), 0);
  reloadComplete = true;
} finally {
  if (!reloadComplete) await reload.abort();
}

// A later selected caller cannot borrow the native quote/compose code donor
// from an earlier unselected installation whose generation-one core binary
// was not independently inspected for selected source/code correspondence.
const missingCore = await cli(binary, 'LSLOT01-selected-origin-missing-core', authority);
let missingCoreComplete = false;
try {
  expectOutcome(await missingCore.configured, 'configured');
  expectOutcome(await missingCore.issue({ operation: 'define', id: 'definition-aliased',
    name: 'aliased', source: sources['definition-aliased'], inputs: [] }), 'definition-retained');
  const unselected = expectOutcome(await missingCore.issue({ operation: 'install',
    id: 'runner2', source: 'run', inputs: ['I64', 'I64', 'Program<I64+I64,I64,pure>'] }), 'installed');
  assert.equal(unselected.generation, '1');
  const later = expectOutcome(await missingCore.issue({ operation: 'install',
    id: 'aliased', source: 'aliased', selected_name: 'aliased',
    inputs: ['I64', 'I64'] }), 'installed');
  assert.equal(later.generation, '2');
  const returned = expectOutcome(await missingCore.issue({ operation: 'invoke', id: 'aliased',
    inputs: [input(2), input(2)], refs: [] }), 'executed').stack[0];
  assert.match(returned.target_identity.definition_id,
    /^anonymous-template-experimental-v3-sha256:[a-f0-9]{64}$/);
  const noCoreAttestation = expectOutcome(await missingCore.issue({ operation: 'publish',
    slot: 'alias-target', owner: returned.owner, expected_epoch: '0' }), 'refused');
  assert.match(noCoreAttestation.diagnostic,
    /selected returned target lacks independently checked composite code donor attestation/);
  expectOutcome(await missingCore.issue({ operation: 'release-program',
    owner: returned.owner }), 'program-released');
  assert.equal(await missingCore.close(), 0);
  missingCoreComplete = true;
} finally {
  if (!missingCoreComplete) await missingCore.abort();
}
console.log('selected origin and experimental D/P: two-donor returned target publication, exact captured generic dispatch and frozen replay, asserted/retired/missing donor refusal, proof-required refusal and opaque controls');
