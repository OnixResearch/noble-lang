#!/usr/bin/env bun
// Reproducible runner for the test-only SMERGE/1 model (issue #30).
//
// It executes the fixture lane, parses the machine-readable receipt line and
// writes a source-bound receipt. A non-zero test exit aborts without writing a
// receipt. This is model evidence only: the Lean relation, termination theorem
// and claim generator remain open.

import { $ } from "bun";

const root = new URL("../..", import.meta.url).pathname;
const prefix = "NOBLE_SEMANTIC_MERGE_RECEIPT ";
const inner =
  "cargo test -p noble-contracts --test source -- --nocapture semantic_merge_model";
const command = `nix develop --command ${inner}`;
const started = Date.now();

const run = Bun.spawnSync(
  ["nix", "develop", "--command", "sh", "-c", inner],
  {
    cwd: root,
    stdout: "pipe",
    stderr: "pipe",
    env: { ...process.env, CARGO_INCREMENTAL: "0" },
  },
);

const stdout = new TextDecoder().decode(run.stdout);
const stderr = new TextDecoder().decode(run.stderr);
if (run.exitCode !== 0) {
  process.stderr.write(stderr);
  process.stderr.write(stdout);
  process.exit(run.exitCode ?? 1);
}

const line = stdout.split("\n").find((entry) => entry.startsWith(prefix));
if (!line) {
  process.stderr.write("semantic-merge receipt line missing\n");
  process.stderr.write(stdout);
  process.exit(1);
}
const counts = JSON.parse(line.slice(prefix.length));

const revision = (await $`git rev-parse HEAD`.cwd(root).text()).trim();
const status = (
  await $`git status --porcelain --untracked-files=no -- . ':(exclude)verification/semantic-merge-model/receipt.json'`
    .cwd(root)
    .text()
).trim();
const rust = (await $`nix develop --command rustc --version`.cwd(root).text()).trim();
const cargo = (await $`nix develop --command cargo --version`.cwd(root).text()).trim();

const receipt = {
  schema: "noble-semantic-merge-model-receipt/v1",
  relation: counts.relation,
  source_revision: revision,
  dirty: status.length !== 0,
  command,
  toolchain: { rust, cargo },
  bounds: {
    inputs: counts.inputs,
    output_len: 2,
    i64: "wrapping-u64",
    model_scope: "pure-first-order-total",
  },
  counts: {
    fixtures: counts.fixtures,
    verified: counts.verified,
    conflicts: counts.conflicts,
    counterexamples: counts.counterexamples,
    incompatible: counts.incompatible,
    abnormal: counts.abnormal,
  },
  decision: counts.decision,
  lean_obligations_open: counts.lean_obligations_open,
  rationale:
    "The bounded model exercises the relation and fixtures, but SM-1 through SM-6 (Lean relation, termination, claim elaboration, checker soundness, evidence binding, outcome discipline) remain open; no integration.",
  elapsed_ms: Date.now() - started,
};

await Bun.write(
  new URL("./receipt.json", import.meta.url),
  `${JSON.stringify(receipt, null, 2)}\n`,
);
console.log(
  `semantic-merge model receipt: ${counts.fixtures} fixtures, decision=${counts.decision}, Lean obligations open=${counts.lean_obligations_open}`,
);
