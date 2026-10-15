#!/usr/bin/env node
// Compact one or more run.mjs receipts into the committed summary. Each plan
// session is matched to the receipt session with the same calculator flags and
// stdin bytes, and is re-checked against the current plan. Raw stdin, stdout,
// stderr and timestamped events stay in each run's external directory.
import fs from 'node:fs';
import crypto from 'node:crypto';
import { expectations } from './check.mjs';

const [planPath, outPath, ...receiptPaths] = process.argv.slice(2);
if (!planPath || !outPath || receiptPaths.length === 0) {
  console.error('usage: summarize.mjs PLAN_JSON SUMMARY_JSON RECEIPT_JSON...');
  process.exit(2);
}
const sha256 = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const planBytes = fs.readFileSync(planPath);
const plan = JSON.parse(planBytes);
const receipts = receiptPaths.map(path => {
  const bytes = fs.readFileSync(path);
  return { bytes, receipt: JSON.parse(bytes) };
});
const binaries = new Set(receipts.map(({ receipt }) => receipt.binary.sha256));
if (binaries.size !== 1) {
  console.error('receipts name different binaries');
  process.exit(2);
}
const stdinOf = session => Buffer.concat(session.lines.map(line => Buffer.from(
  `${typeof line === 'string' ? line : line.repeat.repeat(line.count)}\n`, 'utf8')));
const flagsOf = argv => JSON.stringify(argv.slice(argv.indexOf('calculator') + 1));
const line = text => (text.length > 80 ? { bytes: Buffer.byteLength(text), prefix: text.slice(0, 40) } : text);
const sessions = plan.sessions.map(session => {
  const stdin = sha256(stdinOf(session));
  const found = receipts.flatMap(({ receipt }, index) => receipt.sessions.map(result => ({ result, index })))
    .filter(({ result }) => result.stdin_sha256 === stdin && flagsOf(result.argv) === JSON.stringify(session.flags)
      && (result.memory_max ?? null) === (session.memory_max ?? receipts[0].receipt.configuration.memory_max));
  if (found.length === 0) return { id: session.id, observed: false };
  const { result, index } = found[found.length - 1];
  const check = expectations(session, result);
  return {
    id: session.id,
    category: session.category,
    receipt: index,
    argv: ['noble', ...result.argv.slice(result.argv.indexOf('calculator'))],
    memory_max: result.memory_max,
    timeout_s: result.timeout_s,
    stdin_lines: session.lines.map(item => (typeof item === 'string' ? item : { repeat: item.repeat, count: item.count })),
    stdin_sha256: stdin,
    expected: session.expect,
    stdout_lines: check.stdout_lines,
    stderr_lines: check.stderr_lines.map(line),
    status: result.status,
    signal: result.signal,
    timed_out: result.timed_out,
    wall_s: Math.round(result.wall_ms / 100) / 10,
    max_rss_kib: result.timing?.max_rss_kib ?? null,
    resource_limit_lines: check.resource_limit_lines.length,
    matched_plan: check.matched,
  };
});
const summary = {
  schema: 'noble-calculator-budget-summary/v1',
  status: 'implementation-observation-not-case-evidence',
  case: 'CALC-08',
  binary: { sha256: [...binaries][0], source_id: receipts[0].receipt.binary.source_id },
  plan_sha256: sha256(planBytes),
  receipts: receipts.map(({ bytes, receipt }) => ({ sha256: sha256(bytes), plan_sha256: receipt.plan_sha256,
    runner_sha256: receipt.runner_sha256, node: receipt.node, repository_head: receipt.repository.head,
    configuration: receipt.configuration })),
  non_claims: plan.non_claims,
  sessions,
};
fs.writeFileSync(outPath, `${JSON.stringify(summary, null, 1)}\n`);
const unmatched = sessions.filter(session => session.observed !== false && !session.matched_plan).map(session => session.id);
if (unmatched.length > 0) console.error(`not matching plan: ${unmatched.join(', ')}`);
