// Lean-kernel check for issue #48: the checked-in Charon/Aeneas translation of the ServiceRef codec
// refines the Lean model. Rebuild the checked-in generated sources and the foundation model
// from source (never retained .olean files), audit every inventoried theorem's compiled statement and
// transitive axioms, bind each Rust source file whose generated declarations are reachable from those
// theorems to the bytes the recorded extraction read, and run structure-preserving mutation controls
// against the generated code.
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
const { homedir: home, username } = userInfo();
const childEnv = {
  HOME: home, USER: username, LOGNAME: username, LANG: 'C.UTF-8', CI: '1', GIT_TERMINAL_PROMPT: '0',
  PATH: [leanBin, dirname(git), '/run/current-system/sw/bin', '/usr/bin', '/bin'].join(':'),
};
const requireThat = (condition, message) => { if (!condition) throw Error(message); };
const json = value => JSON.stringify(value, null, 2) + '\n';
const namespace = 'ServiceWireRefinement';
const codeFile = 'proofs/service-wire/ServiceWireRefinement.lean';
const inventoryFile = 'proofs/service-wire/inventory.json';
const profileFile = 'verification/service-wire-refinement/model-inputs.json';
const projectDirectory = 'proofs/service-wire';
const projectFiles = ['lakefile.toml', 'lake-manifest.json', 'lean-toolchain'].map(name => `${projectDirectory}/${name}`);
const modelFile = 'proofs/foundation/NobleFoundation/ServiceWire.lean';
const generatedInputs = [
  'proofs/m3/NobleKernel.lean',
  ...['Types', 'TypesExternal', 'Funs', 'FunsExternal'].map(name => `proofs/m3/NobleKernel/${name}.lean`),
];
const handwrittenModels = new Set(['proofs/m3/NobleKernel/TypesExternal.lean', 'proofs/m3/NobleKernel/FunsExternal.lean']);
const funsFile = 'proofs/m3/NobleKernel/Funs.lean';
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
    const match = line.match(/^(?:@\[[^\]]*\]\s*)?(?:theorem|lemma|def|abbrev|inductive|structure|macro)\s+"?([A-Za-z_0-9.]+)/);
    return match ? [{ name: `${namespace}.${match[1]}`, line: index + 1 }] : [];
  });
}

