#!/usr/bin/env node
// WORKER-10 acceptance gate.
//
// Builds the production `noble` binary from a clean checkout and runs the
// bounded operation observation embedder (`noble worker program test-host
// send-observe*`). The host selects the exact `test.send` binding, its own
// exact-plan grant table, a synthetic in-process peer and the observation
// bounds before it reads the candidate source on stdin, which must equal the
// preselected fixture. The guest makes three `send` requests and learns no
// operation outcome. The host authorizes each request, admits and starts it
// through `noble_kernel::authority`, hands it to the peer and records kernel
// observations; it cancels the invocation while attempt-3's acknowledgement
// is outstanding.
//
// Each case variant is one host mode. Every expected field is derived from
// the record's rows, counters, kernel-retained attempt outcomes, omitted scope
// and diagnostic, cross-checked between variants; no field is copied from a
// host boolean. A changed candidate source is refused before any request.
//
// Usage (inside the pinned devShell):
//   node verification/worker10/gate.mjs --out NEW_DIRECTORY
import assert from 'node:assert/strict';
import { execFileSync, spawnSync } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = fs.realpathSync(path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..'));
const here = path.join(root, 'verification/worker10');
const SCOPE = ['crates', 'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml',
  'verification/worker10/gate.mjs', 'verification/worker10/fixtures'];
const CASE_FILE = 'specs/conformance/worker-cases.json';
const PAYLOADS = ['w10-a', 'w10-b', 'w10-c', 'w10-d'];
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const hash = file => sha(fs.readFileSync(file));

// Case variant -> host mode and the bounds that mode selects.
const VARIANTS = {
  'sufficient-observation-budget': { mode: 'send-observe', rows: 16, diagnostic: 512 },
  'observation-output-exhausted': { mode: 'send-observe-rows', rows: 6, diagnostic: 512 },
  'diagnostic-output-exhausted': { mode: 'send-observe-diagnostic', rows: 16, diagnostic: 48 },
};
// Case observation names are the record's row observation names.
const FIELD = name => name.replaceAll('-', '_');

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
  const selected = cases.filter(c => c.id === 'WORKER-10');
  assert.equal(selected.length, 1, 'expected exactly one WORKER-10 case');
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
  const workdir = fs.mkdtempSync(path.join(os.tmpdir(), 'noble-worker10-'));
  const stdin = fs.readFileSync(path.join(here, 'fixtures', fixture));
  const child = spawnSync(noble, ['worker', 'program', 'test-host', mode], {
    cwd: workdir, input: stdin, encoding: 'buffer', timeout: 300_000, maxBuffer: 16 * 1024 * 1024,
  });
  assert.equal(child.error, undefined, `${name}: ${child.error}`);
  const leftovers = fs.readdirSync(workdir);
  fs.rmSync(workdir, { recursive: true, force: true });
  fs.writeFileSync(path.join(out, 'raw', `${name}.stdout.json`), child.stdout);
  fs.writeFileSync(path.join(out, 'raw', `${name}.stderr`), child.stderr);
  const text = `${child.stdout.toString('utf8')}${child.stderr.toString('utf8')}`;
  return {
    name,
    argv: ['noble', 'worker', 'program', 'test-host', mode],
    stdin: `verification/worker10/fixtures/${fixture}`,
    exit: child.status,
    stdout_sha256: sha(child.stdout),
    stderr_sha256: sha(child.stderr),
    files_left_in_working_directory: leftovers.length,
    payload_bytes_exported: PAYLOADS.filter(payload => text.includes(payload)),
    report: JSON.parse(child.stdout.toString('utf8')),
  };
}

// Row observations grouped by attempt, in sequence order.
function byAttempt(rows) {
  const grouped = new Map();
  for (const row of [...rows].sort((a, b) => a.sequence - b.sequence)) {
    if (!grouped.has(row.attempt)) grouped.set(row.attempt, []);
    grouped.get(row.attempt).push(row.observation);
  }
  return grouped;
}

function retainedOutcome(record, attempt) {
  return record.ownership_disposition.retained_attempts.find(row => row.attempt === attempt)?.outcome;
}

