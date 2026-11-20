// Projections from selected live-slot lane evidence onto the canonical
// LSLOT case records in `specs/conformance/live-reference-cases.json`.
//
// Every canonical leaf path of a case's `input` and `expected`, and every
// variant, gets exactly one entry. `gate.mjs` enforces that coverage and fails
// when a leaf has no entry. Statuses: `observed` (measured and equal),
// `mismatch` (measured and different, fails the gate), `unobserved` (the
// canonical attempt did not run, refuses the case), `declared` (prose only, on
// the per-case allowlist below, with the runtime facts recorded as evidence).
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

// Canonical prose keys that may be `declared`, with the runtime facts behind
// each. Every other key must be observed or unobserved.
export const declaredAllowlist = {
  'LSLOT-01': ['input.harness', 'expected.stage'],
  'LSLOT-02': ['input.harness', 'input.base.registry_epoch_1.A.body',
    'input.base.root_A_entry.pin_operation', 'input.base.root_A_entry.caller',
    'input.transitive_variant.registry_epoch_1.A.body',
    'input.transitive_variant.registry_epoch_1.B.body', 'expected.stage',
    'input.transitive_variant.root_A_entry.borrow_pass_through'],
  'LSLOT-03': ['input.harness', 'input.variant_protocol', 'expected.stage'],
  'LSLOT-04': ['input.harness', 'expected.stage'],
  'LSLOT-06': ['input.harness', 'input.issued_ref', 'expected.stage'],
  'LSLOT-07': ['input.harness', 'input.old_target', 'expected.stage'],
  'LSLOT-09': ['input.harness', 'input.attempt', 'input.profiles', 'input.controls',
    'expected.stage'],
};

function stageEntry(rawStages, mapping, canonical) {
  const evidence = { raw_stages: [...new Set(rawStages)].sort(), mapping };
  const unmapped = rawStages.filter(stage => !Object.hasOwn(mapping, stage));
  if (unmapped.length > 0) {
    return { scope: 'expected', key: 'stage', canonical, observed: evidence,
      status: 'mismatch', note: `unmapped runtime stage: ${unmapped.join(', ')}`, evidence };
  }
  if (new Set(rawStages.map(stage => mapping[stage])).size !== 1) {
    return { scope: 'expected', key: 'stage', canonical, observed: evidence,
      status: 'mismatch', note: 'runtime stages map to more than one phase', evidence };
  }
  return declared('expected', 'stage', canonical,
    'runtime phase names translated to the canonical case phase', evidence);
}
const stepStages = steps => steps.map(step => step.row.stage);

