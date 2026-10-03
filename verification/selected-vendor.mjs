import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

export function selectedVendor(pins) {
  const directory = path.join(pins.vendor, 'source-registry-0');
  assert.ok(fs.statSync(directory).isDirectory(), 'missing selected offline Rust vendor');
  const result = spawnSync('/run/current-system/sw/bin/nix',
    ['hash', 'path', '--type', 'sha256', '--sri', pins.vendor],
    { encoding: 'utf8', timeout: 180_000, maxBuffer: 1024 * 1024 });
  assert.equal(result.status, 0,
    `cannot hash selected offline vendor: ${result.error ?? result.stderr ?? result.signal}`);
  const narHash = result.stdout.trim();
  assert.equal(narHash, pins.vendor_nar_hash, 'selected offline vendor differs from reviewed NAR');
  return { directory, narHash };
}
