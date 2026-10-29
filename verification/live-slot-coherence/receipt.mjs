#!/usr/bin/env node
/**
 * Source/model/artifact-bound receipt for the VC-LIVE-03 host correspondence of coherent entry
 * maps, keeping three kinds of evidence in **distinct** sections:
 *
 *   model                       - named kernel-checked theorems and the retained foundation receipt;
 *   implementation_comparison   - the measured host lane and the observed counts it asserts;
 *   host_correspondence         - the exact selected CLI bytes, the launch recipe and the session
 *                                 identifiers the measurements came from.
 *
 * Nothing here promotes a conformance case, and no section may stand in for another: the model
 * theorems do not prove the runtime behaves this way, the lane does not prove the model, and the
 * host correspondence only binds bytes to observations.
 *
 * Usage (pinned Node, env -i, from the repository root):
 *   node verification/live-slot-coherence/receipt.mjs ABSOLUTE_PREBUILT_CLI NEW_TMP_DIRECTORY
 */
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const sha = bytes => createHash('sha256').update(bytes).digest('hex');
const unsafeStartup = Object.keys(process.env).filter(name =>
  /^(?:NODE_OPTIONS|NODE_PATH|BASH_ENV|ENV|LD_.*|DYLD_.*|NIX_LD(?:_.*)?|GLIBC_TUNABLES)$/.test(name));
assert.deepEqual(unsafeStartup, [], `unsafe receipt startup environment: ${unsafeStartup.join(', ')}`);
assert.equal(process.argv.length, 4,
  'usage: node receipt.mjs ABSOLUTE_PREBUILT_CLI NEW_TMP_DIRECTORY');
const binary = path.resolve(process.argv[2]);
const output = path.resolve(process.argv[3]);
assert.equal(binary, process.argv[2], 'CLI selection must be an absolute path');
assert.match(output, /^\/tmp\/[^/]+$/, 'receipt must be a fresh, direct /tmp directory');
assert.ok(!fs.existsSync(output), 'receipt output must not exist');
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
fs.mkdirSync(output, { mode: 0o700 });
const save = (name, content) => fs.writeFileSync(path.join(output, name),
  typeof content === 'string' ? content : JSON.stringify(content, null, 2) + '\n',
  { flag: 'wx', mode: 0o600 });
const repositoryBytes = relative => {
  const bytes = fs.readFileSync(path.join(root, relative));
  return { path: relative, sha256: sha(bytes) };
};

// The lane is a sibling of the digest-pinned nested-roots lane and must stay self-contained.
const lane = repositoryBytes('crates/noble-cli/tests/live_slot_coherent_maps.mjs');
const runner = repositoryBytes('crates/noble-cli/tests/live_slot_coherent_maps.rs');
const helper = repositoryBytes('crates/noble-cli/tests/live_slot_resource_ref_cli.mjs');

const modelTheorems = [
  'NobleFoundation.LiveDispatch.publish_preserves_other_keys',
  'NobleFoundation.LiveDispatch.publish_selects_the_published_generation',
  'NobleFoundation.LiveDispatch.pin_is_immutable',
  'NobleFoundation.LiveDispatch.root_selections_use_pinned_map',
  'NobleFoundation.LiveDispatch.frozen_entry_map_reference_agrees',
  'NobleFoundation.LiveDispatch.mixed_map_selects_old_and_new',
  'NobleFoundation.LiveDispatch.denied_dispatch_produces_no_guest_event',
  'NobleFoundation.LiveDispatch.authorized_dispatch_selects_the_pinned_generation',
  'NobleFoundation.LiveDispatch.abnormal_prefix_preserves_committed_events',
  'NobleFoundation.LiveDispatch.run_extends_the_events',
  'NobleFoundation.LiveDispatch.denials_only_truncate',
  'NobleFoundation.LiveDispatch.coherent_pins_emit_exactly_once',
  'NobleFoundation.LiveDispatch.current_map_selection_duplicates_and_omits',
  'NobleFoundation.LiveDispatch.a_root_pinned_between_sequential_publications_can_mix',
  'NobleFoundation.LiveDispatch.a_denial_before_the_emitting_hop_keeps_the_prefix',
];
const inventory = JSON.parse(fs.readFileSync(path.join(root, 'proofs/foundation/inventory.json'), 'utf8'));
for (const theorem of modelTheorems) {
  assert.ok(inventory.theorems.some(row => row.name === theorem),
    `model theorem ${theorem} missing from the foundation inventory`);
}
const foundationReceipt = path.join('verification/foundation/receipts/1d4113e6c');
for (const name of ['foundation-check.json', 'Audit.lean']) {
  assert.ok(fs.existsSync(path.join(root, foundationReceipt, name)),
    `retained foundation receipt ${foundationReceipt}/${name} missing`);
}
const foundationCheck = JSON.parse(
  fs.readFileSync(path.join(root, foundationReceipt, 'foundation-check.json'), 'utf8'));

