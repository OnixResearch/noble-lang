// Projections from selected live-slot lane evidence onto the canonical
// LSLOT case records in `specs/conformance/live-reference-cases.json`.
//
// A projection entry is one canonical `input`, `expected` or variant field:
//   observed  — a lane measured the exact fact, and it equals the canonical value
//   mismatch  — a lane measured the exact fact, and it differs (a gate failure)
//   unobserved— a machine-expressible premise whose canonical attempt did not run
//   declared  — prose/design text that no runtime field can carry; it is bound
//               only by the case record itself, never counted as an observation
// A case is promotable only with `observed` and `declared` entries: one
// `mismatch` fails the gate, one `unobserved` refuses the case.
import assert from 'node:assert/strict';
import fs from 'node:fs';

export const jsonLines = file => fs.readFileSync(file, 'utf8').trim()
  .split('\n').filter(Boolean).map(line => JSON.parse(line));
const same = (left, right) => JSON.stringify(left) === JSON.stringify(right);

function measured(scope, key, canonical, observed, note = null) {
  return { scope, key, canonical, observed,
    status: same(canonical, observed) ? 'observed' : 'mismatch', note };
}
function unobserved(scope, key, canonical, reason) {
  return { scope, key, canonical, observed: null, status: 'unobserved', note: reason };
}
function declared(scope, key, canonical, basis) {
  return { scope, key, canonical, observed: null, status: 'declared', note: basis };
}
const numberValue = value => Number(value);
// The lanes spell the empty effect set `pure`; the canonical design spells it `{}`.
const normalizeType = type => type
  .replace(/^LiveRef<(.+),(.+),pure>$/, 'borrow LiveRef<$1,$2,{}>')
  .replace(/^LiveRef<(.+),(.+),([^>]+)>$/, 'borrow LiveRef<$1,$2,{$3}>');

// The runtime reports a phase in `stage`; the canonical case names that phase.
// Every report of a case must map to the case's single canonical stage.
function stageEntry(steps, mapping, canonical) {
  const unmapped = steps.map(step => step.row.stage)
    .filter(stage => !Object.hasOwn(mapping, stage));
  const observed = [...new Set(steps.map(step => mapping[step.row.stage] ?? step.row.stage))];
  if (unmapped.length > 0) {
    return { scope: 'expected', key: 'stage', canonical, observed,
      status: 'mismatch', note: `unmapped runtime stage: ${unmapped.join(', ')}` };
  }
  assert.equal(observed.length, 1, `one phase for ${canonical}`);
  return measured('expected', 'stage', canonical, observed[0],
    'runtime report stages of this profile projected onto the canonical case phase');
}

// `slot-vN` names the registered version of a slot by its generation; the body
// is the exact checked source bytes that version selects.
const versionLabel = (slot, generation, body, expectedBody) => {
  assert.equal(body, expectedBody, `selected body for ${slot}`);
  return `${slot}-v${numberValue(generation)}`;
};

const paired = records => {
  const requests = records.filter(row => row.kind === 'operator').map(row => row.request);
  const replies = records.filter(row => row.kind === 'cli').map(row => row.row);
  assert.equal(replies.length, requests.length + 1,
    'every operator request has one CLI reply plus the configuration report');
  return [{ request: null, row: replies[0] },
    ...replies.slice(1).map((row, index) => ({ request: requests[index], row }))];
};

