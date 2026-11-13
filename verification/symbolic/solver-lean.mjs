#!/usr/bin/env node
/**
 * Generate the Lean modules that bind the retained #56 receipts to the models.
 *
 *   proofs/foundation/NobleFoundation/SymbolicRefusalsReplay.lean
 *     every observation tuple and recorded status of `verification/symbolic/solver.mjs` and of the
 *     endpoint lane's altered-bytes control, each decided by the kernel, plus the claim
 *     aggregation; `SymbolicRefusals.classify` must reproduce every recorded status.
 *   proofs/foundation/NobleFoundation/SymbolicEncoderCorpus.lean
 *     for every corpus pair, the exact query bytes the solvers read, decided equal to
 *     `SymbolicEncoder.renderQuery`, and the claim-simulation theorem on that subject.
 *
 * The receipts are the input, so the modules are evidence, not prose: the generator refuses a
 * receipt whose rows are incomplete, and `--check` (the mode
 * `verification/ngraph/symbolic-pair-inventory.mjs` uses) refuses a committed module that is not
 * this generator's output for the retained receipts.
 *
 * Usage (pinned Node, from a clean checkout):
 *   node verification/symbolic/solver-lean.mjs --solver S --queries Q --endpoints E --check
 *   node verification/symbolic/solver-lean.mjs --solver S --queries Q --endpoints E --write
 */
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseCorpus } from './solver.mjs';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(HERE, '../..');
const CORPUS = 'proofs/foundation/NobleFoundation/SymbolicPairCorpus.lean';
const REFUSALS_MODULE = 'proofs/foundation/NobleFoundation/SymbolicRefusalsReplay.lean';
const ENCODER_MODULE = 'proofs/foundation/NobleFoundation/SymbolicEncoderCorpus.lean';
const VERDICTS = ['certified', 'refuted', 'inconclusive', 'unsupported'];
const ANSWERS = ['sat', 'unsat', 'unknown', 'timeout', 'error', 'notRun'];
const OPS = ['add', 'sub', 'mul', 'eq'];
const sha = bytes => createHash('sha256').update(bytes).digest('hex');

/* ---------- Lean rendering ---------- */

/** A Lean string literal. Every name here is ASCII, so JSON's escaping is Lean's. */
const quote = text => JSON.stringify(text);

const observation = value => {
  for (const field of ['within_budget', 'bound', 'certificate', 'model_parsed', 'model_replays']) {
    assert.equal(typeof value[field], 'boolean', `observation.${field} is a boolean`);
  }
  assert.ok(ANSWERS.includes(value.answer), `answer ${value.answer} is in the vocabulary`);
  return `⟨${value.within_budget}, ${value.bound}, ${value.certificate}, ${value.model_parsed}, ` +
    `${value.model_replays}, .${value.answer}⟩`;
};

const verdict = value => {
  assert.ok(VERDICTS.includes(value), `status ${value} is in the vocabulary`);
  return `.${value}`;
};

const node = value => {
  if (value.op === 'input') return `.input ${value.slot}`;
  if (value.op === 'constant') return `.constant 0x${value.bits.toString(16).padStart(16, '0')}`;
  assert.ok(OPS.includes(value.op), `node operator ${value.op} is in the fragment`);
  return `.${value.op} ${value.a} ${value.b}`;
};

const nodeList = values => `[${values.map(node).join(', ')}]`;

/** One Lean identifier per row, unique, ASCII, and stable across runs. */
const namesOf = rows => {
  const taken = new Set();
  return rows.map(row => {
    const base = `row_${row.name.replace(/[^A-Za-z0-9]+/gu, '_').replace(/^_+|_+$/gu, '')}`;
    let name = base;
    for (let at = 2; taken.has(name); at += 1) name = `${base}_${at}`;
    taken.add(name);
    return name;
  });
};

/* ---------- Rows, from the receipts ---------- */

