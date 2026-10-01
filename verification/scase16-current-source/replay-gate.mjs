#!/usr/bin/env node
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {root,read,sha,caseFiles,caseRow,selectedCases,older,dxReceipt,
  prerequisites,inventory,revision,frozen,freshOutput,commandsAt,
  verifyRecordedCommands,compiledImportControls} from './source.mjs';
import {verifyDx06} from './postpromotion.mjs';

const {editor,forged,initial,historical,previous}=prerequisites();
const dx=verifyDx06();
const cases=Object.fromEntries(selectedCases.map(id=>[id,caseRow(id)]));
for(const id of selectedCases)
  assert.equal(cases[id].evidence.length,3,`${id} needs its three earlier observations`);
const sources=inventory('replay',editor,forged),sourceRevision=revision(sources);
if(process.argv.length===3 && process.argv[2]==='--plan') {
  console.log(JSON.stringify({result:'ready-for-owner-review-not-execution',
    source_revision:sourceRevision,source_files:Object.keys(sources).length,
    cases:selectedCases,prerequisites:{...older,[dxReceipt]:dx.receipt_sha256},
    requires:'newly reviewed source revision and fresh external output'}));
  process.exit(0);
}
assert.equal(process.argv.length,4,
  'usage: bun verification/scase16-current-source/replay-gate.mjs NEW_EXTERNAL_DIR sha256:REVIEWED_REVISION');
assert.equal(process.argv[3],sourceRevision,'source differs from separately reviewed freeze');
const artifacts=freshOutput(process.argv[2]);
const packets=[...new Set(selectedCases.map(id=>caseFiles[id]))];
const inputFile=(name,source)=>{
  const file=path.join(artifacts,'inputs',`${name}.noble`);
  fs.writeFileSync(file,source,{flag:'wx'});
  return file;
};
fs.mkdirSync(path.join(artifacts,'inputs'));
for(const file of packets)
  fs.writeFileSync(path.join(artifacts,'inputs',path.basename(file)),read(file),{flag:'wx'});
const safetyInputs=[inputFile('S-CASE-01',cases['S-CASE-01'].input.source),
  inputFile('S-CASE-05',cases['S-CASE-05'].input.source),
  ...cases['S-CASE-14'].input.sources.map((source,i)=>inputFile(`S-CASE-14-${i}`,source))];
assert.equal(safetyInputs.length,4);
const {run,build,artifact,commands,environment,selected,rust}=commandsAt(artifacts);
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
const diagnostic=compilePeer('diagnostic-check',
  'verification/current-source-replay/diagnostic-check.rs',
  [['noble_contracts',contracts],['noble_kernel',kernel]]);
const safety=compilePeer('safety-core-check','verification/safety-core/check.rs',
  [['noble_contracts',contracts],['noble_kernel',kernel]]);
const handle=compilePeer('safety-handle-check','verification/safety-core/handle-check.rs',
  [['noble_kernel',kernel]]);
const reports=(label,args,stdin,status,count)=>{
  const rows=run(label,cli,args,status,Buffer.from(stdin)).trim().split('\n')
    .map(line=>JSON.parse(line));
  assert.equal(rows.length,count,`${label} reports`);
  return rows;
};
const fields=(observed,expected,label)=>{
  for(const [field,value] of Object.entries(expected))
    assert.deepEqual(observed[field],value,`${label}.${field}`);
};
const oldDx=historical['verification/dx01/current-frozen.json'];
const oldAdapt=historical['verification/adapt-01/current-frozen.json'];
const dxReports=reports('dx01-session',['session'],oldDx.cli_run.stdin_utf8,
  oldDx.cli_run.exit_code,2);
for(let i=0;i<dxReports.length;i++) {
  const {source,failure,...expected}=oldDx.cli_run.reports[i];
  assert.equal(source,cases['DX-01'].input.variants[i].source);
  assert.equal(failure,cases['DX-01'].input.variants[i].failure);
  fields(dxReports[i],expected,`DX-01/${failure}`);
}
const typed=JSON.parse(run('dx01-typed-resource',diagnostic,
  [cases['DX-01'].input.variants[2].source]).trim());
const {path:apiPath,resource_identity,resource_kind,operation_observation,
  ...typedExpected}=oldDx.typed_source_check;
assert.equal(apiPath,'source::ModuleSession::new([]).prepare(source, [Ty::Resource(FIXTURE_RESOURCE)], limits)');
assert.equal(resource_kind,'ResourceKind(0)');
assert.ok(resource_identity.includes('FIXTURE_RESOURCE'));
assert.ok(operation_observation.includes('before producing a candidate'));
fields(typed,typedExpected,'DX-01/typed-resource');
const adaptReports=reports('adapt01-session',['session'],oldAdapt.rejection_run.stdin_utf8,
  oldAdapt.rejection_run.exit_code,3);
