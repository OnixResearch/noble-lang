#!/usr/bin/env node
/**
 * Bounded typed program synthesis with counterexample-guided refinement (issue #116).
 *
 * The declared benchmarks and grammar are `benchmarks-v1.txt`; this lane reads them with a strict
 * grammar, synthesises each task's program, refines it against counterexamples, re-admits every
 * emitted program through the trusted command line, and emits the kernel certificates that
 * `proofs/foundation/NobleFoundation/SynthesisCertified.lean` carries.
 *
 * What each status means, and nothing more:
 *   - proved: the winner is affine over the declared inputs, its affine form equals the reference
 *     graph's, and this lane emitted a `<task>_certified` theorem for the pair. That theorem is
 *     proved only once the foundation lane (`verification/foundation/check.mjs`) accepts it; this
 *     lane does not check the kernel and records that limit.
 *   - unproven: both solvers answered UNSAT on "some listed output differs" and no certificate
 *     applies. Solver UNSAT never certifies (`verification/symbolic/README.md`).
 *   - inconclusive: UNKNOWN, a solver error, a timeout, a model that does not replay, disagreement
 *     between the solvers, or the round budget.
 *   - exhausted: no program within the declared size bound (or the candidate fuel) satisfies the
 *     current examples; the reason distinguishes the size bound from the fuel.
 *   - unsatisfiable: the examples admit no function (one input with two output values).
 *   - run-only: the family has no solver-checkable reference (the list family); the program is
 *     executed, and only executed.
 * Held-out examples are never given to the search: they run on the final program only, and a
 * failure is reported as a failure.
 *
 * Discovery is a deterministic bottom-up breadth-first enumeration by size over the declared
 * words, deduplicated by observational equivalence on the current examples: a state (the whole
 * stack on every example) reached by two programs is kept once, by the shorter program found
 * first, so no size-minimal candidate is lost. The baseline enumerates the same well-typed word
 * sequences without that deduplication, under the same budgets, so the candidate counts compare.
 *
 * A solver model is never trusted: a candidate is refined only when a model replays through this
 * lane's own evaluator on the original graphs, and the replay's exact bits become the
 * counterexample. The generated Lean module also records that refused candidate with its witness,
 * so the refinement step is kernel-visible.
 *
 * Usage (pinned Node, env -i, from a clean checkout):
 *   node verification/synthesis/cegis.mjs --noble CLI --bitwuzla BIN --z3 BIN --time GNU_TIME
 *     --output NEW_FILE --scratch DIR --lean NEW_FILE [--check-lean COMMITTED_FILE]
 */
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  ENCODER, MASK, SOLVERS, admit, encode, encoderChecks, evaluate, hex16, outcome, sha, solve,
} from '../symbolic/qfbv.mjs';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const LANE = 'verification/synthesis/cegis.mjs';
const QFBV = 'verification/symbolic/qfbv.mjs';
const MANIFEST = 'verification/synthesis/benchmarks-v1.txt';
const LEAN = 'proofs/foundation/NobleFoundation/SynthesisCertified.lean';
const IR = 'crates/noble-ngraph/src/ir.rs';
const NAMESPACE = 'NobleFoundation.Synthesis';
/** `i64::MIN` and `i64::MAX` as exact 64-bit values, so every declared point is masked. */
const MIN = 1n << 63n;
const MAX = 2n ** 63n - 1n;
const ZERO = '0x0000000000000000';
const read = relative => fs.readFileSync(path.join(ROOT, relative));
const decimal = bits => BigInt.asIntN(64, bits).toString();
const stable = value => JSON.stringify(value, (_, one) => (typeof one === 'bigint' ? one.toString() : one));
const measurementSince = (started, cpuStart) => {
  const cpu = process.cpuUsage(cpuStart);
  return { wall_ns: Number(process.hrtime.bigint() - started), cpu_user_ns: cpu.user * 1000,
    cpu_system_ns: cpu.system * 1000, process_peak_rss_kb: process.resourceUsage().maxRSS };
};
/** A value as the receipt shows it: signed decimal for I64, `true`/`false`, or a decimal list. */
const shown = value => (Array.isArray(value) ? value.map(decimal) : typeof value === 'boolean' ? String(value) : decimal(value));

/* ---------- The frozen manifest, parsed strictly ---------- */

const I64_GRAMMAR = Object.freeze({ name: 'synth-i64/0' });
const LIST_GRAMMAR = Object.freeze({ name: 'synth-list/0' });
const I64_WORDS = Object.freeze(['dup', 'swap', '+', '-', '*', '=']);
const LIST_WORDS = Object.freeze(['dup', 'swap', 'nil', 'cons', '+', '-']);
const FAMILIES = Object.freeze(['arithmetic', 'bitwise-0/1', 'list-run']);
const OUT_TYPES = Object.freeze(['i64', 'bool', 'list']);
const GLOBAL_KEYS = Object.freeze(['grammar', 'grammar-list', 'seed', 'consts', 'list-consts', 'size',
  'max-stack', 'max-candidates', 'max-rounds', 'solver-ms', 'list-max-stack']);

/** Signed decimal I64 to its exact 64-bit value. */
function signed(text) {
  assert.match(text, /^(?:0|-?[1-9]\d*)$/, `not a canonical signed I64: ${text}`);
  const value = BigInt(text);
  assert.ok(value >= -(1n << 63n) && value < 1n << 63n, `${text} is outside I64`);
  return value & MASK;
}

/** A rendered list of I64 values: `[2, -3]`. */
function renderedList(text) {
  assert.match(text, /^\[(?:-?\d+(?:, -?\d+)*)?\]$/, `not a rendered I64 list: ${text}`);
  return text.slice(1, -1).split(', ').filter(Boolean).map(signed);
}

