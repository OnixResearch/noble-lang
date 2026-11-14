#!/usr/bin/env node
// ADAPT-17 managed-linear-memory lowering experiment.
//
// Reproduction (outside the repository's private campaign):
//   <selected Node 24> <pinned Node flags> verification/adapt17/gate.mjs \
//     <new directory outside the repository> [baseline-only|full]
//   <absolute path of the built noble CLI> and the pinned Node/V8, wasm-tools,
//   and wasm-opt selections are taken from the reviewed runtime configuration.
//   The runner refuses a dirty source tree and writes all raw engine artifacts
//   plus its receipt to the supplied external directory.
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

import { CoreEngine } from '../../crates/noble-cli/src/core/runtime/host.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const forms = path.join(root, 'verification/adapt17/forms');
const usage = 'usage: SELECTED_NODE verification/adapt17/gate.mjs NEW_ARTIFACT_DIRECTORY [baseline-only|full]';
const workloads = [
  { id: 'two-arm-variant-both-arms', form: 'two-arm-variant-both-arms.noble',
    inputTypes: ['I64'], runtimeInputs: [{ type: 'I64', value: '30' }] },
  { id: 'nested-pair-list', form: 'nested-pair-list.noble' },
  { id: 'runtime-captured-quote-and-composition', form: 'runtime-captured-quote-and-composition.noble',
    inputTypes: ['I64'], runtimeInputs: [{ type: 'I64', value: '7' }] },
  { id: 'reflection-only-reachable-recipe', form: 'reflection-only-reachable-recipe.noble',
    inputTypes: ['I64'], runtimeInputs: [{ type: 'I64', value: '9' }] },
];
const controls = [
  { id: 'shared-alias', form: 'shared-alias-control.noble' },
  { id: 'branch-laziness', form: 'branch-laziness-control.noble' },
  { id: 'boundary-single-variant', form: 'boundary-single-variant.noble' },
  { id: 'construction-quota', form: 'construction-quota-control.noble' },
  { id: 'invocation-quota', form: 'invocation-quota-control.noble' },
  { id: 'runtime-trap-after-ordered-effect-prefix', form: 'runtime-trap-after-effect-prefix.noble' },
  { id: 'malformed-host-boundary-rejection', form: 'malformed-boundary-control.noble', inputTypes: ['Bool'] },
];
const logicalCellBytes = 48;
const defaultLimits = {
  allocation_bytes: 65536,
  recipe_leaves: 256,
  program_depth: 256,
  operand_bytes: 2048,
  continuation_bytes: 4096,
  steps: 100000,
};
const warmupCount = 3;
const warmSampleCount = 7;
let ownedArtifactDirectory = null;
const sha256 = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const now = () => process.hrtime.bigint();
const micros = (start, end) => Number((end - start) / 1000n);
const keyFor = (id, arm, optimized) => `${id}/${arm}/opt-${optimized ? 'on' : 'off'}`;
const safeName = value => value.replace(/[^a-zA-Z0-9_-]/g, '-');
const checks = [];

function check(condition, message) {
  if (!condition) throw new Error(message);
  checks.push(message);
}

function equal(actual, expected, message) {
  check(Object.is(actual, expected), `${message}: expected ${JSON.stringify(expected)}, got ${JSON.stringify(actual)}`);
}

function deepEqual(actual, expected, message) {
  check(JSON.stringify(actual) === JSON.stringify(expected),
    `${message}: expected ${JSON.stringify(expected)}, got ${JSON.stringify(actual)}`);
}

function git(args) {
  const result = spawnSync(process.env.NOBLE_GIT ?? 'git', args,
    { cwd: root, encoding: 'utf8', env: process.env, timeout: 10000, maxBuffer: 1024 * 1024 });
  if (result.error || result.status !== 0) throw new Error(`git ${args.join(' ')} failed: ${result.error ?? result.stderr}`);
  return result.stdout.trim();
}

function cleanSourceIdentity() {
  const status = git(['status', '--porcelain']);
  check(status === '', `source tree is clean before the experiment`);
  const commit = git(['rev-parse', 'HEAD']);
  const tree = git(['rev-parse', 'HEAD^{tree}']);
  const branch = git(['branch', '--show-current']);
  return { commit, tree, branch, status: 'clean' };
}

function parseArguments(argv) {
  if (argv.length < 1 || argv.length > 2) {
    console.error(usage);
    process.exit(2);
  }
  const artifacts = path.resolve(argv[0]);
  const mode = argv[1] ?? 'baseline-only';
  if (!['baseline-only', 'full'].includes(mode)) {
    console.error(usage);
    process.exit(2);
  }
  if (artifacts === root || artifacts.startsWith(`${root}${path.sep}`)) {
    throw new Error('refusing to write experiment artifacts inside the repository');
  }
  return { artifacts, mode };
}

