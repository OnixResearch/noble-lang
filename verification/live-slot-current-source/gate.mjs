#!/usr/bin/env node
import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { root, sha, sourceInventory, revision, frozen } from '../dx06-current-source/source.mjs';

// A diagnostic execution receipt, never a canonical promotion or proof receipt.
// Usage: pinned Node gate.mjs ABSOLUTE_PREBUILT_CLI NEW_EXTERNAL_TMP_DIRECTORY
const gate = fileURLToPath(import.meta.url);
assert.equal(gate, path.join(root, 'verification/live-slot-current-source/gate.mjs'));
assert.equal(process.argv.length, 4, 'usage: node gate.mjs ABSOLUTE_PREBUILT_CLI NEW_EXTERNAL_TMP_DIRECTORY');
const binary = path.resolve(process.argv[2]);
const output = path.resolve(process.argv[3]);
assert.equal(binary, process.argv[2], 'CLI selection must be an absolute path');
assert.match(output, /^\/tmp\/[^/]+$/, 'receipt must be a fresh, direct /tmp directory');
assert.ok(!fs.existsSync(output), 'receipt output must not exist');
const binaryStat = fs.lstatSync(binary);
assert.ok(binaryStat.isFile() && !binaryStat.isSymbolicLink(), 'selected CLI must be a regular file');
fs.mkdirSync(output, { mode: 0o700 });
const save = (name, content) => fs.writeFileSync(path.join(output, name), content, { flag: 'wx', mode: 0o600 });
const relative = file => path.relative(root, file).split(path.sep).join('/');
const fixtureFile = 'specs/conformance/live-reference-cases.json';
const fixtureBytes = fs.readFileSync(path.join(root, fixtureFile));
const fixture = JSON.parse(fixtureBytes);
assert.deepEqual(fixture.cases.map(row => row.id),
  Array.from({ length: 9 }, (_, i) => `LSLOT-0${i + 1}`));
for (const row of fixture.cases) {
  assert.deepEqual(row.state,
    { implementation: 'absent', execution: 'not-run', proof: 'open', trust: 'unassessed' });
  assert.deepEqual(row.evidence, []);
}
const inputs = [fixtureFile, 'README.md', 'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml',
  'policy/tool-selection.json', 'crates/noble-cli/Cargo.toml',
  'crates/noble-contracts/Cargo.toml', 'crates/noble-kernel/Cargo.toml',
  'crates/noble-wasm/Cargo.toml', 'crates/noble-cli/src/core/runtime/config.json',
  'crates/noble-wasm/tests/live_slot_admission.rs',
  'crates/noble-cli/tests/live_slot_capture_legacy.mjs',
  'crates/noble-cli/tests/live_slot_cas_policy.mjs',
  'crates/noble-cli/tests/live_slot_cas_policy_authority.json',
  'crates/noble-cli/tests/live_slot_resource_ref_cli.mjs',
  'crates/noble-cli/tests/live_slot_resource_ref_admission.mjs',
  'crates/noble-cli/tests/live_slot_resource_ref_static.mjs',
  'crates/noble-cli/tests/live_slot_nested_roots.mjs',
  'crates/noble-cli/tests/live_slot_proof_refusal.mjs',
  'crates/noble-cli/tests/live_slot_quota_replay.mjs',
  'crates/noble-cli/tests/live_slot_quota_replay_authority.json',
  'verification/live-slot-current-source/gate.mjs',
  'verification/dx06-current-source/source.mjs'];
const selectedTrees = [
  'crates/noble-kernel/src', 'crates/noble-contracts/src', 'crates/noble-wasm/src',
  'crates/noble-wasm/runtime', 'crates/noble-wasm/wit', 'crates/noble-cli/src',
  'proofs/mc1/NobleContracts',
];
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
const wasmTools = config.tools.wasm_tools.path;
assert.equal(fs.realpathSync(process.execPath), fs.realpathSync(node),
  'run this gate itself with the pinned Node executable');
const target = path.dirname(path.dirname(binary));
assert.match(target, /^\/tmp\/[^/]+$/, 'selected CLI build target must be external /tmp');
assert.equal(binary, path.join(target, 'debug/noble'), 'selected CLI must be the debug noble target');
const rustlib = path.join(selected.tool_paths.quality_rust.output,
  'lib/rustlib/x86_64-unknown-linux-gnu/lib');
const standardLibrary = fs.readdirSync(rustlib).find(name => /^libstd-.*\.rlib$/.test(name));
assert.ok(standardLibrary, 'selected toolchain lacks target standard library');
const standardLibraryPath = fs.realpathSync(path.join(rustlib, standardLibrary));
const standardLibraryRoot = standardLibraryPath.split('/lib/rustlib/')[0];
assert.ok(standardLibraryRoot.startsWith('/nix/store/'), 'selected standard library must be pinned');
const env = { ...process.env, PATH: `${rustBin}:${selected.component_sync.linker_bin}:${process.env.PATH}`,
  CARGO_TARGET_DIR: target, RUSTC_WRAPPER: '', RUSTC_WORKSPACE_WRAPPER: '',
  RUSTFLAGS: `--sysroot=${standardLibraryRoot}` };
