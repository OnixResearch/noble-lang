#!/usr/bin/env node
// One fresh Node process owns one fixed module and no guest imports. Runtime
// inputs arrive on stdin only after compilation/instantiation and the ready receipt.
import fs from 'node:fs';
import { once } from 'node:events';
import { REPRESENTATIONS, METRICS, LIMIT_KEYS, CONTROL_IDS, UNKNOWN_METRICS, sha256, digest,
  equal, unknown, validateConfig, runtimeCases, fixedControls, boundaryCases,
  observationErrors, aggregate } from './m3-wasm-policy.mjs';

const emit = message => process.stdout.write(`${JSON.stringify(message)}\n`);
const elapsed = start => Number(process.hrtime.bigint() - start);
function timed(call) {
  const start = process.hrtime.bigint();
  const value = call();
  return { value, ns: elapsed(start) };
}
function memoryObservation(label) {
  let highWater;
  try {
    const match = /^VmHWM:\s+(\d+)\s+kB$/m.exec(fs.readFileSync('/proc/self/status', 'utf8'));
    highWater = match ? { status: 'observed', value: Number(match[1]) * 1024, unit: 'bytes' }
      : unknown('Linux /proc/self/status does not expose VmHWM');
  } catch (error) { highWater = unknown(String(error)); }
  return { label, scope: 'entire-fresh-node-worker-including-v8-js-host-module-inputs-and-retained-observations',
    process_memory_usage_bytes: process.memoryUsage(), linux_proc_self_vmhwm_bytes: highWater,
    attribution_to_guest_or_engine_only: false };
}

