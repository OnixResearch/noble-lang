#!/usr/bin/env node
/**
 * Untrusted-solver lane for the fixed graph pairs of issue #56 (acceptance items 4 to 6).
 *
 * It parses the committed `proofs/foundation/NobleFoundation/SymbolicPairCorpus.lean`, which the
 * generator writes from the pairs `noble-ngraph` builds and checks, with a strict grammar. For each
 * pair it builds one shared-DAG QF_BV query for "some listed output differs" and runs two pinned
 * SMT solvers on it. Neither solver is trusted, and every claim gets one of four outcomes:
 *   - refuted: a satisfying model whose exact 64-bit slots, replayed through this lane's own
 *     evaluator on the original graphs (never on the formula), make a listed output differ;
 *   - certified: only a kernel certificate (`<pair>_certified`) in a clean-source foundation
 *     receipt whose recorded corpus bytes are the ones read here; solver UNSAT never certifies;
 *   - inconclusive: UNSAT without an accepted proof, UNKNOWN, a timeout, a solver error, a model
 *     that does not parse or does not replay, or a query bound to other bytes or revisions;
 *   - unsupported: anything outside the parsed fragment or a declared budget.
 *
 * Every query also passes two encoder checks that do not trust the formula: a concrete
 * simulation, where each defined node is pinned to the value this lane's evaluator computes at
 * fixed points and replayed witnesses and the query must stay satisfiable, and a claim check,
 * where the outputs the formula compares are parsed back and must be the pair's listed outputs.
 *
 * Controls: seeded graph mutations; independently corrupted encodings, each of which must fail
 * an encoder check; stale revisions, altered pair bytes, malformed witnesses and certificates,
 * forced UNKNOWN and timeouts, and exhausted budgets. Measurements: formula size sliced and
 * unsliced, solver time and peak memory per run, warm cache hits, and replay cost, reported
 * separately.
 *
 * Every distinct solver model is also written, one per line, to a witness file: `refutes` for a
 * model that replays on the original graphs, `rejects` for one that does not. The receipt records
 * the file's SHA-256, and `crates/noble-ngraph/tests/schedule_lean_dir/pairs/replay.rs` replays
 * each line through both Rust graph evaluators and generates `SymbolicSolverReplay.lean`, where the
 * kernel decides each line again.
 *
 * Usage (pinned Node, env -i, from a clean checkout):
 *   node verification/symbolic/solver.mjs --bitwuzla BIN --z3 BIN --time GNU_TIME
 *     --foundation-receipt DIR --output NEW_FILE --witnesses NEW_FILE --queries NEW_FILE
 *
 * The query core — strict admission, this lane's evaluator, the shared-DAG encoder, the encoder
 * checks and the measured solver runs — lives in [`qfbv.mjs`](qfbv.mjs) and is imported here, so
 * the bounded synthesis lane (issue #116, `verification/synthesis/cegis.mjs`) reuses exactly the
 * code this lane decides its claims with instead of copying it. This module keeps the corpus
 * parser, the certificate binding, the controls and the run.
 */
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import {
  BUDGET, ENCODER, SOLVERS, Unsupported, admit, encode, encoderChecks, elapsed, hex16, outcome,
  parseModel, refutes, sha, solve,
} from './qfbv.mjs';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const LANE = 'verification/symbolic/solver.mjs';
const CORPUS = 'proofs/foundation/NobleFoundation/SymbolicPairCorpus.lean';
const CERTIFIED = 'proofs/foundation/NobleFoundation/SymbolicPairCertified.lean';
const IR = 'crates/noble-ngraph/src/ir.rs';
const CORPUS_NAMESPACE = 'NobleFoundation.SymbolicSolver.Corpus';
const STRICT_AXIOMS = ['propext', 'Classical.choice', 'Quot.sound'];
const FAULTS = ['sub-order', 'mul-as-add', 'constant-bit', 'drop-output'];

const read = relative => fs.readFileSync(path.join(ROOT, relative));

