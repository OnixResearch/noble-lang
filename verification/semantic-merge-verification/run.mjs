#!/usr/bin/env bun
// Reproducible runner for the Phase 3 SMERGE/1 evidence (issue #30).
//
// It rebuilds the pinned Lean model, re-runs the Phase 2 model lane, runs the
// Phase 3 claim/consumer lane and the isolated pinned Lean check, then writes a
// source-bound receipt. A dirty tree, a missing toolchain or a nonzero exit
// aborts without writing a receipt. This is the test-only lane: it does not
// merge, publish, admit a live slot or integrate anything.

import { $ } from "bun";
import { createHash } from "node:crypto";

const root = new URL("../..", import.meta.url).pathname;
const receiptPath = new URL("./receipt.json", import.meta.url).pathname;
const phase2Prefix = "NOBLE_SEMANTIC_MERGE_RECEIPT ";
const verificationPrefix = "NOBLE_SEMANTIC_MERGE_VERIFICATION ";
const leanPrefix = "NOBLE_SEMANTIC_MERGE_LEAN ";
const started = Date.now();

const run = (args, { cwd = root, env = {} } = {}) => {
  const result = Bun.spawnSync(args, {
    cwd,
    stdout: "pipe",
    stderr: "pipe",
    env: { ...process.env, CARGO_INCREMENTAL: "0", ...env },
  });
  return {
    status: result.exitCode ?? 1,
    stdout: new TextDecoder().decode(result.stdout),
    stderr: new TextDecoder().decode(result.stderr),
  };
};

const fail = (message) => {
  process.stderr.write(`semantic-merge-verification: ${message}\n`);
  process.exit(1);
};

const line = (stdout, prefix) => {
  // `--nocapture` interleaves a test's stdout on its harness line, so the
  // marker is found anywhere in a line and the payload starts after it.
  for (const entry of stdout.split("\n")) {
    const at = entry.indexOf(prefix);
    if (at >= 0) return { ...JSON.parse(entry.slice(at + prefix.length)), raw: entry };
  }
  fail(`missing ${prefix.trim()} line`);
};

const status = (
  await $`git status --porcelain`.cwd(root).text()
).trim();
const excluded = [receiptPath];
const dirty = status
  .split("\n")
  .filter((entry) => entry.trim().length !== 0)
  .filter((entry) => !excluded.some((path) => entry.endsWith(path)));
if (dirty.length !== 0) {
  fail(`working tree is not clean: ${dirty.join("; ")}`);
}
const revision = (await $`git rev-parse HEAD`.cwd(root).text()).trim();

const leanBuild = run(["lake", "build", "NobleContracts.Merge"], {
  cwd: `${root}proofs/mc1`,
});
if (leanBuild.status !== 0) {
  process.stderr.write(leanBuild.stdout);
  process.stderr.write(leanBuild.stderr);
  fail("pinned Lean model did not build");
}

const lane = run([
  "cargo",
  "test",
  "-p",
  "noble-contracts",
  "--test",
  "source",
  "--",
  "--nocapture",
  "--test-threads=1",
  "semantic_merge_model",
]);
if (lane.status !== 0) {
  process.stderr.write(lane.stdout);
  process.stderr.write(lane.stderr);
  fail("the SMERGE/1 model lane failed");
}
const phase2 = line(lane.stdout, phase2Prefix);
const verification = line(lane.stdout, verificationPrefix);

const lean = run(
  [
    "cargo",
    "test",
    "-p",
    "noble-contracts",
    "--test",
    "source",
    "--",
    "--nocapture",
    "--ignored",
    "semantic_merge_lean_check",
  ],
  {
    env: {
      NOBLE_LEAN: process.env.NOBLE_LEAN ?? "lean",
      NOBLE_CONTRACT_LIBRARY: process.env.NOBLE_CONTRACT_LIBRARY ?? `${root}proofs/mc1`,
    },
  },
);
if (lean.status !== 0) {
  process.stderr.write(lean.stdout);
  process.stderr.write(lean.stderr);
  fail("the isolated pinned Lean check failed");
}
const acceptance = line(lean.stdout, leanPrefix);

