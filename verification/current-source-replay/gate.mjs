#!/usr/bin/env node
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

// Plan is read-only. Execution requires a separately reviewed, complete
// kernel + host source revision; neither mode changes a canonical case or an
// earlier receipt. Run only after both source owners declare their freeze.
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const sha = value => crypto.createHash('sha256').update(value).digest('hex');
const bytes = file => fs.readFileSync(path.join(root, file));
const caseFiles = {
  'DX-01': 'specs/conformance/developer-experience-cases.json',
  'ADAPT-01': 'specs/conformance/adaptation-cases.json',
  'S-CASE-01': 'specs/conformance/safety-cases.json',
  'S-CASE-03': 'specs/conformance/safety-cases.json',
  'S-CASE-05': 'specs/conformance/safety-cases.json',
  'S-CASE-14': 'specs/conformance/safety-cases.json',
};
const priorPaths = {
  'DX-01': 'verification/dx01/current-frozen.json',
  'ADAPT-01': 'verification/adapt-01/current-frozen.json',
  'S-CASE-01': 'verification/safety-core/acceptance.json',
  'S-CASE-03': 'verification/safety-core/handle-acceptance.json',
  'S-CASE-05': 'verification/safety-core/acceptance.json',
  'S-CASE-14': 'verification/safety-core/acceptance.json',
};
const priorDigests = {
  'verification/dx01/current-frozen.json': '50a4eed7fa6883b02ca1134dbdaf74ff673e7001df62daf0ef1894d6802e1f73',
  'verification/adapt-01/current-frozen.json': 'fd650ec7451bbf54ed27c90606798e91c4f2837849f8835fccd9d44365749e32',
  'verification/safety-core/acceptance.json': '733b561741cd71790a8bc0f2231f91643155e424440844966c8e342f19d39289',
  'verification/safety-core/handle-acceptance.json': '8c15f03fb0d3724c5b7ee9e8960b6f6336d9ca77d3e21a780b064ed2481f4aa5',
};
const prior = Object.fromEntries(Object.entries(priorDigests).map(([file, digest]) => {
  const content = bytes(file);
  assert.equal(sha(content), digest, `immutable receipt changed: ${file}`);
  return [file, JSON.parse(content)];
}));
const packets = Object.fromEntries([...new Set(Object.values(caseFiles))].map(file => [file, JSON.parse(bytes(file))]));
const cases = Object.fromEntries(Object.entries(caseFiles).map(([id, file]) => {
  const row = packets[file].cases.find(item => item.id === id);
  assert.ok(row, `missing canonical ${id}`);
  assert.deepEqual(row.state, {implementation:'implemented',execution:'passed',proof:'open',trust:'explicit'});
  assert.equal(row.evidence.length, 1, `${id} must still have exactly its historical evidence before replay`);
  assert.equal(row.evidence[0].subject, id);
  assert.equal(row.evidence[0].kind, 'test');
  assert.equal(row.evidence[0].result, 'passed');
  assert.equal(row.evidence[0].configuration.receipt, path.posix.relative(path.posix.dirname(file), priorPaths[id]));
  const oldReceipt = prior[priorPaths[id]];
  assert.equal(row.evidence[0].source_revision, oldReceipt.source_revision);
  if (id.startsWith('S-CASE-')) {
    assert.equal(row.evidence[0].configuration.receipt_sha256, priorDigests[priorPaths[id]]);
    const frozen = oldReceipt.cases.find(item => item.id === id);
    assert.deepEqual({id:row.id,input:row.input,expected:row.expected}, frozen);
  }
  return [id, row];
}));
assert.deepEqual(cases['DX-01'].input.variants.map(row => row.source), [
  '1 true +', 'true [ 1 ] [ false ] if', 'dup',
]);
assert.deepEqual(cases['DX-01'].input.variants.map(row => row.failure), [
  'word-input-type', 'branch-output-type', 'resource-duplication',
]);
assert.deepEqual(cases['DX-01'].input.variants[2].input_stack, ['Resource<test.counter>']);
assert.equal(prior[priorPaths['DX-01']].cli_run.stdin_utf8,
  cases['DX-01'].input.variants.slice(0, 2).map(row => `${row.source}\n`).join(''));
