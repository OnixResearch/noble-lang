// Lean-kernel check for issue #51's bounded host-locality model.
// Rebuild the pinned M5 generated sources; never import archived .olean files.
import { spawnSync } from 'node:child_process';
import { mkdirSync, readFileSync, readdirSync, realpathSync, symlinkSync, writeFileSync } from 'node:fs';
import { userInfo } from 'node:os';
import { basename, dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { digest, fileHash } from '../foundation/execution.mjs';

const root = resolve(fileURLToPath(import.meta.url), '../../..');
const selection = JSON.parse(readFileSync(join(root, 'policy/tool-selection.json')));
const node = join(selection.tool_paths.node.output, 'bin/node');
const leanBin = join(selection.tool_paths.lean.output, 'bin');
const git = realpathSync('/run/current-system/sw/bin/git');
const tar = realpathSync('/run/current-system/sw/bin/tar');
const { homedir: home, username } = userInfo();
const childEnv = {
  HOME: home, USER: username, LOGNAME: username, LANG: 'C.UTF-8', CI: '1', GIT_TERMINAL_PROMPT: '0',
  PATH: [leanBin, dirname(git), '/run/current-system/sw/bin', '/usr/bin', '/bin'].join(':'),
};
const requireThat = (condition, message) => { if (!condition) throw Error(message); };
const json = value => JSON.stringify(value, null, 2) + '\n';
const codeFile = 'proofs/m5/HostLocality.lean';
const inventoryFile = 'proofs/m5/host-locality-inventory.json';
const profileFile = 'verification/host-locality/model-inputs.json';
const reusedNames = [
  'M5Resources.transition_refines', 'M5Resources.table_decide_refines',
  'M5Resources.normal_completion_owner_once', 'M5Resources.release_refines',
  'M5Resources.retired_completion_idempotent',
];
const generatedMembers = [
  './workspace/proofs/m3/NobleKernel.lean',
  ...['Types', 'TypesExternal', 'Funs', 'FunsExternal'].map(name =>
    `./workspace/proofs/m3/NobleKernel/${name}.lean`),
  './workspace/proofs/m5/M5Resources.lean',
];
let commandSerial = 0;
const commandArtifacts = {};
let output;
function run(command, argv, cwd = root) {
  const observed = spawnSync(command, argv, {
    cwd, env: { ...childEnv, TMPDIR: output }, timeout: 3_600_000,
    maxBuffer: 64 * 1024 * 1024, encoding: null,
  });
  const directory = join(output, 'commands', String(++commandSerial).padStart(3, '0'));
  mkdirSync(directory);
  const stdout = observed.stdout ?? Buffer.alloc(0);
  const stderr = observed.stderr ?? Buffer.alloc(0);
  writeFileSync(join(directory, 'stdout'), stdout);
  writeFileSync(join(directory, 'stderr'), stderr);
  const record = {
    command, executable_sha256: fileHash(command), argv, cwd, exit: observed.status,
    signal: observed.signal, error: observed.error?.message ?? null,
    stdout_sha256: digest(stdout), stderr_sha256: digest(stderr),
  };
  const commandJson = json(record);
  writeFileSync(join(directory, 'command.json'), commandJson);
  const prefix = `commands/${basename(directory)}`;
  commandArtifacts[`${prefix}/stdout`] = record.stdout_sha256;
  commandArtifacts[`${prefix}/stderr`] = record.stderr_sha256;
  commandArtifacts[`${prefix}/command.json`] = digest(Buffer.from(commandJson));
  requireThat(!observed.error && observed.signal === null && observed.status === 0,
    `${command} ${argv.join(' ')} failed: ${observed.error?.message ?? observed.status}\n${stderr}\n${stdout}`);
  return stdout.toString('utf8');
}
function sourceFiles(directory) {
  return readdirSync(join(root, directory), { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name))
    .flatMap(entry => {
      const file = `${directory}/${entry.name}`;
      requireThat(!entry.isSymbolicLink(), `unexpected source symlink: ${file}`);
      return entry.isDirectory() ? sourceFiles(file) : [file];
    });
}
function gitState() {
  return Object.fromEntries([
    ['source_commit', ['rev-parse', 'HEAD']], ['source_tree', ['rev-parse', 'HEAD^{tree}']],
    ['status', ['status', '--porcelain=v1', '--untracked-files=all']],
  ].map(([name, argv]) => [name, run(git, argv).trim()]));
}
function checkedSources(profile) {
  const files = [
    codeFile, inventoryFile, profileFile, 'verification/host-locality/check.mjs',
    'proofs/m5/M5Resources.lean', 'proofs/m5/lake-manifest.json', 'proofs/m5/lean-toolchain',
    profile.source, profile.historical_extraction_receipt, 'policy/tool-selection.json',
    ...sourceFiles('crates/noble-kernel/src/resources'),
  ];
  return Object.fromEntries(files.map(file => [file, fileHash(join(root, file))]));
}
function checkDeclarations(inventory) {
  const text = readFileSync(join(root, codeFile), 'utf8');
  const code = text.replace(/\/-[\s\S]*?-\//g, '').replace(/--.*$/gm, '');
  requireThat(!/\b(?:sorry|admit|native_decide|implemented_by|unsafe|opaque|partial)\b/.test(code),
    `${codeFile}: forbidden proof or evaluation escape hatch`);
  requireThat(!/@\[\s*extern\b/.test(code), `${codeFile}: external implementation attribute`);
  requireThat(!/^\s*axiom\s/m.test(code), 'new model declares an axiom');
  const names = inventory.theorems.map(row => row.name);
  requireThat(names.length > 0 && new Set(names).size === names.length, 'empty/duplicate theorem inventory');
  let namespace = '';
  const declared = [];
  for (const line of code.split('\n')) {
    const opened = line.match(/^namespace\s+(\S+)/);
    if (opened) namespace = opened[1];
    const theorem = line.match(/^\s*(?:@\[[^\]]*\]\s*)?(?:theorem|lemma)\s+(\S+)/);
    if (theorem) declared.push(`${namespace}.${theorem[1]}`);
  }
  requireThat(names.every(name => name.startsWith('HostLocality.')), 'unrelated theorem in inventory');
  requireThat(names.length === declared.length && names.every(name => declared.includes(name)),
    `declaration/inventory mismatch: declared=${declared.join(',')} inventoried=${names.join(',')}`);
  return names;
}
function prepareProject(profile, inventory) {
  requireThat(profile.schema === 'noble-host-locality-model-inputs/v1', 'wrong model-input schema');
  requireThat(profile.source === 'verification/m5/discovery.tar.gz' &&
    profile.historical_extraction_receipt === 'verification/m5/extraction.json', 'unselected M5 source packet');
  requireThat(JSON.stringify(Object.keys(profile.members).sort()) === JSON.stringify([...generatedMembers].sort()),
    'M5 generated input member set changed');
  requireThat(fileHash(join(root, profile.source)) === profile.archive_sha256, 'M5 source archive changed');
  requireThat(fileHash(join(root, profile.historical_extraction_receipt)) === profile.historical_extraction_receipt_sha256,
    'historical M5 extraction receipt changed');
  const historical = JSON.parse(readFileSync(join(root, profile.historical_extraction_receipt)));
  requireThat(historical.result === 'passed' && historical.source_revision === profile.historical_source_revision,
    'historical extraction result/source revision differs');
  for (const member of generatedMembers) {
    const source = member.replace('./workspace/', '');
    const expected = historical.source_files[source] ?? historical.extractions.kernel.generated[basename(member)];
    requireThat(expected === profile.members[member].sha256, `model input detached from historical extraction: ${source}`);
  }
  const currentResourceFiles = sourceFiles('crates/noble-kernel/src/resources');
  requireThat(JSON.stringify(currentResourceFiles.sort()) ===
    JSON.stringify(Object.keys(profile.historical_resource_source_sha256).sort()), 'resource source scope changed');
  for (const [file, expected] of Object.entries(profile.historical_resource_source_sha256)) {
    requireThat(historical.source_files[file] === expected, `resource binding detached from historical extraction: ${file}`);
    requireThat(fileHash(join(root, file)) === expected, `resource decision bytes differ from retained M5 extraction: ${file}`);
  }
  const extracted = join(output, 'extracted');
  mkdirSync(extracted);
  run(tar, ['-xzf', join(root, profile.source), '-C', extracted, '--', ...generatedMembers]);
  for (const member of generatedMembers) {
    const file = join(extracted, member.slice(2));
    const bytes = readFileSync(file);
    requireThat(bytes.length === profile.members[member].bytes && digest(bytes) === profile.members[member].sha256,
      `archived generated source differs: ${member}`);
  }
  requireThat(fileHash(join(root, 'proofs/m5/M5Resources.lean')) ===
    profile.members['./workspace/proofs/m5/M5Resources.lean'].sha256, 'M5 proof changed independently of retained extraction');
  const project = join(output, 'project');
  mkdirSync(project);
  mkdirSync(join(project, 'NobleKernel'));
  for (const member of generatedMembers.filter(file => file.includes('/m3/'))) {
    const name = member.split('/m3/')[1];
    writeFileSync(join(project, name), readFileSync(join(extracted, member.slice(2))));
  }
  writeFileSync(join(project, 'M5Resources.lean'), readFileSync(join(root, 'proofs/m5/M5Resources.lean')));
  writeFileSync(join(project, 'HostLocality.lean'), readFileSync(join(root, codeFile)));
  writeFileSync(join(project, 'lean-toolchain'), readFileSync(join(root, 'proofs/m5/lean-toolchain')));
  const manifest = JSON.parse(readFileSync(join(root, 'proofs/m5/lake-manifest.json')));
  const dependencyRoot = join(root, 'proofs/foundation/.lake/packages');
  const packages = {};
  for (const pkg of manifest.packages) {
    const directory = join(dependencyRoot, pkg.name);
    const head = run(git, ['rev-parse', 'HEAD'], directory).trim();
    const status = run(git, ['status', '--porcelain=v1', '--untracked-files=all'], directory).trim();
    requireThat(head === pkg.rev && status === '', `dependency ${pkg.name} differs from selected manifest`);
    packages[pkg.name] = { revision: head, tree: run(git, ['rev-parse', 'HEAD^{tree}'], directory).trim() };
  }
  requireThat(packages.aeneas.revision === profile.aeneas_revision &&
    packages.mathlib.revision === profile.mathlib_revision, 'dependency revision differs from model-input profile');
  manifest.name = 'noble_host_locality';
  writeFileSync(join(project, 'lake-manifest.json'), json(manifest));
  writeFileSync(join(project, 'lakefile.toml'), [
    'name = "noble_host_locality"', 'defaultTargets = ["HostLocality"]', '',
    '[[require]]', 'name = "aeneas"',
    'git = { url = "https://github.com/AeneasVerif/aeneas", subDir = "backends/lean" }',
    `rev = "${profile.aeneas_revision}"`, '',
    ...['NobleKernel', 'M5Resources', 'HostLocality'].flatMap(name => ['[[lean_lib]]', `name = "${name}"`, '']),
  ].join('\n'));
  mkdirSync(join(project, '.lake'));
  symlinkSync(dependencyRoot, join(project, '.lake/packages'), 'dir');
  return { project, packages };
}
function auditTheorems(project, inventory, names) {
  const auditFile = join(output, 'Audit.lean');
  const lines = ['import HostLocality', ''];
  const positions = {};
  for (const name of [...names, ...reusedNames]) {
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
  const standard = new Set(['propext', 'Classical.choice', 'Quot.sound']);
  requireThat(inventory.theorems.every(row => Array.isArray(row.allowed_standard_axioms) &&
    row.allowed_standard_axioms.every(name => standard.has(name))),
    'inventory widened the standard axiom set');
  return [...names, ...reusedNames].map(name => {
    const row = records[name];
    const metadata = inventory.theorems.find(row => row.name === name);
    const allowed = new Set(metadata?.allowed_standard_axioms ?? standard);
    requireThat(row?.statement && row?.axioms, `missing compiled statement/axioms: ${name}`);
    const match = row.axioms.match(/depends on axioms: \[([\s\S]*)\]/);
    const axioms = /does not depend on any axioms/.test(row.axioms) ? [] :
      (requireThat(match, `unparsable axiom report: ${name}`), match[1].split(',').map(value => value.trim()).filter(Boolean));
    requireThat(axioms.every(axiom => allowed.has(axiom)), `disallowed transitive axiom: ${name}: ${axioms.join(', ')}`);
    return { ...(metadata ?? { name, role: 'reused M5 table theorem' }),
      status: 'proved', statement: row.statement.replace(/\s+/g, ' '), axioms };
  });
}

try {
  const argv = process.argv.slice(2);
  const allowDirty = argv.includes('--allow-dirty');
  if (allowDirty) argv.splice(argv.indexOf('--allow-dirty'), 1);
  requireThat(argv.length === 2 && argv[0] === '--output',
    'usage: SELECTED_NODE verification/host-locality/check.mjs [--allow-dirty] --output NEW_EXTERNAL_DIRECTORY');
  requireThat(realpathSync(process.execPath) === realpathSync(node), `use selected Node: ${node}`);
  const forbidden = Object.keys(process.env).filter(key =>
    /^(?:LEAN(?:_|$)|LAKE(?:_|$)|NODE(?:_|$)|BASH(?:_|$)|ENV$|LD_|DYLD_|GIT_|NIX_|ELAN)/.test(key) && process.env[key] !== '');
  requireThat(forbidden.length === 0, `unsafe caller environment: ${forbidden.join(', ')}`);
  output = resolve(argv[1]);
  requireThat(output !== root && !output.startsWith(`${root}/`), 'output must be external');
  mkdirSync(output); mkdirSync(join(output, 'commands'));
  const initial = gitState();
  requireThat(allowDirty || initial.status === '', `source worktree dirty: ${initial.status}`);
  const profile = JSON.parse(readFileSync(join(root, profileFile)));
  const inventory = JSON.parse(readFileSync(join(root, inventoryFile)));
  const names = checkDeclarations(inventory);
  const sources = checkedSources(profile);
  const version = run(join(leanBin, 'lean'), ['--version']).trim();
  requireThat(version.includes('version 4.31.0') && version.includes('68218e876d2a38b1985b8590fff244a83c321783'),
    'wrong pinned Lean executable');
  const { project, packages } = prepareProject(profile, inventory);
  const build = run(join(leanBin, 'lake'), ['build', 'HostLocality'], project);
  const generatedWarnings = build.split('\n').filter(line => line.startsWith('warning:'));
  requireThat(generatedWarnings.every(line => line.startsWith('warning: NobleKernel/Funs.lean:') &&
    line.includes('mathlib naming conventions')), `unexpected Lean build diagnostics: ${build}`);
  const theorems = auditTheorems(project, inventory, names);
  requireThat(JSON.stringify(gitState()) === JSON.stringify(initial), 'source tree changed during proof check');
  for (const [file, hash] of Object.entries(sources)) requireThat(fileHash(join(root, file)) === hash, `source changed: ${file}`);
  const receipt = {
    schema: 'noble-host-locality-proof/v1', source_commit: initial.source_commit, source_tree: initial.source_tree,
    clean_source: initial.status === '', allow_dirty: allowDirty, lean: version,
    lean_path: join(leanBin, 'lean'), lean_sha256: fileHash(join(leanBin, 'lean')),
    model_inputs: profile, dependencies: packages, sources_sha256: sources,
    artifacts_sha256: { 'Audit.lean': fileHash(join(output, 'Audit.lean')), ...commandArtifacts },
    generated_source_lint_warnings: generatedWarnings,
    theorem_count: names.length, reused_m5_theorem_count: reusedNames.length, theorems,
    implementation_theorem: false,
    scope: 'Conditional bounded synchronous adapter/guest model and reused extracted M5 decision lemmas; not Rust host, lowering, Wasmtime or whole-engine refinement.',
    assumptions: ['Pinned Lean and imported Aeneas/Mathlib dependency libraries are trusted; downloaded dependency .oleans are not rebuilt by this audit.',
      'The retained generated M5 sources are compiled anew, not replaced with a handwritten table; all six resource decision source files must retain their extraction-recorded bytes.',
      'Current CLI host wrapper, guest lowering and Wasmtime/Canonical ABI correspondence remain separate explicit obligations.'],
    artifact_directory: output, command_count: commandSerial,
  };
  writeFileSync(join(output, 'proof.json'), json(receipt));
  console.log(json({ receipt: join(output, 'proof.json'), source_commit: initial.source_commit,
    clean_source: receipt.clean_source, theorems: names.length, reused_m5_theorems: reusedNames.length }));
} catch (error) {
  console.error(`[host-locality proof] FAIL ${error.stack ?? error}`);
  process.exitCode = 1;
}