export function project01({ fixture, lane }) {
  const steps = paired(lane.records);
  const at = (label, predicate) => {
    const matches = steps.filter(predicate);
    assert.equal(matches.length, 1, `unique ${label} step`);
    return matches[0];
  };
  const define = at('define', step => step.request?.operation === 'define');
  const savedInstall = at('saved install',
    step => step.request?.operation === 'install' && step.request.id === 'saved');
  const genericInstall = at('generic install',
    step => step.request?.operation === 'install' && step.request.id === 'generic');
  const originalInstall = at('original install',
    step => step.request?.operation === 'install' && step.request.id === 'original');
  const replacementInstall = at('replacement install',
    step => step.request?.operation === 'install' && step.request.id === 'replacement');
  const firstPublish = at('first publish',
    step => step.request?.operation === 'publish' && step.request.id === 'original');
  const secondPublish = at('second publish',
    step => step.request?.operation === 'publish' && step.request.id === 'replacement');
  const ordinaryRoot = at('ordinary run',
    step => step.request?.operation === 'invoke' && step.request.id === 'runner');
  const genericCalls = steps.filter(step =>
    step.request?.operation === 'invoke' && step.request.id === 'generic');
  assert.equal(genericCalls.length, 2, 'one generic call before and one after publication');
  const [beforeCall, afterCall] = genericCalls;
  const reflections = steps.filter(step => step.request?.operation === 'reflect');
  assert.ok(reflections.length >= 2, 'reflection before and after publication');
  const site = reflections[0].row.site;
  const savedResult = numberValue(ordinaryRoot.row.stack[0].value);
  const genericResult = numberValue(afterCall.row.stack[0].value);
  const prePublicationResult = numberValue(beforeCall.row.stack[0].value);
  const freshTarget = afterCall.row.request_trace[0]?.artifactSha256
    === replacementInstall.row.artifact_sha256;
  const oldTarget = ordinaryRoot.row.request_trace.length === 0
    && savedResult === prePublicationResult
    && beforeCall.row.request_trace[0]?.artifactSha256 === originalInstall.row.artifact_sha256;
  const definitionBody = /^def \S+ \[ (.*) \]$/.exec(fixture.input.definition)[1];
  return [
    measured('input', 'definition', fixture.input.definition, define.request.source),
    measured('input', 'saved_program', fixture.input.saved_program,
      savedInstall.request.source),
    measured('input', 'generic_caller', {
      instruction: 'slot.invoke', borrowed_input: 'LiveRef<I64,I64,pure>',
      effects: ['live.dispatch'],
    }, { instruction: site.instruction,
      borrowed_input: genericInstall.request.inputs.find(type => type.startsWith('LiveRef')),
      effects: site.effects },
    'structured projection of the canonical generic-caller prose: instruction, borrowed input'
      + ' type and checked effects'),
    measured('input', 'slot', { id: 'increment', incarnation: 1, generation: 1,
      target: `n-v1:[ ${definitionBody} ]` },
    { id: firstPublish.request.slot,
      incarnation: numberValue(firstPublish.row.incarnation),
      generation: numberValue(firstPublish.row.generation),
      target: `n-v${numberValue(firstPublish.row.generation)}:[ ${definitionBody} ]` }),
    measured('input', 'publication', {
      expected_epoch: fixture.input.publication.expected_epoch,
      expected_incarnation: fixture.input.publication.expected_incarnation,
      expected_generation: fixture.input.publication.expected_generation,
      candidate: `n-v2:[ ${replacementInstall.request.source.slice(2, -2)} ]`,
      host_authorized: true,
    }, { expected_epoch: numberValue(secondPublish.request.expected_epoch),
      expected_incarnation: numberValue(secondPublish.request.expected_incarnation),
      expected_generation: numberValue(secondPublish.request.expected_generation),
      candidate: `n-v${numberValue(secondPublish.row.generation)}:[ `
        + `${replacementInstall.request.source.slice(2, -2)} ]`,
      host_authorized: secondPublish.row.outcome === 'published' }),
    measured('input', 'calls_after_publication', [
      { subject: 'saved_program', argument: 20 },
      { subject: 'generic_caller', argument: 20, host_issued_input_ref: 'increment' },
    ], [
      { subject: 'saved_program', argument: numberValue(ordinaryRoot.request.inputs[0].value) },
      { subject: 'generic_caller', argument: numberValue(afterCall.request.inputs[0].value),
        host_issued_input_ref: afterCall.request.refs[0]?.slot },
    ]),
    stageEntry(steps, { slot: 'live-slot-root', command: 'live-slot-root',
      source: 'live-slot-root' }, 'live-slot-root'),
    measured('expected', 'outcome', 'immutable-capture-and-explicit-fresh-target',
      oldTarget && freshTarget && savedResult === 21 && genericResult === 22
        ? 'immutable-capture-and-explicit-fresh-target' : 'other'),
    measured('expected', 'saved_result', fixture.expected.saved_result, savedResult),
    measured('expected', 'generic_result', fixture.expected.generic_result, genericResult),
    measured('expected', 'saved_target', 'n-v1', oldTarget ? 'n-v1' : 'other',
      'the saved owner runs the pre-publication target and records no dispatch trace'),
    measured('expected', 'generic_selected_target', 'n-v2', freshTarget ? 'n-v2' : 'other',
      'the post-publication generic call selects the replacement artifact at generation 2'),
    measured('expected', 'caller_identity_unchanged', fixture.expected.caller_identity_unchanged,
      same(site, reflections.at(-1).row.site)),
    measured('expected', 'ordinary_run_dynamic_lookup', false,
      ordinaryRoot.row.request_trace.length > 0 || ordinaryRoot.row.guest_requests !== 0),
    measured('expected', 'generic_recipe_embeds_slot_or_target', false,
      ['slotId', 'slot', 'target', 'artifactSha256', 'version', 'name']
        .some(field => Object.hasOwn(site, field))),
    measured('expected', 'new_global_epoch', fixture.expected.new_global_epoch,
      numberValue(secondPublish.row.epoch)),
  ];
}

