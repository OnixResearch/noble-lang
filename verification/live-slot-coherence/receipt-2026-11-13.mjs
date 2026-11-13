#!/usr/bin/env node
/**
 * Dated VC-LIVE-03 implementation observation receipt. It keeps the kernel-checked model,
 * runtime lane results and the selected default/test-only CLI bytes in separate sections.
 *
 * Usage (pinned Node; from a clean committed source tree):
 *   node verification/live-slot-coherence/receipt-2026-11-13.mjs \
 *     ABSOLUTE_DEFAULT_NOBLE ABSOLUTE_TEST_ONLY_CURRENT_MAP_NOBLE NEW_OUTPUT_DIRECTORY
 */
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const sha = bytes => createHash('sha256').update(bytes).digest('hex');
const marker = Buffer.from('noble-test-current-map-seam/v1');
const unsafeStartup = Object.keys(process.env).filter(name =>
  /^(?:NODE_OPTIONS|NODE_PATH|BASH_ENV|ENV|LD_.*|DYLD_.*|NIX_LD(?:_.*)?|GLIBC_TUNABLES)$/.test(name));
assert.deepEqual(unsafeStartup, [], `unsafe receipt startup environment: ${unsafeStartup.join(', ')}`);
assert.equal(process.versions.node.split('.')[0], '24', 'receipt must use pinned Node 24');
assert.equal(process.argv.length, 5,
  'usage: node receipt-2026-11-13.mjs DEFAULT_NOBLE TEST_ONLY_CURRENT_MAP_NOBLE NEW_OUTPUT_DIRECTORY');
const binaries = process.argv.slice(2, 4).map(value => {
  const absolute = path.resolve(value);
  assert.equal(value, absolute, 'CLI selections must be absolute paths');
  assert.ok(fs.statSync(absolute).isFile(), `selected CLI is missing: ${absolute}`);
  return { path: absolute, sha256: sha(fs.readFileSync(absolute)) };
});
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const output = path.resolve(process.argv[4]);
assert.equal(output, process.argv[4], 'receipt output path must be absolute');
assert.ok(!output.startsWith(`${root}/`) && output !== root,
  'receipt output must be outside the repository');
assert.ok(!fs.existsSync(output), 'receipt output directory must be fresh');
const run = (command, args, extraEnv = {}) => spawnSync(command, args, { encoding: 'utf8',
  env: { PATH: process.env.PATH, HOME: os.userInfo().homedir, ...extraEnv },
  maxBuffer: 8 * 1024 * 1024 });
const git = (args, message) => {
  const result = run('git', ['-C', root, ...args]);
  assert.equal(result.status, 0, `${message}: ${result.stderr}`);
  return result.stdout.trim();
};
assert.equal(git(['status', '--porcelain'], 'read clean-tree status'), '',
  'receipt source must be a clean committed tree before measurement');
const sourceCommit = git(['rev-parse', 'HEAD'], 'read source commit');
const repositoryBytes = relative => ({ path: relative,
  sha256: sha(fs.readFileSync(path.join(root, relative))) });
const sourceFiles = [
  'crates/noble-cli/Cargo.toml',
  'crates/noble-cli/src/core/worker.rs',
  'crates/noble-cli/tests/live_slot.rs',
  'crates/noble-cli/tests/live_slot_caller_emits.mjs',
  'crates/noble-cli/tests/live_slot_coherent_maps.mjs',
  'crates/noble-cli/tests/live_slot_relocation_controls.mjs',
  'crates/noble-cli/tests/live_slot_resource_ref_cli.mjs',
  'crates/noble-cli/src/core/runtime/slot-registry.mjs',
  'proofs/foundation/NobleFoundation/LiveDispatch.lean',
  'proofs/foundation/inventory.json',
  'verification/live-slot-coherence/README.md',
  'verification/live-slot-coherence/receipt-2026-11-13.mjs',
].map(repositoryBytes);

const markerPresence = binaries.map(binary => ({
  binary_sha256: binary.sha256,
  present: fs.readFileSync(binary.path).includes(marker),
}));
assert.deepEqual(markerPresence.map(row => row.present), [false, true],
  'only the explicitly feature-built counterfactual CLI may contain the seam marker');

fs.mkdirSync(output, { recursive: true, mode: 0o700 });
const laneTemp = path.join(output, 'tmp');
fs.mkdirSync(laneTemp, { mode: 0o700 });
const save = (name, content) => fs.writeFileSync(path.join(output, name), content,
  { flag: 'wx', mode: 0o600 });
