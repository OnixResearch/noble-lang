#!/usr/bin/env node
// Issue #44 (NGRAPH-ERR-05): compare the checker's certified output bounds with an external, untrusted
// analyzer, Gappa, on the pinned seed-44 corpus. Secondary evidence only: Gappa's enclosures are not
// checked by the kernel here, and nothing below adds to what the kernel check proves.
//
// 1. Corpus. `corpus.mjs` regenerates the 24 graphs, and the lane requires them to rebuild
//    `corpus.lean` byte for byte, so the comparison is about exactly the kernel-checked corpus.
// 2. Gappa. For every graph it writes a script with binary32 round-to-nearest-even arithmetic
//    (`float<ieee_32, ne>`). Each input is `rnd(X)` with `X` in the domain, so it ranges over exactly
//    the binary32 values the certificate admits. The machine chain rounds every operation, and the
//    real chain computes exactly from the same inputs. Gappa encloses the output's machine-minus-real
//    error, with no hints.
// 3. Comparison. Certified output bound over Gappa's bound, per graph; Gappa's run time against the
//    kernel's per-check cost from the corpus lane's receipt. As a cross-check of the modelling, every
//    Gappa bound must cover the worst output error the corpus lane observed.
//
// Usage: node verification/numerical-certificate/gappa.mjs --gappa <gappa binary> --output <new file>
import { spawnSync } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { buildCorpus, budgetOf, leanCorpus, splitMix64, toNumber } from './corpus.mjs';

const argv = process.argv.slice(2);
const value = (name) => {
  const index = argv.indexOf(name);
  if (index < 0 || index + 1 >= argv.length) throw new Error(`missing ${name}`);
  return argv[index + 1];
};
const gappa = path.resolve(value('--gappa'));
const output = path.resolve(value('--output'));
const root = path.resolve(fileURLToPath(import.meta.url), '../../..');
const here = 'verification/numerical-certificate';
const sha256 = (data) => crypto.createHash('sha256').update(data).digest('hex');
const requireThat = (condition, message) => { if (!condition) throw new Error(message); };
const run = (executable, args, options = {}) => spawnSync(executable, args, { encoding: 'utf8', maxBuffer: 16 * 1024 * 1024, ...options });

const SEED = 44n;
const GRAPHS = 24;
const RUNS = 3;

// ---------- source and corpus identity ----------
const git = (args) => run('git', args, { cwd: root }).stdout.trim();
const source = { commit: git(['rev-parse', 'HEAD']), tree: git(['rev-parse', 'HEAD^{tree}']), status: git(['status', '--porcelain=v1', '--untracked-files=all']) };
requireThat(source.status === '', `the source worktree is dirty:\n${source.status}`);
requireThat(!output.startsWith(`${root}${path.sep}`), 'the output must be outside the worktree');
const { corpus, discarded } = buildCorpus(splitMix64(SEED), GRAPHS);
const corpusText = leanCorpus(corpus, budgetOf(corpus).budgetBits);
const committed = fs.readFileSync(path.join(root, here, 'corpus.lean'), 'utf8');
requireThat(corpusText === committed, 'the regenerated corpus differs from the committed corpus.lean');
const laneReceipt = JSON.parse(fs.readFileSync(path.join(root, here, 'receipts/2026-10-30/lane.json'), 'utf8'));
requireThat(laneReceipt.corpus.sha256 === sha256(committed), 'the corpus lane receipt is for another corpus');

