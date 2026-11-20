#!/usr/bin/env node
// Independent owner-law proof-required release gate (VC-OWNER-01, CONTRACT-16).
//
// The gate builds the production CLI from the committed clean tree, emits the
// two honest artifacts, runs the six canonical CONTRACT-16 variants against
// the frozen owner-law records, and retains every command, exit code, output
// hash, law digest, tool and source-manifest hash. A passed receipt requires
// every variant's observed outcome to equal the canonical case's expectation.
//
// Usage: node verification/owner-law/gate.mjs FRESH_OUTPUT_DIRECTORY
//
// The output directory must be outside the repository and must not exist yet.
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const root = fs.realpathSync(
  path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..'));
assert.equal(process.argv.length, 3,
  'usage: node verification/owner-law/gate.mjs FRESH_OUTPUT_DIRECTORY');
const output = path.resolve(process.argv[2]);
assert.ok(output !== root && !output.startsWith(`${root}${path.sep}`),
  'raw acceptance output must be outside the repository');
assert.ok(!fs.existsSync(output), 'the output directory must be fresh');
fs.mkdirSync(output, { recursive: true });
for (const folder of ['commands', 'emitted', 'programs', 'tmp', 'home'])
  fs.mkdirSync(path.join(output, folder), { recursive: true });

const sha256 = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const fileHash = file => sha256(fs.readFileSync(file));
const json = file => JSON.parse(fs.readFileSync(file));
const read = file => fs.readFileSync(file);

const gatePath = fileURLToPath(import.meta.url);
const receipt = {
  schema: 'noble-owner-law-release-acceptance/v1',
  case: 'CONTRACT-16',
  requirement: 'VC-OWNER-01',
  result: 'running',
  source_root: root,
  output_directory: output,
  gate: { path: path.relative(root, gatePath), sha256: fileHash(gatePath) },
  git_head: null,
  source_revision: null,
  source_manifest: null,
  binary: null,
  tools: {},
  laws: [],
  commands: [],
  builds: {},
  variants: [],
  assertions: [],
  observed: null,
  expected: null,
  failures: [],
  assumptions: [
    'The selected Rust, Lean, bubblewrap, prlimit and systemd-run executables, the pinned MC1 contract library and the host kernel/user systemd manager are trusted execution mechanisms; their byte hashes are not correctness proofs.',
    'A passed gate means the frozen owner-law records bound these exact source, binary, law, proof and artifact bytes through the recorded commands; it does not prove the compiler, backend, loader, host or any claim outside the pure MC2 fragment.',
    'The retained raw command outputs live in this output directory; their hashes, not their paths, bind the receipt.',
  ],
};

const gitExecutable = '/run/current-system/sw/bin/git';
assert.ok(fs.existsSync(gitExecutable),
  `the gate requires the system git at ${gitExecutable}`);

function git(args) {
  const run = spawnSync(gitExecutable, args,
    { cwd: root, encoding: 'utf8', timeout: 120_000,
      env: { PATH: '/run/current-system/sw/bin:/usr/bin:/bin' } });
  assert.equal(run.status, 0, `git ${args.join(' ')} failed: ${run.stderr}`);
  return run.stdout.trim();
}

try {
  assert.equal(git(['status', '--porcelain']), '',
    'the gate requires a committed clean tree');
  receipt.git_head = git(['rev-parse', 'HEAD']);
  assert.match(receipt.git_head, /^[0-9a-f]{40}$/,
    'the gate binds one exact lowercase Git HEAD');

  const policy = json(path.join(root, 'policy/tool-selection.json'));
  const selected = name => {
    const entry = policy.tool_paths[name];
    assert.ok(entry && typeof entry.output === 'string', `unselected tool: ${name}`);
    return entry.output;
  };
  const tool = (name, file) => {
    const real = fs.realpathSync(file);
    assert.ok(fs.statSync(real).isFile(), `${name} is not a file: ${file}`);
    const record = { path: real, sha256: fileHash(real) };
    receipt.tools[name] = record;
    return real;
  };
  const rustRoot = selected('quality_rust');
  const cargo = tool('cargo', path.join(rustRoot, 'bin/cargo'));
  const rustc = tool('rustc', path.join(rustRoot, 'bin/rustc'));
  const lean = tool('lean', path.join(selected('lean'), 'bin/lean'));
  const node = tool('node', path.join(selected('node'), 'bin/node'));
  const wasmTools = tool('wasm-tools', path.join(selected('wasm_tools'), 'bin/wasm-tools'));
  const systemPath = ['/run/current-system/sw/bin', '/usr/bin', '/bin'];
  const findTool = (name, folders) => {
    for (const folder of folders) {
      const candidate = path.join(folder, name);
      if (fs.existsSync(candidate)) return tool(name, candidate);
    }
    assert.fail(`the gate cannot find the selected system tool: ${name}`);
  };
  const callerPath = (process.env.PATH ?? '').split(path.delimiter);
  const compiler = findTool('cc', callerPath);
  const gitBin = findTool('git', callerPath);
  // The reviewed proof-sandbox tools are the MC2 gate's selected versions; a
  // host without them falls back to one on the execution path and records it.
  const findReviewed = (name, reviewed, extra = []) => {
    for (const candidate of reviewed)
      if (fs.existsSync(candidate)) return tool(name, candidate);
    return findTool(name, [...extra, ...systemPath]);
  };
  const bwrap = findReviewed('bwrap', [
    '/nix/store/lqndphylsxqwbwm804n473pb4sqb98sh-bubblewrap-0.11.2/bin/bwrap',
    '/nix/store/y0ra9qr3rz81d9wl7dfrldv3j25dc98q-bubblewrap-0.12.0/bin/bwrap',
  ], callerPath);
  const prlimit = findReviewed('prlimit', [
    '/nix/store/mqvbf0flamqaq9c3ihb496aahag897n0-util-linux-2.42.3-bin/bin/prlimit',
  ]);
  const systemdRun = findReviewed('systemd-run', [
    '/nix/store/baxgs06659w4i6ggsfr56srjrb52v1h8-systemd-261.2/bin/systemd-run',
  ]);
  const library = fs.realpathSync(path.join(root, 'proofs/mc1'));

  const runtime = process.env.XDG_RUNTIME_DIR ?? '';
  assert.ok(runtime && fs.existsSync(path.join(runtime, 'bus')),
    'an active user systemd manager is required for aggregate proof-worker limits');

  const buildEnvironment = {
    PATH: `${path.dirname(cargo)}:${path.dirname(compiler)}:${systemPath.join(':')}`,
    HOME: process.env.HOME ?? '',
    CARGO_HOME: process.env.CARGO_HOME ?? path.join(process.env.HOME ?? '', '.cargo'),
    CARGO_TARGET_DIR: path.join(output, 'target'),
    CARGO_NET_OFFLINE: 'true',
    CARGO_BUILD_JOBS: '4',
    CARGO_INCREMENTAL: '0',
    TMPDIR: path.join(output, 'tmp'),
    LANG: 'C.UTF-8',
    LC_ALL: 'C.UTF-8',
  };
  assert.ok(fs.existsSync(buildEnvironment.CARGO_HOME),
    'the gate requires the caller\'s populated Cargo registry for an offline build');

  const commandEnvironment = {
    PATH: [`${path.dirname(lean)}`, `${path.dirname(node)}`, `${path.dirname(wasmTools)}`,
      `${path.dirname(gitBin)}`, ...systemPath].join(':'),
    HOME: buildEnvironment.HOME,
    XDG_RUNTIME_DIR: runtime,
    NOBLE_LEAN: lean,
    NOBLE_BWRAP: bwrap,
    NOBLE_PRLIMIT: prlimit,
    NOBLE_SYSTEMD_RUN: systemdRun,
    NOBLE_CONTRACT_LIBRARY: library,
    TMPDIR: buildEnvironment.TMPDIR,
    LANG: 'C.UTF-8',
    LC_ALL: 'C.UTF-8',
  };

  let serial = 0;
  function run(label, executable, args, environment, timeout = 1_800_000) {
    const id = ++serial;
    const stem = path.join(output, 'commands',
      `${String(id).padStart(4, '0')}-${label}`);
    const started = Date.now();
    const result = spawnSync(executable, args, {
      cwd: root, env: environment, encoding: null, timeout,
      killSignal: 'SIGKILL', maxBuffer: 64 * 1024 * 1024,
    });
    if (result.error && result.error.code !== 'ETIMEDOUT')
      assert.fail(`${label}: ${result.error}`);
    const stdout = result.stdout ?? Buffer.alloc(0);
    const stderr = result.stderr ?? Buffer.alloc(0);
    fs.writeFileSync(`${stem}.stdout`, stdout, { flag: 'wx', mode: 0o400 });
    fs.writeFileSync(`${stem}.stderr`, stderr, { flag: 'wx', mode: 0o400 });
    const entry = {
      id, label, executable, executable_sha256: fileHash(executable),
      argv: args, cwd: root, environment,
      stdout: path.relative(output, `${stem}.stdout`),
      stdout_sha256: sha256(stdout), stdout_bytes: stdout.length,
      stderr: path.relative(output, `${stem}.stderr`),
      stderr_sha256: sha256(stderr),
      status: result.status, signal: result.signal,
      timed_out: result.error?.code === 'ETIMEDOUT',
      elapsed_ms: Date.now() - started,
    };
    receipt.commands.push(entry);
    return { ...entry, stdoutBytes: stdout, stderrBytes: stderr };
  }

  // The production CLI is built from the committed tree into a private target.
  const build = run('build-cli', cargo,
    ['build', '--locked', '--offline', '-p', 'noble-cli', '--bin', 'noble'],
    buildEnvironment);
  assert.equal(build.status, 0,
    `production CLI build failed: ${build.stderrBytes.toString('utf8')}`);
  const binary = path.join(buildEnvironment.CARGO_TARGET_DIR, 'debug/noble');
  receipt.binary = { path: binary, sha256: fileHash(binary) };
  assert.equal(git(['status', '--porcelain']), '',
    'the source tree changed while the CLI was built');
  assert.equal(git(['rev-parse', 'HEAD']), receipt.git_head,
    'Git HEAD changed while the CLI was built');

  // The receipt binds every tracked input the gate builds or reads.
  const manifestPaths = [
    'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', 'policy/tool-selection.json',
    'crates/noble-cli', 'crates/noble-contracts', 'crates/noble-kernel',
    'crates/noble-wasm', 'crates/noble-ngraph', 'proofs/mc1',
    'verification/owner-law', 'verification/mc1',
    'verification/mc2/contracts',
  ];
  const tracked = git(['ls-files', '-z', '--', ...manifestPaths]).split('\0')
    .filter(name => name.length > 0
      && name !== 'verification/owner-law/acceptance.json')
    .sort();
  assert.ok(tracked.length > 100, 'the source manifest is unexpectedly short');
  receipt.source_manifest = tracked.map(name => {
    const file = path.join(root, name);
    assert.ok(fs.lstatSync(file).isFile(), `manifest source is not a file: ${name}`);
    return { path: name, sha256: fileHash(file), bytes: fs.statSync(file).size };
  });
  const framed = Buffer.concat(receipt.source_manifest.flatMap(item => [
    Buffer.from(`${item.sha256}  ${item.path}\n`),
  ]));
  receipt.source_revision = `sha256:${sha256(framed)}`;

  const law = name => {
    const file = path.join(root, 'verification/owner-law/laws', name);
    const bytes = read(file);
    const digest = sha256(bytes);
    const record = JSON.parse(bytes.toString('utf8'));
    receipt.laws.push({
      path: path.relative(root, file), sha256: digest,
      revision: record.revision, declaration: record.declaration,
    });
    return { file, digest };
  };
  const lawV1 = law('owner-law-v1.json');
  const lawV2 = law('owner-law-v2.json');
  assert.equal(lawV1.digest,
    '9a8eda12c74356ca0d312be840411d4445c6f377c16ee7b96a680581b2e91eac',
    'owner-law-v1.json is not the frozen CONTRACT-16 record');
  assert.equal(lawV2.digest,
    '51f2a3158155077dcb29c8f354d356d31df3282b3c76f60c2bc8534c7932ee80',
    'owner-law-v2.json is not the frozen CONTRACT-16 v2 revision');

  const fixture = name => path.join(root, 'verification/owner-law/fixtures', name);
  const mc1 = name => path.join(root, 'verification/mc1', name);
  const mc2 = name => path.join(root, 'verification/mc2/contracts', name);

  const canonical = json(path.join(root, 'specs/conformance/contract-cases.json'));
  const item = canonical.cases.find(entry => entry.id === 'CONTRACT-16');
  assert.ok(item, 'CONTRACT-16 is absent from the canonical case ledger');
  receipt.expected = item.expected;

  // Two honest submissions: the frozen v1 increment and the owner-approved v2
  // add-two revision. Their emitted artifacts are the only admitted artifacts.
  function honestBuild(label, source, contract, proof, timeout = 600_000) {
    const destination = path.join(output, 'emitted', label);
    const result = run(`${label}-build`, binary, [
      'build', '--require-proof', source, '--contract', contract,
      '--proof', proof, '--timeout-ms', String(timeout), '--out', destination,
    ], commandEnvironment);
    const report = JSON.parse(result.stdoutBytes.toString('utf8'));
    assert.equal(report.schema, 'noble-mc2-build/v1');
    assert.equal(result.status, 0, `${label} build exit: ${result.status}`);
    assert.equal(report.outcome, 'proved');
    assert.equal(report.release.allowed, true);
    const artifact = path.join(destination, 'artifact.wasm');
    assert.ok(fs.existsSync(artifact), `${label} emitted no artifact.wasm`);
    const retained = path.join(output, 'emitted', `${label}-artifact.wasm`);
    fs.writeFileSync(retained, read(artifact), { flag: 'wx', mode: 0o400 });
    const facts = {
      command: result.id,
      report: result.stdout,
      outcome: report.outcome,
      release_allowed: report.release.allowed,
      recipe_identity: report.subject_identity?.digest ?? null,
      artifact: { path: retained, sha256: fileHash(retained), bytes: fs.statSync(retained).size },
      destination,
    };
    assert.equal(facts.release_allowed, true);
    return facts;
  }
  receipt.builds.increment = honestBuild('increment',
    mc2('q-one.noble'), mc1('increment.noble-contract'), mc1('increment-proof.lean'));
  receipt.builds.add_two = honestBuild('add-two',
    fixture('add-two.noble'), fixture('add-two.contract'), fixture('add-two.proof.lean'));

  function release(label, argv) {
    const result = run(label, binary, ['release', ...argv], commandEnvironment);
    const report = JSON.parse(result.stdoutBytes.toString('utf8'));
    assert.equal(report.schema, 'noble-owner-law-release/v1');
    assert.equal(report.guest_requests, 0);
    assert.equal(report.protected_operations, 0);
    return { result, report, command: result.id };
  }

  const addTwoArtifact = path.join(output, 'emitted', 'add-two-artifact.wasm');
  const incrementArtifact = path.join(output, 'emitted', 'increment-artifact.wasm');

  // Variant 1: a program that preserves the recorded example and violates the law.
  const runProgram = (label, prefix) => {
    const source = path.join(output, 'programs', `${label}.noble`);
    fs.writeFileSync(source, `${prefix} [ dup 1 = [ drop 2 ] [ ] if ] run\n`,
      { flag: 'wx', mode: 0o400 });
    const result = run(label, binary, ['run', source],
      { ...commandEnvironment, NOBLE_LEAN: '/nonexistent/proof-services-disabled',
        NOBLE_CONTRACT_LIBRARY: '/nonexistent/proof-library-disabled' });
    assert.equal(result.status, 0, `${label}: ${result.stderrBytes}`);
    return { id: result.id, report: JSON.parse(result.stdoutBytes.toString('utf8')) };
  };
  const example = runProgram('wrong-program-input-1', '1');
  const violating = runProgram('wrong-program-input-3', '3');
  const stackValue = report => report.stack?.map(entry => entry.value) ?? [];
  receipt.wrong_program = {
    example_input: 1,
    example_output: stackValue(example.report),
    violating_input: 3,
    violating_output: stackValue(violating.report),
    law_output_for_example: 2,
    law_output_for_violating: 4,
    commands: [example.id, violating.id],
  };
  assert.deepEqual(stackValue(example.report), ['2'],
    'the wrong program does not preserve the recorded example 1 -> 2');
  assert.deepEqual(stackValue(violating.report), ['3'],
    'the wrong program does not violate the law at input 3');

  const v1 = [
    '--law', lawV1.file, '--law-digest', lawV1.digest,
  ];
  // Positive control: the law-conforming v1 submission is eligible, so this
  // gate is not a reject-everything route.
  const baseline = release('v1-law-conforming-increment', [
    ...v1,
    '--source', mc2('q-one.noble'),
    '--contract', mc1('increment.noble-contract'),
    '--proof', mc1('increment-proof.lean'),
    '--artifact', incrementArtifact,
    '--timeout-ms', '600000',
  ]);
  assert.equal(baseline.report.admission, 'eligible',
    JSON.stringify(baseline.report.refusal));
  assert.equal(baseline.result.status, 0);
  assert.equal(baseline.report.exact_check.outcome, 'proved');
  assert.equal(baseline.report.exact_check.release_allowed, true);
  assert.equal(baseline.report.exact_check.emitted_artifact.matches_rebuild, true);
  for (const check of baseline.report.checks)
    assert.equal(check.status, 'matched', `${check.name}: ${check.observed}`);
  receipt.baseline = {
    command: baseline.command,
    exit: baseline.result.status,
    admission: baseline.report.admission,
    law_sha256: baseline.report.law.sha256,
    recipe_identity: baseline.report.exact_check.recipe_identity.digest,
    proposition_sha256: baseline.report.exact_check.generated_proposition.sha256,
    artifact_sha256: baseline.report.exact_check.emitted_artifact.sha256,
  };
  const variants = [];
  function variant(id, change, expected, observed, extra) {
    const entry = { id, change, expected, observed, matches: expected === observed, ...extra };
    variants.push(entry);
    if (!entry.matches) receipt.failures.push({ id, expected, observed });
    return entry;
  }

  const wrong = release('v1-example-preserving-wrong', [
    ...v1,
    '--source', fixture('example-preserving-wrong.noble'),
    '--contract', fixture('example-preserving-wrong.contract'),
    '--proof', mc1('increment-proof.lean'),
    '--artifact', incrementArtifact,
    '--timeout-ms', '600000',
  ]);
  assert.equal(wrong.result.status === 0, false, 'the wrong program was admitted');
  assert.equal(wrong.report.admission, 'rejected');
  assert.ok(['owner-law-proof-not-exact', 'owner-law-program-not-acceptable']
    .includes(wrong.report.refusal.code), JSON.stringify(wrong.report.refusal));
  variant('example-preserving-wrong-program',
    'candidate-special-cases-example-input-1-then-returns-identity-for-other-inputs',
    'reject', 'reject', {
      code: wrong.report.refusal.code, command: wrong.command, exit: wrong.result.status,
    });

  const alteredLaw = release('v2-altered-law', [
    '--law', fixture('altered-law.json'), '--law-digest', lawV1.digest,
    '--source', mc2('q-one.noble'),
    '--contract', mc1('increment.noble-contract'),
    '--proof', mc1('increment-proof.lean'),
    '--artifact', incrementArtifact,
    '--timeout-ms', '600000',
  ]);
  assert.equal(alteredLaw.report.admission, 'rejected');
  assert.equal(alteredLaw.report.refusal.code, 'owner-law-digest-mismatch');
  const alteredStatement = release('v2-altered-statement', [
    ...v1,
    '--source', mc2('q-one.noble'),
    '--contract', fixture('identity-statement.contract'),
    '--proof', mc1('increment-proof.lean'),
    '--artifact', incrementArtifact,
    '--timeout-ms', '600000',
  ]);
  assert.equal(alteredStatement.report.admission, 'rejected');
  assert.equal(alteredStatement.report.refusal.code, 'owner-law-ensures-mismatch');
  variant('altered-law-or-expected-statement',
    'candidate-changes-owner-law-or-expected-statement-to-easier-identity',
    'reject', 'reject', {
      codes: [alteredLaw.report.refusal.code, alteredStatement.report.refusal.code],
      commands: [alteredLaw.command, alteredStatement.command],
      exits: [alteredLaw.result.status, alteredStatement.result.status],
    });

  const unrelatedProof = release('v3-unrelated-proof-artifact', [
    ...v1,
    '--source', mc2('q-one.noble'),
    '--contract', mc1('increment.noble-contract'),
    '--proof', mc2('composed.proof.lean'),
    '--artifact', incrementArtifact,
    '--timeout-ms', '600000',
  ]);
  assert.equal(unrelatedProof.report.admission, 'rejected');
  assert.equal(unrelatedProof.report.refusal.code, 'owner-law-proof-not-exact');
  const staleArtifact = release('v3-stale-emitted-artifact', [
    ...v1,
    '--source', mc2('q-one.noble'),
    '--contract', mc1('increment.noble-contract'),
    '--proof', mc1('increment-proof.lean'),
    '--artifact', addTwoArtifact,
    '--timeout-ms', '600000',
  ]);
  assert.equal(staleArtifact.report.admission, 'rejected');
  assert.equal(staleArtifact.report.refusal.code, 'owner-law-artifact-mismatch');
  variant('unrelated-or-stale-valid-proof-artifact',
    'candidate-substitutes-unrelated-or-stale-valid-proof-artifact',
    'reject', 'reject', {
      codes: [unrelatedProof.report.refusal.code, staleArtifact.report.refusal.code],
      commands: [unrelatedProof.command, staleArtifact.command],
      exits: [unrelatedProof.result.status, staleArtifact.result.status],
    });

  const vacuous = release('v4-weakened-precondition', [
    ...v1,
    '--source', mc2('q-one.noble'),
    '--contract', fixture('weakened-precondition.contract'),
    '--proof', mc2('nonnegative.proof.lean'),
    '--artifact', incrementArtifact,
    '--timeout-ms', '600000',
  ]);
  assert.equal(vacuous.report.admission, 'rejected');
  assert.equal(vacuous.report.refusal.code, 'owner-law-requires-mismatch');
  variant('false-or-unestablished-precondition',
    'candidate-substitutes-false-or-unestablished-precondition',
    'reject', 'reject', {
      code: vacuous.report.refusal.code, command: vacuous.command,
      exit: vacuous.result.status,
    });

  const revision = release('v5-owner-approved-revision', [
    '--law', lawV2.file, '--law-digest', lawV2.digest,
    '--source', fixture('add-two.noble'),
    '--contract', fixture('add-two.contract'),
    '--proof', fixture('add-two.proof.lean'),
    '--artifact', addTwoArtifact,
    '--timeout-ms', '600000',
  ]);
  assert.equal(revision.report.admission, 'eligible', JSON.stringify(revision.report.refusal));
  assert.equal(revision.result.status, 0);
  const exact = revision.report.exact_check;
  assert.equal(exact.outcome, 'proved');
  assert.equal(exact.release_allowed, true);
  assert.ok(exact.recipe_identity.digest, 'no resolved recipe identity');
  assert.ok(exact.generated_proposition.sha256, 'no generated proposition identity');
  assert.equal(exact.checked_theorem.declaration, 'proof');
  assert.equal(exact.checked_theorem.type, 'MC1Obligation.claim');
  assert.equal(exact.checked_theorem.independent_recheck, true);
  assert.equal(exact.emitted_artifact.matches_rebuild, true);
  assert.equal(exact.emitted_artifact.sha256, fileHash(addTwoArtifact));
  assert.ok(exact.selected_toolchain.lean_binary_sha256, 'no selected Lean binding');
  assert.ok(exact.selected_toolchain.library_blake3, 'no selected library binding');
  for (const check of revision.report.checks)
    assert.equal(check.status, 'matched', `${check.name}: ${check.observed}`);
  assert.equal(revision.report.law.supersedes,
    'owner-law-v1-immutable-before-candidate-edit');
  variant('owner-approves-new-law-revision-and-fresh-exact-proof',
    'owner-approves-new-law-revision-and-fresh-exact-proof',
    'eligible-after-independent-check', 'eligible-after-independent-check', {
      command: revision.command,
      exit: revision.result.status,
      recipe_identity: exact.recipe_identity.digest,
      proposition_sha256: exact.generated_proposition.sha256,
      artifact_sha256: exact.emitted_artifact.sha256,
      lean_binary_sha256: exact.selected_toolchain.lean_binary_sha256,
      library_blake3: exact.selected_toolchain.library_blake3,
    });

  // Ordinary admission without the proof policy: the same program compiles,
  // missing evidence stays `unknown`, and supplied evidence is still proved by
  // the pre-existing ordinary route.
  const compile = run('v6-ordinary-compile', binary, ['compile', mc2('q-one.noble')],
    { ...commandEnvironment, NOBLE_LEAN: '/nonexistent/proof-services-disabled',
      NOBLE_CONTRACT_LIBRARY: '/nonexistent/proof-library-disabled' });
  assert.equal(compile.status, 0, `ordinary compile: ${compile.stderrBytes}`);
  const verifyWithoutProof = run('v6-ordinary-verify-without-proof', binary,
    ['verify', mc1('increment.noble-contract')],
    { ...commandEnvironment, NOBLE_LEAN: '/nonexistent/proof-services-disabled',
      NOBLE_CONTRACT_LIBRARY: '/nonexistent/proof-library-disabled' });
  const missingEvidence = JSON.parse(verifyWithoutProof.stdoutBytes.toString('utf8'));
  assert.equal(verifyWithoutProof.status, 3);
  assert.equal(missingEvidence.outcome, 'unknown');
  const verifyWithProof = run('v6-ordinary-verify-with-proof', binary,
    ['verify', mc1('increment.noble-contract'), '--proof', mc1('increment-proof.lean'),
      '--timeout-ms', '600000'], commandEnvironment);
  const ordinaryProof = JSON.parse(verifyWithProof.stdoutBytes.toString('utf8'));
  assert.equal(verifyWithProof.status, 0,
    `ordinary verify with proof: ${verifyWithProof.stderrBytes}`);
  assert.equal(ordinaryProof.outcome, 'proved');
  assert.equal(ordinaryProof.schema, 'noble-mc1-report/v1');
  for (const entry of [wrong, alteredLaw, alteredStatement, unrelatedProof, staleArtifact,
    vacuous, revision])
    assert.equal(entry.report.ordinary_admission, 'unchanged');
  variant('ordinary-well-typed-program-without-proof-policy',
    'ordinary-well-typed-program-without-proof-policy',
    'ordinary-admission-unchanged', 'ordinary-admission-unchanged', {
      compile: { command: compile.id, exit: compile.status },
      verify_without_proof: { command: verifyWithoutProof.id, exit: verifyWithoutProof.status,
        outcome: missingEvidence.outcome },
      verify_with_proof: { command: verifyWithProof.id, exit: verifyWithProof.status,
        outcome: ordinaryProof.outcome },
    });

  // Every canonical variant is present exactly once, and every outcome matches.
  assert.deepEqual(variants.map(entry => entry.change),
    item.input.variants.map(entry => entry.change),
    'the executed variants differ from the canonical CONTRACT-16 variant list');
  for (const [index, expected] of item.input.variants.entries())
    assert.equal(variants[index].expected, expected.expected);
  receipt.variants = variants;
  assert.equal(receipt.failures.length, 0,
    `variant mismatches: ${JSON.stringify(receipt.failures)}`);

  // The aggregate observation must equal every canonical `expected` field.
  const releaseReports = [baseline, wrong, alteredLaw, alteredStatement, unrelatedProof,
    staleArtifact, vacuous, revision].map(entry => entry.report);
  const observed = {
    stage: releaseReports.every(report =>
      report.stage === 'proof-required-release-admission')
      ? 'proof-required-release-admission' : 'unexpected',
    outcome: variants.every(entry => entry.matches) ? 'each-variant-matches' : 'variant-mismatch',
    owner_law_independent: releaseReports.every(report => report.owner_law_independent === true),
    theorem_statement_and_assumptions_compared:
      releaseReports.every(report =>
        report.theorem_statement_and_assumptions_compared === true),
    exact_checked_proof_and_artifact_binding_required:
      releaseReports.every(report =>
        report.exact_checked_proof_and_artifact_binding_required === true),
    proof_author_status_is_authority:
      !releaseReports.every(report => report.proof_author_status_is_authority === false),
    pure_fragment_only: releaseReports.every(report => report.pure_fragment_only === true),
    compiler_backend_host_proof_claimed:
      releaseReports.some(report => report.compiler_backend_host_proof_claimed !== false),
    guest_requests: releaseReports.reduce((total, report) => total + report.guest_requests, 0),
    protected_operations:
      releaseReports.reduce((total, report) => total + report.protected_operations, 0),
  };
  receipt.observed = observed;
  assert.deepEqual(observed, item.expected,
    'aggregate observations differ from the canonical CONTRACT-16 expectations');

  // No input or HEAD may change across the run.
  assert.equal(git(['status', '--porcelain']), '',
    'the source tree changed during the gate run');
  assert.equal(git(['rev-parse', 'HEAD']), receipt.git_head,
    'Git HEAD changed during the gate run');
  for (const entry of receipt.source_manifest)
    assert.equal(fileHash(path.join(root, entry.path)), entry.sha256,
      `${entry.path} changed during the gate run`);
  receipt.result = 'passed';
} catch (error) {
  receipt.result = 'failed';
  receipt.failure = String(error && error.stack ? error.stack : error);
} finally {
  const destination = path.join(output, 'acceptance.json');
  fs.writeFileSync(destination,
    `${JSON.stringify(receipt, null, 2)}\n`, { flag: 'wx', mode: 0o400 });
  console.log(JSON.stringify({
    receipt: destination, sha256: fileHash(destination), result: receipt.result,
    head: receipt.git_head, failure: receipt.failure ?? null,
  }));
}
if (receipt.result !== 'passed') process.exitCode = 1;
