#!/usr/bin/env node
// Canonical LSLOT case gate: build the selected CLI from a committed clean
// tree, run the live-slot lanes that carry each case's observable behavior,
// project every canonical `input`/`expected`/variant field onto that evidence,
// and refuse any variant the lanes did not reach. A case is promotable only
// when every machine-expressible field is observed and equals the canonical
// value; a mismatch always fails.
//
// Usage: node verification/lslot-cases/gate.mjs ABSOLUTE_NEW_OUTPUT_DIRECTORY
// Environment: TMPDIR (lane evidence parents), NOBLE_LSLOT_TARGET_DIR
// (optional external Cargo target dir; default is a new directory under TMPDIR).
// Run it with the pinned Node selected by crates/noble-cli/src/core/runtime/config.json.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  root, sha, read, sourceInventory, revision, frozen,
} from '../dx06-current-source/source.mjs';
import {
  jsonLines, projectors, pending,
} from './projection.mjs';

const unsafeStartup = Object.keys(process.env).filter(name =>
  /^(?:NODE_OPTIONS|NODE_PATH|BASH_ENV|ENV|LD_.*|DYLD_.*|NIX_LD(?:_.*)?|GLIBC_TUNABLES|GCONV_PATH|SHLIB_PATH)$/.test(name));
assert.deepEqual(unsafeStartup, [], `unsafe gate startup environment: ${unsafeStartup.join(', ')}`);
assert.equal(fileURLToPath(import.meta.url),
  path.join(root, 'verification/lslot-cases/gate.mjs'));
assert.equal(process.argv.length, 3,
  'usage: node verification/lslot-cases/gate.mjs ABSOLUTE_NEW_OUTPUT_DIRECTORY');
const output = path.resolve(process.argv[2]);
assert.equal(output, process.argv[2], 'output directory must be an absolute path');
assert.ok(!fs.existsSync(output), 'output directory must not exist yet');
const relative = file => path.relative(root, file).split(path.sep).join('/');
fs.mkdirSync(output, { mode: 0o700 });
const save = (name, content) =>
  fs.writeFileSync(path.join(output, name), content, { flag: 'wx', mode: 0o600 });

const fixtureFile = 'specs/conformance/live-reference-cases.json';
const liveWasmFile = 'specs/conformance/live-wasm-cases.json';
const fixtureBytes = fs.readFileSync(path.join(root, fixtureFile));
const fixture = JSON.parse(fixtureBytes);
assert.deepEqual(fixture.cases.map(row => row.id),
  Array.from({ length: 9 }, (_, index) => `LSLOT-0${index + 1}`));
const caseById = id => {
  const rows = fixture.cases.filter(row => row.id === id);
  assert.equal(rows.length, 1, `${id} unique case`);
  return rows[0];
};
const designSha = id => {
  const row = caseById(id);
  return sha(JSON.stringify({ id: row.id, input: row.input, expected: row.expected }));
};

const inputs = [fixtureFile, liveWasmFile, 'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml',
  'policy/tool-selection.json', 'crates/noble-cli/Cargo.toml',
  'crates/noble-contracts/Cargo.toml', 'crates/noble-kernel/Cargo.toml',
  'crates/noble-wasm/Cargo.toml', 'crates/noble-cli/src/core/runtime/config.json',
  'verification/lslot-cases/gate.mjs', 'verification/lslot-cases/projection.mjs',
  'verification/dx06-current-source/source.mjs',
  'crates/noble-wasm/tests/live_slot_admission.rs',
  'crates/noble-contracts/tests/source_proof.rs',
  'crates/noble-cli/tests/live_slot.rs', 'crates/noble-cli/tests/live_repl.rs',
  'crates/noble-cli/tests/live_slot_resource_ref_cli.mjs',
  'crates/noble-cli/tests/live_slot_capture_legacy.mjs',
  'crates/noble-cli/tests/live_slot_late_control.mjs',
  'crates/noble-cli/tests/live_slot_selected_origin.mjs',
  'crates/noble-cli/tests/live_slot_nested_roots.mjs',
  'crates/noble-cli/tests/live_slot_resource_ref_admission.mjs',
  'crates/noble-cli/tests/live_slot_resource_ref_static.mjs',
  'crates/noble-cli/tests/live_slot_cas_policy.mjs',
  'crates/noble-cli/tests/live_slot_cas_policy_authority.json',
  'crates/noble-cli/tests/live_slot_legacy_refusal.mjs',
  'crates/noble-cli/tests/live_repl_slot_exclusion.mjs'];
