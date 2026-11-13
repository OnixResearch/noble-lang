import { expect, test } from 'bun:test';
import path from 'node:path';
import {
  EVIDENCE_DIRECTORY, RESULT_CLASS_IDS, ROOT, SCHEMA, readJson, validateCorpus, validateReceipt,
} from './corpus.mjs';
import { runControls } from './controls.mjs';

const directory = path.join(ROOT, EVIDENCE_DIRECTORY);
const corpus = readJson(path.join(directory, 'corpus.json'));
const receipt = readJson(path.join(directory, 'receipt.json'));
const schema = readJson(path.join(ROOT, 'verification/live-update-corpus/schema.json'));

test('the retained report matches the versioned schema and seven result classes', () => {
  expect(schema.properties.schema.const).toBe(SCHEMA);
  expect(schema.$defs.resultClass.properties.id.enum).toEqual(RESULT_CLASS_IDS);
  expect(validateCorpus(corpus)).toEqual([]);
  expect(validateReceipt(receipt)).toEqual([]);
  expect(corpus.scenarios.length).toBeGreaterThanOrEqual(3);
  expect(corpus.scenarios.every(scenario => scenario.other_runtime.result_class === 'not-run'
    && scenario.other_runtime.reason.length > 0)).toBe(true);
  expect(Object.values(receipt.result_class_counts).reduce((sum, count) => sum + count, 0))
    .toBe(corpus.scenarios.length * 2);
});

test('retained corpus controls reject deliberate defects for their intended reasons', () => {
  expect(receipt.controls?.passed).toBe(true);
  const report = runControls({ directory, repositoryRoot: ROOT });
  const summary = controls => controls.map(({ control, passed, expected_reason, observed_reasons }) => ({
    control, passed, expected_reason, observed_reasons,
  }));
  expect(report.passed).toBe(true);
  expect(summary(report.controls)).toEqual(summary(receipt.controls.controls));

  const outcomes = new Map(report.controls.map(row => [row.control, row]));
  for (const name of [
    'wrong-version-observation',
    'premature-retirement-observation',
    'stale-raw-evidence',
    'mutated-event-binding',
    'mutated-edit-binding',
    'resealed-effect-mutation',
    'mutated-profile',
    'mutated-expectation',
    'canonical-state-promotion',
    'canonical-lslot-states-unchanged',
    'pristine-evidence-verifies',
  ]) {
    expect(outcomes.get(name)?.passed).toBe(true);
  }
});
