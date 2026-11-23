import assert from 'node:assert/strict';
import { cli, expectOutcome, selectedAuthority } from './live_slot_resource_ref_cli.mjs';

// ID-05's capture change (`.cairn/archive/2026-11-23-runtime-program-template-identity`, task 5,
// P-ID-03): the builder `quote [ + ] compose` applied to the root inputs 2 and 3 yields two
// Programs with
// the same anonymous definition template and different ProgramValueIds, whose captures are the
// root inputs. Control: builders that fix the literal (`2 quote [ + ] compose`,
// `3 quote [ + ] compose`) yield different templates, so the shared template above comes from the
// varying root input, not from equal-looking code. Identities are the live-slot runtime's
// experimental local v4 identities; this is not a ProgramsData-Draft conformance run.
const binary = process.argv[2];
assert.ok(binary, 'pass the selected noble CLI binary');
const definitions = {
  builder: 'def builder [ quote [ + ] compose ]',
  fixed2: 'def fixed2 [ 2 quote [ + ] compose ]',
  fixed3: 'def fixed3 [ 3 quote [ + ] compose ]',
};
const sources = Object.fromEntries(Object.entries(definitions).flatMap(([name, definition]) =>
  [[name, name], [`definition-${name}`, definition]]));
const session = await cli(binary, 'ID-05-capture-identity', selectedAuthority({ sources, slots: [] }));
const issue = async (request, outcome) => expectOutcome(await session.issue(request), outcome);
const identityOf = row => {
  const [program] = row.stack;
  assert.equal(program.kind, 4);
  assert.ok(program.target_identity, 'a checked returned target');
  return program.target_identity;
};
let complete = false;
try {
  expectOutcome(await session.configured, 'configured');
  for (const [name, definition] of Object.entries(definitions)) {
    await issue({ operation: 'define', id: `definition-${name}`, name, source: definition, inputs: [] },
      'definition-retained');
  }
  // The selected-origin builder is installed first, so it owns the native quote/compose code.
  await issue({ operation: 'install', id: 'builder', source: 'builder', selected_name: 'builder',
    inputs: ['I64'] }, 'installed');
  for (const name of ['fixed2', 'fixed3']) {
    await issue({ operation: 'install', id: name, source: name, selected_name: name, inputs: [] },
      'installed');
  }
  const build = async (id, inputs) => identityOf(await issue({ operation: 'invoke', id, inputs,
    refs: [] }, 'executed'));
  const before = await build('builder', [{ kind: 'i64', value: '2' }]);
  const after = await build('builder', [{ kind: 'i64', value: '3' }]);
  assert.deepEqual(before.captures, [{ type: 'I64', value: '2' }]);
  assert.deepEqual(after.captures, [{ type: 'I64', value: '3' }]);
  assert.equal(before.definition_id, after.definition_id, 'one definition template');
  assert.notEqual(before.program_value_id, after.program_value_id, 'two Program values');
  const fixed2 = await build('fixed2', []);
  const fixed3 = await build('fixed3', []);
  assert.notEqual(fixed2.definition_id, fixed3.definition_id, 'a fixed literal is part of the template');
  assert.notEqual(fixed2.definition_id, before.definition_id);
  assert.equal(await session.close(), 0);
  complete = true;
  const pick = identity => ({ program_value_id: identity.program_value_id,
    definition_id: identity.definition_id, captures: identity.captures });
  console.log(JSON.stringify({ case: 'ID-05', capture_before: pick(before), capture_after: pick(after),
    fixed_control: { fixed2: pick(fixed2), fixed3: pick(fixed3) },
    selected_binary_sha256: session.selectedBinarySha256 }));
} finally { if (!complete) await session.abort(); }