function options() {
  const wanted = ['--bitwuzla', '--z3', '--time', '--foundation-receipt', '--output', '--witnesses', '--queries'];
  const argv = process.argv.slice(2);
  assert.equal(argv.length, wanted.length * 2, `usage: solver.mjs ${wanted.map(flag => `${flag} V`).join(' ')}`);
  const out = {};
  for (let index = 0; index < argv.length; index += 2) {
    assert.ok(wanted.includes(argv[index]) && !(argv[index] in out), `unexpected argument ${argv[index]}`);
    out[argv[index]] = argv[index + 1];
  }
  for (const key of ['--bitwuzla', '--z3', '--time']) assert.ok(path.isAbsolute(out[key]), `${key} must be absolute`);
  for (const key of ['--output', '--witnesses', '--queries']) {
    assert.ok(!fs.existsSync(out[key]), `${key} must not exist`);
    assert.ok(!path.resolve(out[key]).startsWith(ROOT + path.sep), `${key} must be outside the repository`);
  }
  return out;
}

/* ---------- The corpus, parsed strictly ---------- */

function parseNode(term) {
  let match;
  if ((match = term.match(/^\.input (\d+)$/))) return { op: 'input', slot: Number(match[1]) };
  if ((match = term.match(/^\.constant 0x([0-9a-fA-F]{16})$/))) return { op: 'constant', bits: BigInt(`0x${match[1]}`) };
  if ((match = term.match(/^\.(add|sub|mul|eq) (\d+) (\d+)$/))) {
    return { op: match[1], a: Number(match[2]), b: Number(match[3]) };
  }
  throw new Unsupported(`node term outside the fragment: ${term}`);
}

function parseCorpus(text) {
  const shape = /^def (\w+) : Pair :=\n {2}\{ left := FirstProjection\.ofList \[([^\]]*)\] \d+,\n {4}right := FirstProjection\.ofList \[([^\]]*)\] \d+,\n {4}outputs := \[([^\]]*)\] \}$/gm;
  const pairs = [...text.matchAll(shape)].map(([whole, name, left, right, outputs]) => {
    const verdict = text.match(new RegExp(`^theorem ${name}_first : firstIndex ${name} ${name}Universe = (none|some \\d+) := by$`, 'm'));
    assert.ok(verdict, `${name}: no kernel universe verdict`);
    return { name, digest: sha(whole), left: left.split(', ').map(parseNode), right: right.split(', ').map(parseNode),
      outputs: outputs.split(', ').map(Number), universe: verdict[1] };
  });
  assert.equal(pairs.length, (text.match(/^def \w+ : Pair :=/gm) ?? []).length, 'every pair parses');
  assert.ok(pairs.length > 0, 'the corpus has pairs');
  return pairs;
}


/** Kernel certificates, accepted only from a clean receipt that recorded the bytes read here. */
function certificates(receipt, corpusBytes, certifiedBytes) {
  const bound = receipt.clean_source === true && receipt.allow_dirty === false
    && receipt.sources_sha256?.[CORPUS] === sha(corpusBytes) && receipt.sources_sha256?.[CERTIFIED] === sha(certifiedBytes);
  const proved = new Set(receipt.theorems.filter(theorem => theorem.status === 'proved'
    && theorem.axioms.every(axiom => STRICT_AXIOMS.includes(axiom))).map(theorem => theorem.name));
  return {
    binding: { source_commit: receipt.source_commit, bound, corpus_sha256: sha(corpusBytes), certified_sha256: sha(certifiedBytes) },
    of: name => (bound && proved.has(`${CORPUS_NAMESPACE}.${name}_certified`)
      ? `${CORPUS_NAMESPACE}.${name}_certified is proved in the bound foundation receipt` : null),
  };
}

/* ---------- Controls ---------- */

