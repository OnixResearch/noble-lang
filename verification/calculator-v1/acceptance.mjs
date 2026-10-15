#!/usr/bin/env node
// Calculator-v1 CLI acceptance runner: executes canonical CALC vectors against
// an exact prebuilt `noble` binary, one process per vector (one per session),
// and keeps raw I/O plus a JSON receipt in a fresh directory outside the
// repository. Results are implementation observations only; this runner never
// edits case state or evidence.
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawn, spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { parseArgs } from 'node:util';
import { RUNTIME_CASES, generate, transcript, validate } from './vectors.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = fs.realpathSync(path.resolve(here, '../..'));
const CASES = 'specs/conformance/calculator-cases.json';
const MAPPING = 'verification/calculator-v1/cli-mapping.json';
const RUNNER_FILES = ['acceptance.mjs', 'supervise.mjs', 'vectors.mjs', 'cli-mapping.json']
  .map(name => `verification/calculator-v1/${name}`);
// Sources whose include_bytes!/include_str! dependencies define what the
// calculator binary embeds (guest fixtures and the Wasm worker host).
const EMBEDDING_SOURCES = ['crates/noble-cli/src/calculator.rs', 'crates/noble-cli/src/core/worker.rs'];
const CLASSES = ['pass', 'fail', 'timeout', 'native-failure', 'host-limit'];
const DIAGNOSTIC = /^(.+?) \[(\d+),(\d+)\) provenance=(\S+)$/;
const NATIVE_PREFIXES = ['NativeFailure:', 'InfrastructureFailure:', 'noble calculator:'];
const HOST_PREFIXES = ['HostLimit ', 'TransportLimit '];
const USAGE = `usage: node verification/calculator-v1/acceptance.mjs \\
  --binary PREBUILT_NOBLE --binary-sha256 HEX --source-id TEXT --build-profile TEXT \\
  --out FRESH_EXTERNAL_DIR [--case CALC-01,...] [--vector ID]... [--vectors FILE] \\
  [--concurrency N] [--vector-timeout S] [--session-timeout S] [--memory-max SIZE] \\
  [--calculator-arg ARG]... [--sample-ms MS] [--no-controls | --controls-only] \\
  [--control-base ID] [--control-timeout S]
       node verification/calculator-v1/acceptance.mjs --emit-vectors NEW_FILE`;

const { values: options } = parseArgs({
  options: {
    binary: { type: 'string' }, 'binary-sha256': { type: 'string' },
    'source-id': { type: 'string' }, 'build-profile': { type: 'string' },
    out: { type: 'string' }, case: { type: 'string', multiple: true },
    vector: { type: 'string', multiple: true }, vectors: { type: 'string' },
    'emit-vectors': { type: 'string' },
    concurrency: { type: 'string', default: '2' },
    'vector-timeout': { type: 'string', default: '7200' },
    'session-timeout': { type: 'string', default: '21600' },
    'memory-max': { type: 'string' }, 'calculator-arg': { type: 'string', multiple: true },
    'sample-ms': { type: 'string', default: '1000' },
    'no-controls': { type: 'boolean', default: false },
    'controls-only': { type: 'boolean', default: false },
    'control-base': { type: 'string', default: 'CALC-02/02' },
    'control-timeout': { type: 'string', default: '1' },
    help: { type: 'boolean', default: false },
  },
  strict: true,
});
if (options.help) {
  console.log(USAGE);
  process.exit(0);
}

const digest = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const hashFile = file => digest(fs.readFileSync(file));
const readJson = file => JSON.parse(fs.readFileSync(file, 'utf8'));
const relative = file => path.relative(root, file);
const casesFile = path.join(root, CASES);
const mappingFile = path.join(root, MAPPING);
const generated = generate(readJson(casesFile), readJson(mappingFile));

if (options['emit-vectors'] !== undefined) {
  const target = path.resolve(options['emit-vectors']);
  fs.writeFileSync(target, `${JSON.stringify({ ...generated,
    generated_from: { cases: { path: CASES, sha256: hashFile(casesFile) },
      mapping: { path: MAPPING, sha256: hashFile(mappingFile) } } }, null, 2)}\n`, { flag: 'wx' });
  console.log(`wrote ${generated.vectors.length} vectors and ${generated.unsupported.length} unsupported variants to ${target}`);
  process.exit(0);
}

