import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { root, fileHash } from './record.mjs';
import { selectedVendor } from '../selected-vendor.mjs';

// The proof/frontend workers produce ONE fresh source-stable build. This gate
// never performs a duplicate Cargo build: it checks and freezes that build's
// binary bytes, selected tools, exact source inventory and raw command logs.
export function bindBuild(record, plan, buildReceiptPath) {
  const receiptFile = fs.realpathSync(path.resolve(buildReceiptPath));
  const receipt = JSON.parse(fs.readFileSync(receiptFile, 'utf8'));
  assert.equal(receipt.schema, 'noble-intrinsic-proofs-build/v1');
  assert.equal(receipt.result, 'built');
  assert.ok(Array.isArray(receipt.commands) && receipt.commands.length === 2,
    'exactly one fresh CLI and one kernel peer build command required');
  const selected = JSON.parse(fs.readFileSync(path.join(root, 'policy/tool-selection.json')));
  const pins = JSON.parse(fs.readFileSync(path.join(root, 'verification/m6/pins.json')));
  const buildRoot = path.dirname(receiptFile);
  const cargo = path.join(selected.tool_paths.quality_rust.output, 'bin/cargo');
  const rustc = path.join(selected.tool_paths.quality_rust.output, 'bin/rustc');
  const linker = path.join(pins.linker_bin, 'cc');
  const binutilsLinker = fs.realpathSync(path.join(pins.linker_bin, 'ld'));
  const mold = fs.realpathSync(`/etc/profiles/per-user/${path.basename(process.env.HOME)}/bin/mold`);
  const cargoConfig = path.join(process.env.HOME, '.cargo/config.toml');
  assert.equal(receipt.tools?.cargo?.path, cargo, 'unselected Cargo executable');
  assert.equal(receipt.tools?.cargo?.sha256, fileHash(cargo), 'unselected Cargo build');
  assert.equal(receipt.tools?.rustc?.path, rustc, 'unselected Rust executable');
  assert.equal(receipt.tools?.rustc?.sha256, fileHash(rustc), 'unselected Rust build');
  assert.equal(receipt.tools?.linker?.path, linker, 'unselected C linker');
  assert.equal(receipt.tools?.linker?.sha256, fileHash(linker), 'modified C linker');
  assert.equal(receipt.tools?.binutils_linker?.path, binutilsLinker,
    'unselected ELF linker');
  assert.equal(receipt.tools?.binutils_linker?.sha256, fileHash(binutilsLinker),
    'modified ELF linker');
  assert.equal(receipt.tools?.mold?.path, mold, 'unselected mold linker');
  assert.equal(receipt.tools?.mold?.sha256, fileHash(mold), 'modified mold linker');
  assert.equal(receipt.tools?.cargo_config?.path, cargoConfig,
    'unselected Cargo linker configuration');
  assert.equal(receipt.tools?.cargo_config?.sha256,
    'fd49ee6f0a53eb27d583fc54fdbf3c179e116e02942b7de4d52990896f961377',
    'Cargo linker configuration differs from the separately reviewed cc/mold config');
  assert.equal(receipt.tools?.cargo_config?.sha256, fileHash(cargoConfig),
    'modified Cargo linker configuration');
  const vendor = selectedVendor(pins);
  assert.equal(receipt.tools?.vendor, vendor.directory,
    'build did not use selected offline vendor');
  assert.equal(receipt.tools?.vendor_nar_hash, vendor.narHash,
    'build did not hash selected offline vendor NAR');
  assert.equal(receipt.tools?.isolated_cargo_config?.path,
    path.join(path.dirname(receiptFile), 'cargo-home/config.toml'),
    'shared build did not isolate reviewed Cargo configuration');
  assert.equal(receipt.tools?.isolated_cargo_config?.sha256, fileHash(cargoConfig),
    'isolated Cargo configuration differs from reviewed linker configuration');
  assert.equal(fileHash(receipt.tools.isolated_cargo_config.path), fileHash(cargoConfig),
    'isolated Cargo configuration changed after shared build');
  for (const [name, digest] of Object.entries({ cargo: plan.tools.cargo.sha256,
    rustc: plan.tools.rustc.sha256 }))
    assert.equal(receipt.tools[name].sha256, digest, `${name}: planned tool changed`);
  const requiredSources = Object.entries(plan.sources).filter(([name]) =>
    name.startsWith('crates/') || name.startsWith('proofs/mc1/')
      || name.startsWith('verification/intrinsic-proofs/peer/')
      || ['Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', 'policy/tool-selection.json',
        'verification/m6/pins.json', 'nix/reviewed-vendor.nix',
        'verification/selected-vendor.mjs',
        'verification/intrinsic-proofs/collect-build.mjs'].includes(name));
  for (const [name, frozen] of requiredSources) {
    assert.equal(receipt.source_files?.[name], frozen.sha256,
      `fresh build omitted or differs from current ${name}`);
    assert.equal(fileHash(path.join(root, name)), frozen.sha256,
      `source changed since shared build: ${name}`);
  }
  const commands = receipt.commands.map((command, index) => {
    assert.equal(command.status, 0, `shared build ${index} failed`);
    assert.ok(Array.isArray(command.argv) && command.argv.length > 0,
      `shared build ${index}: missing command argv`);
    assert.ok(command.environment && typeof command.environment === 'object',
      `shared build ${index}: missing build environment`);
    const config = ['--config', 'source.crates-io.replace-with="m6-vendor"',
      '--config', `source.m6-vendor.directory="${vendor.directory}"`];
    const args = index === 0
      ? ['build', '--manifest-path', 'Cargo.toml', '-p', 'noble-cli', '--bin', 'noble',
        '--locked', '--offline', ...config]
      : ['build', '--manifest-path', 'verification/intrinsic-proofs/peer/Cargo.toml',
        '--bin', 'noble-intrinsic-proofs-peer', '--locked', '--offline', '-vv', ...config];
    assert.deepEqual(command.argv, args,
      `shared build ${index}: did not run exact locked offline Cargo command`);
    assert.equal(command.cwd, root, `shared build ${index}: wrong source root`);
    assert.deepEqual(command.environment, {
      PATH: `${path.dirname(mold)}:${path.dirname(binutilsLinker)}:${pins.linker_bin}:${path.dirname(cargo)}:/run/current-system/sw/bin`,
      COMPILER_PATH: path.dirname(binutilsLinker),
      HOME: path.join(buildRoot, 'home'), TMPDIR: path.join(buildRoot, 'tmp'),
      CARGO_HOME: path.join(buildRoot, 'cargo-home'),
      CARGO_TARGET_DIR: path.join(buildRoot, 'target'),
      CARGO_NET_OFFLINE: 'true', CARGO_BUILD_JOBS: '2', CARGO_INCREMENTAL: '0',
      RUSTC: rustc, RUSTDOC: path.join(path.dirname(rustc), 'rustdoc'),
      CC: linker, LD: binutilsLinker,
      RUSTFLAGS: `-C link-arg=-fuse-ld=mold --remap-path-prefix=${buildRoot}=/reviewed-build`,
      RUSTC_WRAPPER: '', RUSTC_WORKSPACE_WRAPPER: '',
      CARGO_BUILD_RUSTC_WRAPPER: '',
      LANG: 'C.UTF-8', LC_ALL: 'C.UTF-8', USER: process.env.USER ?? 'nobody',
    }, `shared build ${index}: unreviewed compiler, linker or Cargo environment`);
    const logs = {};
    for (const stream of ['stdout', 'stderr']) {
      const entry = command[stream];
      assert.ok(entry?.path && entry.sha256, `shared build ${index}: missing ${stream}`);
      const file = fs.realpathSync(entry.path);
      assert.ok(fs.statSync(file).isFile(), `shared build ${index}: ${stream} is not file`);
      assert.equal(record.watch(file), entry.sha256,
        `shared build ${index}: altered ${stream}`);
      const copy = record.retain(`plan/build/${index}-${stream}.log`, fs.readFileSync(file));
      logs[stream] = { file: record.relative(copy), sha256: fileHash(copy) };
    }
    return { argv: command.argv, cwd: command.cwd, environment: command.environment,
      status: command.status, logs };
  });
  const binaries = {};
  for (const [name, key] of [['noble', 'cli'], ['kernel-peer', 'kernel_peer']]) {
    const item = receipt.binaries?.[key];
    assert.ok(item?.path && item.sha256, `missing fresh ${name} build output`);
    assert.equal(item.path, path.join(buildRoot, 'target/debug',
      name === 'noble' ? 'noble' : 'noble-intrinsic-proofs-peer'),
    `${name}: executable not emitted from isolated Cargo target`);
    const original = fs.realpathSync(item.path);
    assert.equal(record.watch(original), item.sha256, `${name}: build output changed`);
    binaries[name] = record.freeze(name, original);
  }
  assert.equal(receipt.reviewed_cli_sha256, receipt.binaries.cli.sha256,
    'final CLI does not match reviewed compiled source-stable candidate');
  const copied = record.retain('plan/build/build.json', fs.readFileSync(receiptFile));
  record.watch(receiptFile);
  record.receipt.build = { file: record.relative(copied), sha256: fileHash(copied),
    binaries: Object.fromEntries(Object.entries(binaries).map(([name, file]) =>
      [name, { file: record.relative(file), sha256: fileHash(file) }])), commands,
    source_files_checked: requiredSources.length };
  return {
    cli: binaries.noble, peer: binaries['kernel-peer'],
    wasmTools: path.join(record.output, plan.tools['wasm-tools'].file),
    lean: path.join(record.output, plan.tools.lean.file),
  };
}
