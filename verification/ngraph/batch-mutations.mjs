// Issue #41 fault controls for the graph-backed batch: one mutant at a time, retained.
//
// Each mutation disables one guarantee of `crates/noble-ngraph/src/batch.rs` in a scratch copy of
// the committed source, runs the `batch_row_reference` tests, and requires them to fail. A mutant
// the tests do not catch (or one that does not compile) is reported as such and makes the receipt
// `failed`; nothing is rounded to a pass.
//
// The copy is a `git archive` of the commit, so every mutated build is the committed source plus
// exactly one textual edit, and the mutated file's bytes are restored from the archive after each
// run (the SHA-256 of the archive's file and of the committed blob must agree before the first
// mutation).
//
// Usage (repository root, clean tree):
//   node verification/ngraph/batch-mutations.mjs --out /abs/mutations.json --work /abs/scratch
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { archiveWorkspace, gitText, pinnedCargo, runCargo, systemBinary } from './pinned-cargo.mjs';

const SCHEMA = 'noble-ngraph-batch-mutations/v1';
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const FILE = 'crates/noble-ngraph/src/batch.rs';
const TARGET = 'batch_row_reference';
const commandArgs = Object.freeze([
  'test', '--message-format=json', '--locked', '--offline', '-p', 'noble-ngraph', '--test', TARGET,
]);

// [name, category, anchor, replacement]. Every anchor must occur exactly once in the committed file.
const MUTATIONS = [
  ['row swap', 'row order', '        let mut rest = inputs;\n        for row in 0..rows {',
    '        let mut rest = inputs;\n        for row in (0..rows).map(|index| if index == 0 && rows > 1 {\n            1\n        } else if index == 1 {\n            0\n        } else {\n            index\n        }) {'],
  ['stale report reuse', 'stale state', '            reports.push(report);',
    `            let report = if matches!(&report.obs, crate::eval::Obs::Refused(_)) {\n                reports.last().cloned().unwrap_or(report)\n            } else {\n                report\n            };\n            reports.push(report);`],
  ['shared inputs', 'stale state', '            let (row_inputs, tail) = rest.split_at(self.width);\n            rest = tail;',
    '            let (row_inputs, tail) = rest.split_at(self.width);\n            rest = if row >= 1 { rest } else { tail };'],
  ['operand order', 'arithmetic order', '            RowOp::Sub => crate::sem::i64_sub(self.deeper, self.top),',
    '            RowOp::Sub => crate::sem::i64_sub(self.top, self.deeper),'],
  ['run fuel uncharged', 'accounting', '            remaining = remaining.map(|left| left.saturating_sub(report.evaluated_nodes));',
    '            remaining = remaining.map(|left| left.saturating_sub(0));'],
  ['row exhaustion stops the run', 'exhaustion', '            if is_exhausted && is_run_bound {',
    '            if is_exhausted {'],
  ['row limit ignored', 'limits', '        if rows > limits.max_rows {',
    '        if false && rows > limits.max_rows {'],
  ['output limit refuses at equality', 'limits',
    '            .is_none_or(|total| total > limits.max_output_values)',
    '            .is_none_or(|total| total >= limits.max_output_values)'],
  ['cancellation ignored', 'cancellation', '            if cancel_row.is_some_and(|cancel| cancel(row)) {',
    '            if false && cancel_row.is_some_and(|cancel| cancel(row)) {'],
  ['row work charged twice', 'accounting',
    '            remaining = remaining.map(|left| left.saturating_sub(report.evaluated_nodes));',
    '            remaining = remaining.map(|left| left.saturating_sub(report.evaluated_nodes.saturating_mul(2)));'],
  ['reports reversed', 'result association', '        Outcome {\n            reports,\n            stop: None,\n        }',
    '        reports.reverse();\n        Outcome {\n            reports,\n            stop: None,\n        }'],
];

const sha256 = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const requireThat = (condition, message) => { if (!condition) throw new Error(message); };

function parseArgs(argv) {
  const options = { out: null, work: null, timeoutSeconds: 900 };
  for (let index = 0; index < argv.length; index += 1) {
    const flag = argv[index];
    const value = argv[(index += 1)];
    if (value === undefined) throw new Error(`missing value for ${flag}`);
    if (flag === '--out') options.out = value;
    else if (flag === '--work') options.work = value;
    else if (flag === '--timeout-seconds') options.timeoutSeconds = Number(value);
    else throw new Error(`unknown flag ${flag}`);
  }
  requireThat(options.out, '--out is required');
  requireThat(options.work, '--work is required');
  requireThat(Number.isInteger(options.timeoutSeconds) && options.timeoutSeconds >= 1,
    '--timeout-seconds must be a positive integer');
  return options;
}

