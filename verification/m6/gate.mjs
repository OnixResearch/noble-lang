#!/usr/bin/env node
// Bounded source/config/workload-bound observations. This is not a refinement proof.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const root = fs.realpathSync(path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..'));
const args = process.argv.slice(2);
assert.ok(args.length === 4 || (args.length === 6 && args[4] === '--build-report'),
  'usage: SELECTED_NODE verification/m6/gate.mjs CLI PEER TASK_TEST_BINARY NEW_ARTIFACT_DIR [--build-report FILE]');
const artifacts = path.resolve(args[3]);
assert.ok(artifacts !== root && !artifacts.startsWith(`${root}/`), 'use a fresh directory outside the repository');
assert.ok(!fs.existsSync(artifacts), 'acceptance artifact directory must not exist');
assert.equal(fs.realpathSync(path.dirname(artifacts)), path.dirname(artifacts), 'artifact parent must not contain symlinks');
fs.mkdirSync(artifacts);
for (const name of ['sources', 'executables', 'inputs', 'commands', 'components', 'external']) fs.mkdirSync(path.join(artifacts, name));
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const digest = value => hash(JSON.stringify(value));
const relative = file => path.relative(artifacts, file).split(path.sep).join('/');
const receipt = {
  schema: 'noble-m6-acceptance/v1', result: 'running', profile: 'Component-Async-Bootstrap', source_root: root, artifact_directory: artifacts,
  engine: { executable: process.execPath, versions: process.versions, flags: process.execArgv },
  sources: {}, source_trees: [], executables: {}, retained: {}, commands: [], conformance: [], cases: [], controls: [], native_tests: [], gaps: [], integrity_failures: [],
  assumptions: [
    'Selected Rust, Node, Nix, wasm-tools, Wasmtime and the independent peer are trusted execution tooling; compiler and engine correctness are not proved.',
    'The peer independently links and converts actual Component Model values but deliberately reuses production Noble task, resource and authority decisions.',
    'Host callback authenticity, physical native retirement, clocks and thread scheduling remain explicit host/OS obligations; native task cancellation is not claimed.',
    'Isolated invocation Store lifetime and retained host/native-job lifetime are distinct. Cancellation acknowledgement does not authorize releasing native pins.',
    'Compiler subprocess transcripts are not exposed by the CLI. This gate retains its own commands, emitted Wasm/WAT and independent validation/interface inspection.',
    'Hashes and retained bytes bind local observations; neither a digest nor a peer report authenticates an external fact or creates authority.',
  ],
  non_claims: ['Component-Draft', 'full WASI or stable WASI 0.3', 'general async borrowing', 'remote exactly-once effects',
    'cancellation stops blocking native work', 'guest GC or Store drop releases native work', 'universal compiler/backend refinement',
    'extraction/proofs/Nix checks or earlier milestone regressions executed by this gate', 'stream<u8> bytes encode a domain error'],
};
let serial = 0;
const monitored = [];
const sourceNames = new Set();
const cases = new Map();
let pins, selection, bootstrap, resourceComponent, wideComponent;
function save() { fs.writeFileSync(path.join(artifacts, 'report.json'), JSON.stringify(receipt, null, 2) + '\n'); }
function writeNew(name, data, mode = 0o400) {
  const file = path.join(artifacts, name);
  fs.mkdirSync(path.dirname(file), { recursive: true });
  const bytes = typeof data === 'string' || Buffer.isBuffer(data) ? data : JSON.stringify(data, null, 2) + '\n';
  fs.writeFileSync(file, bytes, { flag: 'wx', mode });
  const sha256 = hash(fs.readFileSync(file));
  receipt.retained[name] = { sha256, bytes: fs.statSync(file).size };
  monitored.push({ file, sha256 });
  return file;
}
function freeze(file, name, executable = false) {
  const supplied = path.resolve(file), real = fs.realpathSync(supplied);
  assert.ok(fs.statSync(real).isFile(), `not a regular input: ${supplied}`);
  const frozen = path.join(artifacts, name);
  fs.mkdirSync(path.dirname(frozen), { recursive: true });
  // Reflinks preserve an independent per-invocation snapshot without multiplying
  // large peer binaries on filesystems that support copy-on-write.
  fs.copyFileSync(real, frozen, fs.constants.COPYFILE_EXCL | fs.constants.COPYFILE_FICLONE);
  fs.chmodSync(frozen, executable ? 0o500 : 0o400);
  const bytes = fs.readFileSync(frozen), sha256 = hash(bytes);
  receipt.retained[name] = { sha256, bytes: bytes.length };
  monitored.push({ file: real, sha256 }, { file: frozen, sha256 });
  return { supplied, real, frozen, sha256, bytes: bytes.length };
}
function source(file) {
  if (sourceNames.has(file)) return;
  sourceNames.add(file);
  const binding = freeze(path.join(root, file), `sources/${file}`);
  receipt.sources[file] = { sha256: binding.sha256, snapshot: relative(binding.frozen), bytes: binding.bytes };
}
function entries(directory) {
  const names = [];
  function walk(prefix) {
    for (const entry of fs.readdirSync(path.join(root, prefix), { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name))) {
      if (['target', '.git', '.lake', 'node_modules'].includes(entry.name)) continue;
      const file = `${prefix}/${entry.name}`;
      assert.equal(entry.isSymbolicLink(), false, `source symlink: ${file}`);
      if (entry.isDirectory()) walk(file);
      else { assert.ok(entry.isFile(), `non-file source: ${file}`); names.push(file); }
    }
  }
  walk(directory);
  return names.sort();
}
function sourceTree(directory) {
  const files = entries(directory);
  receipt.source_trees.push({ directory, files });
  files.forEach(source);
}
function sourceBytes(file) { return fs.readFileSync(path.join(artifacts, receipt.sources[file].snapshot)); }
function command(label, executableName, argv, input = '', timeout = 180_000) {
  const id = ++serial;
  const stem = `commands/${String(id).padStart(4, '0')}-${label.replace(/[^A-Za-z0-9_.-]/g, '-')}`;
  const registered = receipt.executables[executableName];
  assert.ok(registered, `executable not bound: ${executableName}`);
  const invocation = freeze(registered.frozen, `executables/invocation-${id}-${executableName}`, true);
  assert.equal(invocation.sha256, registered.sha256, `frozen executable changed before ${label}`);
  const stdin = writeNew(`${stem}.stdin`, input);
  const stdoutPath = path.join(artifacts, `${stem}.stdout`), stderrPath = path.join(artifacts, `${stem}.stderr`);
  const stdout = fs.openSync(stdoutPath, 'wx', 0o600), stderr = fs.openSync(stderrPath, 'wx', 0o600);
  const entry = { id, label, executable: relative(invocation.frozen), executable_sha256: invocation.sha256, executable_role: executableName,
    arguments: argv, cwd: path.join(artifacts, 'sources'), environment: { PATH: '/nonexistent', LANG: 'C', LC_ALL: 'C', RUST_BACKTRACE: '1' },
    timeout_ms: timeout, stdin: relative(stdin), stdout: relative(stdoutPath), stderr: relative(stderrPath) };
  receipt.commands.push(entry); save();
  let result;
  const inputFd = fs.openSync(stdin, 'r');
  try { result = spawnSync(invocation.frozen, argv, { cwd: entry.cwd, env: entry.environment, stdio: [inputFd, stdout, stderr], timeout, killSignal: 'SIGKILL' }); }
  catch (error) { entry.error = String(error); }
  finally { fs.closeSync(inputFd); fs.closeSync(stdout); fs.closeSync(stderr); }
  for (const file of [stdoutPath, stderrPath]) {
    const bytes = fs.readFileSync(file); fs.chmodSync(file, 0o400);
    receipt.retained[relative(file)] = { sha256: hash(bytes), bytes: bytes.length }; monitored.push({ file, sha256: hash(bytes) });
  }
  Object.assign(entry, { status: result?.status ?? null, signal: result?.signal ?? null, error: entry.error ?? result?.error?.message ?? null });
  save();
  return { entry, text: fs.readFileSync(stdoutPath, 'utf8'), stderr: fs.readFileSync(stderrPath, 'utf8') };
}
function completed(run, status = 0) {
  assert.equal(run.entry.error, null, `command ${run.entry.id}: execution failure`);
  assert.equal(run.entry.signal, null, `command ${run.entry.id}: signal termination`);
  assert.equal(run.entry.status, status, `command ${run.entry.id}: unexpected status; retained stderr ${run.entry.stderr}`);
}
function json(run) {
  const lines = run.text.split('\n').filter(line => line.trim());
  assert.equal(lines.length, 1, `command ${run.entry.id}: expected one JSON report`);
  return JSON.parse(lines[0]);
}
function attach(row, run, report) { row.evidence.push({ command: run.entry.id, ...(report === undefined ? {} : { report }) }); }
function control(name, action) {
  const row = { name, result: 'running', evidence: [] }; receipt.controls.push(row);
  try { action(row); row.result = 'passed'; }
  catch (error) { row.result = 'failed'; row.failure = String(error.stack ?? error); }
  save(); return row;
}
function variant(row, name, action) {
  assert.ok(row.required_variants.includes(name), `undeclared variant ${row.id}/${name}`);
  assert.ok(!row.variants.some(item => item.name === name), `duplicate variant ${row.id}/${name}`);
  const child = { name, result: 'running', evidence: [] }; row.variants.push(child);
  try { action(child); child.result = 'passed'; }
  catch (error) { child.result = 'failed'; child.failure = String(error.stack ?? error); }
  save(); return child;
}
function runCase(id, required, action) {
  const row = { id, result: 'running', required_variants: required, variants: [], evidence: [] }; receipt.cases.push(row);
  try { assert.ok(cases.has(id), `missing canonical workload ${id}`); action(row, cases.get(id)); }
  catch (error) { row.failure = String(error.stack ?? error); }
  row.missing_variants = required.filter(name => !row.variants.some(item => item.name === name));
  row.result = !row.failure && !row.missing_variants.length && row.variants.every(item => item.result === 'passed') ? 'passed' : 'failed';
  save();
}
const reviewedWorkloads = [
  { id: 'WI-11', input: { harness: 'async-order', events: ['enter-first', 'suspend-first', 'resume-first', 'complete-first', 'enter-second'], completion: 'controlled-by-host' },
    expected: { stage: 'wasm-component', outcome: 'ordered', second_starts_before_first_completes: false } },
  { id: 'WI-12', input: { harness: 'stream-future-terminal-matrix', values: ['stream<u8>', 'future<result<s64,string>>'], events: ['success', 'domain-error', 'cancel-before-completion', 'late-completion'] },
    expected: { stage: 'async-cleanup', outcome: 'single-owner-retirement', resurrected_handles: 0, duplicate_releases: 0 } },
  { id: 'WI-16', input: { harness: 'live-async-eligibility', input_types: ['stream<u8>', 'future<s64>', 'Pair<Text,stream<u8>>'], operations: ['dup', 'quote', 'generic-drop', 'generic-serialization'] },
    expected: { stage: 'check', outcome: 'eligibility-reject', guest_requests: 0, protected_operations: 0 } },
];
const reviewedWorker = {
  id: 'WORKER-08', input: { harness: 'async-task-owner-races', variants: [
    { case: 'cancel-before-completion', order: ['admit', 'cancel', 'complete', 'deliver'], terminal_state: 'Retired', result_deliveries: 0, result_owner: 'none', pins_at_cancel_ack: 1 },
    { case: 'complete-cancel-before-delivery', order: ['admit', 'complete-with-resource-result', 'cancel', 'deliver'], terminal_state: 'Retired', result_deliveries: 0, result_owner: 'none', undelivered_result_retired: true },
    { case: 'delivery-before-cancel', order: ['admit', 'complete', 'deliver', 'cancel'], terminal_state: 'Delivered', result_deliveries: 1, result_owner: 'receiver', late_cancel_revokes_result: false },
    { case: 'domain-error-returns-owner', order: ['admit', 'complete-with-declared-error-owner', 'deliver'], terminal_state: 'Delivered', result_deliveries: 1, result_owner: 'receiver-error-branch', normal_typed_return: true },
    { case: 'task-quota-preflight-failure', order: ['deny-admission'], terminal_state: 'NotAdmitted', result_deliveries: 0, result_owner: 'caller-input', protected_operations: 0 },
    { case: 'buffer-quota-preflight-failure', order: ['deny-admission'], terminal_state: 'NotAdmitted', result_deliveries: 0, result_owner: 'caller-input', protected_operations: 0 },
    { case: 'stale-generation-completion', order: ['admit-new-generation', 'old-generation-complete'], terminal_state: 'Pending', result_deliveries: 0, result_owner: 'task', current_generation_changed: false },
    { case: 'wrong-context-callback', order: ['admit', 'foreign-context-complete'], terminal_state: 'Pending', result_deliveries: 0, result_owner: 'task', current_generation_changed: false },
    { case: 'duplicate-completion-after-retirement', order: ['admit', 'cancel', 'complete', 'duplicate-complete'], terminal_state: 'Retired', result_deliveries: 0, result_owner: 'none', native_pin_releases: 1 },
    { case: 'oversized-buffered-result', order: ['admit', 'result-exceeds-reservation', 'finish-retirement'], terminal_state: 'Retired', result_deliveries: 0, result_owner: 'none', primary_outcome: 'budget-exhaustion' },
    { case: 'trap-with-native-pin', order: ['admit', 'trap', 'late-native-completion'], terminal_state: 'Retired', result_deliveries: 0, result_owner: 'none', primary_outcome: 'trap' },
    { case: 'delivery-before-ready', order: ['admit', 'deliver'], terminal_state: 'Pending', result_deliveries: 0, result_owner: 'task' },
    { case: 'repeated-cancellation-during-retirement', order: ['admit', 'cancel', 'cancel'], terminal_state: 'Retiring', result_deliveries: 0, result_owner: 'host-retirement', cleanup_restarted: false },
  ] },
  expected: { stage: 'local-task-transitions', outcome: 'each-order-matches', owner_resurrections: 0, double_releases: 0, unaccounted_pins: 0,
    cancellation_ack_proves_native_completion: false, automatic_retry: false, distributed_exactly_once_claimed: false },
};
const terminalCases = ['success', 'domain-error', 'cancel-before-completion', 'late-completion', 'ready-cancel', 'delivery-before-cancel'];
const liveTypes = ['stream<u8>', 'future<s64>', 'Pair<Text,stream<u8>>'];
const liveOperations = ['dup', 'quote', 'generic-drop', 'generic-serialization'];
const expectedCases = ['WI-11', 'WI-12', 'WI-16', 'WORKER-08'];
const lifecycleCases = [
  'delivery-before-cancel', 'cancel-before-delivery', 'cancel-pending-late-success', 'ready-resource-cancel',
  'domain-error-resource-cleanup', 'resource-delivery', 'result-owner-cancel', 'wrong-native-id', 'invalid-handle', 'foreign-context', 'foreign-owner',
  'stale-generation', 'duplicate-callback', 'repeated-cancel', 'pending-quota', 'terminal-quota', 'byte-quota',
  'pin-quota', 'parked-quota', 'wakeup-quota', 'retirement-quota', 'generation-exhaustion', 'wakeup-bound',
  'premature-pin-settlement', 'invalid-input-disposition', 'authority-preflight', 'authority-owner-preflight',
  'authority-changed-plan', 'authority-revoked', 'authority-quota', 'late-operation-receipt',
];
const hostileSchemaCases = ['lifecycle-changed-state-schema', 'lifecycle-changed-event-schema', 'lifecycle-missing-pair',
  'lifecycle-duplicate-pair', 'lifecycle-invalid-pair', 'lifecycle-invented-state', 'lifecycle-invented-event', 'lifecycle-invented-disposition'];
