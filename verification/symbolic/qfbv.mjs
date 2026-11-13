#!/usr/bin/env node
/**
 * The `noble-symbolic-qfbv/v1` query core: strict graph admission, the lane's own evaluator,
 * the shared-DAG QF_BV encoder, the encoder checks that do not trust the formula, and the
 * measured, never-trusted solver runs.
 *
 * This module is the part of issue #56's untrusted-solver lane that issue #116's bounded
 * synthesis lane reuses: the evaluator decides a candidate's meaning, and the encoder plus the
 * solvers decide whether two admitted graphs can be told apart. Importing it has no side effects;
 * the lane's `main` stays in `solver.mjs` (and in `verification/synthesis/cegis.mjs`).
 *
 * Nothing here is trusted, and nothing here decides a verdict: an answer is only an input to
 * `outcome`, which the caller binds to the bytes and revisions it read.
 */
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';

/** The encoder's version: it is part of every query's binding. */
export const ENCODER = 'noble-symbolic-qfbv/v1';
/** Default admission and solver budgets for one query. */
export const BUDGET = Object.freeze({ nodes: 64, slots: 16, formula_bytes: 65536, solver_ms: 10000 });
/** The solvers the lane runs; neither is trusted. */
export const SOLVERS = ['bitwuzla', 'z3'];
export const MASK = (1n << 64n) - 1n;
export const sha = bytes => createHash('sha256').update(bytes).digest('hex');
export const hex16 = bits => bits.toString(16).padStart(16, '0');
export const elapsed = started => Number(process.hrtime.bigint() - started);

export class Unsupported extends Error {}

/** Types every node, refusing reads of later nodes and arithmetic on Bool, within the budget. */
export function typeGraph(graph, budget) {
  if (graph.length > budget.nodes) throw new Unsupported(`graph exceeds ${budget.nodes} nodes`);
  const types = [];
  graph.forEach((node, index) => {
    if (node.op === 'input' && node.slot >= budget.slots) throw new Unsupported(`slot ${node.slot} exceeds the budget`);
    if (node.a !== undefined) {
      for (const ref of [node.a, node.b]) {
        if (!(ref < index)) throw new Unsupported(`node ${index} reads node ${ref}, which is not earlier`);
        if (types[ref] !== 'i64') throw new Unsupported(`node ${index} reads a Bool`);
      }
    }
    types.push(node.op === 'eq' ? 'bool' : 'i64');
  });
  return types;
}

export function admit(pair, budget = BUDGET) {
  const types = { left: typeGraph(pair.left, budget), right: typeGraph(pair.right, budget) };
  for (const out of pair.outputs) {
    if (!(out < pair.left.length && out < pair.right.length)) throw new Unsupported(`output ${out} is not a node of both graphs`);
  }
  return types;
}

/* ---------- Independent replay: the graph semantics, evaluated directly ---------- */

/** `PackedSchedule.evalStep`: wrapping I64 arithmetic, `sub a b` is `a - b`, `eq` yields Bool. */
export function evaluate(graph, slots) {
  const values = [];
  for (const node of graph) {
    const x = values[node.a]?.value;
    const y = values[node.b]?.value;
    if (node.op === 'input') values.push({ type: 'i64', value: slots.get(node.slot) ?? 0n });
    else if (node.op === 'constant') values.push({ type: 'i64', value: node.bits });
    else if (node.op === 'add') values.push({ type: 'i64', value: (x + y) & MASK });
    else if (node.op === 'sub') values.push({ type: 'i64', value: (x - y) & MASK });
    else if (node.op === 'mul') values.push({ type: 'i64', value: (x * y) & MASK });
    else values.push({ type: 'bool', value: x === y });
  }
  return values;
}

export function refutes(pair, slots) {
  const left = evaluate(pair.left, slots);
  const right = evaluate(pair.right, slots);
  return pair.outputs.some(out => left[out].type !== right[out].type || left[out].value !== right[out].value);
}

/* ---------- Encoding ---------- */

export function cone(graph, outputs) {
  const seen = new Set();
  const pending = [...outputs];
  while (pending.length) {
    const index = pending.pop();
    if (seen.has(index)) continue;
    seen.add(index);
    if (graph[index].a !== undefined) pending.push(graph[index].a, graph[index].b);
  }
  return seen;
}

/**
 * One shared-DAG query: each node is one `define-fun`, so shared nodes are never expanded. With
 * `slice`, only the nodes the listed outputs depend on are defined; nothing else is read, so the
 * query and its models are unchanged. `fault` injects one encoder defect for the controls.
 * `assumptions` are asserted before the claim: they restrict the inputs a counterexample may
 * start from (the synthesis lane's declared input preconditions), and they are part of the
 * query's text, so they are part of its SHA-256.
 */
