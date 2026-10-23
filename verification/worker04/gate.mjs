#!/usr/bin/env node
// WORKER-04 acceptance gate.
//
// Builds the production `noble` binary from a clean checkout. The test
// embedder checks preselected Programs with a runtime-attested Text capture,
// an exact dependency and a scoped self owner, and reports an experimental
// local program-value identity (P-ID-04) projected from the checked package
// fields: recipe graph, interface witness, captures with their attested
// runtime binding, exact dependencies and recursive owners. Each variant
// compares identities against its own baseline:
//
// - reflect-prepare: `test-host reflect-prepare` reflects every installed
//   cell through the runtime's observation ABI, requires the decoded recipe
//   to equal the checked recipe, renders it with synthesized names and
//   prepares it again in a fresh session.
// - export-import: `test-host export` builds the grant for its own
//   destination; separate `noble worker program import` processes admit the
//   package at two request budgets.
// - rebound-display-name-original-dependency-retained: `test-host rebound`
//   admits a root that calls `dep.dec` through `import Dep@1 as dep`,
//   rebinds the alias to Dep@2 in the same retained session and admits the
//   root again; a control registered after the rebind resolves to Dep@2.
// - different-optimizer-same-semantics: `test-host optimizer` installs two
//   independent checks under Cranelift opt levels none and speed.
//
// Usage (inside the pinned devShell):
//   node verification/worker04/gate.mjs --out NEW_DIRECTORY
import assert from 'node:assert/strict';
import { execFileSync, spawnSync } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = fs.realpathSync(path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..'));
const here = path.join(root, 'verification/worker04');
const SCOPE = ['crates', 'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml',
  'verification/worker04/gate.mjs', 'verification/worker04/fixtures'];
const CASE_FILE = 'specs/conformance/worker-cases.json';
const FIXTURE = 'recursive.noble';
const REBOUND_FIXTURE = 'rebound-root.noble';
const REDUCED_WORK = '25000000';
const COMPONENTS = ['normalized_recipe', 'interface_witnesses', 'captures', 'dependency_identities',
  'recursive_owner', 'abstract_program_value_identity'];
// Display names of the original fixture; reflection carries none of them.
const FIXTURE_NAMES = ['Worker', 'count', 'dec'];
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const hash = file => sha(fs.readFileSync(file));
const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);
const VARIANTS = ['reflect-prepare', 'export-import', 'rebound-display-name-original-dependency-retained',
  'different-optimizer-same-semantics'];

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
  const selected = cases.filter(c => c.id === 'WORKER-04');
  assert.equal(selected.length, 1, 'expected exactly one WORKER-04 case');
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

// One process in a fresh, initially empty working directory. `files` are
// written there first; the directory must hold nothing else afterwards.
function run(noble, name, argv, out, { stdin, files = {} } = {}) {
  const workdir = fs.mkdtempSync(path.join(os.tmpdir(), 'noble-worker04-'));
  for (const [file, bytes] of Object.entries(files)) fs.writeFileSync(path.join(workdir, file), bytes);
  const child = spawnSync(noble, argv, {
    cwd: workdir, input: stdin, encoding: 'buffer', timeout: 300_000, maxBuffer: 16 * 1024 * 1024,
  });
  assert.equal(child.error, undefined, `${name}: ${child.error}`);
  const leftovers = fs.readdirSync(workdir).filter(file => !(file in files));
  fs.rmSync(workdir, { recursive: true, force: true });
  fs.writeFileSync(path.join(out, 'raw', `${name}.stdout.json`), child.stdout);
  fs.writeFileSync(path.join(out, 'raw', `${name}.stderr`), child.stderr);
  return {
    name,
    argv: ['noble', ...argv],
    exit: child.status,
    stdout_sha256: sha(child.stdout),
    stderr_sha256: sha(child.stderr),
    files_left_in_working_directory: leftovers.length,
    report: JSON.parse(child.stdout.toString('utf8')),
  };
}

function projection(identity) {
  return Object.fromEntries(COMPONENTS.map(field => [field, identity?.[field]]));
}

// 'same' per component when every result equals the variant's baseline.
function compare(baseline, results) {
  const base = projection(baseline);
  const values = {};
  for (const field of COMPONENTS) {
    values[field] = base[field] !== undefined && results.length > 0
      && results.every(identity => same(projection(identity)[field], base[field])) ? 'same' : 'differs';
  }
  values.outcome = COMPONENTS.every(field => values[field] === 'same') ? 'preserve-each-variant' : 'identity-changed';
  return values;
}

