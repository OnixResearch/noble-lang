// Benchmark kernel catalog and independent K-NUM-01 mathematical oracle for
// the Phase-2 GPU CPU-cost study (`.cairn/changes/gpu-offload-research`).
//
// Every expected result is computed directly from the K-NUM-01 mathematical
// model `((z + 2^63) mod 2^64) - 2^63` using arbitrary-precision BigInt
// arithmetic — never from Wasm or V8 execution.

export const TWO63 = 1n << 63n;
export const TWO64 = 1n << 64n;
export const I64_MAX = TWO63 - 1n;
export const I64_MIN = -TWO63;

export function wrap64(z) {
  const shifted = z + TWO63;
  const rem = ((shifted % TWO64) + TWO64) % TWO64;
  return rem - TWO63;
}

export function checksum(values) {
  let acc = 0n;
  for (const v of values) {
    acc = wrap64(acc * 33n + wrap64(v));
  }
  return acc;
}

export function nullChecksum(count) {
  let acc = 0n;
  for (let i = 0n; i < BigInt(count); i++) {
    acc = wrap64(acc * 33n + i);
  }
  return acc;
}

export function sequenceValues(seed, count) {
  const out = [];
  const base = BigInt(seed);
  for (let i = 0n; i < BigInt(count); i++) {
    out.push(wrap64(base + i));
  }
  return out;
}

// GPU-PLAN-01 / GPU-PLAN-02 / GPU-PLAN-04 boundary inputs covering 32-bit and
// 64-bit signed/unsigned carry, borrow, multiplication and branch boundaries.
export const BOUNDARY_INPUTS = [
  I64_MAX,
  I64_MIN,
  -1n,
  0n,
  1n,
  2n,
  3n,
  -3n,
  7n,
  10n,
  2147483648n,   // 2^31
  4294967295n,   // 2^32 - 1
  4294967296n,   // 2^32
  -4294967296n,  // -2^32
  I64_MAX - 1n,
];

function applyChain(x, ops) {
  let v = wrap64(x);
  for (const [op, imm] of ops) {
    if (op === '+') v = wrap64(v + imm);
    else if (op === '-') v = wrap64(v - imm);
    else if (op === '*') v = wrap64(v * imm);
    else throw new Error(`unsupported chain op: ${op}`);
  }
  return v;
}

function chainSource(ops) {
  return `[ ${ops.map(([op, imm]) => `${imm} ${op}`).join(' ')} ]`;
}

const CHAIN_8 = [
  ['+', 1n], ['*', 2n], ['-', 3n], ['+', 5n],
  ['*', 7n], ['-', 11n], ['+', 13n], ['*', 17n],
];
const CHAIN_16 = [
  ...CHAIN_8,
  ['-', 19n], ['+', 23n], ['*', 29n], ['-', 31n],
  ['+', 37n], ['*', 41n], ['-', 43n], ['+', 47n],
];
const CHAIN_32 = [
  ...CHAIN_16,
  ['*', 53n], ['-', 59n], ['+', 61n], ['*', 67n],
  ['-', 71n], ['+', 73n], ['*', 79n], ['-', 83n],
  ['+', 89n], ['*', 97n], ['-', 101n], ['+', 103n],
  ['*', 107n], ['-', 109n], ['+', 113n], ['*', 127n],
];

// Representative GPU-Map-Draft eligible `[I64] -> [I64] ! {}` kernels spanning
// widths w = 2..64, arithmetic (+, -, *), stack manipulation (dup, swap, drop)
// and branch selection (=, if).
export const KERNELS = [
  {
    id: 'w2_add',
    description: 'Minimal 2-word increment kernel',
    source: '[ 1 + ]',
    expected_w: 2,
    expected_steps_per_element: 2,
    oracle: x => wrap64(wrap64(x) + 1n),
  },
  {
    id: 'w3_swap_add',
    description: '3-word add with stack swap',
    source: '[ 1 swap + ]',
    expected_w: 3,
    expected_steps_per_element: 3,
    oracle: x => wrap64(1n + wrap64(x)),
  },
  {
    id: 'w3_sub_order',
    description: '3-word non-commutative subtraction (10 - x, GPU-PLAN-02)',
    source: '[ 10 swap - ]',
    expected_w: 3,
    expected_steps_per_element: 3,
    oracle: x => wrap64(10n - wrap64(x)),
  },
  {
    id: 'w4_add2',
    description: '4-word double increment',
    source: '[ 1 + 1 + ]',
    expected_w: 4,
    expected_steps_per_element: 4,
    oracle: x => wrap64(wrap64(x) + 2n),
  },
  {
    id: 'w6_add3',
    description: '6-word triple increment',
    source: '[ 1 + 1 + 1 + ]',
    expected_w: 6,
    expected_steps_per_element: 6,
    oracle: x => wrap64(wrap64(x) + 3n),
  },
  {
    id: 'w8_add4',
    description: '8-word quadruple increment',
    source: '[ 1 + 1 + 1 + 1 + ]',
    expected_w: 8,
    expected_steps_per_element: 8,
    oracle: x => wrap64(wrap64(x) + 4n),
  },
  {
    id: 'm2_sq',
    description: '2-word square kernel (x * x)',
    source: '[ dup * ]',
    expected_w: 2,
    expected_steps_per_element: 2,
    oracle: x => {
      const v = wrap64(x);
      return wrap64(v * v);
    },
  },
  {
    id: 'm4_sub_sq',
    description: '4-word shifted square ((x - 3)^2)',
    source: '[ 3 - dup * ]',
    expected_w: 4,
    expected_steps_per_element: 4,
    oracle: x => {
      const d = wrap64(wrap64(x) - 3n);
      return wrap64(d * d);
    },
  },
  {
    id: 'm9_poly',
    description: '9-word quadratic polynomial (x^2 + 3x + 7)',
    source: '[ dup dup * swap 3 * + 7 + ]',
    expected_w: 9,
    expected_steps_per_element: 9,
    oracle: x => {
      const v = wrap64(x);
      return wrap64(wrap64(wrap64(v * v) + wrap64(v * 3n)) + 7n);
    },
  },
  {
    id: 'br_if',
    description: 'Eligible scalar if/select kernel (GPU-PLAN-04: x==0 ? 7 : 3*x)',
    source: '[ dup 0 = [ drop 7 ] [ 3 * ] if ]',
    expected_w: 8,
    expected_steps_per_element: 8,
    oracle: x => {
      const v = wrap64(x);
      return v === 0n ? 7n : wrap64(v * 3n);
    },
  },
  {
    id: 'w16_arith',
    description: '16-word mixed +,-,* arithmetic chain',
    source: chainSource(CHAIN_8),
    expected_w: 16,
    expected_steps_per_element: 16,
    oracle: x => applyChain(x, CHAIN_8),
  },
  {
    id: 'w32_arith',
    description: '32-word mixed +,-,* arithmetic chain',
    source: chainSource(CHAIN_16),
    expected_w: 32,
    expected_steps_per_element: 32,
    oracle: x => applyChain(x, CHAIN_16),
  },
  {
    id: 'w64_arith',
    description: '64-word mixed +,-,* arithmetic chain',
    source: chainSource(CHAIN_32),
    expected_w: 64,
    expected_steps_per_element: 64,
    oracle: x => applyChain(x, CHAIN_32),
  },
];
