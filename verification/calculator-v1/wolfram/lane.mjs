#!/usr/bin/env node
// Optional Wolfram exact-arithmetic differential lane (#24).
//
// Separate from the CALC-12 oracle lane (#21), the #19 acceptance runner and
// any proof. It shares only the #21 generator (corpus and shrink candidates)
// and the #19 runner's vector schema and retained process I/O. The reference
// is `bridge.py`; nothing here computes an expected value.
//
//   lane.mjs corpus    --seed N --count N --wide N --out NEW_FILE
//   lane.mjs vectors   --corpus FILE --out NEW_FILE
//   lane.mjs reference --corpus FILE --arithmetic wolfram|python-fractions [--python PY]
//                      [--kernel PATH] [--timeout S] [--memory BYTES] --out NEW_FILE
//   lane.mjs record    --corpus FILE --reference FILE --out NEW_FILE
//   lane.mjs compare   --corpus FILE --run RUN_DIR (--reference FILE | --fixtures FILE)
//                      [--expect-binary-sha256 H] [--expect-source-id S]
//                      [--expect-kernel-version V] [--expect-client-version V] --out NEW_FILE
//   lane.mjs harness   --corpus FILE --reference FILE --implementation fault:NAME|calc12-oracle
//                      [--python PY] [--shrink-rounds N] [--shrink-candidates N] --out NEW_FILE
//   lane.mjs shrink    (--corpus FILE --case ID | --input TEXT) [--shrink-candidates N] --out NEW_FILE
//
// Exit status of `reference`, `compare` and `harness`: 0 pass (or agreement in
// a non-Wolfram mode), 1 disagreement, 2 incomplete, 3 oracle-unavailable,
// 4 stale provenance or oracle-error.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseArgs } from 'node:util';
import { run as calc12Run } from '../oracle/calculator.mjs';
import { FAULTS, FAULT_NAMES, runFault } from '../oracle/faults.mjs';
import { generateExpressions, prng, render, shrinkCandidates } from '../oracle/generate.mjs';
import { VECTOR_SCHEMA, validate as validateVectors } from '../vectors.mjs';
import { POLICY_VERSION, fromText, judge, operatorCount, parseDiagnostics, signature, validateReference }
  from './interchange.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const lane = path.resolve(here, '..');
const root = path.resolve(here, '../../..');
export const SCHEMAS = Object.freeze({
  corpus: 'noble-wolfram-corpus/v1', reference: 'noble-wolfram-reference/v1', fixtures: 'noble-wolfram-fixtures/v1',
  report: 'noble-wolfram-comparison/v1', harness: 'noble-wolfram-harness/v1', request: 'noble-wolfram-request/v1',
});
// Files that determine the reference computation for a given interchange tree
// (and the tree for a given text). A change to any of them invalidates every
// reference file and fixture recorded before it.
const ADAPTER_FILES = ['wolfram/interchange.mjs', 'wolfram/bridge.py', 'oracle/generate.mjs'];
const CORPUS_FILES = ['oracle/generate.mjs', 'oracle/calculator.mjs', 'oracle/rational.mjs', 'wolfram/lane.mjs',
  'wolfram/fixtures/edge-cases.json'];
const EDGE_FILE = path.join(here, 'fixtures/edge-cases.json');
const BUDGET_FLAGS = new Set(['--input-bytes', '--numeric-digits', '--intermediate-bits', '--syntax-depth',
  '--call-depth', '--evaluation-work', '--output-size']);
export const LABELS = Object.freeze({ wolfram: 'fresh-oracle', 'python-fractions': 'stand-in-reference' });
const RECORDED_SOURCE = Object.freeze({ wolfram: 'wolfram-recorded', 'python-fractions': 'python-fractions-stand-in' });
export const NON_CLAIMS = Object.freeze([
  'Agreement on finitely many inputs is test evidence only; it establishes no universal arithmetic theorem and no parser correctness (CALC-EVIDENCE-01).',
  'Reference output (Wolfram or otherwise) is untrusted candidate data: it mints no Certified status, discharges no proof obligation, changes no accepted program semantics and changes no canonical case state.',
  'Only a fresh-oracle run is a Wolfram comparison; stand-in-reference and offline-fixture-replay results are labeled as such and never count as one.',
  'Grammar, diagnostic spans, lexical sessions, session rollback, deterministic budgets and host failures are outside this lane (CALC-12 oracle #21, runner #19, budgets #20).',
  'A Wolfram timeout or memory abort is an oracle failure, not Noble\'s ResourceLimit; a Noble ResourceLimit is a budget divergence, not a disagreement.',
]);

