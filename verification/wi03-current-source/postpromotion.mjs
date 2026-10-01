import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {root,read,sha,caseFiles,caseRow,selectedCases,dxReceipt,replayReceipt,
  older,prerequisites,inventory,revision,sourceDigest,removeNewestEvidence}
  from './source.mjs';
import {restoreReplayEvidence} from './projection.mjs';
import {restoreReplayEvidence as restorePriorReleaseReplayEvidence}
  from '../adapt15-release-source/projection.mjs';
import {restoreReplayEvidence as restoreSecondReplayEvidence}
  from '../adapt15-final-source/projection.mjs';
import {restoreReplayEvidence as restorePriorReplayEvidence}
  from '../scase08-current-source/projection.mjs';

const casePaths=[...new Set(Object.values(caseFiles))];
const replayPaths=[...new Set(selectedCases.map(id=>caseFiles[id]))];
const checkCommands=(commands,external,expected)=>{
  assert.deepEqual(commands.map(item=>item.label),expected);
  for(const item of commands) {
    assert.equal(item.error,null);
    assert.equal(item.signal,null);
    assert.equal(item.status,item.label==='forged-empty-manifest'||
      item.label==='dx01-session'||item.label==='adapt01-session'||
      item.label==='cli-S-CASE-01'||item.label==='cli-S-CASE-14-0'||
      item.label==='cli-S-CASE-14-1'?2:0,`${item.label} status`);
    assert.equal(sha(fs.readFileSync(item.executable)),item.executable_sha256);
    for(const kind of ['stdout','stderr']) assert.equal(
      sha(fs.readFileSync(path.join(external,item[kind]))),
      item[`${kind}_sha256`],`${item.label} ${kind}`);
  }
};
const conditionalLabels=['compile-pure','compile-effect','forged-empty-manifest',
  'trusted-effect-manifest','trusted-pure-manifest'];
