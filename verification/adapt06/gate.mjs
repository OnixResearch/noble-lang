#!/usr/bin/env node
// ADAPT-06 prepromotion gate: a layout-valid typed test.emit Submission whose
// public environment forges the fixed host contract as effect-free. `--plan`
// is read-only. Execution requires the separately reviewed frozen source
// revision and a new external directory; it writes nothing into the repository
// and never changes a canonical case or an earlier receipt.
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const sha = value => crypto.createHash('sha256').update(value).digest('hex');
const bytes = file => fs.readFileSync(path.join(root, file));
const caseFile = 'specs/conformance/adaptation-cases.json';
const design = JSON.parse(bytes(caseFile)).cases.find(row => row.id === 'ADAPT-06');
assert.ok(design, 'missing canonical ADAPT-06');
assert.equal(design.profile, 'Core-Bootstrap');
assert.equal(design.kind, 'admission');
assert.deepEqual(design.requirements, ['S-DECODE-01', 'B-IMPL-03']);
assert.deepEqual(design.input, {
  harness: 'layout-valid-forged-candidate', representation_valid: true,
  source: '"audit" test.emit', input_stack: [], output_stack: [],
  host_contracts: { 'test.emit': 'Text -- ! {test.emit}' },
  non_effect_derivations: 'valid', advertised_effects: [], required_effects: ['test.emit'],
});
assert.deepEqual(design.expected, { stage: 'acceptance', outcome: 'reject',
  accepted_program_created: false, guest_requests: 0, protected_operations: 0 });
assert.deepEqual(design.state,
  { implementation: 'absent', execution: 'not-run', proof: 'open', trust: 'unassessed' });
assert.deepEqual(design.evidence, [], 'ADAPT-06 already has evidence; this is a prepromotion gate');

const selected = JSON.parse(bytes('policy/tool-selection.json'));
const rust = path.join(selected.tool_paths.quality_rust.output, 'bin');
const linker = selected.component_sync.linker_bin;
const sources = {};
const add = file => { sources[file] = sha(bytes(file)); };
function tree(directory) {
  for (const entry of fs.readdirSync(path.join(root, directory), { withFileTypes: true })
    .sort((left, right) => left.name.localeCompare(right.name))) {
    const file = `${directory}/${entry.name}`;
    assert.equal(entry.isSymbolicLink(), false, `unbound source symlink: ${file}`);
    if (entry.isDirectory()) tree(file);
    else if (entry.isFile()) add(file);
  }
}
for (const directory of ['crates/noble-kernel/src', 'crates/noble-contracts/src',
  'crates/noble-wasm/src', 'crates/noble-wasm/runtime', 'crates/noble-wasm/wit',
  'crates/noble-cli/src']) tree(directory);
for (const file of ['Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', 'policy/tool-selection.json',
  ...['noble-kernel', 'noble-contracts', 'noble-wasm', 'noble-cli', 'noble-syndicate']
    .map(name => `crates/${name}/Cargo.toml`),
  // Lean sources the CLI binary embeds with include_str!.
  'proofs/mc1/NobleContracts.lean', 'proofs/mc1/IntrinsicTypeWitness.lean',
  ...['Model', 'Rules', 'Expression', 'Obligation', 'Examples', 'Composition', 'NamedSubject',
    'NamedV2'].map(name => `proofs/mc1/NobleContracts/${name}.lean`),
  caseFile, '.cairn/specs/safety/spec.md', '.cairn/specs/core-bootstrap/spec.md',
  'crates/noble-wasm/tests/forged_effects.rs', 'verification/adapt06/check.rs',
  'verification/adapt06/gate.mjs']) add(file);
const sourceRevision = `sha256:${sha(JSON.stringify(Object.entries(sources).sort()))}`;
if (process.argv.length === 3 && process.argv[2] === '--plan') {
  console.log(JSON.stringify({ result: 'ready-for-owner-review-not-execution', case_id: 'ADAPT-06',
    source_revision: sourceRevision, source_files: Object.keys(sources).length,
    selected_rust: selected.tool_paths.quality_rust,
    runs: ['cargo-build-cli-and-wasm', 'cargo-test-build-forged-effects', 'dep-info-input-coverage',
      'regression-test-binary', 'independent-typed-source-kernel-compiler-peer',
      'compiled-cli-authentic-session'],
    requires: ['explicit final SOURCE FREEZE',
      'new external directory and the reviewed source_revision argument'] }));
  process.exit(0);
}
const [destination, expectedRevision] = process.argv.slice(2);
assert.equal(process.argv.length, 4,
  'usage: node verification/adapt06/gate.mjs --plan | NEW_EXTERNAL_DIRECTORY sha256:REVIEWED_FROZEN_SOURCE_REVISION');
