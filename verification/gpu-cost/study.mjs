#!/usr/bin/env node
// Phase-2 CPU-side cost study for `.cairn/changes/gpu-offload-research`
// (issue #2, Owner Decisions 2 & 3).
//
// Measures `c_disp(w)`, `c_alloc`, `c_traverse`, `c_pack`, `c_unpack`,
// `c_load_only`, and end-to-end sequential `c_cpu_map(w)` on the selected
// Node/V8 Wasm route (`crates/noble-cli/src/core/runtime/config.json`) across
// representative `GPU-Map-Draft` eligible `[I64] -> [I64] ! {}` kernels,
// verified on every sample against an independent mathematical K-NUM-01 oracle.
//
// Usage:
//   node verification/gpu-cost/study.mjs --self-test
//   node verification/gpu-cost/study.mjs --out NEW_DIRECTORY [--noble PATH] [--allow-dirty]
//
// Not a GPU hardware measurement, not a proof, and not a conformance promotion.

import assert from 'node:assert/strict';
import { execFileSync, spawnSync } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  BOUNDARY_INPUTS,
  I64_MAX,
  I64_MIN,
  KERNELS,
  checksum,
  nullChecksum,
  sequenceValues,
  wrap64,
} from './kernels.mjs';

const SCRIPT_PATH = fileURLToPath(import.meta.url);
const DIR = path.dirname(SCRIPT_PATH);
const ROOT = fs.realpathSync(path.resolve(DIR, '../..'));
const CONFIG_PATH = path.join(ROOT, 'crates/noble-cli/src/core/runtime/config.json');
const ABI_PATH = path.join(ROOT, 'crates/noble-cli/src/core/runtime/abi.json');

const SCOPE = [
  'crates/noble-kernel/src',
  'crates/noble-contracts/src',
  'crates/noble-wasm/src',
  'crates/noble-wasm/runtime',
  'crates/noble-cli/src',
  'Cargo.toml',
  'Cargo.lock',
  'verification/gpu-cost',
];

const DEFAULT_LIMITS = [65536, 256, 256, 2048, 4096, 100000];
const MAX_LIMITS = [196608, 256, 256, 2048, 4096, 100000];
const HELPER_COUNTS = [64, 256, 682, 1000];
const DISPATCH_COUNTS = [64, 256, 682, 1000];
const MAP_COUNTS = [64, 256, 682];
const SAMPLES = 15;
const WARMUP_INSTANCES = 8;
const OPTIMIZATIONS = ['off', 'on'];
const PARAMETRIC_FIXED_OVERHEAD_US = [50, 100, 200, 500];
const CEILING_N = {
  default_io_map: 340,
  default_single_list: 682,
  max_io_map: 1022,
  max_single_list: 2044,
  theoretical_max_list: 2048,
};

const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const hashFile = file => sha(fs.readFileSync(file));

function argValue(flag) {
  const index = process.argv.indexOf(flag);
  return index > 0 && index + 1 < process.argv.length ? process.argv[index + 1] : undefined;
}

function arraysEqual(a, b) {
  return Array.isArray(a) && Array.isArray(b) && a.length === b.length && a.every((v, i) => v === b[i]);
}

function summarize(values) {
  assert.ok(values.length > 0, 'summarize requires non-empty array');
  const sorted = [...values].sort((a, b) => a - b);
  const n = sorted.length;
  const pick = p => sorted[Math.min(n - 1, Math.max(0, Math.ceil(p * n) - 1))];
  const median = n % 2 === 1 ? sorted[(n - 1) / 2] : (sorted[n / 2 - 1] + sorted[n / 2]) / 2;
  const mean = sorted.reduce((acc, v) => acc + v, 0) / n;
  const q1 = pick(0.25);
  const q3 = pick(0.75);
  return {
    samples: n,
    min: sorted[0],
    q1,
    median,
    mean,
    q3,
    iqr: q3 - q1,
    p95: pick(0.95),
    max: sorted[n - 1],
  };
}

function round(v, digits = 2) {
  if (v === null || v === undefined || !Number.isFinite(v)) return null;
  const factor = 10 ** digits;
  return Math.round(v * factor) / factor;
}

function computeCrossover(cDispNs, cPackNs, cUnpackNs, fixedOverheadUsList = PARAMETRIC_FIXED_OVERHEAD_US) {
  // List<I64> path: GPU pays c_pack + c_unpack extra per element; c_alloc cancels.
  // Optimistic upper bound on denominator (assumes infinite PCIe bandwidth and
  // zero GPU kernel execution time: 8/B_h2d = 8/B_d2h = w*c_op/lanes = 0).
  const denomListNs = cDispNs - cPackNs - cUnpackNs;
  // Packed path (if a contiguous [I64; n] representation existed): c_pack = c_unpack = 0.
  const denomPackedNs = cDispNs;

  const listNStar = {};
  const packedNStar = {};
  for (const us of fixedOverheadUsList) {
    const fixedNs = us * 1000;
    listNStar[`${us}us`] = denomListNs > 0 ? Math.ceil(fixedNs / denomListNs) : null;
    packedNStar[`${us}us`] = denomPackedNs > 0 ? Math.ceil(fixedNs / denomPackedNs) : null;
  }

  const maxAffordableFixedUs = {};
  for (const [label, n] of Object.entries(CEILING_N)) {
    maxAffordableFixedUs[label] = {
      n,
      list_us: round(Math.max(0, denomListNs) * n / 1000, 2),
      packed_us: round(Math.max(0, denomPackedNs) * n / 1000, 2),
    };
  }

  return {
    c_disp_ns: round(cDispNs, 2),
    c_pack_ns: round(cPackNs, 2),
    c_unpack_ns: round(cUnpackNs, 2),
    denom_list_ns: round(denomListNs, 2),
    denom_list_positive: denomListNs > 0,
    denom_packed_ns: round(denomPackedNs, 2),
    optimistic_n_star_list: listNStar,
    optimistic_n_star_packed: packedNStar,
    max_affordable_fixed_overhead_us: maxAffordableFixedUs,
  };
}

function verifyChecksum(actualRaw, expectedBigInt, label) {
  const actual = BigInt.asIntN(64, actualRaw);
  const expected = wrap64(expectedBigInt);
  if (actual !== expected) {
    throw new Error(`oracle mismatch (${label}): got ${actual}, expected ${expected}`);
  }
  return true;
}

