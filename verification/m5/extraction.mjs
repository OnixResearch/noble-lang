// M5 extends, rather than replaces, the compiler-owned whole-source inventory.
// Pure resource correspondence is a separate strict proof audit. Component and
// authority coverage below is extraction/dependency evidence, not refinement.
import fs from 'node:fs';
import path from 'node:path';
import { account, equal, fail, lanes, sha } from '../m4/accounting.mjs';

const groups = [
  { id: 'resources', lane: 'kernel', module: 'resources' },
  { id: 'authority', lane: 'kernel', module: 'authority' },
  { id: 'component-frontend', lane: 'contracts', module: 'component' },
  { id: 'component-backend', lane: 'wasm', module: 'component' },
];
const transition = 'noble_kernel.resources.transition.transition';
const tableDecide = 'noble_kernel.resources.table.Table.decide';
const accounting = 'noble_kernel.resources.Decision.accounting';
const strictRoots = [
  { declaration: 'M5Resources.transition_refines', owner: 'M5Resources',
    type_dependencies: [transition, 'M5Resources.step'], required_dependencies: [transition, 'M5Resources.step'] },
  { declaration: 'M5Resources.table_decide_refines', owner: 'M5Resources',
    type_dependencies: [tableDecide, 'M5Resources.tableStep'],
    required_dependencies: [tableDecide, 'M5Resources.tableStep', transition, 'M5Resources.step'] },
  ...['cancelled_pin_retained', 'late_completion_once', 'retired_completion_idempotent',
    'normal_completion_owner_once'].map(theorem => ({
    declaration: `M5Resources.${theorem}`, owner: 'M5Resources',
    type_dependencies: [transition, accounting, 'M5Resources.valid', 'M5Resources.scopeCheck'],
    required_dependencies: [transition, accounting, 'M5Resources.valid', 'M5Resources.scopeCheck'],
  })),
  { declaration: 'M5Resources.busy_owner_not_reusable', owner: 'M5Resources',
    type_dependencies: [transition, 'M5Resources.valid'],
    required_dependencies: [transition, 'M5Resources.valid'] },
];
const requiredRoots = [
  { group: 'resources', rust: 'noble_kernel::resources::transition::transition' },
  ...['new', 'register', 'validate', 'begin', 'native_access', 'complete', 'revoke',
    'retire', 'release', 'transfer', 'retire_context', 'snapshot', 'observation', 'decide']
    .map(method => ({ group: 'resources', type: 'Table', method })),
  { group: 'resources', type: 'Decision', method: 'accounting' },
  ...['new', 'advance_from_trusted_host', 'decide', 'authorize_from_trusted_host',
    'admit', 'start_execution', 'retire_witness', 'observe_from_trusted_host',
    'cancel_invocation', 'fail_invocation', 'deliver_success', 'receipt', 'admit_receipt',
    'counters', 'requested_effects', 'invocation_outcome', 'witness_snapshot', 'attempt_snapshot']
    .map(method => ({ group: 'authority', type: 'Authority', method })),
  ...['parse', 'prepare_export', 'check_import_effect', 'environment', 'session', 'build_context']
    .map(method => ({ group: 'component-frontend', type: 'World', method })),
  { group: 'component-backend', rust: 'noble_wasm::component::compile' },
];
const tableTransitions = ['validate', 'begin', 'native_access', 'complete', 'revoke',
  'retire', 'release', 'transfer'];
const leanName = (lane, row) => row.lean_name.startsWith(`${lane.crate}.`)
  ? row.lean_name : `${lane.crate}.${row.lean_name}`;

