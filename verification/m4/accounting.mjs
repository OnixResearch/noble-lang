// Pure accounting shared by the real gate and its adversarial controls.
// Charon is the body inventory; Aeneas is the translation inventory. Neither a
// handwritten Lean model nor a test function can satisfy a local-body entry.
import path from 'node:path';
import crypto from 'node:crypto';

export const schema = 'm4-extraction-lock/v1';
export const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
export const canonical = value => JSON.stringify(value, (_, item) =>
  item && typeof item === 'object' && !Array.isArray(item)
    ? Object.fromEntries(Object.keys(item).sort().map(key => [key, item[key]])) : item);
export function fail(code, detail) { throw Error(`M4-${code}: ${detail}`); }
export function equal(actual, expected, code, detail) {
  if (canonical(actual) !== canonical(expected)) fail(code, detail);
}
export function exactBytes(actual, expected, detail) {
  if (!Buffer.from(actual).equals(Buffer.from(expected))) fail('GENERATED', detail);
}

export const lanes = [
  { id: 'kernel', crate: 'noble_kernel', package: 'noble-kernel', basename: 'noble_kernel',
    module: 'NobleKernel', checked: 'proofs/m3/NobleKernel' },
  { id: 'contracts', crate: 'noble_contracts', package: 'noble-contracts', basename: 'noble_contract_impl',
    module: 'NobleContractImpl', checked: 'proofs/mc1/NobleContractImpl' },
  { id: 'wasm', crate: 'noble_wasm', package: 'noble-wasm', basename: 'noble_wasm_impl',
    module: 'NobleWasmImpl', checked: 'proofs/m3-wasm/NobleWasmImpl' },
];
export const inheritedOpaque = [
  'noble_kernel::shapes::impls::clone_parts',
  'noble_kernel::shapes::impls::debug_parts',
  'noble_kernel::shapes::impls::debug_slots',
  'noble_kernel::types::impls::clone_stack',
  'noble_kernel::types::impls::debug_stack',
];
// These source files contain the five inherited opaque helpers. Changing them
// requires a separately reviewed model renewal, not widening this allowance.
export const inheritedOpaqueSources = {
  'crates/noble-kernel/src/types/impls.rs': 'e5c1107123339ca3233a0b4a01a285ebe155a2a582040d83840645f0aab323ca',
  'crates/noble-kernel/src/shapes/impls.rs': 'b8b1a2df2a236b9703eaeb6df383ea183b31e137ca8d86240608cda260930510',
};
const kinds = { functions: 'fun_decls', types: 'type_decls', globals: 'global_decls',
  trait_decls: 'trait_decls', trait_impls: 'trait_impls' };

