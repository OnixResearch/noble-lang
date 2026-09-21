// The driver, workers, and ADAPT-12 controls share this report/eligibility policy.
// This is experimental host evidence, not a compiler-derived source inventory or proof.
import { createHash } from 'node:crypto';

export const REPRESENTATIONS = ['wasm-gc', 'managed-linear-memory'];
export const OPTIMIZATIONS = ['off', 'on'];
export const CANONICAL_IDS = ['CORE-03', 'CORE-05', 'CORE-09', 'ADAPT-08', 'ADAPT-09', 'ADAPT-10', 'ADAPT-12'];
export const METRICS = ['status', 'quota_reason', 'allocated_logical_bytes', 'released_logical_bytes',
  'live_transient_logical_bytes', 'peak_transient_logical_bytes', 'peak_operand_bytes',
  'peak_continuation_bytes', 'dispatch_steps', 'runtime_quote_entry_invocations',
  'linear_memory_bytes', 'baseline_logical_cell_bytes', 'operand_count', 'reflection_work_steps'];
export const LIMIT_KEYS = ['allocation_bytes', 'recipe_program_leaves', 'composition_depth',
  'operand_bytes', 'continuation_bytes', 'dispatch_steps'];
export const CONTROL_IDS = ['CTRL-calibration-tree', 'CTRL-two-slots', 'CTRL-arity-too-few',
  'CTRL-arity-too-many', 'CTRL-arity-empty', 'CTRL-arity-quote', 'CTRL-continuation-0',
  'CTRL-continuation-1', 'CTRL-allocation-zero-static', 'CTRL-invalid-limit-negative',
  'CTRL-invalid-limit-maximum', 'CTRL-allocation-below', 'CTRL-allocation-at',
  'CTRL-allocation-partial', 'CTRL-depth-below', 'CTRL-depth-at', 'CTRL-leaves-below',
  'CTRL-leaves-at', 'CTRL-continuation-below', 'CTRL-continuation-at', 'CTRL-operands-below',
  'CTRL-operands-at', 'CTRL-dispatch-below', 'CTRL-dispatch-at', 'CTRL-reflection-below',
  'CTRL-reflection-at', 'CTRL-failure-recovery'];
export const UNKNOWN_METRICS = ['engine_heap_peak_bytes', 'per_case_engine_heap_bytes',
  'compiled_native_code_bytes', 'gc_physical_cell_bytes', 'gc_root_array_physical_bytes',
  'gc_collection_time_ns', 'gc_reclaimed_bytes'];

export function stable(value) {
  if (Array.isArray(value)) return `[${value.map(stable).join(',')}]`;
  if (value && typeof value === 'object') return `{${Object.keys(value).sort()
    .map(key => `${JSON.stringify(key)}:${stable(value[key])}`).join(',')}}`;
  return JSON.stringify(value);
}
export const sha256 = bytes => createHash('sha256').update(bytes).digest('hex');
export const digest = value => sha256(stable(value));
export const equal = (left, right) => stable(left) === stable(right);
export const unknown = reason => ({ status: 'unknown', value: null, reason });
const hash = value => typeof value === 'string' && /^[0-9a-f]{64}$/.test(value);
const finite = value => typeof value === 'number' && Number.isFinite(value) && value >= 0;
const integer = value => Number.isSafeInteger(value) && value >= 0;
function requireCondition(condition, message) { if (!condition) throw Error(message); }

export function validateConfig(config) {
  requireCondition(config.schema_version === 1, 'unsupported configuration schema');
  requireCondition(equal(config.representations, REPRESENTATIONS)
    && equal(config.optimizations, OPTIMIZATIONS), 'the complete four-configuration matrix is mandatory');
  requireCondition(config.timing?.samples >= 7 && config.timing?.warmups >= 3
    && integer(config.timing.samples) && integer(config.timing.warmups), 'timing needs >=7 samples and >=3 warmups');
  requireCondition(config.timing.aggregation === 'median-min-max-nearest-rank-p95'
    && config.timing.clock === 'process.hrtime.bigint', 'unreviewed timing method');
  requireCondition(equal(LIMIT_KEYS.map(key => config.limits?.[key]), [65536, 256, 256, 2048, 4096, 100000])
    && equal(config.maximum_limits, [196608, 256, 256, 2048, 4096, 100000]), 'runtime quota protocol mismatch');
  requireCondition(config.logical_cell_bytes === 48 && config.cell_capacity === 4096, 'storage protocol mismatch');
  requireCondition(equal(config.node_flags, ['--no-liftoff', '--no-wasm-lazy-compilation', '--no-wasm-tier-up',
    '--stack-size=1024', '--max-old-space-size=256']), 'unreviewed engine flags');
  requireCondition(equal(config.optimizer_flags, ['--enable-gc', '--enable-reference-types', '--enable-bulk-memory',
    '-O2', '--strip-debug']), 'unreviewed optimizer flags');
  for (const name of ['node', 'wasm_tools', 'wasm_opt']) {
    requireCondition(config.tools?.[name]?.path?.startsWith('/nix/store/') && config.tools[name].version,
      `missing immutable tool pin: ${name}`);
  }
  requireCondition(config.tools.node.v8 && config.nixpkgs?.revision && config.nixpkgs?.nar_hash,
    'missing engine or nixpkgs revision');
  requireCondition(config.runtime_assets?.length === 11 && new Set(config.runtime_assets).size === 11
    && config.runtime_assets.every(file => /^crates\/noble-wasm\/runtime\/[a-z-]+\.wat$/.test(file)),
  'runtime assets must be declared explicitly');
  requireCondition(config.proof_status === 'open', 'the harness cannot claim a backend proof');
  return config;
}

