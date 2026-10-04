import assert from 'node:assert/strict';
import { test } from 'node:test';
import { removeNewestEvidence,sha,sourceInventory,sourceTrees,buildFiles } from './source.mjs';

const prior={claim:'old evidence [ , \\"quote\\"',configuration:{nested:[{x:',]'}]}};
const later={claim:'already accepted current source',assumptions:['proof remains open']};
const fresh={claim:'renewed compiler and host source',configuration:{rows:['a,b',']']}};
for(const count of [2,3]) test(`appended record ${count} restores exact old bytes`,()=>{
  const retained=[prior,later].slice(0,count-1);
  const original=`{"cases":[{"id":"DX-06","evidence":[${retained.map(JSON.stringify).join(', ')}]}],"end":"untouched"}\n`;
  const promoted=`{"cases":[{"id":"DX-06","evidence":[${[...retained,fresh].map(JSON.stringify).join(', ')}]}],"end":"untouched"}\n`;
  assert.equal(removeNewestEvidence(promoted,'DX-06',count),original);
  assert.equal(sha(removeNewestEvidence(promoted,'DX-06',count)),sha(original));
});
test('projection rejects wrong evidence count and ambiguous canonical IDs',()=>{
  const row=`{"id":"DX-06","evidence":[${JSON.stringify(prior)},${JSON.stringify(later)}]}`;
  assert.throws(()=>removeNewestEvidence(`{"cases":[${row}]}`,'DX-06',3),
    /evidence count/);
  assert.throws(()=>removeNewestEvidence(`{"cases":[${row},${row}]}`,'DX-06',2),
    /unique canonical identifier/);
});
test('production inventory covers compiled runtime and proof assets outside src',()=>{
  const files=sourceInventory(buildFiles,sourceTrees);
  for(const path of [
    'crates/noble-wasm/runtime/linear-storage.wat',
    'crates/noble-wasm/runtime/source.wat',
    'crates/noble-wasm/wit/bootstrap.wit',
    'proofs/mc1/IntrinsicTypeWitness.lean',
    'proofs/mc1/NobleContracts/NamedV2.lean',
    'verification/mc2/contracts/increment.contract',
  ]) assert.match(files[path]??'',/^[0-9a-f]{64}$/,`${path} source bound`);
});
