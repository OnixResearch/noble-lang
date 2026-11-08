// #102: export a minimised `source` fuzz corpus as seed cases for the semantic differential runner
// (`verification/foundation/differential.mjs --import`). Only programs the runner can judge are
// kept:
// - a single-unit Core subject (profiles 0 and 1, so no module, calculator or host words);
// - whose tokens lie in the runner's fragment (I64 and Bool literals, `+ - *`, quotation blocks,
//   `quote`, `compose`, `run`);
// - whose reference run is normal.
// Every other corpus input is counted under the reason it was skipped. The runner executes each
// kept program's exact corpus text, so an importer modeling error shows up as a mismatch instead of
// being hidden by a re-rendered source.
//
// Usage: SELECTED_NODE verification/fuzz/export-differential.mjs --corpus MINIMISED_SOURCE_CORPUS \
//          --output NEW_CASES_JSON
import { createHash } from 'node:crypto';
import { existsSync, readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { inRange, refRun } from '../foundation/arithmetic.mjs';

// The input layout of `crates/noble-wasm/tests/fuzz/oracles.rs`.
const HEADER = 5;
const SELECT = 4;
const DEFAULT_LIMITS = 0b11;
const CORE_PROFILES = new Set([0, 1]); // Profile::Core and Profile::CoreWithoutHosts, by byte % 4

const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const requireThat = (condition, message) => { if (!condition) throw Error(message); };

/** Core tokens, following the source lexer: space, tab, CR and LF separate; `#` comments run to the
end of the line; brackets are tokens of their own; a word ends at whitespace, `#`, a bracket or a
quote. Returns null for text literals, which lie outside the fragment. */
function tokenize(text) {
  const tokens = [];
  let at = 0;
  const whitespace = ch => ch === ' ' || ch === '\t' || ch === '\r' || ch === '\n';
  while (at < text.length) {
    const ch = text[at];
    if (whitespace(ch)) { at += 1; continue; }
    if (ch === '#') { while (at < text.length && text[at] !== '\r' && text[at] !== '\n') at += 1; continue; }
    if (ch === '"') return null;
    if (ch === '[' || ch === ']') { tokens.push(ch); at += 1; continue; }
    const start = at;
    while (at < text.length && !whitespace(text[at]) && !'#[]"'.includes(text[at])) at += 1;
    tokens.push(text.slice(start, at));
  }
  return tokens;
}

/** A decimal I64 literal as the lexer reads it: an optional `-`, then digits with no leading zero
unless there is only one. Out-of-range literals parse here and are refused by the reference. */
const isInteger = token => /^-?(0|[1-9][0-9]*)$/.test(token);
const isBool = token => token === 'true' || token === 'false';
const ARITH = { '+': 'add', '-': 'sub', '*': 'mul' };

function op(token) {
  if (isInteger(token)) return { push: BigInt(token) };
  return ARITH[token] ?? null;
}

/** PExpr flattening, as `differential.mjs` and `NobleFoundation.PExpr.flat` define it. */
function flat(program) {
  if (program.arith) return program.arith;
  if (program.quoted !== undefined) return [{ push: program.quoted }];
  return [...flat(program.compose[0]), ...flat(program.compose[1])];
}

function caseOps(c) {
  if (c.kind === 'straight') return c.ops;
  if (c.kind === 'program') return [...c.ins, ...flat(c.program)];
  return [...c.ins, ...c.ops];
}

function opsBetween(tokens) {
  const ops = tokens.map(op);
  return ops.includes(null) ? null : ops;
}

/** The runner's case model for a token list, or null when the program lies outside it. Every Bool
literal must belong to the untouched prefix, which ends with the last one; the operations after it
start from an empty reference stack. */
function parse(tokens) {
  let prefixEnd = 0;
  tokens.forEach((token, index) => { if (isBool(token)) prefixEnd = index + 1; });
  const prefix = [];
  for (const token of tokens.slice(0, prefixEnd)) {
    if (isBool(token)) prefix.push(['Bool', token]);
    else if (isInteger(token) && inRange(BigInt(token))) prefix.push(['I64', String(BigInt(token))]);
    else return null;
  }
  const rest = tokens.slice(prefixEnd);
  if (!rest.some(token => ['[', ']', 'quote', 'compose', 'run'].includes(token))) {
    const ops = opsBetween(rest);
    return ops && { kind: 'straight', prefix, ops };
  }
  if (rest.at(-1) !== 'run') return null;
  let at = 0;
  const ins = [];
  while (at < rest.length && isInteger(rest[at]) && rest[at + 1] !== 'quote') ins.push({ push: BigInt(rest[at++]) });
  const tail = rest.slice(at);
  // `ins [ ops ] quote run run`: a quoted quotation, run twice.
  if (tail.length >= 5 && tail[0] === '[' && tail.slice(-4).join(' ') === '] quote run run') {
    const ops = opsBetween(tail.slice(1, -4));
    return ops && { kind: 'nested', prefix, ins, ops };
  }
  // `ins P run` for a PExpr P built from `[ ops ]`, `n quote` and `P Q compose`.
  const stack = [];
  let index = 0;
  while (index < tail.length - 1) {
    const token = tail[index];
    if (token === '[') {
      const close = tail.indexOf(']', index);
      if (close < 0) return null;
      const ops = opsBetween(tail.slice(index + 1, close));
      if (!ops) return null;
      stack.push({ arith: ops });
      index = close + 1;
    } else if (isInteger(token) && tail[index + 1] === 'quote') {
      stack.push({ quoted: BigInt(token) });
      index += 2;
    } else if (token === 'compose') {
      const second = stack.pop();
      const first = stack.pop();
      if (!first || !second) return null;
      stack.push({ compose: [first, second] });
      index += 1;
    } else {
      return null;
    }
  }
  return stack.length === 1 ? { kind: 'program', prefix, ins, program: stack[0] } : null;
}

const plain = value => JSON.parse(JSON.stringify(value, (_, x) => typeof x === 'bigint' ? x.toString() : x));

function main() {
  const args = process.argv.slice(2);
  const option = name => { const i = args.indexOf(name); return i >= 0 ? args[i + 1] : undefined; };
  const corpus = option('--corpus');
  const output = option('--output');
  requireThat(corpus && output && args.length === 4,
    'usage: export-differential.mjs --corpus MINIMISED_SOURCE_CORPUS --output NEW_CASES_JSON');
  requireThat(!existsSync(resolve(output)), 'the output file must not exist yet');
  const names = readdirSync(resolve(corpus)).sort();
  const counts = { files: names.length, short: 0, module_or_calculator_profile: 0, multi_unit: 0,
    non_utf8: 0, empty: 0, outside_fragment: 0, reference_refusal: 0, duplicate: 0, imported: 0 };
  const seen = new Set();
  const cases = [];
  for (const name of names) {
    const data = readFileSync(resolve(corpus, name));
    if (data.length < HEADER) { counts.short += 1; continue; }
    const profile = data[SELECT] % 4;
    if (!CORE_PROFILES.has(profile)) { counts.module_or_calculator_profile += 1; continue; }
    const body = data.subarray(HEADER);
    if (body.includes(0)) { counts.multi_unit += 1; continue; }
    let text;
    try { text = new TextDecoder('utf-8', { fatal: true }).decode(body); } catch { counts.non_utf8 += 1; continue; }
    const tokens = tokenize(text);
    if (tokens && tokens.length === 0) { counts.empty += 1; continue; }
    const parsed = tokens && parse(tokens);
    if (!parsed) { counts.outside_fragment += 1; continue; }
    if (refRun(caseOps(parsed), []) === null) { counts.reference_refusal += 1; continue; }
    const key = tokens.join(' ');
    if (seen.has(key)) { counts.duplicate += 1; continue; }
    seen.add(key);
    counts.imported += 1;
    cases.push({ ...plain(parsed), origin: { unit: name, sha256: digest(data), profile,
      budget: (data[0] & DEFAULT_LIMITS) === DEFAULT_LIMITS ? 'default' : 'tiny', text } });
  }
  const exported = {
    schema: 'noble-fuzz-differential-cases/v1',
    corpus: { units: names.length, names_sha256: digest(Buffer.from(names.join('\n'))) },
    counts, cases,
  };
  writeFileSync(resolve(output), `${JSON.stringify(exported, null, 2)}\n`);
  process.stdout.write(`${JSON.stringify({ output: resolve(output), counts })}\n`);
}

try {
  main();
} catch (error) {
  console.error(`[fuzz-export] FAIL ${error.message}`);
  process.exitCode = 1;
}