// The Nix toolchain bundle symlinks std from its separately pinned output.
const sysroot = spawnSync(rustc, ['--print', 'sysroot'], { encoding: 'utf8' });
assert.equal(sysroot.status, 0, sysroot.stderr);
const commands = [];
const issues = [];
function gitDiagnostic() {
  const run = args => {
    const result = spawnSync('git', args, { cwd: root, env, encoding: 'utf8' });
    assert.equal(result.status, 0, result.stderr);
    return result.stdout.trim();
  };
  const selectedStatus = run(['status', '--porcelain=v1', '--untracked-files=all',
    '--', ...selectedTrees, 'crates/noble-cli/tests', ...inputs]);
  return { head: run(['rev-parse', 'HEAD']), selected_status: selectedStatus,
    selected_clean: selectedStatus.length === 0 };
}
const beforeGit = gitDiagnostic();
function command(label, executable, args, extraEnv = {}, input = null, expectedStatus = 0) {
  const response = spawnSync(executable, args, { cwd: root, env: { ...env, ...extraEnv },
    ...(input === null ? {} : { input }),
    timeout: 900_000, maxBuffer: 128 * 1024 * 1024 });
  const stdout = response.stdout ?? Buffer.alloc(0), stderr = response.stderr ?? Buffer.alloc(0);
  const stem = `${String(commands.length).padStart(2, '0')}-${label}`;
  save(`${stem}.stdout`, stdout);
  save(`${stem}.stderr`, stderr);
  if (input !== null) save(`${stem}.stdin`, input);
  const result = { label, executable, executable_sha256: sha(fs.readFileSync(executable)), args,
    status: response.status, signal: response.signal, error: response.error?.message ?? null,
    stdin: input === null ? null : `${stem}.stdin`,
    stdin_sha256: input === null ? null : sha(input),
    stdout: `${stem}.stdout`, stdout_sha256: sha(stdout),
    stderr: `${stem}.stderr`, stderr_sha256: sha(stderr) };
  commands.push(result);
  if (result.status !== expectedStatus || result.error || result.signal)
    issues.push(`${label}: exit ${result.status}, ${result.error ?? result.signal ?? stderr.toString().slice(-500)}`);
  return { ...result, text: stdout.toString('utf8') };
}
const toolFacts = {};
for (const [name, executable, args] of [
  ['node', node, ['-p', 'JSON.stringify({node:process.version,v8:process.versions.v8})']],
  ['wasm-tools', wasmTools, ['--version']],
  ['rustc', rustc, ['--version']],
  ['cargo', cargo, ['--version']],
]) {
  const result = command(`tool-${name}`, executable, args);
  toolFacts[name] = { path: executable, sha256: result.executable_sha256,
    version: result.text.trim(), status: result.status };
}
try {
  const observed = JSON.parse(toolFacts.node.version);
  assert.equal(observed.node, `v${config.tools.node.version}`);
  assert.equal(observed.v8, config.tools.node.v8);
  assert.ok(toolFacts['wasm-tools'].version.includes(config.tools.wasm_tools.version));
} catch (error) { issues.push(`selected runtime tool mismatch: ${error}`); }
const beforeBinary = sha(fs.readFileSync(binary));
const built = command('offline-source-build', cargo,
  ['build', '-p', 'noble-cli', '--locked', '--offline', '--message-format=json']);
const afterBinary = sha(fs.readFileSync(binary));
if (built.status === 0) {
  try {
    const artifacts = built.text.split('\n').filter(line => line.startsWith('{'))
      .map(line => JSON.parse(line)).filter(row => row.reason === 'compiler-artifact'
        && row.target.name === 'noble' && row.target.kind.includes('bin'));
    assert.ok(artifacts.some(row => row.executable === binary),
      'selected CLI not present in fresh offline Cargo build receipts');
  } catch (error) { issues.push(`source-to-binary build binding: ${error}`); }
}
const binarySelection = { path: binary, sha256: afterBinary, before_build_sha256: beforeBinary,
  changed_during_offline_cargo_build: beforeBinary !== afterBinary,
  offline_build_status: built.status, cargo_target_dir: target,
  build_claim: beforeBinary === afterBinary
    ? 'offline Cargo reported the selected bin artifact; unchanged bytes do not prove a fresh bytewise compilation'
    : 'offline Cargo reported the selected bin artifact and its selected bytes changed' };
const evidenceDirectories = [];
const work = name => {
  const dir = path.join(output, name);
  fs.mkdirSync(dir, { mode: 0o700 });
  evidenceDirectories.push(dir);
  return dir;
};
const observed = new Map();
const caseId = number => `LSLOT-0${number}`;
function row(id, name, status, reason, evidence = [], counters = null, observation = null,
  diagnostic = {}) {
  assert.ok(!observed.has(`${id}/${name}`), `duplicate evidence row ${id}/${name}`);
  assert.ok(['passed', 'failed', 'blocked'].includes(status));
  observed.set(`${id}/${name}`, { case_id: id, variant: name, status, reason,
    claim: diagnostic.claim ?? (status === 'passed' ? 'canonical_variant_pass' : null),
    guest_requests: counters?.guest_requests ?? null,
    protected_operations: counters?.protected_operations ?? null,
    counter_scope: diagnostic.counter_scope ?? (counters ? 'selected observed attempt' : 'not asserted'),
    observed_attempt: diagnostic.attempt ?? counters ?? null,
    observed_session: diagnostic.session ?? null,
    evidence, observation });
}
function summedCounters(reports) {
  return reports.reduce((total, report) => ({
    guest_requests: total.guest_requests + (report.guest_requests ?? 0),
    protected_operations: total.protected_operations + (report.protected_operations ?? 0),
  }), { guest_requests: 0, protected_operations: 0 });
}
const execution = (name, script, evidence, extraEnv = {}) =>
  command(name, node, [script, binary], { NOBLE_SLOT_EVIDENCE_DIR: evidence, ...extraEnv });
const jsonLines = file => fs.readFileSync(file, 'utf8').trim().split('\n').filter(Boolean).map(line => JSON.parse(line));
const receiptFile = file => ({ path: file, sha256: sha(fs.readFileSync(file)) });
const binaryStill = () => assert.equal(sha(fs.readFileSync(binary)), afterBinary,
  'selected CLI changed during scenario execution');

// LSLOT-01: the instrumented, real anonymous-pipe CLI session retains every
// request, response, selected authority byte and saved owner in its raw JSONL.
{
  const dir = work('capture');
  const result = execution('LSLOT01-capture', 'crates/noble-cli/tests/live_slot_capture_legacy.mjs', dir);
  try {
    assert.equal(result.status, 0);
    const file = path.join(dir, 'LSLOT01.jsonl'), records = jsonLines(file);
    const requests = records.filter(item => item.kind === 'operator');
    const responses = records.filter(item => item.kind === 'cli').map(item => item.row);
    assert.equal(requests.length + 1, responses.length);
    assert.ok(requests.length >= 10 && records.some(item => item.kind === 'authority'));
    const executed = responses.filter(item => item.outcome === 'executed');
    assert.deepEqual(executed.at(-2).stack, [{ kind: 1, value: '21' }]);
    assert.deepEqual(executed.at(-1).stack, [{ kind: 1, value: '22' }]);
    row(caseId(1), 'saved-capture-versus-opt-in-generic-root', 'passed',
      'real selected CLI retained old saved owner and fresh opt-in target',
      [receiptFile(file)], { guest_requests: executed.at(-1).guest_requests,
        protected_operations: executed.at(-1).protected_operations },
      { owners: executed.map(item => item.stack?.[0]?.owner ?? null),
        selected_dispatch: executed.at(-1).request_trace },
      { session: summedCounters(responses) });
  } catch (error) { row(caseId(1), 'saved-capture-versus-opt-in-generic-root', 'failed', String(error)); }
  binaryStill();
}

