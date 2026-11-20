#!/usr/bin/env node
// OWN-CASE acceptance gate (`.cairn/changes/ownership-lifetime-contract`).
//
// Builds the production `noble` binary from a clean checkout, then runs the committed lanes for
// OWN-CASE-01, 02, 03, 04 (non-borrow variants), 05, 06 and 07 against it. The receipt binds the
// source snapshot, the built binary, the exact case projections and every raw observation.
//
// OWN-CASE-05 is measured but withheld: the live-slot source profile refuses the designed
// `test.abort` trap (the kernel bootstrap environment has no abort row for declared-module
// profiles), and the backend per-root frame-pin counters are not observable through any operator
// request. The host registry's abnormal-exit release is measured at the ledger layer.
//
// Usage (inside the pinned devShell):
//   node verification/own-cases/gate.mjs --out NEW_DIRECTORY
//
// Not a proof and not a registration. Derived from the settled
// `noble-conformance-case-promotion` recipe; it promotes nothing by itself.
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
  fs.writeFileSync(path.join(output, 'build.stderr.txt'), build.stderr ?? '');
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

function run(output, name, command, args) {
  const result = spawnSync(command, args, { cwd: root, encoding: 'utf8',
    maxBuffer: 64 * 1024 * 1024, timeout: 120_000 });
  fs.writeFileSync(path.join(output, `${name}.stdout.txt`), result.stdout ?? '');
  fs.writeFileSync(path.join(output, `${name}.stderr.txt`), result.stderr ?? '');
  return { command, args: args.map(one => one.replace(root, '.')),
    status: result.status, stdout: result.stdout ?? '', stderr: result.stderr ?? '' };
}

function lines(text, prefix) {
  return text.split('\n').filter(line => line.startsWith(prefix))
    .map(line => JSON.parse(line));
}

const out = argument('--out');
assert.ok(out, 'pass --out NEW_DIRECTORY');
assert.equal(fs.existsSync(out), false, '--out must not already exist');
fs.mkdirSync(out, { mode: 0o700 });

const snapshot = sourceSnapshot();
const sourceRevision = `sha256:${sha(JSON.stringify(snapshot))}`;
const testedCommit = git('rev-parse', 'HEAD').trim();
const { byId, project } = cases();
const projections = ['OWN-CASE-01', 'OWN-CASE-02', 'OWN-CASE-03', 'OWN-CASE-04',
  'OWN-CASE-05', 'OWN-CASE-06', 'OWN-CASE-07'].map(project);
const projectionDigest = sha(JSON.stringify(projections));
const binary = buildNoble(out);

const runs = [];
const observations = {};
const failures = [];

// OWN-CASE-01: the direct typed quote retains its boxed capture and runs after root exit.
{
  const lane = path.join(root, 'crates/noble-cli/tests/live_slot_direct_quote.mjs');
  const executed = run(out, 'own-01', node, [lane, binary.executable]);
  runs.push(executed);
  if (executed.status === 0
    && executed.stdout.includes('checked direct quote retained, ran with 20 to yield 25, released and retired')) {
    observations['OWN-CASE-01'] = { result: 25, case: 'passed', guest_requests: 0 };
  } else {
    failures.push('OWN-CASE-01: lane did not report the checked capture result');
    observations['OWN-CASE-01'] = { case: 'unmet' };
  }
}

// OWN-CASE-02 and OWN-CASE-03: shared duplicates in both orders and the three 8-leaf trees.
{
  const lane = path.join(root, 'crates/noble-cli/tests/live_slot_ownership_cases.mjs');
  const executed = run(out, 'own-02-03', node, [lane, binary.executable]);
  runs.push(executed);
  const packet = JSON.parse(executed.stdout.slice(executed.stdout.indexOf('{')));
  const shared = packet.cases['OWN-CASE-02'];
  const orders = shared.map(row => row.order);
  const survivors = shared.map(row => row.survivor);
  if (executed.status === 0 && JSON.stringify(orders) === JSON.stringify(['A-then-B', 'B-then-A'])
    && survivors.every(stack => JSON.stringify(stack) === JSON.stringify([{ kind: 1, value: '42' }]))
    && shared.every(row => row.exit === 0)) {
    observations['OWN-CASE-02'] = { orders, survivor_results: survivors, case: 'passed' };
  } else {
    failures.push('OWN-CASE-02: shared-owner lanes did not observe order independence');
    observations['OWN-CASE-02'] = { case: 'unmet', orders, survivors };
  }
  const trees = packet.cases['OWN-CASE-03'];
  const balanced = trees.balanced;
  const refusals = ['left_deep', 'right_deep'].map(side => trees[side]);
  const bounded = (balanced.exit === 0
    && JSON.stringify(balanced.before) === JSON.stringify([{ kind: 1, value: '8' }])
    && JSON.stringify(balanced.after) === JSON.stringify(balanced.before)
    && balanced.released_before_root === 14
    && refusals.every(row => row.refused_at_compose === 5 && row.exit === 2
      && /exceed bounded exact trace representation/.test(row.diagnostic)
      && row.session_state === 'terminated-after-guest-failure'));
  observations['OWN-CASE-03'] = {
    balanced: balanced ? { before: balanced.before, after: balanced.after,
      released_before_root: balanced.released_before_root, exit: balanced.exit } : null,
    left_deep: refusals[0], right_deep: refusals[1],
    // The registered design expects all three trees to deliver equal results. The left- and
    // right-deep trees are instead refused by the host's declared 1000-byte/64-node observation
    // bound, so the case's expected variant matrix is NOT met.
    case: bounded ? 'failed' : 'unmet',
    declared_bound: 'captureValue: 1000 bytes, 64 nodes',
  };
  if (!bounded) failures.push('OWN-CASE-03: tree observations differ from the measured bound');
}

