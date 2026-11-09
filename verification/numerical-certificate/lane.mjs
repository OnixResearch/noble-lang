#!/usr/bin/env node
// Issue #44 corpus lane for the binary32 roundoff certificate checker (Binary32Checker.lean).
//
// 1. A SplitMix64 corpus of add/multiply graphs over binary32 inputs and constants, with input domains
//    chosen for cancellation, subnormal and large magnitudes.
// 2. An untrusted analyzer, here in exact BigInt rationals, proposes for every node the tightest
//    certificate the checker's rules admit: the interval enclosure and the error bound.
// 3. The Lean kernel checks every certificate, and a weakened copy of each, with `decide +kernel`.
//    The analyzer is never the proof: a wrong certificate is refused there.
// 4. Secondary evidence only: independent expected values. On sampled inputs, the host's binary32
//    arithmetic (binary64 then Math.fround) and exact rational arithmetic evaluate every node. Each
//    machine value must be finite, each real value inside its enclosure, and each observed error
//    within its certified bound; the lane also measures bound against observed error. Neither
//    computation reuses the analyzer's interval arithmetic.
import { spawnSync } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { ZERO, ETA, buildCorpus, budgetOf, cmp, floatOf, leanCorpus, max, mul, rat, splitMix64, toNumber } from './corpus.mjs';

const argv = process.argv.slice(2);
const value = (name) => {
  const index = argv.indexOf(name);
  if (index < 0 || index + 1 >= argv.length) throw new Error(`missing ${name}`);
  return argv[index + 1];
};
const leanBin = path.resolve(value('--lean-bin'));
const foundation = path.resolve(value('--foundation'));
const corpusPath = path.resolve(value('--corpus'));
const output = path.resolve(value('--output'));
const GRAPHS = Number(value('--graphs'));
const SAMPLES = Number(value('--samples'));
const SEED = BigInt(value('--seed'));
const sha256 = (data) => crypto.createHash('sha256').update(data).digest('hex');

// ---------- independent expected values ----------
// Expected values never touch the analyzer's float32 decoder or rational arithmetic: inputs, constants
// and machine results are decoded from their binary64 bits, and the real graph is evaluated in dyadic
// (mantissa, exponent) arithmetic, which is exact because sums and products of dyadics are dyadic.
const view64 = new DataView(new ArrayBuffer(8));
function dyadicOfNumber(x) {
  view64.setFloat64(0, x);
  const high = view64.getUint32(0);
  const exponent = (high >>> 20) & 0x7ff;
  let mantissa = (BigInt(high & 0xfffff) << 32n) | BigInt(view64.getUint32(4));
  if (exponent === 0x7ff) throw new Error('non-finite expected value');
  if (exponent !== 0) mantissa |= 1n << 52n;
  return { m: high >>> 31 ? -mantissa : mantissa, e: exponent === 0 ? -1074 : exponent - 1075 };
}
const dyadicAdd = (a, b) => {
  const e = Math.min(a.e, b.e);
  return { m: (a.m << BigInt(a.e - e)) + (b.m << BigInt(b.e - e)), e };
};
const dyadicMul = (a, b) => ({ m: a.m * b.m, e: a.e + b.e });
const dyadicNeg = (a) => ({ m: -a.m, e: a.e });
const dyadicAbs = (a) => ({ m: a.m < 0n ? -a.m : a.m, e: a.e });
// Only the comparison with a certified bound converts to the analyzer's rational form.
const dyadicToRat = (a) => (a.e >= 0 ? rat(a.m << BigInt(a.e)) : rat(a.m, 1n << BigInt(-a.e)));
function sampleInputs(graph, sampleIndex) {
  return graph.domains.map((domain) => {
    const lo = floatOf(domain.lo);
    const hi = floatOf(domain.hi);
    if (sampleIndex === 0) return lo;
    if (sampleIndex === 1) return hi;
    const x = Math.fround(lo + (hi - lo) * stream.fraction());
    return x < lo ? lo : x > hi ? hi : x;
  });
}
function sample(graph, analysis, inputs) {
  const machine = [];
  const real = [];
  let violations = 0;
  const outputErrors = [];
  graph.nodes.forEach((node, index) => {
    if (node.kind === 'input') {
      machine.push(inputs[node.index]);
      real.push(dyadicOfNumber(inputs[node.index]));
    } else if (node.kind === 'const') {
      machine.push(floatOf(node.bits));
      real.push(dyadicOfNumber(floatOf(node.bits)));
    } else if (node.kind === 'add') {
      machine.push(Math.fround(machine[node.left] + machine[node.right]));
      real.push(dyadicAdd(real[node.left], real[node.right]));
    } else {
      machine.push(Math.fround(machine[node.left] * machine[node.right]));
      real.push(dyadicMul(real[node.left], real[node.right]));
    }
    const bound = analysis.bounds[index];
    if (!Number.isFinite(machine[index])) {
      violations++;
      return;
    }
    const error = dyadicToRat(dyadicAbs(dyadicAdd(dyadicOfNumber(machine[index]), dyadicNeg(real[index]))));
    const exact = dyadicToRat(real[index]);
    if (cmp(exact, bound.lo) < 0 || cmp(exact, bound.hi) > 0 || cmp(error, bound.error) > 0) violations++;
    if (index === graph.output) outputErrors.push(error);
  });
  return { violations, outputError: outputErrors[0] };
}

