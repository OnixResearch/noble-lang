import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { root, fileHash } from './record.mjs';

// The proof/frontend workers produce ONE fresh source-stable build. This gate
// never performs a duplicate Cargo build: it checks and freezes that build's
// binary bytes, selected tools, exact source inventory and raw command logs.
export function bindBuild(record, plan, buildReceiptPath) {
  const receiptFile = fs.realpathSync(path.resolve(buildReceiptPath));
  const receipt = JSON.parse(fs.readFileSync(receiptFile, 'utf8'));
  assert.equal(receipt.schema, 'noble-intrinsic-proofs-build/v1');
  assert.equal(receipt.result, 'built');
  assert.ok(Array.isArray(receipt.commands) && receipt.commands.length >= 2,
    'fresh CLI and kernel peer build commands required');
  const selected = JSON.parse(fs.readFileSync(path.join(root, 'policy/tool-selection.json')));
  const pins = JSON.parse(fs.readFileSync(path.join(root, 'verification/m6/pins.json')));
  const cargo = path.join(selected.tool_paths.quality_rust.output, 'bin/cargo');
  const rustc = path.join(selected.tool_paths.quality_rust.output, 'bin/rustc');
  const linker = path.join(pins.linker_bin, 'cc');
  const binutilsLinker = fs.realpathSync(path.join(pins.linker_bin, 'ld'));
  const mold = fs.realpathSync(`/etc/profiles/per-user/${path.basename(process.env.HOME)}/bin/mold`);
  const cargoConfig = path.join(process.env.HOME, '.cargo/config.toml');
  assert.equal(receipt.tools?.cargo?.sha256, fileHash(cargo), 'unselected Cargo build');
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
  assert.equal(receipt.tools?.cargo_config?.sha256, fileHash(cargoConfig),
    'modified Cargo linker configuration');
  assert.equal(receipt.tools?.vendor, path.join(pins.vendor, 'source-registry-0'),
    'build did not use selected offline vendor');
  for (const [name, digest] of Object.entries({ cargo: plan.tools.cargo.sha256,
    rustc: plan.tools.rustc.sha256 }))
    assert.equal(receipt.tools[name].sha256, digest, `${name}: planned tool changed`);
  const requiredSources = Object.entries(plan.sources).filter(([name]) =>
    name.startsWith('crates/') || name.startsWith('proofs/mc1/')
      || name.startsWith('verification/intrinsic-proofs/peer/')
      || ['Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', 'policy/tool-selection.json'].includes(name));
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
    assert.ok(command.argv.includes('--locked') && command.argv.includes('--offline'),
      `shared build ${index}: not locked offline`);
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