function loadSelection() {
  const runtime = path.join(root, 'crates/noble-cli/src/core/runtime');
  const selection = JSON.parse(fs.readFileSync(path.join(runtime, 'config.json'), 'utf8'));
  const abi = JSON.parse(fs.readFileSync(path.join(runtime, 'abi.json'), 'utf8'));
  const selectedLimits = {
    allocation_bytes: selection.limits.allocation_bytes,
    recipe_leaves: selection.limits.recipe_program_leaves,
    program_depth: selection.limits.composition_depth,
    operand_bytes: selection.limits.operand_bytes,
    continuation_bytes: selection.limits.continuation_bytes,
    steps: selection.limits.dispatch_steps,
  };
  deepEqual(selectedLimits, defaultLimits, 'experiment runtime limits equal the reviewed selection');
  const maximumLimits = {
    allocation_bytes: selection.maximum_limits[0],
    recipe_leaves: selection.maximum_limits[1],
    program_depth: selection.maximum_limits[2],
    operand_bytes: selection.maximum_limits[3],
    continuation_bytes: selection.maximum_limits[4],
    steps: selection.maximum_limits[5],
  };
  deepEqual(abi.limits_maximum, {
    ...maximumLimits,
    session_cells: abi.limits_maximum.session_cells,
    session_functions: abi.limits_maximum.session_functions,
    session_literal_bytes: abi.limits_maximum.session_literal_bytes,
  }, 'runtime ABI maxima equal the reviewed selection');
  equal(selection.timing.warmups, warmupCount, 'warm-up count matches the reviewed runtime selection');
  equal(selection.timing.samples, warmSampleCount, 'warm sample count matches the reviewed runtime selection');
  const nodePath = fs.realpathSync(process.execPath);
  check(nodePath === fs.realpathSync(selection.tools.node.path), 'running Node is the selected immutable engine');
  equal(process.versions.node, selection.tools.node.version, 'selected Node version');
  equal(process.versions.v8, selection.tools.node.v8, 'selected V8 version');
  for (const flag of selection.node_flags) check(process.execArgv.includes(flag), `selected Node flag is active: ${flag}`);
  const target = process.env.NOBLE_TARGET;
  check(typeof target === 'string' && path.isAbsolute(target), 'NOBLE_TARGET is an absolute pinned Cargo target');
  const noble = path.join(target, 'debug', 'noble');
  check(fs.existsSync(noble), 'selected debug CLI binary exists');
  fs.accessSync(noble, fs.constants.X_OK);
  const tmpdir = process.env.TMPDIR;
  check(typeof tmpdir === 'string' && path.isAbsolute(tmpdir), 'TMPDIR is explicitly selected');
  check(path.resolve(tmpdir) !== '/tmp' && path.resolve(tmpdir) !== '/var/tmp', 'TMPDIR avoids prohibited temporary filesystems');
  return { runtime, selection, abi, noble, tmpdir };
}

function normalized(value) {
  if (Array.isArray(value)) return value.map(normalized);
  if (value === null || typeof value !== 'object') return value;
  const entries = Object.entries(value)
    .filter(([name]) => !['handle', 'source_epoch', 'source_store_id'].includes(name))
    .sort(([left], [right]) => left.localeCompare(right));
  return Object.fromEntries(entries.map(([name, item]) => [name, normalized(item)]));
}

function rawStack(runtime, abi) {
  const count = Number(runtime.stack_length());
  const rows = [];
  for (let index = 0; index < count; index += 1) {
    const kind = Number(runtime.stack_kind(index));
    const value = runtime.stack_value(index);
    const type = abi.value_tags[String(kind)] ?? `kind-${kind}`;
    if (kind === 1) rows.push({ type, value: BigInt.asIntN(64, value).toString() });
    else if (kind === 2) rows.push({ type, value: Number(value) === 1 });
    else if (kind === 3) rows.push({ type, value: null });
    else rows.push({ type, boxed: true, handle: Number(BigInt.asUintN(64, value)) });
  }
  return rows;
}

function logicalPriorStack(rows) {
  return rows.map(row => row.type === 'I64' || row.type === 'Bool' || row.type === 'Unit'
    ? { type: row.type, value: row.value }
    : { type: row.type, boxed: true });
}

function sourceRecord(form, inputTypes = []) {
  const file = path.join(forms, form);
  const bytes = fs.readFileSync(file);
  const identity = Buffer.concat([
    Buffer.from('noble-adapt17-source-program/v1\0'), bytes,
    Buffer.from(JSON.stringify(inputTypes)),
  ]);
  return { id: form.replace(/\.noble$/, ''), path: path.relative(root, file), file, bytes,
    sha256: sha256(bytes), input_types: inputTypes,
    source_input_identity_sha256: sha256(identity) };
}

function compileWat(noble, tmpdir, source, arm, destination, inputTypes = []) {
  const args = ['compile', source.path];
  for (const inputType of inputTypes) args.push('--input-type', inputType);
  if (arm === 'candidate') args.push('--lowering', 'type-directed-experimental');
  else if (arm === 'baseline-explicit') args.push('--lowering', 'baseline');
  const started = now();
  const result = spawnSync(noble, args, {
    cwd: root,
    env: { ...process.env, TMPDIR: tmpdir },
    timeout: 120000,
    maxBuffer: 32 * 1024 * 1024,
  });
  const elapsed = micros(started, now());
  if (result.error || result.status !== 0) {
    const diagnostic = result.error ? String(result.error)
      : (result.stderr.length ? result.stderr : result.stdout).toString();
    throw new Error(`noble ${args.join(' ')} failed: ${diagnostic}`);
  }
  const wat = Buffer.from(result.stdout);
  check(wat.length > 0, `${source.id}/${arm} emitted WAT`);
  fs.writeFileSync(destination, wat, { flag: 'wx' });
  return { wat, sha256: sha256(wat), compile_us: elapsed,
    command: ['NOBLE_TARGET/debug/noble', ...args], stderr: result.stderr.toString() };
}

function withoutTrialCleanup(wat) {
  const fragment = fs.readFileSync(path.join(root,
    'crates/noble-wasm/runtime/experiment-cleanup.wat'), 'utf8');
  const text = wat.toString('utf8');
  check(text.includes(fragment), 'explicit baseline includes the trial-only cleanup fragment');
  return Buffer.from(text.replace(`${fragment}\n`, ''), 'utf8');
}

function candidateReport(wat, arm, id) {
  if (arm !== 'candidate') return null;
  const text = wat.toString('utf8');
  const match = text.match(/;; noble-type-directed-report:v1 specialized-known-sum-cases=(\d+) baseline-fallback-operations=(\d+)\n/);
  check(match !== null, `${id} candidate WAT contains its lowering report`);
  return { specializations: Number(match[1]), fallback_operations: Number(match[2]) };
}

