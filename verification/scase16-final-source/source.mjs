import assert from 'node:assert/strict';
import {root,read,sha,caseFiles,caseRow,selectedCases,historicalReceipts,
  sourceInventory,sourceTrees,revision,frozen,freshOutput,commandsAt,
  verifyRecordedCommands} from '../dx06-current-source/source.mjs';
import {older as initialOlder,prerequisites as originalPrerequisites,
  sourceFiles as previousInputs,compiledImportControls}
  from '../scase16-current-source/source.mjs';

export {root,read,sha,caseFiles,caseRow,selectedCases,revision,frozen,freshOutput,
  commandsAt,verifyRecordedCommands,historicalReceipts,compiledImportControls};
export const folder='verification/scase16-final-source';
export const dxReceipt=`${folder}/dx06-acceptance.json`;
export const replayReceipt=`${folder}/acceptance.json`;
export const older={...initialOlder,
  'verification/scase16-current-source/dx06-acceptance.json':
    '4385bd6a4b91bc745c94c9ee4ca65ec709ca53f886d0f962eb5a495a138e30a8',
  'verification/scase16-current-source/acceptance.json':
    '487ca0f71ea9740f7f032537cd59f3197c4085735dea9ff9681237b82aa8e77b'};
const own=['source.mjs','gate.mjs','postpromotion.mjs'].map(file=>`${folder}/${file}`);
const trees=[...sourceTrees,'.cairn/changes/safety-wasm-manifest-admission'];
export function prerequisites() {
  const before=originalPrerequisites();
  for(const [file,digest] of Object.entries(older))
    assert.equal(sha(read(file)),digest,`prior immutable receipt changed: ${file}`);
  const dx=caseRow('DX-06');
  assert.equal(dx.evidence.length,3);
  assert.equal(dx.evidence[2].configuration.receipt_sha256,
    older['verification/scase16-current-source/dx06-acceptance.json']);
  for(const id of selectedCases) {
    const row=caseRow(id);
    assert.equal(row.evidence.length,4,`${id} three earlier replays retained`);
    assert.equal(row.evidence[3].configuration.receipt_sha256,
      older['verification/scase16-current-source/acceptance.json']);
  }
  return before;
}
export function inventory(mode,editor,forged) {
  assert.ok(mode==='dx06'||mode==='replay');
  const files=[...new Set([...previousInputs('replay',editor,forged),
    'verification/scase16-current-source/acceptance.json',...own,
    ...(mode==='replay'?[dxReceipt]:[])])];
  return sourceInventory(files,trees);
}