const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const readJson = file => JSON.parse(fs.readFileSync(file, 'utf8'));
const fresh = (file, { inRepository = false } = {}) => {
  const resolved = path.resolve(file);
  assert.ok(!fs.existsSync(resolved), `${resolved} must not exist`);
  if (!inRepository) assert.ok(!resolved.startsWith(`${root}${path.sep}`), 'outputs stay outside the repository');
  return resolved;
};
const write = (file, value, options) => fs.writeFileSync(fresh(file, options), `${JSON.stringify(value, null, 2)}\n`);
const filesSha = names => Object.fromEntries(names.map(name => [name, sha(fs.readFileSync(path.join(lane, name)))]));
const pad = (index, width = 2) => String(index).padStart(width, '0');
const linesOf = text => {
  const parts = text.split('\n');
  if (parts.at(-1) === '') parts.pop();
  return parts;
};
export const interchangeSha = interchange => sha(JSON.stringify(interchange));

export const adapterProvenance = () => {
  const files = filesSha(ADAPTER_FILES);
  return { policy_version: POLICY_VERSION, files, revision: sha(JSON.stringify(files)) };
};

const gitState = () => {
  const git = args => spawnSync('git', ['-C', root, ...args], { encoding: 'utf8' });
  const head = git(['rev-parse', 'HEAD']);
  const status = git(['status', '--porcelain', '--untracked-files=no']);
  return { commit: head.status === 0 ? head.stdout.trim() : null,
    tracked_changes: status.status === 0 ? status.stdout.trim() : null };
};

// ---- corpus ------------------------------------------------------------

const num = text => ({ t: 'num', text: `${text}` });
const bin = (op, l, r) => ({ t: 'bin', op, l, r });
const digits = (random, length) => {
  let text = `${random.int(1, 9)}`;
  while (text.length < length) text += `${random.int(0, 9)}`;
  return text;
};
// Lane family: integer and decimal literals beyond the I64 range, so the
// reference sees long exact digit strings (the #21 generator keeps magnitudes
// small apart from `2^63/2^64 + k`). Every shape is a single binary operator
// or one nested pair, to stay well inside the default guest budgets.
const WIDE_SHAPES = [
  random => bin('+', num(digits(random, random.int(19, 30))), num(digits(random, random.int(19, 30)))),
  random => bin('-', num(digits(random, 20)), num(digits(random, random.int(21, 28)))),
  random => bin('*', num(digits(random, random.int(19, 24))), num(digits(random, random.int(19, 24)))),
  random => bin('/', num(digits(random, random.int(20, 30))), num(digits(random, random.int(20, 30)))),
  random => bin('+', { t: 'neg', e: num(digits(random, 20)) }, num(`0.${digits(random, random.int(20, 25))}`)),
  random => bin('/', { t: 'par', e: bin('-', num(digits(random, 22)), num(digits(random, 22))) }, num(digits(random, 3))),
];
export const wideLiterals = ({ seed, count }) => {
  const random = prng((seed ^ 0x2424) >>> 0);
  return Array.from({ length: count }, (_, index) => render(WIDE_SHAPES[index % WIDE_SHAPES.length](random)));
};

const caseOf = (id, text, extra) => {
  const parsed = fromText(text);
  assert.ok(parsed.supported, `${id}: ${text}: ${parsed.reason}`);
  return { id, text, flags: [], ...extra, operators: operatorCount(parsed.interchange),
    interchange: parsed.interchange, interchange_sha256: interchangeSha(parsed.interchange) };
};

