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
export const folder='verification/scase02-current-source';
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
    '3588522e32149a490ea0ec8b7c33abbcb229150b91e24a31d164a17b60a740d0'};
const own=['source.mjs','gate.mjs','projection.mjs','postpromotion.mjs']
  .map(file=>`${folder}/${file}`);
const inherited=['source.mjs','gate.mjs','postpromotion.mjs']
  .map(file=>`verification/scase16-final-source/${file}`);
const trees=[...sourceTrees,'.cairn/changes/safety-wasm-manifest-admission',
  '.cairn/changes/safety-bounded-region-adapter'];
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
  for(const [file,digest] of Object.entries(accepted.source_sha256)) {
    if(file!=='specs/conformance/safety-cases.json')
      assert.equal(sha(read(file)),digest,`corrected feature source changed: ${file}`);
  }
  const dx=caseRow('DX-06');
  assert.ok(dx.evidence.length===3||dx.evidence.length===4);
  assert.equal(dx.evidence[2].configuration.receipt_sha256,
    older['verification/scase16-current-source/dx06-acceptance.json']);
  for(const id of selectedCases) {
    const row=caseRow(id);
    assert.ok(row.evidence.length===4||row.evidence.length===5,`${id} prior evidence`);
    assert.equal(row.evidence[3].configuration.receipt_sha256,
      older['verification/scase16-current-source/acceptance.json']);
  }
  return {historical,editor,forged,initial,bounded,accepted};
}
export function inventory(mode,editor,forged) {
  assert.ok(mode==='dx06'||mode==='replay');
  const input=[...precedingInputs('replay',editor,forged),...Object.keys(older),
    ...own,...inherited,
    'verification/scase02/gate.mjs',
    'verification/scase02/corrected-gate.mjs',
    'verification/scase02/corrected-postpromotion.mjs',
    '.cairn/specs/resource-adapters/spec.md',
    ...(mode==='replay'?[dxReceipt]:[])];
  return sourceInventory([...new Set(input)],trees);
}
