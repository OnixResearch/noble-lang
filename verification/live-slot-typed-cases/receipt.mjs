#!/usr/bin/env node
/**
 * Source/model/artifact-bound receipt for the typed live-slot case lane (issue #49). Three kinds of
 * evidence stay in distinct sections and none may stand in for another:
 *
 *   model                      - named kernel-checked LiveDispatch theorems and the retained
 *                                foundation receipt; they do not prove the runtime counts below;
 *   implementation_comparison  - the measured lane output (typed coverage, controlled negatives,
 *                                failure-class-preserving minimization, retention budget, type
 *                                boundaries) as a behavioral comparison;
 *   host_correspondence        - the exact selected CLI bytes and the sessions measured.
 *
 * Passing typed cases are NOT theorems, and the negative family asserts declared classes and
 * stages rather than diagnostic wording.
 *
 * Usage (pinned Node, env -i, from the repository root):
 *   node verification/live-slot-typed-cases/receipt.mjs ABSOLUTE_PREBUILT_CLI NEW_TMP_DIRECTORY
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
const repositoryBytes = relative => ({
  path: relative, sha256: sha(fs.readFileSync(path.join(root, relative))),
});

const lane = repositoryBytes('crates/noble-cli/tests/live_slot_typed_cases.mjs');
const runner = repositoryBytes('crates/noble-cli/tests/live_slot_typed_cases.rs');
const helper = repositoryBytes('crates/noble-cli/tests/live_slot_resource_ref_cli.mjs');

// The typed lane measures authorized and denied live dispatch; these named LiveDispatch theorems are
// the model it is compared against, and they do not establish any count.
const modelTheorems = [
  'NobleFoundation.LiveDispatch.authorized_dispatch_selects_the_pinned_generation',
  'NobleFoundation.LiveDispatch.denied_dispatch_produces_no_guest_event',
];
const inventory = JSON.parse(fs.readFileSync(path.join(root, 'proofs/foundation/inventory.json'), 'utf8'));
for (const theorem of modelTheorems) {
  assert.ok(inventory.theorems.some(row => row.name === theorem),
    `model theorem ${theorem} missing from the foundation inventory`);
}
const foundationReceipt = path.join('verification/foundation/receipts/d2aa7a382');
for (const name of ['foundation-check.json', 'Audit.lean']) {
  assert.ok(fs.existsSync(path.join(root, foundationReceipt, name)),
    `retained foundation receipt ${foundationReceipt}/${name} missing`);
}
const foundationCheck = JSON.parse(
  fs.readFileSync(path.join(root, foundationReceipt, 'foundation-check.json'), 'utf8'));

const inherited = [
  ['crates/noble-cli/tests/live_slot_coherent_maps.mjs', 'coherent entry maps (VC-LIVE-03)'],
  ['crates/noble-cli/tests/live_slot_relocation_witness.mjs',
    'independent policy revocation (denied effect) and the abnormal dispatch prefix'],
  ['crates/noble-cli/tests/live_slot_cas_policy.mjs', 'stale-CAS/ABA publication controls'],
  ['crates/noble-cli/tests/live_slot_nested_roots.mjs', 'LSLOT-02 nested and transitive roots'],
].map(([relative, what]) => ({ ...repositoryBytes(relative), what }));

const binarySha256 = sha(fs.readFileSync(binary));
const run = spawnSync(process.execPath, [path.join(root, lane.path), binary],
  { encoding: 'utf8', env: { PATH: process.env.PATH, HOME: os.userInfo().homedir } });
save('lane.stdout', run.stdout ?? '');
save('lane.stderr', run.stderr ?? '');
assert.equal(run.status, 0, `typed-case lane failed: ${(run.stderr ?? '').slice(-800)}`);
const observed = JSON.parse((run.stdout ?? '').trim().split('\n').filter(Boolean).at(-1));

assert.deepEqual(observed.coverage.by_shape, { direct: 4, chain: 1 });
assert.deepEqual(observed.coverage.by_nesting, { single: 4, nested: 1 });
assert.deepEqual(observed.coverage.by_repeated_reborrow, { once: 2, twice: 2 });
assert.deepEqual(observed.negative.by_stage, { command: 3, slot: 1 });
assert.equal(observed.negative.refused, 4);
assert.deepEqual(observed.minimization.rejected_shrinks, []);
assert.equal(observed.minimization.minimal_starting_witnesses, 4);
assert.deepEqual(observed.retention,
  { bound: 128, retained: 128, refusal_stage: 'command',
    observed_diagnostic: observed.retention.observed_diagnostic });
assert.deepEqual(
  { stack: observed.typeBoundaries.stack, nesting: observed.typeBoundaries.nesting,
    refusal_stage: observed.typeBoundaries.refusal_stage },
  { stack: { admitted: 128, refused_first: 129 },
    nesting: { admitted: 31, refused_first: 32 }, refusal_stage: 'authority' });
assert.equal(sha(fs.readFileSync(binary)), binarySha256,
  'selected CLI changed while collecting the typed-case receipt');

save('receipt.json', {
  schema: 'noble-live-slot-typed-cases-receipt/v1',
  claim: 'typed live-case coverage, controlled negatives, minimization, retention budget and type boundaries; not a theorem and not a promotion',
  model: {
    what: 'kernel-checked LiveDispatch theorems about authorized and denied live dispatch',
    theorems: modelTheorems,
    retained_receipt: { directory: foundationReceipt,
      theorem_count: foundationCheck.theorem_count,
      clean_source: foundationCheck.clean_source },
    does_not_establish: 'any dispatch/effect count, refusal class or boundary measured below',
  },
  implementation_comparison: {
    what: 'the measured lane output, as a behavioral comparison against the declared tables',
    lane, runner, helper,
    observed: {
      seed: observed.seed, coverage: observed.coverage, negative: observed.negative,
      minimization: observed.minimization, retention: observed.retention,
      typeBoundaries: observed.typeBoundaries,
    },
    commands: [{ argv: ['node', lane.path, 'ABSOLUTE_PREBUILT_CLI'], status: run.status },
      { argv: ['cargo', 'test', '-p', 'noble-cli', '--test', 'live_slot_typed_cases'] }],
    does_not_establish: 'that the model theorems hold for these fixtures, or that another authority shape behaves so',
  },
  host_correspondence: {
    what: 'the exact selected bytes and the sessions the measurements came from',
    binary: { path: binary, sha256: binarySha256 },
    sessions: ['typed-direct-pure-repeat1-0', 'typed-direct-pure-repeat2-1',
      'typed-direct-dispatch+emit-repeat1-2', 'typed-direct-dispatch+emit-repeat2-3',
      'typed-negative-binding-foreign-root-0', 'typed-negative-binding-wrong-position-1',
      'typed-negative-stack-incompatible-2', 'typed-negative-effect-not-granted-3',
      'typed-retention', 'type-stack-admitted', 'type-stack-refused',
      'nesting-admitted', 'nesting-refused', 'typed-chain-nested-repeat1-4'],
    does_not_establish: 'any claim about a differently built CLI or a different fixture authority',
  },
  inherited_coverage: {
    what: 'coherent maps, revocation, abnormal prefix, stale-CAS/ABA and nested roots measured elsewhere',
    lanes: inherited,
    does_not_establish: 'that those lanes were re-run here; only their bytes are bound',
  },
  non_claims: [
    'a passing typed case is not a theorem and promotes no LSLOT conformance case',
    'the negative family asserts declared semantic classes and stages, never diagnostic wording; observed diagnostics are reported for transparency only',
    'minimality is along the tried witness dimensions (repeat, quota, program index) only',
    'quota is a declared retention quota field, not an invocation budget; no exhausted class is asserted for it',
    'the nested chain case is one fixture assembled through the canonical bootstrap derivation; it does not establish that every nested or transitive chain type-checks',
  ],
});
process.stdout.write(`${JSON.stringify({ receipt: path.join(output, 'receipt.json'),
  binary: binarySha256, coverage: observed.coverage })}\n`);
