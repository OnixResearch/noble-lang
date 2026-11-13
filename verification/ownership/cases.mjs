//! Independent expectation generator for the ordered-stack ownership fragment
//! (issue #59).
//!
//! The expectations below are proposed by a JavaScript mirror of the Lean model in
//! `proofs/foundation/NobleFoundation/Ownership.lean` and
//! `OwnershipTrace.lean`, never from the Rust implementation of the acceptance
//! machine, so the comparison measures the production path against a
//! written-down specification. Two artefacts are emitted, and the same run
//! writes both:
//!
//! * `cases.txt` — the machine-readable corpus the Rust lane parses: one row per
//!   case with its program, incoming and outgoing stacks, the per-word stack trace
//!   and its classification.
//! * `proofs/foundation/NobleFoundation/OwnershipCorpus.lean` — the same corpus as
//!   kernel-checked rows: every proposed row is a `decide`d equation of the model's own
//!   `run` and `runTrace`, so a row whose expectation does not follow from the
//!   model fails the foundation build. The JavaScript generator remains a
//!   handwritten translation; the kernel check, not the mirror, is authoritative.
//!
//! `--check` compares both artefacts with what this generator would emit and
//! exits nonzero on any drift, so a committed corpus cannot silently diverge
//! from the emission.
//!
//! Each case names the operand payloads a word sequence inspects, the accounting
//! the model claims and its production-analogue classification:
//!
//! * `accepted` / `rejected` cases have a production analogue: the same word
//!   sequence instantiated over the corresponding kernel types, driven one word
//!   prefix at a time. A rejected case must surface as the eligibility
//!   constraint.
//! * `model-only` cases have no production analogue in the acceptance machine,
//!   and every one states why in its `reason` field (see `REASONS`).

import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';

const root = join(import.meta.dirname, '../..');
const CASES = join(root, 'verification/ownership/cases.txt');
const CORPUS = join(root, 'proofs/foundation/NobleFoundation/OwnershipCorpus.lean');
const checkOnly = process.argv.includes('--check');

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
// `missing` when the word has no operand to inspect (an operand-arity decision,
// not an accounting one) and `ineligible` when a data-constraining word
// inspected a non-duplicable operand, which is the production eligibility
// rejection. Swap only reorders its operands.
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

// `traceWords` mirrors the Lean trace: the incoming stack first, then one entry
// per admitted word. A refused word ends the trace.
function trace(words, operands) {
  const entries = [operands];
  let current = operands;
  for (const word of words) {
    const outcome = step(word, current);
    if (outcome.kind !== 'ok') return entries;
    entries.push(outcome.stack);
    current = outcome.stack;
  }
  return entries;
}

function kind(payload) {
  if (payload.kind === 'wrapped') return `w:${kind(payload.inner)}`;
  return payload.kind === 'data' ? `d:${payload.tag}` : `r:${payload.tag}`;
}

function renderStack(stack) {
  return stack.map(kind).join(',') || 'empty';
}

function renderTrace(entries) {
  if (entries === null) return '-';
  return entries.map(renderStack).join(';');
}

// Operand-stack shapes: empty, one data operand of each data type (tag 0 the
// I64 payload, tag 1 the text payload), data and
// resource pairs in both orders, two must-use resources (distinct owners), a
// must-use resource at each relevant position, and wrapped payloads at both
// eligible and ineligible depths.
const shapes = [
  ['empty', []],
  ['one-i64', [data(0)]],
  ['one-text', [data(1)]],
  ['pair-i64-text', [data(0), data(1)]],
  ['pair-text-i64', [data(1), data(0)]],
  ['one-resource', [resource(1)]],
  ['pair-resource-resource', [resource(1), resource(2)]],
  ['resource-then-i64', [resource(1), data(0)]],
  ['i64-then-resource', [data(0), resource(1)]],
  ['wrapped-data', [wrapped(data(0))]],
  ['wrapped-resource', [wrapped(resource(1))]],
  ['deep-wrapped-resource', [wrapped(wrapped(resource(1)))]],
];

// Only the retain-in-place fragment has a production analogue. The production
// quotation constructor wraps its operand into a program value, so a programme
// containing `quote` states a different rule; the two-operand `swap` slot
// ordering is drivable because the acceptance machine instantiates one scheme
// per word, which is what the lane compares prefix by prefix.
const programmes = [
  ['dup', ['dup']],
  ['drop', ['drop']],
  ['swap', ['swap']],
  ['quote', ['quote'], false],
  ['dup-drop', ['dup', 'drop']],
  ['quote-dup', ['quote', 'dup'], false],
  ['swap-drop', ['swap', 'drop']],
  ['dup-swap', ['dup', 'swap']],
  ['drop-swap', ['drop', 'swap']],
];

// The production analogue exists only for payloads the kernel can name as a
// binding type; a wrapped payload has no such type.
function analogue(operands) {
  return operands.every(payload => payload.kind !== 'wrapped');
}

