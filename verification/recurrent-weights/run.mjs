#!/usr/bin/env node
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { writeFileSync } from 'node:fs';
import { release } from 'node:os';
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
const testCommand = [
  'test', '-p', 'noble-ngraph', '--test', 'layout_differential', '--', '--nocapture', 'recurrent',
];
const test = spawnSync(cargo, testCommand, { cwd: root, encoding: 'utf8', env: process.env });
const measurementCommand = [
  'test', '-p', 'noble-ngraph', '--test', 'layout_differential', '--', '--ignored', '--nocapture', 'recurrent_unrolled_cost_protocol',
];
const measurement = spawnSync(cargo, measurementCommand, { cwd: root, encoding: 'utf8', env: process.env });
const artifacts = test.stdout.split('\n').filter((line) => line.startsWith('RECURRENT_MANIFEST\t'))
  .map((line) => {
    const [, program, graphId, snapshotId, weightBytes, graphBytes] = line.split('\t');
    const hash = (hex) => createHash('sha256').update(Buffer.from(hex, 'hex')).digest('hex');
    return {
      program,
      graph_id_test_scheme: graphId,
      graph_canonical_bytes: Buffer.from(graphBytes, 'hex').length,
      graph_canonical_sha256: hash(graphBytes),
      snapshot_id_test_scheme: snapshotId,
      snapshot_weight_bytes: Buffer.from(weightBytes, 'hex').length,
      snapshot_weight_sha256: hash(weightBytes),
    };
  });
const trajectories = test.stdout.split('\n').filter((line) => line.startsWith('RECURRENT_TRAJECTORY\t'))
  .map((line) => {
    const [, program, mode, steps] = line.split('\t');
    return { program, mode, steps: Number(steps) };
  });
const costRows = measurement.stdout.split('\n')
  .filter((line) => line.startsWith('RECURRENT_COST\t'))
  .map((line) => JSON.parse(line.split('\t')[1]));
const costSummaries = measurement.stdout.split('\n')
  .filter((line) => line.startsWith('RECURRENT_COST_SUMMARY\t'))
  .map((line) => JSON.parse(line.split('\t')[1]));
if (test.status === 0 && (artifacts.length !== 2 || trajectories.length !== 4
  || trajectories.some(({ steps }) => steps !== 1_000))) {
  throw new Error('successful lane omitted an artifact identity or full trajectory receipt');
}
if (measurement.status === 0 && (costRows.length !== 40 || costSummaries.length !== 2)) {
  throw new Error('successful measurement omitted a repetition or program summary');
}
const receipt = {
  schema: 'noble.recurrent-weights.test-run.v0',
  source_commit: sourceCommit,
  runner_path: cargo,
  host: { platform: process.platform, architecture: process.arch, kernel_release: release(), node: process.version },
  manifest: {
    compiler_revision: 'ngraph-layered/0',
    target_format: 'ngraph-f32/0 exact-bit threshold graph',
    state_codec: 'unsigned 64-bit words, little-endian, one exact +0/1 F32 per bit',
    numeric_contract: 'bit-exact binary32 values only; accumulator wraps modulo 2^64; no I64-to-F32 casts',
    runtime: 'noble-ngraph reference and compiled CPU evaluators; no exported or stock runtime',
    inference_runtime_coverage: { dense: 'none', sparse: 'none', stock: 'none' },
    build_profile: 'cargo test profile (debug assertions enabled)',
    execution_limits: { max_semantic_steps: 1000, max_unrolled_steps: 4 },
    identity_scheme: 'test-only common::TestScheme; stable for the pinned test toolchain, not a production digest',
  },
  command: testCommand,
  exit_code: test.status === 0 && measurement.status === 0 ? 0 : (test.status || measurement.status || 1),
  artifacts,
  trajectories,
  measurement: {
    command: measurementCommand,
    exit_code: measurement.status,
    repetitions: costRows,
    summaries: costSummaries,
    stdout: measurement.stdout,
    stderr: measurement.stderr,
  },
  stdout: test.stdout.split('\n').filter((line) => !line.startsWith('RECURRENT_MANIFEST\t')).join('\n'),
  stderr: test.stderr,
  note: 'Performance evidence is the bounded five-repetition CPU timing and process-RSS sample set; identity digests use canonical graph bytes and exact snapshot weight bytes.',
};
writeFileSync(output, `${JSON.stringify(receipt, null, 2)}\n`, { flag: 'wx' });
if (test.status !== 0 || measurement.status !== 0) process.exit(test.status || measurement.status || 1);
