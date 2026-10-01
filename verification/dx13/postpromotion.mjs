import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';

// The original review receipt binds the complete prepromotion case file.
// Promotion may replace only the final DX-13 state/evidence block. Recreate
// its input byte projection rather than pretending the changed ledger is the
// original source or adding a self-hash of the receipt to itself.
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const receiptBytes = readFileSync('verification/dx13/acceptance.json');
const receipt = JSON.parse(receiptBytes);
const casePath = 'specs/conformance/language-workflow-cases.json';
const promoted = readFileSync(casePath, 'utf8');
const caseRow = JSON.parse(promoted).cases.find(row => row.id === 'DX-13');
if (!caseRow || caseRow.state.execution !== 'passed' || caseRow.state.proof !== 'not-applicable' ||
    caseRow.evidence.length !== 1 || caseRow.evidence[0].kind !== 'review' ||
    caseRow.evidence[0].configuration.receipt_sha256 !== hash(receiptBytes)) {
  throw Error('DX-13 review evidence/state does not match retained receipt');
}
const start = promoted.indexOf('      "state": ', promoted.indexOf('"id": "DX-13"'));
const end = promoted.indexOf('\n    }\n  ]', start);
if (start < 0 || end < 0 || promoted.indexOf('"id": "DX-13"', start) !== -1) {
  throw Error('DX-13 last-case boundary changed');
}
const prepromotion = promoted.slice(0, start) +
  '      "state": {"implementation": "absent", "execution": "not-run", "proof": "not-applicable", "trust": "unassessed"}, "evidence": []' +
  promoted.slice(end);
if (hash(prepromotion) !== receipt.inputs_sha256.case ||
    hash(readFileSync('verification/dx13/review.json')) !== receipt.inputs_sha256.review ||
    hash(readFileSync('verification/dx13/review.mjs')) !== receipt.inputs_sha256.runner ||
    hash(readFileSync('.cairn/specs/developer-experience/spec.md')) !== receipt.inputs_sha256.native ||
    hash(readFileSync('specs/DECISIONS.md')) !== receipt.inputs_sha256.decisions) {
  throw Error('prepromotion source projection or immutable methodology changed');
}
const status = JSON.parse(readFileSync('specs/STATUS.json', 'utf8')).scoped_bend_qcue_research;
if (status.unexecuted_cases.includes('DX-13') || !status.unexecuted_cases.includes('CONTRACT-16') ||
    status.finite_oracle_execution !== 'not-run' || status.universal_proof_or_backend_claim !== false) {
  throw Error('review promotion conflated execution, owner-law or proof status');
}
console.log(JSON.stringify({result:'passed', case:'DX-13', receipt_sha256:hash(receiptBytes),
  prepromotion_case_sha256:hash(prepromotion), postpromotion_case_sha256:hash(promoted),
  review_only:true, owner_law_open:true, actual_oracle_execution:false}));