// LSLOT-02: independent held-root A->B->B and A->B->C sessions. These are
// physical epoch-2/3 baselines because the CLI's global-CAS registry must
// publish each distinct slot separately; canonical epoch 1 remains unproved.
{
  const dir = fs.mkdtempSync('/tmp/noble-lslot-resource-02-');
  fs.chmodSync(dir, 0o700);
  evidenceDirectories.push(dir);
  const result = execution('LSLOT02-nested-roots',
    'crates/noble-cli/tests/live_slot_nested_roots.mjs', dir);
  const summaryFile = path.join(dir, 'LSLOT02-summary.json');
  let summary;
  try { summary = JSON.parse(fs.readFileSync(summaryFile)); }
  catch (error) { issues.push(`LSLOT-02: summary unavailable: ${error}`); }
  for (const name of ['base-parent-child', 'transitive-A-B-C']) {
    const raw = path.join(dir, `LSLOT02-${name}.jsonl`);
    const requests = path.join(dir, `LSLOT02-${name}.requests.jsonl`);
    const responses = path.join(dir, `LSLOT02-${name}.responses.jsonl`);
    const stderrFile = path.join(dir, `LSLOT02-${name}.stderr.txt`);
    const files = [raw, requests, responses, stderrFile, summaryFile]
      .filter(fs.existsSync).map(receiptFile);
    try {
      assert.ok([0, 1].includes(result.status), 'real nested CLI script did not finish with a scenario summary');
      assert.ok(summary, 'real nested CLI script omitted its per-variant scenario summary');
      const item = summary.results.find(value => value.name === name);
      assert.ok(item, `missing independent ${name} session`);
      assert.equal(item.selected_binary_sha256, afterBinary);
      assert.equal(item.status, 'observed', item.error);
      assert.equal(item.initial_epoch, name === 'base-parent-child' ? '2' : '3');
      assert.equal(item.committed_epoch, name === 'base-parent-child' ? '3' : '4');
      const records = jsonLines(raw);
      assert.equal(records[0].kind, 'authority');
      assert.equal(records.at(-1).kind, 'exit');
      assert.equal(records.at(-1).code, 0);
      const replies = records.filter(value => value.kind === 'cli').map(value => value.row);
      const commands = records.filter(value => value.kind === 'operator').map(value => value.request);
      assert.deepEqual(jsonLines(requests), commands);
      assert.deepEqual(jsonLines(responses), replies);
      assert.ok(fs.existsSync(stderrFile));
      const entered = replies.findIndex(value => value.outcome === 'checkpoint-entered');
      const ack = replies.findIndex((value, index) => index > entered &&
        value.outcome === 'published' && value.control_id === '1');
      const oldRoot = replies.findIndex((value, index) => index > ack &&
        value.outcome === 'executed' && value.control_events?.length);
      assert.ok(entered >= 0 && ack > entered && oldRoot > ack,
        'physical checkpoint, committed ACK and pinned root must be ordered');
      assert.deepEqual(replies[oldRoot].request_trace, item.old_root.request_trace);
      assert.deepEqual([item.old_root.guest_requests, item.old_root.protected_operations], [3, 0]);
      assert.equal(item.old_root.epoch, item.initial_epoch);
      assert.equal(item.new_root.epoch, item.committed_epoch);
      assert.equal(item.next_leaf.epoch, item.committed_epoch);
      const sessionCounters = summedCounters(replies);
      row(caseId(2), name, 'blocked', item.limitation, files, null, item,
        { attempt: { guest_requests: item.old_root.guest_requests,
          protected_operations: item.old_root.protected_operations },
        session: sessionCounters });
      row(caseId(2), `${name}-held-control`, 'passed',
        'real selected CLI held root kept its map across committed publication; fresh root selected new target',
        files, { guest_requests: 3, protected_operations: 0 },
        { checkpoint: item.checkpoint, committed_ack: item.committed_ack,
          old_root: item.old_root, new_root: item.new_root, next_leaf: item.next_leaf,
          generic_site: item.generic_site },
        { claim: 'control_pass', session: sessionCounters });
    } catch (error) {
      row(caseId(2), name, 'failed', String(error), files);
      row(caseId(2), `${name}-held-control`, 'failed', String(error), files);
    }
  }
  binaryStill();
}

// LSLOT-05 can execute only the generic missing-evidence refusal, not an
// applicability judgment for P2/D2 or any of its eight canonical variants.
{
  const dir = fs.mkdtempSync('/tmp/noble-lslot-resource-05-');
  fs.chmodSync(dir, 0o700);
  evidenceDirectories.push(dir);
  const result = execution('LSLOT05-proof-required-control',
    'crates/noble-cli/tests/live_slot_proof_refusal.mjs', dir);
  const raw = path.join(dir, 'LSLOT05-proof-required-refusal-control.jsonl');
  const summaryFile = path.join(dir, 'LSLOT05-proof-required-refusal-control.summary.json');
  const requests = path.join(dir, 'LSLOT05-proof-required-refusal-control.requests.jsonl');
  const responses = path.join(dir, 'LSLOT05-proof-required-refusal-control.responses.jsonl');
  const stderrFile = path.join(dir, 'LSLOT05-proof-required-refusal-control.stderr.txt');
  const files = [raw, requests, responses, stderrFile, summaryFile]
    .filter(fs.existsSync).map(receiptFile);
  try {
    assert.equal(result.status, 0);
    const item = JSON.parse(fs.readFileSync(summaryFile));
    assert.equal(item.selected_binary_sha256, afterBinary);
    assert.equal(item.proof_required.outcome, 'refused');
    assert.equal(item.ordinary_control.outcome, 'published');
    assert.equal(item.ordinary_control.epoch, '1');
    assert.deepEqual([item.proof_required.guest_requests, item.proof_required.protected_operations], [0, 0]);
    const records = jsonLines(raw);
    assert.equal(records[0].kind, 'authority');
    assert.equal(records.at(-1).kind, 'exit');
    assert.equal(records.at(-1).code, 0);
    const replies = records.filter(value => value.kind === 'cli').map(value => value.row);
    assert.deepEqual(jsonLines(requests),
      records.filter(value => value.kind === 'operator').map(value => value.request));
    assert.deepEqual(jsonLines(responses), replies);
    assert.ok(fs.existsSync(stderrFile));
    assert.ok(replies.some(value => value.outcome === 'refused' &&
      value.diagnostic === item.proof_required.diagnostic));
    assert.ok(replies.some(value => value.outcome === 'published' && value.epoch === '1'));
    row(caseId(5), 'proof-required-refusal-control', 'passed',
      'real selected CLI refused missing independent proof while the same candidate published to a non-proof slot; this does not test evidence applicability',
      files, { guest_requests: 0, protected_operations: 0 }, item,
      { claim: 'control_pass', session: summedCounters(replies) });
  } catch (error) {
    row(caseId(5), 'proof-required-refusal-control', 'failed', String(error), files);
  }
  binaryStill();
}

