// Finite Equivalence Modulo Inputs (EMI) cases for the foundation comparator.
// Inputs and reference stacks are top-first; source prefixes and observed
// stacks are bottom-first.  The generated programs contain only scalar I64
// arithmetic and one `if` or I64 sum `case`.
import { MAX, MIN, inRange, refRun, opSource, leanInt, leanOps, leanList, prng, wrap } from './arithmetic.mjs';

const ARITH = new Set(['add', 'sub', 'mul']);
const TRANSFORMATIONS = ['replace-constant', 'replace-operator', 'insert-neutral', 'prune-neutral'];
const MAX_POSITIVE_CASES = 64;
const ZERO = Object.freeze({ push: 0n });
const NEUTRAL = Object.freeze([ZERO, 'add']);
const REFUSED_RAW = Object.freeze({
  'wrong-branch-type': 'true',
  'unsupported-word': 'swap',
});

// Boundary values are selected often, with ordinary deterministic noise in
// the remaining slots.  Every value remains an accepted signed-I64 literal.
const BOUNDARY = [
  0n, 1n, -1n, 2n, -2n, 3n, -3n, MAX, MIN, MAX - 1n, MIN + 1n,
  2n ** 31n, -(2n ** 31n), 2n ** 32n, 2n ** 32n - 1n, 2n ** 62n,
  -(2n ** 62n), 3037000499n, 3037000500n, -3037000500n,
];
const SMALL = [3n, 5n, 7n, 11n, 13n, 17n, 19n, 23n, 29n, 31n];

const isRecord = value => value !== null && typeof value === 'object' && !Array.isArray(value);
const isPush = op => isRecord(op) && Object.keys(op).length === 1 && Object.hasOwn(op, 'push') && typeof op.push === 'bigint';
const isRaw = op => isRecord(op) && Object.keys(op).length === 1 && Object.hasOwn(op, 'raw') && typeof op.raw === 'string';
const isNeutralAt = (ops, site) => isPush(ops?.[site]) && ops[site].push === 0n && ops[site + 1] === 'add';

function sameOp(a, b) {
  if (a === b) return true;
  return isPush(a) && isPush(b) && a.push === b.push;
}

function sameOps(a, b) {
  return Array.isArray(a) && Array.isArray(b) && a.length === b.length && a.every((op, i) => sameOp(op, b[i]));
}

function cloneOps(ops) {
  return ops.map(op => isRecord(op) ? { ...op } : op);
}

function cloneCase(c) {
  return {
    ...c,
    prefix: Array.isArray(c.prefix) ? c.prefix.map(item => Array.isArray(item) ? [...item] : item) : c.prefix,
    input: Array.isArray(c.input) ? [...c.input] : c.input,
    original: Array.isArray(c.original) ? c.original.map(ops => Array.isArray(ops) ? cloneOps(ops) : ops) : c.original,
    variant: Array.isArray(c.variant) ? c.variant.map(ops => Array.isArray(ops) ? cloneOps(ops) : ops) : c.variant,
    transformation: isRecord(c.transformation) ? { ...c.transformation } : c.transformation,
    probe: isRecord(c.probe) ? { ...c.probe } : c.probe,
  };
}

function failure(c, reason) {
  if (c?.probe?.disposition === 'intentionally-ineligible' && c.probe.reason !== reason) {
    reason = 'refusal-tag-mismatch';
  }
  return { eligible: false, reason, expected: null, reference: null, input_types: [], output_types: [] };
}

function validPrefix(prefix) {
  if (!Array.isArray(prefix)) return false;
  return prefix.every(item => {
    if (!Array.isArray(item) || item.length !== 2) return false;
    const [type, value] = item;
    if (type === 'Bool') return value === 'true' || value === 'false';
    if (type !== 'I64' || typeof value !== 'string' || !/^-?(?:0|[1-9][0-9]*)$/.test(value)) return false;
    try { return inRange(BigInt(value)); } catch { return false; }
  });
}

