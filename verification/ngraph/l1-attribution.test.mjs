// L1 attribution of a difference to the CLI version, and the lane's exit code
// on a failed result. Synthetic receipts: no CLI or harness is executed.
import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const lane = path.join(root, 'verification/ngraph/l1-attribution.mjs');

const l1 = agree => ({ off: { vectors: 4, steps: 9, agree }, on: { vectors: 4, steps: 9, agree } });
function receipt(agree) {
  return {
    result: 'passed',
    run: {
      source_revision: 'adapter', optimization_mode: 'both', toolchain: 'toolchain', workload: 'workload',
      wall_seconds: 1, ordinary_path: { cli_sha256: 'cli-sha' },
    },
    records: [
      { id: 'B2-I64', evidence: { result: {
        named: [{ name: 'program', l1: l1(agree) }], random: [], l1_totals: {},
      } } },
      { id: 'B3-I64', evidence: { result: [] } },
    ],
  };
}

const mutations = result => ({
  result, source_revision: 'source', file_sha256: 'file', toolchain: 'toolchain',
  unmutated_passed: ['a-test'], silent: result === 'passed' ? [] : ['ordinal'], mutations: [],
});

function run(args) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'noble-l1-'));
  for (const [name, value] of Object.entries(args.files))
    fs.writeFileSync(path.join(dir, name), `${JSON.stringify(value, null, 2)}\n`);
  const out = path.join(dir, 'evidence.json');
  const observed = spawnSync(process.execPath, [lane, '--baseline', path.join(dir, 'baseline.json'),
    '--baseline-cli-revision', 'BASEREV', ...args.runs.flatMap(row =>
      ['--run', row.label, path.join(dir, row.file), row.revision]),
    '--mutations', path.join(dir, 'mutations.json'), '--out', out], { encoding: 'utf8' });
  return {
    status: observed.status, stdout: observed.stdout, stderr: observed.stderr,
    evidence: fs.existsSync(out) ? JSON.parse(fs.readFileSync(out, 'utf8')) : null,
  };
}

const files = extra => ({ 'baseline.json': receipt(true), 'mutations.json': mutations('passed'), ...extra });

test('a run built from the baseline CLI does not attribute its own difference to the CLI version', () => {
  const observed = run({
    files: files({ 'a.json': receipt(true), 'b.json': receipt(false) }),
    runs: [{ label: 'a', file: 'a.json', revision: 'BASEREV' },
      { label: 'b', file: 'b.json', revision: 'BASEREV' }],
  });
  assert.equal(observed.evidence.result, 'failed', observed.stderr);
  const differing = observed.evidence.runs.find(row => row.label === 'b').differing;
  assert.equal(differing.length, 2);
  assert.deepEqual(differing.map(row => row.attributed_to_cli_version), [false, false]);
  assert.equal(observed.evidence.unattributed_differences.length, 2);
  assert.equal(observed.status, 1, 'a failed lane must not exit 0');
});

test('another baseline-CLI run reproducing the record still attributes it', () => {
  const observed = run({
    files: files({ 'a.json': receipt(true), 'b.json': receipt(false) }),
    runs: [{ label: 'a', file: 'a.json', revision: 'BASEREV' },
      { label: 'b', file: 'b.json', revision: 'OTHERREV' }],
  });
  assert.equal(observed.evidence.result, 'passed', observed.stderr);
  const differing = observed.evidence.runs.find(row => row.label === 'b').differing;
  assert.equal(differing.length, 2);
  assert.deepEqual(differing.map(row => row.attributed_to_cli_version), [true, true]);
  assert.equal(observed.status, 0);
});

test('a failed mutation result fails the lane even when every record reproduces', () => {
  const observed = run({
    files: { 'baseline.json': receipt(true), 'mutations.json': mutations('failed'), 'a.json': receipt(true) },
    runs: [{ label: 'a', file: 'a.json', revision: 'BASEREV' }],
  });
  assert.equal(observed.evidence.mutations.result, 'failed');
  assert.equal(observed.evidence.unattributed_differences.length, 0);
  assert.equal(observed.evidence.result, 'failed');
  assert.equal(observed.status, 1);
});
