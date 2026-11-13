// Execute the behavioral-effect-protocol scenarios EPROTO-CASE-01…11 against the source-built
// bounded-region adapter, with the `noble-bounded-region/v2` report and the RegionProtocol-v1
// trace monitor. The hostile components are core modules written here, independently of the
// Noble compiler, and wrapped with the selected wasm-tools. Every verdict is finite execution
// evidence about the run that produced it, not a protocol proof.
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import {
  lstatSync, mkdirSync, readFileSync, readdirSync, realpathSync, writeFileSync,
} from 'node:fs';
import {
  basename, dirname, join, relative, resolve, sep,
} from 'node:path';
import { fileURLToPath } from 'node:url';
import { selectedVendor } from '../selected-vendor.mjs';
import { monitor } from './monitor.mjs';

const root = resolve(fileURLToPath(import.meta.url), '../../..');
const witRelative = 'crates/noble-wasm/wit/bounded-region.wit';
const worldName = 'bounded';
const worldId = 'noble-test:bounded-region/bounded@1.0.0';
const subjectRelative = 'verification/host-locality/fixtures/read-release.noble';
const region = '000102';
const maxFuel = 10_000_000;
const json = value => `${JSON.stringify(value, null, 2)}\n`;
const stable = value => JSON.stringify(value);
const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const fileHash = path => digest(readFileSync(path));
const requireThat = (condition, message) => { if (!condition) throw Error(message); };
const relativeOutputPath = (out, path) => relative(out, path).split(sep).join('/');

let out = null;
let baseEnv = null;
const commands = [];
const cases = [];
const controls = [];
let commandNumber = 0;

function run(label, executable, argv, options = {}) {
  const { allowFailure = false, env = baseEnv, cwd = root } = options;
  const id = String(++commandNumber).padStart(3, '0');
  const captureDirectory = join(out, 'commands', id);
  mkdirSync(captureDirectory);
  const raw = spawnSync(executable, argv, {
    cwd, env, encoding: null, maxBuffer: 64 * 1024 * 1024, timeout: 3_600_000,
  });
  const stdoutBytes = raw.stdout ?? Buffer.alloc(0);
  const stderrBytes = raw.stderr ?? Buffer.alloc(0);
  writeFileSync(join(captureDirectory, 'stdout'), stdoutBytes);
  writeFileSync(join(captureDirectory, 'stderr'), stderrBytes);
  const record = {
    id, label,
    executable: realpathSync(executable).replace(`${realpathSync(root)}${sep}`, `$ROOT${sep}`),
    argv: argv.map(arg => arg.replace(out, '$OUT')),
    exit_status: raw.status, signal: raw.signal, spawn_error: raw.error ? String(raw.error) : null,
    stdout_sha256: digest(stdoutBytes), stderr_sha256: digest(stderrBytes),
  };
  writeFileSync(join(captureDirectory, 'command.json'), json(record));
  commands.push(record);
  const result = {
    status: raw.status, stdoutBytes, stdout: stdoutBytes.toString('utf8'),
    stderr: stderrBytes.toString('utf8'), record,
  };
  if (raw.error || (raw.status !== 0 && !allowFailure)) {
    throw Error(`${label} failed (${raw.error?.message ?? raw.status ?? raw.signal}): ${result.stderr}`);
  }
  return result;
}

function gitState(git) {
  return {
    head: run('source-head', git, ['rev-parse', 'HEAD']).stdout.trim(),
    tree: run('source-tree', git, ['rev-parse', 'HEAD^{tree}']).stdout.trim(),
    status: run('source-status', git, ['status', '--porcelain=v1', '--untracked-files=all']).stdout,
  };
}

function sourceHashes(git) {
  const listed = run('source-path-inventory', git, [
    'ls-files', '-z', '--cached', '--others', '--exclude-standard', '--',
    'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', '.cargo', 'crates',
    'policy/tool-selection.json', 'verification/selected-vendor.mjs',
    'verification/effect-protocol', subjectRelative,
  ]);
  const paths = listed.stdoutBytes.toString('utf8').split('\0').filter(Boolean).sort();
  for (const path of [subjectRelative, 'verification/effect-protocol/execution.mjs',
    'verification/effect-protocol/monitor.mjs']) {
    requireThat(paths.includes(path), `source inventory omits ${path}`);
  }
  const hashes = {};
  for (const path of paths) {
    const absolute = join(root, path);
    requireThat(lstatSync(absolute).isFile(), `nonregular source input: ${path}`);
    hashes[path] = fileHash(absolute);
  }
  return hashes;
}

function outputDirectory(requested) {
  const resolved = resolve(requested);
  const destination = join(realpathSync(dirname(resolved)), basename(resolved));
  try {
    lstatSync(destination);
    throw Error(`NEW_EXTERNAL_DIRECTORY already exists: ${destination}`);
  } catch (error) {
    if (error.code !== 'ENOENT') throw error;
  }
  const realRoot = realpathSync(root);
  requireThat(destination !== realRoot && !destination.startsWith(`${realRoot}${sep}`),
    'NEW_EXTERNAL_DIRECTORY must be outside the source worktree');
  mkdirSync(destination);
  for (const child of ['commands', 'home', 'tmp', 'guest', 'hostile', 'compiled']) {
    mkdirSync(join(destination, child));
  }
  return destination;
}

