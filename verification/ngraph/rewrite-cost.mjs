// Issue #64 cost-lane receipt runner: run the certified-rewrite cost lane and record the report with
// the source, toolchain and host it was measured under.
//
// The lane itself never executes a program, so this receipt prices proposal and certificate-check
// work only; every stage it cannot observe is reported as unmeasured with a reason, never as zero.
// The continuation thresholds come from `verification/ngraph/rewrite-cost-v1.txt`, frozen before
// the run.
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

const args = process.argv.slice(2);
const outIndex = args.indexOf('--output');
const output = outIndex >= 0 ? path.resolve(args[outIndex + 1]) : null;
if (output === null) {
  console.error('usage: node verification/ngraph/rewrite-cost.mjs --output <file>');
  process.exit(2);
}

function run(argv, cwd) {
  const result = spawnSync('nix', ['develop', '--command', ...argv], {
    cwd, encoding: 'utf8', timeout: 1800_000, maxBuffer: 64 * 1024 * 1024,
  });
  const text = `${result.stdout ?? ''}${result.stderr ?? ''}`;
  if (result.status !== 0) {
    throw new Error(`command failed (${result.status}): ${argv.join(' ')}\n${text.slice(-2000)}`);
  }
  return text;
}

const root = process.cwd();
const command = 'CARGO_TARGET_DIR=/var/tmp/noble-target cargo test -p noble-ngraph ' +
  '--test rewrite_cost --offline --locked -- --nocapture';
const text = run(['bash', '-lc', command], root);

// The lane prints exactly one JSON report between the first `{` line and the matching closing brace.
const start = text.indexOf('\n{\n');
if (start < 0) {
  throw new Error(`no report in the lane output:\n${text.slice(-2000)}`);
}
const end = text.indexOf('\n}\n', start);
if (end < 0) {
  throw new Error('the report is unterminated');
}
const report = JSON.parse(text.slice(start + 1, end + 3));

const enclosed = report.stages.every((stage) =>
  stage.status === 'measured' || (stage.status === 'unmeasured' && stage.reason.length > 0));
if (!enclosed) {
  throw new Error('a stage is neither measured nor unmeasured with a reason');
}

const receipt = {
  schema: 'noble-rewrite-cost-receipt/v1',
  source_commit: run(['git', 'rev-parse', 'HEAD'], root).trim(),
  source_tree: run(['git', 'rev-parse', 'HEAD^{tree}'], root).trim(),
  clean_source: run(['git', 'status', '--porcelain'], root).trim() === '',
  toolchain: run(['rustc', '--version'], root).trim(),
  host: `${os.type()} ${os.release()} ${os.arch()}`,
  command,
  manifest: 'verification/ngraph/rewrite-cost-v1.txt',
  corpus: report.corpus,
  report,
};
fs.writeFileSync(output, `${JSON.stringify(receipt, null, 2)}\n`);
console.log(JSON.stringify({
  outcome: 'recorded',
  output,
  source_commit: receipt.source_commit,
  coverage: report.coverage,
  rows: report.rows.length,
}));