function runSelfTest() {
  // 1. K-NUM-01 wrap64 mathematical model checks.
  assert.equal(wrap64(I64_MAX + 1n), I64_MIN);
  assert.equal(wrap64(I64_MIN - 1n), I64_MAX);
  assert.equal(wrap64(I64_MIN * -1n), I64_MIN);
  assert.equal(wrap64((1n << 32n) * (1n << 32n)), 0n);
  assert.equal(wrap64(-1n), -1n);
  assert.equal(wrap64(0n), 0n);

  // 2. Per-kernel exact oracle spot checks (including the kernels whose oracle
  // was previously mis-mapped in probe10.mjs: w4, w6, w8, m4).
  const byId = new Map(KERNELS.map(k => [k.id, k]));
  assert.equal(byId.get('w2_add').oracle(I64_MAX), I64_MIN);
  assert.equal(byId.get('w3_swap_add').oracle(7n), 8n);
  assert.equal(byId.get('w3_sub_order').oracle(3n), 7n);
  assert.equal(byId.get('w3_sub_order').oracle(13n), -3n);
  assert.equal(byId.get('w4_add2').oracle(7n), 9n);
  assert.equal(byId.get('w6_add3').oracle(7n), 10n);
  assert.equal(byId.get('w8_add4').oracle(7n), 11n);
  assert.equal(byId.get('m2_sq').oracle(1n << 32n), 0n);
  assert.equal(byId.get('m4_sub_sq').oracle(7n), 16n);
  assert.equal(byId.get('m9_poly').oracle(4n), 35n);
  assert.equal(byId.get('br_if').oracle(0n), 7n);
  assert.equal(byId.get('br_if').oracle(5n), 15n);

  // 3. Negative control: prove the oracle rejects wrong outputs (e.g. using
  // w2_add's expectation for w4_add2, the probe10.mjs bug).
  const vals = sequenceValues(7n, 64);
  const w4Expected = checksum(vals.map(v => byId.get('w4_add2').oracle(v)));
  const w2Wrong = checksum(vals.map(v => byId.get('w2_add').oracle(v)));
  assert.throws(() => verifyChecksum(w2Wrong, w4Expected, 'tampered-w4'), /oracle mismatch/);
  assert.throws(() => verifyChecksum(w4Expected ^ 1n, w4Expected, 'bitflip'), /oracle mismatch/);
  assert.equal(verifyChecksum(w4Expected, w4Expected, 'exact-w4'), true);

  // 4. Crossover math controls: negative denominator => null (unreachable);
  // positive denominator => exact ceil(L / denom).
  const neg = computeCrossover(50, 60, 5, [100]);
  assert.equal(neg.denom_list_positive, false);
  assert.equal(neg.optimistic_n_star_list['100us'], null);
  assert.equal(neg.max_affordable_fixed_overhead_us.default_single_list.list_us, 0);

  const pos = computeCrossover(200, 50, 0, [100]);
  assert.equal(pos.denom_list_positive, true);
  assert.equal(pos.denom_list_ns, 150);
  assert.equal(pos.optimistic_n_star_list['100us'], 667);
  assert.equal(pos.optimistic_n_star_packed['100us'], 500);

  // 5. Statistics summary control.
  const s = summarize([10, 30, 20, 40, 50]);
  assert.equal(s.min, 10);
  assert.equal(s.median, 30);
  assert.equal(s.max, 50);
  assert.equal(s.mean, 30);

  console.log(JSON.stringify({
    self_test: 'passed',
    kernels: KERNELS.length,
    boundary_inputs: BOUNDARY_INPUTS.length,
    controls: ['wrap64-k-num-01', 'per-kernel-oracles', 'tampered-output-rejected', 'crossover-math', 'summary-stats'],
  }, null, 2));
}

if (process.argv.includes('--self-test')) {
  runSelfTest();
  process.exit(0);
}

const config = JSON.parse(fs.readFileSync(CONFIG_PATH, 'utf8'));
const abi = JSON.parse(fs.readFileSync(ABI_PATH, 'utf8'));

// Ensure execution under the exact selected Node/V8 flags from config.json.
if (!arraysEqual(process.execArgv, config.node_flags)) {
  const selectedNode = fs.existsSync(config.tools.node.path) ? config.tools.node.path : process.execPath;
  const childEnv = { ...process.env };
  delete childEnv.NODE_OPTIONS;
  const child = spawnSync(selectedNode, [...config.node_flags, SCRIPT_PATH, ...process.argv.slice(2)], {
    stdio: 'inherit',
    env: childEnv,
  });
  if (child.error) throw child.error;
  process.exit(child.status ?? 1);
}

assert.equal(process.versions.node, config.tools.node.version,
  `Node version mismatch: got ${process.versions.node}, expected ${config.tools.node.version}`);
assert.equal(process.versions.v8, config.tools.node.v8,
  `V8 version mismatch: got ${process.versions.v8}, expected ${config.tools.node.v8}`);
assert.deepEqual(process.execArgv, config.node_flags, 'Node flags mismatch');

function git(...args) {
  return execFileSync('git', args, { cwd: ROOT, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 });
}

function resolveTool(name, configEntry, envDirVar, binaryName, expectedPrefix) {
  const candidates = [];
  if (configEntry?.path && fs.existsSync(configEntry.path)) candidates.push(configEntry.path);
  if (envDirVar && process.env[envDirVar]) {
    const p = path.join(process.env[envDirVar], binaryName);
    if (fs.existsSync(p)) candidates.push(p);
  }
  const resolved = candidates[0] ?? binaryName;
  const ver = spawnSync(resolved, ['--version'], { encoding: 'utf8', timeout: 15000 });
  assert.equal(ver.status, 0, `${name} --version failed: ${ver.stderr}`);
  const versionText = ver.stdout.trim();
  if (expectedPrefix) {
    assert.ok(versionText.startsWith(expectedPrefix),
      `${name} version mismatch: got "${versionText}", expected prefix "${expectedPrefix}"`);
  }
  const real = fs.existsSync(resolved) ? fs.realpathSync(resolved) : resolved;
  return {
    path: real,
    version: versionText,
    sha256: fs.existsSync(real) ? hashFile(real) : null,
  };
}

function sourceSnapshot(allowDirty) {
  const dirty = git('status', '--porcelain', '--untracked-files=all', '--', ...SCOPE).trim();
  if (!allowDirty) {
    assert.equal(dirty, '', `source scope is not clean (commit changes first or pass --allow-dirty for dev):\n${dirty}`);
  }
  const head = git('rev-parse', 'HEAD').trim();
  const files = git('ls-files', '-z', '--', ...SCOPE).split('\0').filter(Boolean).sort();
  const snapshot = {};
  const lines = [];
  for (const file of files) {
    const digest = hashFile(path.join(ROOT, file));
    snapshot[file] = { sha256: digest };
    lines.push(`${file} ${digest}\n`);
  }
  return {
    git_head: head,
    dirty: dirty.length > 0,
    scope: SCOPE,
    file_count: files.length,
    source_revision: `sha256:${sha(lines.join(''))}`,
    files: snapshot,
  };
}

