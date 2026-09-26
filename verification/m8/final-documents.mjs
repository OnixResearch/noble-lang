#!/usr/bin/env node
// Distinct post-promotion document gate. Only a fresh, fully passing M8
// pre-promotion assurance may authorize promotion; Nix documents and Cairn
// checks then run independently on the exact staged-path/worktree-byte source.
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { gitSourceProjection } from './source-projection.mjs';

const root = fs.realpathSync(path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..'));
const [mode, assuranceFile, destination, ...extra] = process.argv.slice(2);
assert.ok(['plan', 'run'].includes(mode) && assuranceFile && destination && !extra.length,
  'usage: SELECTED_NODE verification/m8/final-documents.mjs {plan|run} PRE_PROMOTION_ASSURANCE_JSON NEW_EXTERNAL_DIRECTORY');
const out = path.resolve(destination);
assert.ok(out !== root && !out.startsWith(`${root}${path.sep}`), 'external artifacts required');
const selected = JSON.parse(fs.readFileSync(path.join(root, 'policy/tool-selection.json')));
const pins = JSON.parse(fs.readFileSync(path.join(root, 'verification/m6/pins.json')));
const node = path.join(selected.tool_paths.node.output, 'bin/node');
assert.equal(fs.realpathSync(process.execPath), fs.realpathSync(node), 'policy-selected Node required');
assert.deepEqual(process.execArgv, []);
const nix = pins.nix ?? '/run/current-system/sw/bin/nix';
const git = '/run/current-system/sw/bin/git';
const bun = process.env.NOBLE_M8_BUN ?? '/nix/store/qkydg77xf2s395md9fq0a6djjpq7fzh4-bun-1.4.2/bin/bun';
const generated = 'verification/m8/nix-checks.json';
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const hash = file => sha(fs.readFileSync(file));
const localReceipt = path.resolve(assuranceFile);
let base;
try { base = JSON.parse(fs.readFileSync(localReceipt)); }
catch (error) {
  if (mode !== 'plan') throw error;
  console.log(JSON.stringify({ schema: 'noble-m8-final-documents-plan/v1', phase: 'post-promotion',
    planned_result: 'blocked', pre_promotion_receipt: localReceipt,
    prerequisites: [`Fresh passing pre-promotion M8 assurance receipt unavailable: ${error}`] }, null, 2));
  process.exit(0);
}
assert.equal(base.schema, 'noble-m8-assurance/v1');
assert.equal(base.phase, 'pre-promotion');
assert.equal(base.result, 'passed', 'fresh complete pre-promotion matrix required');
assert.equal(base.commands.length, 17, 'incomplete M8 command matrix');
assert.equal(Object.keys(base.gates).length, 11, 'missing fresh source/proof/runtime regression gates');
for (const label of ['nix_source', 'build', 'strict-whole-crate-extraction-check', 'm8-acceptance',
  'm7-regression', 'mc1-regression', 'mc2-regression', 'm3-four-configuration-regression',
  'm4-runtime-regression', 'm5-component-regression', 'm6-component-regression']) {
  const binding = base.gates[label];
  assert.ok(binding, `missing pre-promotion gate: ${label}`);
  assert.equal(hash(path.resolve(path.dirname(localReceipt), binding.path)), binding.sha256,
    `pre-promotion ${label} receipt changed`);
}
assert.equal(hash(path.join(root, 'verification/m4/extraction-lock.json')), base.proof_lock.sha256,
  'strict whole-crate proof lock changed since acceptance');
function gitCall(args) {
  const result = spawnSync(git, ['-C', root, ...args], { encoding: 'utf8', maxBuffer: 32 * 1024 * 1024,
    env: { ...process.env, GIT_OPTIONAL_LOCKS: '0' }, timeout: 30_000 });
  assert.equal(result.error, undefined, `Git ${args[0]} launch failed`);
  assert.equal(result.signal, null, `Git ${args[0]} terminated`);
  assert.equal(result.status, 0, `Git ${args[0]}: ${result.stderr}`);
  return result.stdout;
}
const tracked = () => gitCall(['ls-files', '--cached', '-z']).split('\0').filter(Boolean).sort();
function productionUnchanged() {
  const files = Object.keys(base.source_files).filter(file => file.startsWith('crates/')
    || file.startsWith('nix/')
    || file.startsWith('proofs/') || file.startsWith('policy/')
    || file.startsWith('verification/m8/') || file.startsWith('verification/m7/peer/')
    || ['.cairn/specs/safety/spec.md', '.cairn/specs/wit-wasi/spec.md'].includes(file)
    || ['Cargo.toml', 'Cargo.lock', 'flake.nix', 'flake.lock', 'rust-toolchain.toml',
      'verification/m4/extraction-lock.json', 'verification/m4/implementation.mjs'].includes(file));
  for (const file of ['.cairn/specs/safety/spec.md', '.cairn/specs/wit-wasi/spec.md'])
    assert.ok(base.source_files[file]?.sha256, `missing accepted canonical contract binding: ${file}`);
  assert.ok(files.length > 25, 'missing source-frozen implementation bindings');
  for (const file of files) assert.equal(hash(path.join(root, file)), base.source_files[file].sha256,
    `post-acceptance source/config mutation requires a fresh complete pre-promotion run: ${file}`);
  return files.length;
}
function promoted() {
  assert.ok(!fs.existsSync(path.join(root, '.cairn/changes/m8-choreography-projection')),
    'active M8 Cairn change must be synced and archived');
  const archived = fs.readdirSync(path.join(root, '.cairn/archive'))
    .filter(name => name.endsWith('m8-choreography-projection'));
  assert.equal(archived.length, 1, 'exactly one M8 Cairn archive required');
  const archive = `.cairn/archive/${archived[0]}`;
  const names = new Set(tracked());
  const archivedFiles = [];
  const required = [`${archive}/metadata.json`, 'verification/m8/acceptance.json',
    'verification/m8/build.json', 'verification/m8/extraction.json',
    'verification/m8/assurance.json', 'verification/m8/evidence.json',
    'verification/m8/final-documents.mjs',
    'specs/ROADMAP.md', 'specs/DECISIONS.md', 'specs/STATUS.json',
    'specs/SAFETY.md', 'specs/WIT-WASI.md',
    'specs/requirements.json', 'specs/VALIDATION.json',
    'specs/conformance/safety-cases.json', 'specs/conformance/wit-wasi-cases.json',
    '.cairn/specs/safety/spec.md', '.cairn/specs/wit-wasi/spec.md'];
  for (const file of required) assert.ok(names.has(file), `promoted file must be staged/tracked: ${file}`);
  function archiveFiles(directory) {
    for (const item of fs.readdirSync(path.join(root, directory), { withFileTypes: true })) {
      const file = `${directory}/${item.name}`;
      if (item.isDirectory()) archiveFiles(file);
      else {
        assert.ok(item.isFile(), `unreviewed archive entry: ${file}`);
        assert.ok(names.has(file), `archive entry must be staged/tracked: ${file}`);
        archivedFiles.push(file);
      }
    }
  }
  archiveFiles(archive);
  const activePrefix = '.cairn/changes/m8-choreography-projection/';
  const expectedSpecs = Object.keys(base.source_files).filter(file =>
    file.startsWith(`${activePrefix}specs/`)).map(file => file.slice(activePrefix.length)).sort();
  assert.ok(expectedSpecs.includes('specs/safety/spec.md')
    && expectedSpecs.includes('specs/wit-wasi/spec.md'),
  'pre-promotion assurance did not bind both active M8 normative specs');
  const actualSpecs = archivedFiles.filter(file => file.startsWith(`${archive}/specs/`))
    .map(file => file.slice(archive.length + 1)).sort();
  assert.deepEqual(actualSpecs, expectedSpecs,
    'archived normative spec subtree differs from the accepted active change');
  // Tasks and archive metadata may record lifecycle progress; contract and
  // design bytes cannot inherit a pre-promotion receipt after mutation.
  for (const relative of [...expectedSpecs, 'design.md', 'proposal.md']) {
    const before = base.source_files[`${activePrefix}${relative}`];
    assert.ok(before?.sha256, `missing accepted active-change binding: ${relative}`);
    const file = `${archive}/${relative}`;
    assert.ok(names.has(file), `missing staged archive contract entry: ${file}`);
    assert.equal(hash(path.join(root, file)), before.sha256,
      `archived M8 contract changed after pre-promotion acceptance: ${relative}`);
  }
  const gate = base.gates['m8-acceptance'];
  assert.equal(hash(path.join(root, 'verification/m8/acceptance.json')), gate.sha256,
    'promoted acceptance must be the exact freshly run compiled-component gate receipt');
  const accepted = JSON.parse(fs.readFileSync(path.join(root, 'verification/m8/acceptance.json')));
  assert.equal(accepted.schema, 'noble-m8-acceptance/v1');
  assert.equal(accepted.result, 'passed');
  assert.equal(accepted.summary?.strict_json_edges, 10);
  assert.equal(accepted.summary?.one_round_reservations, 3);
  assert.equal(accepted.summary?.variants, 38);
  const evidence = JSON.parse(fs.readFileSync(path.join(root, 'verification/m8/evidence.json')));
  assert.equal(evidence.schema, 'noble-m8-evidence/v1');
  assert.equal(evidence.result, 'passed');
  assert.equal(evidence.build?.receipt_sha256, base.gates.build.sha256);
  assert.equal(evidence.runtime?.receipt_sha256, gate.sha256);
  assert.equal(evidence.extraction?.receipt_sha256,
    base.gates['strict-whole-crate-extraction-check'].sha256);
  assert.equal(evidence.quality?.pre_promotion_receipt_sha256, hash(localReceipt));
  for (const [file, expected] of [
    ['build.json', base.gates.build.sha256],
    ['extraction.json', base.gates['strict-whole-crate-extraction-check'].sha256],
    ['assurance.json', hash(localReceipt)],
  ]) assert.equal(hash(path.join(root, 'verification/m8', file)), expected,
    `promoted M8 ${file} must be the exact fresh source-bound receipt`);
  assert.equal(evidence.build?.source_revision, base.gates.build.source_revision);
  assert.equal(evidence.extraction?.source_revision,
    base.gates['strict-whole-crate-extraction-check'].source_revision);
  assert.equal(evidence.runtime?.variants, 38);
  assert.equal(evidence.runtime?.strict_json_edges, 10);
  assert.equal(evidence.runtime?.one_round_reservations, 3);
  const status = JSON.parse(fs.readFileSync(path.join(root, 'specs/STATUS.json')));
  assert.equal(status.next_milestone, null,
    'M8 is the final selected milestone; MA1/MW1 remain unselected');
  const policy = status.verification_policy;
  assert.equal(policy.body_obligations, 2087, 'M8 authored production bodies');
  assert.equal(policy.open_refinement_obligations, 2087, 'all M8 body refinements remain open');
  for (const count of ['30-unit', '2087', '2026', '939'])
    assert.ok(policy.coverage_scope?.includes(count), `M8 coverage omits ${count}`);
  const extractionScope = policy.extraction_scope?.toLowerCase() ?? '';
  for (const obligation of ['m7', 'm8', 'parser', 'monitor', 'adapter', 'engine', 'open'])
    assert.ok(extractionScope.includes(obligation),
      `M8 extraction scope must retain bounded M7 proof and open ${obligation} obligations`);
  assert.equal(policy.m7_extraction_acceptance,
    'passed-independent-whole-crate-check-three-pure-dataspace-functions-nine-strict-roots-and-37-m7-refusal-controls');
  for (const proof of ['../verification/m7/extraction.json', '../verification/m8/extraction.json'])
    assert.ok(policy.independent_proof_records?.includes(proof), `missing independent proof record: ${proof}`);
  const promotedComponents = status.components?.filter(component => component.milestone === 'M8') ?? [];
  assert.equal(promotedComponents.length, 1, 'one M8 STATUS component must be promoted');
  assert.equal(promotedComponents[0].proof, 'open', 'M8 parser, monitor, adapter and engine proofs remain open');
  assert.equal(promotedComponents[0].execution, 'passed');
  assert.equal(promotedComponents[0].acceptance, 'passed');
  assert.equal(promotedComponents[0].completion_record, '../verification/m8/evidence.json');
  const idsByFile = [
    ['specs/conformance/safety-cases.json', ['S-CASE-18', 'S-CASE-19']],
    ['specs/conformance/wit-wasi-cases.json', ['WI-19', 'WI-20']],
  ];
  const ids = idsByFile.flatMap(([, values]) => values);
  assert.deepEqual([...evidence.runtime.cases].sort(), [...ids].sort());
  assert.deepEqual(accepted.cases.map(row => row.id).sort(), [...ids].sort());
  for (const [file, requiredIds] of idsByFile) {
    const document = JSON.parse(fs.readFileSync(path.join(root, file)));
    for (const id of requiredIds) {
      const cases = document.cases.filter(row => row.id === id);
      assert.equal(cases.length, 1, `${id}: missing/duplicate canonical case`);
      const item = cases[0];
      assert.equal(item.profile, 'Choreography-Service-M8');
      assert.equal(item.state?.implementation, 'implemented');
      assert.equal(item.state?.execution, 'passed');
      assert.equal(item.state?.proof, 'open', 'bounded M8 runtime evidence must not claim universal proof');
      const performed = accepted.cases.find(row => row.id === id);
      assert.equal(performed.result, 'passed');
      const variantNames = performed.variants.map(row => row.name).sort();
      assert.ok(item.evidence?.some(entry => entry.result === 'passed' && entry.kind === 'test'
        && entry.subject === id && entry.source_revision === base.gates.build.source_revision
        && entry.configuration?.profile === 'Choreography-Service-M8'
        && entry.configuration?.case === id && entry.configuration?.receipt_sha256 === gate.sha256
        && JSON.stringify([...(entry.configuration?.variants ?? [])].sort()) === JSON.stringify(variantNames)),
      `${id}: no exact source/case/variant-bound fresh acceptance evidence`);
    }
  }
  return archive;
}
const runtime = process.env.XDG_RUNTIME_DIR;
const bus = runtime && path.join(runtime, 'bus');
const pending = [];
if (!bus || !fs.existsSync(bus) || !fs.statSync(bus).isSocket())
  pending.push('selected Nix inputs require active user XDG_RUNTIME_DIR bus');
let archive, frozen;
try { archive = promoted(); } catch (error) {
  if (mode === 'run') throw error;
  pending.push(String(error.message ?? error));
}
try { frozen = productionUnchanged(); } catch (error) {
  if (mode === 'run') throw error;
  pending.push(String(error.message ?? error));
}
if (mode === 'plan') {
  console.log(JSON.stringify({ schema: 'noble-m8-final-documents-plan/v1', phase: 'post-promotion',
    planned_result: pending.length ? 'blocked' : 'ready', prerequisites: pending,
    pre_promotion_receipt: localReceipt, pre_promotion_sha256: hash(localReceipt),
    frozen_implementation_files_checked: frozen, artifact_directory: out,
    required_before_run: ['Promote four exact fresh case results/evidence and roadmap/ledger views.',
      'Cairn sync/archive M8 change; stage the archive, documents and M8 evidence without a commit.',
      `Only ${generated} is omitted from its own frozen Nix source projection.`],
    checks: ['Compare staged Git blob IDs, worktree bytes and frozen source copy',
      'Independently build Nix documents and Cairn derivations on exact promoted source',
      `Publish and stage ${generated}, then record exact final staged/worktree projection.`] }, null, 2));
  process.exit(0);
}
assert.deepEqual(pending, [], 'final document prerequisites unresolved');
assert.ok(!fs.existsSync(out), 'fresh external final document directory required');
assert.equal(fs.realpathSync(path.dirname(out)), path.dirname(out), 'artifact parent cannot be a symlink');
assert.ok(!fs.existsSync(path.join(root, generated)), 'self-excluded generated Nix receipt already exists');
fs.mkdirSync(out);
for (const name of ['commands', 'home', 'tmp', 'source']) fs.mkdirSync(path.join(out, name));
const names = tracked();
assert.ok(!names.includes(generated), 'previous final receipt must not be tracked');
const projection = gitSourceProjection(root, names, false);
assert.ok(projection.rows.every(row => row.index), 'tracked source index entry missing');
assert.ok(projection.rows.some(row => row.path === `${archive}/metadata.json`));
const source = path.join(out, 'source');
for (const row of projection.rows) {
  const original = path.join(root, row.path), copy = path.join(source, row.path);
  fs.mkdirSync(path.dirname(copy), { recursive: true });
  if (row.kind === 'symlink') fs.symlinkSync(fs.readlinkSync(original), copy);
  else {
    fs.copyFileSync(original, copy, fs.constants.COPYFILE_EXCL);
    fs.chmodSync(copy, row.worktree_mode === '100755' ? 0o555 : 0o444);
  }
}
const receipt = { schema: 'noble-m8-final-documents/v1', phase: 'post-promotion', result: 'running',
  source_root: root, artifact_directory: out,
  pre_promotion_assurance: { path: localReceipt, sha256: hash(localReceipt),
    result: base.result, source_revision: base.source_revision },
  archived_change: archive, frozen_implementation_files: frozen,
  source_projection: { excluded_generated_receipt: generated, git: projection,
    rule: 'All and only staged/tracked Git paths with exact staged blob IDs, worktree bytes, types and modes; only this self-generated receipt is excluded.' },
  tools: {}, commands: [], nix_source: null, outputs: {}, integrity_failures: [],
  non_claims: ['Universal compiler, parser, Wasmtime or engine refinement',
    'Post-promotion document checks re-executed pre-promotion runtime/proof gates',
    'Pre-promotion Nix checks automatically bind newly promoted documents'] };
const save = () => fs.writeFileSync(path.join(out, 'final-documents.json'), JSON.stringify(receipt, null, 2) + '\n');
const common = { PATH: '/run/current-system/sw/bin', HOME: path.join(out, 'home'),
  TMPDIR: path.join(out, 'tmp'), LANG: 'C.UTF-8', LC_ALL: 'C.UTF-8', XDG_RUNTIME_DIR: runtime };
function run(label, executable, argv, timeout = 3_600_000) {
  const tool = fs.realpathSync(executable), digest = hash(tool);
  if (!receipt.tools[tool]) receipt.tools[tool] = { sha256: digest };
  assert.equal(receipt.tools[tool].sha256, digest, `${label}: executable changed`);
  const stem = `commands/${String(receipt.commands.length + 1).padStart(3, '0')}-${label}`;
  const stdout = path.join(out, `${stem}.stdout`), stderr = path.join(out, `${stem}.stderr`);
  const outFd = fs.openSync(stdout, 'wx', 0o600), errFd = fs.openSync(stderr, 'wx', 0o600);
  const row = { label, executable: tool, executable_sha256: digest, argv, cwd: root,
    environment: common, stdout: path.relative(out, stdout), stderr: path.relative(out, stderr), timeout_ms: timeout };
  receipt.commands.push(row); save();
  let result;
  try { result = spawnSync(tool, argv, { cwd: root, env: common, stdio: ['ignore', outFd, errFd],
    timeout, killSignal: 'SIGKILL' }); }
  finally { fs.closeSync(outFd); fs.closeSync(errFd); }
  Object.assign(row, { status: result?.status ?? null, signal: result?.signal ?? null,
    error: result?.error ? String(result.error) : null, stdout_sha256: hash(stdout), stderr_sha256: hash(stderr) });
  for (const file of [stdout, stderr]) fs.chmodSync(file, 0o400);
  save();
  assert.equal(row.error, null, `${label}: launch failed`);
  assert.equal(row.signal, null, `${label}: command terminated`);
  assert.equal(row.status, 0, `${label}: command failed; see ${row.stderr}`);
  assert.equal(hash(tool), digest, `${label}: executable changed`);
  return fs.readFileSync(stdout, 'utf8');
}
function projectionUnchanged(allowGenerated = false) {
  const current = tracked();
  assert.deepEqual(current.filter(file => file !== generated), names,
    'tracked Git source set changed during final checks');
  if (!allowGenerated) assert.ok(!current.includes(generated), 'self-generated receipt appeared before publication');
  assert.deepEqual(gitSourceProjection(root, names, false), projection,
    'staged blob IDs/worktree source changed during final checks');
  for (const row of projection.rows) {
    const copy = path.join(source, row.path);
    const stat = fs.lstatSync(copy);
    assert.equal(stat.isSymbolicLink() ? 'symlink' : 'file', row.kind, `source type changed: ${row.path}`);
    const bytes = row.kind === 'symlink' ? Buffer.from(fs.readlinkSync(copy)) : fs.readFileSync(copy);
    assert.equal(sha(bytes), row.worktree_sha256, `copied source changed: ${row.path}`);
    if (row.kind === 'file') assert.equal(!!(stat.mode & 0o111), row.worktree_mode === '100755',
      `copied executable mode changed: ${row.path}`);
  }
}
try {
  const flags = ['--option', 'builders', '',
    '--option', 'substituters', 'https://cache.nixos.org https://nix-community.cachix.org',
    '--extra-experimental-features', 'nix-command flakes'];
  run('canonical-document-validation', bun,
    [path.join(root, 'tools/check-specs.mjs'), '--self-test', '--report'], 300_000);
  const fetched = JSON.parse(run('final-nix-source-prefetch', nix,
    [...flags, 'flake', 'prefetch', '--json', `path:${source}`], 600_000));
  assert.ok(typeof fetched.storePath === 'string' && typeof fetched.hash === 'string'
    && fetched.hash.startsWith('sha256-'), 'Nix did not materialize exact final source');
  const materialized = fs.realpathSync(fetched.storePath);
  for (const row of projection.rows) {
    const file = path.join(materialized, row.path);
    const stat = fs.lstatSync(file);
    assert.equal(stat.isSymbolicLink() ? 'symlink' : 'file', row.kind, `Nix changed source type: ${row.path}`);
    const bytes = row.kind === 'symlink' ? Buffer.from(fs.readlinkSync(file)) : fs.readFileSync(file);
    assert.equal(sha(bytes), row.worktree_sha256, `Nix omitted/rewrote staged source: ${row.path}`);
    if (row.kind === 'file') assert.equal(!!(stat.mode & 0o111), row.worktree_mode === '100755',
      `Nix changed executable mode: ${row.path}`);
  }
  receipt.nix_source = { path: materialized, nar_hash: fetched.hash,
    projection_sha256: projection.sha256, checked_files: names.length };
  for (const check of ['documents', 'cairn']) {
    const output = run(`final-nix-${check}`, nix,
      [...flags, 'build', '--no-link', '--print-out-paths',
        `path:${source}#checks.x86_64-linux.${check}`]).trim();
    assert.ok(output.startsWith('/nix/store/') && !output.includes('\n'), `${check}: missing Nix output`);
    assert.ok(fs.statSync(output).isDirectory(), `${check}: Nix derivation not built`);
    receipt.outputs[check] = output;
  }
  projectionUnchanged();
  assert.equal(hash(localReceipt), receipt.pre_promotion_assurance.sha256);
  assert.equal(hash(path.join(root, 'verification/m4/extraction-lock.json')), base.proof_lock.sha256);
  receipt.result = 'passed';
  const published = { ...receipt, publication: { path: generated,
    excluded_from_source_projection: true, git_action: `git add -- ${generated}` } };
  const bytes = JSON.stringify(published, null, 2) + '\n';
  fs.writeFileSync(path.join(root, generated), bytes, { flag: 'wx', mode: 0o444 });
  gitCall(['add', '--', generated]);
  assert.deepEqual(tracked().filter(file => file !== generated), names,
    'publishing final receipt changed a different tracked path');
  projectionUnchanged(true);
  receipt.publication = { path: generated, sha256: sha(bytes), staged: true,
    excluded_from_source_projection: true };
  receipt.after_publication_git_projection = gitSourceProjection(root, [...names, generated], false);
  assert.ok(receipt.after_publication_git_projection.rows.every(row => row.index));
} catch (error) {
  receipt.result = 'failed';
  receipt.integrity_failures.push(String(error.stack ?? error));
}
save();
console.log(JSON.stringify({ schema: receipt.schema, phase: receipt.phase, result: receipt.result,
  report: path.join(out, 'final-documents.json'), published: receipt.publication ?? null,
  output_checks: Object.keys(receipt.outputs), integrity_failures: receipt.integrity_failures }));
process.exitCode = receipt.result === 'passed' ? 0 : 1;
