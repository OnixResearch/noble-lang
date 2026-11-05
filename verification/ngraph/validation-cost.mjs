// Issue #43 validation-cost receipt runner: an A/B release measurement of the packed-schedule
// validator at the graph checker's default budgets. Both variants run the same harness
// (crates/noble-ngraph/tests/schedule_cost_dir/measure.rs) over an archived copy of the committed
// source; the baseline variant only swaps in the validator files (`layout.rs`, `schedule.rs`) of a
// named baseline commit. Reporting only: no timing threshold is asserted anywhere.
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { selectedVendor } from '../selected-vendor.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const validatorFiles = Object.freeze(['crates/noble-ngraph/src/layout.rs', 'crates/noble-ngraph/src/schedule.rs']);
const harness = 'crates/noble-ngraph/tests/schedule_cost_dir/measure.rs';
const commandArgs = Object.freeze([
  'test', '--release', '--locked', '--offline', '-p', 'noble-ngraph', '--test', 'schedule_correspondence',
  '--', '--ignored', '--nocapture', 'validation_cost_at_the_graph_budget',
]);
const requireThat = (condition, message) => { if (!condition) throw new Error(message); };
const sha256 = bytes => createHash('sha256').update(bytes).digest('hex');
const git = fs.realpathSync('/run/current-system/sw/bin/git');
const tar = fs.realpathSync('/run/current-system/sw/bin/tar');

function parseArgs(argv) {
  const options = { output: null, baseline: null, allowDirty: false };
  for (let index = 0; index < argv.length; index += 1) {
    if (argv[index] === '--output') options.output = argv[(index += 1)];
    else if (argv[index] === '--baseline') options.baseline = argv[(index += 1)];
    else if (argv[index] === '--allow-dirty') options.allowDirty = true;
    else throw new Error(`usage: SELECTED_NODE verification/ngraph/validation-cost.mjs --output NEW_DIR --baseline COMMIT [--allow-dirty]; unknown ${argv[index]}`);
  }
  requireThat(options.output && options.baseline, 'usage: --output NEW_DIR and --baseline COMMIT are required');
  return options;
}

function gitText(argv, options = {}) {
  const result = spawnSync(git, argv, {
    cwd: root, encoding: options.encoding ?? 'utf8', maxBuffer: 1024 * 1024 * 1024,
    env: { PATH: path.dirname(git), HOME: '/nonexistent', GIT_CONFIG_NOSYSTEM: '1', GIT_CONFIG_GLOBAL: '/dev/null' },
  });
  requireThat(result.status === 0, `git ${argv.join(' ')} failed: ${result.stderr}`);
  return result.stdout;
}

function archive(commit, destination) {
  fs.mkdirSync(destination, { recursive: true });
  // Only the Cargo workspace is needed to build and run the harness.
  const file = `${destination}.tar`;
  gitText(['archive', '--format=tar', `--output=${file}`, commit, 'Cargo.toml', 'Cargo.lock', 'crates']);
  const result = spawnSync(tar, ['-xf', file, '-C', destination], { encoding: 'utf8' });
  requireThat(result.status === 0, `tar failed: ${result.stderr}`);
  fs.rmSync(file);
}

function toolchain(evidence) {
  const selection = JSON.parse(fs.readFileSync(path.join(root, 'policy/tool-selection.json'), 'utf8'));
  const rustBin = path.join(selection.tool_paths.quality_rust.output, 'bin');
  const linkerBin = selection.component_sync.linker_bin;
  const vendor = selectedVendor(selection.component_sync);
  const cargoHome = path.join(evidence, 'cargo-home');
  const home = path.join(evidence, 'home');
  fs.mkdirSync(cargoHome);
  fs.mkdirSync(home);
  fs.writeFileSync(path.join(cargoHome, 'config.toml'),
    `[source.crates-io]\nreplace-with = "reviewed-vendor"\n[source.reviewed-vendor]\ndirectory = ${JSON.stringify(vendor.directory)}\n[net]\noffline = true\n`,
    { flag: 'wx' });
  const environment = {
    HOME: home, USER: 'noble-validation-cost', LOGNAME: 'noble-validation-cost', LANG: 'C.UTF-8', LC_ALL: 'C.UTF-8',
    CI: '1', PATH: [rustBin, linkerBin, '/run/current-system/sw/bin', '/usr/bin', '/bin'].join(':'), TMPDIR: home,
    CARGO_HOME: cargoHome, CARGO_NET_OFFLINE: 'true', CARGO_BUILD_JOBS: '4', CARGO_INCREMENTAL: '0',
    RUSTC: path.join(rustBin, 'rustc'), RUSTDOC: path.join(rustBin, 'rustdoc'), RUSTC_WRAPPER: '',
    RUSTC_WORKSPACE_WRAPPER: '', CARGO_BUILD_RUSTC_WRAPPER: '', RUSTFLAGS: '', CARGO_ENCODED_RUSTFLAGS: '',
    CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS: '', CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER: path.join(linkerBin, 'cc'),
    CARGO_TERM_COLOR: 'never',
  };
  const version = spawnSync(path.join(rustBin, 'rustc'), ['--version'], { env: environment, encoding: 'utf8' }).stdout.trim();
  return { cargo: path.join(rustBin, 'cargo'), environment, vendor, version };
}

