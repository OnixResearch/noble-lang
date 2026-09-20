#!/usr/bin/env bun
// Source binding + compiled theorem audit. Separate from application proof checking.
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const argument = process.argv[2];
if (!argument || argument === '--help' || process.argv.length !== 3) {
  console.log('usage: bun verification/mc1/implementation.mjs NEW_ARTIFACT_DIRECTORY');
  process.exit(argument === '--help' ? 0 : 2);
}
const artifacts = path.resolve(argument);
fs.mkdirSync(path.dirname(artifacts), { recursive: true });
fs.mkdirSync(artifacts); // Never overwrite a previous run.
const selection = JSON.parse(fs.readFileSync(path.join(root, 'policy/tool-selection.json'), 'utf8'));
const tool = (name, binary) => path.join(selection.tool_paths[name].output, 'bin', binary);
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const fileSha = file => sha(fs.readFileSync(file));

function files(directory) {
  return fs.readdirSync(path.join(root, directory), { withFileTypes: true }).flatMap(entry => {
    if (entry.name.startsWith('.') || ['target', 'node_modules'].includes(entry.name)) return [];
    const item = path.posix.join(directory, entry.name);
    if (entry.isSymbolicLink()) throw Error(`unbound source symlink: ${item}`);
    return entry.isDirectory() ? files(item) : [item];
  });
}
function snapshot() {
  const inputs = new Set([
    ...Object.keys(selection.file_sha256), 'policy/tool-selection.json',
    'verification/mc1/implementation.mjs',
    ...files('crates/noble-kernel/src'), ...files('crates/noble-contracts/src'),
    ...files('proofs/mc1'), ...files('proofs/m3/NobleKernel'),
    'proofs/m3/NobleKernel.lean',
    ...files('verification/mc1').filter(file => /\.(noble-contract|lean)$/.test(file)),
  ]);
  return Object.fromEntries([...inputs].sort().map(file => [file, fileSha(path.join(root, file))]));
}
const sourceFiles = snapshot();
for (const [file, expected] of Object.entries(selection.file_sha256)) {
  if (sourceFiles[file] !== expected) throw Error(`stale selected configuration: ${file}`);
}
for (const name of ['CHARON_EXE', 'AENEAS_EXE', 'LEAN_PATH', 'LEAN_SRC_PATH', 'LEAN_SYSROOT',
  'LAKE_HOME', 'RUSTC', 'RUSTC_WRAPPER', 'RUSTC_WORKSPACE_WRAPPER', 'RUSTFLAGS',
  'CARGO_ENCODED_RUSTFLAGS', 'RUSTUP_TOOLCHAIN', 'CHARON_IS_SYMLINK']) {
  if (process.env[name]) throw Error(`unreviewed tool override: ${name}`);
}
const environment = { ...process.env, RUSTC_WRAPPER: '',
  PATH: `${selection.tool_paths.extraction_rust.output}/bin:${selection.tool_paths.lean.output}/bin:${process.env.PATH}`,
  CARGO_TARGET_DIR: path.join(artifacts, 'cargo-target') };
const commands = [];
function run(label, binary, args, cwd = root, timeout = 900_000) {
  const result = spawnSync(binary, args, { cwd, env: environment, encoding: 'utf8',
    timeout, maxBuffer: 16 * 1024 * 1024 });
  const output = (result.stdout ?? '') + (result.stderr ?? '');
  fs.writeFileSync(path.join(artifacts, `${label}.log`), output);
  commands.push({ label, binary, args, cwd: path.relative(root, cwd) || '.',
    exit: result.status, signal: result.signal, output_sha256: sha(output) });
  if (result.error || result.status !== 0) throw Error(`${label} failed; see ${label}.log: ${result.error ?? result.status}`);
  return output;
}
function identical(actual, expected) {
  if (!fs.readFileSync(actual).equals(fs.readFileSync(expected))) {
    throw Error(`stale generated implementation: ${path.relative(root, expected)}`);
  }
}
function identicalEntry(actual, expected) {
  // The inherited entrypoint carries the documented regeneration runbook as
  // line comments. It may contain only the freshly generated import otherwise.
  const imports = file => fs.readFileSync(file, 'utf8').split('\n')
    .filter(line => line.trim() && !line.trimStart().startsWith('--')).join('\n');
  if (imports(actual) !== imports(expected)) throw Error(`changed generated entrypoint: ${expected}`);
}
function extract(label, manifest, basename, checkedDirectory, allowedOpaque) {
  const directory = path.join(artifacts, label);
  fs.mkdirSync(directory);
  const llbc = path.join(directory, `${basename}.llbc`);
  const generated = path.join(directory, 'generated');
  fs.mkdirSync(generated);
  run(`${label}-charon`, tool('charon', 'charon'), ['cargo', ...selection.configuration.charon_args,
    '--dest-file', llbc, '--', '--manifest-path', manifest, ...selection.configuration.cargo_args]);
  run(`${label}-aeneas`, tool('aeneas', 'aeneas'), [...selection.configuration.aeneas_args,
    '-split-files', '-gen-lib-entry', '-all-computable', '-dest', generated, llbc]);
  const prefix = label === 'kernel' ? 'NobleKernel' : 'NobleContractImpl';
  for (const file of ['Types.lean', 'Funs.lean']) {
    identical(path.join(generated, file), path.join(root, checkedDirectory, file));
  }
  identicalEntry(path.join(generated, `${prefix}.lean`), path.join(root, `${checkedDirectory}.lean`));
  const translation = JSON.parse(fs.readFileSync(path.join(generated, 'translation.json'), 'utf8'));
  const opaque = translation.functions.filter(item => item.is_local && item.is_opaque).map(item => item.rust_name).sort();
  if (JSON.stringify(opaque) !== JSON.stringify([...allowedOpaque].sort())) {
    throw Error(`unreviewed local extraction boundaries in ${label}: ${JSON.stringify(opaque)}`);
  }
  return { llbc_sha256: fileSha(llbc), translation_sha256: fileSha(path.join(generated, 'translation.json')),
    generated: Object.fromEntries(['Types.lean', 'Funs.lean', `${prefix}.lean`]
      .map(file => [file, fileSha(path.join(generated, file))])),
    local_functions: translation.functions.filter(item => item.is_local).length,
    local_opaque_functions: opaque,
    external_functions: translation.functions.filter(item => !item.is_local)
      .map(item => ({ rust: item.rust_name, lean: item.lean_name })) };
}

