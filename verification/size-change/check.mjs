// Lean-kernel axiom lane for the size-change reference model (#101).
//
// Builds every `proofs/m3/NobleM2/SizeChange*.lean` module in an isolated project under the pinned
// Lean and audits each inventoried theorem's compiled statement and transitive axioms. The family's
// import closure is read from the sources and must stay inside the reference model: `Std` plus the
// size-change modules, no extraction, no Aeneas, no Mathlib.
//
// The policy is stricter than the m3 gate's: every theorem in this family must keep to
// {propext, Quot.sound}. The issue's decision procedure, its guarded run, its refusal witnesses and
// its matrix lane are constructive, and this lane is what keeps them so. This is a model check: no
// correspondence with the Rust front end is claimed here (that is `scenarios.mjs`).
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdirSync, readdirSync, readFileSync, realpathSync, writeFileSync } from 'node:fs';
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
const inventoryFile = 'proofs/m3/size-change-inventory.json';
const familyPrefix = 'SizeChange';
const strictAxioms = new Set(['propext', 'Quot.sound']);
let output;
const commands = [];

function run(command, argv, cwd = root) {
  const observed = spawnSync(command, argv, {
    cwd, env: { ...childEnv, TMPDIR: output }, timeout: 3_600_000,
    maxBuffer: 64 * 1024 * 1024, encoding: 'utf8',
  });
  commands.push({ command, argv: argv.map(arg => arg.replace(output, '$OUT')), exit: observed.status });
  requireThat(observed.status === 0,
    `${command} ${argv.join(' ')} failed: ${observed.status}\n${observed.stderr ?? ''}${observed.stdout ?? ''}`);
  return observed.stdout ?? '';
}

/// The family: every `SizeChange*.lean` module, and its transitive imports read from the sources.
function importClosure() {
  const modules = readdirSync(join(root, modelDir))
    .filter(name => name.startsWith(familyPrefix) && name.endsWith('.lean'))
    .map(name => `NobleM2.${name.replace(/\.lean$/, '')}`)
    .sort();
  requireThat(modules.length > 0, 'the size-change family is empty');
  const seen = new Set();
  const pending = [...modules];
  while (pending.length > 0) {
    const name = pending.pop();
    if (seen.has(name)) continue;
    seen.add(name);
    const text = readFileSync(join(root, `${modelDir}/${name.replace(/^NobleM2\./, '')}.lean`), 'utf8');
    for (const match of text.matchAll(/^import\s+(\S+)/gm)) {
      if (match[1] === 'Std') continue;
      requireThat(/^NobleM2\.SizeChange[A-Za-z]*$/.test(match[1]),
        `${name} imports ${match[1]}, which is outside the size-change reference model`);
      pending.push(match[1]);
    }
  }
  return { modules, closure: [...seen].sort() };
}

