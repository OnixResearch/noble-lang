#!/usr/bin/env bun
// Reproducible runner for the test-only finite-effects/v1 oracle.
//
// It executes the pinned unit lane, parses the single machine-readable
// receipt line emitted by the test, and writes a source-bound receipt next to
// this script. A non-zero test exit aborts without writing a receipt.

import { $ } from "bun";

const root = new URL("../..", import.meta.url).pathname;
const prefix = "NOBLE_EFFECT_ORACLE_RECEIPT ";
const inner =
  "cargo test -p noble-contracts --lib inference::effects::oracle -- --nocapture";
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
  process.stderr.write("oracle receipt line missing\n");
  process.stderr.write(stdout);
  process.exit(1);
}
const counts = JSON.parse(line.slice(prefix.length));

const revision = (await $`git rev-parse HEAD`.cwd(root).text()).trim();
const status = (
  await $`git status --porcelain --untracked-files=no -- . ':(exclude)verification/effect-oracle/receipt.json'`
    .cwd(root)
    .text()
).trim();
const rust = (await $`nix develop --command rustc --version`.cwd(root).text()).trim();
const cargo = (await $`nix develop --command cargo --version`.cwd(root).text()).trim();

const receipt = {
  schema: "noble-effect-oracle-receipt/v1",
  oracle: counts.oracle,
  source_revision: revision,
  dirty: status.length !== 0,
  command,
  toolchain: { rust, cargo },
  bounds: {
    universe_bits_max: counts.universe_bits_max,
    holes_max: counts.holes_max,
    nodes_max: counts.nodes_max,
    exhaustive_universe_bits_max: 1,
    exhaustive_nodes_max: 3,
    exhaustive_equations_max: 1,
    seeded_nodes_max: 11,
    assignments_max: 1_048_576,
    work_low: 1,
    work_sufficient: 2_000_000,
  },
  seeds: [1, "0x9e3779b97f4a7c15", "0xdeadbeefcafebabe", "0x123456789abcdef0"],
  counts: {
    cases: counts.cases,
    exhaustive_cases: counts.exhaustive_cases,
    agree_accept: counts.agree_accept,
    agree_refuse: counts.agree_refuse,
    conservative_refusal: counts.conservative_refusal,
    wrong_assignment: counts.wrong_assignment,
    unsound_accept: counts.unsound_accept,
    mutants_caught: counts.mutants_caught,
    seeds: counts.seeds,
    low_work_exhausted: counts.low_work_exhausted,
  },
  conservative_refusals: counts.conservative,
  minimized_discrepancies: [],
  elapsed_ms: Date.now() - started,
};

await Bun.write(
  new URL("./receipt.json", import.meta.url),
  `${JSON.stringify(receipt, null, 2)}\n`,
);
console.log(
  `effect oracle receipt: ${counts.cases} cases, ${counts.mutants_caught} mutants, ${counts.conservative_refusal} conservative, dirty=${receipt.dirty}`,
);
