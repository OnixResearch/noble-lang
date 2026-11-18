import { afterEach, test } from 'bun:test';
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, readFileSync, readdirSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { buildSite, renderMarkdown, resolveLink, sliceBetween, stripScenarioRegions } from './site.mjs';

const scratch = [];
afterEach(() => { for (const directory of scratch.splice(0)) rmSync(directory, { recursive: true, force: true }); });
const revision = 'a'.repeat(40);
function fixture() {
  const directory = mkdtempSync(path.join(tmpdir(), 'noble-site-'));
  scratch.push(directory);
  const root = path.join(directory, 'source');
  const out = path.join(directory, 'public');
  function write(file, text) {
    mkdirSync(path.dirname(path.join(root, file)), { recursive: true });
    writeFileSync(path.join(root, file), text);
  }
  write('README.md', '# Noble\n\nA bounded draft.\n\n## Programs as values\n\nSTACK_STEPS\n\n40 2 quote [ + ] compose run\n\n## Try it\n\nUse Nix.\n\n```sh\ngit clone example\nnix build .#noble\n```\n\n### Editor transport reports\n\nEDITOR_ONLY\n\n**Host-local Wasmtime source.** The host must already have the store path.\n\n### Independent Mathlib comparison for implemented I64 arithmetic\n\nMATHLIB_ONLY\n\n### Persistent sessions\n\nSESSION_LINE\n\n### Experimental calculator (opt-in; acceptance not established)\n\nCALC_ONLY\n\n## What works today\n\nCore-Bootstrap only.\n\n## Verification\n\nVERIFY_ONLY\n\n## Working on Noble\n\nnix develop\n\n### Building and publishing the draft website\n\nSITE_DOCS_ONLY\n');
  write('specs/spec-family.json', JSON.stringify({ revision: 'fixture-draft', normative_documents: [
    { id: 'SPEC-0001', cairn_id: 'language', path: '../.cairn/specs/language/spec.md', compatibility_path: 'SPEC-0001.md' },
    { id: 'SPEC-B001', cairn_id: 'core-bootstrap', path: '../.cairn/specs/core-bootstrap/spec.md', compatibility_path: 'CORE-BOOTSTRAP.md' },
  ] }));
  write('.cairn/specs/language/spec.md', '# Language draft\n\n## Values\n\nCanonical text. [Core](../core-bootstrap/spec.md#scope)\n\n## 3. Expression syntax and source units\n\ndef square [ dup * ]\n\n<!-- cairn:scenario-links:start -->\nSCENARIO_ONLY\n<!-- cairn:scenario-links:end -->\n\n```md\n<!-- cairn:scenario-links:start -->\nFENCED_MARKER\n<!-- cairn:scenario-links:end -->\n```\n\n## 4. Values, stacks, and type notation\n\n### 4.1 Stacks\n\nTop on the right.\n\n## 6. Program construction and execution\n\nA quotation is a program.\n\n## 7. Stack operations, data, and control\n\nLater.\n\n# Appendix A \u2014 Worked examples (informative)\n\n## A.1 Construction is not invocation\n\n41 [ 1 + ]\n\n## A.3 Runtime capture\n\nquote\n\n# Appendix B \u2014 Source traceability (informative)\n\nTrace.\n');
  write('docs/note.md', '# Note\n\nA design note. [Language](../.cairn/specs/language/spec.md#values)\n');
  write('crates/noble-cli/src/core/runtime/config.json', '{}\n');
  write('.cairn/specs/core-bootstrap/spec.md', '# Bootstrap\n\n## Scope\n\n[Language](../../../specs/SPEC-0001.md#values)\n');
  write('specs/README.md', '# Family guide\n');
  write('specs/STATUS.json', '{}');
  write('specs/ROADMAP.md', '# Roadmap\n');
  // The generated views deliberately disagree; the builder must never render them.
  write('specs/SPEC-0001.md', '# WRONG GENERATED CONTENT\n');
  write('specs/CORE-BOOTSTRAP.md', '# WRONG GENERATED CONTENT\n');
  write('tools/site.css', readFileSync(new URL('./site.css', import.meta.url)));
  const routes = new Map([['.cairn/specs/language/spec.md', '/spec/language/'], ['specs/SPEC-0001.md', '/spec/language/']]);
  return { root, out, directory, write, context: { root, source: '.cairn/specs/language/spec.md', routes, revision, links: [] } };
}

