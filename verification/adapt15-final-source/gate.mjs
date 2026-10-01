#!/usr/bin/env node
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {root,read,sha,caseFiles,caseRow,selectedCases,older,dxReceipt,
  prerequisites,inventory,revision,frozen,freshOutput,commandsAt,
  verifyRecordedCommands,compiledImportControls} from './source.mjs';
import {verifyDx06} from './postpromotion.mjs';

const mode=process.argv[2];
assert.ok(mode==='dx06'||mode==='replay','select dx06 or replay');
const {editor,forged,initial,historical,bounded,accepted,
  artifact:correspondence,quota,authorization,resource,callback,admission,
  historicalDx}=prerequisites();
const preceding=mode==='replay'?verifyDx06():null;
if(mode==='dx06') assert.equal(caseRow('DX-06').evidence.length,10);
else for(const id of selectedCases)
  assert.equal(caseRow(id).evidence.length,10,`${id} ten earlier observations`);
const sources=inventory(mode,editor,forged),sourceRevision=revision(sources);
if(process.argv.length===4 && process.argv[3]==='--plan') {
  console.log(JSON.stringify({result:'ready-for-owner-review-not-execution',
    mode,source_revision:sourceRevision,source_files:Object.keys(sources).length,
    prior_receipt_sha256:older,
    prerequisite_dx06_receipt_sha256:preceding?.receipt_sha256??null,
    cases:mode==='dx06'?['DX-06']:selectedCases,
    requires:'accepted ADAPT-15 and immutable historical first DX06 on truthful three-ACTIVE Cairn sources, corrected reviewed frozen runners, and fresh external directory; no Octet/Nix assurance or archive claimed'}));
  process.exit(0);
}
assert.equal(process.argv.length,5,
  'usage: bun verification/adapt15-final-source/gate.mjs dx06|replay NEW_EXTERNAL_DIR sha256:REVIEWED_REVISION');
