#!/usr/bin/env bun
// Noble's Markdown adapter for native Cairn specs. No Noble execution or proof checking.
// Pure conversion functions precede the filesystem shell. Cairn owns lifecycle validation.
import { existsSync, readFileSync, mkdirSync, readdirSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import assert from 'node:assert/strict';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
export const SPEC_SLUGS = {
  'SPEC-0001.md': 'language',
  'SAFETY.md': 'safety',
  'WIT-WASI.md': 'wit-wasi',
  'VERIFICATION.md': 'verification',
  'VERIFICATION-TOOLCHAIN.md': 'verification-toolchain',
  'PROGRAM-CONTRACTS.md': 'program-contracts',
  'CORE-BOOTSTRAP.md': 'core-bootstrap',
  'BACKEND-EXPERIMENTS.md': 'backend-experiments',
  'RESOURCE-ADAPTERS.md': 'resource-adapters',
  'EVIDENCE.md': 'evidence',
  'DEVELOPER-EXPERIENCE.md': 'developer-experience',
  'CALCULATOR.md': 'calculator',
};
export const canonicalPath = name => {
  if (!Object.hasOwn(SPEC_SLUGS, name)) throw new Error(`unknown spec: ${name}`);
  return `.cairn/specs/${SPEC_SLUGS[name]}/spec.md`;
};
const mapping = new Map(Object.keys(SPEC_SLUGS).map(name => [`specs/${name}`, canonicalPath(name)]));
const reverseMapping = new Map([...mapping].map(([a, b]) => [b, a]));
const REQUIREMENT = /^\*\*([A-Z][A-Z0-9-]*-\d+)\.\*\*/;
const START = '<!-- cairn:scenario-links:start -->';
const END = '<!-- cairn:scenario-links:end -->';
const WRAPPER = '\n<!-- cairn:purpose:start -->\n## Purpose\n\nThis accepted specification records Noble draft contracts, not completed implementation.\nOriginal requirement IDs, explanatory prose, examples, and open decisions remain authoritative.\nScenario clauses refer to unexecuted designs in the conformance ledger.\n\n## Requirements\n<!-- cairn:purpose:end -->\n';

export function rebaseLinks(text, from, to, targets) {
  return text.replace(/(\[[^\]\n]*\]\()([^\s)]+)(\))/g, (match, prefix, target, suffix) => {
    if (/^[a-z][a-z0-9+.-]*:/i.test(target) || target.startsWith('#')) return match;
    const [file, ...fragment] = target.split('#');
    const resolved = path.posix.normalize(path.posix.join(path.posix.dirname(from), decodeURIComponent(file)));
    const destination = targets.get(resolved) ?? resolved;
    const relative = path.posix.relative(path.posix.dirname(to), destination);
    return `${prefix}${relative}${fragment.length ? '#' + fragment.join('#') : ''}${suffix}`;
  });
}

function scenarioLinks(id, cases, destination) {
  const linked = cases.filter(c => c.requirements.includes(id));
  const lines = [START];
  if (!linked.length) {
    lines.push('Test design remains open for this requirement. No scenario or execution evidence is supplied.');
  }
  for (const c of linked) {
    const file = path.posix.relative(path.posix.dirname(destination), `specs/${c.file}`);
    const reference = `[${c.id}](${file})`;
    lines.push(`#### Scenario: ${c.id} for ${id}`, '',
      `- GIVEN the \`${c.profile}\` profile and every field of \`input\` in ${reference}`,
      `- WHEN the \`${c.kind}\` procedure for case \`${c.id}\` runs against those inputs`,
      `- THEN the observations match every field of \`expected\` in case \`${c.id}\``, '',
      `This is a scenario design, not an execution result. The case's \`state\` and \`evidence\` fields record its status.`, '');
  }
  lines.push(END);
  return '\n' + lines.join('\n') + '\n';
}

