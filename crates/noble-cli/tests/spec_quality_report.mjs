import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

// Real selected-CLI coverage of the specification-quality report producer (issue #65). Each
// refusal class must arrive as its own diagnostic, and the report must keep specification
// quality, partial correctness, termination and correspondence separate.

const binary = process.argv[2];
assert.ok(binary, 'pass the selected noble CLI binary');
const workdir = fs.mkdtempSync(path.join(os.tmpdir(), 'noble-spec-quality-'));

const profile = {
  schema: 'noble-mc2-spec-quality/v1',
  predicate_revision: 1,
  model_revision: 1,
  observation_relation: 'exact-scalar-equality',
  parameter_binding: 'ordered-exact-i64-bool',
};

function report(name, document) {
  const file = path.join(workdir, `${name}.json`);
  fs.writeFileSync(file, typeof document === 'string' ? document : JSON.stringify(document));
  const child = spawn(binary, ['spec-quality', file], { encoding: 'utf8' });
  let stdout = '';
  let stderr = '';
  child.stdout.on('data', chunk => { stdout += chunk; });
  child.stderr.on('data', chunk => { stderr += chunk; });
  return new Promise(resolve => {
    child.on('close', code => {
      assert.equal(stderr, '', `unexpected stderr for ${name}`);
      resolve({ status: code ?? 2, document: JSON.parse(stdout) });
    });
  });
}

const obligation = (claim, kind, checked, vacuous = false) => ({ claim, obligation: kind, checked, vacuous });
const specification = claim => obligation(claim, 'specification-quality', true);
const withBindings = overrides => ({ ...profile, obligations: [], ...overrides });

const refused = async (name, document, outcome) => {
  const { status, document: reported } = await report(name, document);
  assert.equal(status, 2, `${name} must refuse`);
  assert.equal(reported.outcome, outcome, `${name}: ${JSON.stringify(reported)}`);
  assert.equal(reported.schema, 'noble-mc2-spec-quality/v1');
  assert.equal(reported.guest_requests, 0, `${name} performs no guest execution`);
  assert.equal(reported.protected_operations, 0, `${name} performs no guest execution`);
  assert.ok(reported.diagnostic.length > 0, `${name} names its refusal`);
  return reported.outcome;
};

const outcomes = new Set();
outcomes.add(await refused('wrong-obligation',
  withBindings({ obligations: [obligation('realizable', 'implementation-satisfaction', true)] }),
  'wrong-obligation'));
outcomes.add(await refused('implementation-as-specification',
  withBindings({ obligations: [obligation('satisfied', 'specification-quality', true)] }),
  'wrong-obligation'));
outcomes.add(await refused('unknown-claim',
  withBindings({ obligations: [obligation('terminates', 'specification-quality', true)] }),
  'invalid-manifest'));
outcomes.add(await refused('wrong-predicate-revision',
  withBindings({ predicate_revision: 2 }), 'wrong-predicate-revision'));
outcomes.add(await refused('wrong-model-revision',
  withBindings({ model_revision: 2 }), 'wrong-model-revision'));
outcomes.add(await refused('coarser-observation-relation',
  withBindings({ observation_relation: 'to-int-equality' }), 'wrong-observation-relation'));
outcomes.add(await refused('wrong-parameter-binding',
  withBindings({ parameter_binding: 'unordered-any' }), 'wrong-parameter-binding'));
outcomes.add(await refused('unknown-field',
  withBindings({ solver: 'z3' }), 'invalid-manifest'));
outcomes.add(await refused('missing-binding',
  (() => { const { observation_relation: _, ...rest } = profile; return { ...rest, obligations: [] }; })(),
  'invalid-manifest'));
outcomes.add(await refused('non-boolean-checked',
  withBindings({ obligations: [{ claim: 'inhabited', obligation: 'specification-quality', checked: 'yes' }] }),
  'invalid-manifest'));
// Distinct classes never collapse into one another.
assert.equal(outcomes.size, 6, `refusal classes collapsed: ${[...outcomes].join(',')}`);

// A reported submission keeps the four obligations apart and never upgrades an unchecked claim.
const { status, document } = await report('reported', withBindings({
  obligations: [
    specification('inhabited'),
    specification('realizable'),
    obligation('deterministic', 'specification-quality', false),
    obligation('satisfied', 'implementation-satisfaction', false),
  ],
}));
assert.equal(status, 0, 'a consistent submission is reported');
assert.equal(document.outcome, 'reported');
assert.equal(document.specification_quality.established, 2);
assert.equal(document.specification_quality.not_established, 2);
assert.equal(document.specification_quality.vacuous, 0);
assert.equal(document.partial_correctness.substituted_by_specification_quality, false,
  'specification quality never discharges the implementation claim');
assert.equal(document.partial_correctness.reported, false,
  'an unchecked implementation claim is not reported as established');
assert.equal(document.termination.claimed, false, 'termination is never claimed');
assert.equal(document.correspondence.claimed, false, 'correspondence is never claimed');
assert.equal(document.observation_relation, profile.observation_relation,
  'the report states the observation relation it used');

// A checked implementation claim is reported, and still does not move termination or correspondence.
const checkedImplementation = await report('implementation', withBindings({
  obligations: [obligation('satisfied', 'implementation-satisfaction', true)],
}));
assert.equal(checkedImplementation.status, 0);
assert.equal(checkedImplementation.document.partial_correctness.reported, true);
assert.equal(checkedImplementation.document.specification_quality.established, 1);
assert.equal(checkedImplementation.document.termination.claimed, false);

// A vacuous universal obligation is counted as vacuous, not silently as established evidence.
const vacuous = await report('vacuous', withBindings({
  obligations: [obligation('realizable', 'specification-quality', true, true)],
}));
assert.equal(vacuous.document.specification_quality.vacuous, 1);

console.log('spec-quality: 10 refusal classes and 4 separated obligations measured');
