#!/usr/bin/env node
// Allocator admission matrix runner (#121).
//
//   SELECTED_NODE verification/allocator-admission/run.mjs NEW_ARTIFACT_DIRECTORY \
//     [--stable-prefix PATH --stable-tarball FILE --stable-published-sha256 HEX]
//
// For each fixture root this executes the *actual* selected pipeline and records every
// stage separately: released-Rust-1.100.0 source compilation, the extraction nightly's
// `cargo check` (ungated and gated), Charon extraction bounded by
// `--start-from <crate>::<root>`, Aeneas translation with `-emit-json`, a Lean build of
// the generated files against the pinned Lean library, and finally Noble's allocator
// admission decision (`admission.mjs`).
//
// What each stage can and cannot tell you:
//   * source compilation only proves the Rust language surface accepts the fixture;
//   * a passing Charon/Aeneas/Lean run proves the translation type-checks, not that the
//     allocator was modelled: the selected preset erases it (`hide_allocator: true`), so
//     `Vec::new_in` becomes an unmodelled axiom and Lean accepts it;
//   * only the admission stage refuses. A rustc success is never reported as extraction
//     or refinement acceptance anywhere in this receipt.
//
// The expectations in `cells` are the pre-registered decision from `decision.md`; the
// runner fails if an observed verdict differs, or if a refusal arrives without the codes
// that family must produce.
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { admit, explicitConstructors, revalidate, schema as admissionSchema, verdict } from './admission.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const usage = 'usage: SELECTED_NODE verification/allocator-admission/run.mjs NEW_ARTIFACT_DIRECTORY '
  + '[--stable-prefix PATH --stable-tarball FILE --stable-published-sha256 HEX]';

function parseArguments(argv) {
  const positional = [];
  const options = {};
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (!argument.startsWith('--')) { positional.push(argument); continue; }
    const name = argument.slice(2);
    if (!['stable-prefix', 'stable-tarball', 'stable-published-sha256'].includes(name)) {
      console.log(usage); process.exit(2);
    }
    options[name] = argv[index + 1];
    index += 1;
  }
  if (positional.length !== 1) { console.log(usage); process.exit(2); }
  return { artifacts: path.resolve(positional[0]), options };
}

const { artifacts, options } = parseArguments(process.argv.slice(2));
if (artifacts === root || artifacts.startsWith(`${root}${path.sep}`)) {
  throw Error('refusing to write artifacts inside the repository; pass a new directory outside it');
}
fs.mkdirSync(path.dirname(artifacts), { recursive: true });
fs.mkdirSync(artifacts); // Never overwrite or resume a previous run.

const selection = JSON.parse(fs.readFileSync(path.join(root, 'policy/tool-selection.json'), 'utf8'));
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const fileSha = file => sha(fs.readFileSync(file));
const normalize = text => String(text)
  .split(root).join('<repo>')
  .split(path.dirname(path.dirname(path.dirname(artifacts)))).join('<campaign>')
  .split(process.env.HOME ?? '/home/brittonr').join('~');

// The fixture family. Each root is one cell; `expectation` and `required_codes` are the
// pre-registered decision, not an observation.
export const cells = [
  { root: 'vec_baseline', family: 'implicit-global', expectation: 'admitted', required_codes: [] },
  { root: 'box_baseline', family: 'implicit-global', expectation: 'admitted', required_codes: [] },
  { root: 'vec_growth_clone_baseline', family: 'implicit-global', expectation: 'admitted', required_codes: [] },
  { root: 'global_vec', family: 'explicit-global', expectation: 'refused',
    required_codes: ['ALLOCATOR-SOURCE', 'ALLOCATOR-CONSTRUCTOR', 'ALLOCATOR-AXIOM'] },
  { root: 'global_box', family: 'explicit-global', expectation: 'refused',
    required_codes: ['ALLOCATOR-SOURCE', 'ALLOCATOR-CONSTRUCTOR', 'ALLOCATOR-AXIOM'] },
  { root: 'generic_vec', family: 'generic-allocator', expectation: 'refused',
    required_codes: ['ALLOCATOR-SOURCE', 'ALLOCATOR-CONSTRUCTOR', 'ALLOCATOR-AXIOM'] },
  { root: 'generic_box', family: 'generic-allocator', expectation: 'refused',
    required_codes: ['ALLOCATOR-SOURCE', 'ALLOCATOR-CONSTRUCTOR', 'ALLOCATOR-AXIOM'] },
  { root: 'generic_vec_growth_clone', family: 'generic-allocator', expectation: 'refused',
    required_codes: ['ALLOCATOR-SOURCE', 'ALLOCATOR-CONSTRUCTOR', 'ALLOCATOR-AXIOM'] },
  { root: 'borrowed_vec', family: 'borrowed-allocator', expectation: 'refused',
    required_codes: ['ALLOCATOR-SOURCE', 'ALLOCATOR-CONSTRUCTOR', 'ALLOCATOR-AXIOM'] },
  { root: 'system_vec', family: 'system-allocator', expectation: 'refused',
    required_codes: ['ALLOCATOR-SOURCE', 'ALLOCATOR-CONSTRUCTOR', 'ALLOCATOR-AXIOM'] },
  { root: 'stateful_vec', family: 'stateful-allocator', expectation: 'refused',
    required_codes: ['ALLOCATOR-SOURCE', 'ALLOCATOR-CONSTRUCTOR', 'ALLOCATOR-AXIOM'] },
  { root: 'stateful_vec_growth_clone', family: 'stateful-allocator', expectation: 'refused',
    required_codes: ['ALLOCATOR-SOURCE', 'ALLOCATOR-CONSTRUCTOR', 'ALLOCATOR-AXIOM'] },
];

