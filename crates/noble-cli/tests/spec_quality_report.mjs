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

// A JavaScript number cannot carry an exact I64 boundary, so boundary constants are written as
// markers and substituted into the serialized text. Everything else is ordinary JSON.
const I64_MAX = '__exact_i64_max__';
const I64_MIN = '__exact_i64_min__';
function serialize(document) {
  return JSON.stringify(document)
    .replaceAll(`"${I64_MAX}"`, '9223372036854775807')
    .replaceAll(`"${I64_MIN}"`, '-9223372036854775808');
}

function report(name, document) {
  const file = path.join(workdir, `${name}.json`);
  fs.writeFileSync(file, typeof document === 'string' ? document : serialize(document));
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
  assert.equal(status, 2, `${name} must refuse, reported ${JSON.stringify(reported)}`);
  assert.equal(reported.outcome, outcome, `${name}: ${JSON.stringify(reported)}`);
  assert.equal(reported.schema, 'noble-mc2-spec-quality/v1');
  assert.equal(reported.guest_requests, 0, `${name} performs no guest execution`);
  assert.equal(reported.protected_operations, 0, `${name} performs no guest execution`);
  assert.ok(reported.diagnostic.length > 0, `${name} names its refusal`);
  return reported.outcome;
};

const outcomes = new Set();

// Predicate terms are stated once, in a bounded pure-I64/Bool language, and the producer derives the
// specification-quality propositions from them. Control-flow between the obligations is a refusal,
// never agreement.
const truth = value => ({ op: 'constant', args: [{ bool: value }] });
const i64 = value => ({ op: 'constant', args: [{ i64: value }] });
const input = index => ({ op: 'input', args: [index] });
const output = index => ({ op: 'output', args: [index] });
const eq = (left, right) => ({ op: 'eq', args: [left, right] });
const add = (left, right) => ({ op: 'add', args: [left, right] });
const conjunction = (...terms) => ({ op: 'and', args: terms });

// The exact wrapping increment over an admissible domain that spans the I64 boundary, with the
// wrapping result as its only permitted output.
const increment = {
  params: [],
  inputs: ['i64'],
  outputs: ['i64'],
  domain: [
    { params: [], inputs: [{ i64: 0 }] },
    { params: [], inputs: [{ i64: I64_MAX }] },
  ],
  permitted: [[{ i64: 1 }], [{ i64: I64_MIN }]],
  pre: conjunction(truth(true)),
  post: eq(output(0), add(input(0), i64(1))),
};

// Precondition satisfied by one assignment, a postcondition no permitted output can satisfy.
const contradictory = {
  ...increment,
  domain: [{ params: [], inputs: [{ i64: 0 }] }],
  permitted: [[{ i64: 5 }]],
  post: eq(output(0), i64(4)),
};

// No assignment in the declared domain satisfies the precondition, so every universal proposition is
// provable and vacuous.
const uninhabited = {
  ...increment,
  domain: [{ params: [], inputs: [{ i64: 0 }] }],
  permitted: [[{ i64: 1 }]],
  pre: eq(input(0), i64(7)),
  post: eq(output(0), add(input(0), i64(1))),
};

// One admissible input and two distinct permitted Bool outputs satisfying the postcondition.
const relational = {
  params: [],
  inputs: ['i64'],
  outputs: ['bool'],
  domain: [{ params: [], inputs: [{ i64: 0 }] }],
  permitted: [[{ bool: true }], [{ bool: false }]],
  pre: conjunction(truth(true)),
  post: conjunction(truth(true)),
};

// One admissible input and three permitted Bool outputs whose first two coincide: only the third is
// distinct, so a comparison restricted to the first two permitted stacks would call this deterministic.
const duplicateFirstPermitted = {
  params: [],
  inputs: ['i64'],
  outputs: ['bool'],
  domain: [{ params: [], inputs: [{ i64: 0 }] }],
  permitted: [[{ bool: true }], [{ bool: true }], [{ bool: false }]],
  pre: conjunction(truth(true)),
  post: conjunction(truth(true)),
};

