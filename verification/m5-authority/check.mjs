// Lean-kernel check for issue #66 task 3: implementation theorems over the retained M5 authority
// translation. Rebuild the archived generated sources (never archived .olean files), audit every
// inventoried theorem's compiled statement and transitive axioms, bind each Rust source file whose
// generated declarations are reachable from those theorems to its extraction-recorded bytes, and run
// structure-preserving mutation controls against the generated code.
import { spawnSync } from 'node:child_process';
import { mkdirSync, readFileSync, realpathSync, symlinkSync, writeFileSync } from 'node:fs';
import { userInfo } from 'node:os';
import { basename, dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { digest, fileHash } from '../foundation/execution.mjs';

const root = resolve(fileURLToPath(import.meta.url), '../../..');
const selection = JSON.parse(readFileSync(join(root, 'policy/tool-selection.json')));
const node = join(selection.tool_paths.node.output, 'bin/node');
const leanBin = join(selection.tool_paths.lean.output, 'bin');
const lake = join(leanBin, 'lake');
const git = realpathSync('/run/current-system/sw/bin/git');
const tar = realpathSync('/run/current-system/sw/bin/tar');
const { homedir: home, username } = userInfo();
const childEnv = {
  HOME: home, USER: username, LOGNAME: username, LANG: 'C.UTF-8', CI: '1', GIT_TERMINAL_PROMPT: '0',
  PATH: [leanBin, dirname(git), '/run/current-system/sw/bin', '/usr/bin', '/bin'].join(':'),
};
const requireThat = (condition, message) => { if (!condition) throw Error(message); };
const json = value => JSON.stringify(value, null, 2) + '\n';
const codeFile = 'proofs/m5/M5Authority.lean';
const inventoryFile = 'proofs/m5/authority-inventory.json';
const profileFile = 'verification/m5-authority/model-inputs.json';
const generatedMembers = [
  './workspace/proofs/m3/NobleKernel.lean',
  ...['Types', 'TypesExternal', 'Funs', 'FunsExternal'].map(name =>
    `./workspace/proofs/m3/NobleKernel/${name}.lean`),
];
const generatedFiles = ['Types', 'TypesExternal', 'Funs', 'FunsExternal'].map(name => `NobleKernel/${name}.lean`);
const standardAxioms = new Set(['propext', 'Classical.choice', 'Quot.sound']);
let commandSerial = 0;
const commandArtifacts = {};
let output;

function execute(command, argv, cwd, expect) {
  const observed = spawnSync(command, argv, {
    cwd, env: { ...childEnv, TMPDIR: output }, timeout: 3_600_000,
    maxBuffer: 256 * 1024 * 1024, encoding: null,
  });
  const directory = join(output, 'commands', String(++commandSerial).padStart(3, '0'));
  mkdirSync(directory);
  const stdout = observed.stdout ?? Buffer.alloc(0);
  const stderr = observed.stderr ?? Buffer.alloc(0);
  writeFileSync(join(directory, 'stdout'), stdout);
  writeFileSync(join(directory, 'stderr'), stderr);
  const record = {
    command, executable_sha256: fileHash(command), argv, cwd, expect, exit: observed.status,
    signal: observed.signal, error: observed.error?.message ?? null,
    stdout_sha256: digest(stdout), stderr_sha256: digest(stderr),
  };
  const commandJson = json(record);
  writeFileSync(join(directory, 'command.json'), commandJson);
  const prefix = `commands/${basename(directory)}`;
  commandArtifacts[`${prefix}/stdout`] = record.stdout_sha256;
  commandArtifacts[`${prefix}/stderr`] = record.stderr_sha256;
  commandArtifacts[`${prefix}/command.json`] = digest(Buffer.from(commandJson));
  requireThat(!observed.error && observed.signal === null,
    `${command} ${argv.join(' ')} did not complete: ${observed.error?.message ?? observed.signal}`);
  const succeeded = observed.status === 0;
  requireThat(succeeded === (expect === 'success'),
    `${command} ${argv.join(' ')} exited ${observed.status}, expected ${expect}\n${stderr}\n${stdout}`);
  return stdout.toString('utf8');
}
const run = (command, argv, cwd = root) => execute(command, argv, cwd, 'success');

function gitState() {
  return Object.fromEntries([
    ['source_commit', ['rev-parse', 'HEAD']], ['source_tree', ['rev-parse', 'HEAD^{tree}']],
    ['status', ['status', '--porcelain=v1', '--untracked-files=all']],
  ].map(([name, argv]) => [name, run(git, argv).trim()]));
}

/** Top-level declarations of the module with their 1-based start lines. */
function declarationStarts(code) {
  return code.split('\n').flatMap((line, index) => {
    const match = line.match(/^(?:@\[[^\]]*\]\s*)?(?:theorem|lemma|def|inductive|macro)\s+"?([A-Za-z_0-9.]+)/);
    return match ? [{ name: `M5Authority.${match[1]}`, line: index + 1 }] : [];
  });
}

function checkDeclarations(inventory) {
  const text = readFileSync(join(root, codeFile), 'utf8');
  const code = text.replace(/\/-[\s\S]*?-\//g, '').replace(/--.*$/gm, '');
  // Generated function names such as `admission.Authority.admit` are qualified; only a standalone
  // token is a proof or evaluation escape hatch.
  requireThat(!/(?<![.\w'])(?:sorry|admit|native_decide|implemented_by|unsafe|opaque|partial)\b/.test(code),
    `${codeFile}: forbidden proof or evaluation escape hatch`);
  requireThat(!/@\[\s*extern\b/.test(code), `${codeFile}: external implementation attribute`);
  requireThat(!/^\s*axiom\s/m.test(code), 'authority module declares an axiom');
  requireThat(inventory.schema === 'noble-m5-authority-theorem-inventory/v2' &&
    inventory.module === codeFile && inventory.namespace === 'M5Authority', 'wrong inventory header');
  const names = inventory.theorems.map(row => row.name);
  requireThat(names.length > 0 && new Set(names).size === names.length, 'empty/duplicate theorem inventory');
  const declared = [...text.matchAll(/^(?:@\[[^\]]*\]\s*)?(?:theorem|lemma)\s+(\S+)/gm)]
    .map(match => `M5Authority.${match[1]}`);
  requireThat(names.every(name => name.startsWith('M5Authority.')), 'unrelated theorem in inventory');
  requireThat(JSON.stringify(names) === JSON.stringify(declared),
    `declaration/inventory mismatch: declared=${declared.join(',')} inventoried=${names.join(',')}`);
  requireThat(inventory.theorems.every(row => typeof row.role === 'string' && row.role &&
    typeof row.claim === 'string' && row.claim && Array.isArray(row.allowed_standard_axioms) &&
    row.allowed_standard_axioms.every(name => standardAxioms.has(name)) &&
    typeof row.statement === 'string' && Array.isArray(row.subjects)), 'incomplete inventory row');
  // Every hand-written definition a statement can mention is pinned: a definition that changes
  // meaning would otherwise leave every theorem statement textually unchanged.
  const specifications = [...code.matchAll(/^(?:@\[[^\]]*\]\s*)?(?:noncomputable\s+)?(?:def|abbrev|inductive|structure)\s+(\S+)/gm)]
    .map(match => `M5Authority.${match[1]}`);
  requireThat(Array.isArray(inventory.specification) &&
    JSON.stringify(inventory.specification.map(row => row.name)) === JSON.stringify(specifications) &&
    inventory.specification.every(row => typeof row.printed === 'string'),
    `specification/inventory mismatch: declared=${specifications.join(',')}`);
  return { names, specifications, text };
}

function verifyProfile(profile) {
  requireThat(profile.schema === 'noble-m5-authority-model-inputs/v1', 'wrong model-input schema');
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
    requireThat(expected === profile.members[member].sha256, `model input detached from historical extraction: ${member}`);
  }
  requireThat(Array.isArray(profile.mutation_controls) && profile.mutation_controls.length > 0,
    'mutation controls required');
  return historical;
}

function extractGenerated(profile) {
  const extracted = join(output, 'extracted');
  mkdirSync(extracted);
  run(tar, ['-xzf', join(root, profile.source), '-C', extracted, '--', ...generatedMembers]);
  const sources = {};
  for (const member of generatedMembers) {
    const bytes = readFileSync(join(extracted, member.slice(2)));
    requireThat(bytes.length === profile.members[member].bytes && digest(bytes) === profile.members[member].sha256,
      `archived generated source differs: ${member}`);
    sources[member.split('/m3/')[1]] = bytes;
  }
  return sources;
}

function dependencyPackages(profile) {
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
  return { manifest, dependencyRoot, packages };
}

function makeProject(directory, generated, moduleText, dependencies, profile) {
  const project = join(directory, 'project');
  mkdirSync(join(project, 'NobleKernel'), { recursive: true });
  for (const [name, bytes] of Object.entries(generated)) writeFileSync(join(project, name), bytes);
  writeFileSync(join(project, 'M5Authority.lean'), moduleText);
  writeFileSync(join(project, 'lean-toolchain'), readFileSync(join(root, 'proofs/m5/lean-toolchain')));
  writeFileSync(join(project, 'lake-manifest.json'), json({ ...dependencies.manifest, name: 'noble_m5_authority' }));
  writeFileSync(join(project, 'lakefile.toml'), [
    'name = "noble_m5_authority"', 'defaultTargets = ["M5Authority"]', '',
    '[[require]]', 'name = "aeneas"',
    'git = { url = "https://github.com/AeneasVerif/aeneas", subDir = "backends/lean" }',
    `rev = "${profile.aeneas_revision}"`, '',
    ...['NobleKernel', 'M5Authority'].flatMap(name => ['[[lean_lib]]', `name = "${name}"`, '']),
  ].join('\n'));
  mkdirSync(join(project, '.lake'));
  symlinkSync(dependencies.dependencyRoot, join(project, '.lake/packages'), 'dir');
  return project;
}

function messages(text) {
  return text.split('\n').filter(Boolean).map(line => JSON.parse(line));
}

function axiomsOf(data, name) {
  if (/does not depend on any axioms/.test(data)) return [];
  const match = data.match(/depends on axioms: \[([\s\S]*)\]/);
  requireThat(match, `unparsable axiom report: ${name}`);
  return match[1].split(',').map(value => value.trim()).filter(Boolean);
}

/** Statements, axioms, subject functions and the reachable generated declarations of the
 *  inventoried theorems, and the printed form of every specification definition. */
function auditTheorems(project, inventory, names, specifications) {
  const lines = ['import M5Authority', 'open Lean Elab Command', ''];
  const positions = {};
  for (const name of names) {
    positions[lines.length + 1] = { name, kind: 'statement' }; lines.push(`#check @${name}`);
    positions[lines.length + 1] = { name, kind: 'axioms' }; lines.push(`#print axioms ${name}`);
  }
  for (const name of specifications) {
    positions[lines.length + 1] = { name, kind: 'printed' }; lines.push(`#print ${name}`);
  }
  const roots = `  let roots : Array Name := #[${names.map(name => `\`${name}`).join(', ')}]`;
  const reachLine = lines.length + 1;
  lines.push(
    '#eval show CommandElabM Unit from do',
    '  let env ← getEnv',
    roots,
    '  let mut seen : NameSet := {}',
    '  let mut stack := roots',
    '  let mut reached : Array String := #[]',
    '  while !stack.isEmpty do',
    '    let current := stack.back!',
    '    stack := stack.pop',
    '    unless seen.contains current do',
    '      seen := seen.insert current',
    '      if let some info := env.find? current then',
    '        if current.toString.startsWith "noble_kernel." then reached := reached.push current.toString',
    '        let mut used := info.type.getUsedConstants',
    '        if let some value := info.value? (allowOpaque := true) then used := used ++ value.getUsedConstants',
    '        if let .inductInfo kind := info then used := used ++ kind.ctors.toArray',
    '        for dependency in used do',
    '          let text := dependency.toString',
    '          if (text.startsWith "noble_kernel." || text.startsWith "M5Authority.") && !seen.contains dependency then',
    '            stack := stack.push dependency',
    '  logInfo m!"M5-AUTHORITY-REACHABLE {(Json.arr ((reached.qsort (fun a b => decide (a < b))).map Json.str)).compress}"',
  );
  // Subject functions: the generated functions a statement is about. Follow the statement through
  // this module's definitions and constructors, but neither into proofs nor into generated bodies.
  const subjectLine = lines.length + 1;
  lines.push(
    '#eval show CommandElabM Unit from do',
    '  let env ← getEnv',
    roots,
    '  let generated : List Name := [`NobleKernel.Funs, `NobleKernel.FunsExternal]',
    '  let mut rows : Array Json := #[]',
    '  for root in roots do',
    '    let some rootInfo := env.find? root | throwError "missing {root}"',
    '    let mut seen : NameSet := {}',
    '    let mut stack : Array Name := rootInfo.type.getUsedConstants',
    '    let mut subjects : Array String := #[]',
    '    while !stack.isEmpty do',
    '      let current := stack.back!',
    '      stack := stack.pop',
    '      unless seen.contains current do',
    '        seen := seen.insert current',
    '        if let some info := env.find? current then',
    '          let text := current.toString',
    '          let owner := match env.getModuleIdxFor? current with',
    '            | some index => env.header.moduleNames[index.toNat]?.getD .anonymous',
    '            | none => .anonymous',
    '          if text.startsWith "noble_kernel." then',
    '            if generated.contains owner && (info matches .defnInfo _) then subjects := subjects.push text',
    '          else if text.startsWith "M5Authority." && !(info matches .thmInfo _) then',
    '            let mut used := info.type.getUsedConstants',
    '            if let some value := info.value? (allowOpaque := true) then used := used ++ value.getUsedConstants',
    '            if let .inductInfo kind := info then used := used ++ kind.ctors.toArray',
    '            for dependency in used do',
    '              unless seen.contains dependency do stack := stack.push dependency',
    '    rows := rows.push (Json.mkObj [("theorem", Json.str root.toString),',
    '      ("subjects", Json.arr ((subjects.qsort (fun a b => decide (a < b))).map Json.str))])',
    '  logInfo m!"M5-AUTHORITY-SUBJECTS {(Json.arr rows).compress}"',
  );
  const auditFile = join(output, 'Audit.lean');
  writeFileSync(auditFile, lines.join('\n') + '\n');
  const records = {};
  let reachable = null;
  let subjectRows = null;
  for (const message of messages(run(lake, ['env', 'lean', '--json', auditFile], project))) {
    requireThat(message.severity === 'information', `Lean audit diagnostic: ${JSON.stringify(message)}`);
    if (message.pos.line === reachLine) {
      const match = message.data.match(/^M5-AUTHORITY-REACHABLE (\[.*\])$/s);
      requireThat(match && reachable === null, 'unexpected reachability report');
      reachable = JSON.parse(match[1]);
      continue;
    }
    if (message.pos.line === subjectLine) {
      const match = message.data.match(/^M5-AUTHORITY-SUBJECTS (\[.*\])$/s);
      requireThat(match && subjectRows === null, 'unexpected subject report');
      subjectRows = JSON.parse(match[1]);
      continue;
    }
    const slot = positions[message.pos.line];
    requireThat(slot, `unexpected Lean audit output at ${message.pos.line}`);
    (records[slot.name] ??= {})[slot.kind] = message.data.trim();
  }
  requireThat(Array.isArray(reachable) && reachable.length > 0, 'missing reachability report');
  requireThat(Array.isArray(subjectRows) &&
    JSON.stringify(subjectRows.map(row => row.theorem)) === JSON.stringify(names), 'missing subject report');
  const subjectsOf = Object.fromEntries(subjectRows.map(row => [row.theorem, row.subjects]));
  const normalize = text => text.replace(/\s+/g, ' ');
  const theorems = names.map(name => {
    const row = records[name];
    const metadata = inventory.theorems.find(entry => entry.name === name);
    requireThat(row?.statement && row?.axioms, `missing compiled statement/axioms: ${name}`);
    const axioms = axiomsOf(row.axioms, name);
    const allowed = new Set(metadata.allowed_standard_axioms);
    requireThat(axioms.every(axiom => allowed.has(axiom)), `disallowed transitive axiom: ${name}: ${axioms.join(', ')}`);
    return { ...metadata, status: 'proved', statement: normalize(row.statement),
      subjects: subjectsOf[name], axioms };
  });
  const specification = specifications.map(name => {
    requireThat(records[name]?.printed, `missing printed specification: ${name}`);
    return { name, printed: normalize(records[name].printed) };
  });
  // Pins are reviewed expectations: a changed statement, subject set or specification fails the run
  // and leaves the observed values for an explicit renewal.
  const mismatches = [
    ...theorems.flatMap(theorem => {
      const pinned = inventory.theorems.find(entry => entry.name === theorem.name);
      return [
        ...(pinned.statement === theorem.statement ? [] : [`statement ${theorem.name}`]),
        ...(JSON.stringify(pinned.subjects) === JSON.stringify(theorem.subjects) ? [] : [`subjects ${theorem.name}`]),
      ];
    }),
    ...specification.flatMap((row, index) =>
      inventory.specification[index].printed === row.printed ? [] : [`specification ${row.name}`]),
  ];
  if (mismatches.length > 0) {
    writeFileSync(join(output, 'observed-pins.json'), json({
      specification,
      theorems: theorems.map(({ name, statement, subjects }) => ({ name, statement, subjects })),
    }));
  }
  requireThat(mismatches.length === 0,
    `inventory pins differ from the compiled module; renewal is explicit (observed-pins.json): ${mismatches.join('; ')}`);
  return { theorems, specification, reachable };
}

/** Elaborator-shared auxiliaries (matchers, side proofs, equation lemmas) are reused across
 *  declarations with identical shape. Their meaning is fixed by their own type, which the reachability
 *  pass still traverses, so they bind no Rust item of the declaration that first introduced them. */
const auxiliary = /^(?:match_\d+|_proof_\d+|proof_\d+|_auxLemma\.\d+|eq_\d+|eq_def|_sunfold|_unary|_mutual|splitter.*)$/;

/** Map each reachable generated declaration to the Rust span its generated header names, and require
 *  that Rust text to be unchanged: the whole file when its bytes still match the extraction record,
 *  otherwise the exact text of every reachable item, read from the archived extraction input. */
function bindReachable(generated, reachable, historical, profile) {
  const declared = new Map();
  for (const file of generatedFiles) {
    const lines = generated[file].toString('utf8').split('\n');
    let pending = null;
    for (let index = 0; index < lines.length; index++) {
      const source = lines[index].match(/Source: '([^']+)', lines (\d+):\d+-(\d+):\d+/);
      if (source) pending = { file: source[1], first: Number(source[2]), last: Number(source[3]) };
      const header = lines[index].match(
        /^(?:@\[[^\]]*\]\s*)?(?:noncomputable\s+)?(?:def|structure|inductive|abbrev|impl_def|instance)(?:\s+(\S+))?\s*$|^(?:@\[[^\]]*\]\s*)?(?:noncomputable\s+)?(?:def|structure|inductive|abbrev|impl_def|instance)\s+(\S+)/);
      if (!header) continue;
      const name = header[1] ?? header[2] ?? lines[index + 1]?.trim().split(/\s+/)[0];
      requireThat(name, `unparsed generated declaration header: ${file}:${index + 1}`);
      declared.set(name, { file, source: pending });
      pending = null;
    }
  }
  const generatedDeclarations = new Map();
  const externalModels = new Set();
  const unresolved = [];
  let auxiliaries = 0;
  for (const full of reachable) {
    const parts = full.replace(/^noble_kernel\./, '').split('.');
    if (auxiliary.test(parts.at(-1))) { auxiliaries++; continue; }
    let owner = null;
    for (let length = parts.length; length > 0 && owner === null; length--) {
      const candidate = parts.slice(0, length).join('.');
      if (declared.has(candidate)) owner = [candidate, declared.get(candidate)];
    }
    if (owner === null) { unresolved.push(full); continue; }
    const [name, { file, source }] = owner;
    if (file.includes('External')) externalModels.add(`${file}:${name}`);
    else {
      requireThat(source, `generated declaration without a Rust source: ${name}`);
      generatedDeclarations.set(name, source);
    }
  }
  requireThat(unresolved.length === 0, `reachable declarations outside the generated sources: ${unresolved.join(', ')}`);
  const files = [...new Set([...generatedDeclarations.values()].map(source => source.file))].sort();
  const archived = join(output, 'archived-sources');
  mkdirSync(archived);
  run(tar, ['-xzf', join(root, profile.source), '-C', archived, '--', ...files.map(file => `./workspace/${file}`)]);
  const bound = files.map(file => {
    const recorded = historical.source_files[file];
    requireThat(recorded, `reachable source not recorded by the extraction: ${file}`);
    const original = readFileSync(join(archived, 'workspace', file));
    requireThat(digest(original) === recorded, `archived source differs from its extraction record: ${file}`);
    const current = readFileSync(join(root, file));
    const declarations = [...generatedDeclarations].filter(([, source]) => source.file === file);
    if (digest(current) === recorded) {
      return { file, binding: 'exact-file', sha256: recorded, declarations: declarations.length };
    }
    const originalLines = original.toString('utf8').split('\n');
    const currentText = current.toString('utf8');
    const items = declarations.map(([name, { first, last }]) => {
      // Extend an attribute-only span (for example a derive) through the item it decorates.
      let end = last;
      while (end < originalLines.length &&
        !/\b(?:struct|enum|type|fn|impl|const|static|trait|mod)\b/.test(originalLines.slice(first - 1, end).join('\n'))) end++;
      const text = originalLines.slice(first - 1, end).join('\n');
      requireThat(currentText.includes(text), `reachable Rust item changed since extraction: ${name} (${file}:${first})`);
      return { declaration: name, archived_lines: [first, end], sha256: digest(Buffer.from(text)) };
    });
    return { file, binding: 'item-text', extraction_sha256: recorded, current_sha256: digest(current), items };
  });
  return {
    reachable_generated_declarations: generatedDeclarations.size,
    reachable_project_constants: reachable.length,
    shared_auxiliaries: auxiliaries,
    source_files: bound,
    external_models: [...externalModels].sort(),
  };
}

/** Apply a structure-preserving edit to the generated code, rebuild it, and record which
 *  inventoried theorems lose their strict axioms. */
function mutationControl(control, generated, moduleText, names, dependencies, profile) {
  const directory = join(output, 'mutations', control.id);
  mkdirSync(directory, { recursive: true });
  let funs = generated['NobleKernel/Funs.lean'].toString('utf8');
  for (const edit of control.edits) {
    requireThat(funs.split(edit.find).length === 2, `${control.id}: edit anchor must occur exactly once`);
    funs = funs.replace(edit.find, () => edit.replace);
  }
  const mutated = { ...generated, 'NobleKernel/Funs.lean': Buffer.from(funs) };
  const project = makeProject(directory, mutated, moduleText, dependencies, profile);
  run(lake, ['build', 'NobleKernel'], project);
  const moduleLines = moduleText.endsWith('\n') ? moduleText.split('\n').length - 1 : moduleText.split('\n').length;
  const auditText = moduleText + (moduleText.endsWith('\n') ? '' : '\n') +
    names.map(name => `#print axioms ${name}`).join('\n') + '\n';
  writeFileSync(join(project, 'MutationAudit.lean'), auditText);
  const starts = declarationStarts(moduleText);
  const errorDeclarations = new Set();
  const axioms = {};
  for (const message of messages(execute(lake, ['env', 'lean', '--json', 'MutationAudit.lean'], project, 'failure'))) {
    const line = message.pos.line;
    if (line > moduleLines) {
      const name = names[line - moduleLines - 1];
      requireThat(name && message.severity === 'information', `unexpected mutation audit output at ${line}`);
      axioms[name] = axiomsOf(message.data, name);
    } else if (message.severity === 'error') {
      const owner = starts.filter(start => start.line <= line).at(-1);
      errorDeclarations.add(owner?.name ?? `line ${line}`);
    }
  }
  requireThat(names.every(name => axioms[name]), `${control.id}: missing axiom reports`);
  const lost = names.filter(name => axioms[name].includes('sorryAx'));
  const kept = names.filter(name => axioms[name].every(axiom => standardAxioms.has(axiom)));
  requireThat(errorDeclarations.size > 0, `${control.id}: the mutated program still proves every theorem`);
  requireThat(control.must_lose_strict_axioms.every(name => lost.includes(name)),
    `${control.id}: a targeted theorem kept strict axioms`);
  requireThat(kept.length > 0 && lost.length + kept.length === names.length,
    `${control.id}: attribution is not localized`);
  return {
    id: control.id, removes: control.removes, edits: control.edits.length,
    mutated_funs_sha256: digest(Buffer.from(funs)), audit_sha256: digest(Buffer.from(auditText)),
    failing_declarations: [...errorDeclarations].sort(), lost_strict_axioms: lost,
    kept_strict_axioms: kept.length, targeted: control.must_lose_strict_axioms,
  };
}