test('section slices ignore markers inside fences and keep fenced scenario markers', () => {
  const markdown = '## Start\n\n```md\n## End\n```\n\nkeep\n\n## End\n';
  assert.equal(sliceBetween(markdown, '## Start', '## End'), '## Start\n\n```md\n## End\n```\n\nkeep');
  const regions = 'before\n<!-- cairn:scenario-links:start -->\nhide\n<!-- cairn:scenario-links:end -->\nafter\n\n```md\n<!-- cairn:scenario-links:start -->\nFENCED_MARKER\n<!-- cairn:scenario-links:end -->\n```\n';
  const stripped = stripScenarioRegions(regions);
  assert.equal(stripped.includes('hide'), false);
  assert.ok(stripped.includes('before'));
  assert.ok(stripped.includes('after'));
  assert.ok(stripped.includes('FENCED_MARKER'));
  assert.throws(() => stripScenarioRegions('<!-- cairn:scenario-links:start -->\nopen'), /unclosed/);
});

test('GFM preserves code, tables, nested lists, entities, duplicate headings and reference links', async () => {
  const f = fixture();
  const markdown = '# Code & types\n\n## Repeat\n\n## Repeat\n\n```noble\n[ a < b ] & \"x\"\n[literal](javascript:bad)\n```\n\n`Pair<I64,I64>`\n\n| Type | Role |\n|---|---|\n| I64 | value |\n\n1. Outer\n   - Nested **strong**\n\nA hard break.  \nNext line.\n\n[reference][target]\n\n[target]: #repeat-1\n';
  const rendered = await renderMarkdown(markdown, f.context);
  assert.deepEqual([...rendered.ids], ['code-types', 'repeat', 'repeat-1']);
  assert.ok(rendered.html.includes('<pre><code class="language-noble">[ a &lt; b ] &amp; &quot;x&quot;'));
  assert.ok(rendered.html.includes('[literal](javascript:bad)'));
  assert.ok(rendered.html.includes('<code>Pair&lt;I64,I64&gt;</code>'));
  assert.ok(rendered.html.includes('<table>') && rendered.html.includes('<ul>') && rendered.html.includes('<br'));
  assert.ok(rendered.html.includes('href="/spec/language/#repeat-1"'));
});

test('canonical routes and aliases stay local; other repository sources retain revision and fragment', () => {
  const f = fixture();
  assert.equal(resolveLink('../../../specs/SPEC-0001.md#values', f.context), '/spec/language/#values');
  assert.equal(resolveLink('#values', f.context), '/spec/language/#values');
  assert.equal(resolveLink('../../../README.md?plain=1#try-it', f.context), `https://github.com/OnixResearch/noble-lang/blob/${revision}/README.md?plain=1#try-it`);
  assert.equal(resolveLink('https://example.org/a?q=one#two', f.context), 'https://example.org/a?q=one#two');
  assert.equal(resolveLink('../../../specs', f.context), `https://github.com/OnixResearch/noble-lang/tree/${revision}/specs`);
});

test('unsafe schemes, controls, traversal, absent sources and escaping symlinks fail closed', () => {
  const f = fixture();
  for (const href of ['javascript:alert(1)', 'data:text/html,test', '//evil.invalid', 'https:\\evil.invalid', 'java\nscript:x', '/etc/passwd', '../../../../outside', '%2Fetc/passwd', '../../../missing.md']) {
    assert.throws(() => resolveLink(href, f.context), undefined, href);
  }
  writeFileSync(path.join(f.directory, 'outside.md'), 'private');
  symlinkSync(path.join(f.directory, 'outside.md'), path.join(f.root, 'escape.md'));
  assert.throws(() => resolveLink('../../../escape.md', f.context), /symlink outside/);
});

test('raw HTML loses executable attributes; task checkboxes remain disabled', async () => {
  const f = fixture();
  const rendered = await renderMarkdown('<p onclick="evil()" style="color:red">Safe</p>\n\n- [x] Done\n- [ ] Open\n', f.context);
  assert.ok(!rendered.html.includes('onclick') && !rendered.html.includes('style='));
  assert.ok(rendered.html.includes('type="checkbox"') && rendered.html.includes('disabled'));
});

function files(root, prefix = '') {
  return readdirSync(path.join(root, prefix), { withFileTypes: true }).flatMap(entry => entry.isDirectory() ? files(root, prefix + entry.name + '/') : [prefix + entry.name]).sort();
}