const positive = (name, text) => {
  const number = Number(text);
  assert.ok(Number.isFinite(number) && number > 0, `--${name} must be positive`);
  return number;
};
for (const name of ['binary', 'binary-sha256', 'source-id', 'build-profile', 'out']) {
  assert.ok(options[name] !== undefined, `missing --${name}\n${USAGE}`);
}
assert.match(options['binary-sha256'], /^[0-9a-f]{64}$/, 'owner must supply the binary SHA-256');
assert.ok(options['source-id'].length > 0 && options['source-id'].length <= 256,
  'owner must supply the binary build-source identity');
assert.ok(options['build-profile'].length > 0 && options['build-profile'].length <= 64,
  'owner must supply the binary build profile');
assert.ok(!(options['no-controls'] && options['controls-only']), '--no-controls excludes --controls-only');
const concurrency = positive('concurrency', options.concurrency);
assert.ok(Number.isInteger(concurrency), '--concurrency must be an integer');
const vectorTimeoutMs = positive('vector-timeout', options['vector-timeout']) * 1000;
const sessionTimeoutMs = positive('session-timeout', options['session-timeout']) * 1000;
const controlTimeoutMs = positive('control-timeout', options['control-timeout']) * 1000;
const sampleMs = positive('sample-ms', options['sample-ms']);
const memoryMax = options['memory-max'] ?? null;
if (memoryMax !== null) assert.match(memoryMax, /^\d+[KMGT]?$/, '--memory-max takes a systemd size such as 6G');
const calculatorArgs = options['calculator-arg'] ?? [];

const binary = path.resolve(options.binary);
const output = path.resolve(options.out);
assert.ok(output !== root && !output.startsWith(`${root}${path.sep}`),
  'receipt/artifacts must stay outside the repository');
assert.ok(!fs.existsSync(output), 'output directory must be fresh');
assert.equal(fs.realpathSync(path.dirname(output)), path.dirname(output),
  'output parent must not be a symlink');
const binaryBytes = fs.readFileSync(binary);
const binarySha = digest(binaryBytes);
assert.equal(binarySha, options['binary-sha256'], 'binary SHA-256 differs from the owner-supplied hash');

// Selection: generated vectors (or a custom vector file), then cases, then ids.
const source = options.vectors === undefined
  ? generated
  : { ...validate(readJson(options.vectors)), unsupported: [] };
const caseFilter = (options.case ?? []).flatMap(text => text.split(',')).filter(Boolean);
const idFilter = (options.vector ?? []).flatMap(text => text.split(',')).filter(Boolean);
for (const id of caseFilter) {
  assert.ok(source.vectors.some(vector => vector.case === id) || source.unsupported.some(item => item.case === id),
    `--case ${id} selects nothing`);
}
for (const id of idFilter) {
  assert.ok(source.vectors.some(vector => vector.id === id) || source.unsupported.some(item => item.id === id),
    `--vector ${id} selects nothing`);
}
const chosen = item => (caseFilter.length === 0 || caseFilter.includes(item.case))
  && (idFilter.length === 0 || idFilter.includes(item.id));
const selected = options['controls-only'] ? [] : source.vectors.filter(chosen);
const unsupported = options['controls-only'] ? [] : source.unsupported.filter(chosen);

// Negative controls reuse an ordinary vector's exact argv/stdin and must not pass.
const controls = [];
if (!options['no-controls']) {
  const base = generated.vectors.find(vector => vector.id === options['control-base']);
  assert.ok(base, `--control-base ${options['control-base']} is not a generated vector`);
  const wrong = structuredClone(base);
  const last = wrong.operations.at(-1).expect;
  if (last.value !== undefined) {
    last.value = `${last.value}0`;
  } else if (last.diagnostic !== undefined) {
    last.diagnostic = { category: last.diagnostic.category === 'SyntaxError' ? 'UnknownName' : 'SyntaxError',
      span: last.diagnostic.span };
  } else {
    wrong.operations.at(-1).expect = { value: '0' };
  }
  controls.push({ ...wrong, id: `control/wrong-expected/${base.id}`, control: {
    base: base.id, mutation: 'last operation expectation replaced by a deliberately wrong value',
    expected_classification: 'fail' } });
  controls.push({ ...structuredClone(base), id: `control/forced-timeout/${base.id}`, timeout_ms: controlTimeoutMs,
    control: { base: base.id, mutation: `timeout forced to ${controlTimeoutMs} ms`,
      expected_classification: 'timeout' } });
}
assert.ok(selected.length + controls.length > 0, 'nothing selected');