// `LiveRef<IN,OUT,EFF>` in either lane (`+`-joined, `pure`) or canonical
// (bracketed, comma-joined, `{}`) spelling, normalized to one form.
export function normalizeType(type) {
  const text = String(type).trim().replace(/^borrow /, '');
  if (!text.startsWith('LiveRef<')) return text;
  const body = text.slice('LiveRef<'.length, -1);
  const parts = splitTop(body, ',');
  assert.equal(parts.length, 3, `three LiveRef components: ${type}`);
  const strip = list => list.trim().replace(/^\[/, '').replace(/\]$/, '');
  const stripBraces = list => list.trim().replace(/^\{/, '').replace(/\}$/, '');
  const splitList = text_ => text_.trim().startsWith('[')
    ? splitTop(strip(text_), ',') : splitTop(text_, '+');
  const inputs = splitList(parts[0]).map(normalizeType);
  const outputs = splitList(parts[1]).map(normalizeType);
  const effects = splitTop(stripBraces(parts[2]), '+')
    .filter(effect => effect.length > 0 && effect !== 'pure').sort();
  const spell = list => list.length === 1 ? list[0] : `[${list.join(',')}]`;
  return `borrow LiveRef<${spell(inputs)},${spell(outputs)},{${effects.join(',')}}>`;
}
function splitTop(text, separator) {
  const parts = [];
  let depth = 0, current = '';
  for (const character of text) {
    if (character === '<' || character === '[') depth += 1;
    if (character === '>' || character === ']') depth -= 1;
    if (character === separator && depth === 0) { parts.push(current); current = ''; }
    else current += character;
  }
  parts.push(current);
  return parts;
}
// `LiveRef<IN,OUT,EFF>` split into normalized parts, for the typed ordered
// host-input comparison below.
function refParts(type) {
  const text = String(type).trim().replace(/^borrow /, '');
  assert.ok(text.startsWith('LiveRef<') && text.endsWith('>'), `LiveRef type: ${type}`);
  const parts = splitTop(text.slice('LiveRef<'.length, -1), ',');
  assert.equal(parts.length, 3, `three LiveRef components: ${type}`);
  const list = text_ => {
    const trimmed = text_.trim();
    const bracketed = trimmed.startsWith('[');
    const inner = bracketed ? trimmed.slice(1, -1) : trimmed;
    return splitTop(inner, bracketed ? ',' : '+').map(part => part.trim())
      .filter(Boolean).map(normalizeType);
  };
  const effects = splitTop(parts[2].trim().replace(/^\{/, '').replace(/\}$/, ''), '+')
    .map(part => part.trim()).filter(effect => effect && effect !== 'pure').sort();
  return { input: list(parts[0]), output: list(parts[1]), effects };
}
// The canonical ordered host-input prose is re-rendered from the raw invoke
// refs and the caller install request in the case's own convention, so a wrong
// slot, a wrong type or a wrong order produces a different list.
function orderedHostInputs(request, caller, canonical) {
  const value = request.inputs?.[0]?.value;
  const refs = request.refs ?? [];
  const types = caller.inputs.slice(1);
  const observed = [`I64:${value}`];
  if (refs.length !== canonical.length - 1) observed.push(`refs:${refs.length}`);
  const bySlot = new Map();
  refs.forEach((ref, index) => { bySlot.set(ref.slot, types[index]); });
  canonical.slice(1).forEach((claim, index) => {
    const slot = refs[index]?.slot;
    const type = types[index];
    assert.ok(slot && type, `host-issued ref ${index + 1} of the canonical input list`);
    const full = /^borrow (LiveRef<.*>) issued for (\w+)$/.exec(claim);
    if (full) {
      observed.push(`${normalizeType(type)} issued for ${slot}`);
      return;
    }
    const typed = /^borrow (\w+) ref typed for (.+) to (.+)$/.exec(claim);
    if (typed) {
      const parts = refParts(type);
      const dependencies = parts.input.map(part => {
        if (part === 'I64') return 'I64';
        const earlier = [...bySlot.entries()].find(([name, earlierType]) =>
          name !== slot && normalizeType(earlierType) === part);
        return earlier ? `${earlier[0]}-ref` : part;
      });
      observed.push(`borrow ${slot} ref typed for ${dependencies.join('+')}`
        + ` to ${parts.output.join('+')}`);
      return;
    }
    const plain = /^borrow (\w+) ref$/.exec(claim);
    assert.ok(plain, `canonical ordered host input: ${claim}`);
    observed.push(`borrow ${slot} ref`);
  });
  return observed;
}
// The full-type ordered entries compare under the same normalization as every
// other interface entry; the prose shorthand entries compare verbatim.
function canonicalOrdered(canonical) {
  return canonical.map((claim, index) => {
    if (index === 0) return claim;
    const full = /^borrow (LiveRef<.*>) issued for (\w+)$/.exec(claim);
    return full ? `${normalizeType(full[1])} issued for ${full[2]}` : claim;
  });
}
const spellTypes = list => list.map(normalizeType);
function interfaceEntries(prefix, canonical, observed, third = 'effects') {
  return [['input', spellTypes(canonical.input), observed.input],
    ['output', spellTypes(canonical.output), observed.output],
    [third, canonical[third], observed[third]]].map(([leaf, want, seen]) =>
    measured(prefix, leaf, want, seen));
}
// The canonical caller prose is parsed so the comparison uses its own words.
function proseCaller(prose) {
  const borrowed = /LiveRef<[^>]*>/.exec(prose);
  const output = /-> \[([^\]]*)\]/.exec(prose);
  const effects = /! \{([^}]*)\}/.exec(prose);
  assert.ok(borrowed && output && effects, `caller prose names ref, output, effects: ${prose}`);
  return { borrowed: normalizeType(borrowed[0]),
    effects: effects[1].split(',').map(effect => effect.trim()).filter(Boolean).sort(),
    output: output[1].split(',').map(type => normalizeType(type)),
    instruction: 'slot.invoke' };
}
// A variant is one entry: its runtime predicates must all hold.
function variantObservation(variant, predicate) {
  assert.ok(predicate, `variant ${variant.name} has a runtime predicate`);
  return measured('variant', variant.name, variant,
    predicate() ? variant : { failed: variant.name });
}

