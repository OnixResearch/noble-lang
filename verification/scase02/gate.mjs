#!/usr/bin/env node
// Freeze and exercise the real compiler, component guest and production host.
// This gate does not promote the canonical case or claim a general proof.
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
assert.equal(process.argv.length, 3, 'usage: node verification/scase02/gate.mjs NEW_EXTERNAL_DIRECTORY');
const destination = path.resolve(process.argv[2]);
assert.ok(destination !== root && !destination.startsWith(`${root}/`), 'output must be external');
fs.mkdirSync(destination);
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const bytes = file => fs.readFileSync(path.join(root, file));
const caseFile = 'specs/conformance/safety-cases.json';
const design = JSON.parse(bytes(caseFile)).cases.find(row => row.id === 'S-CASE-02');
assert.deepEqual(design.input, {harness:'bounded-buffer-access',buffer_hex:'000102',offset:3,length:1});
assert.deepEqual(design.expected, {stage:'adapter',outcome:'bounds-reject',protected_operations:0,native_memory_access:false});
assert.deepEqual(design.state, {implementation:'absent',execution:'not-run',proof:'open',trust:'unassessed'});
assert.deepEqual(design.evidence, []);
const sources = {};
function add(file) { sources[file] = sha(bytes(file)); }
function tree(directory) {
  for (const entry of fs.readdirSync(path.join(root, directory), {withFileTypes:true}).sort((a,b)=>a.name.localeCompare(b.name))) {
    const file = `${directory}/${entry.name}`;
    assert.equal(entry.isSymbolicLink(), false, `unexpected source symlink ${file}`);
    if (entry.isDirectory()) tree(file);
    else if (entry.isFile()) add(file);
  }
}
for (const directory of ['crates/noble-kernel/src','crates/noble-contracts/src','crates/noble-wasm/src','crates/noble-cli/src']) tree(directory);
for (const file of ['Cargo.toml','Cargo.lock','rust-toolchain.toml','crates/noble-cli/Cargo.toml',
  'crates/noble-wasm/wit/bounded-region.wit','.cairn/specs/safety/spec.md',
  '.cairn/specs/resource-adapters/spec.md','.cairn/changes/safety-bounded-region-adapter/design.md',
  '.cairn/changes/safety-wasm-manifest-admission/tasks.md',caseFile,
  'policy/tool-selection.json','verification/scase02/gate.mjs']) add(file);
const prepromotionCaseSha256 = sources[caseFile];
const selected = JSON.parse(bytes('policy/tool-selection.json'));
const rust = path.join(selected.tool_paths.quality_rust.output,'bin');
const temp = path.join(destination,'tmp');
fs.mkdirSync(temp);
const environment = {...process.env, PATH:`${rust}:${selected.component_sync.linker_bin}:${selected.tool_paths.node.output}/bin:${process.env.PATH}`,
  TMPDIR:temp,RUSTC_WRAPPER:'',RUSTC_WORKSPACE_WRAPPER:'',CARGO_TARGET_DIR:path.join(destination,'target'),
  NIX_CONFIG:'min-free = 0'};
