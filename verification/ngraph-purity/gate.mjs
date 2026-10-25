// NGRAPH-CASE-02: noble-ngraph is a pure domain-core library.
// Static checks over the crate boundary, its resolved normal dependency graph and its use of capability/hashing surfaces.
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

const [root, output, cargo] = process.argv.slice(2);
if (!root || !output || !cargo) throw new Error('usage: node gate.mjs REPO_ROOT NEW_OUTPUT_DIRECTORY CARGO_BINARY');
if (!fs.existsSync(cargo)) throw new Error(`cargo binary missing: ${cargo}`);
const crate = path.join(root, 'crates/noble-ngraph');
const manifest = fs.readFileSync(path.join(crate, 'Cargo.toml'), 'utf8');
const receipt = {
  schema: 'noble-ngraph-purity-observation/v1',
  assertion: 'static boundary checks over the declared crate, not a proof of source purity and not a native-assurance or Octet receipt',
  case: 'NGRAPH-CASE-02',
  checks: [],
};

function record(name, claim, observed) {
  receipt.checks.push({ name, claim, observed });
}

// 1. The crate attribute is unconditionally no_std.
const lib = fs.readFileSync(path.join(crate, 'src/lib.rs'), 'utf8');
const noStd = lib.split('\n').some(line => line.trim() === '#![no_std]');
assert.equal(noStd, true, 'noble-ngraph must declare #![no_std] unconditionally');
record('crate-attribute-no-std', 'The crate declares `#![no_std]` unconditionally in src/lib.rs.', { line: '#![no_std]' });

// 2. Normal dependencies are exactly the two declared Noble crates, and there are no build or dev' dependencies that could inject behaviour into the library.
const sections = {};
let current;
for (const line of manifest.split('\n')) {
  const header = line.match(/^\[([^\]]+)\]$/);
  if (header) { current = header[1]; sections[current] = sections[current] ?? []; continue; }
  if (current && line.trim() && !line.trim().startsWith('#')) sections[current].push(line.trim());
}
const normal = (sections['dependencies'] ?? []).map(line => line.split('=')[0].trim()).filter(Boolean).sort();
assert.deepEqual(normal, ['noble-contracts', 'noble-kernel'], 'unexpected normal dependency set');
assert.equal(sections['build-dependencies'], undefined, 'no build dependencies allowed');
record('normal-dependencies', 'Normal dependencies are exactly noble-contracts and noble-kernel with no build dependencies.', { normal });

// 3. Resolved normal dependency graph from cargo metadata, so a renamed or transitive third-party crate cannot hide behind a feature.
const metadataRun = spawnSync(cargo, ['metadata', '--format-version', '1', '--offline', '--no-deps'], {
  cwd: root, encoding: 'utf8', timeout: 300_000, maxBuffer: 64 * 1024 * 1024,
});
assert.equal(metadataRun.status, 0, `cargo metadata failed: ${(metadataRun.error?.message ?? metadataRun.stderr ?? '').slice(0, 400)}`);
const metadata = JSON.parse(metadataRun.stdout);
const member = metadata.packages.find(p => p.name === 'noble-ngraph');
assert.ok(member, 'noble-ngraph missing from the workspace metadata');
const normalResolved = member.dependencies
  .filter(d => d.kind === null || d.kind === 'normal')
  .map(d => d.name).sort();
assert.deepEqual(normalResolved, ['noble-contracts', 'noble-kernel'], 'resolved normal dependency set differs');
record('resolved-normal-dependencies', 'Workspace metadata resolves only the two Noble crates as normal dependencies of noble-ngraph.', { normal: normalResolved });

// 4. No host-capability surface appears in the library sources.
const forbidden = ['std::fs', 'std::net', 'std::process', 'std::env', 'std::time', 'std::thread', 'std::sync',
  'tokio::', 'std::os', 'extern "C"', 'unsafe '];
const sources = [];
(function walk(directory) {
  for (const entry of fs.readdirSync(directory, { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name))) {
    const file = path.join(directory, entry.name);
    if (entry.isDirectory()) { walk(file); continue; }
    if (entry.name.endsWith('.rs')) sources.push(file);
  }
}(path.join(crate, 'src')));
assert.ok(sources.length > 0, 'no library sources found');
const hits = [];
for (const file of sources) {
  const text = fs.readFileSync(file, 'utf8');
  for (const needle of forbidden) if (text.includes(needle)) hits.push({ file: path.relative(root, file), needle });
}
assert.deepEqual(hits, [], 'forbidden capability surface found in noble-ngraph');
record('forbidden-capability-uses', 'No std filesystem/network/process/environment/time/concurrency surface, extern C, or unsafe block appears in the library sources.',
  { forbidden, sources: sources.map(f => path.relative(root, f)) });

// 5. Identity hashing is injected rather than chosen by the library.
const identity = fs.readFileSync(path.join(crate, 'src/identity.rs'), 'utf8');
assert.ok(/pub trait IdentityHasher/.test(identity), 'identity.rs must declare the injected IdentityHasher trait');
assert.ok(/dyn IdentityHasher/.test(identity), 'identity.rs must consume IdentityHasher through a trait object');
for (const std of ['DefaultHasher', 'RandomState', 'SipHasher', 'std::collections::hash_map']) {
  assert.equal(identity.includes(std), false, `identity.rs must not select ${std}`);
}
record('identity-hash-injected', 'Identity hashing is a caller-supplied trait object; the library selects no standard hasher.', { trait: 'IdentityHasher' });

fs.mkdirSync(output);
const sourceDigest = crypto.createHash('sha256')
  .update(sources.map(f => fs.readFileSync(f)).join('\0')).digest('hex');
receipt.source = { files: sources.length, digest: sourceDigest };
receipt.manifest_sha256 = crypto.createHash('sha256').update(manifest).digest('hex');
receipt.cargo = metadataRun.status === 0 ? { metadata: 'offline --no-deps', packages: metadata.packages.length } : null;
fs.writeFileSync(path.join(output, 'observation.json'), `${JSON.stringify(receipt, null, 2)}\n`);
console.log(JSON.stringify({ outcome: 'observed', case: receipt.case, checks: receipt.checks.length,
  source_files: sources.length, source_digest: sourceDigest }));
