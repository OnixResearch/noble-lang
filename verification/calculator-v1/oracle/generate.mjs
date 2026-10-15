// Seeded, reproducible CALC-PARSE-01 input generator and bounded shrinker.
//
// The generator emits source text only; expectations come from the oracle
// (`calculator.mjs`). Candidates the oracle marks `ambiguous:*` or whose
// category is outside the spec are discarded, so every emitted vector has a
// spec-determined outcome. Magnitudes stay small (every intermediate
// numerator and denominator below 2^12, at most four binary operators) so the
// CLI's default guest budgets are not the thing being tested: the oracle has
// no budget model, and a `ResourceLimit` is reported as a divergence, never
// as agreement.
import { run } from './calculator.mjs';

// mulberry32: a 32-bit seeded PRNG; identical seeds give identical streams.
export const prng = seed => {
  let state = seed >>> 0;
  const next = () => {
    state = (state + 0x6d2b79f5) >>> 0;
    let t = state;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
  return {
    next,
    int: (low, high) => low + Math.floor(next() * (high - low + 1)),
    pick: items => items[Math.floor(next() * items.length)],
    chance: probability => next() < probability,
  };
};

// Expression trees: { t: 'num', text } | { t: 'neg', e } | { t: 'par', e }
// | { t: 'bin', op, l, r } | { t: 'name', text } | { t: 'call', name, args }.
// `render` inserts parentheses wherever CALC-PARSE-01 would otherwise group
// the text differently from the tree, so every subtree is a real operand.
const level = node => {
  if (node.t === 'bin') return { '+': 1, '-': 1, '*': 2, '/': 2, '^': 4 }[node.op];
  if (node.t === 'neg') return 3;
  return 5;
};
const wrap = (node, minimum) => (level(node) >= minimum ? render(node) : `(${render(node)})`);
export const render = node => {
  switch (node.t) {
  case 'num': return node.text;
  case 'name': return node.text;
  case 'neg': return `-${wrap(node.e, 3)}`;
  case 'par': return `(${render(node.e)})`;
  case 'call': return `${node.name}(${node.args.map(render).join(', ')})`;
  case 'bin': {
    if (node.op === '^') return `${wrap(node.l, 5)}^${wrap(node.r, 3)}`;
    const own = level(node);
    return `${wrap(node.l, own)} ${node.op} ${wrap(node.r, own + 1)}`;
  }
  default: throw new Error(node.t);
  }
};

const literal = random => {
  if (random.chance(0.2)) return { t: 'num', text: `${random.int(0, 9)}.${random.int(0, 99)}` };
  return { t: 'num', text: `${random.int(0, 12)}` };
};

// Exponents stay small integers, sometimes signed or rational-valued, so
// negative powers and NonIntegerExponent are both reachable.
const exponent = random => {
  const roll = random.next();
  if (roll < 0.6) return { t: 'num', text: `${random.int(0, 3)}` };
  if (roll < 0.85) return { t: 'neg', e: { t: 'num', text: `${random.int(1, 3)}` } };
  return { t: 'par', e: { t: 'bin', op: '/', l: { t: 'num', text: `${random.int(1, 4)}` }, r: { t: 'num', text: `${random.int(1, 4)}` } } };
};

const expression = (random, budget, names) => {
  if (budget <= 0 || random.chance(0.25)) {
    if (names.length > 0 && random.chance(0.3)) return { t: 'name', text: random.pick(names) };
    return literal(random);
  }
  const roll = random.next();
  if (roll < 0.1) return { t: 'neg', e: expression(random, budget - 1, names) };
  if (roll < 0.2) return { t: 'par', e: expression(random, budget - 1, names) };
  const op = random.pick(['+', '-', '*', '/', '^', '+', '-', '*', '/']);
  if (op === '^') {
    const base = random.chance(0.5) ? literal(random) : { t: 'par', e: expression(random, budget - 2, names) };
    const power = { t: 'bin', op, l: base, r: exponent(random) };
    return random.chance(0.3) ? { t: 'bin', op, l: { t: 'num', text: `${random.int(1, 3)}` }, r: power } : power;
  }
  const left = random.int(0, budget - 1);
  return { t: 'bin', op, l: expression(random, left, names), r: expression(random, budget - 1 - left, names) };
};

const operators = node => {
  switch (node.t) {
  case 'bin': return 1 + operators(node.l) + operators(node.r);
  case 'neg': case 'par': return operators(node.e);
  case 'call': return node.args.reduce((sum, arg) => sum + operators(arg), 0);
  default: return 0;
  }
};

// Upper bound on every intermediate magnitude, evaluated by the oracle's own
// exact numbers through a throwaway walk (no CLI budget model is implied).
const LIMIT = 1n << 12n;
const small = text => {
  const outcome = run([text]).outcomes[0];
  if (outcome.kind !== 'value') return true;
  const { n, d } = outcome.value;
  return (n < 0n ? -n : n) < LIMIT && d < LIMIT;
};
const subtrees = node => {
  switch (node.t) {
  case 'bin': return [node, ...subtrees(node.l), ...subtrees(node.r)];
  case 'neg': case 'par': return [node, ...subtrees(node.e)];
  default: return [node];
  }
};

// Spec-determined outcome only: no ambiguity note, no category the spec
// leaves open. Unjudged spans (numeric errors, end of input) are allowed;
// their categories are still judged.
export const determined = outcome => !outcome.notes.some(note => note.startsWith('ambiguous:'))
  && !(outcome.kind === 'diagnostic' && outcome.category.startsWith('Unspecified'));

// Syntax/name shapes whose spans the CALC-02 examples fix.
const malformed = (random, tree) => {
  const text = render(tree);
  switch (random.int(0, 3)) {
  case 0: return `${text} ${random.int(1, 9)}`; // TrailingInput at the extra token
  case 1: return `${text} + )`; // SyntaxError at ')'
  case 2: return `${text} * ${random.pick(['q', 'unknown', 'zz9'])}`; // UnknownName at the identifier
  default: return `${random.int(1, 9)}(${text})`; // TrailingInput at '('
  }
};

const num = text => ({ t: 'num', text: `${text}` });
const bin = (op, l, r) => ({ t: 'bin', op, l, r });
const neg = e => ({ t: 'neg', e });
const par = e => ({ t: 'par', e });

// Shape families, used round-robin so a modest sample still reaches powers
// (chains, signs, negative exponents), decimals, numeric errors and the
// CALC-02 diagnostic shapes, not only mixed arithmetic.
const FAMILIES = {
  mixed: (random, maxOperators) => expression(random, random.int(2, maxOperators), []),
  power: random => {
    const a = num(random.int(1, 3));
    // Half of the power samples are right-nested chains a^b^c.
    if (random.chance(0.5)) return bin('^', num(random.int(2, 3)), bin('^', num(random.int(2, 3)), num(random.int(0, 2))));
    switch (random.int(0, 3)) {
    case 0: return bin('^', a, bin('^', num(random.int(1, 3)), neg(num(random.int(1, 2)))));
    case 1: return neg(bin('^', num(random.int(2, 4)), num(random.int(1, 3))));
    case 2: return bin('^', par(bin('/', num(random.int(1, 5)), num(random.int(2, 5)))), neg(num(random.int(1, 3))));
    default: return bin('-', bin('^', a, neg(num(random.int(1, 3)))), num(random.int(1, 5)));
    }
  },
  decimal: random => bin(random.pick(['+', '-']), num(`${random.int(0, 9)}.${random.int(1, 9)}`),
    bin(random.pick(['+', '*', '/']), num(`0.${random.int(1, 9)}`), num(random.int(1, 9)))),
  'numeric-error': random => {
    const k = random.int(1, 9);
    switch (random.int(0, 2)) {
    case 0: return bin('/', num(random.int(1, 9)), par(bin('-', num(k), num(k))));
    case 1: return bin('+', num(random.int(1, 9)), bin('^', num(0), neg(num(random.int(1, 3)))));
    default: return bin('^', num(random.int(2, 9)), par(bin('/', num(1), num(random.int(2, 4)))));
    }
  },
  // One vector per sample beyond 64 bits (the canonical I64 boundary class).
  wide: random => bin('+', bin('^', num(2), num(random.int(63, 64))), num(random.int(1, 9))),
  division: random => bin('-', bin('/', num(random.int(1, 9)), num(random.int(2, 9))),
    bin('/', num(random.int(1, 9)), num(random.int(2, 9)))),
};
const FAMILY_ORDER = ['mixed', 'power', 'division', 'decimal', 'numeric-error', 'malformed', 'wide', 'power', 'mixed'];

export const generateExpressions = ({ seed, count, maxOperators = 4 }) => {
  const random = prng(seed);
  const out = [];
  const seen = new Set();
  let attempts = 0;
  while (out.length < count) {
    attempts += 1;
    if (attempts > count * 500) throw new Error('generator could not satisfy its filters');
    const family = FAMILY_ORDER[out.length % FAMILY_ORDER.length];
    const tree = (FAMILIES[family] ?? FAMILIES.mixed)(random, maxOperators);
    if (operators(tree) > maxOperators || operators(tree) === 0) continue;
    if (family !== 'wide' && !subtrees(tree).every(node => small(render(node)))) continue;
    const text = family === 'malformed' ? malformed(random, tree) : render(tree);
    if (seen.has(text)) continue;
    const outcome = run([text]).outcomes[0];
    if (!determined(outcome)) continue;
    seen.add(text);
    out.push({ text, family, outcome });
  }
  return out;
};

// Short lexical sessions: a variable, a function over it, a rebinding, a
// call, and optionally a failing `let` followed by a read of that name.
export const generateSessions = ({ seed, count }) => {
  const random = prng(seed ^ 0x5eed);
  const out = [];
  while (out.length < count) {
    const a = random.int(1, 9);
    const b = random.int(1, 9);
    const c = random.int(1, 9);
    const op = random.pick(['+', '-', '*']);
    const lines = [`let x = ${a}`, `fn f(y)=x ${op} y`, `let x = ${b}`, `f(${c})`];
    if (out.length % 2 === 0) lines.push(`let x = ${random.int(1, 9)} / 0`, 'x');
    const result = run(lines);
    if (!result.outcomes.every(determined)) continue;
    out.push({ lines, result });
  }
  return out;
};

// Bounded shrinking: one round proposes at most `limit` strictly smaller
// candidates (fewer operators, then shorter text), smallest first. The
// caller decides which still disagree; it never runs unbounded.
export const shrinkCandidates = (text, { limit = 6 } = {}) => {
  const candidates = new Set();
  const replaceAt = (node, target, replacement) => {
    if (node === target) return replacement;
    switch (node.t) {
    case 'bin': return { ...node, l: replaceAt(node.l, target, replacement), r: replaceAt(node.r, target, replacement) };
    case 'neg': case 'par': return { ...node, e: replaceAt(node.e, target, replacement) };
    default: return node;
    }
  };
  const tree = parseTree(text);
  if (tree === null) return [];
  for (const node of subtrees(tree)) {
    const children = node.t === 'bin' ? [node.l, node.r] : node.t === 'neg' || node.t === 'par' ? [node.e] : [];
    for (const child of children) candidates.add(render(replaceAt(tree, node, child)));
    if (node.t === 'num' && node.text !== '0' && node.text !== '1') {
      candidates.add(render(replaceAt(tree, node, { t: 'num', text: '1' })));
      if (/^\d+$/.test(node.text) && Number(node.text) > 2) {
        candidates.add(render(replaceAt(tree, node, { t: 'num', text: `${Math.floor(Number(node.text) / 2)}` })));
      }
    }
  }
  candidates.delete(text);
  return [...candidates]
    .filter(candidate => determined(run([candidate]).outcomes[0]))
    .sort((x, y) => x.length - y.length || (x < y ? -1 : 1))
    .slice(0, limit);
};

// Re-parses generator output (well-formed, ASCII, no calls) into the
// generator's tree shape for shrinking.
export const parseTree = text => {
  const tokens = text.match(/\d+(?:\.\d+)?|[A-Za-z_]\w*|[-+*/^()]/g) ?? [];
  let i = 0;
  const peek = () => tokens[i];
  const sum = () => {
    let left = product();
    while (peek() === '+' || peek() === '-') { const op = tokens[i++]; left = { t: 'bin', op, l: left, r: product() }; }
    return left;
  };
  const product = () => {
    let left = unary();
    while (peek() === '*' || peek() === '/') { const op = tokens[i++]; left = { t: 'bin', op, l: left, r: unary() }; }
    return left;
  };
  const unary = () => {
    if (peek() === '-') { i += 1; return { t: 'neg', e: unary() }; }
    return power();
  };
  const power = () => {
    const base = primary();
    if (peek() !== '^') return base;
    i += 1;
    return { t: 'bin', op: '^', l: base, r: unary() };
  };
  const primary = () => {
    const token = tokens[i++];
    if (token === '(') { const e = sum(); if (tokens[i++] !== ')') throw new Error('shape'); return { t: 'par', e }; }
    if (/^\d/.test(token ?? '')) return { t: 'num', text: token };
    if (/^[A-Za-z_]/.test(token ?? '')) return { t: 'name', text: token };
    throw new Error('shape');
  };
  try {
    const tree = sum();
    return i === tokens.length ? tree : null;
  } catch {
    return null;
  }
};