// ---------- Gappa scripts ----------
// A binary32 value as a Gappa dyadic literal, `mantissa b exponent`.
function dyadic(bits) {
  const exponent = (bits >>> 23) & 0xff;
  const fraction = bits & 0x7fffff;
  requireThat(exponent !== 255, `non-finite bits ${bits.toString(16)}`);
  const [m, e] = exponent === 0 ? [fraction, -149] : [fraction + 0x800000, exponent - 150];
  return `${bits >>> 31 ? '-' : ''}${m}b${e}`;
}
function script(graph) {
  const lines = ['@rnd = float<ieee_32, ne>;'];
  graph.domains.forEach((_, k) => lines.push(`x${k} = rnd(X${k});`));
  // An input or a constant is the same term on both chains; an operation has a rounded machine
  // name and an exact real name.
  const term = (i, chain) => {
    const node = graph.nodes[i];
    if (node.kind === 'input') return `x${node.index}`;
    if (node.kind === 'const') return `(${dyadic(node.bits)})`;
    return `${chain}${i}`;
  };
  graph.nodes.forEach((node, i) => {
    if (node.kind !== 'add' && node.kind !== 'mul') return;
    const op = node.kind === 'add' ? '+' : '*';
    lines.push(`m${i} rnd= ${term(node.left, 'm')} ${op} ${term(node.right, 'm')};`,
      `e${i} = ${term(node.left, 'e')} ${op} ${term(node.right, 'e')};`);
  });
  requireThat(['add', 'mul'].includes(graph.nodes[graph.output].kind), 'the output is an operation');
  const hypotheses = graph.domains.map((d, k) => `X${k} in [${dyadic(d.lo)}, ${dyadic(d.hi)}]`).join(' /\\ ');
  lines.push(`{ ${hypotheses} -> m${graph.output} - e${graph.output} in ? }`, '');
  return lines.join('\n');
}

// A Gappa result number (`-12`, `5b-24`) as an exact rational { n, d }.
function parseNumber(text) {
  const match = /^(-?\d+)(?:b(-?\d+))?$/.exec(text);
  requireThat(match, `unexpected Gappa number ${text}`);
  const m = BigInt(match[1]);
  const e = match[2] === undefined ? 0 : Number(match[2]);
  return e >= 0 ? { n: m << BigInt(e), d: 1n } : { n: m, d: 1n << BigInt(-e) };
}
const absRat = (q) => (q.n < 0n ? { n: -q.n, d: q.d } : q);
const larger = (a, b) => (a.n * b.d >= b.n * a.d ? a : b);
const ratText = (q) => (q.d === 1n ? `${q.n}` : `${q.n}/${q.d}`);

function analyze(graph, index, scratch) {
  const file = path.join(scratch, `graph${index}.g`);
  fs.writeFileSync(file, script(graph));
  const times = [];
  let result;
  for (let round = 0; round < RUNS; round++) {
    const started = process.hrtime.bigint();
    result = run(gappa, [file]);
    times.push(Number(process.hrtime.bigint() - started) / 1e6);
  }
  const text = `${result.stdout}${result.stderr}`;
  // An endpoint is `mantissa b exponent`, followed by a decimal annotation unless it is exactly zero.
  const match = new RegExp(`m${graph.output} - e${graph.output} in \\[(-?\\d+(?:b-?\\d+)?)(?: \\{[^}]*\\})?, (-?\\d+(?:b-?\\d+)?)(?: \\{[^}]*\\})?\\]`).exec(text);
  return { exit: result.status, text: text.trim().slice(0, 1000), match, times: times.sort((a, b) => a - b) };
}

// ---------- run ----------
const version = run(gappa, ['--version']).stdout.trim();
const scratch = fs.mkdtempSync(path.join(os.tmpdir(), 'noble-gappa-'));
const perGraph = corpus.map(({ graph, analysis }, index) => {
  const certified = analysis.bounds[graph.output].error;
  const gappaRun = analyze(graph, index, scratch);
  const observed = laneReceipt.sampling.per_graph[index].worst_observed_output_error;
  const row = {
    graph: index,
    nodes: graph.nodes.length,
    operations: graph.nodes.filter((node) => node.kind === 'add' || node.kind === 'mul').length,
    certified_output_bound: toNumber(certified),
    gappa_exit: gappaRun.exit,
    gappa_median_ms: gappaRun.times[Math.floor(RUNS / 2)],
    worst_observed_output_error: observed,
  };
  if (gappaRun.exit !== 0 || !gappaRun.match) return { ...row, gappa_bounded: false, gappa_output: gappaRun.text };
  const lo = parseNumber(gappaRun.match[1]);
  const hi = parseNumber(gappaRun.match[2]);
  const bound = larger(absRat(lo), absRat(hi));
  const gappaBound = toNumber(bound);
  return {
    ...row,
    gappa_bounded: true,
    gappa_enclosure: [ratText(lo), ratText(hi)],
    gappa_output_bound: gappaBound,
    certified_over_gappa: gappaBound === 0 ? null : row.certified_output_bound / gappaBound,
    gappa_covers_observed: gappaBound >= observed,
  };
});
fs.rmSync(scratch, { recursive: true });

