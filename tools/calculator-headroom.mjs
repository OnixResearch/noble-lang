#!/usr/bin/env bun
// Calculator compile-cost headroom check over real `noble calculator` stage
// chains. Every guest module the host compiles appends one
// NOBLE_CALCULATOR_TRACE JSONL record whose `compile` object carries the
// compiler's own metrics and caps (crates/noble-cli/src/calculator/metrics.rs).
// This tool compares each record against the caps and flags any value whose
// remaining headroom is under a configurable margin.
//
//   bun tools/calculator-headroom.mjs run --noble BIN --out FRESH_DIR [--jobs N]
//       [--vector NAME]... [--margin NAME=N[:warn|fail]]...
//   bun tools/calculator-headroom.mjs check TRACE.jsonl... [--margin ...]
//
// Exit 0: no `fail` margin breached (warnings are printed). Exit 1: a `fail`
// margin was breached. Exit 2: usage error, or a vector did not produce the
// expected calculator output or any trace record.
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { spawn } from 'node:child_process';

// headroom = cap - observed value, per compile record.
export const METRICS = {
  work_issued: c => c.caps.work_issued - c.work_issued,
  type_size: c => c.caps.type_size - c.max_type_size,
  environment_rows: c => c.caps.environment_rows - c.environment_rows,
  named_bodies: c => c.caps.named_bodies - c.named_bodies,
  signature_keys: c => c.caps.signature_keys - c.signature_keys,
  table_entries: c => c.caps.table_entries - c.table_end,
  wat_bytes: c => c.caps.wat_bytes - c.wat_bytes,
  text_bytes: c => c.caps.text_end - c.text_end,
};

// The 100M Work margin is the one the formatter/evaluator splits kept; the
// others warn well before their caps.
export const DEFAULT_MARGINS = {
  work_issued: { margin: 100_000_000, severity: 'fail' },
  type_size: { margin: 8, severity: 'warn' },
  environment_rows: { margin: 64, severity: 'warn' },
  named_bodies: { margin: 64, severity: 'warn' },
  signature_keys: { margin: 1024, severity: 'warn' },
  table_entries: { margin: 2048, severity: 'warn' },
  wat_bytes: { margin: 512 * 1024, severity: 'warn' },
  text_bytes: { margin: 64 * 1024, severity: 'warn' },
};

// Fixed vector set: each vector is one calculator process (one session).
export const VECTORS = {
  add: { lines: ['1+2'], stdout: '3\n', status: 0 },
  juxtaposed: { lines: ['1 2'], stdout: '', status: 1 },
  call: { lines: ['fn f(y)=y*2', 'f(3)'], stdout: '6\n', status: 0 },
  power: { lines: ['2^10'], stdout: '1024\n', status: 0 },
  display: { lines: ['1/8'], args: ['--finite-decimal'], stdout: '0.125\n', status: 0 },
  fraction: { lines: ['22/7'], stdout: '22/7\n', status: 0 },
  // Growth of per-session state across replayed definitions. Every line
  // replays each earlier definition, so this vector runs for hours; it is
  // selected only with --vector long_session.
  long_session: {
    lines: ['let a=1', 'let b=a+2', 'fn g(x)=x*a+b', 'let c=g(3)', 'g(c)-a', 'a+b+c'],
    stdout: '8\n10\n',
    status: 0,
  },
};
export const DEFAULT_VECTORS = ['add', 'juxtaposed', 'call', 'power', 'display', 'fraction'];

export function parseMargin(text, margins) {
  const match = /^([a-z_]+)=(\d+)(?::(warn|fail))?$/.exec(text);
  if (!match || !(match[1] in METRICS)) throw new Error(`invalid --margin ${text}`);
  const previous = margins[match[1]];
  return {
    ...margins,
    [match[1]]: { margin: Number(match[2]), severity: match[3] ?? previous.severity },
  };
}

export function parseTrace(text) {
  const records = [];
  for (const line of text.split('\n')) {
    if (line.trim() === '') continue;
    const record = JSON.parse(line);
    if (record.schema !== 'noble-calculator-trace/1') {
      throw new Error(`unsupported trace schema ${record.schema}`);
    }
    if (record.record === 'module' && record.compile) records.push(record);
  }
  return records;
}