export function canonicalProjection(records) {
  return CANONICAL_IDS.map(id => {
    const matches = records.filter(record => record.id === id);
    requireCondition(matches.length === 1, `canonical case missing or duplicated: ${id}`);
    const { input, expected } = matches[0];
    return { id, input, expected };
  });
}
const literal = value => ({ kind: 1, value: String(value) });
const invocation = value => ({ kind: 2, value: String(value) });
export function treeOracle(leaves, input = '20') {
  let value = BigInt(input);
  const recipe = [];
  for (let index = 0; index < leaves; index += 1) {
    const add = index % 2 === 0;
    value = BigInt.asIntN(64, add ? value + 1n : value * 2n);
    recipe.push(literal(add ? 1 : 2), invocation(add ? 4 : 6));
  }
  return { output: [String(value)], recipe };
}
const normal = (output, recipe = null) => ({ status: 0, quota_reason: 0, failure_phase: null, output, recipe });
const failure = (status, reason, phase) => ({ status, quota_reason: reason, failure_phase: phase, output: [], recipe: null });
const callOne = (value, present = 1) => ({ kind: 'invoke', values: [String(value)], has_input: present });
const treeBuild = (leaves = 16, shape = 'left') => ({ kind: 'tree', shape, leaves });
function descriptor(id, caseId, build, invoke, expected, extra = {}) {
  return { id, case_id: caseId, build, invoke, reflect: expected.recipe !== null, expected, ...extra };
}

