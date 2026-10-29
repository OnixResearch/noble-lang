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

function report(name, document, command = 'spec-quality') {
  const file = path.join(workdir, `${name}.${command === 'verify' ? 'noble-contract' : 'json'}`);
  const submitted = Buffer.from(typeof document === 'string' ? document : serialize(document));
  fs.writeFileSync(file, submitted);
  const child = spawn(binary, [command, file], { encoding: 'utf8' });
  let stdout = '';
  let stderr = '';
  child.stdout.on('data', chunk => { stdout += chunk; });
  child.stderr.on('data', chunk => { stderr += chunk; });
  return new Promise(resolve => {
    child.on('close', code => {
      assert.equal(stderr, '', `unexpected stderr for ${name}`);
      assert.deepEqual(fs.readFileSync(file), submitted, `${name}: analysis must not rewrite its input`);
      resolve({ status: code ?? 2, document: JSON.parse(stdout) });
    });
  });
}

const obligation = (claim, kind, checked, vacuous = false) => ({ claim, obligation: kind, checked, vacuous });
const specification = claim => obligation(claim, 'specification-quality', true);
const withBindings = overrides => ({ ...profile, obligations: [], ...overrides });

const refused = async (name, document, outcome, expectedStatus = 2) => {
  const { status, document: reported } = await report(name, document);
  assert.equal(status, expectedStatus, `${name} must refuse, reported ${JSON.stringify(reported)}`);
  assert.equal(reported.outcome, outcome, `${name}: ${JSON.stringify(reported)}`);
  assert.equal(reported.schema, 'noble-mc2-spec-quality/v1');
  assert.equal(reported.guest_requests, 0, `${name} performs no guest execution`);
  assert.equal(reported.protected_operations, 0, `${name} performs no guest execution`);
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

// Change one binding of the accepted wrapping-increment submission at a time. A correct relation
// cannot compensate for a different predicate/model revision, observation rule or parameter binding.
for (const [field, value, outcome] of [
  ['predicate_revision', 2, 'wrong-predicate-revision'],
  ['model_revision', 2, 'wrong-model-revision'],
  ['observation_relation', 'i64-parity-equality', 'wrong-observation-relation'],
  ['parameter_binding', 'unordered-exact-i64-bool', 'wrong-parameter-binding'],
]) {
  outcomes.add(await refused(outcome,
    withBindings({ ...incrementClaims(), [field]: value }), outcome));
}

// Both directions of claim/kind substitution must fail, even with correct profile bindings and a
// retained relation that establishes all three specification-quality claims.
for (const [name, claim, kind] of [
  ['specification-as-implementation', 'deterministic', 'implementation-satisfaction'],
  ['implementation-as-specification', 'satisfied', 'specification-quality'],
]) {
  outcomes.add(await refused(name,
    withBindings({ relation: increment, obligations: [obligation(claim, kind, true)] }),
    'wrong-obligation'));
}

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
outcomes.add(await refused('undefined-postcondition',
  { ...profile, relation: { ...increment, post: eq(output(9), i64(0)) }, obligations: [specification('realizable')] },
  'undefined-relation'));
outcomes.add(await refused('false-conjunct-does-not-hide-undefined',
  { ...profile, relation: { ...increment, pre: conjunction(truth(false), eq(input(9), i64(0))) }, obligations: [] },
  'undefined-relation'));
outcomes.add(await refused('true-disjunct-does-not-hide-undefined',
  { ...profile, relation: { ...increment, pre: { op: 'or', args: [truth(true), eq(input(9), i64(0))] } }, obligations: [] },
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

// Exhaustion must not become evidence, and the final allowed byte/term level must remain usable.
const incrementManifest = serialize(withBindings(incrementClaims()));
const manifestAtLimit = incrementManifest + ' '.repeat(4096 - Buffer.byteLength(incrementManifest));
const byteBoundary = await report('manifest-at-byte-limit', manifestAtLimit);
assert.equal(byteBoundary.status, 0);
assert.deepEqual(byteBoundary.document.specification_quality, wrapping.document.specification_quality,
  'a complete manifest at the byte cap retains the same derived evidence');
outcomes.add(await refused('manifest-over-byte-limit', `${manifestAtLimit} `, 'exhausted', 5));

let preAtDepthLimit = truth(false);
for (let depth = 0; depth < 7; depth += 1) preAtDepthLimit = { op: 'not', args: [preAtDepthLimit] };
const depthBoundary = await derived('predicate-at-depth-limit',
  { ...increment, pre: preAtDepthLimit }, incrementClaims().obligations);
assert.equal(depthBoundary.status, 0);
assert.deepEqual(depthBoundary.document.specification_quality, wrapping.document.specification_quality,
  'seven negations of false remain a true precondition at the final allowed term level');
outcomes.add(await refused('predicate-over-depth-limit',
  withBindings({ ...incrementClaims(), relation: { ...increment, pre: { op: 'not', args: [preAtDepthLimit] } } }),
  'unsupported-relation'));

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
  ['ambiguity-at-domain-limit', {
    ...relational,
    domain: Array.from({ length: 64 }, (_, index) => ({ params: [], inputs: [{ i64: index }] })),
    pre: eq(input(0), i64(63)),
  }, false, ['two-distinct-outputs', 63, 0, 1]],
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

// The report accepts an intentionally relational contract without a determinacy obligation. This is
// report acceptance, not evidence that the separate ordinary program-admission path accepts it.
const relationalReport = await derived('relational-without-determinacy', duplicateFirstPermitted, []);
assert.equal(relationalReport.status, 0, 'a relational report needs no determinacy obligation');
assert.equal(relationalReport.document.specification_quality.derivation.deterministic, false,
  'the report still states what the relation does not establish');

// Criterion 5 uses ordinary contract preparation, not the specification-quality report. Both Bool
// implementations inhabit the same unconstrained relation. Missing implementation proof remains
// unknown after ordinary admission; neither uniqueness nor a spec-quality artifact is required.
const ordinaryContract = program => `(contract 1 relational-bool
  (input)
  (output (answer Bool))
  (program [ ${program} ])
  (requires true)
  (ensures true))
`;
for (const value of ['true', 'false']) {
  const ordinary = await report(`ordinary-relational-${value}`, ordinaryContract(value), 'verify');
  assert.equal(ordinary.status, 3, `${value}: missing implementation proof remains unknown`);
  assert.equal(ordinary.document.outcome, 'unknown');
  assert.equal(ordinary.document.ordinary_typing.outcome, 'accepted',
    `${value}: ordinary admission permits an intentionally relational Bool contract`);
  assert.equal(ordinary.document.independent_recheck, false,
    'ordinary admission must not manufacture implementation proof evidence');
  assert.deepEqual(ordinary.document.diagnostics.map(entry => entry.code), ['evidence-missing']);
}
const wrongOrdinaryType = await report('ordinary-relational-wrong-output-type', ordinaryContract('0'), 'verify');
assert.equal(wrongOrdinaryType.status, 2, 'a permissive postcondition cannot bypass ordinary typing');
assert.equal(wrongOrdinaryType.document.outcome, 'error');
assert.equal(wrongOrdinaryType.document.ordinary_typing, 'not-accepted');
assert.equal(wrongOrdinaryType.document.independent_recheck, false);
assert.deepEqual(wrongOrdinaryType.document.diagnostics.map(entry => entry.code), ['contract-invalid']);

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
assert.equal(document.specification_quality.not_established, 1,
  'an unchecked implementation claim does not count as a specification-quality failure');
assert.equal(document.specification_quality.vacuous, 0);
assert.deepEqual(document.specification_quality.obligations.map(entry => entry.claim),
  ['inhabited', 'realizable', 'deterministic'],
  'implementation claims do not appear among specification-quality obligations');
assert.equal(document.partial_correctness.substituted_by_specification_quality, false,
  'specification quality never discharges the implementation claim');
assert.equal(document.partial_correctness.reported, false,
  'an unchecked implementation claim is not reported as established');
assert.equal(document.termination.claimed, false, 'termination is never claimed');
assert.equal(document.correspondence.claimed, false, 'correspondence is never claimed');
assert.equal(document.observation_relation, profile.observation_relation,
  'the report states the observation relation it used');

// Without a retained relation, an implementation claim cannot establish any specification quality,
// whether checked, unchecked or vacuous. Its own reported status remains separate.
for (const [name, checked, vacuous] of [
  ['implementation', true, false],
  ['unchecked-implementation', false, false],
  ['vacuous-implementation', true, true],
]) {
  const implementation = await report(name, withBindings({
    obligations: [obligation('satisfied', 'implementation-satisfaction', checked, vacuous)],
  }));
  assert.equal(implementation.status, 0);
  const quality = implementation.document.specification_quality;
  assert.deepEqual([quality.established, quality.not_established, quality.vacuous, quality.obligations],
    [0, 0, 0, []], `${name}: an implementation claim supplies no specification-quality evidence`);
  assert.equal(implementation.document.partial_correctness.reported, checked);
  assert.equal(implementation.document.termination.claimed, false);
  assert.equal(implementation.document.correspondence.claimed, false);
}

// Adding a checked implementation claim to genuinely derived specification-quality claims must not
// alter any of their evidence. Conversely, those claims alone did not establish partial correctness.
const combined = await derived('increment-with-implementation', increment,
  [...incrementClaims().obligations, obligation('satisfied', 'implementation-satisfaction', true)]);
assert.equal(combined.status, 0);
assert.deepEqual(combined.document.specification_quality, wrapping.document.specification_quality,
  'implementation evidence cannot change specification-quality rows, tallies or derivation');
assert.equal(wrapping.document.partial_correctness.reported, false);
assert.equal(combined.document.partial_correctness.reported, true);
assert.equal(combined.document.termination.claimed, false);
assert.equal(combined.document.correspondence.claimed, false);

// A vacuous universal obligation is counted as vacuous, not silently as established evidence.
const vacuous = await report('vacuous', withBindings({
  relation: uninhabited,
  obligations: [
    obligation('inhabited', 'specification-quality', false, true),
    obligation('realizable', 'specification-quality', true, true),
    obligation('deterministic', 'specification-quality', true, true),
    obligation('satisfied', 'implementation-satisfaction', true, true),
  ],
}));
assert.equal(vacuous.status, 0, 'an honestly vacuous submission is reported');
assert.deepEqual(
  [vacuous.document.specification_quality.established,
   vacuous.document.specification_quality.not_established,
   vacuous.document.specification_quality.vacuous],
  [0, 1, 2],
  'vacuous universal claims are not established evidence; false inhabitation stays unestablished');
assert.deepEqual(vacuous.document.specification_quality.obligations.map(entry => entry.claim),
  ['inhabited', 'realizable', 'deterministic'],
  'a vacuous implementation claim also remains outside specification-quality evidence');

console.log(`spec-quality: refusal classes ${[...outcomes].sort().join(', ')}; derived propositions and separated obligations measured`);
