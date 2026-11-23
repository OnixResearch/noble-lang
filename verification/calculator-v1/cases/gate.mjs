#!/usr/bin/env node
// Calculator-v1 case gate (`CALC-01..13`, `.cairn/changes/calculator-v1` CALC-78).
//
// Runs clean-tree verification lanes on one debug `noble` binary and cargo
// workspace without `--release`, writes full raw artifacts to `--out <EXT_DIR>`,
// and optionally emits a sanitized compact JSON receipt via `--write-compact`.
// Promotion verdicts (`deriveVerdicts`) are derived strictly from observed
// lane results; this script never edits `specs/conformance/calculator-cases.json`.
//
// Usage:
//   node verification/calculator-v1/cases/gate.mjs --self-test
//   node verification/calculator-v1/cases/gate.mjs --out NEW_EXTERNAL_DIR \
//     [--binary PATH] [--lanes cli,state-host,oracle,numeric-fast,cli-unit] \
//     [--case CALC-01,...] [--concurrency 4] [--slow-timeout-s 900] \
//     [--write-compact verification/calculator-v1/cases/runs/<YYYYMMDD>-<sha>/acceptance.json]
import assert from 'node:assert/strict';
import { execFileSync, spawn, spawnSync } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseArgs } from 'node:util';
import { generate } from '../vectors.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = fs.realpathSync(path.resolve(here, '../../..'));

const ALL_CASE_IDS = Array.from({ length: 13 }, (_, i) => `CALC-${String(i + 1).padStart(2, '0')}`);
const VALID_LANES = ['cli', 'state-host', 'oracle', 'budgets', 'numeric-fast', 'numeric-slow', 'cli-unit'];
const DEFAULT_LANES = ['cli', 'state-host', 'oracle', 'numeric-fast', 'cli-unit'];

const SCOPE = [
  'Cargo.toml',
  'Cargo.lock',
  'rust-toolchain.toml',
  'crates/noble-kernel',
  'crates/noble-contracts',
  'crates/noble-wasm',
  'crates/noble-cli',
  'specs/conformance/calculator-cases.json',
  'verification/calculator-v1/acceptance.mjs',
  'verification/calculator-v1/vectors.mjs',
  'verification/calculator-v1/cli-mapping.json',
  'verification/calculator-v1/supervise.mjs',
  'verification/calculator-v1/budgets',
  'verification/calculator-v1/oracle',
  'verification/calculator-v1/cases/gate.mjs',
  'verification/calculator-v1/cases/README.md',
];

const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const hashFile = file => sha(fs.readFileSync(file));
const readJson = file => JSON.parse(fs.readFileSync(file, 'utf8'));
const cargoExecutable = () => process.env.CARGO || 'cargo';

// Public-repo hygiene check: rejects private home/cache/campaign paths and wrapper names.
const BANNED_HYGIENE_RE = new RegExp(
  [
    '/' + 'home/',
    '\\$HOME/\\.cache',
    '/var/tmp/' + 'noble-cairn',
    '\\.cache/' + 'noble',
    '\\b' + 'n' + 'cargo\\b',
    '\\b' + 'n' + 'lake\\b',
    '\\b' + 'n' + 'cairn\\b',
    '\\b' + 'env\\.' + 'sh\\b',
    '\\b' + 'octet\\.' + 'sh\\b',
  ].join('|'),
);

export function assertCleanHygiene(text, label = 'output') {
  const match = BANNED_HYGIENE_RE.exec(text);
  assert.equal(match, null, `${label} contains banned hygiene string: ${match?.[0]}`);
}

