// Exercise the retained subject decoder's stale-digest refusal using a changed byte copy.
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { mkdirSync, readFileSync, realpathSync, writeFileSync } from 'node:fs';
import { dirname, isAbsolute, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const lane = join(root, 'verification/wasm-refinement-pilot');
const selection = JSON.parse(readFileSync(join(root, 'policy/tool-selection.json'), 'utf8'));
const nodePath = join(selection.tool_paths.node.output, 'bin/node');
const fail = message => { throw new Error(message); };
const requireThat = (condition, message) => { if (!condition) fail(message); };
const sha256 = bytes => createHash('sha256').update(bytes).digest('hex');

function main() {
  requireThat(process.argv.length === 6 && process.argv[2] === '--work-dir' && process.argv[3] !== '' &&
      process.argv[4] === '--output' && process.argv[5] !== '',
    'usage: controls.mjs --work-dir ABS_PATH --output NEW_DIR_OUTSIDE_REPOSITORY');
  requireThat(realpathSync(process.execPath) === realpathSync(nodePath), 'process is not the policy-selected Node executable');
  requireThat(Object.keys(process.env).length === 0, 'refusing ambient environment variables');
  const workDir = resolve(process.argv[3]);
  requireThat(isAbsolute(process.argv[3]) && workDir !== root && !workDir.startsWith(`${root}/`),
    '--work-dir must be an absolute path outside the repository');
  const prepared = JSON.parse(readFileSync(join(workDir, 'talos-source.json'), 'utf8'));
  requireThat(prepared.schema === 'wasm-pilot-talos-preparation/v1' &&
    prepared.upstream_commit === 'd7c84ac660757953b752bdfc9cb8dae3dab623ed' && /^[0-9a-f]{40}$/.test(prepared.upstream_tree),
    'work directory does not contain the required pinned Talos preparation receipt');
  const output = resolve(process.argv[5]);
  requireThat(output !== root && !output.startsWith(`${root}/`), 'output directory must be outside the repository');
  const git = '/run/current-system/sw/bin/git';
  const sourceCommit = spawnSync(git, ['rev-parse', 'HEAD'], { cwd: root, env: {}, encoding: 'utf8' });
  requireThat(sourceCommit.status === 0, `git rev-parse failed: ${sourceCommit.stderr}`);
  requireThat(prepared.source_commit === sourceCommit.stdout.trim(),
    'prepared project belongs to a different Noble source commit');
  const status = spawnSync(git, ['status', '--porcelain'], { cwd: root, env: {}, encoding: 'utf8' });
  requireThat(status.status === 0 && status.stdout.trim() === '', `receipts require a clean committed source tree: ${status.stdout}`);
  mkdirSync(output);
  const sums = readFileSync(join(root, 'verification/owi/receipts/2026-10-26/sha256sums.txt'), 'utf8');
  const artifact = 'verification/owi/receipts/2026-10-26/subject-module-1.wasm';
  const expected = sums.split('\n').map(line => line.match(/^([0-9a-f]{64})\s+(.+)$/))
    .find(match => match?.[2] === artifact)?.[1];
  requireThat(expected, `retained digest missing for ${artifact}`);
  const original = readFileSync(join(root, artifact));
  requireThat(sha256(original) === expected, 'retained subject does not match its sha256sums entry');
  const tampered = Buffer.from(original);
  tampered[0] ^= 1;
  const tamperedPath = join(output, 'tampered-subject.wasm');
  writeFileSync(tamperedPath, tampered, { flag: 'wx' });
  const observed = sha256(tampered);
  requireThat(observed !== expected, 'one-byte mutation unexpectedly retained the subject digest');
  const childOutput = join(output, 'stale-decoder-run');
  const child = spawnSync(nodePath, [join(lane, 'decode.mjs'), '--work-dir', workDir,
    '--output', childOutput, '--artifact-bytes', tamperedPath], {
    cwd: root, env: {}, encoding: 'utf8', maxBuffer: 16 * 1024 * 1024,
  });
  requireThat(child.error === undefined, `decoder launch failed: ${child.error}`);
  requireThat(child.status === 1, `stale-digest control expected refusal (1), observed ${child.status}: ${child.stdout}${child.stderr}`);
  const diagnostic = child.stderr.trim();
  requireThat(diagnostic.includes(`subject sha256 ${observed} does not match retained sha256 ${expected}`),
    `stale-digest refusal diagnostic was not exact: ${diagnostic}`);
  const receipt = {
    schema: 'wasm-pilot-controls/v1',
    source_commit: sourceCommit.stdout.trim(),
    source_worktree_clean: true,
    talos_source_commit: prepared.upstream_commit,
    talos_source_tree: prepared.upstream_tree,
    stale_digest_refusal: {
      artifact,
      retained_sha256: expected,
      tampered_sha256: observed,
      tampered_artifact_path: tamperedPath,
      decoder_exit: child.status,
      decoder_stderr: diagnostic,
      outcome: 'refused-before-wasm-decoding',
    },
  };
  writeFileSync(join(output, 'controls.json'), `${JSON.stringify(receipt, null, 2)}\n`, { flag: 'wx' });
  process.stdout.write(`[wasm-pilot-controls] PASS stale_digest_refusal=1 commit=${receipt.source_commit}\n`);
}

try { main(); }
catch (error) { process.stderr.write(`[wasm-pilot-controls] FAIL ${error.message}\n`); process.exitCode = 1; }
