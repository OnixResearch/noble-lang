#!/usr/bin/env node
// Native-async-interfaces gate: build the selected shell and the pinned
// cancelling/delivering component, then observe every closure row from real
// host, kernel and Component Model state. Nothing here reads a constant the
// shell fabricates: ownership, deliveries, deadlines and receipts are all
// retained-table snapshots.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root=fs.realpathSync(path.resolve(path.dirname(fileURLToPath(import.meta.url)),'../..'));
const SCOPE=['Cargo.toml','Cargo.lock','rust-toolchain.toml','crates/noble-kernel','crates/noble-wasm/wit/worker-send.wit',
  'crates/noble-cli/tests/fixtures/worker-send-dispatch.noble',
  'crates/noble-cli/tests/fixtures/worker-send-finish.noble',
  'verification/worker-send','verification/worker01/fixtures',
  'specs/conformance/worker-cases.json','.cairn/changes/worker-cancellation-slice-native-async/design.md'];
const sha=bytes=>crypto.createHash('sha256').update(bytes).digest('hex');
const hash=file=>sha(fs.readFileSync(file));
const git=(...args)=>spawnSync('git',args,{cwd:root,encoding:'utf8',maxBuffer:64*1024*1024});
function argument(name){const at=process.argv.indexOf(name);return at>0?process.argv[at+1]:undefined;}
function command(file,args,options={}){const result=spawnSync(file,args,{cwd:root,encoding:'utf8',maxBuffer:256*1024*1024,timeout:1_800_000,...options});assert.equal(result.error,undefined,String(result.error));return result;}
const FIXTURES=['schema.noble','transition.noble','state.json','submit.json','cancel.json'];
const ROWS=[
  'authorized socket send','cancel before native send','core start traps after send admission',
  'core start sends before typed lookup failure','injected receiver error',
  'injected receiver corruption','native producer panic after send',
  'rejected native cancellation','unwound native cancellation',
  'failed fallback authority retirement','buffer quota preflight','no-grant',
  'mismatched-attempt-grant','expired-grant','candidate-generated-grant',
  'mismatched-source-grant','delivered once through guest finish',
  'domain error stays a domain error','suspended awaiting withheld result',
  'host deadline retires outstanding native work',
];
function main(){
  const destination=argument('--out');assert.ok(destination,'usage: gate.mjs --out NEW_DIRECTORY');
  const out=path.resolve(destination);assert.ok(!fs.existsSync(out),`output exists: ${out}`);
  fs.mkdirSync(out,{recursive:true});fs.mkdirSync(path.join(out,'raw'));
  const dirty=git('status','--porcelain','--untracked-files=all','--',...SCOPE).stdout;
  assert.equal(dirty,'',`${dirty}\ngate scope is dirty`);
  const files=git('ls-files','-z','--',...SCOPE).stdout.split('\0').filter(Boolean).sort();
  const sourceFiles=Object.fromEntries(files.map(file=>[file,{sha256:hash(path.join(root,file))}]));
  const design=fs.readFileSync(path.join(root,
    '.cairn/changes/worker-cancellation-slice-native-async/design.md'),'utf8');
  const closure=design.split('Native-async-interfaces gate closure')[1];
  assert.ok(closure,'design has no native-async-interfaces closure section');
  const projection={case:'native-async-interfaces',
    criteria:closure.split('\n').map(line=>line.trim()).filter(line=>line.startsWith('-')),
    rows:ROWS};
  // Build both selected products from this exact source scope.
  const buildDir=path.join(out,'build');fs.mkdirSync(buildDir,{recursive:true});
  const built=command('cargo',['build','--locked'],{
    env:{...process.env,CARGO_TARGET_DIR:path.join(buildDir,'target')}});
  fs.writeFileSync(path.join(out,'raw','cargo.stderr'),built.stderr);
  assert.equal(built.status,0,built.stderr);
  const cli=path.join(buildDir,'target','debug','noble');
  const peerBuilt=command('cargo',['build','--locked','--manifest-path',
    'verification/worker-send/peer/Cargo.toml'],{
    env:{...process.env,CARGO_TARGET_DIR:path.join(buildDir,'peer-target')}});
  fs.writeFileSync(path.join(out,'raw','peer.stderr'),peerBuilt.stderr);
  assert.equal(peerBuilt.status,0,peerBuilt.stderr);
  const peer=path.join(buildDir,'peer-target','debug','noble-worker-send-peer');
  // Re-seal the binaries so later mutation worktrees cannot substitute them.
  const sealed=path.join(out,'sealed');fs.mkdirSync(sealed,{recursive:true});
  for (const [name,binary] of [['cli',cli],['peer',peer]]) {
    fs.copyFileSync(binary,path.join(sealed,name));
    fs.chmodSync(path.join(sealed,name),0o555);
  }
  const fixtures=path.join(out,'fixtures');fs.mkdirSync(fixtures,{recursive:true});
  for (const name of FIXTURES) {
    fs.copyFileSync(path.join(root,'verification/worker01/fixtures',name),path.join(fixtures,name));
  }
  const prepared=command(path.join(sealed,'cli'),['worker','prepare',
    path.join(fixtures,'transition.noble'),'--schema',path.join(fixtures,'schema.noble'),
    '--out',path.join(fixtures,'package.json')]);
  fs.writeFileSync(path.join(out,'raw','prepare.stdout.json'),prepared.stdout);
  assert.equal(prepared.status,0,prepared.stderr||prepared.stdout);
  const smokeDir=path.join(out,'rows');
  let failures=[];
  let rows={};
  let summary={cases:[]};
  try{
    const smoke=command(process.execPath,['verification/worker-send/test.mjs',
      path.join(sealed,'cli'),path.join(sealed,'peer'),fixtures,smokeDir]);
    fs.writeFileSync(path.join(out,'raw','smoke.stdout'),smoke.stdout);
    fs.writeFileSync(path.join(out,'raw','smoke.stderr'),smoke.stderr);
    assert.equal(smoke.status,0,smoke.stderr||smoke.stdout);
    summary=JSON.parse(smoke.stdout);
    assert.deepEqual(summary.cases,ROWS);
    // A mismatched worker source grant is refused before any native host
    // exists, so that row legitimately carries no host observation.
    const report=name=>{
      const descriptor=JSON.parse(fs.readFileSync(path.join(smokeDir,
        `${name.replaceAll(' ','-')}.report.json`),'utf8'));
      return descriptor.host?descriptor:{...descriptor,
        host:{lifecycle_sequence:[],task_deliveries:0,successful_result_deliveries:0,
          owner_returns:0,owner_resurrections:0,task_outstanding_pins:0,
          resource_native_pins:0,task_reserved_bytes:0,authority_invocation:null,
          operation_attempt_outcome:null,receipts:[],boundaries:[]}};
    };
    rows=Object.fromEntries(ROWS.map(name=>{
      const descriptor=report(name);
      return [name,{outcome:descriptor.outcome,mode:descriptor.mode,
        future_consumed:descriptor.future_consumed,
        native_pins_at_guest_exit:descriptor.native_pins_at_guest_exit,
        handle_disposition:descriptor.handle_disposition,
        sequence:descriptor.host.lifecycle_sequence,
        deliveries:descriptor.host.task_deliveries,
        successful_deliveries:descriptor.host.successful_result_deliveries,
        owner_returns:descriptor.host.owner_returns,
        owner_resurrections:descriptor.host.owner_resurrections,
        task_outstanding_pins:descriptor.host.task_outstanding_pins,
        resource_native_pins:descriptor.host.resource_native_pins,
        task_reserved_bytes:descriptor.host.task_reserved_bytes,
        invocations:descriptor.host.authority_invocation,
        operation_attempt:descriptor.host.operation_attempt_outcome,
        receipts:descriptor.host.receipts.length,
        boundaries:descriptor.host.boundaries.length}];
    }));
    const expect=(name,field,value)=>{
      if (rows[name][field]!==value) failures.push(`${name}: ${field} ${JSON.stringify(rows[name][field])}!==${JSON.stringify(value)}`);
    };
    expect('authorized socket send','outcome','cancelled-retired');
    expect('authorized socket send','deliveries',0);
    expect('cancel before native send','outcome','cancelled-retired');
    expect('delivered once through guest finish','outcome','delivered-once');
    expect('delivered once through guest finish','deliveries',1);
    expect('delivered once through guest finish','successful_deliveries',1);
    expect('delivered once through guest finish','owner_returns',1);
    expect('delivered once through guest finish','owner_resurrections',0);
    expect('domain error stays a domain error','outcome','domain-error-delivered');
    expect('domain error stays a domain error','successful_deliveries',0);
    expect('domain error stays a domain error','invocations','Failed');
    expect('suspended awaiting withheld result','outcome','suspended-retired');
    expect('suspended awaiting withheld result','deliveries',0);
    expect('suspended awaiting withheld result','native_pins_at_guest_exit',1);
    expect('host deadline retires outstanding native work','outcome','host-deadline-retired');
    expect('host deadline retires outstanding native work','deliveries',0);
    expect('host deadline retires outstanding native work','invocations','Failed');
    const parked=['suspended awaiting withheld result','host deadline retires outstanding native work'];
    for (const name of parked) {
      for (const marker of ['late-native-completion','retirement-finished']) {
        if (!rows[name].sequence.includes(marker)) failures.push(`${name}: missing ${marker}`);
      }
    }

  }catch(error){
    // A failed row must still produce a failure receipt with the real
    // assertion text, so the mutation harness has causal evidence.
    failures.push(String(error.message).split('\n')[0]);
  }
  const receipt={schema:'noble-worker-send-gate/v1',case:'native-async-interfaces',
    result:failures.length?'failed':'passed',failures,
    tested_commit:git('rev-parse','HEAD').stdout.trim(),
    source_scope:SCOPE,source_revision:`sha256:${sha(JSON.stringify(sourceFiles))}`,
    source_file_count:Object.keys(sourceFiles).length,
    case_projection:projection,case_projection_sha256:sha(JSON.stringify(projection)),
    binaries:{cli:{sha256:hash(path.join(sealed,'cli'))},
      peer:{sha256:hash(path.join(sealed,'peer'))},
      component:{sha256:hash(path.join(smokeDir,'component','component.wasm'))}},
    rows,smoke_cases:summary.cases,source_files:sourceFiles,
    non_claims:['The datagram peer is local-socket-only and not the canonical WORKER-01 transfer.',
      'Boundaries are retained-table snapshots of this shell, not kernel witnesses.',
      'The host deadline is the shell bounded driver, distinct from source fuel exhaustion.']};
  fs.writeFileSync(path.join(out,'receipt.json'),`${JSON.stringify(receipt,null,2)}\n`);
  console.log(JSON.stringify({result:receipt.result,failures,rows,source_revision:receipt.source_revision}));
  process.exitCode=failures.length?1:0;
}
main();
