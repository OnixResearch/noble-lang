import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
export const cases = JSON.parse(fs.readFileSync(path.resolve(here, '../../../specs/conformance/live-reference-cases.json'), 'utf8')).cases;
export const canonical = id => {
  const selected = cases.find(row => row.id === id);
  assert.ok(selected, `canonical case ${id} missing`);
  return selected;
};
export const digest = source => createHash('sha256').update(source).digest('hex');
const selectedEvidenceDirectory = process.env.NOBLE_SLOT_EVIDENCE_DIR;
if (selectedEvidenceDirectory) {
  assert.match(selectedEvidenceDirectory, /^\/tmp\/noble-lslot-resource-[^/]+$/,
    'preserved evidence must be a unique precreated /tmp/noble-lslot-resource-* directory');
  const metadata = fs.lstatSync(selectedEvidenceDirectory);
  assert.ok(metadata.isDirectory() && !metadata.isSymbolicLink(),
    'preserved evidence must be a real directory');
  assert.equal(metadata.uid, process.getuid(), 'preserved evidence must belong to the current user');
  assert.equal(metadata.mode & 0o077, 0, 'preserved evidence must not be accessible by other users');
  assert.deepEqual(fs.readdirSync(selectedEvidenceDirectory), [],
    'preserved evidence directory must be new and empty for this script');
}
export const evidenceDirectory = selectedEvidenceDirectory
  ?? fs.mkdtempSync('/tmp/noble-lslot-resource-');
if (!selectedEvidenceDirectory) {
  process.once('exit', () => fs.rmSync(evidenceDirectory, { recursive: true }));
}

// The CLI rejects an authority below /tmp: all ancestors of its pinned path
// must be non-group/world-writable. Only this unique, private directory under
// the checked-out tests directory holds the temporary selected authority.
export async function cli(binary, variant, authority) {
  assert.ok(path.isAbsolute(binary), 'select a real absolute noble CLI binary');
  const selectedBinarySha256 = digest(fs.readFileSync(binary));
  const fixture = fs.mkdtempSync(path.join(here, 'live_slot_resource_ref_authority-'));
  const authorityPath = path.join(fixture, 'selected.json');
  fs.writeFileSync(authorityPath, JSON.stringify(authority), { mode: 0o600 });
  const log = path.join(evidenceDirectory, `${variant}.jsonl`);
  fs.writeFileSync(log, JSON.stringify({
    kind: 'authority', path: authorityPath, authority,
    selected_binary: binary, selected_binary_sha256: selectedBinarySha256,
  }) + '\n', { mode: 0o600 });
  const child = spawn('/bin/bash', ['-c', 'cat | exec "$1" live slot --engine v8 --authority "$2"',
    'sh', binary, authorityPath], { stdio: ['pipe', 'pipe', 'pipe'] });
  let buffer = '';
  let stderr = '';
  const queued = [];
  let awaiting;
  const append = row => fs.appendFileSync(log, JSON.stringify(row) + '\n');
  child.stderr.on('data', bytes => { stderr += bytes.toString(); append({ kind: 'stderr', text: bytes.toString() }); });
  child.stdout.on('data', bytes => {
    buffer += bytes.toString();
    for (let end; (end = buffer.indexOf('\n')) >= 0;) {
      const line = buffer.slice(0, end);
      buffer = buffer.slice(end + 1);
      try {
        const row = JSON.parse(line);
        append({ kind: 'cli', row });
        if (awaiting) { const resolve = awaiting; awaiting = undefined; resolve(row); }
        else queued.push(row);
      } catch (error) {
        append({ kind: 'malformed-cli', line, error: String(error) });
        if (awaiting) { const resolve = awaiting; awaiting = undefined; resolve({ outcome: 'malformed-cli', line }); }
      }
    }
  });
  const next = () => queued.length ? Promise.resolve(queued.shift()) : new Promise((resolve, reject) => {
    const timeout = setTimeout(() => { awaiting = undefined; reject(Error(`CLI receipt timeout for ${variant}: ${stderr}`)); }, 30000);
    awaiting = row => { clearTimeout(timeout); resolve(row); };
  });
  const issue = async request => {
    append({ kind: 'operator', request });
    child.stdin.write(JSON.stringify(request) + '\n');
    return next();
  };
  const close = async () => {
    child.stdin.end();
    const code = child.exitCode === null ? await new Promise(resolve => child.once('exit', resolve)) : child.exitCode;
    append({ kind: 'exit', code, stderr });
    fs.rmSync(fixture, { recursive: true });
    assert.equal(digest(fs.readFileSync(binary)), selectedBinarySha256,
      'selected CLI binary changed while collecting case receipts');
    return code;
  };
  const abort = async () => {
    child.stdin.end();
    child.kill();
    if (child.exitCode === null) await new Promise(resolve => child.once('exit', resolve));
    append({ kind: 'aborted', stderr });
    fs.rmSync(fixture, { recursive: true });
  };
  return { configured: next(), issue, close, abort, log, selectedBinarySha256 };
}

export function selectedAuthority({ sources, resources = [], slots, grants = [], effects = ['live.dispatch'], quota = 24 }) {
  return {
    owner: 'resource-ref-fixture', quota, effects,
    grants, resources, slots,
    sources: Object.entries(sources).map(([id, source]) => ({ id, sha256: digest(source) })),
  };
}
export const resourceRows = [
  { name: 'Account@1', module: '110', ordinal: 0, kind: 41, owner: 'resource-ref-fixture' },
  { name: 'Account@2', module: '111', ordinal: 0, kind: 42, owner: 'resource-ref-fixture' },
];
export const resourceGrants = resourceRows.flatMap(row => [
  { operation: 'register-resource', nominalId: `${row.module}:${row.ordinal}`, allowed: true },
  { operation: 'issue-resource', nominalId: `${row.module}:${row.ordinal}`, allowed: true },
]);
export function expectOutcome(row, outcome) {
  assert.equal(row.outcome, outcome, JSON.stringify(row));
  return row;
}
