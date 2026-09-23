import assert from 'node:assert/strict';
import { digest, stable } from '../../tools/m3-wasm-policy.mjs';

const GENERATOR_REVISION = 'm4-composed-i64-splitmix64/v1';
const SHRINKER_REVISION = 'm4-domain-and-predicate-preserving/v1';
const ORACLE_REVISION = 'm4-wrap64-lit-def-test-emit/v1';
const SEED = '0x0B1E5EED00000004';
const LIMIT_KEYS = ['allocation_bytes', 'recipe_leaves', 'program_depth', 'operand_bytes', 'continuation_bytes', 'steps'];
const OPERATIONS = ['+', '-', '*'];
// Definition order comes from the kernel bootstrap table, not executed output.
// The source-backend admission table overrides slot 22 to Text -- (no Unit).
const DEFINITIONS = { '+': 'builtin:i64.add', '-': 'builtin:i64.sub', '*': 'builtin:i64.mul' };
const OBSERVE = 'pair dup unpair compose swap [ dup [ run ] dip dup reflect pair ] dip';
const DOMAIN = {
  input_types: ['I64'], input_range: ['-9223372036854775808', '9223372036854775807'],
  component_input: '[I64]', component_output: '[I64]',
  output_types: ['I64', 'Pair(Program,Syntax)', 'Pair(Program,Program)'],
  allowed_effects: ['test.emit'],
};
const CONTROL_IDS = [
  'same-seed-revisions-bounds-and-environment', 'generator-emits-invalid-candidate',
  'seeded-result-disagreement', 'shrinker-breaks-input-interface', 'shrinker-changes-failure-predicate',
  'shrink-budget-exhausted', 'trial-budget-exhausted-before-result', 'malformed-candidate-fuzzing',
];
const copy = value => JSON.parse(JSON.stringify(value));
const same = (left, right) => stable(left) === stable(right);
const scalar = value => ({ type: 'I64', value: String(value) });
const wrap = value => BigInt.asIntN(64, value);
const absolute = value => value < 0n ? -value : value;

// MC2 reports retain live handles in addition to ordinary semantic values.
// Keep the raw reports as evidence, validate bookkeeping, then compare only
// semantics where a workload does not declare cross-operation cell identity.
export function semanticObservation(value) {
  if (Array.isArray(value)) return value.map(semanticObservation);
  if (!value || typeof value !== 'object') return value;
  if (Object.hasOwn(value, 'handle')) {
    if (value.handle === null) assert.ok(['I64', 'Bool', 'Unit'].includes(value.type));
    else assert.ok(Number.isSafeInteger(value.handle) && value.handle > 0, 'invalid live cell handle');
  }
  return Object.fromEntries(Object.entries(value).filter(([key]) => key !== 'handle')
    .map(([key, item]) => [key, semanticObservation(item)]));
}

export function stackObservation(stack) {
  assert.ok(Array.isArray(stack));
  for (const value of stack) {
    assert.ok(Object.hasOwn(value, 'handle'), 'live stack entry omitted handle metadata');
    if (['I64', 'Bool', 'Unit'].includes(value.type)) assert.equal(value.handle, null);
    else assert.ok(Number.isSafeInteger(value.handle) && value.handle > 0);
  }
  return semanticObservation(stack);
}

function stream(seed) {
  let state = BigInt(seed);
  return () => {
    state = BigInt.asUintN(64, state + 0x9e3779b97f4a7c15n);
    let value = state;
    value = BigInt.asUintN(64, (value ^ (value >> 30n)) * 0xbf58476d1ce4e5b9n);
    value = BigInt.asUintN(64, (value ^ (value >> 27n)) * 0x94d049bb133111ebn);
    return value ^ (value >> 31n);
  };
}

function arithmetic(input, steps) {
  let value = BigInt(input);
  for (const step of steps) {
    const operand = BigInt(step.operand);
    if (step.operation === '+') value = wrap(value + operand);
    else if (step.operation === '-') value = wrap(value - operand);
    else if (step.operation === '*') value = wrap(value * operand);
    else throw Error(`unknown reference operation: ${step.operation}`);
  }
  return scalar(value);
}

function program(steps) {
  const recipe = [], witnesses = [], effects = [];
  for (const step of steps) {
    recipe.push({ literal: scalar(step.operand) }, { invoke: DEFINITIONS[step.operation] });
    witnesses.push({ node: recipe.length - 1, stack_in: '[I64 I64]', stack_out: '[I64]', effects: [] });
    if (step.emit !== null) {
      recipe.push({ literal: { type: 'Text', value: step.emit, schema: '[Text]' } }, { invoke: 'host:test.emit' });
      witnesses.push({ node: recipe.length - 1, stack_in: '[I64 Text]', stack_out: '[I64]', effects: ['test.emit'] });
      if (!effects.length) effects.push('test.emit');
    }
  }
  return { type: 'Program', interface: { stack_in: '[I64]', stack_out: '[I64]', effects }, recipe, witnesses };
}

