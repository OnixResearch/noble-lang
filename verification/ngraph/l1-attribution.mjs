#!/usr/bin/env node
// L1 attribution evidence (issue #75).
//
// Compares the L1 records (per program, per `--opt` leg, and the `I4`
// transition) and the B3 ordinary-path records of fresh `ngraph-phase-b-extract`
// receipts with the retained receipt, and folds in the rule-mutation result of
// `l1-attribution-mutations.mjs`. A difference is attributed to the CLI version
// only when a run whose CLI was built from the baseline CLI's source revision
// reproduces that record exactly.
//
// Usage (repository root):
//   node verification/ngraph/l1-attribution.mjs \
//     --baseline verification/ngraph/phase-b-extract.json --baseline-cli-revision REV \
//     --run LABEL RECEIPT CLI_SOURCE_REVISION [--run ...] \
//     --mutations MUTATIONS.json --out OUT.json

import crypto from 'node:crypto';
import fs from 'node:fs';

const SCHEMA = 'noble-ngraph-l1-attribution-evidence/v1';

function args(argv) {
  const out = { runs: [] };
  for (let i = 0; i < argv.length;) {
    const flag = argv[i];
    if (flag === '--run') {
      const [label, receipt, cliRevision] = argv.slice(i + 1, i + 4);
      if (cliRevision === undefined) throw new Error('--run needs LABEL RECEIPT CLI_SOURCE_REVISION');
      out.runs.push({ label, receipt, cliRevision });
      i += 4;
      continue;
    }
    const value = argv[i + 1];
    if (value === undefined) throw new Error(`missing value for ${flag}`);
    if (flag === '--baseline') out.baseline = value;
    else if (flag === '--baseline-cli-revision') out.baselineCliRevision = value;
    else if (flag === '--mutations') out.mutations = value;
    else if (flag === '--out') out.out = value;
    else throw new Error(`unknown flag ${flag}`);
    i += 2;
  }
  for (const key of ['baseline', 'baselineCliRevision', 'mutations', 'out']) {
    if (!out[key]) throw new Error(`missing --${key.replace(/[A-Z]/g, (c) => `-${c.toLowerCase()}`)}`);
  }
  if (out.runs.length === 0) throw new Error('at least one --run is required');
  if (fs.existsSync(out.out)) throw new Error(`refusing to overwrite ${out.out}`);
  return out;
}

const readJson = (path) => JSON.parse(fs.readFileSync(path, 'utf8'));
const fileSha = (path) => crypto.createHash('sha256').update(fs.readFileSync(path)).digest('hex');
const record = (receipt, prefix) => {
  const found = receipt.records.find((r) => (r.id ?? '').startsWith(prefix));
  if (!found) throw new Error(`no ${prefix} record`);
  return found;
};

function l1Summary(mode) {
  if (!mode) return null;
  return {
    vectors: mode.vectors ?? null,
    steps: mode.steps ?? null,
    agree: mode.agree ?? null,
    ordinary_quota_outcomes: (mode.ordinary_quota_outcomes ?? []).map((q) => `${q.vector}:${q.outcome}`),
    session_restarts_after_ordinary_quota: mode.session_restarts_after_ordinary_quota ?? null,
    session_failure: mode.session_failure ?? null,
  };
}

function b3Summary(ordinary) {
  return {
    reports: ordinary.reports ?? null,
    first_non_normal: ordinary.first_non_normal ?? null,
    first_non_normal_outcome: ordinary.first_non_normal_outcome?.outcome ?? null,
    last_outcome: ordinary.last_outcome ?? null,
    last_stack: ordinary.last_stack ?? null,
    live_slot_outcomes: ordinary.outcomes ?? null,
  };
}

// Every comparable record of one receipt, keyed by a stable name.
function summaries(receipt) {
  const rows = new Map();
  const b2 = record(receipt, 'B2-I64').evidence.result;
  for (const kind of ['named', 'random']) {
    (b2[kind] ?? []).forEach((program, i) => {
      if (!program.l1) return;
      for (const opt of ['off', 'on']) rows.set(`${kind}[${i}] ${program.name ?? ''} --opt ${opt}`.replace('  ', ' '), l1Summary(program.l1[opt]));
    });
  }
  const transition = b2.i4_transition?.l1;
  for (const opt of ['off', 'on']) rows.set(`i4_transition --opt ${opt}`, l1Summary(transition?.[opt]));
  const b3 = record(receipt, 'B3-I64').evidence;
  const cases = b3.result ?? b3;
  for (const control of Array.isArray(cases) ? cases : cases.cases ?? []) {
    if (control.ordinary?.submissions || control.ordinary?.live_slot_requests) rows.set(`B3 ${control.name}`, b3Summary(control.ordinary));
  }
  return rows;
}

