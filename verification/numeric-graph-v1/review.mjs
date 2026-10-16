#!/usr/bin/env node
// Independent Numeric-Graph-v1 claims receipt binding. This exercises --help,
// rechecks immutable raw observations, and binds the separate review reasoning.
// It does not infer a proof, promote cases, rerun acceptance, or collect Octet.
// NODE review.mjs ACCEPTANCE_JSON RUNTIME_JSON BUILD_PROVENANCE_JSON PREPROMOTION_CASES_JSON NEW_EXTERNAL_DIR
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const [acceptanceFile, runtimeFile, provenanceFile, prepromotionFile, output] = process.argv.slice(2);
assert.equal(process.argv.length, 7, 'usage: SELECTED_NODE review.mjs ACCEPTANCE RUNTIME BUILD_PROVENANCE PREPROMOTION NEW_EXTERNAL_DIR');
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const bytes = file => fs.readFileSync(file);
const hash = file => sha(bytes(file));
const load = file => JSON.parse(bytes(file));
const stable = value => Array.isArray(value) ? value.map(stable) : value && typeof value === 'object'
  ? Object.fromEntries(Object.keys(value).sort().map(key => [key, stable(value[key])])) : value;
const canonical = value => JSON.stringify(stable(value));
const reviewFile = path.join(root, 'verification/numeric-graph-v1/review.json');
const review = load(reviewFile), acceptance = load(acceptanceFile), runtime = load(runtimeFile);
const provenance = load(provenanceFile), prepromotion = load(prepromotionFile);
const deltaFile=path.join(root,'verification/numeric-graph-v1/gate-deltas.json');
const deltas=load(deltaFile);
assert.equal(deltas.tested_commit,review.tested_commit);
for(const row of Object.values(deltas.raw.final_logs)) assert.equal(hash(row.path),row.sha256);
for(const row of Object.values(deltas.capacity_renewal.raw)) assert.equal(hash(row.path),row.sha256);
const historicalWarnings=deltas.tigerstyle_static_lint.prior_confirmation;
assert.equal(hash(historicalWarnings.raw_findings.path),historicalWarnings.raw_findings.sha256);
const prefix='noble-inventory-artifacts> ';
const extract=file=>{
 const text=bytes(file).toString().split('\n').filter(x=>x.startsWith(prefix)).map(x=>x.slice(prefix.length)).join('\n');
 return JSON.parse(text.slice(0,text.indexOf('\n}\n')+2));
};
const gateObservation=extract(deltas.raw.final_logs['source-coverage'].path);
assert.equal(gateObservation.architecture_findings.length,deltas.architecture_findings.total);
assert.equal(gateObservation.lint_findings.length,deltas.tigerstyle_static_lint.total);
assert.deepEqual(gateObservation.architecture_findings,extract(deltas.raw.final_logs['native-assurance'].path).architecture_findings);
const selection = load(path.join(root,'policy/tool-selection.json'));
assert.equal(fs.realpathSync(process.execPath), fs.realpathSync(selection.tool_paths.node.output+'/bin/node'));
assert.deepEqual(process.execArgv, []);
assert.equal(review.selection, 'independent-source-and-execution-claims-review');
assert.equal(hash(acceptanceFile), review.acceptance.sha256);
assert.equal(hash(runtimeFile), review.runtime.sha256);
assert.equal(hash(provenanceFile), review.build_provenance.sha256);
assert.equal(hash(prepromotionFile), review.prepromotion_cases.sha256);
assert.equal(acceptance.result, 'passed');
assert.deepEqual(acceptance.failures, []);
assert.deepEqual(acceptance.integrity_failures, []);
assert.equal(acceptance.source.frozen, true);
assert.equal(acceptance.source.built_sources_equal_tested_commit, true);
assert.equal(acceptance.source_revision, review.acceptance.source_revision);
assert.equal(acceptance.tested_commit, review.tested_commit);
assert.equal(acceptance.base_commit, review.base_commit);
assert.equal(provenance.actual_release_build_revision, review.reviewed_publication);
assert.equal(provenance.source_equivalence.manifests_byte_identical, true);
for(const [side,tool] of [['current',acceptance.tools.noble],['base',acceptance.tools.noble_base]]) {
 const built=provenance.builds[side];
 assert.equal(built.sha256,tool.sha256);
 assert.equal(hash(built.file),built.sha256);
 assert.equal(hash(built.log),built.log_sha256);
 assert.notEqual(built.sha256,built.log_sha256,'binary identity conflated with build log');
 assert.equal(built.exit,0);
}
for(const [name,row] of Object.entries(review.reviewed_sources)) {
 const actual=hash(path.join(root,name));
 assert([row.sha256,row.schema_only_sha256].includes(actual),`reviewed source changed: ${name}`);
 if(actual===row.sha256)assert.equal(fs.statSync(path.join(root,name)).size,row.bytes);
}
const raw=acceptance.external_raw_output.directory;
const manifestFile=path.join(raw,acceptance.external_raw_output.manifest);
assert.equal(hash(manifestFile),acceptance.external_raw_output.manifest_sha256);
const manifest=load(manifestFile);
let checked=0;
for(const [name,row] of Object.entries(manifest)) {
 assert(!path.isAbsolute(name) && !name.split('/').includes('..'),'unsafe raw path');
 const file=path.join(raw,name);assert.equal(hash(file),row.sha256,`raw bytes changed: ${name}`);
 assert.equal(fs.statSync(file).size,row.bytes);checked++;
}
assert.equal(hash(path.join(raw,'acceptance.json')),hash(acceptanceFile));
const plan=load(path.join(raw,acceptance.plan.file));
const lookup=(object,keys)=>keys.reduce((value,key)=>value?.[key],object);
for(const item of [...review.case01.fields,...review.case15.fields]) {
 const id=review.case01.fields.includes(item)?'NGRAPH-CASE-01':'NGRAPH-CASE-15';
 const row=plan.cases.workload.find(x=>x.id===id);
 assert.deepEqual(lookup(row.expected,item.path),item.expected,`${id} reviewed expected field drift`);
 assert.equal(item.verdict,'passed');
}
// Independently recompare each actual stdout/stderr/exit pair, not only the
// acceptance runner's summary result. Publication-byte equality is separately
// bound through the immutable manifest and runner's per-step tree comparison.
const pairs=new Map();
for(const c of acceptance.commands.filter(x=>x.repetition)) {
 assert.equal(c.executable_sha256,review.acceptance.binary_sha256);
 assert.equal(hash(path.join(raw,c.stdout)),c.stdout_sha256);
 assert.equal(hash(path.join(raw,c.stderr)),c.stderr_sha256);
 assert.equal(c.signal,null); assert.equal(c.error,null);
 const key=JSON.stringify([c.scenario,c.step]);
 const pair=pairs.get(key)??{}; assert(!pair[c.repetition]);pair[c.repetition]=c;pairs.set(key,pair);
}
for(const [key,pair] of pairs) {
 assert(pair[1]&&pair[2],`missing repetition: ${key}`);
 for(const field of ['stdout_sha256','stderr_sha256','exit'])assert.equal(pair[1][field],pair[2][field],`${key} ${field}`);
}
assert.equal(pairs.size,100);
assert.equal(acceptance.determinism.result,'passed');assert.deepEqual(acceptance.determinism.mismatches,[]);
assert.equal(runtime.result,'passed');assert.deepEqual(runtime.failures,[]);
let runtimeArtifacts=0;
for(const side of ['base','tested']) {
 const data=runtime.sides[side],dir=path.join(path.dirname(runtimeFile),side);
 assert.equal(hash(data.binary.path),data.binary.sha256);
 for(const [name,row] of Object.entries(data.artifacts)) {assert.equal(hash(path.join(dir,name)),row.sha256);runtimeArtifacts++;}
}
for(const [id,count] of [['CORE-03',6],['CORE-05',4],['CORE-09',2]]) {
 assert.deepEqual(runtime.sides.tested.observations[id],runtime.sides.base.observations[id]);
 assert.equal(runtime.sides.tested.observations[id].length,count);
}
assert.equal(runtime.compiled_artifact_comparison.result,'passed');
// Source selection review supplements the runner's narrow built projection.
const git='/run/current-system/sw/bin/git';
const diff=spawnSync(git,['-C',root,'diff','--no-ext-diff','--name-status',review.base_commit,review.reviewed_publication,'--',...review.case01.source_isolation.paths],{encoding:'utf8'});
assert.equal(diff.status,0);assert.equal(diff.stdout.trim(),'');
const builtDiff=spawnSync(git,['-C',root,'diff','--no-ext-diff','--name-only',review.tested_commit,review.reviewed_publication,'--',...provenance.source_equivalence.scope],{encoding:'utf8'});
assert.equal(builtDiff.status,0);assert.equal(builtDiff.stdout.trim(),'');
const hold=['02','03','04','05','09','11','12'].map(x=>'NGRAPH-CASE-'+x);
const pass=['01','06','07','08','10','13','14','15'].map(x=>'NGRAPH-CASE-'+x);
assert.deepEqual(review.final_recommendation.passed,pass);
assert.deepEqual(review.final_recommendation.held,hold);
assert.deepEqual(review.ng_po_discharged,[]);
assert(Object.values(review.scope).every(x=>x===false));
// Each prepromotion record must remain an unexecuted design. The separate
// record hash map, not the entire-file hash, is future evidence's case digest.
const recordRows=prepromotion.cases??prepromotion.records;
assert(recordRows,'missing prepromotion records');
const records=Array.isArray(recordRows)?recordRows:Object.values(recordRows);
assert.equal(records.length,15);
for(const entry of records) {
 const row=entry.record??entry.case??entry;
 assert(row.id,'prepromotion record missing id');
 assert.equal(sha(Buffer.from(canonical(row))),entry.prepromotion_case_sha256,`${row.id} record hash differs`);
 assert.deepEqual(row.state,{implementation:'absent',execution:'not-run',proof:'open',trust:'unassessed'});
 assert.deepEqual(row.evidence,[]);
}
assert(!fs.existsSync(output));fs.mkdirSync(output);
const binary=path.join(raw,acceptance.tools.noble.frozen);
assert.equal(hash(binary),review.acceptance.binary_sha256);
const help=spawnSync(binary,['--help'],{cwd:root,encoding:null,timeout:10000,env:{PATH:'/nonexistent',LANG:'C',LC_ALL:'C',TMPDIR:path.resolve(output)}});
assert.equal(help.status,0);assert.equal(help.signal,null);assert.equal(help.error,undefined);
fs.writeFileSync(path.join(output,'help.stdout'),help.stdout);fs.writeFileSync(path.join(output,'help.stderr'),help.stderr);
const expectedHelp=acceptance.commands.find(x=>x.id===acceptance.help.command);
assert.equal(sha(help.stdout),expectedHelp.stdout_sha256);assert.equal(sha(help.stderr),expectedHelp.stderr_sha256);
const reportFields=new Set();
for(const c of acceptance.commands.filter(x=>x.repetition===1)) {
 for(const line of bytes(path.join(raw,c.stdout)).toString().split('\n').filter(Boolean)) {
  const value=JSON.parse(line); assert.equal(value.schema,'noble-ngraph-report/v1');
  for(const field of Object.keys(value)) {assert(review.report_field_review.fields.includes(field),`unreviewed report field: ${field}`);reportFields.add(field);}
 }
}
const receipt={schema:'noble-numeric-graph-v1-review-receipt/v1',result:'passed',selection:review.selection,
 reviewed_publication:review.reviewed_publication,tested_commit:review.tested_commit,source_revision:acceptance.source_revision,
 review:{file:'verification/numeric-graph-v1/review.json',sha256:hash(reviewFile)},runner:{file:'verification/numeric-graph-v1/review.mjs',sha256:hash(fileURLToPath(import.meta.url))},
 acceptance:{file:acceptanceFile,sha256:hash(acceptanceFile),manifest_sha256:hash(manifestFile),checked_raw_files:checked},
 build_provenance:{file:provenanceFile,sha256:hash(provenanceFile)},prepromotion_cases:{file:prepromotionFile,sha256:hash(prepromotionFile)},
 runtime:{file:runtimeFile,sha256:hash(runtimeFile),checked_artifact_files:runtimeArtifacts,extra_runtime_inputs_per_binary:12},
 gate_deltas:{file:'verification/numeric-graph-v1/gate-deltas.json',sha256:hash(deltaFile),architecture_findings:gateObservation.architecture_findings.length,warning_findings:gateObservation.lint_findings.length},
 tools:{node:{path:process.execPath,sha256:hash(process.execPath)},binary:{file:binary,sha256:hash(binary)}},
 observed:{help:{stdout_sha256:sha(help.stdout),stderr_sha256:sha(help.stderr),exit:help.status},repeated_command_pairs:pairs.size,report_fields:[...reportFields].sort()},
 verdicts:{case01:review.case01.verdict,case15:review.case15.verdict,passed:pass,held:hold},
 external_output:path.resolve(output),failures:[],ng_po_discharged:[],non_claims:review.non_claims};
fs.writeFileSync(path.join(output,'review-receipt.json'),JSON.stringify(receipt,null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify(receipt,null,2));
