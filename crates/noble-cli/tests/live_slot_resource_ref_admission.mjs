import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { canonical, cli, evidenceDirectory, expectOutcome, resourceGrants,
  resourceRows, selectedAuthority } from './live_slot_resource_ref_cli.mjs';

const binary = process.argv[2];
const chosen = canonical('LSLOT-03');
assert.deepEqual(chosen.input.slot_contract, {
  input: ['Account@1', 'I64'], output: ['Account@1', 'I64'], effect_ceiling: ['test.emit'],
});
assert.equal(chosen.input.current_epoch, 4);
const variants = chosen.input.variants;
assert.deepEqual(variants.map(row => row.name), [
  'same-exact-interface', 'reversed-input', 'same-layout-different-resource-schema',
  'unknown-host-contract',
], 'decision-19 amendment: the unselectable fs.write and impossible under-declare variants are'
  + ' dropped; the input-only variant is replaced by the both-sides Account@2 candidate');
const base = { source: '"admit" test.emit drop', inputs: chosen.input.slot_contract.input };
// A constructible generic caller for the slot: `slot.invoke` with the slot's
// ordered interface as its ref input. Its compiled site is the dispatch-effect
// witness; it is a control, not a case variant.
const genericCaller = { source: 'slot.invoke',
  inputs: ['Account@1', 'I64', 'LiveRef<Account@1+I64,Account@1+I64,test.emit>'] };
const candidates = {
  'same-exact-interface': base,
  'reversed-input': { source: 'swap "admit" test.emit drop', inputs: ['I64', 'Account@1'] },
  'same-layout-different-resource-schema': { source: base.source, inputs: ['Account@2', 'I64'] },
  // Neither fs.write nor unknown.effect is selectable by the pinned live-slot
  // CLI host: their actual source attempts must be reported as profile-limited.
  'effect-ceiling-widened': { source: '"admit" test.emit drop "path" fs.write', inputs: base.inputs },
  'unknown-host-contract': { source: '"admit" test.emit drop unknown.effect', inputs: base.inputs },
};
const sources = { seed: '1 +', exact: base.source,
  ...Object.fromEntries(variants.map(row => [row.name, candidates[row.name].source])) };
