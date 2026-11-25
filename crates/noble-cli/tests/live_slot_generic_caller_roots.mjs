import assert from 'node:assert/strict';
import { cli, expectOutcome, selectedAuthority } from './live_slot_resource_ref_cli.mjs';

// LREF-CASE-05 (`.cairn/archive/2026-11-23-live-reference-lifetimes`): a saved generic caller across
// roots. G is the installed generic caller `slot.invoke` over `[I64, LiveRef<I64,I64,pure>]`, installed
// once and reused. Root R1 runs G with a fresh ref to S at epoch 1 and selects S-v1; S-v2 is
// published; root R2 runs the same G with a fresh ref and selects S-v2. G's identity is unchanged:
// the same installed artifact and handle serve both roots, and its reflected dispatch site names
// no slot before or after the publication. The design's R2 variant that retains R1's ref has no
// request form: a ref exists only as the `refs` binding of the invocation that issued it, so R2
// can present only its own fresh binding (the LSLOT-06 `retain` precedent, decision 25).
const binary = process.argv[2];
assert.ok(binary, 'pass the selected noble CLI binary');
const sources = { s1: '1 +', s2: '2 +', g: 'slot.invoke' };
const slots = [{ slotId: 'S', input: ['I64'], output: ['I64'], effectCeiling: [], proofRequired: false }];
const grants = ['publish', 'dispatch'].map(operation => ({ operation, slotId: 'S', allowed: true }));
const session = await cli(binary, 'LREF-CASE-05', selectedAuthority({ sources, slots, grants, quota: 8 }));
const issue = async (request, outcome) => expectOutcome(await session.issue(request), outcome);
const root = { operation: 'invoke', id: 'g', inputs: [{ kind: 'i64', value: '20' }],
  refs: [{ position: 1, ordinal: 0, slot: 'S' }] };
let complete = false;
try {
  expectOutcome(await session.configured, 'configured');
  const installed = {};
  for (const id of ['s1', 's2']) installed[id] = await issue({ operation: 'install', id,
    source: sources[id], inputs: ['I64'] }, 'installed');
  await issue({ operation: 'publish', slot: 'S', id: 's1', program_index: 0, expected_epoch: '0' },
    'published');
  const g = await issue({ operation: 'install', id: 'g', source: sources.g,
    inputs: ['I64', 'LiveRef<I64,I64,pure>'] }, 'installed');
  const siteBefore = (await issue({ operation: 'reflect', id: 'g', site_id: 0 }, 'reflected')).site;
  assert.ok(!JSON.stringify(siteBefore).includes('slotId'), 'G\'s site names no slot');

  const r1 = await issue(root, 'executed');
  assert.deepEqual([r1.epoch, r1.request_trace[0].slotId, r1.request_trace[0].artifactSha256],
    ['1', 'S', installed.s1.artifact_sha256], 'R1 selects S-v1 at epoch 1');
  assert.deepEqual(r1.stack, [{ kind: 1, value: '21' }]);
  const published = await issue({ operation: 'publish', slot: 'S', id: 's2', program_index: 0,
    expected_epoch: '1', expected_incarnation: '1', expected_generation: '1' }, 'published');
  assert.equal(published.epoch, '2');
  const r2 = await issue(root, 'executed');
  assert.deepEqual([r2.epoch, r2.request_trace[0].slotId, r2.request_trace[0].artifactSha256],
    ['2', 'S', installed.s2.artifact_sha256], 'R2 selects S-v2 at epoch 2');
  assert.deepEqual(r2.stack, [{ kind: 1, value: '22' }]);

  const siteAfter = (await issue({ operation: 'reflect', id: 'g', site_id: 0 }, 'reflected')).site;
  assert.deepEqual(siteAfter, siteBefore, 'G\'s reflected site is unchanged by the publication');
  assert.equal(r1.request_trace[0].policyInput.operation, 'dispatch');
  assert.equal(await session.close(), 0);
  complete = true;
  console.log(JSON.stringify({ case: 'LREF-CASE-05', g: { artifact_sha256: g.artifact_sha256, handle: g.handle,
    site: siteBefore }, r1: { epoch: r1.epoch, stack: r1.stack, selected: r1.request_trace[0].artifactSha256 },
  r2: { epoch: r2.epoch, stack: r2.stack, selected: r2.request_trace[0].artifactSha256 },
  retained_ref_variant: 'no request form: a ref exists only as the refs binding of its own invocation',
  selected_binary_sha256: session.selectedBinarySha256 }));
} finally { if (!complete) await session.abort(); }
