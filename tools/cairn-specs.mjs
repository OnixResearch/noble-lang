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
const WRAPPER = '\n<!-- cairn:purpose:start -->\n## Purpose\n\nThis accepted specification records Noble draft contracts, not completed implementation.\nOriginal requirement IDs, explanatory prose, examples, and open decisions remain authoritative.\nScenario clauses refer to unexecuted designs in the conformance ledger.\n\n## Requirements\n\n<!-- cairn:purpose:end -->\n';
// Cairn's sync normalises a blank line before the end marker; conversions written
// before that normalisation must still round-trip, so both forms are accepted.
const LEGACY_WRAPPER = WRAPPER.replace('## Requirements\n\n<!--', '## Requirements\n<!--');

// Encode each path segment, including parentheses that delimit Markdown links.
function encodeLinkPath(file) {
  return file.split('/').map(part => encodeURIComponent(part)
    .replace(/[!'()*]/g, char => '%' + char.charCodeAt(0).toString(16).toUpperCase())).join('/');
}

export function rebaseLinks(text, from, to, targets) {
  const prose = proseLines(text);
  // Link-like strings inside fenced examples are literal text, not destinations.
  return text.split('\n').map((line, index) => prose[index] === null ? line
    : line.replace(/(\[[^\]\n]*\]\()([^\s)]+)(\))/g, (match, prefix, target, suffix) => {
      if (/^[a-z][a-z0-9+.-]*:/i.test(target) || target.startsWith('#')) return match;
      const [file, ...fragment] = target.split('#');
      const resolved = path.posix.normalize(path.posix.join(path.posix.dirname(from), decodeURIComponent(file)));
      const destination = targets.get(resolved) ?? resolved;
      const relative = path.posix.relative(path.posix.dirname(to), destination);
      return `${prefix}${encodeLinkPath(relative)}${fragment.length ? '#' + fragment.join('#') : ''}${suffix}`;
    })).join('\n');
}

function scenarioLinks(id, cases, destination) {
  const linked = cases.filter(c => c.requirements.includes(id));
  const lines = [START];
  if (!linked.length) {
    lines.push('Test design remains open for this requirement. No scenario or execution evidence is supplied.');
  }
  for (const c of linked) {
    const file = path.posix.relative(path.posix.dirname(destination), `specs/${c.file}`);
    const reference = `[${c.id}](${encodeLinkPath(file)})`;
    lines.push(`#### Scenario: ${c.id} for ${id}`, '',
      `- GIVEN the \`${c.profile}\` profile and every field of \`input\` in ${reference}`,
      `- WHEN the \`${c.kind}\` procedure for case \`${c.id}\` runs against those inputs`,
      `- THEN the observations match every field of \`expected\` in case \`${c.id}\``, '',
      `This is a scenario design, not an execution result. The case's \`state\` and \`evidence\` fields record its status.`, '');
  }
  lines.push(END);
  return '\n' + lines.join('\n') + '\n';
}

// Ignore fenced examples when reading requirement identities and generated regions.
function proseLines(markdown) {
  let fence;
  const prose = markdown.split('\n').map(line => {
    const delimiter = line.match(/^\s*(`{3,}|~{3,})(.*)$/);
    if (fence) {
      if (delimiter && delimiter[1][0] === fence[0] && delimiter[1].length >= fence.length
        && !delimiter[2].trim()) fence = undefined;
      return null;
    }
    if (delimiter) { fence = delimiter[1]; return null; }
    return line;
  });
  if (fence) throw new Error('unclosed code fence');
  return prose;
}

function stripScenarioRegions(markdown) {
  const lines = markdown.split('\n');
  const prose = proseLines(markdown);
  const regions = [];
  let start, offset = 0;
  for (const [index, line] of lines.entries()) {
    if (prose[index] === START) {
      if (start !== undefined) throw new Error('nested scenario region');
      start = offset;
    } else if (prose[index] === END) {
      if (start === undefined) throw new Error('orphan scenario region end');
      regions.push({ start, end: Math.min(markdown.length, offset + line.length + 1) });
      start = undefined;
    }
    offset += line.length + 1;
  }
  if (start !== undefined) throw new Error('unclosed scenario region');
  let result = '', cursor = 0;
  for (let { start, end } of regions) {
    // Remove only the separators added by scenarioLinks and output.join.
    // A final generated region has two leading separators instead of a trailing one.
    if (markdown[start - 1] === '\n') start--;
    if (end === markdown.length) {
      if (markdown[start - 1] === '\n') start--;
    } else if (markdown[end] === '\n') end++;
    result += markdown.slice(cursor, Math.max(cursor, start));
    cursor = Math.max(cursor, end);
  }
  return result + markdown.slice(cursor);
}

function nativeRequirements(markdown) {
  const prose = proseLines(markdown);
  const entries = [];
  const seen = new Set();
  const markers = new Set(), labels = new Set();
  const nextContent = start => {
    while (start < prose.length && prose[start]?.trim() === '') start++;
    return start;
  };
  for (const [start, line] of prose.entries()) {
    if (!line?.startsWith('### Requirement:')) continue;
    const id = line.match(/^### Requirement: ([A-Z][A-Z0-9-]*-\d+)$/)?.[1];
    if (!id) throw new Error(`invalid requirement heading: ${line}`);
    if (seen.has(id)) throw new Error(`duplicate requirement: ${id}`);
    seen.add(id);
    const marker = nextContent(start + 1);
    if (prose[marker] !== `r[${id}]`) throw new Error(`requirement marker identity mismatch: ${id}`);
    markers.add(marker);
    const body = nextContent(marker + 1);
    const label = prose[body]?.match(REQUIREMENT)?.[1];
    if (label && label !== id) throw new Error(`legacy requirement identity mismatch: ${id} -> ${label}`);
    if (label) labels.add(body);
    entries.push({ id, start, marker, labeled: Boolean(label) });
  }
  for (const [index, line] of prose.entries()) {
    if (line?.startsWith('r[') && !markers.has(index)) throw new Error(`unscoped requirement marker: ${line}`);
    if (line?.match(REQUIREMENT) && !labels.has(index)) throw new Error(`unscoped legacy requirement identity: ${line}`);
  }
  if (!entries.length) throw new Error('no native requirements');
  return entries;
}

// Native headings own current IDs; historical inputs retain their legacy labels.
// Both readers share the same fence handling as conversion.
export function nativeRequirementIds(markdown) {
  return nativeRequirements(markdown).map(entry => entry.id);
}

export function legacyRequirementIds(markdown) {
  return proseLines(markdown).flatMap(line => {
    const id = line?.match(REQUIREMENT)?.[1];
    return id ? [id] : [];
  });
}

export function toCairn(source, name, cases) {
  const prose = proseLines(source);
  if (prose.some(line => line?.includes('<!-- cairn:') || line?.startsWith('r['))) throw new Error(`already converted input: ${name}`);
  if (!source.startsWith('# ')) throw new Error(`missing title: ${name}`);
  const destination = canonicalPath(name);
  // Rebase source prose before mixing it with destination-relative generated links.
  const lines = rebaseLinks(source, `specs/${name}`, destination, mapping).split('\n');
  const output = [];
  const seen = new Set();
  let active;
  for (const [index, line] of lines.entries()) {
    const id = prose[index]?.match(REQUIREMENT)?.[1];
    if (active && (id || /^#{1,6} /.test(prose[index] ?? ''))) {
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
  }
  if (!seen.size) throw new Error(`no requirements: ${name}`);
  if (active) output.push(scenarioLinks(active, cases, destination));
  return output.join('\n');
}

export function toLegacy(markdown, name) {
  const entries = nativeRequirements(markdown);
  const lines = markdown.split('\n');
  for (const { id, start, marker, labeled } of entries.reverse()) {
    const count = marker - start + 1 + (lines[marker + 1] === '' ? 1 : 0);
    // Native IDs are authoritative. Add a compatibility label when one is absent.
    lines.splice(start, count, ...(labeled ? [] : [`**${id}.**`, '']));
  }
  let stripped = lines.join('\n');
  // The generated purpose wrapper belongs directly after the title, never inside an example.
  const titleEnd = stripped.indexOf('\n');
  const wrapperSpan = [WRAPPER, LEGACY_WRAPPER]
    .map(form => '\n' + form)
    .find(form => stripped.startsWith(form, titleEnd));
  if (wrapperSpan) {
    stripped = stripped.slice(0, titleEnd) + stripped.slice(titleEnd + wrapperSpan.length);
  }
  return rebaseLinks(stripScenarioRegions(stripped), canonicalPath(name), `specs/${name}`, reverseMapping);
}

function view(markdown, name) {
  return `<!-- Generated compatibility view. Edit ${canonicalPath(name)} instead. -->\n` + toLegacy(markdown, name);
}

export function regenerate(markdown, name, cases) {
  const identities = nativeRequirements(markdown).map(entry => entry.id);
  const canonical = toCairn(toLegacy(markdown, name), name, cases);
  assert.deepEqual(nativeRequirements(canonical).map(entry => entry.id), identities,
    `regeneration changed requirement identities: ${canonicalPath(name)}`);
  return canonical;
}

export function checkConversion(documents, cases) {
  const errors = [];
  const identities = new Set();
  for (const { name, canonical, compatibility } of documents) {
    try {
      if (regenerate(canonical, name, cases) !== canonical) errors.push(`Cairn structure or scenario links stale: ${canonicalPath(name)}`);
      if (view(canonical, name) !== compatibility) errors.push(`compatibility view stale: specs/${name}`);
      for (const { id } of nativeRequirements(canonical)) {
        if (identities.has(id)) errors.push(`duplicate requirement: ${id}`);
        identities.add(id);
      }
    } catch (error) { errors.push(error.message); }
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
  const native = canonical + '\n### Requirement: B-SCOPE-03\nr[B-SCOPE-03]\n\nThe checker MUST reject unsupported input.\n\n#### Scenario: Unsupported input\n\n- GIVEN unsupported input\n- WHEN the checker validates it\n- THEN the checker rejects it\n';
  const regenerated = regenerate(native, name, cases);
  assert.ok(regenerated.includes('r[B-SCOPE-03]'), 'native requirement identity survives regeneration');
  assert.ok(regenerated.includes('#### Scenario: Unsupported input'), 'authored scenario survives regeneration');
  assert.equal(regenerate(regenerated, name, cases), regenerated, 'regeneration is idempotent');
  assert.throws(() => toLegacy(canonical.replace('**B-SCOPE-02.**', '**B-SCOPE-03.**'), name), /identity/);
  assert.throws(() => toLegacy(canonical.replace('r[B-SCOPE-02]', 'r[B-SCOPE-03]'), name), /identity/);
  assert.throws(() => toLegacy(native + '\n### Requirement: B-SCOPE-03\nr[B-SCOPE-03]\n\nDuplicate MUST fail.\n', name), /duplicate/);
  assert.throws(() => toLegacy(canonical.replace('r[B-SCOPE-02]\n', ''), name), /marker/);
  const linkedCases = [...cases, { ...cases[0], id: 'CORE-NEW', requirements: ['B-SCOPE-03'] }];
  const linked = regenerate(native, name, linkedCases);
  assert.deepEqual(checkConversion([{ name, canonical: linked, compatibility: view(linked, name) }], linkedCases), []);
  assert.ok(linked.includes('#### Scenario: CORE-NEW for B-SCOPE-03'));
  assert.deepEqual(nativeRequirements(regenerated).map(entry => entry.id), ['B-SCOPE-01', 'B-SCOPE-02', 'B-SCOPE-03']);
  assert.throws(() => regenerate(canonical + '\nr[B-ORPHAN-01]\n', name, cases), /unscoped/);
  for (const fence of ['```', '~~~~']) {
    const example = source + `\n${fence}markdown\n### Requirement: B-EXAMPLE-01\nr[B-EXAMPLE-01]\n\n**B-EXAMPLE-01.** This is example text.\n${fence}\n`;
    assert.equal(toLegacy(toCairn(example, name, cases), name), example, 'fenced native syntax is not a requirement');
  }
  return 26;
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
      const canonical = args.includes('--write-views') ? regenerate(current, name, cases) : current;
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