try {
  const argv = process.argv.slice(2);
  const allowDirty = argv.includes('--allow-dirty');
  if (allowDirty) argv.splice(argv.indexOf('--allow-dirty'), 1);
  requireThat(argv.length === 2 && argv[0] === '--output',
    'usage: SELECTED_NODE verification/m5-authority/check.mjs [--allow-dirty] --output NEW_EXTERNAL_DIRECTORY');
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
  const { names, specifications, text: moduleText } = checkDeclarations(inventory);
  const historical = verifyProfile(profile);
  const checkedFiles = [codeFile, inventoryFile, profileFile, 'verification/m5-authority/check.mjs',
    'proofs/m5/lake-manifest.json', 'proofs/m5/lean-toolchain', profile.source,
    profile.historical_extraction_receipt, 'policy/tool-selection.json'];
  const sources = Object.fromEntries(checkedFiles.map(file => [file, fileHash(join(root, file))]));
  const version = run(join(leanBin, 'lean'), ['--version']).trim();
  requireThat(version.includes('version 4.31.0') && version.includes('68218e876d2a38b1985b8590fff244a83c321783'),
    'wrong pinned Lean executable');
  const generated = extractGenerated(profile);
  const dependencies = dependencyPackages(profile);
  const project = makeProject(join(output, 'baseline'), generated, moduleText, dependencies, profile);
  const build = run(lake, ['build', 'M5Authority'], project);
  const generatedWarnings = build.split('\n').filter(line => line.startsWith('warning:'));
  requireThat(generatedWarnings.every(line => line.startsWith('warning: NobleKernel/Funs.lean:') &&
    line.includes('mathlib naming conventions')), `unexpected Lean build diagnostics: ${build}`);
  const { theorems, specification, reachable } = auditTheorems(project, inventory, names, specifications);
  const binding = bindReachable(generated, reachable, historical, profile);
  const mutations = profile.mutation_controls.map(control =>
    mutationControl(control, generated, moduleText, names, dependencies, profile));
  requireThat(JSON.stringify(gitState()) === JSON.stringify(initial), 'source tree changed during proof check');
  for (const [file, hash] of Object.entries(sources)) requireThat(fileHash(join(root, file)) === hash, `source changed: ${file}`);
  const roleCount = role => theorems.filter(theorem => theorem.role === role).length;
  const receipt = {
    schema: 'noble-m5-authority-proof/v1', issue: 'https://github.com/OnixResearch/noble-lang/issues/66',
    source_commit: initial.source_commit, source_tree: initial.source_tree,
    clean_source: initial.status === '', allow_dirty: allowDirty, lean: version,
    lean_path: join(leanBin, 'lean'), lean_sha256: fileHash(join(leanBin, 'lean')),
    model_inputs: { ...profile, mutation_controls: profile.mutation_controls.map(control => control.id) },
    dependencies: dependencies.packages, sources_sha256: sources,
    artifacts_sha256: { 'Audit.lean': fileHash(join(output, 'Audit.lean')), ...commandArtifacts },
    generated_source_lint_warnings: generatedWarnings,
    theorem_count: names.length, theorems, specification, binding, mutation_controls: mutations,
    claims: {
      universal_implementation_theorems: names.length,
      committed_admission_theorems: roleCount('committed-admission'),
      quota_invariant_theorems: roleCount('quota-invariant'),
      ledger_invariant_theorems: roleCount('ledger-invariant'),
      ledger_move_theorems: roleCount('ledger-move'),
      trace_theorems: roleCount('trace'),
      attempt_start_theorems: roleCount('attempt-start'),
      finite_rust_comparisons: 'none in this lane; the binding and sequencing Rust controls remain crates/noble-kernel/tests/m5-authority',
      correspondence: 'statements name the retained generated functions; Charon/Aeneas translation of the bound sources is trusted, not re-run',
    },
    assumptions: [
      'Pinned Lean and imported Aeneas/Mathlib dependency libraries are trusted; downloaded dependency .oleans are not rebuilt by this audit.',
      'The retained Charon/Aeneas translation is trusted to model the bound Rust sources; this lane does not re-run the translation.',
      'Reachable handwritten external models are part of the trusted base and are listed in binding.external_models.',
      'Checkpoint renewal is a host premise: a fresh checkpoint opens fresh quota records from fresh host snapshots.',
    ],
    non_claims: [
      'No cross-checkpoint quota conservation: quota records are checkpoint-local.',
      'Consumed witnesses and registered attempts agree in number; no trace-level pairing of a particular witness with a particular attempt is stated beyond the per-admission theorems.',
      'No liveness: nothing forces an admitted attempt to start or an invocation to finish.',
      'No concurrency, crash-recovery, remote-revocation atomicity or exactly-once delivery claim.',
      'No Rust compiler, Charon or Aeneas correctness claim.',
    ],
    scope: 'Universal theorems over the retained generated M5 authority transitions: committed-admission guarantees, refusal preservation, one ledger move per public transition, the checkpoint-local quota invariant and the table/counter agreement over every finite serialized prefix from Authority.new, and over every finite trace a settled witness is never rewritten and a started attempt never starts again.',
    artifact_directory: output, command_count: commandSerial,
  };
  writeFileSync(join(output, 'receipt.json'), json(receipt));
  console.log(json({ receipt: join(output, 'receipt.json'), source_commit: initial.source_commit,
    clean_source: receipt.clean_source, theorems: names.length,
    bound_source_files: binding.source_files.length, mutation_controls: mutations.length }));
} catch (error) {
  console.error(`[m5-authority proof] FAIL ${error.stack ?? error}`);
  process.exitCode = 1;
}