const fixture = {
  probes: 'verification/allocator-admission/fixtures/probes/Cargo.toml',
  probesSource: 'verification/allocator-admission/fixtures/probes/src/lib.rs',
  stateful: 'verification/allocator-admission/fixtures/stateful-alloc/Cargo.toml',
  statefulSource: 'verification/allocator-admission/fixtures/stateful-alloc/src/lib.rs',
};

// A run is only evidence for the bytes it was made from: refuse a dirty tree, and refuse
// an environment that overrides a reviewed tool.
const dirty = spawnSync('git', ['status', '--porcelain'], { cwd: root, encoding: 'utf8' });
if (dirty.status !== 0) throw Error('git status failed; a receipt must bind a commit');
if (dirty.stdout.trim() !== '') throw Error(`refusing to run on a dirty tree: ${dirty.stdout.trim()}`);
const commit = spawnSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).stdout.trim();
for (const name of ['CHARON_EXE', 'AENEAS_EXE', 'LEAN_PATH', 'LEAN_SRC_PATH', 'LEAN_SYSROOT',
  'LAKE_HOME', 'RUSTC', 'RUSTC_WRAPPER', 'RUSTC_WORKSPACE_WRAPPER', 'RUSTFLAGS',
  'CARGO_ENCODED_RUSTFLAGS', 'RUSTUP_TOOLCHAIN', 'CHARON_IS_SYMLINK']) {
  if (process.env[name]) throw Error(`unreviewed tool override: ${name}`);
}
const boundFiles = Object.keys(selection.file_sha256);
for (const file of boundFiles) {
  if (fileSha(path.join(root, file)) !== selection.file_sha256[file]) {
    throw Error(`stale selected configuration: ${file}`);
  }
}

const tool = (name, binary) => path.join(selection.tool_paths[name].output, 'bin', binary);
const scratchHome = path.join(artifacts, 'home');
const scratchTmp = path.join(artifacts, 'tmp');
const fakeUser = { HOME: scratchHome, USER: 'noble', LANG: 'C.UTF-8', TMPDIR: scratchTmp };
fs.mkdirSync(scratchHome, { recursive: true });
fs.mkdirSync(scratchTmp, { recursive: true });
const systemPath = '/nix/store/788mx070y81zjlg5ipcl0cra3afviw9k-gcc-wrapper-15.2.0/bin:/run/current-system/sw/bin:/usr/bin:/bin';

const commands = [];
const logDir = path.join(artifacts, 'logs');
fs.mkdirSync(logDir);
function run(label, binary, args, environment, timeout = 1_200_000, cwd = root) {
  const result = spawnSync(binary, args, { cwd, env: environment, encoding: 'utf8',
    timeout, maxBuffer: 64 * 1024 * 1024 });
  const output = (result.stdout ?? '') + (result.stderr ?? '');
  fs.writeFileSync(path.join(logDir, `${label}.log`), normalize(output));
  const record = { label, binary: normalize(binary), args: args.map(normalize), exit: result.status,
    signal: result.signal ?? null, timed_out: result.error?.code === 'ETIMEDOUT',
    output_sha256: sha(output), log: `logs/${label}.log` };
  commands.push(record);
  return { ...record, output };
}
const tail = (output, lines = 12) => normalize(output).trimEnd().split('\n').slice(-lines).join('\n');

