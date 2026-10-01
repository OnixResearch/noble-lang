#!/usr/bin/env node
// Verify the promoted S-CASE-07 record against its immutable receipt, retained
// external raw output and exact pre-promotion case bytes. Read-only.
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const file = name => fs.readFileSync(path.join(root, name));
const sha = value => crypto.createHash('sha256').update(value).digest('hex');
const caseFile = 'specs/conformance/safety-cases.json';
const bytes = file('verification/scase07/acceptance.json');
const receipt = JSON.parse(bytes);
assert.equal(receipt.schema, 'noble-scase07-artifact-correspondence/v1');
assert.equal(receipt.kind, 'test');
assert.equal(receipt.result, 'passed');

const text = file(caseFile).toString('utf8');
const lines = text.split('\n').filter(line => line.includes('"id":"S-CASE-07"'));
assert.equal(lines.length, 1);
const row = JSON.parse(lines[0].trim().replace(/,$/, ''));
assert.equal(lines[0], `    ${JSON.stringify(row)},`);
assert.deepEqual(Object.keys(row), ['id', 'profile', 'kind', 'requirements', 'input', 'expected', 'state', 'evidence']);
assert.deepEqual({id:row.id, profile:row.profile, kind:row.kind, requirements:row.requirements,
  input:row.input, expected:row.expected}, receipt.case);
assert.deepEqual(row.state, {implementation:'implemented', execution:'passed', proof:'open', trust:'explicit'});
assert.equal(row.evidence.length, 1);
const [evidence] = row.evidence;
assert.equal(evidence.subject, 'S-CASE-07');
assert.equal(evidence.kind, 'test');
assert.equal(evidence.result, 'passed');
assert.equal(evidence.source_revision, receipt.source_revision);
const configuration = evidence.configuration;
assert.equal(configuration.receipt, '../../verification/scase07/acceptance.json');
assert.equal(configuration.receipt_sha256, sha(bytes));
assert.equal(configuration.gate, '../../verification/scase07/gate.mjs');
assert.equal(configuration.gate_sha256, sha(file('verification/scase07/gate.mjs')));
assert.equal(configuration.gate_sha256, receipt.source_sha256['verification/scase07/gate.mjs']);
assert.equal(configuration.prepromotion_case_sha256, receipt.prepromotion_case_sha256);
assert.equal(configuration.binary_sha256, receipt.binary_sha256);
assert.equal(configuration.external_raw_output, receipt.external_raw_output);

// The only canonical byte change is this row's promotion.
const prior = {...row, state:{implementation:'absent', execution:'not-run', proof:'open', trust:'unassessed'},
  evidence:[]};
const priorLine = `    ${JSON.stringify(prior)},`;
assert.equal(sha(priorLine), receipt.prepromotion_case_line_sha256);
assert.equal(sha(text.replace(lines[0], priorLine)), receipt.prepromotion_case_sha256);
for (const [name, digest] of Object.entries(receipt.source_sha256)) {
  if (name !== caseFile) assert.equal(sha(file(name)), digest, `changed bound source: ${name}`);
}
for (const [name, digest] of Object.entries(receipt.historical_receipts_sha256)) {
  assert.equal(sha(file(name)), digest, `historical receipt changed: ${name}`);
}

// Retained external raw output and exact canonical observation.
const external = receipt.external_raw_output;
assert.equal(sha(fs.readFileSync(path.join(external, 'acceptance.json'))), sha(bytes));
const exits = new Map(receipt.rows.map(item => [item.label, item.exit]));
for (const command of receipt.commands) {
  assert.equal(command.status, exits.get(command.label) ?? 0, command.label);
  assert.equal(command.signal, null, command.label);
  assert.equal(command.error, null, command.label);
  assert.equal(sha(fs.readFileSync(path.join(external, command.stdout))), command.stdout_sha256, command.label);
  assert.equal(sha(fs.readFileSync(path.join(external, command.stderr))), command.stderr_sha256, command.label);
}
const canonical = receipt.rows.find(item => item.label === 'canonical-scase07');
assert.equal(canonical.role, 'canonical');
assert.equal(canonical.exit, 2);
for (const [field, value] of Object.entries(row.expected)) assert.deepEqual(canonical.report[field], value, field);
assert.equal(canonical.report.diagnostic, 'final artifact bytes differ from independent host-selected compilation');
const transcript = JSON.parse(fs.readFileSync(path.join(external,
  receipt.commands.find(command => command.label === 'canonical-scase07').stdout), 'utf8'));
assert.deepEqual(transcript, canonical.report);
assert.deepEqual(receipt.corroboration.trace, []);
assert.equal(receipt.corroboration.protected_operations, 0);
assert.equal(receipt.corroboration.compilations, 0);
assert.equal(receipt.corroboration.memory_unchanged, true);
assert.equal(receipt.corroboration.table_unchanged, true);
console.log(JSON.stringify({result:'passed', case_id:row.id, source_revision:receipt.source_revision,
  receipt_sha256:sha(bytes), prepromotion_case_sha256:receipt.prepromotion_case_sha256,
  commands:receipt.commands.length, counts:receipt.counts}));
