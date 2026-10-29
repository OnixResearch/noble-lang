// Issue #38 bounded-shrinking receipt runner: run the shrinker lane and record
// the exact witness it produced, under the configuration it ran under.
//
// The witness is a local minimum under the documented simplicity order. This
// runner records tested-implementation evidence only: the shrinker is not
// kernel evidence, and a witness promoted to evidence must first become a
// retained expectation that the Lean reference checks.
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
const output = outIndex >= 0 ? path.resolve(args[outIndex + 1]) : path.join(root, 'verification/binary32/shrinker.json');

if (verifyIndex >= 0) {
  const receiptPath = path.resolve(root, args[verifyIndex + 1]);
  const receipt = JSON.parse(fs.readFileSync(receiptPath, 'utf8'));
  verifyCommon(root, receipt, 'noble-binary32-shrinker/v2');
  if (receipt.lanes?.length !== 2 || receipt.expectation_witnesses?.length === 0) {
    throw new Error('shrinker receipt lacks its recorded lanes or witnesses');
  }
  console.log(JSON.stringify({ outcome: 'verified', receipt: receiptPath, cargo_rerun: false }));
  process.exit(0);
}

const laneFile = 'crates/noble-ngraph-experiment/tests/b2_differential.rs';
const lanes = [
  'flush_to_zero_disagreement_shrinks_to_a_deterministic_replayable_witness',
  'retained_expectation_discrepancies_shrink_and_preserve_their_classes',
];
const command = 'CARGO_TARGET_DIR=/var/tmp/noble-binary32-shrink-target CARGO_INCREMENTAL=0 ' +
  `cargo test -p noble-ngraph-experiment --test b2_differential shrink -- --nocapture`;

// The toolchain and lane output come from stdout alone: stderr inside `nix develop` can carry
// store garbage-collection progress, which must never land in a receipt field.
function run(argv, cwd) {
  const result = spawnSync('nix', ['develop', '--command', ...argv], {
    cwd, encoding: 'utf8', timeout: 1800_000, maxBuffer: 64 * 1024 * 1024,
  });
  if (result.status !== 0) {
    const text = `${result.stdout ?? ''}${result.stderr ?? ''}`;
    throw new Error(`command failed (${result.status}): ${argv.join(' ')}\n${text.slice(-2000)}`);
  }
  return result.stdout ?? '';
}

const head = plain(['git', 'rev-parse', 'HEAD'], root).trim();
const tree = plain(['git', 'rev-parse', 'HEAD^{tree}'], root).trim();
// A witness recorded from a dirty tree is not attributable to a commit, so the
// runner refuses instead of recording a soft warning nobody reads.
if (plain(['git', 'status', '--porcelain', '--untracked-files=no'], root).trim().length > 0) {
  throw new Error('shrinker receipt requires a clean tree: commit the lane before recording');
}
const toolchain = run(['rustc', '-vV'], root).trim();
// `uname` needs no devShell, and `run` would fold store/GC progress into the recorded
// host string.
const host = plain(['uname', '-srm'], root).trim();
// Inside `nix develop` the host profile's `bash` wrapper replaces PATH with the host
// profile's, which carries no cargo (exit 127); `sh -c` keeps the devShell PATH.
const testRun = run(['sh', '-c', command], root);
for (const lane of lanes) {
  if (!new RegExp(`test ${lane} \\.\.\. ok`).test(testRun)) {
    throw new Error(`shrinker lane did not pass: ${lane}\n${testRun.slice(-2000)}`);
  }
}
const parse = (line) => Object.fromEntries(
  line.trim().split(' ').slice(1).map((pair) => {
    const [key, value] = pair.split('=');
    return [key, value];
  }),
);
const encodings = ['left', 'right', 'expected', 'candidate'];
const checkWitness = (fields, line, keys) => {
  for (const key of keys) {
    if (fields[key] === undefined) throw new Error(`witness line lacks ${key}: ${line}`);
  }
  for (const key of keys.filter((key) => encodings.includes(key))) {
    const value = Number.parseInt(fields[key], 16);
    if (!Number.isInteger(value) || value < 0 || value > 0xFFFF_FFFF) {
      throw new Error(`witness ${key} is not a binary32 encoding: ${fields[key]}`);
    }
  }
  if (fields.exhausted !== 'false') {
    throw new Error(`witness walk exhausted its budget, so it is not a local minimum: ${line}`);
  }
};

