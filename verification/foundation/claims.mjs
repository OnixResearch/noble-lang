//! Claim-table completeness audit (issue #15 closure criterion, owner decision
//! 2026-10-26).
//!
//! The criterion: every claim-table row in `proofs/foundation/README.md` names
//! either a kernel-checked theorem set with a receipt or an explicitly labelled
//! open, tested or assumed obligation, and no receipt claims implementation
//! correctness.
//!
//! This audit is a document check. It does not check proofs, and a passing
//! audit is not a correctness claim about any implementation.
import { readFileSync } from 'node:fs';
import path from 'node:path';

const ROOT = path.resolve(import.meta.dirname, '..', '..');
const read = file => readFileSync(path.join(ROOT, file), 'utf8');

const ALLOWED_STATUS = ['proved', 'tested', 'assumed', 'open'];
const MARKER = /^\|/u;

function claimTables(markdown) {
  const rows = [];
  let header = null;
  for (const line of markdown.split('\n')) {
    if (!MARKER.test(line)) {
      header = null;
      continue;
    }
    const cells = line.replace(/^\||\|$/gu, '').split('|').map(cell => cell.trim());
    if (cells.every(cell => /^-+$/u.test(cell))) continue;
    if (header === null) {
      header = cells;
      continue;
    }
    if (header.length !== cells.length) continue;
    const row = Object.fromEntries(header.map((name, index) => [name, cells[index]]));
    if (!('Status' in row)) continue;
    rows.push(row);
  }
  return rows;
}

// Theorem identifiers the README names: backticked identifiers, with call
// notation stripped, plus dotted projections into a module.
function namedTheorems(cell) {
  const names = [];
  for (const match of cell.matchAll(/`([^`]+)`/gu)) {
    const raw = match[1].replace(/\(.*\)$/u, '').trim();
    if (!/^[A-Za-z][A-Za-z0-9_.]*$/u.test(raw)) continue;
    names.push(raw.split('.').at(-1));
  }
  return names;
}

const readme = read('proofs/foundation/README.md');
const inventory = JSON.parse(read('proofs/foundation/inventory.json'));
const inventoried = new Set(inventory.theorems.map(entry => entry.name.split('.').at(-1)));

// Names the README cites that are specification identifiers rather than
// inventoried theorems. Each entry states why, so a real typo still fails.
const DOCUMENT_ONLY = new Map([
  ['acceptLiteral', 'Phase-1 literal-acceptance specification function, transcribed and proved through accumulate_eq_acceptLiteral'],
  ['ArithSpec', 'Phase-1 arithmetic specification structure whose fields add_spec/sub_spec/mul_spec are inventoried'],
]);

const rows = claimTables(readme).filter(row => row['Claim'] !== undefined);
const diagnostics = [];
let provedRows = 0;
let openRows = 0;
let testedRows = 0;
let assumedRows = 0;
const missing = new Set();
for (const row of rows) {
  const status = row['Status'].toLowerCase();
  const words = ALLOWED_STATUS.filter(word => status.includes(word));
  if (words.length === 0) {
    diagnostics.push(`row "${row['Claim'].slice(0, 60)}" has no allowed status word`);
    continue;
  }
  const modelColumn = row['Model / theorem(s)'];
  if (modelColumn === undefined) {
    diagnostics.push(`row "${row['Claim'].slice(0, 60)}" has no model column`);
    continue;
  }
  const named = namedTheorems(modelColumn);
  const placeholderOnly = named.length === 0 && /^—|-$/u.test(modelColumn);
  if (placeholderOnly && !words.includes('open')) {
    diagnostics.push(
      `row "${row['Claim'].slice(0, 60)}" names no theorem while claiming ${words.join('/')}`,
    );
    continue;
  }
  for (const name of named) {
    if (!inventoried.has(name) && !DOCUMENT_ONLY.has(name)) missing.add(name);
  }
  if (words.includes('proved')) provedRows += 1;
  if (words.includes('open')) openRows += 1;
  if (words.includes('tested')) testedRows += 1;
  if (words.includes('assumed')) assumedRows += 1;
}

for (const name of missing) {
  diagnostics.push(
    `named theorem ${name} is neither inventoried nor a declared specification identifier`,
  );
}

const receipts = inventory.theorems.length;
const receiptClaims = [];
for (const directory of ['proofs/foundation/inventory.json']) {
  const text = read(directory);
  if (text.includes('"implementation_theorem": true')) {
    receiptClaims.push(`${directory} claims an implementation theorem`);
  }
}

const result = {
  schema: 'noble-fclaims-audit/v1',
  criterion: 'claim-table completeness: every claim-table row names kernel-checked theorems or an explicitly labelled open, tested or assumed obligation; no receipt claims implementation correctness',
  owner_decision: '2026-10-26',
  audit_is_document_check_only: true,
  tables: { rows: rows.length, proved: provedRows, tested: testedRows, assumed: assumedRows, open: openRows },
  inventoried_theorems: receipts,
  document_only_names: Object.fromEntries(DOCUMENT_ONLY),
  diagnostics: [...diagnostics, ...receiptClaims],
  result: diagnostics.length === 0 && receiptClaims.length === 0 ? 'passed' : 'failed',
};
process.stdout.write(`${JSON.stringify(result, null, 2)}\n`);
if (result.result !== 'passed') process.exitCode = 1;