// ---------- run ----------
const stream = splitMix64(SEED);
const { corpus, discarded } = buildCorpus(stream, GRAPHS);
const { maxBits, budgetBits } = budgetOf(corpus);
const leanText = leanCorpus(corpus, budgetBits);
fs.mkdirSync(path.dirname(corpusPath), { recursive: true });
fs.writeFileSync(corpusPath, leanText);
const baselinePath = path.join(path.dirname(output), 'baseline.lean.tmp');
fs.mkdirSync(path.dirname(output), { recursive: true });
fs.writeFileSync(baselinePath, 'import NobleFoundation.Binary32Checker\n');

const env = { HOME: process.env.HOME ?? '/', LANG: 'C.UTF-8', PATH: [leanBin, '/run/current-system/sw/bin', '/usr/bin', '/bin'].join(':') };
function timedLean(file) {
  const started = process.hrtime.bigint();
  const result = spawnSync(path.join(leanBin, 'lake'), ['env', 'lean', file], {
    cwd: foundation, env, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024, timeout: 3_600_000, killSignal: 'SIGKILL',
  });
  return {
    exit_code: result.status,
    elapsed_ms: Number((process.hrtime.bigint() - started) / 1_000_000n),
    output_excerpt: `${result.stdout}${result.stderr}`.trim().slice(0, 2000),
  };
}
const warmup = { baseline: timedLean(baselinePath), corpus: timedLean(corpusPath) };
const runs = [];
for (let round = 0; round < 3; round++) runs.push({ baseline: timedLean(baselinePath), corpus: timedLean(corpusPath) });
fs.rmSync(baselinePath);
const quantile = (values, q) => [...values].sort((a, b) => a - b)[Math.floor(q * (values.length - 1))];
const median = (values) => [...values].sort((a, b) => a - b)[Math.floor(values.length / 2)];
const leanAccepted = [warmup, ...runs].every((run) => run.corpus.exit_code === 0 && run.corpus.output_excerpt === '' && run.baseline.exit_code === 0);
const theorems = 2 * corpus.length;
const corpusMedian = median(runs.map((run) => run.corpus.elapsed_ms));
const baselineMedian = median(runs.map((run) => run.baseline.elapsed_ms));

