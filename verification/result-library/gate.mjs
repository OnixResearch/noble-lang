#!/usr/bin/env node
// A passed receipt requires real current-source CLI/peer binaries, compiled
// managed-memory Wasm, selected host traces, negative static checks and raw logs.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { selectedVendor } from '../selected-vendor.mjs';
import { Recorder, root, fileHash, sha256 } from './record.mjs';
import { preparePlan, bindRevision } from './plan.mjs';
import { librarySource, programs, bindings, canonicalCases, hostBinding } from './fixtures.mjs';

assert.equal(process.argv.length, 5,
  'usage: SELECTED_NODE verification/result-library/gate.mjs FRESH_NOBLE CLI_BUILD_RECEIPT NEW_EXTERNAL_DIRECTORY');
const record = new Recorder(process.argv[4]);
let plan;
let cli;
let peer;
let wasmTools;
const asset = (group, name) => {
  const entry = plan[group][name];
  assert.ok(entry, `unreviewed ${group} source: ${name}`);
  const file = path.join(record.output, entry.file);
  assert.equal(fileHash(file), entry.sha256, `${name}: frozen source changed`);
  return file;
};
const source = name => asset('program_bytes', name);
const binding = name => asset('bindings', name);
const snapshot = (file, name) => {
  record.watch(file);
  return record.retain(name, fs.readFileSync(file));
};

