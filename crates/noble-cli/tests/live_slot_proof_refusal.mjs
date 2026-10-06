import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { canonical, cli, evidenceDirectory, expectOutcome, selectedAuthority } from './live_slot_resource_ref_cli.mjs';

const binary = process.argv[2];
const chosen = canonical('LSLOT-05');
assert.equal(chosen.input.policy, 'proof-required-for-this-slot');
const source = '1 +';
const slots = ['proof-required', 'ordinary-control'].map(slotId => ({ slotId,
  input: ['I64'], output: ['I64'], effectCeiling: [], proofRequired: slotId === 'proof-required' }));
const session = await cli(binary, 'LSLOT05-proof-required-refusal-control', selectedAuthority({
  sources: { target: source }, slots, grants: slots.map(slot => ({
    operation: 'publish', slotId: slot.slotId, allowed: true,
  })), effects: [],
}));
try {
  expectOutcome(await session.configured, 'configured');
  const installed = expectOutcome(await session.issue({ operation: 'install', id: 'target',
    source, inputs: ['I64'] }), 'installed');
  assert.deepEqual([installed.guest_requests, installed.protected_operations], [0, 0]);
  const staged = expectOutcome(await session.issue({ operation: 'candidate', id: 'target' }), 'candidate-staged');
  assert.deepEqual([staged.guest_requests, staged.protected_operations], [0, 0]);
  const refused = expectOutcome(await session.issue({ operation: 'publish', slot: 'proof-required',
    id: 'target', expected_epoch: '0' }), 'refused');
  assert.match(refused.diagnostic, /proof-required slot lacks independently checked target evidence/);
  assert.deepEqual([refused.guest_requests, refused.protected_operations], [0, 0]);
  const before = expectOutcome(await session.issue({ operation: 'trace' }), 'trace-observed');
  assert.deepEqual(before.trace, []);
  const ordinary = expectOutcome(await session.issue({ operation: 'publish', slot: 'ordinary-control',
    id: 'target', expected_epoch: '0' }), 'published');
  assert.equal(ordinary.epoch, '1');
  assert.deepEqual([ordinary.guest_requests, ordinary.protected_operations], [0, 0]);
  const after = expectOutcome(await session.issue({ operation: 'trace' }), 'trace-observed');
  assert.deepEqual(after.trace, []);
  assert.equal(await session.close(), 0);
  const summaryFile = path.join(evidenceDirectory, 'LSLOT05-proof-required-refusal-control.summary.json');
  const summary = { status: 'observed', selected_binary_sha256: session.selectedBinarySha256,
    installed_artifact_sha256: installed.artifact_sha256,
    proof_required: { outcome: refused.outcome, diagnostic: refused.diagnostic,
      guest_requests: refused.guest_requests, protected_operations: refused.protected_operations },
    ordinary_control: { outcome: ordinary.outcome, epoch: ordinary.epoch },
    raw: session.log, requests: session.requests, responses: session.responses,
    stderr: session.stderrFile,
    limitation: 'blanket missing-evidence refusal does not distinguish old/capture/context/claim/assumption/artifact evidence or establish exact selected-target proof' };
  fs.writeFileSync(summaryFile, JSON.stringify(summary, null, 2));
  console.log(JSON.stringify({ summary: summaryFile, observation: summary }));
} catch (error) {
  await session.abort();
  throw Error(`proof-required refusal: ${error} raw=${session.log}`, { cause: error });
}