// Each canonical LSLOT-03/06 variant has an independent real CLI session.
for (const [number, script, prefix] of [
  [3, 'live_slot_resource_ref_admission.mjs', 'LSLOT03'],
  [6, 'live_slot_resource_ref_static.mjs', 'LSLOT06'],
]) {
  const id = caseId(number);
  const dir = fs.mkdtempSync(`/tmp/noble-lslot-resource-${number}-`);
  fs.chmodSync(dir, 0o700);
  evidenceDirectories.push(dir);
  const result = execution(id, `crates/noble-cli/tests/${script}`, dir);
  let summary;
  try { summary = JSON.parse(fs.readFileSync(path.join(dir, `${prefix}-summary.json`))); }
  catch (error) { issues.push(`${id}: summary unavailable: ${error}`); }
  const variants = fixture.cases[number - 1].input.variants;
  for (const variant of variants) {
    const item = summary?.results.find(result => result.name === variant.name);
    const raw = path.join(dir, `${prefix}-${variant.name}.jsonl`);
    try {
      assert.equal(result.status, 0);
      assert.ok(item && fs.existsSync(raw), 'variant CLI session missing');
      assert.equal(item.selected_binary_sha256, afterBinary);
      const records = jsonLines(raw);
      assert.ok(records.some(record => record.kind === 'operator'));
      assert.ok(records.some(record => record.kind === 'cli'));
      assert.equal(records.at(-1).kind, 'exit');
      assert.equal(records.at(-1).code, 0);
      const attempted = number === 3
        ? { guest_requests: item.guest_requests, protected_operations: item.protected_operations }
        : variant.name === 'valid-borrow'
          ? { guest_requests: item.valid_guest_requests,
            protected_operations: item.valid_protected_operations }
          : item.negative_guest_requests === null ? null
            : { guest_requests: item.negative_guest_requests,
              protected_operations: item.negative_protected_operations };
      const status = item.status === 'pass' ? 'passed' : 'blocked';
      row(id, variant.name, status,
        item.limitation ?? (item.status === 'pass' ? 'exact CLI variant observed'
          : `source protocol did not reach canonical attempt: ${item.status}`),
        [receiptFile(raw), receiptFile(path.join(dir, `${prefix}-summary.json`))],
        status === 'passed' ? attempted : null, item,
        { attempt: attempted, session: summedCounters(records.filter(record =>
          record.kind === 'cli').map(record => record.row)) });
    } catch (error) { row(id, variant.name, 'failed', String(error),
      fs.existsSync(raw) ? [receiptFile(raw)] : []); }
  }
  if (number === 3) {
    const name = 'noncanonical-effect-ceiling-control';
    const summaryFile = path.join(dir, 'LSLOT03-summary.json');
    const files = [summaryFile];
    for (const kind of ['empty-ceiling', 'matching-ceiling']) {
      const stem = path.join(dir, `LSLOT03-ceiling-control-${kind}`);
      files.push(`${stem}.jsonl`, `${stem}.requests.jsonl`,
        `${stem}.responses.jsonl`, `${stem}.stderr.txt`);
    }
    const evidence = files.filter(fs.existsSync).map(receiptFile);
    try {
      assert.equal(result.status, 0);
      assert.deepEqual(variants.find(variant => variant.name === 'effect-ceiling-widened')
        .candidate_effects, ['test.emit', 'fs.write']);
      const canonicalRecords = jsonLines(path.join(dir, 'LSLOT03-effect-ceiling-widened.jsonl'));
      assert.ok(canonicalRecords.some(record => record.kind === 'operator'
        && record.request.operation === 'install'
        && record.request.source === '"admit" test.emit drop "path" fs.write'));
      assert.equal(observed.get(`${id}/effect-ceiling-widened`)?.status, 'blocked');
      assert.deepEqual(summary.ceiling_control.map(item => item.name),
        ['empty-ceiling', 'matching-ceiling']);
      const artifacts = [];
      const observations = [];
      for (const [kind, ceiling, outcome] of [
        ['empty-ceiling', [], 'refused'], ['matching-ceiling', ['test.emit'], 'published'],
      ]) {
        const stem = path.join(dir, `LSLOT03-ceiling-control-${kind}`);
        const records = jsonLines(`${stem}.jsonl`);
        const selected = records[0];
        assert.equal(selected.kind, 'authority');
        assert.equal(selected.selected_binary, binary);
        assert.equal(selected.selected_binary_sha256, afterBinary);
        const authority = selected.authority;
        assert.deepEqual(authority.sources.map(source => source.id),
          ['candidate', 'decoy', 'pure']);
        assert.equal(authority.sources[0].sha256, sha(Buffer.from('"admit" test.emit drop')));
        assert.equal(authority.sources[1].sha256, sha(Buffer.from('"decoy" test.emit drop')));
        assert.equal(authority.sources[2].sha256, sha(Buffer.from('swap swap')));
        assert.deepEqual(authority.slots.map(slot => [slot.slotId, slot.input, slot.output,
          slot.effectCeiling, slot.proofRequired]), [
          ['account', ['Account@1', 'I64'], ['Account@1', 'I64'], ceiling, false],
          ['decoy', ['Account@1', 'I64'], ['Account@1', 'I64'], ['test.emit'], false],
        ]);
        assert.ok(authority.grants.some(grant => grant.operation === 'publish'
          && grant.slotId === 'account' && grant.allowed === true));
        assert.ok(authority.grants.some(grant => grant.operation === 'effect'
          && grant.slotId === 'decoy' && grant.allowed === true));
        assert.ok(!authority.grants.some(grant => grant.operation === 'effect'
          && grant.slotId === 'account'));
        assert.deepEqual(authority.effects, ['test.emit', 'live.dispatch']);
        assert.equal(records.at(-1).kind, 'exit');
        assert.equal(records.at(-1).code, 0);
        const requests = records.filter(record => record.kind === 'operator')
          .map(record => record.request);
        const replies = records.filter(record => record.kind === 'cli')
          .map(record => record.row);
        assert.deepEqual(jsonLines(`${stem}.requests.jsonl`), requests);
        assert.deepEqual(jsonLines(`${stem}.responses.jsonl`), replies);
        assert.ok(fs.existsSync(`${stem}.stderr.txt`));
        assert.deepEqual(requests, [
          { operation: 'trace' },
          { operation: 'install', id: 'candidate', source: '"admit" test.emit drop',
            inputs: ['Account@1', 'I64'] },
          { operation: 'publish', slot: 'account', id: 'candidate', expected_epoch: '0' },
          ...(kind === 'empty-ceiling' ? [
            { operation: 'install', id: 'pure', source: 'swap swap',
              inputs: ['Account@1', 'I64'] },
            { operation: 'publish', slot: 'account', id: 'pure', expected_epoch: '0' },
          ] : []),
          { operation: 'trace' },
        ]);
        assert.deepEqual(replies.map(reply => reply.outcome),
          kind === 'empty-ceiling'
            ? ['configured', 'trace-observed', 'installed', 'refused',
              'installed', 'published', 'trace-observed']
            : ['configured', 'trace-observed', 'installed', 'published', 'trace-observed']);
        const installed = replies[2], publication = replies[3];
        assert.match(installed.artifact_sha256, /^[0-9a-f]{64}$/);
        artifacts.push(installed.artifact_sha256);
        assert.equal(publication.outcome, outcome);
        if (kind === 'empty-ceiling') {
          assert.match(publication.diagnostic,
            /checked candidate differs from selected slot interface or effect ceiling/);
          assert.equal(replies[5].epoch, '1');
        } else assert.equal(publication.epoch, '1');
        assert.deepEqual(replies[1].trace, []);
        assert.deepEqual(replies.at(-1).trace, []);
        for (const reply of replies)
          assert.deepEqual([reply.guest_requests, reply.protected_operations], [0, 0]);
        const item = summary.ceiling_control.find(control => control.name === kind);
        assert.equal(item.selected_binary_sha256, afterBinary);
        assert.equal(item.raw, `${stem}.jsonl`);
        assert.equal(item.candidate_artifact_sha256, installed.artifact_sha256);
        assert.equal(item.publication, outcome);
        assert.equal(item.epoch_after, '1');
        assert.deepEqual([item.guest_requests, item.protected_operations, item.trace], [0, 0, []]);
        observations.push({ kind, publication, committed: kind === 'empty-ceiling'
          ? replies[5] : publication, artifact_sha256: installed.artifact_sha256 });
      }
      assert.equal(artifacts[0], artifacts[1],
        'two ceiling policies must select identical checked candidate artifacts');
      row(id, name, 'passed',
        'independent decoy-enabled host sessions refuse the effectful candidate under an empty account ceiling and publish it under a matching ceiling; pure candidate publishes after refusal',
        evidence, { guest_requests: 0, protected_operations: 0 }, observations,
        { claim: 'control_pass', session: { guest_requests: 0, protected_operations: 0 } });
    } catch (error) { row(id, name, 'failed', String(error), evidence); }
  }
  binaryStill();
}