export function runtimeCases(projection) {
  const byId = Object.fromEntries(projection.map(record => [record.id, record]));
  const capture = byId['CORE-03'];
  requireCondition(equal(capture.input.captures_after_compile, ['2', '-1', '9223372036854775807'])
    && capture.input.invocation_input === '40' && capture.input.compiler_service === 'disabled'
    && capture.input.builder === 'quote [ + ] compose', 'unsupported CORE-03 workload');
  const cases = capture.input.captures_after_compile.map((value, index) => {
    const result = String(BigInt.asIntN(64, BigInt(value) + BigInt(capture.input.invocation_input)));
    requireCondition(capture.expected.outputs[index] === result, 'CORE-03 contradicts independent wrapping oracle');
    return descriptor(`CORE-03/capture-${index}`, 'CORE-03', { kind: 'capture', value, path: 0 },
      callOne(capture.input.invocation_input), normal([result], [literal(value), invocation(4)]));
  });
  const list = byId['CORE-05'];
  requireCondition(list.input.program_selection === 'after-compilation' && list.input.compiler_service === 'disabled'
    && equal(list.input.program_options, ['[ 1 + ]', '[ 2 * ]'])
    && list.input.source_template === '20 nil [ 1 + ] swap cons [ ] [ drop run ] list.case'
    && equal(list.expected.outputs, ['21', '40']), 'unsupported CORE-05 workload');
  for (let choice = 0; choice < 2; choice += 1) {
    cases.push(descriptor(`CORE-05/choice-${choice}`, 'CORE-05', null, null,
      normal([list.expected.outputs[choice]], [literal(choice === 0 ? 1 : 2), invocation(choice === 0 ? 4 : 6)]),
      { direct: { kind: 'list', choice, input: '20' } }));
  }
  const reflect = byId['CORE-09'];
  requireCondition(reflect.input.capture_after_compile === '2' && reflect.input.invocation_input === '40'
    && equal(reflect.input.optimizations, OPTIMIZATIONS) && reflect.input.also_return_program_in_pair === true
    && reflect.input.compiler_service === 'disabled'
    && reflect.expected.output === '42' && equal(reflect.expected.recipe,
      [{ literal: { type: 'I64', value: '2' } }, { invoke: 'builtin:i64.add' }]), 'unsupported CORE-09 workload');
  for (const path of [0, 4]) cases.push(descriptor(`CORE-09/path-${path}`, 'CORE-09',
    { kind: 'capture', value: reflect.input.capture_after_compile, path }, callOne(reflect.input.invocation_input),
    normal(['42'], [literal('2'), invocation(4)])));
  const trees = byId['ADAPT-09'];
  const matrix = ['left', 'right', 'balanced'].flatMap(shape => [255, 256, 257].map(leaf_count => ({ shape,
    leaf_count, outcome: leaf_count > 256 ? 'construction-quota' : 'normal' })));
  requireCondition(equal(trees.input.variants, matrix) && trees.input.initial.type === 'I64'
    && trees.input.initial.value === '20' && trees.input.operands === 'post-compilation-inputs'
    && trees.input.compiler_service === 'disabled' && trees.input.limits.recipe_leaves === 256
    && trees.input.limits.composition_depth === 256 && trees.expected.quota_is_success === false,
  'unsupported ADAPT-09 workload');
  for (const variant of trees.input.variants) {
    const oracle = treeOracle(variant.leaf_count, trees.input.initial.value);
    cases.push(descriptor(`ADAPT-09/${variant.shape}-${variant.leaf_count}`, 'ADAPT-09',
      treeBuild(variant.leaf_count, variant.shape), callOne(trees.input.initial.value),
      variant.outcome === 'normal' ? normal(oracle.output, oracle.recipe) : failure(1, 2, 'construction')));
  }
  const reachability = byId['ADAPT-10'];
  requireCondition(equal(reachability.input.variants.map(variant => variant.path),
    ['collection-retrieval', 'returned-program', 'reflection-only'])
    && reachability.input.debug_metadata === 'stripped' && reachability.input.compiler_service === 'disabled',
  'unsupported ADAPT-10 workload');
  for (const [index, variant] of reachability.input.variants.entries()) {
    for (const [captureIndex, value] of capture.input.captures_after_compile.entries()) {
      const reflectionOnly = index === 2;
      cases.push(descriptor(`ADAPT-10/${variant.path}-${captureIndex}`, 'ADAPT-10',
        { kind: 'capture', value, path: [2, 3, 1][index] }, reflectionOnly ? null : callOne('0', 0),
        normal(reflectionOnly ? [] : [value], [literal(value)]), { reflection_only: reflectionOnly }));
    }
  }
  return cases;
}

export function fixedControls(config) {
  const oracle = treeOracle(16);
  const result = [descriptor('CTRL-calibration-tree', 'BOUNDARY', treeBuild(), callOne('20'),
    normal(oracle.output, oracle.recipe))];
  result.push(descriptor('CTRL-two-slots', 'BOUNDARY', { kind: 'static', index: 9 },
    { kind: 'invoke2', values: ['10', '20'] }, normal(['11', '20'])));
  for (const [name, index, invoke] of [
    ['too-few', 2, callOne('10')], ['too-many', 0, { kind: 'invoke2', values: ['10', '20'] }],
    ['empty', 0, callOne('0', 0)],
  ]) result.push(descriptor(`CTRL-arity-${name}`, 'BOUNDARY', { kind: 'static', index }, invoke,
    failure(3, 0, 'invocation')));
  result.push(descriptor('CTRL-arity-quote', 'BOUNDARY', { kind: 'capture', value: '2', path: 1 },
    callOne('10'), failure(3, 0, 'invocation')));
  for (let choice = 0; choice < 2; choice += 1) result.push(descriptor(`CTRL-continuation-${choice}`, 'BOUNDARY',
    null, null, normal([choice === 0 ? '22' : '41']), { direct: { kind: 'continuation', choice, input: '20' } }));
  result.push(descriptor('CTRL-allocation-zero-static', 'BOUNDARY', { kind: 'static', index: 0 },
    callOne('41'), normal(['42']), { limits: { allocation_bytes: 0 } }));
  for (const [suffix, value] of [['negative', -1], ['maximum', config.maximum_limits[0] + 1]]) {
    result.push(descriptor(`CTRL-invalid-limit-${suffix}`, 'BOUNDARY', null, null, failure(4, 0, 'begin'),
      { limits: { allocation_bytes: value } }));
  }
  return result;
}

