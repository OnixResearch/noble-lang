#!/usr/bin/env node
// GUARD-CASE acceptance gate (`.cairn/changes/bounded-applicability-guards`).
//
// Builds the production `noble` binary from a clean checkout, then drives `noble companions`
// sessions for the registered GUARD-CASE designs in `specs/conformance/guard-cases.json`: real
// Lean admissions of the fixture contracts, certification of live subjects, guard invocation,
// composition and projection. Every command line and report is retained. A case is promotable only
// when every registered expectation was observed; a variant whose execution route does not exist
// yet is reported `not-executed` with its reason, never as passed.
// GUARD-CASE-11 alone runs a second binary built from the same checkout with the test-only
// `live-test-barrier` feature, whose one-shot guard fault selection is its harness fault model
// (owner decision 33); that binary is never the shipped one.
//
// Usage (inside the pinned devShell, with `proofs/mc1` built for the contract library):
//   node verification/guard-cases/gate.mjs --out NEW_DIRECTORY
//   node verification/guard-cases/gate.mjs --self-test
//
// Not a proof and not a registration. It promotes nothing by itself.
import assert from 'node:assert/strict';
import { execFileSync, spawn, spawnSync } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { createInterface } from 'node:readline';
import { fileURLToPath } from 'node:url';

const root = fs.realpathSync(path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..'));
const SCOPE = ['crates', 'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml',
  'verification/guard-cases', 'verification/mc2/contracts', 'verification/mc1/family.noble-contract',
  'verification/mc1/family-proof.lean', 'specs/conformance/guard-cases.json'];
const CASE_FILE = 'specs/conformance/guard-cases.json';
const REVISION = '0.1.0-draft.5';
const CASE_IDS = Array.from({ length: 12 }, (_, index) => `GUARD-CASE-${String(index + 1).padStart(2, '0')}`);
const MAX = '9223372036854775807';
const MIN = '-9223372036854775808';
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const hash = file => sha(fs.readFileSync(file));

function argument(name) {
  const index = process.argv.indexOf(name);
  return index > 0 ? process.argv[index + 1] : undefined;
}

function git(...args) {
  return execFileSync('git', args, { cwd: root, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 });
}

// A case is `passed` only when every registered expectation was observed. `unmet` records an
// observed difference, `failed` a run that could not observe, `not-executed` a variant whose
// execution route is not implemented yet. Only `passed` is promotable.
function deriveVerdicts(observations) {
  const caseOf = id => observations[id]?.case ?? 'not-executed';
  return {
    promotable: CASE_IDS.filter(id => caseOf(id) === 'passed'),
    unmet: CASE_IDS.filter(id => caseOf(id) === 'unmet'),
    failed: CASE_IDS.filter(id => caseOf(id) === 'failed'),
    not_executed: CASE_IDS.filter(id => caseOf(id) === 'not-executed'),
  };
}

if (process.argv.includes('--self-test')) {
  const all = Object.fromEntries(CASE_IDS.map(id => [id, { case: 'passed' }]));
  assert.deepEqual(deriveVerdicts(all).promotable, CASE_IDS);
  const mixed = { ...all, 'GUARD-CASE-03': { case: 'unmet' }, 'GUARD-CASE-11': { case: 'not-executed' },
    'GUARD-CASE-02': { case: 'failed' } };
  const derived = deriveVerdicts(mixed);
  assert.deepEqual(derived.promotable,
    CASE_IDS.filter(id => !['GUARD-CASE-02', 'GUARD-CASE-03', 'GUARD-CASE-11'].includes(id)));
  assert.deepEqual(derived.unmet, ['GUARD-CASE-03']);
  assert.deepEqual(derived.not_executed, ['GUARD-CASE-11']);
  assert.deepEqual(derived.failed, ['GUARD-CASE-02']);
  const { 'GUARD-CASE-12': _dropped, ...missing } = all;
  assert.deepEqual(deriveVerdicts(missing).not_executed, ['GUARD-CASE-12']);
  console.log('gate self-test: only fully observed cases are promotable');
  process.exit(0);
}

function sourceSnapshot() {
  const dirty = git('status', '--porcelain', '--untracked-files=all', '--', ...SCOPE);
  assert.equal(dirty, '', `source scope is not clean:\n${dirty}`);
  const files = git('ls-files', '-z', '--', ...SCOPE).split('\0').filter(Boolean).sort();
  const snapshot = {};
  for (const file of files) snapshot[file] = { sha256: hash(path.join(root, file)) };
  return snapshot;
}

function cases() {
  const packet = JSON.parse(fs.readFileSync(path.join(root, CASE_FILE), 'utf8'));
  assert.equal(packet.revision, REVISION);
  const byId = new Map(packet.cases.map(one => [one.id, one]));
  for (const id of CASE_IDS) assert.ok(byId.has(id), `case ${id} missing`);
  const project = id => {
    const { profile, kind, requirements, input, expected } = byId.get(id);
    return { id, profile, kind, requirements, input, expected };
  };
  return { byId, project };
}

const raw = [];
function recordRaw(output, name, text) {
  const file = path.join(output, 'raw', `${name}.txt`);
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, text);
  raw.push({ name, file: path.relative(output, file), sha256: sha(text), bytes: Buffer.byteLength(text) });
}

