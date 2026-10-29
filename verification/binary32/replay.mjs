// Issue #38 phase 2 receipt runner: replay the retained kernel-checked binary32
// expectations through the packed Rust graph reference and record the
// configuration it ran under.
//
// The Lean model is the authority for each expectation. This runner records
// tested-implementation evidence only: it does not prove correspondence, and a
// different target or FP configuration needs its own receipt.
import { spawnSync } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { receiptsBindingModel } from '../foundation/model-receipts.mjs';

const args = process.argv.slice(2);
const rootIndex = args.indexOf('--root');
const root = path.resolve(rootIndex >= 0 ? args[rootIndex + 1] : '.');
const outIndex = args.indexOf('--output');
const output = outIndex >= 0 ? path.resolve(args[outIndex + 1]) : path.join(root, 'verification/binary32/replay.json');

// Identity fields and lane output come from stdout alone: stderr inside `nix develop` can carry
// store garbage-collection progress, which must never land in a receipt field.
function run(command, cwd) {
  const result = spawnSync('nix', ['develop', '--command', 'bash', '-c', command], {
    cwd, encoding: 'utf8', timeout: 1800_000, maxBuffer: 64 * 1024 * 1024,
  });
  if (result.status !== 0) {
    const text = `${result.stdout ?? ''}${result.stderr ?? ''}`;
    throw new Error(`command failed (${result.status}): ${command}\n${text.slice(-2000)}`);
  }
  return result.stdout ?? '';
}

const sha256 = (file) => crypto.createHash('sha256').update(fs.readFileSync(path.join(root, file))).digest('hex');

const head = run('git rev-parse HEAD', root).trim();
const tree = run('git rev-parse HEAD^{tree}', root).trim();
const dirty = run('git status --porcelain --untracked-files=no', root).trim().length > 0;
if (dirty) throw new Error('replay receipt requires a clean tracked tree: commit the lane before recording');
const toolchain = run('rustc -vV', root).trim();
const uname = run('uname -srm', root).trim();
const testRun = run(
  'CARGO_TARGET_DIR=/var/tmp/noble-binary32-replay-target CARGO_INCREMENTAL=0 ' +
    'cargo test -p noble-ngraph-experiment --test b2_differential lean_retained_binary32 -- --nocapture',
  root,
);
const lanes = [
  'lean_retained_binary32_expectations_replay_through_the_graph_reference',
  'lean_retained_binary32_expectations_replay_through_the_compiled_graph_legs',
];
for (const lane of lanes) {
  if (!new RegExp(`test ${lane} \\.\\.\\. ok`).test(testRun)) {
    throw new Error(`replay lane did not pass: ${lane}\n${testRun.slice(-2000)}`);
  }
}

const proposals = JSON.parse(fs.readFileSync(path.join(root, 'verification/binary32/proposals.json'), 'utf8'));
const receipt = {
  schema: 'noble-binary32-replay/v1',
  assertion: 'the packed Rust reference and the compiled graph legs agree with every retained kernel-checked expectation under observation equality on this host and target; not a proof of correspondence',
  source_commit: head,
  source_tree: tree,
  clean_source: true,
  toolchain,
  host: uname,
  command: 'CARGO_TARGET_DIR=/var/tmp/noble-binary32-replay-target CARGO_INCREMENTAL=0 cargo test -p noble-ngraph-experiment --test b2_differential lean_retained_binary32 -- --nocapture',
  lane_file: 'crates/noble-ngraph-experiment/tests/b2_differential.rs',
  lane_sha256: sha256('crates/noble-ngraph-experiment/tests/b2_differential.rs'),
  proposals_sha256: sha256('verification/binary32/proposals.json'),
  retained_expectations: proposals.proposals.length,
  observations: 'NaN expectations match by observation equality; every other class matches its exact encoding',
  lean_authority: receiptsBindingModel(root, [
    'proofs/foundation/NobleFoundation/Binary32.lean',
    'proofs/foundation/NobleFoundation/Binary32Expectations.lean',
  ], 'NobleFoundation.Binary32'),
  lanes: [
    { name: 'packed-reference', test: lanes[0], observed: 'noble_ngraph::sem::add/sub/mul' },
    { name: 'compiled-graph-legs', test: lanes[1], observed: 'Compiled::new + run_legs over L2 APFloat, L3 reference and L4 compiled, with legs agreement' },
  ],
  results: { lanes_passed: lanes.length, retained_expectations_matched: proposals.proposals.length },
  claims: [
    'Tested-implementation evidence for the packed Rust graph reference on this host and target only.',
    'No correspondence proof, no cross-platform claim, and no evidence about wasm-opt, V8 or the loader.',
  ],
};
fs.mkdirSync(path.dirname(output), { recursive: true });
fs.writeFileSync(output, `${JSON.stringify(receipt, null, 2)}\n`);
console.log(JSON.stringify({ outcome: 'recorded', output, lanes: receipt.lanes.length, retained_expectations: receipt.retained_expectations,
  clean_source: receipt.clean_source, host: receipt.host }));
