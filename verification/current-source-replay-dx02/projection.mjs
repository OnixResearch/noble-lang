import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { root,sha,selectedCases,caseFiles,removeNewestEvidence,
  historicalReceipts } from '../dx06-current-source/source.mjs';

export const receiptFile='verification/current-source-replay-dx02/acceptance.json';
const idsByFile=Object.fromEntries([...new Set(selectedCases.map(id=>caseFiles[id]))]
  .map(file=>[file,selectedCases.filter(id=>caseFiles[id]===file)]));

// Remove ONLY the new third record for each selected case; the frozen full
// prepromotion hash proves that both earlier observations and every other byte
// (including DX-02, ADAPT-06 and the renewed DX-06) are unchanged.
export function restoreNewestReplayEvidence(promoted,file,receiptBytes) {
  const ids=idsByFile[file];
  assert.ok(ids,`unselected replay case file: ${file}`);
  const packet=JSON.parse(promoted);
  const rows=ids.map(id=>{
    const matches=packet.cases.filter(row=>row.id===id);
    assert.equal(matches.length,1,`${id} unique canonical row`);
    return matches[0];
  });
  if(rows.every(row=>row.evidence.length===2)) return promoted;
  assert.ok(rows.every(row=>row.evidence.length===3),`${file} partial six-case promotion`);
  const bytes=receiptBytes??fs.readFileSync(path.join(root,receiptFile));
  const receipt=JSON.parse(bytes),digest=sha(bytes);
  assert.equal(receipt.schema,'noble-current-source-replay-dx02/v1');
  assert.equal(receipt.result,'passed');
  const external=rows[0].evidence[2].configuration.external_raw_output;
  assert.equal(sha(fs.readFileSync(path.join(external,'acceptance.json'))),digest,
    'external frozen replay differs from the portable receipt');
  for(const row of rows) {
    const record=receipt.cases.find(item=>item.id===row.id);
    assert.ok(record,`${row.id} omitted from renewal`);
    assert.deepEqual({id:row.id,input:row.input,expected:row.expected},
      {id:record.id,input:record.input,expected:record.expected});
    assert.equal(sha(JSON.stringify(row.evidence.slice(0,2))),record.previous_evidence_sha256,
      `${row.id} first two evidence objects changed`);
    assert.equal(row.evidence[0].configuration.receipt,
      path.posix.relative(path.posix.dirname(file),record.original_receipt));
    assert.equal(row.evidence[1].configuration.receipt,
      path.posix.relative(path.posix.dirname(file),
        'verification/current-source-replay/acceptance.json'));
    assert.equal(row.evidence[1].configuration.receipt_sha256,
      historicalReceipts['verification/current-source-replay/acceptance.json']);
    const evidence=row.evidence[2];
    assert.equal(evidence.subject,row.id);
    assert.equal(evidence.kind,'test');
    assert.equal(evidence.result,'passed');
    assert.equal(evidence.revision,packet.revision);
    assert.equal(evidence.claim,record.new_claim,`${row.id} exact renewed observation claim`);
    assert.equal(evidence.source_revision,receipt.source_revision);
    assert.deepEqual(evidence.assumptions,receipt.assumptions);
    assert.deepEqual(evidence.configuration,{
      receipt:path.posix.relative(path.posix.dirname(file),receiptFile),
      case_id:row.id,receipt_sha256:digest,external_raw_output:external,
      prepromotion_case_sha256:receipt.prepromotion_case_sha256[file],
      binary_sha256:receipt.binary.sha256,
    });
  }
  let restored=promoted;
  for(const id of [...ids].reverse()) restored=removeNewestEvidence(restored,id,3);
  assert.equal(sha(restored),receipt.prepromotion_case_sha256[file],
    `${file} earlier evidence or unrelated case bytes changed`);
  return restored;
}