const git = args => {
  const run = spawnSync('git', ['-C', root, ...args], { encoding: 'utf8' });
  return run.status === 0 ? run.stdout.trim() : null;
};
const embedded = () => {
  const files = new Set(EMBEDDING_SOURCES);
  for (const file of EMBEDDING_SOURCES) {
    const text = fs.readFileSync(path.join(root, file), 'utf8');
    for (const match of text.matchAll(/include_(?:bytes|str)!\(\s*"([^"]+)"\s*\)/g)) {
      files.add(relative(path.resolve(root, path.dirname(file), match[1])));
    }
  }
  const fixtures = 'crates/noble-contracts/src/fixtures';
  for (const name of fs.readdirSync(path.join(root, fixtures))) {
    if (/^calc.*\.noble$/.test(name)) files.add(`${fixtures}/${name}`);
  }
  return [...files].sort().map(file => ({ path: file, sha256: hashFile(path.join(root, file)) }));
};
const workerNode = () => {
  const match = /\/nix\/store\/[0-9a-z]{32}-nodejs-[^/\0]+\/bin\/node/.exec(binaryBytes.toString('latin1'));
  if (!match) return { path: null, version: null };
  const run = spawnSync(match[0], ['--version'], { encoding: 'utf8', env: {} });
  return { path: match[0], version: run.status === 0 ? run.stdout.trim() : null,
    error: run.error ? String(run.error) : null };
};
const assembler = () => {
  const argv = [binary, '--internal-calculator-wat-version'];
  const run = spawnSync(argv[0], argv.slice(1), { encoding: 'utf8', env: { PATH: '/nonexistent' } });
  return { argv, status: run.status, stdout: run.stdout.trim() };
};
const systemdRun = () => {
  const run = spawnSync('sh', ['-c', 'command -v systemd-run'], { encoding: 'utf8' });
  assert.equal(run.status, 0, '--memory-max requires systemd-run --user');
  return fs.realpathSync(run.stdout.trim());
};

fs.mkdirSync(output);
const markers = { done: path.join(output, 'ALL_DONE'), failed: path.join(output, 'FAILED') };
const childEnv = tmp => ({ PATH: '/nonexistent', LANG: 'C', LC_ALL: 'C', RUST_BACKTRACE: '1', TMPDIR: tmp });
const wrapper = memoryMax === null ? [] : [systemdRun(), '--user', '--scope', '--quiet', '--collect',
  '-p', `MemoryMax=${memoryMax}`, '-p', 'MemorySwapMax=0', '-p', 'OOMPolicy=continue', '--'];
const wrapperEnv = memoryMax === null ? {} : Object.fromEntries(['XDG_RUNTIME_DIR', 'DBUS_SESSION_BUS_ADDRESS']
  .filter(key => process.env[key] !== undefined).map(key => [key, process.env[key]]));