export function componentPacket(accounts, sources, packet) {
  const scopes = groups.map(group => {
    const lane = lanes.find(lane => lane.id === group.lane);
    const prefix = `${lane.crate}::${group.module}::`;
    const source = `crates/${lane.package}/src/${group.module}`;
    const sourceFiles = Object.fromEntries(Object.entries(sources).filter(([file]) =>
      file === `${source}.rs` || file.startsWith(`${source}/`) && file.endsWith('.rs')));
    if (!sourceFiles[`${source}.rs`] && !sourceFiles[`${source}/mod.rs`]) {
      fail('M5-SOURCE', `missing production ${group.id} module source`);
    }
    for (const [file, digest] of Object.entries(sourceFiles)) {
      equal(accounts[lane.id].collected_files[file], digest, 'M5-SOURCE', `uncollected source: ${file}`);
    }
    const declarations = Object.entries(accounts[lane.id].translation).flatMap(([kind, rows]) =>
      rows.filter(row => row.is_local && row.rust_name.startsWith(prefix)).map(row => {
        const expected = kind === 'types' || kind === 'trait_decls' ? 'Types.lean' : 'Funs.lean';
        if (row.is_opaque || row.lean_file !== expected) {
          fail('M5-BODY', `new authored declaration is not an actual translation: ${row.rust_name}`);
        }
        const declaration = leanName(lane, row);
        const owner = `${lane.module}.${expected.slice(0, -5)}`;
        if (!packet.declarations.some(item => item.name === declaration && item.owner === owner && item.local)) {
          fail('M5-BODY', `missing canonical audit subject: ${declaration}`);
        }
        return { declaration, owner, lane: lane.id, kind, def_id: row.def_id,
          rust: row.rust_name, source: row.source, loop: row.loop ?? null };
      }));
    if (!declarations.some(row => row.kind === 'functions')) fail('M5-BODY', `empty ${group.id} body inventory`);
    return { ...group, source_files: sourceFiles, declarations };
  });
  const roots = requiredRoots.map(required => {
    const scope = scopes.find(scope => scope.id === required.group);
    const matches = scope.declarations.filter(row => row.kind === 'functions' && !row.loop &&
      (required.rust ? row.rust === required.rust : row.rust.endsWith(`::${required.type}}::${required.method}`)));
    if (matches.length !== 1) fail('M5-ROOT', `${JSON.stringify(required)}: found ${matches.length}`);
    const row = matches[0];
    const requiredDependencies = [];
    if (row.declaration === tableDecide) requiredDependencies.push(transition);
    if (required.group === 'resources' && tableTransitions.includes(required.method)) requiredDependencies.push(tableDecide);
    // Whole-context retirement iterates retained records directly; hostile
    // handle entrypoints instead go through Table.decide's checked lookup.
    if (required.group === 'resources' && required.method === 'retire_context') requiredDependencies.push(transition);
    const requiresAcceptance = required.method === 'prepare_export' || required.group === 'component-backend';
    if (requiresAcceptance) requiredDependencies.push('noble_kernel.acceptance.check');
    return { ...required, lane: row.lane, kind: row.kind, rust: row.rust,
      declaration: row.declaration, owner: row.owner, requires_acceptance: requiresAcceptance,
      required_dependencies: requiredDependencies };
  });
  for (const implementation of [transition, tableDecide, accounting]) {
    if (!roots.some(root => root.declaration === implementation && root.owner === 'NobleKernel.Funs')) {
      fail('M5-ROOT', `missing actual resource implementation: ${implementation}`);
    }
  }
  return { schema: 'm5-extraction-subjects/v1', scopes, roots, strict_roots: strictRoots,
    // All model renewals remain audited, including ones shared with older code.
    newly_required_models: packet.boundaries.filter(row => row.classification === 'newly-required-model'),
    scope: 'Whole-source resource/authority/component accounting and compiled logical dependencies; only the separately named resource theorems claim universal correspondence.' };
}

export function componentCoverage(packet, audit) {
  const views = lane => lane === 'kernel' ? [audit.contracts, audit.wasm] : [audit[lane]];
  const declaration = (name, owner, lane) => {
    const rows = views(lane).map(view => view.project_declarations.find(row => row.declaration === name));
    if (rows.some(row => !row || owner && row.owner !== owner)) fail('M5-COMPILED', `missing/substituted declaration: ${name}`);
    for (const row of rows.slice(1)) {
      equal([row.owner, row.kind, row.axioms], [rows[0].owner, rows[0].kind, rows[0].axioms],
        'M5-COMPILED', `inconsistent shared kernel audit: ${name}`);
    }
    return rows[0];
  };
  const roots = packet.roots.map(root => {
    const rows = views(root.lane).map(view => view.root_audits.find(row => row.declaration === root.declaration));
    if (rows.some(row => !row || row.owner !== root.owner)) fail('M5-COMPILED', `missing root audit: ${root.declaration}`);
    for (const row of rows.slice(1)) equal(row, rows[0], 'M5-COMPILED', `inconsistent root: ${root.declaration}`);
    for (const required of root.required_dependencies) {
      if (!rows[0].reachable_project_declarations.includes(required)) {
        fail('M5-DEPENDENCY', `${root.declaration} detached from actual ${required}`);
      }
    }
    return { ...root, ...rows[0] };
  });
  const scopes = packet.scopes.map(scope => ({ ...scope, declarations: scope.declarations.map(row => {
    const compiled = declaration(row.declaration, row.owner, row.lane);
    if (row.kind === 'functions' && compiled.kind !== 'definition') fail('M5-COMPILED', `not a body: ${row.declaration}`);
    return { ...row, compiled_kind: compiled.kind, axioms: compiled.axioms,
      reachable_from_roots: roots.filter(root => root.reachable_project_declarations.includes(row.declaration))
        .map(root => root.declaration) };
  }) }));
  const models = packet.newly_required_models.map(row => {
    const compiled = declaration(row.name, null, row.lane);
    return { ...row, owner: compiled.owner, compiled_kind: compiled.kind, axioms: compiled.axioms,
      reachable_from_roots: roots.filter(root => root.reachable_project_declarations.includes(row.name))
        .map(root => root.declaration) };
  });
  return { schema: 'm5-compiled-coverage/v1', scopes, roots, newly_required_models: models,
    scope: packet.scope };
}