// Lowest headroom per metric over every compile record, plus each breach of
// a configured margin. Status is the worst breached severity.
export function evaluate(records, margins = DEFAULT_MARGINS) {
  const minima = {};
  for (const record of records) {
    for (const [name, measure] of Object.entries(METRICS)) {
      const headroom = measure(record.compile);
      if (!Number.isFinite(headroom)) throw new Error(`record lacks ${name}`);
      if (!(name in minima) || headroom < minima[name].headroom) {
        minima[name] = {
          headroom,
          session: record.session,
          line: record.line,
          phase: record.phase,
          module: record.module,
          source: record.source,
        };
      }
    }
  }
  const findings = [];
  for (const [name, { margin, severity }] of Object.entries(margins)) {
    const minimum = minima[name];
    if (minimum && minimum.headroom < margin) {
      findings.push({ metric: name, severity, margin, ...minimum });
    }
  }
  const status = findings.some(f => f.severity === 'fail')
    ? 'fail'
    : findings.length > 0 ? 'warn' : 'pass';
  return { compiles: records.length, minima, findings, status };
}

// Per candidate session (one worker and compiler, which replays the accepted
// definitions and then runs one submitted line): growth of the monotonic
// static Text cursor and of the per-preparation retained-key charge.
export function sessions(records) {
  const bySession = new Map();
  for (const record of records) {
    const c = record.compile;
    const entry = bySession.get(record.session) ?? {
      session: record.session,
      line: record.line,
      replayed_modules: 0,
      modules: 0,
      text_start: c.caps.text_start,
      text_end: c.caps.text_start,
      max_signature_keys: 0,
      max_signature_key_charge: 0,
      max_table_end: 0,
      max_wat_bytes: 0,
      program_free: 0,
      quiescent: 0,
      normal: 0,
    };
    entry.modules += 1;
    if (record.phase === 'replay') entry.replayed_modules += 1;
    entry.text_end = Math.max(entry.text_end, c.text_end);
    entry.max_signature_keys = Math.max(entry.max_signature_keys, c.signature_keys);
    entry.max_signature_key_charge = Math.max(entry.max_signature_key_charge, c.signature_key_charge);
    entry.max_table_end = Math.max(entry.max_table_end, c.table_end);
    entry.max_wat_bytes = Math.max(entry.max_wat_bytes, c.wat_bytes);
    entry[record.epoch] = (entry[record.epoch] ?? 0) + 1;
    bySession.set(record.session, entry);
  }
  return [...bySession.values()].map(e => ({ ...e, text_bytes: e.text_end - e.text_start }));
}

// How much of the issued Work is the environment allowance, and the lowest
// Work headroom if that allowance covered only the stage-independent scans
// (environment_work × 3) instead of environment_work × (named + 3).
export function environmentShare(records) {
  let issued = 0;
  let environment = 0;
  let worst = null;
  for (const record of records) {
    const c = record.compile;
    issued += c.work_issued;
    environment += c.work_environment_grant;
    const perBody = c.environment_work * c.named_bodies;
    const headroom = c.caps.work_issued - c.work_issued;
    if (!worst || headroom < worst.headroom) {
      worst = {
        source: record.source,
        named_bodies: c.named_bodies,
        environment_rows: c.environment_rows,
        environment_work: c.environment_work,
        work_environment_grant: c.work_environment_grant,
        work_issued: c.work_issued,
        headroom,
        headroom_without_per_body_scans: headroom + perBody,
      };
    }
  }
  return { issued, environment, share: issued === 0 ? 0 : environment / issued, worst };
}

export function summarize(result) {
  const lines = [`status: ${result.status} (${result.compiles} compiles)`];
  for (const [name, minimum] of Object.entries(result.minima)) {
    lines.push(`  ${name.padEnd(17)} min headroom ${String(minimum.headroom).padStart(11)}`
      + `  session ${minimum.session} line ${minimum.line} module ${minimum.module}: ${minimum.source}`);
  }
  for (const finding of result.findings) {
    lines.push(`${finding.severity.toUpperCase()}: ${finding.metric} headroom ${finding.headroom}`
      + ` < margin ${finding.margin} (${finding.source})`);
  }
  return lines.join('\n');
}

function runVector(binary, name, vector, directory) {
  const trace = path.join(directory, `${name}.jsonl`);
  const started = Date.now();
  return new Promise((resolve, reject) => {
    const child = spawn(binary, ['calculator', ...(vector.args ?? [])], {
      env: { ...process.env, NOBLE_CALCULATOR_TRACE: trace },
      stdio: ['pipe', 'pipe', 'pipe'],
    });
    let stdout = '';
    let stderr = '';
    child.stdout.on('data', chunk => { stdout += chunk; });
    child.stderr.on('data', chunk => { stderr += chunk; });
    child.on('error', reject);
    child.on('close', status => resolve({
      name, lines: vector.lines, args: vector.args ?? [], status, stdout, stderr,
      seconds: (Date.now() - started) / 1000, trace,
    }));
    child.stdin.end(vector.lines.map(line => `${line}\n`).join(''));
  });
}

