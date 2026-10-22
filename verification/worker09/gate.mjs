#!/usr/bin/env node
// WORKER-09 acceptance gate for the MW1 rows.
//
// Builds the noble-cli unit-test binary from a clean checkout and runs the one
// scoped test `bounded_execution_profile_keeps_each_outcome_distinct`. That
// test drives the real typed worker engine (the production Node/V8 child and
// the interruptible worker compiler) under the bounded execution profile
// `worker-typed/step-4096/interrupt-8`. Cancellation and fault points are
// host-selected controls that exist only in test builds, which is why this
// gate runs the test binary rather than the CLI. The test retains each row's
// typed worker report; this gate maps the reports onto the case's variants.
//
// The two native-async rows, host-deadline-native-work-outstanding and
// permitted-suspension, are MW2 obligations. The receipt keeps a `not-run`
// result for each of them instead of claiming coverage.
//
// Usage (inside the pinned devShell):
//   node verification/worker09/gate.mjs --out NEW_DIRECTORY
import assert from 'node:assert/strict';
import { execFileSync, spawnSync } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = fs.realpathSync(path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..'));
const SCOPE = ['crates', 'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', 'verification/worker09/gate.mjs'];
const CASE_FILE = 'specs/conformance/worker-cases.json';
const TEST = 'core::worker_typed::tests::bounded_execution_profile_keeps_each_outcome_distinct';
const STEP_CEILING = 4096;
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const hash = file => sha(fs.readFileSync(file));

// Case vocabulary -> typed worker report outcome.
const OUTCOME = {
  'normal-return': 'normal', 'budget-exhaustion': 'runtime-exhausted', trap: 'trap',
  cancelled: 'cancelled', 'internal-failure': 'internal-failure', 'host-deadline': 'host-deadline',
};
// Case variant -> retained test row. The two async rows have no MW1 row.
const ROW = {
  'finite-pure-step': 'finite-pure-step',
  'domain-Result-error': 'domain-error-arm',
  'pure-loop-work-limit': 'pure-loop-work-limit',
  'runtime-builder-allocation-limit': 'runtime-builder-allocation-limit',
  'nested-invocation-budget': 'nested-invocation-budget',
  'guest-trap': 'guest-trap',
  'cancelled-invocation': 'cancelled-invocation',
  'runtime-internal-failure': 'runtime-internal-failure',
};
const DEFERRED = {
  'host-deadline-native-work-outstanding': 'MW2: needs native work with a retained owner after the deadline',
  'permitted-suspension': 'MW2: needs a suspendable native task owner',
};
const CONTROLS = ['pure-loop-within-budget', 'builder-within-limit', 'nested-alone', 'outer-alone', 'host-deadline'];

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
  const selected = cases.filter(c => c.id === 'WORKER-09');
  assert.equal(selected.length, 1, 'expected exactly one WORKER-09 case');
  const { id, profile, kind, requirements, input, expected } = selected[0];
  return { id, profile, kind, requirements, input, expected };
}

function buildTests() {
  const build = spawnSync('cargo', ['test', '--locked', '-p', 'noble-cli', '--bin', 'noble', '--no-run',
    '--message-format=json-render-diagnostics'], { cwd: root, encoding: 'utf8', maxBuffer: 256 * 1024 * 1024 });
  assert.equal(build.status, 0, `cargo test --no-run failed:\n${build.stderr}`);
  const artifacts = build.stdout.split('\n').filter(Boolean).map(line => JSON.parse(line))
    .filter(m => m.reason === 'compiler-artifact' && m.target?.name === 'noble' && m.profile?.test && m.executable);
  assert.equal(artifacts.length, 1, 'expected one noble unit-test executable');
  return artifacts[0].executable;
}

