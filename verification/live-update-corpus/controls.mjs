/**
 * Controls for the bounded live-update corpus (#119).
 *
 * Each control is a deliberate defect injected into either the semantic layer (an observation that
 * contradicts a frozen expectation) or the evidence layer (raw bytes, an event binding, the profile
 * or the canonical case states). A control passes only when the corpus rejects it *for its intended
 * reason*; a rejection for an unrelated reason is reported and fails the control.
 *
 * `runControls` is called by `export.mjs` on the fresh export directory and by `controls.test.mjs`
 * on the committed evidence directory. It never mutates its input: every mutating control works on
 * a temporary copy of the evidence directory.
 */
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {
  SCENARIOS, canonicalPromotions, canonicalSnapshot, classifyScenario, digestValue,
  evaluateExpectations, readJson, sha256, stableJson, verifyEvidence,
} from './corpus.mjs';

const SUITE = 'verification/live-update-corpus/controls.mjs';

function copyEvidence(directory, receiptDocument) {
  const target = fs.mkdtempSync(path.join(os.tmpdir(), 'noble-live-update-control-'));
  fs.cpSync(directory, target, { recursive: true, dereference: false });
  if (!fs.existsSync(path.join(target, 'receipt.json')))
    fs.writeFileSync(path.join(target, 'receipt.json'),
      `${JSON.stringify({ ...receiptDocument, controls: undefined }, null, 2)}\n`);
  return target;
}

/**
 * Recompute the receipt's corpus digest and its own binding over edited evidence. Mutating controls
 * reseal the mechanically refittable bindings on purpose: what is left is the check that must catch
 * the defect.
 */
function reseal(directory, receiptDocument) {
  const receipt = structuredClone(receiptDocument);
  const corpusPath = path.join(directory, 'corpus.json');
  receipt.corpus = { ...receipt.corpus, sha256: sha256(fs.readFileSync(corpusPath)) };
  delete receipt.binding_sha256;
  receipt.binding_sha256 = digestValue(receipt);
  fs.writeFileSync(path.join(directory, 'receipt.json'), `${JSON.stringify(receipt, null, 2)}\n`);
  return { receipt, corpus: readJson(corpusPath) };
}

const failureCodes = result => [...new Set(result.failures.map(row => row.code))].sort();

/** `expected` is a single code, a list of codes, or null for "must pass". */
function judge(control, expected, result) {
  const observed = failureCodes(result);
  if (expected === null)
    return { control, expected_reason: null, observed_reasons: observed,
      passed: observed.length === 0,
      detail: observed.length === 0 ? ['evidence verified'] : result.failures.slice(0, 4) };
  const wanted = Array.isArray(expected) ? expected : [expected];
  return { control, expected_reason: wanted.join('|'), observed_reasons: observed,
    passed: wanted.some(code => observed.includes(code)),
    detail: observed.length === 0 ? ['no failure was reported'] : result.failures.slice(0, 4) };
}

/** A semantic control: inject a wrong observation and require the frozen expectation to catch it. */
function classification({ control, caseId, observations, mutate }, expectationId) {
  const scenario = SCENARIOS.find(row => row.case_id === caseId);
  assert.ok(scenario, `no declared scenario ${caseId}`);
  const forged = structuredClone(observations);
  mutate(forged);
  const expectations = evaluateExpectations(scenario, forged);
  const row = expectations.find(entry => entry.expectation_id === expectationId);
  assert.ok(row, `expectation ${expectationId} is not declared by ${caseId}`);
  const verdict = classifyScenario(expectations, []);
  return {
    control,
    expected_reason: expectationId,
    observed_reasons: [row.status === 'mismatch' ? `mismatch:${expectationId}` : `status:${row.status}`,
      `class:${verdict.result_class}`],
    passed: row.status === 'mismatch' && verdict.result_class === 'contract-violation',
    detail: row.status === 'mismatch'
      ? [`declared ${stableJson(row.declared)}`, `observed ${stableJson(row.observed)}`]
      : ['the forged observation did not contradict the expectation'],
  };
}

