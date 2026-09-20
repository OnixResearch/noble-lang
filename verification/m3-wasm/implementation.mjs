#!/usr/bin/env bun
// Source extraction and compiled dependency audit; not a Wasm execution/refinement gate.
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const argument = process.argv[2];
if (!argument || argument === '--help' || process.argv.length !== 3) {
  console.log('usage: bun verification/m3-wasm/implementation.mjs NEW_ARTIFACT_DIRECTORY');
  process.exit(argument === '--help' ? 0 : 2);
}
const artifacts = path.resolve(argument);
fs.mkdirSync(path.dirname(artifacts), { recursive: true });
fs.mkdirSync(artifacts); // Refuse to overwrite, resume, or reuse an earlier receipt.
const workspace = path.join(artifacts, 'workspace');
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const fileSha = file => sha(fs.readFileSync(file));
const json = file => JSON.parse(fs.readFileSync(file, 'utf8'));
const save = (file, value) => fs.writeFileSync(path.join(artifacts, file), JSON.stringify(value, null, 2) + '\n');
const equal = (actual, expected, label) => {
  if (JSON.stringify(actual) !== JSON.stringify(expected)) throw Error(label);
};
const evidence = {
  schema_version: 1, scope: 'm3-pure-rust-emitter-extraction-v1', result: 'running',
  commands: [], versions: {}, extractions: {},
  assumptions: [
    'Pinned Charon/Aeneas translation and Lean kernel/tool binaries are trusted.',
    'The separately extracted actual kernel is connected by a transparent total representation bridge.',
    'Handwritten external models and the pinned Aeneas library remain explicit semantic trust boundaries.',
    'Inherited kernel copy/format boundaries are enumerated, not claimed to be extracted Rust bodies.',
    'Allocation identity, spare capacity, allocation failure and formatting layout are abstracted by the inherited models.',
    'The pinned abstract Formatter type is audited separately and forbidden from compile dependencies and strict bridge theorems.',
    'Dependency package build caches are reused only with clean sources at the exact locked revisions; Noble project build products are fresh.',
    'Native evaluation, when reported, is restricted to generated/inherited literal UTF-8 byte-array size bounds and is not a strict proof equation.',
  ],
  non_claims: [
    'No universal Rust-to-Lean or checked-lowering refinement theorem.',
    'No generated WAT/Wasm, optimizer, engine, loader or runtime correctness theorem.',
    'No runtime experiment result, backend selection, safety certification or component/WIT ABI claim.',
  ],
};
let environment;
let selection;
let sourceFiles;
const pins = {
  charon: '/nix/store/ka61j6d6zy8zg91ynlbd6nmjssyynrpl-charon',
  aeneas: '/nix/store/q3hsccq1s53aa85vh3mmc3b0k31pnyl2-ocaml5.2.1-aeneas-0.1.0',
  extraction_rust: '/nix/store/wq92i79ivn1w1nrdhmzl1mqx0wcyrawi-rust-default-1.100.0-nightly-2026-08-18',
  lean: '/nix/store/hbxwgcbpkyqllqzq7cidhnvhgb60jb9q-noble-lean-release-4.31.0',
};
const aeneasRevision = '505b6ca35217e7be5c96c3e2f8045edfbdf47291';
const tool = (name, binary) => path.join(selection.tool_paths[name].output, 'bin', binary);
const skipped = new Set(['.git', '.lake', 'target', 'node_modules']);
function files(directory) {
  return fs.readdirSync(path.join(root, directory), { withFileTypes: true }).flatMap(entry => {
    if (skipped.has(entry.name)) return [];
    const file = path.posix.join(directory, entry.name);
    if (entry.isSymbolicLink()) throw Error(`unbound source symlink: ${file}`);
    if (entry.isDirectory()) return files(file);
    if (!entry.isFile()) throw Error(`non-file source: ${file}`);
    return [file];
  });
}
function snapshot() {
  const inputs = new Set([
    ...Object.keys(selection.file_sha256), 'policy/tool-selection.json',
    'nix/tool-selection-files.nix', 'verification/m3-wasm/implementation.mjs',
    'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml',
    ...files('crates/noble-kernel'), ...files('crates/noble-wasm'),
    ...files('crates/noble-contracts'), ...files('crates/noble-cli'),
    ...files('proofs/m3-wasm'), ...files('proofs/m3/NobleKernel'),
    'proofs/m3/NobleKernel.lean', 'proofs/m3/lake-manifest.json',
    'proofs/m3/lakefile.toml', 'proofs/m3/lean-toolchain',
  ]);
  return Object.fromEntries([...inputs].sort().map(file => {
    if (path.isAbsolute(file) || file.split('/').includes('..')) throw Error(`unbound source path: ${file}`);
    if (fs.lstatSync(path.join(root, file)).isSymbolicLink()) throw Error(`unbound source symlink: ${file}`);
    return [file, fileSha(path.join(root, file))];
  }));
}
function run(label, binary, args, cwd = workspace, extraEnvironment = {}, timeout = 900_000) {
  const log = `${label}.log`;
  const fd = fs.openSync(path.join(artifacts, log), 'wx');
  const command = { label, binary, args, cwd, timeout_ms: timeout, log,
    environment: extraEnvironment, started_at: new Date().toISOString(), exit: null };
  evidence.commands.push(command);
  save('commands.json', evidence.commands);
  let result;
  try {
    // Direct file descriptors preserve complete interleaved output, including failed commands.
    result = spawnSync(binary, args, { cwd, env: { ...environment, ...extraEnvironment },
      stdio: ['ignore', fd, fd], timeout });
  } catch (error) {
    command.spawn_error = String(error);
    throw error;
  } finally {
    fs.closeSync(fd);
    command.finished_at = new Date().toISOString();
    command.exit = result?.status ?? null;
    command.signal = result?.signal ?? null;
    if (result?.error) command.spawn_error = String(result.error);
    command.output_sha256 = fileSha(path.join(artifacts, log));
    save('commands.json', evidence.commands);
  }
  if (result.error || result.status !== 0) throw Error(`${label} failed; see ${log}: ${result.error ?? result.status}`);
  return fs.readFileSync(path.join(artifacts, log), 'utf8');
}
function identity(binary) {
  return { path: binary, realpath: fs.realpathSync(binary), sha256: fileSha(binary) };
}
function requireNoCargoConfiguration(directory) {
  for (let parent = path.resolve(directory); ; parent = path.dirname(parent)) {
    for (const name of ['config', 'config.toml']) {
      const config = path.join(parent, '.cargo', name);
      if (fs.existsSync(config)) throw Error(`unreviewed inherited Cargo configuration: ${config}; use a fresh artifact directory outside this ancestor`);
    }
    if (path.dirname(parent) === parent) break;
  }
}
function compareFile(actual, expected) {
  const bytes = fs.readFileSync(actual);
  const checked = fs.readFileSync(path.join(root, expected));
  const comparison = { generated: path.relative(artifacts, actual), checked_in: expected,
    generated_sha256: sha(bytes), checked_in_sha256: sha(checked), byte_identical: bytes.equals(checked) };
  if (!comparison.byte_identical) throw Error(`stale generated implementation: ${expected}`);
  return comparison;
}
// External templates open the crate namespace even for locally opaque Rust helpers.
const boundaryName = (crate, item) => `${crate}.${item.lean_name}`;
function extract(label, manifest, basename, prefix, checked, allowedOpaque) {
  const directory = path.join(artifacts, label);
  fs.mkdirSync(directory);
  const llbc = path.join(directory, `${basename}.llbc`);
  const generated = path.join(directory, 'generated');
  fs.mkdirSync(generated);
  const record = { manifest, llbc: path.relative(artifacts, llbc), comparisons: [] };
  evidence.extractions[label] = record;
  run(`${label}-charon`, tool('charon', 'charon'), ['cargo', ...selection.configuration.charon_args,
    '--dest-file', llbc, '--', '--manifest-path', manifest, ...selection.configuration.cargo_args]);
  run(`${label}-aeneas`, tool('aeneas', 'aeneas'), [...selection.configuration.aeneas_args,
    '-split-files', '-gen-lib-entry', '-all-computable', '-dest', generated, llbc]);
  record.llbc_sha256 = fileSha(llbc);
  const translationFile = path.join(generated, 'translation.json');
  const translation = json(translationFile);
  record.translation_sha256 = fileSha(translationFile);
  if (translation.crate !== (label === 'kernel' ? 'noble_kernel' : 'noble_wasm')) throw Error(`wrong extraction crate: ${label}`);
  if (translation.aeneas_version !== aeneasRevision.slice(0, 7) || translation.charon_version !== '0.1.254') {
    throw Error(`unreviewed translator version in ${label}`);
  }
  for (const kind of ['functions', 'types', 'globals', 'trait_decls', 'trait_impls']) {
    if (!Array.isArray(translation[kind])) throw Error(`missing ${kind} translation inventory: ${label}`);
  }
  const local = translation.functions.filter(item => item.is_local);
  const opaque = local.filter(item => item.is_opaque);
  record.local_functions = local.length;
  record.local_opaque_functions = opaque;
  if (!local.length) throw Error(`empty local extraction: ${label}`);
  equal(opaque.map(item => item.rust_name).sort(), [...allowedOpaque].sort(), `unreviewed local opaque bodies: ${label}`);
  for (const item of local) {
    if (typeof item.is_opaque !== 'boolean') throw Error(`missing opacity classification: ${item.rust_name}`);
    if (!item.is_opaque && item.lean_file !== 'Funs.lean') throw Error(`untranslated local body: ${item.rust_name}`);
  }
  record.external_functions = translation.functions.filter(item => !item.is_local);
  record.external_types = translation.types.filter(item => item.lean_file === 'TypesExternal_Template.lean');
  record.translated_dependency_types = translation.types.filter(item => !item.is_local && item.lean_file === 'Types.lean');
  record.other_external_boundaries = ['globals', 'trait_decls', 'trait_impls'].flatMap(kind =>
    translation[kind].filter(item => item.lean_file?.includes('External')).map(item => ({ kind, ...item })));
  if (record.other_external_boundaries.length) throw Error(`unreviewed external global/trait boundary: ${label}`);
  record.local_definitions = [...local.filter(item => !item.is_opaque), ...translation.globals,
    ...translation.trait_impls].filter(item => item.is_local).map(item => item.lean_name).sort();
  for (const name of ['Types.lean', 'Funs.lean']) record.comparisons.push(compareFile(path.join(generated, name), `${checked}/${name}`));
  const entry = path.join(generated, `${prefix}.lean`);
  if (label === 'kernel') {
    // The inherited kernel entry has its regeneration runbook in comments. Its generated
    // Types/Funs are byte-exact; record this sole, explicit non-byte-exact comparison.
    const imports = file => fs.readFileSync(file, 'utf8').split('\n')
      .filter(line => line.trim() && !line.trimStart().startsWith('--')).join('\n');
    equal(imports(entry), imports(path.join(root, `${checked}.lean`)), 'changed inherited kernel entry imports');
    record.entry = { generated_sha256: fileSha(entry), checked_in_sha256: fileSha(path.join(root, `${checked}.lean`)),
      comparison: 'non-comment lines identical; inherited regeneration comments retained' };
  } else record.comparisons.push(compareFile(entry, `${checked}.lean`));
  return record;
}
function packagePins(proofRoot, phase, git) {
  const manifest = json(path.join(proofRoot, 'lake-manifest.json'));
  if (manifest.packagesDir !== '.lake/packages') throw Error('unreviewed Lean package directory');
  const packages = manifest.packages;
  equal(packages, json(path.join(root, 'proofs/m3/lake-manifest.json')).packages, 'unreviewed Lean dependency lock');
  const backend = packages.find(item => item.name === 'aeneas');
  if (backend?.rev !== aeneasRevision || backend?.inputRev !== aeneasRevision ||
      backend?.url !== selection.lean.backend_url || backend?.subDir !== selection.lean.backend_subdir) {
    throw Error('unreviewed Aeneas library pin');
  }
  return packages.map(pkg => {
    if (pkg.type !== 'git' || !/^[0-9a-f]{40}$/.test(pkg.rev) || !/^[A-Za-z0-9_-]+$/.test(pkg.name)) {
      throw Error(`unreviewed Lean package: ${pkg.name}`);
    }
    const directory = fs.realpathSync(path.join(root, 'proofs/m3/.lake/packages', pkg.name));
    const head = run(`${phase}-${pkg.name}-revision`, git, ['rev-parse', 'HEAD'], directory).trim();
    if (head !== pkg.rev) throw Error(`stale Lean dependency: ${pkg.name}`);
    const changes = run(`${phase}-${pkg.name}-clean`, git,
      ['status', '--porcelain=v1', '--untracked-files=all', '--ignore-submodules=none'], directory).trim();
    if (changes) throw Error(`unreviewed Lean dependency changes: ${pkg.name}; see ${phase}-${pkg.name}-clean.log`);
    return { ...pkg, directory, observed_revision: head };
  });
}

