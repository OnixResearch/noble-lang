#!/usr/bin/env node
// Executed M4 acceptance, not a universal semantic or backend refinement proof.
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { CoreEngine } from '../../crates/noble-cli/src/core/runtime/host.mjs';
import { digest } from '../../tools/m3-wasm-policy.mjs';
import { runDocumentationWorkflow } from './documentation.mjs';
import { runPropertyWorkflow, semanticObservation, stackObservation } from './property.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const [binaryArgument, outputArgument] = process.argv.slice(2);
if (!binaryArgument || !outputArgument || process.argv.length !== 4) throw Error('usage: SELECTED_NODE verification/m4/gate.mjs NOBLE_BINARY NEW_ARTIFACT_DIRECTORY');
const binary = fs.realpathSync(binaryArgument);
const artifacts = path.resolve(outputArgument);
fs.mkdirSync(artifacts);
const config = JSON.parse(fs.readFileSync(path.join(root, 'crates/noble-cli/src/core/runtime/config.json')));
const abi = JSON.parse(fs.readFileSync(path.join(root, 'crates/noble-cli/src/core/runtime/abi.json')));
const conformanceFile = 'specs/conformance/cases.json';
const conformanceBytes = fs.readFileSync(path.join(root, conformanceFile));
const canonical = JSON.parse(conformanceBytes);
const cases = new Map(canonical.cases.map(item => [item.id, item]));
assert.equal(cases.size, canonical.cases.length, 'duplicate canonical case ID');
const workloadProjection = rows => rows.map(({ id, input, expected }) => ({ id, input, expected }))
  .sort((left, right) => left.id < right.id ? -1 : left.id > right.id ? 1 : 0);
const workload = workloadProjection(canonical.cases);
const workflowFile = 'specs/conformance/language-workflow-cases.json';
const workflowBytes = fs.readFileSync(path.join(root, workflowFile));
const workflowCases = JSON.parse(workflowBytes);
const workflowIds = ['DX-10', 'DX-12'];
const workflowRows = rows => rows.filter(row => workflowIds.includes(row.id));
const workflows = new Map(workflowRows(workflowCases.cases).map(row => [row.id, row]));
assert.equal(workflows.size, workflowIds.length, 'missing developer-workflow fixture');
assert.equal(workflowRows(workflowCases.cases).length, workflows.size, 'duplicate developer-workflow fixture');
const workflowWorkload = workloadProjection([...workflows.values()]);
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const save = (name, data) => fs.writeFileSync(path.join(artifacts, name), typeof data === 'string' || Buffer.isBuffer(data) ? data : JSON.stringify(data, null, 2) + '\n', { flag: 'wx' });
save('conformance-input.json', conformanceBytes);
save('workflow-input.json', workflowBytes);
const receipt = { schema: 'noble-m4-acceptance/v1', result: 'running', profile: 'Core-Bootstrap', backend: 'managed-linear-memory',
  binary: { path: binary, sha256: hash(fs.readFileSync(binary)) }, node: { path: process.execPath, versions: process.versions, flags: process.execArgv },
  conformance: { path: conformanceFile, raw_snapshot: 'conformance-input.json', byte_sha256_at_read: hash(conformanceBytes),
    workload_sha256: digest(workload), projection: workload,
    identity_for_workload: 'complete-id-input-expected-projection; state/evidence-not-workload' },
  workflow_design: { path: workflowFile, raw_snapshot: 'workflow-input.json', byte_sha256_at_read: hash(workflowBytes),
    workload_sha256: digest(workflowWorkload), projection: workflowWorkload,
    identity_for_workload: 'complete-DX-10-DX-12-id-input-expected-projection; state/evidence-not-workload' },
  cases: [], controls: [], workflows: [], commands: [], sources: {}, assumptions: [
    'The Rust compiler, selected assembler/optimizer/Node/V8, loader and host shell are trusted execution tooling.',
    'Host observations concern declared resource-free test bindings, not protected external operations.',
    'These bounded execution cases do not prove universal inference, kernel, compiler or backend correctness.',
  ], non_claims: ['MC2', 'live resources', 'components', 'recursion', 'module imports', 'universal refinement'] };

