#!/usr/bin/env node
/**
 * Import compiler-lab's Relax live-update comparison into the revision-2 corpus (#119).
 *
 * Usage (pinned Node, absolute paths, fresh output directory outside the repository):
 *   node verification/live-update-corpus/import-relax.mjs \
 *     NOBLE_EXPORT_DIRECTORY RELAX_RUN_DIRECTORY RELAX_REPLAY_DIRECTORY \
 *     COMPILER_LAB_CHECKOUT FRESH_DIRECTORY
 *
 * NOBLE_EXPORT_DIRECTORY is the output of `export.mjs` at this commit. RELAX_RUN_DIRECTORY is the
 * `--out` of `compiler-lab validate relax-live --corpus NOBLE_EXPORT_DIRECTORY/corpus.json`, and
 * RELAX_REPLAY_DIRECTORY the `--out` of the same command with `--replay RELAX_RUN_DIRECTORY`.
 *
 * What it does, in order:
 *   1. refuses a dirty tree, and an export that is not from this commit or does not verify here;
 *   2. refuses a compiler-lab checkout other than the frozen revision with the frozen profile;
 *   3. refuses a run that consumed other bytes than the export, and a replay of another run;
 *   4. copies the export byte for byte and retains the compiler-lab files with local paths replaced
 *      by placeholders and quoted fixture source lines withheld, refusing any file that still names
 *      a local path or this host;
 *   5. derives the comparison corpus and receipt, runs the comparison controls, and re-verifies
 *      the written evidence from disk.
 */
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {
  COMPARISON_RECEIPT_SCHEMA, ROOT, readJson, sha256, verifyEvidence,
} from './corpus.mjs';
import {
  COMPILER_LAB, PLACEHOLDERS, RELAX_PROFILE, SOURCE_WITHHELD, bindReceipt, privateMatches,
  quotesSource, redactSourceExcerpts, retentionPlan, scrub, sealComparison, verifyComparison,
} from './comparison.mjs';
import { runComparisonControls } from './controls.mjs';

const UNSAFE_STARTUP = /^(?:NODE_OPTIONS|NODE_PATH|BASH_ENV|ENV|LD_.*|DYLD_.*|NIX_LD(?:_.*)?|GLIBC_TUNABLES)$/u;
const unsafe = Object.keys(process.env).filter(name => UNSAFE_STARTUP.test(name));
assert.deepEqual(unsafe, [], `unsafe import startup environment: ${unsafe.join(', ')}`);
assert.equal(process.argv.length, 7, 'usage: node import-relax.mjs NOBLE_EXPORT_DIRECTORY '
  + 'RELAX_RUN_DIRECTORY RELAX_REPLAY_DIRECTORY COMPILER_LAB_CHECKOUT FRESH_DIRECTORY');
const [exportDirectory, runDirectory, replayDirectory, checkout, output] = process.argv.slice(2);
for (const argument of [exportDirectory, runDirectory, replayDirectory, checkout, output])
  assert.ok(path.isAbsolute(argument) && path.resolve(argument) === argument,
    `${argument} must be an absolute, normalized path`);
for (const directory of [exportDirectory, runDirectory, replayDirectory, checkout])
  assert.ok(fs.statSync(directory).isDirectory(), `${directory} is not a directory`);
const repositoryRoot = fs.realpathSync(ROOT);
const resolvedOutput = path.join(fs.realpathSync(path.dirname(output)), path.basename(output));
assert.ok(resolvedOutput !== repositoryRoot
  && !resolvedOutput.startsWith(`${repositoryRoot}${path.sep}`),
  'the comparison directory must be outside the repository: the tree stays clean while importing');
assert.ok(!fs.existsSync(output), 'the comparison directory must not exist');
const tempRoot = process.env.TMPDIR;
assert.ok(typeof tempRoot === 'string' && path.isAbsolute(tempRoot),
  'TMPDIR must name the private scratch directory assigned to this import');
const tempInfo = fs.statSync(tempRoot);
assert.ok(tempInfo.isDirectory() && tempInfo.uid === process.getuid()
  && (tempInfo.mode & 0o077) === 0, 'TMPDIR must be a private directory of the current user');

