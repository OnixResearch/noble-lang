import assert from 'node:assert/strict';
import { cli, expectOutcome, selectedAuthority } from './live_slot_resource_ref_cli.mjs';

// LREF-CASE-06 (`.cairn/archive/2026-11-23-live-reference-lifetimes`): held roots and allowed
// mixing. S-v0 is the quotation `[ 100 + ]` of module s0; invoking s0 saves that exact Program as
// P, so P is of
// S-v0's lineage. S-v1 (`1 +`) replaces v0. Root R1 receives P, dispatches S, and is held at its
// second selection while S-v2 (`2 +`) is published; R1 then resumes, dispatches S again and runs
// P. R1 must select S-v1 for both dispatches with its trace at R1's epoch throughout, and P must
// run v0's code; root R2 then selects S-v2 for both of its dispatches, and P is still v0's.
// No root selects two versions of S. The serial live host accepts only operator controls during
// a held checkpoint, so R2 runs after R1 resumes, not while R1 is held.
const binary = process.argv[2];
assert.ok(binary, 'pass the selected noble CLI binary');
const sources = { s0: '[ 100 + ]', s1: '1 +', s2: '2 +', r: 'swap slot.invoke slot.invoke swap run' };
const slots = [{ slotId: 'S', input: ['I64'], output: ['I64'], effectCeiling: [], proofRequired: false }];
const grants = ['publish', 'dispatch'].map(operation => ({ operation, slotId: 'S', allowed: true }));
const session = await cli(binary, 'LREF-CASE-06', selectedAuthority({ sources, slots, grants, quota: 8 }));
const issue = async (request, outcome) => expectOutcome(await session.issue(request), outcome);
const versions = trace => trace.map(row => [row.slotId, row.generation, row.rootEpoch, row.artifactSha256]);
let complete = false;
try {
  expectOutcome(await session.configured, 'configured');
  const installed = {};
  for (const id of ['s0', 's1', 's2']) {
    installed[id] = await issue({ operation: 'install', id, source: sources[id],
      inputs: id === 's0' ? [] : ['I64'] }, 'installed');
  }
  await issue({ operation: 'publish', slot: 'S', id: 's0', program_index: 1, expected_epoch: '0' },
    'published');
  const p = (await issue({ operation: 'invoke', id: 's0', inputs: [], refs: [] }, 'executed')).stack[0];
  assert.deepEqual([p.kind, p.source_id, p.program_index], [4, 's0', 1], 'P is S-v0\'s exact Program');
  const v1 = await issue({ operation: 'publish', slot: 'S', id: 's1', program_index: 0,
    expected_epoch: '1', expected_incarnation: '1', expected_generation: '1' }, 'published');
  assert.deepEqual([v1.epoch, v1.generation], ['2', '2']);
  await issue({ operation: 'install', id: 'r', source: sources.r,
    inputs: ['I64', 'LiveRef<I64,I64,pure>', 'Program<I64,I64,pure>'] }, 'installed');
  await issue({ operation: 'candidate', id: 's2' }, 'candidate-staged');
  const root = { operation: 'invoke', id: 'r', inputs: [{ kind: 'i64', value: '20' },
    { kind: 'program', owner: p.owner }], refs: [{ position: 1, ordinal: 0, slot: 'S' }] };

  await issue({ operation: 'hold-checkpoint', import: 'live_select', occurrence: 2 },
    'checkpoint-hold-selected');
  session.send(root);
  const entered = expectOutcome(await session.next(), 'checkpoint-entered');
  assert.equal(entered.import, 'live_select');
  session.send({ operation: 'publish', slot: 'S', id: 's2', program_index: 0, expected_epoch: '2',
    expected_incarnation: '1', expected_generation: '2' });
  session.send({ operation: 'resume-checkpoint' });
  const ack = expectOutcome(await session.next(), 'published');
  assert.deepEqual([ack.epoch, ack.root, ack.checkpoint], ['3', entered.root, entered.checkpoint],
    'S-v2 commits at epoch 3 while R1 is held');
  const r1 = expectOutcome(await session.next(), 'executed');
  assert.equal(r1.epoch, '2');
  assert.deepEqual(versions(r1.request_trace), [['S', '2', '2', installed.s1.artifact_sha256],
    ['S', '2', '2', installed.s1.artifact_sha256]], 'R1 selects S-v1 for both dispatches at epoch 2');
  assert.deepEqual(r1.stack, [{ kind: 1, value: '122' }], 'P runs v0\'s code inside R1 (20 + 1 + 1 + 100)');

  const r2 = await issue(root, 'executed');
  assert.equal(r2.epoch, '3');
  assert.deepEqual(versions(r2.request_trace), [['S', '3', '3', installed.s2.artifact_sha256],
    ['S', '3', '3', installed.s2.artifact_sha256]], 'R2 selects S-v2 for both dispatches at epoch 3');
  assert.deepEqual(r2.stack, [{ kind: 1, value: '124' }], 'P still runs v0\'s code (20 + 2 + 2 + 100)');

  assert.equal(await session.close(), 0);
  complete = true;
  console.log(JSON.stringify({ case: 'LREF-CASE-06', p: { source_id: p.source_id, program_index: p.program_index },
    r1: { epoch: r1.epoch, stack: r1.stack, trace: versions(r1.request_trace) },
    publication_while_held: { epoch: ack.epoch, root: ack.root },
    r2: { epoch: r2.epoch, stack: r2.stack, trace: versions(r2.request_trace) },
    selected_binary_sha256: session.selectedBinarySha256 }));
} finally { if (!complete) await session.abort(); }
