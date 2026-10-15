#!/usr/bin/env node
// Renew policy/source-inventory.ncl against one COMPLETE derived compiler
// inventory (`nix/source-inventory-derive.nix` output). Reviewed rows that are
// still observed keep their category, disposition and note; stale rows are
// removed. New production paths are classified conservatively from compiler
// facts alone: `generated` only when every observing fact is macro-expanded,
// `structural` only when every authored fact is a declaration-only item kind,
// otherwise an authored `body` with OPEN extraction and refinement. A reviewed
// `generated` row that gained an authored fact becomes an OPEN body.
// `test_subjects` and `macro_origins` become the exact observed sets.
// Dependency edges and tools are never renewed here: a difference refuses.
// The result is accounting, not proof, refinement, or architecture acceptance;
// the `source-coverage` comparator remains the gate.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

// Item kinds that declare names, layout, implementation blocks or imports
// without an executable body of their own (historical structural convention).
const STRUCTURAL_KINDS = new Set(['module', 'struct', 'enum', 'union', 'type-alias', 'trait', 'trait-alias',
  'implementation', 'extern-crate', 'foreign-module']);
// Compiler kind `other` covers closures (bodies) plus imports and local macro
// definitions (structural markers). Every unlisted kind is conservatively a body.
const structuralRow = row => STRUCTURAL_KINDS.has(row.item_kind)
  || row.item_kind === 'other' && !row.qualified_path.includes('::closure@') && !row.qualified_path.startsWith('noble-');

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const policyFile = path.join(root, 'policy/source-inventory.ncl');
const exportFile = path.join(root, 'policy/source-inventory.json');

function usage() {
  console.error('usage: node tools/source-inventory-renew.mjs INVENTORY_JSON LABEL REPORT_JSON');
  process.exit(2);
}

function nickelString(value) {
  assert.ok(!value.includes('%{'), `Nickel interpolation marker in ${value}`);
  return `"${value.replaceAll('\\', '\\\\').replaceAll('"', '\\"')}"`;
}

function sorted(values) {
  return [...values].sort((left, right) => (left < right ? -1 : left > right ? 1 : 0));
}

