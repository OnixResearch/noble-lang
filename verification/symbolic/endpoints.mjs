#!/usr/bin/env node
/**
 * Source-backed endpoint pairs for issue #56 (plan item 3).
 *
 * The solver lane's pairs are graphs built directly. This lane starts from Noble source with the
 * fixed interface `I64 I64 -- I64 ! {}`, and gives each side a real Wasm endpoint: the selected
 * CLI compiles the source with its ordinary source compiler and runs it on V8
 * (`noble worker scalar`), reporting the executed value and the module's SHA-256. For the same
 * source, `noble ngraph extract` builds the numeric graph, and `noble ngraph eval` evaluates it
 * with both the reference interpreter and the packed machine.
 *
 * For every candidate of a fixed universe the lane requires
 *   endpoint(side) = packed graph(side) = reference graph(side),
 * so a pair's refutation or agreement is the same whether it is decided on the endpoints or on
 * the graphs. Controls: a mutated endpoint that must change the verdict, and a cross-side graph
 * that must disagree with the other side's endpoint.
 *
 * Usage (pinned Node, env -i, from a clean checkout):
 *   node verification/symbolic/endpoints.mjs --noble CLI --output NEW_FILE
 */
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const LANE = 'verification/symbolic/endpoints.mjs';
const INTERFACE = 'I64 I64 -- I64 ! {}';
const MIN = -(2n ** 63n);
const MAX = 2n ** 63n - 1n;
const sha = bytes => createHash('sha256').update(bytes).digest('hex');

/** The canonical wrapping behaviour of each root, as the model defines `sub` and `add`. */
const SEMANTICS = {
  add: (a, b) => BigInt.asIntN(64, a + b),
  sub: (a, b) => BigInt.asIntN(64, a - b),
  mul: (a, b) => BigInt.asIntN(64, a * b),
};

/** One pair: two roots over `[a, b]` (bottom first), and the expected verdict shape. */
const PAIRS = [
  {
    name: 'sourceCommutativeAdd',
    claim: 'wrapping `+` and `swap +` agree on every input',
    left: { source: '+', root: 'add', operands: 'ordered' },
    right: { source: 'swap +', root: 'add', operands: 'swapped' },
    expect: 'agree',
  },
  {
    name: 'sourceSubOrder',
    claim: '`-` and `swap -` differ whenever the inputs differ',
    left: { source: '-', root: 'sub', operands: 'ordered' },
    right: { source: 'swap -', root: 'sub', operands: 'swapped' },
    expect: 'refute',
  },
];

/** Fixed candidates, including the wrap boundaries and both signs. */
const UNIVERSE = [
  [1n, 2n], [5n, 7n], [-3n, 4n], [0n, 0n],
  [MIN, 1n], [MAX, 1n], [MIN, MIN], [0x0123456789abcdefn, MAX],
];

const value = (root, operands, [a, b]) => SEMANTICS[root](operands === 'ordered' ? a : b, operands === 'ordered' ? b : a);

function options() {
  const argv = process.argv.slice(2);
  assert.equal(argv.length, 4, 'usage: endpoints.mjs --noble CLI --output NEW_FILE');
  const out = {};
  for (let at = 0; at < argv.length; at += 2) out[argv[at]] = argv[at + 1];
  assert.ok(path.isAbsolute(out['--noble']), '--noble must be absolute');
  assert.ok(!fs.existsSync(out['--output']), 'the output must not exist');
  assert.ok(!path.resolve(out['--output']).startsWith(ROOT + path.sep), 'the output must be outside the repository');
  return out;
}

function git(argv) {
  return spawnSync('/run/current-system/sw/bin/git', argv, { cwd: ROOT, encoding: 'utf8' }).stdout.trim();
}

/** One line of JSON from a CLI invocation; the CLI reports one line per observation. */
function cli(binary, argv, { lines = false } = {}) {
  const started = process.hrtime.bigint();
  const run = spawnSync(binary, argv, { encoding: 'utf8', timeout: 120_000, maxBuffer: 1 << 22, env: {} });
  const elapsed_ns = Number(process.hrtime.bigint() - started);
  assert.equal(run.signal, null, `${argv.join(' ')}: killed by ${run.signal}`);
  const rows = (run.stdout ?? '').trim().split('\n').filter(Boolean).map(line => JSON.parse(line));
  assert.ok(rows.length > 0, `${argv.join(' ')}: no report (stderr ${(run.stderr ?? '').slice(0, 200)})`);
  return { rows: lines ? rows : rows[0], status: run.status, elapsed_ns };
}