assert.match(expectedRevision, /^sha256:[0-9a-f]{64}$/);
assert.equal(sourceRevision, expectedRevision, 'source differs from the separately reviewed freeze');
const artifacts = path.resolve(destination);
assert.ok(artifacts !== root && !artifacts.startsWith(`${root}${path.sep}`), 'artifacts must be external');
fs.mkdirSync(artifacts);
for (const name of ['home', 'tmp', 'inputs', 'cargo-home']) fs.mkdirSync(path.join(artifacts, name));
const input = (name, content) => {
  const file = path.join(artifacts, 'inputs', name);
  fs.writeFileSync(file, content, { flag: 'wx' });
  return file;
};
input('adaptation-cases.prepromotion.json', bytes(caseFile));
const sourceFile = input('ADAPT-06.noble', design.input.source);
const sessionInput = Buffer.from(`${design.input.source}\n`);
input('ADAPT-06.session.stdin', sessionInput);
// A clean environment: no inherited wrapper, flags, profile or linker overrides.
const target = path.join(artifacts, 'target');
const environment = {
  HOME: path.join(artifacts, 'home'), CARGO_HOME: path.join(artifacts, 'cargo-home'),
  CARGO_TARGET_DIR: target, TMPDIR: path.join(artifacts, 'tmp'), CARGO_BUILD_JOBS: '4',
  CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER: path.join(linker, 'cc'),
  RUSTC_WRAPPER: '', RUSTC_WORKSPACE_WRAPPER: '', RUSTFLAGS: '', CARGO_ENCODED_RUSTFLAGS: '',
  PATH: `${rust}:${linker}:/run/current-system/sw/bin`,
};
const commands = [];
function run(label, executable, args, stdin) {
  const response = spawnSync(executable, args, { cwd: root, env: environment, input: stdin,
    timeout: 900_000, maxBuffer: 64 * 1024 * 1024 });
  const stem = `${String(commands.length).padStart(2, '0')}-${label}`;
  const stdout = response.stdout ?? Buffer.alloc(0);
  const stderr = response.stderr ?? Buffer.alloc(0);
  fs.writeFileSync(path.join(artifacts, `${stem}.stdout`), stdout, { flag: 'wx' });
  fs.writeFileSync(path.join(artifacts, `${stem}.stderr`), stderr, { flag: 'wx' });
  const row = { label, executable, executable_sha256: sha(fs.readFileSync(executable)), args,
    stdin_sha256: stdin === undefined ? null : sha(stdin), status: response.status,
    signal: response.signal, error: response.error?.message ?? null,
    stdout: `${stem}.stdout`, stderr: `${stem}.stderr`,
    stdout_sha256: sha(stdout), stderr_sha256: sha(stderr) };
  commands.push(row);
  assert.equal(row.error, null, `${label} spawn: ${row.error}`);
  assert.equal(row.signal, null, `${label} signal: ${row.signal}`);
  assert.equal(row.status, 0, `${label}: ${stderr.toString('utf8')}`);
  return stdout.toString('utf8');
}
const toolchain = { rustc: run('rustc-version', path.join(rust, 'rustc'), ['-vV']).trim(),
  cargo: run('cargo-version', path.join(rust, 'cargo'), ['-vV']).trim(),
  linker: environment.CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER };
const cargoFlags = ['--all-features', '--locked', '--offline', '--message-format=json'];
const compiled = output => output.split('\n').filter(line => line.startsWith('{'))
  .map(line => JSON.parse(line)).filter(row => row.reason === 'compiler-artifact');
function unique(rows, name, kind) {
  const files = [...new Set(rows.filter(row => row.target.name === name && row.target.kind.includes(kind))
    .flatMap(row => kind === 'lib' ? row.filenames.filter(file => file.endsWith('.rlib'))
      : [row.executable]).filter(Boolean))];
  assert.equal(files.length, 1, `unique compiled ${name}/${kind}`);
  return files[0];
}
const built = compiled(run('build', path.join(rust, 'cargo'),
  ['build', '-p', 'noble-cli', '-p', 'noble-wasm', ...cargoFlags]));
const cli = unique(built, 'noble', 'bin');
const libraries = Object.fromEntries(['noble_kernel', 'noble_contracts', 'noble_wasm']
  .map(name => [name, unique(built, name, 'lib')]));
const testBuilt = compiled(run('test-build', path.join(rust, 'cargo'),
  ['test', '-p', 'noble-wasm', '--test', 'forged_effects', '--no-run', ...cargoFlags]));
