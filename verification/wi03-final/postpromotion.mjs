#!/usr/bin/env node
// Reconstruct the entire original canonical file by reverting only WI-03's
// state/evidence insertion; all prior case rows and receipts remain immutable.
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';

const root=fs.realpathSync(path.resolve(path.dirname(fileURLToPath(import.meta.url)),'../..'));
const read=name=>fs.readFileSync(path.join(root,name));
const sha=value=>crypto.createHash('sha256').update(value).digest('hex');
const receiptPath='verification/wi03-final/acceptance.json';
const bytes=read(receiptPath),receipt=JSON.parse(bytes),digest=sha(bytes);
assert.deepEqual([receipt.schema,receipt.kind,receipt.result,receipt.failures,
  receipt.integrity_failures],['noble-wi03-final-source-body-guard/v1','test','passed',[],[]]);
assert.deepEqual(receipt.prior_prepromotion_receipt,{
  path:'verification/wi03/acceptance.json',
  sha256:'d5d9a7d891582f3247778b9417b638083a2165e762a7f1f89aa5c5fe62288f67',
  source_revision:'sha256:6ff838c42913498a7eb120a6ba1b009621afb56559b2676d9115faa33e7555f0',
  scope:'historical-only-unpromoted'});
const external=receipt.external_raw_output;
assert.ok(path.isAbsolute(external));
assert.equal(sha(fs.readFileSync(path.join(external,'acceptance.json'))),digest,
  'repository receipt is not byte-identical to external immutable receipt');
const file='specs/conformance/wit-wasi-cases.json';
const original=fs.readFileSync(path.join(external,receipt.prepromotion_case_path));
assert.equal(sha(original),receipt.prepromotion_case_sha256);
assert.equal(receipt.source_sha256[file],receipt.prepromotion_case_sha256);
const before=JSON.parse(original),text=read(file).toString('utf8'),now=JSON.parse(text);
assert.equal(before.cases.filter(row=>row.id==='WI-03').length,1);
assert.equal(now.cases.filter(row=>row.id==='WI-03').length,1);
const prior=before.cases.find(row=>row.id==='WI-03');
const current=now.cases.find(row=>row.id==='WI-03');
assert.deepEqual(Object.fromEntries(Object.keys(receipt.case).map(key=>[key,prior[key]])),receipt.case);
assert.deepEqual(Object.fromEntries(Object.keys(receipt.case).map(key=>[key,current[key]])),receipt.case);
assert.deepEqual(prior.state,{implementation:'absent',execution:'not-run',proof:'open',trust:'unassessed'});
assert.deepEqual(prior.evidence,[]);
assert.deepEqual(current.state,{implementation:'implemented',execution:'passed',proof:'open',trust:'explicit'});
assert.equal(current.evidence.length,1);
const evidence=current.evidence[0];
assert.deepEqual({kind:evidence.kind,subject:evidence.subject,result:evidence.result,
  source_revision:evidence.source_revision},
  {kind:'test',subject:'WI-03',result:'passed',source_revision:receipt.source_revision});
assert.equal(evidence.revision,now.revision);
assert.equal(evidence.configuration.receipt,'../../verification/wi03-final/acceptance.json');
assert.equal(evidence.configuration.receipt_sha256,digest);
assert.equal(evidence.configuration.prepromotion_case_sha256,receipt.prepromotion_case_sha256);
assert.equal(evidence.configuration.gate_sha256,receipt.source_sha256['verification/wi03-final/gate.mjs']);
assert.equal(evidence.configuration.external_raw_output,external);
assert.equal(evidence.configuration.binary_sha256,receipt.binaries.cli.sha256);
assert.deepEqual(evidence.assumptions,receipt.assumptions);
assert.match(evidence.claim,/default|implicit/i);
assert.match(evidence.claim,/checked|explicit/i);
const beforeLine=original.toString('utf8').split('\n').filter(line=>line.includes('"id":"WI-03"'));
const afterLine=text.split('\n').filter(line=>line.includes('"id":"WI-03"'));
assert.equal(beforeLine.length,1);assert.equal(afterLine.length,1);
assert.equal(beforeLine[0],`    ${JSON.stringify(prior)},`);
assert.equal(afterLine[0],`    ${JSON.stringify(current)},`);
assert.deepEqual({...current,state:prior.state,evidence:prior.evidence},prior);
assert.equal(text.replace(afterLine[0],beforeLine[0]),original.toString('utf8'),
  'the entire canonical preimage must reconstruct after reverting the single WI-03 row');
assert.equal(receipt.source_revision,
  `sha256:${sha(JSON.stringify(Object.entries(receipt.source_sha256).sort()))}`);