function quotation(steps) {
  return `[ ${steps.map(step => `${step.operand} ${step.operation}${step.emit === null ? '' : ` "${step.emit}" test.emit`}`).join(' ')} ]`;
}

function trial(id, model, fault = null) {
  const steps = [...model.left, ...model.right];
  const composed = program(steps);
  const reference = {
    result: arithmetic(model.input, steps), components: [program(model.left), program(model.right)], composed,
    syntax: { type: 'Syntax', recipe: copy(composed.recipe), witnesses: copy(composed.witnesses) },
    request_trace: steps.filter(step => step.emit !== null).map(step => `test.emit:${step.emit}`),
  };
  const expected = copy(reference);
  if (fault === 'result-plus-one') expected.result = scalar(wrap(BigInt(reference.result.value) + 1n));
  else if (fault === 'changed-recipe-expectation') {
    const at = expected.composed.recipe.findIndex(atom => atom.invoke);
    expected.composed.recipe[at].invoke = expected.composed.recipe[at].invoke === 'builtin:i64.add'
      ? 'builtin:i64.sub' : 'builtin:i64.add';
    expected.syntax.recipe = copy(expected.composed.recipe);
  } else assert.equal(fault, null, 'unknown hostile oracle control');
  return {
    id, model: copy(model), input_types: ['I64'], inputs: [scalar(model.input)],
    source: `${quotation(model.left)} ${quotation(model.right)} ${OBSERVE}`,
    optimization: model.optimization, domain: copy(DOMAIN), reference, expected,
    hostile_harness_control: fault === null ? null : {
      injection: fault, target: 'expected observations only; compiler and Wasm execution remain real',
      compiler_or_backend_fault_claimed: false,
    },
  };
}

function generate(seed, count) {
  const next = stream(seed);
  const edges = ['0', '1', '-1', '9223372036854775807', '-9223372036854775808'];
  return Array.from({ length: count }, (_, index) => {
    const input = index < edges.length ? edges[index] : String(BigInt.asIntN(64, next()));
    const components = [0, 1].map(component => {
      const length = 1 + Number(next() % 3n);
      return Array.from({ length }, (_, position) => ({
        operation: OPERATIONS[Number(next() % 3n)],
        operand: index % 10 === 0 && position === 0 ? edges[Number(next() % BigInt(edges.length))]
          : String(BigInt(next() % 65n) - 32n),
        emit: index % 4 === 0 && position === length - 1 ? `m4-${index}-${component}` : null,
      }));
    });
    return trial(`generated-${index}`, {
      input, left: components[0], right: components[1], optimization: index % 2 ? 'on' : 'off',
    });
  });
}

function validI64(value) {
  if (value?.type !== 'I64' || typeof value.value !== 'string' || !/^-?(0|[1-9][0-9]*)$/.test(value.value)) return false;
  const number = BigInt(value.value);
  return wrap(number) === number;
}

function inputDomain(candidate) {
  return same(candidate.domain, DOMAIN) && same(candidate.input_types, DOMAIN.input_types)
    && candidate.inputs.length === 1 && validI64(candidate.inputs[0]);
}

function outputDomain(observed) {
  const stack = observed.stack;
  if (!Array.isArray(stack) || stack.length !== 3 || !validI64(stack[0])
    || stack[1]?.type !== 'Pair' || stack[1].value?.length !== 2
    || stack[2]?.type !== 'Pair' || stack[2].value?.length !== 2
    || stack[1].value[1]?.type !== 'Syntax') return false;
  return [stack[1].value[0], ...stack[2].value].every(value => value?.type === 'Program'
    && value.interface?.stack_in === '[I64]' && value.interface?.stack_out === '[I64]'
    && Array.isArray(value.interface.effects)
    && value.interface.effects.every(effect => DOMAIN.allowed_effects.includes(effect)));
}