const receipt = {
  schema: 'noble-calculator-v1-cli-observation/v1', result: 'running',
  mode: 'prebuilt-binary-cli-vector-observation',
  claim: 'finite-prebuilt-calculator-cli-observation-only',
  non_claims: [
    'canonical CALC case promotion or any state/evidence change',
    'Calculator-v1 acceptance',
    'source-bound release evidence: the binary hash is checked, its build source is owner-supplied',
    'CALC-04, CALC-05, CALC-08 and CALC-10 through CALC-13',
    'independent numeric oracle: expectations are the cases file values',
    'parser correctness or a universal arithmetic property from finite vectors',
    'a timeout, host limit or native failure as a ResourceLimit or as a pass',
    'performance targets',
  ],
  run: { started: new Date().toISOString(), finished: null, host: os.hostname(),
    kernel: os.release(), cpus: os.cpus().length, memory_bytes: os.totalmem(), argv: process.argv },
  source: {
    repository: root, commit: git(['rev-parse', 'HEAD']),
    tracked_changes: git(['status', '--porcelain=v1', '--untracked-files=no']),
    cases: { path: CASES, sha256: hashFile(casesFile), revision: readJson(casesFile).revision },
    mapping: { path: MAPPING, sha256: hashFile(mappingFile) },
    runner: RUNNER_FILES.map(file => ({ path: file, sha256: hashFile(path.join(root, file)) })),
    embedded: embedded(),
    vector_file: options.vectors === undefined ? null
      : { path: path.resolve(options.vectors), sha256: hashFile(options.vectors) },
  },
  binary: { path: binary, realpath: fs.realpathSync(binary), sha256: binarySha, bytes: binaryBytes.length,
    build_profile: options['build-profile'], source_identity: options['source-id'] },
  toolchain: { runner_node: { path: process.execPath, version: process.version },
    worker_node: workerNode(), assembler: assembler() },
  config: {
    concurrency, vector_timeout_ms: vectorTimeoutMs, session_timeout_ms: sessionTimeoutMs,
    memory_max: memoryMax, memory_wrapper: wrapper, sample_ms: sampleMs, calculator_args: calculatorArgs,
    child_env: childEnv('<OUTPUT>/tmp/<vector>'), cwd: root,
    selection: { cases: caseFilter, vectors: idFilter, controls: !options['no-controls'],
      controls_only: options['controls-only'] },
  },
  vectors: [], unsupported, controls: [], summary: null, error: null,
};
const receiptFile = path.join(output, 'receipt.json');
const save = (file = path.join(output, 'receipt.partial.json')) =>
  fs.writeFileSync(file, `${JSON.stringify(receipt, null, 2)}\n`);
save();

const slug = id => id.replace(/[^A-Za-z0-9._-]+/g, '_');
const retained = (file, bytes) => {
  const text = bytes.length <= 65_536 ? bytes.toString('utf8') : null;
  return { file: path.relative(output, file), sha256: digest(bytes), bytes: bytes.length, text };
};
const lines = bytes => {
  const text = bytes.toString('utf8');
  if (text === '') return [];
  const parts = text.split('\n');
  if (parts.at(-1) === '') parts.pop();
  return parts;
};

const classify = (vector, observed, stdoutLines, stderrLines) => {
  const reasons = [];
  if (observed.supervisor_lost) {
    return observed.guard_fired
      ? { classification: 'timeout', reasons: [`supervisor killed by outer guard after ${observed.timeout_ms} ms + 300 s`] }
      : { classification: 'host-limit', reasons: [`supervisor lost under memory cap: ${JSON.stringify(observed.supervisor)}`] };
  }
  if (observed.launch_error !== null) return { classification: 'native-failure', reasons: [`launch: ${observed.launch_error}`] };
  if (observed.timed_out) return { classification: 'timeout', reasons: [`killed after ${observed.timeout_ms} ms`] };
  const events = observed.memory.cgroup?.memory_events ?? {};
  if ((events.oom_kill ?? 0) > 0 || (events.oom_group_kill ?? 0) > 0) {
    return { classification: 'host-limit', reasons: [`memory cap OOM kill: ${JSON.stringify(events)}`] };
  }
  if (observed.signal !== null) return { classification: 'native-failure', reasons: [`signal ${observed.signal}`] };
  const native = stderrLines.filter(line => NATIVE_PREFIXES.some(prefix => line.startsWith(prefix)));
  if (![0, 1].includes(observed.status) || native.length > 0) {
    return { classification: 'native-failure', reasons: [`exit ${observed.status}`, ...native] };
  }
  const host = stderrLines.filter(line => HOST_PREFIXES.some(prefix => line.startsWith(prefix)));
  if (host.length > 0) return { classification: 'host-limit', reasons: host };
  const expected = transcript(vector);
  if (observed.status !== expected.status) reasons.push(`exit ${observed.status}, expected ${expected.status}`);
  if (JSON.stringify(stdoutLines) !== JSON.stringify(expected.stdout)) {
    reasons.push(`stdout ${JSON.stringify(stdoutLines)}, expected ${JSON.stringify(expected.stdout)}`);
  }
  if (stderrLines.length !== expected.stderr.length) {
    reasons.push(`${stderrLines.length} diagnostic lines, expected ${expected.stderr.length}`);
  }
  expected.stderr.forEach((want, index) => {
    const line = stderrLines[index];
    const match = line === undefined ? null : DIAGNOSTIC.exec(line);
    if (match === null) {
      if (line !== undefined) reasons.push(`diagnostic ${index}: unparsed ${JSON.stringify(line)}`);
      return;
    }
    if (match[1] !== want.category) reasons.push(`diagnostic ${index}: ${match[1]}, expected ${want.category}`);
    if (match[4] !== 'unavailable') reasons.push(`diagnostic ${index}: provenance=${match[4]}`);
    if (want.span !== null && (Number(match[2]) !== want.span[0] || Number(match[3]) !== want.span[1])) {
      reasons.push(`diagnostic ${index}: span [${match[2]},${match[3]}), expected [${want.span[0]},${want.span[1]})`);
    }
  });
  return { classification: reasons.length === 0 ? 'pass' : 'fail', reasons };
};