function buildProvenance(file) {
  const buildRoot = path.dirname(fs.realpathSync(file));
  assert.equal(path.resolve(file), path.join(buildRoot, 'build.json'),
    'CLI build receipt must belong to its fresh isolated output');
  const receipt = JSON.parse(fs.readFileSync(file));
  assert.equal(receipt.schema, 'noble-result-cli-build/v1');
  assert.equal(receipt.result, 'built');
  assert.ok(receipt.binary && Array.isArray(receipt.commands), 'missing build binary/commands');
  assert.equal(path.resolve(receipt.binary.path), path.resolve(process.argv[2]));
  assert.equal(receipt.binary.path, path.join(buildRoot, 'target/debug/noble'),
    'CLI executable must be the isolated Cargo build output');
  assert.equal(fileHash(receipt.binary.path), receipt.binary.sha256,
    'supplied CLI differs from the freshly built source artifact');
  assert.match(receipt.sourceRevision, /^[0-9a-f]{40}$/,
    'CLI build must retain the exact Git HEAD alongside its full source hashes');
  assert.equal(receipt.postSourceRevision, receipt.sourceRevision,
    'CLI build Git HEAD changed during source-stable build');
  const head = spawnSync('/run/current-system/sw/bin/git', ['rev-parse', 'HEAD'],
    { cwd: root, env: { PATH: '/run/current-system/sw/bin' },
      encoding: 'utf8', timeout: 30_000 });
  assert.equal(head.status, 0, 'cannot identify current reviewed Git HEAD');
  assert.equal(receipt.sourceRevision, head.stdout.trim(),
    'CLI build Git HEAD does not match current reviewed checkout');
  assert.deepEqual(receipt.preSources, receipt.postSources,
    'source changed while the production CLI was built');
  assert.deepEqual(receipt.sources, receipt.postSources,
    'CLI source inventory differs from the post-build snapshot');
  const expectedCrates = Object.keys(plan.sources).filter(name => name.startsWith('crates/')).sort();
  const builtCrates = Object.keys(receipt.sources).filter(name => name.startsWith('crates/')).sort();
  assert.deepEqual(builtCrates, expectedCrates,
    'fresh CLI build source inventory differs from reviewed acceptance crates');
  const requiredInputs = ['Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml',
    'policy/tool-selection.json', 'verification/m6/pins.json',
    'nix/reviewed-vendor.nix', 'verification/selected-vendor.mjs',
    'verification/result-library/collect-build.mjs'];
  assert.deepEqual(Object.keys(receipt.sources).sort(), [...expectedCrates, ...requiredInputs].sort(),
    'fresh CLI build omitted or added a source/tool policy input');
  for (const [name, item] of Object.entries(receipt.sources)) {
    assert.ok(plan.sources[name], `unreviewed CLI build source: ${name}`);
    assert.equal(plan.sources[name].sha256, item.sha256,
      `${name}: CLI build and gate did not use identical source bytes`);
  }
  for (const name of ['Cargo.toml', 'Cargo.lock', librarySource,
    'verification/result-library/collect-build.mjs'])
    assert.ok(receipt.sources[name], `${name}: absent from fresh CLI build receipt`);
  const selected = JSON.parse(fs.readFileSync(path.join(root, 'policy/tool-selection.json')));
  const m6 = JSON.parse(fs.readFileSync(path.join(root, 'verification/m6/pins.json')));
  const reviewedVendor = selectedVendor(m6);
  const expectedRust = selected.tool_paths.quality_rust.output;
  for (const tool of ['cargo', 'rustc']) {
    const expected = path.join(expectedRust, 'bin', tool);
    assert.equal(receipt.tools?.[tool]?.path, expected,
      `${tool}: build did not use the selected quality Rust toolchain`);
    assert.equal(fileHash(expected), receipt.tools[tool].sha256,
      `${tool}: build tool byte identity changed`);
    record.watch(expected);
  }
  assert.equal(receipt.tools?.vendor?.path, reviewedVendor.directory,
    'CLI was not built against the reviewed offline vendor');
  assert.equal(receipt.tools.vendor.nar_hash, reviewedVendor.narHash,
    'CLI build did not authenticate the reviewed vendor NAR');
  const linker = fs.realpathSync(path.join(m6.linker_bin, 'ld'));
  const mold = fs.realpathSync(
    `/etc/profiles/per-user/${path.basename(process.env.HOME)}/bin/mold`);
  assert.equal(receipt.tools?.mold?.path, mold, 'CLI did not select reviewed mold linker');
  assert.equal(receipt.tools.mold.sha256, fileHash(mold), 'selected mold linker bytes changed');
  record.watch(mold);
  assert.equal(receipt.commands.length, 1, 'exactly one production CLI build required');
  const commands = receipt.commands.map((command, index) => {
    assert.equal(command.status, 0, `CLI build command ${index} did not succeed`);
    assert.ok(Array.isArray(command.argv) && command.argv.length > 0
      && command.env && typeof command.env === 'object',
    `CLI build command ${index} omitted argv/environment`);
    assert.deepEqual(command.argv, [path.join(expectedRust, 'bin/cargo'), 'build',
      '--manifest-path', 'Cargo.toml', '-p', 'noble-cli', '--bin', 'noble',
      '--locked', '--offline',
      '--config', 'source.crates-io.replace-with="m6-vendor"',
      '--config', `source.m6-vendor.directory="${reviewedVendor.directory}"`],
    'production CLI did not build with exact selected Cargo and locked offline vendor flags');
    assert.equal(command.cwd, root, 'production CLI build ran outside reviewed source root');
    assert.deepEqual(command.env, {
      PATH: `${path.dirname(mold)}:${path.dirname(linker)}:${m6.linker_bin}:${path.join(expectedRust, 'bin')}:/run/current-system/sw/bin`,
      COMPILER_PATH: path.dirname(linker), HOME: path.join(buildRoot, 'home'),
      CARGO_HOME: path.join(buildRoot, 'cargo'),
      CARGO_TARGET_DIR: path.join(buildRoot, 'target'),
      CARGO_NET_OFFLINE: 'true', CARGO_BUILD_JOBS: '2', CARGO_INCREMENTAL: '0',
      RUSTC: path.join(expectedRust, 'bin/rustc'),
      RUSTDOC: path.join(expectedRust, 'bin/rustdoc'),
      CC: path.join(m6.linker_bin, 'cc'), LD: linker,
      RUSTFLAGS: '-C link-arg=-fuse-ld=mold',
      RUSTC_WRAPPER: '', RUSTC_WORKSPACE_WRAPPER: '',
      CARGO_BUILD_RUSTC_WRAPPER: '',
      LANG: 'C.UTF-8', LC_ALL: 'C.UTF-8', USER: process.env.USER ?? 'nobody',
      TMPDIR: path.join(buildRoot, 'tmp'),
    }, 'production CLI command used an unreviewed Cargo, compiler or linker environment');
    const logs = {};
    for (const stream of ['stdout', 'stderr']) {
      const log = command[stream];
      assert.ok(log && typeof log.path === 'string' && typeof log.sha256 === 'string',
        `CLI build command ${index} omitted raw ${stream}`);
      assert.equal(fileHash(log.path), log.sha256,
        `CLI build command ${index} ${stream} was altered`);
      const copied = snapshot(log.path, `plan/build/command-${index}-${stream}.log`);
      logs[stream] = { file: record.relative(copied), sha256: log.sha256 };
    }
    return { status: command.status, argv: command.argv, env: command.env, logs };
  });
  const retained = snapshot(file, 'plan/build/cli-build.json');
  record.receipt.build = { file: record.relative(retained), sha256: fileHash(retained),
    git_head: receipt.sourceRevision, binary_sha256: receipt.binary.sha256,
    selected_cargo_sha256: receipt.tools.cargo.sha256,
    selected_rustc_sha256: receipt.tools.rustc.sha256,
    selected_mold_sha256: receipt.tools.mold.sha256,
    selected_vendor: receipt.tools.vendor.path, selected_vendor_nar_hash: reviewedVendor.narHash,
    commands };
}