function checkEnvironment(selection, selectedNode) {
  const forbidden = Object.keys(process.env).filter(key => {
    if (key === 'GIT_TERMINAL_PROMPT') return process.env[key] !== '0';
    return /^(?:LEAN(?:_|$)|LAKE(?:_|$)|NODE(?:_|$)|BASH(?:_|$)|ENV$|LD_|DYLD_|GIT_|NIX_|ELAN|CARGO_|RUST|NOBLE_)/
      .test(key) && process.env[key] !== '';
  });
  requireThat(forbidden.length === 0, `unsafe caller environment: ${forbidden.sort().join(', ')}`);
  requireThat(realpathSync(process.execPath) === realpathSync(selectedNode),
    `run this script with the selected Node executable ${selectedNode}`);
  requireThat(process.execArgv.length === 0, 'unreviewed Node runtime flags are not accepted');
}

// A number a JavaScript double cannot hold exactly keeps its decimal text (#90).
function parseExactJson(text, label) {
  try {
    return JSON.parse(text, (key, value, context) =>
      (typeof value === 'number' && !Number.isSafeInteger(value) ? JSON.rawJSON(context.source) : value));
  } catch (error) {
    throw Error(`${label} did not emit JSON: ${error.message}`);
  }
}

const decimal = value => (JSON.isRawJSON(value) ? value.rawJSON : String(value));

function fileRecord(path) {
  const bytes = readFileSync(path);
  return { path: relativeOutputPath(out, path), sha256: digest(bytes), byte_length: bytes.length };
}

function artifactHashes() {
  const files = {};
  function walk(directory, prefix = '') {
    for (const entry of readdirSync(directory, { withFileTypes: true })
      .sort((a, b) => a.name.localeCompare(b.name))) {
      const rel = prefix ? `${prefix}/${entry.name}` : entry.name;
      const path = join(directory, entry.name);
      if (entry.isDirectory()) walk(path, rel);
      else if (entry.isFile()) files[rel] = fileHash(path);
      else throw Error(`unexpected nonregular output artifact: ${rel}`);
    }
  }
  walk(out);
  delete files['execution.json'];
  return files;
}

// The hostile core modules. Each imports the two RegionProtocol operations with the synchronous
// memory32 Canonical ABI the selected WIT world lowers to, owns a bump allocator for the host's
// lowered results, and exports `read-region` whose body is the scenario's behavior. A host read
// writes its result at 16 (tag, pointer, length); a body returns 16 to pass that result on, or
// builds its own result at 32.
function hostileModule(body) {
  return `(module
  (import "noble-test:bounded-region/regions@1.0.0" "[method]region.read"
    (func $read (param i32 i64 i64 i32)))
  (import "noble-test:bounded-region/regions@1.0.0" "release"
    (func $release (param i32)))
  (memory (export "memory") 1)
  (data (i32.const 512) "timeout")
  (global $next (mut i32) (i32.const 1024))
  (func (export "cabi_realloc") (param $old i32) (param $old_size i32) (param $align i32) (param $size i32) (result i32)
    (local $ptr i32)
    local.get $old_size
    if
      unreachable
    end
    global.get $next
    local.get $align
    i32.add
    i32.const 1
    i32.sub
    i32.const 0
    local.get $align
    i32.sub
    i32.and
    local.tee $ptr
    local.get $size
    i32.add
    global.set $next
    local.get $ptr)
  (func (export "cm32p2||read-region") (param $owner i32) (param $offset i64) (param $length i64) (result i32)
    (local $count i32)
${body.map(line => `    ${line}`).join('\n')})
  (func (export "cm32p2||read-region_post") (param i32))
)
`;
}

const READ = ['local.get $owner', 'local.get $offset', 'local.get $length', 'i32.const 16', 'call $read'];
const RELEASE = ['local.get $owner', 'call $release'];
const HOSTILE = {
  'trap-after-commit': [...READ, 'unreachable'],
  'trap-before-use': ['unreachable'],
  'omitted-release': [...READ, 'i32.const 16'],
  'scope-exhaustion': [
    'loop $again', ...READ.map(line => `  ${line}`),
    '  local.get $count', '  i32.const 1', '  i32.add', '  local.tee $count', '  i32.const 65',
    '  i32.lt_u', '  br_if $again', 'end', ...RELEASE, 'i32.const 16',
  ],
  'double-release': [...RELEASE, ...RELEASE, 'i32.const 16'],
  'read-after-release': [...RELEASE, ...READ, 'i32.const 16'],
  'read-twice': [...READ, ...READ, ...RELEASE, 'i32.const 16'],
  'read-then-spin': [...READ, 'loop $spin', '  br $spin', 'end', 'unreachable'],
  // err("timeout"): tag 1 at 32, the static string at 512, length 7.
  'unmodeled-error': [
    ...RELEASE, 'i32.const 32', 'i32.const 1', 'i32.store8', 'i32.const 36', 'i32.const 512',
    'i32.store', 'i32.const 40', 'i32.const 7', 'i32.store', 'i32.const 32',
  ],
  // The host's ok, returned one byte short.
  'short-ok': [
    ...READ, ...RELEASE, 'i32.const 24', 'i32.const 24', 'i32.load', 'i32.const 1', 'i32.sub',
    'i32.store', 'i32.const 16',
  ],
};

