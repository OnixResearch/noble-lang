#!/usr/bin/env node
// MW1 Worker transition guest entry-gate.
//
// Proves the shared missing path behind the worker-interface-checker,
// declared-schemas-and-modules, preparation-service, package-encoding and
// bounded-execution-profile gates: a schema-selected Worker Program v2 runs
// through a real component guest, using canonical frames and an opaque owner.
// The host retains all nominal typing and observes bounded source outcomes.
//
// Usage (inside the pinned devShell):
//   node verification/worker-transition-guest/gate.mjs --out NEW_DIRECTORY
import assert from 'node:assert/strict';
import { execFileSync, spawnSync } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = fs.realpathSync(path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..'));
const here = path.join(root, 'verification/worker-transition-guest');
const SCOPE = ['crates', 'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml',
  'verification/worker-transition-guest/gate.mjs', 'verification/worker-transition-guest/fixtures'];
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const hash = file => sha(fs.readFileSync(file));
const FIXTURES = ['schema.noble', 'transition.noble', 'state.json', 'submit.json', 'cancel.json', 'exhausted.noble'];

function argument(name) {
  const at = process.argv.indexOf(name);
  return at > 0 ? process.argv[at + 1] : undefined;
}
function git(...args) {
  return execFileSync('git', args, { cwd: root, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 });
}
function sourceSnapshot() {
  const dirty = git('status', '--porcelain', '--untracked-files=all', '--', ...SCOPE);
  assert.equal(dirty, '', `source scope is not clean:\n${dirty}`);
  const files = git('ls-files', '-z', '--', ...SCOPE).split('\0').filter(Boolean).sort();
  return Object.fromEntries(files.map(file => [file, { sha256: hash(path.join(root, file)) }]));
}
function buildNoble() {
  const build = spawnSync('cargo', ['build', '--locked', '-p', 'noble-cli', '--bin', 'noble',
    '--message-format=json-render-diagnostics'], { cwd: root, encoding: 'utf8', maxBuffer: 256 * 1024 * 1024 });
  assert.equal(build.status, 0, `cargo build failed:\n${build.stderr}`);
  const rows = build.stdout.split('\n').filter(Boolean).map(line => JSON.parse(line))
    .filter(row => row.reason === 'compiler-artifact' && row.target?.name === 'noble' && row.executable);
  assert.equal(rows.length, 1, 'expected one noble executable');
  return rows[0].executable;
}
function fixture(name) { return fs.readFileSync(path.join(here, 'fixtures', name)); }
function run(noble, out, name, argv, { stdin, files = {}, keep = [] } = {}) {
  const cwd = fs.mkdtempSync(path.join(os.tmpdir(), 'noble-transition-'));
  for (const [file, bytes] of Object.entries(files)) fs.writeFileSync(path.join(cwd, file), bytes);
  const child = spawnSync(noble, argv, { cwd, input: stdin, encoding: 'buffer', timeout: 300_000,
    maxBuffer: 32 * 1024 * 1024 });
  assert.equal(child.error, undefined, `${name}: ${child.error}`);
  fs.writeFileSync(path.join(out, 'raw', `${name}.stdout.json`), child.stdout);
  fs.writeFileSync(path.join(out, 'raw', `${name}.stderr`), child.stderr);
  const retained = {};
  for (const file of keep) retained[file] = fs.readFileSync(path.join(cwd, file));
  const leftovers = fs.readdirSync(cwd).filter(file => !(file in files) && !keep.includes(file));
  fs.rmSync(cwd, { recursive: true, force: true });
  return { name, argv: ['noble', ...argv], exit: child.status,
    stdout_sha256: sha(child.stdout), stderr_sha256: sha(child.stderr), leftovers,
    report: JSON.parse(child.stdout.toString('utf8')), retained };
}

function main() {
  const destination = argument('--out');
  assert.ok(destination, 'usage: gate.mjs --out NEW_DIRECTORY');
  const out = path.resolve(destination);
  assert.ok(!fs.existsSync(out), `output exists: ${out}`);
  const sourceFiles = sourceSnapshot();
  const noble = buildNoble();
  fs.mkdirSync(path.join(out, 'raw'), { recursive: true });
  const files = Object.fromEntries(FIXTURES.filter(name => name !== 'exhausted.noble').map(name => [name, fixture(name)]));

  const prepared = run(noble, out, 'prepare-v2', ['worker', 'program', 'prepare-transition',
    'transition.noble', '--name', 'transition', '--schema', 'schema.noble', '--out', 'transition-v2.json',
    '--effect-ceiling', 'pure'], { files, keep: ['transition-v2.json'] });
  const packageBytes = prepared.retained['transition-v2.json'];
  fs.writeFileSync(path.join(out, 'raw', 'transition-v2.json'), packageBytes);
  const source = run(noble, out, 'source', ['worker', 'program', 'guest-transition', 'transition.noble',
    '--name', 'transition', '--schema', 'schema.noble', '--state', 'state.json', '--event', 'submit.json',
    '--effect-ceiling', 'pure'], { files });
  const fromPackage = run(noble, out, 'package', ['worker', 'program', 'guest-transition-package',
    'transition-v2.json', '--schema', 'schema.noble', '--state', 'state.json', '--event', 'submit.json',
    '--effect-ceiling', 'pure'], { files: { ...files, 'transition-v2.json': packageBytes } });

  const preparedV1 = run(noble, out, 'prepare-v1', ['worker', 'prepare', 'transition.noble',
    '--schema', 'schema.noble', '--out', 'transition-v1.json'], { files, keep: ['transition-v1.json'] });
  const typed = run(noble, out, 'typed', ['worker', 'typed', 'transition-v1.json', '--schema',
    'schema.noble', '--state', 'state.json', '--event', 'submit.json'],
  { files: { ...files, 'transition-v1.json': preparedV1.retained['transition-v1.json'] } });

  const controls = {};
  for (const [name, sourceName] of [
    ['normal', 'transition.noble'], ['cancel', 'transition.noble'], ['deadline', 'transition.noble'],
    ['trap', 'transition.noble'], ['exhausted', 'exhausted.noble']]) {
    controls[name] = run(noble, out, `control-${name}`,
      ['worker', 'program', 'test-host', `transition-${name}`], { stdin: fixture(sourceName) });
  }

  const noncanonical = Buffer.concat([fixture('state.json'), Buffer.from(' ')]);
  const badSchema = Buffer.from(fixture('schema.noble').toString().replace('module WorkerData@1', 'module OtherData@1'));
  const hostile = {
    swapped: run(noble, out, 'hostile-swapped', ['worker', 'program', 'guest-transition', 'transition.noble',
      '--name', 'transition', '--schema', 'schema.noble', '--state', 'submit.json', '--event', 'state.json',
      '--effect-ceiling', 'pure'], { files }),
    noncanonical: run(noble, out, 'hostile-noncanonical', ['worker', 'program', 'guest-transition', 'transition.noble',
      '--name', 'transition', '--schema', 'schema.noble', '--state', 'noncanonical.json', '--event', 'submit.json',
      '--effect-ceiling', 'pure'], { files: { ...files, 'noncanonical.json': noncanonical } }),
    schema: run(noble, out, 'hostile-schema', ['worker', 'program', 'guest-transition', 'transition.noble',
      '--name', 'transition', '--schema', 'bad-schema.noble', '--state', 'state.json', '--event', 'submit.json',
      '--effect-ceiling', 'pure'], { files: { ...files, 'bad-schema.noble': badSchema } }),
  };
  const wrongWorld = JSON.parse(packageBytes.toString('utf8'));
  wrongWorld.wit_world = 'noble-test:worker-program/guest@2.0.0';
  hostile.world = run(noble, out, 'hostile-world', ['worker', 'program', 'guest-transition-package',
    'wrong-world.json', '--schema', 'schema.noble', '--state', 'state.json', '--event', 'submit.json',
    '--effect-ceiling', 'pure'], { files: { ...files, 'wrong-world.json': Buffer.from(JSON.stringify(wrongWorld)) } });
  const overwrite = run(noble, out, 'hostile-overwrite', ['worker', 'program', 'prepare-transition',
    'transition.noble', '--name', 'transition', '--schema', 'schema.noble', '--out', 'existing.json',
    '--effect-ceiling', 'pure'], { files: { ...files, 'existing.json': Buffer.from('sentinel') } });

  const failures = [];
  const all = [prepared, source, fromPackage, preparedV1, typed, ...Object.values(controls),
    ...Object.values(hostile), overwrite];
  for (const row of all) if (row.leftovers.length) failures.push(`${row.name}: left files ${row.leftovers}`);
  if (!(prepared.exit === 0 && prepared.report.outcome === 'ready'
    && prepared.report.service_effects.input_files_read === 2
    && prepared.report.service_effects.source_checks === 3
    && prepared.report.service_effects.closure_walks === 1
    && prepared.report.service_effects.backend_compilations === 1
    && prepared.report.service_effects.package_writes === 1
    && prepared.report.service_effects.engines_started === 0)) failures.push('v2 preparation trace differs');
  for (const row of [source, fromPackage]) {
    if (!(row.exit === 0 && row.report.outcome === 'normal' && row.report.program_admissions === 1
      && row.report.program_invocations === 1 && row.report.live_owners === 0
      && row.report.effect_requests === 0 && row.report.protected_operations === 0
      && row.report.interruption.observed_max_guest_work_between_checks <= 8)) failures.push(`${row.name}: transition observations differ`);
  }
  if (JSON.stringify(source.report.typed_output) !== JSON.stringify(fromPackage.report.typed_output))
    failures.push('source and package typed outputs differ');
  if (JSON.stringify(source.report.typed_output) !== JSON.stringify(typed.report.typed_output))
    failures.push('v1 and v2 typed outputs differ');
  const expectedOutcomes = { normal: ['normal', 0, true], cancel: ['cancelled', 2, false],
    deadline: ['host-deadline', 2, false], trap: ['trap', 1, false],
    exhausted: ['runtime-exhausted', 5, false] };
  for (const [name, [outcome, exit, typedOutput]] of Object.entries(expectedOutcomes)) {
    const row = controls[name];
    if (!(row.exit === exit && row.report.outcome === outcome
      && (row.report.typed_output !== null) === typedOutput
      && row.report.interruption.observed_max_guest_work_between_checks <= 8
      && row.report.program_admissions === 1 && row.report.program_invocations === 1))
      failures.push(`control-${name}: expected ${outcome}`);
  }
  for (const [name, row] of Object.entries(hostile)) {
    if (row.report.outcome === 'normal' || row.exit === 0) failures.push(`hostile-${name}: admitted`);
  }
  if (!(overwrite.exit === 2 && overwrite.report.outcome === 'invalid-input'))
    failures.push('prepare overwrote its destination');

  const receipt = {
    schema: 'noble-worker-transition-guest-gate/v1', result: failures.length ? 'failed' : 'passed', failures,
    tested_commit: git('rev-parse', 'HEAD').trim(), source_scope: SCOPE,
    source_revision: `sha256:${sha(JSON.stringify(sourceFiles))}`,
    source_file_count: Object.keys(sourceFiles).length,
    binary: { name: 'noble', sha256: hash(noble), build: 'cargo build --locked -p noble-cli --bin noble' },
    fixtures: Object.fromEntries(FIXTURES.map(name => [name, hash(path.join(here, 'fixtures', name))])),
    package_sha256: sha(packageBytes),
    gates_closed: ['worker-interface-checker', 'declared-schemas-and-modules', 'preparation-service',
      'package-encoding', 'bounded-execution-profile'],
    observations: {
      typed_output_parity: JSON.stringify(source.report.typed_output) === JSON.stringify(typed.report.typed_output),
      source_package_parity: JSON.stringify(source.report.typed_output) === JSON.stringify(fromPackage.report.typed_output),
      outcomes: Object.fromEntries(Object.entries(controls).map(([name, row]) => [name, row.report.outcome])),
      max_intervals: Object.fromEntries(Object.entries(controls).map(([name, row]) =>
        [name, row.report.interruption.observed_max_guest_work_between_checks])),
      hostile_refusals: Object.fromEntries(Object.entries(hostile).map(([name, row]) => [name, row.report.outcome])),
    },
    runs: { prepared, source, package: fromPackage, prepared_v1: preparedV1, typed, controls, hostile, overwrite },
    source_files: sourceFiles,
    non_claims: [
      'WIT strings are transport only; every frame and nominal is host-typed against WorkerData@1.',
      'The transition guest is synchronous; native work ownership and permitted suspension remain MW2.',
      'Wasmtime fuel and epoch are backstops; the measured interval is the source-runtime charge interval.',
      'This gate closes MW1 entry gates; it promotes no Worker case and is not proof acceptance.'
    ]
  };
  fs.writeFileSync(path.join(out, 'acceptance.json'), `${JSON.stringify(receipt, null, 2)}\n`);
  console.log(JSON.stringify({ result: receipt.result, failures, observations: receipt.observations,
    source_revision: receipt.source_revision, binary_sha256: receipt.binary.sha256 }));
  process.exitCode = failures.length ? 1 : 0;
}
main();