function parseManifest(text) {
  const globals = {};
  const tasks = [];
  let current = null;
  for (const [index, raw] of text.split('\n').entries()) {
    const line = raw.trim();
    if (line === '' || line.startsWith('#')) continue;
    const words = line.split(/\s+/);
    const where = `line ${index + 1}`;
    if (words[0] === 'task') {
      const [name, family, out] = words.slice(1, 4);
      assert.match(name, /^[a-z][A-Za-z0-9]*$/, `${where}: task name ${name}`);
      assert.ok(FAMILIES.includes(family) && OUT_TYPES.includes(out), `${where}: task ${name} family or output type`);
      assert.ok(!tasks.some(task => task.name === name), `${where}: duplicate task ${name}`);
      current = { name, family, out, inputs: null, bound: null, controlBound: null, precondition: false,
        refText: null, ref: null, train: [], heldout: [] };
      for (const field of words.slice(4)) {
        const [key, value] = field.split('=');
        assert.ok(['inputs', 'bound', 'control-bound'].includes(key), `${where}: task ${name} field ${field}`);
        const parsed = Number(value);
        assert.ok(Number.isInteger(parsed) && parsed > 0, `${where}: task ${name} ${key} must be a positive integer`);
        if (key === 'inputs') current.inputs = parsed;
        if (key === 'bound') current.bound = parsed;
        if (key === 'control-bound') current.controlBound = parsed;
      }
      assert.ok(current.inputs !== null && current.bound !== null, `${where}: task ${name} needs inputs= and bound=`);
      tasks.push(current);
      continue;
    }
    if (globals[words[0]] !== undefined) assert.fail(`${where}: duplicate ${words[0]}`);
    if (GLOBAL_KEYS.includes(words[0])) {
      assert.equal(current, null, `${where}: a global line must come before the tasks`);
      assert.ok(words.length > 1, `${where}: ${words[0]} needs a value`);
      globals[words[0]] = words.slice(1);
      continue;
    }
    assert.ok(current, `${where} is outside a task: ${line}`);
    if (words[0] === 'ref') {
      assert.equal(current.ref, null, `${where}: task ${current.name} has one ref line`);
      current.refText = words.slice(1).join(' ');
      current.ref = parseReference(current.refText, current);
    } else if (words[0] === 'precondition') {
      assert.deepEqual(words.slice(1), ['0/1'], `${where}: the only declared precondition is 0/1`);
      assert.equal(current.family, 'bitwise-0/1', `${where}: a precondition is a bitwise-0/1 declaration`);
      current.precondition = true;
    } else if (words[0] === 'train' || words[0] === 'heldout') {
      const at = words.indexOf('=');
      assert.ok(at > 1, `${where}: an example needs "= value"`);
      const inputs = words.slice(1, at).map(signed);
      assert.equal(inputs.length, current.inputs, `${where}: example arity`);
      const rawValue = words.slice(at + 1).join(' ');
      const output = current.out === 'bool'
        ? (assert.match(rawValue, /^(?:true|false)$/, `${where}: not a Bool: ${rawValue}`), rawValue === 'true')
        : current.out === 'list' ? renderedList(rawValue) : signed(rawValue);
      (words[0] === 'train' ? current.train : current.heldout).push({ inputs, output });
    } else {
      assert.fail(`${where}: unknown line ${words[0]}`);
    }
  }
  for (const key of GLOBAL_KEYS) assert.ok(globals[key], `manifest is missing ${key}`);
  for (const [key, values] of [['consts', globals.consts], ['list-consts', globals['list-consts']]]) {
    assert.ok(values.length > 0, `${key} needs at least one constant`);
    values.forEach(signed);
  }
  for (const key of ['size', 'max-stack', 'max-candidates', 'max-rounds', 'solver-ms', 'list-max-stack']) {
    assert.match(globals[key][0], /^[1-9]\d*$/, `${key} must be a positive integer`);
    globals[key] = Number(globals[key][0]);
  }
  assert.match(globals.seed[0], /^0x[0-9a-fA-F]{1,16}$/, 'seed must be a hex literal');
  globals.seed = BigInt(globals.seed[0]);
  assert.deepEqual(globals['grammar-list'], ['synth-list/0'], 'the declared list grammar');
  assert.deepEqual(globals.grammar, ['synth-i64/0'], 'the declared grammar');
  assert.ok(tasks.length > 0, 'the manifest declares tasks');
  for (const task of tasks) {
    assert.ok(task.ref, `task ${task.name}: a ref line`);
    assert.ok(task.train.length > 0, `task ${task.name}: at least one train example`);
    assert.equal(task.precondition, task.family === 'bitwise-0/1',
      `task ${task.name}: every bitwise-0/1 task declares its input precondition`);
    const seen = new Map();
    for (const example of task.train) {
      const key = example.inputs.join(',');
      if (seen.has(key)) {
        assert.equal(task.family, 'arithmetic', `task ${task.name}: only a declared negative repeats an input`);
        assert.notEqual(seen.get(key), stable(example.output), `task ${task.name}: a repeated input needs conflicting values`);
      }
      seen.set(key, stable(example.output));
    }
    for (const example of task.heldout) {
      assert.ok(!seen.has(example.inputs.join(',')), `task ${task.name}: held-out inputs must be disjoint from train`);
    }
  }
  return { globals, tasks };
}

/* ---------- Reference expressions: s-expressions over x, y, the list instance and integers ---------- */

function parseReference(text, task) {
  const tokens = text.replace(/\(/g, ' ( ').replace(/\)/g, ' ) ').trim().split(/\s+/);
  let at = 0;
  const expression = () => {
    const token = tokens[at];
    assert.ok(token !== undefined, `task ${task.name}: the reference ends early`);
    at += 1;
    if (token === '(') {
      const head = tokens[at];
      at += 1;
      if (head === 'list') {
        assert.equal(task.family, 'list-run', `task ${task.name}: (list ...) is a list reference`);
        const items = [];
        while (tokens[at] !== ')') {
          assert.ok(tokens[at] !== undefined, `task ${task.name}: an unclosed (list ...)`);
          items.push(tokens[at] === 'n' ? { kind: 'instance' } : { kind: 'int', bits: signed(tokens[at]) });
          at += 1;
        }
        at += 1;
        return { kind: 'list', items };
      }
      assert.ok(['+', '-', '*', '='].includes(head), `task ${task.name}: unknown operator ${head}`);
      const left = expression();
      const right = expression();
      assert.equal(tokens[at], ')', `task ${task.name}: a reference needs a closing parenthesis`);
      at += 1;
      return { kind: head, left, right };
    }
    if (token === 'x' || token === 'y') {
      assert.equal(task.inputs, 2, `task ${task.name}: ${token} needs two inputs`);
      return { kind: 'var', slot: token === 'x' ? 0 : 1 };
    }
    if (token === 'n') {
      assert.equal(task.family, 'list-run', `task ${task.name}: n is the list instance`);
      return { kind: 'instance' };
    }
    return { kind: 'int', bits: signed(token) };
  };
  const result = expression();
  assert.equal(at, tokens.length, `task ${task.name}: trailing tokens after a reference`);
  return result;
}

/** Compiles a reference expression to a graph over the declared inputs, one node per occurrence. */
function referenceGraph(task) {
  const nodes = [...Array(task.inputs)].map((_, slot) => ({ op: 'input', slot }));
  const emit = expression => {
    if (expression.kind === 'var') return expression.slot;
    if (expression.kind === 'int') {
      nodes.push({ op: 'constant', bits: expression.bits });
      return nodes.length - 1;
    }
    assert.ok(['+', '-', '*', '='].includes(expression.kind), `task ${task.name}: a graph reference has no ${expression.kind}`);
    const a = emit(expression.left);
    const b = emit(expression.right);
    nodes.push({ op: { '+': 'add', '-': 'sub', '*': 'mul', '=': 'eq' }[expression.kind], a, b });
    return nodes.length - 1;
  };
  const out = emit(task.ref);
  return { nodes, out, type: nodes[out].op === 'eq' ? 'bool' : 'i64' };
}

/** The list a `(list ...)` reference denotes for one instance value. */
const referenceList = (task, instance) => task.ref.items.map(item => (item.kind === 'instance' ? instance : item.bits));

/* ---------- Graphs, pairs, affine forms ---------- */

/** Both sides' results must sit at one index (`Pair.outputs`): pad the shorter before its result. */
function pairOf(name, candidate, reference) {
  const length = Math.max(candidate.nodes.length, reference.nodes.length);
  const pad = side => {
    assert.equal(side.out, side.nodes.length - 1, `${name}: a side's result is its last node`);
    const nodes = side.nodes.slice(0, -1);
    const result = side.nodes[side.nodes.length - 1];
    while (nodes.length < length - 1) nodes.push({ op: 'constant', bits: 0n });
    nodes.push(result);
    return nodes;
  };
  const pair = { name, left: pad(candidate), right: pad(reference), outputs: [length - 1] };
  pair.digest = sha(JSON.stringify(pair, (_, value) => (typeof value === 'bigint' ? `0x${hex16(value)}` : value)));
  return pair;
}

/**
 * Every node's value as an affine form over the inputs, modulo 2^64: `{ coef, constant }`.
 * `null` for a Bool node; `{ nonlinear }` for a product of two input-dependent values.
 */
function affineForms(graph, inputs) {
  const forms = [];
  const zero = () => [...Array(inputs)].fill(0n);
  const isConstant = form => form.coef.every(coefficient => coefficient === 0n);
  for (const node of graph) {
    if (node.op === 'input') {
      const coef = zero();
      coef[node.slot] += 1n;
      forms.push({ coef, constant: 0n });
    } else if (node.op === 'constant') {
      forms.push({ coef: zero(), constant: node.bits & MASK });
    } else if (node.op === 'eq') {
      forms.push(null);
    } else {
      const a = forms[node.a];
      const b = forms[node.b];
      if (a === null || b === null) return { nonlinear: 'a node reads a Bool' };
      if (node.op === 'add' || node.op === 'sub') {
        const sign = node.op === 'add' ? 1n : -1n;
        forms.push({ coef: a.coef.map((coefficient, at) => (coefficient + sign * b.coef[at]) & MASK),
          constant: (a.constant + sign * b.constant) & MASK });
      } else if (isConstant(a) || isConstant(b)) {
        const [scale, scaled] = isConstant(a) ? [a, b] : [b, a];
        forms.push({ coef: scaled.coef.map(coefficient => (coefficient * scale.constant) & MASK),
          constant: (scaled.constant * scale.constant) & MASK });
      } else {
        return { nonlinear: 'a product of two input-dependent values' };
      }
    }
  }
  return { forms };
}

