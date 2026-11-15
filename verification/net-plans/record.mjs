import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { readFile, stat, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const [testBinary, outputPath] = process.argv.slice(2);
if (!testBinary || !outputPath || !path.isAbsolute(testBinary) || !path.isAbsolute(outputPath)) {
  throw new Error('usage: node verification/net-plans/record.mjs <absolute test executable> <absolute output outside repository>');
}
const repo = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const output = path.resolve(outputPath);
const relativeOutput = path.relative(repo, output);
if (relativeOutput === '' || (relativeOutput !== '..' && !relativeOutput.startsWith(`..${path.sep}`))) {
  throw new Error('measurement output must be outside the repository');
}
const clean = spawnSync('git', ['status', '--porcelain', '--untracked-files=all'], { cwd: repo, encoding: 'utf8' });
if (clean.status !== 0) throw new Error(`git status failed: ${clean.stderr}`);
if (clean.stdout.trim() !== '') throw new Error('measurement requires a clean committed source tree');
const sourceCommit = spawnSync('git', ['rev-parse', 'HEAD'], { cwd: repo, encoding: 'utf8' });
if (sourceCommit.status !== 0) throw new Error(`git rev-parse failed: ${sourceCommit.stderr}`);
const inets = JSON.parse(await readFile(path.join(repo, 'proofs/inets/UPSTREAM.json'), 'utf8'));
const inetsSha256 = createHash('sha256')
  .update(await readFile(path.join(repo, 'proofs/inets/InteractionNets.lean')))
  .digest('hex');
if (inets.sha256 !== inetsSha256 || !/^[a-f0-9]{40}$/.test(inets.commit)) {
  throw new Error('vendored inets identity does not match its pinned upstream provenance');
}
const planModelSha256 = createHash('sha256')
  .update(await readFile(path.join(repo, 'proofs/foundation/NobleFoundation/NetI64Plans.lean')))
  .digest('hex');
const binaryBytes = await readFile(testBinary);
const executableSha256 = createHash('sha256').update(binaryBytes).digest('hex');
const executableBytes = (await stat(testBinary)).size;
const run = spawnSync(testBinary, [
  'plancomparison::plan_storage_comparison_covers_required_families',
  '--exact',
  '--nocapture',
], { cwd: repo, encoding: 'utf8', maxBuffer: 16 * 1024 * 1024 });
if (run.error) throw run.error;
if (run.status !== 0) {
  process.stderr.write(run.stdout);
  process.stderr.write(run.stderr);
  throw new Error(`measurement test exited ${run.status}`);
}
const cases = run.stdout
  .split('\n')
  .filter((line) => line.startsWith('NOBLE_NET_PLAN_SAMPLE:'))
  .map((line) => JSON.parse(line.slice('NOBLE_NET_PLAN_SAMPLE:'.length)));
const expectedFamilies = [
  'chain',
  'fanout',
  'erasure',
  'asymmetric-subtraction',
  'wrapping-subtraction',
];
if (cases.length !== expectedFamilies.length) throw new Error(`expected ${expectedFamilies.length} workload rows, got ${cases.length}`);
for (let index = 0; index < expectedFamilies.length; index += 1) {
  const row = cases[index];
  if (row.family !== expectedFamilies[index]) throw new Error(`workload order/family mismatch at ${index}: ${row.family}`);
  if (!row.same_observation || !row.same_interactions) throw new Error(`${row.family}: arms differ`);
  if (row.reuse.storage !== 'reuse' || row.allocate.storage !== 'allocate') throw new Error(`${row.family}: malformed arm names`);
  if (row.reuse.samples_ns.length !== 31 || row.allocate.samples_ns.length !== 31) throw new Error(`${row.family}: expected 31 paired samples per arm`);
  if (!row.rule_revision || row.reuse.plan_digest === row.allocate.plan_digest) throw new Error(`${row.family}: changed plan did not change digest under a fixed rule identity`);
  if (row.reuse.replay_digest === row.allocate.replay_digest) throw new Error(`${row.family}: changed plan reused replay identity`);
}
for (const row of cases) {
  const reuseMedian = Number(row.reuse.median_ns);
  const allocateMedian = Number(row.allocate.median_ns);
  if (!Number.isFinite(reuseMedian) || !Number.isFinite(allocateMedian) || allocateMedian <= 0) {
    throw new Error(`${row.family}: invalid median for descriptive ratio`);
  }
  row.median_reuse_to_allocate_ratio = reuseMedian / allocateMedian;
}
const identities = cases.map((row) => ({
  family: row.family,
  graph_id: row.graph_id,
  net_key: row.net_key,
  rule_revision: row.rule_revision,
  plan_revision: row.plan_revision,
  representation_revision: row.representation_revision,
  checker_revision: row.checker_revision,
  executor_revision: row.executor_revision,
  inets_upstream_commit: inets.commit,
  inets_sha256: inetsSha256,
  plan_model_sha256: planModelSha256,
  executable_sha256: executableSha256,
  evidence_key: createHash('sha256').update(JSON.stringify([
    sourceCommit.stdout.trim(),
    executableSha256,
    inets.commit,
    inetsSha256,
    planModelSha256,
    row.family,
    row.graph_id,
    row.net_key,
    row.rule_revision,
    row.plan_revision,
    row.representation_revision,
    row.checker_revision,
    row.executor_revision,
    row.reuse.plan_digest,
    row.reuse.replay_digest,
    row.allocate.plan_digest,
    row.allocate.replay_digest,
  ])).digest('hex'),
}));
const report = {
  schema: 'noble-net-plan-comparison/1',
  source_commit: sourceCommit.stdout.trim(),
  test_executable_sha256: executableSha256,
  test_executable_bytes: executableBytes,
  formal_reference: {
    inets_upstream_commit: inets.commit,
    inets_sha256: inetsSha256,
    plan_model_sha256: planModelSha256,
  },
  executable_command: 'plancomparison::plan_storage_comparison_covers_required_families --exact --nocapture',
  criteria: 'verification/net-plans/criteria.md',
  timing_scope: 'complete reduce call, including validation, plan compile/check, execution, read-back and local destruction; excludes graph admission/lowering',
  paired_correctness: 'all five workloads have identical full observations and interaction counts in every measured pair, and match the reference evaluator',
  changed_plan_control: 'reuse and allocate use the same rule revision but distinct plan and replay digests',
  identities,
  workloads: cases,
};
await writeFile(output, `${JSON.stringify(report, null, 2)}\n`);
console.log(`recorded ${cases.length} workloads; executable sha256 ${executableSha256}; source ${sourceCommit.stdout.trim()}`);