assert.equal(prior[priorPaths['DX-01']].typed_source_check.source, cases['DX-01'].input.variants[2].source);
assert.deepEqual(cases['ADAPT-01'].input.variants.map(row => row.failure),
  ['output-type','output-arity','output-order']);
assert.equal(prior[priorPaths['ADAPT-01']].rejection_run.stdin_utf8,
  cases['ADAPT-01'].input.variants.map(row => `${row.source}\n`).join(''));
assert.equal(prior[priorPaths['ADAPT-01']].compatible_control.stdin_utf8,
  'true [ 1 ] [ 2 ] if\n');
for (const id of ['S-CASE-01','S-CASE-05','S-CASE-14']) assert.equal(cases[id].kind, 'static');
assert.equal(cases['S-CASE-03'].kind, 'adapter');

const selected = JSON.parse(bytes('policy/tool-selection.json'));
const rust = path.join(selected.tool_paths.quality_rust.output, 'bin');
const sources = {};
const add = file => { sources[file] = sha(bytes(file)); };
function tree(directory) {
  for (const entry of fs.readdirSync(path.join(root,directory), {withFileTypes:true}).sort((a,b)=>a.name.localeCompare(b.name))) {
    const file = `${directory}/${entry.name}`;
    assert.equal(entry.isSymbolicLink(), false, `unbound source symlink: ${file}`);
    if (entry.isDirectory()) tree(file);
    else if (entry.isFile()) add(file);
  }
}
for (const directory of ['crates/noble-kernel/src','crates/noble-contracts/src',
  'crates/noble-wasm/src','crates/noble-cli/src']) tree(directory);
for (const file of ['Cargo.toml','Cargo.lock','rust-toolchain.toml','policy/tool-selection.json',
  'crates/noble-kernel/Cargo.toml','crates/noble-contracts/Cargo.toml',
  'crates/noble-wasm/Cargo.toml','crates/noble-cli/Cargo.toml',
  'crates/noble-syndicate/Cargo.toml',
  ...new Set(Object.values(caseFiles)), ...Object.keys(priorDigests),
  '.cairn/specs/safety/spec.md','.cairn/specs/developer-experience/spec.md',
  '.cairn/specs/core-bootstrap/spec.md',
  'crates/noble-contracts/tests/source.rs','crates/noble-cli/tests/smoke.rs',
  'verification/safety-core/check.rs','verification/safety-core/gate.mjs',
  'verification/safety-core/handle-check.rs','verification/safety-core/handle-gate.mjs',
  'verification/safety-core/postpromotion.mjs','verification/safety-core/handle-postpromotion.mjs',
  'verification/release-policy/postpromotion.mjs','verification/decoder-experiment/postpromotion.mjs',
  'verification/current-source-replay/diagnostic-check.rs',
  'verification/current-source-replay/projection.mjs',
  'verification/current-source-replay/projection.test.mjs',
  'verification/current-source-replay/postpromotion.mjs',
  'verification/current-source-replay/gate.mjs']) add(file);
const sourceRevision = `sha256:${sha(JSON.stringify(Object.entries(sources).sort()))}`;
if (process.argv.length === 3 && process.argv[2] === '--plan') {
  console.log(JSON.stringify({result:'ready-for-owner-review-not-execution', source_revision:sourceRevision,
    source_files:Object.keys(sources).length, selected_rust:selected.tool_paths.quality_rust,
    cases:Object.keys(cases), runs:['compiled-cli-sessions','typed-source-independent-kernel',
      'safety-core-source-and-kernel','production-resource-table','historical-postpromotions'],
    requires:['explicit KERNEL SOURCE FREEZE','explicit HOST SOURCE FREEZE',
      'new external directory and the reviewed source_revision argument']}));
  process.exit(0);
}
const [destination, expectedRevision] = process.argv.slice(2);
assert.equal(process.argv.length, 4,
  'usage: bun verification/current-source-replay/gate.mjs NEW_EXTERNAL_DIRECTORY sha256:REVIEWED_FROZEN_SOURCE_REVISION');
