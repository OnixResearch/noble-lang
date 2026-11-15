// Issue #32 acceptance item 6: process-global DHAT peak and total for frozen scopes.
// Build the rewrite_cost test target from a clean committed source with the pinned
// Cargo toolchain, then run this source-bound collector with --binary and --output.
import fs from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const valgrind = '/nix/store/39iybd55afmpqlgf979ra06wj3q3wapa-valgrind-3.27.1/bin/valgrind';
const graphFixtures = [
  'chained-add', 'chained-sub', 'wrapping-boundary', 'shared-subexpression',
  'non-prefix-static', 'dead-total-forward', 'growth-cap',
];
const programFixtures = ['opaque-tail', 'nested-partial-pair', 'named-repeated-call'];
const insist = (condition, message) => { if (!condition) throw new Error(message); };
const digest = bytes => createHash('sha256').update(bytes).digest('hex');

function git(args) {
  const result = spawnSync('git', args, { cwd: root, encoding: 'utf8' });
  insist(result.status === 0, `git ${args.join(' ')} failed: ${result.stderr}`);
  return result.stdout.trim();
}

function argumentsOf(argv) {
  insist(argv.length === 4 && argv[0] === '--binary' && argv[2] === '--output',
    'usage: node verification/ngraph/pe-scratch.mjs --binary /abs/rewrite_cost-... --output /new/abs/dir');
  const binary = path.resolve(argv[1]);
  const output = path.resolve(argv[3]);
  insist(path.isAbsolute(argv[1]) && path.isAbsolute(argv[3]), 'binary and output must be absolute paths');
  insist(fs.statSync(binary).isFile(), 'the supplied rewrite_cost test executable is not a file');
  const relative = path.relative(root, output);
  insist(relative === '..' || relative.startsWith(`..${path.sep}`), 'output must be outside source tree');
  insist(!fs.existsSync(output), 'the evidence directory must not exist');
  return { binary, output };
}

function number(text, label) {
  const result = Number(text.replaceAll(',', ''));
  insist(Number.isSafeInteger(result) && result > 0, `${label} is not a positive safe integer: ${text}`);
  return result;
}

function measure(binary, output, fixture, side) {
  const stem = `${fixture}-${side}`;
  const jsonFile = path.join(output, `${stem}.dhat.json`);
  const logFile = path.join(output, `${stem}.valgrind.log`);
  const stdoutFile = path.join(output, `${stem}.stdout`);
  const stderrFile = path.join(output, `${stem}.stderr`);
  const command = [
    '--tool=dhat', `--dhat-out-file=${jsonFile}`, `--log-file=${logFile}`,
    '--error-exitcode=96', binary, '--exact', 'offset::scratch::profile_case', '--ignored', '--nocapture',
  ];
  const result = spawnSync(valgrind, command, {
    cwd: output, encoding: 'utf8', maxBuffer: 16 * 1024 * 1024, timeout: 900_000,
    env: { PATH: '/run/current-system/sw/bin:/usr/bin:/bin', HOME: output,
      NOBLE_PE_HEAP_CASE: fixture, NOBLE_PE_HEAP_SIDE: side },
  });
  fs.writeFileSync(stdoutFile, result.stdout ?? '');
  fs.writeFileSync(stderrFile, result.stderr ?? '');
  insist(result.status === 0 && !result.error,
    `${stem}: valgrind returned ${result.status ?? result.signal ?? result.error?.code}: ${result.stderr}`);
  insist(result.stdout.includes(`PE_HEAP:${fixture}:${side}:checked`), `${stem}: selected leg did not complete`);
  const log = fs.readFileSync(logFile, 'utf8');
  const total = /Total:\s+([\d,]+) bytes in ([\d,]+) blocks/.exec(log);
  const peak = /At t-gmax:\s+([\d,]+) bytes in ([\d,]+) blocks/.exec(log);
  insist(total && peak, `${stem}: DHAT did not report both Total and At t-gmax`);
  const raw = JSON.parse(fs.readFileSync(jsonFile, 'utf8'));
  insist(Array.isArray(raw.ftbl) && Array.isArray(raw.pps), `${stem}: malformed DHAT JSON tables`);
  const files = [jsonFile, logFile, stdoutFile, stderrFile].map(file => ({
    path: path.basename(file), sha256: digest(fs.readFileSync(file)),
  }));
  return {
    fixture, side, peak_live_bytes: number(peak[1], `${stem} peak bytes`),
    peak_live_blocks: number(peak[2], `${stem} peak blocks`),
    total_allocated_bytes: number(total[1], `${stem} total bytes`),
    total_allocated_blocks: number(total[2], `${stem} total blocks`), files,
  };
}

