#!/usr/bin/env node
// Apply each source-level mutant in place, run one real CLI replay, then restore it.
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { execFileSync, spawnSync } from 'node:child_process';

function argument(name) {
  const index = process.argv.indexOf(name);
  if (index < 0 || !process.argv[index + 1]) throw new Error(`missing ${name}`);
  return process.argv[index + 1];
}

const root = path.resolve(argument('--root'));
const output = path.resolve(argument('--output'));
if (output === root || output.startsWith(`${root}${path.sep}`)) {
  throw new Error('receipt output must be outside the repository');
}
const cargo = process.env.NOBLE_CARGO;
if (!cargo) throw new Error('NOBLE_CARGO must name the repository Rust test command');
const target = path.join(root, 'crates/noble-wasm/runtime/checked-i64.wat');
const original = fs.readFileSync(target, 'utf8');
const testName = 'checked_i64::retained_vectors_match_lean_and_real_source_wasm';
const cargoArgs = [
  'test', '-p', 'noble-cli', '--test', 'smoke',
  testName,
  '--offline', '--locked', '--', '--exact',
];
const mutants = [
  {
    id: 'inclusive-overflow-boundary',
    purpose: 'classifying a zero sign-difference as overflow',
    function: '$op_checked_i64_add',
    from: '(i64.lt_s (i64.xor (local.get $left) (local.get $result)) (i64.const 0))',
    to: '(i64.le_s (i64.xor (local.get $left) (local.get $result)) (i64.const 0))',
  },
  {
    id: 'unsigned-comparison',
    purpose: 'using unsigned rather than signed I64 order',
    function: '$op_checked_i64_less',
    from: '(i64.lt_s (local.get $left) (local.get $right))',
    to: '(i64.lt_u (local.get $left) (local.get $right))',
  },
  {
    id: 'floor-divrem',
    purpose: 'adjusting truncating division to floor quotient and divisor-signed remainder',
    function: '$op_checked_i64_div_rem',
    from: '(local.set $quotient (i64.div_s (local.get $left) (local.get $right)))\n (local.set $remainder (i64.rem_s (local.get $left) (local.get $right)))',
    to: '(local.set $quotient (i64.div_s (local.get $left) (local.get $right)))\n (local.set $remainder (i64.rem_s (local.get $left) (local.get $right)))\n (if (i64.ne (local.get $remainder) (i64.const 0))\n  (then\n   (if (i32.ne (i64.lt_s (local.get $left) (i64.const 0))\n                (i64.lt_s (local.get $right) (i64.const 0)))\n    (then\n     (local.set $quotient (i64.sub (local.get $quotient) (i64.const 1)))\n     (local.set $remainder (i64.add (local.get $remainder) (local.get $right)))))))',
  },
  {
    id: 'reversed-subtraction-operands',
    purpose: 'swapping the operands of subtraction',
    function: '$op_checked_i64_sub',
    from: '(i64.sub (local.get $left) (local.get $right))',
    to: '(i64.sub (local.get $right) (local.get $left))',
  },
  {
    id: 'exposed-overflow-partial-result',
    purpose: 'pushing the wrapped intermediate before returning an overflow sum',
    function: '$op_checked_i64_add',
    from: '(then (call $checked_i64_error (i64.const 1)) (return)))',
    to: '(then (call $push_i64 (local.get $result))\n        (call $checked_i64_error (i64.const 1)) (return)))',
  },
];

function git(...args) {
  return execFileSync('git', args, { cwd: root, encoding: 'utf8' }).trim();
}

function replaceFunction(source, mutant) {
  const start = source.indexOf(`(func ${mutant.function}`);
  if (start < 0) throw new Error(`missing function ${mutant.function}`);
  const next = /\n[ \t]*\(func /.exec(source.slice(start + 1));
  const end = next ? start + 1 + next.index : source.length;
  const body = source.slice(start, end);
  const first = body.indexOf(mutant.from);
  if (first < 0 || body.indexOf(mutant.from, first + mutant.from.length) >= 0) {
    throw new Error(`${mutant.id}: expected exactly one mutation target`);
  }
  return source.slice(0, start)
    + body.slice(0, first)
    + mutant.to
    + body.slice(first + mutant.from.length)
    + source.slice(end);
}

function run(label, shouldFail) {
  const result = spawnSync(cargo, cargoArgs, {
    cwd: root,
    env: process.env,
    encoding: 'utf8',
    maxBuffer: 32 * 1024 * 1024,
    timeout: 60 * 60 * 1000,
  });
  if (result.error) throw result.error;
  const transcript = `${result.stdout ?? ''}\n${result.stderr ?? ''}`;
  const testLine = transcript.split(/\r?\n/).find((line) =>
    line.trim().startsWith(`test ${testName} ... `),
  );
  if (result.status === null) throw new Error(`${label}: test process timed out`);
  if (shouldFail) {
    if (result.status === 0 || !testLine?.trim().endsWith('FAILED')) {
      throw new Error(`${label}: mutation was not caught by the target test\n${transcript}`);
    }
  } else if (result.status !== 0 || !testLine?.trim().endsWith('ok')) {
    throw new Error(`${label}: unmutated target test did not pass\n${transcript}`);
  }
  return { exit_status: result.status, test_result: testLine.trim() };
}

const statusBefore = git('status', '--porcelain', '--untracked-files=all');
if (statusBefore) throw new Error(`mutation run requires a clean committed tree:\n${statusBefore}`);
const sourceCommit = git('rev-parse', 'HEAD');
const sourceTree = git('rev-parse', 'HEAD^{tree}');
const sourceDigest = crypto.createHash('sha256').update(original).digest('hex');
const results = [];

try {
  const baseline = run('baseline', false);
  for (const mutant of mutants) {
    const mutated = replaceFunction(original, mutant);
    fs.writeFileSync(target, mutated);
    let result;
    try {
      result = run(mutant.id, true);
    } finally {
      fs.writeFileSync(target, original);
    }
    results.push({
      id: mutant.id,
      purpose: mutant.purpose,
      function: mutant.function,
      outcome: 'caught',
      ...result,
    });
  }
  const statusAfter = git('status', '--porcelain', '--untracked-files=all');
  if (statusAfter) throw new Error(`source did not restore to the clean commit:\n${statusAfter}`);

  const receipt = {
    schema: 'noble-checked-i64-mutants/v1',
    source_commit: sourceCommit,
    source_tree: sourceTree,
    clean_source: true,
    restored_clean_source: true,
    test: testName,
    command: [cargo, ...cargoArgs],
    wat_sha256: sourceDigest,
    baseline,
    mutants: results,
    claim_boundary: 'The mutations are caught by tested source-to-Wasm behavior; this is not a Rust/Wasm refinement proof.',
  };
  fs.mkdirSync(path.dirname(output), { recursive: true });
  fs.writeFileSync(output, `${JSON.stringify(receipt, null, 2)}\n`);
  console.log(JSON.stringify({ outcome: 'all-mutants-caught', source_commit: sourceCommit, count: results.length, output }));
} finally {
  fs.writeFileSync(target, original);
}
