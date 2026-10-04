import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { restoreHandlePromotion } from './handle-postpromotion.mjs';

const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const receiptBytes = fs.readFileSync('verification/safety-core/acceptance.json');
const receipt = JSON.parse(receiptBytes);
const receiptSha = sha(receiptBytes);
assert.equal(receipt.result,'passed');
assert.equal(receipt.kind,'test');
const file = 'specs/conformance/safety-cases.json';
let current = restoreHandlePromotion(fs.readFileSync(file,'utf8'));
const rows = JSON.parse(current).cases;
for (const id of ['S-CASE-01','S-CASE-05','S-CASE-14']) {
  const row = rows.find(item => item.id === id);
  assert.ok(row && row.kind === 'static');
  assert.deepEqual(row.state, {implementation:'implemented',execution:'passed',proof:'open',trust:'explicit'});
  assert.equal(row.evidence.length,1);
  const evidence = row.evidence[0];
  assert.equal(evidence.subject,id);
  assert.equal(evidence.kind,'test');
  assert.equal(evidence.result,'passed');
  assert.equal(evidence.source_revision,receipt.source_revision);
  assert.equal(evidence.configuration.receipt_sha256,receiptSha);
  assert.equal(evidence.configuration.prepromotion_case_sha256,receipt.prepromotion_case_sha256);
  const original = receipt.cases.find(item => item.id === id);
  assert.deepEqual({id:row.id,input:row.input,expected:row.expected},original);
  const lineStart = current.lastIndexOf('\n',current.indexOf(`"id":"${id}"`))+1;
  const start = current.indexOf('"state":',lineStart);
  const end = current.indexOf('\n',start);
  assert.ok(lineStart > 0 && start >= lineStart && end > start);
  current = current.slice(0,start) +
    '"state":{"implementation":"absent","execution":"not-run","proof":"open","trust":"unassessed"},"evidence":[]},' +
    current.slice(end);
}
assert.equal(sha(current),receipt.prepromotion_case_sha256);
assert.equal(sha(fs.readFileSync('verification/safety-core/check.rs')),
  receipt.source_sha256['verification/safety-core/check.rs']);
assert.equal(sha(fs.readFileSync('verification/safety-core/gate.mjs')),
  receipt.source_sha256['verification/safety-core/gate.mjs']);
const external = rows.find(item => item.id === 'S-CASE-01').evidence[0].configuration.external_raw_output;
assert.equal(sha(fs.readFileSync(path.join(external,'acceptance.json'))),receiptSha);
for (const command of receipt.commands) {
  assert.equal(sha(fs.readFileSync(path.join(external,command.stdout))),command.stdout_sha256);
  assert.equal(sha(fs.readFileSync(path.join(external,command.stderr))),command.stderr_sha256);
}
console.log(JSON.stringify({result:'passed',cases:receipt.cases.map(row => row.id),
  receipt_sha256:receiptSha,prepromotion_case_sha256:receipt.prepromotion_case_sha256,
  source_revision:receipt.source_revision,proofs:'open',historical_review_receipt:'unchanged'}));
