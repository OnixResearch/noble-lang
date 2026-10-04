import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { root,read,sha,caseFiles,acceptedReceipt,historic,historicalReceipts,
  sourceInventory,dx06InputFiles,revision,removeNewestEvidence } from './source.mjs';

const receiptFile='verification/dx06-current-source/acceptance.json';
const caseFile=caseFiles['DX-06'];
function filesBelow(directory) {
  const files={};
  function visit(folder) {
    for(const entry of fs.readdirSync(folder,{withFileTypes:true})) {
      const absolute=path.join(folder,entry.name);
      assert.ok(entry.isFile()||entry.isDirectory(),`unfrozen smoke entry: ${absolute}`);
      if(entry.isDirectory()) visit(absolute);
      else files[path.relative(directory,absolute).split(path.sep).join('/')]=sha(fs.readFileSync(absolute));
    }
  }
  visit(directory);
  return files;
}

// caseTexts is a byte-exact outer projection supplied only by the later
// six-case verifier. It strips the newer evidence before this older receipt is
// checked, without modifying either historical projector or canonical files.
export function verifyDx06Renewal({caseTexts}={}) {
  assert.equal(fs.realpathSync(process.cwd()),root,'run from the repository root');
  assert.equal(fileURLToPath(import.meta.url),path.join(root,
    'verification/dx06-current-source/postpromotion.mjs'));
  historic();
  const editor=acceptedReceipt('DX-02'),forged=acceptedReceipt('ADAPT-06');
  const receiptBytes=read(receiptFile),receipt=JSON.parse(receiptBytes);
  const receiptSha=sha(receiptBytes);
  assert.equal(receipt.schema,'noble-dx06-current-source/v1');
  assert.equal(receipt.result,'passed');
  assert.equal(receipt.kind,'test');
  assert.equal(receipt.case.id,'DX-06');
  assert.deepEqual(receipt.historical_receipt_sha256,historicalReceipts);
  for(const [id,accepted] of [['DX-02',editor],['ADAPT-06',forged]])
    assert.deepEqual(receipt.prerequisites[id],{receipt:accepted.receiptFile,
      sha256:accepted.receiptSha256,source_revision:accepted.evidence.source_revision});
  assert.equal(receipt.source_revision,revision(receipt.source_sha256));
  const paths=Object.keys(receipt.source_sha256);
  const casePaths=[...new Set(Object.values(caseFiles))];
  assert.deepEqual(paths.sort(),
    Object.keys(sourceInventory(dx06InputFiles(editor,forged))).sort(),
    'source inventory paths changed');
  for(const [file,digest] of Object.entries(receipt.source_sha256)) {
    if(!casePaths.includes(file)) assert.equal(sha(read(file)),digest,`frozen source changed: ${file}`);
  }
  for(const file of casePaths) {
    const current=caseTexts?.[file]??read(file).toString('utf8');
    const projected=file===caseFile?removeNewestEvidence(current,'DX-06',2):current;
    assert.equal(sha(projected),receipt.prepromotion_case_sha256[file],
      `DX-06 promotion did not preserve exact earlier case file: ${file}`);
    if(file===caseFile) {
      const row=JSON.parse(current).cases.find(item=>item.id==='DX-06');
      assert.ok(row);
      assert.deepEqual(row.state,{implementation:'implemented',execution:'passed',proof:'open',trust:'explicit'});
      assert.equal(row.evidence.length,2);
      assert.deepEqual({id:row.id,input:row.input,expected:row.expected,
        previous_evidence:row.evidence[0]},
      {id:receipt.case.id,input:receipt.case.input,expected:receipt.case.expected,
        previous_evidence:receipt.case.previous_evidence});
      const evidence=row.evidence[1];
      assert.equal(evidence.subject,'DX-06');
      assert.equal(evidence.kind,'test');
      assert.equal(evidence.result,'passed');
      assert.equal(evidence.claim,receipt.case.new_claim);
      assert.equal(evidence.source_revision,receipt.source_revision);
      assert.deepEqual(evidence.assumptions,receipt.assumptions);
      assert.equal(evidence.configuration.receipt,
        path.posix.relative(path.posix.dirname(file),receiptFile));
      assert.equal(evidence.configuration.receipt_sha256,receiptSha);
      assert.equal(evidence.configuration.prepromotion_case_sha256,receipt.prepromotion_case_sha256[file]);
      assert.equal(evidence.configuration.binary_sha256,receipt.binary.sha256);
      assert.equal(evidence.configuration.case_id,'DX-06');
      assert.equal(evidence.configuration.external_raw_output,path.dirname(receipt.smoke.directory));
      assert.equal(sha(fs.readFileSync(path.join(evidence.configuration.external_raw_output,
        'acceptance.json'))),receiptSha,'external acceptance differs from copied bytes');
    }
  }
  const cli=fs.readFileSync(receipt.binary.path);
  assert.equal(sha(cli),receipt.binary.sha256);
  assert.equal(sha(fs.readFileSync(receipt.selected_node.path)),receipt.selected_node.sha256);
  const smokeDirectory=receipt.smoke.directory;
  assert.equal(fs.realpathSync(smokeDirectory),smokeDirectory);
  const smokeBytes=fs.readFileSync(path.join(smokeDirectory,receipt.smoke.file));
  assert.equal(sha(smokeBytes),receipt.smoke.sha256);
  const smoke=JSON.parse(smokeBytes);
  assert.equal(smoke.result,'six-controls-pass');
  assert.equal(smoke.case_id,'DX-06');
  assert.equal(smoke.source_revision,receipt.smoke.original_source_revision);
  assert.equal(smoke.source_revision,`sha256:${sha(JSON.stringify(smoke.source_files))}`);
  assert.equal(smoke.pre_promotion_case_sha256,receipt.prepromotion_case_sha256[caseFile]);
  assert.equal(smoke.production_cli_sha256,receipt.binary.sha256);
  assert.equal(smoke.compiled_wasm_sha256,receipt.smoke.compiled_wasm_sha256);
  assert.deepEqual(smoke.controls,receipt.smoke.controls);
  assert.deepEqual(smoke.latent_effect_witness,receipt.smoke.latent_effect_witness);
  assert.equal(smoke.real_host_evidence,false);
  assert.equal(smoke.real_host_fallback_calls,0);
  const retained=filesBelow(smokeDirectory);
  assert.deepEqual(Object.keys(retained).sort(),[...Object.keys(receipt.smoke.retained),
    'smoke.json'].sort());
  for(const [file,record] of Object.entries(receipt.smoke.retained))
    assert.equal(retained[file],record.sha256,`raw DX-06 smoke changed: ${file}`);
  assert.equal(retained['smoke.json'],receipt.smoke.sha256);
  assert.equal(receipt.commands.length,2);
  for(const command of receipt.commands) {
    assert.equal(command.status,0,`${command.label} status`);
    assert.equal(command.error,null);
    assert.equal(command.signal,null);
    assert.equal(sha(fs.readFileSync(command.executable)),command.executable_sha256);
    for(const kind of ['stdout','stderr'])
      assert.equal(sha(fs.readFileSync(path.join(path.dirname(smokeDirectory),command[kind]))),
        command[`${kind}_sha256`],`${command.label} ${kind}`);
  }
  return {result:'passed',case_id:'DX-06',source_revision:receipt.source_revision,
    receipt_sha256:receiptSha,prior_receipts:'immutable',proof:'open'};
}
if(process.argv[1] && fileURLToPath(import.meta.url)===path.resolve(process.argv[1]))
  console.log(JSON.stringify(verifyDx06Renewal()));
