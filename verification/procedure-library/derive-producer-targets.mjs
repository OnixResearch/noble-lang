#!/usr/bin/env node
// Freeze composed references from #116's retained source programs, not a
// separately transcribed arithmetic formula or a target-search outcome.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const ACQUISITION = 'verification/synthesis/receipts/2026-11-13/synthesis-v2.json';
const PREDECESSOR = 'verification/procedure-library/criteria-producer-targets-v2.json';
const PREDECESSOR_MANIFEST = 'verification/procedure-library/producer-targets-v2.txt';
const CRITERION = 'verification/procedure-library/criteria-producer-targets-v3.json';
const MANIFEST = 'verification/procedure-library/producer-targets-v3.txt';
const ORDERS = [
  { name: 'arithCommutedThenSubtract', source_order: ['arithCommuted', 'arithSubtract'] },
  { name: 'arithSubtractThenCommuted', source_order: ['arithSubtract', 'arithCommuted'] },
];
const TRAIN = [[0, 0, 0], [5, -5, 7], [1, 2, 3]];
const PRODUCER_HELDOUT = [[7, 4, -2], [-2, -3, 5], [3, 9, -8]];
const sha = bytes => createHash('sha256').update(bytes).digest('hex');
const read = relative => fs.readFileSync(path.join(ROOT, relative));
const decimal = value => BigInt.asIntN(64, value).toString();

function execute(program, stack, operation, literal) {
  const words = program.split(/\s+/);
  assert.ok(words.length > 0 && words.length <= 8, 'source program exceeds the frozen grammar');
  for (const word of words) {
    if (word === 'dup') {
      assert.ok(stack.length > 0, 'dup underflow');
      stack.push(stack.at(-1));
    } else if (word === 'swap') {
      assert.ok(stack.length >= 2, 'swap underflow');
      const last = stack.pop();
      const before = stack.pop();
      stack.push(last, before);
    } else if (['+', '-', '*'].includes(word)) {
      assert.ok(stack.length >= 2, `${word} underflow`);
      const right = stack.pop();
      const left = stack.pop();
      stack.push(operation(word, left, right));
    } else {
      assert.match(word, /^(?:0|-?[1-9]\d*)$/, `unsupported source word: ${word}`);
      assert.ok([0, 1, 2, 3, 5, 7].includes(Number(word)), `constant outside #116 grammar: ${word}`);
      stack.push(literal(word));
    }
    assert.ok(stack.length <= 4, 'source stack exceeds the #116 bound');
  }
  return stack;
}

function compose(order, source, inputs, operation, literal) {
  let stack = [...inputs];
  for (const [at, name] of order.entries()) {
    stack = execute(source.get(name), stack, operation, literal);
    assert.equal(stack.length, at === 0 ? 2 : 1, `${name}: wrong composable stack interface`);
  }
  return stack[0];
}

function sourcePrograms(receipt) {
  assert.equal(receipt.schema, 'noble-synthesis-receipt/v1');
  assert.equal(receipt.source.clean, true);
  assert.equal(receipt.grammar.i64.name, 'synth-i64/0');
  const source = new Map();
  for (const name of ['arithCommuted', 'arithSubtract']) {
    const matches = receipt.tasks.filter(task => task.name === name);
    assert.equal(matches.length, 1, `${name}: exactly one #116 source declaration`);
    const task = matches[0];
    assert.equal(task.status, 'proved', `${name}: source status`);
    assert.equal(task.readmission.graph.outcome, 'normal', `${name}: source CLI re-admission`);
    assert.equal(task.declaration.inputs, 2, `${name}: source input arity`);
    assert.equal(task.search.grammar, 'synth-i64/0', `${name}: source grammar`);
    assert.equal(typeof task.search.program, 'string', `${name}: source body missing`);
    source.set(name, task.search.program);
  }
  return source;
}

