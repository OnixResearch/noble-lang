#!/usr/bin/env node
// Validate the initial ADAPT-15 acceptance against its prepromotion source.
// Later chronological replay receipts have their own, separate source scope.
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';

const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'../..');
const read=file=>fs.readFileSync(path.join(root,file));
const sha=bytes=>crypto.createHash('sha256').update(bytes).digest('hex');
const receiptBytes=read('verification/adapt15/acceptance.json');
const receipt=JSON.parse(receiptBytes);
assert.equal(receipt.schema,'noble-adapt15-mandatory-admission/v1');
assert.deepEqual([receipt.kind,receipt.result,receipt.failures,receipt.integrity_failures],
  ['test','passed',[],[]]);
const external=receipt.external_raw_output;
assert.equal(path.isAbsolute(external),true);
assert.equal(sha(fs.readFileSync(path.join(external,'acceptance.json'))),sha(receiptBytes));
const pre=receipt.prepromotion_inputs;
for(const [name,snapshot] of Object.entries(pre)) {
  assert.equal(sha(fs.readFileSync(path.join(external,snapshot.path))),snapshot.sha256,
    `prepromotion snapshot changed: ${name}`);
}
assert.equal(pre.case.sha256,receipt.prepromotion_case_sha256);
for(const input of Object.values(receipt.input_sha256)) {
  const file=path.resolve(external,input.path);
  assert.equal(file.startsWith(`${external}${path.sep}`),true);
  assert.equal(sha(fs.readFileSync(file)),input.sha256,`input changed: ${file}`);
}
const file='specs/conformance/adaptation-cases.json';
const canonical=read(file).toString();
const prior=fs.readFileSync(path.join(external,pre.case.path),'utf8');
const block=/\n    \{\n      "id": "ADAPT-15"[\s\S]*?\n    \},/g;
const original=[...prior.matchAll(block)],current=[...canonical.matchAll(block)];
assert.equal(original.length,1);assert.equal(current.length,1);
const originalRow=JSON.parse(original[0][0].trim().replace(/,$/,''));
const row=JSON.parse(current[0][0].trim().replace(/,$/,''));
assert.deepEqual({id:row.id,profile:row.profile,kind:row.kind,
  requirements:row.requirements,input:row.input,expected:row.expected},receipt.case);
assert.deepEqual(row.state,{implementation:'implemented',execution:'passed',proof:'open',trust:'explicit'});
assert.equal(row.evidence.length,1);
const evidence=row.evidence[0];
assert.deepEqual([evidence.subject,evidence.kind,evidence.result,evidence.source_revision],
  ['ADAPT-15','test','passed',receipt.source_revision]);
