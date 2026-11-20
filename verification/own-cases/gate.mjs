#!/usr/bin/env node
// OWN-CASE acceptance gate (`.cairn/changes/ownership-lifetime-contract`).
//
// Builds the production `noble` binary from a clean checkout, then runs the committed lanes for
// OWN-CASE-01, 02, 03, 04 (non-borrow variants), 05, 06 and 07 against it. The receipt binds the
// source snapshot, the built binary (hashed before and after the run), the exact case projections
// and every raw output file it wrote. Promotion verdicts are derived from the observations: a case
// is `promotable` only when its lane observed every registered expectation.
//
// OWN-CASE-03 is the design's case: 8-leaf `compose` trees of captured I64 Programs. The lane
// records whether each tree executed; a refused deep chain is an unmet case, never a narrowed one.
// OWN-CASE-05 uses a policy-denied effect inside a dispatched frame after the Program is produced;
// `test.abort` is not installable in this profile and its internal-diagnostic refusal is recorded
// as a frontend-defect witness, not as the case's success condition.
//
// Usage (inside the pinned devShell):
//   node verification/own-cases/gate.mjs --out NEW_DIRECTORY
//
// Not a proof and not a registration. It promotes nothing by itself.
import assert from 'node:assert/strict';
import { execFileSync, spawnSync } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = fs.realpathSync(path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..'));
const SCOPE = ['crates', 'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml',
  'verification/own-cases/gate.mjs'];
const CASE_FILE = 'specs/conformance/ownership-cases.json';
const REVISION = '0.1.0-draft.5';
const CASE_IDS = ['OWN-CASE-01', 'OWN-CASE-02', 'OWN-CASE-03', 'OWN-CASE-04', 'OWN-CASE-05',
  'OWN-CASE-06', 'OWN-CASE-07'];
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const hash = file => sha(fs.readFileSync(file));

function argument(name) {
  const index = process.argv.indexOf(name);
  return index > 0 ? process.argv[index + 1] : undefined;
}

function git(...args) {
  return execFileSync('git', args, { cwd: root, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 });
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
  const project = id => {
    const one = byId.get(id);
    assert.ok(one, `case ${id} missing`);
    const { profile, kind, requirements, input, expected } = one;
    return { id, profile, kind, requirements, input, expected };
  };
  return { byId, project };
}

function buildNoble(output) {
  const build = spawnSync('cargo', ['build', '--locked', '-p', 'noble-cli', '--bin', 'noble',
    '--message-format=json-render-diagnostics'],
  { cwd: root, encoding: 'utf8', maxBuffer: 256 * 1024 * 1024 });
  writeRaw(output, 'build', build.stdout ?? '', build.stderr ?? '');
  assert.equal(build.status, 0, `cargo build failed:\n${build.stderr}`);
  const artifacts = build.stdout.split('\n').filter(Boolean).map(line => JSON.parse(line))
    .filter(message => message.reason === 'compiler-artifact'
      && message.target?.name === 'noble' && message.executable);
  assert.equal(artifacts.length, 1, 'expected one noble executable artifact');
  const executable = artifacts[0].executable;
  assert.ok(fs.statSync(executable).isFile(), 'built noble executable is missing');
  return { executable, executable_sha256: hash(executable) };
}

const node = process.execPath;
const raw = [];

function recordRaw(output, name, stream, text) {
  const file = path.join(output, `${name}.${stream}.txt`);
  fs.writeFileSync(file, text);
  raw.push({ name: `${name}.${stream}`, file: path.relative(output, file),
    sha256: sha(text), bytes: Buffer.byteLength(text) });
}

function writeRaw(output, name, stdout, stderr) {
  recordRaw(output, name, 'stdout', stdout);
  recordRaw(output, name, 'stderr', stderr);
}

function run(output, name, command, args, environment = {}) {
  const result = spawnSync(command, args, { cwd: root, encoding: 'utf8',
    maxBuffer: 64 * 1024 * 1024, timeout: 180_000,
    env: { ...process.env, ...environment } });
  writeRaw(output, name, result.stdout ?? '', result.stderr ?? '');
  return { command, args: args.map(one => one.replace(root, '.')),
    status: result.status, stdout: result.stdout ?? '', stderr: result.stderr ?? '' };
}

function parsed(text, at = 0) {
  return JSON.parse(text.slice(text.indexOf('{', at)));
}

function lines(text, prefix) {
  return text.split('\n').filter(line => line.startsWith(prefix)).map(line => JSON.parse(line));
}

// Promotion verdicts are a pure function of the observations so a failed lane can never remain
// promotable. `--self-test` checks that derivation without building anything.
function deriveVerdicts(observations) {
  const caseOf = id => observations[id]?.case ?? 'not-run';
  const receipts = Object.fromEntries(CASE_IDS.map(id => [id, {
    lane: observations[id]?.lane ?? null, case: caseOf(id) }]));
  return {
    receipts,
    promotable: CASE_IDS.filter(id => caseOf(id) === 'passed'),
    unmet: CASE_IDS.filter(id => caseOf(id) === 'unmet'),
    failed: CASE_IDS.filter(id => caseOf(id) === 'failed'),
    withheld: CASE_IDS.filter(id => caseOf(id) === 'withheld'),
  };
}

if (process.argv.includes('--self-test')) {
  const all = Object.fromEntries(CASE_IDS.map(id => [id, { case: 'passed' }]));
  const passing = deriveVerdicts(all);
  assert.deepEqual(passing.promotable, CASE_IDS);
  const oneFailed = { ...all, 'OWN-CASE-04': { case: 'failed', lane: 'lane.mjs' } };
  const derived = deriveVerdicts(oneFailed);
  assert.deepEqual(derived.promotable, CASE_IDS.filter(id => id !== 'OWN-CASE-04'));
  assert.deepEqual(derived.failed, ['OWN-CASE-04']);
  assert.equal(derived.receipts['OWN-CASE-04'].case, 'failed');
  const oneUnmet = { ...all, 'OWN-CASE-03': { case: 'unmet' } };
  assert.deepEqual(deriveVerdicts(oneUnmet).promotable, CASE_IDS.filter(id => id !== 'OWN-CASE-03'));
  assert.equal(deriveVerdicts(oneUnmet).receipts['OWN-CASE-03'].case, 'unmet');
  console.log('gate self-test: a failing case is never promotable');
  process.exit(0);
}

const out = argument('--out');
assert.ok(out, 'pass --out NEW_DIRECTORY');
assert.equal(fs.existsSync(out), false, '--out must not already exist');
fs.mkdirSync(out, { mode: 0o700 });

const snapshot = sourceSnapshot();
const sourceRevision = `sha256:${sha(JSON.stringify(snapshot))}`;
const testedCommit = git('rev-parse', 'HEAD').trim();
const { byId, project } = cases();
const projections = CASE_IDS.map(project);
const projectionDigest = sha(JSON.stringify(projections));
const binary = buildNoble(out);

const observations = {};
const runs = [];

// A registered expectation that the lane did not observe is `unmet`; a lane that fails to run, or
// that cannot report a field at all, is `failed`.
class Mismatch extends Error {}

function expectValue(label, actual, expected) {
  if (JSON.stringify(actual) !== JSON.stringify(expected)) {
    throw new Mismatch(`${label}: observed ${JSON.stringify(actual)}, `
      + `registered ${JSON.stringify(expected)}`);
  }
}

function observe(id, lane, body) {
  try {
    const observation = body(byId.get(id).expected);
    assert.equal(typeof observation.case, 'string', `${id}: observation has no verdict`);
    observations[id] = { ...observation, lane: observation.lane ?? lane };
  } catch (error) {
    observations[id] = { case: error instanceof Mismatch ? 'unmet' : 'failed', lane,
      diagnostic: String(error?.message ?? error) };
  }
}

// OWN-CASE-01: the direct typed quote retains its boxed capture and runs after root exit. The lane
// writes its own JSONL evidence; the gate reads the values the CLI actually returned.
observe('OWN-CASE-01', 'crates/noble-cli/tests/live_slot_direct_quote.mjs', expected => {
  const evidence = path.join(out, 'own-01.evidence.jsonl');
  const executed = run(out, 'own-01',
    node, [path.join(root, 'crates/noble-cli/tests/live_slot_direct_quote.mjs'),
      binary.executable], { NOBLE_SLOT_QUOTE_EVIDENCE: evidence });
  runs.push(executed);
  recordRaw(out, 'own-01.evidence', 'stdout', fs.readFileSync(evidence, 'utf8'));
  const rows = fs.readFileSync(evidence, 'utf8').split('\n').filter(Boolean).map(line => JSON.parse(line));
  const replies = rows.filter(row => row.kind === 'cli').map(row => row.row);
  const invokes = replies.filter(row => row.outcome === 'executed' && row.stack);
  const quoted = invokes.find(row => row.stack[0]?.kind === 4);
  const ran = invokes.find(row => row.stack[0]?.kind === 1);
  const discarded = replies.find(row => row.outcome === 'discarded');
  const stale = replies.filter(row => row.outcome === 'refused');
  const exit = rows.find(row => row.kind === 'exit');
  assert.equal(executed.status, 0, executed.stderr);
  assert.ok(quoted && ran && discarded && exit, 'the lane evidence lacks a reply');
  expectValue('OWN-CASE-01 capture', quoted.stack[0].capture_values?.[0]?.value, '5');
  expectValue('OWN-CASE-01 result', ran.stack, [{ kind: 1, value: String(expected.result) }]);
  expectValue('OWN-CASE-01 guest_requests', ran.guest_requests, expected.guest_requests);
  expectValue('OWN-CASE-01 stale_handle_traps', exit.code, 0);
  assert.ok(stale.length >= 1, 'a released owner must be refused');
  expectValue('OWN-CASE-01 retired spans', discarded.retire_code_spans?.length, 1);
  return {
    case: 'passed',
    lane: 'crates/noble-cli/tests/live_slot_direct_quote.mjs',
    capture: quoted.stack[0].capture_values,
    result: ran.stack,
    guest_requests: ran.guest_requests,
    refused_released_owner: stale.length > 0,
    retired_code_spans: discarded.retire_code_spans?.length ?? null,
    exit: exit.code,
  };
});

// OWN-CASE-02 and OWN-CASE-03: shared duplicates in both orders and the design's captured-I64
// trees. A deep chain refused by the summary node bound makes OWN-CASE-03 unmet, with the refusal
// recorded exactly as observed.
observe('OWN-CASE-02', 'crates/noble-cli/tests/live_slot_ownership_cases.mjs', expected => {
  const executed = run(out, 'own-02-03',
    node, [path.join(root, 'crates/noble-cli/tests/live_slot_ownership_cases.mjs'),
      binary.executable]);
  runs.push(executed);
  const packet = parsed(executed.stdout);
  const shared = packet.cases['OWN-CASE-02'];
  const orders = shared.map(row => row.order);
  const survivors = shared.map(row => row.survivor);
  expectValue('OWN-CASE-02 orders', orders, ['A-then-B', 'B-then-A']);
  expectValue('OWN-CASE-02 survivor results', survivors,
    shared.map(() => [{ kind: 1, value: String(expected.remaining_owner_result_after_first_release) }]));
  expectValue('OWN-CASE-02 exits', shared.map(row => row.exit), [0, 0]);
  return { case: 'passed', orders, survivor_results: survivors };
});

observe('OWN-CASE-03', 'crates/noble-cli/tests/live_slot_ownership_cases.mjs', expected => {
  const executed = run(out, 'own-03',
    node, [path.join(root, 'crates/noble-cli/tests/live_slot_ownership_cases.mjs'),
      binary.executable]);
  runs.push(executed);
  const trees = parsed(executed.stdout).cases['OWN-CASE-03'];
  const treeOf = (name, row) => {
    const registered = expected.variants[name];
    if (!row || row.refused === true) {
      // The design's tree did not execute; record the refusal rather than narrowing the variant.
      throw new Mismatch(`OWN-CASE-03 ${name} did not execute: ${JSON.stringify(row ?? null)}`);
    }
    expectValue(`OWN-CASE-03 ${name} released`, row.released_before_root,
      registered.released_before_root);
    expectValue(`OWN-CASE-03 ${name} before`, row.before,
      [{ kind: 1, value: String(registered.result_before) }]);
    expectValue(`OWN-CASE-03 ${name} after`, row.after,
      [{ kind: 1, value: String(registered.result_after) }]);
    expectValue(`OWN-CASE-03 ${name} exit`, row.exit, 0);
    return { name, executed: true, released_before_root: row.released_before_root,
      before: row.before, after: row.after };
  };
  const balanced = treeOf('balanced', trees.balanced);
  const left = treeOf('left-deep', trees.left_deep);
  const right = treeOf('right-deep', trees.right_deep);
  const control = trees.bounded_control;
  expectValue('OWN-CASE-03 control refusal point', control?.refused_at_compose,
    expected.bounded_control.refused_at_compose);
  expectValue('OWN-CASE-03 control diagnostic', control?.diagnostic,
    expected.bounded_control.diagnostic);
  expectValue('OWN-CASE-03 control session', control?.session_state,
    expected.bounded_control.session_state);
  expectValue('OWN-CASE-03 control exit', control?.exit, 2);
  return {
    case: 'passed',
    balanced, left_deep: left, right_deep: right, bounded_control: control,
    declared_bound: expected.summary_bound,
  };
});

// OWN-CASE-04: the registered `Account@1` nominal and source programs, through the real CLI.
observe('OWN-CASE-04', 'crates/noble-cli/tests/live_slot_own_resource_rejection.mjs', expected => {
  const executed = run(out, 'own-04',
    node, [path.join(root, 'crates/noble-cli/tests/live_slot_own_resource_rejection.mjs'),
      binary.executable]);
  runs.push(executed);
  const observed = parsed(executed.stdout);
  const names = observed.variants.map(row => row.variant);
  const rejecting = Object.entries(expected.variants)
    .filter(([, value]) => value === 'reject').map(([name]) => name);
  const inapplicable = Object.entries(expected.variants)
    .filter(([, value]) => value === 'inapplicable-no-borrow-profile').map(([name]) => name);
  expectValue('OWN-CASE-04 executed variants', names, rejecting);
  for (const row of observed.variants) {
    expectValue(`OWN-CASE-04 ${row.variant} outcome`, row.outcome, 'refused');
    expectValue(`OWN-CASE-04 ${row.variant} stage`, row.stage, 'command');
    expectValue(`OWN-CASE-04 ${row.variant} guest requests`, row.guest_requests,
      expected.guest_requests);
    expectValue(`OWN-CASE-04 ${row.variant} protected operations`, row.protected_operations,
      expected.protected_operations);
    expectValue(`OWN-CASE-04 ${row.variant} emission`, row.emitted, false);
    assert.match(row.diagnostic, /kernel rejected a non-capturable value/,
      `${row.variant} was not refused by the eligibility rule: ${row.diagnostic}`);
    assert.match(row.diagnostic, /required S Data/, `${row.variant}: ${row.diagnostic}`);
    assert.match(row.diagnostic, /Resource\(ResourceKind\(41\)\)/,
      `${row.variant} refusal does not name the registered resource: ${row.diagnostic}`);
  }
  expectValue('OWN-CASE-04 controls', observed.controls.map(row => row.outcome),
    ['installed', 'installed']);
  for (const row of observed.controls) {
    expectValue(`OWN-CASE-04 control ${row.control} emission`, row.emitted, true);
    assert.equal(typeof row.artifact_sha256, 'string', JSON.stringify(row));
  }
  return {
    case: 'passed', variants: observed.variants, controls: observed.controls,
    inapplicable_variants: inapplicable, binary_sha256: observed.selected_binary_sha256,
  };
});

// OWN-CASE-05: an abnormal exit that produces a Program, with the per-root frame-pin ledger.
observe('OWN-CASE-05', 'crates/noble-cli/tests/live_slot_own_trap.mjs', expected => {
  const executed = run(out, 'own-05',
    node, [path.join(root, 'crates/noble-cli/tests/live_slot_own_trap.mjs'), binary.executable]);
  runs.push(executed);
  const observed = parsed(executed.stdout);
  const exit = observed.abnormal_exit;
  const witness = observed.abort_refusal_witness;
  const ledger = observed.host_ledger;
  expectValue('OWN-CASE-05 saved owners', exit.saved_owner_tokens_issued,
    expected.saved_owner_tokens_issued);
  expectValue('OWN-CASE-05 frame pins released', exit.frame_pins.released, exit.frame_pins.entered);
  expectValue('OWN-CASE-05 frame pins active', exit.frame_pins.active,
    expected.frame_pins_active_after);
  expectValue('OWN-CASE-05 guest instance', exit.guest_instance, expected.guest_instance);
  expectValue('OWN-CASE-05 session state', exit.session_state,
    expected.session_state.observed_rule);
  assert.equal(expected.retained_owner_reaches_failed_root_cell, false,
    'the registered case forbids a retained owner reaching the failed root cell');
  expectValue('OWN-CASE-05 dispatch trace', exit.target_trace,
    [['dispatch', 'allowed'], ['effect', 'denied']]);
  expectValue('OWN-CASE-05 abort witness', witness.outcome, 'refused');
  expectValue('OWN-CASE-05 abort witness guest requests', witness.guest_requests, 0);
  expectValue('OWN-CASE-05 ledger pins', ledger.frame_pins.released, ledger.frame_pins.entered);
  expectValue('OWN-CASE-05 normal finish', ledger.normal_finish_refused, true);
  expectValue('OWN-CASE-05 exit', exit.exit, 2);
  return {
    case: 'passed',
    abnormal_exit: exit, abort_refusal_witness: witness, host_ledger: ledger,
    unobserved: ['the backend frame-pin counter itself (the exposed count is the host ledger of '
      + 'frame entries and exits)', 'a trap produced by `test.abort` (not installable)'],
  };
});

// OWN-CASE-06: the saved named caller survives module-root retirement and code retires on release.
observe('OWN-CASE-06', 'crates/noble-cli/tests/live_slot_module_root_retention.mjs', expected => {
  const executed = run(out, 'own-06',
    node, [path.join(root, 'crates/noble-cli/tests/live_slot_module_root_retention.mjs'),
      binary.executable]);
  runs.push(executed);
  const observed = lines(executed.stdout, '{"case":"OWN-CASE-06"')[0];
  assert.ok(observed, 'the module-root lane printed no observation');
  expectValue('OWN-CASE-06 before', observed.before,
    [{ kind: 1, value: String(expected.saved_result) }]);
  expectValue('OWN-CASE-06 after supersession', observed.after_supersession, observed.before);
  assert.ok(observed.retired_span, 'the module code span must retire after the owner release');
  return { case: 'passed', before: observed.before,
    after_supersession: observed.after_supersession, retired_span: observed.retired_span };
});

// OWN-CASE-07: the retained old definition still yields 1, stays pinned, and retires on release.
// The test prints its observed values; they are read from the line, not declared here.
observe('OWN-CASE-07', 'crates/noble-cli/tests/live_repl.rs', expected => {
  const executed = run(out, 'own-07', 'cargo', ['test', '--locked', '-p', 'noble-cli', '--test',
    'live_repl', 'retained_old_definition_yields_old_value_and_retires_after_release', '--',
    '--nocapture', '--test-threads=1']);
  runs.push(executed);
  // The libtest progress prefix shares the line with the captured output, and the object's keys
  // are emitted in sorted order, so locate the id and parse from the enclosing brace.
  const observed = executed.stdout.split('\n').flatMap(line => {
    const at = line.indexOf('"id":"OWN-CASE-07"');
    const open = at < 0 ? -1 : line.lastIndexOf('{', at);
    return open < 0 ? [] : [JSON.parse(line.slice(open))];
  })[0];
  assert.ok(observed, 'the retained-definition test printed no observation');
  expectValue('OWN-CASE-07 saved result', observed.saved_result?.[0]?.value,
    String(expected.saved_result));
  expectValue('OWN-CASE-07 retained span', observed.old_span_retained_while_owner_lives,
    expected.old_code_span_retained_while_saved_owner_lives);
  expectValue('OWN-CASE-07 retired span', observed.old_span_retired_after_release,
    expected.old_code_span_retired_after_release);
  expectValue('OWN-CASE-07 guest requests', observed.guest_requests, expected.guest_requests);
  expectValue('OWN-CASE-07 exit', executed.status, 0);
  return { case: 'passed', saved_result: observed.saved_result,
    old_span_retained_while_owner_lives: observed.old_span_retained_while_owner_lives,
    old_span_retired_after_release: observed.old_span_retired_after_release,
    released_observed_roots: observed.released_observed_roots };
});

const { receipts, promotable, unmet, withheld } = deriveVerdicts(observations);
const failed = deriveVerdicts(observations).failed;
const binaryAfter = hash(binary.executable);
if (binaryAfter !== binary.executable_sha256) {
  failed.push('binary changed during the run');
  receipts.binary = { case: 'failed' };
}
const failedCases = [...unmet, ...failed];
const verdict = failedCases.length === 0 ? 'all-passed'
  : failed.length ? 'lane-failure' : 'unmet-cases';

const acceptance = {
  schema: 'noble-own-cases-acceptance/v1',
  claim: 'selected host executions of OWN-CASE-01..07 on one CLI built from the recorded clean '
    + 'commit; promotion verdicts are derived from the observations, not declared here',
  result: failed.length ? 'failed' : 'completed',
  verdict,
  failures: [...unmet.map(id => `${id}: observed values differ from the registered expectation`),
    ...failed.map(id => `${id}: lane or run failed`)],
  tested_commit: testedCommit,
  source_revision: sourceRevision,
  scope: SCOPE,
  source: snapshot,
  case_file: CASE_FILE,
  projections: { sha256: projectionDigest, ids: CASE_IDS },
  binary: { ...binary, executable_sha256_after: binaryAfter },
  build: 'cargo build --locked -p noble-cli --bin noble',
  runs: runs.map(row => ({ command: row.command, args: row.args, status: row.status })),
  raw_outputs: raw,
  observations,
  receipts,
  promotable,
  failed_cases: failedCases,
  withheld,
  non_claims: [
    'OWN-CASE-03 executes the design\'s captured-I64 leaves (`quote [ + ] compose`, one dynamic I64 capture each). The balanced tree runs (36 before and after 14 releases); the left- and right-deep 8-leaf chains are refused at compose 5 by the anonymous summary\'s 32-node bound. The design\'s deep trees are therefore not executed, and the case is not promoted on this receipt.',
    'OWN-CASE-05 uses a policy-denied effect inside a dispatched frame after the root produced a Program. The exposed per-root frame-pin count is the host ledger of frame entries and exits; the backend\'s own pin counter is still not exposed. `test.abort` is not installable in this profile and its internal-diagnostic refusal is recorded as a frontend-defect witness, not as success.',
    'OWN-CASE-02 observes shared-owner order independence through the surviving owner running and released owners being refused; the runtime exposes no live-cell count, so reclamation timing is not directly observed.',
    'OWN-CASE-04 registers and executes the design\'s `Account@1` opaque nominal and source programs through the real CLI. The borrow variants are inapplicable (owner decision 2026-11-20, decision 7: no borrow profile is selected).',
    'No proof is claimed: every promoted case keeps proof open, and no lowering, runtime or host-ledger correspondence proof is supplied here.',
  ],
};
const receiptPath = path.join(out, 'acceptance.json');
fs.writeFileSync(receiptPath, `${JSON.stringify(acceptance, null, 2)}\n`);
console.log(JSON.stringify({ receipt: receiptPath, receipt_sha256: hash(receiptPath),
  result: acceptance.result, verdict, failures: acceptance.failures, tested_commit: testedCommit,
  source_revision: sourceRevision, promotable, observations }, null, 1));
// An unmet case is a recorded outcome, not a gate crash; a lane failure is.
if (failed.length) process.exitCode = 1;