/** Every recorded row of the solver receipt, in receipt order, with its tuple and status. */
function solverRows(receipt) {
  const rows = [];
  const add = (name, observation, status) => {
    assert.ok(observation && VERDICTS.includes(status), `${name}: incomplete row`);
    rows.push({ name, observation, status });
  };
  for (const claim of receipt.claims) {
    for (const run of claim.runs) add(`claim/${claim.name}/${run.mode}/${run.solver}`, run.observation, run.status);
  }
  for (const entry of receipt.controls.mutations.entries) {
    for (const run of entry.results) add(`mutation/${entry.pair}/${entry.mutation}/${run.solver}`, run.observation, run.status);
  }
  for (const entry of receipt.controls.corrupted_encodings) {
    for (const run of entry.results) add(`corrupt/${entry.pair}/${entry.fault}/${run.solver}`, run.observation, run.status);
  }
  for (const row of receipt.controls.bindings) add(`binding/${row.label}`, row.observation, row.status);
  for (const row of receipt.controls.witnesses) add(`witness/${row.label}`, row.observation, row.status);
  for (const row of receipt.controls.certificates) add(`certificate/${row.label}`, row.observation, row.status);
  for (const row of receipt.controls.forced) add(`forced/${row.label}`, row.observation, row.status);
  for (const row of receipt.controls.refusals) {
    assert.equal(row.status, 'unsupported', `${row.label}: a refused row is unsupported`);
    add(`refusal/${row.label}`, row.observation, row.status);
  }
  for (const entry of receipt.measurements.slicing) {
    for (const mode of ['sliced', 'unsliced']) {
      for (const run of entry[mode].runs) add(`slicing/${entry.pair}/${mode}/${run.solver}`, run.observation, run.status);
    }
  }
  return rows;
}

/** Every claim of the solver receipt, with the statuses of its rows and the recorded verdict. */
function solverClaims(receipt) {
  return receipt.claims.map(claim => ({ name: claim.name, certificate: Boolean(claim.certificate),
    runs: claim.runs.map(run => run.status), status: claim.status }));
}

/** The endpoint lane's altered-loaded-bytes rows: other bytes never produce a verdict. */
function endpointRows(receipt) {
  const hazard = receipt.controls.altered_loaded_bytes;
  assert.ok(hazard, 'the endpoint receipt records the altered-loaded-bytes hazard');
  assert.ok(hazard.rows.length > 0, 'the hazard has executed rows');
  const rows = hazard.rows.map(row => {
    assert.equal(row.binding_current, false, `${row.pair}/${row.side}: an altered row is not bound`);
    assert.equal(row.status, 'inconclusive', `${row.pair}/${row.side}: other bytes are inconclusive`);
    return { name: `endpoint/${row.pair}/${row.side}/${row.kind}`, observation: row.observation, status: row.status };
  });
  assert.equal(hazard.status, 'unsupported', 'the hazard execution leg is a fragment boundary');
  rows.push({ name: `endpoint/${hazard.subject}/execution`, observation: hazard.observation, status: hazard.status });
  return rows;
}

/* ---------- The refusals replay module ---------- */

const MUTANTS = [
  { mutant: 'binding_first', name: 'classifyBindingFirst', args: '', pick: rows => rows.find(row => row.status === 'unsupported' && row.observation.bound === false), why: 'a refused row whose binding is not current' },
  { mutant: 'answer_first', name: 'classifyAnswerFirst', args: '', pick: rows => rows.find(row => row.observation.certificate === true && row.status === 'certified' && row.observation.answer !== 'sat'), why: 'a certified row whose solver did not answer sat' },
  { mutant: 'encoding_checked', name: 'classifyEncodingChecked', args: ' false', pick: rows => rows.find(row => row.name.startsWith('corrupt/') && row.status === 'refuted'), why: 'a corrupted encoding whose model replayed' },
  { mutant: 'sat_refutes', name: 'classifySatRefutes', args: '', pick: rows => rows.find(row => row.observation.answer === 'sat' && row.observation.model_parsed === false && row.status === 'inconclusive'), why: 'a sat answer whose model did not parse' },
  { mutant: 'unsat_certifies', name: 'classifyUnsatCertifies', args: '', pick: rows => rows.find(row => row.observation.answer === 'unsat' && row.observation.certificate === false && row.status === 'inconclusive'), why: 'external UNSAT without a certificate' },
];