const paired = records => {
  const requests = records.filter(row => row.kind === 'operator').map(row => row.request);
  const replies = records.filter(row => row.kind === 'cli').map(row => row.row);
  assert.equal(replies.length, requests.length + 1);
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
  assert.equal(genericCalls.length, 2);
  const [beforeCall, afterCall] = genericCalls;
  const reflections = steps.filter(step => step.request?.operation === 'reflect');
  const site = reflections[0].row.site;
  const ordering = { saved: steps.indexOf(ordinaryRoot),
    publication: steps.indexOf(secondPublish), generic: steps.indexOf(afterCall) };
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
      'the gate runs this case\'s capture lane against the selected CLI'),
    measured('input', 'definition', fixture.input.definition, define.request.source),
    measured('input', 'saved_program', fixture.input.saved_program,
      savedInstall.request.source),
    measured('input', 'generic_caller', proseCaller(fixture.input.generic_caller),
      { borrowed: normalizeType(genericInstall.request.inputs
        .find(type => type.startsWith('LiveRef'))), effects: site.effects.sort(),
      output: spellTypes(site.output),
      instruction: site.instruction.includes('slot.invoke') ? 'slot.invoke' : 'other' }),
    measured('input.slot', 'id', 'increment', firstPublish.request.slot),
    measured('input.slot', 'incarnation', 1, numberValue(firstPublish.row.incarnation)),
    measured('input.slot', 'generation', 1, numberValue(firstPublish.row.generation)),
    measured('input.slot', 'target', fixture.input.slot.target,
      `n-v${numberValue(firstPublish.row.generation)}:[ ${definitionBody} ]`),
    measured('input.publication', 'expected_epoch',
      fixture.input.publication.expected_epoch,
      numberValue(secondPublish.request.expected_epoch)),
    measured('input.publication', 'expected_incarnation',
      fixture.input.publication.expected_incarnation,
      numberValue(secondPublish.request.expected_incarnation)),
    measured('input.publication', 'expected_generation',
      fixture.input.publication.expected_generation,
      numberValue(secondPublish.request.expected_generation)),
    measured('input.publication', 'candidate', fixture.input.publication.candidate,
      `n-v${numberValue(secondPublish.row.generation)}:[ ${replacementBody} ]`),
    measured('input.publication', 'host_authorized',
      fixture.input.publication.host_authorized,
      secondPublish.row.outcome === 'published'),
    measured('input', 'calls_after_publication', fixture.input.calls_after_publication, [
      { subject: 'saved_program', argument: numberValue(ordinaryRoot.request.inputs[0].value) },
      { subject: 'generic_caller', argument: numberValue(afterCall.request.inputs[0].value),
        host_issued_input_ref: afterCall.request.refs[0]?.slot },
    ], `step order: publication ${ordering.publication}, saved ${ordering.saved},`
      + ` generic ${ordering.generic}`),
    stageEntry(stepStages(steps), { slot: 'live-slot-root', command: 'live-slot-root',
      source: 'live-slot-root' }, 'live-slot-root'),
    measured('expected', 'outcome', 'immutable-capture-and-explicit-fresh-target',
      oldTarget && freshTarget && savedResult === 21 && genericResult === 22
        && ordering.generic > ordering.saved
        ? 'immutable-capture-and-explicit-fresh-target' : 'other'),
    measured('expected', 'saved_result', fixture.expected.saved_result, savedResult),
    measured('expected', 'generic_result', fixture.expected.generic_result, genericResult),
    measured('expected', 'saved_target', 'n-v1', oldTarget ? 'n-v1' : 'other'),
    measured('expected', 'generic_selected_target', 'n-v2', freshTarget ? 'n-v2' : 'other'),
    measured('expected', 'caller_identity_unchanged', fixture.expected.caller_identity_unchanged,
      same(site, reflections.at(-1).row.site)),
    measured('expected', 'ordinary_run_dynamic_lookup', false,
      ordinaryRoot.row.request_trace.length > 0 || ordinaryRoot.row.guest_requests !== 0),
    measured('expected', 'generic_recipe_embeds_slot_or_target', false,
      siteText.includes('increment') || siteText.includes('slotId')),
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
    return { input: spellTypes(found.input), output: spellTypes(found.output),
      effects: found.effectCeiling };
  };
  const version = (item, slot, id, withBody) => {
    const generation = generationsOf(item)[slot];
    return withBody ? `${slot}-v${generation}:[ ${installSource(item, id)} ]`
      : `${slot}-v${generation}`;
  };
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
  const callerOf = item => ({
    caller: item.install_requests.find(entry => entry.id === 'callerA'),
    site: item.reflected_sites.find(entry => entry.id === 'callerA').site,
  });
  const baseCanonicalOrdered = fixture.input.base.root_A_entry.ordered_host_issued_inputs;
  const transitiveCanonicalOrdered =
    fixture.input.transitive_variant.root_A_entry.ordered_host_issued_inputs;
  const orderedEntry = (item, canonical, scope) => {
    const observed = orderedHostInputs(refs(item), callerOf(item).caller, canonical);
    const expected = canonicalOrdered(canonical);
    return measured(scope, 'ordered_host_issued_inputs', canonical,
      same(observed, expected) ? canonical : { failed: observed, expected },
      'typed, ordered host-issued inputs rendered from the invoke refs and the caller'
      + ' install request in the case\'s own convention', { observed, expected });
  };
  const state = item => ({
    configured_epoch: item.configured_epoch, bootstrap_epoch: item.bootstrap.epoch,
    session: item.session_log, request: refs(item), caller: callerOf(item).caller,
    targets: ['A1', 'B1', 'C1'].filter(id => item.install_requests.some(entry =>
      entry.id === id)).map(id => installSource(item, id)),
  });
  const baseState = state(base), transitiveState = state(transitive);
  const noSlotIdentity = item => {
    const bound = refs(item).refs.map(ref => ref.slot);
    const sources = [callerOf(item).caller.source, ...state(item).targets];
    return { violated: !item.old_root.request_trace.every(entry => bound.includes(entry.slotId))
      || sources.some(source => bound.some(slot => source.split(/\s+/).includes(slot))) };
  };
  const baseInterleaveChecks = [
    baseB[0].generation === '1' && baseB[0].rootEpoch === base.old_root.epoch,
    numberValue(base.committed_ack.epoch) === 2
      && base.old_root.control_events?.some(event => event.operation === 'publish'
        && event.scope === 'B' && event.epoch === base.committed_ack.epoch),
    baseB[1].generation === '1' && baseB[1].programValueId === baseB[0].programValueId
      && baseB[1].rootEpoch === baseB[0].rootEpoch,
    base.new_root.epoch === base.committed_ack.epoch
      && base.new_root.request_trace.filter(entry => entry.slotId === 'B')
        .every(entry => entry.generation === '2')
      && refs(base).refs.some(ref => ref.slot === 'B'),
  ];
  const transitiveTrace = transitive.old_root.request_trace;
  const transitiveInterleaveChecks = [
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
  const separate = lane.crossProfile;
  return [
    declared('input', 'harness', fixture.input.harness,
      'the gate runs the frozen nested-root lane against the selected CLI'),
    measured('input.base.registry_epoch_1.A', 'version', 'A-v1', version(base, 'A', 'A1', false)),
    ...interfaceEntries('input.base.registry_epoch_1.A.interface',
      fixture.input.base.registry_epoch_1.A.interface, contract(base, 'A')),
    declared('input.base.registry_epoch_1.A', 'body',
      fixture.input.base.registry_epoch_1.A.body,
      'prose body; the installed caller source and its reflected site carry the behavior',
      { source: callerOf(base).caller.source, site: callerOf(base).site }),
    measured('input.base.registry_epoch_1.B', 'version', 'B-v1:[ 1 + ]',
      version(base, 'B', 'B1', true)),
    ...interfaceEntries('input.base.registry_epoch_1.B.interface',
      fixture.input.base.registry_epoch_1.B.interface, contract(base, 'B')),
    orderedEntry(base, baseCanonicalOrdered, 'input.base.root_A_entry'),
    measured('input.base.root_A_entry', 'pin_epoch', 1, numberValue(base.old_root.epoch)),
    declared('input.base.root_A_entry', 'pin_operation',
      fixture.input.base.root_A_entry.pin_operation,
      'the root keeps one epoch for both dispatches; the next root observes the next epoch',
      { old_epoch: numberValue(base.old_root.epoch),
        new_epoch: numberValue(base.new_root.epoch) }),
    declared('input.base.root_A_entry', 'caller', fixture.input.base.root_A_entry.caller,
      'prose caller description; the reflected generic site and result are recorded here',
      { site: callerOf(base).site, result: numberValue(base.old_root.stack[0].value) }),
    measured('input.base', 'interleaving', fixture.input.base.interleaving,
      baseInterleaveChecks.every(Boolean) ? fixture.input.base.interleaving
        : { failed_step: baseInterleaveChecks.indexOf(false) + 1 },
      baseInterleaveChecks.map((value, index) => `${index + 1}:${value}`).join(' ')),
    ...interfaceEntries('input.base.new_root_B_interface',
      fixture.input.base.new_root_B_interface,
      { input: spellTypes(base.next_leaf.request_trace[0].policyInput.version.input),
        output: spellTypes(base.next_leaf.request_trace[0].policyInput.version.output),
        effects: base.next_leaf.request_trace[0].policyInput.version.effects }),
    measured('input.transitive_variant', 'independent_fixture',
      fixture.input.transitive_variant.independent_fixture,
      baseState.configured_epoch === '0' && baseState.bootstrap_epoch === '1'
        && transitiveState.configured_epoch === '0' && transitiveState.bootstrap_epoch === '1'
        && baseState.session !== transitiveState.session
        ? fixture.input.transitive_variant.independent_fixture : { failed: 'fixture reuse' },
      null, { base_session: baseState.session, transitive_session: transitiveState.session,
        base_epochs: [baseState.configured_epoch, baseState.bootstrap_epoch],
        transitive_epochs: [transitiveState.configured_epoch, transitiveState.bootstrap_epoch] }),
    measured('input.transitive_variant.registry_epoch_1.A', 'version', 'A-v1',
      version(transitive, 'A', 'A1', false)),
    ...interfaceEntries('input.transitive_variant.registry_epoch_1.A.interface',
      fixture.input.transitive_variant.registry_epoch_1.A.interface, contract(transitive, 'A')),
    declared('input.transitive_variant.registry_epoch_1.A', 'body',
      fixture.input.transitive_variant.registry_epoch_1.A.body,
      'prose body; the installed caller source and its reflected site carry the behavior',
      { source: callerOf(transitive).caller.source, site: callerOf(transitive).site }),
    measured('input.transitive_variant.registry_epoch_1.B', 'version', 'B-v1',
      version(transitive, 'B', 'B1', false)),
    ...interfaceEntries('input.transitive_variant.registry_epoch_1.B.interface',
      fixture.input.transitive_variant.registry_epoch_1.B.interface, contract(transitive, 'B')),
    declared('input.transitive_variant.registry_epoch_1.B', 'body',
      fixture.input.transitive_variant.registry_epoch_1.B.body,
      'prose body; the B caller source is recorded here',
      { source: installSource(transitive, 'B1') }),
    measured('input.transitive_variant.registry_epoch_1.C', 'version', 'C-v1:[ 1 + ]',
      version(transitive, 'C', 'C1', true)),
    ...interfaceEntries('input.transitive_variant.registry_epoch_1.C.interface',
      fixture.input.transitive_variant.registry_epoch_1.C.interface, contract(transitive, 'C')),
    orderedEntry(transitive, transitiveCanonicalOrdered,
      'input.transitive_variant.root_A_entry'),
    measured('input.transitive_variant.root_A_entry', 'pin_epoch', 1,
      numberValue(transitive.old_root.epoch)),
    declared('input.transitive_variant.root_A_entry', 'borrow_pass_through',
      fixture.input.transitive_variant.root_A_entry.borrow_pass_through,
      'prose steps; the old-root A/B/C dispatch order is recorded here',
      { dispatch_order: transitiveTrace.map(entry => entry.slotId) }),
    measured('input.transitive_variant', 'interleaving',
      fixture.input.transitive_variant.interleaving,
      transitiveInterleaveChecks.every(Boolean)
        ? fixture.input.transitive_variant.interleaving
        : { failed_step: transitiveInterleaveChecks.indexOf(false) + 1 },
      transitiveInterleaveChecks.map((value, index) => `${index + 1}:${value}`).join(' ')),
    stageEntry(lane.stages, { slot: 'live-slot-root', command: 'live-slot-root',
      source: 'live-slot-root', 'operator-control': 'live-slot-root' }, 'live-slot-root'),
    measured('expected', 'outcome', 'one-map-per-root-with-explicit-typed-ref-pass-through',
      numberValue(base.old_root.epoch) === 1 && numberValue(base.new_root.epoch) === 2
        && numberValue(transitive.old_root.epoch) === 1
        && numberValue(transitive.new_root.epoch) === 2
        ? 'one-map-per-root-with-explicit-typed-ref-pass-through' : 'other'),
    measured('expected', 'base_root_A_B_selections', ['B-v1', 'B-v1'],
      baseB.map(entry => labelOf(base, entry))),
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
    measured('expected', 'mixed_registry_epochs_within_root', false,
      [base, transitive].some(item => [item.old_root, item.new_root]
        .some(root => root.request_trace.some(entry => entry.rootEpoch !== root.epoch)))),
    measured('expected', 'implicit_source_name_or_captured_ref_lookup', false,
      noSlotIdentity(base).violated || noSlotIdentity(transitive).violated,
      'every dispatched slot is a host-bound ref slot, and no caller or target source carries a'
      + ' slot identity'),
    measured('expected', 'current_authorization_rechecked_at_each_dispatch', true,
      dispatchPolicy(base) && dispatchPolicy(transitive)),
    measured('expected',
      'registry_epoch_distinct_from_live_wasm_namespace_or_source_generation', true,
      [base, transitive].every(item => numberValue(item.old_root.epoch) === 1
        && numberValue(item.new_root.epoch) === 2)
        && separate.liveReportsHaveEpoch === false && separate.refusedSlotGeneration === 0,
      'slot epochs 1->2 in this case; the guarded Live-Wasm-Draft lane of the same run reports'
      + ' its own source generations and no registry epoch, and refuses the caller at generation 0',
      { slot_epochs: [1, 2], live_generations: separate.liveGenerations,
        live_reports_have_epoch: separate.liveReportsHaveEpoch,
        refused_generation: separate.refusedSlotGeneration }),
  ];
}

