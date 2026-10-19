#!/usr/bin/env bun
// Reproducible runner for the test-only finite-bases/v0 prototype (issue #29).
//
// It executes the prototype lane, parses the machine-readable receipt line and
// writes a source-bound receipt. A non-zero test exit aborts without writing a
// receipt.

import { $ } from "bun";

const root = new URL("../..", import.meta.url).pathname;
const prefix = "NOBLE_FINITE_BASES_RECEIPT ";
const inner =
  "cargo test -p noble-contracts --test source -- --nocapture finite_bases";
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
  process.stderr.write("finite-bases receipt line missing\n");
  process.stderr.write(stdout);
  process.exit(1);
}
const counts = JSON.parse(line.slice(prefix.length));

const revision = (await $`git rev-parse HEAD`.cwd(root).text()).trim();
const status = (
  await $`git status --porcelain --untracked-files=no -- . ':(exclude)verification/finite-subtyping/receipt.json'`
    .cwd(root)
    .text()
).trim();
const rust = (await $`nix develop --command rustc --version`.cwd(root).text()).trim();
const cargo = (await $`nix develop --command cargo --version`.cwd(root).text()).trim();

const receipt = {
  schema: "noble-finite-bases-receipt/v1",
  relation: counts.relation,
  source_revision: revision,
  dirty: status.length !== 0,
  command,
  toolchain: { rust, cargo },
  bounds: {
    universe_bits_max: 4,
    inputs: 0,
    outputs: 1,
    family_cap: 16,
  },
  counts: {
    cases: counts.cases,
    accepted: counts.accepted,
    refused: counts.refused,
    principal_families: counts.principal_families,
    ambiguous: counts.ambiguous,
    max_family: counts.max_family,
    real_accepted: counts.real_accepted,
    real_refused: counts.real_refused,
  },
  decision: counts.decision,
  rationale:
    "Every supported family has a principal candidate; the only non-reflexive relation is directed effect-bound inclusion.",
  elapsed_ms: Date.now() - started,
};

await Bun.write(
  new URL("./receipt.json", import.meta.url),
  `${JSON.stringify(receipt, null, 2)}\n`,
);
console.log(
  `finite-bases receipt: ${counts.cases} cases, ${counts.principal_families} principal, ${counts.ambiguous} ambiguous, decision=${counts.decision}`,
);