const REASONS = {
  'branch-accounting': 'the two-arm branch accounts for both arms, a rule the acceptance machine does not expose as an operand-stack order',
  'error-return': 'the ordinary error return is admitted only with no outstanding operand, which the acceptance machine has no analogue for in this lane',
  'quotation-program-value': 'the production quotation constructor wraps its operand into a program value, so the model\'s retain-in-place quote rule is not the production rule',
  'wrapped-payload': 'a wrapped payload has no kernel binding type, so the acceptance machine cannot name it as an operand',
  'operand-arity': 'the model refuses because the word has no operand to inspect; the acceptance machine\'s scheme instantiation refuses such a shape as a stack join, a different rule',
};

const cases = [];
for (const [programmeName, words, hasAnalogue = true] of programmes) {
  for (const [shapeName, operands] of shapes) {
    const outcome = run(words, operands);
    const accepted = outcome.kind === 'ok';
    const decision = !analogue(operands) || !hasAnalogue
      ? 'model-only'
      : accepted
        ? 'accepted'
        : outcome.kind === 'ineligible'
          ? 'rejected'
          : 'model-only';
    const reason = decision !== 'model-only'
      ? '-'
      : !hasAnalogue
        ? 'quotation-program-value'
        : !analogue(operands)
          ? 'wrapped-payload'
          : 'operand-arity';
    cases.push({
      name: `${programmeName}-${shapeName}`,
      program: { kind: 'words', words },
      operands,
      outcome: accepted ? outcome.stack : null,
      trace: trace(words, operands),
      decision,
      reason,
    });
  }
}

// The two-arm accounting rule and the ordinary error-return rule are named as
// structured programs: no production analogue exposes accounting across arms or
// a normal error return inside the acceptance machine, so these cases are
// model-only but still exercise the shadow checker's branch and error rules.
function runProgram(program, operands) {
  if (program.kind === 'words') return run(program.words, operands);
  if (program.kind === 'branch') {
    const thenOutcome = runProgram(program.thenArm, operands);
    const elseOutcome = runProgram(program.elseArm, operands);
    if (thenOutcome.kind !== 'ok' || elseOutcome.kind !== 'ok') return { kind: 'branch-rejected' };
    if (thenOutcome.stack.length !== elseOutcome.stack.length) return { kind: 'unequal' };
    for (let index = 0; index < thenOutcome.stack.length; index += 1) {
      if (kind(thenOutcome.stack[index]) !== kind(elseOutcome.stack[index])) return { kind: 'unequal' };
    }
    return thenOutcome;
  }
  if (program.kind === 'fail') {
    return operands.length === 0 ? { kind: 'ok', stack: operands } : { kind: 'outstanding' };
  }
  throw new Error('unknown program');
}

function traceProgram(program, operands) {
  return program.kind === 'words' ? trace(program.words, operands) : null;
}

const structured = [
  {
    name: 'branch-equal-accounting',
    program: { kind: 'branch', thenArm: { kind: 'words', words: ['dup'] }, elseArm: { kind: 'words', words: ['dup'] } },
    operands: [data(0)],
  },
  {
    name: 'branch-unequal-accounting',
    program: { kind: 'branch', thenArm: { kind: 'words', words: ['dup'] }, elseArm: { kind: 'words', words: ['drop'] } },
    operands: [data(0)],
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
    operands: [resource(1), data(0)],
  },
  {
    name: 'branch-doubles-resource',
    program: {
      kind: 'branch',
      thenArm: { kind: 'words', words: ['swap'] },
      elseArm: { kind: 'words', words: ['dup'] },
    },
    operands: [resource(1), data(0)],
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
    name: 'fail-outstanding-operand',
    program: { kind: 'fail' },
    operands: [data(0)],
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
    trace: traceProgram(entry.program, entry.operands),
    decision: 'model-only',
    reason: entry.program.kind === 'fail' ? 'error-return' : 'branch-accounting',
  });
}

for (const entry of cases) {
  if (entry.decision === 'model-only' && !Object.hasOwn(REASONS, entry.reason)) {
    throw new Error(`${entry.name}: model-only reason ${entry.reason} is not declared`);
  }
  if (entry.decision !== 'model-only' && entry.reason !== '-') {
    throw new Error(`${entry.name}: production-analogue case carries reason ${entry.reason}`);
  }
}

// Lean rendering of the same rows. `Payload.data` takes the row's `Nat` tag.

function leanPayload(payload) {
  if (payload.kind === 'wrapped') return `.wrapped (${leanPayload(payload.inner)})`;
  if (payload.kind === 'data') return `.data ${payload.tag}`;
  return `.resource ${payload.tag}`;
}

function leanStack(stack) {
  return `[${stack.map(leanPayload).join(', ')}]`;
}

function leanProgram(program) {
  if (program.kind === 'words') {
    return program.words.map(word => `(.word .${word})`).reduce((left, right) => `(.compose ${left} ${right})`);
  }
  if (program.kind === 'branch') {
    return `(.branch ${leanProgram(program.thenArm)} ${leanProgram(program.elseArm)})`;
  }
  if (program.kind === 'fail') return '.fail';
  throw new Error('unknown program');
}

function leanOutcome(outcome) {
  return outcome === null ? 'none' : `some ${leanStack(outcome)}`;
}