function run(tools, source, target, log) {
  const result = spawnSync(tools.cargo, commandArgs, {
    cwd: source, env: { ...tools.environment, CARGO_TARGET_DIR: target }, encoding: 'utf8',
    timeout: 3_600_000, maxBuffer: 256 * 1024 * 1024,
  });
  fs.writeFileSync(`${log}.stdout`, result.stdout ?? '');
  fs.writeFileSync(`${log}.stderr`, result.stderr ?? '');
  requireThat(result.status === 0, `measurement exited ${result.status ?? result.signal}: ${(result.stderr ?? '').slice(-2000)}`);
  const rows = [...(result.stdout ?? '').matchAll(/^MEASURE (\{.*\})$/gm)].map(match => JSON.parse(match[1]));
  requireThat(rows.length === 4, `expected four MEASURE rows, found ${rows.length}`);
  return rows;
}

function main() {
  const options = parseArgs(process.argv.slice(2));
  const output = path.resolve(options.output);
  requireThat(path.relative(root, output).startsWith('..'), 'output directory must be outside the source tree');
  requireThat(!fs.existsSync(output), 'output directory must not exist yet');
  fs.mkdirSync(output, { recursive: true });
  const commit = gitText(['rev-parse', 'HEAD']).trim();
  const tree = gitText(['rev-parse', 'HEAD^{tree}']).trim();
  const status = gitText(['status', '--porcelain', '--untracked-files=all']);
  requireThat(status === '' || options.allowDirty, 'source worktree is dirty; re-run with --allow-dirty');
  const baseline = gitText(['rev-parse', `${options.baseline}^{commit}`]).trim();
  const tools = toolchain(output);
  const variants = {};
  for (const variant of ['baseline', 'current']) {
    const source = path.join(output, variant, 'source');
    archive(commit, source);
    if (variant === 'baseline') {
      for (const file of validatorFiles) fs.writeFileSync(path.join(source, file), gitText(['show', `${baseline}:${file}`]));
    }
    const rows = run(tools, source, path.join(output, variant, 'target'), path.join(output, variant, 'measure'));
    variants[variant] = {
      validator_sha256: Object.fromEntries(validatorFiles.map(file => [file, sha256(fs.readFileSync(path.join(source, file)))])),
      rows,
    };
  }
  const speedup = variants.current.rows.map((row, index) => {
    const before = variants.baseline.rows[index];
    requireThat(before.shape === row.shape && before.nodes === row.nodes, `row ${index} differs in shape between variants`);
    return {
      shape: row.shape, nodes: row.nodes, layers: row.layers,
      layout_check: Number((before.layout_check_median_ns / row.layout_check_median_ns).toFixed(1)),
      schedule_check: Number((before.schedule_check_median_ns / row.schedule_check_median_ns).toFixed(1)),
    };
  });
  const receipt = {
    schema: 'noble-ngraph-validation-cost/v1', result: 'measured',
    source_commit: commit, source_tree: tree, clean_source: status === '', baseline_commit: baseline,
    baseline_scope: `the current source with ${validatorFiles.join(' and ')} replaced by the baseline commit's`,
    harness: { path: harness, sha256: sha256(fs.readFileSync(path.join(root, harness))) },
    toolchain: { rustc: tools.version, vendor_nar_hash: tools.vendor.narHash },
    command: ['cargo', ...commandArgs],
    variants, median_speedup: speedup,
    instruments: {
      time: 'median wall-clock nanoseconds of five runs, release build, one thread',
      memory: 'resident-set growth across the first validation only; the workspace forbids unsafe_code, so no counting allocator exists',
    },
    non_claims: ['Reporting only: no timing threshold is a correctness claim.',
      'One host and one run; the numbers bound nothing on other hardware.'],
  };
  fs.writeFileSync(path.join(output, 'receipt.json'), `${JSON.stringify(receipt, null, 2)}\n`);
  process.stdout.write(`${JSON.stringify({ receipt: path.join(output, 'receipt.json'), median_speedup: speedup })}\n`);
}

try {
  main();
} catch (error) {
  console.error(`[validation-cost] FAIL ${error.message}`);
  process.exitCode = 1;
}