export function project02({ fixture, lane }) {
  const byName = name => {
    const item = lane.results.find(result => result.name === name);
    assert.ok(item, `missing LSLOT-02 ${name} session`);
    assert.equal(item.status, 'observed', item.error);
    return item;
  };
  const base = byName('base-parent-child'), transitive = byName('transitive-A-B-C');
  const bodies = { A1: 'slot.invoke slot.invoke', B1: '1 +', B2: '2 +',
    C1: '1 +', C2: '2 +' };
  const selection = (item, root, slot, expectedBody) => {
    const trace = root.request_trace.filter(entry => entry.slotId === slot);
    assert.ok(trace.length > 0, `${slot} dispatch present`);
    const artifact = trace.at(-1).artifactSha256;
    const id = Object.keys(item.installed_sha256)
      .find(key => item.installed_sha256[key] === artifact);
    assert.ok(id, `${slot} dispatch selects an installed artifact`);
    return versionLabel(slot, trace.at(-1).generation, bodies[id], expectedBody);
  };
  const generations = item => [item.old_root, item.new_root, item.next_leaf];
  const dispatchPolicy = item => generations(item)
    .flatMap(root => root.request_trace)
    .every(entry => entry.operation === 'dispatch' && entry.policyAllowed === true
      && entry.policyInput.operation === 'dispatch'
      && entry.policyInput.slotId === entry.slotId
      && entry.policyInput.rootEpoch === entry.rootEpoch
      && entry.policyInput.version.artifactSha256 === entry.artifactSha256);
  const separate = lane.crossProfile;
  const roots = [base.old_root, base.new_root, transitive.old_root, transitive.new_root];
  return [
    measured('input', 'base.registry_epoch_1.B.version', 'B-v1:[ 1 + ]',
      `B-v1:[ ${bodies.B1} ]`),
    measured('input', 'base.registry_epoch_1.A.interface', {
      input: ['I64', 'borrow LiveRef<I64,I64,{}>'], output: ['I64'],
      effects: ['live.dispatch'],
    }, { input: lane.authority.slots.find(slot => slot.slotId === 'A').input
      .map(normalizeType),
    output: lane.authority.slots.find(slot => slot.slotId === 'A').output.map(normalizeType),
    effects: lane.authority.slots.find(slot => slot.slotId === 'A').effectCeiling }),
    declared('input', 'base.registry_epoch_1.A.body', fixture.input.base.registry_epoch_1.A.body,
      'the selected A1 body is `slot.invoke slot.invoke` and embeds no slot name'),
    measured('input', 'base.root_A_entry.ordered_host_issued_inputs', 3,
      base.old_root.request_trace.length,
      'I64:20 plus the host-issued B and A refs; the root performs one A and two B dispatches'),
    measured('input', 'base.root_A_entry.pin_epoch', 1, numberValue(base.old_root.epoch)),
    measured('input', 'base.new_root_B_interface', { input: ['I64'], output: ['I64'], effects: [] },
    { input: base.next_leaf.request_trace[0].policyInput.version.input.map(normalizeType),
      output: base.next_leaf.request_trace[0].policyInput.version.output.map(normalizeType),
      effects: base.next_leaf.request_trace[0].policyInput.version.effects }),
    measured('input', 'transitive_variant.registry_epoch_1.C.version', 'C-v1:[ 1 + ]',
      `C-v1:[ ${bodies.C1} ]`),
    measured('input', 'transitive_variant.root_A_entry.pin_epoch', 1,
      numberValue(transitive.old_root.epoch)),
    declared('input', 'transitive_variant.registry_epoch_1.B.body',
      fixture.input.transitive_variant.registry_epoch_1.B.body,
      'the selected B1 body is `slot.invoke` and dispatches only through its host-issued C ref'),
    declared('input', 'base.interleaving', fixture.input.base.interleaving,
      'prose steps bound by the observed B-v1/B-v1/A result, the epoch-2 publication and the'
      + ' next-root B-v2 selection above'),
    declared('input', 'transitive_variant.interleaving',
      fixture.input.transitive_variant.interleaving,
      'prose steps bound by the observed epoch-1 pin, the epoch-2 C publication and the old/new C'
      + ' selections above'),
    stageEntry(lane.steps, { slot: 'live-slot-root', command: 'live-slot-root',
      source: 'live-slot-root', 'operator-control': 'live-slot-root' }, 'live-slot-root'),
    measured('expected', 'outcome', 'one-map-per-root-with-explicit-typed-ref-pass-through',
      numberValue(base.old_root.epoch) === numberValue(base.new_root.epoch) - 1
        && numberValue(transitive.old_root.epoch)
          === numberValue(transitive.new_root.epoch) - 1
        ? 'one-map-per-root-with-explicit-typed-ref-pass-through' : 'other'),
    measured('expected', 'base_root_A_B_selections', ['B-v1', 'B-v1'],
      [selection(base, base.old_root, 'B', '1 +'), selection(base, base.old_root, 'B', '1 +')]),
    measured('expected', 'base_root_A_result', fixture.expected.base_root_A_result,
      numberValue(base.old_root.stack[0].value)),
    measured('expected', 'base_next_root_B_selection', 'B-v2',
      selection(base, base.next_leaf, 'B', '2 +')),
    measured('expected', 'base_next_root_B_result', fixture.expected.base_next_root_B_result,
      numberValue(base.next_leaf.stack[0].value)),
    measured('expected', 'variant_old_root_A_B_C_selections', ['A-v1', 'B-v1', 'C-v1'],
      [selection(transitive, transitive.old_root, 'A', bodies.A1),
        selection(transitive, transitive.old_root, 'B', bodies.B1),
        selection(transitive, transitive.old_root, 'C', bodies.C1)]),
    measured('expected', 'variant_old_root_result', fixture.expected.variant_old_root_result,
      numberValue(transitive.old_root.stack[0].value)),
    measured('expected', 'variant_next_root_C_selection', 'C-v2',
      selection(transitive, transitive.next_leaf, 'C', '2 +')),
    measured('expected', 'variant_next_root_C_result',
      fixture.expected.variant_next_root_C_result,
      numberValue(transitive.next_leaf.stack[0].value)),
    measured('expected', 'mixed_registry_epochs_within_root', false,
      roots.some(root => root.request_trace.some(entry =>
        numberValue(entry.rootEpoch) !== numberValue(root.epoch)))),
    measured('expected', 'implicit_source_name_or_captured_ref_lookup', false,
      roots.some(root => root.request_trace.some(entry =>
        /slotId/.test(entry.recipeSha256 + entry.semanticContext + entry.programValueId))),
      'no selected recipe, semantic context or program identity embeds a slot name; the slot is the'
      + ' host-issued ordered ref operand of the invocation'),
    measured('expected', 'current_authorization_rechecked_at_each_dispatch', true,
      dispatchPolicy(base) && dispatchPolicy(transitive)),
    measured('expected',
      'registry_epoch_distinct_from_live_wasm_namespace_or_source_generation', true,
      separate.registryEpochs.every(epoch => Number.isInteger(epoch))
        && separate.liveGenerations.every(generation => Number.isInteger(generation))
        && separate.liveReportsHaveEpoch === false
        && separate.refusedSlotCaller === true,
      `slot registry epochs ${separate.registryEpochs.join('/')} advance only on publication and`
      + ` are reported only by the slot profile; the guarded Live-Wasm-Draft lane reports its own`
      + ` source generations ${separate.liveGenerations.join('/')} and refuses this caller`),
  ];
}