function buildNoble(output, features = []) {
  const selected = features.length ? ['--features', features.join(',')] : [];
  const suffix = features.length ? `-${features.join('-')}` : '';
  const build = spawnSync('cargo', ['build', '--locked', '-p', 'noble-cli', '--bin', 'noble', ...selected,
    '--message-format=json-render-diagnostics'],
  { cwd: root, encoding: 'utf8', maxBuffer: 256 * 1024 * 1024 });
  recordRaw(output, `build${suffix}.stdout`, build.stdout ?? '');
  recordRaw(output, `build${suffix}.stderr`, build.stderr ?? '');
  assert.equal(build.status, 0, `cargo build failed:\n${build.stderr}`);
  const artifacts = build.stdout.split('\n').filter(Boolean).map(line => JSON.parse(line))
    .filter(message => message.reason === 'compiler-artifact'
      && message.target?.name === 'noble' && message.executable);
  assert.equal(artifacts.length, 1, 'expected one noble executable artifact');
  const executable = artifacts[0].executable;
  return { executable, executable_sha256: hash(executable) };
}

// The CLI unit-test executable that carries the core controls.
function buildControls(output) {
  const build = spawnSync('cargo', ['test', '--locked', '-p', 'noble-cli', '--bin', 'noble', '--no-run',
    '--message-format=json-render-diagnostics'],
  { cwd: root, encoding: 'utf8', maxBuffer: 256 * 1024 * 1024 });
  recordRaw(output, 'controls-build.stdout', build.stdout ?? '');
  recordRaw(output, 'controls-build.stderr', build.stderr ?? '');
  assert.equal(build.status, 0, `cargo test --no-run failed:\n${build.stderr}`);
  const artifacts = build.stdout.split('\n').filter(Boolean).map(line => JSON.parse(line))
    .filter(message => message.reason === 'compiler-artifact' && message.target?.name === 'noble'
      && message.profile?.test === true && message.executable);
  assert.equal(artifacts.length, 1, 'expected one CLI unit-test executable');
  return { executable: artifacts[0].executable, executable_sha256: hash(artifacts[0].executable) };
}

// The reviewed Lean sandbox tools, as the MC2 gate selects them.
const REVIEWED = {
  NOBLE_LEAN: '/nix/store/kwvw84h9r7w0q40d3cm5xgfz0rpv7yzc-noble-lean-release-4.35.0/bin/lean',
  NOBLE_BWRAP: '/nix/store/lqndphylsxqwbwm804n473pb4sqb98sh-bubblewrap-0.11.2/bin/bwrap',
  NOBLE_PRLIMIT: '/nix/store/mqvbf0flamqaq9c3ihb496aahag897n0-util-linux-2.42.3-bin/bin/prlimit',
  NOBLE_SYSTEMD_RUN: '/nix/store/baxgs06659w4i6ggsfr56srjrb52v1h8-systemd-261.2/bin/systemd-run',
  NOBLE_CONTRACT_LIBRARY: path.join(root, 'proofs/mc1'),
};
const environment = { ...process.env };
for (const [key, value] of Object.entries(REVIEWED)) environment[key] ??= value;

const fixture = name => path.join(root, 'verification/guard-cases/fixtures', name);
const mc2 = name => path.join(root, 'verification/mc2/contracts', name);
const mc1 = name => path.join(root, 'verification/mc1', name);
const sessions = [];
let sourceSerial = 0;

// One `noble companions` session: each line is answered by exactly one report.
// `selection` adds private test-hook variables, which only the feature-built binary forwards.
async function withSession(binary, output, label, action, selection = {}) {
  const record = { label, args: ['companions', '--timeout-ms', '600000', '--opt', 'off'],
    binary: path.basename(binary), selection, operations: [], exit: null, signal: null, stderr: '',
    error: null };
  sessions.push(record);
  const child = spawn(binary, record.args, { cwd: root, env: { ...environment, ...selection },
    stdio: ['pipe', 'pipe', 'pipe'] });
  let pending = null;
  const ended = new Promise(resolveEnd => {
    child.once('error', error => { record.error = String(error); });
    child.once('close', (exit, signal) => {
      Object.assign(record, { exit, signal });
      if (pending) { pending.reject(new Error(record.error ?? 'session closed before its report')); pending = null; }
      resolveEnd();
    });
  });
  child.stderr.on('data', chunk => { record.stderr += chunk.toString(); });
  const lines = createInterface({ input: child.stdout });
  lines.on('line', text => {
    const request = pending; pending = null;
    if (!request) { record.error = `unsolicited report: ${text}`; return; }
    try { request.resolve(JSON.parse(text)); } catch (error) { request.reject(error); }
  });
  const session = {
    async call(line) {
      assert.equal(pending, null, 'one outstanding request at a time');
      const report = await new Promise((resolveReply, reject) => {
        pending = { resolve: resolveReply, reject };
        child.stdin.write(`${line}\n`);
      });
      record.operations.push({ input: line, report });
      return report;
    },
    async source(name, text) {
      const file = path.join(output, 'sources', `${++sourceSerial}-${name}.noble`);
      fs.mkdirSync(path.dirname(file), { recursive: true });
      fs.writeFileSync(file, `${text}\n`, { flag: 'wx' });
      return file;
    },
    async push(name, text) {
      const report = await session.call(`submit ${await session.source(name, text)}`);
      assert.equal(report.outcome, 'normal', `${name}: ${JSON.stringify(report)}`);
      return report;
    },
    async submitFile(file) {
      const report = await session.call(`submit ${file}`);
      assert.equal(report.outcome, 'normal', `${path.basename(file)}: ${JSON.stringify(report)}`);
      return report;
    },
    async admit(contract, proof) {
      const report = await session.call(`contract ${contract} ${proof}`);
      assert.equal(report.outcome, 'proved', `${path.basename(contract)}: ${JSON.stringify(report)}`);
      return report.contract_reference;
    },
    async certify(index, reference) {
      const report = await session.call(`certify ${index} ${reference}`);
      assert.equal(report.outcome, 'certified', `certify ${index}: ${JSON.stringify(report)}`);
      return report;
    },
    async stack() {
      return (await session.call('stack')).stack;
    },
    async top(type) {
      const stack = await session.stack();
      const index = stack.map(cell => cell.type).lastIndexOf(type);
      assert.ok(index >= 0, `no ${type} slot`);
      return index;
    },
  };
  try {
    return await action(session);
  } finally {
    child.stdin.end();
    await ended;
    lines.close();
    recordRaw(output, `session-${sessions.length}-${label}`, record.operations
      .map(operation => `> ${operation.input}\n${JSON.stringify(operation.report)}\n`).join(''));
    if (record.stderr) recordRaw(output, `session-${sessions.length}-${label}.stderr`, record.stderr);
  }
}

