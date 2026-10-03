#!/usr/bin/env node
// Record one real, source-stable production CLI build for the Result gate.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { root, fileHash } from './record.mjs';
import { selectedVendor } from '../selected-vendor.mjs';

assert.equal(process.argv.length, 3,
  'usage: SELECTED_NODE verification/result-library/collect-build.mjs NEW_EXTERNAL_DIRECTORY');
const output = path.resolve(process.argv[2]);
assert.ok(output !== root && !output.startsWith(`${root}${path.sep}`),
  'CLI build output must be outside source');
assert.ok(!fs.existsSync(output), 'CLI build output must be fresh');
assert.equal(fs.realpathSync(path.dirname(output)), path.dirname(output),
  'CLI build output parent must not be a symlink');
const selected = JSON.parse(fs.readFileSync(path.join(root, 'policy/tool-selection.json')));
const pins = JSON.parse(fs.readFileSync(path.join(root, 'verification/m6/pins.json')));
assert.equal(fs.realpathSync(process.execPath),
  fs.realpathSync(path.join(selected.tool_paths.node.output, 'bin/node')),
  'source-stable build requires selected Node');
const rust = selected.tool_paths.quality_rust.output;
const cargo = path.join(rust, 'bin/cargo');
const rustc = path.join(rust, 'bin/rustc');
const rustdoc = path.join(rust, 'bin/rustdoc');
const linker = fs.realpathSync(path.join(pins.linker_bin, 'ld'));
const mold = fs.realpathSync(`/etc/profiles/per-user/${path.basename(process.env.HOME)}/bin/mold`);
const vendor = selectedVendor(pins);
function reviewedHead() {
  const git = spawnSync('/run/current-system/sw/bin/git', ['rev-parse', 'HEAD'],
    { cwd: root, env: { PATH: '/run/current-system/sw/bin' },
      encoding: 'utf8', timeout: 30_000 });
  assert.equal(git.status, 0, 'cannot identify reviewed Git HEAD');
  const head = git.stdout.trim();
  assert.match(head, /^[0-9a-f]{40}$/u);
  return head;
}
const sourceRevision = reviewedHead();
function snapshot() {
  const sources = {};
  const add = name => {
    const file = path.join(root, name);
    assert.ok(fs.lstatSync(file).isFile(), `non-regular CLI source: ${name}`);
    sources[name] = { sha256: fileHash(file) };
  };
  const walk = directory => {
    for (const entry of fs.readdirSync(path.join(root, directory), { withFileTypes: true })) {
      const name = `${directory}/${entry.name}`;
      if (entry.isDirectory()) walk(name);
      else add(name);
    }
  };
  for (const name of ['Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml',
    'policy/tool-selection.json', 'verification/m6/pins.json',
    'nix/reviewed-vendor.nix', 'verification/selected-vendor.mjs',
    'verification/result-library/collect-build.mjs']) add(name);
  for (const directory of ['crates/noble-contracts', 'crates/noble-kernel',
    'crates/noble-wasm', 'crates/noble-cli', 'crates/noble-syndicate']) walk(directory);
  return Object.fromEntries(Object.entries(sources).sort(([a], [b]) => a.localeCompare(b)));
}
const preSources = snapshot();
fs.mkdirSync(output);
for (const name of ['home', 'cargo', 'target', 'tmp']) fs.mkdirSync(path.join(output, name));
const environment = {
  PATH: `${path.dirname(mold)}:${path.dirname(linker)}:${pins.linker_bin}:${path.join(rust, 'bin')}:/run/current-system/sw/bin`,
  COMPILER_PATH: path.dirname(linker), HOME: path.join(output, 'home'),
  CARGO_HOME: path.join(output, 'cargo'), CARGO_TARGET_DIR: path.join(output, 'target'),
  CARGO_NET_OFFLINE: 'true', CARGO_BUILD_JOBS: '2', CARGO_INCREMENTAL: '0',
  RUSTC: rustc, RUSTDOC: rustdoc, CC: path.join(pins.linker_bin, 'cc'), LD: linker,
  RUSTFLAGS: '-C link-arg=-fuse-ld=mold', RUSTC_WRAPPER: '',
  RUSTC_WORKSPACE_WRAPPER: '', CARGO_BUILD_RUSTC_WRAPPER: '',
  LANG: 'C.UTF-8', LC_ALL: 'C.UTF-8', USER: process.env.USER ?? 'nobody',
  TMPDIR: path.join(output, 'tmp'),
};
const argv = [cargo, 'build', '--manifest-path', 'Cargo.toml', '-p', 'noble-cli',
  '--bin', 'noble', '--locked', '--offline',
  '--config', 'source.crates-io.replace-with="m6-vendor"',
  '--config', `source.m6-vendor.directory="${vendor.directory}"`];
const run = spawnSync(cargo, argv.slice(1), { cwd: root, env: environment,
  encoding: null, timeout: 3_600_000, killSignal: 'SIGKILL', maxBuffer: 64 * 1024 * 1024 });
const logs = {};
for (const stream of ['stdout', 'stderr']) {
  const file = path.join(output, `cargo.${stream}.log`);
  fs.writeFileSync(file, run[stream] ?? Buffer.alloc(0), { flag: 'wx', mode: 0o400 });
  logs[stream] = { path: file, sha256: fileHash(file) };
}
const postSources = snapshot();
const postSourceRevision = reviewedHead();
const binaryPath = path.join(output, 'target/debug/noble');
const failure = run.status !== 0 || run.signal || run.error
  ? `selected offline CLI build failed: ${run.error ?? run.signal ?? run.status}`
  : sourceRevision !== postSourceRevision ? 'reviewed Git HEAD changed during production CLI build'
    : JSON.stringify(preSources) !== JSON.stringify(postSources)
      ? 'production CLI sources changed during build'
      : !fs.existsSync(binaryPath) ? 'production CLI binary was not emitted' : null;
const receipt = {
  schema: 'noble-result-cli-build/v1', result: failure ? 'failed' : 'built', error: failure,
  sourceRevision, postSourceRevision, preSources, postSources, sources: postSources,
  binary: failure ? null : { path: binaryPath, sha256: fileHash(binaryPath) },
  tools: { cargo: { path: cargo, sha256: fileHash(cargo) },
    rustc: { path: rustc, sha256: fileHash(rustc) },
    mold: { path: mold, sha256: fileHash(mold) },
    vendor: { path: vendor.directory, nar_hash: vendor.narHash } },
  commands: [{ argv, cwd: root, env: environment, status: run.status,
    signal: run.signal, error: run.error ? String(run.error) : null, ...logs }],
};
const report = path.join(output, 'build.json');
fs.writeFileSync(report, JSON.stringify(receipt, null, 2) + '\n', { flag: 'wx', mode: 0o400 });
console.log(JSON.stringify({ result: receipt.result, report, binary: receipt.binary,
  source_files: Object.keys(postSources).length, error: failure }));
process.exitCode = failure ? 1 : 0;
