#!/usr/bin/env node
// Fresh post-feature source-bound assurance. The old M8 active change is
// archived; run current-source M4–M7 gates and an explicitly labelled fresh
// M8 regression with only its archived normative contract mirrored externally.
// Historical M8 receipts and hardcoded inventory counts are never promoted.
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { gitSourceProjection } from '../m8/source-projection.mjs';

const root = fs.realpathSync(path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..'));
const [mode, destination, ...extra] = process.argv.slice(2);
assert.ok(['plan', 'run'].includes(mode) && destination && !extra.length,
  'usage: SELECTED_NODE verification/declared-modules-v1/assurance.mjs {plan|run} NEW_EXTERNAL_DIRECTORY');
const out = path.resolve(destination);
assert.ok(out !== root && !out.startsWith(`${root}${path.sep}`), 'external artifact directory required');
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const hash = file => sha(fs.readFileSync(file));
const selection = JSON.parse(fs.readFileSync(path.join(root, 'policy/tool-selection.json')));
const pins = JSON.parse(fs.readFileSync(path.join(root, 'verification/m6/pins.json')));
const node = fs.realpathSync(path.join(selection.tool_paths.node.output, 'bin/node'));
assert.equal(fs.realpathSync(process.execPath), node, 'selected Node required');
assert.deepEqual(process.execArgv, [], 'unreviewed Node flags');
const nix = pins.nix ?? '/run/current-system/sw/bin/nix';
const bun = process.env.NOBLE_M8_BUN ?? '/nix/store/qkydg77xf2s395md9fq0a6djjpq7fzh4-bun-1.4.2/bin/bun';
const cargo = path.join(selection.tool_paths.quality_rust.output, 'bin/cargo');
const rust = path.join(selection.tool_paths.quality_rust.output, 'bin');
const wasmTools = path.join(selection.tool_paths.wasm_tools.output, 'bin/wasm-tools');
const wasmOpt = path.join(selection.tool_paths.binaryen.output, 'bin/wasm-opt');
const cairn = path.join(selection.tool_paths.cairn.output, 'bin/cairn');
const cairnPolicy = '/nix/store/jqz083ahq10fdrakci9xzvf7bl4nh075-source/cairn-policy/generated/cairn-policy.json';
const build = path.join(out, 'build');
const base = path.join(build, 'base/build.json');
const cli = path.join(build, 'base/executables/cli');
const m7Peer = path.join(build, 'executables/m7-peer');
const m5Peer = path.join(build, 'base/executables/m5_peer');
const m6Peer = path.join(build, 'base/executables/peer');
const taskTests = path.join(build, 'base/executables/task_tests');
const resourceTests = path.join(build, 'base/executables/resource_tests');
const authorityTests = path.join(build, 'base/executables/authority_tests');
const mc2Tests = path.join(build, 'executables/mc2-core-control-tests');
const workspace = path.join(build, 'base/workspace');
const privateNixConfig = '/var/tmp/noble-m8-refrozen-20260924/private-nix-no-gc-config-20260926';
const boundSystemNixConfig = '/etc/nix/nix.conf';
const privateNixConfigSha = '226289cf0b807ab6e17058b1a6f3410d9560643d6e2e27edb9b9a5c1a1215013';
const nixArgs = ['--option', 'builders', '', '--option', 'substituters',
  'https://cache.nixos.org https://nix-community.cachix.org',
  '--option', 'min-free', '0', '--option', 'max-free', '0',
  '--option', 'sandbox', 'true', '--option', 'sandbox-fallback', 'false',
  '--option', 'require-sigs', 'true',
  '--extra-experimental-features', 'nix-command flakes'];
const common = { PATH: '/run/current-system/sw/bin', HOME: path.join(out, 'home'),
  TMPDIR: path.join(out, 'tmp'), LANG: 'C.UTF-8', LC_ALL: 'C.UTF-8',
  XDG_RUNTIME_DIR: process.env.XDG_RUNTIME_DIR,
  NIX_USER_CONF_FILES: privateNixConfig,
  // User namespaces map root-owned NixOS SSH configuration to nobody. This
  // workstation's existing framework key fetches pinned private Nix sources;
  // keep known_hosts and strict host-key checks, not product authentication.
  GIT_SSH_COMMAND: '/run/current-system/sw/bin/ssh -F /dev/null -i /home/brittonr/.ssh/framework -o IdentitiesOnly=yes' };
const cargoEnv = { ...common, PATH: `${rust}:${pins.linker_bin}:/run/current-system/sw/bin`,
  RUSTC_WORKSPACE_WRAPPER: '', CARGO_BUILD_RUSTC_WRAPPER: '', RUSTFLAGS: '',
  CARGO_ENCODED_RUSTFLAGS: '', CARGO_INCREMENTAL: '0',
  CARGO_HOME: path.join(out, 'cargo-home'), CARGO_TARGET_DIR: path.join(out, 'quality-target') };
const m4Env = { ...cargoEnv, CARGO_HOME: path.join(build, 'base/cargo-home'),
  CARGO_TARGET_DIR: path.join(build, 'base/target') };
const proofEnv = { ...common, NOBLE_LEAN: path.join(selection.tool_paths.lean.output, 'bin/lean'),
  NOBLE_BWRAP: '/nix/store/lqndphylsxqwbwm804n473pb4sqb98sh-bubblewrap-0.11.2/bin/bwrap',
  NOBLE_PRLIMIT: '/nix/store/mqvbf0flamqaq9c3ihb496aahag897n0-util-linux-2.42.3-bin/bin/prlimit',
  NOBLE_SYSTEMD_RUN: '/nix/store/baxgs06659w4i6ggsfr56srjrb52v1h8-systemd-261.2/bin/systemd-run' };
const extractionGitLink = '/run/current-system/sw/bin/git';
const extractionGit = fs.realpathSync(extractionGitLink);
const vendor = ['--config', 'source.crates-io.replace-with="m6-vendor"', '--config',
  `source.m6-vendor.directory="${pins.vendor}/source-registry-0"`];
const task = (label, executable, argv, environment = common, timeout = 3_600_000, cwd = root) =>
  ({ label, executable, argv, environment, timeout, cwd });
const gate = name => path.join(root, `verification/${name}/gate.mjs`);
const tasks = [
  task('current-nix-source-prefetch', nix, [...nixArgs, 'flake', 'prefetch', '--json', '.'], common, 600_000),
  task('fresh-compiler-inventory', nix, [...nixArgs, 'run', '.#source-inventory', '--',
    'collect', '--root', root, '--selection', path.join(root, 'policy/tool-selection.json'),
    '--artifact-dir', path.join(out, 'inventory')], cargoEnv),
  task('reviewed-compiler-coverage', nix, [...nixArgs, 'run', '.#source-inventory', '--',
    'check', '--root', root, '--selection', path.join(root, 'policy/tool-selection.json'),
    '--inventory', path.join(out, 'inventory/inventory.json'),
    '--policy', path.join(root, 'policy/source-inventory.json'),
    '--artifact-dir', path.join(out, 'coverage')], common, 600_000),
  task('fresh-current-workspace-m7-build', node,
    [path.join(root, 'verification/m7/build.mjs'), build], common, 3_900_000),
  task('strict-whole-crate-extraction-check', node,
    [path.join(root, 'verification/m4/implementation.mjs'), 'check', path.join(out, 'extraction')],
    common, 7_200_000),
  task('fresh-current-production-m8-regression', node,
    [path.join(root, 'verification/declared-modules-v1/m8-regression.mjs'),
      build, path.join(out, 'm8-regression')], common, 3_900_000),
  task('m7-regression', node, [gate('m7'), cli, m7Peer, wasmTools, path.join(out, 'm7')],
    common, 1_200_000),
  task('mc1-regression', bun, [gate('mc1'), cli, path.join(out, 'mc1.json')], proofEnv),
  task('mc2-regression', bun, [gate('mc2'), cli, path.join(out, 'mc2.json')],
    { ...proofEnv, NOBLE_MC2_CORE_TEST_BINARY: mc2Tests }, 7_200_000),
  task('m3-four-configuration-regression', node,
    [path.join(root, 'tools/m3-wasm.mjs'), '--noble', cli, '--out', path.join(out, 'm3')],
    { ...common, NOBLE_M3_NODE: node, NOBLE_M3_WASM_TOOLS: wasmTools, NOBLE_M3_WASM_OPT: wasmOpt }),
  task('m4-runtime-regression', node, [gate('m4'), cli, path.join(out, 'm4')], m4Env, 1_200_000),
  task('m5-component-regression', node,
    [gate('m5'), cli, m5Peer, resourceTests, authorityTests, path.join(out, 'm5'),
      '--opacity-report', path.join(build, 'm5-opacity/opacity.json')]),
  task('m6-component-regression', node,
    [gate('m6'), cli, m6Peer, taskTests, path.join(out, 'm6'), '--build-report', base]),
  task('canonical-documents', bun, [path.join(root, 'tools/check-specs.mjs'), '--self-test', '--report'],
    common, 300_000),
  task('cairn-change-validation', cairn, ['validate', '--root', root, '--policy', cairnPolicy],
    common, 300_000),
  task('rust-all-workspace-tests', cargo,
    ['test', '--workspace', '--all-targets', '--all-features', '--locked', '--offline', ...vendor], cargoEnv),
  task('rust-all-workspace-clippy', cargo,
    ['clippy', '--workspace', '--all-targets', '--all-features', '--locked', '--offline',
      ...vendor, '--', '-D', 'warnings'], cargoEnv),
  task('published-octet-precommit', nix,
    [...nixArgs, 'develop', '-c', 'pre-commit', 'run', 'octet-deny-all', '--all-files'], common),
  task('complete-nix-flake-check', nix, [...nixArgs, '--option', 'max-jobs', '1',
    'flake', 'check', '--keep-going', '-L'],
    common, 7_200_000),
];
const roots = [['noble_kernel.dataspace.admit_shared_memory', 31],
  ['noble_kernel.dataspace.decide_publication', 32], ['noble_kernel.dataspace.permits', 33]];
const theorems = ['publication_refines', 'permission_refines', 'shared_memory_refines',
  'shared_memory_refused', 'publish_right_required', 'observe_right_required',
  'duplicate_publication_unchanged', 'changed_publication_replaces', 'first_publication_adds']
  .map(name => `M7Syndicate.${name}`);
const exactRoots = rows => Array.isArray(rows)
  && JSON.stringify(rows.map(row => [row.declaration, row.def_id])) === JSON.stringify(roots);
function strictAudit(m7) {
  const audit = m7?.syndicate_refinement?.audit;
  return exactRoots(m7?.coverage?.implementation_roots)
    && audit?.schema === 'noble-m7-syndicate-audit/v1' && audit.result === 'passed'
    && exactRoots(audit.implementation_roots)
    && JSON.stringify(audit.strict_roots?.map(row => row.declaration)) === JSON.stringify(theorems)
    && JSON.stringify(audit.packet?.strict_roots?.map(row => row.declaration)) === JSON.stringify(theorems);
}
const architecture = JSON.parse(fs.readFileSync(path.join(root, 'policy/architecture.json')));
const pending = [];
if (!fs.existsSync(privateNixConfig) || hash(privateNixConfig) !== privateNixConfigSha)
  pending.push('reviewed private signed/sandbox Nix configuration differs from accepted M8 bytes');
if (!fs.existsSync(boundSystemNixConfig) || hash(boundSystemNixConfig) !== privateNixConfigSha)
  pending.push('private user-service read-only /etc/nix/nix.conf bind is missing or differs');
if (architecture.collection_limits.max_units < 34 || architecture.collection_limits.max_shards < 34)
  pending.push('reviewed compiler collector limits must cover 32 independently observed units plus two shell-library controls');
if (!common.XDG_RUNTIME_DIR || !fs.existsSync(path.join(common.XDG_RUNTIME_DIR, 'bus'))
  || !fs.statSync(path.join(common.XDG_RUNTIME_DIR, 'bus')).isSocket())
  pending.push('selected Nix/proof tasks need an active user XDG_RUNTIME_DIR bus');
for (const [name, file] of Object.entries(proofEnv).filter(([key]) => key.startsWith('NOBLE_')))
  if (!fs.existsSync(file)) pending.push(`selected proof sandbox tool missing: ${name}=${file}`);
if (!fs.existsSync(cairnPolicy) || hash(cairnPolicy) !==
  '62b9ade2bc8af10b2c68776dba8abb374a6c7331aae7ad8bb41ca938ef100d27')
  pending.push('selected Cairn source policy hash changed');
if (!fs.existsSync(path.join(root, '.cairn/changes/2026-09-26-declared-modules-v1/design.md')))
  pending.push('missing live declared-module source contract');
if (fs.existsSync(path.join(root, '.cairn/changes/m8-choreography-projection')))
  pending.push('M8 historical contract must be archived before mirroring its normative bytes');
const historicalM8 = JSON.parse(fs.readFileSync(path.join(root, 'verification/m8/assurance.json')));
assert.equal(historicalM8.schema, 'noble-m8-assurance/v1');
for (const name of ['design.md', 'proposal.md', 'specs/safety/spec.md', 'specs/wit-wasi/spec.md']) {
  const archived = path.join(root, '.cairn/archive/2026-09-26-m8-choreography-projection', name);
  const original = `.cairn/changes/m8-choreography-projection/${name}`;
  if (!fs.existsSync(archived) || hash(archived) !== historicalM8.source_files[original]?.sha256)
    pending.push(`archived historical M8 normative fixture differs from original accepted source: ${name}`);
}
for (const [name, expected] of Object.entries(selection.file_sha256))
  if (!fs.existsSync(path.join(root, name)) || hash(path.join(root, name)) !== expected)
    pending.push(`selected source/tool policy SHA differs: ${name}`);
let extractionLock;
try { extractionLock = JSON.parse(fs.readFileSync(path.join(root, 'verification/m4/extraction-lock.json'))); }
catch (error) { pending.push(`missing readable reviewed extraction lock: ${error}`); }
if (extractionLock) {
  if (extractionLock.schema !== 'm4-extraction-lock/v1' || !strictAudit(extractionLock.m7))
    pending.push('reviewed M4 lock lacks three exact retained M7 compiler roots and nine strict theorem roots');
  if (extractionLock.tools?.git?.path !== extractionGitLink
    || extractionLock.tools.git.realpath !== extractionGit
    || extractionLock.tools.git.sha256 !== hash(extractionGit))
    pending.push('reviewed M4 lock Git executable path/SHA differs from selected check environment');
  for (const name of ['policy/source-inventory.json', 'policy/tool-selection.json',
    'proofs/m7/M7Syndicate.lean', 'proofs/m7/M7Audit.lean', 'verification/m7/extraction.mjs'])
    if (extractionLock.source_files?.[name] !== hash(path.join(root, name)))
      pending.push(`reviewed whole-crate lock lacks exact current source: ${name}`);
}
const skipped = new Set(['.git', '.lake', 'target', 'node_modules']);
const sources = new Set(['Cargo.toml', 'Cargo.lock', 'flake.nix', 'flake.lock',
  'rust-toolchain.toml', '.pre-commit-config.yaml',
  ...Object.keys(extractionLock?.source_files ?? {})]);
function sourceTree(relative) {
  for (const item of fs.readdirSync(path.join(root, relative), { withFileTypes: true })) {
    if (skipped.has(item.name)) continue;
    const name = `${relative}/${item.name}`;
    if (item.isDirectory()) sourceTree(name);
    else if (item.isFile()) sources.add(name);
    else if (relative.startsWith('proofs') && item.isSymbolicLink()) continue;
    else throw Error(`unreviewed source entry: ${name}`);
  }
}
for (const directory of ['crates', 'policy', 'nix', 'proofs', 'specs', 'tools',
  'verification/m4', 'verification/m5', 'verification/m6', 'verification/m7', 'verification/m8',
  'verification/mc1', 'verification/mc2', 'verification/m3-wasm',
  'verification/declared-modules-v1', '.cairn/specs',
  '.cairn/archive/2026-09-26-m8-choreography-projection',
  '.cairn/changes/2026-09-26-declared-modules-v1']) sourceTree(directory);
if (extractionLock) for (const name of [...sources].filter(item => item.startsWith('crates/')))
  if (extractionLock.source_files?.[name] !== hash(path.join(root, name)))
    pending.push(`reviewed whole-crate lock missing/changed ${name}`);
function snapshot() {
  return Object.fromEntries([...sources].sort().map(name => {
    const file = path.join(root, name);
    assert.ok(fs.lstatSync(file).isFile(), `not a plain source: ${name}`);
    return [name, { sha256: hash(file), bytes: fs.statSync(file).size }];
  }));
}
if (mode === 'plan') {
  console.log(JSON.stringify({ schema: 'noble-declared-modules-assurance-plan/v1',
    planned_result: pending.length ? 'blocked' : 'ready', artifact_directory: out,
    prerequisites: pending, commands: tasks.map(({ label, executable, argv, environment, cwd, timeout }) =>
      ({ label, executable, argv, environment, cwd, timeout_ms: timeout })),
    follow_on: ['Fresh pinned Cargo build/clippy of independent declared kernel peer using inherited full current-source workspace.',
      'Execute declared DX-03/08/09 compiled production and kernel gate on fresh CLI and peer.',
      'The M8 regression helper freshly builds/clippies current-source M8 peer and runs unchanged original M8 gate with four archived normative M8 files explicitly mirrored in an external fixture root.'],
    non_claims: ['Plan executes no gate or compiler collection.',
      'Historical M8 PASS and 30-unit/2087-body counts are not current-source evidence.',
      'Open nominal bodies have no asserted Lean or universal compiler-to-Wasm proof.'] }, null, 2));
  process.exit(0);
}
assert.deepEqual(pending, [], 'resolve source/lock/collector prerequisites before execution');
assert.ok(!fs.existsSync(out), 'fresh assurance output required');
assert.equal(fs.realpathSync(path.dirname(out)), path.dirname(out), 'artifact parent cannot be a symlink');
fs.mkdirSync(out);
for (const name of ['commands', 'home', 'tmp', 'cargo-home']) fs.mkdirSync(path.join(out, name));
const receipt = { schema: 'noble-declared-modules-source-assurance/v1', phase: 'pre-promotion',
  result: 'running', source_root: root, source_files: {}, git_projection: null,
  commands: [], tools: {}, gates: {}, failures: [], integrity_failures: [],
  private_nix_configuration: { path: privateNixConfig,
    bound_system_path: boundSystemNixConfig, sha256: privateNixConfigSha,
    local_builders: true, signed_substitutes_required: true, sandbox_required: true },
  proof_lock: { path: 'verification/m4/extraction-lock.json',
    sha256: hash(path.join(root, 'verification/m4/extraction-lock.json')) },
  assumptions: ['The independently reviewed M4 extraction lock and conservative source inventory were installed before this fresh run.',
    'Selected compiler/Charon/Aeneas/Lean/Octet/Rust/Node/Nix/Wasmtime tools remain trusted.',
    'The M8 fixture alias supplies only byte-verified archived normative M8 contracts; production crates, peer and tools are freshly bound.'],
  non_claims: ['Historical M8 PASS or historical M8 current-source revision',
    'Universal source-to-Wasm/host refinement or nominal Lean proof',
    'Distributed transport, durability, fairness or arbitrary async progress'] };
const save = () => fs.writeFileSync(path.join(out, 'assurance.json'), JSON.stringify(receipt, null, 2) + '\n');
function execute({ label, executable, argv, environment, timeout, cwd }) {
  if (executable === nix)
    assert.equal(hash(boundSystemNixConfig), privateNixConfigSha,
      `${label}: private signed/sandbox Nix bind changed before command`);
  const tool = fs.realpathSync(executable);
  const digest = hash(tool);
  if (!receipt.tools[tool]) receipt.tools[tool] = { sha256: digest };
  assert.equal(receipt.tools[tool].sha256, digest, `${label}: executable changed`);
  const stem = `commands/${String(receipt.commands.length + 1).padStart(3, '0')}-${label}`;
  const stdout = path.join(out, `${stem}.stdout`), stderr = path.join(out, `${stem}.stderr`);
  const outFd = fs.openSync(stdout, 'wx', 0o600), errFd = fs.openSync(stderr, 'wx', 0o600);
  const row = { label, executable: tool, executable_sha256: digest, argv, environment, cwd,
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
  row.passed = row.status === 0 && row.signal === null && row.error === null
    && hash(tool) === digest
    && (executable !== nix || hash(boundSystemNixConfig) === privateNixConfigSha);
  if (!row.passed) receipt.failures.push({ label, status: row.status, error: row.error, stderr: row.stderr });
  save();
  assert.ok(row.passed, `${label}: command failed (${row.stderr})`);
  return row;
}
function bind(label, file, schema, acceptable) {
  const value = JSON.parse(fs.readFileSync(file));
  assert.equal(value.schema, schema, `${label}: receipt schema differs`);
  assert.ok(acceptable(value), `${label}: fresh gate did not pass`);
  receipt.gates[label] = { path: path.relative(out, file), sha256: hash(file), schema,
    source_revision: value.source_revision ?? null };
  save();
  return value;
}
function accept(label, command) {
  if (label === 'current-nix-source-prefetch') {
    const fetched = JSON.parse(fs.readFileSync(path.join(out, command.stdout)));
    assert.ok(typeof fetched.storePath === 'string' && fetched.hash?.startsWith('sha256-'));
    const materialized = fs.realpathSync(fetched.storePath);
    assert.ok(fs.statSync(materialized).isDirectory());
    for (const [name, item] of Object.entries(receipt.source_files)) {
      const file = path.join(materialized, name);
      assert.ok(fs.statSync(file).isFile(), `Nix source omitted ${name}`);
      assert.equal(hash(file), item.sha256, `Nix source changed ${name}`);
    }
    receipt.gates.nix_source = { path: command.stdout, sha256: hash(path.join(out, command.stdout)),
      materialized, nar_hash: fetched.hash, checked_files: Object.keys(receipt.source_files).length };
  } else if (label === 'fresh-compiler-inventory') {
    const inventory = bind(label, path.join(out, 'inventory/inventory.json'),
      'noble-source-inventory/v1', value => value.coverage?.status === 'complete'
        && value.coverage.expected_units === 32 && value.coverage.observed_units === 32
        && ['missing_units', 'unexpected_units', 'duplicate_units'].every(field =>
          value.coverage[field]?.length === 0)
        && value.counts?.production_subjects === value.production_subjects?.length
        && value.counts?.test_subjects === value.test_subjects?.length);
    const collector = 'inventory/collector';
    const collected = name => {
      const file = `${collector}/${name}.json`;
      return { file, value: JSON.parse(fs.readFileSync(path.join(out, file))),
        sha256: hash(path.join(out, file)) };
    };
    const rawCoverage = collected('compiler-architecture-coverage');
    const rawReceipt = collected('compiler-architecture-receipt');
    const rawIssues = collected('compiler-architecture-issues');
    const rawStatus = collected('status');
    const collectorCommandFile = `${collector}/command.txt`;
    const octet = fs.realpathSync(path.join(selection.tool_paths.octet.output,
      'bin/cargo-octet'));
    assert.equal(fs.readFileSync(path.join(out, collectorCommandFile), 'utf8').trimEnd(),
      `${octet} check --workspace --output-format json --artifact-dir ${path.join(out, collector)} -- --all-targets --all-features`,
      'selected compiler collection omitted workspace/all-target/all-feature scope');
    receipt.tools[octet] = { sha256: hash(octet) };
    const summaryFile = `${collector}/summary.txt`;
    const collectorSummary = fs.readFileSync(path.join(out, summaryFile), 'utf8');
    const summaryLines = new Set(collectorSummary.trimEnd().split('\n'));
    for (const line of ['Status: clean', 'Findings: 0', 'Warnings: 0',
      'Errors: 0', 'Architecture findings: 0'])
      assert.ok(summaryLines.has(line),
        `selected compiler collector is not clean: ${line}`);
    assert.equal(rawCoverage.value.schema_version, 'octet-compiler-architecture-coverage/v1');
    assert.equal(rawReceipt.value.schema_version, 'octet-compiler-architecture-receipt/v1');
    assert.equal(rawCoverage.value.status, 'complete');
    assert.equal(rawReceipt.value.status, 'complete');
    assert.equal(rawCoverage.value.expected_unit_ids.length, 32);
    assert.deepEqual(rawCoverage.value.expected_unit_ids, rawCoverage.value.observed_unit_ids);
    for (const name of ['missing_unit_ids', 'unexpected_unit_ids',
      'duplicate_unit_ids', 'invalid_shard_ids']) assert.deepEqual(rawCoverage.value[name], [],
      `selected compiler collection has ${name}`);
    assert.deepEqual(rawReceipt.value.issue_ids, []);
    assert.deepEqual(rawIssues.value, []);
    assert.equal(rawStatus.value.status, 'clean');
    assert.equal(rawStatus.value.cargo_process_exit?.classification, 'success');
    assert.equal(rawStatus.value.cargo_process_exit?.code, 0);
    for (const field of ['error_findings', 'warning_findings', 'total_findings'])
      assert.equal(rawStatus.value[field], 0, `selected compiler has ${field}`);
    assert.equal(rawStatus.value.phases?.lint?.status, 'clean');
    assert.equal(rawStatus.value.phases?.lint?.finding_count, 0);
    assert.equal(rawStatus.value.phases?.architecture?.status, 'clean');
    assert.deepEqual(rawStatus.value.phases?.architecture?.finding_ids, []);
    assert.equal(rawCoverage.value.coverage_id, inventory.generated_by.coverage_id);
    assert.equal(rawReceipt.value.coverage_id, inventory.generated_by.coverage_id);
    assert.equal(rawReceipt.value.architecture_ir_id, inventory.generated_by.ir_id);
    assert.equal(rawReceipt.value.policy_blake3, inventory.generated_by.policy_blake3);
    receipt.compiler = { ir_id: inventory.generated_by.ir_id,
      coverage_id: inventory.generated_by.coverage_id,
      cargo_graph_id: inventory.generated_by.cargo_graph_id,
      policy_blake3: inventory.generated_by.policy_blake3,
      toolchain: inventory.generated_by.toolchain, counts: inventory.counts,
      raw_compiler_receipts: Object.fromEntries([rawCoverage, rawReceipt, rawIssues, rawStatus]
        .map(item => [item.file, item.sha256]).concat([
          [summaryFile, hash(path.join(out, summaryFile))],
          [collectorCommandFile, hash(path.join(out, collectorCommandFile))]])) };
  } else if (label === 'reviewed-compiler-coverage') {
    const coverage = JSON.parse(fs.readFileSync(path.join(out, 'coverage/coverage.json')));
    assert.equal(coverage.valid, true, 'reviewed policy differs from current compiler facts');
    assert.deepEqual(coverage.diagnostics, []);
    const policy = JSON.parse(fs.readFileSync(path.join(root, 'policy/source-inventory.json')));
    assert.equal(policy.schema_version, 'noble-source-inventory-policy/v1');
    assert.equal(coverage.counts.reviewed_subjects, policy.subjects.length);
    assert.equal(coverage.counts.units, 32);
    assert.equal(coverage.counts.bodies + coverage.counts.generated
      + coverage.counts.structural, coverage.counts.reviewed_subjects,
      'every reviewed production path must have exactly one classification');
    assert.ok(coverage.counts.bodies > 0 && coverage.counts.open === coverage.counts.bodies
      && coverage.counts.open_refinement === coverage.counts.bodies
      && coverage.counts.proved === 0 && coverage.counts.excepted === 0);
    assert.ok(policy.subjects.filter(row => row.category === 'body').every(row =>
      row.disposition === 'open' && row.refinement === 'open'),
    'authored bodies must retain open obligations; no nominal proof inferred');
    receipt.gates[label] = { path: 'coverage/coverage.json',
      sha256: hash(path.join(out, 'coverage/coverage.json')),
      valid: coverage.valid, counts: coverage.counts };
  } else if (label === 'fresh-current-workspace-m7-build') {
    const built = bind(label, path.join(build, 'build.json'), 'noble-m7-build/v1',
      value => value.result === 'built' && value.integrity_failures?.length === 0
        && value.binaries?.cli?.sha256 && value.binaries?.m7_peer?.sha256);
    for (const item of Object.values(built.binaries)) assert.equal(hash(item.path), item.sha256);
    const inherited = JSON.parse(fs.readFileSync(base));
    assert.equal(inherited.schema, 'noble-m6-build/v1');
    assert.equal(inherited.result, 'built');
    assert.equal(hash(base), built.base.receipt_sha256);
  } else if (label === 'strict-whole-crate-extraction-check') {
    bind(label, path.join(out, 'extraction/implementation.json'), 'm4-implementation-evidence/v1',
      value => value.mode === 'check' && value.result === 'passed' && strictAudit(value.m7)
        && value.m7_refusals?.length === 37
        && value.m7_refusals.every(control => control.result === 'refused')
        && value.reviewed_lock_sha256 === receipt.proof_lock.sha256);
  } else if (label === 'fresh-current-production-m8-regression') {
    const regression = bind(label, path.join(out, 'm8-regression/regression.json'),
      'noble-declared-m8-fresh-regression/v1', value => value.result === 'passed'
        && value.commands?.length === 3 && value.m8_gate?.fixture_alias_count === 4
        && value.failures?.length === 0 && value.integrity_failures?.length === 0);
    const built = JSON.parse(fs.readFileSync(path.join(build, 'build.json')));
    assert.equal(regression.m7_build.sha256, hash(path.join(build, 'build.json')));
    assert.equal(regression.m7_build.cli_sha256, built.binaries.cli.sha256);
    assert.equal(hash(path.join(out, 'm8-regression', regression.m8_gate.path)),
      regression.m8_gate.sha256);
    assert.equal(regression.m8_gate.current_cli_sha256, built.binaries.cli.sha256);
  } else if (label === 'm7-regression') {
    const accepted = bind(label, path.join(out, 'm7/report.json'), 'noble-m7-acceptance/v1',
      value => value.result === 'passed' && value.summary?.required_cases === 7
        && value.summary?.passed_cases === 7 && value.summary?.variant_count === 16
        && value.integrity_failures?.length === 0);
    const built = JSON.parse(fs.readFileSync(path.join(build, 'build.json')));
    assert.equal(accepted.executables.cli.sha256, built.binaries.cli.sha256);
    assert.equal(accepted.executables.peer.sha256, built.binaries.m7_peer.sha256);
  } else if (label === 'mc1-regression') bind(label, path.join(out, 'mc1.json'),
    'noble-mc1-cli-acceptance/v1', value => value.passed && value.cases.length === 36);
  else if (label === 'mc2-regression') bind(label, path.join(out, 'mc2.json'),
    'noble-mc2-acceptance/v1', value => value.passed && value.summary?.cases === 15
      && value.summary?.executed === 15);
  else if (label === 'm3-four-configuration-regression') {
    const value = JSON.parse(fs.readFileSync(path.join(out, 'm3/report.json')));
    assert.equal(value.schema_version, 1);
    assert.equal(value.scope, 'm3-resource-free-program-representation-comparison');
    assert.equal(value.result, 'eligible-for-selection');
    assert.equal(value.configurations.length, 4);
    assert.ok(value.configurations.every(row => row.outcome === 'executed'));
    receipt.gates[label] = { path: 'm3/report.json',
      sha256: hash(path.join(out, 'm3/report.json')), scope: value.scope };
  } else if (label === 'm4-runtime-regression') bind(label, path.join(out, 'm4/acceptance.json'),
    'noble-m4-acceptance/v1', value => value.result === 'passed'
      && value.cases.length === 18 && value.controls.length === 11);
  else if (label === 'm5-component-regression') bind(label, path.join(out, 'm5/report.json'),
    'noble-m5-acceptance/v1', value => value.result === 'passed'
      && value.summary?.required_cases === 24 && value.summary?.variants === 84
      && value.summary?.native_tests === 44);
  else if (label === 'm6-component-regression') bind(label, path.join(out, 'm6/report.json'),
    'noble-m6-acceptance/v1', value => value.result === 'passed'
      && value.integrity_failures?.length === 0
      && value.summary?.executed_cases?.length === value.summary?.required_cases?.length);
}
try {
  receipt.source_files = snapshot();
  receipt.source_revision = `sha256:${sha(JSON.stringify(receipt.source_files))}`;
  receipt.git_projection = gitSourceProjection(root, Object.keys(receipt.source_files));
  for (const row of receipt.git_projection.rows)
    assert.equal(row.worktree_sha256, receipt.source_files[row.path].sha256,
      `${row.path}: Git staged/worktree projection differs from source snapshot`);
  save();
  for (const job of tasks) {
    const command = execute(job);
    accept(job.label, command);
    save();
  }
  const built = JSON.parse(fs.readFileSync(path.join(build, 'build.json')));
  const inherited = JSON.parse(fs.readFileSync(base));
  for (const [name, item] of Object.entries(receipt.source_files)) if (name.startsWith('crates/'))
    assert.equal(hash(path.join(workspace, name)), item.sha256,
      `${name}: current-source peer would use different crate bytes`);
  for (const name of [...sources].filter(file => file.startsWith('verification/declared-modules-v1/peer/'))) {
    const file = path.join(workspace, name);
    fs.mkdirSync(path.dirname(file), { recursive: true });
    if (!fs.existsSync(file)) fs.writeFileSync(file, fs.readFileSync(path.join(root, name)),
      { flag: 'wx', mode: 0o400 });
    assert.equal(hash(file), receipt.source_files[name].sha256, `copied peer source mismatch: ${name}`);
  }
  assert.equal(hash(cargo), inherited.tools.cargo.sha256,
    'declared peer Cargo differs from fresh workspace build');
  const peerEnv = { ...inherited.environment, CARGO_TARGET_DIR: path.join(out, 'declared-peer-target') };
  const manifest = 'verification/declared-modules-v1/peer/Cargo.toml';
  const peerBuild = execute(task('independent-declared-kernel-peer-build', cargo,
    ['build', '--manifest-path', manifest, '--locked', '--offline',
      '--message-format=json', ...vendor], peerEnv, 3_900_000, workspace));
  execute(task('independent-declared-kernel-peer-clippy', cargo,
    ['clippy', '--manifest-path', manifest, '--all-targets', '--locked', '--offline',
      ...vendor, '--', '-D', 'warnings'], peerEnv, 3_900_000, workspace));
  const rows = fs.readFileSync(path.join(out, peerBuild.stdout), 'utf8').split('\n')
    .filter(line => line.startsWith('{')).map(line => JSON.parse(line));
  const binaries = [...new Set(rows.filter(row => row.reason === 'compiler-artifact'
      && row.target?.name === 'noble-declared-modules-peer'
      && row.target.kind?.includes('bin') && !row.profile?.test)
    .map(row => row.executable).filter(Boolean))];
  assert.equal(binaries.length, 1, 'exactly one compiler-reported independent kernel peer required');
  const peer = binaries[0], peerSha = hash(peer);
  receipt.peer = { executable: peer, sha256: peerSha, cargo_command: peerBuild.label };
  execute(task('declared-modules-production-and-kernel-acceptance', node,
    [gate('declared-modules-v1'), cli, peer, path.join(out, 'declared')], common, 1_800_000));
  const declared = bind('declared-modules-acceptance', path.join(out, 'declared/report.json'),
    'noble-declared-modules-acceptance/v1', value => value.result === 'passed'
      && value.summary?.case_rows === 39 && value.summary?.planned_scenarios === 27
      && value.summary?.failure_count === 0 && value.summary?.integrity_failures === 0
      && [...(value.summary?.resource_operations ?? [])].sort().join(',') === 'capture,drop,dup');
  const declaredPlan = JSON.parse(fs.readFileSync(path.join(out, 'declared/plan/plan.json')));
  const digests = entries => Object.fromEntries(Object.entries(entries).sort()
    .map(([name, entry]) => [name, entry.sha256]));
  const revisionInputs = {
    reviewed_source: digests(declaredPlan.sources),
    source_fixtures: digests(declaredPlan.program_bytes),
    host_bindings: digests(declaredPlan.bindings),
    selected_tools: digests(declaredPlan.tools),
    compiled_executables: digests(declared.tools),
    canonical_workloads: Object.fromEntries(Object.entries(declaredPlan.canonical).sort()
      .map(([file, entry]) => [file, {
        source_sha256: entry.source_sha256, workload_sha256: entry.workload_sha256,
      }])),
  };
  assert.deepEqual(declared.revision_inputs, revisionInputs,
    'declared acceptance revision omitted frozen source/tool/fixture bytes');
  assert.equal(declared.source_revision, `sha256:${sha(JSON.stringify(revisionInputs))}`,
    'declared acceptance source revision differs from reviewed byte table');
  for (const [name, item] of Object.entries(declaredPlan.sources))
    assert.equal(receipt.source_files[name]?.sha256, item.sha256,
      `${name}: declared acceptance did not use frozen assurance source`);
  for (const row of declared.cases.filter(item => ['DX-03', 'DX-08', 'DX-09'].includes(item.case_id)))
    assert.equal(row.source_revision, declared.source_revision,
      `${row.case_id}: canonical case source revision differs from acceptance`);
  assert.equal(declared.tools.noble.sha256, built.binaries.cli.sha256,
    'declared acceptance used another production CLI');
  assert.equal(declared.tools['kernel-peer'].sha256, peerSha,
    'declared acceptance used another kernel peer');
  assert.equal(hash(peer), peerSha, 'kernel peer changed during acceptance');
  assert.equal(hash(cli), built.binaries.cli.sha256, 'production CLI changed during acceptance');
  for (const name of [...sources].filter(file => file.startsWith('verification/declared-modules-v1/peer/')))
    assert.equal(hash(path.join(workspace, name)), receipt.source_files[name].sha256,
      `${name}: built peer source snapshot changed`);
  assert.deepEqual(snapshot(), receipt.source_files, 'source changed during assurance');
  assert.equal(hash(privateNixConfig), receipt.private_nix_configuration.sha256,
    'reviewed signed/sandbox Nix configuration changed during assurance');
  assert.equal(hash(boundSystemNixConfig), receipt.private_nix_configuration.sha256,
    'private user-service read-only Nix bind changed during assurance');
  assert.deepEqual(gitSourceProjection(root, Object.keys(receipt.source_files)), receipt.git_projection,
    'staged/worktree Git projection changed during assurance');
  assert.equal(hash(path.join(root, receipt.proof_lock.path)), receipt.proof_lock.sha256,
    'reviewed M4 source lock changed during assurance');
  for (const [file, item] of Object.entries(receipt.tools))
    assert.equal(hash(file), item.sha256, `tool changed: ${file}`);
  for (const command of receipt.commands) for (const field of ['stdout', 'stderr'])
    assert.equal(hash(path.join(out, command[field])), command[`${field}_sha256`],
      `${command.label}: transcript changed`);
  for (const [label, item] of Object.entries(receipt.gates))
    assert.equal(hash(path.join(out, item.path)), item.sha256, `${label}: gate receipt changed`);
  for (const [file, digest] of Object.entries(receipt.compiler.raw_compiler_receipts))
    assert.equal(hash(path.join(out, file)), digest, `${file}: compiler collector receipt changed`);
} catch (error) { receipt.integrity_failures.push(String(error.stack ?? error)); }
receipt.result = receipt.failures.length === 0 && receipt.integrity_failures.length === 0
  && receipt.commands.length === tasks.length + 3
  && Object.keys(receipt.gates).length === 14 ? 'passed' : 'failed';
save();
console.log(JSON.stringify({ schema: receipt.schema, result: receipt.result,
  report: path.join(out, 'assurance.json'), commands: receipt.commands.length,
  verified_gates: Object.keys(receipt.gates), failures: receipt.failures,
  integrity_failures: receipt.integrity_failures }));
process.exitCode = receipt.result === 'passed' ? 0 : 1;
