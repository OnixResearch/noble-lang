#!/usr/bin/env node
// WORKER-03 acceptance gate.
//
// Builds the production `noble` binary from a clean checkout and runs the
// dedicated synthetic Worker test embedder in its `builders` mode. The
// embedder checks and compiles the preselected fixture once: a pure
// `I64 -- I64 ! {}` Program and a host-bound `I64 -- I64 ! {test.emit}`
// Program in one source closure. After compilation it adapts the pure Program
// to the selected `{test.emit}` destination witness, stores both Programs in a
// typed list, selects, duplicates and composes them, and reports what it
// observed: compilations, recipe nodes and witnesses before and after, the
// synthetic ledger's effect receipt, and whether an allowance was issued.
// A control with an unselected source must be refused before any builder.
//
// Usage (inside the pinned devShell):
//   node verification/worker03/gate.mjs --out NEW_DIRECTORY
import assert from 'node:assert/strict';
import { execFileSync, spawnSync } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = fs.realpathSync(path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..'));
const here = path.join(root, 'verification/worker03');
const SCOPE = ['crates', 'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml',
  'verification/worker03/gate.mjs', 'verification/worker03/fixtures'];
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
  const selected = cases.filter(c => c.id === 'WORKER-03');
  assert.equal(selected.length, 1, 'expected exactly one WORKER-03 case');
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

function run(noble, name, fixture, out) {
  const workdir = fs.mkdtempSync(path.join(os.tmpdir(), 'noble-worker03-'));
  const stdin = fs.readFileSync(path.join(here, 'fixtures', fixture));
  const child = spawnSync(noble, ['worker', 'program', 'test-host', 'builders'], {
    cwd: workdir, input: stdin, encoding: 'buffer', timeout: 300_000, maxBuffer: 16 * 1024 * 1024,
  });
  assert.equal(child.error, undefined, `${name}: ${child.error}`);
  const leftovers = fs.readdirSync(workdir);
  fs.rmSync(workdir, { recursive: true, force: true });
  fs.writeFileSync(path.join(out, 'raw', `${name}.stdout.json`), child.stdout);
  fs.writeFileSync(path.join(out, 'raw', `${name}.stderr`), child.stderr);
  return {
    name,
    argv: ['noble', 'worker', 'program', 'test-host', 'builders'],
    stdin: `verification/worker03/fixtures/${fixture}`,
    exit: child.status,
    stdout_sha256: sha(child.stdout),
    stderr_sha256: sha(child.stderr),
    files_left_in_working_directory: leftovers.length,
    report: JSON.parse(child.stdout.toString('utf8')),
  };
}

function main() {
  const destination = argument('--out');
  assert.ok(destination, 'usage: node verification/worker03/gate.mjs --out NEW_DIRECTORY');
  const out = path.resolve(destination);
  assert.ok(!fs.existsSync(out), `output directory already exists: ${out}`);

  const sourceFiles = sourceSnapshot();
  const projection = caseProjection();
  const build = buildNoble();
  fs.mkdirSync(path.join(out, 'raw'), { recursive: true });

  const builders = run(build.executable, 'builders', 'builders.noble', out);
  const unselected = run(build.executable, 'unselected-source', 'builders-unselected.noble', out);
  const report = builders.report;
  const observed = {
    stage: report.stage,
    outcome: report.builder_result,
    source_compiler_calls: report.source_compiler_calls,
    guest_requests: report.guest_requests,
    protected_operations: report.protected_operations,
    recipe_nodes_unchanged_by_widening: report.recipe_nodes_unchanged_by_widening,
    original_interface_unchanged: report.original_interface_unchanged,
    destination_witness_retained: report.destination_witness_retained,
    duplicate_interfaces: report.duplicate_interfaces,
    authority_created: report.authority_created,
    identity_comparison_includes_interface_witness: report.identity_comparison_includes_interface_witness,
  };
  const failures = [];
  for (const [field, value] of Object.entries(projection.expected)) {
    if (observed[field] !== value) failures.push(`${field}: expected ${JSON.stringify(value)}, observed ${JSON.stringify(observed[field])}`);
  }
  const input = projection.input;
  if (JSON.stringify(report.program_interfaces) !== JSON.stringify(input.program_interfaces)) {
    failures.push(`program_interfaces: expected ${JSON.stringify(input.program_interfaces)}, observed ${JSON.stringify(report.program_interfaces)}`);
  }
  if (report.destination_interface !== input.destination_interface) {
    failures.push(`destination_interface: expected ${input.destination_interface}, observed ${report.destination_interface}`);
  }
  if (JSON.stringify(report.operations) !== JSON.stringify(input.operations)) {
    failures.push('operations differ from the case input');
  }
  if (builders.exit !== 0 || report.outcome !== 'normal') failures.push('builders run did not end normally');
  if (report.unadapted_list_refused !== true) failures.push('a list of the unadapted Programs was admitted');
  if (report.compiled_module_unchanged !== true) failures.push('the compiled module changed during the builders');
  if (report.composed?.interface !== input.destination_interface) failures.push('composition did not retain the destination witness');
  if (report.receipt?.issued_allowance !== false || report.receipt?.ledger_entries !== 0
    || report.receipt?.effect_requests !== 0) failures.push('the synthetic ledger observed a request or an issue');
  if (builders.files_left_in_working_directory !== 0) failures.push('builders left files in the working directory');
  if (!(unselected.exit === 2 && unselected.report.outcome === 'invalid-input'
    && unselected.report.builder_result === undefined)) {
    failures.push('unselected source was not refused before any builder');
  }

  const receipt = {
    schema: 'noble-worker03-acceptance/v1',
    case: 'WORKER-03',
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
      operands_supplied_after_compilation: 'Both Programs are handles into the closure the embedder checked and compiled once; the builders run after that compilation.',
      compiler_service: 'The Store counts its source compilations; the builders add none and the compiled WAT digest is unchanged.',
      program_interfaces: 'increment is I64 -- I64 ! {} and plus, which calls it after a bound test.emit request, is I64 -- I64 ! {test.emit}.',
      destination_interface: 'The host selects {test.emit}; adapt checks inclusion of the pure Program\'s empty bound.',
      adaptation: 'Store::adapt: the source effects must be included in the selected ceiling; it adds a witness, not a recipe node.',
      operations: 'adapt; store-in-list and select over [adapted pure, bound]; duplicate of the selected handle; compose of a duplicate with the bound Program and of the two duplicates.',
    },
    observed,
    observation_methods: {
      guest_requests: 'The synthetic ledger\'s effect receipt: effect requests seen by the bound test.emit host.',
      authority_created: 'True if the ledger receipt shows an issued allowance, a protected operation or a ledger entry.',
      duplicate_interfaces: 'same-instantiation when both copies are the selected handle itself and carry the adapted witness.',
      identity_comparison_includes_interface_witness: 'The adapted handle differs from the pure one although both name the same checked recipe.',
    },
    runs: [builders, unselected],
    source_files: sourceFiles,
    non_claims: [
      'The builders do not execute either Program; executing the adapted pure Program under the bound host is not covered.',
      'The test embedder is a finite synthetic host fixture with one preselected source, not a production issuer.',
      'No proof obligation is discharged.',
    ],
  };
  fs.writeFileSync(path.join(out, 'acceptance.json'), `${JSON.stringify(receipt, null, 2)}\n`);
  console.log(JSON.stringify({ result: receipt.result, failures, observed,
    source_revision: receipt.source_revision, binary_sha256: receipt.binary.sha256 }));
  process.exitCode = failures.length === 0 ? 0 : 1;
}

main();
