// Run the #100 bounded principality differential (`crates/noble-contracts/tests/source/principality.rs`)
// offline from a clean checkout. Retain its report with the source and toolchain revisions. The report is
// bounded evidence over the stated universe, not a proof of principality.
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { lstatSync, mkdirSync, readFileSync, realpathSync, writeFileSync } from 'node:fs';
import { basename, dirname, join, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { selectedVendor } from '../selected-vendor.mjs';

const root = resolve(fileURLToPath(import.meta.url), '../../..');
const moduleFile = 'crates/noble-contracts/tests/source/principality.rs';
const marker = 'NOBLE-PRINCIPALITY-REPORT ';
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

// The properties the receipt states: every generated program was compared at every input stack, and
// every short program in every one-word context; no pair disagreed with its definition; and the two
// routes accepted equally often.
function check(report) {
  requireThat(report.schema === 'noble-principality-bounded/v2', `unexpected schema ${report.schema}`);
  const { plain, contextual } = report;
  requireThat(plain.comparisons === plain.pairs * report.input_stacks, 'a plain comparison is missing');
  requireThat(contextual.pairs === report.context_programs * report.words.length,
    'a program/context pair is missing');
  requireThat(contextual.comparisons === contextual.pairs * report.input_stacks,
    'a contextual comparison is missing');
  for (const tally of [plain, contextual]) {
    requireThat(tally.inline_accepted === tally.call_accepted, 'the two routes accepted different counts');
  }
  requireThat(report.discrepancies === 0 && report.retained.length === 0, 'a program disagrees with its definition');
  requireThat(/not a proof/.test(report.claim), 'the report does not state its bounded claim');
}

function main() {
  const [requested] = process.argv.slice(2);
  requireThat(requested, 'usage: SELECTED_NODE verification/principality/run.mjs NEW_EXTERNAL_DIRECTORY');
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
    'principality::report', '--', '--exact', '--nocapture'];
  const stdout = run(join(rustBin, 'cargo'), argv, env);
  const lines = stdout.split('\n').filter(line => line.startsWith(marker));
  requireThat(lines.length === 1, `expected one report line, found ${lines.length}`);
  const report = JSON.parse(lines[0].slice(marker.length));
  check(report);
  requireThat(JSON.stringify(state()) === JSON.stringify(before), 'the source changed during the run');
  const receipt = {
    schema: 'noble-principality-receipt/v1', issue: 100,
    source_commit: before.commit, source_tree: before.tree, clean_source: true,
    bounds: {
      words: report.words, max_words: report.max_words, max_context_words: report.max_context_words,
      context_programs: report.context_programs, max_depth: report.max_depth, value_types: report.value_types,
    },
    sources_sha256: Object.fromEntries([moduleFile, 'crates/noble-contracts/tests/source.rs',
      'verification/principality/run.mjs'].map(file => [file, digest(readFileSync(join(root, file)))])),
    toolchain: {
      cargo: run(join(rustBin, 'cargo'), ['--version'], env).trim(),
      rustc: run(join(rustBin, 'rustc'), ['--version'], env).trim(),
      vendor_nar_hash: vendor.narHash,
    },
    command: ['cargo', ...argv],
    report,
    non_claims: [
      'Bounded evidence over the stated words, program length, value types and stack depth; not a proof of principality.',
      'An interface the frontend cannot construct even at concrete inputs is outside the comparison; effects belong to #28.',
      'Recursive definitions are out of scope until #99 lands.',
    ],
  };
  const path = join(out, 'receipt.json');
  writeFileSync(path, `${JSON.stringify(receipt, null, 2)}\n`, { flag: 'wx' });
  console.log(JSON.stringify({ result: 'passed', receipt: path, source_commit: before.commit,
    programs: report.plain.pairs, comparisons: report.plain.comparisons + report.contextual.comparisons,
    receipt_sha256: digest(readFileSync(path)) }));
}

try {
  main();
} catch (error) {
  console.error(`[principality] FAIL ${error.message}`);
  process.exitCode = 1;
}