const requiredControls = ['current-source-build-provenance', 'selected-tools-and-engine-closures', 'native-task-test-lane', 'selected-async-component',
  'explicit-future-cancel', 'explicit-stream-close', 'async-owned-resource-return', 'async-owned-resource-domain-error', 'async-wide-parameters',
  'async-owned-resource-denied-admission', 'async-owned-resource-denied-replay',
  ...liveTypes.map(type => `eligibility-positive/${type}`),
  'lifecycle-matrix', ...hostileSchemaCases, ...lifecycleCases.map(name => `lifecycle/${name}`),
  ...['native-enabled', 'async-disabled', 'stackful-disabled', 'wasi-p3-clock', 'wasi-stable-version-rejected'].map(mode => `compatibility-${mode}`),
  ...['runnable-fuel', 'runnable-epoch', 'blocking-deadline'].map(mode => `progress-${mode}`)];
function input(name, bytes) { return writeNew(`inputs/${++serial}-${name}`, bytes); }
function fixture(name) { return sourceBytes(`verification/m6/cases/${name}`); }
function bind(row, wit, world) {
  const witPath = input('bindings.wit', wit);
  const run = command('component-bindings', 'cli', ['component', 'bindings', witPath, world]);
  const report = json(run); attach(row, run, report); completed(run);
  assert.equal(report.schema, 'noble-component/v1'); assert.equal(report.profile, 'Component-Async-Bootstrap');
  assert.equal(report.outcome, 'typed-bindings'); assert.equal(report.component_emitted, false);
  return report;
}
function retainComponent(directory) {
  for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
    const file = path.join(directory, entry.name);
    assert.ok(entry.isFile(), `unexpected compiler output ${file}`);
    if (Object.hasOwn(receipt.retained, relative(file))) continue;
    const bytes = fs.readFileSync(file); fs.chmodSync(file, 0o400);
    receipt.retained[relative(file)] = { sha256: hash(bytes), bytes: bytes.length }; monitored.push({ file, sha256: hash(bytes) });
  }
}
function compile(row, label, wit, world, exports, refusal = false) {
  const witPath = input(`${label}.wit`, wit);
  const sources = Object.entries(exports).map(([name, bytes]) => ({ name, bytes, path: input(`${label}-${name}.noble`, bytes) }));
  const directory = path.join(artifacts, 'components', `${++serial}-${label}`);
  const run = command(`${label}-compile`, 'cli', ['component', 'compile', witPath, world, directory, ...sources.map(item => `${item.name}=${item.path}`)]);
  const report = json(run); attach(row, run, report);
  assert.equal(report.schema, 'noble-component/v1');
  if (refusal) {
    completed(run, 2); assert.equal(report.outcome, 'error'); assert.equal(report.component_emitted, false);
    assert.equal(fs.existsSync(directory), false, 'eligibility refusal must precede publication');
    assert.equal(report.diagnostic?.code, 'component-check', 'must reject in source checking, not lowering or a tool failure');
    assert.match(report.diagnostic.message, /^Export: /, 'WIT admission must succeed before export eligibility is checked');
    assert.match(report.diagnostic.message, /^Export: kernel rejected a non-capturable value/, 'the actual kernel eligibility predicate must reject');
    assert.doesNotMatch(report.diagnostic.message, /closed exact WIT interface|unknown|unsupported|budget|exhaust/i, 'a non-eligibility rejection is not WI-16 evidence');
    return report;
  }
  completed(run); assert.equal(report.outcome, 'compiled'); assert.equal(report.component_emitted, true);
  assert.equal(report.profile, 'Component-Async-Bootstrap'); assert.equal(report.independent_kernel_check, true);
  assert.equal(report.abi, 'wasm-tools-1.245.1-async-stackful-memory32-utf8');
  assert.deepEqual(fs.readdirSync(directory).sort(), ['component.wasm', 'core.wasm', 'module.wat', 'report.json', 'world.wit', ...sources.map((_, index) => `export-${index}.noble`)].sort());
  retainComponent(directory);
  assert.deepEqual(fs.readFileSync(path.join(directory, 'world.wit')), Buffer.from(wit));
  sources.forEach((item, index) => assert.deepEqual(fs.readFileSync(path.join(directory, `export-${index}.noble`)), Buffer.from(item.bytes)));
  assert.deepEqual(JSON.parse(fs.readFileSync(path.join(directory, 'report.json'))), report);
  const component = path.join(directory, 'component.wasm');
  const validate = command(`${label}-validate`, 'wasm_tools', ['validate', component, '--features', 'cm-async,cm-async-stackful,cm-async-builtins']); attach(row, validate); completed(validate);
  const inspect = command(`${label}-interface`, 'wasm_tools', ['component', 'wit', component]); attach(row, inspect); completed(inspect);
  const actualExports = [...inspect.text.matchAll(/^\s*export\s+([^\s:]+)\s*:/gm)].map(match => match[1]).sort();
  assert.deepEqual(actualExports, sources.map(item => item.name).sort());
  row.evidence.push({ component: relative(component), sha256: hash(fs.readFileSync(component)), actual_exports: actualExports });
  return { component, directory, report, exports: actualExports };
}
function requireBootstrap() { assert.ok(bootstrap, 'selected async component did not compile and independently validate'); return bootstrap; }
function cleanHost(host) {
  assert.ok(host && typeof host === 'object', 'missing retained host report');
  for (const state of ['pending', 'ready', 'retiring']) assert.equal(host.tasks[state], 0, `unsettled task state ${state}`);
  for (const slot of host.slots) {
    assert.ok(['Delivered', 'Retired'].includes(slot.snapshot.state));
    assert.equal(slot.snapshot.finalized, true, 'a Delivered state alone does not settle native lifetime debt');
    assert.equal(slot.snapshot.native_stopped, true); assert.equal(slot.snapshot.pins, 0);
    assert.equal(slot.native_finished, true);
  }
  assert.equal(host.tasks.outstanding_pins, 0); assert.equal(host.tasks.queued_wakeups, 0);
  for (const name of ['tasks', 'terminal_results', 'bytes', 'parked_payloads', 'pins', 'wakeups', 'retirement_work']) assert.equal(host.tasks.reserved[name], 0, `leaked reservation ${name}`);
  for (const name of ['live', 'busy', 'retiring', 'native_pins', 'guest_owners', 'task_owners', 'native_payloads']) assert.equal(host.resources[name], 0, `unretired resource ${name}`);
  assert.equal(host.resources.native_releases, host.resources.native_created, 'every created native payload must retire once');
  assert.equal(host.accounting.pins_released, host.accounting.pins_acquired, 'every acquired native pin must settle once');
  assert.equal(host.accounting.reservations_released, host.accounting.admissions, 'admission reservations must settle once');
  assert.equal(host.storage_bytes, 0); assert.equal(host.storage_capacity_bytes, 0);
  assert.equal(host.trace_dropped, 0);
}
function peerRun(row, compiled, exportName, scenario, expected = 'normal') {
  const run = command(`peer-${exportName}-${scenario}`, 'peer', [compiled.component, exportName, scenario]);
  const report = json(run); attach(row, run, report); completed(run);
  assert.equal(report.schema, 'noble-m6-peer/v1'); assert.equal(report.engine, `wasmtime-${pins.wasmtime_version}`);
  assert.equal(report.case, scenario); assert.equal(report.observation.outcome, expected);
  assert.equal(report.native_jobs_joined, true); assert.equal(report.host_retained_after_store_drop, true);
  assert.equal(report.observation.task_exit_observed, expected === 'normal');
  cleanHost(report.host);
  if (expected !== 'normal') assert.deepEqual(report.observation.results, [], 'cancelled/abnormal tasks cannot publish a trusted result');
  return report;
}
const integer = value => ({ type: 'I64', value: String(value) });
const sum = (tag, value) => ({ type: 'Sum', tag, value });
function terminalResult(kind, scenario) {
  if (scenario === 'domain-error') return [sum('right', { type: 'Text', value: 'domain-error' })];
  return [sum('left', kind === 'future' ? integer(42) : { type: 'List', items: [0, 127, 255].map(integer) })];
}
function loadBuild(row, file) {
  if (!file) { receipt.gaps.push({ control: row.name, message: 'No build report: executable/source correspondence is not established.' }); throw Error('source-bound build report required for acceptance; use --build-report FILE'); }
  const binding = freeze(file, 'external/build.json');
  const build = JSON.parse(fs.readFileSync(binding.frozen));
  assert.equal(build.schema, 'noble-m6-build/v1'); assert.equal(build.result, 'built'); assert.deepEqual(build.integrity_failures, []);
  assert.deepEqual(Object.keys(build.sources).sort(), Object.keys(receipt.sources).sort(), 'build/current-source inventory mismatch');
  for (const [name, value] of Object.entries(build.sources)) {
    assert.equal(value.sha256, receipt.sources[name]?.sha256, `stale compiled source/config ${name}`);
    const frozen = freeze(path.resolve(path.dirname(binding.real), value.snapshot), `external/build-sources/${name}`);
    assert.equal(frozen.sha256, value.sha256, `build snapshot changed: ${name}`);
  }
  assert.deepEqual(build.source_trees, receipt.source_trees, 'build source tree inventory mismatch');
  assert.deepEqual(build.pins, pins); assert.equal(build.source_revision, receipt.source_revision);
  assert.equal(build.tools.rustc.real, fs.realpathSync(path.join(selection.tool_paths.quality_rust.output, 'bin/rustc')));
  assert.equal(build.tools.cargo.real, fs.realpathSync(path.join(selection.tool_paths.quality_rust.output, 'bin/cargo')));
  assert.deepEqual(build.configuration.workspace, ['--workspace', '--all-targets', '--all-features', '--locked', '--offline']);
  for (const name of ['cli', 'peer', 'task_tests']) {
    const binary = build.binaries[name]; assert.ok(binary, `build omitted ${name}`);
    assert.equal(binary.sha256, receipt.executables[name].sha256, `supplied ${name} differs from Cargo build artifact`);
    const frozen = freeze(path.resolve(path.dirname(binding.real), binary.snapshot), `external/build-binaries/${name}`, true);
    assert.equal(frozen.sha256, binary.sha256);
    const compileCommand = build.commands.find(item => item.id === binary.command);
    assert.ok(compileCommand && compileCommand.args.includes('--message-format=json'), 'binary has no Cargo JSON provenance');
  }
  const commands = [];
  for (const item of build.commands) {
    assert.equal(item.status, 0); assert.equal(item.error, null); assert.equal(item.signal, null);
    const streams = {};
    for (const stream of ['stdin', 'stdout', 'stderr']) {
      const frozen = freeze(path.resolve(path.dirname(binding.real), item[`${stream}_path`]), `external/build-commands/${item.id}.${stream}`);
      assert.equal(frozen.sha256, item[`${stream}_sha256`], `changed build transcript ${item.id}/${stream}`); streams[stream] = relative(frozen.frozen);
    }
    assert.equal(hash(fs.readFileSync(item.executable)), item.executable_sha256, 'build tool changed after compilation');
    commands.push({ ...item, retained_streams: streams });
  }
  for (const [name, binary] of Object.entries(build.binaries)) {
    const item = commands.find(item => item.id === binary.command);
    const messages = fs.readFileSync(path.join(artifacts, item.retained_streams.stdout), 'utf8').split('\n').filter(line => line.startsWith('{')).map(line => JSON.parse(line));
    assert.ok(messages.some(message => message.reason === 'compiler-artifact' && message.executable === binary.compiled_path
      && message.target.name === binary.target && message.target.kind.includes(binary.kind)), `missing Cargo artifact record ${name}`);
  }
  row.evidence.push({ build_report: relative(binding.frozen), source_revision: build.source_revision, commands });
  receipt.build_provenance = { report: relative(binding.frozen), sha256: binding.sha256, source_revision: build.source_revision, result: build.result };
}
function nativeTests(row) {
  const listing = command('task-test-inventory', 'task_tests', ['--list', '--format', 'terse']); attach(row, listing); completed(listing);
  const names = listing.text.split('\n').filter(line => line.endsWith(': test')).map(line => line.slice(0, -6)).sort();
  assert.ok(names.length > 0 && names.length <= 256, 'empty or unbounded task test inventory');
  assert.equal(new Set(names).size, names.length, 'duplicate task test names');
  const required = [
    'receivers::delivery_transfers_ready_owners_once_without_settling_native_debt',
    'retirement::cancellation_before_completion_never_restores_input_or_result_owners',
    'receivers::ready_domain_error_resources_remain_owned_until_delivery_or_cancellation',
    'coverage::completion_requires_a_disjoint_complete_input_disposition',
    'retirement::oversized_result_retires_without_admitting_unreserved_bytes',
    'callbacks::terminal_slot_reuse_rejects_stale_native_events',
    'callbacks::callback_identity_and_receiver_context_are_independently_checked',
    'retirement::every_abnormal_exit_preserves_its_primary_failure_through_late_success',
    'callbacks::wake_notifications_coalesce_and_exhaustion_retires_without_new_work',
    'budgets::each_admission_budget_rejects_without_consuming_supplied_obligations',
    'budgets::abandoned_preparation_does_not_consume_capacity_or_generation',
    'coverage::schema_bound_matrix_requires_every_pair_and_exact_disposition',
    'receivers::duplicate_completion_cannot_replace_ready_result_or_deliver_it_twice',
  ].sort();
  assert.deepEqual(names, required, 'review a changed native regression inventory explicitly');
  row.observed_tests = names;
  for (const name of names) {
    const run = command(`task-test-${name}`, 'task_tests', ['--exact', name, '--nocapture', '--test-threads=1']);
    const test = { name, command: run.entry.id, result: 'running', scope: 'deterministic production-kernel lane; not independent native async Component Model evidence' };
    receipt.native_tests.push(test);
    try {
      completed(run); assert.ok(run.text.includes(`test ${name} ... ok`), 'exact requested test did not pass');
      assert.match(run.text, /test result: ok\. 1 passed; 0 failed; 0 ignored; 0 measured;/); test.result = 'passed';
    } catch (error) { test.result = 'failed'; test.failure = String(error); }
  }
  assert.ok(receipt.native_tests.every(item => item.result === 'passed'), 'native task test failed');
}
function peerControl(row, flag, name, component) {
  const argv = component ? [flag, component.component, name] : [flag, name];
  const run = command(`${flag.slice(2)}-${name}`, 'peer', argv);
  const report = json(run); attach(row, run, report); completed(run);
  assert.equal(report.schema, 'noble-m6-peer/v1');
  if (Object.hasOwn(report, 'engine')) assert.equal(report.engine, `wasmtime-${pins.wasmtime_version}`);
  return report;
}

