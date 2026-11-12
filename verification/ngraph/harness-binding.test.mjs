// The receipt's harness digest binds the bytes that ran: the lanes measure a
// `git archive` of a commit, so the digest comes from the committed blob even
// when the working tree (and --allow-dirty) differs.
import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdirSync, mkdtempSync, realpathSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { harnessBinding } from './harness-binding.mjs';

// The documents check supplies its pinned git through NOBLE_GIT; the host lane
// default is the system profile's git.
const git = realpathSync(process.env.NOBLE_GIT ?? '/run/current-system/sw/bin/git');
const env = {
  PATH: dirname(git), HOME: '/nonexistent', GIT_CONFIG_NOSYSTEM: '1', GIT_CONFIG_GLOBAL: '/dev/null',
  GIT_AUTHOR_NAME: 'noble-test', GIT_AUTHOR_EMAIL: 'test@example.invalid',
  GIT_COMMITTER_NAME: 'noble-test', GIT_COMMITTER_EMAIL: 'test@example.invalid',
};
const sha256 = bytes => createHash('sha256').update(bytes).digest('hex');
const file = 'crates/placeholder/tests/measure.rs';
const committed = 'fn measured() {}\n';
const dirty = 'fn measured() { /* a working-tree edit that never ran */ }\n';
const uncertain = 'fn only_in_the_worktree() {}\n';

function scratch() {
  const root = mkdtempSync(join(tmpdir(), 'noble-harness-'));
  mkdirSync(join(root, dirname(file)), { recursive: true });
  writeFileSync(join(root, file), committed);
  for (const argv of [['init', '-q', '-b', 'main'], ['add', file],
    ['-c', 'commit.gpgsign=false', 'commit', '-q', '-m', 'harness']]) {
    const observed = spawnSync(git, argv, { cwd: root, env, encoding: 'utf8' });
    assert.equal(observed.status, 0, observed.stderr);
  }
  return root;
}

test('the digest is the committed harness blob, and a dirty copy is not bound', () => {
  const root = scratch();
  assert.deepEqual(harnessBinding(root, 'HEAD', file, git), { path: file, sha256: sha256(committed) });
  writeFileSync(join(root, file), dirty);
  assert.deepEqual(harnessBinding(root, 'HEAD', file, git), { path: file, sha256: sha256(committed) });
  assert.notEqual(sha256(committed), sha256(dirty), 'the fixture copies must differ');
});

test('a file absent from the commit refuses instead of hashing the working tree', () => {
  const root = scratch();
  writeFileSync(join(root, 'untracked.rs'), uncertain);
  assert.throws(() => harnessBinding(root, 'HEAD', 'untracked.rs', git), /git show HEAD:untracked\.rs failed/);
});
