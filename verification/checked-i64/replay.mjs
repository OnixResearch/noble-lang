#!/usr/bin/env node
// Record the clean-source execution result for the retained checked-I64 replay.
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
const tests = [
  'checked_i64::retained_checked_i64_expectations_match_lean_and_real_source_wasm',
  'checked_i64::checked_i64_preserves_stack_prefix_on_success_and_overflow',
  'checked_i64_guards::checked_i64_rejections_and_resource_exhaustion_start_no_guest',
];
const args = [
  'test', '-p', 'noble-cli', '--test', 'smoke',
  'checked_i64', '--offline', '--locked',
];

function git(...gitArgs) {
  return execFileSync('git', gitArgs, { cwd: root, encoding: 'utf8' }).trim();
}

function sha256(relativePath) {
  return crypto.createHash('sha256')
    .update(fs.readFileSync(path.join(root, relativePath)))
    .digest('hex');
}

const statusBefore = git('status', '--porcelain', '--untracked-files=all');
if (statusBefore) throw new Error(`replay receipt requires a clean committed tree:\n${statusBefore}`);
const sourceCommit = git('rev-parse', 'HEAD');
const sourceTree = git('rev-parse', 'HEAD^{tree}');
const result = spawnSync(cargo, args, {
  cwd: root,
  env: process.env,
  encoding: 'utf8',
  maxBuffer: 32 * 1024 * 1024,
  timeout: 60 * 60 * 1000,
});
if (result.error) throw result.error;
const transcript = `${result.stdout ?? ''}\n${result.stderr ?? ''}`;
const testResults = tests.map((name) => {
  const line = transcript.split(/\r?\n/).find((candidate) =>
    candidate.trim().startsWith(`test ${name} ... `),
  );
  return line?.trim();
});
if (result.status !== 0 || testResults.some((line) => !line?.endsWith('ok'))) {
  throw new Error(`checked-I64 replay failed\n${transcript}`);
}
const statusAfter = git('status', '--porcelain', '--untracked-files=all');
if (statusAfter) throw new Error(`replay changed the source tree:\n${statusAfter}`);

const receipt = {
  schema: 'noble-checked-i64-replay/v1',
  source_commit: sourceCommit,
  source_tree: sourceTree,
  clean_source: true,
  command: [cargo, ...args],
  tests: tests.map((name, index) => ({ name, result: testResults[index] })),
  retained_cases: 57,
  optimization_modes: ['off', 'on'],
  expected_guest_requests_per_report: 0,
  refusal_resource_cases: 3,
  refusal_cases_expect_no_host_requests: true,
  sha256: {
    proposals: sha256('verification/checked-i64/proposals.json'),
    lean_expectations: sha256('proofs/foundation/NobleFoundation/CheckedI64Expectations.lean'),
    source_wasm: sha256('crates/noble-wasm/runtime/checked-i64.wat'),
    replay_tests: sha256('crates/noble-cli/tests/smoke/checked_i64.rs'),
    refusal_tests: sha256('crates/noble-cli/tests/smoke/checked_i64_guards.rs'),
  },
  claim_boundary: 'Tested source-to-Wasm execution for the retained vectors under two optimization settings, stack-prefix preservation and refusal/resource boundaries; no Rust/Wasm refinement theorem is claimed.',
};
fs.mkdirSync(path.dirname(output), { recursive: true });
fs.writeFileSync(output, `${JSON.stringify(receipt, null, 2)}\n`);
console.log(JSON.stringify({ outcome: 'replay-passed', source_commit: sourceCommit, tests: testResults, output }));
