// Pure accounting shared by the real gate and its adversarial controls.
// Charon is the body inventory; Aeneas is the translation inventory. Neither a
// handwritten Lean model nor a test function can satisfy a local-body entry.
import path from 'node:path';
import crypto from 'node:crypto';

export const schema = 'm4-extraction-lock/v1';
export const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
export const canonical = (value, normalize) => JSON.stringify(value, (_, item) => {
  if (!item || typeof item !== 'object' || Array.isArray(item)) return item;
  if (normalize) item = normalize(item);
  return Object.fromEntries(Object.keys(item).sort().map(key => [key, item[key]]));
});
export function fail(code, detail) { throw Error(`M4-${code}: ${detail}`); }
export function equal(actual, expected, code, detail) {
  if (canonical(actual) !== canonical(expected)) fail(code, detail);
}
export function exactBytes(actual, expected, detail) {
  if (!Buffer.from(actual).equals(Buffer.from(expected))) fail('GENERATED', detail);
}

// Independent discoveries of identical sources/tooling differed only in these
// seven macro-expansion span end columns: zero width versus the full macro token.
// Bind the exact source site and one occurrence per site; do not erase ordinary
// spans, the rest of these spans, any body, or any extraction option.
const syntheticExpansionSites = {
  noble_kernel: [
    { file: 'crates/noble-kernel/src/lib.rs', crate: 'noble_kernel', line: 24, end: 19,
      source: '        match $step {' },
    { file: '/rustc/library/core/src/macros/mod.rs', crate: 'core', line: 434, end: 25,
      source: null },
    { file: 'crates/noble-kernel/src/contracts/nominal/schemes.rs', crate: 'noble_kernel', line: 5, end: 24,
      source: '        match $candidate {' },
  ],
  noble_contracts: [
    { file: 'crates/noble-contracts/src/lib.rs', crate: 'noble_contracts', line: 9, end: 19,
      source: '        match $step {' },
    { file: '/rustc/library/core/src/macros/mod.rs', crate: 'core', line: 434, end: 25,
      source: null },
    { file: 'crates/noble-contracts/src/source/declared/state.rs', crate: 'noble_contracts', line: 7, end: 24,
      source: '        match $candidate {' },
  ],
  noble_wasm: [
    { file: 'crates/noble-wasm/src/lib.rs', crate: 'noble_wasm', line: 13, end: 19,
      source: '        match $step {' },
  ],
};

