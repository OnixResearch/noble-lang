#!/usr/bin/env node
// Emits the #19-runner vector file for the seeded-fault lane: one probe per
// CALC-12 incorrect implementation, with expectations taken from the
// independent oracle (never from the cases file or a fault model). Each probe
// is chosen so that the exact oracle and exactly its own fault model differ;
// the script refuses to emit a probe that fails that check.
//
//   node fault-vectors.mjs --out NEW_FILE [--fault NAME,...]
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { parseArgs } from 'node:util';
import { JUDGED_SPAN_BASES, run } from '../oracle/calculator.mjs';
import { FAULT_NAMES, runFault } from '../oracle/faults.mjs';

export const PROBES = Object.freeze({
  'integer-division': ['7 / 2'],
  'binary-float-decimal-parser': ['0.1'],
  'I64-wrapping': ['9223372036854775807 + 1'],
  'reversed-subtraction': ['10 - 3'],
  'left-associated-power': ['2^3^2'],
  'non-normalized-rational': ['2 / 4'],
  'failure-mutates-session': ['let x = 20', 'let x = 1 / 0', 'x'],
});

const expectOf = outcome => {
  if (outcome.kind === 'value') return { value: outcome.display };
  if (outcome.kind === 'silent') return { silent: true };
  return { diagnostic: { category: outcome.category, span: JUDGED_SPAN_BASES.has(outcome.basis) ? outcome.span : null } };
};
const same = (a, b) => JSON.stringify([a.stdout, a.stderr.map(d => d.category), a.status])
  === JSON.stringify([b.stdout, b.stderr.map(d => d.category), b.status]);

export const faultVectors = faults => faults.map(fault => {
  const lines = PROBES[fault];
  assert.ok(lines, `no probe for ${fault}`);
  const exact = run(lines);
  const predicted = Object.fromEntries(FAULT_NAMES.map(name => [name, runFault(name, lines)]));
  assert.ok(!same(exact, predicted[fault]), `${fault}: probe does not separate the fault from the oracle`);
  return {
    id: `CALC-12/fault/${fault}`, case: 'CALC-12-seeded-fault', kind: lines.length > 1 ? 'session' : 'line',
    support: 'cli', flags: [],
    operations: lines.map((line, index) => ({ line, expect: expectOf(exact.outcomes[index]) })),
    fault_prediction: { stdout: predicted[fault].stdout, categories: predicted[fault].stderr.map(d => d.category) },
    also_separates: FAULT_NAMES.filter(name => name !== fault && !same(exact, predicted[name])),
  };
});

if (import.meta.url === `file://${process.argv[1]}`) {
  const { values } = parseArgs({ options: { out: { type: 'string' }, fault: { type: 'string' } } });
  assert.ok(values.out, '--out NEW_FILE');
  const out = path.resolve(values.out);
  assert.ok(!fs.existsSync(out), 'output must be fresh');
  const faults = values.fault ? values.fault.split(',') : FAULT_NAMES;
  fs.writeFileSync(out, `${JSON.stringify({ schema: 'noble-calculator-vectors/v1', vectors: faultVectors(faults) }, null, 2)}\n`);
  console.log(`${faults.length} probe vectors`);
}
