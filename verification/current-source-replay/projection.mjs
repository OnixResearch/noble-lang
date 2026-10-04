import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';

const sha = value => crypto.createHash('sha256').update(value).digest('hex');
const receiptPath = 'verification/current-source-replay/acceptance.json';
const affected = {
  'specs/conformance/developer-experience-cases.json': ['DX-01'],
  'specs/conformance/adaptation-cases.json': ['ADAPT-01'],
  'specs/conformance/safety-cases.json': ['S-CASE-01','S-CASE-03','S-CASE-05','S-CASE-14'],
};

// Locate only the named case's evidence array, respecting nested JSON arrays,
// objects, strings and escapes. Keeping the original bytes rather than
// reserializing JSON is necessary to prove the earlier full-file source hash.
export function removeSecondEvidence(text, id) {
  const escaped = id.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const match = [...text.matchAll(new RegExp(`"id"\\s*:\\s*"${escaped}"`, 'g'))];
  assert.equal(match.length, 1, `${id} unique canonical row`);
  const key = text.indexOf('"evidence":', match[0].index);
  assert.ok(key >= 0, `${id} evidence key`);
  const open = text.indexOf('[', key + '"evidence":'.length);
  assert.ok(open >= 0, `${id} evidence array`);
  let arrays = 0, objects = 0, quoted = false, backslash = false, separator = -1;
  for (let index = open; index < text.length; index++) {
    const ch = text[index];
    if (quoted) {
      if (backslash) backslash = false;
      else if (ch === '\\') backslash = true;
      else if (ch === '"') quoted = false;
      continue;
    }
    if (ch === '"') quoted = true;
    else if (ch === '[') arrays++;
    else if (ch === ']') {
      arrays--;
      if (arrays === 0) {
        assert.ok(separator > open, `${id} requires exactly two evidence objects`);
        assert.equal(JSON.parse(text.slice(open,index+1)).length, 2, `${id} evidence count`);
        return text.slice(0, separator) + text.slice(index);
      }
    } else if (ch === '{') objects++;
    else if (ch === '}') objects--;
    else if (ch === ',' && arrays === 1 && objects === 0) {
      assert.equal(separator, -1, `${id} has extra evidence object`);
      separator = index;
    }
  }
  throw Error(`${id} unterminated evidence array`);
}

// Before the new replay is accepted, leave the historical projection intact.
// After promotion, validate the one extra *current-source* receipt record for
// every selected case, remove only its bytes, and recover the frozen
// pre-replay full-file hash before any older promotion is reversed.
export function restoreReplayEvidence(promoted, caseFile) {
  const ids = affected[caseFile];
  assert.ok(ids, `unselected replay case file: ${caseFile}`);
  const packet = JSON.parse(promoted);
  const rows = ids.map(id => {
    const row = packet.cases.find(item => item.id === id);
    assert.ok(row && row.state.execution === 'passed' && row.state.proof === 'open', `${id} state`);
    assert.ok(Array.isArray(row.evidence) && [1,2].includes(row.evidence.length), `${id} evidence count`);
    return row;
  });
  if (rows.every(row => row.evidence.length === 1)) return promoted;
  assert.ok(rows.every(row => row.evidence.length === 2), `${caseFile} partial replay promotion`);
  const receiptBytes = fs.readFileSync(receiptPath);
  const receipt = JSON.parse(receiptBytes);
  const digest = sha(receiptBytes);
  assert.equal(receipt.result,'passed');
  assert.equal(receipt.kind,'test');
  assert.equal(receipt.schema,'noble-current-source-replay/v1');
  const external = rows[0].evidence[1].configuration.external_raw_output;
  assert.equal(sha(fs.readFileSync(path.join(external,'acceptance.json'))),digest,
    'external frozen replay receipt differs from the portable copy');
  for (const row of rows) {
    const evidence = row.evidence[1];
    const captured = receipt.cases.find(item => item.id === row.id);
    assert.ok(captured, `${row.id} absent from replay receipt`);
    assert.deepEqual({id:row.id,input:row.input,expected:row.expected},
      {id:captured.id,input:captured.input,expected:captured.expected});
    assert.equal(evidence.subject,row.id);
    assert.equal(evidence.kind,'test');
    assert.equal(evidence.result,'passed');
    assert.equal(evidence.revision,packet.revision);
    assert.ok(typeof evidence.claim === 'string' && evidence.claim.trim());
    assert.equal(evidence.source_revision,receipt.source_revision);
    assert.deepEqual(evidence.assumptions,receipt.assumptions,
      `${row.id} current evidence must retain the exact receipt assumptions`);
    assert.equal(evidence.configuration.case_id,row.id);
    assert.equal(evidence.configuration.receipt,
      path.posix.relative(path.posix.dirname(caseFile),receiptPath));
    assert.equal(evidence.configuration.receipt_sha256,digest);
    assert.equal(evidence.configuration.external_raw_output,external);
    assert.equal(evidence.configuration.prepromotion_case_sha256,receipt.prepromotion_case_sha256[caseFile]);
    assert.equal(evidence.configuration.binary_sha256,receipt.binary.sha256);
    assert.equal(receipt.source_sha256[captured.previous_receipt],captured.previous_receipt_sha256);
    assert.equal(row.evidence[0].configuration.receipt,
      path.posix.relative(path.posix.dirname(caseFile),captured.previous_receipt));
  }
  let restored = promoted;
  for (const id of [...ids].reverse()) restored = removeSecondEvidence(restored,id);
  assert.equal(sha(restored),receipt.prepromotion_case_sha256[caseFile],
    `replay promotion changed historical case bytes outside appended evidence: ${caseFile}`);
  return restored;
}
