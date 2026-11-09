// Run the #100 bounded principality differential (`crates/noble-contracts/tests/source/principality.rs`)
// and its signed-recursion part (`principality/signed.rs`) offline from a clean checkout. Retain both
// reports with the source and toolchain revisions. They are bounded evidence over the stated universes,
// not a proof of principality.
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { lstatSync, mkdirSync, readFileSync, realpathSync, writeFileSync } from 'node:fs';
import { basename, dirname, join, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { selectedVendor } from '../selected-vendor.mjs';

const root = resolve(fileURLToPath(import.meta.url), '../../..');
const moduleFile = 'crates/noble-contracts/tests/source/principality.rs';
const signedFile = 'crates/noble-contracts/tests/source/principality/signed.rs';
const marker = 'NOBLE-PRINCIPALITY-REPORT ';
const signedMarker = 'NOBLE-PRINCIPALITY-SIGNED-REPORT ';
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

// The properties the signed part states: every body was declared at every signature, and every
// accepted declaration was called at every input stack; the calls agree with the declared signatures;
// every declared instance of an accepted signature accepts the body too; and refusals exist at the
// signatures that are not instances, so the closure is not vacuous.
function checkSigned(report) {
  requireThat(report.schema === 'noble-principality-signed/v1', `unexpected signed schema ${report.schema}`);
  const { counts } = report;
  requireThat(counts.pairs === counts.bodies * report.signatures.length, 'a body/signature pair is missing');
  requireThat(counts.comparisons === counts.accepted * report.input_stacks, 'a signed call comparison is missing');
  requireThat(counts.expected_accepted === counts.call_accepted,
    'the calls and the declared signatures accepted different counts');
  requireThat(counts.instance_checks > 0 && counts.instance_accepted === counts.instance_checks,
    'a declared instance refused an accepted body');
  requireThat(counts.non_instance_refusals > 0, 'the instance closure is vacuous');
  requireThat(report.discrepancies === 0 && report.retained.length === 0,
    'a signed definition disagrees with its declared signature');
  requireThat(/not a proof/.test(report.claim), 'the signed report does not state its bounded claim');
}

function reportLine(stdout, prefix) {
  const lines = stdout.split('\n').filter(line => line.startsWith(prefix));
  requireThat(lines.length === 1, `expected one ${prefix.trim()} line, found ${lines.length}`);
  return JSON.parse(lines[0].slice(prefix.length));
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
    '--', 'principality::report', 'principality::signed::report', '--exact', '--nocapture'];
  const stdout = run(join(rustBin, 'cargo'), argv, env);
  const report = reportLine(stdout, marker);
  const signed = reportLine(stdout, signedMarker);
  check(report);
  checkSigned(signed);
  requireThat(JSON.stringify(state()) === JSON.stringify(before), 'the source changed during the run');
  const receipt = {
    schema: 'noble-principality-receipt/v2', issue: 100,
    source_commit: before.commit, source_tree: before.tree, clean_source: true,
    bounds: {
      words: report.words, max_words: report.max_words, max_context_words: report.max_context_words,
      context_programs: report.context_programs, max_depth: report.max_depth, value_types: report.value_types,
    },
    signed_bounds: {
      self_words: signed.self_words, max_body_words: signed.max_body_words, signatures: signed.signatures,
      instance_edges: signed.instance_edges, max_depth: signed.max_depth, value_types: signed.value_types,
    },
    sources_sha256: Object.fromEntries([moduleFile, signedFile, 'crates/noble-contracts/tests/source.rs',
      'verification/principality/run.mjs'].map(file => [file, digest(readFileSync(join(root, file)))])),
    toolchain: {
      cargo: run(join(rustBin, 'cargo'), ['--version'], env).trim(),
      rustc: run(join(rustBin, 'rustc'), ['--version'], env).trim(),
      vendor_nar_hash: vendor.narHash,
    },
    command: ['cargo', ...argv],
    report,
    signed_report: signed,
    non_claims: [
      'Bounded evidence over the stated words, program length, value types and stack depth; not a proof of principality.',
      'An interface the frontend cannot construct even at concrete inputs is outside the comparison; effects belong to #28.',
      'Signed recursive definitions are judged against their declared signatures, at declared signatures over I64 and Bool only; unsigned recursion stays refused and is not compared.',
    ],
  };
  const path = join(out, 'receipt.json');
  writeFileSync(path, `${JSON.stringify(receipt, null, 2)}\n`, { flag: 'wx' });
  console.log(JSON.stringify({ result: 'passed', receipt: path, source_commit: before.commit,
    programs: report.plain.pairs, comparisons: report.plain.comparisons + report.contextual.comparisons,
    signed_pairs: signed.counts.pairs, signed_comparisons: signed.counts.comparisons,
    receipt_sha256: digest(readFileSync(path)) }));
}

try {
  main();
} catch (error) {
  console.error(`[principality] FAIL ${error.message}`);
  process.exitCode = 1;
}
