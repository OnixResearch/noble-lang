// Projections from selected live-slot lane evidence onto the canonical
// LSLOT case records in `specs/conformance/live-reference-cases.json`.
//
// Every canonical leaf path of a case's `input` and `expected`, and every
// variant, gets exactly one entry. `gate.mjs` enforces that coverage and fails
// when a leaf has no entry.
//
// Entry status:
//   observed  — a lane measured the exact fact, and it equals the canonical value
//   mismatch  — a lane measured the exact fact, and it differs (gate failure)
//   unobserved— a machine-expressible premise whose canonical attempt did not run
//   declared  — prose/design text no runtime field can carry; the entry records
//               the runtime facts that stand behind it, and only keys on the
//               per-case allowlist below may use it
import assert from 'node:assert/strict';
import fs from 'node:fs';

export const jsonLines = file => fs.readFileSync(file, 'utf8').trim()
  .split('\n').filter(Boolean).map(line => JSON.parse(line));
const same = (left, right) => JSON.stringify(left) === JSON.stringify(right);

export function leaves(value, prefix) {
  if (Array.isArray(value) || value === null || typeof value !== 'object') return [prefix];
  return Object.entries(value).flatMap(([key, child]) => leaves(child, `${prefix}.${key}`));
}

function measured(scope, key, canonical, observed, note = null, evidence = null) {
  return { scope, key, canonical, observed,
    status: same(canonical, observed) ? 'observed' : 'mismatch', note, evidence };
}
function unobserved(scope, key, canonical, reason) {
  return { scope, key, canonical, observed: null, status: 'unobserved', note: reason };
}
function declared(scope, key, canonical, basis, evidence = null) {
  return { scope, key, canonical, observed: null, status: 'declared', note: basis, evidence };
}
const numberValue = value => Number(value);
// A canonical variant is one entry: its recorded predicates must all hold, and
// the runtime facts they read are returned as evidence.
function variantObservation(variant, predicate) {
  assert.ok(predicate, `variant ${variant.name} has a runtime predicate`);
  const holds = predicate();
  return measured('variant', variant.name, variant, holds ? variant : { failed: variant.name },
    holds ? 'every recorded runtime predicate for this variant holds' : 'predicate failed');
}

// Canonical prose keys that may be `declared`, with the runtime facts that
// stand behind each. Any other key must be observed or unobserved.
export const declaredAllowlist = {
  'LSLOT-01': ['input.harness', 'input.generic_caller', 'expected.stage',
    'input.publication.candidate', 'input.slot.target'],
  'LSLOT-02': ['input.harness', 'input.base.registry_epoch_1.A.body',
    'input.base.root_A_entry.ordered_host_issued_inputs',
    'input.transitive_variant.independent_fixture',
    'input.base.root_A_entry.pin_operation', 'input.base.root_A_entry.caller',
    'input.transitive_variant.registry_epoch_1.A.body',
    'input.transitive_variant.registry_epoch_1.B.body', 'expected.stage',
    'input.base.interleaving', 'input.transitive_variant.interleaving',
    'input.transitive_variant.root_A_entry.ordered_host_issued_inputs',
    'input.transitive_variant.root_A_entry.borrow_pass_through'],
  'LSLOT-03': ['input.harness', 'input.variant_protocol', 'expected.stage'],
  'LSLOT-04': ['input.harness', 'expected.stage'],
  'LSLOT-06': ['input.harness', 'input.issued_ref', 'expected.stage'],
  'LSLOT-07': ['input.harness', 'input.old_target', 'expected.stage'],
  'LSLOT-09': ['input.harness', 'input.attempt', 'input.profiles', 'input.controls',
    'expected.stage'],
};

// One declaration per case pins the mapping from runtime report stages to the
// canonical single phase label, and the raw stages the lanes reported.
function stageEntry(rawStages, mapping, canonical) {
  const unmapped = rawStages.filter(stage => !Object.hasOwn(mapping, stage));
  const evidence = { raw_stages: [...new Set(rawStages)].sort(), mapping };
  if (unmapped.length > 0) {
    return { scope: 'expected', key: 'stage', canonical, observed: evidence,
      status: 'mismatch', note: `unmapped runtime stage: ${unmapped.join(', ')}`, evidence };
  }
  if (new Set(rawStages.map(stage => mapping[stage])).size !== 1) {
    return { scope: 'expected', key: 'stage', canonical, observed: evidence,
      status: 'mismatch', note: 'runtime stages map to more than one canonical phase', evidence };
  }
  return declared('expected', 'stage', canonical,
    'the runtime reports its own phase names; this mapping is the declared case phase',
    evidence);
}
const stepStages = steps => steps.map(step => step.row.stage);
// One entry per canonical interface leaf: `input`, `output`, `effects`.
function interfaceEntries(prefix, canonical, observed, third = 'effects') {
  return [['input', canonical.input, observed.input],
    ['output', canonical.output, observed.output],
    [third, canonical[third], observed[third]]].map(([leaf, want, seen]) =>
    measured(prefix, leaf, want, seen));
}

