// Survey for #89: would a construction-checked invariant on an opaque nominal type remove repeated
// `requires` clauses from real Noble contracts? Measures the tracked contract corpus (contract
// fixtures, `.contract` files, inline contracts in Rust sources, conformance case inputs) and probes
// both contract surfaces of the production CLI with an opaque-typed subject and a structural
// control. Evidence for a design decision only: it builds no invariant and proves nothing.
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, readFileSync, realpathSync, writeFileSync } from 'node:fs';
import { dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(fileURLToPath(import.meta.url), '../../..');
const requireThat = (condition, message) => { if (!condition) throw Error(message); };
const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const BUILTIN_TYPES = new Set(['Unit', 'Bool', 'I64', 'Text', 'Syntax', 'Contract', 'Evidence', 'Certified',
  'Pair', 'Sum', 'List', 'Program', 'Resource']);

function parseArguments(argv) {
  const options = { allowDirty: false, output: null, cli: null };
  for (let index = 0; index < argv.length; index += 1) {
    if (argv[index] === '--allow-dirty') options.allowDirty = true;
    else if (argv[index] === '--output') options.output = argv[(index += 1)];
    else if (argv[index] === '--cli') options.cli = argv[(index += 1)];
    else throw Error(`usage: SELECTED_NODE verification/opaque-invariants/survey.mjs --cli NOBLE --output NEW_EXTERNAL_DIRECTORY [--allow-dirty]; unknown ${argv[index]}`);
  }
  requireThat(options.output && options.cli, 'usage: --cli NOBLE and --output NEW_EXTERNAL_DIRECTORY are required');
  return options;
}

function git(argv) {
  const observed = spawnSync(realpathSync('/run/current-system/sw/bin/git'), argv, {
    cwd: root, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024, env: { PATH: '/run/current-system/sw/bin', HOME: '/nonexistent' },
  });
  requireThat(observed.status === 0, `git ${argv.join(' ')} failed: ${observed.stderr}`);
  return observed.stdout;
}

// The corpus: authored contract sources (fixtures, contract files, inline contracts in Rust and in
// lane scripts, conformance case inputs), not receipts, locks or specification examples.
function corpusFiles() {
  const self = relative(root, fileURLToPath(import.meta.url));
  return git(['ls-files', '-z']).split('\0').filter(Boolean).filter(file => file !== self && (
    file.endsWith('.contract') || file.endsWith('.noble-contract') || file.endsWith('.noble')
    || (file.endsWith('.rs') && file.startsWith('crates/'))
    || (file.endsWith('.mjs') && (file.startsWith('crates/') || file.startsWith('verification/')))
    || (file.startsWith('specs/conformance/') && file.endsWith('.json')))).sort();
}

// Undo the string escapes of the host format so an inline contract reads as its source text.
function unescape(text, file) {
  if (!(file.endsWith('.rs') || file.endsWith('.json') || file.endsWith('.mjs'))) return text;
  return text.replace(/\\\n\s*/g, '').replace(/\\n/g, '\n').replace(/\\t/g, ' ')
    .replace(/\\"/g, '"').replace(/\\\\/g, '\\');
}

function balanced(text, start) {
  let depth = 0;
  for (let index = start; index < text.length; index += 1) {
    if (text[index] === '(') depth += 1;
    else if (text[index] === ')') {
      depth -= 1;
      if (depth === 0) return text.slice(start, index + 1);
    }
  }
  return null;
}

function tokens(text) {
  return text.match(/"(?:[^"\\]|\\.)*"|[()[\]]|[^\s()[\]"]+/g) ?? [];
}

function read(list, position = 0) {
  const items = [];
  let index = position;
  while (index < list.length) {
    const token = list[index];
    if (token === '(' || token === '[') {
      const [child, next] = read(list, index + 1);
      items.push(child);
      index = next;
    } else if (token === ')' || token === ']') {
      return [items, index + 1];
    } else {
      items.push(token);
      index += 1;
    }
  }
  return [items, index];
}

const show = form => (Array.isArray(form) ? `(${form.map(show).join(' ')})` : form);
const atoms = form => (Array.isArray(form) ? form.flatMap(atoms) : [form]);