// Positional attribution of the two streams back to operations, for review only.
const attribute = (vector, stdoutLines, stderrLines) => {
  let out = 0;
  let err = 0;
  return vector.operations.map(operation => {
    let observed = 'silent';
    if (operation.expect.value !== undefined) observed = stdoutLines[out++] ?? null;
    if (operation.expect.diagnostic !== undefined) observed = stderrLines[err++] ?? null;
    return { line: operation.line, expect: operation.expect, observed_positional: observed };
  });
};

const execute = async vector => {
  const name = slug(vector.id);
  const directory = path.join(output, 'vectors', name);
  const tmp = path.join(output, 'tmp', name);
  fs.mkdirSync(directory, { recursive: true });
  fs.mkdirSync(tmp, { recursive: true });
  const stdinBytes = Buffer.from(vector.operations.map(operation => `${operation.line}\n`).join(''));
  const stdinFile = path.join(directory, 'stdin');
  fs.writeFileSync(stdinFile, stdinBytes, { flag: 'wx', mode: 0o400 });
  const argv = [binary, 'calculator', ...vector.flags, ...calculatorArgs];
  const timeoutMs = vector.timeout_ms ?? (vector.kind === 'session' ? sessionTimeoutMs : vectorTimeoutMs);
  const spec = {
    argv, cwd: root, env: childEnv(tmp), stdin_file: stdinFile, timeout_ms: timeoutMs, sample_ms: sampleMs,
    stdout_file: path.join(directory, 'stdout'), stderr_file: path.join(directory, 'stderr'),
    result_file: path.join(directory, 'supervisor.json'), cgroup: memoryMax !== null,
  };
  const specFile = path.join(directory, 'supervisor-spec.json');
  fs.writeFileSync(specFile, `${JSON.stringify(spec, null, 2)}\n`, { flag: 'wx' });
  const supervisorArgv = [...wrapper, process.execPath, path.join(here, 'supervise.mjs'), specFile];
  const startedAt = new Date().toISOString();
  let guardFired = false;
  const supervisor = await new Promise(resolve => {
    const child = spawn(supervisorArgv[0], supervisorArgv.slice(1), {
      env: wrapperEnv, stdio: ['ignore', 'ignore', fs.openSync(path.join(directory, 'supervisor.stderr'), 'wx')],
      detached: true,
    });
    const guard = setTimeout(() => {
      guardFired = true;
      try { process.kill(-child.pid, 'SIGKILL'); } catch { /* gone */ }
    }, timeoutMs + 300_000);
    child.on('error', error => { clearTimeout(guard); resolve({ status: null, error: String(error) }); });
    child.on('exit', (status, signal) => { clearTimeout(guard); resolve({ status, signal, error: null }); });
  });
  // A supervisor can only vanish if the outer guard or the memory-capped
  // scope killed it; anything else is a runner defect and stops the run.
  const lost = !fs.existsSync(spec.result_file);
  if (lost && !guardFired && memoryMax === null) {
    throw new Error(`${vector.id}: supervisor produced no result (${JSON.stringify(supervisor)}); see ${directory}`);
  }
  const observed = lost
    ? { supervisor_lost: true, guard_fired: guardFired, supervisor, status: null, signal: null,
      timed_out: guardFired, launch_error: null, stdin_error: null, wall_ms: null,
      memory: null, group_survivors_killed: null, timeout_ms: timeoutMs }
    : { ...readJson(spec.result_file), supervisor_lost: false, timeout_ms: timeoutMs };
  const settled = file => (fs.existsSync(file) ? fs.readFileSync(file) : Buffer.alloc(0));
  const stdoutBytes = settled(spec.stdout_file);
  const stderrBytes = settled(spec.stderr_file);
  for (const file of [spec.stdout_file, spec.stderr_file, spec.result_file]) {
    if (fs.existsSync(file)) fs.chmodSync(file, 0o400);
  }
  const stdoutLines = lines(stdoutBytes);
  const stderrLines = lines(stderrBytes);
  const verdict = classify(vector, observed, stdoutLines, stderrLines);
  assert.ok(CLASSES.includes(verdict.classification));
  return {
    id: vector.id, case: vector.case, kind: vector.kind, flags: vector.flags,
    argv, supervisor_argv: supervisorArgv, cwd: root, env: spec.env, timeout_ms: timeoutMs,
    stdin: retained(stdinFile, stdinBytes), expected: transcript(vector),
    observed: {
      started_at: startedAt, status: observed.status, signal: observed.signal, timed_out: observed.timed_out,
      launch_error: observed.launch_error, stdin_error: observed.stdin_error, wall_ms: observed.wall_ms,
      stdout: retained(spec.stdout_file, stdoutBytes), stderr: retained(spec.stderr_file, stderrBytes),
      memory: observed.memory, group_survivors_killed: observed.group_survivors_killed,
      supervisor_lost: observed.supervisor_lost, supervisor_exit: supervisor,
    },
    operations: attribute(vector, stdoutLines, stderrLines),
    classification: verdict.classification, reasons: verdict.reasons,
    ...(vector.control === undefined ? {} : { control: vector.control }),
  };
};

