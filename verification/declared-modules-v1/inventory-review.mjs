#!/usr/bin/env node
// Compare a COMPLETE compiler collection with the currently reviewed policy.
// This emits a review manifest only: it does not classify new subjects, write
// policy, or turn a collector observation into a proof or quality verdict.
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = fs.realpathSync(path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..'));
assert.equal(process.argv.length, 4,
  'usage: SELECTED_NODE verification/declared-modules-v1/inventory-review.mjs COLLECTED_INVENTORY_JSON NEW_EXTERNAL_REPORT_JSON');
const [inventoryFile, reportFile] = process.argv.slice(2).map(value => path.resolve(value));
assert.ok(!fs.existsSync(reportFile), 'fresh external review report required');
assert.ok(reportFile !== root && !reportFile.startsWith(`${root}${path.sep}`),
  'review report belongs outside repository');
assert.equal(fs.realpathSync(path.dirname(reportFile)), path.dirname(reportFile),
  'report parent cannot be a symlink');
const hash = file => crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
const selection = JSON.parse(fs.readFileSync(path.join(root, 'policy/tool-selection.json')));
const selectedNode = fs.realpathSync(path.join(selection.tool_paths.node.output, 'bin/node'));
assert.equal(fs.realpathSync(process.execPath), selectedNode, 'selected Node required');
assert.deepEqual(process.execArgv, [], 'unreviewed Node flags');
const inventory = JSON.parse(fs.readFileSync(inventoryFile));
const policyFile = path.join(root, 'policy/source-inventory.json');
const policy = JSON.parse(fs.readFileSync(policyFile));
assert.equal(inventory.schema, 'noble-source-inventory/v1');
assert.equal(policy.schema_version, 'noble-source-inventory-policy/v1');
assert.equal(inventory.coverage.status, 'complete');
assert.equal(inventory.coverage.expected_units, inventory.coverage.observed_units);
for (const key of ['missing_units', 'unexpected_units', 'duplicate_units'])
  assert.deepEqual(inventory.coverage[key], [], `compiler collection ${key} is not empty`);
const unitById = new Map(inventory.units.map(unit => [unit.unit_id, unit]));
assert.equal(unitById.size, inventory.units.length, 'duplicate compiler unit');
function observedSubjects(items) {
  const groups = new Map();
  for (const item of items) {
    assert.ok(item.units.length > 0, `unowned compiler item: ${item.qualified_path}`);
    const owners = [...new Set(item.units.map(unit => {
      assert.ok(unitById.has(unit), `unknown unit owner: ${unit}`);
      const owner = unitById.get(unit).package_name;
      assert.ok(typeof owner === 'string' && owner.length > 0, `missing compiler unit owner: ${unit}`);
      return owner;
    }))];
    assert.equal(owners.length, 1, `cross-package subject: ${item.qualified_path}`);
    assert.equal(item.origin_crate, owners[0], `compiler subject origin differs from unit: ${item.qualified_path}`);
    const key = item.qualified_path;
    const group = groups.get(key) ?? { qualified_path: key, package: owners[0],
      item_kinds: new Set(), visibility: new Set(), unit_ids: new Set(), facts: 0 };
    assert.equal(group.package, owners[0], `subject package collision: ${key}`);
    group.item_kinds.add(item.item_kind);
    group.visibility.add(item.visibility);
    for (const unit of item.units) group.unit_ids.add(unit);
    group.facts++;
    groups.set(key, group);
  }
  return new Map([...groups].map(([name, item]) => [name, {
    qualified_path: item.qualified_path, package: item.package,
    item_kinds: [...item.item_kinds].sort(), visibility: [...item.visibility].sort(),
    unit_ids: [...item.unit_ids].sort(), facts: item.facts,
  }]));
}
const observed = observedSubjects(inventory.production_subjects);
const tests = observedSubjects(inventory.test_subjects);
const reviewed = new Map(policy.subjects.map(subject => [subject.qualified_path, subject]));
assert.equal(reviewed.size, policy.subjects.length, 'duplicate reviewed production path');
const testNames = new Set(policy.test_subjects);
assert.equal(testNames.size, policy.test_subjects.length, 'duplicate reviewed test path');
const oldClosure = new Map();
for (const row of policy.subjects) {
  const key = row.qualified_path.replace(/:\d+:\d+$/, ':<span>');
  if (key === row.qualified_path) continue;
  const siblings = oldClosure.get(key) ?? [];
  siblings.push({ qualified_path: row.qualified_path, category: row.category,
    disposition: row.disposition, refinement: row.refinement, package: row.package });
  oldClosure.set(key, siblings);
}
const added = [...observed].filter(([name]) => !reviewed.has(name))
  .map(([name, facts]) => ({ ...facts,
    similar_historical_closure_paths: oldClosure.get(name.replace(/:\d+:\d+$/, ':<span>')) ?? [] }))
  .sort((left, right) => left.qualified_path.localeCompare(right.qualified_path));
const removed = [...reviewed].filter(([name]) => !observed.has(name))
  .map(([, value]) => value).sort((left, right) => left.qualified_path.localeCompare(right.qualified_path));
const changedPackage = [...observed].filter(([name, value]) =>
  reviewed.has(name) && reviewed.get(name).package !== value.package)
  .map(([name, facts]) => ({ reviewed: reviewed.get(name), observed: facts }))
  .sort((left, right) => left.observed.qualified_path.localeCompare(right.observed.qualified_path));
const observedEdges = inventory.dependencies.production_edges.map(edge =>
  `${edge.caller_package}->${edge.callee_package}:${edge.edge_kind}`).sort();
const reviewedEdges = policy.dependencies.map(edge => `${edge.caller}->${edge.callee}:${edge.edge_kind}`).sort();
const observedTools = [...inventory.tools].sort();
const reviewedTools = policy.tools.map(tool => tool.name).sort();
const setDelta = (current, prior) => ({ added: [...current].filter(name => !prior.has(name)).sort(),
  removed: [...prior].filter(name => !current.has(name)).sort() });
const report = { schema: 'noble-declared-module-inventory-review/v1', result: 'review-required',
  review_tool: { path: 'verification/declared-modules-v1/inventory-review.mjs',
    sha256: hash(fileURLToPath(import.meta.url)) },
  node: { executable: selectedNode, sha256: hash(selectedNode) },
  inventory: { path: inventoryFile, sha256: hash(inventoryFile),
    generated_by: inventory.generated_by, coverage: inventory.coverage, counts: inventory.counts },
  previous_reviewed_policy: { path: policyFile, sha256: hash(policyFile),
    nickel_sha256: hash(path.join(root, 'policy/source-inventory.ncl')),
    counts: { subjects: reviewed.size, test_subjects: testNames.size } },
  subjects: { added, removed, changed_package: changedPackage,
    unchanged_paths: observed.size - added.length - changedPackage.length,
    observed_unique_paths: observed.size, observed_item_facts: inventory.production_subjects.length },
  test_subjects: { ...setDelta(new Set(tests.keys()), testNames),
    observed_unique_paths: tests.size, observed_item_facts: inventory.test_subjects.length },
  macro_origins: setDelta(new Set(inventory.macro_origins), new Set(policy.macro_origins)),
  dependencies: { observed: observedEdges, reviewed: reviewedEdges,
    changed: JSON.stringify(observedEdges) !== JSON.stringify(reviewedEdges) },
  tools: { observed: observedTools, reviewed: reviewedTools,
    changed: JSON.stringify(observedTools) !== JSON.stringify(reviewedTools) },
  non_claims: ['No new subject has been automatically classified or approved.',
    'A nearby historical closure is a review hint, not a transferred disposition.',
    'Complete compiler collection is not a refinement or full quality verdict.'] };
fs.writeFileSync(reportFile, JSON.stringify(report, null, 2) + '\n', { flag: 'wx', mode: 0o400 });
console.log(JSON.stringify({ schema: report.schema, result: report.result,
  report: reportFile, units: inventory.coverage.observed_units,
  production_added: added.length, production_removed: removed.length,
  test_added: report.test_subjects.added.length,
  test_removed: report.test_subjects.removed.length,
  changed_package: changedPackage.length }));
