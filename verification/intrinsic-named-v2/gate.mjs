#!/usr/bin/env node
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const digest = file => hash(fs.readFileSync(file));
const source = name => path.join(root, name);
const selected = JSON.parse(fs.readFileSync(source('policy/tool-selection.json')));
const pins = JSON.parse(fs.readFileSync(source('verification/m6/pins.json')));
const tool = (name, binary) => path.join(selected.tool_paths[name].output, 'bin', binary);
const node = tool('node', 'node');
assert.equal(fs.realpathSync(process.execPath), fs.realpathSync(node), 'selected Node required');
assert.deepEqual(process.execArgv, [], 'Node flags not selected');
assert.equal(process.argv.length, 4, 'usage: SELECTED_NODE gate.mjs NEW_EXTERNAL_DIRECTORY REVIEWED_SOURCE_STABLE_CLI_SHA256');
const output = path.resolve(process.argv[2]);
assert.match(process.argv[3], /^[a-f0-9]{64}$/u);
assert.ok(output !== root && !output.startsWith(`${root}${path.sep}`), 'external receipt required');
assert.ok(!fs.existsSync(output), 'immutable receipt output already exists');
assert.equal(fs.realpathSync(path.dirname(output)), path.dirname(output), 'symlinked output parent');
fs.mkdirSync(output, {mode: 0o700});
const receipt = {schema: 'noble-intrinsic-named-v2-acceptance/v1', profile: 'CONTRACT-26',
  result: 'running', source_root: root, output_directory: output, sources: {}, source_snapshots: {}, tools: {},
  fixtures: {}, retained: {}, commands: [], cases: [], failures: [], integrity_failures: [],
  portable_receipt_self_exclusion: 'verification/intrinsic-named-v2/acceptance.json',
  assumptions: ['Finite source-bound checking does not prove Rust refinement or generalized Noble-to-Lean translation.',
    'No candidate-authored proof grants host authorization, owner-frozen law release or universal compiler correctness.']};
const watched = new Map();
function watch(file) {
  const current = digest(file);
  assert.ok(!watched.has(file) || watched.get(file) === current, `changed input: ${file}`);
  watched.set(file, current);
  return current;
}
function retain(name, bytes, mode = 0o400) {
  assert.ok(!name.split('/').some(part => !part || part === '..' || part === '.'), 'unsafe retained path');
  const file = path.join(output, name);
  fs.mkdirSync(path.dirname(file), {recursive: true});
  fs.writeFileSync(file, bytes, {flag: 'wx', mode});
  receipt.retained[name] = {sha256: watch(file), bytes: bytes.length};
  return file;
}
function snapshotSource(name) {
  const original = source(name);
  const bytes = fs.readFileSync(original);
  const sha256 = watch(original);
  assert.equal(hash(bytes), sha256, `${name}: source changed during snapshot`);
  const copy = retain(`plan/sources/${name}`, bytes);
  receipt.sources[name] = sha256;
  receipt.source_snapshots[name] = {file: path.relative(output, copy), sha256, bytes: bytes.length};
}
function freeze(name, original) {
  const real = fs.realpathSync(original);
  const copy = retain(`tools/${name}`, fs.readFileSync(real), 0o500);
  assert.equal(watch(real), digest(copy));
  receipt.tools[name] = {original: real, file: path.relative(output, copy), sha256: digest(copy)};
  return copy;
}
function command(label, binary, argv, {input = Buffer.alloc(0), env = {}, cwd = root, timeout = 900_000} = {}) {
  assert.match(label, /^[a-zA-Z0-9-]+$/u);
  const id = receipt.commands.length + 1;
  const prefix = `commands/${String(id).padStart(4, '0')}-${label}`;
  const stdin = retain(`${prefix}.stdin`, input);
  const run = spawnSync(binary, argv, {cwd, env, input, encoding: null,
    maxBuffer: 64 * 1024 * 1024, timeout, killSignal: 'SIGKILL'});
  const stdout = retain(`${prefix}.stdout`, run.stdout ?? Buffer.alloc(0));
  const stderr = retain(`${prefix}.stderr`, run.stderr ?? Buffer.alloc(0));
  receipt.commands.push({id, label, executable: binary, executable_sha256: watch(binary), argv,
    cwd, environment: env,
    stdin: {file: path.relative(output, stdin), sha256: digest(stdin)},
    stdout: {file: path.relative(output, stdout), sha256: digest(stdout)},
    stderr: {file: path.relative(output, stderr), sha256: digest(stderr)},
    status: run.status, signal: run.signal,
    error: run.error ? String(run.error) : null, timeout_ms: timeout});
  assert.equal(run.error, undefined, `${label}: process failed to launch`);
  assert.equal(run.signal, null, `${label}: process timed out`);
  return {id, status: run.status, stdout: run.stdout.toString('utf8'), stderr: run.stderr.toString('utf8')};
}
function walk(directory) {
  for (const item of fs.readdirSync(source(directory), {withFileTypes: true})
    .sort((left, right) => left.name.localeCompare(right.name))) {
    if (item.name === '.lake' || item.name === 'target' ||
        (directory === 'verification/intrinsic-named-v2' && item.name === 'acceptance.json')) continue;
    const name = `${directory}/${item.name}`;
    if (item.isDirectory()) walk(name);
    else {
      assert.ok(item.isFile() && fs.lstatSync(source(name)).isFile(), `non-file source ${name}`);
      snapshotSource(name);
    }
  }
}
function exact(bytes, before, after) {
  assert.ok(bytes.includes(before) && bytes.indexOf(before) === bytes.lastIndexOf(before), `not unique: ${before}`);
  return bytes.replace(before, after);
}
function checked(row, expected, label) {
  assert.equal(row.outcome, expected, `${label}: wrong actual outcome`);
  for (const key of ['candidate_executions', 'guest_requests', 'protected_operations', 'runtime_prover_calls'])
    assert.equal(row[key], 0, `${label}: ${key} is not zero`);
  assert.equal(row.proof_grants_host_authority, false, `${label}: proof authorized host action`);
}
function cleanRuntime(report, label) {
  for (const key of ['guest_requests', 'host_requests', 'protected_operations', 'ambient_fallback_calls'])
    assert.equal(report[key], 0, `${label}: protected request`);
  assert.equal(report.acquired_authority, false, `${label}: proof acquired authority`);
}
function artifactList(dir) {
  const entries = [];
  function visit(parent) {
    for (const item of fs.readdirSync(parent, {withFileTypes: true})
      .sort((left, right) => left.name.localeCompare(right.name))) {
      const file = path.join(parent, item.name);
      if (item.isDirectory()) visit(file);
      else { assert.ok(item.isFile()); entries.push({file: path.relative(output, file), sha256: watch(file), bytes: fs.statSync(file).size}); }
    }
  }
  visit(dir);
  return entries.sort((a,b) => a.file.localeCompare(b.file));
}
const names = ['module Definitions@5', 'import Definitions@5', 'module Subject@3'];
const oldReceipts = {'verification/intrinsic-proofs/acceptance.json':
  '2a7e3a9d98f7a1f8ec6326e0bfe13088ee3deae6bb2ce1fa111444c21d86876c',
'verification/intrinsic-proofs/source-bridge-acceptance.json':
  '1952957e30c9b7f071987e271c8df11797302056ef4b1a16e6c734bc26664f82'};
