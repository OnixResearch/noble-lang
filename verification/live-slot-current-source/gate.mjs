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
  'crates/noble-cli/tests/live_slot_capture_legacy.mjs',
  'crates/noble-cli/tests/live_slot_cas_policy.mjs',
  'crates/noble-cli/tests/live_slot_cas_policy_authority.json',
  'crates/noble-cli/tests/live_slot_resource_ref_cli.mjs',
  'crates/noble-cli/tests/live_slot_resource_ref_admission.mjs',
  'crates/noble-cli/tests/live_slot_resource_ref_static.mjs',
  'crates/noble-cli/tests/live_slot_quota_replay.mjs',
  'crates/noble-cli/tests/live_slot_quota_replay_authority.json',
  'verification/live-slot-current-source/gate.mjs',
  'verification/dx06-current-source/source.mjs'];
const source = sourceInventory(inputs, [
  'crates/noble-kernel/src', 'crates/noble-contracts/src', 'crates/noble-wasm/src',
  'crates/noble-wasm/runtime', 'crates/noble-wasm/wit', 'crates/noble-cli/src',
  'proofs/mc1/NobleContracts',
]);
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
  rebuilt_selected_binary: beforeBinary !== afterBinary,
  offline_build_status: built.status, cargo_target_dir: target };
const evidenceDirectories = [];
const work = name => {
  const dir = path.join(output, name);
  fs.mkdirSync(dir, { mode: 0o700 });
  evidenceDirectories.push(dir);
  return dir;
};
const observed = new Map();
const caseId = number => `LSLOT-0${number}`;
function row(id, name, status, reason, evidence = [], counters = null, observation = null) {
  assert.ok(!observed.has(`${id}/${name}`), `duplicate evidence row ${id}/${name}`);
  assert.ok(['passed', 'failed', 'blocked'].includes(status));
  observed.set(`${id}/${name}`, { case_id: id, variant: name, status, reason,
    guest_requests: counters?.guest_requests ?? null,
    protected_operations: counters?.protected_operations ?? null,
    evidence, observation });
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
        selected_dispatch: executed.at(-1).request_trace });
  } catch (error) { row(caseId(1), 'saved-capture-versus-opt-in-generic-root', 'failed', String(error)); }
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
      row(id, variant.name, item.status === 'pass' ? 'passed' : 'blocked',
        item.limitation ?? (item.status === 'pass' ? 'exact CLI variant observed'
          : `source protocol did not reach canonical attempt: ${item.status}`),
        [receiptFile(raw), receiptFile(path.join(dir, `${prefix}-summary.json`))],
        item.status === 'pass' ? { guest_requests: item.guest_requests ?? item.negative_guest_requests,
          protected_operations: item.protected_operations ?? item.negative_protected_operations } : null,
        item);
    } catch (error) { row(id, variant.name, 'failed', String(error),
      fs.existsSync(raw) ? [receiptFile(raw)] : []); }
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
        row(id, variant.name, 'passed', 'real selected CLI CAS/policy interleaving',
          [summaryFile, raw, requests, stderr].map(receiptFile), counters, item);
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
      row(caseId(8), name, name === 'retention' ? 'passed' : 'blocked',
        name === 'retention' ? 'real retained-owner last-pin quota and retirement observed'
          : 'real replay observation is partial: selected named static definition lacks canonical captured I64:5 Dtrace and scripted ok-A/ok-B',
        [raw, requests, ...(fs.existsSync(stderr) ? [stderr] : [])].map(receiptFile),
        name === 'retention' ? { guest_requests: 0, protected_operations: 0 } : null, item);
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
    row(caseId(8), 'bounded-trace-retention-control', 'passed',
      'bounded trace refuses rather than truncating prior committed trace',
      [capacity, capacityRequests].map(receiptFile),
      { guest_requests: 0, protected_operations: 0 }, summary.boundedTrace);
  } catch (error) {
    row(caseId(8), 'bounded-trace-retention-control', 'failed', String(error),
      [capacity, capacityRequests].filter(fs.existsSync).map(receiptFile),
      summary?.failures.find(item => item.name === 'bounded-trace-retention') ?? null);
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
        null, refusal);
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
      { saved_program: receipts[1].stack[0], old_result: receipts[3].stack });
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
    failure ? null : { guest_requests: 0, protected_operations: 0 }, witness);
  binaryStill();
}

// Unrun canonical obligations are never inferred from a different refusal.
for (const [id, names, reason] of [
  [caseId(2), ['base-parent-child', 'transitive-A-B-C'],
    'no real held nested A/B/C selected CLI root establishes one pinned map across publication'],
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
try { frozen(source); } catch (error) { issues.push(`source freeze: ${error}`); }
if (sha(fs.readFileSync(binary)) !== afterBinary) issues.push('selected binary changed after execution');
if (sha(fs.readFileSync(path.join(root, fixtureFile))) !== sha(fixtureBytes))
  issues.push('canonical fixture bytes changed during gate');
const rows = [...observed.values()];
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
  external_evidence_sha256: evidenceFiles, integrity_issues: issues,
  limitations: [
    'All nine canonical cases remain absent/not-run/open/unassessed; this external diagnostic does not promote any case.',
    'LSLOT-05 lacks a positive genuine selected-target proof bound to authenticated capture-sensitive P/D identities.',
    'LSLOT-08 replay lacks the canonical dynamically captured I64:5 Dtrace and scripted ok-A/ok-B responses; exact replay never grants real effects.',
    'Several admission/static negative variants and the exact typed LSLOT-09 legacy caller are profile-limited; LSLOT-02 has no complete selected nested-root session.',
  ],
};
save('receipt.json', JSON.stringify(acceptance, null, 2) + '\n');
console.log(JSON.stringify({ result, receipt: path.join(output, 'receipt.json'),
  source_revision: sourceRevision, ...summary, integrity_issues: issues.length }));
process.exitCode = 1;
