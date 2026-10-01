import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {root,read,sha,caseFiles,caseRow,selectedCases,historic,acceptedReceipt,
  sourceInventory,sourceTrees,revision,freshOutput,
  commandsAt as baseCommandsAt,verifyRecordedCommands,removeNewestEvidence}
  from '../dx06-current-source/source.mjs';
import {sourceFiles as precedingInputs,compiledImportControls}
  from '../scase16-current-source/source.mjs';
import {older as priorReceipts} from '../scase16-final-source/source.mjs';

export {root,read,sha,caseFiles,caseRow,selectedCases,revision,
  freshOutput,verifyRecordedCommands,removeNewestEvidence,compiledImportControls};
export const folder='verification/scase08-current-source';
export const dxReceipt=`${folder}/dx06-acceptance.json`;
export const replayReceipt=`${folder}/acceptance.json`;
export const older={...priorReceipts,
  'verification/scase16-final-source/dx06-acceptance.json':
    'bc36af63a8d80beb9e91156a6cae086d09c927a07786f2e276eca9c230249244',
  'verification/scase16-final-source/acceptance.json':
    '194ee1cadf54d7e3def7f761758721397bdfb5dcc3f78d04523c639776895464',
  'verification/scase02/acceptance.json':
    '23f2b73aff9ee4dfa0bbd7d613dba3e569caa316409a49abe9f7884eddcec0b1',
  'verification/scase02/corrected-acceptance.json':
    '3588522e32149a490ea0ec8b7c33abbcb229150b91e24a31d164a17b60a740d0',
  'verification/scase02-current-source/dx06-acceptance.json':
    'd092350b77ae605dc182e1b46d60cf5dd3f2737e447b27b3fccd99f0c8bbfb0f',
  'verification/scase02-current-source/acceptance.json':
    'dd36ff5c04808c16461ab34a1c905a73e942e73a63e889fa1ee9647bbca72046',
  'verification/scase07/acceptance.json':
    '6bb9ee9bf3d9ed1077818426d29a1bb9895fe1733e1d8bbb3041e3d7fa2013d5',
  'verification/scase07-current-source/dx06-acceptance.json':
    '3b9e726f1d9204655b72289228c3321d158de336381130b66ef8228a26af7509',
  'verification/scase07-current-source/acceptance.json':
    'b20e176c5cbe4ca7af997771b58ad569e76baae783bb3c7a36f7fdef7292fbc1',
  'verification/scase13/acceptance.json':
    '5e5e538d1e5ec90221f680de03f9063d2994073361e814668d9995fc9c45ae1e',
  'verification/scase13/corrected-acceptance.json':
    'e8211d6d2b0c77ac540874a24dc5f8dedf3bd3773c60b1105975ad9b485fcf28',
  'verification/scase13-current-source/dx06-acceptance.json':
    '4d0c779090282f633e6930ae5a045e88ff44d2f4cc22da2f906528d0a1c266c0',
  'verification/scase13-current-source/acceptance.json':
    '65ea09a8cc7e3a0a05f7f990d4b5e8cff7977192c3ecf9cbee51f4d876ff8e37',
  'verification/scase06/acceptance.json':
    '5fed58285741886c1c019dc07878e7b4c8d138d9110d240b78a0c25de7d299d8',
  'verification/scase06/corrected-acceptance.json':
    '304fad0aced504a9f241c2f0f05be4dc5d221e8e88caa97c36516079f6b428f9',
  'verification/scase06/final-acceptance.json':
    '6943c1a03163ebe5c7e98c1cdc4187cfa02a8c7007c57702e7f241d5679ab418',
  'verification/scase06-current-source/dx06-acceptance.json':
    '8c8ea0587ad033ba433fdfcaaff0a7a1cefd75b8d9e1c4dc54193a77625e22ff',
  'verification/scase06-current-source/acceptance.json':
    'aeaaba6afa2c2a7f4e18ea7704b4fc46b2c6ba029f42eb4779f1d71c15686da1',
  'verification/scase04/acceptance.json':
    '16ba34b6672486271eaef6bc22e7fd86c476374fd4f46acb745e66c2fc87f8be',
  'verification/scase04-current-source/dx06-acceptance.json':
    'cc8abef6995e4ec74dac74bab46789d55014199a8f4afa20c938946284928146',
  'verification/scase04-current-source/acceptance.json':
    '850f455e59f41be52e2ae1b7996473825af80d00653517c720b0d4c0aafcd2c3',
  'verification/scase08/acceptance.json':
    'e4da8ac29888544ef6586b12582cc1abd2d3c1065bd5c7c10d70897179402925'};
