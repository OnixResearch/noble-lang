import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { inheritedOpaqueBodies, inheritedOpaqueSources, sourcePolicy } from './accounting.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const sources = Object.fromEntries(Object.keys(inheritedOpaqueSources).map(file =>
  [file, fs.readFileSync(path.join(root, file), 'utf8')]));
const hash = text => crypto.createHash('sha256').update(text).digest('hex');
const sourceFiles = Object.fromEntries(Object.entries(sources).map(([file, text]) =>
  [file, hash(text)]));
const readSource = file => sources[file];

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