export function encode(pair, types, { slice = true, fault = null, budget = BUDGET, assumptions = [] } = {}) {
  const keep = { L: slice ? cone(pair.left, pair.outputs) : null, R: slice ? cone(pair.right, pair.outputs) : null };
  const slots = new Set();
  const defines = [];
  const defined = [];
  for (const [side, graph, sorts] of [['L', pair.left, types.left], ['R', pair.right, types.right]]) {
    graph.forEach((node, index) => {
      if (keep[side] && !keep[side].has(index)) return;
      const a = `${side}${node.a}`;
      const b = `${side}${node.b}`;
      let body;
      if (node.op === 'input') {
        slots.add(node.slot);
        body = `s${node.slot}`;
      } else if (node.op === 'constant') body = `#x${hex16(fault === 'constant-bit' ? node.bits ^ 1n : node.bits)}`;
      else if (node.op === 'add') body = `(bvadd ${a} ${b})`;
      else if (node.op === 'sub') body = fault === 'sub-order' ? `(bvsub ${b} ${a})` : `(bvsub ${a} ${b})`;
      else if (node.op === 'mul') body = `(${fault === 'mul-as-add' ? 'bvadd' : 'bvmul'} ${a} ${b})`;
      else body = `(= ${a} ${b})`;
      defines.push(`(define-fun ${side}${index} () ${sorts[index] === 'bool' ? 'Bool' : '(_ BitVec 64)'} ${body})`);
      defined.push({ side, index });
    });
  }
  const listed = fault === 'drop-output' ? pair.outputs.slice(0, -1) : pair.outputs;
  const differs = listed.map(out => (types.left[out] === types.right[out] ? `(not (= L${out} R${out}))` : 'true'));
  const names = [...slots].sort((x, y) => x - y).map(slot => `s${slot}`);
  const claim = differs.length === 0 ? 'false' : differs.length === 1 ? differs[0] : `(or ${differs.join(' ')})`;
  const prefix = ['(set-logic QF_BV)', '(set-option :produce-models true)',
    ...names.map(name => `(declare-const ${name} (_ BitVec 64))`), ...defines,
    ...assumptions.map(assumption => `(assert ${assumption})`)];
  const text = [...prefix, `(assert ${claim})`,
    '(check-sat)', ...(names.length ? [`(get-value (${names.join(' ')}))`] : []), '(exit)', ''].join('\n');
  if (Buffer.byteLength(text) > budget.formula_bytes) throw new Unsupported(`formula exceeds ${budget.formula_bytes} bytes`);
  return { text, names, prefix, defined, assumptions: assumptions.length, nodes: defines.length, bytes: Buffer.byteLength(text), sha256: sha(text) };
}

/* ---------- Encoder checks that do not trust the formula ---------- */

/** Fixed points: all zero, all one bits, the sign bit, and distinct values per slot. */
export function points(names) {
  const each = value => new Map(names.map(name => [Number(name.slice(1)), value]));
  const distinct = new Map(names.map((name, at) => [Number(name.slice(1)), (0x0123456789abcdefn + BigInt(at) * 0x1111111111111111n) & MASK]));
  return [each(0n), each(MASK), each(1n << 63n), distinct];
}

/** Pins the inputs and every defined node to this lane's evaluator; a faithful encoding stays SAT. */
export function simulate(tools, solver, pair, query, slots) {
  const values = { L: evaluate(pair.left, slots), R: evaluate(pair.right, slots) };
  const literal = value => (value.type === 'bool' ? String(value.value) : `#x${hex16(value.value)}`);
  const pins = [...query.names.map(name => `(= ${name} #x${hex16(slots.get(Number(name.slice(1))) ?? 0n)})`),
    ...query.defined.map(({ side, index }) => `(= ${side}${index} ${literal(values[side][index])})`)];
  const text = [...query.prefix, `(assert (and ${pins.join(' ')}))`, '(check-sat)', '(exit)', ''].join('\n');
  return solve(tools, solver, { text, names: [] }).answer;
}

/**
 * The outputs a query's claim compares, read back from its text: the assertion immediately
 * before `(check-sat)`. A query with declared `assumptions` asserts them first, so the claim is
 * the last assertion, never the first.
 */
export function claimedOutputs(text) {
  const lines = text.split('\n');
  const check = lines.indexOf('(check-sat)');
  const line = lines.slice(0, check < 0 ? lines.length : check).filter(candidate => candidate.startsWith('(assert ')).at(-1);
  const compared = [...line.matchAll(/\(not \(= L(\d+) R(\d+)\)\)/g)].map(([, left, right]) => {
    assert.equal(left, right, 'a claim compares the same output index on both sides');
    return Number(left);
  });
  return [...compared, ...[...line.matchAll(/\btrue\b/g)].map(() => null)];
}

/**
 * `simulationPoints` lets a caller that restricts the inputs (a declared precondition) supply
 * points inside that restriction; the default is the four fixed points. A faithful encoding of a
 * restricted query is expected to stay satisfiable at every point the caller supplies.
 */
