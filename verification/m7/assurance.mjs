#!/usr/bin/env node
// Usage: SELECTED_NODE verification/m7/assurance.mjs {plan|run} NEW_EXTERNAL_DIRECTORY
// Pre-promotion plan is read-only. Run starts only with a reviewed M7 strict root integrated
// into the whole-crate M4 check lock. Every executable gate below actually runs
// again on frozen implementation source; after document/case promotion and Cairn
// sync/archive, final-documents.mjs checks the distinct completed source.
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const root = fs.realpathSync(path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..'));
const [mode, destination, ...extra] = process.argv.slice(2);
assert.ok(['plan', 'run'].includes(mode) && destination && !extra.length,
  'usage: SELECTED_NODE verification/m7/assurance.mjs {plan|run} NEW_EXTERNAL_DIRECTORY');
const out = path.resolve(destination);
assert.ok(out !== root && !out.startsWith(`${root}${path.sep}`), 'artifacts must be outside the repository');
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const fileHash = file => sha(fs.readFileSync(file));
const selection = JSON.parse(fs.readFileSync(path.join(root, 'policy/tool-selection.json')));
const pins = JSON.parse(fs.readFileSync(path.join(root, 'verification/m6/pins.json')));
const node = path.join(selection.tool_paths.node.output, 'bin/node');
assert.equal(fs.realpathSync(process.execPath), fs.realpathSync(node), 'use policy-selected Node without flags');
assert.deepEqual(process.execArgv, []);
const bun = process.env.NOBLE_M7_BUN ?? '/nix/store/qkydg77xf2s395md9fq0a6djjpq7fzh4-bun-1.4.2/bin/bun';
const cargo = path.join(selection.tool_paths.quality_rust.output, 'bin/cargo');
const rust = path.join(selection.tool_paths.quality_rust.output, 'bin');
const nix = pins.nix ?? '/run/current-system/sw/bin/nix';
const wasmTools = path.join(selection.tool_paths.wasm_tools.output, 'bin/wasm-tools');
const wasmOpt = path.join(selection.tool_paths.binaryen.output, 'bin/wasm-opt');
const vendor = ['--config', 'source.crates-io.replace-with="m6-vendor"', '--config',
  `source.m6-vendor.directory="${pins.vendor}/source-registry-0"`];
const build = path.join(out, 'build');
const built = path.join(build, 'build.json');
const extracted = path.join(out, 'extraction');
const base = path.join(build, 'base/build.json');
const gate = name => path.join(root, `verification/${name}/gate.mjs`);
const artifacts = name => path.join(out, name);
const cli = path.join(build, 'base/executables/cli');
const m6Peer = path.join(build, 'base/executables/peer');
const m5Peer = path.join(build, 'base/executables/m5_peer');
const taskTests = path.join(build, 'base/executables/task_tests');
const resourceTests = path.join(build, 'base/executables/resource_tests');
const authorityTests = path.join(build, 'base/executables/authority_tests');
const m7Peer = path.join(build, 'executables/m7-peer');
const mc2Tests = path.join(build, 'executables/mc2-core-control-tests');
const common = { PATH: '/run/current-system/sw/bin', LANG: 'C.UTF-8', LC_ALL: 'C.UTF-8',
  HOME: path.join(out, 'home'), TMPDIR: path.join(out, 'tmp'),
  XDG_RUNTIME_DIR: process.env.XDG_RUNTIME_DIR };
const nixArgs = ['--option', 'substituters', 'https://cache.nixos.org https://nix-community.cachix.org',
  '--extra-experimental-features', 'nix-command flakes'];
const cargoEnv = { ...common, PATH: `${rust}:${pins.linker_bin}:/run/current-system/sw/bin`,
  RUSTC: path.join(rust, 'rustc'), RUSTDOC: path.join(rust, 'rustdoc'), RUSTC_WRAPPER: '',
  RUSTC_WORKSPACE_WRAPPER: '', RUSTFLAGS: '', CARGO_ENCODED_RUSTFLAGS: '', CARGO_INCREMENTAL: '0',
  CARGO_HOME: path.join(out, 'cargo-home'), CARGO_TARGET_DIR: path.join(out, 'quality-target') };
const m4Env = { ...cargoEnv, CARGO_HOME: path.join(build, 'base/cargo-home'),
  CARGO_TARGET_DIR: path.join(build, 'base/target') };
const proofEnv = { ...common, NOBLE_LEAN: path.join(selection.tool_paths.lean.output, 'bin/lean'),
  NOBLE_BWRAP: '/nix/store/lqndphylsxqwbwm804n473pb4sqb98sh-bubblewrap-0.11.2/bin/bwrap',
  NOBLE_PRLIMIT: '/nix/store/mqvbf0flamqaq9c3ihb496aahag897n0-util-linux-2.42.3-bin/bin/prlimit',
  NOBLE_SYSTEMD_RUN: '/nix/store/baxgs06659w4i6ggsfr56srjrb52v1h8-systemd-261.2/bin/systemd-run' };
const tasks = [
  ['nix-flake-source-prefetch', nix, [...nixArgs, 'flake', 'prefetch', '--json', '.'], common, 300_000],
  ['build', node, [path.join(root, 'verification/m7/build.mjs'), build], common, 3_900_000],
  ['strict-whole-crate-extraction-check', node, [path.join(root, 'verification/m4/implementation.mjs'), 'check', extracted], common, 7_200_000],
  ['m7-acceptance', node, [gate('m7'), cli, m7Peer, wasmTools, artifacts('m7')], common, 1_200_000],
  ['mc1-regression', bun, [gate('mc1'), cli, artifacts('mc1.json')], proofEnv, 3_600_000],
  ['mc2-regression', bun, [gate('mc2'), cli, artifacts('mc2.json')],
    { ...proofEnv, NOBLE_MC2_CORE_TEST_BINARY: mc2Tests }, 7_200_000],
  ['m3-four-configuration-regression', node,
    [path.join(root, 'tools/m3-wasm.mjs'), '--noble', cli, '--out', artifacts('m3')],
    { ...common, NOBLE_M3_NODE: node, NOBLE_M3_WASM_TOOLS: wasmTools, NOBLE_M3_WASM_OPT: wasmOpt }, 3_600_000],
  ['m4-runtime-regression', node, [gate('m4'), cli, artifacts('m4')], m4Env, 1_200_000],
  ['m5-component-regression', node,
    [gate('m5'), cli, m5Peer, resourceTests, authorityTests, artifacts('m5'),
      '--opacity-report', path.join(build, 'm5-opacity/opacity.json')], common, 3_600_000],
  ['m6-component-regression', node,
    [gate('m6'), cli, m6Peer, taskTests, artifacts('m6'), '--build-report', base], common, 3_600_000],
  ['cairn-and-canonical-documents', bun,
    [path.join(root, 'tools/check-specs.mjs'), '--self-test', '--report'], common, 300_000],
  ['cairn-change-validation', bun, [path.join(root, 'tools/cairn.mjs'), 'validate', '--root', root], common, 300_000],
  ['rust-all-workspace-tests', cargo,
    ['test', '--workspace', '--all-targets', '--all-features', '--locked', '--offline', ...vendor], cargoEnv, 3_600_000],
  ['rust-all-workspace-clippy', cargo,
    ['clippy', '--workspace', '--all-targets', '--all-features', '--locked', '--offline', ...vendor, '--', '-D', 'warnings'], cargoEnv, 3_600_000],
  ['published-octet-precommit', nix,
    [...nixArgs, 'develop', '-c', 'pre-commit', 'run', 'octet-deny-all', '--all-files'], common, 3_600_000],
  ['complete-nix-flake-check', nix, [...nixArgs, 'flake', 'check', '--keep-going', '-L'], common, 7_200_000],
];
const sourceRoots = [
  ['noble_kernel.dataspace.admit_shared_memory', 31],
  ['noble_kernel.dataspace.decide_publication', 32],
  ['noble_kernel.dataspace.permits', 33],
];
const proofRoots = [
  'publication_refines', 'permission_refines', 'shared_memory_refines',
  'shared_memory_refused', 'publish_right_required', 'observe_right_required',
  'duplicate_publication_unchanged', 'changed_publication_replaces', 'first_publication_adds',
].map(name => `M7Syndicate.${name}`);
const exactRoots = rows => Array.isArray(rows) &&
  JSON.stringify(rows.map(row => [row.declaration, row.def_id])) === JSON.stringify(sourceRoots);
const strictAudit = m7 => {
  const refinement = m7?.syndicate_refinement?.audit;
  return exactRoots(m7?.coverage?.implementation_roots) &&
    refinement?.schema === 'noble-m7-syndicate-audit/v1' && refinement.result === 'passed' &&
    exactRoots(refinement.implementation_roots) &&
    JSON.stringify(refinement.strict_roots?.map(row => row.declaration)) === JSON.stringify(proofRoots) &&
    JSON.stringify(refinement.packet?.strict_roots?.map(row => row.declaration)) === JSON.stringify(proofRoots);
};
function blockers() {
  const problems = [];
  if (!common.XDG_RUNTIME_DIR || !fs.existsSync(path.join(common.XDG_RUNTIME_DIR, 'bus'))
    || !fs.statSync(path.join(common.XDG_RUNTIME_DIR, 'bus')).isSocket())
    problems.push('MC1/MC2 proofs and pinned SSH fetches require the active user XDG_RUNTIME_DIR bus');
  for (const [name, file] of Object.entries(proofEnv).filter(([key]) => key.startsWith('NOBLE_')))
    if (!fs.existsSync(file)) problems.push(`selected MC1 proof sandbox tool missing: ${name}=${file}`);
  const lockFile = path.join(root, 'verification/m4/extraction-lock.json');
  let lock;
  try { lock = JSON.parse(fs.readFileSync(lockFile)); }
  catch (error) { return [`Missing readable reviewed extraction lock: ${error}`]; }
  if (lock.schema !== 'm4-extraction-lock/v1') problems.push('Unexpected whole-crate extraction lock schema');
  if (!fs.existsSync(path.join(root, 'proofs/m7'))) problems.push('proofs/m7 strict M7 proof root is absent');
  if (!fs.existsSync(path.join(root, 'verification/m7/extraction.mjs')))
    problems.push('verification/m7/extraction.mjs integration is absent');
  if (!strictAudit(lock.m7))
    problems.push('reviewed lock lacks exact compiled three source roots and nine strict M7 theorems');
  for (const file of ['M7Syndicate.lean', 'M7Audit.lean', 'lakefile.toml', 'lake-manifest.json', 'lean-toolchain']) {
    if (!lock.source_files?.[`proofs/m7/${file}`]) problems.push(`reviewed lock does not bind proofs/m7/${file}`);
  }
  if (!lock.source_files?.['verification/m7/extraction.mjs'])
    problems.push('reviewed lock does not bind the M7 extraction adapter');
  return problems;
}
const pending = blockers();
if (mode === 'plan') {
  console.log(JSON.stringify({ schema: 'noble-m7-assurance-plan/v1', phase: 'pre-promotion',
    planned_result: pending.length ? 'blocked' : 'ready',
    artifact_directory: out, prerequisites: pending, commands: tasks.map(([label, executable, argv, environment, timeout_ms]) =>
      ({ label, executable, argv, cwd: root, environment, timeout_ms })),
    next_phase: 'After case/status/evidence/roadmap promotion and Cairn sync/archive, run verification/m7/final-documents.mjs.',
    non_claims: ['Plan mode executes none of the listed gates; prior milestone receipts are not fresh regressions.',
      'A proof-lock field is not itself human review: final check must actually rerun Charon, Aeneas and Lean.'] }, null, 2));
  process.exit(0);
}
assert.ok(!pending.length, `assurance blocked before run:\n${pending.join('\n')}`);
assert.ok(!fs.existsSync(out), 'fresh artifacts required');
assert.equal(fs.realpathSync(path.dirname(out)), path.dirname(out), 'artifact parent must not be a symlink');
fs.mkdirSync(out);
for (const name of ['commands', 'home', 'tmp', 'cargo-home']) fs.mkdirSync(path.join(out, name));
const receipt = { schema: 'noble-m7-assurance/v1', phase: 'pre-promotion', result: 'running', source_root: root,
  artifact_directory: out, proof_lock: { path: 'verification/m4/extraction-lock.json',
    sha256: fileHash(path.join(root, 'verification/m4/extraction-lock.json')) },
  source_revision: null, source_files: {}, tools: {}, commands: [], gates: {}, failures: [], integrity_failures: [],
  assumptions: [
    'The M7 theorem root and renewed lock must have been independently reviewed; a local lock file alone cannot prove review.',
    'Selected compiler, Charon/Aeneas/Lean, Wasmtime/Node, Bun, Octet, Nix and the OS remain trusted tooling.',
    'M1/M2 platform/tool and source-inventory quality evidence comes from the fresh complete Nix check; runtime MC1 through M6 gates rerun separately.',
  ],
  non_claims: ['Universal source-to-Wasm/engine refinement', 'full Syndicate/Preserves/WASI implementation',
    'M1/M2 original milestone acceptance re-execution', 'Historical receipts as fresh M7 regressions',
    'Final documents, promoted case statuses and archived Cairn source checked after this pre-promotion run'],
};
const save = () => fs.writeFileSync(path.join(out, 'assurance.json'), JSON.stringify(receipt, null, 2) + '\n');
function collect(directory) {
  const names = [];
  function walk(relative) {
    for (const entry of fs.readdirSync(path.join(root, relative), { withFileTypes: true })) {
      if (['.git', '.lake', 'target', 'node_modules'].includes(entry.name)) continue;
      const file = `${relative}/${entry.name}`;
      if (entry.isDirectory()) walk(file);
      else if (entry.isFile()) names.push(file);
      else throw Error(`unreviewed source entry: ${file}`);
    }
  }
  walk(directory);
  return names;
}
function snapshot() {
  const names = new Set(['Cargo.toml', 'Cargo.lock', 'flake.nix', 'flake.lock', 'rust-toolchain.toml',
    '.pre-commit-config.yaml', 'verification/m4/implementation.mjs', 'verification/m4/extraction-lock.json',
    'verification/m6/build.mjs', 'verification/m6/pins.json', 'tools/m3-wasm.mjs', 'tools/check-specs.mjs',
    'tools/cairn.mjs', 'verification/m5/build.mjs', 'verification/m5/gate.mjs',
    'verification/m5/core-boundary.mjs', 'verification/m5/borrow-boundary.rs',
    'verification/m5/identity-boundary.rs', 'verification/m5/pins.json',
    'verification/m5/peer/Cargo.toml', 'verification/m5/peer/Cargo.lock',
    'verification/mc2/gate.mjs', 'verification/mc2/runtime-controls.mjs',
    ...collect('verification/mc1').filter(file => /\.(?:mjs|lean|noble-contract)$/.test(file)),
    ...['crates', 'proofs/m7', 'verification/m7', 'verification/m5/peer/src',
      'verification/mc2/contracts', 'policy', 'specs/conformance',
      '.cairn/specs/safety', '.cairn/specs/wit-wasi', '.cairn/changes/m7-syndicate-service'].flatMap(collect)]);
  return Object.fromEntries([...names].sort().map(file => {
    const candidate = path.join(root, file);
    assert.ok(fs.lstatSync(candidate).isFile(), `not a plain source file: ${file}`);
    return [file, { sha256: fileHash(candidate), bytes: fs.statSync(candidate).size }];
  }));
}
function execute(label, executable, argv, environment, timeout_ms) {
  const tool = fs.realpathSync(executable);
  const digest = fileHash(tool);
  if (!receipt.tools[tool]) receipt.tools[tool] = { sha256: digest };
  assert.equal(receipt.tools[tool].sha256, digest, `${label}: executable changed`);
  const id = receipt.commands.length + 1;
  const stem = `commands/${String(id).padStart(3, '0')}-${label}`;
  const stdout = path.join(out, `${stem}.stdout`);
  const stderr = path.join(out, `${stem}.stderr`);
  const outFd = fs.openSync(stdout, 'wx', 0o600), errFd = fs.openSync(stderr, 'wx', 0o600);
  const row = { label, executable: tool, executable_sha256: digest, argv, cwd: root,
    environment, timeout_ms, stdout: path.relative(out, stdout), stderr: path.relative(out, stderr) };
  receipt.commands.push(row); save();
  let result;
  try { result = spawnSync(tool, argv, { cwd: root, env: environment, stdio: ['ignore', outFd, errFd],
    timeout: timeout_ms, killSignal: 'SIGKILL' }); }
  finally { fs.closeSync(outFd); fs.closeSync(errFd); }
  Object.assign(row, { status: result?.status ?? null, signal: result?.signal ?? null,
    error: result?.error ? String(result.error) : null, stdout_sha256: fileHash(stdout), stderr_sha256: fileHash(stderr) });
  for (const file of [stdout, stderr]) fs.chmodSync(file, 0o400);
  row.passed = row.status === 0 && row.signal === null && row.error === null && fileHash(tool) === digest;
  if (!row.passed) receipt.failures.push({ label, command: id, stderr: row.stderr, status: row.status, error: row.error });
  save();
  return row;
}
function gateReceipt(label, file, schema, acceptable) {
  const observed = JSON.parse(fs.readFileSync(file));
  if (schema === 'm3-resource-free-program-representation-comparison/v1') {
    assert.equal(observed.schema_version, 1, `${label}: wrong receipt schema version`);
    assert.equal(observed.scope, 'm3-resource-free-program-representation-comparison',
      `${label}: wrong execution scope`);
  } else assert.equal(observed.schema, schema, `${label}: wrong receipt schema`);
  assert.ok(acceptable(observed), `${label}: gate receipt did not pass its complete execution conditions`);
  const entry = { path: path.relative(out, file), sha256: fileHash(file), schema,
    source_revision: observed.source_revision ?? null };
  receipt.gates[label] = entry;
  return observed;
}
function accept(label) {
  const where = {
    'm7-acceptance': artifacts('m7'),
    'mc1-regression': artifacts('mc1.json'),
    'mc2-regression': artifacts('mc2.json'),
    'm3-four-configuration-regression': artifacts('m3'),
    'm4-runtime-regression': artifacts('m4'),
    'm5-component-regression': artifacts('m5'),
    'm6-component-regression': artifacts('m6'),
  }[label];
  if (label === 'nix-flake-source-prefetch') {
    const row = receipt.commands.find(command => command.label === label);
    const fetched = JSON.parse(fs.readFileSync(path.join(out, row.stdout)));
    assert.ok(typeof fetched.storePath === 'string' && fetched.storePath.length > 0,
      'Nix prefetch must identify a materialized flake source');
    assert.ok(typeof fetched.hash === 'string' && fetched.hash.startsWith('sha256-'),
      'Nix prefetch must retain the source NAR hash');
    const flakeSource = fs.realpathSync(fetched.storePath);
    assert.ok(fs.statSync(flakeSource).isDirectory(), 'Nix prefetched source must be a directory');
    const critical = Object.keys(receipt.source_files).filter(file => file.startsWith('crates/noble-syndicate/')
      || file.startsWith('crates/noble-kernel/src/dataspace/')
      || file.startsWith('proofs/m7/') || file.startsWith('verification/m7/')
      || file.startsWith('.cairn/specs/') || file.startsWith('.cairn/changes/m7-syndicate-service/')
      || file.startsWith('policy/')
      || file === 'Cargo.toml' || file === 'Cargo.lock'
      || file === 'specs/conformance/safety-cases.json'
      || file === 'specs/conformance/wit-wasi-cases.json');
    assert.ok(critical.length >= 12, 'incomplete critical M7 source selection');
    for (const file of critical) {
      const materialized = path.join(flakeSource, file);
      assert.ok(fs.statSync(materialized).isFile(), `Nix prefetch excluded final M7 source: ${file}`);
      assert.equal(fileHash(materialized), receipt.source_files[file].sha256,
        `Nix prefetch rewrote final M7 source: ${file}`);
    }
    receipt.gates.nix_source = { path: row.stdout, sha256: fileHash(path.join(out, row.stdout)),
      source: flakeSource, nar_hash: fetched.hash, critical_files: critical.length };
    return;
  }
  if (label === 'build') {
    const value = gateReceipt(label, built, 'noble-m7-build/v1', row => row.result === 'built'
      && row.integrity_failures?.length === 0 && Object.keys(row.binaries).length === 8
      && row.opacity?.source_count >= 390 && row.opacity?.locked_sources >= 390);
    const baseBuild = JSON.parse(fs.readFileSync(base));
    assert.equal(baseBuild.result, 'built');
    const m5Opacity = JSON.parse(fs.readFileSync(path.join(build, value.opacity.report)));
    const m5Build = JSON.parse(fs.readFileSync(path.join(build, value.opacity.build_receipt)));
    assert.equal(m5Opacity.passed, true);
    assert.equal(m5Build.result, 'passed');
    assert.equal(fileHash(path.join(build, value.opacity.report)), value.opacity.sha256);
    assert.equal(fileHash(path.join(build, value.opacity.build_receipt)), value.opacity.build_receipt_sha256);
    for (const [name, binary] of Object.entries(value.binaries)) {
      assert.equal(fileHash(binary.path), binary.sha256, `compiled ${name} changed`);
      if (name === 'm7_peer') assert.equal(binary.target, 'noble-m7-peer');
    }
    return;
  }
  if (label === 'strict-whole-crate-extraction-check') {
    const file = path.join(extracted, 'implementation.json');
    gateReceipt(label, file, 'm4-implementation-evidence/v1', row => row.mode === 'check'
      && row.result === 'passed' && strictAudit(row.m7)
      && Array.isArray(row.m7_refusals) && row.m7_refusals.length >= 8
      && row.m7_refusals.every(control => control.result === 'refused'));
    return;
  }
  if (label === 'm7-acceptance') {
    const value = gateReceipt(label, path.join(where, 'report.json'), 'noble-m7-acceptance/v1', row =>
      row.result === 'passed' && row.summary?.required_cases === 7 && row.summary?.passed_cases === 7
        && row.summary?.variant_count === 16 && row.integrity_failures?.length === 0);
    const source = JSON.parse(fs.readFileSync(built)).sources;
    for (const [file, entry] of Object.entries(source)) if (value.sources[file])
      assert.equal(entry.sha256, value.sources[file].sha256, `M7 compiled source differs from accepted gate: ${file}`);
    return;
  }
  if (label === 'mc1-regression') gateReceipt(label, where, 'noble-mc1-cli-acceptance/v1', row => row.passed && row.cases.length === 36);
  if (label === 'mc2-regression') gateReceipt(label, where, 'noble-mc2-acceptance/v1', row => row.passed
    && row.summary?.cases === 15 && row.summary?.executed === 15);
  if (label === 'm3-four-configuration-regression') gateReceipt(label, path.join(where, 'report.json'),
    'm3-resource-free-program-representation-comparison/v1', row => row.result === 'eligible-for-selection'
      && row.configurations.length === 4 && row.configurations.every(item => item.outcome === 'executed'));
  if (label === 'm4-runtime-regression') gateReceipt(label, path.join(where, 'acceptance.json'),
    'noble-m4-acceptance/v1', row => row.result === 'passed' && row.cases.length === 18 && row.controls.length === 11);
  if (label === 'm5-component-regression') gateReceipt(label, path.join(where, 'report.json'),
    'noble-m5-acceptance/v1', row => row.result === 'passed' && row.summary?.required_cases === 24
      && row.summary?.variants === 84 && row.summary?.native_tests === 44);
  if (label === 'm6-component-regression') gateReceipt(label, path.join(where, 'report.json'),
    'noble-m6-acceptance/v1', row => row.result === 'passed' && row.integrity_failures?.length === 0
      && row.summary?.executed_cases?.length === row.summary?.required_cases?.length);
}
try {
  receipt.source_files = snapshot();
  receipt.source_revision = `sha256:${sha(JSON.stringify(receipt.source_files))}`;
  save();
  for (const task of tasks) {
    const row = execute(...task);
    if (!row.passed) {
      // Later runtime gates are independent of one another, but not of a build.
      if (task[0] === 'build' || task[0] === 'nix-flake-source-prefetch') break;
      continue;
    }
    try { accept(task[0]); }
    catch (error) { receipt.failures.push({ label: task[0], failure: String(error.stack ?? error) }); }
    save();
    if (['build', 'nix-flake-source-prefetch'].includes(task[0]) && receipt.failures.length) break;
  }
  assert.deepEqual(snapshot(), receipt.source_files, 'source/config changed during complete assurance run');
  assert.equal(fileHash(path.join(root, 'verification/m4/extraction-lock.json')), receipt.proof_lock.sha256);
  for (const [file, entry] of Object.entries(receipt.tools)) assert.equal(fileHash(file), entry.sha256, `tool changed: ${file}`);
  for (const row of receipt.commands) for (const name of ['stdout', 'stderr'])
    assert.equal(fileHash(path.join(out, row[name])), row[`${name}_sha256`], `transcript changed: ${row.label}`);
  for (const entry of Object.values(receipt.gates)) assert.equal(fileHash(path.join(out, entry.path)), entry.sha256);
} catch (error) { receipt.integrity_failures.push(String(error.stack ?? error)); }
receipt.result = receipt.failures.length === 0 && receipt.integrity_failures.length === 0
  && receipt.commands.length === tasks.length && Object.keys(receipt.gates).length === 10 ? 'passed' : 'failed';
save();
console.log(JSON.stringify({ schema: receipt.schema, result: receipt.result,
  report: path.join(out, 'assurance.json'), executed_commands: receipt.commands.length,
  verified_gates: Object.keys(receipt.gates), failures: receipt.failures, integrity_failures: receipt.integrity_failures }));
process.exitCode = receipt.result === 'passed' ? 0 : 1;
