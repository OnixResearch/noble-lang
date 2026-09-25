// Extend (never replace) the M4/M5/M6 extraction inventory with source-bound
// dataspace declarations and strict correspondence to independent Lean models.
import fs from 'node:fs';
import path from 'node:path';
import { account, equal, fail, lanes, sha } from '../m4/accounting.mjs';

const kernel = 'noble_kernel.dataspace';
const mainSource = 'crates/noble-kernel/src/dataspace/mod.rs';
const sourcePrefix = 'crates/noble-kernel/src/dataspace/';
const exactRoots = [
  { rust: 'noble_kernel::dataspace::admit_shared_memory',
    declaration: `${kernel}.admit_shared_memory`, def_id: 31 },
  { rust: 'noble_kernel::dataspace::decide_publication',
    declaration: `${kernel}.decide_publication`, def_id: 32 },
  { rust: 'noble_kernel::dataspace::permits',
    declaration: `${kernel}.permits`, def_id: 33 },
];
const tableRoots = [
  ...['value', 'reserve'].map(method => ({
    rust: `noble_kernel::dataspace::table::{noble_kernel::dataspace::Table}::${method}`,
    declaration: `${kernel}.table.Table.${method}`,
  })),
  ...['publish', 'observe', 'retract', 'retire'].map(method => ({
    rust: `noble_kernel::dataspace::table::actions::{noble_kernel::dataspace::Table}::${method}`,
    declaration: `${kernel}.table.actions.Table.${method}`,
  })),
];
const theoremRoots = [
  { name: 'publication_refines', implementation: exactRoots[1].declaration, reference: 'M7Syndicate.decide' },
  { name: 'permission_refines', implementation: exactRoots[2].declaration, reference: 'M7Syndicate.allowed' },
  { name: 'shared_memory_refines', implementation: exactRoots[0].declaration, reference: 'M7Syndicate.admit' },
  { name: 'shared_memory_refused', implementation: exactRoots[0].declaration,
    correspondence: 'M7Syndicate.shared_memory_refines', reference: 'M7Syndicate.admit' },
  { name: 'publish_right_required', implementation: exactRoots[2].declaration,
    correspondence: 'M7Syndicate.permission_refines', reference: 'M7Syndicate.allowed' },
  { name: 'observe_right_required', implementation: exactRoots[2].declaration,
    correspondence: 'M7Syndicate.permission_refines', reference: 'M7Syndicate.allowed' },
  { name: 'duplicate_publication_unchanged', implementation: exactRoots[1].declaration,
    correspondence: 'M7Syndicate.publication_refines', reference: 'M7Syndicate.decide' },
  { name: 'changed_publication_replaces', implementation: exactRoots[1].declaration,
    correspondence: 'M7Syndicate.publication_refines', reference: 'M7Syndicate.decide' },
  { name: 'first_publication_adds', implementation: exactRoots[1].declaration,
    correspondence: 'M7Syndicate.publication_refines', reference: 'M7Syndicate.decide' },
];
const referenceRoots = ['M7Syndicate.admit', 'M7Syndicate.decide', 'M7Syndicate.allowed']
  .map(declaration => ({ declaration, owner: 'M7Syndicate' }));
const leanName = row => row.lean_name.startsWith('noble_kernel.')
  ? row.lean_name : `noble_kernel.${row.lean_name}`;