export const buildCorpus = ({ seed, count, wide }) => {
  assert.ok([seed, count, wide].every(value => Number.isInteger(value) && value >= 0));
  const edge = readJson(EDGE_FILE);
  assert.equal(edge.schema, 'noble-wolfram-edge-cases/v1');
  const cases = edge.cases.map(item => caseOf(`edge/${item.id}`, item.text,
    { origin: 'edge-fixture', purpose: item.purpose, calc12_fault: item.calc12_fault ?? null }));
  const seen = new Set(cases.map(c => c.text));
  const excluded = [];
  generateExpressions({ seed, count }).forEach((item, index) => {
    const id = `seed-${seed}/calc12-${pad(index + 1)}`;
    const parsed = fromText(item.text);
    if (item.family === 'malformed' || !parsed.supported) {
      excluded.push({ id, text: item.text, family: item.family,
        reason: item.family === 'malformed' ? 'CALC-02 malformed shape: grammar and spans belong to the CALC-12 oracle lane'
          : parsed.reason });
    } else if (seen.has(item.text)) {
      excluded.push({ id, text: item.text, family: item.family, reason: 'duplicate of an earlier case' });
    } else {
      seen.add(item.text);
      cases.push(caseOf(id, item.text, { origin: 'calc12-generator', family: item.family }));
    }
  });
  wideLiterals({ seed, count: wide }).forEach((text, index) => {
    const id = `seed-${seed}/wide-${pad(index + 1)}`;
    if (seen.has(text)) excluded.push({ id, text, family: 'wide-literal', reason: 'duplicate of an earlier case' });
    else { seen.add(text); cases.push(caseOf(id, text, { origin: 'lane-generator', family: 'wide-literal' })); }
  });
  return { schema: SCHEMAS.corpus, policy_version: POLICY_VERSION,
    generator: { seed, count, wide, prng: 'mulberry32 (#21 oracle/generate.mjs)', files: filesSha(CORPUS_FILES) },
    cases, excluded };
};

const loadCorpus = file => {
  const bytes = fs.readFileSync(file);
  const corpus = JSON.parse(bytes);
  assert.equal(corpus.schema, SCHEMAS.corpus);
  for (const item of corpus.cases) assert.equal(interchangeSha(item.interchange), item.interchange_sha256, `${item.id}: interchange hash`);
  return { corpus, sha256: sha(bytes), path: path.resolve(file) };
};

// Runner bookkeeping only: the #19 runner needs an expectation per operation;
// it comes from the CALC-12 oracle and this lane never reads the runner's
// classification.
const runnerExpect = text => {
  const outcome = calc12Run([text]).outcomes[0];
  if (outcome.kind === 'value') return { value: outcome.display };
  return { diagnostic: { category: outcome.category, span: null } };
};

export const vectorsOf = (corpus, corpusSha) => validateVectors({
  schema: VECTOR_SCHEMA, lane: 'optional Wolfram lane (#24); expectations are runner bookkeeping from the CALC-12 oracle',
  corpus_sha256: corpusSha,
  vectors: corpus.cases.map(item => ({ id: `WOLFRAM/${item.id}`, case: 'CALC-12-wolfram', kind: 'line', support: 'cli',
    flags: item.flags, operations: [{ line: item.text, expect: runnerExpect(item.text) }] })),
});

// ---- reference ---------------------------------------------------------

export const requestOf = (cases, { arithmetic, kernel = null, timeout = 30, memory = 1 << 30 }) => ({
  schema: SCHEMAS.request, policy_version: POLICY_VERSION, arithmetic, kernel, timeout_seconds: timeout,
  memory_bytes: memory, cases: cases.map(item => ({ id: item.id, interchange: item.interchange })),
});

// Runs bridge.py once. Returns the bridge response plus how it was invoked.
export const callBridge = (request, { python = 'python3', scratch }) => {
  fs.mkdirSync(scratch, { recursive: true });
  const stamp = `${process.pid}-${Date.now()}-${crypto.randomBytes(4).toString('hex')}`;
  const requestFile = path.join(scratch, `request-${stamp}.json`);
  const responseFile = path.join(scratch, `response-${stamp}.json`);
  fs.writeFileSync(requestFile, `${JSON.stringify(request, null, 2)}\n`, { flag: 'wx' });
  const child = spawnSync(python, [path.join(here, 'bridge.py'), requestFile, responseFile], { encoding: 'utf8' });
  const invocation = { python, exit_status: child.status, stderr_tail: (child.stderr ?? '').slice(-2000),
    launch_error: child.error ? String(child.error) : null };
  if (child.error || !fs.existsSync(responseFile)) {
    const reason = child.error ? `python not runnable: ${child.error.message}` : `bridge exited ${child.status} without a response`;
    // A missing interpreter is a missing prerequisite; a bridge crash is an infrastructure error.
    return { response: { status: child.error?.code === 'ENOENT' ? 'oracle-unavailable' : 'oracle-error', reason, results: [] },
      invocation };
  }
  return { response: readJson(responseFile), invocation };
};

