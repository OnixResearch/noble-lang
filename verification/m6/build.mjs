#!/usr/bin/env node
// Reproducible build provenance only. Acceptance and earlier regressions run separately.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const root = fs.realpathSync(path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..'));
const [destination, ...options] = process.argv.slice(2);
assert.ok(destination && (options.length === 0 || (options.length === 1 && options[0] === '--with-m5-peer')),
  'usage: SELECTED_NODE verification/m6/build.mjs NEW_ARTIFACT_DIRECTORY [--with-m5-peer]');
const output = path.resolve(destination);
assert.ok(output !== root && !output.startsWith(`${root}/`), 'use a fresh directory outside the repository');
assert.ok(!fs.existsSync(output), 'build artifact directory must not exist');
assert.equal(fs.realpathSync(path.dirname(output)), path.dirname(output), 'artifact parent must not contain symlinks');
fs.mkdirSync(output);
for (const directory of ['workspace', 'commands', 'executables', 'home', 'cargo-home']) fs.mkdirSync(path.join(output, directory));
const workspace = path.join(output, 'workspace');
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const fileHash = file => hash(fs.readFileSync(file));
const relative = file => path.relative(output, file).split(path.sep).join('/');
const receipt = {
  schema: 'noble-m6-build/v1', result: 'running', source_root: root, artifact_directory: output,
  engine: { executable: process.execPath, versions: process.versions, flags: process.execArgv },
  sources: {}, tools: {}, commands: [], binaries: {}, source_trees: [], integrity_failures: [],
  configuration: { workspace: ['--workspace', '--all-targets', '--all-features', '--locked', '--offline'], with_m5_peer: options.length === 1 },
  assumptions: ['The selected Rust, linker, Node, Nix, immutable engine/vendor closures and this driver are trusted build tooling.',
    'Source snapshots and Cargo artifact transcripts bind this build; neither a hash nor a successful compilation proves compiler correctness.'],
  non_claims: ['M6 acceptance execution', 'M5 or earlier regression execution', 'proof extraction or Nix check execution', 'universal compiler or engine refinement'],
};
const monitored = [];
let environment;
function save() { fs.writeFileSync(path.join(output, 'build.json'), JSON.stringify(receipt, null, 2) + '\n'); }
function source(file) {
  if (Object.hasOwn(receipt.sources, file)) return;
  const input = path.join(root, file);
  assert.ok(fs.lstatSync(input).isFile(), `not a regular source: ${file}`);
  const bytes = fs.readFileSync(input);
  const target = path.join(workspace, file);
  fs.mkdirSync(path.dirname(target), { recursive: true });
  fs.writeFileSync(target, bytes, { flag: 'wx', mode: 0o400 });
  receipt.sources[file] = { sha256: hash(bytes), bytes: bytes.length, snapshot: relative(target) };
  monitored.push({ file: input, sha256: hash(bytes) }, { file: target, sha256: hash(bytes) });
}
function entries(directory) {
  const names = [];
  function walk(prefix) {
    for (const entry of fs.readdirSync(path.join(root, prefix), { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name))) {
      if (['target', '.git', '.lake', 'node_modules'].includes(entry.name)) continue;
      const file = `${prefix}/${entry.name}`;
      assert.equal(entry.isSymbolicLink(), false, `source symlink: ${file}`);
      if (entry.isDirectory()) walk(file);
      else { assert.ok(entry.isFile(), `non-file source: ${file}`); names.push(file); }
    }
  }
  walk(directory);
  return names.sort();
}
function tree(directory) {
  const files = entries(directory);
  receipt.source_trees.push({ directory, files });
  files.forEach(source);
}
function tool(name, file) {
  const real = fs.realpathSync(file);
  assert.ok(fs.statSync(real).isFile(), `missing build executable ${name}`);
  receipt.tools[name] = { supplied: file, real, sha256: fileHash(real) };
  monitored.push({ file: real, sha256: fileHash(real) });
  return real;
}
function run(label, executable, args, extraEnv = {}, timeout = 1_800_000) {
  const id = receipt.commands.length + 1;
  const stem = `commands/${String(id).padStart(3, '0')}-${label}`;
  const stdoutPath = `${stem}.stdout`, stderrPath = `${stem}.stderr`, stdinPath = `${stem}.stdin`;
  fs.writeFileSync(path.join(output, stdinPath), '', { flag: 'wx', mode: 0o400 });
  const stdout = fs.openSync(path.join(output, stdoutPath), 'wx', 0o600);
  const stderr = fs.openSync(path.join(output, stderrPath), 'wx', 0o600);
  const row = { id, label, executable, executable_sha256: fileHash(executable), args, cwd: workspace,
    environment: { ...environment, ...extraEnv }, stdin_path: stdinPath, stdout_path: stdoutPath, stderr_path: stderrPath, timeout_ms: timeout };
  receipt.commands.push(row); save();
  let result;
  try { result = spawnSync(executable, args, { cwd: workspace, env: row.environment, stdio: ['ignore', stdout, stderr], timeout, killSignal: 'SIGKILL' }); }
  catch (error) { row.error = String(error); }
  finally { fs.closeSync(stdout); fs.closeSync(stderr); }
  Object.assign(row, { status: result?.status ?? null, signal: result?.signal ?? null, error: row.error ?? result?.error?.message ?? null,
    stdin_sha256: fileHash(path.join(output, stdinPath)), stdout_sha256: fileHash(path.join(output, stdoutPath)), stderr_sha256: fileHash(path.join(output, stderrPath)) });
  for (const file of [stdoutPath, stderrPath]) fs.chmodSync(path.join(output, file), 0o400);
  save();
  assert.equal(row.error, null, `${label}: host execution failed`);
  assert.equal(row.signal, null, `${label}: terminated`);
  assert.equal(row.status, 0, `${label}: see ${stderrPath}`);
  assert.equal(fileHash(executable), row.executable_sha256, `${label}: executable changed`);
  return row;
}
function text(row) { return fs.readFileSync(path.join(output, row.stdout_path), 'utf8'); }
function cargoArtifact(row, name, kind) {
  const records = text(row).split('\n').filter(line => line.startsWith('{')).map(line => JSON.parse(line));
  const candidates = records.filter(item => item.reason === 'compiler-artifact' && item.target.name === name && item.target.kind.includes(kind)
    && (kind !== 'bin' || !item.profile.test));
  const paths = [...new Set(candidates.map(item => item.executable).filter(Boolean))];
  assert.equal(paths.length, 1, `expected one Cargo-reported executable for ${name}/${kind}`);
  return { path: paths[0], command: row.id, target: name, kind, artifact: candidates.find(item => item.executable === paths[0]) };
}
function binary(name, artifact) {
  const bytes = fs.readFileSync(artifact.path);
  const frozen = path.join(output, 'executables', name);
  fs.writeFileSync(frozen, bytes, { flag: 'wx', mode: 0o500 });
  receipt.binaries[name] = { path: frozen, snapshot: relative(frozen), sha256: hash(bytes), bytes: bytes.length,
    compiled_path: artifact.path, command: artifact.command, target: artifact.target, kind: artifact.kind, cargo_artifact: artifact.artifact };
  monitored.push({ file: frozen, sha256: hash(bytes) }, { file: artifact.path, sha256: hash(bytes) });
}
try {
  for (let parent = output;; parent = path.dirname(parent)) {
    for (const name of ['config', 'config.toml']) assert.equal(fs.existsSync(path.join(parent, '.cargo', name)), false, `unreviewed inherited Cargo configuration: ${parent}`);
    if (path.dirname(parent) === parent) break;
  }
  for (const directory of ['crates', 'tools', 'verification/m6/peer/src', 'verification/m6/cases', 'verification/m5/peer/src', 'verification/mc2/contracts']) tree(directory);
  if (fs.existsSync(path.join(root, '.cargo'))) tree('.cargo');
  for (const file of ['Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', 'flake.nix', 'flake.lock', 'policy/tool-selection.json',
    'verification/m6/pins.json', 'verification/m6/build.mjs', 'verification/m6/gate.mjs', 'verification/m6/peer/Cargo.toml', 'verification/m6/peer/Cargo.lock',
    'verification/m5/pins.json', 'verification/m5/peer/Cargo.toml', 'verification/m5/peer/Cargo.lock',
    'verification/mc1/monotonic.noble-contract', 'verification/mc1/monotonic-refutation.lean',
    'specs/conformance/wit-wasi-cases.json', 'specs/conformance/worker-cases.json']) source(file);
  const pins = JSON.parse(fs.readFileSync(path.join(workspace, 'verification/m6/pins.json')));
  const selection = JSON.parse(fs.readFileSync(path.join(workspace, 'policy/tool-selection.json')));
  receipt.pins = pins;
  const rust = path.join(selection.tool_paths.quality_rust.output, 'bin');
  const cargo = tool('cargo', path.join(rust, 'cargo'));
  const rustc = tool('rustc', path.join(rust, 'rustc'));
  const rustdoc = tool('rustdoc', path.join(rust, 'rustdoc'));
  const node = tool('node', path.join(selection.tool_paths.node.output, 'bin/node'));
  const nix = tool('nix', pins.nix ?? '/run/current-system/sw/bin/nix');
  tool('linker', path.join(pins.linker_bin, 'cc'));
  tool('wasm_tools', path.join(selection.tool_paths.wasm_tools.output, 'bin/wasm-tools'));
  assert.equal(fileHash(process.execPath), fileHash(node), 'build driver must run with selected Node');
  assert.deepEqual(process.execArgv, [], 'unreviewed Node flags');
  environment = { PATH: `${rust}:${pins.linker_bin}:/run/current-system/sw/bin`, HOME: path.join(output, 'home'),
    CARGO_HOME: path.join(output, 'cargo-home'), CARGO_TARGET_DIR: path.join(output, 'target'),
    RUSTC: rustc, RUSTDOC: rustdoc, RUSTC_WRAPPER: '', RUSTC_WORKSPACE_WRAPPER: '', RUSTFLAGS: '', CARGO_ENCODED_RUSTFLAGS: '',
    CARGO_INCREMENTAL: '0', LANG: 'C.UTF-8', LC_ALL: 'C.UTF-8' };
  receipt.environment = environment;
  receipt.selection = { quality_rust: selection.configuration.quality_rust, quality_rust_output: selection.tool_paths.quality_rust.output,
    node: selection.verification_tools.node.version, wasm_tools: selection.verification_tools.wasm_tools.version };
  receipt.source_revision = `sha256:${hash(JSON.stringify(Object.entries(receipt.sources).map(([file, value]) => [file, value.sha256]).sort()))}`;
  run('rustc-version', rustc, ['--version', '--verbose']);
  run('cargo-version', cargo, ['--version', '--verbose']);
  const closures = [['wasmtime-source', pins.wasmtime_source, pins.wasmtime_source_nar_hash], ['wasmtime-vendor', pins.vendor, pins.vendor_nar_hash],
    ...(pins.extra_sources ?? []).map(item => [item.name, item.path, item.nar_hash])];
  assert.equal(new Set(closures.map(([name]) => name)).size, closures.length, 'duplicate closure pin name');
  for (const match of fs.readFileSync(path.join(workspace, 'verification/m6/peer/Cargo.toml'), 'utf8').matchAll(/path\s*=\s*"(\/nix\/store\/[^"]+)"/g)) {
    assert.ok(closures.some(([, directory]) => match[1] === directory || match[1].startsWith(`${directory}/`)), `unbound compiled dependency: ${match[1]}`);
  }
  for (const [label, directory, expected] of closures) {
    assert.match(directory, /^\/nix\/store\//, `unpinned ${label}`);
    assert.match(expected, /^sha256-/, `missing NAR pin for ${label}`);
    const observed = run(label, nix, ['--extra-experimental-features', 'nix-command', 'hash', 'path', '--type', 'sha256', '--sri', directory]);
    assert.equal(text(observed).trim(), expected, `source closure changed: ${label}`);
  }
  const vendor = ['--config', 'source.crates-io.replace-with="m6-vendor"', '--config', `source.m6-vendor.directory="${pins.vendor}/source-registry-0"`];
  const compiled = run('workspace-build', cargo, ['build', ...receipt.configuration.workspace, '--message-format=json', ...vendor]);
  const peer = run('m6-peer-build', cargo, ['build', '--manifest-path', 'verification/m6/peer/Cargo.toml', '--locked', '--offline', '--message-format=json', ...vendor],
    { CARGO_TARGET_DIR: path.join(output, 'm6-peer-target') });
  binary('cli', cargoArtifact(compiled, 'noble', 'bin'));
  binary('task_tests', cargoArtifact(compiled, 'm6-async', 'test'));
  binary('peer', cargoArtifact(peer, 'noble-m6-peer', 'bin'));
  if (receipt.configuration.with_m5_peer) {
    const previous = JSON.parse(fs.readFileSync(path.join(workspace, 'verification/m5/pins.json')));
    assert.equal(previous.vendor, pins.vendor, 'M5 vendor must be separately reviewed if different');
    assert.equal(previous.wasmtime_source, pins.wasmtime_source, 'M5 engine must be separately reviewed if different');
    const built = run('m5-peer-build', cargo, ['build', '--manifest-path', 'verification/m5/peer/Cargo.toml', '--locked', '--offline', '--message-format=json', ...vendor],
      { CARGO_TARGET_DIR: path.join(output, 'm5-peer-target') });
    binary('m5_peer', cargoArtifact(built, 'noble-m5-peer', 'bin'));
    binary('resource_tests', cargoArtifact(compiled, 'm5-resources', 'test'));
    binary('authority_tests', cargoArtifact(compiled, 'm5-authority', 'test'));
  }
  receipt.result = 'built';
} catch (error) {
  receipt.result = 'failed'; receipt.failure = String(error.stack ?? error);
} finally {
  for (const binding of monitored) {
    try { assert.equal(fileHash(binding.file), binding.sha256); }
    catch (error) { receipt.integrity_failures.push({ file: binding.file, error: String(error) }); }
  }
  for (const inventory of receipt.source_trees) {
    try { assert.deepEqual(entries(inventory.directory), inventory.files); }
    catch (error) { receipt.integrity_failures.push({ directory: inventory.directory, error: String(error) }); }
  }
  if (receipt.integrity_failures.length) receipt.result = 'failed';
  save();
  console.log(JSON.stringify({ schema: receipt.schema, result: receipt.result, report: path.join(output, 'build.json'), binaries: receipt.binaries }));
  process.exitCode = receipt.result === 'built' ? 0 : 1;
}
