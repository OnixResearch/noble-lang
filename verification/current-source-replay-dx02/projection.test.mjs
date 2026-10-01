import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { test } from 'node:test';
import { sha } from '../dx06-current-source/source.mjs';
import { restoreNewestReplayEvidence } from './projection.mjs';

const file='specs/conformance/developer-experience-cases.json';
const parent=path.join(os.homedir(),'.cache','noble-proof-tmp');
const first={kind:'test',subject:'DX-01',configuration:{receipt:'../../verification/dx01/current-frozen.json'}};
const second={kind:'test',subject:'DX-01',configuration:{
  receipt:'../../verification/current-source-replay/acceptance.json',
  receipt_sha256:'20c407e546759402c804824c9b40037a8163e93e8cd22b416c10a56cb8330b1a'}};
const prior=[first,second];
test('third observation preserves the complete two-receipt preimage and refuses edits',()=>{
  fs.mkdirSync(parent,{recursive:true});
  const external=fs.mkdtempSync(path.join(parent,'current-replay-projection-'));
  try {
    const before=`{"revision":"draft","cases":[{"id":"DX-01","input":{"source":"a,]"},"expected":{"outcome":"reject"},"evidence":[${prior.map(JSON.stringify).join(', ')}]}],"unrelated":"frozen"}\n`;
    const receipt={schema:'noble-current-source-replay-dx02/v1',result:'passed',
      source_revision:'sha256:source',binary:{sha256:'binary'},
      assumptions:['finite source-bound observation'],
      prepromotion_case_sha256:{[file]:sha(before)},
      cases:[{id:'DX-01',input:{source:'a,]'},expected:{outcome:'reject'},
        new_claim:'new finite source observation',
        original_receipt:'verification/dx01/current-frozen.json',
        previous_evidence_sha256:sha(JSON.stringify(prior))}]};
    const receiptBytes=Buffer.from(JSON.stringify(receipt)+'\n');
    fs.writeFileSync(path.join(external,'acceptance.json'),receiptBytes);
    const third={kind:'test',subject:'DX-01',claim:'new finite source observation',
      revision:'draft',result:'passed',source_revision:receipt.source_revision,
      assumptions:receipt.assumptions,configuration:{
        receipt:'../../verification/current-source-replay-dx02/acceptance.json',
        case_id:'DX-01',receipt_sha256:sha(receiptBytes),external_raw_output:external,
        prepromotion_case_sha256:sha(before),binary_sha256:'binary'}};
    const promoted=before.replace(`${JSON.stringify(second)}]`,
      `${JSON.stringify(second)}, ${JSON.stringify(third)}]`);
    assert.equal(restoreNewestReplayEvidence(promoted,file,receiptBytes),before);
    const damaged=promoted.replace('"a,]"','"changed"');
    assert.throws(()=>restoreNewestReplayEvidence(damaged,file,receiptBytes),
      /earlier evidence or unrelated case bytes changed|strictly deep-equal/);
    const forged=promoted.replace('new finite source observation','changed claim');
    assert.throws(()=>restoreNewestReplayEvidence(forged,file,receiptBytes),
      /exact renewed observation claim/);
  } finally { fs.rmSync(external,{recursive:true,force:true}); }
});