function derive() {
  const acquisitionBytes = read(ACQUISITION);
  const receipt = JSON.parse(acquisitionBytes);
  const predecessor = JSON.parse(read(PREDECESSOR));
  assert.equal(predecessor.producer.acquisition_receipt_sha256, sha(acquisitionBytes));
  const sources = sourcePrograms(receipt);
  const tasks = ORDERS.map(({ name, source_order }) => {
    const reference = compose(source_order, sources, ['x', 'y', 'z'],
      (op, left, right) => `(${op} ${left} ${right})`, word => word);
    return {
      name, reference, declared_inputs: 3, bound: 8, source_order,
      source_program_sha256: source_order.map(source => sha(Buffer.from(sources.get(source)))),
    };
  });
  const criterion = {
    ...predecessor,
    schema: 'noble-procedure-library-producer-targets-criteria/3',
    predecessor_criteria: PREDECESSOR,
    preregistration_erratum: 'verification/procedure-library/receipts/producer-transfer-v2-disposition.json',
    producer: {
      ...predecessor.producer,
      manifest: MANIFEST,
      target_tasks: tasks,
      reference_derivation: {
        source: ACQUISITION,
        source_sha256: sha(acquisitionBytes),
        lane: 'verification/procedure-library/derive-producer-targets.mjs',
        semantics: 'Symbolically execute each retained #116 source program on [x,y,z]; every word consumes the actual top of stack; the second source consumes the first source result.',
      },
    },
    source_bound_sequence: [
      'Commit this mechanically derived criterion, manifest and derivation lane before running #116 on either target.',
      'Commit a non-held-out cross-check against independently interpreted #116 source programs before target generation.',
      'From a clean source commit, run #116 once against this manifest and retain candidate bodies, search trace, solver answers, held-out producer observations and CLI re-admission status.',
      'From a subsequent clean source commit, run the four transfer arms once and retain raw output and the source-bound receipt regardless of GO or NO-GO. Never rerun or replace targets based on outcomes.',
    ],
    non_claims: [
      'Reference expressions and declared producer examples are computed from already-retained #116 source programs before either new target is generated.',
      'A passing target candidate, solver UNSAT or finite fixture is not an independently checked universal proof.',
      'Neither earlier v1/v2 criterion nor any retained outcome is revised by this third, separately frozen criterion.',
      'Finite transfer does not show lifelong learning, a speedup, or universal alias/encoder correspondence.',
    ],
  };
  const globals = [];
  for (const raw of read(PREDECESSOR_MANIFEST).toString('utf8').split('\n')) {
    const line = raw.trim();
    if (line.startsWith('task ')) break;
    if (line && !line.startsWith('#')) globals.push(line);
  }
  assert.equal(globals.length, 11, 'producer global grammar and budgets changed');
  const manifest = [
    '# Frozen v3 target declarations: references and examples derive only from the retained',
    '# #116 source-program bodies, executed in their declared top-of-stack order.',
    '# No new target candidate or transfer result is an input to this derivation.',
    ...globals, '',
  ];
  const numeric = (op, left, right) => BigInt.asIntN(64,
    op === '+' ? left + right : op === '-' ? left - right : left * right);
  for (const task of tasks) {
    manifest.push(`task ${task.name} arithmetic i64 inputs=3 bound=8`, `  ref ${task.reference}`);
    for (const [kind, points] of [['train', TRAIN], ['heldout', PRODUCER_HELDOUT]]) {
      for (const inputs of points) {
        const actual = compose(task.source_order, sources, inputs.map(BigInt), numeric, BigInt);
        manifest.push(`  ${kind} ${inputs.join(' ')} = ${decimal(actual)}`);
      }
    }
    manifest.push('');
  }
  return { criterion: `${JSON.stringify(criterion, null, 2)}\n`, manifest: `${manifest.join('\n')}\n` };
}

const { criterion, manifest } = derive();
const mode = process.argv[2];
assert.ok(mode === '--write' || mode === '--check', 'usage: derive-producer-targets.mjs --write|--check');
assert.equal(process.argv.length, 3);
for (const [name, bytes] of [[CRITERION, criterion], [MANIFEST, manifest]]) {
  if (mode === '--write') {
    fs.writeFileSync(path.join(ROOT, name), bytes, { flag: 'wx' });
  } else {
    assert.equal(read(name).toString('utf8'), bytes, `${name}: mechanically derived bytes drifted`);
  }
}
process.stdout.write(`${JSON.stringify({ result: 'passed', mode, criterion_sha256: sha(criterion),
  manifest_sha256: sha(manifest), target_count: ORDERS.length })}\n`);