// CAS and current-policy interleavings retain separate CLI stdout, original
// command stream, stderr and summary; dynamic sessions do not share a map.
{
  const dir = work('cas-policy');
  const result = execution('LSLOT04-LSLOT07',
    'crates/noble-cli/tests/live_slot_cas_policy.mjs', dir);
  for (const number of [4, 7]) {
    const id = caseId(number);
    for (const variant of fixture.cases[number - 1].input.variants) {
      const name = `${id.replace('-', '')}-${variant.name}`;
      const summaryFile = path.join(dir, `${name}.counters.json`);
      const raw = path.join(dir, `${name}.jsonl`);
      const requests = path.join(dir, `${name}.requests.jsonl`);
      const stderr = path.join(dir, `${name}.stderr.txt`);
      try {
        assert.equal(result.status, 0);
        const item = JSON.parse(fs.readFileSync(summaryFile));
        const responses = jsonLines(raw), commands = jsonLines(requests);
        assert.equal(item.result, 'pass');
        assert.ok(responses.length > 3 && commands.length > 3);
        assert.ok(fs.existsSync(stderr));
        const refusal = item.refusal ?? item.denied;
        const counters = refusal ? {
          guest_requests: refusal.guest_requests,
          protected_operations: refusal.protected_operations,
        } : { guest_requests: 0, protected_operations: 0 };
        const evidence = [summaryFile, raw, requests, stderr].map(receiptFile);
        const session = summedCounters(responses);
        const missingEvidence = number === 4 && variant.name === 'authorized-rollback';
        const missingProof = number === 7 && variant.name === 'publication-without-grant';
        if (missingProof) {
          const authority = JSON.parse(fs.readFileSync(path.join(root,
            'crates/noble-cli/tests/live_slot_cas_policy_authority.json')));
          assert.equal(authority.slots.find(slot => slot.slotId === 'emit').proofRequired, false);
        }
        if (missingEvidence || missingProof) {
          const reason = missingEvidence
            ? 'newly installed Ainc8 rolled back by CAS, but no independently applicable exact evidence for this selected target was supplied'
            : 'publication denial observed with proofRequired=false and no admitted proof; canonical slot_id_and_proof_present premise is absent';
          row(id, variant.name, 'blocked', reason, evidence, null, item,
            { attempt: counters, session });
          row(id, `${variant.name}-unauthenticated-control`, 'passed',
            'real selected CLI CAS/policy behavior only; canonical evidence/proof premise missing',
            evidence, counters, item, { claim: 'control_pass', session });
        } else {
          row(id, variant.name, 'passed', 'real selected CLI CAS/policy interleaving',
            evidence, counters, item, { session });
        }
      } catch (error) {
        row(id, variant.name, 'failed', String(error),
          [summaryFile, raw, requests, stderr].filter(fs.existsSync).map(receiptFile));
      }
    }
  }
  binaryStill();
}

