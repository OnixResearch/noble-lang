#!/usr/bin/env node
// Actual whole-crate Charon -> Aeneas -> Lean gate, extending MC1 and M3.
//
//   SELECTED_NODE verification/m4/implementation.mjs discover NEW_ARTIFACT_DIRECTORY
//   SELECTED_NODE verification/m4/implementation.mjs check NEW_ARTIFACT_DIRECTORY
//
// Discovery stages fresh generated Lean and produces review-candidate.json only
// after extraction, compilation, audits and refusals. It NEVER updates checked
// files or emits a passing gate receipt. If existing external models need new
// glue, failed discovery retains all three actual extractions for that work.
// After separately reviewing/renewing the generated files and transparent model
// glue, rerun discovery against the settled tree, review its complete inventory,
// and install review-candidate.json as verification/m4/extraction-lock.json.
// Check independently re-extracts, byte-compares generated files and compares
// every source/tool/inventory/dependency/axiom entry with that reviewed lock.
//
// The independent Octet all-target source-coverage/quality gates, historical
// MC1 application/source-equation gates and M3 semantic/refusal gates remain
// required integration checks; this gate neither replaces nor weakens them.
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { schema, sha, canonical, canonicalLlbcHash, fail, equal, exactBytes, lanes, sourcePolicy, account, auditPacket, auditPackets } from './accounting.mjs';
import { refusals } from './refusals.mjs';
import { companionPacket, companionCoverage } from '../mc2/extraction.mjs';
import { componentPacket, componentCoverage, resourceAuditPacket, resourceAudit,
  componentRefusals } from '../m5/extraction.mjs';
import { asyncPacket, asyncCoverage, asyncAuditPacket, asyncAudit,
  asyncRefusals } from '../m6/extraction.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const [mode, argument, ...extra] = process.argv.slice(2);
