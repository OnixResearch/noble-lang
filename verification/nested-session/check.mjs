// Lean-kernel check for the nested session protocol model (`.cairn/changes/nested-session-protocol`,
// WIT-SESSION-01/02). Builds `proofs/m6/NestedSession.lean` in an isolated project with the pinned Lean
// 4.31 toolchain and audits every inventoried theorem's statement and transitive axioms.
// This is a model theorem check: no lowering, runtime, scheduler or peer claim.
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
const codeFile = 'proofs/m6/NestedSession.lean';
const inventoryFile = 'proofs/m6/nested-session-inventory.json';
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
  requireThat(names.every(name => name.startsWith('NestedSession.')), 'unrelated theorem in inventory');
  requireThat(names.length === declared.length && names.every(name => declared.includes(name)),
    `declaration/inventory mismatch: declared=${declared.join(',')} inventoried=${names.join(',')}`);
  return names;
}
function auditTheorems(project, inventory, names) {
  const auditFile = join(output, 'Audit.lean');
  const lines = ['import NestedSession', ''];
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
  'usage: SELECTED_NODE verification/nested-session/check.mjs [--allow-dirty] --output NEW_EXTERNAL_DIRECTORY');
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
writeFileSync(join(project, 'NestedSession.lean'), readFileSync(join(root, codeFile)));
writeFileSync(join(project, 'lean-toolchain'), readFileSync(join(root, 'proofs/m6/lean-toolchain')));
writeFileSync(join(project, 'lakefile.toml'),
  ['name = "noble_nested_session"', 'defaultTargets = ["NestedSession"]', '',
   '[[lean_lib]]', 'name = "NestedSession"', ''].join('\n'));
run(join(leanBin, 'lake'), ['build', 'NestedSession'], project);
const theorems = auditTheorems(project, inventory, names);
const final = {
  commit: run(git, ['rev-parse', 'HEAD']).trim(),
  tree: run(git, ['rev-parse', 'HEAD^{tree}']).trim(),
  status: run(git, ['status', '--porcelain=v1', '--untracked-files=all']).trim(),
};
requireThat(final.status === initial.status && final.commit === initial.commit, 'source tree changed during proof check');
const bindings = {
  // WIT-SESSION-06: every revision this model's verdicts depend on is bound or explicitly absent.
  model: {
    lean: version,
    lean_sha256: spawnSync('sha256sum', [join(leanBin, 'lean')], { encoding: 'utf8' }).stdout.split(' ')[0],
    sources_sha256: Object.fromEntries([codeFile, inventoryFile].map(file => [file,
      spawnSync('sha256sum', [join(root, file)], { encoding: 'utf8' }).stdout.split(' ')[0]])),
  },
  // The closed definitions table, the endpoint identity fields and the mailbox accounting are this
  // model's own constants: their bytes are the source hash above, so they are bound through it and
  // named here rather than left implicit.
  protocol_definitions:
    'proofs/m6/NestedSession.lean: exampleProtocol (legacy depth-only control), balancedProtocol (bounded return stack), choiceProtocol',
  endpoint_identity_fields: ['channel', 'responder', 'protocol', 'state', 'generation', 'closed'],
  queue_accounting: 'depth derived from the enqueued/dequeued ledger, never stored beside it',
  helper: null,
  runtime: null,
  peer: null,
  absent_bindings_reason:
    'This model proves grammar, duality and endpoint-resource theorems over a handwritten closed ' +
    'definitions table. No lowering helper, runtime or peer is a subject of these theorems, so binding ' +
    'one would imply a correspondence this package does not establish. WIT-SESSION-06 stays open.',
};
const claims = {
  model_theorems: names.length,
  implementation_theorems: 0,
  correspondence: 'none',
  liveness: 'none',
  progress: 'guest-local only: an open endpoint whose resolved node declares the action has a successor; no queue, peer, scheduler or host premise',
  cancellation: 'an event class that cuts the committed prefix; it carries no endpoint or session transition',
  nested_recursion: 'the balanced table\'s unbounded trace language sendⁿ·recvⁿ is not regular and needs the return stack; this model admits it only up to the declared stack limit, and the dropped-continuation mutant accepts an unbalanced trace and refuses a balanced one; no runtime correspondence',
  type_equality: 'no algorithm; duality is definitional',
};
const receipt = {
  schema: 'noble-nested-session-proof/v1', result: 'passed',
  source_commit: initial.commit, source_tree: initial.tree, clean_source: initial.status === '',
  allow_dirty: allowDirty, lean: version,
  bindings, claims,
  sources_sha256: Object.fromEntries([codeFile, inventoryFile,
    'verification/nested-session/check.mjs'].map(file => [file,
      spawnSync('sha256sum', [join(root, file)], { encoding: 'utf8' }).stdout.split(' ')[0]])),
  theorem_count: names.length, theorems, commands,
  implementation_theorem: false,
  scope: ['.cairn/changes/nested-session-protocol model: finite grammar, duality, the endpoint resource and a bounded mailbox.',
    'The definitions table is handwritten. No lowering, runtime, scheduler, queue or peer correspondence is claimed;',
    'the only progress statement is guest-local; the balanced table\'s unbounded sendⁿ·recvⁿ language needs a return stack and this model admits it only up to the declared limit; endpoint delegation is excluded and send is never modelled as receipt.'],
};
writeFileSync(join(output, 'receipt.json'), JSON.stringify(receipt, null, 2) + '\n');
process.stdout.write(`NESTED-SESSION: ${names.length} model theorems; strict axioms accepted\n`);
