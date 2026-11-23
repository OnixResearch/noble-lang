#!/usr/bin/env node
// PAUTO-CASE-11 and PAUTO-CASE-15 acceptance gate (`.cairn/changes/proof-automation-diagnostics`, task 3).
//
// Exercises retained-evidence emission/staleness (`verify-module --emit`, `goal-view --evidence`,
// `verify-module --evidence`, `model_revision` mismatch, forged evidence) and editor-only proof
// holes (`goal-view --proof-holes`, misplaced transport refusal, undeclared `(use h)` premise, and
// raw `(editor-hole ...)` refusal across admission and ordinary routes) on one `noble` binary.
//
// Usage (the pinned Lean sandbox tools must exist; the consumer compiles `proofs/mc1` itself):
//   node verification/proof-automation-cases/holes-gate.mjs --self-test
//   node verification/proof-automation-cases/holes-gate.mjs --out NEW_DIRECTORY [--noble PATH] [--allow-dirty]
import assert from 'node:assert/strict';
import { execFileSync, spawnSync } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = fs.realpathSync(path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..'));
const SCHEMA = 'noble-proof-automation-holes-acceptance/v1';
const SCOPE = [
  'crates/noble-contracts',
  'crates/noble-cli',
  'Cargo.toml',
  'Cargo.lock',
  'rust-toolchain.toml',
  'proofs/mc1',
  'verification/proof-automation-cases/holes-gate.mjs',
];
const CASE_IDS = ['PAUTO-CASE-11', 'PAUTO-CASE-15'];
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const hash = file => sha(fs.readFileSync(file));

// The design rows of `.cairn/changes/proof-automation-diagnostics/design.md` (PAUTO-CASE-11, 15).
const DESIGNS = {
  'PAUTO-CASE-11': {
    title: 'stale evidence',
    requirements: ['PAUTO-GOAL-01', 'DX-HOLE-01', 'VC-ADMIT-04'],
    input: 'Accepted `increment-correct`, then the subject changed to `[ 2 + ]`, or a dependency accepted under a different `model_revision`',
    expected: 'Chip `stale`; old evidence listed for old subject; new submission refuses `intrinsic-stale-proof` for the old dependency; `unsupported/stale-dependency`, 4',
  },
  'PAUTO-CASE-15': {
    title: 'undeclared premise / unresolved hole',
    requirements: ['PAUTO-GOAL-01', 'DX-HOLE-01'],
    input: '`(use h)` with no `h` premise binder; source still containing an editor proof hole',
    expected: '`use does not name a proof premise` or unresolved; a hole never parses as `.noble` and every admission path rejects it; `error`, 2 / `unknown/incomplete-hole`, 3',
  },
};

const V1_SOURCE = 'module arithmetic@1 [ export increment def increment [ 1 + ] contract 1 increment-law [ subject increment input [ x I64 ] output [ y I64 ] requires [ true ] ensures [ (eq (out y) (add (in x) 1)) ] ] proof 1 increment-correct for increment-law [ (export-unary-I64 (intro (tail Stack) (intro (x I64) (pc-sequence (pc-exact (exec-literal 1)) (pc-exact (exec-add)) (bridge (by-exact-append-assoc)) (join (by-exact-result))))) (mc1-true-eq-wrap)) ] ]\n';
const V2_SOURCE = V1_SOURCE.replace('def increment [ 1 + ]', 'def increment [ 2 + ]');

function proofHoleJson(source, needle, label) {
  const at = source.indexOf(needle);
  assert.ok(at >= 0, `hole replacement target not found: ${needle}`);
  return JSON.stringify({
    format: 1,
    module: [
      source.slice(0, at),
      { kind: 'proof-hole', label },
      source.slice(at + needle.length),
    ],
  });
}

function spliceProofHole(transportJson, replacementText) {
  const parsed = JSON.parse(transportJson);
  assert.equal(parsed.format, 1, 'unexpected proof-hole format');
  assert.ok(Array.isArray(parsed.module), 'module must be an array');
  return parsed.module.map(segment => {
    if (typeof segment === 'string') return segment;
    assert.equal(segment?.kind, 'proof-hole', 'expected proof-hole segment');
    return replacementText;
  }).join('');
}

const SOURCES = {
  'v1.noble': V1_SOURCE,
  'v2.noble': V2_SOURCE,
  'bridge-hole.json': proofHoleJson(V1_SOURCE, '(bridge (by-exact-append-assoc))', 'bridge'),
  'ensures-hole.json': proofHoleJson(V1_SOURCE, '(add (in x) 1)', 'pred'),
  'exec-hole.json': proofHoleJson(V1_SOURCE, '1 +', 'body'),
  'raw-hole.noble': V1_SOURCE.replace('(bridge (by-exact-append-assoc))', '(editor-hole bridge)'),
  'undeclared-premise-binder.noble': 'module premise_binder@1 [ proof 1 bad : [ (Pi (h I64) (Eq I64 h h)) ] [ (intro (h I64) (use h)) ] ]\n',
  'undeclared-premise-missing.noble': 'module premise_missing@1 [ proof 1 bad : [ (Pi (x I64) (Eq I64 x x)) ] [ (intro (x I64) (use h)) ] ]\n',
  'empty.bindings': '',
};

const HYGIENE_PATTERNS = [
  {
    name: 'home-path',
    pattern: `${['', 'home', ''].join('/')}[^\\s"'\\x60]+`,
  },
  {
    name: 'home-cache',
    pattern: `\\$${'HOME'}/\\.cache/[^\\s"'\\x60]*`,
  },
  {
    name: 'tmp-cairn',
    pattern: `${['', 'var', 'tmp', `noble-${'cairn'}`, ''].join('/')}[^\\s"'\\x60]*`,
  },
  {
    name: 'campaign-wrapper',
    pattern: `\\b(${['n' + 'cargo', 'n' + 'lake', 'n' + 'cairn', 'en' + 'v\\.sh', 'oc' + 'tet\\.sh'].join('|')})\\b`,
  },
];

function findHygieneViolations(text) {
  const violations = [];
  for (const entry of HYGIENE_PATTERNS) {
    const regex = new RegExp(entry.pattern, 'g');
    for (const match of text.matchAll(regex)) {
      violations.push({ rule: entry.name, match: match[0] });
    }
  }
  return violations;
}

function scanDirectoryHygiene(baseDir, relativePaths) {
  const findings = [];
  for (const rel of relativePaths) {
    const full = path.join(baseDir, rel);
    if (!fs.existsSync(full)) continue;
    const text = fs.readFileSync(full, 'utf8');
    for (const violation of findHygieneViolations(text)) {
      findings.push({ file: rel, ...violation });
    }
  }
  return findings;
}

function argument(name) {
  const index = process.argv.indexOf(name);
  if (index <= 0) return undefined;
  const value = process.argv[index + 1];
  assert.ok(value && !value.startsWith('--'), `flag ${name} requires a value`);
  return value;
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
function expectArrayIncludes(label, items, expected) {
  for (const part of expected) {
    if (!Array.isArray(items) || !items.includes(part)) {
      throw new Mismatch(`${label}: ${JSON.stringify(items)} lacks ${JSON.stringify(part)}`);
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

if (process.argv.includes('--self-test')) {
  // 1. Verdict derivation checks
  const all = Object.fromEntries(CASE_IDS.map(id => [id, { case: 'passed' }]));
  assert.deepEqual(deriveVerdicts(all), {
    promotable: CASE_IDS,
    unmet: [],
    failed: [],
    not_executed: [],
  });
  const mixed = { 'PAUTO-CASE-11': { case: 'unmet' }, 'PAUTO-CASE-15': { case: 'failed' } };
  assert.deepEqual(deriveVerdicts(mixed), {
    promotable: [],
    unmet: ['PAUTO-CASE-11'],
    failed: ['PAUTO-CASE-15'],
    not_executed: [],
  });
  assert.deepEqual(deriveVerdicts({ 'PAUTO-CASE-11': { case: 'passed' } }), {
    promotable: ['PAUTO-CASE-11'],
    unmet: [],
    failed: [],
    not_executed: ['PAUTO-CASE-15'],
  });

  // 2. Fixture & proof-hole splice round-trip checks
  assert.equal(new Set(Object.values(SOURCES)).size, Object.keys(SOURCES).length, 'duplicate fixture');
  assert.notEqual(SOURCES['v1.noble'], SOURCES['v2.noble'], 'v1 and v2 must differ');
  for (const [name, content] of Object.entries(SOURCES)) {
    assert.ok(Buffer.byteLength(content, 'utf8') < 65_536, `${name} exceeds source byte bound`);
  }
  assert.equal(
    spliceProofHole(SOURCES['bridge-hole.json'], '(bridge (by-exact-append-assoc))'),
    V1_SOURCE,
    'bridge-hole splice round-trip failed',
  );
  assert.equal(
    spliceProofHole(SOURCES['ensures-hole.json'], '(add (in x) 1)'),
    V1_SOURCE,
    'ensures-hole splice round-trip failed',
  );
  assert.equal(
    spliceProofHole(SOURCES['exec-hole.json'], '1 +'),
    V1_SOURCE,
    'exec-hole splice round-trip failed',
  );
  const parsedBridge = JSON.parse(SOURCES['bridge-hole.json']);
  assert.equal(parsedBridge.format, 1);
  assert.equal(parsedBridge.module.length, 3);
  assert.deepEqual(parsedBridge.module[1], { kind: 'proof-hole', label: 'bridge' });

  // 3. Hygiene scanner positive & negative controls
  const badHome = ['', 'home', 'user', 'repo', 'file'].join('/');
  const badCache = `$${'HOME'}/.cache/noble/work`;
  const badTmp = ['', 'var', 'tmp', `noble-${'cairn'}`, 'en' + 'v.sh'].join('/');
  const badWrappers = ['n' + 'cargo', 'n' + 'lake', 'n' + 'cairn', 'en' + 'v.sh', 'oc' + 'tet.sh'].join(' ');
  assert.ok(findHygieneViolations(badHome).some(v => v.rule === 'home-path'));
  assert.ok(findHygieneViolations(badCache).some(v => v.rule === 'home-cache'));
  assert.ok(findHygieneViolations(badTmp).some(v => v.rule === 'tmp-cairn'));
  assert.equal(
    findHygieneViolations(badWrappers).filter(v => v.rule === 'campaign-wrapper').length,
    5,
  );
  const cleanReceipt = JSON.stringify({
    schema: SCHEMA,
    environment: {
      NOBLE_LEAN: '/nix/store/kwvw84h9r7w0q40d3cm5xgfz0rpv7yzc-noble-lean-release-4.35.0/bin/lean',
      NOBLE_CONTRACT_LIBRARY: 'proofs/mc1',
    },
    source_path: 'sources/v1.noble',
  });
  assert.deepEqual(findHygieneViolations(cleanReceipt), []);

  // 4. Mismatch negative controls
  assert.throws(() => expectValue('chip', 'checked', 'incomplete'), Mismatch);
  assert.throws(() => expectValue('outcome', 'proved', 'unsupported'), Mismatch);
  assert.throws(() => expectIncludes('code', 'other', ['intrinsic-stale-proof']), Mismatch);
  assert.throws(() => expectArrayIncludes('mismatches', ['source_sha256'], ['proofs.increment-correct.model_revision']), Mismatch);

  console.log('holes-gate self-test: verdicts, proof-hole splice round-trips, hygiene controls, and Mismatch checks passed');
  process.exit(0);
}

const out = argument('--out');
assert.ok(out, 'pass --out NEW_DIRECTORY');
assert.equal(fs.existsSync(out), false, '--out must not already exist');
const explicitNoble = argument('--noble');
const allowDirty = process.argv.includes('--allow-dirty');

// The reviewed Lean sandbox tools the MC1 consumer selects, and the repository library.
const REVIEWED_TOOLS = {
  NOBLE_LEAN: '/nix/store/kwvw84h9r7w0q40d3cm5xgfz0rpv7yzc-noble-lean-release-4.35.0/bin/lean',
  NOBLE_BWRAP: '/nix/store/y0ra9qr3rz81d9wl7dfrldv3j25dc98q-bubblewrap-0.12.0/bin/bwrap',
  NOBLE_PRLIMIT: '/nix/store/mqvbf0flamqaq9c3ihb496aahag897n0-util-linux-2.42.3-bin/bin/prlimit',
  NOBLE_SYSTEMD_RUN: '/nix/store/baxgs06659w4i6ggsfr56srjrb52v1h8-systemd-261.2/bin/systemd-run',
};
const REVIEWED_RUNTIME = {
  ...REVIEWED_TOOLS,
  NOBLE_CONTRACT_LIBRARY: path.join(root, 'proofs/mc1'),
};
const REVIEWED_SERIALIZED = {
  ...REVIEWED_TOOLS,
  NOBLE_CONTRACT_LIBRARY: 'proofs/mc1',
};
const TIMEOUT_MS = '600000';

// Verify prerequisite files & tools exist (without requiring `proofs/mc1/.lake/build/`).
assert.ok(fs.existsSync(path.join(root, 'proofs/mc1/lean-toolchain')), 'missing proofs/mc1/lean-toolchain');
for (const [key, toolPath] of Object.entries(REVIEWED_TOOLS)) {
  fs.accessSync(toolPath, fs.constants.X_OK);
  assert.ok(path.isAbsolute(toolPath), `${key} must be absolute`);
}

fs.mkdirSync(out, { recursive: true, mode: 0o700 });
for (const directory of ['home', 'tmp', 'sources', 'evidence', 'raw']) {
  fs.mkdirSync(path.join(out, directory));
}

const raw = [];
function recordRaw(output, name, text) {
  const file = path.join(output, 'raw', `${name}.txt`);
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, text);
  raw.push({ name, file: path.relative(output, file), sha256: sha(text), bytes: Buffer.byteLength(text) });
}

function sourceSnapshot() {
  const statusText = git('status', '--porcelain', '--untracked-files=all', '--', ...SCOPE).trim();
  if (!allowDirty) {
    assert.equal(statusText, '', `source scope is not clean:\n${statusText}`);
  }
  const files = git('ls-files', '-z', '--', ...SCOPE).split('\0').filter(Boolean).sort();
  const snapshot = {};
  for (const file of files) {
    snapshot[file] = { sha256: hash(path.join(root, file)) };
  }
  return {
    dirty: allowDirty || statusText.length > 0,
    snapshot,
  };
}

function sanitizeBuildText(text, targetDir) {
  let clean = text.split(root).join('.');
  if (targetDir) {
    clean = clean.split(targetDir).join('<target>');
  }
  return clean;
}

function resolveNoble(output) {
  if (explicitNoble) {
    assert.ok(path.isAbsolute(explicitNoble), '--noble PATH must be an absolute path');
    const resolved = fs.realpathSync(explicitNoble);
    fs.accessSync(resolved, fs.constants.X_OK);
    return { mode: 'supplied', built: false, source_path: resolved, source_sha256: hash(resolved) };
  }
  const build = spawnSync(
    'cargo',
    ['build', '--locked', '--offline', '-p', 'noble-cli', '--bin', 'noble', '--message-format=json-render-diagnostics'],
    { cwd: root, encoding: 'utf8', maxBuffer: 256 * 1024 * 1024 },
  );
  const artifacts = (build.stdout ?? '').split('\n').filter(Boolean)
    .map(line => { try { return JSON.parse(line); } catch { return null; } })
    .filter(message => message && message.reason === 'compiler-artifact'
      && message.target?.name === 'noble' && message.executable);
  const targetDir = artifacts[0]?.executable ? path.dirname(path.dirname(artifacts[0].executable)) : null;
  recordRaw(output, 'build.stdout', sanitizeBuildText(build.stdout ?? '', targetDir));
  recordRaw(output, 'build.stderr', sanitizeBuildText(build.stderr ?? '', targetDir));
  assert.equal(build.status, 0, `cargo build failed:\n${build.stderr}`);
  assert.equal(artifacts.length, 1, 'expected one noble executable artifact');
  return {
    mode: 'built',
    built: true,
    source_path: artifacts[0].executable,
    source_sha256: hash(artifacts[0].executable),
  };
}

const { dirty, snapshot } = sourceSnapshot();
const sourceRevision = `sha256:${sha(JSON.stringify(snapshot))}`;
const testedCommit = git('rev-parse', 'HEAD').trim();
const designDigest = sha(JSON.stringify(canonical(DESIGNS)));
const resolved = resolveNoble(out);
const binary = path.join(out, 'noble');
fs.copyFileSync(resolved.source_path, binary);
fs.chmodSync(binary, 0o500);
const binarySha = hash(binary);

for (const [name, text] of Object.entries(SOURCES)) {
  const file = path.join(out, 'sources', name);
  fs.writeFileSync(file, text, { flag: 'wx', mode: 0o400 });
}

const baseEnv = {
  PATH: '/run/current-system/sw/bin',
  HOME: path.join(out, 'home'),
  TMPDIR: path.join(out, 'tmp'),
  LANG: 'C.UTF-8',
  LC_ALL: 'C.UTF-8',
  XDG_RUNTIME_DIR: process.env.XDG_RUNTIME_DIR,
};

const runs = [];
function run(label, args, { stripSandbox = false, stdin = undefined } = {}) {
  const env = stripSandbox ? { ...baseEnv } : { ...baseEnv, ...REVIEWED_RUNTIME };
  const result = spawnSync(binary, args, {
    cwd: out,
    env,
    input: stdin,
    encoding: 'utf8',
    maxBuffer: 64 * 1024 * 1024,
    timeout: 1_800_000,
  });
  recordRaw(out, `${label}.stdout`, result.stdout ?? '');
  if (result.stderr) recordRaw(out, `${label}.stderr`, result.stderr);
  runs.push({ label, args, strip_sandbox: stripSandbox, status: result.status, signal: result.signal });
  let report = null;
  const trimmed = (result.stdout ?? '').trim();
  try {
    report = JSON.parse(trimmed);
  } catch {
    const lines = trimmed.split('\n').filter(Boolean);
    if (lines.length >= 1) {
      try {
        report = JSON.parse(lines[0]);
      } catch {
        report = null;
      }
    }
  }
  if (report === null) {
    throw new Error(`${label}: no JSON report (status ${result.status}, signal ${result.signal})`);
  }
  return { status: result.status, report };
}

const observations = {};

observations['PAUTO-CASE-11'] = await observe(async () => {
  // 1. Real Lean-checked emit
  const emitDir = 'evidence/v1-accepted';
  const emit = run('case11-emit', ['verify-module', 'sources/v1.noble', '--emit', emitDir, '--timeout-ms', TIMEOUT_MS]);
  expectValue('emit status', emit.status, 0);
  expectValue('emit schema', emit.report.schema, 'noble-intrinsic-module-report/v2');
  expectValue('emit outcome', emit.report.outcome, 'proved');
  expectValue('emit stage', emit.report.stage, 'independent-recheck');
  expectValue('emit independent_recheck', emit.report.independent_recheck, true);
  expectValue('emit category', emit.report.category, 'checked');
  expectValue('emit remediation', emit.report.remediation, 'none');
  expectValue('emit claim', emit.report.proofs?.[0]?.claim, 'MC1Obligation.claim');

  const emitEvidencePath = path.join(out, emitDir, 'evidence.json');
  const emitSourcePath = path.join(out, emitDir, 'source.noble');
  const emitReportPath = path.join(out, emitDir, 'report.json');
  assert.ok(fs.existsSync(emitReportPath), 'missing emitted report.json');
  assert.ok(fs.existsSync(emitSourcePath), 'missing emitted source.noble');
  assert.ok(fs.existsSync(emitEvidencePath), 'missing emitted evidence.json');
  const evBytesBefore = fs.readFileSync(emitEvidencePath);
  const evShaBefore = sha(evBytesBefore);
  const evJson = JSON.parse(evBytesBefore.toString('utf8'));
  expectValue('evidence schema', evJson.schema, 'noble-intrinsic-evidence/v1');
  expectValue('evidence command', evJson.command, 'verify-module');
  expectValue('evidence outcome', evJson.outcome, 'proved');
  expectValue('evidence independent_recheck', evJson.independent_recheck, true);
  expectValue('evidence source_sha256', evJson.source_sha256, sha(SOURCES['v1.noble']));
  expectValue('evidence module', evJson.module, { name: 'arithmetic', version: 1 });
  expectValue('evidence proof name', evJson.proofs?.[0]?.name, 'increment-correct');
  expectValue('evidence proof kind', evJson.proofs?.[0]?.kind, 'contract');
  expectValue('evidence statement_basis', evJson.proofs?.[0]?.statement_basis, 'generated-statement');
  expectValue('evidence subject definition', evJson.proofs?.[0]?.subject?.definition, 'increment');
  expectValue('evidence model revision hex', /^[0-9a-f]{64}$/.test(evJson.revisions?.model ?? ''), true);
  expectValue('evidence checker revision hex', /^[0-9a-f]{64}$/.test(evJson.revisions?.checker ?? ''), true);

  // 2. Existing --emit destination refusal
  const emitExisting = run('case11-emit-existing', ['verify-module', 'sources/v1.noble', '--emit', emitDir, '--timeout-ms', TIMEOUT_MS]);
  expectValue('emit existing status', emitExisting.status, 2);
  expectValue('emit existing outcome', emitExisting.report.outcome, 'error');
  expectValue('emit existing category', emitExisting.report.category, 'unavailable');
  expectValue('emit existing code', emitExisting.report.diagnostic?.code, 'emit-destination');
  assert.equal(Buffer.compare(fs.readFileSync(emitEvidencePath), evBytesBefore), 0, 'evidence.json modified by emit-existing');

  // 3. Matching goal-view --evidence (sandbox tools stripped) never grants checked
  const gvMatch = run('case11-goal-view-matching', ['goal-view', 'sources/v1.noble', '--evidence', emitDir], { stripSandbox: true });
  expectValue('goal-view matching status', gvMatch.status, 0);
  expectValue('goal-view matching schema', gvMatch.report.schema, 'noble-goal-view/v1');
  expectValue('goal-view matching acceptance', gvMatch.report.acceptance, false);
  expectValue('goal-view matching outcome', gvMatch.report.outcome, 'not-run');
  expectValue('goal-view matching category', gvMatch.report.category, 'explanation-only');
  expectValue('goal-view matching checker_result', gvMatch.report.checker_result, null);
  expectValue('goal-view matching record matches', gvMatch.report.evidence?.records?.[0]?.matches, true);
  expectValue('goal-view matching record mismatches', gvMatch.report.evidence?.records?.[0]?.mismatches, []);
  expectValue('goal-view matching chip', gvMatch.report.sites?.[0]?.chip, 'incomplete');

  // 4. Matching verify-module --evidence still performs full independent recheck
  const vmMatch = run('case11-verify-matching', ['verify-module', 'sources/v1.noble', '--evidence', emitDir, '--timeout-ms', TIMEOUT_MS]);
  expectValue('verify matching status', vmMatch.status, 0);
  expectValue('verify matching outcome', vmMatch.report.outcome, 'proved');
  expectValue('verify matching recheck', vmMatch.report.independent_recheck, true);
  expectValue('verify matching category', vmMatch.report.category, 'checked');

  // 5. Stale goal-view --evidence on v2.noble ([ 2 + ], sandbox tools stripped)
  const gvStale = run('case11-goal-view-stale', ['goal-view', 'sources/v2.noble', '--evidence', emitDir], { stripSandbox: true });
  expectValue('goal-view stale status', gvStale.status, 0);
  expectValue('goal-view stale matches', gvStale.report.evidence?.records?.[0]?.matches, false);
  expectValue('goal-view stale chip', gvStale.report.sites?.[0]?.chip, 'stale');
  expectValue('goal-view stale chip_source', gvStale.report.sites?.[0]?.chip_source, 'retained-evidence');
  expectValue('goal-view stale outcome', gvStale.report.sites?.[0]?.outcome, 'unsupported');
  expectValue('goal-view stale category', gvStale.report.sites?.[0]?.category, 'stale-dependency');
  expectValue('goal-view stale checker_result.checked', gvStale.report.sites?.[0]?.checker_result?.checked, false);
  expectValue(
    'goal-view stale retained definition',
    gvStale.report.sites?.[0]?.retained_evidence?.[0]?.recorded?.subject?.definition,
    'increment',
  );
  expectArrayIncludes(
    'goal-view stale mismatches',
    gvStale.report.evidence?.records?.[0]?.mismatches,
    ['source_sha256', 'proofs.increment-correct.statement_digest'],
  );
  assert.equal(Buffer.compare(fs.readFileSync(emitEvidencePath), evBytesBefore), 0, 'evidence.json modified by stale goal-view');

  // 6. Stale verify-module --evidence refusal (exit 4, intrinsic-stale-proof, evidence.json byte-identical)
  const vmStale = run('case11-verify-stale', ['verify-module', 'sources/v2.noble', '--evidence', emitDir, '--timeout-ms', TIMEOUT_MS]);
  expectValue('verify stale status', vmStale.status, 4);
  expectValue('verify stale outcome', vmStale.report.outcome, 'unsupported');
  expectValue('verify stale category', vmStale.report.category, 'stale-dependency');
  expectValue('verify stale remediation', vmStale.report.remediation, 'recheck-against-current-revisions');
  expectValue('verify stale stage', vmStale.report.stage, 'independent-recheck');
  expectValue('verify stale recheck', vmStale.report.independent_recheck, false);
  expectValue('verify stale diagnostic code', vmStale.report.diagnostic?.code, 'intrinsic-stale-proof');
  expectValue('verify stale evidence path', vmStale.report.diagnostic?.details?.evidence, emitDir);
  expectValue('verify stale evidence sha256', vmStale.report.diagnostic?.details?.evidence_sha256, evShaBefore);
  expectArrayIncludes(
    'verify stale mismatches',
    vmStale.report.diagnostic?.details?.mismatches,
    ['source_sha256', 'proofs.increment-correct.statement_digest'],
  );
  assert.equal(Buffer.compare(fs.readFileSync(emitEvidencePath), evBytesBefore), 0, 'evidence.json modified by stale verify-module');

  // 7. Revision staleness (`model_revision` mismatch in copied old-rev directory)
  const oldRevDir = 'evidence/old-rev';
  fs.mkdirSync(path.join(out, oldRevDir), { recursive: true });
  fs.copyFileSync(emitSourcePath, path.join(out, oldRevDir, 'source.noble'));
  const oldRevEvidence = structuredClone(evJson);
  const priorModelRevision = '0'.repeat(64);
  oldRevEvidence.proofs[0].model_revision = oldRevEvidence.proofs[0].model_revision === priorModelRevision
    ? '1'.repeat(64)
    : priorModelRevision;
  fs.writeFileSync(path.join(out, oldRevDir, 'evidence.json'), `${JSON.stringify(oldRevEvidence, null, 2)}\n`);

  const gvOldRev = run('case11-revision-stale-goal-view', ['goal-view', 'sources/v1.noble', '--evidence', oldRevDir], { stripSandbox: true });
  expectValue('revision-stale goal-view status', gvOldRev.status, 0);
  expectValue('revision-stale goal-view matches', gvOldRev.report.evidence?.records?.[0]?.matches, false);
  expectValue('revision-stale goal-view chip', gvOldRev.report.sites?.[0]?.chip, 'stale');
  expectValue('revision-stale goal-view outcome', gvOldRev.report.sites?.[0]?.outcome, 'unsupported');
  expectValue('revision-stale goal-view category', gvOldRev.report.sites?.[0]?.category, 'stale-dependency');
  expectArrayIncludes(
    'revision-stale goal-view mismatches',
    gvOldRev.report.evidence?.records?.[0]?.mismatches,
    ['proofs.increment-correct.model_revision'],
  );

  const vmOldRev = run('case11-revision-stale-verify', ['verify-module', 'sources/v1.noble', '--evidence', oldRevDir, '--timeout-ms', TIMEOUT_MS]);
  expectValue('revision-stale verify status', vmOldRev.status, 4);
  expectValue('revision-stale verify outcome', vmOldRev.report.outcome, 'unsupported');
  expectValue('revision-stale verify category', vmOldRev.report.category, 'stale-dependency');
  expectValue('revision-stale verify recheck', vmOldRev.report.independent_recheck, false);
  expectValue('revision-stale verify code', vmOldRev.report.diagnostic?.code, 'intrinsic-stale-proof');
  expectArrayIncludes(
    'revision-stale verify mismatches',
    vmOldRev.report.diagnostic?.details?.mismatches,
    ['proofs.increment-correct.model_revision'],
  );

  // 8. Forged evidence refusal
  const forgedDir = 'evidence/forged';
  fs.mkdirSync(path.join(out, forgedDir), { recursive: true });
  fs.writeFileSync(path.join(out, forgedDir, 'source.noble'), 'module forged@1 [ ]\n');
  fs.writeFileSync(path.join(out, forgedDir, 'evidence.json'), evBytesBefore);
  const vmForged = run('case11-verify-forged', ['verify-module', 'sources/v1.noble', '--evidence', forgedDir, '--timeout-ms', TIMEOUT_MS]);
  expectValue('verify forged status', vmForged.status, 2);
  expectValue('verify forged outcome', vmForged.report.outcome, 'error');
  expectValue('verify forged category', vmForged.report.category, 'forged-identity');
  expectValue('verify forged code', vmForged.report.diagnostic?.code, 'evidence-invalid');

  return {
    emitted_evidence_sha256: evShaBefore,
    stale_mismatches: vmStale.report.diagnostic?.details?.mismatches,
    revision_stale_mismatches: vmOldRev.report.diagnostic?.details?.mismatches,
    forged_code: vmForged.report.diagnostic?.code,
  };
});

observations['PAUTO-CASE-15'] = await observe(async () => {
  // 1a. Undeclared premise `(use h)` where `h` is a value binder
  const premBinder = run('case15-undeclared-premise-binder', ['verify-module', 'sources/undeclared-premise-binder.noble', '--timeout-ms', TIMEOUT_MS]);
  expectValue('premise binder status', premBinder.status, 2);
  expectValue('premise binder outcome', premBinder.report.outcome, 'error');
  expectValue('premise binder category', premBinder.report.category, 'rejected-term');
  expectValue('premise binder stage', premBinder.report.stage, 'proof-check');
  expectValue('premise binder recheck', premBinder.report.independent_recheck, false);
  expectValue('premise binder code', premBinder.report.diagnostic?.code, 'intrinsic-source-invalid');
  expectIncludes('premise binder message', premBinder.report.diagnostic?.message, ['use does not name a proof premise']);

  // 1b. Undeclared premise `(use h)` where `h` is absent from context and exports
  const premMissing = run('case15-undeclared-premise-missing', ['verify-module', 'sources/undeclared-premise-missing.noble', '--timeout-ms', TIMEOUT_MS]);
  expectValue('premise missing status', premMissing.status, 2);
  expectValue('premise missing outcome', premMissing.report.outcome, 'error');
  expectValue('premise missing category', premMissing.report.category, 'rejected-term');
  expectValue('premise missing stage', premMissing.report.stage, 'proof-check');
  expectValue('premise missing recheck', premMissing.report.independent_recheck, false);
  expectValue('premise missing code', premMissing.report.diagnostic?.code, 'intrinsic-source-invalid');
  expectIncludes('premise missing message', premMissing.report.diagnostic?.message, ['unresolved, private, forward or cyclic proof use']);

  // 2. Valid editor proof-hole transport (`noble-proof-editor/v1`)
  const gvHole = run('case15-goal-view-hole', ['goal-view', '--proof-holes', 'sources/bridge-hole.json'], { stripSandbox: true });
  expectValue('goal-view hole status', gvHole.status, 3);
  expectValue('goal-view hole schema', gvHole.report.schema, 'noble-goal-view/v1');
  expectValue('goal-view hole outcome', gvHole.report.outcome, 'unknown');
  expectValue('goal-view hole category', gvHole.report.category, 'incomplete-hole');
  expectValue('goal-view hole acceptance', gvHole.report.acceptance, false);
  expectValue('goal-view hole checker_result', gvHole.report.checker_result, null);
  expectValue('goal-view hole transport schema', gvHole.report.transport?.schema, 'noble-proof-editor/v1');
  expectValue('goal-view hole transport label', gvHole.report.transport?.holes?.[0]?.label, 'bridge');
  const holeSite = gvHole.report.sites?.[0] ?? {};
  expectValue('goal-view hole site chip', holeSite.chip, 'incomplete');
  expectValue('goal-view hole site outcome', holeSite.outcome, 'unknown');
  expectValue('goal-view hole site category', holeSite.category, 'incomplete-hole');
  expectValue('goal-view hole site holes', holeSite.holes, ['bridge']);
  const holeRow = (holeSite.sites ?? []).find(row => row.constructor === 'editor-hole');
  assert.ok(holeRow, 'missing editor-hole row in sites[0].sites');
  expectValue('editor-hole chip', holeRow.chip, 'incomplete');
  expectValue('editor-hole label', holeRow.label, 'bridge');
  expectValue('editor-hole parent_constructor', holeRow.parent_constructor, 'pc-sequence');
  expectValue('editor-hole goal declaration', holeRow.goal?.declaration, 'increment-law');
  expectValue('editor-hole goal digest', holeRow.goal?.statement_digest, holeSite.statement_digest);

  // 3. Proof-hole transport with matching retained evidence still reports `incomplete`
  const gvHoleEv = run(
    'case15-goal-view-hole-with-evidence',
    ['goal-view', '--proof-holes', 'sources/bridge-hole.json', '--evidence', 'evidence/v1-accepted'],
    { stripSandbox: true },
  );
  expectValue('hole+evidence status', gvHoleEv.status, 3);
  expectValue('hole+evidence outcome', gvHoleEv.report.outcome, 'unknown');
  expectValue('hole+evidence category', gvHoleEv.report.category, 'incomplete-hole');
  expectValue('hole+evidence chip', gvHoleEv.report.sites?.[0]?.chip, 'incomplete');
  expectValue('hole+evidence retained_evidence', gvHoleEv.report.sites?.[0]?.retained_evidence, []);

  // 4. Misplaced holes outside proof-term positions (`ensures` predicate and executable body)
  for (const [label, sourceRel] of [
    ['case15-misplaced-pred', 'sources/ensures-hole.json'],
    ['case15-misplaced-body', 'sources/exec-hole.json'],
  ]) {
    const misplaced = run(label, ['goal-view', '--proof-holes', sourceRel], { stripSandbox: true });
    expectValue(`${label} status`, misplaced.status, 2);
    expectValue(`${label} outcome`, misplaced.report.outcome, 'error');
    expectValue(`${label} code`, misplaced.report.diagnostic?.code, 'proof-hole-transport');
    expectIncludes(`${label} message`, misplaced.report.diagnostic?.message, ['not in a proof-term position']);
  }

  // 5. Raw `(editor-hole bridge)` refused on all admission and ordinary paths
  const rawVerify = run('case15-raw-hole-verify-module', ['verify-module', 'sources/raw-hole.noble', '--timeout-ms', TIMEOUT_MS], { stripSandbox: true });
  expectValue('raw-hole verify-module status', rawVerify.status, 2);
  expectValue('raw-hole verify-module outcome', rawVerify.report.outcome, 'error');
  expectValue('raw-hole verify-module code', rawVerify.report.diagnostic?.code, 'intrinsic-contract-invalid');
  expectIncludes('raw-hole verify-module message', rawVerify.report.diagnostic?.message, ['editor-only']);

  for (const [label, cmd] of [
    ['case15-raw-hole-obligations', 'obligations'],
    ['case15-raw-hole-goal-view', 'goal-view'],
  ]) {
    const rawView = run(label, [cmd, 'sources/raw-hole.noble'], { stripSandbox: true });
    expectValue(`${label} status`, rawView.status, 2);
    expectValue(`${label} outcome`, rawView.report.outcome, 'error');
    expectIncludes(`${label} message`, rawView.report.diagnostic?.message, ['editor-only']);
  }

  const rawRun = run('case15-raw-hole-run', ['run', 'sources/raw-hole.noble'], { stripSandbox: true });
  expectValue('raw-hole run status', rawRun.status, 2);
  expectValue('raw-hole run schema', rawRun.report.schema, 'noble-core-report/v1');
  assert.notEqual(rawRun.report.outcome, 'normal', 'raw-hole run must refuse');
  expectValue('raw-hole run guest_requests', rawRun.report.guest_requests, 0);
  expectValue('raw-hole run protected_operations', rawRun.report.protected_operations, 0);

  const framedPayload = `${Buffer.byteLength(SOURCES['raw-hole.noble'], 'utf8')}\n${SOURCES['raw-hole.noble']}`;
  const rawSession = run(
    'case15-raw-hole-session',
    ['session', '--framed', '--declared-modules', '--bindings', 'sources/empty.bindings'],
    { stripSandbox: true, stdin: framedPayload },
  );
  expectValue('raw-hole session status', rawSession.status, 2);
  expectValue('raw-hole session schema', rawSession.report.schema, 'noble-core-report/v1');
  expectValue('raw-hole session profile', rawSession.report.profile, 'Declared-Modules-v1');
  expectValue('raw-hole session outcome', rawSession.report.outcome, 'type-reject');
  expectValue('raw-hole session guest_requests', rawSession.report.guest_requests, 0);
  expectValue('raw-hole session prior_namespace', rawSession.report.prior_namespace, 'unchanged');
  expectIncludes('raw-hole session diagnostic', rawSession.report.diagnostic, ['editor-only']);

  return {
    binder_code: premBinder.report.diagnostic?.code,
    missing_code: premMissing.report.diagnostic?.code,
    hole_transport_schema: gvHole.report.transport?.schema,
    raw_session_guest_requests: rawSession.report.guest_requests,
  };
});

const { promotable, unmet, failed, not_executed: notExecuted } = deriveVerdicts(observations);
const binaryAfter = hash(binary);
const binaryChanged = binaryAfter !== binarySha;
const verdictPromotable = binaryChanged ? [] : promotable;
const failures = [
  ...unmet.map(id => `${id}: ${observations[id].diagnostic}`),
  ...failed.map(id => `${id}: ${observations[id].diagnostic?.split('\n')[0]}`),
  ...(binaryChanged ? ['the executed binary changed during the run; no observation is promotable'] : []),
];
const receiptReadyForRetention = !dirty && !allowDirty && !binaryChanged && failures.length === 0 && notExecuted.length === 0;

const acceptance = {
  schema: SCHEMA,
  claim: 'selected executions of PAUTO-CASE-11 and PAUTO-CASE-15 through `noble verify-module`, `noble goal-view`, '
    + '`noble obligations`, `noble run`, and `noble session` on one CLI built from the recorded commit; '
    + 'promotion verdicts are derived from the observations, not declared here',
  result: failed.length || binaryChanged ? 'failed' : 'completed',
  verdict: failures.length === 0 && notExecuted.length === 0 ? 'all-passed'
    : failed.length || binaryChanged ? 'run-failure' : 'incomplete',
  dirty,
  receipt_ready_for_retention: receiptReadyForRetention,
  failures,
  tested_commit: testedCommit,
  source_revision: sourceRevision,
  scope: SCOPE,
  source: snapshot,
  designs: { sha256: designDigest, rows: DESIGNS },
  binary: {
    mode: resolved.mode,
    built: resolved.built,
    supplied_sha256: resolved.source_sha256,
    executed_sha256: binarySha,
    executed_sha256_after: binaryAfter,
  },
  build: 'cargo build --locked --offline -p noble-cli --bin noble',
  environment: REVIEWED_SERIALIZED,
  fixtures: Object.fromEntries(Object.entries(SOURCES).map(([name, text]) => [name, sha(text)])),
  runs,
  raw_outputs: raw,
  observations,
  promotable: verdictPromotable,
  unmet,
  failed,
  not_executed: notExecuted,
  non_claims: [
    'No proof is claimed beyond the pinned Lean 4.35.0 consumer accepting `v1.noble` under `proofs/mc1` (`--emit`) and refusing tampered/stale inputs (`evidence-invalid` / `intrinsic-stale-proof`).',
    '`goal-view --evidence` and `goal-view --proof-holes` are read-only projections; retained evidence never grants `checked`, and proof holes remain `unknown`/`incomplete-hole`.',
    'Registration in `specs/conformance/*.json` and requirement synchronization stay with task 5 and independent review.',
  ],
};

const acceptanceJson = `${JSON.stringify(acceptance, null, 2)}\n`;
const receiptPath = path.join(out, 'acceptance.json');
fs.writeFileSync(receiptPath, acceptanceJson);

// Enforce public-repo hygiene across serialized acceptance.json and all retained artifacts.
const retainedRelFiles = [
  'acceptance.json',
  ...raw.map(entry => entry.file),
  ...Object.keys(SOURCES).map(name => path.join('sources', name)),
  'evidence/v1-accepted/report.json',
  'evidence/v1-accepted/source.noble',
  'evidence/v1-accepted/evidence.json',
  'evidence/old-rev/source.noble',
  'evidence/old-rev/evidence.json',
  'evidence/forged/source.noble',
  'evidence/forged/evidence.json',
];
const hygieneViolations = scanDirectoryHygiene(out, retainedRelFiles);
if (hygieneViolations.length > 0) {
  console.error(JSON.stringify({ hygiene_violations: hygieneViolations }, null, 2));
  process.exitCode = 1;
}

console.log(JSON.stringify({
  receipt: receiptPath,
  receipt_sha256: hash(receiptPath),
  result: acceptance.result,
  verdict: acceptance.verdict,
  dirty,
  receipt_ready_for_retention: receiptReadyForRetention,
  hygiene_violations: hygieneViolations.length,
  failures,
  tested_commit: testedCommit,
  source_revision: sourceRevision,
  promotable: verdictPromotable,
  not_executed: notExecuted,
}, null, 1));
if (unmet.length || failed.length || notExecuted.length || binaryChanged || hygieneViolations.length > 0) {
  process.exitCode = 1;
}
