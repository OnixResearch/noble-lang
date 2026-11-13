// Issue #41 item 4: allocation counts per lane and row count, measured out of process.
//
// The workspace forbids `unsafe_code`, so no counting global allocator can exist in any target.
// This runner therefore profiles the ignored lane `crates/noble-ngraph/tests/batch_rows/alloc.rs`
// with Valgrind's DHAT, pinned by store path, and records the process allocation totals. Each lane
// is measured at 0 rows (the fixed cost of building, checking and preparing the graph) and at the
// reported sizes, so the difference is the lane's own cost for those rows. DHAT's program-point
// table is retained for the largest size of each lane, which is what shows *which* allocations
// remain.
//
// Usage (repository root, clean tree):
//   node verification/ngraph/batch-allocation.mjs --output /abs/OUTSIDE_REPO
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { harnessBinding } from './harness-binding.mjs';
import { archiveWorkspace, gitText, pinnedCargo, runCargo, systemBinary } from './pinned-cargo.mjs';
import { spawnSync } from 'node:child_process';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const harness = 'crates/noble-ngraph/tests/batch_rows/alloc.rs';
// Pinned by store path: DHAT is the tool that makes allocation counts possible without `unsafe`.
const VALGRIND = '/nix/store/39iybd55afmpqlgf979ra06wj3q3wapa-valgrind-3.27.1/bin/valgrind';
const VALGRIND_VERSION = 'valgrind-3.27.1';
const lanes = ['reference', 'single', 'batch'];
const sizes = [0, 1, 16, 256, 4096, 65536];
const largest = sizes[sizes.length - 1];
const buildArgs = Object.freeze([
  'test', '--release', '--message-format=json', '--locked', '--offline', '-p', 'noble-ngraph', '--test', 'batch_row_reference', '--no-run',
]);
// Program-point classes, matched against DHAT's own frame names. Membership is reported, never
// assumed: a class that matches nothing reports zero blocks.
const classes = [
  ['report vector, one block per run', /<noble_ngraph::batch::Prepared>::run/],
  ['per-row preflight slot vector', /noble_ngraph::eval::preflight/],
  ['per-row output value vector', /Vec<noble_ngraph::sem::Value>/],
  ['staged input values in the harness', /batch_row_reference::measure::values/],
  ['operand rows in the harness', /batch_row_reference::measure::rows/],
];
const requireThat = (condition, message) => { if (!condition) throw new Error(message); };

function parseArgs(argv) {
  const options = { output: null, allowDirty: false };
  for (let index = 0; index < argv.length; index += 1) {
    if (argv[index] === '--output') options.output = argv[(index += 1)];
    else if (argv[index] === '--allow-dirty') options.allowDirty = true;
    else throw new Error(`usage: node verification/ngraph/batch-allocation.mjs --output NEW_DIR [--allow-dirty]; unknown ${argv[index]}`);
  }
  requireThat(options.output, 'usage: --output NEW_DIR is required');
  return options;
}

/// The test executable cargo just built, from its JSON artifact messages.
function executable(tools, source, target, log) {
  const result = runCargo({ tools, source, args: buildArgs, target });
  fs.writeFileSync(`${log}.build.stdout`, result.stdout);
  fs.writeFileSync(`${log}.build.stderr`, result.stderr);
  requireThat(result.status === 0, `build exited ${result.status}: ${result.stderr.slice(-2000)}`);
  const artifacts = result.stdout.split('\n').filter(line => line.startsWith('{'))
    .map(line => JSON.parse(line))
    .filter(message => message.reason === 'compiler-artifact' && message.target?.name === 'batch_row_reference' && message.executable);
  requireThat(artifacts.length === 1, `expected one batch_row_reference executable, found ${artifacts.length}`);
  return artifacts[0].executable;
}

/// Totals and program points of one DHAT run of one lane at one row count.
function profile({ binary, lane, rows, output }) {
  const dhat = path.join(output, 'dhat', `${lane}-${rows}.dhat`);
  const result = spawnSync(VALGRIND, [
    '--tool=dhat', `--dhat-out-file=${dhat}`, binary, '--exact', 'measure::allocation_lane', '--ignored', '--nocapture',
  ], {
    cwd: output, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024, timeout: 1_800_000,
    env: { PATH: '/run/current-system/sw/bin:/usr/bin:/bin', HOME: output, NOBLE_BATCH_ALLOC_LANE: lane, NOBLE_BATCH_ALLOC_ROWS: String(rows) },
  });
  const log = `${result.stdout}${result.stderr}`;
  requireThat(result.status === 0 && !result.error,
    `Valgrind exited ${result.status ?? result.signal ?? result.error?.code} for ${lane} at ${rows} rows: ${log.slice(-1500)}`);
  const total = /Total:\s+([\d,]+) bytes in ([\d,]+) blocks/.exec(log);
  requireThat(total, `DHAT reported no total for ${lane} at ${rows} rows: ${log.slice(-1500)}`);
  requireThat(log.includes(`ALLOC {"lane":"${lane}","rows":${rows}`), `the lane did not run: ${log.slice(-1500)}`);
  const dhatOut = JSON.parse(fs.readFileSync(dhat, 'utf8'));
  const frames = dhatOut.ftbl;
  const programPoints = [...dhatOut.pps].sort((left, right) => right.tb - left.tb).map(point => ({
    bytes: point.tb,
    blocks: point.tbk,
    site: point.fs.map(index => frames[index]).find(name => /noble_ngraph|batch_row_reference/.test(name))
      ?? point.fs.map(index => frames[index])[0],
  }));
  return {
    bytes: Number(total[1].replaceAll(',', '')),
    blocks: Number(total[2].replaceAll(',', '')),
    program_points: programPoints,
  };
}

