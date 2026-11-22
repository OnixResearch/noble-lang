#!/usr/bin/env node
// PAUTO-CASE-02..06 acceptance gate (`.cairn/changes/proof-automation-diagnostics`, task 4).
//
// Builds the production `noble` binary from a clean checkout, then runs the five case designs of
// the change's design table through `noble verify` (hand proofs, `--counterexample boundary` and a
// supplied candidate), `noble goal-view`, `noble obligations`, `noble verify-module` and
// `noble release`. Every source, command line and report is retained. The design rows are
// embedded below and digested into the receipt. A case is promotable only when every registered
// expectation was observed; this gate promotes nothing by itself and is not a proof.
//
// Usage (the pinned Lean sandbox tools must exist; the consumer compiles `proofs/mc1` itself):
//   node verification/proof-automation-cases/gate.mjs --out NEW_DIRECTORY
//   node verification/proof-automation-cases/gate.mjs --self-test
import assert from 'node:assert/strict';
import { execFileSync, spawnSync } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = fs.realpathSync(path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..'));
const SCOPE = ['crates', 'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', 'proofs/mc1',
  'verification/mc1/wrap.noble-contract', 'verification/mc1/wrap-proof.lean',
  'verification/mc1/monotonic.noble-contract', 'verification/mc1/increment.noble-contract',
  'verification/mc1/increment-proof.lean', 'verification/mc2/contracts/q-one.noble',
  'verification/owner-law/laws/owner-law-v1.json', 'verification/proof-automation-cases'];
const CASE_IDS = ['PAUTO-CASE-02', 'PAUTO-CASE-03', 'PAUTO-CASE-04', 'PAUTO-CASE-05', 'PAUTO-CASE-06'];
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const hash = file => sha(fs.readFileSync(file));

// The design rows of `.cairn/changes/proof-automation-diagnostics/design.md` (PAUTO-CASE-02..06).
const DESIGNS = {
  'PAUTO-CASE-02': {
    title: 'signed-overflow edge',
    requirements: ['PAUTO-CEX-01', 'VC-NUM-01', 'VC-TOOL-02'],
    input: '`verification/mc1/wrap.noble-contract` (`9223372036854775807 1 +` -> `-9223372036854775808`) with the hand MC1 proof `verification/mc1/wrap-proof.lean`; no template applies to this closed program',
    expected: 'goal view shows the wrap explicitly; proof accepted; no unbounded-integer statement appears; `proved`, 0',
  },
  'PAUTO-CASE-03': {
    title: 'false postcondition + replayed counterexample',
    requirements: ['PAUTO-CEX-01', 'VC-TOOL-02', 'VC-NUM-01'],
    input: '`verification/mc1/monotonic.noble-contract` with the boundary generator',
    expected: 'candidate `x=9223372036854775807`; semantic refutation template accepted against `¬claim`; compiled run observes `y=-9223372036854775808`; report has input, output, source identity, violated `(lt (in x) (out y))`; `disproved/semantic-refutation` + `observed-execution`, 1',
  },
  'PAUTO-CASE-04': {
    title: 'model-only refutation',
    requirements: ['PAUTO-CEX-01', 'VC-TOOL-02'],
    input: 'the same contract; the producer returns a model (stub: the supplied candidate `input:5`) for which the refutation template fails',
    expected: 'reported as candidate only; not `disproved`; `unknown/candidate-model-only`, 3',
  },
  'PAUTO-CASE-05': {
    title: 'impossible precondition',
    requirements: ['PAUTO-CEX-01', 'VC-LOGIC-02', 'VC-OWNER-01'],
    input: 'MC1 `[ 1 + ]` with `(requires false)` and a hand proof; the same precondition in an intrinsic rev-1 module',
    expected: 'proof accepted but shown `checked-vacuous`; an owner-law release refuses; intrinsic rev-1 refuses earlier with `selected unary I64 rule requires typed True precondition`; `proved/checked-vacuous`, 0 (MC1); `error`, 2 (rev-1)',
  },
  'PAUTO-CASE-06': {
    title: 'unsupported goal',
    requirements: ['PAUTO-CEX-01', 'PAUTO-PRODUCER-01', 'VC-TOOL-02'],
    input: '`ensures` with a text literal; and a rev-1 contract with two inputs',
    expected: 'no producer launched; span of the unsupported construct reported; `unsupported`, 4',
  },
};

const REV1_PROOF = '(export-unary-I64 (intro (tail Stack) (intro (x I64) (pc-sequence (pc-exact (exec-literal 1)) (pc-exact (exec-add)) (bridge (by-exact-append-assoc)) (join (by-exact-result))))) (mc1-true-eq-wrap))';
const TWO_INPUT_CONTRACT = 'contract 1 add-law [ subject add input [ a I64 b I64 ] output [ y I64 ] requires [ true ] ensures [ (eq (out y) (add (in a) (in b))) ] ]';
const SOURCES = {
  'vacuous.noble-contract': '(contract 1 vacuous-increment\n  (input (x I64))\n  (output (y I64))\n  (program [ 1 + ])\n  (requires false)\n  (ensures (eq (out y) (add (in x) 1))))\n',
  'vacuous-proof.lean': 'import MC1Obligation\nopen NobleContracts\nnamespace MC1Proof\ntheorem proof : MC1Obligation.claim :=\n  fun _tail _before _params _inputs _parameters hpre => absurd hpre (holds_false _ _ _)\nend MC1Proof\n',
  'text.noble-contract': '(contract 1 text-goal\n  (input (x I64))\n  (output (y I64))\n  (program [ 1 + ])\n  (requires true)\n  (ensures (eq (out y) "one")))\n',
  'rev1-vacuous.noble': `module arithmetic@1 [ export increment def increment [ 1 + ] contract 1 increment-law [ subject increment input [ x I64 ] output [ y I64 ] requires [ false ] ensures [ (eq (out y) (add (in x) 1)) ] ] proof 1 increment-correct for increment-law [ ${REV1_PROOF} ] ]\n`,
  'rev1-two-inputs.noble': `module arithmetic@1 [ export add def add [ + ] ${TWO_INPUT_CONTRACT} proof 1 add-correct for add-law [ (export-unary-I64 (intro (tail Stack) (intro (x I64) (pc-sequence (pc-exact (exec-add)) (join (by-exact-result))))) (mc1-true-eq-wrap)) ] ]\n`,
};

function argument(name) {
  const index = process.argv.indexOf(name);
  return index > 0 ? process.argv[index + 1] : undefined;
}

function git(...args) {
  return execFileSync('git', args, { cwd: root, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 });
}

// A case is `passed` only when every registered expectation was observed. `unmet` records an
// observed difference and `failed` a run that could not observe. Only `passed` is promotable.
function deriveVerdicts(observations) {
  const caseOf = id => observations[id]?.case ?? 'not-executed';
  return {
    promotable: CASE_IDS.filter(id => caseOf(id) === 'passed'),
    unmet: CASE_IDS.filter(id => caseOf(id) === 'unmet'),
    failed: CASE_IDS.filter(id => caseOf(id) === 'failed'),
    not_executed: CASE_IDS.filter(id => caseOf(id) === 'not-executed'),
  };
}

// The claim text must stay over `BitVec 64` values: once the `BitVec.ofInt` coercions are
// removed, no `Int` or `Nat` arithmetic may remain in the program, predicates or claim.
function unboundedMentions(statement) {
  const body = statement.split('\n').filter(line => !/^def (irRevision|semanticRevision) : Nat :=/.test(line))
    .join('\n').replaceAll('BitVec.ofInt', '');
  return body.match(/\b(Int|Nat)\b/g) ?? [];
}

if (process.argv.includes('--self-test')) {
  const all = Object.fromEntries(CASE_IDS.map(id => [id, { case: 'passed' }]));
  assert.deepEqual(deriveVerdicts(all).promotable, CASE_IDS);
  const mixed = { ...all, 'PAUTO-CASE-03': { case: 'unmet' }, 'PAUTO-CASE-05': { case: 'failed' } };
  assert.deepEqual(deriveVerdicts(mixed).unmet, ['PAUTO-CASE-03']);
  assert.deepEqual(deriveVerdicts(mixed).failed, ['PAUTO-CASE-05']);
  const { 'PAUTO-CASE-06': _dropped, ...missing } = all;
  assert.deepEqual(deriveVerdicts(missing).not_executed, ['PAUTO-CASE-06']);
  assert.deepEqual(unboundedMentions('def irRevision : Nat := 1\ndef x : Op := .lit (.i64 (BitVec.ofInt 64 (1)))'), []);
  assert.deepEqual(unboundedMentions('def claim : Prop := ∀ n : Int, n + 1 > n'), ['Int']);
  assert.equal(new Set(Object.values(SOURCES)).size, Object.keys(SOURCES).length, 'duplicate fixture');
  assert.ok(SOURCES['rev1-two-inputs.noble'].includes(TWO_INPUT_CONTRACT));
  console.log('gate self-test: only fully observed cases are promotable; the unbounded-integer scan rejects Int claims');
  process.exit(0);
}

function sourceSnapshot() {
  const dirty = git('status', '--porcelain', '--untracked-files=all', '--', ...SCOPE);
  assert.equal(dirty, '', `source scope is not clean:\n${dirty}`);
  const files = git('ls-files', '-z', '--', ...SCOPE).split('\0').filter(Boolean).sort();
  const snapshot = {};
  for (const file of files) snapshot[file] = { sha256: hash(path.join(root, file)) };
  return snapshot;
}

const raw = [];
function recordRaw(output, name, text) {
  const file = path.join(output, 'raw', `${name}.txt`);
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, text);
  raw.push({ name, file: path.relative(output, file), sha256: sha(text), bytes: Buffer.byteLength(text) });
}

