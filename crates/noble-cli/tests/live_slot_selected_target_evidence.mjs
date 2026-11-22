// LSLOT-05: proof-required selected-target admission. A proof-required slot
// selects the exact claim Q2 about the returned target P2/D2 (`[ 2 + ]`, the
// builder `quote [ + ] compose` applied to 2). Each publication carries one
// evidence record; the host compares every declared field with the target,
// rechecks the proof with the consumer route of `noble build --require-proof`
// and binds the accepted claim to the target's own closed subject. Only the
// exact record publishes. The summary is projected onto the case by the gate.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { cli, digest, evidenceDirectory, expectOutcome, selectedAuthority } from './live_slot_resource_ref_cli.mjs';

const binary = process.argv[2];
assert.ok(binary, 'pass the selected noble CLI binary');
// The consumer's Lean recheck needs the reviewed sandbox tools and the contract
// library; the gate supplies them, and the CLI child inherits this environment.
for (const name of ['NOBLE_LEAN', 'NOBLE_BWRAP', 'NOBLE_PRLIMIT', 'NOBLE_SYSTEMD_RUN', 'NOBLE_CONTRACT_LIBRARY'])
  assert.ok(process.env[name], `the selected-target evidence lane needs ${name}`);
const proofBudget = 600_000;
const proofWait = proofBudget + 60_000;

const contract = (name, body, input, result) => `(contract 1 ${name}
  (input (x I64))
  (output (y I64))
  (program ${body})
  (requires (eq (in x) ${input}))
  (ensures (eq (out y) ${result})))
`;
const proof = (input, result) => `import MC1Obligation
import NobleContracts
import NobleContracts.Expression

open NobleContracts

namespace MC1Proof

theorem proof : MC1Obligation.claim := by
  intro tail before params hi _hp hpre final he
  obtain ⟨n, rfl⟩ := (stackTyped_i64_iff before).mp hi
  have hn : n = ${input} := by
    simpa [MC1Obligation.precondition, MC1Obligation.expression_2, MC1Obligation.expression_0,
      MC1Obligation.expression_1, Holds, evaluate] using hpre
  subst hn
  have hfinal : final = tail ++ [.i64 ${result}] := by
    simpa [MC1Obligation.program, MC1Obligation.node_0, MC1Obligation.node_1, Exec] using he
  refine ⟨[.i64 ${result}], hfinal, .cons .i64 .nil, ?_⟩
  simp [MC1Obligation.postcondition, MC1Obligation.expression_5, MC1Obligation.expression_3,
    MC1Obligation.expression_4, Holds, evaluate]

end MC1Proof
`;
// Q2 is the slot's claim about P2; Q1 is the old version's claim about its own
// target; the other claim is about the same subject with another statement.
const claimQ2 = contract('lslot05', '[ 2 + ]', 20, 22);
const claimQ1 = contract('lslot05-old', '[ 2 - ]', 20, 18);
const claimOther = contract('lslot05-other', '[ 2 + ]', 21, 23);
const proofQ2 = proof(20, 22);
const proofQ1 = proof(20, 18);

const definitions = [
  ['builder', 'builder', 'def builder [ quote [ + ] compose ]'],
  ['previous', 'previous', 'def previous [ quote [ - ] compose ]'],
];
const sources = Object.fromEntries(definitions.flatMap(([id, name, definition]) =>
  [[id, name], [`definition-${id}`, definition]]));
// The generic caller whose proof must not transfer: `slot.invoke` over a
// selected live reference, identified by its own source.
const genericCaller = `caller-generic:${digest('slot.invoke')}`;
const slots = [{ slotId: 'selected', input: ['I64'], output: ['I64'], effectCeiling: [],
  proofRequired: true, claim: claimQ2 }];
const authority = {
  ...selectedAuthority({ sources, slots, effects: ['live.dispatch'], quota: 40,
    grants: [{ operation: 'publish', slotId: 'selected', allowed: true }] }),
  proofTimeoutMs: proofBudget,
};
const session = await cli(binary, 'LSLOT05-selected-target-evidence', authority);
const rows = [];
const issue = async (request, outcome, wait) => {
  const row = expectOutcome(await session.issue(request, wait), outcome);
  rows.push({ request, row });
  return row;
};
const input = value => ({ kind: 'i64', value: String(value) });
const saved = row => {
  const program = row.stack[0];
  assert.equal(program.kind, 4);
  assert.ok(program.owner && program.target_identity?.artifact_sha256, 'a checked returned target');
  return program;
};
const context = (semanticRevision = 0) => ({ semantic_revision: semanticRevision, policy_revision: 1 });
const assumptions = ['propext', 'Classical.choice', 'Quot.sound'];
const record = (target, overrides = {}) => ({
  subject: { program_value_id: target.target_identity.program_value_id,
    definition_id: target.target_identity.definition_id },
  captures: target.target_identity.captures,
  context: context(),
  claim_sha256: digest(claimQ2),
  assumptions,
  artifact_sha256: target.target_identity.artifact_sha256,
  proof: proofQ2,
  ...overrides,
});
const publish = (target, evidence, outcome = 'refused', wait = 30_000) => issue({ operation: 'publish',
  slot: 'selected', owner: target.owner, expected_epoch: '0', evidence }, outcome, wait);