const sha256 = (text) => createHash("sha256").update(text, "utf8").digest("hex");
const claims = [
  {
    case: "SMERGE-CASE-04",
    claim: verification.case04_claim,
    claim_sha256: sha256(verification.case04_claim),
    proof: acceptance.proof_case04,
    proof_sha256: sha256(acceptance.proof_case04),
    outcome: acceptance.case04,
  },
  {
    case: "SMERGE-CASE-01",
    claim: verification.case01_claim,
    claim_sha256: sha256(verification.case01_claim),
    proof: acceptance.proof_case01,
    proof_sha256: sha256(acceptance.proof_case01),
    outcome: acceptance.case01,
  },
];
if (claims[0].claim_sha256 !== verification.case04_digest) {
  fail("case-04 claim digest disagrees between the lane and the runner");
}
if (claims[1].claim_sha256 !== verification.case01_digest) {
  fail("case-01 claim digest disagrees between the lane and the runner");
}
if (claims[0].outcome !== "verified" || claims[1].outcome !== "verified") {
  fail("the pinned Lean consumer did not verify both claims");
}

const rust = run(["rustc", "--version"]);
const cargo = run(["cargo", "--version"]);
if (rust.status !== 0 || cargo.status !== 0) fail("toolchain versions unavailable");

const receipt = {
  schema: "noble-semantic-merge-verification-receipt/v1",
  relation: verification.relation,
  semantic_revision: verification.semantic_revision,
  source_revision: revision,
  dirty: false,
  command:
    "lake build NobleContracts.Merge && cargo test -p noble-contracts --test source -- --nocapture semantic_merge_model && cargo test -p noble-contracts --test source -- --ignored semantic_merge_lean_check",
  toolchain: {
    rust: rust.stdout.trim(),
    cargo: cargo.stdout.trim(),
    lean: acceptance.lean,
  },
  bounds: {
    search_seed: verification.search_seed,
    search_vectors: verification.search_vectors,
    lean_timeout_ms: 60000,
    proof_bytes: 16384,
  },
  phase2_rerun: {
    fixtures: phase2.fixtures,
    verified: phase2.verified,
    conflicts: phase2.conflicts,
    counterexamples: phase2.counterexamples,
    incompatible: phase2.incompatible,
    abnormal: phase2.abnormal,
    inputs: phase2.inputs,
    decision: phase2.decision,
  },
  search: {
    outcome: verification.search_outcome,
    subject_candidate: verification.subject_candidate,
  },
  claims,
  refusals: {
    producer_fields: "refused: stale subject, wrong claim, assumptions, claim hash, trust flag, revision, schema, premarked outcome",
    interfaces: "refused: incompatible interface, vacuous precondition, empty input domain",
    observables: "refused: non-I64 observable corpus (unit, bool, text)",
    proofs: `wrong proof: ${acceptance.wrong_proof}; command submission: ${acceptance.command_submission}`,
  },
  outcomes: verification.outcomes,
  lean_axioms: acceptance.axioms,
  library: {
    digest: acceptance.library_digest,
    root: acceptance.library_root,
  },
  decision: "no-go",
  decision_reasons: [
    "SM-4 is narrowed, not discharged: the isolated check is a fresh pinned Lean process, not the production sandboxed consumer, and no production claim path is wired.",
    "The separately reviewed integration plan is pending the independent review scheduled by the owner (OWNER DECISION 2026-11-20 item 1).",
    "SM-7 stays separate: source-model proof and installed Wasm correspondence are different obligations.",
    "SM-8: this lane performs no merge, publication, live-slot admission, migration or release action.",
  ],
  proof_obligations: {
    "SM-1": "proved (NobleContracts.MergeRel, mergeVal_*, mergeStack_*)",
    "SM-2": "proved for the narrowed fragment (NobleContracts.outer_exec, mergeRel_terminating)",
    "SM-3": "narrowed and tested (consumer regeneration over lowered MC1 subjects)",
    "SM-4": "narrowed (pinned process-level check); production consumer not wired",
    "SM-5": "tested (every differing producer field refused)",
    "SM-6": "tested (total outcome lattice; only Lean acceptance yields verified)",
    "SM-7": "documented separate",
    "SM-8": "documented: no automatic action follows",
  },
  elapsed_ms: Date.now() - started,
};

await Bun.write(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`);
console.log(
  `semantic-merge verification receipt: ${claims.length} kernel-checked claims, decision=${receipt.decision}, revision=${revision}`,
);