export function sanitizeValue(value, replacements = []) {
  if (typeof value === 'string') {
    let out = value;
    for (const [needle, placeholder] of replacements) {
      if (needle && out.includes(needle)) out = out.split(needle).join(placeholder);
    }
    return out.replace(/\/nix\/store\/[0-9a-z]{32}-([^/\s"']+)/g, '<NIX:$1>');
  }
  if (Array.isArray(value)) return value.map(item => sanitizeValue(item, replacements));
  if (value && typeof value === 'object') {
    return Object.fromEntries(Object.entries(value).map(([k, v]) => [k, sanitizeValue(v, replacements)]));
  }
  return value;
}

export function deriveVerdicts(observations, meta = { cleanTree: true, binaryChanged: false }) {
  const cases = {};
  const promotable = [];
  const promotablePartial = [];
  const unmet = [];
  const failed = [];
  const notExecuted = [];
  const blocked = [];

  const lanes = observations?.lanes ?? {};
  const cli = lanes.cli;
  const stateHost = lanes['state-host'];
  const oracle = lanes.oracle;
  const numFast = lanes['numeric-fast'];
  const numSlow = lanes['numeric-slow'];
  const cliUnit = lanes['cli-unit'];
  const budgets = lanes.budgets;

  const cliPassedFor = caseId => {
    if (!cli || cli.status === 'not-executed') return { state: 'not-executed', reason: 'cli lane not executed' };
    if (cli.status === 'failed') return { state: 'failed', reason: 'cli lane execution failed' };
    if (cli.controls_enabled && !cli.controls_ok) return { state: 'unmet', reason: 'cli negative controls failed' };
    const rows = (cli.vectors ?? []).filter(v => v.case === caseId);
    if (rows.length === 0) return { state: 'not-executed', reason: `no cli vectors selected for ${caseId}` };
    const bad = rows.filter(v => v.classification !== 'pass');
    if (bad.length > 0) return { state: 'unmet', reason: `cli vectors failed: ${bad.map(b => b.id).join(', ')}` };
    return { state: 'passed', count: rows.length };
  };

  const oracleAgreedFor = (prefix, expectedCount) => {
    if (!oracle || oracle.status === 'not-executed') return { state: 'not-executed', reason: 'oracle lane not executed' };
    if (oracle.status === 'failed') return { state: 'failed', reason: 'oracle lane execution failed' };
    if (oracle.cases_agree !== oracle.cases_total || oracle.cases_total !== 38) {
      return { state: 'unmet', reason: `oracle cases check ${oracle.cases_agree}/${oracle.cases_total}` };
    }
    const rows = (oracle.receipts ?? []).filter(r => r.id.startsWith(prefix));
    if (rows.length < expectedCount) {
      return { state: 'not-executed', reason: `oracle receipts missing for ${prefix} (${rows.length}/${expectedCount})` };
    }
    const bad = rows.filter(r => r.verdict !== 'agree');
    if (bad.length > 0) {
      return { state: 'unmet', reason: `oracle disagreement on ${bad.map(b => `${b.id}:${b.verdict}`).join(', ')}` };
    }
    return { state: 'passed', count: rows.length };
  };

  const hostReqZeroOk = () => {
    if (!stateHost || stateHost.status === 'not-executed') return { state: 'not-executed', reason: 'state-host lane not executed' };
    if (stateHost.status === 'failed') return { state: 'failed', reason: 'state-host lane failed' };
    const c = stateHost.core_engine_calculator_check;
    if (!c || c.status !== 'passed' || c.host_requests_total !== 0 || c.host_functions_count !== 0) {
      return { state: 'unmet', reason: 'calculator_core_host_requests: 0 check not satisfied' };
    }
    if (stateHost.trace_session_check?.status !== 'passed') {
      return { state: 'unmet', reason: 'trace session observation check not satisfied' };
    }
    return { state: 'passed' };
  };

  const hasTest = (lane, name) => {
    if (!lane || lane.status === 'not-executed') return 'not-executed';
    if (lane.failed_tests?.includes(name)) return 'unmet';
    if (lane.passed_tests?.includes(name)) return 'passed';
    if (lane.status === 'timeout') return 'timeout';
    if (lane.status === 'failed') return 'failed';
    return 'not-executed';
  };

  const combine = (id, checks, { partial = false, blockedReason = null } = {}) => {
    if (blockedReason !== null) {
      cases[id] = { verdict: 'blocked', promotable: false, implementation: 'absent', reason: blockedReason };
      blocked.push(id);
      return;
    }
    const failedCheck = checks.find(c => c.state === 'failed');
    const unmetCheck = checks.find(c => c.state === 'unmet');
    const missingCheck = checks.find(c => c.state === 'not-executed' || c.state === 'timeout');
    if (failedCheck) {
      cases[id] = { verdict: 'failed', promotable: false, implementation: 'absent', reason: failedCheck.reason };
      failed.push(id);
    } else if (unmetCheck) {
      cases[id] = { verdict: 'unmet', promotable: false, implementation: 'absent', reason: unmetCheck.reason };
      unmet.push(id);
    } else if (missingCheck) {
      cases[id] = { verdict: 'not-executed', promotable: false, implementation: 'absent', reason: missingCheck.reason };
      notExecuted.push(id);
    } else if (!meta.cleanTree || meta.binaryChanged) {
      const reason = !meta.cleanTree ? 'working tree was dirty (--allow-dirty)' : 'binary digest changed during run';
      cases[id] = { verdict: 'unmet', promotable: false, implementation: 'absent', reason };
      unmet.push(id);
    } else if (partial) {
      cases[id] = { verdict: 'promotable-partial', promotable: true, implementation: 'partial', execution: 'passed' };
      promotablePartial.push(id);
    } else {
      cases[id] = { verdict: 'promotable', promotable: true, implementation: 'implemented', execution: 'passed' };
      promotable.push(id);
    }
  };

  combine('CALC-01', [cliPassedFor('CALC-01'), oracleAgreedFor('CALC-01/', 15)]);
  combine('CALC-02', [
    cliPassedFor('CALC-02'),
    oracleAgreedFor('CALC-02/', 5),
    hostReqZeroOk(),
    stateHost?.calc02_state_unchanged?.status === 'passed'
      ? { state: 'passed' }
      : { state: stateHost?.status === 'not-executed' ? 'not-executed' : 'unmet', reason: 'CALC-02 multi-line state_unchanged not observed' },
  ]);
  combine('CALC-03', [
    cliPassedFor('CALC-03'),
    oracleAgreedFor('CALC-03/', 4),
    hostReqZeroOk(),
    stateHost?.calc03_state_unchanged?.status === 'passed'
      ? { state: 'passed' }
      : { state: stateHost?.status === 'not-executed' ? 'not-executed' : 'unmet', reason: 'CALC-03 multi-line state_unchanged not observed' },
  ]);
  combine('CALC-04', [
    { state: hasTest(numFast, 'calc_04_zero_denominator'), reason: 'numeric-fast calc_04_zero_denominator' },
    { state: hasTest(numSlow, 'calc_04_constructor_and_decoder_vectors'), reason: 'numeric-slow calc_04_constructor_and_decoder_vectors' },
  ]);
  combine('CALC-05', [
    { state: hasTest(numFast, 'calc_05_core_wrap_is_core_semantics'), reason: 'numeric-fast calc_05_core_wrap_is_core_semantics' },
    { state: hasTest(numFast, 'calc_05_no_implicit_conversion'), reason: 'numeric-fast calc_05_no_implicit_conversion' },
    { state: hasTest(numSlow, 'calc_05_i64_bound_vectors'), reason: 'numeric-slow calc_05_i64_bound_vectors' },
    { state: hasTest(numSlow, 'calc_05_out_of_range_below_and_non_integral'), reason: 'numeric-slow calc_05_out_of_range_below_and_non_integral' },
  ]);
  combine('CALC-06', [
    cliPassedFor('CALC-06'),
    oracleAgreedFor('CALC-06/', 1),
    hostReqZeroOk(),
    { state: hasTest(cliUnit, 'calculator::tests::failed_lines_of_every_class_restore_the_committed_session'),
      reason: 'cli-unit failed_lines_of_every_class_restore_the_committed_session' },
  ]);
  combine('CALC-07', [
    cliPassedFor('CALC-07'),
    hostReqZeroOk(),
    oracleAgreedFor('CALC-07/', 5),
  ], { partial: true });
  combine('CALC-08', [
    budgets?.status === 'passed' && budgets.matched === 20 && budgets.mismatched === 0 && budgets.resource_limit_lines_on_controls === 0
      ? { state: 'passed' }
      : { state: budgets?.status === 'failed' || budgets?.status === 'timeout' ? budgets.status : (budgets && budgets.status !== 'not-executed' ? 'unmet' : 'not-executed'),
          reason: 'budgets v2 20/20 clean-HEAD run not completed' },
    { state: hasTest(numFast, 'native_step_quota_is_not_a_guest_resource_limit'), reason: 'numeric-fast native_step_quota_is_not_a_guest_resource_limit' },
    observations?.review_2026_11_23_exists ? { state: 'passed' } : { state: 'unmet', reason: 'CALC-39 review record missing' },
  ]);
  combine('CALC-09', [
    cliPassedFor('CALC-09'),
    oracleAgreedFor('CALC-09/0', 4),
  ]);
  combine('CALC-10', [], { blockedReason: 'moved-to-MA3-per-owner-decision-17' });
  combine('CALC-11', [], { blockedReason: 'moved-to-MA3-per-owner-decision-17' });
  combine('CALC-12', [], { blockedReason: 'release-mutant-build-boundary-and-open-acorr-O3-O6-MA-12-13' });
  combine('CALC-13', [], { blockedReason: 'open-entry-gates-and-release-build-boundary' });

  return {
    promotable,
    promotable_partial: promotablePartial,
    unmet,
    failed,
    not_executed: notExecuted,
    blocked,
    cases,
  };
}

if (process.argv.includes('--self-test')) {
  const casesJson = readJson(path.join(root, 'specs/conformance/calculator-cases.json'));
  const mappingJson = readJson(path.join(root, 'verification/calculator-v1/cli-mapping.json'));
  const generated = generate(casesJson, mappingJson);
  assert.equal(generated.vectors.length, 40, 'expected 40 generated CLI vectors');
  assert.equal(generated.unsupported.length, 2, 'expected 2 unsupported-on-cli CALC-07 variants');

  const planJson = readJson(path.join(root, 'verification/calculator-v1/budgets/plan.json'));
  assert.equal(planJson.schema, 'noble-calculator-budget-plan/v2');
  assert.equal(planJson.sessions.length, 22);
  const retired = planJson.sessions.filter(s => s.retired === true);
  assert.equal(retired.length, 2);
  for (const row of retired) {
    assert.ok(row.retired_reason, 'retired row requires retired_reason');
    assert.equal(row.expect, undefined, 'retired row must not carry expect');
    assert.equal(row.lines, undefined, 'retired row must not carry lines');
  }

  // Test deriveVerdicts: complete pass -> promotable; missing numeric-slow -> CALC-04/05 not_executed; CALC-10..13 always blocked.
  const fullCliVectors = generated.vectors.map(v => ({ id: v.id, case: v.case, classification: 'pass' }));
  const fullOracleReceipts = [
    ...Array.from({ length: 15 }, (_, i) => ({ id: `CALC-01/${String(i + 1).padStart(2, '0')}`, verdict: 'agree' })),
    ...Array.from({ length: 5 }, (_, i) => ({ id: `CALC-02/${String(i + 1).padStart(2, '0')}`, verdict: 'agree' })),
    ...Array.from({ length: 4 }, (_, i) => ({ id: `CALC-03/${String(i + 1).padStart(2, '0')}`, verdict: 'agree' })),
    { id: 'CALC-06/session', verdict: 'agree' },
    ...Object.keys(mappingJson['CALC-07']).filter(k => mappingJson['CALC-07'][k].support === 'cli').map(k => ({ id: `CALC-07/${k}`, verdict: 'agree' })),
    ...Array.from({ length: 4 }, (_, i) => ({ id: `CALC-09/${String(i + 1).padStart(2, '0')}`, verdict: 'agree' })),
  ];
  const fastOnlyObs = {
    review_2026_11_23_exists: true,
    lanes: {
      cli: { status: 'passed', controls_enabled: true, controls_ok: true, vectors: fullCliVectors },
      'state-host': {
        status: 'passed',
        calc02_state_unchanged: { status: 'passed' },
        calc03_state_unchanged: { status: 'passed' },
        trace_session_check: { status: 'passed' },
        core_engine_calculator_check: { status: 'passed', host_requests_total: 0, host_functions_count: 0 },
      },
      oracle: { status: 'passed', cases_agree: 38, cases_total: 38, receipts: fullOracleReceipts },
      'numeric-fast': {
        status: 'passed',
        passed_tests: ['calc_04_zero_denominator', 'calc_05_core_wrap_is_core_semantics', 'calc_05_no_implicit_conversion', 'native_step_quota_is_not_a_guest_resource_limit'],
        failed_tests: [],
      },
      'numeric-slow': { status: 'not-executed', passed_tests: [], failed_tests: [] },
      'cli-unit': {
        status: 'passed',
        passed_tests: ['calculator::tests::failed_lines_of_every_class_restore_the_committed_session'],
        failed_tests: [],
      },
      budgets: { status: 'not-executed' },
    },
  };
  const v1 = deriveVerdicts(fastOnlyObs, { cleanTree: true, binaryChanged: false });
  assert.deepEqual(v1.promotable, ['CALC-01', 'CALC-02', 'CALC-03', 'CALC-06', 'CALC-09']);
  assert.deepEqual(v1.promotable_partial, ['CALC-07']);
  assert.deepEqual(v1.not_executed, ['CALC-04', 'CALC-05', 'CALC-08']);
  assert.deepEqual(v1.blocked, ['CALC-10', 'CALC-11', 'CALC-12', 'CALC-13']);

  const dirty = deriveVerdicts(fastOnlyObs, { cleanTree: false, binaryChanged: false });
  assert.deepEqual(dirty.promotable, []);
  assert.deepEqual(dirty.promotable_partial, []);

  // Sanitization & hygiene self-test.
  const sample = sanitizeValue(
    { bin: '/tmp/work/noble', nix: '/nix/store/0123456789abcdef0123456789abcdef-nodejs-24.13.0/bin/node' },
    [['/tmp/work/noble', '<NOBLE_BIN>']],
  );
  assert.equal(sample.bin, '<NOBLE_BIN>');
  assert.equal(sample.nix, '<NIX:nodejs-24.13.0>/bin/node');
  assertCleanHygiene(JSON.stringify(sample), 'self-test');
  assert.throws(() => assertCleanHygiene('/' + 'home/alice/repo', 'test'), /banned hygiene/);

  console.log('calculator-v1 cases gate self-test: 40 CLI vectors, plan v2, deriveVerdicts, and hygiene checks passed');
  process.exit(0);
}

const { values: opts } = parseArgs({
  options: {
    out: { type: 'string' },
    binary: { type: 'string' },
    lanes: { type: 'string', default: DEFAULT_LANES.join(',') },
    case: { type: 'string', multiple: true },
    concurrency: { type: 'string', default: '4' },
    'no-controls': { type: 'boolean', default: false },
    'allow-dirty': { type: 'boolean', default: false },
    'slow-timeout-s': { type: 'string', default: '900' },
    'write-compact': { type: 'string' },
    'self-test': { type: 'boolean', default: false },
    help: { type: 'boolean', default: false },
  },
  strict: true,
});

if (opts.help) {
  console.log('usage: node verification/calculator-v1/cases/gate.mjs --out NEW_EXTERNAL_DIR [--binary PATH] [--lanes LIST] [--case CALC-XX,...] [--concurrency N] [--slow-timeout-s S] [--allow-dirty] [--no-controls] [--write-compact PATH]');
  process.exit(0);
}

assert.ok(opts.out, '--out NEW_EXTERNAL_DIR is required');
const out = path.resolve(opts.out);
assert.ok(out !== root && !out.startsWith(`${root}${path.sep}`), '--out must stay outside the repository');
assert.equal(fs.existsSync(out), false, `--out directory must be fresh: ${out}`);

const selectedLanes = opts.lanes.split(',').map(s => s.trim()).filter(Boolean);
for (const lane of selectedLanes) {
  assert.ok(VALID_LANES.includes(lane), `unknown lane ${lane}; valid: ${VALID_LANES.join(',')}`);
}
const concurrency = Number(opts.concurrency);
assert.ok(Number.isInteger(concurrency) && concurrency > 0, '--concurrency must be a positive integer');
const slowTimeoutMs = Number(opts['slow-timeout-s']) * 1000;
assert.ok(Number.isFinite(slowTimeoutMs) && slowTimeoutMs > 0, '--slow-timeout-s must be positive');

const git = (...args) => execFileSync('git', args, { cwd: root, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 }).trim();

const dirtyStatus = git('status', '--porcelain=v1', '--untracked-files=all');
const cleanTree = dirtyStatus === '';
if (!opts['allow-dirty']) {
  assert.equal(dirtyStatus, '', `repository working tree must be clean (pass --allow-dirty for dev):\n${dirtyStatus}`);
}

const testedCommit = git('rev-parse', 'HEAD');
const files = git('ls-files', '-z', '--', ...SCOPE).split('\0').filter(Boolean).sort();
const sourceSnapshot = Object.fromEntries(files.map(f => [f, { sha256: hashFile(path.join(root, f)) }]));
const sourceRevision = `sha256:${sha(JSON.stringify(sourceSnapshot))}`;

fs.mkdirSync(out, { recursive: true, mode: 0o700 });
for (const sub of ['raw', 'state-host']) fs.mkdirSync(path.join(out, sub), { recursive: true });

const rawOutputs = [];
function writeRaw(name, text) {
  const file = path.join(out, 'raw', name);
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, text);
  rawOutputs.push({ name, file: path.relative(out, file), sha256: sha(text), bytes: Buffer.byteLength(text) });
}

let binaryPath;
let buildCommand = 'prebuilt';
if (opts.binary) {
  binaryPath = path.resolve(opts.binary);
  assert.ok(fs.statSync(binaryPath).isFile(), `missing binary: ${binaryPath}`);
} else {
  buildCommand = 'cargo build --locked --offline -p noble-cli --bin noble --no-default-features';
  const build = spawnSync(
    cargoExecutable(),
    ['build', '--locked', '--offline', '-p', 'noble-cli', '--bin', 'noble', '--no-default-features', '--message-format=json-render-diagnostics'],
    { cwd: root, encoding: 'utf8', maxBuffer: 256 * 1024 * 1024 },
  );
  writeRaw('build.stdout.txt', build.stdout ?? '');
  writeRaw('build.stderr.txt', build.stderr ?? '');
  assert.equal(build.status, 0, `cargo build failed:\n${build.stderr}`);
  const artifacts = (build.stdout ?? '').split('\n').filter(Boolean)
    .map(l => { try { return JSON.parse(l); } catch { return null; } })
    .filter(m => m && m.reason === 'compiler-artifact' && m.target?.name === 'noble' && m.executable);
  assert.equal(artifacts.length, 1, 'expected one noble executable artifact');
  binaryPath = artifacts[0].executable;
}
const binaryShaBefore = hashFile(binaryPath);

function parseCargoTestLines(stdoutText) {
  const passed = [];
  const failed = [];
  const ignored = [];
  for (const match of stdoutText.matchAll(/^test (\S+) \.\.\. (ok|FAILED|ignored)$/gm)) {
    if (match[2] === 'ok') passed.push(match[1]);
    else if (match[2] === 'FAILED') failed.push(match[1]);
    else if (match[2] === 'ignored') ignored.push(match[1]);
  }
  return { passed, failed, ignored };
}

function runCargoTests(label, args, timeoutMs = 1_800_000) {
  const started = Date.now();
  const res = spawnSync(cargoExecutable(), args, {
    cwd: root, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024, timeout: timeoutMs,
  });
  const wallMs = Date.now() - started;
  writeRaw(`${label}.stdout.txt`, res.stdout ?? '');
  writeRaw(`${label}.stderr.txt`, res.stderr ?? '');
  const parsed = parseCargoTestLines(res.stdout ?? '');
  const timedOut = res.error?.code === 'ETIMEDOUT' || (res.signal === 'SIGTERM' && wallMs >= timeoutMs - 500);
  return {
    status: timedOut ? 'timeout' : (res.status === 0 && parsed.failed.length === 0 ? 'passed' : 'failed'),
    exit_code: res.status,
    signal: res.signal ?? null,
    timed_out: timedOut,
    wall_ms: wallMs,
    passed_tests: parsed.passed,
    failed_tests: parsed.failed,
    ignored_tests: parsed.ignored,
  };
}

const observations = {
  review_2026_11_23_exists: fs.existsSync(path.join(root, 'verification/calculator-v1/budgets/review-2026-11-23.md')),
  lanes: Object.fromEntries(VALID_LANES.map(l => [l, { status: 'not-executed' }])),
};

const caseArgs = (opts.case ?? []).flatMap(c => ['--case', c]);

// 1. CLI lane (acceptance.mjs)
if (selectedLanes.includes('cli')) {
  const cliOut = path.join(out, 'cli');
  const args = [
    path.join(root, 'verification/calculator-v1/acceptance.mjs'),
    '--binary', binaryPath,
    '--binary-sha256', binaryShaBefore,
    '--source-id', testedCommit,
    '--build-profile', 'debug',
    '--out', cliOut,
    '--concurrency', String(concurrency),
    ...(opts['no-controls'] ? ['--no-controls'] : []),
    ...caseArgs,
  ];
  const started = Date.now();
  const res = spawnSync(process.execPath, args, { cwd: root, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 });
  writeRaw('cli.stdout.txt', res.stdout ?? '');
  writeRaw('cli.stderr.txt', res.stderr ?? '');
  const receiptFile = path.join(cliOut, 'receipt.json');
  if (fs.existsSync(receiptFile)) {
    const cliReceipt = readJson(receiptFile);
    observations.lanes.cli = {
      status: res.status === 0 && cliReceipt.result === 'completed' && cliReceipt.summary?.all_selected_pass ? 'passed' : 'failed',
      exit_code: res.status,
      wall_ms: Date.now() - started,
      controls_enabled: !opts['no-controls'],
      controls_ok: cliReceipt.summary?.controls_ok ?? false,
      summary: cliReceipt.summary,
      unsupported: cliReceipt.unsupported,
      vectors: (cliReceipt.vectors ?? []).map(v => ({
        id: v.id,
        case: v.case,
        classification: v.classification,
        status: v.observed?.status ?? null,
        wall_ms: v.observed?.wall_ms ?? null,
        stdin_sha256: v.stdin?.sha256 ?? null,
        stdout_sha256: v.observed?.stdout?.sha256 ?? null,
        stderr_sha256: v.observed?.stderr?.sha256 ?? null,
        reasons: v.reasons ?? [],
      })),
      controls: (cliReceipt.controls ?? []).map(c => ({
        id: c.id,
        classification: c.classification,
        control_ok: c.control_ok,
        status: c.observed?.status ?? null,
        wall_ms: c.observed?.wall_ms ?? null,
        reasons: c.reasons ?? [],
      })),
    };
  } else {
    observations.lanes.cli = { status: 'failed', exit_code: res.status, wall_ms: Date.now() - started, error: 'missing cli receipt.json' };
  }
}

// 2. state-host lane (multi-line CALC-02/03 state_unchanged + trace + CoreEngine host_requests == 0)
if (selectedLanes.includes('state-host')) {
  const casesData = readJson(path.join(root, 'specs/conformance/calculator-cases.json'));
  const byId = new Map(casesData.cases.map(c => [c.id, c]));
  const runSessionAsync = (label, lines, extraEnv = {}) => new Promise(resolve => {
    const started = Date.now();
    const child = spawn(binaryPath, ['calculator'], {
      cwd: root,
      env: { PATH: '/nonexistent', LANG: 'C', LC_ALL: 'C', ...extraEnv },
      stdio: ['pipe', 'pipe', 'pipe'],
    });
    const outChunks = [];
    const errChunks = [];
    let timedOut = false;
    child.stdout.on('data', chunk => outChunks.push(chunk));
    child.stderr.on('data', chunk => errChunks.push(chunk));
    const timer = setTimeout(() => {
      timedOut = true;
      try { child.kill('SIGKILL'); } catch { /* gone */ }
    }, 3_600_000);
    child.on('close', (status, signal) => {
      clearTimeout(timer);
      const stdout = Buffer.concat(outChunks).toString('utf8');
      const stderr = Buffer.concat(errChunks).toString('utf8');
      writeRaw(`state-host-${label}.stdout.txt`, stdout);
      writeRaw(`state-host-${label}.stderr.txt`, stderr);
      const outLines = stdout.split('\n').filter((_, i, a) => i < a.length - 1 || a[i] !== '');
      const errLines = stderr.split('\n').filter((_, i, a) => i < a.length - 1 || a[i] !== '');
      resolve({ status, signal: signal ?? null, timed_out: timedOut, wall_ms: Date.now() - started, outLines, errLines });
    });
    child.stdin.end(`${lines.join('\n')}\n`);
  });

  const c02Vecs = byId.get('CALC-02').input.vectors;
  const c03Vecs = byId.get('CALC-03').input.vectors;
  const traceFile = path.join(out, 'state-host', 'trace.jsonl');

  const [c02Res, c03Runs, traceRes] = await Promise.all([
    runSessionAsync('calc02', ['let x = 7', ...c02Vecs.map(v => v.expression), 'x']),
    Promise.all(c03Vecs.map((v, i) => runSessionAsync(`calc03-${i + 1}`, ['let x = 7', v.expression, 'x']))),
    runSessionAsync('trace', ['let x = 10', 'let x = 1 / 0', 'x', '1 + )'], { NOBLE_CALCULATOR_TRACE: traceFile }),
  ]);

  const c02Ok = c02Res.status === 1
    && JSON.stringify(c02Res.outLines) === JSON.stringify(['7', '7'])
    && c02Res.errLines.length === c02Vecs.length
    && c02Vecs.every((v, i) => c02Res.errLines[i] === `${v.category} [${v.span[0]},${v.span[1]}) provenance=unavailable`);

  const c03Ok = c03Runs.every((res, i) => res.status === 1
    && JSON.stringify(res.outLines) === JSON.stringify(['7', '7'])
    && res.errLines.length === 1
    && new RegExp(`^${c03Vecs[i].category} \\[\\d+,\\d+\\) provenance=unavailable$`).test(res.errLines[0]));

  let traceCheck = { status: 'failed', record_count: 0, module_count: 0, schemas_ok: false, outcomes_ok: false };
  if (fs.existsSync(traceFile)) {
    const records = fs.readFileSync(traceFile, 'utf8').trim().split('\n').filter(Boolean).map(l => JSON.parse(l));
    const modules = records.filter(r => r.record === 'module');
    const schemasOk = records.length > 0 && records.every(r => r.schema === 'noble-calculator-trace/1');
    const outcomesOk = modules.length > 0 && modules.every(r => r.outcome === 'normal');
    const traceOutOk = traceRes.status === 1
      && JSON.stringify(traceRes.outLines) === JSON.stringify(['10', '10'])
      && traceRes.errLines.length === 2;
    traceCheck = {
      status: traceOutOk && schemasOk && outcomesOk ? 'passed' : 'failed',
      record_count: records.length,
      module_count: modules.length,
      schemas_ok: schemasOk,
      outcomes_ok: outcomesOk,
    };
  }

  // CoreEngine Calculator-v1 host_requests == 0 + engine-calculator.test.mjs
  const { CoreEngine } = await import(new URL('../../../crates/noble-cli/src/core/runtime/host.mjs', import.meta.url));
  const selection = readJson(path.join(root, 'crates/noble-cli/src/core/runtime/config.json'));
  const abi = readJson(path.join(root, 'crates/noble-cli/src/core/runtime/abi.json'));
  let coreProfile = null;
  let hostFunctionsCount = null;
  let hostRequestsTotal = null;
  let protectedOperations = null;
  let coreCommitOutcome = null;
  const engine = new CoreEngine(selection, abi, {
    calculator: true,
    calculator_shell_state: true,
    calculator_assembler: binaryPath,
  });
  try {
    coreProfile = engine.profile;
    hostFunctionsCount = engine.hostFunctions.length;
    hostRequestsTotal = engine.hostRequestsTotal;
    protectedOperations = engine.protectedOperations;
    coreCommitOutcome = engine.calculatorCommit().outcome;
  } finally {
    engine.close();
  }
  const nodeUnit = spawnSync(
    process.execPath,
    ['--test', '--test-reporter=tap', path.join(root, 'crates/noble-cli/src/core/runtime/engine-calculator.test.mjs')],
    { cwd: root, encoding: 'utf8', env: { ...process.env, NOBLE_CALCULATOR_ASSEMBLER: binaryPath } },
  );
  writeRaw('state-host-engine-calculator-test.stdout.txt', nodeUnit.stdout ?? '');
  writeRaw('state-host-engine-calculator-test.stderr.txt', nodeUnit.stderr ?? '');
  const nodeUnitOk = nodeUnit.status === 0 && /^# skipped 0$/m.test(nodeUnit.stdout ?? '');
  const coreCheckOk = coreProfile === 'Calculator-v1'
    && hostFunctionsCount === 0
    && hostRequestsTotal === 0
    && protectedOperations === 0
    && coreCommitOutcome === 'committed'
    && nodeUnitOk;

  observations.lanes['state-host'] = {
    status: c02Ok && c03Ok && traceCheck.status === 'passed' && coreCheckOk ? 'passed' : 'failed',
    calc02_state_unchanged: {
      status: c02Ok ? 'passed' : 'failed',
      exit_code: c02Res.status,
      wall_ms: c02Res.wall_ms,
      stdout: c02Res.outLines,
      stderr: c02Res.errLines,
    },
    calc03_state_unchanged: {
      status: c03Ok ? 'passed' : 'failed',
      sessions: c03Runs.map((res, i) => ({
        expression: c03Vecs[i].expression,
        exit_code: res.status,
        wall_ms: res.wall_ms,
        stdout: res.outLines,
        stderr: res.errLines,
      })),
    },
    trace_session_check: traceCheck,
    core_engine_calculator_check: {
      status: coreCheckOk ? 'passed' : 'failed',
      profile: coreProfile,
      host_functions_count: hostFunctionsCount,
      host_requests_total: hostRequestsTotal,
      protected_operations: protectedOperations,
      commit_outcome: coreCommitOutcome,
      engine_calculator_tests_skipped_0: nodeUnitOk,
    },
  };
}

// 3. Oracle lane
if (selectedLanes.includes('oracle')) {
  const casesOut = path.join(out, 'oracle-cases.json');
  const compareScript = path.join(root, 'verification/calculator-v1/oracle/compare.mjs');
  const casesRun = spawnSync(process.execPath, [compareScript, 'cases', '--out', casesOut], {
    cwd: root, encoding: 'utf8', maxBuffer: 16 * 1024 * 1024,
  });
  writeRaw('oracle-cases.stdout.txt', casesRun.stdout ?? '');
  writeRaw('oracle-cases.stderr.txt', casesRun.stderr ?? '');
  const casesReport = fs.existsSync(casesOut) ? readJson(casesOut) : null;

  let receiptsReport = null;
  let receiptsStatus = 0;
  const cliDir = path.join(out, 'cli');
  if (fs.existsSync(path.join(cliDir, 'vectors'))) {
    const receiptsOut = path.join(out, 'oracle-receipts.json');
    const receiptsRun = spawnSync(process.execPath, [compareScript, 'receipts', '--run', cliDir, '--out', receiptsOut], {
      cwd: root, encoding: 'utf8', maxBuffer: 32 * 1024 * 1024,
    });
    receiptsStatus = receiptsRun.status;
    writeRaw('oracle-receipts.stdout.txt', receiptsRun.stdout ?? '');
    writeRaw('oracle-receipts.stderr.txt', receiptsRun.stderr ?? '');
    if (fs.existsSync(receiptsOut)) receiptsReport = readJson(receiptsOut);
  }

  const casesOk = casesRun.status === 0 && casesReport && casesReport.agree === casesReport.total;
  const receiptsOk = receiptsReport !== null && receiptsStatus === 0 && (receiptsReport.summary?.disagree ?? 0) === 0;
  observations.lanes.oracle = {
    status: casesOk && (receiptsReport === null || receiptsOk) ? 'passed' : 'failed',
    cases_agree: casesReport?.agree ?? 0,
    cases_total: casesReport?.total ?? 0,
    receipts_summary: receiptsReport?.summary ?? null,
    receipts: (receiptsReport?.results ?? []).map(r => ({
      id: r.id,
      verdict: r.verdict,
      reasons: r.reasons ?? [],
      stdin_sha256: r.stdin_sha256,
    })),
  };
}

// 4. numeric-fast lane
if (selectedLanes.includes('numeric-fast')) {
  observations.lanes['numeric-fast'] = runCargoTests(
    'numeric-fast',
    ['test', '--locked', '--offline', '-p', 'noble-wasm', '--test', 'calculator_numeric_paths'],
  );
}

// 5. numeric-slow lane
if (selectedLanes.includes('numeric-slow')) {
  observations.lanes['numeric-slow'] = runCargoTests(
    'numeric-slow',
    ['test', '--locked', '--offline', '-p', 'noble-wasm', '--test', 'calculator_numeric_paths', '--', '--ignored'],
    slowTimeoutMs,
  );
}

// 6. cli-unit lane
if (selectedLanes.includes('cli-unit')) {
  const unitRes = runCargoTests(
    'cli-unit-bin',
    ['test', '--locked', '--offline', '-p', 'noble-cli', '--no-default-features', '--bin', 'noble', 'calculator::tests'],
  );
  const engRes = runCargoTests(
    'cli-unit-engine',
    ['test', '--locked', '--offline', '-p', 'noble-cli', '--no-default-features', '--test', 'calculator_engine'],
  );
  observations.lanes['cli-unit'] = {
    status: unitRes.status === 'passed' && engRes.status === 'passed' ? 'passed' : 'failed',
    passed_tests: [...unitRes.passed_tests, ...engRes.passed_tests],
    failed_tests: [...unitRes.failed_tests, ...engRes.failed_tests],
    ignored_tests: [...unitRes.ignored_tests, ...engRes.ignored_tests],
    bin_wall_ms: unitRes.wall_ms,
    engine_wall_ms: engRes.wall_ms,
  };
}

// 7. budgets lane (CALC-08 v2 plan, debug)
if (selectedLanes.includes('budgets')) {
  const budgetOut = path.join(out, 'budgets');
  const summaryOut = path.join(out, 'budgets-summary.json');
  const started = Date.now();
  const runRes = spawnSync(
    process.execPath,
    [
      path.join(root, 'verification/calculator-v1/budgets/run.mjs'),
      '--binary', binaryPath,
      '--binary-sha256', binaryShaBefore,
      '--source-id', testedCommit,
      '--out', budgetOut,
      '--concurrency', String(Math.min(concurrency, 2)),
    ],
    { cwd: root, encoding: 'utf8', maxBuffer: 32 * 1024 * 1024, timeout: slowTimeoutMs },
  );
  const wallMs = Date.now() - started;
  writeRaw('budgets.stdout.txt', runRes.stdout ?? '');
  writeRaw('budgets.stderr.txt', runRes.stderr ?? '');
  const timedOut = runRes.error?.code === 'ETIMEDOUT' || (runRes.signal === 'SIGTERM' && wallMs >= slowTimeoutMs - 500);
  if (fs.existsSync(path.join(budgetOut, 'receipt.json'))) {
    const sumRes = spawnSync(
      process.execPath,
      [
        path.join(root, 'verification/calculator-v1/budgets/summarize.mjs'),
        path.join(root, 'verification/calculator-v1/budgets/plan.json'),
        summaryOut,
        path.join(budgetOut, 'receipt.json'),
      ],
      { cwd: root, encoding: 'utf8' },
    );
    if (sumRes.status === 0 && fs.existsSync(summaryOut)) {
      const sum = readJson(summaryOut);
      const active = (sum.sessions ?? []).filter(s => !s.retired);
      const retired = (sum.sessions ?? []).filter(s => s.retired);
      const matched = active.filter(s => s.matched).length;
      const mismatched = active.filter(s => !s.matched).length;
      const controlResLimits = active.filter(s => s.id.startsWith('control/'))
        .reduce((acc, s) => acc + (s.resource_limit_lines ?? 0), 0);
      observations.lanes.budgets = {
        status: timedOut ? 'timeout' : (mismatched === 0 && matched === 20 && controlResLimits === 0 ? 'passed' : 'failed'),
        matched,
        retired: retired.length,
        mismatched,
        resource_limit_lines_on_controls: controlResLimits,
        wall_ms: wallMs,
      };
    } else {
      observations.lanes.budgets = { status: 'failed', wall_ms: wallMs, error: 'summarize failed' };
    }
  } else {
    observations.lanes.budgets = { status: timedOut ? 'timeout' : 'failed', wall_ms: wallMs, timed_out: timedOut };
  }
}

const binaryShaAfter = hashFile(binaryPath);
const binaryChanged = binaryShaAfter !== binaryShaBefore;
const verdicts = deriveVerdicts(observations, { cleanTree, binaryChanged });
const anyLaneFailed = Object.values(observations.lanes).some(l => l.status === 'failed');

const receipt = {
  schema: 'noble-calculator-v1-cases-acceptance/v1',
  claim: 'clean-commit tiered verification of calculator-v1 cases through CLI acceptance vectors, multi-line state/host checks, independent BigInt oracle, Rust unit/numeric-path tests, and budget plan checks on one debug build (no --release); promotion verdicts are derived strictly from observed lanes',
  result: anyLaneFailed || binaryChanged ? 'failed' : 'completed',
  verdict: verdicts.failed.length === 0 && verdicts.unmet.length === 0 && verdicts.not_executed.length === 0
    ? 'all-applicable-passed'
    : (anyLaneFailed || binaryChanged ? 'run-failure' : 'partial-applicable-passed'),
  tested_commit: testedCommit,
  clean_tree: cleanTree,
  source_revision: sourceRevision,
  scope: SCOPE,
  source: sourceSnapshot,
  binary: {
    path: binaryPath,
    sha256_before: binaryShaBefore,
    sha256_after: binaryShaAfter,
    changed_during_run: binaryChanged,
    build_profile: 'debug-no-default-features',
    build_command: buildCommand,
  },
  toolchain: {
    node: process.version,
    host: os.hostname(),
    kernel: os.release(),
    cpus: os.cpus().length,
  },
  selected_lanes: selectedLanes,
  observations,
  verdicts,
  raw_outputs: rawOutputs,
  non_claims: [
    'No --release build was performed; debug execution only.',
    'Finite test vectors and unit tests do not constitute universal parser, arithmetic, or accounting proofs (`proof: open`).',
    'CALC-07 marks 2 of 7 variants (`cyclic-imported-definition-graph`, `resource-capability-capture`) `unsupported-on-cli` per owner decision 17.',
    'CALC-09 relies on the accepted `later_features` refusal reading and caveat per owner decision 26.',
    'CALC-10 and CALC-11 belong to MA3; CALC-12 and CALC-13 remain unexecuted and absent.',
  ],
};

const fullReceiptPath = path.join(out, 'acceptance.json');
fs.writeFileSync(fullReceiptPath, `${JSON.stringify(receipt, null, 2)}\n`);

let compactPath = null;
let compactSha = null;
if (opts['write-compact']) {
  const target = path.resolve(opts['write-compact']);
  const relTarget = path.relative(root, target);
  assert.ok(!relTarget.startsWith('..') && !path.isAbsolute(relTarget), '--write-compact must be inside the repository');
  const sanitized = sanitizeValue(receipt, [
    [binaryPath, '<NOBLE_BIN>'],
    [out, '<EXTERNAL_OUT>'],
    [root, '<REPO_ROOT>'],
    [process.execPath, '<NODE_BIN>'],
    [cargoExecutable(), 'cargo'],
    [os.homedir(), '<HOME>'],
  ]);
  sanitized.compact = true;
  const compactText = `${JSON.stringify(sanitized, null, 2)}\n`;
  assertCleanHygiene(compactText, 'compact acceptance receipt');
  fs.mkdirSync(path.dirname(target), { recursive: true });
  fs.writeFileSync(target, compactText);
  compactPath = relTarget;
  compactSha = sha(compactText);
}

console.log(JSON.stringify({
  receipt: fullReceiptPath,
  receipt_sha256: hashFile(fullReceiptPath),
  compact_receipt: compactPath,
  compact_sha256: compactSha,
  tested_commit: testedCommit,
  source_revision: sourceRevision,
  result: receipt.result,
  verdict: receipt.verdict,
  promotable: verdicts.promotable,
  promotable_partial: verdicts.promotable_partial,
  unmet: verdicts.unmet,
  failed: verdicts.failed,
  not_executed: verdicts.not_executed,
  blocked: verdicts.blocked,
}, null, 2));

if (anyLaneFailed || binaryChanged) process.exitCode = 1;
