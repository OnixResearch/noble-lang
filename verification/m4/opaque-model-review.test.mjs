import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { annotatedItem, inheritedOpaqueBodies, inheritedOpaqueSources, reviewedAnnotations,
  reviewedOpaque, reviewedOpaqueTypes, sourcePolicy } from './accounting.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const sources = Object.fromEntries(Object.keys(inheritedOpaqueSources).map(file =>
  [file, fs.readFileSync(path.join(root, file), 'utf8')]));
const hash = text => crypto.createHash('sha256').update(text).digest('hex');
const sourceFiles = Object.fromEntries(Object.entries(sources).map(([file, text]) =>
  [file, hash(text)]));
const readSource = file => sources[file];
const effectOracleFile = 'crates/noble-contracts/src/inference/effects.rs';
const effectOracleSource = fs.readFileSync(path.join(root, effectOracleFile), 'utf8');
const sourcePolicySources = { ...sources, [effectOracleFile]: effectOracleSource };
const sourcePolicyFiles = { ...sourceFiles, [effectOracleFile]: hash(effectOracleSource) };
const readPolicySource = file => sourcePolicySources[file];
const annotationSources = { ...sourcePolicySources, ...Object.fromEntries(
  Object.keys(reviewedAnnotations).map(file => [file, fs.readFileSync(path.join(root, file), 'utf8')])) };
const annotationFiles = Object.fromEntries(Object.entries(annotationSources).map(([file, text]) => [file, hash(text)]));
const readAnnotationSource = file => annotationSources[file];
const intrinsicFile = 'crates/noble-contracts/src/intrinsic.rs';

test('annotated item extent ignores Rust strings, characters and nested comments', () => {
  const item = `fn helper() {
    let text = "}";
    let raw = r##"{ ; }"##;
    let escaped = '\\'';
    let delimiter = '}';
    /* } /* { */ ; */
    // }
    let _ = (text, raw, escaped, delimiter);
}`;
  const source = `#[charon::opaque]\n${item}\nfn unrelated() {}`;
  const observed = annotatedItem(source, source.indexOf('charon'), 'control.rs');
  assert.equal(observed.text, item);
  assert.equal(observed.item, 'fn helper() {');
});

test('annotated tuple struct extent includes semicolons inside array types', () => {
  const item = "struct Handle<'a>(&'a [u8; 16]);";
  const source = `#[charon::opaque]\n${item}\nstruct Unrelated;`;
  assert.equal(annotatedItem(source, source.indexOf('charon'), 'control.rs').text, item);
});

test('annotated function extent includes generic return type commas', () => {
  const item = `fn helper<'a, F: Fn() -> Result<(), Error>>(callback: F) -> Result<(), Error> {
    callback()
}`;
  const source = `#[charon::exclude]\n${item}\nfn unrelated() {}`;
  assert.equal(annotatedItem(source, source.indexOf('charon'), 'control.rs').text, item);
});

test('five reviewed opaque implementations retain exact source and body identities', () => {
  assert.deepEqual(sourceFiles, inheritedOpaqueSources);
  assert.deepEqual(Object.fromEntries(Object.entries(inheritedOpaqueBodies).map(([file, bodies]) =>
    [file, Object.keys(bodies).sort()])), {
    'crates/noble-kernel/src/types/impls.rs': ['clone_stack', 'debug_stack'],
    'crates/noble-kernel/src/shapes/impls.rs': ['clone_parts', 'debug_parts', 'debug_slots'],
  });
  sourcePolicy(sourceFiles, readSource);
});

for (const file of Object.keys(inheritedOpaqueSources)) {
  test(`changed opaque model source refuses review: ${file}`, () => {
    const changed = { ...sourceFiles, [file]: hash(`${sources[file]}\n`) };
    assert.throws(() => sourcePolicy(changed, readSource),
      error => error.message === `M4-OPAQUE-SOURCE: ${file}`);
  });
  for (const name of Object.keys(inheritedOpaqueBodies[file])) {
    test(`changed opaque helper body refuses review: ${name}`, () => {
      const source = sources[file];
      const declaration = source.indexOf(`fn ${name}(`);
      assert.notEqual(declaration, -1);
      const index = source.indexOf('let mut index = 0;', declaration);
      assert.notEqual(index, -1);
      const changed = `${source.slice(0, index)}let mut index = 1;${source.slice(index + 'let mut index = 0;'.length)}`;
      assert.throws(() => sourcePolicy(sourceFiles,
        selected => selected === file ? changed : sources[selected]),
      error => error.message === `M4-OPAQUE-MODEL: ${file}::${name}`);
    });
  }
}

