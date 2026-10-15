// Exact rational numbers over the JavaScript engine's native BigInt.
//
// Independence: this file implements CALC-NUM-02..05 directly from
// `.cairn/specs/calculator/spec.md`. It imports nothing from the repository,
// and the arithmetic is the host engine's (V8) arbitrary-precision BigInt,
// not the guest `calculator-*.noble` limb code or any Rust crate.
// A value is a frozen `{ n, d }` with `d > 0n` and `gcd(|n|, d) = 1`.

export class OracleError extends Error {
  constructor(category, detail) {
    super(`${category}: ${detail}`);
    this.category = category;
  }
}

const abs = value => (value < 0n ? -value : value);

export const gcd = (left, right) => {
  let a = abs(left);
  let b = abs(right);
  while (b !== 0n) [a, b] = [b, a % b];
  return a;
};

// CALC-NUM-02: sign in the numerator, common factors removed, zero is 0/1.
export const make = (numerator, denominator) => {
  if (typeof numerator !== 'bigint' || typeof denominator !== 'bigint') {
    throw new TypeError('rational parts must be BigInt');
  }
  if (denominator === 0n) throw new OracleError('DivisionByZero', 'zero denominator');
  if (numerator === 0n) return Object.freeze({ n: 0n, d: 1n });
  const sign = denominator < 0n ? -1n : 1n;
  const common = gcd(numerator, denominator);
  return Object.freeze({ n: (sign * numerator) / common, d: (sign * denominator) / common });
};

export const integer = value => make(BigInt(value), 1n);
export const ZERO = integer(0);
export const ONE = integer(1);

export const add = (x, y) => make(x.n * y.d + y.n * x.d, x.d * y.d);
export const sub = (x, y) => make(x.n * y.d - y.n * x.d, x.d * y.d);
export const mul = (x, y) => make(x.n * y.n, x.d * y.d);
export const neg = x => make(-x.n, x.d);

// CALC-NUM-04: a zero divisor is an error, never zero, infinity or truncation.
export const div = (x, y) => {
  if (y.n === 0n) throw new OracleError('DivisionByZero', 'divisor is zero');
  return make(x.n * y.d, x.d * y.n);
};

// CALC-NUM-05: integral exponents only (judged on the value, so 2/2 is
// integral); negative exponents take exact reciprocals; x^0 = 1 incl. 0^0.
export const pow = (base, exponent) => {
  if (exponent.d !== 1n) throw new OracleError('NonIntegerExponent', 'exponent is not integral');
  const e = exponent.n;
  if (e === 0n) return ONE;
  if (e < 0n) {
    if (base.n === 0n) throw new OracleError('DivisionByZero', 'zero base with negative exponent');
    return make(base.d ** -e, base.n ** -e);
  }
  return make(base.n ** e, base.d ** e);
};

// CALC-NUM-03: digits [ "." digits ] is the integer of all digits over 10^k.
export const decimal = (whole, fraction = '') => {
  if (!/^[0-9]+$/.test(whole) || !/^[0-9]*$/.test(fraction)) {
    throw new TypeError(`not a decimal literal: ${whole}.${fraction}`);
  }
  return make(BigInt(whole + fraction), 10n ** BigInt(fraction.length));
};

export const equal = (x, y) => x.n === y.n && x.d === y.d;

// CALC-DISPLAY-01 default: an integer when d = 1, otherwise n/d.
export const displayDefault = x => (x.d === 1n ? `${x.n}` : `${x.n}/${x.d}`);

// CALC-DISPLAY-01 finite decimal: exact decimal iff d has no prime factor
// other than 2 and 5; otherwise the exact fraction stays (CALC-APPROX-01).
export const displayFiniteDecimal = x => {
  if (x.d === 1n) return `${x.n}`;
  let rest = x.d;
  let twos = 0n;
  let fives = 0n;
  while (rest % 2n === 0n) { rest /= 2n; twos += 1n; }
  while (rest % 5n === 0n) { rest /= 5n; fives += 1n; }
  if (rest !== 1n) return displayDefault(x);
  const places = twos > fives ? twos : fives;
  const scaled = abs(x.n) * (10n ** places / x.d);
  const digits = scaled.toString().padStart(Number(places) + 1, '0');
  const cut = digits.length - Number(places);
  return `${x.n < 0n ? '-' : ''}${digits.slice(0, cut)}.${digits.slice(cut)}`;
};
