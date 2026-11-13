// Rule pins for the allocator-admission decision (#121). These tests execute the pure
// decision over LLBC/translation documents whose shape is taken from real
// charon-0.1.254 / aeneas-0.1.0 output for the fixtures in `fixtures/`: `fun_decls`
// contains `null` slots, `item_meta.name` is a path of `Ident`/`Impl` elements,
// `item_meta.span` is `{Untagged: {data: ...}}`, and `item_meta.source_text` is the
// retained Rust body. No Lean, Charon or Aeneas process runs here; `run.mjs` executes the
// real pipeline and its controls.
import assert from 'node:assert/strict';
import { test } from 'bun:test';
import {
  admit, admittedAllocator, allocatorTraitLeaves, bindingRefusals, explicitConstructors,
  reachableDeclarations, renderPath, revalidate, schema, sourceConstruct, spanOf, verdict,
} from './admission.mjs';

const source = 'verification/allocator-admission/fixtures/probes/src/lib.rs';
const sha = 'a'.repeat(64);
const expected = {
  schema,
  source: { fixture: source, sha256: sha },
  selection_sha256: 'b'.repeat(64),
  options: { hide_allocator: true },
  tools: { charon: { rev: 'b104e24f' }, aeneas: { rev: '505b6ca3' }, aeneas_lean: { rev: '1238879f' },
    extraction_rust: { version: 'rustc 1.100.0-nightly (2026-08-18)' } },
};

const ident = name => ({ Ident: [name, 0] });
const impl = () => ({ Impl: { Ty: { params: {}, skip_binder: { Deduplicated: 0 }, kind: 'InherentImplBlock' } } });
const pathOf = (...names) => names.map(name => name === 'Impl' ? impl() : ident(name));

function span(beginLine, beginCol, endLine, endCol) {
  return { Untagged: { data: { file_id: 0, beg: { line: beginLine, col: beginCol },
    end: { line: endLine, col: endCol } }, generated_from_span: null } };
}

function declaration({ id, path, text = null, line = 10, col = 0, endLine = line, endCol = 1,
  local = true, opacity = 'Transparent', body = { Structured: { span: null } } }) {
  return { def_id: id, item_meta: { name: path, span: span(line, col, endLine, endCol),
    source_text: text, attr_info: {}, is_local: local, opacity },
  generics: { regions: [], types: [], const_generics: [], trait_clauses: [] }, signature: {}, body };
}

function typeDeclaration({ id, path, parameters = ['T'] }) {
  return { def_id: id, item_meta: { name: path }, generics: { types: parameters.map((name, index) => ({ index, name })) } };
}

function document({ root = 'probes::vec_baseline', startFrom = [root], declarations, types = [],
  traits = [], impls = [], functions = [] }) {
  return {
    root,
    llbc: { charon_version: '0.1.254', translated: { crate_name: 'probes',
      options: { ullbc: false, hide_allocator: true, treat_box_as_builtin: true, preset: 'Aeneas',
        start_from: startFrom },
      files: [{ name: { Local: 'src/lib.rs' } }], item_names: [], global_decls: [],
      type_decls: types, fun_decls: declarations, trait_decls: traits, trait_impls: impls } },
    translation: { aeneas_version: '0.1.0', charon_version: '0.1.254', crate: 'probes',
      functions, types: [], globals: [], trait_decls: [], trait_impls: [] },
    binding: { schema, source: { fixture: source, sha256: sha }, selection_sha256: expected.selection_sha256,
      options: { hide_allocator: true, start_from: startFrom, preset: 'Aeneas' }, root,
      tools: structuredClone(expected.tools) },
    expected,
  };
}

const vecType = typeDeclaration({ id: 2, path: pathOf('alloc', 'vec', 'Vec') });
const globalType = typeDeclaration({ id: 3, path: pathOf('alloc', 'alloc', 'Global'), parameters: [] });

