#!/usr/bin/env node
// Pre-promotion M8 quality matrix. `plan` reports prerequisites without
// executing gates; `run` requires a separately reviewed whole-crate extraction
// lock and executes fresh M8/M7/prior gates against one frozen source revision.
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { gitSourceProjection } from './source-projection.mjs';

const root = fs.realpathSync(path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..'));
const [mode, destination, ...extra] = process.argv.slice(2);
assert.ok(['plan', 'run'].includes(mode) && destination && !extra.length,
  'usage: SELECTED_NODE verification/m8/assurance.mjs {plan|run} NEW_EXTERNAL_DIRECTORY');
const out = path.resolve(destination);
assert.ok(out !== root && !out.startsWith(`${root}${path.sep}`), 'external artifacts required');
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const hash = file => sha(fs.readFileSync(file));
const selection = JSON.parse(fs.readFileSync(path.join(root, 'policy/tool-selection.json')));
const pins = JSON.parse(fs.readFileSync(path.join(root, 'verification/m6/pins.json')));
const node = path.join(selection.tool_paths.node.output, 'bin/node');
assert.equal(fs.realpathSync(process.execPath), fs.realpathSync(node), 'policy-selected Node required');
assert.deepEqual(process.execArgv, []);
const bun = process.env.NOBLE_M8_BUN ?? '/nix/store/qkydg77xf2s395md9fq0a6djjpq7fzh4-bun-1.4.2/bin/bun';
const cargo = path.join(selection.tool_paths.quality_rust.output, 'bin/cargo');
const rust = path.join(selection.tool_paths.quality_rust.output, 'bin');
const nix = pins.nix ?? '/run/current-system/sw/bin/nix';
// The selected Cairn executable and policy are already materialized. The
// generic `nix run` wrapper can re-fetch unrelated flake inputs after Cairn
// has validated the change, so invoke these exact reviewed store objects.
const cairn = path.join(selection.tool_paths.cairn.output, 'bin/cairn');
const cairnPolicy = '/nix/store/jqz083ahq10fdrakci9xzvf7bl4nh075-source/cairn-policy/generated/cairn-policy.json';
assert.equal(selection.sources.cairn.rev, '15f00875562025e7ea7e0d1f4af24d1a2e2ac06f');
assert.equal(hash(cairnPolicy), '62b9ade2bc8af10b2c68776dba8abb374a6c7331aae7ad8bb41ca938ef100d27',
  'selected Cairn source policy changed');
const wasmTools = path.join(selection.tool_paths.wasm_tools.output, 'bin/wasm-tools');
const wasmOpt = path.join(selection.tool_paths.binaryen.output, 'bin/wasm-opt');
const vendor = ['--config', 'source.crates-io.replace-with="m6-vendor"', '--config',
  `source.m6-vendor.directory="${pins.vendor}/source-registry-0"`];
const build = path.join(out, 'build');
const built = path.join(build, 'build.json');
const extracted = path.join(out, 'extraction');
const m7Build = path.join(build, 'm7');
const base = path.join(m7Build, 'base/build.json');
const gate = name => path.join(root, `verification/${name}/gate.mjs`);
const artifacts = name => path.join(out, name);
const cli = path.join(m7Build, 'base/executables/cli');
const m6Peer = path.join(m7Build, 'base/executables/peer');
const m5Peer = path.join(m7Build, 'base/executables/m5_peer');
const taskTests = path.join(m7Build, 'base/executables/task_tests');
const resourceTests = path.join(m7Build, 'base/executables/resource_tests');
const authorityTests = path.join(m7Build, 'base/executables/authority_tests');
const m7Peer = path.join(m7Build, 'executables/m7-peer');
const mc2Tests = path.join(m7Build, 'executables/mc2-core-control-tests');
const m8Peer = path.join(build, 'executables/m8-peer');
const common = { PATH: '/run/current-system/sw/bin', LANG: 'C.UTF-8', LC_ALL: 'C.UTF-8',
  HOME: path.join(out, 'home'), TMPDIR: path.join(out, 'tmp'), XDG_RUNTIME_DIR: process.env.XDG_RUNTIME_DIR };
// The reviewed whole-crate discovery records the real Git executable path,
// not the system-profile symlink. Reuse that exact tool identity in `check`.
const extractionGit = fs.realpathSync('/run/current-system/sw/bin/git');
const extractionEnv = { ...common, PATH: `${path.dirname(extractionGit)}:${common.PATH}` };
const nixArgs = ['--option', 'builders', '',
  '--option', 'substituters', 'https://cache.nixos.org https://nix-community.cachix.org',
  '--extra-experimental-features', 'nix-command flakes'];
const cargoEnv = { ...common, PATH: `${rust}:${pins.linker_bin}:/run/current-system/sw/bin`,
  RUSTC_WORKSPACE_WRAPPER: '', CARGO_BUILD_RUSTC_WRAPPER: '',
  RUSTFLAGS: '', CARGO_ENCODED_RUSTFLAGS: '', CARGO_INCREMENTAL: '0',
  CARGO_HOME: path.join(out, 'cargo-home'), CARGO_TARGET_DIR: path.join(out, 'quality-target') };
const m4Env = { ...cargoEnv, CARGO_HOME: path.join(m7Build, 'base/cargo-home'),
  CARGO_TARGET_DIR: path.join(m7Build, 'base/target') };
const proofEnv = { ...common, NOBLE_LEAN: path.join(selection.tool_paths.lean.output, 'bin/lean'),
  NOBLE_BWRAP: '/nix/store/lqndphylsxqwbwm804n473pb4sqb98sh-bubblewrap-0.11.2/bin/bwrap',
  NOBLE_PRLIMIT: '/nix/store/mqvbf0flamqaq9c3ihb496aahag897n0-util-linux-2.42.3-bin/bin/prlimit',
  NOBLE_SYSTEMD_RUN: '/nix/store/baxgs06659w4i6ggsfr56srjrb52v1h8-systemd-261.2/bin/systemd-run' };
const tasks = [
  ['nix-flake-source-prefetch', nix, [...nixArgs, 'flake', 'prefetch', '--json', '.'], common, 300_000],
  ['build', node, [path.join(root, 'verification/m8/build.mjs'), build], common, 3_900_000],
  ['strict-whole-crate-extraction-check', node,
    [path.join(root, 'verification/m4/implementation.mjs'), 'check', extracted], extractionEnv, 7_200_000],
  ['m8-acceptance', node, [gate('m8'), cli, m8Peer, wasmTools, artifacts('m8')], common, 1_200_000],
  ['m7-regression', node, [gate('m7'), cli, m7Peer, wasmTools, artifacts('m7')], common, 1_200_000],
  ['mc1-regression', bun, [gate('mc1'), cli, artifacts('mc1.json')], proofEnv, 3_600_000],
  ['mc2-regression', bun, [gate('mc2'), cli, artifacts('mc2.json')],
    { ...proofEnv, NOBLE_MC2_CORE_TEST_BINARY: mc2Tests }, 7_200_000],
  ['m3-four-configuration-regression', node,
    [path.join(root, 'tools/m3-wasm.mjs'), '--noble', cli, '--out', artifacts('m3')],
    { ...common, NOBLE_M3_NODE: node, NOBLE_M3_WASM_TOOLS: wasmTools, NOBLE_M3_WASM_OPT: wasmOpt }, 3_600_000],
  ['m4-runtime-regression', node, [gate('m4'), cli, artifacts('m4')], m4Env, 1_200_000],
  ['m5-component-regression', node,
    [gate('m5'), cli, m5Peer, resourceTests, authorityTests, artifacts('m5'),
      '--opacity-report', path.join(m7Build, 'm5-opacity/opacity.json')], common, 3_600_000],
  ['m6-component-regression', node,
    [gate('m6'), cli, m6Peer, taskTests, artifacts('m6'), '--build-report', base], common, 3_600_000],
  ['cairn-and-canonical-documents', bun,
    [path.join(root, 'tools/check-specs.mjs'), '--self-test', '--report'], common, 300_000],
  ['cairn-change-validation', cairn, ['validate', '--root', root, '--policy', cairnPolicy], common, 300_000],
  ['rust-all-workspace-tests', cargo,
    ['test', '--workspace', '--all-targets', '--all-features', '--locked', '--offline', ...vendor], cargoEnv, 3_600_000],
  ['rust-all-workspace-clippy', cargo,
    ['clippy', '--workspace', '--all-targets', '--all-features', '--locked', '--offline', ...vendor, '--', '-D', 'warnings'], cargoEnv, 3_600_000],
  ['published-octet-precommit', nix,
    [...nixArgs, 'develop', '-c', 'pre-commit', 'run', 'octet-deny-all', '--all-files'], common, 3_600_000],
  ['complete-nix-flake-check', nix, [...nixArgs, 'flake', 'check', '--keep-going', '-L'], common, 7_200_000],
];
const roots = [['noble_kernel.dataspace.admit_shared_memory', 31],
  ['noble_kernel.dataspace.decide_publication', 32], ['noble_kernel.dataspace.permits', 33]];
const theorems = ['publication_refines', 'permission_refines', 'shared_memory_refines',
  'shared_memory_refused', 'publish_right_required', 'observe_right_required',
  'duplicate_publication_unchanged', 'changed_publication_replaces', 'first_publication_adds']
  .map(name => `M7Syndicate.${name}`);
const exactRoots = rows => Array.isArray(rows) && JSON.stringify(rows.map(row => [row.declaration, row.def_id])) === JSON.stringify(roots);
const strictAudit = m7 => { const refinement = m7?.syndicate_refinement?.audit;
  return exactRoots(m7?.coverage?.implementation_roots) && refinement?.schema === 'noble-m7-syndicate-audit/v1'
    && refinement.result === 'passed' && exactRoots(refinement.implementation_roots)
    && JSON.stringify(refinement.strict_roots?.map(row => row.declaration)) === JSON.stringify(theorems)
    && JSON.stringify(refinement.packet?.strict_roots?.map(row => row.declaration)) === JSON.stringify(theorems); };
function collect(directory) {
  const names = [];
  function visit(relative) {
    for (const entry of fs.readdirSync(path.join(root, relative), { withFileTypes: true })) {
      if (['.git', '.lake', 'target', 'node_modules'].includes(entry.name)) continue;
      const file = `${relative}/${entry.name}`;
      if (entry.isDirectory()) visit(file);
      else if (entry.isFile()) names.push(file);
      else if (relative.startsWith('proofs') && entry.isSymbolicLink()) continue;
      else throw Error(`unreviewed source entry: ${file}`);
    }
  }
  visit(directory);
  return names;
}
function snapshot() {
  const names = new Set(['Cargo.toml', 'Cargo.lock', 'flake.nix', 'flake.lock', 'rust-toolchain.toml',
    '.pre-commit-config.yaml', 'verification/m4/implementation.mjs', 'verification/m4/extraction-lock.json',
    'verification/m6/build.mjs', 'verification/m6/pins.json', 'verification/m7/build.mjs',
    'verification/m7/gate.mjs', 'tools/m3-wasm.mjs', 'tools/check-specs.mjs', 'tools/cairn.mjs',
    'verification/m5/build.mjs', 'verification/m5/gate.mjs', 'verification/m5/core-boundary.mjs',
    'verification/m5/borrow-boundary.rs', 'verification/m5/identity-boundary.rs', 'verification/m5/pins.json',
    'verification/m5/peer/Cargo.toml', 'verification/m5/peer/Cargo.lock',
    'verification/mc2/gate.mjs', 'verification/mc2/runtime-controls.mjs',
    ...collect('verification/mc1').filter(file => /\.(?:mjs|lean|noble-contract)$/.test(file)),
    ...['crates', 'nix', 'proofs', 'verification/m7/peer', 'verification/m8',
      'verification/m5/peer/src', 'verification/mc2/contracts', 'policy', 'specs/conformance',
      '.cairn/specs/safety', '.cairn/specs/wit-wasi', '.cairn/changes/m8-choreography-projection'].flatMap(collect)]);
  return Object.fromEntries([...names].sort().map(file => {
    const candidate = path.join(root, file);
    assert.ok(fs.lstatSync(candidate).isFile(), `not a plain source: ${file}`);
    return [file, { sha256: hash(candidate), bytes: fs.statSync(candidate).size }];
  }));
}
function blockers() {
  const pending = [];
  if (!common.XDG_RUNTIME_DIR || !fs.existsSync(path.join(common.XDG_RUNTIME_DIR, 'bus'))
    || !fs.statSync(path.join(common.XDG_RUNTIME_DIR, 'bus')).isSocket())
    pending.push('MC1/MC2 proofs and pinned Nix fetches require active user XDG_RUNTIME_DIR bus');
  for (const [name, file] of Object.entries(proofEnv).filter(([key]) => key.startsWith('NOBLE_')))
    if (!fs.existsSync(file)) pending.push(`selected proof sandbox tool missing: ${name}=${file}`);
  let lock;
  try { lock = JSON.parse(fs.readFileSync(path.join(root, 'verification/m4/extraction-lock.json'))); }
  catch (error) { return [`missing readable reviewed extraction lock: ${error}`]; }
  if (lock.schema !== 'm4-extraction-lock/v1' || !strictAudit(lock.m7))
    pending.push('reviewed whole-crate lock lacks exact retained M7 compiler roots and nine strict theorems');
  if (lock.tools?.git?.path !== extractionGit || lock.tools.git.sha256 !== hash(extractionGit))
    pending.push('reviewed whole-crate lock Git executable path/SHA differs from selected check environment');
  for (const file of ['proofs/m7/M7Syndicate.lean', 'proofs/m7/M7Audit.lean', 'verification/m7/extraction.mjs'])
    if (!lock.source_files?.[file]) pending.push(`reviewed lock missing ${file}`);
  for (const file of collect('crates')) if (!lock.source_files?.[file])
    pending.push(`reviewed whole-crate lock missing ${file}`);
  return pending;
}
const pending = blockers();
if (mode === 'plan') {
  console.log(JSON.stringify({ schema: 'noble-m8-assurance-plan/v1', phase: 'pre-promotion',
    planned_result: pending.length ? 'blocked' : 'ready', artifact_directory: out, prerequisites: pending,
    commands: tasks.map(([label, executable, argv, environment, timeout_ms]) =>
      ({ label, executable, argv, cwd: root, environment, timeout_ms })),
    next_phase: 'After source-bound case/evidence promotion and Cairn sync/archive, run a distinct M8 final-documents gate.',
    non_claims: ['Plan mode executes no commands.', 'Existing M7 receipt does not satisfy fresh M8 or M7 regression.',
      'The presence of a lock file alone does not establish independent source/proof review.'] }, null, 2));
  process.exit(0);
}
assert.deepEqual(pending, [], 'resolve blockers before creating artifacts');
assert.ok(!fs.existsSync(out), 'fresh assurance artifacts required');
assert.equal(fs.realpathSync(path.dirname(out)), path.dirname(out), 'artifact parent cannot be a symlink');
fs.mkdirSync(out);
for (const directory of ['commands', 'home', 'tmp', 'cargo-home']) fs.mkdirSync(path.join(out, directory));
const receipt = { schema: 'noble-m8-assurance/v1', phase: 'pre-promotion', result: 'running',
  source_root: root, source_files: {}, commands: [], gates: {}, tools: {}, failures: [], integrity_failures: [],
  proof_lock: { path: 'verification/m4/extraction-lock.json', sha256: hash(path.join(root, 'verification/m4/extraction-lock.json')) },
  assumptions: ['The M4 lock and any renewed generated Lean/source inventory require independent review before this run.',
    'Selected compiler, Charon/Aeneas/Lean, Wasmtime/Node, Bun, Octet, Nix and OS remain trusted tools.',
    'Source-bound strict M7 proofs do not establish universal M8 projection, host, compiler, or engine refinement.'],
  non_claims: ['Full choreography, distributed transport/durability, fairness or arbitrary async progress',
    'Historical M7 receipt as fresh M8 or M7 regression', 'Universal compiler/ABI/host refinement'] };
const save = () => fs.writeFileSync(path.join(out, 'assurance.json'), JSON.stringify(receipt, null, 2) + '\n');
function execute(label, executable, argv, environment, timeout_ms) {
  const tool = fs.realpathSync(executable);
  const digest = hash(tool);
  if (!receipt.tools[tool]) receipt.tools[tool] = { sha256: digest };
  assert.equal(receipt.tools[tool].sha256, digest, `${label}: executable changed`);
  const id = receipt.commands.length + 1;
  const stem = `commands/${String(id).padStart(3, '0')}-${label}`;
  const stdout = path.join(out, `${stem}.stdout`), stderr = path.join(out, `${stem}.stderr`);
  const outFd = fs.openSync(stdout, 'wx', 0o600), errFd = fs.openSync(stderr, 'wx', 0o600);
  const row = { label, executable: tool, executable_sha256: digest, argv, cwd: root, environment,
    timeout_ms, stdout: path.relative(out, stdout), stderr: path.relative(out, stderr) };
  receipt.commands.push(row); save();
  let result;
  try { result = spawnSync(tool, argv, { cwd: root, env: environment, stdio: ['ignore', outFd, errFd],
    timeout: timeout_ms, killSignal: 'SIGKILL' }); }
  finally { fs.closeSync(outFd); fs.closeSync(errFd); }
  Object.assign(row, { status: result?.status ?? null, signal: result?.signal ?? null,
    error: result?.error ? String(result.error) : null, stdout_sha256: hash(stdout), stderr_sha256: hash(stderr) });
  for (const file of [stdout, stderr]) fs.chmodSync(file, 0o400);
  row.passed = row.status === 0 && row.signal === null && row.error === null && hash(tool) === digest;
  if (!row.passed) receipt.failures.push({ label, command: id, stderr: row.stderr, status: row.status, error: row.error });
  save();
  return row;
}
function gateReceipt(label, file, schema, acceptable) {
  const observed = JSON.parse(fs.readFileSync(file));
  if (schema === 'm3-resource-free-program-representation-comparison/v1') {
    assert.equal(observed.schema_version, 1);
    assert.equal(observed.scope, schema.slice(0, -3));
  } else assert.equal(observed.schema, schema, `${label}: receipt schema`);
  assert.ok(acceptable(observed), `${label}: full gate receipt did not pass`);
  receipt.gates[label] = { path: path.relative(out, file), sha256: hash(file), schema,
    source_revision: observed.source_revision ?? null };
  return observed;
}
function accept(label) {
  const where = { 'm8-acceptance': artifacts('m8'), 'm7-regression': artifacts('m7'),
    'mc1-regression': artifacts('mc1.json'), 'mc2-regression': artifacts('mc2.json'),
    'm3-four-configuration-regression': artifacts('m3'), 'm4-runtime-regression': artifacts('m4'),
    'm5-component-regression': artifacts('m5'), 'm6-component-regression': artifacts('m6') }[label];
  if (label === 'nix-flake-source-prefetch') {
    const row = receipt.commands.find(command => command.label === label);
    const fetched = JSON.parse(fs.readFileSync(path.join(out, row.stdout)));
    assert.ok(typeof fetched.storePath === 'string' && typeof fetched.hash === 'string' && fetched.hash.startsWith('sha256-'));
    const materialized = fs.realpathSync(fetched.storePath);
    assert.ok(fs.statSync(materialized).isDirectory());
    const critical = Object.keys(receipt.source_files).filter(file => file.startsWith('crates/')
      || file.startsWith('nix/')
      || file.startsWith('proofs/') || file.startsWith('verification/m4/')
      || file.startsWith('verification/m8/') || file.startsWith('.cairn/changes/m8-choreography-projection/')
      || file.startsWith('.cairn/specs/') || file.startsWith('policy/')
      || file.startsWith('verification/m7/peer/') || file === 'Cargo.toml' || file === 'Cargo.lock'
      || file === 'specs/conformance/safety-cases.json' || file === 'specs/conformance/wit-wasi-cases.json');
    assert.ok(critical.length >= 15, 'incomplete critical M8 source selection');
    for (const file of critical) {
      const pathInFlake = path.join(materialized, file);
      assert.ok(fs.statSync(pathInFlake).isFile(), `Nix source missing ${file}`);
      assert.equal(hash(pathInFlake), receipt.source_files[file].sha256, `Nix source differs ${file}`);
    }
    receipt.gates.nix_source = { path: row.stdout, sha256: hash(path.join(out, row.stdout)),
      source: materialized, nar_hash: fetched.hash, critical_files: critical.length };
    return;
  }
  if (label === 'build') {
    const value = gateReceipt(label, built, 'noble-m8-build/v1', row => row.result === 'built'
      && row.integrity_failures?.length === 0 && row.binaries?.m8_peer && row.binaries?.m7_peer);
    for (const [name, binary] of Object.entries(value.binaries))
      assert.equal(hash(binary.path), binary.sha256, `compiled ${name} changed`);
    const m7 = JSON.parse(fs.readFileSync(path.join(m7Build, 'build.json')));
    assert.equal(m7.schema, 'noble-m7-build/v1');
    assert.equal(m7.result, 'built');
    assert.equal(value.base.sha256, hash(path.join(m7Build, 'build.json')));
    return;
  }
  if (label === 'strict-whole-crate-extraction-check') {
    gateReceipt(label, path.join(extracted, 'implementation.json'), 'm4-implementation-evidence/v1', row =>
      row.mode === 'check' && row.result === 'passed' && strictAudit(row.m7)
      && row.m7_refusals?.length >= 8 && row.m7_refusals.every(control => control.result === 'refused'));
    return;
  }
  if (label === 'm8-acceptance' || label === 'm7-regression') {
    const isM8 = label === 'm8-acceptance';
    const value = gateReceipt(label, path.join(where, 'report.json'),
      isM8 ? 'noble-m8-acceptance/v1' : 'noble-m7-acceptance/v1', row => row.result === 'passed'
        && row.summary?.required_cases === (isM8 ? 4 : 7)
        && row.summary?.passed_cases === (isM8 ? 4 : 7)
        && (isM8 ? row.summary?.variants === 38 : row.summary?.variant_count === 16)
        && (!isM8 || (row.summary?.strict_json_edges === 10 && row.summary?.one_round_reservations === 3))
        && row.integrity_failures?.length === 0);
    const source = JSON.parse(fs.readFileSync(built)).sources;
    for (const [file, item] of Object.entries(source)) if (value.sources[file])
      assert.equal(value.sources[file].sha256, item.sha256, `${label}: compiled source differs from gate`);
    for (const name of isM8 ? ['m8_peer', 'cli'] : ['m7_peer', 'cli']) {
      const compiled = JSON.parse(fs.readFileSync(built)).binaries[name];
      const key = name.endsWith('peer') ? 'peer' : name;
      assert.equal(value.executables[key].sha256, compiled.sha256, `${label}: supplied executable differs from build`);
    }
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
  receipt.git_projection = gitSourceProjection(root, Object.keys(receipt.source_files));
  for (const row of receipt.git_projection.rows)
    assert.equal(row.worktree_sha256, receipt.source_files[row.path].sha256, `${row.path}: Git projection differs from snapshot`);
  save();
  for (const task of tasks) {
    const row = execute(...task);
    if (!row.passed) {
      if (['nix-flake-source-prefetch', 'build'].includes(task[0])) break;
      continue;
    }
    try { accept(task[0]); }
    catch (error) { receipt.failures.push({ label: task[0], failure: String(error.stack ?? error) }); }
    save();
    if (['nix-flake-source-prefetch', 'build'].includes(task[0]) && receipt.failures.length) break;
  }
  assert.deepEqual(snapshot(), receipt.source_files, 'source/configuration changed during assurance');
  assert.deepEqual(gitSourceProjection(root, Object.keys(receipt.source_files)), receipt.git_projection,
    'staged/worktree Git projection changed during assurance');
  assert.equal(hash(path.join(root, 'verification/m4/extraction-lock.json')), receipt.proof_lock.sha256);
  for (const [file, value] of Object.entries(receipt.tools)) assert.equal(hash(file), value.sha256, `tool changed: ${file}`);
  for (const row of receipt.commands) for (const name of ['stdout', 'stderr'])
    assert.equal(hash(path.join(out, row[name])), row[`${name}_sha256`], `transcript changed: ${row.label}`);
  for (const value of Object.values(receipt.gates)) assert.equal(hash(path.join(out, value.path)), value.sha256);
} catch (error) { receipt.integrity_failures.push(String(error.stack ?? error)); }
receipt.result = receipt.failures.length === 0 && receipt.integrity_failures.length === 0
  && receipt.commands.length === tasks.length && Object.keys(receipt.gates).length === 11 ? 'passed' : 'failed';
save();
console.log(JSON.stringify({ schema: receipt.schema, result: receipt.result, report: path.join(out, 'assurance.json'),
  executed_commands: receipt.commands.length, verified_gates: Object.keys(receipt.gates),
  failures: receipt.failures, integrity_failures: receipt.integrity_failures }));
process.exitCode = receipt.result === 'passed' ? 0 : 1;