class Mismatch extends Error {}
// Key order is not part of an observation: compare canonical JSON.
function canonical(value) {
  if (Array.isArray(value)) return value.map(canonical);
  if (value && typeof value === 'object') {
    return Object.fromEntries(Object.keys(value).sort().map(key => [key, canonical(value[key])]));
  }
  return value;
}
function expectValue(label, actual, expected) {
  if (JSON.stringify(canonical(actual)) !== JSON.stringify(canonical(expected))) {
    throw new Mismatch(`${label}: observed ${JSON.stringify(actual)}, registered ${JSON.stringify(expected)}`);
  }
}

// The I64 payload a report carries in `output`, or null.
function payload(report) {
  const value = typeof report.output === 'string' ? JSON.parse(report.output) : report.output;
  return value?.value ?? null;
}

// Stack identity: types, cell handles and plain values, so a preserved session is byte-for-byte
// the same. A stack cell carries its value as the JSON text of `detail` (a Program cell's detail
// has no `value`).
const cellValue = cell => typeof cell?.detail === 'string' ? JSON.parse(cell.detail)?.value ?? null : null;
const shape = stack => stack.map(cell => ({ type: cell.type, handle: cell.handle ?? null,
  value: cellValue(cell) }));

function invocationRow(argument, report) {
  const row = { argument, outcome: report.outcome };
  if (report.outcome === 'certified-invocation') row.output = payload(report);
  else if (report.outcome === 'precondition-reject') row.candidate_body_started = report.candidate_body_started;
  return row;
}

async function observe(id, body) {
  try {
    return { case: 'passed', ...(await body()) };
  } catch (error) {
    return error instanceof Mismatch
      ? { case: 'unmet', diagnostic: error.message }
      : { case: 'failed', diagnostic: error.stack ?? String(error) };
  }
}

// GUARD-CASE-01/02: v1 templates at their boundaries through the frozen invocation path.
async function v1Template(binary, output, label, contract, proof, args) {
  return withSession(binary, output, label, async session => {
    await session.submitFile(fixture('increment.noble'));
    const reference = await session.admit(contract, proof);
    await session.certify(0, reference);
    const companion = await session.top('Certified');
    const before = shape(await session.stack());
    const rows = [];
    const wrappers = new Set();
    const counters = [];
    for (const value of args) {
      const report = await session.call(`invoke ${companion} ${value}`);
      rows.push(invocationRow(value, report));
      if (report.wrapper_source) wrappers.add(report.wrapper_source);
      counters.push([report.guest_requests ?? null, report.protected_operations ?? null]);
      expectValue(`${label} ${value} set version`, report.guard_template_set_version ?? null, null);
    }
    expectValue(`${label} session preserved`, shape(await session.stack()), before);
    return { rows, wrappers: [...wrappers], counters };
  });
}

// A NonEmptyList session: subject, admission, certification and the tail cell below them.
async function listSession(session, subject, contract, proof, tail) {
  if (tail !== undefined) await session.push('tail', tail);
  await session.submitFile(subject);
  const program = (await session.stack()).length - 1;
  const reference = await session.admit(contract, proof);
  await session.certify(program, reference);
  return { companion: await session.top('Certified'), reference, program };
}

// Invoke the companion on one list literal pushed above the session, then drop the literal.
async function invokeOnList(session, companion, name, literal) {
  await session.push(name, literal);
  const slot = (await session.stack()).length - 1;
  const before = shape(await session.stack());
  const report = await session.call(`invoke ${companion} ${slot}`);
  const after = shape(await session.stack());
  await session.push('release', 'drop');
  return { report, preserved: JSON.stringify(before) === JSON.stringify(after), before, after };
}

const out = argument('--out');
assert.ok(out, 'pass --out NEW_DIRECTORY');
assert.equal(fs.existsSync(out), false, '--out must not already exist');
fs.mkdirSync(out, { mode: 0o700 });

const snapshot = sourceSnapshot();
const sourceRevision = `sha256:${sha(JSON.stringify(snapshot))}`;
const testedCommit = git('rev-parse', 'HEAD').trim();
const { byId, project } = cases();
const projections = CASE_IDS.map(project);
const projectionDigest = sha(JSON.stringify(projections));
const built = buildNoble(out);
const binary = path.join(out, 'noble');
fs.copyFileSync(built.executable, binary);
fs.chmodSync(binary, 0o500);
const binarySha = hash(binary);
const faultBuilt = buildNoble(out, ['live-test-barrier']);
const faultBinary = path.join(out, 'noble-guard-fault');
fs.copyFileSync(faultBuilt.executable, faultBinary);
fs.chmodSync(faultBinary, 0o500);
const faultBinarySha = hash(faultBinary);
const expectedOf = id => byId.get(id).expected;
const observations = {};

