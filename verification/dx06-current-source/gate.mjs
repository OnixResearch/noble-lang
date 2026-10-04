#!/usr/bin/env node
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { root,read,sha,caseFiles,caseRow,acceptedReceipt,historic,historicalReceipts,
  selectedOldRows,sourceInventory,dx06InputFiles,revision,frozen,freshOutput,
  commandsAt,verifyRecordedCommands }
  from './source.mjs';

const runner='verification/dx06-current-source/gate.mjs';
assert.equal(fileURLToPath(import.meta.url),path.join(root,runner));
const old=historic();
selectedOldRows(old);
const planning=process.argv.length===3 && process.argv[2]==='--plan';
if(planning) {
  const missing=[];
  for(const id of ['DX-02','ADAPT-06']) {
    try { acceptedReceipt(id); }
    catch(error) { missing.push({case_id:id,reason:String(error.message)}); }
  }
  if(missing.length) {
    console.log(JSON.stringify({result:'blocked-not-execution',missing,
      requires:['both initial receipts and canonical promotions',
        'final kernel/host source freeze before reviewed revision']}));
    process.exit(0);
  }
}
const editor=acceptedReceipt('DX-02'), forged=acceptedReceipt('ADAPT-06');
const dx06=caseRow('DX-06');
assert.deepEqual(dx06.state,{implementation:'implemented',execution:'passed',proof:'open',trust:'explicit'});
assert.equal(dx06.evidence.length,1,'DX-06 has exactly its earlier accepted observation');
assert.equal(dx06.evidence[0].configuration.receipt_sha256,
  historicalReceipts['verification/dx06/acceptance.json']);
assert.equal(dx06.evidence[0].source_revision,
  old['verification/dx06/acceptance.json'].source_revision);
assert.deepEqual(dx06.input,{harness:'explicit-scripted-test-host',operation:'test.clock',
  script:[42],variants:['authorized-matching-interface','denied','missing-mapping',
    'incompatible-interface','second-request-script-exhausted','unexpected-operation']});
assert.deepEqual(dx06.expected,{stage:'adapter',
  outcome:'authorized-returns-42-all-other-variants-fail-explicitly',
  real_host_fallback_calls:0,declared_effect_preserved:true,denied_request_recorded:true,
  report_fields:['substitutions','adapter_versions','scripted_inputs','host_requests'],
  real_host_evidence:false});
const sources=sourceInventory(dx06InputFiles(editor,forged));
const sourceRevision=revision(sources);
if(planning) {
  console.log(JSON.stringify({result:'ready-for-owner-review-not-execution',
    source_revision:sourceRevision,source_files:Object.keys(sources).length,
    cases:['DX-02','ADAPT-06','DX-06'],selected_rust:JSON.parse(read('policy/tool-selection.json')).tool_paths.quality_rust,
    prior_receipts:Object.keys(historicalReceipts),
    requires:['both evidence owners declare final source freeze',
      'new external directory and separately reviewed source revision']}));
  process.exit(0);
}
assert.equal(process.argv.length,4,
  'usage: bun verification/dx06-current-source/gate.mjs NEW_EXTERNAL_DIRECTORY sha256:REVIEWED_FROZEN_SOURCE_REVISION');
assert.match(process.argv[3],/^sha256:[0-9a-f]{64}$/);
assert.equal(process.argv[3],sourceRevision,'source differs from separately reviewed freeze');
const artifacts=freshOutput(process.argv[2]);
const {run,build,artifact,commands,selected}=commandsAt(artifacts);
const built=build();
const cli=artifact(built,'noble','bin');
const node=JSON.parse(read('crates/noble-cli/src/core/runtime/config.json')).tools.node.path;
const smokeDirectory=path.join(artifacts,'smoke');
const smokeOutput=run('six-control-compiled-test-clock',node,
  ['verification/dx06/gate.mjs',cli,smokeDirectory]);
