import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {root,read,sha,caseFiles,caseRow,selectedCases,historic,historicalReceipts,
  acceptedReceipt,sourceInventory,sourceTrees,dx06InputFiles,replayInputFiles,
  revision,frozen,freshOutput,commandsAt,verifyRecordedCommands,removeNewestEvidence}
  from '../dx06-current-source/source.mjs';

export {root,read,sha,caseFiles,caseRow,selectedCases,historic,historicalReceipts,
  sourceInventory,revision,frozen,freshOutput,commandsAt,verifyRecordedCommands,removeNewestEvidence};
export const folder='verification/scase16-current-source';
export const dxReceipt=`${folder}/dx06-acceptance.json`;
export const replayReceipt=`${folder}/acceptance.json`;
export const initialReceipt='verification/scase16/acceptance.json';
export const older={
  'verification/dx02/acceptance.json':'2a6e59c06c26b3120b9a242e990647d64d7b4ed455208398801183ac435d7b5f',
  'verification/adapt06/acceptance.json':'a3be07a092cc48ee62d83f546996f16195195f53c23f1306f2ea1f0859909177',
  'verification/dx06-current-source/acceptance.json':'cf0eb8225a53564478364fe3083b7baea674365586e490e886631ec7673b8357',
  'verification/current-source-replay-dx02/acceptance.json':'c5f6c5956fd5ee2de5fc20bf298c1eb33768f70b0078deb109feae3739e58cfe',
  [initialReceipt]:'7534cfc6501b223e80e04ed0042fdf21fead5ffe246e4528b1358e8d605457d5',
};
const ownFiles=['source.mjs','dx06-gate.mjs','replay-gate.mjs','projection.mjs',
  'postpromotion.mjs'].map(file=>`${folder}/${file}`);
const trees=[...sourceTrees,'.cairn/changes/safety-wasm-manifest-admission'];
const extras=[initialReceipt,'verification/scase16/gate.mjs',
  '.cairn/specs/safety/spec.md',...Object.keys(older),...ownFiles];
export function sourceFiles(kind,editor,forged) {
  const originals=kind==='dx06'?dx06InputFiles(editor,forged):replayInputFiles(editor,forged);
  return [...new Set([...originals,...extras,...(kind==='replay'?[dxReceipt]:[])])];
}
export function inventory(kind,editor,forged) {
  return sourceInventory(sourceFiles(kind,editor,forged),trees);
}
export function prerequisites() {
  const historical=historic();
  for(const [file,digest] of Object.entries(older))
    assert.equal(sha(read(file)),digest,`previous acceptance changed: ${file}`);
  const editor=acceptedReceipt('DX-02'),forged=acceptedReceipt('ADAPT-06');
  const initial=JSON.parse(read(initialReceipt));
  const safety=JSON.parse(read('specs/conformance/safety-cases.json'));
  const row=safety.cases.find(item=>item.id==='S-CASE-16');
  assert.ok(row && safety.cases.filter(item=>item.id==='S-CASE-16').length===1);
  assert.deepEqual(row.state,{implementation:'implemented',execution:'passed',proof:'open',trust:'explicit'});
  assert.equal(row.evidence.length,1);
  assert.equal(initial.schema,'noble-scase16-source-bound-acceptance/v1');
  assert.equal(initial.case,row.id);
  assert.deepEqual([initial.canonical.stage,initial.canonical.outcome,
    initial.canonical.actual_imports,initial.canonical.claimed_effects],
    [row.expected.stage,row.expected.outcome,row.input.actual_imports,row.input.claimed_effects]);
  assert.equal(initial.canonical.case_sha256,sha(JSON.stringify([row.id,row.profile,
    row.kind,row.requirements,row.input,row.expected])));
  assert.deepEqual(initial.observed,{refusals:23,executions:5,unchanged_host:3});
  assert.equal(row.evidence[0].configuration.receipt,'../../'+initialReceipt);
  assert.equal(row.evidence[0].configuration.receipt_sha256,older[initialReceipt]);
  assert.equal(row.evidence[0].source_revision,`sha256:${initial.source_tree_sha256}`);
  for(const [file,digest] of Object.entries(initial.source_sha256))
    assert.equal(sha(read(file)),digest,`initial feature source changed: ${file}`);
  const previous=JSON.parse(read('verification/current-source-replay-dx02/acceptance.json'));
  for(const id of selectedCases) {
    const selected=caseRow(id),record=previous.cases.find(item=>item.id===id);
    assert.deepEqual(selected.state,{implementation:'implemented',execution:'passed',proof:'open',trust:'explicit'});
    assert.ok(selected.evidence.length===3 || selected.evidence.length===4);
    assert.deepEqual({id:selected.id,input:selected.input,expected:selected.expected},
      {id:record.id,input:record.input,expected:record.expected});
    assert.equal(selected.evidence[2].configuration.receipt_sha256,
      older['verification/current-source-replay-dx02/acceptance.json']);
  }
  const dx=caseRow('DX-06');
  assert.equal(dx.evidence[1].configuration.receipt_sha256,
    older['verification/dx06-current-source/acceptance.json']);
  assert.deepEqual(dx.state,{implementation:'implemented',execution:'passed',proof:'open',trust:'explicit'});
  assert.ok(dx.evidence.length===2 || dx.evidence.length===3);
  return {editor,forged,initial,previous,historical,safety};
}