if (!['discover', 'check'].includes(mode) || !argument || extra.length) {
  console.log('usage: SELECTED_NODE verification/m4/implementation.mjs {discover|check} NEW_ARTIFACT_DIRECTORY');
  process.exit(mode === '--help' ? 0 : 2);
}
const artifacts = path.resolve(argument);
if (artifacts === root || artifacts.startsWith(`${root}${path.sep}`)) fail('ARTIFACTS', 'use a new directory outside the repository');
fs.mkdirSync(path.dirname(artifacts), { recursive: true });
fs.mkdirSync(artifacts); // Never overwrite or resume a previous run.
const workspace = path.join(artifacts, 'workspace');
const lockFile = 'verification/m4/extraction-lock.json';
const fileSha = file => sha(fs.readFileSync(file));
const json = file => JSON.parse(fs.readFileSync(file, 'utf8'));
const save = (name, value) => fs.writeFileSync(path.join(artifacts, name), JSON.stringify(value, null, 2) + '\n');
const pins = {
  charon: '/nix/store/ka61j6d6zy8zg91ynlbd6nmjssyynrpl-charon',
  aeneas: '/nix/store/q3hsccq1s53aa85vh3mmc3b0k31pnyl2-ocaml5.2.1-aeneas-0.1.0',
  extraction_rust: '/nix/store/wq92i79ivn1w1nrdhmzl1mqx0wcyrawi-rust-default-1.100.0-nightly-2026-08-18',
  lean: '/nix/store/hbxwgcbpkyqllqzq7cidhnvhgb60jb9q-noble-lean-release-4.31.0',
  node: '/nix/store/sy0c7j0npsq33d9zhnnzvjnzc52f4y0p-nodejs-24.13.0',
};
const aeneasRevision = '505b6ca35217e7be5c96c3e2f8045edfbdf47291';
const evidence = {
  schema: 'm4-implementation-evidence/v1', scope: 'actual-core-sync-and-async-component-bootstrap-pure-rust-extraction',
  mode, result: 'running', commands: [], extractions: {},
  assumptions: [
    'Selected Charon/Aeneas, Rust, Lean, Node, host OS and Nix store integrity are trusted tooling boundaries.',
    'Clean locked Lean dependency sources are checked before and after; upstream compiled dependency caches remain trusted.',
    'Local Rust declarations are joined by compiler IDs with Aeneas output. Expanded type aliases and compiler-generated destructor/vtable scaffolding are separately accounted, not authored-body exemptions or independent refinement evidence.',
    'Exactly the five unchanged inherited kernel copy/format helpers remain local opaque models, not extracted Rust bodies.',
    'Standard-library models abstract allocator failure, allocation identity, spare capacity and formatting layout; new external declarations are separately enumerated and audited.',
    'Layout-free collection models erase destructor effects and type-layout allocation limits. Extracted resource accounting is data, not a native release. The pure extraction uses the Global allocator; split_off does not model arbitrary effectful allocator Clone implementations.',
    'Mutable UTF-8 string backward updates assume the unchanged byte length and valid UTF-8 guaranteed by safe Rust borrowed str values; arbitrary mathematical replacements outside that representation invariant are not a Rust behavior claim.',
    'The inherited M4 native-evaluation allowance covers only generated/inherited literal UTF-8 byte-array size obligations; strict bridge/projection theorems cannot use it.',
    'MC2 separately audits its explicitly named closed extracted-source equations and fixed literal size obligations. These finite native equations are not universal source-refinement theorems.',
    'MC2 assumes an authentic, sound trusted host constructs CheckObservation for the exact independently checked statement, declaration and evidence class; arbitrary trusted Rust callers can forge such observations, while guest ingress cannot grant itself that authority.',
    'The explicit production resource-free test environment uses test.emit Text-- and test.abort; the historical kernel fixture environment keeps its different test.emit Text--Unit contract. No audit silently equates them.',
    'M5 requires universal correspondence between the actual extracted resource transition/production Table.decide and the total reference steps, plus strict semantic lifecycle/accounting invariants. Host serialization, namespace freshness, authentic callbacks and applying returned accounting once remain embedding obligations.',
    'M6 extends the same full inventory with native async transitions, retained Table.decide, schema classifier and strict ownership/accounting correspondence. Checked-machine Result correspondence is not a global panic-freedom theorem. Serialized authentic callbacks and truthful native-stop/cleanup notifications remain embedding obligations.',
    'The transparent try_reserve_exact external model preserves vector elements and checks element-count overflow; like the inherited layout-free vector model, it abstracts successful physical allocation, byte layout and OOM, not native storage or admission availability.',
  ],
  non_claims: [
    'No universal frontend inference, immutable namespace/session, acceptance-to-execution or backend refinement theorem.',
    'No trust is conferred by the public untrusted kernel execution metadata or by a source/frontend acceptance flag.',
    'No WAT/Wasm runtime, optimizer, assembler, engine, loader, host environment or ABI correctness theorem.',
    'No physical host/native release, arbitrary authority-system refinement or externally authenticated observation theorem.',
    'No execution/conformance result, source-inventory policy discharge, M4 milestone completion, MC2 or M5 closure.',
  ],
};
let selection;
let environment;
let sourceFiles;
let expectedLock;
const tool = (name, binary) => path.join(selection.tool_paths[name].output, 'bin', binary);
const skipped = new Set(['.git', '.lake', '.lake-packages', 'target', 'node_modules']);
function files(directory) {
  return fs.readdirSync(path.join(root, directory), { withFileTypes: true }).flatMap(entry => {
    if (skipped.has(entry.name)) return [];
    const file = path.posix.join(directory, entry.name);
    if (entry.isSymbolicLink()) fail('SOURCE', `unbound source symlink: ${file}`);
    if (entry.isDirectory()) return files(file);
    if (!entry.isFile()) fail('SOURCE', `non-file source: ${file}`);
    return [file];
  });
}
function snapshot() {
  const input = new Set([
    ...Object.keys(selection.file_sha256), 'policy/tool-selection.json', 'policy/source-inventory.json',
    'nix/tool-selection-files.nix', 'nix/source-inventory-derive.nix', 'nix/source-inventory.nix',
    'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml',
    ...files('crates'),
    ...['proofs/m3', 'proofs/mc1', 'proofs/mc2', 'proofs/m3-wasm', 'proofs/m4', 'proofs/m5', 'proofs/m6'].flatMap(directory =>
      files(directory).filter(file => /\.(lean|toml|json)$/.test(file) || file.endsWith('/lean-toolchain'))),
    ...['verification/m4', 'verification/m5', 'verification/m6'].flatMap(directory =>
      files(directory).filter(file => file.endsWith('.mjs'))),
    'verification/mc2/extraction.mjs',
    ...['increment', 'guarded'].flatMap(fixture => ['contract', 'proof.lean']
      .map(extension => `verification/mc2/contracts/${fixture}.${extension}`)),
    'verification/m4/inherited-boundaries.json',
    'verification/mc1/implementation.mjs', 'verification/m3-wasm/implementation.mjs',
    'verification/m3-coverage-join.mjs', 'verification/m3-coverage-gate.sh',
    'verification/m3-proof-gate.sh', 'verification/m3-proof-gate-refusals.sh',
  ]);
  if (input.has(lockFile)) fail('LOCK', 'do not recursively include the extraction lock in tool-selection file bindings');
  return Object.fromEntries([...input].sort().map(file => {
    if (path.isAbsolute(file) || file.split('/').includes('..')) fail('SOURCE', file);
    if (fs.lstatSync(path.join(root, file)).isSymbolicLink()) fail('SOURCE', `source symlink: ${file}`);
    return [file, fileSha(path.join(root, file))];
  }));
}
function identity(binary) {
  return { path: binary, realpath: fs.realpathSync(binary), sha256: fileSha(binary) };
}
function noCargoConfiguration(directory) {
  for (let parent = path.resolve(directory); ; parent = path.dirname(parent)) {
    for (const name of ['config', 'config.toml']) {
      if (fs.existsSync(path.join(parent, '.cargo', name))) fail('CARGO-CONFIG', `${parent}/.cargo/${name}`);
    }
    if (path.dirname(parent) === parent) break;
  }
}
function run(label, binary, args, cwd = workspace, extraEnvironment = {}, allowFailure = false,
  timeout = selection.configuration.lean_timeout_seconds * 1000) {
  const log = `${label}.log`;
  const fd = fs.openSync(path.join(artifacts, log), 'wx');
  const command = { label, binary, args, cwd, environment: extraEnvironment, log,
    started_at: new Date().toISOString(), timeout_ms: timeout };
  evidence.commands.push(command);
  save('commands.json', evidence.commands);
  let result;
  try {
    result = spawnSync(binary, args, { cwd, env: { ...environment, ...extraEnvironment },
      stdio: ['ignore', fd, fd], timeout });
  } finally {
    fs.closeSync(fd);
    command.finished_at = new Date().toISOString();
    command.exit = result?.status ?? null;
    command.signal = result?.signal ?? null;
    command.error = result?.error ? String(result.error) : null;
    command.output_sha256 = fileSha(path.join(artifacts, log));
    save('commands.json', evidence.commands);
  }
  if (!allowFailure && (command.error || command.exit !== 0)) fail('COMMAND', `${label}; see ${log}: ${command.error ?? command.exit}`);
  return { ...command, output: fs.readFileSync(path.join(artifacts, log), 'utf8') };
}
function validateSelection() {
  equal(selection.schema_version, 'noble-tool-selection-policy/v1', 'SELECTION', 'schema');
  for (const [name, pin] of Object.entries(pins)) equal(selection.tool_paths[name]?.output, pin, 'TOOL', name);
  equal(selection.configuration.charon_args, ['--preset=aeneas', '--error-on-warnings'], 'SELECTION', 'Charon arguments');
  equal(selection.configuration.cargo_args, ['--lib', '--all-features', '--target',
    'x86_64-unknown-linux-gnu', '--locked', '--offline'], 'SELECTION', 'Cargo arguments');
  equal(selection.configuration.aeneas_args, ['-backend', 'lean', '-abort-on-error', '-warnings-as-errors',
    '-no-progress-bar', '-emit-json'], 'SELECTION', 'Aeneas arguments');
  for (const [name, expected] of Object.entries({ target: 'x86_64-unknown-linux-gnu', word_bits: 64,
    extraction_rust: 'nightly-2026-08-18', extraction_profile: 'dev', all_features: true,
    default_features: true, overflow_checks: true, panic: 'unwind', lean_heartbeats: 1000000,
    lean_recursion_depth: 2048, lean_timeout_seconds: 900, translator_timeout_seconds: 120 })) {
    equal(selection.configuration[name], expected, 'SELECTION', name);
  }
  equal(selection.configuration.features, [], 'SELECTION', 'features');
  equal(selection.sources.aeneas.rev, aeneasRevision, 'SELECTION', 'Aeneas source');
  equal(selection.sources.charon.rev, 'b104e24fea7d721b71e6c39fd70f26ff20bc0980', 'SELECTION', 'Charon source');
  equal(selection.lean.toolchain, 'leanprover/lean4:v4.31.0', 'SELECTION', 'Lean toolchain');
  for (const name of ['lakefile.toml', 'lake-manifest.json', 'lean-toolchain']) {
    if (!selection.file_sha256[`proofs/m4/${name}`]) fail('SELECTION', `missing proofs/m4/${name} binding`);
  }
  if (fs.realpathSync(process.execPath) !== fs.realpathSync(tool('node', 'node'))) fail('TOOL', 'invoke this gate with the selected Node executable');
}
function packagePins(proofRoot, phase, git) {
  const manifest = json(path.join(proofRoot, 'lake-manifest.json'));
  if (manifest.packagesDir !== '.lake/packages') fail('DEPENDENCIES', 'unreviewed packages directory');
  equal(manifest.packages, json(path.join(workspace, 'proofs/m3/lake-manifest.json')).packages,
    'DEPENDENCIES', 'M4 does not share the exact inherited lock');
  const packages = manifest.packages;
  if (!Array.isArray(packages) || !packages.length || new Set(packages.map(pkg => pkg.name)).size !== packages.length) fail('DEPENDENCIES', 'duplicate/empty lock');
  const backend = packages.find(pkg => pkg.name === 'aeneas');
  if (backend?.rev !== aeneasRevision || backend.inputRev !== aeneasRevision ||
      backend.url !== selection.lean.backend_url || backend.subDir !== selection.lean.backend_subdir) fail('DEPENDENCIES', 'Aeneas pin');
  return packages.map(pkg => {
    if (pkg.type !== 'git' || !/^[0-9a-f]{40}$/.test(pkg.rev) || !/^[A-Za-z0-9_-]+$/.test(pkg.name) || pkg.path !== undefined) fail('DEPENDENCIES', `package ${pkg.name}`);
    const directory = path.join(root, 'proofs/m3/.lake/packages', pkg.name);
    if (fs.lstatSync(directory).isSymbolicLink()) fail('DEPENDENCIES', `symlinked package ${pkg.name}`);
    const actual = fs.realpathSync(directory);
    const revision = run(`${phase}-${pkg.name}-revision`, git, ['--no-optional-locks', '--no-replace-objects',
      'rev-parse', 'HEAD'], actual).output.trim();
    equal(revision, pkg.rev, 'DEPENDENCIES', pkg.name);
    const dirty = run(`${phase}-${pkg.name}-clean`, git, ['--no-optional-locks', '--no-replace-objects',
      '-c', 'core.fsmonitor=false', 'status', '--porcelain=v1', '--untracked-files=all', '--ignore-submodules=none'], actual).output.trim();
    if (dirty) fail('DEPENDENCIES', `${pkg.name}: dirty source`);
    return { ...pkg, directory: actual, observed_revision: revision };
  });
}