const git = process.env.NOBLE_GIT ?? 'git';
const gitIn = (directory, ...args) => execFileSync(git, ['-C', directory, ...args],
  { encoding: 'utf8' }).trim();
assert.equal(gitIn(ROOT, 'status', '--porcelain'), '',
  'the comparison must be imported from a clean committed tree');
const commit = gitIn(ROOT, 'rev-parse', 'HEAD');
const tree = gitIn(ROOT, 'rev-parse', 'HEAD^{tree}');
const gitVersion = gitIn(ROOT, '--version');
const startedAt = new Date().toISOString();

// 1. The export is this commit's and verifies against this tree.
const exportVerification = verifyEvidence({ directory: exportDirectory, repositoryRoot: ROOT });
assert.deepEqual(exportVerification.failures, [],
  `the Noble export does not verify:\n${exportVerification.failures.map(row => `${row.code} ${row.target}: ${row.detail}`).join('\n')}`);
const exportReceipt = readJson(path.join(exportDirectory, 'receipt.json'));
assert.equal(exportReceipt.source?.commit, commit, 'the export must come from the importing commit');
assert.equal(exportReceipt.controls?.passed, true, 'the export controls must have passed');
const exportCorpusBytes = fs.readFileSync(path.join(exportDirectory, 'corpus.json'));

// 2. The compiler-lab checkout is the frozen revision with the frozen Relax profile.
assert.equal(gitIn(checkout, 'rev-parse', 'HEAD'), COMPILER_LAB.commit,
  'the compiler-lab checkout is not the frozen revision');
assert.equal(gitIn(checkout, 'status', '--porcelain', '--untracked-files=no'), '',
  'the compiler-lab checkout has tracked changes');
assert.equal(sha256(fs.readFileSync(path.join(checkout, RELAX_PROFILE.path))), RELAX_PROFILE.sha256,
  'the compiler-lab checkout carries another Relax profile');

// 3. The run consumed exactly the export, and the replay replays that run.
const runReport = fs.readFileSync(path.join(runDirectory, 'report.json'));
const report = JSON.parse(runReport.toString('utf8'));
assert.ok(fs.readFileSync(path.join(runDirectory, 'noble', 'corpus.json')).equals(exportCorpusBytes),
  'the Relax run consumed other bytes than the export corpus');
assert.equal(report.corpus?.sha256, sha256(exportCorpusBytes),
  'the Relax report binds another corpus digest');
const replayReport = JSON.parse(fs.readFileSync(path.join(replayDirectory, 'report.json'), 'utf8'));
assert.ok(fs.readFileSync(path.join(replayDirectory, 'replay', 'retained-report.json')).equals(runReport),
  'the replay retained another report than the Relax run');
assert.equal(fs.realpathSync(replayReport.replay_of ?? '/nonexistent-replay'),
  fs.realpathSync(runDirectory), 'the replay names another run directory');

