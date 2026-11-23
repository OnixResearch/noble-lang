// Independent exact BigInt and normalized Rat reference oracle over native JS BigInt.
//
// Independence: imports nothing from the repository and performs all arithmetic
// using JavaScript's native arbitrary-precision BigInt, independent of the
// guest base-10^9 List<I64> Noble modules.

export const BASE = 1_000_000_000n;

export class OracleError extends Error {
  constructor(category, detail) {
    super(`${category}: ${detail}`);
    this.category = category;
  }
}

export const abs = x => (x < 0n ? -x : x);

export const compare = (a, b) => (a < b ? -1 : a > b ? 1 : 0);

// Truncating-toward-zero division matching JS BigInt / and %.
export const divmod = (a, b) => {
  if (b === 0n) throw new OracleError('DivisionByZero', 'zero divisor');
  const q = a / b;
  const r = a % b;
  return Object.freeze({ q, r });
};

export const gcd = (a, b) => {
  let x = abs(a);
  let y = abs(b);
  while (y !== 0n) {
    const t = x % y;
    x = y;
    y = t;
  }
  return x;
};

// Grammar: ["-"] ASCII-digits (1+ digits, no whitespace, no '+', no '.').
export const parseBigInt = text => {
  if (!/^-?[0-9]+$/.test(text)) {
    throw new OracleError('InvalidDecimal', `invalid integer literal: ${JSON.stringify(text)}`);
  }
  return BigInt(text);
};

// Canonical base-10^9 LSB-first representation check for BigInt.
export const toSignedLimbs = value => {
  if (value === 0n) return Object.freeze({ neg: false, limbs: Object.freeze([]) });
  const neg = value < 0n;
  let rest = abs(value);
  const limbs = [];
  while (rest > 0n) {
    limbs.push(Number(rest % BASE));
    rest /= BASE;
  }
  return Object.freeze({ neg, limbs: Object.freeze(limbs) });
};

export const fromSignedLimbs = (neg, limbs) => {
  if (!Array.isArray(limbs)) throw new TypeError('limbs must be an array');
  if (limbs.length === 0) {
    if (neg) throw new Error('non-canonical negative zero');
    return 0n;
  }
  if (limbs[limbs.length - 1] === 0) {
    throw new Error(`non-canonical trailing zero limb: ${JSON.stringify(limbs)}`);
  }
  let acc = 0n;
  for (let i = limbs.length - 1; i >= 0; i -= 1) {
    const limb = BigInt(limbs[i]);
    if (limb < 0n || limb >= BASE) {
      throw new Error(`limb out of range [0, 10^9): ${limbs[i]}`);
    }
    acc = acc * BASE + limb;
  }
  return neg ? -acc : acc;
};

// CALC-NUM-02: sign in numerator, common factors removed, d > 0, zero is 0/1.
export const ratMake = (n, d) => {
  if (d === 0n) throw new OracleError('DivisionByZero', 'zero denominator');
  if (n === 0n) return Object.freeze({ n: 0n, d: 1n });
  const sign = d < 0n ? -1n : 1n;
  const g = gcd(n, d);
  return Object.freeze({ n: (sign * n) / g, d: (sign * d) / g });
};

export const ratNeg = x => ratMake(-x.n, x.d);
export const ratAdd = (x, y) => ratMake(x.n * y.d + y.n * x.d, x.d * y.d);
export const ratSub = (x, y) => ratMake(x.n * y.d - y.n * x.d, x.d * y.d);
export const ratMul = (x, y) => ratMake(x.n * y.n, x.d * y.d);
export const ratDiv = (x, y) => {
  if (y.n === 0n) throw new OracleError('DivisionByZero', 'zero rational divisor');
  return ratMake(x.n * y.d, x.d * y.n);
};
export const ratCmp = (x, y) => compare(x.n * y.d, y.n * x.d);

// CALC-NUM-03: ["-"] digits [ "." digits ] -> exact reduced rational.
export const parseDecimal = text => {
  const match = /^(-?)([0-9]+)(?:\.([0-9]+))?$/.exec(text);
  if (!match) {
    throw new OracleError('InvalidDecimal', `invalid decimal literal: ${JSON.stringify(text)}`);
  }
  const [, sign, whole, frac = ''] = match;
  const raw = BigInt((sign === '-' ? '-' : '') + whole + frac);
  const scale = 10n ** BigInt(frac.length);
  return ratMake(raw, scale);
};