async function run(options) {
  const binary = path.resolve(options.noble);
  fs.mkdirSync(options.out);
  const names = options.vectors.length > 0 ? options.vectors : DEFAULT_VECTORS;
  for (const name of names) if (!(name in VECTORS)) throw new Error(`unknown vector ${name}`);
  const pending = [...names];
  const runs = [];
  const worker = async () => {
    while (pending.length > 0) {
      const name = pending.shift();
      runs.push(await runVector(binary, name, VECTORS[name], options.out));
    }
  };
  await Promise.all(Array.from({ length: options.jobs }, worker));
  runs.sort((a, b) => names.indexOf(a.name) - names.indexOf(b.name));
  let broken = false;
  const vectors = [];
  const all = [];
  for (const outcome of runs) {
    const expected = VECTORS[outcome.name];
    const records = fs.existsSync(outcome.trace)
      ? parseTrace(fs.readFileSync(outcome.trace, 'utf8'))
      : [];
    const ok = outcome.status === expected.status && outcome.stdout === expected.stdout
      && records.length > 0;
    broken ||= !ok;
    all.push(...records);
    vectors.push({
      name: outcome.name, lines: outcome.lines, args: outcome.args, ok,
      status: outcome.status, stdout: outcome.stdout, stderr: outcome.stderr,
      seconds: outcome.seconds,
      ...evaluate(records, options.margins),
      sessions: sessions(records),
      environment: environmentShare(records),
    });
  }
  const overall = evaluate(all, options.margins);
  const report = {
    schema: 'noble-calculator-headroom/1',
    binary_sha256: crypto.createHash('sha256').update(fs.readFileSync(binary)).digest('hex'),
    margins: options.margins,
    status: broken ? 'broken' : overall.status,
    overall: { ...overall, environment: environmentShare(all) },
    vectors,
  };
  fs.writeFileSync(path.join(options.out, 'report.json'), `${JSON.stringify(report, null, 2)}\n`);
  for (const vector of vectors) {
    process.stdout.write(`== ${vector.name} ${JSON.stringify(vector.lines)} exit ${vector.status}`
      + ` ${vector.ok ? 'ok' : 'UNEXPECTED OUTPUT'} ${vector.seconds}s\n${summarize(vector)}\n`);
  }
  process.stdout.write(`== overall\n${summarize(overall)}\n`);
  if (broken) return 2;
  return overall.status === 'fail' ? 1 : 0;
}

function check(options) {
  const records = options.traces.flatMap(file => parseTrace(fs.readFileSync(file, 'utf8')));
  if (records.length === 0) throw new Error('no compile records');
  const result = evaluate(records, options.margins);
  process.stdout.write(`${summarize(result)}\n`);
  process.stdout.write(`${JSON.stringify({ sessions: sessions(records), environment: environmentShare(records) }, null, 2)}\n`);
  return result.status === 'fail' ? 1 : 0;
}

function parseArguments(argv) {
  const options = { command: argv[0], margins: DEFAULT_MARGINS, vectors: [], traces: [], jobs: 1 };
  for (let at = 1; at < argv.length; at += 1) {
    const word = argv[at];
    const value = () => {
      at += 1;
      if (at >= argv.length) throw new Error(`missing ${word} value`);
      return argv[at];
    };
    if (word === '--margin') options.margins = parseMargin(value(), options.margins);
    else if (word === '--noble') options.noble = value();
    else if (word === '--out') options.out = value();
    else if (word === '--vector') options.vectors.push(value());
    else if (word === '--jobs') options.jobs = Math.max(1, Math.min(3, Number(value())));
    else if (word.startsWith('--')) throw new Error(`unknown option ${word}`);
    else options.traces.push(word);
  }
  return options;
}

if (import.meta.main) {
  let status;
  try {
    const options = parseArguments(process.argv.slice(2));
    if (options.command === 'run' && options.noble && options.out) status = await run(options);
    else if (options.command === 'check' && options.traces.length > 0) status = check(options);
    else throw new Error('usage: calculator-headroom.mjs run --noble BIN --out FRESH_DIR | check TRACE...');
  } catch (error) {
    process.stderr.write(`calculator-headroom: ${error.message}\n`);
    status = 2;
  }
  process.exit(status);
}