function main() {
  const [inventoryFile, label, reportFile] = process.argv.slice(2);
  if (!inventoryFile || !label || !reportFile || process.argv.length !== 5) usage();
  assert.ok(!path.resolve(reportFile).startsWith(`${root}${path.sep}`), 'report belongs outside the repository');
  const inventory = JSON.parse(fs.readFileSync(inventoryFile, 'utf8'));
  const reviewed = JSON.parse(fs.readFileSync(exportFile, 'utf8'));
  assert.equal(inventory.schema, 'noble-source-inventory/v2');
  assert.equal(inventory.coverage.status, 'complete', 'renewal requires a complete collection');
  assert.equal(inventory.coverage.expected_units, inventory.coverage.observed_units);
  for (const key of ['missing_units', 'unexpected_units', 'duplicate_units'])
    assert.deepEqual(inventory.coverage[key], [], `collection ${key} is not empty`);

  const edge = e => `${e.caller_package ?? e.caller}->${e.callee_package ?? e.callee}:${e.edge_kind}`;
  assert.deepEqual(sorted(inventory.dependencies.production_edges.map(edge)), sorted(reviewed.dependencies.map(edge)),
    'observed dependency edges differ from review; renew them explicitly');
  assert.deepEqual(sorted(new Set(inventory.tools)), sorted(new Set(reviewed.tools.map(tool => tool.name))),
    'observed tools differ from review; renew them explicitly');

  const observed = new Map();
  for (const row of inventory.production_subjects) {
    const rows = observed.get(row.qualified_path) ?? [];
    rows.push(row);
    observed.set(row.qualified_path, rows);
  }
  // The comparator binds a path to the package of its first observing unit.
  const packageOf = rows => {
    const owner = inventory.units.find(unit => rows[0].units.includes(unit.unit_id));
    return owner ? owner.package_name : '<unowned>';
  };
  const allExpanded = rows => rows.every(row => row.from_expansion === true);
  const kindsOf = rows => sorted(new Set(rows.map(row => row.item_kind)));
  const classify = rows => {
    if (allExpanded(rows)) return 'generated';
    const authored = rows.filter(row => row.from_expansion !== true);
    if (authored.every(structuralRow)) return 'structural';
    return 'body';
  };
  const noteFor = (category, rows) => {
    const kinds = kindsOf(rows).join('/');
    if (category === 'generated')
      return `Renewed ${label}: every observing compiler fact (${kinds}) is macro-expanded; accounting only, with no authored body obligation.`;
    if (category === 'structural')
      return `Renewed ${label}: declaration-only compiler kind (${kinds}); accounting only, with no body obligation.`;
    return `Renewed ${label} from compiler kind ${kinds}: authored source-body semantics; extraction and refinement remain open. Conservative classification, not an exemption or proof.`;
  };

  const lines = fs.readFileSync(policyFile, 'utf8').split('\n');
  const start = lines.indexOf('  subjects | Array Subject = [');
  assert.ok(start > 0, 'subjects array not found');
  const end = lines.indexOf('  ],', start);
  const subjectLine = /^    (body|generated|structural) ("(?:[^"\\]|\\.)*") ("(?:[^"\\]|\\.)*") ("(?:[^"\\]|\\.)*"),$/;
  const kept = new Set();
  const removed = [];
  const reclassified = [];
  const rebound = [];
  const body = [];
  for (let index = start + 1; index < end; index += 1) {
    const line = lines[index];
    if (/^\s*#/.test(line) || line.trim() === '') { body.push(line); continue; }
    const match = subjectLine.exec(line);
    assert.ok(match, `unrecognized subject line ${index + 1}`);
    const [, category, quotedPath, quotedPackage, quotedNote] = match;
    const qualified = JSON.parse(quotedPath);
    assert.ok(!kept.has(qualified), `duplicate reviewed path ${qualified}`);
    const rows = observed.get(qualified);
    if (!rows) {
      removed.push({ qualified_path: qualified, category, package: JSON.parse(quotedPackage) });
      body.push(null);
      continue;
    }
    kept.add(qualified);
    let nextCategory = category;
    let note = quotedNote;
    if (category === 'generated' && !allExpanded(rows)) {
      nextCategory = 'body';
      note = nickelString(`Renewed ${label}: a reviewed generated path now has an authored compiler fact (${kindsOf(rows).join('/')}); extraction and refinement remain open.`);
      reclassified.push({ qualified_path: qualified, from: category, to: nextCategory });
    }
    let owner = JSON.parse(quotedPackage);
    if (packageOf(rows) !== owner) {
      rebound.push({ qualified_path: qualified, from: owner, to: packageOf(rows) });
      owner = packageOf(rows);
    }
    body.push(`    ${nextCategory} ${quotedPath} ${nickelString(owner)} ${note},`);
  }
  const added = sorted([...observed.keys()].filter(name => !kept.has(name))).map(name => {
    const rows = observed.get(name);
    return { qualified_path: name, package: packageOf(rows), category: classify(rows), kinds: kindsOf(rows) };
  });
  if (added.length) {
    body.push(`    # Renewed ${label}: ${added.length} newly observed production paths,`,
      '    # classified from compiler facts only (see tools/source-inventory-renew.mjs).',
      '    # Bodies keep OPEN extraction and refinement obligations.');
    for (const row of added)
      body.push(`    ${row.category} ${nickelString(row.qualified_path)} ${nickelString(row.package)} ${nickelString(noteFor(row.category, observed.get(row.qualified_path)))},`);
  }
  // Keep a comment run only if a retained subject follows it before the next run.
  const isComment = line => line !== null && /^\s*#/.test(line);
  const subjects = body.filter((line, index) => {
    if (line === null) return false;
    if (!isComment(line)) return true;
    let next = index;
    while (next < body.length && isComment(body[next])) next += 1;
    for (; next < body.length && !isComment(body[next]); next += 1)
      if (body[next] !== null) return true;
    return false;
  });

  const replaceList = (output, header, values) => {
    const open = output.indexOf(header);
    assert.ok(open > 0, `${header} not found`);
    const close = output.indexOf('  ],', open);
    return [...output.slice(0, open + 1), ...values.map(value => `    ${nickelString(value)},`), ...output.slice(close)];
  };
  let output = [...lines.slice(0, start + 1), ...subjects, ...lines.slice(end)];
  const testNames = sorted(new Set(inventory.test_subjects.map(row => row.qualified_path)));
  output = replaceList(output, '  test_subjects | Array String = [', testNames);
  output = replaceList(output, '  macro_origins | Array String = [', sorted(new Set(inventory.macro_origins)));
  const headerAt = output.findIndex(line => line.startsWith('# Renewal is explicit'));
  assert.ok(headerAt > 0, 'renewal header anchor not found');
  const counts = { body: 0, generated: 0, structural: 0 };
  for (const line of output) {
    const match = /^    (body|generated|structural) "/.exec(line);
    if (match) counts[match[1]] += 1;
  }
  output.splice(headerAt, 0,
    `# Renewed ${label} from a complete ${inventory.coverage.observed_units}-unit selected collection:`,
    `#   IR ${inventory.generated_by.ir_id}; coverage ${inventory.generated_by.coverage_id};`,
    `#   cargo graph ${inventory.generated_by.cargo_graph_id}; collector policy ${inventory.generated_by.policy_blake3}.`,
    `#   ${counts.body + counts.generated + counts.structural} production paths = ${counts.body} body/open + ${counts.generated} generated + ${counts.structural} structural;`,
    `#   ${testNames.length} test-only paths; ${new Set(inventory.macro_origins).size} macro origins. Earlier header counts are historical.`);
  fs.writeFileSync(policyFile, output.join('\n'));

  const report = {
    schema: 'noble-source-inventory-renewal/v1', label,
    inventory: path.resolve(inventoryFile), generated_by: inventory.generated_by,
    counts: { ...counts, retained: kept.size, added: added.length, removed: removed.length,
      reclassified: reclassified.length, rebound: rebound.length, test_subjects: testNames.length,
      macro_origins: new Set(inventory.macro_origins).size },
    added, removed, reclassified, rebound,
    non_claims: ['accounting-not-proof', 'not-refinement', 'not-architecture-acceptance', 'bodies-remain-open'],
  };
  fs.writeFileSync(reportFile, `${JSON.stringify(report, null, 2)}\n`, { flag: 'wx' });
  console.log(JSON.stringify(report.counts));
}

main();