const referenceFile = (corpusInfo, cases, options) => {
  const started = new Date().toISOString();
  const { response, invocation } = callBridge(requestOf(cases, options),
    { python: options.python, scratch: options.scratch });
  const results = new Map((response.results ?? []).map(r => [r.id, r.reference]));
  return {
    schema: SCHEMAS.reference, label: LABELS[options.arithmetic], arithmetic: options.arithmetic,
    status: response.status, reason: response.reason ?? null, policy_version: POLICY_VERSION,
    adapter: adapterProvenance(), corpus: { path: corpusInfo.path, sha256: corpusInfo.sha256,
      generator: corpusInfo.corpus.generator },
    environment: response.environment ?? null, probes: response.probes ?? null,
    bridge: { ...invocation, response_policy_version: response.policy_version ?? null },
    started, finished: new Date().toISOString(),
    results: cases.map(item => ({ id: item.id, interchange_sha256: item.interchange_sha256,
      reference: results.get(item.id) ?? { kind: response.status === 'ok' ? 'oracle-error' : response.status,
        detail: response.reason ?? 'no result' } })),
  };
};

const exitOf = result => ({ pass: 0, 'stand-in-reference-agree': 0, 'offline-fixture-replay-agree': 0, ok: 0,
  fail: 1, incomplete: 2, 'oracle-unavailable': 3 })[result] ?? 4;

// ---- reference providers and provenance -------------------------------

// A provider answers `expected(case)` with a validated reference and carries
// the label and provenance that the report must show.
const providerOf = (options, corpusInfo) => {
  const current = adapterProvenance();
  const stale = [];
  const check = (what, recorded, now) => { if (recorded !== now) stale.push(`${what}: recorded ${recorded} != current ${now}`); };
  if (options.reference) {
    const file = readJson(options.reference);
    assert.equal(file.schema, SCHEMAS.reference);
    check('policy_version', file.policy_version, POLICY_VERSION);
    check('adapter revision', file.adapter.revision, current.revision);
    check('corpus sha256', file.corpus.sha256, corpusInfo.sha256);
    const kernel = file.environment?.kernel?.version ?? null;
    const client = file.environment?.client?.version ?? null;
    if (options['expect-kernel-version'] !== undefined) check('kernel version', kernel, options['expect-kernel-version']);
    if (options['expect-client-version'] !== undefined) check('client version', client, options['expect-client-version']);
    const results = new Map(file.results.map(r => [r.id, r]));
    return { label: file.label, expectation_source: file.label === 'fresh-oracle' ? 'fresh Wolfram computation'
      : 'fresh python-fractions stand-in computation (not Wolfram)',
    status: file.status, reason: file.reason, stale, arithmetic: file.arithmetic,
    provenance: { file: path.resolve(options.reference), sha256: sha(fs.readFileSync(options.reference)),
      policy_version: file.policy_version, adapter: file.adapter, environment: file.environment, probes: file.probes,
      corpus: file.corpus, started: file.started, finished: file.finished },
    expected: item => {
      const row = results.get(item.id);
      if (row === undefined || row.interchange_sha256 !== item.interchange_sha256) {
        return { kind: 'no-recorded-expectation', detail: 'case is not in the reference file' };
      }
      return validateReference(row.reference);
    } };
  }
  assert.ok(options.fixtures, '--reference FILE or --fixtures FILE is required');
  const file = readJson(options.fixtures);
  assert.equal(file.schema, SCHEMAS.fixtures);
  check('policy_version', file.policy_version, POLICY_VERSION);
  check('adapter revision', file.adapter.revision, current.revision);
  const kernel = file.reference_provenance?.environment?.kernel?.version ?? null;
  if (options['expect-kernel-version'] !== undefined) check('kernel version', kernel, options['expect-kernel-version']);
  const byHash = new Map(file.cases.map(c => [c.interchange_sha256, c]));
  return { label: 'offline-fixture-replay', expectation_source: file.expectation_source, status: 'ok', reason: null,
    stale, arithmetic: null,
    provenance: { file: path.resolve(options.fixtures), sha256: sha(fs.readFileSync(options.fixtures)),
      policy_version: file.policy_version, adapter: file.adapter, expectation_source: file.expectation_source,
      reference_provenance: file.reference_provenance },
    expected: item => {
      const row = byHash.get(item.interchange_sha256);
      return row === undefined ? { kind: 'no-recorded-expectation', detail: 'no retained fixture for this interchange tree' }
        : validateReference(row.expected);
    } };
};

const laneResult = (provider, verdicts) => {
  if (provider.stale.length > 0) return 'stale-provenance';
  if (provider.status !== 'ok') return provider.status;
  if (verdicts.includes('disagree')) return 'fail';
  if (verdicts.length === 0 || verdicts.some(v => v !== 'agree')) return 'incomplete';
  return provider.label === 'fresh-oracle' ? 'pass' : `${provider.label}-agree`;
};