function classify(programPoints) {
  return Object.fromEntries(classes.map(([label, pattern]) => {
    const matched = programPoints.filter(point => pattern.test(point.site ?? ''));
    return [label, {
      blocks: matched.reduce((total, point) => total + point.blocks, 0),
      bytes: matched.reduce((total, point) => total + point.bytes, 0),
    }];
  }));
}

function main() {
  const options = parseArgs(process.argv.slice(2));
  const output = path.resolve(options.output);
  requireThat(path.relative(root, output).startsWith('..'), 'output directory must be outside the source tree');
  requireThat(!fs.existsSync(output), 'output directory must not exist yet');
  fs.mkdirSync(path.join(output, 'dhat'), { recursive: true });
  const version = spawnSync(VALGRIND, ['--version'], { encoding: 'utf8' }).stdout.trim();
  requireThat(version === VALGRIND_VERSION, `pinned valgrind reports ${version}, expected ${VALGRIND_VERSION}`);
  const commit = gitText(root, ['rev-parse', 'HEAD']).trim();
  const tree = gitText(root, ['rev-parse', 'HEAD^{tree}']).trim();
  const status = gitText(root, ['status', '--porcelain', '--untracked-files=all']);
  requireThat(status === '' || options.allowDirty, 'source worktree is dirty; re-run with --allow-dirty');
  const tools = pinnedCargo({ root, evidence: path.join(output, 'evidence') });
  const source = path.join(output, 'source');
  archiveWorkspace({ root, commit, destination: source });
  const binary = executable(tools, source, path.join(output, 'target'), path.join(output, 'alloc'));

  const lanesOut = {};
  for (const lane of lanes) {
    const runs = {};
    for (const rows of sizes) {
      const profiled = profile({ source, binary, lane, rows, output });
      runs[String(rows)] = { blocks: profiled.blocks, bytes: profiled.bytes };
      if (rows === largest) {
        runs[String(rows)].classes = classify(profiled.program_points);
        runs[String(rows)].program_points = profiled.program_points.slice(0, 8);
      }
    }
    const baseline = runs['0'];
    const rounded = value => Math.round(value * 1000) / 1000;
    lanesOut[lane] = {
      runs,
      baseline: { blocks: baseline.blocks, bytes: baseline.bytes },
      per_row: Object.fromEntries(sizes.filter(rows => rows > 0).map(rows => {
        const run = runs[String(rows)];
        return [String(rows), {
          total_blocks_delta: run.blocks - baseline.blocks,
          total_bytes_delta: run.bytes - baseline.bytes,
          blocks: rounded((run.blocks - baseline.blocks) / rows),
          bytes: rounded((run.bytes - baseline.bytes) / rows),
        }];
      })),
    };
  }

  const receipt = {
    schema: 'noble-ngraph-batch-allocations/v1', issue: 41, result: 'measured',
    source_commit: commit, source_tree: tree, clean_source: status === '',
    harness: harnessBinding(root, commit, harness, systemBinary('git')),
    toolchain: { rustc: tools.version, vendor_nar_hash: tools.vendor.narHash },
    valgrind: {
      path: VALGRIND, version, tool: 'dhat',
      command: ['valgrind', '--tool=dhat', '--dhat-out-file=...', 'batch_row_reference', '--exact', 'measure::allocation_lane', '--ignored', '--nocapture'],
      why: 'DHAT counts every allocation out of process, so the workspace keeps `unsafe_code = "forbid"` and no counting allocator is compiled in',
    },
    lanes: lanesOut,
    rows_baseline_note: 'the 0-row run is the fixed cost of building, checking, compiling and preparing the workload graph; a lane\'s cost for n rows is its total minus that baseline',
    class_matchers: classes.map(([label, pattern]) => ({ label, pattern: String(pattern) })),
    non_claims: [
      'Allocation counts, not allocation sizes in bytes of live memory: bytes are the summing of every block allocated, which is why repeated runs of an allocating lane keep reporting allocations.',
      'One lane per process under Valgrind, one host, one run; DHAT changes timing, so these counts are not a measurement of time.',
      'The harness\'s own staged inputs and operand rows are counted and classified separately from the library\'s allocations.',
    ],
  };
  fs.writeFileSync(path.join(output, 'receipt.json'), `${JSON.stringify(receipt, null, 2)}\n`);
  process.stdout.write(`${JSON.stringify({ receipt: path.join(output, 'receipt.json') })}\n`);
}

try {
  main();
} catch (error) {
  console.error(`[batch-allocation] FAIL ${error.message}`);
  process.exitCode = 1;
}