function parseRun(report, engine, elapsedUs) {
  const runtime = engine.last?.exports;
  check(runtime !== undefined, 'execution left a selected guest instance for observation');
  check(typeof runtime.cleanup === 'function', 'source runtime exposes explicit guest-region cleanup');
  const phase = Number(engine.shared.phase.value);
  const beforeCleanup = rawStack(runtime, engine.abi);
  const cleanupLiveBytes = Number(runtime.cleanup());
  const releasedMetric = Object.keys(engine.abi.metrics)
    .find(key => engine.abi.metrics[key] === 'released_total_bytes');
  check(releasedMetric !== undefined, 'selected runtime ABI exposes released guest bytes');
  const cleanupReleasedBytes = Number(runtime.metric(Number(releasedMetric)));
  equal(cleanupReleasedBytes, report.metrics.allocated_since_begin_bytes,
    'explicit cleanup releases every guest cell allocated since begin');
  const cleanup = {
    live_bytes: cleanupLiveBytes,
    stack_length: Number(runtime.stack_length()),
    operand_slots: Number(engine.shared.sp.value),
    continuations: Number(engine.shared.cp.value),
    heap_delta_cells: Number(engine.shared.heap_cursor.value) - Number(engine.shared.heap_baseline.value),
    failure: Number(engine.shared.failure.value),
    released_bytes: cleanupReleasedBytes,
  };
  const semanticStack = normalized(report.stack);
  const semantic = {
    backend: report.backend,
    profile: report.profile,
    session_state: report.session_state,
    status: report.status,
    outcome: report.outcome,
    quota_reason: report.quota_reason,
    quota_phase: phase,
    native_trap: report.native_trap,
    stack: semanticStack,
    request_trace: report.request_trace,
    guest_requests: report.guest_requests,
    candidate_prepare_requests: report.candidate_prepare_requests,
    module_imports: report.module?.imports ?? null,
    prior_stack: report.status === 0 ? null : logicalPriorStack(beforeCleanup),
    cleanup: {
      live_bytes: cleanup.live_bytes,
      stack_length: cleanup.stack_length,
      operand_slots: cleanup.operand_slots,
      continuations: cleanup.continuations,
      heap_delta_cells: cleanup.heap_delta_cells,
      failure: cleanup.failure,
    },
    semantic_output_sha256: sha256(Buffer.from(JSON.stringify(semanticStack))),
  };
  return { semantic, report, phase, before_cleanup_stack: beforeCleanup,
    cleanup, elapsed_us: elapsedUs };
}

function runtimeLimits(overrides = {}) {
  return { ...defaultLimits, ...overrides };
}

function expectCleanState(row, name, failure) {
  equal(row.cleanup.live_bytes, 0, `${name} cleanup live bytes`);
  equal(row.cleanup.stack_length, 0, `${name} cleanup operand stack`);
  equal(row.cleanup.operand_slots, 0, `${name} cleanup operand slots`);
  equal(row.cleanup.continuations, 0, `${name} cleanup continuations`);
  equal(row.cleanup.heap_delta_cells, 0, `${name} cleanup retained cells`);
  equal(row.cleanup.failure, failure, `${name} cleanup failure state`);
}

function rearm(engine, module, instance, record) {
  engine.pending = {
    instance,
    module,
    record: { ...record, stem: null },
    traceStart: engine.trace.length,
  };
}

function runEngineCell(runtimeConfig, source, arm, optimized, watArtifact, label,
  { limits = {}, inputs = [], warm = false, observationId = source.id } = {}) {
  const directory = path.join(runtimeConfig.artifacts, 'engines', safeName(label));
  fs.mkdirSync(directory, { recursive: false });
  const createStarted = now();
  const engine = new CoreEngine(runtimeConfig.selection, runtimeConfig.abi,
    { optimized, artifacts: directory });
  const engineCreateUs = micros(createStarted, now());
  if (runtimeConfig.engineTools === null) runtimeConfig.engineTools = engine.tools;
  else deepEqual(engine.tools, runtimeConfig.engineTools, 'all cells use identical pinned Wasm tools');
  const prepareStarted = now();
  const ready = engine.prepare(watArtifact.wat, source.bytes, 1);
  const enginePrepareUs = micros(prepareStarted, now());
  equal(ready.outcome, 'ready', `${label} module preparation`);
  const active = engine.pending;
  check(active !== null, `${label} prepared module is pending`);
  const module = active.module;
  const instance = active.instance;
  const record = active.record;
  const oneRun = selectedInputs => {
    const started = now();
    const report = engine.execute({ inputs: selectedInputs, limits: runtimeLimits(limits) });
    const elapsedUs = micros(started, now());
    return parseRun(report, engine, elapsedUs);
  };
  let first;
  const warmupSamples = [];
  const samples = [];
  let error = null;
  try {
    first = oneRun(inputs);
    if (warm) {
      check(first.semantic.status === 0, `${label} cold workload execution is normal`);
      check(first.cleanup.live_bytes === 0 && first.cleanup.stack_length === 0
        && first.cleanup.operand_slots === 0 && first.cleanup.continuations === 0,
      `${label} cold execution cleanup is complete`);
      for (let index = 0; index < warmupCount; index += 1) {
        rearm(engine, module, instance, record);
        const sample = oneRun(inputs);
        deepEqual(sample.semantic, first.semantic,
          `${label} warm-up ${index + 1} preserves the complete observation`);
        check(sample.cleanup.live_bytes === 0 && sample.cleanup.stack_length === 0
          && sample.cleanup.operand_slots === 0 && sample.cleanup.continuations === 0,
        `${label} warm-up ${index + 1} cleanup is complete`);
        warmupSamples.push({ elapsed_us: sample.elapsed_us });
      }
      for (let index = 0; index < warmSampleCount; index += 1) {
        rearm(engine, module, instance, record);
        const sample = oneRun(inputs);
        deepEqual(sample.semantic, first.semantic, `${label} warm sample ${index + 1} preserves the complete observation`);
        samples.push({ elapsed_us: sample.elapsed_us,
          allocated_since_begin_bytes: sample.report.metrics.allocated_since_begin_bytes,
          retained_before_begin_bytes: sample.report.metrics.retained_before_begin_bytes,
          memory_bytes: sample.report.metrics.memory_bytes,
          execution_steps: sample.report.metrics.execution_steps,
          semantic_output_sha256: sample.semantic.semantic_output_sha256 });
      }
    }
  } catch (caught) {
    error = caught;
    throw caught;
  } finally {
    engine.close();
  }
  const report = first.report;
  const lowerReport = candidateReport(watArtifact.wat, arm, source.id);
  check(report.candidate_prepare_requests === 0, `${label} starts no candidate preparation request`);
  const timing = {
    compiler_wat_us: watArtifact.compile_us,
    engine_create_us: engineCreateUs,
    engine_prepare_us: enginePrepareUs,
    first_execute_us: first.elapsed_us,
    cold_total_us: watArtifact.compile_us + engineCreateUs + enginePrepareUs + first.elapsed_us,
    warmups: warm ? warmupCount : 0,
    warmup_execute_us: warmupSamples.map(sample => sample.elapsed_us),
    warm_samples: samples,
    warm_median_us: warm ? median(samples.map(sample => sample.elapsed_us)) : null,
  };
  return {
    workload: observationId,
    arm,
    optimized,
    source: { path: source.path, bytes: source.bytes.length, sha256: source.sha256,
      input_types: source.input_types, source_input_identity_sha256: source.source_input_identity_sha256 },
    module: Object.fromEntries(Object.entries(report.module).filter(([key]) => key !== 'stem')),
    module_sha256: report.module.wasm_sha256,
    wat_sha256: watArtifact.sha256,
    lowering_report: lowerReport,
    observation: first.semantic,
    metrics: report.metrics,
    runtime_limits: runtimeLimits(limits),
    runtime_inputs: inputs,
    before_cleanup_stack: first.before_cleanup_stack,
    cleanup: first.cleanup,
    timing,
    ...(error ? { error: String(error) } : {}),
  };
}

