import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';

// This script audits authored observations against frozen source and exercises
// claim-scope counterexamples. It neither implements an admission service nor
// runs a compiler, a proof checker, a Component engine or a release procedure.
const paths = {
  review: 'verification/release-policy/review.json',
  runner: 'verification/release-policy/review.mjs',
  witCase: 'specs/conformance/wit-wasi-cases.json',
  safetyCase: 'specs/conformance/safety-cases.json',
  witSpec: '.cairn/specs/wit-wasi/spec.md',
  safetySpec: '.cairn/specs/safety/spec.md',
  languageSpec: '.cairn/specs/language/spec.md',
  verificationSpec: '.cairn/specs/verification/spec.md',
  ledger: 'specs/verification/obligations.json',
  m5: 'verification/m5/evidence.json',
  m5Pins: 'verification/m5/pins.json',
  m6: 'verification/m6/evidence.json',
  m6Pins: 'verification/m6/pins.json',
  m6Runtime: 'verification/m6/acceptance.json',
};
const bytes = Object.fromEntries(Object.entries(paths).map(([key, path]) => [key, readFileSync(path)]));
const json = key => JSON.parse(bytes[key]);
const text = key => bytes[key].toString('utf8');
const sha256 = input => createHash('sha256').update(input).digest('hex');
const assert = (condition, message) => { if (!condition) throw Error(message); };
const review = json('review');
const wi = json('witCase').cases.find(row => row.id === 'WI-17');
const safety = json('safetyCase').cases.find(row => row.id === 'S-CASE-15');
const m5 = json('m5');
const m6 = json('m6');
const m5Pins = json('m5Pins');
const m6Pins = json('m6Pins');
const runtime = json('m6Runtime');
const obligation = id => json('ledger').obligations.find(row => row.id === id);
const requirement = (key, id, snippet) => {
  const body = text(key).split(`### Requirement: ${id}\n`)[1]?.split('### Requirement: ')[0];
  assert(body?.includes(snippet), `missing source requirement ${id}: ${snippet}`);
};
assert(review.selection === 'independent-source-bound-review-only' &&
  JSON.stringify(review.cases) === JSON.stringify([wi?.id, safety?.id]), 'review/case identity mismatch');
for (const [caseRow, claim, result] of [[wi, 'Component-Draft', 'reject-full-profile-claim'],
  [safety, 'stable-safe', 'reject-claim']]) {
  assert(caseRow.kind === 'review' && caseRow.profile === 'release-policy' &&
    caseRow.input.requested_claim === claim && caseRow.expected.stage === 'release-policy' &&
    caseRow.expected.outcome === result && caseRow.state.execution === 'not-run' &&
    caseRow.state.proof === 'not-applicable', `changed prepromotion design: ${caseRow?.id}`);
}
assert(wi.input.implemented_profiles.includes(m5Pins.profile) &&
  wi.input.implemented_profiles.includes(m6Pins.profile) &&
  wi.input.implemented_profiles.length === 2 &&
  wi.input.stable_wasi_0_3_selected === false &&
  wi.input.async_test_execution === 'passed-selected-M6' &&
  wi.input.broader_assurance === 'open', 'WI-17 input no longer reflects actual selected slices');
assert(m5.result === 'passed' && m5Pins.profile === 'Component-Sync-Bootstrap' &&
  m5Pins.world === 'noble-test:sync/bootstrap@1.0.0' &&
  m5Pins.canonical_abi === 'memory32-sync-utf8' &&
  m5Pins.non_claims.includes('Component-Draft') &&
  m5Pins.non_claims.includes('WASI profile implementation') &&
  m5Pins.non_claims.includes('native async/future/stream execution'), 'M5 slice or pins misrepresented');
assert(m6.result === 'passed' && m6.runtime.result === 'passed' &&
  m6Pins.profile === 'Component-Async-Bootstrap' &&
  m6Pins.canonical_abi.includes('async-lower-stackful-lift-task-return') &&
  runtime.result === 'passed' && runtime.profile === m6Pins.profile &&
  ['WI-11', 'WI-12', 'WI-16'].every(id => runtime.summary.passed_cases.includes(id)) &&
  ['compatibility-wasi-p3-clock','compatibility-wasi-stable-version-rejected'].every(id =>
    runtime.summary.executed_controls.includes(id)), 'M6 runtime selection not verified');
assert(m6.runtime.receipt_sha256 === sha256(bytes.m6Runtime) &&
  JSON.stringify(runtime.pins.wasi) === JSON.stringify(m6Pins.wasi), 'M6 receipt/pin binding differs');