function baseline({ text, root = 'probes::vec_baseline', reachable = [] }) {
  return document({
    root,
    declarations: [
      null,
      declaration({ id: 11, path: pathOf('probes', root.split('::')[1]), text, line: 27, endLine: 31 }),
      ...reachable,
      declaration({ id: 24, path: pathOf('alloc', 'alloc', 'Global', 'Impl', 'DropGlue'), local: false,
        opacity: 'Opaque', body: 'Opaque', line: 1, endLine: 1 }),
      null,
    ],
    types: [vecType, globalType],
    functions: [{ def_id: 11, rust_name: root, lean_name: `probes.${root.split('::')[1]}`, is_opaque: false }],
  });
}

const codes = report => report.refusals.map(refusal => refusal.code);

test('the implicit-Global baseline is admitted, and keeps the Global drop glue it always had', () => {
  const report = admit(baseline({ text: 'pub fn vec_baseline(input: u8) -> usize {\n'
    + '    let mut xs: Vec<u8> = Vec::new();\n    xs.push(input);\n    xs.len()\n}',
    reachable: [declaration({ id: 21, path: pathOf('alloc', 'vec', 'Impl', 'push'), local: false,
      opacity: 'Opaque', body: 'Opaque', line: 1, endLine: 1 })] }));
  assert.equal(report.result, 'admitted');
  assert.deepEqual(report.refusals, []);
  assert.equal(verdict(report), 'admitted');
  assert.equal(report.admitted_allocator, admittedAllocator);
  assert.equal(report.observables.allocator_erased, true);
  assert.deepEqual(report.observables.vec_model_type_parameters, ['T'],
    'the selected library keeps only the element type: that erasure is why the source rule exists');
  assert.deepEqual(report.observables.root_span,
    { file: 'src/lib.rs', begin: { line: 27, col: 0 }, end: { line: 31, col: 1 } });
  assert.ok(report.binding, 'a decision records the identities it was made with');
});

test('an explicit Global constructor is refused at the call site, not only at the root', () => {
  const text = 'pub fn global_vec(input: u8) -> usize {\n'
    + '    let mut xs: Vec<u8, Global> = Vec::new_in(Global);\n    xs.push(input);\n    xs.len()\n}';
  const report = admit(document({
    root: 'probes::global_vec', declarations: [
      declaration({ id: 11, path: pathOf('probes', 'global_vec'), text, line: 54, endLine: 58 }),
      declaration({ id: 20, path: pathOf('alloc', 'vec', 'Impl', 'new_in'), local: false, opacity: 'Opaque',
        body: 'Opaque', line: 1, endLine: 1 }),
    ], types: [vecType, globalType],
    functions: [
      { def_id: 20, rust_name: 'alloc::vec::{alloc::vec::Vec<T>}::new_in', lean_name: 'probes.alloc.vec.Vec.new_in', is_opaque: true },
      { def_id: 11, rust_name: 'probes::global_vec', lean_name: 'probes.global_vec', is_opaque: false },
    ],
  }));
  assert.equal(report.result, 'refused');
  assert.deepEqual([...new Set(codes(report))].sort(),
    ['ALLOCATOR-AXIOM', 'ALLOCATOR-CONSTRUCTOR', 'ALLOCATOR-SOURCE']);
  const located = report.refusals.find(refusal => refusal.code === 'ALLOCATOR-SOURCE');
  assert.deepEqual(located.span, { construct: 'new_in', begin: { line: 55, col: 39 }, file: 'src/lib.rs', excerpt: 'new_in(' });
  assert.match(report.refusals.find(refusal => refusal.code === 'ALLOCATOR-AXIOM').message, /unmodelled/);
});

test('a generic Allocator bound is refused with the bound position, even before any call', () => {
  const text = 'pub fn generic_vec<A: Allocator>(alloc: A, input: u8) -> usize {\n'
    + '    let mut xs: Vec<u8, A> = Vec::new_in(alloc);\n    xs.push(input);\n    xs.len()\n}';
  const report = admit(document({
    root: 'probes::generic_vec',
    declarations: [declaration({ id: 11, path: pathOf('probes', 'generic_vec'), text, line: 70, endLine: 74 })],
    types: [vecType],
  }));
  assert.equal(report.result, 'refused');
  const located = report.refusals.find(refusal => refusal.code === 'ALLOCATOR-SOURCE');
  assert.deepEqual(located.span.begin, { line: 70, col: 22 });
  assert.equal(located.evidence.construct, 'Allocator');
});

