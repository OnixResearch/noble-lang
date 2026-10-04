#!/usr/bin/env node
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'../..');
const read=name=>fs.readFileSync(path.join(root,name));
const sha=value=>crypto.createHash('sha256').update(value).digest('hex');
const receiptBytes=read('verification/scase06/acceptance.json');
const receipt=JSON.parse(receiptBytes);
assert.equal(receipt.schema,'noble-scase06-authorized-fs/v1');
assert.equal(receipt.result,'passed');
assert.deepEqual(receipt.failures,[]);
assert.deepEqual(receipt.integrity_failures,[]);
const canonical=read('specs/conformance/safety-cases.json').toString();
const lines=canonical.split('\n').filter(line=>line.includes('"id":"S-CASE-06"'));
assert.equal(lines.length,1);
const row=JSON.parse(lines[0].trim().replace(/,$/,''));
assert.deepEqual({id:row.id,input:row.input,expected:row.expected},receipt.case);
assert.deepEqual(row.state,{implementation:'implemented',execution:'passed',proof:'open',trust:'explicit'});
assert.equal(row.evidence.length,1);
const evidence=row.evidence[0];
assert.equal(evidence.subject,'S-CASE-06');
assert.equal(evidence.source_revision,receipt.source_revision);
assert.equal(evidence.configuration.receipt,'../../verification/scase06/acceptance.json');
assert.equal(evidence.configuration.receipt_sha256,sha(receiptBytes));
assert.equal(evidence.configuration.gate_sha256,receipt.source_sha256['verification/scase06/gate.mjs']);
assert.equal(evidence.configuration.prepromotion_case_sha256,receipt.prepromotion_case_sha256);
const former={...row,state:{implementation:'absent',execution:'not-run',proof:'open',trust:'unassessed'},evidence:[]};
assert.equal(sha(canonical.replace(lines[0],`    ${JSON.stringify(former)},`)),receipt.prepromotion_case_sha256);
const external=evidence.configuration.external_raw_output;
assert.equal(sha(fs.readFileSync(path.join(external,'acceptance.json'))),sha(receiptBytes));
assert.equal(sha(fs.readFileSync(path.join(external,'component/component.wasm'))),receipt.component_sha256);
assert.equal(sha(fs.readFileSync(path.join(external,'guest.noble'))),receipt.guest_source_sha256);
assert.equal(sha(fs.readFileSync(path.join(external,'main.rs'))),receipt.fixture_sha256);
assert.equal(sha(fs.readFileSync(receipt.commands[0].executable)),receipt.commands[0].executable_sha256);
assert.equal(sha(fs.readFileSync(path.join(external,'target/debug/noble'))),receipt.binary_sha256);
for(const command of receipt.commands){
  assert.equal(command.status,command.label==='mismatched-host-source-refusal'?2:0,`${command.label} exit`);
  assert.equal(command.signal,null,command.label);
  assert.equal(command.error,null,command.label);
  assert.equal(sha(fs.readFileSync(command.executable)),command.executable_sha256);
  for(const kind of ['stdout','stderr'])assert.equal(sha(fs.readFileSync(path.join(external,command[kind]))),command[`${kind}_sha256`]);
}
assert.deepEqual(receipt.observations.map(row=>row.label),['canonical-scase06','same-compiled-guest-positive']);
assert.deepEqual(receipt.observations.map(row=>row.observed.outcome),['denied','read']);
assert.deepEqual(receipt.observations.map(row=>row.observed.protected_operations),[0,1]);
for(const [name,digest] of Object.entries(receipt.historical_receipt_sha256))assert.equal(sha(read(name)),digest,`historical receipt changed: ${name}`);
for(const [name,digest] of Object.entries(receipt.source_sha256)){
  if(name==='specs/conformance/safety-cases.json')continue;
  // Native change text is immutable; only checklist state may advance. The
  // final checkbox remains unchecked until chronological source replay.
  const candidate=name.startsWith('.cairn/changes/safety-authorized-fs-read/')
    ? `.cairn/archive/${evidence.configuration.archive_date}-safety-authorized-fs-read/${name.slice('.cairn/changes/safety-authorized-fs-read/'.length)}`
    : name;
  if(name.endsWith('/tasks.md')){
    const original=fs.readFileSync(path.join(external,'source-tasks.md'),'utf8');
    assert.equal(sha(original),digest);
    const current=read(candidate).toString().replace(/- \[x\]/g,'- [ ]');
    assert.equal(current,original,'only final Cairn checkbox may change after freeze');
  }else assert.equal(sha(read(candidate)),digest,`accepted source changed: ${name}`);
}
console.log(JSON.stringify({result:'passed',case_id:row.id,source_revision:receipt.source_revision,
  receipt_sha256:sha(receiptBytes),commands:receipt.commands.length}));
