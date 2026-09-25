#!/usr/bin/env node
// Extend a fresh M6 full-workspace/previous-peer build with the independent M7
// Wasmtime peer. The base receipt remains a build provenance lane, not M7
// extraction, execution, proof, or historical milestone re-acceptance.
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const root = fs.realpathSync(path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..'));
const args = process.argv.slice(2);
assert.equal(args.length, 1, 'usage: SELECTED_NODE verification/m7/build.mjs NEW_EXTERNAL_DIRECTORY');
const out = path.resolve(args[0]);
assert.ok(out !== root && !out.startsWith(`${root}${path.sep}`), 'build output must be outside the repository');
assert.ok(!fs.existsSync(out), 'build output must be new');
assert.equal(fs.realpathSync(path.dirname(out)), path.dirname(out), 'artifact parent must not be a symlink');
fs.mkdirSync(out);
for (const name of ['commands', 'executables']) fs.mkdirSync(path.join(out, name));
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const fileHash = file => hash(fs.readFileSync(file));
const relative = file => path.relative(out, file).split(path.sep).join('/');
const receipt = { schema: 'noble-m7-build/v1', result: 'running', source_root: root,
  artifact_directory: out, node: { executable: process.execPath, sha256: fileHash(process.execPath), version: process.version },
  base: null, opacity: null, sources: {}, commands: [], binaries: {}, integrity_failures: [],
  assumptions: [
    'The pinned Rust, Nix closures, Node and Cargo build tooling remain trusted; source/binary hashes are provenance, not extraction or proof.',
    'The base M6 build snapshots the complete current root workspace and builds CLI, M5/M6 peers and native tests, but does not accept M7 or rerun earlier milestones.',
    'This script snapshots the separately pinned M7 independent peer source and builds it against the same root source snapshot and vendor selection.',
  ],
  non_claims: ['M7 acceptance or any historical milestone acceptance', 'strict M7 Charon/Aeneas/Lean proof',
    'physical host/engine correctness', 'root workspace Octet or Nix quality gates'],
};
const monitored = [];
const save = () => fs.writeFileSync(path.join(out, 'build.json'), JSON.stringify(receipt, null, 2) + '\n');
function source(file, workspace) {
  const original = path.join(root, file);
  assert.ok(fs.lstatSync(original).isFile(), `missing plain source file: ${file}`);
  const bytes = fs.readFileSync(original);
  const frozen = path.join(workspace, file);
  fs.mkdirSync(path.dirname(frozen), { recursive: true });
  if (fs.existsSync(frozen)) assert.equal(fileHash(frozen), hash(bytes), `base snapshot differs: ${file}`);
  else fs.writeFileSync(frozen, bytes, { flag: 'wx', mode: 0o400 });
  receipt.sources[file] = { sha256: hash(bytes), bytes: bytes.length, snapshot: path.relative(workspace, frozen) };
  monitored.push({ file: original, sha256: hash(bytes) }, { file: frozen, sha256: hash(bytes) });
}
function sourceTree(directory, workspace) {
  for (const item of fs.readdirSync(path.join(root, directory), { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name))) {
    if (['target', '.git', 'node_modules'].includes(item.name)) continue;
    const file = `${directory}/${item.name}`;
    if (item.isDirectory()) sourceTree(file, workspace);
    else if (item.isFile()) source(file, workspace);
    else throw Error(`unreviewed M7 peer source entry: ${file}`);
  }
}
function run(label, executable, argv, cwd, env, timeout = 1_800_000) {
  const stem = `commands/${String(receipt.commands.length + 1).padStart(3, '0')}-${label}`;
  const stdout = path.join(out, `${stem}.stdout`);
  const stderr = path.join(out, `${stem}.stderr`);
  const outFd = fs.openSync(stdout, 'wx', 0o600);
  const errFd = fs.openSync(stderr, 'wx', 0o600);
  const row = { label, executable, executable_sha256: fileHash(executable), argv, cwd, environment: env,
    timeout_ms: timeout, stdout: relative(stdout), stderr: relative(stderr) };
  receipt.commands.push(row);
  save();
  let result;
  try { result = spawnSync(executable, argv, { cwd, env, stdio: ['ignore', outFd, errFd], timeout, killSignal: 'SIGKILL' }); }
  finally { fs.closeSync(outFd); fs.closeSync(errFd); }
  Object.assign(row, { status: result?.status ?? null, signal: result?.signal ?? null,
    error: result?.error ? String(result.error) : null, stdout_sha256: fileHash(stdout), stderr_sha256: fileHash(stderr) });
  fs.chmodSync(stdout, 0o400); fs.chmodSync(stderr, 0o400);
  save();
  assert.equal(row.error, null, `${label} host failure`);
  assert.equal(row.signal, null, `${label} signal`);
  assert.equal(row.status, 0, `${label} failed: ${row.stderr}`);
  assert.equal(fileHash(executable), row.executable_sha256, `${label} executable changed`);
  return row;
}
try {
  const selection = JSON.parse(fs.readFileSync(path.join(root, 'policy/tool-selection.json')));
  const selectedNode = path.join(selection.tool_paths.node.output, 'bin/node');
  assert.equal(fs.realpathSync(process.execPath), fs.realpathSync(selectedNode), 'use the policy-selected Node');
  assert.deepEqual(process.execArgv, [], 'unreviewed Node flags');
  const baseDir = path.join(out, 'base');
  fs.mkdirSync(path.join(out, 'home'));
  run('full-workspace-and-inherited-peer-build', process.execPath,
    [path.join(root, 'verification/m6/build.mjs'), baseDir, '--with-m5-peer'], root,
    { PATH: '/run/current-system/sw/bin', HOME: path.join(out, 'home'),
      LANG: 'C.UTF-8', LC_ALL: 'C.UTF-8', NODE_OPTIONS: '' }, 3_600_000);
  const baseFile = path.join(baseDir, 'build.json');
  const base = JSON.parse(fs.readFileSync(baseFile));
  assert.equal(base.schema, 'noble-m6-build/v1');
  assert.equal(base.result, 'built');
  assert.equal(base.integrity_failures.length, 0);
  assert.equal(base.environment.CARGO_TARGET_DIR, path.join(baseDir, 'target'));
  const required = ['cli', 'm5_peer', 'peer', 'task_tests', 'resource_tests', 'authority_tests'];
  for (const name of required) {
    const binary = base.binaries[name];
    assert.ok(binary?.path && binary?.sha256, `missing inherited build binary: ${name}`);
    assert.equal(fileHash(binary.path), binary.sha256);
    receipt.binaries[name] = { path: binary.path, sha256: binary.sha256, source: 'fresh-current-workspace-base-build' };
  }
  receipt.base = { receipt: relative(baseFile), receipt_sha256: fileHash(baseFile),
    source_revision: base.source_revision, cargo_lock_sha256: base.sources['Cargo.lock']?.sha256 };
  assert.ok(receipt.base.source_revision && receipt.base.cargo_lock_sha256);
  const workspace = path.join(baseDir, 'workspace');
  sourceTree('verification/m7/peer', workspace);
  for (const file of ['verification/m7/build.mjs', 'verification/m7/gate.mjs',
    'specs/conformance/safety-cases.json', 'specs/conformance/wit-wasi-cases.json',
    '.cairn/specs/safety/spec.md', '.cairn/specs/wit-wasi/spec.md']) source(file, workspace);
  receipt.source_revision = `sha256:${hash(JSON.stringify({ base: base.source_revision,
    m7: Object.entries(receipt.sources).map(([file, value]) => [file, value.sha256]).sort() }))}`;
  const pins = JSON.parse(fs.readFileSync(path.join(workspace, 'verification/m6/pins.json')));
  const dependencies = fs.readFileSync(path.join(workspace, 'verification/m7/peer/Cargo.toml'), 'utf8');
  for (const match of dependencies.matchAll(/path\s*=\s*"(\/nix\/store\/[^\"]+)"/g)) {
    assert.ok(match[1].startsWith(`${pins.wasmtime_source}/`) || match[1] === pins.vendor,
      `M7 peer dependency outside reviewed M6 source/vendor closures: ${match[1]}`);
  }
  const cargo = base.tools.cargo.real;
  const vendor = ['--config', 'source.crates-io.replace-with="m6-vendor"', '--config', `source.m6-vendor.directory="${pins.vendor}/source-registry-0"`];
  const environment = { ...base.environment, CARGO_TARGET_DIR: path.join(out, 'm7-peer-target') };
  const built = run('m7-independent-peer-build', cargo,
    ['build', '--manifest-path', 'verification/m7/peer/Cargo.toml', '--locked', '--offline',
      '--message-format=json', ...vendor], workspace, environment);
  const rows = fs.readFileSync(path.join(out, built.stdout), 'utf8').split('\n').filter(line => line.startsWith('{')).map(line => JSON.parse(line));
  const paths = [...new Set(rows.filter(row => row.reason === 'compiler-artifact' && row.target?.name === 'noble-m7-peer'
    && row.target.kind?.includes('bin') && !row.profile?.test).map(row => row.executable).filter(Boolean))];
  assert.equal(paths.length, 1, 'exactly one Cargo-reported M7 peer binary required');
  const bytes = fs.readFileSync(paths[0]);
  const peer = path.join(out, 'executables/m7-peer');
  fs.writeFileSync(peer, bytes, { flag: 'wx', mode: 0o500 });
  receipt.binaries.m7_peer = { path: peer, sha256: hash(bytes), bytes: bytes.length, command: built.label,
    compiled_path: paths[0], kind: 'bin', target: 'noble-m7-peer' };
  monitored.push({ file: peer, sha256: hash(bytes) }, { file: paths[0], sha256: hash(bytes) });
  const core = run('mc2-cli-core-control-test-build', cargo,
    ['test', '-p', 'noble-cli', '--bin', 'noble', '--no-run', '--locked', '--offline',
      '--message-format=json', ...vendor], workspace, base.environment);
  const tests = fs.readFileSync(path.join(out, core.stdout), 'utf8').split('\n').filter(line => line.startsWith('{')).map(line => JSON.parse(line));
  const controls = [...new Set(tests.filter(row => row.reason === 'compiler-artifact' && row.target?.name === 'noble'
    && row.target.kind?.includes('bin') && row.profile?.test).map(row => row.executable).filter(Boolean))];
  assert.equal(controls.length, 1, 'exactly one Cargo-reported MC2 CLI test binary required');
  const controlBytes = fs.readFileSync(controls[0]);
  const control = path.join(out, 'executables/mc2-core-control-tests');
  fs.writeFileSync(control, controlBytes, { flag: 'wx', mode: 0o500 });
  receipt.binaries.mc2_tests = { path: control, sha256: hash(controlBytes), bytes: controlBytes.length, command: core.label,
    compiled_path: controls[0], kind: 'bin/test', target: 'noble' };
  monitored.push({ file: control, sha256: hash(controlBytes) }, { file: controls[0], sha256: hash(controlBytes) });
  const m5Dir = path.join(out, 'm5-opacity');
  run('m5-source-bound-opacity-build', process.execPath,
    [path.join(root, 'verification/m5/build.mjs'), m5Dir], root,
    { PATH: '/run/current-system/sw/bin', HOME: path.join(out, 'home'),
      LANG: 'C.UTF-8', LC_ALL: 'C.UTF-8', NODE_OPTIONS: '' }, 3_600_000);
  const m5File = path.join(m5Dir, 'build.json');
  const m5Build = JSON.parse(fs.readFileSync(m5File));
  const opacityFile = path.join(m5Dir, 'opacity.json');
  const opacity = JSON.parse(fs.readFileSync(opacityFile));
  assert.equal(m5Build.schema, 'noble-m5-build/v1');
  assert.equal(m5Build.result, 'passed');
  assert.equal(opacity.schema, 'noble-m5-opacity/v1');
  assert.equal(opacity.passed, true);
  assert.deepEqual(opacity.source_sha256, m5Build.sources, 'opacity must bind the actual nested M5 build sources');
  const lock = JSON.parse(fs.readFileSync(path.join(root, 'verification/m4/extraction-lock.json')));
  let lockedSources = 0;
  let baseSources = 0;
  for (const [file, digest] of Object.entries(opacity.source_sha256)) {
    assert.equal(fileHash(path.join(root, file)), digest, `M5 opacity source changed: ${file}`);
    if (lock.source_files[file]) {
      assert.equal(lock.source_files[file], digest, `M5 opacity differs from independently reviewed source: ${file}`);
      lockedSources++;
    }
    if (base.sources[file]) {
      assert.equal(base.sources[file].sha256, digest, `M5 opacity differs from compiled base source: ${file}`);
      baseSources++;
    }
  }
  assert.ok(lockedSources >= 390 && baseSources >= 350, 'M5 opacity must cover reviewed and compiled Rust sources');
  receipt.opacity = { report: relative(opacityFile), sha256: fileHash(opacityFile),
    build_receipt: relative(m5File), build_receipt_sha256: fileHash(m5File),
    source_count: Object.keys(opacity.source_sha256).length, locked_sources: lockedSources, base_sources: baseSources };
  receipt.result = 'built';
} catch (error) {
  receipt.result = 'failed'; receipt.failure = String(error.stack ?? error);
} finally {
  for (const binding of monitored) {
    try { assert.equal(fileHash(binding.file), binding.sha256); }
    catch (error) { receipt.integrity_failures.push({ file: binding.file, failure: String(error) }); }
  }
  if (receipt.integrity_failures.length) receipt.result = 'failed';
  save();
  console.log(JSON.stringify({ schema: receipt.schema, result: receipt.result, report: path.join(out, 'build.json'),
    source_revision: receipt.base?.source_revision ?? null, m7_peer_sha256: receipt.binaries.m7_peer?.sha256 ?? null }));
  process.exitCode = receipt.result === 'built' ? 0 : 1;
}
