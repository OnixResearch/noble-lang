import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';

const flags = fs.constants.O_RDONLY | fs.constants.O_NOFOLLOW | fs.constants.O_NONBLOCK;
const zeroes = Buffer.alloc(7);
const length = Buffer.alloc(8);
const maxNodes = 50_000;
const maxBytes = 2 * 1024 * 1024 * 1024;
const maxFileBytes = 256 * 1024 * 1024;
const maxNarBytes = 3 * 1024 * 1024 * 1024;
const maxDepth = 64;

export const reviewedLockNames = Object.freeze([
  'Cargo.lock',
  'verification/m5/peer/Cargo.lock',
  'verification/m6/peer/Cargo.lock',
  'verification/m7/peer/Cargo.lock',
  'verification/intrinsic-proofs/peer/Cargo.lock',
  'verification/intrinsic-named-v2/peer/Cargo.lock',
  'verification/result-library/peer/Cargo.lock',
  'verification/declared-modules-v1/peer/Cargo.lock',
  'verification/worker-send/peer/Cargo.lock',
]);

function registryRows(source, lock) {
  const entries = new Map();
  for (const block of source.split(/^\[\[package\]\]\s*$/mu).slice(1)) {
    const field = key => block.match(new RegExp(`^${key} = "([^"\\n]+)"$`, 'mu'))?.[1];
    if (field('source') !== 'registry+https://github.com/rust-lang/crates.io-index')
      continue;
    const name = field('name');
    const version = field('version');
    const checksum = field('checksum');
    assert.match(name ?? '', /^[a-zA-Z0-9_-]+$/u, `${lock}: invalid registry name`);
    assert.match(version ?? '', /^[0-9A-Za-z.+-]+$/u, `${lock}: invalid registry version`);
    assert.match(checksum ?? '', /^[0-9a-f]{64}$/u, `${lock}: invalid registry checksum`);
    const id = `${name}-${version}`;
    assert.ok(!entries.has(id), `${lock}: duplicate registry package ${id}`);
    entries.set(id, { name, version, checksum });
  }
  return entries;
}

// The NAR authenticates bytes, not correspondence with the selected nine
// source locks. Validate the complete package/checksum/membership closure
// before starting offline Cargo or accepting its production build receipt.
export function reviewedVendorRecipe(root, vendor) {
  const manifestPath = path.join(path.dirname(vendor.directory), 'vendor-provenance.json');
  assert.ok(fs.lstatSync(manifestPath).isFile(), 'selected vendor manifest is not regular');
  const bytes = fs.readFileSync(manifestPath);
  const manifest = JSON.parse(bytes);
  assert.equal(manifest.schema, 'noble-reviewed-offline-vendor/v1');
  assert.equal(Object.keys(manifest.locks).sort().join('\0'),
    [...reviewedLockNames].sort().join('\0'),
    'selected vendor omitted or added a reviewed Cargo lock');
  const expected = new Map();
  let registryCount = 0;
  const locks = {};
  for (const name of reviewedLockNames) {
    const source = fs.readFileSync(path.join(root, name));
    const digest = createHash('sha256').update(source).digest('hex');
    assert.equal(manifest.locks[name], digest,
      `${name}: selected vendor provenance differs from current Cargo lock`);
    locks[name] = { source_sha256: digest, manifest_sha256: digest, exact: true };
    for (const [id, packageRow] of registryRows(source.toString('utf8'), name)) {
      registryCount++;
      const previous = expected.get(id);
      if (previous) {
        assert.equal(previous.checksum, packageRow.checksum,
          `${id}: conflicting reviewed registry checksums`);
        previous.locks.push(name);
      } else {
        expected.set(id, { ...packageRow, locks: [name] });
      }
    }
  }
  assert.equal(registryCount, 615, 'reviewed vendor needs exactly 615 registry rows');
  assert.equal(expected.size, 241, 'reviewed vendor needs exactly 241 unique registry packages');
  assert.equal(manifest.packages.length, expected.size,
    'selected vendor manifest package count differs from lock closure');
  const seen = new Set();
  for (const row of manifest.packages) {
    const id = `${row.name}-${row.version}`;
    assert.ok(expected.has(id) && !seen.has(id), `${id}: extra or duplicated vendor package`);
    seen.add(id);
    const packageRow = expected.get(id);
    assert.equal(row.checksum, packageRow.checksum,
      `${id}: selected vendor package checksum differs from lock closure`);
    assert.deepEqual(row.locks, packageRow.locks,
      `${id}: selected vendor lock membership differs from lock closure`);
    assert.match(row.archive, /^\/nix\/store\/[0-9a-z]{32}-[^\s/]+\.crate$/u,
      `${id}: selected vendor archive is not an exact Nix store path`);
    assert.ok(row.archive.endsWith(`-${id}.crate`),
      `${id}: selected vendor archive does not name the reviewed package`);
  }
  return { manifest_path: manifestPath,
    manifest_sha256: createHash('sha256').update(bytes).digest('hex'),
    locks, registry_rows: registryCount, registry_unique: expected.size,
    exact_nine_lock_recipe: true };
}

function directoryNames(name, remaining) {
  const directory = fs.opendirSync(name, { encoding: 'buffer', bufferSize: 32 });
  const names = [];
  try {
    for (let entry; (entry = directory.readSync()) !== null;) {
      assert.ok(names.length < remaining, 'selected vendor NAR node limit exceeded');
      const bytes = entry.name;
      assert.ok(Buffer.isBuffer(bytes) && bytes.length && !bytes.includes(0) &&
        !bytes.includes(47), 'invalid selected vendor NAR entry');
      names.push(bytes);
    }
  } finally {
    directory.closeSync();
  }
  return names.sort(Buffer.compare);
}

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
  let narBytes = 0;
  const raw = value => {
    narBytes += value.length;
    assert.ok(narBytes <= maxNarBytes, 'selected vendor NAR encoded byte limit exceeded');
    hash.update(value);
  };
  const word = value => {
    const data = Buffer.isBuffer(value) ? value : Buffer.from(value);
    length.writeBigUInt64LE(BigInt(data.length));
    raw(length);
    raw(data);
    raw(zeroes.subarray(0, (-data.length) & 7));
  };
  const walk = (name, depth) => {
    assert.ok(depth < maxDepth, 'selected vendor NAR depth limit exceeded');
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
        const names = directoryNames(name, maxNodes - nodes);
        for (const entry of names) {
          word('entry');
          word('(');
          word('name');
          word(entry);
          word('node');
          walk(Buffer.concat([Buffer.isBuffer(name) ? name : Buffer.from(name),
            Buffer.from('/'), entry]), depth + 1);
          word(')');
        }
        assert.deepEqual(directoryNames(name, maxNodes - nodes + names.length),
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
        assert.ok(before.size <= BigInt(maxFileBytes), 'selected vendor NAR file byte limit exceeded');
        bytes += Number(before.size);
        assert.ok(bytes <= maxBytes, 'selected vendor NAR byte limit exceeded');
        word('regular');
        if (before.mode & 0o100n) {
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
  walk(root, 0);
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