/** The certified class: both graphs are I64 nodes and the listed outputs have equal affine forms. */
function certificateOf(pair, inputs) {
  const left = affineForms(pair.left, inputs);
  const right = affineForms(pair.right, inputs);
  if (left.nonlinear || right.nonlinear) return { certifiable: false, reason: left.nonlinear ?? right.nonlinear };
  const out = pair.outputs[0];
  const a = left.forms[out];
  const b = right.forms[out];
  if (a === null || b === null) return { certifiable: false, reason: 'a Bool output is not a linear equivalence' };
  const equal = a.constant === b.constant && a.coef.every((coefficient, at) => coefficient === b.coef[at]);
  return equal
    ? { certifiable: true, form: { coef: a.coef.map(coefficient => `0x${hex16(coefficient)}`), constant: `0x${hex16(a.constant)}` } }
    : { certifiable: false, reason: 'the affine forms differ' };
}

/* ---------- The grammar: typed stacks, one word at a time ---------- */

/** Deterministic word order derived from the seed (splitmix64 over the declared word list). */
function seededOrder(words, seed) {
  let state = seed & MASK;
  const next = () => {
    state = (state + 0x9e3779b97f4a7c15n) & MASK;
    let z = state;
    z = ((z ^ (z >> 30n)) * 0xbf58476d1ce4e5b9n) & MASK;
    z = ((z ^ (z >> 27n)) * 0x94d049bb133111ebn) & MASK;
    return z ^ (z >> 31n);
  };
  const out = words.slice();
  for (let at = out.length - 1; at > 0; at -= 1) {
    const other = Number(next() % BigInt(at + 1));
    [out[at], out[other]] = [out[other], out[at]];
  }
  return out;
}

/** One grammar's words in seeded order: wiring and arithmetic words, then the constants. */
function wordsOf(globals, grammar) {
  const named = grammar === 'i64' ? I64_WORDS : LIST_WORDS;
  const constants = (grammar === 'i64' ? globals.consts : globals['list-consts']).map(text => ({ kind: 'const', bits: signed(text) }));
  const steps = named.map(word => (word === 'dup' || word === 'swap' ? { kind: word } : { kind: 'word', word }));
  return seededOrder([...steps, ...constants], globals.seed);
}

/** One word applied to one example's stack; `null` when ill-typed or past the depth bound. */
function extendStack(stack, step, maxStack) {
  const top = stack[stack.length - 1];
  const below = stack[stack.length - 2];
  const isI64 = value => value !== undefined && value.type === 'i64';
  const push = value => (stack.length + 1 > maxStack ? null : [...stack, value]);
  if (step.kind === 'const') return push({ type: 'i64', value: step.bits });
  if (step.kind === 'dup') return top === undefined ? null : push(top);
  if (step.kind === 'swap') return below === undefined ? null : [...stack.slice(0, -2), top, below];
  if (step.word === 'nil') return push({ type: 'list', value: [] });
  if (step.word === 'cons') {
    if (!isI64(below) || top === undefined || top.type !== 'list') return null;
    return [...stack.slice(0, -2), { type: 'list', value: [below.value, ...top.value] }];
  }
  if (!isI64(top) || !isI64(below)) return null;
  const reduce = value => [...stack.slice(0, -2), value];
  if (step.word === '+') return reduce({ type: 'i64', value: (below.value + top.value) & MASK });
  if (step.word === '-') return reduce({ type: 'i64', value: (below.value - top.value) & MASK });
  if (step.word === '*') return reduce({ type: 'i64', value: (below.value * top.value) & MASK });
  assert.equal(step.word, '=', 'no other word reaches here');
  return reduce({ type: 'bool', value: below.value === top.value });
}

/** One example's stack as a stable key: types and exact values, in order. */
const stackKey = stack => stack.map(value => (value.type === 'i64' ? `i${hex16(value.value)}`
  : value.type === 'bool' ? `b${value.value}` : `l[${value.value.map(hex16).join(',')}]`)).join('|');

/** A program is complete when one value of the task's output type is on the stack. */
const complete = (stacks, out) => stacks.every(stack => stack.length === 1
  && stack[0].type === (out === 'bool' ? 'bool' : out === 'list' ? 'list' : 'i64'));

/** Prune Bool/List stack values that cannot reach the task's requested output type. */
const canStillReachOutput = (stacks, out) => stacks.every(stack => {
  if (stack.some(value => value.type === 'bool')) {
    return out === 'bool' && stack.length === 1 && stack[0].type === 'bool';
  }
  const lists = stack.filter(value => value.type === 'list').length;
  return lists <= 1 && (lists === 0 || out === 'list');
});

/** A lower bound on the words needed to reduce a viable stack to the requested output type. */
function minimumCompletionWords(stack, out) {
  if (stack.length === 1 && stack[0].type === out) return 0;
  if (out === 'bool') return stack.length === 1 ? 2 : stack.length - 1;
  if (out === 'list') {
    const lists = stack.filter(value => value.type === 'list').length;
    return stack.length - lists + (lists === 0 ? 1 : 0);
  }
  return stack.length - 1;
}

const canCompleteWithin = (stacks, out, remaining) =>
  stacks.every(stack => minimumCompletionWords(stack, out) <= remaining);

/** What one complete program computes on every example. */
const observedOf = stacks => stacks.map(stack => stack[0].value);

/**
 * Deterministic bottom-up enumeration by size, deduplicated by observational equivalence on the
 * current examples: a state is the whole stack on every example, and a state keeps its first
 * (shortest) program. A pruned state can be reached again only by a longer program, and every
 * extension of the second has a no-longer counterpart from the kept one, so no size-minimal
 * matching program is lost.
 */
function discover(task, examples, expected, spec, budget) {
  const start = { stacks: examples.map(inputs => inputs.map(value => ({ type: 'i64', value }))), words: [] };
  const key = state => state.stacks.map(stackKey).join('/');
  const matches = state => state.stacks.every((stack, at) => stable(stack[0].type === 'list'
    ? stack[0].value : stack[0].value) === stable(expected[at]));
  let frontier = [start];
  const seen = new Set([key(start)]);
  const sizes = [{ size: 0, states: 1, complete: 0 }];
  let candidates = 0;
  for (let size = 0; size <= spec.bound; size += 1) {
    let found = null;
    let completeHere = 0;
    for (const state of frontier) {
      if (!canCompleteWithin(state.stacks, task.out, spec.bound - size)) continue;
      if (!complete(state.stacks, task.out)) continue;
      completeHere += 1;
      if (!found && matches(state)) found = state;
    }
    sizes[sizes.length - 1].complete = completeHere;
    if (found) return { program: found.words, sizes, candidates, observed: observedOf(found.stacks) };
    if (size === spec.bound) break;
    const next = new Map();
    for (const state of frontier) {
      for (const step of spec.words) {
        const stacks = [];
        let live = true;
        for (const stack of state.stacks) {
          const extended = extendStack(stack, step, spec.maxStack);
          if (!extended) { live = false; break; }
          stacks.push(extended);
        }
        if (!live || !canStillReachOutput(stacks, task.out)
          || !canCompleteWithin(stacks, task.out, spec.bound - size - 1)) continue;
        // Fuel counts admitted typed extensions, not rejected word/stack combinations.
        if (candidates >= budget) return { exhausted: 'candidate fuel', sizes, candidates };
        candidates += 1;
        const extended = { stacks, words: [...state.words, step] };
        const id = key(extended);
        if (seen.has(id)) continue;
        seen.add(id);
        next.set(id, extended);
        if (complete(stacks, task.out) && matches(extended)) {
          sizes.push({ size: size + 1, states: next.size, complete: 1, partial: true });
          return { program: extended.words, sizes, candidates, observed: observedOf(stacks) };
        }
      }
    }
    frontier = [...next.values()];
    sizes.push({ size: size + 1, states: frontier.length, complete: 0 });
  }
  return { exhausted: 'size bound', sizes, candidates };
}