assert(m6Pins.wasi.version === '0.3.0-rc-2025-09-16' &&
  m6Pins.wasi.stable_0_3_0 === 'rejected at link; not silently substituted' &&
  m6.non_claims.includes('stable WASI 0.3 or full WASI/Component-Draft'), 'prerelease pin falsely promoted');
const clock = runtime.controls.find(row => row.name === 'compatibility-wasi-p3-clock');
const stable = runtime.controls.find(row => row.name === 'compatibility-wasi-stable-version-rejected');
assert(clock?.result === 'passed' && stable?.result === 'passed' &&
  clock.evidence.some(row => row.report?.operation === `${m6Pins.wasi.executed_interface}#wait-for`) &&
  stable.evidence.some(row => row.report?.compatibility?.imports_started === 0),
  'selected compatibility receipts missing executable observations');
for (const id of ['WI-ARCH-01','WI-ARCH-02','WI-ARCH-03','WI-ARCH-04']) requirement('witSpec', id, '**'+id+'.**');
requirement('witSpec','WI-WASI-02','stable WASI 0.3');
requirement('witSpec','WI-SAFE-03','async cancellation/cleanup');
requirement('safetySpec','S-GATE-03','higher-rank polymorphism');
requirement('safetySpec','S-VERIFY-03','unsupported feature');
requirement('verificationSpec','V-SAFE-04','declarative rules, acceptance checks, implementation correspondence');
assert(text('languageSpec').includes('This revision does not introduce higher-rank polymorphism') &&
  safety.input.feature === 'higher-rank' && safety.input.checker_status === 'unsupported' &&
  safety.input.proof_status === 'open', 'higher-rank status contradicted by source');
assert(['SO-01','SO-07'].every(id => obligation(id)?.status === 'open'),
  'broader safety proof ledger was misrepresented');

// Counterexamples distinguish the real selected evidence from an enlarged
// description of the very same evidence. A scoped receipt remains a true
// scoped claim; renaming it cannot add tests, a stable linker or proofs.
const scopedM5 = { profile:m5Pins.profile, world:m5Pins.world, abi:m5Pins.canonical_abi };
const scopedM6 = { profile:m6Pins.profile, world:m6Pins.world, wasi:m6Pins.wasi.version };
assert(scopedM5.profile === 'Component-Sync-Bootstrap' &&
  scopedM6.profile === 'Component-Async-Bootstrap' &&
  scopedM5.world !== scopedM6.world, 'selected positive controls lost');
assert(scopedM5.profile !== wi.input.requested_claim &&
  scopedM6.profile !== wi.input.requested_claim &&
  scopedM6.wasi !== '0.3.0' &&
  runtime.summary.passed_cases.every(id => id !== wi.id),
  'scoped execution incorrectly relabelled as full-profile execution');
assert(safety.input.checker_status !== 'supported' &&
  safety.input.proof_status !== 'accepted' &&
  obligation('PO-09')?.status === 'accepted' &&
  obligation('PO-09').claim.includes('bootstrap fragment v1'),
  'bootstrap proof improperly widened to higher-rank');
const required = ['sync-slice-is-not-full-profile','async-slice-is-executed-but-not-stable-wasi',
  'component-boundary-and-safety-gap','higher-rank-is-open-not-prohibited'];
assert(review.observations.length === required.length &&
  required.every((id, index) => review.observations[index].id === id &&
    paths[Object.keys(paths).find(key => paths[key] === review.observations[index].source)] &&
    review.observations[index].observation.length > 60 &&
    review.observations[index].counterexample.length > 60), 'review observation is incomplete');
assert(Object.values(review.boundaries).every(value => value === false), 'review claims execution or authority');
const receipt = {
  schema: 'noble-release-policy-review-receipt/v1',
  result: 'passed', kind: 'review', cases:review.cases,
  claim: 'Source-bound review rejects full Component-Draft and higher-rank stable-safe claims while retaining actually passed M5/M6 selections and prerelease pins; neither review implements release nor closes proofs.',
  inputs_sha256: Object.fromEntries(Object.entries(bytes).map(([key, value]) => [key, sha256(value)])),
  observations: review.observations.map(({id,source}) => ({id,source})),
  controls: {m5_sync_selected:true,m6_async_selected:true,m6_prerelease_clock_peer_only:true,
    stable_link_rejected:true,full_profile_execution_claimed:false,higher_rank_proof_claimed:false},
  assumptions: ['Authored review judgments are not mechanically proved by source-text assertions; this checks bindings and concrete scope counterexamples only.',
    'Existing M5/M6 receipts are scoped historical evidence, not rerun here; broader host authenticity, ABI, native retirement and formal obligations remain open.',
    'No release implementation, stable WASI profile, higher-rank checker, universal safety theorem or proof admission is supplied by this review.'],
};
console.log(JSON.stringify(receipt, null, 2));
