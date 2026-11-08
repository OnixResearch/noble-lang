// Run the #96 effect-explanation prototype (`crates/noble-contracts/tests/source/causes.rs`) offline
// from a clean checkout. Retain its report with the source, rule and toolchain revisions. The report
// is finite agreement with the existing checker on fixed fixtures, not a soundness proof.
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { lstatSync, mkdirSync, readFileSync, realpathSync, writeFileSync } from 'node:fs';
import { basename, dirname, join, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { selectedVendor } from '../selected-vendor.mjs';

const root = resolve(fileURLToPath(import.meta.url), '../../..');
const moduleFile = 'crates/noble-contracts/tests/source/causes.rs';
const marker = 'NOBLE-EFFECT-CAUSES-REPORT ';
const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const requireThat = (condition, message) => { if (!condition) throw Error(message); };

function run(executable, argv, env) {
  const raw = spawnSync(executable, argv, { cwd: root, env, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 });
  requireThat(!raw.error && raw.status === 0, `${basename(executable)} ${argv.join(' ')} failed: ${raw.stderr}`);
  return raw.stdout;
}

function outputDirectory(requested) {
  const resolved = resolve(requested);
  const destination = join(realpathSync(dirname(resolved)), basename(resolved));
  const realRoot = realpathSync(root);
  requireThat(!destination.startsWith(`${realRoot}${sep}`), 'the output directory must be outside the worktree');
  mkdirSync(destination);
  for (const child of ['home', 'tmp', 'cargo-home']) mkdirSync(join(destination, child));
  return destination;
}

// The properties the receipt states: every fixture agrees with the kernel, every removed effect is
// refused, the cycles reach a fixed point, both seeded faults are seen, and the declared bound is
// accepted and labelled with no contributor.
function check(report) {
  requireThat(report.schema === 'noble-effect-causes/v1', `unexpected schema ${report.schema}`);
  for (const fixture of report.fixtures) {
    requireThat(fixture.compared.mismatches === 0, `${fixture.fixture}: disagrees with the kernel`);
    requireThat(fixture.unknown.length === 0 && fixture.incomplete === 0, `${fixture.fixture}: incomplete`);
    for (const row of fixture.explained) {
      requireThat(row.kernel_without_it === `refused: effect ${row.effect}` && row.path.length > 0,
        `${fixture.fixture}: effect ${row.effect} is not refused or not explained`);
    }
  }
  requireThat(report.cycles.seeded.effects.length === 1 && report.cycles.unseeded.effects.length === 0,
    'the synthetic cycles did not reach the expected fixed points');
  requireThat(report.faults.lost_edge_mismatches > 0 && report.faults.quotation_leak_mismatches > 0,
    'a seeded fault went unseen');
  const bound = report.declared_bound;
  requireThat(bound.kernel === 'accepted' && bound.bound_only.length === 1 && bound.contributors === 0,
    'the declared bound is not labelled as a bound');
}

function main() {
  const [requested] = process.argv.slice(2);
  requireThat(requested, 'usage: SELECTED_NODE verification/effect-causes/run.mjs NEW_EXTERNAL_DIRECTORY');
  const selection = JSON.parse(readFileSync(join(root, 'policy/tool-selection.json')));
  requireThat(realpathSync(process.execPath) === realpathSync(join(selection.tool_paths.node.output, 'bin/node')),
    'run this script with the selected Node');
  const out = outputDirectory(requested);
  const git = realpathSync('/run/current-system/sw/bin/git');
  const rustBin = join(selection.tool_paths.quality_rust.output, 'bin');
  const vendor = selectedVendor(selection.component_sync);
  writeFileSync(join(out, 'cargo-home', 'config.toml'),
    '[source.crates-io]\nreplace-with = "reviewed-vendor"\n[source.reviewed-vendor]\n' +
    `directory = ${JSON.stringify(vendor.directory)}\n[net]\noffline = true\n`);
  mkdirSync(join(root, 'target'), { recursive: true });
  const env = {
    HOME: join(out, 'home'), TMPDIR: join(out, 'tmp'), LANG: 'C.UTF-8', GIT_CONFIG_NOSYSTEM: '1',
    GIT_CONFIG_GLOBAL: '/dev/null', PATH: [rustBin, selection.component_sync.linker_bin, '/run/current-system/sw/bin'].join(':'),
    CARGO_HOME: join(out, 'cargo-home'), CARGO_TARGET_DIR: realpathSync(join(root, 'target')),
    CARGO_NET_OFFLINE: 'true', CARGO_INCREMENTAL: '0', RUSTC_WRAPPER: '', RUSTFLAGS: '',
    CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER: join(selection.component_sync.linker_bin, 'cc'),
  };
  const state = () => ({
    commit: run(git, ['rev-parse', 'HEAD'], env).trim(),
    tree: run(git, ['rev-parse', 'HEAD^{tree}'], env).trim(),
    status: run(git, ['status', '--porcelain=v1', '--untracked-files=all'], env),
  });
  const before = state();
  requireThat(before.status === '', `the source worktree is dirty:\n${before.status}`);
  const argv = ['test', '--locked', '--offline', '-j', '4', '-p', 'noble-contracts', '--test', 'source',
    'causes::report', '--', '--exact', '--nocapture'];
  const stdout = run(join(rustBin, 'cargo'), argv, env);
  const lines = stdout.split('\n').filter(line => line.startsWith(marker));
  requireThat(lines.length === 1, `expected one report line, found ${lines.length}`);
  const report = JSON.parse(lines[0].slice(marker.length));
  check(report);
  requireThat(JSON.stringify(state()) === JSON.stringify(before), 'the source changed during the run');
  const receipt = {
    schema: 'noble-effect-causes-receipt/v1', issue: 96,
    source_commit: before.commit, source_tree: before.tree, clean_source: true,
    rules: report.schema,
    sources_sha256: Object.fromEntries([moduleFile, 'crates/noble-contracts/tests/source.rs',
      'verification/effect-causes/run.mjs'].map(file => [file, digest(readFileSync(join(root, file)))])),
    toolchain: {
      cargo: run(join(rustBin, 'cargo'), ['--version'], env).trim(),
      rustc: run(join(rustBin, 'rustc'), ['--version'], env).trim(),
      vendor_nar_hash: vendor.narHash,
    },
    command: ['cargo', ...argv],
    report,
    non_claims: [
      'Finite agreement with acceptance::check on these fixtures; not a soundness proof of the relation.',
      'The report cannot accept a Program, authorize emission, narrow an admitted bound or certify a behavioural contract.',
      'Identities are session-local; module sessions, bound adapters and live dispatch are outside the fixtures.',
    ],
  };
  const path = join(out, 'receipt.json');
  writeFileSync(path, `${JSON.stringify(receipt, null, 2)}\n`, { flag: 'wx' });
  console.log(JSON.stringify({ result: 'passed', receipt: path, source_commit: before.commit,
    fixtures: report.fixtures.length, receipt_sha256: digest(readFileSync(path)) }));
}

try {
  main();
} catch (error) {
  console.error(`[effect-causes] FAIL ${error.message}`);
  process.exitCode = 1;
}
