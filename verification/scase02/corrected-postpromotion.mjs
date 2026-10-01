#!/usr/bin/env node
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'../..');
const file=name=>fs.readFileSync(path.join(root,name));
const sha=value=>crypto.createHash('sha256').update(value).digest('hex');
const bytes=file('verification/scase02/corrected-acceptance.json');
const receipt=JSON.parse(bytes);
assert.equal(receipt.schema,'noble-scase02-bounded-region-corrected/v1');
assert.equal(receipt.result,'passed');
const text=file('specs/conformance/safety-cases.json').toString();
const lines=text.split('\n').filter(line=>line.includes('"id":"S-CASE-02"'));
assert.equal(lines.length,1);
const row=JSON.parse(lines[0].trim().replace(/,$/,''));
assert.deepEqual({id:row.id,input:row.input,expected:row.expected},receipt.case);
assert.deepEqual(row.state,{implementation:'implemented',execution:'passed',proof:'open',trust:'explicit'});
assert.equal(row.evidence.length,2);
const historical=row.evidence[0],current=row.evidence[1];
assert.equal(historical.configuration.receipt_sha256,receipt.historical_receipt_sha256);
assert.equal(sha(file('verification/scase02/acceptance.json')),receipt.historical_receipt_sha256);
assert.equal(sha(file('verification/scase02/gate.mjs')),
  '43d96df4ae68b0fc2a3d9f15f1664cb162d4a9f1addd16287c3658b0150bdab3');
assert.equal(sha(file('verification/scase02/corrected-gate.mjs')),current.configuration.gate_sha256);
assert.equal(current.source_revision,receipt.source_revision);
assert.equal(current.configuration.receipt,'../../verification/scase02/corrected-acceptance.json');
assert.equal(current.configuration.receipt_sha256,sha(bytes));
assert.equal(current.configuration.prepromotion_case_sha256,receipt.prepromotion_case_sha256);
const prior={...row,evidence:[historical]};
assert.equal(sha(text.replace(lines[0],`    ${JSON.stringify(prior)},`)),receipt.prepromotion_case_sha256);
const external=current.configuration.external_raw_output;
assert.equal(sha(fs.readFileSync(path.join(external,'acceptance.json'))),sha(bytes));
for(const command of receipt.commands) {
  assert.equal(command.status,0,command.label);
  assert.equal(command.signal,null,command.label);
  assert.equal(command.error,null,command.label);
  assert.equal(sha(fs.readFileSync(path.join(external,command.stdout))),command.stdout_sha256);
  assert.equal(sha(fs.readFileSync(path.join(external,command.stderr))),command.stderr_sha256);
}
const cross=receipt.commands.find(command=>command.label==='invalid-owner-and-boundary-tests');
assert.ok(cross);
const transcript=fs.readFileSync(path.join(external,cross.stdout),'utf8');
assert.match(transcript,/same_slot_and_generation_from_another_live_invocation_cannot_cross \.\.\. ok/);
assert.match(transcript,/3 passed; 0 failed/);
const canonical=receipt.cases.find(item=>item.label==='canonical-scase02');
const positive=receipt.cases.find(item=>item.label==='positive-in-bounds');
assert.equal(canonical.observed.region_reads,0);
assert.equal(canonical.observed.protected_operations,0);
assert.equal(positive.observed.bytes_hex,'0102');
assert.equal(positive.observed.released_owners,1);
for(const [name,digest] of Object.entries(receipt.source_sha256)) {
  if(name!=='specs/conformance/safety-cases.json') assert.equal(sha(file(name)),digest,`changed source: ${name}`);
}
console.log(JSON.stringify({result:'passed',case_id:row.id,source_revision:receipt.source_revision,
  receipt_sha256:sha(bytes),historical_receipt_sha256:receipt.historical_receipt_sha256,
  commands:receipt.commands.length}));
