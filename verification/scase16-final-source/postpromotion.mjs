import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {root,read,sha,caseFiles,caseRow,selectedCases,older,
  dxReceipt,replayReceipt,prerequisites,inventory,revision}
  from './source.mjs';

const casePaths=[...new Set(Object.values(caseFiles))];
const conditionalLabels=['compile-pure','compile-effect','forged-empty-manifest',
  'trusted-effect-manifest','trusted-pure-manifest'];
function conditional(receipt,external) {
  const controls=receipt.conditional_imports;
  assert.deepEqual(controls.imports,{pure:[],effect:['noble.test_emit']});
  assert.deepEqual(controls.forged,{stage:'admission',outcome:'effect-manifest-reject',
    guest_requests:0,protected_operations:0});
  assert.deepEqual(controls.trusted,{guest_requests:1,trace:['test.emit:audit']});
  assert.equal(controls.pure.guest_requests,0);
  assert.deepEqual(controls.pure.stack.map(value=>[value.type,value.value]),[['I64','3']]);
  for(const name of ['pure','effect']) {
    const file=path.join(external,'conditional-imports',name,'engine','module-1.wasm');
    const bytes=fs.readFileSync(file);
    assert.equal(sha(bytes),controls.compiled[name]);
    assert.deepEqual(WebAssembly.Module.imports(new WebAssembly.Module(bytes))
      .filter(item=>item.kind==='function').map(item=>`${item.module}.${item.name}`),
    controls.imports[name]);
  }
}
function commands(receipt,external,expected) {
  assert.deepEqual(receipt.commands.map(item=>item.label),expected);
  for(const item of receipt.commands) {
    assert.equal(item.error,null);
    assert.equal(item.signal,null);
    const negative=['forged-empty-manifest','dx01-session','adapt01-session',
      'cli-S-CASE-01','cli-S-CASE-14-0','cli-S-CASE-14-1'].includes(item.label);
    assert.equal(item.status,negative?2:0,`${item.label} status`);
    assert.equal(sha(fs.readFileSync(item.executable)),item.executable_sha256);
    for(const kind of ['stdout','stderr']) assert.equal(
      sha(fs.readFileSync(path.join(external,item[kind]))),item[`${kind}_sha256`],
      `${item.label} ${kind}`);
  }
}
function common(mode,receiptFile) {
  assert.equal(fs.realpathSync(process.cwd()),root);
  const {editor,forged,initial}=prerequisites();
  const bytes=read(receiptFile),receipt=JSON.parse(bytes),digest=sha(bytes);
  assert.deepEqual([receipt.schema,receipt.result,receipt.kind,receipt.mode],
    [mode==='dx06'?'noble-scase16-final-dx06/v1':'noble-scase16-final-replay/v1',
      'passed','test',mode]);
  assert.deepEqual(receipt.prior_receipt_sha256,older);
  assert.deepEqual(receipt.prerequisites['S-CASE-16'],{
    receipt:'verification/scase16/acceptance.json',
    sha256:older['verification/scase16/acceptance.json'],
    source_revision:`sha256:${initial.source_tree_sha256}`});
  assert.equal(receipt.source_revision,revision(receipt.source_sha256));
  assert.deepEqual(Object.keys(receipt.source_sha256).sort(),
    Object.keys(inventory(mode,editor,forged)).sort());
  for(const [file,expected] of Object.entries(receipt.source_sha256))
    assert.equal(sha(read(file)),expected,`final-view source changed: ${file}`);
  assert.deepEqual(receipt.case_sha256,
    Object.fromEntries(casePaths.map(file=>[file,sha(read(file))])));
  assert.equal(sha(fs.readFileSync(receipt.binary.path)),receipt.binary.sha256);
  assert.equal(sha(fs.readFileSync(receipt.selected_node.path)),receipt.selected_node.sha256);
  // The final native view is an exact immutable source input, not exempted
  // from source checking or retroactively projected onto earlier receipts.
  assert.equal(receipt.source_sha256['.cairn/specs/safety/spec.md'],
    sha(read('.cairn/specs/safety/spec.md')));
  return {receipt,digest,initial};
}
export function verifyDx06() {
  const {receipt,digest}=common('dx06',dxReceipt);
  const row=caseRow('DX-06');
  assert.equal(sha(JSON.stringify(row.evidence)),receipt.previous_evidence_sha256);
  const external=path.dirname(receipt.smoke.directory);
  assert.equal(sha(fs.readFileSync(path.join(external,'acceptance.json'))),digest);
  const bytes=fs.readFileSync(path.join(receipt.smoke.directory,receipt.smoke.file));
  assert.equal(sha(bytes),receipt.smoke.sha256);
  const smoke=JSON.parse(bytes);
  assert.equal(smoke.result,'six-controls-pass');
  assert.equal(smoke.production_cli_sha256,receipt.binary.sha256);
  assert.equal(smoke.source_revision,receipt.smoke.original_source_revision);
  assert.deepEqual(smoke.controls,receipt.smoke.controls);
  assert.deepEqual(smoke.latent_effect_witness,['test.clock']);
  assert.equal(smoke.real_host_fallback_calls,0);
  assert.equal(smoke.real_host_evidence,false);
  for(const [file,record] of Object.entries(receipt.smoke.retained))
    assert.equal(sha(fs.readFileSync(path.join(receipt.smoke.directory,file))),record.sha256);
  conditional(receipt,external);
  commands(receipt,external,['build','six-compiled-test-clock-controls',...conditionalLabels]);
  return {receipt_sha256:digest,source_revision:receipt.source_revision,
    case_id:'DX-06',commands:receipt.commands.length,proof:'open'};
}
export function verifyReplay() {
  const prior=verifyDx06();
  const {receipt,digest}=common('replay',replayReceipt);
  assert.deepEqual(receipt.prerequisites['DX-06-final'],{
    receipt:dxReceipt,sha256:prior.receipt_sha256,
    source_revision:prior.source_revision});
  assert.deepEqual(receipt.cases.map(item=>item.id),selectedCases);
  for(const record of receipt.cases) {
    const row=caseRow(record.id);
    assert.deepEqual({id:row.id,input:row.input,expected:row.expected},
      {id:record.id,input:record.input,expected:record.expected});
    assert.equal(sha(JSON.stringify(row.evidence)),record.previous_evidence_sha256);
  }
  const external=path.dirname(receipt.binary.path).replace(/\/target\/debug$/,'');
  assert.equal(sha(fs.readFileSync(path.join(external,'acceptance.json'))),digest);
  for(const peer of Object.values(receipt.peers))
    assert.equal(sha(fs.readFileSync(peer.path)),peer.sha256);
  conditional(receipt,external);
  commands(receipt,external,['build','build-diagnostic-check',
    'build-safety-core-check','build-safety-handle-check','dx01-session',
    'dx01-typed-resource','adapt01-session','adapt01-compatible',
    'safety-source-and-kernel','cli-S-CASE-01','cli-S-CASE-14-0',
    'cli-S-CASE-14-1','safety-handle-forgery',...conditionalLabels]);
  return {receipt_sha256:digest,source_revision:receipt.source_revision,
    cases:selectedCases,commands:receipt.commands.length,proof:'open'};
}
if(process.argv[1]&&fileURLToPath(import.meta.url)===path.resolve(process.argv[1]))
  console.log(JSON.stringify(process.argv[2]==='--dx06'?verifyDx06():verifyReplay()));