// Per-run checks that need no other variant. Each returns a failure or null.
function runChecks(row, variant, input) {
  const failures = [];
  const record = row.report.record;
  const bounds = VARIANTS[variant];
  if (row.exit !== 2) failures.push(`exit ${row.exit}, expected 2 for a cancelled invocation`);
  if (row.files_left_in_working_directory !== 0) failures.push('left files in the working directory');
  if (row.payload_bytes_exported.length !== 0) failures.push(`exported payload Text ${row.payload_bytes_exported}`);
  if (record === undefined) return [...failures, 'no observation record'];
  if (record.execution_profile.observation_rows !== bounds.rows
    || record.execution_profile.diagnostic_bytes !== bounds.diagnostic) failures.push('profile bounds differ from the mode');
  const rows = record.operation_attempt_correlation.rows;
  if (rows.length !== record.operation_attempt_correlation.retained || rows.length > bounds.rows) {
    failures.push('retained rows exceed or misstate the row bound');
  }
  if (record.invocation !== input.invocation || rows.some(r => r.invocation !== input.invocation)) {
    failures.push('a row or the record lost invocation correlation');
  }
  if (record.diagnostic.bytes > record.diagnostic.limit
    || Buffer.byteLength(record.diagnostic.text) !== record.diagnostic.bytes) failures.push('diagnostic exceeds its bound');
  const interfaceText = record.program_build_context.program_interface;
  const declared = /! \{(.*)\}$/.exec(interfaceText)?.[1].split(', ').filter(Boolean);
  if (JSON.stringify(declared) !== JSON.stringify(input.declared_effects)) {
    failures.push(`declared effects ${interfaceText} differ from ${input.declared_effects}`);
  }
  if (record.terminal_event !== input.terminal_event) failures.push(`terminal event ${record.terminal_event}`);
  // A complete record's rows independently recount every counter.
  if (record.omitted_observation_scope.observations_omitted === 0) {
    const count = kind => rows.filter(r => r.observation === kind).length;
    const counters = record.counters;
    if (count('requested') !== counters.guest_requests || count('denied') !== counters.denied_requests
      || count('admitted') !== counters.protected_operation_admissions
      || count('completed') !== counters.completed_operations
      || count('externally-unknown') !== record.unknown_operation_outcomes.length
      || rows.length !== counters.observations_total) failures.push('complete rows disagree with the counters');
  }
  return failures;
}