function resolveNoble(outDir, explicitPath) {
  if (explicitPath) {
    const real = fs.realpathSync(explicitPath);
    assert.ok(fs.existsSync(real), `noble binary not found: ${real}`);
    return { path: real, built_by_harness: false, before_sha256: hashFile(real) };
  }
  const build = spawnSync('cargo', [
    'build', '--locked', '-p', 'noble-cli', '--bin', 'noble',
    '--message-format=json-render-diagnostics',
  ], { cwd: ROOT, encoding: 'utf8', maxBuffer: 256 * 1024 * 1024 });
  fs.mkdirSync(path.join(outDir, 'raw'), { recursive: true });
  fs.writeFileSync(path.join(outDir, 'raw', 'cargo-build.stdout.txt'), build.stdout ?? '');
  fs.writeFileSync(path.join(outDir, 'raw', 'cargo-build.stderr.txt'), build.stderr ?? '');
  assert.equal(build.status, 0, `cargo build failed:\n${build.stderr}`);
  const artifacts = (build.stdout ?? '').split('\n').filter(Boolean)
    .map(line => { try { return JSON.parse(line); } catch { return null; } })
    .filter(m => m && m.reason === 'compiler-artifact' && m.target?.name === 'noble' && m.executable);
  assert.equal(artifacts.length, 1, 'expected one noble executable artifact');
  const real = fs.realpathSync(artifacts[0].executable);
  return { path: real, built_by_harness: true, before_sha256: hashFile(real) };
}

function buildHarnessModules(watSource, wrapperWat, workDir, stem, wasmTools, wasmOpt) {
  const closeIdx = watSource.lastIndexOf(')');
  assert.ok(closeIdx > 0, `malformed WAT for ${stem}`);
  const combinedWat = `${watSource.slice(0, closeIdx)}\n${wrapperWat}\n${watSource.slice(closeIdx)}`;
  const watFile = path.join(workDir, `${stem}.wat`);
  const wasmOff = path.join(workDir, `${stem}-off.wasm`);
  const wasmOn = path.join(workDir, `${stem}-on.wasm`);
  fs.writeFileSync(watFile, combinedWat);

  const parsed = spawnSync(wasmTools.path, ['parse', watFile, '-o', wasmOff], { encoding: 'utf8' });
  assert.equal(parsed.status, 0, `wasm-tools parse failed (${stem}): ${parsed.stderr}`);
  const valOff = spawnSync(wasmTools.path, ['validate', wasmOff], { encoding: 'utf8' });
  assert.equal(valOff.status, 0, `wasm-tools validate off failed (${stem}): ${valOff.stderr}`);

  const opt = spawnSync(wasmOpt.path, [wasmOff, ...config.optimizer_flags, '-o', wasmOn], { encoding: 'utf8' });
  assert.equal(opt.status, 0, `wasm-opt failed (${stem}): ${opt.stderr}`);
  const valOn = spawnSync(wasmTools.path, ['validate', wasmOn], { encoding: 'utf8' });
  assert.equal(valOn.status, 0, `wasm-tools validate on failed (${stem}): ${valOn.stderr}`);

  const offBytes = fs.readFileSync(wasmOff);
  const onBytes = fs.readFileSync(wasmOn);
  return {
    off: {
      module: new WebAssembly.Module(offBytes),
      bytes: offBytes.length,
      sha256: sha(offBytes),
    },
    on: {
      module: new WebAssembly.Module(onBytes),
      bytes: onBytes.length,
      sha256: sha(onBytes),
    },
    wat_sha256: sha(combinedWat),
  };
}

function freshInstance(wasmModule, limits = MAX_LIMITS) {
  const memory = new WebAssembly.Memory({ initial: abi.memory.initial, maximum: abi.memory.maximum });
  const table = new WebAssembly.Table({
    element: abi.table.element,
    initial: abi.table.initial,
    maximum: abi.table.maximum,
  });
  const shared = Object.create(null);
  shared[abi.memory.name] = memory;
  shared[abi.table.name] = table;
  for (const g of abi.globals) {
    shared[g.name] = new WebAssembly.Global(
      { value: g.type, mutable: true },
      g.type === 'i64' ? BigInt(g.value) : g.value,
    );
  }
  const exports = new WebAssembly.Instance(wasmModule, { noble: shared }).exports;
  const begun = exports.begin(...limits);
  assert.equal(begun, 0, `begin failed: ${begun}`);
  const submitted = exports.submit();
  assert.equal(submitted, 0, `submit failed: ${submitted}`);
  return { x: exports, shared, memory };
}

function timed(fn) {
  const t0 = process.hrtime.bigint();
  const result = fn();
  const t1 = process.hrtime.bigint();
  return { result, ns: Number(t1 - t0) };
}

function runBoundaryChecks(compiledKernels) {
  const report = [];
  for (const opt of OPTIMIZATIONS) {
    for (const entry of compiledKernels) {
      const mod = entry.harness[opt].module;
      const { x } = freshInstance(mod, MAX_LIMITS);
      const handle = Number(x.stack_value(0));
      let checked = 0;
      for (const input of BOUNDARY_INPUTS) {
        const begun = x.begin(...MAX_LIMITS);
        assert.equal(begun, 0);
        const actual = BigInt.asIntN(64, x.eval_one(handle, input));
        const expected = entry.kernel.oracle(input);
        const failure = Number(x.metric(0));
        const alloc = Number(x.metric(2));
        const steps = Number(x.metric(8));
        assert.equal(failure, 0, `${entry.kernel.id}/${opt} failed on ${input}: status=${failure}`);
        assert.equal(alloc, 0, `${entry.kernel.id}/${opt} allocated ${alloc} bytes on ${input}`);
        assert.equal(steps, entry.kernel.expected_steps_per_element,
          `${entry.kernel.id}/${opt} steps=${steps} expected=${entry.kernel.expected_steps_per_element} on ${input}`);
        assert.equal(actual, expected,
          `K-NUM-01 mismatch on ${entry.kernel.id}/${opt} input=${input}: got=${actual} expected=${expected}`);
        checked += 1;
      }
      report.push({
        opt,
        kernel: entry.kernel.id,
        expected_steps_per_element: entry.kernel.expected_steps_per_element,
        boundary_inputs_checked: checked,
        passed: true,
      });
    }
  }
  return report;
}