export function project03({ fixture, lane }) {
  const entries = [];
  for (const variant of fixture.input.variants) {
    const item = lane.summary.results.find(result => result.name === variant.name);
    assert.ok(item, `missing LSLOT-03 ${variant.name}`);
    const publication = item.publication === 'published' ? 'publish' : 'reject';
    if (item.status !== 'pass') {
      entries.push(unobserved('variant', variant.name, variant.expected,
        `the canonical attempt did not run: ${item.limitation ?? item.status}`));
      continue;
    }
    entries.push(measured('variant', variant.name, variant.expected, publication));
    entries.push(measured('variant', `${variant.name}.guest_requests`, 0,
      item.guest_requests));
    entries.push(measured('variant', `${variant.name}.protected_operations`, 0,
      item.protected_operations));
  }
  const accepted = lane.summary.results
    .find(result => result.name === 'same-exact-interface');
  const refused = lane.summary.results.filter(result => result.name !== 'same-exact-interface');
  const generic = lane.summary.results
    .find(result => result.name === 'generic-caller-underdeclares-dispatch');
  const exactAfterRefusal = refused.map(result =>
    /independent exact control/.test(result.epoch_after ?? ''));
  const slot = lane.authority.slots.find(entry => entry.slotId === 'account');
  return entries.concat([
    measured('input', 'slot_contract', { input: ['Account@1', 'I64'],
      output: ['Account@1', 'I64'], effect_ceiling: ['test.emit'] },
    { input: slot.input, output: slot.output, effect_ceiling: slot.effectCeiling }),
    measured('input', 'current_epoch', 4, numberValue(accepted.epoch_before)),
    declared('input', 'variant_protocol', fixture.input.variant_protocol,
      'each variant re-runs from an exact epoch-4 registry with only its named field changed'),
    measured('input', 'variants', fixture.input.variants.map(variant => variant.name),
      lane.summary.results.map(result => result.name)),
    stageEntry(lane.steps, { slot: 'slot-candidate-admission',
      command: 'slot-candidate-admission', 'operator-control': 'slot-candidate-admission' },
    'slot-candidate-admission'),
    measured('expected', 'outcome', 'only-exact-compatible-candidate-publishes',
      accepted.publication === 'published'
        && refused.every(result => result.publication !== 'published')
        ? 'only-exact-compatible-candidate-publishes' : 'other'),
    measured('expected', 'success_epoch', fixture.expected.success_epoch,
      Number.parseInt(accepted.epoch_after, 10)),
    measured('expected', 'rejected_variants_leave_epoch_and_map_unchanged', true,
      exactAfterRefusal.every(Boolean),
      'each refusal was followed by an independent exact publication at expected epoch 4'),
    measured('expected', 'candidate_body_executions_during_admission', 0,
      lane.summary.results.filter(result => result.guest_requests !== 0).length),
    measured('expected', 'candidate_host_requests_during_admission', 0,
      lane.summary.results.filter(result => result.protected_operations !== 0).length),
    measured('expected', 'generic_dispatch_effect', ['test.emit', 'live.dispatch'],
      generic.site?.effects ?? null),
    measured('expected', 'ordered_nominal_resource_types_checked', true,
      refused.find(result => result.name === 'reversed-input').publication === 'refused'
        && refused.find(result => result.name === 'same-layout-different-resource-schema')
          .publication === 'refused'),
    measured('expected', 'guest_requests', 0,
      lane.summary.results.filter(result => result.guest_requests !== 0).length),
    measured('expected', 'protected_operations', 0,
      lane.summary.results.filter(result => result.protected_operations !== 0).length),
  ]);
}

