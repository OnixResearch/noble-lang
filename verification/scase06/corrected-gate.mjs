#!/usr/bin/env node
// New chronological source freeze after the original S-CASE-06 receipt.
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import {spawnSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'../..');
assert.equal(process.argv.length,3,'usage: node verification/scase06/corrected-gate.mjs NEW_EXTERNAL_DIRECTORY');
const output=path.resolve(process.argv[2]);
assert.ok(output!==root&&!output.startsWith(`${root}/`),'external output required');
const sha=value=>crypto.createHash('sha256').update(value).digest('hex');
const read=name=>fs.readFileSync(path.join(root,name));
const caseFile='specs/conformance/safety-cases.json';
const design=JSON.parse(read(caseFile)).cases.find(row=>row.id==='S-CASE-06');
assert.deepEqual(design.input,{harness:'authority-denial',source:'"main.rs" fs.read',declared_effects:['fs.read'],directory:{kind:'Directory',context:'invocation-1',live:true,read_right:false}});
assert.deepEqual(design.expected,{stage:'authorization',outcome:'denied',guest_requests:1,request_trace:['fs.read'],protected_operations:0});
assert.deepEqual(design.state,{implementation:'implemented',execution:'passed',proof:'open',trust:'explicit'});
assert.equal(design.evidence.length,1,'first receipt must stay historical');
assert.equal(design.evidence[0].configuration.receipt_sha256,
  '5fed58285741886c1c019dc07878e7b4c8d138d9110d240b78a0c25de7d299d8');
assert.equal(sha(read('verification/scase06/acceptance.json')),
  design.evidence[0].configuration.receipt_sha256);
assert.equal(sha(read('verification/scase06/gate.mjs')),
  design.evidence[0].configuration.gate_sha256);
const selected=JSON.parse(read('policy/tool-selection.json'));
const rust=path.join(selected.tool_paths.quality_rust.output,'bin');
const node=path.join(selected.tool_paths.node.output,'bin/node');
assert.equal(fs.realpathSync(process.execPath),fs.realpathSync(node),'selected Node required');
const sources={};
function source(file){sources[file]=sha(read(file));}
function tree(directory){
  for(const entry of fs.readdirSync(path.join(root,directory),{withFileTypes:true}).sort((a,b)=>a.name.localeCompare(b.name))){
    const file=`${directory}/${entry.name}`;
    assert.equal(entry.isSymbolicLink(),false,`unbound source symlink: ${file}`);
    if(entry.isDirectory())tree(file);
    else if(entry.isFile())source(file);
  }
}
for(const directory of ['crates/noble-kernel/src','crates/noble-contracts/src','crates/noble-wasm/src','crates/noble-cli/src','verification/scase06','.cairn/changes/safety-authorized-fs-read'])tree(directory);
for(const file of ['Cargo.toml','Cargo.lock','rust-toolchain.toml',
  'crates/noble-kernel/Cargo.toml','crates/noble-contracts/Cargo.toml',
  'crates/noble-wasm/Cargo.toml','crates/noble-cli/Cargo.toml',
  'crates/noble-wasm/wit/authorized-fs.wit',
  '.cairn/specs/safety/spec.md','.cairn/specs/resource-adapters/spec.md',
  '.cairn/specs/evidence/spec.md',caseFile,'policy/tool-selection.json'])source(file);
const historical={};
function receipts(directory){
  for(const item of fs.readdirSync(path.join(root,directory),{withFileTypes:true})){const file=`${directory}/${item.name}`;
    if(item.isDirectory())receipts(file);
    else if(item.isFile()&&/^(corrected-)?acceptance\.json$/.test(item.name)
      &&file!=='verification/scase06/corrected-acceptance.json')historical[file]=sha(read(file));
  }
}
receipts('verification');
fs.mkdirSync(output);
fs.writeFileSync(path.join(output,'prepromotion-safety-cases.json'),read(caseFile),{flag:'wx'});
fs.writeFileSync(path.join(output,'source-tasks.md'),
  read('.cairn/changes/safety-authorized-fs-read/tasks.md'),{flag:'wx'});