function runCeilingControls(helperMod, w2Mod, w64Mod) {
  const results = {};

  // 1. Default 65,536-byte limit: single List<I64> (at 682, over 683).
  {
    const { x } = freshInstance(helperMod, DEFAULT_LIMITS);
    x.begin(...DEFAULT_LIMITS);
    const list = x.build_list(682, 5n);
    const fail = Number(x.metric(0));
    const alloc = Number(x.metric(2));
    const chk = x.verify_list(list, 682);
    const expectedDesc = checksum(sequenceValues(5n, 682).reverse());
    assert.equal(fail, 0);
    assert.equal(alloc, (2 * 682 + 1) * 48);
    verifyChecksum(chk, expectedDesc, 'ceiling-default-single-682');
    results.default_single_list_at_bound = { n: 682, allocation_limit: 65536, allocated_bytes: alloc, status: fail, passed: true };
  }
  {
    const { x } = freshInstance(helperMod, DEFAULT_LIMITS);
    x.begin(...DEFAULT_LIMITS);
    x.build_list(683, 5n);
    const fail = Number(x.metric(0));
    const reason = Number(x.metric(1));
    assert.notEqual(fail, 0, 'expected allocation exhaustion at n=683 under 65536-byte limit');
    assert.equal(reason, 1, `expected quota_reason=1 (allocation), got ${reason}`);
    results.default_single_list_over_bound = { n: 683, allocation_limit: 65536, status: fail, quota_reason: reason, refused_as_expected: true };
  }

  // 2. Default 65,536-byte limit: input + output List<I64> in one submission (at 340, over 341).
  const w2Oracle = KERNELS.find(k => k.id === 'w2_add').oracle;
  {
    const { x } = freshInstance(w2Mod, DEFAULT_LIMITS);
    const h = Number(x.stack_value(0));
    x.begin(...DEFAULT_LIMITS);
    const list = x.build_list(340, 7n);
    const mapRes = x.measure_cpu_map(h, list, 340);
    const fail = Number(x.metric(0));
    const alloc = Number(x.metric(2));
    const outList = x.last_out_list();
    const outChk = x.verify_list(outList, 340);
    const inputsAsc = sequenceValues(7n, 340);
    const outputsAsc = inputsAsc.map(w2Oracle);
    assert.equal(fail, 0);
    assert.equal(alloc, 2 * (2 * 340 + 1) * 48);
    verifyChecksum(mapRes, checksum([...outputsAsc].reverse()), 'ceiling-default-io-340-map');
    verifyChecksum(outChk, checksum(outputsAsc), 'ceiling-default-io-340-out');
    results.default_io_map_at_bound = { n: 340, allocation_limit: 65536, allocated_bytes: alloc, status: fail, passed: true };
  }
  {
    const { x } = freshInstance(w2Mod, DEFAULT_LIMITS);
    const h = Number(x.stack_value(0));
    x.begin(...DEFAULT_LIMITS);
    const list = x.build_list(341, 7n);
    x.measure_cpu_map(h, list, 341);
    const fail = Number(x.metric(0));
    const reason = Number(x.metric(1));
    assert.notEqual(fail, 0, 'expected allocation exhaustion for input+output map at n=341');
    assert.equal(reason, 1);
    results.default_io_map_over_bound = { n: 341, allocation_limit: 65536, status: fail, quota_reason: reason, refused_as_expected: true };
  }

  // 3. Maximum 196,608-byte / 4096-cell limit: single list and input+output map.
  const initCells = (() => {
    const { shared } = freshInstance(w2Mod, MAX_LIMITS);
    return shared.heap_cursor.value;
  })();
  const nMaxSingle = Math.floor((4096 - initCells - 1) / 2);
  const nMaxIo = Math.floor((4096 - initCells - 2) / 4);

  {
    const { x, shared } = freshInstance(helperMod, MAX_LIMITS);
    x.begin(...MAX_LIMITS);
    const list = x.build_list(nMaxSingle, 5n);
    const fail = Number(x.metric(0));
    const alloc = Number(x.metric(2));
    const chk = x.verify_list(list, nMaxSingle);
    assert.equal(fail, 0);
    verifyChecksum(chk, checksum(sequenceValues(5n, nMaxSingle).reverse()), 'ceiling-max-single');
    results.max_single_list_at_bound = {
      init_cells: initCells,
      n: nMaxSingle,
      final_heap_cursor: shared.heap_cursor.value,
      cell_capacity: 4096,
      allocated_bytes: alloc,
      status: fail,
      passed: true,
    };
  }
  {
    const { x } = freshInstance(helperMod, MAX_LIMITS);
    x.begin(...MAX_LIMITS);
    x.build_list(nMaxSingle + 1, 5n);
    const fail = Number(x.metric(0));
    const reason = Number(x.metric(1));
    assert.notEqual(fail, 0);
    assert.equal(reason, 1);
    results.max_single_list_over_bound = {
      init_cells: initCells,
      n: nMaxSingle + 1,
      cell_capacity: 4096,
      status: fail,
      quota_reason: reason,
      refused_as_expected: true,
    };
  }

  {
    const { x, shared } = freshInstance(w2Mod, MAX_LIMITS);
    const h = Number(x.stack_value(0));
    x.begin(...MAX_LIMITS);
    const list = x.build_list(nMaxIo, 7n);
    const mapRes = x.measure_cpu_map(h, list, nMaxIo);
    const fail = Number(x.metric(0));
    const alloc = Number(x.metric(2));
    const outChk = x.verify_list(x.last_out_list(), nMaxIo);
    const outputsAsc = sequenceValues(7n, nMaxIo).map(w2Oracle);
    assert.equal(fail, 0);
    verifyChecksum(mapRes, checksum([...outputsAsc].reverse()), 'ceiling-max-io-map');
    verifyChecksum(outChk, checksum(outputsAsc), 'ceiling-max-io-out');
    results.max_io_map_at_bound = {
      init_cells: initCells,
      n: nMaxIo,
      final_heap_cursor: shared.heap_cursor.value,
      cell_capacity: 4096,
      allocated_bytes: alloc,
      status: fail,
      passed: true,
    };
  }
  {
    const { x } = freshInstance(w2Mod, MAX_LIMITS);
    const h = Number(x.stack_value(0));
    x.begin(...MAX_LIMITS);
    const list = x.build_list(nMaxIo + 1, 7n);
    x.measure_cpu_map(h, list, nMaxIo + 1);
    const fail = Number(x.metric(0));
    const reason = Number(x.metric(1));
    assert.notEqual(fail, 0);
    assert.equal(reason, 1);
    results.max_io_map_over_bound = {
      init_cells: initCells,
      n: nMaxIo + 1,
      cell_capacity: 4096,
      status: fail,
      quota_reason: reason,
      refused_as_expected: true,
    };
  }

  // 4. Execution step limit (100,000 steps) at w = 64: n = 1562 passes, n = 1563 refuses.
  const w64Oracle = KERNELS.find(k => k.id === 'w64_arith').oracle;
  {
    const { x } = freshInstance(w64Mod, MAX_LIMITS);
    const h = Number(x.stack_value(0));
    x.begin(...MAX_LIMITS);
    const res = x.measure_dispatch(h, 1562, 7n);
    const fail = Number(x.metric(0));
    const steps = Number(x.metric(8));
    assert.equal(fail, 0);
    assert.equal(steps, 1562 * 64);
    verifyChecksum(res, checksum(sequenceValues(7n, 1562).map(w64Oracle)), 'ceiling-step-w64-1562');
    results.step_limit_w64_at_bound = { n: 1562, w: 64, steps, step_limit: 100000, status: fail, passed: true };
  }
  {
    const { x } = freshInstance(w64Mod, MAX_LIMITS);
    const h = Number(x.stack_value(0));
    x.begin(...MAX_LIMITS);
    x.measure_dispatch(h, 1563, 7n);
    const fail = Number(x.metric(0));
    const reason = Number(x.metric(1));
    assert.notEqual(fail, 0);
    assert.equal(reason, 6, `expected quota_reason=6 (execution work), got ${reason}`);
    results.step_limit_w64_over_bound = { n: 1563, w: 64, required_steps: 1563 * 64, step_limit: 100000, status: fail, quota_reason: reason, refused_as_expected: true };
  }

  return results;
}