function buildPeer() {
  const selected = JSON.parse(fs.readFileSync(path.join(root, 'policy/tool-selection.json')));
  const m6 = JSON.parse(fs.readFileSync(path.join(root, 'verification/m6/pins.json')));
  const rust = selected.tool_paths.quality_rust.output;
  const cargo = path.join(rust, 'bin/cargo');
  const rustc = path.join(rust, 'bin/rustc');
  const rustdoc = path.join(rust, 'bin/rustdoc');
  const vendor = path.join(m6.vendor, 'source-registry-0');
  assert.equal(record.watch(cargo), record.receipt.build.selected_cargo_sha256);
  assert.equal(record.watch(rustc), record.receipt.build.selected_rustc_sha256);
  assert.equal(vendor, record.receipt.build.selected_vendor);
  assert.equal(record.receipt.build.selected_vendor_nar_hash, m6.vendor_nar_hash);
  const manifest = 'verification/result-library/peer/Cargo.toml';
  assert.ok(plan.sources[manifest] && plan.sources['verification/result-library/peer/Cargo.lock'],
    'independent peer manifest/lock omitted from source snapshot');
  const working = path.join(record.output, 'peer-build');
  const home = path.join(working, 'home');
  const cargoHome = path.join(working, 'cargo');
  const target = path.join(working, 'target');
  const tmp = path.join(working, 'tmp');
  for (const directory of [home, cargoHome, target, tmp])
    fs.mkdirSync(directory, { recursive: true });
  const environment = {
    PATH: `${path.join(rust, 'bin')}:${m6.linker_bin}:/run/current-system/sw/bin`,
    HOME: home, CARGO_HOME: cargoHome, CARGO_TARGET_DIR: target,
    CARGO_NET_OFFLINE: 'true',
    RUSTC: rustc, RUSTDOC: rustdoc, CARGO_INCREMENTAL: '0',
    RUSTC_WRAPPER: '', RUSTC_WORKSPACE_WRAPPER: '', RUSTFLAGS: '',
    CARGO_ENCODED_RUSTFLAGS: '', LANG: 'C.UTF-8', LC_ALL: 'C.UTF-8',
    USER: process.env.USER ?? 'nobody', TMPDIR: tmp,
  };
  const args = ['build', '--manifest-path', manifest, '--locked', '--offline',
    '--config', 'source.crates-io.replace-with="m6-vendor"',
    '--config', `source.m6-vendor.directory="${vendor}"`];
  const executed = spawnSync(cargo, args, {
    cwd: root, env: environment, encoding: null, timeout: 180_000,
    killSignal: 'SIGKILL', maxBuffer: 32 * 1024 * 1024,
  });
  const stdin = record.retain('plan/build/peer-build.stdin', Buffer.alloc(0));
  const stdout = record.retain('plan/build/peer-build.stdout', executed.stdout ?? Buffer.alloc(0));
  const stderr = record.retain('plan/build/peer-build.stderr', executed.stderr ?? Buffer.alloc(0));
  record.receipt.peer_build = {
    selected_cargo: { path: cargo, sha256: fileHash(cargo) },
    selected_rustc: { path: rustc, sha256: fileHash(rustc) },
    selected_vendor: vendor, argv: [cargo, ...args], cwd: root, environment,
    stdin: record.relative(stdin), stdout: record.relative(stdout),
    stderr: record.relative(stderr), status: executed.status,
    signal: executed.signal, error: executed.error ? String(executed.error) : null,
    source_snapshot: Object.fromEntries(Object.entries(plan.sources)
      .filter(([name]) => name.startsWith('crates/') || name.startsWith('verification/result-library/peer/'))
      .map(([name, item]) => [name, item.sha256])),
  };
  assert.equal(executed.status, 0, 'independent peer did not build offline from source');
  for (const name of Object.keys(record.receipt.peer_build.source_snapshot))
    assert.equal(fileHash(path.join(root, name)), plan.sources[name].sha256,
      `${name}: source changed during independent peer build`);
  const binary = path.join(target, 'debug', 'noble-result-library-peer');
  assert.ok(fs.statSync(binary).isFile(), 'independent source build emitted no peer binary');
  record.receipt.peer_build.binary_sha256 = fileHash(binary);
  return binary;
}

