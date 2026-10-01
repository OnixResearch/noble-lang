#!/usr/bin/env node
// Runs the actual Noble compiler-produced import-free Core module through the
// production Wasmtime quota CLI. No document validation or prior receipt is
// substituted for a guest-call fuel or generated-heap failure.
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'../..');
assert.equal(process.argv.length,3,'usage: SELECTED_NODE verification/scase13/gate.mjs NEW_EXTERNAL_DIRECTORY');
const destination=path.resolve(process.argv[2]);
assert.ok(destination!==root&&!destination.startsWith(`${root}/`),'output must be external');
const sha=value=>crypto.createHash('sha256').update(value).digest('hex');
const content=file=>fs.readFileSync(path.join(root,file));
const selected=JSON.parse(content('policy/tool-selection.json'));
const node=path.join(selected.tool_paths.node.output,'bin/node');
assert.equal(fs.realpathSync(process.execPath),fs.realpathSync(node),'selected Node required');
assert.deepEqual(process.execArgv,[],'unreviewed Node flags');
const caseFile='specs/conformance/safety-cases.json';
const design=JSON.parse(content(caseFile)).cases.find(row=>row.id==='S-CASE-13');
assert.ok(design,'missing canonical S-CASE-13');
assert.deepEqual(design.input,{harness:'quota-exhaustion',fuel:1,allocation_limit_bytes:64,
  cases:['fuel-exhaustion','allocation-exhaustion']});
assert.deepEqual(design.expected,{stage:'execution',outcome:'specified-quota-failure',reported_success:false});
assert.deepEqual(design.state,{implementation:'absent',execution:'not-run',proof:'open',trust:'unassessed'});
assert.deepEqual(design.evidence,[]);
const frozen={
  'verification/scase07/acceptance.json':'6bb9ee9bf3d9ed1077818426d29a1bb9895fe1733e1d8bbb3041e3d7fa2013d5',
  'verification/scase07/gate.mjs':'61fe169bb1c5fe73d147a9b31471200d0554a00c2793a41d4397f7590a7717a3',
  'verification/scase07-current-source/dx06-acceptance.json':'3b9e726f1d9204655b72289228c3321d158de336381130b66ef8228a26af7509',
  'verification/scase07-current-source/acceptance.json':'b20e176c5cbe4ca7af997771b58ad569e76baae783bb3c7a36f7fdef7292fbc1',
  'verification/scase07-current-source/gate.mjs':'599d4d05cf0ddbc8dc97998fa375b92339ae8cdc8fe8d3d49178dab18a966dbc',
  'verification/scase02/acceptance.json':'23f2b73aff9ee4dfa0bbd7d613dba3e569caa316409a49abe9f7884eddcec0b1',
  'verification/scase02/gate.mjs':'43d96df4ae68b0fc2a3d9f15f1664cb162d4a9f1addd16287c3658b0150bdab3',
  'verification/scase02/corrected-acceptance.json':'3588522e32149a490ea0ec8b7c33abbcb229150b91e24a31d164a17b60a740d0',
  'verification/scase02/corrected-gate.mjs':'d72677105b3d3fa6d9b5f2647ecabead147981e7c1a5ee66578a144136ac3d79',
  'verification/scase16/acceptance.json':'7534cfc6501b223e80e04ed0042fdf21fead5ffe246e4528b1358e8d605457d5',
  'verification/scase16/gate.mjs':'befd7a637538f5089156579c38485428cc182948de9112e4e0d38cb9147e0173',
};
for(const [file,digest] of Object.entries(frozen)) assert.equal(sha(content(file)),digest,`prior receipt/runner changed: ${file}`);
const sources={};
const add=file=>{sources[file]=sha(content(file));};
function tree(directory) {
  for(const entry of fs.readdirSync(path.join(root,directory),{withFileTypes:true})
    .sort((left,right)=>left.name.localeCompare(right.name))) {
    const file=`${directory}/${entry.name}`;
    assert.equal(entry.isSymbolicLink(),false,`unbound source symlink ${file}`);
    if(entry.isDirectory()) tree(file);
    else {assert.equal(entry.isFile(),true,`unbound source kind ${file}`);add(file);}
  }
}
for(const directory of ['crates/noble-kernel/src','crates/noble-contracts/src',
  'crates/noble-wasm/src','crates/noble-cli/src']) tree(directory);