function renderRefusals(receipts) {
  const { solver, endpoints, digests } = receipts;
  const rows = [...solverRows(solver), ...endpointRows(endpoints)];
  assert.ok(rows.length > 0, 'the receipts record rows');
  const names = namesOf(rows);
  const claims = solverClaims(solver);
  const killers = MUTANTS.map(entry => {
    const row = entry.pick(rows);
    assert.ok(row, `no recorded row for mutant ${entry.mutant} (${entry.why})`);
    return { ...entry, row };
  });
  const statusCounts = Object.fromEntries(VERDICTS.map(status => [status, rows.filter(row => row.status === status).length]));
  const lines = [];
  const push = (...added) => lines.push(...added);
  push('import NobleFoundation.SymbolicRefusals', '');
  push('/-!');
  push('# Query verdicts of the retained #56 receipts, decided by the kernel');
  push('');
  push('Generated by `verification/symbolic/solver-lean.mjs` from');
  push(`\`${digests.solver.path}\` (sha256 \`${digests.solver.sha256}\`),`);
  push(`\`${digests.queries.path}\` (sha256 \`${digests.queries.sha256}\`) and`);
  push(`\`${digests.endpoints.path}\` (sha256 \`${digests.endpoints.sha256}\`); do not edit by hand.`);
  push('`verification/ngraph/symbolic-pair-inventory.mjs` refuses when this file is not the');
  push('generator\'s output for those receipts, and the foundation lane audits every theorem.');
  push('');
  push(`Every row of the solver receipt (${statusCounts.certified} certified, ${statusCounts.refuted} refuted,`);
  push(`${statusCounts.inconclusive} inconclusive, ${statusCounts.unsupported} unsupported) and every row of the`);
  push('endpoint lane\'s altered-loaded-bytes control is here with the observation tuple the lane');
  push('recorded. The kernel decides that `classify` reproduces the recorded status from that tuple,');
  push('and each seeded mutation of `classify` is shown to change one named recorded row.');
  push('-/');
  push('');
  push('namespace NobleFoundation.SymbolicSolver.RefusalsReplay');
  push('');
  push('open NobleFoundation.SymbolicSolver');
  push('');
  push('/-- One recorded row: the receipt\'s row name, its observation tuple and its status. -/');
  push('structure Row where');
  push('  /-- The row\'s name in the receipt. -/');
  push('  name : String');
  push('  /-- The facts the lane recorded about the row. -/');
  push('  observation : Observation');
  push('  /-- The status the lane derived from those facts. -/');
  push('  status : Verdict');
  push('deriving DecidableEq, Repr');
  push('');
  push('/-- One claim: its name, whether a bound certificate stood behind it, the statuses of its');
  push('rows in receipt order, and the status the lane recorded for the claim. -/');
  push('structure ClaimRow where');
  push('  /-- The pair the claim is about. -/');
  push('  name : String');
  push('  /-- Whether a bound kernel certificate backed the claim. -/');
  push('  certificate : Bool');
  push('  /-- Each run\'s status, in receipt order. -/');
  push('  runs : List Verdict');
  push('  /-- The claim\'s recorded status. -/');
  push('  status : Verdict');
  push('deriving DecidableEq, Repr');
  push('');
  push('/-- Every recorded row, in receipt order. -/');
  push('def receiptRows : List Row := [');
  rows.forEach((row, at) => push(`  ⟨${quote(row.name)}, ${observation(row.observation)}, ${verdict(row.status)}⟩,`));
  push(']');
  push('');
  push('/-- Every claim of the solver receipt. -/');
  push('def receiptClaims : List ClaimRow := [');
  claims.forEach(claim => push(`  ⟨${quote(claim.name)}, ${claim.certificate}, [${claim.runs.map(verdict).join(', ')}], ` +
    `${verdict(claim.status)}⟩,`));
  push(']');
  push('');
  push('/-- The kernel decides that `classify` reproduces every recorded row status from its tuple. -/');
  push('theorem receipt_rows_decide :');
  push('    (receiptRows.all fun row => classify row.observation == row.status) = true := by');
  push('  decide');
  push('');
  for (const status of VERDICTS) {
    push(`/-- Every recorded row whose status is \`${status}\` is classified \`${status}\`. -/`);
    push(`theorem ${status}_rows_decide :`);
    push(`    ((receiptRows.filter fun row => row.status == .${status}).all`);
    push(`      fun row => classify row.observation == .${status}) = true := by`);
    push('  decide');
    push('');
  }
  push('/-- The kernel decides that claim aggregation reproduces every recorded claim status. -/');
  push('theorem claims_decide :');
  push('    (receiptClaims.all fun claim => claimStatus claim.certificate claim.runs == claim.status) = true := by');
  push('  decide');
  push('');
  push('/-! ### Each recorded row, one theorem, so a failure names its row -/');
  push('');
  rows.forEach((row, at) => {
    push(`/-- \`${row.name}\` (${row.status}). -/`);
    push(`private theorem ${names[at]} : classify ${observation(row.observation)} = ${verdict(row.status)} := by`);
    push('  decide');
    push('');
  });
  push('/-! ### Seeded mutations of `classify`, killed by named recorded rows -/');
  push('');
  for (const entry of killers) {
    push(`/-- Mutation \`${entry.mutant}\` changes \`${entry.row.name}\` (${entry.why}). -/`);
    push(`theorem mutation_${entry.mutant}_killed :`);
    push(`    ${entry.name} ${observation(entry.row.observation)}${entry.args} ≠ ${verdict(entry.row.status)} := by`);
    push('  decide');
    push('');
  }
  push('end NobleFoundation.SymbolicSolver.RefusalsReplay');
  return `${lines.join('\n')}\n`;
}

