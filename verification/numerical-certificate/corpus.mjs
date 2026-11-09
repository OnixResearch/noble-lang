// The pinned roundoff certificate corpus of issue #44, shared by the corpus lane (`lane.mjs`) and the
// external-analyzer comparison (`gappa.mjs`):
// - the SplitMix64 generator of add/multiply graphs over binary32 inputs and constants;
// - exact BigInt rationals and binary32 decoding;
// - the untrusted analyzer, which proposes the tightest certificates the checker's rules admit;
// - the generated Lean corpus, so that a consumer can show it rebuilt exactly `corpus.lean`.

// ---------- deterministic generator ----------
const MASK = (1n << 64n) - 1n;

/** A SplitMix64 stream. The corpus is drawn from it first, and the lane's samples continue it. */
export function splitMix64(seed) {
  let state = seed;
  const next = () => {
    state = (state + 0x9e3779b97f4a7c15n) & MASK;
    let z = state;
    z = ((z ^ (z >> 30n)) * 0xbf58476d1ce4e5b9n) & MASK;
    z = ((z ^ (z >> 27n)) * 0x94d049bb133111ebn) & MASK;
    return z ^ (z >> 31n);
  };
  return {
    below: (n) => Number(next() % BigInt(n)),
    fraction: () => Number(next() >> 11n) / 2 ** 53,
  };
}

// ---------- exact rationals ----------
const gcd = (a, b) => {
  let x = a < 0n ? -a : a;
  let y = b < 0n ? -b : b;
  while (y) [x, y] = [y, x % y];
  return x;
};
export const rat = (n, d = 1n) => {
  if (d === 0n) throw new Error('zero denominator');
  if (d < 0n) [n, d] = [-n, -d];
  const g = gcd(n, d) || 1n;
  return { n: n / g, d: d / g };
};
export const add = (a, b) => rat(a.n * b.d + b.n * a.d, a.d * b.d);
export const sub = (a, b) => rat(a.n * b.d - b.n * a.d, a.d * b.d);
export const mul = (a, b) => rat(a.n * b.n, a.d * b.d);
export const neg = (a) => ({ n: -a.n, d: a.d });
export const abs = (a) => (a.n < 0n ? neg(a) : a);
export const cmp = (a, b) => {
  const left = a.n * b.d;
  const right = b.n * a.d;
  return left < right ? -1 : left > right ? 1 : 0;
};
export const max = (a, b) => (cmp(a, b) >= 0 ? a : b);
export const min = (a, b) => (cmp(a, b) <= 0 ? a : b);
export const pow2 = (k) => (k >= 0 ? rat(1n << BigInt(k)) : rat(1n, 1n << BigInt(-k)));
export const toNumber = (a) => {
  // Scaled division keeps precision for tiny and huge values.
  const shift = BigInt(Math.max(0, a.d.toString(2).length - a.n.toString(2).length + 64));
  return Number((a.n << shift) / a.d) / 2 ** Number(shift);
};
export const ZERO = rat(0n);
export const UNIT = pow2(-24);
export const ETA = pow2(-150);
const LIMIT = pow2(127);

// ---------- binary32 encodings ----------
const view = new DataView(new ArrayBuffer(4));
export const floatOf = (bits) => {
  view.setUint32(0, bits);
  return view.getFloat32(0);
};
const bitsOf = (x) => {
  view.setFloat32(0, x);
  return view.getUint32(0);
};
export function exactValue(bits) {
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

function generate(stream) {
  const { below } = stream;
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

/** Draw graphs from the stream until `graphs` are certifiable; overflow-reaching graphs are discarded. */
export function buildCorpus(stream, graphs) {
  const corpus = [];
  let discarded = 0;
  while (corpus.length < graphs) {
    const graph = generate(stream);
    const analysis = analyze(graph);
    if (analysis.certifiable) corpus.push({ graph, analysis });
    else discarded++;
  }
  return { corpus, discarded };
}

/** The largest certificate rational, in bits, and the checking budget the corpus declares. */
export function budgetOf(corpus) {
  const bitLength = (q) => Math.max((q.n < 0n ? -q.n : q.n).toString(2).length, q.d.toString(2).length);
  const maxBits = Math.max(...corpus.flatMap(({ analysis }) => analysis.bounds.flatMap((b) => [b.lo, b.hi, b.error].map(bitLength))));
  return { maxBits, budgetBits: 64 * Math.ceil((maxBits + 1) / 64) };
}

// ---------- the Lean corpus ----------
const leanRat = (q) => (q.d === 1n ? `(${q.n} : ℚ)` : `((${q.n} : ℚ) / ${q.d})`);
const hex32 = (bits) => `0x${bits.toString(16).toUpperCase().padStart(8, '0')}`;
const leanNode = (node) => {
  if (node.kind === 'input') return `.input ${node.index}`;
  if (node.kind === 'const') return `.const ${hex32(node.bits)}`;
  return `.${node.kind} ${node.left} ${node.right}`;
};
export function leanCorpus(corpus, budgetBits) {
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