// Retention and twelve independent replay attempts are genuine CLI executions.
// Replay uses a named static traced definition with no captured I64:5 and
// 'emitted:A/B', not the canonical captured Dtrace and scripted ok-A/ok-B.
{
  const dir = work('quota-replay');
  const result = execution('LSLOT08-quota-replay',
    'crates/noble-cli/tests/live_slot_quota_replay.mjs', dir);
  let summary;
  try { summary = JSON.parse(result.text.trim()); }
  catch (error) { issues.push(`LSLOT-08 CLI summary unavailable: ${error}`); }
  const names = ['retention', ...fixture.cases[7].input.replay_fixture.variants.map(v => `replay-${v.name}`)];
  for (const name of names) {
    const raw = path.join(dir, `${name}.raw.jsonl`), requests = path.join(dir, `${name}.requests.jsonl`);
    const stderr = path.join(dir, `${name}.stderr.txt`);
    try {
      assert.ok(summary, 'overall CLI script did not emit its execution summary');
      const responses = jsonLines(raw), commands = jsonLines(requests);
      assert.ok(responses.length > 3 && commands.length > 3);
      const item = name === 'retention' ? summary.retention :
        summary.replay.find(row => row.name === name.slice('replay-'.length));
      assert.ok(item, 'missing named CLI scenario summary');
      assert.notEqual(item.status, 'failed', item.error);
      if (name !== 'retention') {
        assert.equal(item.outcome, name === 'replay-exact-frozen-admitted-and-ordered'
          ? 'replay-matched' : name === 'replay-bare-slot-id' ? 'refused' : 'replay-diverged');
        assert.equal(responses.find(row => row.outcome === 'executed' && row.replay_token)?.epoch, '9');
      }
      const attempt = name === 'retention'
        ? responses.find(row => row.outcome === 'executed' && row.epoch === '2' &&
          row.request_trace?.some(trace => trace.operation === 'dispatch'))
        : { guest_requests: item.guest_requests,
          protected_operations: item.protected_operations,
          scripted_operations: item.scripted_operations };
      assert.ok(attempt, `missing ${name} observed attempt counters`);
      const attemptCounters = { guest_requests: attempt.guest_requests,
        protected_operations: attempt.protected_operations,
        ...(attempt.scripted_operations === undefined ? {} :
          { scripted_operations: attempt.scripted_operations }) };
      row(caseId(8), name, name === 'retention' ? 'passed' : 'blocked',
        name === 'retention' ? 'real retained-owner last-pin quota and retirement observed'
          : 'real replay observation is partial: selected named static definition lacks canonical captured I64:5 Dtrace and scripted ok-A/ok-B',
        [raw, requests, ...(fs.existsSync(stderr) ? [stderr] : [])].map(receiptFile),
        name === 'retention' ? attemptCounters : null, item,
        { attempt: attemptCounters, session: summedCounters(responses) });
    } catch (error) { row(caseId(8), name, 'failed', String(error),
      [raw, requests, stderr].filter(fs.existsSync).map(receiptFile)); }
  }
  const capacity = path.join(dir, 'bounded-trace-retention.raw.jsonl');
  const capacityRequests = path.join(dir, 'bounded-trace-retention.requests.jsonl');
  try {
    assert.ok(fs.existsSync(capacity), 'bounded trace CLI session missing');
    const trace = jsonLines(capacity);
    assert.ok(summary?.boundedTrace, summary?.failures.find(item =>
      item.name === 'bounded-trace-retention')?.error ?? 'bounded trace summary missing');
    assert.ok(trace.some(item => item.outcome === 'trace-capacity-refused'));
    assert.equal(trace.filter(item => item.outcome === 'executed').length,
      summary.boundedTrace.completed);
    row(caseId(8), 'bounded-trace-retention-control', 'passed',
      'bounded trace refuses rather than truncating prior committed trace',
      [capacity, capacityRequests].map(receiptFile),
      { guest_requests: summary.boundedTrace.completed, protected_operations: 0 },
      summary.boundedTrace,
      { claim: 'control_pass', session: summedCounters(trace),
        counter_scope: 'all successful roots in bounded-capacity control' });
  } catch (error) {
    row(caseId(8), 'bounded-trace-retention-control', 'failed', String(error),
      [capacity, capacityRequests].filter(fs.existsSync).map(receiptFile),
      summary?.failures.find(item => item.name === 'bounded-trace-retention') ?? null);
  }
  binaryStill();
}

// The checked typed LiveRef caller can reach three legacy compiler-profile
// boundaries through the selected Rust test, but not the legacy CLI surface.
{
  const checked = command('LSLOT09-typed-legacy-compiler', cargo,
    ['test', '-p', 'noble-wasm', '--test', 'live_slot_admission',
      'legacy_profiles_and_forged_slot_metadata_are_rejected',
      '--locked', '--offline', '--', '--exact']);
  const files = [checked.stdout, checked.stderr].map(file =>
    receiptFile(path.join(output, file)));
  try {
    assert.equal(checked.status, 0);
    assert.match(checked.text, /legacy_profiles_and_forged_slot_metadata_are_rejected \.\.\. ok/);
    assert.match(checked.text, /test result: ok\. 1 passed;/);
    row(caseId(9), 'typed-checked-compiler-control', 'passed',
      'selected Rust checker rejected a genuine checked typed LiveRef caller in legacy compilers; this is not the canonical CLI attempt',
      files, null, { test: 'legacy_profiles_and_forged_slot_metadata_are_rejected',
        profiles: ['Core', 'Live-Wasm-Draft', 'Text-Byte-Cursor'] },
      { claim: 'control_pass' });
  } catch (error) {
    row(caseId(9), 'typed-checked-compiler-control', 'failed', String(error), files);
  }
  binaryStill();
}

