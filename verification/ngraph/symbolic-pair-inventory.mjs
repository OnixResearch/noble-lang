#!/usr/bin/env node
// Check the symbolic-pair trust and coverage inventory against the Lean module it describes.
// Usage: node verification/ngraph/symbolic-pair-inventory.mjs [--root <dir>] [--self-test]
import { readFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { join } from 'node:path';

const args = process.argv.slice(2);
const rootFlag = args.indexOf('--root');
const root = rootFlag >= 0 ? args[rootFlag + 1] : '.';

const inspect = (inventory, source) => {
  const problems = [];
  if (inventory.revision !== 1) problems.push(`unsupported revision ${inventory.revision}`);
  const declared = (list, label) => {
    for (const entry of list) {
      const marker = `theorem ${entry.theorem}`;
      if (!source.includes(marker)) problems.push(`${label}: missing ${marker}`);
    }
  };
  declared(inventory.covered_pairs, 'covered pair');
  declared(inventory.refuted_pairs, 'refuted pair');
  if (inventory.covered_pairs.length === 0) problems.push('no covered pair is claimed');
  if (inventory.trusted_steps.length === 0) problems.push('no trusted step is declared');
  return problems;
};

if (args.includes('--self-test')) {
  const source = 'theorem present_certified : True := trivial\n';
  const inventory = {
    revision: 1,
    covered_pairs: [{ theorem: 'absent_certified' }],
    refuted_pairs: [{ theorem: 'present_certified' }],
    trusted_steps: [{ step: 'synthetic' }]
  };
  const problems = inspect(inventory, source);
  const refused = problems.length === 1 && problems[0].includes('absent_certified');
  console.log(JSON.stringify({ self_test: refused, problems }, null, 2));
  process.exit(refused ? 0 : 1);
}

const inventory = JSON.parse(readFileSync(join(root, 'verification/ngraph/symbolic-pair-inventory.json'), 'utf8'));
const source = readFileSync(join(root, inventory.module), 'utf8');
const problems = inspect(inventory, source);
const report = {
  holds: problems.length === 0,
  module: inventory.module,
  module_sha256: createHash('sha256').update(source).digest('hex'),
  covered: inventory.covered_pairs.map((p) => p.theorem),
  refuted: inventory.refuted_pairs.map((p) => p.theorem),
  trusted_steps: inventory.trusted_steps.length,
  problems
};
console.log(JSON.stringify(report, null, 2));
if (!report.holds) process.exit(1);
