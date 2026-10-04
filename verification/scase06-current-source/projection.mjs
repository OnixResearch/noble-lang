import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {root,sha,selectedCases,caseFiles,removeNewestEvidence,replayReceipt,older}
  from './source.mjs';

const idsByFile=Object.fromEntries([...new Set(selectedCases.map(id=>caseFiles[id]))]
  .map(file=>[file,selectedCases.filter(id=>caseFiles[id]===file)]));
export function restoreReplayEvidence(text,file,receiptBytes) {
  const ids=idsByFile[file];
  assert.ok(ids,`unselected case file: ${file}`);
  const packet=JSON.parse(text);
  const rows=ids.map(id=>{
    const matches=packet.cases.filter(row=>row.id===id);
    assert.equal(matches.length,1,`${id} unique canonical row`);
    return matches[0];
  });
  if(rows.every(row=>row.evidence.length===7)) return text;
  assert.ok(rows.every(row=>row.evidence.length===8),`${file} partial eighth-evidence promotion`);
  const bytes=receiptBytes??fs.readFileSync(path.join(root,replayReceipt));
  const receipt=JSON.parse(bytes),digest=sha(bytes);
  assert.equal(receipt.schema,'noble-scase06-active-replay/v1');
  assert.equal(receipt.result,'passed');
  const external=rows[0].evidence[7].configuration.external_raw_output;
  assert.equal(sha(fs.readFileSync(path.join(external,'acceptance.json'))),digest);
  for(const row of rows) {
    const record=receipt.cases.find(item=>item.id===row.id);
    assert.ok(record);
    assert.deepEqual({id:row.id,input:row.input,expected:row.expected},
      {id:record.id,input:record.input,expected:record.expected});
    assert.equal(sha(JSON.stringify(row.evidence.slice(0,7))),record.previous_evidence_sha256);
    assert.equal(row.evidence[6].configuration.receipt_sha256,
      older['verification/scase13-current-source/acceptance.json']);
    const evidence=row.evidence[7];
    assert.deepEqual(evidence,{
      kind:'test',subject:row.id,claim:record.new_claim,revision:packet.revision,
      source_revision:receipt.source_revision,result:'passed',
      configuration:{receipt:path.posix.relative(path.posix.dirname(file),replayReceipt),
        case_id:row.id,receipt_sha256:digest,external_raw_output:external,
        prepromotion_case_sha256:receipt.case_sha256[file],
        binary_sha256:receipt.binary.sha256},assumptions:receipt.assumptions});
  }
  let restored=text;
  for(const id of [...ids].reverse()) restored=removeNewestEvidence(restored,id,8);
  assert.equal(sha(restored),receipt.case_sha256[file],
    `${file} previous evidence or unrelated case bytes changed`);
  return restored;
}
