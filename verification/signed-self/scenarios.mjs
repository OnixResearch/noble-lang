// Execute the K-CHECK-02 and K-ERROR-03 conformance scenarios REC-01…REC-07 (#99) through the
// production framed session of the CLI built from the checked-out commit, and retain a receipt with
// every report. A scenario's observation is evidence about that execution only.
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { lstatSync, mkdirSync, readFileSync, realpathSync, writeFileSync } from 'node:fs';
import { basename, dirname, join, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { selectedVendor } from '../selected-vendor.mjs';

const root = resolve(fileURLToPath(import.meta.url), '../../..');
const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const fileHash = path => digest(readFileSync(path));
const requireThat = (condition, message) => { if (!condition) throw Error(message); };
const stable = value => JSON.stringify(value);

const signed = ['--signed-self-recursion', '--retained-source-bytes', '262144'];
const countdown = 'module count@1 [ signature down forall<S:stack> [ S I64 -- S I64 ! pure ] export down def down [ dup 0 = [ ] [ 1 - down ] if ] ]';
// Each session's submissions, in order, with the report index each scenario reads.
const sessions = [
  {
    id: 'signed', flags: signed, exit: 5,
    submissions: [
      countdown,
      '5 count@1.down',
      'drop',
      'module no_signature@1 [ def recur [ recur ] ]',
      'module polymorphic@1 [ signature recur forall<S:stack> [ S -- S ! pure ] def recur [ 1 recur ] ]',
      'module mutual@1 [ signature even forall<S:stack> [ S -- S ! pure ] signature odd forall<S:stack> [ S -- S ! pure ] def even [ odd ] def odd [ even ] ]',
      'module spin@1 [ signature forever forall<S:stack> [ S -- S ! pure ] export forever def forever [ forever ] ]',
      'spin@1.forever',
    ],
  },
  {
    id: 'signed-non-tail', flags: signed, exit: 5,
    submissions: [
      'module deep@1 [ signature grow forall<S:stack> [ S -- S ! pure ] export grow def grow [ grow 1 drop ] ]',
      'deep@1.grow',
    ],
  },
  { id: 'ordinary', flags: [], exit: 2, submissions: [countdown] },
];
const signedProfile = 'Signed-Declared-Text-v1';
const scenarios = [
  { id: 'REC-01', session: 'signed', report: 1, profile: signedProfile,
    expected: { stage: 'wasm', outcome: 'normal', stack: [{ type: 'I64', value: '0' }] } },
  { id: 'REC-02', session: 'signed', report: 3, profile: signedProfile,
    expected: { stage: 'resolve', outcome: 'unbound-word', diagnostic: 'cyclic local module definitions' } },
  { id: 'REC-03', session: 'signed', report: 4, profile: signedProfile,
    expected: { stage: 'check', outcome: 'type-reject' } },
  { id: 'REC-04', session: 'signed', report: 5, profile: signedProfile,
    expected: { stage: 'resolve', outcome: 'unbound-word', diagnostic: 'cyclic local module definitions' } },
  { id: 'REC-05', session: 'signed', report: 7, profile: signedProfile,
    expected: { stage: 'wasm', outcome: 'runtime-exhausted', quota_reason: 'execution work', stack: [] } },
  { id: 'REC-06', session: 'signed-non-tail', report: 1, profile: signedProfile,
    expected: { stage: 'wasm', outcome: 'runtime-exhausted', quota_reason: 'continuation bytes', stack: [] } },
  { id: 'REC-07', session: 'ordinary', report: 0, profile: 'Declared-Text-v1',
    expected: { stage: 'resolve', outcome: 'unbound-word', diagnostic: 'cyclic local module definitions' } },
];

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
  return { status: raw.status, stdout: stdout.toString('utf8') };
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
  mkdirSync(destination);
  for (const child of ['home', 'tmp', 'cargo-home']) mkdirSync(join(destination, child));
  return destination;
}

function framed(submissions) {
  return Buffer.concat(submissions.map(source => Buffer.from(`${Buffer.byteLength(source)}\n${source}`)));
}

// The observed fields a scenario's expectation names, from its report.
function observe(report, expected) {
  const observed = {};
  for (const key of Object.keys(expected)) {
    if (key === 'stack') {
      observed.stack = (report.stack ?? []).map(entry => ({ type: entry.type, value: entry.value }));
    } else {
      observed[key] = report[key];
    }
  }
  return observed;
}