assert.equal(evidence.configuration.receipt,'../../verification/adapt15/acceptance.json');
assert.equal(evidence.configuration.receipt_sha256,sha(receiptBytes));
assert.equal(evidence.configuration.prepromotion_case_sha256,receipt.prepromotion_case_sha256);
assert.equal(evidence.configuration.gate_sha256,receipt.source_sha256['verification/adapt15/gate.mjs']);
assert.equal(evidence.configuration.external_raw_output,external);
assert.deepEqual({...row,state:originalRow.state,evidence:originalRow.evidence},originalRow);
const stateLine=/\n      "state": \{[^\n]+/g;
const originalState=[...original[0][0].matchAll(stateLine)];
const currentState=[...current[0][0].matchAll(stateLine)];
assert.equal(originalState.length,1);assert.equal(currentState.length,1);
const reverted=current[0][0].replace(currentState[0][0],originalState[0][0]);
assert.equal(sha(canonical.slice(0,current[0].index)+reverted+
  canonical.slice(current[0].index+current[0][0].length)),
  receipt.prepromotion_case_sha256,'only the ADAPT-15 state/evidence line may change');
for(const [name,digest] of Object.entries(receipt.source_sha256)) {
  if(name===file)continue;
  assert.equal(sha(read(name)),digest,`accepted source changed: ${name}`);
}
assert.equal(receipt.source_revision,
  `sha256:${sha(JSON.stringify(Object.entries(receipt.source_sha256).sort()))}`);
for(const [name,digest] of Object.entries(receipt.historical_receipt_sha256))
  assert.equal(sha(read(name)),digest,`historical acceptance changed: ${name}`);
for(const command of receipt.commands) {
  assert.equal(command.error,null);assert.equal(command.signal,null);
  assert.ok([0,2,4].includes(command.expected_status),`${command.label} expected status`);
  assert.equal(command.status,command.expected_status,`${command.label} status`);
  assert.equal(sha(fs.readFileSync(command.executable)),command.executable_sha256);
  for(const kind of ['stdout','stderr'])assert.equal(
    sha(fs.readFileSync(path.join(external,command[kind]))),command[`${kind}_sha256`],
    `${command.label} ${kind}`);
}
assert.equal(receipt.cells.length,4);
for(const build of ['debug','release'])for(const optionalProof of ['absent','unrelated-valid-proof']) {
  const matched=receipt.cells.filter(cell=>cell.build===build&&cell.optional_proof===optionalProof);
  assert.equal(matched.length,1);
  const cell=matched[0];
  assert.deepEqual({stage:cell.stage,outcome:cell.outcome,constraint:cell.constraint,
    accepted_program_created:cell.accepted_program_created,
    guest_requests:cell.guest_requests,protected_operations:cell.protected_operations,
    proof_checked:cell.proof_checked},{stage:'acceptance',outcome:'reject',
    constraint:'EffectInclusion(test.emit)',accepted_program_created:false,
    guest_requests:0,protected_operations:0,
    proof_checked:optionalProof==='unrelated-valid-proof'});
}
for(const build of ['debug','release']) {
  const [without,withProof]=['absent','unrelated-valid-proof'].map(optional_proof=>
    receipt.cells.find(cell=>cell.build===build&&cell.optional_proof===optional_proof));
  assert.equal(without.checker_stdout_sha256,withProof.checker_stdout_sha256,
    `${build}: independent proof cannot alter kernel peer output`);
}
assert.equal(receipt.proof.subject_matches_forged,false);
assert.equal(receipt.proof.attached_to_kernel_admission,false);
assert.equal(receipt.proof.validated,true);
assert.equal(receipt.proof.contract_sha256,
  receipt.source_sha256['verification/mc1/increment.noble-contract']);
assert.equal(receipt.proof.proof_source_sha256,
  receipt.source_sha256['verification/mc1/increment-proof.lean']);
assert.equal(receipt.proof.subject,'increment');
assert.equal(receipt.proof.theorem,'MC1Proof.proof');
for(const build of ['debug','release']) {
  const proofCommands=receipt.commands.filter(command=>
    command.label===(build==='debug'?'verify-unrelated-proof':'verify-unrelated-proof-release'));
  assert.equal(proofCommands.length,1,`${build} independent proof must execute once`);
  assert.equal(proofCommands[0].executable_sha256,receipt.binaries[build].cli.sha256);
  assert.deepEqual(proofCommands[0].args,[
    'verify',path.join(external,receipt.input_sha256.mc1_contract.path),
    '--proof',path.join(external,receipt.input_sha256.mc1_proof.path),
    '--timeout-ms','600000']);
  const proofReport=JSON.parse(fs.readFileSync(path.join(external,proofCommands[0].stdout),'utf8'));
  assert.deepEqual({
    schema:proofReport.schema,command:proofReport.command,
    outcome:proofReport.outcome,theorem:proofReport.theorem,
    independent_recheck:proofReport.independent_recheck,
    subject:proofReport.subject.name,ordinary_typing:proofReport.ordinary_typing.outcome,
  },{
    schema:'noble-mc1-report/v1',command:'verify',outcome:'proved',
    theorem:'MC1Proof.proof',independent_recheck:true,
    subject:'increment',ordinary_typing:'accepted',
  });
  assert.deepEqual(proofReport.diagnostics,[]);
  assert.equal(proofReport.source_path,proofCommands[0].args[1]);
  assert.equal(proofReport.proof_request.source_path,proofCommands[0].args[3]);
  assert.equal(sha(Buffer.from(proofReport.source,'utf8')),receipt.proof.contract_sha256);
  assert.equal(proofReport.proof_request.theorem,'MC1Proof.proof');
  assert.equal(proofReport.proof_request.expected_type,'MC1Obligation.claim');
  assert.equal(proofReport.consumer_policy.lean_toolchain,'leanprover/lean4:v4.31.0');
  assert.equal(proofReport.consumer_policy.lean_commit,
    '68218e876d2a38b1985b8590fff244a83c321783');
  assert.equal(proofReport.implementation_refinement.status,'not-checked-by-this-command');
  assert.equal(proofReport.backend_correspondence.status,'not-claimed');
  for(const axiom of proofReport.assumptions.accepted_transitive_axioms)
    assert.ok(['Classical.choice','propext','Quot.sound'].includes(axiom));
  assert.match(proofReport.subject.accepted_candidate,/Definition\(4\)/);
  assert.doesNotMatch(proofReport.subject.accepted_candidate,/Definition\(22\)/);
  assert.match(proofReport.subject.acceptance_request,/allowed_effects: EffSet\(\[\]\)/);
  assert.doesNotMatch(proofReport.generated_statement,/\.word 22\b|audit/u);
}
const status=JSON.parse(read('specs/STATUS.json'));
assert.deepEqual({case:status.mandatory_admission_all_build_modes.case,
  implementation:status.mandatory_admission_all_build_modes.implementation,
  execution:status.mandatory_admission_all_build_modes.execution,
  proof:status.mandatory_admission_all_build_modes.proof,
  trust:status.mandatory_admission_all_build_modes.trust,
  receipt:status.mandatory_admission_all_build_modes.receipt,
  receipt_sha256:status.mandatory_admission_all_build_modes.receipt_sha256},
  {case:'ADAPT-15',implementation:'implemented',execution:'passed',proof:'open',trust:'explicit',
    receipt:'../verification/adapt15/acceptance.json',receipt_sha256:sha(receiptBytes)});
assert.match(read('specs/ROADMAP.md').toString(),/ADAPT-15 mandatory admission across build modes/);
console.log(JSON.stringify({result:'passed',case_id:'ADAPT-15',source_revision:receipt.source_revision,
  receipt_sha256:sha(receiptBytes),cells:receipt.cells.length,proof:'open'}));
