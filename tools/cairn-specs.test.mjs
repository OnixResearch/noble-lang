// Filesystem-shell regressions. These tests do not execute Noble or check proofs.
import { afterEach, test } from 'bun:test';
import assert from 'node:assert/strict';
import { copyFileSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { SPEC_SLUGS, canonicalPath, toCairn, toLegacy, regenerate, rebaseLinks } from './cairn-specs.mjs';

const temporaryRoots = [];
afterEach(() => {
  for (const root of temporaryRoots.splice(0)) rmSync(root, { recursive: true, force: true });
});

function fixture() {
  const root = mkdtempSync(path.join(tmpdir(), 'noble-cairn-specs-'));
  temporaryRoots.push(root);
  mkdirSync(path.join(root, 'tools'));
  mkdirSync(path.join(root, 'specs'));
  copyFileSync(new URL('./cairn-specs.mjs', import.meta.url), path.join(root, 'tools/cairn-specs.mjs'));
  const documents = Object.keys(SPEC_SLUGS).map((name, index) => {
    const nativePath = canonicalPath(name);
    const canonical = toCairn(`# Fixture\n\n**B-FIXTURE-${index + 1}.** The checker MUST reject invalid input.\n`, name, []);
    mkdirSync(path.dirname(path.join(root, nativePath)), { recursive: true });
    writeFileSync(path.join(root, nativePath), canonical);
    writeFileSync(path.join(root, 'specs', name),
      `<!-- Generated compatibility view. Edit ${nativePath} instead. -->\n` + toLegacy(canonical, name));
    return { path: '../' + nativePath, compatibility_path: name, cairn_id: SPEC_SLUGS[name] };
  });
  writeFileSync(path.join(root, 'specs/spec-family.json'), JSON.stringify({
    lifecycle: '.cairn', normative_documents: documents, scenario_files: [],
  }));
  const files = documents.flatMap(doc => [canonicalPath(doc.compatibility_path), `specs/${doc.compatibility_path}`]);
  return {
    root, files,
    read: file => readFileSync(path.join(root, file), 'utf8'),
    write: (file, text) => writeFileSync(path.join(root, file), text),
    snapshot: () => files.map(file => [file, readFileSync(path.join(root, file), 'utf8')]),
    run: () => spawnSync(process.execPath, ['tools/cairn-specs.mjs', '--write-views'], {
      cwd: root, encoding: 'utf8', timeout: 10_000,
    }),
  };
}

const START = '<!-- cairn:scenario-links:start -->';
const END = '<!-- cairn:scenario-links:end -->';
for (const fence of ['```', '~~~~']) {
  for (const padding of ['', '\n']) {
    test(`--write-views preserves ${fence} marker examples with padding ${JSON.stringify(padding)}`, () => {
      const f = fixture();
      const name = 'CALCULATOR.md';
      const file = canonicalPath(name);
      const example = `\n${fence}markdown\nBefore the region.\n${padding}${START}\nAUTHORED EXAMPLE MUST SURVIVE\n${END}\n${padding}After the region.\n${fence}\n`;
      f.write(file, f.read(file) + example);
      const result = f.run();
      assert.equal(result.status, 0, result.stderr);
      assert.ok(f.read(file).includes(example), 'canonical fenced content stays exact');
      assert.ok(f.read(`specs/${name}`).includes(example), 'compatibility fenced content stays exact');
      const before = f.snapshot();
      assert.equal(f.run().status, 0);
      assert.deepEqual(f.snapshot(), before);
    });
  }
  test(`${fence} link-like code stays literal during regeneration`, () => {
    const name = 'CORE-BOOTSTRAP.md';
    const example = `\n${fence}markdown\n${START}\n[literal](100%not-a-URI.json)\n[unicode](café.json)\n${END}\n${fence}\n`;
    const source = '# Fixture\n\n**B-FIXTURE-01.** The checker MUST reject invalid input.\n' + example;
    const canonical = toCairn(source, name, []);
    assert.ok(canonical.includes(example));
    assert.equal(toLegacy(canonical, name), source);
    assert.equal(regenerate(canonical, name, []), canonical);
  });
  test(`${fence} literal markers and links survive conversion round trips`, () => {
    const name = 'CORE-BOOTSTRAP.md';
    const source = `# Fixture\n\n**B-FIXTURE-01.** The checker MUST reject invalid input.\n\n${fence}markdown\n${START}\n[artifact](conformance/probe%20file.json)\n${END}\n${fence}\n`;
    const canonical = toCairn(source, name, []);
    assert.equal(toLegacy(canonical, name), source);
    assert.equal(regenerate(canonical, name, []), canonical);
    // Unmatched markers are also literal inside a fence, not malformed metadata.
    for (const marker of [START, END]) {
      const example = source + `\n${fence}markdown\n${marker}\n${fence}\n`;
      assert.equal(toLegacy(toCairn(example, name, []), name), example);
    }
  });
}

test('a purpose-wrapper example survives even when the native document has no generated wrapper', () => {
  const name = 'CORE-BOOTSTRAP.md';
  const imported = toCairn('# Fixture\n\n**B-FIXTURE-01.** The checker MUST reject invalid input.\n', name, []);
  const wrapper = imported.match(/\n<!-- cairn:purpose:start -->[\s\S]*?<!-- cairn:purpose:end -->\n/)[0];
  const example = '\n```markdown\n' + wrapper + '\n```\n';
  const native = '# Fixture\n\n### Requirement: B-FIXTURE-01\nr[B-FIXTURE-01]\n\nThe checker MUST reject invalid input.\n' + example;
  assert.ok(toLegacy(native, name).includes(example));
  const canonical = regenerate(native, name, []);
  assert.ok(canonical.includes(example));
  assert.equal(regenerate(canonical, name, []), canonical);
});

for (const ending of ['', '\n', '\n\n']) {
  test(`conversion preserves authored final whitespace ${JSON.stringify(ending)}`, () => {
    const name = 'CORE-BOOTSTRAP.md';
    const source = '# Fixture\n\n**B-FIXTURE-01.** The checker MUST reject invalid input.' + ending;
    assert.equal(toLegacy(toCairn(source, name, []), name), source);
  });
}

test('--write-views preserves encoded destinations and real generated scenario links', () => {
  const f = fixture();
  const name = 'CALCULATOR.md';
  const file = canonicalPath(name);
  const leaf = 'probe%20%23%25%28file%29.json';
  const link = `[artifact](../../../specs/${leaf}#details)`;
  f.write(file, f.read(file) + '\n' + link + '\n');
  const family = JSON.parse(f.read('specs/spec-family.json'));
  family.scenario_files = ['probe file.json'];
  f.write('specs/spec-family.json', JSON.stringify(family));
  f.write('specs/probe file.json', JSON.stringify({ cases: [
    { id: 'FIXTURE-01', requirements: ['B-FIXTURE-12'], profile: 'Fixture', kind: 'review' },
  ] }));
  const result = f.run();
  assert.equal(result.status, 0, result.stderr);
  assert.ok(f.read(file).includes(link));
  assert.ok(f.read(`specs/${name}`).includes(`[artifact](${leaf}#details)`));
  assert.ok(f.read(file).includes('[FIXTURE-01](../../../specs/probe%20file.json)'));
  const before = f.snapshot();
  assert.equal(f.run().status, 0);
  assert.deepEqual(f.snapshot(), before);
});

for (const leaf of ['probe%20file.json', 'probe%23file.json', 'probe%25file.json', 'probe%28file%29.json', 'probe%3Ffile.json', 'caf%C3%A9.json']) {
  test(`link rebasing preserves encoded path ${leaf}`, () => {
    const text = `[artifact](conformance/${leaf}#details)`;
    const canonical = rebaseLinks(text, 'specs/CORE-BOOTSTRAP.md', canonicalPath('CORE-BOOTSTRAP.md'), new Map());
    assert.equal(canonical, `[artifact](../../../specs/conformance/${leaf}#details)`);
    assert.equal(rebaseLinks(canonical, canonicalPath('CORE-BOOTSTRAP.md'), 'specs/CORE-BOOTSTRAP.md', new Map()), text);
  });
}
test('link rebasing preserves external URLs and local anchors', () => {
  const text = '[external](https://example.invalid/probe%20file#details) [anchor](#details)';
  assert.equal(rebaseLinks(text, 'specs/CORE-BOOTSTRAP.md', canonicalPath('CORE-BOOTSTRAP.md'), new Map()), text);
});

const addition = '\n### Requirement: B-NEW-01\nr[B-NEW-01]\n\nThe checker MUST reject unsupported input.\n\n#### Scenario: Unsupported input\n\n- GIVEN unsupported input\n- WHEN acceptance checks it\n- THEN acceptance rejects it without execution\n';

test('--write-views preserves a native requirement without a legacy label or ledger link', () => {
  const f = fixture();
  const file = canonicalPath('CALCULATOR.md');
  f.write(file, f.read(file) + addition);
  const beforeIds = [...f.read(file).matchAll(/^r\[([^\]]+)\]$/gm)].map(match => match[1]);
  const result = f.run();
  assert.equal(result.status, 0, result.stderr);
  const after = f.read(file);
  assert.deepEqual([...after.matchAll(/^r\[([^\]]+)\]$/gm)].map(match => match[1]), beforeIds);
  assert.ok(after.includes('The checker MUST reject unsupported input.'));
  assert.ok(after.includes('#### Scenario: Unsupported input'));
  assert.ok(f.read('specs/CALCULATOR.md').includes('**B-NEW-01.**'));
  const firstOutput = f.snapshot();
  const second = f.run();
  assert.equal(second.status, 0, second.stderr);
  assert.deepEqual(f.snapshot(), firstOutput, 'a second regeneration must not change the output');
});