const printed=JSON.parse(smokeOutput.trim());
const smokeBytes=fs.readFileSync(path.join(smokeDirectory,'smoke.json'));
const smoke=JSON.parse(smokeBytes);
assert.deepEqual(printed,smoke,'printed observations must match retained raw smoke');
assert.equal(smoke.result,'six-controls-pass');
assert.equal(smoke.case_id,'DX-06');
assert.equal(smoke.source_root,root);
assert.equal(smoke.pre_promotion_case_sha256,
  sources['specs/conformance/developer-experience-cases.json']);
assert.equal(smoke.production_cli_sha256,sha(fs.readFileSync(cli)));
assert.equal(smoke.selected_node_sha256,sha(fs.readFileSync(node)));
assert.deepEqual(smoke.latent_effect_witness,['test.clock']);
assert.equal(smoke.real_host_evidence,false);
assert.equal(smoke.real_host_fallback_calls,0);
assert.deepEqual(Object.keys(smoke.controls),dx06.input.variants);
assert.deepEqual(smoke.controls['authorized-matching-interface'],{outcome:'normal',value:'42'});
assert.deepEqual([smoke.controls.denied.outcome,smoke.controls.denied.request.decision,
  smoke.controls.denied.request.reason],['trap','deny','policy-denied']);
assert.equal(smoke.controls['missing-mapping'].outcome,'reject');
assert.equal(smoke.controls['incompatible-interface'].outcome,'invalid-input');
assert.deepEqual(smoke.controls['second-request-script-exhausted'].requests
  .map(row=>[row.decision,row.reason??null]),
  [['allow',null],['deny','script-exhausted']]);
assert.deepEqual([smoke.controls['unexpected-operation'].outcome,
  smoke.controls['unexpected-operation'].request.decision,
  smoke.controls['unexpected-operation'].request.reason],
  ['trap','deny','unexpected-operation']);
assert.equal(smoke.commands.length,6);
for(const [file,digest] of Object.entries(historicalReceipts))
  assert.equal(sha(read(file)),digest,`historical receipt changed: ${file}`);
frozen(sources);
verifyRecordedCommands(commands,artifacts);
const receipt={schema:'noble-dx06-current-source/v1',result:'passed',kind:'test',
  case:{id:dx06.id,input:dx06.input,expected:dx06.expected,
    new_claim:'Six exact compiled test.clock controls and latent effect reflection replayed against the renewed frozen DX-02/ADAPT-06 production source; no real clock or universal host proof.',
    previous_evidence:dx06.evidence[0]},
  source_revision:sourceRevision,source_sha256:sources,
  prepromotion_case_sha256:Object.fromEntries([...new Set(Object.values(caseFiles))]
    .map(file=>[file,sources[file]])),
  selected_rust:selected.tool_paths.quality_rust,
  binary:{path:cli,sha256:sha(fs.readFileSync(cli))},
  selected_node:{path:node,sha256:sha(fs.readFileSync(node))},
  historical_receipt_sha256:historicalReceipts,
  prerequisites:{'DX-02':{receipt:editor.receiptFile,sha256:editor.receiptSha256,
    source_revision:editor.evidence.source_revision},
    'ADAPT-06':{receipt:forged.receiptFile,sha256:forged.receiptSha256,
      source_revision:forged.evidence.source_revision}},
  smoke:{directory:smokeDirectory,file:'smoke.json',sha256:sha(smokeBytes),
    original_source_revision:smoke.source_revision,
    compiled_wasm_sha256:smoke.compiled_wasm_sha256,
    retained:smoke.retained,controls:smoke.controls,
    latent_effect_witness:smoke.latent_effect_witness},
  commands,assumptions:[
    'Selected Rust/Cargo, Node/V8, wasm-tools, production CLI/host/compiler and the separately supplied finite test policy are trusted only for these receipt-bound exact bytes and six controls.',
    'The DX-02 and ADAPT-06 case receipts and earlier DX-06/current-source-replay receipts retain separate historical source revisions; no real clock, universal host/backend correspondence or Lean refinement is established.'
  ]};
fs.writeFileSync(path.join(artifacts,'acceptance.json'),JSON.stringify(receipt,null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({result:'passed',receipt:path.join(artifacts,'acceptance.json'),
  source_revision:sourceRevision,commands:commands.length,cases:['DX-06'],
  failures:0,integrity_failures:0}));