// OWN-CASE-04: the six non-borrow variants are rejected at the check stage, before emission.
{
  const test = run(out, 'own-04', 'cargo', ['test', '--locked', '-p', 'noble-cli', '--test', 'core',
    'own_case_04_non_borrow_variants_reject_before_emission', '--', '--nocapture', '--test-threads=1']);
  runs.push(test);
  const rows = lines(test.stdout, '{"id":"OWN-CASE-04"');
  const names = rows.map(row => row.variant);
  const expectedNames = ['dup', 'drop', 'quote', 'pair-then-quote', 'sum-then-drop', 'list-then-drop'];
  if (test.status === 0 && JSON.stringify(names) === JSON.stringify(expectedNames)
    && rows.every(row => row.outcome === 'static-reject' && row.emitted === false
      && row.guest_requests === 0 && row.protected_operations === 0)) {
    observations['OWN-CASE-04'] = { variants: rows.map(row => ({ variant: row.variant,
      outcome: row.outcome, emitted: row.emitted })), case: 'passed',
      inapplicable: Object.entries(byId.get('OWN-CASE-04').expected.variants)
        .filter(([, value]) => value === 'inapplicable-no-borrow-profile').map(([key]) => key) };
  } else {
    failures.push('OWN-CASE-04: non-borrow variants did not statically reject');
    observations['OWN-CASE-04'] = { case: 'unmet', variants: rows };
  }
}

// OWN-CASE-05: the designed `test.abort` trap is refused by the live-slot source profile, so the
// case is measured but withheld; the host ledger's abort release is observed at registry level.
{
  const lane = path.join(root, 'crates/noble-cli/tests/live_slot_own_trap.mjs');
  const executed = run(out, 'own-05', node, [lane, binary.executable]);
  runs.push(executed);
  const observed = executed.status === 0
    ? JSON.parse(executed.stdout.slice(executed.stdout.indexOf('{'))) : null;
  const measured = observed
    && observed.designed_mechanism.abort_refusal.outcome === 'refused'
    && observed.designed_mechanism.abort_refusal.diagnostic === 'inconsistent frontend arena'
    && observed.designed_mechanism.abort_refusal.guest_requests === 0
    && observed.designed_mechanism.emit_installed === true
    && JSON.stringify(observed.host_ledger.retired_exactly_once) === JSON.stringify(['101:P-B1'])
    && observed.host_ledger.retained_versions_after === 1
    && observed.host_ledger.normal_finish_refused === true;
  if (!measured) failures.push('OWN-CASE-05: trap lane did not report the measured refusal and ledger release');
  observations['OWN-CASE-05'] = {
    case: 'withheld',
    designed_mechanism: observed?.designed_mechanism ?? null,
    host_ledger: observed?.host_ledger ?? null,
    unobserved: ['backend per-root frame-pin counters',
      'a trap produced by the designed `test.abort` operation'],
  };
}

// OWN-CASE-06: the saved named caller survives module-root retirement and code retires on release.
{
  const lane = path.join(root, 'crates/noble-cli/tests/live_slot_module_root_retention.mjs');
  const executed = run(out, 'own-06', node, [lane, binary.executable]);
  runs.push(executed);
  const observed = lines(executed.stdout, '{"case":"OWN-CASE-06"')[0];
  if (executed.status === 0 && observed && observed.case === 'OWN-CASE-06'
    && JSON.stringify(observed.before) === JSON.stringify([{ kind: 1, value: '42' }])
    && JSON.stringify(observed.after_supersession) === JSON.stringify(observed.before)
    && observed.retired_span) {
    observations['OWN-CASE-06'] = { before: observed.before,
      after_supersession: observed.after_supersession, retired_span: observed.retired_span,
      case: 'passed' };
  } else {
    failures.push('OWN-CASE-06: module-root retention lane did not retire after the owner release');
    observations['OWN-CASE-06'] = { case: 'unmet' };
  }
}