export function syndicatePacket(accounts, sources, packet) {
  const lane = lanes.find(item => item.id === 'kernel');
  const sourceFiles = Object.fromEntries(Object.entries(sources).filter(([file]) =>
    file.startsWith(sourcePrefix) && file.endsWith('.rs')));
  for (const file of [mainSource, `${sourcePrefix}table.rs`, `${sourcePrefix}wire.rs`]) {
    if (!sourceFiles[file]) fail('M7-SOURCE', `missing production source ${file}`);
  }
  for (const [file, digest] of Object.entries(sourceFiles)) {
    equal(accounts.kernel.collected_files[file], digest, 'M7-SOURCE', `uncollected production source ${file}`);
  }
  const hostAdapterFiles = Object.fromEntries(Object.entries(sources).filter(([file]) =>
    file.startsWith('crates/noble-syndicate/')));
  if (!hostAdapterFiles['crates/noble-syndicate/Cargo.toml'] ||
      !hostAdapterFiles['crates/noble-syndicate/src/lib.rs']) {
    fail('M7-SOURCE', 'missing separately compiled std host-policy adapter source inventory');
  }
  const declarations = Object.entries(accounts.kernel.translation).flatMap(([kind, entries]) =>
    entries.filter(row => row.is_local && row.source.file.startsWith(sourcePrefix)).map(row => {
      const expected = kind === 'types' || kind === 'trait_decls' ? 'Types.lean' : 'Funs.lean';
      if (row.lean_file !== expected || row.is_opaque) fail('M7-BODY', `not an actual translated body: ${row.rust_name}`);
      const declaration = leanName(row);
      const owner = `NobleKernel.${expected.slice(0, -5)}`;
      if (!packet.declarations.some(item => item.name === declaration && item.owner === owner && item.local)) {
        fail('M7-BODY', `missing canonical M4 audit declaration: ${declaration}`);
      }
      return { declaration, owner, lane: lane.id, kind, def_id: row.def_id,
        rust: row.rust_name, source: row.source, loop: row.loop ?? null };
    }));
  const roots = [...exactRoots, ...tableRoots].map(required => {
    const found = declarations.filter(row => row.kind === 'functions' && !row.loop &&
      row.rust === required.rust && row.declaration === required.declaration);
    if (found.length !== 1 || required.def_id !== undefined && found[0].def_id !== required.def_id ||
        exactRoots.includes(required) && found[0].source.file !== mainSource) {
      fail('M7-ROOT', `${JSON.stringify(required)}: ${found.length} matching nonopaque source bodies`);
    }
    const row = found[0];
    const required_dependencies = required.declaration === `${kernel}.table.Table.value`
      ? [exactRoots[2].declaration]
      : required.declaration === `${kernel}.table.actions.Table.publish`
        ? [exactRoots[1].declaration, exactRoots[2].declaration]
        : ['observe', 'retract'].some(method => required.declaration === `${kernel}.table.actions.Table.${method}`)
          ? [exactRoots[2].declaration] : [];
    return { ...row, required_dependencies };
  });
  return { schema: 'noble-m7-extraction-subjects/v1', source_files: sourceFiles,
    host_adapter_source_files: hostAdapterFiles,
    declarations, roots, implementation_roots: roots.filter(row => exactRoots.some(root => root.declaration === row.declaration)),
    strict_roots: theoremRoots.map(({ name, implementation, reference, correspondence }) => ({
      declaration: `M7Syndicate.${name}`, owner: 'M7Syndicate',
      type_dependencies: correspondence ? [implementation] : [implementation, reference],
      required_dependencies: correspondence ? [implementation, correspondence, reference] : [implementation, reference],
    })), reference_roots: referenceRoots,
    scope: 'All three inherited pure production crates are re-extracted; every new std host-policy adapter source is bound separately but not Aeneas-translated. Complete dataspace local body inventory and dependency coverage; only three named pure functions have strict universal correspondence. Table/wire bodies, host synchronization/Drop, adapter and runtime remain open refinement obligations.' };
}

export function syndicateCoverage(subjects, audits) {
  const views = [audits.contracts, audits.wasm];
  const roots = subjects.roots.map(root => {
    const rows = views.map(view => view.root_audits.find(item => item.declaration === root.declaration));
    if (rows.some(row => !row || row.owner !== root.owner)) fail('M7-COMPILED', `missing compiled root: ${root.declaration}`);
    equal(rows[1], rows[0], 'M7-COMPILED', `inconsistent inherited kernel root: ${root.declaration}`);
    for (const dependency of root.required_dependencies) {
      if (!rows[0].reachable_project_declarations.includes(dependency)) {
        fail('M7-DEPENDENCY', `${root.declaration} detached from ${dependency}`);
      }
    }
    return { ...root, ...rows[0] };
  });
  const declarations = subjects.declarations.map(row => {
    const results = views.map(view => view.project_declarations.find(item => item.declaration === row.declaration));
    if (results.some(result => !result || result.owner !== row.owner ||
        row.kind === 'functions' && result.kind !== 'definition')) {
      fail('M7-COMPILED', `missing actual translated declaration: ${row.declaration}`);
    }
    equal([results[1].owner, results[1].kind, results[1].axioms],
      [results[0].owner, results[0].kind, results[0].axioms],
      'M7-COMPILED', `inconsistent inherited kernel body: ${row.declaration}`);
    return { ...row, compiled_kind: results[0].kind, axioms: results[0].axioms,
      reachable_from_roots: roots.filter(root => root.reachable_project_declarations.includes(row.declaration))
        .map(root => root.declaration) };
  });
  return { schema: 'noble-m7-compiled-coverage/v1', source_files: subjects.source_files,
    host_adapter_source_files: subjects.host_adapter_source_files,
    declarations, roots, implementation_roots: subjects.implementation_roots, strict_roots: subjects.strict_roots,
    scope: subjects.scope };
}