const summarize = items => {
  const summary = {};
  for (const item of items) summary[item.verdict] = (summary[item.verdict] ?? 0) + 1;
  return summary;
};

// Fault models (#21) that predict exactly the observed streams.
const attribute = (text, observed) => FAULT_NAMES.filter(name => {
  const predicted = runFault(name, [text]);
  return predicted.status === observed.status
    && JSON.stringify(predicted.stdout) === JSON.stringify(observed.stdout)
    && JSON.stringify(predicted.stderr.map(d => d.category))
      === JSON.stringify(parseDiagnostics(observed.stderr).map((d, i) => d?.category ?? observed.stderr[i]));
});

export const replayArtifact = (item, provider) => ({
  text: item.text, flags: item.flags, interchange: item.interchange, interchange_sha256: item.interchange_sha256,
  noble: { vector_file: { schema: VECTOR_SCHEMA, vectors: [{ id: 'WOLFRAM/replay', case: 'CALC-12-wolfram', kind: 'line',
    support: 'cli', flags: item.flags, operations: [{ line: item.text, expect: runnerExpect(item.text) }] }] },
  command: 'node verification/calculator-v1/acceptance.mjs --binary BIN --binary-sha256 SHA --source-id SRC --build-profile PROFILE --out NEW_DIR --vectors VECTOR_FILE --no-controls' },
  oracle: { request: requestOf([item], { arithmetic: provider.arithmetic ?? 'wolfram' }),
    command: 'python3 verification/calculator-v1/wolfram/bridge.py REQUEST.json NEW_RESPONSE.json',
    available_in_this_report: provider.label === 'fresh-oracle' },
});

// ---- compare -----------------------------------------------------------

const observedProcesses = runDir => {
  const receiptFile = ['receipt.json', 'receipt.partial.json'].map(f => path.join(runDir, f)).find(fs.existsSync) ?? null;
  const receipt = receiptFile ? readJson(receiptFile) : null;
  const vectorsDir = path.join(runDir, 'vectors');
  const out = [];
  for (const slug of fs.readdirSync(vectorsDir).sort()) {
    const dir = path.join(vectorsDir, slug);
    const resultFile = path.join(dir, 'supervisor.json');
    if (!fs.existsSync(resultFile)) { out.push({ slug, pending: true }); continue; }
    const spec = readJson(path.join(dir, 'supervisor-spec.json'));
    const supervisor = readJson(resultFile);
    const stdin = fs.readFileSync(path.join(dir, 'stdin'));
    out.push({ slug, binary: spec.argv[0], flags: spec.argv.slice(spec.argv.indexOf('calculator') + 1),
      stdin_sha256: sha(stdin), lines: linesOf(stdin.toString('utf8')), wall_ms: supervisor.wall_ms,
      observed: { stdout: linesOf(fs.readFileSync(path.join(dir, 'stdout'), 'utf8')),
        stderr: linesOf(fs.readFileSync(path.join(dir, 'stderr'), 'utf8')),
        status: supervisor.status, signal: supervisor.signal, timedOut: supervisor.timed_out } });
  }
  return { receiptFile, receipt, processes: out };
};

const modeFlags = flags => {
  const mode = [];
  for (let i = 0; i < flags.length; i += 1) {
    if (BUDGET_FLAGS.has(flags[i])) i += 1; else mode.push(flags[i]);
  }
  return mode;
};