// Exercise both faces of conditional imports using freshly compiled source,
// actual Wasm module imports, and independent host-selected artifact admission.
export function compiledImportControls(artifacts,cli,run) {
  const files=path.join(artifacts,'conditional-imports');
  fs.mkdirSync(files);
  function create(name,text) {
    const file=path.join(files,name);
    fs.writeFileSync(file,text,{flag:'wx'});
    return file;
  }
  const pure=create('pure.noble','1 2 +\n');
  const effect=create('effect.noble','"audit" test.emit\n');
  const empty=create('empty.json',JSON.stringify({claimed_effects:[]}));
  const trusted=create('trusted.json',JSON.stringify({claimed_effects:['test.emit']}));
  function compile(label,file) {
    const output=path.join(files,label);
    const report=JSON.parse(run(`compile-${label}`,cli,
      ['run',file,'--opt','off','--emit',output]).trim());
    assert.equal(report.outcome,'normal');
    return {report,file:path.join(output,'engine','module-1.wasm')};
  }
  const p=compile('pure',pure),e=compile('effect',effect);
  const imports=file=>WebAssembly.Module.imports(new WebAssembly.Module(fs.readFileSync(file)))
    .filter(item=>item.kind==='function').map(item=>`${item.module}.${item.name}`);
  assert.deepEqual(imports(p.file),[]);
  assert.deepEqual(imports(e.file),['noble.test_emit']);
  const negative=JSON.parse(run('forged-empty-manifest',cli,
    ['admit-artifact',e.file,'--effects',empty,'--source',effect,
      '--allow-effects','test.emit','--opt','off'],2).trim());
  assert.deepEqual([negative.stage,negative.outcome,negative.guest_requests,
    negative.protected_operations,negative.actual_imports],
    ['admission','effect-manifest-reject',0,0,['test.emit']]);
  const positive=JSON.parse(run('trusted-effect-manifest',cli,
    ['admit-artifact',e.file,'--effects',trusted,'--source',effect,
      '--allow-effects','test.emit','--opt','off']).trim());
  assert.deepEqual([positive.stage,positive.outcome,positive.guest_requests,
    positive.request_trace,positive.admission.correspondence,
    positive.admission.actual_imports],
    ['wasm','normal',1,['test.emit:audit'],'byte-exact',['test.emit']]);
  const pureReport=JSON.parse(run('trusted-pure-manifest',cli,
    ['admit-artifact',p.file,'--effects',empty,'--source',pure,'--opt','off']).trim());
  assert.deepEqual([pureReport.stage,pureReport.outcome,pureReport.guest_requests,
    pureReport.admission.actual_imports,pureReport.stack.map(value=>[value.type,value.value])],
    ['wasm','normal',0,[],[['I64','3']]]);
  return {compiled:{pure:sha(fs.readFileSync(p.file)),effect:sha(fs.readFileSync(e.file))},
    imports:{pure:[],effect:['noble.test_emit']},
    forged:{stage:negative.stage,outcome:negative.outcome,guest_requests:negative.guest_requests,
      protected_operations:negative.protected_operations},
    trusted:{guest_requests:positive.guest_requests,trace:positive.request_trace},
    pure:{stack:pureReport.stack,guest_requests:pureReport.guest_requests}};
}
