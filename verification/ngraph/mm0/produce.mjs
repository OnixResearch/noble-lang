#!/usr/bin/env node
// Untrusted MM1 proof production. It cannot choose the expected specification:
// theory.mm0 and its flattened digest are caller-owned and checked first.
// Usage: node produce.mjs freeze|produce <pinned-mm0-source> <mm0-rs> <mm0-c> <outside-repo-output>
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { spawnSync, spawn } from 'node:child_process';
import { readFileSync, writeFileSync, mkdirSync, symlinkSync, statSync, copyFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const lane = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(lane, '../../..');
const [mode, sourceArg, producerArg, checkerArg, outputArg] = process.argv.slice(2);
assert.ok(['freeze', 'produce'].includes(mode) && sourceArg && producerArg && checkerArg && outputArg,
  'usage: node produce.mjs freeze|produce <source> <mm0-rs> <mm0-c> <outside-repo-output>');
const source = path.resolve(sourceArg);
const output = path.resolve(outputArg);
assert.ok(output !== root && !output.startsWith(`${root}${path.sep}`), 'output must be outside repository');
const producer = path.resolve(producerArg);
const checker = path.resolve(checkerArg);
const policy = JSON.parse(readFileSync(path.join(root, 'policy/tool-selection.json'))).experimental_mm0;
const profile = mode === 'produce' ? JSON.parse(readFileSync(path.join(lane, 'profile.json'))) : null;
const sha = (body) => createHash('sha256').update(body).digest('hex');
const fileSha = (file) => sha(readFileSync(file));
const command = (bin, args, options = {}) => {
  const started = process.hrtime.bigint();
  const result = spawnSync(bin, args, { encoding: 'utf8', timeout: 180_000,
    killSignal: 'SIGKILL', maxBuffer: 4 * 1024 * 1024, ...options });
  return { argv: [bin, ...args], status: result.status, signal: result.signal,
    error: result.error?.message ?? null, stdout: result.stdout ?? '', stderr: result.stderr ?? '',
    wall_ms: Number(process.hrtime.bigint() - started) / 1e6 };
};
function requireSuccess(run, name) {
  assert.equal(run.status, 0, `${name}: status=${run.status} signal=${run.signal} ${run.error ?? ''}\n${run.stderr}`);
}
async function archiveSha() {
  const child = spawn('git', ['archive', 'HEAD'], { cwd: source, stdio: ['ignore', 'pipe', 'pipe'] });
  const digest = createHash('sha256');
  let stderr = '';
  child.stdout.on('data', (chunk) => digest.update(chunk));
  child.stderr.on('data', (chunk) => { stderr += chunk; });
  const status = await new Promise((resolve, reject) => {
    child.on('error', reject);
    child.on('close', resolve);
  });
  assert.equal(status, 0, `source archive failed: ${stderr}`);
  return digest.digest('hex');
}
assert.equal(command('git', ['rev-parse', 'HEAD'], { cwd: source }).stdout.trim(), policy.source_rev,
  'wrong MM0 source revision');
assert.equal(await archiveSha(), policy.git_archive_sha256, 'MM0 source archive mismatch');
assert.equal(policy.parser_enabled, true, 'NO_PARSER is forbidden');
if (profile) {
  for (const [file, expected] of Object.entries(profile.inputs)) {
    assert.equal(fileSha(path.join(lane, file)), expected, `stale caller-owned input: ${file}`);
  }
  assert.equal(fileSha(producer), profile.producer_sha256, 'MM1 producer is not selected');
  assert.equal(fileSha(checker), profile.checker_sha256, 'MM0 checker is not selected');
  assert.equal(fileSha(path.join(source, 'mm0-rs/Cargo.lock')), profile.producer_lock_sha256);
  assert.equal(fileSha(path.join(source, 'mm0-c/main.c')), profile.checker_source_sha256);
}
mkdirSync(output, { recursive: true, mode: 0o700 });
for (const file of ['peano.mm0', 'peano_hex.mm0', 'peano.mm1', 'peano_hex.mm1']) {
  const target = path.join(output, file);
  // A fresh outside-repo directory is required; never silently reuse stale imports.
  symlinkSync(path.join(source, 'examples', file), target);
}
for (const file of ['theory.mm0', 'proof.mm1']) {
  copyFileSync(path.join(lane, file), path.join(output, file));
}
const join = command(producer, ['join', '-H', '-b', 'theory.mm0', 'spec.mm0'], { cwd: output });
requireSuccess(join, 'flatten frozen theory');
const spec = path.join(output, 'spec.mm0');
if (profile) assert.equal(fileSha(spec), profile.spec_sha256, 'flattened expected theory differs');
if (mode === 'freeze') {
  console.log(JSON.stringify({ source_rev: policy.source_rev, spec_sha256: fileSha(spec),
    spec_bytes: statSync(spec).size, flatten_ms: join.wall_ms }));
} else {
  const git = command('git', ['rev-parse', 'HEAD'], { cwd: root });
  requireSuccess(git, 'source commit');
  const dirty = command('git', ['status', '--porcelain'], { cwd: root });
  requireSuccess(dirty, 'clean source');
  assert.equal(dirty.stdout, '', 'commit source before producing evidence');
  const generation = command(producer, ['compile', '-W', 'proof.mm1', 'proof.mmb'], { cwd: output });
  requireSuccess(generation, 'produce MMB');
  const controlsDir = path.join(output, 'controls');
  mkdirSync(controlsDir, { mode: 0o700 });
  const controlProofs = {};
  for (const name of ['different', 'forged', 'sorry']) {
    for (const ext of ['mm0', 'mm1']) {
      copyFileSync(path.join(lane, 'controls', `${name}.${ext}`),
        path.join(controlsDir, `${name}.${ext}`));
    }
    const args = ['compile', ...(name === 'sorry' ? [] : ['-W']),
      `${name}.mm1`, `${name}.mmb`];
    const run = command(producer, args, { cwd: controlsDir });
    requireSuccess(run, `produce ${name} control`);
    controlProofs[name] = { proof_sha256: fileSha(path.join(controlsDir, `${name}.mmb`)),
      proof_bytes: statSync(path.join(controlsDir, `${name}.mmb`)).size,
      producer_status: run.status, producer_stderr: run.stderr };
  }
  const proof = path.join(output, 'proof.mmb');
  const checking = command('/usr/bin/time', ['-f', '%e %M', '-o', path.join(output, 'checker.time'),
    'prlimit', '--as=536870912', '--cpu=20', '--', checker, proof],
  { cwd: output, input: readFileSync(spec) });
  requireSuccess(checking, 'parser-enabled MM0 checker');
  const [elapsed, rss] = readFileSync(path.join(output, 'checker.time'), 'utf8').trim().split(' ');
  const receipt = {
    schema: 'noble-mm0-production/v1', source_commit: git.stdout.trim(), clean_source: true,
    mm0_source: { revision: policy.source_rev, git_archive_sha256: policy.git_archive_sha256 },
    implementation: { checker_binary_sha256: fileSha(checker),
      checker_source_sha256: profile.checker_source_sha256,
      producer_binary_sha256: fileSha(producer), producer_lock_sha256: profile.producer_lock_sha256,
      checker_build: policy.checker_build, producer_build: policy.producer_build,
      compiler: command('gcc', ['--version']).stdout.split('\n')[0],
      checker_flags: policy.checker_flags, parser_enabled: true },
    artifact: { spec_sha256: fileSha(spec), spec_bytes: statSync(spec).size,
      proof_sha256: fileSha(proof), proof_bytes: statSync(proof).size },
    control_proofs: controlProofs,
    generation: { wall_ms: generation.wall_ms, status: generation.status,
      stderr: generation.stderr, stdout: generation.stdout },
    checking: { wall_ms: checking.wall_ms, checker_elapsed_s: Number(elapsed),
      checker_peak_rss_kib: Number(rss), status: checking.status, signal: checking.signal,
      stdout: checking.stdout, stderr: checking.stderr,
      limits: { address_space_bytes: 536870912, cpu_seconds: 20, wall_ms: 180000 } },
  };
  writeFileSync(path.join(output, 'production.json'), `${JSON.stringify(receipt, null, 2)}\n`);
  console.log(JSON.stringify({ source_commit: receipt.source_commit,
    proof_sha256: receipt.artifact.proof_sha256,
    generation_ms: receipt.generation.wall_ms, checker_ms: receipt.checking.wall_ms }));
}