export const compareRun = (corpusInfo, runDir, options) => {
  const provider = providerOf(options, corpusInfo);
  const { receiptFile, receipt, processes } = observedProcesses(runDir);
  const noble = receipt === null ? null : { binary: receipt.binary, source: { commit: receipt.source?.commit ?? null,
    tracked_changes: receipt.source?.tracked_changes ?? null, embedded: receipt.source?.embedded ?? null },
  toolchain: receipt.toolchain ?? null, config: receipt.config ?? null, run: receipt.run ?? null };
  if (options['expect-binary-sha256'] !== undefined && receipt?.binary?.sha256 !== options['expect-binary-sha256']) {
    provider.stale.push(`binary sha256: run ${receipt?.binary?.sha256} != expected ${options['expect-binary-sha256']}`);
  }
  if (options['expect-source-id'] !== undefined && receipt?.binary?.source_identity !== options['expect-source-id']) {
    provider.stale.push(`source identity: run ${receipt?.binary?.source_identity} != expected ${options['expect-source-id']}`);
  }
  const byText = new Map(corpusInfo.corpus.cases.map(item => [item.text, item]));
  const results = [];
  const outside = [];
  const seen = new Set();
  for (const proc of processes) {
    if (proc.pending) { outside.push({ slug: proc.slug, verdict: 'pending' }); continue; }
    const item = proc.lines.length === 1 ? byText.get(proc.lines[0]) : undefined;
    const mode = modeFlags(proc.flags);
    if (item === undefined || mode.length > 0) {
      outside.push({ slug: proc.slug, lines: proc.lines, flags: proc.flags, verdict: 'unsupported',
        reason: item === undefined ? 'proc input is not a single-line case of this corpus'
          : `calculator mode ${mode.join(' ')} is outside this lane (default display only)` });
      continue;
    }
    seen.add(item.id);
    const reference = provider.stale.length > 0 ? { kind: 'stale-provenance' } : provider.expected(item);
    const judgement = provider.stale.length > 0
      ? { verdict: 'stale-provenance', mismatch: null, reasons: provider.stale } : judge(reference, proc.observed);
    results.push({ id: item.id, text: item.text, slug: proc.slug, binary: proc.binary, flags: proc.flags,
      stdin_sha256: proc.stdin_sha256, wall_ms: proc.wall_ms, observed: proc.observed, reference, ...judgement,
      signature: signature(reference, judgement),
      matches_fault_models: judgement.verdict === 'disagree' ? attribute(item.text, proc.observed) : [] });
  }
  const notRun = corpusInfo.corpus.cases.filter(item => !seen.has(item.id)).map(item => item.id);
  const verdicts = [...results.map(r => r.verdict), ...outside.map(o => o.verdict), ...notRun.map(() => 'not-run')];
  const report = {
    schema: SCHEMAS.report,
    lane: 'optional Wolfram exact-arithmetic lane (#24); distinct from the CALC-12 oracle (#21), #19 acceptance and proofs',
    label: provider.label, expectation_source: provider.expectation_source, reference_status: provider.status,
    reference_reason: provider.reason, lane_result: laneResult(provider, verdicts), stale_provenance: provider.stale,
    provenance: { lane: { ...adapterProvenance(), repository: gitState(), node: process.version },
      reference: provider.provenance, corpus: { path: corpusInfo.path, sha256: corpusInfo.sha256,
        generator: corpusInfo.corpus.generator }, noble, run_dir: path.resolve(runDir), run_receipt: receiptFile },
    summary: { ...summarize(results), ...summarize(outside), ...(notRun.length ? { 'not-run': notRun.length } : {}) },
    results, outside_lane: outside, not_run: notRun, non_claims: NON_CLAIMS,
  };
  const shrinkOf = corpusInfo.corpus.shrink_of;
  if (shrinkOf) report.shrink_selection = selectShrink(shrinkOf, results, corpusInfo.corpus, provider);
  return report;
};

// Real-binary shrink round: the smallest candidate whose classification
// signature equals the original's is the round's minimal case.
const selectShrink = (shrinkOf, results, corpus, provider) => {
  const byId = new Map(results.map(r => [r.id, r]));
  const original = byId.get(corpus.cases[0].id);
  if (original === undefined || original.verdict !== 'disagree') {
    return { status: 'original-not-a-disagreement', original: original?.signature ?? null };
  }
  const kept = corpus.cases.slice(1).map(c => byId.get(c.id)).filter(r => r?.signature === original.signature);
  const chosen = kept[0] ?? original;
  const item = corpus.cases.find(c => c.id === chosen.id);
  return { status: kept.length > 0 ? 'reduced' : 'no-smaller-candidate', original: { text: original.text,
    signature: original.signature }, candidates: corpus.cases.slice(1).map(c => ({ text: c.text,
    signature: byId.get(c.id)?.signature ?? 'not-run' })), minimal: { text: chosen.text, signature: chosen.signature,
    replay: replayArtifact(item, provider) } };
};

// ---- harness-level implementations and bounded shrinking ----------------

const implementationOf = name => {
  if (name === 'calc12-oracle') return text => calc12Run([text]);
  const fault = /^fault:(.+)$/.exec(name)?.[1];
  assert.ok(fault && FAULTS[fault], `unknown implementation ${name}`);
  return text => runFault(fault, [text]);
};
const observeModel = (impl, text) => {
  const out = impl(text);
  return { stdout: out.stdout, status: out.status, signal: null, timedOut: false,
    stderr: out.stderr.map(d => `${d.category} [${d.span[0]},${d.span[1]}) provenance=unavailable`) };
};