let complete = false;
const summary = { variants: {}, controls: {} };
try {
  expectOutcome(await session.configured, 'configured');
  for (const [id, name, definition] of definitions)
    await issue({ operation: 'define', id: `definition-${id}`, name, source: definition, inputs: [] },
      'definition-retained');
  const installations = {};
  for (const [id, name] of definitions)
    installations[id] = await issue({ operation: 'install', id, source: name, selected_name: name,
      inputs: ['I64'] }, 'installed');
  const p2 = saved(await issue({ operation: 'invoke', id: 'builder', inputs: [input(2)], refs: [] },
    'executed'));
  const p1 = saved(await issue({ operation: 'invoke', id: 'builder', inputs: [input(1)], refs: [] },
    'executed'));
  const old = saved(await issue({ operation: 'invoke', id: 'previous', inputs: [input(2)], refs: [] },
    'executed'));
  summary.target = p2.target_identity;
  summary.capture_one = p1.target_identity;
  summary.old_version = old.target_identity;
  summary.assumptions = assumptions;
  assert.deepEqual(p2.target_identity.captures, [{ type: 'I64', value: '2' }]);
  assert.equal(p1.target_identity.definition_id, p2.target_identity.definition_id);
  assert.notEqual(p1.target_identity.program_value_id, p2.target_identity.program_value_id);
  assert.notEqual(old.target_identity.definition_id, p2.target_identity.definition_id);

  const variant = (name, row) => { summary.variants[name] = row; return row; };
  // Without evidence the proof-required slot refuses as before.
  summary.no_evidence = await issue({ operation: 'publish', slot: 'selected', owner: p2.owner,
    expected_epoch: '0' }, 'refused');
  assert.match(summary.no_evidence.diagnostic, /lacks independently checked target evidence/);
  // Each refused variant changes exactly its named field of the exact record.
  variant('old-version-proof', await publish(p2, record(p2, {
    subject: { program_value_id: old.target_identity.program_value_id,
      definition_id: old.target_identity.definition_id },
    claim_sha256: digest(claimQ1), proof: proofQ1 })));
  variant('same-recipe-different-capture', await publish(p2, record(p2, {
    captures: p1.target_identity.captures })));
  variant('wrong-context', await publish(p2, record(p2, { context: context(1) })));
  variant('wrong-claim', await publish(p2, record(p2, { claim_sha256: digest(claimOther) })));
  variant('wrong-assumption', await publish(p2, record(p2, {
    assumptions: ['propext', 'Classical.choice', 'Quot.sound', 'sorryAx'] })));
  variant('right-recipe-wrong-artifact', await publish(p2, record(p2, {
    artifact_sha256: p1.target_identity.artifact_sha256 })));
  variant('generic-wrapper-proof-only', await publish(p2, record(p2, {
    subject: { program_value_id: genericCaller, definition_id: genericCaller } })));
  // Controls that reach the consumer: a record naming the exact target whose
  // proof is about another subject, and the slot's own valid proof offered for
  // the capture-1 target, whose subject `[ 1 + ]` the core cannot bind.
  summary.controls.forged_record = await publish(p2, record(p2, { proof: proofQ1 }), 'refused', proofWait);
  summary.controls.capture_one_binding = await publish(p1, record(p1, {
    claim_sha256: digest(claimQ2) }), 'refused', proofWait);
  // The exact record publishes, and nothing refused before it published.
  const exact = variant('exact-selected-subject', await publish(p2, record(p2), 'published', proofWait));
  // The proof does not alter the selected version's identity: a fresh
  // instantiation after the publication has the same P/D and manifest.
  const again = saved(await issue({ operation: 'invoke', id: 'builder', inputs: [input(2)], refs: [] },
    'executed'));
  summary.after_publication = again.target_identity;
  summary.exact = exact;
  complete = true;
} finally {
  if (complete) assert.equal(await session.close(), 0);
  else await session.abort();
}
summary.rows = rows.length;
fs.writeFileSync(path.join(evidenceDirectory, 'LSLOT05-summary.json'), JSON.stringify(summary, null, 2) + '\n',
  { flag: 'wx', mode: 0o600 });
console.log(JSON.stringify({ lane: 'LSLOT-05', variants: Object.fromEntries(Object.entries(summary.variants)
  .map(([name, row]) => [name, row.outcome === 'refused' ? row.refusal ?? row.diagnostic : row.outcome])),
  controls: Object.fromEntries(Object.entries(summary.controls)
    .map(([name, row]) => [name, row.refusal ?? row.diagnostic])) }));