const perGraph = corpus.map(({ graph, analysis }, index) => {
  let violations = 0;
  let worst = ZERO;
  let exactSamples = 0;
  for (let s = 0; s < SAMPLES; s++) {
    const result = sample(graph, analysis, sampleInputs(graph, s));
    violations += result.violations;
    worst = max(worst, result.outputError);
    if (result.outputError.n === 0n) exactSamples++;
  }
  const bound = analysis.bounds[graph.output].error;
  return {
    graph: index,
    nodes: graph.nodes.length,
    operations: graph.nodes.filter((node) => node.kind === 'add' || node.kind === 'mul').length,
    inputs: graph.domains.map((d) => [floatOf(d.lo), floatOf(d.hi)]),
    output_bound: toNumber(bound),
    worst_observed_output_error: toNumber(worst),
    bound_over_observed: worst.n === 0n ? null : toNumber(bound) / toNumber(worst),
    bound_at_most_two_eta_per_node: cmp(mul(rat(BigInt(graph.nodes.length)), ETA), mul(rat(1n, 2n), bound)) >= 0,
    samples_with_exact_output: exactSamples,
    violations,
  };
});
const ratios = perGraph.map((row) => row.bound_over_observed).filter((ratio) => ratio !== null);
const totalViolations = perGraph.reduce((sum, row) => sum + row.violations, 0);
const checks = {
  lean_accepts_every_certificate_and_refuses_every_weakened_copy: leanAccepted,
  no_sampled_violation: totalViolations === 0,
};
const holds = Object.values(checks).every(Boolean);
const host = spawnSync('uname', ['-srm'], { encoding: 'utf8' }).stdout.trim();
const receipt = {
  schema: 'noble-numerical-certificate-lane/v1',
  measured_on: new Date().toISOString().slice(0, 10),
  host,
  node: process.version,
  assertion: 'every analyzer certificate in the pinned corpus is accepted by the Lean kernel and its weakened copy refused; on sampled inputs, host binary32 values and exact rational values stay inside every certified enclosure and error bound',
  holds,
  checks,
  harness: { path: 'verification/numerical-certificate/lane.mjs', sha256: sha256(fs.readFileSync(fileURLToPath(import.meta.url))) },
  corpus: {
    path: 'verification/numerical-certificate/corpus.lean', sha256: sha256(leanText), seed: SEED.toString(),
    graphs: corpus.length, discarded_for_overflow_reach: discarded, theorems,
    nodes_total: corpus.reduce((sum, { graph }) => sum + graph.nodes.length, 0),
    largest_rational_bits: maxBits, budget_bits: budgetBits,
  },
  checker_cost: {
    warmup_ms: { baseline: warmup.baseline.elapsed_ms, corpus: warmup.corpus.elapsed_ms },
    runs, corpus_median_ms: corpusMedian, import_only_median_ms: baselineMedian,
    per_kernel_check_ms: (corpusMedian - baselineMedian) / theorems,
    note: 'one discarded warm-up run each, then medians of three runs; per-check cost is (corpus - import only) / theorems, and includes elaboration of the generated definitions',
  },
  sampling: {
    role: 'secondary evidence only; the proof is the kernel-checked certificate',
    machine: 'host binary32: binary64 arithmetic, then Math.fround. Products of binary32 values are exact in binary64; sums double-round innocuously because 53 >= 2 * 24 + 1',
    expected: 'exact dyadic (BigInt mantissa, exponent) arithmetic over values decoded from their binary64 bits; it shares neither the analyzer\'s float32 decoder nor its rational interval arithmetic, and converts to rationals only to compare with a certified bound',
    samples_per_graph: SAMPLES, first_two_samples: 'all inputs at their lower, then upper, endpoints',
    node_checks: SAMPLES * corpus.reduce((sum, { graph }) => sum + graph.nodes.length, 0),
    violations: totalViolations,
    bound_over_observed: {
      graphs_with_nonzero_observed_error: ratios.length,
      min: Math.min(...ratios), lower_quartile: quantile(ratios, 0.25), median: median(ratios),
      upper_quartile: quantile(ratios, 0.75), max: Math.max(...ratios),
      graphs_with_bound_at_most_two_eta_per_node: perGraph.filter((row) => row.bound_at_most_two_eta_per_node).length,
    },
    per_graph: perGraph,
  },
  non_claims: [
    'Sampling is secondary evidence: a sound certificate is established by the kernel check, not by the absence of sampled violations.',
    'The machine values sampled here are the host\'s binary32 arithmetic, not the Lean model; their agreement with the model is not claimed by this lane.',
    'The corpus is Lean model graphs, not encoded noble-ngraph graphs; no Rust or Wasm evaluator is exercised.',
    'Bound quality is measured for the checker\'s own rules on this corpus only; the external-analyzer comparison is gappa.mjs.',
  ],
};
fs.writeFileSync(output, `${JSON.stringify(receipt, null, 2)}\n`);
console.log(JSON.stringify({ holds, checks, graphs: corpus.length, discarded, theorems, per_check_ms: receipt.checker_cost.per_kernel_check_ms, ratios: receipt.sampling.bound_over_observed, violations: totalViolations }));
process.exitCode = holds ? 0 : 1;