export function boundaryCases(calibration, config) {
  const construction = calibration?.snapshots?.construction;
  const invocationMetrics = calibration?.snapshots?.invocation;
  const reflectionMetrics = calibration?.snapshots?.reflection;
  requireCondition(calibration?.correctness === 'passed' && construction && invocationMetrics && reflectionMetrics,
    'boundary calibration did not execute correctly');
  const oracle = treeOracle(16);
  const boundaries = [
    ['allocation', 'allocation_bytes', construction.peak_transient_logical_bytes, 1, 1, 'construction'],
    ['depth', 'composition_depth', construction.program_depth, 1, 3, 'construction'],
    ['leaves', 'recipe_program_leaves', construction.program_leaves, 1, 2, 'construction'],
    ['continuation', 'continuation_bytes', invocationMetrics.peak_continuation_bytes, 2, 5, 'invocation'],
    ['operands', 'operand_bytes', invocationMetrics.peak_operand_bytes, 2, 4, 'invocation'],
    ['dispatch', 'dispatch_steps', invocationMetrics.dispatch_steps, 2, 6, 'invocation'],
    ['reflection', 'dispatch_steps', reflectionMetrics.reflection_work_steps, 2, 7, 'reflection'],
  ];
  const cases = [];
  for (const [name, key, required, status, reason, phase] of boundaries) {
    requireCondition(integer(required) && required > 0 && required <= config.maximum_limits[LIMIT_KEYS.indexOf(key)],
      `invalid measured boundary: ${name}`);
    for (const at of [false, true]) {
      const reflection = phase === 'reflection';
      cases.push(descriptor(`CTRL-${name}-${at ? 'at' : 'below'}`, 'BOUNDARY', treeBuild(),
        reflection ? null : callOne('20'), at ? normal(reflection ? [] : oracle.output,
          reflection ? oracle.recipe : null) : failure(status, reason, phase),
        { reflect: reflection, limits: { [key]: required - (at ? 0 : 1) },
          calibrated_boundary: { key, required, source: 'CTRL-calibration-tree' },
          require_partial_allocation: !at && name === 'allocation' }));
    }
  }
  for (const recovery of [false, true]) cases.push(descriptor(recovery ? 'CTRL-failure-recovery' : 'CTRL-allocation-partial',
    'BOUNDARY', treeBuild(), callOne('20'), failure(1, 1, 'construction'),
    { limits: { allocation_bytes: 48 }, require_partial_allocation: true, recovery }));
  return cases;
}

export function observationErrors(observation, scenario) {
  const errors = [];
  const check = (condition, message) => { if (!condition) errors.push(message); };
  const expected = scenario.expected;
  check(!observation.engine_error, 'engine trap/exception is not a quota or normal outcome');
  check(observation.status === expected.status, 'runtime status differs from expected outcome');
  check(observation.quota_reason === expected.quota_reason, 'quota reason differs');
  check(observation.failure_phase === expected.failure_phase, 'failure phase differs');
  check(equal(observation.output, expected.output), 'complete ordered output differs from oracle');
  if (expected.recipe !== null) {
    check(equal(observation.recipe, expected.recipe), 'full ordered recipe differs from oracle');
    check(equal(observation.post_reflection_output, expected.output), 'reflection changed complete ordered output');
  }
  else check(observation.recipe === null, 'unexpected recipe observation');
  const cleanup = observation.cleanup;
  check(cleanup?.returned_live_bytes === 0 && cleanup?.metrics?.live_transient_logical_bytes === 0,
    'cleanup retains transient logical cells');
  check(cleanup?.metrics?.allocated_logical_bytes === cleanup?.metrics?.released_logical_bytes,
    'cleanup allocation/release accounting differs');
  check(cleanup?.metrics?.operand_count === 0 && cleanup?.metrics?.program_depth === 0
    && cleanup?.metrics?.program_leaves === 0, 'cleanup retains output or program roots');
  const snapshots = observation.snapshots ?? {};
  const terminal = snapshots[observation.failure_phase ?? (scenario.reflect ? 'reflection'
    : scenario.invoke || scenario.direct ? 'invocation' : scenario.build ? 'construction' : 'begin')];
  check(terminal?.status === observation.status && terminal?.quota_reason === observation.quota_reason,
    'exported status and runtime metric disagree');
  for (const [phase, metrics] of Object.entries({ ...snapshots, cleanup: cleanup?.metrics })) {
    check(metrics && METRICS.every(key => integer(metrics[key])), `${phase}: missing/nonintegral runtime metric`);
    if (!metrics) continue;
    check(metrics.allocated_logical_bytes % 48 === 0 && metrics.released_logical_bytes % 48 === 0,
      `${phase}: non-cell-aligned allocation account`);
    check(metrics.allocated_logical_bytes - metrics.released_logical_bytes === metrics.live_transient_logical_bytes,
      `${phase}: live allocation account differs`);
    check(metrics.peak_transient_logical_bytes >= metrics.live_transient_logical_bytes,
      `${phase}: impossible allocation peak`);
  }
  if (expected.status !== 0) {
    const failed = snapshots[expected.failure_phase];
    check(failed?.live_transient_logical_bytes === 0 && failed?.operand_count === 0 && failed?.program_depth === 0,
      'failure did not automatically roll back before explicit cleanup');
    if (scenario.require_partial_allocation) check(failed?.allocated_logical_bytes > 0
      && failed?.allocated_logical_bytes === failed?.released_logical_bytes, 'partial allocation rollback was not exercised');
  }
  if (scenario.reflection_only) check(snapshots.reflection?.runtime_quote_entry_invocations === 0
    && snapshots.reflection?.operand_count === 0 && observation.timings_ns.invocation === null,
  'reflection-only path executed or retained an operand');
  if (scenario.recovery) {
    check(observation.recovery?.begin_status === 0 && observation.recovery?.build_status === 0
      && observation.recovery?.invoke_status === 0 && equal(observation.recovery?.output, ['42']),
    'begin after failed transaction did not recover');
  }
  return errors;
}

