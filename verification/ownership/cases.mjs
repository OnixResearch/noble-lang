//! Independent expectation generator for the ordered-stack ownership fragment
//! (issue #59).
//!
//! The expectations below are written from the Lean model in
//! `proofs/foundation/NobleFoundation/Ownership.lean`, never from the Rust
//! implementation of the acceptance machine, so the comparison measures the
//! production path against a written-down specification. Each case names the
//! operand payloads a word sequence inspects, the accounting the model claims
//! and the production-analogue classification:
//!
//! * `accepted` / `rejected` cases have a production analogue: the same word
//!   sequence instantiated over the corresponding kernel types, where a
//!   rejected case must surface as the eligibility constraint.
//! * `model-only` cases have no production analogue in the acceptance machine
//!   (wrapped payloads, the two-arm accounting rule and the ordinary
//!   error-return rule), so they are named in the corpus but never counted as
//!   an executable production control.

import { writeFileSync, mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';

const OUTPUT = join(import.meta.dirname, 'cases.txt');

// Payloads mirror `NobleFoundation.Ownership.Payload`.
const data = tag => ({ kind: 'data', tag });
const resource = tag => ({ kind: 'resource', tag });
const wrapped = inner => ({ kind: 'wrapped', inner });

// `containsResource` and `duplicable` mirror the Lean definitions exactly.
function containsResource(payload) {
  if (payload.kind === 'wrapped') return containsResource(payload.inner);
  return payload.kind === 'resource';
}
function duplicable(payload) {
  return !containsResource(payload);
}

// `step` mirrors `NobleFoundation.Ownership.step`. A rejection is classified:
// `missing` when the word has no operand to inspect (a production arity
// mismatch, not an eligibility decision) and `ineligible` when a
// data-constraining word inspected a non-duplicable operand, which is the
// production eligibility rejection. Swap only reorders its operands.
function step(word, operands) {
  const [first, second, ...rest] = operands;
  const tail = [second, ...rest].filter(item => item !== undefined);
  switch (word) {
    case 'dup':
      if (first === undefined) return { kind: 'missing' };
      if (!duplicable(first)) return { kind: 'ineligible' };
      return { kind: 'ok', stack: [first, first, ...tail] };
    case 'drop':
      if (first === undefined) return { kind: 'missing' };
      if (!duplicable(first)) return { kind: 'ineligible' };
      return { kind: 'ok', stack: tail };
    case 'swap':
      if (first === undefined || second === undefined) return { kind: 'missing' };
      return { kind: 'ok', stack: [second, first, ...rest] };
    case 'quote':
      if (first === undefined) return { kind: 'missing' };
      if (!duplicable(first)) return { kind: 'ineligible' };
      return { kind: 'ok', stack: operands };
    default:
      throw new Error(`unknown word ${word}`);
  }
}

// `run` mirrors `NobleFoundation.Ownership.run` for word sequences, i.e. the
// composition chain a candidate body of invocations corresponds to.
function run(words, operands) {
  let current = operands;
  for (const word of words) {
    const outcome = step(word, current);
    if (outcome.kind !== 'ok') return outcome;
    current = outcome.stack;
  }
  return { kind: 'ok', stack: current };
}

function kind(payload) {
  if (payload.kind === 'wrapped') return `w:${kind(payload.inner)}`;
  return payload.kind === 'data' ? `d:${payload.tag}` : `r:${payload.tag}`;
}

// Operand-stack shapes: empty, one data operand of each data type, two data
// operands (the `swap` pair), a must-use resource at each relevant position,
// and wrapped payloads at both eligible and ineligible depths.
const shapes = [
  ['empty', []],
  ['one-i64', [data('i')]],
  ['one-text', [data('t')]],
  ['pair-i64-text', [data('i'), data('t')]],
  ['pair-text-i64', [data('t'), data('i')]],
  ['one-resource', [resource(1)]],
  ['resource-then-i64', [resource(1), data('i')]],
  ['i64-then-resource', [data('i'), resource(1)]],
  ['wrapped-data', [wrapped(data('i'))]],
  ['wrapped-resource', [wrapped(resource(1))]],
  ['deep-wrapped-resource', [wrapped(wrapped(resource(1)))]],
];

// A programme whose words include `quote` has no retain-in-place production
// analogue: the production quotation constructor wraps the operand into a
// program value, so those cases stay model-only while still exercising the
// shadow checker and the corpus mutation controls.
// Only the unary fragment has a retain-in-place production analogue. The
// production quotation constructor wraps its operand into a program value, and
// the two-operand swap slots are a scheme-level value ordering that the
// acceptance machine does not expose as an operand-stack order, so those
// programmes stay model-only while still exercising the shadow checker and the
// corpus mutation controls.
const programmes = [
  ['dup', ['dup']],
  ['drop', ['drop']],
  ['swap', ['swap'], false],
  ['quote', ['quote'], false],
  ['dup-drop', ['dup', 'drop']],
  ['quote-dup', ['quote', 'dup'], false],
  ['swap-drop', ['swap', 'drop'], false],
  ['dup-swap', ['dup', 'swap'], false],
  ['drop-swap', ['drop', 'swap'], false],
];

// The production analogue exists only for payloads the kernel can name as a
// binding type; wrapped payloads are model-only.
function analogue(operands) {
  return operands.every(payload => payload.kind === 'data' || payload.kind === 'resource');
}

const cases = [];
for (const [programmeName, words, hasAnalogue = true] of programmes) {
  for (const [shapeName, operands] of shapes) {
    const outcome = run(words, operands);
    const accepted = outcome.kind === 'ok';
    // Only an eligibility rejection of nameable payloads by a programme with a
    // retain-in-place analogue is an accepted production control; a missing
    // operand is a production arity mismatch instead.
    const decision = !analogue(operands) || !hasAnalogue
      ? 'model-only'
      : accepted
        ? 'accepted'
        : outcome.kind === 'ineligible'
          ? 'rejected'
          : 'model-only';
    cases.push({
      name: `${programmeName}-${shapeName}`,
      words,
      operands,
      outcome: accepted ? outcome.stack : null,
      decision,
    });
  }
}

// The two-arm accounting rule and the ordinary error-return rule are named as
// structured programs: no production analogue exposes accounting across arms
// or a normal error return inside the acceptance machine, so these cases are
// model-only but still exercise the shadow checker's branch and error rules.
function runProgram(program, operands) {
  if (program.kind === 'words') return run(program.words, operands);
  if (program.kind === 'branch') {
    const thenOutcome = runProgram(program.thenArm, operands);
    const elseOutcome = runProgram(program.elseArm, operands);
    if (thenOutcome.kind !== 'ok' || elseOutcome.kind !== 'ok') return { kind: 'branch-rejected' };
    if (thenOutcome.stack.length !== elseOutcome.stack.length) return { kind: 'unequal' };
    for (let index = 0; index < thenOutcome.stack.length; index += 1) {
      if (thenOutcome.stack[index] !== elseOutcome.stack[index]) return { kind: 'unequal' };
    }
    return thenOutcome;
  }
  if (program.kind === 'fail') {
    return operands.length === 0 ? { kind: 'ok', stack: operands } : { kind: 'outstanding' };
  }
  throw new Error('unknown program');
}

function render(program) {
  if (program.kind === 'words') return program.words.join('+');
  if (program.kind === 'branch') return `branch(${render(program.thenArm)}|${render(program.elseArm)})`;
  return 'fail';
}

const structured = [
  {
    name: 'branch-equal-accounting',
    program: { kind: 'branch', thenArm: { kind: 'words', words: ['dup'] }, elseArm: { kind: 'words', words: ['dup'] } },
    operands: [data('i')],
  },
  {
    name: 'branch-unequal-accounting',
    program: { kind: 'branch', thenArm: { kind: 'words', words: ['dup'] }, elseArm: { kind: 'words', words: ['drop'] } },
    operands: [data('i')],
  },
  {
    name: 'branch-rejects-ineligible-arm',
    program: { kind: 'branch', thenArm: { kind: 'words', words: ['dup'] }, elseArm: { kind: 'words', words: ['dup'] } },
    operands: [resource(1)],
  },
  {
    name: 'branch-loses-resource',
    program: {
      kind: 'branch',
      thenArm: { kind: 'words', words: ['swap'] },
      elseArm: { kind: 'words', words: ['drop'] },
    },
    operands: [resource(1), data('i')],
  },
  {
    name: 'branch-doubles-resource',
    program: {
      kind: 'branch',
      thenArm: { kind: 'words', words: ['swap'] },
      elseArm: { kind: 'words', words: ['dup'] },
    },
    operands: [resource(1), data('i')],
  },
  {
    name: 'branch-wrong-owner-identity',
    program: {
      kind: 'branch',
      thenArm: { kind: 'words', words: ['swap'] },
      elseArm: { kind: 'words', words: ['swap', 'swap'] },
    },
    operands: [resource(1), resource(2)],
  },
  {
    name: 'swap-resource-move',
    program: { kind: 'words', words: ['swap'] },
    operands: [resource(1), data('i')],
  },
  {
    name: 'fail-outstanding-operand',
    program: { kind: 'fail' },
    operands: [data('i')],
  },
  {
    name: 'fail-without-outstanding-operand',
    program: { kind: 'fail' },
    operands: [],
  },
];

for (const entry of structured) {
  const outcome = runProgram(entry.program, entry.operands);
  cases.push({
    name: entry.name,
    program: entry.program,
    operands: entry.operands,
    outcome: outcome.kind === 'ok' ? outcome.stack : null,
    decision: 'model-only',
  });
}

const lines = cases.map(entry => {
  const incoming = entry.operands.map(kind).join(',');
  const outgoing = entry.outcome === null ? '-' : entry.outcome.map(kind).join(',');
  const ops = entry.program === undefined ? entry.words.join('+') : render(entry.program);
  return [
    entry.name,
    `ops=${ops}`,
    `in=${incoming || 'empty'}`,
    `out=${outgoing}`,
    `decision=${entry.decision}`,
  ].join(' ');
});

const header = [
  '# Ownership accounting corpus, generated from the Lean model (issue #59).',
  '# Fields: name ops=<program> in=<kind[,kind]> out=<kind[,kind]|-|empty> decision=<accepted|rejected|model-only>',
  '# kinds: d:<tag> a data payload, r:<tag> a tagged must-use resource payload, w:<kind> a recursively wrapped payload.',
  '# programs: w+w a word sequence, branch(then|else) a two-arm branch, fail a normal error return.',
].join('\n');

mkdirSync(dirname(OUTPUT), { recursive: true });
writeFileSync(OUTPUT, `${header}\n${lines.join('\n')}\n`);

const counts = cases.reduce((tally, entry) => {
  tally[entry.decision] = (tally[entry.decision] || 0) + 1;
  return tally;
}, {});
process.stdout.write(JSON.stringify({ cases: cases.length, counts, output: OUTPUT }) + '\n');
