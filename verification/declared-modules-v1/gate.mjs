#!/usr/bin/env node
// Runs only production Noble CLI/worker and the independently built kernel peer.
// No cached receipt or planned fixture result is accepted as execution evidence.
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { preparePlan } from './plan.mjs';

const root = fs.realpathSync(path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..'));
assert.equal(process.argv.length, 5,
  'usage: SELECTED_NODE verification/declared-modules-v1/gate.mjs NOBLE_BINARY KERNEL_PEER NEW_EXTERNAL_DIRECTORY');
const target = path.resolve(process.argv[4]);
assert.ok(target !== root && !target.startsWith(`${root}${path.sep}`), 'output outside repository required');
assert.ok(!fs.existsSync(target), 'fresh output directory required');
assert.equal(fs.realpathSync(path.dirname(target)), path.dirname(target), 'output parent cannot be a symlink');
fs.mkdirSync(target);
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const hash = file => sha(fs.readFileSync(file));
const relative = file => path.relative(target, file).split(path.sep).join('/');
const receipt = { schema: 'noble-declared-modules-acceptance/v1', profile: 'Declared-Modules-v1',
  result: 'running', source_root: root, output_directory: target, plan: null, tools: {},
  retained: {}, commands: [], cases: [], failures: [], integrity_failures: [],
  assumptions: [
    'The independently built Noble/kernel binaries, source compiler, selected Node/V8 and wasm-tools are trusted execution mechanisms; hashes bind bytes, not semantic correctness.',
    'The host-owned adapter manifest and host authorization policy are supplied independently from guest module source; manifest text itself grants no guest authority.',
    'This finite local fixture does not establish universal source-to-Wasm refinement, live resource execution or portable module identity.',
  ] };
let serial = 0;
const watched = [];
let plan;
function retain(name, bytes, mode = 0o400) {
  const file = path.join(target, name);
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, bytes, { flag: 'wx', mode });
  receipt.retained[name] = { sha256: hash(file), bytes: fs.statSync(file).size };
  watched.push({ file, sha256: receipt.retained[name].sha256 });
  return file;
}
function freeze(name, sourceFile) {
  const original = fs.realpathSync(path.resolve(sourceFile));
  assert.ok(fs.statSync(original).isFile(), `${name}: expected selected binary`);
  const frozen = retain(`executables/${name}`, fs.readFileSync(original), 0o500);
  watched.push({ file: original, sha256: hash(original) });
  receipt.tools[name] = { original, frozen: relative(frozen), sha256: hash(frozen) };
  return frozen;
}
function asset(kind, name) {
  const field = kind === 'source' ? 'program_bytes' : 'bindings';
  const entry = plan[field][name];
  assert.ok(entry, `unreviewed ${kind} fixture: ${name}`);
  const file = path.join(target, 'plan', entry.file);
  assert.equal(hash(file), entry.sha256, `${name}: modified frozen ${kind} bytes`);
  return file;
}
function command(label, executable, args, input = Buffer.alloc(0), timeout = 180_000) {
  const id = ++serial;
  const stem = `commands/${String(id).padStart(4, '0')}-${label.replace(/[^A-Za-z0-9_.-]/g, '-')}`;
  const stdin = retain(`${stem}.stdin`, input);
  const result = spawnSync(executable, args, { cwd: root, input, encoding: null,
    timeout, killSignal: 'SIGKILL', maxBuffer: 32 * 1024 * 1024,
    env: { PATH: '/nonexistent', LANG: 'C', LC_ALL: 'C', RUST_BACKTRACE: '1' } });
  const stdout = retain(`${stem}.stdout`, result.stdout ?? Buffer.alloc(0));
  const stderr = retain(`${stem}.stderr`, result.stderr ?? Buffer.alloc(0));
  const entry = { id, label, executable: relative(executable), executable_sha256: hash(executable),
    argv: args, cwd: root, stdin: relative(stdin), stdout: relative(stdout), stderr: relative(stderr),
    timeout_ms: timeout, status: result.status, signal: result.signal,
    error: result.error ? String(result.error) : null };
  receipt.commands.push(entry);
  assert.equal(entry.error, null, `${label}: launch failure`);
  assert.equal(entry.signal, null, `${label}: killed or timed out`);
  assert.notEqual(entry.status, null, `${label}: missing process status`);
  return { ...entry, bytes: result.stdout ?? Buffer.alloc(0) };
}
function reports(run, number) {
  const lines = run.bytes.toString('utf8').split('\n').filter(line => line.length);
  assert.equal(lines.length, number, `${run.label}: report count differs from framed source units`);
  return lines.map((line, index) => {
    const parsed = JSON.parse(line);
    assert.equal(parsed.schema, 'noble-core-report/v1', `${run.label}/${index}: wrong production report`);
    assert.equal(parsed.profile, 'Declared-Modules-v1', `${run.label}/${index}: wrong profile`);
    return parsed;
  });
}
function framed(names) {
  return Buffer.concat(names.flatMap(name => {
    const bytes = fs.readFileSync(asset('source', name));
    return [Buffer.from(`${bytes.length}\n`), bytes];
  }));
}
function artifacts(label, directory, submissions) {
  const entries = [];
  const walk = folder => {
    for (const item of fs.readdirSync(folder, { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name))) {
      const file = path.join(folder, item.name);
      if (item.isDirectory()) walk(file);
      else {
        assert.ok(item.isFile(), `${label}: artifact symlink or special file`);
        entries.push({ file: relative(file), sha256: hash(file), bytes: fs.statSync(file).size });
        watched.push({ file, sha256: hash(file) });
      }
    }
  };
  assert.ok(fs.existsSync(directory), `${label}: missing CLI --emit output directory`);
  walk(directory);
  for (let i = 0; i < submissions.length; i++) {
    const file = path.join(directory, `${i + 1}.noble`);
    assert.ok(fs.existsSync(file), `${label}: missing retained source frame ${i + 1}`);
    assert.deepEqual(fs.readFileSync(file), fs.readFileSync(asset('source', submissions[i])),
      `${label}: CLI did not execute exact frozen source bytes`);
  }
  return entries;
}
function session(label, cli, binding, submissions, optimization) {
  const directory = path.join(target, 'emission', label);
  fs.mkdirSync(path.dirname(directory), { recursive: true });
  const args = ['session', '--framed', '--declared-modules', '--bindings', asset('binding', binding),
    '--opt', optimization, '--emit', directory];
  const run = command(label, cli, args, framed(submissions));
  const parsed = reports(run, submissions.length);
  const emitted = artifacts(label, directory, submissions);
  return { command: run.id, status: run.status, reports: parsed, artifacts: emitted };
}
function quiet(report, label) {
  assert.equal(report.guest_requests, 0, `${label}: guest executed during declaration/link`);
  assert.equal(report.host_requests, 0, `${label}: host was consulted during declaration/link`);
  assert.equal(report.protected_operations, 0, `${label}: protected operation during declaration/link`);
  assert.equal(report.candidate_prepare_requests, 0, `${label}: candidate executed at link`);
  assert.equal(report.acquired_authority, false, `${label}: admission granted authority`);
  assert.equal(report.ambient_fallback_calls, 0, `${label}: ambient fallback used`);
  if (report.stage === 'link') {
    assert.ok(['linked', 'reject'].includes(report.outcome), `${label}: not a real link decision`);
  }
}
function linked(report, version, identity, label) {
  assert.equal(report.stage, 'link', `${label}: not a link report`);
  assert.equal(report.outcome, 'linked', `${label}: import was not published`);
  assert.equal(report.resolved_module?.name, 'ledger', `${label}: wrong resolved module`);
  assert.equal(report.resolved_module?.version, version, `${label}: wrong source interface version`);
  assert.match(report.resolved_module.identity, /^[0-9]+$/,
    `${label}: module identity must be an exact decimal string, not lossy JSON number`);
  if (identity !== null) {
    assert.equal(report.binding?.adapter_identity, identity, `${label}: wrong retained binding identity`);
    assert.ok(Number.isSafeInteger(report.binding.slot) && report.binding.slot >= 0,
      `${label}: no validated immutable binding slot`);
  }
  return report;
}
function wasmArtifacts(label, emitted, wasmTools, bound = false) {
  const modules = emitted.filter(item => item.file.endsWith('.wasm'));
  assert.ok(modules.length > 0, `${label}: no production compiled Wasm retained`);
  const inspected = [];
  for (const item of modules) {
    const file = path.join(target, item.file);
    const run = command(`${label}-validate-${inspected.length}`, wasmTools, ['validate', file]);
    assert.equal(run.status, 0, `${label}: selected wasm-tools rejected retained Wasm`);
    const printed = command(`${label}-print-${inspected.length}`, wasmTools, ['print', file]);
    assert.equal(printed.status, 0, `${label}: selected wasm-tools could not inspect compiled Wasm`);
    if (bound) assert.match(printed.bytes.toString('utf8'), /\(import "noble" "test_emit_bound"/,
      `${label}: actual compiled Wasm lacks bound host import`);
    inspected.push({ file: item.file, sha256: item.sha256,
      validation_command: run.id, inspection_command: printed.id });
  }
  return inspected;
}
function compile(label, cli, wasmTools, moduleNames, program, inputTypes = []) {
  const moduleFiles = moduleNames.map(name => asset('source', name));
  const body = program === 'incomplete_match_module' ? 'unused_expression' : program;
  const source = retain(`inputs/static/${label}.noble`, fs.readFileSync(asset('source', body)));
  const args = ['compile', source, '--declared-modules', '--bindings', asset('binding', 'empty'),
    ...moduleFiles.flatMap(file => ['--module', file]),
    ...inputTypes.flatMap(type => ['--input-type', type])];
  const run = command(label, cli, args);
  if (run.status !== 0) {
    const [report] = reports(run, 1);
    const privateExternal = label.includes('private-constructor-from-outside-module')
      || label.includes('private-arm-match-from-outside-module');
    if (privateExternal && report.stage === 'resolve')
      assert.equal(report.outcome, 'unbound-word', `${label}: early private lookup did not fail closed`);
    else {
      assert.ok(['check', 'acceptance'].includes(report.stage), `${label}: not a static checker refusal`);
      assert.ok(['reject', 'type-reject'].includes(report.outcome), `${label}: not a rejection`);
    }
    assert.equal(report.guest_requests, 0);
    assert.equal(report.protected_operations, 0);
    return { command: run.id, status: run.status, report, source: relative(source) };
  }
  assert.match(run.bytes.toString('utf8'), /^\(module\b/, `${label}: compile returned no real WAT`);
  const wat = retain(`inputs/static/${label}.wat`, run.bytes);
  const wasm = path.join(target, `inputs/static/${label}.wasm`);
  const assembled = command(`${label}-assemble`, wasmTools, ['parse', '-o', wasm, wat]);
  assert.equal(assembled.status, 0, `${label}: selected wasm-tools parse failed`);
  watched.push({ file: wasm, sha256: hash(wasm) });
  receipt.retained[relative(wasm)] = { sha256: hash(wasm), bytes: fs.statSync(wasm).size };
  const valid = command(`${label}-validate`, wasmTools, ['validate', wasm]);
  assert.equal(valid.status, 0, `${label}: selected wasm-tools validate failed`);
  return { command: run.id, status: run.status, source: relative(source),
    wat: relative(wat), wasm: relative(wasm), assembly_command: assembled.id,
    validation_command: valid.id };
}
function staticCase(scenario, cli, wasmTools) {
  const check = scenario.steps.find(step => step.action === 'kernel-check-only');
  assert.ok(check, `missing static checker input: ${scenario.variant}`);
  const modules = scenario.variant === 'variant-elimination-missing-constructor'
    ? ['incomplete_match_module'] : ['schema'];
  const name = `DX-03-${scenario.variant}-${check.resource_operation ?? 'type'}`;
  const result = compile(name, cli, wasmTools, modules, check.source, check.input_stack ?? []);
  const expected = plan.canonical['specs/conformance/developer-experience-cases.json']
    .complete_selected_workload[0].input.variants.find(item => item.case === scenario.variant);
  assert.ok(expected, `${name}: omitted canonical variant`);
  if (expected.outcome === 'accept') assert.equal(result.status, 0, `${name}: expected accepted checked WAT`);
  else {
    assert.notEqual(result.status, 0, `${name}: unexpectedly accepted`);
    const diagnostic = result.report.diagnostic;
    assert.ok(typeof diagnostic === 'string' && diagnostic.length > 0, `${name}: no typed diagnostic`);
    if (check.resource_operation) {
      assert.doesNotMatch(diagnostic,
        /resource input is not a declared top-level component owner|unknown resource|unsupported schema|backend unsupported/i,
        `${name}: resource control failed before eligibility`);
    }
  }
  return { case_id: 'DX-03', variant: scenario.variant,
    resource_operation: check.resource_operation ?? null, expected: expected.outcome, observed: result };
}
function linkCase(scenario, cli, optimization) {
  const moduleSource = scenario.steps.find(step => step.action === 'register-source').source;
  const binding = scenario.steps.find(step => step.action === 'bind-host-adapter').binding;
  const submissions = [moduleSource, 'import_ledger_A'];
  const name = `DX-08-${scenario.variant}-${optimization}`;
  const result = session(name, cli, binding, submissions, optimization);
  assert.ok([0, 1, 2].includes(result.status), `${name}: abnormal CLI process status`);
  for (const [index, report] of result.reports.entries()) quiet(report, `${name}/${index}`);
  const expected = plan.canonical['specs/conformance/language-workflow-cases.json']
    .complete_selected_workload.find(row => row.id === 'DX-08').input.variants
    .find(row => row.binding === scenario.variant);
  assert.ok(expected, `${name}: omitted canonical binding variant`);
  if (expected.outcome === 'link-without-execution') {
    linked(result.reports[1], 1, 'version-A', name);
    assert.equal(result.status, 0);
  } else {
    const rejection = result.reports.find(report => report.outcome === 'reject');
    assert.ok(rejection,
      `${name}: incompatible binding/initializer was not refused`);
    assert.notEqual(result.reports.at(-1).outcome, 'linked', `${name}: bad import published`);
    const reason = rejection.diagnostic;
    assert.ok(typeof reason === 'string' && reason.length > 0, `${name}: unreasoned refusal`);
    if (['missing', 'wrong-input-type', 'incompatible-effect-bound'].includes(scenario.variant))
      assert.equal(rejection.stage, 'link', `${name}: binding mismatch bypassed link admission`);
    if (scenario.variant === 'import-time-host-initializer')
      assert.match(reason, /initializer|executable|declaration|module body/i,
        `${name}: source initializer not the refusal reason`);
    if (scenario.variant === 'non-ASCII-declaration-name') {
      assert.equal(result.reports[0].stage, 'parse', `${name}: invalid identifier reached link`);
      assert.match(result.reports[0].diagnostic, /ASCII|identifier|word/i,
        `${name}: not an identifier-specific refusal`);
      assert.equal(result.reports[1].guest_requests, 0);
      assert.equal(result.reports[1].protected_operations, 0);
    }
  }
  return { case_id: 'DX-08', variant: scenario.variant, optimization,
    expected: expected.outcome, observed: result };
}
function request(report, identity, denied, label) {
  const trace = report.request_trace;
  assert.ok(Array.isArray(trace), `${label}: missing production host/guest request trace`);
  assert.equal(trace.length, 1, `${label}: wrong number of real guest requests`);
  const call = trace[0];
  assert.equal(call.adapter_identity, identity, `${label}: mutable alias redirected the call`);
  assert.equal(call.module, 'ledger', `${label}: wrong source module bound to host request`);
  assert.equal(call.version, identity === 'version-B' ? 2 : 1,
    `${label}: wrong versioned operation bound to host request`);
  assert.equal(call.effect, 'test.emit', `${label}: effect erased or forged`);
  assert.equal(call.decision, denied ? 'deny' : 'allow', `${label}: host authorization wrong`);
  assert.ok(Number.isSafeInteger(call.slot) && call.slot >= 0, `${label}: missing bound adapter slot`);
  assert.deepEqual(report.effect_requests, ['test.emit'], `${label}: incorrect effect request accounting`);
  assert.equal(report.guest_requests, 1);
  assert.equal(report.host_requests, 1);
  assert.equal(report.request_trace_complete, true, `${label}: request trace is truncated`);
  assert.equal(report.effect_requests_unrecorded, 0, `${label}: effect accounting is incomplete`);
  assert.equal(report.native_trap, null, `${label}: host request threw outside native denial`);
  assert.equal(report.protected_operations, denied ? 0 : 1,
    `${label}: host authorization did not gate protected work`);
  assert.ok(Array.isArray(report.adapter_invocations), `${label}: missing complete adapter counters`);
  const counts = new Map(report.adapter_invocations.map(row => [row.adapter_identity, row]));
  assert.equal(counts.size, report.adapter_invocations.length, `${label}: duplicate adapter counter`);
  assert.equal(counts.get(identity)?.count, 1, `${label}: selected adapter was not called exactly once`);
  assert.equal(counts.get(identity)?.slot, call.slot, `${label}: observed slot differs from bound adapter`);
  if (identity === 'version-A' && counts.has('version-B'))
    assert.equal(counts.get('version-B').count, 0, `${label}: rebound version B was invoked`);
  if (identity === 'version-B' && counts.has('version-A'))
    assert.equal(counts.get('version-A').count, 0, `${label}: stale A was invoked`);
  return call;
}
function denial(scenario, cli, wasmTools, optimization) {
  const names = scenario.steps.filter(step => step.source).map(step => step.source);
  assert.deepEqual(names, ['ledger_A', 'ledger_B', 'import_ledger_A', 'retain_A',
    'import_ledger_B', 'execute_saved_program']);
  const label = `DX-09-denied-A-after-rebind-${optimization}`;
  const result = session(label, cli, 'denied_A_and_B', names, optimization);
  assert.equal(result.status, 1, `${label}: denied compiled invocation has wrong CLI exit`);
  for (const [index, report] of result.reports.slice(0, -1).entries()) quiet(report, `${label}/${index}`);
  const linkedA = linked(result.reports[2], 1, 'version-A', `${label}/A`);
  const linkedB = linked(result.reports[4], 2, 'version-B', `${label}/B`);
  assert.notEqual(linkedA.resolved_module.identity, linkedB.resolved_module.identity,
    `${label}: versioned modules collapsed to one checked identity`);
  assert.notEqual(linkedA.binding.slot, linkedB.binding.slot,
    `${label}: version A and B unexpectedly share an immutable adapter slot`);
  const report = result.reports.at(-1);
  assert.equal(report.outcome, 'trap', `${label}: host denial did not stop compiled guest`);
  assert.equal(report.stage, 'wasm');
  assert.equal(report.status, 5, `${label}: not the native host denial status`);
  assert.equal(report.protected_operations, 0, `${label}: denied protected work`);
  const call = request(report, 'version-A', true, label);
  assert.equal(call.text, 'A', `${label}: unexpected actual guest text`);
  assert.equal(call.slot, linkedA.binding.slot, `${label}: denied guest ran a rebound binding`);
  assert.deepEqual(report.adapter_invocations.map(row => row.adapter_identity).sort(),
    ['version-A', 'version-B'], `${label}: did not account for every configured adapter`);
  assert.equal(report.adapter_invocations.find(row => row.adapter_identity === 'version-B')?.count, 0,
    `${label}: version-B configured adapter must appear with zero calls`);
  const wasm = wasmArtifacts(label, result.artifacts, wasmTools, true);
  return { case_id: 'DX-09', variant: scenario.variant, optimization, observed: result,
    guest_call: call, validated_compiled_wasm: wasm };
}
function positive(scenario, cli, wasmTools, optimization) {
  const label = `supplemental-${scenario.variant}-${optimization}`;
  const sources = scenario.steps.filter(step => step.source).map(step => step.source);
  const binding = scenario.steps.find(step => step.binding)?.binding ?? 'empty';
  const result = session(label, cli, binding, sources, optimization);
  assert.equal(result.status, 0, `${label}: positive compiled scenario refused`);
  for (const [index, report] of result.reports.slice(0, -1).entries()) quiet(report, `${label}/${index}`);
  const last = result.reports.at(-1);
  assert.equal(last.outcome, 'normal', `${label}: no compiled normal invocation`);
  assert.equal(last.stage, 'wasm');
  const action = scenario.steps.at(-1);
  if (action.output) {
    assert.ok(Array.isArray(last.stack), `${label}: no compiled guest stack`);
    assert.equal(last.stack.length, action.output.length, `${label}: wrong guest stack arity`);
    for (const [index, expected] of action.output.entries()) {
      assert.equal(last.stack[index].type, expected.type, `${label}: wrong guest value type`);
      assert.equal(last.stack[index].value, expected.value, `${label}: wrong compiled guest value`);
    }
    assert.equal(last.guest_requests, 0);
    assert.equal(last.host_requests, 0);
    assert.equal(last.protected_operations, 0);
    assert.deepEqual(last.effect_requests, []);
    assert.deepEqual(last.request_trace, []);
  } else {
    const call = request(last, action.request.adapter_id, false, label);
    assert.equal(call.text, action.request.text, `${label}: wrong UTF-8 guest argument`);
  }
  const wasm = wasmArtifacts(label, result.artifacts, wasmTools, Boolean(action.request));
  return { case_id: 'supplemental', variant: scenario.variant, optimization,
    observed: result, validated_compiled_wasm: wasm };
}
function peerControls(peer) {
  const hostile = plan.scenarios.find(item => item.variant === 'hostile-kernel-candidates');
  const candidates = hostile?.steps.find(step => step.action === 'independent-kernel-adversarial-check')?.candidates;
  assert.ok(Array.isArray(candidates) && candidates.length >= 7,
    'missing declared independent hostile kernel workload');
  assert.equal(new Set(candidates).size, candidates.length, 'duplicate hostile kernel control');
  const workload = retain('inputs/kernel-peer-workload.json', JSON.stringify({
    schema: 'noble-declared-modules-kernel-workload/v1',
    module_source: asset('source', 'schema'),
    emitter_source: asset('source', 'ledger_A'),
    host_bindings: asset('binding', 'A'),
    stale_preparation_source: asset('source', 'import_schema'),
    mutate_registry_source: asset('source', 'schema_v2'),
    resource_dup_source: asset('source', 'resource_dup'),
    resource_drop_source: asset('source', 'resource_drop'),
    resource_capture_source: asset('source', 'resource_capture'),
    ambient_literal_source: asset('source', 'ambient_literal'),
    dependency_source: asset('source', 'first_named_module'),
    candidates,
  }, null, 2) + '\n');
  const run = command('independent-kernel-adversarial-and-stale', peer, [workload]);
  assert.equal(run.status, 0, 'kernel peer did not finish every control');
  const lines = run.bytes.toString('utf8').trim().split('\n');
  assert.equal(lines.length, 1, 'kernel peer must emit one raw JSON report');
  const report = JSON.parse(lines[0]);
  assert.equal(report.schema, 'noble-declared-modules-kernel-peer/v1');
  assert.equal(report.outcome, 'passed');
  assert.equal(report.workload_sha256, hash(workload), 'kernel peer read different workload bytes');
  for (const [key, file] of Object.entries({
    module: asset('source', 'schema'), second: asset('source', 'schema_v2'),
    import: asset('source', 'import_schema'), emitter: asset('source', 'ledger_A'),
    binding: asset('binding', 'A'), resource_dup: asset('source', 'resource_dup'),
    resource_drop: asset('source', 'resource_drop'),
    resource_capture: asset('source', 'resource_capture'),
    ambient_literal: asset('source', 'ambient_literal'),
    dependency: asset('source', 'first_named_module'),
  })) assert.equal(report.source_sha256?.[key], hash(file), `${key}: kernel peer input differs`);
  assert.equal(report.stale?.prepare_accepted, true);
  assert.equal(report.stale?.intervening_commit_accepted, true);
  assert.equal(report.stale?.old_commit_rejected, true);
  assert.equal(report.stale?.namespace_unchanged_after_rejection, true);
  assert.equal(report.stale?.guest_requests, 0);
  const expected = JSON.parse(fs.readFileSync(workload)).candidates;
  assert.deepEqual(report.candidates.map(row => row.name), expected);
  const expectedKinds = {
    'forged-private-constructor-owner': 'PrivateDefinition',
    'forged-private-arm-match': 'PrivateDefinition',
    'forged-same-layout-nominal': 'StackJoin',
    'forged-resource-free-schema': 'InvalidType',
    'raw-sum-to-nominal': 'StackJoin',
    'wrong-effect-witness': 'EffectInclusion',
    'forged-bound-adapter-contract': 'InvalidContract',
    'stale-module-identity': 'StackJoin',
    'ambient-test-emit-bypass': 'PrivateDefinition',
  };
  assert.deepEqual(Object.keys(expectedKinds), expected,
    'typed kernel control set drifted from the frozen workload');
  for (const item of report.candidates) {
    assert.equal(item.production_kernel_check_called, true, `${item.name}: no production kernel check`);
    assert.equal(item.accepted, false, `${item.name}: hostile candidate accepted`);
    assert.equal(item.kernel_outcome, 'invalid');
    assert.equal(item.baseline_outcome, 'accepted');
    assert.ok(item.kernel_calls >= 2, `${item.name}: no baseline and hostile checker calls`);
    assert.equal(item.constraint_kind, expectedKinds[item.name],
      `${item.name}: wrong typed kernel constraint variant`);
    if (item.constraint_kind === 'PrivateDefinition') {
      assert.ok(Number.isSafeInteger(item.constraint_definition) && item.constraint_definition >= 0,
        `${item.name}: missing exact refused Definition id`);
    } else assert.equal(item.constraint_definition, null, `${item.name}: spurious private Definition id`);
    assert.equal(item.constraint_effect,
      item.constraint_kind === 'EffectInclusion' ? 0 : null,
      `${item.name}: wrong typed effect identity`);
  }
  assert.deepEqual(report.resource_operations.map(item => item.operation),
    ['dup', 'drop', 'capture'], 'kernel peer omitted recursive Resource eligibility control');
  for (const item of report.resource_operations) {
    assert.equal(item.production_kernel_check_called, true);
    assert.equal(item.baseline_outcome, 'accepted');
    assert.equal(item.kernel_outcome, 'invalid');
    assert.equal(item.constraint_kind, 'Eligibility');
    assert.equal(item.known_fixture_resource_kind, true);
    assert.equal(item.nominal_origin, 'ledger@1.CounterOwner');
    assert.deepEqual(item.checked_nominal_id, { module: '1', ordinal: 3 });
    assert.deepEqual(item.checked_nominal_shape, { kind: 'opaque-resource', resource_kind: 0 });
  }
  assert.equal(report.dependencies?.first_named_definition, 23);
  assert.equal(report.dependencies?.referenced_definition, 24);
  assert.equal(report.dependencies?.baseline_outcome, 'accepted');
  assert.equal(report.dependencies?.forged_outcome, 'invalid');
  assert.equal(report.dependencies?.production_backend_prepare_called, true);
  return { command: run.id, workload: relative(workload), report };
}
try {
  const prepared = preparePlan(path.join(target, 'plan'));
  plan = JSON.parse(fs.readFileSync(prepared.manifest));
  assert.equal(plan.status, 'planned-not-executed');
  receipt.plan = { file: relative(prepared.manifest), sha256: hash(prepared.manifest),
    selected_cases: prepared.canonical_cases, declared_scenarios: prepared.planned_scenarios };
  const cli = freeze('noble', process.argv[2]);
  const peer = freeze('kernel-peer', process.argv[3]);
  const wasmTools = path.join(target, 'plan', plan.tools.wasm_tools.frozen);
  assert.equal(hash(wasmTools), plan.tools.wasm_tools.sha256);
  const digests = entries => Object.fromEntries(Object.entries(entries).sort()
    .map(([name, entry]) => [name, entry.sha256]));
  receipt.revision_inputs = {
    reviewed_source: digests(plan.sources),
    source_fixtures: digests(plan.program_bytes),
    host_bindings: digests(plan.bindings),
    selected_tools: digests(plan.tools),
    compiled_executables: digests(receipt.tools),
    canonical_workloads: Object.fromEntries(Object.entries(plan.canonical).sort()
      .map(([file, entry]) => [file, {
        source_sha256: entry.source_sha256, workload_sha256: entry.workload_sha256,
      }])),
  };
  receipt.source_revision = `sha256:${sha(JSON.stringify(receipt.revision_inputs))}`;
  for (const scenario of plan.scenarios) {
    try {
      if (scenario.case_id === 'DX-03') receipt.cases.push(staticCase(scenario, cli, wasmTools));
      else if (scenario.case_id === 'DX-08') for (const opt of ['off', 'on'])
        receipt.cases.push(linkCase(scenario, cli, opt));
      else if (scenario.case_id === 'DX-09') for (const opt of ['off', 'on'])
        receipt.cases.push(denial(scenario, cli, wasmTools, opt));
      else if (['compiled-nominal-match', 'first-dynamic-definition-23-compiled',
        'cross-module-exported-nominal-value',
        'positive-compiled-host-invocation', 'rebound-version-B-positive-control',
        'all-public-variant-external-match-executes', 'UTF8-Text-literal-compiled-guest'].includes(scenario.variant))
        for (const opt of ['off', 'on']) receipt.cases.push(positive(scenario, cli, wasmTools, opt));
      else if (scenario.variant === 'ambient-test-emit-bypass-refused') {
        const result = session('ambient-test-emit-bypass-refused', cli, 'A',
          ['ambient_escape_module', 'import_ledger_A'], 'off');
        for (const [index, report] of result.reports.entries())
          quiet(report, `ambient-test-emit-bypass-refused/${index}`);
        assert.ok(['resolve', 'check', 'acceptance'].includes(result.reports[0].stage),
          'legacy test.emit bypass was not rejected in declared module source');
        assert.notEqual(result.reports[0].outcome, 'defined',
          'legacy test.emit bypass module was published');
        assert.match(result.reports[0].diagnostic, /test\.emit|ambient|unbound|bound/i);
        assert.notEqual(result.reports[1].outcome, 'linked',
          'module with ambient host bypass was linked');
        assert.equal(result.artifacts.some(file => file.file.endsWith('.wasm')), false,
          'ambient host bypass reached Wasm worker');
        receipt.cases.push({ case_id: 'supplemental', variant: scenario.variant, observed: result });
      }
      else if (scenario.variant === 'distinct-source-module-version-identity') {
        const result = compile('distinct-source-module-version-identity', cli, wasmTools,
          ['schema', 'schema_v2'], 'distinct_version');
        assert.notEqual(result.status, 0, 'distinct module versions were silently unified');
        receipt.cases.push({ case_id: 'supplemental', variant: scenario.variant, observed: result });
      } else if (['stale-registry-snapshot', 'hostile-kernel-candidates'].includes(scenario.variant)) {
        // One production-linked peer exercises both, exactly once below.
      } else throw Error(`unexecuted required scenario: ${scenario.variant}`);
    } catch (error) { receipt.failures.push({ case_id: scenario.case_id,
      variant: scenario.variant, error: String(error.stack ?? error) }); }
  }
  try { receipt.peer = peerControls(peer); }
  catch (error) { receipt.failures.push({ case_id: 'supplemental',
    variant: 'independent-kernel-adversarial-and-stale', error: String(error.stack ?? error) }); }
  const expected = plan.scenarios.flatMap(scenario => {
    const count = ['DX-08', 'DX-09', 'compiled-nominal-match',
      'first-dynamic-definition-23-compiled', 'cross-module-exported-nominal-value',
      'positive-compiled-host-invocation', 'rebound-version-B-positive-control',
      'all-public-variant-external-match-executes', 'UTF8-Text-literal-compiled-guest']
      .includes(scenario.case_id === 'DX-08' || scenario.case_id === 'DX-09'
      ? scenario.case_id : scenario.variant) ? 2 : 1;
    return ['stale-registry-snapshot', 'hostile-kernel-candidates'].includes(scenario.variant)
      ? [] : Array.from({ length: count }, () => `${scenario.case_id}/${scenario.variant}`);
  }).sort();
  const actual = receipt.cases.map(row => `${row.case_id}/${row.variant}`).sort();
  assert.deepEqual(actual, expected, 'canonical or supplemental scenario/operation omitted');
  for (const row of receipt.cases) if (['DX-03', 'DX-08', 'DX-09'].includes(row.case_id))
    row.source_revision = receipt.source_revision;
  assert.ok(receipt.peer, 'required independent stale/candidate peer was omitted');
} catch (error) { receipt.failures.push({ case_id: 'gate', error: String(error.stack ?? error) }); }
finally {
  for (const entry of watched) try { assert.equal(hash(entry.file), entry.sha256); }
  catch (error) { receipt.integrity_failures.push({ file: entry.file, error: String(error) }); }
  if (plan) {
    try { assert.equal(hash(path.join(target, 'plan', 'plan.json')), receipt.plan.sha256); }
    catch (error) { receipt.integrity_failures.push({ file: 'plan/plan.json', error: String(error) }); }
    for (const [name, source] of Object.entries(plan.sources)) {
      for (const file of [path.join(root, name), path.join(target, 'plan', source.snapshot)])
        try { assert.equal(hash(file), source.sha256); }
        catch (error) { receipt.integrity_failures.push({ file, error: String(error) }); }
    }
    for (const field of ['program_bytes', 'bindings']) for (const entry of Object.values(plan[field])) {
      const file = path.join(target, 'plan', entry.file);
      try { assert.equal(hash(file), entry.sha256); }
      catch (error) { receipt.integrity_failures.push({ file, error: String(error) }); }
    }
    for (const tool of Object.values(plan.tools)) {
      for (const file of [path.join(target, 'plan', tool.frozen), tool.path])
        try { assert.equal(hash(file), tool.sha256); }
        catch (error) { receipt.integrity_failures.push({ file, error: String(error) }); }
    }
  }
  receipt.summary = { case_rows: receipt.cases.length, planned_scenarios: plan?.scenarios.length ?? 0,
    command_count: receipt.commands.length, failure_count: receipt.failures.length,
    integrity_failures: receipt.integrity_failures.length,
    resource_operations: receipt.cases.filter(item => item.resource_operation).map(item => item.resource_operation) };
  receipt.result = receipt.failures.length === 0 && receipt.integrity_failures.length === 0
    ? 'passed' : 'failed';
  fs.writeFileSync(path.join(target, 'report.json'), JSON.stringify(receipt, null, 2) + '\n', { flag: 'wx', mode: 0o400 });
  console.log(JSON.stringify({ schema: receipt.schema, result: receipt.result,
    report: path.join(target, 'report.json'), summary: receipt.summary }));
  process.exitCode = receipt.result === 'passed' ? 0 : 1;
}
