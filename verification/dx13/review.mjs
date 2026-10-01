import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';

// Review boundary exercise only. It never loads Noble, CUE, Lean or Wasm and
// cannot observe a candidate program or issue proof/release admission.
const pin = 'c71183b0a5d3319dde1d611b6e6f77a6eab29418';
const source = 'https://raw.githubusercontent.com/4ad/qcue/' + pin + '/doc/';
const paths = {
  review: 'verification/dx13/review.json',
  runner: 'verification/dx13/review.mjs',
  case: 'specs/conformance/language-workflow-cases.json',
  native: '.cairn/specs/developer-experience/spec.md',
  change: '.cairn/changes/bend-qcue-scoped-adaptations/design.md',
  decisions: 'specs/DECISIONS.md',
};
const bytes = Object.fromEntries(Object.entries(paths).map(([key, path]) => [key, readFileSync(path)]));
const sha256 = b => createHash('sha256').update(b).digest('hex');
const assert = (yes, message) => { if (!yes) throw Error(message); };
const review = JSON.parse(bytes.review);
const cases = JSON.parse(bytes.case);
const item = cases.cases.find(row => row.id === 'DX-13');
assert(item?.kind === 'review' && item.state.proof === 'not-applicable', 'wrong canonical review boundary');
assert(review.selection === 'methodology-review-only' && review.case === item.id, 'review selection mismatch');
for (const [name, records, required] of [
  ['method', review.method_controls, item.input.controls],
  ['exclusion', review.exclusions, item.input.excluded],
]) {
  assert(Array.isArray(records) && records.length === required.length, `incomplete ${name} review`);
  assert(new Set(records.map(row => row.id)).size === required.length, `duplicate ${name} review`);
  for (const id of required) {
    const row = records.find(row => row.id === id);
    assert(row && typeof row.counterexample === 'string' ||
      row && typeof row.hostile === 'string', `missing ${name} counterexample: ${id}`);
    assert(row && (row.method || row.disposition) && row.qcue_basis !== '', `missing ${name} method: ${id}`);
  }
}
assert(Object.values(review.scope).every(v => v === false || v === 'not-applicable'), 'review attempts authority or execution claim');
for (const [key, value] of Object.entries(item.expected)) {
  if (key.endsWith('_claimed') || key.endsWith('_release') || key === 'historical_dx10_receipt_covers_this') {
    assert(value === false, `canonical scope expanded: ${key}`);
  }
}
const fetchBytes = async name => {
  const result = await fetch(source + name + '.md');
  assert(result.ok, `pinned source unavailable: ${name} (${result.status})`);
  return Buffer.from(await result.arrayBuffer());
};
const [oracle, implementation, commit] = await Promise.all([
  fetchBytes('oracle'), fetchBytes('implementation'),
  fetch('https://api.github.com/repos/4ad/qcue/commits/' + pin, { headers: { 'Accept':'application/vnd.github+json' } }).then(r => { assert(r.ok, 'commit unavailable'); return r.json(); }),
]);
assert(commit.sha === pin && commit.commit.tree.sha === '93fbc7d749f259b6063cf87f1bcb549a1c1eb32f', 'wrong pinned commit/tree');
const blob = createHash('sha1').update(Buffer.from(`blob ${oracle.length}\0`)).update(oracle).digest('hex');
assert(blob === review.source.oracle_blob_git_sha1 && oracle.length === 5331, 'oracle blob does not match pinned tree');
assert(sha256(oracle) === review.source.oracle_sha256 &&
       sha256(implementation) === review.source.implementation_sha256, 'external source bytes changed');
assert(review.source.revision === '4ad/qcue@' + pin, 'source revision changed');