assert.equal(process.argv[4],sourceRevision,'source differs from separately reviewed final-view freeze');
const artifacts=freshOutput(process.argv[3]);
const {run,build,artifact,commands,environment,selected,rust}=commandsAt(artifacts);
const built=build(),cli=artifact(built,'noble','bin');
const node=JSON.parse(read('crates/noble-cli/src/core/runtime/config.json')).tools.node.path;
const observations={};
if(mode==='dx06') {
  const smokeDirectory=path.join(artifacts,'smoke');
  const printed=JSON.parse(run('six-compiled-test-clock-controls',node,
    ['verification/dx06/gate.mjs',cli,smokeDirectory]).trim());
  const bytes=fs.readFileSync(path.join(smokeDirectory,'smoke.json'));
  const smoke=JSON.parse(bytes),design=caseRow('DX-06');
  assert.deepEqual(printed,smoke);
  assert.equal(smoke.result,'six-controls-pass');
  assert.equal(smoke.case_id,'DX-06');
  assert.deepEqual(Object.keys(smoke.controls),design.input.variants);
  assert.deepEqual(smoke.controls['authorized-matching-interface'],{outcome:'normal',value:'42'});
  assert.deepEqual([smoke.controls.denied.outcome,smoke.controls.denied.request.reason],
    ['trap','policy-denied']);
  assert.equal(smoke.controls['missing-mapping'].outcome,'reject');
  assert.equal(smoke.controls['incompatible-interface'].outcome,'invalid-input');
  assert.deepEqual(smoke.controls['second-request-script-exhausted'].requests
    .map(item=>[item.decision,item.reason??null]),
    [['allow',null],['deny','script-exhausted']]);
  assert.deepEqual([smoke.controls['unexpected-operation'].outcome,
    smoke.controls['unexpected-operation'].request.reason],['trap','unexpected-operation']);
  assert.deepEqual(smoke.latent_effect_witness,['test.clock']);
  assert.equal(smoke.real_host_fallback_calls,0);
  assert.equal(smoke.real_host_evidence,false);
  assert.equal(smoke.pre_promotion_case_sha256,sources[caseFiles['DX-06']]);
  assert.equal(smoke.production_cli_sha256,sha(fs.readFileSync(cli)));
  assert.equal(smoke.selected_node_sha256,sha(fs.readFileSync(node)));
  observations.smoke={directory:smokeDirectory,file:'smoke.json',sha256:sha(bytes),
    original_source_revision:smoke.source_revision,
    retained:smoke.retained,controls:smoke.controls,
    latent_effect_witness:smoke.latent_effect_witness};
  observations.previous_evidence_sha256=sha(JSON.stringify(design.evidence));
  observations.new_claim='After ADAPT-15 four-cell mandatory admission and the immutable earlier finite DX-06 observation, a corrected independently frozen source repeats six exact compiled test.clock controls and latent reflection plus compiled pure/test.emit import/admission contrasts on three active changes; no universal host proof follows.';
} else {
  const cases=Object.fromEntries(selectedCases.map(id=>[id,caseRow(id)]));
  const inputDirectory=path.join(artifacts,'inputs');
  fs.mkdirSync(inputDirectory);
  const input=(name,source)=>{
    const file=path.join(inputDirectory,`${name}.noble`);
    fs.writeFileSync(file,source,{flag:'wx'});
    return file;
  };
  const inputs=[input('S-CASE-01',cases['S-CASE-01'].input.source),
    input('S-CASE-05',cases['S-CASE-05'].input.source),
    ...cases['S-CASE-14'].input.sources.map((source,i)=>input(`S-CASE-14-${i}`,source))];
  assert.equal(inputs.length,4);
  for(const file of [...new Set(selectedCases.map(id=>caseFiles[id]))])
    fs.writeFileSync(path.join(inputDirectory,path.basename(file)),read(file),{flag:'wx'});
  const contracts=artifact(built,'noble_contracts','lib');
  const kernel=artifact(built,'noble_kernel','lib');
  const peer=(name,file,extern)=>{
    const output=path.join(artifacts,name);
    run(`build-${name}`,path.join(rust,'rustc'),['--edition=2021',
      '--crate-name',name.replaceAll('-','_'),file,
      ...extern.flatMap(([key,library])=>['--extern',`${key}=${library}`]),
      '-L',`dependency=${path.join(environment.CARGO_TARGET_DIR,'debug/deps')}`,'-o',output]);
    return output;
  };
  const diagnostic=peer('diagnostic-check',
    'verification/current-source-replay/diagnostic-check.rs',
    [['noble_contracts',contracts],['noble_kernel',kernel]]);
  const safety=peer('safety-core-check','verification/safety-core/check.rs',
    [['noble_contracts',contracts],['noble_kernel',kernel]]);
  const handle=peer('safety-handle-check','verification/safety-core/handle-check.rs',
    [['noble_kernel',kernel]]);
  const reports=(label,args,stdin,status,count)=>{
    const rows=run(label,cli,args,status,Buffer.from(stdin)).trim().split('\n')
      .map(line=>JSON.parse(line));
    assert.equal(rows.length,count);
    return rows;
  };
  const fields=(observed,expected,label)=>{
    for(const [field,value] of Object.entries(expected))
      assert.deepEqual(observed[field],value,`${label}.${field}`);
  };
  const oldDx=historical['verification/dx01/current-frozen.json'];
  const oldAdapt=historical['verification/adapt-01/current-frozen.json'];
  const dx=reports('dx01-session',['session'],oldDx.cli_run.stdin_utf8,
    oldDx.cli_run.exit_code,2);
  for(let i=0;i<dx.length;i++) {
    const {source,failure,...expected}=oldDx.cli_run.reports[i];
    assert.equal(source,cases['DX-01'].input.variants[i].source);
    assert.equal(failure,cases['DX-01'].input.variants[i].failure);
    fields(dx[i],expected,`DX-01/${failure}`);
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
  const adapt=reports('adapt01-session',['session'],oldAdapt.rejection_run.stdin_utf8,
    oldAdapt.rejection_run.exit_code,3);
  for(let i=0;i<adapt.length;i++) {
    const {source,failure,...expected}=oldAdapt.rejection_run.reports[i];
    assert.equal(source,cases['ADAPT-01'].input.variants[i].source);
    assert.equal(failure,cases['ADAPT-01'].input.variants[i].failure);
    fields(adapt[i],expected,`ADAPT-01/${failure}`);
  }
  const compatible=reports('adapt01-compatible',['session'],
    oldAdapt.compatible_control.stdin_utf8,oldAdapt.compatible_control.exit_code,1)[0];
  for(const field of ['stage','outcome','stack','guest_requests','protected_operations'])
    assert.deepEqual(compatible[field],oldAdapt.compatible_control[field]);
  const safetyRows=run('safety-source-and-kernel',safety,inputs).trim()
    .split('\n').map(line=>JSON.parse(line));
  assert.deepEqual(safetyRows.map(item=>item.id),['S-CASE-01','S-CASE-05','S-CASE-14']);
  for(const observed of safetyRows) fields(observed,cases[observed.id].expected,observed.id);
  assert.equal(safetyRows[1].constraint,'EffectInclusion(test.emit)');
  assert.equal(safetyRows[2].variants,cases['S-CASE-14'].input.sources.length);
  const safetyCli=[];
  for(const [label,file,design] of [
    ['S-CASE-01',inputs[0],cases['S-CASE-01']],
    ['S-CASE-14-0',inputs[2],cases['S-CASE-14']],
    ['S-CASE-14-1',inputs[3],cases['S-CASE-14']],
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
  observations.cases=selectedCases.map(id=>({id,input:cases[id].input,
    expected:cases[id].expected,
    new_claim:`After ADAPT-15 four-cell mandatory-admission rejection and corrected chronological DX-06 renewal, ${id} retains its exact compiled/independent selected observation against the truthful three-ACTIVE Cairn source; universal refinement remains open.`,
    previous_evidence_sha256:sha(JSON.stringify(cases[id].evidence))}));
  observations.peers=Object.fromEntries([diagnostic,safety,handle]
    .map(file=>[path.basename(file),{path:file,sha256:sha(fs.readFileSync(file))}]));
  observations.reports={dx01_cli:dx,dx01_typed:typed,adapt01_cli:adapt,
    adapt01_compatible:compatible,safety_independent:safetyRows,
    safety_cli:safetyCli,safety_handle:handled};
}
const conditional=compiledImportControls(artifacts,cli,run);
for(const [file,digest] of Object.entries(older)) assert.equal(sha(read(file)),digest);
if(preceding) assert.equal(sha(read(dxReceipt)),preceding.receipt_sha256);
assert.equal(initial.observed.refusals,23);
frozen(sources);
verifyRecordedCommands(commands,artifacts);
const receipt={schema:mode==='dx06'?'noble-adapt15-corrected-three-active-dx06/v1':
  'noble-adapt15-corrected-three-active-replay/v1',result:'passed',kind:'test',mode,
  source_revision:sourceRevision,source_sha256:sources,
  case_sha256:Object.fromEntries([...new Set(Object.values(caseFiles))]
    .map(file=>[file,sources[file]])),
  prior_receipt_sha256:older,
  prerequisites:{'S-CASE-16':{receipt:'verification/scase16/acceptance.json',
    sha256:older['verification/scase16/acceptance.json'],
    source_revision:`sha256:${initial.source_tree_sha256}`},
    'S-CASE-02':{receipt:'verification/scase02/corrected-acceptance.json',
      sha256:older['verification/scase02/corrected-acceptance.json'],
      source_revision:accepted.source_revision,
      previous_receipt_sha256:bounded.evidence[0].configuration.receipt_sha256},
    'S-CASE-07':{receipt:'verification/scase07/acceptance.json',
      sha256:older['verification/scase07/acceptance.json'],
      source_revision:correspondence.source_revision,
      previous_case_sha256:correspondence.prepromotion_case_sha256},
    'S-CASE-13':{receipt:'verification/scase13/corrected-acceptance.json',
      sha256:older['verification/scase13/corrected-acceptance.json'],
      source_revision:quota.source_revision,
      previous_receipt_sha256:older['verification/scase13/acceptance.json'],
      previous_case_sha256:quota.prepromotion_case_sha256},
    'S-CASE-06':{receipt:'verification/scase06/final-acceptance.json',
      sha256:older['verification/scase06/final-acceptance.json'],
      source_revision:authorization.source_revision,
      previous_receipt_sha256:older['verification/scase06/corrected-acceptance.json'],
      previous_case_sha256:authorization.prepromotion_case_sha256},
    'S-CASE-04':{receipt:'verification/scase04/acceptance.json',
      sha256:older['verification/scase04/acceptance.json'],
      source_revision:resource.source_revision,
      previous_case_sha256:resource.prepromotion_case_sha256},
    'S-CASE-08':{receipt:'verification/scase08/acceptance.json',
      sha256:older['verification/scase08/acceptance.json'],
      source_revision:callback.source_revision,
      previous_case_sha256:callback.prepromotion_case_sha256},
    'ADAPT-15':{receipt:'verification/adapt15/acceptance.json',
      sha256:older['verification/adapt15/acceptance.json'],
      source_revision:admission.source_revision,
      previous_case_sha256:admission.prepromotion_case_sha256},
    'DX-06-historical':{receipt:'verification/adapt15-current-source/dx06-acceptance.json',
      sha256:older['verification/adapt15-current-source/dx06-acceptance.json'],
      source_revision:historicalDx.source_revision,
      original_checker_sha256:historicalDx.source_sha256[
        'verification/adapt15-current-source/postpromotion.mjs']},
    ...(preceding?{'DX-06-current':{receipt:dxReceipt,
      sha256:preceding.receipt_sha256,source_revision:preceding.source_revision}}:{})},
  selected_rust:selected.tool_paths.quality_rust,
  selected_node:{path:node,sha256:sha(fs.readFileSync(node))},
  binary:{path:cli,sha256:sha(fs.readFileSync(cli))},
  ...observations,conditional_imports:conditional,commands,
  assumptions:[
    'The finite ADAPT-15 mandatory admission, first historical DX-06, S-CASE-08 callback returned-owner, S-CASE-04 recursive Resource and S-CASE-06 authorized fs.read receipts, truthful three-ACTIVE Cairn normative changes/source views, production and proof-source trees, selected Rust/Node/Wasm/Lean tools and finite policies are trusted only for exact recorded bytes and selected paths. Only task checklist markers are normalized; all task prose remains bound.',
    'Scoped Octet/full signed+sandbox Nix assurance and archive remain blocked/unclaimed for the active changes, including S-CASE-06 Phase3 staged postpromotion. Earlier receipts remain immutable historical scopes. Unrelated pure MC1 proof cannot authorize forged test.emit effect bounds; compiled test.emit/pure and scripted-clock paths do not prove universal checker, host, backend, memory or Rust-to-Lean refinement.'
  ]};
fs.writeFileSync(path.join(artifacts,'acceptance.json'),JSON.stringify(receipt,null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({result:'passed',mode,receipt:path.join(artifacts,'acceptance.json'),
  source_revision:sourceRevision,commands:commands.length,failures:0,integrity_failures:0}));