function measureHelpers(helperHarness) {
  const summary = {};
  const rawSamples = {};
  for (const opt of OPTIMIZATIONS) {
    const mod = helperHarness[opt].module;
    summary[opt] = {};
    rawSamples[opt] = {};

    // Warmup across fresh instances.
    for (let i = 0; i < WARMUP_INSTANCES; i++) {
      const { x } = freshInstance(mod, MAX_LIMITS);
      x.begin(...MAX_LIMITS);
      const list = x.build_list(64, 5n);
      x.measure_pack(list, 64);
      x.measure_pack_traverse(list, 64);
      x.measure_load_only(64);
      x.measure_unpack(64);
      x.build_control(64, 5n);
      x.measure_null(64);
    }

    for (const count of HELPER_COUNTS) {
      const ascValues = sequenceValues(5n, count);
      const descValues = [...ascValues].reverse();
      const expectedNull = nullChecksum(count);
      const expectedAsc = checksum(ascValues);
      const expectedDesc = checksum(descValues);
      const expectedAllocBytes = (2 * count + 1) * 48;

      const rows = [];
      for (let r = 0; r < SAMPLES; r++) {
        // Instance A: null, build_control, build_list (c_alloc).
        const instA = freshInstance(mod, MAX_LIMITS);
        // Per-instance 1-element warmup before begin() so first-call wrapper/cache overhead is excluded.
        instA.x.measure_null(1);
        instA.x.build_control(1, 5n);
        instA.x.build_list(1, 5n);
        instA.x.begin(...MAX_LIMITS);
        const nul = timed(() => instA.x.measure_null(count));
        verifyChecksum(nul.result, expectedNull, `helper-null-${opt}-${count}`);
        const ctl = timed(() => instA.x.build_control(count, 5n));
        verifyChecksum(ctl.result, expectedAsc, `helper-ctl-${opt}-${count}`);
        instA.x.begin(...MAX_LIMITS);
        const alloc = timed(() => instA.x.build_list(count, 5n));
        assert.equal(Number(instA.x.metric(0)), 0);
        assert.equal(Number(instA.x.metric(2)), expectedAllocBytes);
        verifyChecksum(instA.x.verify_list(alloc.result, count), expectedDesc, `helper-alloc-list-${opt}-${count}`);

        // Instance B: pack, pack_traverse, load_only.
        const instB = freshInstance(mod, MAX_LIMITS);
        instB.x.begin(...MAX_LIMITS);
        const listB = instB.x.build_list(count, 5n);
        assert.equal(Number(instB.x.metric(0)), 0);
        instB.x.measure_pack(listB, 1);
        instB.x.measure_pack_traverse(listB, 1);
        instB.x.measure_load_only(1);
        instB.x.begin(...MAX_LIMITS);
        const pack = timed(() => instB.x.measure_pack(listB, count));
        verifyChecksum(pack.result, expectedDesc, `helper-pack-${opt}-${count}`);
        verifyChecksum(instB.x.verify_flat(count), expectedDesc, `helper-flat-${opt}-${count}`);
        const trav = timed(() => instB.x.measure_pack_traverse(listB, count));
        verifyChecksum(trav.result, expectedDesc, `helper-trav-${opt}-${count}`);
        const loadOnly = timed(() => instB.x.measure_load_only(count));
        verifyChecksum(loadOnly.result, expectedDesc, `helper-load-${opt}-${count}`);
        assert.equal(Number(instB.x.metric(0)), 0);
        assert.equal(Number(instB.x.metric(2)), 0);

        // Instance C: populate flat buffer, then measure_unpack and verify rebuilt list.
        const instC = freshInstance(mod, MAX_LIMITS);
        instC.x.begin(...MAX_LIMITS);
        const listC = instC.x.build_list(count, 5n);
        verifyChecksum(instC.x.measure_pack(listC, count), expectedDesc, `helper-prepack-${opt}-${count}`);
        instC.x.measure_unpack(1);
        instC.x.begin(...MAX_LIMITS);
        const unpack = timed(() => instC.x.measure_unpack(count));
        assert.equal(Number(instC.x.metric(0)), 0);
        assert.equal(Number(instC.x.metric(2)), expectedAllocBytes);
        verifyChecksum(unpack.result, expectedDesc, `helper-unpack-${opt}-${count}`);
        verifyChecksum(instC.x.verify_list(instC.x.last_unpack_list(), count), expectedAsc, `helper-unpack-list-${opt}-${count}`);

        rows.push({
          sample: r,
          null_ns: nul.ns,
          build_control_ns: ctl.ns,
          build_list_ns: alloc.ns,
          pack_ns: pack.ns,
          traverse_ns: trav.ns,
          load_only_ns: loadOnly.ns,
          unpack_ns: unpack.ns,
          c_null_per_el: nul.ns / count,
          c_alloc_per_el: (alloc.ns - ctl.ns) / count,
          c_traverse_per_el: (trav.ns - nul.ns) / count,
          c_pack_per_el: (pack.ns - nul.ns) / count,
          c_pack_marginal_per_el: (pack.ns - trav.ns) / count,
          c_load_only_per_el: (loadOnly.ns - nul.ns) / count,
          c_unpack_total_per_el: (unpack.ns - nul.ns) / count,
          c_unpack_marginal_per_el: (unpack.ns - alloc.ns) / count,
          verified: true,
        });
      }

      rawSamples[opt][String(count)] = rows;
      const sOf = key => summarize(rows.map(row => row[key]));
      summary[opt][String(count)] = {
        count,
        samples: SAMPLES,
        all_verified: rows.every(r => r.verified),
        c_null_ns_per_el: sOf('c_null_per_el'),
        c_alloc_ns_per_el: sOf('c_alloc_per_el'),
        c_traverse_ns_per_el: sOf('c_traverse_per_el'),
        c_pack_ns_per_el: sOf('c_pack_per_el'),
        c_pack_marginal_ns_per_el: sOf('c_pack_marginal_per_el'),
        c_load_only_ns_per_el: sOf('c_load_only_per_el'),
        c_unpack_total_ns_per_el: sOf('c_unpack_total_per_el'),
        c_unpack_marginal_ns_per_el: sOf('c_unpack_marginal_per_el'),
      };
    }
  }
  return { summary, rawSamples };
}