try {
  const versions = {
    rust: run('rust-version', tool('extraction_rust', 'rustc'), ['--version', '--verbose']),
    charon: run('charon-version', tool('charon', 'charon'), ['version']),
    aeneas: run('aeneas-version', tool('aeneas', 'aeneas'), ['-version']),
    lean: run('lean-version', tool('lean', 'lean'), ['--version']),
  };
  const kernel = extract('kernel', 'crates/noble-kernel/Cargo.toml', 'noble_kernel',
    'proofs/m3/NobleKernel', [
      'noble_kernel::types::impls::clone_stack', 'noble_kernel::types::impls::debug_stack',
      'noble_kernel::shapes::impls::clone_parts', 'noble_kernel::shapes::impls::debug_parts',
      'noble_kernel::shapes::impls::debug_slots',
    ]);
  const contracts = extract('contracts', 'crates/noble-contracts/Cargo.toml', 'noble_contract_impl',
    'proofs/mc1/NobleContractImpl', []);
  const proofRoot = path.join(root, 'proofs/mc1');
  run('lean-build', tool('lean', 'lake'), ['build', 'NobleContracts',
    'NobleContractImpl.Projection', 'NobleContractImpl.Statement'], proofRoot);
  const library = run('contract-library', tool('lean', 'lake'), ['env', 'lean', 'ContractGate.lean'], proofRoot);
  if (!library.includes('MC1-CONTRACT-LIBRARY: 43 required theorems; strict axioms accepted')) {
    throw Error('missing strict contract-library verdict');
  }
  const audit = run('implementation-audit', tool('lean', 'lake'), ['env', 'lean', 'ImplementationGate.lean'], proofRoot);
  const records = prefix => audit.split('\n').filter(line => line.startsWith(prefix))
    .map(line => JSON.parse(line.slice(prefix.length)));
  const [implementation, ...extra] = records('MC1-IMPLEMENTATION ');
  const statements = records('MC1-SOURCE-EQUATION ');
  if (!implementation || extra.length || implementation.strict_theorems !== 61 || statements.length !== 6) {
    throw Error('incomplete compiled implementation theorem audit');
  }
  if (JSON.stringify(snapshot()) !== JSON.stringify(sourceFiles)) throw Error('source changed during proof gate');
  const evidence = { schema_version: 1, result: 'passed', scope: 'mc1-statement-export-v1',
    source_revision: `sha256:${sha(JSON.stringify(sourceFiles))}`, source_files: sourceFiles,
    versions, commands, kernel, contracts, implementation, statements,
    assumptions: [
      'Pinned Charon/Aeneas translation and handwritten standard-library models are explicit trust boundaries.',
      'Inherited local opaque copy/format helpers are exactly the five enumerated kernel boundaries.',
      'Allocation identity, spare capacity, allocator failure, and formatting layout are abstracted by the inherited models.',
      'Generated string-bound proofs and six closed source equations use the separately listed native-evaluation axioms.',
      'The 61 universal bridge/projection theorems and 43 application/rule theorems use strict standard axioms only.',
      'This is a named source-export fragment, not universal frontend/exporter or runtime correctness.',
    ] };
  fs.writeFileSync(path.join(artifacts, 'implementation.json'), JSON.stringify(evidence, null, 2) + '\n');
  console.log(`MC1-IMPLEMENTATION-GATE passed: ${evidence.source_revision}`);
} catch (error) {
  fs.writeFileSync(path.join(artifacts, 'failure.json'), JSON.stringify({ result: 'failed',
    message: String(error), commands }, null, 2) + '\n');
  console.error(String(error));
  process.exitCode = 1;
}
