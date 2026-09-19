// Document-policy regressions over in-memory fixtures. No Noble execution or proofs.
import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { fileURLToPath } from 'node:url';
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { loadBundle, validate, deriveLedger } from './check-specs.mjs';
import { canonicalPath, regenerate, toLegacy, checkConversion } from './cairn-specs.mjs';

const baseline = loadBundle(fileURLToPath(new URL('../', import.meta.url)));
const fixture = () => ({ texts: new Map(baseline.texts), paths: new Set(baseline.paths) });
function changeJson(bundle, file, change) {
  const value = JSON.parse(bundle.texts.get(file));
  change(value);
  bundle.texts.set(file, JSON.stringify(value));
}
function evidence(subject, kind, result) {
  return { subject, kind, result, revision: '0.1.0-draft.5', source_revision: 'synthetic-test-only',
    configuration: { toolchain: 'synthetic' }, claim: `Declared checks for ${subject}`, assumptions: [] };
}
function scenarioResult(bundle, result, kind = 'test', mutate = () => {}) {
  changeJson(bundle, 'specs/STATUS.json', s => { s.compiler_exists = true; s.runtime_exists = true; });
  changeJson(bundle, 'specs/conformance/cases.json', packet => {
    const c = packet.cases.find(c => c.id === 'CORE-01');
    c.state.implementation = 'implemented';
    c.state.execution = result;
    c.evidence = [evidence(c.id, kind, result)];
    mutate(c.evidence[0]);
  });
}
function rejects(bundle, diagnostic) {
  assert.ok(validate(bundle).errors.some(error => error.includes(diagnostic)), diagnostic);
}

for (const file of ['verification/runbook-probe.md', 'proofs/m1/runbook-probe.md']) {
  test(`runbook links stay checked: ${file}`, () => {
    const bundle = fixture();
    const target = path.posix.join(path.posix.dirname(file), 'fixture target.md');
    bundle.texts.set(file, '[fixture](fixture%20target.md)\n');
    bundle.texts.set(target, '# Fixture\n');
    bundle.paths.add(file);
    bundle.paths.add(target);
    assert.deepEqual(validate(bundle).errors, []);
    bundle.paths.delete(target);
    bundle.texts.delete(target);
    rejects(bundle, `broken local link: ${file}`);
  });
}