function contracts(files) {
  const distinct = new Map();
  let templates = 0;
  for (const file of files) {
    const text = unescape(readFileSync(join(root, file), 'utf8'), file);
    let at = text.indexOf('(contract 1');
    while (at >= 0) {
      const form = balanced(text, at);
      if (form) {
        const normalized = form.replace(/\s+/g, ' ');
        if (/\{[a-z_]+\}/.test(normalized)) templates += 1;
        else {
          const entry = distinct.get(normalized) ?? { text: normalized, files: new Set() };
          entry.files.add(file);
          distinct.set(normalized, entry);
        }
      }
      at = text.indexOf('(contract 1', at + 1);
    }
  }
  return { distinct: [...distinct.values()], templates };
}

function field(contract, name) {
  return contract.find(item => Array.isArray(item) && item[0] === name);
}

function classify(predicate) {
  if (predicate === undefined) return 'absent';
  if (predicate === 'true') return 'true';
  if (Array.isArray(predicate) && predicate.length === 3 && ['lt', 'le'].includes(predicate[0])) {
    const [, left, right] = predicate;
    const literal = value => typeof value === 'string' && /^-?\d+$/.test(value);
    const input = value => Array.isArray(value) && value[0] === 'in';
    if ((input(left) && literal(right)) || (literal(left) && input(right))) return 'i64-bound-on-input';
  }
  return 'other';
}

function survey(entries) {
  const rows = [];
  for (const entry of entries) {
    const [forms] = read(tokens(entry.text));
    const contract = forms[0];
    if (!Array.isArray(contract) || contract[0] !== 'contract') continue;
    const bindings = [...(field(contract, 'input')?.slice(1) ?? []), ...(field(contract, 'output')?.slice(1) ?? [])];
    const types = bindings.filter(Array.isArray).flatMap(binding => atoms(binding.slice(1)));
    const requires = field(contract, 'requires')?.[1];
    rows.push({
      name: contract[2] ?? null,
      files: [...entry.files].sort(),
      requires: requires === undefined ? null : show(requires),
      requires_class: classify(requires),
      nominal_type_mentions: types.filter(type => /^[A-Z]/.test(type) && !BUILTIN_TYPES.has(type)),
    });
  }
  const counts = {};
  for (const row of rows) counts[row.requires_class] = (counts[row.requires_class] ?? 0) + 1;
  const repeated = new Map();
  for (const row of rows.filter(row => row.requires_class !== 'true' && row.requires_class !== 'absent')) {
    const group = repeated.get(row.requires) ?? { requires: row.requires, class: row.requires_class, contracts: 0 };
    group.contracts += 1;
    repeated.set(row.requires, group);
  }
  return {
    distinct_contracts: rows.length,
    requires_classes: counts,
    nontrivial_requires: rows.filter(row => row.requires_class !== 'true' && row.requires_class !== 'absent')
      .map(row => ({ name: row.name, requires: row.requires, class: row.requires_class, files: row.files }))
      .sort((a, b) => a.requires.localeCompare(b.requires) || String(a.name).localeCompare(String(b.name))),
    repeated_nontrivial_requires: [...repeated.values()].filter(group => group.contracts > 1)
      .sort((a, b) => b.contracts - a.contracts || a.requires.localeCompare(b.requires)),
    contracts_mentioning_nominal_types: rows.filter(row => row.nominal_type_mentions.length > 0)
      .map(row => ({ name: row.name, types: row.nominal_type_mentions, files: row.files })),
  };
}

// Each probe: one subject the surface must refuse because it names an opaque type, and a
// structural control the same surface accepts, so the refusal is about the type and not the form.
const PROBES = [
  { id: 'mc1-opaque-input', command: 'explain-proof', file: 'positive.contract', expect: 'refused',
    source: '(contract 1 positive-consumer\n  (input (x Positive))\n  (output (y I64))\n  (program [ Positive.into ])\n  (requires true)\n  (ensures (lt 0 (out y))))\n' },
  { id: 'mc1-i64-control', command: 'explain-proof', file: 'i64.contract', expect: 'accepted',
    source: '(contract 1 i64-consumer\n  (input (x I64))\n  (output (y I64))\n  (program [ ])\n  (requires (lt 0 (in x)))\n  (ensures (lt 0 (out y))))\n' },
  { id: 'intrinsic-opaque-binder', command: 'verify-module', file: 'positive-module.noble', expect: 'refused',
    source: 'module m@1 [ opaque Positive I64 public export Positive proof 1 positive-refl : [ (Pi (x Positive) (Eq Positive x x)) ] [ (intro (x Positive) (refl x)) ] ]\n' },
  { id: 'intrinsic-i64-control', command: 'verify-module', file: 'i64-module.noble', expect: 'accepted',
    source: 'module m@1 [ proof 1 i64-refl : [ (Pi (x I64) (Eq I64 x x)) ] [ (intro (x I64) (refl x)) ] ]\n' },
];