try {
  const selectionFile = path.join(root, 'policy/tool-selection.json');
  evidence.selection_sha256 = fileSha(selectionFile);
  selection = json(selectionFile);
  evidence.selection = selection;
  if (selection.schema_version !== 'noble-tool-selection-policy/v1') throw Error('unreviewed tool-selection schema');
  for (const [name, expected] of Object.entries(pins)) {
    if (selection.tool_paths[name]?.output !== expected) throw Error(`unreviewed selected tool: ${name}`);
  }
  equal(selection.configuration.charon_args, ['--preset=aeneas', '--error-on-warnings'], 'unreviewed Charon options');
  equal(selection.configuration.cargo_args, ['--lib', '--all-features', '--target', 'x86_64-unknown-linux-gnu', '--locked', '--offline'], 'unreviewed Cargo options');
  equal(selection.configuration.aeneas_args, ['-backend', 'lean', '-abort-on-error', '-warnings-as-errors', '-no-progress-bar', '-emit-json'], 'unreviewed Aeneas options');
  for (const [name, expected] of Object.entries({ target: 'x86_64-unknown-linux-gnu', word_bits: 64,
    extraction_rust: 'nightly-2026-08-18', extraction_profile: 'dev', all_features: true,
    default_features: true, overflow_checks: true, panic: 'unwind' })) {
    if (selection.configuration[name] !== expected) throw Error(`unreviewed extraction configuration: ${name}`);
  }
  equal(selection.configuration.features, [], 'unreviewed extraction features');
  if (selection.sources.aeneas.rev !== aeneasRevision || selection.sources.charon.rev !== 'b104e24fea7d721b71e6c39fd70f26ff20bc0980' ||
      selection.lean.toolchain !== 'leanprover/lean4:v4.31.0') throw Error('unreviewed extraction/library revision');
  const forbidden = /^(?:CHARON_|AENEAS_|LEAN_|LAKE_|RUSTC(?:_|$)|RUSTFLAGS$|RUSTDOCFLAGS$|RUSTUP_TOOLCHAIN$|CARGO_(?!HOME$)|LD_PRELOAD$|LD_LIBRARY_PATH$|GIT_(?:CONFIG|DIR|WORK_TREE|OBJECT|ALTERNATE))/;
  for (const [name, value] of Object.entries(process.env)) {
    if (value && forbidden.test(name)) throw Error(`unreviewed tool override: ${name}`);
  }
  requireNoCargoConfiguration(artifacts);
  fs.mkdirSync(workspace);
  const cargoHome = path.join(artifacts, 'cargo-home');
  fs.mkdirSync(cargoHome); // The locked workspace uses only local path dependencies; no ambient Cargo config/cache.
  environment = { ...process.env, RUSTC_WRAPPER: '', CARGO_HOME: cargoHome,
    CARGO_TARGET_DIR: path.join(artifacts, 'cargo-target'),
    PATH: `${pins.extraction_rust}/bin:${pins.lean}/bin:${process.env.PATH ?? ''}` };
  evidence.environment = { PATH: environment.PATH, HOME: environment.HOME,
    CARGO_HOME: cargoHome, CARGO_TARGET_DIR: environment.CARGO_TARGET_DIR, RUSTC_WRAPPER: '' };
  sourceFiles = snapshot();
  evidence.source_files = sourceFiles;
  evidence.source_revision = `sha256:${sha(JSON.stringify(sourceFiles))}`;
  save('source-files.json', sourceFiles);
  for (const [file, expected] of Object.entries(selection.file_sha256)) {
    if (sourceFiles[file] !== expected) throw Error(`stale selected configuration: ${file}`);
  }
  for (const file of ['Cargo.toml', 'Cargo.lock', 'flake.nix', 'flake.lock', 'rust-toolchain.toml', 'crates/noble-wasm/Cargo.toml']) {
    if (!selection.file_sha256[file]) throw Error(`selected configuration omits ${file}`);
  }
  // Both Rust and Lean consume these exact bytes in a fresh workspace, never old target/olean files.
  for (const [file, expected] of Object.entries(sourceFiles)) {
    const destination = path.join(workspace, file);
    fs.mkdirSync(path.dirname(destination), { recursive: true });
    fs.copyFileSync(path.join(root, file), destination);
    if (fileSha(destination) !== expected) throw Error(`source changed while staging: ${file}`);
  }
  const proofRoot = path.join(workspace, 'proofs/m3-wasm');
  if (fs.readFileSync(path.join(proofRoot, 'lean-toolchain'), 'utf8').trim() !== selection.lean.toolchain) throw Error('stale Lean toolchain');
  const git = (process.env.PATH ?? '').split(path.delimiter).map(dir => path.join(dir, 'git'))
    .find(file => { try { fs.accessSync(file, fs.constants.X_OK); return fs.statSync(file).isFile(); } catch { return false; } });
  if (!git) throw Error('git is required to inspect locked Lean dependency source identities');
  const binaries = { charon: tool('charon', 'charon'), aeneas: tool('aeneas', 'aeneas'),
    rustc: tool('extraction_rust', 'rustc'), cargo: tool('extraction_rust', 'cargo'),
    lean: tool('lean', 'lean'), lake: tool('lean', 'lake'), git };
  evidence.tools = Object.fromEntries(Object.entries(binaries).map(([name, binary]) => [name, identity(binary)]));
  evidence.versions.rust = run('rust-version', binaries.rustc, ['--version', '--verbose']);
  evidence.versions.charon = run('charon-version', binaries.charon, ['version']);
  evidence.versions.aeneas = run('aeneas-version', binaries.aeneas, ['-version']);
  evidence.versions.lean = run('lean-version', binaries.lean, ['--version']);
  evidence.versions.cargo = run('cargo-version', binaries.cargo, ['--version']);
  evidence.versions.lake = run('lake-version', binaries.lake, ['--version']);
  evidence.versions.git = run('git-version', binaries.git, ['--version']);
  evidence.dependencies = packagePins(proofRoot, 'before', git);
  const packageCache = fs.realpathSync(path.join(root, 'proofs/m3/.lake/packages'));
  fs.mkdirSync(path.join(proofRoot, '.lake'));
  fs.symlinkSync(packageCache, path.join(proofRoot, '.lake/packages'), 'dir');
  evidence.backend_model_files = Object.fromEntries(['Aeneas/Std/String.lean', 'Aeneas/Std/Array/Array.lean', 'Aeneas/Std/Core/Fmt.lean'].map(file =>
    [file, fileSha(path.join(packageCache, 'aeneas/backends/lean', file))]));
  const charonPin = fs.readFileSync(path.join(packageCache, 'aeneas/charon-pin'), 'utf8').split('\n')
    .map(line => line.trim()).filter(line => line && !line.startsWith('#'));
  equal(charonPin, [selection.sources.charon.rev], 'Aeneas upstream Charon pin mismatch');
  const kernel = extract('kernel', 'crates/noble-kernel/Cargo.toml', 'noble_kernel', 'NobleKernel', 'proofs/m3/NobleKernel', [
    'noble_kernel::types::impls::clone_stack', 'noble_kernel::types::impls::debug_stack',
    'noble_kernel::shapes::impls::clone_parts', 'noble_kernel::shapes::impls::debug_parts',
    'noble_kernel::shapes::impls::debug_slots',
  ]);
  const wasm = extract('wasm', 'crates/noble-wasm/Cargo.toml', 'noble_wasm_impl', 'NobleWasmImpl', 'proofs/m3-wasm/NobleWasmImpl', []);
  const expectedExternal = ['I64.Insts.CoreConvertFromU32.from', 'Usize.Insts.CoreConvertTryFromU32TryFromIntError.try_from',
    'U64.Insts.CoreConvertTryFromUsizeTryFromIntError.try_from', 'U8.Insts.CoreConvertTryFromU64TryFromIntError.try_from',
    'core.num.I64.unsigned_abs', 'core.slice.Slice.last', 'core.str.Str.as_bytes',
    'alloc.vec.Vec.pop', 'alloc.vec.Vec.is_empty',
    'noble_kernel.acceptance.check', 'noble_kernel.contracts.environment',
    'noble_kernel.types.Ty.Insts.CoreCloneClone.clone', 'noble_kernel.types.EffSet.is_empty'];
  equal(wasm.external_functions.map(item => item.lean_name).sort(), expectedExternal.sort(), 'unreviewed emitter external function boundary');
  equal(wasm.external_types.map(item => item.lean_name), ['noble_kernel.types.EffSet'], 'unreviewed emitter external type boundary');
  const auditInput = { schema: 'm3-extraction-audit/v1', local_definitions: wasm.local_definitions,
    external_functions: wasm.external_functions.map(item => boundaryName('noble_wasm', item)),
    external_types: wasm.external_types.map(item => boundaryName('noble_wasm', item)),
    kernel_boundary: [...kernel.external_functions, ...kernel.local_opaque_functions, ...kernel.external_types]
      .map(item => boundaryName('noble_kernel', item)) };
  save('audit-input.json', auditInput);
  evidence.audit_input_sha256 = fileSha(path.join(artifacts, 'audit-input.json'));
  run('lean-build', binaries.lake, ['build', 'NobleWasmImpl'], proofRoot);
  const audit = run('extraction-audit', binaries.lake, ['env', 'lean', 'ExtractionGate.lean'], proofRoot,
    { NOBLE_M3_EXTRACTION_SUBJECTS: path.join(artifacts, 'audit-input.json') });
  const records = audit.split('\n').filter(line => line.startsWith('M3-EXTRACTION '))
    .map(line => JSON.parse(line.slice('M3-EXTRACTION '.length)));
  if (records.length !== 1 || records[0].schema !== auditInput.schema || records[0].result !== 'passed') throw Error('missing compiled extraction audit verdict');
  evidence.audit = records[0];
  equal(evidence.audit.local_definitions, auditInput.local_definitions, 'incomplete compiled local definition audit');
  equal(evidence.audit.external_functions, auditInput.external_functions, 'incomplete compiled external function audit');
  equal(evidence.audit.external_types, auditInput.external_types, 'incomplete compiled external type audit');
  equal(evidence.audit.kernel_boundary, auditInput.kernel_boundary, 'incomplete compiled kernel boundary audit');
  evidence.dependencies_after = packagePins(proofRoot, 'after', git);
  equal(evidence.dependencies_after, evidence.dependencies, 'Lean dependency sources changed during gate');
  for (const [name, binary] of Object.entries(binaries)) equal(identity(binary), evidence.tools[name], `tool changed during gate: ${name}`);
  equal(snapshot(), sourceFiles, 'source changed during extraction gate');
  for (const [file, expected] of Object.entries(sourceFiles)) {
    if (fileSha(path.join(workspace, file)) !== expected) throw Error(`staged source changed during gate: ${file}`);
  }
  requireNoCargoConfiguration(workspace);
  evidence.result = 'passed';
  save('implementation.json', evidence);
  console.log(`M3-EXTRACTION-GATE passed: ${evidence.source_revision}`);
} catch (error) {
  evidence.result = 'failed';
  evidence.message = String(error);
  save('failure.json', evidence);
  console.error(`M3-EXTRACTION-GATE failed: ${error}; evidence: ${artifacts}/failure.json`);
  process.exitCode = 1;
}
