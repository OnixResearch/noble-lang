// Offline tests of the optional Wolfram lane (#24): interchange, untrusted
// reference validation, judging, bounded shrinking, provenance and replay.
// No Wolfram kernel, client or Python is needed.
//   node --test verification/calculator-v1/wolfram/lane.test.mjs
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';
import { run as calc12Run } from '../oracle/calculator.mjs';
import { runFault } from '../oracle/faults.mjs';
import { fromText, judge, signature, validateReference } from './interchange.mjs';
import { adapterProvenance, buildCorpus, compareRun, shrink, SCHEMAS } from './lane.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const scratch = () => fs.mkdtempSync(path.join(os.tmpdir(), 'wolfram-lane-'));
const value = (n, d = '1') => ({ kind: 'value', n, d });
const diagnostic = category => ({ kind: 'diagnostic', category });
const observed = (stdout, stderr = [], status = stderr.length ? 1 : 0) => ({ stdout, stderr, status, signal: null, timedOut: false });

// Test double for a reference: the #21 BigInt oracle, used only to drive the
// shrinker here. The lane itself never uses it as a reference.
const doubleReference = text => {
  const outcome = calc12Run([text]).outcomes[0];
  return outcome.kind === 'value' ? value(`${outcome.value.n}`, `${outcome.value.d}`) : diagnostic(outcome.category);
};
const observeFault = name => text => {
  const out = runFault(name, [text]);
  return observed(out.stdout, out.stderr.map(d => `${d.category} [${d.span[0]},${d.span[1]}) provenance=unavailable`), out.status);
};

test('literals cross as exact digit strings, never floats', () => {
  const wide = '123456789012345678901234567890123456789';
  assert.deepEqual(fromText(`${wide} + 0.1`).interchange, {
    op: 'add', x: { op: 'lit', digits: wide, scale: 0 }, y: { op: 'lit', digits: '01', scale: 1 } });
  assert.deepEqual(fromText('0.00000000000000000001').interchange,
    { op: 'lit', digits: '000000000000000000001', scale: 20 });
  assert.deepEqual(fromText('-2^2').interchange,
    { op: 'neg', x: { op: 'pow', x: { op: 'lit', digits: '2', scale: 0 }, y: { op: 'lit', digits: '2', scale: 0 } } });
  assert.deepEqual(fromText('2^3^2').interchange.y.op, 'pow', '^ is right-associative in the tree');
});

test('only the arithmetic subset in render spelling is admitted', () => {
  for (const text of ['x + 1', 'f(2)', '1 2', '1 + )', '0.1.2', 'let x = 1']) {
    assert.equal(fromText(text).supported, false, text);
  }
  assert.match(fromText('1+2').reason, /does not reproduce/);
});

test('reference results are validated as untrusted data', () => {
  assert.deepEqual(validateReference({ kind: 'value', n: '-7', d: '2', steps: 3 }), value('-7', '2'));
  for (const bad of [{ kind: 'value', n: '2', d: '4' }, { kind: 'value', n: '1', d: '0' },
    { kind: 'value', n: '1', d: '-2' }, { kind: 'value', n: '0.5', d: '1' }, { kind: 'value', n: 3, d: '1' },
    { kind: 'value', n: '0', d: '2' }, { kind: 'diagnostic', category: 'ComplexInfinity' }, { kind: 'Certified' }, null]) {
    assert.equal(validateReference(bad).kind, 'oracle-error', JSON.stringify(bad));
  }
  assert.equal(validateReference({ kind: 'oracle-timeout', detail: 'Wolfram noble-oracle-timeout' }).kind, 'oracle-timeout');
});

