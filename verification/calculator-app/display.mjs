// Transport decoding and terminal formatting only. No expression evaluation.
function shape(value, type) {
  if (value?.type !== type) throw Error(`unexpected guest ${type} representation`);
  return value.value;
}
export function nominal(value) { return shape(value, 'Nominal'); }
export function pair(value) {
  const parts = shape(value, 'Pair');
  if (!Array.isArray(parts) || parts.length !== 2) throw Error('invalid guest pair');
  return parts;
}
export function result(value) {
  if (value?.type !== 'Nominal' && value?.type !== 'Sum') throw Error('invalid guest Result');
  if (value.variant !== 'left' && value.variant !== 'right') throw Error('invalid guest Result');
  return { ok: value.variant === 'left', payload: value.value };
}
export function variantIndex(value) {
  let current = value;
  let index = 0;
  for (let depth = 0; depth < 16; depth++) {
    if (current?.type !== 'Nominal' && current?.type !== 'Sum') throw Error('invalid guest variant');
    if (current.variant === 'left') return [index, current.value];
    if (current.variant !== 'right') throw Error('invalid guest variant arm');
    if (current.value?.type !== 'Sum') return [index + 1, current.value];
    current = current.value;
    index++;
  }
  throw Error('guest variant nesting exceeds bound');
}
function integer(value) {
  const signed = pair(nominal(value));
  const negative = shape(signed[0], 'Bool');
  const bits = shape(signed[1], 'List');
  if (bits.length > 4096) throw Error('guest integer display exceeds implementation bound');
  if (negative && !bits.length) throw Error('guest returned negative zero');
  if (bits.length && shape(bits.at(-1), 'Bool') !== true)
    throw Error('guest returned noncanonical integer bits');
  let n = 0n;
  for (let i = bits.length - 1; i >= 0; --i) n = (n << 1n) | BigInt(shape(bits[i], 'Bool'));
  return negative ? -n : n;
}
export function rational(value) {
  const [numerator, denominator] = pair(nominal(value));
  const n = integer(numerator);
  const d = integer(denominator);
  if (d <= 0n) throw Error('guest returned invalid rational denominator');
  let a = n < 0n ? -n : n, b = d;
  while (b !== 0n) [a, b] = [b, a % b];
  if (a !== 1n) throw Error('guest returned noncanonical rational');
  return [n, d];
}
export function displayExact(n, d, mode) {
  if (mode === 'fraction' || mode === undefined || d === 1n)
    return d === 1n ? String(n) : `${n}/${d}`;
  if (mode !== 'finite-decimal') throw Error('unknown display mode');
  // Conversion for terminal display only; no expression result is computed here.
  let reduced = d, twos = 0, fives = 0;
  while (reduced % 2n === 0n) { reduced /= 2n; twos++; }
  while (reduced % 5n === 0n) { reduced /= 5n; fives++; }
  if (reduced !== 1n) return `${n}/${d}`;
  const decimals = Math.max(twos, fives);
  if (decimals > 4096) throw Error('decimal display exceeds implementation bound');
  const scaled = n * 10n ** BigInt(decimals) / d;
  const negative = scaled < 0n ? '-' : '';
  const digits = String(scaled < 0n ? -scaled : scaled).padStart(decimals + 1, '0');
  return decimals ? `${negative}${digits.slice(0, -decimals)}.${digits.slice(-decimals)}` : String(n);
}
const diagnostics = ['SyntaxError', 'TrailingInput', 'UnknownName',
  'WrongArity', 'DefinitionError', 'Numeric'];
function numericCategory(value) {
  const [outer, detail] = variantIndex(value);
  if (outer === 0) {
    const [kind, nested] = variantIndex(detail);
    if (kind === 0) {
      const [scope, leaf] = variantIndex(nested);
      if (scope === 0) return ['InputBytes', 'OutputSize'][variantIndex(leaf)[0]] ?? 'ResourceLimit';
      return ['NumericDigits', 'IntermediateIntegerBits'][variantIndex(leaf)[0]] ?? 'ResourceLimit';
    }
    const [depth, leaf] = variantIndex(nested);
    return depth === 0 ? 'EvaluationWork' :
      ['SyntaxDepth', 'CallDepth'][variantIndex(leaf)[0]] ?? 'ResourceLimit';
  }
  const [domain, child] = variantIndex(detail);
  if (domain === 0) return ['NonIntegral', 'OutOfRange'][variantIndex(child)[0]] ?? 'NumericError';
  const [extended, nested] = variantIndex(child);
  if (extended === 0) return 'NonIntegerExponent';
  return ['ZeroDenominator', 'DivisionByZero'][variantIndex(nested)[0]] ?? 'NumericError';
}
export function diagnostic(value) {
  const [kind, data] = variantIndex(value);
  if (kind >= diagnostics.length) throw Error('unknown guest diagnostic');
  const [spanValue, cause] = kind === 5 ? pair(data) : [data, null];
  const [start, end] = pair(nominal(spanValue)).map(v => Number(shape(v, 'I64')));
  if (!Number.isSafeInteger(start) || !Number.isSafeInteger(end) || start < 0 || end < start)
    throw Error('invalid guest diagnostic span');
  const category = kind === 5 ? numericCategory(cause) : diagnostics[kind];
  if (kind === 5 && variantIndex(cause)[0] === 0)
    return { category: 'ResourceLimit', limit: category,
      span: [start, end], provenance: 'unavailable' };
  return { category, span: [start, end], provenance: 'unavailable' };
}
