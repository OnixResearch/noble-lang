// Lean-kernel check for the region-trace relation pilot (`.cairn/changes/region-trace-relation`,
// WIT-TRACE-01). Builds `proofs/m5/RegionTrace.lean` in an isolated project with the pinned Lean
// 4.31 toolchain and audits every inventoried theorem's statement and transitive axioms.
// This is a model theorem check: no emitted-step correspondence, no Wasmtime claim.
import { spawnSync } from 'node:child_process';
import { mkdirSync, readFileSync, realpathSync, writeFileSync } from 'node:fs';
import { userInfo } from 'node:os';
import { basename, dirname, join, resolve } from 'node:path';
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
const codeFile = 'proofs/m5/RegionTrace.lean';
const inventoryFile = 'proofs/m5/region-trace-inventory.json';
const standardAxioms = new Set(['propext', 'Classical.choice', 'Quot.sound']);
let output;
const commands = [];
function run(command, argv, cwd = root) {
  const observed = spawnSync(command, argv, {
    cwd, env: { ...childEnv, TMPDIR: output }, timeout: 3_600_000,
    maxBuffer: 64 * 1024 * 1024, encoding: 'utf8',
  });
  commands.push({ command, argv, exit: observed.status });
  requireThat(observed.status === 0, `${command} ${argv.join(' ')} failed: ${observed.status}\n${observed.stderr ?? ''}`);
  return observed.stdout ?? '';
}
function checkDeclarations(inventory) {
  const text = readFileSync(join(root, codeFile), 'utf8');
  const code = text.replace(/\/-[\s\S]*?-\//g, '').replace(/--.*$/gm, '');
  requireThat(!/\b(?:sorry|admit|native_decide|implemented_by|unsafe|opaque|partial)\b/.test(code),
    `${codeFile}: forbidden proof or evaluation escape hatch`);
  requireThat(!/@\[\s*extern\b/.test(code), `${codeFile}: external implementation attribute`);
  requireThat(!/^\s*axiom\s/m.test(code), 'model declares an axiom');
  const names = inventory.theorems.map(row => row.name);
  requireThat(names.length > 0 && new Set(names).size === names.length, 'empty/duplicate theorem inventory');
  const declared = [];
  let namespace = '';
  for (const line of code.split('\n')) {
    const opened = line.match(/^namespace\s+(\S+)/);
    if (opened) namespace = opened[1];
    const theorem = line.match(/^\s*(?:@\[[^\]]*\]\s*)?(?:theorem|lemma)\s+(\S+)/);
    if (theorem) declared.push(`${namespace}.${theorem[1]}`);
  }
  requireThat(names.every(name => name.startsWith('RegionTrace.')), 'unrelated theorem in inventory');
  requireThat(names.length === declared.length && names.every(name => declared.includes(name)),
    `declaration/inventory mismatch: declared=${declared.join(',')} inventoried=${names.join(',')}`);
  return names;
}
function auditTheorems(project, inventory, names) {
  const auditFile = join(output, 'Audit.lean');
  const lines = ['import RegionTrace', ''];
  const positions = {};
  for (const name of names) {
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
  return names.map(name => {
    const row = records[name];
    const metadata = inventory.theorems.find(item => item.name === name);
    requireThat(row?.statement && row?.axioms, `missing compiled statement/axioms: ${name}`);
    const match = row.axioms.match(/depends on axioms: \[([\s\S]*)\]/);
    const axioms = /does not depend on any axioms/.test(row.axioms) ? [] :
      (requireThat(match, `unparsable axiom report: ${name}`), match[1].split(',').map(value => value.trim()).filter(Boolean));
    const allowed = new Set(metadata.allowed_standard_axioms ?? standardAxioms);
    requireThat(axioms.every(axiom => allowed.has(axiom)), `disallowed transitive axiom: ${name}: ${axioms.join(', ')}`);
    return { ...metadata, status: 'proved', statement: row.statement.replace(/\s+/g, ' '), axioms };
  });
}
const argv = process.argv.slice(2);
const allowDirty = argv.includes('--allow-dirty');
if (allowDirty) argv.splice(argv.indexOf('--allow-dirty'), 1);
requireThat(argv.length === 2 && argv[0] === '--output',
  'usage: SELECTED_NODE verification/region-trace/check.mjs [--allow-dirty] --output NEW_EXTERNAL_DIRECTORY');
requireThat(realpathSync(process.execPath) === realpathSync(node), `use selected Node: ${node}`);
const forbidden = Object.keys(process.env).filter(key =>
  /^(?:LEAN(?:_|$)|LAKE(?:_|$)|BASH(?:_|$)|GIT_|NIX_|ELAN)/.test(key) && process.env[key] !== '');
requireThat(forbidden.length === 0, `unsafe caller environment: ${forbidden.join(', ')}`);
output = resolve(argv[1]);
requireThat(output !== root && !output.startsWith(`${root}/`), 'output must be external');
mkdirSync(output);
const initial = {
  commit: run(git, ['rev-parse', 'HEAD']).trim(),
  tree: run(git, ['rev-parse', 'HEAD^{tree}']).trim(),
  status: run(git, ['status', '--porcelain=v1', '--untracked-files=all']).trim(),
};
requireThat(allowDirty || initial.status === '', `source worktree dirty: ${initial.status}`);
const inventory = JSON.parse(readFileSync(join(root, inventoryFile)));
const names = checkDeclarations(inventory);
const version = run(join(leanBin, 'lean'), ['--version']).trim();
requireThat(version.includes('version 4.31.0') && version.includes('68218e876d2a38b1985b8590fff244a83c321783'),
  'wrong pinned Lean executable');
const project = join(output, 'project');
mkdirSync(project);
writeFileSync(join(project, 'RegionTrace.lean'), readFileSync(join(root, codeFile)));
writeFileSync(join(project, 'lean-toolchain'), readFileSync(join(root, 'proofs/m5/lean-toolchain')));
writeFileSync(join(project, 'lakefile.toml'),
  ['name = "noble_region_trace"', 'defaultTargets = ["RegionTrace"]', '',
   '[[lean_lib]]', 'name = "RegionTrace"', ''].join('\n'));
run(join(leanBin, 'lake'), ['build', 'RegionTrace'], project);
const theorems = auditTheorems(project, inventory, names);
const final = {
  commit: run(git, ['rev-parse', 'HEAD']).trim(),
  tree: run(git, ['rev-parse', 'HEAD^{tree}']).trim(),
  status: run(git, ['status', '--porcelain=v1', '--untracked-files=all']).trim(),
};
requireThat(final.status === initial.status && final.commit === initial.commit, 'source tree changed during proof check');
const bindings = {
  // WIT-TRACE-06: every revision this pilot's verdicts depend on is bound or explicitly absent.
  model: {
    lean: version,
    lean_sha256: spawnSync('sha256sum', [join(leanBin, 'lean')], { encoding: 'utf8' }).stdout.split(' ')[0],
    sources_sha256: Object.fromEntries([codeFile, inventoryFile].map(file => [file,
      spawnSync('sha256sum', [join(root, file)], { encoding: 'utf8' }).stdout.split(' ')[0]])),
  },
  component: null,
  helper: null,
  engine: null,
  absent_bindings_reason:
    'This pilot proves a relation between two handwritten alphabets. No component, lowering helper or ' +
    'engine instance is a subject of these theorems, so binding one would imply a correspondence this ' +
    'package does not establish. WIT-TRACE-06 stays open until the correspondence route has one.',
};
const claims = {
  model_theorems: names.length,
  implementation_theorems: 0,
  correspondence: 'none',
  liveness: 'none',
  cancellation: 'unsupported and unmodelled',
};
const receipt = {
  schema: 'noble-region-trace-proof/v1', result: 'passed',
  source_commit: initial.commit, source_tree: initial.tree, clean_source: initial.status === '',
  allow_dirty: allowDirty, lean: version,
  bindings, claims,
  sources_sha256: Object.fromEntries([codeFile, inventoryFile,
    'verification/region-trace/check.mjs'].map(file => [file,
      spawnSync('sha256sum', [join(root, file)], { encoding: 'utf8' }).stdout.split(' ')[0]])),
  theorem_count: names.length, theorems, commands,
  implementation_theorem: false,
  scope: ['.cairn/changes/region-trace-relation WIT-TRACE-01 model: source alphabet, target ABI alphabet and the relation.',
    'The target alphabet is a handwritten machine. No emitted ABI step correspondence, no Wasmtime/Canonical ABI claim,',
    'no Rust host or lowering claim, and no liveness claim. Pairing and the host supplying the exact identity stay premises.'],
};
writeFileSync(join(output, 'receipt.json'), JSON.stringify(receipt, null, 2) + '\n');
process.stdout.write(`REGION-TRACE: ${names.length} model theorems; strict axioms accepted\n`);