test('judging distinguishes value, normalization, category and stream mismatches', () => {
  assert.equal(judge(value('1', '2'), observed(['1/2'])).verdict, 'agree');
  assert.equal(judge(value('1', '2'), observed(['2/4'])).mismatch, 'non-normalized-display');
  assert.equal(judge(value('7'), observed(['-7'])).mismatch, 'value-mismatch');
  assert.equal(judge(value('1', '10'), observed(['3602879701896397/36028797018963968'])).mismatch, 'value-mismatch');
  assert.equal(judge(value('1'), observed([], ['DivisionByZero [2,3) provenance=unavailable'])).mismatch, 'category-mismatch');
  assert.equal(judge(diagnostic('NonIntegerExponent'), observed(['2'])).mismatch, 'category-mismatch');
  const agreed = judge(diagnostic('DivisionByZero'), observed([], ['DivisionByZero [2,3) provenance=unavailable']));
  assert.equal(agreed.verdict, 'agree');
  assert.deepEqual(agreed.spans_not_judged, [[2, 3]]);
  assert.equal(judge(value('1'), observed(['1'], [], 1)).mismatch, 'stream-shape');
});

test('budgets, host failures and reference states are never agreement or disagreement', () => {
  assert.equal(judge(value('18446744073709551617'),
    observed([], ['ResourceLimit EvaluationWork [5,6) provenance=unavailable'])).verdict, 'budget-divergence');
  assert.equal(judge(value('1'), observed([], ['InfrastructureFailure: calculator selected environment clone fanout exceeded'], 1)).verdict,
    'not-comparable');
  assert.equal(judge(value('1'), { ...observed(['1']), timedOut: true }).verdict, 'not-comparable');
  assert.equal(judge(value('1'), { ...observed(['1']), status: 2 }).verdict, 'not-comparable');
  for (const kind of ['oracle-unavailable', 'oracle-timeout', 'oracle-error', 'unsupported', 'no-recorded-expectation']) {
    assert.equal(judge({ kind }, observed(['1'])).verdict, kind);
  }
});

test('bounded shrinking keeps the whole classification signature', () => {
  const observe = observeFault('reversed-subtraction');
  const text = '5 / 4 - 6 / 2';
  const reference = doubleReference(text);
  let calls = 0;
  const referenceOf = texts => { calls += 1; return texts.map(doubleReference); };
  const result = shrink({ text, reference, observe, referenceOf, rounds: 4, width: 6 });
  assert.equal(result.signature, 'disagree:value-mismatch:value:-');
  assert.ok(result.rounds_used <= 4 && calls <= 4);
  assert.ok(result.input.length < text.length && result.input.includes('-'), result.input);
  for (const step of result.trail) {
    const ref = doubleReference(step.kept);
    assert.equal(signature(ref, judge(ref, observe(step.kept))), result.signature, step.kept);
  }
  assert.equal(shrink({ text, reference, observe, referenceOf, rounds: 0, width: 6 }).input, text);
  // A domain premise is kept: a DivisionByZero disagreement never shrinks to a value case.
  const zero = '1 - 2 / (3 - 3)';
  const zeroRef = doubleReference(zero);
  const kept = shrink({ text: zero, reference: zeroRef, observe: () => observed(['0']), referenceOf, rounds: 4, width: 6 });
  assert.equal(kept.signature, 'disagree:category-mismatch:diagnostic:DivisionByZero');
  assert.equal(doubleReference(kept.input).category, 'DivisionByZero');
  // An unavailable reference stops shrinking instead of accepting a candidate.
  const none = shrink({ text, reference, observe, referenceOf: texts => texts.map(() => ({ kind: 'oracle-unavailable' })),
    rounds: 4, width: 6 });
  assert.equal(none.input, text);
});

test('the corpus is reproducible from its seed', () => {
  const first = buildCorpus({ seed: 2424, count: 6, wide: 3 });
  assert.deepEqual(buildCorpus({ seed: 2424, count: 6, wide: 3 }), first);
  assert.notDeepEqual(buildCorpus({ seed: 2425, count: 6, wide: 3 }).cases, first.cases);
  assert.ok(first.cases.some(c => c.family === 'wide-literal' && c.text.length > 40));
});

