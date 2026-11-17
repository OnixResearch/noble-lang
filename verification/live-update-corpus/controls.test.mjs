import { expect, test } from 'bun:test';
import path from 'node:path';
import {
  COMPARISON_DIRECTORY, COMPARISON_SCHEMA, EVIDENCE_DIRECTORY, RESULT_CLASS_IDS, ROOT, SCHEMA,
  readJson, validateCorpus, validateReceipt,
} from './corpus.mjs';
import { RELAX_PROFILE, validateComparisonReceipt, verifyComparison } from './comparison.mjs';
import { runComparisonControls, runControls } from './controls.mjs';

const exportDirectory = path.join(ROOT, EVIDENCE_DIRECTORY);
const comparisonDirectory = path.join(ROOT, COMPARISON_DIRECTORY);
const exported = readJson(path.join(exportDirectory, 'corpus.json'));
const exportReceipt = readJson(path.join(exportDirectory, 'receipt.json'));
const corpus = readJson(path.join(comparisonDirectory, 'corpus.json'));
const receipt = readJson(path.join(comparisonDirectory, 'receipt.json'));
const schema = readJson(path.join(ROOT, 'verification/live-update-corpus/schema.json'));
const summary = controls => controls.map(({ control, passed, expected_reason, observed_reasons }) => ({
  control, passed, expected_reason, observed_reasons,
}));
const byCase = id => corpus.scenarios.find(scenario => scenario.case_id === id);

test('the export matches revision v1 of the schema and the seven result classes', () => {
  expect(schema.properties.schema.enum).toEqual([SCHEMA, COMPARISON_SCHEMA]);
  expect(schema.$defs.resultClassId.enum).toEqual(RESULT_CLASS_IDS);
  expect(exported.schema).toBe(SCHEMA);
  expect(validateCorpus(exported)).toEqual([]);
  expect(validateReceipt(exportReceipt)).toEqual([]);
  expect(exported.scenarios.length).toBeGreaterThanOrEqual(3);
  expect(exported.scenarios.every(scenario => scenario.other_runtime.result_class === 'not-run'
    && scenario.other_runtime.reason.length > 0)).toBe(true);
  expect(Object.values(exportReceipt.result_class_counts).reduce((sum, count) => sum + count, 0))
    .toBe(exported.scenarios.length * 2);
});

test('export controls reject deliberate defects for their intended reasons', () => {
  expect(exportReceipt.controls?.passed).toBe(true);
  const report = runControls({ directory: exportDirectory, repositoryRoot: ROOT });
  expect(report.passed).toBe(true);
  expect(summary(report.controls)).toEqual(summary(exportReceipt.controls.controls));
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

test('the comparison verifies and runs at least three scenarios through both adapters', () => {
  expect(corpus.schema).toBe(COMPARISON_SCHEMA);
  expect(validateCorpus(corpus)).toEqual([]);
  expect(validateComparisonReceipt(receipt)).toEqual([]);
  expect(verifyComparison({ directory: comparisonDirectory, repositoryRoot: ROOT }).failures)
    .toEqual([]);

  // Both runtimes are compared under explicit, frozen profiles.
  expect(corpus.profile.sha256).toBe(exported.profile.sha256);
  expect(corpus.scenarios.every(scenario => scenario.other_runtime.profile.sha256
    === RELAX_PROFILE.sha256)).toBe(true);
  const measured = corpus.scenarios.filter(scenario =>
    !['unsupported', 'not-run'].includes(scenario.other_runtime.result_class)
    && scenario.other_runtime.admission_settings.length > 0
    && scenario.other_runtime.faithful_mapping.length > 0);
  expect(measured.length).toBeGreaterThanOrEqual(3);

  // The held root keeps Noble's pinned selection; Relax's per-call selection is an intentional
  // difference under its own profile, not a defect of either runtime.
  const held = byCase('noble-live-update/held-root-and-new-root');
  expect(held.comparison.result_class).toBe('supported-outcome');
  expect(held.expectations.every(row => row.status === 'holds')).toBe(true);
  expect(held.other_runtime.result_class).toBe('intentional-semantic-difference');
  expect(held.other_runtime.admission_settings.map(setting => setting.relax_contract))
    .toEqual(['supported-outcome']);
  expect(held.other_runtime.anticipated_differences
    .map(row => [row.dimension, row.relation])).toEqual([
    ['version-selection-boundary', 'intentional-semantic-difference'],
    ['held-root-mixing', 'intentional-semantic-difference'],
  ]);

  // The default-setting checked-before-execution failure is a Relax observation; the Noble column
  // of the same case keeps its own class.
  expect(corpus.comparison.relax.temporal_admission.map(row => [row.admission, row.outcome]))
    .toEqual([['default', 'fail'], ['check-first', 'pass']]);
  expect(byCase('noble-live-update/typed-cases-and-bounds').comparison.result_class)
    .toBe('supported-outcome');
  expect(receipt.observations_for_issue_10.relax_observations.length).toBe(1);
  expect(receipt.observations_for_issue_10.relax_observations.every(row =>
    row.noble_defect === false)).toBe(true);
  expect(receipt.observations_for_issue_10.noble_regressions).toEqual([]);

  // Ownership has no faithful Relax counterpart and stays visibly unsupported.
  const ownership = byCase('noble-live-update/ownership-duplicates-and-modules').other_runtime;
  expect(ownership.result_class).toBe('unsupported');
  expect(ownership.reason.length).toBeGreaterThan(0);
  expect(ownership.admission_settings).toEqual([]);

  // Every Relax setting was replayed from its retained edits and reproduced.
  expect(corpus.comparison.relax.replay.relations.length).toBe(
    corpus.scenarios.reduce((sum, scenario) => sum + scenario.other_runtime.admission_settings.length, 0));
  expect(corpus.comparison.relax.replay.relations.every(row => row.relation === 'reproduced'))
    .toBe(true);
});

test('comparison controls reject deliberate defects for their intended reasons', () => {
  expect(receipt.controls?.passed).toBe(true);
  const report = runComparisonControls({ directory: comparisonDirectory, repositoryRoot: ROOT });
  expect(report.passed).toBe(true);
  expect(summary(report.controls)).toEqual(summary(receipt.controls.controls));
  const outcomes = new Map(report.controls.map(row => [row.control, row]));
  for (const name of [
    'pristine-comparison-verifies',
    'stale-relax-record',
    'forged-relax-class',
    'mutated-relax-edit-binding',
    'mutated-relax-profile',
    'wrong-noble-export',
    'mutated-noble-profile',
    'replay-diverged',
    'relax-control-failed',
    'unbound-relax-timing',
    'private-path-leak',
    'source-excerpt-leak',
    'unlisted-relax-file',
  ]) {
    expect(outcomes.get(name)?.passed).toBe(true);
  }
});