test('a borrowed allocator is refused at the reference parameter', () => {
  const text = 'pub fn borrowed_vec<A: Allocator>(alloc: &A, input: u8) -> usize {\n'
    + '    let mut xs: Vec<u8, &A> = Vec::new_in(alloc);\n    xs.push(input);\n    xs.len()\n}';
  const report = admit(document({
    root: 'probes::borrowed_vec',
    declarations: [declaration({ id: 11, path: pathOf('probes', 'borrowed_vec'), text, line: 92, endLine: 96 })],
    types: [vecType],
  }));
  assert.equal(report.result, 'refused');
  assert.deepEqual(report.refusals.find(refusal => refusal.code === 'ALLOCATOR-SOURCE').span.begin,
    { line: 92, col: 23 });
});

test('System and a custom stateful allocator are refused, and CountingAllocator is not called a contract', () => {
  const system = admit(document({
    root: 'probes::system_vec',
    declarations: [declaration({ id: 11, path: pathOf('probes', 'system_vec'),
      text: 'pub fn system_vec(input: u8) -> usize {\n    let mut xs: Vec<u8, System> = Vec::new_in(System);\n    xs.push(input);\n    xs.len()\n}',
      line: 103, endLine: 107 })],
    types: [vecType, typeDeclaration({ id: 3, path: pathOf('std', 'alloc', 'System'), parameters: [] })],
  }));
  assert.equal(system.result, 'refused');
  assert.equal(system.refusals.find(refusal => refusal.code === 'ALLOCATOR-SOURCE').evidence.construct, 'System');

  const custom = admit(document({
    root: 'probes::stateful_vec',
    declarations: [declaration({ id: 11, path: pathOf('probes', 'stateful_vec'),
      text: 'pub fn stateful_vec(input: u8) -> usize {\n    let alloc = stateful_alloc::CountingAllocator::new();\n'
        + '    let mut xs: Vec<u8, stateful_alloc::CountingAllocator> = Vec::new_in(alloc);\n'
        + '    xs.push(input);\n    xs.len()\n}', line: 110, endLine: 115 })],
    impls: [{ def_id: 6, item_meta: { name: pathOf('stateful_alloc', 'CountingAllocator', 'Impl') } }],
  }));
  assert.equal(custom.result, 'refused');
  assert.ok(!codes(custom).includes('ALLOCATOR-TRAIT'),
    'a concrete allocator implementation is refused by the constructor rule, not by the contract rule');
});

test('every explicit-allocator constructor name is refused when it is reachable', () => {
  for (const name of explicitConstructors) {
    const report = admit(baseline({
      text: 'pub fn vec_baseline(input: u8) -> usize {\n    let mut xs: Vec<u8> = Vec::new();\n    xs.push(input);\n    xs.len()\n}',
      reachable: [declaration({ id: 30, path: pathOf('alloc', 'vec', 'Impl', name), local: false,
        opacity: 'Opaque', body: 'Opaque', line: 1, endLine: 1 })],
    }));
    assert.equal(report.result, 'refused', `${name} must not pass admission`);
    assert.ok(codes(report).includes('ALLOCATOR-CONSTRUCTOR'), `${name} must be named`);
  }
});

test('a reachable Allocator contract is refused even when the retained source names none', () => {
  const report = admit(document({
    root: 'probes::vec_baseline',
    declarations: [declaration({ id: 11, path: pathOf('probes', 'vec_baseline'),
      text: 'pub fn vec_baseline(input: u8) -> usize {\n    let mut xs: Vec<u8> = Vec::new();\n    xs.push(input);\n    xs.len()\n}',
      line: 27, endLine: 31 })],
    traits: [{ def_id: 0, item_meta: { name: pathOf('core', 'alloc', 'Allocator') } }],
  }));
  assert.equal(report.result, 'refused');
  assert.deepEqual([...new Set(codes(report))], ['ALLOCATOR-TRAIT']);
  assert.equal(report.observables.vec_model_type_parameters, null);
});