function main() {
  const destination = argument('--out');
  assert.ok(destination, 'usage: node verification/worker04/gate.mjs --out NEW_DIRECTORY');
  const out = path.resolve(destination);
  assert.ok(!fs.existsSync(out), `output directory already exists: ${out}`);

  const sourceFiles = sourceSnapshot();
  const caseRecord = caseProjection();
  const { input, expected } = caseRecord;
  assert.deepEqual([...input.variants].sort(), [...VARIANTS].sort(), 'gate variants differ from the case');
  const build = buildNoble();
  fs.mkdirSync(path.join(out, 'raw'), { recursive: true });
  const noble = build.executable;
  const source = fs.readFileSync(path.join(here, 'fixtures', FIXTURE));
  const reboundSource = fs.readFileSync(path.join(here, 'fixtures', REBOUND_FIXTURE));
  const host = mode => ['worker', 'program', 'test-host', mode];

  const exported = run(noble, 'export', host('export'), out, { stdin: source });
  const packageBytes = Buffer.from(exported.report.package_utf8 ?? '', 'utf8');
  fs.writeFileSync(path.join(out, 'raw', 'package.json'), packageBytes);
  const importArgv = extra => ['worker', 'program', 'import', 'package.json', '--effect-ceiling', 'pure', ...extra];
  const imported = run(noble, 'import', importArgv([]), out, { files: { 'package.json': packageBytes } });
  const reduced = run(noble, 'import-reduced-work', importArgv(['--work', REDUCED_WORK]), out,
    { files: { 'package.json': packageBytes } });
  const optimizer = run(noble, 'optimizer', host('optimizer'), out, { stdin: source });
  const denied = run(noble, 'export-denied', host('export-denied'), out, { stdin: source });
  const reflected = run(noble, 'reflect-prepare', host('reflect-prepare'), out, { stdin: source });
  const rebound = run(noble, 'rebound', host('rebound'), out, { stdin: reboundSource });

  // Packages derived from the exported one; each must be refused.
  const document = JSON.parse(packageBytes.toString('utf8') || 'null');
  const derive = edit => {
    const copy = structuredClone(document);
    edit(copy);
    return Buffer.from(JSON.stringify(copy));
  };
  const controls = Object.fromEntries(Object.entries({
    'accepted-flag': copy => { copy.accepted = true; },
    'proof-evidence': copy => { copy.evidence = [{ kind: 'lean-kernel', result: 'accepted' }]; },
    'changed-capture': copy => {
      copy.source_utf8 = copy.source_utf8.replace('"tag"', '"tah"');
      copy.captures = copy.captures.map(capture => ({ ...capture, text_utf8: 'tah' }));
    },
    'changed-builtin-revision': copy => { copy.builtins[0].semantic_revision = '0.1.0-draft.6'; },
  }).map(([name, edit]) => [name, run(noble, `control-${name}`, importArgv([]), out,
    { files: { 'package.json': document ? derive(edit) : Buffer.alloc(0) } })]));

  const failures = [];
  if (!(exported.exit === 0 && exported.report.outcome === 'normal' && exported.report.package_bytes_emitted === packageBytes.length
    && packageBytes.length > 0)) failures.push('export: the host grant did not export the package');
  for (const row of [imported, reduced]) {
    if (!(row.exit === 0 && row.report.outcome === 'ready')) failures.push(`${row.name}: the package was not admitted`);
  }
  if (!(denied.exit === 2 && denied.report.outcome === 'denied' && denied.report.package_bytes_emitted === 0
    && denied.report.package_utf8 === undefined)) failures.push('export-denied: a refusing grant emitted package bytes');
  for (const [name, row] of Object.entries(controls)) {
    if (row.report.outcome === 'ready' || row.report.source_installations !== 0) {
      failures.push(`control-${name}: the altered package was admitted or installed`);
    }
  }
  const all = [exported, imported, reduced, optimizer, denied, reflected, rebound, ...Object.values(controls)];
  for (const row of all) {
    if (row.files_left_in_working_directory !== 0) failures.push(`${row.name}: left files in the working directory`);
  }
  const artifacts = optimizer.report.artifacts ?? [];
  if (!(optimizer.exit === 0 && artifacts.length === 2
    && same(artifacts.map(row => row.cranelift_opt_level), ['none', 'speed'])
    && artifacts[0].instance !== artifacts[1].instance)) failures.push('optimizer: two independent stores were not reported');

  // reflect-prepare: the prepared recipe came from reflection, not the source.
  const reflection = reflected.report.reflection ?? {};
  const rendered = reflection.rendered_source_utf8 ?? '';
  if (!(reflected.exit === 0 && reflected.report.outcome === 'normal'
    && (reflection.events_per_definition ?? []).length === (exported.report.identity?.normalized_recipe?.definitions ?? []).length
    && reflection.events_per_definition.every(count => count > 0)
    && reflected.report.original?.source_sha256 === sha(source)
    && reflected.report.prepared?.source_sha256 === sha(Buffer.from(rendered, 'utf8'))
    && reflected.report.prepared.source_sha256 !== reflected.report.original.source_sha256
    && FIXTURE_NAMES.every(name => !new RegExp(`\\b${name}\\b`).test(rendered))
    && same(reflected.report.original.identity, exported.report.identity))) {
    failures.push('reflect-prepare: the prepared recipe was not rendered from a runtime reflection of the admitted Program');
  }

  // rebound: the alias really moved, and the same display name now reaches
  // a different dependency, while the original root keeps its own.
  const rebind = rebound.report;
  const owners = identity => (identity?.dependency_identities?.definitions ?? []).map(row => row.module_id);
  const reboundChecks = rebind.modules && rebind.alias_bound && rebind.alias_rebound
    && rebound.exit === 0 && rebind.outcome === 'normal'
    && rebind.alias_bound.alias === 'dep' && rebind.alias_rebound.alias === 'dep'
    && rebind.alias_bound.module_identity === rebind.modules.dependency_a
    && rebind.alias_rebound.module_identity === rebind.modules.dependency_b
    && rebind.alias_bound.rebuilt_definitions === null && rebind.alias_rebound.rebuilt_definitions === 0
    && rebind.alias_rebound.generation_after === rebind.alias_rebound.generation_before + 1
    && rebind.units.find(unit => unit.role === 'root')?.source_sha256 === sha(reboundSource)
    && owners(rebind.before.identity).includes(String(rebind.modules.dependency_a))
    && !owners(rebind.before.identity).includes(String(rebind.modules.dependency_b))
    && owners(rebind.control.identity).includes(String(rebind.modules.dependency_b))
    && !same(rebind.control.identity.dependency_identities, rebind.before.identity.dependency_identities)
    && rebind.control.identity.abstract_program_value_identity !== rebind.before.identity.abstract_program_value_identity
    && rebind.substituted_dependency === 'invalid-input';
  if (!reboundChecks) failures.push('rebound: the alias was not rebound, the control did not reach the rebound dependency, or a substituted dependency was admitted');

  const bodyRequests = [exported, imported, reduced].map(row => row.report.candidate_body_requests)
    .concat(artifacts.map(row => row.candidate_body_requests))
    .concat([reflection.candidate_body_requests, reflected.report.prepared?.candidate_body_requests])
    .concat(['before', 'after', 'control'].map(row => rebind[row]?.candidate_body_requests));
  const exportImport = {
    stage: exported.report.stage === 'export-authorization'
      && [imported, reduced].every(row => row.report.stage === 'import-admission')
      ? 'identity-after-local-admission' : 'mixed',
    ...compare(exported.report.identity, [imported.report.identity, reduced.report.identity]),
  };
  const byVariant = {
    'reflect-prepare': { stage: reflected.report.stage,
      ...compare(reflected.report.original?.identity, [reflected.report.prepared?.identity]) },
    'export-import': exportImport,
    'rebound-display-name-original-dependency-retained': { stage: rebind.stage,
      ...compare(rebind.before?.identity, [rebind.after?.identity]) },
    'different-optimizer-same-semantics': { stage: optimizer.report.stage,
      ...compare(exported.report.identity, artifacts.map(row => row.identity)) },
  };
  // Fields of the identity relation, observed once across the runs.
  const shared = {
    artifact_bytes_required_identical: !(artifacts.length === 2
      && artifacts[0].native_artifact_sha256 !== artifacts[1].native_artifact_sha256
      && byVariant['different-optimizer-same-semantics'].abstract_program_value_identity === 'same'),
    budget_outcomes_required_identical: !(imported.report.request_work_left !== reduced.report.request_work_left
      && exportImport.abstract_program_value_identity === 'same'),
    unchecked_acceptance_transferred: controls['accepted-flag'].report.outcome === 'ready',
    proof_applicability_transferred: controls['proof-evidence'].report.outcome === 'ready',
    live_authority_transferred: !(imported.report.protected_operations === 0
      && imported.report.executable_guest_installed === false && imported.report.guest_requests === 0),
    stable_wire_encoding_claimed: !(String(exported.report.identity?.label).includes('experimental')
      && String(document?.semantic_revision).includes('draft')),
    candidate_body_requests: bodyRequests.every(Number.isInteger) ? bodyRequests.reduce((a, b) => a + b, 0) : 'unmeasured',
  };
  const observed = Object.fromEntries(VARIANTS.map(variant => [variant, { ...byVariant[variant], ...shared }]));
  for (const [variant, values] of Object.entries(observed)) {
    for (const [field, value] of Object.entries(expected)) {
      if (!same(values[field], value)) {
        failures.push(`${variant}: ${field}: expected ${JSON.stringify(value)}, observed ${JSON.stringify(values[field])}`);
      }
    }
  }

  const receipt = {
    schema: 'noble-worker04-acceptance/v1',
    case: 'WORKER-04',
    result: failures.length === 0 ? 'passed' : 'failed',
    failures,
    tested_commit: git('rev-parse', 'HEAD').trim(),
    source_scope: SCOPE,
    source_revision: `sha256:${sha(JSON.stringify(sourceFiles))}`,
    source_file_count: Object.keys(sourceFiles).length,
    case_projection_sha256: sha(JSON.stringify(caseRecord)),
    case_projection: caseRecord,
    binary: { name: 'noble', profile: build.profile, sha256: hash(build.executable),
      build: 'cargo build --locked -p noble-cli --bin noble' },
    fixtures: Object.fromEntries([FIXTURE, REBOUND_FIXTURE].map(name => [name, hash(path.join(here, 'fixtures', name))])),
    package_sha256: sha(packageBytes),
    stage_mapping: 'identity-after-local-admission is compared after test-host export (export-authorization), each import (import-admission), each optimizer row, the reflect-prepare original and prepared Programs, and each rebound admission (identity-after-local-admission).',
    input_mapping: {
      expected_interface: 'The host checks each root at I64 -- I64 ! {}; reflect-prepare prepares the rendered root at the original root input.',
      context: 'Export-import, optimizer and reflect-prepare commit one module in a fresh session, so identities resolve in the same order; the rebound variant admits twice from one retained session.',
      captured_data: 'The "tag" Text literal, bound to its installed runtime cell by attestation.',
      self_reference: 'count calls itself as a scoped self occurrence under its own retained owner; reflect-prepare renders the retained declared scheme as its signature.',
      preparation_authority: 'Admission and reflect-prepare are explicit host preparation; no guest prepares code.',
      export_authority: 'Host code builds the grant for its own destination from the checked source and capture digests; a refusing grant emits nothing.',
      limits: `The full request budget and --work ${REDUCED_WORK} both suffice.`,
      'reflect-prepare': 'The runtime observe/reflect ABI over each installed closure cell; the decoded recipe must equal the checked recipe; the module is rendered with synthesized names (Reflected@1, r0, r1).',
      'rebound-display-name-original-dependency-retained': 'Retained session: Dep@1, Dep@2, import Dep@1 as dep, the root (calls dep.dec); admit; import Dep@2 as dep; admit again. A control Worker@2 registered after the rebind, and a substituted Dep@2-only selection for the root.',
      'different-optimizer-same-semantics': 'Cranelift opt level none and speed over the same checked Wasm text.',
    },
    observed,
    observation_methods: {
      same_fields: 'Each identity component of a variant\'s results equals that variant\'s baseline: the exporter (export-import, optimizer), the reflected original (reflect-prepare) or the pre-rebind admission (rebound).',
      artifact_bytes_required_identical: 'The two optimizer rows have different native artifact digests and the same identity.',
      budget_outcomes_required_identical: 'The two imports leave different request work and report the same identity.',
      unchecked_acceptance_transferred: 'A package with an added accepted flag is refused.',
      proof_applicability_transferred: 'A package with an added proof evidence row is refused.',
      live_authority_transferred: 'Static report fields of a pure-ceiling import: no guest request, protected operation or executable installation; no issue route exists.',
      stable_wire_encoding_claimed: 'The identity is labelled experimental and the package revision is a draft.',
      candidate_body_requests: 'Body invocations counted by the host while installing cells for attestation and reflection, summed over every Program in every run.',
      reflect_prepare_provenance: 'The rendered module source differs from the fixture, names none of its display names, and its runtime events cover every closure definition.',
      rebound_non_vacuity: 'The alias moves from Dep@1 to Dep@2 with no replayed outside definition; the control registered after the rebind reaches Dep@2 and has a different identity; selecting only Dep@2 for the original root is refused.',
    },
    runs: { exported, imported, reduced, optimizer, denied, reflected, rebound, controls },
    source_files: sourceFiles,
    non_claims: [
      'The program-value identity is local and experimental (P-ID-04), not a stable or cross-implementation hash.',
      'reflect-prepare is a host preparation service; no guest reflects or prepares code.',
      'A Program reaching a dependency module is not exported; the package format carries one root source.',
      'Semantic preservation by the optimizer is Wasmtime\'s obligation; identity equality is not a semantic proof.',
      'Authority non-transfer is observed as the absence of any issue route, not as a refused issue.',
      'No proof obligation is discharged.',
    ],
  };
  fs.writeFileSync(path.join(out, 'acceptance.json'), `${JSON.stringify(receipt, null, 2)}\n`);
  console.log(JSON.stringify({ result: receipt.result, failures, observed,
    source_revision: receipt.source_revision, binary_sha256: receipt.binary.sha256 }));
  process.exitCode = failures.length === 0 ? 0 : 1;
}

main();
