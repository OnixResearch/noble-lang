import assert from 'node:assert/strict';
import { cli, selectedAuthority } from './live_slot_resource_ref_cli.mjs';

const binary = process.argv[2];
assert.ok(binary, 'pass the selected noble CLI binary');
// The host's saved-Program ledger is the reviewed 4096-owner capacity. The copy
// is declared here, not read back from the production report.
const savedProgramCapacity = 4096;
const slots = [{ slotId: 'Q', input: ['I64'], output: ['I64'], effectCeiling: [], proofRequired: false }];
const grants = ['publish', 'dispatch'].map(operation => ({ operation, slotId: 'Q', allowed: true }));
const session = await cli(binary, 'LSLOT-saved-program-capacity', selectedAuthority({
  sources: { keeper: '[ 1 + ]' }, slots, grants, effects: ['live.dispatch'], quota: 8,
}));
const issue = request => session.issue(request);
try {
  assert.equal((await session.configured).outcome, 'configured');
  assert.equal((await issue({ operation: 'install', id: 'keeper', source: '[ 1 + ]',
    inputs: ['I64'] })).outcome, 'installed');
  assert.equal((await issue({ operation: 'publish', slot: 'Q', id: 'keeper', program_index: 1,
    expected_epoch: '0' })).outcome, 'published');
  const owners = [];
  for (let pass = 0; pass < savedProgramCapacity; pass += 1) {
    const row = await issue({ operation: 'invoke', id: 'keeper',
      inputs: [{ kind: 'i64', value: '5' }], refs: [] });
    assert.equal(row.outcome, 'executed', `retained invoke ${pass}: ${JSON.stringify(row).slice(0, 400)}`);
    const saved = row.stack.filter(cell => cell.kind === 4);
    assert.equal(saved.length, 1, `retained invoke ${pass} must save exactly one Program`);
    owners.push(saved[0].owner);
  }
  assert.equal(new Set(owners).size, savedProgramCapacity,
    'every retained invocation owns a distinct saved Program');
  // The root past the ledger is refused before the guest runs: no effects, no
  // dropped trace rows, and a diagnostic that names the capacity it exhausted.
  const refused = await issue({ operation: 'invoke', id: 'keeper',
    inputs: [{ kind: 'i64', value: '5' }], refs: [] });
  assert.equal(refused.outcome, 'refused', JSON.stringify(refused).slice(0, 400));
  assert.equal(refused.stage, 'command');
  assert.equal(refused.guest_requests, 0);
  assert.equal(refused.protected_operations, 0);
  assert.match(refused.diagnostic, new RegExp(`saved Program capacity ${savedProgramCapacity}`));
  // A refused root leaves the session usable: releasing one owner makes room.
  assert.equal((await issue({ operation: 'release-program', owner: owners[0] })).outcome,
    'program-released');
  const admitted = await issue({ operation: 'invoke', id: 'keeper',
    inputs: [{ kind: 'i64', value: '5' }], refs: [] });
  assert.equal(admitted.outcome, 'executed', JSON.stringify(admitted).slice(0, 400));
  assert.equal(admitted.stack.filter(cell => cell.kind === 4).length, 1);
  assert.equal(await session.close(), 0,
    'a capacity refusal is a command refusal, never a poisoned session');
  console.log(`saved Program capacity ${savedProgramCapacity} refused the over-budget root before execution`);
} catch (error) {
  await session.abort();
  throw error;
}
