// Exact interchange and judging for the optional Wolfram provider (#24).
//
// A case reaches the reference as an exact tree, never as Noble source text:
//   { op: 'lit', digits: '0001', scale: 3 }      digits / 10^scale (CALC-NUM-03)
//   { op: 'neg', x }
//   { op: 'add' | 'sub' | 'mul' | 'div' | 'pow', x, y }
// `digits` is the literal's decimal digit string exactly as written, so an
// integer of any length crosses the boundary unchanged; no machine float is
// involved. The tree is derived from the #21 generator's tree shape
// (`../oracle/generate.mjs` parseTree/render: the CALC-12 test re-parser, not
// the guest parser), and a case is only admitted when `render(parseTree(text))`
// reproduces its text byte for byte.
//
// Reference results from `bridge.py` are untrusted candidate data: they are
// validated here (decimal integer strings, d > 0, gcd(|n|, d) = 1, known
// diagnostic categories) before any comparison.
import assert from 'node:assert/strict';
import { parseTree, render } from '../oracle/generate.mjs';

export const POLICY_VERSION = 'noble-wolfram-policy/1';
export const CATEGORIES = new Set(['DivisionByZero', 'NonIntegerExponent']);
// Reference states that are never a comparison result.
export const REFERENCE_STATES = new Set(['oracle-unavailable', 'oracle-timeout', 'oracle-error', 'unsupported',
  'no-recorded-expectation']);

const OPS = { '+': 'add', '-': 'sub', '*': 'mul', '/': 'div', '^': 'pow' };
const INTEGER = /^-?(?:0|[1-9]\d*)$/;
const POSITIVE = /^[1-9]\d*$/;
// One guest diagnostic line: `Category [s,e) provenance=P`; ResourceLimit adds
// its budget category.
const DIAGNOSTIC = /^([A-Za-z]+(?: [A-Za-z]+)?) \[(\d+),(\d+)\) provenance=(\S+)$/;

class Unsupported extends Error {}

const fromTree = node => {
  switch (node.t) {
  case 'num': {
    const [whole, fraction = ''] = node.text.split('.');
    assert.match(whole, /^\d+$/);
    assert.match(fraction, /^\d*$/);
    return { op: 'lit', digits: whole + fraction, scale: fraction.length };
  }
  case 'par': return fromTree(node.e);
  case 'neg': return { op: 'neg', x: fromTree(node.e) };
  case 'bin': return { op: OPS[node.op], x: fromTree(node.l), y: fromTree(node.r) };
  case 'name': throw new Unsupported(`name ${node.text} (lexical state is outside this lane)`);
  default: throw new Unsupported(`${node.t} node`);
  }
};

// Text -> { supported: true, interchange } | { supported: false, reason }.
export const fromText = text => {
  const tree = parseTree(text);
  if (tree === null) return { supported: false, reason: 'not a well-formed expression of the arithmetic subset' };
  if (render(tree) !== text) return { supported: false, reason: 'the CALC-12 re-parser does not reproduce the text' };
  try {
    return { supported: true, interchange: fromTree(tree) };
  } catch (error) {
    if (error instanceof Unsupported) return { supported: false, reason: error.message };
    throw error;
  }
};

export const operatorCount = node => (node.op === 'lit' ? 0
  : node.op === 'neg' ? operatorCount(node.x) : 1 + operatorCount(node.x) + operatorCount(node.y));

const abs = value => (value < 0n ? -value : value);
const gcd = (a, b) => {
  let [x, y] = [abs(a), abs(b)];
  while (y !== 0n) [x, y] = [y, x % y];
  return x;
};

// Validates one untrusted reference result and returns it in canonical shape.
export const validateReference = reference => {
  if (reference === null || typeof reference !== 'object') return { kind: 'oracle-error', detail: 'missing reference' };
  if (REFERENCE_STATES.has(reference.kind)) return { kind: reference.kind, detail: reference.detail ?? null };
  if (reference.kind === 'diagnostic') {
    return CATEGORIES.has(reference.category) ? { kind: 'diagnostic', category: reference.category, rule: reference.rule ?? null }
      : { kind: 'oracle-error', detail: `unknown diagnostic category ${reference.category}` };
  }
  if (reference.kind === 'value' && typeof reference.n === 'string' && typeof reference.d === 'string'
    && INTEGER.test(reference.n) && POSITIVE.test(reference.d) && gcd(BigInt(reference.n), BigInt(reference.d)) === 1n) {
    return { kind: 'value', n: reference.n, d: reference.d };
  }
  return { kind: 'oracle-error', detail: `invalid reference ${JSON.stringify(reference)}` };
};