const lanePath = relative => path.join(root, relative);
const runLane = (name, binary, lane, extra = []) => {
  const command = [lanePath(lane), binary.path, ...extra];
  const result = run(process.execPath, command, { TMPDIR: laneTemp });
  save(`${name}.stdout`, result.stdout ?? '');
  save(`${name}.stderr`, result.stderr ?? '');
  assert.equal(result.status, 0, `${name} failed: ${(result.stderr ?? '').slice(-1600)}`);
  const lines = result.stdout.trim().split('\n').filter(Boolean);
  assert.ok(lines.length, `${name} produced no observation JSON`);
  const observed = JSON.parse(lines.at(-1));
  return { lane: path.relative(root, command[0]), argv: ['node', ...command.slice(1)],
    status: result.status, stdout_sha256: sha(Buffer.from(result.stdout)),
    stderr_sha256: sha(Buffer.from(result.stderr)), observed };
};

const callerRelocation = runLane('caller-relocation', binaries[0],
  'crates/noble-cli/tests/live_slot_caller_emits.mjs');
const coherentMaps = runLane('coherent-maps', binaries[0],
  'crates/noble-cli/tests/live_slot_coherent_maps.mjs');
const relocationPinned = runLane('relocation-controls-pinned', binaries[0],
  'crates/noble-cli/tests/live_slot_relocation_controls.mjs');
assert.equal(relocationPinned.observed.mode, 'pinned');
const relocationCurrentMap = runLane('relocation-controls-current-map', binaries[1],
  'crates/noble-cli/tests/live_slot_relocation_controls.mjs', ['current-map']);
assert.equal(relocationCurrentMap.observed.mode, 'current-map');
assert.equal(git(['status', '--porcelain'], 'read final tree status'), '',
  'runtime observations must not dirty the source tree');
fs.rmSync(laneTemp, { recursive: true });

const inventory = JSON.parse(fs.readFileSync(path.join(root, 'proofs/foundation/inventory.json'), 'utf8'));
const modelTheorems = [
  'NobleFoundation.LiveDispatch.coherent_pins_emit_exactly_once',
  'NobleFoundation.LiveDispatch.current_map_selection_duplicates_and_omits',
  'NobleFoundation.LiveDispatch.a_root_pinned_between_sequential_publications_can_mix',
  'NobleFoundation.LiveDispatch.denials_only_truncate',
];
for (const theorem of modelTheorems) {
  assert.ok(inventory.theorems.some(row => row.name === theorem), `model theorem missing: ${theorem}`);
}
const foundationReceipt = 'verification/foundation/receipts/1d4113e6c';
const foundationCheck = JSON.parse(fs.readFileSync(path.join(root, foundationReceipt,
  'foundation-check.json'), 'utf8'));

const receipt = {
  schema: 'noble-live-slot-relocation-receipt/v1',
  measured_on: '2026-11-13',
  source_commit: sourceCommit,
  claim: 'implementation-side live-slot event-relocation observations for issue #47; not an independently proved model/runtime correspondence and not a conformance promotion',
  model: {
    theorems: modelTheorems,
    retained_receipt: { directory: foundationReceipt,
      theorem_count: foundationCheck.theorem_count, clean_source: foundationCheck.clean_source },
    does_not_establish: 'that the selected runtime behaves as modelled',
  },
  implementation_observation: {
    caller_callee_relocation: callerRelocation,
    coherent_map_retention: coherentMaps,
    sequential_publication_mixed_pair: relocationPinned.observed.sequential,
    pinned_dispatch_control: relocationPinned,
    deliberately_broken_current_map_control: relocationCurrentMap,
    does_not_establish: 'a proof that the Lean model corresponds to the JavaScript runtime outside these measured fixtures',
  },
  host_correspondence: {
    binaries,
    test_only_marker: 'noble-test-current-map-seam/v1',
    marker_presence: markerPresence,
    source_files: sourceFiles,
    does_not_establish: 'behavior of other binaries, source revisions, or authority shapes',
  },
  non_claims: [
    'the current-map implementation behavior is measured only in a binary built with the non-default live-test-current-map feature',
    'the default selected CLI excludes the test-only current-map seam marker; release builds must not enable that feature',
    'the three event traces are finite runtime observations, not a universal property or a Lean-to-runtime correspondence proof',
    'no LSLOT conformance case is promoted',
  ],
};
save('receipt.json', `${JSON.stringify(receipt, null, 2)}\n`);
process.stdout.write(`${JSON.stringify({ receipt: path.join(output, 'receipt.json'), source_commit: sourceCommit,
  default_binary: binaries[0].sha256, test_only_binary: binaries[1].sha256,
  caller_relocation: callerRelocation.observed,
  mixed_pair: relocationPinned.observed.sequential,
  current_map_duplicate: relocationCurrentMap.observed.duplicate.held.effects.length,
  current_map_omission: relocationCurrentMap.observed.omission.held.effects.length,
  marker_presence: markerPresence.map(row => row.present) })}\n`);
