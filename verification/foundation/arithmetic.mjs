// Shared arithmetic reference and renderers for the foundation differential lanes.
// The Lean kernel, not this BigInt transcription, checks each expectation.
export const MIN = -(2n ** 63n), MAX = 2n ** 63n - 1n;
export const wrap = z => BigInt.asIntN(64, z);
export const inRange = n => n >= MIN && n <= MAX;
function refStep(op, s, mutant = {}) {
  if (typeof op === 'object') return inRange(op.push) ? [op.push, ...s] : null;
  if (s.length < 2) return null;
  const [y, x, ...rest] = s;
  const value = op === 'add' ? x + y : op === 'mul' ? x * y : (mutant.reversedSub ? y - x : x - y);
  return [wrap(value), ...rest];
}
export function refRun(ops, s, mutant) {
  for (const op of ops) { s = refStep(op, s, mutant); if (!s) return null; }
  return s;
}
export const opSource = op => typeof op === 'object' ? String(op.push) : { add: '+', sub: '-', mul: '*' }[op];
export const opsSource = ops => ops.map(opSource).join(' ');
export const leanInt = n => n < 0n ? `(${n})` : `${n}`;
export const leanOp = op => typeof op === 'object' ? `.push ${leanInt(op.push)}` : `.${op}`;
export const leanOps = ops => `[${ops.map(leanOp).join(', ')}]`;
export const leanList = values => `[${values.map(leanInt).join(', ')}]`;
export function prng(seed) { // splitmix64
  let state = BigInt.asUintN(64, BigInt(seed));
  return () => {
    state = BigInt.asUintN(64, state + 0x9e3779b97f4a7c15n);
    let z = state;
    z = BigInt.asUintN(64, (z ^ (z >> 30n)) * 0xbf58476d1ce4e5b9n);
    z = BigInt.asUintN(64, (z ^ (z >> 27n)) * 0x94d049bb133111ebn);
    return z ^ (z >> 31n);
  };
}
