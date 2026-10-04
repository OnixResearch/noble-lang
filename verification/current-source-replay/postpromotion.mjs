import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { restoreReplayEvidence } from './projection.mjs';

const sha = value => crypto.createHash('sha256').update(value).digest('hex');
const receiptPath = 'verification/current-source-replay/acceptance.json';
const bytes = fs.readFileSync(receiptPath);
const receipt = JSON.parse(bytes);
assert.equal(receipt.schema,'noble-current-source-replay/v1');
assert.equal(receipt.result,'passed');
assert.equal(receipt.kind,'test');
assert.deepEqual(receipt.cases.map(row => row.id),
  ['DX-01','ADAPT-01','S-CASE-01','S-CASE-03','S-CASE-05','S-CASE-14']);
assert.equal(receipt.source_revision,
  `sha256:${sha(JSON.stringify(Object.entries(receipt.source_sha256).sort()))}`);
const projected = new Set(Object.keys(receipt.prepromotion_case_sha256));
assert.deepEqual([...projected].sort(),[
  'specs/conformance/adaptation-cases.json',
  'specs/conformance/developer-experience-cases.json',
  'specs/conformance/safety-cases.json',
]);
let external;
for (const file of projected) {
  const promoted = fs.readFileSync(file,'utf8');
  const packet = JSON.parse(promoted);
  for (const selected of receipt.cases.filter(row => (
    file === 'specs/conformance/safety-cases.json' ? row.id.startsWith('S-CASE-') :
      file === 'specs/conformance/adaptation-cases.json' ? row.id === 'ADAPT-01' : row.id === 'DX-01'
  ))) {
    const row = packet.cases.find(item => item.id === selected.id);
    assert.ok(row);
    assert.equal(row.evidence.length,2,`${selected.id} missing current-source replay evidence`);
    const evidence = row.evidence[1];
    assert.equal(evidence.configuration.receipt_sha256,sha(bytes));
    if (external === undefined) external = evidence.configuration.external_raw_output;
    assert.equal(external,evidence.configuration.external_raw_output);
  }
  assert.equal(sha(restoreReplayEvidence(promoted,file)),receipt.prepromotion_case_sha256[file]);
}
assert.equal(sha(fs.readFileSync(path.join(external,'acceptance.json'))),sha(bytes));
for (const [file,digest] of Object.entries(receipt.source_sha256)) {
  if (!projected.has(file)) assert.equal(sha(fs.readFileSync(file)),digest,
    `accepted current-source replay source changed: ${file}`);
}
assert.equal(sha(fs.readFileSync(receipt.binary.path)),receipt.binary.sha256);
for (const peer of Object.values(receipt.peers)) assert.equal(sha(fs.readFileSync(peer.path)),peer.sha256);
assert.deepEqual(receipt.commands.map(command => command.label),[
  'build','build-diagnostic-check','build-safety-core-check','build-safety-handle-check',
  'dx01-session','dx01-typed-resource','adapt01-session','adapt01-compatible',
  'safety-source-and-kernel','cli-S-CASE-01','cli-S-CASE-14-0','cli-S-CASE-14-1',
  'safety-handle-forgery','historical-safety-core-postpromotion.mjs',
  'historical-safety-core-handle-postpromotion.mjs',
  'historical-release-policy-postpromotion.mjs',
  'historical-decoder-experiment-postpromotion.mjs',
]);
const staticRefusals = new Set([
  'dx01-session','adapt01-session','cli-S-CASE-01','cli-S-CASE-14-0','cli-S-CASE-14-1',
]);
for (const command of receipt.commands) {
  assert.equal(sha(fs.readFileSync(command.executable)),command.executable_sha256);
  assert.equal(sha(fs.readFileSync(path.join(external,command.stdout))),command.stdout_sha256);
  assert.equal(sha(fs.readFileSync(path.join(external,command.stderr))),command.stderr_sha256);
  assert.equal(command.error,null);
  assert.equal(command.signal,null);
  assert.equal(command.status,staticRefusals.has(command.label) ? 2 : 0,
    `${command.label} unexpected status`);
}
console.log(JSON.stringify({result:'passed',cases:receipt.cases.map(row => row.id),
  receipt_sha256:sha(bytes),source_revision:receipt.source_revision,
  original_full_case_bytes_verified:true,commands:receipt.commands.length,
  proof:'open',historical_receipts:'unchanged'}));
