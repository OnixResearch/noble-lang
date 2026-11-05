// Claim-strength rules shared by the M2 and M3 coverage joins (#88).
//
// Every citation of a cited function names a theorem, its statement shape
// (`fixture`, `conditional` or `universal`) and its axiom class (`strict`, or
// `native` when compiled evaluation is trusted). The gates' Lean checkers
// compute both from the elaborated statement and the transitive axiom set
// (verification/coverage-strength.lean); these rules check the record's form,
// derive the status a citation set allows, and compare the record with what
// the checkers computed. Strength classes describe statement shape only: they
// do not show that the reference model is correct.

export const STRENGTHS = ['fixture', 'conditional', 'universal'];
export const AXIOM_CLASSES = ['strict', 'native'];
// Statuses that carry citations. Only a strict universal citation makes a
// function `proved`; fixture and conditional citations make it
// `fixture-checked`, which the totals never count as proved.
export const CITED_STATUSES = ['proved', 'fixture-checked'];
export const NON_CLAIM =
  'strength classes describe statement shape only; they do not show that the reference model is correct';

const isStrictUniversal = citation => citation?.strength === 'universal' && citation?.axioms === 'strict';

// Record-only rules for one entry: citation form, and the status it allows.
export function citationProblems(entry, fail) {
  if (!CITED_STATUSES.includes(entry.status)) {
    if (entry.citations !== undefined) {
      fail('STATUS', `${entry.lean}: citations present but status is ${entry.status}`);
    }
    return;
  }
  if (!Array.isArray(entry.citations) || entry.citations.length === 0) {
    fail('STATUS', `${entry.lean}: status ${entry.status} requires a non-empty citations array`);
    return;
  }
  const names = new Set();
  for (const citation of entry.citations) {
    const keys = citation && typeof citation === 'object' ? Object.keys(citation).sort().join(',') : '';
    if (keys !== 'axioms,strength,theorem' || typeof citation.theorem !== 'string'
      || !STRENGTHS.includes(citation.strength) || !AXIOM_CLASSES.includes(citation.axioms)) {
      fail('CITATION-FORM', `${entry.lean}: citation ${JSON.stringify(citation)} must be exactly ` +
        `{theorem, strength: ${STRENGTHS.join('|')}, axioms: ${AXIOM_CLASSES.join('|')}}`);
      continue;
    }
    if (names.has(citation.theorem)) fail('CITATION-FORM', `${entry.lean}: duplicate citation ${citation.theorem}`);
    names.add(citation.theorem);
  }
  const universal = entry.citations.some(isStrictUniversal);
  if (entry.status === 'proved' && !universal) {
    const shapes = [...new Set(entry.citations.map(c => `${c?.strength}/${c?.axioms}`))].sort();
    fail('STRENGTH-STATUS', `${entry.lean}: status proved requires a universal citation with the strict ` +
      `axiom set, but every citation is ${shapes.join(', ')}; the status is fixture-checked`);
  }
  if (entry.status === 'fixture-checked' && universal) {
    fail('STRENGTH-STATUS', `${entry.lean}: a strict universal citation makes this proved, not fixture-checked`);
  }
}

// The theorem names an entry cites, for the Lean checker payloads.
export const citedTheorems = entry => (entry.citations ?? []).map(citation => citation.theorem);

// Parse the checkers' `<tag>: STRENGTH <theorem> <constant> <strength> <axioms>` lines.
export function parseStrengthLines(text, tag) {
  const pattern = new RegExp(`${tag}: STRENGTH (\\S+) (\\S+) (\\S+) (\\S+)`, 'g');
  return [...text.matchAll(pattern)].map(([, theorem, constant, strength, axioms]) =>
    ({ theorem, constant, strength, axioms }));
}

// Compare what the checkers computed with the record. Every recorded citation
// must be computed exactly once with the recorded strength and axiom class,
// and nothing else may be computed: a missed or extra declaration fails loudly.
export function strengthReport(record, computed) {
  const problems = [];
  const fail = (code, msg) => problems.push({ code, msg });
  const expected = new Map();
  for (const entry of record.functions ?? []) {
    for (const citation of entry.citations ?? []) {
      expected.set(`${entry.lean} ${citation.theorem}`, citation);
    }
  }
  const seen = new Map();
  for (const row of computed) {
    const key = `${row.constant} ${row.theorem}`;
    seen.set(key, (seen.get(key) ?? 0) + 1);
    const recorded = expected.get(key);
    if (!recorded) {
      fail('STRENGTH-COUNT', `${row.theorem} was classified for ${row.constant}, which the record does not cite`);
      continue;
    }
    if (recorded.strength !== row.strength) {
      fail('STRENGTH-DRIFT', `${row.theorem} for ${row.constant}: record says ${recorded.strength}, ` +
        `the elaborated statement is ${row.strength}`);
    }
    if (recorded.axioms !== row.axioms) {
      fail('STRENGTH-DRIFT', `${row.theorem} for ${row.constant}: record says ${recorded.axioms} axioms, ` +
        `the transitive axiom set is ${row.axioms}`);
    }
  }
  for (const [key, count] of seen) {
    if (count !== 1) fail('STRENGTH-COUNT', `${key} was classified ${count} times`);
  }
  for (const key of expected.keys()) {
    if (!seen.has(key)) fail('STRENGTH-COUNT', `${key} is cited but no checker classified it`);
  }
  const counts = { citations: computed.length };
  for (const strength of STRENGTHS) counts[strength] = computed.filter(row => row.strength === strength).length;
  for (const axioms of AXIOM_CLASSES) counts[axioms] = computed.filter(row => row.axioms === axioms).length;
  counts.strict_universal = computed.filter(isStrictUniversal).length;
  return { ok: problems.length === 0, counts, problems, non_claim: NON_CLAIM };
}
