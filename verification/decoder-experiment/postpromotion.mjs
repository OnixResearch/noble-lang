import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { restoreReplayEvidence } from '../current-source-replay/projection.mjs';

const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const raw = restoreReplayEvidence(readFileSync('specs/conformance/adaptation-cases.json','utf8'),
  'specs/conformance/adaptation-cases.json');
const cases = JSON.parse(raw).cases;
const runtimeBytes = readFileSync('verification/decoder-experiment/acceptance.json');
const reviewBytes = readFileSync('verification/decoder-experiment/adapt07-review.json');
const runtime = JSON.parse(runtimeBytes), review = JSON.parse(reviewBytes);
const nativeBytes = readFileSync('verification/decoder-experiment/adapt13-review-final.json');
const snapshotBytes = readFileSync('verification/decoder-experiment/adapt14-review-final.json');
const native = JSON.parse(nativeBytes), snapshot = JSON.parse(snapshotBytes);
for (const [id, bytes, kind] of [
  ['ADAPT-04',runtimeBytes,'test'],['ADAPT-05',runtimeBytes,'test'],['ADAPT-07',reviewBytes,'review']
]) {
  const row = cases.find(item => item.id === id);
  if (row?.state.execution !== 'passed' || row.evidence.length !== 1 ||
      row.evidence[0].kind !== kind || row.evidence[0].configuration.receipt_sha256 !== hash(bytes)) {
    throw Error(`${id} promoted state/evidence does not bind exact receipt`);
  }
}
// Exact prior case-file bytes are retained as frozen source snapshots. The
// shared ADAPT-01 row can subsequently evolve without erasing old receipts.
const predecoderBytes = readFileSync('verification/decoder-experiment/predecoder-cases.json');
const prereviewBytes = readFileSync('verification/decoder-experiment/prereview-cases.json');
const prenativeBytes = readFileSync('verification/decoder-experiment/prenative-final-cases.json');
if (hash(predecoderBytes) !== runtime.source.input_sha256['specs/conformance/adaptation-cases.json'] ||
    hash(prereviewBytes) !== review.source.input_sha256['specs/conformance/adaptation-cases.json'] ||
    hash(prenativeBytes) !== native.source.input_sha256['specs/conformance/adaptation-cases.json'] ||
    hash(prenativeBytes) !== snapshot.source.input_sha256['specs/conformance/adaptation-cases.json']) {
  throw Error('frozen historical full-case source is not the receipt-bound original');
}
const undoReview = (text,id) => {
  const startCase = text.indexOf(`"id": "${id}"`);
  const start = text.indexOf('      "state": ',startCase);
  const end = text.indexOf('\n    },',start);
  if (startCase < 0 || start < 0 || end < 0) throw Error(`${id} review projection missing`);
  return text.slice(0,start) +
    '      "state": {"implementation": "absent", "execution": "not-run", "proof": "not-applicable", "trust": "unassessed"}, "evidence": []' +
    text.slice(end);
};
const prenativeText = prenativeBytes.toString('utf8');
const previousAdapt01 = JSON.parse(prenativeText).cases.find(item => item.id === 'ADAPT-01');
const currentAdapt01 = cases.find(item => item.id === 'ADAPT-01');
if (!previousAdapt01 || !currentAdapt01 || currentAdapt01.evidence.length !== 1 ||
    !Array.isArray(currentAdapt01.evidence[0].assumptions) ||
    currentAdapt01.evidence[0].assumptions.length === 0 ||
    !currentAdapt01.evidence[0].assumptions.every(value => typeof value === 'string' && value.trim())) {
  throw Error('ADAPT-01 supplementary evidence assumptions missing');
}
const withoutAssumptions = structuredClone(currentAdapt01);
delete withoutAssumptions.evidence[0].assumptions;
if (JSON.stringify(withoutAssumptions) !== JSON.stringify(previousAdapt01)) {
  throw Error('ADAPT-01 frozen evidence differs in more than supplementary assumptions');
}
const restoreAdapt01Evidence = text => {
  const original = prenativeText;
  const sourceCase = original.indexOf('"id": "ADAPT-01"');
  const targetCase = text.indexOf('"id": "ADAPT-01"');
  const sourceStart = original.indexOf('      "evidence": ',sourceCase);
  const targetStart = text.indexOf('      "evidence": ',targetCase);
  const sourceEnd = original.indexOf('\n    },',sourceStart);
  const targetEnd = text.indexOf('\n    },',targetStart);
  if ([sourceCase,targetCase,sourceStart,targetStart,sourceEnd,targetEnd].some(index => index < 0)) {
    throw Error('ADAPT-01 evidence source projection missing');
  }
  return text.slice(0,targetStart) + original.slice(sourceStart,sourceEnd) + text.slice(targetEnd);
};
if (restoreAdapt01Evidence(undoReview(undoReview(raw,'ADAPT-14'),'ADAPT-13')) !== prenativeText) {
  throw Error('ordered ADAPT-13/14 postpromotion source projection does not recover exact prepromotion bytes');
}
const originalCases = JSON.parse(predecoderBytes).cases;
const ids = ['ADAPT-04','ADAPT-05','ADAPT-07','ADAPT-13','ADAPT-14'];
for (const id of ids) {
  const row = cases.find(item => item.id === id);
  const historical = originalCases.find(item => item.id === id);
  if (!row || !historical ||
      JSON.stringify([row.profile,row.kind,row.requirements,row.input,row.expected]) !==
      JSON.stringify([historical.profile,historical.kind,historical.requirements,historical.input,historical.expected])) {
    throw Error(`${id} no longer matches original receipt-bound case design`);
  }
}
for (const [path, sourceHash] of Object.entries(runtime.source.input_sha256)) {
  if (path !== 'specs/conformance/adaptation-cases.json' && hash(readFileSync(path)) !== sourceHash) {
    throw Error(`decoder source changed: ${path}`);
  }
}
for (const [path, sourceHash] of Object.entries(review.source.input_sha256)) {
  if (path !== 'specs/conformance/adaptation-cases.json' && hash(readFileSync(path)) !== sourceHash) {
    throw Error(`native review source changed: ${path}`);
  }
}
for (const receipt of [native,snapshot]) {
  for (const [path, sourceHash] of Object.entries(receipt.source.input_sha256)) {
    if (path !== 'specs/conformance/adaptation-cases.json' && hash(readFileSync(path)) !== sourceHash) {
      throw Error(`later review source changed: ${path}`);
    }
  }
}
if (runtime.cases['ADAPT-04'].zerocopy_crate !== 'unselected-not-run-not-passed' ||
    review.observation.canonical_schema_selected !== false || review.observation.dependency_selected !== false) {
  throw Error('unselected native encoding/dependency gained authority');
}
for (const [id, bytes] of [['ADAPT-13',nativeBytes],['ADAPT-14',snapshotBytes]]) {
  const row = cases.find(item => item.id === id);
  if (row.state.execution !== 'passed' || row.state.proof !== 'not-applicable' ||
      row.evidence.length !== 1 || row.evidence[0].kind !== 'review' ||
      row.evidence[0].configuration.receipt_sha256 !== hash(bytes) ||
      row.evidence[0].configuration.prepromotion_case_sha256 !== hash(prenativeBytes)) {
    throw Error(`${id} review promotion does not bind immutable source and receipt`);
  }
}
console.log(JSON.stringify({result:'passed',case_ids:ids,
  decoder_receipt_sha256:hash(runtimeBytes),review_receipt_sha256:hash(reviewBytes),
  native_receipt_sha256:hash(nativeBytes),snapshot_receipt_sha256:hash(snapshotBytes),
  frozen_predecoder_case_sha256:hash(predecoderBytes),
  frozen_prereview_case_sha256:hash(prereviewBytes),
  frozen_prenative_case_sha256:hash(prenativeBytes),
  original_full_case_bytes_verified:true,
  adapt01_only_supplementary_assumptions:true,
  optional_zerocopy_dependency:'unselected-not-run-not-passed',canonical_encoding:false}));
