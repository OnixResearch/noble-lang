// Actual selected CLI controls for slot dispatch, frozen replay, and refusal.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {spawn} from 'node:child_process';

export async function slotControls(binary, authority, directory, mode='effects') {
  const events=path.join(directory,'slot.events.jsonl');
  const stdout=path.join(directory,'slot.stdout.jsonl');
  const stderr=path.join(directory,'slot.stderr');
  const requests=path.join(directory,'slot.requests.jsonl');
  for(const file of [stdout,stderr,requests]) fs.writeFileSync(file,'',{flag:'wx',mode:0o600});
  // Reuse the real live-slot CLI fixture's pipe discipline, not a mock worker.
  const child=spawn('/bin/sh',['-c','cat | exec "$1" live slot --engine v8 --authority "$2"',
    'sh',binary,authority],{cwd:directory,stdio:['pipe','pipe','pipe'],
    env:{...process.env,NOBLE_SAFETY_OBSERVATION:events}});
  let buffer='',closed=false;
  const pending=[],ready=[];
  const finished=new Promise(resolve=>child.once('close',code=>resolve(code)));
  child.stdout.on('data',bytes=>{
    fs.appendFileSync(stdout,bytes);buffer+=bytes.toString();
    for(let end;(end=buffer.indexOf('\n'))>=0;) {
      const row=JSON.parse(buffer.slice(0,end));buffer=buffer.slice(end+1);
      const waiter=pending.shift();waiter?waiter.resolve(row):ready.push(row);
    }
  });
  child.stderr.on('data',bytes=>fs.appendFileSync(stderr,bytes));
  child.once('close',()=>{closed=true;for(const waiter of pending.splice(0))waiter.reject(Error('slot CLI closed before reply'));});
  const next=()=>ready.length?Promise.resolve(ready.shift()):new Promise((resolve,reject)=>{
    if(closed)return reject(Error('slot CLI closed'));
    const timer=setTimeout(()=>reject(Error('slot CLI reply timeout')),15000);
    pending.push({resolve:row=>{clearTimeout(timer);resolve(row);},reject:error=>{clearTimeout(timer);reject(error);}});
  });
  const issue=async request=>{
    const bytes=JSON.stringify(request)+'\n';fs.appendFileSync(requests,bytes);child.stdin.write(bytes);return next();
  };
  const tally=()=>{
    const rows=fs.readFileSync(events,'utf8').trimEnd().split('\n').map(JSON.parse);
    return {guest_requests:rows.filter(r=>r.event==='guest-request').length,
      protected_operations:rows.filter(r=>r.event==='protected-operation').length};
  };
  const phases=[];
  const phase=async(name,request)=>{
    const before=tally(),report=await issue(request),after=tally();
    phases.push({name,request,report,observation:{guest_requests:after.guest_requests-before.guest_requests,
      protected_operations:after.protected_operations-before.protected_operations}});
    return report;
  };
  try {
    assert.equal((await next()).outcome,'configured');
    if(mode==='hard-kill') {
      const rows=fs.readFileSync(events,'utf8').trimEnd().split('\n').map(JSON.parse);
      const root=rows.find(row=>row.event==='begin'),worker=rows.find(row=>row.event==='child-begin');
      assert.ok(Number.isSafeInteger(root?.pid)&&root.pid>1);
      assert.ok(Number.isSafeInteger(worker?.pid)&&worker.pid>1);
      const status=fs.readFileSync(`/proc/${worker.pid}/status`,'utf8');
      assert.equal(Number(/^PPid:\s+(\d+)$/m.exec(status)?.[1]),root.pid,
        'hard termination is restricted to this newly spawned CLI child');
      process.kill(worker.pid,'SIGKILL');
      child.stdin.end();const exit=await finished;
      const result={events,stdout,stderr,requests,phases:[],exit,
        killed_worker_pid:worker.pid,owned_parent_pid:root.pid,signal:'SIGKILL'};
      fs.writeFileSync(path.join(directory,'slot-controls.json'),JSON.stringify(result,null,2)+'\n',{flag:'wx'});
      return result;
    }
    assert.equal((await issue({operation:'install',id:'caller',source:'slot.invoke',
      inputs:['I64',mode==='pure'?'LiveRef<I64,I64,pure>':'LiveRef<I64,I64,test.emit>']})).outcome,'installed');
    if(mode==='pure') {
      assert.equal((await issue({operation:'install',id:'keeper',source:'[ 1 + ]',inputs:[]})).outcome,'installed');
      assert.equal((await issue({operation:'publish',slot:'Q',id:'keeper',program_index:1,expected_epoch:'0'})).outcome,'published');
      const report=await phase('pure-dispatch',{operation:'invoke',id:'caller',
        inputs:[{kind:'i64',value:'5'}],refs:[{position:1,ordinal:0,slot:'Q'}]});
      assert.equal(report.outcome,'executed');assert.equal(report.guest_requests,1);assert.equal(report.protected_operations,0);
      assert.deepEqual(report.stack,[{kind:1,value:'6'}]);
    } else {
    assert.equal((await issue({operation:'define',id:'definition',name:'traced',
      source:'def traced [ drop "A" test.emit drop "B" test.emit drop 42 ]',inputs:[]})).outcome,'definition-retained');
    assert.equal((await issue({operation:'install',id:'named',source:'traced',inputs:['I64'],selected_name:'traced'})).outcome,'installed');
    assert.equal((await issue({operation:'publish',slot:'emit',id:'named',program_index:0,expected_epoch:'0'})).outcome,'published');
    const inputs=[{kind:'i64',value:'5'}],refs=[{position:1,ordinal:0,slot:'emit'}];
    const recorded=await phase('live-dispatch-effects',{operation:'invoke',id:'caller',inputs,refs,record:true});
    assert.equal(recorded.outcome,'executed');assert.equal(recorded.guest_requests,3);assert.equal(recorded.protected_operations,2);
    assert.deepEqual(recorded.request_trace.map(r=>r.operation),['dispatch','effect','effect']);
    assert.equal((await issue({operation:'policy',grant:{operation:'effect',slotId:'emit',allowed:false}})).outcome,'policy-updated');
    const replayed=await phase('frozen-replay',{operation:'replay',token:recorded.replay_token,inputs,refs,
      expected_trace:recorded.request_trace,expected_stack:recorded.stack,expected_identity:recorded.frozen_identity});
    assert.equal(replayed.outcome,'replay-matched');assert.equal(replayed.guest_requests,3);assert.equal(replayed.protected_operations,0);
    assert.equal(replayed.scripted_operations,2);assert.deepEqual(replayed.actual_trace,recorded.request_trace);
    const refused=await phase('refused-emit',{operation:'invoke',id:'caller',inputs,refs});
    assert.equal(refused.outcome,'execution-failed');assert.equal(refused.session_state,'terminated-after-guest-failure');
    assert.equal(refused.guest_requests,2);
    assert.equal(refused.protected_operations,0);
    assert.ok(refused.request_trace.some(r=>r.operation==='effect'&&r.outcome!=='performed'),'real guest emit reaches denied effect');
    const terminalEvents=fs.readFileSync(events,'utf8').trimEnd().split('\n').map(JSON.parse);
    assert.equal(terminalEvents.filter(row=>row.event==='child-end').length,1,
      'terminal guest reply must already have sealed the child operation stream');
    }
    child.stdin.end();const exit=await finished;
    assert.equal(exit,mode==='pure'?0:2,fs.readFileSync(stderr,'utf8'));assert.equal(buffer,'');assert.equal(ready.length,0);
    const result={events,stdout,stderr,requests,phases,exit};
    fs.writeFileSync(path.join(directory,'slot-controls.json'),JSON.stringify(result,null,2)+'\n',{flag:'wx'});
    return result;
  } catch(error) {child.kill();await finished;throw error;}
}