// Lane shrink candidates: the #21 candidates that stay inside the arithmetic
// subset, smallest first, at most `limit`.
export const laneCandidates = (text, limit) => shrinkCandidates(text, { limit: Number.MAX_SAFE_INTEGER })
  .filter(candidate => fromText(candidate).supported).slice(0, limit);

// Bounded shrinking. `referenceOf(texts)` returns validated references for a
// batch (one bridge call per round); `observe(text)` runs the implementation.
// A candidate is kept only when its whole classification signature (verdict,
// mismatch kind, reference outcome kind and category) equals the original's.
export const shrink = ({ text, reference, observe, referenceOf, rounds, width }) => {
  const original = signature(reference, judge(reference, observe(text)));
  let current = text;
  const trail = [];
  for (let round = 0; round < rounds; round += 1) {
    const candidates = laneCandidates(current, width);
    if (candidates.length === 0) break;
    const references = referenceOf(candidates);
    const index = candidates.findIndex((candidate, i) => !['oracle-unavailable', 'oracle-error', 'oracle-timeout']
      .includes(references[i].kind) && signature(references[i], judge(references[i], observe(candidate))) === original);
    if (index < 0) break;
    current = candidates[index];
    trail.push({ round: round + 1, tried: candidates.length, kept: current });
  }
  return { input: current, signature: original, rounds_used: trail.length, rounds_limit: rounds,
    candidates_limit: width, trail };
};

const harness = (options, corpusInfo) => {
  assert.ok(options.reference, 'harness needs --reference FILE');
  const provider = providerOf(options, corpusInfo);
  const impl = implementationOf(options.implementation);
  const rounds = Number(options['shrink-rounds'] ?? 4);
  const width = Number(options['shrink-candidates'] ?? 6);
  const scratch = path.join(path.dirname(path.resolve(options.out)), `${path.basename(options.out)}.bridge`);
  const referenceOf = texts => {
    const items = texts.map((text, index) => {
      const interchange = fromText(text).interchange;
      return { id: `shrink-${pad(index)}`, interchange, interchange_sha256: interchangeSha(interchange) };
    });
    const { response } = callBridge(requestOf(items, { arithmetic: provider.arithmetic }),
      { python: options.python, scratch });
    const byId = new Map((response.results ?? []).map(r => [r.id, r.reference]));
    return items.map(item => validateReference(byId.get(item.id) ?? { kind: response.status === 'ok' ? 'oracle-error'
      : response.status, detail: response.reason }));
  };
  const results = corpusInfo.corpus.cases.map(item => {
    const reference = provider.stale.length > 0 ? { kind: 'stale-provenance' } : provider.expected(item);
    const observed = observeModel(impl, item.text);
    const judgement = provider.stale.length > 0
      ? { verdict: 'stale-provenance', mismatch: null, reasons: provider.stale } : judge(reference, observed);
    const row = { id: item.id, text: item.text, observed, reference, ...judgement, signature: signature(reference, judgement),
      matches_fault_models: judgement.verdict === 'disagree' ? attribute(item.text, observed) : [] };
    if (judgement.verdict === 'disagree' && provider.status === 'ok') {
      row.shrunk = shrink({ text: item.text, reference, observe: text => observeModel(impl, text), referenceOf,
        rounds, width });
      const minimal = fromText(row.shrunk.input).interchange;
      row.shrunk.replay = replayArtifact({ text: row.shrunk.input, flags: [], interchange: minimal,
        interchange_sha256: interchangeSha(minimal) }, provider);
    }
    return row;
  });
  return { schema: SCHEMAS.harness, implementation: options.implementation,
    fault_reason: FAULTS[options.implementation.replace(/^fault:/, '')]?.reason ?? null,
    label: provider.label, expectation_source: provider.expectation_source, reference_status: provider.status,
    lane_result: laneResult(provider, results.map(r => r.verdict)), stale_provenance: provider.stale,
    provenance: { lane: { ...adapterProvenance(), repository: gitState(), node: process.version },
      reference: provider.provenance, corpus: { path: corpusInfo.path, sha256: corpusInfo.sha256 } },
    summary: summarize(results), results, non_claims: NON_CLAIMS };
};

// ---- commands ----------------------------------------------------------

