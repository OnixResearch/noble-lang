import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { canonical, cli, evidenceDirectory, expectOutcome, resourceGrants,
  resourceRows, selectedAuthority } from './live_slot_resource_ref_cli.mjs';

const binary = process.argv[2];
const canonicalCase = canonical('LSLOT-06');
assert.equal(canonicalCase.input.issued_ref, 'LiveRef<I64,I64,{}> for slot increment in root R');
const variants = canonicalCase.input.variants;
assert.deepEqual(variants.map(row => row.name), [
  'direct-quotation-capture', 'aggregate-capture', 'aggregate-storage',
  'export-or-return', 'serialize', 'forge', 'retain', 'valid-borrow',
]);
const liveRef = 'LiveRef<I64,I64,pure>';
const caller = { source: 'slot.invoke', inputs: ['I64', liveRef] };
const invalid = {
  'direct-quotation-capture': { source: '[ slot.invoke ]', inputs: caller.inputs },
  'aggregate-capture': { source: '[ slot.invoke ]', inputs: ['I64', `Pair<I64,${liveRef}>`] },
  'aggregate-storage': { source: 'pair', inputs: caller.inputs },
  'export-or-return': { source: 'slot.invoke', inputs: ['I64', `LiveRef<I64,${liveRef},pure>`] },
  'forge': { source: 'slot.invoke', inputs: ['I64', 'Text'] },
  'retain': caller,
};
const slots = [{ slotId: 'increment', input: ['I64'], output: ['I64'],
  effectCeiling: [], proofRequired: false }];
const grants = [...resourceGrants,
  { operation: 'publish', slotId: 'increment', allowed: true },
  { operation: 'dispatch', slotId: 'increment', allowed: true }];
