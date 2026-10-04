#!/usr/bin/env node
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { root,read,sha,caseFiles,caseRow,acceptedReceipt,selectedCases,
  historicalReceipts,historic,selectedOldRows,sourceInventory,replayInputFiles,
  revision,frozen,freshOutput,commandsAt,verifyRecordedCommands }
  from '../dx06-current-source/source.mjs';
import { verifyDx06Renewal } from '../dx06-current-source/postpromotion.mjs';

const runner='verification/current-source-replay-dx02/gate.mjs';
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
  if(missing.length===0) {
    try { verifyDx06Renewal(); }
    catch(error) { missing.push({case_id:'DX-06-current',reason:String(error.message)}); }
  }
  if(missing.length) {
    console.log(JSON.stringify({result:'blocked-not-execution',missing,
      requires:['DX-02, ADAPT-06 and renewed DX-06 immutable receipts and canonical promotions',
        'final kernel/host source freeze before reviewed revision']}));
    process.exit(0);
  }
}
const editor=acceptedReceipt('DX-02'),forged=acceptedReceipt('ADAPT-06');
const renewedDx06=verifyDx06Renewal();
const newDx06Receipt='verification/dx06-current-source/acceptance.json';
const newDx06Bytes=read(newDx06Receipt);
assert.equal(sha(newDx06Bytes),renewedDx06.receipt_sha256);
const packets=Object.fromEntries([...new Set(selectedCases.map(id=>caseFiles[id]))]
  .map(file=>[file,JSON.parse(read(file))]));
const cases=Object.fromEntries(selectedCases.map(id=>[id,caseRow(id)]));
const prior=old['verification/current-source-replay/acceptance.json'];
const oldDx=old['verification/dx01/current-frozen.json'];
const oldAdapt=old['verification/adapt-01/current-frozen.json'];
const sources=sourceInventory(replayInputFiles(editor,forged));
const sourceRevision=revision(sources);
if(planning) {
  console.log(JSON.stringify({result:'ready-for-owner-review-not-execution',
    source_revision:sourceRevision,source_files:Object.keys(sources).length,
    cases:selectedCases,selected_rust:JSON.parse(read('policy/tool-selection.json')).tool_paths.quality_rust,
    requires:['DX-02, ADAPT-06 and DX-06 renewal accepted and promoted',
      'kernel and host source freeze',
      'new external directory and separately reviewed source revision']}));
  process.exit(0);
}
assert.equal(process.argv.length,4,
  'usage: bun verification/current-source-replay-dx02/gate.mjs NEW_EXTERNAL_DIRECTORY sha256:REVIEWED_FROZEN_SOURCE_REVISION');
assert.match(process.argv[3],/^sha256:[0-9a-f]{64}$/);
assert.equal(process.argv[3],sourceRevision,'source differs from separately reviewed freeze');
const artifacts=freshOutput(process.argv[2]);
const inputs=path.join(artifacts,'inputs');
fs.mkdirSync(inputs);
for(const file of Object.keys(packets))
  fs.writeFileSync(path.join(inputs,path.basename(file)),read(file),{flag:'wx'});
const {run,build,artifact,commands,environment,selected,rust}=commandsAt(artifacts);
const inputFile=(name,source)=>{
  const file=path.join(inputs,`${name}.noble`);
  fs.writeFileSync(file,source,{flag:'wx'});
  return file;
};
const safetyInputs=[inputFile('S-CASE-01',cases['S-CASE-01'].input.source),
  inputFile('S-CASE-05',cases['S-CASE-05'].input.source),
  ...cases['S-CASE-14'].input.sources.map((source,i)=>inputFile(`S-CASE-14-${i}`,source))];
assert.equal(safetyInputs.length,4);
const built=build();
const cli=artifact(built,'noble','bin');
const contracts=artifact(built,'noble_contracts','lib');
const kernel=artifact(built,'noble_kernel','lib');
function compilePeer(name,file,extern) {
  const output=path.join(artifacts,name);
  run(`build-${name}`,path.join(rust,'rustc'),['--edition=2021',
    '--crate-name',name.replaceAll('-','_'),file,
    ...extern.flatMap(([key,library])=>['--extern',`${key}=${library}`]),
    '-L',`dependency=${path.join(environment.CARGO_TARGET_DIR,'debug/deps')}`,'-o',output]);
  return output;
}
const diagnosticPeer=compilePeer('diagnostic-check',
  'verification/current-source-replay/diagnostic-check.rs',
  [['noble_contracts',contracts],['noble_kernel',kernel]]);