for(const file of ['Cargo.toml','Cargo.lock','rust-toolchain.toml',
  'crates/noble-kernel/Cargo.toml','crates/noble-contracts/Cargo.toml',
  'crates/noble-wasm/Cargo.toml','crates/noble-cli/Cargo.toml',
  'crates/noble-wasm/wit/runtime-quotas.wit',
  '.cairn/specs/safety/spec.md',
  '.cairn/changes/safety-runtime-quotas/proposal.md',
  '.cairn/changes/safety-runtime-quotas/design.md',
  '.cairn/changes/safety-runtime-quotas/specs/safety/spec.md',
  'policy/tool-selection.json',caseFile,
  'verification/scase13/compute.noble','verification/scase13/emit-56.noble',
  'verification/scase13/emit-57.noble','verification/scase13/emit-65.noble',
  'verification/scase13/gate.mjs','verification/scase13/postpromotion.mjs']) add(file);
const prepromotionCaseSha256=sources[caseFile];
const rust=path.join(selected.tool_paths.quality_rust.output,'bin');
fs.mkdirSync(destination);
fs.writeFileSync(path.join(destination,'prepromotion-safety-cases.json'),content(caseFile),{flag:'wx'});
const temporary=path.join(destination,'tmp');fs.mkdirSync(temporary);
const environment={...process.env,
  PATH:`${rust}:${selected.component_sync.linker_bin}:${selected.tool_paths.node.output}/bin:${process.env.PATH}`,
  TMPDIR:temporary,RUSTC_WRAPPER:'',RUSTC_WORKSPACE_WRAPPER:'',
  CARGO_TARGET_DIR:path.join(destination,'target'),NIX_CONFIG:'min-free = 0'};
const commands=[];
function run(label,executable,args) {
  const result=spawnSync(executable,args,{cwd:root,env:environment,encoding:'utf8',timeout:900000,maxBuffer:16*1024*1024});
  const stem=`${String(commands.length).padStart(2,'0')}-${label}`;
  fs.writeFileSync(path.join(destination,`${stem}.stdout`),result.stdout??'',{flag:'wx'});
  fs.writeFileSync(path.join(destination,`${stem}.stderr`),result.stderr??'',{flag:'wx'});
  commands.push({label,executable,executable_sha256:sha(fs.readFileSync(executable)),args,
    status:result.status,signal:result.signal,error:result.error?.message??null,
    stdout:`${stem}.stdout`,stderr:`${stem}.stderr`,
    stdout_sha256:sha(result.stdout??''),stderr_sha256:sha(result.stderr??'')});
  assert.equal(result.error,undefined,`${label} spawn error`);
  assert.equal(result.signal,null,`${label} unexpected signal`);
  assert.equal(result.status,0,`${label}: ${result.stderr}`);
  return result.stdout;
}
run('production-build',path.join(rust,'cargo'),['build','-p','noble-cli','--locked','--offline','-j','4']);
const cli=path.join(environment.CARGO_TARGET_DIR,'debug/noble');
const wit='crates/noble-wasm/wit/runtime-quotas.wit';
const bindings=JSON.parse(run('typed-bindings',cli,['component','bindings',wit,'quotas']));
assert.equal(bindings.world,'noble-test:quotas/quotas@1.0.0');
assert.deepEqual(bindings.imports,[],'quota guest unexpectedly requires host callbacks');
assert.deepEqual(bindings.exports.map(row=>row.word),
  ['compute','emit-fifty-six','emit-fifty-seven','emit-sixty-five']);
