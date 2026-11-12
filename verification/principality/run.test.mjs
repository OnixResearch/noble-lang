// The principality lane's receipt checks. Synthetic reports: no cargo run.
import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { check, checkSigned } from './run.mjs';

function report(inline_accepted, call_accepted) {
  const words = ['dup', 'drop'];
  return {
    schema: 'noble-principality-bounded/v2',
    input_stacks: 3, context_programs: 5, words, max_words: 2, max_context_words: 1,
    max_depth: 4, value_types: ['I64', 'Bool'],
    plain: { pairs: 7, comparisons: 21, inline_accepted, call_accepted },
    contextual: { pairs: 10, comparisons: 30, inline_accepted, call_accepted },
    discrepancies: 0, retained: [], claim: 'bounded evidence over the stated universes; not a proof',
  };
}

test('a report whose two routes both accepted nothing is refused as vacuous', () => {
  // The routes agree, every pair was compared and nothing disagrees, so only
  // the positive-acceptance guard can refuse this report.
  assert.throws(() => check(report(0, 0)), /the run accepted no program; the comparison is vacuous/);
});

test('a report with acceptances on both routes passes', () => {
  assert.doesNotThrow(() => check(report(7, 7)));
});

test('a report whose routes disagree is still refused for the disagreement', () => {
  assert.throws(() => check(report(7, 6)), /the two routes accepted different counts/);
});

test('the signed report keeps its non-vacuity guards', () => {
  const signed = {
    schema: 'noble-principality-signed/v1',
    signatures: ['sig'],
    input_stacks: 2,
    counts: {
      bodies: 3, pairs: 3, comparisons: 4, accepted: 2, call_accepted: 2,
      expected_accepted: 2, instance_checks: 1, instance_accepted: 1, non_instance_refusals: 1,
    },
    discrepancies: 0, retained: [], claim: 'bounded evidence; not a proof',
  };
  assert.doesNotThrow(() => checkSigned(signed));
  assert.throws(() => checkSigned({ ...signed, counts: { ...signed.counts, non_instance_refusals: 0 } }),
    /the instance closure is vacuous/);
});