function checkDeclarations(inventory) {
  const text = readFileSync(join(root, codeFile), 'utf8');
  const code = text.replace(/\/-[\s\S]*?-\//g, '').replace(/--.*$/gm, '');
  // Generated names are qualified; only a standalone token is a proof or evaluation escape hatch.
  requireThat(!/(?<![.\w'])(?:sorry|admit|native_decide|implemented_by|unsafe|opaque|partial)\b/.test(code),
    `${codeFile}: forbidden proof or evaluation escape hatch`);
  requireThat(!/@\[\s*extern\b/.test(code), `${codeFile}: external implementation attribute`);
  requireThat(!/^\s*axiom\s/m.test(code), 'refinement module declares an axiom');
  requireThat(!/^\s*(?:set_option\s+debug\.|attribute\s*\[\s*-)/m.test(code), 'refinement module weakens the kernel or attributes');
  requireThat(inventory.schema === 'noble-service-wire-refinement-inventory/v1' &&
    inventory.module === codeFile && inventory.namespace === namespace, 'wrong inventory header');
  const names = inventory.theorems.map(row => row.name);
  requireThat(names.length > 0 && new Set(names).size === names.length, 'empty/duplicate theorem inventory');
  const declared = [...text.matchAll(/^(?:@\[[^\]]*\]\s*)?(?:theorem|lemma)\s+(\S+)/gm)]
    .map(match => `${namespace}.${match[1]}`);
  requireThat(names.every(name => name.startsWith(`${namespace}.`)), 'unrelated theorem in inventory');
  requireThat(JSON.stringify(names) === JSON.stringify(declared),
    `declaration/inventory mismatch: declared=${declared.join(',')} inventoried=${names.join(',')}`);
  requireThat(inventory.theorems.every(row => typeof row.role === 'string' && row.role &&
    typeof row.claim === 'string' && row.claim && Array.isArray(row.allowed_standard_axioms) &&
    row.allowed_standard_axioms.every(name => standardAxioms.has(name)) &&
    typeof row.statement === 'string' && Array.isArray(row.subjects)), 'incomplete inventory row');
  // Every hand-written definition a statement can mention is pinned: a definition that changes
  // meaning would otherwise leave every theorem statement textually unchanged.
  const specifications = [...code.matchAll(/^(?:@\[[^\]]*\]\s*)?(?:noncomputable\s+)?(?:def|abbrev|inductive|structure)\s+(\S+)/gm)]
    .map(match => `${namespace}.${match[1]}`);
  requireThat(Array.isArray(inventory.specification) &&
    JSON.stringify(inventory.specification.map(row => row.name)) === JSON.stringify(specifications) &&
    inventory.specification.every(row => typeof row.printed === 'string'),
    `specification/inventory mismatch: declared=${specifications.join(',')}`);
  return { names, specifications, text };
}

/** The generated inputs are the outputs of one recorded extraction; the handwritten external models
 *  are inputs of that same extraction. Two independent records must agree on every byte. */
function verifyProfile(profile) {
  requireThat(profile.schema === 'noble-service-wire-refinement-inputs/v1', 'wrong model-input schema');
  requireThat(profile.extraction_receipt === 'verification/declared-modules-v1/extraction.json' &&
    profile.extraction_lock === 'verification/m4/extraction-lock.json', 'unselected extraction records');
  requireThat(fileHash(join(root, profile.extraction_receipt)) === profile.extraction_receipt_sha256,
    'extraction receipt changed');
  requireThat(fileHash(join(root, profile.extraction_lock)) === profile.extraction_lock_sha256,
    'extraction lock changed');
  const receipt = JSON.parse(readFileSync(join(root, profile.extraction_receipt)));
  const lock = JSON.parse(readFileSync(join(root, profile.extraction_lock)));
  requireThat(receipt.result === 'passed', 'recorded extraction did not pass');
  requireThat(JSON.stringify(Object.keys(profile.generated_inputs).sort()) === JSON.stringify([...generatedInputs].sort()),
    'generated input set changed');
  for (const file of generatedInputs) {
    const recorded = handwrittenModels.has(file)
      ? [receipt.source_files[file], lock.source_files[file]]
      : [receipt.extractions.kernel.generated[basename(file)], lock.generated_files.kernel[basename(file)]];
    requireThat(recorded.every(hash => hash === profile.generated_inputs[file]),
      `generated input detached from the recorded extraction: ${file}`);
    requireThat(fileHash(join(root, file)) === profile.generated_inputs[file], `generated input changed: ${file}`);
  }
  for (const [file, hash] of Object.entries(profile.model)) {
    requireThat(file === modelFile && fileHash(join(root, file)) === hash, `model changed: ${file}`);
  }
  // The project reuses the reviewed M7 toolchain and dependency manifest unchanged.
  const reference = profile.toolchain_reference;
  requireThat(reference === 'proofs/m7', 'unselected toolchain reference');
  requireThat(readFileSync(join(root, projectDirectory, 'lean-toolchain')).equals(readFileSync(join(root, reference, 'lean-toolchain'))),
    'lean-toolchain differs from the reviewed reference');
  const manifest = JSON.parse(readFileSync(join(root, projectDirectory, 'lake-manifest.json')));
  const referenceManifest = JSON.parse(readFileSync(join(root, reference, 'lake-manifest.json')));
  requireThat(manifest.name === 'noble_service_wire_refinement' &&
    JSON.stringify({ ...manifest, name: null }) === JSON.stringify({ ...referenceManifest, name: null }),
    'lake manifest differs from the reviewed reference beyond its package name');
  requireThat(Array.isArray(profile.mutation_controls) && profile.mutation_controls.length > 0,
    'mutation controls required');
  return { receipt, lock, manifest };
}

function dependencyPackages(profile, manifest) {
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
  return { dependencyRoot, packages };
}

/** A fresh copy of exactly the files the project reads, in the checkout's relative layout, so the
 *  committed lakefile's `srcDir` entries resolve unchanged. */
function makeTree(directory, files, dependencies) {
  const tree = join(directory, 'tree');
  for (const [file, bytes] of Object.entries(files)) {
    mkdirSync(dirname(join(tree, file)), { recursive: true });
    writeFileSync(join(tree, file), bytes);
  }
  const project = join(tree, projectDirectory);
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
  const lines = [`import ${namespace}`, 'open Lean Elab Command', ''];
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
    `          if (text.startsWith "noble_kernel." || text.startsWith "${namespace}.") && !seen.contains dependency then`,
    '            stack := stack.push dependency',
    '  logInfo m!"SERVICE-WIRE-REACHABLE {(Json.arr ((reached.qsort (fun a b => decide (a < b))).map Json.str)).compress}"',
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
    `          else if text.startsWith "${namespace}." && !(info matches .thmInfo _) then`,
    '            let mut used := info.type.getUsedConstants',
    '            if let some value := info.value? (allowOpaque := true) then used := used ++ value.getUsedConstants',
    '            if let .inductInfo kind := info then used := used ++ kind.ctors.toArray',
    '            for dependency in used do',
    '              unless seen.contains dependency do stack := stack.push dependency',
    '    rows := rows.push (Json.mkObj [("theorem", Json.str root.toString),',
    '      ("subjects", Json.arr ((subjects.qsort (fun a b => decide (a < b))).map Json.str))])',
    '  logInfo m!"SERVICE-WIRE-SUBJECTS {(Json.arr rows).compress}"',
  );
  const auditFile = join(output, 'Audit.lean');
  writeFileSync(auditFile, lines.join('\n') + '\n');
  const records = {};
  let reachable = null;
  let subjectRows = null;
  for (const message of messages(run(lake, ['env', 'lean', '--json', auditFile], project))) {
    requireThat(message.severity === 'information', `Lean audit diagnostic: ${JSON.stringify(message)}`);
    if (message.pos.line === reachLine) {
      const match = message.data.match(/^SERVICE-WIRE-REACHABLE (\[.*\])$/s);
      requireThat(match && reachable === null, 'unexpected reachability report');
      reachable = JSON.parse(match[1]);
      continue;
    }
    if (message.pos.line === subjectLine) {
      const match = message.data.match(/^SERVICE-WIRE-SUBJECTS (\[.*\])$/s);
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

/** Map each reachable generated declaration to the Rust file its generated header names, and require
 *  that file to hold exactly the bytes the recorded extraction read. There is no archived input for
 *  this extraction, so a changed file fails closed and needs a new extraction. */
function bindReachable(reachable, records) {
  const declared = new Map();
  for (const file of generatedInputs.filter(file => file !== 'proofs/m3/NobleKernel.lean')) {
    const lines = readFileSync(join(root, file), 'utf8').split('\n');
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
    if (handwrittenModels.has(file)) externalModels.add(`${basename(file)}:${name}`);
    else {
      requireThat(source, `generated declaration without a Rust source: ${name}`);
      generatedDeclarations.set(name, source);
    }
  }
  requireThat(unresolved.length === 0, `reachable declarations outside the generated sources: ${unresolved.join(', ')}`);
  const files = [...new Set([...generatedDeclarations.values()].map(source => source.file))].sort();
  const bound = files.map(file => {
    const recorded = [records.receipt.source_files[file], records.lock.source_files[file]];
    requireThat(recorded[0] && recorded[0] === recorded[1], `reachable source not recorded consistently: ${file}`);
    const current = fileHash(join(root, file));
    requireThat(current === recorded[0], `reachable Rust source changed since the recorded extraction: ${file}`);
    const declarations = [...generatedDeclarations].filter(([, source]) => source.file === file)
      .map(([name, { first, last }]) => ({ declaration: name, lines: [first, last] }))
      .sort((left, right) => left.declaration < right.declaration ? -1 : 1);
    return { file, binding: 'exact-file', sha256: current, declarations };
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
function mutationControl(control, files, moduleText, names, dependencies) {
  const directory = join(output, 'mutations', control.id);
  mkdirSync(directory, { recursive: true });
  let funs = files[funsFile].toString('utf8');
  for (const edit of control.edits) {
    requireThat(funs.split(edit.find).length === 2, `${control.id}: edit anchor must occur exactly once`);
    funs = funs.replace(edit.find, () => edit.replace);
  }
  const project = makeTree(directory, { ...files, [funsFile]: Buffer.from(funs) }, dependencies);
  run(lake, ['build', 'NobleKernel', 'ServiceWireModel'], project);
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
    'usage: SELECTED_NODE verification/service-wire-refinement/check.mjs [--allow-dirty] --output NEW_EXTERNAL_DIRECTORY');
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
  const records = verifyProfile(profile);
  const checkedFiles = [codeFile, inventoryFile, profileFile, 'verification/service-wire-refinement/check.mjs',
    ...projectFiles, `${profile.toolchain_reference}/lean-toolchain`, `${profile.toolchain_reference}/lake-manifest.json`,
    modelFile, ...generatedInputs, profile.extraction_receipt, profile.extraction_lock, 'policy/tool-selection.json'];
  const sources = Object.fromEntries(checkedFiles.map(file => [file, fileHash(join(root, file))]));
  const version = run(join(leanBin, 'lean'), ['--version']).trim();
  requireThat(version.includes('version 4.31.0') && version.includes('68218e876d2a38b1985b8590fff244a83c321783'),
    'wrong pinned Lean executable');
  const dependencies = dependencyPackages(profile, records.manifest);
  const files = Object.fromEntries([...projectFiles, codeFile, modelFile, ...generatedInputs]
    .map(file => [file, readFileSync(join(root, file))]));
  const project = makeTree(join(output, 'baseline'), files, dependencies);
  const build = run(lake, ['build', 'ServiceWireRefinement'], project);
  const buildWarnings = build.split('\n').filter(line => line.startsWith('warning:'));
  // Lint warnings in the checked-in generated and external sources are recorded, never the module's
  // or the model's own.
  requireThat(buildWarnings.every(line => /^warning: (?:\.\.\/m3\/)?NobleKernel\/(?:Funs|FunsExternal|Types|TypesExternal)\.lean:/.test(line)),
    `unexpected Lean build diagnostics: ${build}`);
  const { theorems, specification, reachable } = auditTheorems(project, inventory, names, specifications);
  const binding = bindReachable(reachable, records);
  const mutations = profile.mutation_controls.map(control =>
    mutationControl(control, files, moduleText, names, dependencies));
  requireThat(JSON.stringify(gitState()) === JSON.stringify(initial), 'source tree changed during proof check');
  for (const [file, hash] of Object.entries(sources)) requireThat(fileHash(join(root, file)) === hash, `source changed: ${file}`);
  const roleCount = role => theorems.filter(theorem => theorem.role === role).length;
  const receipt = {
    schema: 'noble-service-wire-refinement-proof/v1', issue: 'https://github.com/OnixResearch/noble-lang/issues/48',
    source_commit: initial.source_commit, source_tree: initial.source_tree,
    clean_source: initial.status === '', allow_dirty: allowDirty, lean: version,
    lean_path: join(leanBin, 'lean'), lean_sha256: fileHash(join(leanBin, 'lean')),
    model_inputs: { ...profile, mutation_controls: profile.mutation_controls.map(control => control.id) },
    dependencies: dependencies.packages, sources_sha256: sources,
    artifacts_sha256: { 'Audit.lean': fileHash(join(output, 'Audit.lean')), ...commandArtifacts },
    generated_source_lint_warnings: buildWarnings,
    theorem_count: names.length, theorems, specification, binding, mutation_controls: mutations,
    claims: {
      theorems_by_role: Object.fromEntries([...new Set(theorems.map(theorem => theorem.role))].map(role => [role, roleCount(role)])),
      refinements: [`${namespace}.decode_service_refines`, `${namespace}.encode_service_refines`],
      rust_round_trips: [`${namespace}.decode_service_encode_service`, `${namespace}.encode_service_decode_service`],
      implementation_subjects: [...new Set(theorems.flatMap(theorem => theorem.subjects))].sort(),
      finite_rust_comparisons: 'none in this lane; verification/service-ref/differential.json remains the finite Rust-level comparison',
      correspondence: 'statements name the checked-in generated functions; the recorded Charon/Aeneas translation of the bound Rust sources is trusted, not re-run',
    },
    assumptions: [
      'Pinned Lean and the imported Aeneas/Mathlib dependency libraries are trusted; downloaded dependency .oleans are not rebuilt by this audit.',
      'The recorded Charon/Aeneas translation (verification/declared-modules-v1/extraction.json) is trusted to model the bound Rust sources; this lane does not re-run it.',
      'Reachable handwritten external models are part of the trusted base and are listed in binding.external_models; core.str.converts.from_utf8 is modelled as returning its input on valid UTF-8 and Utf8Error.Invalid otherwise.',
      'The Aeneas library models of slices, arrays, range indexing and range writes, slice copies, scalars, checked arithmetic and loops are trusted.',
    ],
    non_claims: [
      'The decoded name is related by its byte values; that it borrows the input slice (zero copy) is not stated.',
      'A packet is related by its first len bytes; the contents of the rest of its 64-byte buffer are not stated.',
      'No claim about callers of the codec (dataspace table actions, noble-syndicate) beyond its results.',
      'No Rust compiler, Charon or Aeneas correctness claim.',
    ],
    scope: 'Universal theorems over the checked-in generated ServiceRef codec: for every byte slice and every configured depth the decoder returns, and the decision it returns, read through observed, equals NobleFoundation.ServiceWire.decode; for every string and readiness tag the encoder returns a packet carrying exactly the model encoding of a valid name, and InvalidName otherwise; totality, exact acceptance, exact refusal classes, canonical-only acceptance and both Rust-level round trips through Packet.as_bytes follow.',
    artifact_directory: output, command_count: commandSerial,
  };
  writeFileSync(join(output, 'receipt.json'), json(receipt));
  console.log(json({ receipt: join(output, 'receipt.json'), source_commit: initial.source_commit,
    clean_source: receipt.clean_source, theorems: names.length,
    bound_source_files: binding.source_files.length, mutation_controls: mutations.length }));
} catch (error) {
  console.error(`[service-wire refinement] FAIL ${error.stack ?? error}`);
  process.exitCode = 1;
}
