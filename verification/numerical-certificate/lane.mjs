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

// ---------- deterministic generator ----------
const MASK = (1n << 64n) - 1n;
let state = SEED;
function next() {
  state = (state + 0x9e3779b97f4a7c15n) & MASK;
  let z = state;
  z = ((z ^ (z >> 30n)) * 0xbf58476d1ce4e5b9n) & MASK;
  z = ((z ^ (z >> 27n)) * 0x94d049bb133111ebn) & MASK;
  return z ^ (z >> 31n);
}
const below = (n) => Number(next() % BigInt(n));
const fraction = () => Number(next() >> 11n) / 2 ** 53;

// ---------- exact rationals ----------
const gcd = (a, b) => {
  let x = a < 0n ? -a : a;
  let y = b < 0n ? -b : b;
  while (y) [x, y] = [y, x % y];
  return x;
};
const rat = (n, d = 1n) => {
  if (d === 0n) throw new Error('zero denominator');
  if (d < 0n) [n, d] = [-n, -d];
  const g = gcd(n, d) || 1n;
  return { n: n / g, d: d / g };
};
const add = (a, b) => rat(a.n * b.d + b.n * a.d, a.d * b.d);
const sub = (a, b) => rat(a.n * b.d - b.n * a.d, a.d * b.d);
const mul = (a, b) => rat(a.n * b.n, a.d * b.d);
const neg = (a) => ({ n: -a.n, d: a.d });
const abs = (a) => (a.n < 0n ? neg(a) : a);
const cmp = (a, b) => {
  const left = a.n * b.d;
  const right = b.n * a.d;
  return left < right ? -1 : left > right ? 1 : 0;
};
const max = (a, b) => (cmp(a, b) >= 0 ? a : b);
const min = (a, b) => (cmp(a, b) <= 0 ? a : b);
const pow2 = (k) => (k >= 0 ? rat(1n << BigInt(k)) : rat(1n, 1n << BigInt(-k)));
const toNumber = (a) => {
  // Scaled division keeps precision for tiny and huge values.
  const shift = BigInt(Math.max(0, a.d.toString(2).length - a.n.toString(2).length + 64));
  return Number((a.n << shift) / a.d) / 2 ** Number(shift);
};
const ZERO = rat(0n);
const UNIT = pow2(-24);
const ETA = pow2(-150);
const LIMIT = pow2(127);

// ---------- binary32 encodings ----------
const view = new DataView(new ArrayBuffer(4));
const floatOf = (bits) => {
  view.setUint32(0, bits);
  return view.getFloat32(0);
};
const bitsOf = (x) => {
  view.setFloat32(0, x);
  return view.getUint32(0);
};
function exactValue(bits) {
  const exponent = (bits >>> 23) & 0xff;
  const fractionBits = bits & 0x7fffff;
  if (exponent === 255) throw new Error(`non-finite bits ${bits.toString(16)}`);
  const [m, e] = exponent === 0 ? [fractionBits, -149] : [fractionBits + 0x800000, exponent - 150];
  const v = mul(rat(BigInt(m)), pow2(e));
  return bits >>> 31 ? neg(v) : v;
}

// Domains as binary32 endpoints: ordinary, signed, cancelling, subnormal, large and decimal values.
const DOMAINS = [
  [1, 2], [-3, 5], [-1, 1], [2 ** -130, 2 ** -120], [2 ** 60, 2 ** 62], [Math.fround(0.1), Math.fround(0.3)],
].map(([lo, hi]) => ({ lo: bitsOf(Math.fround(lo)), hi: bitsOf(Math.fround(hi)) }));
const CONSTANTS = [1.5, -1, Math.fround(0.1), 3, -2.5, 2 ** -140, 2 ** 40, -0.375].map((x) => bitsOf(Math.fround(x)));

function generate() {
  const inputs = 1 + below(3);
  const domains = Array.from({ length: inputs }, () => DOMAINS[below(DOMAINS.length)]);
  const nodes = Array.from({ length: inputs }, (_, index) => ({ kind: 'input', index }));
  const operations = 2 + below(8);
  for (let k = 0; k < operations; k++) {
    if (below(4) === 0) nodes.push({ kind: 'const', bits: CONSTANTS[below(CONSTANTS.length)] });
    const pick = () => (below(4) === 0 ? below(nodes.length) : nodes.length - 1 - below(Math.min(3, nodes.length)));
    nodes.push({ kind: below(2) ? 'add' : 'mul', left: pick(), right: pick() });
  }
  return { domains, nodes, output: nodes.length - 1 };
}

