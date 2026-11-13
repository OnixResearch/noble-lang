//! Independent expectation generator for the four-way duplicate/discard permission model
//! (issue #112).
//!
//! This Node model independently transcribes the bounded proposal also represented in
//! `proofs/foundation/NobleFoundation/UsePermissions.lean`. Its fixed expected-decision rows are
//! stated separately from the evaluator below, never derived from Lean or the Rust acceptance
//! machine.
//! The production kernel has one `is_data` predicate and no separate duplicate or discard
//! permission, so each case also records whether the selected *source* profile can express it:
//!
//! * `analogue=source` cases are expressible today: every input position is a plain data value and
//!   the action is one word of the compiled fragment. The four `forms/*.noble` of this lane are the
//!   executed instances, and `measure.mjs` records what they compile to and compute.
//! * `analogue=none` cases name a permission class or a control flow the selected profile cannot
//!   express (a must-use, affine or must-handle position; a raw program position; a two-arm branch
//!   and the ordinary error return, which have no source word in this fragment). They stay
//!   model-only in the lane and are never counted as an executable production control.
//!
//! A decision is `accepted` when the model admits the action, `rejected` when an eligibility bit
//! refuses it, and `rejected-arity` when the action needs a position the stack does not have. The
//! arity rejection is a stack-shape fact, not a permission decision, and the lane keeps it separate.

