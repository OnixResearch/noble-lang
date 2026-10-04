import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

export const root = fs.realpathSync(path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..'));
export const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
export const read = file => fs.readFileSync(path.join(root, file));
export const caseFiles = {
  'DX-01': 'specs/conformance/developer-experience-cases.json',
  'DX-02': 'specs/conformance/developer-experience-cases.json',
  'DX-06': 'specs/conformance/developer-experience-cases.json',
  'ADAPT-01': 'specs/conformance/adaptation-cases.json',
  'ADAPT-06': 'specs/conformance/adaptation-cases.json',
  'S-CASE-01': 'specs/conformance/safety-cases.json',
  'S-CASE-03': 'specs/conformance/safety-cases.json',
  'S-CASE-05': 'specs/conformance/safety-cases.json',
  'S-CASE-14': 'specs/conformance/safety-cases.json',
};
export const selectedCases = ['DX-01','ADAPT-01','S-CASE-01','S-CASE-03','S-CASE-05','S-CASE-14'];
export const historicalReceipts = {
  'verification/dx06/acceptance.json': '650e38c8e38f1f3a753a4e7e8b92f5e15175d13eef35bccb5cff65f8d78fe7b5',
  'verification/current-source-replay/acceptance.json': '20c407e546759402c804824c9b40037a8163e93e8cd22b416c10a56cb8330b1a',
  'verification/dx01/current-frozen.json': '50a4eed7fa6883b02ca1134dbdaf74ff673e7001df62daf0ef1894d6802e1f73',
  'verification/adapt-01/current-frozen.json': 'fd650ec7451bbf54ed27c90606798e91c4f2837849f8835fccd9d44365749e32',
  'verification/safety-core/acceptance.json': '733b561741cd71790a8bc0f2231f91643155e424440844966c8e342f19d39289',
  'verification/safety-core/handle-acceptance.json': '8c15f03fb0d3724c5b7ee9e8960b6f6336d9ca77d3e21a780b064ed2481f4aa5',
};

export function historic() {
  return Object.fromEntries(Object.entries(historicalReceipts).map(([file,digest]) => {
    const content = read(file);
    assert.equal(sha(content),digest,`immutable historical receipt changed: ${file}`);
    const receipt = JSON.parse(content);
    if(file.endsWith('/current-frozen.json')) {
      assert.ok(['DX-01','ADAPT-01'].includes(receipt.case_id),`${file} historical case`);
      assert.match(receipt.source_revision,/^sha256:[0-9a-f]{64}$/);
    } else assert.equal(receipt.result,'passed',`historical receipt failed: ${file}`);
    return [file,receipt];
  }));
}

export function packet(file) { return JSON.parse(read(file)); }
export function caseRow(id) {
  const file = caseFiles[id];
  assert.ok(file,`unselected case: ${id}`);
  const rows = packet(file).cases.filter(row=>row.id===id);
  assert.equal(rows.length,1,`${id} unique case`);
  return rows[0];
}

// The workers select their own receipt names. Resolve only a canonical case's
// exact evidence path beneath verification, and bind its bytes and source hash.
export function acceptedReceipt(id) {
  const row=caseRow(id), file=caseFiles[id];
  assert.equal(row.state.implementation,'implemented',`${id} implementation`);
  assert.equal(row.state.execution,'passed',`${id} execution`);
  assert.equal(row.state.proof,'open',`${id} proof`);
  assert.equal(row.state.trust,'explicit',`${id} trust`);
  assert.equal(row.evidence.length,1,`${id} one accepted worker receipt`);
  const evidence=row.evidence[0];
  assert.equal(evidence.kind,'test');
  assert.equal(evidence.result,'passed');
  assert.equal(evidence.subject,id);
  const relative=evidence.configuration?.receipt;
  assert.ok(typeof relative==='string' && relative.startsWith('../../verification/'),
    `${id} receipt must resolve from the canonical case under verification`);
  const absolute=path.resolve(root,path.dirname(file),relative);
  assert.ok(absolute.startsWith(`${root}/verification/`),`${id} receipt outside verification`);
  assert.equal(fs.realpathSync(absolute),absolute,`${id} receipt symlink`);
  const receiptFile=path.relative(root,absolute).split(path.sep).join('/');
  const content=read(receiptFile);
  assert.equal(sha(content),evidence.configuration.receipt_sha256,`${id} receipt SHA`);
  const receipt=JSON.parse(content);
  assert.equal(receipt.result,'passed',`${id} receipt result`);
  assert.equal(receipt.kind,'test',`${id} execution receipt`);
  assert.equal(receipt.schema,id==='DX-02'?'noble-editor-hole-acceptance/v1':
    'noble-adapt06-receipt/v1',`${id} exact receipt schema`);
  assert.equal(receipt.case_id??receipt.case?.id,id,`${id} exact receipt subject`);
  assert.ok(receipt.case,`${id} complete receipt case design`);
  assert.deepEqual({id:row.id,input:row.input,expected:row.expected},
    {id:receipt.case.id,input:receipt.case.input,expected:receipt.case.expected},
    `${id} canonical case design`);
  if('failures' in receipt) assert.equal(receipt.failures,0,`${id} failures`);
  if('integrity_failures' in receipt) assert.equal(receipt.integrity_failures,0,
    `${id} integrity failures`);
  assert.ok(typeof receipt.external_raw_output==='string' &&
    path.isAbsolute(receipt.external_raw_output),`${id} external raw output`);
  assert.equal(
    sha(fs.readFileSync(path.join(receipt.external_raw_output,
      id==='DX-02'?'smoke.json':'acceptance.json'))),
    sha(content),`${id} external receipt differs from copied bytes`);
  assert.match(receipt.source_revision,/^sha256:[0-9a-f]{64}$/,`${id} source revision`);
  assert.equal(evidence.source_revision,receipt.source_revision,`${id} source revision`);
  return {row,evidence,receipt,receiptFile,receiptSha256:sha(content)};
}

export const sourceTrees=['crates/noble-kernel/src','crates/noble-contracts/src',
  'crates/noble-wasm/src','crates/noble-wasm/runtime','crates/noble-wasm/wit',
  'crates/noble-cli/src','verification/mc2/contracts',
  '.cairn/changes/editor-hole-transport'];
export const buildFiles=['Cargo.toml','Cargo.lock','rust-toolchain.toml',
  'policy/tool-selection.json','policy/boundary-controls.ncl',
  'policy/boundary-controls.json','README.md',
  'crates/noble-kernel/Cargo.toml','crates/noble-contracts/Cargo.toml',
  'crates/noble-wasm/Cargo.toml','crates/noble-cli/Cargo.toml',
  'crates/noble-syndicate/Cargo.toml',
  'proofs/mc1/NobleContracts.lean','proofs/mc1/IntrinsicTypeWitness.lean',
  ...['Model','Rules','Expression','Obligation','Examples','Composition',
    'NamedSubject','NamedV2'].map(name=>`proofs/mc1/NobleContracts/${name}.lean`)];
export function dx06InputFiles(editor,forged) {
  return [...buildFiles,...new Set(Object.values(caseFiles)),
    editor.receiptFile,forged.receiptFile,...Object.keys(historicalReceipts),
    '.cairn/specs/developer-experience/spec.md','.cairn/specs/safety/spec.md',
    'crates/noble-contracts/tests/editor.rs','crates/noble-cli/tests/editor.rs',
    'crates/noble-wasm/tests/forged_effects.rs',
    'verification/dx02/gate.mjs','verification/adapt06/gate.mjs',
    'verification/adapt06/check.rs',
    'verification/dx06/gate.mjs','verification/dx06/postpromotion.mjs',
    'verification/current-source-replay/projection.mjs',
    'verification/current-source-replay/acceptance.json',
    'verification/dx06-current-source/source.mjs',
    'verification/dx06-current-source/gate.mjs',
    'verification/dx06-current-source/source.test.mjs',
    'verification/dx06-current-source/postpromotion.mjs',
    'verification/current-source-replay-dx02/gate.mjs',
    'verification/current-source-replay-dx02/projection.mjs',
    'verification/current-source-replay-dx02/projection.test.mjs',
    'verification/current-source-replay-dx02/postpromotion.mjs'];
}
export function replayInputFiles(editor,forged) {
  return [...buildFiles,...new Set(selectedCases.map(id=>caseFiles[id])),
    editor.receiptFile,forged.receiptFile,
    'verification/dx06-current-source/acceptance.json',
    ...Object.keys(historicalReceipts),
    '.cairn/specs/developer-experience/spec.md',
    '.cairn/specs/core-bootstrap/spec.md','.cairn/specs/safety/spec.md',
    'crates/noble-contracts/tests/source.rs',
    'crates/noble-contracts/tests/editor.rs',
    'crates/noble-cli/tests/smoke.rs','crates/noble-cli/tests/editor.rs',
    'crates/noble-wasm/tests/forged_effects.rs',
    'verification/dx02/gate.mjs','verification/adapt06/gate.mjs',
    'verification/adapt06/check.rs',
    'verification/dx06/gate.mjs','verification/dx06/postpromotion.mjs',
    'verification/dx06-current-source/source.mjs',
    'verification/dx06-current-source/gate.mjs',
    'verification/dx06-current-source/source.test.mjs',
    'verification/dx06-current-source/postpromotion.mjs',
    'verification/safety-core/check.rs',
    'verification/safety-core/handle-check.rs',
    'verification/current-source-replay/diagnostic-check.rs',
    'verification/current-source-replay/projection.mjs',
    'verification/current-source-replay/postpromotion.mjs',
    'verification/current-source-replay-dx02/projection.mjs',
    'verification/current-source-replay-dx02/projection.test.mjs',
    'verification/current-source-replay-dx02/postpromotion.mjs',
    'verification/current-source-replay-dx02/gate.mjs'];
}

export function sourceInventory(files,trees=sourceTrees) {
  const entries={};
  function add(file) {
    assert.ok(!path.isAbsolute(file) && !file.split('/').includes('..'),`unsafe source: ${file}`);
    const full=path.join(root,file);
    const stat=fs.lstatSync(full);
    assert.ok(stat.isFile() && !stat.isSymbolicLink(),`unbound source file: ${file}`);
    entries[file]=sha(fs.readFileSync(full));
  }
  function walk(folder) {
    for(const entry of fs.readdirSync(path.join(root,folder),{withFileTypes:true})
      .sort((a,b)=>a.name.localeCompare(b.name))) {
      const file=`${folder}/${entry.name}`;
      assert.ok(!entry.isSymbolicLink(),`unbound source symlink: ${file}`);
      if(entry.isDirectory()) walk(file);
      else if(entry.isFile()) add(file);
      else throw Error(`unbound source type: ${file}`);
    }
  }
  for(const tree of trees) walk(tree);
  for(const file of files) add(file);
  return entries;
}
export const revision = sources => `sha256:${sha(JSON.stringify(Object.entries(sources).sort()))}`;
export function frozen(sources) {
  for(const [file,digest] of Object.entries(sources))
    assert.equal(sha(read(file)),digest,`source changed during source-bound execution: ${file}`);
}

// Preserve every byte of all earlier evidence. JSON.parse is used only to
// establish the expected count; the reverse projection uses the original text.
export function removeNewestEvidence(text,id,count) {
  const escaped=id.replace(/[.*+?^${}()|[\]\\]/g,'\\$&');
  const matches=[...text.matchAll(new RegExp(`"id"\\s*:\\s*"${escaped}"`,'g'))];
  assert.equal(matches.length,1,`${id} unique canonical identifier`);
  const evidence=text.indexOf('"evidence":',matches[0].index);
  assert.ok(evidence>matches[0].index,`${id} evidence key`);
  const open=text.indexOf('[',evidence+11);
  assert.ok(open>evidence,`${id} evidence array`);
  let arrays=0,objects=0,quoted=false,escapedChar=false,lastComma=-1;
  for(let i=open;i<text.length;i++) {
    const c=text[i];
    if(quoted) {
      if(escapedChar) escapedChar=false;
      else if(c==='\\') escapedChar=true;
      else if(c==='"') quoted=false;
      continue;
    }
    if(c==='"') quoted=true;
    else if(c==='[') arrays++;
    else if(c===']') {
      arrays--;
      if(arrays===0) {
        assert.equal(JSON.parse(text.slice(open,i+1)).length,count,`${id} evidence count`);
        assert.ok(lastComma>open,`${id} missing appended record`);
        return text.slice(0,lastComma)+text.slice(i);
      }
    } else if(c==='{') objects++;
    else if(c==='}') objects--;
    else if(c===',' && arrays===1 && objects===0) lastComma=i;
  }
  throw Error(`${id} unterminated evidence array`);
}

export function selectedOldRows(old) {
  const oldReceipt=old['verification/current-source-replay/acceptance.json'];
  assert.equal(oldReceipt.schema,'noble-current-source-replay/v1');
  for(const id of selectedCases) {
    const row=caseRow(id), evidence=row.evidence;
    assert.deepEqual(row.state,{implementation:'implemented',execution:'passed',proof:'open',trust:'explicit'});
    assert.equal(evidence.length,2,`${id} must retain both earlier evidence entries`);
    assert.equal(evidence[0].subject,id);
    assert.equal(evidence[1].subject,id);
    assert.equal(evidence[1].source_revision,oldReceipt.source_revision);
    assert.equal(evidence[1].configuration.receipt_sha256,
      historicalReceipts['verification/current-source-replay/acceptance.json']);
    const captured=oldReceipt.cases.find(item=>item.id===id);
    assert.ok(captured,`${id} historical six-case receipt row`);
    assert.deepEqual({id:row.id,input:row.input,expected:row.expected},
      {id:captured.id,input:captured.input,expected:captured.expected});
    assert.equal(evidence[0].configuration.receipt,
      path.posix.relative(path.posix.dirname(caseFiles[id]),captured.previous_receipt));
    if(evidence[0].configuration.receipt_sha256)
      assert.equal(evidence[0].configuration.receipt_sha256,
        captured.previous_receipt_sha256,`${id} historical first receipt`);
    assert.equal(captured.previous_receipt_sha256,historicalReceipts[captured.previous_receipt]);
  }
}

export function freshOutput(destination) {
  const artifacts=path.resolve(destination);
  assert.ok(artifacts!==root && !artifacts.startsWith(`${root}/`),
    'new acceptance artifacts must be external');
  fs.mkdirSync(artifacts); // A reused output must fail, never overwrite a receipt.
  for(const name of ['home','tmp']) fs.mkdirSync(path.join(artifacts,name));
  return artifacts;
}

export function commandsAt(artifacts) {
  const selected=JSON.parse(read('policy/tool-selection.json'));
  const rust=path.join(selected.tool_paths.quality_rust.output,'bin');
  const environment={...process.env,HOME:path.join(artifacts,'home'),
    CARGO_HOME:path.join(path.dirname(artifacts),'cargo-home'),
    CARGO_TARGET_DIR:path.join(artifacts,'target'),
    TMPDIR:path.join(artifacts,'tmp'),
    RUSTC_WRAPPER:'',RUSTC_WORKSPACE_WRAPPER:'',RUSTFLAGS:'',CARGO_ENCODED_RUSTFLAGS:'',
    PATH:`${rust}:${selected.component_sync.linker_bin}:/run/current-system/sw/bin`};
  fs.mkdirSync(environment.CARGO_HOME,{recursive:true});
  const commands=[];
  function run(label,executable,args,expected=0,input) {
    const response=spawnSync(executable,args,{cwd:root,env:environment,input,
      timeout:900_000,maxBuffer:16*1024*1024});
    const stem=`${String(commands.length).padStart(2,'0')}-${label}`;
    const stdout=response.stdout??Buffer.alloc(0),stderr=response.stderr??Buffer.alloc(0);
    fs.writeFileSync(path.join(artifacts,`${stem}.stdout`),stdout,{flag:'wx'});
    fs.writeFileSync(path.join(artifacts,`${stem}.stderr`),stderr,{flag:'wx'});
    const recorded={label,executable,executable_sha256:sha(fs.readFileSync(executable)),
      args,stdin_sha256:input===undefined?null:sha(input),
      status:response.status,signal:response.signal,error:response.error?.message??null,
      stdout:`${stem}.stdout`,stderr:`${stem}.stderr`,
      stdout_sha256:sha(stdout),stderr_sha256:sha(stderr)};
    commands.push(recorded);
    assert.equal(recorded.error,null,`${label} launch`);
    assert.equal(recorded.signal,null,`${label} signal`);
    assert.equal(recorded.status,expected,`${label}: ${stderr.toString()}`);
    return stdout.toString('utf8');
  }
  const build=()=>run('build',path.join(rust,'cargo'),
    ['build','-p','noble-cli','-p','noble-contracts','--all-features',
      '--locked','--offline','--message-format=json']);
  function artifact(buildOutput,name,kind) {
    const lines=buildOutput.split('\n').filter(line=>line.startsWith('{')).map(line=>JSON.parse(line))
      .filter(row=>row.reason==='compiler-artifact' &&
        row.target.name===name && row.target.kind.includes(kind));
    const files=[...new Set(lines.flatMap(row=>kind==='bin'?[row.executable]:
      row.filenames.filter(file=>file.endsWith('.rlib'))).filter(Boolean))];
    assert.equal(files.length,1,`unique compiled ${name}/${kind}`);
    return files[0];
  }
  return {run,build,artifact,commands,environment,rust,selected};
}

export function verifyRecordedCommands(commands,artifacts) {
  for(const command of commands) {
    assert.equal(sha(fs.readFileSync(command.executable)),command.executable_sha256,
      `executable changed during ${command.label}`);
    for(const kind of ['stdout','stderr'])
      assert.equal(sha(fs.readFileSync(path.join(artifacts,command[kind]))),
        command[`${kind}_sha256`],`${command.label} raw ${kind} changed`);
    assert.equal(command.error,null,`${command.label} spawn error`);
    assert.equal(command.signal,null,`${command.label} unexpected signal`);
  }
}