function writeSource(directory, name, text) {
  const file = path.join(directory, `${name}.noble`);
  fs.writeFileSync(file, `${text}\n`, { flag: 'wx' });
  return { file, sha256: sha(Buffer.from(`${text}\n`)) };
}

/** The source's real Wasm endpoint: compile, then one checked invocation. */
function endpoint(binary, source, [a, b]) {
  const { rows, status, elapsed_ns } = cli(binary, ['worker', 'scalar', source.file,
    '--state', a.toString(), '--delta', b.toString()]);
  assert.equal(status, 0, `worker scalar refused ${source.file}: ${JSON.stringify(rows).slice(0, 300)}`);
  assert.equal(rows.interface, INTERFACE);
  assert.equal(rows.outcome, 'normal');
  assert.equal(rows.guest_requests, 0);
  assert.equal(rows.protected_operations, 0);
  assert.equal(rows.typed_output.type, 'I64');
  return { value: BigInt(rows.typed_output.value), wasm_sha256: rows.module.wasm_sha256,
    source_sha256: rows.module.source_sha256, compilations: rows.module.compilations, elapsed_ns };
}

/** The source's graph, and both of its evaluators on the same universe. */
function graphs(binary, directory, name, source, universe) {
  const file = path.join(directory, `${name}.graph`);
  const extract = cli(binary, ['ngraph', 'extract', source.file, '--inputs', 'i64,i64', '--out', file]);
  assert.equal(extract.rows.outcome, 'normal', `extraction refused ${name}: ${JSON.stringify(extract.rows).slice(0, 300)}`);
  const vectors = path.join(directory, `${name}.vectors`);
  const text = ['noble-ngraph-vectors/0', 'types i64 i64',
    ...universe.map(([a, b]) => `v ${a} ${b}`), ''].join('\n');
  fs.writeFileSync(vectors, text, { flag: 'wx' });
  const evaluate = flag => {
    const { rows } = cli(binary, ['ngraph', 'eval', file, vectors, ...flag], { lines: true });
    const outputs = rows.filter(row => row.outputs !== undefined).map(row => row.outputs);
    assert.equal(outputs.length, universe.length, `${name}: one evaluation per candidate`);
    return outputs.map(([rendered]) => { assert.match(rendered, /^i64:-?\d+$/); return BigInt(rendered.slice(4)); });
  };
  return { graph: extract.rows.graph, reference: evaluate(['--reference']), packed: evaluate([]),
    extraction_ns: extract.elapsed_ns };
}