const regression = unique(testBuilt, 'forged_effects', 'test');
for (const [name, library] of Object.entries(libraries)) {
  assert.equal(sha(fs.readFileSync(unique(testBuilt, name, 'lib'))), sha(fs.readFileSync(library)),
    `the regression test links a different ${name}`);
}
const peer = path.join(artifacts, 'adapt06-check');
run('build-adapt06-check', path.join(rust, 'rustc'), ['--edition=2021', '--crate-name',
  'adapt06_check', 'verification/adapt06/check.rs', `--emit=dep-info=${peer}.d,link=${peer}`,
  ...Object.entries(libraries).flatMap(([name, library]) => ['--extern', `${name}=${library}`]),
  '-L', `dependency=${path.join(target, 'debug/deps')}`]);
// Every compiler-reported input of the executed artifacts must be hashed above.
function dependencies(file) {
  const found = [];
  for (const line of fs.readFileSync(file, 'utf8').split('\n')) {
    const separator = line.indexOf(': ');
    if (line.startsWith('#') || separator < 0) continue;
    for (const item of line.slice(separator + 2).split(/(?<!\\) /).filter(Boolean)) {
      const absolute = path.resolve(root, item.replaceAll('\\ ', ' '));
      assert.ok(absolute.startsWith(`${root}${path.sep}`), `build input outside the repository: ${item}`);
      found.push(path.relative(root, absolute).split(path.sep).join('/'));
    }
  }
  return found;
}
const buildInputs = [...new Set([`${cli}.d`, libraries.noble_wasm.replace(/\.rlib$/, '.d'),
  `${regression}.d`, `${peer}.d`].flatMap(dependencies))].sort();
for (const file of buildInputs) assert.ok(Object.hasOwn(sources, file), `unbound build input: ${file}`);
assert.ok(buildInputs.includes('crates/noble-wasm/tests/forged_effects.rs'));
assert.ok(buildInputs.includes('verification/adapt06/check.rs'));

const testNames = ['forged_effect_free_emit_contract_is_refused_before_emission',
  'request_only_empty_bound_is_an_effect_inclusion_refusal'];
const testOutput = run('regression-test', regression, ['--test-threads', '1']);
const passed = testOutput.split('\n').map(line => /^test (\S+) \.\.\. ok$/.exec(line)?.[1])
  .filter(Boolean).sort();
assert.deepEqual(passed, testNames);
assert.match(testOutput,
  /^test result: ok\. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;/m);

const wat = path.join(artifacts, 'authentic.wat');
const observed = JSON.parse(run('adapt06-peer', peer, [sourceFile, wat]).trim());
assert.equal(observed.id, 'ADAPT-06');
assert.equal(observed.premise.definitions, 0);
assert.equal(observed.premise.stack_in, design.input.input_stack.length);
assert.equal(observed.premise.stack_out, design.input.output_stack.length);
assert.deepEqual(observed.premise.allowed_effects, design.input.required_effects);
assert.deepEqual(observed.premise.invocations, [{ definition: 22, behavior: 'Some(TestEmit)' }]);
assert.deepEqual(observed.premise.texts, ['audit']);
assert.equal(observed.premise.emit_behavior, 'Some(TestEmit)');
assert.deepEqual(observed.premise.emit_contract_effects, design.input.required_effects);
assert.deepEqual(observed.authentic.kernel,
  { outcome: 'Accepted', effects: design.input.required_effects });
assert.equal(observed.authentic.compiler.outcome, 'prepared');
assert.equal(observed.authentic.compiler.wat_file, wat);
const watBytes = fs.readFileSync(wat);
assert.equal(watBytes.length, observed.authentic.compiler.wat_bytes);
assert.deepEqual(observed.forged.mutations,
  ['environment.defs[22].effects=[]', 'request.expected.allowed_effects=[]']);
assert.deepEqual(observed.forged.emit_contract_effects, design.input.advertised_effects);
// A kernel refusal here would reduce the case to the S-CASE-05 bound check.
assert.deepEqual(observed.forged.kernel,
  { outcome: 'Accepted', effects: design.input.advertised_effects },
  'the forged environment and empty bound are not kernel-consistent; the ADAPT-06 premise failed');
assert.deepEqual(observed.forged.compiler, { outcome: 'Invalid', wat_bytes: null, wat_file: null });
assert.deepEqual(observed.request_only.mutations, ['request.expected.allowed_effects=[]']);
assert.deepEqual(observed.request_only.kernel,
  { outcome: 'Invalid', constraint: 'EffectInclusion(test.emit)' });
assert.deepEqual(observed.request_only.compiler,
  { outcome: 'Invalid', wat_bytes: null, wat_file: null });
// The forged path returns before any WAT exists; the peer constructs no engine,
// guest instance or host, so both counts are structural rather than live.
const forgedPath = { stage: 'acceptance', outcome: 'reject', accepted_program_created: false,
  guest_requests: 0, protected_operations: 0, program_emitted: false, engine_constructed: false,
  basis: 'noble_wasm::source::Compiler::prepare returned Invalid before emitting WAT' };
