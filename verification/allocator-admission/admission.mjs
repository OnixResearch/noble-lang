// Noble allocator admission (#121): the pure decision that decides whether a *selected*
// Charon -> Aeneas -> Lean extraction may enter a Noble proof-backed subject.
//
// The selected toolchain cannot answer this question on its own. With `--preset=aeneas`
// Charon sets `hide_allocator: true`, so the LLBC and the Aeneas Lean both erase the
// allocator: `alloc::vec::Vec<T, A>` becomes `alloc::vec::Vec<T>` (pinned library
// `Aeneas/Std/Vec.lean` keeps only the element type), and `Vec::new_in` becomes an
// `axiom (T : Type) {A : Type} : A -> Result (alloc.vec.Vec T)` in the generated file,
// called as `new_in Std.U8 ()` for an explicit `Global` subject. Aeneas accepts that
// translation and Lean checks it, so a *successful* pipeline says nothing about the
// allocator. This module is where Noble refuses instead.
//
// Every refusal carries the Rust source span it was derived from. The module is pure:
// it takes already-parsed LLBC and `translation.json` documents plus a binding of the
// identities the run was made with, and returns a decision record. `run.mjs` produces
// the documents and the binding; `admission.test.mjs` pins every rule.
//
// Evidence classes stay separate: an `admitted` result here is an *admission* decision
// for one extraction, not a refinement theorem, not a proof, and not evidence that other
// allocator uses are sound.

export const schema = 'noble-allocator-admission/v1';

/// The only allocator this boundary admits: the implicit global allocator, which the
/// subject never names. Every other allocator needs its own reviewed model
/// (see `decision.md`).
export const admittedAllocator = 'implicit-global';

/// Constructor/accessor names that take an allocator argument. A reachable declaration
/// with one of these final name segments proves the subject names an allocator.
export const explicitConstructors = [
  'new_in',
  'try_new_in',
  'with_capacity_in',
  'try_with_capacity_in',
  'new_uninit_in',
  'try_new_uninit_in',
  'new_zeroed_in',
  'try_new_zeroed_in',
  'from_raw_parts_in',
  'from_raw_parts_mut_in',
  'into_raw_parts_with_alloc',
  'allocate_in',
  'try_allocate_in',
];

/// The allocator traits. `hide_allocator` normally drops these clauses entirely, so a
/// hit means the selected pipeline did *not* erase one.
export const allocatorTraits = [
  'core::alloc::Allocator',
  'core::alloc::AllocatorClone',
  'core::alloc::GlobalAlloc',
];

/// The trait *leaf* names that name the allocator contract. Matched as the final path
/// identifier only: a concrete type such as `stateful_alloc::CountingAllocator` is an
/// allocator *implementation*, not the contract, and the contract check must not fire on
/// it (the constructor/source rules already refuse that case for the right reason).
export const allocatorTraitLeaves = ['Allocator', 'AllocatorClone', 'GlobalAlloc'];

/// Final two path segments of a rendered declaration, for the allocator-constructor test.
const finalSegment = reached => reached.slice(-1)[0] ?? '';

/// Whether a path names one of the allocator contracts.
function namesAllocatorContract(segments) {
  return segments.length > 0 && allocatorTraitLeaves.includes(finalSegment(segments));
}

/// Render one LLBC path element list (`item_meta.name`, `item_names[].value`) as text.
/// Only `Ident` elements carry a name; the others are structural and rendered as tags so
/// a rendered path never invents an identifier.
export function renderPath(path) {
  if (path === null || path === undefined) return '<none>';
  if (typeof path === 'string') return path;
  if (!Array.isArray(path)) return JSON.stringify(path);
  return path.map(element => {
    if (element === null || typeof element !== 'object') return String(element);
    const tag = Object.keys(element)[0];
    if (tag === 'Ident') return String(element.Ident?.[0] ?? '');
    return `<${tag}>`;
  }).join('::');
}