const refs = [{ position: 1, ordinal: 0, slot: 'increment' }];
const inputs = [{ kind: 'i64', value: '20' }];
const results = [];
for (const variant of variants) {
  const name = variant.name;
  const attempt = invalid[name];
  const sources = { target: '1 +', caller: caller.source,
    ...(attempt ? { attempt: attempt.source } : {}),
    ...(name === 'aggregate-storage' ? { scalarPair: 'pair' } : {}) };
  const session = await cli(binary, `LSLOT06-${name}`, selectedAuthority({
    sources, resources: resourceRows, slots, grants,
  }));
  try {
    expectOutcome(await session.configured, 'configured');
    let negative = null;
    let sourceAttempt = null;
    let scalarControl = null;
    if (name !== 'valid-borrow' && name !== 'retain' && name !== 'serialize') {
      sourceAttempt = await session.issue({ operation: 'install', id: 'attempt',
        source: attempt.source, inputs: attempt.inputs });
      negative = sourceAttempt;
      assert.ok(['refused', 'install-refused'].includes(negative.outcome), JSON.stringify(negative));
      assert.equal(negative.guest_requests, 0, JSON.stringify(negative));
      assert.equal(negative.protected_operations, 0, JSON.stringify(negative));
      if (name === 'aggregate-storage') {
        scalarControl = await session.issue({
          operation: 'install',
          id: 'scalarPair', source: attempt.source, inputs: ['I64', 'I64'],
        });
      }
      const empty = expectOutcome(await session.issue({ operation: 'trace' }), 'trace-observed');
      assert.deepEqual(empty.trace, []);
    }
    expectOutcome(await session.issue({ operation: 'install', id: 'target',
      source: '1 +', inputs: ['I64'] }), 'installed');
    const published = expectOutcome(await session.issue({ operation: 'publish',
      slot: 'increment', id: 'target', expected_epoch: '0' }), 'published');
    assert.equal(published.epoch, '1');
    expectOutcome(await session.issue({ operation: 'install', id: 'caller',
      source: caller.source, inputs: caller.inputs }), 'installed');
    const site = expectOutcome(await session.issue({ operation: 'reflect', id: 'caller', site_id: 0 }), 'reflected');
    assert.deepEqual(site.site.input, ['I64']);
    assert.deepEqual(site.site.output, ['I64']);
    assert.deepEqual(site.site.effects, ['live.dispatch']);
    assert.equal(JSON.stringify(site.site).includes('increment'), false,
      'generic compiled site may not embed selected slot name');
    if (name === 'forge') {
      negative = await session.issue({ operation: 'invoke', id: 'caller', inputs,
        refs: [{ position: 1, ordinal: 0, slot: 'increment-forged' }] });
      expectOutcome(negative, 'refused');
      const empty = expectOutcome(await session.issue({ operation: 'trace' }), 'trace-observed');
      assert.deepEqual(empty.trace, []);
    }
    const valid = expectOutcome(await session.issue({ operation: 'invoke', id: 'caller', inputs, refs }), 'executed');
    assert.equal(valid.epoch, '1');
    assert.deepEqual(valid.stack, [{ kind: 1, value: '21' }]);
    assert.equal(valid.guest_requests, 1);
    assert.equal(valid.protected_operations, 0);
    assert.deepEqual(valid.request_trace.map(item => item.operation), ['dispatch']);
    if (name === 'retain') {
      // There is no exportable ref credential. Supplying a claimed prior-root
      // owner is refused before any second guest root, not proof that an actual
      // expired borrowed ref could be represented by this input protocol.
      negative = await session.issue({ operation: 'invoke', id: 'caller', inputs,
        refs: [{ ...refs[0], owner: 'previous-root-R' }] });
      expectOutcome(negative, 'refused');
      const trace = expectOutcome(await session.issue({ operation: 'trace' }), 'trace-observed');
      assert.deepEqual(trace.trace, valid.request_trace);
      const freshRoot = expectOutcome(await session.issue({ operation: 'invoke', id: 'caller', inputs, refs }),
        'executed');
      assert.equal(freshRoot.epoch, '1');
      assert.deepEqual(freshRoot.stack, valid.stack);
      assert.equal(freshRoot.guest_requests, 1);
      assert.equal(freshRoot.protected_operations, 0);
    }
    if (negative) {
      assert.equal(negative.guest_requests, 0, JSON.stringify(negative));
      assert.equal(negative.protected_operations, 0, JSON.stringify(negative));
    }
    const limitation = name === 'aggregate-capture'
      ? 'nested Pair<LiveRef> root type is refused before quotation-capture can be checked'
      : name === 'export-or-return'
        ? 'target output of LiveRef is refused at static type boundary; no source form returns the original root borrow'
      : name === 'serialize'
        ? 'no selected CLI source word encodes LiveRef as portable data; an unrelated dup would duplicate I64 instead'
        : name === 'forge'
          ? 'source has no input ref and forged slot binding is refused, but no decode-bytes-to-ref primitive exists'
          : name === 'retain'
            ? 'prior-root owner claim refused; CLI never exposes a ref credential to actually retain/reuse'
            : null;
    results.push({ name, status: name === 'serialize' ? 'non-run'
      : name === 'valid-borrow' || !limitation && (!scalarControl || scalarControl.outcome === 'installed')
        ? 'pass' : 'partial',
      negative_outcome: negative?.outcome ?? 'not applicable',
      negative_diagnostic: negative?.diagnostic ?? null,
      source_attempt_outcome: sourceAttempt?.outcome ?? 'not applicable',
      scalar_control_outcome: scalarControl?.outcome ?? 'not applicable',
      limitation: scalarControl && scalarControl.outcome !== 'installed'
        ? `scalar control source did not install: ${scalarControl.outcome}; ${limitation ?? 'cannot isolate borrowed ref as only cause'}`
        : limitation,
      epoch_before: '0', epoch_after: '1 (separate valid target publication)',
      negative_guest_requests: negative?.guest_requests ?? null,
      negative_protected_operations: negative?.protected_operations ?? null,
      valid_guest_requests: valid.guest_requests, valid_protected_operations: valid.protected_operations,
      valid_stack: valid.stack, site: site.site, selected_binary_sha256: session.selectedBinarySha256,
      raw: session.log });
    assert.equal(await session.close(), 0);
  } catch (error) {
    await session.abort();
    throw Error(`${name}: ${error} raw=${session.log}`, { cause: error });
  }
}
const summary = path.join(evidenceDirectory, 'LSLOT06-summary.json');
fs.writeFileSync(summary, JSON.stringify({ selected_binary: binary,
  canonical: 'specs/conformance/live-reference-cases.json#LSLOT-06', results }, null, 2));
console.log(JSON.stringify({ summary, results }));