const incrementClaims = () => ({
  relation: increment,
  obligations: [specification('inhabited'), specification('realizable'), specification('deterministic')],
});

// A manifest that attests obligations its own retained relation refuses is not read as agreement.
outcomes.add(await refused('self-attested-realizability',
  { ...profile, relation: contradictory, obligations: [specification('realizable')] },
  'self-attested-obligation'));
outcomes.add(await refused('self-attested-determinacy',
  { ...profile, relation: duplicateFirstPermitted, obligations: [specification('deterministic')] },
  'self-attested-obligation'));
outcomes.add(await refused('self-attested-vacuity',
  { ...profile, relation: uninhabited, obligations: [obligation('realizable', 'specification-quality', true, false)] },
  'self-attested-obligation'));
outcomes.add(await refused('specification-claim-without-relation',
  withBindings({ obligations: [specification('inhabited')] }),
  'self-attested-obligation'));
// An undefined predicate never becomes a successful check, and neither does an unsupported construct.
outcomes.add(await refused('undefined-predicate',
  { ...profile, relation: { ...increment, pre: eq(input(9), i64(0)) }, obligations: [specification('inhabited')] },
  'undefined-relation'));
outcomes.add(await refused('mixed-kind-equality',
  { ...profile, relation: { ...increment, pre: eq(input(0), truth(true)) }, obligations: [specification('inhabited')] },
  'undefined-relation'));
outcomes.add(await refused('unsupported-op',
  { ...profile, relation: { ...increment, pre: { op: 'implies', args: [truth(true)] } }, obligations: [specification('inhabited')] },
  'unsupported-relation'));
outcomes.add(await refused('unsupported-kind',
  { ...profile, relation: { ...increment, outputs: ['map'] }, obligations: [specification('inhabited')] },
  'unsupported-relation'));
outcomes.add(await refused('unbounded-domain',
  { ...profile, relation: { ...increment, domain: Array.from({ length: 65 }, () => ({ params: [], inputs: [{ i64: 0 }] })) },
    obligations: [specification('inhabited')] },
  'unsupported-relation'));
outcomes.add(await refused('too-many-permitted-outputs',
  { ...profile, relation: { ...relational, permitted: Array.from({ length: 17 }, () => [{ bool: true }]) },
    obligations: [specification('deterministic')] },
  'unsupported-relation'));
outcomes.add(await refused('wrong-permitted-output-kind',
  { ...profile, relation: { ...relational, permitted: [[{ bool: true }], [{ i64: 1 }]] },
    obligations: [specification('deterministic')] },
  'unsupported-relation'));

// Criteria 1-3 measured by the producer itself over the retained relations.
const derived = async (name, relation, claims) => {
  const result = await report(name, { ...profile, relation, obligations: claims });
  return result;
};
const wrapping = await derived('wrapping-increment', increment, incrementClaims().obligations);
assert.equal(wrapping.status, 0, 'the wrapping increment is accepted');
assert.deepEqual(
  [wrapping.document.specification_quality.derivation.inhabited,
   wrapping.document.specification_quality.derivation.realizable,
   wrapping.document.specification_quality.derivation.deterministic],
  [true, true, true],
  'the wrapping increment is inhabited, realizable and deterministic');
assert.equal(wrapping.document.specification_quality.derivation.refutation, null);

const refuted = await derived('contradictory', contradictory, [obligation('realizable', 'specification-quality', false)]);
assert.equal(refuted.status, 0, 'a contradiction is reported rather than refused as agreement');
assert.equal(refuted.document.specification_quality.derivation.realizable, false,
  'the contradictory postcondition derives non-realizability');
assert.deepEqual(refuted.document.specification_quality.derivation.refutation, ['no-output', 0],
  'the refusal carries the checked admissible assignment, not a failed search');
assert.equal(refuted.document.specification_quality.not_established, 1);