function compare(candidate, observed, before, after) {
  const failures = [];
  const check = (predicate, actual, expected) => {
    if (!same(actual, expected)) failures.push({ predicate, actual: actual ?? null, expected });
  };
  check('runtime/no-compilation-during-execution', after, before);
  check('runtime/no-candidate-preparation', observed.candidate_prepare_requests, 0);
  check('runtime/no-protected-operations', observed.protected_operations, 0);
  check('reference-result-and-effect-agreement/normal-return', observed.outcome, 'normal');
  check('reference-result-and-effect-agreement/native-trap', observed.native_trap, null);
  if (observed.outcome !== 'normal') return failures;
  const [result, retained, operands] = observed.stack ?? [];
  const [composed, syntax] = retained?.type === 'Pair' ? retained.value : [];
  const components = operands?.type === 'Pair' ? operands.value : [];
  check('compatible-composition-interface/output-domain', outputDomain(observed), true);
  check('compatible-composition-interface/composed', composed?.interface, candidate.expected.composed.interface);
  for (let index = 0; index < 2; index += 1) {
    check('compatible-composition-interface/component', components[index]?.interface, candidate.expected.components[index].interface);
  }
  check('compatible-composition-interface/join', components[0]?.interface?.stack_out, components[1]?.interface?.stack_in ?? '[I64]');
  if (components.length === 2 && components.every(value => Array.isArray(value?.interface?.effects))) {
    const union = [...new Set(components.flatMap(value => value.interface.effects))].sort();
    check('compatible-composition-interface/effect-union', composed?.interface?.effects, union);
  }
  for (const [actual, expected] of [
    [composed, candidate.expected.composed], [syntax, candidate.expected.syntax],
    [components[0], candidate.expected.components[0]], [components[1], candidate.expected.components[1]],
  ]) {
    check('retained-recipe-structure/atoms', semanticObservation(actual?.recipe), expected.recipe);
    check('retained-recipe-structure/invocation-witnesses', actual?.witnesses, expected.witnesses);
  }
  check('reference-result-and-effect-agreement/immediate-handle', result?.handle, null);
  check('reference-result-and-effect-agreement/result', semanticObservation(result), candidate.expected.result);
  check('reference-result-and-effect-agreement/trace', observed.request_trace, candidate.expected.request_trace);
  check('reference-result-and-effect-agreement/request-count', observed.guest_requests, candidate.expected.request_trace.length);
  return failures;
}

function errorRecord(error) {
  const record = { name: error?.name ?? 'Error', message: String(error?.message ?? error) };
  for (const key of ['code', 'stdout', 'stderr', 'command', 'actual', 'expected']) {
    if (error?.[key] !== undefined) record[key] = error[key];
  }
  return record;
}

function diagnosticSource(candidate) {
  const inputs = candidate.inputs.map(input => {
    if (input.type === 'I64') return input.value;
    if (input.type === 'Bool') return String(input.value);
    throw Error('diagnostic replay does not support this input type');
  });
  return `${inputs.join(' ')} ${candidate.source}`;
}

function execute(context, label, candidate, limitOverrides = {}) {
  const record = {
    candidate, classification: 'not-run', acceptance: { outcome: 'not-run' },
    input_domain_preserved: inputDomain(candidate), output_domain_preserved: false,
    runtime_limits: { ...context.configuration.limits, ...limitOverrides },
    failures: [], failure_predicates: [],
  };
  if (!context.compilerAvailable) {
    record.reason = 'selected real compiler unavailable; no substitute compiler or interpreter';
    return record;
  }
  let compiled = null;
  try {
    compiled = context.compile(label, candidate.source, candidate.input_types, candidate.optimization);
    record.acceptance = {
      outcome: 'accepted', route: 'noble compile -> noble-wasm source admission -> noble_kernel::acceptance::check',
      metadata: compiled.metadata,
    };
    const before = compiled.engine.compilations;
    record.observed = compiled.engine.execute({ inputs: candidate.inputs, limits: record.runtime_limits });
    const after = compiled.engine.compilations;
    record.compilations = { before_execute: before, after_execute: after };
    record.output_domain_preserved = outputDomain(record.observed);
    // Exhaustion is an incomplete observation, not an observed refutation.
    if (!['runtime-exhausted', 'unsupported'].includes(record.observed.outcome)) {
      record.failures = compare(candidate, record.observed, before, after);
    }
    record.failure_predicates = [...new Set(record.failures.map(failure => failure.predicate))].sort();
    if (!record.input_domain_preserved) record.classification = 'generator-failure';
    else if (record.observed.stage !== 'wasm') record.classification = 'harness-error';
    else if (record.observed.outcome === 'runtime-exhausted') record.classification = 'exhausted';
    else if (record.observed.outcome === 'unsupported') record.classification = 'unsupported';
    else record.classification = record.failures.length ? 'counterexample' : 'passed';
  } catch (error) {
    record.error = errorRecord(error);
    record.classification = 'harness-error';
    if (compiled === null) {
      // A thrown adapter assertion is not itself evidence of a type rejection.
      // Obtain a structured diagnostic from the same real compiler on a closed
      // source equivalent to the explicitly typed inputs.
      try {
        record.diagnostic_source = diagnosticSource(candidate);
        record.diagnostic_replay = context.runSource(`${label}-diagnostic`, record.diagnostic_source, candidate.optimization);
        const observed = record.diagnostic_replay;
        record.acceptance = { outcome: 'not-accepted', stage: observed.stage, diagnostic_outcome: observed.outcome };
        if (['parse', 'resolve', 'check'].includes(observed.stage)
          && ['reject', 'unbound-word', 'type-reject', 'eligibility-reject'].includes(observed.outcome)
          && observed.process_exit !== 0 && observed.guest_requests === 0 && observed.protected_operations === 0) {
          record.classification = 'generator-failure';
        } else if (observed.outcome === 'unsupported') record.classification = 'unsupported';
        else if (['exhausted', 'static-exhausted', 'runtime-exhausted'].includes(observed.outcome)) record.classification = 'exhausted';
      } catch (diagnosticError) {
        record.diagnostic_error = errorRecord(diagnosticError);
      }
    }
  } finally {
    if (compiled !== null) {
      try {
        compiled.engine.close();
        record.engine_closed = true;
      } catch (error) {
        record.close_error = errorRecord(error);
        record.engine_closed = false;
        record.classification = 'harness-error';
      }
    }
  }
  return record;
}