/** Seeded mutations of the right graph: one op, operand order, constant or output each. */
function mutants(pair) {
  const replace = (index, node, label) => ({ ...pair, name: `${pair.name}~${label}`,
    right: pair.right.map((existing, at) => (at === index ? node : existing)), label });
  const found = [];
  pair.right.forEach((node, index) => {
    if (node.op === 'add') found.push(replace(index, { ...node, op: 'sub' }, `add->sub at ${index}`));
    if (node.op === 'mul') found.push(replace(index, { ...node, op: 'add' }, `mul->add at ${index}`));
    if (node.op === 'sub' && node.a !== node.b) found.push(replace(index, { ...node, a: node.b, b: node.a }, `operands swapped at ${index}`));
    if (node.op === 'constant') found.push(replace(index, { ...node, bits: node.bits ^ 1n }, `constant bit 0 flipped at ${index}`));
  });
  const out = pair.outputs[0];
  const kind = node => (node.op === 'eq' ? 'bool' : 'i64');
  for (let from = out - 1; from >= 0; from -= 1) {
    if (kind(pair.right[from]) === kind(pair.right[out])) {
      found.push(replace(out, { ...pair.right[from] }, `output ${out} reads node ${from}'s value`));
      break;
    }
  }
  return found.map(mutant => ({ ...mutant, digest: sha(JSON.stringify(mutant, (key, value) => (typeof value === 'bigint' ? `0x${hex16(value)}` : value))) }));
}

function malformedModels() {
  const good = `((s0 #x${'0'.repeat(15)}1)\n (s1 #x${'0'.repeat(16)}))`;
  return [
    ['correct, the positive control', good],
    ['narrow value', `((s0 #x01) (s1 #x${'0'.repeat(16)}))`],
    ['missing slot', `((s0 #x${'0'.repeat(16)}))`],
    ['duplicate slot', `((s0 #x${'0'.repeat(16)}) (s0 #x${'0'.repeat(16)}) (s1 #x${'0'.repeat(16)}))`],
    ['extra slot', `${good.slice(0, -1)} (s2 #x${'0'.repeat(16)}))`],
    ['no values', 'unexpected output'],
  ];
}

/* ---------- The run ---------- */

