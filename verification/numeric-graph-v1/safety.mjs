#!/usr/bin/env node
// Fresh, bounded safety observations. Never rewrites historical packets or cases.
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import {spawnSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';
import {parseArgs} from 'node:util';
import {forgeSnapshot} from './acceptance.mjs';
import {slotControls} from './safety-slot.mjs';
const root = fs.realpathSync(path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..'));
const {values:args} = parseArgs({options:{binary:{type:'string'},'plain-binary':{type:'string'},
  'current-plain-binary':{type:'string'},
  'test-binary':{type:'string'},'build-source':{type:'string'},out:{type:'string'}},strict:true});
for (const key of ['binary','plain-binary','current-plain-binary','test-binary','build-source','out']) assert.ok(path.isAbsolute(args[key] ?? ''), `${key}: absolute path`);
// mkdir is deliberately nonrecursive: resolve its existing parent before any
// writes, so an external-looking symlink cannot redirect evidence into source.
const requestedOut = path.resolve(args.out);
const out = path.join(fs.realpathSync(path.dirname(requestedOut)),path.basename(requestedOut));
assert.ok(!out.startsWith(root + '/') && out !== root, 'external output required');
assert.ok(!fs.existsSync(out), 'fresh output required');
fs.mkdirSync(out,{mode:0o700});
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const json = file => JSON.parse(fs.readFileSync(file));
const put = (file, value) => fs.writeFileSync(path.join(out,file),JSON.stringify(value,null,2)+'\n',{flag:'wx'});
const sources = () => {
  const files = {};
  const walk = base => {
    for (const entry of fs.readdirSync(path.join(root,base),{withFileTypes:true})) {
      const name = `${base}/${entry.name}`;
      if (entry.isDirectory()) walk(name);
      else if (entry.isFile()) files[name] = {sha256:sha(fs.readFileSync(path.join(root,name))),mode:fs.statSync(path.join(root,name)).mode & 0o777};
      else throw Error(`unsupported source entry ${name}`);
    }
  };
  for (const base of ['crates','policy','nix','verification/numeric-graph-v1']) walk(base);
  for (const file of ['Cargo.toml','Cargo.lock','flake.nix','flake.lock','dylint.toml'])
    files[file] = {sha256:sha(fs.readFileSync(path.join(root,file))),mode:fs.statSync(path.join(root,file)).mode & 0o777};
  return files;
};
const beforeSources = sources();
put('source-manifest.json',beforeSources);
const buildSource=json(args['build-source']);
assert.equal(buildSource.feature,'safety-observe');
for(const [file,observation] of Object.entries(buildSource.files)) sameSource(file,observation);
function sameSource(file,observation) {
  assert.deepEqual(beforeSources[file],observation,`built source ${file}`);
}
const builtPaths=Object.keys(beforeSources).filter(p=>p.startsWith('crates/')||['Cargo.toml','Cargo.lock'].includes(p)).sort();
assert.deepEqual(Object.keys(buildSource.files).sort(),builtPaths,'complete built source scope');
put('build-source.json',buildSource);
const cases = json(path.join(root,'specs/conformance/numeric-graph-cases.json'));
const originalCases = json(path.join(root,'verification/numeric-graph-v1/prepromotion-cases.json'));
for (const suffix of ['02','03','04','05','09','11','12']) {
  const id = `NGRAPH-CASE-${suffix}`;
  const current = cases.cases.find(c=>c.id===id);
  const original = originalCases.cases.find(c=>c.case_id===id)?.record;
  assert.ok(current && original,`${id}: complete immutable original contract required`);
  for (const key of ['id','profile','kind','requirements','input','expected'])
    assert.deepEqual(current[key],original[key],`${id}: original ${key} unchanged`);
}
put('cases.json',cases);
const selectedNode = json(path.join(root,'policy/tool-selection.json')).tool_paths.node.output + '/bin/node';
assert.equal(fs.realpathSync(process.execPath),fs.realpathSync(selectedNode),'selected Node');
const planDir = path.join(out,'plan');
const planned = spawnSync(selectedNode,[path.join(root,'verification/numeric-graph-v1/acceptance.mjs'),'plan',planDir],{cwd:root,encoding:'utf8'});
assert.equal(planned.status,0,planned.stderr);
const plan = json(path.join(planDir,'plan.json'));
for (const key of ['binary','plain-binary','current-plain-binary','test-binary']) {
  fs.copyFileSync(args[key],path.join(out,key));
  fs.chmodSync(path.join(out,key),0o555);
}
const binaries = Object.fromEntries(['binary','plain-binary','current-plain-binary','test-binary'].map(key => {
  const file=path.join(out,key);
  return [key,{input:args[key],file,sha256:sha(fs.readFileSync(file))}];
}));
for(const key of ['binary','current-plain-binary','test-binary']) assert.equal(binaries[key].sha256,buildSource.binaries[key].sha256,`built ${key}`);
const historical=json(path.join(root,'verification/numeric-graph-v1/build-provenance.json')).builds.current;
assert.equal(binaries['plain-binary'].sha256,historical.sha256,'immutable historical output/snapshot comparison binary');
const commands = [], failures = [];
const tree = directory => {
  const result = {};
  const walk = (dir,rel='') => {
    for (const entry of fs.readdirSync(dir,{withFileTypes:true})) {
      const name = rel+entry.name;
      if (entry.isDirectory()) { result[name]='directory'; walk(path.join(dir,entry.name),name+'/'); }
      else result[name] = sha(fs.readFileSync(path.join(dir,entry.name)));
    }
  };
  walk(directory); return result;
};
const same = (a,b) => assert.deepEqual(a,b);
function parseObserved(bytes) {
  assert.ok(bytes.endsWith('\n'),'complete newline-delimited observation');
  const events = bytes.slice(0,-1).split('\n').map(JSON.parse);
  const allowed = new Set(['begin','end','child-launch','child-begin','child-end',
    'guest-request','protected-operation','nodes','format-result','dynamic-run-result']);
  const streams = new Map();
  for (const event of events) {
    assert.ok(allowed.has(event.event),`unknown observer event ${event.event}`);
    assert.ok(Number.isSafeInteger(event.pid) && event.pid > 0,'valid process identity');
    const stream = streams.get(event.pid) ?? [];
    assert.equal(event.sequence,stream.length,'contiguous process observation sequence');
    stream.push(event);streams.set(event.pid,stream);
    if(event.event==='nodes') assert.ok(Number.isSafeInteger(event.evaluated_nodes)&&event.evaluated_nodes>=0);
  }
  assert.equal(events[0]?.event,'begin','observer entered');
  assert.equal(events.at(-1)?.event,'end','observer exited');
  assert.equal(events.filter(e=>e.event==='begin').length,1);
  assert.equal(events.filter(e=>e.event==='end').length,1);
  for (const [pid,stream] of streams) {
    const parent = pid === events[0].pid;
    same(stream[0].event,parent?'begin':'child-begin');
    same(stream.at(-1).event,parent?'end':'child-end');
    same(stream.filter(e=>e.event===(parent?'begin':'child-begin')).length,1);
    same(stream.filter(e=>e.event===(parent?'end':'child-end')).length,1);
    same(stream.at(-1).guest_requests,stream.filter(e=>e.event==='guest-request').length);
    same(stream.at(-1).protected_operations,stream.filter(e=>e.event==='protected-operation').length);
  }
  const count = kind => events.filter(e=>e.event===kind).length;
  same(count('child-launch'),count('child-begin'));
  same(count('child-begin'),count('child-end'));
  return {guest_requests:count('guest-request'),protected_operations:count('protected-operation'),
    evaluated_nodes:events.filter(e=>e.event==='nodes').reduce((n,e)=>n+e.evaluated_nodes,0),events};
}
function observed(file) {
  const bytes = fs.readFileSync(file);
  return {...parseObserved(bytes.toString('utf8')),sha256:sha(bytes),file};
}
function run(id,argv,cwd,test=false,input=undefined) {
  const file = path.join(out,`${id}.events.jsonl`);
  const executable = path.join(out,test?'test-binary':'binary');
  const result = spawnSync(executable,argv,{cwd,env:{...process.env,NOBLE_SAFETY_OBSERVATION:file},input,timeout:180000,maxBuffer:16*1024*1024});
  fs.writeFileSync(path.join(out,`${id}.stdout`),result.stdout ?? '');
  fs.writeFileSync(path.join(out,`${id}.stderr`),result.stderr ?? '');
  assert.equal(result.signal,null,`${id}: ${result.error ?? result.signal}`);
  const observation = observed(file);
  if(argv[0]==='ngraph') same(observation.events.filter(e=>e.event==='nodes').length,1);
  const lines = (result.stdout?.toString().trim() || '').split('\n').filter(Boolean).map(line=>{
    try{return JSON.parse(line);}catch{return null;}
  }).filter(Boolean);
  const record = {id,argv,cwd,exit:result.status,stdout_sha256:sha(result.stdout),stderr_sha256:sha(result.stderr),lines,observation};
  commands.push(record);
  return {record,result};
}
function derive(step,cwd) {
  const file = name => path.join(cwd,name);
  if(step.derive==='mkdir') return step.directories.forEach(name=>fs.mkdirSync(file(name),{recursive:true}));
  if(step.derive==='write') return fs.writeFileSync(file(step.to),step.text,{flag:'wx'});
  if(step.derive==='oversize') {
    fs.writeFileSync(file(step.to),fs.readFileSync(file(step.from)),{flag:'wx'});
    fs.truncateSync(file(step.to),step.bytes); return;
  }
  const text = fs.readFileSync(file(step.from),'utf8');
  let edited;
  if(step.derive==='replace') {
    assert.equal(text.split(step.search).length,2); edited=text.replace(step.search,step.replacement);
  } else if(step.derive==='flip-param') {
    const match=/\nparam (\d+) 0x([0-9a-f]{8})\n/.exec(text); assert.ok(match);
    const bits=((parseInt(match[2],16)^1)>>>0).toString(16).padStart(8,'0');
    edited=text.replace(match[0],`\nparam ${match[1]} 0x${bits}\n`);
  } else if(step.derive==='forge') {
    assert.equal(forgeSnapshot(text,p=>p),text,'independent identity re-encoding control');
    edited=forgeSnapshot(text,params=>{
      if(step.edit==='swap-first-two-params') [params[0],params[1]]=[params[1],params[0]];
      else if(step.edit==='flip-first-param-bit') params[0][1]=(params[0][1]^1)>>>0;
      else throw Error(`unknown forge ${step.edit}`);
      return params;
    });
  } else throw Error(`unknown derivation ${step.derive}`);
  fs.writeFileSync(file(step.to),edited,{flag:'wx'});
}
const results = new Map();
put('observer-boundaries.json',{
  assessment:'INFERRED source inventory of existing semantic entry points; direct measurements are the separate real-command event streams',
  guest_requests:{
    'component/authorized_fs.rs':['fs.read import'],
    'component/bounded_region.rs':['region.read import','region.release import','interface region resource.drop','root region resource.drop'],
    'component/callback_owner.rs':['token.issue import','token.consume import','token resource.drop'],
    'core/runtime/engine-host.mjs':['test_emit','test_emit_bound','test_clock_bound','test_abort'],
    'core/runtime/engine.mjs':['liveGeneration','livePropose'],
    'core/runtime/slot-engine.mjs':['live_select','test_emit including frozen replay','test_abort including refusal']},
  protected_operations:{
    'component/authorized_fs.rs':['authorized native directory read entry'],
    'component/bounded_region.rs':['authorized bounded region copy entry'],
    'component/callback_owner.rs':['authorized token consume operation'],
    'core/runtime/engine-host.mjs':['allowed bound emit','allowed bound scripted clock'],
    'core/runtime/slot-registry.mjs':['authorized performEffect entry; replay does not perform an effect']},
  excluded:['compiler-managed VM bookkeeping imports','trusted CLI argument/input/output I/O',
    'host fallback owner cleanup (not a guest resource.drop callback)','syscall totals are not semantic counters'],
  entry_rule:'Guest semantic imports count before validation/checkpoint/replay branching; delegated registry paths do not double-count',
  evaluated_nodes:'Caller-owned observation of actual node materializations/instructions/guard validations, not Report fuel accounting'
});
try {
  for (const scenario of plan.scenarios) {
    const pair = ['observed','historical','current-default'].map(mode=>{
      const cwd=path.join(out,mode,scenario.id); fs.mkdirSync(cwd,{recursive:true});
      for(const name of scenario.stage) fs.copyFileSync(path.join(planDir,'inputs',name),path.join(cwd,name));
      return cwd;
    });
    for(const step of scenario.steps) {
      if(!step.argv) {pair.forEach(cwd=>derive(step,cwd));continue;}
      const id=`${scenario.id}-${step.id}`;
      const before=tree(pair[0]);
      const {record,result}=run(id,step.argv,pair[0]);
      for(const [index,key] of [[1,'plain-binary'],[2,'current-plain-binary']]) {
        const forbidden=path.join(out,`${id}.${key}.default-must-not-exist`);
        const plain=spawnSync(path.join(out,key),step.argv,{cwd:pair[index],env:{...process.env,NOBLE_SAFETY_OBSERVATION:forbidden},timeout:180000,maxBuffer:16*1024*1024});
        assert.equal(plain.signal,null,`${id}: default terminated`);
        assert.ok(!fs.existsSync(forbidden),'default binary must not read observer environment');
        same(result.status,plain.status);same(result.stdout,plain.stdout);same(result.stderr,plain.stderr);
        same(tree(pair[0]),tree(pair[index]));
      }
      record.default_bytes_identical=true;
      record.case_id=step.case_id; record.variant=step.variant; record.before=before;record.after=tree(pair[0]);
      results.set(`${scenario.id}/${step.id}`,record);
      if(['NGRAPH-CASE-03','NGRAPH-CASE-04','NGRAPH-CASE-05','NGRAPH-CASE-11','NGRAPH-CASE-12'].includes(step.case_id)) {
        same(record.observation.guest_requests,0);same(record.observation.protected_operations,0);
      }
      if(step.case_id==='NGRAPH-CASE-04'||step.case_id==='NGRAPH-CASE-12'&&['eval','verify'].includes(step.argv[1])) same(record.observation.evaluated_nodes,0);
    }
  }
  const get=(scenario,id)=>{const r=results.get(`${scenario}/${id}`);assert.ok(r,`${scenario}/${id}`);return r;};
  const line=r=>r.lines[0];
  const expect=(r,e)=>{
    for(const [key,want] of Object.entries(e)) {
      if(key==='exit') same(r.exit,want);
      else if(key==='usage_on_stderr') {
        assert.equal(want,true);
        same(r.exit,2);
        same(r.lines.some(row=>row.outcome==='normal'),false);
      }
      else same(line(r)?.[key] ?? null,want);
    }
  };
  const c=id=>cases.cases.find(c=>c.id===`NGRAPH-CASE-${id}`);
  const f1=get('check','check-f1'),i1=get('check','check-i1'),extract=get('check','extract-i1');
  for(const r of [f1,i1,extract]) expect(r,{exit:0,outcome:'normal',schema:c('03').expected.schema});
  for(const [key,value] of Object.entries(c('03').expected.F1)) same(line(f1)[key],value);
  same(line(i1).graph,line(extract).graph);
  for(const key of ['inputs','outputs','live_nodes','layers']) same(line(i1)[key],c('03').expected.I1[key]);
  for(const r of commands.filter(r=>r.case_id==='NGRAPH-CASE-04')) expect(r,c('04').expected[r.variant]);
  const extractIds={'test.emit':'test-emit','if':'if','program output':'program-output','frontend refusal':'frontend-refusal','f32 profile':'f32-profile','expansion':'expansion','existing out':'existing-out'};
  for(const name of ['I1','I2','I3','I4']) {
    const r=get('extract',`extract-${name}`),checked=get('extract',`check-${name}`);
    expect(r,{exit:0,outcome:'normal'});expect(checked,{exit:0,outcome:'normal'});same(line(r).graph,line(checked).graph);
  }
  for(const [variant,id] of Object.entries(extractIds)) {
    const r=get('extract',id),expected={...c('05').expected[variant]};delete expected.existing_bytes_unchanged;
    expect(r,expected);same(r.before,r.after);
  }
  same(get('extract','existing-out').after['existing.ngraph'],sha('existing bytes\n'));
  const training = plan.scenarios.find(s=>s.id==='train-refusals').steps.filter(s=>s.argv).slice(1);
  const expectedTraining = Object.keys(c('11').expected).filter(k=>!['stage','outcome','guest_requests','protected_operations','refusals_create_no_output_or_partial_file'].includes(k));
  same(training.length,expectedTraining.length);
  training.forEach((step,index)=>{
    const key=expectedTraining[index],r=get('train-refusals',step.id),expected={...c('11').expected[key]};
    delete expected.opt_in_recorded_in_snapshot;expect(r,expected);
    if(key==='with opt-in') assert.match(fs.readFileSync(path.join(r.cwd,'opt-in.snapshot'),'utf8'),/^opt-in %4 boolean straight-through$/m);
    else same(r.before,r.after);
  });
  const bindingKeys=['param byte','provenance field','other graph','layout','changed facts','pinned id','schema'];
  const bindings=plan.scenarios.find(s=>s.id==='b5').steps.filter(s=>s.argv&&s.case_id==='NGRAPH-CASE-12'&&['eval','verify'].includes(s.argv[1]));
  same(bindings.length,14);
  bindings.forEach((step,index)=>{
    const r=get('b5',step.id); expect(r,{exit:2,code:c('12').expected[bindingKeys[Math.floor(index/2)]]});
    same(r.lines.length,1);same(r.lines.filter(l=>l.vector!==undefined).length,0);same(r.before,r.after);
  });
  const library=run('format-values',['safety_observe::tests::registered_format_values','--exact','--test-threads=1'],out,true).record;
  same(library.exit,0);same(library.observation.guest_requests,0);same(library.observation.protected_operations,0);
  const formats=library.observation.events.filter(e=>e.event==='format-result');
  same(formats.map(e=>e.variant),c('09').input.files);
  for(const row of formats) {
    const want=c('09').expected[row.variant];
    same(row.observed,want==='accepted as that encoding'?'0x7fc00001':want==='accepted as 0x80000000'?'0x80000000':want);
    row.input_sha256=sha(row.input);
  }
  const dynamic=run('dynamic-run',['ngraph::registered_dynamic_run_control','--exact','--test-threads=1'],out,true).record;
  same(dynamic.exit,0);same(dynamic.observation.guest_requests,0);same(dynamic.observation.protected_operations,0);
  same(dynamic.observation.events.filter(e=>e.event==='dynamic-run-result').map(e=>e.observed),['E-DYNAMIC-RUN']);
  const positive=path.join(out,'positive');fs.mkdirSync(positive);
  fs.writeFileSync(path.join(positive,'source.noble'),'"observed" test.emit');
  const host=run('positive-js',['run','source.noble'],positive).record;
  same(host.exit,0);same(host.observation.guest_requests,1);same(host.observation.protected_operations,0);
  assert.ok(host.observation.events.some(e=>e.event==='child-begin'));
  fs.writeFileSync(path.join(positive,'clock.bindings'),
    'bind clock@1 test.clock version-1 -- I64 ! test.clock allow script 42\n');
  const submissions=['module clock@1 [ require now -- I64 ! test.clock export tick def tick [ now ] ]',
    'import clock@1 as c','c.tick'];
  const framed=Buffer.concat(submissions.flatMap(s=>[Buffer.from(`${Buffer.byteLength(s)}\n`),Buffer.from(s)]));
  const clock=run('positive-js-protected',['session','--framed','--declared-modules','--bindings','clock.bindings'],positive,false,framed).record;
  same(clock.exit,0);same(clock.observation.guest_requests,1);same(clock.observation.protected_operations,1);
  same(clock.lines.at(-1).stack.map(v=>[v.type,v.value]),[['I64','42']]);
  same(clock.lines.at(-1).guest_requests,1);same(clock.lines.at(-1).protected_operations,1);
  fs.writeFileSync(path.join(positive,'region.noble'),'region.read swap regions.release\n');
  const compiledRegion=run('compile-region',['component','compile',path.join(root,'crates/noble-wasm/wit/bounded-region.wit'),
    'bounded','region-component','read-region=region.noble'],positive).record;
  same(compiledRegion.exit,0);same(compiledRegion.lines[0].outcome,'compiled');
  const region=run('positive-rust-protected',['component','read-region','region-component/component.wasm','000102','1','2'],positive).record;
  same(region.exit,0);same(region.lines[0].bytes_hex,'0102');same(region.lines[0].protected_operations,1);
  same(region.observation.guest_requests,2);same(region.observation.protected_operations,1);
  const deniedRegion=run('negative-rust-protected',['component','read-region','region-component/component.wasm','000102','3','1'],positive).record;
  same(deniedRegion.lines[0].outcome,'bounds-reject');same(deniedRegion.observation.guest_requests,2);same(deniedRegion.observation.protected_operations,0);
  const destructorSource=`(component
  (import "region" (type $region (sub resource)))
  (core func $drop (canon resource.drop $region))
  (core module $guest
    (import "host" "drop" (func $drop (param i32)))
    (memory (export "memory") 1)
    (func (export "read-region") (param i32 i64 i64) (result i32)
      local.get 0 call $drop i32.const 0))
  (core instance $guest (instantiate $guest
    (with "host" (instance (export "drop" (func $drop))))))
  (func (export "read-region")
    (param "value" (own $region)) (param "offset" s64) (param "length" s64)
    (result (result (list u8) (error string)))
    (canon lift (core func $guest "read-region") (memory $guest "memory"))))
`;
  fs.writeFileSync(path.join(positive,'destructor.wat'),destructorSource,{flag:'wx'});
  const assembler=json(path.join(root,'policy/tool-selection.json')).tool_paths.wasm_tools.output+'/bin/wasm-tools';
  const assembleArgs=['parse','destructor.wat','-o','destructor.wasm'];
  const assembled=spawnSync(assembler,assembleArgs,{cwd:positive,encoding:'utf8',timeout:30000});
  same(assembled.status,0);
  put('destructor-assembly.json',{binary:assembler,binary_sha256:sha(fs.readFileSync(assembler)),argv:assembleArgs,
    exit:assembled.status,stdout:assembled.stdout,stderr:assembled.stderr,source_sha256:sha(destructorSource),
    wasm_sha256:sha(fs.readFileSync(path.join(positive,'destructor.wasm')))});
  const destructor=run('positive-rust-destructor',['component','read-region','destructor.wasm','000102','0','1'],positive).record;
  same(destructor.lines[0].outcome,'read');same(destructor.lines[0].bytes_hex,'');
  same(destructor.lines[0].region_reads,0);same(destructor.lines[0].released_owners,1);same(destructor.lines[0].live_owners,0);
  same(destructor.observation.guest_requests,1);same(destructor.observation.protected_operations,0);
  for(const [index,mode] of ['pure','effects','effects','effects'].entries()) {
    const directory=path.join(positive,`slot-${index}-${mode}`);fs.mkdirSync(directory);
    const slot=await slotControls(path.join(out,'binary'),path.join(root,'crates/noble-cli/tests/live_slot_quota_replay_authority.json'),directory,mode);
    const observation=observed(slot.events);
    for(const phase of slot.phases) {
      const expected={'pure-dispatch':[1,0],'live-dispatch-effects':[3,2],'frozen-replay':[3,0]}[phase.name];
      if(expected) same([phase.observation.guest_requests,phase.observation.protected_operations],expected);
      else {
        same(phase.name,'refused-emit');assert.ok(phase.report.guest_requests>=2);
        same(phase.observation.guest_requests,phase.report.guest_requests);same(phase.observation.protected_operations,0);
      }
    }
    commands.push({id:`positive-slot-${index}-${mode}`,argv:['live','slot','--engine','v8'],cwd:directory,exit:slot.exit,
      stdout_sha256:sha(fs.readFileSync(slot.stdout)),stderr_sha256:sha(fs.readFileSync(slot.stderr)),observation,
      phases:slot.phases,artifacts:tree(directory)});
  }
  const killedDirectory=path.join(positive,'slot-hard-termination');fs.mkdirSync(killedDirectory);
  const killed=await slotControls(path.join(out,'binary'),path.join(root,'crates/noble-cli/tests/live_slot_quota_replay_authority.json'),killedDirectory,'hard-kill');
  const killedEvents=fs.readFileSync(killed.events,'utf8').trimEnd().split('\n').map(JSON.parse);
  same(killedEvents.filter(row=>row.event==='begin').length,1);
  same(killedEvents.filter(row=>row.event==='child-begin').length,1);
  same(killedEvents.filter(row=>row.event==='end').length,1);
  same(killedEvents.filter(row=>row.event==='child-end').length,0);
  assert.throws(()=>observed(killed.events),undefined,'actual killed worker without a terminal transition must not supply complete evidence');
  put('hard-termination-control.json',{...killed,result:'rejected-incomplete-observation',artifacts:tree(killedDirectory)});
  const hostile=[];
  const original=fs.readFileSync(clock.observation.file,'utf8');
  for(const event of ['begin','end','child-launch','child-begin','child-end','guest-request','protected-operation']) {
    const rows=original.trimEnd().split('\n');
    const index=rows.findIndex(row=>JSON.parse(row).event===event);assert.ok(index>=0);
    rows.splice(index,1);
    assert.throws(()=>parseObserved(rows.join('\n')+'\n'),undefined,`missing ${event} must fail closed`);
    hostile.push({mutation:`drop ${event}`,result:'rejected'});
  }
  assert.throws(()=>parseObserved(original.slice(0,-1)));hostile.push({mutation:'truncated terminal newline',result:'rejected'});
  assert.throws(()=>parseObserved(original.replace('guest-request','unknown-operation')));hostile.push({mutation:'unknown event',result:'rejected'});
  put('observer-hostile-controls.json',hostile);
  for(const id of ['eval-probes/eval-i1','eval-probes/eval-i1-reference','train-refusals/with-opt-in']) {
    const r=results.get(id);assert.ok(r&&r.observation.evaluated_nodes>0,`${id}: actual nonzero node control`);
  }
  put('field-observations.json',{format_values:formats,case02:{check_requests:[f1,i1].map(r=>({id:r.id,observation:r.observation})),
    capability_closure:'OPEN: no compiler-authenticated complete closed-world closure; injected callbacks remain effect-capable'},
    non_claims:['No case promotion','No whole-feature/global-gate claim','No capability waiver','No historical packet rewrite']});
} catch(error) { failures.push(error.stack ?? String(error)); }
assert.deepEqual(sources(),beforeSources,'source bytes and modes remain frozen');
for (const [key,binary] of Object.entries(binaries))
  assert.equal(sha(fs.readFileSync(binary.file)),binary.sha256,`${key}: executed bytes remain frozen`);
put('commands.json',commands);
put('receipt.json',{schema:'noble-ngraph-safety-observations/v1',result:failures.length?'failed':'measured-with-open-capability-prerequisite',
  failures,binaries,runner_sha256:beforeSources['verification/numeric-graph-v1/safety.mjs'].sha256,
  slot_helper_sha256:beforeSources['verification/numeric-graph-v1/safety-slot.mjs'].sha256,
  cases_sha256:sha(fs.readFileSync(path.join(out,'cases.json'))),
  source_manifest_sha256:sha(fs.readFileSync(path.join(out,'source-manifest.json'))),commands_sha256:sha(fs.readFileSync(path.join(out,'commands.json'))),
  build_source_sha256:sha(fs.readFileSync(path.join(out,'build-source.json'))),
  observer_boundaries_sha256:sha(fs.readFileSync(path.join(out,'observer-boundaries.json'))),
  observation_configuration:{feature:'safety-observe',transport:'owner-private append-only JSONL with per-process sequence and terminal totals',
    evaluated_node_semantics:'actual materializations/instructions/guard validations; cached constant reuse excluded; not Report::evaluated_nodes fuel accounting'},
  acceptance_claim:false,capability_closure:'open'});
console.log(JSON.stringify({out,commands:commands.length,failures}));
process.exitCode=failures.length?1:0;