const commands=[];
function run(label, program, args) {
  const index=commands.length;
  const result=spawnSync(program,args,{cwd:root,env:environment,encoding:'utf8',timeout:900_000,maxBuffer:16*1024*1024});
  const stem=`${String(index).padStart(2,'0')}-${label}`;
  fs.writeFileSync(path.join(destination,`${stem}.stdout`),result.stdout??'',{flag:'wx'});
  fs.writeFileSync(path.join(destination,`${stem}.stderr`),result.stderr??'',{flag:'wx'});
  commands.push({label,executable:program,executable_sha256:sha(fs.readFileSync(program)),args,
    status:result.status,signal:result.signal,error:result.error?.message??null,
    stdout:`${stem}.stdout`,stderr:`${stem}.stderr`,
    stdout_sha256:sha(result.stdout??''),stderr_sha256:sha(result.stderr??'')});
  assert.equal(result.error,undefined,`${label} spawn failed`);
  assert.equal(result.signal,null,`${label} terminated`);
  assert.equal(result.status,0,`${label}: ${result.stderr}`);
  return result.stdout;
}
run('production-build',path.join(rust,'cargo'),['build','-p','noble-cli','--locked','--offline','-j','4']);
const cli=path.join(environment.CARGO_TARGET_DIR,'debug/noble');
const source='region.read swap regions.release\n';
const guest=path.join(destination,'read-region.noble');
fs.writeFileSync(guest,source,{flag:'wx'});
const output=path.join(destination,'component');
const wit='crates/noble-wasm/wit/bounded-region.wit';
const binding=JSON.parse(run('typed-bindings',cli,['component','bindings',wit,'bounded']));
assert.equal(binding.world,'noble-test:bounded-region/bounded@1.0.0');
assert.deepEqual(binding.imports.map(row=>row.word),['region.read','regions.release']);
assert.deepEqual(binding.exports.map(row=>row.word),['read-region']);
assert.equal(binding.resources[0].data,false);
assert.equal(binding.resources[0].capture,false);
const compiled=JSON.parse(run('compile-guest',cli,['component','compile',wit,'bounded',output,`read-region=${guest}`]));
assert.equal(compiled.outcome,'compiled');
assert.equal(compiled.independent_kernel_check,true);
assert.equal(compiled.component_emitted,true);
const component=path.join(output,'component.wasm');
const artifactSha=sha(fs.readFileSync(component));
const cases=[];
function invocation(label, region, offset, length, expected) {
  const observed=JSON.parse(run(label,cli,['component','read-region',component,region,String(offset),String(length)]));
  assert.equal(observed.schema,'noble-bounded-region/v1');
  assert.equal(observed.stage,'adapter');
  assert.equal(observed.native_memory_access,false);
  assert.equal(observed.released_owners,1);
  assert.equal(observed.live_owners,0);
  assert.equal(observed.native_pins,0);
  for(const [field,value] of Object.entries(expected)) assert.deepEqual(observed[field],value,`${label}.${field}`);
  cases.push({label,buffer_hex:region,offset:String(offset),length:String(length),observed});
  return observed;
}
const rejected={outcome:'bounds-reject',bytes_hex:null,error:'bounds-reject',protected_operations:0,region_reads:0};
const actual=invocation('canonical-scase02',design.input.buffer_hex,design.input.offset,design.input.length,rejected);
for(const [field,value] of Object.entries(design.expected)) assert.deepEqual(actual[field],value,`S-CASE-02.${field}`);
invocation('positive-in-bounds','000102',1,2,{outcome:'read',bytes_hex:'0102',error:null,protected_operations:1,region_reads:1});
invocation('host-selected-distinct-bytes','aabbcc',1,2,{outcome:'read',bytes_hex:'bbcc',error:null,protected_operations:1,region_reads:1});
invocation('end-zero','000102',3,0,{outcome:'read',bytes_hex:'',error:null,protected_operations:1,region_reads:1});
for(const [label,offset,length] of [
  ['partial-overrun',2,2],['zero-outside',4,0],['negative-offset',-1,1],
  ['negative-length',0,-1],['checked-add-overflow','9223372036854775807',1],
]) invocation(label,'000102',offset,length,rejected);
run('invalid-owner-and-boundary-tests',path.join(rust,'cargo'),
  ['test','-p','noble-cli','--locked','--offline','--bin','noble','component::bounded_region::tests','--','--test-threads','1']);
for(const [file,digest] of Object.entries(sources)) assert.equal(sha(bytes(file)),digest,`source changed mid-gate: ${file}`);
const receipt={schema:'noble-scase02-bounded-region/v1',kind:'test',result:'passed',
  case:{id:design.id,input:design.input,expected:design.expected},
  claim:'An actual compiled Noble component calls the production versioned host-owned region adapter: the exact three-byte out-of-range read refuses before protected region access, valid reads return independently host-selected bytes and the owner is released once. This finite result is not a universal memory-safety theorem.',
  source_revision:`sha256:${sha(JSON.stringify(Object.entries(sources).sort()))}`,
  source_sha256:sources,prepromotion_case_sha256:prepromotionCaseSha256,
  guest_source_sha256:sha(source),component_sha256:artifactSha,binary_sha256:sha(fs.readFileSync(cli)),
  selected_rust:selected.tool_paths.quality_rust,selected_wasmtime:'40.0.2',
  commands,cases,assumptions:[
    'Host invocation chooses immutable region bytes, assigns fresh per-invocation owner, table and context, and grants read right; guest cannot inject the region or turn offset into native address.',
    'Pinned Rust, compiler, wasm-tools, Wasmtime 40.0.2 and Canonical ABI are trusted for the finite compiled guest and host observations; region reads count only after checked admission.',
    'Rust unit controls mutate claims directly in the production host method; the Wasmtime Canonical ABI mediates guest resource representations. Proof of general non-forgeability, host native safety, backend correspondence and owner law remains open.',
    'Historical S-CASE-16, DX06 and prior six-case receipts retain their original source revisions; this new tree requires a separate future replay.'
  ]};
fs.writeFileSync(path.join(destination,'acceptance.json'),JSON.stringify(receipt,null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({result:'passed',receipt:path.join(destination,'acceptance.json'),
  source_revision:receipt.source_revision,component_sha256:artifactSha,cases:cases.length,commands:commands.length}));