// ---------- the untrusted analyzer: the checker's rules, in exact rationals ----------
const magnitudeOf = (lo, hi) => max(abs(lo), abs(hi));
function analyze(graph) {
  const bounds = [];
  let reachMax = ZERO;
  for (const node of graph.nodes) {
    if (node.kind === 'input') {
      const domain = graph.domains[node.index];
      bounds.push({ lo: exactValue(domain.lo), hi: exactValue(domain.hi), error: ZERO });
    } else if (node.kind === 'const') {
      const v = exactValue(node.bits);
      bounds.push({ lo: v, hi: v, error: ZERO });
    } else {
      const a = bounds[node.left];
      const b = bounds[node.right];
      if (node.kind === 'add') {
        const reach = magnitudeOf(add(sub(a.lo, a.error), sub(b.lo, b.error)), add(add(a.hi, a.error), add(b.hi, b.error)));
        reachMax = max(reachMax, reach);
        bounds.push({
          lo: add(a.lo, b.lo), hi: add(a.hi, b.hi),
          error: add(add(add(a.error, b.error), mul(UNIT, reach)), ETA), reach,
        });
      } else {
        const products = [mul(a.lo, b.lo), mul(a.lo, b.hi), mul(a.hi, b.lo), mul(a.hi, b.hi)];
        const ma = magnitudeOf(a.lo, a.hi);
        const mb = magnitudeOf(b.lo, b.hi);
        const reach = mul(add(ma, a.error), add(mb, b.error));
        reachMax = max(reachMax, reach);
        bounds.push({
          lo: products.reduce(min), hi: products.reduce(max),
          error: add(add(add(mul(a.error, add(mb, b.error)), mul(ma, b.error)), mul(UNIT, reach)), ETA), reach,
        });
      }
    }
  }
  return { bounds, certifiable: cmp(reachMax, LIMIT) < 0 };
}

// ---------- the Lean corpus ----------
const leanRat = (q) => (q.d === 1n ? `(${q.n} : ℚ)` : `((${q.n} : ℚ) / ${q.d})`);
const hex32 = (bits) => `0x${bits.toString(16).toUpperCase().padStart(8, '0')}`;
const leanNode = (node) => {
  if (node.kind === 'input') return `.input ${node.index}`;
  if (node.kind === 'const') return `.const ${hex32(node.bits)}`;
  return `.${node.kind} ${node.left} ${node.right}`;
};
function leanCorpus(corpus, budgetBits) {
  const lines = [
    '/-',
    '  Generated by verification/numerical-certificate/lane.mjs (issue #44); do not edit.',
    '  Every certificate is the untrusted analyzer\'s proposal. The kernel accepts each one and refuses',
    '  its weakened copy, whose output error bound is halved.',
    '-/',
    'import NobleFoundation.Binary32Checker',
    'open NobleFoundation.Binary32.Checker',
    'namespace NobleCertificateCorpus',
    '',
    `def budget : Budget := { nodes := 64, bits := ${budgetBits} }`,
    '',
    'def halveAt : Nat → List NodeBound → List NodeBound',
    '  | _, [] => []',
    '  | 0, bound :: rest => { bound with error := bound.error / 2 } :: rest',
    '  | k + 1, bound :: rest => bound :: halveAt k rest',
    '',
  ];
  corpus.forEach(({ graph, analysis }, index) => {
    lines.push(`def graph${index} : Graph :=`);
    lines.push(`  { nodes := [${graph.nodes.map(leanNode).join(', ')}], outputs := [${graph.output}] }`);
    lines.push(`def certificate${index} : Certificate :=`);
    lines.push(`  { domains := [${graph.domains.map((d) => `{ lo := ${leanRat(exactValue(d.lo))}, hi := ${leanRat(exactValue(d.hi))}, inputError := 0 }`).join(',\n      ')}],`);
    lines.push(`    bounds := [${analysis.bounds.map((b) => `{ lo := ${leanRat(b.lo)}, hi := ${leanRat(b.hi)}, error := ${leanRat(b.error)} }`).join(',\n      ')}] }`);
    lines.push('set_option maxRecDepth 100000 in');
    lines.push(`theorem certified${index} : check budget graph${index} certificate${index} = .certified := by`);
    lines.push('  decide +kernel');
    lines.push('set_option maxRecDepth 100000 in');
    lines.push(`theorem weakened${index} : check budget graph${index}`);
    lines.push(`    { certificate${index} with bounds := halveAt ${graph.output} certificate${index}.bounds } = .unknown ${graph.output} := by`);
    lines.push('  decide +kernel');
    lines.push('');
  });
  lines.push('end NobleCertificateCorpus', '');
  return lines.join('\n');
}

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
    const x = Math.fround(lo + (hi - lo) * fraction());
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
const corpus = [];
let discarded = 0;
while (corpus.length < GRAPHS) {
  const graph = generate();
  const analysis = analyze(graph);
  if (analysis.certifiable) corpus.push({ graph, analysis });
  else discarded++;
}
const bitLength = (q) => Math.max((q.n < 0n ? -q.n : q.n).toString(2).length, q.d.toString(2).length);
const maxBits = Math.max(...corpus.flatMap(({ analysis }) => analysis.bounds.flatMap((b) => [b.lo, b.hi, b.error].map(bitLength))));
const budgetBits = 64 * Math.ceil((maxBits + 1) / 64);
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
    'Bound quality is measured for the checker\'s own rules on this corpus only; no comparison with an external analyzer such as Daisy or FPTaylor is made.',
  ],
};
fs.writeFileSync(output, `${JSON.stringify(receipt, null, 2)}\n`);
console.log(JSON.stringify({ holds, checks, graphs: corpus.length, discarded, theorems, per_check_ms: receipt.checker_cost.per_kernel_check_ms, ratios: receipt.sampling.bound_over_observed, violations: totalViolations }));
process.exitCode = holds ? 0 : 1;
