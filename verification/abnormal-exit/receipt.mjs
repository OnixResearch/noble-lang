#!/usr/bin/env node
/**
 * Source-bound receipt for LREF-CASE-01 (`.cairn/archive/2026-11-23-live-reference-lifetimes`,
 * r[LREF-EXIT-01], owner decision 6): an abnormal root exit releases the root's map pin and every
 * frame pin exactly once from the host ledger, and the session continues only when that release is
 * provably exact.
 *
 * It runs `crates/noble-cli/tests/live_slot_abnormal_exit.mjs` twice:
 *   fixed - a CLI built from the clean source tree this receipt names; the lane must pass;
 *   prior - a CLI built from the commit before the change; the lane must fail at the save-time
 *           root, whose release is exact but whose session the prior host still terminates.
 *
 * Usage (pinned Node, from a repository root whose ancestors no other principal can write,
 * because the lane's authority lives under the tests directory):
 *   node verification/abnormal-exit/receipt.mjs FIXED_CLI PRIOR_CLI PRIOR_COMMIT NEW_DIRECTORY
 */
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const sha = bytes => createHash('sha256').update(bytes).digest('hex');
assert.equal(process.argv.length, 6,
  'usage: node receipt.mjs FIXED_CLI PRIOR_CLI PRIOR_COMMIT NEW_DIRECTORY');
const [fixed, prior, priorCommit, output] = process.argv.slice(2);
for (const binary of [fixed, prior]) assert.ok(path.isAbsolute(binary), 'CLI paths must be absolute');
assert.match(priorCommit, /^[0-9a-f]{40}$/, 'the prior commit must be a full hash');
assert.ok(path.isAbsolute(output) && !fs.existsSync(output), 'the receipt directory must be new');
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const git = argv => {
  const run = spawnSync('git', argv, { cwd: root, encoding: 'utf8' });
  assert.equal(run.status, 0, `git ${argv.join(' ')}: ${run.stderr}`);
  return run.stdout.trim();
};
const source = { commit: git(['rev-parse', 'HEAD']), tree: git(['rev-parse', 'HEAD^{tree}']),
  clean: git(['status', '--porcelain', '--untracked-files=all']) === '' };
assert.ok(source.clean, 'the receipt binds a clean source tree');
fs.mkdirSync(output, { mode: 0o700 });
const save = (name, content) => fs.writeFileSync(path.join(output, name),
  typeof content === 'string' ? content : JSON.stringify(content, null, 2) + '\n',
  { flag: 'wx', mode: 0o600 });
const bytesOf = relative => ({ path: relative, sha256: sha(fs.readFileSync(path.join(root, relative))) });
const lane = bytesOf('crates/noble-cli/tests/live_slot_abnormal_exit.mjs');
const bound = [lane, bytesOf('crates/noble-cli/tests/live_slot_direct_quote.rs'),
  bytesOf('crates/noble-cli/tests/live_slot_resource_ref_cli.mjs'),
  bytesOf('crates/noble-cli/src/core/runtime/slot-engine.mjs'),
  bytesOf('crates/noble-cli/src/core/runtime/slot-registry.mjs'),
  bytesOf('verification/abnormal-exit/receipt.mjs')];

const runLane = binary => {
  const before = sha(fs.readFileSync(binary));
  const run = spawnSync(process.execPath, [path.join(root, lane.path), binary],
    { encoding: 'utf8', env: { PATH: process.env.PATH, HOME: os.userInfo().homedir } });
  assert.equal(sha(fs.readFileSync(binary)), before, 'a CLI changed while the lane ran');
  const lines = (run.stdout ?? '').trim().split('\n').filter(line => line.startsWith('{'))
    .map(line => JSON.parse(line));
  const observed = (variant, label) =>
    lines.find(line => line.variant === variant && line.observation === label);
  return { status: run.status, sha256: before, stdout: run.stdout ?? '', stderr: run.stderr ?? '',
    lines, observed };
};

const fixedRun = runLane(fixed);
save('fixed.stdout', fixedRun.stdout);
assert.equal(fixedRun.status, 0, `fixed CLI failed the lane: ${fixedRun.stderr.slice(-800)}`);
const summary = fixedRun.lines.find(line => line.case === 'LREF-CASE-01');
assert.deepEqual(summary?.save_time?.failed_root?.exit_release, { exact: true });
assert.equal(summary.save_time.failed_root.session_state, 'retained');
assert.equal(summary.save_time.publication.epoch, '2');
assert.equal(summary.save_time.publication.retired_b1_spans.length, 1);
assert.equal(summary.save_time.next_root.generation, '2');
assert.equal(summary.quota.failed_root.exit_release.exact, false);
assert.equal(summary.quota.failed_root.session_state, 'terminated-after-guest-failure');
assert.equal(summary.abandoned_frames.frame_pins.released_on_abort, 2);

const priorRun = runLane(prior);
save('prior.stdout', priorRun.stdout);
assert.notEqual(priorRun.status, 0, 'the prior CLI must fail the lane');
const reproduced = priorRun.observed('save-time', 'failed-root');
assert.equal(reproduced?.outcome, 'execution-failed', 'the prior CLI must reach the save-time root');
assert.equal(reproduced.session_state, 'terminated-after-guest-failure',
  'the prior CLI terminates the session although the release is exact');
assert.equal(reproduced.exit_release, null, 'the prior CLI reports no exit release');

save('receipt.json', {
  schema: 'noble-abnormal-exit-receipt/v1', case: 'LREF-CASE-01',
  claim: 'selected host evidence for abnormal root exits of one two-frame root in a quota-1 registry; not a proof and not a conformance promotion',
  source: { ...source, bound },
  fixed: { what: 'the lane against a CLI built from the source above', binary_sha256: fixedRun.sha256,
    status: fixedRun.status, observations: fixedRun.lines.filter(line => line.variant), summary },
  prior: { what: 'the same lane against a CLI built from the commit before the change',
    declared_commit: priorCommit, binary_sha256: priorRun.sha256, status: priorRun.status,
    observations: priorRun.lines.filter(line => line.variant), reproduced },
  non_claims: [
    'one caller, one slot and one two-frame root per session; the continue variant is a failure while saving the root\'s output, the poison variant a step-quota failure inside the dispatched target',
    'the trap path with frames still entered (`released_on_abort` 2) is exercised against the host ledger in process, not through the shipped CLI, which has no mechanism that stops the guest inside a frame',
    'host abort and worker cancellation are not executed: `test.abort` is not installable in the live-slot profile, and the shipped binary has no fault model for cancellation (a failed checkpoint poisons the session)',
    'the prior binary is bound by its hash and the commit it was declared to be built from, which this script does not rebuild',
  ],
});
process.stdout.write(`${JSON.stringify({ receipt: path.join(output, 'receipt.json'),
  fixed: fixedRun.sha256, prior: priorRun.sha256 })}\n`);
