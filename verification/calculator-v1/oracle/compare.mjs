#!/usr/bin/env node
// CALC-12 comparison lane: judges observed calculator processes against the
// independent oracle. It reads only the raw stdin/argv/stdout/stderr/status
// retained by a #19 acceptance-runner run (or produced in-process by a
// harness-level fault model); it never reads the runner's own pass/fail
// classification or the cases file's expected values when judging.
//
//   compare.mjs receipts --run RUN_DIR --out NEW_FILE [--ids-prefix CALC-01/,…]
//   compare.mjs cases --out NEW_FILE
//   compare.mjs generate --seed N --count N [--sessions N] --out NEW_FILE
//   compare.mjs harness --vectors FILE --implementation fault:NAME|oracle --out NEW_FILE
//            [--shrink-rounds N] [--shrink-candidates N]
//   compare.mjs shrink --input TEXT [--shrink-candidates N] --out NEW_FILE
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseArgs } from 'node:util';
import { JUDGED_SPAN_BASES, run } from './calculator.mjs';
import { FAULTS, FAULT_NAMES, runFault } from './faults.mjs';
import { determined, generateExpressions, generateSessions, shrinkCandidates } from './generate.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, '../../..');
const ORACLE_FILES = ['rational.mjs', 'calculator.mjs', 'generate.mjs', 'faults.mjs', 'compare.mjs'];
// A guest diagnostic: `Category [s,e) provenance=P`; ResourceLimit adds its budget
// category (`ResourceLimit EvaluationWork [s,e) ...`).
const DIAGNOSTIC = /^([A-Za-z]+(?: [A-Za-z]+)?) \[(\d+),(\d+)\) provenance=(\S+)$/;
const VECTOR_SCHEMA = 'noble-calculator-vectors/v1';
const NON_CLAIMS = [
  'Agreement on finitely many inputs does not establish parser correctness or any universal arithmetic theorem.',
  'The oracle is a test reference, not a proof; no CALC-EVIDENCE-02 proof claim is made.',
  'No canonical case state or evidence is changed by this report.',
  'The oracle has no model of the seven guest budgets; a ResourceLimit is reported as a divergence, never as agreement.',
];

const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const fresh = file => {
  const resolved = path.resolve(file);
  assert.ok(!fs.existsSync(resolved), `${resolved} must not exist`);
  assert.ok(!resolved.startsWith(`${root}${path.sep}`), 'outputs stay outside the repository');
  return resolved;
};
const write = (file, value) => fs.writeFileSync(fresh(file), `${JSON.stringify(value, null, 2)}\n`);
const provenance = () => ({
  oracle_files: Object.fromEntries(ORACLE_FILES.map(name => [name, sha(fs.readFileSync(path.join(here, name)))])),
  node: process.version,
  node_path: process.execPath,
});
const linesOf = text => {
  const parts = text.split('\n');
  if (parts.at(-1) === '') parts.pop();
  return parts;
};
const expectedText = outcome => (outcome.kind === 'value' ? outcome.display
  : `${outcome.category} [${outcome.span[0]},${outcome.span[1]}) provenance=unavailable`);

// Judges one process. `observed` = { stdout, stderr, status, timedOut, signal }.
export const judge = (expected, observed) => {
  const reasons = [];
  if (observed.timedOut) return { verdict: 'not-comparable', reasons: ['process timed out'] };
  if (observed.signal !== null && observed.signal !== undefined) {
    return { verdict: 'not-comparable', reasons: [`signal ${observed.signal}`] };
  }
  if (![0, 1].includes(observed.status)) return { verdict: 'not-comparable', reasons: [`exit status ${observed.status}`] };
  const diagnostics = [];
  for (const line of observed.stderr) {
    const match = DIAGNOSTIC.exec(line);
    if (match === null) return { verdict: 'not-comparable', reasons: [`non-guest stderr line: ${line}`] };
    diagnostics.push({ category: match[1], span: [Number(match[2]), Number(match[3])], provenance: match[4] });
  }
  if (diagnostics.some(d => d.category.startsWith('ResourceLimit'))) {
    return { verdict: 'budget-divergence', reasons: ['observed ResourceLimit; the oracle has no budget model'] };
  }
  const undetermined = expected.outcomes.filter(o => !determined(o));
  if (observed.stdout.length !== expected.stdout.length
    || observed.stdout.some((value, index) => value !== expected.stdout[index])) {
    reasons.push(`stdout ${JSON.stringify(observed.stdout)} != oracle ${JSON.stringify(expected.stdout)}`);
  }
  const spansNotJudged = [];
  if (diagnostics.length !== expected.stderr.length) {
    reasons.push(`diagnostics ${JSON.stringify(observed.stderr)} != oracle ${JSON.stringify(expected.stderr.map(expectedText))}`);
  } else {
    expected.stderr.forEach((want, index) => {
      const got = diagnostics[index];
      if (got.category !== want.category) reasons.push(`diagnostic ${index}: ${got.category} != oracle ${want.category}`);
      if (got.provenance !== 'unavailable') reasons.push(`diagnostic ${index}: provenance=${got.provenance} is not explicit unavailable`);
      const judged = JUDGED_SPAN_BASES.has(want.basis) && !want.notes.some(n => n.startsWith('ambiguous:'));
      if (judged && (got.span[0] !== want.span[0] || got.span[1] !== want.span[1])) {
        reasons.push(`diagnostic ${index}: span [${got.span}) != oracle [${want.span}) (${want.basis})`);
      } else if (!judged) {
        spansNotJudged.push({ index, observed: got.span, oracle: want.span, basis: want.basis });
      }
    });
  }
  if (observed.status !== expected.status) reasons.push(`exit status ${observed.status} != oracle ${expected.status}`);
  if (undetermined.length > 0) {
    return { verdict: 'undetermined', reasons: [...reasons, 'oracle outcome not spec-determined'], spansNotJudged };
  }
  return { verdict: reasons.length === 0 ? 'agree' : 'disagree', reasons, spansNotJudged };
};