for (const [name, corrupt, diagnostic] of [
  ['mismatched legacy label', text => text.replace('**B-FIXTURE-12.**', '**B-WRONG-01.**'), /identity mismatch/],
  ['mismatched native marker', text => text.replace('r[B-FIXTURE-12]', 'r[B-WRONG-01]'), /marker identity mismatch/],
  ['missing native marker', text => text.replace('r[B-FIXTURE-12]\n', ''), /marker/],
  ['duplicate native ID', text => text + addition + addition, /duplicate requirement/],
  ['cross-document duplicate', text => text + addition.replaceAll('B-NEW-01', 'B-FIXTURE-1'), /duplicate requirement/],
  ['nested generated region', text => text.replace(START, START + '\n' + START), /nested scenario region/],
  ['unclosed generated region', text => text.replace(END, ''), /unclosed scenario region/],
  ['orphan generated end', text => text + '\n' + END + '\n', /orphan scenario region end/],
]) {
  test(`--write-views changes no files after ${name}`, () => {
    const f = fixture();
    // The first document needs a new compatibility view. The last document is invalid.
    const first = canonicalPath('SPEC-0001.md');
    f.write(first, f.read(first).replace('reject invalid input', 'reject malformed input'));
    const last = canonicalPath('CALCULATOR.md');
    f.write(last, corrupt(f.read(last)));
    const before = f.snapshot();
    const result = f.run();
    assert.equal(result.status, 1, result.stderr);
    assert.match(result.stderr, diagnostic);
    assert.deepEqual(f.snapshot(), before, 'validation failure must precede every spec/view write');
  });
}
