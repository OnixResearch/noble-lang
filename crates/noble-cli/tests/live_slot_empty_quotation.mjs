import assert from 'node:assert/strict';
import { cli, expectOutcome, selectedAuthority } from './live_slot_resource_ref_cli.mjs';

// An empty quotation `[ ]` compiles to the session's shared core empty entry (table slot 0), not to
// code in its module's span. The live-slot install check used to require every module Program's
// entry inside the module's span, so any source containing `[ ]` failed to install with an internal
// failure that terminated the session. Each module here installs and runs, and the session ends
// normally.
const binary = process.argv[2];
const sources = { identity: '[ ] run', empty: '[ ]', composed: '[ ] [ 1 + ] compose',
  runner: 'run', dropped: '[ ] drop' };
const installs = [['identity', ['I64']], ['empty', []], ['composed', []],
  ['runner', ['I64', 'Program<I64,I64,pure>']], ['dropped', ['I64']]];
const i64 = value => ({ kind: 'i64', value: String(value) });

const live = await cli(binary, 'empty-quotation', selectedAuthority({ sources, slots: [], effects: [] }));
const issue = async (request, outcome) => expectOutcome(await live.issue(request), outcome);
const invoke = (id, inputs) => issue({ operation: 'invoke', id, inputs, refs: [] }, 'executed');
let complete = false;
try {
  expectOutcome(await live.configured, 'configured');
  for (const [id, inputs] of installs) {
    await issue({ operation: 'install', id, source: sources[id], inputs }, 'installed');
  }
  assert.deepEqual((await invoke('identity', [i64(7)])).stack, [{ kind: 1, value: '7' }],
    'running the empty quotation leaves the stack unchanged');
  assert.deepEqual((await invoke('dropped', [i64(5)])).stack, [{ kind: 1, value: '5' }]);
  const [empty] = (await invoke('empty', [])).stack;
  assert.equal(empty.kind, 4, 'the empty quotation is a saved Program');
  const [composed] = (await invoke('composed', [])).stack;
  assert.equal(composed.kind, 4);
  assert.deepEqual((await invoke('runner', [i64(20), { kind: 'program', owner: composed.owner }])).stack,
    [{ kind: 1, value: '21' }], 'the empty quotation composes as the identity');
  for (const owner of [empty.owner, composed.owner]) {
    await issue({ operation: 'release-program', owner }, 'program-released');
  }
  assert.equal(await live.close(), 0, 'the session ends normally');
  complete = true;
} finally { if (!complete) await live.abort(); }
console.log(JSON.stringify({ empty_quotation: 'installs, runs, composes and releases' }));