function verifyConditional(receipt,external) {
  const observed=receipt.conditional_imports;
  assert.deepEqual(observed.imports,{pure:[],effect:['noble.test_emit']});
  assert.deepEqual(observed.forged,{stage:'admission',outcome:'effect-manifest-reject',
    guest_requests:0,protected_operations:0});
  assert.deepEqual(observed.trusted,{guest_requests:1,trace:['test.emit:audit']});
  assert.equal(observed.pure.guest_requests,0);
  assert.deepEqual(observed.pure.stack.map(item=>[item.type,item.value]),[['I64','3']]);
  for(const [name,label] of [['pure','pure'],['effect','effect']]) {
    const wasm=path.join(external,'conditional-imports',label,'engine','module-1.wasm');
    assert.equal(sha(fs.readFileSync(wasm)),observed.compiled[name]);
    assert.deepEqual(WebAssembly.Module.imports(new WebAssembly.Module(fs.readFileSync(wasm)))
      .filter(item=>item.kind==='function').map(item=>`${item.module}.${item.name}`),
    observed.imports[name]);
  }
}
function verifyCallbackOwnerCase(text,callback) {
  const lines=text.split('\n').filter(line=>line.includes('"id":"S-CASE-08"'));
  assert.equal(lines.length,1);
  const row=JSON.parse(lines[0].trim().replace(/,$/,''));
  assert.deepEqual({id:row.id,input:row.input,expected:row.expected},callback.case);
  assert.deepEqual(row.state,{implementation:'implemented',execution:'passed',
    proof:'open',trust:'explicit'});
  assert.equal(row.evidence.length,1);
  assert.equal(row.evidence[0].configuration.receipt_sha256,
    older['verification/scase08/acceptance.json']);
  const prior=`    ${JSON.stringify({...row,
    state:{implementation:'absent',execution:'not-run',proof:'open',trust:'unassessed'},
    evidence:[]})},`;
  assert.equal(sha(text.replace(lines[0],prior)),callback.prepromotion_case_sha256);
}
function verifyMandatoryAdmissionCase(text,admission) {
  const external=admission.external_raw_output;
  const prior=fs.readFileSync(path.join(external,admission.prepromotion_inputs.case.path),'utf8');
  assert.equal(sha(prior),admission.prepromotion_case_sha256);
  const block=/\n    \{\n      "id": "ADAPT-15"[\s\S]*?\n    \},/g;
  const original=[...prior.matchAll(block)],current=[...text.matchAll(block)];
  assert.equal(original.length,1);assert.equal(current.length,1);
  const originalRow=JSON.parse(original[0][0].trim().replace(/,$/,''));
  const row=JSON.parse(current[0][0].trim().replace(/,$/,''));
  assert.deepEqual(Object.fromEntries(['id','profile','kind','requirements',
    'input','expected'].map(key=>[key,row[key]])),admission.case);
  assert.deepEqual(row.state,{implementation:'implemented',execution:'passed',
    proof:'open',trust:'explicit'});
  assert.equal(row.evidence.length,1);
  assert.deepEqual([row.evidence[0].subject,row.evidence[0].result,
    row.evidence[0].source_revision,row.evidence[0].configuration.receipt_sha256],
    ['ADAPT-15','passed',admission.source_revision,
      older['verification/adapt15/acceptance.json']]);
  assert.equal(row.evidence[0].configuration.receipt,
    '../../verification/adapt15/acceptance.json');
  assert.equal(row.evidence[0].configuration.prepromotion_case_sha256,
    admission.prepromotion_case_sha256);
  assert.equal(row.evidence[0].configuration.gate_sha256,
    admission.source_sha256['verification/adapt15/gate.mjs']);
  assert.equal(row.evidence[0].configuration.external_raw_output,external);
  assert.deepEqual({...row,state:originalRow.state,evidence:originalRow.evidence},
    originalRow);
  const stateLine=/\n      "state": \{[^\n]+/g;
  const previous=[...original[0][0].matchAll(stateLine)];
  const promoted=[...current[0][0].matchAll(stateLine)];
  assert.equal(previous.length,1);assert.equal(promoted.length,1);
  const reverted=current[0][0].replace(promoted[0][0],previous[0][0]);
  assert.equal(sha(text.slice(0,current[0].index)+reverted+
    text.slice(current[0].index+current[0][0].length)),
    admission.prepromotion_case_sha256,
    'only the admitted ADAPT-15 row may differ from original prepromotion');
}
export function verifyDx06({caseTexts}={}) {
  assert.equal(fs.realpathSync(process.cwd()),root);
  const {editor,forged,initial,bounded,accepted,artifact,quota,
    authorization,resource,callback,admission,historicalDx,
    secondDx,secondReplay,priorReleaseDx,priorReleaseReplay,wi03}=prerequisites();
  const bytes=read(dxReceipt),receipt=JSON.parse(bytes),digest=sha(bytes);
  assert.deepEqual([receipt.schema,receipt.result,receipt.kind,receipt.mode],
    ['noble-wi03-current-source-dx06/v1','passed','test','dx06']);
  assert.deepEqual(receipt.prior_receipt_sha256,older);
  assert.deepEqual(receipt.prerequisites['S-CASE-16'],{
    receipt:'verification/scase16/acceptance.json',sha256:older['verification/scase16/acceptance.json'],
    source_revision:`sha256:${initial.source_tree_sha256}`});
  assert.deepEqual(receipt.prerequisites['S-CASE-02'],{
    receipt:'verification/scase02/corrected-acceptance.json',
    sha256:older['verification/scase02/corrected-acceptance.json'],
    source_revision:accepted.source_revision,
    previous_receipt_sha256:bounded.evidence[0].configuration.receipt_sha256});
  assert.deepEqual(receipt.prerequisites['S-CASE-07'],{
    receipt:'verification/scase07/acceptance.json',
    sha256:older['verification/scase07/acceptance.json'],
    source_revision:artifact.source_revision,
    previous_case_sha256:artifact.prepromotion_case_sha256});
  assert.deepEqual(receipt.prerequisites['S-CASE-13'],{
    receipt:'verification/scase13/corrected-acceptance.json',
    sha256:older['verification/scase13/corrected-acceptance.json'],
    source_revision:quota.source_revision,
    previous_receipt_sha256:older['verification/scase13/acceptance.json'],
    previous_case_sha256:quota.prepromotion_case_sha256});
  assert.deepEqual(receipt.prerequisites['S-CASE-06'],{
    receipt:'verification/scase06/final-acceptance.json',
    sha256:older['verification/scase06/final-acceptance.json'],
    source_revision:authorization.source_revision,
    previous_receipt_sha256:older['verification/scase06/corrected-acceptance.json'],
    previous_case_sha256:authorization.prepromotion_case_sha256});
  assert.deepEqual(receipt.prerequisites['S-CASE-04'],{
    receipt:'verification/scase04/acceptance.json',
    sha256:older['verification/scase04/acceptance.json'],
    source_revision:resource.source_revision,
    previous_case_sha256:resource.prepromotion_case_sha256});
  assert.deepEqual(receipt.prerequisites['S-CASE-08'],{
    receipt:'verification/scase08/acceptance.json',
    sha256:older['verification/scase08/acceptance.json'],
    source_revision:callback.source_revision,
    previous_case_sha256:callback.prepromotion_case_sha256});
  assert.deepEqual(receipt.prerequisites['ADAPT-15'],{
    receipt:'verification/adapt15/acceptance.json',
    sha256:older['verification/adapt15/acceptance.json'],
    source_revision:admission.source_revision,
    previous_case_sha256:admission.prepromotion_case_sha256});
  assert.deepEqual(receipt.prerequisites['DX-06-historical'],{
    receipt:'verification/adapt15-current-source/dx06-acceptance.json',
    sha256:older['verification/adapt15-current-source/dx06-acceptance.json'],
    source_revision:historicalDx.source_revision,
    original_checker_sha256:historicalDx.source_sha256[
      'verification/adapt15-current-source/postpromotion.mjs']});
  assert.deepEqual(receipt.prerequisites['DX-06-second-historical'],{
    receipt:'verification/adapt15-final-source/dx06-acceptance.json',
    sha256:older['verification/adapt15-final-source/dx06-acceptance.json'],
    source_revision:secondDx.source_revision,
    checker_sha256:secondDx.source_sha256[
      'verification/adapt15-final-source/postpromotion.mjs']});
  assert.deepEqual(receipt.prerequisites['six-case-second-historical'],{
    receipt:'verification/adapt15-final-source/acceptance.json',
    sha256:older['verification/adapt15-final-source/acceptance.json'],
    source_revision:secondReplay.source_revision});
  assert.deepEqual(receipt.prerequisites['DX-06-prior-release'],{
    receipt:'verification/adapt15-release-source/dx06-acceptance.json',
    sha256:older['verification/adapt15-release-source/dx06-acceptance.json'],
    source_revision:priorReleaseDx.source_revision});
  assert.deepEqual(receipt.prerequisites['six-case-prior-release'],{
    receipt:'verification/adapt15-release-source/acceptance.json',
    sha256:older['verification/adapt15-release-source/acceptance.json'],
    source_revision:priorReleaseReplay.source_revision});
  assert.deepEqual(receipt.prerequisites['WI-03-original-historical'],{
    receipt:'verification/wi03/acceptance.json',
    sha256:older['verification/wi03/acceptance.json'],
    source_revision:wi03.prior_prepromotion_receipt.source_revision,
    scope:'historical-only-unpromoted'});
  assert.deepEqual(receipt.prerequisites['WI-03-corrected-prepromotion'],{
    receipt:'verification/wi03-final/acceptance.json',
    sha256:older['verification/wi03-final/acceptance.json'],
    source_revision:wi03.source_revision,
    original_checker_sha256:wi03.source_sha256[
      'verification/wi03-final/postpromotion.mjs'],
    original_checker_result:'failed-native-scenario-placement',
    previous_case_sha256:wi03.prepromotion_case_sha256});
  assert.equal(receipt.source_revision,revision(receipt.source_sha256));
  assert.deepEqual(Object.keys(receipt.source_sha256).sort(),
    Object.keys(inventory('dx06',editor,forged)).sort());
  for(const [file,expected] of Object.entries(receipt.source_sha256)) {
    if(!casePaths.includes(file)) assert.equal(sourceDigest(file),expected,
      `frozen DX06 source: ${file}`);
  }
  const file=caseFiles['DX-06'];
  const safetyFile='specs/conformance/safety-cases.json';
  const safetyText=caseTexts?.[safetyFile]??read(safetyFile).toString('utf8');
  verifyCallbackOwnerCase(restorePriorReplayEvidence(
    restoreSecondReplayEvidence(
      restorePriorReleaseReplayEvidence(safetyText,safetyFile),safetyFile),safetyFile),
    callback);
  const adaptationFile='specs/conformance/adaptation-cases.json';
  verifyMandatoryAdmissionCase(restoreSecondReplayEvidence(
    restorePriorReleaseReplayEvidence(
      caseTexts?.[adaptationFile]??read(adaptationFile).toString('utf8'),
      adaptationFile),
    adaptationFile),admission);
  for(const item of casePaths) {
    const current=caseTexts?.[item]??read(item).toString('utf8');
    const restored=item===file?removeNewestEvidence(current,'DX-06',13):current;
    assert.equal(sha(restored),receipt.case_sha256[item],`DX06 preimage ${item}`);
    if(item===file) {
      const packet=JSON.parse(current),row=packet.cases.find(value=>value.id==='DX-06');
      assert.equal(row.evidence.length,13);
      assert.equal(sha(JSON.stringify(row.evidence.slice(0,12))),receipt.previous_evidence_sha256);
      assert.equal(row.evidence[11].configuration.receipt_sha256,
        older['verification/adapt15-release-source/dx06-acceptance.json']);
      const external=path.dirname(receipt.smoke.directory);
      assert.deepEqual(row.evidence[12],{kind:'test',subject:'DX-06',
        claim:receipt.new_claim,revision:packet.revision,
        source_revision:receipt.source_revision,result:'passed',
        configuration:{receipt:path.posix.relative(path.posix.dirname(file),dxReceipt),
          case_id:'DX-06',receipt_sha256:digest,external_raw_output:external,
          prepromotion_case_sha256:receipt.case_sha256[file],
          binary_sha256:receipt.binary.sha256},assumptions:receipt.assumptions});
      assert.equal(sha(fs.readFileSync(path.join(external,'acceptance.json'))),digest);
    }
  }
  const external=path.dirname(receipt.smoke.directory);
  assert.equal(sha(fs.readFileSync(receipt.binary.path)),receipt.binary.sha256);
  assert.equal(sha(fs.readFileSync(receipt.selected_node.path)),receipt.selected_node.sha256);
  const smokeBytes=fs.readFileSync(path.join(receipt.smoke.directory,receipt.smoke.file));
  assert.equal(sha(smokeBytes),receipt.smoke.sha256);
  const smoke=JSON.parse(smokeBytes);
  assert.equal(smoke.result,'six-controls-pass');
  assert.equal(smoke.production_cli_sha256,receipt.binary.sha256);
  assert.equal(smoke.source_revision,receipt.smoke.original_source_revision);
  assert.deepEqual(smoke.controls,receipt.smoke.controls);
  assert.deepEqual(smoke.latent_effect_witness,['test.clock']);
  assert.equal(smoke.real_host_fallback_calls,0);
  for(const [file,record] of Object.entries(receipt.smoke.retained))
    assert.equal(sha(fs.readFileSync(path.join(receipt.smoke.directory,file))),record.sha256);
  verifyConditional(receipt,external);
  checkCommands(receipt.commands,external,['build','six-compiled-test-clock-controls',
    ...conditionalLabels]);
  return {receipt_sha256:digest,source_revision:receipt.source_revision};
}
export function verifyReplay() {
  assert.equal(fs.realpathSync(process.cwd()),root);
  const {editor,forged}=prerequisites();
  const bytes=read(replayReceipt),receipt=JSON.parse(bytes),digest=sha(bytes);
  assert.deepEqual([receipt.schema,receipt.result,receipt.kind,receipt.mode],
    ['noble-wi03-current-source-replay/v1','passed','test','replay']);
  assert.deepEqual(receipt.cases.map(item=>item.id),selectedCases);
  assert.deepEqual(receipt.prior_receipt_sha256,older);
  assert.equal(receipt.source_revision,revision(receipt.source_sha256));
  assert.deepEqual(Object.keys(receipt.source_sha256).sort(),
    Object.keys(inventory('replay',editor,forged)).sort());
  const projected={};
  for(const file of replayPaths) {
    const text=read(file).toString('utf8');
    const old=restoreReplayEvidence(text,file,bytes);
    assert.notEqual(old,text,`${file} missing thirteenth evidence`);
    assert.equal(sha(old),receipt.case_sha256[file]);
    projected[file]=old;
  }
  const dx=verifyDx06({caseTexts:projected});
  assert.deepEqual(receipt.prerequisites['DX-06-current'],{
    receipt:dxReceipt,sha256:dx.receipt_sha256,source_revision:dx.source_revision});
  for(const [file,expected] of Object.entries(receipt.source_sha256)) {
    if(!replayPaths.includes(file)) assert.equal(sourceDigest(file),expected,
      `frozen six-case source: ${file}`);
  }
  const external=JSON.parse(read(replayPaths[0])).cases
    .find(item=>item.id===selectedCases[0]).evidence[12].configuration.external_raw_output;
  assert.equal(sha(fs.readFileSync(path.join(external,'acceptance.json'))),digest);
  assert.equal(sha(fs.readFileSync(receipt.binary.path)),receipt.binary.sha256);
  for(const peer of Object.values(receipt.peers))
    assert.equal(sha(fs.readFileSync(peer.path)),peer.sha256);
  verifyConditional(receipt,external);
  checkCommands(receipt.commands,external,['build','build-diagnostic-check',
    'build-safety-core-check','build-safety-handle-check','dx01-session',
    'dx01-typed-resource','adapt01-session','adapt01-compatible',
    'safety-source-and-kernel','cli-S-CASE-01','cli-S-CASE-14-0',
    'cli-S-CASE-14-1','safety-handle-forgery',...conditionalLabels]);
  return {receipt_sha256:digest,source_revision:receipt.source_revision,
    cases:selectedCases,commands:receipt.commands.length,proof:'open'};
}
if(process.argv[1] && fileURLToPath(import.meta.url)===path.resolve(process.argv[1]))
  console.log(JSON.stringify(process.argv[2]==='--dx06'?verifyDx06():verifyReplay()));