observations['GUARD-CASE-01'] = await observe('GUARD-CASE-01', async () => {
  const expected = expectedOf('GUARD-CASE-01');
  const input = byId.get('GUARD-CASE-01').input;
  const lt = await v1Template(binary, out, 'lt-i64-max', mc2('guarded.contract'), mc2('guarded.proof.lean'),
    input.arguments.LtI64Max);
  const ne = await v1Template(binary, out, 'ne-i64-min', fixture('ne-i64-min.contract'),
    fixture('increment-true.proof.lean'), input.arguments.NeI64Min);
  expectValue('LtI64Max outcomes', lt.rows, expected.outcomes.LtI64Max);
  expectValue('NeI64Min outcomes', ne.rows, expected.outcomes.NeI64Min);
  expectValue('LtI64Max wrapper bytes', lt.wrappers, [expected.wrapper_source.LtI64Max]);
  expectValue('NeI64Min wrapper bytes', ne.wrappers, [expected.wrapper_source.NeI64Min]);
  for (const [requests, operations] of [...lt.counters, ...ne.counters]) {
    expectValue('guest requests', requests, expected.guest_requests);
    expectValue('protected operations', operations, expected.protected_operations);
  }
  return { LtI64Max: lt.rows, NeI64Min: ne.rows, wrappers: [...lt.wrappers, ...ne.wrappers] };
});

observations['GUARD-CASE-02'] = await observe('GUARD-CASE-02', async () => {
  const expected = expectedOf('GUARD-CASE-02');
  const cli = await v1Template(binary, out, 'eq-i64-literal-5', fixture('eq-i64-literal-5.contract'),
    fixture('increment-true.proof.lean'), byId.get('GUARD-CASE-02').input.arguments);
  expectValue('eq-5 outcomes', cli.rows, expected.outcomes);
  expectValue('eq-5 wrapper bytes', cli.wrappers, [expected.wrapper_source]);
  expectValue('6 accepted under eq-5', cli.rows.some(row => row.argument === '6'
    && row.outcome === 'certified-invocation'), expected.accepts_6_under_eq_5);
  // The substituted binding runs as a Rust core control against the real Core (owner decision
  // 34): the CLI cannot mint a companion whose evidence belongs to another contract.
  const controls = buildControls(out);
  const receipt = path.join(out, 'guard-controls.json');
  const run = spawnSync(controls.executable, ['core::companions::controls::guard::exercise', '--exact',
    '--ignored', '--nocapture'], { cwd: root, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024,
    env: { ...environment, NOBLE_GUARD_CONTROL_RECEIPT: receipt, NOBLE_MC2_CHECKER_BINARY: binary } });
  recordRaw(out, 'guard-controls.stdout', run.stdout ?? '');
  recordRaw(out, 'guard-controls.stderr', run.stderr ?? '');
  assert.equal(run.status, 0, `guard core control failed:\n${run.stdout}\n${run.stderr}`);
  const control = JSON.parse(fs.readFileSync(receipt, 'utf8')).controls
    .find(one => one.case === 'GUARD-CASE-02' && one.variant === 'substituted-eq-6-binding');
  assert.ok(control, 'the substituted-binding control is missing from its receipt');
  const substituted = { stage: 'binding', outcome: control.observed.diagnostic,
    guard_executed: control.input.guard_executed };
  expectValue('substituted binding', substituted, expected.substituted_binding);
  expectValue('no certified status from the substitution', control.observed.new_certified_status, false);
  return { rows: cli.rows, substituted_binding: substituted,
    controls: { executable_sha256: controls.executable_sha256, receipt: path.relative(out, receipt),
      receipt_sha256: hash(receipt) } };
});

observations['GUARD-CASE-03'] = await observe('GUARD-CASE-03', async () => {
  const expected = expectedOf('GUARD-CASE-03');
  const input = byId.get('GUARD-CASE-03').input;
  const main = await withSession(binary, out, 'head-of-nonempty', async session => {
    const { companion } = await listSession(session, fixture('head-of-nonempty.noble'),
      fixture('head-of-nonempty.contract'), fixture('head-of-nonempty.proof.lean'), '41');
    const tailBefore = shape(await session.stack())[0];
    const rows = [];
    const reports = [];
    for (const [index, literal] of input.arguments.entries()) {
      const { report, preserved } = await invokeOnList(session, companion, `argument-${index}`, literal);
      const row = invocationRow(literal, report);
      if (report.outcome === 'precondition-reject') row.output = payload(report);
      rows.push(row);
      reports.push({ report, preserved });
    }
    const tailAfter = shape(await session.stack())[0];
    return { rows, reports, tail_preserved: JSON.stringify(tailBefore) === JSON.stringify(tailAfter)
      && tailBefore?.value === '41' };
  });
  const nested = await withSession(binary, out, 'head-of-nonempty-nested', async session => {
    const { companion } = await listSession(session, fixture('nested.noble'), fixture('nested.contract'),
      fixture('nested.proof.lean'));
    const { report, preserved } = await invokeOnList(session, companion, 'nested', input.nested.argument);
    const value = typeof report.output === 'string' ? JSON.parse(report.output) : report.output;
    // A list value is reported as `{type: "List", value: [items]}`.
    return { report, preserved, output_type: value?.type ?? null,
      output_items: Array.isArray(value?.value) ? value.value.length : null };
  });
  // A rejection reports its `inl` payload as the output and starts no body.
  expectValue('outcomes', main.rows, expected.outcomes);
  expectValue('nested outcome', { outcome: nested.report.outcome, output_type: nested.output_type,
    output_items: nested.output_items }, expected.nested);
  const all = [...main.reports, { report: nested.report, preserved: nested.preserved }];
  expectValue('session preserved by every invocation', all.every(one => one.preserved), expected.session_preserved);
  expectValue('tail preserved', main.tail_preserved, expected.tail_preserved);
  for (const { report } of all) {
    expectValue('set version', report.guard_template_set_version, expected.guard_template_set_version);
    expectValue('wrapper bytes', report.wrapper_source, expected.wrapper_source);
    expectValue('prelude', report.invocation_prelude, expected.invocation_prelude);
    expectValue('guest requests', report.guest_requests, expected.guest_requests);
    expectValue('protected operations', report.protected_operations, expected.protected_operations);
  }
  return { rows: main.rows, nested: { outcome: nested.report.outcome, output_type: nested.output_type,
    output_items: nested.output_items }, tail_preserved: main.tail_preserved,
  argument_copies: all.map(one => one.report.invocation_argument_copy) };
});