function measureKernels(compiledKernels) {
  const summary = {};
  const rawSamples = {};
  for (const opt of OPTIMIZATIONS) {
    summary[opt] = {};
    rawSamples[opt] = {};
    for (const entry of compiledKernels) {
      const { kernel } = entry;
      const mod = entry.harness[opt].module;
      summary[opt][kernel.id] = {
        description: kernel.description,
        source: kernel.source,
        expected_w: kernel.expected_w,
        expected_steps_per_element: kernel.expected_steps_per_element,
        dispatch: {},
        cpu_map: {},
      };
      rawSamples[opt][kernel.id] = { dispatch: {}, cpu_map: {} };

      // Module-level warmup.
      for (let i = 0; i < WARMUP_INSTANCES; i++) {
        const { x } = freshInstance(mod, MAX_LIMITS);
        const h = Number(x.stack_value(0));
        x.begin(...MAX_LIMITS);
        x.measure_null(64);
        x.measure_dispatch(h, 64, 7n);
        const l = x.build_list(32, 7n);
        x.begin(...MAX_LIMITS);
        x.measure_cpu_map(h, l, 32);
      }

      // 1. Dispatch measurement (`c_disp(w)`).
      for (const count of DISPATCH_COUNTS) {
        const inputs = sequenceValues(7n, count);
        const expectedNull = nullChecksum(count);
        const expectedDispatch = checksum(inputs.map(kernel.oracle));
        const expectedSteps = count * kernel.expected_steps_per_element;

        const rows = [];
        for (let r = 0; r < SAMPLES; r++) {
          const { x } = freshInstance(mod, MAX_LIMITS);
          const h = Number(x.stack_value(0));
          x.measure_null(1);
          x.measure_dispatch(h, 1, 7n);
          x.begin(...MAX_LIMITS);
          const nul = timed(() => x.measure_null(count));
          verifyChecksum(nul.result, expectedNull, `disp-null-${opt}-${kernel.id}-${count}`);
          x.begin(...MAX_LIMITS);
          const disp = timed(() => x.measure_dispatch(h, count, 7n));
          assert.equal(Number(x.metric(0)), 0, `dispatch failed for ${kernel.id}/${opt}`);
          assert.equal(Number(x.metric(2)), 0, `dispatch allocated bytes for ${kernel.id}/${opt}`);
          assert.equal(Number(x.metric(8)), expectedSteps, `dispatch step mismatch for ${kernel.id}/${opt}`);
          verifyChecksum(disp.result, expectedDispatch, `disp-${opt}-${kernel.id}-${count}`);

          rows.push({
            sample: r,
            null_ns: nul.ns,
            dispatch_ns: disp.ns,
            steps: Number(x.metric(8)),
            allocated_bytes: Number(x.metric(2)),
            c_disp_per_el: (disp.ns - nul.ns) / count,
            c_disp_per_step: (disp.ns - nul.ns) / expectedSteps,
            verified: true,
          });
        }
        rawSamples[opt][kernel.id].dispatch[String(count)] = rows;
        summary[opt][kernel.id].dispatch[String(count)] = {
          count,
          samples: SAMPLES,
          steps: expectedSteps,
          all_verified: rows.every(r => r.verified),
          c_disp_ns_per_el: summarize(rows.map(r => r.c_disp_per_el)),
          c_disp_ns_per_step: summarize(rows.map(r => r.c_disp_per_step)),
        };
      }

      // 2. End-to-end sequential `List<I64> -> List<I64>` map (`c_cpu_map(w)`).
      for (const count of MAP_COUNTS) {
        const inputsAsc = sequenceValues(7n, count);
        const outputsAsc = inputsAsc.map(kernel.oracle);
        const expectedNull = nullChecksum(count);
        const expectedMapDesc = checksum([...outputsAsc].reverse());
        const expectedOutAsc = checksum(outputsAsc);
        const expectedSteps = count * kernel.expected_steps_per_element;
        const expectedAllocBytes = (2 * count + 1) * 48;

        const rows = [];
        for (let r = 0; r < SAMPLES; r++) {
          const { x } = freshInstance(mod, MAX_LIMITS);
          const h = Number(x.stack_value(0));
          x.measure_null(1);
          x.measure_dispatch(h, 1, 7n);
          x.begin(...MAX_LIMITS);
          const nul = timed(() => x.measure_null(count));
          verifyChecksum(nul.result, expectedNull, `map-null-${opt}-${kernel.id}-${count}`);
          x.begin(...MAX_LIMITS);
          const inList = x.build_list(count, 7n);
          assert.equal(Number(x.metric(0)), 0);
          x.measure_cpu_map(h, inList, 1);
          x.begin(...MAX_LIMITS);
          const mapT = timed(() => x.measure_cpu_map(h, inList, count));
          assert.equal(Number(x.metric(0)), 0, `cpu_map failed for ${kernel.id}/${opt}`);
          assert.equal(Number(x.metric(2)), expectedAllocBytes, `cpu_map alloc mismatch for ${kernel.id}/${opt}`);
          assert.equal(Number(x.metric(8)), expectedSteps, `cpu_map step mismatch for ${kernel.id}/${opt}`);
          verifyChecksum(mapT.result, expectedMapDesc, `map-res-${opt}-${kernel.id}-${count}`);
          verifyChecksum(x.verify_list(x.last_out_list(), count), expectedOutAsc, `map-out-${opt}-${kernel.id}-${count}`);

          rows.push({
            sample: r,
            null_ns: nul.ns,
            cpu_map_ns: mapT.ns,
            steps: Number(x.metric(8)),
            allocated_bytes: Number(x.metric(2)),
            c_cpu_map_per_el: (mapT.ns - nul.ns) / count,
            verified: true,
          });
        }
        rawSamples[opt][kernel.id].cpu_map[String(count)] = rows;
        summary[opt][kernel.id].cpu_map[String(count)] = {
          count,
          samples: SAMPLES,
          steps: expectedSteps,
          allocated_bytes: expectedAllocBytes,
          all_verified: rows.every(r => r.verified),
          c_cpu_map_ns_per_el: summarize(rows.map(r => r.c_cpu_map_per_el)),
        };
      }
    }
  }
  return { summary, rawSamples };
}