const selectedTrees = ['crates/noble-kernel/src', 'crates/noble-contracts/src',
  'crates/noble-wasm/src', 'crates/noble-wasm/runtime', 'crates/noble-wasm/wit',
  'crates/noble-cli/src'];
const source = sourceInventory(inputs, selectedTrees);
const sourceRevision = revision(source);
save('source.json', JSON.stringify({ revision: sourceRevision, sha256: source }, null, 2) + '\n');
save('canonical-fixture.json', fixtureBytes);

const config = JSON.parse(fs.readFileSync(path.join(root,
  'crates/noble-cli/src/core/runtime/config.json')));
const selected = JSON.parse(fs.readFileSync(path.join(root, 'policy/tool-selection.json')));
const rustBin = path.join(selected.tool_paths.quality_rust.output, 'bin');
const cargo = path.join(rustBin, 'cargo');
const rustc = path.join(rustBin, 'rustc');
const node = config.tools.node.path;
const linker = selected.component_sync.linker_bin;
assert.equal(fs.realpathSync(process.execPath), fs.realpathSync(node),
  'run this gate with the pinned Node executable');
assert.equal(sha(fs.readFileSync(node)), sha(fs.readFileSync(process.execPath)));

const target = process.env.NOBLE_LSLOT_TARGET_DIR
  ? path.resolve(process.env.NOBLE_LSLOT_TARGET_DIR)
  : fs.mkdtempSync(path.join(os.tmpdir(), 'noble-lslot-target-'));
assert.ok(!target.startsWith(`${root}/`), 'the Cargo target directory must be external');
fs.mkdirSync(target, { recursive: true, mode: 0o700 });
const environment = Object.freeze({
  HOME: os.userInfo().homedir, TMPDIR: process.env.TMPDIR ?? os.tmpdir(),
  PATH: `${rustBin}:${linker}:/run/current-system/sw/bin:/usr/bin:/bin`,
  CARGO_HOME: path.join(os.userInfo().homedir, '.cargo'),
  CARGO_TARGET_DIR: target, CARGO_NET_OFFLINE: 'true', CARGO_BUILD_JOBS: '4',
  CARGO_INCREMENTAL: '0', RUSTC_WRAPPER: '', RUSTC_WORKSPACE_WRAPPER: '',
  CARGO_BUILD_RUSTC_WRAPPER: '', RUSTFLAGS: '', CARGO_ENCODED_RUSTFLAGS: '',
});

const commands = [];
const issues = [];
function git(args) {
  const result = spawnSync('git', args, { cwd: root, encoding: 'utf8' });
  assert.equal(result.status, 0, result.stderr);
  return result.stdout;
}
function gitState() {
  const status = git(['status', '--porcelain=v1', '--untracked-files=all']);
  return { head: git(['rev-parse', 'HEAD']).trim(), status, clean: status.length === 0 };
}
const beforeGit = gitState();
assert.equal(beforeGit.clean, true,
  `the canonical gate requires a committed clean tree:\n${beforeGit.status}`);