for (const [field, value] of Object.entries(design.expected)) {
  assert.deepEqual(forgedPath[field], value, `ADAPT-06.${field}`);
}

const reports = run('cli-authentic-session', cli, ['session'], sessionInput).trim().split('\n')
  .map(line => JSON.parse(line));
assert.equal(reports.length, 1);
const [live] = reports;
assert.equal(live.profile, 'Core-Bootstrap');
assert.equal(live.stage, 'wasm');
assert.equal(live.outcome, 'normal');
assert.deepEqual(live.stack, []);
assert.deepEqual(live.request_trace, ['test.emit:audit']);
assert.equal(live.guest_requests, 1);
assert.equal(live.protected_operations, 0);
assert.equal(live.module.source_sha256, sha(sessionInput));
assert.equal(live.module.wat_sha256, sha(watBytes), 'the CLI ran a different module than the peer');

for (const [file, digest] of Object.entries(sources)) {
  assert.equal(sha(bytes(file)), digest, `source changed during the gate: ${file}`);
}
const identity = file => ({ path: file, sha256: sha(fs.readFileSync(file)) });
const binaries = { cli: identity(cli), regression_test: identity(regression), peer: identity(peer),
  libraries: Object.fromEntries(Object.entries(libraries)
    .map(([name, file]) => [name, identity(file)])) };
for (const row of commands.filter(row => [cli, regression, peer].includes(row.executable))) {
  assert.equal(row.executable_sha256, sha(fs.readFileSync(row.executable)),
    `${row.label} executable changed during the gate`);
}
const receipt = { schema: 'noble-adapt06-receipt/v1', result: 'passed', kind: 'test',
  case_id: 'ADAPT-06',
  case: { id: design.id, profile: design.profile, kind: design.kind,
    requirements: design.requirements, input: design.input, expected: design.expected },
  claim: 'The actual typed "audit" test.emit Submission, cloned with only environment.defs[22].effects=[] and request.expected.allowed_effects=[], is accepted by the kernel with derived effects [] (a locally consistent false premise; the request-only empty bound is rejected by the kernel with EffectInclusion(test.emit), as in S-CASE-05). The independent Wasm compiler refuses the forged Submission as Invalid against its fixed host contract before emitting a program, while the authentic Submission compiles to the WAT that a compiled CLI session runs with exactly one test.emit request. No engine, guest or host is constructed on the forged path; no proof is claimed.',
  source_revision: sourceRevision,
  source_revision_algorithm: 'sha256(JSON.stringify(Object.entries(source_sha256).sort()))',
  source_sha256: sources, prepromotion_case_sha256: sources[caseFile], build_inputs: buildInputs,
  selected_rust: selected.tool_paths.quality_rust, toolchain,
  environment: Object.fromEntries(Object.entries(environment)
    .map(([key, value]) => [key, value.startsWith(artifacts) ? `EXTERNAL${value.slice(artifacts.length)}` : value])),
  external_raw_output: artifacts, binaries,
  authentic_wat: { path: wat, sha256: sha(watBytes), bytes: watBytes.length },
  commands,
  observed: { peer: observed, regression_tests: passed,
    authentic_session: { profile: live.profile, stage: live.stage, outcome: live.outcome,
      stack: live.stack, request_trace: live.request_trace, guest_requests: live.guest_requests,
      protected_operations: live.protected_operations, module: live.module },
    forged_path: forgedPath },
  failures: 0, integrity_failures: 0,
  assumptions: [
    'The pinned selected Rust toolchain, typed source preparation, kernel, Wasm compiler, compiled CLI and its selected Node host are trusted only at the recorded source and binary identities.',
    'Zero guest requests and protected operations on the forged path are structural: the compiler returned Invalid, no WAT existed and no engine, guest instance or host was constructed. They are not a live runtime count.',
    'This finite hostile case does not establish universal admission soundness, decoder layout separation for other representations or type-level non-forgeability; proof remains open.',
  ] };
const receiptFile = path.join(artifacts, 'acceptance.json');
fs.writeFileSync(receiptFile, JSON.stringify(receipt, null, 2) + '\n', { flag: 'wx' });
console.log(JSON.stringify({ result: 'passed', case_id: 'ADAPT-06', receipt: receiptFile,
  receipt_sha256: sha(fs.readFileSync(receiptFile)), source_revision: sourceRevision,
  prepromotion_case_sha256: sources[caseFile], binary_sha256: binaries.cli.sha256,
  commands: commands.length, failures: 0, integrity_failures: 0 }));