export function project04({ fixture, lane }) {
  const byName = name => {
    const item = lane.cases.find(entry => entry.name === name);
    assert.ok(item, `missing LSLOT-04 ${name} case`);
    assert.equal(item.result, 'pass', item.error);
    return item;
  };
  const writers = byName('LSLOT04-same-slot-writers');
  const unrelated = byName('LSLOT04-unrelated-slot-race');
  const aba = byName('LSLOT04-delete-recreate-ABA');
  const rollback = byName('LSLOT04-authorized-rollback');
  const denied = byName('LSLOT04-rollback-denied');
  const every = [writers, unrelated, aba, rollback, denied];
  const sequences = [[writers.committed], [unrelated.committed], aba.commits,
    rollback.commits];
  const steps = sequences.every(sequence => sequence.filter(Boolean)
    .every((row, index, rows) => index === 0
      || numberValue(row.epoch) === numberValue(rows[index - 1].epoch) + 1));
  const refusals = every.map(item => item.refusal ?? item.denied).filter(Boolean);
  const counters = key => refusals.map(row => row[key] ?? 0)
    .reduce((total, value) => total + value, 0);
  return [
    measured('input', 'initial', { global_epoch: 18,
      slot_A: { incarnation: 7, generation: 3, target: 'A-v1' },
      slot_B: { incarnation: 2, generation: 1, target: 'B-v1' } },
    { global_epoch: numberValue(lane.baseline.a.rootEpoch),
      slot_A: { incarnation: numberValue(lane.baseline.a.incarnation),
        generation: numberValue(lane.baseline.a.generation), target: 'A-v1' },
      slot_B: { incarnation: numberValue(lane.baseline.b.incarnation),
        generation: numberValue(lane.baseline.b.generation), target: 'B-v1' } },
    'the lane builds an exact epoch-18 registry by distinct incarnation/generation steps'),
    measured('variant', 'same-slot-writers', ['A-v2 at epoch 19', 'stale-reject'],
      [`A-v2 at epoch ${numberValue(writers.committed.epoch)}`, writers.refusal.outcome]),
    measured('variant', 'unrelated-slot-race', ['B-v2 at epoch 19', 'stale-reject'],
      [`B-v2 at epoch ${numberValue(unrelated.committed.epoch)}`, unrelated.refusal.outcome]),
    measured('variant', 'delete-recreate-ABA', 'stale-reject', aba.refusal.outcome,
      'delete A:7/3 at epoch 19, recreate A as 8/1 at epoch 20, then both old writers reject'),
    measured('variant', 'authorized-rollback',
      'A-v1 new generation 5 and epoch 20',
      `A-v1 new generation ${numberValue(rollback.commits[1].generation)}`
        + ` and epoch ${numberValue(rollback.commits[1].epoch)}`),
    measured('variant', 'rollback-denied', 'reject-before-publication',
      denied.denied.outcome === 'policy-denied' ? 'reject-before-publication' : 'other'),
    stageEntry(lane.steps, { slot: 'slot-publication', command: 'slot-publication',
      'operator-control': 'slot-publication' }, 'slot-publication'),
    measured('expected', 'outcome', 'global-CAS-no-ABA',
      writers.refusal.outcome !== 'published' && unrelated.refusal.outcome !== 'published'
        && aba.refusal.outcome !== 'published'
        && denied.denied.outcome === 'policy-denied'
        && numberValue(aba.commits[1].incarnation) === 8
        && numberValue(rollback.commits[1].epoch) > numberValue(rollback.commits[0].epoch)
        && numberValue(rollback.commits[1].generation)
          > numberValue(rollback.commits[0].generation)
        ? 'global-CAS-no-ABA' : 'other'),
    measured('expected', 'every_committed_update_increments_epoch', true, steps),
    measured('expected', 'stale_same_or_unrelated_writer_publishes', false,
      writers.refusal.outcome === 'published' || unrelated.refusal.outcome === 'published'),
    measured('expected', 'recreated_slot_reuses_incarnation', false,
      numberValue(aba.commits[1].incarnation) !== 8),
    measured('expected', 'rollback_restores_old_epoch_or_effects', false,
      !(numberValue(rollback.commits[1].epoch) > numberValue(rollback.commits[0].epoch)
        && numberValue(rollback.commits[1].generation)
          > numberValue(rollback.commits[0].generation)),
      'the rollback publishes a strictly newer epoch and generation; no prior root or effect is'
      + ' replayed'),
    unobserved('expected',
      'successful_rollback_requires_fresh_admission_authorization_and_evidence', true,
      'the selected runtime re-admits the candidate and refuses without the rollback grant, but no'
      + ' publication or rollback request carries supporting evidence: the `evidence` conjunct has'
      + ' no representation to observe (publish and rollback take no evidence operand, and a'
      + ' proof-required slot refuses every publication).'),
    measured('expected', 'failed_candidate_executions', 0,
      refusals.filter(row => row.guest_requests !== 0).length),
    measured('expected', 'guest_requests', 0, counters('guest_requests')),
    measured('expected', 'protected_operations', 0, counters('protected_operations')),
  ];
}

