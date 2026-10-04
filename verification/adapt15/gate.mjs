#!/usr/bin/env node
// Prepromotion-only ADAPT-15 gate. A plan never builds; execution requires an
// independently reviewed source revision and a fresh directory outside this repo.
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const root = fs.realpathSync(path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..'));
const sha = value => crypto.createHash('sha256').update(value).digest('hex');
const read = file => fs.readFileSync(path.join(root, file));
const caseFile = 'specs/conformance/adaptation-cases.json';
const canonical = JSON.parse(read(caseFile));
const rows = canonical.cases.filter(row => row.id === 'ADAPT-15');
assert.equal(rows.length, 1, 'one canonical ADAPT-15 case');
const design = rows[0];
assert.deepEqual({ profile: design.profile, kind: design.kind,
  requirements: design.requirements, input: design.input, expected: design.expected }, {
  profile: 'Core-Bootstrap', kind: 'admission', requirements: ['VC-ADMIT-05', 'S-DECODE-01'],
  input: { harness: 'mandatory-admission-check-all-build-modes',
    candidate: 'layout-valid-forged-empty-effects-for-test.emit', source: '"audit" test.emit',
    input_stack: [], output_stack: [],
    host_contracts: { 'test.emit': 'Text -- ! {test.emit}' },
    non_effect_derivations: 'valid', advertised_effects: [], variants: [
      { build: 'debug', optional_proof: 'absent', outcome: 'reject' },
      { build: 'release', optional_proof: 'absent', outcome: 'reject' },
      { build: 'debug', optional_proof: 'unrelated-valid-proof', outcome: 'reject' },
      { build: 'release', optional_proof: 'unrelated-valid-proof', outcome: 'reject' },
    ] },
  expected: { stage: 'acceptance', outcome: 'reject-in-all-modes',
    accepted_program_created: false, guest_requests: 0, protected_operations: 0 },
});
assert.deepEqual(design.state, { implementation: 'absent', execution: 'not-run',
  proof: 'open', trust: 'unassessed' });
assert.deepEqual(design.evidence, [], 'gate must precede canonical promotion');
const selected = JSON.parse(read('policy/tool-selection.json'));
const rust = path.join(selected.tool_paths.quality_rust.output, 'bin');
const lean = path.join(selected.tool_paths.lean.output, 'bin/lean');
const linker = selected.component_sync.linker_bin;
const node = path.join(selected.tool_paths.node.output, 'bin/node');
assert.equal(JSON.parse(read('crates/noble-cli/src/core/runtime/config.json')).tools.node.path,
  node, 'production CLI must use selected host Node');

// Retain the source closure rather than a label for a binary built elsewhere.
const sourcePaths = new Set();
const add = file => { assert.ok(!path.isAbsolute(file)); sourcePaths.add(file); };
function tree(directory) {
  for (const entry of fs.readdirSync(path.join(root, directory), { withFileTypes: true })
    .sort((left, right) => left.name.localeCompare(right.name))) {
    if (directory === 'proofs/mc1' && entry.name === '.lake') continue;
    const file = `${directory}/${entry.name}`;
    assert.equal(entry.isSymbolicLink(), false, `unbound source symlink: ${file}`);
    if (entry.isDirectory()) tree(file);
    else { assert.equal(entry.isFile(), true, `not a source file: ${file}`); add(file); }
  }
}
for (const directory of ['crates/noble-kernel/src', 'crates/noble-contracts/src',
  'crates/noble-wasm/src', 'crates/noble-wasm/runtime', 'crates/noble-wasm/wit',
  'crates/noble-cli/src', 'crates/noble-syndicate/src', 'proofs/mc1']) tree(directory);
const activeChanges = ['safety-callback-returned-owner',
  'safety-recursive-resource-eligibility', 'safety-authorized-fs-read'];
const archived = fs.readdirSync(path.join(root, '.cairn/archive'));
for (const name of activeChanges) {
  assert.ok(!archived.some(entry => entry.endsWith(`-${name}`)),
    `${name}: this source freeze is for the ACTIVE change, not an archive`);
  tree(`.cairn/changes/${name}`);
}
const nativeSpecs = {
  native_program_contracts: '.cairn/specs/program-contracts/spec.md',
  native_safety: '.cairn/specs/safety/spec.md',
  native_core_bootstrap: '.cairn/specs/core-bootstrap/spec.md',
  native_evidence: '.cairn/specs/evidence/spec.md',
  native_resource_adapters: '.cairn/specs/resource-adapters/spec.md',
};
const historicalFiles = [
  'verification/adapt06/acceptance.json',
  'verification/mc1/acceptance.json',
  'verification/safety-core/acceptance.json',
  'verification/scase04/acceptance.json',
  'verification/scase04-current-source/acceptance.json',
  'verification/scase04-current-source/dx06-acceptance.json',
  'verification/scase06/final-acceptance.json',
  'verification/scase06-current-source/acceptance.json',
  'verification/scase06-current-source/dx06-acceptance.json',
  'verification/scase08/acceptance.json',
  'verification/scase08-current-source/acceptance.json',
  'verification/scase08-current-source/dx06-acceptance.json',
];
for (const file of [
  'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', 'policy/tool-selection.json',
  ...['noble-kernel', 'noble-contracts', 'noble-wasm', 'noble-cli', 'noble-syndicate']
    .map(name => `crates/${name}/Cargo.toml`),
  caseFile, 'specs/conformance/safety-cases.json',
  'verification/mc1/increment.noble-contract',
  'verification/mc1/increment-proof.lean',
  ...Object.values(nativeSpecs), ...historicalFiles,
  'verification/adapt15/check.rs', 'verification/adapt15/postpromotion.mjs',
  'verification/adapt15/gate.mjs',
]) add(file);
const source_sha256 = Object.fromEntries([...sourcePaths].sort()
  .map(file => [file, sha(read(file))]));
const source_revision = `sha256:${sha(JSON.stringify(Object.entries(source_sha256).sort()))}`;
const historical_receipt_sha256 = Object.fromEntries(historicalFiles.map(file =>
  [file, source_sha256[file]]));
const safety = JSON.parse(read('specs/conformance/safety-cases.json'));
for (const [id, file] of [
  ['S-CASE-04', 'verification/scase04/acceptance.json'],
  ['S-CASE-06', 'verification/scase06/final-acceptance.json'],
  ['S-CASE-08', 'verification/scase08/acceptance.json'],
]) {
  const record = safety.cases.filter(row => row.id === id);
  assert.equal(record.length, 1, `${id}: one historical case`);
  assert.equal(record[0].evidence.at(-1).configuration.receipt_sha256,
    historical_receipt_sha256[file], `${id}: historical receipt differs from canonical evidence`);
  assert.equal(JSON.parse(read(file)).result, 'passed', `${id}: historical acceptance`);
}
const adapt06 = canonical.cases.filter(row => row.id === 'ADAPT-06');
assert.equal(adapt06.length, 1);
assert.equal(adapt06[0].evidence[0].configuration.receipt_sha256,
  historical_receipt_sha256['verification/adapt06/acceptance.json']);
const prepromotion_case_sha256 = source_sha256[caseFile];
const caseRecord = Object.fromEntries(['id', 'profile', 'kind', 'requirements',
  'input', 'expected'].map(key => [key, design[key]]));
if (process.argv.length === 3 && process.argv[2] === '--plan') {
  console.log(JSON.stringify({ result: 'ready-for-owner-review-not-execution',
    case_id: design.id, source_revision, source_files: Object.keys(source_sha256).length,
    prepromotion_case_sha256, selected_rust: selected.tool_paths.quality_rust,
    cells: design.input.variants,
    requires: ['separately reviewed source freeze', 'fresh external output directory',
      'selected proof verifier and active user sandbox services'] }));
  process.exit(0);
}
assert.equal(process.argv.length, 4,
  'usage: node verification/adapt15/gate.mjs --plan | NEW_EXTERNAL_DIR sha256:REVIEWED_SOURCE_REVISION');
assert.match(process.argv[3], /^sha256:[0-9a-f]{64}$/);
assert.equal(process.argv[3], source_revision, 'source changed from separately reviewed freeze');
const external = path.resolve(process.argv[2]);
assert.ok(external !== root && !external.startsWith(`${root}${path.sep}`),
  'acceptance output must be external to the repository');
fs.mkdirSync(external); // Existing output, including any prior receipt, must fail closed.
for (const directory of ['home', 'tmp', 'inputs', 'snapshots', 'build-debug', 'build-release'])
  fs.mkdirSync(path.join(external, directory));
const retain = (relative, data) => {
  const output = path.join(external, relative);
  fs.mkdirSync(path.dirname(output), { recursive: true });
  fs.writeFileSync(output, data, { flag: 'wx' });
  return { path: relative, sha256: sha(data) };
};
const prepromotion_inputs = {
  case: retain('snapshots/adaptation-cases.prepromotion.json', read(caseFile)),
  ...Object.fromEntries(Object.entries(nativeSpecs).map(([name, file]) =>
    [name, retain(`snapshots/${name}.md`, read(file))])),
};
for (const name of activeChanges) {
  for (const file of sourcePaths) if (file.startsWith(`.cairn/changes/${name}/`)) {
    const key = file.slice('.cairn/changes/'.length).replaceAll('/', '_').replaceAll('.', '_');
    prepromotion_inputs[`active_${key}`] = retain(`snapshots/${file}`, read(file));
  }
}
assert.equal(prepromotion_inputs.case.sha256, prepromotion_case_sha256);
const source = retain('inputs/adapt15-source.noble', Buffer.from(design.input.source, 'utf8'));
const session = retain('inputs/authentic-session.stdin',
  Buffer.from(`${design.input.source}\n`, 'utf8'));
// The real MC1 contract and proof are separate exact-input fixtures. They
// concern pure increment, not the effectful test.emit candidate under test.
const mc1Contract = retain('inputs/unrelated-pure-mc1.noble-contract',
  read('verification/mc1/increment.noble-contract'));
const mc1Proof = retain('inputs/unrelated-pure-mc1-proof.lean',
  read('verification/mc1/increment-proof.lean'));
assert.notEqual(mc1Contract.sha256, source.sha256,
  'an effectful test.emit candidate is not the unrelated Pure proof subject');
const input_sha256 = { source, session, mc1_contract: mc1Contract, mc1_proof: mc1Proof };

const baseEnvironment = {
  HOME: path.join(external, 'home'), TMPDIR: path.join(external, 'tmp'),
  CARGO_HOME: path.join(path.dirname(root), '..', '.cargo'),
  CARGO_BUILD_JOBS: '4', NIX_CONFIG: 'min-free = 0',
  CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER: path.join(linker, 'cc'),
  RUSTC_WRAPPER: '', RUSTC_WORKSPACE_WRAPPER: '', RUSTFLAGS: '',
  CARGO_ENCODED_RUSTFLAGS: '', LANG: 'C.UTF-8', LC_ALL: 'C.UTF-8',
  PATH: `${rust}:${linker}:/run/current-system/sw/bin`,
  XDG_RUNTIME_DIR: process.env.XDG_RUNTIME_DIR ?? '',
};
assert.equal(fs.realpathSync(baseEnvironment.CARGO_HOME), '/home/brittonr/.cargo',
  'the pinned offline Cargo registry must be present');
const proofTools = {
  NOBLE_LEAN: lean,
  NOBLE_BWRAP: '/nix/store/y0ra9qr3rz81d9wl7dfrldv3j25dc98q-bubblewrap-0.12.0/bin/bwrap',
  NOBLE_PRLIMIT: '/nix/store/mqvbf0flamqaq9c3ihb496aahag897n0-util-linux-2.42.3-bin/bin/prlimit',
  NOBLE_SYSTEMD_RUN: '/nix/store/baxgs06659w4i6ggsfr56srjrb52v1h8-systemd-261.2/bin/systemd-run',
  NOBLE_NIX_STORE: '/run/current-system/sw/bin/nix-store',
};
const proof_tool_sha256 = Object.fromEntries(Object.entries(proofTools).map(([name, file]) =>
  [name, { path: file, sha256: sha(fs.readFileSync(file)) }]));
function compilerInputHash(file) {
  const stat = fs.statSync(file);
  if (stat.isFile()) return sha(fs.readFileSync(file));
  assert.equal(stat.isDirectory(), true, `not a regular compiler input: ${file}`);
  return sha(JSON.stringify(fs.readdirSync(file).sort().map(name =>
    [name, compilerInputHash(path.join(file, name))])));
}
const commands = [];
function run(label, executable, args, { profile, expected_status = 0, stdin,
  proof = false, timeout = 900_000 } = {}) {
  const env = { ...baseEnvironment,
    CARGO_TARGET_DIR: path.join(external, `build-${profile ?? 'debug'}`, 'target'),
    ...(proof ? { ...proofTools, NOBLE_CONTRACT_LIBRARY: path.join(root, 'proofs/mc1') } : {}) };
  const response = spawnSync(executable, args, { cwd: root, env, input: stdin,
    maxBuffer: 64 * 1024 * 1024, timeout });
  const stem = `${String(commands.length).padStart(2, '0')}-${label}`;
  const stdout = response.stdout ?? Buffer.alloc(0), stderr = response.stderr ?? Buffer.alloc(0);
  const capturedOut = retain(`${stem}.stdout`, stdout);
  const capturedErr = retain(`${stem}.stderr`, stderr);
  const row = { label, executable, executable_sha256: sha(fs.readFileSync(executable)),
    args, expected_status, status: response.status, signal: response.signal,
    error: response.error?.message ?? null, stdin_sha256: stdin === undefined ? null : sha(stdin),
    stdout: capturedOut.path, stderr: capturedErr.path,
    stdout_sha256: capturedOut.sha256, stderr_sha256: capturedErr.sha256 };
  commands.push(row);
  assert.equal(row.error, null, `${label}: process could not start`);
  assert.equal(row.signal, null, `${label}: process was interrupted`);
  assert.equal(row.status, expected_status,
    `${label}: expected exit ${expected_status}: ${stderr.toString('utf8')} ${stdout.toString('utf8')}`);
  return stdout.toString('utf8');
}
const rustVersion = run('rustc-version', path.join(rust, 'rustc'), ['-vV']);
const cargoVersion = run('cargo-version', path.join(rust, 'cargo'), ['-vV']);
const nodeVersion = run('node-version', node, ['--version']);
const leanVersion = run('lean-version', lean, ['--version']);
assert.match(rustVersion, /release: 1\.96\.0-nightly/);
assert.match(cargoVersion, /cargo 1\.96\.0-nightly/);
assert.match(nodeVersion, /^v24\.13\.0\s*$/);
assert.match(leanVersion, /Lean \(version 4\.31\.0,/);
const cargoArtifacts = output => output.split('\n').filter(line => line.startsWith('{'))
  .map(line => JSON.parse(line)).filter(row => row.reason === 'compiler-artifact');
function artifact(built, name, kind, profile) {
  const matches = built.filter(row => row.target.name === name && row.target.kind.includes(kind));
  assert.equal(matches.length, 1, `${profile}: one compiled ${name}/${kind}`);
  const match = matches[0];
  assert.equal(match.profile.opt_level, profile === 'debug' ? '0' : '3',
    `${profile}: actual Cargo optimization profile`);
  assert.equal(match.profile.debug_assertions, profile === 'debug',
    `${profile}: actual Cargo debug assertions`);
  assert.equal(match.profile.overflow_checks, true, `${profile}: actual Cargo overflow policy`);
  const files = kind === 'bin' ? [match.executable] :
    match.filenames.filter(file => file.endsWith('.rlib'));
  assert.equal(files.length, 1, `${profile}: one executed artifact for ${name}`);
  const file = files[0];
  assert.ok(file.startsWith(`${path.join(external, `build-${profile}`, 'target',
    profile === 'debug' ? 'debug' : 'release')}${path.sep}`),
  `${profile}: artifact outside the real Cargo profile output`);
  return file;
}
const binaries = {};
const dep_info = {};
for (const profile of ['debug', 'release']) {
  const args = ['build', '-p', 'noble-cli', '-p', 'noble-wasm', '--all-features',
    '--locked', '--offline', '-j', '4', '--message-format=json',
    ...(profile === 'release' ? ['--profile', 'release'] : [])];
  const built = cargoArtifacts(run(`cargo-build-${profile}`, path.join(rust, 'cargo'), args,
    { profile, timeout: 1_800_000 }));
  const cli = artifact(built, 'noble', 'bin', profile);
  const libraries = Object.fromEntries(['noble_contracts', 'noble_kernel', 'noble_wasm']
    .map(name => [name, artifact(built, name, 'lib', profile)]));
  const checker = path.join(external, `build-${profile}`, 'adapt15-check');
  const checkerProfileFlags = ['-C', `opt-level=${profile === 'debug' ? '0' : '3'}`,
    '-C', `debug-assertions=${profile === 'debug' ? 'yes' : 'no'}`,
    '-C', 'overflow-checks=yes', '-C', 'panic=unwind'];
  const cfg = run(`rustc-profile-${profile}`, path.join(rust, 'rustc'),
    ['--print', 'cfg', ...checkerProfileFlags], { profile });
  assert.equal(cfg.split('\n').includes('debug_assertions'), profile === 'debug',
    `${profile}: independently observed rustc debug_assertions cfg`);
  run(`compile-check-${profile}`, path.join(rust, 'rustc'), [
    '--edition=2021', '--crate-name', 'adapt15_check', 'verification/adapt15/check.rs',
    `--emit=dep-info=${checker}.d,link=${checker}`,
    ...checkerProfileFlags,
    ...Object.entries(libraries).flatMap(([name, file]) => ['--extern', `${name}=${file}`]),
    '-L', `dependency=${path.join(path.dirname(cli), 'deps')}`,
  ], { profile });
  const identity = file => ({ path: file, sha256: sha(fs.readFileSync(file)) });
  binaries[profile] = { cli: identity(cli), checker: identity(checker),
    libraries: Object.fromEntries(Object.entries(libraries).map(([name, file]) =>
      [name, identity(file)])), cargo_profile: profile,
    opt_level: profile === 'debug' ? '0' : '3' };
  // Every first-party compiler input is included in the pre-build source map.
  // Cargo also names generated dependency inputs under its external target;
  // retain their exact bytes rather than pretending they were repository files.
  const generatedInputs = new Map();
  const immutableInputs = new Map();
  const dependencies = file => {
    const found = [];
    const dep = fs.readFileSync(file, 'utf8').replace(/\\\r?\n/g, ' ');
    for (const line of dep.split('\n')) {
      const separator = line.indexOf(': ');
      if (line.startsWith('#') || separator < 0) continue;
      for (const entry of line.slice(separator + 2).split(/(?<!\\) /).filter(Boolean)) {
        const absolute = path.resolve(root, entry.replaceAll('\\ ', ''));
        if (!absolute.startsWith(`${root}${path.sep}`)) {
          const target = path.join(external, `build-${profile}`, 'target');
          if (absolute.startsWith('/nix/store/') &&
            fs.realpathSync(absolute).startsWith('/nix/store/')) {
            immutableInputs.set(absolute, compilerInputHash(absolute));
            continue;
          }
          assert.ok(absolute.startsWith(`${target}${path.sep}`) &&
            fs.realpathSync(absolute).startsWith(`${target}${path.sep}`) &&
            /^(?:debug|release)\/build\/[^/]+\/out\/.+$/u.test(
              path.relative(target, absolute).split(path.sep).join('/')),
          `${profile}: unbound external compiler input: ${entry}`);
          generatedInputs.set(absolute, compilerInputHash(absolute));
          continue;
        }
        const relative = path.relative(root, absolute).split(path.sep).join('/');
        assert.equal(sha(read(relative)), source_sha256[relative],
          `${profile}: unhashed or mutated compiler input: ${relative}`);
        found.push(relative);
      }
    }
    return found;
  };
  const depFiles = [`${cli}.d`, ...Object.values(libraries).map(file =>
    path.join(path.dirname(file),
      (path.basename(path.dirname(file)) === 'deps'
        ? path.basename(file).replace(/^lib/u, '') : path.basename(file))
        .replace(/\.rlib$/u, '.d'))), `${checker}.d`];
  const inputs = [...new Set(depFiles.flatMap(dependencies))].sort();
  for (const file of ['verification/adapt15/check.rs',
    'crates/noble-contracts/src/lib.rs', 'crates/noble-kernel/src/lib.rs',
    'crates/noble-wasm/src/lib.rs', 'crates/noble-cli/src/main.rs'])
    assert.ok(inputs.includes(file), `${profile}: missing first-party compiler input ${file}`);
  dep_info[profile] = {
    files: depFiles.map(identity), inputs,
    source_sha256: Object.fromEntries(inputs.map(file => [file, source_sha256[file]])),
    generated_inputs_sha256: Object.fromEntries([...generatedInputs].sort()),
    immutable_inputs_sha256: Object.fromEntries([...immutableInputs].sort()),
  };
}
assert.notEqual(binaries.debug.cli.sha256, binaries.release.cli.sha256,
  'debug and optimized release must not be the same production binary');
assert.notEqual(binaries.debug.checker.sha256, binaries.release.checker.sha256,
  'debug and optimized release must not be the same verifier binary');
for (const name of ['noble_contracts', 'noble_kernel', 'noble_wasm'])
  assert.notEqual(binaries.debug.libraries[name].sha256, binaries.release.libraries[name].sha256,
    `debug and release ${name} are identical`);

// The real compiled CLI accepts and runs the unchanged, correct-effects source
// in each actual Cargo mode. This is a positive control, not the hostile bound.
const positive = {};
for (const profile of ['debug', 'release']) {
  const result = run(`cli-authentic-${profile}`, binaries[profile].cli.path, ['session'],
    { profile, stdin: readExternal(session.path) });
  const lines = result.trim().split('\n');
  assert.equal(lines.length, 1, `${profile}: one authentic CLI observation`);
  const report = JSON.parse(lines[0]);
  assert.deepEqual({ profile: report.profile, stage: report.stage, outcome: report.outcome,
    stack: report.stack, request_trace: report.request_trace,
    guest_requests: report.guest_requests, protected_operations: report.protected_operations },
  { profile: 'Core-Bootstrap', stage: 'wasm', outcome: 'normal', stack: [],
    request_trace: ['test.emit:audit'], guest_requests: 1, protected_operations: 0 });
  assert.equal(report.module.source_sha256, session.sha256,
    `${profile}: production CLI ran a different source`);
  positive[profile] = report;
}
function readExternal(relative) { return fs.readFileSync(path.join(external, relative)); }

// Both genuinely compiled production CLIs independently recheck the same
// exact MC1 proof. Their reports have no authority over the kernel request.
const proofReports = {};
for (const profile of ['debug', 'release']) {
  const label = profile === 'debug' ? 'verify-unrelated-proof' : 'verify-unrelated-proof-release';
  const report = JSON.parse(run(label, binaries[profile].cli.path,
    ['verify', path.join(external, mc1Contract.path), '--proof',
      path.join(external, mc1Proof.path), '--timeout-ms', '600000'],
    { profile, proof: true, timeout: 900_000 }).trim());
  assert.deepEqual({ schema: report.schema, command: report.command,
    outcome: report.outcome, independent_recheck: report.independent_recheck,
    source_path: report.source_path, source: report.source,
    theorem: report.theorem, ordinary_typing: report.ordinary_typing.outcome,
    subject: report.subject.name, implementation_refinement: report.implementation_refinement.status,
    backend_correspondence: report.backend_correspondence.status }, {
    schema: 'noble-mc1-report/v1', command: 'verify', outcome: 'proved',
    independent_recheck: true, source_path: path.join(external, mc1Contract.path),
    source: readExternal(mc1Contract.path).toString('utf8'), theorem: 'MC1Proof.proof',
    ordinary_typing: 'accepted', subject: 'increment',
    implementation_refinement: 'not-checked-by-this-command',
    backend_correspondence: 'not-claimed' });
  assert.deepEqual(report.diagnostics, []);
  assert.deepEqual(report.subject.inputs, [{ name: 'x', type: 'I64' }]);
  assert.deepEqual(report.subject.outputs, [{ name: 'y', type: 'I64' }]);
  assert.match(report.subject.acceptance_request, /allowed_effects: EffSet\(\[\]\)/,
    `${profile}: theorem subject must have an empty effect set`);
  assert.match(report.subject.accepted_candidate, /Definition\(4\)/);
  assert.doesNotMatch(report.subject.accepted_candidate, /Definition\(22\)/);
  assert.deepEqual({ theorem: report.proof_request.theorem,
    expected_type: report.proof_request.expected_type },
  { theorem: 'MC1Proof.proof', expected_type: 'MC1Obligation.claim' });
  assert.equal(report.consumer_policy.lean_toolchain, selected.lean.toolchain);
  assert.equal(report.consumer_policy.lean_commit, selected.lean.rev);
  for (const [key, tool] of [['lean', 'NOBLE_LEAN'], ['bubblewrap', 'NOBLE_BWRAP'],
    ['prlimit', 'NOBLE_PRLIMIT'], ['systemd_run', 'NOBLE_SYSTEMD_RUN']])
    assert.equal(report.consumer_policy.trusted_tools[key],
      fs.realpathSync(proofTools[tool]), `${profile}: unselected proof tool ${key}`);
  assert.ok(!report.generated_statement.includes('test.emit') &&
    !report.generated_statement.includes('audit') &&
    !report.generated_statement.includes('.word 22'),
    `${profile}: the MC1 theorem must not concern the effectful candidate`);
  assert.match(report.generated_statement, /def node_1 : Op := \.word 4/,
    `${profile}: expected independently checked pure increment candidate`);
  assert.deepEqual(report.assumptions.accepted_transitive_axioms.filter(name =>
    !['propext', 'Classical.choice', 'Quot.sound'].includes(name)), [],
  `${profile}: disallowed theorem axiom`);
  proofReports[profile] = report;
}
const proof = { validated: true, subject_matches_forged: false,
  attached_to_kernel_admission: false,
  attachment_api: 'none: noble_kernel::acceptance::check takes only environment, request and candidate',
  contract_sha256: mc1Contract.sha256, proof_source_sha256: mc1Proof.sha256,
  candidate_sha256: source.sha256, subject: 'increment', theorem: 'MC1Proof.proof',
  verifier_sha256: Object.fromEntries(['debug', 'release']
    .map(profile => [profile, binaries[profile].cli.sha256])) };

const observations = {};
const cells = [];
const checker_stdout = {};
for (const variant of design.input.variants) {
  const { build, optional_proof } = variant;
  assert.equal(variant.outcome, 'reject');
  const label = `check-${build}-${optional_proof}`;
  const report = JSON.parse(run(label,
    binaries[build].checker.path, [path.join(external, source.path)],
    { profile: build }).trim());
  assert.deepEqual({ id: report.id, source: report.source },
    { id: design.id, source: design.input.source });
  assert.deepEqual(report.premise, { source_derived: true, definitions: 0,
    candidate_nodes: 2, candidate_body: [0, 1], input_stack: [], output_stack: [],
    allowed_effects: ['test.emit'], host: { definition: 22, behavior: 'TestEmit',
      stack_in: ['StackVar(0)', 'Text'], stack_out: ['StackVar(0)'],
      effects: ['test.emit'] },
    invocations: [{ definition: 22, behavior: 'TestEmit' }], texts: ['audit'] });
  assert.deepEqual(report.authentic.kernel, { outcome: 'Accepted', stack_in: [],
    stack_out: [], effects: ['test.emit'] });
  assert.equal(report.authentic.compiler.outcome, 'prepared');
  assert.ok(Number.isSafeInteger(report.authentic.compiler.wat_bytes) &&
    report.authentic.compiler.wat_bytes > 0, `${build}: authentic candidate is compiled`);
  assert.deepEqual(report.forged, { mutations: ['request.expected.allowed_effects=[]'],
    allowed_effects: [], environment_unchanged: true, candidate_unchanged: true,
    non_effect_premises_unchanged: true,
    kernel: { outcome: 'Invalid', constraint: 'EffectInclusion(test.emit)', node: null,
      definition: null, expected: [], actual: [], provenance_available: false,
      truncated: false },
    compiler: { outcome: 'Invalid', wat_bytes: null }, accepted_program_created: false });
  assert.deepEqual({ guest_created: report.guest_created,
    guest_requests: report.guest_requests, protected_operations: report.protected_operations },
  { guest_created: false, guest_requests: 0, protected_operations: 0 });
  const raw = commands.at(-1).stdout_sha256;
  if (optional_proof === 'absent') checker_stdout[build] = raw;
  else assert.equal(raw, checker_stdout[build],
    `${build}: adding a separately checked unrelated proof changed kernel rejection bytes`);
  cells.push({ build, optional_proof, stage: 'acceptance', outcome: 'reject',
    constraint: report.forged.kernel.constraint,
    accepted_program_created: report.forged.accepted_program_created,
    guest_requests: report.guest_requests,
    protected_operations: report.protected_operations,
    proof_checked: optional_proof === 'unrelated-valid-proof' &&
      proofReports[build].independent_recheck, checker_stdout_sha256: raw });
  observations[`${build}/${optional_proof}`] = report;
}
assert.deepEqual(cells.map(({ build, optional_proof, outcome }) => ({ build, optional_proof, outcome })),
  design.input.variants);
for (const [file, digest] of Object.entries(source_sha256))
  assert.equal(sha(read(file)), digest, `source changed during the gate: ${file}`);
for (const [file, digest] of Object.entries(historical_receipt_sha256))
  assert.equal(sha(read(file)), digest, `historical acceptance changed: ${file}`);
for (const [name, item] of Object.entries(prepromotion_inputs))
  assert.equal(sha(readExternal(item.path)), item.sha256,
    `prepromotion snapshot changed during the gate: ${name}`);
for (const [name, item] of Object.entries(input_sha256))
  assert.equal(sha(readExternal(item.path)), item.sha256,
    `fixture changed during the gate: ${name}`);
for (const command of commands) {
  assert.equal(sha(fs.readFileSync(command.executable)), command.executable_sha256,
    `${command.label}: executable changed during the gate`);
  assert.equal(sha(readExternal(command.stdout)), command.stdout_sha256,
    `${command.label}: raw stdout changed`);
  assert.equal(sha(readExternal(command.stderr)), command.stderr_sha256,
    `${command.label}: raw stderr changed`);
}
for (const build of ['debug', 'release']) {
  for (const item of [binaries[build].cli, binaries[build].checker,
    ...Object.values(binaries[build].libraries), ...dep_info[build].files])
    assert.equal(sha(fs.readFileSync(item.path)), item.sha256,
      `${build}: compiled artifact changed during the gate`);
  for (const [file, digest] of Object.entries(dep_info[build].generated_inputs_sha256))
    assert.equal(compilerInputHash(file), digest,
      `${build}: generated compiler input changed during the gate`);
  for (const [file, digest] of Object.entries(dep_info[build].immutable_inputs_sha256))
    assert.equal(compilerInputHash(file), digest,
      `${build}: Nix-store compiler input changed during the gate`);
}
for (const [name, item] of Object.entries(proof_tool_sha256))
  assert.equal(sha(fs.readFileSync(item.path)), item.sha256,
    `proof tool changed during the gate: ${name}`);
const receipt = {
  schema: 'noble-adapt15-mandatory-admission/v1', result: 'passed', kind: 'test',
  case: caseRecord, claim: 'Source-derived authentic test.emit succeeds in both actual Cargo profiles; the only hostile mutation empties the admission request effect bound and the production kernel and Wasm compiler reject in all four cells. Both production CLIs independently verify the unrelated Pure MC1 increment proof; it is not attached and cannot authorize that effect mismatch. No guest is constructed for the negative submissions; no universal checker or host proof is claimed.',
  source_revision, source_revision_algorithm: 'sha256(JSON.stringify(Object.entries(source_sha256).sort()))',
  source_sha256, historical_receipt_sha256, prepromotion_case_sha256,
  prepromotion_inputs, input_sha256, external_raw_output: external,
  selected_tools: { rust: selected.tool_paths.quality_rust,
    lean: selected.tool_paths.lean, node: selected.tool_paths.node,
    rustc_version: rustVersion.trim(), cargo_version: cargoVersion.trim(),
    node_version: nodeVersion.trim(), lean_version: leanVersion.trim(),
    proof_tool_sha256 },
  environment: { CARGO_BUILD_JOBS: baseEnvironment.CARGO_BUILD_JOBS,
    NIX_CONFIG: baseEnvironment.NIX_CONFIG, linker: baseEnvironment.CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER,
    cargo_home: baseEnvironment.CARGO_HOME,
    separate_targets: { debug: path.join(external, 'build-debug/target'),
      release: path.join(external, 'build-release/target') } },
  binaries, dep_info, commands, proof, proof_reports: proofReports, positive, observations, cells,
  failures: [], integrity_failures: [],
  assumptions: [
    'These are four finite selected-source observations, not universal checker, host, or backend refinement proofs.',
    'The standalone verified pure increment MC1 contract proof has a distinct source and subject and is not attached to or consulted by production kernel admission; the kernel has no proof attachment parameter.',
    'Zero guest requests and protected operations on hostile paths are structural: source-derived independent peer returns Invalid before any WAT/engine/guest/host construction.',
  ],
};
const receiptPath = path.join(external, 'acceptance.json');
fs.writeFileSync(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`, { flag: 'wx' });
console.log(JSON.stringify({ result: 'passed', case_id: design.id, receipt: receiptPath,
  receipt_sha256: sha(fs.readFileSync(receiptPath)), source_revision,
  prepromotion_case_sha256, cells: cells.length, proof: 'independently-verified-unrelated-only',
  commands: commands.length, failures: 0, integrity_failures: 0 }));
