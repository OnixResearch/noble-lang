// Issue #59 differential receipt runner: run the ownership corpus lane against
// the independent case file and record the configuration, coverage counts and
// the digests it ran under.
//
// The expectations come from a JavaScript mirror in `verification/ownership/cases.mjs`
// of the Lean model in `proofs/foundation/NobleFoundation/Ownership.lean` and
// `OwnershipTrace.lean`. The same generator emits
// `proofs/foundation/NobleFoundation/OwnershipCorpus.lean`, where the kernel
// decides `run` and `runTrace` for every row; this runner refuses to record a
// receipt when the committed corpus differs from the emission, and the Rust lane
// compares every corpus row with its kernel-checked twin.
//
// This runner never regenerates expectations from the production acceptance
// machine, and it is not a refinement proof.
//
// Usage:
//   node verification/ownership/differential.mjs \
//     [--root .] [--output FILE] [--cargo CMD] [--rustc CMD]
//
// `--cargo` names the cargo executable; the campaign passes its pinned wrapper,
// which supplies the offline vendor, the toolchain and the build slot.
import { spawnSync } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { receiptsBindingModel } from '../foundation/model-receipts.mjs';

const args = process.argv.slice(2);
const option = (name, fallback) => {
  const index = args.indexOf(`--${name}`);
  return index >= 0 ? args[index + 1] : fallback;
};
const root = path.resolve(option('root', '.'));
const cargo = option('cargo', 'cargo');
const rustc = option('rustc', 'rustc');

const caseFile = 'verification/ownership/cases.txt';
const referenceFile = 'verification/ownership/cases.mjs';
const modelFile = 'proofs/foundation/NobleFoundation/Ownership.lean';
const traceFile = 'proofs/foundation/NobleFoundation/OwnershipTrace.lean';
const corpusFile = 'proofs/foundation/NobleFoundation/OwnershipCorpus.lean';
const laneFile = 'crates/noble-kernel/tests/acceptance/ownership.rs';
const lanes = [
  'ownership::ownership_corpus_matches_the_acceptance_machine',
  'ownership::ownership_corpus_detects_each_declared_checker_mutation',
  'ownership::ownership_minimiser_finds_shortest_failing_prefix',
];
const command = 'cargo test -p noble-kernel --test acceptance ownership --offline --locked -- --nocapture';

