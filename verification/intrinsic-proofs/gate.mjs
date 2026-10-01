#!/usr/bin/env node
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { Recorder, root, sha256, fileHash } from './record.mjs';
import { preparePlan, executedCases } from './plan.mjs';
import { scenarios } from './fixtures.mjs';
import { bindBuild } from './build.mjs';
import { ProofRunner } from './proof.mjs';
import { Runtime } from './runtime.mjs';
import { executeCases } from './cases.mjs';

assert.equal(process.argv.length, 4,
  'usage: SELECTED_NODE verification/intrinsic-proofs/gate.mjs SHARED_BUILD_RECEIPT NEW_EXTERNAL_DIRECTORY');
const record = new Recorder(process.argv[3]);
let plan;
try {
  plan = preparePlan(record, scenarios());
  record.receipt.case_ledger = structuredClone(plan.case_ledger);
  const tools = bindBuild(record, plan, process.argv[2]);
  const selection = JSON.parse(fs.readFileSync(path.join(root, 'policy/tool-selection.json')));
  const selectedLean = path.join(selection.tool_paths.lean.output, 'bin/lean');
  const proofTools = {
    NOBLE_LEAN: selectedLean,
    NOBLE_BWRAP: '/nix/store/y0ra9qr3rz81d9wl7dfrldv3j25dc98q-bubblewrap-0.12.0/bin/bwrap',
    NOBLE_PRLIMIT: '/nix/store/mqvbf0flamqaq9c3ihb496aahag897n0-util-linux-2.42.3-bin/bin/prlimit',
    NOBLE_SYSTEMD_RUN: '/nix/store/baxgs06659w4i6ggsfr56srjrb52v1h8-systemd-261.2/bin/systemd-run',
  };
  const retainedTools = {};
  for (const [name, file] of Object.entries(proofTools)) {
    const frozen = record.freeze(name.toLowerCase(), file);
    retainedTools[name] = { sha256: fileHash(frozen), original: record.receipt.tools[name.toLowerCase()].original };
  }
  assert.equal(retainedTools.NOBLE_LEAN.sha256, plan.tools.lean.sha256,
    'Lean verifier tool differs from frozen plan');
  const temp = path.join(record.output, 'tmp');
  const home = path.join(record.output, 'home');
  fs.mkdirSync(temp);
  fs.mkdirSync(home);
  const environment = {
    PATH: '/run/current-system/sw/bin', HOME: home, TMPDIR: temp,
    LANG: 'C.UTF-8', LC_ALL: 'C.UTF-8', XDG_RUNTIME_DIR: process.env.XDG_RUNTIME_DIR,
    NOBLE_CONTRACT_LIBRARY: path.join(root, 'proofs/mc1'), ...proofTools,
  };
  const revisionInputs = {
    sources: Object.fromEntries(Object.entries(plan.sources).map(([name, item]) => [name, item.sha256])),
    selected_cases: plan.canonical.workload_sha256,
    fixture_sources: Object.fromEntries(Object.entries(plan.inputs).map(([id, row]) =>
      [id, { baseline: row.baseline.sha256,
        variants: Object.fromEntries(Object.entries(row.variants).map(([name, file]) => [name, file.sha256])),
        extras: Object.fromEntries(Object.entries(row.extras).map(([name, file]) => [name, file.sha256])) }])),
    selected_tools: Object.fromEntries(Object.entries(record.receipt.tools).map(([name, value]) =>
      [name, value.sha256])),
    shared_build: record.receipt.build.sha256,
  };
  record.receipt.revision_inputs = revisionInputs;
  record.receipt.source_revision = `sha256:${sha256(Buffer.from(JSON.stringify(revisionInputs)))}`;
  const proof = new ProofRunner(record, plan, tools.cli, environment);
  const runtime = new Runtime(record, plan, tools.cli, tools.wasmTools, environment);
  await executeCases({ record, plan, proof, runtime, tools, environment });
} catch (error) {
  record.receipt.failures.push({ case_id: 'gate', error: String(error.stack ?? error) });
} finally {
  const sealed = record.seal(executedCases);
  console.log(JSON.stringify({ schema: record.receipt.schema, result: sealed.result,
    report: sealed.report, summary: sealed.summary }));
  process.exitCode = sealed.result === 'passed' ? 0 : 1;
}
