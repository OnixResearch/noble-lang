// Shared facts for the allocation-strategy baselines: host identity, pinned tools,
// source binding, metric decoding and the statistics the design's benchmark plan asks
// for (median, p99, bootstrap 95% CI), plus the noise floor that decides whether a
// measurement is allowed to support a D1-D4 decision.
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

export const here = path.dirname(fileURLToPath(import.meta.url));
export const root = path.resolve(here, '../..');
export const runtime = path.join(root, 'crates/noble-cli/src/core/runtime');
export const config = JSON.parse(fs.readFileSync(path.join(runtime, 'config.json'), 'utf8'));
export const abi = JSON.parse(fs.readFileSync(path.join(runtime, 'abi.json'), 'utf8'));

export const sha256 = bytes => createHash('sha256').update(bytes).digest('hex');

export function git(args) {
  const run = spawnSync('git', args, { cwd: root, encoding: 'utf8' });
  if (run.status !== 0) throw Error(`git ${args.join(' ')}: ${run.stderr}`);
  return run.stdout.trim();
}

export function sourceBinding() {
  const commit = git(['rev-parse', 'HEAD']);
  const tree = git(['rev-parse', 'HEAD^{tree}']);
  const status = git(['status', '--porcelain=v1', '--untracked-files=all']);
  const files = [
    'crates/noble-cli/src/core/runtime/engine.mjs',
    'crates/noble-cli/src/core/runtime/engine-host.mjs',
    'crates/noble-cli/src/core/runtime/slot-engine.mjs',
    'crates/noble-cli/src/core/runtime/slot-registry.mjs',
    'crates/noble-cli/src/core/runtime/abi.json',
    'crates/noble-cli/src/core/runtime/config.json',
    'crates/noble-wasm/runtime/accounting.wat',
    'crates/noble-wasm/runtime/linear-address.wat',
    'crates/noble-wasm/runtime/linear-storage.wat',
    'crates/noble-wasm/runtime/live-accounting.wat',
    'crates/noble-wasm/runtime/live-linear-storage.wat',
    'crates/noble-wasm/runtime/calculator.wat',
    'crates/noble-wasm/runtime/common.wat',
    'crates/noble-wasm/runtime/experiment.wat',
    'crates/noble-wasm/runtime/gc-storage.wat',
    'crates/noble-wasm/src/component/runtime.wat',
    'verification/allocation-baselines/core.mjs',
    'verification/allocation-baselines/drivers.mjs',
    'verification/allocation-baselines/fragments.mjs',
    'verification/allocation-baselines/workloads.mjs',
    'verification/allocation-baselines/run.mjs',
    'verification/allocation-baselines/cases.mjs',
  ].filter(file => fs.existsSync(path.join(root, file)));
  return {
    commit, tree, clean: status === '',
    clean_status: status,
    files: Object.fromEntries(files.map(file => [file,
      { bytes: fs.statSync(path.join(root, file)).size,
        sha256: sha256(fs.readFileSync(path.join(root, file))) }])),
  };
}

export function resolveWasmTools() {
  const candidates = [];
  if (process.env.NOBLE_WASM_TOOLS) candidates.push(process.env.NOBLE_WASM_TOOLS);
  if (process.env.WASM_TOOLS_BIN) {
    const envBin = process.env.WASM_TOOLS_BIN;
    try {
      candidates.push(fs.statSync(envBin).isDirectory() ? path.join(envBin, 'wasm-tools') : envBin);
    } catch {
      candidates.push(path.join(envBin, 'wasm-tools'));
    }
  }
  for (const dir of (process.env.PATH ?? '').split(path.delimiter).filter(Boolean)) {
    candidates.push(path.join(dir, 'wasm-tools'));
  }
  candidates.push(path.join(path.dirname(fs.realpathSync(process.execPath)), 'wasm-tools'));
  for (const candidate of candidates) {
    try {
      if (fs.existsSync(candidate) && fs.statSync(candidate).isFile()) {
        fs.accessSync(candidate, fs.constants.X_OK);
        return fs.realpathSync(candidate);
      }
    } catch { /* try next candidate */ }
  }
  return candidates[0];
}

export function toolBinding(cli) {
  const node = fs.realpathSync(process.execPath);
  const wasmTools = resolveWasmTools();
  const tool = candidate => {
    if (!candidate) return null;
    const run = spawnSync(candidate, ['--version'], { encoding: 'utf8' });
    return run.status === 0
      ? { path: fs.realpathSync(candidate), sha256: sha256(fs.readFileSync(candidate)),
        version: run.stdout.trim() }
      : null;
  };
  const cliPath = fs.realpathSync(path.resolve(cli));
  return {
    node: { path: node, version: process.versions.node, v8: process.versions.v8,
      flags: process.execArgv, sha256: sha256(fs.readFileSync(node)) },
    wasm_tools: tool(wasmTools),
    noble: { path: cliPath, sha256: sha256(fs.readFileSync(cliPath)),
      version: spawnSync(cliPath, ['version'], { encoding: 'utf8' }).stdout.trim() || null,
      source_correspondence: 'not-established-by-this-harness' },
  };
}