// GUARD-CASE-04/05/06: every unsupported precondition shape refuses before any guard runs.
async function unsupported(label, subject, contract, proof, argument) {
  return withSession(binary, out, label, async session => {
    await session.submitFile(subject);
    const reference = await session.admit(contract, proof);
    await session.certify(0, reference);
    const companion = await session.top('Certified');
    let slot = 0;
    if (argument !== undefined) {
      await session.push('argument', argument);
      slot = (await session.stack()).length - 1;
    }
    const before = shape(await session.stack());
    const report = await session.call(`invoke ${companion} ${slot}`);
    const after = shape(await session.stack());
    return { outcome: report.outcome, candidate_body_started: report.candidate_body_started ?? false,
      guest_requests: report.guest_requests ?? 0, protected_operations: report.protected_operations ?? 0,
      session_preserved: JSON.stringify(before) === JSON.stringify(after), report };
  });
}

async function unsupportedCase(id, rows) {
  const expected = expectedOf(id);
  const observed = [];
  for (const row of rows) observed.push(await unsupported(...row));
  expectValue(`${id} outcomes`, observed.map(one => one.outcome), expected.per_contract);
  for (const one of observed) {
    expectValue('body started', one.candidate_body_started, expected.candidate_body_started);
    expectValue('session preserved', one.session_preserved, true);
    expectValue('guest requests', one.guest_requests, expected.guest_requests);
    expectValue('protected operations', one.protected_operations, expected.protected_operations);
  }
  return { outcomes: observed.map(one => one.outcome) };
}

const listSubject = fixture('head-of-nonempty.noble');
const listProof = fixture('list-true.proof.lean');
observations['GUARD-CASE-04'] = await observe('GUARD-CASE-04', () => unsupportedCase('GUARD-CASE-04', [
  ['head-lt', listSubject, fixture('head-lt.contract'), listProof, '5 nil cons'],
  ['sum-left', fixture('sum-case.noble'), fixture('sum-left.contract'), fixture('sum-true.proof.lean')],
  ['sum-left-eq', fixture('sum-case.noble'), fixture('sum-left-eq.contract'), fixture('sum-true.proof.lean')],
]));
observations['GUARD-CASE-05'] = await observe('GUARD-CASE-05', () => unsupportedCase('GUARD-CASE-05', [
  ['conj-list', listSubject, fixture('conj-list.contract'), listProof, '5 nil cons'],
  ['conj-i64', fixture('increment.noble'), fixture('conj-i64.contract'), fixture('increment-true.proof.lean')],
]));
observations['GUARD-CASE-06'] = await observe('GUARD-CASE-06', () => unsupportedCase('GUARD-CASE-06', [
  ['length-lt', listSubject, fixture('length-lt.contract'), listProof, '5 nil cons'],
  ['length-ne', listSubject, fixture('length-ne.contract'), listProof, '5 nil cons'],
  ['triple-not', listSubject, fixture('triple-not.contract'), listProof, '5 nil cons'],
  ['is-nil', listSubject, fixture('is-nil.contract'), listProof, 'nil'],
  ['two-input', fixture('two-input.noble'), fixture('two-input.contract'), fixture('two-input.proof.lean')],
]));

observations['GUARD-CASE-07'] = await observe('GUARD-CASE-07', async () => {
  const expected = expectedOf('GUARD-CASE-07');
  const argument = byId.get('GUARD-CASE-07').input.argument;
  const observed = await withSession(binary, out, 'changed-predicate', async session => {
    await session.submitFile(listSubject);
    const original = await session.admit(fixture('nonempty-true.contract'), listProof);
    const changed = await session.admit(fixture('list-true.contract'), listProof);
    await session.certify(0, changed);
    const unguarded = await session.top('Certified');
    const changedRun = await invokeOnList(session, unguarded, 'changed', argument);
    await session.certify(0, original);
    const guarded = await session.top('Certified');
    const policy = await session.call('policy 2');
    assert.equal(policy.outcome, 'selected');
    const staleRun = await invokeOnList(session, guarded, 'stale', argument);
    const readmitted = await session.admit(fixture('nonempty-true.contract'), listProof);
    await session.certify(0, readmitted);
    const fresh = await session.top('Certified');
    const freshRun = await invokeOnList(session, fresh, 'readmitted', argument);
    return { changedRun, staleRun, freshRun };
  });
  const rows = {
    changed_predicate: { outcome: observed.changedRun.report.outcome,
      candidate_body_started: observed.changedRun.report.candidate_body_started ?? false },
    after_policy_change: { outcome: observed.staleRun.report.outcome,
      candidate_body_started: observed.staleRun.report.candidate_body_started ?? false },
    after_readmission: { outcome: observed.freshRun.report.outcome, output: payload(observed.freshRun.report) },
  };
  expectValue('changed predicate', rows.changed_predicate, expected.changed_predicate);
  expectValue('after policy change', rows.after_policy_change, expected.after_policy_change);
  expectValue('after readmission', rows.after_readmission, expected.after_readmission);
  for (const run of [observed.changedRun, observed.staleRun]) {
    expectValue('refusal guest requests', run.report.guest_requests ?? 0, expected.guest_requests);
    expectValue('refusal protected operations', run.report.protected_operations ?? 0, expected.protected_operations);
    expectValue('refusal preserves the session', run.preserved, true);
  }
  return rows;
});

