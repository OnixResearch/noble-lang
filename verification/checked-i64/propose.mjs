#!/usr/bin/env node
// Independent BigInt proposals for the checked-I64 boundary replay (issue #39).
// Lean kernel reduction checks every retained expected string against its model.
import fs from 'node:fs';
import path from 'node:path';

const MIN = -(1n << 63n);
const MAX = (1n << 63n) - 1n;
const cases = [
  ['add-max-minus-one-plus-one', 'add', MAX - 1n, 1n],
  ['add-max-plus-one', 'add', MAX, 1n],
  ['add-min-plus-one-minus-one', 'add', MIN + 1n, -1n],
  ['add-min-minus-one', 'add', MIN, -1n],
  ['add-zero-minus-one', 'add', 0n, -1n],
  ['add-minus-one-zero', 'add', -1n, 0n],
  ['add-zero-zero', 'add', 0n, 0n],
  ['add-positive-positive', 'add', 1n, 2n],
  ['add-positive-negative', 'add', 1n, -2n],
  ['add-negative-positive', 'add', -1n, 2n],
  ['add-negative-negative', 'add', -1n, -2n],
  ['sub-min-plus-one-minus-one', 'sub', MIN + 1n, 1n],
  ['sub-min-minus-one', 'sub', MIN, 1n],
  ['sub-max-minus-negative-one', 'sub', MAX, -1n],
  ['sub-max-minus-one', 'sub', MAX, 1n],
  ['sub-zero-minus-negative-one', 'sub', 0n, -1n],
  ['sub-minus-one-zero', 'sub', -1n, 0n],
  ['sub-one-minus-two', 'sub', 1n, 2n],
  ['sub-two-minus-one', 'sub', 2n, 1n],
  ['sub-negative-negative', 'sub', -1n, -2n],
  ['mul-max-one', 'mul', MAX, 1n],
  ['mul-min-one', 'mul', MIN, 1n],
  ['mul-min-negative-one', 'mul', MIN, -1n],
  ['mul-max-two', 'mul', MAX, 2n],
  ['mul-min-zero', 'mul', MIN, 0n],
  ['mul-negative-one-three', 'mul', -1n, 3n],
  ['mul-positive-negative', 'mul', 3n, -2n],
  ['mul-negative-negative', 'mul', -2n, -3n],
  ['neg-max', 'neg', MAX, 0n],
  ['neg-min-plus-one', 'neg', MIN + 1n, 0n],
  ['neg-min', 'neg', MIN, 0n],
  ['neg-zero', 'neg', 0n, 0n],
  ['neg-negative-one', 'neg', -1n, 0n],
  ['abs-min', 'abs', MIN, 0n],
  ['abs-min-plus-one', 'abs', MIN + 1n, 0n],
  ['abs-negative-one', 'abs', -1n, 0n],
  ['abs-zero', 'abs', 0n, 0n],
  ['abs-max', 'abs', MAX, 0n],
  ['lt-negative-one-zero', 'lt', -1n, 0n],
  ['lt-zero-negative-one', 'lt', 0n, -1n],
  ['lt-min-max', 'lt', MIN, MAX],
  ['lt-max-min', 'lt', MAX, MIN],
  ['lt-negative-one-equal', 'lt', -1n, -1n],
  ['lt-positive-positive', 'lt', 1n, 2n],
  ['lt-positive-negative', 'lt', 1n, -2n],
  ['lt-negative-negative', 'lt', -2n, -1n],
  ['divrem-negative-seven-two', 'divrem', -7n, 2n],
  ['divrem-seven-negative-two', 'divrem', 7n, -2n],
  ['divrem-seven-two', 'divrem', 7n, 2n],
  ['divrem-negative-negative', 'divrem', -7n, -2n],
  ['divrem-two-seven', 'divrem', 2n, 7n],
  ['divrem-min-one', 'divrem', MIN, 1n],
  ['divrem-min-plus-one-one', 'divrem', MIN + 1n, 1n],
  ['divrem-max-negative-one', 'divrem', MAX, -1n],
  ['divrem-min-negative-one', 'divrem', MIN, -1n],
  ['divrem-seven-zero', 'divrem', 7n, 0n],
  ['divrem-zero-negative-one', 'divrem', 0n, -1n],
];

function scalar(value) {
  return value < MIN || value > MAX ? 'error:1' : `ok:${value}`;
}

function expected(operation, left, right) {
  switch (operation) {
    case 'add': return scalar(left + right);
    case 'sub': return scalar(left - right);
    case 'mul': return scalar(left * right);
    case 'neg': return scalar(-left);
    case 'abs': return scalar(left < 0n ? -left : left);
    case 'lt': return left < right ? 'bool:true' : 'bool:false';
    case 'divrem': {
      if (right === 0n) return 'error:2';
      const quotient = left / right;
      const remainder = left % right;
      if (quotient < MIN || quotient > MAX) return 'error:1';
      return `pair:${quotient}:${remainder}`;
    }
    default: throw new Error(`unknown operation ${operation}`);
  }
}

const output = process.argv[2];
if (!output || process.argv.length !== 3) {
  throw new Error('usage: propose.mjs OUT.json');
}
const retained = cases.map(([name, operation, left, right]) => ({
  name,
  operation,
  left: String(left),
  right: String(right),
  expected: expected(operation, left, right),
}));
fs.mkdirSync(path.dirname(path.resolve(output)), { recursive: true });
fs.writeFileSync(output, `${JSON.stringify({ schema: 'noble-checked-i64-proposals/v1', cases: retained }, null, 2)}\n`);
console.log(JSON.stringify({ outcome: 'proposed', output: path.resolve(output), cases: retained.length }));