export function resourceAuditPacket(subjects, binding) {
  return { schema: 'm5-resource-audit/v1', binding, strict_roots: subjects.strict_roots,
    implementation_roots: subjects.roots.filter(root => [transition, tableDecide, accounting].includes(root.declaration)),
    // This independent kernel environment also checks every new resources and
    // authority declaration, not only definitions reached by the proof roots.
    declarations: subjects.scopes.filter(scope => scope.lane === 'kernel').flatMap(scope => scope.declarations) };
}

export function resourceAudit({ packet, run, proofRoot, lake, artifacts, save }) {
  const inputFile = path.join(artifacts, 'm5-audit-input.json');
  save('m5-audit-input.json', packet);
  const inputBytes = fs.readFileSync(inputFile);
  const source = 'M5ExtractionGate.lean';
  fs.writeFileSync(path.join(proofRoot, source), 'import M5Audit\n\ncheck_m5_resources\n', { flag: 'wx' });
  run('m5-resource-build', lake, ['build', 'M5Audit'], proofRoot);
  const observed = run('m5-resource-audit', lake, ['env', 'lean', source], proofRoot,
    { NOBLE_M5_EXTRACTION_SUBJECTS: inputFile });
  const records = observed.output.split('\n').filter(line => line.startsWith('M5-RESOURCES '))
    .map(line => JSON.parse(line.slice('M5-RESOURCES '.length)));
  if (records.length !== 1 || records[0].schema !== packet.schema || records[0].result !== 'passed') {
    fail('M5-VERDICT', 'missing compiled strict resource-refinement audit');
  }
  const audit = records[0];
  equal(audit.packet, packet, 'M5-VERDICT', 'incomplete byte-bound proof input echo');
  equal(audit.strict_roots.map(row => row.declaration), packet.strict_roots.map(row => row.declaration),
    'M5-VERDICT', 'missing strict resource theorem');
  equal(audit.implementation_roots.map(row => row.declaration), packet.implementation_roots.map(row => row.declaration),
    'M5-VERDICT', 'missing implementation dependency audit');
  equal(audit.declarations.map(row => row.declaration), packet.declarations.map(row => row.declaration),
    'M5-VERDICT', 'missing resource/authority compiled declaration');
  if (!fs.readFileSync(inputFile).equals(inputBytes)) fail('M5-PACKET', 'proof input changed during audit');
  const evidence = { packet_sha256: sha(inputBytes), audit };
  save('m5-resource-refinement.json', evidence);
  return evidence;
}