function buildCostModel(helperSummary, kernelSummary) {
  // Use n = 682 (the exact default single-list ceiling) as the primary reference
  // point, with n = 256 and n = 1000 retained for scale comparison.
  const refN = '682';
  const model = {};
  for (const opt of OPTIMIZATIONS) {
    const h = helperSummary[opt][refN];
    const cAlloc = h.c_alloc_ns_per_el.median;
    const cTraverse = h.c_traverse_ns_per_el.median;
    const cPack = h.c_pack_ns_per_el.median;
    const cPackMarginal = h.c_pack_marginal_ns_per_el.median;
    const cUnpackTotal = h.c_unpack_total_ns_per_el.median;
    const cUnpackMarginalRaw = h.c_unpack_marginal_ns_per_el.median;
    // Clamp non-negative marginal unpack for conservative GPU-favourable crossover
    // analysis (negative median reflects measurement noise around ~0 ns/el).
    const cUnpackEffective = Math.max(0, cUnpackMarginalRaw);
    const cLoadOnly = h.c_load_only_ns_per_el.median;

    const perKernel = {};
    for (const kernel of KERNELS) {
      const k = kernelSummary[opt][kernel.id];
      const cDisp = k.dispatch[refN].c_disp_ns_per_el.median;
      const cCpuMap = k.cpu_map[refN].c_cpu_map_ns_per_el.median;
      const additiveCheck = cTraverse + cDisp + cAlloc;
      perKernel[kernel.id] = {
        w: kernel.expected_w,
        steps_per_element: kernel.expected_steps_per_element,
        c_disp_ns_per_el: round(cDisp, 2),
        c_disp_ns_per_step: round(k.dispatch[refN].c_disp_ns_per_step.median, 2),
        c_cpu_map_ns_per_el: round(cCpuMap, 2),
        additive_check_traverse_plus_disp_plus_alloc_ns: round(additiveCheck, 2),
        crossover_conservative_c_pack_full: computeCrossover(cDisp, cPack, cUnpackEffective),
        // Alternative accounting where input list traversal is paid on BOTH CPU
        // and GPU, so only c_pack_marginal + c_unpack_effective is GPU-extra.
        crossover_if_cpu_also_traverses_list: computeCrossover(cDisp, Math.max(0, cPackMarginal), cUnpackEffective),
      };
    }

    model[opt] = {
      reference_n: Number(refN),
      helpers_median_ns_per_el: {
        c_alloc: round(cAlloc, 2),
        c_traverse: round(cTraverse, 2),
        c_pack: round(cPack, 2),
        c_pack_marginal: round(cPackMarginal, 2),
        c_load_only: round(cLoadOnly, 2),
        c_unpack_total: round(cUnpackTotal, 2),
        c_unpack_marginal_raw: round(cUnpackMarginalRaw, 2),
        c_unpack_effective: round(cUnpackEffective, 2),
      },
      kernels: perKernel,
    };
  }
  return model;
}