function buildNoble(output) {
  const build = spawnSync('cargo', ['build', '--locked', '-p', 'noble-cli', '--bin', 'noble',
    '--message-format=json-render-diagnostics'],
  { cwd: root, encoding: 'utf8', maxBuffer: 256 * 1024 * 1024 });
  recordRaw(output, 'build.stdout', build.stdout ?? '');
  recordRaw(output, 'build.stderr', build.stderr ?? '');
  assert.equal(build.status, 0, `cargo build failed:\n${build.stderr}`);
  const artifacts = build.stdout.split('\n').filter(Boolean).map(line => JSON.parse(line))
    .filter(message => message.reason === 'compiler-artifact'
      && message.target?.name === 'noble' && message.executable);
  assert.equal(artifacts.length, 1, 'expected one noble executable artifact');
  return { executable: artifacts[0].executable, executable_sha256: hash(artifacts[0].executable) };
}

// The reviewed Lean sandbox tools the MC1 consumer selects, and the repository library.
const REVIEWED = {
  NOBLE_LEAN: '/nix/store/kwvw84h9r7w0q40d3cm5xgfz0rpv7yzc-noble-lean-release-4.35.0/bin/lean',
  NOBLE_BWRAP: '/nix/store/y0ra9qr3rz81d9wl7dfrldv3j25dc98q-bubblewrap-0.12.0/bin/bwrap',
  NOBLE_PRLIMIT: '/nix/store/mqvbf0flamqaq9c3ihb496aahag897n0-util-linux-2.42.3-bin/bin/prlimit',
  NOBLE_SYSTEMD_RUN: '/nix/store/baxgs06659w4i6ggsfr56srjrb52v1h8-systemd-261.2/bin/systemd-run',
  NOBLE_CONTRACT_LIBRARY: path.join(root, 'proofs/mc1'),
};
// The largest proof timeout `noble verify` accepts; the consumer compiles the whole MC1 library.
const TIMEOUT_MS = '600000';