function linked(reports, names, label) {
  assert.equal(reports.length, names.length + 1, `${label}: unexpected source frame count`);
  for (let index = 0; index < names.length; index++) {
    const report = reports[index];
    record.quiet(report, `${label}/module-${index}`);
    assert.equal(report.stage, 'link', `${label}: module was not registered`);
    assert.equal(report.outcome, 'linked', `${label}: module did not link`);
    assert.equal(report.resolved_module?.name, names[index], `${label}: unexpected module name`);
    assert.equal(report.resolved_module?.version, 1, `${label}: unversioned module`);
    assert.match(report.resolved_module.identity, /^[0-9]+$/,
      `${label}: module identity is not exact decimal source identity`);
  }
  return reports[0].resolved_module.identity;
}

function runtime(label, names, program, optimization, host = 'empty') {
  const sources = [...names.map(name => source(name)), source(program)];
  const observed = record.session(label, cli, binding(host), sources, optimization);
  assert.equal(observed.status, 0, `${label}: actual compiled invocation failed`);
  const identities = names.map(name => ({
    result_module: 'result', trace_module: 'result_host', helper_module: 'result_cases',
    program_module: 'program_cases', syntax_module: 'syntax_cases', resource_module: 'result_resource',
    monomorphic_module: 'mono_data',
    inert_resource_module: 'inert_resource',
  })[name]);
  assert.ok(identities.every(Boolean), `${label}: unreviewed versioned module`);
  const moduleIdentity = linked(observed.reports, identities, label);
  const report = observed.reports.at(-1);
  assert.equal(report.stage, 'wasm', `${label}: no compiled guest execution`);
  assert.equal(report.outcome, 'normal', `${label}: non-normal guest outcome`);
  assert.equal(report.status, 0, `${label}: native guest status not successful`);
  assert.equal(report.session_state, 'retained', `${label}: guest session poisoned`);
  assert.equal(report.module?.optimized, optimization === 'on',
    `${label}: selected optimization mode was not used for compiled Wasm`);
  const wasm = record.validateWasm(label, wasmTools, observed.artifacts);
  return { observed, report, wasm, moduleIdentity };
}

function nominal(report, moduleIdentity, variant, payloadType, payloadValue, label) {
  assert.equal(report.stack.length, 1, `${label}: changed ordered result stack height`);
  const result = report.stack[0];
  assert.ok(Number.isSafeInteger(result.handle) && result.handle > 0,
    `${label}: nominal result has no live cell`);
  assert.equal(result.type, 'Nominal', `${label}: result lost nominal identity`);
  assert.equal(result.module, moduleIdentity, `${label}: value came from another module version`);
  assert.equal(result.ordinal, 0, `${label}: value used another source type`);
  assert.equal(result.shape, 'variant', `${label}: result lost two-arm representation`);
  assert.equal(result.variant, variant, `${label}: wrong selected constructor`);
  assert.ok(Number.isSafeInteger(result.value?.handle) && result.value.handle > 0,
    `${label}: nominal payload has no live cell`);
  assert.equal(result.value.type, payloadType, `${label}: wrong ordered payload type`);
  if (payloadValue !== null)
    assert.equal(result.value.value, payloadValue, `${label}: wrong result payload`);
  return result;
}