try {
  for (const [file, expected] of Object.entries(oldReceipts)) assert.equal(watch(source(file)), expected, 'historical receipt altered');
  receipt.historical_receipts_unchanged = oldReceipts;
  for (const name of ['Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', 'flake.lock', 'flake.nix',
    'policy/tool-selection.json', 'verification/m6/pins.json',
    'specs/conformance/contract-cases.json', '.cairn/changes/intrinsic-noble-proofs/design.md',
    '.cairn/changes/intrinsic-noble-proofs/tasks.md']) snapshotSource(name);
  for (const tree of ['crates/noble-contracts', 'crates/noble-kernel', 'crates/noble-cli',
    'crates/noble-wasm', 'proofs/mc1', '.cairn/changes/intrinsic-noble-proofs/specs',
    'verification/intrinsic-named-v2']) walk(tree);
  const canonical = JSON.parse(fs.readFileSync(source('specs/conformance/contract-cases.json')));
  const caseRows = canonical.cases.filter(row => row.id === 'CONTRACT-26');
  assert.equal(caseRows.length, 1, 'one frozen canonical CONTRACT-26 required');
  const target = caseRows[0];
  assert.equal(target.profile, 'Intrinsic-Proofs-Draft');
  assert.equal(target.input.harness, 'actual-named-two-call-source-proof-and-host-origin-admission');
  assert.equal(target.input.source_units.length, 3);
  target.input.source_units.forEach((unit,index) => assert.ok(unit.startsWith(names[index])));
  const [definitions, imported, subject] = target.input.source_units;
  assert.match(subject, /export twice def twice \[ d\.step d\.step \] contract 2/u);
  const canonicalVariants = target.input.variants.map(variant => variant.change);
  assert.equal(new Set(canonicalVariants).size, canonicalVariants.length);
  receipt.canonical = {source_sha256: receipt.sources['specs/conformance/contract-cases.json'],
    workload_sha256: hash(Buffer.from(JSON.stringify({input: target.input, expected: target.expected}))),
    changes: canonicalVariants};
  const cargo = freeze('cargo', tool('quality_rust', 'cargo'));
  freeze('rustc', tool('quality_rust', 'rustc'));
  freeze('node', node);
  const lean = tool('lean', 'lean');
  const wasmTools = freeze('wasm-tools', tool('wasm_tools', 'wasm-tools'));
  for (const [name, original] of Object.entries({lean,
    bubblewrap: '/nix/store/y0ra9qr3rz81d9wl7dfrldv3j25dc98q-bubblewrap-0.12.0/bin/bwrap',
    prlimit: '/nix/store/mqvbf0flamqaq9c3ihb496aahag897n0-util-linux-2.42.3-bin/bin/prlimit',
    'nix-store': '/run/current-system/sw/bin/nix-store',
    'systemd-run': '/nix/store/baxgs06659w4i6ggsfr56srjrb52v1h8-systemd-261.2/bin/systemd-run'})) freeze(name, original);
  const linker = path.join(pins.linker_bin, 'cc');
  const binutilsLinker = fs.realpathSync(path.join(pins.linker_bin, 'ld'));
  const mold = fs.realpathSync(`/etc/profiles/per-user/${path.basename(process.env.HOME)}/bin/mold`);
  const vendor = path.join(pins.vendor, 'source-registry-0');
  assert.ok(fs.statSync(vendor).isDirectory(), 'offline vendor missing');
  receipt.tools.linker = {original: linker, sha256: watch(linker)};
  receipt.tools.binutils_linker = {original: binutilsLinker, sha256: watch(binutilsLinker)};
  receipt.tools.mold = {original: mold, sha256: watch(mold)};
  const cargoConfigFile = path.join(process.env.HOME, '.cargo/config.toml');
  receipt.tools.cargo_config = {original: cargoConfigFile, sha256: watch(cargoConfigFile)};
  assert.match(fs.readFileSync(cargoConfigFile, 'utf8'), /link-arg=-fuse-ld=mold/u);
  const tmp = path.join(output, 'tmp'), home = path.join(output, 'home');
  fs.mkdirSync(tmp); fs.mkdirSync(home);
  const cargoConfig = ['--config', 'source.crates-io.replace-with="m6-vendor"',
    '--config', `source.m6-vendor.directory="${vendor}"`];
  const buildEnv = {PATH: `${path.dirname(mold)}:${path.dirname(binutilsLinker)}:${path.dirname(linker)}:${path.dirname(cargo)}:/run/current-system/sw/bin`,
    HOME: process.env.HOME, TMPDIR: tmp, CARGO_HOME: path.join(process.env.HOME, '.cargo'),
    CARGO_TARGET_DIR: source('target'), CARGO_NET_OFFLINE: 'true', CARGO_BUILD_JOBS: '2',
    RUSTC: tool('quality_rust', 'rustc'), RUSTC_WRAPPER: '', CARGO_BUILD_RUSTC_WRAPPER: '',
    CC: linker, LD: binutilsLinker, COMPILER_PATH: path.dirname(binutilsLinker),
    LANG: 'C.UTF-8', LC_ALL: 'C.UTF-8'};
  for (const [label, argv] of [
    ['build-cli', ['build', '--manifest-path', 'Cargo.toml', '-p', 'noble-cli', '--bin', 'noble', '--locked', '--offline', ...cargoConfig]],
    ['build-graph-peer', ['build', '--manifest-path', 'verification/intrinsic-named-v2/peer/Cargo.toml',
      '--bin', 'noble-intrinsic-named-v2-peer', '--locked', '--offline', ...cargoConfig]]]) {
    const result = command(label, cargo, argv, {env: buildEnv, timeout: 3_600_000});
    assert.equal(result.status, 0, `${label}: strict offline Rust build failed: ${result.stderr.slice(-1500)}`);
  }
  const cli = freeze('noble', source('target/debug/noble'));
  assert.equal(digest(cli), process.argv[3], 'fresh selected CLI does not match reviewed source-stable binary');
  const peer = freeze('graph-peer', source('target/debug/noble-intrinsic-named-v2-peer'));
  const env = {PATH: '/run/current-system/sw/bin', HOME: home, TMPDIR: tmp,
    LANG: 'C.UTF-8', LC_ALL: 'C.UTF-8', XDG_RUNTIME_DIR: process.env.XDG_RUNTIME_DIR,
    NOBLE_LEAN: lean, NOBLE_BWRAP: receipt.tools.bubblewrap.original,
    NOBLE_PRLIMIT: receipt.tools.prlimit.original,
    NOBLE_SYSTEMD_RUN: receipt.tools['systemd-run'].original,
    NOBLE_CONTRACT_LIBRARY: source('proofs/mc1'), NOBLE_NIX_STORE: receipt.tools['nix-store'].original};
  function fixture(label, text) {
    const file = retain(`inputs/${label}.noble`, Buffer.from(text));
    receipt.fixtures[label] = {file: path.relative(output, file), sha256: digest(file)};
    return file;
  }
  const defFile = fixture('Definitions-5', definitions);
  const importFile = fixture('import-Definitions-5', imported);
  const subjectFile = fixture('Subject-3', subject);
  const bindingFile = retain('inputs/empty-bindings.txt', Buffer.alloc(0));
  const base = [defFile, importFile, subjectFile];
  const peerRun = command('public-api-graph-mutations', peer, base, {env, timeout: 300_000});
  assert.equal(peerRun.status, 0, `public API graph peer failed: ${peerRun.stderr}`);
  const graph = JSON.parse(peerRun.stdout);
  assert.equal(graph.result, 'passed');
  assert.deepEqual(graph.source_sha256, base.map(digest));
  assert.equal(graph.guest_requests, 0); assert.equal(graph.protected_operations, 0);
  assert.equal(hash(Buffer.from(graph.generated_statement)), graph.generated_statement_sha256);
  assert.match(graph.generated_statement, /NamedV2\.exportedNamedClaim subject \[\]/u);
  assert.match(graph.generated_statement, /Holds₂ subject\.env \(\.bool true\)/u);
  assert.match(graph.generated_statement, /Holds₂ subject\.env \(\.eq \(\.output 0\) \(\.add \(\.input 0\) \(\.i64 2\)\)\)/u);
  assert.match(graph.generated_statement, /private theorem typedA : NamedV1\.CodeTyped/u);
  assert.match(graph.generated_statement, /private theorem typedB : NamedV1\.CodeTyped/u);
  assert.match(graph.generated_statement, /private theorem typedRoot : NamedV1\.CodeTyped/u);
  assert.doesNotMatch(graph.generated_statement, /NamedV2\.Examples\.proof|NamedV1\.Satisfies/u);
  const mutations = new Map(graph.mutations.map(item => [item.change, item]));
  assert.equal(mutations.size, graph.mutations.length);
  assert.equal(mutations.get('call-byte-span-covers-only-d-instead-of-full-d.step-original-token')?.diagnostic,
    'named call differs from exact original word token');
  assert.equal(mutations.get('call-byte-span-covers-wrong-original-word-token-with-ordered-nonempty-span')?.diagnostic,
    'original named word has no retained import alias');
  assert.equal(mutations.get('original-imported-donor-not-exported')?.diagnostic,
    'named source definition is absent or not exported');
  const staleChange = 'replay-previously-prepared-Definitions@5-source-bound-goal-graph-and-claim-after-d-alias-rebind-to-Definitions@6-with-identical-d.step-token-text';
  const stale = mutations.get(staleChange);
  assert.equal(stale.stage, 'acceptance');
  assert.equal(stale.checker_callback_called, false);
  assert.equal(stale.generation_before, stale.generation_after);
  assert.equal(stale.fresh_v6_prepared_and_source_checked, true);
  function verify(label, changed, units = [definitions, imported]) {
    const file = fixture(label, changed);
    const dependencyFiles = units.map((unit,index) => fixture(`${label}-dependency-${index}`, unit));
    const run = command(`verify-${label}`, cli,
      ['verify-module', file, ...dependencyFiles.flatMap(dep => ['--module', dep]), '--timeout-ms', '600000'],
      {env, timeout: 900_000});
    const report = JSON.parse(run.stdout.trim());
    assert.equal(report.command, 'verify-module');
    assert.equal(report.source_sha256, digest(file), `${label}: wrong submitted source`);
    assert.equal(report.guest_requests, 0);
    assert.equal(report.candidate_executions, 0);
    assert.equal(report.protected_operations, 0);
    assert.equal(report.proof_grants_host_authority, false);
    return {file, command: run.id, status: run.status, report};
  }
  const positive = verify('positive', subject);
  assert.equal(positive.status, 0);
  checked(positive.report, 'proved', 'positive');
  assert.equal(positive.report.stage, 'independent-recheck');
  assert.equal(positive.report.schema, 'noble-intrinsic-module-report/v2');
  assert.equal(positive.report.profile, 'Named-Contracts-v2');
  assert.equal(positive.report.independent_recheck, true);
  const proof = positive.report.proofs[0];
  assert.equal(positive.report.proofs.length, 1);
  assert.equal(proof.kind, 'named-contract');
  assert.equal(proof.claim, 'NamedV2Obligation.claim');
  assert.equal(proof.subject.generated_statement_sha256, graph.generated_statement_sha256,
    'public API regenerated statement differs from strict CLI statement');
  assert.equal(hash(Buffer.from(proof.lowered_term)), graph.source_proof_term_sha256,
    'public API source proof differs from consumer proof term');
  assert.deepEqual([...proof.strict_lean_axioms].sort(),
    ['Classical.choice', 'Quot.sound', 'propext'].sort());
  assert.equal(proof.strict_lean_tools.lean, lean);
  assert.equal(proof.strict_lean_tools.bubblewrap, receipt.tools.bubblewrap.original);
  assert.equal(proof.strict_lean_tools.prlimit, receipt.tools.prlimit.original);
  assert.equal(proof.strict_lean_tools.systemd_run, receipt.tools['systemd-run'].original);
  assert.match(proof.model_revision, /^[a-f0-9]{64}$/u);
  assert.match(proof.checker_revision, /^[a-f0-9]{64}$/u);
  assert.match(proof.consumer_wire_sha256, /^[a-f0-9]{64}$/u);
  assert.ok(positive.report.work.total > 0, 'strict source proof did no checked work');
  assert.match(proof.lowered_term, /NamedV2\.twoStepClaim/u);
  assert.doesNotMatch(proof.lowered_term, /Examples\.proof|NamedV1\.Satisfies/u);
  assert.equal(proof.subject.generated_claim, 'NamedV2Obligation.claim');
  const observed = proof.subject;
  const uses = observed.definition_body_uses;
  assert.equal(observed.named_use_count, 3);
  assert.equal(uses.length, 2);
  assert.equal(observed.original_subject.module_source_sha256, digest(subjectFile));
  assert.equal(observed.original_subject.module, 'Subject');
  assert.equal(observed.original_subject.version, 3);
  assert.equal(observed.original_subject.definition, 'twice');
  assert.equal(observed.resolved_imports.length, 1);
  assert.equal(observed.resolved_imports[0].alias, 'd');
  assert.equal(observed.resolved_imports[0].owner_session_local, uses[0].original.owner_session_local);
  assert.equal(observed.resolved_imports[0].original_module_index, graph.donor_module_index);
  assert.notEqual(graph.donor_module_index, graph.root_module_index);
  assert.notEqual(observed.resolved_imports[0].owner_session_local,
    observed.original_subject.owner_session_local);
  const slice = (text, span) => Buffer.from(text).subarray(span.start, span.end);
  assert.equal(slice(subject, observed.original_subject.definition_span).toString(),
    'def twice [ d.step d.step ]');
  assert.notEqual(observed.accepted_submission_root.specialization_slot, uses[0].accepted.specialization_slot);
  assert.deepEqual(observed.accepted_submission_root.inst, []);
  assert.deepEqual(observed.accepted_submission_root.derivation,
    {stack_in: ['I64'], stack_out: ['I64'], effects: 'EffSet([])'});
  const positions = [...subject.matchAll(/d\.step/gu)].map(match => match.index);
  assert.deepEqual(positions.length, 2);
  for (const [index, use] of uses.entries()) {
    assert.equal(use.order, index);
    assert.equal(use.original.module, 'Definitions');
    assert.equal(use.original.version, 5);
    assert.equal(use.original.module_source_sha256, digest(defFile));
    assert.equal(use.original.definition, 'step');
    assert.equal(use.original.ordinal, 0);
    assert.equal(slice(definitions, use.original.definition_span).toString(), 'def step [ 1 + ]');
    assert.equal(slice(definitions, use.original.body_span).toString(), '[ 1 + ]');
    assert.equal(use.original.definition_sha256, hash(slice(definitions, use.original.definition_span)));
    assert.equal(use.caller.module, 'Subject');
    assert.equal(use.caller.definition, 'twice');
    assert.equal(use.caller.lexical_span.start, positions[index]);
    assert.equal(use.caller.lexical_span.end, positions[index] + 6);
    assert.equal(slice(subject, use.caller.lexical_span).toString(), 'd.step');
    assert.equal(use.caller.source_node, index);
    assert.equal(use.caller.candidate_node, index);
    assert.deepEqual(use.accepted.inst, []);
    assert.deepEqual(use.accepted.derivation.stack_in, ['I64']);
    assert.deepEqual(use.accepted.derivation.stack_out, ['I64']);
    assert.equal(use.accepted.derivation.effects, 'EffSet([])');
    const specialization = use.accepted.specialization_body;
    assert.equal(specialization.candidate_format, 1);
    assert.equal(specialization.candidate_revision, 0);
    assert.deepEqual(specialization.interface, {stack_in: ['I64'], stack_out: ['I64'],
      effects: 'EffSet([])'});
    assert.equal(specialization.nodes.length, 2);
    assert.deepEqual(specialization.nodes.map(node => node.candidate_node), [0, 1]);
    assert.deepEqual(specialization.nodes[0].node,
      {kind: 'literal', value: 'I64(1)', inst: [{kind: 'stack', types: ['I64']}]});
    assert.deepEqual(specialization.nodes[0].derivation.stack_in, ['I64']);
    assert.deepEqual(specialization.nodes[0].derivation.stack_out, ['I64', 'I64']);
    assert.deepEqual(specialization.nodes[1].node,
      {kind: 'invocation', slot: 4, inst: [{kind: 'stack', types: []}]});
    assert.deepEqual(specialization.nodes[1].derivation.stack_in, ['I64', 'I64']);
    assert.deepEqual(specialization.nodes[1].derivation.stack_out, ['I64']);
    for (const node of specialization.nodes) assert.equal(node.derivation.effects, 'EffSet([])');
    assert.equal(use.original.owner_session_local, uses[0].original.owner_session_local);
    assert.equal(use.original.definition_sha256, uses[0].original.definition_sha256);
  }
  assert.deepEqual(uses.map(use => use.accepted.specialization_slot), graph.call_slots);
  assert.deepEqual(uses.map(use => use.caller.source_node), graph.source_nodes);
  assert.notEqual(graph.call_slots[0], graph.call_slots[1]);
  assert.notEqual(uses[0].caller.lexical_span.start, uses[1].caller.lexical_span.start);
  assert.notEqual(observed.original_subject.owner_session_local, uses[0].original.owner_session_local);
  const freshDefinition = exact(definitions, 'Definitions@5', 'Definitions@6');
  const freshImport = exact(imported, 'Definitions@5', 'Definitions@6');
  const freshV6 = verify('fresh-Definitions-6-independent-proof', subject,
    [freshDefinition, freshImport]);
  assert.equal(freshV6.status, 0, 'fresh source-authenticated @6 named claim refused');
  checked(freshV6.report, 'proved', 'fresh authenticated @6');
  assert.equal(freshV6.report.stage, 'independent-recheck');
  assert.equal(freshV6.report.independent_recheck, true);
  assert.deepEqual([...freshV6.report.proofs[0].strict_lean_axioms].sort(),
    ['Classical.choice', 'Quot.sound', 'propext'].sort());
  assert.equal(freshV6.report.proofs[0].subject.definition_body_uses.length, 2);
  for (const use of freshV6.report.proofs[0].subject.definition_body_uses) {
    assert.equal(use.original.module, 'Definitions');
    assert.equal(use.original.version, 6);
    assert.equal(use.original.module_source_sha256, hash(Buffer.from(freshDefinition)));
    assert.equal(slice(freshDefinition, use.original.definition_span).toString(),
      'def step [ 1 + ]');
  }
  assert.notEqual(freshV6.report.proofs[0].subject.generated_statement_sha256,
    proof.subject.generated_statement_sha256,
    'independent fresh @6 claim improperly reused stale @5 source provenance');
  function framed(files) { return Buffer.concat(files.flatMap(file => {const bytes = fs.readFileSync(file); return [Buffer.from(`${bytes.length}\n`), bytes];})); }
  function session(label, files, opt, extraEnv = {}) {
    const emit = path.join(output, 'emission', label);
    fs.mkdirSync(path.dirname(emit), {recursive: true});
    const run = command(`session-${label}`, cli,
      ['session', '--framed', '--declared-modules', '--bindings', bindingFile, '--opt', opt, '--emit', emit],
      {env: {...env, ...extraEnv}, input: framed(files), timeout: 900_000});
    const reports = run.stdout.trimEnd().split('\n').map(line => JSON.parse(line));
    assert.equal(reports.length, files.length, `${label}: wrong frame report count`);
    const artifacts = artifactList(emit);
    files.forEach((file,index) => assert.deepEqual(fs.readFileSync(path.join(emit, `${index + 1}.noble`)), fs.readFileSync(file)));
    const wasm = artifacts.filter(item => item.file.endsWith('.wasm'));
    assert.ok(wasm.length > 0, `${label}: compiled Wasm missing`);
    for (const [index, artifact] of wasm.entries()) {
      for (const op of ['validate', 'print']) {
        const inspected = command(`wasm-${label}-${op}-${index}`, wasmTools,
          [op, path.join(output, artifact.file)], {env: {PATH: '/nonexistent', LANG: 'C', LC_ALL: 'C'}});
        assert.equal(inspected.status, 0, `${label}: wasm-tools ${op} failed`);
        if (op === 'print') assert.doesNotMatch(inspected.stdout,
          /twice-correct|export-named-unary-I64|twoStepClaim|NamedV2Obligation|proof-service/iu);
      }
    }
    return {command: run.id, status: run.status, reports, wasm};
  }
  const invoke41 = fixture('invoke-41', '41 Subject@3.twice');
  const invokeMax = fixture('invoke-max', '9223372036854775807 Subject@3.twice');
  const reflectResult = fixture('quote-reflect-result', '41 Subject@3.twice quote reflect');
  const reflectDefinition = fixture('reflect-definition', '[ Subject@3.twice ] reflect');
  const importSubject = fixture('import-Subject-3', 'import Subject@3 as Subject');
  // Source declarations are separate frames; guest calls are separate from the
  // authenticated subject invocation used to inspect the definition body.
  function runtime(label, subjectSource, mode, proofFree = false) {
    const run = session(label, [defFile, importFile, subjectSource, importSubject,
      invoke41, invokeMax, reflectResult, reflectDefinition],
      mode, proofFree ? {NOBLE_LEAN: '/nonexistent/proof-service-disabled',
        NOBLE_CONTRACT_LIBRARY: '/nonexistent/proof-library-disabled'} : {});
    assert.equal(run.status, 0, `${label}: source/runtime failed`);
    for (const row of run.reports) cleanRuntime(row, label);
    const first = {handle: null, type: 'I64', value: '43'};
    const second = {handle: null, type: 'I64', value: '-9223372036854775807'};
    for (const [index, expected] of [[4, [first]], [5, [first, second]]]) {
      const row = run.reports[index];
      assert.equal(row.stage, 'wasm'); assert.equal(row.outcome, 'normal');
      assert.deepEqual(row.stack, expected, `${label}: framed invocation lost its retained stack`);
      assert.equal(row.runtime_prover_calls, 0);
      assert.deepEqual(row.effect_requests, []); assert.deepEqual(row.request_trace, []);
      assert.equal(row.request_trace_complete, true);
    }
    const reflectedValue = run.reports[6];
    assert.equal(reflectedValue.stage, 'wasm');
    assert.equal(reflectedValue.outcome, 'normal');
    const valueHandle = reflectedValue.stack[2]?.handle;
    assert.ok(Number.isSafeInteger(valueHandle) && valueHandle > 0);
    assert.deepEqual(reflectedValue.stack, [first, second,
      {handle: valueHandle, type: 'Syntax',
        recipe: [{literal: {type: 'I64', value: '43'}}], witnesses: []}]);
    const reflectedDefinition = run.reports[7];
    assert.equal(reflectedDefinition.stage, 'wasm');
    assert.equal(reflectedDefinition.outcome, 'normal');
    const definition = reflectedDefinition.stack[3];
    assert.ok(Number.isSafeInteger(definition?.handle) && definition.handle > valueHandle);
    assert.match(definition.recipe?.[0]?.invoke, /^definition:[0-9]+$/u);
    assert.deepEqual(reflectedDefinition.stack, [...reflectedValue.stack,
      {handle: definition.handle, type: 'Syntax',
        recipe: [{invoke: definition.recipe[0].invoke}],
        witnesses: [{node: 0, stack_in: '[I64]', stack_out: '[I64]', effects: []}]}]);
    return run;
  }
  const without = fixture('proof-free', exact(subject,
    ' contract 2 twice-law [ subject twice input [ x I64 ] output [ y I64 ] requires [ true ] ensures [ (eq (out y) (add (in x) 2)) ] ] proof 2 twice-correct for twice-law [ (export-named-unary-I64 (pc-named-call 0 (exec-literal-1) (exec-add)) (pc-named-call 1 (exec-literal-1) (exec-add)) (by-exact-tail) (named-true-eq-wrap)) ]', ''));
  const proofOnly = fixture('proof-only-change', exact(subject, 'proof 2 twice-correct for', 'proof 2 alternate-correct for'));
  const runtimeResults = {};
  for (const mode of ['off', 'on']) {
    const baseline = runtime(`proof-free-${mode}`, without, mode, true);
    const bearing = runtime(`proof-bearing-${mode}`, subjectFile, mode);
    const changed = runtime(`proof-only-change-${mode}`, proofOnly, mode);
    assert.deepEqual(bearing.wasm.map(item => item.sha256), baseline.wasm.map(item => item.sha256), `${mode}: proof changed executable Wasm`);
    assert.deepEqual(changed.wasm.map(item => item.sha256), baseline.wasm.map(item => item.sha256), `${mode}: proof edit changed executable Wasm`);
    assert.deepEqual(bearing.reports.slice(4).map(row => row.stack), baseline.reports.slice(4).map(row => row.stack));
    assert.deepEqual(changed.reports.slice(4).map(row => row.stack), baseline.reports.slice(4).map(row => row.stack));
    runtimeResults[mode] = {proof_free: baseline.command, proof_bearing: bearing.command,
      proof_only: changed.command, wasm_sha256: baseline.wasm.map(item => item.sha256)};
  }
  const bodyWrong = exact(subject, 'd.step d.step', 'd.step d.step d.step');
  const wrongLiteral = exact(definitions, 'def step [ 1 + ]', 'def step [ 2 + ]');
  const invalids = new Map([
    ['wrong-second-body-call-or-replaced-literal', () => verify('wrong-second-body-call', bodyWrong)],
    ['wrong-alias-name', () => verify('wrong-alias-name', subject,
      [definitions, 'import Definitions@5 as wrong'])],
    ['requires-false-or-weakened-ensures-against-selected-exact-True-plus-two-rule', () => verify('requires-false', exact(subject, 'requires [ true ]', 'requires [ false ]'))],
    ['false-x-plus-two-theorem-or-NamedV1-Satisfies-instead-of-typed-claim', () => verify('false-plus-three', exact(subject, '(add (in x) 2)', '(add (in x) 3)'))],
    ['proof-2-for-contract-1-or-proof-1-for-contract-2', () => verify('cross-version', exact(subject, 'proof 2 twice-correct', 'proof 1 twice-correct'))],
    ['proof-2-colon-standalone-or-params-field', () => verify('colon-proof', exact(subject, 'proof 2 twice-correct for twice-law', 'proof 2 twice-correct : twice-law'))],
    ['quote-or-reflect-program-identity-substituted-for-named-definition-body-or-submitted-invocation', () => verify('quote-body', exact(subject, 'def twice [ d.step d.step ]', 'def twice [ [ d.step ] run d.step ]'))],
  ]);
  const extra = [
    ['wrong-literal', () => verify('wrong-literal', subject, [wrongLiteral, imported])],
    ['named-v1-satisfies', () => verify('named-v1-satisfies', exact(subject,
      '(export-named-unary-I64 (pc-named-call 0 (exec-literal-1) (exec-add)) (pc-named-call 1 (exec-literal-1) (exec-add)) (by-exact-tail) (named-true-eq-wrap))',
      '(NamedV1.Satisfies)'))],
    ['weakened-ensures', () => verify('weakened-ensures', exact(subject, '(eq (out y) (add (in x) 2))', 'true'))],
    ['cross-contract-version', () => verify('cross-contract-version', exact(subject, 'contract 2 twice-law', 'contract 1 twice-law'))],
    ['params-field', () => verify('params-field', exact(subject, 'input [ x I64 ]', 'params [ ] input [ x I64 ]'))],
    ['second-call-wrong', () => verify('second-call-wrong', exact(subject, 'd.step d.step', 'd.step 2'))],
    ['reflect-substituted-proof', () => verify('reflect-substituted-proof', exact(subject,
      '(export-named-unary-I64 (pc-named-call 0 (exec-literal-1) (exec-add)) (pc-named-call 1 (exec-literal-1) (exec-add)) (by-exact-tail) (named-true-eq-wrap))',
      '(reflect [ d.step d.step ])'))],
  ];
  const results = new Map();
  for (const [name, run] of [...invalids, ...extra]) {
    const result = run();
    assert.notEqual(result.status, 0, `${name}: hostile source was proved`);
    assert.notEqual(result.report.outcome, 'proved', `${name}: hostile source admitted`);
    results.set(name, {command: result.command, outcome: result.report.outcome,
      stage: result.report.stage, diagnostic: result.report.diagnostic});
  }
  for (const name of ['requires-false-or-weakened-ensures-against-selected-exact-True-plus-two-rule',
    'weakened-ensures']) {
    assert.ok(['check', 'proof-check'].includes(results.get(name)?.stage),
      `${name}: refused outside selected exact assertion preparation/proof check`);
  }
  for (const [name, attack] of mutations) {
    assert.equal(attack.outcome, 'rejected', `${name}: graph mutation admitted`);
    results.set(name, {command: peerRun.id, outcome: 'rejected-public-API-graph',
      ...(name === staleChange ? {fresh_independent_proof_command: freshV6.command} : {})});
  }
  for (const change of canonicalVariants) {
    let evidence;
    if (change.startsWith('positive-authenticated-')) evidence = {command: positive.command, outcome: 'proved'};
    else if (change.startsWith('proof-only-change-')) evidence = {command: runtimeResults.off.proof_only, outcome: 'identical-executable-wasm'};
    else evidence = results.get(change);
    assert.ok(evidence, `canonical variant has no actual source or public API observation: ${change}`);
    receipt.cases.push({change, expected: target.input.variants.find(item => item.change === change).expected,
      observation: evidence, passed: true});
  }
  receipt.additional_controls = Object.fromEntries(results);
  receipt.runtime = runtimeResults;
  receipt.positive = {command: positive.command, strict_axioms: proof.strict_lean_axioms,
    claim_sha256: graph.generated_statement_sha256, peer_command: peerRun.id,
    fresh_v6_independent_proof_command: freshV6.command,
    source_graph: {root: graph.root_slot, calls: graph.call_slots, lexical_nodes: graph.source_nodes}};
  assert.equal(receipt.cases.length, canonicalVariants.length);
  receipt.source_revision = `sha256:${hash(Buffer.from(JSON.stringify({sources: receipt.sources,
    canonical: receipt.canonical.workload_sha256, tools: Object.fromEntries(Object.entries(receipt.tools)
      .map(([name,value]) => [name,value.sha256])), binary: digest(cli), peer: digest(peer)})))}`;
} catch (error) {
  receipt.failures.push({case_id: 'CONTRACT-26', error: String(error.stack ?? error)});
} finally {
  for (const [file, sha256] of watched) {
    try { assert.equal(digest(file), sha256, `changed while acceptance ran: ${file}`); }
    catch (error) { receipt.integrity_failures.push({file, error: String(error)}); }
  }
  receipt.summary = {variants: receipt.cases.length, commands: receipt.commands.length,
    failures: receipt.failures.length, integrity_failures: receipt.integrity_failures.length};
  receipt.result = receipt.failures.length === 0 && receipt.integrity_failures.length === 0 &&
    receipt.cases.length === receipt.canonical?.changes.length ? 'passed' : 'failed';
  const report = path.join(output, 'report.json');
  fs.writeFileSync(report, JSON.stringify(receipt, null, 2) + '\n', {flag: 'wx', mode: 0o400});
  let portable = null;
  if (receipt.result === 'passed') {
    portable = source(receipt.portable_receipt_self_exclusion);
    fs.copyFileSync(report, portable, fs.constants.COPYFILE_EXCL);
    fs.chmodSync(portable, 0o400);
    assert.equal(digest(portable), digest(report), 'portable receipt differs from immutable external receipt');
  }
  console.log(JSON.stringify({schema: receipt.schema, result: receipt.result, report,
    sha256: digest(report), portable, summary: receipt.summary}));
  process.exitCode = receipt.result === 'passed' ? 0 : 1;
}