function rawReason(c, op) {
  if (!isRaw(op)) return null;
  if (c?.probe?.disposition !== 'intentionally-ineligible') return 'raw-node-not-designated';
  if (c.probe.reason === 'wrong-branch-type' && op.raw === REFUSED_RAW['wrong-branch-type']) return 'wrong-branch-type';
  if (c.probe.reason === 'unsupported-word' && op.raw === REFUSED_RAW['unsupported-word']) return 'unsupported-word';
  return 'raw-node-not-designated';
}

function depthAfter(ops, initialDepth, c) {
  if (!Array.isArray(ops)) return { reason: 'malformed-branch', depth: null };
  let depth = initialDepth;
  for (const op of ops) {
    const raw = rawReason(c, op);
    if (raw) return { reason: raw, depth: null };
    if (isPush(op)) {
      if (!inRange(op.push)) return { reason: 'out-of-range-literal', depth: null };
      depth += 1;
      continue;
    }
    if (typeof op !== 'string' || !ARITH.has(op)) return { reason: 'unsupported-word', depth: null };
    if (depth < 2) return { reason: 'stack-underflow', depth: null };
    depth -= 1;
  }
  return { reason: null, depth };
}

function removePair(ops, site) {
  return [...ops.slice(0, site), ...ops.slice(site + 2)];
}

function insertNeutral(ops, site) {
  return [...ops.slice(0, site), ...cloneOps(NEUTRAL), ...ops.slice(site)];
}

function hasOnlyDeclaredDifference(c) {
  const branch = c.transformation.branch;
  if (branch !== 1 - c.selected) return false;
  if (!Number.isInteger(c.transformation.site) || c.transformation.site < 0) return false;
  for (let i = 0; i < 2; i++) {
    if (i !== branch && !sameOps(c.original[i], c.variant[i])) return false;
  }
  const original = c.original[branch];
  const variant = c.variant[branch];
  if (sameOps(original, variant)) return false;
  const { kind, site } = c.transformation;
  if (kind === 'replace-constant') {
    return site < original.length && site < variant.length && isPush(original[site]) && isPush(variant[site]) &&
      original[site].push !== variant[site].push &&
      sameOps(original.slice(0, site), variant.slice(0, site)) &&
      sameOps(original.slice(site + 1), variant.slice(site + 1));
  }
  if (kind === 'replace-operator') {
    return site < original.length && site < variant.length && typeof original[site] === 'string' &&
      ARITH.has(original[site]) && typeof variant[site] === 'string' && ARITH.has(variant[site]) &&
      original[site] !== variant[site] && sameOps(original.slice(0, site), variant.slice(0, site)) &&
      sameOps(original.slice(site + 1), variant.slice(site + 1));
  }
  if (kind === 'insert-neutral') {
    return isNeutralAt(variant, site) && sameOps(original, removePair(variant, site));
  }
  if (kind === 'prune-neutral') {
    return isNeutralAt(original, site) && sameOps(removePair(original, site), variant);
  }
  return false;
}

/**
 * Admit one pair to the narrow EMI model.  `reference` is the selected body
 * result (top-first); `expected.stack` is the complete driver result
 * (bottom-first), including the literal prefix.  The driver's input interface
 * is empty because `input` and the selector are literals in `source()`.
 */
