#!/usr/bin/env node
/**
 * The `noble` host module for the Owi lane (issue #57).
 *
 * The committed subject imports one memory, one table and twenty-five mutable globals from module
 * module `noble`, and no host function, so `host/noble-host.wat` satisfies it. This lane validates
 * the subject imports and initial values against the ABI, including a zero-step-limit refusal.
 *
 * Usage (pinned Node and wasm-tools, env -i, from a clean checkout):
 *   node verification/owi/host.mjs --wasm-tools BIN --output NEW_FILE
 */
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const LANE = 'verification/owi/host.mjs';
const STUB = 'verification/owi/host/noble-host.wat';
const SUBJECT = 'verification/owi/receipts/2026-10-26/subject-module-1.wasm';
const DUMP = 'verification/owi/receipts/2026-10-26/subject-imports-exports.txt';
const ABI = 'crates/noble-cli/src/core/runtime/abi.json';
const sha = bytes => createHash('sha256').update(bytes).digest('hex');

function options() {
  const argv = process.argv.slice(2);
  assert.equal(argv.length, 4, 'usage: host.mjs --wasm-tools BIN --output NEW_FILE');
  const out = {};
  for (let at = 0; at < argv.length; at += 2) out[argv[at]] = argv[at + 1];
  assert.ok(path.isAbsolute(out['--wasm-tools']), '--wasm-tools must be absolute');
  assert.ok(!fs.existsSync(out['--output']), 'the output must not exist');
  assert.ok(!path.resolve(out['--output']).startsWith(ROOT + path.sep), 'the output must be outside the repository');
  return out;
}

function git(argv) {
  return spawnSync('/run/current-system/sw/bin/git', argv, { cwd: ROOT, encoding: 'utf8' }).stdout.trim();
}

/** Every `noble` import the subject declares, as `{ name, kind, detail }`. */
function imports() {
  const text = fs.readFileSync(path.join(ROOT, DUMP), 'utf8');
  const found = [];
  for (const line of text.split('\n')) {
    const match = line.match(/\(import "noble" "([^"]+)" \((memory|table|global) (.*?)\)\)\s*$/);
    if (!match) continue;
    const [, name, kind, rest] = match;
    const limits = rest.match(/^\(;0;\) (\d+) (\d+)/);
    const global = rest.match(/\(mut (i32|i64)\)/);
    found.push({ name, kind, limits: limits ? [Number(limits[1]), Number(limits[2])] : null,
      global: global ? global[1] : null });
  }
  return found;
}

function initialGlobals(file) {
  const text = fs.readFileSync(file, 'utf8');
  return Object.fromEntries([...text.matchAll(
    /\(global \(export "([^"]+)"\) \(mut i32\) \(i32\.const (-?\d+)\)\)/g,
  )].map(([, name, value]) => [name, Number(value)]));
}

