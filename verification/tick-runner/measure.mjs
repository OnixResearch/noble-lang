#!/usr/bin/env node
// Finite, bounded independent model replay for noble-tick-export/v1. Not a benchmark.
import { createHash } from 'node:crypto';
import { readFileSync, statSync } from 'node:fs';

const MAX_BYTES = 16 * 1024 * 1024;
const MAX_EVENTS = 65536;
const MAX_NODES = 4096;
const MAX_WORDS = 256;
const MAX_GRAPHS = 16;
const MAX_TICKS = 4096;
const sha256 = bytes => createHash('sha256').update(bytes).digest('hex');
const fail = message => { throw new Error(message); };
const insist = (test, message) => { if (!test) fail(message); };
const obj = value => value !== null && typeof value === 'object' && !Array.isArray(value);
const uint = (value, cap, label) => {
  insist(Number.isSafeInteger(value) && value >= 0 && value <= cap, `${label}: invalid unsigned integer`);
  return value;
};
const text = (value, label) => {
  insist(typeof value === 'string' && value.length > 0 && value.length <= 256, `${label}: invalid string`);
  return value;
};
const hex = (value, label, lengths = [64]) => {
  insist(typeof value === 'string' && lengths.includes(value.length) && /^[0-9a-f]+$/.test(value), `${label}: invalid digest`);
  return value;
};
const word = value => {
  if (typeof value !== 'string' || !/^(0|-?[1-9][0-9]*)$/.test(value) || value.length > 20) return null;
  const n = BigInt(value);
  return n >= -(1n << 63n) && n < (1n << 63n) ? n : null;
};
const words = (value, count) => Array.isArray(value) && value.length === count && value.every(v => word(v) !== null);
const canonical = value => Array.isArray(value) ? value.map(canonical) :
  obj(value) ? Object.fromEntries(Object.keys(value).sort().map(key => [key, canonical(value[key])])) : value;
const same = (a, b) => JSON.stringify(canonical(a)) === JSON.stringify(canonical(b));
const next = state => ({
  graph_epoch: state.graph_epoch,
  control_epoch: state.control_epoch,
  next_tick: state.next_tick,
  graph_key: state.graph_key,
  state: [...state.state],
  controls: [...state.controls],
  output: state.output === null ? null : [...state.output],
});

function graphEntry(g, limits, keys) {
  insist(obj(g), 'graph: expected object');
  const key = text(g.key, 'graph.key');
  insist(!keys.has(key), `graph ${key}: duplicate key`);
  keys.add(key);
  text(g.graph_logical_id, `${key}.graph_logical_id`);
  text(g.selected_schedule_id, `${key}.selected_schedule_id`);
  hex(g.export_sha256, `${key}.export_sha256`);
  const p = g.partition;
  insist(obj(p), `${key}: missing partition`);
  for (const name of ['state_words', 'input_words', 'control_words', 'output_words']) {
    uint(p[name], limits.max_words, `${key}.${name}`);
  }
  insist(p.state_words > 0 && p.input_words > 0 && p.output_words > 0, `${key}: empty state/input/output partition`);
  const inputCount = p.state_words + p.input_words + p.control_words;
  const outputCount = p.state_words + p.output_words;
  insist(inputCount <= limits.max_words && outputCount <= limits.max_words, `${key}: aggregate interface too wide`);
  insist(Array.isArray(g.nodes) && g.nodes.length >= inputCount && g.nodes.length <= limits.max_nodes, `${key}: node count`);
  insist(g.nodes.length <= limits.max_work_per_tick, `${key}: graph exceeds admitted work per tick`);
  insist(Array.isArray(g.outputs) && g.outputs.length === outputCount, `${key}: output count`);
  for (let i = 0; i < g.nodes.length; i++) {
    const node = g.nodes[i];
    insist(obj(node), `${key}: malformed node ${i}`);
    if (i < inputCount) {
      insist(node.op === 'input' && node.index === i && Object.keys(node).length === 2, `${key}: input slots out of order`);
    } else if (node.op === 'const') {
      insist(Object.keys(node).length === 2 && word(node.value) !== null, `${key}: malformed I64 constant ${i}`);
    } else {
      insist(['add', 'sub', 'mul'].includes(node.op) && Object.keys(node).length === 3, `${key}: unsupported op ${i}`);
      uint(node.lhs, i - 1, `${key}.lhs`);
      uint(node.rhs, i - 1, `${key}.rhs`);
    }
  }
  for (const index of g.outputs) uint(index, g.nodes.length - 1, `${key}.output`);
  const canonical = { partition: p, nodes: g.nodes, outputs: g.outputs };
  insist(sha256(Buffer.from(JSON.stringify(canonical), 'utf8')) === g.export_sha256, `${key}: export digest mismatch`);
  return g;
}

