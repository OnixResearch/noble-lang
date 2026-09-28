#!/usr/bin/env node
// Reruns the unchanged M8 runtime gate against a newly built current-source
// CLI and peer. An EXTERNAL mirror provides only the four archived normative
// M8 contract files at the gate's former active-change path; this is a
// historical contract fixture alias, never a current-source revision claim.
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const root = fs.realpathSync(path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..'));
assert.equal(process.argv.length, 4,
  'usage: SELECTED_NODE verification/declared-modules-v1/m8-regression.mjs FRESH_M7_BUILD NEW_EXTERNAL_DIRECTORY');
const m7Directory = path.resolve(process.argv[2]);
const out = path.resolve(process.argv[3]);
assert.ok(out !== root && !out.startsWith(`${root}${path.sep}`), 'external output required');
assert.ok(!fs.existsSync(out), 'fresh M8 regression output required');
assert.equal(fs.realpathSync(path.dirname(out)), path.dirname(out), 'output parent cannot be a symlink');
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const hash = file => sha(fs.readFileSync(file));
const selection = JSON.parse(fs.readFileSync(path.join(root, 'policy/tool-selection.json')));
const pins = JSON.parse(fs.readFileSync(path.join(root, 'verification/m6/pins.json')));
const node = fs.realpathSync(path.join(selection.tool_paths.node.output, 'bin/node'));
assert.equal(fs.realpathSync(process.execPath), node, 'selected Node required');
assert.deepEqual(process.execArgv, []);
const m7File = path.join(m7Directory, 'build.json');
const m7 = JSON.parse(fs.readFileSync(m7File));
assert.equal(m7.schema, 'noble-m7-build/v1');
assert.equal(m7.result, 'built');
assert.deepEqual(m7.integrity_failures, []);
const base = JSON.parse(fs.readFileSync(path.join(m7Directory, 'base/build.json')));
assert.equal(base.schema, 'noble-m6-build/v1');
assert.equal(base.result, 'built');
const cli = m7.binaries.cli.path;
assert.equal(hash(cli), m7.binaries.cli.sha256, 'current-source CLI binary changed');
const workspace = path.join(m7Directory, 'base/workspace');
fs.mkdirSync(out);
for (const name of ['commands', 'mirror', 'home']) fs.mkdirSync(path.join(out, name));
const mirror = path.join(out, 'mirror');
const receipt = { schema: 'noble-declared-m8-fresh-regression/v1', result: 'running',
  current_source_root: root, m7_build: { path: m7File, sha256: hash(m7File),
    cli_sha256: m7.binaries.cli.sha256, source_revision: m7.source_revision },
  source_files: {}, archived_contract_alias: {}, fresh_test_inputs: {}, tools: {}, commands: [],
  peer: null, m8_gate: null, failures: [], integrity_failures: [],
  non_claims: ['The archived M8 contract is an explicitly mirrored historical fixture, not unchanged current M8 source revision.',
    'Historical M8 PASS is never used as fresh execution evidence.',
    'This finite M8 regression establishes no universal parser/monitor/host/engine refinement.'] };
const save = () => fs.writeFileSync(path.join(out, 'regression.json'), JSON.stringify(receipt, null, 2) + '\n');
function copyFile(name) {
  const original = path.join(root, name), destination = path.join(mirror, name);
  assert.ok(fs.lstatSync(original).isFile(), `not a plain current source: ${name}`);
  fs.mkdirSync(path.dirname(destination), { recursive: true });
  fs.writeFileSync(destination, fs.readFileSync(original), { flag: 'wx', mode: 0o400 });
  assert.equal(hash(destination), hash(original), `mirrored source differs: ${name}`);
  receipt.source_files[name] = { sha256: hash(original), bytes: fs.statSync(original).size };
}
function copyTree(name) {
  for (const item of fs.readdirSync(path.join(root, name), { withFileTypes: true })) {
    if (['target', '.git', '.lake', 'node_modules'].includes(item.name)) continue;
    const file = `${name}/${item.name}`;
    if (item.isDirectory()) copyTree(file);
    else if (item.isFile()) copyFile(file);
    else throw Error(`unreviewed source entry: ${file}`);
  }
}
function execute(label, executable, argv, cwd, environment, timeout) {
  const tool = fs.realpathSync(executable);
  const digest = hash(tool);
  if (!receipt.tools[tool]) receipt.tools[tool] = { sha256: digest };
  assert.equal(receipt.tools[tool].sha256, digest, `${label}: tool changed`);
  const stem = `commands/${String(receipt.commands.length + 1).padStart(3, '0')}-${label}`;
  const stdout = path.join(out, `${stem}.stdout`), stderr = path.join(out, `${stem}.stderr`);
  const outFd = fs.openSync(stdout, 'wx', 0o600), errFd = fs.openSync(stderr, 'wx', 0o600);
  const row = { label, executable: tool, executable_sha256: digest, argv, cwd, environment,
    timeout_ms: timeout, stdout: path.relative(out, stdout), stderr: path.relative(out, stderr) };
  receipt.commands.push(row); save();
  let result;
  try { result = spawnSync(tool, argv, { cwd, env: environment,
    stdio: ['ignore', outFd, errFd], timeout, killSignal: 'SIGKILL' }); }
  finally { fs.closeSync(outFd); fs.closeSync(errFd); }
  Object.assign(row, { status: result?.status ?? null, signal: result?.signal ?? null,
    error: result?.error ? String(result.error) : null,
    stdout_sha256: hash(stdout), stderr_sha256: hash(stderr) });
  for (const file of [stdout, stderr]) fs.chmodSync(file, 0o400);
  row.passed = row.status === 0 && row.signal === null && row.error === null && hash(tool) === digest;
  if (!row.passed) receipt.failures.push({ label, status: row.status, stderr: row.stderr, error: row.error });
  save();
  assert.ok(row.passed, `${label}: command failed (${row.stderr})`);
  return row;
}
try {
  for (const tree of ['crates', 'policy', 'verification/m8/peer/src',
    '.cairn/specs/safety', '.cairn/specs/wit-wasi']) copyTree(tree);
  for (const file of ['Cargo.toml', 'Cargo.lock', 'flake.nix', 'flake.lock',
    'rust-toolchain.toml', '.pre-commit-config.yaml', 'verification/m6/pins.json',
    'verification/m8/peer/Cargo.toml',
    'verification/m8/gate.mjs', 'verification/m8/build.mjs',
    'verification/m8/assurance.mjs', 'verification/m8/source-projection.mjs',
    'verification/m8/final-documents.mjs', 'verification/m4/extraction-lock.json',
    'specs/conformance/safety-cases.json', 'specs/conformance/wit-wasi-cases.json']) copyFile(file);
  const archived = '.cairn/archive/2026-09-26-m8-choreography-projection';
  const active = '.cairn/changes/m8-choreography-projection';
  assert.ok(!fs.existsSync(path.join(root, active)), 'M8 historical change is no longer active');
  const historicalFile = 'verification/m8/assurance.json';
  const historical = JSON.parse(fs.readFileSync(path.join(root, historicalFile)));
  assert.equal(historical.schema, 'noble-m8-assurance/v1');
  assert.equal(historical.phase, 'pre-promotion');
  assert.equal(historical.result, 'passed');
  assert.equal(historical.commands.length, 17);
  receipt.historical_acceptance = { path: historicalFile,
    sha256: hash(path.join(root, historicalFile)) };
  const historicalGate = 'verification/m8/acceptance.json';
  assert.equal(hash(path.join(root, historicalGate)), historical.gates['m8-acceptance'].sha256,
    'archived M8 acceptance changed since its passed pre-promotion assurance');
  const acceptedHistorical = JSON.parse(fs.readFileSync(path.join(root, historicalGate)));
  assert.equal(acceptedHistorical.result, 'passed');
  for (const name of ['design.md', 'proposal.md', 'specs/safety/spec.md', 'specs/wit-wasi/spec.md']) {
    const source = `${archived}/${name}`, target = `${active}/${name}`;
    const expected = historical.source_files[target]?.sha256;
    assert.ok(expected, `original M8 source identity unavailable: ${target}`);
    assert.equal(hash(path.join(root, source)), expected,
      `archived normative bytes differ from originally accepted M8 source: ${name}`);
    const destination = path.join(mirror, target);
    fs.mkdirSync(path.dirname(destination), { recursive: true });
    fs.writeFileSync(destination, fs.readFileSync(path.join(root, source)), { flag: 'wx', mode: 0o400 });
    receipt.archived_contract_alias[target] = { archive: source, sha256: hash(destination),
      original_accepted_sha256: expected, classification: 'historical-contract-fixture-alias' };
  }
  // The M8 gate insists on unexecuted cases. Only its external test mirror
  // resets execution metadata; the current canonical cases and archived
  // contract remain byte-for-byte unchanged and retain their historical PASS.
  for (const [name, ids] of [
    ['specs/conformance/safety-cases.json', ['S-CASE-18', 'S-CASE-19']],
    ['specs/conformance/wit-wasi-cases.json', ['WI-19', 'WI-20']],
  ]) {
    const original = fs.readFileSync(path.join(root, name), 'utf8');
    const document = JSON.parse(original);
    const historicalInput = acceptedHistorical.inputs[name];
    assert.equal(historicalInput.sha256, historical.source_files[name].sha256,
      `${name}: archived pre-promotion input identity changed`);
    const selected = ids.map(id => {
      const item = document.cases.find(row => row.id === id);
      assert.ok(item, `${name}: selected historical case missing: ${id}`);
      assert.deepEqual(item.state, { implementation: 'implemented', execution: 'passed',
        proof: 'open', trust: 'explicit' }, `${id}: current accepted case state changed`);
      assert.deepEqual({ id, input: item.input, expected: item.expected },
        historicalInput.workload.find(row => row.id === id),
        `${id}: historical normative input or expectation changed`);
      return { id, original_state: item.state, original_case_sha256: sha(JSON.stringify(item)) };
    });
    let fresh = original;
    const passedState = '"state":{"implementation":"implemented","execution":"passed","proof":"open","trust":"explicit"}';
    const unexecutedState = '"state":{"implementation":"implemented","execution":"not-run","proof":"open","trust":"explicit"}';
    for (const id of ids) {
      const marker = `"id":"${id}"`;
      assert.equal(fresh.split(marker).length, 2, `${id}: case marker is not unique`);
      const start = fresh.indexOf(marker);
      const at = fresh.indexOf(passedState, start);
      assert.ok(at > start, `${id}: selected passed state missing`);
      fresh = fresh.slice(0, at) + unexecutedState + fresh.slice(at + passedState.length);
    }
    const testDocument = JSON.parse(fresh);
    const expected = structuredClone(document);
    for (const item of expected.cases.filter(row => ids.includes(row.id)))
      item.state.execution = 'not-run';
    assert.deepEqual(testDocument, expected, `${name}: more than selected execution metadata changed`);
    const destination = path.join(mirror, name);
    fs.chmodSync(destination, 0o600);
    fs.writeFileSync(destination, fresh, { flag: 'w', mode: 0o400 });
    fs.chmodSync(destination, 0o400);
    receipt.fresh_test_inputs[name] = {
      classification: 'isolated-external-fresh-test-input-not-current-source',
      original_current_source_sha256: receipt.source_files[name].sha256,
      archived_pre_promotion_sha256: historicalInput.sha256,
      archived_acceptance_sha256: historical.gates['m8-acceptance'].sha256,
      mirror_sha256: hash(destination), selected,
      transformation: 'only the selected accepted cases execution state passed -> not-run',
    };
    assert.equal(hash(path.join(root, name)), receipt.source_files[name].sha256,
      `${name}: canonical accepted case file changed`);
  }
  const lock = fs.readFileSync(path.join(workspace, 'verification/m7/peer/Cargo.lock'), 'utf8');
  const marker = 'name = "noble-m7-peer"';
  assert.equal(lock.split(marker).length, 2, 'unique inherited M7 peer lock package required');
  const derived = lock.replace(marker, 'name = "noble-m8-peer"');
  const peerDirectory = path.join(workspace, 'verification/m8/peer');
  fs.mkdirSync(path.join(peerDirectory, 'src'), { recursive: true });
  for (const name of Object.keys(receipt.source_files).filter(file => file.startsWith('verification/m8/peer/'))) {
    const destination = path.join(workspace, name);
    fs.mkdirSync(path.dirname(destination), { recursive: true });
    fs.writeFileSync(destination, fs.readFileSync(path.join(root, name)), { flag: 'wx', mode: 0o400 });
    assert.equal(hash(destination), receipt.source_files[name].sha256,
      `M8 peer source differs from current production snapshot: ${name}`);
  }
  fs.writeFileSync(path.join(peerDirectory, 'Cargo.lock'), derived, { flag: 'wx', mode: 0o400 });
  receipt.peer_lock = { source: 'verification/m7/peer/Cargo.lock', source_sha256: sha(lock),
    transformation: 'replace unique local package noble-m7-peer with noble-m8-peer',
    derived_sha256: sha(derived) };
  assert.ok(fs.readFileSync(path.join(peerDirectory, 'Cargo.toml'), 'utf8')
    .includes(`path = "${pins.wasmtime_source}/crates/wasmtime"`),
  'M8 peer must retain the policy-selected Wasmtime source');
  const cargo = path.join(selection.tool_paths.quality_rust.output, 'bin/cargo');
  assert.equal(hash(cargo), base.tools.cargo.sha256, 'M8 peer Cargo differs from current-source build');
  const environment = { ...base.environment, CARGO_TARGET_DIR: path.join(out, 'peer-target') };
  const vendor = ['--config', 'source.crates-io.replace-with="m6-vendor"', '--config',
    `source.m6-vendor.directory="${pins.vendor}/source-registry-0"`];
  const built = execute('m8-current-source-peer-build', cargo,
    ['build', '--manifest-path', 'verification/m8/peer/Cargo.toml', '--locked', '--offline',
      '--message-format=json', ...vendor], workspace, environment, 3_900_000);
  execute('m8-current-source-peer-clippy', cargo,
    ['clippy', '--manifest-path', 'verification/m8/peer/Cargo.toml', '--all-targets',
      '--locked', '--offline', ...vendor, '--', '-D', 'warnings'], workspace, environment, 3_900_000);
  const rows = fs.readFileSync(path.join(out, built.stdout), 'utf8').split('\n')
    .filter(line => line.startsWith('{')).map(line => JSON.parse(line));
  const peers = [...new Set(rows.filter(row => row.reason === 'compiler-artifact'
      && row.target?.name === 'noble-m8-peer' && row.target.kind?.includes('bin')
      && !row.profile?.test).map(row => row.executable).filter(Boolean))];
  assert.equal(peers.length, 1, 'exactly one freshly compiled M8 peer executable required');
  const peer = peers[0], peerSha = hash(peer);
  receipt.peer = { executable: peer, sha256: peerSha, source: 'fresh-current-source-build',
    cargo_command: built.label };
  const wasmTools = path.join(selection.tool_paths.wasm_tools.output, 'bin/wasm-tools');
  const gated = execute('m8-real-compiled-regression', node,
    [path.join(mirror, 'verification/m8/gate.mjs'), cli, peer, wasmTools, path.join(out, 'acceptance')],
    mirror, { PATH: '/run/current-system/sw/bin', HOME: path.join(out, 'home'),
      LANG: 'C.UTF-8', LC_ALL: 'C.UTF-8' }, 1_200_000);
  const file = path.join(out, 'acceptance/report.json');
  const accepted = JSON.parse(fs.readFileSync(file));
  assert.equal(accepted.schema, 'noble-m8-acceptance/v1');
  assert.equal(accepted.result, 'passed', 'fresh M8 runtime gate did not pass');
  assert.equal(accepted.summary?.required_cases, 4);
  assert.equal(accepted.summary?.passed_cases, 4);
  assert.equal(accepted.summary?.variants, 38);
  assert.equal(accepted.summary?.strict_json_edges, 10);
  assert.equal(accepted.summary?.one_round_reservations, 3);
  assert.deepEqual(accepted.integrity_failures, []);
  assert.equal(accepted.executables.cli.sha256, m7.binaries.cli.sha256);
  assert.equal(accepted.executables.peer.sha256, peerSha);
  for (const [name, item] of Object.entries(accepted.sources)) {
    const expected = receipt.fresh_test_inputs[name]?.mirror_sha256
      ?? receipt.source_files[name]?.sha256
      ?? receipt.archived_contract_alias[name]?.sha256;
    assert.equal(item.sha256, expected, `M8 gate used unreviewed source: ${name}`);
  }
  for (const [name, item] of Object.entries(receipt.archived_contract_alias))
    assert.equal(accepted.sources[name]?.sha256, item.sha256,
      `M8 gate omitted archived normative fixture: ${name}`);
  receipt.m8_gate = { path: path.relative(out, file), sha256: hash(file),
    schema: accepted.schema, fresh_command: gated.label, current_cli_sha256: m7.binaries.cli.sha256,
    current_peer_sha256: peerSha, fixture_alias_count: Object.keys(receipt.archived_contract_alias).length };
  for (const [name, item] of Object.entries(receipt.source_files)) {
    assert.equal(hash(path.join(root, name)), item.sha256, `live current source changed: ${name}`);
    assert.equal(hash(path.join(mirror, name)),
      receipt.fresh_test_inputs[name]?.mirror_sha256 ?? item.sha256,
      `mirror source changed: ${name}`);
  }
  for (const [name, item] of Object.entries(receipt.archived_contract_alias)) {
    assert.equal(hash(path.join(root, item.archive)), item.sha256, `archive changed: ${name}`);
    assert.equal(hash(path.join(mirror, name)), item.sha256, `fixture alias changed: ${name}`);
  }
  assert.equal(hash(path.join(root, receipt.historical_acceptance.path)),
    receipt.historical_acceptance.sha256, 'originally accepted M8 source receipt changed');
  assert.equal(hash(path.join(peerDirectory, 'Cargo.lock')), receipt.peer_lock.derived_sha256,
    'derived locked M8 peer dependency graph changed');
  assert.equal(hash(m7File), receipt.m7_build.sha256);
  assert.equal(hash(cli), m7.binaries.cli.sha256);
  assert.equal(hash(peer), peerSha);
  for (const [name, item] of Object.entries(receipt.tools))
    assert.equal(hash(name), item.sha256, `tool changed: ${name}`);
  for (const command of receipt.commands) for (const field of ['stdout', 'stderr'])
    assert.equal(hash(path.join(out, command[field])), command[`${field}_sha256`],
      `${command.label}: transcript changed`);
  assert.equal(hash(file), receipt.m8_gate.sha256);
} catch (error) { receipt.integrity_failures.push(String(error.stack ?? error)); }
receipt.result = receipt.failures.length === 0 && receipt.integrity_failures.length === 0
  && receipt.commands.length === 3 && receipt.m8_gate ? 'passed' : 'failed';
save();
console.log(JSON.stringify({ schema: receipt.schema, result: receipt.result,
  report: path.join(out, 'regression.json'), commands: receipt.commands.length,
  m8_gate: receipt.m8_gate, failures: receipt.failures,
  integrity_failures: receipt.integrity_failures }));
process.exitCode = receipt.result === 'passed' ? 0 : 1;