const attribute = (lines, mode, observed) => FAULT_NAMES.filter(name => {
  const predicted = runFault(name, lines, { mode });
  return predicted.status === observed.status
    && JSON.stringify(predicted.stdout) === JSON.stringify(observed.stdout)
    && JSON.stringify(predicted.stderr.map(d => d.category))
      === JSON.stringify(observed.stderr.map(line => DIAGNOSTIC.exec(line)?.[1] ?? line));
});

const modeOf = flags => {
  assert.ok(!flags.includes('--framed'), 'framed vectors are outside this comparison');
  return flags.includes('--finite-decimal') ? 'finite-decimal' : 'default';
};

const compareObserved = (lines, flags, observed) => {
  const mode = modeOf(flags);
  const expected = run(lines, { mode });
  const result = judge(expected, observed);
  return {
    lines, flags, mode,
    oracle: { stdout: expected.stdout, stderr: expected.stderr.map(expectedText), status: expected.status,
      notes: expected.outcomes.flatMap(o => o.notes) },
    observed: { stdout: observed.stdout, stderr: observed.stderr, status: observed.status },
    ...result,
    matches_fault_models: result.verdict === 'disagree' ? attribute(lines, mode, observed) : [],
  };
};

const summarize = results => {
  const summary = {};
  for (const item of results) summary[item.verdict] = (summary[item.verdict] ?? 0) + 1;
  return summary;
};

const receipts = options => {
  const runDir = path.resolve(options.run);
  const receiptFile = ['receipt.json', 'receipt.partial.json'].map(f => path.join(runDir, f)).find(fs.existsSync);
  const receipt = receiptFile ? JSON.parse(fs.readFileSync(receiptFile, 'utf8')) : null;
  const prefixes = (options['ids-prefix'] ?? '').split(',').filter(Boolean);
  const vectorsDir = path.join(runDir, 'vectors');
  const ids = new Map([...(receipt?.vectors ?? []), ...(receipt?.controls ?? [])]
    .map(r => [r.id.replace(/[^A-Za-z0-9._-]+/g, '_'), r.id]));
  const results = [];
  for (const slug of fs.readdirSync(vectorsDir).sort()) {
    const dir = path.join(vectorsDir, slug);
    const id = ids.get(slug) ?? slug;
    if (prefixes.length > 0 && !prefixes.some(prefix => id.startsWith(prefix))) continue;
    const resultFile = path.join(dir, 'supervisor.json');
    if (!fs.existsSync(resultFile)) { results.push({ id, verdict: 'pending', reasons: ['no supervisor result yet'] }); continue; }
    const spec = JSON.parse(fs.readFileSync(path.join(dir, 'supervisor-spec.json'), 'utf8'));
    const supervisor = JSON.parse(fs.readFileSync(resultFile, 'utf8'));
    const flags = spec.argv.slice(spec.argv.indexOf('calculator') + 1);
    const stdinBytes = fs.readFileSync(path.join(dir, 'stdin'));
    const observed = {
      stdout: linesOf(fs.readFileSync(path.join(dir, 'stdout'), 'utf8')),
      stderr: linesOf(fs.readFileSync(path.join(dir, 'stderr'), 'utf8')),
      status: supervisor.status, signal: supervisor.signal, timedOut: supervisor.timed_out,
    };
    results.push({ id, binary: spec.argv[0], stdin_sha256: sha(stdinBytes), wall_ms: supervisor.wall_ms,
      ...compareObserved(linesOf(stdinBytes.toString('utf8')), flags, observed) });
  }
  write(options.out, {
    schema: 'noble-calculator-oracle-comparison/v1', lane: 'CALC-12 independent oracle (distinct from #19 acceptance and from proofs)',
    run_dir: runDir, run_receipt: receiptFile ?? null,
    run_binary: receipt?.binary ?? null, run_result: receipt?.result ?? null,
    provenance: provenance(), summary: summarize(results), results, non_claims: NON_CLAIMS,
  });
  console.log(JSON.stringify(summarize(results)));
};

