// Issue #48 differential receipt runner: run the Rust comparison lane against
// the independent case file and record the configuration, coverage counts and
// the case-file digest it ran under.
//
// The expectations come from `verification/service-ref/cases.mjs`, an
// independent decoder written from the Lean model; this runner never
// regenerates them from the codec under test.
import { spawnSync } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';

const args = process.argv.slice(2);
const rootIndex = args.indexOf('--root');
const root = path.resolve(rootIndex >= 0 ? args[rootIndex + 1] : '.');
const outIndex = args.indexOf('--output');
const output = outIndex >= 0 ? path.resolve(args[outIndex + 1]) : path.join(root, 'verification/service-ref/differential.json');

const caseFile = 'verification/service-ref/cases.txt';
const referenceFile = 'verification/service-ref/cases.mjs';
const laneFile = 'crates/noble-kernel/tests/m7-dataspace.rs';
const lanes = [
  'service_ref_differential::service_ref_differential_matches_the_codec',
  'service_ref_differential::service_ref_round_trip_through_the_rust_encoder_holds',
];
const command = 'CARGO_TARGET_DIR=/var/tmp/noble-service-ref-target CARGO_INCREMENTAL=0 ' +
  'cargo test -p noble-kernel --test m7-dataspace service_ref_differential -- --nocapture';

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
if (run(['git', 'status', '--porcelain', '--untracked-files=no'], root).trim().length > 0) {
  throw new Error('differential receipt requires a clean tree: commit the lane before recording');
}
const toolchain = run(['rustc', '-vV'], root).trim();
const host = run(['uname', '-srm'], root).trim();
const laneRun = run(['bash', '-c', command], root);
for (const lane of lanes) {
  if (!new RegExp(`test ${lane} \\.\.\. ok`).test(laneRun)) {
    throw new Error(`differential lane did not pass: ${lane}\n${laneRun.slice(-4000)}`);
  }
}
const summary = laneRun.split('\n').find((line) => line.startsWith('service-ref-differential '));
if (!summary) throw new Error(`differential lane printed no coverage line\n${laneRun.slice(-4000)}`);
const cases = Number(/cases=(\d+)/u.exec(summary)?.[1]);
const divergence = Number(/divergence=(\d+)/u.exec(summary)?.[1]);
if (!Number.isInteger(cases) || cases === 0) throw new Error(`coverage line lacks a case count: ${summary}`);
if (!Number.isInteger(divergence)) throw new Error(`coverage line lacks a divergence count: ${summary}`);

const receipt = {
  schema: 'noble-service-ref-differential/v1',
  assertion: 'the Rust ServiceRef codec matches every independent expectation generated from the Lean model on this host and target, except for the one documented bracket-scan divergence; not a refinement proof',
  source_commit: head,
  source_tree: tree,
  clean_source: true,
  toolchain,
  host,
  command,
  lanes,
  case_file: caseFile,
  case_file_sha256: sha256(caseFile),
  reference_file: referenceFile,
  reference_file_sha256: sha256(referenceFile),
  lane_file: laneFile,
  lane_file_sha256: sha256(laneFile),
  coverage: { cases, documented_divergences: divergence, summary: summary.trim() },
  correspondence: {
    route: 'handwritten Lean reference compared against the Rust functions by this differential lane',
    model_theorems: 'proofs/foundation/receipts/b25314b1/foundation-check.json',
    open_edge: 'no extraction or refinement proof relates crates/noble-kernel/src/dataspace/wire.rs to the Lean model; the differential is finite evidence on one host and target',
  },
  claims: [
    'Tested-implementation evidence for the Rust ServiceRef codec against a specification written down independently of it.',
    'The model-to-Rust edge stays open: no refinement proof, no cross-platform claim.',
    'The model counts opening brackets for its depth scan; the documented divergence case records where that is stricter than the Rust scanner.',
  ],
};
fs.mkdirSync(path.dirname(output), { recursive: true });
fs.writeFileSync(output, `${JSON.stringify(receipt, null, 2)}\n`);
console.log(JSON.stringify({ outcome: 'recorded', output, coverage: receipt.coverage, host }));
