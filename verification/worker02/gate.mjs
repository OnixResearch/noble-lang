#!/usr/bin/env node
// WORKER-02 acceptance gate.
//
// Builds the production `noble` binary from a clean checkout, then runs every
// interface-rejection variant of the canonical WORKER-02 design through
// `noble worker typed`. The six variants are applied to one consumer-selected
// local package by the mutations below, which are keyed by the case's own
// variant names; two further controls give the checked Program itself a
// reversed or extra-output stack. The receipt binds the source snapshot, the
// built binary, the exact case projection and every raw observation.
//
// Usage (inside the pinned devShell):
//   node verification/worker02/gate.mjs --out NEW_DIRECTORY
//
// Not a proof and not WORKER-03: Worker v1 admits no captures, effects or
// adapters, so only refusals are exercised here.
import assert from 'node:assert/strict';
import { execFileSync, spawnSync } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = fs.realpathSync(path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..'));
const here = path.join(root, 'verification/worker02');
const SCOPE = ['crates', 'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml',
  'verification/worker02/gate.mjs', 'verification/worker02/fixtures'];
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

// The build scope must equal the committed tree: no staged, unstaged or
// untracked file may contribute bytes that the receipt does not name.
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
  const selected = cases.filter(c => c.id === 'WORKER-02');
  assert.equal(selected.length, 1, 'expected exactly one WORKER-02 case');
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

// One mutation per canonical variant name. Each starts from the unmodified
// package and changes only the claim the variant names.
const MUTATIONS = {
  'reversed-state-event-stack': p => {
    const [state, event] = p.interface.inputs;
    p.interface.inputs = [event, state];
  },
  'extra-output-value': p => {
    const [state, actions] = p.interface.outputs;
    p.interface.outputs = [state, actions, state];
  },
  'same-layout-different-schema-identity': p => {
    const state = p.interface.inputs[0];
    p.interface.inputs[0] = { ...state, module_id: (BigInt(state.module_id) + 1n).toString() };
  },
  'effect-outside-dispatch-ceiling': p => {
    p.interface.effects = ['test.emit'];
  },
  'resource-hidden-in-worker-capture': p => {
    p.captures = [{ tag: 'opaque', schema: p.interface.inputs[0], value: { tag: 'pair',
      left: { tag: 'i64', value: '0' }, right: { tag: 'resource', value: 'hidden' } } }];
  },
  'heterogeneous-list-without-adapter': p => {
    p.interface.outputs[1] = { list: [p.interface.outputs[1].list, p.interface.inputs[1]] };
  },
};

const PROGRAM_CONTROLS = ['program-reversed-state-event-stack', 'program-extra-output-value'];

// Worker v1 packages must be canonical serde_json bytes (sorted object keys,
// no whitespace); any other spelling is refused as malformed before the
// interface checker runs, which would mask the variant under test.
function canonical(value) {
  if (Array.isArray(value)) return value.map(canonical);
  if (value === null || typeof value !== 'object') return value;
  return Object.fromEntries(Object.keys(value).sort().map(key => [key, canonical(value[key])]));
}

const encode = value => Buffer.from(JSON.stringify(canonical(value)));

function run(noble, name, packageBytes, out) {
  const packagePath = path.join(out, 'packages', `${name}.json`);
  fs.writeFileSync(packagePath, packageBytes);
  const fixtures = path.join(here, 'fixtures');
  const argv = ['worker', 'typed', packagePath, '--schema', path.join(fixtures, 'schema.noble'),
    '--state', path.join(fixtures, 'state.json'), '--event', path.join(fixtures, 'event.json')];
  const child = spawnSync(noble, argv, { encoding: 'buffer', timeout: 120_000, maxBuffer: 16 * 1024 * 1024 });
  assert.equal(child.error, undefined, `${name}: ${child.error}`);
  fs.writeFileSync(path.join(out, 'raw', `${name}.stdout.json`), child.stdout);
  fs.writeFileSync(path.join(out, 'raw', `${name}.stderr`), child.stderr);
  const report = JSON.parse(child.stdout.toString('utf8'));
  return {
    name,
    argv: ['noble', ...argv.map(a => a.startsWith(out) ? path.relative(out, a)
      : a.startsWith(root) ? path.relative(root, a) : a)],
    exit: child.status,
    package_sha256: sha(packageBytes),
    stdout_sha256: sha(child.stdout),
    stderr_sha256: sha(child.stderr),
    observed: {
      stage: report.stage, outcome: report.outcome, guest_requests: report.guest_requests,
      protected_operations: report.protected_operations, typed_output: report.typed_output ?? null,
      ownership_disposition: report.ownership_disposition, invocation: report.invocation ?? null,
      module_compiled: report.module !== null && report.module !== undefined,
      diagnostic: report.diagnostic ?? null,
    },
  };
}

function rejection(row, outcome) {
  const o = row.observed;
  return row.exit === 2 && o.stage === 'admission' && o.outcome === outcome && o.guest_requests === 0
    && o.protected_operations === 0 && o.typed_output === null && o.invocation === null
    && o.module_compiled === false && o.ownership_disposition === 'not-admitted';
}

function main() {
  const destination = argument('--out');
  assert.ok(destination, 'usage: node verification/worker02/gate.mjs --out NEW_DIRECTORY');
  const out = path.resolve(destination);
  assert.ok(!fs.existsSync(out), `output directory already exists: ${out}`);

  const sourceFiles = sourceSnapshot();
  const projection = caseProjection();
  const variants = projection.input.variants.map(v => v.case);
  assert.deepEqual([...variants].sort(), Object.keys(MUTATIONS).sort(),
    'gate mutations differ from the canonical WORKER-02 variant set');
  assert.equal(projection.expected.guest_requests, 0);
  assert.equal(projection.expected.protected_operations, 0);

  const build = buildNoble();
  fs.mkdirSync(path.join(out, 'packages'), { recursive: true });
  fs.mkdirSync(path.join(out, 'raw'));

  const fixtureDir = path.join(here, 'fixtures');
  const base = fs.readFileSync(path.join(fixtureDir, 'package.json'));
  assert.ok(encode(JSON.parse(base.toString('utf8'))).equals(base),
    'canonical encoder does not reproduce the base package bytes');
  const rows = [];
  const failures = [];
  const baseline = run(build.executable, 'baseline', base, out);
  rows.push({ ...baseline, expected: { exit: 0, outcome: 'normal' } });
  if (!(baseline.exit === 0 && baseline.observed.outcome === 'normal' && baseline.observed.guest_requests === 0
    && baseline.observed.protected_operations === 0)) failures.push('baseline was not admitted and run normally');

  for (const variant of projection.input.variants) {
    const mutated = JSON.parse(base.toString('utf8'));
    MUTATIONS[variant.case](mutated);
    const row = run(build.executable, variant.case, encode(mutated), out);
    rows.push({ ...row, expected: { exit: 2, stage: 'admission', outcome: variant.outcome } });
    if (!rejection(row, variant.outcome)) failures.push(`${variant.case}: expected ${variant.outcome} at admission`);
  }
  for (const control of PROGRAM_CONTROLS) {
    const row = run(build.executable, control, fs.readFileSync(path.join(fixtureDir, `${control}.json`)), out);
    rows.push({ ...row, expected: { exit: 2, stage: 'admission', outcome: 'interface-reject' } });
    if (!rejection(row, 'interface-reject')) failures.push(`${control}: expected interface-reject at admission`);
  }

  const byName = Object.fromEntries(rows.map(r => [r.name, r]));
  const receipt = {
    schema: 'noble-worker02-acceptance/v1',
    case: 'WORKER-02',
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
    fixtures: Object.fromEntries(fs.readdirSync(fixtureDir).sort()
      .map(name => [name, hash(path.join(fixtureDir, name))])),
    stage_mapping: 'The case stage `static` is observed as the CLI `admission` stage: refusal before compilation, guest start or protected operation (module null, invocation null, ownership not-admitted).',
    derived_observations: {
      dynamic_Any_fallback: byName['heterogeneous-list-without-adapter']?.observed.outcome === 'interface-reject' ? false : null,
      implicit_schema_conversion: byName['same-layout-different-schema-identity']?.observed.outcome === 'schema-reject' ? false : null,
    },
    runs: rows,
    source_files: sourceFiles,
    non_claims: [
      'Worker v1 admits no captures, effects or adapters; positive effect widening and adapter admission (WORKER-03) are not exercised.',
      'The two derived observations record refusals of the heterogeneous-list and same-layout variants, not a general absence proof.',
      'The v2 worker program package boundary is not exercised.',
      'No proof obligation is discharged.',
    ],
  };
  fs.writeFileSync(path.join(out, 'acceptance.json'), `${JSON.stringify(receipt, null, 2)}\n`);
  console.log(JSON.stringify({ result: receipt.result, failures, runs: rows.length,
    source_revision: receipt.source_revision, binary_sha256: receipt.binary.sha256 }));
  process.exitCode = failures.length === 0 ? 0 : 1;
}

main();