function assembleHostile(label, wasmTools, witBytes) {
  const directory = join(out, 'hostile', label);
  mkdirSync(directory);
  writeFileSync(join(directory, 'world.wit'), witBytes, { flag: 'wx' });
  writeFileSync(join(directory, 'module.wat'), hostileModule(HOSTILE[label]), { flag: 'wx' });
  const steps = [
    ['parse', 'module.wat', '-o', 'core.wasm'],
    ['component', 'embed', '--world', worldName, '--encoding', 'utf8', 'world.wit', 'core.wasm',
      '-o', 'embedded.wasm'],
    ['component', 'new', 'embedded.wasm', '-o', 'component.wasm'],
    ['validate', 'component.wasm'],
  ].map((argv, step) => run(`hostile-${label}-${step}`, wasmTools, argv, { cwd: directory }).record.id);
  const path = join(directory, 'component.wasm');
  return {
    label, path,
    record: {
      wat: fileRecord(join(directory, 'module.wat')),
      core: fileRecord(join(directory, 'core.wasm')),
      component: fileRecord(path),
      component_base64: readFileSync(path).toString('base64'),
      assembly_commands: steps,
    },
  };
}

function kinds(report) {
  return report.events.map(event => (event.kind === 'request' ? event.operation
    : event.kind === 'commit' ? 'commit' : `response:${event.result}`));
}

const CLEAN_READ = ['region.read', 'commit', 'response:value'];
const GUEST_RELEASE = ['region.release', 'response:value'];
const HOST_RETIRE = ['host.retire', 'response:value'];

// One public CLI invocation with the v2 report.
function invoke(context, { label, component, offset, length, flags = [] }) {
  const result = run(`read-region-${label}`, context.cli, [
    'component', 'read-region', '--report', 'v2', ...flags, component.path, region,
    String(offset), String(length),
  ], { env: context.cliEnv, allowFailure: true });
  requireThat(result.status === 0, `${label}: read-region failed: ${result.stderr}`);
  const report = parseExactJson(result.stdout, `read-region ${label}`);
  requireThat(report.schema === 'noble-bounded-region/v2' && report.stage === 'adapter',
    `${label}: not a v2 adapter report`);
  return {
    report, command_id: result.record.id,
    input: { host_buffer_hex: region, offset: String(offset), length: String(length), flags },
  };
}

// Run one scenario, check its expected observation and the monitor's verdicts, and retain it.
// Every executed scenario must end with no live owner, no native pin, no lost owner, no native
// memory access and, unless the trace is truncated, exactly one release commit.
function scenario(context, {
  id, variant, kind, component, offset, length, flags, expected, verdicts, errorIncludes = null,
}) {
  const label = `${id}-${variant}`;
  const { report, command_id: commandId, input } = invoke(context,
    { label, component, offset, length, flags });
  const observed = {
    outcome: report.outcome, exit_class: report.exit_class, bytes_hex: report.bytes_hex,
    trace: kinds(report), region_reads: report.region_reads,
    events_truncated: report.events_truncated, event_count: report.events.length,
    releases: report.releases, retirement_reason: report.cleanup.retirement_reason,
    cleanup_state: report.cleanup.state,
  };
  for (const [key, value] of Object.entries(expected)) {
    const actual = key.startsWith('refusals.') ? report.refusals[key.slice(9)]
      : key === 'cancellation' ? report.cancellation : observed[key];
    requireThat(stable(actual) === stable(value),
      `${label}.${key}: expected ${stable(value)}, observed ${stable(actual)}`);
  }
  requireThat(report.native_memory_access === false && report.live_owners === 0
    && report.native_pins === 0 && report.lost_owners === 0,
  `${label}: an owner, pin or native access survived: ${stable(report)}`);
  // #50: the report names the owner the host registered before guest start, and every event of
  // this one-owner host carries exactly that identity, a host retirement included.
  requireThat(Number.isInteger(report.registration?.identity)
    && report.events.every(event => !('identity' in event) || event.identity === report.registration.identity),
  `${label}: an event names an identity the host did not register: ${stable(report)}`);
  if (errorIncludes !== null) {
    requireThat(typeof report.error === 'string' && report.error.includes(errorIncludes),
      `${label}: the error does not name ${stable(errorIncludes)}: ${stable(report.error)}`);
  }
  const checked = monitor(report);
  requireThat(stable(checked.codes) === stable(verdicts),
    `${label}: monitor verdicts ${stable(checked.codes)}, expected ${stable(verdicts)}`);
  const row = {
    id, variant, kind, status: 'passed', component: component.name, input,
    expected, expected_error: errorIncludes, expected_verdicts: verdicts, monitor: checked,
    observed: report, command_id: commandId,
  };
  cases.push(row);
  return row;
}