// Coverage inherited from lanes that already measure revocation, abnormal prefix, stale-CAS/ABA
// and last-pin retention. Digests bind them; their own receipts stay their own claims.
const inherited = [
  ['crates/noble-cli/tests/live_slot_relocation_witness.mjs',
    'independent policy revocation (denied effect) and the abnormal dispatch prefix'],
  ['crates/noble-cli/tests/live_slot_cas_policy.mjs', 'stale-CAS/ABA publication controls'],
  ['crates/noble-cli/tests/live_slot_selected_retention.mjs', 'old-root/new-root retention'],
  ['crates/noble-cli/tests/live_slot_nested_roots.mjs', 'LSLOT-02 nested and transitive roots'],
].map(([relative, what]) => ({ ...repositoryBytes(relative), what }));

const binarySha256 = sha(fs.readFileSync(binary));
const run = spawnSync(process.execPath, [path.join(root, lane.path), binary],
  { encoding: 'utf8', env: { PATH: process.env.PATH, HOME: os.userInfo().homedir } });
save('lane.stdout', run.stdout ?? '');
save('lane.stderr', run.stderr ?? '');
assert.equal(run.status, 0, `coherent-map lane failed: ${(run.stderr ?? '').slice(-800)}`);
const observed = JSON.parse((run.stdout ?? '').trim().split('\n').filter(Boolean).at(-1));
assert.deepEqual(observed.leafFirstCounts, { dispatches: 1, effects: 1 });
assert.deepEqual(observed.callerFirstCounts, { dispatches: 1, effects: 1 });
assert.equal(observed.heldPublicationOutcome, 'published');
assert.notEqual(observed.publishedEpoch, observed.pinnedEpoch);
assert.deepEqual(observed.heldRootCounts, { dispatches: 1, effects: 1 });
assert.deepEqual(observed.freshRootCounts, { dispatches: 1, effects: 1 });
assert.notEqual(observed.heldRootDefinition, observed.freshRootDefinition);
assert.equal(observed.withoutCompareAndSetOutcome, 'stale-reject');
assert.deepEqual(observed.omissionCounts, { dispatches: 1, effects: 0 });
assert.equal(observed.halfPublishedRows, 0);
assert.equal(observed.halfPublishedSessionState, 'terminated-after-guest-failure');
assert.equal(sha(fs.readFileSync(binary)), binarySha256,
  'selected CLI changed while collecting the coherent-map receipt');

save('receipt.json', {
  schema: 'noble-live-slot-coherence-receipt/v1',
  claim: 'selected host correspondence for coherent entry maps; not a proof and not a promotion',
  model: {
    what: 'kernel-checked model theorems about pinned entry maps, single publications, event relocation under a pin, the broken current-map control and denial truncation',
    theorems: modelTheorems,
    retained_receipt: { directory: foundationReceipt,
      theorem_count: foundationCheck.theorem_count,
      clean_source: foundationCheck.clean_source },
    does_not_establish: 'that the selected runtime behaves this way',
  },
  implementation_comparison: {
    what: 'measured counts the host lane asserts, as a behavioral comparison',
    lane, runner, helper,
    observed,
    commands: [{ argv: ['node', lane.path, 'ABSOLUTE_PREBUILT_CLI'],
      status: run.status }],
    does_not_establish: 'that the model theorems hold, or that any other authority shape behaves so',
  },
  host_correspondence: {
    what: 'the exact selected bytes and the sessions the measurements came from',
    binary: { path: binary, sha256: binarySha256 },
    sessions: ['coherent-leaf-first', 'coherent-caller-first', 'coherent-sequential', 'coherent-omission'],
    does_not_establish: 'any claim about a differently built CLI or a different fixture',
  },
  inherited_coverage: {
    what: 'revocation, abnormal prefix, stale-CAS/ABA and retention coverage measured elsewhere',
    lanes: inherited,
    does_not_establish: 'that those lanes were re-run here; only their bytes are bound',
  },
  non_claims: [
    'the deliberately broken per-dispatch-current-map duplicate control is not constructible from this authority shape; it exists only in the model (current_map_selection_duplicates_and_omits)',
    'retention is measured for one root held before its only selection while one compare-and-set publication lands; publications between two selections of one root are the nested-roots lane, bound here by digest only',
    'a publication without the incarnation/generation compare-and-set fields stale-rejects whether or not a root is pinned; that refusal is not retention evidence',
    'no conformance case (LSLOT-*) is promoted by this receipt',
  ],
});
process.stdout.write(`${JSON.stringify({ receipt: path.join(output, 'receipt.json'),
  binary: binarySha256, observed })}\n`);