test('an opaque replacement body is refused: an unreached operation is not audited', () => {
  const opaque = admit(baseline({ text: 'pub fn vec_baseline(input: u8) -> usize {\n    let mut xs: Vec<u8> = Vec::new();\n    xs.push(input);\n    xs.len()\n}' }));
  const mutated = structuredClone(opaque.binding);
  assert.equal(admit(baseline({ text: 'pub fn vec_baseline(input: u8) -> usize {\n    let mut xs: Vec<u8> = Vec::new();\n    xs.push(input);\n    xs.len()\n}' })).result, 'admitted');
  const documentWithOpaqueBody = baseline({ text: 'pub fn vec_baseline(input: u8) -> usize {\n    let mut xs: Vec<u8> = Vec::new();\n    xs.push(input);\n    xs.len()\n}' });
  documentWithOpaqueBody.llbc.translated.fun_decls[1].body = 'Opaque';
  const report = admit(documentWithOpaqueBody);
  assert.equal(report.result, 'refused');
  assert.deepEqual([...new Set(codes(report))], ['ROOT-OPAQUE']);
  assert.deepEqual(report.refusals[0].span.begin, { line: 27, col: 0 });
  assert.ok(mutated);
});

test('an opaque declaration opacity is refused like an opaque body', () => {
  const documentWithOpaque = baseline({ text: 'pub fn vec_baseline(input: u8) -> usize {\n    let mut xs: Vec<u8> = Vec::new();\n    xs.push(input);\n    xs.len()\n}' });
  documentWithOpaque.llbc.translated.fun_decls[1].item_meta.opacity = 'Opaque';
  assert.deepEqual([...new Set(codes(admit(documentWithOpaque)))], ['ROOT-OPAQUE']);
});

test('an unreached root cannot borrow another root\'s extraction', () => {
  const report = admit({
    ...baseline({ text: 'pub fn vec_baseline(input: u8) -> usize {\n    let mut xs: Vec<u8> = Vec::new();\n    xs.push(input);\n    xs.len()\n}' }),
    root: 'probes::global_vec',
  });
  assert.equal(report.result, 'refused');
  assert.ok(codes(report).includes('ROOT-MISSING'));
  // The decision's root is the claim; the LLBC's own `start_from` is the evidence. A
  // decision cannot take its root from the run it is auditing.
  assert.deepEqual([...new Set(codes(report))].sort(), ['BINDING-ROOT', 'ROOT-MISSING']);
  assert.deepEqual(report.observables.local_declarations, ['probes::vec_baseline']);
});

test('a changed source or allocator identity refuses the decision', () => {
  const clean = baseline({ text: 'pub fn vec_baseline(input: u8) -> usize {\n    let mut xs: Vec<u8> = Vec::new();\n    xs.push(input);\n    xs.len()\n}' });
  assert.equal(admit(clean).result, 'admitted');
  const changedSource = structuredClone(clean);
  changedSource.expected = { ...expected, source: { fixture: source, sha256: 'c'.repeat(64) } };
  assert.deepEqual([...new Set(codes(admit(changedSource)))], ['BINDING-IDENTITY']);
  const changedLibrary = structuredClone(clean);
  changedLibrary.expected = { ...expected, tools: { ...expected.tools, aeneas_lean: { rev: 'deadbeef' } } };
  assert.deepEqual([...new Set(codes(admit(changedLibrary)))], ['BINDING-IDENTITY']);
  const noErasure = structuredClone(clean);
  noErasure.expected = { ...expected, options: { hide_allocator: false } };
  assert.deepEqual([...new Set(codes(admit(noErasure)))].sort(), ['BINDING-IDENTITY', 'BINDING-OPTIONS']);
});