export function syndicateAuditPacket(subjects, binding) {
  return { schema: 'noble-m7-syndicate-audit/v1', binding,
    strict_roots: subjects.strict_roots, reference_roots: subjects.reference_roots,
    implementation_roots: subjects.implementation_roots,
    declarations: subjects.declarations, host_adapter_source_files: subjects.host_adapter_source_files };
}

export function syndicateAudit({ packet, run, proofRoot, lake, artifacts, save }) {
  const inputFile = path.join(artifacts, 'm7-audit-input.json');
  save('m7-audit-input.json', packet);
  const inputBytes = fs.readFileSync(inputFile);
  const source = 'M7ExtractionGate.lean';
  fs.writeFileSync(path.join(proofRoot, source), 'import M7Audit\n\ncheck_m7_syndicate\n', { flag: 'wx' });
  run('m7-syndicate-build', lake, ['build', 'M7Audit'], proofRoot);
  const observed = run('m7-syndicate-audit', lake, ['env', 'lean', source], proofRoot,
    { NOBLE_M7_EXTRACTION_SUBJECTS: inputFile });
  const records = observed.output.split('\n').filter(line => line.startsWith('M7-SYNDICATE '))
    .map(line => JSON.parse(line.slice('M7-SYNDICATE '.length)));
  if (records.length !== 1 || records[0].schema !== packet.schema || records[0].result !== 'passed') {
    fail('M7-VERDICT', 'missing compiled strict actual-source syndicate audit');
  }
  const audit = records[0];
  equal(audit.packet, packet, 'M7-VERDICT', 'incomplete byte-bound proof input echo');
  for (const key of ['strict_roots', 'implementation_roots', 'declarations']) {
    equal(audit[key].map(row => row.declaration), packet[key].map(row => row.declaration),
      'M7-VERDICT', `missing ${key}`);
  }
  equal(audit.reference_roots?.map(row => row.declaration), packet.reference_roots.map(row => row.declaration),
    'M7-VERDICT', 'missing independent reference model');
  equal(audit.strict_axioms, ['propext', 'Classical.choice', 'Quot.sound'],
    'M7-AXIOM', 'strict theorem axiom allowance changed');
  if (!fs.readFileSync(inputFile).equals(inputBytes)) fail('M7-PACKET', 'proof input changed during audit');
  const evidence = { packet_sha256: sha(inputBytes), audit };
  save('m7-syndicate-refinement.json', evidence);
  return evidence;
}

