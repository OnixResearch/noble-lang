#!/usr/bin/env node
/**
 * Source-bound receipt for LREF-CASE-04, 05 and 06 (`.cairn/changes/live-reference-lifetimes`,
 * r[LREF-ESCAPE-01], r[LREF-EV-01]): escaped results, saved generic callers and held roots
 * against the selected live-slot CLI. Each case runs its own lane:
 *   LREF-CASE-04 - crates/noble-cli/tests/live_slot_escaped_result.mjs
 *   LREF-CASE-05 - crates/noble-cli/tests/live_slot_generic_caller_roots.mjs
 *   LREF-CASE-06 - crates/noble-cli/tests/live_slot_held_root_mixing.mjs
 * The CLI must be built from the clean source tree this receipt names; the receipt binds its
 * digest and the lanes' sources, checks each case's recorded observations and writes them with
 * the case's non-claims. It is execution evidence, not a proof and not a conformance promotion.
 *
 * Usage (pinned Node, from a repository root whose ancestors no other principal can write,
 * because the lanes' authorities live under the tests directory):
 *   node verification/live-reference-cases/receipt.mjs CLI NEW_DIRECTORY
 */
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const sha = bytes => createHash('sha256').update(bytes).digest('hex');
assert.equal(process.argv.length, 4, 'usage: node receipt.mjs CLI NEW_DIRECTORY');
const [binary, output] = process.argv.slice(2);
assert.ok(path.isAbsolute(binary), 'the CLI path must be absolute');
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
const lanes = {
  'LREF-CASE-04': 'crates/noble-cli/tests/live_slot_escaped_result.mjs',
  'LREF-CASE-05': 'crates/noble-cli/tests/live_slot_generic_caller_roots.mjs',
  'LREF-CASE-06': 'crates/noble-cli/tests/live_slot_held_root_mixing.mjs',
};
const bound = [...Object.values(lanes),
  'crates/noble-cli/tests/live_slot_resource_ref_cli.mjs',
  'crates/noble-cli/tests/live_slot_direct_quote.rs',
  'crates/noble-cli/src/core/live_slot.rs',
  'crates/noble-cli/src/core/runtime/slot-engine.mjs',
  'crates/noble-cli/src/core/runtime/slot-registry.mjs',
  'verification/live-reference-cases/receipt.mjs'].map(bytesOf);
const binarySha256 = sha(fs.readFileSync(binary));

const runLane = (name, lane) => {
  const run = spawnSync(process.execPath, [path.join(root, lane), binary], { encoding: 'utf8',
    env: { PATH: process.env.PATH, HOME: os.userInfo().homedir, TMPDIR: os.tmpdir() } });
  assert.equal(sha(fs.readFileSync(binary)), binarySha256, 'the CLI changed while a lane ran');
  save(`${name}.stdout`, run.stdout ?? '');
  assert.equal(run.status, 0, `${name} lane failed: ${(run.stderr ?? '').slice(-800)}`);
  const summary = (run.stdout ?? '').trim().split('\n').filter(line => line.startsWith('{'))
    .map(line => JSON.parse(line)).find(line => line.case === name);
  assert.ok(summary, `${name} summary line`);
  assert.equal(summary.selected_binary_sha256, binarySha256);
  return summary;
};

const case04 = runLane('LREF-CASE-04', lanes['LREF-CASE-04']);
assert.deepEqual(case04.o_after_publication, [{ kind: 1, value: '21' }]);
assert.deepEqual(case04.fresh_result, [{ kind: 1, value: '22' }]);
assert.deepEqual([case04.o_epoch, case04.fresh_epoch], ['1', '2']);
const case05 = runLane('LREF-CASE-05', lanes['LREF-CASE-05']);
assert.deepEqual([case05.r1.epoch, case05.r1.stack, case05.r2.epoch, case05.r2.stack],
  ['1', [{ kind: 1, value: '21' }], '2', [{ kind: 1, value: '22' }]]);
assert.notEqual(case05.r1.selected, case05.r2.selected);
const case06 = runLane('LREF-CASE-06', lanes['LREF-CASE-06']);
assert.deepEqual([case06.r1.epoch, case06.r1.stack, case06.publication_while_held.epoch,
  case06.r2.epoch, case06.r2.stack],
['2', [{ kind: 1, value: '122' }], '3', '3', [{ kind: 1, value: '124' }]]);

save('receipt.json', {
  schema: 'noble-live-reference-cases-receipt/v1',
  claim: 'selected host execution evidence for LREF-CASE-04, 05 and 06 against one CLI built from the source below; not a proof and not a conformance promotion',
  source: { ...source, bound }, binary_sha256: binarySha256,
  cases: {
    'LREF-CASE-04': { lane: lanes['LREF-CASE-04'], summary: case04, non_claims: [
      'one slot whose versions return a Program composed from a literal; O is observed through its saved record and by running it, not through a reflection operation on saved Programs, which the live-slot protocol does not offer',
      'the recorded cell graph is the host\'s bounded observation of O; that a LiveRef cannot appear in it follows from LiveRefs being root bindings rather than cells',
    ] },
    'LREF-CASE-05': { lane: lanes['LREF-CASE-05'], summary: case05, non_claims: [
      'G is the installed generic caller module, reused across roots; a quotation of `slot.invoke` is refused (LSLOT-06), so no saved Program value can be a generic caller',
      'the design\'s variant in which R2 retains R1\'s ref has no request form: a ref exists only as the refs binding of the invocation that issued it',
    ] },
    'LREF-CASE-06': { lane: lanes['LREF-CASE-06'], summary: case06, non_claims: [
      'the serial live host accepts only operator controls during a held checkpoint, so R2 runs after R1 resumes rather than while R1 is held; the publication itself commits while R1 is held',
      'the design numbers R1\'s epoch 1; here S-v0 is published first (epoch 1), so S-v1 and R1 are at epoch 2 and S-v2 at epoch 3',
    ] },
  },
});
process.stdout.write(`${JSON.stringify({ receipt: path.join(output, 'receipt.json'), binary: binarySha256 })}\n`);
