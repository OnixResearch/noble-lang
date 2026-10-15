#!/usr/bin/env node
// Runs the four CALC-12 invalid-evidence controls (plus three calibration
// controls) through `gate.mjs` and keeps a receipt in a fresh directory
// outside the repository. Distinct from the #19 acceptance runner and from
// any proof: nothing here executes calculator vectors or checks a proof.
//
//   node verification/calculator-v1/evidence-controls/controls.mjs \
//     --binary NOBLE --binary-sha256 SHA --out NEW_DIR [--comparison ORACLE_REPORT.json]
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseArgs } from 'node:util';
import { CLAIM_SCHEMA, RULES, judgeClaim } from './gate.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, '../../..');
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const hashFile = file => sha(fs.readFileSync(file));

const { values: options } = parseArgs({ options: {
  binary: { type: 'string' }, 'binary-sha256': { type: 'string' }, out: { type: 'string' },
  comparison: { type: 'string' },
} });
for (const key of ['binary', 'binary-sha256', 'out']) assert.ok(options[key], `missing --${key}`);
const binary = path.resolve(options.binary);
const out = path.resolve(options.out);
assert.ok(!fs.existsSync(out), 'output directory must be fresh');
assert.ok(!out.startsWith(`${root}${path.sep}`), 'receipts stay outside the repository');
const binarySha = hashFile(binary);
assert.equal(binarySha, options['binary-sha256'], 'binary SHA-256 differs from the owner-supplied hash');
fs.mkdirSync(out);

