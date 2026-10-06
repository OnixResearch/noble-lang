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
  'effect-ceiling-widened', 'unknown-host-contract', 'generic-caller-underdeclares-dispatch',
]);
const base = { source: '"admit" test.emit drop', inputs: chosen.input.slot_contract.input };
const candidates = {
  'same-exact-interface': base,
  'reversed-input': { source: 'swap "admit" test.emit drop', inputs: ['I64', 'Account@1'] },
  'same-layout-different-resource-schema': { source: base.source, inputs: ['Account@2', 'I64'] },
  // Neither fs.write nor unknown.effect is selectable by the pinned live-slot
  // CLI host: their actual source attempts must be reported as profile-limited.
  'effect-ceiling-widened': { source: '"admit" test.emit drop "path" fs.write', inputs: base.inputs },
  'unknown-host-contract': { source: '"admit" test.emit drop unknown.effect', inputs: base.inputs },
  'generic-caller-underdeclares-dispatch': {
    source: 'slot.invoke', inputs: ['Account@1', 'I64',
      'LiveRef<Account@1+I64,Account@1+I64,test.emit>'],
  },
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
    let site = null;
    if (installed.outcome === 'installed') {
      if (name === 'generic-caller-underdeclares-dispatch') {
        site = await session.issue({ operation: 'reflect', id: name, site_id: 0 });
        // Reflect reads the genuinely compiled site; its required live.dispatch
        // cannot be silently omitted by a caller asserting only test.emit.
        expectOutcome(site, 'reflected');
        assert.ok(site.site.effects.includes('live.dispatch'), JSON.stringify(site));
      }
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
      name === 'reversed-input'
        && installed.outcome === 'installed' && publication?.outcome === 'refused'
      ? 'pass' : 'partial';
    results.push({ name, status, installed: installed.outcome,
      publication: publication?.outcome ?? 'not-reached', epoch_before: '4',
      epoch_after: name === 'same-exact-interface' ? '5 (candidate)' : '5 (independent exact control)',
      limitation: name === 'same-layout-different-resource-schema'
        ? 'Account@2 cannot be converted to the canonical Account@1 output by checked source; input AND output differ'
        : name === 'effect-ceiling-widened' || name === 'unknown-host-contract'
          ? 'fs.write/unknown.effect cannot be registered in the pinned live-slot CLI profile; source rejected before publication'
          : name === 'generic-caller-underdeclares-dispatch'
            ? 'compiled caller reflects live.dispatch, but source has no caller-claimed-effect override and adds a LiveRef input'
            : null,
      site: site?.site ?? null,
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
      assert.match(publication.diagnostic, /checked candidate differs from selected slot interface or effect ceiling/);
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
  results, ceiling_control: ceilingControl }, null, 2));
console.log(JSON.stringify({ summary, results, ceiling_control: ceilingControl }));