/** The stub's exports and definitions, read back from `wasm-tools print`. */
function stubShape(binary, file) {
  const validated = spawnSync(binary, ['validate', file], { encoding: 'utf8' });
  assert.equal(validated.status, 0, `stub does not validate: ${validated.stderr}`);
  const printed = spawnSync(binary, ['print', file], { encoding: 'utf8' });
  assert.equal(printed.status, 0, `stub does not print: ${printed.stderr}`);
  const memory = [...printed.stdout.matchAll(/\(memory \(;0;\) (\d+) (\d+)\)/g)].map(m => [Number(m[1]), Number(m[2])]);
  const table = [...printed.stdout.matchAll(/\(table \(;0;\) (\d+) (\d+) funcref\)/g)].map(m => [Number(m[1]), Number(m[2])]);
  const globals = [...printed.stdout.matchAll(/\(global \(;\d+;\) \(mut (i32|i64)\)/g)].map(m => m[1]);
  return { printed: printed.stdout, memory, table, globals };
}

/** One stub's exports against the subject's imports; the empty list means a faithful host. */
function mismatches(binary, file) {
  const shape = stubShape(binary, file);
  const wanted = imports();
  const problems = [];
  const expectMemory = wanted.find(entry => entry.kind === 'memory');
  const expectTable = wanted.find(entry => entry.kind === 'table');
  if (!expectMemory || shape.memory.length !== 1 || shape.memory[0][0] !== expectMemory.limits[0]
    || shape.memory[0][1] !== expectMemory.limits[1]) problems.push('memory limits differ');
  if (!expectTable || shape.table.length !== 1 || shape.table[0][0] !== expectTable.limits[0]
    || shape.table[0][1] !== expectTable.limits[1]) problems.push('table limits differ');
  const wantedGlobals = wanted.filter(entry => entry.kind === 'global');
  const counts = kind => wantedGlobals.filter(entry => entry.global === kind).length;
  if (shape.globals.length !== wantedGlobals.length) problems.push(`global count ${shape.globals.length} is not ${wantedGlobals.length}`);
  for (const kind of ['i32', 'i64']) {
    const got = shape.globals.filter(entry => entry === kind).length;
    if (got !== counts(kind)) problems.push(`${kind} globals ${got} are not ${counts(kind)}`);
  }
  const exported = [...shape.printed.matchAll(/\(export "([^"]+)" \((memory|table|global) /g)].map(m => `${m[2]}:${m[1]}`);
  const imported = wanted.map(entry => `${entry.kind}:${entry.name}`);
  const missing = imported.filter(name => !exported.includes(name));
  const extra = exported.filter(name => !imported.includes(name));
  if (missing.length) problems.push(`missing exports: ${missing.join(', ')}`);
  if (extra.length) problems.push(`extra exports: ${extra.join(', ')}`);
  return { problems, memory: shape.memory, table: shape.table,
    globals: { i32: shape.globals.filter(k => k === 'i32').length, i64: shape.globals.filter(k => k === 'i64').length } };
}

function initialMismatches(file, limits) {
  const observed = initialGlobals(file);
  return { observed, problems: Object.entries(limits)
    .filter(([name, expected]) => observed[name] !== expected)
    .map(([name, expected]) => `${name} initial value ${observed[name]} is not ${expected}`) };
}

function main() {
  const flags = options();
  const binary = flags['--wasm-tools'];
  const source = { commit: git(['rev-parse', 'HEAD']), tree: git(['rev-parse', 'HEAD^{tree}']),
    clean: git(['status', '--porcelain', '--untracked-files=all']) === '' };
  const wanted = imports();
  assert.ok(wanted.length >= 27, `the dump declares every import: found ${wanted.length}`);
  assert.equal(wanted.filter(entry => entry.kind === 'global').length, 25);
  const stub = mismatches(binary, path.join(ROOT, STUB));
  assert.deepEqual(stub.problems, [], `stub is not a faithful host: ${stub.problems.join('; ')}`);
  const abi = JSON.parse(fs.readFileSync(path.join(ROOT, ABI), 'utf8'));
  const limits = Object.fromEntries(abi.globals.filter(entry => entry.name.endsWith('_limit'))
    .map(entry => [entry.name, Number(entry.value)]));
  assert.deepEqual(Object.keys(limits).sort(), ['allocation_limit','continuation_limit','depth_limit','operand_limit','recipe_limit','step_limit'].sort());
  const initial = initialMismatches(path.join(ROOT, STUB), limits);
  assert.deepEqual(initial.problems, [], `stub initial state differs from ABI: ${initial.problems.join('; ')}`);
  // A matching interface with a zeroed step limit is not a faithful initial state.
  const text = fs.readFileSync(path.join(ROOT, STUB), 'utf8');
  const mutated = text.replace('(global (export "step_limit") (mut i32) (i32.const 100000))',
    '(global (export "step_limit") (mut i32) (i32.const 0))');
  assert.notEqual(mutated, text, 'control zeroes step_limit');
  const control = path.join(process.env.TMPDIR ?? '', `noble-host-control-${process.pid}.wat`);
  assert.ok(path.isAbsolute(control) && process.env.TMPDIR, 'TMPDIR must be an absolute private directory');
  fs.writeFileSync(control, mutated, { flag: 'wx' });
  const refused = initialMismatches(control, limits);
  fs.rmSync(control);
  assert.ok(refused.problems.includes('step_limit initial value 0 is not 100000'), 'zero step_limit is refused');
  const receipt = {
    schema: 'noble-owi-host-receipt/v1',
    claim: 'the committed stub satisfies the committed subject\'s noble imports by name, kind, limits and global types',
    source: { ...source, lane_sha256: sha(fs.readFileSync(path.join(ROOT, LANE))) },
    stub: { path: STUB, sha256: sha(fs.readFileSync(path.join(ROOT, STUB))) },
    subject: { path: SUBJECT, sha256: sha(fs.readFileSync(path.join(ROOT, SUBJECT))),
      dump: DUMP, dump_sha256: sha(fs.readFileSync(path.join(ROOT, DUMP))) },
    imports: wanted,
    observed: stub,
    abi: { path: ABI, sha256: sha(fs.readFileSync(path.join(ROOT, ABI))), expected_limits: limits },
    initial_state: initial,
    control: { zeroed: 'step_limit', observed: refused.observed, problems: refused.problems },
    non_claims: [
      'a faithful host module is not a faithful runtime: pointer/counter globals start at zero, quota limits use ABI initial values, and no host function exists because the subject imports none',
      'symbolic execution still needs the engine to link the stub with the subject; this lane only proves the interface matches',
      'no Owi counterexample, witness or claim is produced here',
    ],
  };
  fs.writeFileSync(flags['--output'], `${JSON.stringify(receipt, null, 2)}\n`, { flag: 'wx' });
  process.stdout.write(`${JSON.stringify({ output: flags['--output'], clean: source.clean,
    imports: wanted.length, globals: stub.globals })}\n`);
}

main();