// Oracle versus the hand-written expectations of the cases file (a separate
// three-way check: neither side is the CLI).
const cases = options => {
  const file = path.join(root, 'specs/conformance/calculator-cases.json');
  const byId = new Map(JSON.parse(fs.readFileSync(file, 'utf8')).cases.map(c => [c.id, c]));
  const rows = [];
  const check = (id, lines, mode, want) => {
    const got = run(lines, { mode });
    const last = got.outcomes.at(-1);
    const actual = last.kind === 'value' ? { value: last.display }
      : last.kind === 'silent' ? { silent: true } : { category: last.category, span: last.span };
    const agrees = Object.entries(want).every(([key, value]) => JSON.stringify(actual[key]) === JSON.stringify(value));
    rows.push({ id, lines, mode, cases_file: want, oracle: actual, agrees });
  };
  const display = (n, d) => (d === '1' ? n : `${n}/${d}`);
  byId.get('CALC-01').input.vectors.forEach((v, i) => check(`CALC-01/${i + 1}`, [v.expression], 'default', { value: display(v.numerator, v.denominator) }));
  byId.get('CALC-02').input.vectors.forEach((v, i) => check(`CALC-02/${i + 1}`, [v.expression], 'default', { category: v.category, span: v.span }));
  byId.get('CALC-03').input.vectors.forEach((v, i) => check(`CALC-03/${i + 1}`, [v.expression], 'default', { category: v.category }));
  byId.get('CALC-09').input.vectors.forEach((v, i) => check(`CALC-09/${i + 1}`, [v.fraction], v.mode, { value: v.display }));
  const spell = op => (op.operation === 'define-variable' ? `let ${op.name} = ${op.expression}`
    : op.operation === 'define-function' ? `fn ${op.name}(${op.parameters.join(', ')})=${op.body}` : op.expression);
  const ops = byId.get('CALC-06').input.operations;
  ops.forEach((op, i) => {
    // The cases file states no outcome for a successful definition; only
    // stated values and errors are compared.
    if (op.expected_error === undefined && op.expected_fraction === undefined) return;
    const lines = ops.slice(0, i + 1).map(spell);
    const [n, d] = (op.expected_fraction ?? '').split('/');
    const want = op.expected_error ? { category: op.expected_error } : { value: display(n, d) };
    check(`CALC-06/${i + 1}`, lines, 'default', want);
  });
  // CALC-07 variants with a README spelling; imports and resources have none.
  const admission = {
    'unused-body-with-unknown-name': ['fn h()=q'],
    'duplicate-parameter-name': ['fn f(y, y)=y'],
    'self-recursive-definition': ['fn f(y)=f(y)'],
    'zero-argument-function-body-1-divided-by-0': ['fn z()=1 / 0'],
    'call-that-zero-argument-function': ['fn z()=1 / 0', 'z()'],
  };
  for (const variant of byId.get('CALC-07').input.variants) {
    const lines = admission[variant.case];
    if (lines === undefined) continue;
    const want = variant.outcome === 'definition-accepted-body-not-evaluated' ? { silent: true } : { category: variant.outcome };
    check(`CALC-07/${variant.case}`, lines, 'default', want);
  }
  write(options.out, { schema: 'noble-calculator-oracle-cases-check/v1', cases_file_sha256: sha(fs.readFileSync(file)),
    provenance: provenance(), agree: rows.filter(r => r.agrees).length, total: rows.length, rows, non_claims: NON_CLAIMS });
  console.log(`${rows.filter(r => r.agrees).length}/${rows.length} cases-file expectations agree with the oracle`);
};

const expectOf = outcome => {
  if (outcome.kind === 'value') return { value: outcome.display };
  if (outcome.kind === 'silent') return { silent: true };
  const judged = JUDGED_SPAN_BASES.has(outcome.basis);
  return { diagnostic: { category: outcome.category, span: judged ? outcome.span : null } };
};