function run(label, executable, args, extraEnvironment = {}, input = null, expected = 0) {
  const childEnvironment = { ...environment, ...extraEnvironment };
  const response = spawnSync(executable, args, { cwd: root, env: childEnvironment,
    ...(input === null ? {} : { input }), timeout: 1_800_000, maxBuffer: 128 * 1024 * 1024 });
  const stdout = response.stdout ?? Buffer.alloc(0);
  const stderr = response.stderr ?? Buffer.alloc(0);
  const stem = `${String(commands.length).padStart(2, '0')}-${label}`;
  save(`${stem}.stdout`, stdout);
  save(`${stem}.stderr`, stderr);
  if (input !== null) save(`${stem}.stdin`, input);
  const record = { label, executable, executable_sha256: sha(fs.readFileSync(executable)), args,
    environment: childEnvironment, status: response.status, signal: response.signal,
    error: response.error?.message ?? null,
    stdin: input === null ? null : `${stem}.stdin`,
    stdin_sha256: input === null ? null : sha(input),
    stdout: `${stem}.stdout`, stdout_sha256: sha(stdout),
    stderr: `${stem}.stderr`, stderr_sha256: sha(stderr) };
  commands.push(record);
  if (record.status !== expected || record.error || record.signal) {
    issues.push(`${label}: exit ${record.status}`
      + ` ${record.error ?? record.signal ?? stderr.toString().slice(-600)}`);
  }
  return { ...record, text: stdout.toString('utf8') };
}
const toolFacts = {};
for (const [name, executable, args] of [
  ['node', node, ['-p', 'JSON.stringify({node:process.version,v8:process.versions.v8})']],
  ['rustc', rustc, ['--version']],
  ['cargo', cargo, ['--version']],
]) {
  const result = run(`tool-${name}`, executable, args);
  toolFacts[name] = { path: executable, sha256: result.executable_sha256,
    version: result.text.trim(), status: result.status };
}
const built = run('source-build', cargo,
  ['build', '-p', 'noble-cli', '--bin', 'noble', '--locked', '--offline',
    '--message-format=json']);
const binary = path.join(target, 'debug/noble');
assert.ok(fs.existsSync(binary), 'selected CLI binary missing after the offline build');
const artifacts = built.text.split('\n').filter(line => line.startsWith('{'))
  .map(line => JSON.parse(line)).filter(row => row.reason === 'compiler-artifact'
    && row.target.name === 'noble' && row.target.kind.includes('bin'));
const artifact = artifacts.find(row => row.executable === binary);
if (built.status === 0 && artifact === undefined) {
  issues.push('selected CLI not present in the fresh offline Cargo build receipts');
}
const binarySha256 = sha(fs.readFileSync(binary));

// Lane evidence must be a new private directory directly under TMPDIR: the
// resource-reference lanes refuse any other parent.
const temporaryRoot = fs.realpathSync(environment.TMPDIR);
function evidenceDirectory(label) {
  const directory = fs.mkdtempSync(path.join(temporaryRoot, `noble-lslot-resource-${label}-`));
  fs.chmodSync(directory, 0o700);
  const metadata = fs.lstatSync(directory);
  assert.ok(metadata.isDirectory() && !metadata.isSymbolicLink());
  assert.equal(metadata.uid, process.getuid());
  assert.equal(metadata.mode & 0o077, 0);
  return directory;
}
const evidenceDirectories = [];
function lane(label, script, name) {
  const directory = evidenceDirectory(label);
  evidenceDirectories.push(directory);
  const result = run(`lane-${label}`, node,
    [path.join(root, 'crates/noble-cli/tests', script), binary],
    { NOBLE_SLOT_EVIDENCE_DIR: directory }, null, 0);
  return { directory, name: name ?? label, ...result };
}
const pairedRecords = records => {
  const requests = records.filter(row => row.kind === 'operator').map(row => row.request);
  const replies = records.filter(row => row.kind === 'cli').map(row => row.row);
  assert.equal(replies.length, requests.length + 1);
  return replies.map((row, index) => ({ request: requests[index] ?? null, row }));
};
const pairedFiles = (requestsFile, repliesFile) => {
  const requests = jsonLines(requestsFile);
  const replies = jsonLines(repliesFile);
  assert.equal(replies.length, requests.length + 1,
    `${relative(requestsFile)} request/reply pairing`);
  return replies.map((row, index) => ({ request: requests[index] ?? null, row }));
};
const authorityOf = file => {
  const records = jsonLines(file);
  const header = records.find(row => row.kind === 'authority');
  assert.ok(header, `${relative(file)} authority header`);
  return header.authority;
};
const counters = (directory, name) =>
  JSON.parse(fs.readFileSync(path.join(directory, `${name}.counters.json`)));
const summaryFile = (directory, name) =>
  JSON.parse(fs.readFileSync(path.join(directory, name)));