async function main() {
  if (process.argv.length !== 3) throw Error('worker requires exactly one driver-created job file');
  const job = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'));
  const config = validateConfig(job.configuration);
  if (process.versions.node !== config.tools.node.version || process.versions.v8 !== config.tools.node.v8
    || fs.realpathSync(process.execPath) !== fs.realpathSync(config.tools.node.path)
    || !equal(process.execArgv, config.node_flags) || Object.hasOwn(process.env, 'NODE_OPTIONS')) {
    throw Error('worker engine/version/flags differ from pinned configuration');
  }
  if (!REPRESENTATIONS.includes(job.representation)) throw Error('invalid representation');
  const before = memoryObservation('before-feature-probes-and-main-module-compilation');
  let compileCount = 0;
  function compile(bytes) { compileCount += 1; return new WebAssembly.Module(bytes); }
  const features = {};
  for (const probe of job.feature_probes) {
    const bytes = fs.readFileSync(probe.path);
    if (sha256(bytes) !== probe.sha256) throw Error(`feature probe byte mismatch: ${probe.name}`);
    const record = { module_sha256: probe.sha256, supported: false, probe: probe.name,
      used_by_workload: probe.name === 'gc' ? job.representation === 'wasm-gc' : false,
      required_by_workload: probe.name === 'gc' && job.representation === 'wasm-gc' };
    try {
      const module = compile(bytes);
      const imports = WebAssembly.Module.imports(module);
      if (imports.length) throw Error('feature probe unexpectedly imports host functions');
      const instance = new WebAssembly.Instance(module, Object.create(null));
      const observed = probe.name === 'component' ? null : instance.exports.probe();
      record.observed = observed;
      record.supported = probe.name === 'component' || equal(observed, probe.expected);
      record.outcome = record.supported ? 'supported-by-probe' : 'unexpected-probe-result';
    } catch (error) {
      record.outcome = 'not-supported-by-selected-core-api';
      record.error = `${error.name}: ${error.message}`;
    }
    features[probe.name] = record;
  }
  const bytes = fs.readFileSync(job.module.path);
  if (sha256(bytes) !== job.module.sha256) throw Error('main module byte mismatch');
  const firstCompile = timed(() => compile(bytes));
  const module = firstCompile.value;
  const imports = WebAssembly.Module.imports(module);
  if (imports.length !== 0) throw Error(`nonempty guest import surface: ${JSON.stringify(imports)}`);
  const firstInstantiate = timed(() => new WebAssembly.Instance(module, Object.create(null)));
  const instance = firstInstantiate.value;
  const exports = instance.exports;
  for (const name of ['begin', 'build_capture', 'build_tree', 'build_static', 'invoke', 'invoke2',
    'run_list', 'run_continuation', 'result', 'reflect', 'recipe_length', 'recipe_kind', 'recipe_value',
    'cleanup', 'metric', 'program_depth', 'program_leaves']) {
    if (typeof exports[name] !== 'function') throw Error(`missing runtime export: ${name}`);
  }
  if (!(exports.memory instanceof WebAssembly.Memory)) throw Error('missing bounded linear scratch memory');
  const compilation = { first_ns: firstCompile.ns, samples: [],
    scope: 'synchronous-WebAssembly.Module; repeated-identical-bytes-may-use-v8-process-cache' };
  const instantiation = { first_ns: firstInstantiate.ns, samples: [],
    scope: 'synchronous-WebAssembly.Instance-including-module-start-and-permanent-vocabulary' };
  for (const role of ['warmup', 'sample']) {
    const count = role === 'warmup' ? config.timing.warmups : config.timing.samples;
    for (let index = 0; index < count; index += 1) {
      compilation.samples.push({ role, index, ns: timed(() => compile(bytes)).ns });
      instantiation.samples.push({ role, index, ns: timed(() => new WebAssembly.Instance(module, Object.create(null))).ns });
    }
  }
  compilation.aggregate_ns = aggregate(compilation.samples.filter(row => row.role === 'sample').map(row => row.ns));
  instantiation.aggregate_ns = aggregate(instantiation.samples.filter(row => row.role === 'sample').map(row => row.ns));
  const compileCountAtReady = compileCount;
  const ready = { type: 'ready', module_sha256: job.module.sha256,
    type_section_sha256: job.module.type_section_sha256, imports, compile_count: compileCountAtReady,
    node: { version: process.versions.node, v8: process.versions.v8, flags: process.execArgv },
    compilation, instantiation, features, memory: memoryObservation('ready-before-runtime-inputs') };
  if (!emit(ready)) await once(process.stdout, 'drain');

  let input = '';
  for await (const chunk of process.stdin) {
    input += chunk.toString('utf8');
    if (Buffer.byteLength(input) > 1024 * 1024) throw Error('runtime input envelope exceeds host bound');
  }
  const request = JSON.parse(input);
  if (request.kind !== 'run' || request.workload_sha256 !== digest(request.projection)) {
    throw Error('invalid post-compilation input envelope');
  }
  const canonical = runtimeCases(request.projection);
  const memory = [before, ready.memory];
  const cases = [];

  function metrics() {
    const result = Object.fromEntries(METRICS.map((name, index) => {
      const raw = exports.metric(index);
      const value = Number(raw);
      if (!Number.isSafeInteger(value) || value < 0 || BigInt(value) !== raw) throw Error(`invalid metric: ${name}`);
      return [name, value];
    }));
    result.program_depth = exports.program_depth();
    result.program_leaves = exports.program_leaves();
    result.allocated_cells = result.allocated_logical_bytes / config.logical_cell_bytes;
    result.released_cells = result.released_logical_bytes / config.logical_cell_bytes;
    if (result.linear_memory_bytes !== exports.memory.buffer.byteLength) throw Error('linear memory metric differs from actual buffer');
    return result;
  }
  function output() {
    const count = Number(exports.metric(12));
    if (!Number.isSafeInteger(count) || count < 0 || count > config.maximum_limits[3] / 16) {
      throw Error('invalid output slot count');
    }
    return Array.from({ length: count }, (_, index) => String(exports.result(index)));
  }
  function recipe() {
    const count = exports.recipe_length();
    if (!Number.isSafeInteger(count) || count < 0 || count > 2048) throw Error('invalid reflected recipe length');
    return Array.from({ length: count }, (_, index) => {
      const kind = exports.recipe_kind(index);
      if (!Number.isInteger(kind) || kind < 1 || kind > 6) throw Error('invalid reflected recipe kind');
      return { kind, value: String(exports.recipe_value(index)) };
    });
  }
  function build(specification) {
    switch (specification.kind) {
      case 'capture': return exports.build_capture(BigInt(specification.value), specification.path);
      case 'tree': return exports.build_tree(['left', 'right', 'balanced'].indexOf(specification.shape), specification.leaves);
      case 'static': return exports.build_static(specification.index);
      default: throw Error(`unknown constructor: ${specification.kind}`);
    }
  }
  function invoke(specification) {
    if (specification.kind === 'invoke') return exports.invoke(BigInt(specification.values[0]), specification.has_input);
    if (specification.kind === 'invoke2') return exports.invoke2(...specification.values.map(BigInt));
    throw Error(`unknown invocation: ${specification.kind}`);
  }
  function observe(scenario, role, index) {
    const observation = { role, index, status: null, quota_reason: null, failure_phase: null,
      output: [], recipe: null, snapshots: {}, cleanup: null,
      timings_ns: { begin: null, construction: null, invocation: null, reflection: null, cleanup: null },
      engine_error: null };
    let activePhase = 'begin';
    function phase(name, call) {
      activePhase = name;
      const start = process.hrtime.bigint();
      try {
        observation.status = call();
        if (observation.status !== 0) observation.failure_phase = name;
      } finally { observation.timings_ns[name] = elapsed(start); }
      observation.snapshots[name] = metrics();
      observation.quota_reason = observation.snapshots[name].quota_reason;
      return observation.status === 0;
    }
    try {
      const limits = { ...config.limits, ...scenario.limits };
      let okay = phase('begin', () => exports.begin(...LIMIT_KEYS.map(key => limits[key])));
      if (okay && scenario.build) okay = phase('construction', () => build(scenario.build));
      if (okay && scenario.invoke) okay = phase('invocation', () => invoke(scenario.invoke));
      if (okay && scenario.direct) {
        const direct = scenario.direct;
        okay = phase('invocation', () => direct.kind === 'list'
          ? exports.run_list(direct.choice, BigInt(direct.input))
          : exports.run_continuation(direct.choice, BigInt(direct.input)));
      }
      if (okay) observation.output = output();
      if (okay && scenario.reflect) {
        okay = phase('reflection', () => exports.reflect());
        if (okay) {
          observation.recipe = recipe();
          observation.post_reflection_output = output();
        } else observation.output = [];
      }
      if (scenario.recovery && observation.status !== 0) {
        // No explicit cleanup between the failed transaction and begin: inspect
        // automatic rollback above, then exercise recovery on the same instance.
        activePhase = 'recovery';
        const recoveryStart = process.hrtime.bigint();
        const recovered = { begin_status: exports.begin(...LIMIT_KEYS.map(key => config.limits[key])) };
        recovered.build_status = exports.build_static(0);
        recovered.invoke_status = exports.invoke(41n, 1);
        recovered.output = recovered.invoke_status === 0 ? output() : [];
        recovered.metrics = metrics();
        observation.recovery = recovered;
        observation.timings_ns.recovery = elapsed(recoveryStart);
      }
    } catch (error) {
      observation.engine_error = { phase: activePhase, name: error.name, message: error.message, stack: error.stack };
    } finally {
      const cleanupStart = process.hrtime.bigint();
      try {
        const returned = exports.cleanup();
        observation.timings_ns.cleanup = elapsed(cleanupStart);
        observation.cleanup = { returned_live_bytes: returned, metrics: metrics() };
      } catch (error) {
        observation.timings_ns.cleanup = elapsed(cleanupStart);
        observation.cleanup_error = `${error.name}: ${error.message}`;
      }
    }
    observation.errors = observationErrors(observation, scenario);
    observation.correctness = observation.errors.length === 0 ? 'passed' : 'failed';
    return observation;
  }
  function runCase(scenario) {
    const row = { id: scenario.id, case_id: scenario.case_id, scenario, observations: [] };
    for (const role of ['warmup', 'sample']) {
      const count = role === 'warmup' ? config.timing.warmups : config.timing.samples;
      for (let index = 0; index < count; index += 1) row.observations.push(observe(scenario, role, index));
    }
    row.correctness = row.observations.every(item => item.correctness === 'passed') ? 'passed' : 'failed';
    row.timing_aggregation = config.timing.aggregation;
    row.aggregate_ns = {};
    for (const phase of ['begin', 'construction', 'invocation', 'reflection', 'cleanup', 'recovery']) {
      row.aggregate_ns[phase] = aggregate(row.observations.filter(item => item.role === 'sample'
        && item.timings_ns[phase] !== null && item.timings_ns[phase] !== undefined).map(item => item.timings_ns[phase]));
    }
    row.performance_claim_eligible = row.correctness === 'passed' && scenario.expected.status === 0;
    row.quota_counts_as_successful_execution = false;
    row.memory_observation = memoryObservation(`after-${scenario.id}`);
    cases.push(row);
    emit({ type: 'case', case: row });
    return row;
  }
  for (const scenario of [...canonical, ...fixedControls(config)]) runCase(scenario);
  const calibration = cases.find(row => row.id === 'CTRL-calibration-tree');
  try {
    for (const scenario of boundaryCases(calibration.observations[0], config)) runCase(scenario);
  } catch (error) {
    for (const id of CONTROL_IDS.filter(id => !cases.some(row => row.id === id))) {
      const row = { id, case_id: 'BOUNDARY', blocked: true, correctness: 'failed', observations: [],
        reason: `calibration-dependent control unavailable: ${error.message}` };
      cases.push(row);
      emit({ type: 'case', case: row });
    }
  }
  memory.push(memoryObservation('after-all-cases-and-accounted-cleanup'));
  const baseline = metrics();
  const measurements = Object.fromEntries(UNKNOWN_METRICS.map(key => [key,
    unknown('No scoped V8 GC/object-layout/engine-only per-case instrumentation is selected; logical accounting is not a substitute.')]));
  measurements.host_memory = memory;
  measurements.resource_usage_max_rss = unknown('Not used: inherited pre-exec parent high-water can contaminate resourceUsage().maxRSS.');
  measurements.logical_charge_bytes_per_cell = { status: 'policy-charge', value: config.logical_cell_bytes,
    meaning: 'Not an observed GC object size. Managed records use this width, separately from scratch, retained pages, and vocabulary.' };
  measurements.permanent_vocabulary = { logical_bytes: baseline.baseline_logical_cell_bytes,
    logical_cells: baseline.baseline_logical_cell_bytes / config.logical_cell_bytes, excluded_from_transient_metrics: true };
  measurements.gc_root_table = { entries: job.representation === 'wasm-gc' ? config.cell_capacity : null,
    storage: job.representation === 'wasm-gc' ? 'per-instance-GC-array' : 'not-used',
    physical_bytes: unknown('No physical root-array/object-layout measurement is available.') };
  measurements.retained_linear_memory = { status: 'observed', value: exports.memory.buffer.byteLength,
    unit: 'bytes', after_logical_cleanup: true, memory_shrink_claim: false };
  const report = { schema_version: 1, representation: job.representation, optimization: job.optimization,
    module_sha256: sha256(bytes), type_section_sha256: job.module.type_section_sha256,
    workload_sha256: request.workload_sha256, imports, node: ready.node,
    isolation: { kind: 'fresh-pinned-node-process-per-configuration', inputs_sent_after_ready: true,
      compile_count_at_ready: compileCountAtReady, compile_count_at_end: compileCount,
      candidate_prepare_requests_after_inputs: 0, new_wasm_types_per_tree: 0,
      guest_requests: 0, protected_operations: 0, import_object: 'empty-null-prototype',
      no_recipe_interpreter_or_host_compiler_service: true },
    features, compilation, instantiation, measurements, cases,
    correctness: cases.every(row => row.correctness === 'passed') ? 'passed' : 'failed',
    reclamation: { logical_region: 'accounted-by-cleanup', gc_collection: 'deferred-and-unmeasured',
      reference_counting: false, immediate_physical_reclamation_claim: false },
    timing_method: config.timing, performance_claim: false };
  emit({ type: 'result', report });
  if (report.correctness !== 'passed') process.exitCode = 1;
}

main().catch(error => {
  emit({ type: 'fatal', error: { name: error.name, message: error.message, stack: error.stack } });
  process.exitCode = 1;
});
