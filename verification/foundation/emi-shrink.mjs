// Shrinking for the finite scalar EMI lane.  Every yielded row remains a
// normal, admitted pair with the same selected branch and exactly one
// declared untaken-branch mutation; the parent reruns admission and execution.
import { assess } from './emi-model.mjs';
import { refRun } from './arithmetic.mjs';

const isRecord = value => value !== null && typeof value === 'object' && !Array.isArray(value);
const isPush = op => isRecord(op) && Object.keys(op).length === 1 && Object.hasOwn(op, 'push') && typeof op.push === 'bigint';
const isZeroAdd = (ops, i) => isPush(ops[i]) && ops[i].push === 0n && ops[i + 1] === 'add';
const removePair = (ops, site) => [...ops.slice(0, site), ...ops.slice(site + 2)];
const insertPair = (ops, site) => [...ops.slice(0, site), { push: 0n }, 'add', ...ops.slice(site)];

function cloneOps(ops) { return ops.map(op => isRecord(op) ? { ...op } : op); }

function cloneCase(c) {
  return {
    ...c,
    prefix: c.prefix.map(item => [...item]),
    input: [...c.input],
    original: c.original.map(cloneOps),
    variant: c.variant.map(cloneOps),
    transformation: { ...c.transformation },
  };
}

function valueBits(value) {
  if (typeof value !== 'bigint') return 0;
  const magnitude = value < 0n ? -value : value;
  return magnitude === 0n ? 0 : magnitude.toString(2).length;
}

function opCost(op) {
  if (isPush(op)) return 4 + valueBits(op.push);
  if (typeof op === 'string') return 2;
  return 64;
}

/** A deterministic ordering over pair size, literal magnitude and mutation site. */
export function complexity(c) {
  if (!isRecord(c) || !Array.isArray(c.input) || !Array.isArray(c.prefix) ||
      !Array.isArray(c.original) || !Array.isArray(c.variant) || !isRecord(c.transformation)) {
    return Number.MAX_SAFE_INTEGER;
  }
  let score = 0;
  for (const value of c.input) score += 24 + valueBits(value);
  for (const item of c.prefix) {
    score += 24;
    if (Array.isArray(item) && item[0] === 'I64') {
      try { score += valueBits(BigInt(item[1])); } catch { score += 64; }
    } else if (Array.isArray(item) && item[0] === 'Bool' && item[1] === 'true') score += 1;
  }
  for (const subject of [c.original, c.variant]) {
    for (const body of subject) {
      if (!Array.isArray(body)) return Number.MAX_SAFE_INTEGER;
      score += 8 * body.length;
      for (const op of body) score += opCost(op);
    }
  }
  if (Number.isInteger(c.transformation.site) && c.transformation.site > 0) score += c.transformation.site;
  return score;
}

function* simplerValues(value) {
  const candidates = [0n, 1n, -1n, value / 2n];
  const seen = new Set();
  for (const candidate of candidates) {
    if (candidate !== value && valueBits(candidate) < valueBits(value) && !seen.has(candidate)) {
      seen.add(candidate);
      yield candidate;
    }
  }
}

function pairMaps(c) {
  const selected = c.selected;
  const untaken = 1 - selected;
  const transform = c.transformation;
  const maps = [];
  for (let i = 0; i < c.original[selected].length; i++) maps.push([selected, i, i]);

  const original = c.original[untaken];
  const variant = c.variant[untaken];
  if (transform.kind === 'insert-neutral') {
    for (let i = 0; i < original.length; i++) {
      const j = i < transform.site ? i : i + 2;
      if (j !== transform.site && j !== transform.site + 1) maps.push([untaken, i, j]);
    }
  } else if (transform.kind === 'prune-neutral') {
    for (let j = 0; j < variant.length; j++) {
      const i = j < transform.site ? j : j + 2;
      if (i !== transform.site && i !== transform.site + 1) maps.push([untaken, i, j]);
    }
  } else {
    for (let i = 0; i < original.length; i++) {
      if ((transform.kind === 'replace-constant' || transform.kind === 'replace-operator') && i === transform.site) continue;
      maps.push([untaken, i, i]);
    }
  }
  return maps;
}

function* commonNeutralPairs(c) {
  for (const subjectBranch of [c.selected]) {
    const body = c.original[subjectBranch];
    for (let i = 0; i + 1 < body.length; i++) {
      if (isZeroAdd(body, i)) yield { branch: subjectBranch, originalSite: i, variantSite: i };
    }
  }

  const branch = 1 - c.selected;
  const { kind, site } = c.transformation;
  if (kind === 'insert-neutral') {
    const base = c.original[branch];
    for (let i = 0; i + 1 < base.length; i++) {
      if (!isZeroAdd(base, i)) continue;
      const j = i < site ? i : i + 2;
      if (j === site || j === site + 1 || !isZeroAdd(c.variant[branch], j)) continue;
      yield { branch, originalSite: i, variantSite: j };
    }
  } else if (kind === 'prune-neutral') {
    const base = c.variant[branch];
    for (let j = 0; j + 1 < base.length; j++) {
      if (!isZeroAdd(base, j)) continue;
      const i = j < site ? j : j + 2;
      if (i === site || i === site + 1 || !isZeroAdd(c.original[branch], i)) continue;
      yield { branch, originalSite: i, variantSite: j };
    }
  } else {
    const body = c.original[branch];
    for (let i = 0; i + 1 < body.length; i++) {
      if (!isZeroAdd(body, i)) continue;
      yield { branch, originalSite: i, variantSite: i };
    }
  }
}

