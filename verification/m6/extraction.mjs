// M6 adds to the M4/M5 inventories; it never replaces their roots or allowances.
// Constructor coverage below is compiled extraction evidence, NOT runtime
// conformance. The independent compiled peer exercises validate_coverage itself.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { account, equal, fail, lanes, sha } from '../m4/accounting.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const kernel = 'noble_kernel.async_tasks';
const transition = `${kernel}.transition.transition`;
const tableDecide = `${kernel}.table.Table.decide`;
const classify = `${kernel}.schema.classify`;
const validateCoverage = `${kernel}.schema.coverage.validate_coverage`;
const validateRows = `${kernel}.schema.coverage.validate_rows`;
const checkRow = `${kernel}.schema.coverage.check_row`;
const representative = `${kernel}.schema.coverage.EventKind.representative`;
const implementationRoots = [transition, tableDecide, classify];
const referenceRoots = ['M6Async.classify', 'M6Async.step', 'M6Async.tableStep'];
const qualifiedModels = ['M6Async.WellFormed', 'M6Async.completionEvent', 'M6Async.settled'];
const strictRoots = [
  { declaration: 'M6Async.transition_refines', owner: 'M6Async',
    type_dependencies: [transition, 'M6Async.step'], required_dependencies: [transition, 'M6Async.step'] },
  { declaration: 'M6Async.table_decide_refines', owner: 'M6Async',
    type_dependencies: [tableDecide, 'M6Async.tableStep'],
    required_dependencies: [tableDecide, 'M6Async.tableStep', transition, 'M6Async.step'] },
  { declaration: 'M6Async.classify_refines', owner: 'M6Async',
    type_dependencies: [classify, 'M6Async.classify'], required_dependencies: [classify, 'M6Async.classify'] },
  ...['cancellation_preserves_native_pins', 'completion_does_not_stop_native',
    'late_completion_not_delivered', 'delivered_cancel_idempotent', 'duplicate_completion_idempotent',
    'pins_require_native_stop', 'finish_requires_settlement', 'ready_results_owned_until_delivery',
    'failure_preserves_primary', 'release_once'].map(theorem => ({
    declaration: `M6Async.${theorem}`, owner: 'M6Async',
    type_dependencies: [transition], required_dependencies: [transition],
  })),
  { declaration: 'M6Async.recordCheck_refines', owner: 'M6Async',
    type_dependencies: [`${kernel}.transition.validation.record`, 'M6Async.recordCheck'],
    required_dependencies: [`${kernel}.transition.validation.record`, 'M6Async.recordCheck'] },
];
const groups = [
  { id: 'async-tasks', lane: 'kernel', module: 'async_tasks' },
  { id: 'async-frontend', lane: 'contracts', module: 'component' },
  { id: 'async-backend', lane: 'wasm', module: 'component',
    assets: ['crates/noble-wasm/src/component/async.wat', 'crates/noble-wasm/wit/async.wit'] },
];
const tableMethods = ['new', 'prepare', 'admit', 'free_slot', 'decide', 'apply', 'complete',
  'native_stopped', 'settle_pins', 'wake', 'deliver', 'cancel', 'consume', 'retire',
  'cleanup', 'take_wake', 'finish', 'inspect', 'snapshot', 'observation'];
const tableTransitions = ['apply', 'complete', 'native_stopped', 'settle_pins', 'wake',
  'deliver', 'cancel', 'consume', 'retire', 'cleanup', 'take_wake', 'finish', 'inspect'];
