import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {root,read,sha,caseFiles,caseRow,selectedCases,historic,acceptedReceipt,
  sourceInventory,sourceTrees,revision,freshOutput,
  commandsAt as baseCommandsAt,verifyRecordedCommands,removeNewestEvidence}
  from '../dx06-current-source/source.mjs';
import {sourceFiles as precedingInputs,compiledImportControls}
  from '../scase16-current-source/source.mjs';
import {older as priorReceipts} from '../scase16-final-source/source.mjs';
import {regenerate,view} from '../../tools/cairn-specs.mjs';
import {restoreReplayEvidence as restoreSecondReplayEvidence}
  from '../adapt15-final-source/projection.mjs';
import {restoreReplayEvidence as restorePriorReleaseReplayEvidence}
  from '../adapt15-release-source/projection.mjs';
import {restoreReplayEvidence as restoreCurrentReplayEvidence}
  from '../wi03-current-source/projection.mjs';
import {restoreReplayEvidence as restoreFinalReplayEvidence}
  from './projection.mjs';
import {restoreReplayEvidence as restoreLastReleaseReplayEvidence}
  from '../wi03-release-source/projection.mjs';

export {root,read,sha,caseFiles,caseRow,selectedCases,revision,
  freshOutput,verifyRecordedCommands,removeNewestEvidence,compiledImportControls};
export const folder='verification/diagnostic-join-source';
export const dxReceipt=`${folder}/dx06-acceptance.json`;
export const replayReceipt=`${folder}/acceptance.json`;
export const older={...priorReceipts,
  'verification/mc1/acceptance.json':
    '2241decc33341c7229e863c6f99032dcbad042776be25e54cf9b73d4c555af11',
  'verification/safety-core/acceptance.json':
    '733b561741cd71790a8bc0f2231f91643155e424440844966c8e342f19d39289',
  'verification/scase16-final-source/dx06-acceptance.json':
    'bc36af63a8d80beb9e91156a6cae086d09c927a07786f2e276eca9c230249244',
  'verification/scase16-final-source/acceptance.json':
    '194ee1cadf54d7e3def7f761758721397bdfb5dcc3f78d04523c639776895464',
  'verification/scase02/acceptance.json':
    '23f2b73aff9ee4dfa0bbd7d613dba3e569caa316409a49abe9f7884eddcec0b1',
  'verification/scase02/corrected-acceptance.json':
    '3588522e32149a490ea0ec8b7c33abbcb229150b91e24a31d164a17b60a740d0',
  'verification/scase02-current-source/dx06-acceptance.json':
    'd092350b77ae605dc182e1b46d60cf5dd3f2737e447b27b3fccd99f0c8bbfb0f',
  'verification/scase02-current-source/acceptance.json':
    'dd36ff5c04808c16461ab34a1c905a73e942e73a63e889fa1ee9647bbca72046',
  'verification/scase07/acceptance.json':
    '6bb9ee9bf3d9ed1077818426d29a1bb9895fe1733e1d8bbb3041e3d7fa2013d5',
  'verification/scase07-current-source/dx06-acceptance.json':
    '3b9e726f1d9204655b72289228c3321d158de336381130b66ef8228a26af7509',
  'verification/scase07-current-source/acceptance.json':
    'b20e176c5cbe4ca7af997771b58ad569e76baae783bb3c7a36f7fdef7292fbc1',
  'verification/scase13/acceptance.json':
    '5e5e538d1e5ec90221f680de03f9063d2994073361e814668d9995fc9c45ae1e',
  'verification/scase13/corrected-acceptance.json':
    'e8211d6d2b0c77ac540874a24dc5f8dedf3bd3773c60b1105975ad9b485fcf28',
  'verification/scase13-current-source/dx06-acceptance.json':
    '4d0c779090282f633e6930ae5a045e88ff44d2f4cc22da2f906528d0a1c266c0',
  'verification/scase13-current-source/acceptance.json':
    '65ea09a8cc7e3a0a05f7f990d4b5e8cff7977192c3ecf9cbee51f4d876ff8e37',
  'verification/scase06/acceptance.json':
    '5fed58285741886c1c019dc07878e7b4c8d138d9110d240b78a0c25de7d299d8',
  'verification/scase06/corrected-acceptance.json':
    '304fad0aced504a9f241c2f0f05be4dc5d221e8e88caa97c36516079f6b428f9',
  'verification/scase06/final-acceptance.json':
    '6943c1a03163ebe5c7e98c1cdc4187cfa02a8c7007c57702e7f241d5679ab418',
  'verification/scase06-current-source/dx06-acceptance.json':
    '8c8ea0587ad033ba433fdfcaaff0a7a1cefd75b8d9e1c4dc54193a77625e22ff',
  'verification/scase06-current-source/acceptance.json':
    'aeaaba6afa2c2a7f4e18ea7704b4fc46b2c6ba029f42eb4779f1d71c15686da1',
  'verification/scase04/acceptance.json':
    '16ba34b6672486271eaef6bc22e7fd86c476374fd4f46acb745e66c2fc87f8be',
  'verification/scase04-current-source/dx06-acceptance.json':
    'cc8abef6995e4ec74dac74bab46789d55014199a8f4afa20c938946284928146',
  'verification/scase04-current-source/acceptance.json':
    '850f455e59f41be52e2ae1b7996473825af80d00653517c720b0d4c0aafcd2c3',
  'verification/scase08/acceptance.json':
    'e4da8ac29888544ef6586b12582cc1abd2d3c1065bd5c7c10d70897179402925',
  'verification/scase08-current-source/dx06-acceptance.json':
    '8de91ac4459991125462ff509df64845e85c5b26cd7810cb791c937bdbeb5588',
  'verification/scase08-current-source/acceptance.json':
    'a95ce45b7c1bd2a6ca1613e0feaf6d634e65c65923e78fd480e50bf70033feee',
  'verification/adapt15/acceptance.json':
    '84dd9752dc305ee340f8cdfe93d8c93f25b9d4462ce2c5c95fd683e567217f27',
  'verification/adapt15-current-source/dx06-acceptance.json':
    '030e820d3e6c131c3cdade5cff592a8ec9d8b1db336260bef05a8e9e0c1c54be',
  'verification/adapt15-final-source/dx06-acceptance.json':
    '14cd296c347995774d939874e259a9f4fc469eb84263a6b4aea32b61bfaffc28',
  'verification/adapt15-final-source/acceptance.json':
    '6813c93fe5cbfb7f474f84ff9d3c451245013384f7024c2475d72fc0ecc540f7',
  'verification/adapt15-release-source/dx06-acceptance.json':
    'd0bebd2159549ba0b8f025328a6c1c4972dff6976e9bff2997aa73dd2c56cc49',
  'verification/adapt15-release-source/acceptance.json':
    '26453b3d895e5f56d6b10f95b2bf9b0ae8259e12c7ff3e4530f7bfd44660b7dd',
  'verification/wi03/acceptance.json':
    'd5d9a7d891582f3247778b9417b638083a2165e762a7f1f89aa5c5fe62288f67',
  'verification/wi03-final/acceptance.json':
    '38460dc2de15b1e0cd9f5d9bb4636edba4f3c9b78502066f14f7fe083914109d',
  'verification/wi03-current-source/dx06-acceptance.json':
    '6d7fb9640d9804cda34e082aee33cabd8c52b77b8e5f46869c68230f5ed7380d',
  'verification/wi03-current-source/acceptance.json':
    'c91596d589424209adec87b3f624ec5a253748af9e5222a573f28bf9428ea248',
  'verification/wi03-final-source/dx06-acceptance.json':
    '7df51fd4f663c6727cd2c6837282df286c16276833b242a67a0d8ad10bb4887f',
  'verification/wi03-release-source/dx06-acceptance.json':
    '8d49776c1789641f349a71ec65b006d62f2ebcbb1a1772e318d6a3c85f6b3bd6',
  'verification/wi03-release-source/acceptance.json':
    '39995d3446f3a331f0c90b485ad5c5b44a881faa2b20d5dd6e1b44a3d0606f47'};