import { writeFileSync, mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';

const OUTPUT = join(import.meta.dirname, 'cases.txt');

// Payload classes mirror `NobleFoundation.UsePermissions.Permission`.
const unrestricted = Object.freeze({ dup: true, drop: true });
const mustUse = Object.freeze({ dup: false, drop: false });
const atMostOnce = Object.freeze({ dup: false, drop: true });
const atLeastOnce = Object.freeze({ dup: true, drop: false });

const classes = [
  ['data', unrestricted],
  ['must-use', mustUse],
  ['affine', atMostOnce],
  ['must-handle', atLeastOnce],
];

// Expected *decisions* are frozen independently of `step`, `run`, `branch`, and the Lean
// evaluator below. These rows are the finite oracle: each class/action combination names the
// decision it should have before the implementation of the model is consulted.
const expectedSingle = {
  data: {
    dup: 'accepted', drop: 'accepted', swap: 'rejected-arity', dip: 'rejected-arity',
    quote: 'accepted', compose: 'rejected-arity', run: 'rejected-arity',
  },
  'must-use': {
    dup: 'rejected', drop: 'rejected', swap: 'rejected-arity', dip: 'rejected-arity',
    quote: 'rejected', compose: 'rejected-arity', run: 'rejected-arity',
  },
  affine: {
    dup: 'rejected', drop: 'accepted', swap: 'rejected-arity', dip: 'rejected-arity',
    quote: 'rejected', compose: 'rejected-arity', run: 'rejected-arity',
  },
  'must-handle': {
    dup: 'accepted', drop: 'rejected', swap: 'rejected-arity', dip: 'rejected-arity',
    quote: 'rejected', compose: 'rejected-arity', run: 'rejected-arity',
  },
};

// `Permission.combine` is the pointwise conjunction.
const combine = (left, right) => ({
  dup: left.dup && right.dup,
  drop: left.drop && right.drop,
});

// Recursive shape of List<Pair<Data, Sum<Variant<T, Data>, Data>>>. The wrappers only expose
// payload permissions; they do not grant new bits.
const nestedPayload = inner => combine(unrestricted,
  combine(combine(inner, unrestricted), unrestricted));

// `Permission.capturable` is `dup && drop`.
const capturable = permission => permission.dup && permission.drop;

let nextTag = 1;
const value = permission => ({ kind: 'value', tag: nextTag++, permission });
const program = () => ({ kind: 'program', tag: nextTag++, permission: unrestricted });

const isProgram = item => item.kind === 'program';

// `step` mirrors `NobleFoundation.UsePermissions.step`. The head of the stack is the position the
// word constrains, exactly as the Lean table orders it.
function step(word, stack) {
  const [first, second, ...rest] = stack;
  const tail = second === undefined ? [] : [second, ...rest];
  switch (word) {
    case 'dup':
      if (first === undefined) return { kind: 'arity' };
      if (!first.permission.dup) return { kind: 'eligibility' };
      return { kind: 'ok', stack: [first, first, ...tail] };
    case 'drop':
      if (first === undefined) return { kind: 'arity' };
      if (!first.permission.drop) return { kind: 'eligibility' };
      return { kind: 'ok', stack: tail };
    case 'swap':
      if (first === undefined || second === undefined) return { kind: 'arity' };
      return { kind: 'ok', stack: [second, first, ...rest] };
    case 'dip':
      if (first === undefined || second === undefined) return { kind: 'arity' };
      if (!isProgram(second)) return { kind: 'arity' };
      return { kind: 'ok', stack: [first, ...rest] };
    case 'quote':
      if (first === undefined) return { kind: 'arity' };
      if (!capturable(first.permission)) return { kind: 'eligibility' };
      return { kind: 'ok', stack: [{ kind: 'program', tag: first.tag, permission: unrestricted },
        ...tail] };
    case 'compose':
      if (first === undefined || second === undefined) return { kind: 'arity' };
      if (!isProgram(first) || !isProgram(second)) return { kind: 'arity' };
      return { kind: 'ok', stack: [{ kind: 'program', tag: first.tag, permission: unrestricted },
        ...rest] };
    case 'run':
      if (first === undefined) return { kind: 'arity' };
      if (!isProgram(first)) return { kind: 'arity' };
      return { kind: 'ok', stack: tail };
    default:
      throw new Error(`unknown word ${word}`);
  }
}

function run(words, stack) {
  let current = stack;
  for (const word of words) {
    const outcome = step(word, current);
    if (outcome.kind !== 'ok') return outcome;
    current = outcome.stack;
  }
  return { kind: 'ok', stack: current };
}

// `fail` mirrors `NobleFoundation.UsePermissions.fail`: the error return discards the stack, so it
// is admitted only when every position is discardable.
function errorReturn(stack) {
  if (stack.every(item => item.permission.drop)) return { kind: 'ok', stack: [] };
  return { kind: 'eligibility' };
}

// `branch` mirrors `NobleFoundation.UsePermissions.branch`: both arms run from the same stack, and
// the branch is admitted only when both arms are admitted and leave the identical stack.
function branch(thenArm, elseArm, stack) {
  const thenOutcome = run(thenArm, stack);
  const elseOutcome = run(elseArm, stack);
  if (thenOutcome.kind !== 'ok' || elseOutcome.kind !== 'ok') return { kind: 'eligibility' };
  if (JSON.stringify(thenOutcome.stack) !== JSON.stringify(elseOutcome.stack)) {
    return { kind: 'join' };
  }
  return { kind: 'ok', stack: thenOutcome.stack };
}

function apply(action, stack) {
  if (action.kind === 'words') return run(action.words, stack);
  if (action.kind === 'fail') return errorReturn(stack);
  if (action.kind === 'branch') return branch(action.thenArm, action.elseArm, stack);
  throw new Error('unknown action');
}

// A case has a source analogue when every input position is a plain data value and the action is
// one word of the compiled fragment. Nothing else is expressible in the selected profile: there is
// no must-use, affine or must-handle word, no source spelling for a raw program position, and no
// ordinary-error-return or two-arm-branch word in this fragment.
function analogue(action, stack) {
  if (action.kind !== 'words' || action.words.length !== 1) return 'none';
  if (!stack.every(item => item.kind === 'value' && item.permission === unrestricted)) {
    return 'none';
  }
  return 'source';
}

const render = action => {
  if (action.kind === 'words') return action.words.join('+') || 'empty';
  if (action.kind === 'branch') {
    return `branch(${action.thenArm.join('+') || 'empty'}|${action.elseArm.join('+') || 'empty'})`;
  }
  return 'fail';
};

const kindOf = item => `${item.kind === 'program' ? 'p' : 'v'}:${describe(item.permission)}`;

function describe(permission) {
  if (permission.dup && permission.drop) return 'data';
  if (!permission.dup && !permission.drop) return 'must-use';
  if (!permission.dup && permission.drop) return 'affine';
  return 'must-handle';
}

const decisionOf = outcome => {
  if (outcome.kind === 'ok') return 'accepted';
  if (outcome.kind === 'arity') return 'rejected-arity';
  return 'rejected';
};

const cases = [];

const push = (name, action, stack, expected) => {
  const outcome = apply(action, stack);
  const decision = decisionOf(outcome);
  if (decision !== expected) {
    throw new Error(`${name}: expected ${expected}, model returned ${decision}`);
  }
  cases.push({
    name,
    action,
    stack,
    expected,
    decision,
    analogue: analogue(action, stack),
    output: outcome.kind === 'ok' ? outcome.stack : [],
  });
};

// Every word over every single-position class, plus the shapes that need two positions.
const words = ['dup', 'drop', 'swap', 'dip', 'quote', 'compose', 'run'];
for (const [className, permission] of classes) {
  for (const word of words) {
    push(`${word}-${className}`, { kind: 'words', words: [word] }, [value(permission)],
      expectedSingle[className][word]);
  }
  push(`swap-${className}-with-data`, { kind: 'words', words: ['swap'] },
    [value(permission), value(unrestricted)], 'accepted');
  push(`dip-${className}-held-with-program`, { kind: 'words', words: ['dip'] },
    [value(permission), program()], 'accepted');
  push(`quote-${className}-then-dup-program`, { kind: 'words', words: ['quote', 'dup'] },
    [value(permission)], className === 'data' ? 'accepted' : 'rejected');
  push(`drop-${className}-then-swap`, { kind: 'words', words: ['drop', 'swap'] },
    [value(permission), value(unrestricted)],
    className === 'data' || className === 'affine' ? 'rejected-arity' : 'rejected');
}
push('swap-program-pair', { kind: 'words', words: ['swap'] }, [program(), program()], 'accepted');
push('compose-program-pair', { kind: 'words', words: ['compose'] }, [program(), program()], 'accepted');
push('run-program-then-data', { kind: 'words', words: ['run'] },
  [program(), value(unrestricted)], 'accepted');
push('dup-program', { kind: 'words', words: ['dup'] }, [program()], 'accepted');
push('drop-program', { kind: 'words', words: ['drop'] }, [program()], 'accepted');
push('quote-program', { kind: 'words', words: ['quote'] }, [program()], 'accepted');

// The recursive combine, as the two-position shapes it produces.
push('combine-data-data', { kind: 'words', words: ['dup'] },
  [value(combine(unrestricted, unrestricted))], 'accepted');
push('combine-data-must-use', { kind: 'words', words: ['dup'] },
  [value(combine(unrestricted, mustUse))], 'rejected');
push('combine-must-use-must-use', { kind: 'words', words: ['drop'] },
  [value(combine(mustUse, mustUse))], 'rejected');
push('combine-affine-data', { kind: 'words', words: ['drop'] },
  [value(combine(atMostOnce, unrestricted))], 'accepted');
push('combine-must-handle-data', { kind: 'words', words: ['drop'] },
  [value(combine(atLeastOnce, unrestricted))], 'rejected');
push('nested-wrapper-data-quote', { kind: 'words', words: ['quote'] },
  [value(nestedPayload(unrestricted))], 'accepted');
push('nested-wrapper-must-use-dup', { kind: 'words', words: ['dup'] },
  [value(nestedPayload(mustUse))], 'rejected');
push('nested-wrapper-affine-drop', { kind: 'words', words: ['drop'] },
  [value(nestedPayload(atMostOnce))], 'accepted');
push('nested-wrapper-must-handle-drop', { kind: 'words', words: ['drop'] },
  [value(nestedPayload(atLeastOnce))], 'rejected');

// The ordinary error return.
for (const [className, permission] of classes) {
  push(`fail-${className}`, { kind: 'fail' }, [value(permission)],
    className === 'data' || className === 'affine' ? 'accepted' : 'rejected');
}
push('fail-data-and-must-use', { kind: 'fail' }, [value(unrestricted), value(mustUse)], 'rejected');
push('fail-empty', { kind: 'fail' }, [], 'accepted');

// The two-arm join: equal arms, a missing obligation, a duplicated obligation, and an arm that
// discards a position the other keeps.
push('branch-equal-arms', { kind: 'branch', thenArm: ['swap'], elseArm: ['swap'] },
  [value(unrestricted), value(unrestricted)], 'accepted');
push('branch-missing-obligation', { kind: 'branch', thenArm: ['dup'], elseArm: [] },
  [value(unrestricted)], 'rejected');
push('branch-duplicated-obligation', { kind: 'branch', thenArm: ['dup'], elseArm: [] },
  [value(atLeastOnce)], 'rejected');
push('branch-discards-must-use', { kind: 'branch', thenArm: ['swap'], elseArm: ['drop'] },
  [value(mustUse), value(unrestricted)], 'rejected');
push('branch-keeps-program', { kind: 'branch', thenArm: ['compose'], elseArm: ['compose'] },
  [program(), program()], 'accepted');

const lines = cases.map(entry => {
  const incoming = entry.stack.map(kindOf).join(',');
  const outgoing = entry.output.map(kindOf).join(',');
  return [
    entry.name,
    `action=${render(entry.action)}`,
    `in=${incoming || 'empty'}`,
    `out=${outgoing || 'empty'}`,
    `decision=${entry.decision}`,
    `expected=${entry.expected}`,
    `analogue=${entry.analogue}`,
  ].join(' ');
});

const header = [
  '# Independent Node expectation corpus for the four-way permission model (issue #112).',
  '# Fields: name action=<word+...+ | branch(then|else) | fail> in=<position[,position]> out=<position[,position]|empty>',
  '#         decision=<accepted|rejected|rejected-arity> expected=<same values> analogue=<source|none>',
  '# positions: v:<class> a value position, p:<class> a program position; classes data, must-use, affine, must-handle.',
].join('\n');

mkdirSync(dirname(OUTPUT), { recursive: true });
writeFileSync(OUTPUT, `${header}\n${lines.join('\n')}\n`);

const counts = cases.reduce((tally, entry) => {
  tally[entry.decision] = (tally[entry.decision] || 0) + 1;
  tally[`analogue:${entry.analogue}`] = (tally[`analogue:${entry.analogue}`] || 0) + 1;
  return tally;
}, {});
process.stdout.write(JSON.stringify({ cases: cases.length, counts, output: OUTPUT }) + '\n');