const ambiguous = await derived('relational', relational, [obligation('deterministic', 'specification-quality', false)]);
assert.equal(ambiguous.status, 0);
assert.equal(ambiguous.document.specification_quality.derivation.deterministic, false,
  'two distinct satisfying outputs for one input refuse determinacy');
assert.deepEqual(ambiguous.document.specification_quality.derivation.refutation, ['two-distinct-outputs', 0, 0, 1],
  'both checked distinct outputs are named');

// Issue #70: determinacy compares exact whole stacks after postcondition filtering, not just the
// first two candidates. Witness indices refer to the original permitted list, including at its cap.
const determinacyCases = [
  ['duplicate-first-permitted', duplicateFirstPermitted, false, ['two-distinct-outputs', 0, 0, 2]],
  ['reordered-permitted',
    { ...relational, permitted: [[{ bool: true }], [{ bool: false }], [{ bool: true }]] },
    false, ['two-distinct-outputs', 0, 0, 1]],
  ['duplicate-only-permitted',
    { ...relational, permitted: [[{ bool: true }], [{ bool: true }], [{ bool: true }]] },
    true, null],
  ['postcondition-filters-distinct-output',
    { ...duplicateFirstPermitted, post: eq(output(0), truth(true)) },
    true, null],
  ['postcondition-filters-prefix', {
    ...relational,
    outputs: ['i64'],
    permitted: [0, 1, 1, 2].map(value => [{ i64: value }]),
    post: { op: 'not', args: [eq(output(0), i64(0))] },
  }, false, ['two-distinct-outputs', 0, 1, 3]],
  ['whole-stack-difference', {
    ...relational,
    outputs: ['i64', 'bool'],
    permitted: [
      [{ i64: 1 }, { bool: true }],
      [{ i64: 1 }, { bool: true }],
      [{ i64: 1 }, { bool: false }],
    ],
  }, false, ['two-distinct-outputs', 0, 0, 2]],
  ['distinct-at-output-limit', {
    ...relational,
    permitted: [...Array.from({ length: 15 }, () => [{ bool: true }]), [{ bool: false }]],
  }, false, ['two-distinct-outputs', 0, 0, 15]],
];
for (const [name, relation, deterministic, refutation] of determinacyCases) {
  const result = await derived(name, relation, [
    specification('inhabited'), specification('realizable'),
    obligation('deterministic', 'specification-quality', deterministic),
  ]);
  assert.equal(result.status, 0, `${name}: a consistent derivation is reported`);
  const quality = result.document.specification_quality;
  assert.deepEqual(
    [quality.derivation.inhabited, quality.derivation.realizable, quality.derivation.deterministic,
     quality.derivation.vacuous, quality.derivation.refutation],
    [true, true, deterministic, false, refutation],
    `${name}: only distinct satisfying stacks refute non-vacuous determinacy`);
  assert.equal(quality.established, deterministic ? 3 : 2, `${name}: only derived claims are established`);
  assert.equal(quality.not_established, deterministic ? 0 : 1, `${name}: ambiguity remains unestablished`);
}

// Ordinary admission of an intentionally relational contract is untouched: no obligation is forced.
const relationalAdmission = await derived('relational-admission', duplicateFirstPermitted, []);
assert.equal(relationalAdmission.status, 0, 'a relational contract needs no determinacy obligation');
assert.equal(relationalAdmission.document.specification_quality.derivation.deterministic, false,
  'the report still states what the relation does not establish');

// A reported submission keeps the four obligations apart and never upgrades an unchecked claim.
const { status, document } = await report('reported', withBindings({
  relation: relational,
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
  relation: uninhabited,
  obligations: [obligation('realizable', 'specification-quality', true, true)],
}));
assert.equal(vacuous.status, 0, 'an honestly vacuous submission is reported');
assert.equal(vacuous.document.specification_quality.vacuous, 1,
  'a provable universal proposition over an uninhabited domain is marked vacuous');

console.log('spec-quality: refusal classes, derived propositions and separated obligations measured');