// Subject manifest: the pinned binary plus every fixture it embeds.
const calculatorRs = fs.readFileSync(path.join(root, 'crates/noble-cli/src/calculator.rs'), 'utf8');
const fixtures = Object.fromEntries([...calculatorRs.matchAll(/include_bytes!\("\.\.\/\.\.\/(noble-contracts\/src\/fixtures\/[^"]+)"\)/g)]
  .map(match => `crates/${match[1]}`).sort().map(file => [file, hashFile(path.join(root, file))]));
assert.ok(Object.keys(fixtures).length >= 11, 'calculator.rs embeds the guest fixtures');
const manifest = { binary_sha256: binarySha, fixtures, profile: 'Calculator-v1' };

// A real but unrelated Wasm module: Core-Bootstrap `1 2 +` built and run by
// the same pinned binary (it even yields 3, the value of calculator `1+2`).
const unrelated = path.join(out, 'unrelated-wasm');
fs.writeFileSync(path.join(out, 'unrelated.noble'), '1 2 +\n');
const emitted = spawnSync(binary, ['run', path.join(out, 'unrelated.noble'), '--opt', 'off', '--emit', unrelated],
  { encoding: 'utf8', env: { PATH: '/nonexistent', LANG: 'C', TMPDIR: out }, timeout: 600_000 });
fs.writeFileSync(path.join(out, 'unrelated.stdout'), emitted.stdout ?? '');
fs.writeFileSync(path.join(out, 'unrelated.stderr'), emitted.stderr ?? '');
assert.equal(emitted.status, 0, 'unrelated Wasm emission failed');
const wasm = path.join(unrelated, 'engine/module-1.wasm');
const wasmReport = path.join(unrelated, '1.json');

const bound = { kind: 'calculator-binary', binary_sha256: binarySha, fixtures };
const claim = fields => ({ schema: CLAIM_SCHEMA, ...fields });
const artifact = (file, extra = {}) => ({ path: file, sha256: hashFile(file), ...extra });
const controls = [
  { id: 'proof-for-wrapping-I64', intended: ['wrong-numeric-model'], claim: claim({
    kind: 'proof', statement: 'The #15 Mathlib comparator for wrapping i64.add/i64.sub/i64.mul certifies calculator + - *.',
    subject: bound, model: 'wrapping-i64', covers: ['addition', 'subtraction', 'multiplication'],
    artifacts: [artifact(path.join(root, 'verification/mathlib-i64/compare.mjs'), { role: 'proof-script' })] }) },
  { id: 'proof-for-handwritten-reference-only', intended: ['subject-not-actual-implementation'], claim: claim({
    kind: 'proof', statement: 'Normalization and addition are correct, proved about the hand-written CALC-12 reference rationals.',
    subject: { kind: 'reference-model', path: 'verification/calculator-v1/oracle/rational.mjs',
      sha256: hashFile(path.join(root, 'verification/calculator-v1/oracle/rational.mjs')) },
    model: 'exact-rational', covers: ['normalization', 'addition'],
    artifacts: [artifact(path.join(root, 'verification/calculator-v1/oracle/rational.mjs'), { role: 'proof-subject' })] }) },
  { id: 'proof-with-unmet-zero-divisor-precondition', intended: ['unmet-zero-divisor-precondition'], claim: claim({
    kind: 'proof', statement: 'Calculator division returns the exact quotient for all inputs, assuming the divisor is nonzero.',
    subject: bound, model: 'exact-rational', covers: ['division'], domain: 'all-inputs',
    preconditions: [{ kind: 'nonzero-divisor', text: 'divisor <> 0' }], error_branches: [], artifacts: [] }) },
  { id: 'unrelated-Wasm', intended: ['unrelated-wasm'], claim: claim({
    kind: 'test-observation', statement: 'Calculator `1+2` evaluates to 3; the cited Wasm module is the evidence.',
    subject: bound, model: 'exact-rational', covers: ['addition'], domain: 'finite-vectors',
    artifacts: [artifact(wasm, { role: 'wasm-module', report: wasmReport, produced_by_binary_sha256: binarySha })] }) },
  // Calibration: the gate must also refuse a universal reading of tests and
  // must not refuse a properly bound claim merely for its kind.
  { id: 'calibration/test-claimed-universal', intended: ['tests-claimed-universal'], claim: claim({
    kind: 'test-observation', quantifier: 'universal', statement: 'The parser is correct for every input because the vectors passed.',
    subject: bound, model: 'exact-rational', covers: ['parsing'], domain: 'finite-vectors', artifacts: [] }) },
  { id: 'calibration/bound-proof-shape', intended: [], expected_decision: 'not-refused-structurally', claim: claim({
    kind: 'proof', statement: 'Shape-only control: a bound exact-model division claim with an explicit DivisionByZero branch.',
    subject: bound, model: 'exact-rational', covers: ['division'], domain: 'all-inputs', preconditions: [],
    error_branches: ['DivisionByZero'], artifacts: [] }) },
];
if (options.comparison !== undefined) {
  const report = path.resolve(options.comparison);
  const parsed = JSON.parse(fs.readFileSync(report, 'utf8'));
  controls.push({ id: 'calibration/bound-test-observation', intended: [], expected_decision: 'admissible-test-observation',
    comparison_summary: parsed.summary, claim: claim({
      kind: 'test-observation', statement: 'Finite CALC vectors agree with the independent oracle on the pinned binary.',
      subject: bound, model: 'exact-rational', covers: ['addition', 'division'], domain: 'finite-vectors',
      error_branches: ['DivisionByZero'], artifacts: [artifact(report, { role: 'oracle-comparison' })] }) });
}

const results = controls.map(control => {
  const file = path.join(out, 'claims', `${control.id.replace(/[^A-Za-z0-9._-]+/g, '_')}.json`);
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, `${JSON.stringify(control.claim, null, 2)}\n`);
  const verdict = judgeClaim(control.claim, manifest, root);
  const codes = verdict.refusals.map(r => r.code).sort();
  const ok = control.intended.length > 0
    ? verdict.decision === 'refused' && JSON.stringify(codes) === JSON.stringify([...control.intended].sort())
    : verdict.decision === control.expected_decision;
  return { id: control.id, claim_file: file, claim_sha256: hashFile(file), intended: control.intended,
    expected_decision: control.expected_decision ?? 'refused', ...verdict, ok };
});

const receipt = {
  schema: 'noble-calculator-evidence-controls/v1',
  lane: 'CALC-12 invalid-evidence refusal checks (distinct from the #19 acceptance runner and from any proof)',
  binary: { path: binary, sha256: binarySha }, manifest, rules: RULES,
  gate_sha256: hashFile(path.join(here, 'gate.mjs')), controls_sha256: hashFile(fileURLToPath(import.meta.url)),
  node: process.version, results,
  all_ok: results.every(r => r.ok),
  non_claims: [
    'No proof is checked or accepted; `not-refused-structurally` is not acceptance.',
    'An admissible test observation establishes no universal property.',
    'No canonical case state or evidence changes.',
  ],
};
fs.writeFileSync(path.join(out, 'receipt.json'), `${JSON.stringify(receipt, null, 2)}\n`);
fs.writeFileSync(path.join(out, receipt.all_ok ? 'ALL_DONE' : 'FAILED'), '');
for (const r of results) console.log(`${r.ok ? 'ok  ' : 'BAD '} ${r.id}: ${r.decision} [${r.refusals.map(x => x.code).join(', ')}]`);
process.exitCode = receipt.all_ok ? 0 : 1;