observations['GUARD-CASE-08'] = await observe('GUARD-CASE-08', async () => {
  const expected = expectedOf('GUARD-CASE-08');
  async function changed(label, subject, changedSubject, contract, proof) {
    return withSession(binary, out, label, async session => {
      await session.submitFile(subject);
      await session.submitFile(changedSubject);
      const reference = await session.admit(contract, proof);
      await session.certify(0, reference);
      const companion = await session.top('Certified');
      const identity = (await session.call(`inspect ${companion}`)).detail;
      const before = shape(await session.stack());
      const report = await session.call(`certify 1 ${reference}`);
      const after = shape(await session.stack());
      const identityAfter = (await session.call(`inspect ${companion}`)).detail;
      return { outcome: report.outcome, published_companion: JSON.stringify(before) !== JSON.stringify(after),
        identity_unchanged: JSON.stringify(identity) === JSON.stringify(identityAfter) };
    });
  }
  const subject = await changed('changed-subject', listSubject, fixture('changed-subject.noble'),
    fixture('head-of-nonempty.contract'), fixture('head-of-nonempty.proof.lean'));
  const family = await changed('changed-family-capture', mc2('builder.noble'), fixture('builder-constant.noble'),
    mc1('family.noble-contract'), mc1('family-proof.lean'));
  expectValue('changed subject', { outcome: subject.outcome, published_companion: subject.published_companion },
    expected.changed_subject);
  expectValue('changed family capture', { outcome: family.outcome, published_companion: family.published_companion },
    expected.changed_family_capture);
  expectValue('original identity unchanged', subject.identity_unchanged && family.identity_unchanged,
    expected.original_companion_identity_unchanged);
  return { changed_subject: subject, changed_family_capture: family };
});

observations['GUARD-CASE-09'] = await observe('GUARD-CASE-09', async () => {
  const expected = expectedOf('GUARD-CASE-09');
  const input = byId.get('GUARD-CASE-09').input;
  const wrapper = expectedOf('GUARD-CASE-03').wrapper_source;
  const projected = await withSession(binary, out, 'erased-evidence', async session => {
    const { companion } = await listSession(session, listSubject, fixture('head-of-nonempty.contract'),
      fixture('head-of-nonempty.proof.lean'));
    const certified = [];
    for (const [index, literal] of input.arguments.entries()) {
      certified.push((await invokeOnList(session, companion, `certified-${index}`, literal)).report);
    }
    const projection = await session.call(`project ${companion}`);
    const rows = [];
    // Keep only the projected subject, then run the fixed live text on [list, Program] as an
    // ordinary (plain) submission under a `dip` that holds a spare copy of the subject: no
    // companion is consulted.
    const depth = (await session.stack()).length - 1;
    await session.push('keep-projected', `[ ${Array(depth).fill('drop').join(' ')} ] dip`);
    for (const [index, literal] of input.arguments.entries()) {
      await session.push(`argument-${index}`, `[ ${literal} ] dip dup`);
      const plain = await session.call(`plain ${await session.source(`erased-${index}`, `[ ${wrapper} run ] dip`)}`);
      // A stack cell carries its value as the JSON text of `detail`.
      const cell = plain.stack?.[0] ?? null;
      const value = typeof cell?.detail === 'string' ? JSON.parse(cell.detail) : null;
      rows.push({ argument: literal, runtime_return: value?.variant ?? null,
        output: value?.value?.value ?? null, behavioral_certification_claimed: plain.behavioral_certification_claimed,
        outcome: plain.outcome });
      await session.push(`release-${index}`, 'swap drop');
    }
    return { projection, certified, rows };
  });
  const sameAsCertified = projected.rows.every((row, index) =>
    String(payload(projected.certified[index])) === String(row.output));
  const build = spawnSync(binary, ['build', '--require-proof', listSubject, '--contract',
    fixture('head-of-nonempty.contract'), '--timeout-ms', '600000', '--out', path.join(out, 'static-build')],
  { cwd: root, encoding: 'utf8', env: environment, maxBuffer: 64 * 1024 * 1024 });
  recordRaw(out, 'static-build.stdout', build.stdout ?? '');
  recordRaw(out, 'static-build.stderr', build.stderr ?? '');
  const buildReport = JSON.parse((build.stdout ?? '').trim().split('\n').pop() || 'null');
  const released = fs.existsSync(path.join(out, 'static-build', 'artifact.wasm'));
  expectValue('projected outcomes', projected.rows.map(({ argument, runtime_return, output }) =>
    ({ argument, runtime_return, output })), expected.projected.outcomes);
  expectValue('no certification claimed after projection',
    projected.rows.every(row => row.behavioral_certification_claimed === false) ? false : true,
    expected.projected.behavioral_certification_claimed);
  expectValue('same results as the certified invocation', sameAsCertified,
    expected.projected.same_results_as_certified_invocation);
  expectValue('static release without proof', { release: released ? 'released' : 'blocked' },
    expected.static_without_proof);
  return { projected: projected.rows, static_build: { exit: build.status, outcome: buildReport?.outcome ?? null,
    released } };
});

