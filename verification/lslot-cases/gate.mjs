#!/usr/bin/env node
// Canonical LSLOT case gate: build the selected CLI from a committed clean
// tree, run the live-slot lanes that carry each case's observable behavior,
// project every canonical `input`/`expected`/variant field onto that evidence,
// and refuse any variant the lanes did not reach. A case is promotable only
// when every machine-expressible field is observed and equals the canonical
// value; one mismatch always fails, and a leaf with no projection entry fails
// the completeness check. A run writes a new immutable receipt file; it never
// writes, overwrites or binds a retained receipt.
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
  root, sha, sourceInventory, revision, frozen,
} from '../dx06-current-source/source.mjs';
import {
  jsonLines, leaves, projectors, pending, declaredAllowlist,
} from './projection.mjs';
import {
  LIVE_BASELINE_COMMIT, APPROVED_LIVE_CHANGES, liveDesign, liveContractDiffs,
} from './live-baseline.mjs';

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
// The retained receipt the committed promotions cite. A run writes a new file
// and never rewrites this one.
const promotedReceiptPath = '../../verification/lslot-cases/acceptance-2026-11-20-r4.json';
const fixtureBytes = fs.readFileSync(path.join(root, fixtureFile));
const fixture = JSON.parse(fixtureBytes);
assert.deepEqual(fixture.cases.map(row => row.id),
  Array.from({ length: 9 }, (_, index) => `LSLOT-0${index + 1}`));
const caseById = id => {
  const rows = fixture.cases.filter(row => row.id === id);
  assert.equal(rows.length, 1, `${id} unique case`);
  return rows[0];
};
// The digest scope is exactly the canonical design the promotion does not edit.
const designSha = id => {
  const row = caseById(id);
  return sha(JSON.stringify({ id: row.id, input: row.input, expected: row.expected }));
};