export function project03({ fixture, lane }) {
  const item = name => {
    const found = lane.variants.find(variant => variant.name === name);
    assert.ok(found, `missing LSLOT-03 ${name}`);
    return found;
  };
  const entries = [];
  for (const variant of fixture.input.variants) {
    const found = item(variant.name);
    const observed = found.install_outcome === 'installed'
      && found.publication_outcome === 'published' ? 'publish' : 'reject';
    entries.push(measured('variant', variant.name, variant.expected, observed));
  }
  const accepted = item('same-exact-interface');
  const refused = fixture.input.variants.filter(variant =>
    variant.name !== 'same-exact-interface').map(variant => item(variant.name));
  const slot = lane.authority.slots.find(entry => entry.slotId === 'account');
  // The accepted install reply carries no epoch; the registry epoch the
  // candidate ran at is the last seed publication reply, and every variant's
  // own file records it.
  const current = accepted.seed_epoch;
  assert.ok(Number.isInteger(current), 'the accepted variant seed chain reports its epoch');
  return entries.concat([
    declared('input', 'harness', fixture.input.harness,
      'the gate runs the exact-admission lane against the selected CLI'),
    ...interfaceEntries('input.slot_contract',
      { input: fixture.input.slot_contract.input,
        output: fixture.input.slot_contract.output,
        effect_ceiling: fixture.input.slot_contract.effect_ceiling },
      { input: slot.input, output: slot.output, effect_ceiling: slot.effectCeiling },
      'effect_ceiling'),
    measured('input', 'current_epoch', fixture.input.current_epoch, current,
      'the epoch of the last seed publication reply the accepted candidate was admitted at'),
    declared('input', 'variant_protocol', fixture.input.variant_protocol,
      'each variant re-runs from an exact epoch-4 registry with only its named field changed'),
    measured('input', 'variants', fixture.input.variants.map(variant => variant.name),
      lane.variants.map(variant => variant.name)),
    stageEntry(lane.stageSteps, { slot: 'slot-candidate-admission',
      command: 'slot-candidate-admission', 'operator-control': 'slot-candidate-admission' },
    'slot-candidate-admission'),
    measured('expected', 'outcome', 'only-exact-compatible-candidate-publishes',
      accepted.publication_outcome === 'published'
        && refused.every(found => found.publication_outcome !== 'published')
        ? 'only-exact-compatible-candidate-publishes' : 'other'),
    measured('expected', 'success_epoch', fixture.expected.success_epoch,
      numberValue(accepted.attempt_epoch)),
    measured('expected', 'rejected_variants_leave_epoch_and_map_unchanged', true,
      refused.every(found => found.seed_epoch === current
        && found.attempt_outcome !== 'published'
        && (found.publish_expected_epoch == null
          || numberValue(found.publish_expected_epoch) === current)
        && found.control_outcome === 'published'
        && numberValue(found.control_expected_epoch) === current
        && numberValue(found.control_epoch) === current + 1
        && found.traces.every(trace => Array.isArray(trace) && trace.length === 0))
        && numberValue(accepted.attempt_epoch) === current + 1,
      'every refused variant\'s seed chain ends at the unchanged epoch, its post-refusal trace'
      + ' carries no entry, and the independent exact control then publishes at the next'
      + ' epoch; only the accepted candidate advances the epoch'),
    measured('expected', 'candidate_body_executions_during_admission', 0,
      lane.variants.reduce((total, found) => total + (found.guest_requests ?? 0), 0)),
    measured('expected', 'candidate_host_requests_during_admission', 0,
      lane.variants.reduce((total, found) => total + (found.protected_operations ?? 0), 0)),
    measured('expected', 'generic_dispatch_effect', fixture.expected.generic_dispatch_effect,
      item('generic-caller-underdeclares-dispatch').site?.effects ?? null),
    measured('expected', 'ordered_nominal_resource_types_checked', true,
      item('reversed-input').publication_outcome === 'refused'
        && item('same-layout-different-resource-schema').publication_outcome === 'refused'
        && accepted.publication_outcome === 'published',
      'ordering and the both-sides Account@2 nominal schema are refused while the exact'
      + ' candidate publishes'),
    measured('expected', 'guest_requests', 0,
      lane.variants.reduce((total, found) => total + (found.guest_requests ?? 0), 0)),
    measured('expected', 'protected_operations', 0,
      lane.variants.reduce((total, found) => total + (found.protected_operations ?? 0), 0)),
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
    measured('input.initial.slot_A', 'target', 'A-v1', lane.baseline.a_published,
      'the baseline A selection dispatched the installed A1 artifact'),
    measured('input.initial.slot_B', 'incarnation', 2, numberValue(lane.baseline.b.incarnation)),
    measured('input.initial.slot_B', 'generation', 1, numberValue(lane.baseline.b.generation)),
    measured('input.initial.slot_B', 'target', 'B-v1', lane.baseline.b_published,
      'the baseline B selection dispatched the installed B1 artifact'),
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
      're-admission and the rollback grant are observed, but the `evidence` conjunct needs'
      + ' evidence-carrying publication, the capability LSLOT-05 waits for under template'
      + ' identity.'),
    measured('expected', 'failed_candidate_executions', 0,
      refusals.filter(row => row.guest_requests !== 0).length),
    measured('expected', 'guest_requests', 0, counters('guest_requests')),
    measured('expected', 'protected_operations', 0, counters('protected_operations')),
  ];
}

