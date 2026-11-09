#!/usr/bin/env node
/**
 * Source-bound receipt for OWN-CASE-01, 02 and 03 (`.cairn/changes/ownership-lifetime-contract`)
 * against the live-slot runtime.
 *
 * It runs two lanes against one CLI built from the clean source tree it names:
 *   - `crates/noble-cli/tests/live_slot_direct_quote.mjs`, whose direct quote is OWN-CASE-01;
 *   - `crates/noble-cli/tests/live_slot_ownership_cases.mjs`, for OWN-CASE-02 and 03.
 * Both must pass. No case is registered or promoted here.
 *
 * Usage (pinned Node, env -i, from a repository root whose ancestors no other principal can
 * write, because the lanes' authorities live under the tests directory):
 *   node verification/ownership-cases/receipt.mjs CLI NEW_TMP_DIRECTORY
 */
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const sha = bytes => createHash('sha256').update(bytes).digest('hex');
assert.equal(process.argv.length, 4, 'usage: node receipt.mjs CLI NEW_TMP_DIRECTORY');
const [binary, output] = process.argv.slice(2);
assert.ok(path.isAbsolute(binary), 'the CLI path must be absolute');
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
const lanes = {
  'OWN-CASE-01': bytesOf('crates/noble-cli/tests/live_slot_direct_quote.mjs'),
  'OWN-CASE-02/03': bytesOf('crates/noble-cli/tests/live_slot_ownership_cases.mjs'),
};
const bound = [...Object.values(lanes), bytesOf('crates/noble-cli/tests/live_slot_direct_quote.rs'),
  bytesOf('crates/noble-cli/tests/live_slot_resource_ref_cli.mjs'),
  bytesOf('crates/noble-cli/src/core/runtime/slot-engine.mjs'),
  bytesOf('crates/noble-cli/src/core/runtime/slot-registry.mjs')];

const cliSha256 = sha(fs.readFileSync(binary));
const runs = Object.fromEntries(Object.entries(lanes).map(([label, lane], at) => {
  const run = spawnSync(process.execPath, [path.join(root, lane.path), binary],
    { encoding: 'utf8', env: { PATH: process.env.PATH, HOME: os.userInfo().homedir } });
  save(`lane-${at + 1}.stdout`, run.stdout ?? '');
  assert.equal(run.status, 0, `${label} failed: ${(run.stdout ?? '').slice(-800)} ${(run.stderr ?? '').slice(-800)}`);
  return [label, { lane: lane.path, status: run.status, stdout: (run.stdout ?? '').trim() }];
}));
assert.equal(sha(fs.readFileSync(binary)), cliSha256, 'the CLI changed while the lanes ran');
assert.match(runs['OWN-CASE-01'].stdout, /ran with 20 to yield 25/);
const { cases } = JSON.parse(runs['OWN-CASE-02/03'].stdout.split('\n').at(-1));

save('receipt.json', {
  schema: 'noble-ownership-cases-receipt/v1', cases: ['OWN-CASE-01', 'OWN-CASE-02', 'OWN-CASE-03'],
  claim: 'selected host evidence for three ownership cases on one CLI; not a proof, not a registration and not a promotion',
  source: { ...source, bound },
  cli_sha256: cliSha256,
  'OWN-CASE-01': { lane: runs['OWN-CASE-01'].lane, observed: runs['OWN-CASE-01'].stdout },
  'OWN-CASE-02': cases['OWN-CASE-02'],
  'OWN-CASE-03': cases['OWN-CASE-03'],
  non_claims: [
    'OWN-CASE-03 runs a balanced 8-leaf tree of one static leaf Program; the left- and right-deep 8-leaf trees and trees of captured leaves exceed the host bound on observing an anonymous saved Program, which refuses the save and terminates the session',
    'storage reclamation is observed only through owners being refused after release and the root running alike; no live-cell count is exposed',
    'OWN-CASE-04, 05 and 07 are not executed here, and no OWN-CASE design is registered in conformance',
  ],
});
process.stdout.write(`${JSON.stringify({ receipt: path.join(output, 'receipt.json'), cli: cliSha256 })}\n`);
