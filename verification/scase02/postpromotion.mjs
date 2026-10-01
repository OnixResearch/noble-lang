#!/usr/bin/env node
// Verify canonical promotion against immutable prepromotion bytes and raw runs.
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'../..');
const sha=value=>crypto.createHash('sha256').update(value).digest('hex');
const file=name=>fs.readFileSync(path.join(root,name));
const receiptFile='verification/scase02/acceptance.json';
const receiptBytes=file(receiptFile),receipt=JSON.parse(receiptBytes);
assert.equal(receipt.schema,'noble-scase02-bounded-region/v1');
assert.equal(receipt.result,'passed');
assert.equal(receipt.case.id,'S-CASE-02');
const caseFile='specs/conformance/safety-cases.json';
const text=file(caseFile).toString();
const rows=text.split('\n');
const matches=rows.filter(line=>line.includes('"id":"S-CASE-02"'));
assert.equal(matches.length,1,'one canonical case row');
const line=matches[0];
const row=JSON.parse(line.trim().replace(/,$/,''));
assert.deepEqual({id:row.id,input:row.input,expected:row.expected},receipt.case);
assert.deepEqual(row.state,{implementation:'implemented',execution:'passed',proof:'open',trust:'explicit'});
assert.equal(row.evidence.length,1);
const evidence=row.evidence[0];
assert.equal(evidence.kind,'test');
assert.equal(evidence.result,'passed');
assert.equal(evidence.subject,'S-CASE-02');
assert.equal(evidence.source_revision,receipt.source_revision);
assert.equal(evidence.configuration.receipt,'../../'+receiptFile);
assert.equal(evidence.configuration.receipt_sha256,sha(receiptBytes));
assert.equal(evidence.configuration.prepromotion_case_sha256,receipt.prepromotion_case_sha256);
assert.equal(evidence.configuration.component_sha256,receipt.component_sha256);
assert.equal(evidence.configuration.binary_sha256,receipt.binary_sha256);
const old={...row,state:{implementation:'absent',execution:'not-run',proof:'open',trust:'unassessed'},evidence:[]};
assert.equal(sha(text.replace(line,`    ${JSON.stringify(old)},`)),receipt.prepromotion_case_sha256);
const external=evidence.configuration.external_raw_output;
assert.equal(sha(fs.readFileSync(path.join(external,'acceptance.json'))),sha(receiptBytes));
for(const command of receipt.commands) {
  assert.equal(command.status,0,command.label);
  assert.equal(command.signal,null,command.label);
  assert.equal(command.error,null,command.label);
  assert.equal(sha(fs.readFileSync(path.join(external,command.stdout))),command.stdout_sha256);
  assert.equal(sha(fs.readFileSync(path.join(external,command.stderr))),command.stderr_sha256);
}
assert.equal(receipt.cases.length,9);
const exact=receipt.cases.find(item=>item.label==='canonical-scase02');
assert.equal(exact.observed.outcome,'bounds-reject');
assert.equal(exact.observed.protected_operations,0);
assert.equal(exact.observed.region_reads,0);
for(const [name,digest] of Object.entries(receipt.source_sha256)) {
  if(name!==caseFile) assert.equal(sha(file(name)),digest,`reviewed gate source changed: ${name}`);
}
console.log(JSON.stringify({result:'passed',case_id:row.id,receipt_sha256:sha(receiptBytes),
  source_revision:receipt.source_revision,commands:receipt.commands.length}));
