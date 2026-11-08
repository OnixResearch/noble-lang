// Issue #82, step 1: the exact section, import and instruction inventory of the pilot's subject
// artifact, which a Lean Wasm semantics must cover. The artifact bytes are bound by their sha256 in
// the retained #57 receipt, and the inventory is read from them with the pinned wasm-tools.
// Reporting only: this checks no semantics.
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const receiptDir = 'verification/owi/receipts/2026-10-26';
const artifact = `${receiptDir}/subject-module-1.wasm`;
const requireThat = (condition, message) => { if (!condition) throw new Error(message); };
const sha256 = bytes => createHash('sha256').update(bytes).digest('hex');

function run(tool, argv) {
  const result = spawnSync(tool, argv, { cwd: root, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024, env: {} });
  requireThat(result.status === 0, `${path.basename(tool)} ${argv[0]} failed: ${result.stderr}`);
  return result.stdout;
}

/** `name | start - end | bytes | count` rows of `wasm-tools objdump`. */
function sections(objdump) {
  return objdump.split('\n').filter(line => line.includes('|')).map(line => {
    const [name, , size, count] = line.split('|').map(field => field.trim());
    return { section: name, bytes: Number(size.split(' ')[0]), count: Number(count.split(' ')[0]) };
  });
}

/** Mnemonic occurrences in the function bodies of the flat `wasm-tools print` text. */
function instructions(printed) {
  const counts = {};
  let inFunction = false;
  for (const raw of printed.split('\n')) {
    const line = raw.trim();
    if (/^\(func\b/.test(line)) { inFunction = true; continue; }
    if (/^ {2}\(/.test(raw)) inFunction = false;
    if (!inFunction || line === '' || line.startsWith('(') || line.startsWith(';;') || line === ')') continue;
    const mnemonic = line.split(/\s+/)[0].replace(/\)+$/, '');
    if (mnemonic && !/^[$\d]/.test(mnemonic)) counts[mnemonic] = (counts[mnemonic] ?? 0) + 1;
  }
  return Object.fromEntries(Object.entries(counts).sort(([a], [b]) => a.localeCompare(b)));
}

/** Import kinds and value types of the flat `wasm-tools print` text. */
function imports(printed) {
  const rows = printed.split('\n').map(line => line.trim()).filter(line => line.startsWith('(import '));
  const kind = row => row.match(/\((memory|table|global|func|tag)\b/)?.[1] ?? 'unknown';
  const counts = {};
  for (const row of rows) {
    const key = kind(row) === 'global' ? `global ${row.includes('(mut ') ? 'mut ' : ''}${row.match(/\b(i32|i64|f32|f64)\)/)?.[1]}` : kind(row);
    counts[key] = (counts[key] ?? 0) + 1;
  }
  return { total: rows.length, by_kind: counts };
}

function main() {
  const output = process.argv[2] === '--output' ? path.resolve(process.argv[3]) : null;
  const selection = JSON.parse(fs.readFileSync(path.join(root, 'policy/tool-selection.json'), 'utf8'));
  const wasmTools = path.join(selection.tool_paths.wasm_tools.output, 'bin/wasm-tools');
  const bytes = fs.readFileSync(path.join(root, artifact));
  const digest = sha256(bytes);
  const retained = fs.readFileSync(path.join(root, receiptDir, 'sha256sums.txt'), 'utf8')
    .split('\n').find(line => line.endsWith(` ${artifact}`))?.split(/\s+/)[0];
  requireThat(retained === digest, `artifact digest ${digest} is not the retained ${retained}`);
  const printed = run(wasmTools, ['print', artifact]);
  const opcodes = instructions(printed);
  const receipt = {
    schema: 'noble-wasm-pilot-inventory/v1', issue: 82,
    artifact, sha256: digest, bytes: bytes.length,
    tool: run(wasmTools, ['--version']).trim(),
    sections: sections(run(wasmTools, ['objdump', artifact])),
    imports: imports(printed),
    exports: printed.split('\n').filter(line => line.trim().startsWith('(export ')).length,
    instruction_count: Object.values(opcodes).reduce((sum, count) => sum + count, 0),
    distinct_opcodes: Object.keys(opcodes).length,
    opcodes,
    method: 'mnemonic occurrences in the function bodies of `wasm-tools print` (flat text); `end` counts block ends, not function ends',
  };
  const text = `${JSON.stringify(receipt, null, 2)}\n`;
  if (output) fs.writeFileSync(output, text, { flag: 'wx' });
  else process.stdout.write(text);
}

try {
  main();
} catch (error) {
  console.error(`[wasm-pilot-inventory] FAIL ${error.message}`);
  process.exitCode = 1;
}