class Mismatch extends Error {}
function canonical(value) {
  if (Array.isArray(value)) return value.map(canonical);
  if (value && typeof value === 'object') {
    return Object.fromEntries(Object.keys(value).sort().map(key => [key, canonical(value[key])]));
  }
  return value;
}
function expectValue(label, actual, expected) {
  if (JSON.stringify(canonical(actual)) !== JSON.stringify(canonical(expected))) {
    throw new Mismatch(`${label}: observed ${JSON.stringify(actual)}, registered ${JSON.stringify(expected)}`);
  }
}
function expectIncludes(label, text, parts) {
  for (const part of parts) {
    if (typeof text !== 'string' || !text.includes(part)) {
      throw new Mismatch(`${label}: ${JSON.stringify(text)} lacks ${JSON.stringify(part)}`);
    }
  }
}
async function observe(body) {
  try {
    return { case: 'passed', ...(await body()) };
  } catch (error) {
    return error instanceof Mismatch
      ? { case: 'unmet', diagnostic: error.message }
      : { case: 'failed', diagnostic: error.stack ?? String(error) };
  }
}

const out = argument('--out');
assert.ok(out, 'pass --out NEW_DIRECTORY');
assert.equal(fs.existsSync(out), false, '--out must not already exist');
fs.mkdirSync(out, { mode: 0o700 });
for (const directory of ['home', 'tmp', 'sources', 'emitted']) fs.mkdirSync(path.join(out, directory));
const environment = {
  PATH: '/run/current-system/sw/bin', HOME: path.join(out, 'home'), TMPDIR: path.join(out, 'tmp'),
  LANG: 'C.UTF-8', LC_ALL: 'C.UTF-8', XDG_RUNTIME_DIR: process.env.XDG_RUNTIME_DIR, ...REVIEWED,
};

