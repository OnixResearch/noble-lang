#!/usr/bin/env node
// Compare an immutable Lean 4.31 pilot and a fresh Lean 4.35 multi-checker receipt against
// ONE independently selected source/profile. Never use a receipt to select its own trust anchors.
// Usage (from repository root, AFTER the current multi.mjs receipt was produced on a clean commit):
// node verification/independent-replay/receipt-admission.mjs \
//   --source-commit <full committed source SHA> \
//   --checkers-sha256 <independently selected checkers.json SHA-256> \
//   --statements-sha256 <independently selected statements.json SHA-256> \
//   --current <fresh multi.mjs v2 receipt> --output <new JSON file outside the repository>
// The retained 2026-10-29 receipt is fixed by its byte digest; both pass through admit().
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { fileSha256, sha256 } from './pins.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const repo = path.resolve(here, '../..');
const args = process.argv.slice(2);
const options = new Map();
for (let i = 0; i < args.length; i += 2) {
  if (!args[i]?.startsWith('--') || !args[i + 1] || options.has(args[i])) {
    throw new Error(`invalid or duplicate option at ${args[i] ?? '<end>'}`);
  }
  options.set(args[i], args[i + 1]);
}
for (const key of options.keys()) {
  if (!['--source-commit', '--checkers-sha256', '--statements-sha256', '--current', '--output'].includes(key)) {
    throw new Error(`unknown option ${key}`);
  }
}
function required(key) {
  const selected = options.get(key);
  if (!selected) throw new Error(`missing ${key}`);
  return selected;
}
function selectedSha(key) {
  const value = required(key);
  if (!/^[0-9a-f]{64}$/.test(value)) throw new Error(`${key} must be a full lowercase SHA-256`);
  return value;
}
const sourceCommit = required('--source-commit');
if (!/^[0-9a-f]{40}$/.test(sourceCommit)) throw new Error('--source-commit must be a full lowercase commit SHA');
const checkersSha = selectedSha('--checkers-sha256');
const statementsSha = selectedSha('--statements-sha256');
const checkersFile = path.join(here, 'checkers.json');
const statementsFile = path.join(here, 'statements.json');
const criteriaFile = path.join(here, 'batch9-criteria.json');
if (fileSha256(checkersFile) !== checkersSha || fileSha256(statementsFile) !== statementsSha) {
  throw new Error('selected checker or statement profile digest differs from the local file');
}
const checkers = JSON.parse(fs.readFileSync(checkersFile, 'utf8'));
const statements = JSON.parse(fs.readFileSync(statementsFile, 'utf8'));
const criteria = JSON.parse(fs.readFileSync(criteriaFile, 'utf8'));
if (criteria.schema !== 'noble-issue62-batch9-criteria/v1'
    || criteria.selected_lean !== checkers.toolchain.lean
    || criteria.selected_export_format !== checkers.export_format.version
    || statements.source?.lean !== criteria.selected_lean
    || !criteria.items?.historical
    || new Set(criteria.allowed_axioms).size !== 3
    || criteria.allowed_axioms.join('\0') !== ['propext', 'Classical.choice', 'Quot.sound'].join('\0')) {
  throw new Error('selected local profile does not satisfy frozen batch-nine criteria');
}
if (sourceCommit === criteria.source_commit_before_followup
    || criteria.items.source_builds.required_profiles.length !== checkers.checkers.length
    || criteria.items.source_builds.required_profiles.some((id) =>
      checkers.checkers.filter((checker) => checker.id === id).length !== 1)) {
  throw new Error('source must be a post-followup commit and every required checker must be selected');
}
const claimMap = new Map();
for (const claim of statements.claims) {
  if (!claim.root || claimMap.has(claim.root) || sha256(claim.statement) !== claim.statement_sha256) {
    throw new Error('statement profile has a duplicate root or a false statement digest');
  }
  claimMap.set(claim.root, claim.statement_sha256);
}
if (claimMap.size === 0 || checkers.checkers.length === 0
    || new Set(checkers.checkers.map((checker) => checker.id)).size !== checkers.checkers.length) {
  throw new Error('empty or duplicate selection in the statement/checker profile');
}
const historicFile = path.join(here, 'receipts/2026-10-29/receipt.json');
const historicalSha = '94706cc5884d765f864f4f48dbf04cd954b17c80b4bd0c94f914562931f43040';
const currentFile = path.resolve(required('--current'));
const output = path.resolve(required('--output'));
if (currentFile === historicFile || output === currentFile || output === historicFile
    || output === repo || output.startsWith(`${repo}${path.sep}`)) {
  throw new Error('inputs must be distinct and output must be new and outside the repository');
}