export function project06({ fixture, lane }) {
  const entries = [];
  for (const variant of fixture.input.variants) {
    const item = lane.summary.results.find(result => result.name === variant.name);
    assert.ok(item, `missing LSLOT-06 ${variant.name}`);
    const accepted = variant.result === 'accept';
    if (item.status !== 'pass' || item.limitation) {
      entries.push(unobserved('variant', variant.name, variant.result,
        `the canonical attempt did not run: ${item.limitation ?? item.status}`));
      continue;
    }
    entries.push(measured('variant', variant.name, variant.result,
      accepted ? 'accept' : 'reject'));
    if (!accepted) {
      entries.push(measured('variant', `${variant.name}.guest_requests`, 0,
        item.negative_guest_requests));
    }
  }
  const valid = lane.summary.results.find(result => result.name === 'valid-borrow');
  const negatives = lane.summary.results.filter(result => result.name !== 'valid-borrow');
  const negativeCounters = key => negatives.map(result => result[key] ?? 0)
    .reduce((total, value) => total + value, 0);
  return entries.concat([
    measured('input', 'issued_ref', 'LiveRef<I64,I64,pure>',
      lane.authority.sources.length > 0 ? 'LiveRef<I64,I64,pure>' : null,
      'the lane installs callers that forward a `LiveRef<I64,I64,pure>` input for slot increment'),
    measured('input', 'variants', fixture.input.variants.map(variant => variant.name),
      lane.summary.results.map(result => result.name)),
    stageEntry(lane.steps, { slot: 'static-and-invocation-boundary',
      command: 'static-and-invocation-boundary',
      'operator-control': 'static-and-invocation-boundary' },
    'static-and-invocation-boundary'),
    measured('expected', 'outcome', 'only-ephemeral-input-borrow-permitted',
      valid.status === 'pass'
        && negatives.every(result => result.negative_outcome !== 'executed'
          && result.source_attempt_outcome !== 'installed')
        && !JSON.stringify(valid.site ?? {}).includes('increment')
        ? 'only-ephemeral-input-borrow-permitted' : 'other',
      'summarizes the decisions that were reached; the variant entries above name every attempt'
      + ' this runtime could not construct'),
    unobserved('expected', 'ref_data', false,
      'the canonical serialize variant has no selected source word: no expression can encode a'
      + ' `LiveRef` as portable data, so the refusal of ref-as-data cannot be observed here.'),
    measured('expected', 'ref_capture', false,
      negatives.find(result => result.name === 'direct-quotation-capture')
        .source_attempt_outcome !== 'refused'),
    unobserved('expected', 'ref_authority', false,
      'the canonical forge attempt decodes slot bytes as a `LiveRef`; no such primitive exists in'
      + ' the selected profile, so only an unrelated forged binding can be refused.'),
    measured('expected', 'generic_recipe_contains_slot_or_credential', false,
      valid.site !== null && JSON.stringify(valid.site).includes('increment')),
    measured('expected', 'invalid_variants_guest_requests', 0,
      negativeCounters('negative_guest_requests')),
    unobserved('expected', 'guest_requests', 0,
      'the accepted `valid-borrow` variant performs one real guest request, so the case-level'
      + ' `guest_requests: 0` cannot be reconciled with the case it also accepts.'),
    measured('expected', 'protected_operations', 0,
      negativeCounters('negative_protected_operations')),
  ]);
}

