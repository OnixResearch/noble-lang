#!/usr/bin/env node
// Verify the accepted prepromotion source, exact S-CASE-04 projection, and
// permitted native Cairn sync without rerunning an already accepted guest gate.
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';

const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'../..');
const read=name=>fs.readFileSync(path.join(root,name));
const sha=bytes=>crypto.createHash('sha256').update(bytes).digest('hex');
const receiptBytes=read('verification/scase04/acceptance.json');
const receipt=JSON.parse(receiptBytes);
assert.equal(receipt.schema,'noble-scase04-recursive-resource-eligibility/v1');
assert.deepEqual([receipt.kind,receipt.result,receipt.failures,receipt.integrity_failures],
  ['test','passed',[],[]]);
assert.deepEqual(receipt.canonical_mapping && {
  stage:receipt.canonical_mapping.stage,outcome:receipt.canonical_mapping.outcome,
  cli_stage:receipt.canonical_mapping.cli_stage,
  cli_outcome:receipt.canonical_mapping.cli_outcome,
  guest_requests:receipt.canonical_mapping.guest_requests,
  protected_operations:receipt.canonical_mapping.protected_operations,
},{stage:'check',outcome:'eligibility-reject',cli_stage:'component-check',
  cli_outcome:'error',guest_requests:0,protected_operations:0});
const external=receipt.external_raw_output;
assert.equal(path.isAbsolute(external),true);
assert.equal(sha(fs.readFileSync(path.join(external,'acceptance.json'))),sha(receiptBytes));
for(const [name,snapshot] of Object.entries(receipt.prepromotion_inputs)) {
  const bytes=fs.readFileSync(path.join(external,snapshot.path));
  assert.equal(sha(bytes),snapshot.sha256,`changed prepromotion snapshot: ${name}`);
}
assert.equal(receipt.prepromotion_inputs.case.sha256,receipt.prepromotion_case_sha256);
const inputs=Object.values(receipt.input_sha256);
assert.ok(inputs.length>=18,'all WIT and source fixtures must be bound');
for(const input of inputs) {
  const file=path.resolve(external,input.path);
  assert.equal(file.startsWith(`${external}${path.sep}`),true,'input escapes frozen output');
  assert.equal(sha(fs.readFileSync(file)),input.sha256,`changed input fixture: ${file}`);
}
const wits=inputs.filter(input=>input.path.endsWith('.wit'));
assert.equal(wits.length,3,'real, malformed and unsupported WIT controls must be frozen');
assert.ok(wits.some(input=>input.sha256===receipt.wit_sha256));
const caseFile='specs/conformance/safety-cases.json';
const canonical=read(caseFile).toString();
const lines=canonical.split('\n').filter(line=>line.includes('"id":"S-CASE-04"'));
assert.equal(lines.length,1);
const row=JSON.parse(lines[0].trim().replace(/,$/,''));
assert.deepEqual({id:row.id,input:row.input,expected:row.expected},receipt.case);
assert.deepEqual(row.state,{implementation:'implemented',execution:'passed',proof:'open',trust:'explicit'});
assert.equal(row.evidence.length,1);
const evidence=row.evidence[0];
assert.equal(evidence.subject,'S-CASE-04');
assert.equal(evidence.result,'passed');
assert.equal(evidence.source_revision,receipt.source_revision);
assert.equal(evidence.configuration.receipt,'../../verification/scase04/acceptance.json');
assert.equal(evidence.configuration.receipt_sha256,sha(receiptBytes));
assert.equal(evidence.configuration.gate_sha256,receipt.source_sha256['verification/scase04/gate.mjs']);
assert.equal(evidence.configuration.prepromotion_case_sha256,receipt.prepromotion_case_sha256);
assert.equal(evidence.configuration.external_raw_output,external);
const prior={...row,state:{implementation:'absent',execution:'not-run',proof:'open',trust:'unassessed'},evidence:[]};
assert.equal(sha(canonical.replace(lines[0],`    ${JSON.stringify(prior)},`)),receipt.prepromotion_case_sha256,
  'only the S-CASE-04 state and its new evidence may change');
