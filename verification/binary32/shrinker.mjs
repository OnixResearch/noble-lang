// Issue #38 bounded-shrinking receipt runner: run the shrinker lane and record
// the exact witness it produced, under the configuration it ran under.
//
// The witness is a local minimum under the documented simplicity order. This
// runner records tested-implementation evidence only: the shrinker is not
// kernel evidence, and a witness promoted to evidence must first become a
// retained expectation that the Lean reference checks.
import { spawnSync } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';

const args = process.argv.slice(2);
const rootIndex = args.indexOf('--root');
const root = path.resolve(rootIndex >= 0 ? args[rootIndex + 1] : '.');
const outIndex = args.indexOf('--output');
const output = outIndex >= 0 ? path.resolve(args[outIndex + 1]) : path.join(root, 'verification/binary32/shrinker.json');

const laneFile = 'crates/noble-ngraph-experiment/tests/b2_differential.rs';
const lane = 'flush_to_zero_disagreement_shrinks_to_a_deterministic_replayable_witness';
const command = 'CARGO_TARGET_DIR=/var/tmp/noble-binary32-shrink-target CARGO_INCREMENTAL=0 ' +
  `cargo test -p noble-ngraph-experiment --test b2_differential shrink -- --nocapture`;

function run(argv, cwd) {
  const result = spawnSync('nix', ['develop', '--command', ...argv], {
    cwd, encoding: 'utf8', timeout: 1800_000, maxBuffer: 64 * 1024 * 1024,
  });
  const text = `${result.stdout ?? ''}${result.stderr ?? ''}`;
  if (result.status !== 0) throw new Error(`command failed (${result.status}): ${argv.join(' ')}\n${text.slice(-2000)}`);
  return text;
}

const sha256 = (file) => crypto.createHash('sha256').update(fs.readFileSync(path.join(root, file))).digest('hex');

const head = run(['git', 'rev-parse', 'HEAD'], root).trim();
const tree = run(['git', 'rev-parse', 'HEAD^{tree}'], root).trim();
// A witness recorded from a dirty tree is not attributable to a commit, so the
// runner refuses instead of recording a soft warning nobody reads.
if (run(['git', 'status', '--porcelain', '--untracked-files=no'], root).trim().length > 0) {
  throw new Error('shrinker receipt requires a clean tree: commit the lane before recording');
}
const toolchain = run(['rustc', '-vV'], root).trim();
const host = run(['uname', '-srm'], root).trim();
const testRun = run(['bash', '-c', command], root);
if (!new RegExp(`test ${lane} \\.\.\. ok`).test(testRun)) {
  throw new Error(`shrinker lane did not pass: ${lane}\n${testRun.slice(-2000)}`);
}
const line = testRun.split('\n').find((candidate) => candidate.startsWith('shrink-witness '));
if (!line) throw new Error(`shrinker lane printed no witness line\n${testRun.slice(-2000)}`);
const fields = Object.fromEntries(line.trim().split(' ').slice(1).map((pair) => {
  const [key, value] = pair.split('=');
  return [key, value];
}));
for (const key of ['operation', 'left', 'right', 'expected', 'candidate', 'attempts', 'exhausted']) {
  if (fields[key] === undefined) throw new Error(`witness line lacks ${key}: ${line}`);
}
const encodings = ['left', 'right', 'expected', 'candidate'].map((key) => [key, Number.parseInt(fields[key], 16)]);
for (const [key, value] of encodings) {
  if (!Number.isInteger(value) || value < 0 || value > 0xFFFF_FFFF) throw new Error(`witness ${key} is not a binary32 encoding: ${fields[key]}`);
}
if (fields.exhausted !== 'false') throw new Error(`witness walk exhausted its budget, so it is not a local minimum: ${line}`);

const receipt = {
  schema: 'noble-binary32-shrinker/v1',
  assertion: 'the flush-to-zero disagreement between the host multiply and the packed multiplier shrinks, deterministically and inside its documented budget, to the recorded witness on this host and target; not kernel evidence',
  source_commit: head,
  source_tree: tree,
  clean_source: true,
  toolchain,
  host,
  command,
  lane_file: laneFile,
  lane_sha256: sha256(laneFile),
  lane,
  witness: {
    operation: Number(fields.operation),
    left: fields.left,
    right: fields.right,
    expected: fields.expected,
    candidate: fields.candidate,
    attempts: Number(fields.attempts),
    budget_exhausted: false,
  },
  scope: {
    order: 'documented simplicity order over the documented simplification list',
    minimum: 'local minimum under that order, not a global minimum',
    reachability_limit: 'pairs that need a simplification outside the documented list are reported unchanged rather than invented',
  },
  claims: [
    'Tested-implementation evidence for the shrinker lane on this host and target only.',
    'The witness is replayable and deterministic; it is not a retained kernel-checked expectation.',
    'No correspondence proof, no cross-platform claim, and no claim about wasm-opt, V8 or the loader.',
  ],
};
fs.mkdirSync(path.dirname(output), { recursive: true });
fs.writeFileSync(output, `${JSON.stringify(receipt, null, 2)}\n`);
console.log(JSON.stringify({ outcome: 'recorded', output, witness: receipt.witness, host }));