observations['GUARD-CASE-10'] = await observe('GUARD-CASE-10', async () => {
  const expected = expectedOf('GUARD-CASE-10');
  async function compose(label, left, right, value, invokeComposite = false) {
    return withSession(binary, out, label, async session => {
      await session.submitFile(left.subject);
      const leftReference = await session.admit(left.contract, left.proof);
      await session.certify(0, leftReference);
      const leftCompanion = await session.top('Certified');
      await session.submitFile(right.subject);
      const rightProgram = (await session.stack()).length - 1;
      const rightReference = await session.admit(right.contract, right.proof);
      await session.certify(rightProgram, rightReference);
      const rightCompanion = await session.top('Certified');
      const before = shape(await session.stack());
      const composed = await session.call(`compose ${leftCompanion} ${rightCompanion} ${value}`);
      const after = shape(await session.stack());
      const row = { outcome: composed.outcome, new_certified_status: after.filter(cell => cell.type === 'Certified').length
        > before.filter(cell => cell.type === 'Certified').length,
      guest_requests: composed.guest_requests ?? 0, protected_operations: composed.protected_operations ?? 0 };
      if (invokeComposite && row.new_certified_status) {
        const composite = await session.top('Certified');
        row.composite_invocation = (await session.call(`invoke ${composite} ${value}`)).outcome;
      }
      return row;
    });
  }
  const increment = { subject: fixture('increment.noble'), contract: mc2('increment.contract'),
    proof: mc2('increment.proof.lean') };
  const eq5 = { subject: fixture('increment.noble'), contract: fixture('eq-i64-literal-5.contract'),
    proof: fixture('increment-true.proof.lean') };
  const lt = { subject: fixture('increment.noble'), contract: mc2('guarded.contract'), proof: mc2('guarded.proof.lean') };
  const producer = { subject: fixture('list-producer.noble'), contract: fixture('list-producer.contract'),
    proof: fixture('list-producer.proof.lean') };
  const head = { subject: listSubject, contract: fixture('head-of-nonempty.contract'),
    proof: fixture('head-of-nonempty.proof.lean') };
  const guardedThenUnguarded = await compose('eq5-then-increment', eq5, increment, '5');
  const twoGuarded = await compose('two-lt-i64-max', lt, lt, '0');
  const derived = await compose('increment-then-increment', increment, increment, '41', true);
  const witness = await compose('producer-then-head', producer, head, '0');
  expectValue('guarded then unguarded', { outcome: guardedThenUnguarded.outcome,
    new_certified_status: guardedThenUnguarded.new_certified_status }, expected.guarded_then_unguarded);
  expectValue('two guarded', { outcome: twoGuarded.outcome, new_certified_status: twoGuarded.new_certified_status },
    expected.two_guarded);
  expectValue('derived composite', { composition: derived.outcome, composite_invocation: derived.composite_invocation,
    inherits_guard: derived.composite_invocation !== 'unsupported-guard-template' }, expected.derived_composite);
  expectValue('static witness', { outcome: witness.outcome, new_certified_status: witness.new_certified_status },
    expected.static_witness);
  for (const row of [guardedThenUnguarded, twoGuarded, witness]) {
    expectValue('refusal guest requests', row.guest_requests, expected.guest_requests);
    expectValue('refusal protected operations', row.protected_operations, expected.protected_operations);
  }
  return { guarded_then_unguarded: guardedThenUnguarded, two_guarded: twoGuarded, derived_composite: derived,
    static_witness: witness };
});

// GUARD-CASE-11: the guard's exact engine execution faults under the feature binary's one-shot
// selection, keyed by the SHA-256 of the guard's checked source (the fixed prelude and wrapper
// the baseline invocation reports, then `run`): `exhaust:CAP` lowers that execution's step cap so
// the guest's own meter exhausts; `trap` raises a Wasm trap before its first guest instruction.
// The engine poisons itself on either, so the session must end rather than restore the frame.
// Guard and body work share one meter that does not mark where the body starts, so the body is
// observed unstarted only when no execution step was charged; any other count claims nothing.
observations['GUARD-CASE-11'] = await observe('GUARD-CASE-11', async () => {
  const expected = expectedOf('GUARD-CASE-11');
  const input = byId.get('GUARD-CASE-11').input;
  const listArgument = async session => {
    const { companion } = await listSession(session, listSubject, fixture('head-of-nonempty.contract'),
      fixture('head-of-nonempty.proof.lean'));
    await session.push('argument', input.argument);
    return { companion, slot: (await session.stack()).length - 1 };
  };
  const baseline = await withSession(faultBinary, out, 'guard-fault-baseline', async session => {
    const { companion, slot } = await listArgument(session);
    const accepted = await session.call(`invoke ${companion} ${slot}`);
    await session.push('release', 'drop');
    await session.push('empty', input.reject_argument);
    const rejected = await session.call(`invoke ${companion} ${slot}`);
    return { accepted, rejected };
  });
  const guardSource = `${baseline.accepted.invocation_prelude} ${baseline.accepted.wrapper_source} run`;
  const guardDigest = sha(guardSource);
  const faults = {};
  for (const [name, selection] of [['exhaustion', `exhaust:${input.step_cap}:${guardDigest}`],
    ['trap', `trap:${guardDigest}`]]) {
    const run = await withSession(faultBinary, out, `guard-fault-${name}`, async session => {
      const { companion, slot } = await listArgument(session);
      const invoked = await session.call(`invoke ${companion} ${slot}`);
      const later = await session.call('stack');
      return { invoked, later };
    }, { NOBLE_LIVE_TEST_GUARD_FAULT: selection });
    const { invoked, later } = run;
    faults[name] = {
      selection,
      outcome: invoked.outcome,
      engine_outcome: invoked.engine_outcome ?? null,
      quota_reason: invoked.quota_reason ?? null,
      native_trap: invoked.native_trap ?? null,
      execution_steps: invoked.execution_steps ?? null,
      reflection_steps: invoked.reflection_steps ?? null,
      session_state: invoked.session_state ?? null,
      frame_restored: invoked.frame_restored ?? null,
      guest_requests: invoked.guest_requests ?? null,
      protected_operations: invoked.protected_operations ?? null,
      distinct_from_precondition_reject: invoked.outcome !== baseline.rejected.outcome,
      candidate_body_started: invoked.execution_steps === 0 ? false : null,
      later_operation: { outcome: later.outcome, code: later.diagnostic?.code ?? null },
    };
  }
  expectValue('baseline', { accepted: baseline.accepted.outcome, rejected: baseline.rejected.outcome },
    expected.baseline);
  for (const name of ['exhaustion', 'trap']) {
    const { selection, ...observed } = faults[name];
    expectValue(name, observed, expected[name]);
  }
  return { guard_source: guardSource, guard_source_sha256: guardDigest,
    baseline: { accepted: baseline.accepted.outcome, rejected: baseline.rejected.outcome }, faults };
});

