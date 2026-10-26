// Independent expectations for the closed ServiceRef wire format (issue #48).
//
// The reference below is written from the Lean model
// `proofs/foundation/NobleFoundation/ServiceWire.lean`, not from the Rust
// decoder: prefix, quoted name, readiness suffix, the name alphabet, the
// 64-byte wire budget, the 1..=4 configured depth and the model's
// bracket-count depth scan. The Rust test is compared against this file, so a
// Rust behaviour change surfaces here rather than being re-pinned.
//
// The model's depth scan counts opening brackets, which is stricter than the
// Rust scanner's saturating decrement on closing-bracket runs. Cases whose
// decisions differ for that reason are labelled `diverges` and the Rust test
// asserts the divergence: the model-to-Rust edge stays open and reported.
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const output = path.join(root, 'verification/service-ref/cases.json');

const MAX_WIRE_BYTES = 64;
const MAX_WIRE_DEPTH = 4;
const MAX_NAME_BYTES = 32;

const nameByte = (byte) =>
  (byte >= 0x30 && byte <= 0x39) || (byte >= 0x41 && byte <= 0x5a) || (byte >= 0x61 && byte <= 0x7a)
  || byte === 0x5f || byte === 0x2d;

const validName = (name) =>
  name.length >= 1 && name.length <= MAX_NAME_BYTES && name.every(nameByte);

const encode = (name, ready) => [
  ...Buffer.from('<service "'), ...name, 0x22, 0x20, 0x23, ready ? 0x74 : 0x66, 0x3e,
];

// The model decoder. `openBrackets` counts `<` bytes, which is the model's
// precedence-preserving stand-in for the Rust scanner.
function decode(bytes, configuredDepth) {
  if (bytes.length > MAX_WIRE_BYTES) return { kind: 'byteLimit' };
  if (configuredDepth < 1 || configuredDepth > MAX_WIRE_DEPTH) return { kind: 'depthLimit' };
  if (bytes.filter((byte) => byte === 0x3c).length > configuredDepth) return { kind: 'depthLimit' };
  const prefix = Buffer.from('<service "');
  if (bytes.length < 15) return { kind: 'schema' };
  for (let index = 0; index < prefix.length; index += 1)
    if (bytes[index] !== prefix[index]) return { kind: 'schema' };
  const rest = bytes.slice(prefix.length);
  const quote = rest.indexOf(0x22);
  if (quote < 0) return { kind: 'schema' };
  const name = rest.slice(0, quote);
  const tail = rest.slice(quote + 1);
  let ready;
  if (tail.length === 4 && tail[0] === 0x20 && tail[1] === 0x23 && tail[3] === 0x3e) {
    if (tail[2] === 0x74) ready = true;
    else if (tail[2] === 0x66) ready = false;
    else return { kind: 'schema' };
  } else return { kind: 'schema' };
  // Full-input consumption and one encoding per value: the accept branch is
  // reachable only for the canonical bytes.
  if (encode(name, ready).length !== bytes.length) return { kind: 'schema' };
  if (!validName(name)) return { kind: 'schema' };
  return { kind: 'accepted', name: Buffer.from(name).toString('latin1'), ready };
}

// The Rust scanner's saturating depth, used only to label divergence cases.
function rustDepthExceeds(bytes, configuredDepth) {
  let depth = 0;
  for (const byte of bytes) {
    if (byte === 0x3c) {
      depth += 1;
      if (depth > configuredDepth) return true;
    } else if (byte === 0x3e) depth = Math.max(0, depth - 1);
  }
  return false;
}

const cases = [];
const add = (name, bytes, configuredDepth, expect, note) => {
  cases.push({ name, bytes: [...bytes], configured_depth: configuredDepth, expect, note });
};

const alphabetEdges = [
  ['digit-low', 0x30], ['digit-high', 0x39], ['upper-low', 0x41], ['upper-high', 0x5a],
  ['lower-low', 0x61], ['lower-high', 0x7a], ['underscore', 0x5f], ['hyphen', 0x2d],
];
for (const [label, byte] of alphabetEdges)
  add(`alphabet-${label}`, encode([byte], true), 4, { kind: 'accepted', name: String.fromCharCode(byte), ready: true });

for (const [label, name] of [['empty', []], ['one', [0x61]], ['edge-32', Array(32).fill(0x61)], ['edge-33', Array(33).fill(0x61)]]) {
  const bytes = encode(name, false);
  const expect = validName(name) ? { kind: 'accepted', name: Buffer.from(name).toString('latin1'), ready: false } : { kind: 'schema' };
  add(`name-${label}`, bytes, 4, expect);
}

for (const ready of [true, false]) {
  const bytes = encode([0x63, 0x6c, 0x6f, 0x63, 0x6b], ready);
  add(`round-trip-${ready ? 'ready' : 'not-ready'}`, bytes, 4, { kind: 'accepted', name: 'clock', ready });
}

for (const depth of [0, 1, 4, 5]) {
  const bytes = encode([0x63], true);
  const expect = depth === 0 || depth > MAX_WIRE_DEPTH ? { kind: 'depthLimit' } : { kind: 'accepted', name: 'c', ready: true };
  add(`configured-depth-${depth}`, bytes, depth, expect);
}

add('wire-length-65', Array(65).fill(0x78), 4, { kind: 'byteLimit' });
{
  const bytes = [...encode(Array(32).fill(0x61), true), 0x21];
  add('wire-overrun-by-substitution', bytes, 4, decode(bytes, 4));
}

