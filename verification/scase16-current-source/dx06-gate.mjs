#!/usr/bin/env node
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {root,read,sha,caseFiles,caseRow,older,prerequisites,inventory,revision,
  frozen,freshOutput,commandsAt,verifyRecordedCommands,compiledImportControls}
  from './source.mjs';

const {editor,forged,initial}=prerequisites();
const dx=caseRow('DX-06');
assert.equal(dx.evidence.length,2,'only the two historical DX06 observations may precede renewal');
const sources=inventory('dx06',editor,forged),sourceRevision=revision(sources);
if(process.argv.length===3 && process.argv[2]==='--plan') {
  console.log(JSON.stringify({result:'ready-for-owner-review-not-execution',
    source_revision:sourceRevision,source_files:Object.keys(sources).length,
    prerequisites:older,cases:['S-CASE-16','DX-06'],
    requires:'final frozen source including new runners and fresh external output'}));
  process.exit(0);
}
assert.equal(process.argv.length,4,
  'usage: bun verification/scase16-current-source/dx06-gate.mjs NEW_EXTERNAL_DIR sha256:REVIEWED_REVISION');
assert.equal(process.argv[3],sourceRevision,'source differs from separately reviewed freeze');
const artifacts=freshOutput(process.argv[2]);
const {run,build,artifact,commands,selected}=commandsAt(artifacts);
const cli=artifact(build(),'noble','bin');
const node=JSON.parse(read('crates/noble-cli/src/core/runtime/config.json')).tools.node.path;
const smokeDirectory=path.join(artifacts,'smoke');
const observed=JSON.parse(run('six-compiled-test-clock-controls',node,
  ['verification/dx06/gate.mjs',cli,smokeDirectory]).trim());
const smokeBytes=fs.readFileSync(path.join(smokeDirectory,'smoke.json'));
const smoke=JSON.parse(smokeBytes);
assert.deepEqual(observed,smoke);
assert.equal(smoke.result,'six-controls-pass');
assert.equal(smoke.case_id,'DX-06');
assert.equal(smoke.pre_promotion_case_sha256,
  sources[caseFiles['DX-06']]);
assert.deepEqual(Object.keys(smoke.controls),dx.input.variants);
assert.deepEqual(smoke.controls['authorized-matching-interface'],{outcome:'normal',value:'42'});
assert.deepEqual([smoke.controls.denied.outcome,smoke.controls.denied.request.decision,
  smoke.controls.denied.request.reason],['trap','deny','policy-denied']);
assert.equal(smoke.controls['missing-mapping'].outcome,'reject');
assert.equal(smoke.controls['incompatible-interface'].outcome,'invalid-input');
assert.deepEqual(smoke.controls['second-request-script-exhausted'].requests
  .map(item=>[item.decision,item.reason??null]),
  [['allow',null],['deny','script-exhausted']]);
assert.deepEqual([smoke.controls['unexpected-operation'].outcome,
  smoke.controls['unexpected-operation'].request.decision,
  smoke.controls['unexpected-operation'].request.reason],
  ['trap','deny','unexpected-operation']);
assert.deepEqual(smoke.latent_effect_witness,['test.clock']);
assert.equal(smoke.real_host_evidence,false);
assert.equal(smoke.real_host_fallback_calls,0);
assert.equal(smoke.production_cli_sha256,sha(fs.readFileSync(cli)));
assert.equal(smoke.selected_node_sha256,sha(fs.readFileSync(node)));
const conditional=compiledImportControls(artifacts,cli,run);
for(const [file,digest] of Object.entries(older)) assert.equal(sha(read(file)),digest);
assert.equal(initial.observed.refusals,23);
frozen(sources);
verifyRecordedCommands(commands,artifacts);
const newClaim='Later S-CASE-16-frozen source replays six compiled test.clock controls and latent reflection; separately compiled effectful test.emit and pure I64(3) artifacts preserve distinct actual import/admission behavior without real host fallback or universal proof.';
const receipt={schema:'noble-scase16-dx06-current-source/v1',result:'passed',kind:'test',
  case:{id:dx.id,input:dx.input,expected:dx.expected,new_claim:newClaim,
    previous_evidence_sha256:sha(JSON.stringify(dx.evidence))},
  source_revision:sourceRevision,source_sha256:sources,
  prepromotion_case_sha256:Object.fromEntries([...new Set(Object.values(caseFiles))]
    .map(file=>[file,sources[file]])),
  prior_receipt_sha256:older,
  prerequisites:{'S-CASE-16':{receipt:'verification/scase16/acceptance.json',
    sha256:older['verification/scase16/acceptance.json'],
    source_revision:`sha256:${initial.source_tree_sha256}`},
    'DX-02':{receipt:editor.receiptFile,sha256:editor.receiptSha256},
    'ADAPT-06':{receipt:forged.receiptFile,sha256:forged.receiptSha256}},
  selected_rust:selected.tool_paths.quality_rust,
  selected_node:{path:node,sha256:sha(fs.readFileSync(node))},
  binary:{path:cli,sha256:sha(fs.readFileSync(cli))},
  smoke:{directory:smokeDirectory,file:'smoke.json',sha256:sha(smokeBytes),
    original_source_revision:smoke.source_revision,
    compiled_wasm_sha256:smoke.compiled_wasm_sha256,
    retained:smoke.retained,controls:smoke.controls,
    latent_effect_witness:smoke.latent_effect_witness},
  conditional_imports:conditional,commands,
  assumptions:[
    'Pinned Rust/Cargo, Node/V8, wasm-tools, actual compiled Wasm, independent host-selected manifest and finite test policy are trusted only for these recorded source/binary bytes and controls.',
    'S-CASE-16 admission and earlier receipts retain separate immutable source scopes; neither a real clock, universal host/backend correspondence nor Rust-to-Lean refinement is established.'
  ]};
fs.writeFileSync(path.join(artifacts,'acceptance.json'),JSON.stringify(receipt,null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({result:'passed',receipt:path.join(artifacts,'acceptance.json'),
  source_revision:sourceRevision,commands:commands.length,cases:['DX-06'],
  failures:0,integrity_failures:0}));