assert.match(expectedRevision, /^sha256:[0-9a-f]{64}$/);
assert.equal(sourceRevision, expectedRevision, 'kernel/host source differs from separately reviewed freeze');
const artifacts = path.resolve(destination);
assert.ok(artifacts !== root && !artifacts.startsWith(`${root}/`), 'artifacts must be external');
fs.mkdirSync(artifacts);
for (const name of ['home','tmp','inputs']) fs.mkdirSync(path.join(artifacts,name));
const cargoHome = path.join(path.dirname(artifacts),'cargo-home');
fs.mkdirSync(cargoHome,{recursive:true});
for (const file of Object.keys(packets)) {
  fs.writeFileSync(path.join(artifacts,'inputs',path.basename(file)),bytes(file),{flag:'wx'});
}
const environment = {...process.env,
  HOME:path.join(artifacts,'home'), CARGO_HOME:cargoHome,
  CARGO_TARGET_DIR:path.join(artifacts,'target'),TMPDIR:path.join(artifacts,'tmp'),
  RUSTC_WRAPPER:'',RUSTC_WORKSPACE_WRAPPER:'',RUSTFLAGS:'',CARGO_ENCODED_RUSTFLAGS:'',
  PATH:`${rust}:${selected.component_sync.linker_bin}:/run/current-system/sw/bin`};
const commands = [];
function run(label, executable, args, expected=0, input) {
  const response = spawnSync(executable,args,{cwd:root,env:environment,input,
    timeout:900_000,maxBuffer:16*1024*1024});
  const stem=`${String(commands.length).padStart(2,'0')}-${label}`;
  fs.writeFileSync(path.join(artifacts,`${stem}.stdout`),response.stdout??Buffer.alloc(0),{flag:'wx'});
  fs.writeFileSync(path.join(artifacts,`${stem}.stderr`),response.stderr??Buffer.alloc(0),{flag:'wx'});
  const recorded={label,executable,executable_sha256:sha(fs.readFileSync(executable)),args,
    stdin_sha256:input===undefined?null:sha(input),status:response.status,signal:response.signal,
    error:response.error?.message??null, stdout:`${stem}.stdout`,stderr:`${stem}.stderr`,
    stdout_sha256:sha(response.stdout??Buffer.alloc(0)),
    stderr_sha256:sha(response.stderr??Buffer.alloc(0))};
  commands.push(recorded);
  assert.equal(recorded.error,null,`${label} spawn: ${recorded.error}`);
  assert.equal(recorded.signal,null,`${label} signal: ${recorded.signal}`);
  assert.equal(recorded.status,expected,`${label}: ${response.stderr?.toString()}`);
  return response.stdout.toString('utf8');
}
function inputFile(name,source) {
  const file = path.join(artifacts,'inputs',`${name}.noble`);
  fs.writeFileSync(file,source,{flag:'wx'});
  return file;
}
const safetyInputs = [inputFile('S-CASE-01',cases['S-CASE-01'].input.source),
  inputFile('S-CASE-05',cases['S-CASE-05'].input.source),
  ...cases['S-CASE-14'].input.sources.map((source,i)=>inputFile(`S-CASE-14-${i}`,source))];
assert.equal(safetyInputs.length,4);
const build = run('build',path.join(rust,'cargo'),['build','-p','noble-cli','-p','noble-contracts',
  '--all-features','--locked','--offline','--message-format=json']);