export function aggregate(values) {
  const ordered = [...values].sort((left, right) => left - right);
  if (!ordered.length) return null;
  const middle = Math.floor(ordered.length / 2);
  return { count: ordered.length, min: ordered[0], max: ordered.at(-1),
    median: ordered.length % 2 ? ordered[middle] : (ordered[middle - 1] + ordered[middle]) / 2,
    p95: ordered[Math.ceil(ordered.length * 0.95) - 1] };
}

function validateTiming(samples, config, errors, label) {
  if (!Array.isArray(samples)) { errors.push(`${label}: missing raw samples`); return; }
  for (const role of ['warmup', 'sample']) {
    const expected = role === 'warmup' ? config.timing.warmups : config.timing.samples;
    const rows = samples.filter(sample => sample.role === role);
    if (rows.length !== expected || rows.some((sample, index) => sample.index !== index)) {
      errors.push(`${label}: incomplete ${role} measurements`);
    }
  }
  if (samples.some(sample => !finite(sample.ns))) errors.push(`${label}: invalid elapsed time`);
}

// Report validity and execution correctness are deliberately different. An honestly
// retained failed execution is a valid report, but cannot make a candidate eligible.
export function evaluateReport(report, { requirePolicyControls = true } = {}) {
  const errors = [];
  const executionFailures = [];
  const check = (condition, message) => { if (!condition) errors.push(message); };
  let config;
  let runtime;
  try {
    config = validateConfig(report.configuration);
    runtime = runtimeCases(report.workload.projection);
  } catch (error) {
    return { valid: false, outcome: 'reject-report', eligible_for_selection: false,
      eligible_representations: [], m3_execution_gates: 'not-run', errors: [String(error)], execution_failures: [] };
  }
  check(report.schema_version === 1 && report.selected_representation === null, 'invalid schema or premature representation selection');
  check(report.integrity?.unchanged === true, 'source/configuration changed or was not rebound after execution');
  check(report.workload.sha256 === digest(report.workload.projection), 'canonical workload binding differs');
  check(equal(report.workload.projection.map(row => row.id), CANONICAL_IDS), 'canonical workload matrix differs');
  check(hash(report.source?.sha256) && report.source.sha256 === digest(report.source.files), 'source byte manifest differs');
  check(report.source?.classification === 'byte-manifest-not-compiler-inventory'
    && report.source?.compiler_inventory_claim === false, 'host source manifest is not a compiler inventory');
  check(report.configuration_sha256 === digest(config), 'configuration semantic binding differs');
  check(report.source?.files?.['crates/noble-cli/src/core/runtime/config.json']?.sha256 === report.configuration_bytes_sha256,
    'configuration byte binding differs');
  for (const file of config.runtime_assets) {
    const asset = report.source?.runtime_assets?.find(row => row.path === file);
    check(asset?.classification === 'owned-runtime-source-outside-rust-extraction'
      && asset?.proof_status === 'open' && asset?.sha256 === report.source?.files?.[file]?.sha256
      && hash(asset?.sha256), `runtime source asset not explicitly bound: ${file}`);
  }
  for (const name of ['node', 'wasm_tools', 'wasm_opt']) {
    const tool = report.tools?.[name];
    check(tool?.pin_matched === true && tool.version === config.tools[name].version && hash(tool.sha256)
      && tool.pinned_path === config.tools[name].path && tool.realpath?.startsWith('/nix/store/'),
    `missing/mismatched tool pin: ${name}`);
  }
  check(report.tools?.node?.v8 === config.tools.node.v8 && hash(report.tools?.noble?.sha256), 'missing V8/CLI byte identity');
  check(report.tools?.nixpkgs?.revision === config.nixpkgs.revision
    && report.tools?.nixpkgs?.nar_hash === config.nixpkgs.nar_hash && hash(report.tools?.nixpkgs?.lock_sha256),
  'missing immutable nixpkgs binding');
  check(report.trust_boundaries?.length >= 5 && report.proof_status === 'open', 'missing trust boundaries or false proof claim');
  const owner = report.adapt08;
  const expectedOwner = report.workload.projection.find(row => row.id === 'ADAPT-08')?.expected;
  const ownerPassed = owner?.correctness === 'passed' && equal(owner.observed, expectedOwner)
    && hash(owner.stdout_sha256) && owner.exit === 0 && equal(owner.representations, REPRESENTATIONS);
  if (!ownerPassed) executionFailures.push('ADAPT-08 backend-interface rejection did not pass');
  const expectedIds = REPRESENTATIONS.flatMap(representation => OPTIMIZATIONS.map(mode => `${representation}/${mode}`));
  const configurations = report.configurations ?? [];
  check(equal(configurations.map(row => row.id).sort(), [...expectedIds].sort()), 'missing or duplicate comparison configuration');
  const passed = new Map();
  for (const row of configurations) {
    const label = row.id;
    check(label === `${row.representation}/${row.optimization}` && REPRESENTATIONS.includes(row.representation)
      && OPTIMIZATIONS.includes(row.optimization), `${label}: invalid candidate identity`);
    if (row.outcome === 'blocked') {
      check(typeof row.blocker?.reason === 'string' && row.blocker.reason.length > 0
        && row.blocker.evidence?.length > 0 && row.blocker.evidence.every(item => hash(item.sha256) && item.path),
      `${label}: unsupported candidate needs retained blocker evidence`);
      check(row.worker === null && !row.performance_rank && row.measured_success === false,
        `${label}: blocker presented as measured success`);
      check(row.measurements?.status === 'unknown' && row.measurements.value === null && row.measurements.reason,
        `${label}: unavailable candidate measurements must remain explicitly unknown`);
      passed.set(label, false);
      continue;
    }
    if (row.outcome !== 'executed') { errors.push(`${label}: unaccounted/unsupported outcome`); passed.set(label, false); continue; }
    let good = true;
    const fail = message => { executionFailures.push(`${label}: ${message}`); good = false; };
    const module = row.module;
    check(hash(module?.sha256) && integer(module?.bytes) && module.bytes > 8 && hash(module?.type_section_sha256),
      `${label}: missing module identity`);
    const accounting = module?.sections;
    check(accounting?.header_bytes === 8 && Array.isArray(accounting?.entries)
      && accounting.entries.every(section => integer(section.payload_bytes) && integer(section.framing_bytes)
        && section.total_bytes === section.payload_bytes + section.framing_bytes && hash(section.payload_sha256))
      && accounting.entries.reduce((sum, section) => sum + section.total_bytes, 8) === module?.bytes,
    `${label}: emitted section accounting does not sum to module bytes`);
    if (row.optimization === 'on') check(module?.debug_metadata_stripped === true
      && !accounting?.entries.some(section => section.id === 0 && (section.custom_name === 'name'
        || section.custom_name?.startsWith('.debug'))), `${label}: optimized reachability artifact retains debug metadata`);
    for (const phase of ['cli', 'assembly', ...(row.optimization === 'on' ? ['optimization'] : [])]) {
      validateTiming(row.preparation?.[phase], config, errors, `${label}/${phase}`);
    }
    const worker = row.worker;
    if (!worker) { fail('worker result missing'); passed.set(label, false); continue; }
    check(worker.node?.version === config.tools.node.version && worker.node?.v8 === config.tools.node.v8
      && equal(worker.node?.flags, config.node_flags), `${label}: worker engine mismatch`);
    check(worker.module_sha256 === module?.sha256 && worker.type_section_sha256 === module?.type_section_sha256,
      `${label}: worker executed different module bytes`);
    check(equal(worker.imports, []) && worker.isolation?.inputs_sent_after_ready === true
      && worker.isolation?.compile_count_at_ready === worker.isolation?.compile_count_at_end
      && worker.isolation?.compile_count_at_ready > 0 && worker.isolation?.candidate_prepare_requests_after_inputs === 0
      && worker.isolation?.new_wasm_types_per_tree === 0 && worker.isolation?.guest_requests === 0
      && worker.isolation?.protected_operations === 0, `${label}: host/compilation boundary violated`);
    for (const name of ['gc', 'function_references', 'multi_value', 'tail_call', 'component']) {
      check(typeof worker.features?.[name]?.supported === 'boolean' && hash(worker.features?.[name]?.module_sha256),
        `${label}: missing separate ${name} feature probe`);
    }
    if (row.representation === 'wasm-gc' && worker.features?.gc?.supported !== true) fail('required GC feature unavailable');
    for (const metric of UNKNOWN_METRICS) {
      check(worker.measurements?.[metric]?.status === 'unknown' && worker.measurements[metric].value === null
        && worker.measurements[metric].reason, `${label}: unsupported ${metric} instrumentation must be explicitly unknown`);
    }
    validateTiming(worker.compilation?.samples, config, errors, `${label}/compilation`);
    validateTiming(worker.instantiation?.samples, config, errors, `${label}/instantiation`);
    check(finite(worker.compilation?.first_ns) && finite(worker.instantiation?.first_ns), `${label}: missing first compilation/instantiation`);
    const actualCases = worker.cases ?? [];
    const expectedScenarios = [...runtime, ...fixedControls(config)];
    try {
      const calibration = actualCases.find(item => item.id === 'CTRL-calibration-tree')?.observations?.[0];
      expectedScenarios.push(...boundaryCases(calibration, config));
    } catch (error) { fail(String(error)); }
    const requiredIds = [...runtime.map(item => item.id), ...CONTROL_IDS].sort();
    check(equal(actualCases.map(item => item.id).sort(), requiredIds), `${label}: acceptance/negative-control cases omitted or duplicated`);
    for (const scenario of expectedScenarios) {
      const actual = actualCases.find(item => item.id === scenario.id);
      if (!actual || actual.blocked) { fail(`case not executed: ${scenario.id}`); continue; }
      check(equal(actual.scenario, scenario), `${label}/${scenario.id}: input/expected oracle was changed`);
      const observations = actual.observations;
      if (!Array.isArray(observations)) { errors.push(`${label}/${scenario.id}: missing trials`); fail('missing trials'); continue; }
      for (const role of ['warmup', 'sample']) {
        const trials = observations.filter(item => item.role === role);
        check(trials.length === (role === 'warmup' ? config.timing.warmups : config.timing.samples)
          && trials.every((item, index) => item.index === index), `${label}/${scenario.id}: incomplete ${role} trials`);
      }
      let caseGood = true;
      for (const observed of observations) {
        const reasons = observationErrors(observed, scenario);
        const correct = reasons.length === 0 && !observed.engine_error;
        check(observed.correctness === (correct ? 'passed' : 'failed'), `${label}/${scenario.id}: dishonest correctness label`);
        check(observed.timings_ns && Object.values(observed.timings_ns).every(value => value === null || finite(value)),
          `${label}/${scenario.id}: invalid raw timing`);
        if (!correct) { caseGood = false; fail(`${scenario.id}/${observed.role}-${observed.index}: ${reasons.join('; ')}`); }
      }
      check(actual.correctness === (caseGood ? 'passed' : 'failed'), `${label}/${scenario.id}: case correctness label differs`);
    }
    if (worker.fatal_error) fail(worker.fatal_error);
    check(row.measured_success === good, `${label}: candidate measured-success label differs from retained executions`);
    check(worker.correctness === (good ? 'passed' : 'failed'), `${label}: worker correctness summary differs`);
    passed.set(label, good);
  }
  const eligible = REPRESENTATIONS.filter(representation => OPTIMIZATIONS.every(mode => passed.get(`${representation}/${mode}`)))
    .filter(() => ownerPassed);
  if (requirePolicyControls && (report.adapt12?.correctness !== 'passed'
    || report.adapt12?.controls?.length !== 5 || report.adapt12.controls.some(control => control.actual !== control.expected))) {
    executionFailures.push('ADAPT-12 substantive selection-policy controls did not pass');
    eligible.length = 0;
  }
  const valid = errors.length === 0;
  return { valid, outcome: !valid ? 'reject-report' : eligible.length ? 'eligible-for-selection' : 'cannot-close-M3',
    eligible_for_selection: valid && eligible.length > 0, eligible_representations: valid ? eligible : [],
    m3_execution_gates: eligible.length ? 'passed' : configurations.some(row => row.outcome === 'executed') ? 'failed' : 'not-run',
    errors, execution_failures: executionFailures };
}