function main() {
  const flags = options();
  const tools = { bitwuzla: flags['--bitwuzla'], z3: flags['--z3'], time: flags['--time'] };
  const git = argv => spawnSync('/run/current-system/sw/bin/git', argv, { cwd: ROOT, encoding: 'utf8' }).stdout.trim();
  const source = { commit: git(['rev-parse', 'HEAD']), tree: git(['rev-parse', 'HEAD^{tree}']),
    clean: git(['status', '--porcelain', '--untracked-files=all']) === '' };
  const corpusBytes = read(CORPUS);
  const certifiedBytes = read(CERTIFIED);
  const irText = read(IR).toString('utf8');
  const semantic = irText.match(/pub const SEMANTIC_REVISION: &str = "([^"]+)";/)?.[1];
  const profile = irText.match(/Self::I64 => "([^"]+)",/)?.[1];
  assert.ok(semantic && profile, 'the graph IR names its semantic revision and I64 profile');
  const expected = { encoder: ENCODER, semantic_revision: semantic, profile };
  const bind = pair => ({ ...expected, pair_sha256: pair.digest });
  const receiptJson = JSON.parse(fs.readFileSync(path.join(flags['--foundation-receipt'], 'foundation-check.json'), 'utf8'));
  const certs = certificates(receiptJson, corpusBytes, certifiedBytes);
  assert.ok(certs.binding.bound, 'the foundation receipt must be clean and record the corpus bytes read here');
  const pairs = parseCorpus(corpusBytes.toString('utf8'));
  const cache = new Map();
  const witnessesOf = new Map();
  const queries = [];

  // The claims: every corpus pair, both solvers, sliced and unsliced queries.
  const claims = pairs.map(pair => {
    const types = admit(pair);
    const sliced = encode(pair, types);
    const unsliced = encode(pair, types, { slice: false });
    assert.ok(sliced.bytes <= unsliced.bytes && sliced.nodes <= unsliced.nodes, `${pair.name}: slicing never grows a query`);
    for (const [mode, query] of [['sliced', sliced], ['unsliced', unsliced]]) {
      queries.push({ pair: pair.name, mode, bytes: query.bytes, nodes: query.nodes, sha256: query.sha256, text: query.text });
    }
    const runs = [];
    for (const [mode, query] of [['sliced', sliced], ['unsliced', unsliced]]) {
      for (const solver of SOLVERS) {
        const answer = solve(tools, solver, query);
        const result = outcome(pair, bind(pair), expected, answer, null);
        runs.push({ solver, mode, answer: answer.answer, wall_ns: answer.wall_ns, peak_rss_kb: answer.peak_rss_kb, ...result });
        if (mode === 'sliced') cache.set(`${solver}:${query.sha256}`, answer);
      }
    }
    const warm = SOLVERS.map(solver => {
      const started = process.hrtime.bigint();
      const hit = cache.get(`${solver}:${sliced.sha256}`);
      return { solver, cache_hit: hit !== undefined, lookup_ns: elapsed(started), answer: hit?.answer ?? null };
    });
    const certificate = certs.of(pair.name);
    const refutedRuns = runs.filter(run => run.status === 'refuted');
    assert.ok(!(certificate && refutedRuns.length), `${pair.name}: a certified pair must not replay a counterexample`);
    const seen = new Set();
    const witnesses = refutedRuns.map(run => JSON.stringify(run.witness)).filter(key => !seen.has(key) && seen.add(key))
      .map(key => new Map(Object.entries(JSON.parse(key)).map(([name, bits]) => [Number(name.slice(1)), BigInt(bits)])));
    witnessesOf.set(pair.name, witnesses);
    const checks = Object.fromEntries([['sliced', sliced], ['unsliced', unsliced]].map(([mode, query]) => {
      const { simulation, ...summary } = encoderChecks(tools, pair, types, query, witnesses);
      assert.deepEqual(summary.detected_by, [], `${pair.name}/${mode}: the honest encoding passes its checks`);
      return [mode, { ...summary, answers: simulation.map(entry => entry.answers.join('/')) }];
    }));
    const status = certificate ? 'certified' : refutedRuns.length ? 'refuted' : 'inconclusive';
    return { name: pair.name, pair_sha256: pair.digest, kernel_universe_verdict: pair.universe, status,
      certificate, formula: { sliced: { bytes: sliced.bytes, nodes: sliced.nodes, sha256: sliced.sha256 },
        unsliced: { bytes: unsliced.bytes, nodes: unsliced.nodes, sha256: unsliced.sha256 } }, encoder_checks: checks, runs, warm };
  });
  for (const claim of claims) {
    if (claim.kernel_universe_verdict !== 'none') assert.equal(claim.status, 'refuted', `${claim.name}: the kernel refuted it on its universe`);
  }

  // Item 4a: seeded mutations of the certified pairs.
  const mutations = pairs.filter(pair => certs.of(pair.name)).flatMap(pair => mutants(pair).map(mutant => {
    const types = admit(mutant);
    const query = encode(mutant, types);
    const results = SOLVERS.map(solver => {
      const answer = solve(tools, solver, query);
      return { solver, answer: answer.answer, ...outcome(mutant, bind(mutant), expected, answer, null) };
    });
    assert.ok(results.every(result => result.status !== 'certified'), `${mutant.name}: a mutant is never certified`);
    const detected = results.some(result => result.status === 'refuted');
    if (!detected) {
      assert.ok(results.every(result => result.answer === 'unsat'), `${mutant.name}: an undetected mutant has only UNSAT answers`);
    }
    return { pair: pair.name, mutation: mutant.label, detected, results };
  }));

  // Item 4b: independently corrupted encodings, replayed against the original graphs.
  const corrupted = pairs.flatMap(pair => FAULTS.map(fault => {
    const types = admit(pair);
    const honest = encode(pair, types);
    const query = encode(pair, types, { fault });
    if (query.sha256 === honest.sha256) return null;
    const results = SOLVERS.map(solver => {
      const answer = solve(tools, solver, query);
      return { solver, answer: answer.answer, ...outcome(pair, bind(pair), expected, answer, null) };
    });
    assert.ok(results.every(result => result.status !== 'certified'), `${pair.name}/${fault}: a corrupted query never certifies`);
    const solversAgree = results.every(result => result.answer === results[0].answer);
    const rejected = results.filter(result => result.reason === 'model does not replay on the original graphs').length;
    const { detected_by } = encoderChecks(tools, pair, types, query, witnessesOf.get(pair.name));
    assert.ok(detected_by.length > 0, `${pair.name}/${fault}: a corrupted encoding fails an encoder check`);
    return { pair: pair.name, fault, solvers_agree: solversAgree, answers: results.map(result => result.answer),
      statuses: results.map(result => result.status), models_rejected_by_replay: rejected, detected_by, results };
  })).filter(Boolean);
  assert.ok(corrupted.some(entry => entry.solvers_agree && entry.answers[0] === 'sat' && entry.models_rejected_by_replay === SOLVERS.length),
    'some corrupted query has both solvers agreeing on models that fail replay');

  // Item 6: slicing on variants with dead nodes. No corpus graph has a node its outputs do not
  // read, so each pair is padded with eight such nodes; slicing must drop them and keep every answer.
  const slicing = pairs.map(pair => {
    const dead = [...Array(8)].map((_, at) => ({ op: at % 2 ? 'mul' : 'add', a: 0, b: 0 }));
    const padded = { ...pair, left: [...pair.left, ...dead], right: [...pair.right, ...dead] };
    padded.digest = sha(JSON.stringify(padded, (key, value) => (typeof value === 'bigint' ? `0x${hex16(value)}` : value)));
    const types = admit(padded);
    const modes = Object.fromEntries([['sliced', true], ['unsliced', false]].map(([mode, slice]) => {
      const query = encode(padded, types, { slice });
      const runs = SOLVERS.map(solver => {
        const answer = solve(tools, solver, query);
        return { solver, answer: answer.answer, wall_ns: answer.wall_ns, peak_rss_kb: answer.peak_rss_kb,
          ...outcome(padded, bind(padded), expected, answer, null) };
      });
      return [mode, { bytes: query.bytes, nodes: query.nodes, runs }];
    }));
    assert.ok(modes.sliced.bytes < modes.unsliced.bytes, `${pair.name}: slicing drops the dead nodes`);
    assert.deepEqual(modes.sliced.runs.map(run => run.answer), modes.unsliced.runs.map(run => run.answer),
      `${pair.name}: slicing keeps every answer`);
    return { pair: pair.name, dead_nodes_per_graph: dead.length, ...modes };
  });
  const cacheRetainedBytes = [...cache].reduce((total, [key, answer]) =>
    total + Buffer.byteLength(key) + Buffer.byteLength(JSON.stringify({ answer: answer.answer,
      slots: answer.slots ? [...answer.slots].map(([slot, bits]) => [slot, `0x${hex16(bits)}`]) : null })), 0);

  // Item 5: bindings, witnesses, certificates, UNKNOWN, timeouts and budgets.
  const refutedPair = pairs.find(pair => pair.name === 'subtractionOrder');
  assert.ok(refutedPair, 'the subtractionOrder pair is the binding control');
  const goodAnswer = solve(tools, 'z3', encode(refutedPair, admit(refutedPair)));
  const bindings = [
    ['bound query, the positive control', bind(refutedPair)],
    ['stale semantic revision', { ...bind(refutedPair), semantic_revision: `${semantic}-stale` }],
    ['other I64 profile', { ...bind(refutedPair), profile: 'ngraph-i64/stale' }],
    ['other encoder', { ...bind(refutedPair), encoder: 'noble-symbolic-qfbv/v0' }],
    ['altered pair bytes', { ...bind(refutedPair), pair_sha256: sha(`${refutedPair.digest} altered`) }],
  ].map(([label, query]) => ({ label, ...outcome(refutedPair, query, expected, goodAnswer, null) }));
  assert.equal(bindings[0].status, 'refuted');
  assert.ok(bindings.slice(1).every(entry => entry.status === 'inconclusive'), 'stale or altered bindings never refute');
  const witnesses = malformedModels().map(([label, text]) => {
    const parsed = parseModel(text, ['s0', 's1']);
    return { label, ...outcome(refutedPair, bind(refutedPair), expected, { answer: 'sat', ...parsed }, null) };
  });
  assert.equal(witnesses[0].status, 'refuted');
  assert.ok(witnesses.slice(1).every(entry => entry.status === 'inconclusive'), 'malformed witnesses never refute');
  const certifiedPair = pairs.find(pair => certs.of(pair.name));
  assert.ok(certifiedPair, 'some pair is certified');
  const tamper = (label, edit) => {
    const copy = structuredClone(receiptJson);
    edit(copy);
    return { label, certificate: certificates(copy, corpusBytes, certifiedBytes).of(certifiedPair.name) };
  };
  const certifiedQuery = encode(certifiedPair, admit(certifiedPair));
  const certifiedAnswer = solve(tools, 'z3', certifiedQuery);
  const certificateControls = [
    { label: 'bound receipt, the positive control', certificate: certs.of(certifiedPair.name) },
    tamper('corpus bytes differ from the receipt', copy => { copy.sources_sha256[CORPUS] = sha('other corpus'); }),
    tamper('dirty receipt', copy => { copy.clean_source = false; }),
    tamper('theorem not proved', copy => {
      copy.theorems.find(theorem => theorem.name === `${CORPUS_NAMESPACE}.${certifiedPair.name}_certified`).status = 'failed';
    }),
    tamper('disallowed axiom', copy => {
      copy.theorems.find(theorem => theorem.name === `${CORPUS_NAMESPACE}.${certifiedPair.name}_certified`).axioms.push('sorryAx');
    }),
  ].map(({ label, certificate }) => ({ label, certificate,
    ...outcome(certifiedPair, bind(certifiedPair), expected, certifiedAnswer, certificate) }));
  assert.ok(certificateControls[0].certificate && certificateControls.slice(1).every(entry => entry.certificate === null),
    'only a bound, clean, proved, strict certificate certifies');
  const forced = [
    ['injected UNKNOWN', { answer: 'unknown' }, null],
    ['injected solver error', { answer: 'error' }, null],
    ['watchdog timeout', solve(tools, 'bitwuzla', certifiedQuery, { watchdogMs: 1 }), null],
    ['bound certificate with an injected UNKNOWN', { answer: 'unknown' }, certs.of(certifiedPair.name)],
  ].map(([label, answer, certificate]) => ({ label, answer: answer.answer,
    ...outcome(certifiedPair, bind(certifiedPair), expected, answer, certificate) }));
  assert.ok(forced.slice(0, 3).every(entry => entry.status === 'inconclusive'), 'UNKNOWN, errors and timeouts are inconclusive');
  assert.equal(forced[3].status, 'certified', 'a bound kernel certificate decides whatever the solver answered');
  const refusals = [
    ['node budget', () => admit({ ...refutedPair, left: [...Array(BUDGET.nodes + 1)].map(() => ({ op: 'input', slot: 0 })) })],
    ['slot budget', () => admit({ ...refutedPair, left: [{ op: 'input', slot: BUDGET.slots }, ...refutedPair.left.slice(1)] })],
    ['formula budget', () => encode(refutedPair, admit(refutedPair), { budget: { ...BUDGET, formula_bytes: 64 } })],
    ['forward read', () => admit({ ...refutedPair, left: [{ op: 'add', a: 1, b: 1 }, ...refutedPair.left.slice(1)] })],
    ['arithmetic on Bool', () => admit({ ...refutedPair, left: [{ op: 'input', slot: 0 }, { op: 'eq', a: 0, b: 0 }, { op: 'add', a: 1, b: 0 }] })],
  ].map(([label, attempt]) => {
    try {
      attempt();
      return { label, status: 'admitted' };
    } catch (error) {
      if (!(error instanceof Unsupported)) throw error;
      // A refused row never ran: admission comes first, so no binding, certificate, model or
      // answer was examined. The tuple records exactly that, and `classify` must report
      // `unsupported` from it.
      return { label, status: 'unsupported', reason: error.message,
        observation: { within_budget: false, bound: false, certificate: false,
          model_parsed: false, model_replays: false, answer: 'notRun' } };
    }
  });
  assert.ok(refusals.every(entry => entry.status === 'unsupported'), 'every exhausted budget or fragment violation is unsupported');

  const tool = name => {
    const real = fs.realpathSync(tools[name]);
    return { path: tools[name], sha256: sha(fs.readFileSync(real)) };
  };
  // Every distinct model of the corpus pairs, from the claims and the corrupted encodings, in run
  // order: `refutes` when it replayed on the original graphs, `rejects` when it did not.
  const witnessLines = [];
  const record = (kind, pair, slots) => {
    const line = `${kind} ${pair} ${Object.entries(slots).map(([name, bits]) => `${name}=${bits}`).join(' ')}`;
    if (!witnessLines.includes(line)) witnessLines.push(line);
  };
  for (const claim of claims) {
    for (const run of claim.runs) if (run.witness) record('refutes', claim.name, run.witness);
  }
  for (const entry of corrupted) {
    for (const result of entry.results) {
      if (result.witness) record('refutes', entry.pair, result.witness);
      if (result.model) record('rejects', entry.pair, result.model);
    }
  }
  const witnessText = ['# noble-symbolic-witnesses/v1: distinct solver models for the fixed graph pairs,',
    '# written by verification/symbolic/solver.mjs; refutes = replayed on the original graphs, rejects = did not',
    ...witnessLines, ''].join('\n');
  fs.writeFileSync(flags['--witnesses'], witnessText, { flag: 'wx' });
  // Every emitted query's text, so the Lean encoder model can be bound to the exact bytes the
  // solvers read (`SymbolicEncoderCorpus`, generated by verification/symbolic/solver-lean.mjs).
  const queryText = `${JSON.stringify({ schema: 'noble-symbolic-queries/v1',
    note: 'queries as encode() writes them, in corpus order, sliced and unsliced, without any injected fault',
    encoder: ENCODER, corpus_sha256: sha(corpusBytes), queries }, null, 2)}\n`;
  fs.writeFileSync(flags['--queries'], queryText, { flag: 'wx' });
  const receipt = {
    schema: 'noble-symbolic-solver-receipt/v1',
    claim: 'untrusted SMT solving of the fixed graph pairs, with replay, kernel-certificate binding and controls; graph-model statuses, not Wasm theorems',
    source: { ...source, lane_sha256: sha(read(LANE)), corpus_sha256: sha(corpusBytes), certified_sha256: sha(certifiedBytes),
      semantic_revision: semantic, profile, encoder: ENCODER },
    foundation_receipt: certs.binding,
    tools: { bitwuzla: tool('bitwuzla'), z3: tool('z3'), time: tool('time') },
    budget: BUDGET,
    claims,
    queries: { sha256: sha(queryText), rows: queries.length },
    witnesses: { sha256: sha(witnessText), refutes: witnessLines.filter(line => line.startsWith('refutes')).length,
      rejects: witnessLines.filter(line => line.startsWith('rejects')).length },
    measurements: { slicing, warm_cache: { entries: cache.size, retained_bytes: cacheRetainedBytes } },
    controls: {
      mutations: { total: mutations.length, detected: mutations.filter(entry => entry.detected).length,
        survivors: mutations.filter(entry => !entry.detected).map(entry => `${entry.pair}: ${entry.mutation}`), entries: mutations },
      corrupted_encodings: corrupted,
      bindings, witnesses, certificates: certificateControls, forced, refusals,
    },
    non_claims: [
      'statuses are about the model graphs the corpus states; no source extraction, Wasm lowering or Rust evaluator is a subject',
      'solver UNSAT is recorded but never certifies; only the bound kernel certificates do',
      'refuted means the model replays through this lane\'s evaluator on the original graphs; the kernel and the Rust evaluators replay only the universe witnesses of the corpus',
      'timings and peak memory are one run on one host, not a performance claim',
    ],
  };
  fs.writeFileSync(flags['--output'], `${JSON.stringify(receipt, null, 2)}\n`, { flag: 'wx' });
  const summary = Object.fromEntries(claims.map(claim => [claim.name, claim.status]));
  process.stdout.write(`${JSON.stringify({ output: flags['--output'], clean: source.clean, claims: summary,
    mutations: `${receipt.controls.mutations.detected}/${receipt.controls.mutations.total}`, corrupted: corrupted.length })}\n`);
}

// The corpus parser and the encoder are exported for the Lean corpus generator
// (`verification/symbolic/solver-lean.mjs`), which must encode the same graph the lane does.
// `admit` and `encode` are re-exported from the shared query core (`qfbv.mjs`).
export { parseCorpus, admit, encode };

if (process.argv[1] && pathToFileURL(path.resolve(process.argv[1])).href === import.meta.url) main();