for(let i=0;i<adaptReports.length;i++) {
  const {source,failure,...expected}=oldAdapt.rejection_run.reports[i];
  assert.equal(source,cases['ADAPT-01'].input.variants[i].source);
  assert.equal(failure,cases['ADAPT-01'].input.variants[i].failure);
  fields(adaptReports[i],expected,`ADAPT-01/${failure}`);
}
const positive=reports('adapt01-compatible',['session'],
  oldAdapt.compatible_control.stdin_utf8,oldAdapt.compatible_control.exit_code,1)[0];
for(const field of ['stage','outcome','stack','guest_requests','protected_operations'])
  assert.deepEqual(positive[field],oldAdapt.compatible_control[field]);
const safetyReports=run('safety-source-and-kernel',safety,safetyInputs).trim()
  .split('\n').map(line=>JSON.parse(line));
assert.deepEqual(safetyReports.map(item=>item.id),['S-CASE-01','S-CASE-05','S-CASE-14']);
for(const observed of safetyReports)
  fields(observed,cases[observed.id].expected,observed.id);
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
const handled=JSON.parse(run('safety-handle-forgery',handle,
  [String(cases['S-CASE-03'].input.representation)]).trim());
fields(handled,cases['S-CASE-03'].expected,'S-CASE-03');
assert.equal(handled.positive_registered_owner,true);
assert.equal(handled.absent_after_registration,true);
const conditional=compiledImportControls(artifacts,cli,run);
for(const [file,digest] of Object.entries(older)) assert.equal(sha(read(file)),digest);
assert.equal(sha(read(dxReceipt)),dx.receipt_sha256);
assert.equal(initial.observed.executions,5);
frozen(sources);
verifyRecordedCommands(commands,artifacts);
const claims={
  'DX-01':'After S-CASE-16 admission changes, both exact compiled word/branch diagnostics and independent typed Resource Data refusal still reject before execution; general proof remains open.',
  'ADAPT-01':'After S-CASE-16 admission changes, three exact compiled branch-join refusals and one compatible typed branch retain their outcomes.',
  'S-CASE-01':'After S-CASE-16 admission changes, exact ill-typed source still rejects in compiled CLI and independent source/kernel peer before execution.',
  'S-CASE-03':'After S-CASE-16 admission changes, absent handle 42 still rejects in production resource Table and one trusted registration succeeds.',
  'S-CASE-05':'After S-CASE-16 admission changes, typed test.emit candidate still rejects against the empty effect bound before execution.',
  'S-CASE-14':'After S-CASE-16 admission changes, both unsafe builtin names remain unbound in compiled CLI and independent source/kernel peer.',
};
const receipt={schema:'noble-scase16-current-source-replay/v1',result:'passed',kind:'test',
  cases:selectedCases.map(id=>({id,input:cases[id].input,expected:cases[id].expected,
    new_claim:claims[id],previous_evidence_sha256:sha(JSON.stringify(cases[id].evidence))})),
  source_revision:sourceRevision,source_sha256:sources,
  prepromotion_case_sha256:Object.fromEntries(packets.map(file=>[file,sources[file]])),
  prior_receipt_sha256:older,
  prerequisites:{'S-CASE-16':{receipt:'verification/scase16/acceptance.json',
    sha256:older['verification/scase16/acceptance.json'],
    source_revision:`sha256:${initial.source_tree_sha256}`},
    'DX-06-current':{receipt:dxReceipt,sha256:dx.receipt_sha256,
      source_revision:dx.source_revision},
    'DX-02':{receipt:editor.receiptFile,sha256:editor.receiptSha256},
    'ADAPT-06':{receipt:forged.receiptFile,sha256:forged.receiptSha256}},
  selected_rust:selected.tool_paths.quality_rust,
  binary:{path:cli,sha256:sha(fs.readFileSync(cli))},
  peers:Object.fromEntries([diagnostic,safety,handle].map(file=>
    [path.basename(file),{path:file,sha256:sha(fs.readFileSync(file))}])),
  observed:{dx01_cli:dxReports,dx01_typed:typed,adapt01_cli:adaptReports,
    adapt01_compatible:positive,safety_independent:safetyReports,
    safety_cli:safetyCli,safety_handle:handled},
  conditional_imports:conditional,commands,
  assumptions:[
    'Selected Rust/Cargo, compiled CLI, production source/kernel/resource peers, Node/V8 and host-selected artifact admission are trusted only for exact source/binary bytes and these finite observations.',
    'S-CASE-16 and DX-06 renewals retain separate immutable receipt scopes; negative paths do not enter guest code and the pure/emit controls do not prove universal artifact correspondence or native memory safety.',
    'General Aeneas/Lean, whole-source architecture gate, backend and host authority refinement remain open.'
  ]};
fs.writeFileSync(path.join(artifacts,'acceptance.json'),JSON.stringify(receipt,null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({result:'passed',receipt:path.join(artifacts,'acceptance.json'),
  source_revision:sourceRevision,cases:selectedCases,commands:commands.length,
  failures:0,integrity_failures:0}));