function main() {
  const { binary, output } = argumentsOf(process.argv.slice(2));
  insist(git(['status', '--porcelain', '--untracked-files=all']) === '', 'source tree must be clean');
  const sourceCommit = git(['rev-parse', 'HEAD']);
  const sourceTree = git(['rev-parse', 'HEAD^{tree}']);
  const version = spawnSync(valgrind, ['--version'], { encoding: 'utf8' });
  insist(version.status === 0 && version.stdout.trim() === 'valgrind-3.27.1', 'Valgrind version is not the frozen 3.27.1');
  const harnessPath = 'crates/noble-ngraph/tests/rewrite_cost/offset/scratch.rs';
  const committedHarness = spawnSync('git', ['show', `${sourceCommit}:${harnessPath}`], {
    cwd: root, encoding: null,
  });
  insist(committedHarness.status === 0, 'the test harness must be committed');
  fs.mkdirSync(output);
  const rows = [];
  for (const fixture of graphFixtures) {
    for (const side of ['setup', 'baseline', 'offset']) {
      rows.push(measure(binary, output, fixture, side));
    }
  }
  for (const fixture of programFixtures) {
    for (const side of ['setup', 'program']) {
      rows.push(measure(binary, output, fixture, side));
    }
  }
  insist(rows.length === 27, 'the frozen process roster changed');
  const comparison = [...graphFixtures, ...programFixtures].map(fixture => {
    const setup = rows.find(row => row.fixture === fixture && row.side === 'setup');
    const other = rows.filter(row => row.fixture === fixture && row.side !== 'setup');
    return { fixture, setup_peak_live_bytes: setup.peak_live_bytes,
      other: other.map(row => ({ side: row.side, peak_live_bytes: row.peak_live_bytes,
        peak_minus_setup_bytes: row.peak_live_bytes - setup.peak_live_bytes,
        total_allocated_bytes: row.total_allocated_bytes,
        total_minus_setup_bytes: row.total_allocated_bytes - setup.total_allocated_bytes })) };
  });
  const receipt = {
    schema: 'noble-pe-scratch-dhat/v1', issue: 32, protocol_commit: '8bca226',
    source_commit: sourceCommit, source_tree: sourceTree, clean_source: true,
    valgrind: { path: valgrind, version: version.stdout.trim(), tool: 'dhat' },
    binary: { path: binary, sha256: digest(fs.readFileSync(binary)) },
    harness: { path: harnessPath, sha256: digest(committedHarness.stdout) },
    invocation: [valgrind, '--tool=dhat', '--dhat-out-file=<raw>', '--log-file=<raw>', '--error-exitcode=96',
      binary, '--exact', 'offset::scratch::profile_case', '--ignored', '--nocapture'],
    scope: 'one libtest process per side; process-global peak includes harness, fixture construction, transformation and independent readmission; setup differences are descriptive, not isolated scratch bounds',
    non_claims: ['Total is cumulative allocated bytes, not peak live bytes.',
      'DHAT changes timing and these runs are not a timing or efficiency comparison.',
      'A binary digest is recorded; independent build-to-source correspondence is not proved by this receipt.'],
    rows, comparison,
  };
  fs.writeFileSync(path.join(output, 'receipt.json'), `${JSON.stringify(receipt, null, 2)}\n`);
  process.stdout.write(`${JSON.stringify({ result: 'measured', rows: rows.length, source_commit: sourceCommit,
    receipt: path.join(output, 'receipt.json') })}\n`);
}

try { main(); } catch (error) { console.error(`[pe-scratch] FAIL ${error.message}`); process.exitCode = 1; }
