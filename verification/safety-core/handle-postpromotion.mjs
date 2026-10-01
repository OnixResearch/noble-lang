import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { restoreReplayEvidence } from '../current-source-replay/projection.mjs';

const sha = value => crypto.createHash('sha256').update(value).digest('hex');
const receiptBytes = fs.readFileSync('verification/safety-core/handle-acceptance.json');
const receipt = JSON.parse(receiptBytes);
const receiptSha = sha(receiptBytes);
const caseId = 'S-CASE-03';
const caseFile = 'specs/conformance/safety-cases.json';

export function restoreHandlePromotion(promoted) {
  promoted = restoreReplayEvidence(promoted,caseFile);
  assert.equal(receipt.result,'passed');
  assert.equal(receipt.kind,'test');
  assert.equal(receipt.cases.length,1);
  assert.equal(receipt.cases[0].id,caseId);
  const row = JSON.parse(promoted).cases.find(item => item.id === caseId);
  assert.ok(row && row.kind === 'adapter');
  assert.deepEqual(row.state,{implementation:'implemented',execution:'passed',proof:'open',trust:'explicit'});
  assert.equal(row.evidence.length,1);
  const evidence = row.evidence[0];
  assert.equal(evidence.subject,caseId);
  assert.equal(evidence.kind,'test');
  assert.equal(evidence.result,'passed');
  assert.equal(evidence.source_revision,receipt.source_revision);
  assert.equal(evidence.configuration.receipt_sha256,receiptSha);
  assert.equal(evidence.configuration.prepromotion_case_sha256,receipt.prepromotion_case_sha256);
  assert.equal(evidence.configuration.binary_sha256,receipt.binary.sha256);
  assert.equal(evidence.configuration.receipt,'../../verification/safety-core/handle-acceptance.json');
  assert.equal(evidence.configuration.gate,'../../verification/safety-core/handle-gate.mjs');
  assert.deepEqual({id:row.id,input:row.input,expected:row.expected},receipt.cases[0]);
  const external = evidence.configuration.external_raw_output;
  assert.equal(sha(fs.readFileSync(path.join(external,'acceptance.json'))),receiptSha);
  assert.equal(sha(fs.readFileSync(receipt.binary.path)),receipt.binary.sha256);
  for (const command of receipt.commands) {
    assert.equal(sha(fs.readFileSync(path.join(external,command.stdout))),command.stdout_sha256);
    assert.equal(sha(fs.readFileSync(path.join(external,command.stderr))),command.stderr_sha256);
  }
  for (const file of ['verification/safety-core/handle-check.rs',
    'verification/safety-core/handle-gate.mjs','verification/safety-core/acceptance.json',
    'verification/release-policy/acceptance.json']) {
    assert.equal(sha(fs.readFileSync(file)),receipt.source_sha256[file],`${file} changed since source-bound gate`);
  }
  const lineStart = promoted.lastIndexOf('\n',promoted.indexOf(`"id":"${caseId}"`))+1;
  const stateStart = promoted.indexOf('"state":',lineStart);
  const lineEnd = promoted.indexOf('\n',stateStart);
  assert.ok(lineStart>0 && stateStart>=lineStart && lineEnd>stateStart);
  const original = promoted.slice(0,stateStart) +
    '"state":{"implementation":"absent","execution":"not-run","proof":"open","trust":"unassessed"},"evidence":[]},' +
    promoted.slice(lineEnd);
  assert.equal(sha(original),receipt.prepromotion_case_sha256,
    'handle promotion did not reconstruct exact historical safety case bytes');
  return original;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  restoreHandlePromotion(fs.readFileSync(caseFile,'utf8'));
  console.log(JSON.stringify({result:'passed',cases:[caseId],receipt_sha256:receiptSha,
    prepromotion_case_sha256:receipt.prepromotion_case_sha256,proof:'open'}));
}
