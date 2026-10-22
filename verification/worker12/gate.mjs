#!/usr/bin/env node
// WORKER-12 acceptance gate.
//
// Builds the production `noble` binary from a clean checkout and runs the
// dedicated synthetic Worker test embedder (`noble worker program test-host`)
// with the checked private-capture fixture. The embedder installs the actual
// compiled guest component and source module, binds the checked Program owner
// and sends the guest's one `request-code-export` to a default-deny host.
//
// The host report states only what the host counts. Publication is observed
// here, outside the process: each run starts in a fresh empty directory that
// must stay empty, and no captured Text may appear on stdout or stderr. A
// control with an unselected source must be refused before any guest runs, so
// the denial is attributable to the missing export authority alone.
//
// Usage (inside the pinned devShell):
//   node verification/worker12/gate.mjs --out NEW_DIRECTORY
import assert from 'node:assert/strict';
import { execFileSync, spawnSync } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = fs.realpathSync(path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..'));
const here = path.join(root, 'verification/worker12');
const SCOPE = ['crates', 'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml',
  'verification/worker12/gate.mjs', 'verification/worker12/fixtures'];
const CASE_FILE = 'specs/conformance/worker-cases.json';
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const hash = file => sha(fs.readFileSync(file));

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
  const selected = cases.filter(c => c.id === 'WORKER-12');
  assert.equal(selected.length, 1, 'expected exactly one WORKER-12 case');
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

// Every Text literal of the checked fixture is captured data.
function captureLiterals(source) {
  const literals = [...source.matchAll(/"([^"]*)"/g)].map(match => match[1]);
  assert.ok(literals.length > 0, 'fixture captures no Text');
  return literals;
}

function run(noble, name, mode, fixture, out, captures) {
  const workdir = fs.mkdtempSync(path.join(os.tmpdir(), 'noble-worker12-'));
  const stdin = fs.readFileSync(path.join(here, 'fixtures', fixture));
  const child = spawnSync(noble, ['worker', 'program', 'test-host', mode], {
    cwd: workdir, input: stdin, encoding: 'buffer', timeout: 300_000, maxBuffer: 16 * 1024 * 1024,
  });
  assert.equal(child.error, undefined, `${name}: ${child.error}`);
  const leftovers = fs.readdirSync(workdir);
  fs.rmSync(workdir, { recursive: true, force: true });
  fs.writeFileSync(path.join(out, 'raw', `${name}.stdout.json`), child.stdout);
  fs.writeFileSync(path.join(out, 'raw', `${name}.stderr`), child.stderr);
  const printed = Buffer.concat([child.stdout, child.stderr]);
  const leakedCaptures = captures.filter(text => printed.includes(Buffer.from(text)));
  return {
    name,
    argv: ['noble', 'worker', 'program', 'test-host', mode],
    stdin: `verification/worker12/fixtures/${fixture}`,
    exit: child.status,
    stdout_sha256: sha(child.stdout),
    stderr_sha256: sha(child.stderr),
    files_left_in_working_directory: leftovers.length,
    captured_text_on_stdout_or_stderr: leakedCaptures.length,
    report: JSON.parse(child.stdout.toString('utf8')),
  };
}

function main() {
  const destination = argument('--out');
  assert.ok(destination, 'usage: node verification/worker12/gate.mjs --out NEW_DIRECTORY');
  const out = path.resolve(destination);
  assert.ok(!fs.existsSync(out), `output directory already exists: ${out}`);

  const sourceFiles = sourceSnapshot();
  const projection = caseProjection();
  const fixture = fs.readFileSync(path.join(here, 'fixtures/export.noble'), 'utf8');
  const captures = captureLiterals(fixture);
  const build = buildNoble();
  fs.mkdirSync(path.join(out, 'raw'), { recursive: true });

  const wit = run(build.executable, 'wit-export', 'wit-export', 'export.noble', out, captures);
  const core = run(build.executable, 'core-export', 'core-export', 'export.noble', out, captures);
  const unselected = run(build.executable, 'unselected-source', 'wit-export',
    'export-unselected.noble', out, captures);

  const denial = wit.report.denial ?? {};
  const published = (row) => row.files_left_in_working_directory > 0
    || row.captured_text_on_stdout_or_stderr > 0;
  const capturePublished = published(wit) || published(core);
  const observed = {
    stage: denial.stage,
    outcome: denial.outcome,
    guest_requests: denial.guest_requests,
    protected_operations: denial.protected_operations,
    capture_published: capturePublished,
    // Capturability grants export authority only if a request for the
    // capturable Program was not refused as denied, or its bytes got out.
    capturability_grants_export_authority: !(wit.report.outcome === 'denied'
      && denial.denial_recorded === true && denial.protected_operations === 0) || capturePublished,
    candidate_body_requests: denial.candidate_body_requests,
    denial_recorded: denial.denial_recorded,
  };
  const failures = [];
  for (const [field, value] of Object.entries(projection.expected)) {
    if (observed[field] !== value) failures.push(`${field}: expected ${JSON.stringify(value)}, observed ${JSON.stringify(observed[field])}`);
  }
  if (wit.exit !== 2 || wit.report.operation !== 'wit.request-code-export') failures.push('wit-export did not end as a denied WIT export request');
  const coreDenial = core.report.denial ?? {};
  if (!(core.exit === 2 && core.report.operation === 'core.test.code-export' && coreDenial.outcome === 'denied'
    && coreDenial.core_requests === 1 && coreDenial.core_denials === 1 && coreDenial.protected_operations === 0
    && coreDenial.completed_invocations === 0)) failures.push('core-export did not end as one denied Core request');
  if (!(unselected.exit === 2 && unselected.report.outcome === 'invalid-input' && unselected.report.denial === undefined
    && !published(unselected))) failures.push('unselected source was not refused before any guest request');

  const receipt = {
    schema: 'noble-worker12-acceptance/v1',
    case: 'WORKER-12',
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
    capture_literals: captures.length,
    input_mapping: {
      capture: 'The checked fixture captures two synthetic private Text literals.',
      declared_effects: 'The host-selected bound adapter is worker-test-code-export@1.0.0 for test.code-export.',
      destination_authority: 'The test embedder issues no export grant; its request-code-export host always denies.',
      program_valid: 'The fixture source equals the preselected source and is checked, compiled, installed and bound before the request.',
      digest_valid: 'The installed capture cell is attested against the checked Text before the request; no package digest is involved in this path.',
    },
    observed,
    observation_methods: {
      capture_published: 'Outside the host: each run starts in a fresh empty working directory that must stay empty, and no captured Text literal may appear on stdout or stderr.',
      capturability_grants_export_authority: 'Derived from the denied outcome, the recorded denial and zero protected operations of the request for the capturable Program, or any observed publication.',
    },
    runs: [wit, core, unselected],
    source_files: sourceFiles,
    non_claims: [
      'No positive exporter exists; the observation covers the files and output streams the host process can write, not channels outside it.',
      'The test embedder is a finite synthetic host fixture, not a production export issuer.',
      'No proof obligation is discharged.',
    ],
  };
  fs.writeFileSync(path.join(out, 'acceptance.json'), `${JSON.stringify(receipt, null, 2)}\n`);
  console.log(JSON.stringify({ result: receipt.result, failures, observed,
    source_revision: receipt.source_revision, binary_sha256: receipt.binary.sha256 }));
  process.exitCode = failures.length === 0 ? 0 : 1;
}

main();