// LSLOT-01: the selected capture lane plus its two independent controls.
const lane01 = lane('01', 'live_slot_capture_legacy.mjs');
const lane01Late = lane('01-late', 'live_slot_late_control.mjs');
const lane01Origin = lane('01-origin', 'live_slot_selected_origin.mjs');
const control01Late = summaryFile(lane01Late.directory, 'LSLOT01-late-control.summary.json');
const control01Origin = summaryFile(lane01Origin.directory,
  'LSLOT01-selected-origin-control.summary.json');
if (lane01Late.status === 0) {
  assert.equal(control01Late.late.outcome, 'refused');
  assert.equal(control01Late.late.when, 'too-late');
  assert.equal(control01Late.next_root.epoch, '1');
  assert.deepEqual(control01Late.next_root.stack, [{ kind: 1, value: '3' }]);
}
if (lane01Origin.status === 0) {
  assert.equal(control01Origin.selected_binary_sha256, binarySha256);
  assert.ok(control01Origin.owner_count > 0);
}

// LSLOT-02: the epoch-1 nested-root chain, never edited by this gate.
const lane02 = lane('02', 'live_slot_nested_roots.mjs');
const summary02 = summaryFile(lane02.directory, 'LSLOT02-summary.json');

// LSLOT-03 and LSLOT-06: independent exact admission and static boundary.
const lane03 = lane('03', 'live_slot_resource_ref_admission.mjs');
const summary03 = summaryFile(lane03.directory, 'LSLOT03-summary.json');
const lane06 = lane('06', 'live_slot_resource_ref_static.mjs');
const summary06 = summaryFile(lane06.directory, 'LSLOT06-summary.json');

// LSLOT-04 and LSLOT-07: CAS, rollback and current host policy.
const lane0407 = lane('0407', 'live_slot_cas_policy.mjs');
const casNames = ['LSLOT04-same-slot-writers', 'LSLOT04-unrelated-slot-race',
  'LSLOT04-delete-recreate-ABA', 'LSLOT04-authorized-rollback', 'LSLOT04-rollback-denied',
  'LSLOT07-publication-without-grant', 'LSLOT07-static-dispatch-without-grant',
  'LSLOT07-dispatch-without-grant', 'LSLOT07-old-version-effect-revoked-after-pin',
  'LSLOT07-revoked-after-root-pin'];
const cas = casNames.map(name => ({ ...counters(lane0407.directory, name),
  raw: path.join(lane0407.directory, `${name}.jsonl`),
  requests: path.join(lane0407.directory, `${name}.requests.jsonl`) }));
const casSteps = cas.flatMap(item => pairedFiles(item.requests, item.raw)
  .map(step => ({ request: step.request, row: step.row, case: item.name })));
const earliest = (name, slot) => {
  const steps = pairedFiles(path.join(lane0407.directory, `${name}.requests.jsonl`),
    path.join(lane0407.directory, `${name}.jsonl`));
  const match = steps.find(step => step.row.outcome === 'executed'
    && step.row.request_trace?.[0]?.slotId === slot);
  assert.ok(match, `${name} ${slot} baseline select`);
  return match.row.request_trace[0];
};
const casBaseline = { a: earliest('LSLOT04-same-slot-writers', 'A'),
  b: earliest('LSLOT04-unrelated-slot-race', 'B') };
const publicationGrant = pairedFiles(
  path.join(lane0407.directory, 'LSLOT07-publication-without-grant.requests.jsonl'),
  path.join(lane0407.directory, 'LSLOT07-publication-without-grant.jsonl'));
const pinnedEpoch = publicationGrant.find(step => step.row.outcome === 'executed')
  .row.request_trace[0].rootEpoch;