const own=['source.mjs','gate.mjs','projection.mjs','postpromotion.mjs']
  .map(file=>`${folder}/${file}`);
const inherited=[
  ...['source.mjs','gate.mjs','postpromotion.mjs']
    .map(file=>`verification/scase16-final-source/${file}`),
  ...['source.mjs','gate.mjs','projection.mjs','postpromotion.mjs']
    .map(file=>`verification/scase02-current-source/${file}`),
  ...['source.mjs','gate.mjs','projection.mjs','postpromotion.mjs']
    .map(file=>`verification/scase07-current-source/${file}`),
  ...['source.mjs','gate.mjs','projection.mjs','postpromotion.mjs']
    .map(file=>`verification/scase13-current-source/${file}`),
  ...['source.mjs','gate.mjs','projection.mjs','postpromotion.mjs']
    .map(file=>`verification/scase06-current-source/${file}`),
  ...['source.mjs','gate.mjs','projection.mjs','postpromotion.mjs']
    .map(file=>`verification/scase04-current-source/${file}`),
  ...['source.mjs','gate.mjs','projection.mjs','postpromotion.mjs']
    .map(file=>`verification/scase08-current-source/${file}`),
  ...['source.mjs','gate.mjs','projection.mjs','postpromotion.mjs']
    .map(file=>`verification/adapt15-current-source/${file}`),
  ...['source.mjs','gate.mjs','projection.mjs','postpromotion.mjs']
    .map(file=>`verification/adapt15-final-source/${file}`),
  ...['source.mjs','gate.mjs','projection.mjs','postpromotion.mjs']
    .map(file=>`verification/adapt15-release-source/${file}`),
  ...['source.mjs','gate.mjs','projection.mjs','postpromotion.mjs']
    .map(file=>`verification/wi03-current-source/${file}`),
  ...['source.mjs','gate.mjs','projection.mjs','postpromotion.mjs']
    .map(file=>`verification/wi03-final-source/${file}`),
  ...['source.mjs','gate.mjs','projection.mjs','postpromotion.mjs']
    .map(file=>`verification/wi03-release-source/${file}`),
  'verification/wi03-final/gate.mjs','verification/wi03-final/postpromotion.mjs',
  'verification/wi03/gate.mjs','verification/wi03/postpromotion.mjs'];
const trees=[...sourceTrees,'.cairn/changes/safety-wasm-manifest-admission',
  '.cairn/changes/safety-bounded-region-adapter',
  '.cairn/changes/safety-artifact-correspondence',
  '.cairn/changes/wit-exact-u64-adapter',
  'verification/wi03/peer','crates/noble-syndicate/src'];
const quotaChange='.cairn/changes/safety-runtime-quotas';
function quotaChangeSources(directory=quotaChange) {
  return fs.readdirSync(path.join(root,directory),{withFileTypes:true})
    .sort((left,right)=>left.name.localeCompare(right.name))
    .flatMap(entry=>{
      const file=`${directory}/${entry.name}`;
      assert.equal(entry.isSymbolicLink(),false,`unbound quota source symlink: ${file}`);
      if(entry.isDirectory()) return quotaChangeSources(file);
      assert.equal(entry.isFile(),true,`unbound quota source kind: ${file}`);
      return file===`${quotaChange}/tasks.md`?[]:[file];
    });
}
const fsChange='.cairn/changes/safety-authorized-fs-read';
const archivedFsChange='.cairn/archive/2026-09-30-safety-authorized-fs-read';
const fsTasks=`${fsChange}/tasks.md`;
const resourceChange='.cairn/changes/safety-recursive-resource-eligibility';
const archivedResourceChange='.cairn/archive/2026-09-30-safety-recursive-resource-eligibility';
const resourceTasks=`${resourceChange}/tasks.md`;
const callbackChange='.cairn/changes/safety-callback-returned-owner';
const callbackTasks=`${callbackChange}/tasks.md`;
const wi03Tasks='.cairn/changes/wit-exact-u64-adapter/tasks.md';
const wi03Case='specs/conformance/wit-wasi-cases.json';
const wi03Native='.cairn/specs/wit-wasi/spec.md';
const wi03Delta='.cairn/changes/wit-exact-u64-adapter/specs/wit-wasi/spec.md';
const wi03Proposal='.cairn/changes/wit-exact-u64-adapter/proposal.md';
const wi03Design='.cairn/changes/wit-exact-u64-adapter/design.md';
const correctedWit='parsed WIT `u64` MUST remain `RawType::U64` and resolve to the distinct boundary `Type::CheckedU64`, not implicitly bind to Noble `I64`';
const originalWit='WIT `u64` MUST remain distinct as RawType/Type `U64` and MUST NOT implicitly bind to Noble `I64`';
const correctedPhrases={
  [wi03Delta]:[[correctedWit,originalWit]],
  [wi03Native]:[[correctedWit,originalWit]],
  'specs/WIT-WASI.md':[[correctedWit,originalWit]],
  [wi03Proposal]:[[
    'Preserve parsed WIT `RawType::U64` and distinct resolved boundary `Type::CheckedU64`.',
    'Preserve distinct `U64` WIT raw and typed boundary identities.']],
  [wi03Design]:[[
    'WIT parsing keeps `u64` as `RawType::U64` and resolution maps it to distinct boundary `Type::CheckedU64` through generated binding and signature checking.',
    'WIT parsing/resolution keeps `u64` as distinct RawType and Type `U64` through generated binding and signature checking.'],[
    'Parsed `RawType::U64` resolves to distinct boundary `Type::CheckedU64`; only this explicit checked selection maps the latter to internal `I64`, never the default binding.',
    'WIT RawType/Type `U64` stays distinct; an explicit checked internal marker may mediate this selected export but must not turn default `U64` into `I64`.']],
  [wi03Tasks]:[[
    'parsed `RawType::U64` and distinct resolved `Type::CheckedU64`;',
    'distinct `U64` RawType/Type;']]};
const diagnosticChangedSources={
  'crates/noble-contracts/src/intrinsic/named_v2.rs':['0bbcb4f775ed08cfc6f80187017a1300fe9d6c0f43a3902b15efa813352d69a8','2d44533df6231d2c54572f4676c75019d76e3955b122508c1732a60588146458'],
  'crates/noble-contracts/src/intrinsic.rs':['4e4ed5c22bd039fbce6e4fa9d4b247ad76495dc442e391531076bade2f591512','b23e4bef1a2d7f836dd4b6eb8a9fc7999cca591f85638ef8dd22ca7bffc3b529'],
  'crates/noble-contracts/src/lib.rs':['c5f78cea53b8c4ff22b98a62956dbbfc2dde80208431ab10c115bdc3e802b375','39792a302c4569296860fba75bedbf9dc4e1eb49c635b616de69cb590ee2077a'],
  'crates/noble-contracts/src/source/declared/state/prepare.rs':['c50ece6346193a6fbfb5b4ddabcbf9c918621d6f14e4b699481de9eaa4a07cb2','b10327ac2e6c77d78fae082e6ac0ea42d22a142714ce78f4200a9a204bcdc5a6'],
  'crates/noble-contracts/src/source/declared/state/register/logical.rs':['691d30c5f2897e86dbdd21cff3fba2e0ac7ddc13c4ac60481ee86169b81bbc51','46808548aa53ce53ede2daf6eebb695ba83ec1e2bcde1c7e48cab92b38cab2b5'],
  'crates/noble-contracts/src/source/declared.rs':['34a18b85a57e8ecdf72b94339c04cc1c6e039e16b873e35778593fc048cd6497','2b816419d221c9ae2f4d4225d061140b9afd511e4d52fdcfb71a5247835384fc'],
  'crates/noble-contracts/src/source/inference/operations.rs':['dacf4026042552c45719028e56cf52b9606fc02a5d479b99b8c711f8e1577a82','54e4e39ae6ce485810bf988a500c0d6bbfb7cbbde6a2298a83605e2821b2d702'],
  'crates/noble-contracts/src/source.rs':['fe28d87fd4479a2b3e79d1d25903dbf82f7568c0e675ab25702bd31af1809219','f6e26cc7a5f2398ffed434ecd7a050d978971dfc976545f9f56b3a34b87e70b6'],
  'crates/noble-cli/src/component/authorized_fs.rs':['cf52c8e9d8a4377f8956806620d4fec654f6665f2433a37d477568745fb46ce7','1eea910f635cd69425cd8aea2e44b367c9be2718bdc1e86ef54758fb27f179f3'],
  'crates/noble-cli/src/component/bounded_region.rs':['e0b268fdace9f7a54884a5c3be488bd3f8ffb3a145fd1ce82a4cb0078f457f58','8b8f4226eac3ae0113bcb3cba9596205bdc001321dc4668e9d536647564a7321'],
  'crates/noble-cli/src/core/arguments/bindings.rs':['5c3cebb556956d62c6518f914862aa1fce4b6d2db9e430493461ec357e57a53e','13604d23423f9985b646a3db9df38e864a463a6173db5294ad67f7cd3d2493c7'],
  'crates/noble-cli/src/core/artifact.rs':['6c4ce4e03e19f498c73ffc1f61bf30101d8230de123103724e73ad74aeea6bfa','1d4a3a6cf51b87a72fce20e551f7555dac2273d4def0f53f1b284c0635fdded3'],
  'crates/noble-cli/src/workflow/intrinsic.rs':['fe49539c969d0b5cce76e35e6990146d80316bd8d6b9118c4db886cd891fdb40','7aed86c662e7900df5e673a7df8a3778d06e4a14edfd67ae58e6c19955b62051'],
  'crates/noble-contracts/tests/source.rs':['80a8dc4f796b004bd98271cd378fbcb25fea4fe04746aff00adff827e35b61e7','43e704168b1c03f7c0e50b9211e773fe85a1acccdcada2513f6b647139c45fa5']};
