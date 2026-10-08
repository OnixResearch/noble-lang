import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';

const flags = fs.constants.O_RDONLY | fs.constants.O_NOFOLLOW | fs.constants.O_NONBLOCK;
const zeroes = Buffer.alloc(7);
const length = Buffer.alloc(8);
const maxNodes = 50_000;
const maxBytes = 2 * 1024 * 1024 * 1024;

function identity(info) {
  return [info.dev, info.ino, info.mode, info.size, info.mtimeNs, info.ctimeNs];
}

function unchanged(before, after, message) {
  assert.deepEqual(identity(after), identity(before), message);
}

function immutable(info, name) {
  assert.equal(info.uid, 0n, `non-root-owned selected vendor entry: ${name}`);
  assert.equal(info.mode & 0o222n, 0n, `writable selected vendor entry: ${name}`);
}

function storeBoundary() {
  for (const name of ['/', '/nix', '/nix/store']) {
    const info = fs.lstatSync(name, { bigint: true });
    assert.ok(info.isDirectory() && info.uid === 0n, `unsafe selected store parent: ${name}`);
    try {
      fs.accessSync(name, fs.constants.W_OK);
      assert.fail(`selected store parent writable by caller: ${name}`);
    } catch (error) {
      if (error.code !== 'EACCES' && error.code !== 'EROFS') throw error;
    }
  }
}

// NAR encodes byte strings as little-endian u64 length, data, then 8-byte padding.
// The supplied validator is invoked on each opened node; selectedVendor requires
// root ownership and no write bits, while fixtures may hash their own private trees.
export function hashNarPath(root, validate = () => {}) {
  const hash = createHash('sha256');
  const buffer = Buffer.allocUnsafe(1024 * 1024);
  let nodes = 0;
  let bytes = 0;
  const raw = value => hash.update(value);
  const word = value => {
    const data = Buffer.isBuffer(value) ? value : Buffer.from(value);
    length.writeBigUInt64LE(BigInt(data.length));
    raw(length);
    raw(data);
    raw(zeroes.subarray(0, (-data.length) & 7));
  };
  const walk = name => {
    const before = fs.lstatSync(name, { bigint: true });
    validate(before, name);
    assert.ok(++nodes <= maxNodes, 'selected vendor NAR node limit exceeded');
    word('(');
    word('type');
    if (before.isDirectory()) {
      const fd = fs.openSync(name, flags | fs.constants.O_DIRECTORY);
      try {
        unchanged(before, fs.fstatSync(fd, { bigint: true }), 'vendor directory changed at open');
        word('directory');
        const names = fs.readdirSync(name, { encoding: 'buffer' }).sort(Buffer.compare);
        for (const entry of names) {
          assert.ok(entry.length && !entry.includes(0) && !entry.includes(47),
            'invalid selected vendor NAR entry');
          word('entry');
          word('(');
          word('name');
          word(entry);
          word('node');
          walk(Buffer.concat([Buffer.isBuffer(name) ? name : Buffer.from(name),
            Buffer.from('/'), entry]));
          word(')');
        }
        assert.deepEqual(fs.readdirSync(name, { encoding: 'buffer' }).sort(Buffer.compare),
          names, 'vendor directory entries changed during hashing');
        unchanged(before, fs.fstatSync(fd, { bigint: true }),
          'vendor directory changed during hashing');
      } finally {
        fs.closeSync(fd);
      }
    } else if (before.isFile()) {
      const fd = fs.openSync(name, flags);
      try {
        unchanged(before, fs.fstatSync(fd, { bigint: true }), 'vendor file changed at open');
        assert.ok(before.size <= BigInt(Number.MAX_SAFE_INTEGER), 'vendor file is too large');
        bytes += Number(before.size);
        assert.ok(bytes <= maxBytes, 'selected vendor NAR byte limit exceeded');
        word('regular');
        if (before.mode & 0o111n) {
          word('executable');
          word('');
        }
        word('contents');
        length.writeBigUInt64LE(before.size);
        raw(length);
        let remaining = Number(before.size);
        while (remaining) {
          const size = fs.readSync(fd, buffer, 0, Math.min(buffer.length, remaining), null);
          assert.ok(size, 'vendor file truncated during hashing');
          raw(buffer.subarray(0, size));
          remaining -= size;
        }
        raw(zeroes.subarray(0, (-Number(before.size)) & 7));
        unchanged(before, fs.fstatSync(fd, { bigint: true }),
          'vendor file changed during hashing');
      } finally {
        fs.closeSync(fd);
      }
    } else if (before.isSymbolicLink()) {
      // The reviewed vendor contains no links. Refuse any new links, even if
      // their NAR targets would hash correctly, to exclude Cargo path escape.
      throw new Error(`selected vendor contains a symlink: ${name}`);
    } else {
      throw new Error(`unsupported selected vendor entry: ${name}`);
    }
    unchanged(before, fs.lstatSync(name, { bigint: true }),
      'vendor entry changed during hashing');
    word(')');
  };
  word('nix-archive-1');
  walk(root);
  return `sha256-${hash.digest('base64')}`;
}

export function selectedVendor(pins) {
  assert.match(pins.vendor, /^\/nix\/store\/[0-9a-z]{32}-noble-reviewed-offline-vendor$/u,
    'selected offline vendor must be an exact Nix store output');
  assert.match(pins.vendor_nar_hash, /^sha256-[A-Za-z0-9+/]{43}=$/u,
    'selected offline vendor needs a reviewed SHA-256 NAR SRI');
  storeBoundary();
  const directory = path.join(pins.vendor, 'source-registry-0');
  const root = fs.lstatSync(pins.vendor, { bigint: true });
  immutable(root, pins.vendor);
  assert.ok(root.isDirectory(), 'missing selected offline Rust vendor');
  const narHash = hashNarPath(pins.vendor, immutable);
  storeBoundary();
  unchanged(root, fs.lstatSync(pins.vendor, { bigint: true }),
    'selected vendor root changed during hashing');
  assert.equal(narHash, pins.vendor_nar_hash, 'selected offline vendor differs from reviewed NAR');
  assert.ok(fs.lstatSync(directory).isDirectory(), 'missing selected offline Rust vendor registry');
  return { directory, narHash };
}
