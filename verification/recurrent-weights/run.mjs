#!/usr/bin/env node
import { spawnSync } from 'node:child_process';
import { writeFileSync } from 'node:fs';
import { resolve } from 'node:path';

function argument(name) {
  const index = process.argv.indexOf(name);
  if (index < 0 || index + 1 >= process.argv.length) throw new Error(`missing ${name}`);
  return process.argv[index + 1];
}

const root = resolve(argument('--root'));
const cargo = resolve(argument('--cargo'));
const output = resolve(argument('--output'));
const git = (args) => {
  const result = spawnSync('git', ['-C', root, ...args], { encoding: 'utf8' });
  if (result.status !== 0) throw new Error(result.stderr || `git ${args[0]} failed`);
  return result.stdout.trim();
};
const status = git(['status', '--porcelain']);
if (status !== '') throw new Error(`receipt requires a clean committed source tree: ${status}`);
const sourceCommit = git(['rev-parse', 'HEAD']);
const test = spawnSync(cargo, [
  'test', '-p', 'noble-ngraph', '--test', 'layout_differential', '--', '--nocapture', 'recurrent',
], { cwd: root, encoding: 'utf8', env: process.env });
const artifacts = test.stdout.split('\n').filter((line) => line.startsWith('RECURRENT_MANIFEST\t'))
  .map((line) => {
    const [, program, graphId, snapshotId, weightBytes] = line.split('\t');
    return { program, graph_id: graphId, snapshot_id: snapshotId, weight_bytes: weightBytes };
  });
const trajectories = test.stdout.split('\n').filter((line) => line.startsWith('RECURRENT_TRAJECTORY\t'))
  .map((line) => {
    const [, mode, steps] = line.split('\t');
    return { mode, steps: Number(steps) };
  });
if (test.status === 0 && (artifacts.length !== 2 || trajectories.length !== 2
  || trajectories.some(({ steps }) => steps !== 1_000))) {
  throw new Error('successful lane omitted an artifact identity or full trajectory receipt');
}
const receipt = {
  schema: 'noble.recurrent-weights.test-run.v0',
  source_commit: sourceCommit,
  manifest: {
    compiler_revision: 'ngraph-layered/0',
    target_format: 'ngraph-f32/0 exact-bit threshold graph',
    state_codec: 'unsigned 64-bit words, little-endian, one exact +0/1 F32 per bit',
    numeric_contract: 'bit-exact binary32 values only; accumulator wraps modulo 2^64; no I64-to-F32 casts',
    runtime: 'noble-ngraph reference and compiled CPU evaluators; no exported or stock runtime',
    execution_limits: { max_semantic_steps: 1000, max_unrolled_steps: 4 },
    identity_scheme: 'test-only common::TestScheme; stable for the pinned test toolchain, not a production digest',
  },
  command: ['cargo', 'test', '-p', 'noble-ngraph', '--test', 'layout_differential', '--', '--nocapture', 'recurrent'],
  exit_code: test.status,
  artifacts,
  trajectories,
  stdout: test.stdout,
  stderr: test.stderr,
  note: 'Correctness test output only; no performance or runtime claim.',
};
writeFileSync(output, `${JSON.stringify(receipt, null, 2)}\n`, { flag: 'wx' });
if (test.status !== 0) process.exit(test.status ?? 1);