const queue = [...controls, ...selected].sort((a, b) => (b.kind === 'session') - (a.kind === 'session'));
const pool = async () => {
  while (queue.length > 0) {
    const vector = queue.shift();
    const row = await execute(vector);
    if (row.control !== undefined) {
      receipt.controls.push({ ...row, control_ok: row.classification === row.control.expected_classification });
    } else {
      receipt.vectors.push(row);
    }
    fs.appendFileSync(path.join(output, 'progress.jsonl'), `${JSON.stringify({
      at: new Date().toISOString(), id: row.id, classification: row.classification,
      wall_ms: row.observed.wall_ms, reasons: row.reasons })}\n`);
    const wall = row.observed.wall_ms === null ? '?' : (row.observed.wall_ms / 1000).toFixed(1);
    console.log(`${row.classification.padEnd(14)} ${wall.padStart(9)} s  ${row.id}`);
    save();
  }
};

try {
  await Promise.all(Array.from({ length: Math.min(concurrency, queue.length) }, pool));
  const order = new Map([...controls, ...selected].map((vector, index) => [vector.id, index]));
  receipt.vectors.sort((a, b) => order.get(a.id) - order.get(b.id));
  receipt.controls.sort((a, b) => order.get(a.id) - order.get(b.id));
  const count = rows => Object.fromEntries(CLASSES.map(name => [name,
    rows.filter(row => row.classification === name).length]));
  receipt.summary = {
    selected: receipt.vectors.length, by_class: count(receipt.vectors),
    by_case: Object.fromEntries([...new Set(receipt.vectors.map(row => row.case))]
      .map(id => [id, count(receipt.vectors.filter(row => row.case === id))])),
    unsupported: unsupported.map(item => item.id),
    all_selected_pass: receipt.vectors.every(row => row.classification === 'pass'),
    controls: receipt.controls.length,
    controls_ok: receipt.controls.every(row => row.control_ok),
  };
  receipt.result = 'completed';
} catch (error) {
  receipt.result = 'error';
  receipt.error = String(error?.stack ?? error);
} finally {
  receipt.run.finished = new Date().toISOString();
  fs.writeFileSync(receiptFile, `${JSON.stringify(receipt, null, 2)}\n`, { flag: 'wx', mode: 0o400 });
  fs.rmSync(path.join(output, 'receipt.partial.json'), { force: true });
  fs.writeFileSync(receipt.result === 'completed' ? markers.done : markers.failed, `${receipt.run.finished}\n`);
}
console.log(JSON.stringify({ result: receipt.result, receipt: receiptFile, summary: receipt.summary,
  error: receipt.error }, null, 2));
process.exitCode = receipt.result === 'completed' && receipt.summary.controls_ok ? 0 : 1;