/* ---------- The encoder corpus module ---------- */

function renderEncoder(receipts, root) {
  const { solver, queries, digests } = receipts;
  const pairs = parseCorpus(fs.readFileSync(path.join(root, CORPUS), 'utf8'));
  const byName = new Map(pairs.map(pair => [pair.name, pair]));
  assert.equal(queries.corpus_sha256, solver.source.corpus_sha256, 'the query file binds the receipt corpus');
  for (const claim of solver.claims) {
    for (const mode of ['sliced', 'unsliced']) {
      const row = queries.queries.find(entry => entry.pair === claim.name && entry.mode === mode);
      assert.ok(row, `${claim.name}/${mode}: the query file records the emitted text`);
      assert.equal(row.sha256, claim.formula[mode].sha256, `${claim.name}/${mode}: the text is the receipt's query`);
      assert.equal(Buffer.byteLength(row.text), claim.formula[mode].bytes, `${claim.name}/${mode}: the text is the receipt's bytes`);
    }
  }
  const lines = [];
  const push = (...added) => lines.push(...added);
  push('import NobleFoundation.SymbolicEncoder');
  push('import NobleFoundation.SymbolicShrink');
  push('');
  push('/-!');
  push('# The emitted #56 queries, byte for byte, and the encoder\'s simulation on their subjects');
  push('');
  push('Generated by `verification/symbolic/solver-lean.mjs` from');
  push(`\`${digests.solver.path}\` (sha256 \`${digests.solver.sha256}\`), its`);
  push(`\`${digests.queries.path}\` (sha256 \`${digests.queries.sha256}\`) and`);
  push(`\`${digests.endpoints.path}\` (sha256 \`${digests.endpoints.sha256}\`); do not edit by hand.`);
  push('`verification/ngraph/symbolic-pair-inventory.mjs` refuses when this file is not the');
  push('generator\'s output for those receipts.');
  push('');
  push('For every corpus pair the kernel decides two text equalities — the sliced and unsliced query');
  push('the lane emitted is `renderQuery`\'s output — and states the encoder simulation on that');
  push('subject: the assertion\'s claim, read through the emitted bodies, is the model\'s refutation');
  push('predicate for every 64-bit input. The seeded `sub`-operand-swap fault is shown not to be');
  push('simulated by the subtraction-order subject.');
  push('-/');
  push('');
  push('namespace NobleFoundation.SymbolicSolver.EncoderCorpus');
  push('');
  push('open NobleFoundation.PackedSchedule');
  push('open NobleFoundation.SymbolicEncoder');
  push('open NobleFoundation.SymbolicPair');
  push('open NobleFoundation.SymbolicShrink (inputsOf)');
  push('');
  push('set_option maxRecDepth 40000');
  push('');
  for (const entry of queries.queries) {
    if (entry.mode !== 'sliced') continue;
    const pair = byName.get(entry.pair);
    assert.ok(pair, `${entry.pair}: the corpus states the pair`);
    push(`/-- \`${pair.name}\` as the lane encodes it: both node lists and the ordered outputs. -/`);
    push(`def ${pair.name}Subject : Subject :=`);
    push(`  ⟨${nodeList(pair.left)}, ${nodeList(pair.right)}, [${pair.outputs.join(', ')}]⟩`);
    push('');
  }
  push('/-! ### The emitted queries are the model\'s rendering -/');
  push('');
  for (const entry of queries.queries) {
    const slice = entry.mode === 'sliced';
    const suffix = slice ? 'query_text' : 'query_text_unsliced';
    push(`/-- The ${entry.mode} query for \`${entry.pair}\` is byte for byte \`renderQuery\`'s output. -/`);
    push(`theorem ${entry.pair}_${suffix} :`);
    push(`    renderQuery ${entry.pair}Subject ${slice} = ${quote(entry.text)} := by`);
    push('  decide');
    push('');
  }
  push('/-! ### The encoder simulation on every corpus subject -/');
  push('');
  for (const entry of queries.queries) {
    if (entry.mode !== 'sliced') continue;
    push(`/-- \`${entry.pair}\`'s assertion is the model's refutation predicate, for every input. -/`);
    push(`theorem ${entry.pair}_claim_simulates (inputs : Nat → BitVec 64) :`);
    push(`    claimValue ${entry.pair}Subject inputs = refuted (subjectPair ${entry.pair}Subject) inputs :=`);
    push('  claim_simulates _ inputs (EarlierWithin.mp (by decide)) (EarlierWithin.mp (by decide))');
    push('    (OutputsWithin.mp (by decide))');
    push('');
  }
  const fault = byName.get('subtractionOrder');
  assert.ok(fault, 'the subtraction-order subject is in the corpus');
  assert.ok(fault.left.some(value => value.op === 'sub'), 'the subtraction-order left graph has a `sub` node');
  push('/-- The seeded `sub`-operand-swap encoder fault is not simulated by the subtraction-order');
  push('left graph at `(5, 7)`: the faulted body computes `7 - 5` where the graph computes `5 - 7`. -/');
  push('theorem subtractionOrder_sub_swap_fault_not_simulated :');
  push('    bodyValueSubSwapped (nodeValues subtractionOrderSubject.left (inputsOf [5, 7])) (inputsOf [5, 7])');
  push('        (.sub 0 1) ≠');
  push('      nodeValues subtractionOrderSubject.left (inputsOf [5, 7]) 2 := by');
  push('  decide');
  push('');
  push('end NobleFoundation.SymbolicSolver.EncoderCorpus');
  return `${lines.join('\n')}\n`;
}