function expectedNominal(text, report, identity, label) {
  const match = /^(Ok|Err)\((.*)\)$/.exec(text);
  assert.ok(match, `${label}: canonical output is not an explicit Result`);
  const [, constructor, value] = match;
  return nominal(report, identity, constructor === 'Ok' ? 'left' : 'right',
    constructor === 'Ok' ? 'I64' : 'Text', value, label);
}

function quietRuntime(report, label) {
  assert.equal(report.guest_requests, 0, `${label}: pure Result issued guest request`);
  assert.equal(report.host_requests, 0, `${label}: pure Result consulted host`);
  assert.deepEqual(report.request_trace, [], `${label}: pure Result has host trace`);
  assert.deepEqual(report.effect_requests, [], `${label}: pure Result has effect request`);
  assert.equal(report.effect_requests_unrecorded, 0, `${label}: effect trace incomplete`);
  assert.equal(report.request_trace_complete, true, `${label}: request trace incomplete`);
  assert.equal(report.protected_operations, 0, `${label}: pure Result did protected work`);
  assert.equal(report.ambient_fallback_calls, 0, `${label}: ambient fallback used`);
  assert.equal(report.acquired_authority, false, `${label}: guest acquired authority`);
  assert.equal(report.native_trap, null, `${label}: pure Result trapped`);
}

function canonicalPath(item, expected, opt) {
  assert.equal(item.operation, expected.operation);
  assert.equal(item.input, expected.input);
  assert.equal(Number(item.selected), expected.callback_count,
    'canonical selected callback count differs from independent fixture');
  const label = `dx04-${item.index}-${opt}`;
  const run = runtime(label, ['result_module', 'trace_module', 'helper_module'],
    `canonical_${item.index}`, opt, 'trace');
  expectedNominal(expected.output, run.report, run.moduleIdentity, label);
  const request = record.selectedTrace(run.report, label, {
    module: hostBinding.module, version: hostBinding.version,
    adapterIdentity: hostBinding.adapter_identity, marker: item.marker,
    selected: item.selected,
  });
  return { case_id: 'DX-04', variant: `${item.operation}/${item.input}`,
    optimization: opt, expected: { output: expected.output, callback_count: expected.callback_count },
    observed: run.observed, guest_call: request, validated_compiled_wasm: run.wasm,
    source_revision: record.receipt.source_revision };
}

function secondType(name, variant, payloadType, payloadValue, marker, opt) {
  const label = `second-${name}-${opt}`;
  const run = runtime(label, ['result_module', 'trace_module', 'helper_module'], name, opt, 'trace');
  nominal(run.report, run.moduleIdentity, variant, payloadType, payloadValue, label);
  record.selectedTrace(run.report, label, { module: hostBinding.module,
    version: hostBinding.version, adapterIdentity: hostBinding.adapter_identity,
    marker, selected: true });
  return { case_id: 'DX-04-supplemental', variant: name, optimization: opt,
    observed: run.observed, validated_compiled_wasm: run.wasm,
    source_revision: record.receipt.source_revision };
}

function programPayload(opt, runValue) {
  const name = runValue ? 'program_run' : 'program_value';
  const label = `${name}-${opt}`;
  const run = runtime(label, ['result_module', 'program_module'], name, opt);
  quietRuntime(run.report, label);
  if (runValue) nominal(run.report, run.moduleIdentity, 'left', 'I64', '3', label);
  else {
    const result = nominal(run.report, run.moduleIdentity, 'left', 'Program', null, label);
    assert.match(result.value.interface?.stack_in, /I64/,
      `${label}: captured program has wrong ordered input stack`);
    assert.match(result.value.interface?.stack_out, /I64/,
      `${label}: captured program has wrong ordered output stack`);
    assert.deepEqual(result.value.interface?.effects, [],
      `${label}: pure Program payload acquired a host effect`);
  }
  return { case_id: 'DX-04-supplemental', variant: name, optimization: opt,
    observed: run.observed, validated_compiled_wasm: run.wasm,
    source_revision: record.receipt.source_revision };
}

