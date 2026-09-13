#!/usr/bin/env bun
// Run Cairn and its policy from one immutable published source. No ambient checkout.
import { spawnSync } from 'node:child_process';
import path from 'node:path';
import assert from 'node:assert/strict';

export const CAIRN_REVISION = '15f00875562025e7ea7e0d1f4af24d1a2e2ac06f';
export const CAIRN_FLAKE = `git+ssh://git@github.com/OnixResearch/cairn?rev=${CAIRN_REVISION}`;

export function invocation(metadata, args) {
  if (metadata.locked?.rev !== CAIRN_REVISION || !path.isAbsolute(metadata.path ?? '')) {
    throw new Error('Nix metadata does not match the pinned Cairn revision');
  }
  if (!args.length || args.includes('--policy')) throw new Error('supply a Cairn command without a policy override');
  return ['run', `${CAIRN_FLAKE}#cairn`, '--', ...args,
    '--policy', path.join(metadata.path, 'cairn-policy/generated/cairn-policy.json')];
}

function main() {
  const args = process.argv.slice(2);
  if (args.length === 1 && args[0] === '--self-test') {
    const metadata = { locked: { rev: CAIRN_REVISION }, path: '/nix/store/fixture-source' };
    assert.deepEqual(invocation(metadata, ['validate', '--root', '.']), ['run', `${CAIRN_FLAKE}#cairn`, '--',
      'validate', '--root', '.', '--policy', '/nix/store/fixture-source/cairn-policy/generated/cairn-policy.json']);
    assert.throws(() => invocation({ ...metadata, locked: { rev: 'wrong' } }, ['validate']), /pinned/);
    assert.throws(() => invocation({ ...metadata, path: 'relative' }, ['validate']), /pinned/);
    assert.throws(() => invocation(metadata, []), /command/);
    assert.throws(() => invocation(metadata, ['validate', '--policy', 'other.json']), /override/);
    console.log('Cairn runner: 5 self-tests passed');
    return;
  }
  if (!args.length || args.includes('--policy')) throw new Error('supply a Cairn command without a policy override');
  const metadata = spawnSync('nix', ['flake', 'metadata', '--json', CAIRN_FLAKE], {
    encoding: 'utf8', stdio: ['ignore', 'pipe', 'inherit'], maxBuffer: 4 * 1024 * 1024,
  });
  if (metadata.error) throw metadata.error;
  if (metadata.status !== 0) throw new Error(`Nix metadata failed: ${metadata.status ?? metadata.signal}`);
  const child = spawnSync('nix', invocation(JSON.parse(metadata.stdout), args), { stdio: 'inherit' });
  if (child.error) throw child.error;
  process.exitCode = child.status ?? 1;
}

if (import.meta.main) {
  try { main(); } catch (error) { console.error(`Cairn runner failed: ${error.message}`); process.exitCode = 1; }
}
