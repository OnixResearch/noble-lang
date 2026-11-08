#!/usr/bin/env node
/**
 * Source-bound receipt for OWN-CASE-06 (`.cairn/changes/ownership-lifetime-contract`): a saved
 * Program whose compiled body reads a named definition through its module's global, invoked
 * after that module's published root is superseded.
 *
 * It runs `crates/noble-cli/tests/live_slot_module_root_retention.mjs` twice:
 *   fixed - a CLI built from the clean source tree this receipt names; the lane must pass;
 *   prior - a CLI built from the commit before the fix; the lane must fail at the saved owner's
 *           invocation after supersession, which is the reproduced defect.
 *
 * Usage (pinned Node, env -i, from a repository root whose ancestors no other principal can
 * write, because the lane's authority lives under the tests directory):
 *   node verification/module-root-retention/receipt.mjs FIXED_CLI PRIOR_CLI PRIOR_COMMIT NEW_TMP_DIRECTORY
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
const lane = bytesOf('crates/noble-cli/tests/live_slot_module_root_retention.mjs');
const bound = [lane, bytesOf('crates/noble-cli/tests/live_slot_direct_quote.rs'),
  bytesOf('crates/noble-cli/tests/live_slot_resource_ref_cli.mjs'),
  bytesOf('crates/noble-cli/src/core/runtime/slot-engine.mjs')];

const runLane = binary => {
  const before = sha(fs.readFileSync(binary));
  const run = spawnSync(process.execPath, [path.join(root, lane.path), binary],
    { encoding: 'utf8', env: { PATH: process.env.PATH, HOME: os.userInfo().homedir } });
  assert.equal(sha(fs.readFileSync(binary)), before, 'a CLI changed while the lane ran');
  const lines = (run.stdout ?? '').trim().split('\n').filter(line => line.startsWith('{'))
    .map(line => JSON.parse(line));
  return { status: run.status, sha256: before, stdout: run.stdout ?? '', lines };
};
const fixedRun = runLane(fixed);
save('fixed.stdout', fixedRun.stdout);
assert.equal(fixedRun.status, 0, `fixed CLI failed the lane: ${fixedRun.stdout.slice(-800)}`);
const observed = label => fixedRun.lines.find(line => line.observation === label);
assert.deepEqual(observed('after-supersession').stack, [{ kind: 1, value: '42' }]);
const summary = fixedRun.lines.find(line => line.case === 'OWN-CASE-06');
assert.ok(summary?.retired_span, 'the fixed run must retire M after the saved owner is released');
const priorRun = runLane(prior);
save('prior.stdout', priorRun.stdout);
assert.notEqual(priorRun.status, 0, 'the prior CLI must fail the lane');
const reproduced = priorRun.lines.find(line => line.observation === 'after-supersession');
assert.equal(reproduced?.outcome, 'execution-failed', 'the prior CLI must fail at the saved owner');

save('receipt.json', {
  schema: 'noble-module-root-retention-receipt/v1', case: 'OWN-CASE-06',
  claim: 'selected host evidence for one saved named caller; not a proof and not a conformance promotion',
  source: { ...source, bound },
  fixed: { what: 'the lane against a CLI built from the source above', binary_sha256: fixedRun.sha256,
    status: fixedRun.status, after_supersession: observed('after-supersession'),
    after_release: observed('after-release'), summary },
  prior: { what: 'the same lane against a CLI built from the commit before the fix',
    declared_commit: priorCommit, binary_sha256: priorRun.sha256, status: priorRun.status,
    after_supersession: reproduced },
  non_claims: [
    'one module, one saved static Program and one supersession; no other retirement route is exercised here',
    'the prior binary is bound by its hash and the commit it was declared to be built from, which this script does not rebuild',
    'module roots and code now outlive every saved owner whose graph enters the module, which increases retention until that owner is released',
  ],
});
process.stdout.write(`${JSON.stringify({ receipt: path.join(output, 'receipt.json'),
  fixed: fixedRun.sha256, prior: priorRun.sha256 })}\n`);
