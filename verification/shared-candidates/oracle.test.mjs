// Unit tests for the shared-candidate lane's oracle and generators:
//
//   NODE_ENV= bun test verification/shared-candidates/
//
// These assertions are hand-derived: every candidate fragment's value is
// compared with a formula written independently of the stack program, and
// every program's stack arity is checked. They run without the CLI.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

import {
  candidatePrograms,
  evalOutputs,
  firstDisagreement,
  mergedProgram,
  parseSource,
  prefixProgram,
  rows as oracleRows,
  run as oracleRun,
  tokens,
  wrap,
} from './oracle.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const criteria = JSON.parse(fs.readFileSync(path.join(here, 'criteria.json'), 'utf8'));
const shared = criteria.families.find((f) => f.name === 'shared-prefix');
const distinct = criteria.families.find((f) => f.name === 'distinct-prefix');

// Sample inputs: the frozen boundary rows plus two ordinary values.
const SAMPLES = [0n, 1n, -1n, 2n, -2n, 3n, 7n, -5n, 1001n, -99n];

/** The prefix of `shared-prefix`, written as a formula over `x`. */
function prefixValue(x) {
  const a = x * x * x;
  const b = a + 5n * x;
  const c = b * b - 3n;
  const d = c + 7n * c;
  const e = d + 11n;
  const f = e * e - 13n;
  return e + f;
}

/** Every shared-prefix tail as (value, S). */
const TAIL_FORMULAS = [
  (s) => s + 5n,
  (s) => s * s,
  (s) => s - 7n,
  (s) => s * s * s,
  (s) => 2n * s + 3n,
  (s) => s * (s + 1n),
  (s) => (s - 4n) * (s - 4n),
  (s) => (s + 10n) * (s + 10n),
  (s) => 3n * s,
  (s) => 3n * s - 5n + (3n * s - 5n),
  (s) => s + 6n * s * s + 1n,
  (s) => 2n * (s - 9n),
];

/** Every distinct-prefix candidate as (value, x). */
const FRAGMENT_FORMULAS = [
  (x) => x * x + x + 5n,
  (x) => 3n * x - 7n,
  (x) => x + 16n * x * x,
  (x) => (x + 1n) * (x - 1n),
  (x) => x + x * x * x - 9n,
  (x) => 4n * (6n - x),
  (x) => 2n * x - 4n * x * x,
  (x) => 23n * x + 8n,
  (x) => x - 5n + (x - 5n) * (x - 5n),
  (x) => 6n * x,
  (x) => (x + 13n) * (x + 10n),
  (x) => x * (10n - x),
];

function evaluateRoot(program, inputs = []) {
  const parsed = parseSource(program);
  return oracleRun(parsed.root, parsed.definitions, inputs);
}

test('the interpreter wraps like an i64', () => {
  assert.equal(evaluateRoot('1 1 +')[0], 2n);
  assert.equal(evaluateRoot(`${2n ** 63n} 1 -`)[0], 2n ** 63n - 1n);
  assert.equal(evaluateRoot(`${2n ** 63n - 1n} 1 +`)[0], -(2n ** 63n));
  assert.equal(evaluateRoot(`-1 ${2n ** 63n} *`)[0], -(2n ** 63n));
  assert.equal(evaluateRoot('5 dup *')[0], 25n);
  assert.equal(evaluateRoot('1 2 -')[0], -1n);
  assert.equal(evaluateRoot('1 2 swap -')[0], 1n);
  assert.equal(evaluateRoot('1 2 drop')[0], 1n);
});

test('the interpreter refuses what it does not model', () => {
  assert.throws(() => evaluateRoot('dup'), /needs 1 stack value/);
  assert.throws(() => evaluateRoot('1 2 + +'), /needs 2 stack value/);
  assert.throws(() => evaluateRoot('1 if'), /outside the modelled fragment/);
  assert.throws(() => evaluateRoot('[ 1 ]'), /outside the modelled fragment/);
  assert.throws(() => parseSource('1 [ 2 ] +\n3\n'), /not a def/);
  assert.throws(() => parseSource('def f [ 1 ] [ 2 ]\n1\n'), /nested brackets/);
});

test('every shared-prefix tail matches its hand-written formula', () => {
  assert.equal(shared.tails.length, TAIL_FORMULAS.length);
  for (let index = 0; index < shared.tails.length; index += 1) {
    const program = `${shared.prefix} ${shared.tails[index]}`;
    for (const x of SAMPLES) {
      const stack = evaluateRoot(program, [x]);
      assert.equal(stack.length, 1, `tail ${index + 1} must consume its input and leave one value`);
      assert.equal(stack[0], wrap(TAIL_FORMULAS[index](prefixValue(x))), `tail ${index + 1} at x = ${x}`);
    }
  }
});

test('the prefix alone matches its hand-written formula', () => {
  for (const x of SAMPLES) {
    const stack = evaluateRoot(shared.prefix, [x]);
    assert.equal(stack.length, 1);
    assert.equal(stack[0], wrap(prefixValue(x)), `prefix at x = ${x}`);
  }
});

test('every distinct-prefix candidate matches its hand-written formula', () => {
  assert.equal(distinct.fragments.length, FRAGMENT_FORMULAS.length);
  for (let index = 0; index < distinct.fragments.length; index += 1) {
    for (const x of SAMPLES) {
      const stack = evaluateRoot(distinct.fragments[index], [x]);
      assert.equal(stack.length, 1, `fragment ${index + 1} must leave one value`);
      assert.equal(stack[0], wrap(FRAGMENT_FORMULAS[index](x)), `fragment ${index + 1} at x = ${x}`);
    }
  }
});

