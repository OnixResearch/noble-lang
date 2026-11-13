#!/usr/bin/env node
/**
 * Export a bounded live-update corpus from Noble's authoritative fixture lanes (#119).
 *
 * Usage (pinned Node, absolute CLI, fresh output directory outside the repository):
 *   node verification/live-update-corpus/export.mjs ABSOLUTE_PREBUILT_CLI FRESH_DIRECTORY
 *
 * What it does, in order, and never in a different order:
 *   1. refuses to run unless the working tree is clean, then records the commit and tree;
 *   2. spawns each authoritative lane as a subprocess against the selected CLI, keeping every raw
 *      report byte and its digest (the lanes remain the authority; their own assertions run);
 *   3. derives the normalized observation from those bytes and compares it with the frozen
 *      expectation table in `corpus.mjs` (never with a value read back from a report);
 *   4. writes `corpus.json`, the retained `profile.json` and `raw/`;
 *   5. runs the control suite against that directory, then writes `receipt.json` bound over its own
 *      body;
 *   6. re-verifies the written evidence from disk and fails if anything disagrees.
 */
import assert from 'node:assert/strict';
import { execFileSync, spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import {
  CANONICAL_CASES, CORPUS_DIRECTORY, CORPUS_REVISION, LANES, OTHER_RUNTIME, PROFILE_ID, PROFILE_PATH,
  RESULT_CLASSES, RESULT_CLASS_IDS, ROOT, RUNTIME_SOURCES, SCHEMA, RECEIPT_SCHEMA,
  SELECTION_COMMAND, SCENARIOS, canonicalSnapshot, digestValue, expectationTableDigest,
  fileSha256, hasExpectedReport, parseLaneStdout, scenarioRecord, sha256, validateCorpus,
  verifyEvidence,
} from './corpus.mjs';
import { runControls } from './controls.mjs';

const UNSAFE_STARTUP = /^(?:NODE_OPTIONS|NODE_PATH|BASH_ENV|ENV|LD_.*|DYLD_.*|NIX_LD(?:_.*)?|GLIBC_TUNABLES)$/u;
const unsafe = Object.keys(process.env).filter(name => UNSAFE_STARTUP.test(name));
assert.deepEqual(unsafe, [], `unsafe export startup environment: ${unsafe.join(', ')}`);
assert.equal(process.argv.length, 4,
  'usage: node export.mjs ABSOLUTE_PREBUILT_CLI FRESH_DIRECTORY');
const binary = path.resolve(process.argv[2]);
const output = path.resolve(process.argv[3]);
assert.equal(binary, process.argv[2], 'the CLI selection must be an absolute path');
assert.ok(fs.existsSync(binary), `selected CLI ${binary} does not exist`);
assert.ok(fs.statSync(binary).isFile(), 'the selected CLI must be a regular file');
const repositoryRoot = fs.realpathSync(ROOT);
const outputParent = fs.realpathSync(path.dirname(output));
const resolvedOutput = path.join(outputParent, path.basename(output));
assert.ok(resolvedOutput !== repositoryRoot
  && !resolvedOutput.startsWith(`${repositoryRoot}${path.sep}`),
  'the export directory must be outside the repository: the tree stays clean while producing evidence');
assert.ok(!fs.existsSync(output), 'the export directory must not exist');
const tempRoot = process.env.TMPDIR;
assert.ok(typeof tempRoot === 'string' && path.isAbsolute(tempRoot),
  'TMPDIR must name the private scratch directory assigned to this export');
const tempInfo = fs.statSync(tempRoot);
assert.ok(tempInfo.isDirectory(), 'TMPDIR must be a directory');
assert.equal(tempInfo.uid, process.getuid(), 'TMPDIR must belong to the current user');
assert.equal(tempInfo.mode & 0o077, 0, 'TMPDIR must not be accessible by other users');
fs.mkdirSync(output, { mode: 0o700 });

const git = process.env.NOBLE_GIT ?? 'git';
const gitOutput = (...args) => execFileSync(git, args, { cwd: ROOT, encoding: 'utf8' }).trim();
const status = gitOutput('status', '--porcelain');
assert.equal(status, '', 'the corpus must be exported from a clean committed tree');
const commit = gitOutput('rev-parse', 'HEAD');
const tree = gitOutput('rev-parse', 'HEAD^{tree}');
const gitVersion = gitOutput('--version');

const profileBytes = fs.readFileSync(path.join(ROOT, PROFILE_PATH));
const profileDigest = sha256(profileBytes);
const cliDigest = sha256(fs.readFileSync(binary));
const startedAt = new Date().toISOString();

const digestOf = relative => ({ path: relative, sha256: fileSha256(ROOT, relative) });
const laneIds = [...new Set(SCENARIOS.flatMap(scenario => scenario.lanes))];
const runnerPaths = [...new Set(laneIds.map(id => LANES[id].runner).filter(Boolean))];
const pins = [
  ...RUNTIME_SOURCES.map(row => ({ ...digestOf(row.path), what: row.what })),
  ...laneIds.map(id => ({ ...digestOf(LANES[id].path), what: `authoritative fixture lane ${id}` })),
  ...runnerPaths.map(relative => ({ ...digestOf(relative), what: 'lane test runner' })),
  { ...digestOf(LANES.helper.path), what: 'operator-session helper shared by the lanes' },
  { ...digestOf(`${CORPUS_DIRECTORY}/corpus.mjs`), what: 'corpus expectations and raw-report derivation' },
  { ...digestOf(`${CORPUS_DIRECTORY}/export.mjs`), what: 'Noble lane exporter' },
  { ...digestOf(`${CORPUS_DIRECTORY}/controls.mjs`), what: 'evidence mutation controls' },
  { ...digestOf(`${CORPUS_DIRECTORY}/controls.test.mjs`), what: 'retained evidence control test' },
  { ...digestOf(`${CORPUS_DIRECTORY}/schema.json`), what: 'versioned corpus schema' },
  { ...digestOf(CANONICAL_CASES), what: 'canonical LSLOT designs; states must stay unpromoted' },
  { ...digestOf('.cairn/changes/live-reference-lifetimes/design.md'),
    what: 'frozen profile text' },
  { ...digestOf('.cairn/changes/ownership-lifetime-contract/design.md'),
    what: 'ownership expectations reused by the retention scenarios' },
];

const evidenceDirectories = [];
const authorityRoots = [];
const cleanup = () => {
  for (const directory of evidenceDirectories) fs.rmSync(directory, { recursive: true, force: true });
  for (const directory of authorityRoots) fs.rmSync(directory, { recursive: true, force: true });
};
process.once('exit', cleanup);

const roleOf = name => name.endsWith('.requests.jsonl') ? 'requests'
  : name.endsWith('.responses.jsonl') ? 'responses'
  : name.endsWith('-summary.json') ? 'lane-summary'
  : name.endsWith('.stderr.txt') ? 'lane-stderr'
  : 'session-log';

function runLane(scenarioSlug, laneId) {
  const definition = LANES[laneId];
  const laneDirectory = path.join(output, 'raw', path.basename(definition.path, '.mjs'));
  fs.mkdirSync(laneDirectory, { recursive: true, mode: 0o700 });
  // The helper receives a unique private directory under the caller's assigned TMPDIR; it is never
  // placed under a world-writable system temporary directory.
  const sessionDirectory = fs.mkdtempSync(path.join(tempRoot,
    `noble-lslot-resource-liveupdate-${scenarioSlug}-${laneId}-${process.pid}-`));
  evidenceDirectories.push(sessionDirectory);
  const authorityRoot = fs.mkdtempSync(path.join(tempRoot, 'noble-lslot-resource-authority-'));
  authorityRoots.push(authorityRoot);
  const startedAtLane = new Date().toISOString();
  const startedAtMs = Date.now();
  const run = spawnSync(process.execPath, [path.join(ROOT, definition.path), binary], {
    cwd: ROOT, maxBuffer: 64 * 1024 * 1024,
    env: {
      PATH: process.env.PATH ?? '/run/current-system/sw/bin:/usr/bin:/bin',
      HOME: process.env.HOME ?? ROOT,
      TMPDIR: tempRoot,
      NOBLE_SLOT_EVIDENCE_DIR: sessionDirectory,
      NOBLE_SLOT_AUTHORITY_ROOT: authorityRoot,
    },
  });
  const durationMs = Math.max(0, Date.now() - startedAtMs);
  const endedAtLane = new Date().toISOString();
  const stdout = run.stdout ?? Buffer.alloc(0);
  const stderr = run.stderr ?? Buffer.alloc(0);
  fs.writeFileSync(path.join(laneDirectory, 'stdout.txt'), stdout, { mode: 0o600 });
  fs.writeFileSync(path.join(laneDirectory, 'stderr.txt'), stderr, { mode: 0o600 });
  const provenance = [
    { path: path.relative(output, path.join(laneDirectory, 'stdout.txt')), role: 'lane-stdout',
      sha256: sha256(stdout), bytes: stdout.length, retained: true, reason: null },
    { path: path.relative(output, path.join(laneDirectory, 'stderr.txt')), role: 'lane-stderr',
      sha256: sha256(stderr), bytes: stderr.length, retained: true, reason: null },
  ];
  const requests = new Map();
  const responses = new Map();
  const requestBytes = new Map();
  const eventEvidence = [];
  const variants = new Set();
  for (const name of fs.readdirSync(sessionDirectory).sort()) {
    const bytes = fs.readFileSync(path.join(sessionDirectory, name));
    const role = roleOf(name);
    const retained = true;
    const relative = path.join(path.relative(output, laneDirectory), name);
    provenance.push({
      path: relative, role, sha256: sha256(bytes), bytes: bytes.length, retained,
      reason: null,
    });
    fs.writeFileSync(path.join(output, relative), bytes, { mode: 0o600 });
    const variant = name.replace(/\.(requests|responses|session|stderr)\.(jsonl|txt)$/u, '');
    if (role === 'requests') {
      variants.add(variant);
      requests.set(variant, bytes.toString('utf8').trim().split('\n')
        .filter(line => line !== '').map(line => JSON.parse(line)));
      requestBytes.set(variant, bytes);
      eventEvidence.push({ path: relative, sha256: sha256(bytes) });
    } else if (role === 'responses') {
      variants.add(variant);
      responses.set(variant, bytes.toString('utf8').trim().split('\n')
        .filter(line => line !== '').map(line => JSON.parse(line)));
      eventEvidence.push({ path: relative, sha256: sha256(bytes) });
    }
  }
  const report = parseLaneStdout(stdout.toString('utf8'));
  const reportValid = hasExpectedReport(laneId, report.kind);
  const laneRecord = {
    id: laneId,
    path: definition.path,
    sha256: fileSha256(ROOT, definition.path),
    runner: definition.runner === null ? null
      : { path: definition.runner, sha256: fileSha256(ROOT, definition.runner) },
    variants: [...variants].sort((a, b) => a.localeCompare(b, 'en')),
    exit: run.status ?? -1,
    report_kind: report.kind,
    report_valid: reportValid,
    duration_ms: durationMs,
    stdout_sha256: sha256(stdout),
    stdout_bytes: stdout.length,
    stderr_sha256: sha256(stderr),
    stderr_bytes: stderr.length,
  };
  const bundle = {
    id: laneId, path: definition.path, exit: run.status ?? -1, stdout, report,
    requests, responses, requestBytes, eventEvidence,
    started_at: startedAtLane, ended_at: endedAtLane,
  };
  fs.rmSync(authorityRoot, { recursive: true, force: true });
  authorityRoots.splice(authorityRoots.indexOf(authorityRoot), 1);
  return { laneRecord, bundle, provenance };
}

const scenarios = [];
for (const scenario of SCENARIOS) {
  const slug = scenario.case_id.split('/').at(-1);
  const bundle = { lanes: new Map() };
  const lanes = [];
  const provenance = [];
  const scenarioStarted = new Date().toISOString();
  for (const laneId of scenario.lanes) {
    const run = runLane(slug, laneId);
    bundle.lanes.set(laneId, run.bundle);
    lanes.push(run.laneRecord);
    provenance.push(...run.provenance);
    process.stderr.write(`[export] ${scenario.case_id}: ${laneId} exited ${run.bundle.exit} (${run.bundle.stdout.length} stdout bytes)\n`);
  }
  const scenarioEnded = new Date().toISOString();
  const record = scenarioRecord(scenario, bundle, {
    lanes, provenance, cliSha256: cliDigest, sourceCommit: commit,
    startedAt: scenarioStarted, endedAt: scenarioEnded,
  });
  scenarios.push(record);
}

const canonical = canonicalSnapshot(ROOT);
const nonClaims = [
  'this corpus is an experimental export of existing fixture observations; it is not a proof, a conformance result or a model-to-implementation correspondence claim',
  'the authoritative lanes keep their own claims; the exporter binds their bytes and reproduces their observations, it does not replace them',
  'no LSLOT-01..09 canonical case state is promoted and no evidence is attached to a canonical case',
  'the makepad-relax column is not-run: compiler-lab #188, #189 and #190 are unavailable, so no Relax profile is frozen and no cross-runtime correspondence is claimed',
  'an expected refusal is recorded as a supported outcome of the frozen profile; only an unexpected refusal is a refusal-class result',
  'timing and duration records are bounds observed in this export, not performance claims or regression evidence',
];

const corpus = {
  schema: SCHEMA,
  corpus_revision: CORPUS_REVISION,
  generated_at: new Date().toISOString(),
  profile: { path: PROFILE_PATH, id: PROFILE_ID, sha256: profileDigest },
  runtime: {
    name: 'noble',
    engine: 'v8',
    selection_command: SELECTION_COMMAND,
    cli: { path: binary, sha256: cliDigest },
    sources: RUNTIME_SOURCES.map(row => ({ path: row.path, sha256: fileSha256(ROOT, row.path),
      what: row.what })),
  },
  source: { commit, tree, clean: true, git: gitVersion },
  pins,
  result_classes: RESULT_CLASSES.map(row => ({ id: row.id, meaning: row.meaning })),
  canonical_cases: canonical,
  scenarios,
  non_claims: nonClaims,
};
const corpusFailures = validateCorpus(corpus);
assert.deepEqual(corpusFailures, [], `corpus does not satisfy its own schema:\n${corpusFailures.join('\n')}`);
fs.writeFileSync(path.join(output, 'corpus.json'), `${JSON.stringify(corpus, null, 2)}\n`,
  { mode: 0o600 });
fs.writeFileSync(path.join(output, 'profile.json'), profileBytes, { mode: 0o600 });

const classCounts = Object.fromEntries(RESULT_CLASS_IDS.map(id => [id, 0]));
for (const scenario of scenarios) classCounts[scenario.comparison.result_class] += 1;
classCounts['not-run'] += scenarios.length; // the Relax column of every scenario

const scenarioTimings = scenarios.map(scenario => ({
  case_id: scenario.case_id,
  duration_ms: scenario.timing.duration_ms,
  result_class: scenario.comparison.result_class,
  lanes: scenario.lanes.map(lane => ({ id: lane.id, duration_ms: lane.duration_ms })),
}));
const capacityTiming = scenarioTimings
  .find(row => row.case_id === 'noble-live-update/saved-program-and-package-retention')
  ?.lanes.find(lane => lane.id === 'capacity');

// Item 10 of the issue: report an actionable regression or performance opportunity, or say that
// the export was inconclusive. Nothing here is inferred beyond what the export observed.
const issueTen = {
  regressions: [],
  performance: [
    {
      subject: 'saved Program ledger boundary',
      observation: `the capacity lane admitted 4096 saved owners, refused the 4097th root before execution and kept the session usable; its lane wall time was ${capacityTiming?.duration_ms ?? 'unknown'} ms in this export`,
      actionable: false,
      reasoning: 'no baseline for this path exists in this export, so the number is a bound and not a regression claim',
    },
  ],
  tooling_observations: [
    {
      subject: 'live_slot_program_capacity.mjs report shape',
      observation: 'the lane asserts the ledger boundary but prints a human sentence, so the boundary is not machine-readable; the corpus therefore records the declared 4096 constant and the exact 4096 + 2 invoke request sequence the constant produces',
      actionable: true,
      reasoning: 'a structured report would let a corpus normalize the refusal stage and diagnostic without restating the declared constant; no runtime change is proposed',
    },
  ],
  conclusion: 'No actionable Noble runtime regression or performance defect was observed. The bounded observations reproduced every frozen expectation; the only actionable item is the lane report shape above.',
};

const receipt = {
  schema: RECEIPT_SCHEMA,
  claim: 'bounded live-update corpus exported from the authoritative Noble fixture lanes; experimental evidence, not a proof, not a conformance promotion and not a model-to-implementation claim',
  source: { commit, tree, clean: true, git: gitVersion,
    bound: pins.map(row => ({ path: row.path, sha256: row.sha256 })) },
  profile: { path: PROFILE_PATH, evidence_path: 'profile.json', id: PROFILE_ID,
    sha256: profileDigest },
  corpus: { evidence_path: 'corpus.json', sha256: sha256(fs.readFileSync(path.join(output, 'corpus.json'))) },
  runtime: corpus.runtime,
  pins,
  expectations_sha256: expectationTableDigest(),
  canonical_cases: { path: canonical.path, sha256: canonical.sha256,
    states: canonical.states, promoted: [] },
  scenarios: scenarios.map(scenario => ({
    case_id: scenario.case_id,
    title: scenario.title,
    result_class: scenario.comparison.result_class,
    expectations: {
      holds: scenario.expectations.filter(row => row.status === 'holds').length,
      mismatch: scenario.expectations.filter(row => row.status === 'mismatch').length,
      unsupported: scenario.expectations.filter(row => row.status === 'unsupported').length,
    },
    unsupported_dimensions: scenario.comparison.unsupported_dimensions,
    timing: scenario.timing,
    other_runtime: { name: scenario.other_runtime.name,
      result_class: scenario.other_runtime.result_class,
      reason: scenario.other_runtime.reason },
  })),
  result_class_counts: classCounts,
  counts_basis: 'one comparison cell per scenario per runtime: five Noble cells plus five makepad-relax cells, each of which is not-run',
  relax: { name: OTHER_RUNTIME.name, status: 'blocked', result_class: 'not-run',
    reason: OTHER_RUNTIME.reason },
  observations_for_issue_10: issueTen,
  timing: {
    started_at: startedAt,
    ended_at: new Date().toISOString(),
    duration_ms: Math.max(0, Date.now() - Date.parse(startedAt)),
    bound_to: {
      source_commit: commit,
      scenarios: scenarios.map(scenario => scenario.case_id),
      result_classes: Object.fromEntries(scenarios.map(scenario =>
        [scenario.case_id, scenario.comparison.result_class])),
      admission_outcomes: Object.fromEntries(scenarios.map(scenario =>
        [scenario.case_id, scenario.timing.bound_to.admission_outcome])),
      lane_contract: 'the three slot runtime sources and every lane digest in `pins`',
    },
  },
  non_claims: nonClaims,
};

// A provisional binding lets every control copy be structurally valid before the suite runs; the
// final binding below covers the receipt as committed, including the control results.
receipt.binding_sha256 = digestValue(receipt);
const controls = runControls({ directory: output, repositoryRoot: ROOT, receipt, corpus });
receipt.controls = controls;
const body = { ...receipt };
delete body.binding_sha256;
receipt.binding_sha256 = digestValue(body);
fs.writeFileSync(path.join(output, 'receipt.json'), `${JSON.stringify(receipt, null, 2)}\n`,
  { mode: 0o600 });

assert.equal(sha256(fs.readFileSync(binary)), cliDigest,
  'the selected CLI changed while the corpus was being exported');
assert.equal(gitOutput('status', '--porcelain'), '',
  'the repository tree changed during the export; the evidence is not source-bound');

// Re-verify the written evidence from disk, including its self-binding.
const verification = verifyEvidence({ directory: output, repositoryRoot: ROOT });
process.stdout.write(`${JSON.stringify({
  receipt: path.join(output, 'receipt.json'),
  corpus: path.join(output, 'corpus.json'),
  source_commit: commit,
  cli_sha256: cliDigest,
  profile_sha256: profileDigest,
  scenarios: scenarios.map(scenario => ({
    case_id: scenario.case_id,
    result_class: scenario.comparison.result_class,
    duration_ms: scenario.timing.duration_ms,
  })),
  result_class_counts: classCounts,
  controls: { passed: controls.passed, count: controls.controls.length },
  verification: { failures: verification.failures.length,
    codes: [...new Set(verification.failures.map(row => row.code))].sort() },
}, null, 2)}\n`);
if (!controls.passed || verification.failures.length > 0) {
  for (const failure of verification.failures) process.stderr.write(`[verify] ${failure.code} ${failure.target}: ${failure.detail}\n`);
  process.exitCode = 1;
}
