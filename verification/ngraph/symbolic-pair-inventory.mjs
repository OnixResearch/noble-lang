#!/usr/bin/env node
// Check the symbolic-pair trust, coverage, hazard and generation inventories against the Lean
// modules, the retained receipts and the generator they describe. Declarations are matched after
// comment stripping, so a name that survives only in a comment or as the prefix of a longer name
// does not count. Usage:
// node verification/ngraph/symbolic-pair-inventory.mjs [--root <dir>] [--self-test]
import { readFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { join } from 'node:path';
import { generate as generateDefault } from '../symbolic/solver-lean.mjs';

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

const selectorText = (selector) => `${selector.file} ${selector.path.join('.')}` +
  `${selector.label === undefined ? '' : ` [${selector.label}]`}`;

// Resolve a selector against a receipt: string path steps index objects, number steps arrays, and
// an optional `label` picks the row with that recorded label out of the resulting list.
const selectRow = (receipt, selector) => {
  let value = receipt;
  for (const step of selector.path) {
    if (value === null || typeof value !== 'object') return undefined;
    value = value[step];
  }
  if (selector.label !== undefined) {
    if (!Array.isArray(value)) return undefined;
    return value.find((entry) => entry.label === selector.label);
  }
  return value;
};

const inspect = (inventory, readText, options) => {
  const problems = [];
  const readJson = options?.readJson;
  const generate = options?.generate ?? ((solver, queries, endpoints, where) =>
    generateDefault(solver, queries, endpoints, where));
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

  // A trusted step must name the record that bounds it, and that record must carry the heading.
  for (const step of inventory.trusted_steps) {
    const bound = step.bounded_by;
    if (!bound || typeof bound !== 'object' || !bound.record || !bound.heading) {
      problems.push(`trusted step ${step.step}: bounded_by does not name a record and heading`);
      continue;
    }
    let record;
    try {
      record = readText(bound.record);
    } catch (error) {
      problems.push(`trusted step ${step.step}: missing record ${bound.record}`);
      continue;
    }
    if (!record.includes(bound.heading)) {
      problems.push(`trusted step ${step.step}: ${bound.record} has no heading ${bound.heading}`);
    }
  }

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
    const named = inventory.hazards.named ?? [];
    if (named.length === 0) problems.push('hazards: no named hazards are declared');
    for (const entry of inventory.hazards.covered) {
      if (!named.includes(entry.hazard)) {
        problems.push(`hazard ${entry.hazard}: not one of the named hazards`);
      }
      if (!hasDeclaration(codeOf(entry.module), entry.theorem)) {
        problems.push(`hazard ${entry.hazard}: missing theorem ${entry.theorem}`);
      }
      if (!entry.receipt) {
        problems.push(`hazard ${entry.hazard}: no executed receipt row is cited`);
        continue;
      }
      const row = selectRow(readJson(entry.receipt.file), entry.receipt);
      if (!row || typeof row.status !== 'string') {
        problems.push(`hazard ${entry.hazard}: no receipt row ${selectorText(entry.receipt)}`);
        continue;
      }
      if (row.status !== 'inconclusive' && row.status !== 'unsupported') {
        problems.push(`hazard ${entry.hazard}: ${selectorText(entry.receipt)} recorded ${row.status}, which is not a refusal`);
      }
    }
    if ((inventory.hazards.open ?? []).length > 0) {
      problems.push(`hazards open: ${inventory.hazards.open.join(', ')}`);
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

  // The positive controls are recorded apart from the hazards: a status a clean query may produce
  // must never be cited as coverage for a hazard.
  const controls = inventory.acceptance_controls ?? [];
  if (controls.length === 0) problems.push('no acceptance control is declared');
  const hazardTheorems = new Set((inventory.hazards?.covered ?? []).map((entry) => entry.theorem));
  for (const entry of controls) {
    if (!hasDeclaration(codeOf(entry.module), entry.theorem)) {
      problems.push(`acceptance control ${entry.control}: missing theorem ${entry.theorem}`);
    }
    if (hazardTheorems.has(entry.theorem)) {
      problems.push(`acceptance control ${entry.control}: ${entry.theorem} is also cited as a hazard`);
    }
    if (entry.receipt) {
      const row = selectRow(readJson(entry.receipt.file), entry.receipt);
      if (!row || typeof row.status !== 'string') {
        problems.push(`acceptance control ${entry.control}: no receipt row ${selectorText(entry.receipt)}`);
      } else if (row.status !== entry.expect) {
        problems.push(`acceptance control ${entry.control}: ${selectorText(entry.receipt)} recorded ` +
          `${row.status}, expected ${entry.expect}`);
      }
    }
  }

  // The generated Lean modules must be this repository's generator's output for the retained
  // receipts, so a receipt cannot move without the replay and the encoder corpus moving with it.
  if (inventory.generated) {
    const spec = inventory.generated;
    let rendered;
    try {
      rendered = generate(spec.solver_receipt, spec.queries, spec.endpoints_receipt, root);
    } catch (error) {
      problems.push(`generated: the generator refused the retained receipts: ${error.message}`);
    }
    for (const module of spec.modules ?? []) {
      if (rendered && !(module in rendered)) {
        problems.push(`generated: ${spec.generator} does not write ${module}`);
        continue;
      }
      let committed;
      try {
        committed = readText(module);
      } catch (error) {
        problems.push(`generated: missing module ${module}`);
        continue;
      }
      if (rendered && committed !== rendered[module]) {
        problems.push(`generated: ${module} is not the generator's output for ` +
          `${spec.solver_receipt}, ${spec.queries} and ${spec.endpoints_receipt}`);
      }
    }
  } else {
    problems.push('no generated-module inventory is declared');
  }
  return problems;
};

if (args.includes('--self-test')) {
  const receipt = {
    controls: {
      bindings: [{ label: 'stale semantic revision', status: 'inconclusive' }],
      certificates: [{ label: 'theorem not proved', status: 'inconclusive' }],
      corrupted_encodings: [{ results: [{ status: 'inconclusive' }] }],
      altered_loaded_bytes: { status: 'unsupported', rows: [{ status: 'inconclusive' }] },
      good: { status: 'certified' },
    },
  };
  const clean = { status: 'inconclusive' };
  const sources = {
    'synthetic.lean': [
      '-- theorem comment_only_certified : True := trivial',
      '/- theorem block_only_certified : True := trivial -/',
      'theorem present_certified : True := trivial',
      'theorem prefix_certified_weakened : True := trivial',
      'private theorem hidden_certified : True := trivial',
      'protected theorem guarded_certified : True := trivial',
      'theorem hazard_theorem : True := trivial',
      'theorem clean_control : True := trivial',
      'theorem control_theorem : True := trivial',
      '',
    ].join('\n'),
    'verdict.lean': 'inductive Verdict where\n  | certified\n  | refuted\n',
    'verdict-extra.lean': 'inductive Verdict where\n  | certified\n  | refuted\n  | extra\n',
    'record.md': '### Synthetic record heading\n',
    'replay.lean': 'generated replay solver.json queries.json endpoints.json\n',
    'encoder.lean': 'generated corpus\n',
  };
  const readText = (path) => {
    if (!(path in sources)) throw new Error(`unexpected read of ${path}`);
    return sources[path];
  };
  const readJson = (path) => {
    if (path !== 'receipt.json') throw new Error(`unexpected json read of ${path}`);
    return receipt;
  };
  const generated = (solver, queries, endpoints) => ({
    'replay.lean': `generated replay ${solver} ${queries} ${endpoints}\n`,
    'encoder.lean': 'generated corpus\n',
  });
  const base = {
    revision: 1,
    module: 'synthetic.lean',
    covered_pairs: [{ theorem: 'present_certified' }],
    refuted_pairs: [],
    trusted_steps: [{ step: 'synthetic', bounded_by: { record: 'record.md', heading: '### Synthetic record heading' } }],
    hazards: {
      named: ['stale semantic revision'],
      covered: [{
        hazard: 'stale semantic revision',
        module: 'synthetic.lean',
        theorem: 'hazard_theorem',
        receipt: { file: 'receipt.json', path: ['controls', 'bindings'], label: 'stale semantic revision' },
      }],
      open: [],
    },
    acceptance_controls: [{
      control: 'a clean query certifies',
      module: 'synthetic.lean',
      theorem: 'clean_control',
      receipt: { file: 'receipt.json', path: ['controls', 'good'] },
      expect: 'certified',
    }],
    generated: { generator: 'generator.mjs', solver_receipt: 'solver.json', queries: 'queries.json',
      endpoints_receipt: 'endpoints.json', modules: ['replay.lean', 'encoder.lean'] },
  };
  const cases = [];
  const expect = (label, inventory, accepted, rendered = generated) => {
    const problems = inspect(inventory, readText, { readJson, generate: rendered });
    cases.push({ case: label, refused: accepted(problems), problems });
  };
  expect('a bound hazard, acceptance control and generated modules are accepted', { ...base },
    (problems) => problems.length === 0);
  expect('declared theorem is accepted', { ...base, covered_pairs: [{ theorem: 'present_certified' }] },
    (problems) => problems.length === 0);
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
    { ...base, hazards: { ...base.hazards, verdicts: { module: 'verdict.lean', statuses: ['certified', 'refuted'] } } },
    (problems) => problems.length === 0);
  expect('an extra verdict constructor is refused',
    { ...base, hazards: { ...base.hazards, verdicts: { module: 'verdict-extra.lean', statuses: ['certified', 'refuted'] } } },
    (problems) => problems.some((problem) => problem.includes('verdict-extra.lean')));
  expect('a hazard recorded certified is refused',
    { ...base,
      hazards: { ...base.hazards, covered: [{ ...base.hazards.covered[0],
        receipt: { file: 'receipt.json', path: ['controls', 'good'] } }] } },
    (problems) => problems.some((problem) => problem.includes('is not a refusal')));
  expect('a hazard outside the named set is refused',
    { ...base, hazards: { ...base.hazards, covered: [{ ...base.hazards.covered[0], hazard: 'invented hazard' }] } },
    (problems) => problems.some((problem) => problem.includes('not one of the named hazards')));
  expect('an open hazard is refused',
    { ...base, hazards: { ...base.hazards, open: ['stale semantic revision'] } },
    (problems) => problems.some((problem) => problem.includes('hazards open')));
  expect('a trusted step whose record lacks the heading is refused',
    { ...base, trusted_steps: [{ step: 'synthetic', bounded_by: { record: 'record.md', heading: '### Absent' } }] },
    (problems) => problems.some((problem) => problem.includes('has no heading')));
  expect('an acceptance control cited as a hazard is refused',
    { ...base, acceptance_controls: [{ ...base.acceptance_controls[0], theorem: 'hazard_theorem' }] },
    (problems) => problems.some((problem) => problem.includes('also cited as a hazard')));
  expect('a drifted generated module is refused', { ...base },
    (problems) => problems.some((problem) => problem.includes('is not the generator')),
    () => ({ 'replay.lean': 'drifted\n', 'encoder.lean': 'generated corpus\n' }));
  expect('a generator that cannot read the receipts is refused', { ...base },
    (problems) => problems.some((problem) => problem.includes('refused the retained receipts')),
    () => { throw new Error('receipt drift'); });
  expect('an altered endpoint row is an accepted hazard row',
    { ...base, hazards: { ...base.hazards, covered: [{ ...base.hazards.covered[0],
      receipt: { file: 'receipt.json', path: ['controls', 'altered_loaded_bytes', 'rows', 0] } }] } },
    (problems) => problems.length === 0);
  expect('a clean row is refused as a hazard row',
    { ...base, hazards: { ...base.hazards, covered: [{ ...base.hazards.covered[0],
      receipt: { file: 'receipt.json', path: ['controls', 'good'] } }] } },
    (problems) => problems.some((problem) => problem.includes('is not a refusal')));
  void clean;
  const refused = cases.every((entry) => entry.refused);
  console.log(JSON.stringify({ self_test: refused, cases }, null, 2));
  process.exit(refused ? 0 : 1);
}

const inventory = JSON.parse(readFileSync(join(root, 'verification/ngraph/symbolic-pair-inventory.json'), 'utf8'));
const readText = (path) => readFileSync(join(root, path), 'utf8');
const readJson = (path) => JSON.parse(readFileSync(join(root, path), 'utf8'));
const problems = inspect(inventory, readText, { readJson });
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
  acceptance_controls: inventory.acceptance_controls
    ? inventory.acceptance_controls.map((entry) => entry.control) : [],
  generated: inventory.generated
    ? Object.fromEntries(inventory.generated.modules.map((module) =>
      [module, createHash('sha256').update(readText(module)).digest('hex')]))
    : {},
  trusted_steps: inventory.trusted_steps.length,
  problems
};
console.log(JSON.stringify(report, null, 2));
if (!report.holds) process.exit(1);
