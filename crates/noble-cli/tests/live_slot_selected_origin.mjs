import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { cli, evidenceDirectory, expectOutcome, selectedAuthority } from './live_slot_resource_ref_cli.mjs';

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
  ['renamed-alias', 'otheralias', 'def otheralias [ swap drop dup quote swap quote compose [ + + + ] compose ]'],
  ['independent', 'independent', 'def independent [ quote swap quote compose [ + + + ] compose ]'],
  ['fixed-pair', 'fixedpair', 'def fixedpair [ drop 2 quote 2 quote compose [ + + + ] compose ]'],
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
    /^anonymous-template-experimental-v2-blake3:[a-f0-9]{64}$/);
  assert.match(varying2.target_identity.program_value_id,
    /^anonymous-program-experimental-v2-blake3:[a-f0-9]{64}$/);
  // Independently derived with `b3sum --derive-key` over the strict binary
  // projection (including the domain-specific context and LE I64 capture).
  assert.equal(varying2.target_identity.definition_id,
    'anonymous-template-experimental-v2-blake3:effbd15ea069fc0d952e38024efebca823c96c2b0d7db7b41214957543602268');
  assert.equal(varying2.target_identity.program_value_id,
    'anonymous-program-experimental-v2-blake3:c6cd72597427ab4586b858614b13a2838cb99d855f748ade658f0248df233248');
  assert.deepEqual((await run(varying2.owner, 1)).stack, [{ kind: 1, value: '3' }]);
  const varying3 = saved(await invoke('variable', [input(3)]), 0, 'variable', installations.variable);
  assert.equal(composedCapture(varying3), '3');
  assert.equal(varying2.target_identity.definition_id, varying3.target_identity.definition_id);
  assert.notEqual(varying2.target_identity.program_value_id, varying3.target_identity.program_value_id);
  assert.equal(varying3.target_identity.program_value_id,
    'anonymous-program-experimental-v2-blake3:c54691eb93f0847326198765a609aaef63686214545b542dd60e71a9f9cd37d3');
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
  const subtract = saved(await invoke('subtract', [input(2)]), 0, 'subtract',
    installations.subtract);
  assert.equal(composedCapture(subtract), '2');
  assert.equal(subtract.target_identity, undefined,
    'same-interface pure subtraction must not borrow the checked addition identity');
  assert.deepEqual((await run(subtract.owner, 9)).stack, [{ kind: 1, value: '7' }]);
  const fixed = {};
  for (const id of ['fixed', 'computed', 'fixed3', 'computed3']) {
    const row = await invoke(id, [input(3)]);
    assert.deepEqual(row.stack[0], { kind: 1, value: '3' });
    const program = saved(row, 1, id, installations[id]);
    const literal = id.endsWith('3') ? 3 : 2;
    assert.equal(composedCapture(program), String(literal));
    assert.deepEqual(program.target_identity.captures, [{ type: 'I64', value: String(literal) }]);
    if (id === 'fixed' || id === 'computed') {
      assert.equal(program.target_identity.definition_id,
        'anonymous-template-experimental-v2-blake3:318c102d0313f5a7ea5c93c36c6bf8c51883a6c974fe73820e709abe9be8c21a');
      assert.equal(program.target_identity.program_value_id,
        'anonymous-program-experimental-v2-blake3:46f4308d213416a663e5a6e876eaf4d149e2b4378fb1a746fbe4d9b21999f972');
    }
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
  assert.match(sameIdentity.definition_id, /^anonymous-template-experimental-v2-blake3:[a-f0-9]{64}$/);
  assert.match(sameIdentity.program_value_id, /^anonymous-program-experimental-v2-blake3:[a-f0-9]{64}$/);
  assert.notEqual(same.verified_origin.definition_identity,
    renamedAlias.verified_origin.definition_identity,
    'renamed alias fixtures must really use distinct checked builder owners');
  assert.notEqual(installations.aliased.artifact_sha256,
    installations['renamed-alias'].artifact_sha256,
    'alias equivalence must hold across distinct installed caller artifacts');
  assert.equal(sameIdentity.definition_id, renamedAlias.target_identity.definition_id,
    'renaming and a different root binding/physical source site preserve one logical alias');
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
  assert.equal(sameIdentity.artifact_manifest.donors.static.entries.length, 3);
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
    'fixed-pair', installations['fixed-pair'],
    'backend-observed-source-occurrence-unavailable');
  assert.equal(fixedPair.target_identity, undefined,
    'equal fixed quote literals do not acquire the checked root-binding alias identity');
  assert.deepEqual((await run2(fixedPair.owner, 1, 4)).stack, [{ kind: 1, value: '9' }]);
  const assertedManifest = await issue({ operation: 'publish', slot: 'alias-target',
    owner: same.owner, expected_epoch: '0',
    target_manifest: { artifact_sha256: installations.aliased.artifact_sha256 } }, 'refused');
  assert.match(assertedManifest.diagnostic, /unknown slot command or authority field/,
    'operator cannot supply a builder-hash target artifact or manifest');
  const fixedPairPublication = await issue({ operation: 'publish', slot: 'alias-target',
    owner: fixedPair.owner, expected_epoch: '0' }, 'refused');
  assert.match(fixedPairPublication.diagnostic,
    /dynamic saved Program lacks independently checked installed target metadata/);
  const firstPublish = await issue({ operation: 'publish', slot: 'alias-target',
    owner: same.owner, expected_epoch: '0' }, 'published');
  assert.equal(firstPublish.epoch, '1');
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
    owner: secondCapture.owner, expected_epoch: '1', expected_incarnation: '1',
    expected_generation: '1' }, 'published');
  assert.equal(secondPublish.epoch, '2');
  await issue({ operation: 'release-program', owner: same.owner }, 'program-released');
  const fresh = await issue({ operation: 'invoke', id: 'generic',
    inputs: [input(1), input(4)], refs }, 'executed', 1);
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
    owner: secondCapture.owner, expected_epoch: '2' }, 'refused');
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
    owner: same.owner, expected_epoch: '2',
    expected_incarnation: '1', expected_generation: '2' }, 'refused');
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
    /^anonymous-template-experimental-v2-blake3:[a-f0-9]{64}$/);
  const noCoreAttestation = expectOutcome(await missingCore.issue({ operation: 'publish',
    slot: 'alias-target', owner: returned.owner, expected_epoch: '0' }), 'refused');
  assert.match(noCoreAttestation.diagnostic,
    /selected native core code donor lacks first-generation binary attestation/);
  expectOutcome(await missingCore.issue({ operation: 'release-program',
    owner: returned.owner }), 'program-released');
  assert.equal(await missingCore.close(), 0);
  missingCoreComplete = true;
} finally {
  if (!missingCoreComplete) await missingCore.abort();
}
console.log('selected origin and experimental D/P: two-donor returned target publication, exact captured generic dispatch and frozen replay, asserted/retired/missing donor refusal, proof-required refusal and opaque controls');