const safetyPeer=compilePeer('safety-core-check','verification/safety-core/check.rs',
  [['noble_contracts',contracts],['noble_kernel',kernel]]);
const handlePeer=compilePeer('safety-handle-check','verification/safety-core/handle-check.rs',
  [['noble_kernel',kernel]]);
const reportLines=(label,argv,stdin,status,length)=>{
  const output=run(label,cli,argv,status,Buffer.from(stdin));
  const reports=output.trim().split('\n').map(line=>JSON.parse(line));
  assert.equal(reports.length,length,`${label} report count`);
  return reports;
};
const fields=(observed,expected,label)=>{
  for(const [field,value] of Object.entries(expected))
    assert.deepEqual(observed[field],value,`${label}.${field}`);
};
const dxReports=reportLines('dx01-session',['session'],oldDx.cli_run.stdin_utf8,
  oldDx.cli_run.exit_code,2);
for(let i=0;i<dxReports.length;i++) {
  const {source,failure,...expected}=oldDx.cli_run.reports[i];
  assert.equal(source,cases['DX-01'].input.variants[i].source);
  assert.equal(failure,cases['DX-01'].input.variants[i].failure);
  fields(dxReports[i],expected,`DX-01/${failure}`);
}
const typed=JSON.parse(run('dx01-typed-resource',diagnosticPeer,
  [cases['DX-01'].input.variants[2].source]).trim());
const {path:apiPath,resource_identity,resource_kind,operation_observation,
  ...typedExpected}=oldDx.typed_source_check;
assert.equal(apiPath,'source::ModuleSession::new([]).prepare(source, [Ty::Resource(FIXTURE_RESOURCE)], limits)');
assert.equal(resource_kind,'ResourceKind(0)');
assert.ok(resource_identity.includes('FIXTURE_RESOURCE'));
assert.ok(operation_observation.includes('before producing a candidate'));
fields(typed,typedExpected,'DX-01/typed-resource');
const adaptReports=reportLines('adapt01-session',['session'],oldAdapt.rejection_run.stdin_utf8,
  oldAdapt.rejection_run.exit_code,3);
for(let i=0;i<adaptReports.length;i++) {
  const {source,failure,...expected}=oldAdapt.rejection_run.reports[i];
  assert.equal(source,cases['ADAPT-01'].input.variants[i].source);
  assert.equal(failure,cases['ADAPT-01'].input.variants[i].failure);
  fields(adaptReports[i],expected,`ADAPT-01/${failure}`);
}
const positive=reportLines('adapt01-compatible',['session'],
  oldAdapt.compatible_control.stdin_utf8,oldAdapt.compatible_control.exit_code,1)[0];
for(const field of ['stage','outcome','stack','guest_requests','protected_operations'])
  assert.deepEqual(positive[field],oldAdapt.compatible_control[field]);
const safetyReports=run('safety-source-and-kernel',safetyPeer,safetyInputs)
  .trim().split('\n').map(line=>JSON.parse(line));
assert.deepEqual(safetyReports.map(row=>row.id),['S-CASE-01','S-CASE-05','S-CASE-14']);
for(const observed of safetyReports) fields(observed,cases[observed.id].expected,observed.id);
assert.equal(safetyReports[1].constraint,'EffectInclusion(test.emit)');
assert.equal(safetyReports[2].variants,cases['S-CASE-14'].input.sources.length);
const safetyCli=[];
for(const [label,file,design] of [
  ['S-CASE-01',safetyInputs[0],cases['S-CASE-01']],
  ['S-CASE-14-0',safetyInputs[2],cases['S-CASE-14']],
  ['S-CASE-14-1',safetyInputs[3],cases['S-CASE-14']],
]) {
  const report=JSON.parse(run(`cli-${label}`,cli,['compile',file],2).trim());
  fields(report,design.expected,label);
  assert.equal(report.candidate_prepare_requests,0);
  safetyCli.push({id:label,stage:report.stage,outcome:report.outcome,
    guest_requests:report.guest_requests,protected_operations:report.protected_operations});
}
const handle=JSON.parse(run('safety-handle-forgery',handlePeer,
  [String(cases['S-CASE-03'].input.representation)]).trim());