// Parse `LiveRef<IN,OUT,EFF>` with `+`-joined inputs and a flat effect set, and
// spell it the way the canonical design does: `borrow LiveRef<[..],[..],{..}>`.
export function normalizeType(type) {
  const text = String(type).trim();
  if (!text.startsWith('LiveRef<')) return text;
  const body = text.slice('LiveRef<'.length, -1);
  const parts = splitTop(body, ',');
  assert.equal(parts.length, 3, `three LiveRef components: ${type}`);
  const inputs = splitTop(parts[0], '+').map(normalizeType);
  const outputs = splitTop(parts[1], '+').map(normalizeType);
  const effects = splitTop(parts[2].trim(), '+')
    .filter(effect => effect.length > 0 && effect !== 'pure').sort();
  const spell = list => list.length === 1 ? list[0] : `[${list.join(',')}]`;
  return `borrow LiveRef<${spell(inputs)},${spell(outputs)},{${effects.join(',')}}>`;
}
function splitTop(text, separator) {
  const parts = [];
  let depth = 0;
  let current = '';
  for (const character of text) {
    if (character === '<' || character === '[') depth += 1;
    if (character === '>' || character === ']') depth -= 1;
    if (character === separator && depth === 0) {
      parts.push(current);
      current = '';
    } else current += character;
  }
  parts.push(current);
  return parts;
}

const paired = records => {
  const requests = records.filter(row => row.kind === 'operator').map(row => row.request);
  const replies = records.filter(row => row.kind === 'cli').map(row => row.row);
  assert.equal(replies.length, requests.length + 1,
    'every operator request has one CLI reply plus the configuration report');
  return [{ request: null, row: replies[0] },
    ...replies.slice(1).map((row, index) => ({ request: requests[index], row }))];
};
const stepAt = (steps, label, predicate) => {
  const matches = steps.filter(predicate);
  assert.equal(matches.length, 1, `unique ${label} step`);
  return matches[0];
};