function compile(context, label, source) {
  const guestPath = join(out, 'guest', `${label}.noble`);
  writeFileSync(guestPath, source, { flag: 'wx' });
  const directory = join(out, 'compiled', label);
  const result = run(`compile-${label}`, context.cli, [
    'component', 'compile', witRelative, worldName, directory, `read-region=${guestPath}`,
  ], { env: context.cliEnv, allowFailure: true });
  return { result, report: parseExactJson(result.stdout, `compile ${label}`), directory, guestPath };
}

function hostFixtureTests(cargo, cliEnv) {
  const names = [
    'tests::invalid_claims_and_boundaries_do_not_read_or_consume_region',
    'tests::same_slot_and_generation_from_another_live_invocation_cannot_cross',
    'report_v2_tests::v2_classifies_scope_exhaustion_apart_from_an_identity_refusal',
    'report_v2_tests::results_outside_the_result_schema_are_outside_the_model',
    'report_v2_tests::cancellation_revokes_access_once_the_requested_reads_are_answered',
  ];
  const tests = run('direct-host-fixtures', cargo, [
    'test', '--locked', '--offline', '-j', '4', '-p', 'noble-cli', '--bin', 'noble',
    'component::bounded_region', '--', '--test-threads=1',
  ], { env: cliEnv, allowFailure: true });
  requireThat(tests.status === 0, `direct Host fixtures failed: ${tests.stderr}`);
  for (const name of names) {
    requireThat(new RegExp(`test component::bounded_region::${name} \\.\\.\\. ok`).test(tests.stdout),
      `direct Host fixture did not pass: ${name}`);
  }
  return { command_id: tests.record.id, tests: names };
}

// Each control mutates the real EPROTO-CASE-01 trace into one the transition table forbids; the
// monitor must report it. The unmutated trace must stay clean. The two host-* controls are the
// traces a modified host answering outside the result schema would record (EPROTO-CASE-11).
function monitorControls(base) {
  const copy = () => parseExactJson(stable(base), 'trace copy');
  const [readRequest, commit, readResponse, releaseRequest, releaseResponse] = copy().events;
  const retire = [{ ...releaseRequest, operation: 'host.retire' }, releaseResponse];
  const release = [releaseRequest, releaseResponse];
  const mutations = [
    ['omitted-release', 'omitted-release', [readRequest, commit, readResponse, ...retire]],
    ['double-release', 'double-release', [readRequest, commit, readResponse, ...release, ...release]],
    ['post-release-use', 'post-release-use',
      [readRequest, commit, readResponse, ...release, readRequest, commit, readResponse]],
    ['unsolicited-response', 'unsolicited-response',
      [releaseResponse, readRequest, commit, readResponse, ...release]],
    ['host-short-ok', 'outside-model',
      [readRequest, commit, { ...readResponse, bytes: 1 }, ...release]],
    ['host-unmodeled-error', 'outside-model',
      [readRequest, { ...readResponse, result: 'refused', bytes: null, reason: 'timeout' }, ...release]],
    ['uncommitted-read', 'uncommitted-read', [readRequest, readResponse, ...release]],
    ['unanswered-request', 'unanswered-request', [readRequest, commit, ...release]],
    ['unsolicited-commit', 'unsolicited-commit', [commit, readRequest, commit, readResponse, ...release]],
    ['double-commit', 'unsolicited-commit', [readRequest, commit, commit, readResponse, ...release]],
    ['release-count', 'release-count', [readRequest, commit, readResponse]],
  ];
  requireThat(monitor(base).codes.length === 0, 'the monitor flags the unmutated EPROTO-CASE-01 trace');
  for (const [id, verdict, events] of mutations) {
    const mutated = { ...copy(), events };
    const { codes } = monitor(mutated);
    requireThat(codes.includes(verdict), `monitor control ${id}: reported ${stable(codes)}`);
    controls.push({ id: `monitor-${id}`, status: 'passed', class: 'monitor-mutation-control',
      expected_verdict: verdict, reported: codes, trace: kinds(mutated) });
  }
}