function evaluate(g, state, input, controls) {
  const values = [...state, ...input, ...controls].map(word);
  const inputCount = values.length;
  for (let i = inputCount; i < g.nodes.length; i++) {
    const n = g.nodes[i];
    if (n.op === 'const') values.push(word(n.value));
    else {
      const a = values[n.lhs], b = values[n.rhs];
      values.push(BigInt.asIntN(64, n.op === 'add' ? a + b : n.op === 'sub' ? a - b : a * b));
    }
  }
  const output = g.outputs.map(id => values[id].toString());
  return [output.slice(0, g.partition.state_words), output.slice(g.partition.state_words)];
}

function limitsFrom(h) {
  const p = h.limits;
  insist(obj(p), 'missing limits');
  const caps = {
    max_graphs: MAX_GRAPHS, max_nodes: MAX_NODES, max_words: MAX_WORDS,
    max_events: MAX_EVENTS, max_ticks: MAX_TICKS,
    max_work_per_tick: MAX_NODES, max_trace_bytes: MAX_BYTES,
  };
  for (const [key, cap] of Object.entries(caps)) {
    uint(p[key], cap, `limits.${key}`);
    insist(p[key] > 0, `limits.${key}: zero cap`);
  }
  return p;
}

function expected(row, state, graphs, limits) {
  const current = graphs.get(state.graph_key);
  insist(obj(row) && ['tick', 'update_controls', 'reset'].includes(row.op), 'unsupported operation');
  uint(row.expected_graph_epoch, Number.MAX_SAFE_INTEGER, 'expected_graph_epoch');
  uint(row.expected_control_epoch, Number.MAX_SAFE_INTEGER, 'expected_control_epoch');
  if (row.expected_graph_epoch !== state.graph_epoch || row.expected_control_epoch !== state.control_epoch) return 'stale';
  if (row.op === 'tick') {
    uint(row.expected_tick, Number.MAX_SAFE_INTEGER, 'expected_tick');
    if (row.expected_tick !== state.next_tick) return 'stale';
    if (!words(row.input, current.partition.input_words)) return 'malformed';
    if (row.cancel_before_commit !== undefined && row.cancel_before_commit !== true) return 'malformed';
    if (row.work_budget !== undefined && (!Number.isSafeInteger(row.work_budget) || row.work_budget < 0)) return 'malformed';
    if (state.next_tick >= limits.max_ticks) return 'exhausted';
    if (row.cancel_before_commit) return 'cancelled';
    if (current.nodes.length > limits.max_work_per_tick ||
        (row.work_budget !== undefined && row.work_budget < current.nodes.length)) return 'budget';
    const [newState, output] = evaluate(current, state.state, row.input, state.controls);
    state.state = newState;
    state.output = output;
    state.next_tick++;
    return 'ok';
  }
  if (row.op === 'update_controls') {
    if (!words(row.controls, current.partition.control_words)) return 'malformed';
    if (state.control_epoch === Number.MAX_SAFE_INTEGER) return 'terminal';
    state.controls = [...row.controls];
    state.control_epoch++;
    return 'ok';
  }
  const replacement = graphs.get(row.graph_key);
  if (!replacement || !words(row.state, replacement.partition.state_words) ||
      !words(row.controls, replacement.partition.control_words)) return 'malformed';
  if (state.graph_epoch === Number.MAX_SAFE_INTEGER || state.control_epoch === Number.MAX_SAFE_INTEGER) return 'terminal';
  state.graph_key = row.graph_key;
  state.state = [...row.state];
  state.controls = [...row.controls];
  state.output = null;
  state.next_tick = 0;
  state.graph_epoch++;
  state.control_epoch++;
  return 'ok';
}