const emitted=path.join(destination,'compiled');
const prepared=JSON.parse(run('compile-reviewed-guest',cli,['component','compile',wit,'quotas',emitted,
  'compute=verification/scase13/compute.noble',
  'emit-fifty-six=verification/scase13/emit-56.noble',
  'emit-fifty-seven=verification/scase13/emit-57.noble',
  'emit-sixty-five=verification/scase13/emit-65.noble']));
assert.equal(prepared.outcome,'compiled');
assert.equal(prepared.independent_kernel_check,true);
assert.equal(prepared.component_emitted,true);
const core=path.join(emitted,'core.wasm');
const component=path.join(emitted,'component.wasm');
assert.ok(fs.statSync(core).size>0&&fs.statSync(component).size>0);
for(const [name,file] of [
  ['world.wit',wit],['export-0.noble','verification/scase13/compute.noble'],
  ['export-1.noble','verification/scase13/emit-56.noble'],
  ['export-2.noble','verification/scase13/emit-57.noble'],
  ['export-3.noble','verification/scase13/emit-65.noble']
]) assert.deepEqual(fs.readFileSync(path.join(emitted,name)),content(file),`compiler output differs: ${name}`);
const module=new WebAssembly.Module(fs.readFileSync(core));
assert.deepEqual(WebAssembly.Module.imports(module),[],'emitted guest has imports');
const exports=new Set(WebAssembly.Module.exports(module).map(row=>row.name));
for(const name of ['memory','noble$set-allocation-limit','noble$quota-exceeded',
  'noble$allocation-count','noble$allocated-bytes','noble$copied-bytes','noble$live-bytes',
  'noble$cleanup-count','noble$cleanup',
  ...bindings.exports.flatMap(row=>[`cm32p2||${row.word}`,`cm32p2||${row.word}_post`])])
  assert.ok(exports.has(name),`missing production generated export ${name}`);
const variants=[];
function invoke(label,exportName,fuel,allocation,expected) {
  const observed=JSON.parse(run(label,cli,['component','quota-core',core,exportName,
    String(fuel),String(allocation)]));
  assert.equal(observed.schema,'noble-runtime-quota/v1');
  assert.equal(observed.stage,'execution');
  assert.equal(observed.export,exportName);
  assert.equal(observed.fuel,fuel);
  assert.equal(observed.allocation_limit_bytes,allocation);
  assert.equal(observed.wasm_memory_bytes,1048576);
  assert.equal(observed.guest_callbacks,0);
  assert.equal(observed.live_bytes_after_cleanup,0);
  assert.equal(observed.cleanups,1);
  for(const [field,value] of Object.entries(expected))
    assert.deepEqual(observed[field],value,`${label}.${field}`);
  variants.push({label,export:exportName,fuel,allocation_limit_bytes:allocation,observed});
  return observed;
}
const refusal={outcome:'specified-quota-failure',reported_success:false,returned_i64:null,returned_hex:null};
const fuel=invoke('canonical-fuel-one','compute',design.input.fuel,983040,
  {...refusal,quota_reason:'fuel',wasmtime_trap:'OutOfFuel',fuel_remaining_at_return:0,
    quota_exceeded:false,allocations_before_cleanup:0,live_bytes_before_cleanup:0});
const allocation=invoke('canonical-heap-sixty-five','emit-sixty-five',10000000,design.input.allocation_limit_bytes,
  {...refusal,quota_reason:'allocation',wasmtime_trap:'UnreachableCodeReached',quota_exceeded:true,
    allocations_before_cleanup:0,live_bytes_before_cleanup:0});
for(const row of [fuel,allocation]) for(const [field,value] of Object.entries(design.expected))
  assert.deepEqual(row[field],value,`S-CASE-13.${field}`);
const success={outcome:'normal',reported_success:true,quota_reason:null,wasmtime_trap:null,quota_exceeded:false};
invoke('positive-compute','compute',10000000,983040,
  {...success,returned_i64:'3',returned_hex:null,allocations_before_cleanup:0});