function captured(report, label) {
  const matches = report.control.snapshots.filter(item => item.label === label);
  assert.equal(matches.length, 1, `missing unique observed snapshot ${label}`);
  return matches[0].host;
}
function currentTask(host) {
  const slots = host.slots.filter(item => item.snapshot);
  assert.equal(slots.length, 1, 'control must expose its exact current retained task');
  return slots[0].snapshot;
}
function rejected(report, operation, error) {
  const matches = report.control.results.filter(item => item.operation === operation);
  assert.equal(matches.length, 1, `missing actual API rejection ${operation}`);
  assert.equal(matches[0].status, 'rejected'); assert.ok(matches[0].error.includes(error), `wrong ${operation} refusal`);
  return matches[0];
}
function accepted(report, operation) {
  const matches = report.control.results.filter(item => item.operation === operation);
  assert.equal(matches.length, 1, `missing actual API acceptance ${operation}`);
  assert.equal(matches[0].status, 'accepted');
  return matches[0].value;
}
function unchangedAdmission(host) {
  const snapshot = currentTask(host);
  const admitted = host.events.filter(item => item.action === 'Admitted' && item.snapshot.table === snapshot.table
    && item.snapshot.slot === snapshot.slot && item.snapshot.generation === snapshot.generation);
  assert.equal(admitted.length, 1); assert.deepEqual(snapshot, admitted[0].snapshot, 'rejected request changed retained task/owner state');
}
function operationReceipt(host, { claim, invocation, operation, argument }) {
  const receipts = host.authority.receipts.filter(item => item.claim === claim);
  assert.equal(receipts.length, 1, 'must retain exactly one authentic local operation receipt');
  const observed = receipts[0];
  assert.equal(observed.scope, 'OperationBoundary'); assert.equal(observed.source, 2);
  assert.equal(observed.invocation, invocation); assert.equal(observed.operation, operation);
  const bytes = Buffer.alloc(8); bytes.writeBigInt64LE(BigInt(argument));
  assert.deepEqual(observed.arguments, [...bytes]); assert.ok(Number.isInteger(observed.attempt));
  return observed;
}
function assertLifecycle(name, report) {
  assert.equal(report.case, name); assert.equal(report.control.case, name);
  assert.equal(report.observation.outcome, 'normal'); assert.deepEqual(report.observation.results, []);
  cleanHost(report.host);
  const aliases = {
    'cancel-before-completion': 'cancel-pending-late-success',
    'complete-cancel-before-delivery': 'ready-resource-cancel',
    'task-quota-preflight-failure': 'pending-quota', 'buffer-quota-preflight-failure': 'byte-quota',
    'stale-generation-completion': 'stale-generation', 'wrong-context-callback': 'foreign-context',
    'duplicate-completion-after-retirement': 'duplicate-callback',
    'repeated-cancellation-during-retirement': 'repeated-cancel',
  };
  const scenario = aliases[name] ?? name;
  const final = report.host, accounting = final.accounting;
  let boundary;
  if (scenario === 'delivery-before-cancel') {
    const ready = captured(report, 'ready'), delivered = captured(report, 'delivered'), late = captured(report, 'late-cancel');
    assert.equal(ready.tasks.ready, 1); assert.equal(ready.tasks.outstanding_pins, 1);
    assert.equal(delivered.tasks.delivered, 1); assert.equal(delivered.resources.guest_owners, 1);
    assert.deepEqual(delivered.resources.returned_values, [42]);
    assert.deepEqual(currentTask(late), currentTask(delivered)); assert.deepEqual(late.resources, delivered.resources);
    assert.equal(late.accounting.cancellations, 0); assert.equal(late.accounting.duplicates, 1);
    assert.equal(accepted(report, 'late-cancel').finalized, true);
    assert.equal(accounting.deliveries, 1); assert.equal(final.resources.native_releases, 1); boundary = 'late-cancel';
  } else if (['cancel-before-delivery', 'ready-resource-cancel', 'result-owner-cancel'].includes(scenario)) {
    const ready = captured(report, 'ready'), cancelled = captured(report, 'cancelled');
    assert.equal(ready.tasks.ready, 1); assert.equal(currentTask(ready).outcome, 'Success');
    assert.equal(cancelled.tasks.retiring, 1); assert.equal(cancelled.tasks.outstanding_pins, 1);
    assert.equal(currentTask(cancelled).failure, 'Cancelled'); assert.equal(currentTask(cancelled).native_stopped, false);
    rejected(report, 'deliver-cancelled', 'Retiring'); assert.equal(accounting.deliveries, 0); assert.equal(accounting.cancellations, 1);
    if (scenario !== 'cancel-before-delivery') {
      assert.equal(ready.resources.task_owners, 1); assert.equal(cancelled.resources.native_releases, 0);
      assert.equal(cancelled.resources.task_owners, 1); assert.equal(final.resources.native_releases, 1); assert.equal(accounting.cleaned_results, 1);
    }
    boundary = 'retired';
  } else if (['cancel-pending-late-success', 'late-operation-receipt'].includes(scenario)) {
    const admitted = captured(report, 'admitted'), cancelled = captured(report, 'cancelled'), late = captured(report, 'late-complete');
    assert.equal(admitted.tasks.pending, 1); assert.equal(admitted.authority.counters.witness_consumptions, 1);
    assert.equal(cancelled.tasks.retiring, 1); assert.equal(cancelled.tasks.outstanding_pins, 1);
    assert.equal(currentTask(cancelled).native_stopped, false); assert.equal(cancelled.authority.local_total, 0);
    assert.equal(cancelled.authority.invocation, 'Cancelled'); assert.equal(cancelled.authority.witness_available, false);
    assert.equal(currentTask(late).state, 'Retiring'); assert.equal(currentTask(late).failure, 'Cancelled'); assert.equal(late.tasks.outstanding_pins, 1);
    assert.equal(late.authority.local_total, 40); assert.equal(final.authority.local_total, 40);
    operationReceipt(late, { claim: 'OperationSuccess', invocation: 'Cancelled', operation: 'noble-test:async-boundary/host@1.0.0#first', argument: 40 });
    assert.deepEqual(final.authority.receipts, late.authority.receipts);
    assert.equal(final.authority.counters.witness_consumptions, 1); assert.equal(final.authority.counters.protected_operations, 1);
    assert.equal(final.authority.counters.successful_deliveries, 0);
    assert.equal(accounting.completions, 1); assert.equal(accounting.deliveries, 0); assert.equal(accounting.pins_released, 1);
    rejected(report, 'deliver-cancelled', 'Retiring'); rejected(report, 'replay-consumed-witness', 'ConsumedWitness'); boundary = 'retired';
  } else if (['resource-delivery', 'domain-error-returns-owner', 'domain-error-resource-cleanup'].includes(scenario)) {
    const domain = scenario !== 'resource-delivery', returning = scenario !== 'domain-error-resource-cleanup';
    const admitted = captured(report, 'admitted'), ready = captured(report, 'ready'), settled = captured(report, 'native-settled-ready');
    rejected(report, 'sender-reuse', 'unknown resource'); assert.equal(admitted.resources.task_owners, 1); assert.equal(admitted.resources.guest_owners, 0);
    assert.equal(currentTask(ready).outcome, domain ? 'DomainError' : 'Success'); assert.equal(ready.tasks.outstanding_pins, 1);
    assert.equal(settled.tasks.ready, 1); assert.equal(settled.tasks.outstanding_pins, 0);
    if (returning) {
      assert.equal(settled.resources.task_owners, 1); assert.equal(settled.resources.native_releases, 0);
      const returned = accepted(report, 'owner-return'), delivered = captured(report, 'delivered');
      assert.equal(returned.before.table, returned.after.table); assert.equal(returned.before.slot, returned.after.slot);
      assert.notEqual(returned.before.generation, returned.after.generation); assert.equal(returned.before.value, 41);
      assert.equal(returned.after.value, domain ? 41 : 42); assert.equal(delivered.resources.guest_owners, 1);
      assert.deepEqual(delivered.resources.returned_values, [domain ? 41 : 42]); boundary = 'delivered';
    } else {
      assert.equal(settled.resources.task_owners, 0); assert.equal(settled.resources.native_releases, 1);
      assert.equal(final.resources.guest_owners, 0); assert.equal(accounting.cleaned_inputs, 1); boundary = 'native-settled-ready';
    }
    assert.equal(accounting.deliveries, 1); assert.equal(final.resources.native_releases, 1);
    assert.equal(final.authority.counters.witness_consumptions, 1); assert.equal(final.authority.counters.protected_operations, 1);
    const receipt_ = final.authority.receipts.find(item => item.claim === (domain ? 'OperationFailure' : 'OperationSuccess'));
    assert.ok(receipt_); assert.equal(receipt_.scope, 'OperationBoundary');
  } else if (['wrong-native-id', 'invalid-handle', 'foreign-context'].includes(scenario)) {
    rejected(report, 'hostile-completion', { 'wrong-native-id': 'WrongNative', 'invalid-handle': 'InvalidHandle', 'foreign-context': 'WrongContext' }[scenario]);
    const after = captured(report, 'rejected'); assert.equal(after.tasks.pending, 1); unchangedAdmission(after);
    assert.equal(after.accounting.completions, 0); assert.equal(after.accounting.deliveries, 0); boundary = 'rejected';
  } else if (scenario === 'foreign-owner') {
    for (const [operation, error] of [['foreign-owner', 'InvalidHandle'], ['foreign-owner-context', 'WrongContext'],
      ['stale-owner-generation', 'WrongGeneration'], ['wrong-owner-kind', 'WrongKind']]) rejected(report, operation, error);
    const after = captured(report, 'rejected'); assert.equal(after.resources.guest_owners, 1); assert.equal(after.resources.native_releases, 0);
    assert.equal(after.authority.counters.protected_operations, 0); cleanHost(accepted(report, 'foreign-host-cleanup')); boundary = 'rejected';
  } else if (scenario === 'stale-generation') {
    rejected(report, 'old-generation-completion', 'WrongGeneration');
    const after = captured(report, 'rejected'); assert.equal(after.tasks.pending, 1); unchangedAdmission(after);
    const admitted = after.events.filter(item => item.action === 'Admitted').map(item => item.snapshot);
    assert.equal(admitted.length, 2); assert.equal(admitted[0].slot, admitted[1].slot);
    assert.notEqual(admitted[0].generation, admitted[1].generation); assert.notEqual(admitted[0].native, admitted[1].native);
    const current = currentTask(after);
    assert.equal(after.events.filter(item => item.action === 'ResultDelivered' && item.snapshot.generation === current.generation).length, 0);
    boundary = 'rejected';
  } else if (scenario === 'duplicate-callback') {
    assert.equal(accounting.admissions, 1); assert.equal(accounting.completions, 1); assert.equal(accounting.deliveries, 0);
    assert.equal(accounting.pins_released, 1); assert.equal(accounting.cleaned_results, 1);
    assert.equal(accounting.duplicates, 2); assert.equal(final.resources.native_releases, 1); boundary = 'duplicate-rejected-as-noop';
  } else if (scenario === 'repeated-cancel') {
    const repeated = captured(report, 'repeated-cancel');
    assert.equal(repeated.tasks.retiring, 1); assert.equal(repeated.tasks.outstanding_pins, 1);
    assert.equal(repeated.accounting.cancellations, 1); assert.equal(repeated.accounting.duplicates, 2);
    assert.equal(repeated.accounting.cleaned_buffers, 0); assert.equal(repeated.accounting.pins_released, 0);
    assert.deepEqual(repeated.events.filter(item => item.operation === 'cancel').map(item => item.action),
      ['CancellationAcknowledged', 'Duplicate', 'Duplicate']);
    assert.equal(accounting.completions, 1); assert.equal(accounting.deliveries, 0); boundary = 'repeated-cancel';
  } else if (['pending-quota', 'terminal-quota', 'byte-quota', 'pin-quota', 'parked-quota', 'wakeup-quota', 'retirement-quota'].includes(scenario)) {
    const error = { 'pending-quota': 'TaskCapacity', 'terminal-quota': 'TerminalCapacity', 'byte-quota': 'ByteCapacity',
      'pin-quota': 'PinCapacity', 'parked-quota': 'ParkedCapacity', 'wakeup-quota': 'WakeCapacity', 'retirement-quota': 'RetirementCapacity' }[scenario];
    rejected(report, 'quota-admission', error);
    const after = captured(report, 'rejected'); assert.equal(after.accounting.admissions, 1); assert.equal(after.tasks.pending, 1);
    assert.equal(after.authority.witness_available, true); assert.equal(after.authority.counters.witness_consumptions, 0);
    assert.equal(after.authority.counters.protected_operations, 0); unchangedAdmission(after); boundary = 'rejected';
  } else if (scenario === 'generation-exhaustion') {
    rejected(report, 'generation-exhaustion', 'GenerationExhausted');
    const after = captured(report, 'rejected'); assert.equal(after.tasks.reserved.tasks, 0); assert.equal(after.accounting.admissions, 1);
    assert.equal(after.authority.witness_available, true); assert.equal(after.authority.counters.witness_consumptions, 0); boundary = 'rejected';
  } else if (scenario === 'wakeup-bound') {
    const coalesced = captured(report, 'coalesced'), exhausted = captured(report, 'exhausted');
    assert.equal(coalesced.tasks.queued_wakeups, 1); assert.equal(coalesced.accounting.wake_queued, 1);
    assert.ok(coalesced.events.some(item => item.action === 'WakeCoalesced'));
    assert.equal(currentTask(exhausted).failure, 'Budget'); assert.equal(exhausted.tasks.retiring, 1);
    assert.equal(exhausted.tasks.outstanding_pins, 1); assert.equal(accounting.wake_queued, 2); assert.equal(accounting.wake_removed, 2);
    assert.equal(accounting.deliveries, 0); boundary = 'exhausted';
  } else if (scenario === 'premature-pin-settlement') {
    for (const [operation, error] of [['shell-settle-running', 'NativeStillRunning'], ['pin-settle-running', 'NativeStillRunning'],
      ['completion-is-not-stop', 'NativeStillRunning'], ['duplicate-pin-settlement', 'InvalidPins'],
      ['result-cleanup-before-delivery', 'InvalidCleanup'], ['finish-ready', 'Ready']]) rejected(report, operation, error);
    const completed_ = captured(report, 'completed-pinned');
    assert.equal(completed_.tasks.ready, 1); assert.equal(completed_.tasks.outstanding_pins, 1);
    assert.equal(currentTask(completed_).native_stopped, false); assert.equal(accounting.deliveries, 1); boundary = 'completed-pinned';
  } else if (scenario === 'invalid-input-disposition') {
    for (const [operation, error] of [['overlapping-disposition', 'InvalidDisposition'], ['missing-disposition', 'InvalidDisposition'],
      ['invented-input', 'InvalidDisposition'], ['unreserved-result-owner', 'ResultCapacity']]) rejected(report, operation, error);
    const after = captured(report, 'rejected'); unchangedAdmission(after); assert.equal(after.resources.task_owners, 1);
    assert.equal(after.resources.native_releases, 0); assert.equal(after.accounting.completions, 0);
    assert.deepEqual(final.resources.returned_values, [42]); assert.equal(accounting.deliveries, 1); boundary = 'rejected';
  } else if (['authority-preflight', 'authority-owner-preflight'].includes(scenario)) {
    rejected(report, 'reservation-before-authority', 'TaskCapacity');
    const before = captured(report, 'preflight-preserved'), after = captured(report, 'admitted-after-capacity-returned');
    assert.equal(before.authority.counters.witness_consumptions, 0); assert.equal(before.authority.counters.protected_operations, 0);
    assert.equal(before.authority.witness_available, true);
    if (scenario === 'authority-owner-preflight') { assert.equal(before.resources.guest_owners, 1); assert.equal(before.resources.native_releases, 0); }
    assert.equal(after.authority.counters.witness_consumptions, 1); assert.equal(after.authority.counters.protected_operations, 1);
    assert.equal(after.accounting.admissions, 2); assert.equal(after.accounting.deliveries, 2);
    assert.equal(after.authority.counters.successful_deliveries, 1); boundary = 'admitted-after-capacity-returned';
  } else if (['authority-changed-plan', 'authority-revoked', 'authority-quota'].includes(scenario)) {
    const error = { 'authority-changed-plan': 'ChangedPlan', 'authority-revoked': 'Revoked', 'authority-quota': 'QuotaExhausted' }[scenario];
    rejected(report, 'authority-denial', error); const denied = captured(report, 'denied');
    assert.equal(denied.tasks.reserved.tasks, 0); assert.equal(denied.accounting.admissions, 0);
    assert.equal(denied.authority.counters.witness_consumptions, 0); assert.equal(denied.authority.counters.protected_operations, 0);
    assert.equal(denied.authority.witness_available, true); assert.equal(denied.authority.receipts.length, 1);
    assert.equal(denied.authority.receipts[0].claim, 'Denial'); assert.equal(denied.authority.receipts[0].denial, error);
    assert.equal(denied.authority.receipts[0].scope, 'AdmissionBoundary'); boundary = 'denied';
  } else if (scenario === 'oversized-buffered-result') {
    const oversized = captured(report, 'oversized');
    assert.equal(currentTask(oversized).failure, 'Budget'); assert.equal(oversized.tasks.retiring, 1);
    assert.equal(oversized.tasks.outstanding_pins, 1); assert.equal(oversized.accounting.rejected_result_bytes, 9);
    assert.equal(oversized.resources.native_releases, 0); assert.equal(final.resources.native_releases, 1);
    assert.equal(accounting.deliveries, 0); assert.equal(currentTask(final).failure, 'Budget'); boundary = 'oversized';
  } else if (scenario === 'trap-with-native-pin') {
    const trapped = captured(report, 'trapped');
    assert.equal(trapped.tasks.retiring, 1); assert.equal(trapped.tasks.outstanding_pins, 1);
    assert.equal(currentTask(trapped).failure, 'Trap'); assert.equal(currentTask(final).failure, 'Trap');
    assert.equal(accounting.deliveries, 0);
    operationReceipt(final, { claim: 'OperationSuccess', invocation: 'Failed', operation: 'noble-test:async-boundary/host@1.0.0#first', argument: 40 });
    assert.equal(final.authority.counters.successful_deliveries, 0); boundary = 'trapped';
  } else if (scenario === 'delivery-before-ready') {
    rejected(report, 'deliver-pending', 'Pending'); const pending = captured(report, 'pending');
    unchangedAdmission(pending); assert.equal(pending.tasks.pending, 1); assert.equal(pending.accounting.deliveries, 0); boundary = 'pending';
  } else throw Error(`unreviewed lifecycle scenario ${name}`);
  return { label: boundary, host: captured(report, boundary), final_host: final };
}
const lifecycleStates = ['Pending', 'Ready', 'Delivered', 'Retiring', 'Retired'];
const lifecycleEvents = ['Inspect', 'CompleteSuccess', 'CompleteDomainError', 'Deliver', 'Cancel', 'Trap', 'Deadline', 'Budget',
  'InternalFailure', 'NativeStopped', 'SettlePins', 'Cleanup', 'Wake', 'TakeWake', 'Finish'];
