// Issue #45 differential receipt runner: run the Rust gradient comparison lane against the corpus
// generated from the ordered binary32 reference, and record the configuration, coverage counts and
// the digests it ran under.
//
// The expectations come from `verification/foundation/grad-ordered/corpus.lean`, which prints the
// forward values and gradients of `NobleFoundation.ReverseAD.Ordered`; this runner never regenerates
// them from the code under test.
import { spawnSync } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { receiptsBindingModel } from '../foundation/model-receipts.mjs';

const args = process.argv.slice(2);
const rootIndex = args.indexOf('--root');
const root = path.resolve(rootIndex >= 0 ? args[rootIndex + 1] : '.');
const outIndex = args.indexOf('--output');
const output = outIndex >= 0 ? path.resolve(args[outIndex + 1]) : path.join(root, 'verification/grad-ordered/differential.json');

const corpusFile = 'crates/noble-ngraph/tests/grad_ordered/corpus.txt';
const generatorFile = 'verification/foundation/grad-ordered/corpus.lean';
const modelFile = 'proofs/foundation/NobleFoundation/ReverseADOrdered.lean';
const binary32File = 'proofs/foundation/NobleFoundation/Binary32.lean';
const laneFile = 'crates/noble-ngraph/tests/grad_ordered/differential.rs';
const subjectFile = 'crates/noble-ngraph/src/grad.rs';
const lanes = [
  'grad_ordered::ordered_gradients_match_the_lean_reference',
  'grad_ordered::ordered_gradient_corpus_detects_each_seeded_fault',
];
const command = 'CARGO_TARGET_DIR=/var/tmp/noble-grad-ordered-target CARGO_INCREMENTAL=0 ' +
  'cargo test -p noble-ngraph --test train_admission grad_ordered -- --nocapture';

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

// Host tools (git, uname) are called directly: inside `nix develop` a store auto-GC prints progress
// lines into the captured text, which would read as a dirty tree.
function runHost(argv, cwd) {
  const result = spawnSync(argv[0], argv.slice(1), { cwd, encoding: 'utf8', timeout: 120_000 });
  if (result.status !== 0) {
    throw new Error(`host command failed (${result.status}): ${argv.join(' ')}\n${`${result.stdout ?? ''}${result.stderr ?? ''}`.slice(-2000)}`);
  }
  return result.stdout ?? '';
}

const sha256 = (file) => crypto.createHash('sha256').update(fs.readFileSync(path.join(root, file))).digest('hex');

const head = runHost(['git', 'rev-parse', 'HEAD'], root).trim();
const tree = runHost(['git', 'rev-parse', 'HEAD^{tree}'], root).trim();
if (runHost(['git', 'status', '--porcelain', '--untracked-files=no'], root).trim().length > 0) {
  throw new Error('differential receipt requires a clean tree: commit the lane before recording');
}
const toolchain = run(['rustc', '-vV'], root).trim();
const host = runHost(['uname', '-srm'], root).trim();
// `sh -c`, not `bash -c`: inside `nix develop` a nested bash is the host profile wrapper and
// replaces PATH, so cargo would be missing (exit 127).
const laneRun = run(['sh', '-c', command], root);
for (const lane of lanes) {
  if (!new RegExp(`test ${lane} \\.\\.\\. ok`).test(laneRun)) {
    throw new Error(`differential lane did not pass: ${lane}\n${laneRun.slice(-4000)}`);
  }
}
const summary = laneRun.split('\n').find((line) => line.startsWith('grad-ordered-differential '));
if (!summary) throw new Error(`differential lane printed no coverage line\n${laneRun.slice(-4000)}`);
const mutations = laneRun.split('\n').find((line) => line.startsWith('grad-ordered-mutations '));
if (!mutations) throw new Error(`differential lane printed no mutation line\n${laneRun.slice(-4000)}`);
const count = (line, key) => {
  const value = Number(new RegExp(`${key}=(\\d+)`, 'u').exec(line)?.[1]);
  if (!Number.isInteger(value)) throw new Error(`coverage line lacks ${key}: ${line}`);
  return value;
};
const cases = count(summary, 'cases');
const declared = Number(/^# cases: (\d+)$/mu.exec(fs.readFileSync(path.join(root, corpusFile), 'utf8'))?.[1]);
if (cases === 0 || cases !== declared) throw new Error(`lane compared ${cases} cases; the corpus declares ${declared}`);
const faults = Object.fromEntries(mutations.trim().split(' ').slice(2).map((pair) => {
  const [name, value] = pair.split('=');
  return [name, Number(value)];
}));
if (Object.values(faults).some((value) => !(value > 0))) throw new Error(`a seeded fault went undetected: ${mutations}`);

const receipt = {
  schema: 'noble-grad-ordered-differential/v1',
  assertion: 'grad::forward and grad::backward for f32 match every forward value and gradient the ordered binary32 Lean reference generates, on this host and target, with NaN encodings one observation class; each declared single-rule reverse-pass fault disagrees with at least one expectation; not a refinement proof',
  source_commit: head,
  source_tree: tree,
  clean_source: true,
  toolchain,
  host,
  command,
  lanes,
  corpus_file: corpusFile,
  corpus_file_sha256: sha256(corpusFile),
  generator_file: generatorFile,
  generator_file_sha256: sha256(generatorFile),
  model_file: modelFile,
  model_file_sha256: sha256(modelFile),
  binary32_file: binary32File,
  binary32_file_sha256: sha256(binary32File),
  lane_file: laneFile,
  lane_file_sha256: sha256(laneFile),
  subject_file: subjectFile,
  subject_file_sha256: sha256(subjectFile),
  coverage: {
    cases,
    values: count(summary, 'values'),
    gradients: count(summary, 'grads'),
    nonfinite_cases: count(summary, 'nonfinite'),
    repeated_operand_cases: count(summary, 'repeated-operand'),
    faults_detected_by_cases: faults,
    summary: summary.trim(),
    mutations: mutations.trim(),
  },
  correspondence: {
    route: 'Lean-generated corpus from the ordered binary32 reference compared against the Rust gradient functions by this differential lane',
    model_theorems: receiptsBindingModel(root, [modelFile], 'NobleFoundation.ReverseAD.Ordered'),
    open_edge: 'no extraction or refinement proof relates crates/noble-ngraph/src/grad.rs to the Lean model; the differential is finite evidence on one host and target',
  },
  claims: [
    'Tested-implementation evidence for grad.rs forward and backward over inputs, parameters, constants, addition, subtraction and multiplication in binary32.',
    'The model-to-Rust edge stays open: no refinement proof, no cross-platform or cross-compiler claim.',
    'Each declared fault is the ordered reverse pass with one rule changed, simulated over the corpus reference values; they are seeded faults, not an exhaustive fault model.',
    'The fused multiply-add fault is detected by one case only and the presummed repeated-operand fault by few cases; detection counts are recorded, not assumed.',
  ],
};
fs.mkdirSync(path.dirname(output), { recursive: true });
fs.writeFileSync(output, `${JSON.stringify(receipt, null, 2)}\n`);
console.log(JSON.stringify({ outcome: 'recorded', output, coverage: receipt.coverage, host }));