const explicitInputs = [fixtureFile, liveWasmFile, 'Cargo.toml', 'Cargo.lock',
  'rust-toolchain.toml', 'policy/tool-selection.json', 'crates/noble-cli/Cargo.toml',
  'crates/noble-ngraph/Cargo.toml',
  'crates/noble-contracts/Cargo.toml', 'crates/noble-kernel/Cargo.toml',
  'crates/noble-wasm/Cargo.toml', 'crates/noble-cli/src/core/runtime/config.json',
  'verification/lslot-cases/gate.mjs', 'verification/lslot-cases/projection.mjs',
  'verification/lslot-cases/live-baseline.mjs',
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
if (built.status === 0 && artifacts.find(row => row.executable === binary) === undefined) {
  issues.push('selected CLI not present in the fresh offline Cargo build receipts');
}
const binarySha256 = sha(fs.readFileSync(binary));

// Every repository path the executed binary was compiled from joins the
// manifest. The dep-info is Cargo's own compile-input list.
const depInfo = fs.readFileSync(path.join(target, 'debug/noble.d'), 'utf8');
const dependencies = depInfo.replace(/\\\n/g, ' ').split(/\s+/)
  .map(token => token.trim()).filter(Boolean)
  .map(token => path.resolve(root, token))
  .filter(file => file.startsWith(`${root}/`))
  .map(relative)
  .filter(file => !file.startsWith('target/'));
const missingDependencies = dependencies.filter(file => !fs.existsSync(path.join(root, file)));
assert.deepEqual(missingDependencies, [], `dep-info paths missing: ${missingDependencies}`);
// Every retained receipt beside the gate is excluded from the manifest, so no
// receipt binds itself or an earlier receipt. A new receipt file must be added
// here (and never replaces a retained one).
const retainedReceipts = [
  'verification/lslot-cases/acceptance.json',
  'verification/lslot-cases/acceptance.sha256',
  'verification/lslot-cases/acceptance-2026-11-20-r3.json',
  'verification/lslot-cases/acceptance-2026-11-20-r3.sha256',
  'verification/lslot-cases/acceptance-2026-11-20-r4.json',
  'verification/lslot-cases/acceptance-2026-11-20-r4.sha256',
  'verification/lslot-cases/acceptance-2026-11-21-b1.json',
  'verification/lslot-cases/acceptance-2026-11-21-b1.sha256',
  'verification/lslot-cases/acceptance-2026-11-21-b2.json',
  'verification/lslot-cases/acceptance-2026-11-21-b2.sha256',
];
const presentReceipts = fs.readdirSync(path.join(root, 'verification/lslot-cases'))
  .filter(name => name.startsWith('acceptance'))
  .map(name => `verification/lslot-cases/${name}`)
  .sort();
const unlistedReceipts = presentReceipts.filter(file => !retainedReceipts.includes(file));
assert.deepEqual(unlistedReceipts, [],
  `retained receipt files missing from retainedReceipts: ${unlistedReceipts.join(', ')}`);
const inputs = [...new Set([...explicitInputs, ...dependencies])]
  .filter(file => !retainedReceipts.includes(file));
const source = sourceInventory(inputs, selectedTrees);
const sourceRevision = revision(source);
save('source.json', JSON.stringify({ revision: sourceRevision, sha256: source }, null, 2) + '\n');
save('canonical-fixture.json', fixtureBytes);

// Lane evidence must be a new private directory directly under TMPDIR: the
// resource-reference lanes refuse any other parent. The authority root keeps
// the lanes from writing into the source tree.
const temporaryRoot = fs.realpathSync(environment.TMPDIR);
function privateDirectory(label, prefix = `noble-lslot-resource-${label}-`) {
  const directory = fs.mkdtempSync(path.join(temporaryRoot, prefix));
  fs.chmodSync(directory, 0o700);
  const metadata = fs.lstatSync(directory);
  assert.ok(metadata.isDirectory() && !metadata.isSymbolicLink());
  assert.equal(metadata.uid, process.getuid());
  assert.equal(metadata.mode & 0o077, 0);
  return directory;
}
const evidenceDirectories = [];
function lane(label, script) {
  const directory = privateDirectory(label);
  evidenceDirectories.push(directory);
  const authorityRoot = privateDirectory(label,
    `noble-lslot-resource-authority-${label}-`);
  evidenceDirectories.push(authorityRoot);
  const result = run(`lane-${label}`, node,
    [path.join(root, 'crates/noble-cli/tests', script), binary],
    { NOBLE_SLOT_EVIDENCE_DIR: directory, NOBLE_SLOT_AUTHORITY_ROOT: authorityRoot });
  const mirror = path.join(output, 'raw', label);
  fs.mkdirSync(mirror, { recursive: true, mode: 0o700 });
  for (const name of fs.readdirSync(directory)) {
    fs.copyFileSync(path.join(directory, name), path.join(mirror, name));
  }
  return { directory, mirror, ...result };
}
const pairedRecords = records => {
  const requests = records.filter(row => row.kind === 'operator').map(row => row.request);
  const replies = records.filter(row => row.kind === 'cli').map(row => row.row);
  assert.equal(replies.length, requests.length + 1,
    'one configuration report plus one reply per operator request');
  return [{ request: null, row: replies[0] },
    ...replies.slice(1).map((row, index) => ({ request: requests[index], row }))];
};
const pairedFiles = (requestsFile, repliesFile) => {
  const requests = jsonLines(requestsFile);
  const replies = jsonLines(repliesFile);
  assert.equal(replies.length, requests.length + 1,
    `${relative(requestsFile)} request/reply pairing`);
  return [{ request: null, row: replies[0] },
    ...replies.slice(1).map((row, index) => ({ request: requests[index], row }))];
};
const authorityOf = file => {
  const records = jsonLines(file);
  const header = records.find(row => row.kind === 'authority');
  assert.ok(header, `${relative(file)} authority header`);
  return header.authority;
};
const summaryFile = (directory, name) =>
  JSON.parse(fs.readFileSync(path.join(directory, name)));
const counters = (directory, name) =>
  JSON.parse(fs.readFileSync(path.join(directory, `${name}.counters.json`)));

// LSLOT-01: the capture lane plus its two independent controls.
const lane01 = lane('01', 'live_slot_capture_legacy.mjs');
const lane01Late = lane('01-late', 'live_slot_late_control.mjs');
const lane01Origin = lane('01-origin', 'live_slot_selected_origin.mjs');
const control01Late = summaryFile(lane01Late.directory, 'LSLOT01-late-control.summary.json');
const control01Origin = summaryFile(lane01Origin.directory,
  'LSLOT01-selected-origin-control.summary.json');
if (lane01Late.status === 0) {
  assert.equal(control01Late.late.outcome, 'refused');
  assert.equal(control01Late.next_root.epoch, '1');
}
if (lane01Origin.status === 0) {
  assert.equal(control01Origin.selected_binary_sha256, binarySha256);
}

// LSLOT-02: the epoch-1 nested-root chain, never edited by this gate.
const lane02 = lane('02', 'live_slot_nested_roots.mjs');
const summary02 = summaryFile(lane02.directory, 'LSLOT02-summary.json');
const fixture02 = name => {
  const raw = path.join(lane02.directory, `LSLOT02-${name}.jsonl`);
  const steps = pairedRecords(jsonLines(raw));
  const item = summary02.results.find(result => result.name === name);
  assert.ok(item, `LSLOT-02 ${name} summary`);
  // Cross-check the lane summary against the raw CLI records it wrote.
  const executed = steps.filter(step => step.row.outcome === 'executed')
    .map(step => step.row);
  assert.deepEqual(item.old_root.request_trace, executed[0].request_trace,
    `${name} old-root trace equals the raw executed reply`);
  assert.deepEqual(item.committed_ack, steps.find(step => step.row.outcome === 'published'
    && step.row.control_id === '1').row, `${name} ACK equals the raw published reply`);
  const bootstrap = steps.find(step => step.row.outcome === 'bootstrapped').row;
  assert.deepEqual(item.bootstrap, bootstrap, `${name} bootstrap equals the raw reply`);
  assert.deepEqual(item.new_root.request_trace, executed[1].request_trace, `${name} new root`);
  assert.deepEqual(item.next_leaf.request_trace, executed[2].request_trace, `${name} leaf`);
  return {
    ...item,
    session_log: raw,
    configured_epoch: steps[0].row.epoch,
    authority: authorityOf(raw),
    install_requests: steps.filter(step => step.request?.operation === 'install')
      .map(step => step.request),
    invoke_requests: steps.filter(step => step.request?.operation === 'invoke')
      .map(step => step.request),
    reflected_sites: steps.filter(step => step.request?.operation === 'reflect')
      .map(step => ({ id: step.request.id, site_id: step.request.site_id, site: step.row.site })),
    stages: steps.map(step => step.row.stage),
  };
};
const results02 = ['base-parent-child', 'transitive-A-B-C'].map(fixture02);

// LSLOT-03 and LSLOT-06: exact admission and the static/lifetime boundary.
const lane03 = lane('03', 'live_slot_resource_ref_admission.mjs');
const summary03 = summaryFile(lane03.directory, 'LSLOT03-summary.json');
const steps03 = summary03.results.flatMap(result =>
  pairedRecords(jsonLines(path.join(lane03.directory, `LSLOT03-${result.name}.jsonl`))));
// LSLOT-03's `generic_dispatch_effect` witness is the raw caller-reflection
// reply; the lane summary must agree with it.
const callerReflectionSteps = pairedRecords(jsonLines(path.join(lane03.directory,
  'LSLOT03-caller-reflection.jsonl')));
const callerInstallSteps = callerReflectionSteps.filter(step =>
  step.request?.operation === 'install' && step.request.id === 'caller');
const callerReflectSteps = callerReflectionSteps.filter(step =>
  step.request?.operation === 'reflect' && step.request.id === 'caller');
assert.equal(callerInstallSteps.length, 1, 'LSLOT-03 caller install step');
assert.equal(callerReflectSteps.length, 1, 'LSLOT-03 caller reflect step');
assert.equal(callerInstallSteps[0].row.outcome, 'installed');
const callerSiteRaw = callerReflectSteps[0].row.site;
assert.equal(callerSiteRaw?.instruction, 'slot.invoke');
assert.deepEqual(summary03.caller_reflection.site, callerSiteRaw,
  'LSLOT-03 caller reflection summary equals the raw reply');
assert.deepEqual(summary03.caller_reflection.install_inputs,
  callerInstallSteps[0].request.inputs,
  'LSLOT-03 caller install inputs equal the raw request');
assert.equal(summary03.caller_reflection.install_source,
  callerInstallSteps[0].request.source,
  'LSLOT-03 caller install source equals the raw request');
const lane06 = lane('06', 'live_slot_resource_ref_static.mjs');
const summary06 = summaryFile(lane06.directory, 'LSLOT06-summary.json');
const steps06 = summary06.results.flatMap(result =>
  pairedRecords(jsonLines(path.join(lane06.directory, `LSLOT06-${result.name}.jsonl`))));
for (const [label, summary, directory] of [['LSLOT-03', summary03, lane03.directory],
  ['LSLOT-06', summary06, lane06.directory]]) {
  for (const result of summary.results) {
    const raw = jsonLines(path.join(directory, `${label.replace('-', '')}-${result.name}.jsonl`));
    const configured = raw.find(row => row.kind === 'cli').row;
    assert.equal(configured.outcome, 'configured', `${label} ${result.name} configured`);
  }
}
const installInputs06 = steps06.filter(step => step.request?.operation === 'install')
  .flatMap(step => step.request.inputs);
// Raw CLI replies, not lane-written strings, carry each variant's outcome and
// epoch. One file per variant: the seed chain, the candidate attempt, the
// post-refusal trace and the independent exact control are all attributed to
// that variant only.
const epochOf = reply => (reply.epoch == null ? null : Number(reply.epoch));
const variantFacts = steps => {
  const facts = new Map();
  const touch = name => {
    if (!facts.has(name)) facts.set(name, { name, guest_requests: 0,
      protected_operations: 0 });
    return facts.get(name);
  };
  let seedEpoch = null, control = null;
  const traces = [];
  for (const step of steps) {
    const request = step.request, reply = step.row;
    if (!request) continue;
    if (request.operation === 'trace') traces.push(reply.trace ?? null);
    if (request.operation === 'publish' && request.id === 'seed') {
      seedEpoch = epochOf(reply);
    }
    if (request.operation === 'publish' && request.id === 'exact') {
      control = { expected_epoch: request.expected_epoch, outcome: reply.outcome,
        epoch: epochOf(reply) };
    }
    if (['install', 'invoke', 'publish'].includes(request.operation) && request.id
      && !['seed', 'exact', 'target', 'caller'].includes(request.id)) {
      const found = touch(request.id);
      if (request.operation === 'install') {
        found.install_outcome = reply.outcome;
        found.attempt_outcome = reply.outcome;
      }
      if (request.operation === 'publish') {
        found.publication_outcome = reply.outcome;
        found.attempt_epoch = epochOf(reply);
        found.publish_expected_epoch = request.expected_epoch;
      }
      if (request.operation === 'invoke') found.invoke_outcome = reply.outcome;
      found.guest_requests = Math.max(found.guest_requests, reply.guest_requests ?? 0);
      found.protected_operations = Math.max(found.protected_operations,
        reply.protected_operations ?? 0);
    }
    if (request.operation === 'reflect' && request.id) touch(request.id).site = reply.site;
  }
  for (const found of facts.values()) {
    found.seed_epoch = seedEpoch;
    found.traces = traces;
    if (control) Object.assign(found, { control_expected_epoch: control.expected_epoch,
      control_outcome: control.outcome, control_epoch: control.epoch });
  }
  return [...facts.values()];
};

// LSLOT-04 and LSLOT-07: CAS, rollback and current host policy.
const lane0407 = lane('0407', 'live_slot_cas_policy.mjs');
const casNames = ['LSLOT04-same-slot-writers', 'LSLOT04-unrelated-slot-race',
  'LSLOT04-delete-recreate-ABA', 'LSLOT04-authorized-rollback', 'LSLOT04-rollback-denied',
  'LSLOT07-publication-without-grant', 'LSLOT07-static-dispatch-without-grant',
  'LSLOT07-dispatch-without-grant', 'LSLOT07-old-version-effect-revoked-after-pin',
  'LSLOT07-revoked-after-root-pin'];
const casSteps = [];
const cas = casNames.map(name => {
  const raw = path.join(lane0407.directory, `${name}.jsonl`);
  const requests = path.join(lane0407.directory, `${name}.requests.jsonl`);
  const steps = pairedFiles(requests, raw);
  casSteps.push(...steps.map(step => ({ request: step.request, row: step.row })));
  return { ...counters(lane0407.directory, name), raw, requests,
    stages: steps.map(step => step.row.stage) };
});
const earliest = (name, slot) => {
  const steps = pairedFiles(path.join(lane0407.directory, `${name}.requests.jsonl`),
    path.join(lane0407.directory, `${name}.jsonl`));
  const match = steps.find(step => step.row.outcome === 'executed'
    && step.row.request_trace?.[0]?.slotId === slot);
  assert.ok(match, `${name} ${slot} baseline select`);
  return match.row.request_trace[0];
};
// The baseline target the case names is the artifact the matching publication
// installed for that incarnation/generation; the dispatch must select exactly
// it.
const publishedAt = (name, slot, incarnation, generation) => {
  const steps = pairedFiles(path.join(lane0407.directory, `${name}.requests.jsonl`),
    path.join(lane0407.directory, `${name}.jsonl`));
  const published = steps.find(step => step.request?.operation === 'publish'
    && step.request.slot === slot && step.row.outcome === 'published'
    && step.row.incarnation === String(incarnation)
    && step.row.generation === String(generation));
  assert.ok(published, `${name} ${slot} ${incarnation}/${generation} baseline publication`);
  const installed = steps.find(step => step.request?.operation === 'install'
    && step.request.id === published.request.id && step.row.outcome === 'installed');
  assert.ok(installed, `${name} ${published.request.id} baseline install`);
  return { id: published.request.id, epoch: published.row.epoch,
    incarnation: published.row.incarnation, generation: published.row.generation,
    source: installed.request.source, artifactSha256: installed.row.artifact_sha256 };
};
const casBaseline = { a: earliest('LSLOT04-same-slot-writers', 'A'),
  b: earliest('LSLOT04-unrelated-slot-race', 'B'),
  a_published: publishedAt('LSLOT04-same-slot-writers', 'A', 7, 3),
  b_published: publishedAt('LSLOT04-unrelated-slot-race', 'B', 2, 1) };
const pinnedEpoch = pairedFiles(
  path.join(lane0407.directory, 'LSLOT07-publication-without-grant.requests.jsonl'),
  path.join(lane0407.directory, 'LSLOT07-publication-without-grant.jsonl'))
  .find(step => step.row.outcome === 'executed').row.request_trace[0].rootEpoch;

// LSLOT-09: the same checked caller under the three legacy profiles.
const lane09Legacy = lane('09-legacy', 'live_slot_legacy_refusal.mjs');
const lane09Repl = lane('09-repl', 'live_repl_slot_exclusion.mjs');
const summary09Legacy = summaryFile(lane09Legacy.directory, 'LSLOT09-legacy-summary.json');
const summary09Repl = summaryFile(lane09Repl.directory, 'LSLOT09-repl-summary.json');
// Cross-check the lane summaries against their raw CLI records.
const legacyRaw = jsonLines(path.join(lane09Legacy.directory, 'LSLOT09-legacy.jsonl'));
const legacyCommands = legacyRaw.filter(row => row.kind === 'command');
for (const refusal of summary09Legacy.refusals) {
  const raw = legacyCommands.find(row => row.args[0] === (refusal.entry === 'compile'
    ? 'compile' : 'session'));
  assert.ok(raw, `LSLOT-09 ${refusal.entry} raw command record`);
  const report = raw.reports.at(-1);
  assert.equal(report.outcome, refusal.outcome, `LSLOT-09 ${refusal.entry} outcome`);
  assert.equal(report.guest_requests, refusal.guest_requests);
  refusal.raw_profile = report.profile;
  refusal.stdin = refusal.entry === 'session' ? raw.args : null;
  assert.ok(refusal.stage !== undefined);
}
const replRaw = jsonLines(path.join(lane09Repl.directory, 'LSLOT09-repl.jsonl'));
const replCli = replRaw.filter(row => row.kind === 'cli').map(row => row.report);
assert.equal(replCli[0].outcome, summary09Repl.refusal.outcome);
assert.equal(replCli[0].generation, summary09Repl.refusal.generation);
assert.equal(replCli[0].stage, summary09Repl.refusal.stage);
// The REPL and Core control records are lane summaries; every value the
// projection reads is checked against the raw CLI rows they were built from.
assert.deepEqual(replCli.map(report => report.outcome),
  ['reload-refused', 'reload-committed', 'normal', 'reload-committed', 'normal'],
  'LSLOT-09 REPL raw report sequence');
assert.equal(replCli[1].generation, summary09Repl.control.initial_generation);
assert.equal(replCli[2].generation, summary09Repl.control.saved_generation);
assert.equal(replCli[3].generation, summary09Repl.control.reload.generation);
assert.equal(replCli[3].installed_code_span?.generation,
  summary09Repl.control.reload.installed_generation);
assert.deepEqual(replCli[4].stack, summary09Repl.control.stack);
assert.equal(replCli[4].stack[0].value, '21');
const legacyControl = legacyCommands.find(row => row.reports.map(report => report.outcome)
  .join(',') === 'defined,normal,defined,normal');
assert.ok(legacyControl, 'LSLOT-09 legacy Core control raw command');
assert.deepEqual(summary09Legacy.control.stack, legacyControl.reports.at(-1).stack);
assert.equal(summary09Legacy.control.guest_requests,
  legacyControl.reports.at(-1).guest_requests);
assert.equal(summary09Legacy.control.protected_operations,
  legacyControl.reports.at(-1).protected_operations);

// B-LSLOT-01: the LIVE-01..10 contract fields must not change from the change's
// base commit. State/evidence transitions by other changes are recorded, not
// treated as design changes.
const liveBaselineBytes = Buffer.from(git(['show', `${LIVE_BASELINE_COMMIT}:${liveWasmFile}`]));
const liveCurrentBytes = fs.readFileSync(path.join(root, liveWasmFile));
assert.equal(liveDesign(liveBaselineBytes).length, 10, 'ten baseline LIVE cases');
const liveDiffs = liveContractDiffs({ baselineBytes: liveBaselineBytes,
  currentBytes: liveCurrentBytes });
const liveContractsChanged = liveDiffs.length > 0;
const liveStateChanges = liveDesign(liveCurrentBytes).map((row, index) => {
  const current = JSON.parse(liveCurrentBytes).cases.find(entry => entry.id === row.id);
  const baseline = JSON.parse(liveBaselineBytes).cases.find(entry => entry.id === row.id);
  return { id: row.id, baseline_state: baseline.state, state: current.state,
    baseline_evidence: baseline.evidence.length, evidence: current.evidence.length };
}).filter(entry => JSON.stringify(entry.baseline_state) !== JSON.stringify(entry.state)
  || entry.baseline_evidence !== entry.evidence);

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
// The run's own command outputs join the raw mirror beside the lane evidence,
// so a retained receipt's stdout/stderr are re-derivable from the repository.
const commandOutputs = commands.flatMap(command => [command.stdout, command.stderr,
  ...(command.stdin ? [command.stdin] : [])]);
fs.mkdirSync(path.join(output, 'raw'), { recursive: true, mode: 0o700 });
for (const name of commandOutputs) {
  fs.copyFileSync(path.join(output, name), path.join(output, 'raw', name));
}

const stagesOf = entries => [...new Set(entries.flatMap(entry => entry.stages ?? []))];
// LSLOT-06 `ref_authority`: a host-issued ref dispatched with its grant
// revoked is denied by current host policy (raw LSLOT-07 reply).
const ungrantedDispatchDenial = jsonLines(path.join(lane0407.directory,
  'LSLOT07-dispatch-without-grant.jsonl'))
  .flatMap(reply => reply.request_trace ?? [])
  .find(entry => entry.outcome === 'policy-denied') ?? null;
const lanes = {
  'LSLOT-01': { records: jsonLines(path.join(lane01.directory, 'LSLOT01.jsonl')) },
  'LSLOT-02': { results: results02, stages: stagesOf(results02),
    crossProfile: {
      registryEpochs: [1, 2],
      liveGenerations: [summary09Repl.control.initial_generation,
        summary09Repl.control.saved_generation, summary09Repl.control.reload.generation,
        summary09Repl.control.reload.installed_generation],
      liveReportsHaveEpoch: replCli.some(report => Object.hasOwn(report, 'epoch')),
      refusedSlotGeneration: summary09Repl.refusal.generation,
    } },
  'LSLOT-03': { summary: summary03,
    authority: authorityOf(path.join(lane03.directory, 'LSLOT03-same-exact-interface.jsonl')),
    variants: summary03.results.flatMap(result => variantFacts(pairedRecords(
      jsonLines(path.join(lane03.directory, `LSLOT03-${result.name}.jsonl`))))),
    callerSite: callerSiteRaw,
    callerInstall: { outcome: callerInstallSteps[0].row.outcome,
      source: callerInstallSteps[0].request.source,
      inputs: callerInstallSteps[0].request.inputs },
    stageSteps: steps03.map(step => step.row.stage) },
  'LSLOT-04': { cases: cas, baseline: casBaseline,
    stageSteps: casSteps.map(step => step.row.stage) },
  'LSLOT-06': { summary: summary06,
    authority: authorityOf(path.join(lane06.directory, 'LSLOT06-valid-borrow.jsonl')),
    variants: summary06.results.map(result => {
      const steps = pairedRecords(jsonLines(path.join(lane06.directory,
        `LSLOT06-${result.name}.jsonl`)));
      const found = { name: result.name, install_inputs: [], executed_invokes: 0 };
      for (const step of steps) {
        const request = step.request, reply = step.row;
        if (!request) continue;
        if (request.operation === 'install' && request.id === 'attempt') {
          found.install_outcome = reply.outcome;
          found.negative = { kind: 'install', outcome: reply.outcome,
            guest_requests: reply.guest_requests ?? 0,
            protected_operations: reply.protected_operations ?? 0,
            diagnostic: reply.diagnostic ?? null };
        }
        if (request.operation === 'install' && request.id === 'caller') {
          found.install_inputs = request.inputs ?? [];
        }
        if (request.operation === 'invoke' && request.id === 'caller') {
          if (reply.outcome === 'executed') {
            found.executed_invokes += 1;
            found.valid_guest_requests = reply.guest_requests ?? 0;
            found.valid_protected_operations = reply.protected_operations ?? 0;
          }
        }
        if (request.operation === 'reflect' && request.id === 'caller') found.site = reply.site;
      }
      // The negative attempt is the variant's own refused `attempt` install.
      found.attempt_outcome = found.negative?.outcome ?? null;
      found.guest_requests = found.negative ? found.negative.guest_requests
        : found.valid_guest_requests ?? 0;
      found.protected_operations = found.negative ? found.negative.protected_operations
        : found.valid_protected_operations ?? 0;
      return found;
    }),
    authorityRefusal: ungrantedDispatchDenial,
    authorityRefused: ungrantedDispatchDenial !== null,
    stageSteps: steps06.map(step => step.row.stage), installInputs06 },
  'LSLOT-07': { cases: cas, slot: 'emit', pinnedEpoch,
    emit_v1_source: pairedFiles(
      path.join(lane0407.directory, 'LSLOT07-publication-without-grant.requests.jsonl'),
      path.join(lane0407.directory, 'LSLOT07-publication-without-grant.jsonl'))
      .find(step => step.request?.operation === 'install' && step.request.id === 'emit1')
      .request.source,
    stageSteps: casSteps.map(step => step.row.stage) },
  'LSLOT-09': { legacy: summary09Legacy, repl: summary09Repl,
    replProfileLabel: 'guarded Live-Wasm-Draft',
    liveContractsChanged,
    liveBaseline: { commit: LIVE_BASELINE_COMMIT,
      baseline_sha256: sha(liveBaselineBytes), current_sha256: sha(liveCurrentBytes),
      design_changed: liveContractsChanged, state_changes: liveStateChanges,
      approved_changes: APPROVED_LIVE_CHANGES,
      decision: APPROVED_LIVE_CHANGES['LIVE-02'].decision,
      changed_cases: liveDiffs } },
};

// Two failures the review demanded be demonstrable rather than asserted:
// N2-05, removing the in-root publication must turn the LSLOT-02 interleaving
// projection into a mismatch; and R4, the LIVE comparison must accept the
// original field and the owner-approved decision-21a field, and reject any
// other LIVE-01..10 contract change.
const regressions = {};
{
  const fixture02Case = caseById('LSLOT-02');
  const laneState = { results: lanes['LSLOT-02'].results, stages: lanes['LSLOT-02'].stages,
    crossProfile: lanes['LSLOT-02'].crossProfile };
  const interleaving = entries => entries.find(entry => entry.scope === 'input.base'
    && entry.key === 'interleaving');
  const intact = interleaving(projectors['LSLOT-02']({ fixture: fixture02Case, lane: laneState }));
  const mutated = structuredClone(laneState);
  mutated.results = mutated.results.map(item => ({ ...item,
    old_root: { ...item.old_root,
      control_events: (item.old_root.control_events ?? []).filter(event =>
        event.operation !== 'publish') } }));
  const removed = interleaving(projectors['LSLOT-02']({ fixture: fixture02Case, lane: mutated }));
  regressions.lslot02_in_root_publication = { intact: intact.status,
    publication_removed: removed.status, publication_removed_observed: removed.observed };
  if (intact.status !== 'observed') {
    issues.push(`regression: LSLOT-02 interleaving is ${intact.status} on the real evidence`);
  }
  if (removed.status !== 'mismatch') {
    issues.push('regression: removing the in-root publication did not fail the LSLOT-02'
      + ` interleaving (status ${removed.status})`);
  }
}
{
  const withField = (id, field, value) => {
    const document = JSON.parse(liveCurrentBytes);
    let node = document.cases.find(row => row.id === id);
    for (const key of field.slice(0, -1)) node = node[key];
    node[field.at(-1)] = value;
    return Buffer.from(JSON.stringify(document, null, 2) + '\n');
  };
  const diffs = bytes => liveContractDiffs({ baselineBytes: liveBaselineBytes,
    currentBytes: bytes });
  const amendment = APPROVED_LIVE_CHANGES['LIVE-02'];
  const observed = {
    original: diffs(liveCurrentBytes),
    amended: diffs(withField('LIVE-02', amendment.path, amendment.amended)),
    superseded_decision_21: diffs(withField('LIVE-02', amendment.path,
      ['20 twice swap run'])),
    other_case: diffs(withField('LIVE-03', ['expected', 'outcome'], 'other')),
  };
  regressions.live_contract_baseline = observed;
  const expected = { original: [], amended: [], superseded_decision_21: ['LIVE-02'],
    other_case: ['LIVE-03'] };
  for (const [label, value] of Object.entries(observed)) {
    if (JSON.stringify(value) !== JSON.stringify(expected[label])) {
      issues.push(`regression: LIVE baseline ${label} changed=${JSON.stringify(value)}`
        + ` expected=${JSON.stringify(expected[label])}`);
    }
  }
}

// Completeness: every canonical leaf and every variant has exactly one entry.
const cases = [];
const mismatches = [];
const coverageIssues = [];
for (const row of fixture.cases) {
  if (!Object.hasOwn(projectors, row.id)) {
    cases.push(pending(row.id,
      'template identity: this gate does not execute the positive selected-target proof'
      + ' or exact replay case before P-ID-03/P-ID-04 target identity is established'));
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
  const required = [...leaves(row.input, 'input'), ...leaves(row.expected, 'expected'),
    ...(row.input.variants ?? []).map(variant => `variant.${variant.name}`)];
  const covered = entries.map(entry => `${entry.scope}.${entry.key}`);
  const uncovered = required.filter(key => !covered.includes(key));
  const extra = covered.filter(key => !required.includes(key));
  const duplicated = covered.filter((key, index) => covered.indexOf(key) !== index);
  if (uncovered.length > 0 || extra.length > 0 || duplicated.length > 0) {
    coverageIssues.push({ id: row.id, uncovered, extra,
      duplicated: [...new Set(duplicated)] });
    mismatches.push(`${row.id}: coverage uncovered=${uncovered.join(',')}`
      + ` extra=${extra.join(',')} duplicated=${[...new Set(duplicated)].join(',')}`);
  }
  const allowed = declaredAllowlist[row.id] ?? [];
  for (const entry of entries.filter(item => item.status === 'declared')) {
    const key = `${entry.scope}.${entry.key}`;
    if (!allowed.includes(key)) {
      coverageIssues.push({ id: row.id, undeclared: key });
      mismatches.push(`${row.id}: ${key} declared but not on the allowlist`);
    }
  }
  const counted = {};
  for (const status of ['observed', 'declared', 'unobserved', 'mismatch']) {
    counted[status] = entries.filter(entry => entry.status === status).length;
  }
  for (const entry of entries.filter(item => item.status === 'mismatch')) {
    mismatches.push(`${row.id} ${entry.scope}.${entry.key}:`
      + ` canonical ${JSON.stringify(entry.canonical)}`
      + ` observed ${JSON.stringify(entry.observed)}`);
  }
  cases.push({ id: row.id, promotable: counted.unobserved === 0 && counted.mismatch === 0,
    counts: counted, entries,
    declared_keys: allowed.filter(key => covered.includes(key)),
    ...(counted.unobserved > 0 ? { unobserved: entries.filter(entry =>
      entry.status === 'unobserved').map(entry => `${entry.scope}.${entry.key}`) } : {}) });
}

// A previously promoted case must still project as promotable, its committed
// receipt must bind the unchanged design, and the fresh projection must
// reproduce the committed observation values.
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
  assert.equal(record.kind, 'test');
  assert.equal(record.result, 'passed');
  assert.equal(record.subject, row.id);
  assert.equal(record.revision, fixture.revision);
  assert.equal(record.configuration.gate, 'verification/lslot-cases/gate.mjs');
  assert.equal(record.configuration.receipt, promotedReceiptPath);
  assert.equal(record.configuration.case, row.id);
  assert.equal(record.configuration.prepromotion_case_sha256, designSha(row.id),
    `${row.id} canonical design drifted after promotion`);
  const receiptFile = path.resolve(root, 'specs/conformance', record.configuration.receipt);
  const receiptBytes = fs.readFileSync(receiptFile);
  assert.equal(sha(receiptBytes), record.configuration.receipt_sha256,
    `${row.id} committed receipt bytes`);
  const receipt = JSON.parse(receiptBytes);
  assert.equal(receipt.schema, 'noble-lslot-case-acceptance/v1');
  assert.equal(receipt.result, 'passed');
  assert.equal(record.source_revision, receipt.source.revision);
  if (!row.promotable) mismatches.push(`${row.id}: promoted case no longer promotable`);
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
if (sha(fs.readFileSync(binary)) !== binarySha256) issues.push('selected binary changed');
if (sha(fs.readFileSync(path.join(root, fixtureFile))) !== sha(fixtureBytes)) {
  issues.push('canonical fixture bytes changed during the gate');
}
const stageProjection = cases.flatMap(row => row.entries
  .filter(entry => entry.scope === 'expected' && entry.key === 'stage')
  .map(entry => ({ id: row.id, canonical: entry.canonical, status: entry.status,
    ...entry.evidence })));

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
    files: source, selected_trees: selectedTrees, dep_info_paths: dependencies.length,
    dep_info_closure_complete: missingDependencies.length === 0, ingredients: inputs.length },
  fixture: { path: fixtureFile, sha256: sha(fixtureBytes), revision: fixture.revision,
    amendments: fixture.amendments ?? [] },
  tool_selection: { configuration_sha256: source['policy/tool-selection.json'],
    runtime_configuration_sha256: source['crates/noble-cli/src/core/runtime/config.json'],
    child_environment: environment, tools: toolFacts },
  binary: { path: binary, sha256: binarySha256, cargo_target_dir: target,
    build_status: built.status,
    build_claim: 'offline Cargo reported the selected `noble` bin artifact for this target; the'
      + ' recorded sha256 is the exact selected binary this gate executed' },
  projection_rule: {
    completeness: 'every canonical leaf path of `input` and `expected` (arrays are one leaf,'
      + ' objects recurse) and every variant has exactly one entry; uncovered, extra or'
      + ' duplicated keys fail the gate',
    declared_allowlist: declaredAllowlist,
    declared: 'prose/design values only on the per-case allowlist; the entry records the'
      + ' runtime facts that stand behind the prose and is never counted as observed',
    observed: 'a runtime value measured from the lane evidence; it must equal the canonical value',
    unobserved: 'a machine-expressible premise whose canonical attempt did not run; it refuses'
      + ' the case and names the field',
  },
  projectors: Object.keys(projectors),
  coverage: { complete: coverageIssues.length === 0, issues: coverageIssues },
  stage_projection: stageProjection,
  cases: cases.map(row => ({ ...row, binary_sha256: binarySha256,
    prepromotion_case_sha256: designSha(row.id) })),
  promotion: { promoted: summary.promoted, refused: summary.refused,
    not_executed: summary.not_executed,
    rule: 'a case is promotable when every machine-expressible input/expected/variant field is'
      + ' observed and equals the canonical value; one unobserved field refuses the case and one'
      + ' mismatch fails the gate' },
  controls: [
    { lane: 'live_slot_late_control.mjs', summary: control01Late },
    { lane: 'live_slot_selected_origin.mjs', summary: control01Origin },
  ],
  regressions,
  cross_profile: lanes['LSLOT-02'].crossProfile,
  live_cases: lanes['LSLOT-09'].liveBaseline,
  commands,
  summary,
  integrity_issues: issues,
  limitations: [
    'This receipt executes behavior; it is not a proof. Every promoted case keeps `proof: open`.',
    'LSLOT-05 and LSLOT-08 are not executed by this gate: it carries no projector for the'
      + ' positive selected-target proof or the exact replay case, so it neither observes nor'
      + ' refuses them.',
    'LSLOT-04 and LSLOT-07 remain refused on evidence-carrying publication, the same capability'
      + ' LSLOT-05 waits for; their other fields are recorded.',
    '`verification/live-slot-current-source/gate.mjs` and `verification/live-update-corpus` are'
      + ' historical: they replay only at their recorded source states and are not re-run here.',
  ],
  external_evidence_sha256: evidenceFiles,
  raw_mirror: { directory: 'raw', files: Object.keys(evidenceFiles).length,
    command_outputs: commandOutputs.length },
  version_control: { before: beforeGit, after: afterGit },
};
const text = JSON.stringify(acceptance, null, 2) + '\n';
// A run writes a new receipt file. The retained receipts (above) are never
// written: promoting copies this file to a new retained name.
const stamp = new Date().toISOString().replace(/[-:]/g, '').replace(/\.\d+Z$/, 'Z');
const receiptName = `receipt-${stamp}.json`;
assert.ok(!retainedReceipts.some(file => file.endsWith(`/${receiptName}`)),
  `a gate run never writes a retained receipt name: ${receiptName}`);
save(receiptName, text);
save(`${receiptName.replace(/\.json$/, '')}.sha256`, `${sha(text)}  ${receiptName}\n`);
console.log(JSON.stringify({ result, receipt: path.join(output, receiptName),
  receipt_sha256: sha(text), source_revision: sourceRevision,
  binary_sha256: binarySha256, ...summary, integrity_issues: issues.length }));
process.exitCode = result === 'passed' ? 0 : 1;