test('a stale receipt cannot be revalidated into an admission for changed identities', () => {
  const clean = baseline({ text: 'pub fn vec_baseline(input: u8) -> usize {\n    let mut xs: Vec<u8> = Vec::new();\n    xs.push(input);\n    xs.len()\n}' });
  const record = admit(clean);
  const current = structuredClone(clean.binding);
  assert.deepEqual(revalidate(record, current), []);
  const mutations = [
    { ...current, source: { fixture: source, sha256: 'd'.repeat(64) } },
    { ...current, selection_sha256: 'e'.repeat(64) },
    { ...current, root: 'probes::global_vec' },
    { ...current, options: { ...current.options, hide_allocator: false } },
    { ...current, tools: { ...current.tools, aeneas: { rev: 'deadbeef' } } },
    { ...current, tools: { ...current.tools, charon: { rev: 'deadbeef' } } },
    { ...current, tools: { ...current.tools, aeneas_lean: { rev: 'deadbeef' } } },
    { ...current, tools: { ...current.tools, extraction_rust: { version: 'rustc 1.99.0' } } },
  ];
  for (const mutation of mutations) {
    const drift = revalidate(record, mutation);
    assert.deepEqual([...new Set(drift.map(refusal => refusal.code))], ['STALE-RECEIPT'],
      `mutation ${JSON.stringify(mutation).slice(0, 80)} must not revalidate`);
  }
  assert.deepEqual(revalidate({ schema: 'other' }, current).map(refusal => refusal.code), ['STALE-RECEIPT']);
  assert.deepEqual(revalidate({ schema }, current).map(refusal => refusal.code), ['STALE-RECEIPT'],
    'a record with no binding cannot be revalidated');
});

test('bindingRefusals names every field it compares', () => {
  const binding = baseline({ text: 'pub fn vec_baseline(input: u8) -> usize {\n    let mut xs: Vec<u8> = Vec::new();\n    xs.push(input);\n    xs.len()\n}' }).binding;
  assert.deepEqual(bindingRefusals(binding, expected), []);
  const drift = bindingRefusals(binding, { ...expected, root: 'probes::other' });
  assert.deepEqual(drift.map(refusal => refusal.evidence.field), ['root', 'start_from']);
});

test('span rendering and declaration filtering match the real LLBC encoding', () => {
  const declarations = reachableDeclarations({ translated: { files: [{ name: { Local: 'src/lib.rs' } }],
    fun_decls: [null, declaration({ id: 7, path: pathOf('probes', 'vec_baseline'), line: 27, endLine: 31 }), null] } });
  assert.equal(declarations.length, 1);
  assert.equal(declarations[0].rendered, 'probes::vec_baseline');
  assert.deepEqual(declarations[0].segments, ['probes', 'vec_baseline']);
  assert.equal(declarations[0].body_is_structured, true);
  assert.deepEqual(spanOf({ span: span(4, 2, 6, 3) }, [{ name: { Local: 'src/lib.rs' } }]),
    { file: 'src/lib.rs', begin: { line: 4, col: 2 }, end: { line: 6, col: 3 } });
  assert.equal(spanOf({ span: { Value: [9, { data: { file_id: 0, beg: { line: 1, col: 0 },
    end: { line: 1, col: 1 } } }] } }, [{ name: { Local: 'src/lib.rs' } }]).begin.line, 1,
  'a deduplicated span is the same shape');
  assert.equal(spanOf({}, []), null);
  assert.equal(renderPath(null), '<none>');
  assert.equal(renderPath(pathOf('alloc', 'vec', 'Impl', 'new_in')), 'alloc::vec::<Impl>::new_in');
  assert.deepEqual(allocatorTraitLeaves, ['Allocator', 'AllocatorClone', 'GlobalAlloc']);
});

test('a source construct maps an offset back to its own file position', () => {
  const [declarationWithConstruct] = reachableDeclarations({ translated: {
    files: [{ name: { Local: 'src/lib.rs' } }], fun_decls: [declaration({ id: 1, path: pathOf('probes', 'x'),
      line: 40, col: 4, text: 'pub fn x() {\n    let a = Vec::new_in(Global);\n}', endLine: 42 })] } });
  const construct = sourceConstruct(declarationWithConstruct);
  assert.equal(construct.construct, 'new_in');
  assert.deepEqual(construct.begin, { line: 41, col: 17 });
  const plain = reachableDeclarations({ translated: { files: [{ name: { Local: 'src/lib.rs' } }],
    fun_decls: [declaration({ id: 2, path: pathOf('probes', 'y'), text: 'pub fn y() {}' }),
      declaration({ id: 3, path: pathOf('probes', 'z'), text: null })] } });
  assert.equal(sourceConstruct(plain[0]), null);
  assert.equal(sourceConstruct(plain[1]), null);
});