const commands = {
  corpus: options => {
    const corpus = buildCorpus({ seed: Number(options.seed), count: Number(options.count), wide: Number(options.wide ?? 0) });
    write(options.out, corpus);
    console.log(`${corpus.cases.length} cases, ${corpus.excluded.length} excluded`);
    return 0;
  },
  vectors: options => {
    const info = loadCorpus(options.corpus);
    const file = vectorsOf(info.corpus, info.sha256);
    write(options.out, file);
    console.log(`${file.vectors.length} vectors`);
    return 0;
  },
  reference: options => {
    const info = loadCorpus(options.corpus);
    assert.ok(LABELS[options.arithmetic], '--arithmetic wolfram|python-fractions');
    const out = fresh(options.out);
    const file = referenceFile(info, info.corpus.cases, { arithmetic: options.arithmetic, python: options.python ?? 'python3',
      kernel: options.kernel ?? null, timeout: Number(options.timeout ?? 30), memory: Number(options.memory ?? 2 ** 30),
      scratch: `${out}.bridge` });
    write(out, file);
    console.log(`${file.label}: ${file.status}${file.reason ? `: ${file.reason}` : ''}`);
    return exitOf(file.status);
  },
  record: options => {
    const info = loadCorpus(options.corpus);
    const provider = providerOf({ reference: options.reference }, info);
    assert.equal(provider.stale.length, 0, provider.stale.join('; '));
    assert.equal(provider.status, 'ok', `reference status ${provider.status}`);
    const cases = info.corpus.cases.map(item => ({ id: item.id, text: item.text, interchange_sha256: item.interchange_sha256,
      expected: provider.expected(item) })).filter(row => ['value', 'diagnostic'].includes(row.expected.kind));
    const file = readJson(options.reference);
    write(options.out, { schema: SCHEMAS.fixtures, expectation_source: RECORDED_SOURCE[file.arithmetic],
      policy_version: POLICY_VERSION, adapter: file.adapter,
      reference_provenance: { file_sha256: provider.provenance.sha256, label: file.label, arithmetic: file.arithmetic,
        environment: file.environment, probes: file.probes, corpus_sha256: file.corpus.sha256, started: file.started },
      cases }, { inRepository: true });
    console.log(`${cases.length} fixtures (${RECORDED_SOURCE[file.arithmetic]})`);
    return 0;
  },
  compare: options => {
    const report = compareRun(loadCorpus(options.corpus), options.run, options);
    write(options.out, report);
    console.log(`${report.label} (${report.expectation_source}): ${report.lane_result} ${JSON.stringify(report.summary)}`);
    return exitOf(report.lane_result);
  },
  harness: options => {
    const report = harness(options, loadCorpus(options.corpus));
    write(options.out, report);
    console.log(`${options.implementation} vs ${report.label}: ${report.lane_result} ${JSON.stringify(report.summary)}`);
    return exitOf(report.lane_result);
  },
  shrink: options => {
    const info = options.input === undefined ? loadCorpus(options.corpus) : null;
    const item = info === null ? { id: 'input', text: options.input } : info.corpus.cases.find(c => c.id === options.case);
    assert.ok(item, `no case ${options.case}`);
    const width = Number(options['shrink-candidates'] ?? 3);
    const texts = [item.text, ...laneCandidates(item.text, width)];
    write(options.out, { schema: SCHEMAS.corpus, policy_version: POLICY_VERSION,
      generator: { shrink_round_of: item.id, corpus_sha256: info?.sha256 ?? null, candidates: width,
        files: filesSha(CORPUS_FILES) },
      shrink_of: { id: item.id, text: item.text },
      cases: texts.map((text, index) => caseOf(`shrink/${pad(index)}`, text, { origin: index === 0 ? 'shrink-original' : 'shrink-candidate' })),
      excluded: [] });
    console.log(texts.join('\n'));
    return 0;
  },
};

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const [command, ...rest] = process.argv.slice(2);
  const { values } = parseArgs({ args: rest, options: Object.fromEntries(['seed', 'count', 'wide', 'out', 'corpus',
    'arithmetic', 'python', 'kernel', 'timeout', 'memory', 'reference', 'fixtures', 'run', 'expect-binary-sha256',
    'expect-source-id', 'expect-kernel-version', 'expect-client-version', 'implementation', 'shrink-rounds',
    'shrink-candidates', 'case', 'input'].map(name => [name, { type: 'string' }])) });
  assert.ok(commands[command], `usage: lane.mjs ${Object.keys(commands).join('|')} …`);
  assert.ok(values.out, '--out NEW_FILE is required');
  process.exitCode = commands[command](values);
}