const snapshot = sourceSnapshot();
const sourceRevision = `sha256:${sha(JSON.stringify(snapshot))}`;
const testedCommit = git('rev-parse', 'HEAD').trim();
const designDigest = sha(JSON.stringify(canonical(DESIGNS)));
const built = buildNoble(out);
const binary = path.join(out, 'noble');
fs.copyFileSync(built.executable, binary);
fs.chmodSync(binary, 0o500);
const binarySha = hash(binary);

const sourceFile = {};
for (const [name, text] of Object.entries(SOURCES)) {
  const file = path.join(out, 'sources', name);
  fs.writeFileSync(file, text, { flag: 'wx', mode: 0o400 });
  sourceFile[name] = file;
}
const mc1 = name => path.join(root, 'verification/mc1', name);
const relative = item => (path.isAbsolute(item) && !path.relative(out, item).startsWith('..')
  ? path.relative(out, item) : item.startsWith(`${root}/`) ? path.relative(root, item) : item);

const runs = [];
function run(label, args) {
  const result = spawnSync(binary, args, { cwd: out, env: environment, encoding: 'utf8',
    maxBuffer: 64 * 1024 * 1024, timeout: 1_800_000 });
  recordRaw(out, `${label}.stdout`, result.stdout ?? '');
  if (result.stderr) recordRaw(out, `${label}.stderr`, result.stderr);
  runs.push({ label, args: args.map(relative), status: result.status, signal: result.signal });
  let report = null;
  try { report = JSON.parse((result.stdout ?? '').trim()); } catch { report = null; }
  if (report === null) throw new Error(`${label}: no JSON report (status ${result.status}, signal ${result.signal})`);
  return { status: result.status, report };
}
const diagnosticCodes = report => (report.diagnostics ?? []).map(entry => entry.code);
const observations = {};