export function toCairn(source, name, cases) {
  if (source.includes('<!-- cairn:') || /^r\[/m.test(source)) throw new Error(`already converted input: ${name}`);
  if (!source.startsWith('# ')) throw new Error(`missing title: ${name}`);
  const destination = canonicalPath(name);
  const lines = source.split('\n');
  const output = [];
  const seen = new Set();
  let active;
  let fenced = false;
  for (const [index, line] of lines.entries()) {
    const boundary = /^\s*(```|~~~)/.test(line);
    const id = !fenced && !boundary ? line.match(REQUIREMENT)?.[1] : undefined;
    if (!fenced && !boundary && active && (id || /^#{1,6} /.test(line))) {
      output.push(scenarioLinks(active, cases, destination));
      active = undefined;
    }
    if (id) {
      if (seen.has(id)) throw new Error(`duplicate requirement: ${id}`);
      seen.add(id);
      output.push(`### Requirement: ${id}\nr[${id}]\n`);
      active = id;
    }
    output.push(line);
    if (index === 0) output.push(WRAPPER);
    if (boundary) fenced = !fenced;
  }
  if (fenced) throw new Error(`unclosed code fence: ${name}`);
  if (!seen.size) throw new Error(`no requirements: ${name}`);
  if (active) output.push(scenarioLinks(active, cases, destination));
  // Scenario links already use destination-relative paths; rebase source prose first.
  const rendered = output.join('\n');
  const regions = rendered.split(/(\n<!-- cairn:scenario-links:start -->[\s\S]*?<!-- cairn:scenario-links:end -->\n)/g);
  return regions.map(region => region.startsWith('\n' + START) ? region
    : rebaseLinks(region, `specs/${name}`, destination, mapping)).join('');
}

export function toLegacy(markdown, name) {
  const stripped = markdown
    .replace('\n' + WRAPPER, '')
    .replace(/^### Requirement: ([A-Z][A-Z0-9-]*-\d+)\nr\[\1\]\n\n/gm, '')
    .replace(/\n\n<!-- cairn:scenario-links:start -->(?:(?!<!-- cairn:scenario-links:end -->)[\s\S])*<!-- cairn:scenario-links:end -->\n$/, '')
    .replace(/\n<!-- cairn:scenario-links:start -->[\s\S]*?<!-- cairn:scenario-links:end -->\n\n?/g, '');
  return rebaseLinks(stripped, canonicalPath(name), `specs/${name}`, reverseMapping);
}

function view(markdown, name) {
  return `<!-- Generated compatibility view. Edit ${canonicalPath(name)} instead. -->\n` + toLegacy(markdown, name);
}

export function checkConversion(documents, cases) {
  const errors = [];
  const identities = new Set();
  for (const { name, canonical, compatibility } of documents) {
    const legacy = toLegacy(canonical, name);
    try {
      if (toCairn(legacy, name, cases) !== canonical) errors.push(`Cairn structure or scenario links stale: ${canonicalPath(name)}`);
    } catch (error) { errors.push(error.message); }
    if (view(canonical, name) !== compatibility) errors.push(`compatibility view stale: specs/${name}`);
    for (const match of legacy.matchAll(/^\*\*([A-Z][A-Z0-9-]*-\d+)\.\*\*/gm)) {
      if (identities.has(match[1])) errors.push(`duplicate requirement: ${match[1]}`);
      identities.add(match[1]);
    }
  }
  for (const c of cases) {
    for (const id of c.requirements) if (!identities.has(id)) errors.push(`unresolved scenario requirement: ${c.id} -> ${id}`);
  }
  return errors;
}

function selfTest() {
  const name = 'CORE-BOOTSTRAP.md';
  const source = '# Fixture\n\nRevision: test\n\n## Scope\n\n**B-SCOPE-01.** The implementation MUST report its scope.\n\nKeep [safety](SAFETY.md#scope) and [data](conformance/cases.json).\n\n### Details\n\n**B-SCOPE-02.** Unsupported inputs MUST be rejected.\n';
  const cases = [{ id: 'CORE-01', requirements: ['B-SCOPE-01'], profile: 'Core-Bootstrap', kind: 'runtime', file: 'conformance/cases.json' }];
  const canonical = toCairn(source, name, cases);
  assert.equal(toLegacy(canonical, name), source);
  assert.ok(canonical.includes('../safety/spec.md#scope'));
  assert.ok(canonical.includes('../../../specs/conformance/cases.json'));
  assert.ok(canonical.includes('- GIVEN') && canonical.includes('- WHEN') && canonical.includes('- THEN'));
  const doc = { name, canonical, compatibility: view(canonical, name) };
  assert.deepEqual(checkConversion([doc], cases), []);
  assert.ok(checkConversion([{ ...doc, compatibility: 'stale' }], cases).some(e => e.includes('view stale')));
  assert.ok(checkConversion([{ ...doc, canonical: canonical.replace('r[B-SCOPE-01]', 'r[wrong]') }], cases).length);
  assert.ok(checkConversion([doc], [{ ...cases[0], requirements: ['UNKNOWN-01'] }]).some(e => e.includes('unresolved')));
  assert.throws(() => toCairn(source + '\n**B-SCOPE-01.** Duplicate MUST fail.\n', name, cases), /duplicate/);
  assert.throws(() => toCairn(canonical, name, cases), /already converted/);
  assert.throws(() => canonicalPath('../escape.md'), /unknown spec/);
  const fenced = source + '\n```text\n**EXAMPLE-01.** Not a requirement.\n```\n';
  assert.equal(toLegacy(toCairn(fenced, name, cases), name), fenced);
  assert.throws(() => toCairn(source + '\n```\n', name, cases), /unclosed/);
  return 13;
}

// Filesystem shell: plan all outputs and check round trips before the first write.
function main() {
  const args = process.argv.slice(2);
  if (args.some(a => !['--import', '--write-views', '--self-test'].includes(a))
    || (args.includes('--import') && args.includes('--write-views'))) throw new Error('invalid options');
  const read = file => readFileSync(path.join(ROOT, file), 'utf8');
  const family = JSON.parse(read('specs/spec-family.json'));
  const cases = family.scenario_files.flatMap(file => JSON.parse(read(`specs/${file}`)).cases.map(c => ({ ...c, file })));
  let documents;
  if (args.includes('--import')) {
    documents = family.normative_documents.map(doc => {
      const name = doc.path;
      const file = canonicalPath(name);
      if (existsSync(path.join(ROOT, file))) throw new Error(`destination exists: ${file}`);
      const original = read(`specs/${name}`);
      const canonical = toCairn(original, name, cases);
      assert.equal(toLegacy(canonical, name), original, `lossless round trip: ${name}`);
      return { name, canonical, compatibility: view(canonical, name) };
    });
    const errors = checkConversion(documents, cases);
    if (errors.length) throw new Error(errors.join('\n'));
    const migrated = { ...family, lifecycle: '.cairn', normative_documents: family.normative_documents.map(doc => ({
      ...doc, path: '../' + canonicalPath(doc.path), compatibility_path: doc.path, cairn_id: SPEC_SLUGS[doc.path],
    })) };
    for (const doc of documents) {
      mkdirSync(path.dirname(path.join(ROOT, canonicalPath(doc.name))), { recursive: true });
      writeFileSync(path.join(ROOT, canonicalPath(doc.name)), doc.canonical);
      writeFileSync(path.join(ROOT, 'specs', doc.name), doc.compatibility);
    }
    writeFileSync(path.join(ROOT, 'specs/spec-family.json'), JSON.stringify(migrated, null, 2) + '\n');
  } else {
    const expected = Object.keys(SPEC_SLUGS).sort();
    assert.deepEqual(family.normative_documents.map(d => d.compatibility_path).sort(), expected, 'canonical family inventory');
    assert.equal(family.lifecycle, '.cairn', 'native lifecycle');
    assert.deepEqual(readdirSync(path.join(ROOT, '.cairn/specs')).sort(), Object.values(SPEC_SLUGS).sort(), 'Cairn spec inventory');
    documents = family.normative_documents.map(doc => {
      const name = doc.compatibility_path;
      assert.equal(doc.path, '../' + canonicalPath(name), 'canonical manifest path');
      assert.equal(doc.cairn_id, SPEC_SLUGS[name], 'Cairn spec identity');
      const current = read(canonicalPath(name));
      const canonical = args.includes('--write-views') ? toCairn(toLegacy(current, name), name, cases) : current;
      return { name, canonical, compatibility: args.includes('--write-views') ? view(canonical, name) : read(`specs/${name}`) };
    });
    const errors = checkConversion(documents, cases);
    if (errors.length) throw new Error(errors.join('\n'));
    if (args.includes('--write-views')) for (const doc of documents) {
      writeFileSync(path.join(ROOT, canonicalPath(doc.name)), doc.canonical);
      writeFileSync(path.join(ROOT, 'specs', doc.name), doc.compatibility);
    }
  }
  const tests = args.includes('--self-test') ? selfTest() : 0;
  console.log(JSON.stringify({ result: 'passed', specs: documents.length, scenarios: cases.length,
    self_tests_passed: tests, lane: 'document-conversion-only' }, null, 2));
}

if (import.meta.main) {
  try { main(); } catch (error) { console.error(`Cairn spec conversion failed: ${error.message}`); process.exitCode = 1; }
}