function sources(directory) {
  for (const entry of fs.readdirSync(path.join(root, directory), { withFileTypes: true })) {
    if (['target', '.lake', '.git', 'node_modules'].includes(entry.name)) continue;
    const file = path.posix.join(directory, entry.name);
    if (entry.isSymbolicLink()) throw Error(`unbound source symlink: ${file}`);
    if (entry.isDirectory()) sources(file);
    else if (entry.isFile()) receipt.sources[file] = hash(fs.readFileSync(path.join(root, file)));
  }
}
// Bind runtime sources and the executed harness, not its later published
// receipts/archives. The extraction harness has its own complete source audit.
for (const directory of ['crates', 'tools']) sources(directory);
for (const file of ['Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', 'flake.lock', 'policy/tool-selection.json',
  'verification/m4/gate.mjs', 'verification/m4/documentation.mjs', 'verification/m4/property.mjs']) {
  receipt.sources[file] = hash(fs.readFileSync(path.join(root, file)));
}
const limits = Object.fromEntries(['allocation_bytes', 'recipe_leaves', 'program_depth',
  'operand_bytes', 'continuation_bytes', 'steps'].map(key => [key, abi.limits_maximum[key]]));
const workflowConfiguration = {
  specification_revision: workflowCases.revision,
  compiler: receipt.binary,
  build_context: {
    source_revision: `sha256:${hash(Buffer.from(JSON.stringify(Object.entries(receipt.sources).sort())))}`,
    rust_toolchain_sha256: receipt.sources['rust-toolchain.toml'],
    tool_selection_sha256: receipt.sources['policy/tool-selection.json'],
    environment: Object.fromEntries(['PATH', 'RUSTC', 'RUSTDOC', 'RUSTC_WRAPPER',
      'RUSTC_WORKSPACE_WRAPPER', 'RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'CARGO_TARGET_DIR']
      .map(key => [key, process.env[key] ?? null])),
    source_correspondence: 'source snapshot and binary identity; compilation provenance is retained separately',
  },
  node: receipt.node,
  backend: receipt.backend,
  engine_selection: config,
  abi,
  limits,
  test_host: {
    ambient_credentials: false, guest_request_budget: 4096, limits,
    mappings: {
      'test.emit': { resource_free: true, module: 'noble', import: 'test_emit', effect_id: 0,
        contract: 'Text -- ! {test.emit}' },
      'test.abort': { resource_free: true, module: 'noble', import: 'test_abort', effect_id: 1,
        contract: '-- ! {test.abort}' },
    },
  },
};

let serial = 0;
function command(label, executable, args, input, { timeout = 180000, allowTimeout = false } = {}) {
  const result = spawnSync(executable, args, { cwd: root, input, encoding: 'utf8', timeout,
    killSignal: 'SIGKILL', maxBuffer: 16 * 1024 * 1024 });
  const log = `${++serial}-${label}`;
  save(`${log}.stdout`, result.stdout ?? ''); save(`${log}.stderr`, result.stderr ?? '');
  const record = { label, executable, args, timeout_ms: timeout, status: result.status, signal: result.signal,
    error: result.error ? String(result.error) : null, stdout: `${log}.stdout`, stderr: `${log}.stderr` };
  receipt.commands.push(record);
  if (!allowTimeout || result.error?.code !== 'ETIMEDOUT') {
    assert.equal(result.error, undefined, `${label}: ${result.error}`);
    assert.equal(result.signal, null, `${label}: ${result.signal}`);
  }
  return { ...result, record };
}
function report(result) {
  const lines = result.stdout.trim().split('\n').filter(Boolean);
  return lines.map(line => JSON.parse(line));
}
function runSource(label, source, optimization = 'off') {
  const file = path.join(artifacts, `${++serial}-${label}.noble`);
  fs.writeFileSync(file, source, { flag: 'wx' });
  const result = command(label, binary, ['run', file, '--opt', optimization, '--emit', path.join(artifacts, `${serial}-${label}-artifacts`)]);
  const reports = report(result);
  assert.equal(reports.length, 1);
  return { ...reports[0], process_exit: result.status, command: result.record };
}
function timedRun(label, source, timeout) {
  assert.ok(Number.isSafeInteger(timeout) && timeout > 0);
  const file = path.join(artifacts, `${++serial}-${label}.noble`);
  fs.writeFileSync(file, source, { flag: 'wx' });
  const result = command(label, binary, ['run', file, '--opt', 'off', '--emit',
    path.join(artifacts, `${serial}-${label}-artifacts`)], undefined, { timeout, allowTimeout: true });
  const timedOut = result.error?.code === 'ETIMEDOUT';
  return { timed_out: timedOut, process_exit: result.status, signal: result.signal,
    error: result.error ? String(result.error) : null, stdout: result.stdout ?? '', stderr: result.stderr ?? '',
    reports: timedOut ? [] : report(result), command: result.record,
    observation_scope: 'deadline covers complete run command; timeout does not establish guest entry' };
}
function malformed() {
  const result = command('DX-10-malformed-lane', 'cargo', ['test', '-p', 'noble-kernel', '--test', 'property',
    'malformed_candidates_never_panic_or_accept', '--', '--exact', '--nocapture']);
  assert.equal(result.status, 0, result.stdout + result.stderr);
  const counts = /property\/malformed: (\d+) corrupted candidates, (\d+) accepted, (\d+) panics; outcomes: invalid=(\d+) exhausted=(\d+) unsupported=(\d+) internal-failure=(\d+);/.exec(result.stdout);
  assert.ok(counts, 'malformed lane did not report its observed outcomes');
  const observations = Object.fromEntries(['trials', 'accepted', 'panics', 'invalid', 'exhausted',
    'unsupported', 'internal_failure'].map((key, index) => [key, Number(counts[index + 1])]));
  assert.equal(observations.trials, 200);
  assert.equal(observations.accepted, 0); assert.equal(observations.panics, 0);
  assert.equal(observations.invalid + observations.exhausted + observations.unsupported
    + observations.internal_failure, observations.trials);
  return { command: result.record, process_exit: result.status, seed: '0x0B1E_5EED_0000_0002',
    bounds: { max_trials: 200 }, observations,
    sources: Object.fromEntries(Object.entries(receipt.sources).filter(([file]) =>
      file === 'crates/noble-kernel/tests/property.rs' || file.startsWith('crates/noble-kernel/tests/property/'))),
    scope: 'separate structurally malformed candidate nonacceptance/panic lane; not well-typed Wasm execution coverage' };
}
function runSession(label, submissions, optimization = 'off') {
  const input = Buffer.concat(submissions.map(source => {
    const bytes = Buffer.isBuffer(source) ? source : Buffer.from(source);
    return Buffer.concat([Buffer.from(`${bytes.length}\n`), bytes]);
  }));
  const result = command(label, binary, ['session', '--framed', '--opt', optimization, '--emit', path.join(artifacts, `${++serial}-${label}-artifacts`)], input);
  return { reports: report(result), process_exit: result.status, command: result.record };
}
function quiet(value) {
  assert.equal(value.guest_requests, 0); assert.equal(value.protected_operations, 0);
}
function normal(value, expected) {
  assert.equal(value.stage, 'wasm'); assert.equal(value.outcome, 'normal');
  assert.deepEqual(stackObservation(value.stack), expected); assert.equal(value.candidate_prepare_requests, 0);
}
function scalar(value) { return { type: 'I64', value: String(value) }; }
function addCase(id, observations) {
  receipt.cases.push({ id, input: cases.get(id).input, expected: cases.get(id).expected, result: 'passed', observations });
}
function control(id, run) {
  const observations = run(); receipt.controls.push({ id, result: 'passed', observations });
}

function compile(label, source, inputTypes, optimization) {
  const file = path.join(artifacts, `${++serial}-${label}.noble`);
  fs.writeFileSync(file, source, { flag: 'wx' });
  const args = ['compile', file, ...inputTypes.flatMap(type => ['--input-type', type])];
  const output = command(label, binary, args);
  assert.equal(output.status, 0, output.stdout + output.stderr);
  const engine = new CoreEngine(config, abi, { optimized: optimization === 'on', artifacts: fs.mkdtempSync(path.join(artifacts, `${label}-`)) });
  try {
    engine.prepare(Buffer.from(output.stdout), Buffer.from(source));
    return { engine, module: engine.pending.module, metadata: engine.pending.record };
  } catch (error) {
    engine.close();
    throw error;
  }
}
function dynamicRuns(label, source, type, inputs, optimization) {
  const compiled = compile(label, source, [type], optimization);
  const observations = [];
  for (let index = 0; index < inputs.length; index += 1) {
    const engine = index === 0 ? compiled.engine : new CoreEngine(config, abi, { optimized: optimization === 'on', artifacts: fs.mkdtempSync(path.join(artifacts, `${label}-variant-`)) });
    if (index !== 0) engine.install(compiled.module, { ...compiled.metadata, stem: null });
    const before = engine.compilations;
    const observed = engine.execute({ inputs: [{ type, value: inputs[index] }] });
    assert.equal(engine.compilations, before, 'runtime input triggered compilation');
    observations.push({ optimization, input_after_compile: inputs[index], compiler_service: 'absent', ...observed });
    engine.close();
  }
  return observations;
}

try {
  for (const id of ['CORE-01', 'CORE-02', 'CORE-06', 'CORE-11', 'CORE-12', 'CORE-14']) {
    const item = cases.get(id), observations = [];
    for (const optimization of ['off', 'on']) {
      const observed = runSource(id, item.input.source, optimization);
      normal(observed, item.expected.stack);
      if (item.expected.guest_requests === 0) quiet(observed);
      observations.push({ optimization, ...observed });
    }
    addCase(id, observations);
  }
  for (const id of ['CORE-07', 'CORE-10']) {
    const observed = runSource(id, cases.get(id).input.source);
    assert.equal(observed.stage, 'check'); assert.equal(observed.outcome, cases.get(id).expected.outcome); quiet(observed);
    addCase(id, [observed]);
  }
  {
    const observations = [];
    for (const optimization of ['off', 'on']) {
      const observed = runSession('CORE-04', cases.get('CORE-04').input.submissions, optimization);
      assert.equal(observed.reports[0].outcome, 'defined'); quiet(observed.reports[0]);
      normal(observed.reports[1], cases.get('CORE-04').expected.stack);
      observations.push({ optimization, ...observed });
    }
    addCase('CORE-04', observations);
  }
  {
    const observed = runSession('CORE-08', ['7', 'def keep [ dup ]', cases.get('CORE-08').input.source, 'keep']);
    assert.equal(observed.reports.length, 4); const rejected = observed.reports[2];
    assert.equal(rejected.stage, 'check'); assert.equal(rejected.outcome, 'type-reject'); quiet(rejected);
    assert.equal(rejected.prior_session, 'unchanged');
    assert.equal(rejected.prior_stack, 'unchanged'); assert.equal(rejected.prior_namespace, 'unchanged');
    normal(observed.reports[3], [scalar(7), scalar(7)]); addCase('CORE-08', [observed]);
  }
  {
    const observations = cases.get('CORE-13').input.sources.map(source => runSource('CORE-13', source));
    for (const observed of observations) { assert.equal(observed.stage, 'resolve'); assert.equal(observed.outcome, 'unbound-word'); quiet(observed); }
    addCase('CORE-13', observations);
  }
  {
    const input = cases.get('CORE-18').input;
    const observations = [...input.sources, ...input.byte_sources_hex.map(hex => Buffer.from(hex, 'hex'))].map(source => runSource('CORE-18', source));
    for (const observed of observations) { assert.equal(observed.stage, 'parse'); assert.equal(observed.outcome, 'reject'); quiet(observed); }
    addCase('CORE-18', observations);
  }
  {
    const observations = [];
    for (const optimization of ['off', 'on']) {
      const observed = runSource('CORE-17', cases.get('CORE-17').input.source, optimization);
      assert.equal(observed.stage, 'wasm'); assert.equal(observed.outcome, 'trap');
      assert.deepEqual(observed.request_trace, cases.get('CORE-17').expected.request_trace);
      assert.equal(observed.guest_requests, 2); assert.equal(observed.protected_operations, 0);
      observations.push({ optimization, second_program_started: false, ...observed });
    }
    addCase('CORE-17', observations);
  }
  {
    const item = cases.get('CORE-03'), observations = [];
    for (const optimization of ['off', 'on']) {
      const runs = dynamicRuns('CORE-03', `${item.input.invocation_input} swap ${item.input.builder} run`, 'I64', item.input.captures_after_compile, optimization);
      runs.forEach((run, at) => normal(run, [scalar(item.expected.outputs[at]) ])); observations.push(...runs);
    }
    addCase('CORE-03', observations);
  }
  {
    const item = cases.get('CORE-05'), observations = [];
    const source = '[ 20 nil ] dip [ [ 1 + ] ] [ [ 2 * ] ] if swap cons [ ] [ drop run ] list.case';
    for (const optimization of ['off', 'on']) {
      const runs = dynamicRuns('CORE-05', source, 'Bool', [true, false], optimization);
      runs.forEach((run, at) => normal(run, [scalar(item.expected.outputs[at])])); observations.push(...runs);
    }
    addCase('CORE-05', observations);
  }
  {
    const item = cases.get('CORE-09'), observations = [];
    const source = `${item.input.invocation_input} swap ${item.input.builder} dup [ run ] dip dup reflect pair`;
    for (const optimization of item.input.optimizations) {
      const [observed] = dynamicRuns('CORE-09', source, 'I64', [item.input.capture_after_compile], optimization);
      assert.equal(observed.outcome, 'normal');
      assert.equal(observed.stack[0].handle, null);
      assert.deepEqual(semanticObservation(observed.stack[0]), scalar(item.expected.output));
      const [program, syntax] = observed.stack[1].value;
      assert.equal(program.type, 'Program'); assert.equal(syntax.type, 'Syntax');
      assert.deepEqual(program.recipe, item.expected.recipe); assert.deepEqual(syntax.recipe, item.expected.recipe);
      observations.push(observed);
    }
    assert.deepEqual(stackObservation(observations[0].stack), stackObservation(observations[1].stack));
    addCase('CORE-09', observations);
  }
  {
    const result = command('structured-core-controls', 'cargo', ['test', '-p', 'noble-cli', '--test', 'core', '--locked', '--offline', '--', '--nocapture']);
    assert.equal(result.status, 0, result.stdout + result.stderr);
    const observations = result.stdout.split('\n').filter(line => line.startsWith('{"id":"CORE-')).map(line => JSON.parse(line));
    for (const id of ['CORE-15', 'CORE-16']) {
      const observed = observations.find(row => row.id === id); assert.ok(observed); quiet(observed);
      assert.equal(observed.stage, cases.get(id).expected.stage); assert.equal(observed.outcome, cases.get(id).expected.outcome);
      addCase(id, [{ ...observed, command: result.record }]);
    }
  }
  control('complete-vocabulary', () => {
    const rows = [
      ['9 4 - 3 *', [scalar(15)]], ['3 3 =', [{ type: 'Bool', value: true }]],
      ['4 true [ 1 + ] dip', [scalar(5), { type: 'Bool', value: true }]],
      ['1 true pair unpair swap drop', [{ type: 'Bool', value: true }]],
      ['5 inl [ 1 + ] [ drop 0 ] case', [scalar(6)]], ['5 inr [ drop 0 ] [ 2 * ] case', [scalar(10)]],
      ['nil [ 7 ] [ drop drop 0 ] list.case', [scalar(7)]],
      ['false [ 1 ] [ 2 ] if', [scalar(2)]], ['unit quote run', [{ type: 'Unit', value: null }]],
      ['"α\\u{1f600}" quote run', [{ type: 'Text', value: 'α😀' }]],
      ['1 true pair quote run unpair', [scalar(1), { type: 'Bool', value: true }]],
      ['1 nil cons quote run [ 0 ] [ drop ] list.case', [scalar(1)]],
      ['[ 1 ] quote run run', [scalar(1)]], ['[ 1 ] reflect quote run', [{ type: 'Syntax', recipe: [{ literal: scalar(1) }], witnesses: [] }]],
    ];
    return rows.flatMap(([source, stack]) => ['off', 'on'].map(optimization => {
      const observed = runSource('vocabulary', source, optimization); normal(observed, stack); return { source, optimization, ...observed };
    }));
  });
  control('immutable-definitions-and-persistent-programs', () => {
    const observed = runSession('persistence', ['def n [ 1 ]', '[ n ]', 'def n [ 2 ]', 'run n']);
    normal(observed.reports[3], [scalar(1), scalar(2)]); return observed;
  });
  control('persistent-runtime-capture', () => {
    const observed = runSession('capture-session', ['2 quote', 'run']); normal(observed.reports[1], [scalar(2)]); return observed;
  });
  control('exact-quotation-normalization-and-identity', () => {
    const observations = [];
    for (const optimization of ['off', 'on']) {
      const normalization = runSource('quote-normalization', '[[ 1 ]] reflect [ 1 ] quote reflect', optimization);
      assert.equal(normalization.outcome, 'normal');
      assert.deepEqual(normalization.stack[0].recipe, normalization.stack[1].recipe);
      const identity = runSession('definition-identity', [
        'def a [ 1 ]', 'def b [ 1 # same resolved body\n ]', '[ a ] reflect [ b ] reflect',
      ], optimization);
      const values = identity.reports[2].stack;
      assert.equal(identity.reports[2].outcome, 'normal');
      assert.deepEqual(values[0].recipe, values[1].recipe);
      assert.match(values[0].recipe[0].invoke, /^definition:/);
      observations.push({ optimization, normalization, identity });
    }
    assert.deepEqual(stackObservation(observations[0].normalization.stack),
      stackObservation(observations[1].normalization.stack));
    return observations;
  });
  control('fresh-rank-one-uses', () => {
    const observed = runSession('rank-one', ['def copy [ dup ]', '1 copy true copy']);
    normal(observed.reports[1], [scalar(1), scalar(1), { type: 'Bool', value: true }, { type: 'Bool', value: true }]); return observed;
  });
  control('named-legacy-spellings-are-not-aliases', () => {
    const observed = runSession('legacy-name', [
      'def call [ run ]', '41 [ 1 + ] call', 'def reify [ reflect ]',
      '[ 2 ] reify drop', 'def // [ 1 + ]', '//',
    ]);
    normal(observed.reports[1], [scalar(42)]);
    normal(observed.reports[5], [scalar(43)]);
    return observed;
  });
  control('first-class-program-is-monomorphic', () => {
    const observed = runSource('monomorphic', '[ dup ] dup 1 swap run true swap run');
    assert.equal(observed.outcome, 'type-reject'); quiet(observed); return observed;
  });
  control('complete-ordered-program-interfaces', () => {
    const observed = runSession('ordered-interface', [
      '[ [ 1 + ] [ 2 + ] if ]',
      'false swap 41 swap true swap run',
      'true swap 41 swap run',
      '41 swap true swap run',
    ]);
    for (const at of [1, 2]) {
      assert.equal(observed.reports[at].outcome, 'type-reject');
      quiet(observed.reports[at]);
    }
    normal(observed.reports[3], [scalar(42)]);
    return observed;
  });
  control('latent-effects-remain-conservative', () => {
    const observed = runSource('latent-effects', 'true [ 1 ] [ "audit" test.emit 1 ] if');
    normal(observed, [scalar(1)]);
    quiet(observed);
    return observed;
  });
  control('source-byte-limit-boundary', () => {
    const file = path.join(artifacts, 'byte-boundary.noble'); fs.writeFileSync(file, '1');
    const exact = command('byte-exact', binary, ['run', file, '--source-bytes', '1']); normal(report(exact)[0], [scalar(1)]);
    const over = command('byte-exhausted', binary, ['run', file, '--source-bytes', '0']); assert.equal(report(over)[0].outcome, 'exhausted');
    return [exact.record, over.record];
  });
  control('runtime-limit-boundaries', () => {
    const compiled = compile('runtime-bounds', '40 2 quote [ + ] compose run', [], 'off');
    const baseline = compiled.engine.execute();
    normal(baseline, [scalar(42)]);
    compiled.engine.close();
    const observations = [];
    for (const key of ['allocation_bytes', 'recipe_leaves', 'program_depth', 'operand_bytes', 'continuation_bytes', 'steps']) {
      const trial = value => {
        const engine = new CoreEngine(config, abi, { artifacts: fs.mkdtempSync(path.join(artifacts, `limit-${key}-`)) });
        engine.install(compiled.module, { ...compiled.metadata, stem: null });
        const observed = engine.execute({ limits: { [key]: value } });
        engine.close();
        return observed;
      };
      let low = 0, high = abi.limits_maximum[key];
      while (low < high) {
        const middle = Math.floor((low + high) / 2);
        const observed = trial(middle);
        if (observed.outcome === 'normal') high = middle;
        else {
          assert.equal(observed.outcome, 'runtime-exhausted', `${key} must fail explicitly`);
          low = middle + 1;
        }
      }
      assert.ok(low > 0, `${key} workload must actually consume the resource`);
      const exact = trial(low), exhausted = trial(low - 1);
      normal(exact, [scalar(42)]);
      assert.equal(exhausted.outcome, 'runtime-exhausted');
      observations.push({ limit: key, exact_value: low, exact, exhausted_value: low - 1, exhausted });
    }
    return observations;
  });
  for (const [id, run] of [['DX-10', runPropertyWorkflow], ['DX-12', runDocumentationWorkflow]]) {
    const observation = run({ fixture: workflows.get(id), configuration: workflowConfiguration,
      save, runSource, compile, malformed, timedRun, compilerAvailable: true });
    assert.equal(observation.result, 'passed', `${id}: developer workflow did not pass`);
    receipt.workflows.push({ id, result: observation.result, observations: observation });
  }
  const ids = receipt.cases.map(row => row.id).sort(); assert.deepEqual(ids, [...cases.keys()].sort());
  for (const [file, expected] of Object.entries(receipt.sources)) {
    assert.equal(hash(fs.readFileSync(path.join(root, file))), expected, `source changed during acceptance: ${file}`);
  }
  assert.equal(hash(fs.readFileSync(binary)), receipt.binary.sha256, 'binary changed during acceptance');
  assert.equal(digest(workloadProjection(JSON.parse(fs.readFileSync(path.join(root, conformanceFile))).cases)),
    receipt.conformance.workload_sha256, 'canonical workload changed during acceptance');
  assert.equal(digest(workloadProjection(workflowRows(JSON.parse(fs.readFileSync(path.join(root, workflowFile))).cases))),
    receipt.workflow_design.workload_sha256, 'developer workflow changed during acceptance');
  receipt.result = 'passed';
} catch (error) {
  receipt.result = 'failed'; receipt.failure = `${error.stack ?? error}`; process.exitCode = 1;
} finally {
  receipt.source_digest = hash(Buffer.from(JSON.stringify(Object.entries(receipt.sources).sort())));
  save('acceptance.json', receipt);
  console.log(JSON.stringify({ result: receipt.result, cases: receipt.cases.length, controls: receipt.controls.length,
    workflows: receipt.workflows.map(({ id, result }) => ({ id, result })),
    receipt: path.join(artifacts, 'acceptance.json'), failure: receipt.failure ?? null }));
}