observations['PAUTO-CASE-02'] = await observe(async () => {
  const view = run('case02-goal-view', ['goal-view', mc1('wrap.noble-contract')]);
  const proof = run('case02-verify', ['verify', mc1('wrap.noble-contract'), '--proof', mc1('wrap-proof.lean'),
    '--timeout-ms', TIMEOUT_MS]);
  expectValue('goal-view status', view.status, 0);
  expectValue('goal-view acceptance', view.report.acceptance, false);
  expectValue('goal-view arithmetic', view.report.arithmetic?.i64, "two's-complement 64-bit wrapping");
  const site = view.report.sites?.[0] ?? {};
  expectValue('goal-view postcondition text', site.predicates?.post?.source_text, '(eq (out y) -9223372036854775808)');
  expectIncludes('goal-view statement', site.statement, ['.lit (.i64 (BitVec.ofInt 64 (9223372036854775807)))',
    '.word 4', '.i64 (-9223372036854775808)']);
  expectValue('goal-view unbounded-integer mentions', unboundedMentions(site.statement ?? ''), []);
  expectValue('verify status', proof.status, 0);
  expectValue('verify outcome', proof.report.outcome, 'proved');
  expectValue('verify theorem', proof.report.theorem, 'MC1Proof.proof');
  expectValue('verify recheck', proof.report.independent_recheck ?? proof.report.rechecked, true);
  expectValue('verify arithmetic model', proof.report.arithmetic_model, 'i64-wrapping');
  expectValue('verify statement equals the goal view statement', proof.report.generated_statement, site.statement);
  expectValue('verify unbounded-integer mentions', unboundedMentions(proof.report.generated_statement ?? ''), []);
  return { outcome: proof.report.outcome, axioms: proof.report.assumptions?.accepted_transitive_axioms ?? null,
    statement_sha256: sha(proof.report.generated_statement) };
});

observations['PAUTO-CASE-03'] = await observe(async () => {
  const { status, report } = run('case03-verify', ['verify', mc1('monotonic.noble-contract'),
    '--counterexample', 'boundary', '--timeout-ms', TIMEOUT_MS]);
  const record = report.counterexample ?? {};
  expectValue('status', status, 1);
  expectValue('outcome', report.outcome, 'disproved');
  expectValue('theorem', report.theorem, 'MC1Proof.refutation');
  expectValue('recheck', report.independent_recheck ?? report.rechecked, true);
  expectValue('producer', record.producer?.id, 'boundary-value');
  expectValue('producer candidates', record.producer?.candidates,
    [{ input: '9223372036854775807', semantic_output: '-9223372036854775808', wrapped: true }]);
  expectValue('category', record.category, 'semantic-refutation');
  expectValue('advertised as', record.advertised_as, 'executable counterexample');
  expectValue('arithmetic shown', record.arithmetic, "two's-complement 64-bit wrapping; values are signed");
  expectValue('candidate', record.candidate,
    { input: '9223372036854775807', semantic_output: '-9223372036854775808', wrapped: true });
  expectValue('violated predicate', record.violated, 'postcondition');
  expectValue('postcondition text', record.postcondition?.declared?.text, '(lt (in x) (out y))');
  expectValue('semantic evaluation', record.postcondition?.semantic_evaluation, 'false');
  expectValue('observed evaluation', record.postcondition?.observed_evaluation, 'false');
  const observation = record.observation ?? {};
  expectValue('evidence class', observation.evidence_class, 'observed-execution');
  expectValue('observed outcome', observation.outcome, 'normal');
  expectValue('observed output', observation.output, '-9223372036854775808');
  expectValue('observed source', observation.source, '9223372036854775807 1 +\n');
  for (const field of ['source_sha256', 'wat_sha256', 'wasm_sha256']) {
    expectValue(`observed ${field} form`, /^[0-9a-f]{64}$/.test(observation[field] ?? ''), true);
  }
  expectValue('observed source identity', observation.source_sha256, sha(observation.source));
  expectValue('discrepancy', record.discrepancy, null);
  expectValue('subject identity present', report.subject !== null && typeof report.subject === 'object', true);
  return { candidate: record.candidate, observation: { output: observation.output,
    source_sha256: observation.source_sha256, wat_sha256: observation.wat_sha256,
    wasm_sha256: observation.wasm_sha256 }, axioms: report.assumptions?.accepted_transitive_axioms ?? null };
});

