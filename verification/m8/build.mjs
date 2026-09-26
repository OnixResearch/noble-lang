#!/usr/bin/env node
// Fresh M8 peer build, inheriting the complete current-workspace M7 build and
// retaining each new source/tool/binary binding. Build provenance is not an
// M8 execution, extraction, or proof verdict.
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const root = fs.realpathSync(path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..'));
assert.equal(process.argv.length, 3, 'usage: SELECTED_NODE verification/m8/build.mjs NEW_EXTERNAL_DIRECTORY');
const out = path.resolve(process.argv[2]);
assert.ok(out !== root && !out.startsWith(`${root}${path.sep}`), 'output must be outside repository');
assert.ok(!fs.existsSync(out), 'fresh output required');
assert.equal(fs.realpathSync(path.dirname(out)), path.dirname(out), 'output parent cannot be a symlink');
const selection = JSON.parse(fs.readFileSync(path.join(root, 'policy/tool-selection.json')));
const node = path.join(selection.tool_paths.node.output, 'bin/node');
assert.equal(fs.realpathSync(process.execPath), fs.realpathSync(node), 'policy-selected Node required');
assert.deepEqual(process.execArgv, [], 'unreviewed Node flags');
fs.mkdirSync(out);
for (const directory of ['commands', 'executables', 'home']) fs.mkdirSync(path.join(out, directory));
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const hash = file => sha(fs.readFileSync(file));
const relative = file => path.relative(out, file).split(path.sep).join('/');
const receipt = { schema: 'noble-m8-build/v1', result: 'running', source_root: root,
  artifact_directory: out, sources: {}, binaries: {}, commands: [], base: null,
  lock_derivation: null, integrity_failures: [],
  assumptions: ['Pinned Cargo/Rust/Node/Nix/vendor tools and binary provenance remain trusted; hashes alone prove no compiler or engine semantics.',
    'The inherited M7 build freshly compiles the full current workspace and previous peers, not M8 acceptance.'],
  non_claims: ['M8 runtime acceptance', 'universal projection/host/engine refinement', 'historical M7 re-acceptance'] };
const watched = [];
const save = () => fs.writeFileSync(path.join(out, 'build.json'), JSON.stringify(receipt, null, 2) + '\n');
function source(file, workspace) {
  const original = path.join(root, file);
  assert.ok(fs.lstatSync(original).isFile(), `not a plain source: ${file}`);
  const bytes = fs.readFileSync(original);
  const frozen = path.join(workspace, file);
  fs.mkdirSync(path.dirname(frozen), { recursive: true });
  if (fs.existsSync(frozen)) assert.equal(hash(frozen), sha(bytes), `base snapshot differs: ${file}`);
  else fs.writeFileSync(frozen, bytes, { flag: 'wx', mode: 0o400 });
  receipt.sources[file] = { sha256: sha(bytes), bytes: bytes.length, snapshot: path.relative(workspace, frozen) };
  watched.push({ file: original, sha256: sha(bytes) }, { file: frozen, sha256: sha(bytes) });
}
function tree(directory, workspace) {
  for (const item of fs.readdirSync(path.join(root, directory), { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name))) {
    const file = `${directory}/${item.name}`;
    if (item.isDirectory()) tree(file, workspace);
    else if (item.isFile()) source(file, workspace);
    else throw Error(`unreviewed source entry: ${file}`);
  }
}
function run(label, executable, argv, cwd, environment, timeout = 3_900_000) {
  const stem = `commands/${String(receipt.commands.length + 1).padStart(3, '0')}-${label}`;
  const stdout = path.join(out, `${stem}.stdout`), stderr = path.join(out, `${stem}.stderr`);
  const outFd = fs.openSync(stdout, 'wx', 0o600), errFd = fs.openSync(stderr, 'wx', 0o600);
  const row = { label, executable, executable_sha256: hash(executable), argv, cwd,
    stdout: relative(stdout), stderr: relative(stderr), timeout_ms: timeout };
  receipt.commands.push(row);
  let result;
  try { result = spawnSync(executable, argv, { cwd, env: environment, stdio: ['ignore', outFd, errFd],
    timeout, killSignal: 'SIGKILL' }); }
  finally { fs.closeSync(outFd); fs.closeSync(errFd); }
  Object.assign(row, { status: result?.status ?? null, signal: result?.signal ?? null,
    error: result?.error ? String(result.error) : null, stdout_sha256: hash(stdout), stderr_sha256: hash(stderr) });
  for (const file of [stdout, stderr]) fs.chmodSync(file, 0o400);
  assert.equal(row.error, null, `${label}: launch failure`);
  assert.equal(row.signal, null, `${label}: terminated by signal`);
  assert.equal(row.status, 0, `${label}: command failure at ${row.stderr}`);
  assert.equal(hash(executable), row.executable_sha256, `${label}: binary changed`);
  return row;
}
try {
  const baseDirectory = path.join(out, 'm7');
  run('fresh-full-workspace-m7-build', node,
    [path.join(root, 'verification/m7/build.mjs'), baseDirectory], root,
    { PATH: '/run/current-system/sw/bin', HOME: path.join(out, 'home'), LANG: 'C.UTF-8', LC_ALL: 'C.UTF-8', NODE_OPTIONS: '' });
  const baseFile = path.join(baseDirectory, 'build.json');
  const base = JSON.parse(fs.readFileSync(baseFile));
  assert.equal(base.schema, 'noble-m7-build/v1');
  assert.equal(base.result, 'built');
  assert.deepEqual(base.integrity_failures, []);
  assert.ok(base.base?.source_revision && base.binaries?.m7_peer?.path);
  for (const name of ['cli', 'm5_peer', 'peer', 'task_tests', 'resource_tests', 'authority_tests', 'm7_peer', 'mc2_tests']) {
    const binary = base.binaries[name];
    assert.ok(binary?.path && binary?.sha256, `missing inherited binary: ${name}`);
    assert.equal(hash(binary.path), binary.sha256, `${name}: inherited binary changed`);
    receipt.binaries[name] = { path: binary.path, sha256: binary.sha256, source: 'fresh-m7-build' };
  }
  assert.ok(receipt.binaries.cli && receipt.binaries.m7_peer, 'missing inherited CLI or M7 peer');
  receipt.base = { receipt: relative(baseFile), sha256: hash(baseFile), source_revision: base.source_revision,
    full_workspace_source_revision: base.base.source_revision };
  const workspace = path.join(baseDirectory, 'base/workspace');
  assert.ok(fs.statSync(workspace).isDirectory(), 'inherited full-workspace snapshot missing');
  tree('verification/m8', workspace);
  tree('nix', workspace);
  tree('.cairn/changes/m8-choreography-projection', workspace);
  for (const file of ['specs/conformance/safety-cases.json', 'specs/conformance/wit-wasi-cases.json',
    '.cairn/specs/safety/spec.md', '.cairn/specs/wit-wasi/spec.md', 'policy/tool-selection.json',
    'policy/architecture.ncl', 'policy/source-inventory.json', 'verification/m4/extraction-lock.json',
    'verification/m7/peer/Cargo.lock']) source(file, workspace);
  // The M8 peer has its own source/manifest, with the exact dependency graph
  // pinned by the inherited M7 Wasmtime lock. Only the local package name differs.
  const priorLock = fs.readFileSync(path.join(workspace, 'verification/m7/peer/Cargo.lock'), 'utf8');
  const marker = 'name = "noble-m7-peer"';
  assert.equal(priorLock.split(marker).length, 2, 'unique prior peer package lock required');
  const nextLock = priorLock.replace(marker, 'name = "noble-m8-peer"');
  const newLock = path.join(workspace, 'verification/m8/peer/Cargo.lock');
  fs.writeFileSync(newLock, nextLock, { flag: 'wx', mode: 0o400 });
  watched.push({ file: newLock, sha256: sha(nextLock) });
  receipt.lock_derivation = { source: 'verification/m7/peer/Cargo.lock', source_sha256: sha(priorLock),
    transformation: 'replace unique local package noble-m7-peer with noble-m8-peer',
    derived_sha256: sha(nextLock), snapshot: path.relative(workspace, newLock) };
  const inherited = JSON.parse(fs.readFileSync(path.join(baseDirectory, 'base/build.json')));
  const pins = JSON.parse(fs.readFileSync(path.join(workspace, 'verification/m6/pins.json')));
  const peerManifest = fs.readFileSync(path.join(workspace, 'verification/m8/peer/Cargo.toml'), 'utf8');
  assert.ok(peerManifest.includes(`path = "${pins.wasmtime_source}/crates/wasmtime"`),
    'independent peer must use policy-selected Wasmtime source');
  const vendor = ['--config', 'source.crates-io.replace-with="m6-vendor"', '--config',
    `source.m6-vendor.directory="${pins.vendor}/source-registry-0"`];
  const cargo = inherited.tools.cargo.real;
  const environment = { ...inherited.environment, CARGO_TARGET_DIR: path.join(out, 'm8-peer-target') };
  const built = run('m8-independent-peer-build', cargo,
    ['build', '--manifest-path', 'verification/m8/peer/Cargo.toml', '--locked', '--offline',
      '--message-format=json', ...vendor], workspace, environment);
  run('m8-independent-peer-clippy', cargo,
    ['clippy', '--manifest-path', 'verification/m8/peer/Cargo.toml', '--all-targets',
      '--locked', '--offline', ...vendor, '--', '-D', 'warnings'], workspace, environment);
  const rows = fs.readFileSync(path.join(out, built.stdout), 'utf8').split('\n')
    .filter(line => line.startsWith('{')).map(line => JSON.parse(line));
  const paths = [...new Set(rows.filter(row => row.reason === 'compiler-artifact'
    && row.target?.name === 'noble-m8-peer' && row.target.kind?.includes('bin') && !row.profile?.test)
    .map(row => row.executable).filter(Boolean))];
  assert.equal(paths.length, 1, 'exactly one compiler-reported independent M8 peer binary required');
  const binary = fs.readFileSync(paths[0]);
  const peer = path.join(out, 'executables/m8-peer');
  fs.writeFileSync(peer, binary, { flag: 'wx', mode: 0o500 });
  watched.push({ file: paths[0], sha256: sha(binary) }, { file: peer, sha256: sha(binary) });
  receipt.binaries.m8_peer = { path: peer, sha256: sha(binary), bytes: binary.length,
    source: 'independent-current-source-cargo-build', command: built.label };
  receipt.source_revision = `sha256:${sha(JSON.stringify({ inherited: base.source_revision,
    m8: Object.entries(receipt.sources).map(([file, value]) => [file, value.sha256]).sort(),
    peer_lock: receipt.lock_derivation.derived_sha256 }))}`;
  receipt.result = 'built';
} catch (error) { receipt.result = 'failed'; receipt.failure = String(error.stack ?? error); }
finally {
  for (const entry of watched) try { assert.equal(hash(entry.file), entry.sha256); }
  catch (error) { receipt.integrity_failures.push({ file: entry.file, failure: String(error) }); }
  if (receipt.integrity_failures.length) receipt.result = 'failed';
  save();
  console.log(JSON.stringify({ schema: receipt.schema, result: receipt.result, report: path.join(out, 'build.json'),
    source_revision: receipt.source_revision ?? null }));
  process.exitCode = receipt.result === 'built' ? 0 : 1;
}