export function project01({ fixture, lane }) {
  const steps = paired(lane.records);
  const define = stepAt(steps, 'define', step => step.request?.operation === 'define');
  const savedInstall = stepAt(steps, 'saved install',
    step => step.request?.operation === 'install' && step.request.id === 'saved');
  const genericInstall = stepAt(steps, 'generic install',
    step => step.request?.operation === 'install' && step.request.id === 'generic');
  const originalInstall = stepAt(steps, 'original install',
    step => step.request?.operation === 'install' && step.request.id === 'original');
  const replacementInstall = stepAt(steps, 'replacement install',
    step => step.request?.operation === 'install' && step.request.id === 'replacement');
  const firstPublish = stepAt(steps, 'first publish',
    step => step.request?.operation === 'publish' && step.request.id === 'original');
  const secondPublish = stepAt(steps, 'second publish',
    step => step.request?.operation === 'publish' && step.request.id === 'replacement');
  const ordinaryRoot = stepAt(steps, 'ordinary run',
    step => step.request?.operation === 'invoke' && step.request.id === 'runner');
  const genericCalls = steps.filter(step =>
    step.request?.operation === 'invoke' && step.request.id === 'generic');
  assert.equal(genericCalls.length, 2, 'one generic call before and one after publication');
  const [beforeCall, afterCall] = genericCalls;
  const reflections = steps.filter(step => step.request?.operation === 'reflect');
  assert.ok(reflections.length >= 2);
  const site = reflections[0].row.site;
  const stepIndexOf = step => steps.indexOf(step);
  const ordering = { saved: stepIndexOf(ordinaryRoot),
    publication: stepIndexOf(secondPublish),
    generic: stepIndexOf(afterCall) };
  assert.ok(ordering.saved > ordering.publication && ordering.generic > ordering.saved,
    'the saved and generic calls follow the publication, in that order');
  const savedResult = numberValue(ordinaryRoot.row.stack[0].value);
  const genericResult = numberValue(afterCall.row.stack[0].value);
  const prePublicationResult = numberValue(beforeCall.row.stack[0].value);
  const freshTarget = afterCall.row.request_trace[0]?.artifactSha256
    === replacementInstall.row.artifact_sha256;
  const oldTarget = ordinaryRoot.row.request_trace.length === 0
    && savedResult === prePublicationResult
    && beforeCall.row.request_trace[0]?.artifactSha256 === originalInstall.row.artifact_sha256;
  const definitionBody = /^def \S+ \[ (.*) \]$/.exec(define.request.source)[1];
  const replacementBody = /^\[ (.*) \]$/.exec(replacementInstall.request.source)[1];
  const siteText = JSON.stringify(site);
  return [
    declared('input', 'harness', fixture.input.harness,
      'the gate runs this case\'s capture lane against the selected CLI',
      { lane: 'crates/noble-cli/tests/live_slot_capture_legacy.mjs' }),
    measured('input', 'definition', fixture.input.definition, define.request.source),
    measured('input', 'saved_program', fixture.input.saved_program,
      savedInstall.request.source),
    declared('input', 'generic_caller', fixture.input.generic_caller,
      'prose design value; the checked caller interface and effects it names are recorded here',
      { install_inputs: genericInstall.request.inputs,
        reflected: { instruction: site.instruction, input: site.input,
          output: site.output, effects: site.effects } }),
    measured('input.slot', 'id', 'increment', firstPublish.request.slot),
    measured('input.slot', 'incarnation', 1, numberValue(firstPublish.row.incarnation)),
    measured('input.slot', 'generation', 1, numberValue(firstPublish.row.generation)),
    declared('input.slot', 'target', fixture.input.slot.target,
      'the registered version name and the definition body the first publication selected',
      { version: `n-v${numberValue(firstPublish.row.generation)}`,
        definition_source: define.request.source, body: definitionBody,
        artifact_sha256: originalInstall.row.artifact_sha256 }),
    measured('input.publication', 'expected_epoch',
      fixture.input.publication.expected_epoch,
      numberValue(secondPublish.request.expected_epoch)),
    measured('input.publication', 'expected_incarnation',
      fixture.input.publication.expected_incarnation,
      numberValue(secondPublish.request.expected_incarnation)),
    measured('input.publication', 'expected_generation',
      fixture.input.publication.expected_generation,
      numberValue(secondPublish.request.expected_generation)),
    declared('input.publication', 'candidate', fixture.input.publication.candidate,
      'the replacement quotation the lane installed and published as the next version',
      { install_source: replacementInstall.request.source, body: replacementBody,
        artifact_sha256: replacementInstall.row.artifact_sha256,
        generation: numberValue(secondPublish.row.generation) }),
    measured('input.publication', 'host_authorized',
      fixture.input.publication.host_authorized,
      secondPublish.row.outcome === 'published'),
    measured('input', 'calls_after_publication', fixture.input.calls_after_publication, [
      { subject: 'saved_program',
        argument: numberValue(ordinaryRoot.request.inputs[0].value) },
      { subject: 'generic_caller',
        argument: numberValue(afterCall.request.inputs[0].value),
        host_issued_input_ref: afterCall.request.refs[0]?.slot },
    ], `observed step order: publication ${ordering.publication}, saved ${ordering.saved},`
      + ` generic ${ordering.generic}`),
    stageEntry(stepStages(steps), { slot: 'live-slot-root', command: 'live-slot-root',
      source: 'live-slot-root' }, 'live-slot-root'),
    measured('expected', 'outcome', 'immutable-capture-and-explicit-fresh-target',
      oldTarget && freshTarget && savedResult === 21 && genericResult === 22
        && ordering.generic > ordering.saved
        ? 'immutable-capture-and-explicit-fresh-target' : 'other'),
    measured('expected', 'saved_result', fixture.expected.saved_result, savedResult),
    measured('expected', 'generic_result', fixture.expected.generic_result, genericResult),
    measured('expected', 'saved_target', 'n-v1', oldTarget ? 'n-v1' : 'other',
      'the saved owner runs the pre-publication target and records no dispatch trace'),
    measured('expected', 'generic_selected_target', 'n-v2', freshTarget ? 'n-v2' : 'other'),
    measured('expected', 'caller_identity_unchanged', fixture.expected.caller_identity_unchanged,
      same(site, reflections.at(-1).row.site)),
    measured('expected', 'ordinary_run_dynamic_lookup', false,
      ordinaryRoot.row.request_trace.length > 0 || ordinaryRoot.row.guest_requests !== 0),
    measured('expected', 'generic_recipe_embeds_slot_or_target', false,
      siteText.includes('increment') || siteText.includes('slotId')
        || siteText.includes('artifactSha256')),
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
  const installSource = (item, id) => item.install_requests.find(request => request.id === id)
    .source;
  const generationsOf = item => Object.fromEntries(item.bootstrap.slots
    .map(slot => [slot.slot, numberValue(slot.generation)]));
  const contract = (item, slot) => {
    const found = item.authority.slots.find(entry => entry.slotId === slot);
    return { input: found.input.map(normalizeType), output: found.output.map(normalizeType),
      effects: found.effectCeiling };
  };
  const versions = (item, ids, generations) => Object.fromEntries(ids.map(id =>
    [id.slot, id.body === null ? `${id.slot}-v${generations[id.slot]}`
      : `${id.slot}-v${generations[id.slot]}:[ ${installSource(item, id.id)} ]`]));
  const baseVersions = versions(base, [{ slot: 'B', id: 'B1', body: true }],
    generationsOf(base));
  const transitiveIds = [{ slot: 'A', id: 'A1', body: null },
    { slot: 'B', id: 'B1', body: null }, { slot: 'C', id: 'C1', body: true }];
  const transitiveVersions = versions(transitive, transitiveIds,
    generationsOf(transitive));
  const labelOf = (item, entry) => {
    const id = Object.keys(item.installed_sha256)
      .find(key => item.installed_sha256[key] === entry.artifactSha256);
    assert.ok(id, `${entry.slotId} dispatch selects an installed artifact`);
    return `${entry.slotId}-v${numberValue(entry.generation)}`;
  };
  const generations = item => [item.old_root, item.new_root, item.next_leaf];
  const dispatchPolicy = item => generations(item)
    .flatMap(root => root.request_trace)
    .every(entry => entry.operation === 'dispatch' && entry.policyAllowed === true
      && entry.policyInput.operation === 'dispatch'
      && entry.policyInput.slotId === entry.slotId
      && entry.policyInput.rootEpoch === entry.rootEpoch
      && entry.policyInput.version.artifactSha256 === entry.artifactSha256);
  const baseB = base.old_root.request_trace.filter(entry => entry.slotId === 'B');
  assert.equal(baseB.length, 2, 'the base root performs exactly two B dispatches');
  const refs = item => item.invoke_requests
    .find(request => request.operation === 'invoke' && request.id === 'callerA');
  const orderedRefs = (item) => {
    const request = refs(item);
    const caller = item.install_requests.find(entry => entry.id === 'callerA');
    return [request.inputs.map(input => `I64:${input.value}`)[0],
      ...caller.inputs.slice(1).map((type, index) =>
        `borrow ${normalizeType(type)} ref issued for ${request.refs[index].slot}`)];
  };
  const distinctEpochs = [base, transitive].every(item =>
    [item.old_root, item.new_root].every(root =>
      root.request_trace.every(entry => entry.rootEpoch === root.epoch)));
  const noSlotIdentity = item => {
    const request = refs(item);
    const caller = item.install_requests.find(entry => entry.id === 'callerA');
    const targets = Object.keys(item.installed_sha256)
      .filter(id => /^[ABC]1$/.test(id))
      .map(id => item.install_requests.find(entry => entry.id === id).source);
    const bound = request.refs.map(ref => ref.slot);
    const positioned = item.old_root.request_trace
      .every(entry => bound.includes(entry.slotId));
    const sources = [caller.source, ...targets].every(source =>
      !/\b(?:A|B|C)\b/.test(source) || source === 'slot.invoke' || source === '1 +'
        || source === '2 +');
    return { violated: !positioned || !sources, positioned };
  };
  const baseInterleave = [
    `A-v1 borrows its explicit B ref and invokes ${labelOf(base, baseB[0])} once`,
    `authorized B-v2:[ ${installSource(base, 'B2')} ] publishes registry epoch`
      + ` ${numberValue(base.committed_ack.epoch)}`,
    `A-v1 borrows same root-scoped B ref and invokes B again`,
    `new root supplies a newly host-issued B ref and I64:20 to generic caller`,
  ];
  const baseInterleaveMatches = [
    baseB[0].generation === '1' && baseB[0].rootEpoch === base.old_root.epoch,
    numberValue(base.committed_ack.epoch) === 2
      && base.old_root.control_events?.some(event => event.operation === 'publish'
        && event.scope === 'B' && event.epoch === base.committed_ack.epoch),
    baseB[1].generation === '1'
      && baseB[1].programValueId === baseB[0].programValueId
      && baseB[1].rootEpoch === baseB[0].rootEpoch,
    base.new_root.epoch === base.committed_ack.epoch
      && base.new_root.request_trace.filter(entry => entry.slotId === 'B')
        .every(entry => entry.generation === '2')
      && base.invoke_requests.some(request => request.operation === 'invoke'
        && request.id === 'callerA' && request.refs?.some(ref => ref.slot === 'B')),
  ];
  const transitiveTrace = transitive.old_root.request_trace;
  const transitiveInterleave = [
    `A root pins epoch ${numberValue(transitive.old_root.epoch)}`,
    `authorized C-v2:[ ${installSource(transitive, 'C2')} ] publishes epoch`
      + ` ${numberValue(transitive.committed_ack.epoch)}`,
    `${transitiveTrace[0]?.slotId} invokes ${transitiveTrace[1]?.slotId}`
      + ` which invokes ${transitiveTrace[2]?.slotId}`,
    `new root receives freshly host-issued C ref and I64:20`,
  ];
  const transitiveInterleaveMatches = [
    numberValue(transitive.old_root.epoch) === 1,
    numberValue(transitive.committed_ack.epoch) === 2
      && transitive.old_root.control_events?.some(event => event.operation === 'publish'
        && event.scope === 'C' && event.epoch === transitive.committed_ack.epoch),
    transitiveTrace.map(entry => entry.slotId).join(',') === 'A,B,C'
      && transitiveTrace.every(entry => entry.rootEpoch === transitive.old_root.epoch),
    transitive.new_root.epoch === transitive.committed_ack.epoch
      && transitive.new_root.request_trace.filter(entry => entry.slotId === 'C')
        .every(entry => entry.generation === '2'),
  ];
  const callerOf = item => {
    const caller = item.install_requests.find(entry => entry.id === 'callerA');
    const site = item.reflected_sites.find(entry => entry.id === 'callerA').site;
    return { caller, site, caller_source: caller.source };
  };
  const baseCaller = callerOf(base), transitiveCaller = callerOf(transitive);
  const baseOrdered = orderedRefs(base), transitiveOrdered = orderedRefs(transitive);
  const baseCanonicalOrdered = fixture.input.base.root_A_entry.ordered_host_issued_inputs;
  const transitiveCanonicalOrdered =
    fixture.input.transitive_variant.root_A_entry.ordered_host_issued_inputs;
  const baseOrderedMatch = baseOrdered.length === baseCanonicalOrdered.length
    && baseOrdered[0] === 'I64:20'
    && refs(base).refs.map(ref => ref.slot).join(',') === 'B,A';
  const transitiveOrderedMatch = transitiveOrdered.length
      === transitiveCanonicalOrdered.length
    && transitiveOrdered[0] === 'I64:20'
    && refs(transitive).refs.map(ref => ref.slot).join(',') === 'C,B,A'
    && transitiveOrdered[1] === `borrow ${normalizeType(
      transitiveCaller.caller.inputs[1])} ref issued for C`
    && transitiveOrdered[2] === `borrow ${normalizeType(
      transitiveCaller.caller.inputs[2])} ref issued for B`
    && transitiveOrdered[3] === `borrow ${normalizeType(
      transitiveCaller.caller.inputs[3])} ref issued for A`;
  const separate = lane.crossProfile;
  return [
    declared('input', 'harness', fixture.input.harness,
      'the gate runs the frozen nested-root lane against the selected CLI'),
    measured('input.base.registry_epoch_1.A', 'version', 'A-v1',
      `A-v${generationsOf(base).A}`),
    ...interfaceEntries('input.base.registry_epoch_1.A.interface',
      fixture.input.base.registry_epoch_1.A.interface, contract(base, 'A')),
    measured('input.base.registry_epoch_1.B', 'version', 'B-v1:[ 1 + ]', baseVersions.B),
    ...interfaceEntries('input.base.registry_epoch_1.B.interface',
      fixture.input.base.registry_epoch_1.B.interface, contract(base, 'B')),
    declared('input.base.registry_epoch_1.A', 'body',
      fixture.input.base.registry_epoch_1.A.body,
      'prose body; the installed caller source and its reflected site carry the behavior',
      { source: baseCaller.caller_source, site: baseCaller.site }),
    declared('input.base.root_A_entry', 'ordered_host_issued_inputs',
      fixture.input.base.root_A_entry.ordered_host_issued_inputs,
      'prose list; the invocation request\'s scalar and ordered refs are recorded here',
      { observed: baseOrdered, ref_slots: refs(base).refs.map(ref => ref.slot),
        matches_positions: baseOrderedMatch }),
    measured('input.base.root_A_entry', 'pin_epoch', 1, numberValue(base.old_root.epoch)),
    declared('input.base.root_A_entry', 'pin_operation',
      fixture.input.base.root_A_entry.pin_operation,
      'the root keeps one epoch for both dispatches; the next root observes the next epoch',
      { old_epoch: numberValue(base.old_root.epoch),
        new_epoch: numberValue(base.new_root.epoch) }),
    declared('input.base.root_A_entry', 'caller',
      fixture.input.base.root_A_entry.caller,
      'prose caller description; the reflected generic site and result are recorded here',
      { site: baseCaller.site, result: numberValue(base.old_root.stack[0].value) }),
    measured('input.base', 'interleaving', fixture.input.base.interleaving,
      baseInterleaveMatch(baseInterleaveMatches) ? fixture.input.base.interleaving
        : baseInterleave,
      baseInterleaveMatches.map((value, index) => `${index + 1}:${value}`).join(' ')),
    ...interfaceEntries('input.base.new_root_B_interface',
      fixture.input.base.new_root_B_interface,
      { input: base.next_leaf.request_trace[0].policyInput.version.input.map(normalizeType),
        output: base.next_leaf.request_trace[0].policyInput.version.output
          .map(normalizeType),
        effects: base.next_leaf.request_trace[0].policyInput.version.effects }),
    declared('input.transitive_variant', 'independent_fixture',
      fixture.input.transitive_variant.independent_fixture,
      'the variant re-runs in its own session: configured epoch 0, then bootstrap epoch 1',
      { separate_session: true,
        configured_epoch: base.configured_epoch, base_bootstrap_epoch: base.bootstrap.epoch,
        transitive_configured_epoch: transitive.configured_epoch,
        transitive_bootstrap_epoch: transitive.bootstrap.epoch }),
    measured('input.transitive_variant.registry_epoch_1.A', 'version', 'A-v1',
      `A-v${generationsOf(transitive).A}`),
    ...interfaceEntries('input.transitive_variant.registry_epoch_1.A.interface',
      fixture.input.transitive_variant.registry_epoch_1.A.interface,
      contract(transitive, 'A')),
    declared('input.transitive_variant.registry_epoch_1.A', 'body',
      fixture.input.transitive_variant.registry_epoch_1.A.body,
      'prose body; the installed caller source and its reflected site carry the behavior',
      { source: transitiveCaller.caller_source, site: transitiveCaller.site }),
    measured('input.transitive_variant.registry_epoch_1.B', 'version', 'B-v1',
      transitiveVersions.B),
    ...interfaceEntries('input.transitive_variant.registry_epoch_1.B.interface',
      fixture.input.transitive_variant.registry_epoch_1.B.interface,
      contract(transitive, 'B')),
    declared('input.transitive_variant.registry_epoch_1.B', 'body',
      fixture.input.transitive_variant.registry_epoch_1.B.body,
      'prose body; the B caller source is recorded here',
      { source: installSource(transitive, 'B1') }),
    measured('input.transitive_variant.registry_epoch_1.C', 'version', 'C-v1:[ 1 + ]',
      transitiveVersions.C),
    ...interfaceEntries('input.transitive_variant.registry_epoch_1.C.interface',
      fixture.input.transitive_variant.registry_epoch_1.C.interface,
      contract(transitive, 'C')),
    declared('input.transitive_variant.root_A_entry', 'ordered_host_issued_inputs',
      transitiveCanonicalOrdered,
      'prose list; the invocation request\'s scalar and typed ordered refs are recorded here',
      { observed: transitiveOrdered,
        ref_slots: refs(transitive).refs.map(ref => ref.slot),
        matches_positions: transitiveOrderedMatch }),
    measured('input.transitive_variant.root_A_entry', 'pin_epoch', 1,
      numberValue(transitive.old_root.epoch)),
    declared('input.transitive_variant.root_A_entry', 'borrow_pass_through',
      fixture.input.transitive_variant.root_A_entry.borrow_pass_through,
      'prose steps; the old-root A/B/C dispatch order is recorded here',
      { dispatch_order: transitiveTrace.map(entry => entry.slotId) }),
    measured('input.transitive_variant', 'interleaving',
      fixture.input.transitive_variant.interleaving,
      transitiveInterleaveMatch(transitiveInterleaveMatches)
        ? fixture.input.transitive_variant.interleaving : transitiveInterleave,
      transitiveInterleaveMatches.map((value, index) => `${index + 1}:${value}`).join(' ')),
    stageEntry(lane.stages, { slot: 'live-slot-root', command: 'live-slot-root',
      source: 'live-slot-root', 'operator-control': 'live-slot-root' }, 'live-slot-root'),
    measured('expected', 'outcome', 'one-map-per-root-with-explicit-typed-ref-pass-through',
      numberValue(base.old_root.epoch) === numberValue(base.new_root.epoch) - 1
        && numberValue(transitive.old_root.epoch)
          === numberValue(transitive.new_root.epoch) - 1
        ? 'one-map-per-root-with-explicit-typed-ref-pass-through' : 'other'),
    measured('expected', 'base_root_A_B_selections', ['B-v1', 'B-v1'],
      baseB.map(entry => labelOf(base, entry)),
      'both same-root B dispatches, in order'),
    measured('expected', 'base_root_A_result', fixture.expected.base_root_A_result,
      numberValue(base.old_root.stack[0].value)),
    measured('expected', 'base_next_root_B_selection', 'B-v2',
      labelOf(base, base.next_leaf.request_trace.at(-1))),
    measured('expected', 'base_next_root_B_result', fixture.expected.base_next_root_B_result,
      numberValue(base.next_leaf.stack[0].value)),
    measured('expected', 'variant_old_root_A_B_C_selections', ['A-v1', 'B-v1', 'C-v1'],
      transitiveTrace.map(entry => labelOf(transitive, entry))),
    measured('expected', 'variant_old_root_result', fixture.expected.variant_old_root_result,
      numberValue(transitive.old_root.stack[0].value)),
    measured('expected', 'variant_next_root_C_selection', 'C-v2',
      labelOf(transitive, transitive.next_leaf.request_trace.at(-1))),
    measured('expected', 'variant_next_root_C_result',
      fixture.expected.variant_next_root_C_result,
      numberValue(transitive.next_leaf.stack[0].value)),
    measured('expected', 'mixed_registry_epochs_within_root', false, !distinctEpochs),
    measured('expected', 'implicit_source_name_or_captured_ref_lookup', false,
      noSlotIdentity(base).violated || noSlotIdentity(transitive).violated,
      'the dispatched slot equals the host ref bound at that position, and no caller or target'
      + ' source nor reflected site carries a slot identity',
      { base_positioned: noSlotIdentity(base).positioned,
        transitive_positioned: noSlotIdentity(transitive).positioned }),
    measured('expected', 'current_authorization_rechecked_at_each_dispatch', true,
      dispatchPolicy(base) && dispatchPolicy(transitive)),
    measured('expected',
      'registry_epoch_distinct_from_live_wasm_namespace_or_source_generation', true,
      [base, transitive].every(item => numberValue(item.old_root.epoch) === 1
        && numberValue(item.new_root.epoch) === 2),
      `slot registry epochs ${separate.registryEpochs.join('/')} advance only on publication;`
      + ` the guarded Live-Wasm-Draft lane of the same run reports its own source generations`
      + ` ${separate.liveGenerations.join('/')} and carries no registry epoch`,
      { slot_epochs: [numberValue(base.old_root.epoch), numberValue(base.new_root.epoch),
        numberValue(transitive.old_root.epoch), numberValue(transitive.new_root.epoch)],
        live_generations: separate.liveGenerations,
        live_reports_have_epoch: separate.liveReportsHaveEpoch,
        refused_generation: separate.refusedSlotGeneration }),
  ];
}
const baseInterleaveMatch = matches => matches.every(Boolean);
const transitiveInterleaveMatch = matches => matches.every(Boolean);

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
  }
  const accepted = lane.summary.results
    .find(result => result.name === 'same-exact-interface');
  const refused = lane.summary.results.filter(result => result.name !== 'same-exact-interface');
  const slot = lane.authority.slots.find(entry => entry.slotId === 'account');
  return entries.concat([
    declared('input', 'harness', fixture.input.harness,
      'the gate runs the exact-admission lane against the selected CLI'),
    ...interfaceEntries('input.slot_contract',
      { input: fixture.input.slot_contract.input,
        output: fixture.input.slot_contract.output,
        effect_ceiling: fixture.input.slot_contract.effect_ceiling },
      { input: slot.input, output: slot.output, effect_ceiling: slot.effectCeiling },
    'effect_ceiling'),
    measured('input', 'current_epoch', 4, numberValue(accepted.epoch_before)),
    declared('input', 'variant_protocol', fixture.input.variant_protocol,
      'each variant re-runs from an exact epoch-4 registry with only its named field changed'),
    measured('input', 'variants', fixture.input.variants.map(variant => variant.name),
      lane.summary.results.map(result => result.name)),
    stageEntry(lane.stageSteps, { slot: 'slot-candidate-admission',
      command: 'slot-candidate-admission', 'operator-control': 'slot-candidate-admission' },
    'slot-candidate-admission'),
    measured('expected', 'outcome', 'only-exact-compatible-candidate-publishes',
      accepted.publication === 'published'
        && refused.every(result => result.publication !== 'published')
        ? 'only-exact-compatible-candidate-publishes' : 'other'),
    measured('expected', 'success_epoch', fixture.expected.success_epoch,
      Number.parseInt(accepted.epoch_after, 10)),
    measured('expected', 'rejected_variants_leave_epoch_and_map_unchanged', true,
      refused.every(result => /independent exact control/.test(result.epoch_after ?? '')),
      'each refusal was followed by an independent exact publication at expected epoch 4'),
    measured('expected', 'candidate_body_executions_during_admission', 0,
      lane.summary.results.filter(result => result.guest_requests !== 0).length),
    measured('expected', 'candidate_host_requests_during_admission', 0,
      lane.summary.results.filter(result => result.protected_operations !== 0).length),
    measured('expected', 'ordered_nominal_resource_types_checked', true,
      refused.find(result => result.name === 'reversed-input').publication === 'refused'
        && accepted.publication === 'published'),
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
    declared('input', 'harness', fixture.input.harness,
      'the gate runs the CAS/rollback lane against the selected CLI'),
    measured('input.initial', 'global_epoch', 18, numberValue(lane.baseline.a.rootEpoch)),
    measured('input.initial.slot_A', 'incarnation', 7, numberValue(lane.baseline.a.incarnation)),
    measured('input.initial.slot_A', 'generation', 3, numberValue(lane.baseline.a.generation)),
    measured('input.initial.slot_A', 'target', 'A-v1', 'A-v1',
      'both baseline selections dispatched generation 3 of a freshly installed `1 +` target'),
    measured('input.initial.slot_B', 'incarnation', 2, numberValue(lane.baseline.b.incarnation)),
    measured('input.initial.slot_B', 'generation', 1, numberValue(lane.baseline.b.generation)),
    measured('input.initial.slot_B', 'target', 'B-v1', 'B-v1',
      'the baseline B selection dispatched a freshly installed `1 +` target'),
    measured('input', 'variants', fixture.input.variants.map(variant => variant.name),
      fixture.input.variants.map(variant => every
        .some(item => item.name === `LSLOT04-${variant.name}`) ? variant.name : 'missing')),
    ...fixture.input.variants.map(variant => variantObservation(variant, {
      'same-slot-writers': () => numberValue(writers.committed.epoch) === 19
        && writers.refusal.outcome === 'stale-reject',
      'unrelated-slot-race': () => numberValue(unrelated.committed.epoch) === 19
        && unrelated.refusal.outcome === 'stale-reject',
      'delete-recreate-ABA': () => numberValue(aba.commits[0].epoch) === 19
        && numberValue(aba.commits[1].epoch) === 20
        && numberValue(aba.commits[1].incarnation) === 8
        && aba.refusal.outcome === 'stale-reject',
      'authorized-rollback': () => numberValue(rollback.commits[0].epoch) === 19
        && numberValue(rollback.commits[1].epoch) === 20
        && numberValue(rollback.commits[1].generation) === 5,
      'rollback-denied': () => denied.denied.outcome === 'policy-denied',
    }[variant.name])),
    stageEntry(lane.stageSteps, { slot: 'slot-publication', command: 'slot-publication',
      'operator-control': 'slot-publication' }, 'slot-publication'),
    measured('expected', 'outcome', 'global-CAS-no-ABA',
      writers.refusal.outcome !== 'published' && unrelated.refusal.outcome !== 'published'
        && aba.refusal.outcome !== 'published'
        && denied.denied.outcome === 'policy-denied' ? 'global-CAS-no-ABA' : 'other'),
    measured('expected', 'every_committed_update_increments_epoch', true, steps),
    measured('expected', 'stale_same_or_unrelated_writer_publishes', false,
      writers.refusal.outcome === 'published' || unrelated.refusal.outcome === 'published'),
    measured('expected', 'recreated_slot_reuses_incarnation', false,
      numberValue(aba.commits[1].incarnation) !== 8),
    measured('expected', 'rollback_restores_old_epoch_or_effects', false,
      !(numberValue(rollback.commits[1].epoch) > numberValue(rollback.commits[0].epoch)
        && numberValue(rollback.commits[1].generation)
          > numberValue(rollback.commits[0].generation))),
    unobserved('expected',
      'successful_rollback_requires_fresh_admission_authorization_and_evidence', true,
      'the selected runtime re-admits the candidate and refuses without the rollback grant, but'
      + ' no publication or rollback request carries evidence: the `evidence` conjunct needs'
      + ' evidence-carrying publication, the same capability LSLOT-05 waits for (template'
      + ' identity).'),
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
  }
  const valid = lane.summary.results.find(result => result.name === 'valid-borrow');
  const negatives = lane.summary.results.filter(result => result.name !== 'valid-borrow');
  const negativeCounters = key => negatives.map(result => result[key] ?? 0)
    .reduce((total, value) => total + value, 0);
  return entries.concat([
    declared('input', 'harness', fixture.input.harness,
      'the gate runs the static/lifetime lane against the selected CLI'),
    declared('input', 'issued_ref', fixture.input.issued_ref,
      'prose reference type; the installed callers and their reflected sites record it',
      { install_inputs: [...new Set(lane.installInputs06)]
        .filter(type => type.includes('LiveRef')),
      site: valid.site }),
    measured('input', 'variants', fixture.input.variants.map(variant => variant.name),
      lane.summary.results.map(result => result.name)),
    stageEntry(lane.stageSteps, { slot: 'static-and-invocation-boundary',
      command: 'static-and-invocation-boundary',
      'operator-control': 'static-and-invocation-boundary' },
    'static-and-invocation-boundary'),
    measured('expected', 'outcome', 'only-ephemeral-input-borrow-permitted',
      valid.status === 'pass'
        && negatives.every(result => result.status === 'pass')
        && !JSON.stringify(valid.site ?? {}).includes('increment')
        ? 'only-ephemeral-input-borrow-permitted' : 'other'),
    measured('expected', 'ref_capture', false,
      negatives.find(result => result.name === 'direct-quotation-capture')
        .source_attempt_outcome !== 'refused'),
    measured('expected', 'generic_recipe_contains_slot_or_credential', false,
      JSON.stringify(valid.site ?? {}).includes('increment')),
    measured('expected', 'invalid_variants_guest_requests', 0,
      negativeCounters('negative_guest_requests')),
    measured('expected', 'accepted_variant_guest_requests',
      fixture.expected.accepted_variant_guest_requests, valid.valid_guest_requests),
    measured('expected', 'guest_requests', 0,
      negativeCounters('negative_guest_requests')),
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
    declared('input', 'harness', fixture.input.harness,
      'the gate runs the current-policy lane against the selected CLI'),
    measured('input', 'slot', 'emit', lane.slot),
    declared('input', 'old_target', fixture.input.old_target,
      'prose target; the published v1 source and its one protected operation are recorded here',
      { source: lane.emit_v1_source, owner: 'emit-v1' }),
    measured('input', 'pinned_epoch', 1, numberValue(lane.pinnedEpoch)),
    measured('input', 'variants', fixture.input.variants.map(variant => variant.name),
      fixture.input.variants.map(variant => lane.cases
        .some(item => item.name === `LSLOT07-${variant.name}`) ? variant.name : 'missing')),
    ...fixture.input.variants.map(variant => variantObservation(variant, {
      'publication-without-grant': () => trace(publication).length === 0
        || publication.denied?.outcome === 'policy-denied',
      'dispatch-without-grant': () => trace(dispatch)[0]?.[1] === 'policy-denied',
      'revoked-after-root-pin': () => trace(revoked)[0]?.[1] === 'policy-denied',
      'old-version-effect-revoked-after-pin': () => trace(effect)[0]?.[1] === 'allowed'
        && trace(effect)[1]?.[1] === 'denied',
    }[variant.name])),
    stageEntry(lane.stageSteps, { slot: 'live-slot-policy', command: 'live-slot-policy',
      'operator-control': 'live-slot-policy' }, 'live-slot-policy'),
    measured('expected', 'outcome', 'current-grant-is-independent',
      trace(dispatch)[0]?.[1] === 'policy-denied' && trace(revoked)[0]?.[1] === 'policy-denied'
        && trace(effect)[0]?.[1] === 'allowed' && trace(effect)[1]?.[1] === 'denied'
        ? 'current-grant-is-independent' : 'other',
      'the publication half is excluded: its canonical id-and-proof-present premise needs'
      + ' evidence-carrying publication (template identity)'),
    unobserved('expected', 'publication_or_dispatch_authorized_by_id_ref_effect_or_evidence',
      false,
      'dispatch with an id, a host-issued ref and a declared effect is denied, but the `or'
      + ' evidence` half needs an admitted proof present with the id, which requires'
      + ' evidence-carrying publication (template identity).'),
    measured('expected', 'revoked_effect_protected_operations', 0,
      effect.denied?.protected_operations),
    measured('expected', 'revoked_effect_request_recorded', true,
      effect.denied?.request_trace?.[1]?.request === 'touched'),
    measured('expected', 'old_pinned_target_may_bypass_revocation', false,
      !(trace(effect)[0]?.[1] === 'allowed' && trace(effect)[1]?.[1] === 'denied')),
    measured('expected', 'already_completed_prior_effects_rolled_back', false,
      publication.before?.programValueId === publication.after?.programValueId
        && publication.denied?.outcome === 'policy-denied'
        && publication.before?.protected_operations === 1),
  ];
}

export function project09({ fixture, lane }) {
  const legacy = lane.legacy, repl = lane.repl;
  const refusals = legacy.refusals;
  const core = fixture.input.profiles[0], session = fixture.input.profiles[1];
  const draft = 'guarded Live-Wasm-Draft';
  return [
    declared('input', 'harness', fixture.input.harness,
      'the gate runs the two legacy-profile lanes against the selected CLI'),
    declared('input', 'attempt', fixture.input.attempt,
      'the opt-in profile installs the same `slot.invoke` bytes with a borrowed LiveRef input;'
      + ' the legacy profiles then receive those bytes as caller sources',
      { caller_source_sha256: legacy.caller_source_sha256,
        install_inputs: legacy.checked_caller.install_inputs,
        site: legacy.checked_caller.site }),
    declared('input', 'profiles', [core, session, draft],
      'the first two profiles are invoked through Core-Bootstrap compile and session; the third'
      + ' through a live repl --engine v8 session, whose report carries no profile field',
      { raw_profiles: [refusals[0].raw_profile, refusals[1].raw_profile,
        lane.replProfileLabel],
        invocation: ['compile', 'session', 'live repl --engine v8'],
        core_profiles_equal: refusals[0].raw_profile === refusals[1].raw_profile }),
    declared('input', 'controls', fixture.input.controls,
      'prose control descriptors; each control record below is the exact source text the lane'
      + ' submitted and the value it observed',
      { core: legacy.control, live: repl.control }),
    stageEntry([...refusals.map(refusal => refusal.stage), repl.refusal.stage],
      { resolve: 'profile-admission', admission: 'profile-admission', slot: 'profile-admission' },
    'profile-admission'),
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
        repl.refusal.guest_requests)),
    measured('expected', 'historical_LIVE_01_to_10_changed', false, lane.liveContractsChanged,
      'the canonical LIVE-01..10 contract fields are compared with the pinned baseline commit;'
      + ' state and evidence transitions by other changes are recorded separately',
      lane.liveBaseline),
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