/// The identifiers of a path, in order.
function pathIdents(path) {
  if (!Array.isArray(path)) return [];
  return path.filter(element => element && typeof element === 'object' && element.Ident)
    .map(element => String(element.Ident[0]));
}

/// A `{line, col}` position from an LLBC `span`, or null for a generated span.
export function spanOf(meta, files) {
  const span = meta?.span;
  const data = span?.Untagged?.data ?? span?.Value?.[1]?.data ?? null;
  if (!data) return null;
  const file = files?.[data.file_id]?.name?.Local ?? files?.[data.file_id]?.name?.Virtual ?? null;
  return { file, begin: { line: data.beg.line, col: data.beg.col }, end: { line: data.end.line, col: data.end.col } };
}

/// Every function declaration the LLBC kept, in `def_id` order. Charon's `--start-from`
/// bounds the LLBC to the declared root's closure, so this list *is* the reachable set.
export function reachableDeclarations(llbc) {
  const translated = llbc?.translated ?? llbc;
  const files = translated?.files ?? [];
  return (translated?.fun_decls ?? []).filter(Boolean).map(decl => ({
    def_id: decl.def_id,
    is_local: decl.item_meta?.is_local === true,
    opacity: decl.item_meta?.opacity ?? null,
    ident_count: pathIdents(decl.item_meta?.name).length,
    segments: pathIdents(decl.item_meta?.name),
    rendered: renderPath(decl.item_meta?.name),
    span: spanOf(decl.item_meta, files),
    source_text: decl.item_meta?.source_text ?? null,
    body_is_structured: decl.body?.Structured !== undefined,
  }));
}

/// The type declarations the LLBC kept, with their declared type parameters.
export function reachableTypes(llbc) {
  const translated = llbc?.translated ?? llbc;
  return (translated?.type_decls ?? []).filter(Boolean).map(decl => ({
    def_id: decl.def_id,
    rendered: renderPath(decl.item_meta?.name),
    type_parameters: (decl.generics?.types ?? []).map(parameter => parameter.name),
  }));
}