observations['PAUTO-CASE-04'] = await observe(async () => {
  const { status, report } = run('case04-verify', ['verify', mc1('monotonic.noble-contract'),
    '--counterexample', 'input:5', '--timeout-ms', TIMEOUT_MS]);
  const record = report.counterexample ?? {};
  expectValue('status', status, 3);
  expectValue('outcome', report.outcome, 'unknown');
  expectValue('theorem', report.theorem ?? null, null);
  expectValue('producer', record.producer?.id, 'supplied-input');
  expectValue('category', record.category, 'candidate-model-only');
  expectValue('advertised as', record.advertised_as, 'candidate');
  expectValue('candidate', record.candidate, { input: '5', semantic_output: '6', wrapped: false });
  expectValue('no violated predicate', record.violated, null);
  expectValue('semantic evaluation', record.postcondition?.semantic_evaluation, 'true');
  expectValue('consumer refusal', diagnosticCodes(report).includes('proof-rejected'), true);
  return { candidate: record.candidate, diagnostics: diagnosticCodes(report) };
});

observations['PAUTO-CASE-05'] = await observe(async () => {
  const map = run('case05-obligations', ['obligations', sourceFile['vacuous.noble-contract']]);
  expectValue('obligation map status', map.status, 0);
  expectValue('obligation map vacuity', map.report.obligations?.[0]?.vacuity, 'suspected');
  const proof = run('case05-verify', ['verify', sourceFile['vacuous.noble-contract'], '--proof',
    sourceFile['vacuous-proof.lean'], '--timeout-ms', TIMEOUT_MS]);
  expectValue('verify status', proof.status, 0);
  expectValue('verify outcome', proof.report.outcome, 'proved');
  expectValue('verify vacuity status', proof.report.vacuity?.status, 'suspected');
  expectValue('verify vacuity category', proof.report.vacuity?.category, 'checked-vacuous');
  // The owner-law release needs an honestly built artifact; the v1 increment build supplies it.
  const destination = path.join(out, 'emitted', 'increment');
  const build = run('case05-honest-build', ['build', '--require-proof',
    path.join(root, 'verification/mc2/contracts/q-one.noble'), '--contract', mc1('increment.noble-contract'),
    '--proof', mc1('increment-proof.lean'), '--timeout-ms', TIMEOUT_MS, '--out', destination]);
  assert.equal(build.status, 0, 'the honest increment build must succeed');
  assert.equal(build.report.release?.allowed, true, 'the honest increment build must allow release');
  const law = path.join(root, 'verification/owner-law/laws/owner-law-v1.json');
  const release = run('case05-release', ['release', '--law', law, '--law-digest', hash(law),
    '--source', path.join(root, 'verification/mc2/contracts/q-one.noble'),
    '--contract', sourceFile['vacuous.noble-contract'], '--proof', sourceFile['vacuous-proof.lean'],
    '--artifact', path.join(destination, 'artifact.wasm'), '--timeout-ms', TIMEOUT_MS]);
  expectValue('release status', release.status, 1);
  expectValue('release admission', release.report.admission, 'rejected');
  expectValue('release refusal', release.report.refusal?.code, 'owner-law-requires-mismatch');
  const module = run('case05-verify-module', ['verify-module', sourceFile['rev1-vacuous.noble'],
    '--timeout-ms', TIMEOUT_MS]);
  expectValue('rev-1 status', module.status, 2);
  expectValue('rev-1 outcome', module.report.outcome, 'error');
  expectIncludes('rev-1 diagnostic', module.report.diagnostic?.message,
    ['selected unary I64 rule requires typed True precondition']);
  return { vacuity: proof.report.vacuity, release_refusal: release.report.refusal?.code,
    rev1: module.report.diagnostic };
});