function orderedOutputs(c) {
  for (const subject of [c.original, c.variant]) {
    for (const body of subject) {
      const result = refRun(body, c.input);
      if (result === null || result.length < 2 || result[0] === result[1]) return false;
    }
  }
  return true;
}

function candidateKey(c) {
  return JSON.stringify(c, (_key, value) => typeof value === 'bigint' ? { $bigint: String(value) } : value);
}

/** Yield only strictly simpler, still-eligible pairs with observable output order. */
export function* shrinkCandidates(c) {
  const baseAssessment = assess(c);
  if (!baseAssessment.eligible) return;
  const baseComplexity = complexity(c);
  const baseInterface = JSON.stringify([baseAssessment.input_types, baseAssessment.output_types]);
  const seen = new Set();

  function* offer(candidate) {
    if (complexity(candidate) >= baseComplexity) return;
    const key = candidateKey(candidate);
    if (seen.has(key)) return;
    seen.add(key);
    const assessment = assess(candidate);
    if (!assessment.eligible || !orderedOutputs(candidate) ||
        JSON.stringify([assessment.input_types, assessment.output_types]) !== baseInterface) return;
    yield candidate;
  }

  // Prefix values may simplify, but removing a slot would change the complete
  // declared output interface of the fixed-input driver.
  for (let i = 0; i < c.prefix.length; i++) {
    const [type, value] = c.prefix[i];
    if (type === 'I64') {
      let number;
      try { number = BigInt(value); } catch { continue; }
      for (const simpler of simplerValues(number)) {
        const candidate = cloneCase(c);
        candidate.prefix[i][1] = String(simpler);
        yield* offer(candidate);
      }
    } else if (type === 'Bool' && value === 'true') {
      const candidate = cloneCase(c);
      candidate.prefix[i][1] = 'false';
      yield* offer(candidate);
    }
  }

  // Fixed-input values shrink independently; the case selector and source
  // stack interface remain unchanged.
  for (let i = 0; i < c.input.length; i++) {
    for (const simpler of simplerValues(c.input[i])) {
      const candidate = cloneCase(c);
      candidate.input[i] = simpler;
      yield* offer(candidate);
    }
  }

  // Shrink shared constants in both exact subjects together.  The constant
  // replacement site is handled as a pair so the mutation stays nonzero.
  for (const [branch, originalIndex, variantIndex] of pairMaps(c)) {
    const old = c.original[branch][originalIndex];
    const variant = c.variant[branch][variantIndex];
    if (!isPush(old) || !isPush(variant) || old.push !== variant.push) continue;
    for (const simpler of simplerValues(old.push)) {
      const candidate = cloneCase(c);
      candidate.original[branch][originalIndex] = { push: simpler };
      candidate.variant[branch][variantIndex] = { push: simpler };
      yield* offer(candidate);
    }
  }
  if (c.transformation.kind === 'replace-constant') {
    const branch = c.transformation.branch;
    const site = c.transformation.site;
    const old = c.original[branch][site].push;
    const variant = c.variant[branch][site].push;
    const oldOptions = [old, ...simplerValues(old)];
    const variantOptions = [variant, ...simplerValues(variant)];
    for (const oldValue of oldOptions) for (const variantValue of variantOptions) {
      if (oldValue === variantValue || (oldValue === old && variantValue === variant)) continue;
      const candidate = cloneCase(c);
      candidate.original[branch][site] = { push: oldValue };
      candidate.variant[branch][site] = { push: variantValue };
      yield* offer(candidate);
    }
  }

  // Remove common neutral arithmetic from each branch in both subjects.  If
  // it precedes the mutation, update its site so the location shrinks too.
  for (const neutral of commonNeutralPairs(c)) {
    const candidate = cloneCase(c);
    const { branch, originalSite, variantSite } = neutral;
    candidate.original[branch] = removePair(candidate.original[branch], originalSite);
    candidate.variant[branch] = removePair(candidate.variant[branch], variantSite);
    if (branch === c.transformation.branch) {
      const kind = c.transformation.kind;
      const mutationSite = c.transformation.site;
      const beforeOriginal = originalSite + 2 <= mutationSite;
      const beforeVariant = variantSite + 2 <= mutationSite;
      if ((kind === 'insert-neutral' && beforeVariant) || (kind === 'prune-neutral' && beforeOriginal) ||
          ((kind === 'replace-constant' || kind === 'replace-operator') && beforeOriginal)) {
        candidate.transformation.site -= 2;
      }
    }
    yield* offer(candidate);
  }

  // Move neutral insertion/removal sites toward the start of the body.  This
  // is an independent shrink dimension from reducing integer magnitudes.
  const { kind, branch, site } = c.transformation;
  if (kind === 'insert-neutral') {
    for (let nextSite = site - 1; nextSite >= 0; nextSite -= 1) {
      const candidate = cloneCase(c);
      candidate.variant[branch] = insertPair(candidate.original[branch], nextSite);
      candidate.transformation.site = nextSite;
      yield* offer(candidate);
    }
  } else if (kind === 'prune-neutral') {
    for (let nextSite = site - 1; nextSite >= 0; nextSite -= 1) {
      const candidate = cloneCase(c);
      candidate.original[branch] = insertPair(candidate.variant[branch], nextSite);
      candidate.transformation.site = nextSite;
      yield* offer(candidate);
    }
  }
}