function median(values) {
  const ordered = [...values].sort((left, right) => left - right);
  return ordered[Math.floor(ordered.length / 2)];
}

function expectNormal(row, name) {
  equal(row.observation.status, 0, `${name} status`);
  equal(row.observation.outcome, 'normal', `${name} outcome`);
  equal(row.observation.quota_reason, null, `${name} quota reason`);
  equal(row.observation.candidate_prepare_requests, 0, `${name} candidate preparation requests`);
  expectCleanState(row, name, 0);
}

function checkWorkloadExpected(row) {
  const stack = row.observation.stack;
  expectNormal(row, row.workload);
  equal(row.observation.request_trace.length, 0, `${row.workload} request prefix`);
  if (row.workload === 'two-arm-variant-both-arms') {
    deepEqual(stack, [{ type: 'Pair', value: [
      { type: 'I64', value: '30' }, { type: 'I64', value: '35' },
    ] }], 'both finite Sum arms consume post-compilation input');
  } else if (row.workload === 'nested-pair-list') {
    deepEqual(stack, [{ type: 'Pair', value: [
      { type: 'Pair', value: [{ type: 'I64', value: '1' }, { type: 'I64', value: '2' }] },
      { type: 'List', value: [{ type: 'I64', value: '4' }, { type: 'I64', value: '3' }] },
    ] }], 'nested Pair/List observation');
  } else if (row.workload === 'runtime-captured-quote-and-composition') {
    deepEqual(stack, [{ type: 'I64', value: '8' }], 'runtime-captured composition result');
  } else if (row.workload === 'reflection-only-reachable-recipe') {
    equal(stack.length, 2, 'reflection preserves the input below its output');
    equal(stack[0].type, 'I64', 'reflection preserves the input type');
    equal(stack[0].value, '9', 'reflection preserves the runtime input');
    equal(stack[1].type, 'Syntax', 'reflection output type');
    deepEqual(stack[1].recipe, [
      { literal: { type: 'I64', value: '9' } },
      { invoke: 'builtin:i64.add' },
    ], 'exact reflection-only reachable recipe and captured value');
  }
}

function compareSemantic(base, candidate, label) {
  deepEqual(candidate.observation, base.observation, `${label} independent semantic comparison`);
  equal(candidate.source.sha256, base.source.sha256, `${label} source bytes`);
  equal(candidate.source.source_input_identity_sha256, base.source.source_input_identity_sha256,
    `${label} checked source/input identity`);
  equal(candidate.module.source_sha256, base.module.source_sha256, `${label} module source identity`);
  deepEqual(candidate.module.imports, base.module.imports, `${label} checked import identity`);
}