export function componentRefusals({ raw, accounts, sources, packet, subjects, coverage, proofPacket,
  run, proofRoot, lake, artifacts, save }) {
  const results = [];
  function reject(label, code, action) {
    try { action(); } catch (error) {
      if (!String(error).includes(`${code}:`)) fail('M5-REFUSAL', `${label}: wrong refusal: ${error}`);
      results.push({ label, expected: code, result: 'refused', diagnostic: String(error),
        kind: 'mutated-extraction-accounting-control; not additional Rust extraction' });
      return;
    }
    fail('M5-REFUSAL', `${label}: mutation accepted`);
  }
  for (const scope of subjects.scopes) {
    const lane = lanes.find(lane => lane.id === scope.lane);
    const body = scope.declarations.find(row => row.kind === 'functions' && !row.loop &&
      subjects.roots.some(root => root.declaration === row.declaration));
    reject(`missing-${scope.id}-body`, 'M4-OMITTED-BODY', () => {
      const translation = structuredClone(raw[lane.id].translation);
      translation.functions = translation.functions.filter(row => row.def_id !== body.def_id);
      account(lane, raw[lane.id].llbc, translation, sources);
    });
    reject(`opaque-${scope.id}-body`, 'M4-LOCAL-OPAQUE', () => {
      const llbc = structuredClone(raw[lane.id].llbc);
      const translation = structuredClone(raw[lane.id].translation);
      llbc.translated.fun_decls[body.def_id].body = 'Opaque';
      translation.functions = translation.functions.filter(row => row.def_id !== body.def_id || !row.loop);
      for (const row of translation.functions.filter(row => row.def_id === body.def_id)) {
        row.is_opaque = true;
        row.lean_file = 'FunsExternal_Template.lean';
      }
      account(lane, llbc, translation, sources);
    });
  }
  reject('missing-production-table-decide', 'M4-M5-ROOT', () => {
    const changed = structuredClone(accounts);
    changed.kernel.translation.functions = changed.kernel.translation.functions.filter(row => row.lean_name !== tableDecide);
    componentPacket(changed, sources, packet);
  });
  reject('detached-table-transition-dependency', 'M4-M5-DEPENDENCY', () => {
    const changed = structuredClone(coverage);
    for (const lane of ['contracts', 'wasm']) {
      const root = changed[lane].root_audits.find(row => row.declaration === tableDecide);
      root.reachable_project_declarations = root.reachable_project_declarations.filter(name => name !== transition);
    }
    componentCoverage(subjects, changed);
  });
  reject('detached-context-retirement-transition', 'M4-M5-DEPENDENCY', () => {
    const changed = structuredClone(coverage);
    const retirement = subjects.roots.find(root =>
      root.group === 'resources' && root.method === 'retire_context').declaration;
    for (const lane of ['contracts', 'wasm']) {
      const root = changed[lane].root_audits.find(row => row.declaration === retirement);
      root.reachable_project_declarations = root.reachable_project_declarations.filter(name => name !== transition);
    }
    componentCoverage(subjects, changed);
  });
  function lean(label, code, prelude = '', mutate = () => {}) {
    const input = structuredClone(proofPacket);
    mutate(input);
    const inputFile = path.join(artifacts, `m5-refusal-${label}.json`);
    const bytes = JSON.stringify(input, null, 2) + '\n';
    fs.writeFileSync(inputFile, bytes, { flag: 'wx' });
    const source = `M5Refusal_${label.replaceAll('-', '_')}.lean`;
    fs.writeFileSync(path.join(proofRoot, source),
      `import M5Audit\n\n${prelude}\n\ncheck_m5_resources\n`, { flag: 'wx' });
    const observed = run(`m5-refusal-${label}`, lake, ['env', 'lean', source], proofRoot,
      { NOBLE_M5_EXTRACTION_SUBJECTS: inputFile }, true);
    if (observed.exit === 0 || observed.signal || observed.error || !observed.output.includes(`${code}:`)) {
      fail('M5-REFUSAL', `${label}: not the required compiled proof-audit refusal`);
    }
    equal(fs.readFileSync(inputFile, 'utf8'), bytes, 'M5-PACKET', 'control input changed during audit');
    results.push({ label, expected: code, result: 'refused', kind: 'executed-Lean-audit-control',
      input_sha256: sha(bytes), log: observed.log, output_sha256: observed.output_sha256 });
    save('m5-refusals.json', results);
  }
  lean('missing-inventory-binding', 'M5-PACKET', '', input => { delete input.binding.inventory_sha256; });
  lean('missing-strict-root', 'M5-ROOT', '', input => { input.strict_roots.pop(); });
  lean('substituted-strict-root', 'M5-ROOT', 'theorem M5Resources.m5_substitute : True := by trivial',
    input => { input.strict_roots[0].declaration = 'M5Resources.m5_substitute'; });
  lean('missing-implementation-root', 'M5-ROOT', '', input => { input.implementation_roots.pop(); });
  lean('substituted-implementation-body', 'M5-OWNER', 'def noble_kernel.resources.m5_substitute : Bool := true', input => {
    input.declarations.push({ ...input.declarations.find(row => row.kind === 'functions'),
      declaration: 'noble_kernel.resources.m5_substitute' });
  });
  lean('omitted-compiled-resource-body', 'M5-COVERAGE', '', input => {
    input.declarations = input.declarations.filter(row => row.declaration !== transition);
  });
  lean('unused-proof-axiom', 'M5-AXIOM', 'axiom M5Resources.m5_axiom : Bool');
  lean('unused-proof-hole', 'M5-AXIOM', 'theorem M5Resources.m5_hole : True := by sorry');
  lean('native-resource-proof', 'M5-AXIOM',
    'theorem M5Resources.m5_native : (17 : Nat) + 1 = 18 := by native_decide');
  lean('opaque-resource-model', 'M5-TRANSPARENT', 'opaque M5Resources.m5_opaque : Bool := true');
  save('m5-refusals.json', results);
  return results;
}
