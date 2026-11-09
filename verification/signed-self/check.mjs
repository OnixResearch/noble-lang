// Lean-kernel check for the K-CHECK-02 signed-self reference model (#99).
//
// Builds proofs/m3/NobleM2/SignedSelf.lean with its import closure in an isolated project under the
// pinned Lean and audits every inventoried theorem's compiled statement and transitive axioms. The
// closure is read from the sources, and it must stay inside the reference model: no extraction,
// no Aeneas, no Mathlib. Every theorem is strict ({propext, Classical.choice, Quot.sound}) except
// the inventoried evaluation-closed fixtures. Those may also carry the per-declaration
// `native_decide` axiom of an evaluation-closed fixture, as the reference model's own acceptance
// fixtures do. This is a model theorem check: no correspondence with the extracted kernel.
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdirSync, readFileSync, realpathSync, writeFileSync } from 'node:fs';
import { userInfo } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(fileURLToPath(import.meta.url), '../../..');
const selection = JSON.parse(readFileSync(join(root, 'policy/tool-selection.json')));
const node = join(selection.tool_paths.node.output, 'bin/node');
const leanBin = join(selection.tool_paths.lean.output, 'bin');
const git = realpathSync('/run/current-system/sw/bin/git');
const { homedir: home, username } = userInfo();
const childEnv = {
  HOME: home, USER: username, LOGNAME: username, LANG: 'C.UTF-8', CI: '1',
  PATH: [leanBin, dirname(git), '/run/current-system/sw/bin', '/usr/bin', '/bin'].join(':'),
};
const requireThat = (condition, message) => { if (!condition) throw Error(message); };
const digest = path => createHash('sha256').update(readFileSync(path)).digest('hex');
const modelDir = 'proofs/m3/NobleM2';
const rootModule = 'NobleM2.SignedSelf';
const codeFile = `${modelDir}/SignedSelf.lean`;
const inventoryFile = 'proofs/m3/signed-self-inventory.json';
const standardAxioms = new Set(['propext', 'Classical.choice', 'Quot.sound']);
const nativeAxiom = /^([A-Za-z_][A-Za-z0-9_.]*)\._native\.native_decide\.ax_\d+_\d+$/;
let output;
const commands = [];

function run(command, argv, cwd = root) {
  const observed = spawnSync(command, argv, {
    cwd, env: { ...childEnv, TMPDIR: output }, timeout: 3_600_000,
    maxBuffer: 64 * 1024 * 1024, encoding: 'utf8',
  });
  commands.push({ command, argv: argv.map(arg => arg.replace(output, '$OUT')), exit: observed.status });
  requireThat(observed.status === 0, `${command} ${argv.join(' ')} failed: ${observed.status}\n${observed.stderr ?? ''}${observed.stdout ?? ''}`);
  return observed.stdout ?? '';
}

const moduleFile = name => `${modelDir}/${name.replace(/^NobleM2\./, '')}.lean`;

// The root module's transitive imports, read from the sources. Every import must be a module of
// the reference model.
function importClosure() {
  const seen = new Set();
  const pending = [rootModule];
  while (pending.length > 0) {
    const name = pending.pop();
    if (seen.has(name)) continue;
    seen.add(name);
    const text = readFileSync(join(root, moduleFile(name)), 'utf8');
    for (const match of text.matchAll(/^import\s+(\S+)/gm)) {
      requireThat(/^NobleM2\.[A-Za-z]+$/.test(match[1]),
        `${name} imports ${match[1]}, which is outside the reference model`);
      pending.push(match[1]);
    }
  }
  return [...seen].sort();
}