function checkControlExpected(controlId, row) {
  const name = `${controlId}/${row.arm}/opt-${row.optimized ? 'on' : 'off'}`;
  if (controlId === 'shared-alias') {
    expectNormal(row, name);
    equal(row.observation.stack.length, 2, `${name} retains its Program and Pair roots`);
    equal(row.observation.stack[0].type, 'Program', `${name} captured Program root type`);
    deepEqual(row.observation.stack[0].recipe, [
      { literal: { type: 'I64', value: '7' } },
    ], `${name} captured Program recipe`);
    equal(row.observation.stack[1].type, 'Pair', `${name} alias result type`);
    equal(row.observation.stack[1].value[0].type, 'Program', `${name} aliased Program retained`);
    equal(row.observation.stack[1].value[1].type, 'I64', `${name} selected branch result type`);
    equal(row.observation.stack[1].value[1].value, '1', `${name} selected branch result`);
    deepEqual(row.observation.stack[1].value[0].recipe, [
      { literal: { type: 'I64', value: '1' } },
      { invoke: 'builtin:i64.add' },
    ], `${name} exact retained branch Program recipe`);
  } else if (controlId === 'branch-laziness') {
    expectNormal(row, name);
    deepEqual(row.observation.stack, [{ type: 'Pair', value: [
      { type: 'I64', value: '11' }, { type: 'I64', value: '22' },
    ] }], `${name} both selected branches are lazy`);
    deepEqual(row.observation.request_trace, [], `${name} unselected abort branches make no request`);
  } else if (controlId === 'construction-quota') {
    equal(row.observation.status, 2, `${name} quotation-construction quota status`);
    equal(row.observation.outcome, 'runtime-exhausted', `${name} initialization quota outcome`);
    equal(row.observation.quota_reason, 'recipe leaves', `${name} initialization quota reason`);
    equal(row.observation.quota_phase, 1, `${name} quotation-construction quota phase`);
    expectCleanState(row, name, 2);
  } else if (controlId === 'invocation-quota') {
    equal(row.observation.status, 2, `${name} invocation quota status`);
    equal(row.observation.outcome, 'runtime-exhausted', `${name} invocation quota outcome`);
    equal(row.observation.quota_reason, 'execution work', `${name} invocation quota reason`);
    equal(row.observation.quota_phase, 1, `${name} invocation quota phase`);
    expectCleanState(row, name, 2);
  } else if (controlId === 'runtime-trap-after-ordered-effect-prefix') {
    equal(row.observation.status, 6, `${name} abort status`);
    equal(row.observation.outcome, 'trap', `${name} abort outcome`);
    equal(row.observation.quota_phase, 1, `${name} abort invocation phase`);
    deepEqual(row.observation.request_trace, ['test.emit:first', 'test.abort'], `${name} exact ordered request prefix`);
    deepEqual(row.observation.prior_stack, [{ type: 'I64', value: '42' }], `${name} preserves prior stack`);
    expectCleanState(row, name, 6);
  } else if (controlId === 'malformed-host-boundary-rejection') {
    equal(row.observation.status, 6, `${name} malformed host value status`);
    equal(row.observation.outcome, 'trap', `${name} malformed host value outcome`);
    equal(row.observation.quota_phase, 0, `${name} host boundary phase`);
    check(String(row.observation.native_trap).includes('invalid Bool input'), `${name} malformed value is rejected before guest execution`);
    deepEqual(row.observation.request_trace, [], `${name} no request on malformed input`);
    deepEqual(row.observation.prior_stack, [], `${name} no partial host value reaches guest stack`);
    expectCleanState(row, name, 0);
  }
}

function verifyFallback(row) {
  if (row.arm !== 'candidate') return;
  const expectedSpecializations = row.workload === 'two-arm-variant-both-arms'
    || row.workload === 'branch-laziness' ? 2
    : row.workload.startsWith('allocation-boundary-') ? 1 : 0;
  equal(row.lowering_report.specializations, expectedSpecializations,
    `${row.workload} candidate specialization count`);
  check(row.lowering_report.fallback_operations > 0,
    `${row.workload} candidate reports baseline-fallback operations`);
}

function boundaryLimits(source, arm, optimized, rows, runtimeConfig, watArtifact) {
  for (const allocationBytes of [47, 48, 49]) {
    const id = `allocation-boundary-${allocationBytes}`;
    const inputs = [];
    const row = runEngineCell(runtimeConfig, source, arm, optimized,
      watArtifact, id, { limits: { allocation_bytes: allocationBytes }, inputs,
        observationId: id });
    verifyFallback(row);
    rows.push(row);
  }
}

function matrixObservations(workloadRows, mode) {
  const cells = [];
  for (const workload of workloads) {
    for (const optimized of [false, true]) {
      const baseline = workloadRows[keyFor(workload.id, 'baseline', optimized)];
      checkWorkloadExpected(baseline);
      verifyFallback(baseline);
      cells.push(baseline);
      if (mode === 'full') {
        const candidate = workloadRows[keyFor(workload.id, 'candidate', optimized)];
        checkWorkloadExpected(candidate);
        verifyFallback(candidate);
        compareSemantic(baseline, candidate, keyFor(workload.id, 'candidate', optimized));
        cells.push(candidate);
      }
    }
  }
  return cells;
}

function checkBoundaryGroups(boundaries, mode) {
  const arms = mode === 'full' ? ['baseline', 'candidate'] : ['baseline'];
  for (const arm of arms) {
    for (const optimized of [false, true]) {
      const rows = boundaries.filter(row => row.arm === arm && row.optimized === optimized);
      equal(rows.length, 3, `${arm}/opt-${optimized ? 'on' : 'off'} boundary controls count`);
      const byLimit = new Map(rows.map(row => [Number(row.workload.slice('allocation-boundary-'.length)), row]));
      const chargeRow = byLimit.get(48);
      const measuredCharge = chargeRow.metrics.allocated_since_begin_bytes;
      equal(measuredCharge, arm === 'baseline' ? logicalCellBytes : 0,
        `${arm}/opt-${optimized ? 'on' : 'off'} checked specialized-wrapper charge`);
      for (const limit of [47, 48, 49]) {
        const row = byLimit.get(limit);
        const expectedStatus = limit < measuredCharge ? 2 : 0;
        equal(row.observation.status, expectedStatus, `${arm}/opt-${optimized ? 'on' : 'off'} allocation limit ${limit} status`);
        equal(row.observation.quota_reason, expectedStatus === 2 ? 'allocation' : null,
          `${arm}/opt-${optimized ? 'on' : 'off'} allocation limit ${limit} reason`);
        equal(row.observation.quota_phase, 1,
          `${arm}/opt-${optimized ? 'on' : 'off'} allocation limit ${limit} phase`);
        expectCleanState(row, `${arm}/opt-${optimized ? 'on' : 'off'} allocation limit ${limit}`,
          expectedStatus);
        if (mode === 'full' && arm === 'candidate') {
          const baseline = boundaries.find(other => other.arm === 'baseline'
            && other.optimized === optimized && other.workload === row.workload);
          if (row.observation.status === baseline.observation.status) {
            deepEqual(row.observation, baseline.observation,
              `candidate/baseline ${row.workload} boundary equivalence at matching outcomes`);
          }
        }
      }
    }
  }
}

