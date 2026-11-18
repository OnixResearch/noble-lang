#!/usr/bin/env node
// Source-bound receipt runner for the #134 order-variation harness.
//
// Usage: node verification/schedule-independence/run.mjs <new receipt path outside the repository>
//
// It refuses a dirty checkout, runs the harness lane of the existing `schedule_correspondence`
// test target with the repository's pinned toolchain (`NOBLE_CARGO` and `NOBLE_RUSTC` name the
// cargo and rustc to run; defaults `cargo` and `rustc`, e.g. inside `nix develop`), parses the
// family, oracle, control and retained-fixture lines the tests print, checks them against the
// frozen criteria (criteria.md revision 1 and criteria-oracle.md revision 2) and writes one JSON
// receipt. A failing test, a divergence line, a missing family, a coverage count below the criteria
// minimum, a normalized-output conflict or a control that is not caught writes no receipt.

import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const lane = dirname(fileURLToPath(import.meta.url));
const root = resolve(lane, "../..");
const output = process.argv[2];
if (!output || process.argv.length !== 3) {
  console.error("usage: run.mjs <new receipt path outside the repository>");
  process.exit(2);
}
if (resolve(output).startsWith(`${root}/`) || existsSync(output)) {
  console.error("run: the receipt path must be new and outside the repository");
  process.exit(2);
}

const sha256 = (bytes) => createHash("sha256").update(bytes).digest("hex");
const git = (...args) => {
  const run = spawnSync("git", args, { cwd: root, encoding: "utf8" });
  if (run.status !== 0) throw new Error(`git ${args.join(" ")} failed: ${run.stderr}`);
  return run.stdout.trim();
};

if (git("status", "--porcelain", "--untracked-files=all") !== "") {
  console.error("run: refusing a dirty checkout; commit first so the receipt binds one source");
  process.exit(2);
}

const cargo = process.env.NOBLE_CARGO ?? "cargo";
const rustc = process.env.NOBLE_RUSTC ?? "rustc";
const args = [
  "test", "--offline", "--locked", "-p", "noble-ngraph", "--test", "schedule_correspondence",
  "--all-features", "--", "schedule_independence", "--nocapture", "--test-threads=1",
];
const started = Date.now();
const run = spawnSync(cargo, args, { cwd: root, encoding: "utf8", maxBuffer: 64 * 1024 * 1024 });
const elapsedMs = Date.now() - started;
const text = `${run.stdout}\n${run.stderr}`;
if (run.status !== 0) {
  process.stderr.write(text);
  console.error(`run: the harness exited ${run.status}; no receipt written`);
  process.exit(1);
}

const lines = text.split("\n");
const payloads = (prefix) =>
  lines.flatMap((line) => {
    const at = line.indexOf(prefix);
    return at < 0 ? [] : [JSON.parse(line.slice(at + prefix.length))];
  });
const families = payloads("NOBLE_SCHEDULE_FAMILY ");
const oracle = payloads("NOBLE_SCHEDULE_ORACLE ");
const control = payloads("NOBLE_SCHEDULE_ORACLE_CONTROL ");
const retained = payloads("NOBLE_SCHEDULE_RETAINED ");
const divergences = lines.filter((line) => line.includes("NOBLE_SCHEDULE_DIVERGENCE"));
const oracleConflicts = lines.filter((line) => line.includes("NOBLE_SCHEDULE_ORACLE_CONFLICT"));
const summary = lines.find((line) => line.startsWith("test result: "));
const binary = /Running tests\/schedule_correspondence\.rs \(([^)]+)\)/.exec(text)?.[1];

// Criteria section 5 minimums, as orders run per case (the serial baseline included).
const seeded = 32;
const minimum = {
  "F1 kernel-dependencies": () => true,
  "F2 kernel-threads": (f) => f.schedules >= 32,
  "F3 contracts-queries": (f) => f.schedules >= f.cases * (3 + seeded),
  "F4 contracts-commits": () => true,
  "F5 contracts-modules": (f) => f.schedules >= f.cases * 25,
  "F6 graph-orders": (f) => f.schedules >= f.cases * (2 + seeded),
  "F7 net-schedules": (f) => f.schedules >= f.cases * (2 + seeded),
  "F8 cache-sequences": (f) => f.schedules >= f.cases * (3 + seeded),
  "F9 threads": (f) => f.schedules >= 32,
};
const problems = [];
for (const [name, enough] of Object.entries(minimum)) {
  const family = families.filter((entry) => entry.family === name);
  if (family.length !== 1) problems.push(`${name}: ${family.length} receipt lines`);
  else if (family[0].divergences !== 0) problems.push(`${name}: divergences`);
  else if (!enough(family[0])) problems.push(`${name}: below the criteria minimum`);
}
if (families.length !== Object.keys(minimum).length) problems.push("unexpected family lines");
if (divergences.length !== 0) problems.push(`${divergences.length} divergence lines`);
if (retained.length !== 1) problems.push("retained-fixture line missing");
if (!summary || !summary.startsWith("test result: ok.")) problems.push("no passing test summary");
if (!binary || !existsSync(binary)) problems.push("test executable not found");