const own=['source.mjs','gate.mjs','projection.mjs','postpromotion.mjs']
  .map(file=>`${folder}/${file}`);
const inherited=[
  ...['source.mjs','gate.mjs','postpromotion.mjs']
    .map(file=>`verification/scase16-final-source/${file}`),
  ...['source.mjs','gate.mjs','projection.mjs','postpromotion.mjs']
    .map(file=>`verification/scase02-current-source/${file}`),
  ...['source.mjs','gate.mjs','projection.mjs','postpromotion.mjs']
    .map(file=>`verification/scase07-current-source/${file}`),
  ...['source.mjs','gate.mjs','projection.mjs','postpromotion.mjs']
    .map(file=>`verification/scase13-current-source/${file}`),
  ...['source.mjs','gate.mjs','projection.mjs','postpromotion.mjs']
    .map(file=>`verification/scase06-current-source/${file}`),
  ...['source.mjs','gate.mjs','projection.mjs','postpromotion.mjs']
    .map(file=>`verification/scase04-current-source/${file}`)];
const trees=[...sourceTrees,'.cairn/changes/safety-wasm-manifest-admission',
  '.cairn/changes/safety-bounded-region-adapter',
  '.cairn/changes/safety-artifact-correspondence'];
const quotaChange='.cairn/changes/safety-runtime-quotas';
function quotaChangeSources(directory=quotaChange) {
  return fs.readdirSync(path.join(root,directory),{withFileTypes:true})
    .sort((left,right)=>left.name.localeCompare(right.name))
    .flatMap(entry=>{
      const file=`${directory}/${entry.name}`;
      assert.equal(entry.isSymbolicLink(),false,`unbound quota source symlink: ${file}`);
      if(entry.isDirectory()) return quotaChangeSources(file);
      assert.equal(entry.isFile(),true,`unbound quota source kind: ${file}`);
      return file===`${quotaChange}/tasks.md`?[]:[file];
    });
}
const fsChange='.cairn/changes/safety-authorized-fs-read';
const archivedFsChange='.cairn/archive/2026-09-30-safety-authorized-fs-read';
const fsTasks=`${fsChange}/tasks.md`;
const resourceChange='.cairn/changes/safety-recursive-resource-eligibility';
const archivedResourceChange='.cairn/archive/2026-09-30-safety-recursive-resource-eligibility';
const resourceTasks=`${resourceChange}/tasks.md`;
const callbackChange='.cairn/changes/safety-callback-returned-owner';
const callbackTasks=`${callbackChange}/tasks.md`;
export function sourceDigest(file) {
  const bytes=read(file);
  return sha(file===fsTasks||file===resourceTasks||file===callbackTasks
    ?bytes.toString('utf8').replace(/- \[x\]/g,'- [ ]'):bytes);
}
export function frozen(sources) {
  for(const [file,digest] of Object.entries(sources))
    assert.equal(sourceDigest(file),digest,`source changed during source-bound execution: ${file}`);
}
function fsChangeSources(directory=fsChange) {
  assert.equal(fs.existsSync(path.join(root,archivedFsChange)),false,
    'S-CASE-06 assurance/archive is blocked; do not relabel the active snapshot');
  return fs.readdirSync(path.join(root,directory),{withFileTypes:true})
    .sort((left,right)=>left.name.localeCompare(right.name))
    .flatMap(entry=>{
      const file=`${directory}/${entry.name}`;
      assert.equal(entry.isSymbolicLink(),false,`unbound FS source symlink: ${file}`);
      if(entry.isDirectory()) return fsChangeSources(file);
      assert.equal(entry.isFile(),true,`unbound FS source kind: ${file}`);
      return [file];
    });
}
function resourceChangeSources(directory=resourceChange) {
  assert.equal(fs.existsSync(path.join(root,archivedResourceChange)),false,
    'S-CASE-04 is accepted on an active, not archived, Cairn change');
  return fs.readdirSync(path.join(root,directory),{withFileTypes:true})
    .sort((left,right)=>left.name.localeCompare(right.name))
    .flatMap(entry=>{
      const file=`${directory}/${entry.name}`;
      assert.equal(entry.isSymbolicLink(),false,`unbound Resource source symlink: ${file}`);
      if(entry.isDirectory()) return resourceChangeSources(file);
      assert.equal(entry.isFile(),true,`unbound Resource source kind: ${file}`);
      return [file];
    });
}
function callbackChangeSources(directory=callbackChange) {
  assert.equal(fs.readdirSync(path.join(root,'.cairn/archive'))
    .some(name=>name.endsWith('-safety-callback-returned-owner')),false,
  'S-CASE-08 is accepted on an active, not archived, Cairn change');
  return fs.readdirSync(path.join(root,directory),{withFileTypes:true})
    .sort((left,right)=>left.name.localeCompare(right.name))
    .flatMap(entry=>{
      const file=`${directory}/${entry.name}`;
      assert.equal(entry.isSymbolicLink(),false,`unbound callback source symlink: ${file}`);
      if(entry.isDirectory()) return callbackChangeSources(file);
      assert.equal(entry.isFile(),true,`unbound callback source kind: ${file}`);
      return [file];
    });
}
function verifyAcceptedCallbackTaskProse(digest) {
  const text=read(callbackTasks).toString('utf8');
  const checklist=/^- \[[ x]\]/gm;
  const markers=[...text.matchAll(checklist)];
  assert.ok(markers.length>0&&markers.length<=12,
    'bounded finite callback checklist projection');
  let matches=0;
  for(let mask=0;mask<2**markers.length;mask++) {
    let index=0;
    const candidate=text.replace(checklist,()=>`- [${mask&(1<<index++)?'x':' '}]`);
    if(sha(candidate)===digest) matches++;
  }
  assert.equal(matches,1,
    'exactly one accepted S-CASE-08 task preimage must exist by checkbox markers alone');
}
export function commandsAt(artifacts) {
  const command=baseCommandsAt(artifacts);
  // The newer pinned Wasmtime path dependency requires addr2line, whose
  // offline registry entry exists in the selected user's populated Cargo home.
  // Keep the target, temp and compiler wrappers externally isolated.
  command.environment.CARGO_HOME=path.join(path.dirname(root), '..', '.cargo');
  assert.equal(fs.realpathSync(command.environment.CARGO_HOME),
    '/home/brittonr/.cargo');
  return command;
}
export function prerequisites() {
  const historical=historic();
  for(const [file,digest] of Object.entries(older))
    assert.equal(sha(read(file)),digest,`immutable prerequisite changed: ${file}`);
  const editor=acceptedReceipt('DX-02'),forged=acceptedReceipt('ADAPT-06');
  const initial=JSON.parse(read('verification/scase16/acceptance.json'));
  const safety=JSON.parse(read('specs/conformance/safety-cases.json'));
  const case16=safety.cases.find(row=>row.id==='S-CASE-16');
  assert.deepEqual(case16.state,{implementation:'implemented',execution:'passed',
    proof:'open',trust:'explicit'});
  assert.equal(case16.evidence[0].configuration.receipt_sha256,
    older['verification/scase16/acceptance.json']);
  const bounded=safety.cases.find(row=>row.id==='S-CASE-02');
  const accepted=JSON.parse(read('verification/scase02/corrected-acceptance.json'));
  assert.deepEqual(bounded.state,{implementation:'implemented',execution:'passed',
    proof:'open',trust:'explicit'});
  assert.equal(bounded.evidence.length,2);
  assert.deepEqual({id:bounded.id,input:bounded.input,expected:bounded.expected},accepted.case);
  assert.deepEqual([accepted.schema,accepted.kind,accepted.result],
    ['noble-scase02-bounded-region-corrected/v1','test','passed']);
  assert.equal(accepted.source_revision,bounded.evidence[1].source_revision);
  assert.equal(bounded.evidence[0].configuration.receipt_sha256,
    older['verification/scase02/acceptance.json']);
  assert.equal(bounded.evidence[1].configuration.receipt_sha256,
    older['verification/scase02/corrected-acceptance.json']);
  assert.equal(accepted.historical_receipt_sha256,
    older['verification/scase02/acceptance.json']);
  assert.equal(accepted.cases.find(row=>row.label==='canonical-scase02').observed.region_reads,0);
  assert.equal(accepted.cases.find(row=>row.label==='positive-in-bounds').observed.bytes_hex,'0102');
  const external=bounded.evidence[1].configuration.external_raw_output;
  assert.equal(sha(fs.readFileSync(path.join(external,'acceptance.json'))),
    older['verification/scase02/corrected-acceptance.json']);
  const artifact=JSON.parse(read('verification/scase07/acceptance.json'));
  const corresponding=safety.cases.find(row=>row.id==='S-CASE-07');
  assert.deepEqual([artifact.schema,artifact.kind,artifact.result],
    ['noble-scase07-artifact-correspondence/v1','test','passed']);
  assert.deepEqual(corresponding.state,{implementation:'implemented',
    execution:'passed',proof:'open',trust:'explicit'});
  assert.equal(corresponding.evidence.length,1);
  assert.deepEqual(Object.fromEntries(
    ['id','profile','kind','requirements','input','expected']
      .map(key=>[key,corresponding[key]])),artifact.case);
  assert.equal(corresponding.evidence[0].configuration.receipt_sha256,
    older['verification/scase07/acceptance.json']);
  assert.equal(corresponding.evidence[0].source_revision,artifact.source_revision);
  assert.equal(corresponding.evidence[0].configuration.prepromotion_case_sha256,
    artifact.prepromotion_case_sha256);
  assert.equal(sha(fs.readFileSync(path.join(artifact.external_raw_output,'acceptance.json'))),
    older['verification/scase07/acceptance.json']);
  const quota=JSON.parse(read('verification/scase13/corrected-acceptance.json'));
  const limited=safety.cases.find(row=>row.id==='S-CASE-13');
  assert.deepEqual([quota.schema,quota.kind,quota.result],
    ['noble-scase13-runtime-quotas/v2','test','passed']);
  assert.deepEqual(limited.state,{implementation:'implemented',
    execution:'passed',proof:'open',trust:'explicit'});
  assert.equal(limited.evidence.length,2);
  assert.deepEqual({id:limited.id,input:limited.input,expected:limited.expected},quota.case);
  assert.equal(limited.evidence[0].configuration.receipt_sha256,
    older['verification/scase13/acceptance.json']);
  assert.equal(limited.evidence[1].configuration.receipt_sha256,
    older['verification/scase13/corrected-acceptance.json']);
  assert.equal(limited.evidence[1].source_revision,quota.source_revision);
  const quotaExternal=limited.evidence[1].configuration.external_raw_output;
  assert.equal(sha(fs.readFileSync(path.join(quotaExternal,'prepromotion-safety-cases.json'))),
    quota.prepromotion_case_sha256);
  assert.equal(sha(fs.readFileSync(path.join(quotaExternal,'acceptance.json'))),
    older['verification/scase13/corrected-acceptance.json']);
  const mismatch=quota.commands.find(row=>row.label==='reject-unmatched-source-recipe');
  assert.equal(mismatch.status,2);
  assert.equal(fs.readFileSync(path.join(quotaExternal,mismatch.stdout)).length,0);
  assert.equal(fs.readFileSync(path.join(quotaExternal,mismatch.stderr),'utf8').trim(),
    'quota-core: Core artifact does not match the independently compiled Noble source recipe');
  const authorization=JSON.parse(read('verification/scase06/final-acceptance.json'));
  const granted=safety.cases.find(row=>row.id==='S-CASE-06');
  assert.deepEqual([authorization.schema,authorization.result,authorization.failures,
    authorization.integrity_failures],
    ['noble-scase06-authorized-fs-final/v1','passed',[],[]]);
  assert.deepEqual({id:granted.id,input:granted.input,expected:granted.expected},
    authorization.case);
  assert.deepEqual(granted.state,{implementation:'implemented',execution:'passed',
    proof:'open',trust:'explicit'});
  assert.equal(granted.evidence.length,3);
  assert.deepEqual(granted.evidence.map(row=>row.configuration.receipt_sha256),[
    older['verification/scase06/acceptance.json'],
    older['verification/scase06/corrected-acceptance.json'],
    older['verification/scase06/final-acceptance.json']]);
  assert.equal(granted.evidence[2].source_revision,authorization.source_revision);
  assert.equal(granted.evidence[2].configuration.prepromotion_case_sha256,
    authorization.prepromotion_case_sha256);
  const fsExternal=granted.evidence[2].configuration.external_raw_output;
  assert.equal(sha(fs.readFileSync(path.join(fsExternal,'acceptance.json'))),
    older['verification/scase06/final-acceptance.json']);
  assert.deepEqual(authorization.observations.map(item=>
    [item.label,item.observed.outcome,item.observed.protected_operations]),[
      ['canonical-scase06','denied',0],
      ['same-compiled-guest-positive','read',1]]);
  assert.equal(authorization.commands.find(item=>
    item.label==='mismatched-host-source-refusal').status,2);
  fsChangeSources();
  const resource=JSON.parse(read('verification/scase04/acceptance.json'));
  const eligibility=safety.cases.find(row=>row.id==='S-CASE-04');
  assert.deepEqual([resource.schema,resource.kind,resource.result,
    resource.failures,resource.integrity_failures],
    ['noble-scase04-recursive-resource-eligibility/v1','test','passed',[],[]]);
  assert.deepEqual({id:eligibility.id,input:eligibility.input,expected:eligibility.expected},
    resource.case);
  assert.deepEqual(eligibility.state,{implementation:'implemented',
    execution:'passed',proof:'open',trust:'explicit'});
  assert.equal(eligibility.evidence.length,1);
  assert.equal(eligibility.evidence[0].configuration.receipt_sha256,
    older['verification/scase04/acceptance.json']);
  assert.equal(eligibility.evidence[0].source_revision,resource.source_revision);
  assert.equal(eligibility.evidence[0].configuration.prepromotion_case_sha256,
    resource.prepromotion_case_sha256);
  assert.deepEqual([resource.canonical_mapping.stage,resource.canonical_mapping.outcome,
    resource.canonical_mapping.guest_requests,resource.canonical_mapping.protected_operations],
    ['check','eligibility-reject',0,0]);
  assert.equal(resource.typed.negatives.length,16);
  assert.equal(sha(fs.readFileSync(path.join(resource.external_raw_output,'acceptance.json'))),
    older['verification/scase04/acceptance.json']);
  resourceChangeSources();
  const callback=JSON.parse(read('verification/scase08/acceptance.json'));
  const returned=safety.cases.find(row=>row.id==='S-CASE-08');
  assert.deepEqual([callback.schema,callback.kind,callback.result,
    callback.failures,callback.integrity_failures],
    ['noble-scase08-callback-owner/v1','test','passed',[],[]]);
  assert.deepEqual({id:returned.id,input:returned.input,expected:returned.expected},
    callback.case);
  assert.deepEqual(returned.state,{implementation:'implemented',
    execution:'passed',proof:'open',trust:'explicit'});
  assert.equal(returned.evidence.length,1);
  assert.equal(returned.evidence[0].configuration.receipt_sha256,
    older['verification/scase08/acceptance.json']);
  assert.equal(returned.evidence[0].source_revision,callback.source_revision);
  assert.equal(returned.evidence[0].configuration.prepromotion_case_sha256,
    callback.prepromotion_case_sha256);
  assert.equal(sha(fs.readFileSync(path.join(
    returned.evidence[0].configuration.external_raw_output,'acceptance.json'))),
    older['verification/scase08/acceptance.json']);
  assert.deepEqual(callback.observations.map(row=>[
    row.mode,row.report.stage,row.report.outcome,row.report.guest_requests,
    row.report.trusted_values_created,row.report.protected_operations,
    row.report.released_owners]),[
      ['authentic','callback','accepted',2,1,1,1],
      ['stale-generation','callback','invalid-result',1,0,0,1],
      ['wrong-kind','callback','invalid-result',1,0,0,1],
      ['wrong-context','callback','invalid-result',1,0,0,1]]);
  assert.deepEqual(callback.historical_receipt_sha256,
    Object.fromEntries(Object.entries(older)
      .filter(([file])=>Object.hasOwn(callback.historical_receipt_sha256,file))));
  assert.equal(callback.prepromotion_tasks_sha256,callback.source_sha256[callbackTasks]);
  verifyAcceptedCallbackTaskProse(callback.prepromotion_tasks_sha256);
  callbackChangeSources();
  for(const [file,digest] of Object.entries(callback.source_sha256)) {
    if(file!=='specs/conformance/safety-cases.json'&&
      file!=='.cairn/specs/safety/spec.md'&&
      file!=='.cairn/specs/resource-adapters/spec.md'&&
      file!=='specs/SAFETY.md'&&
      file!==callbackTasks)
      assert.equal(sourceDigest(file),digest,`S-CASE-08 feature source changed: ${file}`);
  }
  const dx=caseRow('DX-06');
  assert.ok(dx.evidence.length===8||dx.evidence.length===9);
  assert.equal(dx.evidence[7].configuration.receipt_sha256,
    older['verification/scase04-current-source/dx06-acceptance.json']);
  for(const id of selectedCases) {
    const row=caseRow(id);
    assert.ok(row.evidence.length===9||row.evidence.length===10,`${id} prior evidence`);
    assert.equal(row.evidence[8].configuration.receipt_sha256,
      older['verification/scase04-current-source/acceptance.json']);
  }
  return {historical,editor,forged,initial,bounded,accepted,artifact,quota,authorization,
    resource,callback};
}
export function inventory(mode,editor,forged) {
  assert.ok(mode==='dx06'||mode==='replay');
  const callback=JSON.parse(read('verification/scase08/acceptance.json'));
  const input=[...precedingInputs('replay',editor,forged),...Object.keys(older),
    ...Object.keys(callback.source_sha256),
    ...quotaChangeSources(),
    ...fsChangeSources(),
    ...resourceChangeSources(),
    ...callbackChangeSources(),
    ...own,...inherited,
    'verification/scase02/gate.mjs',
    'verification/scase02/corrected-gate.mjs',
    'verification/scase02/corrected-postpromotion.mjs',
    'verification/scase07/gate.mjs',
    'verification/scase07/postpromotion.mjs',
    'verification/scase13/gate.mjs',
    'verification/scase13/postpromotion.mjs',
    'verification/scase13/corrected-gate.mjs',
    'verification/scase13/corrected-postpromotion.mjs',
    'verification/scase06/gate.mjs',
    'verification/scase06/postpromotion.mjs',
    'verification/scase06/corrected-gate.mjs',
    'verification/scase06/corrected-postpromotion.mjs',
    'verification/scase06/final-gate.mjs',
    'verification/scase06/final-postpromotion.mjs',
    'verification/scase04/gate.mjs',
    'verification/scase04/postpromotion.mjs',
    'verification/scase08/gate.mjs',
    '.cairn/specs/resource-adapters/spec.md',
    '.cairn/specs/evidence/spec.md',
    ...(mode==='replay'?[dxReceipt]:[])];
  const sources=sourceInventory([...new Set(input)],trees);
  sources[fsTasks]=sourceDigest(fsTasks);
  sources[resourceTasks]=sourceDigest(resourceTasks);
  sources[callbackTasks]=sourceDigest(callbackTasks);
  return sources;
}