function main() {
  const flags = options();
  const binary = flags['--noble'];
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'noble-endpoints-'));
  const source = { commit: git(['rev-parse', 'HEAD']), tree: git(['rev-parse', 'HEAD^{tree}']),
    clean: git(['status', '--porcelain', '--untracked-files=all']) === '' };
  const cliSha256 = sha(fs.readFileSync(fs.realpathSync(binary)));
  try {
    const rows = PAIRS.map(pair => {
      const sides = { left: writeSource(directory, `${pair.name}-left`, pair.left.source),
        right: writeSource(directory, `${pair.name}-right`, pair.right.source) };
      const observed = {};
      for (const side of ['left', 'right']) {
        const spec = pair[side];
        const points = UNIVERSE.map(inputs => {
          const end = endpoint(binary, sides[side], inputs);
          return { inputs: inputs.map(String), endpoint: end, expected: value(spec.root, spec.operands, inputs) };
        });
        const graph = graphs(binary, directory, `${pair.name}-${side}`, sides[side], UNIVERSE);
        points.forEach((point, at) => {
          assert.equal(point.endpoint.value, point.expected, `${pair.name}/${side}: endpoint computes the root's value`);
          assert.equal(graph.packed[at], point.expected, `${pair.name}/${side}: packed machine agrees with the root`);
          assert.equal(graph.reference[at], point.expected, `${pair.name}/${side}: reference interpreter agrees with the root`);
        });
        const digests = new Set(points.map(point => point.endpoint.wasm_sha256));
        assert.equal(digests.size, 1, `${pair.name}/${side}: one module per source`);
        assert.ok([...digests][0].match(/^[0-9a-f]{64}$/), `${pair.name}/${side}: module digest is a SHA-256`);
        assert.equal(graph.reference.length, UNIVERSE.length);
        observed[side] = { source: sides[side].sha256, graph: graph.graph,
          wasm_sha256: [...digests][0], extraction_ns: graph.extraction_ns, points };
      }
      // The pair's verdict must be the same on the endpoints and on the graphs, candidate by candidate.
      const differing = UNIVERSE.map((_, at) => observed.left.points[at].endpoint.value !== observed.right.points[at].endpoint.value);
      const onGraphs = UNIVERSE.map((_, at) => observed.left.points[at].expected !== observed.right.points[at].expected ||
        observed.left.points[at].endpoint.value !== observed.right.points[at].endpoint.value);
      assert.deepEqual(differing, onGraphs, `${pair.name}: endpoints and graphs decide every candidate alike`);
      const first = differing.indexOf(true);
      const verdict = first < 0 ? 'agree' : 'refute';
      assert.equal(verdict, pair.expect, `${pair.name}: ${pair.claim}`);
      return { name: pair.name, claim: pair.claim, expected: pair.expect, verdict,
        first_differing_candidate: first < 0 ? null : first, sides: observed };
    });
    // Controls.
    const controlSource = writeSource(directory, 'control-mul', '*');
    const controlPoints = UNIVERSE.map(inputs => endpoint(binary, controlSource, inputs).value);
    assert.notDeepEqual(controlPoints, rows[0].sides.right.points.map(point => point.endpoint.value),
      'a mutated endpoint changes the observations');
    const crossed = rows[1].sides.left.points.map((point, at) => point.endpoint.value !== rows[1].sides.right.points[at].expected);
    assert.ok(crossed.some(Boolean), 'one side\'s graph disagrees with the other side\'s endpoint somewhere');
    const wrongInterface = writeSource(directory, 'control-interface', '1 +');
    const refused = cli(binary, ['worker', 'scalar', wrongInterface.file, '--state', '1', '--delta', '2']);
    assert.equal(refused.rows.outcome, 'interface-reject', 'a checked source outside the interface is refused');
    assert.equal(refused.rows.module, null, 'a refused source compiles no module');
    const illTyped = writeSource(directory, 'control-type', '+ +');
    const rejected = cli(binary, ['worker', 'scalar', illTyped.file, '--state', '1', '--delta', '2']);
    assert.equal(rejected.rows.outcome, 'type-reject', 'an ill-typed source is rejected at checking');
    assert.equal(sha(fs.readFileSync(fs.realpathSync(binary))), cliSha256, 'the CLI changed while the lane ran');
    const receipt = {
      schema: 'noble-symbolic-endpoints-receipt/v1',
      claim: 'source-backed pairs whose two sides each run as a checked Wasm module; graph and endpoint verdicts agree on a fixed universe',
      source: { ...source, lane_sha256: sha(fs.readFileSync(path.join(ROOT, LANE))) },
      cli_sha256: cliSha256,
      interface: INTERFACE,
      universe: UNIVERSE.map(inputs => inputs.map(String)),
      pairs: rows,
      controls: {
        mutated_endpoint: controlPoints.map(String),
        crossed_graph_disagrees: true,
        interface_refusal: refused.rows.outcome,
        ill_typed_refusal: rejected.rows.outcome,
      },
      non_claims: [
        'agreement is on one fixed universe of eight candidates per pair, not a proof for every input',
        'each endpoint is the CLI\'s own compiled module run on V8, identified by its SHA-256; no independent Wasm engine or second implementation is compared',
        'the graph the pair is decided on comes from `noble ngraph extract` on the same source; the extractor\'s own preservation claim remains NG-PO-A',
      ],
    };
    const decimal = (key, entry) => (typeof entry === 'bigint' ? entry.toString() : entry);
    fs.writeFileSync(flags['--output'], `${JSON.stringify(receipt, decimal, 2)}\n`, { flag: 'wx' });
    process.stdout.write(`${JSON.stringify({ output: flags['--output'], clean: source.clean,
      verdicts: Object.fromEntries(rows.map(row => [row.name, row.verdict])) })}\n`);
  } finally {
    fs.rmSync(directory, { recursive: true });
  }
}

main();
