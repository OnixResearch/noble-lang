import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { spawn } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

// Real selected-CLI coverage of the native source-bound specification-quality report producer
// (issue #65). Every refusal class arrives as its own diagnostic, and the report keeps
// specification quality, partial correctness, termination and correspondence separate.

const binary = process.argv[2];
assert.ok(binary, 'pass the selected noble CLI binary');
const workdir = fs.mkdtempSync(path.join(os.tmpdir(), 'noble-spec-quality-'));

const profile = {
  schema: 'noble-mc2-spec-quality/v2',
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

function sha256(bytes) {
  return createHash('sha256').update(bytes).digest('hex');
}

function sourceHash(source) {
  return sha256(Buffer.from(source));
}

function report(name, source, document, command = 'spec-quality') {
  const contract = path.join(workdir, `${name}.noble-contract`);
  const sourceBytes = Buffer.from(source);
  fs.writeFileSync(contract, sourceBytes);
  const argumentsForChild = [command, contract];
  let manifest;
  let manifestBytes;
  if (command === 'spec-quality') {
    manifest = path.join(workdir, `${name}.json`);
    manifestBytes = Buffer.from(typeof document === 'string' ? document : serialize(document));
    fs.writeFileSync(manifest, manifestBytes);
    argumentsForChild.push(manifest);
  }
  const child = spawn(binary, argumentsForChild, { encoding: 'utf8' });
  let stdout = '';
  let stderr = '';
  child.stdout.on('data', chunk => { stdout += chunk; });
  child.stderr.on('data', chunk => { stderr += chunk; });
  return new Promise(resolve => {
    child.on('close', code => {
      assert.equal(stderr, '', `unexpected stderr for ${name}`);
      assert.deepEqual(fs.readFileSync(contract), sourceBytes,
        `${name}: source analysis must not rewrite its input`);
      if (manifestBytes !== undefined) {
        assert.deepEqual(fs.readFileSync(manifest), manifestBytes,
          `${name}: manifest analysis must not rewrite its input`);
      }
      resolve({ status: code ?? 2, document: JSON.parse(stdout) });
    });
  });
}

const obligation = (claim, kind, checked, vacuous = false) => ({ claim, obligation: kind, checked, vacuous });
const specification = claim => obligation(claim, 'specification-quality', true);

function manifestFor(source, {
  domain = incrementDomain,
  permitted = incrementPermitted,
  obligations = [],
  ...overrides
} = {}) {
  return {
    ...profile,
    source_sha256: sourceHash(source),
    domain,
    permitted,
    obligations,
    ...overrides,
  };
}

const refused = async (name, source, document, outcome, expectedStatus = 2) => {
  const { status, document: reported } = await report(name, source, document);
  assert.equal(status, expectedStatus, `${name} must refuse, reported ${JSON.stringify(reported)}`);
  assert.equal(reported.outcome, outcome, `${name}: ${JSON.stringify(reported)}`);
  assert.equal(reported.schema, 'noble-mc2-spec-quality/v2');
  assert.equal(reported.guest_requests, 0, `${name} performs no guest execution`);
  assert.equal(reported.protected_operations, 0, `${name} performs no guest execution`);
  return reported.outcome;
};

const outcomes = new Set();

// These are real native .noble-contract fixtures. The grammar and definition/parameter forms are
// the same ones used by crates/noble-contracts/src/fixtures/*.noble; predicates are resolved by
// the native frontend, never reconstructed from manifest text.
function contractSource({
  name,
  input,
  output,
  params = null,
  definitions = null,
  program = '[]',
  requires = 'true',
  ensures = 'true',
}) {
  const fields = [
    `(contract 1 ${name}`,
    `  ${input}`,
    `  ${output}`,
  ];
  if (params !== null) fields.push(`  ${params}`);
  if (definitions !== null) fields.push(`  ${definitions}`);
  fields.push(`  (program ${program})`);
  fields.push(`  (requires ${requires})`);
  fields.push(`  (ensures ${ensures}))`);
  return `${fields.join('\n')}\n`;
}

const incrementSource = contractSource({
  name: 'increment',
  input: '(input (x I64))',
  output: '(output (y I64))',
  definitions: '(define one I64 1)',
  program: '[ 1 + ]',
  ensures: '(eq (out y) (add (in x) (def one)))',
});

const incrementDomain = [
  { inputs: [{ i64: 0 }], params: [] },
  { inputs: [{ i64: I64_MAX }], params: [] },
];
const incrementPermitted = [[{ i64: 1 }], [{ i64: I64_MIN }]];

const relationalSource = contractSource({
  name: 'relational-bool',
  input: '(input (x I64))',
  output: '(output (answer Bool))',
  program: '[ drop true ]',
  ensures: 'true',
});
const relationalDomain = [{ inputs: [{ i64: 0 }], params: [] }];
const relationalPermitted = [[{ bool: true }], [{ bool: false }]];

const duplicateFirstSource = relationalSource;
const duplicateFirstDomain = relationalDomain;
const duplicateFirstPermitted = [[{ bool: true }], [{ bool: true }], [{ bool: false }]];

const incrementClaims = () => [
  specification('inhabited'),
  specification('realizable'),
  specification('deterministic'),
];

const derived = (name, source, domain, permitted, claims, overrides = {}) => report(
  name,
  source,
  manifestFor(source, { domain, permitted, obligations: claims, ...overrides }),
);

// Source identity is checked before native preparation. None of these changed source bytes may
// reuse the old digest, even when the changed fixture remains well typed.
const oldIncrementManifest = manifestFor(incrementSource, {
  domain: incrementDomain,
  permitted: incrementPermitted,
  obligations: incrementClaims(),
});
const changedSourceCases = [
  ['changed-predicate-body', contractSource({
    name: 'increment',
    input: '(input (x I64))',
    output: '(output (y I64))',
    definitions: '(define one I64 1)',
    program: '[ 1 + ]',
    ensures: '(eq (out y) (add (in x) 2))',
  })],
  ['changed-program-body', contractSource({
    name: 'increment',
    input: '(input (x I64))',
    output: '(output (y I64))',
    definitions: '(define one I64 1)',
    program: '[ 2 + ]',
    ensures: '(eq (out y) (add (in x) (def one)))',
  })],
  ['changed-definition-body', contractSource({
    name: 'increment',
    input: '(input (x I64))',
    output: '(output (y I64))',
    definitions: '(define one I64 2)',
    program: '[ 1 + ]',
    ensures: '(eq (out y) (add (in x) (def one)))',
  })],
  ['changed-interface', contractSource({
    name: 'increment',
    input: '(input (x Bool))',
    output: '(output (y Bool))',
    program: '[ drop true ]',
    ensures: '(eq (out y) (in x))',
  })],
  ['changed-contract-name', contractSource({
    name: 'renamed-increment',
    input: '(input (x I64))',
    output: '(output (y I64))',
    definitions: '(define one I64 1)',
    program: '[ 1 + ]',
    ensures: '(eq (out y) (add (in x) (def one)))',
  })],
  ['changed-parameters', contractSource({
    name: 'increment',
    input: '(input (x I64))',
    output: '(output (y I64))',
    params: '(params (bias I64))',
    definitions: '(define one I64 1)',
    program: '[ 1 + ]',
    ensures: '(eq (out y) (add (in x) (param bias)))',
  })],
];
for (const [name, source] of changedSourceCases) {
  outcomes.add(await refused(name, source, oldIncrementManifest, 'wrong-source-binding'));
}

// Hold the finite domain, outputs and requested claims fixed while changing only the native
// postcondition and its source digest. The old true claim must not survive a correct rebinding.
const falsePostSource = contractSource({
  name: 'increment',
  input: '(input (x I64))',
  output: '(output (y I64))',
  definitions: '(define one I64 1)',
  program: '[ 1 + ]',
  ensures: 'false',
});
const reboundManifest = { ...oldIncrementManifest, source_sha256: sourceHash(falsePostSource) };
outcomes.add(await refused('same-domain-rebound-false-post', falsePostSource,
  reboundManifest, 'self-attested-obligation'));
const reboundRefutation = await report('same-domain-rebound-refutation', falsePostSource, {
  ...reboundManifest,
  obligations: [obligation('realizable', 'specification-quality', false)],
});
assert.equal(reboundRefutation.status, 0);
assert.equal(reboundRefutation.document.specification_quality.derivation.realizable, false);
assert.equal(reboundRefutation.document.specification_quality.derivation.refutation[0], 'no-output');

// Changing the source and rebinding its digest does not turn a false postcondition into a true
// realizability claim. The matching false flag carries the measured no-output refutation.
const contradictorySource = contractSource({
  name: 'contradictory',
  input: '(input (x I64))',
  output: '(output (y I64))',
  program: '[ 1 + ]',
  ensures: '(eq (out y) 4)',
});
const contradictoryDomain = [{ inputs: [{ i64: 0 }], params: [] }];
const contradictoryPermitted = [[{ i64: 5 }]];
outcomes.add(await refused('rebound-false-postcondition', contradictorySource,
  manifestFor(contradictorySource, {
    domain: contradictoryDomain,
    permitted: contradictoryPermitted,
    obligations: [specification('realizable')],
  }), 'self-attested-obligation'));
const refuted = await derived('contradictory', contradictorySource,
  contradictoryDomain, contradictoryPermitted, [obligation('realizable', 'specification-quality', false)]);
assert.equal(refuted.status, 0, 'a contradiction is reported rather than refused as agreement');
assert.equal(refuted.document.specification_quality.derivation.realizable, false,
  'the contradictory postcondition derives non-realizability');
assert.deepEqual(refuted.document.specification_quality.derivation.refutation, ['no-output', 0],
  'the refutation carries the checked admissible assignment, not a failed search');
assert.equal(refuted.document.specification_quality.not_established, 1);

// Native source hash and native contract identity are part of the successful report. File paths are
// not identities: byte-identical source and manifest at two paths must produce the same report.
const wrappingManifest = manifestFor(incrementSource, {
  domain: incrementDomain,
  permitted: incrementPermitted,
  obligations: incrementClaims(),
});
const samePathA = await report('same-source-first-path', incrementSource, wrappingManifest);
const samePathB = await report('same-source-second-path', incrementSource, wrappingManifest);
assert.deepEqual(samePathA.document, samePathB.document,
  'the same source bytes at another path have equivalent report identity');
assert.equal(samePathA.document.source_sha256, sourceHash(incrementSource));
assert.equal(samePathA.document.manifest_sha256, sha256(Buffer.from(serialize(wrappingManifest))));
assert.equal(samePathA.document.contract.name, 'increment');

// Change one retained binding at a time. A correct native contract cannot compensate for a
// different predicate/model revision, observation rule or parameter binding.
for (const [field, value, outcome] of [
  ['predicate_revision', 2, 'wrong-predicate-revision'],
  ['model_revision', 2, 'wrong-model-revision'],
  ['observation_relation', 'i64-parity-equality', 'wrong-observation-relation'],
  ['parameter_binding', 'unordered-exact-i64-bool', 'wrong-parameter-binding'],
]) {
  outcomes.add(await refused(outcome, incrementSource,
    manifestFor(incrementSource, {
      domain: incrementDomain,
      permitted: incrementPermitted,
      obligations: incrementClaims(),
      [field]: value,
    }), outcome));
}

// Both directions of claim/kind substitution must fail, even with a native source whose derived
// relation establishes all three specification-quality claims.
for (const [name, claim, kind] of [
  ['specification-as-implementation', 'deterministic', 'implementation-satisfaction'],
  ['implementation-as-specification', 'satisfied', 'specification-quality'],
]) {
  outcomes.add(await refused(name, incrementSource,
    manifestFor(incrementSource, {
      domain: incrementDomain,
      permitted: incrementPermitted,
      obligations: [obligation(claim, kind, true)],
    }), 'wrong-obligation'));
}

const uninhabitedSource = contractSource({
  name: 'uninhabited',
  input: '(input (x I64))',
  output: '(output (y I64))',
  definitions: '(define one I64 1)',
  program: '[ 1 + ]',
  requires: 'false',
  ensures: '(eq (out y) (add (in x) (def one)))',
});
const uninhabitedDomain = [{ inputs: [{ i64: 0 }], params: [] }];
const uninhabitedPermitted = [[{ i64: 1 }]];

// A manifest that attests obligations its native source does not derive is not read as agreement.
outcomes.add(await refused('self-attested-realizability', contradictorySource,
  manifestFor(contradictorySource, {
    domain: contradictoryDomain,
    permitted: contradictoryPermitted,
    obligations: [specification('realizable')],
  }), 'self-attested-obligation'));
outcomes.add(await refused('self-attested-determinacy', duplicateFirstSource,
  manifestFor(duplicateFirstSource, {
    domain: duplicateFirstDomain,
    permitted: duplicateFirstPermitted,
    obligations: [specification('deterministic')],
  }), 'self-attested-obligation'));
outcomes.add(await refused('self-attested-vacuity', uninhabitedSource,
  manifestFor(uninhabitedSource, {
    domain: uninhabitedDomain,
    permitted: uninhabitedPermitted,
    obligations: [obligation('realizable', 'specification-quality', true, false)],
  }), 'self-attested-obligation'));

// Every report requires the native source binding and finite model. Manifest-only predicate/type
// fields and the old retained-relation field are rejected as unknown input, including nested
// assignment injection.
for (const [name, field, value] of [
  ['manifest-predicate-injection', 'pre', 'true'],
  ['manifest-post-injection', 'post', 'true'],
  ['manifest-type-injection', 'types', ['I64']],
  ['legacy-relation-injection', 'relation', { pre: true, post: true }],
  ['legacy-v1-schema', 'schema', 'noble-mc2-spec-quality/v1'],
]) {
  const injected = manifestFor(incrementSource, {
    domain: incrementDomain,
    permitted: incrementPermitted,
    obligations: [],
    [field]: value,
  });
  outcomes.add(await refused(name, incrementSource, injected, 'invalid-manifest'));
}
const assignmentInjection = manifestFor(incrementSource, {
  domain: [{ ...incrementDomain[0], pre: true }],
  permitted: incrementPermitted,
  obligations: [],
});
outcomes.add(await refused('manifest-assignment-injection', incrementSource,
  assignmentInjection, 'invalid-manifest'));
const missingDomain = manifestFor(incrementSource, {
  domain: incrementDomain,
  permitted: incrementPermitted,
  obligations: [],
});
delete missingDomain.domain;
outcomes.add(await refused('missing-domain', incrementSource, missingDomain, 'invalid-manifest'));
const missingPermitted = manifestFor(incrementSource, {
  domain: incrementDomain,
  permitted: incrementPermitted,
  obligations: [],
});
delete missingPermitted.permitted;
outcomes.add(await refused('missing-permitted', incrementSource, missingPermitted, 'invalid-manifest'));

// Native frontend errors are not undefined relation values. Correctly rebound source hashes still
// fail ordinary source typing, while native but out-of-profile forms are refused by the report
// predicate checker.
const nativeInvalidCases = [
  ['native-out-of-scope-input', contractSource({
    name: 'bad-input-reference',
    input: '(input (x I64))',
    output: '(output (y I64))',
    program: '[ 1 + ]',
    ensures: '(eq (in missing) 0)',
  })],
  ['native-mixed-kind-equality', contractSource({
    name: 'bad-kind-equality',
    input: '(input (x I64))',
    output: '(output (y I64))',
    program: '[ 1 + ]',
    ensures: '(eq (in x) true)',
  })],
  ['native-out-of-scope-output', contractSource({
    name: 'bad-output-reference',
    input: '(input (x I64))',
    output: '(output (y I64))',
    program: '[ 1 + ]',
    ensures: '(eq (out missing) 0)',
  })],
];
for (const [name, source] of nativeInvalidCases) {
  outcomes.add(await refused(name, source,
    manifestFor(source, {
      domain: incrementDomain,
      permitted: incrementPermitted,
      obligations: [],
    }), 'invalid-contract'));
}

const unsupportedPredicateSource = contractSource({
  name: 'unsupported-predicate',
  input: '(input (x I64))',
  output: '(output (y I64))',
  program: '[ 1 + ]',
  ensures: '(implies true true)',
});
outcomes.add(await refused('unsupported-op', unsupportedPredicateSource,
  manifestFor(unsupportedPredicateSource, {
    domain: incrementDomain,
    permitted: incrementPermitted,
    obligations: [specification('inhabited')],
  }), 'unsupported-relation'));
const falseConjunctUnsupported = contractSource({
  name: 'false-conjunct-unsupported',
  input: '(input (x I64))',
  output: '(output (y I64))',
  program: '[ 1 + ]',
  requires: '(and false (implies true true))',
});
outcomes.add(await refused('false-conjunct-does-not-hide-unsupported', falseConjunctUnsupported,
  manifestFor(falseConjunctUnsupported, {
    domain: incrementDomain,
    permitted: incrementPermitted,
    obligations: [],
  }), 'unsupported-relation'));
const trueDisjunctUnsupported = contractSource({
  name: 'true-disjunct-unsupported',
  input: '(input (x I64))',
  output: '(output (y I64))',
  program: '[ 1 + ]',
  requires: '(or true (implies true true))',
});
outcomes.add(await refused('true-disjunct-does-not-hide-unsupported', trueDisjunctUnsupported,
  manifestFor(trueDisjunctUnsupported, {
    domain: incrementDomain,
    permitted: incrementPermitted,
    obligations: [],
  }), 'unsupported-relation'));

// A native definition reference is a supported transparent expression. The increment fixture above
// uses `(define one I64 1)` and `(def one)` rather than a manifest-side predicate copy.
const wrapping = await derived('wrapping-increment', incrementSource,
  incrementDomain, incrementPermitted, incrementClaims());
assert.equal(wrapping.status, 0, 'the wrapping increment is accepted');
assert.deepEqual(
  [wrapping.document.specification_quality.derivation.inhabited,
   wrapping.document.specification_quality.derivation.realizable,
   wrapping.document.specification_quality.derivation.deterministic],
  [true, true, true],
  'the wrapping increment is inhabited, realizable and deterministic');
assert.equal(wrapping.document.specification_quality.derivation.refutation, null);

// Native interface and parameter stacks, rather than manifest-declared types, are authoritative.
const parameterizedSource = contractSource({
  name: 'parameterized-increment',
  input: '(input (x I64))',
  output: '(output (y I64))',
  params: '(params (bias I64))',
  program: '[ 1 + ]',
  ensures: '(eq (out y) (add (in x) (param bias)))',
});
const parameterizedDomain = [{ inputs: [{ i64: 0 }], params: [{ i64: 1 }] }];
const parameterizedPermitted = [[{ i64: 1 }]];
const parameterized = await derived('native-parameter-binding', parameterizedSource,
  parameterizedDomain, parameterizedPermitted, [specification('realizable')]);
assert.equal(parameterized.status, 0, 'native parameter binding is accepted');
const orderedParameters = params => contractSource({
  name: 'ordered-parameters', input: '(input)', output: '(output (y I64))',
  params, program: '[ 1 ]', ensures: '(eq (out y) (param left))',
});
const orderedSource = orderedParameters('(params (left I64) (right I64))');
const reorderedSource = orderedParameters('(params (right I64) (left I64))');
const orderedManifest = manifestFor(orderedSource, {
  domain: [{ inputs: [], params: [{ i64: 1 }, { i64: 2 }] }],
  permitted: [[{ i64: 1 }]], obligations: [specification('realizable')],
});
assert.equal((await report('ordered-native-parameters', orderedSource, orderedManifest)).status, 0);
outcomes.add(await refused('reordered-native-parameters', reorderedSource,
  { ...orderedManifest, source_sha256: sourceHash(reorderedSource) }, 'self-attested-obligation'));
const missingParameter = manifestFor(parameterizedSource, {
  domain: [{ inputs: [{ i64: 0 }], params: [] }],
  permitted: parameterizedPermitted,
  obligations: [],
});
outcomes.add(await refused('native-parameter-stack-mismatch', parameterizedSource,
  missingParameter, 'unsupported-relation'));
const boolParameterSource = contractSource({
  name: 'bool-parameter',
  input: '(input (x I64))',
  output: '(output (y I64))',
  params: '(params (flag Bool))',
  program: '[ 1 + ]',
  ensures: 'true',
});
outcomes.add(await refused('native-parameter-kind-mismatch', boolParameterSource,
  manifestFor(boolParameterSource, {
    domain: [{ inputs: [{ i64: 0 }], params: [{ i64: 0 }] }],
    permitted: incrementPermitted,
    obligations: [],
  }), 'unsupported-relation'));
const boolOutputSource = contractSource({
  name: 'bool-output',
  input: '(input (x I64))',
  output: '(output (answer Bool))',
  program: '[ drop true ]',
  ensures: 'true',
});
outcomes.add(await refused('native-output-kind-mismatch', boolOutputSource,
  manifestFor(boolOutputSource, {
    domain: relationalDomain,
    permitted: incrementPermitted,
    obligations: [],
  }), 'unsupported-relation'));

// Exhaustion must not become evidence, and the final allowed byte/term levels remain usable.
const incrementManifest = serialize(wrappingManifest);
const manifestAtLimit = incrementManifest + ' '.repeat(4096 - Buffer.byteLength(incrementManifest));
const byteBoundary = await report('manifest-at-byte-limit', incrementSource, manifestAtLimit);
assert.equal(byteBoundary.status, 0);
assert.deepEqual(byteBoundary.document.specification_quality, wrapping.document.specification_quality,
  'a complete manifest at the byte cap retains the same derived evidence');
outcomes.add(await refused('manifest-over-byte-limit', incrementSource, `${manifestAtLimit} `,
  'exhausted', 5));

const sourceAtLimit = incrementSource + ' '.repeat(65_536 - Buffer.byteLength(incrementSource));
const sourceBoundary = await report('source-at-byte-limit', sourceAtLimit,
  { ...wrappingManifest, source_sha256: sourceHash(sourceAtLimit) });
assert.equal(sourceBoundary.status, 0);
assert.deepEqual(sourceBoundary.document.specification_quality, wrapping.document.specification_quality);
outcomes.add(await refused('source-over-byte-limit', `${sourceAtLimit} `,
  { ...wrappingManifest, source_sha256: sourceHash(`${sourceAtLimit} `) }, 'exhausted', 5));

function nestedNot(term, count) {
  let current = term;
  for (let depth = 0; depth < count; depth += 1) current = `(not ${current})`;
  return current;
}
const preAtDepthLimit = nestedNot('false', 7);
const depthBoundarySource = contractSource({
  name: 'predicate-at-depth-limit',
  input: '(input (x I64))',
  output: '(output (y I64))',
  definitions: '(define one I64 1)',
  program: '[ 1 + ]',
  requires: preAtDepthLimit,
  ensures: '(eq (out y) (add (in x) (def one)))',
});
const depthBoundary = await derived('predicate-at-depth-limit', depthBoundarySource,
  incrementDomain, incrementPermitted, incrementClaims());
assert.equal(depthBoundary.status, 0);
assert.deepEqual(depthBoundary.document.specification_quality, wrapping.document.specification_quality,
  'seven negations of false remain a true precondition at the final allowed term level');
const overDepthSource = contractSource({
  name: 'predicate-over-depth-limit',
  input: '(input (x I64))',
  output: '(output (y I64))',
  definitions: '(define one I64 1)',
  program: '[ 1 + ]',
  requires: nestedNot('false', 8),
  ensures: '(eq (out y) (add (in x) (def one)))',
});
outcomes.add(await refused('predicate-over-depth-limit', overDepthSource,
  manifestFor(overDepthSource, {
    domain: incrementDomain,
    permitted: incrementPermitted,
    obligations: incrementClaims(),
  }), 'unsupported-relation'));

const relational = await derived('relational', relationalSource,
  relationalDomain, relationalPermitted, [obligation('deterministic', 'specification-quality', false)]);
assert.equal(relational.status, 0);
assert.equal(relational.document.specification_quality.derivation.deterministic, false,
  'two distinct satisfying outputs for one input refuse determinacy');
assert.deepEqual(relational.document.specification_quality.derivation.refutation,
  ['two-distinct-outputs', 0, 0, 1], 'both checked distinct outputs are named');

// Issue #70: determinacy compares exact whole stacks after postcondition filtering, not just the
// first two candidates. Witness indices refer to the original permitted list, including its cap.
const i64RelationSource = contractSource({
  name: 'i64-relation',
  input: '(input (x I64))',
  output: '(output (y I64))',
  program: '[ 1 + ]',
  requires: '(eq (in x) 2)',
  ensures: '(not (eq (out y) 0))',
});
const domainBoundarySource = contractSource({
  name: 'domain-boundary', input: '(input (x I64))', output: '(output (answer Bool))',
  program: '[ drop true ]', requires: '(eq (in x) 63)',
});
const twoOutputSource = contractSource({
  name: 'whole-stack-relation',
  input: '(input (x I64))',
  output: '(output (a I64) (b Bool))',
  program: '[ drop 1 true ]',
  ensures: 'true',
});
const determinacyCases = [
  ['duplicate-first-permitted', duplicateFirstSource, duplicateFirstDomain,
    duplicateFirstPermitted, false, ['two-distinct-outputs', 0, 0, 2]],
  ['reordered-permitted', relationalSource, relationalDomain,
    [[{ bool: true }], [{ bool: false }], [{ bool: true }]], false,
    ['two-distinct-outputs', 0, 0, 1]],
  ['duplicate-only-permitted', relationalSource, relationalDomain,
    [[{ bool: true }], [{ bool: true }], [{ bool: true }]], true, null],
  ['postcondition-filters-distinct-output', contractSource({
    name: 'postcondition-filters-output',
    input: '(input (x I64))',
    output: '(output (answer Bool))',
    program: '[ drop true ]',
    ensures: '(eq (out answer) true)',
  }), relationalDomain, duplicateFirstPermitted, true, null],
  ['postcondition-filters-prefix', i64RelationSource,
    Array.from({ length: 4 }, (_, index) => ({ inputs: [{ i64: index }], params: [] })),
    [0, 1, 1, 2].map(value => [{ i64: value }]), false,
    ['two-distinct-outputs', 2, 1, 3]],
  ['whole-stack-difference', twoOutputSource, relationalDomain,
    [[{ i64: 1 }, { bool: true }], [{ i64: 1 }, { bool: true }], [{ i64: 1 }, { bool: false }]],
    false, ['two-distinct-outputs', 0, 0, 2]],
  ['distinct-at-output-limit', relationalSource, relationalDomain,
    [...Array.from({ length: 15 }, () => [{ bool: true }]), [{ bool: false }]], false,
    ['two-distinct-outputs', 0, 0, 15]],
  ['ambiguity-at-domain-limit', domainBoundarySource,
    Array.from({ length: 64 }, (_, index) => ({ inputs: [{ i64: index }], params: [] })),
    relationalPermitted, false, ['two-distinct-outputs', 63, 0, 1]],
];
for (const [name, source, domain, permitted, deterministic, refutation] of determinacyCases) {
  const result = await derived(name, source, domain, permitted, [
    specification('inhabited'), specification('realizable'),
    obligation('deterministic', 'specification-quality', deterministic),
  ]);
  assert.equal(result.status, 0, `${name}: a consistent derivation is reported`);
  const quality = result.document.specification_quality;
  assert.deepEqual(
    [quality.derivation.inhabited, quality.derivation.realizable, quality.derivation.deterministic,
     quality.derivation.vacuous, quality.derivation.refutation],
    [true, true, deterministic, false, refutation],
    `${name}: only distinct satisfying stacks refute non-vacuous determinacy`,
  );
  assert.equal(quality.established, deterministic ? 3 : 2,
    `${name}: only derived claims are established`);
  assert.equal(quality.not_established, deterministic ? 0 : 1,
    `${name}: ambiguity remains unestablished`);
}

// A relational report needs no determinacy obligation. This is report acceptance, not evidence that
// the separate ordinary program-admission path accepts a relational contract.
const relationalReport = await derived('relational-without-determinacy', duplicateFirstSource,
  duplicateFirstDomain, duplicateFirstPermitted, []);
assert.equal(relationalReport.status, 0, 'a relational report needs no determinacy obligation');
assert.equal(relationalReport.document.specification_quality.derivation.deterministic, false,
  'the report still states what the relation does not establish');

// Criterion 5 uses ordinary native contract preparation, not the specification-quality report. Both
// Bool implementations inhabit the same unconstrained relation. Missing implementation proof remains
// unknown after ordinary admission; neither uniqueness nor a spec-quality artifact is required.
const ordinaryContract = program => contractSource({
  name: 'relational-bool',
  input: '(input)',
  output: '(output (answer Bool))',
  program: `[ ${program} ]`,
  requires: 'true',
  ensures: 'true',
});
for (const value of ['true', 'false']) {
  const ordinary = await report(`ordinary-relational-${value}`, ordinaryContract(value), undefined, 'verify');
  assert.equal(ordinary.status, 3, `${value}: missing implementation proof remains unknown`);
  assert.equal(ordinary.document.outcome, 'unknown');
  assert.equal(ordinary.document.ordinary_typing.outcome, 'accepted',
    `${value}: ordinary admission permits an intentionally relational Bool contract`);
  assert.equal(ordinary.document.independent_recheck, false,
    'ordinary admission must not manufacture implementation proof evidence');
  assert.deepEqual(ordinary.document.diagnostics.map(entry => entry.code), ['evidence-missing']);
}
const wrongOrdinaryType = await report(
  'ordinary-relational-wrong-output-type', ordinaryContract('0'), undefined, 'verify',
);
assert.equal(wrongOrdinaryType.status, 2, 'a permissive postcondition cannot bypass ordinary typing');
assert.equal(wrongOrdinaryType.document.outcome, 'error');
assert.equal(wrongOrdinaryType.document.ordinary_typing, 'not-accepted');
assert.equal(wrongOrdinaryType.document.independent_recheck, false);
assert.deepEqual(wrongOrdinaryType.document.diagnostics.map(entry => entry.code), ['contract-invalid']);

// A reported submission keeps the four obligations apart and never upgrades an unchecked claim.
const { status, document } = await report('reported', relationalSource,
  manifestFor(relationalSource, {
    domain: relationalDomain,
    permitted: relationalPermitted,
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

// Every implementation-only report still supplies the native source, domain and permitted stacks.
for (const [name, checked, vacuous] of [
  ['implementation', true, false],
  ['unchecked-implementation', false, false],
  ['vacuous-implementation', true, true],
]) {
  const implementation = await report(name, incrementSource, manifestFor(incrementSource, {
    domain: incrementDomain,
    permitted: incrementPermitted,
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
const combined = await derived('increment-with-implementation', incrementSource,
  incrementDomain, incrementPermitted,
  [...incrementClaims(), obligation('satisfied', 'implementation-satisfaction', true)]);
assert.equal(combined.status, 0);
assert.deepEqual(combined.document.specification_quality, wrapping.document.specification_quality,
  'implementation evidence cannot change specification-quality rows, tallies or derivation');
assert.equal(wrapping.document.partial_correctness.reported, false);
assert.equal(combined.document.partial_correctness.reported, true);
assert.equal(combined.document.termination.claimed, false);
assert.equal(combined.document.correspondence.claimed, false);

// Finite domain/output bounds and native unsupported interfaces are refusal classes, not evidence.
const structuralSource = contractSource({
  name: 'structural-output',
  input: '(input (p (Pair I64 Bool)))',
  output: '(output (q (Pair Bool I64)))',
  program: '[ unpair swap pair ]',
  ensures: 'true',
});
outcomes.add(await refused('unsupported-native-interface', structuralSource, manifestFor(structuralSource, {
  domain: [{ inputs: [{ i64: 0 }], params: [] }],
  permitted: [[{ i64: 0 }]],
  obligations: [],
}), 'unsupported-relation'));
outcomes.add(await refused('unbounded-domain', incrementSource,
  manifestFor(incrementSource, {
    domain: Array.from({ length: 65 }, () => ({ inputs: [{ i64: 0 }], params: [] })),
    permitted: incrementPermitted,
    obligations: [specification('inhabited')],
  }), 'unsupported-relation'));
outcomes.add(await refused('too-many-permitted-outputs', relationalSource,
  manifestFor(relationalSource, {
    domain: relationalDomain,
    permitted: Array.from({ length: 17 }, () => [{ bool: true }]),
    obligations: [specification('deterministic')],
  }), 'unsupported-relation'));
outcomes.add(await refused('wrong-permitted-output-kind', relationalSource,
  manifestFor(relationalSource, {
    domain: relationalDomain,
    permitted: [[{ bool: true }], [{ i64: 1 }]],
    obligations: [specification('deterministic')],
  }), 'unsupported-relation'));

// A vacuous universal obligation is counted as vacuous, not silently as established evidence.
const vacuous = await report('vacuous', uninhabitedSource, manifestFor(uninhabitedSource, {
  domain: uninhabitedDomain,
  permitted: uninhabitedPermitted,
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

console.log(`spec-quality: refusal classes ${[...outcomes].sort().join(', ')}; native source-bound finite propositions and separated obligations measured`);