const selection = {
  source_commit: sourceCommit,
  criteria_sha256: fileSha256(criteriaFile),
  toolchain: {
    lean: criteria.selected_lean,
    githash: checkers.toolchain.githash,
    release: checkers.toolchain.output,
    lean_sha256: checkers.toolchain.lean_sha256,
    lake_sha256: checkers.toolchain.lake_sha256,
    leanexport_sha256: checkers.toolchain.leanexport_sha256,
  },
  export_format: criteria.selected_export_format,
  checker_profile_sha256: checkersSha,
  statement_profile_sha256: statementsSha,
  statements: Object.fromEntries(claimMap),
  checkers: Object.fromEntries(checkers.checkers.map((checker) => [checker.id, checker.sha256])),
};

// Do not infer an absent field from another field or from the expected profile. Missing evidence
// is an explicit mismatch, including on the historical receipt's older schema.
function extract(receipt) {
  if (receipt.schema === 'noble-independent-replay-export/v1') {
    return {
      source_commit: receipt.foundation?.fresh_build_source_commit,
      lean_version: receipt.tools?.lean?.version,
      lean_githash: receipt.tools?.lean?.revision,
      lean_release: receipt.tools?.lean?.binary?.replace(/\/bin\/lean$/, ''),
      lean_sha256: receipt.tools?.lean?.binary_sha256,
      leanexport_sha256: receipt.tools?.lean4export?.binary_sha256,
      format: receipt.tools?.lean4export?.format,
      checker_digests: { nanoda: receipt.tools?.nanoda_lib?.binary_sha256 },
      roots: receipt.exports?.combined?.roots,
      // Old textual statements are not canonical-profile hashes. In particular a matching root
      // NAME cannot stand in for a statement or closure binding.
      statement_digests: null,
      receipt,
    };
  }
  if (receipt.schema === 'noble-independent-replay-multi/v2') {
    const baseline = receipt.subjects?.filter((subject) => subject.kind === 'baseline');
    return {
      source_commit: receipt.source_commit,
      lean_version: receipt.toolchain_pins?.release?.match(/lean-release-(\d+\.\d+\.\d+)$/)?.[1],
      lean_githash: receipt.toolchain_pins?.githash,
      lean_release: receipt.toolchain_pins?.release,
      lean_sha256: receipt.toolchain_pins?.lean?.binary_sha256,
      lake_sha256: receipt.toolchain_pins?.lake?.binary_sha256,
      leanexport_sha256: receipt.toolchain_pins?.leanexport?.binary_sha256,
      format: baseline?.map((subject) => subject.export?.format),
      checker_profile_sha256: receipt.harness?.checkers?.sha256,
      statement_profile_sha256: receipt.harness?.manifest?.sha256,
      checker_digests: Object.fromEntries((receipt.toolchain_pins?.checkers ?? [])
        .map((checker) => [checker.id, checker.binary_sha256])),
      roots: baseline?.flatMap((subject) => subject.roots ?? []),
      statement_digests: receipt.statement_source?.manifest_check,
      baselines: baseline,
      receipt,
    };
  }
  return { receipt };
}
function admit(raw) {
  const receipt = JSON.parse(raw);
  const evidence = extract(receipt);
  const mismatches = [];
  const equal = (field, expected, observed) => {
    if (expected !== observed) mismatches.push({ field, expected, observed: observed ?? null,
      class: observed == null ? 'missing-evidence' : 'pin-mismatch' });
  };
  const fail = (field, observed, detail) => mismatches.push({ field, observed: observed ?? null, detail,
    class: 'invalid-evidence' });
  if (!['noble-independent-replay-export/v1', 'noble-independent-replay-multi/v2'].includes(receipt.schema)) {
    fail('schema', receipt.schema, 'unrecognized receipt schema');
  }
  equal('source_commit', selection.source_commit, evidence.source_commit);
  equal('toolchain.lean_version', selection.toolchain.lean, evidence.lean_version);
  // multi.mjs/v2 retains the release store path and exact Lean executable digest, but not
  // Lean's revision string. Do not manufacture a revision from the independently selected pin.
  // The legacy receipt *does* carry its revision, so compare that claim when it exists.
  if (evidence.lean_githash != null) {
    equal('toolchain.lean_githash', selection.toolchain.githash, evidence.lean_githash);
  }
  equal('toolchain.release', selection.toolchain.release, evidence.lean_release);
  if (receipt.schema === 'noble-independent-replay-multi/v2') {
    equal('toolchain.runner_release', selection.toolchain.release, receipt.toolchain);
    equal('harness.multi.sha256', fileSha256(path.join(here, 'multi.mjs')), receipt.harness?.multi?.sha256);
  }
  equal('toolchain.lean_sha256', selection.toolchain.lean_sha256, evidence.lean_sha256);
  equal('toolchain.lake_sha256', selection.toolchain.lake_sha256, evidence.lake_sha256);
  equal('toolchain.leanexport_sha256', selection.toolchain.leanexport_sha256, evidence.leanexport_sha256);
  equal('checker_profile_sha256', selection.checker_profile_sha256, evidence.checker_profile_sha256);
  equal('statement_profile_sha256', selection.statement_profile_sha256, evidence.statement_profile_sha256);
  for (const [id, digest] of Object.entries(selection.checkers)) {
    equal(`checkers.${id}.binary_sha256`, digest, evidence.checker_digests?.[id]);
  }
  const rootRows = Array.isArray(evidence.roots) ? evidence.roots : [];
  equal('roots.count', claimMap.size, rootRows.length);
  for (const root of claimMap.keys()) {
    equal(`roots.${root}`, true, rootRows.filter((item) => item === root).length === 1);
    const rows = evidence.statement_digests?.filter((row) => row.root === root) ?? [];
    equal(`statements.${root}.count`, 1, rows.length);
    equal(`statements.${root}.environment_sha256`, claimMap.get(root), rows[0]?.environment_sha256);
    equal(`statements.${root}.pinned_sha256`, claimMap.get(root), rows[0]?.pinned_sha256);
  }
  if (evidence.statement_digests && evidence.statement_digests.length !== claimMap.size) {
    fail('statement_source.manifest_check', evidence.statement_digests.length, 'unexpected statement rows');
  }
  const formats = Array.isArray(evidence.format) ? evidence.format : [evidence.format];
  if (formats.length === 0) fail('export.format', [], 'no baseline exports');
  for (let i = 0; i < formats.length; i++) equal(`exports[${i}].format`, selection.export_format, formats[i]);
  // The verdict is deliberately computed only AFTER the selected-source/profile comparisons.
  if (mismatches.length === 0) {
    equal('holds', true, receipt.holds);
    equal('pin_guard_passed', true, receipt.pin_guard_passed);
    if (!Array.isArray(evidence.baselines) || evidence.baselines.length !== claimMap.size) {
      fail('subjects', evidence.baselines?.length, 'missing baseline subjects');
    } else for (const subject of evidence.baselines) {
      if (!['baseline-large', 'baseline-small', 'baseline-fresh'].includes(subject.id)) {
        fail('subjects.id', subject.id, 'unexpected baseline subject');
      }
      equal(`subjects.${subject.id}.quorum.accepted`, true, subject.quorum?.accepted);
      equal(`subjects.${subject.id}.roots.count`, 1, subject.roots?.length);
      for (const axiom of subject.export?.axioms ?? []) {
        if (!criteria.allowed_axioms.includes(axiom)) fail(`subjects.${subject.id}.axioms`, axiom, 'unpermitted axiom');
      }
      if (!Array.isArray(subject.export?.axioms)) fail(`subjects.${subject.id}.axioms`, null, 'no exported axiom list');
      equal(`subjects.${subject.id}.export.sha256`, receipt.inputs?.[subject.id === 'baseline-large' ? 'large'
        : subject.id === 'baseline-small' ? 'small' : 'fresh']?.sha256, subject.export?.sha256);
      const runs = subject.runs ?? [];
      equal(`subjects.${subject.id}.runs.count`, checkers.checkers.length, runs.length);
      for (const checker of checkers.checkers) {
        const matches = runs.filter((run) => run.checker === checker.id);
        equal(`subjects.${subject.id}.${checker.id}.count`, 1, matches.length);
        const run = matches[0];
        equal(`subjects.${subject.id}.${checker.id}.classification`, 'accepted', run?.classification);
        equal(`subjects.${subject.id}.${checker.id}.exit_code`, 0, run?.exit_code);
        equal(`subjects.${subject.id}.${checker.id}.signal`, null, run?.signal);
        equal(`subjects.${subject.id}.${checker.id}.lane`, 'accepted', run?.lane?.verdict);
        if (!(run?.count > 0)) fail(`subjects.${subject.id}.${checker.id}.checked`, run?.count, 'no positive declaration count');
        const printed = run?.stdout?.match(new RegExp(checker.accept_pattern));
        if (!printed || Number(printed[1]) !== run?.count) {
          fail(`subjects.${subject.id}.${checker.id}.raw_stdout`, run?.stdout,
            'the recorded count is not present in the retained checker stdout');
        }
        if (!Number.isFinite(run?.wall_seconds) || !Number.isFinite(run?.max_rss_kb) || run.max_rss_kb <= 0) {
          fail(`subjects.${subject.id}.${checker.id}.resources`, null, 'missing wall time or positive max RSS');
        }
        for (const statement of run?.lane?.statements ?? []) {
          equal(`subjects.${subject.id}.${checker.id}.${statement.root}.statement`, claimMap.get(statement.root), statement.export_statement_sha256);
          equal(`subjects.${subject.id}.${checker.id}.${statement.root}.pin`, claimMap.get(statement.root), statement.pinned_sha256);
        }
        equal(`subjects.${subject.id}.${checker.id}.statement.count`, subject.roots?.length, run?.lane?.statements?.length);
      }
    }
    for (const [check, passed] of Object.entries(receipt.checks ?? {})) {
      if (passed !== true) fail(`checks.${check}`, passed, 'receipt check failed');
    }
    if (!receipt.checks || Object.keys(receipt.checks).length === 0) fail('checks', null, 'no producer checks recorded');
  }
  return { outcome: mismatches.length === 0 ? 'accepted' : 'receipt-refused',
    schema: receipt.schema ?? null, source_commit: evidence.source_commit ?? null,
    roots: rootRows, mismatches, checker_success_counted: mismatches.length === 0,
  };
}

