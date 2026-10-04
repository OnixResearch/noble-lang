import assert from 'node:assert/strict';
import { test } from 'node:test';
import { removeSecondEvidence } from './projection.mjs';

const historical = {claim:'a historical ] , \\"quoted\\" observation',
  configuration:{rows:[{ids:['a,b',']']}]},assumptions:['proof remains open']};
const current = {claim:'one exact new source-bound observation',
  configuration:{receipt:'../../verification/current-source-replay/acceptance.json'},
  assumptions:['no universal theorem']};

for (const [id,spacing] of [['DX-01','"id": "DX-01", "evidence": '],
  ['ADAPT-01','"id": "ADAPT-01", "evidence": '],
  ['S-CASE-03','"id":"S-CASE-03","evidence":']]) {
  test(`${id} removes only the appended second evidence object and restores exact bytes`, () => {
    const old = JSON.stringify(historical);
    const addition = JSON.stringify(current);
    const before = `{"cases":[{${spacing}[${old}]}],"unrelated":"untouched"}\n`;
    const after = `{"cases":[{${spacing}[${old}, ${addition}]}],"unrelated":"untouched"}\n`;
    assert.equal(removeSecondEvidence(after,id),before);
    assert.deepEqual(JSON.parse(after).cases[0].evidence,[historical,current]);
  });
}

test('projection rejects three evidence objects, not just the second', () => {
  const text = `{"cases":[{"id":"DX-01","evidence":[${JSON.stringify(historical)},` +
    `${JSON.stringify(current)},${JSON.stringify(current)}]}]}\n`;
  assert.throws(() => removeSecondEvidence(text,'DX-01'),/extra evidence object/);
});

test('projection rejects ambiguous duplicate selected cases', () => {
  const row = `{"id":"S-CASE-03","evidence":[${JSON.stringify(historical)},${JSON.stringify(current)}]}`;
  assert.throws(() => removeSecondEvidence(`{"cases":[${row},${row}]}`,'S-CASE-03'),
    /unique canonical row/);
});
