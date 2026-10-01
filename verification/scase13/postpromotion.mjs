#!/usr/bin/env node
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'../..');
const file=name=>fs.readFileSync(path.join(root,name));
const sha=value=>crypto.createHash('sha256').update(value).digest('hex');
const receiptFile='verification/scase13/acceptance.json';
const receiptBytes=file(receiptFile);
const receipt=JSON.parse(receiptBytes);
assert.equal(receipt.schema,'noble-scase13-runtime-quotas/v1');
assert.equal(receipt.result,'passed');
const caseFile='specs/conformance/safety-cases.json';
const content=file(caseFile).toString();
const lines=content.split('\n').filter(line=>line.includes('"id":"S-CASE-13"'));
assert.equal(lines.length,1);
const row=JSON.parse(lines[0].trim().replace(/,$/,''));
assert.deepEqual({id:row.id,input:row.input,expected:row.expected},receipt.case);
assert.deepEqual(row.state,{implementation:'implemented',execution:'passed',proof:'open',trust:'explicit'});
assert.equal(row.evidence.length,1);
const evidence=row.evidence[0];
assert.equal(evidence.claim,receipt.claim);
assert.equal(evidence.source_revision,receipt.source_revision);
assert.equal(evidence.kind,'test');
assert.equal(evidence.result,'passed');
assert.equal(evidence.configuration.receipt,'../../verification/scase13/acceptance.json');
assert.equal(evidence.configuration.receipt_sha256,sha(receiptBytes));
const external=evidence.configuration.external_raw_output;
assert.equal(path.resolve(external),external);
assert.equal(sha(fs.readFileSync(path.join(external,'acceptance.json'))),sha(receiptBytes));
const prepromotion=fs.readFileSync(path.join(external,'prepromotion-safety-cases.json'),'utf8');
assert.equal(sha(prepromotion),receipt.prepromotion_case_sha256);
const previous=prepromotion.split('\n').filter(line=>line.includes('"id":"S-CASE-13"'));
assert.equal(previous.length,1);
assert.equal(content,prepromotion.replace(previous[0],lines[0]),'other canonical rows changed');
for(const [name,digest] of Object.entries(receipt.source_sha256)) {
  if(name!==caseFile) assert.equal(sha(file(name)),digest,`accepted source changed: ${name}`);
}
for(const [name,digest] of Object.entries(receipt.preserved_historical_sha256))
  assert.equal(sha(file(name)),digest,`historical receipt/runner changed: ${name}`);
for(const item of Object.values(receipt.binaries))
  assert.equal(sha(fs.readFileSync(item.path)),item.sha256);
assert.deepEqual(receipt.variants.map(variant=>variant.label),[
  'canonical-fuel-one','canonical-heap-sixty-five','positive-compute',
  'positive-heap-fifty-six','heap-record-overflow-fifty-seven','positive-heap-sixty-five']);
for(const variant of receipt.variants) {
  assert.equal(variant.observed.stage,'execution');
  assert.equal(variant.observed.wasm_memory_bytes,1048576);
  assert.equal(variant.observed.guest_callbacks,0);
  assert.equal(variant.observed.live_bytes_after_cleanup,0);
  assert.equal(variant.observed.cleanups,1);
}
for(const command of receipt.commands) {
  assert.equal(command.error,null);
  assert.equal(command.signal,null);
  assert.equal(command.status,0);
  assert.equal(sha(fs.readFileSync(command.executable)),command.executable_sha256);
  for(const kind of ['stdout','stderr'])
    assert.equal(sha(fs.readFileSync(path.join(external,command[kind]))),command[`${kind}_sha256`],`${command.label}.${kind}`);
}
console.log(JSON.stringify({result:'passed',case_id:'S-CASE-13',receipt_sha256:sha(receiptBytes),
  source_revision:receipt.source_revision,variants:receipt.variants.length,commands:receipt.commands.length}));