// The declared theorems of the module, with their full names, and the forbidden-construct scan.
function declaredTheorems() {
  const text = readFileSync(join(root, codeFile), 'utf8');
  const code = text.replace(/\/-[\s\S]*?-\//g, '').replace(/--.*$/gm, '');
  requireThat(!/\b(?:sorry|admit|implemented_by|unsafe|opaque|partial)\b/.test(code),
    `${codeFile}: forbidden proof or evaluation escape hatch`);
  requireThat(!/@\[\s*extern\b/.test(code), `${codeFile}: external implementation attribute`);
  requireThat(!/^\s*axiom\s/m.test(code), `${codeFile}: declares an axiom`);
  const declared = [];
  const namespaces = [];
  for (const line of code.split('\n')) {
    const opened = line.match(/^namespace\s+(\S+)/);
    if (opened) namespaces.push(opened[1]);
    const closed = line.match(/^end\s+(\S+)/);
    if (closed && namespaces.at(-1) === closed[1]) namespaces.pop();
    const theorem = line.match(/^\s*(?:@\[[^\]]*\]\s*)?(?:theorem|lemma)\s+(\S+)/);
    if (theorem) declared.push([...namespaces, theorem[1]].join('.'));
  }
  return { declared, nativeUses: (code.match(/\bnative_decide\b/g) ?? []).length };
}

function auditTheorems(project, inventory) {
  const auditFile = join(output, 'Audit.lean');
  const lines = [`import ${rootModule}`, ''];
  const positions = {};
  for (const { name } of inventory.theorems) {
    positions[lines.length + 1] = { name, kind: 'statement' }; lines.push(`#check @${name}`);
    positions[lines.length + 1] = { name, kind: 'axioms' }; lines.push(`#print axioms ${name}`);
  }
  writeFileSync(auditFile, lines.join('\n') + '\n');
  const outputText = run(join(leanBin, 'lake'), ['env', 'lean', '--json', auditFile], project);
  const records = {};
  for (const line of outputText.split('\n').filter(Boolean)) {
    const message = JSON.parse(line);
    requireThat(message.severity === 'information', `Lean audit diagnostic: ${line}`);
    const slot = positions[message.pos.line];
    requireThat(slot, `unexpected Lean audit output at ${message.pos.line}`);
    (records[slot.name] ??= {})[slot.kind] = message.data.trim();
  }
  const evaluationClosed = new Set(inventory.theorems.filter(row => row.evaluation_closed)
    .map(row => row.name));
  return inventory.theorems.map(row => {
    const record = records[row.name];
    requireThat(record?.statement && record?.axioms, `missing compiled statement/axioms: ${row.name}`);
    const match = record.axioms.match(/depends on axioms: \[([\s\S]*)\]/);
    const axioms = /does not depend on any axioms/.test(record.axioms) ? [] :
      (requireThat(match, `unparsable axiom report: ${row.name}`),
      match[1].split(',').map(value => value.trim()).filter(Boolean));
    for (const axiom of axioms) {
      if (standardAxioms.has(axiom)) continue;
      const native = axiom.match(nativeAxiom);
      requireThat(row.evaluation_closed === true && native && evaluationClosed.has(native[1]),
        `disallowed transitive axiom: ${row.name}: ${axiom}`);
    }
    return {
      ...row, status: 'proved', statement: record.statement.replace(/\s+/g, ' '), axioms,
      strict: axioms.every(axiom => standardAxioms.has(axiom)),
    };
  });
}

const argv = process.argv.slice(2);
const allowDirty = argv.includes('--allow-dirty');
if (allowDirty) argv.splice(argv.indexOf('--allow-dirty'), 1);
requireThat(argv.length === 2 && argv[0] === '--output',
  'usage: SELECTED_NODE verification/signed-self/check.mjs [--allow-dirty] --output NEW_EXTERNAL_DIRECTORY');
requireThat(realpathSync(process.execPath) === realpathSync(node), `use selected Node: ${node}`);
const forbidden = Object.keys(process.env).filter(key =>
  /^(?:LEAN(?:_|$)|LAKE(?:_|$)|BASH(?:_|$)|GIT_|NIX_|ELAN)/.test(key) && process.env[key] !== '');
requireThat(forbidden.length === 0, `unsafe caller environment: ${forbidden.join(', ')}`);
output = resolve(argv[1]);
requireThat(output !== root && !output.startsWith(`${root}/`), 'output must be external');
mkdirSync(output);
const state = () => ({
  commit: run(git, ['rev-parse', 'HEAD']).trim(),
  tree: run(git, ['rev-parse', 'HEAD^{tree}']).trim(),
  status: run(git, ['status', '--porcelain=v1', '--untracked-files=all']).trim(),
});
const initial = state();
requireThat(allowDirty || initial.status === '', `source worktree dirty: ${initial.status}`);
const inventory = JSON.parse(readFileSync(join(root, inventoryFile)));
const names = inventory.theorems.map(row => row.name);
requireThat(names.length > 0 && new Set(names).size === names.length, 'empty/duplicate theorem inventory');
const { declared, nativeUses } = declaredTheorems();
requireThat(names.length === declared.length && names.every(name => declared.includes(name)),
  `declaration/inventory mismatch: declared=${declared.join(',')} inventoried=${names.join(',')}`);
requireThat(nativeUses <= inventory.theorems.filter(row => row.evaluation_closed).length,
  'more native_decide uses than evaluation-closed fixtures');
const version = run(join(leanBin, 'lean'), ['--version']).trim();
requireThat(version.includes('version 4.35.0'), `wrong pinned Lean executable: ${version}`);
const closure = importClosure();
const project = join(output, 'project');
mkdirSync(join(project, 'NobleM2'), { recursive: true });
for (const name of closure) {
  writeFileSync(join(project, moduleFile(name).replace(`${modelDir}/`, 'NobleM2/')),
    readFileSync(join(root, moduleFile(name))));
}
writeFileSync(join(project, 'lean-toolchain'), readFileSync(join(root, 'proofs/m3/lean-toolchain')));
writeFileSync(join(project, 'lakefile.toml'),
  ['name = "noble_signed_self"', 'defaultTargets = ["NobleM2"]', '', '[[lean_lib]]', 'name = "NobleM2"',
    `roots = [${closure.map(name => JSON.stringify(name)).join(', ')}]`, ''].join('\n'));
run(join(leanBin, 'lake'), ['build'], project);
const theorems = auditTheorems(project, inventory);
const final = state();
requireThat(final.status === initial.status && final.commit === initial.commit, 'source tree changed during proof check');
const sources = [...closure.map(moduleFile), inventoryFile, 'verification/signed-self/check.mjs'];
const receipt = {
  schema: 'noble-signed-self-proof/v1', result: 'passed', issue: 99, requirement: 'K-CHECK-02',
  source_commit: initial.commit, source_tree: initial.tree, clean_source: initial.status === '',
  allow_dirty: allowDirty,
  lean: version, lean_sha256: digest(join(leanBin, 'lean')),
  closure,
  sources_sha256: Object.fromEntries(sources.map(file => [file, digest(join(root, file))])),
  claims: {
    model_theorems: theorems.length,
    strict_theorems: theorems.filter(row => row.strict).length,
    evaluation_closed_theorems: theorems.filter(row => !row.strict).map(row => row.name),
    implementation_theorems: 0,
    correspondence: 'none: the retained kernel extraction predates SignedSelf',
  },
  theorem_count: theorems.length, theorems, commands,
  scope: [
    'The signed check over the M2 reference model: a self site is the owner invocation the kernel types it as, marked by index.',
    'checkSigned_soundness: acceptance implies SignedDerives (the body derives the owner\'s declared signature at the owner witness, every self site at that witness).',
    'Module ownership, the public checker\'s refusal of SignedSelf nodes and self edges through ordinary invocations are outside the model.',
  ],
};
writeFileSync(join(output, 'receipt.json'), JSON.stringify(receipt, null, 2) + '\n');
process.stdout.write(`SIGNED-SELF: ${theorems.length} theorems (${receipt.claims.strict_theorems} strict); axioms accepted\n`);