function artifact(name,kind) {
  const matches=build.split('\n').filter(line=>line.startsWith('{')).map(line=>JSON.parse(line))
    .filter(row=>row.reason==='compiler-artifact' && row.target.name===name && row.target.kind.includes(kind));
  const files=[...new Set(matches.flatMap(row=>kind==='bin'?[row.executable]:
    row.filenames.filter(file=>file.endsWith('.rlib'))).filter(Boolean))];
  assert.equal(files.length,1,`unique compiled ${name}/${kind}`);
  return files[0];
}
const cli = artifact('noble','bin');
const contracts = artifact('noble_contracts','lib');
const kernel = artifact('noble_kernel','lib');
function compilePeer(name,file,extern) {
  const output=path.join(artifacts,name);
  run(`build-${name}`,path.join(rust,'rustc'),['--edition=2021','--crate-name',name.replaceAll('-','_'),
    file,...extern.flatMap(([name,library])=>['--extern',`${name}=${library}`]),
    '-L',`dependency=${path.join(environment.CARGO_TARGET_DIR,'debug/deps')}`,'-o',output]);
  return output;
}
const diagnosticPeer=compilePeer('diagnostic-check','verification/current-source-replay/diagnostic-check.rs',
  [['noble_contracts',contracts],['noble_kernel',kernel]]);
const safetyPeer=compilePeer('safety-core-check','verification/safety-core/check.rs',
  [['noble_contracts',contracts],['noble_kernel',kernel]]);
const handlePeer=compilePeer('safety-handle-check','verification/safety-core/handle-check.rs',
  [['noble_kernel',kernel]]);
const reportLines=(label,argv,stdin,exit,length)=>{
  const output=run(label,cli,argv,exit,Buffer.from(stdin));
  const lines=output.trim().split('\n').map(line=>JSON.parse(line));
  assert.equal(lines.length,length,`${label} report count`);
  return lines;
};
const verifyFields=(observed,expected,label)=>{
  for(const [field,value] of Object.entries(expected)) assert.deepEqual(observed[field],value,`${label}.${field}`);
};
const dxOld=prior[priorPaths['DX-01']];
const dxReports=reportLines('dx01-session',['session'],dxOld.cli_run.stdin_utf8,dxOld.cli_run.exit_code,2);
for(let i=0;i<dxReports.length;i++) {
  const {source,failure,...expected}=dxOld.cli_run.reports[i];
  assert.equal(source,cases['DX-01'].input.variants[i].source);
  assert.equal(failure,cases['DX-01'].input.variants[i].failure);
  verifyFields(dxReports[i],expected,`DX-01/${failure}`);
}
const typed=JSON.parse(run('dx01-typed-resource',diagnosticPeer,[cases['DX-01'].input.variants[2].source]).trim());
const {path:oldApiPath,resource_identity,resource_kind,operation_observation,...oldTyped}=dxOld.typed_source_check;
assert.equal(oldApiPath,'source::ModuleSession::new([]).prepare(source, [Ty::Resource(FIXTURE_RESOURCE)], limits)');
assert.equal(resource_kind,'ResourceKind(0)');
assert.ok(resource_identity.includes('FIXTURE_RESOURCE'));
assert.ok(operation_observation.includes('before producing a candidate'));
verifyFields(typed,oldTyped,'DX-01/typed-resource');
const adaptOld=prior[priorPaths['ADAPT-01']];
const adaptReports=reportLines('adapt01-session',['session'],adaptOld.rejection_run.stdin_utf8,
  adaptOld.rejection_run.exit_code,3);
for(let i=0;i<adaptReports.length;i++) {
  const {source,failure,...expected}=adaptOld.rejection_run.reports[i];
  assert.equal(source,cases['ADAPT-01'].input.variants[i].source);
  assert.equal(failure,cases['ADAPT-01'].input.variants[i].failure);
  verifyFields(adaptReports[i],expected,`ADAPT-01/${failure}`);
}
const adaptPositive=reportLines('adapt01-compatible',['session'],adaptOld.compatible_control.stdin_utf8,
  adaptOld.compatible_control.exit_code,1)[0];
for(const field of ['stage','outcome','stack','guest_requests','protected_operations'])
  assert.deepEqual(adaptPositive[field],adaptOld.compatible_control[field],`ADAPT-01/positive.${field}`);