function decisionFor(matrix) {
  const reasons = [];
  const byWorkloadOpt = new Map(matrix.map(row => [keyFor(row.workload, row.arm, row.optimized), row]));
  let baselineAllocated = 0, candidateAllocated = 0;
  let baselineLinearMemory = 0, candidateLinearMemory = 0;
  let baselineRetainedBeforeBegin = 0, candidateRetainedBeforeBegin = 0;
  const family = {};
  let maximumCellRegressionPct = -Infinity;
  for (const workload of workloads) {
    const familyRows = {};
    for (const optimized of [false, true]) {
      const baseline = byWorkloadOpt.get(keyFor(workload.id, 'baseline', optimized));
      const candidate = byWorkloadOpt.get(keyFor(workload.id, 'candidate', optimized));
      baselineAllocated += baseline.metrics.allocated_since_begin_bytes;
      candidateAllocated += candidate.metrics.allocated_since_begin_bytes;
      baselineLinearMemory += baseline.metrics.memory_bytes;
      candidateLinearMemory += candidate.metrics.memory_bytes;
      baselineRetainedBeforeBegin += baseline.metrics.retained_before_begin_bytes;
      candidateRetainedBeforeBegin += candidate.metrics.retained_before_begin_bytes;
      const baselineMedian = baseline.timing.warm_median_us;
      const candidateMedian = candidate.timing.warm_median_us;
      check(baselineMedian > 0 && candidateMedian > 0,
        `${workload.id}/opt-${optimized ? 'on' : 'off'} warm medians exceed timer resolution`);
      const regressionPct = baselineMedian === 0 ? 0 : (candidateMedian - baselineMedian) / baselineMedian * 100;
      maximumCellRegressionPct = Math.max(maximumCellRegressionPct, regressionPct);
      familyRows[optimized ? 'on' : 'off'] = { baseline_median_us: baselineMedian,
        candidate_median_us: candidateMedian, regression_pct: regressionPct };
      if (regressionPct > 5) reasons.push(`${workload.id}/opt-${optimized ? 'on' : 'off'} regressed ${regressionPct.toFixed(2)}%`);
    }
    const baselineMean = (familyRows.off.baseline_median_us + familyRows.on.baseline_median_us) / 2;
    const candidateMean = (familyRows.off.candidate_median_us + familyRows.on.candidate_median_us) / 2;
    const gainPct = baselineMean === 0 ? 0 : (baselineMean - candidateMean) / baselineMean * 100;
    family[workload.id] = { ...familyRows, optimizer_averaged_gain_pct: gainPct };
  }
  const improvedFamilies = Object.values(family).filter(row => row.optimizer_averaged_gain_pct >= 5).length;
  if (candidateAllocated > baselineAllocated) reasons.push('candidate total allocation charge exceeded baseline');
  if (candidateLinearMemory > baselineLinearMemory) reasons.push('candidate guest linear-memory size exceeded baseline');
  if (candidateRetainedBeforeBegin > baselineRetainedBeforeBegin)
    reasons.push('candidate guest cells retained before begin exceeded baseline');
  if (improvedFamilies < 2) reasons.push(`only ${improvedFamilies} workload families met the 5% warm-time gain`);
  const specializable = workloads.find(row => row.id === 'two-arm-variant-both-arms');
  for (const optimized of [false, true]) {
    const baseline = byWorkloadOpt.get(keyFor(specializable.id, 'baseline', optimized));
    const candidate = byWorkloadOpt.get(keyFor(specializable.id, 'candidate', optimized));
    const expectedSaving = candidate.lowering_report.specializations * logicalCellBytes;
    equal(baseline.metrics.allocated_since_begin_bytes - candidate.metrics.allocated_since_begin_bytes,
      expectedSaving, `two-arm candidate saves one cell per reached known Sum case (${optimized ? 'opt-on' : 'opt-off'})`);
  }
  return {
    outcome: reasons.length === 0 ? 'GO' : 'NO-GO',
    reasons,
    allocation: { baseline_total_bytes: baselineAllocated, candidate_total_bytes: candidateAllocated,
      baseline_linear_memory_size_bytes: baselineLinearMemory,
      candidate_linear_memory_size_bytes: candidateLinearMemory,
      baseline_retained_before_begin_bytes: baselineRetainedBeforeBegin,
      candidate_retained_before_begin_bytes: candidateRetainedBeforeBegin,
      maximum_cell_regression_pct: maximumCellRegressionPct, improved_workload_families: improvedFamilies },
    warm_time_by_family: family,
  };
}

