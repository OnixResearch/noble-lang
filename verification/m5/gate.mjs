#!/usr/bin/env node
// Bounded, source/workload/binary-bound M5 execution evidence, not a proof.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const args = process.argv.slice(2);
if (args.length < 5 || (args.length !== 5 && !(args.length === 7 && args[5] === '--opacity-report'))) {
  throw Error('usage: NODE verification/m5/gate.mjs CLI PEER RESOURCE_TEST_BINARY AUTHORITY_TEST_BINARY NEW_ARTIFACT_DIR [--opacity-report REPORT_JSON]');
}
const artifacts = path.resolve(args[4]);
fs.mkdirSync(artifacts);
for (const directory of ['sources', 'executables', 'inputs', 'commands', 'components', 'external']) {
  fs.mkdirSync(path.join(artifacts, directory));
}
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const digest = value => hash(Buffer.from(JSON.stringify(value)));
const relative = file => path.relative(artifacts, file).split(path.sep).join('/');
const receipt = {
  schema: 'noble-m5-acceptance/v1', result: 'running', profile: 'Component-Sync-Bootstrap',
  artifact_directory: artifacts, source_root: root,
  engine: { executable: process.execPath, versions: process.versions, flags: process.execArgv },
  sources: {}, executables: {}, retained: {}, conformance: [], commands: [], native_tests: [],
  cases: [], controls: [], gaps: [], integrity_failures: [],
  assumptions: [
    'Rust, the supplied native executables, the pinned wasm-tools, Bash/prlimit quota-probe tools, Node/V8, Wasmtime, the filesystem and this gate are trusted execution tooling.',
    'Copied source bytes and executable hashes bind the observations; separate build provenance is required to establish that the supplied native binaries were built from these sources.',
    'The Rust peer independently implements canonical component linking and typed conversion, but deliberately reuses the production Noble resource/authority decision libraries for host policy.',
    'Native resource tests concern deterministic ownership and pin accounting. Physical native releases, real credentials and authenticated external observations remain trusted host obligations.',
    'Core hostile-import probes execute unchanged production Wasm. Their diagnostics are core-only instrumentation, never exported WIT or an independent Component Model implementation.',
    'Success-path canonical post-return and explicit trap-path host cleanup are distinct. Core counters do not measure all engine allocations or physical copies.',
  ],
  non_claims: ['Component-Draft or full WASI', 'async/future/stream execution', 'general Program-to-WIT adapters',
    'universal compiler or backend refinement', 'remote exactly-once effects', 'receipt authenticity from a digest',
    'physical guest GC implies resource release', 'source review or a fixture label alone establishes execution',
    'compiler-internal subprocess transcripts are available: the gate retains its own commands and independently reassembles the emitted artifacts'],
};
let serial = 0;
const monitored = [];
const sourceNames = new Set();
function writeNew(name, data, mode = 0o400) {
  const file = path.join(artifacts, name);
  fs.mkdirSync(path.dirname(file), { recursive: true });
  const bytes = typeof data === 'string' || Buffer.isBuffer(data) ? data : JSON.stringify(data, null, 2) + '\n';
  fs.writeFileSync(file, bytes, { flag: 'wx', mode });
  receipt.retained[name] = { sha256: hash(fs.readFileSync(file)), bytes: fs.statSync(file).size };
  monitored.push({ file, sha256: receipt.retained[name].sha256 });
  return file;
}
function freeze(source, name, executable = false) {
  const supplied = path.resolve(source);
  const real = fs.realpathSync(supplied);
  assert.ok(fs.statSync(real).isFile(), `not a regular input: ${supplied}`);
  const bytes = fs.readFileSync(real);
  const frozen = writeNew(name, bytes, executable ? 0o500 : 0o400);
  monitored.push({ file: real, sha256: hash(bytes) });
  return { supplied, real, frozen, sha256: hash(bytes), bytes: bytes.length };
}
function source(file) {
  if (sourceNames.has(file)) return;
  sourceNames.add(file);
  const binding = freeze(path.join(root, file), `sources/${file}`);
  receipt.sources[file] = { sha256: binding.sha256, snapshot: relative(binding.frozen), bytes: binding.bytes };
}
function sourceTree(directory) {
  for (const item of fs.readdirSync(path.join(root, directory), { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name))) {
    if (['target', '.git', '.lake', 'node_modules'].includes(item.name)) continue;
    const file = `${directory}/${item.name}`;
    if (item.isSymbolicLink()) throw Error(`source symlink requires an explicit binding: ${file}`);
    if (item.isDirectory()) sourceTree(file);
    else if (item.isFile()) source(file);
  }
}
function command(label, executable, argv, input = '', timeout = 180_000) {
  const id = ++serial;
  const stem = `commands/${String(id).padStart(4, '0')}-${label.replace(/[^A-Za-z0-9_.-]/g, '-')}`;
  const stdin = writeNew(`${stem}.stdin`, input);
  const result = spawnSync(executable, argv, { cwd: root, input, encoding: 'utf8', timeout,
    killSignal: 'SIGKILL', maxBuffer: 32 * 1024 * 1024, env: { PATH: '/nonexistent', LANG: 'C', LC_ALL: 'C', RUST_BACKTRACE: '1' } });
  const stdout = writeNew(`${stem}.stdout`, result.stdout ?? '');
  const stderr = writeNew(`${stem}.stderr`, result.stderr ?? '');
  const entry = { id, label, executable, executable_sha256: hash(fs.readFileSync(executable)), arguments: argv,
    cwd: root, environment: { PATH: '/nonexistent', LANG: 'C', LC_ALL: 'C', RUST_BACKTRACE: '1' },
    timeout_ms: timeout, stdin: relative(stdin), stdout: relative(stdout), stderr: relative(stderr),
    status: result.status, signal: result.signal, error: result.error ? String(result.error) : null };
  receipt.commands.push(entry);
  return { entry, text: result.stdout ?? '', stderr: result.stderr ?? '' };
}
function completed(run, status = 0) {
  assert.equal(run.entry.error, null, `command ${run.entry.id} host failure`);
  assert.equal(run.entry.signal, null, `command ${run.entry.id} terminated by signal`);
  assert.equal(run.entry.status, status, `command ${run.entry.id} exit status; see retained stderr`);
}
function json(run) {
  const lines = run.text.split('\n').filter(line => line.trim());
  assert.equal(lines.length, 1, `command ${run.entry.id} must emit exactly one JSON report`);
  return JSON.parse(lines[0]);
}
function attach(row, run, report) {
  row.evidence.push({ command: run.entry.id, ...(report ? { report } : {}) });
}
function gap(row, message) {
  row.result = 'gap';
  row.gaps ??= [];
  row.gaps.push(message);
  receipt.gaps.push({ case: row.case, variant: row.name, message });
}
function control(name, action) {
  const row = { name, result: 'running', evidence: [] };
  receipt.controls.push(row);
  try { action(row); if (row.result === 'running') row.result = 'passed'; }
  catch (error) { row.result = 'failed'; row.failure = String(error.stack ?? error); }
  return row;
}
const selected = {
  'specs/conformance/resource-cases.json': Array.from({ length: 9 }, (_, i) => `RA-CASE-${String(i + 1).padStart(2, '0')}`),
  'specs/conformance/adaptation-cases.json': ['ADAPT-11'],
  'specs/conformance/wit-wasi-cases.json': ['WI-01', 'WI-02', 'WI-04', 'WI-05', 'WI-06', 'WI-07', 'WI-08', 'WI-09', 'WI-10', 'WI-15'],
  'specs/conformance/octet-adoption-cases.json': ['OCTET-01', 'OCTET-02', 'OCTET-03', 'OCTET-04'],
};
// Any declared input/expectation edit needs a deliberate driver review, not just
// a new digest on observations produced by the old fixture. State/evidence edits
// are intentionally excluded from these complete workload projections.
const supportedWorkloads = {
  'specs/conformance/resource-cases.json': '474543505b8d86983c1901e9bcd514fa88e7c74a7ffd29f4b6fe5db03d4cec73',
  'specs/conformance/adaptation-cases.json': '447c001479d6bb3c3f58783adad42aeb1fff111acd983d548e63c86c87a62a72',
  'specs/conformance/wit-wasi-cases.json': 'a37e2a9d61d84f93f7e284770abb591fe1fca01f7331125dcf1cc94f4cd969b2',
  'specs/conformance/octet-adoption-cases.json': '34c38f62a79f0dca406a22943162a0d73850ff8faad4dbf7e2dde72721ba645e',
};
const cases = new Map();
let cli, peer, resourceBinary, authorityBinary, node, wasmTools;
const native = new Map();
let bootstrap;
let opacity;
let resourceControl;
const mathWit = 'package noble-test:math@1.0.0; world demo { export inc: func(x: s64) -> s64; }';
const mathImportWit = 'package noble-test:math@1.0.0; interface arithmetic { inc: func(x: s64) -> s64; } world demo { import arithmetic; export inc: func(x: s64) -> s64; }';
const resourceWit = (parameters = 'value: own<counter>', result = 'own<counter>') => `package noble-test:counter@1.0.0; interface counters { resource counter { read: func() -> result<s64, string>; } } world demo { use counters.{counter}; import counters; export check: func(${parameters})${result ? ` -> ${result}` : ''}; }`;
const witnessWit = (parameters, result = '') => `package noble-test:sync@1.0.0; interface authorization { resource authorization; } world demo { use authorization.{authorization}; import authorization; export check: func(${parameters})${result ? ` -> ${result}` : ''}; }`;
function input(name, bytes) { return writeNew(`inputs/${++serial}-${name}`, bytes); }
function bindings(row, wit, world = 'demo') {
  const witPath = input('bindings.wit', wit);
  const run = command('bindings', cli, ['component', 'bindings', witPath, world]);
  const value = json(run); attach(row, run, value); completed(run);
  assert.equal(value.schema, 'noble-component/v1');
  assert.equal(value.outcome, 'typed-bindings');
  assert.equal(value.component_emitted, false);
  return value;
}
function compile(row, label, wit, exports, { world = 'demo', refusal = null } = {}) {
  const witPath = input(`${label}.wit`, wit);
  const sources = Object.entries(exports).map(([name, body]) => ({ name, body, path: input(`${label}-${name}.noble`, body) }));
  const directory = path.join(artifacts, 'components', `${++serial}-${label}`);
  const run = command(`${label}-compile`, cli, ['component', 'compile', witPath, world, directory,
    ...sources.map(item => `${item.name}=${item.path}`)]);
  const value = json(run); attach(row, run, value);
  assert.equal(value.schema, 'noble-component/v1');
  if (refusal) {
    completed(run, refusal === 'unsupported' ? 4 : 2);
    assert.equal(value.outcome, refusal);
    assert.equal(value.component_emitted, false);
    assert.equal(fs.existsSync(directory), false, 'refusal must precede component emission');
    assert.equal(value.diagnostic?.code, 'component-check', 'refusal must arise from source/WIT checking, not tool failure');
    row.observed_refusal = value.diagnostic;
    return value;
  }
  completed(run);
  assert.equal(value.outcome, 'compiled');
  assert.equal(value.component_emitted, true);
  assert.equal(value.independent_kernel_check, true);
  assert.equal(value.abi, 'wasm-tools-1.245.1-sync-memory32-utf8');
  const names = fs.readdirSync(directory).sort();
  assert.deepEqual(names, ['component.wasm', 'core.wasm', 'module.wat', 'report.json', 'world.wit',
    ...sources.map((_, i) => `export-${i}.noble`)].sort());
  for (const name of names) {
    const file = path.join(directory, name);
    assert.ok(fs.lstatSync(file).isFile(), 'compiler emitted a non-file artifact');
    const bytes = fs.readFileSync(file);
    fs.chmodSync(file, 0o400);
    receipt.retained[relative(file)] = { sha256: hash(bytes), bytes: bytes.length };
    monitored.push({ file, sha256: hash(bytes) });
  }
  assert.deepEqual(fs.readFileSync(path.join(directory, 'world.wit')), Buffer.from(wit));
  sources.forEach((item, index) => assert.deepEqual(fs.readFileSync(path.join(directory, `export-${index}.noble`)), Buffer.from(item.body)));
  assert.deepEqual(JSON.parse(fs.readFileSync(path.join(directory, 'report.json'))), value);
  const component = path.join(directory, 'component.wasm');
  const core = path.join(directory, 'core.wasm');
  const validate = command(`${label}-component-validate`, wasmTools, ['validate', component]);
  attach(row, validate); completed(validate);
  const inspect = command(`${label}-component-interface`, wasmTools, ['component', 'wit', component]);
  attach(row, inspect); completed(inspect);
  const actualExports = [...inspect.text.matchAll(/^\s*export\s+([^\s:]+)\s*:/gm)].map(match => match[1]).sort();
  assert.deepEqual(actualExports, sources.map(item => item.name).sort(), 'actual component interface must have exactly the selected WIT exports');
  row.evidence.push({ component: relative(component), core: relative(core), actual_component_exports: actualExports });
  return { component, core, directory, report: value, witPath, sources };
}
function bootstrapComponent(row) {
  if (!bootstrap) {
    const wit = fs.readFileSync(path.join(artifacts, 'sources/crates/noble-wasm/wit/bootstrap.wit'));
    const own = { name: 'compiled-pinned-bootstrap-world', evidence: [] };
    bootstrap = compile(own, 'bootstrap', wit, {
      inc: 'arithmetic.inc', 'echo-text': 'echo.text', 'echo-bytes': 'echo.bytes',
      'read-counter': 'counter.read drop', 'transfer-counter': 'counters.transfer',
      protected: 'dup authorization.prepare swap authorization.protected',
    }, { world: 'bootstrap' });
    receipt.controls.push({ ...own, result: 'passed' });
  }
  row.evidence.push({ component: relative(bootstrap.component), core: relative(bootstrap.core), control: 'compiled-pinned-bootstrap-world' });
  return bootstrap;
}
function peerRun(row, compiled, exportName, arguments_, expected = 'normal', options = {}) {
  const payload = Object.keys(options).length ? { arguments: arguments_, ...options } : arguments_;
  const run = command(`peer-${exportName}`, peer, [compiled.component, exportName], JSON.stringify(payload) + '\n');
  const value = json(run); attach(row, run, value); completed(run);
  assert.equal(value.schema, 'noble-m5-peer/v1');
  assert.equal(value.engine, 'wasmtime-40.0.2');
  assert.equal(value.observation.outcome, expected);
  assert.equal(value.cleanup, 'store-dropped-before-report');
  assert.equal(value.host.resources.live, 0);
  assert.equal(value.host.resources.busy, 0);
  assert.equal(value.host.resources.retiring, 0);
  assert.equal(value.host.resources.native_pins, 0);
  if (expected === 'normal') assert.equal(value.observation.post_return, true);
  else {
    assert.deepEqual(value.observation.results, []);
    assert.equal(value.observation.partial_trusted_values_published, 0);
  }
  return value;
}
function nativeEvidence(row, kind, name, scope) {
  const result = native.get(`${kind}:${name}`);
  assert.ok(result, `required native behavior test was not discovered: ${kind}:${name}`);
  row.evidence.push({ native_test: `${kind}:${name}`, command: result.command, scope });
  assert.equal(result.result, 'passed', `native behavior test failed: ${name}`);
}
function runNative(kind, executable, expectedCount) {
  const listing = command(`${kind}-test-list`, executable, ['--list', '--format', 'terse']);
  completed(listing);
  const names = listing.text.split('\n').filter(line => line.endsWith(': test')).map(line => line.slice(0, -6)).sort();
  assert.equal(names.length, expectedCount, `${kind} behavior test inventory changed; update coverage explicitly`);
  assert.equal(new Set(names).size, names.length);
  receipt.controls.push({ name: `${kind}-native-inventory`, result: 'passed', evidence: [{ command: listing.entry.id }], tests: names });
  for (const name of names) {
    const run = command(`${kind}-${name}`, executable, ['--exact', name, '--nocapture', '--test-threads=1']);
    const row = { kind, name, command: run.entry.id, result: 'running' };
    try {
      completed(run);
      assert.ok(run.text.includes(`test ${name} ... ok`), 'exact requested test did not pass');
      assert.match(run.text, /test result: ok\. 1 passed; 0 failed; 0 ignored; 0 measured;/);
      row.result = 'passed';
    } catch (error) { row.result = 'failed'; row.failure = String(error); }
    native.set(`${kind}:${name}`, row); receipt.native_tests.push(row);
  }
}
function runResourceControl(row) {
  const run = command('peer-cancellation-gc', peer, ['--resource-control', 'cancellation-gc']);
  const value = json(run); attach(row, run, value); completed(run);
  assert.equal(value.schema, 'noble-m5-resource-control/v1');
  assert.equal(value.engine, 'wasmtime-40.0.2');
  assert.equal(value.collector, 'deferred-reference-counting');
  assert.equal(value.guest_payload, 'nonauthority-handle-description');
  assert.equal(value.profile, 'Component-Sync-Bootstrap');
  assert.equal(value.execution, 'bounded-single-threaded-local-control');
  for (const scenario of [value.cancellation, value.unexpected_suspension]) {
    assert.equal(scenario.observation_boundary, 'before-store-drop');
    assert.equal(scenario.guest_visible_before_revocation, 1);
    exact(scenario.steps.map(step => step.step), ['busy', 'revoked', 'after-gc', 'callback', 'duplicate-callback', 'retire-again']);
    exact(scenario.collections.map(collection => collection.phase), ['guest-reachable', 'after-revocation']);
    for (const [index, collection] of scenario.collections.entries()) {
      assert.equal(collection.api, 'Store::gc(None)');
      assert.equal(collection.observation_boundary, 'before-store-drop');
      assert.equal(collection.finalizers_before, 0);
      assert.equal(collection.finalizers_after, index);
      assert.equal(collection.finalized_during_collection, index);
    }
    const [busy, revoked, collected, ...completedSteps] = scenario.steps;
    assert.equal(busy.final_state, 'Busy(call-1)');
    assert.equal(busy.native_pins, 1);
    assert.equal(busy.guest_reference_present, true);
    assert.equal(busy.guest_finalizers, 0);
    for (const step of [revoked, collected]) {
      assert.equal(step.final_state, 'Retiring(call-1)');
      assert.equal(step.guest_access, false);
      assert.equal(step.native_access, false);
      assert.equal(step.guest_reference_present, false);
      assert.equal(step.native_pins, 1);
      assert.equal(step.callback_pin_retained, true);
      assert.equal(step.native_storage_alive, true);
      assert.ok(step.native_strong_references > 0);
      assert.equal(step.local_release_count, 0);
      assert.equal(step.native_release_count, 0);
      assert.equal(step.returned_owner_count, 0);
      assert.equal(step.fully_cleaned_up, false);
    }
    assert.equal(revoked.guest_finalizers, 0);
    assert.equal(collected.guest_finalizers, 1);
    assert.equal(collected.gc_collections, 2);
    for (const [index, step] of completedSteps.entries()) {
      assert.equal(step.final_state, 'Retired');
      assert.equal(step.native_pins, 0);
      assert.equal(step.callback_pin_retained, false);
      assert.equal(step.native_storage_alive, false);
      assert.equal(step.native_strong_references, 0);
      assert.equal(step.local_release_count, 1);
      assert.equal(step.native_release_count, 1);
      assert.equal(step.returned_owner_count, 0);
      assert.equal(step.callback_storage_accesses, 1);
      assert.equal(step.callback_deliveries, index === 0 ? 1 : 2);
      assert.equal(step.fully_cleaned_up, true);
    }
    for (const step of scenario.steps) assert.equal(step.protected_operations, 0);
    assert.equal(scenario.protected_operations, 0);
  }
  exact(value.unexpected_suspension.adapter, {
    declared: 'sync', native_poll: 'Pending', polls: 1, suspension_refused: true,
    boundary: 'single-poll-local-native-adapter',
  });
  resourceControl = { command: run.entry.id, value };
}
function resourceEvidence(row, id) {
  assert.ok(resourceControl, 'real collector/native callback control did not complete');
  const scenario = id === 'RA-CASE-09' ? resourceControl.value.unexpected_suspension : resourceControl.value.cancellation;
  const observed = scenario.observations[id];
  assert.ok(observed, `resource control omitted ${id}`);
  for (const [key, expected] of Object.entries(cases.get(id).expected)) assert.deepEqual(observed[key], expected, `${id}: ${key}`);
  row.evidence.push({ command: resourceControl.command, observation: observed,
    scope: 'actual Wasmtime collection before Store drop, with a nonauthority guest handle description and a separately pinned local native callback; not arbitrary external cancellation or full async support' });
}
function variant(caseRow, name, declared, action) {
  const row = { case: caseRow.id, name, declared, result: 'running', evidence: [] };
  caseRow.variants.push(row);
  try { action(row); if (row.result === 'running') row.result = 'passed'; }
  catch (error) { row.result = 'failed'; row.failure = String(error.stack ?? error); }
  return row;
}
function runCase(id, action) {
  const item = cases.get(id);
  assert.ok(item, `required case absent: ${id}`);
  const row = { id, input: item.input, expected: item.expected, result: 'running', variants: [] };
  receipt.cases.push(row);
  try { action(row, item); assert.ok(row.variants.length, 'case has no variant evidence'); }
  catch (error) { row.failure = String(error.stack ?? error); }
  row.result = row.failure || row.variants.some(v => v.result === 'failed') ? 'failed'
    : row.variants.some(v => v.result === 'gap') ? 'gap' : 'passed';
}
function exact(actual, expected) { assert.deepEqual(actual, expected, 'declared scenario changed; do not silently reuse a narrower driver'); }
function opacityEvidence(row, name) {
  if (!opacity) { gap(row, 'Separate source-bound rustdoc compile-fail execution is required; native runtime binaries cannot establish Rust constructor opacity. Supply --opacity-report.'); return; }
  const entry = opacity.variants.find(item => item.name === name);
  assert.ok(entry, `opacity report omits exact variant ${name}`);
  assert.equal(entry.passed, true);
  const command_ = opacity.commands.find((item, index) => (item.id ?? index) === entry.command);
  assert.ok(command_, 'opacity variant references a missing command');
  const stdout = fs.readFileSync(command_.frozen_stdout, 'utf8');
  assert.ok(stdout.includes(`test ${entry.test_name} ... ok`), 'opacity variant has no exact successful rustdoc test line');
  assert.match(entry.test_name, /compile fail/);
  row.evidence.push({ external_opacity_variant: name, command: entry.command, test_name: entry.test_name,
    stdout: relative(command_.frozen_stdout), stderr: relative(command_.frozen_stderr) });
}
function borrowEvidence(row, target) {
  opacityEvidence(row, `borrow-${target}`);
  row.scope = 'a real adapter-local Borrow cannot inhabit Wasmtime component Tuple/List resource values, a guest quotation literal, or an approved session submission; these are exact Rust ingress non-representability controls, not owning-Resource aggregate refusals or a guest BorrowToken runtime';
  if (!opacity) return;
  assert.equal(opacity.source_sha256?.['verification/m5/borrow-boundary.rs'],
    receipt.sources['verification/m5/borrow-boundary.rs'].sha256, 'borrow ingress fixture was not bound when rustdoc ran');
  const positive = opacity.variants.find(item => item.name === 'borrow-positive-ingress');
  assert.ok(positive, 'borrow controls require a positive well-typed ingress control');
  assert.equal(positive.passed, true);
  const command_ = opacity.commands.find((item, index) => (item.id ?? index) === positive.command);
  assert.ok(command_);
  assert.equal(positive.test_name.includes('compile fail'), false);
  assert.ok(fs.readFileSync(command_.frozen_stdout, 'utf8').includes(`test ${positive.test_name} ... ok`));
  row.evidence.push({ external_positive_control: positive.name, command: positive.command,
    test_name: positive.test_name, stdout: relative(command_.frozen_stdout) });
}
function loadOpacity(reportPath) {
  const binding = freeze(reportPath, 'external/opacity-report.json');
  const value = JSON.parse(fs.readFileSync(binding.frozen));
  assert.equal(value.schema, 'noble-m5-opacity/v1'); assert.equal(value.passed, true);
  assert.ok(Object.keys(value.source_sha256 ?? {}).some(file =>
    file === 'crates/noble-kernel/src/authority.rs' || file.startsWith('crates/noble-kernel/src/authority/')));
  for (const [file, sha256] of Object.entries(value.source_sha256)) assert.equal(receipt.sources[file]?.sha256, sha256, `stale opacity source ${file}`);
  assert.ok(Array.isArray(value.commands) && value.commands.length);
  const passes = new Set();
  for (const [index, command_] of value.commands.entries()) {
    assert.equal(command_.status, 0); assert.equal(command_.signal ?? null, null);
    assert.ok(Array.isArray(command_.args) && command_.args.length);
    assert.ok(command_.args.includes('--test') || command_.args.includes('--doc'), 'opacity commands must actually invoke the rustdoc test path');
    const stdout = freeze(path.resolve(path.dirname(binding.real), command_.stdout_path), `external/opacity-${index}.stdout`);
    const stderr = freeze(path.resolve(path.dirname(binding.real), command_.stderr_path), `external/opacity-${index}.stderr`);
    command_.frozen_stdout = stdout.frozen; command_.frozen_stderr = stderr.frozen;
    const executable = freeze(command_.executable, `external/opacity-executable-${index}`, true);
    assert.equal(command_.executable_sha256, executable.sha256, 'opacity executable was not bound at execution');
    for (const line of fs.readFileSync(stdout.frozen, 'utf8').split('\n')) {
      if (line.startsWith('test ') && /authority(?:\.rs|\/)/.test(line) &&
          line.includes('compile fail') && line.endsWith(' ... ok')) passes.add(line);
    }
  }
  assert.equal(passes.size, 8, 'all eight independent authority opacity/role compile-fail controls are required');
  assert.ok(Array.isArray(value.variants));
  assert.equal(new Set(value.variants.map(item => item.name)).size, value.variants.length);
  return value;
}

