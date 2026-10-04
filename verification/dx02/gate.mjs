#!/usr/bin/env node
// Frozen, finite DX-02 editor analysis/admission. Run before case promotion.
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {spawnSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';

const root = fs.realpathSync(path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..'));
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const content = file => fs.readFileSync(path.join(root, file));
const selected = JSON.parse(content('policy/tool-selection.json'));
const rustBin = path.join(selected.tool_paths.quality_rust.output, 'bin');
const node = path.join(selected.tool_paths.node.output, 'bin/node');
const source = {};
function inventory(directory) {
  for (const item of fs.readdirSync(path.join(root, directory), {withFileTypes:true})
    .sort((a,b) => a.name.localeCompare(b.name))) {
    const file = `${directory}/${item.name}`;
    assert.equal(item.isSymbolicLink(), false, `symlink in DX-02 source: ${file}`);
    if (item.isDirectory()) inventory(file);
    else {
      assert.equal(item.isFile(), true, `non-file in DX-02 source: ${file}`);
      source[file] = hash(content(file));
    }
  }
}
for (const dir of ['crates/noble-kernel/src','crates/noble-contracts/src',
  'crates/noble-wasm/src','crates/noble-cli/src',
  'crates/noble-kernel/tests','crates/noble-contracts/tests',
  'crates/noble-wasm/tests','crates/noble-cli/tests']) inventory(dir);
for (const file of [
  'Cargo.toml','Cargo.lock','rust-toolchain.toml',
  'crates/noble-kernel/Cargo.toml','crates/noble-contracts/Cargo.toml',
  'crates/noble-wasm/Cargo.toml','crates/noble-cli/Cargo.toml',
  'policy/tool-selection.json','policy/boundary-controls.ncl','policy/boundary-controls.json',
  '.cairn/specs/developer-experience/spec.md',
  '.cairn/changes/editor-hole-transport/specs/developer-experience/spec.md',
  'verification/dx02/gate.mjs','README.md'
]) source[file] = hash(content(file));
const revision = `sha256:${hash(JSON.stringify(Object.entries(source).sort()))}`;
const caseFile = 'specs/conformance/developer-experience-cases.json';
const caseBytes = content(caseFile);
const caseHash = hash(caseBytes);
const dx02 = JSON.parse(caseBytes).cases.find(entry => entry.id === 'DX-02');
assert.ok(dx02, 'canonical DX-02 must exist');
assert.deepEqual(dx02.state, {implementation:'absent',execution:'not-run',proof:'open',trust:'unassessed'});
assert.deepEqual(dx02.evidence, []);
assert.deepEqual(dx02.input.nodes, ['integer:1','editor-hole','word:+']);
assert.deepEqual(dx02.input.variants,
  ['editor-analysis','direct-admission','nested-quotation-admission','serialized-candidate-admission']);
assert.deepEqual(dx02.expected.hole_required_output_suffix, ['I64']);
assert.equal(dx02.expected.unresolved_constraints_retained, true);
assert.equal(dx02.expected.accepted_program_created, false);
assert.equal(dx02.expected.guest_requests, 0);
assert.equal(dx02.expected.protected_operations, 0);
if (process.argv.length === 3 && process.argv[2] === '--plan') {
  console.log(JSON.stringify({result:'ready-for-owner-review-not-execution',
    source_revision:revision, pre_promotion_case_sha256:caseHash,
    source_files:Object.keys(source).length,
    selected_rust:selected.tool_paths.quality_rust, selected_node:selected.tool_paths.node,
    variants:dx02.input.variants, positive:'compiled managed-Wasm 1 2 + -> I64(3)'}));
  process.exit(0);
}
const [destination, expectedRevision] = process.argv.slice(2);
assert.equal(process.argv.length, 4,
  'usage: SELECTED_NODE verification/dx02/gate.mjs NEW_EXTERNAL_DIRECTORY sha256:REVIEWED_REVISION');
assert.match(expectedRevision, /^sha256:[0-9a-f]{64}$/);
assert.equal(revision, expectedRevision, 'source changed since reviewed freeze');
const artifacts = path.resolve(destination);
assert.ok(artifacts !== root && !artifacts.startsWith(`${root}${path.sep}`),
  'DX-02 artifacts must be outside the repository');
fs.mkdirSync(artifacts);
for (const folder of ['home','tmp','inputs','executables']) fs.mkdirSync(path.join(artifacts,folder));
fs.writeFileSync(path.join(artifacts,'inputs',path.basename(caseFile)),caseBytes,{flag:'wx'});
const env = {...process.env,
  HOME:path.join(artifacts,'home'),
  CARGO_HOME:process.env.CARGO_HOME || path.join(os.homedir(),'.cargo'),
  CARGO_TARGET_DIR:path.join(artifacts,'target'),TMPDIR:path.join(artifacts,'tmp'),
  RUSTC_WRAPPER:'',RUSTC_WORKSPACE_WRAPPER:'',RUSTFLAGS:'',CARGO_ENCODED_RUSTFLAGS:'',
  PATH:`${rustBin}:${path.dirname(node)}:${selected.component_sync.linker_bin}:/run/current-system/sw/bin`,
};
const commands = [];
function run(label, executable, args, status=0) {
  const reply=spawnSync(executable,args,{cwd:root,env,timeout:900_000,maxBuffer:16*1024*1024});
  const stem=`${String(commands.length).padStart(2,'0')}-${label}`;
  fs.writeFileSync(path.join(artifacts,`${stem}.stdout`),reply.stdout??Buffer.alloc(0),{flag:'wx'});
  fs.writeFileSync(path.join(artifacts,`${stem}.stderr`),reply.stderr??Buffer.alloc(0),{flag:'wx'});
  assert.equal(reply.error,undefined,`${label}: process did not launch`);
  assert.equal(reply.status,status,`${label}: unexpected exit; stderr=${reply.stderr}`);
  commands.push({label,executable,executable_sha256:hash(fs.readFileSync(executable)),
    args,status:reply.status,stdout_sha256:hash(reply.stdout),stderr_sha256:hash(reply.stderr)});
  return reply;
}
const cargo = path.join(rustBin,'cargo');
for (const crate of ['noble-contracts','noble-cli']) {
  run(`${crate}-editor-test`,cargo,['test','--offline','-j','4','-p',crate,'--test','editor',
    '--','--test-threads','4']);
}
for (const [variant,test] of [
  ['direct-admission','typed_direct_admission_refuses_hole_before_prepared'],
  ['nested-quotation-admission','typed_nested_quotation_admission_refuses_hole_before_prepared'],
]) {
  const reply=run(`DX-02-typed-${variant}`,cargo,
    ['test','--offline','-j','4','-p','noble-contracts','--test','editor',
      test,'--','--exact','--test-threads','4']);
  assert.match(reply.stdout.toString('utf8'),/running 1 test/);
  assert.match(reply.stdout.toString('utf8'),new RegExp(`test ${test} \\.\\.\\. ok`));
}
run('frozen-cli-build',cargo,['build','--offline','-j','4','-p','noble-cli','--bin','noble']);
const compiled=path.join(artifacts,'target/debug/noble');
const frozen=path.join(artifacts,'executables/noble');
fs.copyFileSync(compiled,frozen,fs.constants.COPYFILE_EXCL);
fs.chmodSync(frozen,0o500);
assert.equal(hash(fs.readFileSync(compiled)),hash(fs.readFileSync(frozen)));

const fixture = (name, nodes) => {
  const file=path.join(artifacts,'inputs',`${name}.json`);
  fs.writeFileSync(file,JSON.stringify({format:1,nodes}),{flag:'wx'});
  return file;
};
const number=n=>({kind:'integer',value:n});
const word=value=>({kind:'word',value});
const hole=()=>({kind:'hole'});
const direct=fixture('direct',[number(1),hole(),word('+')]);
const nested=fixture('nested',[{kind:'quotation',nodes:[hole()]}]);
const serialized=fixture('serialized',[hole()]);
const positive=fixture('positive',[number(1),number(2),word('+')]);
const effectful=fixture('effectful',[number(1),hole(),word('test.emit')]);
function cli(label, action, file, status, emission) {
  const reply=run(label,frozen,['editor',action,file,
    ...(emission ? ['--emit',emission] : [])],status);
  const report=JSON.parse(reply.stdout);
  assert.equal(reply.stdout.toString('utf8').trim().split('\n').length,1,
    `${label}: exactly one report`);
  return report;
}
const analysis=cli('DX-02-editor-analysis','analyze',direct,0);
assert.equal(analysis.schema,'noble-editor-report/v1');
assert.equal(analysis.stage,'analysis');
assert.equal(analysis.outcome,'constraints');
assert.equal(analysis.accepted_program_created,false);
assert.equal(analysis.guest_requests,0);
assert.equal(analysis.protected_operations,0);
assert.equal(analysis.holes.length,1);
assert.equal(analysis.holes[0].input_stack,'?stack I64');
assert.equal(analysis.holes[0].required_output_stack,'?stack I64 I64');
assert.deepEqual(analysis.holes[0].required_output_suffix,dx02.expected.hole_required_output_suffix);
assert.deepEqual(analysis.holes[0].effect,{known:[],unresolved:true});
assert.deepEqual(analysis.effect,{known:[],unresolved:true});
const alternate=cli('scheme-derived-text-and-host-effect','analyze',effectful,0);
assert.deepEqual(alternate.holes[0].required_output_suffix,['Text']);
assert.equal(alternate.holes[0].effect.unresolved,true);
assert.deepEqual(alternate.effect,{known:[0],unresolved:true});
const nestedAnalysis=cli('nested-quotation-analysis','analyze',nested,0);
assert.equal(nestedAnalysis.holes.length,1);
assert.equal(nestedAnalysis.holes[0].effect.unresolved,true);
for (const [variant,file] of [
  ['direct-admission',direct],['nested-quotation-admission',nested],
  ['serialized-candidate-admission',serialized]
]) {
  const emission=path.join(artifacts,`forbidden-${variant}`);
  const report=cli(`DX-02-${variant}`,'admit',file,2,emission);
  assert.equal(report.stage,'admission',variant);
  assert.equal(report.outcome,'reject',variant);
  assert.equal(report.accepted_program_created,false,variant);
  assert.equal(report.guest_requests,dx02.expected.guest_requests,variant);
  assert.equal(report.protected_operations,dx02.expected.protected_operations,variant);
  assert.equal(report.module,undefined,`${variant}: no compiler/worker module report`);
  assert.equal(fs.existsSync(emission),false,`${variant}: no worker/Wasm artifact directory`);
}
const positiveEmission=path.join(artifacts,'hole-free-emission');
const runtime=cli('hole-free-independent-compiled-wasm','admit',positive,0,positiveEmission);
assert.equal(runtime.profile,'Core-Bootstrap');
assert.equal(runtime.backend,'managed-linear-memory');
assert.equal(runtime.stage,'wasm');
assert.equal(runtime.outcome,'normal');
assert.deepEqual(runtime.stack.map(cell=>[cell.type,cell.value]),[['I64','3']]);
assert.equal(runtime.guest_requests,0);
assert.equal(runtime.protected_operations,0);
assert.ok(runtime.metrics.execution_steps>0);
assert.match(runtime.module.wasm_sha256,/^[a-f0-9]{64}$/);
const wasm=path.join(positiveEmission,'engine/module-1.wasm');
assert.equal(hash(fs.readFileSync(wasm)),runtime.module.wasm_sha256);
run('selected-wasm-tools-validation',path.join(selected.tool_paths.wasm_tools.output,'bin/wasm-tools'),
  ['validate',wasm]);
const hostile = [
  ['duplicate-kind','{"format":1,"nodes":[{"kind":"hole","kind":"integer","value":1}]}'],
  ['duplicate-envelope','{"format":1,"nodes":[{"kind":"hole"}],"nodes":[]}'],
  ['unknown-effect','{"format":1,"nodes":[{"kind":"integer","value":1,"effect":"pure"},{"kind":"hole"}]}'],
  ['invalid-lexical-word','{"format":1,"nodes":[{"kind":"word","value":"1 2 +"}]}'],
  ['escaped-bracket-word','{"format":1,"nodes":[{"kind":"word","value":"\\u005b"}]}'],
  ['nested-forged-effect','{"format":1,"nodes":[{"kind":"quotation","nodes":[{"kind":"hole","effect":"pure"}]}]}'],
  ['escaped-duplicate-kind','{"format":1,"nodes":[{"ki\\u006ed":"hole","kind":"integer","value":3}]}'],
  ['distinct-output','{"format":1,"nodes":[{"kind":"integer","value":1},{"kind":"hole"},{"kind":"word","value":"test.emit"}]}'],
  ['wrong-value-type','{"format":1,"nodes":[{"kind":"integer","value":"1"}]}'],
  ['unsupported-version','{"format":2,"nodes":[]}'],
  ['malformed','{"format":1,"nodes":'],
  ['oversized',' '.repeat(65_537)],
];
for (const [name,text] of hostile) {
  const file=path.join(artifacts,'inputs',`hostile-${name}.json`);
  fs.writeFileSync(file,text,{flag:'wx'});
  const emission=path.join(artifacts,`forbidden-hostile-${name}`);
  const report=cli(`hostile-${name}`,'admit',file,2,emission);
  assert.equal(report.outcome,'reject',name);
  assert.equal(report.accepted_program_created,false,name);
  assert.equal(report.guest_requests,0,name);
  assert.equal(report.protected_operations,0,name);
  assert.equal(fs.existsSync(emission),false,`${name}: no guest worker emission`);
}
assert.equal(hash(content(caseFile)),caseHash,'prepromotion case mutated during gate');
const current={};
for (const file of Object.keys(source)) current[file]=hash(content(file));
assert.deepEqual(current,source,'source mutated during gate');
const files={};
function retained(directory) {
  for (const item of fs.readdirSync(directory,{withFileTypes:true}).sort((a,b)=>a.name.localeCompare(b.name))) {
    const file=path.join(directory,item.name);
    const relative=path.relative(artifacts,file);
    if (['target','home','tmp'].includes(relative)) continue;
    if (item.isDirectory()) retained(file);
    else {
      assert.ok(item.isFile()&&!item.isSymbolicLink(),'retained symlink/special file');
      files[relative]={sha256:hash(fs.readFileSync(file)),bytes:fs.statSync(file).size};
    }
  }
}
retained(artifacts);
const receipt={schema:'noble-editor-hole-acceptance/v1',kind:'test',case_id:'DX-02',
  case:{id:dx02.id,input:dx02.input,expected:dx02.expected},
  profile:'Editor-Draft',result:'passed',proof:'open',
  source_root:root,source_revision:revision,source_files:source,
  external_raw_output:artifacts,
  pre_promotion_case_sha256:caseHash,gate_sha256:source['verification/dx02/gate.mjs'],
  selected_rust:selected.tool_paths.quality_rust,selected_node:selected.tool_paths.node,
  binary_sha256:hash(fs.readFileSync(frozen)),compiled_wasm_sha256:runtime.module.wasm_sha256,
  compiled_wasm_path:path.relative(artifacts,wasm),
  accepted_source_path:'ordinary source preparation → independent kernel → managed-linear-memory Wasm',
  variants:dx02.input.variants,analysis:analysis.holes[0],
  variant_routes:{
    'editor-analysis':'DX-02-editor-analysis: compiled CLI/real source inference',
    'direct-admission':'DX-02-typed-direct-admission: typed frontend Candidate API',
    'nested-quotation-admission':'DX-02-typed-nested-quotation-admission: typed frontend Candidate API',
    'serialized-candidate-admission':'DX-02-serialized-candidate-admission: JSON/CLI ingress',
  },
  positive_result:runtime.stack,controls:hostile.map(([name])=>name),commands,retained_files:files,
  assumptions:['Selected pinned Rust, Node/V8, Wasm backend and host/engine implementation are trusted mechanisms; finite observations do not prove general refinement.'],
  non_claims:[
    'No hole executes or enters kernel candidate admission; no general language-hole grammar, owner-approved law, universal Rust/Lean proof or backend/host theorem.',
    'Whole-workspace boundary/architecture assurance did not pass; this receipt proves only selected DX-02 source, kernel and Wasm behavior and does not establish that assurance.',
  ]};
fs.writeFileSync(path.join(artifacts,'smoke.json'),JSON.stringify(receipt,null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({result:'passed',receipt:path.join(artifacts,'smoke.json'),
  source_revision:revision,commands:commands.length,variants:dx02.input.variants}));