export function encoderChecks(tools, pair, types, query, witnesses, simulationPoints = points(query.names)) {
  const simulation = [...simulationPoints, ...witnesses].map(slots => {
    const answers = SOLVERS.map(solver => simulate(tools, solver, pair, query, slots));
    return { point: Object.fromEntries([...slots].map(([slot, bits]) => [`s${slot}`, `0x${hex16(bits)}`])), answers };
  });
  const compared = claimedOutputs(query.text);
  const expectedOutputs = pair.outputs.map(out => (types.left[out] === types.right[out] ? out : null));
  const claimFaithful = JSON.stringify([...compared].sort()) === JSON.stringify([...expectedOutputs].sort());
  const detected_by = [
    ...(simulation.some(entry => entry.answers.includes('unsat')) ? ['concrete simulation'] : []),
    ...(claimFaithful ? [] : ['claim check']),
  ];
  return { simulation_points: simulation.length, claim_outputs: compared, detected_by, simulation };
}

/* ---------- Solvers: run, measured, never trusted ---------- */

export function parseModel(output, names) {
  const slots = new Map();
  for (const [, name, literal] of output.matchAll(/\(\s*(s\d+)\s+#(x[0-9a-fA-F]+|b[01]+)\s*\)/g)) {
    const width = literal[0] === 'x' ? (literal.length - 1) * 4 : literal.length - 1;
    const slot = Number(name.slice(1));
    if (width !== 64 || slots.has(slot)) return { rejected: `malformed value for ${name}` };
    slots.set(slot, BigInt(`0${literal}`));
  }
  const got = [...slots.keys()].sort((x, y) => x - y).map(slot => `s${slot}`);
  if (got.join(',') !== names.join(',')) return { rejected: `model names [${got.join(',')}] are not [${names.join(',')}]` };
  return { slots };
}

export function solve(tools, solver, query, { timeoutMs = BUDGET.solver_ms, watchdogMs = BUDGET.solver_ms + 5000 } = {}) {
  const args = solver === 'bitwuzla' ? ['--produce-models', '-t', String(timeoutMs)] : ['-in', '-smt2', `-t:${timeoutMs}`];
  const started = process.hrtime.bigint();
  const run = spawnSync(tools.time, ['-f', 'noble-time rss_kb=%M user_s=%U system_s=%S', tools[solver], ...args],
    { input: query.text, encoding: 'utf8', timeout: watchdogMs, maxBuffer: 1 << 20, env: {} });
  const timed = (run.stderr ?? '').match(/noble-time rss_kb=(\d+) user_s=([\d.]+) system_s=([\d.]+)/);
  const measured = { solver, wall_ns: elapsed(started), peak_rss_kb: Number(timed?.[1] ?? NaN),
    cpu_user_ns: timed ? Number(timed[2]) * 1e9 : null, cpu_system_ns: timed ? Number(timed[3]) * 1e9 : null };
  if (run.error?.code === 'ETIMEDOUT' || run.signal) return { ...measured, answer: 'timeout' };
  const [first, ...rest] = (run.stdout ?? '').split('\n');
  if (first === 'unsat' || first === 'unknown') return { ...measured, answer: first };
  if (first !== 'sat') return { ...measured, answer: 'error', detail: `${run.stdout}${run.stderr}`.slice(0, 200) };
  return { ...measured, answer: 'sat', ...(query.names.length ? parseModel(rest.join('\n'), query.names) : { slots: new Map() }) };
}

/* ---------- Outcomes ---------- */

/** The status one solver answer supports for one claim, given what the query was bound to. */
export function outcome(pair, query, expected, answer, certificate) {
  if (query.encoder !== expected.encoder || query.semantic_revision !== expected.semantic_revision
    || query.profile !== expected.profile || query.pair_sha256 !== pair.digest) {
    return { status: 'inconclusive', reason: 'query bound to other bytes or revisions' };
  }
  if (certificate) return { status: 'certified', reason: certificate };
  if (answer.answer === 'sat') {
    if (answer.rejected) return { status: 'inconclusive', reason: `model rejected: ${answer.rejected}` };
    const started = process.hrtime.bigint();
    const replays = refutes(pair, answer.slots);
    const replay_ns = elapsed(started);
    const witness = Object.fromEntries([...answer.slots].map(([slot, bits]) => [`s${slot}`, `0x${hex16(bits)}`]));
    if (!replays) return { status: 'inconclusive', reason: 'model does not replay on the original graphs', model: witness, replay_ns };
    return { status: 'refuted', reason: 'model replays on the original graphs', witness, replay_ns };
  }
  if (answer.answer === 'unsat') return { status: 'inconclusive', reason: 'external UNSAT without an accepted proof' };
  return { status: 'inconclusive', reason: `solver ${answer.answer}` };
}
