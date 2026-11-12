// Renewal behaviour that the reviewed source-inventory evidence depends on: a
// reviewed `structural` row observed as an authored function body is reclassified
// to `body` (the comparator refuses the combination), and a refused report path
// never leaves a rewritten policy behind. Synthetic roots only; no compiler run.
import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const tool = path.join(path.dirname(fileURLToPath(import.meta.url)), 'source-inventory-renew.mjs');
const policyText = `# Synthetic reviewed policy (scratch copy).
# Renewal is explicit: the build never refreshes reviewed expectations.
{
  subjects | Array Subject = [
    SUBJECT
    body "a::new_fn" "pkg" "reviewed body",
  ],
  test_subjects | Array String = [
  ],
  macro_origins | Array String = [
  ],
}
`;

function inventory(rows) {
  return {
    schema: 'noble-source-inventory/v2',
    coverage: {
      status: 'complete', expected_units: 1, observed_units: 1,
      missing_units: [], unexpected_units: [], duplicate_units: [],
    },
    dependencies: { production_edges: [] },
    tools: [],
    units: [{ unit_id: 'u1', package_name: 'pkg' }],
    production_subjects: rows,
    test_subjects: [],
    macro_origins: [],
    generated_by: { ir_id: 'ir', coverage_id: 'cov', cargo_graph_id: 'cg', policy_blake3: 'pb' },
  };
}

const observed = (qualified_path, item_kind) => ({
  qualified_path, units: ['u1'], item_kind, from_expansion: false, span_id: `s-${qualified_path}`,
});

// A scratch root shaped like the repository: the policy pair and the tool.
function root(subject) {
  const base = fs.mkdtempSync(path.join(os.tmpdir(), 'noble-renew-'));
  fs.mkdirSync(path.join(base, 'tools'));
  fs.mkdirSync(path.join(base, 'policy'));
  fs.mkdirSync(path.join(base, 'out'));
  fs.copyFileSync(tool, path.join(base, 'tools', 'source-inventory-renew.mjs'));
  fs.writeFileSync(path.join(base, 'policy', 'source-inventory.ncl'), policyText.replace('SUBJECT', subject));
  fs.writeFileSync(path.join(base, 'policy', 'source-inventory.json'), '{"dependencies":[],"tools":[]}\n');
  return base;
}

// The report belongs outside the repository root (the tool refuses it inside).
function reportPath() {
  return path.join(fs.mkdtempSync(path.join(os.tmpdir(), 'noble-renew-report-')), 'report.json');
}

function renew(base, report) {
  fs.writeFileSync(path.join(base, 'out', 'inventory.json'), `${JSON.stringify(inventory([
    observed('a::Thing', 'function'), observed('a::new_fn', 'function'),
  ]))}\n`);
  const run = spawnSync(process.execPath, [
    path.join(base, 'tools', 'source-inventory-renew.mjs'),
    path.join(base, 'out', 'inventory.json'), 'probe', report,
  ], { encoding: 'utf8' });
  return { status: run.status, stderr: run.stderr, policy: fs.readFileSync(path.join(base, 'policy', 'source-inventory.ncl'), 'utf8') };
}

test('a reviewed structural row observed as an authored function becomes body', () => {
  const base = root('structural "a::Thing" "pkg" "reviewed as a struct declaration",');
  const report = reportPath();
  const result = renew(base, report);
  assert.equal(result.status, 0, result.stderr);
  const written = JSON.parse(fs.readFileSync(report, 'utf8'));
  assert.equal(written.counts.reclassified, 1);
  assert.deepEqual(written.reclassified.map(row => [row.qualified_path, row.from, row.to]),
    [['a::Thing', 'structural', 'body']]);
  assert.equal(written.counts.structural, 0);
  assert.equal(written.counts.body, 2);
  assert.match(result.policy, /^ {4}body "a::Thing" "pkg" "Renewed probe: a reviewed structural path is observed as an authored function body \(function\); extraction and refinement remain open\.",$/m);
  assert.doesNotMatch(result.policy, /structural "a::Thing"/);
});

test('a reviewed structural row still observed as a declaration keeps its category', () => {
  const base = root('structural "a::Thing" "pkg" "reviewed as a struct declaration",');
  fs.writeFileSync(path.join(base, 'out', 'inventory.json'), `${JSON.stringify(inventory([
    observed('a::Thing', 'struct'), observed('a::new_fn', 'function'),
  ]))}\n`);
  const report = reportPath();
  const run = spawnSync(process.execPath, [
    path.join(base, 'tools', 'source-inventory-renew.mjs'),
    path.join(base, 'out', 'inventory.json'), 'probe', report,
  ], { encoding: 'utf8' });
  assert.equal(run.status, 0, run.stderr);
  assert.equal(JSON.parse(fs.readFileSync(report, 'utf8')).counts.reclassified, 0);
  assert.match(fs.readFileSync(path.join(base, 'policy', 'source-inventory.ncl'), 'utf8'),
    /^ {4}structural "a::Thing" "pkg" "reviewed as a struct declaration",$/m);
});

test('an existing report path refuses before the policy is rewritten', () => {
  const base = root('structural "a::Thing" "pkg" "reviewed as a struct declaration",');
  const report = reportPath();
  fs.writeFileSync(report, '{"retained":true}\n');
  const before = fs.readFileSync(path.join(base, 'policy', 'source-inventory.ncl'));
  const result = renew(base, report);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /report path already exists/);
  assert.deepEqual(fs.readFileSync(path.join(base, 'policy', 'source-inventory.ncl')), before);
  assert.equal(fs.readFileSync(report, 'utf8'), '{"retained":true}\n');
});
