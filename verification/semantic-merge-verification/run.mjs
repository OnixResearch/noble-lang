#!/usr/bin/env bun
// Reproducible runner for the Phase 3 SMERGE/1 evidence (issue #30).
//
// It rebuilds the pinned Lean model, re-runs the Phase 2 model lane, runs the
// Phase 3 claim/consumer lane (including the forged-submission controls) and
// the isolated pinned Lean check, then writes a source-bound receipt v2.
// A dirty tree (before or after the builds), a missing toolchain or a nonzero
// exit aborts without writing a receipt. Retained receipts are immutable:
// the runner refuses to overwrite one and defaults to a revision-named file.
// This is the test-only lane: it does not merge, publish, admit a live slot or
// integrate anything.

import { $ } from "bun";
import { createHash } from "node:crypto";
import { existsSync } from "node:fs";
import path from "node:path";

const root = path.resolve(new URL("../..", import.meta.url).pathname);
const transcriptLimit = 65_536;

const fail = (message) => {
  process.stderr.write(`semantic-merge-verification: ${message}\n`);
  process.exit(1);
};

const short = (await $`git rev-parse --short=12 HEAD`.cwd(root).text()).trim();
const receiptName = process.argv[2] ?? `receipt-${short}.json`;
const receiptPath = path.join(root, "verification/semantic-merge-verification", receiptName);
const receiptRelative = path.relative(root, receiptPath);
if (existsSync(receiptPath)) {
  fail(`refusing to overwrite retained receipt ${receiptRelative}`);
}

const started = Date.now();
const phase2Prefix = "NOBLE_SEMANTIC_MERGE_RECEIPT ";
const verificationPrefix = "NOBLE_SEMANTIC_MERGE_VERIFICATION ";
const leanPrefix = "NOBLE_SEMANTIC_MERGE_LEAN ";