/// Each module's declared theorems, with their fully qualified names, and the construct scan.
function declaredTheorems(modules) {
  const declared = [];
  let nativeUses = 0;
  for (const module of modules) {
    const file = `${modelDir}/${module.replace(/^NobleM2\./, '')}.lean`;
    const text = readFileSync(join(root, file), 'utf8');
    const code = text.replace(/\/-[\s\S]*?-\//g, '').replace(/--.*$/gm, '');
    requireThat(!/\b(?:sorry|admit|implemented_by|run_cmd|run_elab|skipKernelTC|addDecl)\b/.test(code),
      `${file}: forbidden proof or evaluation escape hatch`);
    requireThat(!/^\s*(?:unsafe|partial|opaque|extern)\b/m.test(code),
      `${file}: forbidden opaque or external implementation`);
    requireThat(!/@\[\s*extern\b/.test(code), `${file}: external implementation attribute`);
    requireThat(!/^\s*#eval\b/m.test(code), `${file}: evaluation command in proof source`);
    requireThat(!/^\s*axiom\s/m.test(code), `${file}: declares an axiom`);
    nativeUses += (code.match(/\bnative_decide\b/g) ?? []).length;
    const namespaces = [];
    for (const line of code.split('\n')) {
      const opened = line.match(/^namespace\s+(\S+)/);
      if (opened) namespaces.push(...opened[1].split('.'));
      const closed = line.match(/^end\s+(\S+)/);
      if (closed) {
        for (let index = 0; index < closed[1].split('.').length; index += 1) namespaces.pop();
      }
      const theorem = line.match(/^\s*(?:@\[[^\]]*\]\s*)?(?:theorem|lemma)\s+(\S+)/);
      if (theorem) declared.push({ module, name: [...namespaces, theorem[1]].join('.') });
    }
  }
  return { declared, nativeUses };
}

function auditTheorems(project, inventory) {
  const auditFile = join(output, 'Audit.lean');
  const lines = [...inventory.modules.map(module => `import ${module}`), ''];
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
    requireThat(slot, `unexpected Lean audit output at line ${message.pos.line}`);
    (records[slot.name] ??= {})[slot.kind] = message.data.trim();
  }
  return inventory.theorems.map(row => {
    const record = records[row.name];
    requireThat(record?.statement && record?.axioms, `missing compiled statement/axioms: ${row.name}`);
    const match = record.axioms.match(/depends on axioms: \[([\s\S]*)\]/);
    const axioms = /does not depend on any axioms/.test(record.axioms) ? [] :
      (requireThat(match, `unparsable axiom report: ${row.name}`),
      match[1].split(',').map(value => value.trim()).filter(Boolean));
    for (const axiom of axioms) {
      requireThat(strictAxioms.has(axiom), `disallowed transitive axiom: ${row.name}: ${axiom}`);
    }
    return {
      ...row, status: 'proved', statement: record.statement.replace(/\s+/g, ' '), axioms,
      strict: axioms.every(axiom => strictAxioms.has(axiom)),
    };
  });
}

const argv = process.argv.slice(2);
const allowDirty = argv.includes('--allow-dirty');
if (allowDirty) argv.splice(argv.indexOf('--allow-dirty'), 1);
requireThat(argv.length === 2 && argv[0] === '--output',
  'usage: SELECTED_NODE verification/size-change/check.mjs [--allow-dirty] --output NEW_EXTERNAL_DIRECTORY');
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
const { modules, closure } = importClosure();
requireThat(JSON.stringify([...inventory.modules].sort()) === JSON.stringify(modules),
  `inventory modules differ from the family: ${modules.join(',')}`);
const { declared, nativeUses } = declaredTheorems(modules);
requireThat(nativeUses === 0, 'the size-change family must stay evaluation-free');
requireThat(names.length === declared.length,
  `declaration/inventory count mismatch: declared=${declared.length} inventoried=${names.length}`);
const declaredNames = new Set(declared.map(row => row.name));
for (const name of names) requireThat(declaredNames.has(name), `inventoried theorem is not declared: ${name}`);
for (const row of declared) requireThat(names.includes(row.name), `declared theorem is not inventoried: ${row.name}`);
const version = run(join(leanBin, 'lean'), ['--version']).trim();
requireThat(version.includes('version 4.35.0'), `wrong pinned Lean executable: ${version}`);
const project = join(output, 'project');
mkdirSync(join(project, 'NobleM2'), { recursive: true });
for (const name of closure) {
  writeFileSync(join(project, `${name.replace(/^NobleM2\./, 'NobleM2/')}.lean`),
    readFileSync(join(root, `${modelDir}/${name.replace(/^NobleM2\./, '')}.lean`)));
}
writeFileSync(join(project, 'lean-toolchain'), readFileSync(join(root, 'proofs/m3/lean-toolchain')));
writeFileSync(join(project, 'lakefile.toml'),
  ['name = "noble_size_change"', 'defaultTargets = ["NobleM2"]', '', '[[lean_lib]]', 'name = "NobleM2"',
    `roots = [${closure.map(name => JSON.stringify(name)).join(', ')}]`, ''].join('\n'));
run(join(leanBin, 'lake'), ['build'], project);
const theorems = auditTheorems(project, inventory);
const final = state();
requireThat(final.status === initial.status && final.commit === initial.commit, 'source tree changed during proof check');
const sources = [...closure.map(name => `${modelDir}/${name.replace(/^NobleM2\./, '')}.lean`),
  'proofs/m3/NobleM2.lean', inventoryFile, 'verification/size-change/check.mjs'];
const receipt = {
  schema: 'noble-size-change-proof/v1', result: 'passed', issue: 101,
  source_commit: initial.commit, source_tree: initial.tree, clean_source: initial.status === '',
  allow_dirty: allowDirty,
  lean: version, lean_sha256: digest(join(leanBin, 'lean')),
  modules, closure,
  sources_sha256: Object.fromEntries(sources.map(file => [file, digest(join(root, file))])),
  claims: {
    model_theorems: theorems.length,
    strict_theorems: theorems.filter(row => row.strict).length,
    axioms: [...strictAxioms].sort(),
    implementation_theorems: 0,
    correspondence: 'none: the front end is checked in verification/size-change/scenarios.mjs',
  },
  theorem_count: theorems.length, theorems, commands,
  scope: [
    'The size-change reference model: one definition, several self-call sites over shared measures, the argument classifier, the guarded run, the constructive refusal witness, and the matrix lane.',
    'Every theorem keeps to {propext, Quot.sound}; a theorem needing Classical.choice fails this lane.',
    'The model proves classified relations, not an execution of the Rust front end; the scenarios lane records what the CLI observed.',
  ],
};
writeFileSync(join(output, 'receipt.json'), JSON.stringify(receipt, null, 2) + '\n');
process.stdout.write(
  `SIZE-CHANGE: ${theorems.length} theorems (${receipt.claims.strict_theorems} strict); axioms accepted\n`);
