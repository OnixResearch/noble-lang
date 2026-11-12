import assert from 'node:assert/strict';
import { cli, selectedAuthority } from './live_slot_resource_ref_cli.mjs';

const binary = process.argv[2];
assert.ok(binary, 'pass the selected noble CLI binary');
const QUOTE = String.fromCharCode(34);
const definition = `def traced [ drop ${QUOTE}A${QUOTE} test.emit drop 21 ]`;
const effects = ['live.dispatch', 'test.emit'];
const callerInput = 'LiveRef<I64,I64,live.dispatch+test.emit>';
const slots = [
  { slotId: 'S1', input: ['I64'], output: ['I64'], effectCeiling: effects, proofRequired: false },
  { slotId: 'B', input: ['I64', callerInput], output: ['I64'], effectCeiling: effects,
    proofRequired: false },
];
const grantsFor = effectAllowed => ['publish', 'dispatch'].flatMap(operation =>
  slots.map(slot => ({ operation, slotId: slot.slotId, allowed: true })))
  .concat(slots.map(slot => ({ operation: 'effect', slotId: slot.slotId,
    allowed: effectAllowed(slot.slotId) })));

/**
 * Authority-file path: a nominal that spells the same identifier as the denied slot cannot be
 * granted the protected effect. The loader refuses the whole authority, so the effect is not
 * merely denied by ordering: the widening grant never enters the session at all.
 */
const authorityScope = async () => {
  const grants = grantsFor(slotId => slotId !== 'S1');
  // The finding's exact trigger: deny the slot-scoped effect, then allow the same identifier
  // through the nominal namespace.
  grants.push({ operation: 'effect', nominalId: 'S1', allowed: true });
  const session = await cli(binary, 'grant-scope-authority', selectedAuthority({
    sources: { definition, named: 'traced', caller: 'slot.invoke' },
    slots, grants, effects, quota: 8,
  }));
  const row = await session.configured;
  assert.equal(row.outcome, 'refused', JSON.stringify(row).slice(0, 500));
  assert.equal(row.stage, 'authority');
  assert.match(row.diagnostic, /effect grant requires a slot scope/);
  assert.equal(await session.close(), 2,
    'an authority whose effect grant names a nominal is refused before the session exists');
  return { outcome: row.outcome, stage: row.stage, diagnostic: row.diagnostic };
};

/**
 * Runtime-policy path: with the effect denied on slot S1, a policy request naming the same
 * identifier as a nominal may not mint a grant. The host refuses the widening decision, and the
 * following dispatch still records a denied effect rather than performing it.
 */
const runtimePolicyScope = async () => {
  const session = await cli(binary, 'grant-scope-runtime', selectedAuthority({
    sources: { definition, named: 'traced', caller: 'slot.invoke' },
    slots, grants: grantsFor(slotId => slotId === 'B'),
    effects, quota: 8,
  }));
  const expectRow = async (promise, outcome) => {
    const row = await promise;
    assert.equal(row.outcome, outcome, JSON.stringify(row).slice(0, 600));
    return row;
  };
  try {
    await expectRow(session.configured, 'configured');
    await expectRow(session.issue({ operation: 'define', id: 'definition', name: 'traced',
      source: definition, inputs: [] }), 'definition-retained');
    await expectRow(session.issue({ operation: 'install', id: 'named', source: 'traced',
      inputs: ['I64'], selected_name: 'traced' }), 'installed');
    await expectRow(session.issue({ operation: 'candidate', id: 'named' }), 'candidate-staged');
    await expectRow(session.issue({ operation: 'install', id: 'caller', source: 'slot.invoke',
      inputs: ['I64', callerInput] }), 'installed');
    await expectRow(session.issue({ operation: 'candidate', id: 'caller' }), 'candidate-staged');
    const leaf = await expectRow(session.issue({ operation: 'publish', slot: 'S1', id: 'named',
      program_index: 0, expected_epoch: '0' }), 'published');
    await expectRow(session.issue({ operation: 'publish', slot: 'B', id: 'caller',
      program_index: 0, expected_epoch: leaf.epoch }), 'published');
    const widened = await session.issue({ operation: 'policy',
      grant: { operation: 'effect', nominalId: 'S1', allowed: true } });
    assert.equal(widened.outcome, 'refused', JSON.stringify(widened).slice(0, 500));
    assert.equal(widened.stage, 'command');
    assert.match(widened.diagnostic, /policy cannot add or widen selected host authority/);
    const denied = await session.issue({ operation: 'invoke', id: 'caller',
      inputs: [{ kind: 'i64', value: '5' }], refs: [{ position: 1, ordinal: 0, slot: 'S1' }] });
    assert.equal(denied.outcome, 'execution-failed', JSON.stringify(denied).slice(0, 600));
    assert.deepEqual(denied.request_trace.map(row => [row.operation, row.outcome]),
      [['dispatch', 'allowed'], ['effect', 'denied']],
      'the denied effect must stay denied: only a slot-scoped grant authorizes it');
    assert.equal(denied.protected_operations, 0);
    assert.equal(await session.close(), 2,
      'the denied effect fails the guest, which terminates the session');
    return { widened: { outcome: widened.outcome, stage: widened.stage },
      denied: denied.request_trace.map(row => [row.operation, row.outcome]) };
  } catch (error) {
    await session.abort();
    throw error;
  }
};

const authority = await authorityScope();
const runtime = await runtimePolicyScope();
console.log(JSON.stringify({ authority, runtime }));