function run(path) {
  const size = statSync(path).size;
  insist(size > 0 && size <= MAX_BYTES, 'trace exceeds hard byte cap');
  const bytes = readFileSync(path);
  insist(bytes.length === size && bytes[bytes.length - 1] === 10, 'trace must be complete and newline-terminated');
  const raw = bytes.toString('utf8');
  insist(Buffer.from(raw, 'utf8').equals(bytes), 'trace must be UTF-8');
  const lines = raw.slice(0, -1).split('\n');
  insist(lines.length > 1 && lines.length <= MAX_EVENTS + 1 && lines.every(line => line.length > 0),
    'operation count or blank line out of bounds');
  const h = JSON.parse(lines[0]);
  insist(obj(h) && h.schema === 'noble-tick-export/v1' && h.profile === 'noble-tick-i64/v1', 'wrong trace profile');
  hex(h.source_commit, 'source_commit', [40, 64]);
  hex(h.source_tree, 'source_tree', [40, 64]);
  text(h.rustc, 'rustc');
  hex(h.producer_sha256, 'producer_sha256');
  hex(h.runner_artifact_sha256, 'runner_artifact_sha256');
  const limits = limitsFrom(h);
  insist(size <= limits.max_trace_bytes && lines.length - 1 <= limits.max_events, 'declared trace bounds exceeded');
  insist(Array.isArray(h.graphs) && h.graphs.length > 0 && h.graphs.length <= limits.max_graphs, 'graph count');
  const keys = new Set();
  const graphs = new Map(h.graphs.map(g => {
    const entry = graphEntry(g, limits, keys);
    return [entry.key, entry];
  }));
  insist(obj(h.initial) && graphs.has(h.initial.graph_key), 'missing initial graph');
  const first = graphs.get(h.initial.graph_key);
  insist(words(h.initial.state, first.partition.state_words) &&
         words(h.initial.controls, first.partition.control_words), 'invalid initial tuple');
  const state = {
    graph_epoch: 0, control_epoch: 0, next_tick: 0, graph_key: h.initial.graph_key,
    state: [...h.initial.state], controls: [...h.initial.controls], output: null,
  };
  const counts = { tick: 0, update_controls: 0, reset: 0, refused: 0 };
  for (let seq = 0; seq < lines.length - 1; seq++) {
    const row = JSON.parse(lines[seq + 1]);
    insist(obj(row) && row.seq === seq, `row ${seq}: missing/duplicate sequence`);
    const before = next(state);
    const result = expected(row, state, graphs, limits);
    const outcome = result === 'ok' ? { kind: 'ok' } : { kind: 'refused', class: result };
    insist(same(row.outcome, outcome), `row ${seq}: outcome mismatch: expected ${JSON.stringify(outcome)}`);
    if (result !== 'ok') {
      insist(same(before, next(state)), `row ${seq}: model mutated on refusal`);
      counts.refused++;
    }
    insist(same(row.observed, next(state)), `row ${seq}: complete state/output observation mismatch`);
    counts[row.op]++;
  }
  return {
    schema: 'noble-tick-replay/v1', profile: h.profile,
    source_commit: h.source_commit, source_tree: h.source_tree,
    producer_sha256: h.producer_sha256, runner_artifact_sha256: h.runner_artifact_sha256,
    raw_trace_sha256: sha256(bytes), raw_trace_bytes: size,
    graphs: h.graphs.map(g => ({ key: g.key, graph_logical_id: g.graph_logical_id,
      selected_schedule_id: g.selected_schedule_id, export_sha256: g.export_sha256 })),
    limits, counts, final_observation: next(state),
    comparison: 'finite independent I64 model replay only; not a benchmark or refinement theorem',
  };
}

try {
  insist(process.argv.length === 3, 'usage: node measure.mjs <trace.ndjson>');
  console.log(JSON.stringify(run(process.argv[2]), null, 2));
} catch (error) {
  console.error(`tick replay refused: ${error.message}`);
  process.exitCode = 1;
}