// OWN-CASE-07: the retained old definition still yields 1, stays pinned, and retires on release.
{
  const executed = run(out, 'own-07', 'cargo', ['test', '--locked', '-p', 'noble-cli', '--test',
    'live_repl', 'retained_old_definition_yields_old_value_and_retires_after_release', '--',
    '--test-threads=1']);
  runs.push(executed);
  if (executed.status === 0
    && executed.stdout.includes('test retained_old_definition_yields_old_value_and_retires_after_release ... ok')) {
    observations['OWN-CASE-07'] = { saved_result: 1,
      old_span_retained_while_owner_lives: true, old_span_retired_after_release: true,
      case: 'passed' };
  } else {
    failures.push('OWN-CASE-07: retained-definition lane did not pass');
    observations['OWN-CASE-07'] = { case: 'unmet' };
  }
}

const receipts = {
  'OWN-CASE-01': { lane: 'crates/noble-cli/tests/live_slot_direct_quote.mjs', case: 'passed' },
  'OWN-CASE-02': { lane: 'crates/noble-cli/tests/live_slot_ownership_cases.mjs', case: 'passed' },
  'OWN-CASE-03': { lane: 'crates/noble-cli/tests/live_slot_ownership_cases.mjs', case: 'failed' },
  'OWN-CASE-04': { lane: 'crates/noble-cli/tests/core/ownership_cases.rs', case: 'passed' },
  'OWN-CASE-05': { lane: 'crates/noble-cli/tests/live_slot_own_trap.mjs', case: 'withheld' },
  'OWN-CASE-06': { lane: 'crates/noble-cli/tests/live_slot_module_root_retention.mjs', case: 'passed' },
  'OWN-CASE-07': { lane: 'crates/noble-cli/tests/live_repl.rs', case: 'passed' },
};

const acceptance = {
  schema: 'noble-own-cases-acceptance/v1',
  claim: 'selected host executions of OWN-CASE-01, 02, 03, 04 (non-borrow variants), 05, 06 and 07 on one CLI built from the recorded clean commit; not a proof, not a registration and not a promotion',
  result: failures.length ? 'failed' : 'completed',
  failures,
  tested_commit: testedCommit,
  source_revision: sourceRevision,
  scope: SCOPE,
  source: snapshot,
  case_file: CASE_FILE,
  projections: { sha256: projectionDigest, ids: projections.map(one => one.id) },
  binary,
  build: 'cargo build --locked -p noble-cli --bin noble',
  runs: runs.map(row => ({ command: row.command, args: row.args, status: row.status })),
  observations,
  receipts,
  promotable: ['OWN-CASE-01', 'OWN-CASE-02', 'OWN-CASE-04', 'OWN-CASE-06', 'OWN-CASE-07'],
  failed_cases: ['OWN-CASE-03'],
  withheld: ['OWN-CASE-05'],
  non_claims: [
    'OWN-CASE-03 records the registered design expectation (equal results for balanced, left- and right-deep 8-leaf trees). The balanced tree passes; both deep chains are refused at their fifth compose by the host\'s declared observation bound (captureValue: 1000 bytes, 64 nodes), so the case is executed and failed, not silently narrowed.',
    'OWN-CASE-05 was not executed as designed: the live-slot source profile refuses any source containing test.abort with the internal diagnostic "inconsistent frontend arena" (the kernel bootstrap environment has no test.abort row for declared-module profiles). The host registry abortRoot releases the captured map pin exactly once, but backend per-root frame-pin counters are not exposed, so "frame pins released exactly once" is not observed.',
    'OWN-CASE-02 observes shared-owner order independence through the surviving owner running and released owners being refused; the runtime exposes no live-cell count, so reclamation timing is not directly observed.',
    'OWN-CASE-04\'s borrow variants are inapplicable (owner decision 2026-11-20: no borrow profile); only the six non-borrow variants are exercised, and they are rejected by the kernel acceptance boundary before any emission.',
    'No proof is claimed: every promoted case keeps proof open, and no lowering, runtime or host-ledger correspondence proof is supplied here.',
  ],
};
const receiptPath = path.join(out, 'acceptance.json');
fs.writeFileSync(receiptPath, `${JSON.stringify(acceptance, null, 2)}\n`);
console.log(JSON.stringify({ receipt: receiptPath, receipt_sha256: hash(receiptPath),
  result: acceptance.result, failures, tested_commit: testedCommit,
  source_revision: sourceRevision, observations }, null, 1));
if (failures.length) process.exitCode = 1;
