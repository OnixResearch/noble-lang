import assert from 'node:assert/strict';
import { cli, expectOutcome, selectedAuthority } from './live_slot_resource_ref_cli.mjs';

// LREF-CASE-04 (`.cairn/archive/2026-11-23-live-reference-lifetimes`): an escaped result never follows
// publication. Slot S's v1 returns `1 quote [ + ] compose`, a Program that captured 1; a root R
// dispatches to v1 through `slot.invoke` and saves the returned Program O. v2, which returns the
// Program that captured 2, then replaces v1. O on 20 must still give 21 after the publication,
// while a new root's fresh result gives 22. O's lineage is its own: the reply that saved it names
// no slot, epoch or reference, and its recorded cell graph is v1's recipe: Program cells, the
// captured I64 1 and the recipe nodes that record it and the static `[ + ]` (backend kinds 4, 1
// and 8). No LiveRef can appear there: a LiveRef is a root binding, never a cell.
const binary = process.argv[2];
assert.ok(binary, 'pass the selected noble CLI binary');
const sources = {
  v1def: 'def mk1 [ 1 quote [ + ] compose ]', v1: 'mk1',
  v2def: 'def mk2 [ 2 quote [ + ] compose ]', v2: 'mk2',
  caller: 'slot.invoke', runner: 'run',
};
const program = 'Program<I64,I64,pure>';
const slots = [{ slotId: 'S', input: [], output: [program], effectCeiling: [], proofRequired: false }];
const grants = ['publish', 'dispatch'].map(operation => ({ operation, slotId: 'S', allowed: true }));
const ref = [{ position: 0, ordinal: 0, slot: 'S' }];
const twenty = { kind: 'i64', value: '20' };
// The cells a saved record reports, as [kind, payload] pairs: 1 is an I64 cell, 4 a Program
// cell, 8 a recipe node (see `collect_signature_holders` in noble-wasm's calculator.wat).
const cells = saved => saved.capture_values.flatMap(field => field.value.map(cell => [cell.kind, cell.payload]));

const session = await cli(binary, 'LREF-CASE-04', selectedAuthority({ sources, slots, grants, quota: 4 }));
const issue = async (request, outcome) => expectOutcome(await session.issue(request), outcome);
let complete = false;
try {
  expectOutcome(await session.configured, 'configured');
  for (const [id, name] of [['v1def', 'mk1'], ['v2def', 'mk2']])
    await issue({ operation: 'define', id, name, source: sources[id], inputs: [] }, 'definition-retained');
  const v1 = await issue({ operation: 'install', id: 'v1', source: sources.v1, selected_name: 'mk1',
    inputs: [] }, 'installed');
  await issue({ operation: 'publish', slot: 'S', id: 'v1', expected_epoch: '0' }, 'published');
  for (const [id, inputs] of [['caller', [`LiveRef<empty,${program},pure>`]], ['runner', ['I64', program]]])
    await issue({ operation: 'install', id, source: sources[id], inputs }, 'installed');
  const v2 = await issue({ operation: 'install', id: 'v2', source: sources.v2, selected_name: 'mk2',
    inputs: [] }, 'installed');

  const root = await issue({ operation: 'invoke', id: 'caller', inputs: [], refs: ref }, 'executed');
  assert.deepEqual([root.epoch, root.request_trace[0].generation, root.request_trace[0].artifactSha256],
    ['1', '1', v1.artifact_sha256], 'R dispatches to v1 at epoch 1');
  const [saved] = root.stack;
  assert.equal(saved.kind, 4);
  assert.equal(saved.source_id, null, 'O is a composed Program, not a static Program of a module');
  assert.deepEqual(Object.keys(saved).sort(),
    ['capture_status', 'capture_values', 'kind', 'owner', 'program_index', 'source_id'],
    'the saved record names no slot, epoch or reference');
  const recipe = cells(saved);
  assert.ok(recipe.some(([kind, payload]) => kind === 1 && payload === '1'), 'O holds the captured 1');
  assert.ok(recipe.some(([kind, payload]) => kind === 8 && payload === '1'), 'O records its literal 1');
  assert.ok(recipe.every(([kind]) => [1, 4, 8].includes(kind)),
    'O holds only Program, I64 and recipe-node cells');

  const published = await issue({ operation: 'publish', slot: 'S', id: 'v2', expected_epoch: '1',
    expected_incarnation: '1', expected_generation: '1' }, 'published');
  assert.deepEqual([published.epoch, published.generation], ['2', '2']);
  const after = await issue({ operation: 'invoke', id: 'runner',
    inputs: [twenty, { kind: 'program', owner: saved.owner }], refs: [] }, 'executed');
  assert.deepEqual(after.stack, [{ kind: 1, value: '21' }], 'O keeps its v1 behavior after publication');
  assert.deepEqual(after.request_trace, [], 'running O dispatches nothing');

  const fresh = await issue({ operation: 'invoke', id: 'caller', inputs: [], refs: ref }, 'executed');
  assert.deepEqual([fresh.epoch, fresh.request_trace[0].generation, fresh.request_trace[0].artifactSha256],
    ['2', '2', v2.artifact_sha256], 'a new root dispatches to v2');
  const freshRun = await issue({ operation: 'invoke', id: 'runner',
    inputs: [twenty, { kind: 'program', owner: fresh.stack[0].owner }], refs: [] }, 'executed');
  assert.deepEqual(freshRun.stack, [{ kind: 1, value: '22' }]);
  assert.ok(cells(fresh.stack[0]).some(([kind, payload]) => kind === 1 && payload === '2'),
    'the fresh result holds v2\'s captured 2');
  assert.notEqual(fresh.stack[0].owner, saved.owner);

  assert.equal(await session.close(), 0);
  complete = true;
  console.log(JSON.stringify({ case: 'LREF-CASE-04', o_after_publication: after.stack,
    fresh_result: freshRun.stack, o_cells: recipe, o_epoch: root.epoch, fresh_epoch: fresh.epoch,
    selected_binary_sha256: session.selectedBinarySha256 }));
} finally { if (!complete) await session.abort(); }