const slots = [
  ...Array.from({ length: 4 }, (_, i) => ({ slotId: `seed${i}`, input: ['I64'],
    output: ['I64'], effectCeiling: [], proofRequired: false })),
  { slotId: 'account', input: chosen.input.slot_contract.input,
    output: chosen.input.slot_contract.output,
    effectCeiling: chosen.input.slot_contract.effect_ceiling, proofRequired: false },
];
const grants = [
  ...resourceGrants,
  ...slots.map(slot => ({ operation: 'publish', slotId: slot.slotId, allowed: true })),
  { operation: 'effect', slotId: 'account', allowed: true },
  { operation: 'dispatch', slotId: 'account', allowed: true },
];
const results = [];
for (const variant of variants) {
  const name = variant.name;
  const session = await cli(binary, `LSLOT03-${name}`, selectedAuthority({
    sources, resources: resourceRows, slots, grants, effects: ['test.emit', 'live.dispatch'],
  }));
  try {
    expectOutcome(await session.configured, 'configured');
    const seed = await session.issue({ operation: 'install', id: 'seed', source: sources.seed,
      inputs: ['I64'] });
    expectOutcome(seed, 'installed');
    for (let i = 0; i < 4; ++i) {
      const published = await session.issue({ operation: 'publish', slot: `seed${i}`, id: 'seed',
        expected_epoch: String(i) });
      expectOutcome(published, 'published');
      assert.equal(published.epoch, String(i + 1), JSON.stringify(published));
    }
    const before = expectOutcome(await session.issue({ operation: 'trace' }), 'trace-observed');
    assert.deepEqual(before.trace, [], JSON.stringify(before));
    const candidate = candidates[name];
    const installed = await session.issue({ operation: 'install', id: name,
      source: candidate.source, inputs: candidate.inputs });
    assert.equal(installed.guest_requests, 0, JSON.stringify(installed));
    assert.equal(installed.protected_operations, 0, JSON.stringify(installed));
    let publication = null;
    if (installed.outcome === 'installed') {
      publication = await session.issue({ operation: 'publish', slot: 'account', id: name,
        expected_epoch: '4' });
      assert.equal(publication.guest_requests, 0, JSON.stringify(publication));
      assert.equal(publication.protected_operations, 0, JSON.stringify(publication));
    }
    if (name === 'same-exact-interface') {
      expectOutcome(installed, 'installed');
      expectOutcome(publication, 'published');
      assert.equal(publication.epoch, '5');
    } else {
      assert.notEqual(publication?.outcome, 'published', JSON.stringify(publication));
      const after = expectOutcome(await session.issue({ operation: 'trace' }), 'trace-observed');
      assert.deepEqual(after.trace, before.trace, JSON.stringify(after));
      const positive = expectOutcome(await session.issue({ operation: 'install', id: 'exact',
        source: sources.exact, inputs: base.inputs }), 'installed');
      const control = expectOutcome(await session.issue({ operation: 'publish', slot: 'account',
        id: 'exact', expected_epoch: '4' }), 'published');
      assert.equal(control.epoch, '5');
      assert.ok(positive.handle > 0);
    }
    const status = name === 'same-exact-interface' ||
      ['reversed-input', 'same-layout-different-resource-schema'].includes(name)
        && installed.outcome === 'installed' && publication?.outcome === 'refused'
      ? 'pass' : 'partial';
    results.push({ name, status, installed: installed.outcome,
      publication: publication?.outcome ?? 'not-reached', epoch_before: '4',
      epoch_after: name === 'same-exact-interface' ? '5 (candidate)' : '5 (independent exact control)',
      limitation: name === 'effect-ceiling-widened' || name === 'unknown-host-contract'
          ? 'fs.write/unknown.effect cannot be registered in the pinned live-slot CLI profile; source rejected before publication'
          : null,
      guest_requests: publication?.guest_requests ?? installed.guest_requests,
      protected_operations: publication?.protected_operations ?? installed.protected_operations,
      candidate_body_executions: 'no invoke issued; guest trace []',
      selected_binary_sha256: session.selectedBinarySha256, raw: session.log });
    assert.equal(await session.close(), 0);
  } catch (error) {
    await session.abort();
    throw Error(`${name}: ${error} raw=${session.log}`, { cause: error });
  }
}
// A non-canonical generic-caller control: the caller's own compiled site is the
// `generic_dispatch_effect` witness. The caller declares the slot's ordered
// interface as its ref input, so no interface refusal can stand in for the
// dispatch-effect check.
let callerReflection;
{
  const session = await cli(binary, 'LSLOT03-caller-reflection', selectedAuthority({
    sources: { ...sources, caller: genericCaller.source },
    resources: resourceRows, slots, grants, effects: ['test.emit', 'live.dispatch'],
  }));
  try {
    expectOutcome(await session.configured, 'configured');
    const installed = expectOutcome(await session.issue({ operation: 'install', id: 'caller',
      source: genericCaller.source, inputs: genericCaller.inputs }), 'installed');
    assert.deepEqual([installed.guest_requests, installed.protected_operations], [0, 0]);
    const site = expectOutcome(await session.issue({ operation: 'reflect', id: 'caller',
      site_id: 0 }), 'reflected');
    // Reflect reads the genuinely compiled site; its required live.dispatch
    // cannot be silently omitted by a caller asserting only test.emit.
    assert.equal(site.site.instruction, 'slot.invoke');
    assert.ok(site.site.effects.includes('live.dispatch'), JSON.stringify(site));
    assert.deepEqual([site.guest_requests, site.protected_operations], [0, 0]);
    assert.equal(await session.close(), 0);
    callerReflection = { install: installed.outcome, install_inputs: genericCaller.inputs,
      install_source: genericCaller.source, site: site.site,
      selected_binary_sha256: session.selectedBinarySha256, raw: session.log };
  } catch (error) {
    await session.abort();
    throw Error(`caller-reflection: ${error} raw=${session.log}`, { cause: error });
  }
}
// A non-canonical widening control: the decoy slot selects the checked test
// host, but only the independently selected account slot is published.
const ceilingControl = [];
for (const [name, ceiling] of [
  ['empty-ceiling', []], ['matching-ceiling', ['test.emit']],
]) {
  const authority = selectedAuthority({
    sources: { candidate: base.source, decoy: '"decoy" test.emit drop',
      pure: 'swap swap' },
    resources: resourceRows,
    slots: [
      { slotId: 'account', input: base.inputs, output: chosen.input.slot_contract.output,
        effectCeiling: ceiling, proofRequired: false },
      { slotId: 'decoy', input: base.inputs, output: chosen.input.slot_contract.output,
        effectCeiling: ['test.emit'], proofRequired: false },
    ],
    grants: [...resourceGrants,
      { operation: 'effect', slotId: 'decoy', allowed: true },
      { operation: 'publish', slotId: 'account', allowed: true }],
    effects: ['test.emit', 'live.dispatch'],
  });
  const session = await cli(binary, `LSLOT03-ceiling-control-${name}`, authority);
  try {
    expectOutcome(await session.configured, 'configured');
    const before = expectOutcome(await session.issue({ operation: 'trace' }), 'trace-observed');
    assert.deepEqual(before.trace, []);
    const installed = expectOutcome(await session.issue({ operation: 'install',
      id: 'candidate', source: base.source, inputs: base.inputs }), 'installed');
    assert.match(installed.artifact_sha256, /^[0-9a-f]{64}$/);
    assert.deepEqual([installed.guest_requests, installed.protected_operations], [0, 0]);
    const publication = await session.issue({ operation: 'publish', slot: 'account',
      id: 'candidate', expected_epoch: '0' });
    expectOutcome(publication, name === 'empty-ceiling' ? 'refused' : 'published');
    assert.deepEqual([publication.guest_requests, publication.protected_operations], [0, 0]);
    if (name === 'empty-ceiling') {
      const pure = expectOutcome(await session.issue({ operation: 'install', id: 'pure',
        source: 'swap swap', inputs: base.inputs }), 'installed');
      assert.deepEqual([pure.guest_requests, pure.protected_operations], [0, 0]);
      const control = expectOutcome(await session.issue({ operation: 'publish', slot: 'account',
        id: 'pure', expected_epoch: '0' }), 'published');
      assert.equal(control.epoch, '1');
      assert.deepEqual([control.guest_requests, control.protected_operations], [0, 0]);
    } else {
      assert.equal(publication.epoch, '1');
    }
    const after = expectOutcome(await session.issue({ operation: 'trace' }), 'trace-observed');
    assert.deepEqual(after.trace, before.trace);
    assert.equal(await session.close(), 0);
    ceilingControl.push({ name, account_ceiling: ceiling, installed: installed.outcome,
      candidate_artifact_sha256: installed.artifact_sha256, publication: publication.outcome,
      refusal: name === 'empty-ceiling' ? publication.diagnostic : null,
      epoch_after: '1', guest_requests: publication.guest_requests,
      protected_operations: publication.protected_operations,
      trace: after.trace, selected_binary_sha256: session.selectedBinarySha256,
      raw: session.log });
  } catch (error) {
    await session.abort();
    throw Error(`ceiling-control-${name}: ${error} raw=${session.log}`, { cause: error });
  }
}
const summary = path.join(evidenceDirectory, 'LSLOT03-summary.json');
fs.writeFileSync(summary, JSON.stringify({ selected_binary: binary,
  canonical: 'specs/conformance/live-reference-cases.json#LSLOT-03',
  results, caller_reflection: callerReflection, ceiling_control: ceilingControl }, null, 2));
console.log(JSON.stringify({ summary, results, caller_reflection: callerReflection,
  ceiling_control: ceilingControl }));