const temp=path.join(output,'tmp');fs.mkdirSync(temp);
const environment={...process.env,PATH:`${rust}:${selected.component_sync.linker_bin}:${selected.tool_paths.node.output}/bin:${process.env.PATH}`,
  TMPDIR:temp,RUSTC_WRAPPER:'',RUSTC_WORKSPACE_WRAPPER:'',CARGO_TARGET_DIR:path.join(output,'target'),NIX_CONFIG:'min-free = 0'};
const commands=[];
function run(label,program,args,expectedStatus=0){
  const result=spawnSync(program,args,{cwd:root,env:environment,encoding:'utf8',timeout:900_000,maxBuffer:16*1024*1024});
  const stem=`${String(commands.length).padStart(2,'0')}-${label}`;
  fs.writeFileSync(path.join(output,`${stem}.stdout`),result.stdout??'',{flag:'wx'});
  fs.writeFileSync(path.join(output,`${stem}.stderr`),result.stderr??'',{flag:'wx'});
  commands.push({label,executable:program,executable_sha256:sha(fs.readFileSync(program)),args,status:result.status,
    signal:result.signal,error:result.error?.message??null,stdout:`${stem}.stdout`,stderr:`${stem}.stderr`,
    stdout_sha256:sha(result.stdout??''),stderr_sha256:sha(result.stderr??'')});
  assert.equal(result.error,undefined,`${label} spawn failed`);
  assert.equal(result.signal,null,`${label} signaled`);
  assert.equal(result.status,expectedStatus,`${label}: ${result.stderr}`);
  return result.stdout;
}
run('production-build',path.join(rust,'cargo'),['build','-p','noble-cli','--locked','--offline','-j','4']);
const cli=path.join(environment.CARGO_TARGET_DIR,'debug/noble');
const guest=path.join(output,'guest.noble');
fs.writeFileSync(guest,`${design.input.source}\n`,{flag:'wx'});
const fixture=path.join(output,'main.rs');
const fileBytes=Buffer.from('safe capability-scoped bytes\n');
fs.writeFileSync(fixture,fileBytes,{flag:'wx'});
const wit='crates/noble-wasm/wit/authorized-fs.wit';
const binding=JSON.parse(run('typed-bindings',cli,['component','bindings',wit,'bounded']));
assert.equal(binding.world,'noble-test:authorized-fs/bounded@1.0.0');
assert.deepEqual(binding.imports.map(row=>row.word),['fs.read']);
assert.deepEqual(binding.exports.map(row=>row.word),['read-file']);
const componentDirectory=path.join(output,'component');
const compiled=JSON.parse(run('compile-guest',cli,['component','compile',wit,'bounded',componentDirectory,`read-file=${guest}`]));
assert.equal(compiled.outcome,'compiled');
assert.equal(compiled.independent_kernel_check,true);
assert.equal(compiled.component_emitted,true);
const component=path.join(componentDirectory,'component.wasm');
const componentSha=sha(fs.readFileSync(component));
const observations=[];
function invocation(label,grant,expected){
  const observed=JSON.parse(run(label,cli,['component','read-fs',component,fixture,grant,
    wit,'bounded',`read-file=${guest}`]));
  assert.equal(observed.schema,'noble-authorized-fs/v1');
  assert.equal(observed.context,'invocation-1');
  assert.equal(observed.released_owners,1);
  assert.equal(observed.live_owners,0);
  assert.equal(observed.native_pins,0);
  for(const [key,value] of Object.entries(expected))assert.deepEqual(observed[key],value,`${label}.${key}`);
  observations.push({label,grant,observed});return observed;
}
const denied=invocation('canonical-scase06','none',{...design.expected,bytes_hex:null});
const positive=invocation('same-compiled-guest-positive','read',{stage:'adapter',outcome:'read',
  guest_requests:1,request_trace:['fs.read'],protected_operations:1,bytes_hex:fileBytes.toString('hex')});
