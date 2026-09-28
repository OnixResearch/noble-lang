#!/usr/bin/env node
// Distinct post-promotion gate. The accepted compiler/proof source stays frozen;
// the promoted documents and native Cairn archive get their own staged-source Nix checks.
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { gitSourceProjection } from '../m8/source-projection.mjs';
import { view } from '../../tools/cairn-specs.mjs';

const root = fs.realpathSync(path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..'));
const [mode, assurancePath, destination, ...extra] = process.argv.slice(2);
assert.ok(['plan', 'run'].includes(mode) && assurancePath && destination && !extra.length,
  'usage: SELECTED_NODE verification/declared-modules-v1/final-documents.mjs {plan|run} PRE_PROMOTION_ASSURANCE_JSON NEW_EXTERNAL_DIRECTORY');
const out = path.resolve(destination), localAssurance = path.resolve(assurancePath);
assert.ok(out !== root && !out.startsWith(`${root}${path.sep}`), 'external artifacts required');
const selected = JSON.parse(fs.readFileSync(path.join(root, 'policy/tool-selection.json')));
const node = path.join(selected.tool_paths.node.output, 'bin/node');
assert.equal(fs.realpathSync(process.execPath), fs.realpathSync(node), 'policy-selected Node required');
assert.deepEqual(process.execArgv, []);
// The historical M8 full-flake pass used this reviewed, user-owned client
// configuration inside a private systemd user mount, not an isolated store.
// Pin its bytes and propagate it to each Nix child rather than trusting
// inherited shell variables stripped by the minimal execution environment.
const nixConfiguration = '/var/tmp/noble-m8-refrozen-20260924/private-nix-no-gc-config-20260926';
const nixConfigurationSha256 = '226289cf0b807ab6e17058b1a6f3410d9560643d6e2e27edb9b9a5c1a1215013';
const generated = 'verification/declared-modules-v1/nix-checks.json';
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const hash = file => sha(fs.readFileSync(file));
const load = file => JSON.parse(fs.readFileSync(file));
const file = name => path.join(root, name);
assert.equal(hash(nixConfiguration), nixConfigurationSha256,
  'reviewed private Nix client configuration changed');
const git = '/run/current-system/sw/bin/git';
function gitCall(args) {
  const result = spawnSync(git, ['-C', root, ...args], {
    encoding: 'utf8', maxBuffer: 32 * 1024 * 1024, timeout: 30_000,
    env: { ...process.env, GIT_OPTIONAL_LOCKS: '0', GIT_CONFIG_NOSYSTEM: '1' },
  });
  assert.equal(result.error, undefined, `Git ${args[0]} launch failed`);
  assert.equal(result.signal, null, `Git ${args[0]} terminated`);
  assert.equal(result.status, 0, `Git ${args[0]}: ${result.stderr}`);
  return result.stdout;
}
const tracked = () => gitCall(['ls-files', '--cached', '-z']).split('\0').filter(Boolean).sort();
let base;
try { base = load(localAssurance); } catch (error) {
  if (mode !== 'plan') throw error;
  console.log(JSON.stringify({ schema: 'noble-declared-modules-final-documents-plan/v1',
    planned_result: 'blocked', prerequisites: [`Fresh pre-promotion assurance unavailable: ${error}`] }, null, 2));
  process.exit(0);
}
assert.equal(base.schema, 'noble-declared-modules-source-assurance/v1');
assert.equal(base.phase, 'pre-promotion');
assert.equal(base.result, 'passed', 'full fresh pre-promotion assurance required');
assert.equal(base.commands.length, 22, 'incomplete pre-promotion command matrix');
assert.equal(Object.keys(base.gates).length, 14, 'incomplete pre-promotion gates');
assert.equal(hash(file('verification/m4/extraction-lock.json')), base.proof_lock.sha256,
  'reviewed whole-crate extraction lock changed after acceptance');
for (const [label, gate] of Object.entries(base.gates)) assert.equal(
  hash(path.resolve(path.dirname(localAssurance), gate.path)), gate.sha256,
  `pre-promotion ${label} gate receipt changed`);
const acceptanceGate = base.gates['declared-modules-acceptance'];
assert.ok(acceptanceGate, 'missing fresh independent declared acceptance gate');
const buildGate = base.gates['fresh-current-workspace-m7-build'];
const extractionGate = base.gates['strict-whole-crate-extraction-check'];
assert.ok(buildGate && extractionGate, 'missing fresh production build or whole-crate extraction gate');
const acceptance = load(path.resolve(path.dirname(localAssurance), acceptanceGate.path));
assert.equal(acceptance.schema, 'noble-declared-modules-acceptance/v1');
assert.equal(acceptance.result, 'passed');
assert.equal(acceptance.summary?.case_rows, 39);
assert.equal(acceptance.summary?.planned_scenarios, 27);
assert.equal(acceptance.summary?.failure_count, 0);
assert.equal(acceptance.summary?.integrity_failures, 0);
const cases = [
  ['specs/conformance/developer-experience-cases.json', 'DX-03'],
  ['specs/conformance/language-workflow-cases.json', 'DX-08'],
  ['specs/conformance/language-workflow-cases.json', 'DX-09'],
];
const active = '.cairn/changes/2026-09-26-declared-modules-v1';
const pending = [];
let archive, frozenCount;
function promoted() {
  assert.ok(!fs.existsSync(file(active)), 'accepted native change must be archived');
  const archives = fs.readdirSync(file('.cairn/archive')).filter(name =>
    /^\d{4}-\d{2}-\d{2}-2026-09-26-declared-modules-v1$/.test(name));
  assert.equal(archives.length, 1, 'exactly one date-stamped native declared change archive required');
  const archived = `.cairn/archive/${archives[0]}`;
  const names = new Set(tracked());
  for (const name of [generated, `${active}/design.md`])
    assert.ok(!names.has(name), `obsolete or self-generated path still staged: ${name}`);
  for (const name of [
    `${archived}/metadata.json`, `${archived}/design.md`, `${archived}/proposal.md`,
    `${archived}/tasks.md`, `${archived}/specs/developer-experience/spec.md`,
    'verification/declared-modules-v1/assurance.json',
    'verification/declared-modules-v1/acceptance.json',
    'verification/declared-modules-v1/build.json',
    'verification/declared-modules-v1/extraction.json',
    'verification/declared-modules-v1/evidence.json',
    'verification/declared-modules-v1/final-documents.mjs',
    'specs/STATUS.json', 'specs/roadmap.json', 'specs/ROADMAP.md',
    'specs/DECISIONS.md', 'README.md', 'specs/README.md',
    'specs/DEVELOPER-EXPERIENCE.md',
    'verification/source-inventory.md', '.cairn/specs/developer-experience/spec.md',
    ...new Set(cases.map(([name]) => name)),
  ]) assert.ok(names.has(name), `promoted source not staged: ${name}`);
  assert.equal(fs.readFileSync(file('specs/DEVELOPER-EXPERIENCE.md'), 'utf8'),
    view(fs.readFileSync(file('.cairn/specs/developer-experience/spec.md'), 'utf8'),
      'DEVELOPER-EXPERIENCE.md'),
    'promoted DX compatibility view must exactly match the authoritative native Cairn spec');
  const archivedFiles = [];
  function visit(dir) {
    for (const item of fs.readdirSync(file(dir), { withFileTypes: true })) {
      const name = `${dir}/${item.name}`;
      if (item.isDirectory()) visit(name);
      else {
        assert.ok(item.isFile() && names.has(name), `unreviewed or unstaged archived file: ${name}`);
        archivedFiles.push(name);
      }
    }
  }
  visit(archived);
  const original = Object.keys(base.source_files).filter(name => name.startsWith(`${active}/`));
  const expected = original.filter(name => name.includes('/specs/')).map(name =>
    name.slice(active.length + 1)).sort();
  const actual = archivedFiles.filter(name => name.startsWith(`${archived}/specs/`)).map(name =>
    name.slice(archived.length + 1)).sort();
  assert.deepEqual(actual, expected, 'archived normative delta changed since acceptance');
  for (const name of [...expected, 'design.md', 'proposal.md']) assert.equal(
    hash(file(`${archived}/${name}`)), base.source_files[`${active}/${name}`]?.sha256,
    `accepted native contract/design changed during publication: ${name}`);
  assert.equal(hash(file('verification/declared-modules-v1/assurance.json')),
    hash(localAssurance), 'published pre-promotion assurance is not the accepted receipt');
  assert.equal(hash(file('verification/declared-modules-v1/acceptance.json')),
    acceptanceGate.sha256, 'published declared acceptance is not the fresh receipt');
  assert.equal(hash(file('verification/declared-modules-v1/build.json')),
    buildGate.sha256, 'published production build is not the fresh receipt');
  assert.equal(hash(file('verification/declared-modules-v1/extraction.json')),
    extractionGate.sha256, 'published whole-crate extraction is not the fresh receipt');
  const evidence = load(file('verification/declared-modules-v1/evidence.json'));
  assert.equal(evidence.schema, 'noble-declared-modules-evidence/v1');
  assert.equal(evidence.result, 'passed');
  assert.equal(evidence.pre_promotion_assurance_sha256, hash(localAssurance));
  assert.equal(evidence.acceptance_sha256, acceptanceGate.sha256);
  assert.equal(evidence.build_sha256, buildGate.sha256);
  assert.equal(evidence.extraction_sha256, extractionGate.sha256);
  assert.equal(evidence.source_revision, acceptance.source_revision);
  assert.deepEqual([...evidence.cases].sort(), ['DX-03', 'DX-08', 'DX-09']);
  for (const [document, id] of cases) {
    const selectedCases = load(file(document)).cases.filter(item => item.id === id);
    assert.equal(selectedCases.length, 1, `${id}: missing or duplicated canonical case`);
    const item = selectedCases[0];
    assert.equal(item.profile, 'Declared-Modules-v1');
    assert.deepEqual(item.state, { implementation: 'implemented', execution: 'passed',
      proof: 'open', trust: 'explicit' }, `${id}: incorrect bounded state/proof disclosure`);
    const originalCase = JSON.parse(fs.readFileSync(path.resolve(
      path.dirname(localAssurance), 'declared/plan/sources', document))).cases.find(row => row.id === id);
    assert.ok(originalCase, `${id}: pre-promotion canonical input missing`);
    assert.deepEqual({ input: item.input, expected: item.expected },
      { input: originalCase.input, expected: originalCase.expected },
      `${id}: canonical input/expected changed after execution`);
    const rows = acceptance.cases.filter(row => row.case_id === id);
    assert.ok(rows.length > 0 && rows.every(row =>
      row.source_revision === acceptance.source_revision),
    `${id}: incomplete matching actual results`);
    const variants = [...new Set(rows.map(row => row.variant))].sort();
    assert.ok(item.evidence?.some(entry => entry.kind === 'test' && entry.subject === id
      && entry.result === 'passed' && entry.source_revision === acceptance.source_revision
      && entry.configuration?.profile === 'Declared-Modules-v1'
      && entry.configuration?.case === id
      && entry.configuration?.receipt_sha256 === acceptanceGate.sha256
      && JSON.stringify([...(entry.configuration?.variants ?? [])].sort()) === JSON.stringify(variants)),
    `${id}: no exact receipt/source/variant-bound passing evidence`);
  }
  return archived;
}
function acceptedSourceUnchanged() {
  const promotedDocuments = new Set([
    '.cairn/specs/developer-experience/spec.md',
    'specs/STATUS.json', 'specs/roadmap.json', 'specs/ROADMAP.md',
    'specs/DECISIONS.md', 'specs/README.md',
    'specs/DEVELOPER-EXPERIENCE.md',
    'specs/conformance/developer-experience-cases.json',
    'specs/conformance/language-workflow-cases.json',
  ]);
  const acceptedChange = '.cairn/changes/2026-09-26-declared-modules-v1/';
  const files = Object.keys(base.source_files).filter(name =>
    !name.startsWith(acceptedChange) && !promotedDocuments.has(name));
  assert.ok(files.length > 50, 'accepted unchanged source list unexpectedly short');
  for (const name of files) assert.equal(hash(file(name)), base.source_files[name].sha256,
    `post-acceptance source mutation outside promoted documents requires fresh assurance: ${name}`);
  return files.length;
}
try { archive = promoted(); } catch (error) { if (mode === 'run') throw error; pending.push(String(error)); }
try { frozenCount = acceptedSourceUnchanged(); } catch (error) { if (mode === 'run') throw error; pending.push(String(error)); }
const runtime = process.env.XDG_RUNTIME_DIR;
if (!runtime || !fs.existsSync(path.join(runtime, 'bus')) ||
    !fs.statSync(path.join(runtime, 'bus')).isSocket())
  pending.push('Nix inputs require an active user XDG_RUNTIME_DIR bus');
if (!fs.existsSync('/etc/nix/nix.conf') ||
    hash('/etc/nix/nix.conf') !== nixConfigurationSha256)
  pending.push('run final Nix checks inside the reviewed user-owned read-only /etc/nix/nix.conf service bind');
if (mode === 'plan') {
  console.log(JSON.stringify({ schema: 'noble-declared-modules-final-documents-plan/v1',
    planned_result: pending.length ? 'blocked' : 'ready', prerequisites: pending,
    pre_promotion_assurance_sha256: hash(localAssurance), frozen_accepted_source_files: frozenCount,
    archive, artifact_directory: out, checks: ['staged Git source/worktree identity',
      'independent Nix documents', 'independent Nix Cairn', 'final self-excluded receipt publication'] }, null, 2));
  process.exit(0);
}
assert.deepEqual(pending, []);
assert.ok(!fs.existsSync(out), 'new external final document directory required');
assert.equal(fs.realpathSync(path.dirname(out)), path.dirname(out), 'artifact parent cannot be a symlink');
assert.ok(!fs.existsSync(file(generated)), 'previous final document receipt already exists');
fs.mkdirSync(out);
for (const name of ['commands', 'home', 'tmp', 'source']) fs.mkdirSync(path.join(out, name));
const names = tracked();
assert.ok(!names.includes(generated), 'self-excluded receipt must not be staged before checks');
const projection = gitSourceProjection(root, names);
const source = path.join(out, 'source');
for (const row of projection.rows) {
  const copy = path.join(source, row.path), original = file(row.path);
  fs.mkdirSync(path.dirname(copy), { recursive: true });
  if (row.kind === 'symlink') fs.symlinkSync(fs.readlinkSync(original), copy);
  else {
    fs.copyFileSync(original, copy, fs.constants.COPYFILE_EXCL);
    fs.chmodSync(copy, row.worktree_mode === '100755' ? 0o555 : 0o444);
  }
}
const report = { schema: 'noble-declared-modules-final-documents/v1', phase: 'post-promotion',
  result: 'running', source_root: root, artifact_directory: out,
  pre_promotion_assurance: { path: localAssurance, sha256: hash(localAssurance),
    source_revision: base.source_revision },
  declared_acceptance_source_revision: acceptance.source_revision,
  archived_change: archive, frozen_accepted_source_files: frozenCount,
  nix_configuration: { path: nixConfiguration, sha256: nixConfigurationSha256,
    mounted_system_path: '/etc/nix/nix.conf',
    scope: 'user-owned read-only private service bind; signed substitutes, sandbox and local builders' },
  source_projection: { excluded_generated_receipt: generated, git: projection,
    rule: 'All staged/tracked Git paths with exact staged blob IDs, worktree bytes, types and modes; only this generated receipt is excluded.' },
  tools: {}, commands: [], nix_source: null, outputs: {}, integrity_failures: [],
  non_claims: ['Universal nominal or source-to-Wasm refinement',
    'Postpromotion document checks re-executed prepromotion runtime/proof gates',
    'Historical M8 acceptance reissued as a declared-profile receipt',
    'The reviewed client configuration alone repairs shared Nix daemon or proves isolated-store equivalence'] };
const save = () => fs.writeFileSync(path.join(out, 'final-documents.json'), JSON.stringify(report, null, 2) + '\n');
const nix = '/run/current-system/sw/bin/nix';
const flags = ['--option', 'builders', '',
  '--option', 'min-free', '0', '--option', 'max-free', '0',
  '--option', 'max-jobs', '1',
  '--option', 'require-sigs', 'true',
  '--option', 'sandbox', 'true', '--option', 'sandbox-fallback', 'false',
  '--option', 'substituters',
  'https://cache.nixos.org https://nix-community.cachix.org',
  '--extra-experimental-features', 'nix-command flakes'];
const environment = { PATH: '/run/current-system/sw/bin', HOME: path.join(out, 'home'),
  TMPDIR: path.join(out, 'tmp'), LANG: 'C.UTF-8', LC_ALL: 'C.UTF-8', XDG_RUNTIME_DIR: runtime,
  NIX_USER_CONF_FILES: nixConfiguration };
function execute(label, argv, timeout = 3_600_000) {
  assert.equal(hash(nixConfiguration), nixConfigurationSha256,
    `${label}: reviewed private Nix client configuration changed`);
  assert.equal(hash('/etc/nix/nix.conf'), nixConfigurationSha256,
    `${label}: private Nix service bind changed`);
  const executable = fs.realpathSync(nix), digest = hash(executable);
  if (!report.tools[executable]) report.tools[executable] = { sha256: digest };
  assert.equal(report.tools[executable].sha256, digest, `${label}: Nix changed`);
  const stem = `commands/${String(report.commands.length + 1).padStart(3, '0')}-${label}`;
  const stdout = path.join(out, `${stem}.stdout`), stderr = path.join(out, `${stem}.stderr`);
  const fdout = fs.openSync(stdout, 'wx', 0o600), fderr = fs.openSync(stderr, 'wx', 0o600);
  const row = { label, executable, executable_sha256: digest, argv, cwd: root,
    environment, stdout: path.relative(out, stdout), stderr: path.relative(out, stderr), timeout_ms: timeout };
  report.commands.push(row); save();
  let result;
  try { result = spawnSync(executable, argv, { cwd: root, env: environment,
    stdio: ['ignore', fdout, fderr], timeout, killSignal: 'SIGKILL' }); }
  finally { fs.closeSync(fdout); fs.closeSync(fderr); }
  Object.assign(row, { status: result?.status ?? null, signal: result?.signal ?? null,
    error: result?.error ? String(result.error) : null,
    stdout_sha256: hash(stdout), stderr_sha256: hash(stderr) });
  for (const name of [stdout, stderr]) fs.chmodSync(name, 0o400);
  save();
  assert.equal(row.error, null, `${label}: launch failed`);
  assert.equal(row.signal, null, `${label}: command terminated`);
  assert.equal(row.status, 0, `${label}: command failed, see ${stderr}`);
  assert.equal(hash(executable), digest, `${label}: Nix changed during execution`);
  assert.equal(hash(nixConfiguration), nixConfigurationSha256,
    `${label}: reviewed Nix client configuration changed during execution`);
  return fs.readFileSync(stdout, 'utf8');
}
function unchanged(allowGenerated = false) {
  const current = tracked();
  assert.deepEqual(current.filter(name => name !== generated), names,
    'staged path set changed during final verification');
  if (!allowGenerated) assert.ok(!current.includes(generated));
  assert.deepEqual(gitSourceProjection(root, names), projection,
    'staged Git identity or worktree bytes changed during final verification');
  for (const row of projection.rows) {
    const copy = path.join(source, row.path), stat = fs.lstatSync(copy);
    assert.equal(stat.isSymbolicLink() ? 'symlink' : 'file', row.kind);
    assert.equal(sha(row.kind === 'symlink' ? Buffer.from(fs.readlinkSync(copy)) : fs.readFileSync(copy)),
      row.worktree_sha256, `source snapshot changed: ${row.path}`);
  }
}
try {
  const fetched = JSON.parse(execute('final-nix-source-prefetch',
    [...flags, 'flake', 'prefetch', '--json', `path:${source}`], 600_000));
  assert.ok(fetched.hash?.startsWith('sha256-') && fetched.storePath?.startsWith('/nix/store/'),
    'Nix did not materialize final staged source');
  const materialized = fs.realpathSync(fetched.storePath);
  for (const row of projection.rows) {
    const candidate = path.join(materialized, row.path), stat = fs.lstatSync(candidate);
    assert.equal(stat.isSymbolicLink() ? 'symlink' : 'file', row.kind,
      `Nix source changed file type: ${row.path}`);
    assert.equal(sha(row.kind === 'symlink' ? Buffer.from(fs.readlinkSync(candidate)) : fs.readFileSync(candidate)),
      row.worktree_sha256, `Nix omitted or changed staged source: ${row.path}`);
  }
  report.nix_source = { path: materialized, nar_hash: fetched.hash,
    projection_sha256: projection.sha256, checked_files: names.length };
  for (const check of ['documents', 'cairn']) {
    const output = execute(`final-nix-${check}`, [...flags, 'build', '--no-link',
      '--print-out-paths', `path:${source}#checks.x86_64-linux.${check}`]).trim();
    assert.ok(output.startsWith('/nix/store/') && !output.includes('\n')
      && fs.statSync(output).isDirectory(), `${check}: missing Nix check derivation`);
    report.outputs[check] = output;
  }
  unchanged();
  assert.equal(hash(localAssurance), report.pre_promotion_assurance.sha256);
  assert.equal(hash(file(base.proof_lock.path)), base.proof_lock.sha256);
  assert.equal(hash(nixConfiguration), nixConfigurationSha256);
  assert.equal(hash('/etc/nix/nix.conf'), nixConfigurationSha256);
  assert.equal(hash(file('verification/declared-modules-v1/acceptance.json')),
    acceptanceGate.sha256);
  report.result = 'passed';
  const publication = { ...report, publication: { path: generated,
    excluded_from_source_projection: true, git_action: `git add -- ${generated}` } };
  const bytes = JSON.stringify(publication, null, 2) + '\n';
  fs.writeFileSync(file(generated), bytes, { flag: 'wx', mode: 0o444 });
  gitCall(['add', '--', generated]);
  unchanged(true);
  report.publication = { path: generated, sha256: sha(bytes), staged: true,
    excluded_from_source_projection: true };
  report.after_publication_git_projection = gitSourceProjection(root, [...names, generated]);
} catch (error) {
  report.result = 'failed';
  report.integrity_failures.push(String(error.stack ?? error));
}
save();
console.log(JSON.stringify({ schema: report.schema, phase: report.phase, result: report.result,
  report: path.join(out, 'final-documents.json'), published: report.publication ?? null,
  output_checks: Object.keys(report.outputs), integrity_failures: report.integrity_failures }));
process.exitCode = report.result === 'passed' ? 0 : 1;