// Three independent legacy entry points reject the opt-in word before any
// guest request. The historical capture controls are separate obligations.
{
  const sourceFile = path.join(output, 'legacy-slot-attempt.noble');
  save('legacy-slot-attempt.noble', 'slot.invoke\n');
  for (const [name, args, input] of [
    ['Core-Bootstrap', ['compile', sourceFile], null],
    ['ordinary Core session', ['session'], 'slot.invoke\n'],
    ['guarded Live-Wasm-Draft', ['live', 'repl', '--source', sourceFile, '--engine', 'v8'], null],
  ]) {
    const result = command(`LSLOT09-${name.replaceAll(' ', '-')}`, binary, args, {}, input,
      name === 'guarded Live-Wasm-Draft' ? 2 : 4);
    try {
      assert.ok([2, 4].includes(result.status), 'unsupported profile unexpectedly accepted slot.invoke');
      const refusal = JSON.parse(result.text.trim());
      assert.equal(refusal.outcome, name === 'guarded Live-Wasm-Draft' ? 'reload-refused' : 'unsupported');
      assert.match(refusal.diagnostic, /slot\.invoke requires the opt-in live-slot source profile/);
      assert.equal(refusal.guest_requests, 0);
      assert.equal(refusal.protected_operations, 0);
      row(caseId(9), name, 'blocked',
        'real legacy entry point explicitly rejects slot.invoke, but cannot construct the canonical checked caller with typed input LiveRef',
        [receiptFile(sourceFile), receiptFile(path.join(output, result.stdout)),
          receiptFile(path.join(output, result.stderr)),
          ...(result.stdin ? [receiptFile(path.join(output, result.stdin))] : [])],
        null, refusal, { attempt: { guest_requests: refusal.guest_requests,
          protected_operations: refusal.protected_operations },
        session: { guest_requests: refusal.guest_requests,
          protected_operations: refusal.protected_operations } });
    } catch (error) {
      row(caseId(9), name, 'failed', String(error),
        [receiptFile(path.join(output, result.stdout)), receiptFile(path.join(output, result.stderr))]);
    }
  }
  const controlInput = 'def n [ 1 + ]\n[ n ]\ndef n [ 2 + ]\n20 swap run\n';
  const core = command('LSLOT09-Core-saved-capture', binary, ['session'], {}, controlInput);
  try {
    assert.equal(core.status, 0);
    const receipts = jsonLines(path.join(output, core.stdout));
    assert.deepEqual(receipts.map(item => item.outcome),
      ['defined', 'normal', 'defined', 'normal']);
    assert.equal(receipts[1].stack[0].type, 'Program');
    assert.deepEqual(receipts[3].stack.map(item => item.value), ['21']);
    assert.equal(receipts[3].guest_requests, 0);
    assert.equal(receipts[3].protected_operations, 0);
    row(caseId(9), 'control:Core-Bootstrap', 'passed',
      'saved Core quotation still selects pre-redefinition n despite current definition n-v2',
      [core.stdin, core.stdout, core.stderr].map(name => receiptFile(path.join(output, name))),
      { guest_requests: 0, protected_operations: 0 },
      { saved_program: receipts[1].stack[0], old_result: receipts[3].stack },
      { claim: 'control_pass', session: summedCounters(receipts) });
  } catch (error) {
    row(caseId(9), 'control:Core-Bootstrap', 'failed', String(error),
      [core.stdin, core.stdout, core.stderr].map(name => receiptFile(path.join(output, name))));
  }
}

// Draft control needs a real live child: the selected file changes only
// between an observed saved Program and the explicit :reload transaction.
{
  const dir = work('legacy-draft');
  const selectedSource = path.join(dir, 'selected.noble');
  const oldSource = 'def n [ 1 + ]\n', newSource = 'def n [ 2 + ]\n';
  save('draft-v1.noble', oldSource);
  save('draft-v2.noble', newSource);
  fs.writeFileSync(selectedSource, oldSource, { flag: 'wx', mode: 0o600 });
  const transcript = path.join(dir, 'transcript.jsonl');
  const child = spawn(binary, ['live', 'repl', '--source', selectedSource, '--engine', 'v8'],
    { cwd: root, env, stdio: ['pipe', 'pipe', 'pipe'] });
  let buffer = '', stderr = '', pending;
  const queue = [];
  const append = item => fs.appendFileSync(transcript, JSON.stringify(item) + '\n',
    { flag: 'a', mode: 0o600 });
  child.stdout.on('data', bytes => {
    buffer += bytes.toString();
    for (let end; (end = buffer.indexOf('\n')) >= 0;) {
      const report = JSON.parse(buffer.slice(0, end));
      buffer = buffer.slice(end + 1);
      append({ kind: 'cli', report });
      if (pending) { const resolve = pending; pending = null; resolve(report); }
      else queue.push(report);
    }
  });
  child.stderr.on('data', bytes => {
    stderr += bytes.toString();
    append({ kind: 'stderr', text: bytes.toString() });
  });
  const next = () => queue.length ? Promise.resolve(queue.shift()) : new Promise((resolve, reject) => {
    const timeout = setTimeout(() => { pending = null; reject(Error(`guarded draft timeout: ${stderr}`)); }, 30000);
    pending = report => { clearTimeout(timeout); resolve(report); };
  });
  const issue = async text => {
    append({ kind: 'operator', text });
    child.stdin.write(`${text}\n`);
    return next();
  };
  let failure = null, witness = null;
  try {
    const initial = await next();
    assert.equal(initial.outcome, 'reload-committed');
    const saved = await issue('[ n ]');
    assert.equal(saved.outcome, 'normal');
    assert.equal(saved.stack[0].type, 'Program');
    fs.writeFileSync(selectedSource, newSource);
    const reloaded = await issue(`:reload ${selectedSource}`);
    assert.equal(reloaded.outcome, 'reload-committed');
    assert.deepEqual(reloaded.stack, saved.stack);
    const invoked = await issue('20 swap run');
    assert.equal(invoked.outcome, 'normal');
    assert.deepEqual(invoked.stack.map(item => item.value), ['21']);
    assert.equal(invoked.guest_requests, 0);
    assert.equal(invoked.protected_operations, 0);
    witness = { initial, saved_program: saved.stack[0], reload: reloaded,
      old_result: invoked.stack };
  } catch (error) { failure = String(error.stack ?? error); child.kill(); }
  child.stdin.end();
  const status = child.exitCode ?? await new Promise(resolve => child.once('exit', resolve));
  append({ kind: 'exit', status, stderr });
  const commandRecord = { label: 'LSLOT09-draft-saved-capture', executable: binary,
    executable_sha256: afterBinary, args: ['live', 'repl', '--source', selectedSource, '--engine', 'v8'],
    status, transcript, transcript_sha256: sha(fs.readFileSync(transcript)) };
  commands.push(commandRecord);
  if (status !== 0 && failure === null) failure = `guarded draft CLI exited ${status}: ${stderr}`;
  if (buffer) failure = `incomplete guarded draft CLI receipt: ${buffer}`;
  row(caseId(9), 'control:guarded Live-Wasm-Draft', failure ? 'failed' : 'passed',
    failure ?? 'guarded :reload rebinds current n while saved quotation retains old n',
    [transcript, selectedSource, path.join(output, 'draft-v1.noble'),
      path.join(output, 'draft-v2.noble')].map(receiptFile),
    failure ? null : { guest_requests: 0, protected_operations: 0 }, witness,
    { claim: 'control_pass', session: summedCounters(jsonLines(transcript)
      .filter(item => item.kind === 'cli').map(item => item.report)) });
  binaryStill();
}

