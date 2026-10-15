// Reproducible check of the issue #15 Lean foundation (proofs/foundation).
// Builds the library with the policy-selected Lean, prints the axioms and the
// statement of every inventoried theorem, refuses unlisted theorems, sorry,
// native evaluation and non-standard axioms, and emits a source-bound receipt.
// Run with the Node selected by policy/tool-selection.json under `env -i`
// (see proofs/foundation/README.md).
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, readdirSync, readFileSync, realpathSync, rmSync, writeFileSync } from 'node:fs';
import { userInfo } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(fileURLToPath(import.meta.url), '../../..');
const project = join(root, 'proofs/foundation');
const selection = JSON.parse(readFileSync(join(root, 'policy/tool-selection.json')));
const leanBin = join(selection.tool_paths.lean.output, 'bin');
const node = join(selection.tool_paths.node.output, 'bin/node');
const git = realpathSync('/run/current-system/sw/bin/git');
const { homedir: home, username } = userInfo();
const childBase = Object.freeze({
  HOME: home, USER: username, LOGNAME: username, LANG: 'C.UTF-8', CI: '1', GIT_TERMINAL_PROMPT: '0',
  PATH: [leanBin, dirname(git), '/run/current-system/sw/bin', '/usr/bin', '/bin'].join(':'),
});
const mathlibRevision = 'fabf563a7c95a166b8d7b6efca11c8b4dc9d911f';
const leanCommit = '68218e876d2a38b1985b8590fff244a83c321783';

const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const fileHash = path => digest(readFileSync(path));
const requireThat = (condition, message) => { if (!condition) throw Error(message); };
function run(command, args, options = {}) {
  const result = spawnSync(command, args, { encoding: 'utf8', maxBuffer: 64 * 1024 * 1024,
    timeout: 3600000, env: childBase, ...options });
  if (result.error || (result.status !== 0 && !options.allowFailure)) {
    throw Error(`${command} ${args.join(' ')} failed: ${result.error?.message ?? result.status}\n${result.stderr}\n${result.stdout}`);
  }
  return options.allowFailure ? result : result.stdout.trim();
}

const args = process.argv.slice(2);
const allowDirty = args[0] === '--allow-dirty' ? (args.shift(), true) : false;
requireThat(args.length === 0 || (args.length === 2 && args[0] === '--output'),
  'usage: check.mjs [--allow-dirty] [--output NEW_DIR]');
const forbidden = Object.keys(process.env).filter(key =>
  /^(?:LEAN(?:_|$)|LAKE(?:_|$)|NODE(?:_|$)|BASH(?:_|$)|ENV$|LD_|DYLD_|GIT_|NIX_|ELAN)/.test(key)
  && process.env[key] !== '');
