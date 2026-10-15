// CALC-12 invalid-evidence refusal gate (CALC-EVIDENCE-01/02).
//
// Judges an evidence *claim* about the exact calculator against a subject
// manifest of the actual implementation (calculator binary hash and the
// embedded guest fixture hashes). It never runs a prover, never accepts a
// proof, and never changes a case state. The strongest outcome is
// `admissible-test-observation` (finite tests only) or
// `not-refused-structurally` for a proof claim, which still needs a proof
// checker this lane does not have.
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';

export const CLAIM_SCHEMA = 'noble-calculator-evidence-claim/v1';
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');

// Refusal codes, one per CALC-EVIDENCE rule.
export const RULES = Object.freeze({
  'wrong-numeric-model': 'CALC-EVIDENCE-02: a claim about wrapping I64, floating point or another model cannot certify exact rational calculator arithmetic',
  'subject-not-actual-implementation': 'CALC-EVIDENCE-01/02: the claim must bind the actual calculator binary and guest modules, not a rewritten or hand-written reference',
  'unmet-zero-divisor-precondition': 'CALC-EVIDENCE-02: zero-divisor preconditions and DivisionByZero branches must stay explicit; a nonzero-divisor assumption cannot cover all inputs',
  'unrelated-wasm': 'CALC-EVIDENCE-01: Wasm evidence must come from the actual calculator backend (Calculator-v1 profile, produced by the pinned binary)',
  'tests-claimed-universal': 'CALC-EVIDENCE-01: passing finite tests cannot establish parser correctness or a universal arithmetic theorem',
  'artifact-hash-mismatch': 'every cited artifact must exist and match its recorded SHA-256',
  'malformed-claim': 'the claim does not follow the claim schema',
});

const refuse = (refusals, code, detail) => refusals.push({ code, rule: RULES[code], detail });

// `manifest` = { binary_sha256, fixtures: { path: sha256 }, profile: 'Calculator-v1' }.
// `root` resolves relative artifact paths.
export const judgeClaim = (claim, manifest, root) => {
  const refusals = [];
  if (claim?.schema !== CLAIM_SCHEMA || !['proof', 'test-observation'].includes(claim.kind)
    || typeof claim.subject !== 'object' || !Array.isArray(claim.covers) || !Array.isArray(claim.artifacts)) {
    refuse(refusals, 'malformed-claim', 'schema, kind, subject, covers or artifacts missing');
    return { decision: 'refused', refusals };
  }

  // Cited artifacts must exist with the recorded bytes.
  const artifacts = [];
  for (const artifact of claim.artifacts) {
    const file = path.resolve(root, artifact.path);
    if (!fs.existsSync(file)) { refuse(refusals, 'artifact-hash-mismatch', `${artifact.path} is missing`); continue; }
    const actual = sha(fs.readFileSync(file));
    if (actual !== artifact.sha256) refuse(refusals, 'artifact-hash-mismatch', `${artifact.path}: ${actual} != ${artifact.sha256}`);
    artifacts.push({ ...artifact, file, actual });
  }

  // Model: exact rational only (CALC-NUM-01..05).
  if (claim.model !== 'exact-rational') {
    refuse(refusals, 'wrong-numeric-model', `claim model is ${claim.model}`);
  }

  // Subject: the pinned calculator binary and every embedded guest fixture.
  const subject = claim.subject;
  const fixtureKeys = Object.keys(manifest.fixtures).sort();
  const boundFixtures = subject.fixtures !== undefined
    && JSON.stringify(Object.keys(subject.fixtures).sort()) === JSON.stringify(fixtureKeys)
    && fixtureKeys.every(key => subject.fixtures[key] === manifest.fixtures[key]);
  if (subject.kind !== 'calculator-binary' || subject.binary_sha256 !== manifest.binary_sha256 || !boundFixtures) {
    refuse(refusals, 'subject-not-actual-implementation',
      `subject ${subject.kind} binary=${subject.binary_sha256 ?? 'none'} fixtures=${boundFixtures ? 'bound' : 'not bound'}`);
  }

  // Division coverage must keep the zero-divisor branch explicit.
  const coversDivision = claim.covers.some(item => ['division', 'all-arithmetic', 'integer-power'].includes(item));
  if (coversDivision) {
    const assumesNonzero = (claim.preconditions ?? []).some(p => p.kind === 'nonzero-divisor');
    const branch = (claim.error_branches ?? []).includes('DivisionByZero');
    const domain = claim.domain ?? 'all-inputs';
    if (!branch || (assumesNonzero && domain === 'all-inputs')) {
      refuse(refusals, 'unmet-zero-divisor-precondition',
        `domain=${domain}, nonzero-divisor precondition=${assumesNonzero}, DivisionByZero branch=${branch}`);
    }
  }

  // Wasm artifacts: produced by the pinned binary in the Calculator-v1 profile.
  for (const artifact of artifacts.filter(a => a.role === 'wasm-module')) {
    const report = artifact.report === undefined ? null : path.resolve(root, artifact.report);
    let observed = null;
    if (report !== null && fs.existsSync(report)) observed = JSON.parse(fs.readFileSync(report, 'utf8'));
    const producedBy = artifact.produced_by_binary_sha256;
    if (observed?.profile !== manifest.profile || producedBy !== manifest.binary_sha256) {
      refuse(refusals, 'unrelated-wasm',
        `${artifact.path}: report profile=${observed?.profile ?? 'none'}, produced by ${producedBy ?? 'unknown'}`);
    }
  }

  // Test observations never become a universal claim.
  if (claim.kind === 'test-observation' && claim.quantifier === 'universal') {
    refuse(refusals, 'tests-claimed-universal', 'a finite test observation states a universal quantifier');
  }

  if (refusals.length > 0) return { decision: 'refused', refusals };
  return claim.kind === 'proof'
    ? { decision: 'not-refused-structurally', refusals, note: 'a proof checker and CALC-EVIDENCE-02 proof report are required; nothing is accepted here' }
    : { decision: 'admissible-test-observation', refusals, note: 'finite observation only; establishes no universal property' };
};