// LSLOT-09: the same checked caller under the three legacy profiles.
const lane09Legacy = lane('09-legacy', 'live_slot_legacy_refusal.mjs');
const lane09Repl = lane('09-repl', 'live_repl_slot_exclusion.mjs');
const summary09Legacy = summaryFile(lane09Legacy.directory, 'LSLOT09-legacy-summary.json');
const summary09Repl = summaryFile(lane09Repl.directory, 'LSLOT09-repl-summary.json');
const liveWasmBytes = fs.readFileSync(path.join(root, liveWasmFile));
const liveWasmCases = JSON.parse(liveWasmBytes).cases.filter(row => row.id.startsWith('LIVE-'));
assert.equal(liveWasmCases.length, 10, 'ten canonical LIVE cases');
const liveWasmUnchanged = sha(liveWasmBytes)
    === sha(Buffer.from(git(['show', `HEAD:${liveWasmFile}`])))
  && git(['status', '--porcelain=v1', '--', liveWasmFile]).length === 0;

// Every lane evidence directory must be fully hashed and symlink-free.
const evidenceFiles = {};
function walk(directory) {
  for (const name of fs.readdirSync(directory)) {
    const file = path.join(directory, name), stat = fs.lstatSync(file);
    assert.ok(!stat.isSymbolicLink(), `external evidence symlink: ${file}`);
    if (stat.isDirectory()) walk(file);
    else {
      assert.ok(stat.isFile(), `unexpected evidence object: ${file}`);
      evidenceFiles[file] = sha(fs.readFileSync(file));
    }
  }
}
for (const directory of evidenceDirectories) walk(directory);

const crossProfile = {
  registryEpochs: [...new Set(casSteps.filter(step => step.row.epoch !== undefined)
    .map(step => Number(step.row.epoch)))].sort((left, right) => left - right),
  liveGenerations: [summary09Repl.control.initial_generation,
    summary09Repl.control.saved_generation, summary09Repl.control.reload.generation,
    summary09Repl.control.reload.installed_generation],
  liveReportsHaveEpoch: jsonLines(path.join(lane09Repl.directory, 'LSLOT09-repl.jsonl'))
    .filter(row => row.kind === 'cli').some(row => Object.hasOwn(row.report, 'epoch')),
  refusedSlotCaller: summary09Repl.refusal.generation === 0,
};
const caseSteps = (directory, prefix, names) => names.flatMap(name =>
  pairedRecords(jsonLines(path.join(directory, `${prefix}-${name}.jsonl`))));
const lanes = {
  'LSLOT-01': { records: jsonLines(path.join(lane01.directory, 'LSLOT01.jsonl')) },
  'LSLOT-02': { results: summary02.results, crossProfile,
    authority: authorityOf(path.join(lane02.directory, 'LSLOT02-base-parent-child.jsonl')) },
  'LSLOT-03': { summary: summary03,
    authority: authorityOf(path.join(lane03.directory, 'LSLOT03-same-exact-interface.jsonl')),
    steps: caseSteps(lane03.directory, 'LSLOT03',
      summary03.results.map(result => result.name)) },
  'LSLOT-04': { cases: cas, baseline: casBaseline, steps: casSteps },
  'LSLOT-06': { summary: summary06,
    authority: authorityOf(path.join(lane06.directory, 'LSLOT06-valid-borrow.jsonl')),
    steps: caseSteps(lane06.directory, 'LSLOT06',
      summary06.results.map(result => result.name)) },
  'LSLOT-07': { cases: cas, slot: 'emit', pinnedEpoch, steps: casSteps },
  'LSLOT-09': { legacy: summary09Legacy, repl: summary09Repl, liveUnchanged: liveWasmUnchanged },
};
const cases = [];
const mismatches = [];
for (const row of fixture.cases) {
  if (!Object.hasOwn(projectors, row.id)) {
    cases.push({ id: row.id, promotable: false, entries: [],
      reason: 'template identity: this gate does not execute the positive selected-target proof'
        + ' or exact replay case before P-ID-03/P-ID-04 target identity is established' });
    continue;
  }
  let entries;
  try {
    entries = projectors[row.id]({ fixture: row, lane: lanes[row.id] });
  } catch (error) {
    cases.push({ id: row.id, promotable: false, entries: [],
      reason: `projection failed: ${error.stack ?? error}` });
    mismatches.push(`${row.id}: projection failed: ${error.message}`);
    continue;
  }
  const counted = {};
  for (const status of ['observed', 'declared', 'unobserved', 'mismatch']) {
    counted[status] = entries.filter(entry => entry.status === status).length;
  }
  for (const entry of entries.filter(item => item.status === 'mismatch')) {
    mismatches.push(`${row.id} ${entry.scope} ${entry.key}:`
      + ` canonical ${JSON.stringify(entry.canonical)}`
      + ` observed ${JSON.stringify(entry.observed)}`);
  }
  cases.push({ id: row.id, promotable: counted.unobserved === 0 && counted.mismatch === 0,
    counts: counted, entries,
    ...(counted.unobserved > 0 ? { unobserved: entries.filter(entry =>
      entry.status === 'unobserved').map(entry => `${entry.scope}.${entry.key}`) } : {}) });
}