const requiredRoots = [
  ...['transition::transition', 'schema::classify', 'schema::coverage::validate_coverage',
    'schema::coverage::validate_rows', 'schema::coverage::check_row', 'schema::coverage::state_index'].map(suffix =>
    ({ group: 'async-tasks', rust: `noble_kernel::async_tasks::${suffix}` })),
  ...tableMethods.map(method => ({ group: 'async-tasks', type: 'Table', method })),
  { group: 'async-tasks', type: 'Admission', method: 'commit' },
  { group: 'async-tasks', type: 'Task', method: 'handle' },
  { group: 'async-tasks', type: 'Decision', method: 'accounting' },
  { group: 'async-tasks', type: 'Decision', method: 'unchanged' },
  { group: 'async-tasks', type: 'Accounting', method: 'empty' },
  { group: 'async-tasks', type: 'Obligations', method: 'empty' },
  { group: 'async-tasks', type: 'Event', method: 'kind' },
  ...['index', 'representative'].map(method => ({ group: 'async-tasks', type: 'EventKind', method })),
  ...['STATE_SCHEMA', 'EVENT_SCHEMA', 'STATE_CONSTRUCTORS', 'EVENT_CONSTRUCTORS'].map(name =>
    ({ group: 'async-tasks', kind: 'globals', rust: `noble_kernel::async_tasks::schema::${name}` })),
  ...['parse', 'prepare_export', 'check_import_effect', 'environment', 'session',
    'build_context', 'profile', 'is_async'].map(method => ({ group: 'async-frontend', type: 'World', method })),
  ...['compile', 'context', 'abi::indirect_parameters', 'emit::async_imports', 'emit::task::write',
    'lower::calls::invoke', 'lower::results::finish'].map(suffix =>
    ({ group: 'async-backend', rust: `noble_wasm::component::${suffix}` })),
];
const states = ['Pending', 'Ready', 'Delivered', 'Retiring', 'Retired'];
const events = ['Inspect', 'CompleteSuccess', 'CompleteDomainError', 'Deliver', 'Cancel', 'Trap',
  'Deadline', 'Budget', 'InternalFailure', 'NativeStopped', 'SettlePins', 'Cleanup', 'Wake', 'TakeWake', 'Finish'];
const schemaFiles = ['crates/noble-kernel/src/async_tasks/domain.rs',
  'crates/noble-kernel/src/async_tasks/schema.rs', 'crates/noble-kernel/src/async_tasks/schema/coverage.rs'];
const leanName = (lane, row) => row.lean_name.startsWith(`${lane.crate}.`)
  ? row.lean_name : `${lane.crate}.${row.lean_name}`;

