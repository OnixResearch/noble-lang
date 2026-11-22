// Shared selected-target evidence fixture for the proof-required live-slot
// lanes (LSLOT-05 and the LSLOT-04/07 evidence conjuncts): the slot claim Q2
// about `[ 2 + ]`, the MC2 contracts and Lean proofs the lanes present, and
// the evidence record a publication carries for a checked returned target.
import assert from 'node:assert/strict';
import { digest } from './live_slot_resource_ref_cli.mjs';

// The consumer's Lean recheck needs the reviewed sandbox tools and the contract
// library; the gate supplies them, and the CLI child inherits this environment.
export function requireConsumerEnvironment(lane) {
  for (const name of ['NOBLE_LEAN', 'NOBLE_BWRAP', 'NOBLE_PRLIMIT', 'NOBLE_SYSTEMD_RUN', 'NOBLE_CONTRACT_LIBRARY'])
    assert.ok(process.env[name], `the ${lane} lane needs ${name}`);
}
export const proofBudget = 600_000;
export const proofWait = proofBudget + 60_000;

export const contract = (name, body, input, result) => `(contract 1 ${name}
  (input (x I64))
  (output (y I64))
  (program ${body})
  (requires (eq (in x) ${input}))
  (ensures (eq (out y) ${result})))
`;
export const proof = (input, result) => `import MC1Obligation
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
export const claimQ2 = contract('lslot05', '[ 2 + ]', 20, 22);
export const claimQ1 = contract('lslot05-old', '[ 2 - ]', 20, 18);
export const claimOther = contract('lslot05-other', '[ 2 + ]', 21, 23);
export const proofQ2 = proof(20, 22);
export const proofQ1 = proof(20, 18);

export const assumptions = ['propext', 'Classical.choice', 'Quot.sound'];
export const context = (semanticRevision = 0) => ({ semantic_revision: semanticRevision, policy_revision: 1 });
// The exact record for a checked returned target under Q2; each override
// changes exactly the named field.
export const record = (target, overrides = {}) => ({
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
// The checked returned target a builder invocation saved on the stack.
export const saved = row => {
  const program = row.stack[0];
  assert.equal(program.kind, 4);
  assert.ok(program.owner && program.target_identity?.artifact_sha256, 'a checked returned target');
  return program;
};
