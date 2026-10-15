// The seven CALC-12 seeded incorrect implementations as harness-level
// models. Each one is the exact oracle with exactly one deliberate defect.
// They serve two purposes:
//   1. harness-level mutants: `compare.mjs --implementation fault:<name>`
//      treats a model's output as the observed implementation, so the
//      comparison and the bounded shrinker are exercised for every fault;
//   2. attribution: when an observed CLI result disagrees with the oracle,
//      `compare.mjs` reports which fault models predict exactly that result,
//      which is how a real mutant binary is shown to be rejected for its
//      intended reason.
// These models are incorrect by construction and never serve as an oracle.
import * as Q from './rational.mjs';
import { EXACT, emptySession, submit } from './calculator.mjs';

const raw = (n, d) => {
  // Sign into the numerator but no common-factor removal.
  if (d === 0n) throw new Q.OracleError('DivisionByZero', 'zero denominator');
  return d < 0n ? Object.freeze({ n: -n, d: -d }) : Object.freeze({ n, d });
};

// Exact rational value of an IEEE-754 double (finite).
const doubleToRational = number => {
  if (!Number.isFinite(number)) throw new Error('non-finite double');
  const view = new DataView(new ArrayBuffer(8));
  view.setFloat64(0, number);
  const bits = view.getBigUint64(0);
  const sign = bits >> 63n ? -1n : 1n;
  const exponent = Number((bits >> 52n) & 0x7ffn);
  const fraction = bits & ((1n << 52n) - 1n);
  const mantissa = exponent === 0 ? fraction : fraction | (1n << 52n);
  const power = (exponent === 0 ? 1 : exponent) - 1075;
  return power >= 0 ? Q.make(sign * mantissa * (1n << BigInt(power)), 1n)
    : Q.make(sign * mantissa, 1n << BigInt(-power));
};

const wrap64 = value => BigInt.asIntN(64, value);
const wrapped = (n, d) => {
  const wn = wrap64(n);
  const wd = wrap64(d);
  if (wd === 0n) throw new Q.OracleError('DivisionByZero', 'wrapped denominator is zero');
  return Q.make(wn, wd);
};

const integerQuotient = (x, y) => {
  if (y.n === 0n) throw new Q.OracleError('DivisionByZero', 'divisor is zero');
  return Q.make((x.n * y.d) / (x.d * y.n), 1n); // BigInt division truncates toward zero
};

const arithmetic = overrides => Object.freeze({ ...Q, ...overrides });

export const FAULTS = Object.freeze({
  'integer-division': {
    reason: 'rational `/` truncates to an integer quotient',
    semantics: { arith: arithmetic({ div: integerQuotient }), power: 'right' },
  },
  'binary-float-decimal-parser': {
    reason: 'numeric literals pass through an IEEE-754 binary double',
    semantics: {
      arith: arithmetic({ decimal: (whole, fraction) => doubleToRational(Number(fraction ? `${whole}.${fraction}` : whole)) }),
      power: 'right',
    },
  },
  'I64-wrapping': {
    reason: 'numerators and denominators wrap modulo 2^64 (two\'s complement)',
    semantics: {
      arith: arithmetic({
        decimal: (whole, fraction) => { const v = Q.decimal(whole, fraction); return wrapped(v.n, v.d); },
        add: (x, y) => wrapped(x.n * y.d + y.n * x.d, x.d * y.d),
        sub: (x, y) => wrapped(x.n * y.d - y.n * x.d, x.d * y.d),
        mul: (x, y) => wrapped(x.n * y.n, x.d * y.d),
        div: (x, y) => { if (y.n === 0n) throw new Q.OracleError('DivisionByZero', 'zero'); return wrapped(x.n * y.d, x.d * y.n); },
        neg: x => wrapped(-x.n, x.d),
        pow: (b, e) => { const v = Q.pow(b, e); return wrapped(v.n, v.d); },
      }),
      power: 'right',
    },
  },
  'reversed-subtraction': {
    reason: '`a - b` computes `b - a`',
    semantics: { arith: arithmetic({ sub: (x, y) => Q.sub(y, x) }), power: 'right' },
  },
  'left-associated-power': {
    reason: '`^` groups to the left, so `a^b^c` is `(a^b)^c`',
    semantics: { arith: Q, power: 'left' },
  },
  'non-normalized-rational': {
    reason: 'arithmetic results keep common factors (no gcd reduction)',
    semantics: {
      arith: arithmetic({
        add: (x, y) => raw(x.n * y.d + y.n * x.d, x.d * y.d),
        sub: (x, y) => raw(x.n * y.d - y.n * x.d, x.d * y.d),
        mul: (x, y) => raw(x.n * y.n, x.d * y.d),
        div: (x, y) => { if (y.n === 0n) throw new Q.OracleError('DivisionByZero', 'zero'); return raw(x.n * y.d, x.d * y.n); },
      }),
      power: 'right',
    },
  },
  'failure-mutates-session': {
    reason: 'a failed submission changes the session: it discards the previously accepted definitions',
    semantics: EXACT,
    sessionFault: true,
  },
});

export const FAULT_NAMES = Object.keys(FAULTS);

// Runs one process worth of lines under fault `name`.
export const runFault = (name, lines, { mode = 'default' } = {}) => {
  const fault = FAULTS[name];
  if (fault === undefined) throw new Error(`unknown fault ${name}`);
  let session = emptySession();
  const outcomes = [];
  for (const line of lines) {
    const result = submit(session, line, { mode, semantics: fault.semantics });
    // The exact oracle keeps the prior session on failure (CALC-STATE-02);
    // this fault instead publishes an emptied session.
    session = fault.sessionFault && result.outcome.kind === 'diagnostic' ? emptySession() : result.session;
    outcomes.push(result.outcome);
  }
  return {
    outcomes,
    stdout: outcomes.filter(o => o.kind === 'value').map(o => o.display),
    stderr: outcomes.filter(o => o.kind === 'diagnostic'),
    status: outcomes.some(o => o.kind === 'diagnostic') ? 1 : 0,
  };
};