invoke('positive-heap-fifty-six','emit-fifty-six',10000000,64,
  {...success,returned_i64:null,returned_hex:'61'.repeat(56),
    allocations_before_cleanup:2,allocated_bytes_before_cleanup:64,
    copied_bytes_before_cleanup:56,live_bytes_before_cleanup:64});
invoke('heap-record-overflow-fifty-seven','emit-fifty-seven',10000000,64,
  {...refusal,quota_reason:'allocation',wasmtime_trap:'UnreachableCodeReached',quota_exceeded:true,
    allocations_before_cleanup:1,allocated_bytes_before_cleanup:57,
    copied_bytes_before_cleanup:57,live_bytes_before_cleanup:57});
invoke('positive-heap-sixty-five','emit-sixty-five',10000000,76,
  {...success,returned_i64:null,returned_hex:'61'.repeat(65),
    allocations_before_cleanup:2,allocated_bytes_before_cleanup:73,
    copied_bytes_before_cleanup:65,live_bytes_before_cleanup:76});
for(const [file,digest] of Object.entries(sources)) assert.equal(sha(content(file)),digest,`source changed during gate: ${file}`);
for(const [file,digest] of Object.entries(frozen)) assert.equal(sha(content(file)),digest,`historical source changed: ${file}`);
const receipt={schema:'noble-scase13-runtime-quotas/v1',kind:'test',result:'passed',
  case:{id:design.id,input:design.input,expected:design.expected},
  claim:'The genuinely compiler-emitted import-free Noble Core artifact is instantiated at its fixed 16-page baseline before separately metered Wasmtime guest-call fuel=1 and generated heap quota=64. The typed OutOfFuel and private allocator-only diagnostic each reject during execution without guest result or callback, while source-identical ample-budget and 56/57/65/76-byte controls distinguish both paths. This finite result is not a general engine, compiler or language-safety proof.',
  source_revision:`sha256:${sha(JSON.stringify(Object.entries(sources).sort()))}`,
  source_sha256:sources,prepromotion_case_sha256:prepromotionCaseSha256,
  preserved_historical_sha256:frozen,
  binaries:{cli:{path:cli,sha256:sha(fs.readFileSync(cli))},
    core:{path:core,sha256:sha(fs.readFileSync(core))},
    component:{path:component,sha256:sha(fs.readFileSync(component))}},
  selected_rust:selected.tool_paths.quality_rust,selected_node:selected.tool_paths.node,
  selected_wasmtime:'40.0.2',commands,variants,
  assumptions:[
    'The host independently selects exactly the guest Core artifact, fuel and generated-heap limit. Source preparation, pinned compiler, wasm-tools and Wasmtime are trusted at their recorded byte identities; neither guest code nor a candidate artifact is permitted to establish its own correspondence.',
    'The private generated diagnostic is set only by the production allocator quota guard; a generic unreachable trap or failed instantiation is not promoted as quota exhaustion. The imported callback list is empty and guest memory remains 16 pages in every fresh Store.',
    'Generated guest heap counters exclude all Wasmtime, host and physical allocations. Explicit trap cleanup is different from successful generated post-return; neither reports success for a failed call.',
    'This finite source-bound execution leaves general compiler/engine correspondence, language safety, owner law, Octet and Aeneas proof open. Earlier S-CASE-07, S-CASE-02, S-CASE-16, DX-06 and six-case receipts are historical immutable source scopes; a new later-source replay is required.'
  ]};
fs.writeFileSync(path.join(destination,'acceptance.json'),JSON.stringify(receipt,null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({result:'passed',receipt:path.join(destination,'acceptance.json'),
  source_revision:receipt.source_revision,core_sha256:receipt.binaries.core.sha256,
  binary_sha256:receipt.binaries.cli.sha256,variants:variants.length,commands:commands.length}));