if (forbidden.length) {
  console.error(`[foundation] FAIL unsafe caller environment: ${forbidden.sort().join(', ')}`);
  process.exit(1);
}
const retained = args.length === 2;
const scratch = retained ? resolve(args[1]) : mkdtempSync(join(home, '.cache/noble-foundation-'));
if (retained) mkdirSync(scratch);
try {
  requireThat(realpathSync(process.execPath) === realpathSync(node), `use selected Node: ${node}`);
  const gitState = cwd => ({
    head: run(git, ['rev-parse', 'HEAD'], { cwd }),
    tree: run(git, ['rev-parse', 'HEAD^{tree}'], { cwd }),
    status: run(git, ['status', '--porcelain=v1', '--untracked-files=all'], { cwd }),
  });
  const initial = gitState(root);
  requireThat(allowDirty || initial.status === '', `Noble source worktree dirty: ${initial.status}`);

  // The foundation shares the m3/mc1 Lean and Mathlib pins exactly.
  for (const file of ['lean-toolchain', 'lake-manifest.json']) {
    requireThat(readFileSync(join(project, file), 'utf8') === readFileSync(join(root, 'proofs/m3', file), 'utf8'),
      `proofs/foundation/${file} differs from proofs/m3/${file}`);
  }
  const manifest = JSON.parse(readFileSync(join(project, 'lake-manifest.json')));
  const mathlib = manifest.packages.find(pkg => pkg.name === 'mathlib');
  requireThat(mathlib?.rev === mathlibRevision, 'Mathlib manifest pin differs');
  const packages = {};
  for (const pkg of manifest.packages) {
    const state = gitState(join(project, '.lake/packages', pkg.name));
    requireThat(state.head === pkg.rev && state.status === '', `package ${pkg.name} checkout differs from manifest`);
    packages[pkg.name] = { rev: state.head, tree: state.tree };
  }
  const lean = join(leanBin, 'lean');
  const lake = join(leanBin, 'lake');
  const version = run(lean, ['--version']);
  requireThat(version.includes('version 4.31.0') && version.includes(leanCommit), 'wrong Lean executable');

  // Source hygiene: no escape hatches anywhere in the foundation sources.
  const sourceDir = join(project, 'NobleFoundation');
  const sources = ['NobleFoundation.lean', ...readdirSync(sourceDir).filter(f => f.endsWith('.lean')).sort()
    .map(f => `NobleFoundation/${f}`)];
  const declared = [];
  for (const file of sources) {
    const text = readFileSync(join(project, file), 'utf8');
    const code = text.replace(/\/-[\s\S]*?-\//g, '').replace(/--.*$/gm, '');
    for (const word of ['sorry', 'admit', 'native_decide', 'implemented_by', '@[extern', 'unsafe ', 'opaque ', 'partial ']) {
      requireThat(!code.includes(word), `${file}: forbidden construct ${word}`);
    }
    requireThat(!/^\s*axiom\s/m.test(code), `${file}: declares an axiom`);
    let namespace = '';
    for (const line of code.split('\n')) {
      const ns = line.match(/^namespace\s+(\S+)/);
      if (ns) namespace = ns[1];
      const th = line.match(/^\s*(?:@\[[^\]]*\]\s*)?theorem\s+(\S+)/);
      if (th) declared.push(th[1].startsWith('_root_.') ? th[1].slice(7) : `${namespace}.${th[1]}`);
    }
  }
  const inventory = JSON.parse(readFileSync(join(project, 'inventory.json')));
  const names = inventory.theorems.map(t => t.name);
  requireThat(new Set(names).size === names.length, 'duplicate inventory names');
  const unlisted = declared.filter(name => !names.includes(name));
  const missing = names.filter(name => !declared.includes(name));
  requireThat(unlisted.length === 0, `theorems missing from inventory: ${unlisted.join(', ')}`);
  requireThat(missing.length === 0, `inventory names not declared: ${missing.join(', ')}`);

  const env = { ...childBase, TMPDIR: scratch };
  const build = run(lake, ['build', 'NobleFoundation'], { cwd: project, env });
  requireThat(!/warning|error/i.test(build.split('\n').filter(l => !/^(✔|⣿|Build completed)/.test(l)).join('\n')),
    `lake build reported diagnostics:\n${build}`);

  // Audit: one `#check` and one `#print axioms` per inventoried theorem.
  const audit = join(scratch, 'Audit.lean');
  const lines = ['import NobleFoundation', ''];
  const lineOf = {};
  for (const name of names) {
    lineOf[lines.length + 1] = { name, kind: 'check' }; lines.push(`#check @${name}`);
    lineOf[lines.length + 1] = { name, kind: 'axioms' }; lines.push(`#print axioms ${name}`);
  }
  writeFileSync(audit, lines.join('\n') + '\n');
  const raw = run(lake, ['env', 'lean', '--json', audit], { cwd: project, env });
  const records = {};
  for (const line of raw.split('\n').filter(Boolean)) {
    const message = JSON.parse(line);
    requireThat(message.severity === 'information', `audit diagnostic: ${line}`);
    const slot = lineOf[message.pos.line];
    requireThat(slot, `unexpected audit message at line ${message.pos.line}`);
    (records[slot.name] ??= {})[slot.kind] = message.data.trim();
  }
  const allowed = new Set(inventory.allowed_axioms);
  const theorems = inventory.theorems.map(entry => {
    const record = records[entry.name] ?? {};
    requireThat(record.check && record.axioms, `${entry.name}: missing audit output`);
    let axioms;
    if (/does not depend on any axioms/.test(record.axioms)) axioms = [];
    else {
      const list = record.axioms.match(/depends on axioms: \[([\s\S]*)\]/);
      requireThat(list, `${entry.name}: unparsable axioms: ${record.axioms}`);
      axioms = list[1].split(',').map(s => s.trim()).filter(Boolean);
    }
    const extra = axioms.filter(axiom => !allowed.has(axiom));
    requireThat(extra.length === 0, `${entry.name}: disallowed axioms ${extra.join(', ')}`);
    return { ...entry, status: 'proved', statement: record.check.replace(/\s+/g, ' '), axioms };
  });

  const final = gitState(root);
  requireThat(final.head === initial.head && final.tree === initial.tree && final.status === initial.status,
    'Noble source worktree changed during check');
  const hashed = [
    ...sources.map(f => `proofs/foundation/${f}`),
    'proofs/foundation/inventory.json', 'proofs/foundation/lakefile.toml',
    'proofs/foundation/lake-manifest.json', 'proofs/foundation/lean-toolchain',
    'proofs/m3/MathlibI64.lean', 'proofs/mc1/NobleContracts/Model.lean',
    'proofs/mc1/NobleContracts/Rules.lean', 'proofs/mc1/NobleContracts/Obligation.lean',
    'proofs/mc1/NobleContracts/Composition.lean', 'proofs/mc1/NobleContracts/Examples.lean',
    'policy/tool-selection.json', 'verification/foundation/check.mjs',
  ];
  const result = {
    schema: 'noble-foundation-check/v1',
    scope: inventory.scope,
    source_commit: initial.head, source_tree: initial.tree,
    clean_source: initial.status === '', allow_dirty: allowDirty,
    lean: version, lean_path: lean, lean_sha256: fileHash(realpathSync(lean)),
    mathlib_commit: mathlibRevision, packages,
    mathlib_oleans: 'downloaded Lake cache (lake exe cache get), not rebuilt: separate trust assumption',
    sources_sha256: Object.fromEntries(hashed.map(p => [p, fileHash(join(root, p))])),
    allowed_axioms: inventory.allowed_axioms,
    theorem_count: theorems.length,
    theorems,
    implementation_theorem: false,
    artifact_directory: retained ? scratch : null,
  };
  if (retained) writeFileSync(join(scratch, 'foundation-check.json'), JSON.stringify(result, null, 2) + '\n');
  console.log(JSON.stringify({ ...result, theorems: theorems.map(t => `${t.name} [${t.axioms.join(', ')}]`) }, null, 2));
} catch (error) {
  console.error(`[foundation] FAIL ${error.message}`);
  process.exitCode = 1;
} finally {
  if (!retained) rmSync(scratch, { recursive: true, force: true });
}