// Identity fields and parsed lane output come from stdout alone: a compiler or
// store notice printed to stderr must never land in a receipt field.
function run(argv, cwd) {
  const result = spawnSync(argv[0], argv.slice(1), {
    cwd,
    encoding: 'utf8',
    timeout: 3_600_000,
    maxBuffer: 256 * 1024 * 1024,
    env: process.env,
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
const dirt = run(['git', 'status', '--porcelain'], root)
  .split('\n')
  .filter(line => line.trim() !== '');
if (dirt.length > 0) {
  throw new Error(`differential receipt requires a clean tracked tree: ${dirt.join('; ')}`);
}
const output = path.resolve(
  option(
    'output',
    path.join(os.homedir(), '.cache/noble-iss/scratch/ownership-59', `differential-${head.slice(0, 8)}.json`),
  ),
);

const freshness = (() => {
  const result = spawnSync(process.execPath, ['verification/ownership/cases.mjs', '--check'], {
    cwd: root, encoding: 'utf8', timeout: 600_000, env: process.env,
  });
  if (result.status !== 0) {
    throw new Error(`committed corpus differs from the emission:\n${result.stderr ?? ''}${result.stdout ?? ''}`);
  }
  return JSON.parse(result.stdout);
})();

const toolchain = {
  cargo: (() => {
    const result = spawnSync(cargo, ['-V'], { cwd: root, encoding: 'utf8', env: process.env });
    return result.status === 0 ? result.stdout.trim() : null;
  })(),
  rustc: run([rustc, '-vV'], root).trim(),
  host: run(['uname', '-srm'], root).trim().split('\n')[0],
};

const laneRun = run([cargo, 'test', '-p', 'noble-kernel', '--test', 'acceptance', 'ownership',
  '--offline', '--locked', '--', '--nocapture'], root);
for (const lane of lanes) {
  if (!new RegExp(`test ${lane} \\.\\.\\. ok`).test(laneRun)) {
    throw new Error(`ownership lane did not pass: ${lane}\n${laneRun.slice(-4000)}`);
  }
}
const summary = laneRun.split('\n').find(line => line.startsWith('ownership-differential '));
if (!summary) throw new Error(`ownership lane printed no coverage line\n${laneRun.slice(-4000)}`);
const mutations = laneRun.split('\n').find(line => line.startsWith('ownership-mutations '));
if (!mutations) throw new Error(`ownership lane printed no mutation line\n${laneRun.slice(-4000)}`);
const minimiser = laneRun.split('\n').find(line => line.startsWith('ownership-minimiser '));
if (!minimiser) throw new Error(`ownership lane printed no minimiser line\n${laneRun.slice(-4000)}`);

const count = (pattern, text) => {
  const value = Number(pattern.exec(text)?.[1]);
  if (!Number.isInteger(value)) throw new Error(`line lacks a count: ${text}`);
  return value;
};
const cases = count(/cases=(\d+)/u, summary);
const accepted = count(/accepted=(\d+)/u, summary);
const rejected = count(/rejected=(\d+)/u, summary);
const modelOnly = count(/model-only=(\d+)/u, summary);
const prefixes = count(/prefixes=(\d+)/u, summary);
const eligibility = count(/eligibility:(\d+)/u, summary);
const stackJoin = count(/stack-join:(\d+)/u, summary);
const otherRejection = count(/other:(\d+)/u, summary);
if (cases !== accepted + rejected + modelOnly) {
  throw new Error(`coverage line does not account for every case: ${summary}`);
}
if (eligibility + stackJoin + otherRejection !== rejected) {
  throw new Error(`rejection categories do not account for every rejected case: ${summary}`);
}
if (!/first-divergence=none/u.test(summary)) {
  throw new Error(`coverage line reports a divergence: ${summary}`);
}
const reasonText = /model-only-reasons=(.+?) first-divergence=/u.exec(summary)?.[1];
if (!reasonText) throw new Error(`coverage line lacks the model-only reasons: ${summary}`);
const modelOnlyByReason = {};
for (const entry of reasonText.split(/\s+/u)) {
  const match = /^([a-z-]+):(\d+)$/u.exec(entry);
  if (!match || Object.hasOwn(modelOnlyByReason, match[1])) {
    throw new Error(`invalid or duplicate model-only reason count ${entry}: ${summary}`);
  }
  modelOnlyByReason[match[1]] = Number(match[2]);
}
if (Object.values(modelOnlyByReason).reduce((total, value) => total + value, 0) !== modelOnly) {
  throw new Error(`model-only reasons do not account for every model-only case: ${summary}`);
}
if (freshness.cases !== cases) {
  throw new Error(`the emission and the lane disagree on the case count: ${freshness.cases} vs ${cases}`);
}
const corpusRows = fs.readFileSync(path.join(root, corpusFile), 'utf8')
  .split('\n')
  .filter(line => line.startsWith('theorem case_')).length;
if (corpusRows !== cases) {
  throw new Error(`the kernel-checked corpus has ${corpusRows} rows for ${cases} cases`);
}
const modelTheorems = receiptsBindingModel(
  root,
  [modelFile, traceFile, corpusFile],
  'NobleFoundation.Ownership',
);
const corpusTheorems = receiptsBindingModel(
  root,
  [modelFile, traceFile, corpusFile],
  'NobleFoundation.OwnershipCorpus',
);
if (corpusTheorems.receipts.some(receipt => receipt.model_theorems !== corpusRows)) {
  throw new Error(`the clean-source foundation receipt does not inventory all ${corpusRows} corpus theorems`);
}

const receipt = {
  schema: 'noble-ownership-differential/v2',
  assertion: 'the ownership-accounting fragment of the production acceptance machine agrees with every independent expectation generated from the Lean model on this host and target, one word prefix at a time; every corpus row is also a kernel-checked equation, the shadow checker agrees with every expectation and each declared mutation of it is caught. Not a refinement proof.',
  source_commit: head,
  source_tree: tree,
  clean_source: true,
  toolchain,
  cargo,
  rustc,
  host: toolchain.host,
  command,
  lanes,
  case_file: caseFile,
  case_file_sha256: sha256(caseFile),
  reference_file: referenceFile,
  reference_file_sha256: sha256(referenceFile),
  model_file: modelFile,
  model_file_sha256: sha256(modelFile),
  trace_file: traceFile,
  trace_file_sha256: sha256(traceFile),
  corpus_file: corpusFile,
  corpus_file_sha256: sha256(corpusFile),
  lane_file: laneFile,
  lane_file_sha256: sha256(laneFile),
  freshness: {
    check: 'node verification/ownership/cases.mjs --check',
    result: freshness.check,
    files: freshness.files,
    cases: freshness.cases,
    kernel_checked_rows: corpusRows,
  },
  coverage: {
    cases,
    accepted,
    rejected,
    model_only: modelOnly,
    model_only_by_reason: modelOnlyByReason,
    production_prefixes: prefixes,
    rejection_categories: { eligibility, stack_join: stackJoin, other: otherRejection },
    summary: summary.trim(),
    mutations: mutations.trim(),
    minimiser: minimiser.trim(),
  },
  correspondence: {
    route: 'handwritten Lean reference, emitted as kernel-decided corpus rows and compared against the production acceptance machine one word prefix at a time, with a test-local shadow checker for the mutation controls',
    model_theorems: modelTheorems,
    corpus_theorems: corpusTheorems,
    production_analogue: 'dup, drop and swap word sequences, driven one prefix at a time through noble_kernel::acceptance::check',
    first_divergence: 'none: every accepted prefix matched the model trace and every refused case refused at the word the model refuses',
    open_edge: 'no extraction or refinement proof relates crates/noble-kernel/src/acceptance to the Lean model; the differential is finite evidence on one host and target',
  },
  claims: [
    'Tested-implementation evidence that the word fragment of the acceptance machine agrees with a specification written down independently of it, at every word prefix rather than at the final stack only.',
    'Model-only cases state why they have no production analogue (branch accounting, the ordinary error return, the quotation constructor, wrapped payloads, operand arity) and are counted by reason, never reported as executable production controls.',
    'The production rejection class of a must-use resource is the machine\'s own classification; the lane asserts the agreement of the accounting decision and reports the class split.',
    'The model-to-Rust edge stays open: no refinement proof, no cross-platform claim, no claim about release of native objects.',
  ],
};
fs.mkdirSync(path.dirname(output), { recursive: true });
fs.writeFileSync(output, `${JSON.stringify(receipt, null, 2)}\n`);
console.log(JSON.stringify({ outcome: 'recorded', output, coverage: receipt.coverage, host: receipt.host }));