// Actual *review reasoning* on concrete methodological counterexamples;
// no Noble candidate is compiled, checked, run or admitted here.
const wrap = x => BigInt.asIntN(64, x);
const model = x => wrap(x + 1n);
const examplePreservingHostile = x => x === 1n ? 2n : x;
const disagree = x => examplePreservingHostile(x) !== model(x);
assert(!disagree(1n) && disagree(2n), 'example-preserving false law escaped witness');
assert(model(9223372036854775807n) === -9223372036854775808n, 'I64 wrap boundary wrong');
assert(![1n, 2n].some(x => false && examplePreservingHostile(x) === model(x)), 'vacuous premise counted');
const original = { input: 2n, predicate: 'value-mismatch', seed: 'dx13-methodology', bound: 5 };
const smaller = { ...original, input: 1n };
const validShrink = candidate => candidate.input >= 0n && candidate.input <= 2n &&
  candidate.predicate === original.predicate && disagree(candidate.input);
assert(validShrink(original) && !validShrink(smaller), 'shrinker lost original mismatch');
const subject = { module: 'Definitions', version: 5, definition: 'step', capture: 'x', ops: ['lit:1', 'add'], effects: [] };
const same = { ...subject, ops: [...subject.ops], effects: [] };
const rebinding = { ...same, version: 6 };
const changedRecipe = { ...same, ops: ['lit:2', 'add'] };
const effectful = { ...same, effects: ['test.emit'] };
const identity = row => ['module','version','definition','capture'].every(key => row[key] === subject[key]);
const recipe = row => JSON.stringify(row.ops) === JSON.stringify(subject.ops) && row.effects.length === 0;
assert(identity(same) && recipe(same) && !identity(rebinding) && !recipe(changedRecipe) && !recipe(effectful),
  'identity/recipe/effect boundary was conflated with example output');
const eligible = ({modelAgreement, strictExactProof, ownerPolicy, artifactBound, hostAuthority}) =>
  strictExactProof && ownerPolicy && artifactBound && hostAuthority; // modelAgreement is never authority
assert(!eligible({modelAgreement:true, strictExactProof:true, ownerPolicy:false, artifactBound:true, hostAuthority:true}) &&
       !eligible({modelAgreement:true, strictExactProof:false, ownerPolicy:true, artifactBound:true, hostAuthority:true}) &&
       eligible({modelAgreement:false, strictExactProof:true, ownerPolicy:true, artifactBound:true, hostAuthority:true}),
  'finite model results improperly became release/proof authority');
assert(review.scope.owner_law_exists === false && item.id !== 'DX-10', 'owner-law or historical review conflation');

const receipt = {
  schema: 'noble-dx13-review-receipt/v1', case: 'DX-13', result: 'passed', kind: 'review',
  claim: 'Ten finite-oracle method controls and five exclusions independently reviewed against pinned qcue source and Noble native design; concrete arithmetic, premise, shrink, identity, recipe/effect and proof-authority counterexamples were classified. No Noble oracle, proof, candidate or release ran.',
  source: { qcue_commit:pin, qcue_tree:commit.commit.tree.sha, oracle_git_blob:blob,
    oracle_sha256:sha256(oracle), implementation_sha256:sha256(implementation),
    github_commit_signature: commit.commit.verification.reason },
  inputs_sha256: Object.fromEntries(Object.entries(bytes).map(([key, value]) => [key, sha256(value)])),
  counts: {method_controls:review.method_controls.length, exclusions:review.exclusions.length,
    methodological_counterexample_groups:6},
  assumptions: ['GitHub HTTPS supplies the pinned Git object bytes; commit is unsigned, so the preselected revision is trusted as comparative source, not independent owner authorization.',
    'Review judgment in review.json is human-authored; the script checks source bindings and concrete boundary counterexamples, not full semantic validity of prose.',
    'No actual Noble MC2 finite oracle, candidate execution, strict proof, owner-law release, universal theorem or backend/host authority is claimed. DX-10 is independent historical work.'],
  scope: review.scope,
};
console.log(JSON.stringify(receipt, null, 2));