// A #19-runner-shaped output directory with the given processes.
const fakeRun = (dir, rows, binarySha = 'f7f02dfb') => {
  fs.mkdirSync(dir, { recursive: true });
  fs.writeFileSync(path.join(dir, 'receipt.json'), JSON.stringify({ binary: { sha256: binarySha, source_identity: 'src' } }));
  rows.forEach(([text, streams], index) => {
    const vector = path.join(dir, 'vectors', `v${index}`);
    fs.mkdirSync(vector, { recursive: true });
    fs.writeFileSync(path.join(vector, 'stdin'), `${text}\n`);
    fs.writeFileSync(path.join(vector, 'stdout'), streams.stdout.map(l => `${l}\n`).join(''));
    fs.writeFileSync(path.join(vector, 'stderr'), streams.stderr.map(l => `${l}\n`).join(''));
    fs.writeFileSync(path.join(vector, 'supervisor-spec.json'), JSON.stringify({ argv: ['noble', 'calculator'] }));
    fs.writeFileSync(path.join(vector, 'supervisor.json'), JSON.stringify({ status: streams.status, signal: null,
      timed_out: false, wall_ms: 1 }));
  });
};

const edgeCorpus = dir => {
  const corpus = buildCorpus({ seed: 0, count: 0, wide: 0 });
  const file = path.join(dir, 'corpus.json');
  fs.writeFileSync(file, JSON.stringify(corpus));
  return { corpus, sha256: crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex'), path: file };
};

test('offline fixture replay is labeled and never reported as a Wolfram pass', () => {
  const dir = scratch();
  const info = edgeCorpus(dir);
  const fixtures = path.join(here, 'fixtures/edge-expected-stand-in.json');
  const pick = id => info.corpus.cases.find(c => c.id === id);
  fakeRun(path.join(dir, 'run'), [[pick('edge/decimal-sum').text, observed(['3/10'])],
    [pick('edge/zero-power-zero').text, observed(['1'])],
    [pick('edge/subtract').text, observed(['-7'])],
    ['1 2', observed([], ['TrailingInput [2,3) provenance=unavailable'])]]);
  const report = compareRun(info, path.join(dir, 'run'), { fixtures });
  assert.equal(report.label, 'offline-fixture-replay');
  assert.equal(report.expectation_source, 'python-fractions-stand-in');
  assert.equal(report.lane_result, 'fail');
  const byId = new Map(report.results.map(r => [r.id, r]));
  assert.equal(byId.get('edge/decimal-sum').verdict, 'agree');
  assert.equal(byId.get('edge/zero-power-zero').verdict, 'agree');
  assert.deepEqual(byId.get('edge/subtract').matches_fault_models, ['reversed-subtraction']);
  assert.equal(report.outside_lane[0].verdict, 'unsupported');
  assert.equal(report.not_run.length, info.corpus.cases.length - 3);

  fakeRun(path.join(dir, 'agree'), [[pick('edge/decimal-sum').text, observed(['3/10'])]]);
  const one = { ...info, corpus: { ...info.corpus, cases: [pick('edge/decimal-sum')] } };
  assert.equal(compareRun(one, path.join(dir, 'agree'), { fixtures }).lane_result, 'offline-fixture-replay-agree');
});

test('stale or mismatched provenance invalidates every verdict', () => {
  const dir = scratch();
  const info = edgeCorpus(dir);
  const pick = info.corpus.cases.find(c => c.id === 'edge/decimal-sum');
  fakeRun(path.join(dir, 'run'), [[pick.text, observed(['3/10'])]]);
  const fixtures = JSON.parse(fs.readFileSync(path.join(here, 'fixtures/edge-expected-stand-in.json'), 'utf8'));
  const variants = {
    adapter: { ...fixtures, adapter: { ...fixtures.adapter, revision: '0'.repeat(64) } },
    policy: { ...fixtures, policy_version: 'noble-wolfram-policy/0' },
  };
  for (const [name, content] of Object.entries(variants)) {
    const file = path.join(dir, `${name}.json`);
    fs.writeFileSync(file, JSON.stringify(content));
    const report = compareRun(info, path.join(dir, 'run'), { fixtures: file });
    assert.equal(report.lane_result, 'stale-provenance', name);
    assert.ok(report.results.every(r => r.verdict === 'stale-provenance'), name);
  }
  const fixturesFile = path.join(here, 'fixtures/edge-expected-stand-in.json');
  assert.equal(compareRun(info, path.join(dir, 'run'), { fixtures: fixturesFile, 'expect-binary-sha256': 'other' })
    .lane_result, 'stale-provenance');
  assert.equal(compareRun(info, path.join(dir, 'run'), { fixtures: fixturesFile, 'expect-kernel-version': '14.1.0' })
    .lane_result, 'stale-provenance', 'a stand-in fixture cannot satisfy a kernel pin');
});

test('an unavailable or stand-in reference is visible and never a pass', () => {
  const dir = scratch();
  const info = edgeCorpus(dir);
  const pick = info.corpus.cases.find(c => c.id === 'edge/decimal-sum');
  fakeRun(path.join(dir, 'run'), [[pick.text, observed(['3/10'])]]);
  const one = { ...info, corpus: { ...info.corpus, cases: [pick] } };
  const referenceFile = (status, label, arithmetic, reference) => {
    const file = path.join(dir, `${label}-${status}.json`);
    fs.writeFileSync(file, JSON.stringify({ schema: SCHEMAS.reference, label, arithmetic, status,
      reason: status === 'ok' ? null : 'no usable Wolfram kernel', policy_version: adapterProvenance().policy_version,
      adapter: adapterProvenance(), corpus: { sha256: info.sha256 }, environment: null, probes: null,
      results: [{ id: pick.id, interchange_sha256: pick.interchange_sha256, reference }] }));
    return file;
  };
  const unavailable = compareRun(one, path.join(dir, 'run'), { reference: referenceFile('oracle-unavailable', 'fresh-oracle',
    'wolfram', { kind: 'oracle-unavailable', detail: 'no usable Wolfram kernel' }) });
  assert.equal(unavailable.lane_result, 'oracle-unavailable');
  assert.deepEqual(unavailable.summary, { 'oracle-unavailable': 1 });
  const standIn = compareRun(one, path.join(dir, 'run'), { reference: referenceFile('ok', 'stand-in-reference',
    'python-fractions', { kind: 'value', n: '3', d: '10' }) });
  assert.equal(standIn.lane_result, 'stand-in-reference-agree');
  const timeout = compareRun(one, path.join(dir, 'run'), { reference: referenceFile('ok', 'fresh-oracle', 'wolfram',
    { kind: 'oracle-timeout', detail: 'Wolfram noble-oracle-timeout' }) });
  assert.equal(timeout.lane_result, 'incomplete');
  const pass = compareRun(one, path.join(dir, 'run'), { reference: referenceFile('ok', 'fresh-oracle', 'wolfram',
    { kind: 'value', n: '3', d: '10' }) });
  assert.equal(pass.lane_result, 'pass');
});

test('retained edge fixtures are current and agree with the independent BigInt oracle', () => {
  const file = JSON.parse(fs.readFileSync(path.join(here, 'fixtures/edge-expected-stand-in.json'), 'utf8'));
  assert.equal(file.schema, SCHEMAS.fixtures);
  assert.equal(file.adapter.revision, adapterProvenance().revision, 're-record the fixtures after an adapter change');
  const corpus = buildCorpus({ seed: 0, count: 0, wide: 0 });
  assert.deepEqual(file.cases.map(c => c.interchange_sha256), corpus.cases.map(c => c.interchange_sha256));
  for (const row of file.cases) {
    assert.ok(['value', 'diagnostic'].includes(validateReference(row.expected).kind), row.id);
    const independent = doubleReference(row.text);
    assert.equal(row.expected.kind, independent.kind, row.id);
    if (independent.kind === 'value') assert.deepEqual([row.expected.n, row.expected.d], [independent.n, independent.d], row.id);
    else assert.equal(row.expected.category, independent.category, row.id);
  }
});