const generate = options => {
  const seed = Number(options.seed);
  const count = Number(options.count);
  const sessions = Number(options.sessions ?? 0);
  assert.ok(Number.isInteger(seed) && seed >= 0 && Number.isInteger(count) && count > 0);
  const vectors = generateExpressions({ seed, count }).map((item, index) => ({
    id: `CALC-12/gen-${seed}/${String(index + 1).padStart(2, '0')}`, case: 'CALC-12-generated', kind: 'line',
    support: 'cli', flags: [], operations: [{ line: item.text, expect: expectOf(item.outcome) }],
  }));
  generateSessions({ seed, count: sessions }).forEach((item, index) => vectors.push({
    id: `CALC-12/gen-${seed}/session-${index + 1}`, case: 'CALC-12-generated', kind: 'session', support: 'cli', flags: [],
    operations: item.lines.map((line, i) => ({ line, expect: expectOf(item.result.outcomes[i]) })),
  }));
  write(options.out, { schema: VECTOR_SCHEMA, generator: { seed, count, sessions, provenance: provenance() }, vectors });
  console.log(`${vectors.length} vectors`);
};

// In-process implementations: the oracle itself (sanity) or a fault model.
const implementation = name => {
  if (name === 'oracle') return (lines, mode) => run(lines, { mode });
  const fault = /^fault:(.+)$/.exec(name)?.[1];
  assert.ok(fault && FAULTS[fault], `unknown implementation ${name}`);
  return (lines, mode) => runFault(fault, lines, { mode });
};
const observe = (impl, lines, mode) => {
  const out = impl(lines, mode);
  return { stdout: out.stdout, stderr: out.stderr.map(expectedText), status: out.status, signal: null, timedOut: false };
};

const harness = options => {
  const file = JSON.parse(fs.readFileSync(options.vectors, 'utf8'));
  assert.equal(file.schema, VECTOR_SCHEMA);
  const impl = implementation(options.implementation);
  const rounds = Number(options['shrink-rounds'] ?? 4);
  const width = Number(options['shrink-candidates'] ?? 6);
  const results = file.vectors.map(vector => {
    const lines = vector.operations.map(op => op.line);
    const mode = modeOf(vector.flags);
    const compared = compareObserved(lines, vector.flags, observe(impl, lines, mode));
    const item = { id: vector.id, ...compared };
    if (compared.verdict === 'disagree' && lines.length === 1) {
      // Bounded shrinking: at most `rounds` rounds of `width` candidates.
      let current = lines[0];
      const trail = [];
      for (let round = 0; round < rounds; round += 1) {
        const next = shrinkCandidates(current, { limit: width })
          .find(candidate => compareObserved([candidate], vector.flags, observe(impl, [candidate], mode)).verdict === 'disagree');
        if (next === undefined) break;
        trail.push(next);
        current = next;
      }
      item.shrunk = { input: current, rounds_used: trail.length, trail,
        verdict: compareObserved([current], vector.flags, observe(impl, [current], mode)) };
    }
    return item;
  });
  write(options.out, { schema: 'noble-calculator-oracle-harness/v1', implementation: options.implementation,
    vectors_file: path.resolve(options.vectors), vectors_sha256: sha(fs.readFileSync(options.vectors)),
    provenance: provenance(), summary: summarize(results), results, non_claims: NON_CLAIMS });
  console.log(`${options.implementation}: ${JSON.stringify(summarize(results))}`);
};

// One bounded shrinking round against a real binary: the disagreeing input
// plus at most N strictly smaller, spec-determined candidates, as a #19
// runner vector file. Each candidate costs one CLI process; the caller runs
// the file, then `receipts` names the smallest candidate that still disagrees.
const shrink = options => {
  const width = Number(options['shrink-candidates'] ?? 3);
  const inputs = [options.input, ...shrinkCandidates(options.input, { limit: width })];
  const vectors = inputs.map((line, index) => ({
    id: `CALC-12/shrink/${String(index).padStart(2, '0')}`, case: 'CALC-12-shrink', kind: 'line', support: 'cli',
    flags: [], operations: [{ line, expect: expectOf(run([line]).outcomes[0]) }],
  }));
  write(options.out, { schema: VECTOR_SCHEMA, shrink: { input: options.input, candidates: width, provenance: provenance() }, vectors });
  console.log(inputs.join('\n'));
};

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const [command, ...rest] = process.argv.slice(2);
  const { values } = parseArgs({ args: rest, options: {
    run: { type: 'string' }, out: { type: 'string' }, 'ids-prefix': { type: 'string' },
    seed: { type: 'string' }, count: { type: 'string' }, sessions: { type: 'string' },
    vectors: { type: 'string' }, implementation: { type: 'string' },
    'shrink-rounds': { type: 'string' }, 'shrink-candidates': { type: 'string' }, input: { type: 'string' },
  } });
  const commands = { receipts, cases, generate, harness, shrink };
  assert.ok(commands[command], 'usage: compare.mjs receipts|cases|generate|harness|shrink …');
  assert.ok(values.out, '--out NEW_FILE is required');
  commands[command](values);
}