/* ---------- Entry points ---------- */

/** Read the three retained files and render both modules. Every argument is a repository path. */
export function generate(solverPath, queriesPath, endpointsPath, root = ROOT) {
  const readJson = relative => {
    const bytes = fs.readFileSync(path.join(root, relative));
    return { value: JSON.parse(bytes.toString('utf8')), sha256: sha(bytes), path: relative };
  };
  const solver = readJson(solverPath);
  const queries = readJson(queriesPath);
  const endpoints = readJson(endpointsPath);
  assert.equal(solver.value.schema, 'noble-symbolic-solver-receipt/v1', `${solverPath}: solver receipt schema`);
  assert.equal(queries.value.schema, 'noble-symbolic-queries/v1', `${queriesPath}: queries schema`);
  assert.equal(endpoints.value.schema, 'noble-symbolic-endpoints-receipt/v1', `${endpointsPath}: endpoints receipt schema`);
  assert.equal(queries.sha256, solver.value.queries?.sha256, `${queriesPath}: the receipt names these query bytes`);
  const receipts = { solver: solver.value, queries: queries.value, endpoints: endpoints.value,
    digests: { solver: { path: solverPath, sha256: solver.sha256 },
      queries: { path: queriesPath, sha256: queries.sha256 },
      endpoints: { path: endpointsPath, sha256: endpoints.sha256 } } };
  return { [REFUSALS_MODULE]: renderRefusals(receipts), [ENCODER_MODULE]: renderEncoder(receipts, root) };
}

function main() {
  const argv = process.argv.slice(2);
  const flags = {};
  for (let at = 0; at < argv.length; at += 1) {
    const flag = argv[at];
    if (flag === '--check' || flag === '--write') {
      flags[flag] = true;
      continue;
    }
    assert.ok(['--solver', '--queries', '--endpoints'].includes(flag), `unexpected argument ${flag}`);
    assert.ok(at + 1 < argv.length, `${flag} needs a value`);
    flags[flag] = argv[at + 1];
    at += 1;
  }
  assert.ok(flags['--solver'] && flags['--queries'] && flags['--endpoints'],
    'usage: solver-lean.mjs --solver S --queries Q --endpoints E --check|--write');
  assert.equal(Number('--check' in flags) + Number('--write' in flags), 1, 'pass exactly one of --check and --write');
  const rendered = generate(flags['--solver'], flags['--queries'], flags['--endpoints']);
  const report = {};
  for (const [relative, text] of Object.entries(rendered)) {
    if ('--write' in flags) {
      fs.writeFileSync(path.join(ROOT, relative), text);
      report[relative] = { written: true, bytes: Buffer.byteLength(text), sha256: sha(text) };
    } else {
      const committed = fs.readFileSync(path.join(ROOT, relative), 'utf8');
      report[relative] = { fresh: committed === text, committed_sha256: sha(committed), generated_sha256: sha(text) };
      assert.equal(committed, text, `${relative} is not this generator's output for the retained receipts`);
    }
  }
  process.stdout.write(`${JSON.stringify(report)}\n`);
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main();