export function project06({ fixture, lane }) {
  const item = name => {
    const found = lane.variants.find(variant => variant.name === name);
    assert.ok(found, `missing LSLOT-06 ${name}`);
    return found;
  };
  const entries = [];
  // The negative attempt is either the refused `attempt` install or, where the
  // case has no installable negative source (retain), the refused re-invoke.
  const refusedSomehow = found => ['refused', 'install-refused']
    .includes(found.negative?.outcome);
  for (const variant of fixture.input.variants) {
    const found = item(variant.name);
    const accepted = variant.result === 'accept';
    const observed = accepted ? 'accept' : 'reject';
    entries.push(measured('variant', variant.name, variant.result,
      accepted ? (found.guest_requests === 1 ? observed : { failed: found.guest_requests })
        : (refusedSomehow(found) ? observed
          : { failed: found.negative ?? { attempt: 'not-refused' } })));
  }
  const valid = item('valid-borrow');
  const negatives = lane.variants.filter(found => found.name !== 'valid-borrow');
  const sum = key => negatives.reduce((total, found) => total + (found[key] ?? 0), 0);
  return entries.concat([
    declared('input', 'harness', fixture.input.harness,
      'the gate runs the static/lifetime lane against the selected CLI'),
    declared('input', 'issued_ref', fixture.input.issued_ref,
      'prose reference type; the installed callers and reflected sites record it',
      { install_inputs: [...new Set(lane.installInputs06)]
        .filter(type => type.includes('LiveRef')), site: valid.site }),
    measured('input', 'variants', fixture.input.variants.map(variant => variant.name),
      lane.variants.map(variant => variant.name)),
    stageEntry(lane.stageSteps, { slot: 'static-and-invocation-boundary',
      command: 'static-and-invocation-boundary',
      'operator-control': 'static-and-invocation-boundary' },
    'static-and-invocation-boundary'),
    measured('expected', 'outcome', 'only-ephemeral-input-borrow-permitted',
      valid.guest_requests === 1 && negatives.every(refusedSomehow)
        && !JSON.stringify(valid.site ?? {}).includes('increment')
        ? 'only-ephemeral-input-borrow-permitted' : 'other'),
    measured('expected', 'ref_data', false,
      ['aggregate-capture', 'aggregate-storage']
        .some(name => !refusedSomehow(item(name))),
      'the aggregate attempts over a host-issued ref are refused before any storage exists'),
    measured('expected', 'ref_capture', false,
      !refusedSomehow(item('direct-quotation-capture'))),
    measured('expected', 'ref_authority', false, lane.authorityRefused !== true,
      'a host-issued ref with no dispatch grant is refused by current host policy, observed'
      + ' from the raw policy-denied dispatch reply',
      { refusal: lane.authorityRefusal }),
    measured('expected', 'generic_recipe_contains_slot_or_credential', false,
      JSON.stringify(valid.site ?? {}).includes('increment')),
    measured('expected', 'invalid_variants_guest_requests', 0, sum('guest_requests')),
    measured('expected', 'accepted_variant_guest_requests',
      fixture.expected.accepted_variant_guest_requests, valid.guest_requests),
    measured('expected', 'guest_requests', 0, sum('guest_requests')),
    measured('expected', 'protected_operations', 0, sum('protected_operations')),
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
      'prose target; the published v1 source is recorded here',
      { source: lane.emit_v1_source }),
    measured('input', 'pinned_epoch', 1, numberValue(lane.pinnedEpoch)),
    measured('input', 'variants', fixture.input.variants.map(variant => variant.name),
      fixture.input.variants.map(variant => lane.cases
        .some(item => item.name === `LSLOT07-${variant.name}`) ? variant.name : 'missing')),
    ...fixture.input.variants.map(variant => variantObservation(variant, {
      'publication-without-grant': () => publication.denied?.outcome === 'policy-denied',
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
        ? 'current-grant-is-independent' : 'other'),
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
      'the opt-in profile installs the same bytes with a borrowed LiveRef input; the legacy'
      + ' profiles then receive those bytes as caller sources',
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
      'prose control descriptors; the control records below are the exact sources the lanes'
      + ' submitted and the values they observed',
      { core: legacy.control, live: repl.control }),
    stageEntry([...refusals.map(refusal => refusal.stage), repl.refusal.stage],
      { resolve: 'profile-admission', admission: 'profile-admission', slot: 'profile-admission' },
    'profile-admission'),
    measured('expected', 'outcome', 'explicit-unsupported-in-all-legacy-profiles',
      refusals.every(refusal => refusal.outcome === 'unsupported')
        && repl.refusal.outcome === 'reload-refused' && repl.refusal.generation === 0
        && legacy.control.stack[0].value === '21' && repl.control.stack[0].value === '21'
        ? 'explicit-unsupported-in-all-legacy-profiles' : 'other'),
    measured('expected', 'unsupported_produces_generic_run_or_empty_effect', false,
      !refusals.every(refusal => refusal.outcome === 'unsupported')
        || repl.refusal.outcome !== 'reload-refused'),
    measured('expected', 'candidate_body_executions', 0,
      refusals.reduce((total, refusal) => total + refusal.guest_requests,
        repl.refusal.guest_requests)),
    measured('expected', 'historical_LIVE_01_to_10_changed', false, lane.liveContractsChanged,
      'the LIVE-01..10 contract fields are compared with the pinned baseline commit, allowing'
      + ' exactly the owner-approved decision-21 LIVE-02 change',
      lane.liveBaseline),
    measured('expected', 'old_capture_semantics_changed', false,
      legacy.control.stack[0].value !== '21' || repl.control.stack[0].value !== '21'),
    measured('expected', 'profile_supported_by_current_runtime', false,
      refusals.some(refusal => refusal.outcome !== 'unsupported')
        || repl.refusal.exit === 0 || repl.refusal.generation !== 0),
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
