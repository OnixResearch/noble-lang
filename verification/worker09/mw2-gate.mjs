#!/usr/bin/env node
// WORKER-09 MW2 rows: independently build the selected M6 async peer, then
// observe a real Component Model suspension and a driver deadline while native
// work remains pinned. The eight MW1 rows stay in the immutable earlier receipt.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root=fs.realpathSync(path.resolve(path.dirname(fileURLToPath(import.meta.url)),'../..'));
const SCOPE=['crates','Cargo.toml','Cargo.lock','rust-toolchain.toml','flake.nix','flake.lock','policy/tool-selection.json',
  'verification/m6/build.mjs','verification/m6/pins.json','verification/m6/peer','verification/m6/cases',
  'verification/worker09/mw2-gate.mjs','specs/conformance/worker-cases.json'];
const sha=bytes=>crypto.createHash('sha256').update(bytes).digest('hex');
const hash=file=>sha(fs.readFileSync(file));
const git=(...args)=>spawnSync('git',args,{cwd:root,encoding:'utf8',maxBuffer:64*1024*1024}).stdout;
function argument(name){const at=process.argv.indexOf(name);return at>0?process.argv[at+1]:undefined;}
function command(file,args,options={}){const result=spawnSync(file,args,{cwd:root,encoding:'utf8',maxBuffer:256*1024*1024,timeout:1_800_000,...options});assert.equal(result.error,undefined,String(result.error));return result;}
function main(){
  const destination=argument('--out');assert.ok(destination,'usage: mw2-gate.mjs --out NEW_DIRECTORY');
  const out=path.resolve(destination);assert.ok(!fs.existsSync(out),`output exists: ${out}`);fs.mkdirSync(out,{recursive:true});fs.mkdirSync(path.join(out,'raw'));
  const dirty=git('status','--porcelain','--untracked-files=all','--',...SCOPE);assert.equal(dirty,'',dirty);
  const files=git('ls-files','-z','--',...SCOPE).split('\0').filter(Boolean).sort();
  const sourceFiles=Object.fromEntries(files.map(file=>[file,{sha256:hash(path.join(root,file))}]));
  const cases=JSON.parse(fs.readFileSync(path.join(root,'specs/conformance/worker-cases.json'))).cases;
  const worker=cases.find(row=>row.id==='WORKER-09');const {id,profile,kind,requirements,input,expected}=worker;
  const projection={id,profile,kind,requirements,input,expected};
  const buildDir=path.join(out,'build');const built=command(process.execPath,['verification/m6/build.mjs',buildDir]);
  fs.writeFileSync(path.join(out,'raw','build.stdout'),built.stdout);fs.writeFileSync(path.join(out,'raw','build.stderr'),built.stderr);
  assert.equal(built.status,0,built.stderr||built.stdout);const build=JSON.parse(fs.readFileSync(path.join(buildDir,'build.json')));assert.equal(build.result,'built');
  const cli=path.join(buildDir,build.binaries.cli.snapshot);const peer=path.join(buildDir,build.binaries.peer.snapshot);
  const componentDir=path.join(out,'component');const compiled=command(cli,['component','compile','crates/noble-wasm/wit/async.wit','bootstrap',componentDir,'order=verification/m6/cases/order.noble']);
  fs.writeFileSync(path.join(out,'raw','compile.stdout.json'),compiled.stdout);fs.writeFileSync(path.join(out,'raw','compile.stderr'),compiled.stderr);assert.equal(compiled.status,0,compiled.stderr||compiled.stdout);
  const component=path.join(componentDir,'component.wasm');
  const suspendedRun=command(peer,[component,'order','success']);fs.writeFileSync(path.join(out,'raw','suspension.stdout.json'),suspendedRun.stdout);fs.writeFileSync(path.join(out,'raw','suspension.stderr'),suspendedRun.stderr);assert.equal(suspendedRun.status,0,suspendedRun.stderr||suspendedRun.stdout);const suspension=JSON.parse(suspendedRun.stdout);
  const deadlineRun=command(peer,['--progress','blocking-deadline']);fs.writeFileSync(path.join(out,'raw','deadline.stdout.json'),deadlineRun.stdout);fs.writeFileSync(path.join(out,'raw','deadline.stderr'),deadlineRun.stderr);assert.equal(deadlineRun.status,0,deadlineRun.stderr||deadlineRun.stdout);const deadline=JSON.parse(deadlineRun.stdout);
  const rows={
    'permitted-suspension':{execution:'passed',outcome:suspension.suspended?.outcome,typed_output:suspension.suspended?.typed_output!==null,pending_owner:suspension.suspended?.pending_owner,
      task_pending:suspension.suspended?.host?.tasks?.pending,pins:suspension.suspended?.host?.tasks?.outstanding_pins,deliveries:suspension.suspended?.host?.accounting?.deliveries},
    'host-deadline-native-work-outstanding':{execution:'passed',outcome:deadline.progress?.outcome==='deadline'?'host-deadline':deadline.progress?.outcome,typed_output:false,native_owner:'host-retirement',
      pins_at_ack:deadline.progress?.native_pins_at_ack,pins_after_join:deadline.progress?.native_pins_after_join,tasks_retiring:deadline.at_ack?.tasks?.retiring,deliveries:deadline.at_ack?.accounting?.deliveries,
      fuel_remaining:deadline.progress?.guest_work_remaining}
  };
  const failures=[];const declared=Object.fromEntries(input.variants.map(row=>[row.case,row]));
  const s=rows['permitted-suspension'],d=rows['host-deadline-native-work-outstanding'];
  if(!(s.outcome===declared['permitted-suspension'].outcome&&s.typed_output===declared['permitted-suspension'].typed_output
    &&s.pending_owner===declared['permitted-suspension'].pending_owner&&s.task_pending===1&&s.pins===1&&s.deliveries===0))failures.push('permitted-suspension differs');
  if(!(d.outcome===declared['host-deadline-native-work-outstanding'].outcome&&d.typed_output===declared['host-deadline-native-work-outstanding'].typed_output
    &&d.native_owner===declared['host-deadline-native-work-outstanding'].native_owner&&d.pins_at_ack===1&&d.pins_after_join===0&&d.tasks_retiring===1&&d.deliveries===0&&d.fuel_remaining>0))failures.push('deadline-native-work differs');
  const receipt={schema:'noble-worker09-mw2-acceptance/v1',case:'WORKER-09',result:failures.length?'failed':'passed',failures,
    tested_commit:git('rev-parse','HEAD').trim(),source_scope:SCOPE,source_revision:`sha256:${sha(JSON.stringify(sourceFiles))}`,
    source_file_count:Object.keys(sourceFiles).length,case_projection:projection,case_projection_sha256:sha(JSON.stringify(projection)),
    binaries:{cli:{sha256:hash(cli)},peer:{sha256:hash(peer)},component:{sha256:hash(component)}},rows,
    runs:{suspension,deadline,build_report_sha256:hash(path.join(buildDir,'build.json'))},source_files:sourceFiles,
    non_claims:['The suspension is an independent Component Model async peer observation, not a transferable guest continuation.','The deadline uses the M6 bounded cooperative driver and retained native pin; it is distinct from source fuel exhaustion.','The earlier WORKER-09 MW1 receipt is unchanged; this receipt covers only its two MW2 rows.']};
  fs.writeFileSync(path.join(out,'acceptance.json'),`${JSON.stringify(receipt,null,2)}\n`);console.log(JSON.stringify({result:receipt.result,failures,rows,source_revision:receipt.source_revision}));process.exitCode=failures.length?1:0;
}
main();