export function sourcePolicy(sourceFiles, readSource) {
  for (const [file, expected] of Object.entries(inheritedOpaqueSources)) {
    equal(sourceFiles[file], expected, 'OPAQUE-SOURCE', file);
  }
  for (const file of Object.keys(sourceFiles).filter(file =>
    lanes.some(lane => file.startsWith(`crates/${lane.package}/src/`)) && file.endsWith('.rs'))) {
    const text = readSource(file);
    for (const match of text.matchAll(/charon\s*::\s*([A-Za-z_][A-Za-z_0-9]*)/g)) {
      if (match[1] !== 'variants_suffix' && !(match[1] === 'opaque' && inheritedOpaqueSources[file])) {
        fail('SOURCE-FILTER', `${file}: unreviewed Charon annotation ${match[1]}`);
      }
    }
    // Production-only extraction may not silently hide a new authored body in
    // a cfg branch. Existing test-only Eq impls and derives remain unchanged.
    for (const match of text.matchAll(/#\[\s*(cfg(?:_attr)?)\s*\(([^\n]*)/g)) {
      const allowed = match[1] === 'cfg' && inheritedOpaqueSources[file] && match[2] === 'test)]' ||
        match[1] === 'cfg_attr' && ['crates/noble-kernel/src/words.rs',
          'crates/noble-kernel/src/untrusted.rs'].includes(file) && match[2] === 'test, derive(PartialEq, Eq))]';
      if (!allowed) fail('SOURCE-FILTER', `${file}: unreviewed conditional source selection`);
    }
  }
}

function sourceName(file) {
  if (!file || typeof file.name?.Local !== 'string') fail('LLBC-SOURCE', 'local source name missing');
  const name = path.posix.normalize(file.name.Local);
  if (path.posix.isAbsolute(name) || name.startsWith('../')) fail('LLBC-SOURCE', name);
  return name;
}
function uniqueNames(rows, label) {
  const names = rows.map(row => row.lean_name);
  if (names.some(name => typeof name !== 'string' || !name) || new Set(names).size !== names.length) {
    fail('INVENTORY', `missing or duplicate Lean name: ${label}`);
  }
}
function isDropGlue(decl, crate) {
  const implementation = decl.src?.TraitImpl?.impl_ref?.id;
  return Number.isInteger(implementation) && crate.trait_impls[implementation]?.src === 'Destruct' &&
    decl.item_meta.name.some(part => part.Builtin?.[0] === 'DropGlue');
}
function isVTable(decl) {
  return decl?.src?.VTableInstance && decl.item_meta.name.some(part => part.Builtin?.[0] === 'VTable');
}
function directRustName(decl) {
  return decl.item_meta.name.every(part => part.Ident)
    ? decl.item_meta.name.map(part => part.Ident[0]).join('::') : null;
}

function compilerSpans(crate) {
  const spans = new Map();
  const pending = [crate];
  while (pending.length) {
    const value = pending.pop();
    if (!value || typeof value !== 'object') continue;
    if (Array.isArray(value.Value) && Number.isInteger(value.Value[0]) &&
        Number.isInteger(value.Value[1]?.data?.file_id)) {
      spans.set(value.Value[0], value.Value[1]);
    }
    pending.push(...Object.values(value));
  }
  return spans;
}

function loopOrigin(item, declaration, translation, crate, spans) {
  const parent = translation.functions.find(row => row.def_id === item.def_id &&
    row.is_local && !row.loop && row.lean_name === item.parent_lean_name);
  if (!parent) fail('OMITTED-BODY', `loop helper has no translated parent: ${item.lean_name}`);
  if (parent.rust_name !== item.rust_name || item.is_opaque ||
      !Number.isInteger(item.loop.id) || item.loop.id < 0 ||
      typeof item.loop.is_body !== 'boolean' || !Array.isArray(item.loop.pos) ||
      item.loop.pos.some(position => !Number.isInteger(position) || position < 0)) {
    fail('LOCAL-OWNER', `invalid generated loop identity: ${item.lean_name}`);
  }
  // A helper may point into matches!/other standard macros. Its compiler ID
  // still owns a real local body, and the reported span must occur in that
  // body's compiler tree; an arbitrary external source origin is not accepted.
  const pending = [declaration.body];
  while (pending.length) {
    const value = pending.pop();
    if (!value || typeof value !== 'object') continue;
    const span = value.span;
    const data = (span?.Untagged ?? span?.Value?.[1] ?? spans.get(span?.Deduplicated))?.data;
    if (data && crate.files[data.file_id]?.name?.Local === item.source.file &&
        data.beg.line === item.source.begin_line && data.end.line === item.source.end_line) return;
    pending.push(...Object.values(value));
  }
  fail('LOCAL-OWNER', `loop origin is absent from its compiler body: ${item.lean_name}`);
}

export function account(lane, llbc, translation, sources) {
  if (llbc.has_errors !== false || llbc.charon_version !== '0.1.254') fail('LLBC', `${lane.id}: failed/unreviewed Charon output`);
  const crate = llbc.translated;
  if (crate?.crate_name !== lane.crate || translation.crate !== lane.crate ||
      translation.aeneas_version !== '505b6ca' || translation.charon_version !== '0.1.254') {
    fail('TRANSLATOR', lane.id);
  }
  for (const option of ['start_from', 'start_from_if_exists', 'start_from_attribute', 'include', 'exclude', 'opaque', 'rustc_args', 'targets']) {
    equal(crate.options?.[option], [], 'EXTRACTION-SCOPE', `${lane.id}: ${option}`);
  }
  for (const option of ['start_from_pub', 'monomorphize', 'extract_opaque_bodies', 'skip_borrowck', 'no_typecheck', 'no_serialize']) {
    equal(crate.options?.[option], false, 'EXTRACTION-SCOPE', `${lane.id}: ${option}`);
  }
  if (crate.options?.preset !== 'Aeneas' || crate.options?.error_on_warnings !== true) fail('EXTRACTION-SCOPE', lane.id);
  const targets = crate.target_information;
  if (targets?.length !== 1 || targets[0].key !== 'x86_64-unknown-linux-gnu' ||
      targets[0].value?.target_pointer_size !== 8 || targets[0].value?.is_little_endian !== true) fail('TARGET', lane.id);
  if (!Array.isArray(crate.files)) fail('LLBC-SOURCE', `${lane.id}: no source inventory`);
  const collectedFiles = {};
  for (const file of crate.files.filter(file => file?.crate_name === lane.crate)) {
    const name = sourceName(file);
    if (typeof file.contents !== 'string') fail('LLBC-SOURCE', `${lane.id}: ${name}`);
    const digest = sha(file.contents);
    // Distinct include_str! paths may normalize to one file. Bind every
    // occurrence to the same frozen bytes rather than skipping duplicates.
    equal(digest, sources[name], 'LLBC-SOURCE', `${lane.id}: source contents differ: ${name}`);
    collectedFiles[name] = digest;
  }
  for (const file of Object.keys(sources).filter(file => file.startsWith(`crates/${lane.package}/src/`) && file.endsWith('.rs'))) {
    if (!collectedFiles[file]) fail('SOURCE-COVERAGE', `${lane.id}: uncollected production source ${file}`);
  }
  const inventories = {};
  const collector = {};
  const spans = compilerSpans(crate);
  for (const [kind, field] of Object.entries(kinds)) {
    if (!Array.isArray(translation[kind]) || !Array.isArray(crate[field])) fail('INVENTORY', `${lane.id}: ${kind}`);
    uniqueNames(translation[kind], `${lane.id}/${kind}`);
    const local = crate[field].filter(item => item?.item_meta?.is_local);
    const rows = translation[kind].map(item => {
      if (!Number.isInteger(item.def_id) || typeof item.is_local !== 'boolean' || !item.source?.file ||
          typeof item.rust_name !== 'string' || typeof item.lean_file !== 'string') fail('INVENTORY', `${lane.id}: malformed ${kind}`);
      const declaration = crate[field][item.def_id];
      if (!declaration || declaration.def_id !== item.def_id || declaration.item_meta.is_local !== item.is_local) {
        fail('INVENTORY', `${lane.id}: forged ${kind} ID ${item.def_id}`);
      }
      const source = crate.files[declaration.item_meta.span?.Untagged?.data?.file_id];
      if (!source) fail('INVENTORY', `${lane.id}: missing span for ${item.rust_name}`);
      if (item.is_local) {
        const name = sourceName(source);
        if (!name.startsWith(`crates/${lane.package}/src/`)) {
          fail('LOCAL-OWNER', `${lane.id}: ${item.rust_name} is not a production body`);
        }
        if (kind === 'functions' && item.loop) loopOrigin(item, declaration, translation, crate, spans);
        else if (item.source.file !== name) fail('LOCAL-OWNER', `${lane.id}: substituted source for ${item.rust_name}`);
        if (kind === 'functions') {
          if (typeof item.is_opaque !== 'boolean') fail('OPACITY', item.rust_name);
          if (item.is_opaque !== (declaration.body === 'Opaque')) fail('OPACITY', item.rust_name);
          if (declaration.signature?.is_unsafe !== false) fail('UNSAFE', item.rust_name);
        }
        const expectedFile = kind === 'types' || kind === 'trait_decls' ? 'Types.lean' : 'Funs.lean';
        if (!(kind === 'functions' && item.is_opaque) && item.lean_file !== expectedFile) {
          fail('LOCAL-OWNER', `${item.rust_name}: ${item.lean_file}`);
        }
      }
      return { ...item, source: { ...item.source, file: path.posix.normalize(item.source.file) } };
    }).sort((a, b) => a.lean_name.localeCompare(b.lean_name, 'en'));
    collector[kind] = local.map(decl => {
      let translated = rows.filter(row => row.is_local && row.def_id === decl.def_id);
      let disposition = 'translated';
      if (kind === 'functions' && decl.src?.GlobalInitializer) {
        const global = crate.global_decls[decl.src.GlobalInitializer.id];
        translated = translation.globals.filter(row => row.is_local && row.def_id === decl.src.GlobalInitializer.id);
        disposition = isVTable(global) && decl.item_meta.name.some(part => part.Builtin?.[0] === 'VTable')
          ? 'compiler-generated-vtable-not-translated' : 'global-initializer-translated-with-global';
      } else if (kind === 'functions' && isDropGlue(decl, crate) || kind === 'trait_impls' && decl.src === 'Destruct') {
        // Compiler-generated destructor scaffolding is absent from the selected
        // no-drop semantics. It is accounted explicitly, never an authored-body exemption.
        disposition = 'compiler-generated-destruct-not-translated';
      } else if (kind === 'globals' && isVTable(decl)) {
        disposition = 'compiler-generated-vtable-not-translated';
      } else if (kind === 'types' && decl.kind?.Alias && !translated.length) {
        // Aliases have no independent body; the translator expands their type.
        disposition = 'type-alias-expanded';
      }
      if (!translated.length && !['compiler-generated-destruct-not-translated',
        'compiler-generated-vtable-not-translated', 'type-alias-expanded'].includes(disposition)) {
        fail('OMITTED-BODY', `${lane.id}: ${kind} ${decl.def_id} ${directRustName(decl) ?? ''}`);
      }
      if (kind === 'functions' && disposition === 'translated' && !translated.some(row =>
        row.lean_name.endsWith(`.${row.rust_name.split('::').at(-1)}`) && !row.lean_name.endsWith('_loop.body'))) {
        fail('OMITTED-BODY', `${lane.id}: only loop helpers remain for function ${decl.def_id}`);
      }
      if (kind === 'functions' && decl.body === 'Opaque' &&
          !['compiler-generated-vtable-not-translated', 'compiler-generated-destruct-not-translated'].includes(disposition)) {
        const names = translated.map(row => row.rust_name);
        if (!names.length || names.some(name => !inheritedOpaque.includes(name))) fail('LOCAL-OPAQUE', names.join(','));
        disposition = 'inherited-opaque-model';
      }
      return { def_id: decl.def_id, source_name: decl.item_meta.name, source_span: decl.item_meta.span,
        disposition, expanded_alias: disposition === 'type-alias-expanded' ? decl.kind.Alias : null,
        lean_names: translated.map(row => row.lean_name).sort() };
    }).sort((a, b) => a.def_id - b.def_id);
    inventories[kind] = rows;
  }
  equal(inventories.functions.filter(item => item.is_local && item.is_opaque).map(item => item.rust_name).sort(),
    lane.id === 'kernel' ? inheritedOpaque : [], 'LOCAL-OPAQUE', lane.id);
  for (const kind of ['trait_decls', 'trait_impls']) {
    if (inventories[kind].some(item => item.lean_file.includes('External'))) fail('EXTERNAL-KIND', `${lane.id}/${kind}`);
  }
  if (!collector.functions.length || !inventories.functions.some(item => item.is_local && !item.is_opaque)) fail('INVENTORY', `${lane.id}: empty`);
  return { crate: lane.crate, collected_files: collectedFiles, collector, translation: inventories };
}

const sourceRoot = (lane, type, method) => ({ lane, kind: 'functions', type, method, source: true });
export const requiredRoots = [
  { lane: 'kernel', kind: 'functions', rust: 'noble_kernel::acceptance::check' },
  ...['TextLiteral', 'Body', 'Definition', 'Submission'].map(type =>
    ({ lane: 'kernel', kind: 'types', rust: `noble_kernel::execution::${type}` })),
  { lane: 'contracts', kind: 'functions', rust: 'noble_contracts::prepare' },
  ...['new', 'prepare', 'commit'].map(method => sourceRoot('contracts', 'Session', method)),
  ...['submission', 'output', 'is_definition'].map(method => sourceRoot('contracts', 'Prepared', method)),
  ...['stage', 'diagnostic'].map(method => sourceRoot('contracts', 'Error', method)),
  { lane: 'wasm', kind: 'functions', rust: 'noble_wasm::compile' },
  ...['new', 'prepare', 'commit'].map(method => sourceRoot('wasm', 'Compiler', method)),
  sourceRoot('wasm', 'Prepared', 'wat'),
];
export function rootsFor(accounts) {
  return requiredRoots.map(required => {
    const lane = lanes.find(lane => lane.id === required.lane);
    const matches = accounts[lane.id].translation[required.kind].filter(item => item.is_local &&
      (!item.is_opaque) && (required.rust ? item.rust_name === required.rust :
        item.source.file.startsWith(`crates/${lane.package}/src/source`) &&
        item.rust_name.endsWith(`::${required.type}}::${required.method}`)) &&
      (required.kind !== 'functions' || item.lean_name.endsWith(`.${required.method ?? required.rust.split('::').at(-1)}`)));
    if (matches.length !== 1) fail('REQUIRED-ROOT', `${JSON.stringify(required)}: found ${matches.length}`);
    return { ...required, declaration: matches[0].lean_name,
      owner: `${lane.module}.${required.kind === 'types' ? 'Types' : 'Funs'}`,
      requires_acceptance: required.method === 'prepare' || required.rust === 'noble_wasm::compile' };
  });
}

export function auditPacket(accounts, retainedModels) {
  const roots = rootsFor(accounts);
  if (!Array.isArray(retainedModels) || !retainedModels.length) fail('BASELINE', 'missing historical boundary inventory');
  const inheritedNames = new Set(retainedModels.map(item => item.name));
  const declarations = [];
  const boundaries = [];
  const bridges = [];
  for (const lane of lanes) {
    const inventory = accounts[lane.id].translation;
    for (const [kind, rows] of Object.entries(inventory)) {
      for (const item of rows) {
        if (item.lean_file === 'Funs.lean' || item.lean_file === 'Types.lean') {
          const name = item.lean_name.startsWith(`${lane.crate}.`) ? item.lean_name : `${lane.crate}.${item.lean_name}`;
          declarations.push({ name, owner: `${lane.module}.${item.lean_file.slice(0, -5)}`,
            transparent: item.lean_file === 'Funs.lean', kind, local: item.is_local });
        } else if (item.lean_file.includes('External')) {
          const name = `${lane.crate}.${item.lean_name}`;
          boundaries.push({ name, lane: lane.id, kind, rust: item.rust_name, inherited_opaque: item.is_local,
            classification: inheritedNames.has(name) ? 'inherited-model' : 'newly-required-model' });
          const dependency = lanes.find(other => other.crate !== lane.crate && item.rust_name.startsWith(`${other.crate}::`));
          if (kind === 'globals' && !dependency) fail('EXTERNAL-KIND', `${lane.id}: unbound global ${item.rust_name}`);
          if (dependency && (kind === 'functions' || kind === 'globals')) {
            const actual = accounts[dependency.id].translation[kind].filter(other => other.is_local && !other.is_opaque &&
              other.lean_file === 'Funs.lean' &&
              other.rust_name === item.rust_name && other.lean_name === item.lean_name);
            if (actual.length !== 1) fail('BRIDGE-TARGET', `${name}: no unique actual extracted dependency`);
            bridges.push({ model: name, actual: actual[0].lean_name });
          }
        } else fail('INVENTORY', `${lane.id}: unsupported generated file ${item.lean_file}`);
      }
    }
  }
  const requested = new Set(boundaries.map(item => item.name));
  return { schema: 'm4-extraction-audit/v1', declarations, boundaries, bridges, roots,
    retained_models: retainedModels,
    newly_required_models: boundaries.filter(item => !inheritedNames.has(item.name)).map(item => item.name).sort(),
    retained_unrequested_models: retainedModels.filter(item => !requested.has(item.name)).map(item => item.name).sort() };
}

// The pinned discriminant elaborator creates colliding global instance names
// when both consumer extractions share one Lean environment. Audit each against
// the same kernel, without modifying generated bytes or omitting any subject.
export function auditPackets(packet) {
  const selectors = {
    declarations: row => lanes.find(lane =>
      [`${lane.module}.Types`, `${lane.module}.Funs`].includes(row.owner))?.id,
    boundaries: row => row.lane,
    bridges: row => lanes.find(lane => row.model.startsWith(`${lane.crate}.`))?.id,
    roots: row => row.lane,
    retained_models: row => row.lane,
  };
  const packets = Object.fromEntries(['contracts', 'wasm'].map(lane => [lane, {
    schema: packet.schema, lane,
    ...Object.fromEntries(Object.keys(selectors).map(field => [field, []])),
  }]));
  for (const [field, select] of Object.entries(selectors)) {
    for (const row of packet[field]) {
      const lane = select(row);
      if (!lanes.some(known => known.id === lane)) fail('PACKET', `unassigned ${field}: ${JSON.stringify(row)}`);
      for (const target of lane === 'kernel' ? ['contracts', 'wasm'] : [lane]) {
        packets[target][field].push(row);
      }
    }
  }
  return packets;
}