export function runControls({ directory, repositoryRoot = undefined, receipt = null, corpus = null }) {
  assert.ok(fs.existsSync(path.join(directory, 'corpus.json')),
    'the controls need an exported corpus directory');
  const corpusDocument = corpus ?? readJson(path.join(directory, 'corpus.json'));
  // The receipt may still be in memory when the exporter runs the suite, before it is written.
  const baseReceipt = receipt ?? readJson(path.join(directory, 'receipt.json'));
  const controls = [];

  const heldRoot = corpusDocument.scenarios
    .find(row => row.case_id === 'noble-live-update/held-root-and-new-root');
  controls.push(classification({
    control: 'wrong-version-observation',
    caseId: heldRoot.case_id,
    observations: heldRoot.observations,
    mutate: observations => {
      // The held root is claimed to have selected the definition the *new* root selected.
      observations.coherent.held_effect_definition = observations.coherent.fresh_effect_definition;
    },
  }, 'held-root.held-root-keeps-pinned-definition'));

  const retention = corpusDocument.scenarios
    .find(row => row.case_id === 'noble-live-update/saved-program-and-package-retention');
  controls.push(classification({
    control: 'premature-retirement-observation',
    caseId: retention.case_id,
    observations: retention.observations,
    mutate: observations => {
      // M1's span is claimed to retire while the saved owner still reaches it.
      observations.retention.replacement.publication_while_owned.retire_spans = [
        { generation: 1, start: 4, length: 5 },
      ];
    },
  }, 'retention.no-retirement-while-owned'));

  // A raw report byte changes and nothing is resealed: the digest binding must reject it.
  {
    const copy = copyEvidence(directory, baseReceipt);
    const target = path.join(copy, 'raw', 'live_slot_package_retention',
      'LREF-CASE-03-replacement.responses.jsonl');
    const mutated = Buffer.from(fs.readFileSync(target));
    mutated[mutated.length - 2] = (mutated[mutated.length - 2] + 1) % 256;
    fs.writeFileSync(target, mutated);
    controls.push(judge('stale-raw-evidence', 'stale-evidence',
      verifyEvidence({ directory: copy, repositoryRoot })));
    fs.rmSync(copy, { recursive: true, force: true });
  }

  // The event binding itself changes while the underlying retained bytes stay fixed.
  {
    const copy = copyEvidence(directory, baseReceipt);
    const corpusEdited = readJson(path.join(copy, 'corpus.json'));
    const scenario = corpusEdited.scenarios
      .find(row => row.case_id === 'noble-live-update/relocation-and-effect-prefix');
    const binding = scenario.event_binding.files[0];
    binding.sha256 = `${binding.sha256[0] === '0' ? '1' : '0'}${binding.sha256.slice(1)}`;
    fs.writeFileSync(path.join(copy, 'corpus.json'), `${JSON.stringify(corpusEdited, null, 2)}\n`);
    reseal(copy, baseReceipt);
    controls.push(judge('mutated-event-binding', 'event-binding-mismatch',
      verifyEvidence({ directory: copy, repositoryRoot })));
    fs.rmSync(copy, { recursive: true, force: true });
  }

  // An edit changes (a request value) and every mechanically refittable binding except the event
  // binding is resealed: the event binding must still reject the comparison evidence.
  {
    const copy = copyEvidence(directory, baseReceipt);
    const rawRoot = path.join(copy, 'raw');
    const laneDirectory = fs.readdirSync(rawRoot).find(name => name === 'live_slot_ownership_cases');
    const target = path.join(rawRoot, laneDirectory, 'OWN-CASE-02-A-then-B.requests.jsonl');
    const lines = fs.readFileSync(target, 'utf8').trim().split('\n');
    lines[lines.length - 1] = lines.at(-1).replace('"value":"41"', '"value":"40"');
    fs.writeFileSync(target, `${lines.join('\n')}\n`);
    const corpusEdited = readJson(path.join(copy, 'corpus.json'));
    const scenario = corpusEdited.scenarios
      .find(row => row.case_id === 'noble-live-update/ownership-duplicates-and-modules');
    const relative = path.relative(copy, target);
    const bytes = fs.readFileSync(target);
    for (const entry of scenario.provenance)
      if (entry.path === relative) Object.assign(entry, { sha256: sha256(bytes), bytes: bytes.length });
    for (const variant of scenario.edits.variants)
      if (variant.variant === 'OWN-CASE-02-A-then-B') variant.requests_sha256 = sha256(bytes);
    fs.writeFileSync(path.join(copy, 'corpus.json'), `${JSON.stringify(corpusEdited, null, 2)}\n`);
    reseal(copy, baseReceipt); // deliberately leaves scenario.event_binding.sha256 alone
    controls.push(judge('mutated-edit-binding', 'event-binding-mismatch',
      verifyEvidence({ directory: copy, repositoryRoot })));
    fs.rmSync(copy, { recursive: true, force: true });
  }

  // An effect binding changes and *everything* mechanically derived is resealed: only the semantic
  // re-derivation can notice, and it must.
  {
    const copy = copyEvidence(directory, baseReceipt);
    const target = path.join(copy, 'raw', 'live_slot_relocation_witness',
      'reloc-old.responses.jsonl');
    const lines = fs.readFileSync(target, 'utf8').trim().split('\n');
    let changed = false;
    const mutated = lines.map(line => {
      const row = JSON.parse(line);
      for (const event of row.request_trace ?? []) {
        if (event.operation !== 'effect' || event.request !== 'relocated') continue;
        event.request = 'relocated-forged';
        event.response = 'emitted:relocated-forged';
        event.policyInput.request = 'relocated-forged';
        changed = true;
      }
      return JSON.stringify(row);
    });
    assert.equal(changed, true, 'effect-mutation control must change an observed effect');
    fs.writeFileSync(target, `${mutated.join('\n')}\n`);
    const corpusEdited = readJson(path.join(copy, 'corpus.json'));
    const scenario = corpusEdited.scenarios
      .find(row => row.case_id === 'noble-live-update/relocation-and-effect-prefix');
    const relative = path.relative(copy, target);
    const bytes = fs.readFileSync(target);
    for (const entry of scenario.provenance)
      if (entry.path === relative) Object.assign(entry, { sha256: sha256(bytes), bytes: bytes.length });
    for (const entry of scenario.event_binding.files)
      if (entry.path === relative) entry.sha256 = sha256(bytes);
    scenario.event_binding.sha256 = sha256(scenario.event_binding.files
      .map(entry => entry.sha256).join('\n'));
    fs.writeFileSync(path.join(copy, 'corpus.json'), `${JSON.stringify(corpusEdited, null, 2)}\n`);
    reseal(copy, baseReceipt);
    const result = verifyEvidence({ directory: copy, repositoryRoot });
    const semantic = ['derivation-drift', 'expectation-drift', 'comparison-drift'];
    const observed = failureCodes(result);
    controls.push({
      control: 'resealed-effect-mutation',
      expected_reason: semantic.join('|'),
      observed_reasons: observed,
      passed: observed.some(code => semantic.includes(code)),
      detail: result.failures.filter(row => semantic.includes(row.code)).slice(0, 4),
    });
    fs.rmSync(copy, { recursive: true, force: true });
  }

  // The profile changes and the receipts are resealed around it: the live profile binding must
  // reject the evidence, because comparison evidence is frozen to one profile.
  {
    const copy = copyEvidence(directory, baseReceipt);
    const profilePath = path.join(copy, 'profile.json');
    const profile = readJson(profilePath);
    profile.dimensions.reclamation.quota_bound.maximum = 9000;
    fs.writeFileSync(profilePath, `${JSON.stringify(profile, null, 2)}\n`);
    const digest = sha256(fs.readFileSync(profilePath));
    const { receipt: resealedReceipt } = reseal(copy, baseReceipt);
    resealedReceipt.profile = { ...resealedReceipt.profile, sha256: digest };
    delete resealedReceipt.binding_sha256;
    resealedReceipt.binding_sha256 = digestValue(resealedReceipt);
    fs.writeFileSync(path.join(copy, 'receipt.json'),
      `${JSON.stringify(resealedReceipt, null, 2)}\n`);
    controls.push(judge('mutated-profile', 'profile-changed',
      verifyEvidence({ directory: copy, repositoryRoot })));
    fs.rmSync(copy, { recursive: true, force: true });
  }

  // A declared expectation changes and the receipts are resealed: the recorded status must no
  // longer agree with the frozen declaration.
  {
    const copy = copyEvidence(directory, baseReceipt);
    const corpusEdited = readJson(path.join(copy, 'corpus.json'));
    const scenario = corpusEdited.scenarios
      .find(row => row.case_id === 'noble-live-update/typed-cases-and-bounds');
    scenario.expectations.find(entry => entry.expectation_id === 'typed.retention-saturation')
      .declared = { bound: 256, retained: 256, refusal_stage: 'command' };
    fs.writeFileSync(path.join(copy, 'corpus.json'), `${JSON.stringify(corpusEdited, null, 2)}\n`);
    reseal(copy, baseReceipt);
    controls.push(judge('mutated-expectation', 'expectation-drift',
      verifyEvidence({ directory: copy, repositoryRoot })));
    fs.rmSync(copy, { recursive: true, force: true });
  }

  // A canonical case is promoted inside the recorded snapshot and the receipts are resealed: the
  // promotion guard must reject it.
  {
    const copy = copyEvidence(directory, baseReceipt);
    const corpusEdited = readJson(path.join(copy, 'corpus.json'));
    corpusEdited.canonical_cases.states['LSLOT-02'].implementation = 'present';
    fs.writeFileSync(path.join(copy, 'corpus.json'), `${JSON.stringify(corpusEdited, null, 2)}\n`);
    reseal(copy, baseReceipt);
    controls.push(judge('canonical-state-promotion', 'canonical-case-promoted',
      verifyEvidence({ directory: copy, repositoryRoot })));
    fs.rmSync(copy, { recursive: true, force: true });
  }

  // The guard itself: the live canonical cases are unchanged and none is promoted.
  {
    const recorded = corpusDocument.canonical_cases;
    const failures = canonicalPromotions(recorded.states);
    let liveMatch = true;
    if (repositoryRoot !== undefined) {
      const live = canonicalSnapshot(repositoryRoot);
      liveMatch = live.sha256 === recorded.sha256
        && stableJson(live.states) === stableJson(recorded.states);
      failures.push(...canonicalPromotions(live.states));
    }
    controls.push({
      control: 'canonical-lslot-states-unchanged',
      expected_reason: null,
      observed_reasons: failures.map(row => row.code),
      passed: failures.length === 0 && liveMatch,
      detail: liveMatch ? ['no canonical case is promoted']
        : ['the live canonical case file differs from the recorded snapshot'],
    });
  }

  // Pristine verification runs last, against a receipt that already carries the computed controls,
  // because the receipt schema requires a control section. When the receipt came from disk its own
  // binding is verified too; an in-memory receipt has not been written yet.
  const pristineReceipt = receipt === null ? null : (() => {
    const copy = structuredClone(receipt);
    copy.controls = { suite: SUITE, controls, passed: controls.every(row => row.passed) };
    delete copy.binding_sha256;
    copy.binding_sha256 = digestValue(copy);
    return copy;
  })();
  const pristine = judge('pristine-evidence-verifies', null,
    verifyEvidence({ directory, repositoryRoot, receipt: pristineReceipt, corpus: corpusDocument }));

  const report = {
    suite: SUITE,
    controls: [pristine, ...controls],
    passed: pristine.passed && controls.every(row => row.passed),
  };
  return report;
}

/** Standalone report: prints the suite and exits nonzero when any control fails. */
if (import.meta.main) {
  const directory = path.resolve(process.argv[2]
    ?? 'verification/live-update-corpus/evidence');
  const root = path.resolve(process.argv[3] ?? '.');
  const report = runControls({ directory, repositoryRoot: root });
  process.stdout.write(`${JSON.stringify(report, null, 2)}\n`);
  if (!report.passed) process.exitCode = 1;
}
