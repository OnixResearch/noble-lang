#!/usr/bin/env node
// Check the symbolic-pair trust, coverage and mutation inventories against the Lean modules they
// describe. Usage: node verification/ngraph/symbolic-pair-inventory.mjs [--root <dir>] [--self-test]
import { readFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { join } from 'node:path';

const args = process.argv.slice(2);
const rootFlag = args.indexOf('--root');
const root = rootFlag >= 0 ? args[rootFlag + 1] : '.';

const inspect = (inventory, readText) => {
  const problems = [];
  if (inventory.revision !== 1) problems.push(`unsupported revision ${inventory.revision}`);
  const declared = (list, label, source) => {
    for (const entry of list) {
      const marker = `theorem ${entry.theorem}`;
      if (!source.includes(marker)) problems.push(`${label}: missing ${marker}`);
    }
  };
  const certified = readText(inventory.module);
  declared(inventory.covered_pairs, 'covered pair', certified);
  declared(inventory.refuted_pairs, 'refuted pair', certified);
  if (inventory.covered_pairs.length === 0) problems.push('no covered pair is claimed');
  if (inventory.trusted_steps.length === 0) problems.push('no trusted step is declared');
  if (inventory.mutations) {
    const mutations = readText(inventory.mutations.module);
    declared(inventory.mutations.caught, 'mutation', mutations);
    if (!mutations.includes(`theorem ${inventory.mutations.summary}`)) {
      problems.push(`mutation: missing summary theorem ${inventory.mutations.summary}`);
    }
    if (inventory.mutations.caught.length < 4) {
      problems.push('mutation battery covers fewer than the four named fault places');
    }
  }
  if (inventory.hazards) {
    const cache = new Map();
    for (const entry of inventory.hazards.covered) {
      const source = cache.get(entry.module) ?? readText(entry.module);
      cache.set(entry.module, source);
      if (!source.includes(`theorem ${entry.theorem}`)) {
        problems.push(`hazard ${entry.hazard}: missing theorem ${entry.theorem}`);
      }
    }
  }
  return problems;
};

if (args.includes('--self-test')) {
  const sources = {
    'synthetic.lean': 'theorem present_certified : True := trivial\n'
  };
  const inventory = {
    revision: 1,
    module: 'synthetic.lean',
    covered_pairs: [{ theorem: 'absent_certified' }],
    refuted_pairs: [{ theorem: 'present_certified' }],
    trusted_steps: [{ step: 'synthetic' }]
  };
  const problems = inspect(inventory, (path) => {
    if (!(path in sources)) throw new Error(`unexpected read of ${path}`);
    return sources[path];
  });
  const refused = problems.length === 1 && problems[0].includes('absent_certified');
  console.log(JSON.stringify({ self_test: refused, problems }, null, 2));
  process.exit(refused ? 0 : 1);
}

const inventory = JSON.parse(readFileSync(join(root, 'verification/ngraph/symbolic-pair-inventory.json'), 'utf8'));
const readText = (path) => readFileSync(join(root, path), 'utf8');
const problems = inspect(inventory, readText);
const report = {
  holds: problems.length === 0,
  module: inventory.module,
  module_sha256: createHash('sha256').update(readText(inventory.module)).digest('hex'),
  covered: inventory.covered_pairs.map((p) => p.theorem),
  refuted: inventory.refuted_pairs.map((p) => p.theorem),
  mutations: inventory.mutations ? inventory.mutations.caught.map((p) => p.fault) : [],
  mutation_module_sha256: inventory.mutations
    ? createHash('sha256').update(readText(inventory.mutations.module)).digest('hex')
    : null,
  hazards_covered: inventory.hazards ? inventory.hazards.covered.map((h) => h.hazard) : [],
  hazards_open: inventory.hazards ? inventory.hazards.open : [],
  trusted_steps: inventory.trusted_steps.length,
  problems
};
console.log(JSON.stringify(report, null, 2));
if (!report.holds) process.exit(1);