const stateSchema = 'noble-kernel::async_tasks::domain::State{Pending;Ready;Delivered;Retiring;Retired}';
const eventSchema = 'noble-kernel::async_tasks::domain::Event{Inspect;CompleteSuccess{native:NativeId,completion:Completion};'
  + 'CompleteDomainError{native:NativeId,completion:Completion};Deliver;Cancel;Trap;Deadline;Budget;InternalFailure;'
  + 'NativeStopped{native:NativeId};SettlePins{native:NativeId,pins:u64};Cleanup(Obligations);Wake{native:NativeId};TakeWake;Finish};'
  + 'noble-kernel::async_tasks::domain::NativeId(u64);'
  + 'noble-kernel::async_tasks::domain::Completion{inputs:Disposition,produced:u64,bytes:usize};'
  + 'noble-kernel::async_tasks::domain::Disposition{returned:u64,consumed:u64,retired:u64};'
  + 'noble-kernel::async_tasks::domain::Obligations{inputs:u64,results:u64,buffers:u8}';
function pairExpectation(state, event) {
  const allowed = rule => ({ disposition: 'allowed', rule, error: null });
  const denied = error => ({ disposition: 'rejected', rule: null, error });
  if (event === 'Inspect') return allowed('Inspect');
  if (event.startsWith('Complete')) return allowed(['Pending', 'Retiring'].includes(state) ? 'Complete' : 'Duplicate');
  if (event === 'Deliver') return state === 'Ready' ? allowed('Deliver') : denied(state);
  if (['Cancel', 'Trap', 'Deadline', 'Budget', 'InternalFailure'].includes(event)) return allowed(['Pending', 'Ready'].includes(state) ? 'Retire' : 'Duplicate');
  if (event === 'NativeStopped') return allowed(state === 'Retired' ? 'Duplicate' : 'ObserveStop');
  if (event === 'SettlePins') return state === 'Retired' ? denied('Retired') : allowed('SettlePins');
  if (event === 'Cleanup') return ['Pending', 'Retired'].includes(state) ? denied(state) : allowed('Cleanup');
  if (event === 'Wake' || event === 'TakeWake') return ['Pending', 'Ready'].includes(state) ? allowed(event) : denied(state);
  if (event === 'Finish') return ['Pending', 'Ready'].includes(state) ? denied(state) : allowed(state === 'Retired' ? 'Duplicate' : 'Finish');
  throw Error(`unreviewed event ${event}`);
}
function assertMatrix(report, name) {
  const matrix = report.lifecycle;
  assert.equal(matrix.state_schema, stateSchema); assert.equal(matrix.event_schema, eventSchema);
  assert.deepEqual(matrix.states, lifecycleStates); assert.deepEqual(matrix.events, lifecycleEvents);
  assert.equal(matrix.pairs.length, lifecycleStates.length * lifecycleEvents.length);
  const seen = new Set();
  for (const pair of matrix.pairs) {
    assert.ok(lifecycleStates.includes(pair.state) && lifecycleEvents.includes(pair.event));
    const key = `${pair.state}/${pair.event}`; assert.ok(!seen.has(key), `duplicate observed pair ${key}`); seen.add(key);
    const expected = pairExpectation(pair.state, pair.event);
    assert.equal(pair.disposition, expected.disposition); assert.equal(pair.error, expected.error);
    if (expected.rule === null) assert.equal(pair.rule, null);
    else {
      assert.ok(pair.rule === expected.rule || pair.rule.startsWith(`${expected.rule}(`) || pair.rule.startsWith(`${expected.rule} {`), `wrong observed rule ${key}`);
      if (expected.rule === 'Complete') assert.match(pair.rule, new RegExp(`outcome: ${pair.event === 'CompleteSuccess' ? 'Success' : 'DomainError'}[, }]`));
      if (expected.rule === 'Retire') assert.equal(pair.rule, `Retire(${{
        Cancel: 'Cancelled', Trap: 'Trap', Deadline: 'Deadline', Budget: 'Budget', InternalFailure: 'Internal',
      }[pair.event]})`);
      if (expected.rule === 'SettlePins') assert.equal(pair.rule, 'SettlePins(0)');
    }
  }
  for (const state of lifecycleStates) for (const event of lifecycleEvents) assert.ok(seen.has(`${state}/${event}`));
  if (name === 'lifecycle-matrix') {
    assert.deepEqual(matrix.validation, { status: 'accepted' });
    assert.deepEqual(matrix.submitted_pairs, matrix.pairs); assert.equal(matrix.submitted_state_schema, stateSchema); assert.equal(matrix.submitted_event_schema, eventSchema);
  } else {
    const errors = { 'lifecycle-changed-state-schema': 'StateSchema', 'lifecycle-changed-event-schema': 'EventSchema',
      'lifecycle-missing-pair': 'MissingPair', 'lifecycle-duplicate-pair': 'DuplicatePair', 'lifecycle-invalid-pair': 'WrongDisposition',
      'lifecycle-invented-state': 'StateSchema', 'lifecycle-invented-event': 'EventSchema', 'lifecycle-invented-disposition': 'WrongDisposition' };
    assert.deepEqual(matrix.validation, { status: 'rejected', error: errors[name] });
    if (errors[name] === 'StateSchema') {
      assert.equal(matrix.submitted_state_schema, name === 'lifecycle-invented-state' ? stateSchema.replace('Pending', 'InventedPending') : `${stateSchema}@foreign-revision`);
      assert.equal(matrix.submitted_event_schema, eventSchema); assert.deepEqual(matrix.submitted_pairs, matrix.pairs);
    } else if (errors[name] === 'EventSchema') {
      assert.equal(matrix.submitted_event_schema, name === 'lifecycle-invented-event' ? eventSchema.replace('TakeWake', 'InventedEvent') : `${eventSchema}@foreign-revision`);
      assert.equal(matrix.submitted_state_schema, stateSchema); assert.deepEqual(matrix.submitted_pairs, matrix.pairs);
    } else if (errors[name] === 'MissingPair') assert.deepEqual(matrix.submitted_pairs, matrix.pairs.slice(0, -1));
    else if (errors[name] === 'DuplicatePair') {
      const submitted = [...matrix.pairs]; submitted[1] = submitted[0]; assert.deepEqual(matrix.submitted_pairs, submitted);
    } else {
      const submitted = matrix.pairs.map(pair => {
        if (name === 'lifecycle-invalid-pair' && pair.state === 'Pending' && pair.event === 'Deliver') return { ...pair, disposition: 'allowed', rule: 'Deliver', error: null };
        if (name === 'lifecycle-invented-disposition' && pair.state === 'Pending' && pair.event === 'Inspect') return { ...pair, disposition: 'rejected', rule: null, error: 'InvalidRequest' };
        return pair;
      });
      assert.deepEqual(matrix.submitted_pairs, submitted);
    }
  }
}
function runLifecycleControls() {
  for (const name of ['lifecycle-matrix', ...hostileSchemaCases]) control(name, row => {
    const report = peerControl(row, '--lifecycle', name); assert.equal(report.case, name);
    assertMatrix(report, name); cleanHost(report.host);
    row.scope = 'schema-bound production classifier constructor coverage; payload/data invariants are exercised by separate retained-table controls';
  });
  for (const name of lifecycleCases) control(`lifecycle/${name}`, row => {
    const report = peerControl(row, '--lifecycle', name);
    row.observed_boundary = assertLifecycle(name, report);
  });
  runCase('WORKER-08', reviewedWorker.input.variants.map(item => item.case), row => {
    for (const item of reviewedWorker.input.variants) variant(row, item.case, child => {
      const report = peerControl(child, '--lifecycle', item.case);
      child.observed_boundary = assertLifecycle(item.case, report);
      child.scope = 'the named snapshot is the requested race/admission boundary; mandatory subsequent retirement is reported separately; unrelated filler/previous-generation deliveries are not attributed to the rejected task';
    });
  });
}