function syntaxPayload(opt, matched) {
  const name = matched ? 'syntax_match' : 'syntax_value';
  const label = `${name}-${opt}`;
  const run = runtime(label, ['result_module', 'syntax_module'], name, opt);
  quietRuntime(run.report, label);
  if (matched) {
    assert.equal(run.report.stack.length, 1, `${label}: changed branch output stack`);
    assert.equal(run.report.stack[0].type, 'I64');
    assert.equal(run.report.stack[0].value, '22', `${label}: wrong reflected Syntax branch`);
  } else {
    const result = nominal(run.report, run.moduleIdentity, 'right', 'Syntax', null, label);
    assert.deepEqual(result.value.recipe, [
      { literal: { type: 'I64', value: '1' } }, { invoke: 'builtin:i64.add' },
    ], `${label}: reflected Syntax was not the constructed first-class payload`);
  }
  return { case_id: 'DX-04-supplemental', variant: name, optimization: opt,
    observed: run.observed, validated_compiled_wasm: run.wasm,
    source_revision: record.receipt.source_revision };
}

function monomorphicPayload(opt, syntax) {
  const name = syntax ? 'monomorphic_syntax' : 'monomorphic_program';
  const label = `${name}-${opt}`;
  const run = runtime(label, ['monomorphic_module'], name, opt);
  quietRuntime(run.report, label);
  assert.equal(run.report.stack.length, 1, `${label}: wrong selected-arm output stack`);
  assert.equal(run.report.stack[0].type, 'I64', `${label}: wrong payload-consumer result`);
  assert.equal(run.report.stack[0].value, syntax ? '22' : '6',
    `${label}: monomorphic Program/Syntax selected arm was not executed`);
  return { case_id: 'DX-04-supplemental', variant: name, optimization: opt,
    observed: run.observed, validated_compiled_wasm: run.wasm,
    source_revision: record.receipt.source_revision };
}

function inertResourceInterface(opt, matched) {
  const name = matched ? 'inert_resource_match' : 'inert_resource_value';
  const label = `${name}-${opt}`;
  const run = runtime(label,
    ['result_module', 'trace_module', 'inert_resource_module'], name, opt, 'trace');
  record.selectedTrace(run.report, label, { module: hostBinding.module,
    version: hostBinding.version, adapterIdentity: hostBinding.adapter_identity,
    marker: 'unused', selected: false });
  if (matched) {
    assert.equal(run.report.stack.length, 1, `${label}: wrong selected-arm stack`);
    assert.equal(run.report.stack[0].type, 'I64', `${label}: wrong matcher output type`);
    assert.equal(run.report.stack[0].value, '11',
      `${label}: first-class resource-interface Program did not select its Ok arm`);
  } else {
    const result = nominal(run.report, run.moduleIdentity, 'left', 'Program', null, label);
    assert.equal(result.value.interface?.stack_in, '[Resource(0)]',
      `${label}: Program lost its resource-typed input interface`);
    assert.equal(result.value.interface?.stack_out, '[Resource(0)]',
      `${label}: Program lost its resource-typed output interface`);
    assert.deepEqual(result.value.interface?.effects, ['test.emit'],
      `${label}: Program lost its latent declared callback effect`);
  }
  return { case_id: 'DX-04-supplemental', variant: name, optimization: opt,
    observed: run.observed, validated_compiled_wasm: run.wasm,
    source_revision: record.receipt.source_revision };
}

function staticRefusal(name, modules, type, reason) {
  const label = `static-${name}`;
  const result = record.compile(label, cli, binding('empty'),
    modules.map(source), source(name), type ? [type] : []);
  assert.notEqual(result.status, 0, `${name}: ill-typed source was accepted`);
  if (name === 'resource_success' || name === 'resource_error') {
    assert.equal(result.report?.stage, 'resolve',
      `${name}: Resource generic argument was not statically refused by the type resolver`);
    assert.equal(result.report.outcome, 'unbound-word',
      `${name}: invalid generic instance was misclassified`);
  } else {
    assert.ok(result.report && ['check', 'acceptance'].includes(result.report.stage),
      `${name}: not a typed source/kernel refusal`);
    assert.ok(['reject', 'type-reject'].includes(result.report.outcome),
      `${name}: not an explicit static rejection`);
  }
  assert.match(result.report.diagnostic, reason,
    `${name}: rejected for the wrong reason`);
  record.quiet(result.report, label);
  return { case_id: 'DX-04', variant: name, observed: result,
    source_revision: record.receipt.source_revision };
}

