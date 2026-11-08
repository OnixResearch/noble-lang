import assert from 'node:assert/strict';
import { cli, expectOutcome, selectedAuthority } from './live_slot_resource_ref_cli.mjs';

// OWN-CASE-06 (`.cairn/changes/ownership-lifetime-contract`): module M defines `h` and `g`, and its
// root returns the saved Program `[ g ]`, whose compiled body reads `g` through M's module global.
// M's published root is superseded, which sweeps M; the saved owner is then invoked. It must
// return the checked result, and M's code may retire only after that owner is released. Before the
// fix, the sweep released M's module roots while the owner lived, and the invocation trapped.
const binary = process.argv[2];
const sources = {
  hdef: 'def h [ 1 + ]', gdef: 'def g [ h ]', mkdef: 'def mk [ [ g ] ]', mk: 'mk',
  v2def: 'def mk2 [ [ 5 + ] ]', v2: 'mk2', runner: 'run',
};
const authority = selectedAuthority({ sources, quota: 24, effects: [],
  slots: [{ slotId: 'Q', input: [], output: ['Program<I64,I64,pure>'], effectCeiling: [],
    proofRequired: false }],
  grants: [{ operation: 'publish', slotId: 'Q', allowed: true }] });
const session = await cli(binary, 'OWN-CASE-06-module-root-retention', authority);
const issue = async (request, outcome) => expectOutcome(await session.issue(request), outcome);
const observe = (label, row) => console.log(JSON.stringify({ observation: label,
  outcome: row.outcome, stack: row.stack ?? null, diagnostic: row.diagnostic ?? null,
  retire_code_spans: row.retire_code_spans ?? null }));
let complete = false;
try {
  expectOutcome(await session.configured, 'configured');
  for (const [id, name] of [['hdef', 'h'], ['gdef', 'g'], ['mkdef', 'mk']]) {
    await issue({ operation: 'define', id, name, source: sources[id], inputs: [] },
      'definition-retained');
  }
  const module = await issue({ operation: 'install', id: 'mk', source: sources.mk,
    selected_name: 'mk', inputs: [] }, 'installed');
  const made = await issue({ operation: 'invoke', id: 'mk', inputs: [], refs: [] }, 'executed');
  assert.equal(made.stack.length, 1);
  const [saved] = made.stack;
  assert.equal(saved.kind, 4);
  assert.equal(saved.source_id, 'mk', 'the saved Program is a static Program of M');
  await issue({ operation: 'install', id: 'runner', source: sources.runner,
    inputs: ['I64', 'Program<I64,I64,pure>'] }, 'installed');
  const run = () => session.issue({ operation: 'invoke', id: 'runner',
    inputs: [{ kind: 'i64', value: '41' }, { kind: 'program', owner: saved.owner }], refs: [] });
  const before = expectOutcome(await run(), 'executed');
  assert.deepEqual(before.stack, [{ kind: 1, value: '42' }]);
  await issue({ operation: 'publish', slot: 'Q', id: 'mk', expected_epoch: '0' }, 'published');
  await issue({ operation: 'define', id: 'v2def', name: 'mk2', source: sources.v2def, inputs: [] },
    'definition-retained');
  await issue({ operation: 'install', id: 'v2', source: sources.v2, selected_name: 'mk2',
    inputs: [] }, 'installed');
  const superseded = await issue({ operation: 'publish', slot: 'Q', id: 'v2', expected_epoch: '1',
    expected_incarnation: '1', expected_generation: '1' }, 'published');
  const ofModule = spans => spans.filter(span => span.generation === Number(module.generation));
  assert.deepEqual(ofModule(superseded.retire_code_spans), [],
    'M code must not become retirable while the saved owner reaches it');
  const after = await run();
  observe('after-supersession', after);
  expectOutcome(after, 'executed');
  assert.deepEqual(after.stack, [{ kind: 1, value: '42' }]);
  const released = await issue({ operation: 'release-program', owner: saved.owner },
    'program-released');
  const [span] = ofModule(released.retire_code_spans);
  assert.ok(span, 'releasing the last saved owner must let M retire');
  observe('after-release', released);
  expectOutcome(await run(), 'refused');
  assert.equal(await session.close(), 0);
  complete = true;
  console.log(JSON.stringify({ case: 'OWN-CASE-06', selected_binary_sha256: session.selectedBinarySha256,
    before: before.stack, after_supersession: after.stack, retired_span: span }));
} finally { if (!complete) await session.abort(); }