const preNative=fs.readFileSync(path.join(external,receipt.prepromotion_inputs.safety_spec.path),'utf8');
const nowNative=read('.cairn/specs/safety/spec.md').toString();
const sections=text=>text.split(/(?=^### Requirement: )/m);
const before=sections(preNative),after=sections(nowNative);
assert.equal(after.length,before.length,'native requirement inventory changed');
for(let at=0;at<before.length;at++) {
  const id=/^### Requirement: ([A-Z0-9-]+)/.exec(before[at])?.[1];
  assert.equal(/^### Requirement: ([A-Z0-9-]+)/.exec(after[at])?.[1],id);
  if(id==='S-RES-01') {
    assert.match(after[at],/production checker MUST refuse `dup`, generic `drop` and `quote` capture/);
    assert.match(after[at],/selected route to generic serialization is `quote reflect`/);
  } else if(id==='S-RES-04') {
    assert.match(after[at],/checker MUST inspect both alternatives of `Sum<Resource<R>,I64>`/);
    assert.match(after[at],/canonical `check\/eligibility-reject` result is a harness classification/);
  } else assert.equal(after[at],before[at],`unrelated native requirement changed: ${id}`);
}
const change='.cairn/changes/safety-recursive-resource-eligibility/';
function current(file) {
  if(!file.startsWith(change)||fs.existsSync(path.join(root,file))) return file;
  const date=evidence.configuration.archive_date;
  assert.match(date??'',/^\d{4}-\d{2}-\d{2}$/,'archive requires exact date');
  const archived=`.cairn/archive/${date}-safety-recursive-resource-eligibility/${file.slice(change.length)}`;
  assert.equal(fs.existsSync(path.join(root,archived)),true,`missing accepted change file: ${file}`);
  return archived;
}
for(const [name,digest] of Object.entries(receipt.source_sha256)) {
  if(name===caseFile||name==='.cairn/specs/safety/spec.md') continue;
  const live=current(name),bytes=read(live);
  if(name===`${change}tasks.md`) {
    assert.equal(bytes.toString().replace(/- \[x\]/g,'- [ ]'),
      fs.readFileSync(path.join(external,receipt.prepromotion_inputs.change_tasks.path),'utf8'),
      'only checklist completion may differ');
  } else assert.equal(sha(bytes),digest,`accepted source changed: ${name}`);
}
assert.equal(receipt.source_revision,
  `sha256:${sha(JSON.stringify(Object.entries(receipt.source_sha256).sort()))}`);
for(const [name,digest] of Object.entries(receipt.historical_receipt_sha256))
  assert.equal(sha(read(name)),digest,`accepted historical receipt changed: ${name}`);
for(const command of receipt.commands) {
  assert.equal(command.error,null,`${command.label} spawn`);
  assert.equal(command.signal,null,`${command.label} signal`);
  assert.ok([0,2,4].includes(command.status),`${command.label} exit`);
  assert.equal(sha(fs.readFileSync(command.executable)),command.executable_sha256,
    `${command.label} executable`);
  for(const kind of ['stdout','stderr'])assert.equal(
    sha(fs.readFileSync(path.join(external,command[kind]))),command[`${kind}_sha256`],
    `${command.label} ${kind}`);
}
const kind=receipt.typed.resource_kind;
assert.equal(Number.isInteger(kind)&&kind>0,true);
const whole={
  'Resource<R>':`Resource(ResourceKind(${kind}))`,
  'Pair<Text,Resource<R>>':`Pair(Text, Resource(ResourceKind(${kind})))`,
  'Sum<Resource<R>,I64>':`Sum(Resource(ResourceKind(${kind})), I64)`,
  'List<Resource<R>>':`List(Resource(ResourceKind(${kind})))`,
};
assert.deepEqual(receipt.typed.prefixes.map(item=>item.type),row.input.input_types);
for(const item of receipt.typed.prefixes)
  assert.equal(item.actual_type,whole[item.type]);
assert.equal(receipt.typed.negatives.length,16);
assert.equal(receipt.matrix.length,16);
assert.deepEqual(receipt.typed.positive_sum,{
  source:'false [ counters.open inl ] [ 7 inr ] if [ counters.close ] [ dup drop ] case',
  output_type:'I64',lowered:false,
});
for(const type of row.input.input_types) for(const operation of row.input.operations) {
  const word=operation==='generic-serialization'?'quote':operation;
  const typed=receipt.typed.negatives.filter(item=>item.type===type&&item.operation===operation);
  const matrix=receipt.matrix.filter(item=>item.type===type&&item.operation===operation);
  assert.equal(typed.length,1);assert.equal(matrix.length,1);
  assert.equal(typed[0].word,word);
  assert.equal(typed[0].constraint,'eligibility:Data');
  assert.equal(typed[0].actual_stack,`S ${whole[type]}`);
  assert.equal(matrix[0].word,word);
  assert.equal(matrix[0].actual_type,whole[type]);
  assert.equal(matrix[0].observed.outcome,'error');
  assert.equal(matrix[0].observed.component_emitted,false);
  assert.equal(matrix[0].observed.diagnostic.code,'component-check');
  assert.equal(matrix[0].observed.diagnostic.message,
    `Export: kernel rejected a non-capturable value; word ${word}; required S Data; actual S ${whole[type]}`);
}
const status=JSON.parse(read('specs/STATUS.json'));
const summary=status.recursive_resource_eligibility;
assert.deepEqual({
  case:summary.case,implementation:summary.implementation,
  execution:summary.execution,proof:summary.proof,trust:summary.trust,
  receipt:summary.receipt,receipt_sha256:summary.receipt_sha256,
  source_revision:summary.source_revision,variants:summary.variants,
},{
  case:'S-CASE-04',implementation:'implemented',execution:'passed',
  proof:'open',trust:'explicit',receipt:'../verification/scase04/acceptance.json',
  receipt_sha256:sha(receiptBytes),source_revision:receipt.source_revision,variants:16,
});
assert.match(read('specs/ROADMAP.md').toString(),/S-CASE-04 recursive resource eligibility/);
console.log(JSON.stringify({result:'passed',case_id:'S-CASE-04',source_revision:receipt.source_revision,
  receipt_sha256:sha(receiptBytes),negatives:receipt.matrix.length,
  proof:'open',archive:evidence.configuration.archive_date??'active'}));