function main() {
  const args = process.argv.slice(2);
  requireThat(args.length === 1 || (args.length === 2 && args[1] === '--allow-dirty'),
    'usage: SELECTED_NODE verification/effect-protocol/execution.mjs NEW_EXTERNAL_DIRECTORY [--allow-dirty]');
  const allowDirty = args[1] === '--allow-dirty';
  const selection = JSON.parse(readFileSync(join(root, 'policy/tool-selection.json')));
  const selectedNode = join(selection.tool_paths.node.output, 'bin/node');
  checkEnvironment(selection, selectedNode);
  out = outputDirectory(args[0]);
  const rustBin = join(selection.tool_paths.quality_rust.output, 'bin');
  const cargo = join(rustBin, 'cargo');
  const wasmTools = join(selection.tool_paths.wasm_tools.output, 'bin/wasm-tools');
  const linker = join(selection.component_sync.linker_bin, 'cc');
  const git = realpathSync('/run/current-system/sw/bin/git');
  baseEnv = {
    HOME: join(out, 'home'), USER: 'noble-effect-protocol', LOGNAME: 'noble-effect-protocol',
    LANG: 'C.UTF-8', LC_ALL: 'C.UTF-8', CI: '1', GIT_TERMINAL_PROMPT: '0',
    GIT_CONFIG_NOSYSTEM: '1', GIT_CONFIG_GLOBAL: '/dev/null',
    PATH: [rustBin, selection.component_sync.linker_bin, '/run/current-system/sw/bin', '/usr/bin', '/bin'].join(':'),
    TMPDIR: join(out, 'tmp'),
  };

  const initialState = gitState(git);
  const hashes = sourceHashes(git);
  const cleanSource = initialState.status === '';
  requireThat(cleanSource || allowDirty,
    'source worktree is dirty; re-run with --allow-dirty to bind the current source snapshot');
  const wasmToolsVersion = run('selected-wasm-tools-version', wasmTools, ['--version']).stdout.trim();
  requireThat(wasmToolsVersion === `wasm-tools ${selection.component_sync.wasm_tools_version}`,
    'selected wasm-tools version differs from policy');

  const vendor = selectedVendor(selection.component_sync);
  const cargoHome = join(out, 'cargo-home');
  mkdirSync(cargoHome);
  writeFileSync(join(cargoHome, 'config.toml'),
    '[source.crates-io]\nreplace-with = "reviewed-vendor"\n[source.reviewed-vendor]\n' +
    `directory = ${JSON.stringify(vendor.directory)}\n[net]\noffline = true\n`, { flag: 'wx' });
  mkdirSync(join(root, 'target'), { recursive: true });
  const target = realpathSync(join(root, 'target'));
  const cliEnv = {
    ...baseEnv,
    CARGO_HOME: cargoHome, CARGO_TARGET_DIR: target, CARGO_NET_OFFLINE: 'true',
    CARGO_BUILD_JOBS: '4', CARGO_INCREMENTAL: '0',
    RUSTC: join(rustBin, 'rustc'), RUSTDOC: join(rustBin, 'rustdoc'),
    RUSTC_WRAPPER: '', RUSTC_WORKSPACE_WRAPPER: '', CARGO_BUILD_RUSTC_WRAPPER: '',
    RUSTFLAGS: '', CARGO_ENCODED_RUSTFLAGS: '',
    CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS: '',
    CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER: linker,
  };
  const build = run('source-cli-build', cargo, ['build', '--locked', '--offline', '-j', '4', '-p', 'noble-cli'],
    { env: cliEnv });
  const cli = join(target, 'debug/noble');
  const cliSha256 = fileHash(cli);
  const context = { cli, cliEnv };

  // The compiled subject `region.read swap regions.release`.
  const witBytes = readFileSync(join(root, witRelative));
  const compiled = compile(context, 'subject', readFileSync(join(root, subjectRelative)));
  requireThat(compiled.result.status === 0 && compiled.report.outcome === 'compiled'
    && compiled.report.world === worldId && compiled.report.independent_kernel_check === true,
  `the selected subject did not compile: ${compiled.result.stdout}`);
  const subject = { name: 'compiled-subject', path: join(compiled.directory, 'component.wasm') };
  const subjectRecord = {
    source: subjectRelative, source_sha256: hashes[subjectRelative],
    component: fileRecord(subject.path),
    component_base64: readFileSync(subject.path).toString('base64'),
    compile_command: compiled.result.record.id,
  };
  const hostile = {};
  for (const label of Object.keys(HOSTILE)) {
    const assembled = assembleHostile(label, wasmTools, witBytes);
    hostile[label] = { name: label, path: assembled.path, record: assembled.record };
  }

  // EPROTO-CASE-01: success.
  const success = [[1, 2, '0102'], [3, 0, '']].map(([offset, length, bytes]) => scenario(context, {
    id: 'EPROTO-CASE-01', variant: `read-${offset}-${length}`, kind: 'compiled-noble-subject',
    component: subject, offset, length,
    expected: { outcome: 'read', exit_class: 'normal', bytes_hex: bytes,
      trace: [...CLEAN_READ, ...GUEST_RELEASE], region_reads: 1,
      releases: { guest: 1, destructor: 0, host_retirement: 0 } },
    verdicts: [],
  }));

  // EPROTO-CASE-02: ordinary errors.
  for (const [offset, length] of [[3, 1], [2, 2], [-1, 1], ['9223372036854775807', 1]]) {
    scenario(context, {
      id: 'EPROTO-CASE-02', variant: `bounds-${offset}-${length}`, kind: 'compiled-noble-subject',
      component: subject, offset, length,
      expected: { outcome: 'bounds-reject', exit_class: 'rejected', bytes_hex: null,
        trace: ['region.read', 'response:refused', ...GUEST_RELEASE], region_reads: 0,
        releases: { guest: 1, destructor: 0, host_retirement: 0 }, 'refusals.bounds': 1 },
      verdicts: [],
    });
  }

  // EPROTO-CASE-03 (i): 65 in-bounds reads meet the scope ceiling. The v2 bound reaches the
  // refusal (three events per admitted read, then the refused read and the guest's release), so the
  // trace itself records the host's refusal of the read past the ceiling instead of truncating.
  const exhaustion = scenario(context, {
    id: 'EPROTO-CASE-03', variant: 'scope-exhaustion', kind: 'hostile-component',
    component: hostile['scope-exhaustion'], offset: 0, length: 1,
    expected: { outcome: 'invalid-handle', exit_class: 'rejected', region_reads: 64,
      events_truncated: false, event_count: 3 * 64 + 4,
      releases: { guest: 1, destructor: 0, host_retirement: 0 },
      'refusals.scope-exhausted': 1, 'refusals.identity': 0, 'refusals.bounds': 0 },
    verdicts: [],
  });
  {
    const { events, events_truncated: truncated } = exhaustion.observed;
    requireThat(!truncated, 'EPROTO-CASE-03: the v2 trace still truncates before the scope ceiling');
    requireThat(events.length === 3 * 64 + 4,
      `EPROTO-CASE-03: the trace records ${events.length} events, not the whole ceiling trace`);
    const [request, refusal] = events.slice(-4, -2);
    requireThat(request.kind === 'request' && request.operation === 'region.read'
      && request.offset === 0 && request.length === 1,
    `EPROTO-CASE-03: the read past the ceiling is not recorded: ${stable(request)}`);
    requireThat(refusal.kind === 'response' && refusal.result === 'refused'
      && refusal.reason === 'invalid-handle' && refusal.host_class === 'scope-exhausted',
    `EPROTO-CASE-03: the host's refusal is not recorded as the runtime host-refusal trace: ${stable(refusal)}`);
    const [release, answer] = events.slice(-2);
    requireThat(release.kind === 'request' && release.operation === 'region.release'
      && answer.kind === 'response' && answer.result === 'value',
    'EPROTO-CASE-03: the guest release after the refusal is not recorded');
    requireThat(exhaustion.monitor.release_commits === 1,
      `EPROTO-CASE-03: the complete trace has ${exhaustion.monitor.release_commits} release commits`);
  }
  const fixtures = hostFixtureTests(cargo, cliEnv);
  cases.push({
    id: 'EPROTO-CASE-03', variant: 'forged-and-cross-host-claims', kind: 'direct-host-fixture',
    status: 'passed', fixture_level: true, command_id: fixtures.command_id, tests: fixtures.tests,
    observation: 'Forged (table, context, generation, kind, rights, representation) and cross-host claims reach only direct Host::read fixtures: a guest always presents the retained handle. Each is refused with invalid-handle before any protected read, and the owner is unchanged until its single release.',
  });
  cases.push({
    id: 'EPROTO-CASE-03', variant: 'native-access-refusal', kind: 'direct-host-fixture',
    status: 'not-exercised',
    observation: 'Host::read passes native_access only the borrow begin has just opened, and T4 proves the table authorizes it (open_borrow_returns, proofs/m5/RegionTable.lean). No direct Host fixture reaches the refusal without corrupting table state.',
  });

  // EPROTO-CASE-04: a trap after the protected commit.
  scenario(context, {
    id: 'EPROTO-CASE-04', variant: 'trap-after-commit', kind: 'hostile-component',
    component: hostile['trap-after-commit'], offset: 1, length: 2,
    expected: { outcome: 'adapter-trap', exit_class: 'trap', trace: [...CLEAN_READ, ...HOST_RETIRE],
      region_reads: 1, releases: { guest: 0, destructor: 0, host_retirement: 1 },
      retirement_reason: 'trap' },
    verdicts: [],
  });

  // EPROTO-CASE-05: a trap before any use.
  scenario(context, {
    id: 'EPROTO-CASE-05', variant: 'trap-before-use', kind: 'hostile-component',
    component: hostile['trap-before-use'], offset: 1, length: 2,
    expected: { outcome: 'adapter-trap', exit_class: 'trap', trace: HOST_RETIRE, region_reads: 0,
      releases: { guest: 0, destructor: 0, host_retirement: 1 }, retirement_reason: 'trap' },
    verdicts: [],
  });

  // EPROTO-CASE-06: the smallest fuel budget that reaches the subject's read exhausts the quota
  // after the commit and before the release; one unit less never reaches the read.
  const reads = fuel => invoke(context, { label: `fuel-probe-${fuel}`, component: subject,
    offset: 1, length: 2, flags: ['--fuel', String(fuel)] }).report.region_reads;
  requireThat(reads(maxFuel) === 1, 'the subject does not read within the default fuel');
  let low = 1;
  let high = maxFuel;
  while (low < high) {
    const middle = Math.floor((low + high) / 2);
    if (reads(middle) >= 1) high = middle;
    else low = middle + 1;
  }
  const quota = scenario(context, {
    id: 'EPROTO-CASE-06', variant: `fuel-${low}`, kind: 'compiled-noble-subject-fuel-knob',
    component: subject, offset: 1, length: 2, flags: ['--fuel', String(low)],
    expected: { outcome: 'adapter-trap', exit_class: 'quota-exhausted', trace: [...CLEAN_READ, ...HOST_RETIRE],
      region_reads: 1, releases: { guest: 0, destructor: 0, host_retirement: 1 },
      retirement_reason: 'quota-exhausted' },
    verdicts: [],
  });
  scenario(context, {
    id: 'EPROTO-CASE-06', variant: `fuel-${low - 1}`, kind: 'compiled-noble-subject-fuel-knob',
    component: subject, offset: 1, length: 2, flags: ['--fuel', String(low - 1)],
    expected: { outcome: 'adapter-trap', exit_class: 'quota-exhausted', trace: HOST_RETIRE,
      region_reads: 0, releases: { guest: 0, destructor: 0, host_retirement: 1 },
      retirement_reason: 'quota-exhausted' },
    verdicts: [],
  });
  quota.boundary = { smallest_reading_fuel: low };

  // EPROTO-CASE-07: the host cancels after the first answered read. The second read never
  // reaches the protocol; a guest that only computes is interrupted, where without the control the
  // same component exhausts its quota.
  const cancelled = { guest: 0, destructor: 0, host_retirement: 1 };
  const cancel = ['--cancel-after-reads', '1'];
  scenario(context, {
    id: 'EPROTO-CASE-07', variant: 'cancel-between-two-reads', kind: 'hostile-component',
    component: hostile['read-twice'], offset: 1, length: 2, flags: cancel,
    expected: { outcome: 'adapter-trap', exit_class: 'cancelled', trace: [...CLEAN_READ, ...HOST_RETIRE],
      region_reads: 1, releases: cancelled, retirement_reason: 'cancelled',
      cancellation: { after_reads: 1, requested: true } },
    verdicts: [],
  });
  scenario(context, {
    id: 'EPROTO-CASE-07', variant: 'cancel-a-computing-guest', kind: 'hostile-component',
    component: hostile['read-then-spin'], offset: 1, length: 2, flags: cancel,
    expected: { outcome: 'adapter-trap', exit_class: 'cancelled', trace: [...CLEAN_READ, ...HOST_RETIRE],
      region_reads: 1, releases: cancelled, retirement_reason: 'cancelled' },
    verdicts: [],
  });
  scenario(context, {
    id: 'EPROTO-CASE-07', variant: 'same-guest-uncancelled', kind: 'hostile-component',
    component: hostile['read-then-spin'], offset: 1, length: 2,
    expected: { outcome: 'adapter-trap', exit_class: 'quota-exhausted', trace: [...CLEAN_READ, ...HOST_RETIRE],
      region_reads: 1, releases: cancelled, retirement_reason: 'quota-exhausted',
      cancellation: { after_reads: null, requested: false } },
    verdicts: [],
  });
  scenario(context, {
    id: 'EPROTO-CASE-07', variant: 'cancel-the-compiled-subject', kind: 'compiled-noble-subject',
    component: subject, offset: 1, length: 2, flags: cancel,
    expected: { outcome: 'adapter-trap', exit_class: 'cancelled', trace: [...CLEAN_READ, ...HOST_RETIRE],
      region_reads: 1, releases: cancelled, retirement_reason: 'cancelled' },
    verdicts: [],
  });

  // EPROTO-CASE-08: the guest returns its bytes without releasing.
  scenario(context, {
    id: 'EPROTO-CASE-08', variant: 'omitted-release', kind: 'hostile-component',
    component: hostile['omitted-release'], offset: 1, length: 2,
    expected: { outcome: 'read', exit_class: 'normal', bytes_hex: '0102', trace: [...CLEAN_READ, ...HOST_RETIRE],
      releases: { guest: 0, destructor: 0, host_retirement: 1 }, retirement_reason: 'omitted-release' },
    verdicts: ['omitted-release'],
  });

  // EPROTO-CASE-09: Wasmtime's handle table refuses the consumed handle before the second call
  // reaches the host. It reports an untyped error, not a Trap, so the adapter classifies the exit
  // as adapter-invalid; the v2 error chain carries the cause.
  for (const label of ['double-release', 'read-after-release']) {
    scenario(context, {
      id: 'EPROTO-CASE-09', variant: label, kind: 'hostile-component',
      component: hostile[label], offset: 1, length: 2,
      expected: { outcome: 'adapter-trap', exit_class: 'adapter-invalid', trace: GUEST_RELEASE, region_reads: 0,
        releases: { guest: 1, destructor: 0, host_retirement: 0 }, cleanup_state: 'already-retired' },
      verdicts: [],
      errorIncludes: 'unknown handle index',
    });
  }

  // EPROTO-CASE-10: out-of-order Noble source is refused at check time.
  for (const [label, source] of [
    ['release-then-read', '[ [ regions.release ] dip ] dip region.read\n'],
    ['reordered-words', 'regions.release region.read\n'],
  ]) {
    const refused = compile(context, label, source);
    requireThat(refused.result.status !== 0 && refused.report.outcome === 'error'
      && refused.report.diagnostic?.code === 'component-check' && refused.report.component_emitted === false,
    `${label}: out-of-order source was not refused at check time: ${refused.result.stdout}`);
    requireThat(!lstatExists(refused.directory), `${label}: a refused source produced output`);
    cases.push({
      id: 'EPROTO-CASE-10', variant: label, kind: 'check-time-refusal', status: 'passed',
      source, compile_command: refused.result.record.id, observed: refused.report,
    });
  }

  // EPROTO-CASE-11: results outside the result schema.
  scenario(context, {
    id: 'EPROTO-CASE-11', variant: 'unmodeled-error', kind: 'hostile-component',
    component: hostile['unmodeled-error'], offset: 1, length: 2,
    expected: { outcome: 'timeout', exit_class: 'outside-model', trace: GUEST_RELEASE,
      releases: { guest: 1, destructor: 0, host_retirement: 0 } },
    verdicts: ['outside-model'],
  });
  scenario(context, {
    id: 'EPROTO-CASE-11', variant: 'short-ok', kind: 'hostile-component',
    component: hostile['short-ok'], offset: 1, length: 2,
    expected: { outcome: 'read', exit_class: 'outside-model', bytes_hex: '01',
      trace: [...CLEAN_READ, ...GUEST_RELEASE], releases: { guest: 1, destructor: 0, host_retirement: 0 } },
    verdicts: ['outside-model'],
  });
  cases.push({
    id: 'EPROTO-CASE-11', variant: 'host-produced-unmodeled-response', kind: 'host-assumption',
    status: 'trace-level-only',
    controls: ['monitor-host-short-ok', 'monitor-host-unmodeled-error'],
    observation: 'The audited host answers a read only with ok bytes of the requested length, bounds-reject or invalid-handle (A3), and no modified host is built. The executed variants return unmodeled results from the guest, exercising the adapter\'s outside-model class. The traces a modified host would record, an ok one byte short and err("timeout"), are monitor controls that must be reported as outside-model.',
  });

  monitorControls(success[0].observed);

  const finalState = gitState(git);
  requireThat(stable(finalState) === stable(initialState), 'source commit/tree/status changed during execution');
  requireThat(stable(sourceHashes(git)) === stable(hashes), 'source files changed during execution');
  requireThat(fileHash(cli) === cliSha256, 'the source-built CLI changed during the run');
  const artifacts = artifactHashes();
  const receipt = {
    schema: 'noble-effect-protocol-execution/v1', change: 'behavioral-effect-protocol', issue: 9,
    source_commit: initialState.head, source_tree: initialState.tree, clean_source: cleanSource,
    source_status_before: initialState.status,
    sources_sha256: hashes,
    toolchain: {
      cargo: realpathSync(cargo), wasm_tools: wasmToolsVersion,
      wasmtime: selection.component_sync.wasmtime_version, vendor_nar_hash: vendor.narHash,
      cli_sha256: cliSha256, build_command: build.record.id,
    },
    subject: subjectRecord,
    hostile_components: Object.fromEntries(Object.entries(hostile).map(([label, row]) => [label, row.record])),
    cases,
    controls,
    commands,
    artifacts_sha256: artifacts,
    assumptions: [
      'Every executed scenario is one public `noble component read-region --report v2` invocation of the CLI built from this commit, over the compiled subject or a hostile component assembled here.',
      'The hostile components are core modules written in this script against the selected WIT world\'s synchronous memory32 Canonical ABI, not derived from Noble compiler output.',
      'The monitor replays each recorded trace against the RegionProtocol-v1 transition table. Its verdicts describe these finite executions only; a call the runtime refused before it reached the host leaves no event to judge.',
      'Cancellation is the --cancel-after-reads test knob: the host cancels after answering the requested number of reads, refuses every later call and interrupts the guest through a Wasmtime epoch deadline.',
      'Quota placement uses the --fuel test knob; the boundary found by search is a property of this subject, CLI and Wasmtime build.',
    ],
  };
  const receiptPath = join(out, 'execution.json');
  writeFileSync(receiptPath, json(receipt), { flag: 'wx' });
  console.log(stable({
    result: 'passed', receipt: receiptPath, receipt_sha256: fileHash(receiptPath),
    source_commit: receipt.source_commit, clean_source: cleanSource,
    cases: cases.length, passed: cases.filter(row => row.status === 'passed').length,
    limited: cases.filter(row => row.status !== 'passed').map(row => `${row.id}/${row.variant}:${row.status}`),
    monitor_controls: controls.length, commands: commands.length,
  }));
}

function lstatExists(path) {
  try { lstatSync(path); return true; } catch (error) {
    if (error.code === 'ENOENT') return false;
    throw error;
  }
}

try {
  main();
} catch (error) {
  console.error(`[effect-protocol] FAIL ${error.message}`);
  console.log(stable({ result: 'failed', cases: cases.length, commands: commands.length, failure: error.message }));
  process.exitCode = 1;
}