function main() {
  const outArg = argValue('--out');
  assert.ok(outArg, 'missing required --out NEW_DIRECTORY');
  const outDir = path.resolve(outArg);
  assert.ok(!fs.existsSync(outDir), `output directory already exists: ${outDir}`);
  const allowDirty = process.argv.includes('--allow-dirty');
  const nobleArg = argValue('--noble');

  const startedAt = new Date().toISOString();
  const source = sourceSnapshot(allowDirty);
  const wasmTools = resolveTool('wasm-tools', config.tools.wasm_tools, 'WASM_TOOLS_BIN', 'wasm-tools', `wasm-tools ${config.tools.wasm_tools.version}`);
  const wasmOpt = resolveTool('wasm-opt', config.tools.wasm_opt, null, 'wasm-opt', `wasm-opt version ${config.tools.wasm_opt.version}`);
  const nodeTool = {
    path: fs.realpathSync(process.execPath),
    version: process.versions.node,
    v8: process.versions.v8,
    sha256: hashFile(fs.realpathSync(process.execPath)),
    flags: process.execArgv,
  };

  fs.mkdirSync(path.join(outDir, 'raw/kernels'), { recursive: true });
  fs.mkdirSync(path.join(outDir, 'raw/emit'), { recursive: true });
  fs.mkdirSync(path.join(outDir, 'raw/harness'), { recursive: true });

  const noble = resolveNoble(outDir, nobleArg);
  const dispatchWrap = fs.readFileSync(path.join(DIR, 'wrap-dispatch.wat'), 'utf8');
  const helperWrap = fs.readFileSync(path.join(DIR, 'wrap-helpers.wat'), 'utf8');

  const compiledKernels = [];
  const emittedDigests = [];

  for (const kernel of KERNELS) {
    const srcFile = path.join(outDir, 'raw/kernels', `${kernel.id}.noble`);
    const emitDir = path.join(outDir, 'raw/emit', kernel.id);
    fs.writeFileSync(srcFile, `${kernel.source}\n`);

    const run = spawnSync(noble.path, ['run', srcFile, '--emit', emitDir], {
      encoding: 'utf8',
      timeout: 60000,
    });
    assert.equal(run.status, 0, `noble run failed for ${kernel.id}:\n${run.stderr}`);

    const execReport = JSON.parse(fs.readFileSync(path.join(emitDir, 'engine/module-1-execution.json'), 'utf8'));
    assert.equal(execReport.outcome, 'normal', `unexpected execution outcome for ${kernel.id}`);
    assert.equal(execReport.status, 0, `unexpected execution status for ${kernel.id}`);
    assert.equal(execReport.stack.length, 1, `expected 1 stack item for ${kernel.id}`);
    const prog = execReport.stack[0];
    assert.equal(prog.type, 'Program', `expected Program on stack for ${kernel.id}`);
    assert.equal(prog.interface.stack_in, '[I64]', `unexpected stack_in for ${kernel.id}`);
    assert.equal(prog.interface.stack_out, '[I64]', `unexpected stack_out for ${kernel.id}`);
    assert.deepEqual(prog.interface.effects, [], `unexpected effects for ${kernel.id}`);

    const watPath = path.join(emitDir, 'engine/module-1.wat');
    const wasmPath = path.join(emitDir, 'engine/module-1.wasm');
    const watText = fs.readFileSync(watPath, 'utf8');
    const harness = buildHarnessModules(
      watText,
      dispatchWrap,
      path.join(outDir, 'raw/harness'),
      `dispatch-${kernel.id}`,
      wasmTools,
      wasmOpt,
    );

    compiledKernels.push({ kernel, watText, harness });
    emittedDigests.push({
      id: kernel.id,
      source: kernel.source,
      expected_w: kernel.expected_w,
      recipe_length: Array.isArray(prog.recipe) ? prog.recipe.length : null,
      module_wat_sha256: sha(watText),
      module_wasm_sha256: hashFile(wasmPath),
      harness_wat_sha256: harness.wat_sha256,
      harness_off_wasm_sha256: harness.off.sha256,
      harness_off_wasm_bytes: harness.off.bytes,
      harness_on_wasm_sha256: harness.on.sha256,
      harness_on_wasm_bytes: harness.on.bytes,
    });
  }

  const w2Entry = compiledKernels.find(e => e.kernel.id === 'w2_add');
  const w64Entry = compiledKernels.find(e => e.kernel.id === 'w64_arith');
  const helperHarness = buildHarnessModules(
    w2Entry.watText,
    helperWrap,
    path.join(outDir, 'raw/harness'),
    'helpers',
    wasmTools,
    wasmOpt,
  );

  // Run all 4 verification suites.
  const boundaryReport = runBoundaryChecks(compiledKernels);
  const ceilingReport = runCeilingControls(
    helperHarness.off.module,
    w2Entry.harness.off.module,
    w64Entry.harness.off.module,
  );
  const helpers = measureHelpers(helperHarness);
  const kernels = measureKernels(compiledKernels);
  const costModel = buildCostModel(helpers.summary, kernels.summary);

  noble.after_sha256 = hashFile(noble.path);
  assert.equal(noble.before_sha256, noble.after_sha256, 'noble binary changed during run');

  const rawFiles = [];
  const saveRawJson = (name, data) => {
    const rel = path.join('raw', `${name}.json`);
    const abs = path.join(outDir, rel);
    const text = `${JSON.stringify(data, null, 2)}\n`;
    fs.writeFileSync(abs, text);
    rawFiles.push({ name, file: rel, sha256: sha(text), bytes: Buffer.byteLength(text) });
  };
  saveRawJson('emitted-digests', {
    kernels: emittedDigests,
    helpers: {
      wat_sha256: helperHarness.wat_sha256,
      off_wasm_sha256: helperHarness.off.sha256,
      off_wasm_bytes: helperHarness.off.bytes,
      on_wasm_sha256: helperHarness.on.sha256,
      on_wasm_bytes: helperHarness.on.bytes,
    },
  });
  saveRawJson('boundary-oracle', boundaryReport);
  saveRawJson('ceilings', ceilingReport);
  saveRawJson('helpers-samples', helpers.rawSamples);
  saveRawJson('kernels-samples', kernels.rawSamples);

  const cpus = os.cpus();
  const receipt = {
    schema_version: 1,
    lane: 'gpu-cpu-cost-study',
    change: 'gpu-offload-research',
    owner_decisions: ['2026-11-20/2', '2026-11-20/3'],
    started_at: startedAt,
    finished_at: new Date().toISOString(),
    source,
    tools: {
      node: nodeTool,
      noble,
      wasm_tools: wasmTools,
      wasm_opt: { ...wasmOpt, flags: config.optimizer_flags },
    },
    host: {
      platform: os.platform(),
      arch: os.arch(),
      release: os.release(),
      cpu_count: cpus.length,
      cpu_model: cpus[0]?.model ?? 'unknown',
    },
    methodology: {
      clock: 'process.hrtime.bigint',
      warmup_instances: WARMUP_INSTANCES,
      samples: SAMPLES,
      helper_counts: HELPER_COUNTS,
      dispatch_counts: DISPATCH_COUNTS,
      map_counts: MAP_COUNTS,
      optimizations: OPTIMIZATIONS,
      oracle: 'independent mathematical K-NUM-01 model ((z + 2^63) mod 2^64) - 2^63 over arbitrary-precision BigInt',
    },
    verdicts: {
      all_passed: true,
      oracle_boundaries_passed: boundaryReport.every(r => r.passed),
      oracle_boundary_evaluations: boundaryReport.reduce((acc, r) => acc + r.boundary_inputs_checked, 0),
      ceiling_controls_passed: Object.values(ceilingReport).every(r => r.passed || r.refused_as_expected),
      helper_samples_verified: OPTIMIZATIONS.every(opt => HELPER_COUNTS.every(n => helpers.summary[opt][String(n)].all_verified)),
      kernel_samples_verified: OPTIMIZATIONS.every(opt => KERNELS.every(k =>
        DISPATCH_COUNTS.every(n => kernels.summary[opt][k.id].dispatch[String(n)].all_verified)
        && MAP_COUNTS.every(n => kernels.summary[opt][k.id].cpu_map[String(n)].all_verified))),
    },
    ceilings: ceilingReport,
    helpers: helpers.summary,
    kernels: kernels.summary,
    cost_model: costModel,
    raw_files: rawFiles,
    non_claims: [
      'CPU-side measurements on the selected Node/V8 Wasm route only; no GPU hardware, driver, SPIR-V, WGSL or PTX execution occurred.',
      'Crossover n* rows under parametric fixed overheads (50-500 us) are optimistic lower bounds assuming zero device kernel time and infinite PCIe/memory bandwidth; real GPU overheads can only raise n*.',
      'Does not implement batch.map, does not add a packed array type, and promotes no conformance case or proof obligation.',
    ],
  };

  const receiptPath = path.join(outDir, 'receipt.json');
  const receiptText = `${JSON.stringify(receipt, null, 2)}\n`;
  fs.writeFileSync(receiptPath, receiptText);
  const receiptSha = sha(receiptText);

  console.log(JSON.stringify({
    status: 'passed',
    receipt: receiptPath,
    receipt_sha256: receiptSha,
    source_revision: source.source_revision,
    dirty: source.dirty,
    verdicts: receipt.verdicts,
    helpers_n682_off: costModel.off.helpers_median_ns_per_el,
    helpers_n682_on: costModel.on.helpers_median_ns_per_el,
  }, null, 2));
}

main();
