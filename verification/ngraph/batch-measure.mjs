// Issue #41 post-decision re-measurement runner: the four evaluation lanes over the seven
// re-measurement sizes, five timed repetitions each, plus the batch lane's first and second run
// resident growth. Reporting only; no timing threshold is asserted and the frozen continuation
// criteria are quoted, never re-applied.
//
// It runs the ignored release harness `crates/noble-ngraph/tests/batch_rows/measure_repeated.rs`
// (`measure::repeated::timed_workloads`) over an archived copy of the committed source with the pinned
// toolchain and reviewed vendor, then writes a source-bound receipt.
//
// Usage (repository root, clean tree):
//   node verification/ngraph/batch-measure.mjs --output /abs/OUTSIDE_REPO
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { harnessBinding } from './harness-binding.mjs';
import { archiveWorkspace, gitText, pinnedCargo, runCargo, systemBinary } from './pinned-cargo.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const harness = 'crates/noble-ngraph/tests/batch_rows/measure_repeated.rs';
const lanes = ['reference', 'single', 'batch', 'native'];
const sizes = [1, 16, 64, 256, 1024, 4096, 65536];
const commandArgs = Object.freeze([
  'test', '--release', '--locked', '--offline', '-p', 'noble-ngraph', '--test', 'batch_row_reference',
  '--', '--ignored', '--nocapture', '--exact', 'measure::repeated::timed_workloads',
]);
const requireThat = (condition, message) => { if (!condition) throw new Error(message); };

function parseArgs(argv) {
  const options = { output: null, allowDirty: false };
  for (let index = 0; index < argv.length; index += 1) {
    if (argv[index] === '--output') options.output = argv[(index += 1)];
    else if (argv[index] === '--allow-dirty') options.allowDirty = true;
    else throw new Error(`usage: node verification/ngraph/batch-measure.mjs --output NEW_DIR [--allow-dirty]; unknown ${argv[index]}`);
  }
  requireThat(options.output, 'usage: --output NEW_DIR is required');
  return options;
}

function measure(tools, source, target, log) {
  const result = runCargo({ tools, source, args: commandArgs, target });
  fs.writeFileSync(`${log}.stdout`, result.stdout);
  fs.writeFileSync(`${log}.stderr`, result.stderr);
  requireThat(result.status === 0,
    `measurement exited ${result.status}: ${(result.stderr || result.stdout).slice(-2000)}`);
  return [...result.stdout.matchAll(/^MEASURE (\{.*\})$/gm)].map(match => JSON.parse(match[1]));
}