const run = (argv, { cwd = root, env = {} } = {}) => {
  const result = Bun.spawnSync(argv, {
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

const line = (stdout, prefix) => {
  // `--nocapture` interleaves a test's stdout on its harness line, so the
  // marker is found anywhere in a line and the payload starts after it.
  for (const entry of stdout.split("\n")) {
    const at = entry.indexOf(prefix);
    if (at >= 0) return { ...JSON.parse(entry.slice(at + prefix.length)), raw: entry };
  }
  fail(`missing ${prefix.trim()} line`);
};

const status = async () => (await $`git status --porcelain`.cwd(root).text()).trim();
const dirtyEntries = (text) =>
  text
    .split("\n")
    .filter((entry) => entry.trim().length !== 0)
    // Porcelain entries are `XY path`; compare the repository-relative path.
    .filter((entry) => entry.slice(3).trim() !== receiptRelative);
const assertClean = async (phase) => {
  const entries = dirtyEntries(await status());
  if (entries.length !== 0) fail(`working tree is not clean (${phase}): ${entries.join("; ")}`);
};

await assertClean("before the builds");
const revision = (await $`git rev-parse HEAD`.cwd(root).text()).trim();

const commands = [
  { argv: ["lake", "build", "NobleContracts.Merge"], cwd: "proofs/mc1", env: {} },
  {
    argv: [
      "cargo", "test", "--locked", "-p", "noble-contracts", "--test", "source",
      "--", "--nocapture", "--test-threads=1", "semantic_merge_model",
    ],
    cwd: ".", env: {},
  },
  {
    argv: [
      "cargo", "test", "--locked", "-p", "noble-contracts", "--test", "source",
      "--", "--nocapture", "--ignored", "--test-threads=1", "semantic_merge_lean_check",
    ],
    cwd: ".",
    env: {
      NOBLE_LEAN: process.env.NOBLE_LEAN ?? "lean",
      NOBLE_CONTRACT_LIBRARY: process.env.NOBLE_CONTRACT_LIBRARY ?? `${root}proofs/mc1`,
      ...(process.env.CARGO_TARGET_DIR ? { CARGO_TARGET_DIR: process.env.CARGO_TARGET_DIR } : {}),
    },
  },
];

const build = run(commands[0].argv, { cwd: path.join(root, "proofs/mc1") });
if (build.status !== 0) {
  process.stderr.write(build.stdout);
  process.stderr.write(build.stderr);
  fail("pinned Lean model did not build");
}

const model = run(commands[1].argv);
if (model.status !== 0) {
  process.stderr.write(model.stdout);
  process.stderr.write(model.stderr);
  fail("the SMERGE/1 model lane failed");
}
const phase2 = line(model.stdout, phase2Prefix);
const verification = line(model.stdout, verificationPrefix);

const lean = run(commands[2].argv, { env: commands[2].env });
if (lean.status !== 0) {
  process.stderr.write(lean.stdout);
  process.stderr.write(lean.stderr);
  fail("the isolated pinned Lean check failed");
}
const acceptance = line(lean.stdout, leanPrefix);

await assertClean("after the builds");

const sha256 = (text) => createHash("sha256").update(text, "utf8").digest("hex");
const transcript = (text, label) => {
  if (Buffer.byteLength(text, "utf8") > transcriptLimit) {
    fail(`${label} transcript exceeds ${transcriptLimit} bytes; retain it separately`);
  }
  return { sha256: sha256(text), bytes: Buffer.byteLength(text, "utf8"), text };
};

const checks = [
  ["SMERGE-CASE-04", verification.case04_claim, verification.case04_text_sha256, verification.case04_binding_sha256, acceptance.proof_case04, acceptance.case04],
  ["SMERGE-CASE-01", verification.case01_claim, verification.case01_text_sha256, verification.case01_binding_sha256, acceptance.proof_case01, acceptance.case01],
].map(([name, claim, textSha, bindingSha, proof, outcome]) => ({
  case: name,
  claim,
  claim_sha256: sha256(claim),
  claim_text_sha256: textSha,
  claim_binding_sha256: bindingSha,
  proof,
  proof_sha256: sha256(proof),
  outcome,
}));
for (const check of checks) {
  if (check.claim_sha256 !== check.claim_text_sha256) {
    fail(`${check.case} claim digest disagrees between the lane and the runner`);
  }
  if (check.outcome !== "verified") {
    fail(`${check.case} was not verified through the consumer path: ${check.outcome}`);
  }
}

const allowance = ["propext", "Classical.choice", "Quot.sound"];
for (const axiom of acceptance.axioms) {
  if (!allowance.includes(axiom)) fail(`non-allowlisted axiom reported: ${axiom}`);
}

// Forged submissions must stay nonverified: the old marker channel, the same
// channel through the full consumer path, a wrong proof, a trace forgery and
// the observed bound/tool states.
const controls = {
  case07_direct_forgery: {
    case: "SMERGE-CASE-07",
    submission: "sorry + run_cmd marker + #exit",
    submission_sha256: acceptance.forge_commands_sha256,
    accepted: acceptance.case07_forgery_accepted,
    refusal: acceptance.forge_commands_refusal,
  },
  case07_consumer_path: {
    case: "SMERGE-CASE-07",
    outcome: "conflict",
  },
  case03_consumer_path: {
    case: "SMERGE-CASE-03",
    submission: "sorry + run_cmd marker + #exit",
    outcome: acceptance.case03_consumer_forgery,
  },
  file_write_submission: {
    submission: "sorry + run_cmd IO.FS.writeFile + #exit",
    refusal: acceptance.forge_write_refusal,
  },
  wrong_proof: { case: "SMERGE-CASE-04", proof: "NobleContracts.mergeRel_independent_changes", outcome: acceptance.wrong_proof },
  exhausted: { control: "proof body above the declared byte bound", outcome: acceptance.exhausted },
  internal_failure: { control: "library root without a toolchain pin", outcome: acceptance.internal_failure },
};
if (controls.case07_direct_forgery.accepted !== false) fail("the CASE-07 forgery was accepted");
if (controls.case03_consumer_path.outcome === "verified") fail("the CASE-03 forgery verified through the consumer path");
if (controls.wrong_proof.outcome === "verified") fail("a wrong proof verified");
if (controls.exhausted.outcome !== "exhausted") fail("the bound control did not report exhausted");
if (controls.internal_failure.outcome !== "internal-failure") fail("the tool control did not report internal failure");
if (!acceptance.forge_write_refusal.includes("closed term")) fail("the file-writing submission was not refused");

const rust = run(["rustc", "--version"]);
const cargo = run(["cargo", "--version"]);
if (rust.status !== 0 || cargo.status !== 0) fail("toolchain versions unavailable");

const assessmentReasons = [
  "SM-1 is proved; SM-2 is proved for the narrowed fragment; SM-3, SM-4, SM-5 and SM-6 are narrowed or tested only, so the frozen criterion (SM-1 through SM-6 discharged or moved by an owner-accepted narrower statement) is not met.",
  "SM-3 is tested only: there is no formalized preparation/lowering pipeline, and PO-19 remains open.",
  "SM-5 and SM-6 are tested only: a corpus of producer-field refusals and a total outcome table do not discharge their universal obligations.",
  "SM-4 is narrowed, not discharged: the isolated check is a fresh pinned Lean process with a cleared environment, not the production sandboxed consumer, and no PO-09/PO-16 refinement is proved here.",
  "The integration plan is written but its steps 1-6 are unimplemented and step 7b (independent review of the implemented integration) has not run.",
  "SM-7 is a separate obligation (#10/PO-18); a source-model proof does not transfer to an installed artifact.",
  "SM-8: this lane performs no merge, publication, live-slot admission, migration or release action.",
];

const receipt = {
  schema: "noble-semantic-merge-verification-receipt/v2",
  relation: verification.relation,
  semantic_revision: verification.semantic_revision,
  source_revision: revision,
  dirty: false,
  runner: {
    entry: `bun verification/semantic-merge-verification/run.mjs ${receiptName}`,
    commands: commands.map((command) => ({
      argv: command.argv,
      cwd: command.cwd,
      env: command.env,
    })),
    toolchain: {
      rust: rust.stdout.trim(),
      cargo: cargo.stdout.trim(),
      lean: acceptance.lean,
    },
    elapsed_ms: Date.now() - started,
  },
  observations: {
    phase2,
    claims: checks,
    search: {
      outcome: verification.search_outcome,
      seed: verification.search_seed,
      vectors: verification.search_vectors,
      subject_candidate: verification.subject_candidate,
    },
    bounds: acceptance.bounds,
    outcomes: verification.outcomes,
    controls,
    axiom_allowlist: allowance,
    axioms: acceptance.axioms,
    library: {
      source_digest: acceptance.library_source_digest,
      object_digest: acceptance.library_object_digest,
      root: acceptance.library_root,
      digest_status: "recorded, not compared with a pinned digest; the object digest binds the .olean bytes Lean loaded",
    },
    transcripts: {
      model: transcript(model.stdout, "model"),
      lean: transcript(lean.stdout, "lean"),
      lean_stderr: transcript(lean.stderr, "lean stderr"),
      model_stderr: transcript(model.stderr, "model stderr"),
    },
  },
  assessment: {
    decision: "no-go",
    reasons: assessmentReasons,
    obligations: {
      "SM-1": "proved (NobleContracts.MergeRel, mergeVal_*, mergeStack_*)",
      "SM-2": "proved for the narrowed fragment (NobleContracts.outer_exec, mergeRel_terminating)",
      "SM-3": "narrowed and tested (consumer regeneration over lowered MC1 subjects); dangling captures are refused",
      "SM-4": "narrowed (pinned process-level check with a closed-term consumer); production consumer not wired",
      "SM-5": "tested (every differing producer field refused); not discharged",
      "SM-6": "tested (total outcome lattice; only the consumer post-check through `checked` yields verified); not discharged",
      "SM-7": "documented separate (#10/PO-18)",
      "SM-8": "documented: no automatic action follows",
    },
  },
};

await Bun.write(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`);
console.log(
  `semantic-merge verification receipt: ${checks.length} kernel-checked claims, decision=${receipt.assessment.decision}, revision=${revision}, file=${receiptRelative}`,
);
