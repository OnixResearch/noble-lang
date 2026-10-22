#!/usr/bin/env node
// WORKER-05 acceptance gate.
//
// Builds the production `noble` binary from a clean checkout and imports the
// canonical WORKER-05 hostile Worker Program v2 packages. Pure packages go
// through `noble worker program import`; the host-contract variant needs an
// independently selected binding, so it goes through the dedicated synthetic
// test embedder (`noble worker program test-host emit-import`), whose
// preselected `test.emit` binding is the only one that can match.
//
// Each variant mutates one exact, valid base package (committed under
// fixtures/, exported once from checked sources) in the single claim the
// variant names. Both base packages must import, so every refusal is
// attributable to its mutation. A refusal must install no source module,
// leave its fresh working directory empty and run no guest.
//
// Usage (inside the pinned devShell):
//   node verification/worker05/gate.mjs --out NEW_DIRECTORY
import assert from 'node:assert/strict';
import { execFileSync, spawnSync } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = fs.realpathSync(path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..'));
const here = path.join(root, 'verification/worker05');
const SCOPE = ['crates', 'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml',
  'verification/worker05/gate.mjs', 'verification/worker05/fixtures'];
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
  const selected = cases.filter(c => c.id === 'WORKER-05');
  assert.equal(selected.length, 1, 'expected exactly one WORKER-05 case');
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

// Worker Program v2 packages must be canonical serde_json bytes: sorted keys,
// no whitespace. Any other spelling would be refused before the claim under
// test is examined.
function canonical(value) {
  if (Array.isArray(value)) return value.map(canonical);
  if (value === null || typeof value !== 'object') return value;
  return Object.fromEntries(Object.keys(value).sort().map(key => [key, canonical(value[key])]));
}
const encode = value => Buffer.from(JSON.stringify(canonical(value)));

function load(name) {
  const bytes = fs.readFileSync(path.join(here, 'fixtures', `${name}.json`));
  assert.ok(encode(JSON.parse(bytes.toString('utf8'))).equals(bytes), `${name}.json is not canonical`);
  return JSON.parse(bytes.toString('utf8'));
}

// One mutation per canonical variant, each changing only the named claim.
const VARIANTS = {
  'missing-exact-transitive-dependency': { base: 'text', route: 'cli',
    mutate: p => { p.definitions.splice(0, 1); } },
  'same-name-wrong-host-contract-version': { base: 'emit', route: 'host',
    mutate: p => { p.host_contracts[0].adapter_identity = 'worker-test-emit@1.0.1'; } },
  'capture-schema-witness-mismatch': { base: 'text', route: 'cli',
    mutate: p => { p.captures[0].schema = 'Bytes'; } },
  'foreign-scoped-self-owner': { base: 'recursive', route: 'cli',
    mutate: p => {
      const recursive = p.definitions.findIndex(row => row.self_owner !== null);
      const foreign = p.definitions.findIndex(row => row.self_owner === null);
      assert.ok(recursive >= 0 && foreign >= 0, 'recursive base lost its scoped self owner');
      p.definitions[recursive].self_owner = p.definitions[foreign].owner;
    } },
  'invalid-body-with-accepted-flag-and-digest': { base: 'text', route: 'cli',
    mutate: p => {
      // The digest of the valid package is kept; the body no longer checks.
      assert.ok(p.source_utf8.includes('1 - ]'), 'text base changed shape');
      p.source_utf8 = p.source_utf8.replace('1 - ]', '1 - - ]');
      p.accepted = true;
    } },
  'unrelated-executable-artifact': { base: 'text', route: 'cli',
    mutate: p => { p.artifact = { wasm: 'AGFzbQEAAAA=', accepted: true }; } },
  'unsupported-format-revision': { base: 'text', route: 'cli',
    mutate: p => { p.semantic_revision = '0.1.0-draft.99'; } },
};

const OUTCOME = { reject: 'invalid-input', unsupported: 'unsupported' };
// The CLI's documented exit codes for those report outcomes.
const EXIT = { 'invalid-input': 2, unsupported: 4 };

function run(noble, name, route, bytes, out) {
  const workdir = fs.mkdtempSync(path.join(os.tmpdir(), 'noble-worker05-'));
  const packagePath = path.join(out, 'packages', `${name}.json`);
  fs.writeFileSync(packagePath, bytes);
  const argv = route === 'cli'
    ? ['worker', 'program', 'import', packagePath, '--effect-ceiling', 'pure']
    : ['worker', 'program', 'test-host', 'emit-import'];
  const child = spawnSync(noble, argv, {
    cwd: workdir, input: route === 'cli' ? undefined : bytes, encoding: 'buffer',
    timeout: 300_000, maxBuffer: 16 * 1024 * 1024,
  });
  assert.equal(child.error, undefined, `${name}: ${child.error}`);
  const leftovers = fs.readdirSync(workdir).length;
  fs.rmSync(workdir, { recursive: true, force: true });
  fs.writeFileSync(path.join(out, 'raw', `${name}.stdout.json`), child.stdout);
  fs.writeFileSync(path.join(out, 'raw', `${name}.stderr`), child.stderr);
  const report = JSON.parse(child.stdout.toString('utf8'));
  return {
    name, route,
    argv: ['noble', ...argv.map(a => a.startsWith(out) ? path.relative(out, a) : a)],
    exit: child.status,
    package_sha256: sha(bytes),
    stdout_sha256: sha(child.stdout),
    stderr_sha256: sha(child.stderr),
    files_left_in_working_directory: leftovers,
    observed: {
      outcome: report.outcome,
      source_installations: report.source_installations,
      guest_requests: report.guest_requests,
      protected_operations: report.protected_operations,
      executable_guest_installed: report.executable_guest_installed,
    },
  };
}

function main() {
  const destination = argument('--out');
  assert.ok(destination, 'usage: node verification/worker05/gate.mjs --out NEW_DIRECTORY');
  const out = path.resolve(destination);
  assert.ok(!fs.existsSync(out), `output directory already exists: ${out}`);

  const sourceFiles = sourceSnapshot();
  const projection = caseProjection();
  assert.deepEqual(projection.input.variants.map(v => v.case).sort(), Object.keys(VARIANTS).sort(),
    'gate mutations differ from the canonical WORKER-05 variant set');
  assert.equal(projection.input.dependency_fetch_authority, 'absent');
  const build = buildNoble();
  fs.mkdirSync(path.join(out, 'packages'), { recursive: true });
  fs.mkdirSync(path.join(out, 'raw'));

  const bases = { text: load('text'), recursive: load('recursive'), emit: load('emit') };
  const rows = [];
  const failures = [];
  for (const [name, base, route] of [['baseline-text', 'text', 'cli'],
    ['baseline-recursive', 'recursive', 'cli'], ['baseline-emit', 'emit', 'host']]) {
    const row = run(build.executable, name, route, encode(bases[base]), out);
    rows.push({ ...row, expected: { exit: 0, outcome: 'ready' } });
    if (!(row.exit === 0 && row.observed.outcome === 'ready' && row.observed.guest_requests === 0
      && row.observed.protected_operations === 0 && row.observed.executable_guest_installed === false)) {
      failures.push(`${name}: the unmodified base package was not admitted`);
    }
  }
  for (const variant of projection.input.variants) {
    const { base, route, mutate } = VARIANTS[variant.case];
    const mutated = structuredClone(bases[base]);
    mutate(mutated);
    const row = run(build.executable, variant.case, route, encode(mutated), out);
    const outcome = OUTCOME[variant.outcome];
    rows.push({ ...row, expected: { exit: EXIT[outcome], outcome, source_installations: 0 } });
    const o = row.observed;
    if (!(row.exit === EXIT[outcome] && o.outcome === outcome && o.source_installations === 0 && o.guest_requests === 0
      && o.protected_operations === 0 && o.executable_guest_installed === false
      && row.files_left_in_working_directory === 0)) {
      failures.push(`${variant.case}: expected ${outcome} with no installation or publication`);
    }
  }
  const variantRows = rows.filter(row => !row.name.startsWith('baseline-'));
  const byName = Object.fromEntries(rows.map(row => [row.name, row]));
  const refused = name => byName[name]?.observed.outcome !== 'ready';
  const observed = {
    stage: 'import-admission',
    outcome: variantRows.every(row => row.observed.outcome !== 'ready') ? 'no-variant-admitted' : 'variant-admitted',
    guest_requests: variantRows.reduce((sum, row) => sum + (row.observed.guest_requests ?? 1), 0),
    protected_operations: variantRows.reduce((sum, row) => sum + (row.observed.protected_operations ?? 1), 0),
    implicit_fetches: refused('missing-exact-transitive-dependency') ? 0 : 1,
    version_substitutions: refused('same-name-wrong-host-contract-version') ? 0 : 1,
    partial_programs_published: variantRows.filter(row => row.observed.source_installations !== 0
      || row.files_left_in_working_directory !== 0).length,
    signature_or_digest_bypasses_checks: !refused('invalid-body-with-accepted-flag-and-digest'),
  };
  for (const [field, value] of Object.entries(projection.expected)) {
    if (observed[field] !== value) failures.push(`${field}: expected ${JSON.stringify(value)}, observed ${JSON.stringify(observed[field])}`);
  }

  const receipt = {
    schema: 'noble-worker05-acceptance/v1',
    case: 'WORKER-05',
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
    observed,
    observation_methods: {
      implicit_fetches: 'The package missing a transitive definition is refused; nothing outside the package supplied it.',
      version_substitutions: 'The package naming a different host-contract version is refused under the selected binding instead of being run against it.',
      partial_programs_published: 'Variants that installed any source module or left a file in their fresh working directory.',
      signature_or_digest_bypasses_checks: 'The invalid body keeping the valid package digest and an accepted flag is refused.',
    },
    runs: rows,
    source_files: sourceFiles,
    non_claims: [
      'The base packages were exported once by the in-crate test exporter; no CLI export route exists.',
      'The importer has no fetch facility; implicit_fetches records that a missing dependency was refused, not a network observation.',
      'Recursive packages are admitted only for pure Programs; host-bound recursion stays refused.',
      'No proof obligation is discharged.',
    ],
  };
  fs.writeFileSync(path.join(out, 'acceptance.json'), `${JSON.stringify(receipt, null, 2)}\n`);
  console.log(JSON.stringify({ result: receipt.result, failures, observed,
    source_revision: receipt.source_revision, binary_sha256: receipt.binary.sha256 }));
  process.exitCode = failures.length === 0 ? 0 : 1;
}

main();