function main() {
  const { artifacts, mode } = parseArguments(process.argv.slice(2));
  const sourceIdentity = cleanSourceIdentity();
  const runtimeConfig = loadSelection();
  runtimeConfig.artifacts = artifacts;
  runtimeConfig.engineTools = null;
  fs.mkdirSync(artifacts, { recursive: false });
  ownedArtifactDirectory = artifacts;
  fs.mkdirSync(path.join(artifacts, 'wat'));
  fs.mkdirSync(path.join(artifacts, 'engines'));
  const cliSha256 = sha256(fs.readFileSync(runtimeConfig.noble));
  const formSpecs = Array.from(new Map([...workloads, ...controls].map(row => [row.form, row])).values());
  const sources = new Map();
  const compiled = new Map();
  const compilerRecords = [];
  for (const spec of formSpecs) {
    const source = sourceRecord(spec.form, spec.inputTypes ?? []);
    sources.set(spec.form, source);
    const defaultPath = path.join(artifacts, 'wat', `${spec.id}-default.wat`);
    const defaultBaseline = compileWat(runtimeConfig.noble, runtimeConfig.tmpdir, source, 'default', defaultPath,
      spec.inputTypes ?? []);
    const explicitPath = path.join(artifacts, 'wat', `${spec.id}-baseline-explicit.wat`);
    const explicit = compileWat(runtimeConfig.noble, runtimeConfig.tmpdir, source, 'baseline-explicit', explicitPath,
      spec.inputTypes ?? []);
    equal(sha256(withoutTrialCleanup(explicit.wat)), defaultBaseline.sha256,
      `${spec.id} default CLI equals explicit baseline after removing only trial cleanup`);
    check(!defaultBaseline.wat.toString('utf8').includes('(export "cleanup"'),
      `${spec.id} default CLI does not expose trial cleanup`);
    compiled.set(`${spec.form}/baseline`, explicit);
    compilerRecords.push({ source: source.id, arm: 'default-baseline', bytes: defaultBaseline.wat.length,
      sha256: defaultBaseline.sha256, compile_us: defaultBaseline.compile_us,
      command: defaultBaseline.command, explicit_baseline_sha256: explicit.sha256,
      explicit_baseline_compile_us: explicit.compile_us, explicit_baseline_command: explicit.command });
    if (mode === 'full') {
      const candidatePath = path.join(artifacts, 'wat', `${spec.id}-candidate.wat`);
      const candidate = compileWat(runtimeConfig.noble, runtimeConfig.tmpdir, source, 'candidate', candidatePath,
        spec.inputTypes ?? []);
      const lowerReport = candidateReport(candidate.wat, 'candidate', spec.id);
      compiled.set(`${spec.form}/candidate`, { ...candidate, lowerReport });
      compilerRecords.push({ source: source.id, arm: 'type-directed-experimental', bytes: candidate.wat.length,
        sha256: candidate.sha256, compile_us: candidate.compile_us, command: candidate.command,
        lowering_report: lowerReport });
    }
  }
  const warmMatrix = {};
  for (const arm of mode === 'full' ? ['baseline', 'candidate'] : ['baseline']) {
    for (const workload of workloads) {
      const source = sources.get(workload.form);
      for (const optimized of [false, true]) {
        const row = runEngineCell(runtimeConfig, source, arm, optimized,
          compiled.get(`${workload.form}/${arm}`), keyFor(workload.id, arm, optimized),
          { warm: true, inputs: workload.runtimeInputs ?? [], observationId: workload.id });
        verifyFallback(row);
        warmMatrix[keyFor(workload.id, arm, optimized)] = row;
      }
    }
  }
  matrixObservations(warmMatrix, mode);
  const controlRows = [];
  const boundaryRows = [];
  for (const arm of mode === 'full' ? ['baseline', 'candidate'] : ['baseline']) {
    for (const optimized of [false, true]) {
      for (const control of controls) {
        const source = sources.get(control.form);
        const watArtifact = compiled.get(`${control.form}/${arm}`);
        if (control.id === 'construction-quota') {
          const row = runEngineCell(runtimeConfig, source, arm, optimized, watArtifact,
            keyFor(control.id, arm, optimized), { limits: { recipe_leaves: 0 }, observationId: control.id });
          verifyFallback(row);
          checkControlExpected(control.id, row);
          controlRows.push(row);
        } else if (control.id === 'invocation-quota') {
          const row = runEngineCell(runtimeConfig, source, arm, optimized, watArtifact,
            keyFor(control.id, arm, optimized), { limits: { steps: 1 }, observationId: control.id });
          verifyFallback(row);
          checkControlExpected(control.id, row);
          controlRows.push(row);
        } else if (control.id === 'malformed-host-boundary-rejection') {
          const row = runEngineCell(runtimeConfig, source, arm, optimized, watArtifact,
            keyFor(control.id, arm, optimized), { inputs: [{ type: 'Bool', value: 'not-a-Bool' }],
              observationId: control.id });
          verifyFallback(row);
          checkControlExpected(control.id, row);
          controlRows.push(row);
        } else if (control.id === 'shared-alias' || control.id === 'branch-laziness'
          || control.id === 'runtime-trap-after-ordered-effect-prefix') {
          const row = runEngineCell(runtimeConfig, source, arm, optimized, watArtifact,
            keyFor(control.id, arm, optimized), { observationId: control.id });
          verifyFallback(row);
          checkControlExpected(control.id, row);
          controlRows.push(row);
        }
      }
      const boundary = controls.find(row => row.id === 'boundary-single-variant');
      // This is an additional control, separate from the five canonical case controls.
      boundaryLimits(sources.get(boundary.form), arm, optimized, boundaryRows, runtimeConfig,
        compiled.get(`${boundary.form}/${arm}`));
    }
  }
  for (const row of controlRows) {
    if (mode === 'full') {
      const baseline = controlRows.find(other => other.workload === row.workload
        && other.optimized === row.optimized && other.arm === 'baseline');
      if (row.arm === 'candidate') compareSemantic(baseline, row, keyFor(row.workload, row.arm, row.optimized));
    }
  }
  checkBoundaryGroups(boundaryRows, mode);
  let decision = { outcome: 'pending-candidate-comparison', reasons: [] };
  if (mode === 'full') {
    const completeMatrix = Object.values(warmMatrix);
    decision = decisionFor(completeMatrix);
  }
  const finalStatus = git(['status', '--porcelain']);
  check(finalStatus === '', 'source tree remains clean after all executions');
  const sourceFiles = [
    'Cargo.lock',
    'crates/noble-cli/Cargo.toml',
    'crates/noble-cli/src/core.rs',
    'crates/noble-cli/src/core/arguments.rs',
    'crates/noble-cli/src/core/companions/session.rs',
    'crates/noble-cli/src/core/entry/compilation/legacy.rs',
    'crates/noble-cli/src/core/worker/assembly.rs',
    'crates/noble-cli/src/core/runtime/config.json',
    'crates/noble-cli/src/core/runtime/abi.json',
    'crates/noble-cli/src/core/runtime/host.mjs',
    'crates/noble-cli/src/core/runtime/preamble.mjs',
    'crates/noble-cli/src/core/runtime/values.mjs',
    'crates/noble-cli/src/core/runtime/engine-host.mjs',
    'crates/noble-cli/src/core/runtime/engine.mjs',
    'crates/noble-wasm/runtime/data.wat',
    'crates/noble-wasm/runtime/experiment-cleanup.wat',
    'crates/noble-wasm/runtime/source.wat',
    'crates/noble-wasm/Cargo.toml',
    'crates/noble-wasm/src/source/emit.rs',
    'crates/noble-wasm/src/source/emit/functions.rs',
    'crates/noble-wasm/src/source/emit/imports/entry.rs',
    'crates/noble-wasm/src/source/mod.rs',
    'crates/noble-wasm/src/source/plan.rs',
    'crates/noble-wasm/src/source/plan/operations.rs',
    'crates/noble-wasm/src/source/transaction.rs',
    'specs/BACKEND-EXPERIMENTS.md',
    'specs/STATUS.json',
    'specs/conformance/adaptation-cases.json',
    'verification/adapt17/freeze.md',
    'verification/adapt17/gate.mjs',
    'verification/adapt17/forms/branch-laziness-control.noble',
    'verification/adapt17/forms/boundary-single-variant.noble',
    'verification/adapt17/forms/construction-quota-control.noble',
    'verification/adapt17/forms/invocation-quota-control.noble',
    'verification/adapt17/forms/malformed-boundary-control.noble',
    'verification/adapt17/forms/nested-pair-list.noble',
    'verification/adapt17/forms/reflection-only-reachable-recipe.noble',
    'verification/adapt17/forms/runtime-captured-quote-and-composition.noble',
    'verification/adapt17/forms/runtime-trap-after-effect-prefix.noble',
    'verification/adapt17/forms/shared-alias-control.noble',
    'verification/adapt17/forms/two-arm-variant-both-arms.noble',
  ];
  const toolHashes = runtimeConfig.engineTools;
  const receipt = {
    schema: 'noble-adapt17-experiment-receipt/v1',
    mode,
    source: sourceIdentity,
    invocation: { harness: 'verification/adapt17/gate.mjs',
      cli: 'NOBLE_TARGET/debug/noble', artifact_directory: 'external directory (not recorded)', mode },
    configuration: {
      selected_profile: runtimeConfig.selection.scope,
      target: runtimeConfig.selection.target,
      nixpkgs_revision: runtimeConfig.selection.nixpkgs.revision,
      wasm_feature_policy: runtimeConfig.selection.feature_policy,
      node: { path: fs.realpathSync(process.execPath), version: process.versions.node,
        v8: process.versions.v8, flags: runtimeConfig.selection.node_flags,
        active_flags: process.execArgv },
      tools: toolHashes,
      wasm_optimizer_flags: runtimeConfig.selection.optimizer_flags,
      runtime_limits: defaultLimits,
      logical_cell_bytes: logicalCellBytes,
      workload_optimizer_settings: ['off', 'on'],
      warmup_count: warmupCount,
      warm_samples_per_cell: warmSampleCount,
      timing_clock: 'process.hrtime.bigint',
      engine_process_memory: 'unsupported: complete Node/V8 RSS is not measured; guest linear-memory bytes are reported by the selected runtime ABI',
      cli_binary: { path: 'NOBLE_TARGET/debug/noble', sha256: cliSha256 },
      experiment_source_sha256: Object.fromEntries(sourceFiles.map(file => [file,
        sha256(fs.readFileSync(path.join(root, file)))])),
      abi_sha256: sha256(fs.readFileSync(path.join(runtimeConfig.runtime, 'abi.json'))),
      runtime_config_sha256: sha256(fs.readFileSync(path.join(runtimeConfig.runtime, 'config.json'))),
    },
    workloads: Object.fromEntries(Object.entries(warmMatrix)),
    controls: { case_controls: controlRows, allocation_limit_boundaries: boundaryRows,
      shared_alias_and_branch_laziness_are_additional_controls: true,
      runtime_capture_control: 'runtime-captured-quote-and-composition is included in the workload matrix' },
    compiler: compilerRecords,
    decision,
    validation: { result: 'passed', checks: checks.length,
      claim_boundary: 'finite checked workloads and controls only; no universal equivalence, WIT ownership, process-memory, or default-backend claim' },
  };
  const receiptPath = path.join(artifacts, 'receipt.json');
  fs.writeFileSync(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`, { flag: 'wx' });
  const summary = mode === 'baseline-only'
    ? `ADAPT17_GATE baseline-only: workloads=${workloads.length * 2}, controls=${controlRows.length + boundaryRows.length}, checks=${checks.length}, result=PASS`
    : `ADAPT17_GATE full: workloads=${workloads.length * 4}, controls=${controlRows.length + boundaryRows.length}, checks=${checks.length}, decision=${decision.outcome}`;
  console.log(`${summary} receipt=${receiptPath} source=${sourceIdentity.commit}`);
}

try {
  main();
} catch (error) {
  const args = process.argv.slice(2);
  const artifacts = args.length ? path.resolve(args[0]) : null;
  if (artifacts && artifacts === ownedArtifactDirectory) {
    fs.writeFileSync(path.join(artifacts, 'failure.json'), `${JSON.stringify({
      schema: 'noble-adapt17-experiment-failure/v1', error: `${error.name}: ${error.message}`,
      stack: error.stack ?? null, checks_completed: checks.length,
    }, null, 2)}\n`, { flag: 'wx' });
  }
  console.error(error.stack ?? error);
  process.exitCode = 1;
}