const bounded = perGraph.filter((row) => row.gappa_bounded);
const ratios = bounded.map((row) => row.certified_over_gappa).filter((ratio) => ratio !== null).sort((a, b) => a - b);
const quantile = (q) => ratios[Math.floor(q * (ratios.length - 1))];
const gappaTimes = perGraph.map((row) => row.gappa_median_ms).sort((a, b) => a - b);
const checks = {
  corpus_is_the_kernel_checked_corpus: true,
  gappa_bounds_every_graph: bounded.length === corpus.length,
  every_gappa_bound_covers_the_observed_error: bounded.every((row) => row.gappa_covers_observed),
};
const holds = Object.values(checks).every(Boolean);
const receipt = {
  schema: 'noble-numerical-certificate-gappa/v1',
  issue: 44,
  measured_on: new Date().toISOString().slice(0, 10),
  host: run('uname', ['-srm']).stdout.trim(),
  node: process.version,
  source_commit: source.commit, source_tree: source.tree, clean_source: true,
  holds, checks,
  harness: Object.fromEntries([`${here}/gappa.mjs`, `${here}/corpus.mjs`].map((file) => [file, sha256(fs.readFileSync(path.join(root, file)))])),
  analyzer: {
    name: 'Gappa', version, binary_sha256: sha256(fs.readFileSync(gappa)),
    source: 'nixpkgs as locked by the repository flake (nixpkgs follows octet/nixpkgs)',
    model: 'float<ieee_32, ne> on every operation; inputs rnd(X) with X in the declared domain; no hints; the output\'s machine-minus-real error is enclosed',
  },
  corpus: {
    path: `${here}/corpus.lean`, sha256: sha256(committed), seed: SEED.toString(), graphs: corpus.length,
    discarded_for_overflow_reach: discarded,
  },
  comparison: {
    graphs_bounded_by_gappa: bounded.length,
    certified_over_gappa: ratios.length === 0 ? null : {
      graphs: ratios.length, min: ratios[0], lower_quartile: quantile(0.25), median: quantile(0.5),
      upper_quartile: quantile(0.75), max: ratios[ratios.length - 1],
    },
    gappa_median_ms: gappaTimes[Math.floor(gappaTimes.length / 2)],
    gappa_max_ms: gappaTimes[gappaTimes.length - 1],
    kernel_per_check_ms: laneReceipt.checker_cost.per_kernel_check_ms,
    kernel_cost_source: `${here}/receipts/2026-10-30/lane.json`,
    per_graph: perGraph,
  },
  non_claims: [
    'Gappa is an untrusted analyzer here: its enclosures are not checked by the Lean kernel, and no Gappa-generated proof is used.',
    'The comparison is secondary evidence about bound quality and cost; a sound bound is established only by the kernel-checked certificate.',
    'Gappa\'s ieee_32 format has no overflow; the corpus already excludes graphs whose reach meets the 2^127 overflow test.',
    'Gappa run time includes process start-up; the kernel cost is the corpus lane\'s per-check figure, which includes elaboration. They measure different work.',
    'FPTaylor and Daisy are not packaged in the pinned nixpkgs and are not compared.',
  ],
};
fs.writeFileSync(output, `${JSON.stringify(receipt, null, 2)}\n`, { flag: 'wx' });
console.log(JSON.stringify({ holds, checks, bounded: bounded.length, ratios: receipt.comparison.certified_over_gappa, gappa_median_ms: receipt.comparison.gappa_median_ms, kernel_per_check_ms: receipt.comparison.kernel_per_check_ms }));
process.exitCode = holds ? 0 : 1;