export function assess(c) {
  if (!isRecord(c) || (c.kind !== 'if' && c.kind !== 'case') ||
      (c.selected !== 0 && c.selected !== 1) || !Array.isArray(c.input) ||
      (c.kind === 'case' && c.input.length === 0) || !c.input.every(value => typeof value === 'bigint' && inRange(value)) ||
      !validPrefix(c.prefix) || !Array.isArray(c.original) || c.original.length !== 2 ||
      !Array.isArray(c.variant) || c.variant.length !== 2 || !isRecord(c.transformation)) {
    return failure(c, 'malformed-pair');
  }
  if (c.probe !== undefined && (c.probe?.disposition !== 'intentionally-ineligible' || typeof c.probe.reason !== 'string')) {
    return failure(c, 'invalid-probe-tag');
  }

  const depths = [];
  for (const subject of [c.original, c.variant]) {
    const subjectDepths = [];
    for (const body of subject) {
      const result = depthAfter(body, c.input.length, c);
      if (result.reason) return failure(c, result.reason);
      subjectDepths.push(result.depth);
    }
    depths.push(subjectDepths);
  }
  if (depths.some(subjectDepths => subjectDepths[0] !== subjectDepths[1]) ||
      depths[0][0] !== depths[1][0]) return failure(c, 'branch-interface-mismatch');
  const outputDepth = depths[0][0];
  if (outputDepth < 2) return failure(c, 'fewer-than-two-outputs');

  // Structural identity is the defining EMI condition.  Do this before
  // validating the mutation descriptor so selected-branch controls fail for
  // the precise reason they are designed to exercise.
  if (!sameOps(c.original[c.selected], c.variant[c.selected])) return failure(c, 'selected-branch-changed');
  if (!hasOnlyDeclaredDifference(c)) return failure(c, 'mutation-not-confined-to-untaken-branch');
  if (c.probe !== undefined) return failure(c, 'tagged-refusal-is-eligible');

  const selected = c.original[c.selected];
  const reference = refRun(selected, c.input);
  if (reference === null) return failure(c, 'reference-refused');
  const expected = {
    outcome: 'normal',
    stack: [
      ...c.prefix.map(([type, value]) => [type, value]),
      ...[...reference].reverse().map(value => ['I64', String(value)]),
    ],
    trace: [],
  };
  return {
    eligible: true,
    reason: null,
    expected,
    reference,
    input_types: [],
    output_types: [...c.prefix.map(([type]) => type), ...Array(outputDepth).fill('I64')],
  };
}

function rawSourceAllowed(c, op) {
  const reason = rawReason(c, op);
  if (!reason || reason === 'raw-node-not-designated') throw new TypeError('raw source nodes are reserved for tagged ineligible probes');
  return op.raw;
}

function renderOp(op, c) {
  if (isRaw(op)) return rawSourceAllowed(c, op);
  const rendered = opSource(op);
  if (rendered === undefined) throw new TypeError('cannot render an unsupported operation without a tagged raw probe');
  return rendered;
}

function renderOps(ops, c) {
  if (!Array.isArray(ops)) throw new TypeError('branch body must be an array');
  return `[ ${ops.map(op => renderOp(op, c)).join(' ')} ]`;
}

/** Render the full fixed-input program for one exact subject (`original` or `variant`). */
export function source(c, member = 'original') {
  if (member !== 'original' && member !== 'variant') throw new TypeError('member must be original or variant');
  if (!isRecord(c) || !Array.isArray(c.prefix) || !Array.isArray(c.input) || !Array.isArray(c[member]) || c[member].length !== 2) {
    throw new TypeError('cannot render a malformed EMI pair');
  }
  const words = [];
  for (const [type, value] of c.prefix) {
    if (type !== 'I64' && type !== 'Bool') throw new TypeError('cannot render an invalid prefix type');
    words.push(String(value));
  }
  for (const value of [...c.input].reverse()) words.push(String(value));
  if (c.kind === 'if') words.push(c.selected === 0 ? 'true' : 'false');
  else if (c.kind === 'case') words.push(c.selected === 0 ? 'inl' : 'inr');
  else throw new TypeError('cannot render an unsupported branch kind');
  words.push(renderOps(c[member][0], c), renderOps(c[member][1], c), c.kind);
  return `${words.join(' ')}\n`;
}

function leanKind(kind) { return kind === 'if' ? '.ite' : '.sum'; }