function size(candidate) {
  const steps = [...candidate.model.left, ...candidate.model.right];
  return {
    arithmetic_steps: steps.length,
    numeric_magnitude: String(steps.reduce((total, step) => total + absolute(BigInt(step.operand)), absolute(BigInt(candidate.model.input)))),
    source_bytes: candidate.source.length,
  };
}

function smaller(left, right) {
  const a = size(left), b = size(right);
  return a.arithmetic_steps < b.arithmetic_steps
    || a.arithmetic_steps === b.arithmetic_steps && (BigInt(a.numeric_magnitude) < BigInt(b.numeric_magnitude)
      || a.numeric_magnitude === b.numeric_magnitude && a.source_bytes < b.source_bytes);
}

function reductions(candidate) {
  const models = [];
  for (const side of ['left', 'right']) {
    if (candidate.model[side].length > 1) {
      const model = copy(candidate.model);
      model[side].pop();
      models.push(model);
    }
  }
  if (candidate.model.input !== '0') models.push({ ...copy(candidate.model), input: '0' });
  for (const side of ['left', 'right']) {
    for (let index = 0; index < candidate.model[side].length; index += 1) {
      if (candidate.model[side][index].operand === '0') continue;
      const model = copy(candidate.model);
      model[side][index].operand = '0';
      models.push(model);
    }
  }
  const fault = candidate.hostile_harness_control?.injection ?? null;
  return models.map((model, index) => trial(`${candidate.id}-reduction-${index}`, model, fault));
}

function shrink(context, label, original, budget, hostileProposals = null) {
  const reproduction = execute(context, `${label}-reproduce`, original.candidate);
  const record = {
    original, reproduction, retained: original, original_size: size(original.candidate),
    failure_predicates: original.failure_predicates, max_steps: budget, attempts: [],
    accepted_shrinks: 0, exhausted: false, status: 'unreproduced',
    size_order: ['arithmetic_steps', 'numeric_magnitude', 'source_bytes'],
  };
  if (original.classification !== 'counterexample' || reproduction.classification !== 'counterexample'
    || !same(reproduction.failure_predicates, original.failure_predicates)
    || !reproduction.input_domain_preserved || !reproduction.output_domain_preserved) return record;
  record.retained = reproduction;
  let proposals = hostileProposals ?? reductions(record.retained.candidate);
  let at = 0;
  while (at < proposals.length && record.attempts.length < budget) {
    const candidate = proposals[at++];
    const observed = execute(context, `${label}-attempt-${record.attempts.length}`, candidate);
    const isSmaller = smaller(candidate, record.retained.candidate);
    const domainPreserved = observed.input_domain_preserved && observed.output_domain_preserved;
    const predicatePreserved = observed.classification === 'counterexample'
      && same(observed.failure_predicates, record.failure_predicates);
    const accepted = isSmaller && domainPreserved && predicatePreserved;
    record.attempts.push({
      candidate_size: size(candidate), smaller: isSmaller, domain_preserved: domainPreserved,
      same_failure_predicate: predicatePreserved, accepted,
      reason: !isSmaller ? 'not-smaller' : !domainPreserved ? 'input-or-output-domain-changed'
        : !predicatePreserved ? 'failure-predicate-not-reproduced' : 'smaller-reproduced-counterexample',
      observed,
    });
    if (accepted) {
      record.retained = observed;
      record.accepted_shrinks += 1;
      if (hostileProposals === null) {
        proposals = reductions(candidate);
        at = 0;
      }
    }
  }
  record.exhausted = at < proposals.length;
  record.unattempted_proposals = proposals.slice(at);
  record.retained_size = size(record.retained.candidate);
  record.status = record.exhausted ? 'exhausted' : record.accepted_shrinks ? 'reduced' : 'retained-original';
  return record;
}