function main() {
  const destination = argument('--out');
  assert.ok(destination, 'usage: node verification/worker10/gate.mjs --out NEW_DIRECTORY');
  const out = path.resolve(destination);
  assert.ok(!fs.existsSync(out), `output directory already exists: ${out}`);

  const sourceFiles = sourceSnapshot();
  const projection = caseProjection();
  const { input, expected } = projection;
  assert.deepEqual([...input.variants].sort(), Object.keys(VARIANTS).sort(), 'gate variants differ from the case');
  const build = buildNoble();
  fs.mkdirSync(path.join(out, 'raw'), { recursive: true });

  const runs = Object.fromEntries(input.variants.map(variant =>
    [variant, run(build.executable, variant, VARIANTS[variant].mode, 'sender.noble', out)]));
  const control = run(build.executable, 'changed-source-control', 'send-observe', 'sender-extra-request.noble', out);

  const failures = [];
  for (const [variant, row] of Object.entries(runs)) {
    for (const failure of runChecks(row, variant, input)) failures.push(`${variant}: ${failure}`);
  }
  if (!(control.exit === 2 && control.report.outcome === 'invalid-input' && control.report.record === undefined
    && control.report.effect_requests === 0 && control.payload_bytes_exported.length === 0)) {
    failures.push('changed-source-control: a changed candidate reached the host or exported a payload');
  }
  const record = variant => runs[variant].report.record ?? {};
  const sufficient = record('sufficient-observation-budget');
  const rowsExhausted = record('observation-output-exhausted');
  const diagnosticExhausted = record('diagnostic-output-exhausted');

  // Exact operation correlation, from the complete record.
  const grouped = byAttempt(sufficient.operation_attempt_correlation?.rows ?? []);
  const correlation = input.operations.every(operation =>
    JSON.stringify(grouped.get(operation.attempt)) === JSON.stringify(operation.observations))
    && grouped.size === input.operations.length;
  if (!correlation) failures.push('sufficient-observation-budget: rows differ from the case operations');
  const fullRows = JSON.stringify(sufficient.operation_attempt_correlation?.rows ?? null);
  const prefix = JSON.stringify((sufficient.operation_attempt_correlation?.rows ?? [])
    .slice(0, rowsExhausted.operation_attempt_correlation?.retained ?? 0));
  if (JSON.stringify(rowsExhausted.operation_attempt_correlation?.rows) !== prefix) {
    failures.push('observation-output-exhausted: retained rows are not the leading rows of the complete record');
  }
  if (JSON.stringify(diagnosticExhausted.operation_attempt_correlation?.rows) !== fullRows) {
    failures.push('diagnostic-output-exhausted: rows differ from the complete record');
  }

  // Omission is marked exactly when a bound is exhausted.
  const scope = rec => rec.omitted_observation_scope ?? {};
  const total = rec => rec.operation_attempt_correlation?.total;
  const retained = rec => rec.operation_attempt_correlation?.retained;
  const sufficientUnmarked = scope(sufficient).complete === true && scope(sufficient).observations_omitted === 0
    && scope(sufficient).diagnostic_bytes_omitted === 0 && retained(sufficient) === total(sufficient)
    && sufficient.diagnostic?.truncated === false;
  const rowsMarked = scope(rowsExhausted).complete === false && scope(rowsExhausted).observations_omitted > 0
    && retained(rowsExhausted) + scope(rowsExhausted).observations_omitted === total(rowsExhausted)
    && scope(rowsExhausted).first_omitted_sequence === retained(rowsExhausted) + 1;
  const fullDiagnostic = sufficient.diagnostic?.text ?? '';
  const diagnosticMarked = scope(diagnosticExhausted).complete === false
    && diagnosticExhausted.diagnostic?.truncated === true
    && scope(diagnosticExhausted).observations_omitted === 0
    && scope(diagnosticExhausted).diagnostic_bytes_omitted > 0
    && fullDiagnostic.startsWith(diagnosticExhausted.diagnostic?.text)
    && diagnosticExhausted.diagnostic.bytes + scope(diagnosticExhausted).diagnostic_bytes_omitted
      === Buffer.byteLength(fullDiagnostic);
  const omissionMarked = sufficientUnmarked && rowsMarked && diagnosticMarked;

  // The rows variant omits attempt-3's admission and unknown outcome rows.
  const omittedRows = (sufficient.operation_attempt_correlation?.rows ?? [])
    .slice(retained(rowsExhausted) ?? 0).map(row => `${row.attempt}:${row.observation}`);
  const attemptsInScope = scope(rowsExhausted).first_attempt === 'attempt-3' && scope(rowsExhausted).last_attempt === 'attempt-3';
  const missingImpliesNoEffect = !(omittedRows.includes('attempt-3:admitted')
    && omittedRows.includes('attempt-3:externally-unknown') && attemptsInScope
    && retainedOutcome(rowsExhausted, 'attempt-3') === 'unknown'
    && rowsExhausted.counters?.protected_operation_admissions === 2
    && rowsExhausted.unknown_operation_outcomes?.includes('attempt-3')
    && rowsExhausted.synthetic_peer?.deliveries_received === 2);

  const observedFor = variant => {
    const rec = record(variant);
    const counters = rec.counters ?? {};
    const rows = rec.operation_attempt_correlation?.rows ?? [];
    const requestedAttempts = rows.filter(row => row.observation === 'requested').map(row => row.attempt);
    const admittedAttempts = rows.filter(row => row.observation === 'admitted').map(row => row.attempt);
    const attempt3 = rec.unknown_operation_outcomes?.includes('attempt-3') && retainedOutcome(rec, 'attempt-3') === 'unknown'
      ? 'unknown' : (retainedOutcome(rec, 'attempt-3') ?? 'absent');
    const priorRetained = retainedOutcome(rec, 'attempt-1') === 'completed'
      && rows.some(row => row.attempt === 'attempt-1' && row.observation === 'completed');
    const completedFromKernel = (rec.ownership_disposition?.retained_attempts ?? [])
      .filter(row => row.outcome === 'completed').length;
    return {
      stage: runs[variant].report.stage,
      outcome: rec.primary_outcome === 'cancelled' && rec.terminal_event === 'cancellation' && priorRetained
        ? 'cancelled-with-prior-effects-retained' : `${rec.primary_outcome}-prior-${priorRetained ? 'retained' : 'lost'}`,
      required_fields: expected.required_fields.filter(field => rec[FIELD(field)] !== undefined),
      primary_outcome: rec.primary_outcome,
      guest_requests: counters.guest_requests,
      protected_operation_admissions: counters.protected_operation_admissions,
      completed_operations: counters.completed_operations === completedFromKernel ? counters.completed_operations : 'inconsistent',
      denied_attempt_counted_as_request: requestedAttempts.includes('attempt-2')
        && rows.some(row => row.attempt === 'attempt-2' && row.observation === 'denied')
        && counters.guest_requests === counters.protected_operation_admissions + counters.denied_requests,
      attempt_3_outcome: attempt3,
      omission_marked_when_exhausted: omissionMarked,
      prior_effects_rolled_back: !priorRetained,
      effect_plan_counts_as_completion: !(counters.completed_operations < counters.protected_operation_admissions
        && counters.protected_operation_admissions < counters.guest_requests
        && counters.started_operations === counters.protected_operation_admissions),
      missing_observations_imply_no_effects: missingImpliesNoEffect,
      automatic_retry: !(new Set(requestedAttempts).size === requestedAttempts.length
        && new Set(admittedAttempts).size === admittedAttempts.length
        && rec.synthetic_peer?.deliveries_received === counters.protected_operation_admissions
        && counters.started_operations === counters.protected_operation_admissions),
    };
  };
  const observed = Object.fromEntries(Object.keys(runs).map(variant => [variant, observedFor(variant)]));
  for (const [variant, values] of Object.entries(observed)) {
    for (const [field, value] of Object.entries(expected)) {
      if (JSON.stringify(values[field]) !== JSON.stringify(value)) {
        failures.push(`${variant}: ${field}: expected ${JSON.stringify(value)}, observed ${JSON.stringify(values[field])}`);
      }
    }
  }

  const receipt = {
    schema: 'noble-worker10-acceptance/v1',
    case: 'WORKER-10',
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
    stage_mapping: 'The embedder report stage host-outcome-record carries the record; its primary_outcome is the case primary outcome.',
    input_mapping: {
      declared_effects: 'The checked Program interface I64 -- I64 ! {test.send} of the preselected fixture, bound to worker-test-send@1.0.0 slot 5.',
      invocation: 'The kernel authority owner context invocation number of the one issued invocation.',
      operations: 'attempt-N is the host-assigned N-th request; its rows are the record rows of that attempt in sequence order.',
      'attempt-1': 'Granted by the host table, admitted and started; the synthetic peer acknowledgement is observed as kernel OperationSuccess.',
      'attempt-2': 'Absent from the host grant table, so the kernel authority denies its exact plan at authorization; the guest continues.',
      'attempt-3': 'Granted, admitted, started and delivered; its acknowledgement is lost, the host cancels the invocation and observes kernel Unknown.',
      terminal_event: 'Host-selected cancellation of the kernel authority invocation and the Worker task owner while attempt-3 is outstanding.',
      variants: Object.fromEntries(Object.entries(VARIANTS).map(([variant, bounds]) =>
        [variant, `test-host ${bounds.mode}: ${bounds.rows} retained rows, ${bounds.diagnostic} diagnostic bytes`])),
      required_fields: 'Each hyphenated field is the underscored record key.',
    },
    observed,
    observation_methods: {
      outcome: 'Primary outcome cancelled with terminal event cancellation, and attempt-1 still completed in the kernel-retained attempts and the rows after the cancellation.',
      required_fields: 'The expected fields present as record keys.',
      guest_requests: 'Record counter of import calls; the complete record has one requested row per attempt.',
      protected_operation_admissions: 'Kernel authority attempts_admitted.',
      completed_operations: 'Record counter, which must equal the kernel-retained attempts whose outcome is an approved success observation.',
      denied_attempt_counted_as_request: 'attempt-2 has requested and denied rows, and requests equal admissions plus denials.',
      attempt_3_outcome: 'attempt-3 in the unknown list and its kernel-retained outcome unknown: started with no terminal observation.',
      omission_marked_when_exhausted: 'The sufficient record is complete and untruncated; the rows record omits a counted contiguous tail starting after its retained rows; the diagnostic record keeps a counted prefix of the complete diagnostic.',
      prior_effects_rolled_back: 'attempt-1 completion missing after the cancellation.',
      effect_plan_counts_as_completion: 'Not (completed < admitted < requested, with every admission started).',
      missing_observations_imply_no_effects: 'In the rows record, attempt-3 admitted and unknown rows are omitted and the scope names attempt-3, yet the record keeps two admissions, attempt-3 unknown and two peer deliveries.',
      automatic_retry: 'Any repeated requested or admitted attempt, or peer deliveries or starts other than the admissions.',
    },
    runs: { variants: runs, control },
    source_files: sourceFiles,
    non_claims: [
      'The peer is synthetic and in-process: no network, native asynchronous work or late completion handling.',
      'The cancellation is host-selected and fires when an admitted attempt\'s acknowledgement is lost; late acknowledgements and their reconciliation are WORKER-01 (MW2) obligations.',
      'The grant table, acknowledgement script and output bounds are host constants selected per mode.',
      'No proof obligation is discharged.',
    ],
  };
  fs.writeFileSync(path.join(out, 'acceptance.json'), `${JSON.stringify(receipt, null, 2)}\n`);
  console.log(JSON.stringify({ result: receipt.result, failures, observed: observed['sufficient-observation-budget'],
    source_revision: receipt.source_revision, binary_sha256: receipt.binary.sha256 }));
  process.exitCode = failures.length === 0 ? 0 : 1;
}

main();
