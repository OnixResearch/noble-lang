#!/usr/bin/env node
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { root, fileHash } from './record.mjs';
import { selectedVendor } from '../selected-vendor.mjs';

assert.equal(process.argv.length, 4,
  'usage: SELECTED_NODE verification/intrinsic-proofs/collect-build.mjs NEW_EXTERNAL_DIRECTORY SOURCE_STABLE_CLI_SHA256');
const output = path.resolve(process.argv[2]);
const expectedCliSha256 = process.argv[3];
assert.match(expectedCliSha256, /^[0-9a-f]{64}$/u, 'source-stable CLI binary fingerprint required');
assert.ok(!output.startsWith(`${root}${path.sep}`) && output !== root,
  'source-stable build receipt belongs outside source tree');
assert.ok(!fs.existsSync(output), 'do not overwrite any build receipt');
assert.equal(fs.realpathSync(path.dirname(output)), path.dirname(output),
  'build receipt parent may not be a symlink');
const selection = JSON.parse(fs.readFileSync(path.join(root, 'policy/tool-selection.json')));
const pins = JSON.parse(fs.readFileSync(path.join(root, 'verification/m6/pins.json')));
const node = fs.realpathSync(path.join(selection.tool_paths.node.output, 'bin/node'));
assert.equal(fs.realpathSync(process.execPath), node, 'selected Node required for build receipt');
const cargo = path.join(selection.tool_paths.quality_rust.output, 'bin/cargo');
const rustc = path.join(selection.tool_paths.quality_rust.output, 'bin/rustc');
const linker = path.join(pins.linker_bin, 'cc');
const binutilsLinker = fs.realpathSync(path.join(pins.linker_bin, 'ld'));
const binutilsDirectory = path.dirname(binutilsLinker);
const mold = fs.realpathSync(`/etc/profiles/per-user/${path.basename(process.env.HOME)}/bin/mold`);
const moldDirectory = path.dirname(mold);
const cargoHome = path.join(process.env.HOME, '.cargo');
const cargoConfig = path.join(cargoHome, 'config.toml');
const cargoConfigBytes = fs.readFileSync(cargoConfig, 'utf8');
const cargoConfigSha256 = fileHash(cargoConfig);
assert.equal(cargoConfigSha256,
  'fd49ee6f0a53eb27d583fc54fdbf3c179e116e02942b7de4d52990896f961377',
  'Cargo linker configuration differs from the separately reviewed cc/mold config');
assert.match(cargoConfigBytes, /linker\s*=\s*"cc"/u,
  'reviewed Cargo configuration must select cc');
assert.match(cargoConfigBytes, /link-arg=-fuse-ld=mold/u,
  'reviewed Cargo configuration must select mold');
const { directory: vendor, narHash: vendorNarHash } = selectedVendor(pins);
assert.ok(fs.statSync(linker).isFile(), 'missing selected C linker');
fs.mkdirSync(output);
fs.mkdirSync(path.join(output, 'tmp'));
fs.mkdirSync(path.join(output, 'home'));
const isolatedCargoHome = path.join(output, 'cargo-home');
fs.mkdirSync(isolatedCargoHome);
const isolatedCargoConfig = path.join(isolatedCargoHome, 'config.toml');
fs.copyFileSync(cargoConfig, isolatedCargoConfig, fs.constants.COPYFILE_EXCL);
const environment = {
  PATH: `${moldDirectory}:${binutilsDirectory}:${pins.linker_bin}:${path.dirname(cargo)}:/run/current-system/sw/bin`,
  COMPILER_PATH: binutilsDirectory,
  HOME: path.join(output, 'home'),
  TMPDIR: path.join(output, 'tmp'),
  CARGO_HOME: isolatedCargoHome,
  CARGO_TARGET_DIR: path.join(output, 'target'),
  CARGO_NET_OFFLINE: 'true',
  CARGO_BUILD_JOBS: '2',
  CARGO_INCREMENTAL: '0',
  RUSTC: rustc,
  RUSTDOC: path.join(path.dirname(rustc), 'rustdoc'),
  CC: linker,
  LD: binutilsLinker,
  RUSTFLAGS: `-C link-arg=-fuse-ld=mold --remap-path-prefix=${output}=/reviewed-build`,
  RUSTC_WRAPPER: '',
  RUSTC_WORKSPACE_WRAPPER: '',
  CARGO_BUILD_RUSTC_WRAPPER: '',
  LANG: 'C.UTF-8', LC_ALL: 'C.UTF-8', USER: process.env.USER ?? 'nobody',
};