export function runPolicyControls(report, retainFixture = null) {
  const canonical = report.workload.projection.find(item => item.id === 'ADAPT-12');
  const variants = canonical?.input?.variants;
  const expectedVariants = [
    { working_candidates: 1, m3_execution_gates: 'passed', other_candidate: 'evidenced-blocker', metrics: 'scoped-or-explicitly-unknown', pins: 'recorded', outcome: 'eligible-for-selection' },
    { working_candidates: 0, m3_execution_gates: 'not-run', other_candidate: 'evidenced-blocker', metrics: 'unknown', pins: 'recorded', outcome: 'cannot-close-M3' },
    { working_candidates: 1, m3_execution_gates: 'passed', other_candidate: 'unsupported-recorded-as-fastest', metrics: 'fabricated-zero', pins: 'recorded', outcome: 'reject-report' },
    { working_candidates: 1, m3_execution_gates: 'passed', other_candidate: 'evidenced-blocker', metrics: 'present', pins: 'missing', outcome: 'reject-report' },
    { working_candidates: 1, m3_execution_gates: 'failed', other_candidate: 'evidenced-blocker', metrics: 'fast', pins: 'recorded', outcome: 'cannot-close-M3' },
  ];
  requireCondition(equal(variants, expectedVariants) && canonical.input.synthetic_records_only === true,
    'unsupported ADAPT-12 policy-control workload');
  const initial = evaluateReport(report, { requirePolicyControls: false });
  if (!initial.valid || !initial.eligible_for_selection) return { correctness: 'failed', controls: [],
    reason: 'A structurally valid, genuinely executed passing candidate is required to seed synthetic policy controls.', initial };
  const working = initial.eligible_representations[0];
  const other = REPRESENTATIONS.find(name => name !== working);
  const syntheticBlocker = row => ({ id: row.id, representation: row.representation, optimization: row.optimization,
    outcome: 'blocked', measured_success: false, worker: null,
    measurements: unknown('Synthetic unavailable candidate; no executed timing or memory result.'),
    blocker: { reason: 'Synthetic unsupported-engine control, not an observed candidate failure.',
      evidence: [{ path: 'synthetic-control:not-actual-evidence', sha256: digest({ control: 'ADAPT-12', blocked: row.id }) }] } });
  const baseline = structuredClone(report);
  baseline.configurations = baseline.configurations.map(row => row.representation === other ? syntheticBlocker(row) : row);
  const transformations = [
    { name: 'one-passing-candidate-and-evidenced-blocker', apply() {} },
    { name: 'no-executed-working-candidate', apply(fixture) { fixture.configurations = fixture.configurations.map(syntheticBlocker); } },
    { name: 'unsupported-as-fastest-with-fabricated-memory-zero', apply(fixture) {
      const blocked = fixture.configurations.find(row => row.representation === other);
      blocked.outcome = 'unsupported'; blocked.performance_rank = 1; blocked.measured_success = true;
      const running = fixture.configurations.find(row => row.representation === working);
      running.worker.measurements.engine_heap_peak_bytes = { status: 'measured', value: 0, reason: 'synthetic fabricated zero' };
    } },
    { name: 'missing-immutable-pin', apply(fixture) { delete fixture.tools.node; } },
    { name: 'retained-fast-but-incorrect-executions', apply(fixture) {
      for (const row of fixture.configurations.filter(item => item.representation === working)) {
        const sampleCase = row.worker.cases.find(item => item.id === 'CORE-03/capture-0');
        const sample = sampleCase.observations.find(item => item.role === 'sample');
        sample.output = ['43']; sample.correctness = 'failed';
        sample.errors = observationErrors(sample, sampleCase.scenario);
        sampleCase.correctness = 'failed'; row.worker.correctness = 'failed'; row.measured_success = false;
      }
    } },
  ];
  const controls = transformations.map((transformation, index) => {
    const fixture = structuredClone(baseline);
    transformation.apply(fixture);
    fixture.synthetic_control = { id: `ADAPT-12/${index}`, not_actual_candidate_evidence: true };
    const decision = evaluateReport(fixture, { requirePolicyControls: false });
    return { id: `ADAPT-12/${index}`, synthetic: true, transformation: transformation.name,
      canonical_variant: variants[index], fixture_sha256: digest(fixture), expected: variants[index].outcome,
      fixture_artifact: retainFixture ? retainFixture(index, fixture) : null, actual: decision.outcome, decision };
  });
  return { correctness: controls.every(control => control.actual === control.expected) ? 'passed' : 'failed',
    synthetic_only: true, backend_selected_by_fixture: false, same_validator_as_actual_results: true,
    seed_report_sha256: digest(report), controls };
}
