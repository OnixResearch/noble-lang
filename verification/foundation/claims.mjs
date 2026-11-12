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
import { readFileSync, readdirSync } from 'node:fs';
import path from 'node:path';

const ROOT = path.resolve(import.meta.dirname, '..', '..');
const read = file => readFileSync(path.join(ROOT, file), 'utf8');

const ALLOWED_STATUS = ['proved', 'tested', 'assumed', 'open'];
const MARKER = /^\|/u;

// Split one table row on unescaped pipes, so a cell may contain `\|` (a value
// like a norm) without shifting every later cell.
function cells(line) {
  const inner = line.trim().replace(/^\|/u, '').replace(/\|$/u, '');
  const out = [];
  let cell = '';
  for (let index = 0; index < inner.length; index += 1) {
    const char = inner[index];
    if (char === '\\' && inner[index + 1] === '|') {
      cell += '\\|';
      index += 1;
      continue;
    }
    if (char === '|') {
      out.push(cell.trim());
      cell = '';
      continue;
    }
    cell += char;
  }
  out.push(cell.trim());
  return out;
}

function claimTables(markdown) {
  const rows = [];
  let header = null;
  for (const line of markdown.split('\n')) {
    if (!MARKER.test(line)) {
      header = null;
      continue;
    }
    const row = cells(line);
    if (row.every(cell => /^-+$/u.test(cell))) continue;
    if (header === null) {
      header = row;
      continue;
    }
    if (header.length !== row.length) continue;
    const record = Object.fromEntries(header.map((name, index) => [name, row[index]]));
    if (!('Status' in record)) continue;
    rows.push(record);
  }
  return rows;
}

// Status words are whole words: `unproved` and `not proved` are not `proved`. A
// bolded phrase is the table's own label, so its words are matched exactly; a plain
// cell is split into words and matched the same way.
function statusWords(cell) {
  const bold = [...cell.matchAll(/\*\*([^*]+)\*\*/gu)].map(match => match[1]);
  const words = [];
  for (const candidate of bold.length > 0 ? bold : [cell]) {
    for (const word of candidate.toLowerCase().split(/[^a-z]+/u)) {
      if (ALLOWED_STATUS.includes(word) && !words.includes(word)) words.push(word);
    }
  }
  return words;
}

// Theorem identifiers the README names: backticked identifiers, with call
// notation stripped, plus dotted projections into a module. The name is kept
// qualified so a citation of another module's same-named theorem cannot pass.
function namedTheorems(cell) {
  const names = [];
  for (const match of cell.matchAll(/`([^`]+)`/gu)) {
    const raw = match[1].replace(/\(.*\)$/u, '').trim();
    if (!/^[A-Za-z][A-Za-z0-9_.]*$/u.test(raw)) continue;
    if (!names.includes(raw)) names.push(raw);
  }
  return names;
}

const readme = read('proofs/foundation/README.md');
const inventory = JSON.parse(read('proofs/foundation/inventory.json'));
const inventoried = new Set(inventory.theorems.map(entry => entry.name));
// A citation matches a qualified inventory name exactly, or as one of its dotted
// suffixes after the root module (`CheckedI64.checkedAdd_ok_iff`,
// `checkedAdd_ok_iff`); `Other.Module.real_theorem` is not a suffix of anything.
const knownNames = new Set();
for (const name of inventoried) {
  const parts = name.split('.');
  for (let start = 1; start < parts.length; start += 1) knownNames.add(parts.slice(start).join('.'));
  knownNames.add(name);
}

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
  const words = statusWords(row['Status']);
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
    if (!knownNames.has(name) && !DOCUMENT_ONLY.has(name)) missing.add(name);
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

// The implementation-correctness criterion is about receipts: read every retained
// foundation receipt, which is the only place the flag is recorded.
const receiptDirectory = path.join(ROOT, 'verification/foundation/receipts');
const receiptClaims = [];
let receiptsChecked = 0;
for (const entry of readdirSync(receiptDirectory, { withFileTypes: true })) {
  if (!entry.isDirectory()) continue;
  const file = path.join(receiptDirectory, entry.name, 'foundation-check.json');
  let text;
  try {
    text = readFileSync(file, 'utf8');
  } catch {
    continue;
  }
  receiptsChecked += 1;
  if (JSON.parse(text).implementation_theorem === true) {
    receiptClaims.push(`verification/foundation/receipts/${entry.name}/foundation-check.json claims an implementation theorem`);
  }
}
if (inventory.implementation_theorem === true) {
  receiptClaims.push('proofs/foundation/inventory.json claims an implementation theorem');
}

const result = {
  schema: 'noble-fclaims-audit/v1',
  criterion: 'claim-table completeness: every claim-table row names kernel-checked theorems or an explicitly labelled open, tested or assumed obligation; no receipt claims implementation correctness',
  owner_decision: '2026-10-26',
  audit_is_document_check_only: true,
  tables: { rows: rows.length, proved: provedRows, tested: testedRows, assumed: assumedRows, open: openRows },
  inventoried_theorems: inventoried.size,
  receipts_checked: receiptsChecked,
  document_only_names: Object.fromEntries(DOCUMENT_ONLY),
  diagnostics: [...diagnostics, ...receiptClaims],
  result: diagnostics.length === 0 && receiptClaims.length === 0 ? 'passed' : 'failed',
};
process.stdout.write(`${JSON.stringify(result, null, 2)}\n`);
if (result.result !== 'passed') process.exitCode = 1;
