// Bind a measurement receipt to the harness bytes that actually ran.
//
// `validation-cost.mjs` and `batch-breakdown.mjs` run the harness from a
// `git archive` of a commit, so a receipt that hashed the working-tree file
// would describe a harness that never ran (and would disagree with itself
// under --allow-dirty). The digest is taken from the committed blob, and the
// caller records the same `commit` as the receipt's source.
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';

const sha256 = bytes => createHash('sha256').update(bytes).digest('hex');

export function harnessBinding(root, commit, file, git) {
  const result = spawnSync(git, ['show', `${commit}:${file}`], {
    cwd: root, encoding: null, maxBuffer: 64 * 1024 * 1024,
    env: { PATH: git.slice(0, git.lastIndexOf('/')), HOME: '/nonexistent', GIT_CONFIG_NOSYSTEM: '1', GIT_CONFIG_GLOBAL: '/dev/null' },
  });
  if (result.status !== 0) throw new Error(`git show ${commit}:${file} failed: ${String(result.stderr)}`);
  return { path: file, sha256: sha256(result.stdout) };
}