function main() {
  const destination = argument('--out');
  assert.ok(destination, 'usage: node verification/worker09/gate.mjs --out NEW_DIRECTORY');
  const out = path.resolve(destination);
  assert.ok(!fs.existsSync(out), `output directory already exists: ${out}`);

  const sourceFiles = sourceSnapshot();
  const projection = caseProjection();
  const executable = buildTests();
  const raw = path.join(out, 'raw');
  fs.mkdirSync(raw, { recursive: true });
  const child = spawnSync(executable, [TEST, '--exact', '--nocapture', '--test-threads=1'], {
    cwd: root, encoding: 'utf8', timeout: 600_000, maxBuffer: 64 * 1024 * 1024,
    env: { ...process.env, NOBLE_WORKER09_REPORT_DIR: raw },
  });
  assert.equal(child.error, undefined, `${child.error}`);
  fs.writeFileSync(path.join(out, 'test.stdout'), child.stdout);
  fs.writeFileSync(path.join(out, 'test.stderr'), child.stderr);
  const reports = Object.fromEntries(fs.readdirSync(raw).sort()
    .map(name => [name.replace(/\.json$/, ''), JSON.parse(fs.readFileSync(path.join(raw, name), 'utf8'))]));

  const failures = [];
  if (child.status !== 0 || !/test result: ok\. 1 passed/.test(child.stdout)) failures.push('scoped profile test did not pass');
  const input = projection.input;
  const rows = [];
  for (const variant of input.variants) {
    if (DEFERRED[variant.case]) {
      rows.push({ variant: variant.case, expected: variant.outcome, execution: 'not-run', reason: DEFERRED[variant.case] });
      continue;
    }
    const report = reports[ROW[variant.case]];
    if (!report) { failures.push(`${variant.case}: no retained report`); continue; }
    const observed = report.outcome;
    const typedOutput = report.typed_output !== null && report.typed_output !== undefined;
    rows.push({ variant: variant.case, row: ROW[variant.case], expected: variant.outcome, observed,
      typed_output: typedOutput, diagnostic: report.diagnostic ?? null, interruption: report.interruption ?? null });
    if (observed !== OUTCOME[variant.outcome]) failures.push(`${variant.case}: expected ${variant.outcome}, observed ${observed}`);
    if (typedOutput !== variant.typed_output) failures.push(`${variant.case}: typed_output ${typedOutput}`);
    if (report.stage !== 'execution') failures.push(`${variant.case}: stopped at stage ${report.stage}`);
  }
  for (const name of CONTROLS) if (!reports[name]) failures.push(`${name}: no retained control report`);
  const executed = rows.filter(row => row.execution !== 'not-run');
  const steps = Object.values(reports).map(report => report.interruption?.execution_steps).filter(Number.isInteger);
  const intervals = Object.values(reports).map(report => report.interruption?.observed_max_guest_work_between_checks)
    .filter(Number.isInteger);
  const report = name => reports[name] ?? {};
  const classes = new Map();
  for (const row of executed) {
    const seen = classes.get(row.expected) ?? new Set();
    seen.add(row.observed);
    classes.set(row.expected, seen);
  }
  const distinct = [...classes.values()].every(set => set.size === 1)
    && new Set([...classes.values()].map(set => [...set][0])).size === classes.size;
  const deadline = report('host-deadline');
  const observed = {
    stage: executed.every(row => reports[row.row]?.stage === 'execution') ? 'bounded-execution' : 'admission',
    outcome: distinct && executed.every(row => row.observed === OUTCOME[row.expected]) ? 'each-outcome-distinct' : 'outcome-collision',
    observed_max_guest_work_between_checks: intervals.length ? Math.max(...intervals) : null,
    pure_code_exempt_from_limits: report('pure-loop-work-limit').outcome !== 'runtime-exhausted',
    nested_budget_reset: !(report('nested-invocation-budget').outcome === 'runtime-exhausted'
      && report('nested-alone').outcome === 'normal' && report('outer-alone').outcome === 'normal'),
    failed_output_exposed_as_T: Object.values(reports).some(r => r.outcome !== 'normal'
      && r.typed_output !== null && r.typed_output !== undefined),
    host_deadline_is_deterministic_fuel: !(deadline.outcome === 'host-deadline'
      && deadline.interruption?.execution_steps < STEP_CEILING),
    exhaustion_proves_divergence: !(report('pure-loop-work-limit').outcome === 'runtime-exhausted'
      && report('pure-loop-within-budget').outcome === 'normal'),
    budget_silently_increased: steps.some(value => value > STEP_CEILING)
      || ['pure-loop-work-limit', 'nested-invocation-budget']
        .some(name => report(name).interruption?.execution_steps !== STEP_CEILING),
    // H-LIMIT-03: equal programs need not consume equal budgets across
    // backends. The profile binds one engine and no other backend runs here.
    cross_backend_budget_equality_required: false,
  };
  for (const [field, value] of Object.entries(projection.expected)) {
    if (observed[field] !== value) failures.push(`${field}: expected ${JSON.stringify(value)}, observed ${JSON.stringify(observed[field])}`);
  }
  const revisions = new Set(Object.values(reports).map(r => r.accounting_revision));
  if (revisions.size !== 1 || !revisions.has('worker-typed/step-4096/interrupt-8')) failures.push('reports name more than the selected accounting revision');

  const receipt = {
    schema: 'noble-worker09-acceptance/v1',
    case: 'WORKER-09',
    result: failures.length === 0 ? 'passed' : 'failed',
    coverage: 'mw1-rows',
    failures,
    tested_commit: git('rev-parse', 'HEAD').trim(),
    source_scope: SCOPE,
    source_revision: `sha256:${sha(JSON.stringify(sourceFiles))}`,
    source_file_count: Object.keys(sourceFiles).length,
    case_projection_sha256: sha(JSON.stringify(projection)),
    case_projection: projection,
    binary: { name: 'noble unit-test binary', sha256: hash(executable),
      build: 'cargo test --locked -p noble-cli --bin noble --no-run', test: TEST },
    input_mapping: {
      accounting_revision: 'The symbolic test profile maps to the typed worker revision worker-typed/step-4096/interrupt-8.',
      max_guest_work_between_checks: 'Every eighth charged unit of execution or observation work calls the host check; the host records the work between checks itself.',
      outcomes: OUTCOME,
      'domain-Result-error': 'A schema whose Action has a Refused Text error arm; the worker schema catalog does not admit the generic result@1.Result.',
      'pure-loop-work-limit': 'Pure source has no unbounded loop: a 256-leaf pure recipe applied twelve times with run exceeds 4096 units; eight applications return normally.',
      'runtime-builder-allocation-limit': 'Eight self-compositions exceed the 256 recipe-leaf limit; seven return normally.',
      'nested-invocation-budget': 'Outer and nested run work exhaust the shared counter while each part alone returns normally.',
      controls: 'Cancellation, guest trap and host fault are host-selected at an interruption check; a zero wall-clock deadline exercises host-deadline with budget left.',
    },
    observed,
    observation_methods: {
      stage: 'Every executed row\'s typed worker report reached stage execution, after admission.',
      outcome: 'Each executed variant\'s report outcome equals its mapped case outcome, and distinct case outcomes map to distinct report outcomes.',
      observed_max_guest_work_between_checks: 'Largest work difference the host recorded between consecutive interruption checks, including the start and the tail, over every retained report.',
      pure_code_exempt_from_limits: 'The pure-loop row, whose effect bound is empty, exhausts.',
      nested_budget_reset: 'Outer plus nested run work exhausts while the outer and nested parts alone each return normally.',
      failed_output_exposed_as_T: 'Any retained report with a non-normal outcome and a typed output.',
      host_deadline_is_deterministic_fuel: 'The zero-deadline control reports host-deadline with execution steps below the 4096 ceiling.',
      exhaustion_proves_divergence: 'The exhausting pure loop is a finite program whose eight-application form returns normally.',
      budget_silently_increased: 'Any retained execution step count above 4096, or an exhausting row that stopped anywhere but exactly 4096.',
      cross_backend_budget_equality_required: 'Declared: the revision binds the typed V8 engine; no other backend is run or compared.',
    },
    rows,
    deferred_mw2_rows: Object.keys(DEFERRED),
    controls: Object.fromEntries(CONTROLS.map(name => [name, { outcome: report(name).outcome ?? null,
      interruption: report(name).interruption ?? null }])),
    reports_sha256: Object.fromEntries(fs.readdirSync(raw).sort().map(name => [name, hash(path.join(raw, name))])),
    source_files: sourceFiles,
    non_claims: [
      'host-deadline-native-work-outstanding and permitted-suspension were not run; they remain MW2 obligations.',
      'The cancellation, trap and internal-failure controls are host-selected test-build fault points, not candidate or CLI inputs.',
      'Budgets are this engine\'s; no cross-backend equality is claimed.',
      'No proof obligation is discharged.',
    ],
  };
  fs.writeFileSync(path.join(out, 'acceptance.json'), `${JSON.stringify(receipt, null, 2)}\n`);
  console.log(JSON.stringify({ result: receipt.result, failures, observed,
    source_revision: receipt.source_revision, binary_sha256: receipt.binary.sha256 }));
  process.exitCode = failures.length === 0 ? 0 : 1;
}

main();