/// Source location of the first explicit-allocator construct in a retained Rust body.
/// `source_text` begins at the declaration's `span.begin`, so an offset maps back to a
/// real file position instead of a synthetic one.
export function sourceConstruct(decl) {
  if (!decl.source_text || !decl.span?.begin) return null;
  const pattern = new RegExp(`(?:\\b(?:${explicitConstructors.join('|')})\\s*\\(|`
    + '\\bAllocator(?:Clone)?\\b|\\bSystem\\b)', 'g');
  const match = pattern.exec(decl.source_text);
  if (!match) return null;
  const before = decl.source_text.slice(0, match.index);
  const lines = before.split('\n');
  const line = decl.span.begin.line + lines.length - 1;
  const col = lines.length === 1 ? decl.span.begin.col + match.index : lines[lines.length - 1].length;
  return { construct: match[0].trim().replace(/\s*\($/, ''), begin: { line, col },
    file: decl.span.file, excerpt: match[0] };
}

function refusal(code, message, evidence, span) {
  return { code, message, span: span ?? null, evidence };
}

/// Compare the run's binding with the frozen expectation. A changed source, allocator,
/// tool or configuration identity invalidates the decision: admission is per bound run.
export function bindingRefusals(binding, expected) {
  const refusals = [];
  const required = expected?.root ?? binding?.root;
  const fields = [
    ['schema', binding?.schema, expected?.schema],
    ['root', binding?.root, required],
    ['source_sha256', binding?.source?.sha256, expected?.source?.sha256],
    ['fixture', binding?.source?.fixture, expected?.source?.fixture],
    ['selection_sha256', binding?.selection_sha256, expected?.selection_sha256],
    ['hide_allocator', binding?.options?.hide_allocator, expected?.options?.hide_allocator],
    ['charon_rev', binding?.tools?.charon?.rev, expected?.tools?.charon?.rev],
    ['aeneas_rev', binding?.tools?.aeneas?.rev, expected?.tools?.aeneas?.rev],
    ['aeneas_lean_rev', binding?.tools?.aeneas_lean?.rev, expected?.tools?.aeneas_lean?.rev],
    ['rustc_version', binding?.tools?.extraction_rust?.version, expected?.tools?.extraction_rust?.version],
  ];
  for (const [name, observed, wanted] of fields) {
    if (observed !== wanted) {
      refusals.push(refusal('BINDING-IDENTITY',
        `${name} differs from the bound identity: ${JSON.stringify(observed)} != ${JSON.stringify(wanted)}`,
        { field: name, observed, expected: wanted }));
    }
  }
  // The audit's applicability condition: the selected preset must erase the allocator.
  // A pipeline that stopped erasing it could see allocator evidence this decision does not
  // model, so the decision does not extend to it.
  if (expected?.options?.hide_allocator !== true) {
    refusals.push(refusal('BINDING-OPTIONS',
      'the selected Charon preset does not erase the allocator; this audit does not apply',
      { hide_allocator: expected?.options?.hide_allocator }));
  }
  const startFrom = binding?.options?.start_from;
  if (!Array.isArray(startFrom) || startFrom.length !== 1 || startFrom[0] !== required) {
    refusals.push(refusal('BINDING-ROOT',
      `the LLBC was not produced for ${required}: start_from ${JSON.stringify(startFrom)}`,
      { field: 'start_from', start_from: startFrom, root: required }));
  }
  return refusals;
}

/// Decide admission for one bound extraction.
///
/// * `document.root`        - the declared extraction root, `crate::function`.
/// * `document.llbc`        - the parsed LLBC (`translated` subtree accepted too).
/// * `document.translation` - the parsed `translation.json` Aeneas wrote.
/// * `document.binding`     - source/tool/configuration identities of this run.
/// * `document.expected`    - the frozen identities the run must match.
export function admit(document) {
  const { root, llbc, translation, binding, expected } = document;
  const declarations = reachableDeclarations(llbc);
  const types = reachableTypes(llbc);
  const functions = (translation?.functions ?? []);
  const rootDeclaration = declarations.find(declaration => declaration.rendered === root) ?? null;
  // Omitting `options` from the expectation means "the audit's own applicability
  // condition", i.e. the selected preset erases the allocator.
  const frozen = { options: { hide_allocator: true }, ...expected, root };
  const refusals = [...bindingRefusals({ ...binding, root }, frozen)];

  if (!rootDeclaration) {
    refusals.push(refusal('ROOT-MISSING',
      `${root} is not in the translation: the extraction root is unreached, so no allocator evidence exists`,
      { root, candidates: declarations.filter(d => d.is_local).map(d => d.rendered) }));
  } else {
    if (!rootDeclaration.is_local) {
      refusals.push(refusal('ROOT-MISSING', `${root} is not a local declaration`, { rendered: root }));
    }
    if (rootDeclaration.opacity !== 'Transparent' || !rootDeclaration.body_is_structured) {
      refusals.push(refusal('ROOT-OPAQUE',
        `${root} has no transparent body (opacity ${rootDeclaration.opacity}), so the operations it reaches cannot be audited`,
        { opacity: rootDeclaration.opacity, body_is_structured: rootDeclaration.body_is_structured },
        rootDeclaration.span));
    }
    const construct = sourceConstruct(rootDeclaration);
    if (construct) {
      refusals.push(refusal('ALLOCATOR-SOURCE',
        `${root} names an explicit allocator (${construct.construct}); the admitted model is ${admittedAllocator} only`,
        { construct: construct.construct, excerpt: construct.excerpt }, construct));
    }
  }

  for (const declaration of declarations) {
    const last = finalSegment(declaration.segments);
    if (explicitConstructors.includes(last)) {
      refusals.push(refusal('ALLOCATOR-CONSTRUCTOR',
        `${root} reaches ${declaration.rendered}, which takes an explicit allocator argument`,
        { rendered: declaration.rendered, def_id: declaration.def_id },
        rootDeclaration?.span ?? null));
    }
    if (namesAllocatorContract(declaration.segments)) {
      refusals.push(refusal('ALLOCATOR-TRAIT',
        `${root} reaches ${declaration.rendered}, which mentions the Allocator contract`,
        { rendered: declaration.rendered, def_id: declaration.def_id }, rootDeclaration?.span ?? null));
    }
  }

  const translated = llbc?.translated ?? llbc;
  const traitNames = [
    ...(translated?.trait_decls ?? []).filter(Boolean).map(decl => decl.item_meta?.name),
    ...(translated?.trait_impls ?? []).filter(Boolean).map(decl => decl.item_meta?.name),
  ];
  for (const name of traitNames) {
    if (namesAllocatorContract(pathIdents(name))) {
      refusals.push(refusal('ALLOCATOR-TRAIT', `${root} translates the allocator contract ${renderPath(name)}`,
        { rendered: renderPath(name) }, rootDeclaration?.span ?? null));
    }
  }

  for (const fn of functions) {
    const segments = String(fn.rust_name ?? '').split('::');
    const last = segments.slice(-1)[0].replace(/[{}<>@0-9]/g, '');
    if (fn.is_opaque === true && explicitConstructors.includes(last)) {
      refusals.push(refusal('ALLOCATOR-AXIOM',
        `${root} translates ${fn.rust_name} as an opaque external, so its allocator argument and failure are unmodelled`,
        { rust_name: fn.rust_name, lean_name: fn.lean_name, is_opaque: fn.is_opaque },
        rootDeclaration?.span ?? null));
    }
    if (allocatorTraits.some(trait => String(fn.rust_name ?? '').includes(trait))) {
      refusals.push(refusal('ALLOCATOR-AXIOM', `${root} translates the allocator contract ${fn.rust_name}`,
        { rust_name: fn.rust_name }, rootDeclaration?.span ?? null));
    }
  }

  const vecModel = types.find(type => type.rendered === 'alloc::vec::Vec') ?? null;
  const concreteAllocators = types
    .filter(type => type.rendered !== 'alloc::vec::Vec'
      && (type.rendered.endsWith('::System') || type.rendered.endsWith('::Global')
        || type.rendered.includes('Allocator')))
    .map(type => type.rendered);
  const opaqueExternals = functions.filter(fn => fn.is_opaque === true).map(fn => fn.rust_name).sort();

  return {
    schema,
    root,
    result: refusals.length === 0 ? 'admitted' : 'refused',
    admitted_allocator: admittedAllocator,
    // The identities this decision was made with. Re-checking a retained decision against
    // the identities in force later compares exactly these fields.
    binding: { ...binding, root },
    refusals,
    observables: {
      reachable_declarations: declarations.length,
      local_declarations: declarations.filter(declaration => declaration.is_local).map(d => d.rendered),
      root_span: rootDeclaration?.span ?? null,
      allocator_erased: binding?.options?.hide_allocator === true,
      vec_model_type_parameters: vecModel?.type_parameters ?? null,
      allocator_type_declarations: concreteAllocators,
      opaque_externals: opaqueExternals,
      translation_versions: { aeneas: translation?.aeneas_version ?? null, charon: translation?.charon_version ?? null },
    },
  };
}

/// Re-check a retained decision against the identities in force now. A receipt whose
/// source, allocator, tool or configuration binding changed cannot produce admission:
/// the answer it recorded belongs to the run it was made for.
export function revalidate(record, current) {
  if (!record || record.schema !== schema) {
    return [refusal('STALE-RECEIPT', 'the record is not an allocator-admission decision', { schema: record?.schema })];
  }
  if (!record.binding) {
    return [refusal('STALE-RECEIPT', 'the decision records no binding, so nothing can be re-checked', {})];
  }
  return bindingRefusals(record.binding, current ?? {}).map(drift => ({
    ...drift, code: 'STALE-RECEIPT', message: `stale decision: ${drift.message}`,
  }));
}

/// The one-line matrix cell for a decision: `admitted` only when nothing was refused.
export function verdict(record) {
  return record.result === 'admitted' && record.refusals.length === 0 ? 'admitted' : 'refused';
}
