#!/usr/bin/env node
// Check the symbolic-pair trust, coverage and mutation inventories against the Lean modules they
// describe. Declarations are matched after comment stripping, so a name that survives only in a
// comment or as the prefix of a longer name does not count. Usage:
// node verification/ngraph/symbolic-pair-inventory.mjs [--root <dir>] [--self-test]
import { readFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { join } from 'node:path';

const args = process.argv.slice(2);
const rootFlag = args.indexOf('--root');
const root = rootFlag >= 0 ? args[rootFlag + 1] : '.';

// Strip Lean comments, keeping line structure. Strings are kept as code.
const stripComments = (text) => {
  let out = '';
  let index = 0;
  let depth = 0;
  while (index < text.length) {
    const char = text[index];
    const next = text[index + 1];
    if (depth > 0) {
      if (char === '/' && next === '-') { depth += 1; index += 2; continue; }
      if (char === '-' && next === '/') { depth -= 1; index += 2; continue; }
      if (char === '\n') out += char;
      index += 1;
      continue;
    }
    if (char === '/' && next === '-') { depth = 1; index += 2; continue; }
    if (char === '-' && next === '-') {
      while (index < text.length && text[index] !== '\n') index += 1;
      continue;
    }
    out += char;
    index += 1;
  }
  return out;
};

// A declaration, not a substring: `theorem <name>` followed by whitespace or the start of
// its signature, with or without a `private`/`protected` prefix.
const escapeName = (name) => name.replace(/[.*+?^${}()|[\]\\]/gu, '\\$&');
const hasDeclaration = (code, name) =>
  new RegExp(`\\btheorem\\s+${escapeName(name)}(?=[\\s(:{\\[])`, 'u').test(code);

// The constructors an `inductive Verdict where` block declares, for the status vocabulary.
const verdictConstructors = (code) => {
  const lines = code.split('\n');
  const start = lines.findIndex((line) => /^\s*inductive\s+Verdict\s+where\s*$/u.test(line));
  if (start < 0) return null;
  const names = [];
  for (const line of lines.slice(start + 1)) {
    if (line.trim() === '') continue;
    const match = line.match(/^\s*\|\s*([A-Za-z_][A-Za-z0-9_']*)/u);
    if (!match) break;
    names.push(match[1]);
  }
  return names;
};

const inspect = (inventory, readText) => {
  const problems = [];
  if (inventory.revision !== 1) problems.push(`unsupported revision ${inventory.revision}`);
  const cache = new Map();
  const codeOf = (path) => {
    if (!cache.has(path)) cache.set(path, stripComments(readText(path)));
    return cache.get(path);
  };
  const declared = (list, label, source) => {
    for (const entry of list) {
      if (!hasDeclaration(source, entry.theorem)) problems.push(`${label}: missing theorem ${entry.theorem}`);
    }
  };
  const certified = codeOf(inventory.module);
  declared(inventory.covered_pairs, 'covered pair', certified);
  declared(inventory.refuted_pairs, 'refuted pair', certified);
  if (inventory.covered_pairs.length === 0) problems.push('no covered pair is claimed');
  if (inventory.trusted_steps.length === 0) problems.push('no trusted step is declared');
  if (inventory.mutations) {
    const mutations = codeOf(inventory.mutations.module);
    declared(inventory.mutations.caught, 'mutation', mutations);
    if (!hasDeclaration(mutations, inventory.mutations.summary)) {
      problems.push(`mutation: missing summary theorem ${inventory.mutations.summary}`);
    }
    if (inventory.mutations.caught.length < 4) {
      problems.push('mutation battery covers fewer than the four named fault places');
    }
  }
  if (inventory.hazards) {
    for (const entry of inventory.hazards.covered) {
      if (!hasDeclaration(codeOf(entry.module), entry.theorem)) {
        problems.push(`hazard ${entry.hazard}: missing theorem ${entry.theorem}`);
      }
    }
    if (inventory.hazards.verdicts) {
      const verdicts = inventory.hazards.verdicts;
      const statuses = verdictConstructors(codeOf(verdicts.module));
      const listed = verdicts.statuses ?? [];
      if (statuses === null) {
        problems.push(`hazard verdicts: no inductive Verdict in ${verdicts.module}`);
      } else if (statuses.join() !== listed.join()) {
        problems.push(
          `hazard verdicts: ${verdicts.module} declares ${statuses.join('/')}, inventory lists ${listed.join('/')}`,
        );
      }
    }
  }
  return problems;
};

if (args.includes('--self-test')) {
  const sources = {
    'synthetic.lean': [
      '-- theorem comment_only_certified : True := trivial',
      '/- theorem block_only_certified : True := trivial -/',
      'theorem present_certified : True := trivial',
      'theorem prefix_certified_weakened : True := trivial',
      'private theorem hidden_certified : True := trivial',
      'protected theorem guarded_certified : True := trivial',
      '',
    ].join('\n'),
    'verdict.lean': 'inductive Verdict where\n  | certified\n  | refuted\n',
    'verdict-extra.lean': 'inductive Verdict where\n  | certified\n  | refuted\n  | extra\n',
  };
  const readText = (path) => {
    if (!(path in sources)) throw new Error(`unexpected read of ${path}`);
    return sources[path];
  };
  const base = {
    revision: 1,
    module: 'synthetic.lean',
    covered_pairs: [{ theorem: 'present_certified' }],
    refuted_pairs: [],
    trusted_steps: [{ step: 'synthetic' }],
  };
  const cases = [];
  const expect = (label, inventory, accepted) => {
    const problems = inspect(inventory, readText);
    cases.push({ case: label, refused: accepted(problems), problems });
  };
  expect('declared theorem is accepted', { ...base }, (problems) => problems.length === 0);
  expect('absent declaration is refused',
    { ...base, covered_pairs: [{ theorem: 'absent_certified' }] },
    (problems) => problems.length === 1 && problems[0].includes('absent_certified'));
  expect('comment-only declaration is refused',
    { ...base, covered_pairs: [{ theorem: 'comment_only_certified' }] },
    (problems) => problems.some((problem) => problem.includes('comment_only_certified')));
  expect('block-comment-only declaration is refused',
    { ...base, covered_pairs: [{ theorem: 'block_only_certified' }] },
    (problems) => problems.some((problem) => problem.includes('block_only_certified')));
  expect('prefix-only declaration is refused',
    { ...base, covered_pairs: [{ theorem: 'prefix_certified' }] },
    (problems) => problems.some((problem) => problem.includes('prefix_certified')));
  expect('private and protected declarations are found',
    { ...base, covered_pairs: [{ theorem: 'hidden_certified' }, { theorem: 'guarded_certified' }] },
    (problems) => problems.length === 0);
  expect('declared verdict vocabulary is accepted',
    { ...base, hazards: { covered: [], open: [], verdicts: { module: 'verdict.lean',
      statuses: ['certified', 'refuted'] } } },
    (problems) => problems.length === 0);
  expect('an extra verdict constructor is refused',
    { ...base, hazards: { covered: [], open: [], verdicts: { module: 'verdict-extra.lean',
      statuses: ['certified', 'refuted'] } } },
    (problems) => problems.some((problem) => problem.includes('verdict-extra.lean')));
  const refused = cases.every((entry) => entry.refused);
  console.log(JSON.stringify({ self_test: refused, cases }, null, 2));
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
