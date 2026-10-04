#!/usr/bin/env node
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const [destination] = process.argv.slice(2);
assert.ok(destination && process.argv.length === 3, 'usage: node verification/safety-core/handle-gate.mjs NEW_EXTERNAL_DIRECTORY');
const artifacts = path.resolve(destination);
assert.ok(artifacts !== root && !artifacts.startsWith(`${root}/`), 'artifacts must be external');
fs.mkdirSync(artifacts);
for (const name of ['home','cargo-home','tmp']) fs.mkdirSync(path.join(artifacts,name));
const sha = value => crypto.createHash('sha256').update(value).digest('hex');
const bytes = file => fs.readFileSync(path.join(root,file));
const caseFile = 'specs/conformance/safety-cases.json';
const design = JSON.parse(bytes(caseFile)).cases.find(row => row.id === 'S-CASE-03');
assert.deepEqual(design?.input,{harness:'handle-forgery',representation:42,table_entry:'absent'});
assert.deepEqual(design.expected,{stage:'adapter',outcome:'invalid-handle',protected_operations:0});
assert.deepEqual(design.state,{implementation:'absent',execution:'not-run',proof:'open',trust:'unassessed'});
assert.deepEqual(design.evidence,[]);
fs.writeFileSync(path.join(artifacts,'safety-cases.prepromotion.json'),bytes(caseFile),{flag:'wx'});
const selected = JSON.parse(bytes('policy/tool-selection.json'));
const rust = path.join(selected.tool_paths.quality_rust.output,'bin');
const environment = {...process.env,
  HOME:path.join(artifacts,'home'), CARGO_HOME:path.join(path.dirname(artifacts),'cargo-home'),
  CARGO_TARGET_DIR:path.join(path.dirname(artifacts),'target'), TMPDIR:path.join(artifacts,'tmp'),
  RUSTC_WRAPPER:'',RUSTC_WORKSPACE_WRAPPER:'',RUSTFLAGS:'',CARGO_ENCODED_RUSTFLAGS:'',
  PATH:`${rust}:${selected.component_sync.linker_bin}:/run/current-system/sw/bin`};
const sources = {};
const add = file => { sources[file] = sha(bytes(file)); };
function tree(directory) {
  for (const entry of fs.readdirSync(path.join(root,directory),{withFileTypes:true}).sort((a,b)=>a.name.localeCompare(b.name))) {
    const file = `${directory}/${entry.name}`;
    assert.equal(entry.isSymbolicLink(),false,`unbound source symlink ${file}`);
    if (entry.isDirectory()) tree(file);
    else if (entry.isFile()) add(file);
  }
}
for (const directory of ['crates/noble-kernel/src','crates/noble-contracts/src','crates/noble-wasm/src','crates/noble-cli/src']) tree(directory);
for (const file of ['Cargo.toml','Cargo.lock','rust-toolchain.toml','policy/tool-selection.json',caseFile,
  '.cairn/specs/safety/spec.md','verification/safety-core/handle-check.rs',
  'verification/safety-core/handle-gate.mjs','verification/safety-core/acceptance.json',
  'verification/release-policy/acceptance.json']) add(file);
const commands = [];
function run(label,executable,args) {
  const index=commands.length;
  const result=spawnSync(executable,args,{cwd:root,env:environment,encoding:'utf8',timeout:900_000,maxBuffer:16*1024*1024});
  const stem=`${String(index).padStart(2,'0')}-${label}`;
  fs.writeFileSync(path.join(artifacts,`${stem}.stdout`),result.stdout??'',{flag:'wx'});
  fs.writeFileSync(path.join(artifacts,`${stem}.stderr`),result.stderr??'',{flag:'wx'});
  commands.push({label,executable,executable_sha256:sha(fs.readFileSync(executable)),args,
    status:result.status,signal:result.signal,error:result.error?.message??null,
    stdout:`${stem}.stdout`,stderr:`${stem}.stderr`,
    stdout_sha256:sha(result.stdout??''),stderr_sha256:sha(result.stderr??'')});
  assert.equal(result.error,undefined,`${label} spawn: ${result.error}`);
  assert.equal(result.signal,null,`${label} signal: ${result.signal}`);
  assert.equal(result.status,0,`${label} failed: ${result.stderr}`);
  return result.stdout;
}
const cargoOutput=run('build',path.join(rust,'cargo'),['build','-p','noble-kernel','--locked','--offline','--message-format=json']);
const artifactsFound=cargoOutput.split('\n').filter(line=>line.startsWith('{')).map(line=>JSON.parse(line))
  .filter(item=>item.reason==='compiler-artifact' && item.target.name==='noble_kernel' && item.target.kind.includes('lib'))
  .flatMap(item=>item.filenames.filter(file=>file.endsWith('.rlib')));
assert.equal(artifactsFound.length,1,'unique compiled production kernel rlib');
const runner=path.join(artifacts,'handle-check');
run('check-build',path.join(rust,'rustc'),['--edition=2021','--crate-name','safety_handle_check',
  'verification/safety-core/handle-check.rs','--extern',`noble_kernel=${artifactsFound[0]}`,
  '-L',`dependency=${path.join(environment.CARGO_TARGET_DIR,'debug/deps')}`,'-o',runner]);
const observed=JSON.parse(run('handle-forgery',runner,[String(design.input.representation)]).trim());
for (const [key,value] of Object.entries(design.expected)) assert.deepEqual(observed[key],value,`S-CASE-03.${key}`);
assert.equal(observed.positive_registered_owner,true);
assert.equal(observed.absent_after_registration,true);
for (const [file,digest] of Object.entries(sources)) assert.equal(sha(bytes(file)),digest,`source changed mid-flight: ${file}`);
const receipt={schema:'noble-safety-handle-receipt/v1',result:'passed',kind:'test',
  cases:[{id:design.id,input:design.input,expected:design.expected}],
  claim:'The exact absent representation 42 rejects in the retained production resource Table without protected operations; a host-registered owner passes validation while slot 42 stays absent. No native storage or general proof is claimed.',
  source_revision:`sha256:${sha(JSON.stringify(Object.entries(sources).sort()))}`,
  source_sha256:sources,prepromotion_case_sha256:sources[caseFile],
  binary:{path:runner,sha256:sha(fs.readFileSync(runner))},selected_rust:selected.tool_paths.quality_rust,
  commands,observed,assumptions:['The trusted host assigns unique table and context identities and independently owns any native resource before registration.',
    'Read-only Table::validate rejects unregistered slot 42; neither validation nor this probe starts native work.',
    'Proof remains open; no native memory-safety or universal non-forgeability theorem is asserted.']};
fs.writeFileSync(path.join(artifacts,'acceptance.json'),JSON.stringify(receipt,null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({result:receipt.result,receipt:path.join(artifacts,'acceptance.json'),
  source_revision:receipt.source_revision,prepromotion_case_sha256:receipt.prepromotion_case_sha256,observed}));