// 4. Copy the export and retain the compiler-lab files without local paths.
fs.mkdirSync(output, { mode: 0o700 });
fs.cpSync(exportDirectory, path.join(output, 'export'), { recursive: true });
const literals = new Map();
const addLiteral = (literal, placeholder) => {
  for (const form of new Set([literal, fs.realpathSync(literal)]))
    if (form.length > 1) literals.set(form, placeholder);
};
addLiteral(runDirectory, PLACEHOLDERS.run);
addLiteral(replayDirectory, PLACEHOLDERS.replay);
addLiteral(exportDirectory, PLACEHOLDERS.export);
addLiteral(checkout, PLACEHOLDERS.checkout);
addLiteral(os.homedir(), PLACEHOLDERS.home);
const substitutions = [...literals.entries()].sort(([left], [right]) => right.length - left.length);
const hostname = os.hostname().toLowerCase();
const retained = [];
for (const entry of retentionPlan(report)) {
  const sourceRoot = entry.source === 'run' ? runDirectory : replayDirectory;
  const originalPath = `${entry.source}/${entry.path}`;
  const original = fs.readFileSync(path.join(sourceRoot, entry.path));
  const scrubbed = scrub(original, substitutions);
  const redacted = redactSourceExcerpts(entry.path, scrubbed.bytes.toString('utf8'));
  const text = redacted.text;
  const retainedBytes = Buffer.from(text, 'utf8');
  const leaks = privateMatches(text);
  assert.deepEqual(leaks, [], `${originalPath} still carries a local path (${leaks.join(', ')})`);
  assert.ok(!quotesSource(entry.path, text), `${originalPath} still quotes fixture source`);
  assert.ok(hostname.length === 0 || !text.toLowerCase().includes(hostname),
    `${originalPath} names the producing host`);
  for (const [literal] of substitutions)
    assert.ok(!text.includes(literal), `${originalPath} still carries a substituted path`);
  const relative = `relax/${originalPath}`;
  fs.mkdirSync(path.dirname(path.join(output, relative)), { recursive: true, mode: 0o700 });
  fs.writeFileSync(path.join(output, relative), retainedBytes, { mode: 0o600 });
  retained.push({
    path: relative, role: entry.role, sha256: sha256(retainedBytes), bytes: retainedBytes.length,
    original_path: originalPath, original_sha256: sha256(original), original_bytes: original.length,
    substitutions: Object.fromEntries(Object.entries({ ...scrubbed.counts,
      [SOURCE_WITHHELD]: redacted.count }).filter(([, count]) => count > 0)),
  });
}

// 5. Derive, control and verify.
const receiptBase = {
  schema: COMPARISON_RECEIPT_SCHEMA,
  claim: 'comparison of the bounded Noble live-update export with compiler-lab\'s pinned Makepad Relax lane over the same scenarios, each runtime classified under its own frozen profile; experimental evidence, not a proof, not a conformance promotion and not a model-to-implementation claim',
  source: { commit, tree, clean: true, git: gitVersion },
  pins: readJson(path.join(exportDirectory, 'corpus.json')).pins,
};
const wallTimes = () => {
  const endedAt = new Date().toISOString();
  return { started_at: startedAt, ended_at: endedAt,
    duration_ms: Math.max(0, Date.parse(endedAt) - Date.parse(startedAt)) };
};
const generatedAt = new Date().toISOString();
const sealed = sealComparison({ directory: output, retained, generatedAt,
  receiptBase: { ...receiptBase, timing: wallTimes() } });
const controls = runComparisonControls({ directory: output, repositoryRoot: ROOT });
const receipt = bindReceipt({ ...sealed.receipt,
  timing: { ...wallTimes(), bound_to: sealed.receipt.timing.bound_to }, controls });
fs.writeFileSync(path.join(output, 'receipt.json'), `${JSON.stringify(receipt, null, 2)}\n`,
  { mode: 0o600 });

assert.equal(gitIn(ROOT, 'status', '--porcelain'), '',
  'the repository tree changed during the import; the evidence is not source-bound');
const verification = verifyComparison({ directory: output, repositoryRoot: ROOT });
process.stdout.write(`${JSON.stringify({
  comparison: output,
  source_commit: commit,
  compiler_lab_commit: COMPILER_LAB.commit,
  corpus_sha256: receipt.corpus.sha256,
  scenarios: receipt.scenarios.map(row => ({ case_id: row.case_id, noble: row.noble.result_class,
    relax: row.relax.result_class,
    settings: row.relax.admission_settings.map(setting => `${setting.admission}:${setting.result_class}`) })),
  result_class_counts: receipt.result_class_counts,
  retained: retained.length,
  controls: { passed: controls.passed, count: controls.controls.length },
  verification: { failures: verification.failures.length,
    codes: [...new Set(verification.failures.map(row => row.code))].sort() },
}, null, 2)}\n`);
if (!controls.passed || verification.failures.length > 0) {
  for (const failure of verification.failures)
    process.stderr.write(`[verify] ${failure.code} ${failure.target}: ${failure.detail}\n`);
  for (const control of controls.controls.filter(row => !row.passed))
    process.stderr.write(`[control] ${control.control}: expected ${control.expected_reason}, observed ${control.observed_reasons.join(',')}\n`);
  process.exitCode = 1;
}
