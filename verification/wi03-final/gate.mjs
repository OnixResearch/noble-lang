#!/usr/bin/env node
// Corrected original-source WI-03 acceptance. The earlier wi03 runner/receipt
// are historical only: $enter runs before the unsigned guard, but the guard
// precedes Noble source-body use. Never edits canonical state.
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import {spawnSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';

const root=fs.realpathSync(path.resolve(path.dirname(fileURLToPath(import.meta.url)),'../..'));
const sha=value=>crypto.createHash('sha256').update(value).digest('hex');
const read=name=>fs.readFileSync(path.join(root,name));
const file='specs/conformance/wit-wasi-cases.json';
const caseBytes=read(file),document=JSON.parse(caseBytes);
const [row]=document.cases.filter(item=>item.id==='WI-03');
assert.equal(document.cases.filter(item=>item.id==='WI-03').length,1);
const design={id:'WI-03',profile:'Component-Draft',kind:'adapter',
  requirements:['WI-WIT-02','WI-WIT-06'],input:{harness:'exact-u64-boundary',
    wit_type:'u64',value:'18446744073709551615',requested_implicit_target:'I64'},
  expected:{stage:'binding',outcome:'lossy-conversion-reject',wrapped_value_exposed:false}};
assert.deepEqual(Object.fromEntries(Object.keys(design).map(key=>[key,row[key]])),design);
assert.deepEqual(row.state,{implementation:'absent',execution:'not-run',proof:'open',trust:'unassessed'});
assert.deepEqual(row.evidence,[]);
assert.equal(BigInt(row.input.value),2n**64n-1n);
assert.ok(!fs.existsSync(path.join(root,'verification/wi03-final/acceptance.json')),
  'final receipt already exists: do not overwrite or rerun a prepromotion gate');
const priorFile='verification/wi03/acceptance.json';
const priorDigest='d5d9a7d891582f3247778b9417b638083a2165e762a7f1f89aa5c5fe62288f67';
assert.equal(sha(read(priorFile)),priorDigest,'old receipt must remain byte-identical');
const oldReceipt=JSON.parse(read(priorFile));
assert.equal(oldReceipt.source_revision,
  'sha256:6ff838c42913498a7eb120a6ba1b009621afb56559b2676d9115faa33e7555f0');
assert.equal(sha(fs.readFileSync(path.join(oldReceipt.external_raw_output,'acceptance.json'))),
  priorDigest,'old external receipt must remain untouched');
const selected=JSON.parse(read('policy/tool-selection.json'));
const runtime=JSON.parse(read('crates/noble-cli/src/core/runtime/config.json'));
const rust=path.join(selected.tool_paths.quality_rust.output,'bin');
const node=path.join(selected.tool_paths.node.output,'bin/node');
const wasm=path.join(selected.tool_paths.wasm_tools.output,'bin/wasm-tools');
assert.equal(fs.realpathSync(process.execPath),fs.realpathSync(node),'selected Node required');
assert.equal(fs.realpathSync(runtime.tools.node.path),fs.realpathSync(node));
assert.equal(fs.realpathSync(runtime.tools.wasm_tools.path),fs.realpathSync(wasm));
const inventory=()=>{
  const names=new Set(['Cargo.toml','Cargo.lock','rust-toolchain.toml',
    'policy/tool-selection.json',file,'.cairn/specs/wit-wasi/spec.md',
    'crates/noble-kernel/Cargo.toml','crates/noble-contracts/Cargo.toml',
    'crates/noble-wasm/Cargo.toml','crates/noble-cli/Cargo.toml',
    'crates/noble-syndicate/Cargo.toml']);
  function tree(directory){
    for(const entry of fs.readdirSync(path.join(root,directory),{withFileTypes:true})){
      const name=`${directory}/${entry.name}`;
      assert.equal(entry.isSymbolicLink(),false,`source symlink: ${name}`);
      if(entry.isDirectory())tree(name);
      else {assert.equal(entry.isFile(),true,`source file: ${name}`);names.add(name);}
    }
  }
  for(const directory of ['crates/noble-kernel/src','crates/noble-contracts/src',
    'crates/noble-wasm/src','crates/noble-wasm/runtime','crates/noble-wasm/wit',
    'crates/noble-cli/src','crates/noble-syndicate/src','verification/wi03/peer',
    'verification/wi03-final','.cairn/changes/wit-exact-u64-adapter'])tree(directory);
  for(const entry of fs.readdirSync(path.join(root,'verification/wi03'),{withFileTypes:true})){
    if(entry.isFile() && (entry.name.endsWith('.noble')||entry.name.endsWith('.wit')||
      ['gate.mjs','postpromotion.mjs','README.md'].includes(entry.name)))
      names.add(`verification/wi03/${entry.name}`);
  }
  return Object.fromEntries([...names].sort().map(name=>{
    const stat=fs.lstatSync(path.join(root,name));
    assert.ok(stat.isFile()&&!stat.isSymbolicLink(),`source: ${name}`);
    return [name,sha(read(name))];
  }));
};
const sources=inventory();
const sourceRevision=`sha256:${sha(JSON.stringify(Object.entries(sources).sort()))}`;
const historical={};
historical[priorFile]=priorDigest;
for(const old of document.cases) for(const evidence of old.evidence??[]){
  const relative=evidence.configuration?.receipt;
  if(!relative)continue;
  const absolute=path.resolve(root,'specs/conformance',relative);
  assert.ok(absolute.startsWith(`${root}/verification/`),'historical receipt escapes verification');
  const name=path.relative(root,absolute);
  assert.equal(fs.realpathSync(absolute),absolute,`historical receipt symlink: ${name}`);
  const digest=sha(fs.readFileSync(absolute));
  if(evidence.configuration.receipt_sha256!==undefined)
    assert.equal(evidence.configuration.receipt_sha256,digest,`historical receipt changed: ${name}`);
  historical[name]=digest;
}
const plan={schema:'noble-wi03-final-plan/v1',source_revision:sourceRevision,
  source_files:Object.keys(sources).length,historical_receipts:Object.keys(historical).length,
  canonical_preimage_sha256:sha(caseBytes),case:design,
  requirement:'fresh external directory and independently reviewed source revision'};
if(process.argv.length===3&&process.argv[2]==='--plan'){
  console.log(JSON.stringify(plan));process.exit(0);
}
assert.equal(process.argv.length,4,
  'usage: SELECTED_NODE verification/wi03-final/gate.mjs NEW_EXTERNAL_DIR sha256:REVIEWED_REVISION (or --plan)');
assert.equal(process.argv[3],sourceRevision,'wrong recipe/source revision');
const output=path.resolve(process.argv[2]);
assert.ok(output!==root&&!output.startsWith(`${root}/`),'external output required');
const parent=fs.realpathSync(path.dirname(output));
assert.ok(parent!==root&&!parent.startsWith(`${root}/`),'external parent required');
fs.mkdirSync(output); // Existing output must fail; never rewrite a previous run.
assert.equal(fs.realpathSync(output),output,'output symlink');
fs.mkdirSync(path.join(output,'tmp'));
fs.writeFileSync(path.join(output,'prepromotion-wit-wasi-cases.json'),caseBytes,{flag:'wx'});
fs.writeFileSync(path.join(output,'prepromotion-wit-spec.md'),
  read('.cairn/specs/wit-wasi/spec.md'),{flag:'wx'});
fs.writeFileSync(path.join(output,'prepromotion-native-tasks.md'),
  read('.cairn/changes/wit-exact-u64-adapter/tasks.md'),{flag:'wx'});
const env={...process.env,PATH:`${rust}:${selected.component_sync.linker_bin}:${selected.tool_paths.node.output}/bin:${process.env.PATH}`,
  TMPDIR:path.join(output,'tmp'),RUSTC_WRAPPER:'',RUSTC_WORKSPACE_WRAPPER:'',
  RUSTFLAGS:'',CARGO_ENCODED_RUSTFLAGS:'',NIX_CONFIG:'min-free = 0',
  CARGO_TARGET_DIR:path.join(output,'target')};
const commands=[];
function run(label,executable,args,expected=0,overrides={}){
  const result=spawnSync(executable,args,{cwd:root,env:{...env,...overrides},timeout:900000,
    maxBuffer:16*1024*1024});
  const stdout=result.stdout??Buffer.alloc(0),stderr=result.stderr??Buffer.alloc(0);
  const stem=`${String(commands.length).padStart(2,'0')}-${label}`;
  fs.writeFileSync(path.join(output,`${stem}.stdout`),stdout,{flag:'wx'});
  fs.writeFileSync(path.join(output,`${stem}.stderr`),stderr,{flag:'wx'});
  commands.push({label,executable,executable_sha256:sha(fs.readFileSync(executable)),args,
    status:result.status,expected_status:expected,signal:result.signal,error:result.error?.message??null,
    stdout:`${stem}.stdout`,stdout_sha256:sha(stdout),stderr:`${stem}.stderr`,stderr_sha256:sha(stderr)});
  assert.equal(result.error,undefined,`${label} launch`);
  assert.equal(result.signal,null,`${label} signal`);
  assert.equal(result.status,expected,`${label}: ${stderr.toString()} ${stdout.toString()}`);
  return stdout.toString('utf8').trim();
}
function cliReport(label,cli,args,status){
  const result=JSON.parse(run(label,cli,args,status));
  assert.equal(result.schema,'noble-component/v1');
  return result;
}
const cargo=path.join(rust,'cargo');
const vendor=selected.component_sync.vendor;
const config=['--config','source.crates-io.replace-with="wi03-vendor"',
  '--config',`source.wi03-vendor.directory="${vendor}/source-registry-0"`];
run('build-production-cli',cargo,['build','-p','noble-cli','--locked','--offline','-j','4']);
const cli=path.join(output,'target/debug/noble');
const binary={path:cli,sha256:sha(fs.readFileSync(cli))};
run('build-independent-peer',cargo,['build','--manifest-path',
  'verification/wi03/peer/Cargo.toml','--locked','--offline','-j','4',...config],0,
  {CARGO_TARGET_DIR:path.join(output,'peer-target')});
const peer=path.join(output,'peer-target/debug/noble-wi03-peer');
const peerBinary={path:peer,sha256:sha(fs.readFileSync(peer))};
const fixture=name=>path.join(root,'verification/wi03',name);
const echo=fixture('echo.wit'),guest=fixture('echo.noble');
const defaultOutput=path.join(output,'implicit-refused');
const implicit=cliReport('default-implicit-binding',cli,
  ['component','compile',echo,'demo',defaultOutput,`echo=${guest}`],2);
assert.deepEqual({outcome:implicit.outcome,code:implicit.diagnostic?.code,
  message:implicit.diagnostic?.message,component_emitted:implicit.component_emitted},
{outcome:'error',code:'component-check',
  message:'Binding: WIT u64 requires an explicitly selected checked I64 boundary',
  component_emitted:false});
assert.equal(fs.existsSync(defaultOutput),false);
const refusal={stage:design.expected.stage,outcome:design.expected.outcome,
  wrapped_value_exposed:design.expected.wrapped_value_exposed,
  actual_cli_stage:'Binding',actual_cli_outcome:implicit.outcome,component_emitted:false,
  runtime_value_consumed:false,guest_requests:0,protected_operations:0};
const rejected={};
for(const [label,name] of [['unsigned-import','import-u64.wit'],
  ['signed-import','import-s64.wit'],['async-export','async.wit'],['mixed-export','mixed.wit']]){
  const destination=path.join(output,`rejected-${label}`);
  const report=cliReport(label,cli,['component','compile-checked-u64',fixture(name),
    'demo',destination,`echo=${guest}`],4);
  assert.equal(report.outcome,'unsupported',label);
  assert.equal(report.diagnostic?.code,'component-check',label);
  assert.match(report.diagnostic?.message??'',/^Binding: /,label);
  if(label.endsWith('import'))assert.equal(report.diagnostic.message,
    'Binding: checked u64 boundary requires an import-free world');
  assert.equal(report.component_emitted,false,label);
  assert.equal(fs.existsSync(destination),false,label);
  rejected[label]={stage:'binding',outcome:'unsupported',report};
}
function compile(label,source){
  const destination=path.join(output,label);
  const report=cliReport(`compile-${label}`,cli,['component','compile-checked-u64',
    echo,'demo',destination,`echo=${fixture(source)}`]);
  assert.deepEqual([report.outcome,report.component_emitted,report.checked_u64_boundary,
    report.profile,report.world,report.independent_kernel_check],
    ['compiled',true,true,'Component-Sync-Bootstrap','noble-test:unsigned/demo@1.0.0',true]);
  assert.deepEqual(report.exports,[{name:'echo',source:'export-0.noble'}]);
  assert.equal(report.assembler,wasm);
  assert.equal(report.output,destination);
  assert.deepEqual(fs.readFileSync(path.join(destination,'export-0.noble')),fs.readFileSync(fixture(source)));
  assert.deepEqual(fs.readFileSync(path.join(destination,'world.wit')),fs.readFileSync(echo));
  const component=path.join(destination,'component.wasm');
  const bytes=fs.readFileSync(component);
  assert.equal(bytes.length,report.component_bytes);
  const wat=path.join(destination,'module.wat');
  const core=path.join(destination,'core.wasm');
  const assembly=fs.readFileSync(wat).toString('utf8');
  const body=assembly.match(/\(func \(export "cm32p2\|\|echo"\)[\s\S]*?\n\)/)?.[0];
  assert.ok(body,'selected generated core export must exist');
  const guard='i64.const 9223372036854775807 i64.gt_u if unreachable end';
  assert.equal(body.split(guard).length-1,2,
    'selected ingress and egress unsigned range guards');
  const first=body.indexOf(guard),last=body.lastIndexOf(guard);
  assert.ok(first<last);
  const prologue=body.indexOf('call $enter');
  assert.ok(prologue>=0 && prologue<first,
    'the internal guest $enter prologue executes before ingress validation');
  if(source==='negative.noble'){
    const guest=body.indexOf('i64.sub');
    assert.ok(first<guest && guest<last,'guard, negative guest, guard order');
  }else assert.match(body,
    /local\.get 0\s+i64\.const 9223372036854775807 i64\.gt_u if unreachable end\s+local\.get 0\s+i64\.const 9223372036854775807 i64\.gt_u if unreachable end\s+local\.get 0/);
  return {report,component,sha256:sha(bytes),
    emitted:{wat:{path:wat,sha256:sha(fs.readFileSync(wat))},
      core:{path:core,sha256:sha(fs.readFileSync(core))}},
    guard_order:{internal_enter_prologue_before_ingress_guard:true,
      ingress_before_source_body:true,egress_before_publication:true,
      source_body_entries_measured:false}};
}
const checked=compile('checked-positive','echo.noble');
const negative=compile('checked-negative','negative.noble');
function probe(label,artifact){
  const report=JSON.parse(run(label,peer,[artifact.component]));
  assert.equal(report.schema,'noble-wi03-peer/v1');
  assert.equal(report.engine,'wasmtime-40.0.2');
  assert.equal(report.export,'echo');
  assert.deepEqual(report.component,{path:artifact.component,sha256:artifact.sha256},
    'peer must hash the exact compiled component bytes');
  assert.deepEqual(report.observations.map(item=>item.input),
    ['0','42','9223372036854775807','9223372036854775808',design.input.value]
      .map(value=>({type:'u64',value})));
  return report;
}
const positive=probe('independent-checked-u64',checked);
for(const [index,value] of ['0','42','9223372036854775807'].entries())
  assert.deepEqual(positive.observations[index].invocation,{outcome:'normal',
    result:{type:'u64',value},post_return:true},`exact U64 result ${value}`);
for(const item of positive.observations.slice(3)){
  assert.equal(item.invocation.outcome,'trap','unsigned ingress above i64::MAX must trap');
  assert.equal(item.invocation.post_return,false);
  assert.ok(!Object.hasOwn(item.invocation,'result'),'no exposed I64 or wrapped U64 result');
}
const negativeReport=probe('independent-negative-result',negative);
for(const item of negativeReport.observations){
  assert.equal(item.invocation.outcome,'trap','negative guest I64 must not publish U64');
  assert.equal(item.invocation.post_return,false);
  assert.ok(!Object.hasOwn(item.invocation,'result'));
}
for(const [name,digest] of Object.entries(sources))assert.equal(sha(read(name)),digest,
  `source changed during gate: ${name}`);
for(const [name,digest] of Object.entries(historical))assert.equal(sha(read(name)),digest,
  `historical receipt changed during gate: ${name}`);
for(const command of commands){
  assert.equal(command.error,null);assert.equal(command.signal,null);
  assert.equal(command.status,command.expected_status);
  assert.equal(sha(fs.readFileSync(command.executable)),command.executable_sha256);
  for(const stream of ['stdout','stderr'])assert.equal(
    sha(fs.readFileSync(path.join(output,command[stream]))),command[`${stream}_sha256`]);
}
assert.equal(sha(fs.readFileSync(cli)),binary.sha256);
assert.equal(sha(fs.readFileSync(peer)),peerBinary.sha256);
for(const artifact of [checked,negative])assert.equal(
  sha(fs.readFileSync(artifact.component)),artifact.sha256);
for(const artifact of [checked,negative])for(const emitted of Object.values(artifact.emitted))
  assert.equal(sha(fs.readFileSync(emitted.path)),emitted.sha256);
const counts=observations=>({
  typed_calls:observations.length,
  normal:observations.filter(item=>item.invocation.outcome==='normal').length,
  traps:observations.filter(item=>item.invocation.outcome==='trap').length,
  import_callbacks_configured:0,source_body_entries_measured:null
});
assert.deepEqual(counts(positive.observations),{typed_calls:5,normal:3,traps:2,
  import_callbacks_configured:0,source_body_entries_measured:null});
assert.deepEqual(counts(negativeReport.observations),{typed_calls:5,normal:0,traps:5,
  import_callbacks_configured:0,source_body_entries_measured:null});
const receipt={schema:'noble-wi03-final-source-body-guard/v1',kind:'test',result:'passed',
  failures:[],integrity_failures:[],source_revision:sourceRevision,source_sha256:sources,
  external_raw_output:output,prepromotion_case_sha256:sha(caseBytes),
  prepromotion_case_path:'prepromotion-wit-wasi-cases.json',case:design,
  historical_receipt_sha256:historical,
  prior_prepromotion_receipt:{path:priorFile,sha256:priorDigest,
    source_revision:oldReceipt.source_revision,scope:'historical-only-unpromoted'},
  tools:{rust:selected.tool_paths.quality_rust,node:{path:node,sha256:sha(fs.readFileSync(node))},
    wasm_tools:{path:wasm,sha256:sha(fs.readFileSync(wasm))},
    wasmtime_source:selected.component_sync.wasmtime_source,
    wasmtime_source_nar_hash:selected.component_sync.wasmtime_source_nar_hash,
    vendor, vendor_nar_hash:selected.component_sync.vendor_nar_hash,
    linker_bin:selected.component_sync.linker_bin},
  binaries:{cli:binary,peer:peerBinary},canonical_refusal:refusal,
  implicit_cli_report:implicit,rejected_signatures:rejected,
  components:{checked,negative},typed_peer:{checked:positive,negative:negativeReport},commands,
  typed_call_counts:{checked:counts(positive.observations),
    negative:counts(negativeReport.observations)},
  assumptions:[
    'Canonical WI-03 binds a default implicit u64-to-I64 refusal at Binding; its all-ones decimal is a case design, not a runtime value consumed by that refusal.',
    'Separate explicit checked export admits only import-free synchronous u64-to-u64 scalar; typed peer runs fresh instances and observes returned Val::U64 or trap. The internal $enter guest prologue executes before the unsigned ingress guard; source-body use follows the guard. No source-body counter exists and no before-any-guest-instruction claim is made. Source-body and pre-publication ordering rely on compiled guard placement and selected compiler/Wasmtime behavior.',
    'The public CLI and World API let the caller opt into checked mode; build_context is deterministic recipe identity, not an authenticated host authorization grant. This gate makes no host-selection authority claim.',
    'Selected Rust, pinned Wasmtime and vendor store closures, linker, wasm-tools, Noble compiler/host, source snapshots and historical receipts are trusted for this finite observation; universal compiler/ABI/engine correctness and proof remain open.',
    'Full Octet/Clippy and signed+sandbox Nix checks are blocked/unclaimed; the native Cairn change remains ACTIVE and is not archived.'
  ]};
fs.writeFileSync(path.join(output,'acceptance.json'),JSON.stringify(receipt,null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({result:'passed',receipt:path.join(output,'acceptance.json'),
  source_revision:sourceRevision,commands:commands.length,failures:0,integrity_failures:0}));
