// LSLOT-04 and LSLOT-07's evidence conjuncts on a proof-required slot whose
// claim is Q2 about `[ 2 + ]` (integrator decision 46). LSLOT-07: an exact,
// valid evidence record does not authorize a publication without the host's
// current publication grant, and a target admitted with evidence is not
// dispatched without the current dispatch grant. LSLOT-04: after v2 (the same
// template from a second builder; every instantiation's composite artifact
// binds its own saved owner, so v1 and v2 differ in their artifacts) replaces
// v1, a rollback that re-publishes v1's saved target is a new publication,
// admitted again, that needs the current grant and evidence applicable to v1:
// v1's earlier evidence is not inherited, v2's record does not apply, and only
// v1's exact record, presented again, publishes, at a new epoch and generation.
// The summary is projected by the LSLOT gate.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { cli, evidenceDirectory, expectOutcome, selectedAuthority } from './live_slot_resource_ref_cli.mjs';
import { claimQ2, proofBudget, proofWait, record, requireConsumerEnvironment, saved } from './live_slot_evidence_fixture.mjs';

const binary = process.argv[2];
assert.ok(binary, 'pass the selected noble CLI binary');
requireConsumerEnvironment('LSLOT-04/07 evidence');

const definitions = [
  ['builder', 'def builder [ quote [ + ] compose ]'],
  ['twin', 'def twin [ quote [ + ] compose ]'],
];
const sources = {
  caller: 'slot.invoke',
  ...Object.fromEntries(definitions.flatMap(([name, definition]) =>
    [[name, name], [`definition-${name}`, definition]])),
};
const slot = 'selected';
const slots = [{ slotId: slot, input: ['I64'], output: ['I64'], effectCeiling: [],
  proofRequired: true, claim: claimQ2 }];
const authority = {
  ...selectedAuthority({ sources, slots, effects: ['live.dispatch'], quota: 40,
    grants: ['publish', 'dispatch'].map(operation => ({ operation, slotId: slot, allowed: true })) }),
  proofTimeoutMs: proofBudget,
};
const input = value => ({ kind: 'i64', value: String(value) });
const refs = [{ position: 1, ordinal: 0, slot }];
// A refusal changes nothing. The host refuses some publications before the
// registry is consulted, so such replies carry no epoch; the next successful
// compare-and-swap from the same expected epoch shows the registry unchanged.
const unchanged = (row, epoch) => {
  if (Object.hasOwn(row, 'epoch')) assert.equal(row.epoch, epoch, JSON.stringify(row));
  assert.equal(row.guest_requests, 0, JSON.stringify(row));
  assert.equal(row.protected_operations, 0, JSON.stringify(row));
  return row;
};
const at = (row, epoch, incarnation, generation) => {
  assert.deepEqual([row.epoch, row.incarnation, row.generation], [epoch, incarnation, generation],
    JSON.stringify(row));
  return row;
};
const dispatched = row => row.request_trace.map(step => [step.operation, step.outcome]);

async function lane(name, expectedExit, body) {
  const session = await cli(binary, name, authority);
  const rows = [];
  const issue = async (request, outcome, wait) => {
    const row = expectOutcome(await session.issue(request, wait), outcome);
    rows.push({ request, row });
    return row;
  };
  const grant = (operation, allowed) => issue({ operation: 'policy',
    grant: { operation, slotId: slot, allowed } }, 'policy-updated');
  const publish = (target, evidence, outcome, expected = { expected_epoch: '0' }, wait = proofWait) =>
    issue({ operation: 'publish', slot, owner: target.owner, ...expected,
      ...(evidence ? { evidence } : {}) }, outcome, wait);
  const build = async id => saved(await issue({ operation: 'invoke', id, inputs: [input(2)], refs: [] },
    'executed'));
  let complete = false;
  try {
    expectOutcome(await session.configured, 'configured');
    for (const [id, definition] of definitions) {
      await issue({ operation: 'define', id: `definition-${id}`, name: id, source: definition, inputs: [] },
        'definition-retained');
      await issue({ operation: 'install', id, source: id, selected_name: id, inputs: ['I64'] }, 'installed');
    }
    await issue({ operation: 'install', id: 'caller', source: sources.caller,
      inputs: ['I64', 'LiveRef<I64,I64,pure>'] }, 'installed');
    const summary = await body({ issue, grant, publish, build });
    complete = true;
    return { ...summary, rows: rows.length };
  } finally {
    if (complete) assert.equal(await session.close(), expectedExit, `${name} exit`);
    else await session.abort();
  }
}

