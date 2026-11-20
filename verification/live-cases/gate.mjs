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
const promoted = ['LIVE-01', 'LIVE-06', 'LIVE-07'];

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
  constructor(label, args, environment) {
    this.label = label;
    this.argv = args;
    this.stdout = '';
    this.stderr = '';
    this.queue = [];
    this.waiters = [];
    this.closed = false;
    this.started = Date.now();
    this.child = spawn(binary, args, {
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
      label: this.label, kind: 'live-child', command: [binary, ...this.argv], exit: code,
      duration_ms: Date.now() - this.started, pid: this.pid, script: this.script,
      stdout: { path: `raw/${this.label}.out`, bytes: Buffer.byteLength(this.stdout), sha256: sha256(this.stdout) },
      stderr: { path: `raw/${this.label}.err`, bytes: Buffer.byteLength(this.stderr), sha256: sha256(this.stderr) },
    });
  }
  kill() { this.child.kill('SIGKILL'); }
}

function start(label, args, environment) {
  const child = new LiveChild(label, args, environment);
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
      stack: stackOf(cycleRefused), diagnostic: cycleRefused.diagnostic, old_call_stack: stackOf(cycleOldCall) });
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
  const live = start('seam-absence-default', ['live', 'repl', '--source', files.source, '--engine', 'v8'], {
    NOBLE_LIVE_TEST_BARRIER_DIR: directory,
    NOBLE_LIVE_TEST_BARRIER_POINT: 'in-flight-generation',
    NOBLE_LIVE_TEST_BARRIER_EXPECTED: '1',
  });
  const ack = await live.report();
  const defined = await live.submit('def twice [ addone addone ]');
  const evaluated = await live.submit('20 addone');
  await live.close(0);
  seam('barrier-env-ignored-by-default-binary',
    ack.stage === 'wasm-live' && defined.outcome === 'defined' && evaluated.outcome === 'normal',
    { stage: ack.stage, defined: defined.outcome, stack: stackOf(evaluated),
      env: ['NOBLE_LIVE_TEST_BARRIER_DIR', 'NOBLE_LIVE_TEST_BARRIER_POINT', 'NOBLE_LIVE_TEST_BARRIER_EXPECTED'] });
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
  'LIVE-06': ['live06_host_revokes_old_effect_before_actual_retained_wasm_request',
    'live06_a_active_controls_isolate_host_revocation_from_alias_rebind',
    'revoked_declared_adapter_cannot_reauthorize_on_later_import'],
  'LIVE-07': ['interpreter_choice_refuses_at_startup_without_v8_fallback'],
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
await observeLive05();
await observeSeamAbsence();
const feature = buildFeature();
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
const exercised = ['LIVE-01', 'LIVE-02', 'LIVE-03', 'LIVE-05', 'LIVE-06', 'LIVE-07'];
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
  claim: `The promoted LIVE cases (${promoted.join(', ')}) are exercised against the production ${'`'}noble live repl${'`'} child built from this committed tree: LIVE-01 by a resident V8 reload whose ACK precedes a fresh next top-level stack, LIVE-06 by a retained old Program whose request is denied under the revoked current host policy, and LIVE-07 by an explicit interpreter choice refused at startup without a V8 fallback. No proof, no case outside the promoted set and no feature-gated seam result is claimed.`,
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
    'LIVE-04, LIVE-08, LIVE-09 and LIVE-10 are not exercised by this gate.',
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
