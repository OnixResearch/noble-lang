#!/usr/bin/env node
/**
 * The Owi `memory.init` decode fix, on the committed #57 artifacts (issue #57).
 *
 * Upstream decodes `memory.init` as `dataidx memidx zero-byte`, so the extra byte consumes the next
 * instruction's opcode and every module that uses `memory.init` fails with "data count section
 * required". The fix is carried in `OnixResearch/owi` branch `noble/memory-init-decode` and applied
 * here as `nix/owi-memory-init-decode.patch` over the pinned nixpkgs source, the same way the Aeneas
 * and Octet patches are carried.
 *
 * This lane runs `owi validate` from the unpatched and the patched package on three committed
 * artifacts: the 94-byte reproducer (body `fc 08 00 00 42 07`), a passive-data-only control, and
 * the real emitted 10 KB subject. Unpatched, the reproducer and the subject must exit 30 with the
 * recorded message and the control must pass; patched, all three must pass. It also records what
 * `owi sym` still needs (the `noble` host module), which is Noble-side work, not an upstream block.
 *
 * Usage (pinned Node, env -i, from a clean checkout):
 *   node verification/owi/patched.mjs --unpatched OWI --patched OWI --output NEW_FILE
 */
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const LANE = 'verification/owi/patched.mjs';
const PATCH = 'nix/owi-memory-init-decode.patch';
const RECEIPTS = 'verification/owi/receipts/2026-10-26';
const ARTIFACTS = {
  reproducer: `${RECEIPTS}/reproducer-memory-init.wasm`,
  control: `${RECEIPTS}/control-passive-data-only.wasm`,
  subject: `${RECEIPTS}/subject-module-1.wasm`,
};
const sha = bytes => createHash('sha256').update(bytes).digest('hex');

function options() {
  const argv = process.argv.slice(2);
  assert.equal(argv.length, 6, 'usage: patched.mjs --unpatched OWI --patched OWI --output NEW_FILE');
  const out = {};
  for (let at = 0; at < argv.length; at += 2) out[argv[at]] = argv[at + 1];
  for (const key of ['--unpatched', '--patched']) assert.ok(path.isAbsolute(out[key]), `${key} must be absolute`);
  assert.ok(!fs.existsSync(out['--output']), 'the output must not exist');
  assert.ok(!path.resolve(out['--output']).startsWith(ROOT + path.sep), 'the output must be outside the repository');
  return out;
}

function git(argv) {
  return spawnSync('/run/current-system/sw/bin/git', argv, { cwd: ROOT, encoding: 'utf8' }).stdout.trim();
}

/** `owi validate FILE`: exit code and the first stderr line. */
function validate(binary, relative) {
  const started = process.hrtime.bigint();
  const run = spawnSync(binary, ['validate', path.join(ROOT, relative)], { encoding: 'utf8', timeout: 120_000 });
  const first = (run.stderr ?? '').split('\n').find(line => line.trim() !== '') ?? '';
  return { exit: run.status, message: first.trim().slice(0, 160), elapsed_ns: Number(process.hrtime.bigint() - started) };
}

/** What `owi sym` reports before any host module exists; recorded, never asserted away. */
function symbolic(binary, relative) {
  const run = spawnSync(binary, ['sym', path.join(ROOT, relative), '--entry-point', 'g'], { encoding: 'utf8', timeout: 120_000 });
  const first = (run.stderr ?? '').split('\n').find(line => line.trim() !== '') ?? '';
  return { exit: run.status, message: first.trim().slice(0, 160) };
}

function main() {
  const flags = options();
  const source = { commit: git(['rev-parse', 'HEAD']), tree: git(['rev-parse', 'HEAD^{tree}']),
    clean: git(['status', '--porcelain', '--untracked-files=all']) === '' };
  const packages = {};
  for (const [label, key] of [['unpatched', '--unpatched'], ['patched', '--patched']]) {
    const real = fs.realpathSync(flags[key]);
    packages[label] = { path: flags[key], realpath: real, sha256: sha(fs.readFileSync(real)),
      name: path.basename(path.dirname(path.dirname(real))) };
  }
  const results = {};
  for (const [label, key] of [['unpatched', '--unpatched'], ['patched', '--patched']]) {
    results[label] = Object.fromEntries(Object.entries(ARTIFACTS).map(([name, relative]) =>
      [name, { artifact: relative, artifact_sha256: sha(fs.readFileSync(path.join(ROOT, relative))),
        ...validate(flags[key], relative) }]));
  }
  // The recorded #57 before state, and the after state the fix must produce.
  for (const name of ['reproducer', 'subject']) {
    assert.equal(results.unpatched[name].exit, 30, `unpatched owi still refuses ${name} with the decode error`);
    assert.match(results.unpatched[name].message, /data count section required/, `${name}: recorded message`);
  }
  assert.equal(results.unpatched.control.exit, 0, 'the control passes even unpatched');
  for (const name of Object.keys(ARTIFACTS)) {
    assert.equal(results.patched[name].exit, 0, `patched owi validates ${name}`);
    assert.equal(results.patched[name].message, '', `${name}: no diagnostic from the patched decoder`);
  }
  const symbolicResult = symbolic(flags['--patched'], ARTIFACTS.reproducer);
  assert.match(symbolicResult.message, /unknown module noble/, 'symbolic execution still needs the host module');
  const receipt = {
    schema: 'noble-owi-patch-receipt/v1',
    claim: 'the forked Owi decodes `memory.init`; the committed #57 artifacts validate instead of failing to decode',
    source: { ...source, lane_sha256: sha(fs.readFileSync(path.join(ROOT, LANE))),
      patch_sha256: sha(fs.readFileSync(path.join(ROOT, PATCH))) },
    fork: { repository: 'OnixResearch/owi', branch: 'noble/memory-init-decode',
      commit: '44eadcdc3b924515f2ea7c1cdbae3638cb4816fd',
      upstream_rev_patched: '07b96ef58021810352caceffbadf2d614785bb0d',
      upstream_head_still_buggy: '88e3713bb69f14eee96ba89928be797bb842469d' },
    packages,
    results,
    symbolic_allocation: symbolicResult,
    non_claims: [
      'validation is not execution: the fix makes the decoder read the module, and nothing here runs the symbolic lane',
      'the patched package is built from the pinned nixpkgs source with this patch, not from the fork checkout; the fork commit is the same four-line change',
      'owi remains an untrusted downstream tool: no proof authority and no source-to-Wasm claim',
    ],
  };
  fs.writeFileSync(flags['--output'], `${JSON.stringify(receipt, null, 2)}\n`, { flag: 'wx' });
  process.stdout.write(`${JSON.stringify({ output: flags['--output'], clean: source.clean,
    unpatched: Object.fromEntries(Object.entries(results.unpatched).map(([name, row]) => [name, row.exit])),
    patched: Object.fromEntries(Object.entries(results.patched).map(([name, row]) => [name, row.exit])) })}\n`);
}

main();