export function hostFacts() {
  const cpuinfo = fs.readFileSync('/proc/cpuinfo', 'utf8');
  const model = /model name\s*:\s*(.*)/.exec(cpuinfo)?.[1] ?? 'unknown';
  const affinity = (() => {
    const status = fs.readFileSync('/proc/self/status', 'utf8');
    const list = /Cpus_allowed_list:\s*(\S+)/.exec(status)?.[1];
    if (!list) return null;
    return list.split(',').flatMap(part => {
      const [low, high] = part.split('-').map(Number);
      return Number.isInteger(high)
        ? Array.from({ length: high - low + 1 }, (_, index) => low + index)
        : [low];
    });
  })();
  const governor = (() => {
    try { return fs.readFileSync('/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor', 'utf8').trim(); }
    catch { return null; }
  })();
  const mem = /MemTotal:\s*(\d+) kB/.exec(fs.readFileSync('/proc/meminfo', 'utf8'));
  return {
    uname: { sysname: os.type(), release: os.release(), machine: os.arch() },
    cpu_model: model, logical_cpus: os.cpus().length, affinity_cpus: affinity,
    governor, memory_total_bytes: mem ? Number(mem[1]) * 1024 : null,
    loadavg_at_start: os.loadavg(), nproc: os.availableParallelism(),
    node_flags: process.execArgv,
    note: 'affinity is /proc/self/status Cpus_allowed_list; the harness never changes it',
  };
}

export function pressure() {
  const load = os.loadavg();
  let psi = null;
  try {
    psi = fs.readFileSync('/proc/pressure/cpu', 'utf8').trim();
  } catch { /* older kernels */ }
  return { loadavg_1_5_15: load, psi_cpu: psi,
    stamp: new Date().toISOString() };
}

export const METRICS = Object.fromEntries(Object.entries(abi.metrics).map(([id, name]) => [name, Number(id)]));

export function decodeMetrics(exportsObject) {
  const metrics = {};
  for (const [name, id] of Object.entries(METRICS)) {
    const raw = exportsObject.metric(id);
    if (!Number.isSafeInteger(Number(raw)) || Number(raw) < 0) throw Error(`invalid metric ${name}`);
    metrics[name] = Number(raw);
  }
  metrics.committed_bytes = metrics.memory_bytes;
  metrics.live_logical_bytes = metrics.allocated_since_begin_bytes;
  metrics.fragmentation = metrics.committed_bytes === 0 || metrics.live_logical_bytes === 0
    ? null
    : (metrics.committed_bytes - metrics.live_logical_bytes) / metrics.committed_bytes;
  return metrics;
}

export function median(values) {
  const sorted = [...values].sort((a, b) => a - b);
  const middle = Math.floor(sorted.length / 2);
  return sorted.length % 2 === 1 ? sorted[middle] : (sorted[middle - 1] + sorted[middle]) / 2;
}

export function nearestRank(values, percentile) {
  const sorted = [...values].sort((a, b) => a - b);
  if (!sorted.length) return null;
  return sorted[Math.min(sorted.length - 1, Math.ceil(percentile * sorted.length) - 1)];
}

// Deterministic bootstrap: xorshift seeded by the point name, so every receipt can
// reproduce the interval without a random source.
export function bootstrapCI(values, iterations = 2000) {
  if (values.length < 2) return null;
  let state = 0x9e3779b9;
  const random = () => {
    state ^= state << 13; state ^= state >>> 17; state ^= state << 5;
    return ((state >>> 0) / 0x1_0000_0000);
  };
  const medians = [];
  for (let iteration = 0; iteration < iterations; iteration += 1) {
    const draw = Array.from({ length: values.length }, () => values[Math.floor(random() * values.length)]);
    medians.push(median(draw));
  }
  return { low: nearestRank(medians, 0.025), high: nearestRank(medians, 0.975),
    iterations, method: 'percentile-bootstrap-of-median-deterministic-xorshift' };
}

export function summarize(values) {
  if (!values.length) return null;
  const med = median(values);
  const deviations = values.map(value => Math.abs(value - med));
  const mad = median(deviations);
  return {
    count: values.length, median: med, p99: nearestRank(values, 0.99),
    min: Math.min(...values), max: Math.max(...values),
    mean: values.reduce((sum, value) => sum + value, 0) / values.length,
    mad, relative_mad: med === 0 ? null : mad / med,
    ci95: bootstrapCI(values),
  };
}

// The design asks for a decision only when the noise floor permits one. A point is
// decisive when its bootstrap CI is narrower than the smallest effect the design
// tests (20% for D2). Recorded rather than tuned: no threshold is adjusted from data.
export const DECISION_EFFECT = 0.2;
export function decisive(interval) {
  if (!interval || interval.low === null || interval.high === null) return false;
  return (interval.high - interval.low) / interval.low <= DECISION_EFFECT;
}

export function writeJson(file, value, flag = 'wx') {
  fs.writeFileSync(file, `${JSON.stringify(value, null, 2)}\n`, { flag });
}

export function freshDirectory(requested) {
  const destination = path.resolve(requested);
  fs.mkdirSync(path.dirname(destination), { recursive: true });
  fs.mkdirSync(destination);
  return destination;
}

export function timed(call) {
  const start = process.hrtime.bigint();
  const value = call();
  return { value, ns: Number(process.hrtime.bigint() - start) };
}
