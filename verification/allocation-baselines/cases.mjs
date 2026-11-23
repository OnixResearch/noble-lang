#!/usr/bin/env node
// Failure-class gate for ALLOC-CASE-02, ALLOC-CASE-09 and ALLOC-CASE-10
// (r[ALLOC-FAIL-01], box 2 of `.cairn/changes/allocation-strategy-contract`).
//
//   node verification/allocation-baselines/cases.mjs --cli BIN --out NEW_DIR [--allow-dirty]
//
// These cases are NOT registered in specs/conformance; this gate writes a
// source-bound receipt and retains raw observations outside the case records. It
// refuses a dirty tracked tree unless --allow-dirty is given for development.
import { spawn, spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

import { abi, config, freshDirectory, pressure, resolveWasmTools, root, sha256, sourceBinding,
  toolBinding, writeJson } from './core.mjs';
import {
  ComponentDriver, ExperimentDriver, lineSession, slotInteractive, workerDeathProbe,
} from './drivers.mjs';
import { liveFragment, linearFragment, writeFragment } from './fragments.mjs';
import { mixedLifetimeLines } from './workloads.mjs';

function argumentsForRun() {
  const options = { cli: process.env.NOBLE_ALLOC_CLI ?? null, out: null, allowDirty: false };
  for (let index = 2; index < process.argv.length; index += 1) {
    const argument = process.argv[index];
    if (argument === '--allow-dirty') { options.allowDirty = true; continue; }
    if (!['--cli', '--out'].includes(argument)) throw Error(`invalid argument: ${argument}`);
    options[argument.slice(2)] = process.argv[++index];
  }
  if (!options.cli || !options.out) throw Error('usage: cases.mjs --cli BIN --out NEW_DIR');
  return options;
}

const options = argumentsForRun();
const out = freshDirectory(options.out);
const scratch = fs.mkdtempSync(path.join(process.env.TMPDIR ?? os.tmpdir(), 'alloc-cases-'));
const cli = fs.realpathSync(path.resolve(options.cli));
const binding = sourceBinding();
if (!binding.clean && !options.allowDirty) {
  throw Error(`the receipt binds a clean tracked tree; status:\n${binding.clean_status}`);
}
const tools = toolBinding(cli);
const wasmToolsPath = resolveWasmTools();

const failures = [];
const checks = [];
function check(name, condition, detail) {
  const entry = { name, passed: Boolean(condition), detail };
  checks.push(entry);
  if (!condition) failures.push(entry);
  return entry;
}

function save(name, value) {
  const content = typeof value === 'string' ? value : `${JSON.stringify(value, null, 2)}\n`;
  fs.writeFileSync(path.join(out, name), content);
}

function assemble(text, name) {
  const { file, sha256: digest } = writeFragment(scratch, name, text);
  const run = spawnSync(wasmToolsPath, ['parse', file, '-o',
    file.replace(/\.wat$/, '.wasm')], { encoding: 'utf8' });
  if (run.status !== 0) throw Error(`fragment assembly failed: ${run.stderr}`);
  return { file, sha256: digest, bytes: fs.readFileSync(file.replace(/\.wat$/, '.wasm')) };
}

function fragmentInstance(text, name) {
  const fragment = assemble(text, name);
  const module = new WebAssembly.Module(fragment.bytes);
  const exports = new WebAssembly.Instance(module, Object.create(null)).exports;
  return { ...fragment, module, exports };
}

function fill(exports, limit = 5000) {
  let allocated = 0;
  while (allocated < limit) {
    const handle = exports.alloc_one();
    if (handle === 0) break;
    allocated += 1;
  }
  return { allocated, failure: exports.failure(), quota_reason: exports.quota_reason(),
    live: Number(exports.live_count()), heap_cursor: exports.heap_cursor(),
    live_logical_bytes: Number(exports.metric(4)), committed_bytes: Number(exports.metric(10)) };
}

function importCore() {
  return import(path.join(root, 'crates/noble-cli/src/core/runtime/host.mjs'));
}

async function engineRun({ wat, limits = {}, source = 'alloc-case' }) {
  const { CoreEngine } = await importCore();
  const directory = fs.mkdtempSync(path.join(scratch, 'engine-'));
  const engine = new CoreEngine(config, abi, { artifacts: directory });
  try {
    const prepared = engine.prepare(wat, Buffer.from(source), 1);
    if (prepared.outcome !== 'ready') return { prepared };
    const report = engine.execute({ inputs: [], limits });
    return { prepared, report };
  } finally {
    engine.close();
    fs.rmSync(directory, { recursive: true, force: true });
  }
}

function compileExpression(text) {
  const file = path.join(scratch, `case-${sha256(Buffer.from(text)).slice(0, 8)}.noble`);
  fs.writeFileSync(file, text);
  const run = spawnSync(cli, ['compile', file], { encoding: 'buffer', maxBuffer: 64 * 1024 * 1024 });
  if (run.status !== 0) throw Error(`compile refused: ${run.stdout?.toString('utf8').slice(0, 300)}`);
  return run.stdout;
}

function experiment(representation) {
  const watFile = path.join(scratch, `${representation}.wat`);
  const emit = spawnSync(cli, ['wasm-experiment', representation], { encoding: 'buffer',
    maxBuffer: 64 * 1024 * 1024 });
  if (emit.status !== 0) throw Error(`wasm-experiment ${representation} failed`);
  fs.writeFileSync(watFile, emit.stdout);
  return new ExperimentDriver(representation, watFile, path.join(scratch, `${representation}.wasm`));
}

// A session killed at a known point, for ALLOC-CASE-10.
function killedSession(spawnSpec, lines, killAfter) {
  return new Promise((resolve, reject) => {
    const child = spawn(spawnSpec.command, spawnSpec.args, { stdio: ['pipe', 'pipe', 'pipe'],
      env: process.env });
    const reports = [];
    let pending = '';
    let stderr = '';
    child.stdout.on('data', bytes => {
      pending += bytes.toString('utf8');
      for (let end; (end = pending.indexOf('\n')) >= 0;) {
        const line = pending.slice(0, end);
        pending = pending.slice(end + 1);
        if (!line.trim()) continue;
        reports.push(line);
        if (reports.length === killAfter) child.kill('SIGKILL');
      }
    });
    child.stderr.on('data', bytes => { stderr += bytes.toString(); });
    child.once('exit', (code, signal) => resolve({ reports, stderr, code, signal,
      lines_sent: lines }));
    child.once('error', reject);
    for (const line of lines) child.stdin.write(`${line}\n`);
  });
}

async function main() {
  // ALLOC-CASE-02: four distinct classifications.
  const prefixSource = `"prefix" test.emit 20 [ 1 + ] [ 2 * ] [ 1 + ] [ 2 * ] [ 1 + ] [ 2 * ] `
    + `[ 1 + ] [ 2 * ] [ 1 + ] [ 2 * ] [ 1 + ] [ 2 * ] [ 1 + ] [ 2 * ] [ 1 + ] [ 2 * ] `
    + 'compose compose compose compose compose compose compose compose compose compose compose '
    + 'compose compose compose compose run';
  const prefixWat = compileExpression(prefixSource);
  const calibration = await engineRun({ wat: prefixWat });
  const peak = calibration.report?.metrics?.allocation_peak_bytes;
  check('CASE-02/F1/calibration', calibration.report?.outcome === 'normal' && peak > 0,
    { outcome: calibration.report?.outcome, peak });
  const f1 = await engineRun({ wat: prefixWat, limits: { allocation_bytes: peak - 48 } });
  save('case-02-f1.json', { source: prefixSource, peak, report: f1.report });
  check('CASE-02/F1/distinct-class',
    f1.report?.quota_reason === 'allocation' && f1.report?.failure_class === 'F1-logical-quota',
    { quota_reason: f1.report?.quota_reason, failure_class: f1.report?.failure_class });
  check('CASE-02/F1/prefix-retained',
    Array.isArray(f1.report?.request_trace) && f1.report.request_trace.includes('test.emit:prefix'),
    { request_trace: f1.report?.request_trace });
  check('CASE-02/F1/no-partial-cell',
    f1.report?.stack?.length === 0
      && f1.report?.session_state === 'terminated-after-runtime-failure'
      && f1.report?.metrics?.allocated_since_begin_bytes === peak - 48,
    { stack: f1.report?.stack, session_state: f1.report?.session_state,
      peak, metrics: f1.report?.metrics });

  const f2 = fragmentInstance(liveFragment(), 'case-02-f2-live');
  const f2Fill = fill(f2.exports);
  save('case-02-f2.json', { fragment_sha256: f2.sha256, ...f2Fill });
  check('CASE-02/F2/strategy-exhaustion',
    f2Fill.failure !== 0 && f2Fill.quota_reason === 8 && f2Fill.live === 4096,
    f2Fill);
  check('CASE-02/F2/no-partial-publication', f2Fill.heap_cursor === 4096 && f2Fill.live === 4096,
    { heap_cursor: f2Fill.heap_cursor, live: f2Fill.live });

  const f3 = fragmentInstance(linearFragment({ maxPages: 2 }), 'case-02-f3-linear');
  const f3Fill = fill(f3.exports);
  save('case-02-f3.json', { fragment_sha256: f3.sha256, ...f3Fill });
  check('CASE-02/F3/memory-growth',
    f3Fill.failure !== 0 && f3Fill.quota_reason === 9 && f3Fill.live < 4096,
    f3Fill);
  check('CASE-02/F3/not-logical-limit', f3Fill.live_logical_bytes < 196608, f3Fill);

  const f4 = workerDeathProbe(64);
  save('case-02-f4.json', f4);
  check('CASE-02/F4/process-death', f4.signal !== null || f4.exit !== 0,
    { signal: f4.signal, exit: f4.exit, stderr_tail: f4.stderr.slice(-300) });
  const observedClasses = {
    F1: f1.report?.failure_class ?? (f1.report?.quota_reason ? `unsplit:${f1.report.quota_reason}` : null),
    F2: f2Fill.failure === 0 ? null
      : f2Fill.quota_reason === 8 ? 'F2-strategy-exhaustion'
      : `unsplit:quota_reason_${f2Fill.quota_reason}`,
    F3: f3Fill.failure === 0 ? null
      : f3Fill.quota_reason === 9 ? 'F3-memory-growth'
      : `unsplit:quota_reason_${f3Fill.quota_reason}`,
    F4: (f4.signal !== null || f4.exit !== 0) ? 'F4-engine-oom' : null,
  };
  const expectedClasses = [
    'F1-logical-quota',
    'F2-strategy-exhaustion',
    'F3-memory-growth',
    'F4-engine-oom',
  ];
  const actualClasses = [
    observedClasses.F1,
    observedClasses.F2,
    observedClasses.F3,
    observedClasses.F4,
  ];
  check('CASE-02/four-distinct-classes',
    expectedClasses.every((expected, index) => actualClasses[index] === expected)
      && new Set(actualClasses).size === 4,
    { observed: observedClasses, expected: expectedClasses });

  // CASE-09: traps mid-construction and at reservation publish no partial cell.
  for (const representation of ['managed-linear-memory', 'wasm-gc']) {
    const driver = experiment(representation);
    const mid = driver.root({ build: { kind: 'tree', shape: 0, leaves: 256 },
      limits: { allocation_bytes: 512 } });
    save(`case-09-${representation}.json`, mid);
    check(`CASE-09/${representation}/trap-mid-construction`,
      mid.status !== 0 && mid.phase === 'construction',
      { status: mid.status, phase: mid.phase, reason: mid.snapshots[mid.phase]?.quota_reason });
    check(`CASE-09/${representation}/no-live-partial-cell`,
      mid.snapshots[mid.phase]?.live_logical_bytes === 0
        && mid.cleanup.metrics.live_logical_bytes === 0,
      { failure: mid.snapshots[mid.phase], cleanup: mid.cleanup });
    check(`CASE-09/${representation}/cleanup-accounted`,
      mid.cleanup.metrics.allocated_total_bytes === mid.cleanup.metrics.released_total_bytes,
      mid.cleanup.metrics);
    check(`CASE-09/${representation}/engine-error-none`, mid.engine_error == null,
      mid.engine_error);
  }
  const component = new ComponentDriver(path.join(root, 'crates/noble-wasm/src/component/runtime.wat'),
    path.join(scratch, 'case-09-a7.wasm'));
  const a7Trap = component.quota(64, 72);
  save('case-09-a7.json', a7Trap);
  check('CASE-09/A7/reservation-trap',
    a7Trap.outcome === 'trap' && a7Trap.metrics.quota_exceeded === 1
      && a7Trap.metrics.live_bytes === 0,
    a7Trap);

  // CASE-10: kill a mixed-lifetime session and prove no stale owner is reused.
  const killed = await killedSession({ command: cli, args: ['session'] },
    mixedLifetimeLines(200), 20);
  save('case-10-killed-session.json', { reports: killed.reports.length, code: killed.code,
    signal: killed.signal, stderr_tail: killed.stderr.slice(-400) });
  check('CASE-10/session-killed', killed.signal === 'SIGKILL', { signal: killed.signal });
  const restarted = await lineSession({ command: cli, args: ['session'] }, ['41 1 +'],
    { timeoutMs: 120000 });
  save('case-10-restarted.json', { reports: restarted.reports });
  const restartedReport = JSON.parse(restarted.reports.at(-1) ?? '{}');
  check('CASE-10/new-session-normal', restartedReport.outcome === 'normal',
    { outcome: restartedReport.outcome });

  const slotSource = '40 [ 1 + ]';
  const runnerSource = '20 swap run';
  const directory = fs.mkdtempSync(path.join(scratch, 'case-10-slot-'));
  const authority = path.join(directory, 'authority.json');
  fs.writeFileSync(authority, JSON.stringify({ owner: 'alloc-case-host', quota: 4,
    effects: [], resources: [], slots: [], grants: [],
    sources: [
      { id: 'quoted', sha256: sha256(Buffer.from(slotSource)) },
      { id: 'runner', sha256: sha256(Buffer.from(runnerSource)) },
    ] }));
  let owner = null;
  const first = await slotInteractive(cli, authority, async live => {
    await live.issue({ operation: 'install', id: 'quoted', source: slotSource, inputs: ['I64'] });
    const row = await live.issue({ operation: 'invoke', id: 'quoted',
      inputs: [{ kind: 'i64', value: '5' }], refs: [] });
    owner = row.stack?.at(-1)?.owner ?? null;
  }, { timeoutMs: 120000 });
  check('CASE-10/slot-owner-observed', first.exit.code === 0 && owner !== null,
    { exit: first.exit, owner });
  const second = await slotInteractive(cli, authority, async live => {
    await live.issue({ operation: 'install', id: 'runner', source: runnerSource,
      inputs: ['Program<I64,I64,pure>'] });
    const invoke = await live.issue({ operation: 'invoke', id: 'runner',
      inputs: [{ kind: 'program', owner }], refs: [] });
    save('case-10-stale-owner.json', invoke);
    check('CASE-10/stale-owner-refused',
      invoke.outcome === 'refused'
        && invoke.diagnostic === 'root Program differs from checked ordered type',
      { outcome: invoke.outcome, diagnostic: invoke.diagnostic });
    const release = await live.issue({ operation: 'release-program', owner });
    save('case-10-stale-release.json', release);
    check('CASE-10/stale-release-refused',
      release.outcome === 'refused' && release.diagnostic === 'unknown saved Program owner',
      { outcome: release.outcome, diagnostic: release.diagnostic });
  }, { timeoutMs: 120000 });
  fs.rmSync(directory, { recursive: true, force: true });

  const receipt = {
    schema: 'noble-allocation-cases/v1',
    change: '.cairn/changes/allocation-strategy-contract',
    cases: ['ALLOC-CASE-02', 'ALLOC-CASE-09', 'ALLOC-CASE-10'],
    requirements: ['ALLOC-FAIL-01'],
    source: binding,
    source_rebound: { ...sourceBinding(), cli_unchanged:
      sha256(fs.readFileSync(cli)) === tools.noble.sha256 },
    tools,
    controls: { pinned_node: config.tools.node, pinned_wasm_tools: config.tools.wasm_tools,
      accounting_revision: 'F1-F4 split: reason 1 logical quota, 8 strategy exhaustion, '
        + '9 memory growth, F4 host/engine classification',
      addresses: { live_count: 57344, live_free_head: 57348 } },
    fragments: {
      linear: linearFragment({ maxPages: 2 }),
      live: liveFragment(),
    },
    checks,
    failures,
    registration: {
      registered: false,
      note: 'ALLOC-CASE-02/09/10 are designs in the change, not entries in specs/conformance. '
        + 'Registering them needs a case record (id, profile, input, expected, state: absent, '
        + 'evidence: []) and a gate that reproduces this receipt from a clean commit.',
    },
    pressure: pressure(),
  };
  writeJson(path.join(out, 'receipt.json'), receipt);
  const raw = checks.map(entry => JSON.stringify(entry)).join('\n');
  fs.writeFileSync(path.join(out, 'checks.jsonl'), `${raw}\n`);
  process.stdout.write(`ALLOC-CASES ${path.join(out, 'receipt.json')} `
    + `${checks.filter(entry => entry.passed).length}/${checks.length}\n`);
  if (failures.length) process.exitCode = 1;
}

try {
  await main();
} catch (error) {
  console.error(`ALLOC-CASES failed: ${error.stack ?? error}`);
  process.exitCode = 1;
} finally {
  fs.rmSync(scratch, { recursive: true, force: true });
}
