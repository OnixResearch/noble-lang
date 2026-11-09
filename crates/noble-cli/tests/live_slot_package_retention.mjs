import assert from 'node:assert/strict';
import { cli, expectOutcome, selectedAuthority } from './live_slot_resource_ref_cli.mjs';

// LREF-CASE-03 (`.cairn/changes/live-reference-lifetimes`): package retention across slot
// replacement. Slot S's version v1 is module M1's root, whose body returns `[ h ]`; the compiled
// body reads `h` through an M1 global. A root R dispatches to v1 through `slot.invoke` and saves
// the returned Program O; R then ends and v2 replaces v1, so the publication's sweep finds O as
// the only owner of M1. `20 O run` must still yield 21 (a stale-handle trap fails the case), M1's
// span must not retire while O lives, and releasing O must retire the span and reclaim the
// retained-version charge.
//   replacement (quota 2): v2 is published while O lives; O's release retires M1's span.
//   charge (quota 1): O holds v1's charge, so v2's publication is refused while O lives and
//   admitted once O is released, retiring M1's span.
const binary = process.argv[2];
const sources = {
  hdef: 'def h [ 1 + ]', mkdef: 'def mk [ [ h ] ]', mk: 'mk',
  v2def: 'def mk2 [ [ 5 + ] ]', v2: 'mk2', caller: 'slot.invoke', runner: 'run',
};
const slots = [{ slotId: 'S', input: [], output: ['Program<I64,I64,pure>'], effectCeiling: [],
  proofRequired: false }];
const grants = ['publish', 'dispatch'].map(operation => ({ operation, slotId: 'S', allowed: true }));

async function scenario(variant, quota, body) {
  const session = await cli(binary, `LREF-CASE-03-${variant}`,
    selectedAuthority({ sources, quota, slots, grants }));
  const issue = async (request, outcome) => expectOutcome(await session.issue(request), outcome);
  const observe = (label, row) => console.log(JSON.stringify({ variant, observation: label,
    outcome: row.outcome, stack: row.stack ?? null, epoch: row.epoch ?? null,
    diagnostic: row.diagnostic ?? null, retire_code_spans: row.retire_code_spans ?? null }));
  let complete = false;
  try {
    expectOutcome(await session.configured, 'configured');
    for (const [id, name] of [['hdef', 'h'], ['mkdef', 'mk'], ['v2def', 'mk2']]) {
      await issue({ operation: 'define', id, name, source: sources[id], inputs: [] },
        'definition-retained');
    }
    const m1 = await issue({ operation: 'install', id: 'mk', source: sources.mk,
      selected_name: 'mk', inputs: [] }, 'installed');
    await issue({ operation: 'publish', slot: 'S', id: 'mk', expected_epoch: '0' }, 'published');
    for (const [id, inputs] of [['caller', ['LiveRef<empty,Program<I64,I64,pure>,pure>']],
      ['runner', ['I64', 'Program<I64,I64,pure>']]]) {
      await issue({ operation: 'install', id, source: sources[id], inputs }, 'installed');
    }
    await issue({ operation: 'install', id: 'v2', source: sources.v2, selected_name: 'mk2',
      inputs: [] }, 'installed');
    const root = await issue({ operation: 'invoke', id: 'caller', inputs: [],
      refs: [{ position: 0, ordinal: 0, slot: 'S' }] }, 'executed');
    assert.equal(root.request_trace[0].generation, '1', 'R must dispatch to v1');
    assert.equal(root.stack.length, 1);
    const [saved] = root.stack;
    assert.equal(saved.kind, 4);
    assert.equal(saved.source_id, 'mk', 'O must be a static Program of M1');
    const context = {
      issue, observe,
      ofM1: spans => spans.filter(span => span.generation === Number(m1.generation)),
      run: async label => {
        const row = await session.issue({ operation: 'invoke', id: 'runner',
          inputs: [{ kind: 'i64', value: '20' }, { kind: 'program', owner: saved.owner }], refs: [] });
        observe(label, row);
        return row;
      },
      publishV2: async label => {
        const row = await session.issue({ operation: 'publish', slot: 'S', id: 'v2',
          expected_epoch: '1', expected_incarnation: '1', expected_generation: '1' });
        observe(label, row);
        return row;
      },
      release: async label => {
        const row = await issue({ operation: 'release-program', owner: saved.owner },
          'program-released');
        observe(label, row);
        return row;
      },
    };
    const result = await body(context);
    assert.equal(await session.close(), 0);
    complete = true;
    return { ...result, selected_binary_sha256: session.selectedBinarySha256 };
  } finally { if (!complete) await session.abort(); }
}

const replacement = await scenario('replacement', 2, async ({ ofM1, run, publishV2, release }) => {
  const published = expectOutcome(await publishV2('publication-while-owned'), 'published');
  assert.deepEqual(ofM1(published.retire_code_spans), [],
    'M1 must not become retirable while O reaches it');
  const after = expectOutcome(await run('run-after-replacement'), 'executed');
  assert.deepEqual(after.stack, [{ kind: 1, value: '21' }]);
  const [span] = ofM1((await release('release')).retire_code_spans);
  assert.ok(span, 'releasing O must let M1 retire');
  expectOutcome(await run('run-after-release'), 'refused');
  return { after_replacement: after.stack, retired_span: span };
});

const charge = await scenario('charge', 1, async ({ ofM1, run, publishV2, release }) => {
  const refused = expectOutcome(await publishV2('publication-while-owned'),
    'retention-budget-refused');
  assert.equal(refused.epoch, '1');
  const owned = expectOutcome(await run('run-while-owned'), 'executed');
  assert.deepEqual(owned.stack, [{ kind: 1, value: '21' }]);
  await release('release');
  const published = expectOutcome(await publishV2('publication-after-release'), 'published');
  const [span] = ofM1(published.retire_code_spans);
  assert.ok(span, 'the publication after O is released must retire M1');
  return { refused_while_owned: refused.outcome, retired_span: span };
});

console.log(JSON.stringify({ case: 'LREF-CASE-03', replacement, charge }));