// CALC-DISPLAY-01 default mode: `n` when d = 1, otherwise `n/d`.
export const expectedDisplay = ({ n, d }) => (d === '1' ? n : `${n}/${d}`);

// Exact value of an observed display line, or null. Used only to say why a
// line differs (another value, or the same value not normalized).
const observedValue = line => {
  const fraction = /^(-?\d+)(?:\/(\d+))?$/.exec(line);
  if (fraction) {
    const d = fraction[2] === undefined ? 1n : BigInt(fraction[2]);
    return d === 0n ? null : { n: BigInt(fraction[1]), d };
  }
  const decimal = /^(-?)(\d+)\.(\d+)$/.exec(line);
  if (decimal) return { n: BigInt(`${decimal[1]}${decimal[2]}${decimal[3]}`), d: 10n ** BigInt(decimal[3].length) };
  return null;
};

export const parseDiagnostics = lines => lines.map(line => {
  const match = DIAGNOSTIC.exec(line);
  return match === null ? null : { category: match[1], span: [Number(match[2]), Number(match[3])], provenance: match[4] };
});

// Judges one single-line process against one validated reference.
// observed = { stdout: [lines], stderr: [lines], status, signal, timedOut }.
// Verdicts: agree | disagree (with `mismatch`) | budget-divergence |
// not-comparable | the reference state itself (never agree).
export const judge = (reference, observed) => {
  if (REFERENCE_STATES.has(reference.kind)) return { verdict: reference.kind, mismatch: null, reasons: [reference.detail ?? reference.kind] };
  if (observed.timedOut) return { verdict: 'not-comparable', mismatch: null, reasons: ['process timed out'] };
  if (observed.signal !== null && observed.signal !== undefined) return { verdict: 'not-comparable', mismatch: null, reasons: [`signal ${observed.signal}`] };
  if (![0, 1].includes(observed.status)) return { verdict: 'not-comparable', mismatch: null, reasons: [`exit status ${observed.status}`] };
  const diagnostics = parseDiagnostics(observed.stderr);
  const foreign = observed.stderr.filter((_, index) => diagnostics[index] === null);
  if (foreign.length > 0) return { verdict: 'not-comparable', mismatch: null, reasons: foreign.map(line => `non-guest stderr line: ${line}`) };
  if (diagnostics.some(d => d.category.startsWith('ResourceLimit'))) {
    return { verdict: 'budget-divergence', mismatch: null,
      reasons: ['observed a deterministic guest ResourceLimit; the reference has no budget model'] };
  }
  const reasons = [];
  let mismatch = null;
  const note = (kind, reason) => { mismatch ??= kind; reasons.push(reason); };
  if (reference.kind === 'value') {
    const want = expectedDisplay(reference);
    if (diagnostics.length > 0) note('category-mismatch', `diagnostic ${diagnostics[0].category} where the reference has value ${want}`);
    if (observed.stdout.length !== 1) {
      if (diagnostics.length === 0) note('stream-shape', `stdout ${JSON.stringify(observed.stdout)} is not one value line`);
    } else if (observed.stdout[0] !== want) {
      const got = observedValue(observed.stdout[0]);
      if (got !== null && got.n * BigInt(reference.d) === BigInt(reference.n) * got.d) {
        note('non-normalized-display', `display ${observed.stdout[0]} has the reference value but not the normalized form ${want}`);
      } else {
        note('value-mismatch', `value ${observed.stdout[0]} != reference ${want}`);
      }
    }
    if (observed.status !== 0) note('stream-shape', `exit status ${observed.status} != 0`);
  } else {
    if (observed.stdout.length > 0) note('category-mismatch', `value ${observed.stdout.join(' ')} where the reference has ${reference.category}`);
    if (diagnostics.length !== 1) {
      if (observed.stdout.length === 0) note('stream-shape', `${diagnostics.length} diagnostics where the reference has one`);
    } else if (diagnostics[0].category !== reference.category) {
      note('category-mismatch', `diagnostic ${diagnostics[0].category} != reference ${reference.category}`);
    }
    if (observed.status !== 1) note('stream-shape', `exit status ${observed.status} != 1`);
  }
  return { verdict: mismatch === null ? 'agree' : 'disagree', mismatch, reasons,
    spans_not_judged: diagnostics.map(d => d.span) };
};

// The classification a shrink step must preserve: the verdict, the mismatch
// kind, and the domain premise (reference outcome kind and category).
export const signature = (reference, judgement) => [judgement.verdict, judgement.mismatch ?? '-', reference.kind,
  reference.category ?? '-'].join(':');