export function canonicalLlbcHash(llbc, normalizedSpans) {
  // Charon serializes this name map in hash-table order and records the physical
  // output directory. Preserve every other field, including all bodies/options.
  const translated = llbc.translated;
  const sites = syntheticExpansionSites[translated.crate_name];
  if (!sites) fail('LLBC-SPAN', `unreviewed crate ${translated.crate_name}`);
  const byFile = new Map();
  for (const site of sites) {
    const files = translated.files.filter(file => file?.name?.Local === site.file);
    if (files.length !== 1 || files[0].crate_name !== site.crate ||
        (site.source === null ? files[0].contents !== null :
          files[0].contents?.split('\n')[site.line - 1] !== site.source)) {
      fail('LLBC-SPAN', `unreviewed expansion source ${site.file}:${site.line}`);
    }
    byFile.set(files[0].id, { ...site, seen: 0 });
  }
  const normalizeSpan = item => {
    const value = item.Value;
    if (!Array.isArray(value) || value.length !== 2 || typeof value[0] !== 'number') return item;
    const span = value[1];
    const data = span?.data;
    const site = byFile.get(data?.file_id);
    if (!site || data.beg?.line !== site.line || data.beg?.col !== 8 ||
        data.end?.line !== site.line || span.generated_from_span !== null) return item;
    if (data.end.col !== 8 && data.end.col !== site.end) {
      fail('LLBC-SPAN', `unreviewed expansion end ${site.file}:${site.line}:${data.end.col}`);
    }
    if (++site.seen !== 1) fail('LLBC-SPAN', `duplicate expansion ${site.file}:${site.line}`);
    normalizedSpans?.push({ source: site.file, line: site.line, begin_col: 8,
      observed_end_col: data.end.col, canonical_end_col: site.end, field: 'span.Value[1].data.end.col' });
    return data.end.col === site.end ? item : { ...item, Value: [value[0], {
      ...span, data: { ...data, end: { ...data.end, col: site.end } },
    }] };
  };
  const names = translated.short_names.map(row => ({ key: canonical(row.key), row }));
  names.sort((a, b) => a.key < b.key ? -1 : a.key > b.key ? 1 : 0);
  for (let index = 1; index < names.length; index++) {
    if (names[index - 1].key === names[index].key) fail('LLBC-NAMES', 'duplicate short-name key');
  }
  const binding = canonical({ ...llbc, translated: { ...translated,
    options: { ...translated.options, dest_file: path.basename(translated.options.dest_file) },
    short_names: names.map(entry => entry.row) } }, normalizeSpan);
  for (const site of byFile.values()) {
    if (site.seen !== 1) fail('LLBC-SPAN', `missing expansion ${site.file}:${site.line}`);
  }
  return sha(binding);
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
// Reviewed renewal: the original complete-file hashes were
// types/impls.rs=f834b36f972feb518dfcdaf9c9f4f05c2ecc787a18f937a7f2e6d1a0898f0c3e
// shapes/impls.rs=c7aec68c0201b14d978d4d3b7b9906794176088e0b3b3ca8912090ad1a505203.
// Finite nominal Clone/Debug branches and one nominal equality arm were added.
// The latest renewal directly clones and formats ordered nominal payloads and
// enqueues their structural equality pairs rather than introducing mutual
// trait-implementation recursion through NominalShape's derived methods.
// Pattern::Nominal's new formatting arm writes the same default debug fields
// directly rather than creating a borrowed debug-tuple builder.
// All five inherited opaque helper implementations below retain their
// independently compared, separately pinned original body bytes.
// Cloning a finite stack still preserves order and invokes each element's
// structural Clone; nominal identity and shape are copied, not re-resolved.
// Formatting remains observational and propagates formatter failure; pretty
// layout and allocator success remain outside the extracted claim.
// Selected rustfmt and the 300-line cap renew the complete-file source pin;
// both inherited opaque helper bodies retain their exact reviewed hashes.
export const inheritedOpaqueSources = {
  'crates/noble-kernel/src/types/impls.rs': '27f120e09d1931e4c52919988316e2324e33bc9a1c707ca02478178c43d30f7d',
  'crates/noble-kernel/src/shapes/impls.rs': '47b18106144c0004abec92a0fb3ef3364797bee3b57e774e39e6a2b55970b30a',
};
export const inheritedOpaqueBodies = {
  'crates/noble-kernel/src/types/impls.rs': {
    clone_stack: '5ae77f5a6d771ef94aec049717bb2f6a8d350dcb258ac22e28ec604d58d3f6e7',
    debug_stack: '631e7abdd5d4a1210a72d195f021f3de8f5b99fb41e5e4def25b29073124f04e',
  },
  'crates/noble-kernel/src/shapes/impls.rs': {
    clone_parts: '934493f41ceb76c80f78b1800bf9a38b3c619d0ba0217c54db9e7e19142beb26',
    debug_parts: 'c93c255598269fbc11e74206efca41ac2729955b8c948ec5ae7a481f2e942e9d',
    debug_slots: '8295e5745e323de390d59d074d1e348c2345111badc8cf82d1adcb73c8bb9527',
  },
};

function opaqueBody(source, name) {
  const declaration = new RegExp(`(?:^|\\n)fn ${name}\\(`, 'g');
  const matches = [...source.matchAll(declaration)];
  if (matches.length !== 1) fail('OPAQUE-MODEL', `${name}: missing or duplicate helper`);
  const start = matches[0].index + (matches[0][0].startsWith('\n') ? 1 : 0);
  const opening = source.indexOf('{', start);
  if (opening < 0) fail('OPAQUE-MODEL', `${name}: missing function body`);
  let depth = 0;
  for (let at = opening; at < source.length; at++) {
    if (source[at] === '{') depth++;
    else if (source[at] === '}' && --depth === 0) return source.slice(start, at + 1);
  }
  fail('OPAQUE-MODEL', `${name}: unterminated function body`);
}
const kinds = { functions: 'fun_decls', types: 'type_decls', globals: 'global_decls',
  trait_decls: 'trait_decls', trait_impls: 'trait_impls' };

export function sourcePolicy(sourceFiles, readSource) {
  for (const [file, expected] of Object.entries(inheritedOpaqueSources)) {
    equal(sourceFiles[file], expected, 'OPAQUE-SOURCE', file);
    const source = readSource(file);
    for (const [name, reviewed] of Object.entries(inheritedOpaqueBodies[file])) {
      equal(sha(opaqueBody(source, name)), reviewed, 'OPAQUE-MODEL', `${file}::${name}`);
    }
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
          'crates/noble-kernel/src/untrusted.rs'].includes(file) && match[2] === 'test, derive(PartialEq, Eq))]' ||
        match[1] === 'cfg_attr' && file === 'crates/noble-kernel/src/types.rs' &&
          match[2] === 'test, derive(Eq))]' &&
          text.slice(match.index + match[0].length).startsWith('\npub enum NominalShape {');
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
      // An ADT constructor has a real compiler body and a real translated
      // `.Variant.constructor` definition. It is not an omitted-body waiver.
      const constructor = kind === 'functions' && decl.src === 'AdtConstructor';
      if (kind === 'functions' && disposition === 'translated' && !translated.some(row =>
        !row.loop && row.lean_name.endsWith(
          `.${row.rust_name.split('::').at(-1)}${constructor ? '.constructor' : ''}`))) {
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
            // An opaque foreign type hides projection-name collisions: its
            // accessor may be `index` while actual extraction emits `impl.index`.
            // The complete Rust identity, not that local Lean spelling, joins
            // exactly one non-opaque entrypoint, not its generated loop helpers,
            // in the actual dependency extraction.
            const actual = accounts[dependency.id].translation[kind].filter(other => other.is_local && !other.is_opaque && !other.loop &&
              other.lean_file === 'Funs.lean' &&
              other.rust_name === item.rust_name);
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

// The backend now depends on the frontend's checked WIT exports. Its external
// type module imports frontend types before elaborating backend types, so the
// pinned discriminant elaborator allocates fresh names in dependency order.
// Audit the whole transitive lane set without editing generated bytes.
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
      for (const target of lane === 'kernel' || lane === 'contracts' ? ['contracts', 'wasm'] : [lane]) {
        packets[target][field].push(row);
      }
    }
  }
  return packets;
}