// Unrun canonical obligations are never inferred from a different refusal.
for (const [id, names, reason] of [
  [caseId(5), fixture.cases[4].input.variants.map(item => item.name),
    'no authenticated checked source-to-quote operand/capture provenance for selected P2/D2, and no real positive exact evidence; blanket proof-required denial is not an admission proof'],
]) for (const name of names) row(id, name, 'blocked', reason);

for (const id of fixture.cases.map(item => item.id)) {
  assert.ok([...observed.values()].some(item => item.case_id === id), `${id} missing case rows`);
}
const evidenceFiles = {};
for (const dir of evidenceDirectories) {
  function walk(current) {
    for (const name of fs.readdirSync(current)) {
      const file = path.join(current, name), stat = fs.lstatSync(file);
      assert.ok(!stat.isSymbolicLink(), `external evidence symlink: ${file}`);
      if (stat.isDirectory()) walk(file);
      else {
        assert.ok(stat.isFile(), `unexpected external evidence object: ${file}`);
        evidenceFiles[file] = sha(fs.readFileSync(file));
      }
    }
  }
  walk(dir);
}
try {
  frozen(source);
  assert.deepEqual(sourceInventory(inputs, selectedTrees), source,
    'selected source tree paths or bytes changed during execution');
} catch (error) { issues.push(`source freeze: ${error}`); }
const afterGit = gitDiagnostic();
if (beforeGit.head !== afterGit.head || beforeGit.selected_status !== afterGit.selected_status)
  issues.push('selected Git HEAD or dirty/clean status changed during source-bound execution');
if (sha(fs.readFileSync(binary)) !== afterBinary) issues.push('selected binary changed after execution');
if (sha(fs.readFileSync(path.join(root, fixtureFile))) !== sha(fixtureBytes))
  issues.push('canonical fixture bytes changed during gate');
const rows = [...observed.values()];
for (const observedRow of rows) {
  if (observedRow.status === 'blocked') {
    assert.equal(observedRow.guest_requests, null,
      `blocked ${observedRow.case_id}/${observedRow.variant} cannot claim canonical counters`);
    assert.equal(observedRow.protected_operations, null);
  }
  if (observedRow.claim === 'canonical_variant_pass') {
    assert.ok(Number.isSafeInteger(observedRow.guest_requests) &&
      Number.isSafeInteger(observedRow.protected_operations),
    `passed canonical variant lacks measured counters: ${observedRow.case_id}/${observedRow.variant}`);
  }
}
const summary = { passed: rows.filter(row => row.status === 'passed').length,
  failed: rows.filter(row => row.status === 'failed').length,
  blocked: rows.filter(row => row.status === 'blocked').length };
const result = summary.failed || issues.length ? 'failed' : 'blocked';
const acceptance = {
  schema: 'noble-live-slot-current-source-diagnostic/v1', result,
  canonical_promotion: false, proof: 'open',
  source_revision: sourceRevision, source_sha256: source,
  fixture: { path: fixtureFile, sha256: sha(fixtureBytes), canonical_state_unchanged: true },
  tool_selection: { configuration_sha256: source['policy/tool-selection.json'],
    runtime_configuration_sha256: source['crates/noble-cli/src/core/runtime/config.json'],
    rust_sysroot: standardLibraryRoot, rustc_default_sysroot: sysroot.stdout.trim(),
    ...toolFacts },
  binary: binarySelection, commands, cases: rows, summary,
  counter_definition: {
    guest_requests_and_protected_operations:
      'selected observed attempt or explicitly named control scope only on passed rows; null for blocked canonical variants and unmeasured static checks',
    observed_attempt:
      'measured CLI attempt even if its full canonical premises are blocked; scripted_operations counts replay-only scripted effects, not real protected operations',
    observed_session:
      'sum of guest_requests/protected_operations fields on each real CLI reply of this independent variant session, including setup/control roots; not a canonical-case counter',
  },
  version_control: { before: beforeGit, after: afterGit },
  external_evidence_sha256: evidenceFiles, integrity_issues: issues,
  limitations: [
    'All nine canonical cases remain absent/not-run/open/unassessed; this external diagnostic does not promote any case.',
    'LSLOT-05 lacks a positive genuine selected-target proof bound to authenticated capture-sensitive P/D identities.',
    'LSLOT-08 replay lacks the canonical dynamically captured I64:5 Dtrace and scripted ok-A/ok-B responses; exact replay never grants real effects.',
    'LSLOT-02 has observed held nested roots at reachable physical epochs 2/3, not its canonical simultaneous epoch-1 multi-slot registry; several admission/static negatives and the exact typed LSLOT-09 legacy caller remain profile-limited.',
  ],
};
save('receipt.json', JSON.stringify(acceptance, null, 2) + '\n');
console.log(JSON.stringify({ result, receipt: path.join(output, 'receipt.json'),
  source_revision: sourceRevision, ...summary, integrity_issues: issues.length }));
process.exitCode = 1;