/// Fails unless every lane reported exactly once at every size and every size agreed across lanes.
function collect(rows) {
  const declared = rows.filter(row => row.lane === 'declared');
  const agreement = new Map(rows.filter(row => row.agreement).map(row => [row.agreement.rows, row.agreement.outputs]));
  const table = new Map(sizes.map(size => [size, {}]));
  for (const row of rows) {
    if (row.lane === 'declared' || row.agreement) continue;
    requireThat(lanes.includes(row.lane), `unexpected MEASURE lane ${row.lane}`);
    requireThat(table.has(row.rows), `unexpected MEASURE size ${row.rows}`);
    const sizeLanes = table.get(row.rows);
    requireThat(!sizeLanes[row.lane], `duplicate ${row.lane} row at ${row.rows} rows`);
    sizeLanes[row.lane] = row;
  }
  requireThat(declared.length === 1, `expected one declared row, found ${declared.length}`);
  for (const [size, sizeLanes] of table) {
    for (const lane of lanes) requireThat(sizeLanes[lane], `missing ${lane} lane at ${size} rows`);
    requireThat(agreement.has(size), `missing the cross-lane agreement row at ${size} rows`);
    for (const lane of lanes) {
      requireThat(sizeLanes[lane].runs === 5, `${lane} reports five runs at ${size} rows`);
    }
  }
  return { declared: declared[0], table, agreement };
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
  const tools = pinnedCargo({ root, evidence: output });
  const source = path.join(output, 'source');
  archiveWorkspace({ root, commit, destination: source });
  const measured = collect(measure(tools, source, path.join(output, 'target'), path.join(output, 'measure')));

  const table = Object.fromEntries([...measured.table].map(([size, byLane]) => [String(size),
    Object.fromEntries(lanes.map(lane => [lane, {
      total_ns: byLane[lane].total_ns,
      best_total_ns: byLane[lane].best_total_ns,
      ns_per_row: byLane[lane].ns_per_row,
      best_ns_per_row: byLane[lane].best_ns_per_row,
      first_row_ns: byLane[lane].first_row_ns,
      resident_growth_bytes: byLane[lane].resident_growth_bytes,
      resident_growth_second_bytes: byLane[lane].resident_growth_second_bytes,
    }]))]));
  const comparison = sizes.map(size => {
    const batching = table[String(size)].batch;
    const single = table[String(size)].single;
    return {
      rows: size,
      batch_ns_per_row: batching.ns_per_row,
      single_ns_per_row: single.ns_per_row,
      batch_faster_by_median: batching.ns_per_row < single.ns_per_row,
      batch_faster_by_best: batching.best_ns_per_row < single.best_ns_per_row,
    };
  });
  const firstBy = key => {
    const hit = comparison.find(row => row[key]);
    return hit ? hit.rows : null;
  };
  const receipt = {
    schema: 'noble-ngraph-batch-remeasurement/v1', issue: 41, result: 'measured',
    lane: 'post-decision re-measurement',
    source_commit: commit, source_tree: tree, clean_source: status === '',
    harness: harnessBinding(root, commit, harness, systemBinary('git')),
    toolchain: { rustc: tools.version, vendor_nar_hash: tools.vendor.narHash },
    command: ['cargo', ...commandArgs],
    workload: 'the measure.rs graph: two I64 inputs, add, mul, mul, sub, eq; two outputs; rows cycle the seven i64 boundary values',
    sizes,
    runs_per_lane_and_size: 5,
    declared: { retained_bytes: measured.declared.retained_bytes, debug_assertions: measured.declared.debug_assertions },
    table,
    agreement: Object.fromEntries([...measured.agreement].map(([size, outputs]) => [String(size), outputs])),
    comparison,
    summary: {
      smallest_rows_batch_faster_by_median: firstBy('batch_faster_by_median'),
      smallest_rows_batch_faster_by_best: firstBy('batch_faster_by_best'),
      batch_growth_at_65536_bytes: table['65536'].batch.resident_growth_bytes,
      batch_second_run_growth_at_65536_bytes: table['65536'].batch.resident_growth_second_bytes,
    },
    instruments: {
      time: 'std::time::Instant, release build, one thread, five timed repetitions per lane and size; median and best total reported; first_row_ns is the first row own time for the per-row lanes and the run mean row time for batch',
      native: 'every row value is folded into a checksum that reaches the caller through std::hint::black_box, so the loop body cannot be eliminated',
      memory: 'resident-set growth of the first and of the second identical run at each size, from /proc/self/statm with a 4096-byte page; this is transient footprint of an allocating run, not retained memory',
    },
    unavailable: ['allocation counts in-process: the workspace forbids unsafe_code, so no counting global allocator can exist; verification/ngraph/batch-allocation.mjs measures counts out of process with Valgrind DHAT instead'],
    criteria: 'verification/ngraph/batch-continuation-criteria.md is frozen and is quoted, not re-applied: this receipt is post-decision evidence and does not re-run the go/no-go rule',
    non_claims: ['Reporting only: no timing threshold is a correctness claim.',
      'One shared host and one run; load between runs differs, so a median of five is reported beside the best sample and neither bounds other hardware.'],
  };
  fs.writeFileSync(path.join(output, 'receipt.json'), `${JSON.stringify(receipt, null, 2)}\n`);
  process.stdout.write(`${JSON.stringify({ receipt: path.join(output, 'receipt.json') })}\n`);
}

try {
  main();
} catch (error) {
  console.error(`[batch-measure] FAIL ${error.message}`);
  process.exitCode = 1;
}
