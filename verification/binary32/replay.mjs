// Issue #38 phase 2 receipt runner: replay the retained kernel-checked binary32
// expectations through the packed Rust graph reference and record the
// configuration it ran under.
//
// The Lean model is the authority for each expectation. This runner records
// tested-implementation evidence only: it does not prove correspondence, and a
// different target or FP configuration needs its own receipt.
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import {
  LANE_FILE,
  PROPOSAL_FILE,
  binding,
  plain,
  sha256,
  targetMetadata,
  verifyCommon,
} from './receipt-binding.mjs';

const args = process.argv.slice(2);
const rootIndex = args.indexOf('--root');
const root = path.resolve(rootIndex >= 0 ? args[rootIndex + 1] : '.');
const verifyIndex = args.indexOf('--verify');
const outIndex = args.indexOf('--output');
const output = outIndex >= 0 ? path.resolve(args[outIndex + 1]) : path.join(root, 'verification/binary32/replay.json');

if (verifyIndex >= 0) {
  const receiptPath = path.resolve(root, args[verifyIndex + 1]);
  const receipt = JSON.parse(fs.readFileSync(receiptPath, 'utf8'));
  verifyCommon(root, receipt, 'noble-binary32-replay/v1');
  if (receipt.results?.lanes_passed !== receipt.lanes?.length || receipt.results?.retained_expectations_matched !== receipt.retained_expectations) {
    throw new Error('replay result counts do not match recorded lanes');
  }
  console.log(JSON.stringify({ outcome: 'verified', receipt: receiptPath, cargo_rerun: false }));
  process.exit(0);
}

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

const head = plain(['git', 'rev-parse', 'HEAD'], root).trim();
const tree = plain(['git', 'rev-parse', 'HEAD^{tree}'], root).trim();
const dirty = plain(['git', 'status', '--porcelain', '--untracked-files=no'], root).trim().length > 0;
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

const proposalBinding = binding(root);
const receipt = {
  schema: 'noble-binary32-replay/v1',
  assertion: 'the packed Rust reference and the compiled graph legs agree with every retained kernel-checked expectation under observation equality on this host and target; not a proof of correspondence',
  source_commit: head,
  source_tree: tree,
  clean_source: true,
  toolchain,
  host: uname,
  command: 'CARGO_TARGET_DIR=/var/tmp/noble-binary32-replay-target CARGO_INCREMENTAL=0 cargo test -p noble-ngraph-experiment --test b2_differential lean_retained_binary32 -- --nocapture',
  target: targetMetadata(toolchain, { ...process.env, CARGO_INCREMENTAL: '0' }),
  lane_file: LANE_FILE,
  lane_sha256: sha256(root, LANE_FILE),
  proposer_file: 'verification/binary32/propose.py',
  proposer_sha256: proposalBinding.proposer_sha256,
  proposal_file: PROPOSAL_FILE,
  proposals_sha256: proposalBinding.proposals_sha256,
  retained_expectations: proposalBinding.retained_expectations,
  observations: 'NaN expectations match by observation equality; every other class matches its exact encoding',
  lean_authority: proposalBinding.lean_authority,
  lanes: [
    { name: 'packed-reference', test: lanes[0], observed: 'noble_ngraph::sem::add/sub/mul' },
    { name: 'compiled-graph-legs', test: lanes[1], observed: 'Compiled::new + run_legs over L2 APFloat, L3 reference and L4 compiled, with legs agreement' },
  ],
  results: { lanes_passed: lanes.length, retained_expectations_matched: proposalBinding.retained_expectations },
  claims: [
    'Tested-implementation evidence for the packed Rust graph reference on this host and target only.',
    'No correspondence proof, no cross-platform claim, and no evidence about wasm-opt, V8 or the loader.',
  ],
};
fs.mkdirSync(path.dirname(output), { recursive: true });
fs.writeFileSync(output, `${JSON.stringify(receipt, null, 2)}\n`);
console.log(JSON.stringify({ outcome: 'recorded', output, lanes: receipt.lanes.length, retained_expectations: receipt.retained_expectations,
  clean_source: receipt.clean_source, host: receipt.host }));