test('test-only oracle modules are excluded from production source selection', () => {
  assert.match(effectOracleSource, /#\[cfg\(test\)\]\s*\nmod oracle;/);
  assert.doesNotThrow(() => sourcePolicy(sourcePolicyFiles, readPolicySource));
});

test('feature-selected source remains refused', () => {
  const changed = effectOracleSource.replace('#[cfg(test)]\nmod oracle;',
    '#[cfg(feature = "oracle")]\nmod oracle;');
  assert.notEqual(changed, effectOracleSource);
  assert.throws(() => sourcePolicy({ ...sourcePolicyFiles, [effectOracleFile]: hash(changed) },
    file => file === effectOracleFile ? changed : readPolicySource(file)),
  error => error.message === `M4-SOURCE-FILTER: ${effectOracleFile}: unreviewed conditional source selection`);
});

test('local opaque review is exactly ten frontend functions, four types and one backend function', () => {
  assert.equal(reviewedOpaque.contracts.length, 10);
  assert.equal(reviewedOpaqueTypes.contracts.length, 4);
  assert.equal(reviewedOpaque.wasm.length, 1);
  assert.deepEqual(reviewedOpaqueTypes.wasm, []);
  const contracts = Object.entries(reviewedAnnotations).filter(([file]) => file.startsWith('crates/noble-contracts/'))
    .flatMap(([, rows]) => rows);
  for (const [kind, count] of [['rename', 10], ['opaque', 14], ['exclude', 10]]) {
    assert.equal(contracts.filter(row => row.annotation.split('(')[0] === kind).length, count);
  }
  assert.doesNotThrow(() => sourcePolicy(annotationFiles, readAnnotationSource));
});

for (const [file, rows] of Object.entries(reviewedAnnotations)) {
  for (const [index, row] of rows.entries()) {
    test(`changed complete annotated item refuses review: ${file}/${index}/${row.annotation}`, () => {
      const source = annotationSources[file];
      const matches = [...source.matchAll(/#\[\s*charon\s*::/g)];
      const match = matches[index];
      const parsed = annotatedItem(source, match.index + match[0].indexOf('charon'), file);
      assert.equal(parsed.text.split('\n').length, row.lines);
      assert.equal(hash(parsed.text), row.sha256);
      const end = source.indexOf(parsed.text, match.index) + parsed.text.length - 1;
      const changed = `${source.slice(0, end)} /* changed */${source.slice(end)}`;
      assert.throws(() => sourcePolicy(annotationFiles,
        selected => selected === file ? changed : readAnnotationSource(selected)),
      error => /^M4-(?:SOURCE-FILTER|OPAQUE-MODEL):/.test(error.message));
    });
  }
}

test('new opaque items and changed variant suffixes are not blanket exemptions', () => {
  const changed = `${effectOracleSource}\n#[charon::opaque]\nfn newly_hidden() {}`;
  assert.throws(() => sourcePolicy(annotationFiles,
    file => file === effectOracleFile ? changed : readAnnotationSource(file)),
  error => error.message === `M4-SOURCE-FILTER: ${effectOracleFile}: unreviewed Charon annotation set`);
  const alteredSuffix = annotationSources[intrinsicFile].replace('variants_suffix("Code")', 'variants_suffix("Other")');
  assert.throws(() => sourcePolicy(annotationFiles,
    file => file === intrinsicFile ? alteredSuffix : readAnnotationSource(file)),
  error => error.message === `M4-SOURCE-FILTER: ${intrinsicFile}: unreviewed Charon annotation set`);
});

test('macro-fed Charon exclusion refuses unreviewed source selection', () => {
  const changed = `${effectOracleSource}
macro_rules! extraction_hidden {
    ($attribute:meta) => {
        #[$attribute]
        pub fn unaccounted_function() -> bool { false }
    };
}
extraction_hidden!(charon::exclude);
`;
  assert.throws(() => sourcePolicy(annotationFiles,
    file => file === effectOracleFile ? changed : readAnnotationSource(file)),
  error => error.message.startsWith(`M4-SOURCE-FILTER: ${effectOracleFile}:`));
});

test('spaced Charon exclusion refuses unreviewed source selection', () => {
  const changed = `${effectOracleSource}\n# [charon::exclude]\npub fn unaccounted_function() -> bool { false }`;
  assert.throws(() => sourcePolicy(annotationFiles,
    file => file === effectOracleFile ? changed : readAnnotationSource(file)),
  error => error.message.startsWith(`M4-SOURCE-FILTER: ${effectOracleFile}:`));
});

test('whitespace inside reviewed variant suffix refuses review', () => {
  const source = annotationSources[intrinsicFile];
  const changed = source.replace('variants_suffix("Code")', 'variants_suffix("Co de")');
  assert.notEqual(changed, source);
  assert.throws(() => sourcePolicy(annotationFiles,
    file => file === intrinsicFile ? changed : readAnnotationSource(file)),
  error => error.message === `M4-SOURCE-FILTER: ${intrinsicFile}: unreviewed Charon annotation set`);
});

for (const item of ['enum Code {', 'enum Sort {', 'enum Proposition {']) {
  test(`missing or production-selected Debug derive refuses review: ${item}`, () => {
    const source = annotationSources[intrinsicFile];
    const end = source.indexOf(item);
    const at = source.lastIndexOf('#[cfg_attr(test, derive(Debug))]', end);
    assert.notEqual(at, -1);
    for (const replacement of ['', '#[cfg_attr(feature = "debug", derive(Debug))]']) {
      const changed = source.slice(0, at) + replacement + source.slice(at + '#[cfg_attr(test, derive(Debug))]'.length);
      assert.throws(() => sourcePolicy(annotationFiles,
        file => file === intrinsicFile ? changed : readAnnotationSource(file)),
      error => error.message.startsWith(`M4-SOURCE-FILTER: ${intrinsicFile}:`));
    }
  });
}

test('comments and string contents cannot introduce source-selection attributes', () => {
  const changed = `${effectOracleSource}\n// #[charon::opaque]\nconst TEXT: &str = "#[cfg(feature = \\\"hidden\\\")]";`;
  assert.doesNotThrow(() => sourcePolicy(annotationFiles,
    file => file === effectOracleFile ? changed : readAnnotationSource(file)));
});