function firstError(output) {
  const line = String(output).split('\n').find(candidate => /^error(\[|:)/.test(candidate));
  return line ? normalize(line) : null;
}

// ---------------------------------------------------------------------------
// Identities.
// ---------------------------------------------------------------------------
const versionArguments = { charon: ['version'], aeneas: ['-version'], rustc: ['--version', '--verbose'],
  lean: ['--version'], lake: ['--version'] };
const identity = (name, binary, kind = name) => {
  const digest = fileSha(binary);
  const version = run(`version-${name}`, binary, versionArguments[kind],
    { ...fakeUser, PATH: `${path.dirname(binary)}:${systemPath}` }, 60_000);
  return { path: normalize(binary), sha256: digest, version: version.output.trim() };
};
const tools = {
  charon: { ...identity('charon', tool('charon', 'charon')), rev: selection.sources.charon.rev },
  aeneas: { ...identity('aeneas', tool('aeneas', 'aeneas')), rev: selection.sources.aeneas.rev,
    lean_library_rev: selection.sources.aeneas_lean.rev },
  extraction_rust: { ...identity('extraction_rust', tool('extraction_rust', 'rustc'), 'rustc'),
    nightly: selection.configuration.extraction_rust },
  lean: { ...identity('lean', tool('lean', 'lean')) },
  lake: { ...identity('lake', tool('lean', 'lake'), 'lake') },
};
const leanLibrary = fs.readFileSync(path.join(root, 'proofs/m1/lake-manifest.json'), 'utf8');
const leanLibraryRev = JSON.parse(leanLibrary).packages.find(pkg => pkg.name === 'aeneas')?.rev ?? null;
if (leanLibraryRev !== selection.sources.aeneas_lean.rev) {
  throw Error(`the proof projects' Aeneas Lean library ${leanLibraryRev} is not the selected ${selection.sources.aeneas_lean.rev}`);
}

// The released compiler column. Absent toolchain => `not-run` with the reason, never a
// fabricated success.
let stable = { status: 'not-run', reason: 'no --stable-prefix was supplied' };
if (options['stable-prefix']) {
  const prefix = path.resolve(options['stable-prefix']);
  const rustc = path.join(prefix, 'bin/rustc');
  const cargo = path.join(prefix, 'bin/cargo');
  if (!fs.existsSync(rustc) || !fs.existsSync(cargo)) {
    stable = { status: 'not-run', reason: `no rustc/cargo under ${normalize(prefix)}` };
  } else {
    const version = run('version-stable', rustc, ['--version', '--verbose'], fakeUser, 60_000);
    let tarball = { status: 'not-checked', reason: 'no --stable-tarball was supplied' };
    if (options['stable-tarball']) {
      const observed = fileSha(path.resolve(options['stable-tarball']));
      const published = options['stable-published-sha256'] ?? null;
      tarball = { status: published === null ? 'digest-recorded' : observed === published ? 'match' : 'mismatch',
        file: path.basename(options['stable-tarball']), bytes: fs.statSync(options['stable-tarball']).size,
        sha256: observed, published_sha256: published };
      if (tarball.status === 'mismatch') throw Error('the stable tarball does not match its published sha256');
    }
    stable = { status: 'installed', prefix: normalize(prefix), rustc_sha256: fileSha(rustc),
      cargo_sha256: fileSha(cargo), version: version.output.trim(), tarball,
      installer: 'install.sh --prefix <dir> --components=rustc,cargo,rust-std-x86_64-unknown-linux-gnu' };
    if (/\bnightly\b/.test(stable.version) || !/^release: 1\.100\.0$/m.test(stable.version)) {
      throw Error(`the stable column requires released 1.100.0, got: ${stable.version}`);
    }
  }
}

// ---------------------------------------------------------------------------
// Source stage: the fixture variants through both compilers.
// ---------------------------------------------------------------------------
const cargoEnvironment = (binDirectory, target) => ({ ...fakeUser,
  PATH: `${binDirectory}:${systemPath}`, CARGO_HOME: path.join(artifacts, 'cargo-home'),
  CARGO_TARGET_DIR: path.join(artifacts, target), CARGO_NET_OFFLINE: 'true', CARGO_INCREMENTAL: '0',
  RUSTC_WRAPPER: '', RUSTC_WORKSPACE_WRAPPER: '', RUSTFLAGS: '', CARGO_ENCODED_RUSTFLAGS: '' });
const extractionBin = path.dirname(tool('extraction_rust', 'cargo'));
const stableBin = options['stable-prefix'] ? path.join(path.resolve(options['stable-prefix']), 'bin') : null;

function compile(label, bin, variant, extra = []) {
  const args = ['build', '--manifest-path', fixture.probes, '--lib', '--offline', '--locked', ...extra];
  const result = run(label, path.join(bin, 'cargo'), args, cargoEnvironment(bin, `${label}-target`));
  return { status: result.exit === 0 ? 'compiled' : 'blocked', variant, exit: result.exit,
    diagnostic: result.exit === 0 ? null : firstError(result.output) ?? tail(result.output, 3),
    unstable_feature_errors: (normalize(result.output).match(/error\[E0658\]/g) ?? []).length,
    stable_feature_errors: (normalize(result.output).match(/error\[E0554\]/g) ?? []).length,
    first_error_lines: normalize(result.output).split('\n')
      .filter(line => /E0658|E0554|^error/.test(line)).slice(0, 4),
    log: result.log };
}
const sourceStages = {
  stable_1_100_ungated: stable.status === 'installed'
    ? compile('source-stable-ungated', stableBin, 'stable')
    : { status: 'not-run', variant: 'stable', reason: stable.reason ?? stable.status },
  stable_1_100_gated: stable.status === 'installed'
    ? compile('source-stable-gated', stableBin, 'extraction-nightly', ['--features', 'nightly-allocator-api'])
    : { status: 'not-run', variant: 'extraction-nightly', reason: stable.reason ?? stable.status },
  extraction_nightly_ungated: compile('source-nightly-ungated', extractionBin, 'stable'),
  extraction_nightly_gated: compile('source-nightly-gated', extractionBin, 'extraction-nightly',
    ['--all-features']),
};
// The pre-registered source expectations: 1.100.0 compiles the stable variant, the pinned
// extraction nightly does not (the API is unstable there without the crate gate), and the
// gated variant is not a stable-1.100 subject.
const sourceExpectations = {
  stable_1_100_ungated: 'compiled',
  stable_1_100_gated: 'blocked',
  extraction_nightly_ungated: 'blocked',
  extraction_nightly_gated: 'compiled',
};
const sourceFailures = Object.entries(sourceExpectations)
  .filter(([stage, expected]) => sourceStages[stage].status !== expected)
  .map(([stage, expected]) => ({ stage, expected, observed: sourceStages[stage].status,
    diagnostic: sourceStages[stage].diagnostic }));

// ---------------------------------------------------------------------------
// Pipeline stage, per cell.
// ---------------------------------------------------------------------------
const bindingFor = (cellRoot, llbcOptions) => ({
  schema: admissionSchema,
  source: { fixture: fixture.probesSource, sha256: fileSha(path.join(root, fixture.probesSource)) },
  selection_sha256: fileSha(path.join(root, 'policy/tool-selection.json')),
  tools: { charon: { rev: tools.charon.rev }, aeneas: { rev: tools.aeneas.rev },
    aeneas_lean: { rev: tools.aeneas.lean_library_rev },
    extraction_rust: { version: tools.extraction_rust.version.split('\n')[0] } },
  options: { hide_allocator: llbcOptions?.hide_allocator ?? null,
    start_from: llbcOptions?.start_from ?? null, preset: llbcOptions?.preset ?? null },
  root: cellRoot,
});
const expected = {
  schema: admissionSchema,
  source: { fixture: fixture.probesSource, sha256: fileSha(path.join(root, fixture.probesSource)) },
  selection_sha256: fileSha(path.join(root, 'policy/tool-selection.json')),
  options: { hide_allocator: true },
  tools: { charon: { rev: tools.charon.rev }, aeneas: { rev: tools.aeneas.rev },
    aeneas_lean: { rev: tools.aeneas.lean_library_rev },
    extraction_rust: { version: tools.extraction_rust.version.split('\n')[0] } },
};

// The Lean stage reuses the proof projects' Aeneas Lean library: the packages tree stays
// read-only (its build directory is the prebuilt one), and only the generated file is
// elaborated into the artifact project.
const packages = fs.realpathSync(path.join(root, 'proofs/mc1/.lake/packages'));
const libraryMarker = path.join(packages, 'aeneas/backends/lean/.lake/build/lib/lean/Aeneas/Std/Vec.olean');
if (!fs.existsSync(libraryMarker)) throw Error(`no prebuilt Aeneas Lean library at ${normalize(packages)}`);
const packagesSnapshot = () => ({
  vec_olean_mtime_ms: fs.statSync(libraryMarker).mtimeMs,
  aeneas_std_dir_mtime_ms: fs.statSync(path.dirname(libraryMarker)).mtimeMs,
  newest_entry: fs.readdirSync(path.dirname(libraryMarker))
    .map(entry => ({ entry, mtime_ms: fs.statSync(path.join(path.dirname(libraryMarker), entry)).mtimeMs }))
    .sort((left, right) => right.mtime_ms - left.mtime_ms).slice(0, 3),
});
const before = packagesSnapshot();
const leanProject = path.join(artifacts, 'lean-project');
fs.mkdirSync(leanProject);
fs.mkdirSync(path.join(leanProject, '.lake'));
fs.copyFileSync(path.join(root, 'proofs/m1/lake-manifest.json'), path.join(leanProject, 'lake-manifest.json'));
fs.copyFileSync(path.join(root, 'proofs/m1/lean-toolchain'), path.join(leanProject, 'lean-toolchain'));
fs.symlinkSync(packages, path.join(leanProject, '.lake/packages'), 'dir');
// The project's own lakefile never builds the shared packages: the stage runs
// `lake env lean <file>`, which only sets LEAN_PATH and elaborates the one generated file into
// nothing. A `lake build` here could write rebuilt oleans through the `.lake/packages` symlink
// into the prebuilt library the proof projects share.
fs.writeFileSync(path.join(leanProject, 'lakefile.toml'),
  'name = "noble-allocator-admission"\n\n[[require]]\nname = "aeneas"\n'
  + 'git = { url = "https://github.com/AeneasVerif/aeneas", subDir = "backends/lean" }\n'
  + `rev = "${tools.aeneas.lean_library_rev}"\n`);

const operational = [];
for (const cell of cells) {
  const name = cell.root;
  const directory = path.join(artifacts, 'cells', name);
  fs.mkdirSync(directory, { recursive: true });
  const llbc = path.join(directory, `${name}.llbc`);
  const generated = path.join(directory, 'generated');
  fs.mkdirSync(generated);
  const stages = {};

  const charon = run(`${name}-charon`, tool('charon', 'charon'), ['cargo',
    ...selection.configuration.charon_args, '--start-from', `probes::${name}`, '--dest-file', llbc,
    '--', '--manifest-path', fixture.probes, ...selection.configuration.cargo_args],
  cargoEnvironment(extractionBin, `${name}-charon-target`));
  stages.charon = { status: charon.exit === 0 ? 'extracted' : 'blocked', exit: charon.exit,
    diagnostic: charon.exit === 0 ? null : firstError(charon.output) ?? tail(charon.output, 3),
    llbc_sha256: fs.existsSync(llbc) ? fileSha(llbc) : null, log: charon.log };
  if (charon.exit !== 0) { operational.push({ root: name, stages }); continue; }

  const llbcDocument = JSON.parse(fs.readFileSync(llbc, 'utf8'));
  const options = llbcDocument.translated.options;
  stages.charon.options = { hide_allocator: options.hide_allocator, treat_box_as_builtin: options.treat_box_as_builtin,
    preset: options.preset, start_from: options.start_from, monomorphize: options.monomorphize,
    opaque: options.opaque, exclude: options.exclude };

  const aeneas = run(`${name}-aeneas`, tool('aeneas', 'aeneas'), [...selection.configuration.aeneas_args,
    '-dest', generated, llbc], { ...fakeUser, PATH: `${path.dirname(tool('aeneas', 'aeneas'))}:${systemPath}` });
  const translationFile = path.join(generated, 'translation.json');
  const generatedFiles = fs.existsSync(generated) ? fs.readdirSync(generated).sort() : [];
  stages.aeneas = { status: aeneas.exit === 0 ? 'translated' : 'blocked', exit: aeneas.exit,
    diagnostic: aeneas.exit === 0 ? null : firstError(aeneas.output) ?? tail(aeneas.output, 3),
    translation_sha256: fs.existsSync(translationFile) ? fileSha(translationFile) : null,
    generated_files: Object.fromEntries(generatedFiles.filter(file => file.endsWith('.lean'))
      .map(file => [file, fileSha(path.join(generated, file))])), log: aeneas.log };
  if (aeneas.exit !== 0 || !fs.existsSync(translationFile)) { operational.push({ root: name, stages }); continue; }

  const translation = JSON.parse(fs.readFileSync(translationFile, 'utf8'));
  const produced = generatedFiles.filter(file => file.endsWith('.lean'));
  const entry = produced.find(file => !['Funs.lean', 'Types.lean', 'FunsExternal_Template.lean'].includes(file))
    ?? produced[0];
  const library = entry.replace(/\.lean$/, '');
  fs.copyFileSync(path.join(generated, entry), path.join(leanProject, entry));
  const leanRun = run(`${name}-lean`, tool('lean', 'lake'), ['env', 'lean', entry],
    { ...fakeUser, PATH: `${path.dirname(tool('lean', 'lake'))}:${systemPath}` }, 1_800_000, leanProject);
  stages.lean = { status: leanRun.exit === 0 ? 'checked' : 'blocked', exit: leanRun.exit,
    module: library, entry_sha256: fileSha(path.join(leanProject, entry)),
    diagnostic: leanRun.exit === 0 ? null : firstError(leanRun.output) ?? tail(leanRun.output, 3),
    log: leanRun.log };

  const rootInTranslation = (translation.functions ?? []).some(fn => fn.rust_name === `probes::${name}`);
  const report = admit({ root: `probes::${name}`, llbc: llbcDocument, translation,
    binding: bindingFor(`probes::${name}`, options), expected });
  stages.admission = { status: verdict(report), schema: report.schema,
    refusal_codes: [...new Set(report.refusals.map(refusal => refusal.code))].sort(),
    refusals: report.refusals.map(refusal => ({ code: refusal.code, message: normalize(refusal.message),
      span: refusal.span, evidence: refusal.evidence })),
    observables: report.observables };
  stages.root_in_translation = rootInTranslation;
  stages.binding = report.binding;
  operational.push({ root: name, family: cell.family, expectation: cell.expectation,
    required_codes: cell.required_codes, stages, record: report });
}

// ---------------------------------------------------------------------------
// Negative controls: each is a mutation of real evidence and must not admit.
// ---------------------------------------------------------------------------
const controls = [];
function control(name, expectedCodes, action, { exact = false } = {}) {
  const { record, document } = action();
  const codes = [...new Set(record.refusals.map(refusal => refusal.code))].sort();
  const missing = expectedCodes.filter(code => !codes.includes(code));
  const unexpected = exact ? codes.filter(code => !expectedCodes.includes(code)) : [];
  const accepted = record.result === 'admitted' && expectedCodes.length > 0;
  controls.push({ name, expected_codes: expectedCodes, observed_codes: codes, exact,
    result: missing.length === 0 && unexpected.length === 0 && !accepted ? 'refused-as-required' : 'failed',
    missing_codes: missing, unexpected_codes: unexpected, accepted, root: document.root, schema: record.schema,
    refusals: record.refusals.map(refusal => ({ code: refusal.code, message: normalize(refusal.message),
      span: refusal.span })) });
}
function readCell(name) {
  const llbc = path.join(artifacts, 'cells', name, `${name}.llbc`);
  const translation = path.join(artifacts, 'cells', name, 'generated/translation.json');
  return { llbc: JSON.parse(fs.readFileSync(llbc, 'utf8')),
    translation: JSON.parse(fs.readFileSync(translation, 'utf8')) };
}
control('unreached-root', ['BINDING-ROOT', 'ROOT-MISSING'], () => {
  // A real extraction of `box_baseline` cannot stand in for `global_vec`: the root is
  // absent from that LLBC, so the only refusals are the missing root and the wrong
  // `start_from`. The extraction itself is genuine evidence for `box_baseline`.
  const { llbc, translation } = readCell('box_baseline');
  const document = { root: 'probes::global_vec', llbc, translation,
    binding: bindingFor('probes::global_vec', llbc.translated.options), expected };
  return { record: admit(document), document };
}, { exact: true });
control('borrowed-extraction', ['ALLOCATOR-AXIOM', 'ALLOCATOR-CONSTRUCTOR', 'BINDING-ROOT', 'ROOT-MISSING'], () => {
  // The mirror image: an explicit-allocator subject's extraction cannot certify an
  // implicit-Global baseline, because the baseline is not in it and the allocator evidence
  // it does carry is refusable on its own.
  const { llbc, translation } = readCell('global_vec');
  const document = { root: 'probes::vec_baseline', llbc, translation,
    binding: bindingFor('probes::vec_baseline', llbc.translated.options), expected };
  return { record: admit(document), document };
}, { exact: true });
control('omitted-body', ['ROOT-OPAQUE'], () => {
  const { llbc, translation } = readCell('vec_baseline');
  const mutated = structuredClone(llbc);
  const declaration = mutated.translated.fun_decls.find(entry => entry && entry.def_id !== undefined
    && entry.item_meta?.is_local === true);
  declaration.body = 'Opaque';
  const document = { root: 'probes::vec_baseline', llbc: mutated, translation,
    binding: bindingFor('probes::vec_baseline', llbc.translated.options), expected };
  return { record: admit(document), document };
});
control('changed-allocator-identity', ['BINDING-IDENTITY'], () => {
  const { llbc, translation } = readCell('vec_baseline');
  const document = { root: 'probes::vec_baseline', llbc, translation,
    binding: bindingFor('probes::vec_baseline', llbc.translated.options), expected };
  const changed = structuredClone(document);
  changed.expected = { ...expected, tools: { ...expected.tools, aeneas_lean: { rev: 'f'.repeat(40) } } };
  return { record: admit(changed), document };
});
control('changed-source-identity', ['BINDING-IDENTITY'], () => {
  const { llbc, translation } = readCell('vec_baseline');
  const document = { root: 'probes::vec_baseline', llbc, translation,
    binding: bindingFor('probes::vec_baseline', llbc.translated.options), expected };
  const changed = structuredClone(document);
  changed.expected = { ...expected, source: { fixture: fixture.probesSource, sha256: '0'.repeat(64) } };
  return { record: admit(changed), document };
});
control('stale-receipt', ['STALE-RECEIPT'], () => {
  const baselineCell = operational.find(cell => cell.root === 'vec_baseline');
  const current = structuredClone(baselineCell.record.binding);
  current.source = { fixture: fixture.probesSource, sha256: '1'.repeat(64) };
  const document = { root: 'probes::vec_baseline' };
  return { record: { schema: admissionSchema, result: 'refused', refusals: revalidate(baselineCell.record, current) },
    document };
});
const liveControl = (() => {
  const baselineCell = operational.find(cell => cell.root === 'vec_baseline');
  const drift = revalidate(baselineCell.record, structuredClone(baselineCell.record.binding));
  return { name: 'unchanged-receipt-revalidates', refusals: drift.length, result: drift.length === 0 ? 'accepted' : 'failed' };
})();

const after = packagesSnapshot();
// Observed, not asserted: the prebuilt Aeneas Lean library is shared with every proof project
// and other lanes work against it concurrently. This lane's own commands are `lake env lean`,
// which cannot write it, so a change here is another lane's build and is reported as such.
const packageCacheChanged = before.vec_olean_mtime_ms !== after.vec_olean_mtime_ms
  || before.aeneas_std_dir_mtime_ms !== after.aeneas_std_dir_mtime_ms;

// ---------------------------------------------------------------------------
// Verdict.
// ---------------------------------------------------------------------------
// A *refused* cell only has to be refused for the right reasons; what the rest of the
// pipeline did with it is an observation, not a criterion. An *admitted* cell is the
// positive control, so it has to clear every stage: a baseline that fails somewhere is not
// evidence that the baseline works.
const blockedStages = cell => Object.entries(cell.stages)
  .filter(([, stage]) => stage && typeof stage === 'object' && stage.status === 'blocked')
  .map(([stage]) => stage);
const missingCodes = cell => (cell.required_codes ?? [])
  .filter(code => !cell.stages.admission?.refusal_codes?.includes(code));
const mismatches = operational.filter(cell => {
  const admitted = cell.expectation === 'admitted';
  if (admitted) {
    return cell.stages.admission?.status !== 'admitted' || blockedStages(cell).length > 0
      || (cell.stages.admission?.refusal_codes ?? []).length > 0;
  }
  return cell.stages.admission?.status !== 'refused' || missingCodes(cell).length > 0;
}).map(cell => ({ root: cell.root, expectation: cell.expectation,
  observed: cell.stages.admission?.status ?? 'no-admission',
  blocked_stages: blockedStages(cell), missing_codes: missingCodes(cell) }));
const observedBlockedStages = operational.flatMap(cell =>
  blockedStages(cell).map(stage => ({ root: cell.root, stage })));

const tree = spawnSync('git', ['status', '--porcelain'], { cwd: root, encoding: 'utf8' });
const treeCleanAfter = tree.stdout.trim() === '';

const evidence = {
  schema: 'noble-allocator-admission-receipt/v1',
  lane: 'verification/allocator-admission',
  issue: 121,
  role: 'extraction and admission evidence only: not a refinement proof, not a model-to-implementation correspondence, not M1 acceptance',
  source: {
    commit,
    tree_clean_before: true,
    tree_clean_after: treeCleanAfter,
    files: Object.fromEntries([fixture.probes, fixture.probesSource, fixture.stateful,
      fixture.statefulSource, 'verification/allocator-admission/run.mjs',
      'verification/allocator-admission/admission.mjs',
      'verification/allocator-admission/admission.test.mjs']
      .map(file => [file, fileSha(path.join(root, file))])),
    fixture_isolation: {
      workspace_members: (fs.readFileSync(path.join(root, 'Cargo.toml'), 'utf8')
        .match(/^members = \[(.*)\]$/m)?.[1] ?? '').split(',').map(entry => entry.trim().replace(/"/g, '')),
      probes_is_own_workspace: /^\[workspace\]$/m.test(fs.readFileSync(path.join(root, fixture.probes), 'utf8')),
      stateful_is_own_workspace: /^\[workspace\]$/m.test(fs.readFileSync(path.join(root, fixture.stateful), 'utf8')),
      unsafe_code_forbid_in_probes: /unsafe_code = "forbid"/.test(fs.readFileSync(path.join(root, fixture.probes), 'utf8')),
      unsafe_in_stateful_only: /unsafe impl Allocator/.test(fs.readFileSync(path.join(root, fixture.statefulSource), 'utf8')),
    },
  },
  tools,
  selection: { sha256: fileSha(path.join(root, 'policy/tool-selection.json')),
    charon_args: selection.configuration.charon_args, aeneas_args: selection.configuration.aeneas_args,
    cargo_args: selection.configuration.cargo_args, extraction_profile: selection.configuration.extraction_profile },
  stable_toolchain: stable,
  lean_library: { packages: normalize(packages), manifest_sha256: sha(leanLibrary), rev: leanLibraryRev,
    marker: normalize(libraryMarker), shared_cache_changed_during_run: packageCacheChanged,
    lane_commands_can_write_cache: false, lane_stage: 'lake env lean <generated file>',
    snapshot_before: before, snapshot_after: after },
  source_stage: { expectations: sourceExpectations, stages: sourceStages, failures: sourceFailures,
    ungated_unstable_feature_errors: sourceStages.extraction_nightly_ungated.unstable_feature_errors ?? 0 },
  cells: operational.map(cell => ({ root: cell.root, family: cell.family, expectation: cell.expectation,
    required_codes: cell.required_codes, observed: cell.stages.admission?.status ?? null,
    stages: cell.stages })),
  controls: { results: controls, unchanged_receipt_revalidates: liveControl },
  observed_blocked_stages: observedBlockedStages,
  matrix: {
    row: 'fixture root', column: 'pipeline stage',
    columns: ['source_compile', 'charon', 'aeneas', 'lean', 'admission'],
    source_scope: 'crate: the fixture crate is compiled as one unit, so the source column is a crate verdict',
  },
  assumptions: [
    'the source column measures the Rust language surface only',
    'charon/aeneas/lean success measures translation type-checking only; the selected preset erases the allocator',
    'admission is Noble\'s refusal boundary, not a proof about allocators',
    'the extraction-nightly fixture variant is not a stable-1.100 subject',
  ],
  commands,
  mismatches,
  result: mismatches.length === 0 && sourceFailures.length === 0 && controls.every(item => item.result !== 'failed')
    && liveControl.result === 'accepted' && treeCleanAfter ? 'passed' : 'failed',
};
fs.writeFileSync(path.join(artifacts, 'evidence.json'), JSON.stringify(evidence, null, 2) + '\n');

const summary = {
  result: evidence.result,
  stable: stable.status,
  source: Object.fromEntries(Object.entries(sourceStages).map(([name, stage]) => [name, stage.status])),
  cells: Object.fromEntries(operational.map(cell => [cell.root, cell.stages.admission?.status ?? 'no-admission'])),
  controls: Object.fromEntries(controls.map(item => [item.name, item.result])),
  mismatches: mismatches.length,
  blocked_stages: observedBlockedStages,
  shared_package_cache_changed: packageCacheChanged,
  artifacts: normalize(artifacts),
};
console.log(JSON.stringify(summary, null, 2));
if (evidence.result !== 'passed') {
  console.error(`allocator admission matrix: FAILED (${mismatches.length} cell mismatch(es), `
    + `${sourceFailures.length} source expectation failure(s))`);
  process.exit(1);
}
console.log(`allocator admission matrix: passed (${operational.length} cells, `
  + `${controls.length} executed controls, ${explicitConstructors.length} frozen constructor names)`);
