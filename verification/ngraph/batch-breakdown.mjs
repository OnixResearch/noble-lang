// Issue #41 item 6 receipt runner: the cold, staging, output-copy and byte breakdown of the CPU batch
// entry point. It runs one ignored release harness
// (crates/noble-ngraph/tests/batch_rows/breakdown.rs) over an archived copy of the committed source
// with the pinned toolchain and reviewed vendor. Reporting only: no timing threshold is asserted.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { harnessBinding } from './harness-binding.mjs';
import { archiveWorkspace, gitText, pinnedCargo, runCargo, systemBinary } from './pinned-cargo.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const harness = 'crates/noble-ngraph/tests/batch_rows/breakdown.rs';
const commandArgs = Object.freeze([
  'test', '--release', '--locked', '--offline', '-p', 'noble-ngraph', '--test', 'batch_row_reference',
  '--', '--ignored', '--nocapture', '--exact', 'breakdown::cold_staging_and_bytes',
]);
const requireThat = (condition, message) => { if (!condition) throw new Error(message); };
const git = systemBinary('git');

function parseArgs(argv) {
  const options = { output: null, allowDirty: false };
  for (let index = 0; index < argv.length; index += 1) {
    if (argv[index] === '--output') options.output = argv[(index += 1)];
    else if (argv[index] === '--allow-dirty') options.allowDirty = true;
    else throw new Error(`usage: SELECTED_NODE verification/ngraph/batch-breakdown.mjs --output NEW_DIR [--allow-dirty]; unknown ${argv[index]}`);
  }
  requireThat(options.output, 'usage: --output NEW_DIR is required');
  return options;
}

function run(tools, source, target, log) {
  const result = runCargo({ tools, source, args: commandArgs, target });
  fs.writeFileSync(`${log}.stdout`, result.stdout ?? '');
  fs.writeFileSync(`${log}.stderr`, result.stderr ?? '');
  requireThat(result.status === 0, `measurement exited ${result.status ?? result.signal}: ${(result.stderr ?? '').slice(-2000)}`);
  const rows = [...(result.stdout ?? '').matchAll(/^MEASURE (\{.*\})$/gm)].map(match => JSON.parse(match[1]));
  requireThat(rows.length === 4, `expected four MEASURE rows, found ${rows.length}`);
  requireThat(rows[0].lane === 'cold' && rows.slice(1).every(row => row.lane === 'size'), 'unexpected MEASURE lanes');
  return rows;
}

function main() {
  const options = parseArgs(process.argv.slice(2));
  const output = path.resolve(options.output);
  requireThat(path.relative(root, output).startsWith('..'), 'output directory must be outside the source tree');
  requireThat(!fs.existsSync(output), 'output directory must not exist yet');
  fs.mkdirSync(output, { recursive: true });
  const commit = gitText(root, ['rev-parse', 'HEAD']).trim();
  const tree = gitText(root, ['rev-parse', 'HEAD^{tree}']).trim();
  const status = gitText(root, ['status', '--porcelain', '--untracked-files=all']);
  requireThat(status === '' || options.allowDirty, 'source worktree is dirty; re-run with --allow-dirty');
  const tools = pinnedCargo({ root, evidence: output, jobs: 4 });
  const source = path.join(output, 'source');
  archiveWorkspace({ root, commit, destination: source });
  const [cold, ...sizes] = run(tools, source, path.join(output, 'target'), path.join(output, 'measure'));
  const receipt = {
    schema: 'noble-ngraph-batch-breakdown/v1', issue: 41, result: 'measured',
    source_commit: commit, source_tree: tree, clean_source: status === '',
    harness: harnessBinding(root, commit, harness, git),
    toolchain: { rustc: tools.version, vendor_nar_hash: tools.vendor.narHash },
    command: ['cargo', ...commandArgs],
    workload: 'the measure.rs graph: two I64 inputs, add, mul, mul, sub, eq; two outputs; rows cycle the seven i64 boundary values',
    cold, sizes,
    instruments: {
      time: 'wall-clock nanoseconds, release build, one thread; cold costs as the first call in the process and the median of five fresh calls; per-size costs as the median of five',
      bytes: 'declared from sizes, not measured: retained_bytes and scratch_bytes of the compiled graph; staged inputs, per-row reports, the outputs each report owns, and the copied outputs as element counts times size_of',
      memory: 'resident-set growth across the first run at each size, from /proc/self/statm with a 4096-byte page',
    },
    unavailable: ['allocation counts: the workspace forbids unsafe_code, so no counting global allocator can exist in any target',
      'extraction: the workload is an authored ngraph-ir/0 graph, so no extraction cost exists for it'],
    non_claims: ['Reporting only: no timing threshold is a correctness claim.',
      'One host and one run; the numbers bound nothing on other hardware.'],
  };
  fs.writeFileSync(path.join(output, 'receipt.json'), `${JSON.stringify(receipt, null, 2)}\n`);
  process.stdout.write(`${JSON.stringify({ receipt: path.join(output, 'receipt.json') })}\n`);
}

try {
  main();
} catch (error) {
  console.error(`[batch-breakdown] FAIL ${error.message}`);
  process.exitCode = 1;
}