function outcomeProjection(record) {
  const observed = record.observed;
  return {
    classification: record.classification, input_domain_preserved: record.input_domain_preserved,
    output_domain_preserved: record.output_domain_preserved, failure_predicates: record.failure_predicates,
    acceptance: record.acceptance.outcome, compilations: record.compilations ?? null,
    observed: observed ? {
      stage: observed.stage, outcome: observed.outcome, status: observed.status, quota_reason: observed.quota_reason,
      stack: observed.stack, request_trace: observed.request_trace, guest_requests: observed.guest_requests,
      protected_operations: observed.protected_operations, candidate_prepare_requests: observed.candidate_prepare_requests,
      native_trap: observed.native_trap, session_state: observed.session_state, metrics: observed.metrics,
      module: {
        wasm_sha256: observed.module.wasm_sha256, wat_sha256: observed.module.wat_sha256,
        source_sha256: observed.module.source_sha256, optimized: observed.module.optimized, imports: observed.module.imports,
      },
    } : null,
    diagnostic: record.diagnostic_replay ? {
      stage: record.diagnostic_replay.stage, outcome: record.diagnostic_replay.outcome,
      process_exit: record.diagnostic_replay.process_exit,
    } : null,
  };
}

function counts(records) {
  const result = { passed: 0, counterexample: 0, 'generator-failure': 0, discarded: 0, exhausted: 0, unsupported: 0, 'harness-error': 0, 'not-run': 0 };
  for (const record of records) result[record.classification] += 1;
  return result;
}

function runTrials(context, label, candidates, maximumTrials) {
  assert.ok(Number.isSafeInteger(maximumTrials) && maximumTrials >= 0 && maximumTrials <= 100);
  const records = [];
  for (const candidate of candidates) {
    if (records.length === maximumTrials) break;
    records.push(execute(context, `${label}-${candidate.id}`, candidate));
  }
  const pending = candidates.slice(records.length);
  return {
    records,
    schedule: {
      maximum_trials: maximumTrials, requested_trials: candidates.length, attempted_trials: records.length,
      passed: records.filter(record => record.classification === 'passed').length,
      counterexamples: records.filter(record => record.classification === 'counterexample').length,
      classification: pending.length ? 'exhausted' : records.length === 0 ? 'not-run'
        : records.every(record => record.classification === 'passed') ? 'passed' : 'failed',
      pending_trial_ids: pending.map(candidate => candidate.id),
      source_and_expected_records: 'generated_trials',
    },
  };
}

function retainedOriginal(record) {
  return record.status !== 'unreproduced' && record.accepted_shrinks === 0
    && record.retained.classification === 'counterexample'
    && same(record.original.candidate, record.retained.candidate)
    && same(record.original.failure_predicates, record.retained.failure_predicates);
}

