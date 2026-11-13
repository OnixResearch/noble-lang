// Execute the size-change conformance scenarios REC-SC-01…REC-SC-04 (#101) through the production
// framed session of the CLI built from the checked-out commit, and retain a receipt with every
// report and certificate.
//
// Each case declares one module in the signed declared-text profile with the opt-in
// `--size-change-evidence` flag, so the link report carries the checker's verdict and a Lean
// certificate directory. The lane then:
//
//   1. asserts the reported verdict, measures, sites and arguments;
//   2. asserts the certificate file's digest matches the report;
//   3. re-runs the case with `--size-change-lean`, so the CLI itself re-checks the certificate;
//   4. independently checks every certificate with the pinned Lean in an isolated project built
//      from the size-change reference-model sources.
//
// An observation is evidence about that execution only: the model theorems in
// `proofs/m3/NobleM2/SizeChange*.lean` are audited by `verification/size-change/check.mjs`, and
// the certificate is about the classified relations, not about an execution of the body.
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { lstatSync, mkdirSync, readFileSync, realpathSync, statSync, writeFileSync } from 'node:fs';
import { basename, dirname, join, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { selectedVendor } from '../selected-vendor.mjs';

const root = resolve(fileURLToPath(import.meta.url), '../../..');
const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const fileHash = path => digest(readFileSync(path));
const requireThat = (condition, message) => { if (!condition) throw Error(message); };
const stable = value => JSON.stringify(value);

const signed = ['--signed-self-recursion', '--retained-source-bytes', '262144'];
const profile = 'Signed-Declared-Text-v1';
const sizeChangeIds = ['REC-SC-01', 'REC-SC-02', 'REC-SC-03', 'REC-SC-04'];
const caseFlags = ['--framed', '--declared-modules', '--text-byte-cursor', ...signed,
  '--size-change-evidence', '--size-change-certificate', '<new-directory>'];

let out = null;
const commands = [];

function run(label, executable, argv, options = {}) {
  const { env, cwd = root, input, allowFailure = false } = options;
  const raw = spawnSync(executable, argv, { cwd, env, input, encoding: null, maxBuffer: 64 * 1024 * 1024, timeout: 3_600_000 });
  const stdout = raw.stdout ?? Buffer.alloc(0);
  const stderr = raw.stderr ?? Buffer.alloc(0);
  commands.push({
    label, executable: realpathSync(executable).replace(`${realpathSync(root)}${sep}`, `$ROOT${sep}`),
    argv, exit_status: raw.status, stdout_sha256: digest(stdout), stderr_sha256: digest(stderr),
  });
  requireThat(!raw.error && (raw.status === 0 || allowFailure),
    `${label} failed (${raw.error?.message ?? raw.status}): ${stderr.toString('utf8')}`);
  return { status: raw.status, stdout: stdout.toString('utf8'), stderr: stderr.toString('utf8') };
}

function outputDirectory(requested) {
  const resolved = resolve(requested);
  const destination = join(realpathSync(dirname(resolved)), basename(resolved));
  requireThat(!destination.startsWith(`${realpathSync(root)}${sep}`), 'the output directory must be outside the worktree');
  try {
    lstatSync(destination);
    throw Error(`output directory exists: ${destination}`);
  } catch (error) {
    if (error.code !== 'ENOENT') throw error;
  }
  mkdirSync(destination, { mode: 0o700 });
  for (const child of ['home', 'tmp', 'cargo-home']) mkdirSync(join(destination, child), { mode: 0o700 });
  return destination;
}

function framed(submissions) {
  return Buffer.concat(submissions.map(source => Buffer.from(`${Buffer.byteLength(source)}\n${source}`)));
}

function loadCases() {
  const registry = JSON.parse(readFileSync(join(root, 'specs/conformance/recursion-cases.json')));
  const byId = new Map(registry.cases.map(entry => [entry.id, entry]));
  const registeredIds = registry.cases.filter(entry => entry.id.startsWith('REC-SC-')).map(entry => entry.id);
  requireThat(stable(registeredIds) === stable(sizeChangeIds),
    `expected exactly ${sizeChangeIds.join(', ')}, found ${registeredIds.join(', ')}`);
  return sizeChangeIds.map(id => {
    const entry = byId.get(id);
    requireThat(entry.profile === profile && entry.kind === 'static'
      && entry.requirements.includes('K-CHECK-02'), `${id}: wrong profile, kind or requirement`);
    requireThat(stable(entry.input.session_flags) === stable(caseFlags), `${id}: session flags differ`);
    requireThat(entry.input.submissions.length === 1, `${id}: expected one module submission`);
    requireThat(entry.expected.stage === 'link' && entry.expected.outcome === 'linked'
      && entry.expected.guest_requests === 0 && entry.expected.protected_operations === 0,
    `${id}: invalid link expectation`);
    const field = entry.expected.size_change;
    requireThat(field?.schema === 'noble-size-change/v1' && field.lean_check === null
      && field.definitions?.length === 1, `${id}: invalid size-change expectation`);
    return {
      id,
      module: entry.input.submissions[0],
      expected: field.definitions[0],
    };
  });
}

/// The reference-model closure of one root module, read from the sources.
function importClosure(rootModule) {
  const seen = new Set();
  const pending = [rootModule];
  while (pending.length > 0) {
    const name = pending.pop();
    if (seen.has(name)) continue;
    seen.add(name);
    const text = readFileSync(join(root, `proofs/m3/NobleM2/${name.replace(/^NobleM2\./, '')}.lean`), 'utf8');
    for (const match of text.matchAll(/^import\s+(\S+)/gm)) {
      if (match[1] === 'Std') continue;
      requireThat(/^NobleM2\.SizeChange[A-Za-z]*$/.test(match[1]) || match[1] === 'NobleM2.SignedSelf',
        `${name} imports ${match[1]}, outside the size-change certificate's model closure`);
      pending.push(match[1]);
    }
  }
  return [...seen].sort();
}

/// Build the isolated Lean project that gives the certificate its library.
function buildLibrary(leanBin) {
  const closure = importClosure('NobleM2.SizeChangeFixtures');
  const project = join(out, 'lean');
  mkdirSync(join(project, 'NobleM2'), { recursive: true });
  for (const name of closure) {
    writeFileSync(join(project, `${name.replace(/^NobleM2\./, 'NobleM2/')}.lean`),
      readFileSync(join(root, `proofs/m3/NobleM2/${name.replace(/^NobleM2\./, '')}.lean`)));
  }
  writeFileSync(join(project, 'lean-toolchain'), readFileSync(join(root, 'proofs/m3/lean-toolchain')));
  writeFileSync(join(project, 'lakefile.toml'),
    ['name = "noble_size_change_certificate"', 'defaultTargets = ["NobleM2"]', '', '[[lean_lib]]',
      'name = "NobleM2"', `roots = [${closure.map(name => JSON.stringify(name)).join(', ')}]`, ''].join('\n'));
  const env = {
    HOME: join(out, 'home'), TMPDIR: join(out, 'tmp'), LANG: 'C.UTF-8',
    PATH: [leanBin, '/run/current-system/sw/bin', '/usr/bin', '/bin'].join(':'),
  };
  run('lean-library-build', join(leanBin, 'lake'), ['build'], { env, cwd: project });
  return { project, closure, library: join(project, '.lake/build/lib/lean') };
}

/// Check one certificate with the pinned Lean and the isolated library.
function checkCertificate(leanBin, library, certificate) {
  const env = {
    HOME: join(out, 'home'), TMPDIR: join(out, 'tmp'), LANG: 'C.UTF-8', LEAN_PATH: library,
    PATH: [leanBin, '/run/current-system/sw/bin', '/usr/bin', '/bin'].join(':'),
  };
  run(`lean-certificate-${basename(dirname(certificate))}`, join(leanBin, 'lean'), [certificate], { env });
}

function observed(entry, expected) {
  const record = {
    definition: entry.definition, measures: entry.measures, verdict: entry.verdict,
    sites: (entry.sites ?? []).map(site => ({ arguments: site.arguments, guarded: site.guarded })),
  };
  if (entry.verdict === 'accepted') record.measure = entry.measure;
  if (entry.verdict === 'refused') record.witnesses = entry.witnesses;
  const wanted = {};
  for (const key of Object.keys(expected)) wanted[key] = record[key];
  return wanted;
}

function parseReports(result, id) {
  const reports = result.stdout.split('\n').filter(Boolean).map(line => JSON.parse(line));
  requireThat(reports.length === 1, `${id}: expected one report, got ${reports.length}`);
  return reports[0];
}

function assertLinked(report, id) {
  requireThat(report.stage === 'link', `${id}: stage ${report.stage}`);
  requireThat(report.profile === profile, `${id}: profile ${report.profile}`);
  requireThat(report.outcome === 'linked', `${id}: outcome ${report.outcome}`);
  requireThat(report.guest_requests === 0 && report.protected_operations === 0,
    `${id}: guest requests or protected operations`);
}

function assertDefinition(report, id, expected, leanChecked = false) {
  assertLinked(report, id);
  const field = report.size_change;
  requireThat(field?.schema === 'noble-size-change/v1', `${id}: missing size_change field`);
  requireThat(field.criterion === 'shared-measure: one declared measure decreases at every self-call site'
    && field.profile === 'list structure and guarded i64 counters under one stack prefix',
  `${id}: unexpected criterion or profile`);
  requireThat(leanChecked ? field.lean_check?.passed === true : field.lean_check === null,
    `${id}: unexpected Lean-check state`);
  requireThat(field.definitions.length === 1, `${id}: expected one definition entry`);
  const result = observed(field.definitions[0], expected);
  requireThat(stable(result) === stable(expected),
    `${id}: observed ${stable(result)}, expected ${stable(expected)}`);
  return { field, observed: result };
}

function ordinaryControl(cli, env, cases, signedBase) {
  const result = run('session-without-size-change-flag', cli, signedBase,
    { env, input: framed([cases[1].module]) });
  const report = parseReports(result, 'ordinary-control');
  assertLinked(report, 'ordinary-control');
  requireThat(!Object.hasOwn(report, 'size_change'),
    'ordinary signed-recursion execution unexpectedly carries size_change');
  return { status: 'passed', size_change_field_absent: true, report };
}

function multiReportControl(cli, env, cases, evidenceBase, directory) {
  const selected = cases.slice(0, 2);
  const result = run('session-two-size-change-reports', cli,
    [...evidenceBase, '--size-change-certificate', directory],
    { env, input: framed(selected.map(entry => entry.module)) });
  const reports = result.stdout.split('\n').filter(Boolean).map(line => JSON.parse(line));
  requireThat(reports.length === selected.length,
    `multi-report: expected ${selected.length} reports, got ${reports.length}`);
  const certificates = reports.map((report, index) => {
    const entry = selected[index];
    const { field } = assertDefinition(report, `multi-report-${entry.id}`, entry.expected);
    const path = join(directory, `submission-${report.submission}.lean`);
    requireThat(field.certificate.path === path, `multi-report-${entry.id}: certificate path`);
    requireThat(fileHash(path) === field.certificate.sha256,
      `multi-report-${entry.id}: certificate digest`);
    requireThat(statSync(path).size === field.certificate.bytes,
      `multi-report-${entry.id}: certificate byte count`);
    return {
      id: entry.id, submission: report.submission,
      path: path.replace(`${out}${sep}`, ''), sha256: field.certificate.sha256,
    };
  });
  requireThat(new Set(certificates.map(entry => entry.path)).size === selected.length,
    'multi-report: submissions did not receive distinct certificate files');
  return { status: 'passed', reports, certificates };
}

function main() {
  const [requested] = process.argv.slice(2);
  requireThat(requested, 'usage: SELECTED_NODE verification/size-change/scenarios.mjs NEW_EXTERNAL_DIRECTORY');
  const selection = JSON.parse(readFileSync(join(root, 'policy/tool-selection.json')));
  requireThat(realpathSync(process.execPath) === realpathSync(join(selection.tool_paths.node.output, 'bin/node')),
    'run this script with the selected Node');
  const forbidden = Object.keys(process.env).filter(key =>
    /^(?:LEAN(?:_|$)|LAKE(?:_|$)|BASH(?:_|$)|GIT_|NIX_|ELAN)/.test(key) && process.env[key] !== '');
  requireThat(forbidden.length === 0, `unsafe caller environment: ${forbidden.join(', ')}`);
  out = outputDirectory(requested);
  const git = realpathSync('/run/current-system/sw/bin/git');
  const leanBin = join(selection.tool_paths.lean.output, 'bin');
  const rustBin = join(selection.tool_paths.quality_rust.output, 'bin');
  const nodeBin = join(selection.tool_paths.node.output, 'bin');
  const wasmToolsBin = join(selection.tool_paths.wasm_tools.output, 'bin');
  const binaryenBin = join(selection.tool_paths.binaryen.output, 'bin');
  const vendor = selectedVendor(selection.component_sync);
  const cargo = join(rustBin, 'cargo');
  const cargoHome = join(out, 'cargo-home');
  writeFileSync(join(cargoHome, 'config.toml'),
    '[source.crates-io]\nreplace-with = "reviewed-vendor"\n[source.reviewed-vendor]\n' +
    `directory = ${JSON.stringify(vendor.directory)}\n[net]\noffline = true\n`);
  const target = resolve(process.env.NOBLE_TARGET ?? join(out, 'target'));
  let realTarget;
  try {
    realTarget = realpathSync(target);
  } catch (error) {
    if (error.code !== 'ENOENT') throw error;
    realTarget = join(realpathSync(dirname(target)), basename(target));
  }
  const realRoot = realpathSync(root);
  requireThat(realTarget !== realRoot && !realTarget.startsWith(`${realRoot}${sep}`),
    'Cargo target must be outside the worktree');
  const env = {
    HOME: join(out, 'home'), TMPDIR: join(out, 'tmp'), LANG: 'C.UTF-8', GIT_CONFIG_NOSYSTEM: '1',
    GIT_CONFIG_GLOBAL: '/dev/null',
    PATH: [rustBin, selection.component_sync.linker_bin, nodeBin, wasmToolsBin, binaryenBin,
      '/run/current-system/sw/bin', '/usr/bin', '/bin'].join(':'),
  };
  const cargoEnv = {
    ...env,
    CARGO_HOME: cargoHome,
    CARGO_TARGET_DIR: target,
    CARGO_NET_OFFLINE: 'true',
    CARGO_INCREMENTAL: '0',
    RUSTC_WRAPPER: '',
    RUSTC_WORKSPACE_WRAPPER: '',
    CARGO_BUILD_RUSTC_WRAPPER: '',
    RUSTFLAGS: '',
    CARGO_ENCODED_RUSTFLAGS: '',
    CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS: '',
    CARGO_BUILD_JOBS: '4',
    CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER: join(selection.component_sync.linker_bin, 'cc'),
  };
  const state = () => ({
    commit: run('source-head', git, ['rev-parse', 'HEAD'], { env }).stdout.trim(),
    tree: run('source-tree', git, ['rev-parse', 'HEAD^{tree}'], { env }).stdout.trim(),
    status: run('source-status', git, ['status', '--porcelain=v1', '--untracked-files=all'], { env }).stdout,
  });
  const before = state();
  requireThat(before.status === '', `the source worktree is dirty:\n${before.status}`);
  const cases = loadCases();
  mkdirSync(join(out, 'evidence'));
  const version = run('lean-version', join(leanBin, 'lean'), ['--version'], { env }).stdout.trim();
  requireThat(version.includes('version 4.35.0'), `the selected Lean is not 4.35.0: ${version}`);
  const { closure, library } = buildLibrary(leanBin);
  run('source-cli-build', cargo, ['build', '--locked', '--offline', '-j', '4', '-p', 'noble-cli'],
    { env: cargoEnv });
  const cli = join(target, 'debug/noble');
  const cliSha256 = fileHash(cli);
  const signedBase = ['session', '--framed', '--declared-modules', '--text-byte-cursor', ...signed];
  const evidenceBase = [...signedBase, '--size-change-evidence'];
  const results = cases.map(entry => {
    const plain = join(out, 'evidence', entry.id);
    const checked = join(out, 'evidence', `${entry.id}-lean`);
    const submissions = framed([entry.module]);
    const first = run(`session-${entry.id}`, cli,
      [...evidenceBase, '--size-change-certificate', plain], { env, input: submissions });
    const report = parseReports(first, entry.id);
    const { field, observed: entryObserved } = assertDefinition(report, entry.id, entry.expected);
    requireThat(field.lean_check === null, `${entry.id}: Lean was checked without being requested`);
    const certificate = join(plain, `submission-${report.submission}.lean`);
    requireThat(field.certificate.path === certificate, `${entry.id}: certificate path`);
    requireThat(fileHash(certificate) === field.certificate.sha256, `${entry.id}: certificate digest`);
    requireThat(statSync(certificate).size === field.certificate.bytes,
      `${entry.id}: certificate byte count`);
    checkCertificate(leanBin, library, certificate);
    // The CLI's own re-check, with the caller naming the Lean binary and library explicitly.
    const second = run(`session-${entry.id}-lean`, cli,
      [...evidenceBase, '--size-change-certificate', checked,
        '--size-change-lean', join(leanBin, 'lean'), '--size-change-lean-library', library],
      { env, input: submissions });
    const checkedReport = parseReports(second, `${entry.id}-lean`);
    const { field: checkedField } = assertDefinition(checkedReport, `${entry.id}-lean`, entry.expected, true);
    const check = checkedField.lean_check;
    requireThat(check?.passed === true, `${entry.id}: CLI Lean check did not pass`);
    requireThat(check.library === library, `${entry.id}: CLI recorded another library`);
    requireThat(check.binary === join(leanBin, 'lean') && check.version === version,
      `${entry.id}: CLI recorded another Lean identity`);
    requireThat(check.binary_sha256 === fileHash(join(leanBin, 'lean')),
      `${entry.id}: CLI recorded another Lean binary`);
    requireThat(checkedField.certificate.sha256 === field.certificate.sha256,
      `${entry.id}: re-checked certificate differs`);
    requireThat(stable(checkedField.definitions) === stable(field.definitions),
      `${entry.id}: re-check changed the classified definitions`);
    return {
      id: entry.id, status: 'passed', module: entry.module, submission: report.submission,
      expected: entry.expected, observed: entryObserved,
      certificate: {
        path: certificate.replace(`${out}${sep}`, ''),
        sha256: field.certificate.sha256, bytes: field.certificate.bytes,
        independent_check: 'pinned Lean accepted the certificate with LEAN_PATH at the isolated library',
        reported_lean_check: check,
      },
      report,
    };
  });
  const controls = {
    ordinary: ordinaryControl(cli, env, cases, signedBase),
    multi_report: multiReportControl(cli, env, cases, evidenceBase, join(out, 'evidence', 'multi')),
  };
  requireThat(stable(state()) === stable(before), 'the source changed during the run');
  const modelSources = closure.map(name => `proofs/m3/NobleM2/${name.replace(/^NobleM2\./, '')}.lean`);
  const implementationSources = [
    'crates/noble-cli/src/core.rs',
    'crates/noble-cli/src/core/arguments.rs',
    'crates/noble-cli/src/core/companions/session.rs',
    'crates/noble-cli/src/core/declared/mod.rs',
    'crates/noble-cli/src/core/declared/sizechange.rs',
    'crates/noble-cli/src/core/output/declared.rs',
    'crates/noble-cli/src/core/worker/assembly.rs',
    'crates/noble-contracts/src/source.rs',
    'crates/noble-contracts/src/source/declared/state.rs',
    'crates/noble-contracts/src/source/sizechange.rs',
    'crates/noble-contracts/tests/declared.rs',
    'crates/noble-contracts/tests/declared/sizechange.rs',
    'specs/conformance/recursion-cases.json',
    'verification/size-change/README.md',
    'verification/size-change/scenarios.mjs',
  ];
  const caseRegistrySha256 = digest(Buffer.from(stable(cases.map(entry => ({
    id: entry.id, module: entry.module, expected: entry.expected, profile, session_flags: caseFlags,
  })))));
  const receipt = {
    schema: 'noble-size-change-scenarios/v1', issue: 101, requirement: 'K-CHECK-02',
    source_commit: before.commit, source_tree: before.tree, clean_source: true,
    case_registry_sha256: caseRegistrySha256,
    lean: version, lean_sha256: fileHash(join(leanBin, 'lean')),
    library: { project: '.', path: library.replace(`${out}${sep}`, ''), closure,
      sources_sha256: Object.fromEntries(modelSources.map(file => [file, fileHash(join(root, file))])) },
    implementation_sources_sha256: Object.fromEntries(
      implementationSources.map(file => [file, fileHash(join(root, file))])),
    toolchain: {
      cargo: run('cargo-version', cargo, ['--version'], { env: cargoEnv }).stdout.trim(),
      cargo_sha256: fileHash(join(rustBin, 'cargo')),
      vendor_nar_hash: vendor.narHash,
      cli_sha256: cliSha256,
    },
    cases: results, controls, commands,
    assumptions: [
      'Each case is one link report of a production framed session of the CLI built from this commit.',
      'The unflagged signed-recursion control carries no size_change field; the multi-report control proves one fresh certificate directory serves distinct reports in one session.',
      'The verdict is the front end classification of the checked body; it is not a theorem about an execution. The model soundness, exactness and refusal-completeness theorems are verification/size-change/check.mjs.',
      'The certificate is accepted by the pinned Lean against an isolated library built from these sources; the CLI records the binary digest, its version line and the library path, and the library sources are bound by digest above.',
    ],
  };
  const path = join(out, 'receipt.json');
  writeFileSync(path, `${JSON.stringify(receipt, null, 2)}\n`, { flag: 'wx' });
  console.log(stable({ result: 'passed', receipt: path, source_commit: before.commit,
    cases: results.length, controls: Object.keys(controls).length,
    receipt_sha256: fileHash(path) }));
}

try {
  main();
} catch (error) {
  console.error(`[size-change-scenarios] FAIL ${error.message}`);
  process.exitCode = 1;
}