const lines = testRun.split('\n').filter((candidate) => candidate.startsWith('shrink-witness '));
if (lines.length === 0) throw new Error(`shrinker lane printed no witness line\n${testRun.slice(-2000)}`);
const witnesses = lines.map((line) => {
  const fields = parse(line);
  checkWitness(fields, line, ['operation', 'left', 'right', 'expected', 'candidate', 'attempts', 'exhausted']);
  return {
    operation: Number(fields.operation),
    left: fields.left,
    right: fields.right,
    expected: fields.expected,
    candidate: fields.candidate,
    attempts: Number(fields.attempts),
    budget_exhausted: false,
  };
});

const expectationLines = testRun.split('\n')
  .filter((candidate) => candidate.startsWith('expectation-shrink-witness '));
if (expectationLines.length === 0) {
  throw new Error(`the expectation-driven lane printed no witness line\n${testRun.slice(-2000)}`);
}
const classes = ['zero', 'subnormal', 'normal', 'infinite', 'nan'];
const expectationWitnesses = expectationLines.map((line) => {
  const fields = parse(line);
  checkWitness(fields, line, [
    'expectation', 'operation', 'retained', 'class', 'left', 'right', 'expected', 'candidate',
    'attempts', 'exhausted',
  ]);
  const retained = Number.parseInt(fields.retained, 16);
  if (!Number.isInteger(retained) || retained < 0 || retained > 0xFFFF_FFFF) {
    throw new Error(`retained expectation is not a binary32 encoding: ${fields.retained}`);
  }
  if (!classes.includes(fields.class)) {
    throw new Error(`unexpected observation class: ${fields.class}`);
  }
  return {
    expectation_index: Number(fields.expectation),
    operation: Number(fields.operation),
    retained: fields.retained,
    class: fields.class,
    left: fields.left,
    right: fields.right,
    expected: fields.expected,
    candidate: fields.candidate,
    attempts: Number(fields.attempts),
    budget_exhausted: false,
  };
});

const countLine = testRun.split('\n').find((candidate) => candidate.startsWith('expectation-shrink-count='));
if (!countLine) throw new Error(`no expectation-shrink count line\n${testRun.slice(-2000)}`);
// The count line's first token is the count itself, so it is parsed whole rather
// than by `parse` (which drops the leading token of a witness line).
const countFields = Object.fromEntries(
  countLine.trim().split(' ').map((pair) => {
    const [key, value] = pair.split('=');
    return [key, value];
  }),
);
if (Number(countFields['expectation-shrink-count']) !== expectationWitnesses.length) {
  throw new Error(`expectation witness count disagrees with the lane: ${countLine}`);
}

const proposalBinding = binding(root);

const receipt = {
  schema: 'noble-binary32-shrinker/v2',
  assertion: 'bare operation disagreements and retained-expectation discrepancies shrink, deterministically, inside their documented budget and without changing observation class, to the recorded witnesses on this host and target; not kernel evidence',
  lanes,
  source_commit: head,
  source_tree: tree,
  clean_source: true,
  toolchain,
  host,
  command,
  target: targetMetadata(toolchain, { ...process.env, CARGO_INCREMENTAL: '0' }),
  lane_file: LANE_FILE,
  lane_sha256: sha256(root, LANE_FILE),
  proposer_file: 'verification/binary32/propose.py',
  proposer_sha256: proposalBinding.proposer_sha256,
  proposal_file: PROPOSAL_FILE,
  proposals_sha256: proposalBinding.proposals_sha256,
  retained_expectations: proposalBinding.retained_expectations,
  lean_authority: proposalBinding.lean_authority,
  witnesses,
  expectation_witnesses: expectationWitnesses,
  carried_expectations: Number(countFields.carried),
  scope: {
    order: 'documented simplicity order over the documented simplification list',
    minimum: 'local minimum under that order, not a global minimum',
    reachability_limit: 'pairs that need a simplification outside the documented list are reported unchanged rather than invented',
  },
  claims: [
    'Tested-implementation evidence for the shrinker lane on this host and target only.',
    'Every witness is replayable and deterministic; none is a retained kernel-checked expectation.',
    'The expectation-driven witnesses name the retained expectation they came from (packet index and class) and preserve that class, so a shrink cannot convert a subnormal rounding discrepancy into another class.',
    'The expectation-driven lane carries multiplication only: the additive and subtractive retained expectations have no flush-to-zero candidate and stay covered by their own replay lanes, not by this walk.',
    'No correspondence proof, no cross-platform claim, and no claim about wasm-opt, V8 or the loader.',
  ],
};
fs.mkdirSync(path.dirname(output), { recursive: true });
fs.writeFileSync(output, `${JSON.stringify(receipt, null, 2)}\n`);
console.log(JSON.stringify({
  outcome: 'recorded',
  output,
  witnesses: witnesses.length,
  expectation_witnesses: expectationWitnesses.length,
  carried_expectations: receipt.carried_expectations,
  host,
}));