for(const [name,expected] of Object.entries(receipt.source_sha256)){
  if(name===file)continue;
  if(name==='.cairn/specs/wit-wasi/spec.md'){
    // Native sync may substitute exactly the reviewed delta's two requirements;
    // generated scenario links and every unrelated byte remain original.
    const old=fs.readFileSync(path.join(external,'prepromotion-wit-spec.md'),'utf8');
    assert.equal(sha(old),expected);
    const sections=value=>value.split(/(?=^### Requirement: )/m);
    const oldParts=sections(old),newParts=sections(read(name).toString());
    const delta=read('.cairn/changes/wit-exact-u64-adapter/specs/wit-wasi/spec.md')
      .toString('utf8');
    const deltaParts=sections(delta);
    assert.equal(newParts.length,oldParts.length,'native requirement inventory changed');
    for(let i=0;i<oldParts.length;i++){
      const id=/^### Requirement: ([A-Z0-9-]+)/.exec(oldParts[i])?.[1];
      assert.equal(/^### Requirement: ([A-Z0-9-]+)/.exec(newParts[i])?.[1],id);
      if(['WI-WIT-02','WI-WIT-06'].includes(id)){
        const revised=deltaParts.filter(part=>part.startsWith(`### Requirement: ${id}\n`));
        assert.equal(revised.length,1,`one native delta for ${id}`);
        const marker='<!-- cairn:scenario-links:start -->';
        const at=oldParts[i].indexOf(marker);
        assert.ok(at>0,`existing scenario links for ${id}`);
        const generated=oldParts[i].slice(at);
        const currentAt=newParts[i].indexOf(marker);
        assert.ok(currentAt>0,`retained scenario links for ${id}`);
        assert.equal(newParts[i].slice(currentAt),generated,
          `native ${id} generated links differ from prepromotion`);
        const currentProse=newParts[i].slice(0,currentAt).trimEnd()
          .replace(`\n\n**${id}.** `,'\n\n');
        assert.equal(currentProse,revised[0].trimEnd(),
          `native ${id} prose differs from reviewed delta`);
      }else assert.equal(newParts[i],oldParts[i],`unrelated native change: ${id}`);
    }
    continue;
  }
  const observed=read(name);
  if(name==='.cairn/changes/wit-exact-u64-adapter/tasks.md'){
    const frozen=fs.readFileSync(path.join(external,'prepromotion-native-tasks.md'),'utf8');
    assert.equal(sha(frozen),expected);
    assert.equal(observed.toString().replace(/- \[x\]/g,'- [ ]'),
      frozen.replace(/- \[x\]/g,'- [ ]'),
      'only native task checkbox completion is allowed');
  }else assert.equal(sha(observed),expected,`source changed: ${name}`);
}
for(const [name,expected] of Object.entries(receipt.historical_receipt_sha256))
  assert.equal(sha(read(name)),expected,`historical receipt changed: ${name}`);
for(const command of receipt.commands){
  assert.equal(command.error,null,`${command.label} spawn`);
  assert.equal(command.signal,null,`${command.label} signal`);
  assert.equal(command.status,command.expected_status,`${command.label} exit`);
  assert.equal(sha(fs.readFileSync(command.executable)),command.executable_sha256,
    `${command.label} executable`);
  for(const stream of ['stdout','stderr'])assert.equal(
    sha(fs.readFileSync(path.join(external,command[stream]))),command[`${stream}_sha256`],
    `${command.label} ${stream}`);
}
for(const artifact of Object.values(receipt.binaries))
  assert.equal(sha(fs.readFileSync(artifact.path)),artifact.sha256);
for(const artifact of Object.values(receipt.components)){
  assert.equal(sha(fs.readFileSync(artifact.component)),artifact.sha256);
  assert.deepEqual(artifact.report.exports,[{name:'echo',source:'export-0.noble'}]);
  assert.equal(artifact.report.checked_u64_boundary,true);
  for(const emitted of Object.values(artifact.emitted))
    assert.equal(sha(fs.readFileSync(emitted.path)),emitted.sha256);
  assert.deepEqual(artifact.guard_order,{
    internal_enter_prologue_before_ingress_guard:true,
    ingress_before_source_body:true,egress_before_publication:true,
    source_body_entries_measured:false});
}
assert.deepEqual(receipt.canonical_refusal,{stage:'binding',outcome:'lossy-conversion-reject',
  wrapped_value_exposed:false,actual_cli_stage:'Binding',actual_cli_outcome:'error',
  component_emitted:false,runtime_value_consumed:false,guest_requests:0,
  protected_operations:0});
const observations=receipt.typed_peer.checked.observations;
assert.deepEqual(observations.map(item=>item.input.value),
  ['0','42','9223372036854775807','9223372036854775808','18446744073709551615']);
for(const [i,value] of ['0','42','9223372036854775807'].entries())
  assert.deepEqual(observations[i].invocation,{outcome:'normal',result:{type:'u64',value},post_return:true});
for(const item of [...observations.slice(3),...receipt.typed_peer.negative.observations]){
  assert.equal(item.invocation.outcome,'trap');
  assert.equal(item.invocation.post_return,false);
  assert.equal(Object.hasOwn(item.invocation,'result'),false);
}
for(const [kind,artifact] of Object.entries(receipt.components))
  assert.equal(receipt.typed_peer[kind].component.sha256,artifact.sha256);
assert.deepEqual(receipt.typed_call_counts,{
  checked:{typed_calls:5,normal:3,traps:2,import_callbacks_configured:0,
    source_body_entries_measured:null},
  negative:{typed_calls:5,normal:0,traps:5,import_callbacks_configured:0,
    source_body_entries_measured:null}});
assert.deepEqual(Object.keys(receipt.rejected_signatures).sort(),
  ['async-export','mixed-export','signed-import','unsigned-import']);
console.log(JSON.stringify({result:'passed',case:'WI-03',receipt_sha256:digest,
  source_revision:receipt.source_revision,original_canonical_sha256:receipt.prepromotion_case_sha256,
  proof:'open'}));