try {
  for (const directory of ['crates', 'tools', 'verification/m5/peer/src', 'verification/mc2/contracts']) sourceTree(directory);
  for (const file of ['Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', 'flake.lock', 'policy/tool-selection.json',
    'verification/m5/gate.mjs', 'verification/m5/core-boundary.mjs', 'verification/m5/borrow-boundary.rs',
    'verification/m5/identity-boundary.rs',
    'verification/m5/build.mjs', 'verification/m5/pins.json',
    'verification/mc1/monotonic.noble-contract', 'verification/mc1/monotonic-refutation.lean',
    'verification/m5/peer/Cargo.toml', 'verification/m5/peer/Cargo.lock']) source(file);
  for (const [file, required] of Object.entries(selected)) {
    source(file);
    const bytes = fs.readFileSync(path.join(artifacts, 'sources', file));
    const document = JSON.parse(bytes);
    assert.equal(new Set(document.cases.map(item => item.id)).size, document.cases.length, `duplicate canonical case: ${file}`);
    const workload = required.map(id => {
      const item = document.cases.find(item => item.id === id);
      assert.ok(item, `missing ${id}`); cases.set(id, item);
      return { id, input: item.input, expected: item.expected };
    });
    receipt.conformance.push({ path: file, snapshot: `sources/${file}`, revision: document.revision,
      byte_sha256: hash(bytes), workload_sha256: digest(workload), workload,
      identity_for_workload: 'complete-selected-id-input-expected-projection; state/evidence-not-workload' });
    assert.equal(digest(workload), supportedWorkloads[file], `unsupported workload change: ${file}`);
  }
  receipt.source_revision = `sha256:${digest(Object.entries(receipt.sources).map(([file, value]) => [file, value.sha256]).sort())}`;
  const publicationTools = JSON.parse(fs.readFileSync(path.join(artifacts, 'sources/verification/m5/pins.json'))).publication_probe;
  for (const [name, supplied] of [['cli', args[0]], ['peer', args[1]], ['resources', args[2]], ['authority', args[3]],
    ['shell', publicationTools.shell], ['prlimit', publicationTools.prlimit],
    ['node', process.execPath], ['wasm_tools', '/nix/store/2n9b5yzj7n8jlnglp60gd5nmfjnf4xhf-wasm-tools-1.245.1/bin/wasm-tools']]) {
    receipt.executables[name] = freeze(supplied, `executables/${name}`, true);
  }
  ({ cli, peer, resources: resourceBinary, authority: authorityBinary, node, wasm_tools: wasmTools } =
    Object.fromEntries(Object.entries(receipt.executables).map(([key, value]) => [key, value.frozen])));
  if (args[6]) control('rustdoc-opacity-evidence', row => {
    opacity = loadOpacity(args[6]);
    row.evidence.push({ report: 'external/opacity-report.json', variants: opacity.variants });
  });
  control('selected-wasm-tools-version', row => {
    const run = command('wasm-tools-version', wasmTools, ['--version']); attach(row, run); completed(run);
    assert.equal(run.text.trim(), 'wasm-tools 1.245.1');
  });
  control('selected-node-version', row => {
    const run = command('node-version', node, ['--version']); attach(row, run); completed(run);
    assert.equal(run.text.trim(), 'v24.13.0');
    assert.equal(process.versions.node, '24.13.0');
  });
  control('resource-native-execution', () => runNative('resources', resourceBinary, 24));
  control('authority-native-execution', () => runNative('authority', authorityBinary, 20));
  control('engine-collection-and-native-retirement', runResourceControl);

  control('wit-export-and-runtime-namespaces', row => {
    const compiled = compile(row, 'export-namespaces',
      'package noble-test:names@1.0.0; world demo { export memory: func(x: string) -> string; export noble-cleanup: func(x: s64) -> s64; }',
      { memory: 'drop "ok"', 'noble-cleanup': '1 +' });
    const memory = peerRun(row, compiled, 'memory', [{ type: 'string', value: 'input' }]);
    exact(memory.observation.results, [{ type: 'string', value: 'ok' }]);
    const cleanup = peerRun(row, compiled, 'noble-cleanup', [{ type: 's64', value: '41' }]);
    exact(cleanup.observation.results, [{ type: 's64', value: '42' }]);
  });
  control('atomic-component-bundle-publication', row => {
    const compiled = compile(row, 'publication-baseline', mathWit, { inc: ' '.repeat(60_000) + '1 +' });
    const payloads = fs.readdirSync(compiled.directory).filter(name => name !== 'report.json')
      .map(name => fs.statSync(path.join(compiled.directory, name)).size);
    const limit = Math.max(...payloads) + 1;
    assert.ok(fs.statSync(path.join(compiled.directory, 'report.json')).size > limit,
      'receipt quota must be below the report size and above every other complete artifact');
    const parent = path.join(artifacts, 'components');
    const before = fs.readdirSync(parent).sort();
    const destination = path.join(parent, `${++serial}-publication-refusal`);
    const run = command('publication-file-quota', receipt.executables.shell.frozen,
      ['--noprofile', '--norc', '-c', 'trap "" XFSZ; exec "$@"', 'm5-publication-quota',
        receipt.executables.prlimit.frozen, `--fsize=${limit}:${limit}`, '--',
        cli, 'component', 'compile', compiled.witPath, 'demo', destination,
        `inc=${compiled.sources[0].path}`]);
    const value = json(run); attach(row, run, value); completed(run, 2);
    assert.equal(value.outcome, 'error');
    assert.equal(value.diagnostic.code, 'artifact-io', 'failure must occur while writing the assembled bundle');
    assert.equal(value.component_emitted, false);
    assert.deepEqual(fs.readdirSync(destination), [], 'failed publication exposes neither component nor success report');
    assert.deepEqual(fs.readdirSync(parent).sort(), [...before, path.basename(destination)].sort(),
      'unpublished private staging is cleaned without touching existing bundles');
    const repeated = command('publication-existing-destination', cli,
      ['component', 'compile', compiled.witPath, 'demo', compiled.directory, `inc=${compiled.sources[0].path}`]);
    const refusal = json(repeated); attach(row, repeated, refusal); completed(repeated, 2);
    assert.equal(refusal.diagnostic.code, 'component-destination');
    assert.deepEqual(JSON.parse(fs.readFileSync(path.join(compiled.directory, 'report.json'))), compiled.report);
    row.scope = 'real child RLIMIT_FSIZE with SIGXFSZ ignored yields a recoverable receipt-write error after assembly; no injected success or mocked filesystem';
  });

  runCase('RA-CASE-01', (row, item) => {
    exact(item.input.result_alternatives, ['ok:42', 'err:read-failed']);
    for (const alternative of item.input.result_alternatives) variant(row, alternative, { result_alternative: alternative }, v => {
      nativeEvidence(v, 'resources', 'lifecycle::ra_case_01_both_normal_results_return_the_same_owner_once', 'both declared completions, owner identity, duplicate completion and pin release');
      const value = peerRun(v, bootstrapComponent(v), 'read-counter', [{ type: 'host-counter', value: '42' }], 'normal', { counter_error: alternative.startsWith('err:') });
      exact(value.observation.results, [{ type: 'own<counter>', disposition: 'released-by-peer' }]);
      assert.equal(value.host.resources.returned_owners, 1); assert.equal(value.host.resources.local_releases, 1);
      assert.equal(value.host.resources.native_releases, 1); assert.equal(value.host.guest_requests, 1);
      v.scope = 'native evidence checks Live before explicit owner release; peer report is deliberately after cleanup';
    });
  });
  runCase('RA-CASE-02', (row, item) => {
    exact(item.input.targets, ['Pair', 'List', 'quote', 'session', 'borrowed-export']);
    for (const target of item.input.targets) variant(row, target, { target }, v => {
      if (target === 'borrowed-export') compile(v, 'borrowed-export', resourceWit('value: borrow<counter>'), { check: '' }, { refusal: 'unsupported' });
      else borrowEvidence(v, target);
    });
  });
  runCase('RA-CASE-03', (row, item) => {
    exact(item.input.attempts, ['read', 'release', 'transfer', 'borrow']);
    for (const attempt of item.input.attempts) variant(row, attempt, { attempt }, v =>
      nativeEvidence(v, 'resources', 'lifecycle::ra_case_03_busy_reentry_rejects_read_release_transfer_and_borrow', `the native test asserts ${attempt} refusal against the retained Busy scope`));
  });
  runCase('RA-CASE-04', (row, item) => variant(row, item.input.harness, item.input, v => {
    const wit = 'package noble-test:counter@1.0.0; interface counters { resource counter { read: async func() -> result<s64, string>; } } world demo { use counters.{counter}; import counters; export check: func(value: own<counter>) -> own<counter>; }';
    compile(v, 'async-borrow', wit, { check: 'counter.read drop' }, { refusal: 'unsupported' });
  }));
  runCase('RA-CASE-05', (row, item) => variant(row, item.input.harness, item.input, v => {
    nativeEvidence(v, 'resources', 'retirement::ra_case_05_cancelled_unstoppable_work_keeps_its_pin_after_tokens_are_dropped', 'production retirement and retained native pin after Rust token drop');
    resourceEvidence(v, item.id);
  }));
  runCase('RA-CASE-06', (row, item) => {
    exact(item.input.events, ['native-complete', 'duplicate-native-complete', 'retire-again']);
    for (const event of item.input.events) variant(row, event, { event }, v => {
      nativeEvidence(v, 'resources', 'retirement::ra_case_06_late_completion_retires_once_without_returning_an_owner', 'the ordered three-event test checks one local release, zero returned owners and no remaining pin');
      resourceEvidence(v, item.id);
    });
  });
  for (const [id, test] of [
    ['RA-CASE-07', 'handoff::ra_case_07_invalid_second_resource_argument_preserves_all_sender_owners'],
    ['RA-CASE-08', 'handoff::ra_case_08_domain_error_keeps_transfer_cleanup_with_receiver'],
    ['RA-CASE-09', 'retirement::ra_case_09_unexpected_sync_suspension_revokes_access_but_not_the_pin'],
  ]) runCase(id, (row, item) => variant(row, item.input.harness, item.input, v => {
    nativeEvidence(v, 'resources', test, 'production ownership table decisions and accounting; no physical native cancellation or remote release claim');
    if (id === 'RA-CASE-09') resourceEvidence(v, id);
    if (id === 'RA-CASE-08') {
      const value = peerRun(v, bootstrapComponent(v), 'transfer-counter', [{ type: 'host-counter', value: '42' }]);
      exact(value.observation.results, [{ type: 'result', err: { type: 'string', value: 'receiver-error' } }]);
      assert.equal(value.host.resources.returned_owners, 0); assert.equal(value.host.resources.local_releases, 1);
      assert.equal(value.host.resources.native_releases, 1);
    }
  }));

  runCase('ADAPT-11', (row, item) => {
    const required = [
      { value: 'string:héllo', fault: 'none', outcome: 'lossless-round-trip' },
      { value: 'list<u8>:0,127,255', fault: 'none', outcome: 'lossless-round-trip' },
      { value: 'string:héllo', fault: 'allocation-quota', outcome: 'defined-failure-with-cleanup' },
      { value: 'list<u8>:0,127,255', fault: 'allocation-quota', outcome: 'defined-failure-with-cleanup' },
      { value: 'string:invalid-UTF8', fault: 'malformed-peer-bytes', outcome: 'reject-with-cleanup' },
      { value: 'list<u8>:truncated', fault: 'malformed-peer-range', outcome: 'reject-with-cleanup' },
    ];
    exact(item.input.variants, required);
    for (const declared of item.input.variants) variant(row, `${declared.value}|${declared.fault}`, declared, v => {
      const compiled = bootstrapComponent(v);
      const text = declared.value.startsWith('string:');
      const exportName = text ? 'echo-text' : 'echo-bytes';
      if (declared.fault === 'none') {
        const input_ = { type: text ? 'string' : 'list<u8>', value: text ? 'héllo' : [0, 127, 255] };
        const value = peerRun(v, compiled, exportName, [input_]);
        exact(value.observation.results, [text ? input_ : { type: 'list', value: [0, 127, 255].map(byte => ({ type: 'u8', value: byte })) }]);
        assert.equal(value.host.guest_requests, 1); assert.equal(value.host.protected_operations, 0);
        v.canonical_peer_accounting = { allocations: null, copies: null,
          reason: 'Wasmtime does not expose these core probes through WIT; measured independently below, never fabricated as zero', cleanup: value.cleanup };
      }
      const points = declared.fault === 'allocation-quota'
        ? ['argument-lowering', 'import-result-area', 'peer-result-lowering', 'export-result-copy', 'export-result-area'] : ['none'];
      for (const point of points) {
        const run = command(`core-${exportName}-${declared.fault}-${point}`, node,
          [path.join(artifacts, 'sources/verification/m5/core-boundary.mjs'), compiled.core, exportName, declared.fault, point]);
        const value = json(run); attach(v, run, value); completed(run);
        assert.equal(value.schema, 'noble-m5-core-boundary/v1'); assert.equal(value.result, 'passed');
        assert.equal(value.outcome, declared.outcome); assert.equal(value.fault, declared.fault);
        assert.equal(value.quota_point, point); assert.equal(value.core.sha256, receipt.retained[relative(compiled.core)].sha256);
        assert.equal(value.partial_trusted_values_published, 0); assert.equal(value.after_cleanup.live_bytes, 0);
        v.ABI_revision = value.ABI_revision;
      }
      v.scope = 'independent Rust canonical component round trips are paired with unchanged core-Wasm hostile JS conversion and cleanup instrumentation; malformed core imports are not represented as malformed typed Wasmtime values';
    });
  });

  runCase('WI-01', (row, item) => variant(row, item.input.harness, item.input, v => {
    const value = bindings(v, item.input.wit);
    assert.equal(value.imports.length, 1);
    const operation = value.imports[0];
    exact(operation.input_types, item.expected.input_types); exact(operation.output_types, item.expected.output_types);
    exact(operation.effects, [item.expected.effect_identity]); assert.equal(operation.identity, item.expected.effect_identity);
    const compiled = compile(v, 'math-import', mathImportWit, { inc: 'arithmetic.inc' });
    const observed = peerRun(v, compiled, 'inc', [{ type: 's64', value: '41' }]);
    exact(observed.observation.results, [{ type: 's64', value: '42' }]);
    exact(observed.host.requests, [item.expected.effect_identity]);
  }));
  runCase('WI-02', (row, item) => variant(row, item.input.harness, item.input, v => {
    exact(item.input.noble_interface, 'Bool -- Bool ! {}');
    compile(v, 'bool-interface-mismatch', item.input.wit, { inc: '[ false ] [ true ] if' }, { refusal: 'error' });
    v.source_interface = 'Bool -- Bool ! {}; source body consumes the Bool branch selector and returns Bool, never coerces an I64';
  }));
  runCase('WI-04', (row, item) => {
    exact(item.input.operations, ['dup', 'quote', 'generic-serialization']);
    for (const operation of item.input.operations) variant(row, operation, { operation }, v => {
      compile(v, `resource-${operation}`, resourceWit(),
        { check: operation === 'generic-serialization' ? 'quote reflect' : operation }, { refusal: 'error' });
      if (operation === 'generic-serialization') v.scope = 'the actual generic serialization route first quotes Data, then reflects to inert Syntax; a live Resource is refused at the capture boundary before serializable Syntax exists';
    });
  });
  runCase('WI-05', (row, item) => {
    exact(item.input.escape_targets, ['returned-pair', 'quotation-capture', 'session-storage', 'borrowed-export']);
    for (const target of item.input.escape_targets) variant(row, target, { target, expected_outcome: item.expected.outcomes_by_target[target] }, v => {
      if (target === 'borrowed-export') compile(v, 'wi-borrowed-export', resourceWit('value: borrow<counter>'), { check: '' }, { refusal: 'unsupported' });
      else borrowEvidence(v, { 'returned-pair': 'Pair', 'quotation-capture': 'quote', 'session-storage': 'session' }[target]);
    });
  });
  runCase('WI-06', (row, item) => {
    exact(item.input.mutations, ['forged-slot', 'stale-generation', 'wrong-type', 'instance-B']);
    for (const mutation of item.input.mutations) variant(row, mutation, { mutation }, v =>
      nativeEvidence(v, 'resources', 'handoff::wi_06_every_handle_binding_is_checked_before_guest_work', 'production table rejects each declared handle mutation before native pin/work admission'));
  });
  runCase('WI-07', (row, item) => {
    exact(item.input.variants, [
      { reviewed_pure_contract: false, claimed_effects: [], outcome: 'effect-reject' },
      { reviewed_pure_contract: true, claimed_effects: [], outcome: 'effect-reject' },
      { reviewed_pure_contract: false, claimed_effects: [item.input.operation], outcome: 'accept' },
      { reviewed_pure_contract: true, claimed_effects: [item.input.operation], outcome: 'accept' },
    ]);
    for (const declared of item.input.variants) variant(row,
      `reviewed-pure=${declared.reviewed_pure_contract};claimed-effects=${JSON.stringify(declared.claimed_effects)}`, declared, v => {
        const wit = input('effect-matrix.wit', mathImportWit);
        const bound = bindings(v, mathImportWit);
        exact(bound.imports[0].effects, [item.input.operation]);
        const run = command('check-import-effect', cli, ['component', 'check-effect', wit, 'demo', 'arithmetic.inc', ...declared.claimed_effects]);
        const value = json(run); attach(v, run, value);
        const accepted = declared.outcome === 'accept';
        completed(run, accepted ? 0 : 2); assert.equal(value.outcome, accepted ? 'accepted' : 'error');
        assert.equal(value.component_emitted, false);
        v.reviewed_purity_scope = 'review metadata is not a binding/admission input; the exact same immutable operation identity is checked and never erased for either declared review state';
      });
  });
  runCase('WI-08', (row, item) => variant(row, item.input.harness, item.input, v => {
    exact(item.input.operation, 'noble-test:store/api@1.0.0#read'); exact(item.input.host_grant, false);
    const wit = 'package noble-test:store@1.0.0; interface api { read: func() -> s64; } world demo { import api; export read: func() -> s64; }';
    const bound = bindings(v, wit); exact(bound.imports[0].effects, item.input.declared_effects);
    const compiled = compile(v, 'store-denial', wit, { read: 'api.read' });
    const value = peerRun(v, compiled, 'read', [], 'trap', { deny_protected: true });
    exact(value.host.requests, [item.input.operation]); assert.equal(value.host.guest_requests, item.expected.guest_requests);
    assert.equal(value.host.protected_operations, item.expected.protected_operations);
  }));
  runCase('WI-09', (row, item) => {
    const built = [];
    variant(row, 'build-key-changed', item.input, v => {
      exact([item.input.world_before, item.input.world_after], ['noble-test:math/demo@1.0.0', 'noble-test:math/demo@1.1.0']);
      for (const version of ['1.0.0', '1.1.0']) built.push(compile(v, `world-${version}`, mathWit.replace('@1.0.0', `@${version}`), { inc: '1 +' }));
      assert.notEqual(built[0].report.build_context_hex, built[1].report.build_context_hex);
    });
    variant(row, 'definition-identity-depends-on-resolved-source-not-world-label', { definition_identity: item.expected.definition_identity }, v => {
      assert.equal(built.length, 2);
      v.evidence.push({ components: built.map(value => relative(value.component)) });
      if (!opacity) {
        gap(v, 'The source-bound positive namespace identity control must run via --opacity-report.');
        return;
      }
      const fixture = 'verification/m5/identity-boundary.rs';
      assert.equal(opacity.source_sha256?.[fixture], receipt.sources[fixture].sha256);
      const entry = opacity.variants.find(item => item.name === 'namespace-local-definition-identity');
      assert.equal(entry?.passed, true);
      assert.equal(entry.test_name.includes('compile fail'), false);
      const run = opacity.commands.find((item, index) => (item.id ?? index) === entry.command);
      assert.ok(run);
      assert.ok(fs.readFileSync(run.frozen_stdout, 'utf8').includes(`test ${entry.test_name} ... ok`));
      v.evidence.push({ external_variant: entry.name, command: entry.command, test_name: entry.test_name,
        stdout: relative(run.frozen_stdout), stderr: relative(run.frozen_stderr) });
      v.scope = 'Resolved import aliases intern equally and different resolved dependencies differ within each generated namespace; numeric IDs are never compared across namespaces. Versioned build contexts differ independently.';
    });
  });
  runCase('WI-10', (row, item) => variant(row, item.input.harness, item.input, v => {
    exact(item.input, { harness: 'program-export', noble_type: 'Program<I64,I64,{}>', wit_representation: 'u32-address', declared_program_adapter: false });
    compile(v, 'program-address-export', 'package noble-test:program@1.0.0; world demo { export program: func() -> u32; }', { program: '[ 1 + ]' }, { refusal: 'unsupported' });
    v.scope = 'exact proposed u32-address representation is unsupported at WIT admission, before source lowering; not a claimed general Program adapter or Program-specific identity proof';
  }));
  runCase('WI-15', (row, item) => variant(row, item.input.harness, item.input, v => {
    exact(item.input.peer_language, 'Rust');
    const compiled = compile(v, 'independent-inc', item.input.wit, { inc: item.input.noble_body });
    const value = peerRun(v, compiled, 'inc', [{ type: 's64', value: item.input.argument }]);
    exact(value.observation.results, [{ type: 's64', value: item.expected.result.value }]);
    assert.equal(value.host.guest_requests, 0); assert.equal(value.host.protected_operations, 0);
  }));

  runCase('OCTET-01', (row, item) => {
    const mapping = {
      'allow-and-observed-success': 'sequencing::octet01_commit_precedes_execution_and_success_needs_observation',
      deny: 'octet01_denial_creates_neither_witness_nor_attempt',
      'preflight-capacity-failure': 'octet01_preflight_preserves_the_valid_caller_obligation',
      'external-outcome-unknown-after-commit': 'sequencing::octet01_unknown_does_not_restore_or_retry_authority',
    };
    exact(item.input.variants.map(value => value.case).sort(), Object.keys(mapping).sort());
    for (const declared of item.input.variants) variant(row, declared.case, declared, v => {
      nativeEvidence(v, 'authority', mapping[declared.case], 'production authority phase, commitment, witness and receipt assertions for this exact named variant');
      if (['allow-and-observed-success', 'deny'].includes(declared.case)) {
        const denied = declared.case === 'deny';
        const value = peerRun(v, bootstrapComponent(v), 'protected', [{ type: 's64', value: '7' }], denied ? 'trap' : 'normal', { deny_protected: denied });
        assert.equal(value.host.witnesses_created, declared.witnesses_created);
        assert.equal(value.host.witness_consumptions, declared.witness_consumptions);
        assert.equal(value.host.protected_operations, declared.protected_operations);
        assert.equal(value.host.attempts_admitted, denied ? 0 : 1);
        exact(value.host.receipts.map(receipt => receipt.claim), [denied ? 'Denial' : 'OperationSuccess']);
        if (!denied) exact(value.observation.results, [{ type: 's64', value: '7' }]);
      }
    });
  });
  runCase('OCTET-02', (row, item) => {
    exact(item.input.variants, ['changed-operation-contract', 'changed-plan-target', 'changed-plan-amount', 'wrong-actor',
      'wrong-owner-context', 'changed-policy-revision', 'expired', 'revoked', 'quota-exhausted', 'missing-current-facts',
      'wrong-resource-kind', 'stale-generation', 'consumed-witness-replay', 'serialized-witness-tag']);
    for (const name of item.input.variants) variant(row, name, { variant: name }, v => {
      const test = ['consumed-witness-replay', 'serialized-witness-tag'].includes(name)
        ? 'bindings::octet02_serialized_tag_cannot_mint_live_authority'
        : 'bindings::octet02_every_binding_and_required_current_fact_is_checked';
      nativeEvidence(v, 'authority', test, 'native test separately constructs each mutation and checks refusal before commit; replay counters are compared against the earlier committed attempt, not invented as global zero');
      if (name === 'consumed-witness-replay') nativeEvidence(v, 'authority',
        'sequencing::octet01_unknown_does_not_restore_or_retry_authority',
        'the replay is exactly one additional admission request, with no additional consumption, attempt or protected operation after the prior committed work');
    });
  });
  runCase('OCTET-03', (row, item) => {
    exact(item.input.variants, ['allow-data-as-resource', 'raw-id-as-resource', 'default-construct-witness', 'public-field-construction',
      'dup-live-witness', 'drop-live-witness', 'capture-live-witness', 'serialize-live-witness']);
    for (const name of item.input.variants) variant(row, name, { variant: name }, v => {
      if (['default-construct-witness', 'public-field-construction'].includes(name)) { opacityEvidence(v, name); return; }
      const source_ = { 'allow-data-as-resource': '', 'raw-id-as-resource': '', 'dup-live-witness': 'dup',
        'drop-live-witness': 'drop', 'capture-live-witness': 'quote', 'serialize-live-witness': 'quote reflect' }[name];
      const inputType = name === 'allow-data-as-resource' ? 'bool' : name === 'raw-id-as-resource' ? 's64' : 'own<authorization>';
      const outputType = ['allow-data-as-resource', 'raw-id-as-resource'].includes(name) ? 'own<authorization>' : '';
      compile(v, name, witnessWit(`value: ${inputType}`, outputType), { check: source_ }, { refusal: 'error' });
      if (name === 'serialize-live-witness') v.scope = 'real quote/reflect route rejects the live witness before constructing serializable inert Syntax';
    });
  });
  runCase('OCTET-04', (row, item) => {
    const mapping = {
      'matching-approved-success-observation': 'sequencing::octet01_commit_precedes_execution_and_success_needs_observation',
      'matching-approved-failure-observation': 'receipts::octet04_failure_observation_supports_failure_not_success',
      'plan-as-success': 'receipts::octet04_plan_witness_attempt_and_missing_observation_cannot_claim_success',
      'witness-as-success': 'receipts::octet04_plan_witness_attempt_and_missing_observation_cannot_claim_success',
      'attempt-as-success': 'receipts::octet04_plan_witness_attempt_and_missing_observation_cannot_claim_success',
      'failed-observation-as-success': 'receipts::octet04_failure_observation_supports_failure_not_success',
      'unknown-observation-as-success': 'sequencing::octet01_unknown_does_not_restore_or_retry_authority',
      'missing-observation-as-success': 'receipts::octet04_plan_witness_attempt_and_missing_observation_cannot_claim_success',
      'different-attempt-observation': 'receipts::octet04_another_attempts_observation_is_inapplicable',
      'schema-valid-import-without-provenance': 'receipts::octet04_imported_bytes_need_provenance_and_cannot_import_trust_flags',
      'cancelled-invocation-late-confirmed-external-success': 'receipts::finality::octet04_cancelled_invocation_allows_late_operation_success_only',
    };
    exact(item.input.variants.map(value => value.case).sort(), Object.keys(mapping).sort());
    for (const declared of item.input.variants) variant(row, declared.case, declared, v => {
      nativeEvidence(v, 'authority', mapping[declared.case], 'production claim-specific observation/receipt boundary');
      if (['plan-as-success', 'witness-as-success', 'attempt-as-success'].includes(declared.case)) opacityEvidence(v, declared.case);
    });
  });

  control('owning-resource-generic-drop-rejection', row => {
    compile(row, 'resource-drop', resourceWit('value: own<counter>', ''), { check: 'drop' }, { refusal: 'error' });
  });
  control('typed-input-failure-retires-prior-resource-arguments', row => {
    const value = peerRun(row, bootstrapComponent(row), 'read-counter', [
      { type: 'host-counter', value: '42' }, { type: 'list<u8>', value: [256] },
    ], 'trap');
    assert.equal(value.host.guest_requests, 0);
    assert.equal(value.host.resources.local_releases, 1);
    assert.equal(value.host.resources.native_releases, 1);
    assert.equal(value.host.invocation, 'Failed');
  });
  control('changed-plan-denial-retires-preserved-witness-once', row => {
    const original = bootstrapComponent(row);
    const bodies = Object.fromEntries(original.sources.map(item => [item.name, item.body]));
    bodies.protected = 'dup authorization.prepare swap 1 + authorization.protected';
    const compiled = compile(row, 'changed-plan-cleanup', fs.readFileSync(original.witPath), bodies, { world: 'bootstrap' });
    const value = peerRun(row, compiled, 'protected', [{ type: 's64', value: '41' }], 'trap');
    assert.equal(value.host.guest_requests, 2);
    assert.equal(value.host.witnesses_created, 1);
    assert.equal(value.host.witness_consumptions, 0);
    assert.equal(value.host.attempts_admitted, 0);
    assert.equal(value.host.protected_operations, 0);
    assert.equal(value.host.invocation, 'Failed');
    assert.equal(value.host.receipts.length, 1);
    assert.equal(value.host.receipts[0].claim, 'Denial');
    assert.equal(value.host.receipts[0].scope, 'AdmissionBoundary');
  });
  control('bootstrap-component-artifact-reassembly', row => {
    const compiled = bootstrapComponent(row);
    const directory = path.join(artifacts, 'components', `${++serial}-reassembly`); fs.mkdirSync(directory);
    const parsed = path.join(directory, 'parsed.wasm'); const embedded = path.join(directory, 'core.wasm');
    const component = path.join(directory, 'component.wasm');
    for (const argv of [
      ['parse', path.join(compiled.directory, 'module.wat'), '-o', parsed],
      ['component', 'embed', '--world', 'bootstrap', '--encoding', 'utf8', compiled.witPath, parsed, '-o', embedded],
      ['component', 'new', embedded, '-o', component], ['validate', component],
    ]) { const run = command('independent-artifact-reassembly', wasmTools, argv); attach(row, run); completed(run); }
    assert.deepEqual(fs.readFileSync(embedded), fs.readFileSync(compiled.core));
    assert.deepEqual(fs.readFileSync(component), fs.readFileSync(compiled.component));
    for (const file of [parsed, embedded, component]) {
      fs.chmodSync(file, 0o400); const bytes = fs.readFileSync(file);
      receipt.retained[relative(file)] = { sha256: hash(bytes), bytes: bytes.length }; monitored.push({ file, sha256: hash(bytes) });
    }
  });
} catch (error) {
  receipt.failure = String(error.stack ?? error);
} finally {
  // Preserve partial artifacts too: a failed invocation is not permission to
  // omit the component bytes that happened to be emitted before its failure.
  function retainOutputs(directory) {
    for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
      const file = path.join(directory, entry.name);
      if (entry.isDirectory()) retainOutputs(file);
      else if (entry.isFile()) {
        const name = relative(file);
        if (!Object.hasOwn(receipt.retained, name)) {
          const bytes = fs.readFileSync(file);
          fs.chmodSync(file, 0o400);
          receipt.retained[name] = { sha256: hash(bytes), bytes: bytes.length };
          monitored.push({ file, sha256: hash(bytes) });
        }
      } else throw Error(`unexpected output filesystem entry: ${file}`);
    }
  }
  try { retainOutputs(path.join(artifacts, 'components')); }
  catch (error) { receipt.integrity_failures.push({ phase: 'retaining-partial-outputs', error: String(error) }); }
  for (const binding of monitored) {
    try { assert.equal(hash(fs.readFileSync(binding.file)), binding.sha256); }
    catch (error) { receipt.integrity_failures.push({ file: binding.file, error: String(error) }); }
  }
  const expectedIds = Object.values(selected).flat().sort();
  const executedIds = receipt.cases.map(row => row.id).sort();
  const complete = JSON.stringify(executedIds) === JSON.stringify(expectedIds);
  receipt.summary = {
    required_cases: expectedIds.length, executed_cases: receipt.cases.length,
    passed_cases: receipt.cases.filter(row => row.result === 'passed').length,
    failed_cases: receipt.cases.filter(row => row.result === 'failed').map(row => row.id),
    gap_cases: receipt.cases.filter(row => row.result === 'gap').map(row => row.id),
    missing_cases: expectedIds.filter(id => !executedIds.includes(id)),
    variants: receipt.cases.reduce((count, row) => count + row.variants.length, 0),
    native_tests: receipt.native_tests.length, failed_native_tests: receipt.native_tests.filter(row => row.result !== 'passed').map(row => `${row.kind}:${row.name}`),
    failed_controls: receipt.controls.filter(row => row.result !== 'passed').map(row => row.name), complete,
  };
  receipt.passed = complete && !receipt.failure && !receipt.integrity_failures.length && !receipt.gaps.length
    && receipt.cases.every(row => row.result === 'passed') && receipt.controls.every(row => row.result === 'passed')
    && receipt.native_tests.length === 44 && receipt.native_tests.every(row => row.result === 'passed');
  receipt.result = receipt.passed ? 'passed' : 'failed';
  fs.writeFileSync(path.join(artifacts, 'report.json'), JSON.stringify(receipt, null, 2) + '\n', { flag: 'wx', mode: 0o400 });
  console.log(JSON.stringify({ schema: receipt.schema, result: receipt.result, report: path.join(artifacts, 'report.json'), summary: receipt.summary }));
  process.exitCode = receipt.passed ? 0 : 1;
}