test('the merged source puts every candidate in candidate order', () => {
  for (const x of SAMPLES) {
    const expected = TAIL_FORMULAS.map((f) => wrap(f(prefixValue(x))));
    const merged = parseSource(mergedProgram(shared, false).source);
    assert.deepEqual(oracleRun(merged.root, merged.definitions, [x]), expected, `merged stack at x = ${x}`);
    const duplicated = parseSource(mergedProgram(shared, true).source);
    assert.deepEqual(oracleRun(duplicated.root, duplicated.definitions, [x]), expected, `recomputing stack at x = ${x}`);
  }
});

test('the distinct-prefix merged source concatenates the candidates', () => {
  const program = mergedProgram(distinct, false);
  assert.equal(program.identical, true);
  const parsed = parseSource(program.source);
  for (const x of SAMPLES) {
    assert.deepEqual(
      oracleRun(parsed.root, parsed.definitions, [x]),
      FRAGMENT_FORMULAS.map((f) => wrap(f(x))),
      `merged distinct stack at x = ${x}`,
    );
  }
});

test('every candidate program is the frozen text', () => {
  const sharedPrograms = candidatePrograms(shared);
  assert.equal(sharedPrograms.length, 12);
  assert.equal(sharedPrograms[0].source, `${shared.prefix} ${shared.tails[0]}\n`);
  const distinctPrograms = candidatePrograms(distinct);
  assert.equal(distinctPrograms[11].source, `${distinct.fragments[11]}\n`);
  assert.equal(prefixProgram(shared).source, `${shared.prefix}\n`);
});


test('tokens keeps brackets separate and parseSource reads definitions', () => {
  assert.deepEqual(tokens('a [ b ]c'), ['a', '[', 'b', ']', 'c']);
  const parsed = parseSource('def twice [ dup + ]\ntwice\n');
  assert.deepEqual(parsed.root, ['twice']);
  assert.deepEqual(parsed.definitions.get('twice'), ['dup', '+']);
});

test('the frozen row set is deterministic and starts with the boundaries', () => {
  const first = oracleRows(criteria.vectors);
  const second = oracleRows(criteria.vectors);
  assert.deepEqual(first, second);
  assert.equal(first.length, criteria.vectors.rows);
  assert.deepEqual(
    first.slice(0, criteria.vectors.boundary.length),
    criteria.vectors.boundary.map((v) => wrap(BigInt(v))),
  );
  for (const value of first.slice(criteria.vectors.boundary.length)) {
    assert.ok(value >= 0n && value <= BigInt(criteria.vectors.range), 'generated rows stay in range');
  }
});

test('the comparator catches a permuted, a truncated and an over-long arm', () => {
  assert.equal(firstDisagreement([[1n, 2n]], [[1n, 2n]], 'same'), null);
  const permuted = firstDisagreement([[2n, 1n]], [[1n, 2n]], 'permuted');
  assert.notEqual(permuted, null);
  assert.match(permuted.detail, /candidate 0/);
  const short = firstDisagreement([[1n]], [[1n, 2n]], 'short');
  assert.equal(short.index, null);
  assert.match(short.detail, /1 output/);
  const rows = firstDisagreement([[1n], [2n]], [[1n]], 'rows');
  assert.match(rows.detail, /arm has 2 rows/);
});

test('evalOutputs reads normal rows and refuses anything else', () => {
  const normal = [
    '{"schema":"noble-ngraph-report/v1","command":"eval","stage":"evaluation","outcome":"normal","graph":"x","snapshot":null}',
    '{"schema":"noble-ngraph-report/v1","command":"eval","stage":"evaluation","outcome":"normal","vector":0,"outputs":["i64:5","i64:-1"]}',
    '{"schema":"noble-ngraph-report/v1","command":"eval","stage":"evaluation","outcome":"normal","vector":1,"outputs":["i64:0","i64:2"]}',
  ].join('\n');
  assert.deepEqual(evalOutputs(normal, 'arm'), [[5n, -1n], [0n, 2n]]);
  const trap = [
    '{"schema":"noble-ngraph-report/v1","command":"eval","stage":"evaluation","outcome":"normal","graph":"x","snapshot":null}',
    '{"schema":"noble-ngraph-report/v1","command":"eval","stage":"evaluation","outcome":"trap","code":"G-FACT-FALSE","vector":0}',
  ].join('\n');
  assert.throws(() => evalOutputs(trap, 'arm'), /did not evaluate normally/);
  assert.throws(() => evalOutputs('not json', 'arm'), /not JSON/);
});

test('the criteria freeze the arms, families and thresholds the lane needs', () => {
  assert.equal(criteria.schema, 'noble-shared-candidates-criteria/v1');
  assert.deepEqual(criteria.arms.map((a) => a.name), ['separate', 'merged', 'merged-duplicated', 'floor']);
  assert.deepEqual(criteria.families.map((f) => f.name), ['shared-prefix', 'distinct-prefix']);
  assert.equal(criteria.families[0].sharing, 'shared');
  assert.equal(criteria.families[1].sharing, 'none');
  assert.equal(shared.tails.length, 12);
  assert.equal(distinct.fragments.length, 12);
  for (const key of ['d1_separate_wall_ratio_max', 'd2_sharing_wall_ratio_max', 'd3_noise_band', 'd4_rss_growth_kib_max']) {
    assert.equal(typeof criteria.thresholds[key], 'number', key);
  }
  assert.ok(criteria.not_measured.some((line) => line.includes('#37')));
  assert.ok(criteria.not_measured.some((line) => line.includes('#105')));
});
