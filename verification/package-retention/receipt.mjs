#!/usr/bin/env node
/**
 * Source-bound receipt for LREF-CASE-03 (`.cairn/changes/live-reference-lifetimes`): a Program
 * saved from a slot dispatch keeps its version's package across replacement, and is charged for it
 * against the retained-version quota until it is released.
 *
 * It runs `crates/noble-cli/tests/live_slot_package_retention.mjs` twice:
 *   fixed - a CLI built from the clean source tree this receipt names; the lane must pass;
 *   prior - a CLI built from the commit before the fix; its `replacement` session must pass and its
 *           `charge` session must fail because v2 is published while the saved owner lives, which
 *           is the reproduced defect.
 *
 * Usage (pinned Node, env -i, from a repository root whose ancestors no other principal can
 * write, because the lane's authority lives under the tests directory):
 *   node verification/package-retention/receipt.mjs FIXED_CLI PRIOR_CLI PRIOR_COMMIT NEW_TMP_DIRECTORY
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
  'usage: node receipt.mjs FIXED_CLI PRIOR_CLI PRIOR_COMMIT NEW_TMP_DIRECTORY');
const [fixed, prior, priorCommit, output] = process.argv.slice(2);
for (const binary of [fixed, prior]) assert.ok(path.isAbsolute(binary), 'CLI paths must be absolute');
assert.match(priorCommit, /^[0-9a-f]{40}$/, 'the prior commit must be a full hash');
assert.match(output, /^\/tmp\/[^/]+$/, 'receipt must be a fresh, direct /tmp directory');
assert.ok(!fs.existsSync(output), 'receipt output must not exist');
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const git = argv => {
  const run = spawnSync('/run/current-system/sw/bin/git', argv, { cwd: root, encoding: 'utf8' });
  assert.equal(run.status, 0, `git ${argv.join(' ')}: ${run.stderr}`);
  return run.stdout.trim();
};
const source = { commit: git(['rev-parse', 'HEAD']), tree: git(['rev-parse', 'HEAD^{tree}']),
  clean: git(['status', '--porcelain', '--untracked-files=all']) === '' };
fs.mkdirSync(output, { mode: 0o700 });
const save = (name, content) => fs.writeFileSync(path.join(output, name),
  typeof content === 'string' ? content : JSON.stringify(content, null, 2) + '\n',
  { flag: 'wx', mode: 0o600 });
const bytesOf = relative => ({ path: relative, sha256: sha(fs.readFileSync(path.join(root, relative))) });
const lane = bytesOf('crates/noble-cli/tests/live_slot_package_retention.mjs');
const bound = [lane, bytesOf('crates/noble-cli/tests/live_slot_direct_quote.rs'),
  bytesOf('crates/noble-cli/tests/live_slot_resource_ref_cli.mjs'),
  bytesOf('crates/noble-cli/src/core/runtime/slot-engine.mjs'),
  bytesOf('crates/noble-cli/src/core/runtime/slot-registry.mjs')];

const runLane = binary => {
  const before = sha(fs.readFileSync(binary));
  const run = spawnSync(process.execPath, [path.join(root, lane.path), binary],
    { encoding: 'utf8', env: { PATH: process.env.PATH, HOME: os.userInfo().homedir } });
  assert.equal(sha(fs.readFileSync(binary)), before, 'a CLI changed while the lane ran');
  const lines = (run.stdout ?? '').trim().split('\n').filter(line => line.startsWith('{'))
    .map(line => JSON.parse(line));
  const observed = (variant, label) =>
    lines.find(line => line.variant === variant && line.observation === label);
  return { status: run.status, sha256: before, stdout: run.stdout ?? '', lines, observed };
};
const fixedRun = runLane(fixed);
save('fixed.stdout', fixedRun.stdout);
assert.equal(fixedRun.status, 0, `fixed CLI failed the lane: ${fixedRun.stdout.slice(-800)}`);
assert.deepEqual(fixedRun.observed('replacement', 'run-after-replacement').stack,
  [{ kind: 1, value: '21' }]);
assert.equal(fixedRun.observed('charge', 'publication-while-owned').outcome,
  'retention-budget-refused');
const summary = fixedRun.lines.find(line => line.case === 'LREF-CASE-03');
assert.ok(summary?.replacement?.retired_span && summary?.charge?.retired_span,
  'the fixed run must retire M1 in both sessions once the saved owner is released');
const priorRun = runLane(prior);
save('prior.stdout', priorRun.stdout);
assert.notEqual(priorRun.status, 0, 'the prior CLI must fail the lane');
assert.deepEqual(priorRun.observed('replacement', 'run-after-replacement')?.stack,
  [{ kind: 1, value: '21' }], 'the prior CLI must already keep M1 alive for the saved owner');
const reproduced = priorRun.observed('charge', 'publication-while-owned');
assert.equal(reproduced?.outcome, 'published',
  'the prior CLI must admit v2 while the saved owner holds v1');

const pick = run => Object.fromEntries(['replacement', 'charge'].map(variant => [variant,
  run.lines.filter(line => line.variant === variant)]));
save('receipt.json', {
  schema: 'noble-package-retention-receipt/v1', case: 'LREF-CASE-03',
  claim: 'selected host evidence for one dispatched saved result; not a proof and not a conformance promotion',
  source: { ...source, bound },
  fixed: { what: 'the lane against a CLI built from the source above', binary_sha256: fixedRun.sha256,
    status: fixedRun.status, observations: pick(fixedRun), summary },
  prior: { what: 'the same lane against a CLI built from the commit before the fix',
    declared_commit: priorCommit, binary_sha256: priorRun.sha256, status: priorRun.status,
    observations: pick(priorRun), reproduced },
  non_claims: [
    'one slot, one module, one dispatched saved static Program and one replacement; no other package member or retirement route is exercised here',
    'the package is approximated by the modules whose code the saved graph enters, as the OWN-CASE-06 retention already does; table and capture members are covered only as far as that graph walk observes them',
    'the prior binary is bound by its hash and the commit it was declared to be built from, which this script does not rebuild',
    'charging every version of an entered module makes retention admission stricter: a saved result can now refuse a publication the cell-only charge admitted',
  ],
});
process.stdout.write(`${JSON.stringify({ receipt: path.join(output, 'receipt.json'),
  fixed: fixedRun.sha256, prior: priorRun.sha256 })}\n`);