// criteria-oracle section 2: one table per family, zero normalized-output conflicts.
for (const name of Object.keys(minimum)) {
  const table = oracle.filter((entry) => entry.family === name);
  if (table.length !== 1) problems.push(`${name}: ${table.length} oracle tables`);
  else if (table[0].normalized_conflicts !== 0) problems.push(`${name}: normalized-output conflicts`);
  else if (!(table[0].rows > 0) || !(table[0].groups > 0)) problems.push(`${name}: empty table`);
}
if (oracle.length !== Object.keys(minimum).length) problems.push("unexpected oracle tables");
if (oracleConflicts.length !== 0) problems.push(`${oracleConflicts.length} oracle conflict lines`);

// criteria-oracle section 3 and 4: exactly two retained control rows, from two schedules, seeded.
if (control.length !== 1) {
  problems.push("control line missing");
} else {
  const caught = control[0];
  const rows = Array.isArray(caught.rows) ? caught.rows : [];
  if (rows.length !== 2) problems.push(`control retained ${rows.length} rows`);
  else {
    const [first, second] = rows;
    if (first.input !== second.input) problems.push("control rows name different inputs");
    if (first.normalized === second.normalized) problems.push("control rows share a normalized output");
    if (first.schedule === second.schedule) problems.push("control rows share a schedule");
    if (!(`${first.schedule}${second.schedule}`).includes("seed-")) {
      problems.push("control pair is not seeded");
    }
  }
  if (caught.reproduced !== 2) problems.push("control rows did not both re-execute");
  if (!(caught.replayed >= 1)) problems.push("control pair did not replay");
}
if (problems.length !== 0) {
  process.stderr.write(text);
  console.error(`run: ${problems.join("; ")}; no receipt written`);
  process.exit(1);
}

const version = (program, flags) => {
  const probe = spawnSync(program, flags, { cwd: root, encoding: "utf8" });
  return probe.status === 0 ? probe.stdout.trim() : `unavailable (exit ${probe.status})`;
};
const criteria = join(lane, "criteria.md");
const oracleCriteria = join(lane, "criteria-oracle.md");
const receipt = {
  schema: "noble-schedule-independence-receipt/v2",
  issue: 134,
  source_commit: git("rev-parse", "HEAD"),
  source_tree: git("rev-parse", "HEAD^{tree}"),
  dirty: false,
  criteria: { path: "verification/schedule-independence/criteria.md", revision: 1, sha256: sha256(readFileSync(criteria)) },
  oracle_criteria: {
    path: "verification/schedule-independence/criteria-oracle.md",
    revision: 2,
    sha256: sha256(readFileSync(oracleCriteria)),
  },
  command: `cargo ${args.join(" ")}`,
  toolchain: { cargo: version(cargo, ["--version"]), rustc: version(rustc, ["--version", "--verbose"]) },
  test_executable_sha256: sha256(readFileSync(binary)),
  test_summary: summary,
  families: families.toSorted((first, second) => first.family.localeCompare(second.family)),
  oracle: oracle.toSorted((first, second) => first.family.localeCompare(second.family)),
  oracle_control: control[0],
  retained_fixtures: retained[0].fixtures,
  divergences: 0,
  elapsed_ms: elapsedMs,
  non_claims: [
    "finite order-variation evidence for the listed cases, not a proof of confluence",
    "no Rust-to-Lean refinement and no performance result",
    "allowed differences are counted by class, not compared",
    "the table oracle's digests are a 128-bit table fixpoint, not a cryptographic identity claim",
    "the net-level proved results (#124 and inets MRun.confluent) are not re-proved and do not discharge #134",
  ],
};
writeFileSync(output, `${JSON.stringify(receipt, null, 2)}\n`, { flag: "wx" });
console.log(
  `schedule-independence receipt: ${families.length} families, 0 divergences, ${oracle.length} oracle tables, control caught with 2 retained rows, ${output}`,
);
