// Issue #64 cost-lane receipt runner: run the certified-rewrite cost lane and record the report with
// the source, toolchain and host it was measured under.
//
// The lane itself never executes a program, so this receipt prices proposal and certificate-check
// work only; every stage it cannot observe is reported as unmeasured with a reason, never as zero.
// The continuation thresholds come from `verification/ngraph/rewrite-cost-v1.txt`, frozen before
// the run.
import { createHash } from 'node:crypto';
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
// The devShell's `bash` is the host profile's wrapper and replaces PATH with the host profile's,
// which has no `cargo` (exit 127 when this runner was replayed); `sh` keeps the devShell PATH.
const text = run(['sh', '-c', command], root);

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

// nix develop writes store/GC progress to the same stream, so a version line is taken from the
// first line that carries one rather than from the whole capture.
function versionLine(text) {
  const line = text.split('\n').map((part) => part.trim())
    .find((part) => /^(rustc|cargo)\s+\d/.test(part));
  if (line === undefined) {
    throw new Error(`no toolchain version line in:\n${text.slice(-2000)}`);
  }
  return line;
}

function git(...argv) {
  const result = spawnSync('git', argv, { cwd: root, encoding: 'utf8' });
  if (result.status !== 0) {
    throw new Error(`git ${argv.join(' ')} failed: ${result.stderr}`);
  }
  return result.stdout.trim();
}

// Item 1 of issue #64 asks the corpus binding to include source/model/tool identities. The
// manifest names its lens (`lens noble-ngraph cert.rs`); that path is resolved to every file in the
// named crate's `src` tree and digested here, because noble-ngraph is deliberately
// dependency-free and cannot hash itself. A source edit that the corpus was not re-reviewed
// against therefore changes the receipt instead of passing silently.
function lensDigests(manifestPath) {
  const lines = fs.readFileSync(path.join(root, manifestPath), 'utf8').split('\n');
  const digests = [];
  for (const line of lines) {
    const trimmed = line.trim();
    if (!trimmed.startsWith('lens ')) continue;
    const [crate, module = ''] = trimmed.slice('lens '.length).trim().split(/\s+/);
    const sourceRoot = path.join(root, 'crates', crate, 'src');
    if (!fs.existsSync(sourceRoot)) {
      throw new Error(`lens crate has no source tree: ${crate}`);
    }
    const files = [];
    const walk = (dir) => {
      for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
        const full = path.join(dir, entry.name);
        if (entry.isDirectory()) walk(full);
        else if (entry.name.endsWith('.rs')) files.push(full);
      }
    };
    walk(sourceRoot);
    files.sort();
    const hashed = files.map((file) => ({
      path: path.relative(root, file),
      sha256: createHash('sha256').update(fs.readFileSync(file)).digest('hex'),
    }));
    digests.push({ lens: `${crate} ${module}`.trim(), crate, files: hashed });
  }
  if (digests.length === 0) {
    throw new Error(`no lens declared in ${manifestPath}`);
  }
  return digests;
}

const manifestPath = path.join('verification', 'ngraph', 'rewrite-cost-v1.txt');
const corpusManifestPath = path.join('verification', 'ngraph', 'rewrite-quality-v1.txt');
const receipt = {
  schema: 'noble-rewrite-cost-receipt/v1',
  source_commit: git('rev-parse', 'HEAD'),
  source_tree: git('rev-parse', 'HEAD^{tree}'),
  clean_source: git('status', '--porcelain') === '',
  toolchain: versionLine(run(['rustc', '--version', '--verbose'], root)),
  host: `${os.type()} ${os.release()} ${os.arch()}`,
  command,
  manifest: manifestPath,
  corpus: report.corpus,
  lens: lensDigests(corpusManifestPath),
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
