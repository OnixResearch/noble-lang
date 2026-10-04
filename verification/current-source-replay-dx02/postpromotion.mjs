import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { root,read,sha,caseFiles,selectedCases,acceptedReceipt,
  sourceInventory,replayInputFiles,revision,historic,historicalReceipts }
  from '../dx06-current-source/source.mjs';
import { verifyDx06Renewal } from '../dx06-current-source/postpromotion.mjs';
import { receiptFile,restoreNewestReplayEvidence } from './projection.mjs';

const bytes=read(receiptFile),receipt=JSON.parse(bytes);
assert.equal(fs.realpathSync(process.cwd()),root);
assert.equal(receipt.schema,'noble-current-source-replay-dx02/v1');
assert.equal(receipt.kind,'test');
assert.equal(receipt.result,'passed');
assert.deepEqual(receipt.cases.map(row=>row.id),selectedCases);
assert.deepEqual(receipt.historical_receipt_sha256,historicalReceipts);
historic();
const editor=acceptedReceipt('DX-02'),forged=acceptedReceipt('ADAPT-06');
for(const [id,accepted] of [['DX-02',editor],['ADAPT-06',forged]])
  assert.deepEqual(receipt.prerequisites[id],{receipt:accepted.receiptFile,
    sha256:accepted.receiptSha256,source_revision:accepted.evidence.source_revision});
assert.equal(receipt.source_revision,revision(receipt.source_sha256));
const cases=[...new Set(selectedCases.map(id=>caseFiles[id]))];
assert.deepEqual(Object.keys(receipt.prepromotion_case_sha256).sort(),[...cases].sort());
assert.deepEqual(Object.keys(receipt.source_sha256).sort(),
  Object.keys(sourceInventory(replayInputFiles(editor,forged))).sort(),
  'source inventory paths changed');
const projected={};
for(const file of cases) {
  const promoted=read(file).toString('utf8');
  const restored=restoreNewestReplayEvidence(promoted,file,bytes);
  assert.notEqual(restored,promoted,`${file} missing new third evidence`);
  assert.equal(sha(restored),receipt.prepromotion_case_sha256[file]);
  projected[file]=restored;
}
assert.deepEqual(receipt.prerequisites['DX-06-current'],(()=>{
  const dx06=verifyDx06Renewal({caseTexts:projected});
  return {receipt:'verification/dx06-current-source/acceptance.json',
    sha256:dx06.receipt_sha256,source_revision:dx06.source_revision};
})());
for(const [file,digest] of Object.entries(receipt.source_sha256)) {
  if(!cases.includes(file)) assert.equal(sha(read(file)),digest,
    `renewed replay accepted source changed: ${file}`);
}
assert.equal(sha(fs.readFileSync(receipt.binary.path)),receipt.binary.sha256);
for(const peer of Object.values(receipt.peers))
  assert.equal(sha(fs.readFileSync(peer.path)),peer.sha256);
assert.deepEqual(receipt.commands.map(command=>command.label),[
  'build','build-diagnostic-check','build-safety-core-check','build-safety-handle-check',
  'dx01-session','dx01-typed-resource','adapt01-session','adapt01-compatible',
  'safety-source-and-kernel','cli-S-CASE-01','cli-S-CASE-14-0','cli-S-CASE-14-1',
  'safety-handle-forgery',
]);
const external=JSON.parse(read(cases[0])).cases.find(row=>row.id===selectedCases[0])
  .evidence[2].configuration.external_raw_output;
assert.equal(sha(fs.readFileSync(path.join(external,'acceptance.json'))),sha(bytes));
const refusals=new Set(['dx01-session','adapt01-session','cli-S-CASE-01',
  'cli-S-CASE-14-0','cli-S-CASE-14-1']);
for(const command of receipt.commands) {
  assert.equal(command.error,null);
  assert.equal(command.signal,null);
  assert.equal(command.status,refusals.has(command.label)?2:0,
    `${command.label} status`);
  assert.equal(sha(fs.readFileSync(command.executable)),command.executable_sha256);
  for(const kind of ['stdout','stderr'])
    assert.equal(sha(fs.readFileSync(path.join(external,command[kind]))),
      command[`${kind}_sha256`],`${command.label} ${kind}`);
}
console.log(JSON.stringify({result:'passed',cases:selectedCases,
  receipt_sha256:sha(bytes),source_revision:receipt.source_revision,
  exact_previous_case_files_verified:true,commands:receipt.commands.length,
  historical_receipts:'unchanged',proof:'open'}));
