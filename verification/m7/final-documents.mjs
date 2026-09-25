#!/usr/bin/env node
// Usage: SELECTED_NODE verification/m7/final-documents.mjs {plan|run} PRE_PROMOTION_ASSURANCE_JSON NEW_EXTERNAL_DIRECTORY
// Run ONLY after canonical case/evidence/roadmap promotion, Cairn sync/archive,
// and staging those new files. Distinct documents/Cairn Nix checks bind a frozen
// final Git-backed source projection. The generated Nix receipt is the sole
// excluded tracked path, avoiding a circular source hash.
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const root = fs.realpathSync(path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..'));
const [mode, assuranceFile, destination, ...extra] = process.argv.slice(2);
assert.ok(['plan', 'run'].includes(mode) && assuranceFile && destination && !extra.length,
  'usage: SELECTED_NODE verification/m7/final-documents.mjs {plan|run} PRE_PROMOTION_ASSURANCE_JSON NEW_EXTERNAL_DIRECTORY');
const out = path.resolve(destination);
assert.ok(out !== root && !out.startsWith(`${root}${path.sep}`), 'artifacts must be outside the repository');
const selected = JSON.parse(fs.readFileSync(path.join(root, 'policy/tool-selection.json')));
const pins = JSON.parse(fs.readFileSync(path.join(root, 'verification/m6/pins.json')));
const node = path.join(selected.tool_paths.node.output, 'bin/node');
assert.equal(fs.realpathSync(process.execPath), fs.realpathSync(node), 'use policy-selected Node');
assert.deepEqual(process.execArgv, []);
const nix = pins.nix ?? '/run/current-system/sw/bin/nix';
const git = '/run/current-system/sw/bin/git';
const bun = process.env.NOBLE_M7_BUN ?? '/nix/store/qkydg77xf2s395md9fq0a6djjpq7fzh4-bun-1.4.2/bin/bun';
const generated = 'verification/m7/nix-checks.json';
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const fileHash = file => sha(fs.readFileSync(file));
const localReceipt = path.resolve(assuranceFile);
let base;
try { base = JSON.parse(fs.readFileSync(localReceipt)); }
catch (error) {
  if (mode !== 'plan') throw error;
  console.log(JSON.stringify({ schema: 'noble-m7-final-documents-plan/v1', phase: 'post-promotion',
    planned_result: 'blocked', pre_promotion_receipt: localReceipt,
    prerequisites: [`Fresh passing pre-promotion assurance receipt is unavailable: ${error}`],
    next_steps: ['Complete strict extraction, compiled M7 acceptance, prior regressions and quality on frozen source.',
      'Promote canonical cases/evidence/roadmap, then Cairn sync/archive and stage the new source.',
      'Rerun this plan and its post-promotion documents/Cairn gate.'] }, null, 2));
  process.exit(0);
}
assert.equal(base.schema, 'noble-m7-assurance/v1');
assert.equal(base.phase, 'pre-promotion');
assert.equal(base.result, 'passed', 'pre-promotion acceptance, strict extraction, regression and quality receipt required');
assert.equal(base.commands.length, 16, 'incomplete previous command matrix');
assert.equal(Object.keys(base.gates).length, 10, 'missing previously verified gates');
for (const label of ['build', 'strict-whole-crate-extraction-check', 'm7-acceptance', 'mc1-regression',
  'mc2-regression', 'm3-four-configuration-regression', 'm4-runtime-regression', 'm5-component-regression',
  'm6-component-regression', 'nix_source']) assert.ok(base.gates[label], `missing pre-promotion gate: ${label}`);
for (const [label, binding] of Object.entries(base.gates)) {
  assert.equal(fileHash(path.resolve(path.dirname(localReceipt), binding.path)), binding.sha256,
    `previous ${label} receipt changed or disappeared`);
}
assert.equal(fileHash(path.join(root, 'verification/m4/extraction-lock.json')), base.proof_lock.sha256);
function gitCall(args) {
  const result = spawnSync(git, ['-C', root, ...args], { maxBuffer: 32 * 1024 * 1024,
    env: { PATH: '/run/current-system/sw/bin', HOME: process.env.HOME ?? '/nonexistent' }, timeout: 30_000 });
  assert.equal(result.error, undefined, `Git invocation failed: ${args.join(' ')}`);
  assert.equal(result.status, 0, `Git ${args.join(' ')} failed: ${result.stderr.toString('utf8')}`);
  return result.stdout;
}
const tracked = () => gitCall(['ls-files', '--cached', '-z']).toString('utf8').split('\0').filter(Boolean).sort();
function identity(file) {
  const absolute = path.join(root, file);
  const stat = fs.lstatSync(absolute);
  if (stat.isSymbolicLink()) return { kind: 'symlink', target: fs.readlinkSync(absolute) };
  assert.ok(stat.isFile(), `unreviewed source type: ${file}`);
  return { kind: 'file', sha256: fileHash(absolute), bytes: stat.size, executable: !!(stat.mode & 0o111) };
}
function productionUnchanged() {
  const protectedFiles = Object.keys(base.source_files).filter(file => file.startsWith('crates/')
    || file.startsWith('proofs/m7/') || file.startsWith('policy/')
    || (file.startsWith('verification/m7/') && file !== generated)
    || ['Cargo.toml', 'Cargo.lock', 'flake.nix', 'flake.lock', 'rust-toolchain.toml',
      'verification/m4/extraction-lock.json', 'verification/m4/implementation.mjs'].includes(file));
  assert.ok(protectedFiles.length > 12, 'missing frozen production/config bindings');
  for (const file of protectedFiles) {
    assert.equal(fileHash(path.join(root, file)), base.source_files[file].sha256,
      `post-acceptance production/config mutation requires a fresh complete pre-promotion run: ${file}`);
  }
  return protectedFiles.length;
}
function promotion() {
  assert.ok(!fs.existsSync(path.join(root, '.cairn/changes/m7-syndicate-service')),
    'M7 Cairn change must be synchronized and archived');
  const archived = fs.readdirSync(path.join(root, '.cairn/archive'))
    .filter(name => name.endsWith('m7-syndicate-service'));
  assert.equal(archived.length, 1, 'exactly one archived M7 Cairn change required');
  const names = new Set(tracked());
  const needed = [
    `.cairn/archive/${archived[0]}/metadata.json`,
    'specs/ROADMAP.md', 'specs/DECISIONS.md', 'specs/SAFETY.md', 'specs/WIT-WASI.md',
    'specs/requirements.json', 'specs/VALIDATION.json',
    'verification/m7/evidence.json', 'verification/m7/acceptance.json',
    'verification/m7/final-documents.mjs',
  ];
  for (const file of needed) assert.ok(names.has(file), `promoted file must be Git staged/tracked: ${file}`);
  const archiveRoot = `.cairn/archive/${archived[0]}`;
  function visit(directory) {
    for (const item of fs.readdirSync(path.join(root, directory), { withFileTypes: true })) {
      const file = `${directory}/${item.name}`;
      if (item.isDirectory()) visit(file);
      else {
        assert.ok(item.isFile(), `unexpected archived change entry: ${file}`);
        assert.ok(names.has(file), `archived change file is not Git staged/tracked: ${file}`);
      }
    }
  }
  visit(archiveRoot);
  const gate = base.gates['m7-acceptance'];
  assert.equal(fileHash(path.join(root, 'verification/m7/acceptance.json')), gate.sha256,
    'promoted M7 acceptance must be the exact fresh pre-promotion gate receipt');
  const evidence = JSON.parse(fs.readFileSync(path.join(root, 'verification/m7/evidence.json')));
  assert.equal(evidence.schema, 'noble-m7-evidence/v1');
  assert.equal(evidence.result, 'passed');
  assert.equal(evidence.build?.receipt_sha256, base.gates.build.sha256);
  assert.equal(evidence.runtime?.receipt_sha256, gate.sha256);
  assert.equal(evidence.extraction?.receipt_sha256,
    base.gates['strict-whole-crate-extraction-check'].sha256);
  assert.equal(evidence.quality?.pre_promotion_receipt_sha256, fileHash(localReceipt));
  assert.equal(evidence.runtime?.variants, 16);
  assert.deepEqual([...evidence.runtime.cases].sort(),
    ['S-CASE-09', 'S-CASE-10', 'S-CASE-11', 'S-CASE-12', 'S-CASE-17', 'WI-14', 'WI-18'].sort());
  for (const [file, ids] of [['specs/conformance/safety-cases.json', ['S-CASE-09', 'S-CASE-10', 'S-CASE-11', 'S-CASE-12', 'S-CASE-17']],
    ['specs/conformance/wit-wasi-cases.json', ['WI-14', 'WI-18']]]) {
    const doc = JSON.parse(fs.readFileSync(path.join(root, file)));
    for (const id of ids) {
      const rows = doc.cases.filter(row => row.id === id);
      assert.equal(rows.length, 1, `${id}: duplicate/missing canonical case`);
      assert.equal(rows[0].state?.implementation, 'implemented', `${id}: implementation not promoted`);
      assert.equal(rows[0].state?.execution, 'passed', `${id}: execution not promoted`);
      assert.ok(rows[0].evidence?.some(entry => entry.result === 'passed' && entry.kind === 'test'
        && entry.subject === id && entry.source_revision === base.gates.build.source_revision
        && entry.configuration?.profile === 'Syndicate-Sync-Service-M7'
        && entry.configuration?.case === id && entry.configuration?.receipt_sha256 === gate.sha256),
      `${id}: no passing evidence bound to the exact M7 source and fresh acceptance receipt`);
    }
  }
  return archived[0];
}
const runtimeDirectory = process.env.XDG_RUNTIME_DIR;
const userBus = runtimeDirectory && path.join(runtimeDirectory, 'bus');
const pending = [];
if (!userBus || !fs.existsSync(userBus) || !fs.statSync(userBus).isSocket())
  pending.push('Pinned Nix inputs require the active user XDG_RUNTIME_DIR bus');
let archive, frozen;
try { archive = promotion(); } catch (error) {
  if (mode === 'run') throw error;
  pending.push(String(error.message ?? error));
}
try { frozen = productionUnchanged(); } catch (error) {
  if (mode === 'run') throw error;
  pending.push(String(error.message ?? error));
}
if (mode === 'plan') {
  console.log(JSON.stringify({ schema: 'noble-m7-final-documents-plan/v1', phase: 'post-promotion',
    planned_result: pending.length ? 'blocked' : 'ready', prerequisites: pending,
    pre_promotion_receipt: localReceipt, pre_promotion_sha256: fileHash(localReceipt),
    frozen_implementation_files_checked: frozen, artifact_directory: out,
    required_before_run: ['Promote exactly seven canonical case states with passed evidence and update roadmap/decision/ledger views.',
      'Cairn sync/archive the M7 change; stage the archive, generated evidence and all final changed files explicitly.',
      `Only ${generated} is excluded from the generated receipt's own frozen Git-backed source projection.`],
    checks: ['Fresh final Git source snapshot with exact byte/mode/symlink manifest',
      'Separate Nix documents and Cairn derivation builds on that frozen post-promotion source',
      `Generate and stage ${generated}; verify current tracked source still matches the projection excluding that receipt.`],
    non_claims: ['The pre-promotion Nix check binds post-promotion documents', 'A document check replaces source-bound execution/proof evidence'] }, null, 2));
  process.exit(0);
}
assert.deepEqual(pending, [], 'final document prerequisites must pass before creating output');
assert.ok(!fs.existsSync(out), 'use a new artifact directory');
assert.equal(fs.realpathSync(path.dirname(out)), path.dirname(out), 'artifact parent must not be a symlink');
assert.ok(!fs.existsSync(path.join(root, generated)), 'final self-excluding Nix receipt must not exist before run');
fs.mkdirSync(out);
for (const directory of ['commands', 'home', 'tmp', 'source']) fs.mkdirSync(path.join(out, directory));
const source = path.join(out, 'source');
const names = tracked();
assert.ok(!names.includes(generated), 'prior Nix receipt must not be tracked before this run');
const manifest = Object.fromEntries(names.map(file => [file, identity(file)]));
assert.ok(manifest[`.cairn/archive/${archive}/metadata.json`], 'archive not staged');
for (const [file, value] of Object.entries(manifest)) {
  const original = path.join(root, file), snapshot = path.join(source, file);
  fs.mkdirSync(path.dirname(snapshot), { recursive: true });
  if (value.kind === 'symlink') fs.symlinkSync(value.target, snapshot);
  else {
    fs.copyFileSync(original, snapshot, fs.constants.COPYFILE_EXCL);
    fs.chmodSync(snapshot, value.executable ? 0o555 : 0o444);
  }
}
const receipt = { schema: 'noble-m7-final-documents/v1', phase: 'post-promotion', result: 'running',
  source_root: root, artifact_directory: out,
  pre_promotion_assurance: { path: localReceipt, sha256: fileHash(localReceipt),
    result: base.result, source_revision: base.source_revision },
  archived_change: archive, frozen_implementation_files: frozen,
  source_projection: { excluded_generated_receipt: generated, files: manifest,
    sha256: sha(JSON.stringify(manifest)), file_count: names.length,
    rule: 'All and only staged/tracked Git paths at post-promotion review, exact bytes/executable bit/symlink target; only this generated receipt is omitted.' },
  tools: {}, commands: [], nix_source: null, outputs: {}, integrity_failures: [],
  non_claims: ['Universal compiler or engine refinement', 'This receipt re-executed the pre-promotion runtime/strict extraction gates',
    'Pre-promotion quality Nix checks bind the promoted/archived document source'],
};
const save = () => fs.writeFileSync(path.join(out, 'final-documents.json'), JSON.stringify(receipt, null, 2) + '\n');
const common = { PATH: '/run/current-system/sw/bin', HOME: path.join(out, 'home'), TMPDIR: path.join(out, 'tmp'),
  LANG: 'C.UTF-8', LC_ALL: 'C.UTF-8', XDG_RUNTIME_DIR: runtimeDirectory };
function run(label, executable, argv, timeout = 1_800_000) {
  const tool = fs.realpathSync(executable), digest = fileHash(tool);
  if (!receipt.tools[tool]) receipt.tools[tool] = { sha256: digest };
  assert.equal(receipt.tools[tool].sha256, digest);
  const stem = `commands/${String(receipt.commands.length + 1).padStart(3, '0')}-${label}`;
  const stdout = path.join(out, `${stem}.stdout`), stderr = path.join(out, `${stem}.stderr`);
  const outFd = fs.openSync(stdout, 'wx', 0o600), errFd = fs.openSync(stderr, 'wx', 0o600);
  const row = { label, executable: tool, sha256: digest, argv, cwd: root,
    environment: common, timeout_ms: timeout, stdout: path.relative(out, stdout), stderr: path.relative(out, stderr) };
  receipt.commands.push(row); save();
  let result;
  try { result = spawnSync(tool, argv, { cwd: root, env: common, stdio: ['ignore', outFd, errFd],
    timeout, killSignal: 'SIGKILL' }); }
  finally { fs.closeSync(outFd); fs.closeSync(errFd); }
  Object.assign(row, { exit: result?.status ?? null, signal: result?.signal ?? null,
    error: result?.error ? String(result.error) : null, stdout_sha256: fileHash(stdout), stderr_sha256: fileHash(stderr) });
  for (const file of [stdout, stderr]) fs.chmodSync(file, 0o400);
  save();
  assert.equal(row.error, null, `${label}: failed to execute`);
  assert.equal(row.signal, null, `${label}: killed`);
  assert.equal(row.exit, 0, `${label}: see ${row.stderr}`);
  assert.equal(fileHash(tool), digest, `${label}: tool changed`);
  return fs.readFileSync(stdout, 'utf8');
}
function snapshotUnchanged() {
  assert.deepEqual(tracked(), names, 'Git tracked path set changed during final Nix checks');
  for (const [file, value] of Object.entries(manifest)) {
    assert.deepEqual(identity(file), value, `final working source changed: ${file}`);
    const staged = path.join(source, file);
    if (value.kind === 'symlink') assert.equal(fs.readlinkSync(staged), value.target, `frozen symlink changed: ${file}`);
    else {
      assert.equal(fileHash(staged), value.sha256, `frozen bytes changed: ${file}`);
      assert.equal(!!(fs.statSync(staged).mode & 0o111), value.executable, `frozen mode changed: ${file}`);
    }
  }
}
let generatedBytes;
let publishedCreated = false;
try {
  const flags = ['--option', 'substituters', 'https://cache.nixos.org https://nix-community.cachix.org',
    '--extra-experimental-features', 'nix-command flakes'];
  run('canonical-document-validation', bun,
    [path.join(root, 'tools/check-specs.mjs'), '--self-test', '--report'], 300_000);
  const fetched = JSON.parse(run('final-nix-source-prefetch', nix,
    [...flags, 'flake', 'prefetch', '--json', `path:${source}`], 600_000));
  assert.ok(typeof fetched.storePath === 'string' && fetched.storePath.length > 0,
    'Nix prefetch must identify a materialized final source');
  assert.ok(typeof fetched.hash === 'string' && fetched.hash.startsWith('sha256-'),
    'Nix prefetch must retain the final source NAR hash');
  const frozenSource = fs.realpathSync(fetched.storePath);
  assert.ok(fs.statSync(frozenSource).isDirectory(), 'Nix prefetched final source must be a directory');
  for (const [file, value] of Object.entries(manifest)) {
    const observed = path.join(frozenSource, file);
    if (value.kind === 'symlink') assert.equal(fs.readlinkSync(observed), value.target, `Nix excluded final symlink ${file}`);
    else {
      assert.ok(fs.statSync(observed).isFile(), `Nix prefetch excluded final document source ${file}`);
      assert.equal(fileHash(observed), value.sha256, `Nix excluded/rewrote final document source ${file}`);
      assert.equal(!!(fs.statSync(observed).mode & 0o111), value.executable, `Nix changed source mode ${file}`);
    }
  }
  receipt.nix_source = { path: frozenSource, nar_hash: fetched.hash, projection_sha256: receipt.source_projection.sha256,
    checked_files: names.length };
  const target = `path:${source}`;
  for (const name of ['documents', 'cairn']) {
    const result = run(`final-nix-${name}`, nix,
      [...flags, 'build', '--no-link', '--print-out-paths',
        `${target}#checks.x86_64-linux.${name}`], 3_600_000).trim();
    assert.ok(!result.includes('\n'), `${name}: expected one Nix output`);
    assert.ok(result.startsWith('/nix/store/'), `missing ${name} Nix output`);
    assert.ok(fs.statSync(result).isDirectory(), `missing built ${name} Nix output`);
    receipt.outputs[name] = result;
  }
  snapshotUnchanged();
  assert.equal(fileHash(localReceipt), receipt.pre_promotion_assurance.sha256);
  assert.equal(fileHash(path.join(root, 'verification/m4/extraction-lock.json')), base.proof_lock.sha256);
  receipt.result = 'passed';
  const published = { ...receipt, publication: { path: generated,
    excluded_from_source_projection: true, git_action: `git add -- ${generated}` } };
  generatedBytes = JSON.stringify(published, null, 2) + '\n';
  fs.writeFileSync(path.join(root, generated), generatedBytes, { flag: 'wx', mode: 0o444 });
  publishedCreated = true;
  gitCall(['add', '--', generated]);
  assert.deepEqual(tracked().filter(file => file !== generated), names,
    'only self-generated Nix receipt may change the final source projection');
  snapshotUnchangedAfterPublication();
  receipt.publication = { path: generated, sha256: sha(generatedBytes), staged: true,
    excluded_from_source_projection: true };
} catch (error) {
  receipt.result = 'failed';
  receipt.integrity_failures.push(String(error.stack ?? error));
  if (publishedCreated) {
    try {
      const failed = path.join(root, generated);
      fs.chmodSync(failed, 0o644);
      fs.writeFileSync(failed, JSON.stringify({ schema: receipt.schema, phase: receipt.phase,
        result: 'failed', artifact_directory: out, integrity_failures: receipt.integrity_failures }, null, 2) + '\n');
      gitCall(['add', '--', generated]);
    } catch (failure) { receipt.integrity_failures.push(`Could not stage failure notice: ${failure}`); }
  }
}
save();
console.log(JSON.stringify({ schema: receipt.schema, phase: receipt.phase, result: receipt.result,
  report: path.join(out, 'final-documents.json'), published: receipt.publication ?? null,
  output_checks: Object.keys(receipt.outputs), integrity_failures: receipt.integrity_failures }));
process.exitCode = receipt.result === 'passed' ? 0 : 1;

function snapshotUnchangedAfterPublication() {
  assert.equal(fileHash(path.join(root, generated)), sha(generatedBytes));
  for (const [file, value] of Object.entries(manifest)) assert.deepEqual(identity(file), value,
    `final source changed after publication: ${file}`);
}
