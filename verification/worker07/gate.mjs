#!/usr/bin/env node
// WORKER-07 acceptance gate.
//
// Builds the production `noble` binary from a clean checkout and runs the
// consumer candidate embedder (`noble worker program test-host candidate`).
// Before it reads a candidate, the host selects its prior session (a checked
// Worker Program closure), the expected interface (its own pure Program's,
// `I64 -- I64 ! {}`), the bootstrap environment and finite limits. The
// untrusted `noble-candidate/v1` document on stdin carries only a candidate;
// the kernel acceptance checker is the only route and runs at most once, and
// only an accepted candidate is published into the session.
//
// The baseline candidate must be accepted and change the session, which shows
// the session and its counters are live. Each case variant must be refused
// with its outcome, publish nothing and leave the session fingerprint
// unchanged. The injected internal failure is a host-selected fault inside
// the acceptance stage on the valid baseline (`candidate-fault`). Two controls
// remove the producer's metadata from the interface and environment variants
// and show the checker itself refuses the body under the consumer's choices.
//
// Usage (inside the pinned devShell):
//   node verification/worker07/gate.mjs --out NEW_DIRECTORY
import assert from 'node:assert/strict';
import { execFileSync, spawnSync } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = fs.realpathSync(path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..'));
const here = path.join(root, 'verification/worker07');
const SCOPE = ['crates', 'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml',
  'verification/worker07/gate.mjs', 'verification/worker07/fixtures'];
const CASE_FILE = 'specs/conformance/worker-cases.json';
const KERNEL_ROUTE = 'noble_kernel::acceptance::check';
const CONSUMER_INTERFACE = 'I64 -- I64 ! {}';
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const hash = file => sha(fs.readFileSync(file));

// Case outcome -> embedder report outcome.
const OUTCOME = { invalid: 'invalid-input', unsupported: 'unsupported', 'internal-failure': 'internal-failure' };
// Case variant -> fixture and host mode.
const VARIANTS = {
  'cyclic-witness-premises': { fixture: 'cyclic-witness-premises.json', mode: 'candidate' },
  'missing-premise': { fixture: 'missing-premise.json', mode: 'candidate' },
  'witness-for-another-node': { fixture: 'witness-for-another-node.json', mode: 'candidate' },
  'infinite-type-equation': { fixture: 'infinite-type-equation.json', mode: 'candidate' },
  'candidate-replaces-expected-interface': { fixture: 'candidate-replaces-expected-interface.json', mode: 'candidate' },
  'candidate-selects-unadmitted-environment': { fixture: 'candidate-selects-unadmitted-environment.json', mode: 'candidate' },
  'unsupported-witness': { fixture: 'unsupported-witness.json', mode: 'candidate' },
  'unsupported-candidate-format': { fixture: 'unsupported-candidate-format.json', mode: 'candidate' },
  'injected-checker-internal-failure': { fixture: 'baseline.json', mode: 'candidate-fault' },
};

function argument(name) {
  const index = process.argv.indexOf(name);
  return index > 0 ? process.argv[index + 1] : undefined;
}

function git(...args) {
  return execFileSync('git', args, { cwd: root, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 });
}

function sourceSnapshot() {
  const dirty = git('status', '--porcelain', '--untracked-files=all', '--', ...SCOPE);
  assert.equal(dirty, '', `source scope is not clean:\n${dirty}`);
  const files = git('ls-files', '-z', '--', ...SCOPE).split('\0').filter(Boolean).sort();
  const snapshot = {};
  for (const file of files) snapshot[file] = { sha256: hash(path.join(root, file)) };
  return snapshot;
}

function caseProjection() {
  const cases = JSON.parse(fs.readFileSync(path.join(root, CASE_FILE), 'utf8')).cases;
  const selected = cases.filter(c => c.id === 'WORKER-07');
  assert.equal(selected.length, 1, 'expected exactly one WORKER-07 case');
  const { id, profile, kind, requirements, input, expected } = selected[0];
  return { id, profile, kind, requirements, input, expected };
}

function buildNoble() {
  const build = spawnSync('cargo', ['build', '--locked', '-p', 'noble-cli', '--bin', 'noble',
    '--message-format=json-render-diagnostics'], { cwd: root, encoding: 'utf8', maxBuffer: 256 * 1024 * 1024 });
  assert.equal(build.status, 0, `cargo build failed:\n${build.stderr}`);
  const artifacts = build.stdout.split('\n').filter(Boolean).map(line => JSON.parse(line))
    .filter(m => m.reason === 'compiler-artifact' && m.target?.name === 'noble' && m.executable);
  assert.equal(artifacts.length, 1, 'expected one noble executable artifact');
  return { executable: artifacts[0].executable, profile: artifacts[0].profile?.opt_level === '0' ? 'dev' : 'release' };
}

function run(noble, name, mode, fixture, out) {
  const workdir = fs.mkdtempSync(path.join(os.tmpdir(), 'noble-worker07-'));
  const stdin = fs.readFileSync(path.join(here, 'fixtures', fixture));
  const child = spawnSync(noble, ['worker', 'program', 'test-host', mode], {
    cwd: workdir, input: stdin, encoding: 'buffer', timeout: 300_000, maxBuffer: 16 * 1024 * 1024,
  });
  assert.equal(child.error, undefined, `${name}: ${child.error}`);
  const leftovers = fs.readdirSync(workdir);
  fs.rmSync(workdir, { recursive: true, force: true });
  fs.writeFileSync(path.join(out, 'raw', `${name}.stdout.json`), child.stdout);
  fs.writeFileSync(path.join(out, 'raw', `${name}.stderr`), child.stderr);
  return {
    name,
    argv: ['noble', 'worker', 'program', 'test-host', mode],
    stdin: `verification/worker07/fixtures/${fixture}`,
    exit: child.status,
    stdout_sha256: sha(child.stdout),
    stderr_sha256: sha(child.stderr),
    files_left_in_working_directory: leftovers.length,
    report: JSON.parse(child.stdout.toString('utf8')),
  };
}

function main() {
  const destination = argument('--out');
  assert.ok(destination, 'usage: node verification/worker07/gate.mjs --out NEW_DIRECTORY');
  const out = path.resolve(destination);
  assert.ok(!fs.existsSync(out), `output directory already exists: ${out}`);

  const sourceFiles = sourceSnapshot();
  const projection = caseProjection();
  const caseVariants = projection.input.variants.map(variant => variant.case).sort();
  assert.deepEqual(caseVariants, Object.keys(VARIANTS).sort(), 'gate variants differ from the case');
  const build = buildNoble();
  fs.mkdirSync(path.join(out, 'raw'), { recursive: true });

  const baseline = run(build.executable, 'baseline', 'candidate', 'baseline.json', out);
  const variants = projection.input.variants.map(variant => {
    const { fixture, mode } = VARIANTS[variant.case];
    return { ...run(build.executable, variant.case, mode, fixture, out), expected: variant.outcome };
  });
  const interfaceControl = run(build.executable, 'interface-control', 'candidate',
    'candidate-replaces-expected-interface-control.json', out);
  const environmentControl = run(build.executable, 'environment-control', 'candidate',
    'candidate-selects-unadmitted-environment-control.json', out);

  const failures = [];
  const all = [baseline, ...variants, interfaceControl, environmentControl];
  for (const row of all) {
    const report = row.report;
    if (report.expected_interface_owner !== 'consumer' || report.expected_interface !== CONSUMER_INTERFACE) {
      failures.push(`${row.name}: the expected interface was not the consumer's`);
    }
    if (row.files_left_in_working_directory !== 0) failures.push(`${row.name}: left files in the working directory`);
  }
  if (!(baseline.exit === 0 && baseline.report.outcome === 'ready' && baseline.report.accepted_programs === 1
    && baseline.report.prior_session_unchanged === false && baseline.report.checker_invocations === 1)) {
    failures.push('the baseline candidate was not accepted into the session');
  }
  for (const row of variants) {
    const want = OUTCOME[row.expected];
    if (row.report.outcome !== want) failures.push(`${row.name}: expected ${row.expected}, observed ${row.report.outcome}`);
  }
  const fault = variants.find(row => row.name === 'injected-checker-internal-failure');
  if (fault?.report.fault_injected !== true || fault?.report.checker_invocations !== 1) {
    failures.push('the injected fault did not fire once inside the acceptance stage');
  }
  for (const [control, detail] of [[interfaceControl, 'StackJoin'], [environmentControl, 'UnknownDefinition(Definition(26))']]) {
    if (!(control.report.outcome === 'invalid-input' && control.report.refused_at === 'acceptance'
      && control.report.detail === detail)) failures.push(`${control.name}: the checker did not refuse the body (${detail})`);
  }
  const fallback = all.some(row => row.report.checker_invocations > 1
    || (row.report.checker_routes ?? []).some(route => route !== KERNEL_ROUTE));
  const observed = {
    stage: variants.every(row => row.report.stage === 'acceptance') ? 'acceptance' : 'mixed',
    outcome: variants.every(row => row.report.outcome !== 'ready'
      && row.report.outcome === OUTCOME[row.expected]) ? 'no-variant-admitted' : 'variant-admitted-or-misclassified',
    guest_requests: variants.reduce((sum, row) => sum + row.report.guest_requests, 0),
    protected_operations: variants.reduce((sum, row) => sum + row.report.protected_operations, 0),
    accepted_programs: variants.reduce((sum, row) => sum + row.report.accepted_programs, 0),
    prior_session_unchanged: variants.every(row => row.report.prior_session_unchanged === true
      && row.report.session.before === row.report.session.after),
    fallback_checker_used: fallback,
  };
  for (const [field, value] of Object.entries(projection.expected)) {
    if (observed[field] !== value) failures.push(`${field}: expected ${JSON.stringify(value)}, observed ${JSON.stringify(observed[field])}`);
  }

  const receipt = {
    schema: 'noble-worker07-acceptance/v1',
    case: 'WORKER-07',
    result: failures.length === 0 ? 'passed' : 'failed',
    failures,
    tested_commit: git('rev-parse', 'HEAD').trim(),
    source_scope: SCOPE,
    source_revision: `sha256:${sha(JSON.stringify(sourceFiles))}`,
    source_file_count: Object.keys(sourceFiles).length,
    case_projection_sha256: sha(JSON.stringify(projection)),
    case_projection: projection,
    binary: { name: 'noble', profile: build.profile, sha256: hash(build.executable),
      build: 'cargo build --locked -p noble-cli --bin noble' },
    fixtures: Object.fromEntries(fs.readdirSync(path.join(here, 'fixtures')).sort()
      .map(name => [name, hash(path.join(here, 'fixtures', name))])),
    input_mapping: {
      expected_interface_owner: `The embedder takes ${CONSUMER_INTERFACE} from its own checked pure Program before reading stdin; the document cannot name one.`,
      'cyclic-witness-premises': 'swap binds a := ref b and b := ref a, a type-equation cycle.',
      'missing-premise': 'swap\'s witness omits the binding for its second value variable.',
      'witness-for-another-node': 'The literal and the addition exchange their witnesses.',
      'infinite-type-equation': 'swap binds a := Pair<ref a, I64>; candidate types have no variables, so the decoder refuses the equation as malformed.',
      'candidate-replaces-expected-interface': 'A dup body that computes I64 -- I64 I64 carries an `expected` field claiming that interface.',
      'candidate-selects-unadmitted-environment': 'An invocation of text.byte (definition 26), absent from the bootstrap environment, carries an `environment` field naming the text-cursor environment.',
      'unsupported-witness': 'The addition is spelled as a signed owner-local self occurrence, which the public checker does not authorize.',
      'unsupported-candidate-format': 'The document names noble-candidate/v2.',
      'injected-checker-internal-failure': 'The valid baseline under candidate-fault: a host-selected fault fires inside the acceptance stage after the checker returns.',
    },
    observed,
    observation_methods: {
      stage: 'Every variant report names stage acceptance; refused_at separates decoding from the checker.',
      outcome: 'Every variant is refused with its case outcome; the baseline in the same embedder is accepted.',
      accepted_programs: 'Programs published into the session during each variant run, summed.',
      prior_session_unchanged: 'The session fingerprint (prior Worker Program graph, node interfaces, compile count, WAT digest and accepted ledger) before and after each variant run.',
      guest_requests: 'Effect requests in the session\'s synthetic ledger receipt, summed.',
      fallback_checker_used: 'Any run with more than one checker invocation or a route other than the kernel acceptance check.',
    },
    runs: { baseline, variants, controls: [interfaceControl, environmentControl] },
    source_files: sourceFiles,
    non_claims: [
      'The internal failure is injected by the host inside the acceptance stage, not raised by the kernel.',
      'The prior session is a host-selected Worker Program closure; candidates are checked against the bootstrap environment only.',
      'The candidate wire form is local and experimental, not a portable encoding.',
      'No proof obligation is discharged.',
    ],
  };
  fs.writeFileSync(path.join(out, 'acceptance.json'), `${JSON.stringify(receipt, null, 2)}\n`);
  console.log(JSON.stringify({ result: receipt.result, failures, observed,
    source_revision: receipt.source_revision, binary_sha256: receipt.binary.sha256 }));
  process.exitCode = failures.length === 0 ? 0 : 1;
}

main();