function peerControls() {
  const modules = ['result_module', 'trace_module', 'helper_module'].map(source);
  const input = {
    schema: 'noble-result-library-kernel-workload/v1', modules, imports: [],
    positive_source: source('pure_baseline'),
    effectful_bypass_source: source('effectful_bypass'),
    host_binding_file: binding('trace'), host_binding: hostBinding,
  };
  const file = record.retain('plan/peer-workload.json', Buffer.from(JSON.stringify(input, null, 2) + '\n'));
  const observed = record.command('independent-kernel-peer', peer, [file]);
  assert.equal(observed.status, 0, 'production-linked adversarial peer failed');
  const lines = observed.bytes.toString('utf8').split('\n').filter(Boolean);
  assert.equal(lines.length, 1, 'peer did not emit exactly one actual kernel report');
  const result = JSON.parse(lines[0]);
  assert.equal(result.schema, 'noble-result-library-kernel-peer/v1');
  assert.deepEqual(result.instances, ['Result<I64,Text>', 'Result<Text,I64>']);
  assert.equal(result.baseline.production_kernel, 'accepted');
  assert.equal(result.baseline.independent_wasm_admission, 'accepted');
  assert.equal(result.conservative_effect?.bypassed_source_static, 'accepted');
  assert.equal(result.conservative_effect?.checked_effect, 'test.emit');
  assert.equal(result.conservative_effect?.erased_effect, 'EffectInclusion(test.emit)');
  assert.deepEqual(result.program_payload_control, { data: true, generic_instance: 'accepted',
    resource_interface: 'accepted' });
  assert.deepEqual(result.resource_argument_controls,
    { success_arm: 'refused', error_arm: 'refused' });
  assert.deepEqual(result.controls.map(control => control.name), [
    'reordered-generic-payloads', 'erased-Ok-constructor-contract',
    'erased-matcher-callback-effects', 'unowned-generic-matcher',
  ]);
  for (const control of result.controls) {
    assert.equal(control.mutated_environment, true);
    assert.equal(control.baseline_kernel, 'accepted');
    assert.equal(control.hostile_kernel, 'invalid');
    assert.equal(control.constraint_kind, 'InvalidContract');
    assert.equal(control.backend_prepare, 'invalid');
  }
  assert.deepEqual(result.source_inputs.map(item => [item.file, item.sha256]),
    modules.map(file => [file, fileHash(file)]));
  for (const [field, file] of [
    ['positive_source', source('pure_baseline')],
    ['effectful_bypass_source', source('effectful_bypass')],
    ['host_binding_source', binding('trace')],
  ]) {
    assert.equal(result[field]?.file, file, `${field}: peer read another source`);
    assert.equal(result[field]?.sha256, fileHash(file), `${field}: peer source bytes differ`);
  }
  return { command: observed.id, workload: record.relative(file), report: result };
}