// A previously promoted case must still project as promotable, its committed
// receipt must exist and bind the unchanged case design, and the fresh
// projection must reproduce the committed observation values.
const promoted = [];
const refused = [];
for (const row of cases) {
  const canonical = caseById(row.id);
  if (canonical.evidence.length === 0) {
    if (row.promotable) promoted.push(row.id);
    else if (!Object.hasOwn(projectors, row.id)) refused.push({ id: row.id, reason: row.reason });
    else refused.push({ id: row.id, unobserved: row.unobserved });
    continue;
  }
  assert.equal(canonical.evidence.length, 1, `${row.id} one promotion record`);
  assert.deepEqual(canonical.state, { implementation: 'implemented', execution: 'passed',
    proof: 'open', trust: 'explicit' }, `${row.id} promoted state`);
  const record = canonical.evidence[0];
  assert.equal(record.kind, 'test', `${row.id} test evidence`);
  assert.equal(record.result, 'passed', `${row.id} passed evidence`);
  assert.equal(record.subject, row.id);
  assert.equal(record.revision, fixture.revision);
  assert.equal(record.configuration.gate, 'verification/lslot-cases/gate.mjs');
  assert.equal(record.configuration.receipt, '../../verification/lslot-cases/acceptance.json');
  assert.equal(record.configuration.case, row.id);
  assert.equal(record.configuration.prepromotion_case_sha256, designSha(row.id),
    `${row.id} canonical design drifted after promotion`);
  const receiptFile = path.resolve(root, 'specs/conformance',
    record.configuration.receipt);
  assert.ok(receiptFile.startsWith(`${root}/verification/lslot-cases/`),
    `${row.id} receipt outside the promotion lane`);
  const receiptBytes = fs.readFileSync(receiptFile);
  assert.equal(sha(receiptBytes), record.configuration.receipt_sha256,
    `${row.id} committed receipt bytes`);
  const receipt = JSON.parse(receiptBytes);
  assert.equal(receipt.schema, 'noble-lslot-case-acceptance/v1');
  assert.equal(receipt.result, 'passed');
  assert.equal(record.source_revision, receipt.source.revision,
    `${row.id} evidence source revision equals the committed receipt`);
  assert.equal(record.configuration.binary_sha256,
    receipt.cases.find(entry => entry.id === row.id)?.binary_sha256 ?? binarySha256,
    `${row.id} receipt binary binding`);
  if (!row.promotable) {
    mismatches.push(`${row.id}: promoted case no longer projects as promotable`);
  }
  const committed = receipt.cases.find(entry => entry.id === row.id);
  assert.ok(committed, `${row.id} committed receipt row`);
  for (const entry of row.entries) {
    const previous = committed.entries.find(candidate =>
      candidate.scope === entry.scope && candidate.key === entry.key);
    assert.ok(previous, `${row.id} committed entry ${entry.scope}.${entry.key}`);
    if (entry.status !== 'declared') {
      assert.equal(JSON.stringify(entry.observed), JSON.stringify(previous.observed),
        `${row.id} ${entry.scope}.${entry.key} reproducible observation`);
    }
  }
  promoted.push(row.id);
}
if (mismatches.length > 0) issues.push(...mismatches);