/** The plain enumerative baseline: the same well-typed sequences, with no equivalence dedup. */
function baseline(task, examples, expected, spec, budget) {
  let candidates = 0;
  const outcomeOf = found => found ?? { exhausted: 'size bound' };
  const visit = (stacks, depth) => {
    if (!canStillReachOutput(stacks, task.out)
      || !canCompleteWithin(stacks, task.out, spec.bound - depth.length)) return null;
    if (complete(stacks, task.out)) {
      const observed = observedOf(stacks);
      const matches = observed.every((value, at) => stable(value) === stable(expected[at]));
      if (matches) return { program: depth, observed };
    }
    if (depth.length === spec.bound) return null;
    for (const step of spec.words) {
      const next = [];
      let live = true;
      for (const stack of stacks) {
        const extended = extendStack(stack, step, spec.maxStack);
        if (!extended) { live = false; break; }
        next.push(extended);
      }
      if (!live || !canStillReachOutput(next, task.out)
        || !canCompleteWithin(next, task.out, spec.bound - depth.length - 1)) continue;
      // The baseline uses the same candidate definition and budget as the deduplicated search.
      if (candidates >= budget) return { exhausted: 'candidate fuel' };
      candidates += 1;
      const found = visit(next, [...depth, step]);
      if (found) return found;
    }
    return null;
  };
  const found = outcomeOf(visit(examples.map(inputs => inputs.map(value => ({ type: 'i64', value }))), []));
  return { ...found, candidates };
}

/* ---------- The program as a graph and as source ---------- */

/** The graph of a stack program: pre-order nodes, one per word, the result last. */
function programGraph(task, program) {
  const nodes = [...Array(task.inputs)].map((_, slot) => ({ op: 'input', slot }));
  const stack = nodes.map((_, at) => at);
  for (const step of program) {
    if (step.kind === 'const') {
      nodes.push({ op: 'constant', bits: step.bits });
      stack.push(nodes.length - 1);
    } else if (step.kind === 'dup') {
      stack.push(stack[stack.length - 1]);
    } else if (step.kind === 'swap') {
      [stack[stack.length - 1], stack[stack.length - 2]] = [stack[stack.length - 2], stack[stack.length - 1]];
    } else {
      const b = stack.pop();
      const a = stack.pop();
      nodes.push({ op: { '+': 'add', '-': 'sub', '*': 'mul', '=': 'eq' }[step.word], a, b });
      stack.push(nodes.length - 1);
    }
  }
  assert.equal(stack.length, 1, `${task.name}: a complete program leaves one value`);
  return { nodes, out: stack[0], type: task.out };
}

/** The program's words as Noble source text, with an optional leading instance word. */
function sourceOf(program, instance = null) {
  const words = program.map(step => (step.kind === 'const' ? decimal(step.bits)
    : step.kind === 'dup' ? 'dup' : step.kind === 'swap' ? 'swap' : step.word));
  return [...(instance === null ? [] : [decimal(instance)]), ...words].join(' ');
}

/** A program's value on one input tuple, through this lane's evaluator. */
const programValue = (task, program, inputs) => {
  const graph = programGraph(task, program);
  const slots = new Map(inputs.map((bits, slot) => [slot, bits]));
  return evaluate(graph.nodes, slots)[graph.out];
};

/* ---------- The trusted path: the selected CLI ---------- */

function cli(binary, argv, { lines = false, input = null } = {}) {
  const started = process.hrtime.bigint();
  const run = spawnSync(binary, argv, { encoding: 'utf8', timeout: 180_000, maxBuffer: 1 << 24, env: {}, input: input ?? undefined });
  const elapsed_ns = Number(process.hrtime.bigint() - started);
  assert.equal(run.signal, null, `${argv.join(' ')}: killed by ${run.signal}`);
  assert.equal(run.status, 0, `${argv.join(' ')}: exit status ${run.status}: ${(run.stderr ?? '').slice(0, 300)}`);
  const rows = (run.stdout ?? '').trim().split('\n').filter(Boolean).map(line => JSON.parse(line));
  assert.ok(rows.length > 0, `${argv.join(' ')}: no report (stderr ${(run.stderr ?? '').slice(0, 300)})`);
  return { rows: lines ? rows : rows[0], status: run.status, elapsed_ns };
}

const renderValue = value => (typeof value === 'boolean' ? `bool:${value}` : `i64:${decimal(value)}`);

/** `noble ngraph extract` then `noble ngraph eval` with both of its evaluators. */
function readmitGraph(binary, directory, task, program, points, expected) {
  const file = path.join(directory, `${task.name}.noble`);
  fs.writeFileSync(file, `${sourceOf(program)}\n`, { flag: 'wx' });
  const graph = path.join(directory, `${task.name}.graph`);
  const inputs = [...Array(task.inputs)].map(() => 'i64').join(',');
  const extracted = cli(binary, ['ngraph', 'extract', file, '--inputs', inputs, '--out', graph]);
  assert.equal(extracted.rows.outcome, 'normal', `${task.name}: extraction refused: ${JSON.stringify(extracted.rows).slice(0, 400)}`);
  const vectors = path.join(directory, `${task.name}.vectors`);
  fs.writeFileSync(vectors, ['noble-ngraph-vectors/0', `types ${inputs.replaceAll(',', ' ')}`,
    ...points.map(inputsAt => `v ${inputsAt.map(decimal).join(' ')}`), ''].join('\n'), { flag: 'wx' });
  const run = flag => {
    const { rows, elapsed_ns } = cli(binary, ['ngraph', 'eval', graph, vectors, ...flag], { lines: true });
    const outputs = rows.filter(row => row.outputs !== undefined).map(row => row.outputs[0]);
    assert.equal(outputs.length, points.length, `${task.name}: one evaluation per point`);
    return { outputs, elapsed_ns };
  };
  const reference = run(['--reference']);
  const packed = run([]);
  points.forEach((inputsAt, at) => {
    assert.equal(packed.outputs[at], reference.outputs[at], `${task.name}: the graph evaluators disagree at ${inputsAt.map(decimal).join(',')}`);
    assert.equal(packed.outputs[at], renderValue(expected[at]), `${task.name}: the graph disagrees with the model at ${inputsAt.map(decimal).join(',')}`);
  });
  return { outcome: extracted.rows.outcome, graph: extracted.rows.graph, graph_sha256: sha(fs.readFileSync(graph)),
    points: points.length, evaluators_agree: true, reference_ns: reference.elapsed_ns, packed_ns: packed.elapsed_ns };
}

/** `noble worker scalar`: the compiled Wasm endpoint of the same source. */
function readmitScalar(binary, directory, task, program, points, expected) {
  const file = path.join(directory, `${task.name}.noble`);
  const runs = points.map((inputsAt, at) => {
    const { rows } = cli(binary, ['worker', 'scalar', file, '--state', decimal(inputsAt[0]), '--delta', decimal(inputsAt[1])]);
    assert.equal(rows.outcome, 'normal', `${task.name}: worker scalar refused: ${JSON.stringify(rows).slice(0, 300)}`);
    assert.equal(rows.interface, 'I64 I64 -- I64 ! {}');
    assert.equal(rows.guest_requests, 0);
    assert.equal(rows.protected_operations, 0);
    assert.equal(rows.typed_output.type, 'I64');
    assert.equal(rows.typed_output.value, decimal(expected[at]), `${task.name}: the endpoint disagrees at ${inputsAt.map(decimal).join(',')}`);
    return { inputs: inputsAt.map(decimal), value: rows.typed_output.value, wasm_sha256: rows.module.wasm_sha256 };
  });
  return { interface: 'I64 I64 -- I64 ! {}', points: runs.length,
    wasm_sha256: [...new Set(runs.map(run => run.wasm_sha256))] };
}

/** `noble live repl`: the only path that executes a list program. */
function readmitRun(binary, task, program, instances, expected) {
  const runs = instances.map((instance, at) => {
    const source = sourceOf(program, instance);
    const { rows } = cli(binary, ['live', 'repl'], { lines: true, input: `${source}\n` });
    const last = rows.at(-1);
    assert.equal(last.metrics.status, 0, `${task.name}: live repl status at ${decimal(instance)}`);
    assert.equal(last.stack.length, 1, `${task.name}: one value on the stack at ${decimal(instance)}`);
    const rendered = last.stack[0];
    assert.equal(rendered.type, 'List', `${task.name}: the program leaves a list at ${decimal(instance)}`);
    const values = rendered.value.map(item => (assert.equal(item.type, 'I64'), BigInt(item.value) & MASK));
    assert.deepEqual(values, expected[at], `${task.name}: live repl disagrees at ${decimal(instance)}`);
    return { instance: decimal(instance), source, stack: rendered, execution_steps: last.metrics.execution_steps };
  });
  return { interface: 'I64 -- List<I64>', runs };
}