function probe(cli, output, spec) {
  const path = join(output, 'probes', spec.file);
  writeFileSync(path, spec.source);
  const observed = spawnSync(cli, [spec.command, path], {
    cwd: output, encoding: 'utf8', timeout: 600_000, maxBuffer: 16 * 1024 * 1024,
    env: { PATH: '/run/current-system/sw/bin:/usr/bin:/bin', HOME: join(output, 'home'), LANG: 'C.UTF-8' },
  });
  const report = JSON.parse(observed.stdout);
  const diagnostics = report.diagnostics ?? (report.diagnostic ? [report.diagnostic] : []);
  // Accepted means the surface admitted the subject: a contract statement was generated, or a
  // module proof passed acceptance and reached the independent recheck.
  const admitted = spec.command === 'explain-proof'
    ? typeof report.generated_statement === 'string' && report.generated_statement.length > 0
    : report.stage === 'independent-recheck';
  const row = {
    id: spec.id, command: spec.command, source_sha256: digest(spec.source), exit: observed.status,
    outcome: report.outcome, stage: report.stage ?? null, diagnostics, admitted, expect: spec.expect,
  };
  requireThat((spec.expect === 'accepted') === admitted,
    `${spec.id}: expected ${spec.expect}, observed ${JSON.stringify(row)}`);
  return row;
}

function main() {
  const options = parseArguments(process.argv.slice(2));
  const output = resolve(options.output);
  requireThat(relative(root, output).startsWith('..'), 'output directory must be outside the source tree');
  requireThat(!existsSync(output), 'output directory must not exist yet');
  mkdirSync(join(output, 'probes'), { recursive: true });
  mkdirSync(join(output, 'home'));
  const cli = realpathSync(options.cli);
  const state = {
    commit: git(['rev-parse', 'HEAD']).trim(), tree: git(['rev-parse', 'HEAD^{tree}']).trim(),
    status: git(['status', '--porcelain', '--untracked-files=all']),
  };
  requireThat(state.status === '' || options.allowDirty, 'source worktree is dirty; re-run with --allow-dirty');
  const files = corpusFiles();
  const { distinct, templates } = contracts(files);
  const measured = survey(distinct);
  const probes = PROBES.map(spec => probe(cli, output, spec));
  const receipt = {
    schema: 'noble-opaque-invariant-survey/v1', result: 'measured',
    source_commit: state.commit, source_tree: state.tree, clean_source: state.status === '',
    cli: { path: cli, sha256: digest(readFileSync(cli)) },
    corpus: { files: files.length, files_with_contracts: new Set(distinct.flatMap(entry => [...entry.files])).size,
      format_templates_excluded: templates },
    ...measured,
    probes,
    non_claims: ['A survey of the tracked corpus, not of every possible contract.',
      'No invariant mechanism is built or proved; the probes show what the current surfaces admit.'],
  };
  writeFileSync(join(output, 'receipt.json'), `${JSON.stringify(receipt, null, 2)}\n`);
  process.stdout.write(`${JSON.stringify({ receipt: join(output, 'receipt.json'), distinct_contracts: measured.distinct_contracts,
    requires_classes: measured.requires_classes, nominal: measured.contracts_mentioning_nominal_types.length,
    probes: probes.map(row => `${row.id}:${row.admitted ? 'admitted' : 'refused'}`) })}\n`);
}

try {
  main();
} catch (error) {
  console.error(`[opaque-invariants] FAIL ${error.message}`);
  process.exitCode = 1;
}