// One-byte substitutions across the record: the decoder must reject each
// altered byte rather than accept a second encoding of the value.
{
  const canonical = encode([0x63, 0x6c, 0x6f, 0x63, 0x6b], true);
  for (let index = 0; index < canonical.length; index += 1) {
    for (const replacement of [0x00, 0x20, 0x22, 0x2f, 0xff]) {
      if (replacement === canonical[index]) continue;
      const mutated = [...canonical];
      mutated[index] = replacement;
      const expect = decode(mutated, 4);
      if (expect.kind === 'accepted') throw new Error(`mutation ${index}/${replacement} accepted`);
      add(`substitution-${index}-${replacement.toString(16).padStart(2, '0')}`, mutated, 4, expect);
    }
  }
}

// Appended bytes: trailing data is never ignored.
for (const trailer of [[0x00], [0x20], [0x3e], [0x00, 0x00]]) {
  const bytes = [...encode([0x61], true), ...trailer];
  add(`trailer-${trailer.length}-${trailer.join('-')}`, bytes, 4, decode(bytes, 4));
}

// Truncation at every offset: a prefix decoder must not be promoted.
{
  const canonical = encode([0x63, 0x6c, 0x6f, 0x63, 0x6b], false);
  for (let length = 0; length < canonical.length; length += 1) {
    const bytes = canonical.slice(0, length);
    const expect = decode(bytes, 4);
    if (expect.kind === 'accepted') throw new Error(`truncation ${length} accepted`);
    add(`truncation-${length}`, bytes, 4, expect);
  }
}

// Seeded deterministic generated values and bytes.
function splitMix64(state) {
  let seed = BigInt(state);
  return () => {
    seed = (seed + 0x9e3779b97f4a7c15n) & 0xffffffffffffffffn;
    let z = seed;
    z = ((z ^ (z >> 30n)) * 0xbf58476d1ce4e5b9n) & 0xffffffffffffffffn;
    z = ((z ^ (z >> 27n)) * 0x94d049bb133111ebn) & 0xffffffffffffffffn;
    return z ^ (z >> 31n);
  };
}
const random = splitMix64(0x5eed_48);
const alphabet = [...Array(26).fill(0).map((_, index) => 0x61 + index), ...Array(10).fill(0).map((_, index) => 0x30 + index), 0x5f, 0x2d];
for (let index = 0; index < 64; index += 1) {
  const length = 1 + Number(random() % 32n);
  const name = Array.from({ length }, () => alphabet[Number(random() % BigInt(alphabet.length))]);
  const ready = random() % 2n === 0n;
  add(`generated-record-${index}`, encode(name, ready), 4, { kind: 'accepted', name: Buffer.from(name).toString('latin1'), ready });
  const junk = Array.from({ length: 1 + Number(random() % 70n) }, () => Number(random() % 256n));
  add(`generated-junk-${index}`, junk, 4, decode(junk, 4));
}

// Divergence cases: opening-bracket runs are counted by the model and scanned
// with a saturating decrement by Rust, so some inputs decide differently. The
// Rust test asserts the divergence instead of hiding it.
{
  const bytes = [0x3c, 0x3c, 0x3c, 0x3c, 0x3e, 0x3e, 0x3e, 0x3e, 0x3c, 0x3c, 0x3c, 0x3c];
  const expect = decode(bytes, 4);
  if (!(expect.kind === 'depthLimit' && !rustDepthExceeds(bytes, 4)))
    throw new Error('the documented divergence case no longer diverges');
  add('divergence-bracket-runs', bytes, 4, expect,
    'the model counts eight opening brackets and rejects by depth; the Rust scanner returns to zero between the runs and never exceeds four');
}

const coverage = {
  total: cases.length,
  accepted: cases.filter((entry) => entry.expect.kind === 'accepted').length,
  byte_limit: cases.filter((entry) => entry.expect.kind === 'byteLimit').length,
  depth_limit: cases.filter((entry) => entry.expect.kind === 'depthLimit').length,
  schema: cases.filter((entry) => entry.expect.kind === 'schema').length,
};
const family = (entry) => entry.name.replace(/-\d+$/u, '');
const families = [...new Set(cases.map((entry) => entry.name.split('-')[0]))].sort();
const payload = {
  schema: 'noble-service-ref-cases/v1',
  reference: 'verification/service-ref/cases.mjs: an independent decoder written from the Lean model, not from the Rust codec',
  limits: { wire_bytes: MAX_WIRE_BYTES, configured_depth: MAX_WIRE_DEPTH, name_bytes: MAX_NAME_BYTES },
  seed: '0x5eed48',
  coverage: { ...coverage, families },
  cases,
};
const bytes = `${JSON.stringify(payload, null, 2)}
`;
fs.writeFileSync(output, bytes);
// The Rust test reads a line-oriented file: one case per line, `name depth
// kind name-hex ready bytes-hex`, with `-` for an absent field. It keeps the
// differential free of a JSON dependency in the kernel crate.
const lines = cases.map((entry) => {
  const name = entry.expect.name === undefined ? '-' : Buffer.from(entry.expect.name, 'latin1').toString('hex');
  const ready = entry.expect.ready === undefined ? '-' : entry.expect.ready ? '1' : '0';
  const hex = entry.bytes.map((byte) => byte.toString(16).padStart(2, '0')).join('');
  return [entry.name, entry.configured_depth, entry.expect.kind, name, ready, hex].join(' ');
});
const textBytes = `${lines.join('\n')}
`;
const textPath = `${output.replace(/\.json$/, '.txt')}`;
fs.writeFileSync(textPath, textBytes);
const digest = crypto.createHash('sha256').update(textBytes).digest('hex');
console.log(JSON.stringify({ output, textPath, cases: cases.length, coverage, digest, family_sample: families.slice(0, 6) }));
