#!/usr/bin/env node
// Rule-mutation check for the L1 report attribution (issue #75).
//
// Disables one attribution rule of crates/noble-ngraph-experiment/src/l1/mod.rs
// at a time, runs the fixture-child tests (`extract_b2` module `attribution`),
// and records which tests fail. A rule whose removal fails no test and does not
// hang is reported as silent. The source file is edited in place and restored
// byte-for-byte on exit, error or signal; the restore is verified by SHA-256.
//
// Usage (repository root, clean tree, pinned toolchain on PATH):
//   node verification/ngraph/l1-attribution-mutations.mjs --cargo "$(command -v cargo)" \
//     --out /abs/mutations.json [--timeout-seconds 120]

import crypto from 'node:crypto';
import fs from 'node:fs';
import { spawnSync } from 'node:child_process';

const SCHEMA = 'noble-ngraph-l1-attribution-mutations/v1';
const FILE = 'crates/noble-ngraph-experiment/src/l1/mod.rs';
const FILTER = 'attribution::';

// [rule, anchor, replacement]: each anchor must occur exactly once.
const MUTATIONS = [
  ['ordinal', 'if report.submission != Some(ordinal) {', 'if false && report.submission != Some(ordinal) {'],
  ['source digest', 'if runtime.source_sha256 != expect.source_sha256 {', 'if false && runtime.source_sha256 != expect.source_sha256 {'],
  ['opt leg', 'if runtime.optimized != opt.optimized() {', 'if false && runtime.optimized != opt.optimized() {'],
  ['definition role', 'if expect.role == Role::Definition && report.outcome == "normal" {', 'if false && expect.role == Role::Definition && report.outcome == "normal" {'],
  ['input/root role', '(expect.role == Role::Input || expect.role == Role::Root) && report.outcome == "defined"', 'false && (expect.role == Role::Input || expect.role == Role::Root) && report.outcome == "defined"'],
  ['truncation', 'if reports.len() < submitted && !ended_early {', 'if false && reports.len() < submitted && !ended_early {'],
  ['after terminal', 'if terminal(&last.outcome) {', 'if false && terminal(&last.outcome) {'],
  ['exit status', 'if finish.exit_code != Some(explained) {', 'if false && finish.exit_code != Some(explained) {'],
  ['delivery', 'if finish.write_error.is_some() && !ended_early {', 'if false && finish.write_error.is_some() && !ended_early {'],
  ['kill on violation', 'violation = Some(match child.kill() {', 'violation = Some(match Ok::<(), std::io::Error>(()) {'],
];

function args(argv) {
  const out = { timeoutSeconds: 120 };
  for (let i = 0; i < argv.length; i += 2) {
    const [flag, value] = [argv[i], argv[i + 1]];
    if (value === undefined) throw new Error(`missing value for ${flag}`);
    if (flag === '--cargo') out.cargo = value;
    else if (flag === '--out') out.out = value;
    else if (flag === '--timeout-seconds') out.timeoutSeconds = Number(value);
    else throw new Error(`unknown flag ${flag}`);
  }
  if (!out.cargo || !out.out) throw new Error('--cargo and --out are required');
  if (!Number.isInteger(out.timeoutSeconds) || out.timeoutSeconds < 1) throw new Error('--timeout-seconds must be a positive integer');
  if (fs.existsSync(out.out)) throw new Error(`refusing to overwrite ${out.out}`);
  return out;
}

const sha256 = (bytes) => crypto.createHash('sha256').update(bytes).digest('hex');
const git = (...a) => spawnSync('git', a, { encoding: 'utf8' }).stdout.trim();

function run(opts) {
  const result = spawnSync('timeout', [String(opts.timeoutSeconds), opts.cargo, 'test', '--offline', '--locked',
    '-p', 'noble-ngraph-experiment', '--test', 'extract_b2', FILTER], { encoding: 'utf8', maxBuffer: 64 << 20 });
  const output = `${result.stdout}${result.stderr}`;
  const named = (verdict) => [...output.matchAll(new RegExp(`^test ${FILTER}(\\S+) \\.\\.\\. ${verdict}$`, 'gm'))].map((m) => m[1]);
  return {
    passed: named('ok'),
    failed: named('FAILED'),
    timed_out: result.status === 124,
    compiled: !/error\[E\d+\]|could not compile/.test(output),
  };
}

const opts = args(process.argv.slice(2));
if (git('status', '--porcelain', '--untracked-files=no') !== '') throw new Error('tracked tree is not clean');
const original = fs.readFileSync(FILE);
const originalSha = sha256(original);
const text = original.toString('utf8');
for (const [rule, anchor] of MUTATIONS) {
  if (text.split(anchor).length !== 2) throw new Error(`anchor for ${rule} must occur exactly once`);
}
const restore = () => {
  fs.writeFileSync(FILE, original);
  if (sha256(fs.readFileSync(FILE)) !== originalSha) throw new Error(`restore of ${FILE} failed`);
};
for (const signal of ['SIGINT', 'SIGTERM', 'SIGHUP']) {
  process.on(signal, () => {
    restore();
    process.exit(130);
  });
}

const baseline = run(opts);
if (!baseline.compiled || baseline.failed.length !== 0 || baseline.timed_out || baseline.passed.length === 0) {
  throw new Error(`unmutated tests must pass: ${JSON.stringify(baseline)}`);
}
const mutations = [];
try {
  for (const [rule, anchor, replacement] of MUTATIONS) {
    fs.writeFileSync(FILE, text.replace(anchor, replacement));
    const observed = run(opts);
    mutations.push({ rule, anchor, compiled: observed.compiled, failed: observed.failed, timed_out: observed.timed_out });
    console.error(JSON.stringify(mutations.at(-1)));
  }
} finally {
  restore();
}
const silent = mutations.filter((m) => m.compiled && m.failed.length === 0 && !m.timed_out).map((m) => m.rule);
const uncompiled = mutations.filter((m) => !m.compiled).map((m) => m.rule);
const receipt = {
  schema: SCHEMA,
  source_revision: git('rev-parse', 'HEAD'),
  file: FILE,
  file_sha256: originalSha,
  toolchain: spawnSync(opts.cargo.replace(/cargo$/, 'rustc'), ['-V'], { encoding: 'utf8' }).stdout.trim(),
  test_filter: `extract_b2 ${FILTER}`,
  timeout_seconds: opts.timeoutSeconds,
  unmutated_passed: baseline.passed,
  mutations,
  silent,
  uncompiled,
  result: silent.length === 0 && uncompiled.length === 0 ? 'passed' : 'failed',
};
fs.writeFileSync(opts.out, `${JSON.stringify(receipt, null, 2)}\n`);
console.log(`${opts.out} ${receipt.result}: ${mutations.length} mutations, silent ${JSON.stringify(silent)}`);