test('complete static builds are byte reproducible and render only canonical sources', async () => {
  const f = fixture();
  await buildSite({ root: f.root, out: f.out, revision });
  const second = path.join(f.directory, 'second');
  await buildSite({ root: f.root, out: second, revision });
  assert.deepEqual(files(f.out), files(second));
  for (const file of files(f.out)) assert.deepEqual(readFileSync(path.join(f.out, file)), readFileSync(path.join(second, file)), file);
  const language = readFileSync(path.join(f.out, 'spec/language/index.html'), 'utf8');
  assert.ok(language.includes('Canonical text.') && !language.includes('WRONG GENERATED CONTENT'));
  assert.ok(language.includes('href="/spec/core-bootstrap/#scope"'));
  const home = readFileSync(path.join(f.out, 'index.html'), 'utf8');
  const guide = readFileSync(path.join(f.out, 'guide/index.html'), 'utf8');
  const start = readFileSync(path.join(f.out, 'start/index.html'), 'utf8');
  const note = readFileSync(path.join(f.out, 'docs/note/index.html'), 'utf8');
  const playground = readFileSync(path.join(f.out, 'playground/index.html'), 'utf8');
  assert.ok(home.includes('STACK_STEPS') && home.includes('41 [ 1 + ]'));
  assert.ok(!home.includes('EDITOR_ONLY') && !home.includes('MATHLIB_ONLY') && !home.includes('<script'));
  assert.ok(guide.includes('def square [ dup * ]') && guide.includes('Top on the right') && guide.includes('41 [ 1 + ]'));
  assert.ok(guide.includes('FENCED_MARKER') && !guide.includes('SCENARIO_ONLY'));
  assert.ok(language.includes('SCENARIO_ONLY') && language.includes('Canonical text.'));
  assert.ok(start.includes('git clone example') && start.includes('Host-local Wasmtime') && start.includes('SESSION_LINE'));
  assert.ok(start.includes('Core-Bootstrap only') && start.includes('nix develop'));
  assert.ok(!start.includes('EDITOR_ONLY') && !start.includes('MATHLIB_ONLY') && !start.includes('CALC_ONLY') && !start.includes('SITE_DOCS_ONLY'));
  assert.ok(note.includes('A design note.') && note.includes('href="/spec/language/#values"'));
  assert.ok(playground.includes('does not compile or run Noble') && playground.includes('/issues/137'));
  for (const file of files(f.out).filter(name => name.endsWith('.html'))) assert.ok(!readFileSync(path.join(f.out, file), 'utf8').includes('<script'), file);
  const cssFile = files(f.out).find(name => name.startsWith('assets/site-'));
  assert.ok(readFileSync(path.join(f.out, cssFile), 'utf8').includes('prefers-color-scheme: dark'));
  const redirects = readFileSync(path.join(f.out, 'cloudflare-bulk-redirects.csv'), 'utf8');
  assert.deepEqual(redirects.trim().split(','), ['www.noble-lang.org/', 'https://noble-lang.org/', '301', 'TRUE', 'FALSE', 'TRUE', 'TRUE']);
  assert.ok(readFileSync(path.join(f.out, '_headers'), 'utf8').includes("frame-ancestors 'none'"));
  await assert.rejects(buildSite({ root: f.root, out: f.out, revision }), /output must be empty/);
});

test('unresolved canonical anchors and noncanonical manifests are rejected', async () => {
  const f = fixture();
  f.write('.cairn/specs/language/spec.md', '# Language\n\n## Values\n\n[bad](../core-bootstrap/spec.md#missing)\n\n## 3. Expression syntax and source units\n\nWords.\n\n## 4. Values, stacks, and type notation\n\nTypes.\n\n## 6. Program construction and execution\n\nPrograms.\n\n## 7. Stack operations, data, and control\n\nLater.\n\n# Appendix A \u2014 Worked examples (informative)\n\n## A.1 Construction is not invocation\n\n41 [ 1 + ]\n\n## A.3 Runtime capture\n\nquote\n\n# Appendix B \u2014 Source traceability (informative)\n\nTrace.\n');
  await assert.rejects(buildSite({ root: f.root, out: f.out, revision }), /unresolved published anchor/);
  f.write('specs/spec-family.json', JSON.stringify({ normative_documents: [{ cairn_id: 'language', path: 'SPEC-0001.md', compatibility_path: 'SPEC-0001.md' }] }));
  await assert.rejects(buildSite({ root: f.root, out: f.out, revision }), /noncanonical spec/);
});