export function runPropertyWorkflow(context) {
  const { fixture, configuration } = context;
  const receipt = {
    schema: 'noble-m4-property-workflow/v1', scenario: fixture, stage: 'property-harness',
    result: 'running', subject: 'independently accepted Core-Bootstrap source compiled to managed-linear-memory Wasm',
    property: fixture.input.properties, generator_revision: GENERATOR_REVISION,
    shrinker_revision: SHRINKER_REVISION, oracle_revision: ORACLE_REVISION, seed: SEED,
    bounds: {
      max_trials: fixture.input.max_trials, max_shrink_steps: fixture.input.max_shrink_steps,
      components: 2, arithmetic_steps_per_component: [1, 3], effectful_trial_period: 4,
      maximum_requests_per_trial: 2, input_domain: DOMAIN,
      replay_trials: fixture.input.max_trials, generated_counterexample_shrink_budget: fixture.input.max_shrink_steps,
    },
    environment: configuration, trials: [], replay_trials: [], controls: [], counterexamples: [], harness_errors: [],
    oracle: {
      arithmetic: 'independent JavaScript BigInt +,-,* with BigInt.asIntN(64) after every operation; never a source interpreter',
      recipe: 'explicit Lit/Def atoms, complete invocation witnesses and ordered concatenation from the generated data model',
      references: [
        'crates/noble-kernel/tests/property/generator/random.rs',
        'crates/noble-kernel/src/contracts/bootstrap/data.rs',
        'crates/noble-wasm/src/source/admission/environment.rs',
        'crates/noble-cli/src/core/runtime/abi.json',
        'crates/noble-cli/src/core/runtime/preamble.mjs',
      ],
      effects: 'resource-free test.emit only, Text --, effect ID 0, two ordered requests on each effectful trial',
      independence_limit: 'compiler, kernel, Wasm engine, host observation decoder and harness remain trusted; shared bugs may invalidate the oracle',
    },
    universal_proof_claimed: false,
    coverage_policy: 'only independently accepted, actually executed, in-domain agreeing trials count as passed; no rejected or unsupported inputs count',
  };
  const controls = new Map(fixture.input.variants.map(variant => [variant.control, variant]));
  const addControl = (id, classification, observations) => {
    const expected = controls.get(id).outcome;
    receipt.controls.push({ control: id, expected, classification, result: classification === expected ? 'passed' : 'failed', observations });
  };
  try {
    assert.equal(fixture.id, 'DX-10');
    assert.deepEqual(fixture.input.properties, [
      'compatible-composition-interface', 'retained-recipe-structure', 'reference-result-and-effect-agreement',
    ]);
    assert.equal(fixture.input.max_trials, 100);
    assert.equal(fixture.input.max_shrink_steps, 50);
    assert.equal(controls.size, CONTROL_IDS.length);
    assert.deepEqual([...controls.keys()].sort(), [...CONTROL_IDS].sort());
    assert.equal(fixture.expected.independent_acceptance_required, true);
    assert.equal(fixture.expected.universal_proof_claimed, false);
    const mapping = configuration.test_host.mappings['test.emit'];
    assert.deepEqual(mapping, { resource_free: true, module: 'noble', import: 'test_emit', effect_id: 0, contract: 'Text -- ! {test.emit}' });
    for (const key of LIMIT_KEYS) assert.ok(Number.isSafeInteger(configuration.limits[key]) && configuration.limits[key] > 0, `explicit ${key} bound`);
    assert.equal(typeof context.compilerAvailable, 'boolean');
    const generated = generate(SEED, receipt.bounds.max_trials);
    receipt.generated_trials = generated;
    receipt.replay_binding = {
      subject: receipt.subject, property: receipt.property, generator_revision: GENERATOR_REVISION,
      shrinker_revision: SHRINKER_REVISION, oracle_revision: ORACLE_REVISION, seed: SEED,
      bounds: receipt.bounds, environment: configuration, generated_trials_sha256: digest(generated),
    };
    for (const field of fixture.expected.replay_fields) {
      assert.ok(field === 'outcomes' || Object.hasOwn(receipt.replay_binding, field), `missing replay field ${field}`);
    }
    const initialRun = runTrials(context, 'DX-10', generated, receipt.bounds.max_trials);
    receipt.trials = initialRun.records;
    receipt.trial_schedule = initialRun.schedule;
    let remainingShrinkSteps = receipt.bounds.generated_counterexample_shrink_budget;
    for (const failed of receipt.trials.filter(record => record.classification === 'counterexample')) {
      const minimized = shrink(context, `DX-10-discovered-${receipt.counterexamples.length}`, failed, remainingShrinkSteps);
      remainingShrinkSteps -= minimized.attempts.length;
      receipt.counterexamples.push(minimized);
    }

    const replayed = generate(SEED, receipt.bounds.max_trials);
    const replayRun = runTrials(context, 'DX-10-replay', replayed, receipt.bounds.max_trials);
    receipt.replay_trials = replayRun.records;
    receipt.replay_schedule = replayRun.schedule;
    const firstOutcomes = receipt.trials.map(outcomeProjection), replayOutcomes = receipt.replay_trials.map(outcomeProjection);
    const sameTrials = same(generated, replayed), sameOutcomes = same(firstOutcomes, replayOutcomes);
    receipt.replay_binding.outcomes = firstOutcomes;
    addControl(CONTROL_IDS[0], sameTrials && sameOutcomes && receipt.trials.every(row => row.classification === 'passed')
      && receipt.replay_trials.every(row => row.classification === 'passed') ? 'replay-same-trials' : 'replay-failed', {
      same_generated_trials: sameTrials, same_actual_outcomes: sameOutcomes,
      original_generated_sha256: digest(generated), replay_generated_sha256: digest(replayed),
      original_outcomes_sha256: digest(firstOutcomes), replay_outcomes_sha256: digest(replayOutcomes),
      compared_observations: 'complete semantic stack/recipes/witnesses, interfaces, effects, status, metrics, module hashes/imports, acceptance and compilation counters',
      excluded_from_equality: 'command labels and artifact locations only; complete actual reports remain in both trial lists',
    });

    const model = {
      input: '100', optimization: 'off',
      left: [{ operation: '+', operand: '7', emit: null }, { operation: '*', operand: '3', emit: null }, { operation: '-', operand: '5', emit: null }],
      right: [{ operation: '+', operand: '11', emit: null }, { operation: '*', operand: '2', emit: null }, { operation: '-', operand: '9', emit: null }],
    };
    const invalid = trial('invalid-generator', model);
    invalid.source = `[ true + ] ${quotation(model.right)} ${OBSERVE}`;
    invalid.hostile_harness_control = { injection: 'generator-emits-Bool-as-add-operand', target: 'generated source; expectation still advertises I64' };
    const invalidRun = execute(context, 'DX-10-invalid-generator', invalid);
    addControl(CONTROL_IDS[1], invalidRun.classification === 'generator-failure'
      && invalidRun.diagnostic_replay?.stage === 'check' && invalidRun.diagnostic_replay?.outcome === 'type-reject'
      ? 'generator-failure-not-passed-coverage' : invalidRun.classification, {
      failed_generated_candidate: invalidRun, passed_coverage_added: 0, discarded_input: false,
    });

    const original = execute(context, 'DX-10-seeded-disagreement', trial('seeded-result-disagreement', model, 'result-plus-one'));
    const minimized = shrink(context, 'DX-10-seeded-shrink', original, receipt.bounds.max_shrink_steps);
    addControl(CONTROL_IDS[2], original.classification === 'counterexample'
      && same(original.failure_predicates, ['reference-result-and-effect-agreement/result'])
      && minimized.retained.classification === 'counterexample' && minimized.accepted_shrinks > 0
      && same(minimized.retained.failure_predicates, original.failure_predicates) ? 'counterexample' : 'counterexample-control-failed', {
      hostile_harness_control: 'expected scalar intentionally offset by one; no backend result is injected',
      shrink: minimized,
    });

    const shortModel = { ...copy(model), left: [copy(model.left[0])], right: [copy(model.right[0])] };
    const brokenInput = trial('broken-input-shrink', shortModel, 'result-plus-one');
    brokenInput.inputs = [{ type: 'Bool', value: true }];
    brokenInput.hostile_harness_control = { injection: 'shrinker-replaces-I64-input-with-Bool', target: 'runtime input; compiled entry still requires I64' };
    const brokenShrink = shrink(context, 'DX-10-domain-shrink', original, receipt.bounds.max_shrink_steps, [brokenInput]);
    const brokenAttempt = brokenShrink.attempts[0];
    addControl(CONTROL_IDS[3], retainedOriginal(brokenShrink) && brokenAttempt?.smaller
      && !brokenAttempt.domain_preserved && !brokenAttempt.accepted
      && brokenAttempt.observed.acceptance.outcome === 'accepted' && brokenAttempt.observed.observed?.status === 3
      && brokenAttempt.observed.observed.guest_requests === 0
      ? 'reject-shrink-retain-prior-counterexample' : 'domain-shrink-control-failed', {
      hostile_harness_control: 'a smaller source receives Bool at an independently accepted I64 entry; actual Wasm must reject the interface',
      shrink: brokenShrink,
    });

    const changed = trial('changed-predicate-shrink', shortModel, 'changed-recipe-expectation');
    const changedShrink = shrink(context, 'DX-10-predicate-shrink', original, receipt.bounds.max_shrink_steps, [changed]);
    const changedAttempt = changedShrink.attempts[0];
    addControl(CONTROL_IDS[4], retainedOriginal(changedShrink) && changedAttempt?.smaller && changedAttempt.domain_preserved
      && !changedAttempt.same_failure_predicate && !changedAttempt.accepted
      && same(changedAttempt.observed.failure_predicates, ['retained-recipe-structure/atoms'])
      ? 'reject-shrink-retain-prior-counterexample' : 'predicate-shrink-control-failed', {
      hostile_harness_control: 'shrinker changes oracle corruption from scalar-result to retained-recipe disagreement; actual execution is unchanged',
      shrink: changedShrink,
    });

    const exhaustingProposals = Array.from({ length: receipt.bounds.max_shrink_steps + 1 }, (_, index) => {
      const candidate = trial(`exhaust-shrink-${index}`, { ...copy(shortModel), input: String(index) });
      candidate.hostile_harness_control = { injection: 'shrinker-removes-result-oracle-corruption', target: 'expected observations', ordinal: index };
      return candidate;
    });
    const exhaustedShrink = shrink(context, 'DX-10-budget-shrink', original, receipt.bounds.max_shrink_steps, exhaustingProposals);
    addControl(CONTROL_IDS[5], retainedOriginal(exhaustedShrink) && exhaustedShrink.exhausted
      && exhaustedShrink.attempts.length === receipt.bounds.max_shrink_steps
      && exhaustedShrink.attempts.every(attempt => attempt.smaller && attempt.domain_preserved && !attempt.accepted && attempt.observed.classification === 'passed')
      ? 'retain-reproduced-failure-report-exhaustion' : 'shrink-budget-control-failed', {
      hostile_harness_control: '51 strictly smaller, in-domain proposals whose repaired oracle no longer reproduces the original disagreement; stop after 50 actual attempts',
      shrink: exhaustedShrink,
    });

    const preResult = execute(context, 'DX-10-trial-work-budget', generated[0], { steps: 0 });
    const budgetRun = runTrials(context, 'DX-10-no-trial-budget', generated, 0);
    const schedule = budgetRun.schedule;
    schedule.reason = 'hostile harness control exhausts admission budget before the first trial result';
    addControl(CONTROL_IDS[6], schedule.classification === 'exhausted' && preResult.classification === 'exhausted'
      && schedule.attempted_trials === 0 && schedule.passed === 0 && schedule.counterexamples === 0
      && preResult.observed?.outcome === 'runtime-exhausted' && [1, 2].includes(preResult.observed.status)
      && preResult.observed.stack.length === 0 && preResult.observed.guest_requests === 0
      ? 'exhaustion-not-counterexample-or-pass' : 'trial-budget-control-failed', {
      scheduler: schedule, independently_executed_zero_work_budget_probe: preResult,
      probe_counted_as_trial_pass_or_counterexample: false,
      entry_claim: 'real Wasm preparation and zero-work execution attempted; no claim that the guest body entered',
    });

    const malformed = context.malformed();
    receipt.malformed_lane = {
      lane: 'seeded malformed kernel candidates, not well-typed Wasm generation or M4 hostile backend controls',
      included_in_well_typed_trials: false, included_in_backend_controls: false, ...malformed,
    };
    const malformedObserved = malformed.observations;
    const malformedCounts = ['invalid', 'exhausted', 'unsupported', 'internal_failure'].map(key => malformedObserved?.[key]);
    addControl(CONTROL_IDS[7], malformed.process_exit === 0 && malformed.bounds?.max_trials === 200
      && malformedObserved?.trials === 200 && malformedObserved.accepted === 0 && malformedObserved.panics === 0
      && malformedCounts.every(count => Number.isSafeInteger(count) && count >= 0)
      && malformedCounts.reduce((sum, count) => sum + count, 0) === 200
      && typeof malformed.seed === 'string' && malformed.sources && malformed.command
      ? 'separate-lane' : 'malformed-lane-failed', receipt.malformed_lane);
  } catch (error) {
    receipt.harness_errors.push(errorRecord(error));
  }
  for (const variant of fixture.input.variants) {
    if (!receipt.controls.some(control => control.control === variant.control)) {
      receipt.controls.push({
        control: variant.control, expected: variant.outcome, classification: 'not-run', result: 'failed',
        observations: { reason: 'workflow failed before this control; no fallback or success inferred' },
      });
    }
  }
  receipt.outcomes = { well_typed: counts(receipt.trials), replay: counts(receipt.replay_trials) };
  receipt.outcomes.effectful_passed = receipt.trials.filter(row => row.classification === 'passed' && row.observed.guest_requests === 2).length;
  receipt.outcomes.pure_passed = receipt.trials.filter(row => row.classification === 'passed' && row.observed.guest_requests === 0).length;
  receipt.result = receipt.harness_errors.length === 0 && receipt.trials.length === 100
    && receipt.replay_trials.length === 100 && receipt.outcomes.well_typed.passed === 100
    && receipt.outcomes.replay.passed === 100 && receipt.outcomes.effectful_passed === 25 && receipt.outcomes.pure_passed === 75
    && receipt.controls.length === CONTROL_IDS.length && receipt.controls.every(control => control.result === 'passed') ? 'passed' : 'failed';
  receipt.outcome = receipt.result === 'passed' ? fixture.expected.outcome : 'property-workflow-failed';
  context.save('property-workflow.json', receipt);
  return receipt;
}