observations['GUARD-CASE-12'] = await observe('GUARD-CASE-12', async () => {
  const expected = expectedOf('GUARD-CASE-12');
  const rows = await withSession(binary, out, 'wrong-typed-slots', async session => {
    const { companion } = await listSession(session, listSubject, fixture('head-of-nonempty.contract'),
      fixture('head-of-nonempty.proof.lean'));
    const observed = [];
    for (const [name, literal] of [['i64', '5'], ['list-program', '[ 1 + ] nil cons']]) {
      const run = await invokeOnList(session, companion, name, literal);
      observed.push({ outcome: run.report.outcome, code: run.report.diagnostic?.code ?? run.report.code ?? null,
        preserved: run.preserved, guest_requests: run.report.guest_requests ?? 0,
        protected_operations: run.report.protected_operations ?? 0 });
    }
    return observed;
  });
  expectValue('stage per slot', rows.map(row => row.code), expected.per_slot);
  expectValue('session preserved', rows.every(row => row.preserved), expected.session_preserved);
  for (const row of rows) {
    expectValue('guest requests', row.guest_requests, expected.guest_requests);
    expectValue('protected operations', row.protected_operations, expected.protected_operations);
  }
  return { rows };
});

const { promotable, unmet, failed, not_executed: notExecuted } = deriveVerdicts(observations);
const binaryAfter = hash(binary);
const faultBinaryAfter = hash(faultBinary);
const binaryChanged = binaryAfter !== binarySha || faultBinaryAfter !== faultBinarySha;
const verdictPromotable = binaryChanged ? [] : promotable;
const failures = [...unmet.map(id => `${id}: ${observations[id].diagnostic}`),
  ...failed.map(id => `${id}: ${observations[id].diagnostic?.split('\n')[0]}`),
  ...(binaryChanged ? ['the executed binary changed during the run; no observation is promotable'] : [])];
const acceptance = {
  schema: 'noble-guard-cases-acceptance/v1',
  claim: 'selected executions of GUARD-CASE-01..12 through `noble companions` on one CLI built from the '
    + 'recorded clean commit; promotion verdicts are derived from the observations, not declared here',
  result: failed.length || binaryChanged ? 'failed' : 'completed',
  verdict: failures.length === 0 && notExecuted.length === 0 ? 'all-passed'
    : failed.length || binaryChanged ? 'run-failure' : 'incomplete',
  failures,
  tested_commit: testedCommit,
  source_revision: sourceRevision,
  scope: SCOPE,
  source: snapshot,
  case_file: CASE_FILE,
  projections: { sha256: projectionDigest, ids: CASE_IDS },
  binary: { built: built.executable_sha256, executed_sha256: binarySha, executed_sha256_after: binaryAfter },
  build: 'cargo build --locked -p noble-cli --bin noble',
  guard_fault_binary: { built: faultBuilt.executable_sha256, executed_sha256: faultBinarySha,
    executed_sha256_after: faultBinaryAfter,
    build: 'cargo build --locked -p noble-cli --bin noble --features live-test-barrier',
    used_by: ['GUARD-CASE-11'] },
  environment: Object.fromEntries(Object.keys(REVIEWED).map(key => [key, environment[key]])),
  sessions: sessions.map(({ label, binary: executable, selection, args, exit, signal, error, operations }) =>
    ({ label, binary: executable, selection, args, exit, signal, error, operations: operations.length })),
  raw_outputs: raw,
  observations,
  promotable: verdictPromotable,
  unmet,
  failed,
  not_executed: notExecuted,
  non_claims: [
    'No proof is claimed: promoted cases keep proof open; C1 for templates_of, C6 (Wasm lowering of wrappers) and PO-21 stay open.',
    'Admission uses the reviewed Lean sandbox and the repository MC1 contract library; their soundness is an explicit assumption (VC-GATE-03).',
    'GUARD-CASE-11 is observed under a harness fault model (owner decisions 33 and 33a): a test-only feature build lowers one guard execution\'s step cap or traps it at the top of its guest frame. Faults the shipped binary could meet (allocation failure, a V8 trap mid-guard) are not injected, the quota expires before the guard\'s first execution step rather than inside dup/list.case, and that the body did not start rests on zero charged execution steps, not on a body-side witness.',
  ],
};
const receiptPath = path.join(out, 'acceptance.json');
fs.writeFileSync(receiptPath, `${JSON.stringify(acceptance, null, 2)}\n`);
console.log(JSON.stringify({ receipt: receiptPath, receipt_sha256: hash(receiptPath), result: acceptance.result,
  verdict: acceptance.verdict, failures, tested_commit: testedCommit, source_revision: sourceRevision,
  promotable: verdictPromotable, not_executed: notExecuted }, null, 1));
if (failed.length || binaryChanged) process.exitCode = 1;
