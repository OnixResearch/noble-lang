#!/usr/bin/env node
// WORKER-11 scope review.
//
// This is a review case, not a runtime case. It binds the canonical case
// projection to the selected milestone/status records and verifies that the
// executed Worker slice stayed inside existing language/runtime boundaries.
// MW1 may complete; MW2 remains design-only until native-async evidence.
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = fs.realpathSync(path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..'));
const CASE = 'specs/conformance/worker-cases.json';
const SCOPE = [CASE, 'specs/roadmap.json', 'specs/STATUS.json', 'specs/ROADMAP.md',
  'specs/WORKER-CONFORMANCE.md', 'Cargo.toml', 'crates/noble-kernel/src/contracts/bootstrap.rs',
  'crates/noble-kernel/src/contracts/bootstrap/data.rs',
  'verification/worker11/review.mjs'];
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const hash = file => sha(fs.readFileSync(file));
const git = (...args) => execFileSync('git', args, { cwd: root, encoding: 'utf8' });
function argument(name) { const at = process.argv.indexOf(name); return at > 0 ? process.argv[at + 1] : undefined; }

function main() {
  const destination = argument('--out');
  assert.ok(destination, 'usage: review.mjs --out NEW_DIRECTORY');
  const out = path.resolve(destination);
  assert.ok(!fs.existsSync(out), `output exists: ${out}`);
  const dirty = git('status', '--porcelain', '--untracked-files=all', '--', ...SCOPE);
  assert.equal(dirty, '', `review scope is not clean:\n${dirty}`);
  const sourceFiles = Object.fromEntries(git('ls-files', '-z', '--', ...SCOPE).split('\0').filter(Boolean)
    .sort().map(file => [file, { sha256: hash(path.join(root, file)) }]));
  const cases = JSON.parse(fs.readFileSync(path.join(root, CASE))).cases;
  const selected = cases.filter(row => row.id === 'WORKER-11');
  assert.equal(selected.length, 1);
  const { id, profile, kind, requirements, input, expected } = selected[0];
  const projection = { id, profile, kind, requirements, input, expected };
  const status = JSON.parse(fs.readFileSync(path.join(root, 'specs/STATUS.json')));
  const roadmap = JSON.parse(fs.readFileSync(path.join(root, 'specs/roadmap.json')));
  const milestones = Object.fromEntries(roadmap.milestones.map(row => [row.id, row]));
  const cargo = fs.readFileSync(path.join(root, 'Cargo.toml'), 'utf8');
  const bootstrap = fs.readFileSync(path.join(root, 'crates/noble-kernel/src/contracts/bootstrap/data.rs'), 'utf8');
  const roadmapText = fs.readFileSync(path.join(root, 'specs/ROADMAP.md'), 'utf8');
  const conformance = fs.readFileSync(path.join(root, 'specs/WORKER-CONFORMANCE.md'), 'utf8');
  const mw1Cases = ['WORKER-02','WORKER-03','WORKER-04','WORKER-05','WORKER-06','WORKER-07','WORKER-10','WORKER-12'];
  const byId = Object.fromEntries(cases.map(row => [row.id, row]));
  const mw1Executed = mw1Cases.every(caseId => byId[caseId]?.state.execution === 'passed')
    && byId['WORKER-09']?.state.execution === 'passed';
  const nativeOpen = status.worker_contracts.task_abi === 'open'
    && byId['WORKER-01']?.state.execution === 'not-run'
    && milestones.MW2.status === 'not-started';
  const observed = {
    stage: 'scope-review',
    outcome: nativeOpen ? 'design-only-until-executed-evidence' : 'native-async-claim-present',
    new_kernel_syntax: status.worker_contracts.new_kernel_syntax,
    bootstrap_expanded: status.worker_contracts.bootstrap_expanded,
    ordinary_core_blocked: !(milestones.M1.status === 'completed' && milestones.M2.status === 'completed'
      && milestones.M3.status === 'completed' && milestones.M4.status === 'completed'),
    calculator_replaced: !(status.calculator?.implementation && status.calculator?.implementation !== 'absent'
      && roadmapText.includes('calculator as the first reference application')),
    swarm_runtime_claimed: status.worker_contracts.swarm_runtime_claimed,
    durable_replay_claimed: !conformance.includes('durability, and safe remote retries require separate profiles'),
    syndicate_replaced: !(cargo.includes('"crates/noble-syndicate"')
      && conformance.includes('Syndicate/Synit remains the selected concurrency direction')),
  };
  const failures = [];
  if (!mw1Executed) failures.push('MW1 case execution roster is incomplete');
  if (!roadmapText.includes('All five MW1 entry gates are closed')) failures.push('entry-gate closure is not recorded');
  if (!bootstrap.includes('complete 23-entry bootstrap table')) failures.push('bootstrap differs from the reviewed 23 entries');
  for (const [field, value] of Object.entries(expected)) {
    if (JSON.stringify(observed[field]) !== JSON.stringify(value))
      failures.push(`${field}: expected ${JSON.stringify(value)}, observed ${JSON.stringify(observed[field])}`);
  }
  const receipt = {
    schema: 'noble-worker11-scope-review/v1', case: 'WORKER-11',
    result: failures.length ? 'failed' : 'passed', failures,
    tested_commit: git('rev-parse','HEAD').trim(),
    source_scope: SCOPE, source_revision: `sha256:${sha(JSON.stringify(sourceFiles))}`,
    source_file_count: Object.keys(sourceFiles).length,
    case_projection: projection, case_projection_sha256: sha(JSON.stringify(projection)),
    observations: { ...observed, mw1_case_execution_complete: mw1Executed,
      mw1_entry_gates_closed: roadmapText.includes('All five MW1 entry gates are closed'),
      native_async_interfaces_open: nativeOpen, bootstrap_entry_count: 23 },
    source_files: sourceFiles,
    non_claims: [
      'Review evidence checks scope and claim discipline; it is not runtime or proof evidence.',
      'MW1 completion does not complete MW2: WORKER-01 and WORKER-09 native rows remain open.',
      'The Worker changes use existing module, Program, effect/resource and component contracts; no agent/swarm syntax was added.'
    ]
  };
  fs.mkdirSync(out, { recursive: true });
  fs.writeFileSync(path.join(out, 'review.json'), `${JSON.stringify(receipt, null, 2)}\n`);
  console.log(JSON.stringify({ result: receipt.result, failures, observed, source_revision: receipt.source_revision }));
  process.exitCode = failures.length ? 1 : 0;
}
main();