function beforeWitCorrection(file,text) {
  const phrases=correctedPhrases[file];
  assert.ok(phrases,`unreviewed WI-03 correction: ${file}`);
  for(const [corrected,original] of phrases) {
    const at=text.indexOf(corrected);
    assert.ok(at>=0&&text.indexOf(corrected,at+corrected.length)===-1,
      `${file} exactly one reviewed corrected phrase`);
    text=text.slice(0,at)+original+text.slice(at+corrected.length);
  }
  return text;
}
export function sourceDigest(file) {
  const bytes=read(file);
  return sha(file===fsTasks||file===resourceTasks||file===callbackTasks||file===wi03Tasks
    ?bytes.toString('utf8').replace(/- \[x\]/g,'- [ ]'):bytes);
}
export function frozen(sources) {
  for(const [file,digest] of Object.entries(sources))
    assert.equal(sourceDigest(file),digest,`source changed during source-bound execution: ${file}`);
}
function fsChangeSources(directory=fsChange) {
  assert.equal(fs.existsSync(path.join(root,archivedFsChange)),false,
    'S-CASE-06 assurance/archive is blocked; do not relabel the active snapshot');
  return fs.readdirSync(path.join(root,directory),{withFileTypes:true})
    .sort((left,right)=>left.name.localeCompare(right.name))
    .flatMap(entry=>{
      const file=`${directory}/${entry.name}`;
      assert.equal(entry.isSymbolicLink(),false,`unbound FS source symlink: ${file}`);
      if(entry.isDirectory()) return fsChangeSources(file);
      assert.equal(entry.isFile(),true,`unbound FS source kind: ${file}`);
      return [file];
    });
}
function resourceChangeSources(directory=resourceChange) {
  assert.equal(fs.existsSync(path.join(root,archivedResourceChange)),false,
    'S-CASE-04 is accepted on an active, not archived, Cairn change');
  return fs.readdirSync(path.join(root,directory),{withFileTypes:true})
    .sort((left,right)=>left.name.localeCompare(right.name))
    .flatMap(entry=>{
      const file=`${directory}/${entry.name}`;
      assert.equal(entry.isSymbolicLink(),false,`unbound Resource source symlink: ${file}`);
      if(entry.isDirectory()) return resourceChangeSources(file);
      assert.equal(entry.isFile(),true,`unbound Resource source kind: ${file}`);
      return [file];
    });
}
function callbackChangeSources(directory=callbackChange) {
  assert.equal(fs.readdirSync(path.join(root,'.cairn/archive'))
    .some(name=>name.endsWith('-safety-callback-returned-owner')),false,
  'S-CASE-08 is accepted on an active, not archived, Cairn change');
  return fs.readdirSync(path.join(root,directory),{withFileTypes:true})
    .sort((left,right)=>left.name.localeCompare(right.name))
    .flatMap(entry=>{
      const file=`${directory}/${entry.name}`;
      assert.equal(entry.isSymbolicLink(),false,`unbound callback source symlink: ${file}`);
      if(entry.isDirectory()) return callbackChangeSources(file);
      assert.equal(entry.isFile(),true,`unbound callback source kind: ${file}`);
      return [file];
    });
}
function mc1ProofSources(directory='proofs/mc1') {
  return fs.readdirSync(path.join(root,directory),{withFileTypes:true})
    .sort((left,right)=>left.name.localeCompare(right.name))
    .flatMap(entry=>{
      if(directory==='proofs/mc1'&&entry.name==='.lake') return [];
      const file=`${directory}/${entry.name}`;
      assert.equal(entry.isSymbolicLink(),false,`unbound MC1 proof source symlink: ${file}`);
      if(entry.isDirectory()) return mc1ProofSources(file);
      assert.equal(entry.isFile(),true,`unbound MC1 proof source kind: ${file}`);
      return [file];
    });
}
function verifyAcceptedTaskProse(file,digest) {
  const text=read(file).toString('utf8');
  const checklist=/^- \[[ x]\]/gm;
  const markers=[...text.matchAll(checklist)];
  assert.ok(markers.length>0&&markers.length<=12,
    `bounded finite task checklist projection: ${file}`);
  let matches=0;
  for(let mask=0;mask<2**markers.length;mask++) {
    let index=0;
    const candidate=text.replace(checklist,()=>`- [${mask&(1<<index++)?'x':' '}]`);
    if(sha(candidate)===digest) matches++;
  }
  assert.equal(matches,1,
    `exactly one accepted task preimage by checkbox markers alone: ${file}`);
}
const wi03ChangedProduction=[
  'crates/noble-cli/src/component.rs',
  'crates/noble-cli/src/component/report.rs',
  'crates/noble-contracts/src/component/bindings.rs',
  'crates/noble-contracts/src/component/context.rs',
  'crates/noble-contracts/src/component/mod.rs',
  'crates/noble-contracts/src/component/parser.rs',
  'crates/noble-contracts/src/component/parser/function.rs',
  'crates/noble-contracts/src/component/parser/resolve.rs',
  'crates/noble-contracts/src/component/parser/resolve/signature.rs',
  'crates/noble-contracts/src/component/parser/resolve/types.rs',
  'crates/noble-contracts/src/component/preparation.rs',
  'crates/noble-wasm/src/component/abi.rs',
  'crates/noble-wasm/src/component/admission/mod.rs',
  'crates/noble-wasm/src/component/lower.rs',
  'crates/noble-wasm/src/component/lower/calls.rs',
  'crates/noble-wasm/src/component/lower/inputs.rs',
  'crates/noble-wasm/src/component/lower/operations.rs',
  'crates/noble-wasm/src/component/lower/results.rs'];