try {
  for (const directory of ['crates', 'tools', 'verification/m6/peer/src', 'verification/m6/cases', 'verification/m5/peer/src', 'verification/mc2/contracts']) sourceTree(directory);
  if (fs.existsSync(path.join(root, '.cargo'))) sourceTree('.cargo');
  for (const file of ['Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', 'flake.nix', 'flake.lock', 'policy/tool-selection.json',
    'verification/m6/pins.json', 'verification/m6/build.mjs', 'verification/m6/gate.mjs', 'verification/m6/peer/Cargo.toml', 'verification/m6/peer/Cargo.lock',
    'verification/m5/pins.json', 'verification/m5/peer/Cargo.toml', 'verification/m5/peer/Cargo.lock',
    'verification/mc1/monotonic.noble-contract', 'verification/mc1/monotonic-refutation.lean',
    'specs/conformance/wit-wasi-cases.json', 'specs/conformance/worker-cases.json']) source(file);
  pins = JSON.parse(sourceBytes('verification/m6/pins.json')); selection = JSON.parse(sourceBytes('policy/tool-selection.json'));
  receipt.pins = pins;
  for (const [file, reviewed] of [['specs/conformance/wit-wasi-cases.json', reviewedWorkloads], ['specs/conformance/worker-cases.json', [reviewedWorker]]]) {
    const document = JSON.parse(sourceBytes(file));
    assert.equal(new Set(document.cases.map(item => item.id)).size, document.cases.length, 'duplicate canonical workload');
    const projection = reviewed.map(({ id }) => {
      const item = document.cases.find(item => item.id === id); assert.ok(item, `missing ${id}`); cases.set(id, item);
      return { id, input: item.input, expected: item.expected };
    });
    receipt.conformance.push({ path: file, byte_sha256: receipt.sources[file].sha256, workload_sha256: digest(projection),
      workload: projection, identity_for_workload: 'complete-selected-id-input-expected-projection; state/evidence-not-workload' });
    assert.deepEqual(projection, reviewed, `declared workload changed: ${file}; review every new variant instead of reusing narrower evidence`);
  }
  receipt.source_revision = `sha256:${digest(Object.entries(receipt.sources).map(([file, value]) => [file, value.sha256]).sort())}`;
  for (const [name, supplied] of [['cli', args[0]], ['peer', args[1]], ['task_tests', args[2]], ['node', process.execPath],
    ['wasm_tools', path.join(selection.tool_paths.wasm_tools.output, 'bin/wasm-tools')], ['nix', pins.nix ?? '/run/current-system/sw/bin/nix']]) {
    receipt.executables[name] = freeze(supplied, `executables/${name}`, true);
  }
  control('current-source-build-provenance', row => loadBuild(row, args[5]));
  control('selected-tools-and-engine-closures', row => {
    assert.equal(receipt.executables.node.sha256, hash(fs.readFileSync(path.join(selection.tool_paths.node.output, 'bin/node'))));
    assert.deepEqual(process.execArgv, []); assert.equal(process.versions.node, selection.verification_tools.node.version);
    const nodeVersion = command('node-version', 'node', ['--version']); attach(row, nodeVersion); completed(nodeVersion);
    assert.equal(nodeVersion.text.trim(), `v${selection.verification_tools.node.version}`);
    const toolsVersion = command('wasm-tools-version', 'wasm_tools', ['--version']); attach(row, toolsVersion); completed(toolsVersion);
    assert.equal(toolsVersion.text.trim(), `wasm-tools ${pins.wasm_tools_version}`);
    const closures = [['engine', pins.wasmtime_source, pins.wasmtime_source_nar_hash], ['vendor', pins.vendor, pins.vendor_nar_hash],
      ...(pins.extra_sources ?? []).map(item => [item.name, item.path, item.nar_hash])];
    assert.equal(new Set(closures.map(([name]) => name)).size, closures.length, 'duplicate immutable closure pin');
    for (const match of sourceBytes('verification/m6/peer/Cargo.toml').toString('utf8').matchAll(/path\s*=\s*"(\/nix\/store\/[^"]+)"/g)) {
      assert.ok(closures.some(([, directory]) => match[1] === directory || match[1].startsWith(`${directory}/`)), `unbound compiled dependency: ${match[1]}`);
    }
    for (const [name, directory, expected] of closures) {
      const observed = command(`${name}-nar-hash`, 'nix', ['--extra-experimental-features', 'nix-command', 'hash', 'path', '--type', 'sha256', '--sri', directory]);
      attach(row, observed); completed(observed); assert.equal(observed.text.trim(), expected);
    }
  });
  control('native-task-test-lane', nativeTests);
  control('selected-async-component', row => {
    assert.equal(pins.wit, 'crates/noble-wasm/wit/async.wit');
    const wit = sourceBytes(pins.wit);
    const bindings = bind(row, wit, 'bootstrap'); assert.equal(bindings.world, 'noble-test:async-boundary/bootstrap@1.0.0');
    bootstrap = compile(row, 'selected-bootstrap', wit, 'bootstrap', Object.fromEntries(
      ['order', 'future-result', 'stream-result', 'future-cancel', 'stream-close'].map(name => [name, fixture(`${name}.noble`)])));
  });
  runCase('WI-11', ['pending-first-completes-before-second-enters'], row => variant(row, row.required_variants[0], child => {
    const report = peerRun(child, requireBootstrap(), 'order', 'success');
    assert.deepEqual(report.observation.results, [integer(42)]);
    const events = report.observation.events;
    assert.ok(Array.isArray(events));
    const order = events.filter(event => ['enter-first', 'suspend-first', 'resume-first', 'complete-first', 'enter-second', 'suspend-second', 'resume-second', 'complete-second'].includes(event));
    assert.deepEqual(order, ['enter-first', 'suspend-first', 'resume-first', 'complete-first', 'enter-second', 'suspend-second', 'resume-second', 'complete-second']);
    assert.ok(events.indexOf('enter-second') > events.indexOf('complete-first'));
    assert.ok(events.slice(events.indexOf('suspend-first') + 1, events.indexOf('resume-first')).includes('native-complete'), 'first import must actually suspend until native completion');
    assert.equal(report.host.authority.counters.witness_consumptions, 1); assert.equal(report.host.authority.counters.protected_operations, 1);
    assert.equal(report.host.authority.counters.successful_deliveries, 1); assert.equal(report.host.authority.invocation, 'Succeeded');
    operationReceipt(report.host, { claim: 'OperationSuccess', invocation: 'Pending', operation: 'noble-test:async-boundary/host@1.0.0#first', argument: 40 });
    assert.equal(report.host.authority.receipts.filter(item => item.claim === 'InvocationSuccess').length, 1);
    child.observed_events = events;
  }));
  runCase('WI-12', ['future', 'stream'].flatMap(kind => terminalCases.map(scenario => `${kind}/${scenario}`)), row => {
    for (const kind of ['future', 'stream']) for (const scenario of terminalCases) variant(row, `${kind}/${scenario}`, child => {
      const cancelled = ['cancel-before-completion', 'late-completion', 'ready-cancel'].includes(scenario);
      const report = peerRun(child, requireBootstrap(), `${kind}-result`, scenario, cancelled ? 'cancelled' : 'normal');
      if (!cancelled) assert.deepEqual(report.observation.results, terminalResult(kind, scenario));
      assert.equal(report.host.accounting.admissions, 1); assert.equal(report.host.accounting.completions, 1);
      assert.equal(report.host.accounting.deliveries, cancelled ? 0 : 1);
      assert.equal(report.host.accounting.cancellations, cancelled ? 1 : 0);
      const completion = report.host.events.filter(event => event.operation === 'complete' && event.snapshot);
      assert.equal(completion.length, 1);
      assert.equal(completion[0].snapshot.outcome, scenario === 'domain-error' ? 'DomainError' : 'Success');
      if (cancelled) {
        const cancellation = report.host.events.find(event => event.action === 'CancellationAcknowledged');
        assert.ok(cancellation, 'must observe the actual cancellation decision');
        assert.equal(cancellation.snapshot.state, 'Retiring'); assert.equal(cancellation.snapshot.failure, 'Cancelled');
        if (scenario !== 'ready-cancel') {
          assert.equal(cancellation.snapshot.pins, 1); assert.equal(cancellation.snapshot.native_stopped, false);
          assert.equal(completion[0].snapshot.state, 'Retiring'); assert.equal(completion[0].snapshot.failure, 'Cancelled');
        }
        if (scenario === 'cancel-before-completion') {
          assert.equal(report.native_pins_at_store_drop, 1);
          assert.ok(report.guest_stop_ms < report.elapsed_ms, 'guest access must stop before the independently joined native worker');
        }
      }
      if (scenario === 'delivery-before-cancel') {
        const cancellation = report.host.events.find(event => event.operation === 'cancel');
        assert.equal(cancellation?.action, 'Duplicate', 'post-delivery cancellation cannot undo published ownership');
      }
      const events = report.observation.events;
      if (['cancel-before-completion', 'late-completion'].includes(scenario)) {
        assert.ok(events.includes('cancel-ack') && events.includes('native-complete'));
        assert.ok(events.indexOf('cancel-ack') < events.indexOf('native-complete'), 'must observe cancellation before authentic late completion');
      }
      if (scenario === 'ready-cancel') assert.ok(events.indexOf('native-complete') >= 0 && events.indexOf('cancel-ready-ack') > events.indexOf('native-complete'));
      child.terminal_channel = kind === 'stream' ? 'producer terminal result<list<u8>,string>; stream bytes are not errors' : 'future<result<s64,string>>';
    });
  });
  for (const type of liveTypes) control(`eligibility-positive/${type}`, row => {
    const body = type === 'future<s64>' ? 'host.make-future host.finish-future'
      : type === 'stream<u8>' ? 'host.make-stream host.close-stream 42'
      : '"tag" host.make-stream pair unpair host.close-stream drop 42';
    const compiled = compile(row, `eligibility-positive-${liveTypes.indexOf(type)}`, fixture('eligibility.wit'), 'demo', { check: `${body}\n` });
    const report = peerRun(row, compiled, 'check', 'success');
    assert.deepEqual(report.observation.results, [{ type: 'I64', value: '42' }]);
    if (type.startsWith('Pair')) {
      assert.ok(report.observation.events.includes('close-stream'));
      row.scope = 'actual Noble Pair construction, unpair and typed live-stream close execute in the independent native component peer; WIT tuple support is not claimed';
    }
  });
  runCase('WI-16', liveTypes.flatMap(type => liveOperations.map(operation => `${type}/${operation}`)), row => {
    for (const type of liveTypes) for (const operation of liveOperations) variant(row, `${type}/${operation}`, child => {
      const wit = fixture('eligibility.wit'); bind(child, wit, 'demo');
      const prefix = type === 'future<s64>' ? 'host.make-future' : type === 'stream<u8>' ? 'host.make-stream' : '"tag" host.make-stream pair';
      const word = operation === 'generic-drop' ? 'drop' : operation === 'generic-serialization' ? 'quote reflect' : operation;
      const report = compile(child, `eligibility-${liveTypes.indexOf(type)}-${operation}`, wit, 'demo', { check: `${prefix} ${word}\n` }, true);
      child.observed_refusal = report.diagnostic;
      assert.equal(receipt.controls.find(control => control.name === `eligibility-positive/${type}`)?.result, 'passed',
        'eligibility rejection needs the independently admitted positive live-value construction');
      if (operation === 'generic-serialization') child.scope = 'the actual quote-then-reflect serialization route refuses live capture before inert serializable Syntax exists';
      if (type.startsWith('Pair')) child.construction = 'Noble words push Text, obtain an admitted live stream, and construct Pair before the failing operation; no unsupported WIT tuple is used';
    });
  });
  for (const [name, exportName] of [['explicit-future-cancel', 'future-cancel'], ['explicit-stream-close', 'stream-close']]) control(name, row => {
    const report = peerRun(row, requireBootstrap(), exportName, 'success'); assert.deepEqual(report.observation.results, []);
    assert.ok(report.observation.events.includes(exportName === 'future-cancel' ? 'cancel-future' : 'close-stream'));
    assert.equal(report.host.accounting.admissions, 1); assert.equal(report.host.accounting.completions, 1);
    assert.equal(report.host.accounting.cancellations, exportName === 'future-cancel' ? 1 : 0);
    assert.equal(report.host.accounting.deliveries, exportName === 'future-cancel' ? 0 : 1);
    assert.notEqual(report.host.authority.invocation, 'Cancelled', 'endpoint lifetime is not invocation cancellation');
    if (exportName === 'stream-close') {
      const closure = report.host.events.find(item => item.operation === 'reader-closed');
      assert.equal(closure?.guest_bytes_delivered, 0); assert.equal(closure?.terminal_consumer, 'host');
    }
  });
  control('async-owned-resource-return', row => {
    resourceComponent = compile(row, 'async-resources', fixture('resources.wit'), 'demo', { returned: fixture('returned.noble'), error: fixture('error.noble') });
    const report = peerRun(row, resourceComponent, 'returned', 'success');
    assert.deepEqual(report.observation.results, [{ type: 'Resource', value: '42', disposition: 'peer-released' }]);
    assert.equal(report.host.resources.native_releases, 1);
  });
  control('async-owned-resource-domain-error', row => {
    assert.ok(resourceComponent, 'resource-bearing fixture did not compile');
    const report = peerRun(row, resourceComponent, 'error', 'domain-error');
    assert.deepEqual(report.observation.results, [sum('right', { type: 'Text', value: 'domain-error' })]);
    assert.equal(report.host.resources.native_releases, 1);
  });
  for (const [name, source, denial, admissions, value] of [
    ['denied-admission', 'denied-resource.noble', 'ChangedPlan', 0, 41],
    ['denied-replay', 'trapped-resource.noble', 'ConsumedWitness', 1, 42],
  ]) control(`async-owned-resource-${name}`, row => {
    const compiled = compile(row, `async-resources-${name}`, fixture('resources.wit'), 'demo',
      { returned: fixture(source), error: fixture('error.noble') });
    const report = peerRun(row, compiled, 'returned', 'success', 'trap');
    assert.ok(report.observation.error.includes(denial), `missing ${denial} refusal`);
    assert.deepEqual(report.host.resources.released_values, [value]);
    assert.equal(report.host.resources.native_releases, 1);
    assert.equal(report.host.accounting.admissions, admissions);
    assert.equal(report.host.authority.counters.witness_consumptions, admissions);
    assert.equal(report.host.authority.counters.protected_operations, admissions);
    assert.equal(report.host.authority.counters.successful_deliveries, 0);
    assert.equal(report.host.authority.invocation, 'Failed');
    if (admissions === 1) {
      assert.deepEqual(report.host.resources.returned_values, [42]);
      operationReceipt(report.host, { claim: 'OperationSuccess', invocation: 'Pending',
        operation: 'noble-test:async-resources/counters@1.0.0#transfer', argument: 1 });
    } else {
      assert.deepEqual(report.host.resources.returned_values, []);
      assert.ok(report.host.authority.receipts.every(item => item.claim !== 'OperationSuccess'));
    }
    row.scope = 'compiled guest traps before ownership transfer or after a returned owner; Store teardown retires remaining guest custody without retry or invocation-success promotion';
  });
  control('async-wide-parameters', row => {
    wideComponent = compile(row, 'async-wide', fixture('wide.wit'), 'demo', { sum5: fixture('sum5.noble') });
    const report = peerRun(row, wideComponent, 'sum5', 'success'); assert.deepEqual(report.observation.results, [integer(15)]);
  });
  // Lifecycle, schema, compatibility and progress observations are independent peer lanes.
  // Their explicit inventories and assertions below must all execute for acceptance.
  runLifecycleControls();
  for (const mode of ['native-enabled', 'async-disabled', 'stackful-disabled', 'wasi-p3-clock', 'wasi-stable-version-rejected']) control(`compatibility-${mode}`, row => {
    const report = peerControl(row, '--compatibility', mode, requireBootstrap());
    const observed = report.compatibility;
    assert.equal(observed.mode, mode);
    if (mode === 'native-enabled') {
      assert.equal(observed.outcome, 'accepted'); assert.equal(observed.stage, 'engine-component-admission'); assert.equal(observed.imports_started, 0);
    } else if (mode === 'wasi-p3-clock') {
      assert.equal(observed.outcome, 'normal'); assert.equal(observed.stage, 'native-component-execution'); assert.equal(observed.imports_started, 1);
      assert.equal(report.wasi_version, '0.3.0-rc-2025-09-16'); assert.equal(report.operation, 'wasi:clocks/monotonic-clock@0.3.0-rc-2025-09-16#wait-for');
      assert.equal(report.requested_ns, 1_000_000); assert.ok(report.elapsed_ns >= report.requested_ns); assert.equal(report.task_exit_observed, true);
    } else {
      assert.equal(observed.outcome, 'rejected'); assert.ok(typeof observed.diagnostic === 'string' && observed.diagnostic.length > 0); assert.equal(observed.imports_started, 0);
      assert.equal(observed.stage, mode === 'wasi-stable-version-rejected' ? 'link' : 'engine-component-admission');
      if (mode === 'wasi-stable-version-rejected') { assert.equal(report.wasi_version, '0.3.0-rc-2025-09-16'); assert.equal(report.requested_wasi_version, '0.3.0'); }
    }
  });
  for (const mode of ['runnable-fuel', 'runnable-epoch', 'blocking-deadline']) control(`progress-${mode}`, row => {
    const report = peerControl(row, '--progress', mode);
    const observed = report.progress;
    assert.equal(observed.case, mode);
    assert.equal(observed.outcome, mode === 'runnable-fuel' ? 'budget' : 'deadline');
    assert.ok(Number.isFinite(observed.elapsed_ms) && observed.elapsed_ms >= 0);
    assert.ok(observed.heartbeat_progress > 0, 'cooperative executor heartbeat must make measured progress');
    assert.ok(observed.guest_budget > 0); assert.ok(observed.fuel_quantum > 0 && observed.fuel_quantum < observed.guest_budget);
    assert.ok(observed.poll_count > 0); assert.ok(observed.longest_poll_ns >= 0 && observed.longest_poll_ns < 100_000_000);
    if (mode === 'runnable-fuel') { assert.equal(observed.guest_work_remaining, 0); assert.match(observed.diagnostic, /fuel/); }
    else {
      assert.ok(observed.guest_work_remaining > 0, 'deadline/epoch must stop before guest fuel exhaustion');
      if (mode === 'runnable-epoch') assert.match(observed.diagnostic, /interrupt/);
    }
    assert.ok(observed.cancel_ack_ms >= 0 && observed.cancel_ack_ms < observed.native_complete_ms, 'interruption acknowledgement must precede native stop');
    assert.equal(observed.native_pins_at_ack, 1); assert.equal(observed.native_pins_after_join, 0);
    assert.equal(report.at_ack.tasks.retiring, 1); assert.equal(report.at_ack.tasks.outstanding_pins, 1);
    assert.equal(report.at_ack.accounting.deliveries, 0);
    assert.equal(report.native_jobs_joined, true); assert.equal(report.host_retained_after_store_drop, true);
    row.scope = 'measured interruption and executor progress within this bounded probe; guest_work_remaining is observed fuel, not a proof of remaining semantic work or an OS scheduling bound';
    cleanHost(report.host);
  });
} catch (error) {
  receipt.failure = String(error.stack ?? error);
} finally {
  try {
    for (const directory of fs.readdirSync(path.join(artifacts, 'components'), { withFileTypes: true })) {
      assert.ok(directory.isDirectory(), 'unexpected component root output'); retainComponent(path.join(artifacts, 'components', directory.name));
    }
  } catch (error) { receipt.integrity_failures.push({ phase: 'retaining-partial-compiler-output', error: String(error) }); }
  for (const binding of monitored) {
    try { assert.equal(hash(fs.readFileSync(binding.file)), binding.sha256); }
    catch (error) { receipt.integrity_failures.push({ file: binding.file, error: String(error) }); }
  }
  for (const inventory of receipt.source_trees) {
    try { assert.deepEqual(entries(inventory.directory), inventory.files); }
    catch (error) { receipt.integrity_failures.push({ directory: inventory.directory, error: String(error) }); }
  }
  const executedCases = receipt.cases.map(row => row.id), executedControls = receipt.controls.map(row => row.name);
  receipt.summary = { required_cases: expectedCases, executed_cases: executedCases, missing_cases: expectedCases.filter(id => !executedCases.includes(id)),
    passed_cases: receipt.cases.filter(row => row.result === 'passed').map(row => row.id),
    failed_cases: receipt.cases.filter(row => row.result !== 'passed').map(row => row.id),
    required_controls: requiredControls, executed_controls: executedControls, missing_controls: requiredControls.filter(name => !executedControls.includes(name)),
    failed_controls: receipt.controls.filter(row => row.result !== 'passed').map(row => row.name),
    executed_variants: receipt.cases.reduce((count, row) => count + row.variants.length, 0),
    passed_variants: receipt.cases.reduce((count, row) => count + row.variants.filter(item => item.result === 'passed').length, 0),
    native_tests: receipt.native_tests.map(row => ({ name: row.name, result: row.result })) };
  receipt.passed = !receipt.failure && !receipt.gaps.length && !receipt.integrity_failures.length
    && !receipt.summary.missing_cases.length && !receipt.summary.missing_controls.length
    && !receipt.summary.failed_cases.length && !receipt.summary.failed_controls.length
    && receipt.native_tests.length > 0 && receipt.native_tests.every(row => row.result === 'passed');
  receipt.result = receipt.passed ? 'passed' : 'failed'; save(); fs.chmodSync(path.join(artifacts, 'report.json'), 0o400);
  console.log(JSON.stringify({ schema: receipt.schema, result: receipt.result, report: path.join(artifacts, 'report.json'), summary: receipt.summary }));
  process.exitCode = receipt.passed ? 0 : 1;
}