/// Runs the batch tests in `source` and names what happened to each test.
function runTests(tools, source, target, timeoutSeconds) {
  const result = runCargo({ tools, source, args: commandArgs, target, timeout: timeoutSeconds * 1000 });
  const output = `${result.stdout}${result.stderr}`;
  const named = verdict => [...output.matchAll(new RegExp(`^test (\\S+) \\.\\.\\. ${verdict}$`, 'gm'))].map(match => match[1]);
  return {
    status: result.status,
    timed_out: result.timedOut,
    compiled: !/error\[E\d+\]|could not compile|error: expected|error: the trait/.test(output),
    passed: named('ok'),
    failed: named('FAILED'),
    output_tail: output.slice(-4000),
  };
}

function main() {
  const options = parseArgs(process.argv.slice(2));
  const out = path.resolve(options.out);
  const work = path.resolve(options.work);
  requireThat(path.relative(root, out).startsWith('..'), 'the receipt must be written outside the source tree');
  requireThat(path.relative(root, work).startsWith('..'), 'the scratch copy must be outside the source tree');
  requireThat(!fs.existsSync(out), 'refusing to overwrite an existing receipt');
  requireThat(!fs.existsSync(work), 'the scratch directory must not exist yet');
  const status = gitText(root, ['status', '--porcelain', '--untracked-files=all']);
  requireThat(status === '', 'source tree is not clean; commit before recording a receipt');
  const commit = gitText(root, ['rev-parse', 'HEAD']).trim();
  const tree = gitText(root, ['rev-parse', 'HEAD^{tree}']).trim();
  const committed = gitText(root, ['show', `${commit}:${FILE}`], { encoding: null });
  const committedSha = sha256(committed);
  const original = committed.toString('utf8');
  for (const [name, , anchor] of MUTATIONS) {
    requireThat(original.split(anchor).length === 2, `the anchor for ${name} must occur exactly once`);
  }
  fs.mkdirSync(work, { recursive: true });
  const source = path.join(work, 'source');
  archiveWorkspace({ root, commit, destination: source });
  const file = path.join(source, FILE);
  requireThat(sha256(fs.readFileSync(file)) === committedSha,
    'the archived harness is not the committed blob');
  const tools = pinnedCargo({ root, evidence: path.join(work, 'evidence') });
  const target = path.join(work, 'target');

  const baseline = runTests(tools, source, target, options.timeoutSeconds);
  requireThat(baseline.compiled && baseline.failed.length === 0 && !baseline.timed_out && baseline.passed.length > 0,
    `unmutated tests must pass: ${JSON.stringify({ status: baseline.status, failed: baseline.failed, passed: baseline.passed.length })}`);

  const mutations = [];
  try {
    for (const [name, category, anchor, replacement] of MUTATIONS) {
      fs.writeFileSync(file, original.replace(anchor, replacement));
      const observed = runTests(tools, source, target, options.timeoutSeconds);
      mutations.push({
        name, category, anchor, replacement,
        compiled: observed.compiled, timed_out: observed.timed_out, exit_status: observed.status,
        failed: observed.failed, passed_count: observed.passed.length,
        detected: observed.compiled && !observed.timed_out && observed.failed.length > 0,
        output_tail: observed.compiled && !observed.timed_out && observed.failed.length > 0
          ? undefined : observed.output_tail,
      });
      console.error(JSON.stringify({ name, detected: mutations.at(-1).detected, failed: observed.failed }));
    }
  } finally {
    fs.writeFileSync(file, committed);
    requireThat(sha256(fs.readFileSync(file)) === committedSha, `restore of ${FILE} failed`);
  }
  const silent = mutations.filter(mutation => !mutation.detected).map(mutation => mutation.name);
  const receipt = {
    schema: SCHEMA,
    issue: 41,
    result: silent.length === 0 ? 'passed' : 'failed',
    source_commit: commit,
    source_tree: tree,
    clean_source: true,
    file: FILE,
    file_sha256: committedSha,
    harness: { target: TARGET, command: [tools.cargo, ...commandArgs] },
    toolchain: { rustc: tools.version, vendor_nar_hash: tools.vendor.narHash },
    timeout_seconds: options.timeoutSeconds,
    unmutated_passed: baseline.passed,
    mutations,
    undetected: silent,
    note: 'Each mutant is one textual edit of the archived batch.rs; the tests must fail. A mutant that still compiles and passes every test is a failed fault control and is reported in undetected.',
  };
  fs.mkdirSync(path.dirname(out), { recursive: true });
  fs.writeFileSync(out, `${JSON.stringify(receipt, null, 2)}\n`);
  process.stdout.write(`${JSON.stringify({ receipt: out, result: receipt.result, mutations: mutations.length, undetected: silent })}\n`);
  if (silent.length !== 0) process.exitCode = 1;
}

try {
  main();
} catch (error) {
  console.error(`[batch-mutations] FAIL ${error.message}`);
  process.exitCode = 1;
}