export function syndicateRefusals({ raw, accounts, sources, packet, subjects, coverage, proofPacket,
  run, proofRoot, lake, artifacts, save }) {
  const results = [];
  function reject(label, code, action) {
    try { action(); } catch (error) {
      if (!String(error).includes(`${code}:`)) fail('M7-REFUSAL', `${label}: wrong refusal: ${error}`);
      results.push({ label, expected: code, result: 'refused', diagnostic: String(error),
        kind: 'mutated-source-bound-extraction-inventory; not runtime-conformance' });
      return;
    }
    fail('M7-REFUSAL', `${label}: mutation accepted`);
  }
  const lane = lanes.find(item => item.id === 'kernel');
  for (const root of subjects.implementation_roots) {
    reject(`omitted-${root.declaration}`, 'M4-OMITTED-BODY', () => {
      const translated = structuredClone(raw.kernel.translation);
      translated.functions = translated.functions.filter(row => row.def_id !== root.def_id);
      account(lane, raw.kernel.llbc, translated, sources);
    });
    reject(`opaque-${root.declaration}`, 'M4-LOCAL-OPAQUE', () => {
      const llbc = structuredClone(raw.kernel.llbc);
      const translated = structuredClone(raw.kernel.translation);
      llbc.translated.fun_decls[root.def_id].body = 'Opaque';
      for (const row of translated.functions.filter(row => row.def_id === root.def_id)) {
        row.is_opaque = true;
        row.lean_file = 'FunsExternal_Template.lean';
      }
      account(lane, llbc, translated, sources);
    });
    reject(`missing-${root.declaration}`, 'M4-M7-ROOT', () => {
      const changed = structuredClone(accounts);
      changed.kernel.translation.functions = changed.kernel.translation.functions.filter(row =>
        leanName(row) !== root.declaration);
      syndicatePacket(changed, sources, packet);
    });
  }
  for (const file of Object.keys(subjects.source_files).filter(file => file !== mainSource)) {
    const bodies = subjects.declarations.filter(row =>
      row.kind === 'functions' && !row.loop && row.source.file === file);
    const body = bodies.find(row => subjects.roots.some(root => root.declaration === row.declaration)) ??
      bodies.find(row => !row.rust.includes('::{impl ')) ?? bodies[0];
    if (!body) fail('M7-REFUSAL', `no extracted authored body in ${file}`);
    const label = path.basename(file, '.rs');
    reject(`omitted-${label}-body`, 'M4-OMITTED-BODY', () => {
      const translated = structuredClone(raw.kernel.translation);
      translated.functions = translated.functions.filter(row => row.def_id !== body.def_id);
      account(lane, raw.kernel.llbc, translated, sources);
    });
    reject(`opaque-${label}-body`, 'M4-LOCAL-OPAQUE', () => {
      const llbc = structuredClone(raw.kernel.llbc);
      const translated = structuredClone(raw.kernel.translation);
      llbc.translated.fun_decls[body.def_id].body = 'Opaque';
      translated.functions = translated.functions.filter(row => row.def_id !== body.def_id || !row.loop);
      for (const row of translated.functions.filter(row => row.def_id === body.def_id)) {
        row.is_opaque = true;
        row.lean_file = 'FunsExternal_Template.lean';
      }
      account(lane, llbc, translated, sources);
    });
  }
  for (const root of subjects.roots.filter(row => row.required_dependencies.length)) {
    for (const dependency of root.required_dependencies) {
      reject(`detached-${root.declaration}-${dependency}`, 'M4-M7-DEPENDENCY', () => {
        const changed = structuredClone(coverage);
        for (const laneId of ['contracts', 'wasm']) {
          for (const row of changed[laneId].root_audits.filter(row => row.declaration === root.declaration)) {
            row.reachable_project_declarations = row.reachable_project_declarations.filter(name => name !== dependency);
          }
        }
        syndicateCoverage(subjects, changed);
      });
    }
  }
  function lean(label, code, prelude = '', mutate = () => {}) {
    const input = structuredClone(proofPacket);
    mutate(input);
    const inputFile = path.join(artifacts, `m7-refusal-${label}.json`);
    const bytes = JSON.stringify(input, null, 2) + '\n';
    fs.writeFileSync(inputFile, bytes, { flag: 'wx' });
    const source = `M7Refusal_${label.replaceAll('-', '_')}.lean`;
    fs.writeFileSync(path.join(proofRoot, source), `import M7Audit\n\n${prelude}\n\ncheck_m7_syndicate\n`, { flag: 'wx' });
    const observed = run(`m7-refusal-${label}`, lake, ['env', 'lean', source], proofRoot,
      { NOBLE_M7_EXTRACTION_SUBJECTS: inputFile }, true);
    if (observed.exit === 0 || observed.signal || observed.error || !observed.output.includes(`${code}:`)) {
      fail('M7-REFUSAL', `${label}: not the required compiled proof-audit refusal`);
    }
    equal(fs.readFileSync(inputFile, 'utf8'), bytes, 'M7-PACKET', 'refusal input changed during audit');
    results.push({ label, expected: code, result: 'refused', kind: 'executed-Lean-audit-control; not runtime-conformance',
      input_sha256: sha(bytes), log: observed.log, output_sha256: observed.output_sha256 });
    save('m7-refusals.json', results);
  }
  lean('missing-inventory-binding', 'M7-PACKET', '', input => { delete input.binding.inventory_sha256; });
  lean('missing-source-binding', 'M7-PACKET', '', input => { delete input.binding.source_files[mainSource]; });
  lean('missing-strict-root', 'M7-ROOT', '', input => { input.strict_roots.pop(); });
  lean('substituted-strict-root', 'M7-ROOT', 'theorem M7Syndicate.m7_substitute : True := by trivial',
    input => { input.strict_roots[0].declaration = 'M7Syndicate.m7_substitute'; });
  lean('missing-implementation-root', 'M7-ROOT', '', input => { input.implementation_roots.pop(); });
  lean('missing-reference-root', 'M7-ROOT', '', input => { input.reference_roots.pop(); });
  lean('substituted-production-body', 'M7-OWNER', 'def noble_kernel.dataspace.m7_substitute : Bool := true',
    input => { input.declarations.push({ ...input.declarations[0], declaration: `${kernel}.m7_substitute` }); });
  lean('omitted-compiled-body', 'M7-COVERAGE', '', input => {
    input.declarations = input.declarations.filter(row => row.declaration !== subjects.implementation_roots[0].declaration);
  });
  lean('unused-proof-axiom', 'M7-AXIOM', 'axiom M7Syndicate.m7_axiom : Bool');
  lean('unused-proof-hole', 'M7-AXIOM', 'theorem M7Syndicate.m7_hole : True := by sorry');
  lean('native-proof', 'M7-AXIOM', 'theorem M7Syndicate.m7_native : (17 : Nat) + 1 = 18 := by native_decide');
  lean('opaque-proof-model', 'M7-TRANSPARENT', 'opaque M7Syndicate.m7_opaque : Bool := true');
  lean('finite-publication-instance', 'M7-TYPE',
    'theorem M7Syndicate.m7_finite :\n' +
    '  noble_kernel.dataspace.decide_publication none true = .ok (M7Syndicate.decide none true) :=\n' +
    '  M7Syndicate.publication_refines none true\n' +
    'check_m7_contract_control "M7Syndicate.m7_finite" "M7Syndicate.publication_refines"');
  lean('vacuous-publication-premise', 'M7-TYPE',
    'theorem M7Syndicate.m7_vacuous (current : Option Bool) (requested : Bool) (_h : False) :\n' +
    '  noble_kernel.dataspace.decide_publication current requested =\n' +
    '    .ok (M7Syndicate.decide current requested) :=\n' +
    '  M7Syndicate.publication_refines current requested\n' +
    'check_m7_contract_control "M7Syndicate.m7_vacuous" "M7Syndicate.publication_refines"');
  lean('weakened-publication-claim', 'M7-TYPE',
    'theorem M7Syndicate.m7_weakened (current : Option Bool) (requested : Bool) :\n' +
    '  noble_kernel.dataspace.decide_publication current requested =\n' +
    '    .ok (M7Syndicate.decide current requested) ∨ True := Or.inr trivial\n' +
    'check_m7_contract_control "M7Syndicate.m7_weakened" "M7Syndicate.publication_refines"');
  lean('implementation-backed-reference', 'M7-INDEPENDENCE',
    'def M7Syndicate.m7_reference_wrapper := noble_kernel.dataspace.decide_publication\n' +
    'check_m7_reference_control "M7Syndicate.m7_reference_wrapper"');
  lean('detached-table-wrapper', 'M7-DEPENDENCY',
    'def M7Syndicate.m7_detached := M7Syndicate.decide\n' +
    'check_m7_dependency_control "M7Syndicate.m7_detached" "noble_kernel.dataspace.decide_publication"');
  save('m7-refusals.json', results);
  return results;
}
