import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {root,read,sha,caseFiles,caseRow,selectedCases,historic,acceptedReceipt,
  sourceInventory,sourceTrees,revision,frozen,freshOutput,
  commandsAt as baseCommandsAt,verifyRecordedCommands,removeNewestEvidence}
  from '../dx06-current-source/source.mjs';
import {sourceFiles as precedingInputs,compiledImportControls}
  from '../scase16-current-source/source.mjs';
import {older as priorReceipts} from '../scase16-final-source/source.mjs';

export {root,read,sha,caseFiles,caseRow,selectedCases,revision,frozen,
  freshOutput,verifyRecordedCommands,removeNewestEvidence,compiledImportControls};
export const folder='verification/scase13-current-source';
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
    'e8211d6d2b0c77ac540874a24dc5f8dedf3bd3773c60b1105975ad9b485fcf28'};
const own=['source.mjs','gate.mjs','projection.mjs','postpromotion.mjs']
  .map(file=>`${folder}/${file}`);
const inherited=[
  ...['source.mjs','gate.mjs','postpromotion.mjs']
    .map(file=>`verification/scase16-final-source/${file}`),
  ...['source.mjs','gate.mjs','projection.mjs','postpromotion.mjs']
    .map(file=>`verification/scase02-current-source/${file}`),
  ...['source.mjs','gate.mjs','projection.mjs','postpromotion.mjs']
    .map(file=>`verification/scase07-current-source/${file}`)];
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
  for(const [file,digest] of Object.entries(quota.source_sha256)) {
    if(file!=='specs/conformance/safety-cases.json')
      assert.equal(sha(read(file)),digest,`corrected quota source changed: ${file}`);
  }
  const dx=caseRow('DX-06');
  assert.ok(dx.evidence.length===5||dx.evidence.length===6);
  assert.equal(dx.evidence[4].configuration.receipt_sha256,
    older['verification/scase07-current-source/dx06-acceptance.json']);
  for(const id of selectedCases) {
    const row=caseRow(id);
    assert.ok(row.evidence.length===6||row.evidence.length===7,`${id} prior evidence`);
    assert.equal(row.evidence[5].configuration.receipt_sha256,
      older['verification/scase07-current-source/acceptance.json']);
  }
  return {historical,editor,forged,initial,bounded,accepted,artifact,quota};
}
export function inventory(mode,editor,forged) {
  assert.ok(mode==='dx06'||mode==='replay');
  const quota=JSON.parse(read('verification/scase13/corrected-acceptance.json'));
  const input=[...precedingInputs('replay',editor,forged),...Object.keys(older),
    ...Object.keys(quota.source_sha256),
    ...quotaChangeSources(),
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
    '.cairn/specs/resource-adapters/spec.md',
    ...(mode==='replay'?[dxReceipt]:[])];
  return sourceInventory([...new Set(input)],trees);
}