/** Evaluate a complete program in this lane's model, including the run-only list fragment. */
function modelValue(task, program, inputs, maxStack) {
  if (task.out !== 'list') return programValue(task, program, inputs).value;
  let stack = inputs.map(value => ({ type: 'i64', value }));
  for (const step of program) {
    stack = extendStack(stack, step, maxStack);
    assert.ok(stack, `${task.name}: a declared list program is ill-typed on its input`);
  }
  assert.equal(stack.length, 1, `${task.name}: a list program leaves one value`);
  assert.equal(stack[0].type, 'list');
  return stack[0].value;
}

/** The deterministic boundary points shared by every admitted candidate. */
function boundaryPoints(task) {
  return task.inputs === 2 ? [[MIN, 0n], [MAX, 1n], [0n, MIN], [(-7n) & MASK, 7n]]
    : [[MIN], [MAX], [(-7n) & MASK]];
}

const uniquePoints = points => {
  const seen = new Set();
  return points.filter(inputs => {
    const key = inputs.join(',');
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
};

/** Re-admit one reported program through its applicable existing Noble execution paths. */
function readmitProgram(binary, directory, task, program, points, maxStack) {
  fs.mkdirSync(directory, { recursive: true });
  const expected = points.map(inputs => modelValue(task, program, inputs, maxStack));
  const report = { points: points.map(inputs => inputs.map(decimal)),
    outputs: expected.map(shown) };
  if (task.out === 'list') {
    report.run = readmitRun(binary, task, program, points.map(inputs => inputs[0]), expected);
  } else {
    report.graph = readmitGraph(binary, directory, task, program, points, expected);
    if (task.out === 'i64' && task.inputs === 2) {
      report.scalar = readmitScalar(binary, directory, task, program, points, expected);
    }
  }
  return report;
}

/* ---------- The generated Lean module ---------- */

const leanNode = node => (node.op === 'input' ? `.input ${node.slot}`
  : node.op === 'constant' ? `.constant 0x${hex16(node.bits)}`
    : `.${{ add: 'add', sub: 'sub', mul: 'mul', eq: 'eq' }[node.op]} ${node.a} ${node.b}`);

const leanPair = pair => [
  `  { left := FirstProjection.ofList [${pair.left.map(leanNode).join(', ')}] ${pair.left.length - 1},`,
  `    right := FirstProjection.ofList [${pair.right.map(leanNode).join(', ')}] ${pair.right.length - 1},`,
  `    outputs := [${pair.outputs.join(', ')}] }`,
];

function leanWitness(witness, inputs) {
  const parts = [];
  for (let slot = 0; slot < inputs; slot += 1) {
    const bits = witness?.[`s${slot}`];
    assert.match(bits ?? '', /^0x[0-9a-f]{16}$/, `counterexample witness supplies exact bits for slot ${slot}`);
    parts.push(`if slot = ${slot} then ${bits}`);
  }
  return `fun slot => ${parts.length ? `${parts.join(' else ')} else ${ZERO}` : ZERO}`;
}

/** Exact witness and negative certificate for a candidate the CEGIS loop actually refused. */
function leanRefusalForTask(result) {
  const refused = result.rounds.find(round => round.verdict === 'refuted');
  if (!refused || !result.lean?.refused) return { lines: [], theorems: [] };
  const { name } = result;
  const lines = [
    `/-! ### \`${name}\`: the candidate the examples alone admitted, refused -/`, '',
    '/-- The program the examples alone admitted, whose refinement found a counterexample. -/',
    `def ${name}_refused : Pair :=`, ...leanPair(result.lean.refused), '',
    '/-- The counterexample, as exact bits: the kernel decides `refuted` on them. -/',
    `def ${name}_refusedWitness : Nat → BitVec 64 :=`,
    `  ${leanWitness(refused.witness, result.declaration.inputs)}`, '',
    `theorem ${name}_refused_witness_refutes :`,
    `    refuted ${name}_refused ${name}_refusedWitness = true := by`, '  decide', '',
    '/-- **The refused candidate has no certificate**, from its kernel-checked witness. -/',
    `theorem ${name}_refused_not_certifiable :`,
    `    ¬ ∀ inputs, agreesOn ${name}_refused inputs :=`,
    `  not_certifiable_of_refuted ${name}_refused _ ${name}_refused_witness_refutes`, '',
  ];
  const theorems = [
    { name: `${NAMESPACE}.${name}_refused_witness_refutes`, phase: 1, role: 'negative-control',
      claim: `the refused candidate \`${refused.program}\` really refutes \`${name}\`'s reference at the recorded bits` },
    { name: `${NAMESPACE}.${name}_refused_not_certifiable`, phase: 1, role: 'negative-control',
      claim: `the refused candidate for \`${name}\` has no certificate, from its kernel-checked witness` },
  ];
  return { lines, theorems };
}

/** The certificate fragment for one proved task, plus its first refused round when present. */
function leanForTask(result) {
  const lines = [];
  const { name } = result;
  lines.push(`/-! ## \`${name}\`: \`${result.search.program}\` against \`${result.reference}\` -/`, '');
  lines.push('/-- The pair: the synthesized program on the left, the declared reference on the right. -/');
  lines.push(`def ${name} : Pair :=`, ...leanPair(result.lean.pair), '');
  lines.push(`/-- **\`${name}\` agrees with its reference graph on every input.** -/`);
  lines.push(`theorem ${name}_certified (inputs : Nat → BitVec 64) : agreesOn ${name} inputs := by`);
  lines.push('  intro out hout', `  simp [${name}] at hout`, '  subst hout');
  lines.push(`  simp [${name}, value, valuesUpTo, evalStep, NobleFoundation.FirstProjection.ofList]`, '  bv_omega', '');
  const theorems = [{ name: `${NAMESPACE}.${name}_certified`, phase: 1, role: 'real-artifact',
    claim: `the synthesized program \`${result.search.program}\` and its reference \`${result.reference}\` agree on every input, a graph-model theorem` }];
  const refusal = leanRefusalForTask(result);
  lines.push(...refusal.lines);
  theorems.push(...refusal.theorems);
  return { lines, theorems };
}

function emitLean(results) {
  const proved = results.filter(result => result.status === 'proved');
  const controls = results.flatMap(result => {
    const control = result.controls?.control_bound;
    if (!control?.certificate?.theorem) return [];
    return [{ name: `${result.name}Control`, reference: result.reference,
      search: { program: control.program }, declaration: result.declaration, rounds: [],
      lean: { pair: control.pair, refused: null } }];
  });
  const certified = [...proved, ...controls];
  const lines = [
    '/-',
    'Strict-kernel certificates for the bounded synthesis benchmarks (issue #116).',
    '',
    'Generated by `verification/synthesis/cegis.mjs` from `verification/synthesis/benchmarks-v1.txt`',
    'and the programs that run discovered; do not edit by hand. `cegis.mjs --check-lean` refuses a',
    'committed file that differs from its emission.',
    '',
    "Each `_certified` theorem says the synthesized program's graph and its declared reference graph",
    'agree on **every** input. The lane emits one only after both graphs have equal affine forms',
    'modulo 2^64; the grammar also permits nonlinear products, which this certificate class does',
    'not cover. The proofs unfold the model and close with `bv_omega`, whose proof terms the kernel',
    'checks; no native evaluation is involved, and the foundation lane audits the axioms.',
    '',
    'For each task with a refused candidate, its first refused pair and exact witness are emitted;',
    'the kernel decides `refuted` on that witness, and `_refused_not_certifiable` derives that the',
    'pair cannot agree on every input. These are negative controls, not claims about later candidates.',
    '',
    'These are graph-model theorems: no encoder, formula, solver, emitted Wasm or Rust evaluator is a',
    "subject. Reading the synthesized words as this model's nodes is the one trusted translation, and",
    'the command-line re-admission in the lane receipt is finite evidence, not a theorem.',
    '-/',
    'import NobleFoundation.SymbolicPair',
    'import NobleFoundation.FirstProjection',
    '',
    `namespace ${NAMESPACE}`,
    '',
    'open NobleFoundation.PackedSchedule NobleFoundation.SymbolicPair',
    '',
    '/-- A pair with a refuting input has no certificate. -/',
    'theorem not_certifiable_of_refuted (pair : Pair) (witness : Nat → BitVec 64)',
    '    (hrefuted : refuted pair witness = true) : ¬ ∀ inputs, agreesOn pair inputs := by',
    '  intro hall',
    '  have hfalse := (not_refuted_iff_agrees pair witness).mpr (hall witness)',
    '  rw [hrefuted] at hfalse',
    '  exact Bool.noConfusion hfalse',
    '',
  ];
  const theorems = [{ name: `${NAMESPACE}.not_certifiable_of_refuted`, phase: 2, role: 'symbolic-solver',
    claim: 'a pair with a refuting input has no certificate for every input' }];
  for (const result of certified) {
    const fragment = leanForTask(result);
    lines.push(...fragment.lines);
    theorems.push(...fragment.theorems);
  }
  for (const result of results.filter(result => result.status !== 'proved' && result.lean?.refused)) {
    const fragment = leanRefusalForTask(result);
    lines.push(...fragment.lines);
    theorems.push(...fragment.theorems);
  }
  lines.push(`end ${NAMESPACE}`, '');
  return { text: lines.join('\n'), theorems };
}

/* ---------- Options ---------- */

function options() {
  const allowed = ['--noble', '--bitwuzla', '--z3', '--time', '--output', '--lean', '--check-lean', '--scratch'];
  const argv = process.argv.slice(2);
  assert.equal(argv.length % 2, 0, `usage: cegis.mjs ${allowed.map(flag => `${flag} V`).join(' ')}`);
  const out = {};
  for (let at = 0; at < argv.length; at += 2) {
    assert.ok(allowed.includes(argv[at]) && !(argv[at] in out), `unexpected argument ${argv[at]}`);
    out[argv[at]] = argv[at + 1];
  }
  for (const key of ['--noble', '--bitwuzla', '--z3', '--time', '--output', '--scratch']) {
    assert.ok(path.isAbsolute(out[key]), `${key} must be absolute`);
  }
  for (const key of ['--output', '--lean']) {
    if (out[key] === undefined) continue;
    assert.ok(path.isAbsolute(out[key]), `${key} must be absolute`);
    assert.ok(!fs.existsSync(out[key]), `${key} must not exist`);
    assert.ok(!path.resolve(out[key]).startsWith(ROOT + path.sep), `${key} must be outside the repository`);
  }
  if (out['--check-lean'] !== undefined) assert.ok(path.isAbsolute(out['--check-lean']), '--check-lean must be absolute');
  assert.ok(!path.resolve(out['--scratch']).startsWith(ROOT + path.sep), '--scratch must be outside the repository');
  assert.ok(!path.resolve(out['--scratch']).startsWith('/tmp/'), '--scratch must not use /tmp');
  assert.ok(!path.resolve(out['--scratch']).startsWith('/var/tmp/'), '--scratch must not use /var/tmp');
  assert.ok(out['--output'] && (out['--lean'] || out['--check-lean']), 'the lane needs --output and --lean or --check-lean');
  return out;
}

/* ---------- One task ---------- */

/** The equivalence query for one pair: the declared precondition, if any, restricts the inputs. */
function solverCheck(task, pair, tools, solverMs, semantic, profile) {
  const types = admit(pair);
  const bare = encode(pair, types);
  const assumptions = task.precondition
    ? bare.names.map(name => `(or (= ${name} #x0000000000000000) (= ${name} #x0000000000000001))`)
    : [];
  const query = assumptions.length ? encode(pair, types, { assumptions }) : bare;
  const expected = { encoder: ENCODER, semantic_revision: semantic, profile };
  const binding = { ...expected, pair_sha256: pair.digest };
  const answers = SOLVERS.map(solver => solve(tools, solver, query, { timeoutMs: solverMs }));
  const outcomes = answers.map(answer => outcome(pair, binding, expected, answer, null));
  const points = task.precondition
    ? [...Array(2 ** query.names.length)].map((_, bits) => new Map(query.names.map((name, at) => [Number(name.slice(1)), (BigInt(bits) >> BigInt(at)) & 1n])))
    : undefined;
  const { detected_by, ...checks } = encoderChecks(tools, pair, types, query, [], points);
  assert.deepEqual(detected_by, [], `${task.name}: the honest encoding passes its checks`);
  return {
    query: { bytes: query.bytes, nodes: query.nodes, sha256: query.sha256, assumptions: query.assumptions },
    answers: answers.map((answer, at) => ({ solver: answer.solver, answer: answer.answer, status: outcomes[at].status,
      reason: outcomes[at].reason, wall_ns: answer.wall_ns, cpu_user_ns: answer.cpu_user_ns,
      cpu_system_ns: answer.cpu_system_ns, peak_rss_kb: answer.peak_rss_kb,
      witness: outcomes[at].witness ?? null, replay_ns: outcomes[at].replay_ns ?? null })),
    encoder_checks: checks,
  };
}

function runTask(task, context) {
  const { globals, tools, binary, directory, i64Words, listWords, semantic, profile } = context;
  const listFamily = task.family === 'list-run';
  const spec = { words: listFamily ? listWords : i64Words, bound: task.bound,
    maxStack: listFamily ? globals['list-max-stack'] : globals['max-stack'] };
  const reference = listFamily ? null : referenceGraph(task);
  assert.equal(reference?.type ?? task.out, task.out, `${task.name}: the reference's output type is the declared one`);

  const referenceValueAt = inputs => (listFamily ? referenceList(task, inputs[0])
    : evaluate(reference.nodes, new Map(inputs.map((bits, slot) => [slot, bits])))[reference.out].value);
  const examplesByInput = new Map();
  for (const example of task.train) {
    const key = example.inputs.join(',');
    if (!examplesByInput.has(key)) examplesByInput.set(key, []);
    examplesByInput.get(key).push(example);
  }
  const contradictory = [...examplesByInput.values()].find(group =>
    new Set(group.map(example => stable(example.output))).size > 1) ?? null;

  // The declared examples must be the reference's own values: a typo is a refused run, not a benchmark.
  for (const example of [...task.train, ...task.heldout]) {
    const referenceValue = referenceValueAt(example.inputs);
    const group = examplesByInput.get(example.inputs.join(','));
    if (group && new Set(group.map(item => stable(item.output))).size > 1) {
      assert.ok(group.some(item => stable(item.output) === stable(referenceValue)),
        `${task.name}: a contradictory example group must include the reference's output`);
      assert.ok(group.some(item => stable(item.output) !== stable(referenceValue)),
        `${task.name}: a contradictory example group must contradict its reference`);
    } else {
      assert.equal(stable(referenceValue), stable(example.output),
        `${task.name}: the declared example at ${example.inputs.map(decimal).join(',')} is not the reference's value`);
    }
  }
  const examples = task.train.map(example => example.inputs.slice());
  const expected = task.train.map(example => example.output);
  const pairFor = program => pairOf(task.name, programGraph(task, program), reference);
  const record = { name: task.name, family: task.family, out: task.out, reference: task.refText,
    declaration: { inputs: task.inputs, bound: task.bound, control_bound: task.controlBound ?? null,
      precondition: task.precondition ? '0/1 on every slot the query reads' : null },
    examples: { train: task.train.length, heldout: task.heldout.length, counterexamples: 0 },
    search: { grammar: listFamily ? LIST_GRAMMAR.name : I64_GRAMMAR.name,
      words: spec.words.map(step => (step.kind === 'const' ? decimal(step.bits) : (step.word ?? step.kind))),
      rounds: 0, candidates: 0, program: null, size: null, sizes: null, measurement: null } };

  // The declared negative whose examples admit no function at all.
  const runBaseline = () => {
    const started = process.hrtime.bigint();
    const cpuStart = process.cpuUsage();
    const plain = baseline(task, task.train.map(example => example.inputs.slice()),
      task.train.map(example => example.output), spec, globals['max-candidates']);
    record.baseline = { candidates: plain.candidates, program: plain.program ? sourceOf(plain.program) : null,
      size: plain.program?.length ?? null, exhausted: plain.exhausted ?? null,
      measurement: measurementSince(started, cpuStart) };
    return plain;
  };

  if (contradictory) {
    record.status = 'unsatisfiable';
    record.reason = 'the examples give one input two output values, and no program is a function there';
    record.rounds = [];
    record.solver_check = null;
    record.heldout = { status: 'not-run', passed: 0, total: 0, entries: [],
      reason: 'unsatisfiable examples; no program is emitted' };
    record.readmission = null;
    record.search.measurement = { wall_ns: 0, cpu_user_ns: 0, cpu_system_ns: 0,
      process_peak_rss_kb: process.resourceUsage().maxRSS };
    const plain = runBaseline();
    assert.equal(plain.program, undefined, `${task.name}: enumerative baseline cannot satisfy contradictory examples`);
    return record;
  }

  // Discovery with counterexample-guided refinement (non-list families only reach the solvers).
  const rounds = [];
  const candidateRounds = [];
  let fuel = globals['max-candidates'];
  let program = null;
  let lastCheck = null;
  let certificate = null;
  let refusedPair = null;
  let status = null;
  const searchStarted = process.hrtime.bigint();
  const searchCpuStart = process.cpuUsage();
  for (let round = 1; round <= globals['max-rounds']; round += 1) {
    record.search.rounds = round;
    const roundExamples = examples.map(inputs => inputs.slice());
    const found = discover(task, examples, expected, spec, fuel);
    record.search.candidates += found.candidates;
    fuel = Math.max(0, fuel - found.candidates);
    record.search.sizes = found.sizes;
    if (found.exhausted) {
      status = 'exhausted';
      record.reason = `no program within the declared ${found.exhausted} satisfies the examples after ${round - 1} refinements`;
      rounds.push({ round, exhausted: found.exhausted, candidates: found.candidates, states: found.sizes?.at(-1) ?? null });
      break;
    }
    program = found.program;
    candidateRounds.push({ round, program, examples: roundExamples });
    found.observed.forEach((value, at) => assert.equal(stable(value), stable(expected[at]),
      `${task.name}: the discovered program agrees with the examples it was given`));
    if (listFamily) {
      status = 'run-only';
      rounds.push({ round, program: sourceOf(program), size: program.length, candidates: found.candidates });
      break;
    }
    const pair = pairFor(program);
    const affine = certificateOf(pair, task.inputs);
    const check = solverCheck(task, pair, tools, globals['solver-ms'], semantic, profile);
    lastCheck = check;
    const answers = check.answers.map(answer => answer.answer);
    const refutation = check.answers.find(answer => answer.status === 'refuted');
    if (refutation) {
      const witness = refutation.witness;
      assert.ok(witness, `${task.name}: a replayed solver refutation has its full witness`);
      const counterexample = [...Array(task.inputs)].map((_, slot) => {
        assert.ok(witness[`s${slot}`] !== undefined, `${task.name}: witness supplies slot ${slot}`);
        return BigInt(witness[`s${slot}`]);
      });
      const referenceOutput = referenceValueAt(counterexample);
      const candidateOutput = programValue(task, program, counterexample).value;
      assert.notEqual(stable(candidateOutput), stable(referenceOutput),
        `${task.name}: the replayed counterexample separates candidate and reference in the lane evaluator`);
      refusedPair ??= pair;
      examples.push(counterexample);
      expected.push(referenceOutput);
      rounds.push({ round, program: sourceOf(program), size: program.length, candidates: found.candidates,
        solver: answers, verdict: 'refuted', witness,
        counterexample: { inputs: counterexample.map(decimal), candidate: shown(candidateOutput),
          reference: shown(referenceOutput) }, reason: refutation.reason, query: check.query,
        answers: check.answers, encoder_checks: check.encoder_checks });
      program = null;
      continue;
    }
    const bothUnsat = check.answers.every(answer => answer.answer === 'unsat');
    if (bothUnsat && affine.certifiable) {
      status = 'proved';
      certificate = { theorem: `${NAMESPACE}.${task.name}_certified`, form: affine.form,
        kernel: 'checked by verification/foundation/check.mjs, not by this lane' };
      record.lean = { pair, refused: refusedPair };
    } else if (bothUnsat) {
      status = 'unproven';
      certificate = { reason: affine.reason, solver: 'both solvers answered UNSAT; UNSAT never certifies' };
    } else {
      status = 'inconclusive';
    }
    rounds.push({ round, program: sourceOf(program), size: program.length, candidates: found.candidates,
      solver: answers, verdict: status, affine: { certifiable: affine.certifiable, reason: affine.reason ?? null },
      query: check.query, answers: check.answers, encoder_checks: check.encoder_checks });
    break;
  }
  if (refusedPair) record.lean = { ...record.lean, refused: refusedPair };
  record.status = status ?? 'inconclusive';
  record.certificate = certificate;
  record.rounds = rounds;
  record.solver_check = lastCheck;
  record.examples.counterexamples = rounds.filter(round => round.verdict === 'refuted').length;
  record.search.measurement = measurementSince(searchStarted, searchCpuStart);

  // The plain enumerative baseline on the same examples and budget.
  const plain = runBaseline();

  const grid = boundaryPoints(task);
  if (program === null) {
    record.heldout = { status: 'not-run', passed: 0, total: 0, entries: [], reason: 'no program was emitted' };
    record.readmission = null;
  } else {
    record.search.program = sourceOf(program);
    record.search.size = program.length;

    // Held-out examples are checked only on the final CEGIS program.
    const heldout = task.heldout.map(example => {
      const observed = modelValue(task, program, example.inputs, spec.maxStack);
      return { inputs: example.inputs.map(decimal), expected: shown(example.output), observed: shown(observed),
        agrees: stable(observed) === stable(example.output) };
    });
    const heldoutPassed = heldout.filter(entry => entry.agrees).length;
    record.heldout = { status: heldoutPassed === heldout.length ? 'pass' : 'fail',
      passed: heldoutPassed, total: heldout.length, entries: heldout };

    for (const candidate of candidateRounds) {
      const finalCandidate = program !== null && candidate === candidateRounds.at(-1);
      const points = uniquePoints([...candidate.examples, ...(finalCandidate ? task.heldout.map(example => example.inputs) : []), ...grid]);
      const readmission = readmitProgram(binary, path.join(directory, `${task.name}-round-${candidate.round}`),
        task, candidate.program, points, spec.maxStack);
      const round = rounds.find(entry => entry.round === candidate.round);
      assert.ok(round, `${task.name}: a reported candidate has a round record`);
      round.readmission = readmission;
      if (finalCandidate) record.readmission = readmission;
    }
  }

  // The plain enumerative baseline uses the initial examples, with no counterexample refinement.
  if (plain.program) {
    const baselinePoints = uniquePoints([...task.train.map(example => example.inputs), ...grid]);
    record.baseline.readmission = readmitProgram(binary, path.join(directory, `${task.name}-baseline`),
      task, plain.program, baselinePoints, spec.maxStack);
  }

  // A declared control bound must solve and certify the task the narrower bound exhausts.
  if (task.controlBound !== null) {
    const control = discover(task, task.train.map(example => example.inputs.slice()),
      task.train.map(example => example.output), { ...spec, bound: task.controlBound }, globals['max-candidates']);
    const controlProgram = control.program ?? null;
    assert.equal(record.status, 'exhausted', `${task.name}: a control-bound task is the narrow-bound negative`);
    if (!controlProgram) {
      assert.fail(`${task.name}: the control bound must find a program; ${JSON.stringify({
        exhausted: control.exhausted, candidates: control.candidates, sizes: control.sizes,
      })}`);
    }
    const controlPair = pairFor(controlProgram);
    const controlAffine = certificateOf(controlPair, task.inputs);
    assert.ok(controlAffine.certifiable, `${task.name}: the control program must have the reference's affine form`);
    const controlPoints = uniquePoints([...task.train.map(example => example.inputs), ...grid]);
    const controlOutputs = controlPoints.map(inputs => modelValue(task, controlProgram, inputs, spec.maxStack));
    controlPoints.forEach((inputs, at) => assert.equal(stable(controlOutputs[at]), stable(referenceValueAt(inputs)),
      `${task.name}: the control-bound program agrees with its reference at ${inputs.map(decimal).join(',')}`));
    const controlName = `${task.name}Control`;
    record.controls = { control_bound: { bound: task.controlBound, program: sourceOf(controlProgram),
      size: controlProgram.length, candidates: control.candidates, sizes: control.sizes,
      affine: controlAffine.form, pair: controlPair,
      certificate: { theorem: `${NAMESPACE}.${controlName}_certified`, status: 'emitted; kernel check is separate' },
      readmission: readmitProgram(binary, path.join(directory, `${task.name}-control-bound`),
        task, controlProgram, controlPoints, spec.maxStack) } };
  }
  return record;
}

/* ---------- The run ---------- */

function main() {
  const flags = options();
  const started = process.hrtime.bigint();
  const cpuStart = process.cpuUsage();
  const tools = { bitwuzla: flags['--bitwuzla'], z3: flags['--z3'], time: flags['--time'] };
  const binary = flags['--noble'];
  fs.mkdirSync(flags['--scratch'], { recursive: true });
  const scratch = fs.realpathSync(flags['--scratch']);
  assert.ok(!scratch.startsWith('/tmp/') && !scratch.startsWith('/var/tmp/'), 'resolved scratch must not use a shared temporary directory');
  const directory = fs.mkdtempSync(path.join(scratch, 'noble-synthesis-'));
  const git = argv => {
    const result = spawnSync('/run/current-system/sw/bin/git', argv, { cwd: ROOT, encoding: 'utf8' });
    assert.equal(result.status, 0, `git ${argv.join(' ')}: ${result.stderr}`);
    return result.stdout.trim();
  };
  const source = { commit: git(['rev-parse', 'HEAD']), tree: git(['rev-parse', 'HEAD^{tree}']),
    clean: git(['status', '--porcelain', '--untracked-files=all']) === '' };
  assert.ok(source.clean, 'the synthesis lane must run from a clean committed source tree');
  const manifestBytes = read(MANIFEST);
  const { globals, tasks } = parseManifest(manifestBytes.toString('utf8'));
  const irText = read(IR).toString('utf8');
  const semantic = irText.match(/pub const SEMANTIC_REVISION: &str = "([^"]+)";/)?.[1];
  const profile = irText.match(/Self::I64 => "([^"]+)",/)?.[1];
  assert.ok(semantic && profile, 'the graph IR names its semantic revision and I64 profile');
  const i64Words = wordsOf(globals, 'i64');
  const listWords = wordsOf(globals, 'list');
  try {
    const results = tasks.map(task => runTask(task, { globals, tools, binary, directory, i64Words, listWords,
      semantic, profile }));
    for (const result of results) {
      if (result.search.program !== null) {
        assert.equal(result.heldout.total, result.examples.heldout,
          `${result.name}: the final program is run on every held-out example`);
      } else {
        assert.equal(result.heldout.total, 0, `${result.name}: an unsolved task runs no held-out program`);
      }
    }
    const lean = emitLean(results);
    if (flags['--lean']) fs.writeFileSync(flags['--lean'], lean.text, { flag: 'wx' });
    let freshness = null;
    if (flags['--check-lean']) {
      const committed = fs.readFileSync(path.resolve(flags['--check-lean']));
      freshness = { committed: path.relative(ROOT, path.resolve(flags['--check-lean'])), bytes: committed.length,
        committed_sha256: sha(committed), emission_sha256: sha(lean.text), matches: committed.equals(Buffer.from(lean.text)) };
      assert.ok(freshness.matches, 'the committed Lean module differs from this emission; regenerate and commit it');
    }
    const cpu = process.cpuUsage(cpuStart);
    const solverRuns = results.flatMap(result => result.rounds.flatMap(round => round.answers ?? []));
    const solverCpu = (field) => solverRuns.reduce((sum, answer) =>
      sum + (Number.isFinite(answer[field]) ? answer[field] : 0), 0);
    const nodePeakRss = process.resourceUsage().maxRSS;
    const solverPeakRss = Math.max(0, ...solverRuns.map(answer => answer.peak_rss_kb).filter(Number.isFinite));
    const totalUserCpu = cpu.user * 1000 + solverCpu('cpu_user_ns');
    const totalSystemCpu = cpu.system * 1000 + solverCpu('cpu_system_ns');
    const receipt = {
      schema: 'noble-synthesis-receipt/v1',
      claim: 'bounded typed synthesis with counterexample-guided refinement over the frozen benchmark set; discovery, refinement, kernel-certificate emission and command-line re-admission kept separate',
      source: { ...source, lane_sha256: sha(read(LANE)), qfbv_sha256: sha(read(QFBV)),
        manifest_sha256: sha(manifestBytes), semantic_revision: semantic, profile, encoder: ENCODER },
      cli_sha256: sha(fs.readFileSync(fs.realpathSync(binary))),
      grammar: { i64: { name: I64_GRAMMAR.name, words: I64_WORDS, constants: globals.consts.map(decimal), wiring: 'dup and swap only; no arithmetic beyond + - * =' },
        list: { name: LIST_GRAMMAR.name, words: LIST_WORDS, constants: globals['list-consts'].map(decimal) } },
      budget: { seed: `0x${hex16(globals.seed)}`, size: globals.size, max_stack: globals['max-stack'],
        max_candidates: globals['max-candidates'], max_rounds: globals['max-rounds'], solver_ms: globals['solver-ms'],
        list_max_stack: globals['list-max-stack'] },
      tools: Object.fromEntries(Object.entries(tools).map(([name, tool]) =>
        [name, { path: tool, sha256: sha(fs.readFileSync(fs.realpathSync(tool))) }])),
      measurement: { wall_ns: Number(process.hrtime.bigint() - started), cpu_user_ns: totalUserCpu,
        cpu_system_ns: totalSystemCpu, node_cpu_user_ns: cpu.user * 1000,
        node_cpu_system_ns: cpu.system * 1000, solver_cpu_user_ns: solverCpu('cpu_user_ns'),
        solver_cpu_system_ns: solverCpu('cpu_system_ns'), process_peak_rss_kb: nodePeakRss,
        solver_peak_rss_kb: solverPeakRss, peak_rss_kb: Math.max(nodePeakRss, solverPeakRss),
        solver_runs: solverRuns.length },
      lean: { path: LEAN, emission_sha256: sha(lean.text), theorems: lean.theorems, freshness,
        kernel: 'the emitted theorems are proved only once verification/foundation/check.mjs accepts them; this lane does not check the kernel' },
      tasks: results,
      summary: Object.fromEntries(results.map(result => [result.name, result.status])),
      non_claims: [
        'a status is about the declared model graphs, the declared examples and the declared bounds; nothing outside them is claimed',
        'proved means a generated bv_omega theorem over the pair was emitted; the kernel is checked by the foundation lane, not here',
        'solver UNSAT is recorded and never certifies; every solver is untrusted',
        'the list family is run only: no solver and no certificate covers it',
        're-admission shows the selected CLI accepts the program and its evaluators agree on the listed points; that is finite evidence, and extraction preservation (NG-PO-A) stays open',
        'timings and peak memory are one run on one host, not a performance claim',
      ],
    };
    fs.writeFileSync(flags['--output'], `${JSON.stringify(receipt, (_, value) => (typeof value === 'bigint' ? `0x${hex16(value)}` : value), 2)}\n`, { flag: 'wx' });
    process.stdout.write(`${JSON.stringify({ output: flags['--output'], clean: source.clean, summary: receipt.summary,
      lean: { sha256: receipt.lean.emission_sha256, theorems: lean.theorems.length, freshness },
      measurement: receipt.measurement })}\n`);
  } finally {
    fs.rmSync(directory, { recursive: true, force: true });
  }
}

main();
