#!/usr/bin/env node
// IMPL-CASE acceptance gate (`.cairn/changes/implicit-proof-arguments`).
//
// Builds the production `noble` binary from a clean checkout, then runs the registered
// IMPL-CASE designs in `specs/conformance/implicit-cases.json` through `noble verify-module`
// (and, for IMPL-CASE-20, framed declared-module `noble session`s). A positive case is accepted
// only by the unchanged explicit checker and the pinned Lean consumer (`independent_recheck`);
// a refusal is observed with its outcome, diagnostic code and message. Every source, command
// line and report is retained. A case is promotable only when every registered expectation was
// observed; this gate promotes nothing by itself and is not a proof.
//
// Usage (with `proofs/mc1` built for the contract library):
//   node verification/implicit-cases/gate.mjs --out NEW_DIRECTORY [--determinism-runs N] [--jobs N]
//   node verification/implicit-cases/gate.mjs --self-test
import assert from 'node:assert/strict';
import { execFileSync, spawn, spawnSync } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = fs.realpathSync(path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..'));
const SCOPE = ['crates', 'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', 'proofs/mc1',
  'verification/implicit-cases', 'specs/conformance/implicit-cases.json'];
const CASE_FILE = 'specs/conformance/implicit-cases.json';
const REVISION = '0.1.0-draft.5';
const CASE_IDS = Array.from({ length: 20 }, (_, index) => `IMPL-CASE-${String(index + 1).padStart(2, '0')}`);
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const hash = file => sha(fs.readFileSync(file));

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

// Proof modules. Each case's sources are these exact texts; spans in registered messages are
// computed from them, so a fixture edit cannot silently keep an old expectation.
const POLY = 'proof 1 poly-refl : [ (Pi (A Type0) (Pi (x A) (Eq A x x))) ] [ (intro (A Type0) (intro (x A) (refl x))) ]';
const BASE = 'proof 1 base : [ (Pi (x I64) (Pi (h (Eq I64 x x)) (Pi (A Type0) (Pi (y (Pair (List A) I64)) (Eq (Pair (List A) I64) y y))))) ] [ (intro (x I64) (intro (h (Eq I64 x x)) (intro (A Type0) (intro (y (Pair (List A) I64)) (refl y))))) ]';
const TWICE = 'proof 1 twice-refl : [ (Pi (x I64) (Pi (h (Eq I64 x x)) (Eq I64 x x))) ] [ (intro (x I64) (intro (h (Eq I64 x x)) (refl x))) ]';
const CONST = 'proof 1 const-refl : [ (Pi (A Type0) (Pi (B Type0) (Pi (x A) (Eq A x x)))) ] [ (intro (A Type0) (intro (B Type0) (intro (x A) (refl x)))) ]';
const T3 = 'proof 1 t : [ (Pi (A Type0) (Pi (x A) (Pi (y A) (Eq A y y)))) ] [ (intro (A Type0) (intro (x A) (intro (y A) (refl y)))) ]';
const T2 = 'proof 1 t2 : [ (Pi (A Type0) (Pi (x A) (Pi (y A) (Eq A x x)))) ] [ (intro (A Type0) (intro (x A) (intro (y A) (refl x)))) ]';
const logic = (...members) => `module logic@1 [ ${members.join(' ')} ]`;
const EXPORTER = 'module logic@1 [ proof 1 poly-refl : [ (Pi (A Type0) (Pi (x A) (Eq A x x))) ] [ (intro (A Type0) (intro (x A) (refl x))) ] export poly-refl ]';
const EXPORTER2 = 'module logic@2 [ proof 1 poly-refl : [ (Pi (A Type0) (Pi (B Type0) (Pi (x A) (Eq A x x)))) ] [ (intro (A Type0) (intro (B Type0) (intro (x A) (refl x)))) ] export poly-refl ]';
const IMPORT1 = 'import logic@1 as alias';
const IMPORT2 = 'import logic@2 as alias';
const LAWS = name => `module ${name}@1 [ proof 1 assoc : [ (Pi (A Type0) (Pi (x A) (Eq A x x))) ] [ (intro (A Type0) (intro (x A) (refl x))) ] export assoc ]`;
const P1_CLAIM = '[ (Pi (x (List I64)) (Eq (List I64) x x)) ]';
const P1 = `module imported@1 [ proof 1 imported-list infer : ${P1_CLAIM} [ (intro (x (List I64)) (app (use alias.poly-refl) x)) ] ]`;
const P1_EXPLICIT = `module imported@1 [ proof 1 imported-list : ${P1_CLAIM} [ (intro (x (List I64)) (apply (apply (use alias.poly-refl) (List I64)) x)) ] ]`;
const P2_CLAIM = '[ (Pi (x I64) (Pi (z (Pair (List I64) I64)) (Eq (Pair (List I64) I64) z z))) ]';
const P2 = logic(BASE, `proof 1 derived infer : ${P2_CLAIM} [ (intro (x I64) (intro (z (Pair (List I64) I64)) (app (use base) x (refl x) z))) ]`);
const P2_EXPLICIT = logic(BASE, `proof 1 derived : ${P2_CLAIM} [ (intro (x I64) (intro (z (Pair (List I64) I64)) (apply (apply (apply (apply (use base) x) (refl x)) I64) z))) ]`);
const P3_LAW = 'contract 1 law [ subject selected input [ x I64 ] output [ y I64 ] requires [ true ] ensures [ (eq (out y) (add (in x) 1)) ] ]';
const P3_PROOF = literal => `[ (export-unary-I64 (intro (tail Stack) (intro (x I64) (pc-sequence (pc-exact (exec-literal ${literal})) (pc-exact (exec-add)) (bridge (by-exact-append-assoc)) (join (by-exact-result))))) (mc1-true-eq-wrap)) ]`;
const P3 = `module selected@1 [ export selected def selected [ 1 + ] ${P3_LAW} proof 1 verified infer for law ${P3_PROOF('(infer)')} ]`;
const P3_EXPLICIT = `module selected@1 [ export selected def selected [ 1 + ] ${P3_LAW} proof 1 verified for law ${P3_PROOF('1')} ]`;
const P3_FREE = `module selected@1 [ export selected def selected [ 1 + ] ${P3_LAW} ]`;
const CONTEXT_CODE = logic(POLY, 'proof 1 context infer : [ (Pi (B Type0) (Pi (y (List B)) (Eq (List B) y y))) ] [ (intro (B Type0) (intro (y (List B)) (app (use poly-refl) y))) ]');

// IMPL-CASE-18: a head with `n` Type0 and `n` value binders, used under `n` intros.
function deepHead(n) {
  const ids = Array.from({ length: n }, (_, index) => index + 1);
  let claim = '(Eq A1 x1 x1)';
  let proof = '(refl x1)';
  for (const at of [...ids].reverse()) {
    claim = `(Pi (x${at} A${at}) ${claim})`;
    proof = `(intro (x${at} A${at}) ${proof})`;
  }
  for (const at of [...ids].reverse()) {
    claim = `(Pi (A${at} Type0) ${claim})`;
    proof = `(intro (A${at} Type0) ${proof})`;
  }
  let goal = '(Eq I64 y1 y1)';
  let use = `(app (use deep) ${ids.map(at => `y${at}`).join(' ')})`;
  for (const at of [...ids].reverse()) {
    goal = `(Pi (y${at} I64) ${goal})`;
    use = `(intro (y${at} I64) ${use})`;
  }
  return logic(`proof 1 deep : [ ${claim} ] [ ${proof} ]`, `proof 1 deep-use infer : [ ${goal} ] [ ${use} ]`);
}
function nestedHead(n) {
  let claim = `(Pi (x A${n}) (Eq A${n} x x))`;
  let proof = `(intro (x A${n}) (refl x))`;
  for (let at = n; at >= 1; at -= 1) {
    claim = `(Pi (A${at} Type0) ${claim})`;
    proof = `(intro (A${at} Type0) ${proof})`;
  }
  return logic(`proof 1 nested : [ ${claim} ] [ ${proof} ]`);
}
// IMPL-CASE-19: `count` local premises of the placeholder's proposition.
function manyPremises(count) {
  const ids = Array.from({ length: count }, (_, index) => index + 1);
  const claim = `(Pi (x I64) ${ids.map(at => `(Pi (h${at} (Eq I64 x x)) `).join('')}(Pi (z (Pair (List I64) I64)) (Eq (Pair (List I64) I64) z z))${')'.repeat(count)})`;
  const term = `(intro (x I64) ${ids.map(at => `(intro (h${at} (Eq I64 x x)) `).join('')}(intro (z (Pair (List I64) I64)) (app (use base) x (infer) z))${')'.repeat(count)})`;
  return logic(BASE, `proof 1 premises infer : [ ${claim} ] [ ${term} ]`);
}
// IMPL-CASE-19: one Type0 binder constrained by `count` arguments, the last of the wrong code.
function manyConstraints(count) {
  const ids = Array.from({ length: count }, (_, index) => index + 1);
  let claim = '(Eq A x1 x1)';
  let proof = '(refl x1)';
  for (const at of [...ids].reverse()) {
    claim = `(Pi (x${at} A) ${claim})`;
    proof = `(intro (x${at} A) ${proof})`;
  }
  const values = ids.slice(0, -1).map(at => `y${at}`);
  let goal = '(Pi (w Bool) (Eq I64 y1 y1))';
  let use = `(intro (w Bool) (app (use many) ${values.join(' ')} w))`;
  for (const name of [...values].reverse()) {
    goal = `(Pi (${name} I64) ${goal})`;
    use = `(intro (${name} I64) ${use})`;
  }
  return logic(`proof 1 many : [ (Pi (A Type0) ${claim}) ] [ (intro (A Type0) ${proof}) ]`,
    `proof 1 clash infer : [ ${goal} ] [ ${use} ]`);
}
function wideClash(depth) {
  const tree = level => level === 0 ? 'I64' : `(Pair ${tree(level - 1)} ${tree(level - 1)})`;
  const wide = tree(depth);
  const pair = `(Pair ${wide} I64)`;
  const sum = `(Sum ${wide} I64)`;
  return logic(T2, `proof 1 wide infer : [ (Pi (u ${pair}) (Pi (v ${sum}) (Eq ${pair} u u))) ] [ (intro (u ${pair}) (intro (v ${sum}) (app (use t2) u v))) ]`);
}

const SOURCES = {
  'p1': P1, 'p1-explicit': P1_EXPLICIT, 'exporter': EXPORTER, 'exporter-2': EXPORTER2,
  'import-1': IMPORT1, 'import-2': IMPORT2, 'laws-a': LAWS('lawsA'), 'laws-b': LAWS('lawsB'),
  'import-a': 'import lawsA@1 as a', 'import-b': 'import lawsB@1 as b',
  'p1-context': CONTEXT_CODE, 'p2': P2, 'p2-explicit': P2_EXPLICIT, 'p3': P3, 'p3-explicit': P3_EXPLICIT,
  'underconstrained': logic(CONST, 'proof 1 c05 infer : [ (Pi (x I64) (Eq I64 x x)) ] [ (intro (x I64) (app (use const-refl) x)) ]'),
  'premise-two': logic(TWICE, 'proof 1 c06 infer : [ (Pi (x I64) (Pi (h1 (Eq I64 x x)) (Pi (h2 (Eq I64 x x)) (Eq I64 x x)))) ] [ (intro (x I64) (intro (h1 (Eq I64 x x)) (intro (h2 (Eq I64 x x)) (app (use twice-refl) x (infer))))) ]'),
  'premise-one': logic(TWICE, 'proof 1 c06 infer : [ (Pi (x I64) (Pi (h1 (Eq I64 x x)) (Eq I64 x x))) ] [ (intro (x I64) (intro (h1 (Eq I64 x x)) (app (use twice-refl) x (infer)))) ]'),
  'two-imports': 'module user@1 [ proof 1 c07 infer : [ (Pi (x I64) (Eq I64 x x)) ] [ (intro (x I64) (app (use a.assoc) x)) ] ]',
  'two-imports-unqualified': 'module user@1 [ proof 1 c07u infer : [ (Pi (x I64) (Eq I64 x x)) ] [ (intro (x I64) (app (use assoc) x)) ] ]',
  'meta-only': logic(T3, 'proof 1 c08 infer : [ (Pi (w I64) (Eq I64 w w)) ] [ (intro (w I64) (apply (apply (apply (use t) (infer)) (infer)) w)) ]'),
  'wrong-value': logic(BASE, 'proof 1 c09 infer : [ (Pi (x I64) (Pi (w Bool) (Eq Bool w w))) ] [ (intro (x I64) (intro (w Bool) (app (use base) x (refl x) w))) ]'),
  'forged-argument': logic(POLY, 'proof 1 c10 : [ (Pi (x (List I64)) (Eq (List I64) x x)) ] [ (intro (x (List I64)) (apply (apply (use poly-refl) I64) x)) ]'),
  'code-resource': logic(POLY, 'proof 1 c11 : [ (Pi (x I64) (Eq I64 x x)) ] [ (intro (x I64) (apply (apply (use poly-refl) (Resource test.counter)) x)) ]'),
  'code-program': logic(POLY, 'proof 1 c11 : [ (Pi (x I64) (Eq I64 x x)) ] [ (intro (x I64) (apply (apply (use poly-refl) (Program (Stack I64) (Stack I64))) x)) ]'),
  'code-syntax': logic(POLY, 'proof 1 c11 : [ (Pi (x I64) (Eq I64 x x)) ] [ (intro (x I64) (apply (apply (use poly-refl) Syntax) x)) ]'),
  'capture-apply': logic(T3, 'proof 1 c12 infer : [ (Pi (w I64) (Eq I64 w w)) ] [ (apply (apply (use t) (infer)) (infer)) ]'),
  'capture-app': logic(T3, 'proof 1 c12 infer : [ (Pi (w I64) (Eq I64 w w)) ] [ (app (use t) (infer)) ]'),
  'rebase': logic(BASE, 'proof 1 c13 infer : [ (Pi (C Type0) (Pi (D Type0) (Pi (E Type0) (Pi (x I64) (Pi (z (Pair (List C) I64)) (Eq (Pair (List C) I64) z z)))))) ] [ (intro (C Type0) (intro (D Type0) (intro (E Type0) (intro (x I64) (intro (z (Pair (List C) I64)) (app (use base) x (refl x) z)))))) ]'),
  'rebase-off-by-two': logic(BASE, 'proof 1 c13 : [ (Pi (C Type0) (Pi (D Type0) (Pi (E Type0) (Pi (x I64) (Pi (z (Pair (List C) I64)) (Eq (Pair (List C) I64) z z)))))) ] [ (intro (C Type0) (intro (D Type0) (intro (E Type0) (intro (x I64) (intro (z (Pair (List C) I64)) (apply (apply (apply (apply (use base) x) (refl x)) E) z)))))) ]'),
  'changed-claim-stored': logic(POLY, 'proof 1 c15 : [ (Pi (x (List Bool)) (Eq (List Bool) x x)) ] [ (intro (x (List Bool)) (apply (apply (use poly-refl) (List I64)) x)) ]'),
  'changed-claim': logic(POLY, 'proof 1 c15 infer : [ (Pi (x (List Bool)) (Eq (List Bool) x x)) ] [ (intro (x (List Bool)) (app (use poly-refl) x)) ]'),
  'hole-premise': logic(BASE, `proof 1 c16 infer : ${P2_CLAIM} [ (intro (x I64) (intro (z (Pair (List I64) I64)) (app (use base) x ?h z))) ]`),
  'hole-code': `module imported@1 [ proof 1 imported-list infer : ${P1_CLAIM} [ (intro (x (List I64)) (apply (apply (use alias.poly-refl) ?A) x)) ] ]`,
  'hole-value': `module imported@1 [ proof 1 imported-list infer : ${P1_CLAIM} [ (intro (x (List I64)) (app (use alias.poly-refl) ?x)) ] ]`,
  'nested-200': nestedHead(200),
  'deep-20': deepHead(20),
  'premises-50': manyPremises(50),
  'constraints-20': manyConstraints(20),
  'wide-clash': wideClash(9),
  'p3-free': P3_FREE,
};

if (process.argv.includes('--self-test')) {
  const all = Object.fromEntries(CASE_IDS.map(id => [id, { case: 'passed' }]));
  assert.deepEqual(deriveVerdicts(all).promotable, CASE_IDS);
  const mixed = { ...all, 'IMPL-CASE-03': { case: 'unmet' }, 'IMPL-CASE-17': { case: 'failed' } };
  const derived = deriveVerdicts(mixed);
  assert.deepEqual(derived.unmet, ['IMPL-CASE-03']);
  assert.deepEqual(derived.failed, ['IMPL-CASE-17']);
  const { 'IMPL-CASE-20': _dropped, ...missing } = all;
  assert.deepEqual(deriveVerdicts(missing).not_executed, ['IMPL-CASE-20']);
  assert.equal(new Set(Object.values(SOURCES)).size, Object.keys(SOURCES).length, 'duplicate fixture');
  assert.ok(Buffer.byteLength(SOURCES['wide-clash']) < 65_536, 'wide clash exceeds the source limit');
  console.log('gate self-test: only fully observed cases are promotable; fixtures are distinct');
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

function cases() {
  const packet = JSON.parse(fs.readFileSync(path.join(root, CASE_FILE), 'utf8'));
  assert.equal(packet.revision, REVISION);
  const byId = new Map(packet.cases.map(one => [one.id, one]));
  for (const id of CASE_IDS) assert.ok(byId.has(id), `case ${id} missing`);
  const project = id => {
    const { profile, kind, requirements, input, expected } = byId.get(id);
    return { id, profile, kind, requirements, input, expected };
  };
  return { byId, project };
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

// The reviewed Lean sandbox tools the intrinsic-proof gate selects, and the repository library.
const REVIEWED = {
  NOBLE_LEAN: '/nix/store/kwvw84h9r7w0q40d3cm5xgfz0rpv7yzc-noble-lean-release-4.35.0/bin/lean',
  NOBLE_BWRAP: '/nix/store/y0ra9qr3rz81d9wl7dfrldv3j25dc98q-bubblewrap-0.12.0/bin/bwrap',
  NOBLE_PRLIMIT: '/nix/store/mqvbf0flamqaq9c3ihb496aahag897n0-util-linux-2.42.3-bin/bin/prlimit',
  NOBLE_SYSTEMD_RUN: '/nix/store/baxgs06659w4i6ggsfr56srjrb52v1h8-systemd-261.2/bin/systemd-run',
  NOBLE_CONTRACT_LIBRARY: path.join(root, 'proofs/mc1'),
};

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
const determinismRuns = Number(argument('--determinism-runs') ?? 100);
const jobs = Number(argument('--jobs') ?? 4);
assert.ok(Number.isInteger(determinismRuns) && determinismRuns >= 2, 'determinism runs');
assert.ok(Number.isInteger(jobs) && jobs >= 1, 'jobs');
for (const directory of ['home', 'tmp', 'sources', 'emit']) fs.mkdirSync(path.join(out, directory));
const environment = {
  PATH: '/run/current-system/sw/bin', HOME: path.join(out, 'home'), TMPDIR: path.join(out, 'tmp'),
  LANG: 'C.UTF-8', LC_ALL: 'C.UTF-8', XDG_RUNTIME_DIR: process.env.XDG_RUNTIME_DIR, ...REVIEWED,
};
assert.ok(fs.existsSync(path.join(REVIEWED.NOBLE_CONTRACT_LIBRARY, '.lake/build/lib/lean/NobleContracts.olean')),
  'build proofs/mc1 before running the gate');

const snapshot = sourceSnapshot();
const sourceRevision = `sha256:${sha(JSON.stringify(snapshot))}`;
const testedCommit = git('rev-parse', 'HEAD').trim();
const { byId, project } = cases();
const projections = CASE_IDS.map(project);
const projectionDigest = sha(JSON.stringify(projections));
const built = buildNoble(out);
const binary = path.join(out, 'noble');
fs.copyFileSync(built.executable, binary);
fs.chmodSync(binary, 0o500);
const binarySha = hash(binary);
const expectedOf = id => byId.get(id).expected;

// Every fixture is written once; a run names the files it submits.
const sourceFile = {};
for (const [name, text] of Object.entries(SOURCES)) {
  const file = path.join(out, 'sources', `${name}.noble`);
  fs.writeFileSync(file, `${text}\n`, { flag: 'wx' });
  sourceFile[name] = file;
}
for (const [name, text] of [['import', 'import selected@1 as s'], ['run', '41 s.selected'],
  ['quote', '[ s.selected ] reflect'], ['compose', '41 [ s.selected ] [ 1 + ] compose run']]) {
  const file = path.join(out, 'sources', `frame-${name}.txt`);
  fs.writeFileSync(file, text, { flag: 'wx' });
  sourceFile[`frame-${name}`] = file;
}
fs.writeFileSync(path.join(out, 'sources', 'empty-bindings.txt'), '');
// A span of `needle` inside a fixture, as the CLI renders it.
function spanOf(name, needle, from = 'last') {
  const text = SOURCES[name];
  const start = from === 'last' ? text.lastIndexOf(needle) : text.indexOf(needle);
  assert.ok(start >= 0, `${needle} not in ${name}`);
  return `${start}..${start + Buffer.byteLength(needle)}`;
}

const runs = [];
function verifyArgs(target, modules, options) {
  return ['verify-module', sourceFile[target], ...modules.flatMap(name => ['--module', sourceFile[name]]),
    '--timeout-ms', '600000', ...options];
}
function viewOf(status, stdout) {
  let report = null;
  try { report = JSON.parse(stdout.trim()); } catch { report = null; }
  const elaborated = report?.elaboration?.proofs ?? [];
  return {
    status,
    report,
    outcome: report?.outcome ?? null,
    code: report?.diagnostic?.code ?? null,
    message: report?.diagnostic?.message ?? null,
    category: report?.category ?? null,
    recheck: report?.independent_recheck ?? null,
    guest_requests: report?.guest_requests ?? null,
    protected_operations: report?.protected_operations ?? null,
    records: elaborated.map(proof => ({ explicit: proof.explicit,
      inserted: proof.inserted.map(({ binder, position, head, solution }) => ({ binder, position, head, solution })) })),
    claims: Object.fromEntries((report?.proofs ?? []).map(proof => [proof.name, proof.claim])),
    work: report?.work ?? null,
    elaboration_work: report?.elaboration?.work ?? null,
  };
}
// One `verify-module` run, retained in full.
function verify(label, target, modules = [], options = []) {
  const args = verifyArgs(target, modules, options);
  const run = spawnSync(binary, args, { cwd: out, env: environment, encoding: 'utf8',
    maxBuffer: 64 * 1024 * 1024, timeout: 900_000 });
  recordRaw(out, `verify-${runs.length + 1}-${label}.stdout`, run.stdout ?? '');
  if (run.stderr) recordRaw(out, `verify-${runs.length + 1}-${label}.stderr`, run.stderr);
  runs.push({ label, args: args.map(item => path.relative(out, item).startsWith('..') ? item : path.relative(out, item)),
    status: run.status, signal: run.signal });
  return viewOf(run.status, run.stdout ?? '');
}
// The same, concurrently, for the determinism repetitions.
function verifyAsync(label, target, modules) {
  const args = verifyArgs(target, modules, []);
  return new Promise(resolveRun => {
    const child = spawn(binary, args, { cwd: out, env: environment, stdio: ['ignore', 'pipe', 'pipe'] });
    let stdout = '';
    let stderr = '';
    child.stdout.on('data', chunk => { stdout += chunk; });
    child.stderr.on('data', chunk => { stderr += chunk; });
    child.on('close', (status, signal) => {
      recordRaw(out, `determinism-${label}.stdout`, stdout);
      if (stderr) recordRaw(out, `determinism-${label}.stderr`, stderr);
      runs.push({ label: `determinism-${label}`, args: args.map(item => path.relative(out, item)), status, signal });
      resolveRun(viewOf(status, stdout));
    });
  });
}
const proved = (label, view) => {
  expectValue(`${label} outcome`, { outcome: view.outcome, recheck: view.recheck, code: view.code },
    { outcome: 'proved', recheck: true, code: null });
};
const refused = (label, view, expected) => {
  expectValue(`${label} outcome`, { outcome: view.outcome, code: view.code, category: view.category },
    { outcome: expected.outcome, code: expected.code, category: expected.category });
  expectIncludes(`${label} message`, view.message, expected.message_includes);
};
const quiet = (label, view) => expectValue(`${label} counters`,
  { guest_requests: view.guest_requests, protected_operations: view.protected_operations },
  { guest_requests: 0, protected_operations: 0 });
const record = view => view.records.at(-1);
const observations = {};

// Positive pairs: the abbreviated form elaborates to exactly the registered explicit term, the
// independent recheck accepts it, and the hand-written explicit form checks against the same claim.
async function positivePair(id, abbreviated, explicit, modules, name) {
  const expected = expectedOf(id);
  const view = verify(`${id}-abbreviated`, abbreviated, modules);
  proved(id, view);
  quiet(id, view);
  expectValue(`${id} expansion`, record(view), { explicit: expected.explicit, inserted: expected.inserted });
  const control = verify(`${id}-explicit`, explicit, modules);
  proved(`${id} explicit control`, control);
  expectValue(`${id} explicit control has no record`, control.records, []);
  expectValue(`${id} same claim`, view.claims[name], control.claims[name]);
  return { abbreviated: record(view), claim: view.claims[name], elaboration_work: view.elaboration_work };
}

observations['IMPL-CASE-01'] = await observe(() => positivePair('IMPL-CASE-01', 'p1', 'p1-explicit',
  ['exporter', 'import-1'], 'imported-list'));

observations['IMPL-CASE-02'] = await observe(async () => {
  const expected = expectedOf('IMPL-CASE-02');
  const view = verify('IMPL-CASE-02', 'p1-context');
  proved('IMPL-CASE-02', view);
  quiet('IMPL-CASE-02', view);
  expectValue('expansion', record(view), { explicit: expected.explicit, inserted: expected.inserted });
  return { abbreviated: record(view) };
});

observations['IMPL-CASE-03'] = await observe(() => positivePair('IMPL-CASE-03', 'p2', 'p2-explicit', [], 'derived'));

observations['IMPL-CASE-04'] = await observe(async () => {
  const expected = expectedOf('IMPL-CASE-04');
  const view = verify('IMPL-CASE-04-abbreviated', 'p3');
  proved('IMPL-CASE-04', view);
  quiet('IMPL-CASE-04', view);
  expectValue('inserted', record(view).inserted, expected.inserted);
  expectIncludes('explicit', record(view).explicit, [expected.explicit_includes]);
  const control = verify('IMPL-CASE-04-explicit', 'p3-explicit');
  proved('IMPL-CASE-04 explicit control', control);
  expectValue('same claim', view.claims.verified, control.claims.verified);
  return { abbreviated: record(view) };
});

// Refusal cases: one run per registered variant.
async function refusals(id, variants) {
  const expected = expectedOf(id);
  const rows = {};
  for (const [variant, target, modules = []] of variants) {
    const view = verify(`${id}-${variant}`, target, modules);
    refused(`${id} ${variant}`, view, expected.variants[variant]);
    quiet(`${id} ${variant}`, view);
    rows[variant] = { outcome: view.outcome, code: view.code, category: view.category, message: view.message };
  }
  return { variants: rows };
}

observations['IMPL-CASE-05'] = await observe(() => refusals('IMPL-CASE-05', [['underconstrained', 'underconstrained']]));

observations['IMPL-CASE-06'] = await observe(async () => {
  const id = 'IMPL-CASE-06';
  const expected = expectedOf(id);
  const two = verify(`${id}-two`, 'premise-two');
  const one = verify(`${id}-one`, 'premise-one');
  for (const [label, view] of [['two', two], ['one', one]]) {
    refused(`${id} ${label}`, view, expected.variants[label]);
    quiet(`${id} ${label}`, view);
  }
  // Each candidate row names its intro binder span; nothing is chosen.
  expectIncludes('two candidates', two.message, [
    `local premise \`h1\` at ${spanOf('premise-two', '(h1 (Eq I64 x x))')}`,
    `local premise \`h2\` at ${spanOf('premise-two', '(h2 (Eq I64 x x))')}`]);
  expectIncludes('one candidate', one.message, [`local premise \`h1\` at ${spanOf('premise-one', '(h1 (Eq I64 x x))')}`]);
  expectValue('one candidate lists no second', one.message.includes('h2'), false);
  return { two: two.message, one: one.message };
});

observations['IMPL-CASE-07'] = await observe(async () => {
  const id = 'IMPL-CASE-07';
  const expected = expectedOf(id);
  const forward = verify(`${id}-a-then-b`, 'two-imports', ['laws-a', 'laws-b', 'import-a', 'import-b']);
  const swapped = verify(`${id}-b-then-a`, 'two-imports', ['laws-b', 'laws-a', 'import-b', 'import-a']);
  proved(`${id} a-then-b`, forward);
  proved(`${id} b-then-a`, swapped);
  expectValue('expansion', record(forward), { explicit: expected.explicit, inserted: expected.inserted });
  expectValue('import order changes nothing', record(swapped), record(forward));
  const bare = verify(`${id}-unqualified`, 'two-imports-unqualified', ['laws-a', 'laws-b', 'import-a', 'import-b']);
  refused(`${id} unqualified`, bare, expected.variants.unqualified);
  return { qualified: record(forward), unqualified: bare.message };
});

observations['IMPL-CASE-08'] = await observe(() => refusals('IMPL-CASE-08', [['meta-only', 'meta-only']]));

observations['IMPL-CASE-09'] = await observe(async () => {
  const result = await refusals('IMPL-CASE-09', [['wrong-value', 'wrong-value']]);
  // The listed origin is the clashing argument `w` of the spine.
  const w = SOURCES['wrong-value'].lastIndexOf(' w)))') + 1;
  expectIncludes('origin span', result.variants['wrong-value'].message, [`constrained at ${w}..${w + 1}`]);
  return result;
});

observations['IMPL-CASE-10'] = await observe(() => refusals('IMPL-CASE-10', [['forged-argument', 'forged-argument']]));

observations['IMPL-CASE-11'] = await observe(() => refusals('IMPL-CASE-11',
  [['resource', 'code-resource'], ['program', 'code-program'], ['syntax', 'code-syntax']]));

observations['IMPL-CASE-12'] = await observe(() => refusals('IMPL-CASE-12',
  [['apply', 'capture-apply'], ['app', 'capture-app']]));

observations['IMPL-CASE-13'] = await observe(async () => {
  const id = 'IMPL-CASE-13';
  const expected = expectedOf(id);
  const view = verify(`${id}-rebased`, 'rebase');
  proved(id, view);
  expectValue('rebased expansion', record(view), { explicit: expected.explicit, inserted: expected.inserted });
  const off = verify(`${id}-off-by-two`, 'rebase-off-by-two');
  refused(`${id} off-by-two`, off, expected.variants['off-by-two']);
  return { rebased: record(view), off_by_two: off.message };
});

observations['IMPL-CASE-14'] = await observe(async () => {
  const id = 'IMPL-CASE-14';
  const expected = expectedOf(id);
  const old = verify(`${id}-logic1`, 'p1', ['exporter', 'import-1']);
  proved(`${id} logic@1`, old);
  expectValue('logic@1 head', record(old).inserted.map(row => row.head), [expected.recorded_head]);
  const replay = verify(`${id}-replay`, 'p1-explicit', ['exporter-2', 'import-2']);
  refused(`${id} replay`, replay, expected.variants.replay);
  const reelaborate = verify(`${id}-reelaborate`, 'p1', ['exporter-2', 'import-2']);
  refused(`${id} re-elaboration`, reelaborate, expected.variants.reelaborate);
  const both = verify(`${id}-both`, 'p1', ['exporter', 'exporter-2', 'import-1']);
  proved(`${id} both versions`, both);
  expectValue('old evidence stays bound to logic@1', record(both), record(old));
  return { logic1: record(old), replay: replay.message, reelaborate: reelaborate.message };
});

observations['IMPL-CASE-15'] = await observe(async () => {
  const id = 'IMPL-CASE-15';
  const expected = expectedOf(id);
  const stored = verify(`${id}-stored`, 'changed-claim-stored');
  refused(`${id} stored expansion`, stored, expected.variants.stored);
  const fresh = verify(`${id}-reelaborated`, 'changed-claim');
  proved(`${id} re-elaboration`, fresh);
  expectValue('new expansion', record(fresh), { explicit: expected.explicit, inserted: expected.inserted });
  return { stored: stored.message, reelaborated: record(fresh) };
});

observations['IMPL-CASE-16'] = await observe(() => refusals('IMPL-CASE-16', [['premise', 'hole-premise'],
  ['code', 'hole-code', ['exporter', 'import-1']], ['value', 'hole-value', ['exporter', 'import-1']]]));

observations['IMPL-CASE-17'] = await observe(async () => {
  const id = 'IMPL-CASE-17';
  const expected = expectedOf(id);
  // Each pair alternates two dependency-module orders around an unrelated module.
  const orders = {
    P1: [['laws-b', 'exporter', 'import-1'], ['exporter', 'import-1', 'laws-b']],
    P2: [['laws-a', 'laws-b'], ['laws-b', 'laws-a']],
    P3: [['laws-a', 'laws-b'], ['laws-b', 'laws-a']],
  };
  const targets = { P1: 'p1', P2: 'p2', P3: 'p3' };
  const tasks = [];
  for (const pair of Object.keys(targets)) {
    for (let index = 0; index < determinismRuns; index += 1) {
      tasks.push({ pair, index, modules: orders[pair][index % 2] });
    }
  }
  const results = [];
  let next = 0;
  await Promise.all(Array.from({ length: jobs }, async () => {
    while (next < tasks.length) {
      const task = tasks[next];
      next += 1;
      const view = await verifyAsync(`${task.pair}-${task.index + 1}`, targets[task.pair], task.modules);
      results.push({ ...task, view });
    }
  }));
  const summary = {};
  for (const pair of Object.keys(targets)) {
    const rows = results.filter(row => row.pair === pair);
    const digests = new Set(rows.map(row => sha(JSON.stringify(canonical(row.view.report?.elaboration ?? null)))));
    summary[pair] = {
      runs: rows.length,
      proved: rows.filter(row => row.view.outcome === 'proved' && row.view.recheck === true).length,
      distinct_records: digests.size,
      record_sha256: [...digests][0],
      orders: orders[pair].length,
    };
  }
  expectValue('determinism', Object.fromEntries(Object.entries(summary)
    .map(([pair, row]) => [pair, { runs: row.runs, proved: row.proved, distinct_records: row.distinct_records }])),
  Object.fromEntries(Object.keys(targets).map(pair => [pair,
    { runs: determinismRuns, proved: determinismRuns, distinct_records: expected.distinct_records }])));
  return { summary };
});

observations['IMPL-CASE-18'] = await observe(async () => {
  const id = 'IMPL-CASE-18';
  const expected = expectedOf(id);
  const nested = verify(`${id}-nested-200`, 'nested-200');
  refused(`${id} nested 200`, nested, expected.variants['nested-200']);
  const full = verify(`${id}-deep-20`, 'deep-20');
  proved(`${id} deep head`, full);
  // Needs come from the accepted run: elaboration's own charge and the combined request.
  const need = { normalization: { elaboration: full.elaboration_work.normalization, request: full.work.normalization },
    substitution: { elaboration: full.elaboration_work.substitution, request: full.work.substitution } };
  const rows = {};
  for (const budget of ['normalization', 'substitution']) {
    const flag = `--${budget}-work`;
    const below = verify(`${id}-${budget}-elaboration-1`, 'deep-20', [], [flag, String(need[budget].elaboration - 1)]);
    refused(`${id} ${budget} below elaboration`, below, expected.variants[`${budget}-elaboration`]);
    const between = verify(`${id}-${budget}-request-1`, 'deep-20', [], [flag, String(need[budget].request - 1)]);
    refused(`${id} ${budget} below request`, between, expected.variants[`${budget}-request`]);
    const exact = verify(`${id}-${budget}-exact`, 'deep-20', [], [flag, String(need[budget].request)]);
    proved(`${id} ${budget} exact`, exact);
    rows[budget] = { need: need[budget], below_elaboration: below.message, below_request: between.message };
  }
  return { nested: nested.message, budgets: rows };
});

observations['IMPL-CASE-19'] = await observe(async () => {
  const id = 'IMPL-CASE-19';
  const expected = expectedOf(id);
  const premises = verify(`${id}-premises-50`, 'premises-50');
  refused(`${id} candidates`, premises, expected.variants.candidates);
  expectValue('candidates listed', (premises.message.match(/local premise `h\d+` at \d+\.\.\d+/gu) ?? []).length,
    expected.listed_candidates);
  const constraints = verify(`${id}-constraints-20`, 'constraints-20');
  refused(`${id} constraints`, constraints, expected.variants.constraints);
  expectValue('constraint spans listed', (constraints.message.match(/\d+\.\.\d+/gu) ?? []).length,
    expected.listed_constraint_spans);
  const wide = verify(`${id}-wide-clash`, 'wide-clash');
  refused(`${id} wide clash`, wide, expected.variants['wide-clash']);
  const terms = [...wide.message.matchAll(/code (\(.*?\.\.\.\[\d+ more bytes\]) does not match code (\(.*?\.\.\.\[\d+ more bytes\])/gu)];
  expectValue('both rendered terms cut', terms.length === 1 && terms[0].length === 3, true);
  const kept = terms[0].slice(1).map(term => Buffer.byteLength(term.replace(/\.\.\.\[\d+ more bytes\]$/u, '')));
  expectValue('rendered term bound', kept.every(bytes => bytes <= expected.term_bytes), true);
  expectValue('refusal bound', Buffer.byteLength(wide.message) <= expected.message_bytes, true);
  return { premises: premises.message, constraints: constraints.message, wide_bytes: Buffer.byteLength(wide.message),
    wide_kept_term_bytes: kept };
});

// IMPL-CASE-20: ordinary run, quote and compose of a module's definition with the abbreviated,
// the explicit and no proof.
function session(label, module) {
  const files = [sourceFile[module], sourceFile['frame-import'], sourceFile['frame-run'],
    sourceFile['frame-quote'], sourceFile['frame-compose']];
  const input = Buffer.concat(files.flatMap(file => {
    const bytes = fs.readFileSync(file);
    return [Buffer.from(`${bytes.length}\n`), bytes];
  }));
  const emit = path.join(out, 'emit', label);
  const args = ['session', '--framed', '--declared-modules', '--bindings',
    path.join(out, 'sources', 'empty-bindings.txt'), '--opt', 'off', '--emit', emit];
  const run = spawnSync(binary, args, { cwd: out, env: environment, input, encoding: 'utf8',
    maxBuffer: 64 * 1024 * 1024, timeout: 900_000 });
  recordRaw(out, `session-${label}.stdout`, run.stdout ?? '');
  if (run.stderr) recordRaw(out, `session-${label}.stderr`, run.stderr);
  runs.push({ label: `session-${label}`, args: args.map(item => path.relative(out, item).startsWith('..') ? item : path.relative(out, item)),
    status: run.status, signal: run.signal });
  const lines = (run.stdout ?? '').trimEnd().split('\n');
  const wasm = fs.readdirSync(path.join(emit, 'engine')).filter(file => file.endsWith('.wasm')).sort()
    .map(file => ({ file, sha256: hash(path.join(emit, 'engine', file)) }));
  return { status: run.status, lines, wasm };
}
observations['IMPL-CASE-20'] = await observe(async () => {
  const expected = expectedOf('IMPL-CASE-20');
  const forms = { abbreviated: session('abbreviated', 'p3'), explicit: session('explicit', 'p3-explicit'),
    'proof-free': session('proof-free', 'p3-free') };
  const frames = form => forms[form].lines.slice(2);
  const link = form => {
    const report = JSON.parse(forms[form].lines[0]);
    const mask = value => JSON.parse(JSON.stringify(value).replaceAll(sha(fs.readFileSync(sourceFile[
      { abbreviated: 'p3', explicit: 'p3-explicit', 'proof-free': 'p3-free' }[form]])), 'SOURCE'));
    return { outcome: report.outcome, contracts: mask(report.contracts ?? []).map(({ definition, definition_identity,
      accepted_recipe }) => ({ definition, definition_identity, accepted_recipe })),
    intrinsic_proof: mask(report.intrinsic_proof ?? null) };
  };
  for (const form of Object.keys(forms)) expectValue(`${form} status`, forms[form].status, 0);
  expectValue('run/quote/compose stacks', frames('abbreviated').map(line => JSON.parse(line).stack),
    expected.stacks);
  expectValue('frames identical (explicit)', frames('explicit'), frames('abbreviated'));
  expectValue('frames identical (proof-free)', frames('proof-free'), frames('abbreviated'));
  expectValue('Wasm identical (explicit)', forms.explicit.wasm, forms.abbreviated.wasm);
  expectValue('Wasm identical (proof-free)', forms['proof-free'].wasm, forms.abbreviated.wasm);
  expectValue('executable identity', link('explicit').contracts, link('abbreviated').contracts);
  expectValue('executable identity (proof-free)', link('proof-free').contracts, link('abbreviated').contracts);
  expectValue('proof evidence identical up to source', link('explicit').intrinsic_proof,
    link('abbreviated').intrinsic_proof);
  for (const line of frames('abbreviated')) {
    const report = JSON.parse(line);
    expectValue('runtime prover calls', report.runtime_prover_calls, 0);
  }
  return { frames_sha256: sha(frames('abbreviated').join('\n')), wasm: forms.abbreviated.wasm,
    link: link('abbreviated').contracts };
});

const { promotable, unmet, failed, not_executed: notExecuted } = deriveVerdicts(observations);
const binaryAfter = hash(binary);
const binaryChanged = binaryAfter !== binarySha;
const verdictPromotable = binaryChanged ? [] : promotable;
const failures = [...unmet.map(id => `${id}: ${observations[id].diagnostic}`),
  ...failed.map(id => `${id}: ${observations[id].diagnostic?.split('\n')[0]}`),
  ...(binaryChanged ? ['the executed binary changed during the run; no observation is promotable'] : [])];
const acceptance = {
  schema: 'noble-implicit-cases-acceptance/v1',
  claim: 'selected executions of IMPL-CASE-01..20 through `noble verify-module` and declared-module '
    + '`noble session`s on one CLI built from the recorded clean commit; promotion verdicts are derived '
    + 'from the observations, not declared here',
  result: failed.length || binaryChanged ? 'failed' : 'completed',
  verdict: failures.length === 0 && notExecuted.length === 0 ? 'all-passed'
    : failed.length || binaryChanged ? 'run-failure' : 'incomplete',
  failures,
  tested_commit: testedCommit,
  source_revision: sourceRevision,
  scope: SCOPE,
  source: snapshot,
  case_file: CASE_FILE,
  projections: { sha256: projectionDigest, ids: CASE_IDS },
  binary: { built: built.executable_sha256, executed_sha256: binarySha, executed_sha256_after: binaryAfter },
  build: 'cargo build --locked -p noble-cli --bin noble',
  environment: REVIEWED,
  determinism: { runs_per_pair: determinismRuns, jobs },
  fixtures: Object.fromEntries(Object.entries(SOURCES).map(([name, text]) => [name, sha(`${text}\n`)])),
  runs,
  raw_outputs: raw,
  observations,
  promotable: verdictPromotable,
  unmet,
  failed,
  not_executed: notExecuted,
  non_claims: [
    'No proof is claimed: promoted cases keep proof open; unification uniqueness and Rust elaborator refinement stay with task 4.',
    'The elaborator is an untrusted producer: positive cases are accepted by the unchanged explicit checker and the reviewed Lean consumer, whose soundness and the MC1 library are explicit assumptions (VC-GATE-03).',
    'Determinism is observed for the selected inputs, module orders and repetition count only.',
    'IMPL-CASE-20 observes identical runtime reports, Wasm and executable identity across the abbreviated, explicit and proof-free modules; that elaboration runs only at module registration rests on its single production call site, not on a runtime counter.',
  ],
};
const receiptPath = path.join(out, 'acceptance.json');
fs.writeFileSync(receiptPath, `${JSON.stringify(acceptance, null, 2)}\n`);
console.log(JSON.stringify({ receipt: receiptPath, receipt_sha256: hash(receiptPath), result: acceptance.result,
  verdict: acceptance.verdict, failures, tested_commit: testedCommit, source_revision: sourceRevision,
  promotable: verdictPromotable, not_executed: notExecuted }, null, 1));
if (failed.length || binaryChanged) process.exitCode = 1;
