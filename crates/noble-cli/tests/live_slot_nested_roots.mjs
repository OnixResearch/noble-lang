import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { canonical, cli, evidenceDirectory, expectOutcome, selectedAuthority } from './live_slot_resource_ref_cli.mjs';

const binary = process.argv[2];
const case02 = canonical('LSLOT-02');
assert.equal(case02.input.harness, 'parent-child-transitive-snapshot-and-new-root');
const pure = 'LiveRef<I64,I64,pure>';
const nested = `LiveRef<I64+${pure},I64,live.dispatch>`;
const transitive = `LiveRef<I64+${pure}+${nested},I64,live.dispatch>`;
const scalar = value => [{ kind: 'i64', value: String(value) }];
const bindings = slots => slots.map((slot, ordinal) => ({ position: ordinal + 1, ordinal, slot }));
const invoke = (id, slots) => ({ operation: 'invoke', id, inputs: scalar(20), refs: bindings(slots) });
const fixtures = [
  {
    name: 'base-parent-child', slots: ['B', 'A'],
    sources: { callerA: 'slot.invoke', callerLeaf: 'slot.invoke', A1: 'slot.invoke slot.invoke',
      B1: '1 +', B2: '2 +' },
    types: { callerA: ['I64', pure, nested], callerLeaf: ['I64', pure],
      A1: ['I64', pure], B1: ['I64'], B2: ['I64'] },
    contracts: [
      { slotId: 'B', input: ['I64'], output: ['I64'], effectCeiling: [], proofRequired: false },
      { slotId: 'A', input: ['I64', pure], output: ['I64'], effectCeiling: ['live.dispatch'], proofRequired: false },
    ],
    first: 'B1', replacement: 'B2', leaf: 'B', oldTrace: ['A', 'B', 'B'],
    newTrace: ['A', 'B', 'B'], holdOccurrence: 3, heldOrdinal: 0,
    oldResult: case02.expected.base_root_A_result, newResult: 24,
    canonicalNextLeaf: case02.expected.base_next_root_B_result,
  },
  {
    name: 'transitive-A-B-C', slots: ['C', 'B', 'A'],
    sources: { callerA: 'slot.invoke', callerLeaf: 'slot.invoke', A1: 'slot.invoke',
      B1: 'slot.invoke', C1: '1 +', C2: '2 +' },
    types: { callerA: ['I64', pure, nested, transitive], callerLeaf: ['I64', pure],
      A1: ['I64', pure, nested], B1: ['I64', pure], C1: ['I64'], C2: ['I64'] },
    contracts: [
      { slotId: 'C', input: ['I64'], output: ['I64'], effectCeiling: [], proofRequired: false },
      { slotId: 'B', input: ['I64', pure], output: ['I64'], effectCeiling: ['live.dispatch'], proofRequired: false },
      { slotId: 'A', input: ['I64', pure, nested], output: ['I64'], effectCeiling: ['live.dispatch'], proofRequired: false },
    ],
    first: 'C1', replacement: 'C2', leaf: 'C', oldTrace: ['A', 'B', 'C'],
    newTrace: ['A', 'B', 'C'], holdOccurrence: 2, heldOrdinal: 1,
    oldResult: case02.expected.variant_old_root_result,
    newResult: case02.expected.variant_next_root_C_result,
    canonicalNextLeaf: case02.expected.variant_next_root_C_result,
  },
];
const summary = [];
for (const fixture of fixtures) {
  const slots = fixture.slots;
  const grants = fixture.contracts.flatMap(slot => ['publish', 'dispatch'].map(operation =>
    ({ operation, slotId: slot.slotId, allowed: true })));
  const session = await cli(binary, `LSLOT02-${fixture.name}`, selectedAuthority({
    sources: fixture.sources, slots: fixture.contracts, grants, effects: ['live.dispatch'], quota: 32,
  }));
  try {
    const configured = expectOutcome(await session.configured, 'configured');
    assert.deepEqual([configured.epoch, configured.guest_requests, configured.protected_operations], ['0', 0, 0]);
    const installed = {};
    for (const [id, source] of Object.entries(fixture.sources)) {
      installed[id] = expectOutcome(await session.issue({ operation: 'install', id, source,
        inputs: fixture.types[id] }), 'installed');
      assert.equal(installed[id].guest_requests, 0);
      assert.equal(installed[id].protected_operations, 0);
    }
    expectOutcome(await session.issue({ operation: 'candidate', id: fixture.replacement }), 'candidate-staged');
    const members = fixture.contracts.map(slot => ({ slot: slot.slotId, id: `${slot.slotId}1` }));
    const invalid = await session.issue({ operation: 'bootstrap', expected_epoch: '0',
      members: members.map((member, index) =>
        index === members.length - 1 ? { ...member, id: 'callerA' } : member) });
    expectOutcome(invalid, 'refused');
    assert.match(invalid.diagnostic, /checked bootstrap candidate differs from selected slot interface/);
    const published = expectOutcome(await session.issue({ operation: 'bootstrap',
      expected_epoch: '0', members }), 'bootstrapped');
    assert.deepEqual([published.epoch, published.guest_requests, published.protected_operations],
      ['1', 0, 0]);
    assert.deepEqual(published.slots, slots.map(slot =>
      ({ slot, incarnation: '1', generation: '1' })));
    const reinit = expectOutcome(await session.issue({ operation: 'bootstrap',
      expected_epoch: '0', members }), 'bootstrap-unavailable');
    assert.equal(reinit.epoch, '1');
    const initialEpoch = published.epoch;
    const outerSite = expectOutcome(await session.issue({ operation: 'reflect', id: 'callerA', site_id: 0 }), 'reflected');
    assert.equal(outerSite.site.instruction, 'slot.invoke');
    assert.deepEqual(outerSite.site.effects, ['live.dispatch'],
      'E ∪ {live.dispatch} is a set even when the nested target E already contains dispatch');
    assert.equal(outerSite.site.input.length, fixture.contracts.at(-1).input.length);
    assert.deepEqual(outerSite.site.output, ['I64']);
    assert.ok(!JSON.stringify(outerSite.site).includes('slotId'));
    for (const id of fixture.name === 'base-parent-child' ? ['A1'] : ['A1', 'B1']) {
      const site = expectOutcome(await session.issue({ operation: 'reflect', id, site_id: 0 }), 'reflected');
      assert.deepEqual(site.site.effects, ['live.dispatch']);
      assert.ok(!JSON.stringify(site.site).includes('slotId'));
    }
    // Base pauses before the second B; transitive pauses before the nested B
    // so C is still unselected when the operator commits its replacement.
    expectOutcome(await session.issue({ operation: 'hold-checkpoint',
      import: 'live_select', occurrence: fixture.holdOccurrence }),
      'checkpoint-hold-selected');
    const oldRequest = invoke('callerA', slots);
    session.send(oldRequest);
    const entered = expectOutcome(await session.next(), 'checkpoint-entered');
    assert.equal(entered.import, 'live_select');
    assert.equal(entered.ordinal, fixture.heldOrdinal);
    const update = { operation: 'publish', slot: fixture.leaf, id: fixture.replacement,
      program_index: 0,
      expected_epoch: initialEpoch, expected_incarnation: '1', expected_generation: '1' };
    session.send(update);
    // Queue resume immediately: the checkpoint itself has an eight-second
    // deadline. The router applies the ordered publication and emits its ACK
    // before resuming the guest, independently of operator stdout latency.
    session.send({ operation: 'resume-checkpoint' });
    const ack = expectOutcome(await session.next(), 'published');
    assert.equal(ack.epoch, '2');
    assert.equal(ack.root, entered.root);
    assert.equal(ack.checkpoint, entered.checkpoint);
    assert.equal(ack.control_id, '1');
    assert.deepEqual([ack.guest_requests, ack.protected_operations], [undefined, undefined]);
    const old = expectOutcome(await session.next(), 'executed');
    assert.equal(old.epoch, initialEpoch);
    assert.deepEqual(old.stack, [{ kind: 1, value: String(fixture.oldResult) }]);
    assert.deepEqual([old.guest_requests, old.protected_operations], [3, 0]);
    assert.deepEqual(old.request_trace.map(item => item.slotId), fixture.oldTrace);
    assert.deepEqual(old.request_trace.map(item =>
      [item.slotId, item.artifactSha256, item.generation]),
    fixture.oldTrace.map(slot => [slot, installed[`${slot}1`].artifact_sha256, '1']));
    assert.deepEqual(old.control_events.map(item => [item.control_id, item.operation,
      item.scope, item.epoch, item.outcome]),
    [['1', 'publish', fixture.leaf, ack.epoch, 'published']]);
    const oldLeaf = old.request_trace.filter(item => item.slotId === fixture.leaf);
    assert.ok(oldLeaf.length >= 1);
    assert.ok(oldLeaf.every(item => item.generation === '1' &&
      item.artifactSha256 === installed[fixture.first].artifact_sha256));
    if (fixture.name === 'base-parent-child') {
      assert.equal(oldLeaf.length, 2);
      assert.equal(oldLeaf[0].programValueId, oldLeaf[1].programValueId);
      assert.equal(oldLeaf[0].rootEpoch, oldLeaf[1].rootEpoch);
    }
    const newOuter = expectOutcome(await session.issue(oldRequest), 'executed');
    assert.equal(newOuter.epoch, ack.epoch);
    assert.deepEqual(newOuter.stack, [{ kind: 1, value: String(fixture.newResult) }]);
    assert.deepEqual([newOuter.guest_requests, newOuter.protected_operations], [3, 0]);
    assert.deepEqual(newOuter.request_trace.map(item => item.slotId), fixture.newTrace);
    assert.deepEqual(newOuter.request_trace.map(item =>
      [item.slotId, item.artifactSha256, item.generation]),
    fixture.newTrace.map(slot => [slot, installed[slot === fixture.leaf
      ? fixture.replacement : `${slot}1`].artifact_sha256,
    slot === fixture.leaf ? '2' : '1']));
    for (const [trace, epoch] of [[old.request_trace, initialEpoch],
      [newOuter.request_trace, ack.epoch]]) {
      assert.ok(trace.every(item => item.rootEpoch === epoch && item.policyAllowed === true
        && item.policyInput.operation === 'dispatch'
        && item.policyInput.slotId === item.slotId
        && item.policyInput.rootEpoch === epoch
        && item.policyInput.version.artifactSha256 === item.artifactSha256),
      'each actual dispatch must recheck current authority for its selected slot/version');
    }
    const newLeaf = newOuter.request_trace.filter(item => item.slotId === fixture.leaf);
    assert.ok(newLeaf.every(item => item.generation === '2' &&
      item.artifactSha256 === installed[fixture.replacement].artifact_sha256));
    assert.notEqual(oldLeaf[0].programValueId, newLeaf[0].programValueId);
    const leaf = expectOutcome(await session.issue(invoke('callerLeaf', [fixture.leaf])), 'executed');
    assert.equal(leaf.epoch, ack.epoch);
    assert.deepEqual(leaf.stack, [{ kind: 1, value: String(fixture.canonicalNextLeaf) }]);
    assert.deepEqual([leaf.guest_requests, leaf.protected_operations], [1, 0]);
    assert.deepEqual(leaf.request_trace.map(item => item.slotId), [fixture.leaf]);
    assert.equal(leaf.request_trace[0].programValueId, newLeaf[0].programValueId);
    assert.equal(leaf.request_trace[0].policyAllowed, true);
    const afterSite = expectOutcome(await session.issue({ operation: 'reflect', id: 'callerA', site_id: 0 }), 'reflected');
    assert.deepEqual(afterSite.site, outerSite.site);
    assert.equal(await session.close(), 0);
    summary.push({ name: fixture.name, status: 'observed',
      initial_epoch: initialEpoch, committed_epoch: ack.epoch,
      bootstrap: published, invalid_refusal: invalid, reinit_refusal: reinit,
      old_root: { epoch: old.epoch, stack: old.stack, guest_requests: old.guest_requests,
        protected_operations: old.protected_operations, request_trace: old.request_trace,
        control_events: old.control_events },
      new_root: { epoch: newOuter.epoch, stack: newOuter.stack,
        guest_requests: newOuter.guest_requests, protected_operations: newOuter.protected_operations,
        request_trace: newOuter.request_trace },
      next_leaf: { epoch: leaf.epoch, stack: leaf.stack,
        guest_requests: leaf.guest_requests, protected_operations: leaf.protected_operations,
        request_trace: leaf.request_trace },
      checkpoint: entered, committed_ack: ack, generic_site: outerSite.site,
      installed_sha256: Object.fromEntries(Object.entries(installed).map(([id, report]) =>
        [id, report.artifact_sha256])), selected_binary_sha256: session.selectedBinarySha256,
      raw: session.log, requests: session.requests, responses: session.responses,
      stderr: session.stderrFile });
  } catch (error) {
    await session.abort();
    summary.push({ name: fixture.name, status: 'failed',
      error: String(error.stack ?? error), raw: session.log,
      requests: session.requests, responses: session.responses,
      stderr: session.stderrFile, selected_binary_sha256: session.selectedBinarySha256 });
  }
}
// Distinct checked B1/A1 pass Rust bootstrap preflight. The worker must
// reject the assembled two-member map under quota 1, not publish B first.
{
  const fixture = fixtures[0], name = 'later-member-budget-refusal';
  const sources = { B1: fixture.sources.B1, A1: fixture.sources.A1 };
  const grants = fixture.slots.flatMap(slot => ['publish', 'dispatch', 'delete']
    .map(operation => ({ operation, slotId: slot, allowed: true })));
  const session = await cli(binary, `LSLOT02-${name}`, selectedAuthority({
    sources, slots: fixture.contracts, grants, effects: ['live.dispatch'], quota: 1,
  }));
  try {
    const configured = expectOutcome(await session.configured, 'configured');
    assert.deepEqual([configured.epoch, configured.guest_requests, configured.protected_operations],
      ['0', 0, 0]);
    for (const [id, source] of Object.entries(sources)) {
      const installed = expectOutcome(await session.issue({ operation: 'install', id, source,
        inputs: fixture.types[id] }), 'installed');
      assert.deepEqual([installed.guest_requests, installed.protected_operations], [0, 0]);
    }
    const members = fixture.slots.map(slot => ({ slot, id: `${slot}1` }));
    const attempt = { operation: 'bootstrap', expected_epoch: '0', members };
    const refused = expectOutcome(await session.issue(attempt), 'retention-budget-refused');
    assert.deepEqual([refused.epoch, refused.guest_requests, refused.protected_operations],
      ['0', 0, 0]);
    const absent = [];
    for (const slot of fixture.slots) {
      const probe = expectOutcome(await session.issue({ operation: 'delete', slot,
        expected_epoch: '0', expected_incarnation: '1', expected_generation: '1' }), 'stale-reject');
      assert.deepEqual([probe.epoch, probe.guest_requests, probe.protected_operations], ['0', 0, 0]);
      absent.push({ slot, ...probe });
    }
    const trace = expectOutcome(await session.issue({ operation: 'trace' }), 'trace-observed');
    assert.deepEqual(trace.trace, []);
    const repeat = expectOutcome(await session.issue(attempt), 'retention-budget-refused');
    assert.deepEqual([repeat.epoch, repeat.guest_requests, repeat.protected_operations], ['0', 0, 0]);
    assert.equal(await session.close(), 0);
    summary.push({ name, status: 'observed', refusal: refused, map_absence: absent,
      repeat_refusal: repeat, trace, selected_binary_sha256: session.selectedBinarySha256,
      raw: session.log, requests: session.requests, responses: session.responses,
      stderr: session.stderrFile });
  } catch (error) {
    await session.abort();
    summary.push({ name, status: 'failed', error: String(error.stack ?? error),
      selected_binary_sha256: session.selectedBinarySha256,
      raw: session.log, requests: session.requests, responses: session.responses,
      stderr: session.stderrFile });
  }
}
const summaryFile = path.join(evidenceDirectory, 'LSLOT02-summary.json');
fs.writeFileSync(summaryFile, JSON.stringify({ selected_binary: binary,
  canonical: 'specs/conformance/live-reference-cases.json#LSLOT-02', results: summary }, null, 2));
console.log(JSON.stringify({ summary: summaryFile, results: summary }));
if (summary.some(item => item.status === 'failed')) process.exitCode = 1;
