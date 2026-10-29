// Issue #59 differential receipt runner: run the ownership corpus lane against
// the independent case file and record the configuration, coverage counts and
// the case-file digest it ran under.
//
// The expectations come from `verification/ownership/cases.mjs`, written from
// the Lean model in `proofs/foundation/NobleFoundation/Ownership.lean`; this
// runner never regenerates them from the production acceptance machine.
import { spawnSync } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { receiptsBindingModel } from '../foundation/model-receipts.mjs';

const args = process.argv.slice(2);
const rootIndex = args.indexOf('--root');
const root = path.resolve(rootIndex >= 0 ? args[rootIndex + 1] : '.');
const outIndex = args.indexOf('--output');
const output = outIndex >= 0
  ? path.resolve(args[outIndex + 1])
  : path.join(root, 'verification/ownership/differential.json');

const caseFile = 'verification/ownership/cases.txt';
const referenceFile = 'verification/ownership/cases.mjs';
const modelFile = 'proofs/foundation/NobleFoundation/Ownership.lean';
const laneFile = 'crates/noble-kernel/tests/acceptance/ownership.rs';
const lanes = [
  'ownership::ownership_corpus_matches_the_acceptance_machine',
  'ownership::ownership_corpus_detects_each_declared_checker_mutation',
];
const command = 'CARGO_TARGET_DIR=/var/tmp/noble-ownership-target CARGO_INCREMENTAL=0 ' +
  'cargo test -p noble-kernel --test acceptance ownership -- --nocapture';

// Identity fields and parsed lane output come from stdout alone: inside `nix develop` an automatic
// store garbage collection prints progress to stderr, and that must never land in a receipt field.
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

const sha256 = file =>
  crypto.createHash('sha256').update(fs.readFileSync(path.join(root, file))).digest('hex');

const head = run(['git', 'rev-parse', 'HEAD'], root).trim();
const tree = run(['git', 'rev-parse', 'HEAD^{tree}'], root).trim();
const dirt = run(['git', 'status', '--porcelain', '--untracked-files=no'], root)
  .split('\n')
  .filter(line => /^(?:[ MADRCU?!]{2}) \S/u.test(line) || /^\?\? /u.test(line));
if (dirt.length > 0) {
  throw new Error(`differential receipt requires a clean tracked tree: ${dirt.join('; ')}`);
}
const toolchain = run(['rustc', '-vV'], root).trim();
const host = run(['uname', '-srm'], root).trim().split('\n')[0];
const laneRun = run(['bash', '-c', command], root);
for (const lane of lanes) {
  if (!new RegExp(`test ${lane} \\.\.\. ok`).test(laneRun)) {
    throw new Error(`ownership lane did not pass: ${lane}\n${laneRun.slice(-4000)}`);
  }
}
const summary = laneRun.split('\n').find(line => line.startsWith('ownership-differential '));
if (!summary) throw new Error(`ownership lane printed no coverage line\n${laneRun.slice(-4000)}`);
const mutations = laneRun.split('\n').find(line => line.startsWith('ownership-mutations '));
if (!mutations) throw new Error(`ownership lane printed no mutation line\n${laneRun.slice(-4000)}`);

const count = (pattern, text) => {
  const value = Number(pattern.exec(text)?.[1]);
  if (!Number.isInteger(value)) throw new Error(`line lacks a count: ${text}`);
  return value;
};
const cases = count(/cases=(\d+)/u, summary);
const accepted = count(/accepted=(\d+)/u, summary);
const rejected = count(/rejected=(\d+)/u, summary);
const modelOnly = count(/model-only=(\d+)/u, summary);
const eligibility = count(/eligibility:(\d+)/u, summary);
const stackJoin = count(/stack-join:(\d+)/u, summary);
const otherRejection = count(/other:(\d+)/u, summary);
if (cases !== accepted + rejected + modelOnly) {
  throw new Error(`coverage line does not account for every case: ${summary}`);
}
if (eligibility + stackJoin + otherRejection !== rejected) {
  throw new Error(`rejection categories do not account for every rejected case: ${summary}`);
}

const receipt = {
  schema: 'noble-ownership-differential/v1',
  assertion: 'the unary ownership-accounting fragment of the production acceptance machine agrees with every independent expectation generated from the Lean model on this host and target; the shadow checker agrees with every expectation and each declared mutation of it is caught. Not a refinement proof.',
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
  model_file: modelFile,
  model_file_sha256: sha256(modelFile),
  lane_file: laneFile,
  lane_file_sha256: sha256(laneFile),
  coverage: {
    cases,
    accepted,
    rejected,
    model_only: modelOnly,
    rejection_categories: { eligibility, stack_join: stackJoin, other: otherRejection },
    summary: summary.trim(),
    mutations: mutations.trim(),
  },
  correspondence: {
    route: 'handwritten Lean reference compared against the production acceptance machine and a test-local shadow checker by this differential lane',
    model_theorems: receiptsBindingModel(root, [modelFile], 'NobleFoundation.Ownership'),
    production_analogue: 'dup and drop sequences over nameable operand payloads, driven through noble_kernel::acceptance::check',
    open_edge: 'no extraction or refinement proof relates crates/noble-kernel/src/acceptance to the Lean model; the differential is finite evidence on one host and target',
  },
  claims: [
    'Tested-implementation evidence that the unary fragment of the acceptance machine agrees with a specification written down independently of it.',
    'Model-only cases (wrapped payloads, the two-operand swap slot ordering, the production quotation constructor, two-arm accounting and the ordinary error return) are named and counted, never reported as executable production controls.',
    'The production rejection category of a resource payload can be a stack-join failure rather than eligibility; the lane asserts the agreement of the accounting decision and reports the category split.',
    'The model-to-Rust edge stays open: no refinement proof, no cross-platform claim, no claim about release of native objects.',
  ],
};
fs.mkdirSync(path.dirname(output), { recursive: true });
fs.writeFileSync(output, `${JSON.stringify(receipt, null, 2)}\n`);
console.log(JSON.stringify({ outcome: 'recorded', output, coverage: receipt.coverage, host }));
