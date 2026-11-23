// Strategy drivers. Each driver runs the repository's own host for one strategy
// (A1 ordinary source, A3 in-process live, A4 live slots, A5/A6 M3 experiment,
// A7 component ABI) and returns per-sample observations with the guest's own
// accounting metrics. A2 Calculator-v1 is driven through its terminal session.
import { spawn, spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

import { abi, config, decodeMetrics, runtime, sha256, timed } from './core.mjs';

const hostModule = path.join(runtime, 'host.mjs');

let CoreEngine = null;
async function engineClass() {
  if (CoreEngine === null) ({ CoreEngine } = await import(hostModule));
  return CoreEngine;
}

const assemble = (watFile, wasmFile) => {
  const tools = process.env.NOBLE_WASM_TOOLS
    ?? path.join(path.dirname(fs.realpathSync(process.execPath)), 'wasm-tools');
  const run = spawnSync(tools, ['parse', watFile, '-o', wasmFile], { encoding: 'utf8' });
  if (run.status !== 0) throw Error(`wasm-tools parse failed: ${run.stderr}`);
  // The bytes are the subject: record their digest, not the tool's timing.
  return { bytes: fs.readFileSync(wasmFile), tool: tools };
};

export function compileSource(cli, source, { file = null, flags = [] } = {}) {
  const sourceFile = file ?? path.join(os.tmpdir(), `alloc-baseline-${process.pid}-${Date.now()}.noble`);
  fs.writeFileSync(sourceFile, source);
  try {
    const run = spawnSync(cli, ['compile', sourceFile, ...flags], { encoding: null, maxBuffer: 64 * 1024 * 1024 });
    if (run.status !== 0) {
      const report = run.stdout?.toString('utf8').trim();
      throw Error(`compile refused (${run.status}): ${report?.slice(0, 400)}`);
    }
    return run.stdout;
  } finally {
    if (file === null) fs.rmSync(sourceFile, { force: true });
  }
}

// A1 ordinary source: a real CoreEngine session over compiler-emitted WAT. Compilation
// is held constant outside the timed region; the timed unit is guest execution.
export class OrdinaryDriver {
  constructor(cli, scratch) {
    this.cli = cli;
    this.scratch = scratch;
    this.name = 'A1';
  }

  async session(wats) {
    const Engine = await engineClass();
    const directory = fs.mkdtempSync(path.join(this.scratch, 'a1-'));
    const engine = new Engine(config, abi, { artifacts: directory });
    const records = [];
    try {
      let submission = 0;
      for (const wat of wats) {
        submission += 1;
        const prepared = timed(() => engine.prepare(wat, Buffer.alloc(0), submission));
        if (prepared.value.outcome !== 'ready') {
          records.push({ phase: 'prepare', ready: prepared.value, prepare_ns: prepared.ns });
          continue;
        }
        const executed = timed(() => engine.execute({ inputs: [] }));
        records.push({ phase: 'execute', prepare_ns: prepared.ns, execute_ns: executed.ns,
          report: executed.value, metrics: executed.value.metrics ?? null });
        if (executed.value.outcome !== 'normal') break;
      }
      return { records };
    } finally {
      engine.close();
      fs.rmSync(directory, { recursive: true, force: true });
    }
  }

  // N identical short roots in one persistent session, as W2 declares.
  async repeatedRoots(wat, count) {
    const Engine = await engineClass();
    const directory = fs.mkdtempSync(path.join(this.scratch, 'a1-'));
    const engine = new Engine(config, abi, { artifacts: directory });
    const latencies = [];
    const metrics = [];
    const outcomes = new Set();
    try {
      let prepared = engine.prepare(wat, Buffer.alloc(0), 1);
      if (prepared.outcome !== 'ready') throw Error(`root not ready: ${prepared.outcome}`);
      for (let index = 0; index < count; index += 1) {
        const executed = timed(() => engine.execute({ inputs: [] }));
        latencies.push(executed.ns);
        outcomes.add(executed.value.outcome);
        metrics.push(executed.value.metrics);
        if (executed.value.outcome !== 'normal') break;
        // Each root after the first is a new submission of identical checked bytes.
        if (index + 1 < count) {
          prepared = engine.prepare(wat, Buffer.alloc(0), index + 2);
          if (prepared.outcome !== 'ready') throw Error(`root ${index + 2} not ready: ${prepared.outcome}`);
        }
      }
      return { latencies, metrics, outcomes: [...outcomes] };
    } finally {
      engine.close();
      fs.rmSync(directory, { recursive: true, force: true });
    }
  }
}

// A5/A6: the selected M3 experiment module, driven in a fresh V8 instance per sample.
export class ExperimentDriver {
  constructor(name, watFile, wasmFile, limits = null) {
    this.name = name;
    this.watFile = watFile;
    this.wasmFile = wasmFile;
    this.limits = limits ?? [config.limits.allocation_bytes, config.limits.recipe_program_leaves,
      config.limits.composition_depth, config.limits.operand_bytes,
      config.limits.continuation_bytes, config.limits.dispatch_steps];
    this.wat = fs.readFileSync(watFile);
    this.assembled = assemble(watFile, wasmFile);
    this.wasmSha256 = sha256(this.assembled.bytes);
    this.module = new WebAssembly.Module(this.assembled.bytes);
  }

  instance() {
    return new WebAssembly.Instance(this.module, Object.create(null)).exports;
  }

  // One declared workload root, phased like the selected M3 worker.
  root({ build = null, invoke = null, direct = null, reflect = false, limits = null } = {}) {
    const exports = this.instance();
    const observation = { status: null, phase: null, output: [], snapshots: {},
      timings_ns: { begin: null, construction: null, invocation: null, reflection: null, cleanup: null } };
    const limitKeys = ['allocation_bytes', 'recipe_program_leaves', 'composition_depth',
      'operand_bytes', 'continuation_bytes', 'dispatch_steps'];
    const limitsValue = { ...Object.fromEntries(limitKeys.map((key, index) => [key, this.limits[index]])),
      ...(limits ?? {}) };
    const run = (name, call) => {
      const result = timed(call);
      observation.timings_ns[name] = result.ns;
      observation.status = Number(result.value);
      if (observation.status !== 0) observation.phase = name;
      observation.snapshots[name] = decodeMetrics(exports);
      return observation.status === 0;
    };
    try {
      let okay = run('begin', () => exports.begin(...limitKeys.map(key => limitsValue[key])));
      if (okay && build) {
        okay = run('construction', () => build.kind === 'tree'
          ? exports.build_tree(build.shape, build.leaves)
          : build.kind === 'capture'
            ? exports.build_capture(BigInt(build.value), build.path)
            : exports.build_static(build.index));
      }
      if (okay && invoke) {
        okay = run('invocation', () => invoke.kind === 'invoke'
          ? exports.invoke(BigInt(invoke.value), 1) : exports.invoke2(BigInt(invoke.a), BigInt(invoke.b)));
      }
      if (okay && direct) {
        okay = run('invocation', () => direct.kind === 'list'
          ? exports.run_list(direct.choice, BigInt(direct.input))
          : exports.run_continuation(direct.choice, BigInt(direct.input)));
      }
      if (okay) observation.output = [String(exports.result(0))];
      if (okay && reflect) {
        okay = run('reflection', () => exports.reflect());
        observation.recipe_length = Number(exports.recipe_length());
      }
    } catch (error) {
      observation.engine_error = `${error.name}: ${error.message}`;
    } finally {
      const cleanup = timed(() => exports.cleanup());
      observation.timings_ns.cleanup = cleanup.ns;
      observation.cleanup = { returned_live_bytes: Number(cleanup.value), metrics: decodeMetrics(exports) };
    }
    return observation;
  }

  // W2: N roots over one module instance, as a long session.
  repeatedRoots(count) {
    const exports = this.instance();
    const latencies = [];
    const metrics = [];
    const ports = [];
    for (let index = 0; index < count; index += 1) {
      const start = process.hrtime.bigint();
      const status = exports.begin(...this.limits);
      let live = null;
      if (status === 0) {
        const built = exports.build_static(0);
        const invoked = built === 0 ? exports.invoke(41n, 1) : built;
        live = invoked === 0 ? [String(exports.result(0))] : [];
      }
      const returned = exports.cleanup();
      const snapshot = decodeMetrics(exports);
      latencies.push(Number(process.hrtime.bigint() - start));
      metrics.push(snapshot);
      ports.push({ status, live, returned_live_bytes: Number(returned) });
      if (status !== 0) break;
    }
    return { latencies, metrics, ports };
  }

  // W5: capture retained across one begin while a scratch tree is built, then cleanup.
  mixedLifetime(iterations) {
    const latencies = [];
    const metrics = [];
    const observations = [];
    for (let index = 0; index < iterations; index += 1) {
      const start = process.hrtime.bigint();
      const exports = this.instance();
      const status = exports.begin(...this.limits);
      const capture = status === 0 ? exports.build_capture(2n, 0) : 0;
      const scratch = capture === 0 ? exports.build_tree(0, 16) : 0;
      const invoked = scratch === 0 ? exports.invoke(40n, 1) : scratch;
      const output = invoked === 0 ? String(exports.result(0)) : null;
      const returned = exports.cleanup();
      const snapshot = decodeMetrics(exports);
      latencies.push(Number(process.hrtime.bigint() - start));
      metrics.push(snapshot);
      observations.push({ status, capture, scratch, invoked, output, returned_live_bytes: Number(returned) });
    }
    return { latencies, metrics, observations };
  }
}

// A7: the component runtime fragment as the shipped component emitter assembles it.
export class ComponentDriver {
  constructor(watFile, wasmFile) {
    this.name = 'A7';
    const fragment = fs.readFileSync(watFile, 'utf8');
    fs.writeFileSync(`${wasmFile}.wat`, `(module\n${fragment}\n)`);
    this.assembled = assemble(`${wasmFile}.wat`, wasmFile);
    this.wat = fs.readFileSync(`${wasmFile}.wat`);
    this.wasmSha256 = sha256(this.assembled.bytes);
    this.module = new WebAssembly.Module(this.assembled.bytes);
  }

  instance() {
    return new WebAssembly.Instance(this.module, Object.create(null)).exports;
  }

  static metrics(exports) {
    return {
      allocations: Number(exports['noble$allocation-count']()),
      allocated_bytes: Number(exports['noble$allocated-bytes']()),
      copied_bytes: Number(exports['noble$copied-bytes']()),
      live_bytes: exports['noble$live-bytes'](),
      cleanups: Number(exports['noble$cleanup-count']()),
      quota_exceeded: exports['noble$quota-exceeded'](),
      committed_bytes: exports.memory.buffer.byteLength,
    };
  }

  repeatedRoots(count) {
    const exports = this.instance();
    exports['noble$set-allocation-limit'](65536);
    const latencies = [];
    const metrics = [];
    for (let index = 0; index < count; index += 1) {
      const start = process.hrtime.bigint();
      const pointer = exports.cabi_realloc(0, 0, 8, 16);
      const one = { pointer, live: exports['noble$live-bytes']() };
      exports['noble$cleanup']();
      latencies.push(Number(process.hrtime.bigint() - start));
      metrics.push({ ...ComponentDriver.metrics(exports), allocation: one });
    }
    return { latencies, metrics };
  }

  mixedLifetime(iterations) {
    const exports = this.instance();
    exports['noble$set-allocation-limit'](65536);
    const latencies = [];
    const metrics = [];
    const observations = [];
    for (let index = 0; index < iterations; index += 1) {
      const start = process.hrtime.bigint();
      const retained = exports.cabi_realloc(0, 0, 8, 16);
      const scratch = exports.cabi_realloc(0, 0, 8, 64);
      const copied = exports.cabi_realloc(retained, 16, 8, 32);
      const live = exports['noble$live-bytes']();
      exports['noble$cleanup']();
      latencies.push(Number(process.hrtime.bigint() - start));
      const snapshot = ComponentDriver.metrics(exports);
      metrics.push(snapshot);
      observations.push({ retained, scratch, copied, live, cleanup_live: snapshot.live_bytes });
    }
    return { latencies, metrics, observations };
  }

  quota(allocation) {
    const exports = this.instance();
    exports['noble$set-allocation-limit'](allocation);
    let outcome = 'normal';
    let error = null;
    let pointer = null;
    try { pointer = exports.cabi_realloc(0, 0, 8, allocation + 8); }
    catch (caught) { outcome = 'trap'; error = caught.message; }
    return { outcome, error, pointer, metrics: ComponentDriver.metrics(exports) };
  }

  trapOnRange() {
    const exports = this.instance();
    exports['noble$set-allocation-limit'](65536);
    let outcome = 'normal';
    let error = null;
    let pointer = null;
    try { pointer = exports.cabi_realloc(65535, 64, 8, 8); }
    catch (caught) { outcome = 'trap'; error = caught.message; }
    return { outcome, error, pointer, metrics: ComponentDriver.metrics(exports) };
  }
}

// Line-oriented CLI sessions: one process, one report per submission, latency from
// writing the line to receiving its report.
export function lineSession(spawnSpec, lines, { timeoutMs = 900000, allowEarlyExit = false } = {}) {
  const { command, args } = spawnSpec;
  const child = spawn(command, args, { stdio: ['pipe', 'pipe', 'pipe'], env: process.env });
  const reports = [];
  const latencies = [];
  const stamps = [];
  let pending = '';
  let stderr = '';
  let resolveReport = null;
  const notify = () => { if (resolveReport) { const resolve = resolveReport; resolveReport = null; resolve(); } };
  child.stdout.on('data', bytes => {
    pending += bytes.toString('utf8');
    for (let end; (end = pending.indexOf('\n')) >= 0;) {
      const line = pending.slice(0, end);
      pending = pending.slice(end + 1);
      if (!line.trim()) continue;
      reports.push(line);
      if (stamps.length > latencies.length) {
        latencies.push(Number(process.hrtime.bigint() - stamps[latencies.length]));
      }
      notify();
    }
  });
  child.stderr.on('data', bytes => { stderr += bytes.toString(); });
  const deadline = setTimeout(() => child.kill('SIGKILL'), timeoutMs);
  const waitFor = count => new Promise((resolve, reject) => {
    if (reports.length >= count) return resolve();
    resolveReport = () => { if (reports.length >= count) resolve(); };
    child.once('exit', () => (reports.length >= count
      ? resolve() : allowEarlyExit ? resolve() : reject(Error(`session exited before ${count} reports: ${stderr.slice(-600)}`))));
  });
  return (async () => {
    try {
      for (const [index, line] of lines.entries()) {
        stamps.push(process.hrtime.bigint());
        child.stdin.write(`${line}\n`);
        await waitFor(index + 1);
      }
      child.stdin.end();
      const exit = await new Promise(resolve => child.once('exit', (code, signal) => resolve({ code, signal })));
      return { reports, latencies, stderr, exit };
    } finally {
      clearTimeout(deadline);
    }
  })();
}

// A4 live slot: the authority check requires an anonymous pipe, so the child is
// started as `cat | exec noble live slot`.
export function slotSession(cli, authority, operations, { timeoutMs = 900000 } = {}) {
  const child = spawn('/bin/sh', ['-c', 'cat | exec "$1" live slot --engine v8 --authority "$2"',
    'sh', cli, authority], { stdio: ['pipe', 'pipe', 'pipe'], env: process.env });
  const rows = [];
  const latencies = [];
  const stamps = [];
  let pending = '';
  let stderr = '';
  let resolveRow = null;
  const notify = () => { if (resolveRow) { const resolve = resolveRow; resolveRow = null; resolve(); } };
  child.stdout.on('data', bytes => {
    pending += bytes.toString('utf8');
    for (let end; (end = pending.indexOf('\n')) >= 0;) {
      const line = pending.slice(0, end);
      pending = pending.slice(end + 1);
      if (!line.trim()) continue;
      let row;
      try { row = JSON.parse(line); } catch { row = { raw: line }; }
      rows.push(row);
      if (stamps.length > latencies.length) {
        latencies.push(Number(process.hrtime.bigint() - stamps[latencies.length]));
      }
      notify();
    }
  });
  child.stderr.on('data', bytes => { stderr += bytes.toString(); });
  const deadline = setTimeout(() => child.kill('SIGKILL'), timeoutMs);
  const waitFor = count => new Promise((resolve, reject) => {
    if (rows.length >= count) return resolve();
    resolveRow = () => { if (rows.length >= count) resolve(); };
    child.once('exit', () => (rows.length >= count
      ? resolve() : reject(Error(`slot exited before ${count} rows: ${stderr.slice(-600)}`))));
  });
  return (async () => {
    try {
      await waitFor(1); // configured
      for (const [index, operation] of operations.entries()) {
        stamps.push(process.hrtime.bigint());
        child.stdin.write(`${JSON.stringify(operation)}\n`);
        await waitFor(index + 2);
      }
      child.stdin.end();
      const exit = await new Promise(resolve => child.once('exit', (code, signal) => resolve({ code, signal })));
      return { rows, latencies, stderr, exit };
    } finally {
      clearTimeout(deadline);
    }
  })();
}

// Interactive live-slot session: issue one operator request at a time and observe its
// row before choosing the next. The authority check still requires the anonymous pipe.
export async function slotInteractive(cli, authority, script, { timeoutMs = 900000 } = {}) {
  const child = spawn('/bin/sh', ['-c', 'cat | exec "$1" live slot --engine v8 --authority "$2"',
    'sh', cli, authority], { stdio: ['pipe', 'pipe', 'pipe'], env: process.env });
  const rows = [];
  const latencies = [];
  let pending = '';
  let stderr = '';
  let resolveRow = null;
  const notify = () => { if (resolveRow) { const resolve = resolveRow; resolveRow = null; resolve(); } };
  child.stdout.on('data', bytes => {
    pending += bytes.toString('utf8');
    for (let end; (end = pending.indexOf('\n')) >= 0;) {
      const line = pending.slice(0, end);
      pending = pending.slice(end + 1);
      if (!line.trim()) continue;
      let row;
      try { row = JSON.parse(line); } catch { row = { raw: line }; }
      rows.push(row);
      notify();
    }
  });
  child.stderr.on('data', bytes => { stderr += bytes.toString(); });
  const deadline = setTimeout(() => child.kill('SIGKILL'), timeoutMs);
  const waitFor = count => new Promise((resolve, reject) => {
    if (rows.length >= count) return resolve();
    resolveRow = () => { if (rows.length >= count) resolve(); };
    child.once('exit', () => (rows.length >= count
      ? resolve() : reject(Error(`slot exited before ${count} rows: ${stderr.slice(-600)}`))));
  });
  try {
    await waitFor(1); // configured
    const issue = async operation => {
      const count = rows.length + 1;
      const started = process.hrtime.bigint();
      child.stdin.write(`${JSON.stringify(operation)}\n`);
      await waitFor(count);
      latencies.push(Number(process.hrtime.bigint() - started));
      return rows.at(-1);
    };
    const view = { issue, rows, latencies, get stderr() { return stderr; } };
    await script(view);
    child.stdin.end();
    const exit = await new Promise(resolve => child.once('exit', (code, signal) => resolve({ code, signal })));
    return { rows, latencies, stderr, exit };
  } finally {
    clearTimeout(deadline);
  }
}

// A2 Calculator-v1: the shipped terminal session. One output line per accepted input.
export function calculatorSession(cli, lines, { timeoutMs = 1800000 } = {}) {
  const child = spawn(cli, ['calculator'], { stdio: ['pipe', 'pipe', 'pipe'], env: process.env });
  const stdout = [];
  let pending = '';
  let stderr = '';
  child.stdout.on('data', bytes => {
    pending += bytes.toString('utf8');
    for (let end; (end = pending.indexOf('\n')) >= 0;) {
      const line = pending.slice(0, end);
      pending = pending.slice(end + 1);
      stdout.push(line);
    }
  });
  child.stderr.on('data', bytes => { stderr += bytes.toString(); });
  const deadline = setTimeout(() => child.kill('SIGKILL'), timeoutMs);
  const started = process.hrtime.bigint();
  return (async () => {
    try {
      for (const line of lines) child.stdin.write(`${line}\n`);
      child.stdin.end();
      const exit = await new Promise(resolve => child.once('exit', (code, signal) => resolve({ code, signal })));
      return { stdout, stderr, exit, wall_ns: Number(process.hrtime.bigint() - started) };
    } finally {
      clearTimeout(deadline);
    }
  })();
}

// Worker death under an address-space bound: F4 is outside guest accounting and is
// observed at the host boundary, exactly as the design's failure table states.
export function workerDeathProbe(count) {
  const script = `
    const held = [];
    for (let index = 0; index < ${count}; index += 1) held.push(new Uint8Array(8 * 1024 * 1024).fill(index & 255));
    process.stdout.write(JSON.stringify({ survived: held.length }) + '\\n');
  `;
  const run = spawnSync('/bin/sh', ['-c', 'ulimit -v 262144; exec "$1" -e "$2"', 'sh',
    process.execPath, script], { encoding: 'utf8', timeout: 120000 });
  return { exit: run.status, signal: run.signal, stdout: run.stdout?.trim() ?? '',
    stderr: (run.stderr ?? '').slice(-2000) };
}
