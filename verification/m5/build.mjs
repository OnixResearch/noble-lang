#!/usr/bin/env node
// Build provenance and real Rust opacity controls; acceptance is a separate gate.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const [destination, ...extra] = process.argv.slice(2);
assert.ok(destination && extra.length === 0, 'usage: SELECTED_NODE verification/m5/build.mjs NEW_ARTIFACT_DIRECTORY');
const output = path.resolve(destination);
assert.ok(output !== root && !output.startsWith(`${root}/`), 'use a fresh directory outside the repository');
for (let parent = output;; parent = path.dirname(parent)) {
  for (const name of ['config', 'config.toml']) assert.equal(fs.existsSync(path.join(parent, '.cargo', name)), false, `unreviewed Cargo configuration: ${parent}`);
  if (path.dirname(parent) === parent) break;
}
fs.mkdirSync(output);
const workspace = path.join(output, 'workspace');
fs.mkdirSync(workspace);
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const fileHash = file => hash(fs.readFileSync(file));
const save = (file, value) => fs.writeFileSync(path.join(output, file), JSON.stringify(value, null, 2) + '\n');
const pins = JSON.parse(fs.readFileSync(path.join(root, 'verification/m5/pins.json')));
const selection = JSON.parse(fs.readFileSync(path.join(root, 'policy/tool-selection.json')));
const rust = path.join(selection.tool_paths.quality_rust.output, 'bin');
const cargo = path.join(rust, 'cargo');
const rustdoc = path.join(rust, 'rustdoc');
const home = path.join(output, 'home');
fs.mkdirSync(home);
const environment = { PATH: `${rust}:${pins.linker_bin}:/run/current-system/sw/bin`, HOME: home,
  CARGO_HOME: path.join(output, 'cargo-home'), CARGO_TARGET_DIR: path.join(output, 'target'),
  RUSTC_WRAPPER: '', RUSTC_WORKSPACE_WRAPPER: '', RUSTFLAGS: '', CARGO_ENCODED_RUSTFLAGS: '',
  LANG: 'C.UTF-8', LC_ALL: 'C.UTF-8' };
const receipt = { schema: 'noble-m5-build/v1', result: 'running', sources: {}, commands: [], binaries: {},
  pins, environment, assumptions: ['Selected Rust/linker, immutable Nix store source closure and this build driver are trusted tooling. No Rust/Wasmtime compiler correctness theorem is claimed.'] };
