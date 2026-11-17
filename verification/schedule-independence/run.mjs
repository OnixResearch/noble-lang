#!/usr/bin/env node
// Source-bound receipt runner for the #134 order-variation harness.
//
// Usage: node verification/schedule-independence/run.mjs <new receipt path outside the repository>
//
// It refuses a dirty checkout, runs the harness lane of the existing `schedule_correspondence`
// test target with the repository's pinned toolchain (`NOBLE_CARGO` and `NOBLE_RUSTC` name the
// cargo and rustc to run; defaults `cargo` and `rustc`, e.g. inside `nix develop`), parses the
// family and retained-fixture lines the tests print, checks them against the frozen criteria
// (criteria.md, revision 1) and writes one JSON receipt. A failing test, a divergence line, a
// missing family or a coverage count below the criteria minimum writes no receipt.

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
const retained = payloads("NOBLE_SCHEDULE_RETAINED ");
const divergences = lines.filter((line) => line.includes("NOBLE_SCHEDULE_DIVERGENCE"));
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
const receipt = {
  schema: "noble-schedule-independence-receipt/v1",
  issue: 134,
  source_commit: git("rev-parse", "HEAD"),
  source_tree: git("rev-parse", "HEAD^{tree}"),
  dirty: false,
  criteria: { path: "verification/schedule-independence/criteria.md", revision: 1, sha256: sha256(readFileSync(criteria)) },
  command: `cargo ${args.join(" ")}`,
  toolchain: { cargo: version(cargo, ["--version"]), rustc: version(rustc, ["--version", "--verbose"]) },
  test_executable_sha256: sha256(readFileSync(binary)),
  test_summary: summary,
  families,
  retained_fixtures: retained[0].fixtures,
  divergences: 0,
  elapsed_ms: elapsedMs,
  non_claims: [
    "finite order-variation evidence for the listed cases, not a proof of confluence",
    "no Rust-to-Lean refinement and no performance result",
    "allowed differences are counted by class, not compared",
  ],
};
writeFileSync(output, `${JSON.stringify(receipt, null, 2)}\n`, { flag: "wx" });
console.log(`schedule-independence receipt: ${families.length} families, 0 divergences, ${output}`);