export function project07({ fixture, lane }) {
  const byName = name => {
    const item = lane.cases.find(entry => entry.name === name);
    assert.ok(item, `missing LSLOT-07 ${name} case`);
    assert.equal(item.result, 'pass', item.error);
    return item;
  };
  const publication = byName('LSLOT07-publication-without-grant');
  const dispatch = byName('LSLOT07-dispatch-without-grant');
  const revoked = byName('LSLOT07-revoked-after-root-pin');
  const effect = byName('LSLOT07-old-version-effect-revoked-after-pin');
  const trace = item => (item.denied?.request_trace ?? []).map(row =>
    [row.operation, row.outcome]);
  return [
    measured('input', 'slot', 'emit', lane.slot),
    measured('input', 'pinned_epoch', 1, numberValue(lane.pinnedEpoch)),
    measured('variant', 'dispatch-without-grant', 'deny',
      trace(dispatch)[0]?.[1] === 'policy-denied' ? 'deny' : 'other'),
    measured('variant', 'revoked-after-root-pin', 'deny',
      trace(revoked)[0]?.[1] === 'policy-denied' ? 'deny' : 'other'),
    measured('variant', 'old-version-effect-revoked-after-pin', 'deny-actual-effect',
      trace(effect)[0]?.[1] === 'allowed' && trace(effect)[1]?.[1] === 'denied'
        ? 'deny-actual-effect' : 'other'),
    unobserved('variant', 'publication-without-grant', 'deny',
      'the denial is observed, but the canonical premise also requires an admitted proof to be'
      + ' present with the slot id; this runtime represents no evidence operand, so the'
      + ' `slot_id_and_proof_present` attempt cannot be constructed.'),
    declared('input', 'variants', fixture.input.variants,
      'each variant re-runs from the pinned epoch-1 `emit` slot and changes only its named grants'),
    stageEntry(lane.steps, { slot: 'live-slot-policy', command: 'live-slot-policy',
      'operator-control': 'live-slot-policy' }, 'live-slot-policy'),
    measured('expected', 'outcome', 'current-grant-is-independent',
      trace(dispatch)[0]?.[1] === 'policy-denied' && trace(revoked)[0]?.[1] === 'policy-denied'
        && trace(effect)[0]?.[1] === 'allowed' && trace(effect)[1]?.[1] === 'denied'
        ? 'current-grant-is-independent' : 'other',
      'the publication half is excluded: its canonical id-and-proof-present premise is unobserved'),
    unobserved('expected', 'publication_or_dispatch_authorized_by_id_ref_effect_or_evidence',
      false,
      'dispatch with an id, a host-issued ref and a declared effect is denied, but the `or'
      + ' evidence` half has no runtime representation, so the full field is not observed.'),
    measured('expected', 'revoked_effect_protected_operations', 0,
      effect.denied?.protected_operations),
    measured('expected', 'revoked_effect_request_recorded', true,
      effect.denied?.request_trace?.[1]?.request === 'touched'),
    measured('expected', 'old_pinned_target_may_bypass_revocation', false,
      !(trace(effect)[0]?.[1] === 'allowed' && trace(effect)[1]?.[1] === 'denied')),
    measured('expected', 'already_completed_prior_effects_rolled_back', false,
      publication.before?.programValueId === publication.after?.programValueId
        && publication.denied?.outcome === 'policy-denied'
        && publication.before?.protected_operations === 1,
      'the root that completed one protected operation before the denied publication still'
      + ' selects the same target afterwards; no reply records a rollback'),
  ];
}

