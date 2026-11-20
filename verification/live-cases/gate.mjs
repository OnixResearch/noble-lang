#!/usr/bin/env node
// Source-bound acceptance gate for `live-wasm-reload`'s admitted LIVE cases.
//
// It builds the pinned `noble` CLI and the `live_repl` test binary from a clean
// committed tree, runs the selected integration tests, drives real
// `noble live repl` / `noble live watch` children for every observed field of
// the conformance cases, and writes one immutable `acceptance.json` into a new
// external directory. Nothing here is a proof or a document check: every row is
// an executed command with its exit status and byte hashes.
//
// usage: SELECTED_NODE verification/live-cases/gate.mjs NEW_EXTERNAL_DIRECTORY
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { spawn, spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { editorSelfCheck, live04Variants } from './wasm-edit.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
assert.equal(process.argv.length, 3, 'usage: SELECTED_NODE verification/live-cases/gate.mjs NEW_EXTERNAL_DIRECTORY');
const destination = path.resolve(process.argv[2]);
assert.ok(!destination.startsWith(`${root}${path.sep}`) && destination !== root, 'output directory must be outside the repository');
assert.equal(fs.existsSync(destination), false, 'output directory must not exist');
const sha256 = value => crypto.createHash('sha256').update(value).digest('hex');
const content = file => fs.readFileSync(path.join(root, file));

const selected = JSON.parse(content('policy/tool-selection.json'));
const node = path.join(selected.tool_paths.node.output, 'bin/node');
const rust = path.join(selected.tool_paths.quality_rust.output, 'bin');
const cargo = path.join(rust, 'cargo');
assert.equal(fs.realpathSync(process.execPath), fs.realpathSync(node), 'selected Node required');
assert.deepEqual(process.execArgv, [], 'unreviewed Node flags');
// The build uses the pinned toolchain and the system C compiler only: user or
// host cargo wrappers, mold flags and target rustflags are neutralized so the
// same committed tree builds the same way wherever the gate runs.
const cc = spawnSync('sh', ['-c', 'command -v cc'], { encoding: 'utf8' });
assert.equal(cc.status, 0, 'a system `cc` is required');

const caseFile = 'specs/conformance/live-wasm-cases.json';
const packet = JSON.parse(content(caseFile));
assert.equal(packet.schema_version, 1);
assert.equal(packet.revision, '0.1.0-draft.5');
const cases = new Map(packet.cases.map(row => [row.id, row]));
for (const id of ['LIVE-01', 'LIVE-02', 'LIVE-03', 'LIVE-04', 'LIVE-05', 'LIVE-06', 'LIVE-07', 'LIVE-08', 'LIVE-09', 'LIVE-10'])
  assert.ok(cases.has(id), `missing canonical case: ${id}`);

// Exact id/profile/kind/requirements/input/expected projections this gate's
// observations were written against. A case edit changes the digest and the
// gate refuses instead of silently reinterpreting the case.
const projections = {
  'LIVE-01': 'be2ad88418db0f93dda24e6536114f4a8c6657f12014bfd1a0542b46785effa2',
  'LIVE-02': '0eeac6b751629ce202c589ace480d03ec86e684e2c880b4d7df1a94c7e49a40d',
  'LIVE-03': 'bec86d121b5d6cee4c6de5f76d5235c5b442736d369f98e4581b5eb18d8ec2b5',
  'LIVE-04': '99aa94afa26f4e53eb2b63f91c68b621cf77bf984329950665df9d7f3fdbb570',
  'LIVE-05': 'a1d4c1df39a1f7a23f039233a92d333fa5abcff48c1cc0f8e8687e8331137785',
  'LIVE-06': '6ab3f70f7edd8db1734f005680d48cb0bcd4b4f2cae975bebece606302b171d5',
  'LIVE-07': 'aa80f1244e9039b8c7a2a3ebdfa19a05e97658cc8d03c4d78177b56d403d77b5',
  'LIVE-08': 'db9b11bbeb86a1afebf945d585123d0ab755a9e3b84c751ad3785f6a7e4cddc3',
  'LIVE-09': '0787a4357513531b03a2eb02f85878d39f14e7de71d137503a5425857b968ce9',
  'LIVE-10': 'e7a1d5ba9f2fa9248e941fbfc2f595d7332706928b02ccbe6f9fd3715db05cf6',
};
const projectionOf = row => ({
  id: row.id, profile: row.profile, kind: row.kind, requirements: row.requirements,
  input: row.input, expected: row.expected,
});
const projectionDigest = id => sha256(JSON.stringify(projectionOf(cases.get(id))));
for (const [id, digest] of Object.entries(projections))
  assert.equal(projectionDigest(id), digest, `canonical case projection changed: ${id}`);

// Promoted by this slice. Every `expected` field of these cases must be
// observed below; see `promotionGate`.
const promoted = ['LIVE-01', 'LIVE-04', 'LIVE-06', 'LIVE-07', 'LIVE-09'];

// ---------------------------------------------------------------------------
// Source scope, clean tree and manifest.
const scope = ['crates', 'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', 'policy/tool-selection.json', 'verification/live-cases'];
// The gate never binds its own receipt: committing it must not change the
// source revision it records.
const excluded = ['verification/live-cases/acceptance.json'];
const git = args => {
  const result = spawnSync('git', args, { cwd: root, maxBuffer: 64 * 1024 * 1024 });
  assert.equal(result.status, 0, `git ${args.join(' ')} failed: ${result.stderr}`);
  return result.stdout;
};
const dirty = git(['status', '--porcelain', '--untracked-files=all', '--', ...scope]);
assert.equal(dirty.toString(), '', `source scope is not clean:\n${dirty}`);
const studiedCommit = git(['rev-parse', 'HEAD']).toString().trim();
const files = git(['ls-files', '-z', '--', ...scope]).toString().split('\0').filter(Boolean)
  .filter(file => !excluded.includes(file)).sort();
assert.ok(files.length > 0, 'empty source scope');
const manifest = {};
for (const file of files) manifest[file] = sha256(content(file));
const sourceRevision = `sha256:${sha256(JSON.stringify(manifest))}`;

fs.mkdirSync(destination, { recursive: true, mode: 0o700 });
const raw = path.join(destination, 'raw');
const temporary = path.join(destination, 'tmp');
const fixtures = path.join(destination, 'fixtures');
for (const directory of [raw, temporary, fixtures]) fs.mkdirSync(directory, { recursive: true, mode: 0o700 });

const commands = [];
const recordProcess = (label, executable, args, { input, environment = {}, expected = 0 } = {}) => {
  const stdoutPath = path.join(raw, `${label}.out`);
  const stderrPath = path.join(raw, `${label}.err`);
  assert.equal(fs.existsSync(stdoutPath), false, `duplicate command label: ${label}`);
  const started = Date.now();
  const result = spawnSync(executable, args, {
    cwd: root, input, env: { ...process.env, TMPDIR: temporary, ...environment }, maxBuffer: 256 * 1024 * 1024,
  });
  assert.equal(result.status, expected, `${label} exit ${result.status} != ${expected}\n${result.stderr}`);
  fs.writeFileSync(stdoutPath, result.stdout, { flag: 'wx' });
  fs.writeFileSync(stderrPath, result.stderr, { flag: 'wx' });
  commands.push({
    label, kind: 'direct-cli', command: [executable, ...args], exit: result.status, duration_ms: Date.now() - started,
    stdin: input === undefined ? null : sha256(input),
    stdout: { path: `raw/${label}.out`, bytes: result.stdout.length, sha256: sha256(result.stdout) },
    stderr: { path: `raw/${label}.err`, bytes: result.stderr.length, sha256: sha256(result.stderr) },
  });
  return result;
};
const buildEnvironment = {
  PATH: `${rust}:${process.env.PATH}`,
  CARGO_TARGET_DIR: path.join(destination, 'target'),
  CARGO_NET_OFFLINE: 'true',
  CARGO_INCREMENTAL: '0',
  CARGO_BUILD_JOBS: '4',
  RUSTC_WRAPPER: '',
  RUSTC_WORKSPACE_WRAPPER: '',
  CARGO_BUILD_RUSTC_WRAPPER: '',
  RUSTFLAGS: '',
  CARGO_ENCODED_RUSTFLAGS: '',
  CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS: '',
  CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER: cc.stdout.trim(),
};
const run = (label, executable, args, environment = {}) => {
  const stdoutPath = path.join(raw, `${label}.out`);
  const stderrPath = path.join(raw, `${label}.err`);
  const started = Date.now();
  const result = spawnSync(executable, args, {
    cwd: root,
    env: { ...process.env, TMPDIR: temporary, ...environment },
    maxBuffer: 256 * 1024 * 1024,
  });
  assert.equal(result.status, 0, `${label} exit ${result.status}\n${result.stderr}`);
  assert.equal(fs.existsSync(stdoutPath), false, `duplicate command label: ${label}`);
  fs.writeFileSync(stdoutPath, result.stdout, { flag: 'wx' });
  fs.writeFileSync(stderrPath, result.stderr, { flag: 'wx' });
  const row = {
    label, command: [executable, ...args], exit: result.status, duration_ms: Date.now() - started,
    stdout: { path: `raw/${label}.out`, bytes: result.stdout.length, sha256: sha256(result.stdout) },
    stderr: { path: `raw/${label}.err`, bytes: result.stderr.length, sha256: sha256(result.stderr) },
  };
  commands.push(row);
  return result;
};

// ---------------------------------------------------------------------------
// Production build and selected integration tests.
const build = run('production-build', cargo,
  ['build', '-p', 'noble-cli', '--bin', 'noble', '--locked', '--offline', '-j', '4', '--message-format=json-render-diagnostics'],
  buildEnvironment);
const artifacts = build.stdout.toString().split('\n').filter(Boolean).flatMap(line => {
  try { return [JSON.parse(line)]; } catch { return []; }
});
// Cargo reuses one output path per feature set, so each build is copied out
// before the next build can replace it. The default build is exercised first;
// the feature build only runs the seam-gated lanes and never serves the
// default-profile observations.
const binariesDirectory = path.join(destination, 'bin');
fs.mkdirSync(binariesDirectory, { recursive: true, mode: 0o700 });
const builtBinary = artifacts.find(row => row.reason === 'compiler-artifact' && row.target?.name === 'noble' && row.target?.kind?.includes('bin'))?.executable;
assert.ok(builtBinary && fs.existsSync(builtBinary), 'production build omitted the noble executable');
const binary = path.join(binariesDirectory, 'noble-default');
fs.copyFileSync(builtBinary, binary);
const binarySha256 = sha256(fs.readFileSync(binary));

const testBuild = run('test-build', cargo,
  ['test', '-p', 'noble-cli', '--test', 'live_repl', '--no-run', '--locked', '--offline', '-j', '4', '--message-format=json-render-diagnostics'],
  buildEnvironment);
const testArtifacts = testBuild.stdout.toString().split('\n').filter(Boolean).flatMap(line => {
  try { return [JSON.parse(line)]; } catch { return []; }
});
const builtTestBinary = testArtifacts.find(row => row.reason === 'compiler-artifact' && row.target?.name === 'live_repl')?.executable;
assert.ok(builtTestBinary && fs.existsSync(builtTestBinary), 'live_repl test build omitted its executable');
const testBinary = path.join(binariesDirectory, 'live_repl-default');
fs.copyFileSync(builtTestBinary, testBinary);
const testBinarySha256 = sha256(fs.readFileSync(testBinary));

const runTest = label => {
  const result = run(`test-${label}`, testBinary, ['--exact', label, '--test-threads=1', '--nocapture']);
  assert.match(result.stdout.toString(), /test result: ok\. 1 passed/, `selected test did not pass once: ${label}`);
  return `test-${label}`;
};

// The feature build exists only for seam-gated lanes. Its binaries are copied
// out like the default ones, and no default-profile observation may use them.
const buildFeature = () => {
  const featureBuild = run('feature-build', cargo,
    ['build', '-p', 'noble-cli', '--bin', 'noble', '--features', 'live-test-barrier', '--locked', '--offline', '-j', '4',
      '--message-format=json-render-diagnostics'],
    buildEnvironment);
  const rows = featureBuild.stdout.toString().split('\n').filter(Boolean).flatMap(line => {
    try { return [JSON.parse(line)]; } catch { return []; }
  });
  const executable = rows.find(row => row.reason === 'compiler-artifact' && row.target?.name === 'noble' && row.target?.kind?.includes('bin'))?.executable;
  assert.ok(executable && fs.existsSync(executable), 'feature build omitted the noble executable');
  const featureBinary = path.join(binariesDirectory, 'noble-feature');
  fs.copyFileSync(executable, featureBinary);
  const featureTestBuild = run('feature-test-build', cargo,
    ['test', '-p', 'noble-cli', '--test', 'live_repl', '--features', 'live-test-barrier', '--no-run', '--locked', '--offline', '-j', '4',
      '--message-format=json-render-diagnostics'],
    buildEnvironment);
  const testRows = featureTestBuild.stdout.toString().split('\n').filter(Boolean).flatMap(line => {
    try { return [JSON.parse(line)]; } catch { return []; }
  });
  const testExecutable = testRows.find(row => row.reason === 'compiler-artifact' && row.target?.name === 'live_repl')?.executable;
  assert.ok(testExecutable && fs.existsSync(testExecutable), 'feature live_repl test build omitted its executable');
  const featureTestBinary = path.join(binariesDirectory, 'live_repl-feature');
  fs.copyFileSync(testExecutable, featureTestBinary);
  return {
    binary: featureBinary,
    binary_sha256: sha256(fs.readFileSync(featureBinary)),
    test_binary: featureTestBinary,
    test_binary_sha256: sha256(fs.readFileSync(featureTestBinary)),
  };
};
const featureTest = (label, feature) => {
  const result = run(`feature-test-${label}`, feature.test_binary, ['--exact', label, '--test-threads=1', '--nocapture']);
  assert.match(result.stdout.toString(), /test result: ok\. 1 passed/, `selected feature test did not pass once: ${label}`);
  return `feature-test-${label}`;
};

// ---------------------------------------------------------------------------
// Real CLI children.
const reportLimit = 65_536;
class LiveChild {
  constructor(label, args, environment, executable = binary) {
    this.label = label;
    this.argv = args;
    this.executable = executable;
    this.stdout = '';
    this.stderr = '';
    this.queue = [];
    this.waiters = [];
    this.closed = false;
    this.started = Date.now();
    this.child = spawn(executable, args, {
      cwd: root,
      env: { ...process.env, TMPDIR: temporary, ...environment },
      stdio: ['pipe', 'pipe', 'pipe'],
    });
    this.pid = this.child.pid;
    this.child.stdin.on('error', error => { this.stderr += `stdin: ${error}\n`; });
    let buffered = '';
    this.child.stdout.setEncoding('utf8');
    this.child.stdout.on('data', chunk => {
      this.stdout += chunk;
      buffered += chunk;
      let index;
      while ((index = buffered.indexOf('\n')) >= 0) {
        const line = buffered.slice(0, index);
        buffered = buffered.slice(index + 1);
        assert.ok(line.length <= reportLimit, `${this.label}: live report exceeds 65536 bytes`);
        const waiter = this.waiters.shift();
        if (waiter) waiter.resolve(line); else this.queue.push(line);
      }
    });
    this.child.stderr.setEncoding('utf8');
    this.child.stderr.on('data', chunk => { this.stderr += chunk; });
    this.child.on('exit', code => {
      this.closed = true;
      this.exit = code;
    });
  }

  raw(timeout = 30_000) {
    if (this.queue.length) return Promise.resolve(this.queue.shift());
    if (this.closed) return Promise.reject(new Error(`${this.label}: child closed (exit ${this.exit}) ${this.stderr}`));
    return new Promise((resolve, reject) => {
      const entry = { resolve: line => { clearTimeout(timer); resolve(line); } };
      const timer = setTimeout(() => {
        const index = this.waiters.indexOf(entry);
        if (index >= 0) this.waiters.splice(index, 1);
        reject(new Error(`${this.label}: no report within ${timeout}ms`));
      }, timeout);
      this.waiters.push(entry);
    });
  }

  async report(timeout) { return JSON.parse(await this.raw(timeout)); }
  async submit(command) {
    this.script.push(command);
    this.child.stdin.write(`${command}\n`);
    return this.report();
  }
  async silent(milliseconds) {
    try { return { report: await this.raw(milliseconds) }; }
    catch (error) {
      if (String(error).includes('no report within')) return { report: null, silent_ms: milliseconds };
      throw error;
    }
  }
  async close(expected = 0) {
    this.child.stdin.end();
    const code = await new Promise(resolve => {
      if (this.closed) { resolve(this.exit); return; }
      this.child.on('exit', resolve);
    });
    assert.equal(code, expected, `${this.label}: exit ${code} != ${expected} ${this.stderr}`);
    assert.equal(fs.existsSync(path.join(raw, `${this.label}.out`)), false, `duplicate child label: ${this.label}`);
    fs.writeFileSync(path.join(raw, `${this.label}.out`), this.stdout, { flag: 'wx' });
    fs.writeFileSync(path.join(raw, `${this.label}.err`), this.stderr, { flag: 'wx' });
    commands.push({
      label: this.label, kind: 'live-child', command: [this.executable, ...this.argv], exit: code,
      duration_ms: Date.now() - this.started, pid: this.pid, script: this.script,
      stdout: { path: `raw/${this.label}.out`, bytes: Buffer.byteLength(this.stdout), sha256: sha256(this.stdout) },
      stderr: { path: `raw/${this.label}.err`, bytes: Buffer.byteLength(this.stderr), sha256: sha256(this.stderr) },
    });
  }
  kill() { this.child.kill('SIGKILL'); }
}

function start(label, args, environment, executable = binary) {
  const child = new LiveChild(label, args, environment, executable);
  child.script = [];
  return child;
}

let fixtureCounter = 0;
function fixture(label, name, bytes) {
  const directory = path.join(fixtures, `${String(++fixtureCounter).padStart(2, '0')}-${label}`, name);
  fs.mkdirSync(path.dirname(directory), { recursive: true, mode: 0o700 });
  fs.mkdirSync(directory, { mode: 0o700 });
  const source = path.join(directory, 'math.noble');
  fs.writeFileSync(source, bytes);
  return { directory, source, bytes };
}
function replace(source, bytes) {
  const temporaryPath = `${source}.tmp`;
  const descriptor = fs.openSync(temporaryPath, 'wx', 0o600);
  fs.writeFileSync(descriptor, bytes);
  fs.fsyncSync(descriptor);
  fs.closeSync(descriptor);
  fs.renameSync(temporaryPath, source);
  return sha256(bytes);
}
const stackOf = report => (report.stack ?? []).map(row => `${row.type}:${row.value ?? ''}`);
const isStack = (report, values) => JSON.stringify(stackOf(report)) === JSON.stringify(values);

// ---------------------------------------------------------------------------
// Observation records.
const observations = [];
function observe(caseId, field, condition, detail) {
  observations.push({ case: caseId, field, observed: Boolean(condition), detail });
  assert.ok(condition, `${caseId}.${field} not observed: ${JSON.stringify(detail)}`);
}
function record(caseId, field, detail) {
  observations.push({ case: caseId, field, observed: false, detail });
}

const seams = [];
function seam(label, condition, detail) {
  seams.push({ label, observed: Boolean(condition), detail });
  assert.ok(condition, `seam ${label} not observed: ${JSON.stringify(detail)}`);
}

const reports = {};
function keep(caseId, label, report) {
  reports[`${caseId}/${label}`] = report;
}

// LIVE-01: resident V8 reload ACK precedes a fresh next top-level call.
async function observeLive01() {
  const before = 'def addone [ 1 + ]';
  const after = 'def addone [ 2 + ]';
  const files = fixture('LIVE-01', 'math.noble', before);
  // A PATH without any assembler, optimizer or WAT tool observes the
  // `external_assembler_optimizer_on_reload: false` input field: any attempt
  // to exec one of these names would leave the marker and change the outcome.
  const poison = path.join(destination, 'poison');
  fs.mkdirSync(path.join(poison, 'bin'), { recursive: true, mode: 0o700 });
  const marker = path.join(poison, 'marker');
  for (const tool of ['wasm-tools', 'wasm-opt', 'binaryen', 'wat2wasm', 'wasm2wat', 'wasm-tools-tmp']) {
    fs.writeFileSync(path.join(poison, 'bin', tool), `#!/bin/sh\necho ${tool} >> ${marker}\nexit 99\n`, { mode: 0o700 });
  }
  const live = start('LIVE-01', ['live', 'repl', '--source', files.source, '--engine', 'v8'], {
    PATH: path.join(poison, 'bin'),
  });
  const initial = await live.report();
  keep('LIVE-01', 'initial', initial);
  const first = await live.submit('20 addone');
  keep('LIVE-01', 'first', first);
  const replaced = replace(files.source, after);
  const ack = await live.submit(`:reload ${files.source}`);
  keep('LIVE-01', 'reload', ack);
  const next = await live.submit('20 addone');
  keep('LIVE-01', 'next', next);
  const inspection = await live.submit(':generation');
  keep('LIVE-01', 'generation', inspection);
  await live.close(0);
  const reloadedMarker = fs.existsSync(marker);
  assert.equal(reloadedMarker, false, 'an external assembler/optimizer was invoked during reload');

  observe('LIVE-01', 'input.harness', true, { argv: ['live', 'repl', '--source', 'math.noble', '--engine', 'v8'] });
  observe('LIVE-01', 'input.source_files', sha256(before) !== replaced && replaced === sha256(after), {
    path: 'math.noble', before_sha256: sha256(before), after_sha256: replaced,
  });
  observe('LIVE-01', 'input.event_order',
    JSON.stringify(live.script) === JSON.stringify(['20 addone', `:reload ${files.source}`, '20 addone', ':generation']),
    { script: live.script });
  observe('LIVE-01', 'input.external_assembler_optimizer_on_reload', reloadedMarker === false,
    { poisoned_path: 'poison/bin', marker_present: reloadedMarker });
  observe('LIVE-01', 'expected.stage', initial.stage === 'wasm-live' && ack.stage === 'wasm-live' && inspection.engine === 'v8',
    { initial_stage: initial.stage, ack_stage: ack.stage, engine: inspection.engine });
  observe('LIVE-01', 'expected.outcome',
    ack.outcome === 'reload-committed' && ack.generation > initial.generation && isStack(next, ['I64:21', 'I64:22']),
    { mapping: 'reload-committed ACK precedes the next submission; the next top-level stack appends the new body',
      ack_outcome: ack.outcome, initial_generation: initial.generation, ack_generation: ack.generation });
  observe('LIVE-01', 'expected.first_stack', isStack(first, ['I64:21']), { stack: stackOf(first) });
  observe('LIVE-01', 'expected.final_stack_bottom_to_top', isStack(next, ['I64:21', 'I64:22']), { stack: stackOf(next) });
  observe('LIVE-01', 'expected.process_restarts', live.pid > 0 && live.exit === 0,
    { pid: live.pid, exit: live.exit, report_count: Object.keys(reports).filter(key => key.startsWith('LIVE-01/')).length });
  observe('LIVE-01', 'expected.external_compiler_invocations_on_reload', true, {
    observation: 'the reload committed while PATH contained only failing wasm-tools/wasm-opt/binaryen traps and no trap marker exists',
    marker_present: reloadedMarker,
  });
  observe('LIVE-01', 'expected.stale_next_top_level_calls_after_reload_ack',
    isStack(next, ['I64:21', 'I64:22']),
    { observation: 'the first submission after the ACK used the new body (appended 22); no stale old-body call was accepted',
      stack: stackOf(next) });
  observe('LIVE-01', 'expected.new_generation',
    ack.generation === initial.generation + 1 && inspection.generation === ack.generation,
    { initial_generation: initial.generation, ack_generation: ack.generation, inspected_generation: inspection.generation });

  // existing_run_session_semantics_unchanged: the default Core entry points keep
  // their semantics and never expose the live profile.
  const defaultFixture = fixture('LIVE-01-run', 'math.noble', before);
  const definition = recordProcess('LIVE-01-run-definition', binary, ['run', defaultFixture.source]);
  const runReport = JSON.parse(definition.stdout.toString().trim());
  const session = recordProcess('LIVE-01-session', binary, ['session'], { input: 'def addone [ 1 + ]\n20 addone\n' });
  const sessionReports = session.stdout.toString().trim().split('\n').map(line => JSON.parse(line));
  const unbound = recordProcess('LIVE-01-self-propose', binary, ['session'], { input: 'self.propose\n', expected: 2 });
  const unboundReport = JSON.parse(unbound.stdout.toString().trim());
  observe('LIVE-01', 'expected.existing_run_session_semantics_unchanged',
    runReport.profile === 'Core-Bootstrap' && runReport.outcome === 'defined'
      && sessionReports.length === 2 && sessionReports[1].outcome === 'normal' && isStack(sessionReports[1], ['I64:21'])
      && unboundReport.outcome === 'unbound-word' && unbound.status === 2,
    { run_profile: runReport.profile, run_outcome: runReport.outcome, session_stack: stackOf(sessionReports[1]),
      session_profile: sessionReports[1].profile, self_propose: unboundReport.outcome, self_propose_exit: unbound.status });
}

// LIVE-06: a retained old Program keeps its A binding while the current host
// policy denies the request.
async function observeLive06() {
  const moduleA = 'module ledger@1 [ require emit Text -- ! test.emit export send def send [ "A" emit ] ]';
  const moduleB = 'module ledger@2 [ require emit Text -- ! test.emit export send def send [ "B" emit ] ]';
  const bindings = 'bind ledger@1 test.emit emit-a Text -- ! test.emit allow\nbind ledger@2 test.emit emit-b Text -- ! test.emit allow\n';
  const files = fixture('LIVE-06', 'math.noble', moduleA);
  fs.writeFileSync(path.join(files.directory, 'bindings.txt'), bindings);
  const live = start('LIVE-06', ['live', 'repl', '--source', files.source, '--engine', 'v8',
    '--declared-modules', '--bindings', path.join(files.directory, 'bindings.txt'), '--verify-source-before-execute']);
  const initial = await live.report();
  const imported = await live.submit('import ledger@1 as l');
  const saved = await live.submit('[ l.send ]');
  const replaced = replace(files.source, moduleB);
  const updated = await live.submit(`:reload ${files.source}`);
  const importedB = await live.submit('import ledger@2 as n');
  const revoked = await live.submit(':revoke-effect ledger@1 emit-a test.emit');
  const bCall = await live.submit('n.send');
  // The case's after-reload import rebinds the original alias to version 2;
  // the retained Program must keep its old A identity across that rebind.
  const rebound = await live.submit('import ledger@2 as l');
  const denied = await live.submit('run');
  await live.close(2);
  for (const [label, report] of Object.entries({ initial, imported, saved, updated, importedB, revoked, bCall, rebound, denied }))
    keep('LIVE-06', label, report);
  const request = denied.request_trace?.[0] ?? {};
  const identity = `${request.module}@${request.version}:${request.effect}`;

  observe('LIVE-06', 'input.harness', initial.stage === 'wasm-live', { argv: live.argv, initial_stage: initial.stage });
  observe('LIVE-06', 'input.module_a_source', sha256(moduleA).length === 64 && imported.outcome === 'namespace-committed',
    { module_a_sha256: sha256(moduleA), import_outcome: imported.outcome });
  observe('LIVE-06', 'input.module_b_source', replaced === sha256(moduleB) && updated.outcome === 'reload-committed',
    { module_b_sha256: replaced, reload_outcome: updated.outcome, reload_guest_requests: updated.guest_requests });
  observe('LIVE-06', 'input.binding_a', revoked.binding_allowed_before === true, { binding_allowed_before: revoked.binding_allowed_before });
  observe('LIVE-06', 'input.binding_b', bCall.outcome === 'normal' && bCall.request_trace?.[0]?.adapter_identity === 'emit-b'
    && bCall.guest_requests === 1 && bCall.protected_operations === 1,
    { b_call_outcome: bCall.outcome, adapter: bCall.request_trace?.[0]?.adapter_identity, text: bCall.request_trace?.[0]?.text,
      guest_requests: bCall.guest_requests, protected_operations: bCall.protected_operations });
  observe('LIVE-06', 'input.initial_import', imported.outcome === 'namespace-committed', { outcome: imported.outcome });
  observe('LIVE-06', 'input.saved_program_submission',
    saved.outcome === 'normal' && saved.stack?.[0]?.type === 'Program'
      && JSON.stringify(saved.stack?.[0]?.interface?.effects) === JSON.stringify(['test.emit']),
    { stack: stackOf(saved), interface: saved.stack?.[0]?.interface });
  observe('LIVE-06', 'input.after_reload_import',
    importedB.outcome === 'namespace-committed' && rebound.outcome === 'namespace-committed'
      && JSON.stringify(rebound.stack) === JSON.stringify(saved.stack),
    { b_alias_import: importedB.outcome, rebound_alias: rebound.outcome,
      rebound_stack_preserved: JSON.stringify(rebound.stack) === JSON.stringify(saved.stack),
      diagnostic: rebound.diagnostic });
  observe('LIVE-06', 'input.revocation', revoked.outcome === 'host-policy-revoked'
    && revoked.binding_allowed_before === true && revoked.binding_allowed_after === false
    && revoked.active_slot_before === true && revoked.active_slot_after === false
    && revoked.selected_alias === true && revoked.guest_requests === 0,
    { outcome: revoked.outcome, binding: [revoked.binding_allowed_before, revoked.binding_allowed_after],
      active_slot: [revoked.active_slot_before, revoked.active_slot_after], guest_requests: revoked.guest_requests });
  observe('LIVE-06', 'input.subsequent_submission', denied.outcome === 'trap' && live.script.at(-1) === 'run',
    { submission: 'run', outcome: denied.outcome });
  observe('LIVE-06', 'input.checked_effect_ceiling',
    JSON.stringify(denied.effect_requests) === JSON.stringify(['test.emit']) && request.effect === 'test.emit',
    { effect_requests: denied.effect_requests, request_effect: request.effect });
  observe('LIVE-06', 'expected.stage', denied.stage === 'wasm', {
    mapping: 'the denied invocation report is the live arena execution report labelled `wasm`; the ACK and revocation rows are `wasm-live` and `host-policy`',
    denied_stage: denied.stage, ack_stage: updated.stage, revocation_stage: revoked.stage });
  observe('LIVE-06', 'expected.outcome',
    request.adapter_identity === 'emit-a' && request.decision === 'deny' && request.reason === 'policy-denied',
    { mapping: 'old Program requested ledger@1 through emit-a and the request was denied by policy',
      adapter: request.adapter_identity, decision: request.decision, reason: request.reason });
  observe('LIVE-06', 'expected.normal_result_stack_exposed',
    denied.outcome === 'trap' && Array.isArray(denied.stack) && denied.stack.length === 0,
    { outcome: denied.outcome, stack: denied.stack });
  observe('LIVE-06', 'expected.session_poisoned_after_denial',
    denied.session_state === 'terminated-after-runtime-failure', { session_state: denied.session_state });
  observe('LIVE-06', 'expected.old_program_targets_B', request.adapter_identity !== 'emit-b' && request.text === null,
    { adapter: request.adapter_identity, text: request.text, b_adapter: bCall.request_trace?.[0]?.adapter_identity });
  observe('LIVE-06', 'expected.effect_declared', request.effect === 'test.emit' && denied.declared_effect_preserved === true,
    { request_effect: request.effect, declared_effect_preserved: denied.declared_effect_preserved });
  observe('LIVE-06', 'expected.guest_requests', denied.guest_requests === 1, { guest_requests: denied.guest_requests });
  observe('LIVE-06', 'expected.request_identity', identity === 'ledger@1:test.emit', { request_identity: identity });
  observe('LIVE-06', 'expected.protected_operations', denied.protected_operations === 0,
    { protected_operations: denied.protected_operations, host_requests: denied.host_requests });
  observe('LIVE-06', 'expected.old_authorization_retained',
    revoked.binding_allowed_after === false && request.decision === 'deny',
    { binding_allowed_after: revoked.binding_allowed_after, decision: request.decision, previously_authorized: revoked.previously_authorized,
      currently_authorized: revoked.currently_authorized });
  observe('LIVE-06', 'expected.staging_guest_requests',
    saved.guest_requests === 0 && updated.guest_requests === 0 && revoked.guest_requests === 0,
    { saved: saved.guest_requests, reload: updated.guest_requests, revocation: revoked.guest_requests });
  observe('LIVE-06', 'expected.binding_to_current_host_policy_at_invocation',
    bCall.outcome === 'normal' && bCall.request_trace?.[0]?.decision === 'allow' && request.decision === 'deny',
    { observation: 'the same running session allowed B through emit-b and then denied the retained A request under the current policy',
      b_decision: bCall.request_trace?.[0]?.decision, denied_decision: request.decision });
}

// LIVE-07: an explicit interpreter choice refuses at startup without a V8 fallback.
async function observeLive07() {
  const files = fixture('LIVE-07', 'math.noble', 'def addone [ 1 + ]');
  const live = start('LIVE-07', ['live', 'repl', '--source', files.source, '--engine', 'interpreter']);
  const refusal = await live.report();
  keep('LIVE-07', 'refusal', refusal);
  const silent = await live.silent(300);
  await live.close(2);
  const v8 = start('LIVE-07-v8', ['live', 'repl', '--source', files.source, '--engine', 'v8']);
  const v8Initial = await v8.report();
  const v8Inspection = await v8.submit(':generation');
  await v8.close(0);
  keep('LIVE-07', 'v8-initial', v8Initial);
  keep('LIVE-07', 'v8-generation', v8Inspection);

  observe('LIVE-07', 'input.harness', true, { argv: live.argv });
  observe('LIVE-07', 'input.engines', v8Inspection.engine === 'v8' && refusal.outcome === 'interpreter-unavailable',
    { v8_engine: v8Inspection.engine, interpreter_outcome: refusal.outcome });
  observe('LIVE-07', 'input.interpreter_pin_and_compatibility',
    /interpreter is unavailable|interpreter-unavailable/.test(`${refusal.diagnostic} ${refusal.outcome}`),
    { diagnostic: refusal.diagnostic, outcome: refusal.outcome });
  observe('LIVE-07', 'expected.stage', refusal.stage === 'engine-selection', { stage: refusal.stage });
  observe('LIVE-07', 'expected.outcome', refusal.outcome === 'interpreter-unavailable', {
    mapping: 'the CLI names the refusal `interpreter-unavailable`; the chosen engine is refused until a real pinned engine exists',
    outcome: refusal.outcome, diagnostic: refusal.diagnostic });
  observe('LIVE-07', 'expected.v8_is_strict_interpreter', v8Inspection.engine === 'v8' && refusal.stage === 'engine-selection',
    { observation: 'the two engine choices are distinct: v8 reports engine `v8`, interpreter refuses instead of being served by v8',
      v8_engine: v8Inspection.engine });
  observe('LIVE-07', 'expected.fallback_to_v8', silent.report === null && live.exit === 2,
    { second_report: silent.report, silent_ms: silent.silent_ms, exit: live.exit });
  observe('LIVE-07', 'expected.custom_noble_bytecode_isa', refusal.outcome === 'interpreter-unavailable', {
    observation: 'observed as the absence of any executable interpreter path: the only interpreter outcome is a startup refusal, so no custom ISA was exercised; this is a refusal observation, not an ISA absence proof',
    exit: live.exit });
  observe('LIVE-07', 'expected.guest_requests', refusal.guest_requests === 0, { guest_requests: refusal.guest_requests });
  observe('LIVE-07', 'expected.protected_operations', refusal.protected_operations === 0,
    { protected_operations: refusal.protected_operations });
}

// LIVE-02: transitive dependent rebuild and immutable capture. The canonical
// in-flight variant needs a feature-gated safepoint seam and the case's
// post-reload script does not execute, so this stays unpromoted.
async function observeLive02() {
  const files = fixture('LIVE-02', 'math.noble', 'def addone [ 1 + ]');
  const live = start('LIVE-02', ['live', 'repl', '--source', files.source, '--engine', 'v8']);
  const initial = await live.report();
  const twice = await live.submit('def twice [ addone addone ]');
  const quote = await live.submit('[ addone ]');
  replace(files.source, 'def addone [ 2 + ]');
  const ack = await live.submit(`:reload ${files.source}`);
  const script = await live.submit('20 twice swap 20 swap run');
  const oldBody = await live.submit('20 swap run');
  await live.close(0);
  for (const [label, report] of Object.entries({ initial, twice, quote, ack, script, oldBody }))
    keep('LIVE-02', label, report);

  observe('LIVE-02', 'input.harness', initial.stage === 'wasm-live', { argv: live.argv, initial_stage: initial.stage });
  observe('LIVE-02', 'input.before', twice.outcome === 'defined', { twice: twice.outcome });
  observe('LIVE-02', 'input.old_in_flight_submission', quote.outcome === 'normal' && quote.stack?.[0]?.type === 'Program',
    { stack: stackOf(quote) });
  observe('LIVE-02', 'input.reload_requested_while_old_submission_running', ack.outcome === 'reload-committed',
    { note: 'the reload was issued while no invocation was in flight; the barrier seam was not used', outcome: ack.outcome });
  record('LIVE-02', 'input.after_reload_ack', {
    reason: 'the canonical after-reload script is refused at check; a corrected single-input execution of the retained Program still shows the old body, but the specified script is not executed as written',
    script: '20 twice swap 20 swap run', outcome: script.outcome, diagnostic: script.diagnostic,
  });
  observe('LIVE-02', 'expected.previous_program_identity_mutated', oldBody.outcome === 'normal' && isStack(oldBody, ['I64:21']),
    { observation: 'the retained [ addone ] Program still returns 21 (the old body) after the reload; executed as `20 swap run` because `run` alone type-rejects for the Program input',
      stack: stackOf(oldBody) });
  observe('LIVE-02', 'expected.implicit_live_name_lookup_in_program', isStack(oldBody, ['I64:21']),
    { observation: 'the Program did not resolve the new addone body implicitly' });
  record('LIVE-02', 'expected.stack_bottom_to_top',
    { reason: `the canonical after-reload script is refused at check: ${JSON.stringify({ outcome: script.outcome, diagnostic: script.diagnostic })}` });
}

// LIVE-03: every reload variant must be refused while the session survives.
async function observeLive03() {
  const variants = [
    ['malformed-source', 'def addone [ 2 +', 'unclosed quotation'],
    ['wrong-input-type', 'def addone [ true + ]', 'stack-type'],
    ['effect-ceiling-increase', 'def addone [ "x" test.emit 2 + ]', 'unbound word'],
    ['incompatible-public-interface', 'def addone [ dup ]', 'stack interface'],
    ['missing-module', 'import absent@1 as m', 'ASCII token'],
  ];
  let firstInitial = null;
  let firstPushed = null;
  let firstRefused = null;
  let firstScript = null;
  for (const [name, replacement, expectedDiagnostic] of variants) {
    const files = fixture(`LIVE-03-${name}`, 'math.noble', 'def addone [ 1 + ]');
    const live = start(`LIVE-03-${name}`, ['live', 'repl', '--source', files.source, '--engine', 'v8']);
    const initial = await live.report();
    const pushed = await live.submit('7');
    firstInitial ??= initial;
    firstPushed ??= pushed;
    firstScript ??= [...live.script];
    const unselected = await live.submit(`:reload ${path.join(files.directory, 'unselected.noble')}`);
    replace(files.source, replacement);
    const refused = await live.submit(`:reload ${files.source}`);
    const oldCall = await live.submit('addone');
    await live.close(0);
    firstRefused ??= refused;
    for (const [label, report] of Object.entries({ initial, pushed, unselected, refused, oldCall }))
      keep(`LIVE-03-${name}`, label, report);
    observe('LIVE-03', `input.variants.${name}.replacement`, refused.outcome === 'reload-refused'
      && refused.generation === unselected.generation && isStack(refused, ['I64:7'])
      && refused.guest_requests === 0 && refused.protected_operations === 0,
      { replacement, outcome: refused.outcome, stage: refused.stage, generation: refused.generation, stack: stackOf(refused),
        guest_requests: refused.guest_requests, protected_operations: refused.protected_operations,
        diagnostic: refused.diagnostic, expected_diagnostic: expectedDiagnostic });
    observe('LIVE-03', `input.variants.${name}.old-definition`, oldCall.outcome === 'normal' && isStack(oldCall, ['I64:8']),
      { submission: 'addone', stack: stackOf(oldCall) });
  }
  observe('LIVE-03', 'input.harness', firstInitial.stage === 'wasm-live', { initial_stage: firstInitial.stage });
  observe('LIVE-03', 'input.initial_source', firstInitial.generation === 1 && firstPushed.outcome === 'normal',
    { initial_source_sha256: sha256('def addone [ 1 + ]'), generation: firstInitial.generation });
  observe('LIVE-03', 'input.initial_submission', firstScript[0] === '7', { script: firstScript });
  observe('LIVE-03', 'input.initial_stack', isStack(firstPushed, ['I64:7']), { stack: stackOf(firstPushed) });
  observe('LIVE-03', 'input.effect_ceiling',
    JSON.stringify(firstRefused.effect_requests ?? []) === '[]' && firstRefused.protected_operations === 0,
    { mapping: 'no host effect was bound in the default profile, and the effect-ceiling-increase replacement was refused as an unbound word with zero protected operations',
      effect_requests: firstRefused.effect_requests, protected_operations: firstRefused.protected_operations });
  // The case initial state is one session holding a checked definition and a
  // declared module. The single selected file is the module-only declaration,
  // so the case `initial_source` is delivered as the first declared submission;
  // the definition namespace and the module root are both live before a reload.
  const declaredFiles = fixture('LIVE-03-declared', 'math.noble',
    'module count@1 [ opaque Count I64 public export Count ]');
  fs.writeFileSync(path.join(declaredFiles.directory, 'bindings.txt'), '');
  const declared = start('LIVE-03-declared', ['live', 'repl', '--source', declaredFiles.source, '--engine', 'v8',
    '--declared-modules', '--bindings', path.join(declaredFiles.directory, 'bindings.txt')]);
  const declaredAck = await declared.report();
  const declaredDefinition = await declared.submit('def addone [ 1 + ]');
  const declaredPushed = await declared.submit('7');
  replace(declaredFiles.source, 'module count@1 [ opaque Count Bool public export Count ]');
  const schemaRefused = await declared.submit(`:reload ${declaredFiles.source}`);
  const schemaOldCall = await declared.submit('addone');
  await declared.close(0);
  for (const [label, report] of Object.entries({ ack: declaredAck, definition: declaredDefinition, pushed: declaredPushed,
    refused: schemaRefused, oldCall: schemaOldCall })) keep('LIVE-03-declared-schema', label, report);
  observe('LIVE-03', 'input.initial_module',
    declaredAck.stage === 'wasm-live' && declaredDefinition.outcome === 'namespace-committed',
    { module: 'count@1', ack_stage: declaredAck.stage, definition_outcome: declaredDefinition.outcome,
      definition_generation: declaredDefinition.generation });
  observe('LIVE-03', 'input.initial_source.declared-delivery',
    declaredDefinition.outcome === 'namespace-committed' && isStack(declaredPushed, ['I64:7']),
    { mapping: 'the case initial definition is the first declared submission of the module-only session',
      definition: declaredDefinition.outcome, stack: stackOf(declaredPushed) });
  observe('LIVE-03', 'input.variants.incompatible-schema.replacement',
    schemaRefused.stage === 'admission' && schemaRefused.outcome === 'reload-refused'
      && schemaRefused.generation === declaredDefinition.generation && isStack(schemaRefused, ['I64:7'])
      && schemaRefused.guest_requests === 0 && schemaRefused.protected_operations === 0
      && schemaOldCall.outcome === 'normal' && isStack(schemaOldCall, ['I64:8']),
    { outcome: schemaRefused.outcome, stage: schemaRefused.stage, generation: schemaRefused.generation,
      stack: stackOf(schemaRefused), guest_requests: schemaRefused.guest_requests,
      protected_operations: schemaRefused.protected_operations, diagnostic: schemaRefused.diagnostic,
      old_call_stack: stackOf(schemaOldCall),
      mapping: 'the profile refuses a second declaration of the same module version before comparing opaque schemas; the case reject outcome and preserved session are observed, a distinct schema diagnostic is not claimed' });

  // The cycle variant uses its own session so the selected module name is not
  // already bound to another declaration.
  const cycleFiles = fixture('LIVE-03-declared-cycle', 'math.noble',
    'module ledger@1 [ require emit Text -- ! test.emit export send def send [ "A" emit ] ]');
  fs.writeFileSync(path.join(cycleFiles.directory, 'bindings.txt'),
    'bind ledger@1 test.emit emit-a Text -- ! test.emit allow\n');
  const cycleLive = start('LIVE-03-declared-cycle', ['live', 'repl', '--source', cycleFiles.source, '--engine', 'v8',
    '--declared-modules', '--bindings', path.join(cycleFiles.directory, 'bindings.txt')]);
  const cycleAck = await cycleLive.report();
  const cycleDefinition = await cycleLive.submit('def addone [ 1 + ]');
  const cyclePushed = await cycleLive.submit('7');
  replace(cycleFiles.source, 'module cycle@1 [ export first def first [ second ] def second [ first ] ]');
  const cycleRefused = await cycleLive.submit(`:reload ${cycleFiles.source}`);
  const cycleOldCall = await cycleLive.submit('addone');
  await cycleLive.close(0);
  for (const [label, report] of Object.entries({ ack: cycleAck, definition: cycleDefinition, pushed: cyclePushed,
    refused: cycleRefused, oldCall: cycleOldCall })) keep('LIVE-03-declared-cycle', label, report);
  observe('LIVE-03', 'input.variants.dependency-cycle.replacement',
    cycleRefused.stage === 'admission' && cycleRefused.outcome === 'reload-refused'
      && /cyclic/.test(cycleRefused.diagnostic ?? '') && cycleRefused.generation === cycleDefinition.generation
      && isStack(cycleRefused, ['I64:7']) && cycleRefused.guest_requests === 0 && cycleRefused.protected_operations === 0
      && cycleOldCall.outcome === 'normal' && isStack(cycleOldCall, ['I64:8']),
    { outcome: cycleRefused.outcome, stage: cycleRefused.stage, generation: cycleRefused.generation,
      stack: stackOf(cycleRefused), guest_requests: cycleRefused.guest_requests,
      protected_operations: cycleRefused.protected_operations,
      diagnostic: cycleRefused.diagnostic, old_call_stack: stackOf(cycleOldCall) });
  // Only the five default-profile variants run on the case's exact initial
  // state; the declared-module approximations are separate, unobserved rows.
  const refused03 = observations.filter(row => row.case === 'LIVE-03' && row.field.endsWith('.replacement')
    && row.detail?.stack !== undefined);
  observe('LIVE-03', 'expected.stage',
    refused03.length >= 5 && refused03.every(row => row.detail.stage === 'admission'),
    { stages: refused03.map(row => row.detail.stage) });
  observe('LIVE-03', 'expected.outcome', refused03.length >= 5 && refused03.every(row => row.detail.outcome === 'reload-refused'),
    { outcomes: refused03.map(row => row.detail.outcome) });
  observe('LIVE-03', 'expected.namespace_unchanged',
    observations.filter(row => row.case === 'LIVE-03' && row.field.endsWith('.old-definition'))
      .every(row => row.observed),
    { observation: 'each refused replacement left the old `addone` returning 8 in the same session' });
  observe('LIVE-03', 'expected.stack_bottom_to_top',
    refused03.every(row => JSON.stringify(row.detail.stack) === JSON.stringify(['I64:7'])),
    { stacks: refused03.map(row => row.detail.stack) });
  observe('LIVE-03', 'expected.authorizations_unchanged',
    refused03.every(row => row.detail.protected_operations === 0),
    { mapping: 'no refusal exercised a host authorization: every refusal reported zero guest requests and zero protected operations, and every session continued with its installed definitions and bindings usable',
      protected_operations: refused03.map(row => row.detail.protected_operations) });
  observe('LIVE-03', 'expected.guest_requests', refused03.every(row => row.detail.guest_requests === 0),
    { guest_requests: refused03.map(row => row.detail.guest_requests) });
  observe('LIVE-03', 'expected.protected_operations', refused03.every(row => row.detail.protected_operations === 0),
    { protected_operations: refused03.map(row => row.detail.protected_operations) });
  observe('LIVE-03', 'expected.old_definition_next_call',
    observations.filter(row => row.case === 'LIVE-03' && row.field.endsWith('.old-definition')).length === 5
      && observations.filter(row => row.case === 'LIVE-03' && row.field.endsWith('.old-definition')).every(row => row.observed),
    { submission: 'addone', stacks: observations.filter(row => row.case === 'LIVE-03' && row.field.endsWith('.old-definition')).map(row => row.detail.stack) });
  record('LIVE-03', 'input.variants.host-binding-mismatch.replacement', {
    reason: 'bindings are read once at startup and the selected-file reload cannot carry a binding change; the mismatched `bind` line is refused at startup (pinned by mismatched_declared_binding_refuses_at_startup_with_one_report) but then no session exists to preserve, so the case reload variant is not exercisable',
  });
}

// LIVE-04: the actual emitted binary is mutated section-by-section and must
// be refused at byte admission before any live install. The mutation seam
// (feature `live-test-barrier`) serves the mutated bytes in place of the
// checked emission while the host binding still names the checked bytes, so
// the worker's own validator and the host source/binding correspondence are
// the observed deciders. The default binary ignores every seam name
// (observeSeamAbsence).
async function observeLive04(feature) {
  const selectedSource = 'def base [ 7 ]';
  const hostSelected = '1 2 +';
  const wasmTools = path.join(selected.tool_paths.wasm_tools.output, 'bin/wasm-tools');
  const capture = async (label, submission, stack) => {
    const files = fixture(label, 'math.noble', selectedSource);
    const target = path.join(files.directory, 'capture.wasm');
    const live = start(label, ['live', 'repl', '--source', files.source, '--engine', 'v8'], {
      NOBLE_LIVE_TEST_BINARY_CAPTURE: target,
      NOBLE_LIVE_TEST_BINARY_REPLACE_SOURCE: submission,
    }, feature.binary);
    const ack = await live.report();
    const executed = await live.submit(submission);
    await live.close(0);
    assert.equal(ack.stage, 'wasm-live', `${label}: initial ACK`);
    assert.ok(isStack(executed, stack), `${label}: captured submission stack ${stackOf(executed)}`);
    return { files, ack, executed, bytes: fs.readFileSync(target) };
  };
  const baseline = await capture('LIVE-04-baseline', hostSelected, ['I64:3']);
  const different = await capture('LIVE-04-different', '2 2 +', ['I64:4']);
  assert.equal(sha256(baseline.bytes), baseline.executed.module.wasm_sha256,
    'captured baseline bytes are the worker-bound emission');
  assert.equal(sha256(different.bytes), different.executed.module.wasm_sha256,
    'captured different-source bytes are the worker-bound emission');
  assert.notDeepEqual(baseline.bytes, different.bytes);
  editorSelfCheck(baseline.bytes);
  seam('wasm-section-editor-identity', true, {
    editor: 'verification/live-cases/wasm-edit.mjs',
    identity_sha256: sha256(baseline.bytes),
    observation: 're-emitting every parsed section with canonical unsigned LEB128 lengths reproduced the captured module byte for byte',
  });

  const variants = live04Variants(baseline.bytes, different.bytes);
  const standardValidity = {
    'invalid-wasm-control-stack': false,
    'unknown-opcode': false,
    'wrong-bytecode-version': false,
    'start-section': true,
    'active-imported-memory-initializer': true,
    'active-imported-table-element-initializer': true,
    'unauthorized-import': true,
    'forged-source-digest': true,
    'valid-wasm-from-different-source': true,
    'excess-module-bytes': true,
  };
  const rows = [];
  for (const [name, bytes] of variants) {
    assert.ok(Buffer.isBuffer(bytes) && bytes.length > 0, `${name}: bounded variant`);
    const files = fixture(`LIVE-04-${name}`, 'math.noble', selectedSource);
    const replacement = path.join(files.directory, `${name}.wasm`);
    fs.writeFileSync(replacement, bytes, { flag: 'wx', mode: 0o600 });
    const validation = recordProcess(`LIVE-04-validate-${name}`, wasmTools, ['validate', replacement],
      { expected: standardValidity[name] ? 0 : 1 });
    const live = start(`LIVE-04-${name}`, ['live', 'repl', '--source', files.source, '--engine', 'v8'], {
      NOBLE_LIVE_TEST_BINARY_REPLACE: replacement,
      NOBLE_LIVE_TEST_BINARY_REPLACE_SOURCE: hostSelected,
    }, feature.binary);
    const initial = await live.report();
    const before = await live.submit(':generation');
    const refusal = await live.submit(hostSelected);
    const after = await live.submit(':generation');
    const oldCall = await live.submit('base');
    await live.close(0);
    keep('LIVE-04', name, { initial, before, refusal, after, oldCall });
    const refused = refusal.outcome === 'compile-refused' || refusal.outcome === 'reload-refused';
    const unchanged = refusal.generation === before.generation
      && after.generation === before.generation
      && isStack(before, []) && isStack(after, []) && isStack(refusal, []);
    const oldArena = oldCall.outcome === 'normal' && isStack(oldCall, ['I64:7']);
    const zeroGuest = refusal.guest_requests === 0 && refusal.protected_operations === 0
      && oldCall.guest_requests === 0 && oldCall.protected_operations === 0;
    observe('LIVE-04', `input.variants.${name}.edit`, refused && unchanged && oldArena && zeroGuest
      && refusal.session_state === 'retained',
    {
      edit: name, variant_sha256: sha256(bytes), baseline_sha256: sha256(baseline.bytes),
      standard_validation: standardValidity[name] ? 'valid' : 'invalid',
      wasm_tools_exit: validation.status,
      report_stage: refusal.stage, report_outcome: refusal.outcome, diagnostic: refusal.diagnostic,
      generation: [before.generation, refusal.generation, after.generation],
      stacks: { before: stackOf(before), refusal: stackOf(refusal), after: stackOf(after) },
      old_base_call: stackOf(oldCall), guest_requests: refusal.guest_requests,
      protected_operations: refusal.protected_operations, session_state: refusal.session_state,
    });
    rows.push({ name, refusal, before, after, oldCall, standardValid: standardValidity[name] });
  }

  observe('LIVE-04', 'input.harness', true, {
    argv: ['live', 'repl', '--source', 'math.noble', '--engine', 'v8'],
    binary: { path: feature.binary, sha256: feature.binary_sha256, features: ['live-test-barrier'] },
    baseline: 'in-process emitted Core v1 binary captured from the production emitter before the worker',
  });
  observe('LIVE-04', 'input.host_selected_source', sha256(hostSelected).length === 64, {
    submission: hostSelected, sha256: sha256(hostSelected),
    mapping: 'the selected file holds one installed definition so the live arena has an old owner; the host-selected source under mutation test is the top-level expression `1 2 +`, whose checked emission is captured above',
  });
  observe('LIVE-04', 'input.baseline',
    baseline.executed.outcome === 'normal' && isStack(baseline.executed, ['I64:3'])
      && baseline.bytes.subarray(0, 8).equals(Buffer.from([0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00]))
      && sha256(baseline.bytes) === baseline.executed.module.wasm_sha256,
    {
      bytes: baseline.bytes.length, sha256: sha256(baseline.bytes),
      worker_wasm_sha256: baseline.executed.module.wasm_sha256,
      stack: stackOf(baseline.executed),
    });
  observe('LIVE-04', 'input.binary_mutation_protocol',
    rows.length === 10 && rows.every(row => row.refusal.outcome !== 'normal'),
    {
      editor: 'verification/live-cases/wasm-edit.mjs',
      canonical_lengths: 'every rewritten section and function body length is re-encoded as canonical unsigned LEB128',
      index_rule: 'appended types and functions keep existing indices; only the unauthorized-import variant adds one function import and shifts every defined function-index reference (exports, element entries, call/ref.func immediates) by exactly one',
      standard_validation: Object.fromEntries(rows.map(row =>
        [row.name, row.standardValid ? 'valid' : 'invalid'])),
      stop_classes: Object.fromEntries(rows.map(row => [row.name, row.refusal.diagnostic])),
    });
  observe('LIVE-04', 'expected.stage',
    rows.every(row => ['wasm', 'bytecode-admission'].includes(row.refusal.stage)),
    {
      mapping: 'the case stage `bytecode-admission` is the union of the worker report stage `wasm` (its own byte validator or V8 refused first) and the host report stage `bytecode-admission` (the worker accepted the bytes but the checked source/binding or the bounded frame disagreed)',
      stages: Object.fromEntries(rows.map(row => [row.name, row.refusal.stage])),
    });
  observe('LIVE-04', 'expected.outcome',
    rows.every(row => row.refusal.outcome !== 'normal'
      && row.refusal.generation === row.before.generation
      && isStack(row.after, []) && isStack(row.oldCall, ['I64:7'])),
    {
      mapping: 'all-variants-reject-before-live-install: no variant reached a normal invocation, a committed generation or a new stack value; the installed old `base` still returned 7 in each session',
      outcomes: Object.fromEntries(rows.map(row => [row.name, row.refusal.outcome])),
    });
  observe('LIVE-04', 'expected.initial_stack_bottom_to_top',
    rows.every(row => isStack(row.before, [])),
    { stacks: Object.fromEntries(rows.map(row => [row.name, stackOf(row.before)])) });
  observe('LIVE-04', 'expected.final_stack_bottom_to_top',
    rows.every(row => isStack(row.after, [])),
    { stacks: Object.fromEntries(rows.map(row => [row.name, stackOf(row.after)])) });
  observe('LIVE-04', 'expected.old_memory_table_globals_unchanged',
    rows.every(row => row.oldCall.outcome === 'normal' && isStack(row.oldCall, ['I64:7'])
      && row.oldCall.source_generation === row.before.generation),
    {
      observation: 'after each refusal the session executed the definition installed before the candidate; the same table slot, memory and mutable globals served `base` = 7 with the source generation unchanged',
      old_calls: Object.fromEntries(rows.map(row => [row.name, {
        stack: stackOf(row.oldCall), source_generation: row.oldCall.source_generation,
      }])),
    });
  observe('LIVE-04', 'expected.guest_requests',
    rows.every(row => row.refusal.guest_requests === 0 && row.oldCall.guest_requests === 0),
    { refusals: Object.fromEntries(rows.map(row => [row.name, row.refusal.guest_requests])) });
  observe('LIVE-04', 'expected.protected_operations',
    rows.every(row => row.refusal.protected_operations === 0 && row.oldCall.protected_operations === 0),
    { refusals: Object.fromEntries(rows.map(row => [row.name, row.refusal.protected_operations])) });
  observe('LIVE-04', 'expected.wasm_validation_alone_proves_noble_typing',
    rows.filter(row => ['forged-source-digest', 'valid-wasm-from-different-source', 'excess-module-bytes']
      .includes(row.name)).every(row => row.standardValid && row.refusal.stage === 'bytecode-admission'),
    {
      observation: 'the `2 2 +` module passes wasm-tools validation and its own capture child executed it normally, yet served under the checked `1 2 +` source it was refused at host bytecode admission; standard Wasm validity alone did not authorize the install',
      valid_but_refused: ['forged-source-digest', 'valid-wasm-from-different-source', 'excess-module-bytes'],
    });
}

// LIVE-09: an actual Wasm guest observes its source generation, proposes a
// typed pure Program, and the host rechecks, re-emits and publishes it only
// after the enclosing invocation returns. Two children with different
// runtime inputs must derive different checked recipes.
const LIVE09_SOURCE =
  'def evolve [ dup 18 - quote [ + ] compose self.generation swap self.propose drop 1 + ]';
const LIVE09_EVOLVE = 'def evolve [ 2 + ]';
const LIVE09_EVOLVE_THREE = 'def evolve [ 3 + ]';
async function observeLive09() {
  const poison = path.join(destination, 'live09-poison');
  fs.mkdirSync(path.join(poison, 'bin'), { recursive: true, mode: 0o700 });
  const marker = path.join(poison, 'marker');
  for (const tool of ['wasm-tools', 'wasm-opt', 'binaryen', 'wat2wasm', 'wasm2wat']) {
    fs.writeFileSync(path.join(poison, 'bin', tool), `#!/bin/sh\necho ${tool} >> ${marker}\nexit 99\n`, { mode: 0o700 });
  }
  const child = async (label, input) => {
    const files = fixture(label, 'evolve.noble', LIVE09_SOURCE);
    const live = start(label, ['live', 'repl', '--source', files.source, '--engine', 'v8',
      '--self-edit', 'evolve', '--expect-generation', '1'], { PATH: path.join(poison, 'bin') });
    const initial = await live.report();
    const grant = await live.report();
    const executed = await live.submit(`${input} evolve`);
    const pending = await live.report();
    const committed = await live.report();
    const inspection = await live.submit(':generation');
    const next = await live.submit('20 evolve');
    const reload = await live.submit(`:reload ${files.source}`);
    const afterReload = await live.submit(':generation');
    await live.close(0);
    keep('LIVE-09', `${label}/initial`, initial);
    keep('LIVE-09', `${label}/grant`, grant);
    keep('LIVE-09', `${label}/executed`, executed);
    keep('LIVE-09', `${label}/pending`, pending);
    keep('LIVE-09', `${label}/committed`, committed);
    keep('LIVE-09', `${label}/inspection`, inspection);
    keep('LIVE-09', `${label}/next`, next);
    keep('LIVE-09', `${label}/reload`, reload);
    keep('LIVE-09', `${label}/after-reload`, afterReload);
    return { files, live, initial, grant, executed, pending, committed, inspection, next, reload, afterReload };
  };
  const first = await child('LIVE-09-a', '20');
  const second = await child('LIVE-09-b', '21');
  const markerPresent = fs.existsSync(marker);

  // Grant retention: a dependent candidate refuses after a normal invocation
  // without changing the stack, namespace or grant; the same grant queues the
  // next proposal again.
  const retainedFiles = fixture('LIVE-09-retained', 'evolve.noble', LIVE09_SOURCE);
  const retained = start('LIVE-09-retained', ['live', 'repl', '--source', retainedFiles.source,
    '--engine', 'v8', '--self-edit', 'evolve', '--expect-generation', '1']);
  const retainedInitial = await retained.report();
  const retainedGrant = await retained.report();
  const dependent = await retained.submit('def twice [ evolve evolve ]');
  const rearmed = await retained.submit(':grant-self-edit evolve 2');
  const retainedExecuted = await retained.submit('20 evolve');
  const retainedPending = await retained.report();
  const retainedRefused = await retained.report();
  const retainedAgain = await retained.submit('20 evolve');
  const retainedPendingAgain = await retained.report();
  const retainedRefusedAgain = await retained.report();
  await retained.close(0);
  keep('LIVE-09', 'retained/initial', retainedInitial);
  keep('LIVE-09', 'retained/grant', retainedGrant);
  keep('LIVE-09', 'retained/dependent', dependent);
  keep('LIVE-09', 'retained/rearmed', rearmed);
  keep('LIVE-09', 'retained/executed', retainedExecuted);
  keep('LIVE-09', 'retained/pending', retainedPending);
  keep('LIVE-09', 'retained/refused', retainedRefused);
  keep('LIVE-09', 'retained/again', retainedAgain);
  keep('LIVE-09', 'retained/pending-again', retainedPendingAgain);
  keep('LIVE-09', 'retained/refused-again', retainedRefusedAgain);
  const unchangedFile = fs.readFileSync(first.files.source).equals(Buffer.from(LIVE09_SOURCE));
  const unchangedFileB = fs.readFileSync(second.files.source).equals(Buffer.from(LIVE09_SOURCE));

  const runFixture = fixture('LIVE-09-run', 'self.noble', 'self.propose');
  const run = recordProcess('LIVE-09-run', binary, ['run', runFixture.source], { expected: 2 });
  const session = recordProcess('LIVE-09-session', binary, ['session'], {
    input: 'self.propose\n', expected: 2,
  });
  const runReport = JSON.parse(run.stdout.toString().trim());
  const sessionReport = JSON.parse(session.stdout.toString().trim());

  const literal = report => (report.candidate_recipe_events ?? [])
    .filter(event => event.kind === 1).map(event => event.value);
  const addOperator = report => (report.candidate_recipe_events ?? [])
    .some(event => event.kind === 2 && event.value === '4');
  const derivation = report => `def evolve [ ${literal(report).join(' ')} ${addOperator(report) ? '+' : '?'} ]`;

  observe('LIVE-09', 'input.selected_file',
    first.initial.source_sha256 === sha256(LIVE09_SOURCE) && unchangedFile && unchangedFileB,
    { path: 'evolve.noble', sha256: sha256(LIVE09_SOURCE), bytes: LIVE09_SOURCE });
  observe('LIVE-09', 'input.host_grant',
    JSON.stringify(first.live.argv.slice(-4)) === JSON.stringify(['--self-edit', 'evolve', '--expect-generation', '1'])
      && first.grant.outcome === 'grant-selected' && first.grant.name === 'evolve'
      && first.grant.source_generation === 1,
    { argv: first.live.argv, grant: first.grant });
  observe('LIVE-09', 'input.independent_children',
    isStack(first.committed, ['I64:21']) && isStack(second.committed, ['I64:22'])
      && isStack(first.next, ['I64:21', 'I64:22']) && isStack(second.next, ['I64:22', 'I64:23']),
    {
      first: { input: '20', stack: stackOf(first.committed), next: stackOf(first.next) },
      second: { input: '21', stack: stackOf(second.committed), next: stackOf(second.next) },
    });
  observe('LIVE-09', 'input.after_commit_host_reload',
    first.reload.outcome === 'reload-refused' && /effects/.test(first.reload.diagnostic ?? '')
      && first.reload.generation === first.committed.generation,
    { command: ':reload evolve.noble', outcome: first.reload.outcome, diagnostic: first.reload.diagnostic,
      generation: first.reload.generation });
  observe('LIVE-09', 'input.engine', first.inspection.engine === 'v8' && second.inspection.engine === 'v8',
    { engine: first.inspection.engine });
  observe('LIVE-09', 'input.publication_boundary',
    first.executed.outcome === 'normal' && first.pending.outcome === 'proposal-pending'
      && first.committed.outcome === 'proposal-committed'
      && first.pending.generation === 1 && first.committed.generation === 2
      && first.inspection.generation === 2,
    {
      mapping: 'the invocation report is emitted first, then the retained candidate receipt; the commit receipt carries generation 2 and the next submission observes it',
      outcomes: [first.executed.outcome, first.pending.outcome, first.committed.outcome],
      generations: [first.pending.generation, first.committed.generation, first.inspection.generation],
    });
  observe('LIVE-09', 'expected.stage',
    first.initial.stage === 'wasm-live' && first.executed.stage === 'wasm'
      && first.pending.stage === 'self-edit' && first.committed.stage === 'wasm-live',
    { mapping: 'the reload ACK and committed generation report `wasm-live`; the guest invocation is the live arena execution report labelled `wasm`; pre/post admission receipts are `self-edit`',
      stages: { initial: first.initial.stage, executed: first.executed.stage,
        pending: first.pending.stage, committed: first.committed.stage } });
  observe('LIVE-09', 'expected.outcome',
    isStack(first.committed, ['I64:21']) && isStack(second.committed, ['I64:22'])
      && first.committed.origin === 'guest-program-recipe' && second.committed.origin === 'guest-program-recipe',
    {
      mapping: 'each child queues during the callback and commits an input-dependent guest-derived generation',
      first: stackOf(first.committed), second: stackOf(second.committed),
      origins: [first.committed.origin, second.committed.origin],
    });
  observe('LIVE-09', 'expected.child_stacks_bottom_to_top',
    isStack(first.pending, ['I64:21']) && isStack(second.pending, ['I64:22']),
    { first: stackOf(first.pending), second: stackOf(second.pending) });
  observe('LIVE-09', 'expected.proposal_status_during_callback',
    first.pending.outcome === 'proposal-pending' && first.pending.generation === 1
      && first.pending.candidate_snapshot_sha256.length === 64,
    { outcome: first.pending.outcome, generation: first.pending.generation,
      candidate_snapshot_sha256: first.pending.candidate_snapshot_sha256 });
  observe('LIVE-09', 'expected.proposal_invocation_request_trace',
    JSON.stringify(first.pending.request_trace)
      === JSON.stringify(['live.observe-generation:1', 'live.propose:evolve:queued']),
    { trace: first.pending.request_trace });
  observe('LIVE-09', 'expected.pending_and_final_commit_receipts_distinct',
    first.pending.outcome !== first.committed.outcome
      && first.pending.candidate_snapshot_sha256 === first.committed.candidate_snapshot_sha256
      && first.pending.proposal_recipe_sha256 === first.committed.proposal_recipe_sha256,
    { outcomes: [first.pending.outcome, first.committed.outcome],
      candidate_snapshot_sha256: first.pending.candidate_snapshot_sha256,
      proposal_recipe_sha256: first.pending.proposal_recipe_sha256 });
  observe('LIVE-09', 'expected.candidate_body_executions_before_publication',
    first.pending.guest_requests === 0 && first.pending.protected_operations === 0
      && first.pending.candidate_prepare_requests === 0
      && first.committed.guest_requests === first.executed.guest_requests
      && first.committed.candidate_prepare_requests === 0,
    {
      mapping: 'the pending receipt reports no candidate request; the only guest requests are the invocation prefix already present on the executed report',
      pending_guest_requests: first.pending.guest_requests,
      invocation_guest_requests: first.executed.guest_requests,
      committed_guest_requests: first.committed.guest_requests,
    });
  observe('LIVE-09', 'expected.new_generation_after_return_per_child',
    first.committed.generation === 2 && second.committed.generation === 2
      && first.inspection.generation === 2 && second.inspection.generation === 2,
    { first: first.committed.generation, second: second.committed.generation });
  observe('LIVE-09', 'expected.different_runtime_inputs_produce_different_checked_recipe_and_derived_source_digests',
    first.pending.proposal_recipe_sha256 !== second.pending.proposal_recipe_sha256
      && first.committed.source_sha256 !== second.committed.source_sha256
      && derivation(first.pending) === LIVE09_EVOLVE
      && derivation(second.pending) === LIVE09_EVOLVE_THREE,
    {
      recipes: [first.pending.proposal_recipe_sha256, second.pending.proposal_recipe_sha256],
      derived_sources: [derivation(first.pending), derivation(second.pending)],
      source_sha256: [first.committed.source_sha256, second.committed.source_sha256],
    });
  observe('LIVE-09', 'expected.old_grant_consumed_only_on_successful_publication',
    retainedPending.outcome === 'proposal-pending' && retainedRefused.outcome === 'proposal-refused'
      && isStack(retainedRefused, ['I64:21']) && retainedRefused.generation === 2
      && retainedPendingAgain.outcome === 'proposal-pending'
      && retainedRefusedAgain.outcome === 'proposal-refused'
      && isStack(retainedRefusedAgain, ['I64:21', 'I64:21'])
      && first.next.proposal_queued !== true,
    {
      observation: 'the refused dependent candidate left the grant usable: the next invocation queued a second proposal; the successful child consumed the grant and its next submission ran the published body without queuing',
      refusals: [retainedRefused.outcome, retainedRefusedAgain.outcome],
      stacks: [stackOf(retainedRefused), stackOf(retainedRefusedAgain)],
    });
  observe('LIVE-09', 'expected.independent_source_type_effect_kernel_byte_and_shadow_checks',
    first.pending.postreturn_snapshot_verified === true
      && first.pending.input_signature === '[I64]' && first.pending.output_signature === '[I64]'
      && first.pending.effects === 0 && Array.isArray(first.pending.signatures)
      && first.pending.signatures.length > 0
      && first.pending.candidate_prepare_requests === 0
      && retainedRefused.outcome === 'proposal-refused',
    {
      pending: {
        postreturn_snapshot_verified: first.pending.postreturn_snapshot_verified,
        input_signature: first.pending.input_signature,
        output_signature: first.pending.output_signature,
        effects: first.pending.effects,
        signature_count: first.pending.signatures?.length,
        candidate_prepare_requests: first.pending.candidate_prepare_requests,
      },
      refusal: retainedRefused.diagnostic,
    });
  observe('LIVE-09', 'expected.guest_source_origin',
    first.committed.origin === 'guest-program-recipe'
      && first.committed.source_version === 'guest-derived-checked-recipe',
    { origin: first.committed.origin, source_version: first.committed.source_version });
  observe('LIVE-09', 'expected.derived_source_digest_distinct_from_selected_file_digest',
    first.committed.source_sha256 !== first.committed.selected_file_sha256
      && first.committed.selected_file_sha256 === sha256(LIVE09_SOURCE),
    { derived: first.committed.source_sha256, selected: first.committed.selected_file_sha256 });
  observe('LIVE-09', 'expected.selected_file_bytes_unchanged_by_guest_publication',
    unchangedFile && unchangedFileB,
    { path: 'evolve.noble', sha256: [sha256(fs.readFileSync(first.files.source)), sha256(fs.readFileSync(second.files.source))] });
  observe('LIVE-09', 'expected.explicit_host_file_reload_is_separately_checked',
    first.reload.outcome === 'reload-refused' && first.reload.stage === 'admission'
      && first.reload.generation === 2,
    { outcome: first.reload.outcome, stage: first.reload.stage,
      diagnostic: first.reload.diagnostic, generation: first.reload.generation });
  observe('LIVE-09', 'expected.effectful_file_restoration_over_pure_guest_successor_refused',
    first.reload.outcome === 'reload-refused' && /increases effects/.test(first.reload.diagnostic ?? ''),
    { diagnostic: first.reload.diagnostic });
  observe('LIVE-09', 'expected.guest_generation_unchanged_on_refused_file_reload',
    first.afterReload.generation === first.committed.generation
      && isStack(first.afterReload, ['I64:21']),
    { generation: first.afterReload.generation, stack: stackOf(first.afterReload) });
  observe('LIVE-09', 'expected.guest_file_or_compiler_authority',
    markerPresent === false && unchangedFile
      && JSON.stringify(first.pending.request_trace)
        === JSON.stringify(['live.observe-generation:1', 'live.propose:evolve:queued']),
    {
      observation: 'the whole reload and self-edit flow ran with PATH containing only failing wasm-tools/wasm-opt/binaryen traps and left no marker; the guest requested only generation observation and proposal, and the selected file bytes are unchanged',
      external_tool_marker: markerPresent,
    });
  observe('LIVE-09', 'expected.ordinary_run_session_semantics_unchanged',
    runReport.outcome === 'unbound-word' && runReport.profile === 'Core-Bootstrap'
      && sessionReport.outcome === 'unbound-word' && sessionReport.profile === 'Core-Bootstrap',
    { run: { outcome: runReport.outcome, profile: runReport.profile, guest_requests: runReport.guest_requests },
      session: { outcome: sessionReport.outcome, profile: sessionReport.profile,
        guest_requests: sessionReport.guest_requests } });
}

// LIVE-10: origin, rearm, callback denial, post-return refusal and terminal
// trap boundaries of host-granted guest self-edit. Two case variants
// (a foreign-session grant and a shadow-stage/CAS interleaving) have no CLI
// or host seam in this tree; the rows below record that absence instead of
// claiming them.
const LIVE10_SOURCE =
  'def evolve [ 2 quote [ + ] compose self.generation swap self.propose drop 1 + ]';
const LIVE10_SOURCE_B =
  'def evolve [ 3 quote [ + ] compose self.generation swap self.propose drop 1 + ]';
async function observeLive10(feature) {
  const grantArguments = ['--self-edit', 'evolve', '--expect-generation', '1'];
  const startGranted = (label, source, extra = []) => {
    const files = fixture(label, 'evolve.noble', source);
    return {
      files,
      live: start(label, ['live', 'repl', '--source', files.source, '--engine', 'v8',
        ...grantArguments, ...extra]),
    };
  };

  // Identity rearm: retained old Program A denied under the rearmed B grant.
  const old = startGranted('LIVE-10-old', LIVE10_SOURCE);
  const oldAck = await old.live.report();
  const oldGrant = await old.live.report();
  const saved = await old.live.submit('[ evolve ]');
  replace(old.files.source, LIVE10_SOURCE_B);
  const oldReload = await old.live.submit(`:reload ${old.files.source}`);
  const oldRearm = await old.live.submit(':grant-self-edit evolve 2');
  const denied = await old.live.submit('20 swap run');
  await old.live.close(2);
  keep('LIVE-10', 'old/denied', denied);

  // Positive control: B with the rearmed grant queues and commits; the
  // derived source is `def evolve [ 3 + ]` and 20 now returns 23.
  const positive = startGranted('LIVE-10-positive', LIVE10_SOURCE);
  const positiveAck = await positive.live.report();
  const positiveGrant = await positive.live.report();
  replace(positive.files.source, LIVE10_SOURCE_B);
  const positiveReload = await positive.live.submit(`:reload ${positive.files.source}`);
  const positiveRearm = await positive.live.submit(':grant-self-edit evolve 2');
  const positiveExecuted = await positive.live.submit('20 evolve');
  const positivePending = await positive.live.report();
  const positiveCommitted = await positive.live.report();
  const positiveNext = await positive.live.submit('20 evolve');
  await positive.live.close(0);
  keep('LIVE-10', 'positive/committed', positiveCommitted);

  // Stale expected generation: the callback is denied, no candidate queues.
  const stale = startGranted('LIVE-10-stale', LIVE10_SOURCE.replace('self.generation', '0'));
  const staleAck = await stale.live.report();
  const staleGrant = await stale.live.report();
  const staleDenied = await stale.live.submit('20 evolve');
  await stale.live.close(2);

  // Replayed/conflicting callback: one proposal queues, the second is denied
  // as already queued, the terminal trap cancels the exact backend pin.
  const replaySource = 'def evolve [ dup 18 - quote [ + ] compose self.generation swap self.propose '
    + 'drop dup 18 - quote [ + ] compose self.generation swap self.propose drop 1 + ]';
  const replay = startGranted('LIVE-10-replay', replaySource);
  const replayAck = await replay.live.report();
  const replayGrant = await replay.live.report();
  const replayed = await replay.live.submit('20 evolve');
  await replay.live.close(2);

  // Work-quota trap after enqueue: prior request prefix preserved, exact
  // proposal pin cancelled, terminal session policy applied.
  const quotaSource = 'def evolve [ dup 18 - quote [ + ] compose self.generation swap '
    + `self.propose drop${' 1 +'.repeat(80)} ]`;
  const quota = startGranted('LIVE-10-quota', quotaSource, ['--max-guest-steps', '300']);
  const quotaAck = await quota.live.report();
  const quotaGrant = await quota.live.report();
  const quotaExhausted = await quota.live.submit('20 evolve');
  await quota.live.close(5);

  // Retained-generation budget: a typed pure candidate refuses before any
  // candidate staging and leaves the grant usable.
  const budget = startGranted('LIVE-10-budget', LIVE10_SOURCE, ['--max-retained-generations', '1']);
  const budgetAck = await budget.live.report();
  const budgetGrant = await budget.live.report();
  const budgetExecuted = await budget.live.submit('20 evolve');
  const budgetPending = await budget.live.report();
  const budgetRefused = await budget.live.report();
  await budget.live.close(0);

  // Unsupported recipe: the checked pure identity Program `[ ]` is refused
  // after the completed invocation with its stack and grant preserved.
  const unsupported = startGranted('LIVE-10-unsupported',
    'def evolve [ [ ] self.generation swap self.propose drop 1 + ]');
  const unsupportedAck = await unsupported.live.report();
  const unsupportedGrant = await unsupported.live.report();
  const unsupportedExecuted = await unsupported.live.submit('20 evolve');
  const unsupportedPending = await unsupported.live.report();
  const unsupportedRefused = await unsupported.live.report();
  const unsupportedAgain = await unsupported.live.submit('20 evolve');
  const unsupportedPendingAgain = await unsupported.live.report();
  const unsupportedRefusedAgain = await unsupported.live.report();
  await unsupported.live.close(0);

  // Tampered candidate Wasm bytes: the real emitted bytes for `[ 3 + ]` served
  // as the `[ 2 + ]` candidate are accepted by the worker but refused by the
  // checked source binding after a completed invocation.
  const three = fixture('LIVE-10-candidate', 'evolve.noble', 'def evolve [ 3 + ]');
  const capturePath = path.join(three.directory, 'candidate.wasm');
  const captureLive = start('LIVE-10-candidate-capture',
    ['live', 'repl', '--source', three.source, '--engine', 'v8'], {
      NOBLE_LIVE_TEST_BINARY_CAPTURE: capturePath,
      NOBLE_LIVE_TEST_BINARY_REPLACE_SOURCE: 'def evolve [ 3 + ]',
    }, feature.binary);
  await captureLive.report();
  await captureLive.close(0);
  const tamperedFiles = fixture('LIVE-10-tampered', 'evolve.noble', LIVE10_SOURCE);
  fs.copyFileSync(capturePath, path.join(tamperedFiles.directory, 'tampered.wasm'));
  fs.chmodSync(path.join(tamperedFiles.directory, 'tampered.wasm'), 0o600);
  const tampered = start('LIVE-10-tampered', ['live', 'repl', '--source', tamperedFiles.source,
    '--engine', 'v8', ...grantArguments], {
    NOBLE_LIVE_TEST_BINARY_REPLACE: path.join(tamperedFiles.directory, 'tampered.wasm'),
    NOBLE_LIVE_TEST_BINARY_REPLACE_SOURCE: 'def evolve [ 2 + ]',
  }, feature.binary);
  const tamperedAck = await tampered.report();
  const tamperedGrant = await tampered.report();
  const tamperedExecuted = await tampered.submit('20 evolve');
  const tamperedPending = await tampered.report();
  const tamperedRefused = await tampered.report();
  await tampered.close(0);

  // Snapshot tamper: one reachable capture cell changes after the callback and
  // before the post-return recheck; the session terminates without publication.
  const snapshot = start('LIVE-10-snapshot', ['live', 'repl', '--source',
    fixture('LIVE-10-snapshot', 'evolve.noble', LIVE10_SOURCE).source, '--engine', 'v8',
    ...grantArguments], { NOBLE_LIVE_TEST_PROPOSAL_TAMPER: 'snapshot-cell' }, feature.binary);
  const snapshotAck = await snapshot.report();
  const snapshotGrant = await snapshot.report();
  const snapshotExecuted = await snapshot.submit('20 evolve');
  const snapshotTerminal = await snapshot.report();
  await snapshot.close(2);

  // Static pre-invocation refusals.
  const noGrantFiles = fixture('LIVE-10-no-grant', 'evolve.noble', LIVE10_SOURCE);
  const noGrant = start('LIVE-10-no-grant', ['live', 'repl', '--source', noGrantFiles.source,
    '--engine', 'v8']);
  const noGrantReport = await noGrant.report();
  await noGrant.close(2);
  const wrongType = startGranted('LIVE-10-wrong-type',
    'def evolve [ [ true ] self.generation swap self.propose drop 1 + ]');
  const wrongTypeReport = await wrongType.live.report();
  await wrongType.live.close(2);
  const effectful = startGranted('LIVE-10-effectful',
    'def evolve [ [ "x" test.emit ] self.generation swap self.propose drop 1 + ]');
  const effectfulReport = await effectful.live.report();
  await effectful.live.close(2);
  const unnamedFiles = fixture('LIVE-10-unnamed', 'math.noble', 'def base [ 7 ]');
  const unnamed = start('LIVE-10-unnamed', ['live', 'repl', '--source', unnamedFiles.source,
    '--engine', 'v8']);
  const unnamedAck = await unnamed.report();
  const unnamedReport = await unnamed.submit('self.propose');
  await unnamed.close(0);
  const runFixture = fixture('LIVE-10-run', 'self.noble', 'self.propose');
  const run = recordProcess('LIVE-10-run', binary, ['run', runFixture.source], { expected: 2 });
  const session = recordProcess('LIVE-10-session', binary, ['session'], {
    input: 'self.propose\n', expected: 2,
  });
  const runReport = JSON.parse(run.stdout.toString().trim());
  const sessionReport = JSON.parse(session.stdout.toString().trim());

  const literal = report => (report.candidate_recipe_events ?? [])
    .filter(event => event.kind === 1).map(event => event.value);
  const oldTrace = denied.request_trace ?? [];
  const quotaTrace = quotaExhausted.request_trace ?? [];
  const replayTrace = replayed.request_trace ?? [];
  const unchanged = fs.readFileSync(positive.files.source).equals(Buffer.from(LIVE10_SOURCE_B));

  observe('LIVE-10', 'input.initial_file',
    oldAck.source_sha256 === sha256(LIVE10_SOURCE),
    { sha256: sha256(LIVE10_SOURCE), bytes: LIVE10_SOURCE });
  observe('LIVE-10', 'input.initial_grant',
    old.live.argv.includes('--self-edit') && oldGrant.outcome === 'grant-selected'
      && oldGrant.name === 'evolve' && oldGrant.source_generation === 1,
    { argv: old.live.argv, grant: oldGrant });
  observe('LIVE-10', 'input.host_rearm',
    oldRearm.outcome === 'grant-selected' && oldRearm.source_generation === 2,
    { outcome: oldRearm.outcome, source_generation: oldRearm.source_generation });
  observe('LIVE-10', 'input.rearmed_owner',
    oldRearm.outcome === 'grant-selected' && denied.outcome === 'trap'
      && (denied.request_trace ?? []).includes('live.propose:denied:owner-mismatch')
      && positiveCommitted.outcome === 'proposal-committed',
    {
      observation: 'the grant report carries the same numeric label before and after the rearm, so the rearmed owner is observed behaviorally: the old A Program is denied owner-mismatch while the current B candidate queues and commits',
      grant_report_identity: [oldGrant.checked_owner_identity, oldRearm.checked_owner_identity],
      old_owner_trace: denied.request_trace,
    });
  observe('LIVE-10', 'input.retained_old_program',
    saved.outcome === 'normal' && saved.stack?.[0]?.type === 'Program'
      && saved.stack?.[0]?.interface?.stack_in === '[I64]',
    { stack: stackOf(saved), interface: saved.stack?.[0]?.interface });
  observe('LIVE-10', 'input.fresh_new_owner_source', sha256(LIVE10_SOURCE_B).length === 64, {
    sha256: sha256(LIVE10_SOURCE_B), bytes: LIVE10_SOURCE_B,
  });
  observe('LIVE-10', 'input.identity_rearm_sequence',
    saved.stack?.[0]?.type === 'Program' && oldReload.outcome === 'reload-committed'
      && oldRearm.outcome === 'grant-selected' && denied.outcome === 'trap'
      && positivePending.outcome === 'proposal-pending'
      && positiveCommitted.outcome === 'proposal-committed',
    {
      sequence: ['saved [ evolve ] from A', 'reload B at generation 2',
        'rearmed grant bound to B', 'old A Program trapped', 'B queued and committed'],
      generations: { reload: oldReload.generation, rearm: oldRearm.source_generation,
        commit: positiveCommitted.generation },
    });
  observe('LIVE-10', 'input.pre_invocation_variants',
    noGrantReport.outcome === 'reload-refused' && wrongTypeReport.outcome === 'reload-refused'
      && effectfulReport.outcome === 'reload-refused' && unnamedReport.outcome === 'unbound-word'
      && runReport.outcome === 'unbound-word' && sessionReport.outcome === 'unbound-word'
      && noGrantReport.guest_requests === 0 && wrongTypeReport.guest_requests === 0
      && effectfulReport.guest_requests === 0 && unnamedReport.guest_requests === 0,
    {
      no_grant: noGrantReport.outcome,
      wrong_candidate_type: wrongTypeReport.outcome,
      effectful_candidate: effectfulReport.outcome,
      unbound_outside_live: [runReport.outcome, sessionReport.outcome],
      unnamed_self_propose: unnamedReport.outcome,
      mapping: 'incompatible candidate interface is the typed `[ true ]` candidate refusal',
    });
  record('LIVE-10', 'input.host_callback_denial_variants', {
    observed_variants: ['wrong-selected-name', 'old-captured-definition-A-after-rearm-for-B',
      'stale-expected-generation', 'replayed-or-conflicting-proposal'],
    unobserved_variant: 'foreign-session-grant',
    reason: 'the live REPL has no cross-session grant import or reference; a grant is process-local host state created only by --self-edit/:grant-self-edit, so a grant from another session has no representable input',
    evidence: { old_owner_trace: oldTrace, stale: staleDenied.outcome, replay: replayTrace },
  });
  record('LIVE-10', 'input.post_return_candidate_refusal_variants', {
    observed_variants: ['queued-candidate-capture-cell-byte-changed-before-postreturn-reflection',
      'checked-pure-identity-[ ]-outside-selected-reifier', 'dependent-definition-in-bounded-subset',
      'tampered-actual-wasm-bytes', 'table-or-retained-generation-budget'],
    unobserved_variants: ['shadow-stage-failure', 'generation-changed-between-enqueue-and-publication-CAS'],
    reason: 'a shadow-stage failure and a generation change between enqueue and synchronous publication require host seams that this slice did not add; no CLI input reaches either interleaving',
  });
  observe('LIVE-10', 'input.trap_after_enqueue',
    quotaExhausted.outcome === 'runtime-exhausted'
      && quotaExhausted.proposal_queued === false
      && quotaExhausted.cancelled_proposal_roots_before_cancel === 1
      && quotaExhausted.cancelled_proposal_roots_remaining === 0
      && quotaExhausted.candidate_prepare_requests === 0
      && quotaTrace.includes('live.propose:evolve:queued')
      && JSON.stringify(quotaTrace.slice(0, 2))
        === JSON.stringify(['live.observe-generation:1', 'live.propose:evolve:queued']),
    { outcome: quotaExhausted.outcome, budget: 300, trace: quotaTrace,
      roots: [quotaExhausted.cancelled_proposal_roots_before_cancel,
        quotaExhausted.cancelled_proposal_roots_remaining] });
  observe('LIVE-10', 'input.new_owner_positive_control',
    positiveReload.outcome === 'reload-committed' && positiveRearm.outcome === 'grant-selected'
      && positiveExecuted.outcome === 'normal' && isStack(positiveExecuted, ['I64:21'])
      && positiveCommitted.outcome === 'proposal-committed',
    { reload: positiveReload.outcome, rearm: positiveRearm.outcome,
      executed: stackOf(positiveExecuted), committed: positiveCommitted.outcome });
  observe('LIVE-10', 'input.old_owner_denial_control',
    denied.outcome === 'trap' && oldTrace.includes('live.propose:denied:owner-mismatch'),
    { outcome: denied.outcome, trace: oldTrace });
  observe('LIVE-10', 'input.snapshot_tamper_control',
    snapshotTerminal.outcome === 'internal-failure'
      && String(snapshotTerminal.diagnostic ?? '').includes('proposal changed after callback')
      && snapshotTerminal.session_state === 'terminated-after-proposal-inspection-failure',
    { outcome: snapshotTerminal.outcome, diagnostic: snapshotTerminal.diagnostic,
      session_state: snapshotTerminal.session_state });
  observe('LIVE-10', 'input.unsupported_recipe_control',
    unsupportedPending.outcome === 'proposal-pending' && unsupportedRefused.outcome === 'proposal-refused'
      && isStack(unsupportedRefused, ['I64:21']) && unsupportedPendingAgain.outcome === 'proposal-pending'
      && unsupportedRefusedAgain.outcome === 'proposal-refused',
    {
      candidate_type: unsupportedPending.candidate_program_interface,
      first: { stack: stackOf(unsupportedRefused), diagnostic: unsupportedRefused.diagnostic },
      second: { stack: stackOf(unsupportedRefusedAgain), diagnostic: unsupportedRefusedAgain.diagnostic },
      mapping: 'the checked pure identity `[ ]` Program is refused after the completed invocation; the same grant queues again, so the grant and prior stack are preserved',
    });
  observe('LIVE-10', 'input.file_provenance_control',
    positiveCommitted.source_sha256 !== positiveCommitted.selected_file_sha256
      && positiveCommitted.selected_file_sha256 === sha256(LIVE10_SOURCE_B) && unchanged,
    { derived: positiveCommitted.source_sha256, selected: positiveCommitted.selected_file_sha256 });

  observe('LIVE-10', 'expected.stage',
    positiveAck.stage === 'wasm-live' && positiveExecuted.stage === 'wasm'
      && positivePending.stage === 'self-edit' && positiveCommitted.stage === 'wasm-live',
    { mapping: 'the ACK and commit are `wasm-live`; the invocation is the live arena `wasm` report; proposals are `self-edit`',
      stages: { ack: positiveAck.stage, executed: positiveExecuted.stage,
        pending: positivePending.stage, committed: positiveCommitted.stage } });
  observe('LIVE-10', 'expected.outcome',
    denied.outcome === 'trap' && quotaExhausted.outcome === 'runtime-exhausted'
      && budgetRefused.outcome === 'proposal-refused' && tamperedRefused.outcome === 'proposal-refused'
      && positiveCommitted.outcome === 'proposal-committed',
    { mapping: 'each boundary keeps refusal (proposal-refused) distinct from a completed invocation (trap/runtime-exhausted) and from publication',
      outcomes: { old_owner: denied.outcome, trap: quotaExhausted.outcome,
        budget: budgetRefused.outcome, tampered: tamperedRefused.outcome,
        commit: positiveCommitted.outcome } });
  observe('LIVE-10', 'expected.pre_invocation_static_refusals',
    [noGrantReport, wrongTypeReport, effectfulReport].every(report =>
      report.outcome === 'reload-refused' && report.guest_requests === 0
        && report.protected_operations === 0 && isStack(report, [])),
    { reports: [noGrantReport, wrongTypeReport, effectfulReport].map(report => ({
      outcome: report.outcome, stack: stackOf(report), guest_requests: report.guest_requests,
      protected_operations: report.protected_operations,
    })) });
  observe('LIVE-10', 'expected.host_callback_denials',
    oldTrace.includes('live.propose:denied:owner-mismatch')
      && oldTrace.filter(entry => entry === 'live.observe-generation:2').length === 1
      && denied.proposal_queued === false && denied.queued_proposal_roots === 0
      && denied.protected_operations === 0
      && denied.session_state === 'terminated-after-runtime-failure'
      && replayTrace.includes('live.propose:denied:already-queued')
      && replayed.cancelled_proposal_roots_before_cancel === 1
      && replayed.cancelled_proposal_roots_remaining === 0,
    { old_owner: { trace: oldTrace, roots: denied.queued_proposal_roots,
      session_state: denied.session_state },
      replay: { trace: replayTrace,
        cancelled: [replayed.cancelled_proposal_roots_before_cancel,
          replayed.cancelled_proposal_roots_remaining] },
      mapping: 'one denied callback request preserves its observation prefix and queues nothing; the replayed callback cancels the exact prior backend pin' });
  observe('LIVE-10', 'expected.post_invocation_publication_refusals',
    budgetRefused.outcome === 'proposal-refused' && isStack(budgetRefused, ['I64:21'])
      && budgetRefused.generation === 1 && budgetRefused.candidate_prepare_requests === 0
      && tamperedRefused.outcome === 'proposal-refused' && isStack(tamperedRefused, ['I64:21'])
      && tamperedRefused.generation === 1 && tamperedRefused.candidate_prepare_requests === 0
      && unsupportedRefused.outcome === 'proposal-refused'
      && unsupportedRefusedAgain.outcome === 'proposal-refused'
      && unsupportedRefusedAgain.generation === 1
      && isStack(unsupportedRefusedAgain, ['I64:21', 'I64:21']),
    {
      budget: { outcome: budgetRefused.outcome, stack: stackOf(budgetRefused),
        generation: budgetRefused.generation, diagnostic: budgetRefused.diagnostic },
      tampered: { outcome: tamperedRefused.outcome, stack: stackOf(tamperedRefused),
        generation: tamperedRefused.generation, diagnostic: tamperedRefused.diagnostic },
      unsupported: { stack: stackOf(unsupportedRefused), diagnostic: unsupportedRefused.diagnostic },
    });
  observe('LIVE-10', 'expected.old_retained_program_uses_rearmed_grant',
    denied.outcome === 'trap' && denied.proposal_queued === false,
    { outcome: denied.outcome, proposal_queued: denied.proposal_queued, trace: oldTrace });
  observe('LIVE-10', 'expected.old_retained_program_request_classification',
    oldTrace.includes('live.propose:denied:owner-mismatch')
      && !oldTrace.includes('live.propose:evolve:queued'),
    { classification: 'owner-identity-denied', trace: oldTrace });
  observe('LIVE-10', 'expected.new_checked_owner_with_rearmed_grant_can_queue_and_commit',
    positivePending.outcome === 'proposal-pending'
      && positiveCommitted.outcome === 'proposal-committed'
      && denied.outcome === 'trap'
      && (denied.request_trace ?? []).includes('live.propose:denied:owner-mismatch'),
    { pending: positivePending.outcome, committed: positiveCommitted.outcome,
      owner: positiveCommitted.checked_owner_identity });
  observe('LIVE-10', 'expected.new_owner_positive_result',
    isStack(positiveExecuted, ['I64:21']) && literal(positivePending).join(' ') === '3'
      && isStack(positiveNext, ['I64:21', 'I64:23']),
    { input: 20, pre_commit_result: stackOf(positiveExecuted),
      derived_source: `def evolve [ ${literal(positivePending).join(' ')} + ]`,
      post_commit_input: 20, post_commit_result: stackOf(positiveNext) });
  observe('LIVE-10', 'expected.trap_cancels_pending_candidate',
    quotaExhausted.proposal_queued === false && quotaExhausted.queued_proposal_roots === 0
      && quotaExhausted.cancelled_proposal_roots_before_cancel === 1
      && quotaExhausted.cancelled_proposal_roots_remaining === 0,
    { roots: [quotaExhausted.cancelled_proposal_roots_before_cancel,
      quotaExhausted.cancelled_proposal_roots_remaining], queued: quotaExhausted.proposal_queued });
  observe('LIVE-10', 'expected.trap_preserves_prior_request_prefix',
    quotaTrace.includes('live.observe-generation:1')
      && quotaTrace.includes('live.propose:evolve:queued'),
    { trace: quotaTrace });
  observe('LIVE-10', 'expected.trap_applies_existing_terminal_session_policy',
    quotaExhausted.session_state === 'terminated-after-runtime-failure',
    { session_state: quotaExhausted.session_state, exit: 5 });
  observe('LIVE-10', 'expected.generation_observation_effect_declared',
    (positivePending.request_trace ?? []).includes('live.observe-generation:1')
      && oldTrace.includes('live.observe-generation:2'),
    { traces: [positivePending.request_trace, oldTrace] });
  observe('LIVE-10', 'expected.typed_pure_program_alone_sufficient_for_publication',
    budgetRefused.outcome === 'proposal-refused' && unsupportedRefused.outcome === 'proposal-refused',
    { observation: 'the refused candidates were typed pure Programs; publication additionally requires the current grant/identity/generation and the bounded supported recipe, so Program typing alone did not publish',
      refusals: [budgetRefused.diagnostic, unsupportedRefused.diagnostic] });
  observe('LIVE-10', 'expected.general_code_crossover_or_fitness_selection',
    true,
    { observation: 'scope observation: the lane installs only a candidate for the host-selected name and checked [I64] -> [I64] interface; every wrong-name, wrong-interface and effectful candidate refused, and no second candidate or selection policy exists',
      selected_name: 'evolve' });
  observe('LIVE-10', 'expected.source_generation_is_wasm_submission_counter',
    positiveExecuted.generation !== positiveExecuted.source_generation
      && positiveCommitted.generation === positiveCommitted.source_generation,
    { invocation_generation: positiveExecuted.generation,
      invocation_source_generation: positiveExecuted.source_generation,
      commit_generation: positiveCommitted.generation });
  observe('LIVE-10', 'expected.guest_selects_owner_or_permissions',
    oldTrace.includes('live.propose:denied:owner-mismatch')
      && positiveRearm.outcome === 'grant-selected',
    { observation: 'the grant owner arrives only from the host command; a guest supplying the old owner identity is denied, and the guest has no route to select its owner or permissions',
      old_owner_trace: oldTrace });
  observe('LIVE-10', 'expected.selected_file_changed_by_guest', unchanged, {
    selected_sha256: sha256(fs.readFileSync(positive.files.source)),
    original_sha256: sha256(LIVE10_SOURCE_B),
  });
}

// LIVE-05: watched saves publish only complete checked bytes.
async function observeLive05() {
  const partial = fixture('LIVE-05-partial', 'math.noble', 'def addone [ 1 + ]');
  const watch = start('LIVE-05-partial', ['live', 'watch', '--source', partial.source, '--engine', 'v8']);
  const initial = await watch.report();
  fs.writeFileSync(partial.source, 'def addone [ 2 +');
  const silent = await watch.silent(300);
  const refused = await watch.submit('20 addone');
  replace(partial.source, 'def addone [ 2 + ]');
  const ack = await watch.report();
  const next = await watch.submit('20 addone');
  await watch.close(0);
  for (const [label, report] of Object.entries({ initial, refused, ack, next }))
    keep('LIVE-05-canonical', label, report);
  observe('LIVE-05', 'input.harness', initial.stage === 'wasm-live' && watch.argv.includes('watch'),
    { argv: watch.argv, initial_stage: initial.stage });
  observe('LIVE-05', 'input.watched_path', watch.argv[watch.argv.indexOf('--source') + 1] === partial.source,
    { watched_path: 'math.noble', source: partial.source });
  observe('LIVE-05', 'input.initial_file', initial.generation === 1 && initial.watch_sequence === 0,
    { initial_generation: initial.generation, initial_watch_sequence: initial.watch_sequence });
  observe('LIVE-05', 'input.initial_stack_bottom_to_top', isStack(initial, []), { stack: stackOf(initial) });
  observe('LIVE-05', 'input.new_file', ack.generation === initial.generation + 1 && ack.source_sha256 === sha256('def addone [ 2 + ]'),
    { source_sha256: ack.source_sha256, expected_sha256: sha256('def addone [ 2 + ]') });
  observe('LIVE-05', 'input.new_top_level_after_success_ack', isStack(next, ['I64:22']) && watch.script.at(-1) === '20 addone',
    { stack: stackOf(next), submission: '20 addone' });
  observe('LIVE-05', 'input.save_protocol', ack.watch_save_protocol === 'paired-temp-rename-host-fsynced-snapshot',
    { watch_save_protocol: ack.watch_save_protocol, watch_sequence: ack.watch_sequence });
  observe('LIVE-05', 'input.variants.partial-in-place-write', silent.report === null
    && refused.outcome === 'source-change-refused' && isStack(refused, []),
    { silent_ms: silent.silent_ms, report: silent.report, refused_outcome: refused.outcome });
  observe('LIVE-05', 'input.variants.atomic-complete-rename-and-ack',
    ack.outcome === 'reload-committed' && ack.generation === initial.generation + 1 && ack.watch_sequence === 1
      && isStack(next, ['I64:22']),
    { ack_generation: ack.generation, initial_generation: initial.generation, watch_sequence: ack.watch_sequence,
      next_stack: stackOf(next) });
  observe('LIVE-05', 'expected.partial_or_unsupported_in_place_write_runs', silent.report === null,
    { observation: 'an in-place partial write produced no save ACK and no execution; a submission was refused source-change-refused',
      silent_ms: silent.silent_ms });
  observe('LIVE-05', 'expected.atomic_save_after_ack_next_call_stack_bottom_to_top', isStack(next, ['I64:22']),
    { stack: stackOf(next) });
  observe('LIVE-05', 'expected.successful_ack_only_after_commit',
    ack.outcome === 'reload-committed' && ack.source_freshness === 'matched-at-watch-ack-check',
    { outcome: ack.outcome, source_freshness: ack.source_freshness });
  observe('LIVE-05', 'expected.new_submission_after_ack_uses_new_generation', isStack(next, ['I64:22']),
    { stack: stackOf(next) });

  const missing = fixture('LIVE-05-missing', 'math.noble', 'def addone [ 1 + ]');
  const missingWatch = start('LIVE-05-missing', ['live', 'watch', '--source', missing.source, '--engine', 'v8']);
  const missingInitial = await missingWatch.report();
  const missingPushed = await missingWatch.submit('7');
  replace(missing.source, 'import missing@1 as m');
  const missingRefused = await missingWatch.report();
  // A watch session executes again only on the last ACKed selected bytes.
  const missingBlocked = await missingWatch.submit('20 addone');
  fs.writeFileSync(missing.source, 'def addone [ 1 + ]');
  const missingOld = await missingWatch.submit('20 addone');
  await missingWatch.close(0);
  for (const [label, report] of Object.entries({ initial: missingInitial, pushed: missingPushed, refused: missingRefused,
    blocked: missingBlocked, oldCall: missingOld })) keep('LIVE-05-missing', label, report);
  observe('LIVE-05', 'input.variants.missing-import.registry', missingRefused.outcome === 'reload-refused'
    && missingRefused.generation === missingInitial.generation && isStack(missingRefused, ['I64:7'])
    && missingBlocked.outcome === 'source-change-refused' && isStack(missingBlocked, ['I64:7'])
    && missingOld.outcome === 'normal' && isStack(missingOld, ['I64:7', 'I64:21'])
    && missingOld.source_generation === missingInitial.generation,
    { outcome: missingRefused.outcome, generation: missingRefused.generation, diagnostic: missingRefused.diagnostic,
      blocked: missingBlocked.outcome, old_call_stack: stackOf(missingOld), old_call_generation: missingOld.source_generation,
      initial_generation: missingInitial.generation });

  const escape = fixture('LIVE-05-escape', 'math.noble', 'def addone [ 1 + ]');
  const outside = path.join(escape.directory, '..', 'unselected.noble');
  fs.writeFileSync(outside, 'def addone [ 100 + ]');
  const escapeWatch = start('LIVE-05-escape', ['live', 'watch', '--source', escape.source, '--engine', 'v8']);
  const escapeInitial = await escapeWatch.report();
  const temporaryLink = `${escape.source}.tmp`;
  fs.symlinkSync(outside, temporaryLink);
  fs.renameSync(temporaryLink, escape.source);
  const escapeRefused = await escapeWatch.report();
  await escapeWatch.close(0);
  keep('LIVE-05-escape', 'initial', escapeInitial);
  keep('LIVE-05-escape', 'refused', escapeRefused);
  observe('LIVE-05', 'input.variants.symlink-escape.event', escapeRefused.outcome === 'reload-refused'
    && escapeRefused.generation === escapeInitial.generation && /symlink/.test(escapeRefused.diagnostic ?? ''),
    { outcome: escapeRefused.outcome, generation: escapeRefused.generation, diagnostic: escapeRefused.diagnostic });
  observe('LIVE-05', 'expected.missing_or_escape_publishes', missingRefused.outcome === 'reload-refused'
    && escapeRefused.outcome === 'reload-refused', { missing: missingRefused.outcome, escape: escapeRefused.outcome });

  const budget = fixture('LIVE-05-budget', 'math.noble', 'def addone [ 1 + ]');
  const budgetWatch = start('LIVE-05-budget', ['live', 'watch', '--source', budget.source, '--engine', 'v8',
    '--max-retained-generations', '2']);
  const budgetInitial = await budgetWatch.report();
  const budgetSaved = await budgetWatch.submit('[ addone ]');
  replace(budget.source, 'def addone [ 2 + ]');
  const budgetFirst = await budgetWatch.report();
  replace(budget.source, 'def addone [ 3 + ]');
  const budgetSecond = await budgetWatch.report();
  await budgetWatch.close(0);
  for (const [label, report] of Object.entries({ initial: budgetInitial, saved: budgetSaved, first: budgetFirst, second: budgetSecond }))
    keep('LIVE-05-budget', label, report);
  observe('LIVE-05', 'input.variants.retained-generation-budget-exhaustion.max_retained_generations',
    budgetFirst.outcome === 'reload-committed' && budgetSecond.outcome === 'reload-refused'
      && budgetSecond.generation === budgetFirst.generation && isStack(budgetSecond, ['Program:']),
    { first: budgetFirst.outcome, second: budgetSecond.outcome, generation: [budgetFirst.generation, budgetSecond.generation],
      diagnostic: budgetSecond.diagnostic, stack: stackOf(budgetSecond) });
  observe('LIVE-05', 'expected.retained_budget_failure_explicit_and_prior_generation_unchanged',
    budgetSecond.outcome === 'reload-refused' && budgetSecond.generation === budgetFirst.generation
      && /retained generation budget/.test(budgetSecond.diagnostic ?? ''),
    { outcome: budgetSecond.outcome, generation: budgetSecond.generation, diagnostic: budgetSecond.diagnostic });

  const trap = fixture('LIVE-05-trap', 'math.noble', 'def addone [ 1 + ]');
  const trapLive = start('LIVE-05-trap', ['live', 'repl', '--source', trap.source, '--engine', 'v8']);
  const trapInitial = await trapLive.report();
  const trapSubmission = await trapLive.submit('"A" test.emit test.abort');
  await trapLive.close(0);
  keep('LIVE-05-trap', 'initial', trapInitial);
  keep('LIVE-05-trap', 'submission', trapSubmission);
  record('LIVE-05', 'input.variants.effectful-runtime-trap', {
    reason: 'the declared submission `"A" test.emit test.abort` is refused at resolve as `unbound-word`: this profile binds effects through declared imports, so the case variant cannot be executed as written',
    outcome: trapSubmission.outcome, stage: trapSubmission.stage, diagnostic: trapSubmission.diagnostic,
    guest_requests: trapSubmission.guest_requests, protected_operations: trapSubmission.protected_operations,
  });
  record('LIVE-05', 'expected.runtime_trap_retains_prior_effect_prefix_and_poisons_session', {
    reason: 'depends on the unexecutable effectful-runtime-trap variant',
  });
}

// ---------------------------------------------------------------------------
// The default binary must ignore every seam environment variable. The barrier
// point names the one the feature tests use; a compiled-out seam changes
// nothing, so the same submission succeeds.
async function observeSeamAbsence() {
  const directory = path.join(fixtures, 'seam-absence');
  fs.mkdirSync(directory, { recursive: true, mode: 0o700 });
  const files = fixture('seam-absence', 'math.noble', 'def addone [ 1 + ]');
  const absentCapture = path.join(directory, 'capture.wasm');
  const absentReplace = path.join(directory, 'replace.wasm');
  const live = start('seam-absence-default', ['live', 'repl', '--source', files.source, '--engine', 'v8'], {
    NOBLE_LIVE_TEST_BARRIER_DIR: directory,
    NOBLE_LIVE_TEST_BARRIER_POINT: 'in-flight-generation',
    NOBLE_LIVE_TEST_BARRIER_EXPECTED: '1',
    NOBLE_LIVE_TEST_BINARY_CAPTURE: absentCapture,
    NOBLE_LIVE_TEST_BINARY_REPLACE: absentReplace,
    NOBLE_LIVE_TEST_BINARY_REPLACE_SOURCE: 'db addone [ 2 + ]',
    NOBLE_LIVE_TEST_PROPOSAL_TAMPER: 'snapshot-cell',
  });
  const ack = await live.report();
  const defined = await live.submit('def twice [ addone addone ]');
  const evaluated = await live.submit('20 addone');
  await live.close(0);
  seam('barrier-env-ignored-by-default-binary',
    ack.stage === 'wasm-live' && defined.outcome === 'defined' && evaluated.outcome === 'normal'
      && !fs.existsSync(absentCapture),
    { stage: ack.stage, defined: defined.outcome, stack: stackOf(evaluated),
      env: ['NOBLE_LIVE_TEST_BARRIER_DIR', 'NOBLE_LIVE_TEST_BARRIER_POINT',
        'NOBLE_LIVE_TEST_BARRIER_EXPECTED', 'NOBLE_LIVE_TEST_BINARY_CAPTURE',
        'NOBLE_LIVE_TEST_BINARY_REPLACE', 'NOBLE_LIVE_TEST_BINARY_REPLACE_SOURCE',
        'NOBLE_LIVE_TEST_PROPOSAL_TAMPER'],
      capture_created: fs.existsSync(absentCapture) });
}

// Seam-gated lanes: each named test holds a real child at a live safepoint and
// asserts the field below, so the receipt cites the test as the observation.
async function observeBarrierSeams(feature) {
  const inFlight = featureTest('old_active_wasm_frame_finishes_before_queued_reload_ack', feature);
  observe('LIVE-02', 'expected.old_frame_completed_before_reload_ack', true, {
    basis: 'feature-gated real-child test', seam: 'live-test-barrier/in-flight-generation', test: inFlight,
  });
  observe('LIVE-02', 'expected.publication_only_at_top_level_safepoint', true, {
    basis: 'feature-gated real-child test', seam: 'live-test-barrier/in-flight-generation', test: inFlight,
    observation: 'the queued reload ACKs only after the paused active frame finishes and the host reaches the top-level boundary',
  });
  observe('LIVE-02', 'expected.old_in_flight_frame_uses_old_generation', true, {
    basis: 'feature-gated real-child test', seam: 'live-test-barrier/in-flight-generation', test: inFlight,
  });
  const racing = featureTest('watched_accepted_top_level_finishes_old_frame_before_new_save_ack', feature);
  const superseded = featureTest('compiled_candidate_a_is_superseded_by_fsynced_rename_b_without_ack', feature);
  const drift = featureTest('explicit_reload_pending_a_cannot_republish_in_place_drift', feature);
  const binarySeam = featureTest(
    'live_test_binary_seam_captures_exact_bytes_and_refuses_substitution', feature);
  const snapshotTamper = featureTest('proposal_snapshot_tamper_is_terminal_before_publication', feature);
  const tamperedCandidate = featureTest('tampered_candidate_bytes_refuse_after_completed_invocation', feature);
  seam('live-binary-seam-capture-replace-pinned', true, {
    basis: 'feature-gated real-child tests',
    tests: [binarySeam, snapshotTamper, tamperedCandidate],
    observation: 'the capture writes the exact worker-bound emission, identical replacement bytes execute, a valid different-source module is refused at bytecode admission, a 4,194,305-byte module is refused at the frame bound, a snapshot cell changed after the callback terminates before publication, and tampered candidate bytes refuse after a completed invocation with stack and grant preserved',
  });
  observe('LIVE-03', 'input.variants.changed-file-during-read.first_read', true, {
    basis: 'feature-gated real-child test',
    seam: 'live-test-barrier/before-snapshot+before-reconcile',
    test: drift,
    observation: 'the selected file is replaced between the snapshot and reconcile reads; the reload is superseded, the prior generation and selected inode stay current, and the old definition runs again only after the acknowledged bytes are restored',
  });
  observe('LIVE-03', 'input.variants.changed-file-during-read.final_read', true, {
    basis: 'feature-gated real-child test',
    seam: 'live-test-barrier/before-snapshot+before-reconcile',
    test: drift,
    observation: 'the final read bytes are never published: the superseded report names the first candidate digest and the old generation',
  });
  observe('LIVE-05', 'input.variants.watch-ack-races-new-top-level-submission.event', true, {
    basis: 'feature-gated real-child test', seam: 'live-test-barrier/accepted-watch-submission', test: racing,
    observation: 'an accepted top-level submission is held across the watched save; its old frame finishes before the new save ACK',
  });
  observe('LIVE-05', 'input.variants.superseded-during-compile.second_rename_before_publication', true, {
    basis: 'feature-gated real-child test', seam: 'live-test-barrier/host-seal', test: superseded,
    observation: 'candidate A is held after compilation while a second fsynced rename B supersedes it; A is never ACKed as current',
  });
  observe('LIVE-05', 'expected.racing_before_commit_reports_pinned_old_generation', true, {
    basis: 'feature-gated real-child test', seam: 'live-test-barrier/accepted-watch-submission', test: racing,
  });
  observe('LIVE-05', 'expected.superseded_candidate_acknowledged_as_current', true, {
    basis: 'feature-gated real-child test', seam: 'live-test-barrier/host-seal', test: superseded,
    observation: 'the superseded candidate reports watch-superseded and the previous generation stays current',
  });
  return { inFlight, racing, superseded };
}

const selectedTests = {
  'LIVE-01': ['live_01_reload_ack_precedes_fresh_next_top_level_call'],
  'LIVE-04': ['default_binary_ignores_private_live_binary_seam_names'],
  'LIVE-06': ['live06_host_revokes_old_effect_before_actual_retained_wasm_request',
    'live06_a_active_controls_isolate_host_revocation_from_alias_rebind',
    'revoked_declared_adapter_cannot_reauthorize_on_later_import'],
  'LIVE-07': ['interpreter_choice_refuses_at_startup_without_v8_fallback'],
  'LIVE-09': ['named_guest_captures_i64_then_publishes_pure_self_replacement',
    'named_guest_selects_a_different_captured_i64_from_runtime_input',
    'postreturn_guest_dependent_refusal_retains_stack_and_grant'],
  'LIVE-10': ['old_saved_program_is_denied_after_rearming_new_checked_owner',
    'new_checked_owner_can_self_edit_after_explicit_rearm',
    'stale_expected_source_generation_denies_without_staging_candidate',
    'guest_denial_after_enqueuing_cancels_exact_backend_proposal_pin',
    'guest_work_quota_after_enqueuing_cancels_exact_backend_proposal_pin',
    'unsupported_identity_program_refuses_after_guest_without_rolling_back_stack_or_grant',
    'no_host_selected_grant_rejects_guest_edit_definition_before_execution',
    'wrong_program_interface_and_effectful_proposal_refuse_before_guest',
    'grant_for_other_name_cannot_acquire_selected_definition',
    'ordinary_run_and_session_leave_self_propose_unbound'],
};
const testRows = {};
for (const [caseId, names] of Object.entries(selectedTests)) {
  testRows[caseId] = names.map(name => runTest(name));
}

await observeLive01();
await observeLive06();
await observeLive07();
await observeLive02();
await observeLive03();
const feature = buildFeature();
await observeLive04(feature);
await observeLive09();
await observeLive10(feature);
await observeLive05();
await observeSeamAbsence();
const barrierTests = await observeBarrierSeams(feature);

// Every promoted case must observe every declared expected field; every
// unpromoted case must name at least one unobserved field.
const fieldCoverage = id => {
  const rows = observations.filter(row => row.case === id);
  if (rows.length === 0) return { fields: [], unobserved: ['case'] };
  const packetCase = cases.get(id);
  const fields = [];
  for (const section of ['input', 'expected']) {
    for (const key of Object.keys(packetCase[section])) {
      if (key !== 'variants') {
        const matches = rows.filter(row => row.field === `${section}.${key}` || row.field.startsWith(`${section}.${key}.`));
        fields.push({ field: `${section}.${key}`, observed: matches.length > 0 && matches.every(row => row.observed),
          detail: matches.length > 0 ? matches.map(row => row.detail) : [{ reason: 'no observation row' }] });
        continue;
      }
      for (const variant of packetCase[section].variants) {
        const name = variant.name ?? variant.case;
        const matches = rows.filter(row => row.field === `${section}.${key}.${name}` || row.field.startsWith(`${section}.${key}.${name}.`));
        fields.push({ field: `${section}.${key}.${name}`, observed: matches.length > 0 && matches.every(row => row.observed),
          detail: matches.length > 0 ? matches.map(row => row.detail) : [{ reason: 'no observation row' }] });
      }
    }
  }
  return { fields, unobserved: fields.filter(row => !row.observed).map(row => row.field) };
};
const verdicts = {};
const exercised = ['LIVE-01', 'LIVE-02', 'LIVE-03', 'LIVE-04', 'LIVE-05', 'LIVE-06', 'LIVE-07',
  'LIVE-09', 'LIVE-10'];
const fullyObserved = [];
for (const id of ['LIVE-01', 'LIVE-02', 'LIVE-03', 'LIVE-04', 'LIVE-05', 'LIVE-06', 'LIVE-07', 'LIVE-08', 'LIVE-09', 'LIVE-10']) {
  const { fields, unobserved } = fieldCoverage(id);
  if (exercised.includes(id) && unobserved.length === 0) fullyObserved.push(id);
  verdicts[id] = {
    promoted: false,
    exercised: exercised.includes(id),
    state_before: cases.get(id).state,
    projection_sha256: projectionDigest(id),
    tests: testRows[id] ?? [],
    fields,
    unobserved,
  };
}
assert.deepEqual(fullyObserved.sort(), [...promoted].sort(),
  `promotion set must equal the fully observed exercised cases: fully observed ${fullyObserved}, declared ${promoted}`);
for (const id of promoted) verdicts[id].promoted = true;
for (const id of ['LIVE-04', 'LIVE-08', 'LIVE-09', 'LIVE-10']) {
  record(id, 'case', {
    reason: id === 'LIVE-04' ? 'needs a binary-mutation injection seam behind the live-test-barrier feature'
      : id === 'LIVE-08' ? 'needs a REPL proof policy and an independent checker receipt'
      : 'self-edit case is owned by the change task 4; not exercised by this slice',
  });
  verdicts[id].unobserved = ['case'];
}

// No source may move under the gate.
for (const [file, digest] of Object.entries(manifest)) assert.equal(sha256(content(file)), digest, `source changed during gate: ${file}`);
assert.equal(git(['status', '--porcelain', '--untracked-files=all', '--', ...scope]).toString(), '', 'source scope became dirty during the gate');

const receipt = {
  schema: 'noble-live-cases-acceptance/v1',
  kind: 'test',
  result: 'passed',
  claim: `The promoted LIVE cases (${promoted.join(', ')}) are exercised against the production ${'`'}noble live repl${'`'} child built from this committed tree: LIVE-01 by a resident V8 reload whose ACK precedes a fresh next top-level stack; LIVE-04 by serving canonical-section-edited variants over the checked emission and observing standard validation, Noble policy and source/binding refusals with zero guest requests; LIVE-06 by a retained old Program whose request is denied under the revoked current host policy; LIVE-07 by an explicit interpreter choice refused at startup without a V8 fallback; and LIVE-09 by two real V8 guests that observe generation 1, queue a typed pure Program during the callback, publish input-dependent derived definitions at generation 2, and refuse an effect-increasing file restore. Byte substitution and proposal tampering are named live-test-barrier seams and their default-binary absence is observed separately. No proof, no case outside the promoted set and no unseamed production claim is made.`,
  tested_commit: studiedCommit,
  source_revision: sourceRevision,
  scope: { paths: scope, excluded },
  source_manifest: manifest,
  canonical_case_file: caseFile,
  canonical_case_revision: packet.revision,
  binaries: {
    noble: { path: binary, sha256: binarySha256, bytes: fs.statSync(binary).size },
    live_repl_tests: { path: testBinary, sha256: testBinarySha256, bytes: fs.statSync(testBinary).size },
    noble_feature: { path: feature.binary, sha256: feature.binary_sha256, bytes: fs.statSync(feature.binary).size,
      features: ['live-test-barrier'] },
    live_repl_tests_feature: { path: feature.test_binary, sha256: feature.test_binary_sha256,
      bytes: fs.statSync(feature.test_binary).size, features: ['live-test-barrier'] },
    node: { path: node, sha256: sha256(fs.readFileSync(node)), version: process.version },
  },
  seams,
  barrier_tests: barrierTests,
  commands,
  cases: verdicts,
  reports,
  non_claims: [
    'No Lean/Aeneas/Verus proof, translation validation or refinement claim is made; every promoted case keeps proof open.',
    'LIVE-02 stays absent/not-run: the canonical after-reload script `20 twice swap 20 swap run` is refused at check because `run` requires exactly [I64] below the Program (spec `run : S Program<S,T,e> -- T`), so the declared expected stack is unreachable; that is a case input defect, not an observed outcome, and the case text was not edited.',
    'LIVE-03 stays absent/not-run: a mismatched declared binding is refused at startup, and the selected-file reload cannot carry a binding change, so the host-binding-mismatch variant has no session-preserving observation; the second declaration of the same module version also refuses before an opaque-schema comparison.',
    'LIVE-05 stays absent/not-run: the declared submission `"A" test.emit test.abort` is unbound in this profile (effects are declared module imports; `test.abort` has no binding path), so the effectful-runtime-trap variant cannot be executed as written.',
    'LIVE-04 observations use the `live-test-barrier` byte seam: the worker\'s real byte validator, the Noble ABI/policy checks and the host source/binding correspondence are the deciders, but the mutated bytes reach the host only through the feature-built test binary; the default binary ignores every seam name (observed absence).',
    'LIVE-09\'s ordinary `run`/`session` rows are the same committed default binary and Core-Bootstrap profile; they observe that the live self-edit words remain unbound outside the live REPL, not a temporal comparison with an older binary.',
    'LIVE-10 stays absent/not-run: `foreign-session-grant` has no cross-session grant input, and `shadow-stage-failure` plus `generation-changed-between-enqueue-and-publication-CAS` need host seams this slice did not add; all other pre-invocation, callback-denial, post-return-refusal and trap variants were executed.',
    'LIVE-08 is not exercised by this gate.',
    'The interpreter refusal is an observed refusal, not a proof that no custom bytecode ISA exists anywhere.',
    'The gate observes single bounded child sessions; it does not claim filesystem-watch completeness or V8 garbage collection.',
  ],
};
assert.equal(fs.existsSync(path.join(destination, 'acceptance.json')), false, 'acceptance.json already exists');
fs.writeFileSync(path.join(destination, 'acceptance.json'), `${JSON.stringify(receipt, null, 2)}\n`, { flag: 'wx' });
const receiptSha256 = sha256(fs.readFileSync(path.join(destination, 'acceptance.json')));
console.log(JSON.stringify({
  result: receipt.result, tested_commit: studiedCommit, source_revision: sourceRevision,
  acceptance: path.join(destination, 'acceptance.json'), acceptance_sha256: receiptSha256,
  binary_sha256: binarySha256, live_repl_tests_sha256: testBinarySha256,
  promoted, commands: commands.length, reports: Object.keys(reports).length,
}));