test('document discovery omits private evidence and compiler caches, not runbooks', () => {
  const root = mkdtempSync(path.join(tmpdir(), 'noble-document-scope-'));
  try {
    for (const dir of ['.pi', '.octet', 'proofs/m1/.lake', 'verification', 'proofs/m1']) {
      mkdirSync(path.join(root, dir), { recursive: true });
      writeFileSync(path.join(root, dir, 'probe.md'), '# Probe\n');
    }
    const bundle = loadBundle(root);
    assert.deepEqual([...bundle.texts.keys()].sort(), ['proofs/m1/probe.md', 'verification/probe.md']);
    assert.ok([...bundle.paths].every(name => !name.split('/').some(part => ['.pi', '.octet', '.lake'].includes(part))));
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('current family passes document validation', () => {
  assert.deepEqual(validate(baseline).errors, []);
});
for (const [name, mutate] of [
  ['reviewed purity erases request', c => c.input.variants.find(v => v.reviewed_pure_contract && !v.claimed_effects.length).outcome = 'accept'],
  ['missing matrix row', c => c.input.variants.pop()],
  ['duplicate matrix row', c => c.input.variants[3] = c.input.variants[2]],
  ['wrong operation identity', c => c.input.operation = 'another:package/api@1.0.0#inc'],
  ['request removed from positive row', c => c.input.variants[3].claimed_effects = []],
  ['reject-all checker', c => c.input.variants.forEach(v => v.outcome = 'effect-reject')],
]) {
  test(`WIT effect fixture rejects ${name}`, () => {
    const bundle = fixture();
    changeJson(bundle, 'specs/conformance/wit-wasi-cases.json', p => mutate(p.cases.find(c => c.id === 'WI-07')));
    rejects(bundle, 'WIT effect oracle:');
  });
}
for (const result of ['passed', 'failed', 'timeout']) {
  test(`runtime ${result} requires test evidence, not review evidence`, () => {
    const bundle = fixture();
    scenarioResult(bundle, result, 'review');
    rejects(bundle, 'execution evidence mismatch: CORE-01');
  });
  test(`runtime ${result} accepts a claim-bound test record`, () => {
    const bundle = fixture();
    scenarioResult(bundle, result);
    assert.deepEqual(validate(bundle).errors, []);
  });
}
for (const [name, mutate] of [
  ['missing', record => delete record.claim],
  ['empty', record => record.claim = ''],
  ['whitespace', record => record.claim = ' \n\t'],
  ['wrong type', record => record.claim = { text: 'not the declared string schema' }],
]) {
  test(`evidence rejects claim: ${name}`, () => {
    const bundle = fixture();
    scenarioResult(bundle, 'passed', 'test', mutate);
    rejects(bundle, 'evidence claim: CORE-01');
  });
}
test('review evidence remains valid for an explicitly review-kind scenario without a runtime', () => {
  const bundle = fixture();
  changeJson(bundle, 'specs/conformance/wit-wasi-cases.json', packet => {
    const c = packet.cases.find(c => c.id === 'WI-17');
    c.state.execution = 'passed';
    c.evidence = [evidence(c.id, 'review', 'passed')];
  });
  assert.deepEqual(validate(bundle).errors, []);
});
for (const id of ['core-checker', 'wasm-runtime']) {
  test(`${id} execution cannot use review-only evidence`, () => {
    const bundle = fixture();
    changeJson(bundle, 'specs/STATUS.json', s => {
      s.compiler_exists = true; s.runtime_exists = true;
      const component = s.components.find(c => c.id === id);
      component.implementation = 'implemented'; component.execution = 'passed';
      component.evidence = [evidence(id, 'review', 'passed')];
    });
    rejects(bundle, `execution evidence mismatch: ${id}`);
  });
}
test('proof records also require an explicit claim', () => {
  const bundle = fixture();
  changeJson(bundle, 'specs/STATUS.json', s => {
    s.proof_implementation_exists = true;
    const component = s.components.find(c => c.id === 'core-checker');
    component.proof = 'accepted';
    component.evidence = [evidence(component.id, 'lean-kernel', 'accepted')];
    delete component.evidence[0].claim;
  });
  rejects(bundle, 'evidence claim: core-checker');
});

function obligationResult(bundle, id, result, mutate = () => {}) {
  changeJson(bundle, 'specs/STATUS.json', s => { s.proof_implementation_exists = true; });
  changeJson(bundle, 'specs/verification/obligations.json', packet => {
    const o = packet.obligations.find(o => o.id === id);
    o.status = result;
    o.evidence = [evidence(id, 'lean-kernel', result)];
    mutate(o);
  });
}
// The fixture bundle already carries accepted ledger entries, so the count
// is whatever the mutated packet holds — never a hardcoded one.
function acceptedObligations(bundle) {
  const packet = JSON.parse(bundle.texts.get('specs/verification/obligations.json'));
  return packet.obligations.filter(o => o.status === 'accepted').length;
}
for (const result of ['accepted', 'failed']) {
  for (const id of ['PO-01', 'SO-01']) {
    test(`${id} ${result} accepts a claim-bound proof record`, () => {
      const bundle = fixture();
      obligationResult(bundle, id, result);
      const checked = validate(bundle);
      assert.deepEqual(checked.errors, []);
      assert.equal(checked.summary.completed_proofs, acceptedObligations(bundle));
    });
  }
  for (const [name, mutate, diagnostic] of [
    ['empty evidence', o => o.evidence = [], 'evidence required:'],
    ['missing evidence', o => delete o.evidence, 'evidence array:'],
    ['non-array evidence', o => o.evidence = {}, 'evidence array:'],
    ['null record', o => o.evidence = [null], 'evidence binding:'],
    ['string record', o => o.evidence = ['not proof evidence'], 'evidence binding:'],
    ['review-only evidence', o => o.evidence[0].kind = 'review', 'proof evidence mismatch:'],
    ['test-only evidence', o => o.evidence[0].kind = 'test', 'proof evidence mismatch:'],
    ['wrong result', o => o.evidence[0].result = result === 'accepted' ? 'failed' : 'accepted', 'proof evidence mismatch:'],
    ['wrong subject', o => o.evidence[0].subject = 'PO-02', 'evidence binding:'],
    ['stale revision', o => o.evidence[0].revision = 'stale', 'evidence binding:'],
    ['missing source', o => delete o.evidence[0].source_revision, 'evidence binding:'],
    ['empty configuration', o => o.evidence[0].configuration = {}, 'evidence binding:'],
    ['missing assumptions', o => delete o.evidence[0].assumptions, 'evidence binding:'],
    ['missing claim', o => delete o.evidence[0].claim, 'evidence claim:'],
    ['blank claim', o => o.evidence[0].claim = ' \n\t', 'evidence claim:'],
    ['wrong claim type', o => o.evidence[0].claim = {}, 'evidence claim:'],
    ['invalid companion record', o => o.evidence.push(null), 'evidence binding:'],
  ]) {
    test(`obligation ${result} rejects ${name}`, () => {
      const bundle = fixture();
      obligationResult(bundle, 'PO-01', result, mutate);
      rejects(bundle, `${diagnostic} PO-01`);
    });
  }
}
test('open obligations still validate any supplied evidence', () => {
  const bundle = fixture();
  obligationResult(bundle, 'PO-01', 'open', o => { delete o.evidence[0].claim; });
  rejects(bundle, 'evidence claim: PO-01');
});
test('well-formed obligation evidence cannot bypass the greenfield proof gate', () => {
  const bundle = fixture();
  obligationResult(bundle, 'PO-01', 'accepted');
  changeJson(bundle, 'specs/STATUS.json', s => { s.proof_implementation_exists = false; });
  rejects(bundle, 'greenfield obligation claim: PO-01');
});

function updateCanonical(bundle, name, text) {
  const family = JSON.parse(bundle.texts.get('specs/spec-family.json'));
  const cases = family.scenario_files.flatMap(file => JSON.parse(bundle.texts.get(`specs/${file}`)).cases.map(c => ({ ...c, file })));
  const canonical = regenerate(text, name, cases);
  bundle.texts.set(canonicalPath(name), canonical);
  bundle.texts.set(`specs/${name}`, `<!-- Generated compatibility view. Edit ${canonicalPath(name)} instead. -->\n` + toLegacy(canonical, name));
  const documents = family.normative_documents.map(d => ({ name: d.compatibility_path,
    canonical: bundle.texts.get(canonicalPath(d.compatibility_path)), compatibility: bundle.texts.get(`specs/${d.compatibility_path}`) }));
  assert.deepEqual(checkConversion(documents, cases), []);
}
for (const fence of ['```', '~~~~']) {
  test(`${fence} fenced examples never enter the requirement ledger`, () => {
    const bundle = fixture();
    const name = 'CORE-BOOTSTRAP.md';
    const example = `\n${fence}markdown\n**B-DOC-EXAMPLE-01.** Not a requirement.\n### Requirement: B-DOC-EXAMPLE-02\nr[B-DOC-EXAMPLE-02]\n\n**B-DOC-EXAMPLE-02.** Also not a requirement.\n${fence}\n`;
    updateCanonical(bundle, name, bundle.texts.get(canonicalPath(name)) + example);
    const before = deriveLedger(baseline);
    const after = deriveLedger(bundle);
    assert.equal(after.requirements.length, before.requirements.length, 'fenced text must not change the requirement count');
    assert.deepEqual(after, before);
    assert.deepEqual(validate(bundle).errors, []);
  });
  test(`${fence} fenced historical examples create no preservation obligation`, () => {
    const bundle = fixture();
    const family = JSON.parse(bundle.texts.get('specs/spec-family.json'));
    const doc = family.normative_documents.find(d => d.compatibility_path === 'SAFETY.md');
    const source = fileURLToPath(new URL(doc.source, new URL('../specs/', import.meta.url)));
    const root = fileURLToPath(new URL('../', import.meta.url));
    const key = source.slice(root.length);
    assert.ok(bundle.texts.has(key));
    bundle.texts.set(key, bundle.texts.get(key) + `\n${fence}markdown\n**S-EXAMPLE-01.** Not normative.\n${fence}\n`);
    assert.deepEqual(validate(bundle).errors, []);
  });
}
test('native requirements do not need legacy labels to enter the ledger', () => {
  const bundle = fixture();
  const file = canonicalPath('CORE-BOOTSTRAP.md');
  bundle.texts.set(file, bundle.texts.get(file) + '\n### Requirement: B-NATIVE-99\nr[B-NATIVE-99]\n\nThe checker MUST reject unsupported input.\n');
  const ledger = deriveLedger(bundle);
  assert.ok(ledger.requirements.some(r => r.id === 'B-NATIVE-99'));
  bundle.texts.set('specs/requirements.json', JSON.stringify(ledger));
  assert.deepEqual(validate(bundle).errors, []);
});
test('encoded local links retain their path and remain subject to link validation', () => {
  const bundle = fixture();
  const name = 'CORE-BOOTSTRAP.md';
  const link = '[artifact](../../../specs/probe%20%23%25%28file%29.json#details)';
  const target = 'specs/probe #%(file).json';
  bundle.texts.set(target, '{}');
  bundle.paths.add(target);
  updateCanonical(bundle, name, bundle.texts.get(canonicalPath(name)) + '\n' + link + '\n');
  assert.ok(bundle.texts.get(canonicalPath(name)).includes(link));
  assert.ok(bundle.texts.get(`specs/${name}`).includes('[artifact](probe%20%23%25%28file%29.json#details)'));
  assert.deepEqual(validate(bundle).errors, []);
  bundle.paths.delete(target);
  bundle.texts.delete(target);
  rejects(bundle, 'broken local link:');
});

test('an unfenced duplicate native requirement still fails', () => {
  const bundle = fixture();
  const file = canonicalPath('CORE-BOOTSTRAP.md');
  bundle.texts.set(file, bundle.texts.get(file) + '\n### Requirement: B-SCOPE-01\nr[B-SCOPE-01]\n\nDuplicate MUST fail.\n');
  rejects(bundle, 'duplicate requirement: B-SCOPE-01');
});