const opts = args(process.argv.slice(2));
const baseline = readJson(opts.baseline);
const baselineRows = summaries(baseline);
const runs = opts.runs.map((run) => {
  const receipt = readJson(run.receipt);
  const rows = summaries(receipt);
  const keys = [...new Set([...baselineRows.keys(), ...rows.keys()])];
  const records = keys.map((key) => {
    const fresh = rows.get(key) ?? null;
    const old = baselineRows.get(key) ?? null;
    const identical = JSON.stringify(fresh) === JSON.stringify(old);
    // The baseline summary is retained only where it differs.
    return identical ? { record: key, identical, fresh } : { record: key, identical, fresh, baseline: old };
  });
  return {
    label: run.label,
    cli_source_revision: run.cliRevision,
    cli_sha256: receipt.run.ordinary_path?.cli_sha256 ?? null,
    adapter_source_revision: receipt.run.source_revision,
    harness: {
      result: receipt.result,
      optimization_mode: receipt.run.optimization_mode,
      toolchain: receipt.run.toolchain,
      workload: receipt.run.workload,
      node: receipt.run.oracle?.node ?? null,
      wall_seconds: receipt.run.wall_seconds,
      l1_totals: record(receipt, 'B2-I64').evidence.result.l1_totals ?? null,
    },
    session_failures: records.filter((r) => r.fresh?.session_failure).length,
    compared: records.length,
    identical: records.filter((r) => r.identical).length,
    records,
  };
});

const sameCli = runs.filter((run) => run.cli_source_revision === opts.baselineCliRevision);
for (const run of runs) {
  run.differing = run.records.filter((r) => !r.identical).map((r) => ({
    record: r.record,
    attributed_to_cli_version: sameCli.some((other) => other !== run && other.records.some((o) => o.record === r.record && o.identical)),
  }));
}

const mutations = readJson(opts.mutations);
const unattributed = runs.flatMap((run) => run.differing.filter((d) => !d.attributed_to_cli_version).map((d) => `${run.label}: ${d.record}`));
const passed = runs.every((run) => run.harness.result === 'passed' && run.session_failures === 0)
  && mutations.result === 'passed' && unattributed.length === 0;
const evidence = {
  schema: SCHEMA,
  issue: 75,
  scope: 'L1 report attribution and session completion in noble-ngraph-experiment: the hardened adapter against real CLIs in both --opt legs, compared with the retained extraction receipt, plus the rule-mutation check of the fixture-child tests.',
  baseline: {
    path: opts.baseline,
    sha256: fileSha(opts.baseline),
    source_revision: baseline.run.source_revision ?? null,
    cli_source_revision: opts.baselineCliRevision,
    cli_sha256: baseline.run.ordinary_path?.cli_sha256 ?? null,
  },
  runs,
  mutations: {
    path_sha256: fileSha(opts.mutations),
    source_revision: mutations.source_revision,
    file_sha256: mutations.file_sha256,
    toolchain: mutations.toolchain,
    unmutated_passed: mutations.unmutated_passed.length,
    result: mutations.result,
    silent: mutations.silent,
    rules: mutations.mutations.map((m) => ({ rule: m.rule, failed: m.failed, timed_out: m.timed_out })),
  },
  unattributed_differences: unattributed,
  non_claims: [
    'The checks bind reports to the submitted sequence and to the self-reported module identity; they do not attest a malicious child or prove the semantics of the pinned CLI or engine.',
    'The fixture children and mutations are finite negative controls, not a proof of execution correctness.',
    'A CLI-version difference is attributed by exact reproduction with a CLI built from the baseline revision; it is not explained beyond that.',
  ],
  result: passed ? 'passed' : 'failed',
};
fs.writeFileSync(opts.out, `${JSON.stringify(evidence, null, 2)}\n`);
console.log(`${opts.out} ${evidence.result}: ${runs.map((r) => `${r.label} ${r.identical}/${r.compared} identical`).join('; ')}; unattributed ${unattributed.length}`);