function rawReceipt(file) {
  const fd = fs.openSync(file, fs.constants.O_RDONLY | fs.constants.O_NOFOLLOW | fs.constants.O_NONBLOCK);
  try {
    const before = fs.fstatSync(fd);
    if (!before.isFile() || before.size <= 0 || before.size > 16 * 1024 * 1024) {
      throw new Error('receipt is not a nonempty regular file within the 16 MiB limit');
    }
    const bytes = Buffer.allocUnsafe(before.size);
    let offset = 0;
    while (offset < bytes.length) {
      const count = fs.readSync(fd, bytes, offset, bytes.length - offset, null);
      if (count === 0) throw new Error('receipt changed or ended during bounded read');
      offset += count;
    }
    const after = fs.fstatSync(fd);
    if (after.size !== before.size || after.mtimeMs !== before.mtimeMs) {
      throw new Error('receipt changed during bounded read');
    }
    return { bytes, text: bytes.toString('utf8') };
  } finally {
    fs.closeSync(fd);
  }
}
const old = rawReceipt(historicFile);
if (sha256(old.bytes) !== historicalSha) {
  throw new Error('retained historical receipt bytes differ from the frozen 2026-10-29 digest');
}
const current = rawReceipt(currentFile);
const oldResult = admit(old.text);
const currentResult = admit(current.text);
const historicalIdentity = JSON.parse(old.text);
if (historicalIdentity.schema !== 'noble-independent-replay-export/v1'
    || historicalIdentity.tools?.lean?.version !== '4.31.0') {
  throw new Error('historical input is not the retained Lean 4.31 export receipt');
}
const holds = currentResult.outcome === 'accepted' && oldResult.outcome === 'receipt-refused'
  && oldResult.mismatches.some((row) => row.field === 'source_commit')
  && oldResult.mismatches.some((row) => row.field === 'toolchain.lean_version')
  && oldResult.mismatches.some((row) => row.field === 'toolchain.lean_sha256')
  && oldResult.mismatches.some((row) => row.field === 'checker_profile_sha256')
  && oldResult.mismatches.some((row) => row.field.startsWith('statements.'));
const report = {
  schema: 'noble-independent-replay-receipt-admission/v1', selection,
  holds, historical: oldResult, current: currentResult,
  raw_results: {
    // Exact original bytes survive even if the caller's scratch current-receipt path disappears.
    historical: { sha256: sha256(old.bytes), bytes: old.bytes.length, base64: old.bytes.toString('base64') },
    current: { sha256: sha256(current.bytes), bytes: current.bytes.length, base64: current.bytes.toString('base64') },
  },
  non_claims: ['A receipt is not a proof of source authenticity.',
    'Historical success is never rebound to the selected current cohort.',
    'The producer must be run on a clean committed tree; receipt admission cannot establish worktree cleanliness.',
    'Admission does not establish that a checker is sound or equivalent to Lean.'],
};
const fd = fs.openSync(output, 'wx', 0o600);
try { fs.writeFileSync(fd, `${JSON.stringify(report, null, 2)}\n`); } finally { fs.closeSync(fd); }
console.log(JSON.stringify({ holds, historical: oldResult.outcome, current: currentResult.outcome,
  historical_mismatches: oldResult.mismatches.map((row) => row.field), output }));
process.exitCode = holds ? 0 : 1;
