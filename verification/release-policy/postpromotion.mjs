import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { restoreHandlePromotion } from '../safety-core/handle-postpromotion.mjs';

const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const receiptBytes = readFileSync('verification/release-policy/acceptance.json');
const receipt = JSON.parse(receiptBytes);
const review = readFileSync('verification/release-policy/review.json');
const runner = readFileSync('verification/release-policy/review.mjs');
if (receipt.result !== 'passed' || receipt.kind !== 'review' ||
    hash(review) !== receipt.inputs_sha256.review ||
    hash(runner) !== receipt.inputs_sha256.runner) throw Error('immutable review inputs changed');
const cases = [
  ['WI-17', 'witCase', 'specs/conformance/wit-wasi-cases.json'],
  ['S-CASE-15', 'safetyCase', 'specs/conformance/safety-cases.json'],
];
function restoreUnexecuted(content, id) {
  const lineStart = content.lastIndexOf('\n', content.indexOf(`"id":"${id}"`)) + 1;
  const stateStart = content.indexOf('"state":', lineStart);
  const lineEnd = content.indexOf('\n', stateStart);
  if (lineStart < 1 || stateStart < lineStart || lineEnd < 0 ||
      content.slice(stateStart, lineEnd).indexOf('"evidence":') < 0 ||
      content.slice(stateStart, lineEnd).indexOf('"id":') >= 0) {
    throw Error(`${id} one-line case boundary changed`);
  }
  return content.slice(0, stateStart) +
    '"state":{"implementation":"absent","execution":"not-run","proof":"' +
    (id === 'S-CASE-15' || id === 'WI-17' ? 'not-applicable' : 'open') +
    '","trust":"unassessed"},"evidence":[]},' + content.slice(lineEnd);
}
for (const [id, key, path] of cases) {
  const promoted = id === 'S-CASE-15' ?
    restoreHandlePromotion(readFileSync(path, 'utf8')) : readFileSync(path, 'utf8');
  const row = JSON.parse(promoted).cases.find(item => item.id === id);
  if (row?.kind !== 'review' || row.state.implementation !== 'implemented' ||
      row.state.execution !== 'passed' || row.state.proof !== 'not-applicable' ||
      row.state.trust !== 'explicit' || row.evidence.length !== 1 ||
      row.evidence[0].kind !== 'review' || row.evidence[0].result !== 'passed' ||
      row.evidence[0].subject !== id ||
      row.evidence[0].configuration.receipt_sha256 !== hash(receiptBytes) ||
      row.evidence[0].configuration.prepromotion_case_sha256 !== receipt.inputs_sha256[key]) {
    throw Error(`${id} state/evidence not bound to prepromotion receipt`);
  }
  let beforeHistoricalReview = promoted;
  if (id === 'S-CASE-15') {
    const laterIds = ['S-CASE-01', 'S-CASE-05', 'S-CASE-14'];
    const laterRows = JSON.parse(promoted).cases.filter(item => laterIds.includes(item.id) &&
      item.state.execution === 'passed');
    if (laterRows.length > 0) {
      const laterBytes = readFileSync('verification/safety-core/acceptance.json');
      const laterReceipt = JSON.parse(laterBytes);
      if (laterRows.length !== laterIds.length ||
          laterReceipt.result !== 'passed' || laterReceipt.kind !== 'test') {
        throw Error('partial or unauthenticated later safety promotion');
      }
      for (const laterId of laterIds) {
        const later = laterRows.find(item => item.id === laterId);
        if (later?.state.proof !== 'open' || later.evidence.length !== 1 ||
            later.evidence[0].subject !== laterId ||
            later.evidence[0].configuration.receipt_sha256 !== hash(laterBytes)) {
          throw Error(`${laterId} later safety evidence does not match its receipt`);
        }
        beforeHistoricalReview = restoreUnexecuted(beforeHistoricalReview, laterId);
      }
      if (hash(beforeHistoricalReview) !== laterReceipt.prepromotion_case_sha256) {
        throw Error('later safety case projection does not reconstruct its prepromotion input');
      }
    }
  }
  const prepromotion = restoreUnexecuted(beforeHistoricalReview, id);
  if (hash(prepromotion) !== receipt.inputs_sha256[key]) {
    throw Error(`${id} changed outside review state/evidence since prepromotion`);
  }
}
console.log(JSON.stringify({result:'passed',cases:receipt.cases,
  receipt_sha256:hash(receiptBytes),prepromotion_sha256:{
    'WI-17':receipt.inputs_sha256.witCase,'S-CASE-15':receipt.inputs_sha256.safetyCase},
  review_only:true, release_implemented:false, proof_discharged:false}));
