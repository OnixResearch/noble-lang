#!/usr/bin/env node
// WORKER-01 acceptance gate: explicit preparation, pure Worker transitions,
// independently authorized shell Send, cancellation commit, post-commit
// Cancelled event, late native success and fully accounted retirement.
import assert from 'node:assert/strict';
import { execFileSync, spawnSync } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = fs.realpathSync(path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..'));
const here = path.join(root, 'verification/worker01');
const SCOPE = ['crates', 'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml',
  'verification/worker01/gate.mjs', 'verification/worker01/fixtures'];
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const hash = file => sha(fs.readFileSync(file));
const git = (...args) => execFileSync('git', args, { cwd: root, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 });
function argument(name) { const at = process.argv.indexOf(name); return at > 0 ? process.argv[at + 1] : undefined; }
function build() {
  const p = spawnSync('cargo', ['build','--locked','-p','noble-cli','--bin','noble','--message-format=json-render-diagnostics'],
    {cwd:root,encoding:'utf8',maxBuffer:256*1024*1024});
  assert.equal(p.status,0,p.stderr);
  const rows=p.stdout.split('\n').filter(Boolean).map(JSON.parse)
    .filter(r=>r.reason==='compiler-artifact'&&r.target?.name==='noble'&&r.executable);
  assert.equal(rows.length,1); return rows[0].executable;
}
function fixture(name){return fs.readFileSync(path.join(here,'fixtures',name));}
function run(noble,out,name,args,{files={},keep=[]}={}){
  const cwd=fs.mkdtempSync(path.join(os.tmpdir(),'noble-worker01-'));
  for(const [file,bytes] of Object.entries(files))fs.writeFileSync(path.join(cwd,file),bytes);
  const p=spawnSync(noble,args,{cwd,encoding:'buffer',timeout:300000,maxBuffer:32*1024*1024});
  assert.equal(p.error,undefined,`${name}: ${p.error}`);
  fs.writeFileSync(path.join(out,'raw',`${name}.stdout.json`),p.stdout);
  fs.writeFileSync(path.join(out,'raw',`${name}.stderr`),p.stderr);
  const retained={}; for(const file of keep)retained[file]=fs.readFileSync(path.join(cwd,file));
  const leftovers=fs.readdirSync(cwd).filter(file=>!(file in files)&&!keep.includes(file));
  fs.rmSync(cwd,{recursive:true,force:true});
  return {name,argv:['noble',...args],exit:p.status,stdout_sha256:sha(p.stdout),stderr_sha256:sha(p.stderr),
    leftovers,report:JSON.parse(p.stdout.toString()),retained};
}
function state(value){
  const inner=value.value;
  const pending=inner.right.arm==='left'?null:inner.right.value.value.value;
  return {count:{type:'I64',value:inner.left.value},pending:pending===null?null:`attempt-${pending}`};
}
function actions(value){return value.items.map(row=>{
  if(row.arm==='right')return {kind:'Noop'};
  return {kind:'Send',attempt:`attempt-${row.value.left.value.value}`,text:row.value.right.value};
});}
function main(){
  const destination=argument('--out'); assert.ok(destination,'usage: gate.mjs --out NEW_DIRECTORY');
  const out=path.resolve(destination); assert.ok(!fs.existsSync(out),`output exists: ${out}`);
  const dirty=git('status','--porcelain','--untracked-files=all','--',...SCOPE); assert.equal(dirty,'',dirty);
  const files=git('ls-files','-z','--',...SCOPE).split('\0').filter(Boolean).sort();
  const sourceFiles=Object.fromEntries(files.map(file=>[file,{sha256:hash(path.join(root,file))}]));
  const cases=JSON.parse(fs.readFileSync(path.join(root,'specs/conformance/worker-cases.json'))).cases;
  const row=cases.find(value=>value.id==='WORKER-01'); const {id,profile,kind,requirements,input,expected}=row;
  const projection={id,profile,kind,requirements,input,expected}; const noble=build();
  fs.mkdirSync(path.join(out,'raw'),{recursive:true});
  const baseNames=['schema.noble','transition.noble','state.json','submit.json','cancel.json','grant.json'];
  const base=Object.fromEntries(baseNames.map(name=>[name,fixture(name)]));
  const prepared=run(noble,out,'prepare',['worker','prepare','transition.noble','--schema','schema.noble','--out','package.json'],
    {files:base,keep:['package.json']});
  const packageBytes=prepared.retained['package.json']; fs.writeFileSync(path.join(out,'raw','package.json'),packageBytes);
  const shell=run(noble,out,'shell',['worker','shell','package.json','--schema','schema.noble','--state','state.json',
    '--submit','submit.json','--cancel','cancel.json','--authority','grant.json'],
    {files:{...base,'package.json':packageBytes}});
  const controlMap={
    'submit-while-pending':'submit-while-pending',
    'cancel-wrong-attempt':'cancel-wrong-attempt',
    'cancel-without-pending-attempt':'cancel-without-pending',
    'wrapping-submission-count':'wrapping-submission-count'};
  const controls={};
  for(const control of input.transition_controls){const f=controlMap[control.case]; controls[control.case]=run(noble,out,control.case,
    ['worker','typed','package.json','--schema','schema.noble','--state',`${f}-state.json`,'--event',`${f}-event.json`],
    {files:{...base,'package.json':packageBytes,[`${f}-state.json`]:fixture(`${f}-state.json`),[`${f}-event.json`]:fixture(`${f}-event.json`)}});}
  const report=shell.report, first=report.first_guest.typed_output, second=report.cancel_guest.typed_output;
  const observed={stage:'prepare-worker-shell-and-retirement',outcome:report.outcome,
    states:[state(first.state),state(second.state)],actions:[actions(first.actions),actions(second.actions)],
    worker_host_requests:report.worker_host_requests,shell_operation_attempts:report.protected_operation_attempts,
    preparation_trace_separate:prepared.report.schema==='noble-worker-preparation/v1'
      && !Object.hasOwn(report,'service_effects') && prepared.report.service_effects.packages_written===1,
    owner_after_admission:report.owner_after_admission,state_at_cancel_ack:report.state_at_cancel_ack,
    pins_at_cancel_ack:report.pins_at_cancel_ack,state_after_late_completion:report.task_state,
    pins_after_late_completion:report.outstanding_native_pins,
    successful_result_deliveries:report.successful_result_deliveries,
    late_worker_events:report.late_worker_events,owner_resurrections:report.owner_resurrections,
    external_send_outcome_at_cancel:report.external_send_outcome_at_cancel,
    external_rollback_claimed:report.external_rollback_claimed};
  const failures=[];
  const sequence=report.lifecycle_sequence;
  if(JSON.stringify(sequence)!==JSON.stringify([
    'admission-commit','cancellation-commit','cancelled-worker-event','late-native-completion']))
    failures.push(`lifecycle order differs: ${JSON.stringify(sequence)}`);
  if(JSON.stringify(sequence.filter(event=>event!=='cancelled-worker-event'))!==JSON.stringify(input.task_order))
    failures.push('case task_order differs from observed lifecycle');
  for(const run of [prepared,shell,...Object.values(controls)])if(run.leftovers.length)failures.push(`${run.name}: left files ${run.leftovers}`);
  for(const [field,value] of Object.entries(expected))if(JSON.stringify(observed[field])!==JSON.stringify(value))
    failures.push(`${field}: expected ${JSON.stringify(value)}, observed ${JSON.stringify(observed[field])}`);
  for(const control of input.transition_controls){const actual=controls[control.case].report.typed_output;
    const actualState=state(actual.state),actualActions=actions(actual.actions);
    const wanted={count:control.expected_count,pending:control.expected_pending,actions:control.expected_actions};
    if(JSON.stringify(actualState.count)!==JSON.stringify(wanted.count)||actualState.pending!==wanted.pending
      ||JSON.stringify(actualActions)!==JSON.stringify(wanted.actions))failures.push(`${control.case}: transition result differs`);}
  if(!(shell.exit===2&&prepared.exit===0&&report.late_success_receipt===true&&report.authority_invocation==='cancelled'
    && report.task_deliveries===0&&report.resource_state==='Retired'&&report.resource_native_pins===0))
    failures.push('shell receipt or final retirement differs');
  const receipt={schema:'noble-worker01-acceptance/v1',case:'WORKER-01',result:failures.length?'failed':'passed',failures,
    tested_commit:git('rev-parse','HEAD').trim(),source_scope:SCOPE,
    source_revision:`sha256:${sha(JSON.stringify(sourceFiles))}`,source_file_count:Object.keys(sourceFiles).length,
    case_projection:projection,case_projection_sha256:sha(JSON.stringify(projection)),
    binary:{name:'noble',sha256:hash(noble),build:'cargo build --locked -p noble-cli --bin noble'},
    fixtures:Object.fromEntries(fs.readdirSync(path.join(here,'fixtures')).sort().map(name=>[name,hash(path.join(here,'fixtures',name))])),
    package_sha256:sha(packageBytes),observed,runs:{prepared,shell,controls},source_files:sourceFiles,
    input_mapping:{task_order:'The report snapshots admission before cancellation, then the cancellation acknowledgement before the Cancelled transition, then late completion/final retirement.',
      external_send_outcome_at_cancel:'Authority attempt_snapshot immediately after approved Unknown observation.',
      preparation_service:'A separate noble worker prepare process publishes create-new; shell evaluation has no service trace.'},
    non_claims:['The native operation is one bounded in-process datagram, not network delivery or exactly-once external semantics.',
      'Late OperationSuccess supports an operation receipt while the invocation stays Cancelled; it is not a successful task delivery.',
      'Proof remains open.']};
  fs.writeFileSync(path.join(out,'acceptance.json'),`${JSON.stringify(receipt,null,2)}\n`);
  console.log(JSON.stringify({result:receipt.result,failures,observed,source_revision:receipt.source_revision,binary_sha256:receipt.binary.sha256}));
  process.exitCode=failures.length?1:0;
}
main();