const safetyOutput=run('safety-source-and-kernel',safetyPeer,safetyInputs);
const safetyReports=safetyOutput.trim().split('\n').map(line=>JSON.parse(line));
assert.deepEqual(safetyReports.map(row=>row.id),['S-CASE-01','S-CASE-05','S-CASE-14']);
for(const observed of safetyReports) verifyFields(observed,cases[observed.id].expected,observed.id);
assert.equal(safetyReports[1].constraint,'EffectInclusion(test.emit)');
assert.equal(safetyReports[2].variants,cases['S-CASE-14'].input.sources.length);
const safetyCli=[];
for (const [label,file,design] of [
  ['S-CASE-01',safetyInputs[0],cases['S-CASE-01']],
  ['S-CASE-14-0',safetyInputs[2],cases['S-CASE-14']],
  ['S-CASE-14-1',safetyInputs[3],cases['S-CASE-14']],
]) {
  const observed=JSON.parse(run(`cli-${label}`,cli,['compile',file],2).trim());
  verifyFields(observed,design.expected,label);
  assert.equal(observed.candidate_prepare_requests,0,`${label} prepared guest requests`);
  safetyCli.push({id:label,stage:observed.stage,outcome:observed.outcome,
    guest_requests:observed.guest_requests,protected_operations:observed.protected_operations});
}
const handle=JSON.parse(run('safety-handle-forgery',handlePeer,
  [String(cases['S-CASE-03'].input.representation)]).trim());
verifyFields(handle,cases['S-CASE-03'].expected,'S-CASE-03');
assert.equal(handle.positive_registered_owner,true);
assert.equal(handle.absent_after_registration,true);
for (const runner of ['verification/safety-core/postpromotion.mjs',
  'verification/safety-core/handle-postpromotion.mjs',
  'verification/release-policy/postpromotion.mjs',
  'verification/decoder-experiment/postpromotion.mjs']) {
  const result=JSON.parse(run(`historical-${path.basename(path.dirname(runner))}-${path.basename(runner)}`,
    process.execPath,[runner]).trim());
  assert.equal(result.result,'passed',runner);
}
for(const [file,digest] of Object.entries(sources))
  assert.equal(sha(bytes(file)),digest,`source changed during replay: ${file}`);
const receipt={schema:'noble-current-source-replay/v1',result:'passed',kind:'test',
  cases:Object.entries(cases).map(([id,row])=>({id,input:row.input,expected:row.expected,
    previous_receipt:priorPaths[id],previous_receipt_sha256:priorDigests[priorPaths[id]]})),
  claim:'Six selected DX-01/ADAPT-01 and S-CASE-01/03/05/14 cases replayed against one frozen kernel/host source inventory, freshly compiled CLI, independent source/kernel peers and retained production resource Table. Historical receipts retain only their original source scope.',
  source_revision:sourceRevision,source_sha256:sources,
  prepromotion_case_sha256:Object.fromEntries(Object.keys(packets).map(file=>[file,sources[file]])),
  selected_rust:selected.tool_paths.quality_rust,
  binary:{path:cli,sha256:sha(fs.readFileSync(cli))},
  peers:Object.fromEntries([diagnosticPeer,safetyPeer,handlePeer].map(file=>[path.basename(file),{path:file,sha256:sha(fs.readFileSync(file))}])),
  commands,observed:{dx01_cli:dxReports,dx01_typed:typed,adapt01_cli:adaptReports,
    adapt01_compatible:adaptPositive,safety_independent:safetyReports,safety_cli:safetyCli,safety_handle:handle},
  assumptions:['Pinned selected Rust toolchain, compiled CLI, independent production source/kernel and production resource Table are trusted only at the recorded exact source/binary identities.',
    'The negative paths do not enter guest execution; S-CASE-03 validates an absent slot and one trusted registration, not a general handle non-forgeability or native-memory theorem.',
    'The original four receipts and their raw historical observations remain separate source-revision evidence; broad Lean, host, backend and whole-source refinement remains open.']};
fs.writeFileSync(path.join(artifacts,'acceptance.json'),JSON.stringify(receipt,null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({result:'passed',receipt:path.join(artifacts,'acceptance.json'),
  source_revision:sourceRevision,binary:receipt.binary,cases:receipt.cases.map(row=>row.id),
  commands:commands.length,failures:0,integrity_failures:0}));