try {
  frozen(source);
  assert.deepEqual(sourceInventory(inputs, selectedTrees), source,
    'selected source paths or bytes changed during execution');
} catch (error) { issues.push(`source freeze: ${error.message}`); }
const afterGit = gitState();
if (beforeGit.head !== afterGit.head || beforeGit.clean !== afterGit.clean) {
  issues.push('selected Git HEAD or clean status changed during source-bound execution');
}
if (sha(fs.readFileSync(binary)) !== binarySha256) {
  issues.push('selected binary changed after execution');
}
if (sha(fs.readFileSync(path.join(root, fixtureFile))) !== sha(fixtureBytes)) {
  issues.push('canonical fixture bytes changed during the gate');
}

const summary = {
  promotable: cases.filter(row => row.promotable).map(row => row.id),
  refused: cases.filter(row => !row.promotable && Object.hasOwn(projectors, row.id))
    .map(row => row.id),
  not_executed: cases.filter(row => !Object.hasOwn(projectors, row.id)).map(row => row.id),
  promoted, mismatches: mismatches.length,
};
const result = issues.length === 0 ? 'passed' : 'failed';
const acceptance = {
  schema: 'noble-lslot-case-acceptance/v1',
  result,
  canonical_promotion: result === 'passed' ? summary.promoted : [],
  proof: 'open',
  source: { commit: beforeGit.head, clean: beforeGit.clean, revision: sourceRevision,
    files: source, selected_trees: selectedTrees },
  fixture: { path: fixtureFile, sha256: sha(fixtureBytes), revision: fixture.revision,
    live_wasm: { path: liveWasmFile, sha256: sha(liveWasmBytes),
      unchanged_from_head: liveWasmUnchanged } },
  tool_selection: { configuration_sha256: source['policy/tool-selection.json'],
    runtime_configuration_sha256: source['crates/noble-cli/src/core/runtime/config.json'],
    child_environment: environment, tools: toolFacts },
  binary: { path: binary, sha256: binarySha256, cargo_target_dir: target,
    build_status: built.status,
    build_claim: 'offline Cargo reported the selected `noble` bin artifact for this target; the'
      + ' recorded sha256 is the exact selected binary this gate executed' },
  projectors: Object.keys(projectors),
  cases: cases.map(row => ({ ...row, binary_sha256: binarySha256,
    prepromotion_case_sha256: designSha(row.id) })),
  promotion: { promoted: summary.promoted, refused: summary.refused,
    not_executed: summary.not_executed,
    rule: 'a case is promotable when every machine-expressible input/expected/variant field is'
      + ' observed and equals the canonical value; `declared` entries are prose design text and'
      + ' are never counted as observations; one mismatch fails the gate' },
  controls: [
    { lane: 'live_slot_late_control.mjs', summary: control01Late },
    { lane: 'live_slot_selected_origin.mjs', summary: control01Origin },
  ],
  cross_profile: crossProfile,
  commands,
  summary,
  integrity_issues: issues,
  limitations: [
    'This receipt executes behavior; it is not a proof. Every promoted case keeps `proof: open`.',
    'LSLOT-05 and LSLOT-08 are not executed here: their positive selected-target and exact replay'
      + ' outcomes wait on runtime template identity (P-ID-03/P-ID-04).',
    'LSLOT-03, LSLOT-04, LSLOT-06 and LSLOT-07 are refused with the exact unobserved fields'
      + ' recorded under `cases[*].unobserved`; they are not promoted.',
    '`verification/live-slot-current-source/gate.mjs` and `verification/live-update-corpus` are'
      + ' historical receipts for the pre-promotion states and are not re-run or regenerated.',
  ],
  external_evidence_sha256: evidenceFiles,
  version_control: { before: beforeGit, after: afterGit },
};
save('acceptance.json', JSON.stringify(acceptance, null, 2) + '\n');
save('acceptance.sha256', `${sha(JSON.stringify(acceptance, null, 2) + '\n')}  acceptance.json\n`);
console.log(JSON.stringify({ result, receipt: path.join(output, 'acceptance.json'),
  receipt_sha256: sha(JSON.stringify(acceptance, null, 2) + '\n'),
  source_revision: sourceRevision, binary_sha256: binarySha256, ...summary,
  integrity_issues: issues.length }));
process.exitCode = result === 'passed' ? 0 : 1;
