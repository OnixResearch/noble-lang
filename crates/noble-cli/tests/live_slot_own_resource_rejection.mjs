import assert from 'node:assert/strict';
import { cli, expectOutcome, selectedAuthority } from './live_slot_resource_ref_cli.mjs';

// OWN-CASE-04 (`.cairn/changes/ownership-lifetime-contract`) through the real CLI: the registered
// `Account@1` opaque nominal carries one ownership obligation, and each of the design's six
// non-borrow source programs is refused at the check stage before any candidate is emitted. The
// borrow variants of the case are inapplicable under OWNER DECISION 2026-11-20 (decision 7: no
// borrow profile is selected).
//
// Every row below is the CLI's own refusal, not a constant: the harness reads `outcome`, `stage`,
// `diagnostic`, `guest_requests` and `protected_operations` from the reply the operator received.
// `emitted` is derived the same way: a refusal carries no checked artifact or handle, while the
// two controls -- an I64 program and a swap that passes the registered resource through -- report
// both, so the harness is observed to emit whenever the checker accepts the program.
const binary = process.argv[2];
const resource = { name: 'Account@1', module: '110', ordinal: 0, kind: 41, owner: 'resource-ref-fixture' };
const grants = [
  { operation: 'register-resource', nominalId: '110:0', allowed: true },
  { operation: 'issue-resource', nominalId: '110:0', allowed: true },
];
const variants = [
  { variant: 'dup', source: 'dup', inputs: ['Account@1'], input: 'Account@1' },
  { variant: 'drop', source: 'drop', inputs: ['Account@1'], input: 'Account@1' },
  { variant: 'quote', source: 'quote', inputs: ['Account@1'], input: 'Account@1' },
  { variant: 'pair-then-quote', source: 'quote', inputs: ['Pair<I64,Account@1>'],
    input: 'Pair<I64,Account@1>' },
  { variant: 'sum-then-drop', source: 'drop', inputs: ['Sum<Account@1,I64>'],
    input: 'Sum<Account@1,I64>' },
  { variant: 'list-then-drop', source: 'drop', inputs: ['List<Account@1>'],
    input: 'List<Account@1>' },
];
const controls = [
  { control: 'i64-program', source: 'drop [ 1 + ]', inputs: ['I64'] },
  { control: 'resource-pass-through', source: 'swap', inputs: ['I64', 'Account@1'] },
];
const sources = Object.fromEntries([
  ...variants.map((row, index) => [`variant-${index}`, row.source]),
  ...controls.map((row, index) => [`control-${index}`, row.source]),
]);
const live = await cli(binary, 'OWN-CASE-04', selectedAuthority({ sources, resources: [resource],
  slots: [], grants }));
const observed = { case: 'OWN-CASE-04', variants: [], controls: [], selected_binary_sha256: null };
let complete = false;
try {
  expectOutcome(await live.configured, 'configured');
  for (const [index, row] of variants.entries()) {
    const reply = await live.issue({ operation: 'install', id: `variant-${index}`,
      source: sources[`variant-${index}`], inputs: row.inputs });
    assert.equal(reply.outcome, 'refused', JSON.stringify(reply));
    assert.equal(reply.stage, 'command', JSON.stringify(reply));
    assert.match(reply.diagnostic, /kernel rejected a non-capturable value/, JSON.stringify(reply));
    assert.equal(reply.guest_requests, 0, JSON.stringify(reply));
    assert.equal(reply.protected_operations, 0, JSON.stringify(reply));
    assert.equal(reply.handle, undefined, 'a refused program never acquires an installed handle');
    assert.equal(reply.artifact_sha256, undefined, 'a refused program never emits an artifact');
    observed.variants.push({ variant: row.variant, input: row.input, source: row.source,
      outcome: reply.outcome, stage: reply.stage, diagnostic: reply.diagnostic,
      guest_requests: reply.guest_requests, protected_operations: reply.protected_operations,
      emitted: reply.artifact_sha256 !== undefined });
  }
  for (const [index, row] of controls.entries()) {
    const reply = await live.issue({ operation: 'install', id: `control-${index}`,
      source: sources[`control-${index}`], inputs: row.inputs });
    expectOutcome(reply, 'installed');
    assert.equal(typeof reply.artifact_sha256, 'string', JSON.stringify(reply));
    assert.equal(reply.guest_requests, 0, JSON.stringify(reply));
    observed.controls.push({ control: row.control, inputs: row.inputs, outcome: reply.outcome,
      handle: reply.handle, artifact_sha256: reply.artifact_sha256,
      guest_requests: reply.guest_requests, emitted: true });
  }
  observed.exit = await live.close();
  observed.selected_binary_sha256 = live.selectedBinarySha256;
  complete = true;
} finally { if (!complete) await live.abort(); }
assert.equal(observed.exit, 0);
console.log(JSON.stringify(observed));