function leanTrace(entries) {
  return entries === null ? 'none' : `some [${entries.map(leanStack).join(', ')}]`;
}

function row(entry) {
  const ops = entry.program.kind === 'words'
    ? entry.program.words.join('+')
    : entry.program.kind === 'branch'
      ? `branch(${entry.program.thenArm.words.join('+')}|${entry.program.elseArm.words.join('+')})`
      : 'fail';
  return [
    entry.name,
    `ops=${ops}`,
    `in=${renderStack(entry.operands)}`,
    `out=${entry.outcome === null ? '-' : renderStack(entry.outcome)}`,
    `trace=${renderTrace(entry.trace)}`,
    `decision=${entry.decision}`,
    `reason=${entry.reason}`,
  ].join(' ');
}

const rows = cases.map(row);

const caseHeader = [
  '# Ownership accounting corpus, generated from the Lean model (issue #59).',
  '# Fields: name ops=<program> in=<stack> out=<stack|-> trace=<stack;stack> decision=<accepted|rejected|model-only> reason=<token|->',
  '# stacks: comma-separated kinds or `empty`; kinds: d:<nat> a data payload (0 the I64 payload, 1 the text payload),',
  '# r:<nat> a tagged must-use resource payload, w:<kind> a recursively wrapped payload. The trace lists the incoming stack followed by the stack after each admitted',
  '# word, so it has one entry more than the word list exactly when the program is accepted; `-` marks a program that is',
  '# not a word sequence (a branch or the ordinary error return).',
  '# programs: w+w a word sequence, branch(then|else) a two-arm branch, fail a normal error return.',
  '# Every row is also decided by the Lean kernel in proofs/foundation/NobleFoundation/OwnershipCorpus.lean.',
  '# reason tokens: branch-accounting, error-return, quotation-program-value, wrapped-payload, operand-arity.',
].join('\n');

const counts = cases.reduce((tally, entry) => {
  tally[entry.decision] = (tally[entry.decision] || 0) + 1;
  return tally;
}, {});
const reasons = cases.reduce((tally, entry) => {
  if (entry.decision === 'model-only') tally[entry.reason] = (tally[entry.reason] || 0) + 1;
  return tally;
}, {});

const corpusHeader = [
  '/-',
  '  Ownership accounting corpus, decided by the kernel (issue #59).',
  '',
  '  Generated by `verification/ownership/cases.mjs`; do not edit by hand. Each row states that the model\'s own',
  '  executable checker `run` and per-word trace `runTrace` give the values recorded in the corpus row named in the',
  '  row\'s docstring, and the kernel decides the equation. A row whose expectation does not follow from the model fails',
  '  this build, so the JavaScript generator proposes nothing the kernel does not check.',
  '',
  '  Coverage: ' +
    `${cases.length} cases (` +
    `${counts.accepted ?? 0} accepted and ${counts.rejected ?? 0} rejected through the production acceptance machine, ` +
    `${counts['model-only'] ?? 0} model-only).` +
    ' Data payloads carry their `Nat` tag (0 the I64 payload, 1 the text payload) and resources carry their corpus',
  '  tag. The one trusted translation is reading a corpus row',
  '  into these terms. This is agreement at these rows, not a proof about `crates/noble-kernel/src/acceptance`.',
  '-/',
  'import NobleFoundation.OwnershipTrace',
  '',
  'namespace NobleFoundation.OwnershipCorpus',
  '',
  'open NobleFoundation.Ownership',
].join('\n');

const corpusRows = cases.map(entry => {
  const name = `case_${entry.name.replaceAll('-', '_')}`;
  const program = leanProgram(entry.program);
  const operands = leanStack(entry.operands);
  return [
    `/-- \`${entry.name}\` ${row(entry)} -/`,
    `theorem ${name} :`,
    `    run ${program} ${operands} = ${leanOutcome(entry.outcome)} ∧`,
    `      runTrace ${program} ${operands} = ${leanTrace(entry.trace)} := by`,
    '  decide',
    '',
  ].join('\n');
});

const corpusText = `${corpusHeader}\n\n${corpusRows.join('\n')}end NobleFoundation.OwnershipCorpus\n`;
const casesText = `${caseHeader}\n${rows.join('\n')}\n`;

const emissions = [
  [CASES, casesText],
  [CORPUS, corpusText],
];

if (checkOnly) {
  const drift = emissions.filter(([file, text]) => readFileSync(file, 'utf8') !== text);
  if (drift.length > 0) {
    process.stderr.write(`cases.mjs: committed corpus differs from the emission: ${drift.map(([file]) => file).join(', ')}\n`);
    process.exit(1);
  }
  process.stdout.write(JSON.stringify({ check: 'clean', files: emissions.map(([file]) => file), cases: cases.length, counts, reasons }) + '\n');
} else {
  for (const [file, text] of emissions) {
    mkdirSync(dirname(file), { recursive: true });
    writeFileSync(file, text);
  }
  process.stdout.write(JSON.stringify({
    cases: cases.length,
    counts,
    reasons,
    outputs: emissions.map(([file]) => file),
  }) + '\n');
}