fields(handle,cases['S-CASE-03'].expected,'S-CASE-03');
assert.equal(handle.positive_registered_owner,true);
assert.equal(handle.absent_after_registration,true);
for(const [file,digest] of Object.entries(historicalReceipts))
  assert.equal(sha(read(file)),digest,`historical receipt changed: ${file}`);
assert.equal(sha(read(newDx06Receipt)),renewedDx06.receipt_sha256);
frozen(sources);
verifyRecordedCommands(commands,artifacts);
const claims={
  'DX-01':'Later frozen source confirms both exact compiled CLI word/branch diagnostics and the independent typed Resource Data-eligibility refusal before execution; universal proof remains open.',
  'ADAPT-01':'Later frozen source confirms three exact compiled CLI branch-join refusals and one compatible typed branch; universal checker proof remains open.',
  'S-CASE-01':'Later frozen source confirms the exact ill-typed source rejects in compiled CLI and independent production source/kernel peer before execution.',
  'S-CASE-03':'Later frozen source confirms absent handle slot 42 rejects in production resource Table while one trusted registration succeeds; general non-forgeability remains open.',
  'S-CASE-05':'Later frozen source independently rejects the production-typed test.emit candidate against an empty allowed-effect bound before execution.',
  'S-CASE-14':'Later frozen source confirms both exact unsafe builtin names remain unbound in compiled CLI and independent source/kernel peer.',
};
const receipt={schema:'noble-current-source-replay-dx02/v1',result:'passed',kind:'test',
  cases:selectedCases.map(id=>({id,input:cases[id].input,expected:cases[id].expected,
    new_claim:claims[id],
    original_receipt:prior.cases.find(row=>row.id===id).previous_receipt,
    previous_evidence_sha256:sha(JSON.stringify(cases[id].evidence))})),
  claim:'Six exact selected diagnostic, branch, type, handle, effect and unbound-word cases re-executed on one later frozen DX-02/ADAPT-06/DX-06 source revision. Historical receipts and both earlier observations per case remain immutable, and general proof remains open.',
  source_revision:sourceRevision,source_sha256:sources,
  prepromotion_case_sha256:Object.fromEntries(Object.keys(packets)
    .map(file=>[file,sources[file]])),
  historical_receipt_sha256:historicalReceipts,
  prerequisites:{'DX-02':{receipt:editor.receiptFile,sha256:editor.receiptSha256,
    source_revision:editor.evidence.source_revision},
    'ADAPT-06':{receipt:forged.receiptFile,sha256:forged.receiptSha256,
      source_revision:forged.evidence.source_revision},
    'DX-06-current':{receipt:newDx06Receipt,sha256:renewedDx06.receipt_sha256,
      source_revision:renewedDx06.source_revision}},
  selected_rust:selected.tool_paths.quality_rust,
  binary:{path:cli,sha256:sha(fs.readFileSync(cli))},
  peers:Object.fromEntries([diagnosticPeer,safetyPeer,handlePeer]
    .map(file=>[path.basename(file),{path:file,sha256:sha(fs.readFileSync(file))}])),
  commands,observed:{dx01_cli:dxReports,dx01_typed:typed,adapt01_cli:adaptReports,
    adapt01_compatible:positive,safety_independent:safetyReports,safety_cli:safetyCli,
    safety_handle:handle},
  assumptions:[
    'Pinned Rust/Cargo, freshly compiled CLI and independent production source/kernel and resource Table paths are trusted only for their recorded exact bytes and finite tests.',
    'Negative paths do not execute guest work; the handle case checks one absent slot and trusted registration, not universal native memory or non-forgeability.',
    'DX-02, ADAPT-06, renewed DX-06 and historical six-case receipts remain separate source-scoped evidence; general Lean, host, backend and whole-source refinement remain open.'
  ]};
fs.writeFileSync(path.join(artifacts,'acceptance.json'),JSON.stringify(receipt,null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({result:'passed',receipt:path.join(artifacts,'acceptance.json'),
  source_revision:sourceRevision,cases:selectedCases,commands:commands.length,
  failures:0,integrity_failures:0}));