try {
  plan = preparePlan(record, {
    moduleSource: librarySource,
    programs: Object.fromEntries(Object.entries(programs).map(([name, text]) =>
      [name, Buffer.from(text)])),
    bindings: Object.fromEntries(Object.entries(bindings).map(([name, text]) =>
      [name, Buffer.from(text)])),
  });
  buildProvenance(process.argv[3]);
  cli = record.freeze('noble', process.argv[2]);
  peer = record.freeze('kernel-peer', buildPeer());
  wasmTools = path.join(record.output, plan.tools.wasm_tools.file);
  assert.equal(fileHash(wasmTools), plan.tools.wasm_tools.sha256);
  bindRevision(record, plan);
  const canonical = plan.canonical.workload.input.result_paths;
  assert.deepEqual(canonicalCases.map(row => [row.operation, row.input]),
    canonical.map(row => [row.operation, row.input]), 'all eight canonical paths required');
  for (const item of canonicalCases) for (const opt of ['off', 'on'])
    record.receipt.cases.push(canonicalPath(item, canonical[item.index], opt));
  for (const opt of ['off', 'on']) {
    record.receipt.cases.push(secondType('second_success', 'left', 'Text', 'three',
      'dx04-second-success', opt));
    record.receipt.cases.push(secondType('second_error', 'right', 'I64', '10',
      'dx04-second-error', opt));
    record.receipt.cases.push(programPayload(opt, false), programPayload(opt, true));
    record.receipt.cases.push(syntaxPayload(opt, false), syntaxPayload(opt, true));
    record.receipt.cases.push(monomorphicPayload(opt, false), monomorphicPayload(opt, true));
    record.receipt.cases.push(inertResourceInterface(opt, false), inertResourceInterface(opt, true));
  }
  const bypass = record.compile('static-effectful-bypass', cli, binding('trace'),
    ['result_module', 'trace_module', 'helper_module'].map(source), source('effectful_bypass'));
  assert.equal(bypass.status, 0, 'bypassed effectful callback not statically accepted');
  record.receipt.cases.push({ case_id: 'DX-04', variant: 'bypassed-effectful-callback-static',
    observed: bypass, source_revision: record.receipt.source_revision });
  for (const [name, type, reason] of [
    ['resource_success', 'result@1.Result<Resource<test.counter>,Text>', /unknown or unexported versioned type/i],
    ['resource_error', 'result@1.Result<I64,Resource<test.counter>>', /unknown or unexported versioned type/i],
    ['invalid_callback_stack', null, /stack|callback|type|unif|compatible/i],
  ]) {
    const result = staticRefusal(name, ['result_module', 'helper_module'], type, reason);
    result.variant = type === null ? 'incompatible-ordered-callback-stack'
      : name === 'resource_success' ? 'resource-in-unselected-arm-implicitly-discarded'
        : 'resource-in-error-arm-implicitly-discarded';
    record.receipt.cases.push(result);
  }
  record.receipt.cases.push(staticRefusal('resource_capture',
    ['result_module', 'resource_module'], 'result_resource@1.CounterOwner',
    /non-capturable|resource|data|eligib/i));
  record.receipt.peer = peerControls();
  const required = [
    ...canonicalCases.flatMap(row => ['off', 'on'].map(opt => `DX-04/${row.operation}/${row.input}/${opt}`)),
    ...['second_success', 'second_error', 'program_value', 'program_run',
      'syntax_value', 'syntax_match', 'monomorphic_program', 'monomorphic_syntax',
      'inert_resource_value', 'inert_resource_match']
      .flatMap(name => ['off', 'on'].map(opt =>
      `DX-04-supplemental/${name}/${opt}`)),
    ...['bypassed-effectful-callback-static',
      'resource-in-unselected-arm-implicitly-discarded',
      'resource-in-error-arm-implicitly-discarded',
      'incompatible-ordered-callback-stack', 'resource_capture'].map(name => `DX-04/${name}/static`),
  ].sort();
  assert.deepEqual(record.receipt.cases.map(item =>
    `${item.case_id}/${item.variant}/${item.optimization ?? 'static'}`).sort(), required,
  'required Result outcomes or adversarial controls omitted');
} catch (error) {
  record.receipt.failures.push({ case_id: 'DX-04-gate', error: String(error.stack ?? error) });
} finally {
  const counts = Object.fromEntries(['DX-04', 'DX-04-supplemental'].map(name =>
    [name, record.receipt.cases.filter(row => row.case_id === name).length]));
  const summary = { case_rows: record.receipt.cases.length, canonical_and_static: counts['DX-04'],
    supplemental_program_syntax_and_second_instantiation: counts['DX-04-supplemental'],
    command_count: record.receipt.commands.length,
    failures: record.receipt.failures.length,
    integrity_failures: record.receipt.integrity_failures.length,
    first_class_program_runtime: record.receipt.cases.filter(row =>
      ['program_value', 'program_run'].includes(row.variant)).length,
    reflected_syntax_runtime: record.receipt.cases.filter(row =>
      ['syntax_value', 'syntax_match'].includes(row.variant)).length };
  const result = record.seal(summary);
  console.log(JSON.stringify({ schema: record.receipt.schema, result: result.result,
    report: result.report, summary: result.summary }));
  process.exitCode = result.result === 'passed' ? 0 : 1;
}