observations['PAUTO-CASE-06'] = await observe(async () => {
  const text = run('case06-verify', ['verify', sourceFile['text.noble-contract'], '--counterexample', 'boundary',
    '--timeout-ms', TIMEOUT_MS]);
  const textStart = SOURCES['text.noble-contract'].indexOf('"');
  expectValue('text status', text.status, 4);
  expectValue('text outcome', text.report.outcome, 'unsupported');
  expectValue('text diagnostic', diagnosticCodes(text.report), ['contract-unsupported']);
  expectValue('text span', text.report.diagnostics?.[0]?.details, { start: textStart, end: textStart + 1 });
  expectValue('no producer launched', Object.hasOwn(text.report, 'counterexample'), false);
  const module = run('case06-verify-module', ['verify-module', sourceFile['rev1-two-inputs.noble'],
    '--timeout-ms', TIMEOUT_MS]);
  const contractStart = SOURCES['rev1-two-inputs.noble'].indexOf(TWO_INPUT_CONTRACT);
  expectValue('rev-1 status', module.status, 4);
  expectValue('rev-1 outcome', module.report.outcome, 'unsupported');
  expectValue('rev-1 category', module.report.category, 'outside-profile');
  expectValue('rev-1 diagnostic code', module.report.diagnostic?.code, 'intrinsic-source-unsupported');
  expectValue('rev-1 span', module.report.diagnostic?.details,
    { start: contractStart, end: contractStart + Buffer.byteLength(TWO_INPUT_CONTRACT) });
  expectValue('rev-1 recheck', module.report.independent_recheck, false);
  return { text: text.report.diagnostics?.[0] ?? null, rev1: module.report.diagnostic };
});

const { promotable, unmet, failed, not_executed: notExecuted } = deriveVerdicts(observations);
const binaryAfter = hash(binary);
const binaryChanged = binaryAfter !== binarySha;
const verdictPromotable = binaryChanged ? [] : promotable;
const failures = [...unmet.map(id => `${id}: ${observations[id].diagnostic}`),
  ...failed.map(id => `${id}: ${observations[id].diagnostic?.split('\n')[0]}`),
  ...(binaryChanged ? ['the executed binary changed during the run; no observation is promotable'] : [])];
const acceptance = {
  schema: 'noble-proof-automation-cases-acceptance/v1',
  claim: 'selected executions of PAUTO-CASE-02..06 through `noble verify`, `noble goal-view`, `noble obligations`, '
    + '`noble verify-module` and `noble release` on one CLI built from the recorded clean commit; promotion '
    + 'verdicts are derived from the observations, not declared here',
  result: failed.length || binaryChanged ? 'failed' : 'completed',
  verdict: failures.length === 0 && notExecuted.length === 0 ? 'all-passed'
    : failed.length || binaryChanged ? 'run-failure' : 'incomplete',
  failures,
  tested_commit: testedCommit,
  source_revision: sourceRevision,
  scope: SCOPE,
  source: snapshot,
  designs: { sha256: designDigest, rows: DESIGNS },
  binary: { built: built.executable_sha256, executed_sha256: binarySha, executed_sha256_after: binaryAfter },
  build: 'cargo build --locked -p noble-cli --bin noble',
  environment: REVIEWED,
  fixtures: Object.fromEntries(Object.entries(SOURCES).map(([name, text]) => [name, sha(text)])),
  runs,
  raw_outputs: raw,
  observations,
  promotable: verdictPromotable,
  unmet,
  failed,
  not_executed: notExecuted,
  non_claims: [
    'No proof is claimed for the producer, the host evaluator or the compiled backend: `disproved` rests only on the pinned Lean consumer accepting the reviewed refutation template instantiated at the candidate; the consumer and the MC1 library are explicit assumptions (VC-GATE-03).',
    'The compiled run is test-class `observed-execution` evidence for one input; it never decides an outcome, and a disagreement would be reported for artifact correspondence (#10) without resolving anything.',
    'Vacuity detection covers only the literal `false` precondition; other unsatisfiable preconditions stay not-assessed.',
    'The supplied candidate stands in for an external solver model (the SMT profile is not selected); registration in conformance JSON and the requirement sync stay with the change\'s review task.',
  ],
};
const receiptPath = path.join(out, 'acceptance.json');
fs.writeFileSync(receiptPath, `${JSON.stringify(acceptance, null, 2)}\n`);
console.log(JSON.stringify({ receipt: receiptPath, receipt_sha256: hash(receiptPath), result: acceptance.result,
  verdict: acceptance.verdict, failures, tested_commit: testedCommit, source_revision: sourceRevision,
  promotable: verdictPromotable, not_executed: notExecuted }, null, 1));
if (failed.length || binaryChanged) process.exitCode = 1;