// LSLOT-07: the denied dispatch fails the guest, which ends the session (exit 2).
const policy07 = await lane('LSLOT07-evidence-policy', 2, async ({ issue, grant, publish, build }) => {
  const v1 = await build('builder');
  await grant('publish', false);
  const publicationWithoutGrant = unchanged(await publish(v1, record(v1), 'policy-denied'), '0');
  assert.equal(publicationWithoutGrant.generation, null);
  await grant('publish', true);
  const published = at(await publish(v1, record(v1), 'published'), '1', '1', '1');
  const dispatchWithGrant = await issue({ operation: 'invoke', id: 'caller', inputs: [input(20)], refs },
    'executed');
  assert.deepEqual(dispatchWithGrant.stack, [{ kind: 1, value: '22' }]);
  assert.deepEqual(dispatched(dispatchWithGrant), [['dispatch', 'allowed']]);
  assert.equal(dispatchWithGrant.request_trace[0].programValueId, v1.target_identity.program_value_id);
  await grant('dispatch', false);
  const dispatchWithoutGrant = await issue({ operation: 'invoke', id: 'caller', inputs: [input(20)],
    refs }, 'execution-failed');
  assert.deepEqual(dispatched(dispatchWithoutGrant), [['dispatch', 'policy-denied']]);
  assert.equal(dispatchWithoutGrant.request_trace[0].programValueId, v1.target_identity.program_value_id);
  assert.equal(dispatchWithoutGrant.protected_operations, 0);
  return { target: v1.target_identity, publication_without_grant: publicationWithoutGrant,
    published, dispatch_with_grant: dispatchWithGrant, dispatch_without_grant: dispatchWithoutGrant };
});

const rollback04 = await lane('LSLOT04-evidence-rollback', 0, async ({ issue, grant, publish, build }) => {
  const v1 = await build('builder');
  const v2 = await build('twin');
  assert.equal(v1.target_identity.definition_id, v2.target_identity.definition_id, 'one template');
  assert.equal(v1.target_identity.program_value_id, v2.target_identity.program_value_id, 'one capture');
  assert.notEqual(v1.target_identity.artifact_sha256, v2.target_identity.artifact_sha256, 'two artifacts');
  const v1Published = at(await publish(v1, record(v1), 'published'), '1', '1', '1');
  const v2Published = at(await publish(v2, record(v2), 'published',
    { expected_epoch: '1', expected_incarnation: '1', expected_generation: '1' }), '2', '1', '2');
  const v2Dispatch = await issue({ operation: 'invoke', id: 'caller', inputs: [input(20)], refs }, 'executed');
  assert.equal(v2Dispatch.request_trace[0].artifactSha256, v2.target_identity.artifact_sha256);
  // The rollback re-publishes v1's own saved target; the publication admits it
  // again and checks whatever record it carries now.
  const expected = { expected_epoch: '2', expected_incarnation: '1', expected_generation: '2' };
  const withoutEvidence = unchanged(await publish(v1, null, 'refused', expected), '2');
  assert.match(withoutEvidence.diagnostic, /lacks independently checked target evidence/);
  const withV2Evidence = unchanged(await publish(v1, record(v2), 'refused', expected), '2');
  await grant('publish', false);
  const withoutGrant = unchanged(await publish(v1, record(v1), 'policy-denied', expected), '2');
  await grant('publish', true);
  const rolledBack = at(await publish(v1, record(v1), 'published', expected), '3', '1', '3');
  const v1Dispatch = await issue({ operation: 'invoke', id: 'caller', inputs: [input(20)], refs }, 'executed');
  assert.deepEqual(v1Dispatch.stack, [{ kind: 1, value: '22' }]);
  assert.equal(v1Dispatch.request_trace[0].artifactSha256, v1.target_identity.artifact_sha256);
  assert.equal(v1Dispatch.request_trace[0].rootEpoch, '3');
  assert.equal(v1Dispatch.request_trace[0].generation, '3');
  return { v1: v1.target_identity, v2: v2.target_identity, v1_published: v1Published,
    v2_published: v2Published, v2_dispatch: v2Dispatch,
    without_evidence: withoutEvidence, with_v2_evidence: withV2Evidence, without_grant: withoutGrant,
    rolled_back: rolledBack, v1_dispatch: v1Dispatch };
});

const summary = { lslot07: policy07, lslot04: rollback04 };
fs.writeFileSync(path.join(evidenceDirectory, 'LSLOT0407-evidence-summary.json'),
  JSON.stringify(summary, null, 2) + '\n', { flag: 'wx', mode: 0o600 });
console.log(JSON.stringify({ lane: 'LSLOT-04/07 evidence',
  lslot07: { publication_without_grant: policy07.publication_without_grant.outcome,
    published: policy07.published.outcome,
    dispatch_without_grant: dispatched(policy07.dispatch_without_grant) },
  lslot04: { without_evidence: rollback04.without_evidence.diagnostic,
    with_v2_evidence: rollback04.with_v2_evidence.refusal ?? rollback04.with_v2_evidence.diagnostic,
    without_grant: rollback04.without_grant.outcome,
    rolled_back: [rollback04.rolled_back.epoch, rollback04.rolled_back.generation] } }));