try {
  selection = json(path.join(root, 'policy/tool-selection.json'));
  validateSelection();
  evidence.selection = selection;
  const forbidden = /^(?:CHARON_|AENEAS_|LEAN_|LAKE_|NOBLE_M[45]_|RUSTC(?:_|$)|RUSTFLAGS$|RUSTDOCFLAGS$|RUSTUP_TOOLCHAIN$|CARGO_(?!HOME$)|LD_PRELOAD$|LD_LIBRARY_PATH$|NODE_OPTIONS$|NODE_PATH$|GIT_(?:CONFIG|DIR|WORK_TREE|OBJECT|ALTERNATE))/;
  for (const [name, value] of Object.entries(process.env)) if (value && forbidden.test(name)) fail('OVERRIDE', name);
  noCargoConfiguration(artifacts);
  fs.mkdirSync(workspace);
  const cargoHome = path.join(artifacts, 'cargo-home');
  const home = path.join(artifacts, 'home');
  fs.mkdirSync(cargoHome);
  fs.mkdirSync(home);
  environment = { ...process.env, HOME: home, CARGO_HOME: cargoHome, RUSTC_WRAPPER: '',
    CARGO_TARGET_DIR: path.join(artifacts, 'cargo-target'), GIT_CONFIG_NOSYSTEM: '1', GIT_CONFIG_GLOBAL: '/dev/null',
    PATH: `${pins.extraction_rust}/bin:${pins.lean}/bin:${process.env.PATH ?? ''}` };
  evidence.environment = { HOME: home, CARGO_HOME: cargoHome, CARGO_TARGET_DIR: environment.CARGO_TARGET_DIR,
    PATH: environment.PATH, RUSTC_WRAPPER: '', GIT_CONFIG_NOSYSTEM: '1', GIT_CONFIG_GLOBAL: '/dev/null' };
  sourceFiles = snapshot();
  evidence.source_files = sourceFiles;
  evidence.source_revision = `sha256:${sha(canonical(sourceFiles))}`;
  sourcePolicy(sourceFiles, file => fs.readFileSync(path.join(root, file), 'utf8'));
  for (const [file, expected] of Object.entries(selection.file_sha256)) equal(sourceFiles[file], expected, 'SELECTION', `stale ${file}`);
  save('source-files.json', sourceFiles);
  if (mode === 'check') {
    if (!fs.existsSync(path.join(root, lockFile))) fail('LOCK', 'missing reviewed extraction lock; use discovery, do not invent counts');
    if (fs.lstatSync(path.join(root, lockFile)).isSymbolicLink()) fail('LOCK', 'symlinked review');
    expectedLock = json(path.join(root, lockFile));
    equal(expectedLock.schema, schema, 'LOCK', 'schema');
    equal(expectedLock.source_files, sourceFiles, 'SOURCE', 'reviewed source/tool/model/configuration drift');
    evidence.reviewed_lock_sha256 = fileSha(path.join(root, lockFile));
  }
  const staged = { ...sourceFiles };
  for (const [file, expected] of Object.entries(sourceFiles)) {
    const destination = path.join(workspace, file);
    fs.mkdirSync(path.dirname(destination), { recursive: true });
    fs.copyFileSync(path.join(root, file), destination);
    equal(fileSha(destination), expected, 'SOURCE', `changed while staging ${file}`);
  }
  const git = (process.env.PATH ?? '').split(path.delimiter).map(directory => path.join(directory, 'git'))
    .find(file => { try { fs.accessSync(file, fs.constants.X_OK); return fs.statSync(file).isFile(); } catch { return false; } });
  if (!git) fail('TOOL', 'git is required to inspect exact locked dependency sources');
  const binaries = { charon: tool('charon', 'charon'), aeneas: tool('aeneas', 'aeneas'),
    rustc: tool('extraction_rust', 'rustc'), cargo: tool('extraction_rust', 'cargo'),
    lean: tool('lean', 'lean'), lake: tool('lean', 'lake'), node: process.execPath, git };
  evidence.tools = Object.fromEntries(Object.entries(binaries).map(([name, binary]) => [name, identity(binary)]));
  if (expectedLock) equal(evidence.tools, expectedLock.tools, 'TOOL', 'reviewed binary identity drift');
  evidence.versions = Object.fromEntries(Object.entries(binaries).map(([name, binary]) => [name,
    run(`${name}-version`, binary, name === 'charon' ? ['version'] : name === 'aeneas' ? ['-version'] :
      name === 'rustc' ? ['--version', '--verbose'] : ['--version'], workspace, {}, false, 30_000).output]));
  const proofRoot = path.join(workspace, 'proofs/m4');
  const dependencies = packagePins(proofRoot, 'before', git);
  evidence.dependencies = dependencies;
  const cache = fs.realpathSync(path.join(root, 'proofs/m3/.lake/packages'));
  fs.mkdirSync(path.join(proofRoot, '.lake'));
  fs.symlinkSync(cache, path.join(proofRoot, '.lake/packages'), 'dir');
  const charonPin = fs.readFileSync(path.join(cache, 'aeneas/charon-pin'), 'utf8').split('\n')
    .map(line => line.trim()).filter(line => line && !line.startsWith('#'));
  equal(charonPin, [selection.sources.charon.rev], 'DEPENDENCIES', 'Aeneas upstream Charon pin');
  evidence.backend_model_files = Object.fromEntries(['Aeneas/Std/String.lean', 'Aeneas/Std/Array/Array.lean',
    'Aeneas/Std/Core/Fmt.lean'].map(file => [file, fileSha(path.join(cache, 'aeneas/backends/lean', file))]));
  const raw = {};
  const accounts = {};
  const generatedFiles = {};
  const extractionBindings = {};
  const stale = [];
  for (const lane of lanes) {
    const directory = path.join(artifacts, lane.id);
    const generated = path.join(directory, 'generated');
    fs.mkdirSync(directory);
    fs.mkdirSync(generated);
    const llbc = path.join(directory, `${lane.basename}.llbc`);
    const manifest = `crates/${lane.package}/Cargo.toml`;
    run(`${lane.id}-charon`, binaries.charon, ['cargo', ...selection.configuration.charon_args,
      '--dest-file', llbc, '--', '--manifest-path', manifest, ...selection.configuration.cargo_args], workspace,
    {}, false, selection.configuration.translator_timeout_seconds * 1000);
    run(`${lane.id}-aeneas`, binaries.aeneas, [...selection.configuration.aeneas_args,
      '-split-files', '-gen-lib-entry', '-all-computable', '-dest', generated, llbc], workspace,
    {}, false, selection.configuration.translator_timeout_seconds * 1000);
    raw[lane.id] = { llbc: json(llbc), translation: json(path.join(generated, 'translation.json')) };
    accounts[lane.id] = account(lane, raw[lane.id].llbc, raw[lane.id].translation, sourceFiles);
    generatedFiles[lane.id] = Object.fromEntries(fs.readdirSync(generated).sort().map(file => {
      const target = path.join(generated, file);
      if (!fs.lstatSync(target).isFile()) fail('GENERATED', `unexpected generated entry ${file}`);
      return [file, fileSha(target)];
    }));
    const comparisons = [];
    for (const [file, checked] of [['Types.lean', `${lane.checked}/Types.lean`],
      ['Funs.lean', `${lane.checked}/Funs.lean`], [`${lane.module}.lean`, `${lane.checked}.lean`]]) {
      const actual = fs.readFileSync(path.join(generated, file));
      const expected = fs.readFileSync(path.join(root, checked));
      const same = actual.equals(expected);
      comparisons.push({ generated: file, checked, generated_sha256: sha(actual), checked_sha256: sha(expected),
        comparison: 'byte-identical', identical: same });
      if (!same) stale.push(checked);
      if (mode === 'check') exactBytes(actual, expected, checked);
      if (mode === 'discover') {
        fs.writeFileSync(path.join(workspace, checked), actual);
        staged[checked] = sha(actual);
      }
    }
    extractionBindings[lane.id] = { canonical_llbc_sha256: canonicalLlbcHash(raw[lane.id].llbc),
      translation_sha256: fileSha(path.join(generated, 'translation.json')), comparisons,
      generated: generatedFiles[lane.id] };
    // Physical artifact bytes remain independently retained and hashed, but their
    // output path and unordered name-map serialization cannot define a lock.
    evidence.extractions[lane.id] = { llbc_sha256: fileSha(llbc), ...extractionBindings[lane.id] };
    save('translation-inventory.json', accounts);
    save('extraction-progress.json', evidence.extractions);
  }
  const baseline = json(path.join(workspace, 'verification/m4/inherited-boundaries.json'));
  equal(baseline.schema, 'm4-inherited-boundaries/v1', 'BASELINE', 'schema');
  const packet = auditPacket(accounts, baseline.models);
  const mc2Subjects = companionPacket(accounts, sourceFiles, raw.contracts.llbc);
  const m5Subjects = componentPacket(accounts, sourceFiles, packet);
  packet.roots.push(...mc2Subjects.roots, ...m5Subjects.roots);
  const m6Subjects = asyncPacket(accounts, sourceFiles, packet);
  for (const entry of m6Subjects.roots) {
    if (!packet.roots.some(root => root.declaration === entry.declaration && root.owner === entry.owner)) {
      packet.roots.push(entry);
    }
  }
  const packets = auditPackets(packet);
  save('mc2-subjects.json', mc2Subjects);
  save('m5-subjects.json', m5Subjects);
  save('m6-subjects.json', m6Subjects);
  save('audit-input.json', packet);
  for (const [lane, input] of Object.entries(packets)) save(`audit-input-${lane}.json`, input);
  evidence.boundary_renewal = { provenance: baseline.provenance,
    newly_required_models: packet.newly_required_models, retained_unrequested_models: packet.retained_unrequested_models };
  evidence.audit_input_sha256 = fileSha(path.join(artifacts, 'audit-input.json'));
  evidence.stale_generated_files = stale;
  if (expectedLock) {
    equal(accounts, expectedLock.inventory, 'INVENTORY', 'missing/extra/changed compiler-to-translator declarations');
    equal(generatedFiles, expectedLock.generated_files, 'GENERATED', 'unreviewed generated outputs/templates');
  }
  run('lean-build', binaries.lake, ['build', 'M4Audit', 'NobleContractImpl.Projection', 'NobleWasmImpl'], proofRoot);
  const audit = {};
  for (const [lane, entrypoint] of [['contracts', 'ExtractionGate.lean'], ['wasm', 'WasmExtractionGate.lean']]) {
    const input = packets[lane];
    const observed = run(`extraction-audit-${lane}`, binaries.lake, ['env', 'lean', entrypoint], proofRoot,
      { NOBLE_M4_EXTRACTION_SUBJECTS: path.join(artifacts, `audit-input-${lane}.json`) });
    const records = observed.output.split('\n').filter(line => line.startsWith('M4-EXTRACTION '))
      .map(line => JSON.parse(line.slice('M4-EXTRACTION '.length)));
    if (records.length !== 1 || records[0].schema !== input.schema || records[0].lane !== lane ||
        records[0].result !== 'passed') fail('VERDICT', `missing compiled ${lane} audit verdict`);
    audit[lane] = records[0];
    for (const field of ['declarations', 'boundaries', 'bridges', 'retained_models', 'roots']) {
      equal(audit[lane][field], input[field], 'VERDICT', `incomplete ${lane} ${field}`);
    }
    equal(audit[lane].root_audits.map(row => row.declaration), input.roots.map(row => row.declaration),
      'VERDICT', `incomplete ${lane} root audits`);
  }
  evidence.audit = audit;
  const mc2Root = path.join(workspace, 'proofs/mc2');
  fs.mkdirSync(path.join(mc2Root, '.lake'));
  fs.symlinkSync(cache, path.join(mc2Root, '.lake/packages'), 'dir');
  equal(json(path.join(mc2Root, 'lake-manifest.json')).packages,
    json(path.join(proofRoot, 'lake-manifest.json')).packages,
    'DEPENDENCIES', 'MC2 must share the exact audited dependency lock');
  run('mc2-correspondence-build', binaries.lake, ['build', 'MC2ExtractionGate'], mc2Root);
  const correspondenceRun = run('mc2-correspondence-audit', binaries.lake,
    ['env', 'lean', 'MC2ExtractionGate.lean'], mc2Root);
  const correspondenceRows = correspondenceRun.output.split('\n')
    .filter(line => line.startsWith('MC2-EXTRACTION '))
    .map(line => JSON.parse(line.slice('MC2-EXTRACTION '.length)));
  if (correspondenceRows.length !== 1 ||
      correspondenceRows[0].schema !== 'mc2-extraction-correspondence/v1' ||
      correspondenceRows[0].result !== 'passed') {
    fail('MC2-VERDICT', 'missing compiled actual-source correspondence audit');
  }
  const sourceFixtures = correspondenceRows[0].source_fixtures;
  equal(sourceFixtures?.map(row => row.fixture), ['increment', 'guarded'],
    'MC2-FIXTURE', 'closed source equations must identify the real independently checked fixtures');
  for (const fixture of sourceFixtures) {
    for (const [field, extension] of [['source', 'contract'], ['declaration', 'proof.lean']]) {
      equal(fixture[field], fs.readFileSync(path.join(workspace,
        `verification/mc2/contracts/${fixture.fixture}.${extension}`), 'utf8'),
      'MC2-FIXTURE', `${fixture.fixture}: closed equation ${field} differs from the real host-check input`);
    }
  }
  evidence.mc2 = { coverage: companionCoverage(mc2Subjects, audit.contracts),
    correspondence: correspondenceRows[0] };
  if (expectedLock) equal(evidence.mc2, expectedLock.mc2, 'MC2-VERDICT',
    'unreviewed companion body, boundary or correspondence theorem');
  const m5Root = path.join(workspace, 'proofs/m5');
  fs.mkdirSync(path.join(m5Root, '.lake'));
  fs.symlinkSync(cache, path.join(m5Root, '.lake/packages'), 'dir');
  equal(fs.readFileSync(path.join(m5Root, 'lean-toolchain'), 'utf8').trim(),
    selection.lean.toolchain, 'DEPENDENCIES', 'M5 Lean toolchain');
  equal(packagePins(m5Root, 'm5-before', git), dependencies,
    'DEPENDENCIES', 'M5 must share the exact audited dependency lock and sources');
  const m5Coverage = componentCoverage(m5Subjects, audit);
  save('m5-compiled-coverage.json', m5Coverage);
  const m5Packet = resourceAuditPacket(m5Subjects, {
    source_revision: evidence.source_revision, source_files: sourceFiles, staged_source_files: staged,
    tools: evidence.tools, dependencies, extractions: extractionBindings,
    inventory_sha256: sha(canonical(accounts)), canonical_audit_input_sha256: evidence.audit_input_sha256,
    component_subjects_sha256: fileSha(path.join(artifacts, 'm5-subjects.json')),
  });
  evidence.m5 = { coverage: m5Coverage, resource_refinement: resourceAudit({
    packet: m5Packet, run, proofRoot: m5Root, lake: binaries.lake, artifacts, save }) };
  if (expectedLock) equal(evidence.m5, expectedLock.m5, 'M5-VERDICT',
    'unreviewed resource/authority/component body, model, dependency or strict theorem');
  const m6Root = path.join(workspace, 'proofs/m6');
  fs.mkdirSync(path.join(m6Root, '.lake'));
  fs.symlinkSync(cache, path.join(m6Root, '.lake/packages'), 'dir');
  equal(fs.readFileSync(path.join(m6Root, 'lean-toolchain'), 'utf8').trim(),
    selection.lean.toolchain, 'DEPENDENCIES', 'M6 Lean toolchain');
  equal(packagePins(m6Root, 'm6-before', git), dependencies,
    'DEPENDENCIES', 'M6 must share the exact audited dependency lock and sources');
  const m6Coverage = asyncCoverage(m6Subjects, audit);
  save('m6-compiled-coverage.json', m6Coverage);
  const m6Packet = asyncAuditPacket(m6Subjects, {
    source_revision: evidence.source_revision, source_files: sourceFiles, staged_source_files: staged,
    tools: evidence.tools, dependencies, extractions: extractionBindings,
    inventory_sha256: sha(canonical(accounts)), canonical_audit_input_sha256: evidence.audit_input_sha256,
    async_subjects_sha256: fileSha(path.join(artifacts, 'm6-subjects.json')),
  });
  evidence.m6 = { coverage: m6Coverage, async_refinement: asyncAudit({
    packet: m6Packet, run, proofRoot: m6Root, lake: binaries.lake, artifacts, save }) };
  if (expectedLock) equal(evidence.m6, expectedLock.m6, 'M6-VERDICT',
    'unreviewed async extraction or strict correspondence');
  if (expectedLock) equal(audit, expectedLock.audit, 'DEPENDENCIES', 'unexpected dependency, axiom, theorem or compiled declaration');
  evidence.refusals = refusals({ raw, accounts, sources: sourceFiles, tools: evidence.tools, packet, packets, audit, run, proofRoot,
    lake: binaries.lake, artifacts, save });
  evidence.m5_refusals = componentRefusals({ raw, accounts, sources: sourceFiles, packet, subjects: m5Subjects,
    coverage: audit, proofPacket: m5Packet, run, proofRoot: m5Root, lake: binaries.lake, artifacts, save });
  evidence.m6_refusals = asyncRefusals({ raw, accounts, sources: sourceFiles, packet,
    subjects: m6Subjects, coverage: audit, proofPacket: m6Packet,
    run, proofRoot: m6Root, lake: binaries.lake, artifacts, save });
  equal(packagePins(m6Root, 'm6-after', git), dependencies, 'DEPENDENCIES', 'M6 Lean source changed during gate');
  equal(packagePins(m5Root, 'm5-after', git), dependencies, 'DEPENDENCIES', 'M5 Lean source changed during gate');
  equal(packagePins(proofRoot, 'after', git), dependencies, 'DEPENDENCIES', 'Lean source changed during gate');
  for (const [name, binary] of Object.entries(binaries)) equal(identity(binary), evidence.tools[name], 'TOOL', name);
  equal(snapshot(), sourceFiles, 'SOURCE', 'repository changed during gate');
  for (const [file, expected] of Object.entries(staged)) equal(fileSha(path.join(workspace, file)), expected, 'SOURCE', `staged source changed: ${file}`);
  noCargoConfiguration(workspace);
  if (expectedLock) equal(fileSha(path.join(root, lockFile)), evidence.reviewed_lock_sha256, 'LOCK', 'reviewed lock changed during gate');
  const candidate = { schema, source_files: sourceFiles, tools: evidence.tools, inventory: accounts,
    generated_files: generatedFiles, audit, mc2: evidence.mc2, m5: evidence.m5, m6: evidence.m6 };
  if (mode === 'discover') {
    evidence.result = 'review-required';
    save('review-candidate.json', candidate);
    save('discovery.json', evidence);
    console.log(`M4-DISCOVERY review-required: ${evidence.source_revision}; no gate acceptance claimed`);
  } else {
    equal(candidate, expectedLock, 'LOCK', 'unreviewed lock field or missing required coverage');
    evidence.result = 'passed';
    save('implementation.json', evidence);
    console.log(`M4-IMPLEMENTATION-GATE passed: ${evidence.source_revision}`);
  }
} catch (error) {
  evidence.result = 'failed';
  evidence.message = String(error);
  save('failure.json', evidence);
  console.error(`M4-IMPLEMENTATION-GATE failed: ${error}; see ${artifacts}/failure.json`);
  process.exitCode = 1;
}