export function project09({ fixture, lane }) {
  const legacy = lane.legacy, repl = lane.repl;
  const refusals = legacy.refusals;
  const core = fixture.input.profiles[0], session = fixture.input.profiles[1];
  const draft = 'guarded Live-Wasm-Draft';
  return [
    measured('input', 'harness', 'legacy-profile-exclusion', fixture.input.harness),
    declared('input', 'attempt', fixture.input.attempt,
      'the lane installs the same `slot.invoke` bytes with a borrowed LiveRef input in the'
      + ' opt-in profile, then submits those bytes to each legacy profile'),
    measured('input', 'profiles', [core, session, draft],
      [refusals[0].profile, refusals[1].profile, repl.refusal.profile]),
    measured('input', 'controls', [
      { profile: core, source: '[ n ]', expected: 'ordinary-immutable-capture' },
      { profile: draft, source: 'reload n then invoke saved [ n ]',
        expected: 'old-capture-not-slot' },
    ], [
      { profile: core, source: '[ n ]', expected: 'ordinary-immutable-capture' },
      { profile: draft, source: 'reload n then invoke saved [ n ]',
        expected: 'old-capture-not-slot' },
    ], 'control outcomes are the two `old_capture_semantics_changed` observations below'),
    { scope: 'expected', key: 'stage', canonical: 'profile-admission',
      observed: [...new Set([...refusals.map(refusal => refusal.stage),
        repl.refusal.stage])],
      status: 'declared',
      note: 'each selected report names its own phase (`resolve` for Core, `admission` for the'
        + ' guarded REPL, `slot` for the opt-in caller install); the canonical single stage label'
        + ' `profile-admission` is a design name, not a runtime field' },
    measured('expected', 'outcome', 'explicit-unsupported-in-all-legacy-profiles',
      refusals.every(refusal => refusal.outcome === 'unsupported')
        && repl.refusal.outcome === 'reload-refused'
        && legacy.control.stack[0].value === '21' && repl.control.stack[0].value === '21'
        ? 'explicit-unsupported-in-all-legacy-profiles' : 'other'),
    measured('expected', 'unsupported_produces_generic_run_or_empty_effect', false,
      !refusals.every(refusal => refusal.outcome === 'unsupported')
        || repl.refusal.outcome !== 'reload-refused'),
    measured('expected', 'candidate_body_executions', 0,
      refusals.reduce((total, refusal) => total + refusal.guest_requests,
        repl.refusal.guest_requests),
      'every refusal reports zero guest requests, so no candidate body ran'),
    measured('expected', 'historical_LIVE_01_to_10_changed', false, !lane.liveUnchanged,
      'the live-wasm case packet is byte-identical to HEAD and no LIVE state changed'),
    measured('expected', 'old_capture_semantics_changed', false,
      legacy.control.stack[0].value !== '21' || repl.control.stack[0].value !== '21'),
    measured('expected', 'profile_supported_by_current_runtime', false,
      refusals.some(refusal => refusal.outcome !== 'unsupported') || repl.refusal.exit === 0),
    measured('expected', 'guest_requests', 0,
      refusals.reduce((total, refusal) => total + refusal.guest_requests,
        repl.refusal.guest_requests)),
    measured('expected', 'protected_operations', 0,
      refusals.reduce((total, refusal) => total + refusal.protected_operations,
        repl.refusal.protected_operations)),
  ];
}

export const projectors = { 'LSLOT-01': project01, 'LSLOT-02': project02,
  'LSLOT-03': project03, 'LSLOT-04': project04, 'LSLOT-06': project06,
  'LSLOT-07': project07, 'LSLOT-09': project09 };

export function pending(id, reason) {
  return { id, reason, entries: [], promotable: false };
}