function verifyHistoricalSource(file,expected,wi03) {
  const historical=wi03ChangedProduction.includes(file)
    ?wi03.source_sha256[file]:expected;
  assert.ok(historical,`historical source missing WI-03 bridge: ${file}`);
  assertDiagnosticTransition(file,historical,'historical source/WI-03 bridge');
}
function assertDiagnosticTransition(file,previous,label) {
  const change=diagnosticChangedSources[file];
  if(change) assert.equal(previous,change[0],`${label}: exact previous digest ${file}`);
  assert.equal(sourceDigest(file),change?.[1]??previous,`${label}: ${file}`);
}
function restoreDiagnosticChronology(text,file,{priorReplay=false,priorDx=false}={}) {
  const replayPaths=[...new Set(selectedCases.map(id=>caseFiles[id]))];
  if(replayPaths.includes(file)) text=restoreFinalReplayEvidence(text,file);
  if(file===caseFiles['DX-06']) {
    const length=JSON.parse(text).cases.find(row=>row.id==='DX-06').evidence.length;
    if(length===16) text=removeNewestEvidence(text,'DX-06',16);
    else assert.equal(length,15,`new Diagnostic DX06 promotion boundary ${file}`);
  }
  if(priorReplay&&replayPaths.includes(file))
    text=restoreLastReleaseReplayEvidence(text,file);
  if(priorDx&&file===caseFiles['DX-06'])
    text=removeNewestEvidence(text,'DX-06',15);
  return text;
}
function verifyLastAcceptedRelease() {
  const latestReplay=JSON.parse(read('verification/wi03-release-source/acceptance.json'));
  const casePaths=[...new Set(Object.values(caseFiles))];
  for(const file of Object.keys(diagnosticChangedSources))
    assert.ok(Object.hasOwn(latestReplay.source_sha256,file),
      `explicitly changed production/test source existed in previous release: ${file}`);
  for(const [file,expected] of Object.entries(latestReplay.source_sha256))
    if(!casePaths.includes(file))
      assertDiagnosticTransition(file,expected,'latest accepted release source bridge');
  const results={};
  for(const [mode,file] of [
    ['dx06','verification/wi03-release-source/dx06-acceptance.json'],
    ['replay','verification/wi03-release-source/acceptance.json']]) {
    const bytes=read(file),receipt=JSON.parse(bytes);
    assert.equal(sha(bytes),older[file],`${mode} previous immutable release receipt`);
    assert.deepEqual([receipt.schema,receipt.result,receipt.mode],[
      `noble-wi03-release-source-${mode}/v1`,'passed',mode]);
    assert.equal(receipt.source_revision,revision(receipt.source_sha256));
    const id=mode==='dx06'?'DX-06':selectedCases[0];
    const evidence=caseRow(id).evidence[mode==='dx06'?14:13];
    assert.equal(evidence.configuration.receipt_sha256,older[file]);
    assert.equal(sha(fs.readFileSync(path.join(
      evidence.configuration.external_raw_output,'acceptance.json'))),older[file]);
    for(const [input,expected] of Object.entries(receipt.source_sha256)) {
      if(casePaths.includes(input)) {
        const text=restoreDiagnosticChronology(read(input).toString('utf8'),input,
          {priorReplay:true,priorDx:mode==='dx06'});
        assert.equal(sha(text),expected,`${mode} exact prior release canonical preimage: ${input}`);
      } else assertDiagnosticTransition(input,expected,`${mode} previous release source`);
    }
    results[mode]=receipt;
  }
  return results;
}
export function verifyWi03Projection() {
  const bytes=read('verification/wi03-final/acceptance.json');
  assert.equal(sha(bytes),older['verification/wi03-final/acceptance.json']);
  const receipt=JSON.parse(bytes),external=receipt.external_raw_output;
  assert.deepEqual([receipt.schema,receipt.kind,receipt.result,receipt.failures,
    receipt.integrity_failures],[
    'noble-wi03-final-source-body-guard/v1','test','passed',[],[]]);
  assert.equal(receipt.source_revision,revision(receipt.source_sha256));
  assert.deepEqual(receipt.prior_prepromotion_receipt,{
    path:'verification/wi03/acceptance.json',
    sha256:older['verification/wi03/acceptance.json'],
    source_revision:'sha256:6ff838c42913498a7eb120a6ba1b009621afb56559b2676d9115faa33e7555f0',
    scope:'historical-only-unpromoted'});
  assert.equal(path.isAbsolute(external),true);
  assert.equal(sha(fs.readFileSync(path.join(external,'acceptance.json'))),sha(bytes));
  const oldCase=fs.readFileSync(path.join(external,receipt.prepromotion_case_path),'utf8');
  assert.equal(sha(oldCase),receipt.prepromotion_case_sha256);
  assert.equal(receipt.prepromotion_case_sha256,
    receipt.source_sha256[wi03Case]);
  const now=read(wi03Case).toString('utf8'),oldPacket=JSON.parse(oldCase);
  const oldRows=oldPacket.cases.filter(row=>row.id==='WI-03');
  const rows=JSON.parse(now).cases.filter(row=>row.id==='WI-03');
  assert.equal(oldRows.length,1);assert.equal(rows.length,1);
  const [oldRow]=oldRows,[row]=rows;
  assert.deepEqual(Object.fromEntries(Object.keys(receipt.case)
    .map(key=>[key,row[key]])),receipt.case);
  assert.deepEqual(Object.fromEntries(Object.keys(receipt.case)
    .map(key=>[key,oldRow[key]])),receipt.case);
  assert.deepEqual(oldRow.state,{implementation:'absent',execution:'not-run',
    proof:'open',trust:'unassessed'});
  assert.deepEqual(oldRow.evidence,[]);
  assert.deepEqual(row.state,{implementation:'implemented',execution:'passed',
    proof:'open',trust:'explicit'});
  assert.equal(row.evidence.length,1);
  assert.deepEqual([row.evidence[0].kind,row.evidence[0].subject,
    row.evidence[0].result,row.evidence[0].source_revision,
    row.evidence[0].configuration.receipt,
    row.evidence[0].configuration.receipt_sha256],[
    'test','WI-03','passed',receipt.source_revision,
    '../../verification/wi03-final/acceptance.json',
    older['verification/wi03-final/acceptance.json']]);
  assert.equal(row.evidence[0].configuration.prepromotion_case_sha256,
    receipt.prepromotion_case_sha256);
  assert.equal(row.evidence[0].configuration.gate_sha256,
    receipt.source_sha256['verification/wi03-final/gate.mjs']);
  assert.equal(row.evidence[0].configuration.external_raw_output,external);
  assert.equal(row.evidence[0].configuration.binary_sha256,receipt.binaries.cli.sha256);
  assert.deepEqual(row.evidence[0].assumptions,receipt.assumptions);
  const beforeLine=oldCase.split('\n').filter(line=>line.includes('"id":"WI-03"'));
  const afterLine=now.split('\n').filter(line=>line.includes('"id":"WI-03"'));
  assert.equal(beforeLine.length,1);assert.equal(afterLine.length,1);
  assert.equal(beforeLine[0],`    ${JSON.stringify(oldRow)},`);
  assert.equal(afterLine[0],`    ${JSON.stringify(row)},`);
  assert.deepEqual({...row,state:oldRow.state,evidence:oldRow.evidence},oldRow);
  assert.equal(now.replace(afterLine[0],beforeLine[0]),oldCase,
    'only WI-03 canonical state/evidence may differ from -d prepromotion');

  const oldNative=fs.readFileSync(path.join(external,'prepromotion-wit-spec.md'),'utf8');
  assert.equal(sha(oldNative),receipt.source_sha256[wi03Native]);
  const native=read(wi03Native).toString('utf8');
  const delta=read(wi03Delta).toString('utf8');
  assert.equal(sha(beforeWitCorrection(wi03Delta,delta)),
    receipt.source_sha256[wi03Delta],
    'original WI-03 producer delta differs only in the reviewed type-name correction');
  const family=JSON.parse(read('specs/spec-family.json'));
  const cases=family.scenario_files.flatMap(file=>
    JSON.parse(read(`specs/${file}`)).cases.map(item=>({...item,file})));
  assert.equal(regenerate(native,'WIT-WASI.md',cases),native,
    'synced native WI-03 document must be exact Cairn renderer output');
  assert.equal(view(native,'WIT-WASI.md'),read('specs/WIT-WASI.md').toString('utf8'));
  const sections=text=>text.split(/(?=^### Requirement: )/m);
  const oldParts=sections(oldNative),newParts=sections(native),deltaParts=sections(delta);
  assert.equal(newParts.length,oldParts.length,'native WI requirement inventory');
  const start='<!-- cairn:scenario-links:start -->';
  const end='<!-- cairn:scenario-links:end -->';
  for(let i=0;i<oldParts.length;i++) {
    const id=/^### Requirement: ([A-Z0-9-]+)/.exec(oldParts[i])?.[1];
    assert.equal(/^### Requirement: ([A-Z0-9-]+)/.exec(newParts[i])?.[1],id);
    if(id!=='WI-WIT-02'&&id!=='WI-WIT-06') {
      assert.equal(newParts[i],oldParts[i],`unrelated WI native requirement: ${id}`);
      continue;
    }
    const matching=deltaParts.filter(part=>part.startsWith(`### Requirement: ${id}\n`));
    assert.equal(matching.length,1,`${id} one immutable authored delta`);
    const before=oldParts[i],after=newParts[i];
    const a=before.indexOf(start),b=before.indexOf(end,a);
    const c=after.indexOf(start),d=after.indexOf(end,c);
    assert.ok(a>=0&&b>a&&c>=0&&d>c,`${id} generated region bounds`);
    assert.equal(before.slice(a,b+end.length),after.slice(c,d+end.length),
      `${id} original generated scenario links unchanged`);
    assert.equal(after.indexOf(start,c+start.length),-1);
    const withoutGenerated=after.slice(0,c).replace(/\n+$/,'\n\n')+
      after.slice(d+end.length).replace(/^\n+/,'');
    const withoutLabel=withoutGenerated.replace(`\n\n**${id}.**\n\n`,'\n\n');
    assert.notEqual(withoutLabel,withoutGenerated,`${id} expected Cairn legacy label`);
    assert.equal(withoutLabel.trimEnd(),matching[0].trimEnd(),
      `${id} only reviewed authored delta around renderer links`);
  }
  const frozenTasks=fs.readFileSync(path.join(external,'prepromotion-native-tasks.md'),'utf8');
  assert.equal(sha(frozenTasks),receipt.source_sha256[wi03Tasks]);
  assert.equal(beforeWitCorrection(wi03Tasks,read(wi03Tasks).toString('utf8'))
    .replace(/- \[x\]/g,'- [ ]'),
    frozenTasks.replace(/- \[x\]/g,'- [ ]'),
    'WI-03 task prose differs only by exact reviewed type-name and checklist marks');
  for(const file of [wi03Proposal,wi03Design]) assert.equal(
    sha(beforeWitCorrection(file,read(file).toString('utf8'))),
    receipt.source_sha256[file],`${file} exact original producer type-name preimage`);
  assert.equal(fs.readdirSync(path.join(root,'.cairn/archive'))
    .some(name=>name.endsWith('-wit-exact-u64-adapter')),false);
  for(const [name,digest] of Object.entries(receipt.source_sha256)) {
    if(name!==wi03Case&&name!==wi03Native&&name!==wi03Tasks&&
      name!==wi03Delta&&name!==wi03Proposal&&name!==wi03Design)
      assertDiagnosticTransition(name,digest,'WI-03 prepromotion bound source bridge');
  }
  for(const [name,digest] of Object.entries(receipt.historical_receipt_sha256))
    assert.equal(sha(read(name)),digest,`WI-03 historical receipt changed: ${name}`);
  for(const command of receipt.commands) {
    assert.deepEqual([command.error,command.signal,command.status],
      [null,null,command.expected_status]);
    assert.equal(sha(fs.readFileSync(command.executable)),command.executable_sha256);
    for(const stream of ['stdout','stderr'])
      assert.equal(sha(fs.readFileSync(path.join(external,command[stream]))),
        command[`${stream}_sha256`]);
  }
  for(const artifact of Object.values(receipt.binaries))
    assert.equal(sha(fs.readFileSync(artifact.path)),artifact.sha256);
  return receipt;
}
function verifyPreviousRelease() {
  const results={};
  const casePaths=[...new Set(Object.values(caseFiles))];
  const replayPaths=[...new Set(selectedCases.map(id=>caseFiles[id]))];
  const dxFile=caseFiles['DX-06'];
  for(const [mode,file] of [
    ['dx06','verification/wi03-current-source/dx06-acceptance.json'],
    ['replay','verification/wi03-current-source/acceptance.json']]) {
    const bytes=read(file),receipt=JSON.parse(bytes);
    assert.equal(sha(bytes),older[file],`${mode} unchanged prior WI-03 receipt`);
    assert.deepEqual([receipt.schema,receipt.result,receipt.mode],[
      `noble-wi03-current-source-${mode}/v1`,'passed',mode]);
    assert.equal(receipt.source_revision,revision(receipt.source_sha256));
    const id=mode==='dx06'?'DX-06':selectedCases[0];
    const external=caseRow(id).evidence[12].configuration.external_raw_output;
    assert.equal(caseRow(id).evidence[12].configuration.receipt_sha256,older[file]);
    assert.equal(sha(fs.readFileSync(path.join(external,'acceptance.json'))),older[file],
      `${mode} immutable prior raw receipt`);
    for(const [input,expected] of Object.entries(receipt.source_sha256)) {
      if(correctedPhrases[input]) {
        let text=beforeWitCorrection(input,read(input).toString('utf8'));
        if(input===wi03Tasks) text=text.replace(/- \[x\]/g,'- [ ]');
        assert.equal(sha(text),expected,`${mode} exact WI-03 normative preimage: ${input}`);
      } else if(casePaths.includes(input)) {
        let text=restoreDiagnosticChronology(read(input).toString('utf8'),input,
          {priorReplay:true,priorDx:true});
        if(input===dxFile) {
          text=removeNewestEvidence(text,'DX-06',14);
        }
        if(replayPaths.includes(input)) text=restoreCurrentReplayEvidence(text,input);
        if(input===dxFile&&mode==='dx06')
          text=removeNewestEvidence(text,'DX-06',13);
        assert.equal(sha(text),expected,`${mode} exact chronological case preimage: ${input}`);
      } else assertDiagnosticTransition(input,expected,
        `${mode} historical source bridge`);
    }
    results[mode]=receipt;
  }
  return results;
}
function verifyFailedDxRelease() {
  const file='verification/wi03-final-source/dx06-acceptance.json';
  const bytes=read(file),receipt=JSON.parse(bytes);
  assert.equal(sha(bytes),older[file],'first corrected-native DX receipt immutable');
  assert.deepEqual([receipt.schema,receipt.result,receipt.mode],[
    'noble-wi03-final-source-dx06/v1','passed','dx06']);
  assert.equal(receipt.source_revision,revision(receipt.source_sha256));
  const dx=caseRow('DX-06');
  assert.ok(dx.evidence.length>=15&&dx.evidence.length<=16);
  assert.equal(dx.evidence[13].configuration.receipt_sha256,older[file]);
  const external=dx.evidence[13].configuration.external_raw_output;
  assert.equal(sha(fs.readFileSync(path.join(external,'acceptance.json'))),older[file]);
  const casePaths=[...new Set(Object.values(caseFiles))];
  const replayPaths=[...new Set(selectedCases.map(id=>caseFiles[id]))];
  for(const [input,expected] of Object.entries(receipt.source_sha256)) {
    if(casePaths.includes(input)) {
      let text=restoreDiagnosticChronology(read(input).toString('utf8'),input,
        {priorReplay:true,priorDx:true});
      if(input===caseFiles['DX-06']) {
        text=removeNewestEvidence(text,'DX-06',14);
      }
      assert.equal(sha(text),expected,`first DX exact historical case preimage: ${input}`);
    } else assertDiagnosticTransition(input,expected,
      'first DX historical source/runner bridge');
  }
  return receipt;
}
export function commandsAt(artifacts) {
  const command=baseCommandsAt(artifacts);
  // The newer pinned Wasmtime path dependency requires addr2line, whose
  // offline registry entry exists in the selected user's populated Cargo home.
  // Keep the target, temp and compiler wrappers externally isolated.
  command.environment.CARGO_HOME=path.join(path.dirname(root), '..', '.cargo');
  assert.equal(fs.realpathSync(command.environment.CARGO_HOME),
    '/home/brittonr/.cargo');
  return command;
}
export function prerequisites() {
  const historical=historic();
  for(const [file,digest] of Object.entries(older))
    assert.equal(sha(read(file)),digest,`immutable prerequisite changed: ${file}`);
  const lastRelease=verifyLastAcceptedRelease();
  const wi03=verifyWi03Projection();
  const previous=verifyPreviousRelease();
  const failedDx=verifyFailedDxRelease();
  const editor=acceptedReceipt('DX-02'),forged=acceptedReceipt('ADAPT-06');
  const initial=JSON.parse(read('verification/scase16/acceptance.json'));
  const safety=JSON.parse(read('specs/conformance/safety-cases.json'));
  const case16=safety.cases.find(row=>row.id==='S-CASE-16');
  assert.deepEqual(case16.state,{implementation:'implemented',execution:'passed',
    proof:'open',trust:'explicit'});
  assert.equal(case16.evidence[0].configuration.receipt_sha256,
    older['verification/scase16/acceptance.json']);
  const bounded=safety.cases.find(row=>row.id==='S-CASE-02');
  const accepted=JSON.parse(read('verification/scase02/corrected-acceptance.json'));
  assert.deepEqual(bounded.state,{implementation:'implemented',execution:'passed',
    proof:'open',trust:'explicit'});
  assert.equal(bounded.evidence.length,2);
  assert.deepEqual({id:bounded.id,input:bounded.input,expected:bounded.expected},accepted.case);
  assert.deepEqual([accepted.schema,accepted.kind,accepted.result],
    ['noble-scase02-bounded-region-corrected/v1','test','passed']);
  assert.equal(accepted.source_revision,bounded.evidence[1].source_revision);
  assert.equal(bounded.evidence[0].configuration.receipt_sha256,
    older['verification/scase02/acceptance.json']);
  assert.equal(bounded.evidence[1].configuration.receipt_sha256,
    older['verification/scase02/corrected-acceptance.json']);
  assert.equal(accepted.historical_receipt_sha256,
    older['verification/scase02/acceptance.json']);
  assert.equal(accepted.cases.find(row=>row.label==='canonical-scase02').observed.region_reads,0);
  assert.equal(accepted.cases.find(row=>row.label==='positive-in-bounds').observed.bytes_hex,'0102');
  const external=bounded.evidence[1].configuration.external_raw_output;
  assert.equal(sha(fs.readFileSync(path.join(external,'acceptance.json'))),
    older['verification/scase02/corrected-acceptance.json']);
  const artifact=JSON.parse(read('verification/scase07/acceptance.json'));
  const corresponding=safety.cases.find(row=>row.id==='S-CASE-07');
  assert.deepEqual([artifact.schema,artifact.kind,artifact.result],
    ['noble-scase07-artifact-correspondence/v1','test','passed']);
  assert.deepEqual(corresponding.state,{implementation:'implemented',
    execution:'passed',proof:'open',trust:'explicit'});
  assert.equal(corresponding.evidence.length,1);
  assert.deepEqual(Object.fromEntries(
    ['id','profile','kind','requirements','input','expected']
      .map(key=>[key,corresponding[key]])),artifact.case);
  assert.equal(corresponding.evidence[0].configuration.receipt_sha256,
    older['verification/scase07/acceptance.json']);
  assert.equal(corresponding.evidence[0].source_revision,artifact.source_revision);
  assert.equal(corresponding.evidence[0].configuration.prepromotion_case_sha256,
    artifact.prepromotion_case_sha256);
  assert.equal(sha(fs.readFileSync(path.join(artifact.external_raw_output,'acceptance.json'))),
    older['verification/scase07/acceptance.json']);
  const quota=JSON.parse(read('verification/scase13/corrected-acceptance.json'));
  const limited=safety.cases.find(row=>row.id==='S-CASE-13');
  assert.deepEqual([quota.schema,quota.kind,quota.result],
    ['noble-scase13-runtime-quotas/v2','test','passed']);
  assert.deepEqual(limited.state,{implementation:'implemented',
    execution:'passed',proof:'open',trust:'explicit'});
  assert.equal(limited.evidence.length,2);
  assert.deepEqual({id:limited.id,input:limited.input,expected:limited.expected},quota.case);
  assert.equal(limited.evidence[0].configuration.receipt_sha256,
    older['verification/scase13/acceptance.json']);
  assert.equal(limited.evidence[1].configuration.receipt_sha256,
    older['verification/scase13/corrected-acceptance.json']);
  assert.equal(limited.evidence[1].source_revision,quota.source_revision);
  const quotaExternal=limited.evidence[1].configuration.external_raw_output;
  assert.equal(sha(fs.readFileSync(path.join(quotaExternal,'prepromotion-safety-cases.json'))),
    quota.prepromotion_case_sha256);
  assert.equal(sha(fs.readFileSync(path.join(quotaExternal,'acceptance.json'))),
    older['verification/scase13/corrected-acceptance.json']);
  const mismatch=quota.commands.find(row=>row.label==='reject-unmatched-source-recipe');
  assert.equal(mismatch.status,2);
  assert.equal(fs.readFileSync(path.join(quotaExternal,mismatch.stdout)).length,0);
  assert.equal(fs.readFileSync(path.join(quotaExternal,mismatch.stderr),'utf8').trim(),
    'quota-core: Core artifact does not match the independently compiled Noble source recipe');
  const authorization=JSON.parse(read('verification/scase06/final-acceptance.json'));
  const granted=safety.cases.find(row=>row.id==='S-CASE-06');
  assert.deepEqual([authorization.schema,authorization.result,authorization.failures,
    authorization.integrity_failures],
    ['noble-scase06-authorized-fs-final/v1','passed',[],[]]);
  assert.deepEqual({id:granted.id,input:granted.input,expected:granted.expected},
    authorization.case);
  assert.deepEqual(granted.state,{implementation:'implemented',execution:'passed',
    proof:'open',trust:'explicit'});
  assert.equal(granted.evidence.length,3);
  assert.deepEqual(granted.evidence.map(row=>row.configuration.receipt_sha256),[
    older['verification/scase06/acceptance.json'],
    older['verification/scase06/corrected-acceptance.json'],
    older['verification/scase06/final-acceptance.json']]);
  assert.equal(granted.evidence[2].source_revision,authorization.source_revision);
  assert.equal(granted.evidence[2].configuration.prepromotion_case_sha256,
    authorization.prepromotion_case_sha256);
  const fsExternal=granted.evidence[2].configuration.external_raw_output;
  assert.equal(sha(fs.readFileSync(path.join(fsExternal,'acceptance.json'))),
    older['verification/scase06/final-acceptance.json']);
  assert.deepEqual(authorization.observations.map(item=>
    [item.label,item.observed.outcome,item.observed.protected_operations]),[
      ['canonical-scase06','denied',0],
      ['same-compiled-guest-positive','read',1]]);
  assert.equal(authorization.commands.find(item=>
    item.label==='mismatched-host-source-refusal').status,2);
  fsChangeSources();
  const resource=JSON.parse(read('verification/scase04/acceptance.json'));
  const eligibility=safety.cases.find(row=>row.id==='S-CASE-04');
  assert.deepEqual([resource.schema,resource.kind,resource.result,
    resource.failures,resource.integrity_failures],
    ['noble-scase04-recursive-resource-eligibility/v1','test','passed',[],[]]);
  assert.deepEqual({id:eligibility.id,input:eligibility.input,expected:eligibility.expected},
    resource.case);
  assert.deepEqual(eligibility.state,{implementation:'implemented',
    execution:'passed',proof:'open',trust:'explicit'});
  assert.equal(eligibility.evidence.length,1);
  assert.equal(eligibility.evidence[0].configuration.receipt_sha256,
    older['verification/scase04/acceptance.json']);
  assert.equal(eligibility.evidence[0].source_revision,resource.source_revision);
  assert.equal(eligibility.evidence[0].configuration.prepromotion_case_sha256,
    resource.prepromotion_case_sha256);
  assert.deepEqual([resource.canonical_mapping.stage,resource.canonical_mapping.outcome,
    resource.canonical_mapping.guest_requests,resource.canonical_mapping.protected_operations],
    ['check','eligibility-reject',0,0]);
  assert.equal(resource.typed.negatives.length,16);
  assert.equal(sha(fs.readFileSync(path.join(resource.external_raw_output,'acceptance.json'))),
    older['verification/scase04/acceptance.json']);
  resourceChangeSources();
  const callback=JSON.parse(read('verification/scase08/acceptance.json'));
  const returned=safety.cases.find(row=>row.id==='S-CASE-08');
  assert.deepEqual([callback.schema,callback.kind,callback.result,
    callback.failures,callback.integrity_failures],
    ['noble-scase08-callback-owner/v1','test','passed',[],[]]);
  assert.deepEqual({id:returned.id,input:returned.input,expected:returned.expected},
    callback.case);
  assert.deepEqual(returned.state,{implementation:'implemented',
    execution:'passed',proof:'open',trust:'explicit'});
  assert.equal(returned.evidence.length,1);
  assert.equal(returned.evidence[0].configuration.receipt_sha256,
    older['verification/scase08/acceptance.json']);
  assert.equal(returned.evidence[0].source_revision,callback.source_revision);
  assert.equal(returned.evidence[0].configuration.prepromotion_case_sha256,
    callback.prepromotion_case_sha256);
  assert.equal(sha(fs.readFileSync(path.join(
    returned.evidence[0].configuration.external_raw_output,'acceptance.json'))),
    older['verification/scase08/acceptance.json']);
  assert.deepEqual(callback.observations.map(row=>[
    row.mode,row.report.stage,row.report.outcome,row.report.guest_requests,
    row.report.trusted_values_created,row.report.protected_operations,
    row.report.released_owners]),[
      ['authentic','callback','accepted',2,1,1,1],
      ['stale-generation','callback','invalid-result',1,0,0,1],
      ['wrong-kind','callback','invalid-result',1,0,0,1],
      ['wrong-context','callback','invalid-result',1,0,0,1]]);
  assert.deepEqual(callback.historical_receipt_sha256,
    Object.fromEntries(Object.entries(older)
      .filter(([file])=>Object.hasOwn(callback.historical_receipt_sha256,file))));
  assert.equal(callback.prepromotion_tasks_sha256,callback.source_sha256[callbackTasks]);
  verifyAcceptedTaskProse(callbackTasks,callback.prepromotion_tasks_sha256);
  callbackChangeSources();
  for(const [file,digest] of Object.entries(callback.source_sha256)) {
    if(file!=='specs/conformance/safety-cases.json'&&
      file!=='.cairn/specs/safety/spec.md'&&
      file!=='.cairn/specs/resource-adapters/spec.md'&&
      file!=='specs/SAFETY.md'&&
      file!==callbackTasks)
      verifyHistoricalSource(file,digest,wi03);
  }
  const admission=JSON.parse(read('verification/adapt15/acceptance.json'));
  const adaptation=JSON.parse(read('specs/conformance/adaptation-cases.json'));
  const admitted=adaptation.cases.find(row=>row.id==='ADAPT-15');
  assert.deepEqual([admission.schema,admission.kind,admission.result,
    admission.failures,admission.integrity_failures],
    ['noble-adapt15-mandatory-admission/v1','test','passed',[],[]]);
  assert.deepEqual(Object.fromEntries(['id','profile','kind','requirements',
    'input','expected'].map(key=>[key,admitted[key]])),admission.case);
  assert.deepEqual(admitted.state,{implementation:'implemented',
    execution:'passed',proof:'open',trust:'explicit'});
  assert.equal(admitted.evidence.length,1);
  assert.equal(admitted.evidence[0].configuration.receipt_sha256,
    older['verification/adapt15/acceptance.json']);
  assert.equal(admitted.evidence[0].source_revision,admission.source_revision);
  assert.equal(admitted.evidence[0].configuration.prepromotion_case_sha256,
    admission.prepromotion_case_sha256);
  assert.equal(sha(fs.readFileSync(path.join(admission.external_raw_output,'acceptance.json'))),
    older['verification/adapt15/acceptance.json']);
  assert.equal(path.isAbsolute(admission.external_raw_output),true);
  for(const [file,digest] of Object.entries(admission.historical_receipt_sha256))
    assert.equal(older[file],digest,`accepted ADAPT-15 historical receipt mismatch: ${file}`);
  for(const [name,snapshot] of Object.entries(admission.prepromotion_inputs))
    assert.equal(sha(fs.readFileSync(path.join(admission.external_raw_output,snapshot.path))),
      snapshot.sha256,`accepted ADAPT-15 source snapshot changed: ${name}`);
  assert.equal(admission.prepromotion_inputs.case.sha256,admission.prepromotion_case_sha256);
  for(const [name,input] of Object.entries(admission.input_sha256))
    assert.equal(sha(fs.readFileSync(path.join(admission.external_raw_output,input.path))),
      input.sha256,`accepted ADAPT-15 proof/input bytes changed: ${name}`);
  assert.deepEqual(admission.cells.map(cell=>[cell.build,cell.optional_proof,
    cell.stage,cell.outcome,cell.constraint,cell.accepted_program_created,
    cell.guest_requests,cell.protected_operations,cell.proof_checked]),
    admitted.input.variants.map(cell=>[cell.build,cell.optional_proof,
      'acceptance','reject','EffectInclusion(test.emit)',false,0,0,
      cell.optional_proof==='unrelated-valid-proof']));
  assert.equal(admission.proof.validated,true);
  assert.equal(admission.proof.attached_to_kernel_admission,false);
  assert.equal(admission.proof.subject_matches_forged,false);
  assert.deepEqual(Object.keys(wi03.source_sha256).filter(file=>
    Object.hasOwn(admission.source_sha256,file)&&
    wi03.source_sha256[file]!==admission.source_sha256[file]).sort(),
    wi03ChangedProduction,'exact WI-03 production transition since ADAPT-15');
  for(const [file,digest] of Object.entries(admission.source_sha256)) {
    if(file==='specs/conformance/adaptation-cases.json') continue;
    if(file===fsTasks||file===resourceTasks||file===callbackTasks)
      verifyAcceptedTaskProse(file,digest);
    else if(file==='specs/conformance/safety-cases.json') {
      const afterFinal=restoreDiagnosticChronology(read(file).toString('utf8'),file,
        {priorReplay:true});
      const afterCurrent=restoreCurrentReplayEvidence(afterFinal,file);
      const afterRelease=restorePriorReleaseReplayEvidence(afterCurrent,file);
      const afterSecond=restoreSecondReplayEvidence(afterRelease,file);
      assert.equal(sha(afterSecond),digest,
        'ADAPT-15 accepted safety preimage after exact newer source-bound projections');
    } else verifyHistoricalSource(file,digest,wi03);
  }
  const historicalDx=JSON.parse(read('verification/adapt15-current-source/dx06-acceptance.json'));
  assert.deepEqual([historicalDx.schema,historicalDx.result,historicalDx.mode],
    ['noble-adapt15-three-active-dx06/v1','passed','dx06']);
  assert.equal(historicalDx.source_sha256['verification/adapt15-current-source/postpromotion.mjs'],
    'c2da9cfd7ee4ba561e6414f5573f3da52d010bf15f313bbf2618084809e72c63');
  assert.equal(sha(fs.readFileSync(path.join(
    path.dirname(historicalDx.smoke.directory),'acceptance.json'))),
    older['verification/adapt15-current-source/dx06-acceptance.json']);
  const dx=caseRow('DX-06');
  assert.ok(dx.evidence.length>=15&&dx.evidence.length<=16);
  const dxFile=caseFiles['DX-06'];
  const afterReleaseDx=restoreDiagnosticChronology(read(dxFile).toString('utf8'),dxFile,
    {priorReplay:true,priorDx:true});
  const afterFirstFinalDx=removeNewestEvidence(afterReleaseDx,'DX-06',14);
  const afterCurrentReplay=restoreCurrentReplayEvidence(afterFirstFinalDx,dxFile);
  const beforeCurrentDx=removeNewestEvidence(afterCurrentReplay,'DX-06',13);
  const afterRelease=restorePriorReleaseReplayEvidence(beforeCurrentDx,dxFile);
  const beforeReleaseDx=removeNewestEvidence(afterRelease,'DX-06',12);
  const secondOnly=restoreSecondReplayEvidence(beforeReleaseDx,dxFile);
  const firstOnly=removeNewestEvidence(secondOnly,'DX-06',11);
  const first=JSON.parse(firstOnly).cases.find(row=>row.id==='DX-06');
  assert.equal(first.evidence.length,10);
  assert.equal(first.evidence[9].configuration.receipt_sha256,
    older['verification/adapt15-current-source/dx06-acceptance.json']);
  assert.equal(first.evidence[9].source_revision,historicalDx.source_revision);
  assert.equal(sha(removeNewestEvidence(firstOnly,'DX-06',10)),
    historicalDx.case_sha256[dxFile],'historical finite DX-06 case preimage');
  const secondDx=JSON.parse(read('verification/adapt15-final-source/dx06-acceptance.json'));
  const secondReplay=JSON.parse(read('verification/adapt15-final-source/acceptance.json'));
  assert.deepEqual([secondDx.schema,secondDx.result,secondDx.mode,
    secondReplay.schema,secondReplay.result,secondReplay.mode],[
      'noble-adapt15-corrected-three-active-dx06/v1','passed','dx06',
      'noble-adapt15-corrected-three-active-replay/v1','passed','replay']);
  assert.equal(secondDx.source_sha256['verification/adapt15-final-source/postpromotion.mjs'],
    '81addf54699531bd9c095802553c43b0d0cbb0ac736444bf1abfde90138e6f6a');
  assert.equal(sha(fs.readFileSync(path.join(
    path.dirname(secondDx.smoke.directory),'acceptance.json'))),
    older['verification/adapt15-final-source/dx06-acceptance.json']);
  assert.equal(sha(fs.readFileSync(path.join(
    path.dirname(secondReplay.binary.path), '..','..','acceptance.json'))),
    older['verification/adapt15-final-source/acceptance.json']);
  const second=JSON.parse(secondOnly).cases.find(row=>row.id==='DX-06');
  assert.equal(second.evidence.length,11);
  assert.equal(second.evidence[10].configuration.receipt_sha256,
    older['verification/adapt15-final-source/dx06-acceptance.json']);
  assert.equal(second.evidence[10].source_revision,secondDx.source_revision);
  assert.equal(sha(removeNewestEvidence(secondOnly,'DX-06',11)),
    secondDx.case_sha256[dxFile],'second historical finite DX-06 case preimage');
  const priorReleaseDx=JSON.parse(read('verification/adapt15-release-source/dx06-acceptance.json'));
  const priorReleaseReplay=JSON.parse(read('verification/adapt15-release-source/acceptance.json'));
  assert.deepEqual([priorReleaseDx.schema,priorReleaseDx.result,priorReleaseDx.mode,
    priorReleaseReplay.schema,priorReleaseReplay.result,priorReleaseReplay.mode],[
      'noble-adapt15-release-three-active-dx06/v1','passed','dx06',
      'noble-adapt15-release-three-active-replay/v1','passed','replay']);
  assert.equal(sha(fs.readFileSync(path.join(
    path.dirname(priorReleaseDx.smoke.directory),'acceptance.json'))),
    older['verification/adapt15-release-source/dx06-acceptance.json']);
  assert.equal(sha(fs.readFileSync(path.join(
    path.dirname(priorReleaseReplay.binary.path),'..','..','acceptance.json'))),
    older['verification/adapt15-release-source/acceptance.json']);
  const priorRelease=JSON.parse(afterRelease).cases.find(row=>row.id==='DX-06');
  assert.equal(priorRelease.evidence.length,12);
  assert.equal(priorRelease.evidence[11].configuration.receipt_sha256,
    older['verification/adapt15-release-source/dx06-acceptance.json']);
  assert.equal(sha(removeNewestEvidence(afterRelease,'DX-06',12)),
    priorReleaseDx.case_sha256[dxFile],'prior accepted DX-06 source preimage');
  for(const id of selectedCases) {
    const row=caseRow(id);
    assert.ok(row.evidence.length>=14&&row.evidence.length<=15,`${id} prior evidence`);
    assert.equal(row.evidence[9].configuration.receipt_sha256,
      older['verification/scase08-current-source/acceptance.json']);
    assert.equal(row.evidence[10].configuration.receipt_sha256,
      older['verification/adapt15-final-source/acceptance.json']);
    assert.equal(row.evidence[11].configuration.receipt_sha256,
      older['verification/adapt15-release-source/acceptance.json']);
    assert.equal(row.evidence[12].configuration.receipt_sha256,
      older['verification/wi03-current-source/acceptance.json']);
    assert.equal(row.evidence[13].configuration.receipt_sha256,
      older['verification/wi03-release-source/acceptance.json']);
  }
  return {historical,editor,forged,initial,bounded,accepted,artifact,quota,authorization,
    resource,callback,admission,historicalDx,secondDx,secondReplay,
    priorReleaseDx,priorReleaseReplay,wi03,previous,failedDx,lastRelease};
}
export function inventory(mode,editor,forged) {
  assert.ok(mode==='dx06'||mode==='replay');
  const admission=JSON.parse(read('verification/adapt15/acceptance.json'));
  const wi03=JSON.parse(read('verification/wi03-final/acceptance.json'));
  const family=JSON.parse(read('specs/spec-family.json'));
  const input=[...precedingInputs('replay',editor,forged),...Object.keys(older),
    ...Object.keys(admission.source_sha256),
    ...Object.keys(wi03.source_sha256),
    'tools/cairn-specs.mjs','specs/spec-family.json',
    ...family.scenario_files.map(file=>`specs/${file}`),
    'specs/WIT-WASI.md',
    ...quotaChangeSources(),
    ...fsChangeSources(),
    ...resourceChangeSources(),
    ...callbackChangeSources(),
    ...mc1ProofSources(),
    ...own,...inherited,
    'verification/scase02/gate.mjs',
    'verification/scase02/corrected-gate.mjs',
    'verification/scase02/corrected-postpromotion.mjs',
    'verification/scase07/gate.mjs',
    'verification/scase07/postpromotion.mjs',
    'verification/scase13/gate.mjs',
    'verification/scase13/postpromotion.mjs',
    'verification/scase13/corrected-gate.mjs',
    'verification/scase13/corrected-postpromotion.mjs',
    'verification/scase06/gate.mjs',
    'verification/scase06/postpromotion.mjs',
    'verification/scase06/corrected-gate.mjs',
    'verification/scase06/corrected-postpromotion.mjs',
    'verification/scase06/final-gate.mjs',
    'verification/scase06/final-postpromotion.mjs',
    'verification/scase04/gate.mjs',
    'verification/scase04/postpromotion.mjs',
    'verification/scase08/gate.mjs',
    '.cairn/specs/resource-adapters/spec.md',
    '.cairn/specs/evidence/spec.md',
    ...(mode==='replay'?[dxReceipt]:[])];
  const sources=sourceInventory([...new Set(input)],trees);
  sources[fsTasks]=sourceDigest(fsTasks);
  sources[resourceTasks]=sourceDigest(resourceTasks);
  sources[callbackTasks]=sourceDigest(callbackTasks);
  sources[wi03Tasks]=sourceDigest(wi03Tasks);
  return sources;
}
