#!/usr/bin/env node
// Exercises actual source-checked calculator modules through their CLI Wasm
// worker. This gate records no Cairn implementation/execution state.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { spawnSync } from 'node:child_process';
import path from 'node:path';

const binary = process.argv[2];
assert.ok(binary && path.isAbsolute(binary) && fs.statSync(binary).isFile(),
  'usage: node verification/calculator-v1/gate.mjs /absolute/path/to/noble');
const cases = JSON.parse(fs.readFileSync(new URL('../../specs/conformance/calculator-cases.json', import.meta.url)));
const named = id => cases.cases.find(item => item.id === id);
const run = (lines, flags = []) => {
  const result = spawnSync(binary, ['calculator', ...flags], {
    input: Buffer.from(lines.join('\n') + '\n'), encoding: 'utf8',
    timeout: 60000, maxBuffer: 4 * 1024 * 1024,
    env: process.env,
  });
  assert.ifError(result.error);
  assert.equal(result.signal, null, `calculator terminated externally: ${result.signal}`);
  return { status: result.status, values: result.stdout.trimEnd().split('\n'),
    diagnostics: result.stderr.trimEnd().split('\n') };
};
const exact = named('CALC-01').input.vectors;
const arithmetic = run(exact.map(vector => vector.expression));
assert.equal(arithmetic.status, 0, arithmetic.diagnostics.join('\n'));
assert.deepEqual(arithmetic.values, exact.map(vector => vector.denominator === '1'
  ? vector.numerator : `${vector.numerator}/${vector.denominator}`), 'CALC-01 exact arithmetic/precedence');

const errors = named('CALC-02').input.vectors;
const diagnosed = run(errors.map(vector => vector.expression));
assert.equal(diagnosed.status, 1);
assert.equal(diagnosed.values.join(''), '');
assert.equal(diagnosed.diagnostics.length, errors.length);
for (let i = 0; i < errors.length; i++) {
  assert.match(diagnosed.diagnostics[i], new RegExp(`^${errors[i].category} \\[${errors[i].span[0]},${errors[i].span[1]}\\)`),
    `CALC-02 ${errors[i].expression}`);
  assert.match(diagnosed.diagnostics[i], /provenance=unavailable$/);
}

const numeric = named('CALC-03').input.vectors;
const numericErrors = run(numeric.map(vector => vector.expression));
assert.equal(numericErrors.status, 1);
assert.equal(numericErrors.values.join(''), '');
for (let i = 0; i < numeric.length; i++) {
  assert.match(numericErrors.diagnostics[i], new RegExp(`^${numeric[i].category} \\[`),
    `CALC-03 ${numeric[i].expression}`);
}

const display = named('CALC-09').input.vectors;
for (const mode of ['default', 'finite-decimal']) {
  const selected = display.filter(vector => vector.mode === mode);
  const observed = run(selected.map(vector => vector.fraction),
    mode === 'default' ? [] : ['--finite-decimal']);
  assert.equal(observed.status, 0, observed.diagnostics.join('\n'));
  assert.deepEqual(observed.values, selected.map(vector => vector.display), `CALC-09 ${mode}`);
}

const boundaries = [
  { flag: '--input-bytes', value: '2', source: '100', category: 'InputBytes' },
  { flag: '--numeric-digits', value: '1', source: '12', category: 'NumericDigits' },
  { flag: '--intermediate-bits', value: '2', source: '9', category: 'IntermediateIntegerBits' },
  { flag: '--syntax-depth', value: '1', source: '((1))', category: 'SyntaxDepth' },
  { flag: '--call-depth', value: '0', source: 'fn f() = 1\nf()', category: 'CallDepth' },
  { flag: '--evaluation-work', value: '0', source: '1 + 2', category: 'EvaluationWork' },
  { flag: '--output-size', value: '1', source: '100', category: 'OutputSize' },
];
for (const { flag, value, source, category } of boundaries) {
  const observed = run(source.split('\n'), [flag, value]);
  assert.equal(observed.status, 1, `${category}: ${observed.values.join(' ')}`);
  assert.match(observed.diagnostics.join('\n'), new RegExp(`ResourceLimit ${category} \\[`),
    `CALC-08 independent ${category}`);
}

const lexical = run(['let x = 10', 'fn f(y) = x + y', 'let x = 20', 'f(1)',
  'fn g(x) = x + 1', 'g(2)', 'let x = 1 / 0', 'x', 'f(1, 2)']);
assert.equal(lexical.status, 1);
assert.ok(lexical.values.includes('11'), 'CALC-06 captured old global value');
assert.ok(lexical.values.includes('3'), 'CALC-06 function parameter shadows global');
assert.ok(lexical.values.includes('20'), 'CALC-06 failed definition preserves x');
assert.match(lexical.diagnostics.join('\n'), /DivisionByZero/);
assert.match(lexical.diagnostics.join('\n'), /WrongArity/);

const admission = run(['fn bad() = missing', 'fn duplicate(x,x) = x',
  'fn self() = self()', 'fn dormant() = 1 / 0', 'dormant()']);
assert.equal(admission.status, 1);
assert.match(admission.diagnostics.join('\n'), /UnknownName/);
assert.match(admission.diagnostics.join('\n'), /DefinitionError/);
assert.match(admission.diagnostics.join('\n'), /DivisionByZero/);

console.log('CALC-01/02/03/06/07/08/09 actual calculator CLI guest Wasm vectors passed; no Cairn state promoted');