const hostileSource=path.join(output,'escape.noble');
fs.writeFileSync(hostileSource,'"../main.rs" fs.read\n',{flag:'wx'});
const hostileDirectory=path.join(output,'escape-component');
assert.equal(JSON.parse(run('compile-hostile-path',cli,['component','compile',wit,'bounded',
  hostileDirectory,`read-file=${hostileSource}`])).outcome,'compiled');
const hostileComponent=path.join(hostileDirectory,'component.wasm');
const pathRefusal=JSON.parse(run('compiled-path-escape',cli,['component','read-fs',
  hostileComponent,fixture,'read',wit,'bounded',`read-file=${hostileSource}`]));
assert.deepEqual({
  outcome:pathRefusal.outcome,guest_requests:pathRefusal.guest_requests,
  request_trace:pathRefusal.request_trace,protected_operations:pathRefusal.protected_operations,
  released_owners:pathRefusal.released_owners,bytes_hex:pathRefusal.bytes_hex
},{outcome:'invalid-path',guest_requests:1,request_trace:['fs.read'],
  protected_operations:0,released_owners:1,bytes_hex:null});
const mismatch=run('mismatched-host-source-refusal',cli,['component','read-fs',
  component,path.join(output,'not-opened'),'read',wit,'bounded',`read-file=${hostileSource}`],2);
assert.equal(mismatch,'');
assert.match(fs.readFileSync(path.join(output,`${String(commands.length-1).padStart(2,'0')}-mismatched-host-source-refusal.stderr`),'utf8'),
  /component differs from independent host-selected Noble source recipe/);
const tests=run('hostile-right-handle-path-controls',path.join(rust,'cargo'),
  ['test','-p','noble-cli','--locked','--offline','--bin','noble','component::authorized_fs::tests','--','--test-threads','1']);
assert.match(tests,/authorization_and_hostile_claims_never_read \.\.\. ok/);
assert.match(tests,/1 passed; 0 failed/);
assert.equal(sha(fs.readFileSync(component)),componentSha,'canonical component changed during invocations');
for(const [file,digest] of Object.entries(sources))assert.equal(sha(read(file)),digest,`source changed: ${file}`);
for(const [file,digest] of Object.entries(historical))assert.equal(sha(read(file)),digest,`historical receipt changed: ${file}`);
const receipt={schema:'noble-scase06-authorized-fs-corrected/v1',kind:'test',result:'passed',
  failures:[],integrity_failures:[],case:{id:design.id,input:design.input,expected:design.expected},
  source_revision:`sha256:${sha(JSON.stringify(Object.entries(sources).sort()))}`,source_sha256:sources,
  prepromotion_case_sha256:sources[caseFile],historical_receipt_sha256:historical,
  original_receipt_sha256:historical['verification/scase06/acceptance.json'],
  selected_rust:selected.tool_paths.quality_rust,selected_node:selected.tool_paths.node,
  guest_source_sha256:sha(fs.readFileSync(guest)),fixture_sha256:sha(fileBytes),component_sha256:componentSha,
  binary_sha256:sha(fs.readFileSync(cli)),commands,observations,
  claim:'Exact canonical compiled Noble fs.read traversed Wasmtime imported callback once. A live invocation-scoped host Directory with no READ was denied at authorization without a protected read; the same component with independent host READ returned bytes from its preopened exact file. Direct production host controls reject absent right, forged handle and guest path escape.',
  assumptions:['The host invoker independently selects and preopens one regular file; guest path is compared to main.rs, never resolved or joined into a native path.',
    'A unique host TableId/Context and move-only owner are retained for each invocation. Missing READ creates no protected operation; the read is bounded to 4097 physical bytes and only up to 4096 bytes are delivered.',
    'Selected Rust, Noble compiler, Wasmtime and Canonical ABI are trusted for this finite observation. General filesystem safety, owner law, Octet, Aeneas and proof claims remain open.']};
fs.writeFileSync(path.join(output,'acceptance.json'),JSON.stringify(receipt,null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({result:'passed',receipt:path.join(output,'acceptance.json'),source_revision:receipt.source_revision,
  receipt_sha256:sha(fs.readFileSync(path.join(output,'acceptance.json'))),component_sha256:componentSha,commands:commands.length}));