function walk(directory, sources) {
  for (const entry of fs.readdirSync(path.join(root, directory), { withFileTypes: true })
    .sort((a, b) => a.name.localeCompare(b.name))) {
    if (entry.name === '.lake' || entry.name === 'target') continue;
    const relative = `${directory}/${entry.name}`;
    if (entry.isDirectory()) walk(relative, sources);
    else {
      assert.ok(entry.isFile(), `non-regular build input: ${relative}`);
      sources[relative] = fileHash(path.join(root, relative));
    }
  }
}
function sourceSnapshot() {
  const sources = {};
  for (const name of ['Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml',
    'policy/tool-selection.json', 'verification/m6/pins.json',
    'nix/reviewed-vendor.nix', 'verification/selected-vendor.mjs',
    'verification/intrinsic-proofs/collect-build.mjs'])
    sources[name] = fileHash(path.join(root, name));
  for (const tree of ['crates/noble-contracts', 'crates/noble-kernel',
    'crates/noble-cli', 'crates/noble-wasm', 'proofs/mc1',
    'verification/intrinsic-proofs/peer']) walk(tree, sources);
  return Object.fromEntries(Object.entries(sources).sort(([a], [b]) => a < b ? -1 : a > b ? 1 : 0));
}

const sources = sourceSnapshot();
const config = [
  '--config', 'source.crates-io.replace-with="m6-vendor"',
  '--config', `source.m6-vendor.directory="${vendor}"`,
];
const commands = [];
let failure = null;
for (const [name, argv] of [
  ['cli', ['build', '--manifest-path', 'Cargo.toml', '-p', 'noble-cli', '--bin', 'noble',
    '--locked', '--offline', ...config]],
  ['peer', ['build', '--manifest-path', 'verification/intrinsic-proofs/peer/Cargo.toml',
    '--bin', 'noble-intrinsic-proofs-peer', '--locked', '--offline', '-vv', ...config]],
]) {
  const run = spawnSync(cargo, argv, { cwd: root, env: environment, encoding: null,
    maxBuffer: 64 * 1024 * 1024, timeout: 3_600_000, killSignal: 'SIGKILL' });
  const stdout = path.join(output, `${name}.stdout.log`);
  const stderr = path.join(output, `${name}.stderr.log`);
  fs.writeFileSync(stdout, run.stdout ?? Buffer.alloc(0), { flag: 'wx', mode: 0o400 });
  fs.writeFileSync(stderr, run.stderr ?? Buffer.alloc(0), { flag: 'wx', mode: 0o400 });
  commands.push({ argv, cwd: root, environment, status: run.status,
    signal: run.signal, error: run.error ? String(run.error) : null,
    stdout: { path: stdout, sha256: fileHash(stdout) },
    stderr: { path: stderr, sha256: fileHash(stderr) } });
  if (run.status !== 0 || run.signal || run.error) {
    failure = `${name} source-stable build failed: ${run.error ?? run.signal ?? run.status}`;
    break;
  }
}
if (JSON.stringify(sources) !== JSON.stringify(sourceSnapshot()))
  failure = 'one or more production/peer source inputs changed during shared build';
if (fileHash(cargoConfig) !== cargoConfigSha256)
  failure = 'selected Cargo linker configuration changed during shared build';
if (fileHash(isolatedCargoConfig) !== cargoConfigSha256)
  failure = 'isolated Cargo linker configuration changed during shared build';
const binaries = {};
for (const [name, file] of [
  ['cli', path.join(output, 'target/debug/noble')],
  ['kernel_peer', path.join(output, 'target/debug/noble-intrinsic-proofs-peer')],
]) {
  if (!failure) {
    assert.ok(fs.statSync(file).isFile(), `final ${name} binary missing`);
    binaries[name] = { path: file, sha256: fileHash(file) };
  }
}
if (!failure && binaries.cli.sha256 !== expectedCliSha256)
  failure = 'selected CLI bytes differ from jointly reviewed source-stable compiled candidate';
const receipt = {
  schema: 'noble-intrinsic-proofs-build/v1', result: failure ? 'failed' : 'built',
  error: failure, source_root: root, output_directory: output,
  reviewed_cli_sha256: expectedCliSha256,
  tools: { cargo: { path: cargo, sha256: fileHash(cargo) },
    rustc: { path: rustc, sha256: fileHash(rustc) },
    linker: { path: linker, sha256: fileHash(linker) },
    binutils_linker: { path: binutilsLinker, sha256: fileHash(binutilsLinker) },
    mold: { path: mold, sha256: fileHash(mold) },
    cargo_config: { path: cargoConfig, sha256: cargoConfigSha256 },
    isolated_cargo_config: { path: isolatedCargoConfig, sha256: fileHash(isolatedCargoConfig) },
    vendor, vendor_nar_hash: vendorNarHash },
  source_files: sources, commands, binaries,
};
const report = path.join(output, 'build.json');
fs.writeFileSync(report, JSON.stringify(receipt, null, 2) + '\n',
  { flag: 'wx', mode: 0o400 });
console.log(JSON.stringify({ schema: receipt.schema, result: receipt.result,
  report, cli: binaries.cli ?? null, peer: binaries.kernel_peer ?? null,
  source_files: Object.keys(sources).length, commands: commands.length,
  error: failure }));
process.exitCode = failure ? 1 : 0;