function checkedIdentifier(identifier) {
  if (typeof identifier !== 'string' || !/^[a-z][A-Za-z0-9_']*$/.test(identifier)) {
    throw new TypeError('identifier must be a fresh Lean identifier beginning with a lowercase letter');
  }
  return identifier;
}

function leanSubjectDefinitions(identifier, member, c) {
  const subject = c[member];
  const name = `${identifier}_${member}`;
  return [
    `def ${name}_p : List NobleFoundation.AOp := ${leanOps(subject[0])}`,
    `def ${name}_q : List NobleFoundation.AOp := ${leanOps(subject[1])}`,
    `def ${name}_code : List NobleContracts.Op := NobleFoundation.branchCode ${identifier}_kind ${identifier}_left ${name}_p ${name}_q`,
  ];
}

function leanPrefix(prefix) {
  return `[${prefix.map(([type, value]) => type === 'I64'
    ? `(NobleFoundation.i64v ${leanInt(BigInt(value))})`
    : `.bool ${value}`).join(', ')}]`;
}

function leanSubjectEvidence(identifier, member, c, result, outputDepth) {
  const name = `${identifier}_${member}`;
  const defs = [
    `theorem ${name}_ref : NobleFoundation.branchRef ${identifier}_kind ${identifier}_left ${name}_p ${name}_q ${identifier}_input = some ${leanList(result)} := by decide`,
  ];
  for (let branch = 0; branch < 2; branch++) {
    defs.push(`theorem ${name}_depth_${branch} : NobleFoundation.depthRun ${name}_${branch === 0 ? 'p' : 'q'} ${identifier}_input.length = some ${outputDepth} := by decide`);
  }
  const visits = c.selected === 0 ? '[1, 0]' : '[0, 1]';
  defs.push(`theorem ${name}_visits : NobleFoundation.branchVisits ${identifier}_left = ${visits} := by decide`);
  defs.push(
    `theorem ${name}_exec (t : NobleContracts.Stack) : NobleContracts.Exec ${name}_code (${identifier}_prefix ++ NobleFoundation.enc ${identifier}_input) t ↔ t = ${identifier}_prefix ++ NobleFoundation.enc ${leanList(result)} := by`,
    `  exact NobleFoundation.exec_branchRef ${name}_ref ${identifier}_prefix t`,
  );
  return defs;
}

/** Emit exact-subject Lean declarations; the caller imports Branch.lean once. */
export function leanEvidence(c, identifier) {
  identifier = checkedIdentifier(identifier);
  const assessment = assess(c);
  if (!assessment.eligible) throw new TypeError(`Lean evidence requires an eligible pair (${assessment.reason})`);
  const outputDepth = assessment.output_types.length - c.prefix.length;
  const left = c.selected === 0;
  const declarations = [
    `def ${identifier}_kind : NobleFoundation.BranchKind := ${leanKind(c.kind)}`,
    `def ${identifier}_left : Bool := ${left ? 'true' : 'false'}`,
    `def ${identifier}_prefix : NobleContracts.Stack := ${leanPrefix(c.prefix)}`,
    `def ${identifier}_input : List ℤ := ${leanList(c.input)}`,
    ...leanSubjectDefinitions(identifier, 'original', c),
    ...leanSubjectDefinitions(identifier, 'variant', c),
    `theorem ${identifier}_selectedOps_equal : NobleFoundation.selectedOps ${identifier}_left ${identifier}_original_p ${identifier}_original_q = NobleFoundation.selectedOps ${identifier}_left ${identifier}_variant_p ${identifier}_variant_q := by rfl`,
    ...leanSubjectEvidence(identifier, 'original', c, assessment.reference, outputDepth),
    ...leanSubjectEvidence(identifier, 'variant', c, assessment.reference, outputDepth),
  ];
  return `${declarations.join('\n')}\n`;
}

function boundaryChoice(next) {
  const boundary = BOUNDARY[Number(next() % BigInt(BOUNDARY.length))];
  return Number(next() % 4n) === 0 ? wrap(BigInt.asIntN(64, next())) : boundary;
}

function baseBody(constant, operator) {
  return [
    ...cloneOps(NEUTRAL), { push: constant }, operator, ...cloneOps(NEUTRAL),
  ];
}

function applyPrefix(index) {
  const value = BOUNDARY[(index * 7 + 3) % BOUNDARY.length];
  switch (index % 5) {
    case 0: return [['I64', '19'], ['Bool', 'true']];
    case 1: return [['I64', '17'], ['Bool', 'true']];
    case 2: return [['Bool', 'false'], ['I64', '-23'], ['Bool', 'true']];
    case 3: return [['I64', String(value)], ['Bool', 'false'], ['I64', '0']];
    default: return [['Bool', 'true'], ['I64', String(value)]];
  }
}

function bodyTop(x, body) {
  const result = refRun(body, [x, 0n]);
  if (result === null) throw new Error('generator produced a refusing body');
  return result[0];
}

function distinctLowerInputs(next, x, firstValues, count) {
  const candidates = [
    ...BOUNDARY,
    wrap(x + 41n), wrap(x - 43n), wrap(x + 101n), wrap(x - 103n),
    101n, -101n, 1009n, -1009n,
  ];
  const used = new Set(firstValues.map(String));
  const result = [];
  const start = Number(next() % BigInt(candidates.length));
  for (let offset = 0; result.length < count && offset < candidates.length * 2; offset++) {
    const candidate = candidates[(start + offset) % candidates.length];
    if (!used.has(String(candidate))) {
      result.push(candidate);
      used.add(String(candidate));
    }
  }
  if (result.length !== count) throw new Error('generator could not choose distinct ordered outputs');
  return result;
}

function makePositive(next, index) {
  const kind = index % 4 === 3 ? 'case' : 'if';
  const selected = kind === 'case' ? Math.floor(index / 4) % 2 : Math.floor(index / 2) % 2;
  const transform = TRANSFORMATIONS[Math.floor(index / 4) % TRANSFORMATIONS.length];
  const x = boundaryChoice(next);
  const selectedConstant = index === 0 ? 17n : SMALL[Number(next() % BigInt(SMALL.length))];
  let untakenConstant = SMALL[Number(next() % BigInt(SMALL.length))];
  if (index === 0) untakenConstant = 3n;
  if (untakenConstant === selectedConstant) untakenConstant = SMALL[(SMALL.indexOf(untakenConstant) + 1) % SMALL.length];
  const selectedOperator = index === 0 ? 'add' : ['add', 'sub', 'mul'][Number(next() % 3n)];
  let untakenOperator = transform === 'replace-constant' || transform === 'replace-operator'
    ? 'add'
    : ['add', 'sub', 'mul'][Number(next() % 3n)];
  if (transform === 'replace-constant') untakenOperator = 'add';
  if (transform === 'replace-operator') untakenOperator = 'add';

  const selectedBody = baseBody(selectedConstant, selectedOperator);
  const untakenBody = baseBody(untakenConstant, untakenOperator);
  const original = selected === 0 ? [selectedBody, untakenBody] : [untakenBody, selectedBody];
  const variant = original.map(cloneOps);
  const branch = 1 - selected;
  let site;

  if (transform === 'replace-constant') {
    site = 2;
    const variantConstant = SMALL[(SMALL.indexOf(untakenConstant) + 3) % SMALL.length];
    variant[branch][site] = { push: variantConstant };
  } else if (transform === 'replace-operator') {
    site = 3;
    variant[branch][site] = 'sub';
  } else {
    site = (index % 6) + 1;
    if (transform === 'insert-neutral') variant[branch] = insertNeutral(original[branch], site);
    else original[branch] = insertNeutral(original[branch], site);
  }

  // Choose the remaining fixed inputs only after both subjects are known, so
  // each subject has at least two distinct observable I64 outputs in order.
  const allBodies = [original[0], original[1], variant[0], variant[1]];
  const firstValues = allBodies.map(body => bodyTop(x, body));
  let input = [x, ...distinctLowerInputs(next, x, firstValues, index % 3 === 0 ? 2 : 1)];
  // The first lower value is the second stack output; an optional third is
  // also distinct so order changes remain visible across the whole stack.
  if (input.length === 2 && input[1] === x) input[1] = wrap(input[1] + 1n);

  const c = {
    index,
    kind,
    selected,
    input,
    prefix: applyPrefix(index),
    original,
    variant,
    transformation: { kind: transform, branch, site },
  };
  if (!assess(c).eligible) throw new Error(`generator produced an ineligible positive at ${index}: ${assess(c).reason}`);
  return c;
}

function taggedProbe(base, index, reason) {
  const c = cloneCase(base);
  c.index = index;
  c.probe = { disposition: 'intentionally-ineligible', reason };
  return c;
}

function refusalCases(positives, firstIndex) {
  // The first scheduled pair is an `if`, selects its unchanged left branch,
  // and differs by a constant in the right branch.
  const outOfRange = taggedProbe(positives[0], firstIndex, 'out-of-range-literal');
  const branch = outOfRange.transformation.branch;
  outOfRange.variant[branch][outOfRange.transformation.site] = { push: MAX + 1n };

  // `true` is a real source literal but gives the untaken body a Bool result.
  const wrongType = taggedProbe(positives[2], firstIndex + 1, 'wrong-branch-type');
  wrongType.original = wrongType.original.map(cloneOps);
  wrongType.variant = wrongType.original.map(cloneOps);
  const wrongBranch = 1 - wrongType.selected;
  wrongType.variant[wrongBranch].push({ raw: REFUSED_RAW['wrong-branch-type'] });
  wrongType.transformation = { kind: 'wrong-branch-type', branch: wrongBranch, site: wrongType.variant[wrongBranch].length - 1 };

  // `swap` is accepted scalar source syntax but outside the AOp whitelist.
  const unsupported = taggedProbe(positives[3], firstIndex + 2, 'unsupported-word');
  unsupported.original = unsupported.original.map(cloneOps);
  unsupported.variant = unsupported.original.map(cloneOps);
  const unsupportedBranch = 1 - unsupported.selected;
  unsupported.variant[unsupportedBranch].push({ raw: REFUSED_RAW['unsupported-word'] });
  unsupported.transformation = { kind: 'unsupported-word', branch: unsupportedBranch, site: unsupported.variant[unsupportedBranch].length - 1 };

  return [outOfRange, wrongType, unsupported];
}

/**
 * Generate exactly `count` positives, including the complete first sixteen
 * coverage cases. Three tagged refusal probes follow the positives.
 */
export function generateEMI(seed, count) {
  if (!Number.isSafeInteger(count) || count < 16 || count > MAX_POSITIVE_CASES) {
    throw new RangeError(`count must be an integer from 16 through ${MAX_POSITIVE_CASES}`);
  }
  const next = prng(seed);
  const positives = Array.from({ length: count }, (_, index) => makePositive(next, index));
  return [...positives, ...refusalCases(positives, count)];
}

/** Return a separate pair whose selected bodies differ by a neutral suffix. */
export function selectedMutation(c) {
  const assessment = assess(c);
  if (!assessment.eligible) throw new TypeError(`selectedMutation requires an eligible pair (${assessment.reason})`);
  const mutated = cloneCase(c);
  mutated.variant = mutated.original.map(cloneOps);
  const selected = c.selected;
  const site = mutated.variant[selected].length;
  mutated.variant[selected].push({ push: 0n }, 'add');
  mutated.transformation = { kind: 'selected-neutral-mutation', branch: selected, site };
  delete mutated.probe;
  return mutated;
}