// A small, deliberately closed schema grammar: unknown field syntax fails, it
// never guesses a constructor or silently discards an unrecognized payload.
function sourceSchema(accounts, sources) {
  const texts = Object.fromEntries(schemaFiles.map(file => {
    const bytes = fs.readFileSync(path.join(root, file));
    equal(sha(bytes), sources[file], 'M6-SOURCE', `schema source changed: ${file}`);
    equal(accounts.kernel.collected_files[file], sources[file], 'M6-SOURCE', `uncollected schema source: ${file}`);
    return [file, bytes.toString('utf8')];
  }));
  const domain = texts[schemaFiles[0]].replace(/\/\/[^\n]*|\/\*[\s\S]*?\*\//g, '');
  function body(kind, name, tuple = false) {
    const start = new RegExp(`\\bpub\\s+${kind}\\s+${name}\\s*${tuple ? '\\(' : '\\{'}`, 'g');
    const matches = [...domain.matchAll(start)];
    if (matches.length !== 1) fail('M6-SCHEMA', `missing/duplicate production ${name}`);
    let depth = 1;
    const begin = matches[0].index + matches[0][0].length;
    let end = begin;
    const open = tuple ? '(' : '{';
    const close = tuple ? ')' : '}';
    for (; end < domain.length && depth; end++) {
      if (domain[end] === open) depth++;
      if (domain[end] === close) depth--;
    }
    if (depth) fail('M6-SCHEMA', `unclosed ${name}`);
    const contents = domain.slice(begin, end - 1).replace(/\bpub\s+/g, '').replace(/\s+/g, '');
    if (!/^[A-Za-z0-9_:,{}()]*$/.test(contents)) fail('M6-SCHEMA', `unreviewed ${name} payload syntax`);
    return contents;
  }
  function variants(text) {
    const result = [];
    let depth = 0;
    let begin = 0;
    for (let index = 0; index < text.length; index++) {
      if ('{('.includes(text[index])) depth++;
      if ('})'.includes(text[index])) depth--;
      if (depth < 0) fail('M6-SCHEMA', 'unbalanced constructor payload');
      if (text[index] === ',' && depth === 0) { result.push(text.slice(begin, index)); begin = index + 1; }
    }
    if (depth) fail('M6-SCHEMA', 'unbalanced constructor payload');
    if (begin < text.length) result.push(text.slice(begin));
    if (result.some(value => !value)) fail('M6-SCHEMA', 'empty constructor');
    return result.map(value => value.replace(/,([})])/g, '$1'));
  }
  const prefix = 'noble-kernel::async_tasks::domain::';
  const stateVariants = variants(body('enum', 'State'));
  const eventVariants = variants(body('enum', 'Event'));
  const stateSchema = `${prefix}State{${stateVariants.join(';')}}`;
  const payloads = ['Completion', 'Disposition', 'Obligations'].map(name =>
    `${prefix}${name}{${body('struct', name).replace(/,$/, '')}}`);
  const eventSchema = `${prefix}Event{${eventVariants.join(';')}};` +
    `${prefix}NativeId(${body('struct', 'NativeId', true)});${payloads.join(';')}`;
  function literal(name) {
    const match = texts[schemaFiles[1]].match(new RegExp(`pub\\s+const\\s+${name}\\s*:\\s*&str\\s*=\\s*concat!\\(((?:"(?:\\\\.|[^"\\\\])*"|[\\s,])*)\\);`));
    if (!match) fail('M6-SCHEMA', `missing production ${name} literal`);
    const literals = match[1].match(/"(?:\\.|[^"\\])*"/g) ?? [];
    if (!literals.length || match[1].replace(/"(?:\\.|[^"\\])*"|[\s,]/g, '')) {
      fail('M6-SCHEMA', `non-literal ${name}`);
    }
    try { return literals.map(value => JSON.parse(value)).join(''); }
    catch { fail('M6-SCHEMA', `unreviewed Rust string escape in ${name}`); }
  }
  equal(literal('STATE_SCHEMA'), stateSchema, 'M6-SCHEMA', 'STATE_SCHEMA is stale against production State');
  equal(literal('EVENT_SCHEMA'), eventSchema, 'M6-SCHEMA', 'EVENT_SCHEMA omits/changes a production payload field');
  equal(stateVariants, states, 'M6-SCHEMA', 'unreviewed State constructors');
  equal(eventVariants.map(value => value.split(/[({]/)[0]), events, 'M6-SCHEMA', 'unreviewed Event constructors');
  return { schema: 'noble-m6-state-event-schema/v1', state_schema: stateSchema, event_schema: eventSchema,
    states, events, source_files: Object.fromEntries(schemaFiles.map(file => [file, sources[file]])),
    state_declaration: `${kernel}.domain.State`, event_declaration: `${kernel}.domain.Event`,
    classifier: classify, coverage_admission: validateCoverage,
    classification: 'source-bound-compiled-constructor-coverage; not runtime-conformance' };
}

function pairRule(state, event) {
  const live = state === 'Pending' || state === 'Ready';
  if (event === 'Inspect') return 'ok:Inspect';
  if (event === 'CompleteSuccess' || event === 'CompleteDomainError') {
    return state === 'Pending' || state === 'Retiring' ? `ok:${event}` : 'ok:Duplicate';
  }
  if (event === 'Deliver') return state === 'Ready' ? 'ok:Deliver' : `error:${state}`;
  const failures = { Cancel: 'Cancelled', Trap: 'Trap', Deadline: 'Deadline', Budget: 'Budget', InternalFailure: 'Internal' };
  if (failures[event]) return live ? `ok:Retire:${failures[event]}` : 'ok:Duplicate';
  if (event === 'NativeStopped') return state === 'Retired' ? 'ok:Duplicate' : 'ok:ObserveStop';
  if (event === 'SettlePins') return state === 'Retired' ? 'error:Retired' : 'ok:SettlePins';
  if (event === 'Cleanup') return state === 'Pending' || state === 'Retired' ? `error:${state}` : 'ok:Cleanup';
  if (event === 'Wake' || event === 'TakeWake') return live ? `ok:${event}` : `error:${state}`;
  if (event === 'Finish') return live ? `error:${state}` : state === 'Retired' ? 'ok:Duplicate' : 'ok:Finish';
  fail('M6-PAIR', `invented event ${event}`);
}

export function asyncPacket(accounts, sources, packet) {
  const scopes = groups.map(group => {
    const lane = lanes.find(lane => lane.id === group.lane);
    const prefix = `${lane.crate}::${group.module}::`;
    const source = `crates/${lane.package}/src/${group.module}`;
    const sourceFiles = Object.fromEntries(Object.entries(sources).filter(([file]) =>
      file === `${source}.rs` || file.startsWith(`${source}/`) && file.endsWith('.rs') || group.assets?.includes(file)));
    if (!sourceFiles[`${source}.rs`] && !sourceFiles[`${source}/mod.rs`]) fail('M6-SOURCE', `missing ${group.id} source`);
    for (const file of group.assets ?? []) if (!sourceFiles[file]) fail('M6-SOURCE', `missing async ABI asset ${file}`);
    for (const [file, digest] of Object.entries(sourceFiles)) {
      equal(accounts[lane.id].collected_files[file], digest, 'M6-SOURCE', `uncollected source: ${file}`);
    }
    const declarations = Object.entries(accounts[lane.id].translation).flatMap(([kind, rows]) =>
      rows.filter(row => row.is_local && row.rust_name.startsWith(prefix)).map(row => {
        const expected = kind === 'types' || kind === 'trait_decls' ? 'Types.lean' : 'Funs.lean';
        if (row.is_opaque || row.lean_file !== expected) fail('M6-BODY', `not an actual translation: ${row.rust_name}`);
        const declaration = leanName(lane, row);
        const owner = `${lane.module}.${expected.slice(0, -5)}`;
        if (!packet.declarations.some(item => item.name === declaration && item.owner === owner && item.local)) {
          fail('M6-BODY', `missing canonical audit subject: ${declaration}`);
        }
        return { declaration, owner, lane: lane.id, kind, def_id: row.def_id,
          rust: row.rust_name, source: row.source, loop: row.loop ?? null };
      }));
    if (!declarations.some(row => row.kind === 'functions')) fail('M6-BODY', `empty ${group.id} body inventory`);
    return { ...group, source_files: sourceFiles, declarations };
  });
  const roots = requiredRoots.map(required => {
    const scope = scopes.find(scope => scope.id === required.group);
    const kind = required.kind ?? 'functions';
    const matches = scope.declarations.filter(row => row.kind === kind && !row.loop &&
      (required.rust ? row.rust === required.rust :
        new RegExp(`::${required.type}(?:<[^{}]*>)?}::${required.method}$`).test(row.rust)));
    if (matches.length !== 1) fail('M6-ROOT', `${JSON.stringify(required)}: found ${matches.length}`);
    const row = matches[0];
    const dependencies = [];
    if (row.declaration === tableDecide) dependencies.push(transition);
    if (row.declaration === transition) dependencies.push(classify);
    if (row.declaration === validateCoverage) dependencies.push(validateRows, checkRow, classify, representative,
      `${kernel}.schema.STATE_SCHEMA`, `${kernel}.schema.EVENT_SCHEMA`);
    if (row.declaration === validateRows) dependencies.push(checkRow, classify, representative);
    if (row.declaration === checkRow) dependencies.push(classify, representative);
    if (required.group === 'async-tasks' && required.type === 'Table' && tableTransitions.includes(required.method)) {
      dependencies.push(tableDecide, transition, classify);
    }
    const requiresAcceptance = required.method === 'prepare_export' || required.rust === 'noble_wasm::component::compile';
    if (requiresAcceptance) dependencies.push('noble_kernel.acceptance.check');
    return { ...required, lane: row.lane, kind, rust: row.rust, declaration: row.declaration, owner: row.owner,
      requires_acceptance: requiresAcceptance, required_dependencies: dependencies };
  });
  for (const declaration of implementationRoots) {
    if (!roots.some(row => row.declaration === declaration && row.owner === 'NobleKernel.Funs')) {
      fail('M6-ROOT', `missing actual async implementation ${declaration}`);
    }
  }
  return { schema: 'noble-m6-extraction-subjects/v1', scopes, roots, strict_roots: strictRoots,
    reference_roots: referenceRoots, qualified_models: qualifiedModels, domain: sourceSchema(accounts, sources),
    pairs: states.flatMap(state => events.map(event => ({ state, event, rule: pairRule(state, event) }))),
    newly_required_models: packet.boundaries.filter(row => row.classification === 'newly-required-model'),
    scope: 'Complete async task/component extraction inventory and strict actual Rust async correspondence; M4/M5 obligations remain independent and mandatory.' };
}

export function asyncCoverage(packet, audit) {
  const views = lane => lane === 'kernel' ? [audit.contracts, audit.wasm] : [audit[lane]];
  const declaration = (name, owner, lane) => {
    const rows = views(lane).map(view => view.project_declarations.find(row => row.declaration === name));
    if (rows.some(row => !row || owner && row.owner !== owner)) fail('M6-COMPILED', `missing/substituted declaration: ${name}`);
    for (const row of rows.slice(1)) equal([row.owner, row.kind, row.axioms], [rows[0].owner, rows[0].kind, rows[0].axioms],
      'M6-COMPILED', `inconsistent shared kernel audit: ${name}`);
    return rows[0];
  };
  const roots = packet.roots.map(root => {
    const rows = views(root.lane).map(view => view.root_audits.find(row => row.declaration === root.declaration));
    if (rows.some(row => !row || row.owner !== root.owner)) fail('M6-COMPILED', `missing root audit: ${root.declaration}`);
    for (const row of rows.slice(1)) equal(row, rows[0], 'M6-COMPILED', `inconsistent root: ${root.declaration}`);
    for (const required of root.required_dependencies) {
      if (!rows[0].reachable_project_declarations.includes(required)) fail('M6-DEPENDENCY', `${root.declaration} detached from ${required}`);
    }
    return { ...root, ...rows[0] };
  });
  const scopes = packet.scopes.map(scope => ({ ...scope, declarations: scope.declarations.map(row => {
    const compiled = declaration(row.declaration, row.owner, row.lane);
    if (row.kind === 'functions' && compiled.kind !== 'definition') fail('M6-COMPILED', `not a body: ${row.declaration}`);
    return { ...row, compiled_kind: compiled.kind, axioms: compiled.axioms,
      reachable_from_roots: roots.filter(root => root.reachable_project_declarations.includes(row.declaration))
        .map(root => root.declaration) };
  }) }));
  return { schema: 'noble-m6-compiled-coverage/v1', scopes, roots, domain: packet.domain,
    newly_required_models: packet.newly_required_models.map(row => {
      const compiled = declaration(row.name, null, row.lane);
      return { ...row, owner: compiled.owner, compiled_kind: compiled.kind, axioms: compiled.axioms,
        reachable_from_roots: roots.filter(root => root.reachable_project_declarations.includes(row.name)).map(root => root.declaration) };
    }), scope: packet.scope };
}

export function asyncAuditPacket(subjects, binding) {
  return { schema: 'noble-m6-async-audit/v1', binding, strict_roots: subjects.strict_roots,
    reference_roots: subjects.reference_roots, qualified_models: subjects.qualified_models,
    domain: subjects.domain, pairs: subjects.pairs,
    implementation_roots: implementationRoots.map(declaration => subjects.roots.find(root => root.declaration === declaration)),
    coverage_roots: [validateCoverage, validateRows, checkRow].map(declaration => subjects.roots.find(root => root.declaration === declaration)),
    declarations: subjects.scopes.filter(scope => scope.lane === 'kernel').flatMap(scope => scope.declarations) };
}

export function asyncAudit({ packet, run, proofRoot, lake, artifacts, save }) {
  const inputFile = path.join(artifacts, 'm6-audit-input.json');
  save('m6-audit-input.json', packet);
  const inputBytes = fs.readFileSync(inputFile);
  const source = 'M6ExtractionGate.lean';
  fs.writeFileSync(path.join(proofRoot, source), 'import M6Audit\n\ncheck_m6_async\n', { flag: 'wx' });
  run('m6-async-build', lake, ['build', 'M6Audit'], proofRoot);
  const observed = run('m6-async-audit', lake, ['env', 'lean', source], proofRoot,
    { NOBLE_M6_EXTRACTION_SUBJECTS: inputFile });
  const records = observed.output.split('\n').filter(line => line.startsWith('M6-ASYNC '))
    .map(line => JSON.parse(line.slice('M6-ASYNC '.length)));
  if (records.length !== 1 || records[0].schema !== packet.schema || records[0].result !== 'passed') {
    fail('M6-VERDICT', 'missing compiled strict actual-Rust async audit');
  }
  const audit = records[0];
  equal(audit.packet, packet, 'M6-VERDICT', 'incomplete byte-bound proof input echo');
  for (const key of ['strict_roots', 'implementation_roots', 'coverage_roots', 'declarations']) {
    equal(audit[key].map(row => row.declaration), packet[key].map(row => row.declaration), 'M6-VERDICT', `missing ${key}`);
  }
  equal(audit.reference_roots.map(row => row.declaration), packet.reference_roots, 'M6-VERDICT', 'missing independent total reference');
  equal(audit.qualified_models.map(row => row.declaration), packet.qualified_models, 'M6-VERDICT', 'missing exact invariant qualifier definitions');
  equal(audit.constructor_coverage.pairs, packet.pairs, 'M6-VERDICT', 'incomplete compiled 75-pair classifier evidence');
  if (!fs.readFileSync(inputFile).equals(inputBytes)) fail('M6-PACKET', 'proof input changed during audit');
  const evidence = { packet_sha256: sha(inputBytes), audit };
  save('m6-async-refinement.json', evidence);
  return evidence;
}

export function asyncRefusals({ raw, accounts, sources, packet, subjects, coverage, proofPacket,
  run, proofRoot, lake, artifacts, save }) {
  const results = [];
  function reject(label, code, action) {
    try { action(); } catch (error) {
      if (!String(error).includes(`${code}:`)) fail('M6-REFUSAL', `${label}: wrong refusal: ${error}`);
      results.push({ label, expected: code, result: 'refused', diagnostic: String(error),
        kind: 'mutated-source-bound-extraction-inventory; not runtime-conformance' });
      return;
    }
    fail('M6-REFUSAL', `${label}: mutation accepted`);
  }
  for (const scope of subjects.scopes) {
    const lane = lanes.find(lane => lane.id === scope.lane);
    const bodies = new Map();
    for (const row of scope.declarations.filter(row => row.kind === 'functions' && !row.loop)) {
      if (!bodies.has(row.source.file) || subjects.roots.some(root => root.declaration === row.declaration)) bodies.set(row.source.file, row);
    }
    for (const [file, body] of bodies) {
      const label = file.replaceAll('/', '-').replaceAll('.', '-');
      reject(`omitted-${label}`, 'M4-OMITTED-BODY', () => {
        const translation = structuredClone(raw[lane.id].translation);
        translation.functions = translation.functions.filter(row => row.def_id !== body.def_id);
        account(lane, raw[lane.id].llbc, translation, sources);
      });
      reject(`opaque-${label}`, 'M4-LOCAL-OPAQUE', () => {
        const llbc = structuredClone(raw[lane.id].llbc);
        const translation = structuredClone(raw[lane.id].translation);
        llbc.translated.fun_decls[body.def_id].body = 'Opaque';
        translation.functions = translation.functions.filter(row => row.def_id !== body.def_id || !row.loop);
        for (const row of translation.functions.filter(row => row.def_id === body.def_id)) {
          row.is_opaque = true; row.lean_file = 'FunsExternal_Template.lean';
        }
        account(lane, llbc, translation, sources);
      });
    }
  }
  for (const declaration of [transition, tableDecide, classify, validateCoverage, validateRows, checkRow]) {
    reject(`missing-${declaration}`, 'M4-M6-ROOT', () => {
      const changed = structuredClone(accounts);
      changed.kernel.translation.functions = changed.kernel.translation.functions.filter(row =>
        leanName(lanes.find(lane => lane.id === 'kernel'), row) !== declaration);
      asyncPacket(changed, sources, packet);
    });
  }
  for (const root of subjects.roots.filter(row => row.required_dependencies.length)) {
    for (const dependency of root.required_dependencies) {
      reject(`detached-${root.declaration}-from-${dependency}`, 'M4-M6-DEPENDENCY', () => {
        const changed = structuredClone(coverage);
        for (const lane of root.lane === 'kernel' ? ['contracts', 'wasm'] : [root.lane]) {
          for (const record of changed[lane].root_audits.filter(row => row.declaration === root.declaration)) {
            record.reachable_project_declarations = record.reachable_project_declarations.filter(name => name !== dependency);
          }
        }
        asyncCoverage(subjects, changed);
      });
    }
  }
  function lean(label, code, prelude = '', mutate = () => {}) {
    const input = structuredClone(proofPacket);
    mutate(input);
    const inputFile = path.join(artifacts, `m6-refusal-${label}.json`);
    const bytes = JSON.stringify(input, null, 2) + '\n';
    fs.writeFileSync(inputFile, bytes, { flag: 'wx' });
    const source = `M6Refusal_${label.replaceAll('-', '_')}.lean`;
    fs.writeFileSync(path.join(proofRoot, source), `import M6Audit\n\n${prelude}\n\ncheck_m6_async\n`, { flag: 'wx' });
    const observed = run(`m6-refusal-${label}`, lake, ['env', 'lean', source], proofRoot,
      { NOBLE_M6_EXTRACTION_SUBJECTS: inputFile }, true);
    if (observed.exit === 0 || observed.signal || observed.error || !observed.output.includes(`${code}:`)) {
      fail('M6-REFUSAL', `${label}: not the required compiled proof-audit refusal`);
    }
    equal(fs.readFileSync(inputFile, 'utf8'), bytes, 'M6-PACKET', 'control input changed during audit');
    results.push({ label, expected: code, result: 'refused', kind: 'executed-Lean-audit-control; not runtime-conformance',
      input_sha256: sha(bytes), log: observed.log, output_sha256: observed.output_sha256 });
    save('m6-refusals.json', results);
  }
  lean('missing-inventory-binding', 'M6-PACKET', '', input => { delete input.binding.inventory_sha256; });
  lean('missing-schema-source-binding', 'M6-SCHEMA', '', input => { delete input.domain.source_files[schemaFiles[0]]; });
  lean('stale-state-schema', 'M6-SCHEMA', '', input => { input.domain.state_schema += '@stale'; });
  lean('stale-event-payload-field', 'M6-SCHEMA', '', input => {
    input.domain.event_schema = input.domain.event_schema.replace('pins:u64', 'pins:u32');
  });
  lean('missing-event-payload-field', 'M6-SCHEMA', '', input => {
    input.domain.event_schema = input.domain.event_schema.replace(',bytes:usize', '');
  });
  lean('invented-state-constructor', 'M6-SCHEMA', '', input => { input.domain.states[0] = 'InventedState'; });
  lean('invented-event-constructor', 'M6-SCHEMA', '', input => { input.domain.events[0] = 'InventedEvent'; });
  lean('missing-pair', 'M6-PAIR', '', input => { input.pairs.pop(); });
  lean('duplicate-pair', 'M6-PAIR', '', input => { input.pairs[1] = input.pairs[0]; });
  lean('invented-pair', 'M6-PAIR', '', input => { input.pairs[0].event = 'InventedEvent'; });
  lean('invented-disposition', 'M6-PAIR', '', input => { input.pairs[0].rule = 'ok:InventedRule'; });
  lean('invalid-pair-disposition', 'M6-PAIR', '', input => {
    input.pairs.find(row => row.state === 'Pending' && row.event === 'Deliver').rule = 'ok:Deliver';
  });
  lean('missing-strict-root', 'M6-ROOT', '', input => { input.strict_roots.pop(); });
  lean('substituted-strict-root', 'M6-ROOT', 'theorem M6Async.m6_substitute : True := by trivial',
    input => { input.strict_roots[0].declaration = 'M6Async.m6_substitute'; });
  lean('missing-implementation-root', 'M6-ROOT', '', input => { input.implementation_roots.pop(); });
  lean('missing-coverage-root', 'M6-ROOT', '', input => { input.coverage_roots.pop(); });
  lean('detached-transition-wrapper', 'M6-DEPENDENCY',
    'def M6Async.m6_detached := M6Async.step\n' +
    `check_m6_dependency_control "M6Async.m6_detached" "${transition}"`);
  lean('implementation-backed-reference', 'M6-INDEPENDENCE',
    `def M6Async.m6_reference_wrapper := ${transition}\n` +
    'check_m6_reference_control "M6Async.m6_reference_wrapper"');
  const proofPrelude = 'open Aeneas Aeneas.Std noble_kernel noble_kernel.async_tasks.domain\n';
  lean('finite-event-correspondence', 'M6-TYPE', proofPrelude +
    'theorem M6Async.m6_finite (record : Snapshot) (claim : Handle) :\n' +
    '  async_tasks.transition.transition record claim .Inspect = M6Async.step record claim .Inspect :=\n' +
    '  M6Async.transition_refines record claim .Inspect\n' +
    'check_m6_contract_control "M6Async.m6_finite" "M6Async.transition_refines"');
  lean('vacuous-extra-precondition', 'M6-TYPE', proofPrelude +
    'theorem M6Async.m6_vacuous (record : Snapshot) (claim : Handle) (event : Event) (_ : False) :\n' +
    '  async_tasks.transition.transition record claim event = M6Async.step record claim event :=\n' +
    '  M6Async.transition_refines record claim event\n' +
    'check_m6_contract_control "M6Async.m6_vacuous" "M6Async.transition_refines"');
  lean('weakened-correspondence', 'M6-TYPE', proofPrelude +
    'theorem M6Async.m6_weakened (record : Snapshot) (claim : Handle) (event : Event) :\n' +
    '  async_tasks.transition.transition record claim event = M6Async.step record claim event ∨ True := Or.inr trivial\n' +
    'check_m6_contract_control "M6Async.m6_weakened" "M6Async.transition_refines"');
  lean('vacuous-invariant-guard', 'M6-MODEL', proofPrelude +
    'def M6Async.m6_vacuous_guard (_ : Snapshot) : Prop := False\n' +
    'check_m6_model_control "M6Async.m6_vacuous_guard" "M6Async.WellFormed"');
  lean('weakened-settlement-condition', 'M6-MODEL', proofPrelude +
    'def M6Async.m6_unsettled (_ : Snapshot) : Bool := true\n' +
    'check_m6_model_control "M6Async.m6_unsettled" "M6Async.settled"');
  lean('substituted-production-body', 'M6-OWNER', 'def noble_kernel.async_tasks.m6_substitute : Bool := true', input => {
    input.declarations.push({ ...input.declarations.find(row => row.kind === 'functions'), declaration: `${kernel}.m6_substitute` });
  });
  for (const declaration of [transition, tableDecide, classify, validateCoverage, validateRows, checkRow]) {
    lean(`omitted-${declaration.split('.').slice(2).join('-')}`, 'M6-COVERAGE', '', input => {
      input.declarations = input.declarations.filter(row => row.declaration !== declaration);
    });
  }
  lean('unused-proof-axiom', 'M6-AXIOM', 'axiom M6Async.m6_axiom : Bool');
  lean('unused-proof-hole', 'M6-AXIOM', 'theorem M6Async.m6_hole : True := by sorry');
  lean('native-async-proof', 'M6-AXIOM', 'theorem M6Async.m6_native : (17 : Nat) + 1 = 18 := by native_decide');
  lean('opaque-async-model', 'M6-TRANSPARENT', 'opaque M6Async.m6_opaque : Bool := true');
  save('m6-refusals.json', results);
  return results;
}