function source(file) {
  const input = path.join(root, file);
  assert.ok(fs.lstatSync(input).isFile(), `not a regular source file: ${file}`);
  const bytes = fs.readFileSync(input);
  const target = path.join(workspace, file);
  fs.mkdirSync(path.dirname(target), { recursive: true });
  fs.writeFileSync(target, bytes);
  receipt.sources[file] = hash(bytes);
}
function tree(directory) {
  for (const entry of fs.readdirSync(path.join(root, directory), { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name))) {
    if (['target', '.git', '.lake'].includes(entry.name)) continue;
    const file = `${directory}/${entry.name}`;
    assert.equal(entry.isSymbolicLink(), false, `source symlink: ${file}`);
    if (entry.isDirectory()) tree(file); else source(file);
  }
}
function run(label, executable, args, extraEnv = {}, timeout = 900_000) {
  const id = receipt.commands.length;
  const stem = `${String(id).padStart(3, '0')}-${label}`;
  const stdoutPath = `${stem}.stdout`, stderrPath = `${stem}.stderr`;
  const stdout = fs.openSync(path.join(output, stdoutPath), 'wx');
  const stderr = fs.openSync(path.join(output, stderrPath), 'wx');
  const command = { id, executable, executable_sha256: fileHash(executable), args, cwd: workspace,
    environment: extraEnv, stdout_path: stdoutPath, stderr_path: stderrPath, timeout_ms: timeout };
  let result;
  try { result = spawnSync(executable, args, { cwd: workspace, env: { ...environment, ...extraEnv }, stdio: ['ignore', stdout, stderr], timeout }); }
  finally { fs.closeSync(stdout); fs.closeSync(stderr); }
  Object.assign(command, { status: result.status, signal: result.signal, error: result.error?.message ?? null,
    stdout_sha256: fileHash(path.join(output, stdoutPath)), stderr_sha256: fileHash(path.join(output, stderrPath)) });
  receipt.commands.push(command); save('build.json', receipt);
  assert.equal(command.error, null, label); assert.equal(command.signal, null, label); assert.equal(command.status, 0, `${label}: see ${stderrPath}`);
  assert.equal(command.executable_sha256, fileHash(executable), 'executable changed during execution');
  return command;
}
function text(command) { return fs.readFileSync(path.join(output, command.stdout_path), 'utf8'); }
function artifact(command, name, kind) {
  const candidates = text(command).split('\n').filter(line => line.startsWith('{')).map(line => JSON.parse(line))
    .filter(item => item.reason === 'compiler-artifact' && item.target.name === name && item.target.kind.includes(kind) &&
      (kind !== 'bin' || !item.profile.test));
  const paths = [...new Set(candidates.flatMap(item => kind === 'lib' ? item.filenames.filter(file => file.endsWith('.rlib')) : [item.executable]).filter(Boolean))];
  assert.equal(paths.length, 1, `exact compiled artifact: ${name}/${kind}`);
  return paths[0];
}
function passes(command) { return text(command).split('\n').flatMap(line => /^test (.+) \.\.\. ok$/.exec(line)?.slice(1) ?? []); }
function fence(file, includes, command) {
  const lines = fs.readFileSync(path.join(workspace, file), 'utf8').split('\n');
  let start = null, snippet = '';
  for (const [index, line] of lines.entries()) {
    if (/^\s*\/\/[!/].*```/.test(line)) {
      if (start === null) { start = index + 1; snippet = ''; }
      else {
        if (snippet.includes(includes)) {
          const names = passes(command).filter(name => name.includes(file) && name.includes(`(line ${start})`));
          assert.equal(names.length, 1, `missing exact rustdoc execution for ${file}:${start}`);
          return names[0];
        }
        start = null;
      }
    } else if (start !== null) snippet += `${line}\n`;
  }
  throw Error(`missing source-bound snippet ${file}: ${includes}`);
}
try {
  for (const directory of ['crates', 'verification/m5/peer/src', 'verification/mc2/contracts']) tree(directory);
  for (const file of ['Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', 'verification/m5/peer/Cargo.toml', 'verification/m5/peer/Cargo.lock',
    'verification/m5/pins.json', 'verification/m5/build.mjs', 'verification/m5/borrow-boundary.rs', 'verification/m5/identity-boundary.rs',
    'verification/mc1/monotonic.noble-contract', 'verification/mc1/monotonic-refutation.lean']) source(file);
  for (const [label, directory, expected] of [['wasmtime-source', pins.wasmtime_source, pins.wasmtime_source_nar_hash], ['wasmtime-vendor', pins.vendor, pins.vendor_nar_hash]]) {
    const observed = run(label, '/run/current-system/sw/bin/nix', ['--extra-experimental-features', 'nix-command', 'hash', 'path', '--type', 'sha256', '--sri', directory]);
    assert.equal(text(observed).trim(), expected, `source closure changed: ${label}`);
  }
  const compiled = run('workspace-build', cargo, ['build', '--workspace', '--all-targets', '--all-features', '--locked', '--offline', '--message-format=json']);
  const peerBuild = run('peer-build', cargo, ['build', '--manifest-path', 'verification/m5/peer/Cargo.toml', '--locked', '--offline', '--message-format=json',
    '--config', 'source.crates-io.replace-with="m5-vendor"', '--config', `source.m5-vendor.directory="${pins.vendor}/source-registry-0"`],
    { CARGO_TARGET_DIR: path.join(output, 'peer-target') });
  for (const [name, file] of Object.entries({ cli: artifact(compiled, 'noble', 'bin'),
    resource_tests: artifact(compiled, 'm5-resources', 'test'), authority_tests: artifact(compiled, 'm5-authority', 'test'),
    peer: artifact(peerBuild, 'noble-m5-peer', 'bin') })) receipt.binaries[name] = { path: file, sha256: fileHash(file) };
  const authority = run('authority-opacity', cargo, ['test', '--doc', '-p', 'noble-kernel', '--locked', '--offline']);
  const rustdocArgs = ['--test', '--edition=2021', '-L', `dependency=${path.join(output, 'target/debug/deps')}`,
    '-L', `dependency=${path.join(output, 'peer-target/debug/deps')}`, '--extern', `noble_kernel=${artifact(compiled, 'noble_kernel', 'lib')}`,
    '--extern', `noble_contracts=${artifact(compiled, 'noble_contracts', 'lib')}`, '--extern', `wasmtime=${artifact(peerBuild, 'wasmtime', 'lib')}`];
  const borrow = run('borrow-opacity', rustdoc, [...rustdocArgs, 'verification/m5/borrow-boundary.rs']);
  const identity = run('namespace-identity', rustdoc, [...rustdocArgs, 'verification/m5/identity-boundary.rs']);
  const variant = (name, file, includes, command) => ({ name, passed: true, command: command.id, test_name: fence(file, includes, command) });
  const permit = 'crates/noble-kernel/src/authority/permit.rs';
  const report = 'crates/noble-kernel/src/authority/report.rs';
  const variants = [
    variant('default-construct-witness', permit, 'Witness::default()', authority),
    variant('public-field-construction', permit, 'Witness { claim }', authority),
    ...['plan', 'witness', 'attempt'].map(value => variant(`${value}-as-success`, report, `Some(&${value})`, authority)),
    ...[['Pair', 'Val::Tuple'], ['List', 'Val::List'], ['quote', 'Node::Literal'], ['session', 'session.commit(token.into())']]
      .map(([name, snippet]) => variant(`borrow-${name}`, 'verification/m5/borrow-boundary.rs', snippet, borrow)),
    variant('borrow-positive-ingress', 'verification/m5/borrow-boundary.rs', 'fn pair(owner:', borrow),
    variant('namespace-local-definition-identity', 'verification/m5/identity-boundary.rs', 'assert_ne!(before.build_context()', identity),
  ];
  for (const [file, digest] of Object.entries(receipt.sources)) {
    assert.equal(fileHash(path.join(root, file)), digest, `source changed during build: ${file}`);
    assert.equal(fileHash(path.join(workspace, file)), digest, `build source changed: ${file}`);
  }
  save('opacity.json', { schema: 'noble-m5-opacity/v1', passed: true, source_sha256: receipt.sources,
    commands: [authority, borrow, identity], variants });
  receipt.result = 'passed'; save('build.json', receipt);
  console.log(JSON.stringify({ result: receipt.result, binaries: receipt.binaries, opacity_report: path.join(output, 'opacity.json') }));
} catch (error) {
  receipt.result = 'failed'; receipt.error = String(error.stack ?? error); save('build.json', receipt); throw error;
}
