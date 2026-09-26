// Compare the exact staged Git blobs and plain worktree bytes of a selected
// source projection. Nix's Git-backed flake includes staged paths but reads
// their worktree contents; the two views must be frozen together.
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

const git = '/run/current-system/sw/bin/git';
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
function command(root, args) {
  const result = spawnSync(git, ['-C', root, ...args], {
    encoding: 'utf8', maxBuffer: 32 * 1024 * 1024,
    env: { ...process.env, GIT_OPTIONAL_LOCKS: '0', GIT_CONFIG_NOSYSTEM: '1' },
  });
  assert.equal(result.error, undefined, `git ${args[0]}: launch failure`);
  assert.equal(result.signal, null, `git ${args[0]}: terminated`);
  assert.equal(result.status, 0, `git ${args[0]}: ${result.stderr}`);
  return result.stdout;
}
const split = output => output ? output.split('\0').slice(0, -1) : [];

export function gitSourceProjection(root, sourceFiles, requireFrozen = true) {
  const names = [...new Set(sourceFiles)].sort();
  assert.ok(names.length, 'source projection cannot be empty');
  for (const name of names) {
    assert.ok(name && !name.startsWith('/') && !name.startsWith('../')
      && !name.includes('/../') && !name.includes('\0') && !name.includes('\n'),
    `invalid projected source path: ${name}`);
  }
  const index = new Map();
  for (const entry of split(command(root, ['ls-files', '--stage', '-z', '--', ...names]))) {
    const match = /^(\d+) ([0-9a-f]+) ([0-3])\t(.+)$/s.exec(entry);
    assert.ok(match, `malformed Git index entry: ${entry}`);
    const [, mode, oid, stage, name] = match;
    assert.equal(stage, '0', `unresolved source merge: ${name}`);
    assert.ok(!index.has(name), `duplicate source index entry: ${name}`);
    index.set(name, { mode, oid });
  }
  const unstaged = split(command(root, ['diff', '--no-ext-diff', '--name-only', '-z', '--', ...names]));
  const untracked = split(command(root, ['ls-files', '--others', '--exclude-standard', '-z', '--', ...names]));
  const status = split(command(root, ['status', '--porcelain=v1', '-z', '--untracked-files=all', '--', ...names]));
  const rows = names.map(name => {
    const file = path.join(root, name);
    const stat = fs.lstatSync(file);
    assert.ok(stat.isFile() || stat.isSymbolicLink(), `unsupported staged source: ${name}`);
    const kind = stat.isSymbolicLink() ? 'symlink' : 'file';
    const bytes = kind === 'symlink' ? Buffer.from(fs.readlinkSync(file)) : fs.readFileSync(file);
    const staged = index.get(name) ?? null;
    const worktreeMode = kind === 'symlink' ? '120000' : stat.mode & 0o111 ? '100755' : '100644';
    if (staged && requireFrozen) assert.equal(staged.mode, worktreeMode,
      `frozen staged/worktree type or executable mode differs: ${name}`);
    return { path: name, kind, index: staged, worktree_mode: worktreeMode, worktree_sha256: sha(bytes),
      worktree_bytes: bytes.length };
  });
  if (requireFrozen) {
    assert.ok(rows.every(row => row.index), `Nix source omits unstaged/untracked paths: ${rows.filter(row => !row.index).map(row => row.path)}`);
    assert.deepEqual(unstaged, [], 'frozen staged source has unstaged modifications');
    assert.deepEqual(untracked, [], 'frozen staged source has untracked files');
  }
  const projection = { schema: 'noble-m8-git-source-projection/v1', git: {
    executable: fs.realpathSync(git), sha256: sha(fs.readFileSync(fs.realpathSync(git))),
    version: command(root, ['--version']).trim(),
  }, rows, unstaged, untracked, status };
  projection.sha256 = sha(Buffer.from(JSON.stringify(projection)));
  return projection;
}