function main() {
  const [requested] = process.argv.slice(2);
  requireThat(requested, 'usage: SELECTED_NODE verification/signed-self/scenarios.mjs NEW_EXTERNAL_DIRECTORY');
  const selection = JSON.parse(readFileSync(join(root, 'policy/tool-selection.json')));
  requireThat(realpathSync(process.execPath) === realpathSync(join(selection.tool_paths.node.output, 'bin/node')),
    'run this script with the selected Node');
  out = outputDirectory(requested);
  const git = realpathSync('/run/current-system/sw/bin/git');
  const rustBin = join(selection.tool_paths.quality_rust.output, 'bin');
  const vendor = selectedVendor(selection.component_sync);
  writeFileSync(join(out, 'cargo-home', 'config.toml'),
    '[source.crates-io]\nreplace-with = "reviewed-vendor"\n[source.reviewed-vendor]\n' +
    `directory = ${JSON.stringify(vendor.directory)}\n[net]\noffline = true\n`);
  mkdirSync(join(root, 'target'), { recursive: true });
  const target = realpathSync(join(root, 'target'));
  const env = {
    HOME: join(out, 'home'), TMPDIR: join(out, 'tmp'), LANG: 'C.UTF-8', GIT_CONFIG_NOSYSTEM: '1',
    GIT_CONFIG_GLOBAL: '/dev/null',
    PATH: [rustBin, selection.component_sync.linker_bin, '/run/current-system/sw/bin', '/usr/bin', '/bin'].join(':'),
    CARGO_HOME: join(out, 'cargo-home'), CARGO_TARGET_DIR: target, CARGO_NET_OFFLINE: 'true',
    CARGO_INCREMENTAL: '0', RUSTC_WRAPPER: '', RUSTFLAGS: '',
    CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER: join(selection.component_sync.linker_bin, 'cc'),
  };
  const state = () => ({
    commit: run('source-head', git, ['rev-parse', 'HEAD'], { env }).stdout.trim(),
    tree: run('source-tree', git, ['rev-parse', 'HEAD^{tree}'], { env }).stdout.trim(),
    status: run('source-status', git, ['status', '--porcelain=v1', '--untracked-files=all'], { env }).stdout,
  });
  const before = state();
  requireThat(before.status === '', `the source worktree is dirty:\n${before.status}`);
  run('source-cli-build', join(rustBin, 'cargo'), ['build', '--locked', '--offline', '-j', '4', '-p', 'noble-cli'], { env });
  const cli = join(target, 'debug/noble');
  const cliSha256 = fileHash(cli);
  const recorded = {};
  for (const session of sessions) {
    const result = run(`session-${session.id}`, cli,
      ['session', '--framed', '--declared-modules', '--text-byte-cursor', ...session.flags],
      { env, input: framed(session.submissions), allowFailure: true });
    const reports = result.stdout.split('\n').filter(Boolean).map(line => JSON.parse(line));
    requireThat(result.status === session.exit, `${session.id}: exit ${result.status}, expected ${session.exit}`);
    recorded[session.id] = { ...session, exit_status: result.status, reports };
  }
  const cases = scenarios.map(scenario => {
    const report = recorded[scenario.session].reports[scenario.report];
    requireThat(report, `${scenario.id}: no report at ${scenario.session}[${scenario.report}]`);
    requireThat(report.profile === scenario.profile, `${scenario.id}: profile ${report.profile}`);
    requireThat(report.guest_requests === 0 && report.protected_operations === 0,
      `${scenario.id}: guest requests or protected operations`);
    const observed = observe(report, scenario.expected);
    requireThat(stable(observed) === stable(scenario.expected),
      `${scenario.id}: observed ${stable(observed)}, expected ${stable(scenario.expected)}`);
    const { report: reportIndex, ...rest } = scenario;
    return { ...rest, report_index: reportIndex, status: 'passed', observed, report };
  });
  requireThat(stable(state()) === stable(before), 'the source changed during the run');
  const sources = ['verification/signed-self/scenarios.mjs', 'crates/noble-cli/tests/smoke.rs'];
  const receipt = {
    schema: 'noble-signed-self-scenarios/v1', issue: 99, requirements: ['K-CHECK-02', 'K-ERROR-03'],
    source_commit: before.commit, source_tree: before.tree, clean_source: true,
    sources_sha256: Object.fromEntries(sources.map(file => [file, fileHash(join(root, file))])),
    toolchain: {
      cargo: run('cargo-version', join(rustBin, 'cargo'), ['--version'], { env }).stdout.trim(),
      vendor_nar_hash: vendor.narHash, cli_sha256: cliSha256,
    },
    sessions: Object.values(recorded).map(({ id, flags, submissions, exit_status, reports }) =>
      ({ id, flags, submissions, exit_status, reports })),
    cases, commands,
    assumptions: [
      'Each scenario is one report of a production framed session of the CLI built from this commit.',
      'A runaway ends the session: the process exits 5 after the runtime-exhausted report.',
      'The observations describe these executions only; the K-CHECK-02 model theorem is verification/signed-self/check.mjs.',
    ],
  };
  const path = join(out, 'receipt.json');
  writeFileSync(path, `${JSON.stringify(receipt, null, 2)}\n`, { flag: 'wx' });
  console.log(stable({ result: 'passed', receipt: path, source_commit: before.commit, cases: cases.length,
    receipt_sha256: fileHash(path) }));
}

try {
  main();
} catch (error) {
  console.error(`[signed-self-scenarios] FAIL ${error.message}`);
  process.exitCode = 1;
}
