// Source-bound finite execution evidence for issue #52's declared-module link.
// The CLI compiles the caller and explicitly supplied module/import units to a
// single WAT artifact; CoreEngine then assembles, loads and executes that exact
// artifact against the selected managed-linear-memory runtime.
import { spawnSync } from 'node:child_process';
import {
  mkdirSync, readFileSync, readdirSync, realpathSync, statSync, writeFileSync,
} from 'node:fs';
import { userInfo } from 'node:os';
import { dirname, join, relative, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { CoreEngine } from '../../crates/noble-cli/src/core/runtime/host.mjs';
import { buildCli, digest, fileHash } from '../foundation/execution.mjs';
import { selectedVendor } from '../selected-vendor.mjs';

const root = resolve(fileURLToPath(import.meta.url), '../../..');
const fixtures = 'verification/linking/fixtures';
const selection = JSON.parse(readFileSync(join(root, 'policy/tool-selection.json')));
const runtimeConfigPath = 'crates/noble-cli/src/core/runtime/config.json';
const abiPath = 'crates/noble-cli/src/core/runtime/abi.json';
const declaredAbiPath = 'crates/noble-cli/src/core/runtime/declared-abi.json';
const runtimeConfig = JSON.parse(readFileSync(join(root, runtimeConfigPath)));
const abi = JSON.parse(readFileSync(join(root, abiPath)));
const declaredAbi = readFileSync(join(root, declaredAbiPath), 'utf8');
const node = join(selection.tool_paths.node.output, 'bin/node');
const rustBin = join(selection.tool_paths.quality_rust.output, 'bin');
const cargo = join(rustBin, 'cargo');
const rustc = join(rustBin, 'rustc');
const git = realpathSync('/run/current-system/sw/bin/git');
const nix = realpathSync('/run/current-system/sw/bin/nix');
const { homedir: home, username } = userInfo();
const childBase = Object.freeze({
  HOME: home, USER: username, LOGNAME: username, LANG: 'C.UTF-8', CI: '1', GIT_TERMINAL_PROMPT: '0',
  PATH: [rustBin, dirname(git), '/run/current-system/sw/bin', '/usr/bin', '/bin'].join(':'),
});

const modes = ['off', 'on'];
const MAX_I64 = (1n << 63n) - 1n;
const MIN_I64 = -(1n << 63n);
const selectedLimits = Object.fromEntries([
  'allocation_bytes', 'recipe_leaves', 'program_depth', 'operand_bytes', 'continuation_bytes', 'steps',
].map(key => [key, abi.limits_maximum[key]]));
const theoremPrefix = 'NobleFoundation.LinkBoundary.';
const requiredTheorems = [
  'split_entry_lookup', 'split_entry_matches_use', 'split_entry_typed', 'pilot_root_typed',
  'pilot_closure', 'scalar_call_entry_return', 'caller_after_named_call', 'horizontal_compose',
  'horizontal_runtime_link', 'vertical_simulation_compose', 'pilot_bindings_share_artifact',
  'pilot_entry_selected_binding', 'pilot_caller_root_binding', 'pilot_runtime_entry_boundary',
  'pilot_runtime_callee_boundary', 'pilot_runtime_final_boundary', 'pilot_runtime_callee_run',
  'pilot_runtime_caller_post_run', 'pilot_runtime_two_results_and_caller_law',
  'operand_slot_address', 'continuation_slot_address', 'bool_cell_decodes', 'i64_cell_decodes',
  'bool_prefix_typed', 'pilot_bool_input_values_typed', 'runtime_run_preserves_control',
  'runtime_step_preserves_sp_count', 'runtime_run_preserves_sp_count',
  'runtime_run_preserves_counters', 'runtime_run_preserves_protected_suffix',
  'enqueueOne_preserves_counters', 'same_identity_required', 'same_signature_wrong_body_refused',
  'operand_order_distinguished', 'result_order_distinguished', 'wrong_return_target_refused',
  'return_target_exact', 'return_target_exact_after_run', 'dependency_version_identity_exact',
  'dependency_version_change_refused', 'dependency_snapshot_change_refused',
  'embedded_callee_substitution_refused',
].map(name => theoremPrefix + name);
const vectors = [
  { id: 'small-positive', prefix: true, x: 19n, y: 4n },
  { id: 'negative', prefix: false, x: -3n, y: 8n },
  { id: 'equal-operands', prefix: true, x: 42n, y: 42n },
  { id: 'i64-maximum-wrap', prefix: false, x: MAX_I64, y: -1n },
  { id: 'i64-minimum-wrap', prefix: true, x: MIN_I64, y: 1n },
  { id: 'wide-noncommutative', prefix: false, x: (1n << 62n) + 17n, y: -(1n << 62n) + 5n },
];
const profile = 'Declared-Modules-v1 / Core-Bootstrap managed-linear-memory';
const claimBoundary = {
  kind: 'conditional normal-return pure-scalar linking subject',
  actual_supported_path: 'CLI compile --declared-modules --module callee --module import emits one self-contained linked WAT; CoreEngine assembles, instantiates and executes that exact WAT.',
  same_artifact_internal_dispatch: true,
  separate_module_loading_abi: false,
  production_typechecker_or_lowering_changed: false,
  arbitrary_context_claim: false,
  compiler_or_loader_correctness_theorem: false,
  termination_or_quota_claim: false,
  proof_receipt_alone_is_not_semantic_authority: true,
};
const bounds = {
  cases_per_root_per_mode: vectors.length,
  modes: modes.length,
  roots: ['caller', 'boundary'],
  prefix: 'Bool preserved unchanged',
  scalar_domain: 'signed BitVec64; each subtraction wraps independently',
  call_boundary_results: '[wrap64(x-y), 7], ordered and distinct for every fixed vector',
  mutable_shared_state_premise: 'Exclusive engine ownership; no external mutation of the protected prefix, pending caller continuation, function table or private imported bookkeeping during this scalar execution.',
  operator_slots: 'tag:i32,padding:i32,payload:i64 at 16*i; sp is slot count',
  continuation_slots: 'index:i32,environment:i32 at 4096+8*i; cp is slot count',
};

const json = value => JSON.stringify(value, (_, item) => typeof item === 'bigint' ? item.toString() : item, 2) + '\n';
const stable = value => JSON.stringify(value, (_, item) => typeof item === 'bigint' ? item.toString() : item);
const requireThat = (condition, message) => { if (!condition) throw Error(message); };
const writeJson = (path, value) => writeFileSync(path, json(value));
const repoPath = path => join(root, path);
const relativeOutputPath = (out, path) => relative(out, path).split(sep).join('/');
const wrap64 = value => BigInt.asIntN(64, value);
const asVectorJson = vector => ({ id: vector.id, prefix: vector.prefix, x: String(vector.x), y: String(vector.y) });
function fixtureRecord(fixed) {
  return {
    schema: fixed.subject.schema,
    callee: fixed.subject.callee,
    caller_root: fixed.subject.caller_root,
    boundary_root: fixed.subject.boundary_root,
    input_types_bottom_first: ['Bool', 'I64', 'I64'],
    import_source_sha256: digest(fixed.importBytes),
    callee_source_sha256: digest(fixed.calleeBytes),
    caller_source_sha256: digest(fixed.callerBytes),
    boundary_source_sha256: digest(fixed.boundaryBytes),
    same_artifact_internal_dispatch: true,
  };
}

function run(command, args, options = {}) {
  const { captureDirectory, ...spawnOptions } = options;
  const raw = spawnSync(command, args, {
    encoding: null, maxBuffer: 64 * 1024 * 1024, timeout: 3_600_000,
    env: childBase, ...spawnOptions,
  });
  if (captureDirectory) {
    mkdirSync(captureDirectory);
    writeFileSync(join(captureDirectory, 'stdout'), raw.stdout ?? Buffer.alloc(0));
    writeFileSync(join(captureDirectory, 'stderr'), raw.stderr ?? Buffer.alloc(0));
    if (spawnOptions.input !== undefined) writeFileSync(join(captureDirectory, 'stdin'), spawnOptions.input);
    writeJson(join(captureDirectory, 'command.json'), {
      command, argv: args, cwd: spawnOptions.cwd ?? null, exit: raw.status,
      signal: raw.signal, error: raw.error ? String(raw.error) : null,
    });
  }
  const stdoutBytes = raw.stdout ?? Buffer.alloc(0);
  const stderrBytes = raw.stderr ?? Buffer.alloc(0);
  const result = {
    ...raw, stdoutBytes, stderrBytes,
    stdout: stdoutBytes.toString('utf8'), stderr: stderrBytes.toString('utf8'),
  };
  if (result.error || (result.status !== 0 && !options.allowFailure)) {
    throw Error(`${command} ${args.join(' ')} failed: ${result.error?.message ?? result.status}\n${result.stderr}\n${result.stdout}`);
  }
  return result;
}

let commandSerial = 0;
let commandDirectory = null;
function capturedRun(command, args, options = {}) {
  const captureDirectory = join(commandDirectory, String(++commandSerial).padStart(3, '0'));
  return run(command, args, { ...options, captureDirectory });
}

function gitState() {
  const gitRun = (args, cwd = root) => run(git, args, { cwd });
  return {
    head: gitRun(['rev-parse', 'HEAD']).stdout.trim(),
    tree: gitRun(['rev-parse', 'HEAD^{tree}']).stdout.trim(),
    status: gitRun(['status', '--porcelain=v1', '--untracked-files=all']).stdout.trim(),
  };
}

function checkEnvironment() {
  const forbidden = Object.keys(process.env).filter(key => {
    if (key === 'GIT_TERMINAL_PROMPT') return process.env[key] !== '0';
    return /^(?:LEAN(?:_|$)|LAKE(?:_|$)|NODE(?:_|$)|BASH(?:_|$)|ENV$|LD_|DYLD_|GIT_|NIX_|ELAN|CARGO_|RUST|NOBLE_)/.test(key)
      && process.env[key] !== '';
  });
  requireThat(forbidden.length === 0, `unsafe caller environment: ${forbidden.sort().join(', ')}`);
  requireThat(realpathSync(process.execPath) === realpathSync(node), `use selected Node: ${node}`);
  requireThat(stable(process.execArgv) === stable(runtimeConfig.node_flags),
    `Node flags differ from runtime config: ${process.execArgv.join(' ')}`);
  requireThat(realpathSync(runtimeConfig.tools.node.path) === realpathSync(node),
    'runtime Node differs from policy-selected Node');
}

function safeRepoRelative(path) {
  return typeof path === 'string' && path.length > 0 && !path.startsWith('/')
    && !path.split('/').some(part => part === '..' || part === '');
}

function currentSourceHashes(foundation) {
  const runtimeHelpers = [
    'crates/noble-cli/src/core/runtime/host.mjs',
    'crates/noble-cli/src/core/runtime/preamble.mjs',
    'crates/noble-cli/src/core/runtime/values.mjs',
    'crates/noble-cli/src/core/runtime/engine-host.mjs',
    'crates/noble-cli/src/core/runtime/engine.mjs',
    'crates/noble-cli/src/core/runtime/protocol.mjs',
  ];
  const implementationPaths = [
    'Cargo.toml', 'Cargo.lock', 'crates/noble-cli/Cargo.toml',
    'crates/noble-cli/src/core.rs', 'crates/noble-cli/src/core/arguments.rs',
    'crates/noble-cli/src/core/arguments/bindings.rs',
    'crates/noble-cli/src/core/entry/compilation.rs',
    'crates/noble-wasm/Cargo.toml',
    'crates/noble-wasm/src/source/plan/operations.rs',
    'crates/noble-wasm/src/source/plan/operations/nominal.rs',
    'crates/noble-wasm/src/source/plan.rs',
    'crates/noble-wasm/src/source/emit.rs',
    'crates/noble-wasm/src/source/emit/functions.rs',
    'crates/noble-wasm/src/source/emit/functions/slots.rs',
    'crates/noble-wasm/src/source/emit/initialize.rs',
    'crates/noble-wasm/src/emit/functions.rs',
    'crates/noble-wasm/runtime/operations.wat',
    'crates/noble-wasm/runtime/programs.wat',
    'crates/noble-wasm/runtime/stack.wat',
    'crates/noble-cli/src/core/runtime/config.json',
    'crates/noble-cli/src/core/runtime/abi.json',
    declaredAbiPath,
    'verification/foundation/execution.mjs', 'verification/selected-vendor.mjs',
    'verification/linking/run.mjs',
    ...runtimeHelpers,
    ...runtimeConfig.runtime_assets,
    ...readdirSync(repoPath(fixtures)).sort().map(name => `${fixtures}/${name}`),
  ];
  const proofPaths = Object.keys(foundation.sources_sha256 ?? {});
  const paths = [...new Set([...implementationPaths, ...proofPaths])].sort();
  const hashes = {};
  for (const path of paths) {
    requireThat(safeRepoRelative(path), `unsafe repository source path: ${path}`);
    hashes[path] = fileHash(repoPath(path));
  }
  return hashes;
}

function proofBinding(foundation, foundationBytes, state) {
  requireThat(foundation.schema === 'noble-foundation-check/v1', 'foundation receipt schema mismatch');
  requireThat(foundation.source_commit === state.head && foundation.source_tree === state.tree,
    'foundation receipt source revision/tree mismatch');
  requireThat(foundation.clean_source === true && foundation.allow_dirty === false,
    'foundation receipt was not produced from a clean committed source');
  requireThat(foundation.implementation_theorem === false,
    'foundation receipt overstates proof-to-implementation correspondence');
  requireThat(foundation.sources_sha256 && typeof foundation.sources_sha256 === 'object',
    'foundation receipt has no source hash inventory');
  const checkedProofSources = [
    'proofs/foundation/NobleFoundation.lean',
    'proofs/foundation/NobleFoundation/LinkBoundary.lean',
    'proofs/foundation/inventory.json',
    'verification/foundation/check.mjs',
  ];
  for (const path of checkedProofSources) {
    requireThat(Object.hasOwn(foundation.sources_sha256, path), `foundation checker did not bind ${path}`);
    requireThat(fileHash(repoPath(path)) === foundation.sources_sha256[path], `stale foundation proof source: ${path}`);
  }
  const inventory = JSON.parse(readFileSync(repoPath('proofs/foundation/inventory.json')));
  const inventoryNames = inventory.theorems.map(entry => entry.name).sort();
  const receiptNames = (foundation.theorems ?? []).map(entry => entry.name).sort();
  requireThat(stable(inventoryNames) === stable(receiptNames), 'theorem inventory differs from checked proof receipt');
  requireThat(foundation.theorem_count === foundation.theorems.length,
    'foundation theorem count differs from its checked inventory');
  requireThat(stable(inventory.allowed_axioms) === stable(foundation.allowed_axioms),
    'allowed-axiom inventory differs from checked proof receipt');
  const allowedAxioms = new Set(inventory.allowed_axioms);
  for (const theorem of foundation.theorems) {
    requireThat(theorem.status === 'proved' && typeof theorem.statement === 'string' && theorem.statement.length > 0,
      `unproved or unreported theorem: ${theorem.name}`);
    requireThat(Array.isArray(theorem.axioms) && theorem.axioms.every(axiom => allowedAxioms.has(axiom)),
      `unapproved transitive axiom in ${theorem.name}`);
  }
  const names = new Set(foundation.theorems.map(theorem => theorem.name));
  for (const theorem of requiredTheorems) requireThat(names.has(theorem), `missing linking theorem: ${theorem}`);
  const linkTheorems = foundation.theorems.filter(theorem => theorem.name.startsWith(theoremPrefix));
  requireThat(linkTheorems.length >= requiredTheorems.length, 'foundation receipt has no complete LinkBoundary inventory');
  return {
    receipt_sha256: digest(foundationBytes),
    receipt_path: 'foundation-check.json',
    inventory_sha256: fileHash(repoPath('proofs/foundation/inventory.json')),
    source_hashes: foundation.sources_sha256,
    theorem_count: foundation.theorem_count,
    theorem_inventory: foundation.theorems,
    linking_theorems: linkTheorems,
    allowed_axioms: inventory.allowed_axioms,
    implementation_theorem: false,
    proof_acceptance_from_receipt_alone: false,
    boundary: 'The checker receipt is re-bound to the current committed Lean sources, complete theorem inventory and transitive axiom list. It is a separate formal model; it is not a real compiler/lowering/loader theorem.',
  };
}

function selectedTools(vendor = null) {
  const nodeVersion = run(node, ['--version']).stdout.trim();
  const cargoVersion = run(cargo, ['--version']).stdout.trim();
  const rustcVersion = run(rustc, ['--version']).stdout.trim();
  requireThat(nodeVersion.includes(runtimeConfig.tools.node.version), 'wrong selected Node version');
  const rustToolchain = selection.configuration.quality_rust;
  requireThat(cargoVersion.length > 0 && rustcVersion.length > 0, 'selected Rust tools unavailable');
  return {
    node: { path: realpathSync(node), sha256: fileHash(realpathSync(node)), version: nodeVersion,
      v8: process.versions.v8, flags: process.execArgv },
    cargo: { path: realpathSync(cargo), sha256: fileHash(realpathSync(cargo)), version: cargoVersion },
    rustc: { path: realpathSync(rustc), sha256: fileHash(realpathSync(rustc)), version: rustcVersion,
      policy: rustToolchain },
    wasm_tools: { path: realpathSync(runtimeConfig.tools.wasm_tools.path),
      sha256: fileHash(runtimeConfig.tools.wasm_tools.path), version: runtimeConfig.tools.wasm_tools.version },
    wasm_opt: { path: realpathSync(runtimeConfig.tools.wasm_opt.path),
      sha256: fileHash(runtimeConfig.tools.wasm_opt.path), version: runtimeConfig.tools.wasm_opt.version },
    nix: { path: nix, sha256: fileHash(nix) },
    ...(vendor ? { vendor } : {}),
  };
}

function bodyBytes(moduleSource, name) {
  const marker = Buffer.from(`def ${name} [`);
  const start = moduleSource.indexOf(marker);
  requireThat(start >= 0 && moduleSource.indexOf(marker, start + 1) < 0,
    `could not uniquely bind selected definition ${name}`);
  let depth = 1;
  const begin = start + marker.length;
  for (let index = begin; index < moduleSource.length; index += 1) {
    if (moduleSource[index] === 0x5b) depth += 1;
    if (moduleSource[index] === 0x5d) {
      depth -= 1;
      if (depth === 0) return moduleSource.subarray(begin, index);
    }
  }
  throw Error(`unterminated selected definition ${name}`);
}

function subjectIdentity({ rootBytes, calleeBytes, importBytes, version, selected, rootKind }) {
  const rootSha = digest(rootBytes), calleeSha = digest(calleeBytes), importSha = digest(importBytes);
  const bodySha = digest(bodyBytes(calleeBytes, selected.definition));
  const closure = [
    { role: 'callee-module', sha256: calleeSha, byte_length: calleeBytes.length },
    { role: 'import-unit', sha256: importSha, byte_length: importBytes.length },
  ];
  const closureSha = digest(Buffer.from(stable(closure)));
  const identity = {
    root_kind: rootKind, caller_root_sha256: rootSha,
    callee_module: selected.module, callee_version: version,
    callee_module_source_sha256: calleeSha,
    import_source_sha256: importSha,
    selected_definition: {
      name: selected.definition, definition_slot: selected.definition_slot,
      body_source_sha256: bodySha, input: selected.input, output: selected.output,
    },
    source_dependency_closure_sha256: closureSha,
  };
  return {
    ...identity,
    identity_sha256: digest(Buffer.from(stable(identity))),
    source_unit_sha256: { caller_root: rootSha, callee_module: calleeSha, import: importSha },
    selected_definition: {
      ...identity.selected_definition,
      module: selected.module, version, module_source_sha256: calleeSha,
      source_dependency_closure_sha256: closureSha,
    },
    source_dependency_closure: closure,
  };
}

function mkdirParent(path) { mkdirSync(dirname(path), { recursive: true }); }
function replaceExactly(source, before, after, description) {
  const at = source.indexOf(before);
  requireThat(at >= 0 && source.indexOf(before, at + 1) < 0, `expected one ${description} site`);
  return Buffer.concat([source.subarray(0, at), after, source.subarray(at + before.length)]);
}

function artifactIndex(directory, prefix = '') {
  const rows = {};
  const entries = readdirSync(join(directory, prefix), { withFileTypes: true })
    .sort((left, right) => left.name.localeCompare(right.name));
  for (const entry of entries) {
    const path = prefix ? `${prefix}/${entry.name}` : entry.name;
    if (path === 'linking.json') continue;
    if (entry.isDirectory()) Object.assign(rows, artifactIndex(directory, path));
    else if (entry.isFile()) rows[path] = fileHash(join(directory, path));
    else throw Error(`nonregular linking evidence artifact: ${path}`);
  }
  return rows;
}

function expectedStacks(vector, rootKind, { constant = 7n, reverseResults = false, reverseInputs = false } = {}) {
  const x = reverseInputs ? vector.y : vector.x;
  const y = reverseInputs ? vector.x : vector.y;
  const difference = wrap64(x - y);
  const outputs = reverseResults ? [constant, difference] : [difference, constant];
  if (rootKind === 'boundary') {
    return [['Bool', vector.prefix], ...outputs.map(value => ['I64', String(value)])];
  }
  requireThat(rootKind === 'caller', `unknown root kind: ${rootKind}`);
  const result = wrap64(outputs[0] - outputs[1]);
  return [['Bool', vector.prefix], ['I64', String(result)]];
}

function inputsFor(vector, reverse = false) {
  const x = reverse ? vector.y : vector.x;
  const y = reverse ? vector.x : vector.y;
  return [
    { type: 'Bool', value: vector.prefix },
    { type: 'I64', value: String(x) },
    { type: 'I64', value: String(y) },
  ];
}

function stackValues(report) {
  return (report.stack ?? []).map(value => [value.type, value.value]);
}

function checkStack(report, expected, label) {
  requireThat(report.outcome === 'normal', `${label}: expected normal execution, got ${report.outcome}`);
  requireThat(report.guest_requests === 0 && report.protected_operations === 0,
    `${label}: pure scalar path made host/protected requests`);
  requireThat(stable(report.request_trace ?? []) === '[]', `${label}: unexpected runtime request trace`);
  requireThat(stable(stackValues(report)) === stable(expected),
    `${label}: ordered complete stack mismatch; expected ${stable(expected)}, got ${stable(stackValues(report))}`);
}

function executeWat({ out, label, subject, mode, watBytes, sourcePath, inputs, expected = null }) {
  const directory = join(out, 'executions', label, mode);
  mkdirSync(directory, { recursive: true });
  const sourceBytes = readFileSync(sourcePath);
  const inputWatPath = join(directory, 'input.wat');
  writeFileSync(inputWatPath, watBytes);
  const inputsPath = join(directory, 'inputs.json');
  writeJson(inputsPath, inputs);
  const engine = new CoreEngine(runtimeConfig, abi, {
    optimized: mode === 'on', artifacts: directory,
    declared_modules: true, declared_extension: declaredAbi,
  });
  let report;
  let preparation;
  let wasmPath;
  try {
    preparation = engine.prepare(watBytes, sourceBytes);
    writeJson(join(directory, 'preparation.json'), preparation);
    requireThat(preparation.profile === 'Declared-Modules-v1', `${label}/${mode}: wrong runtime profile`);
    requireThat(preparation.outcome === 'ready', `${label}/${mode}: CoreEngine did not admit WAT`);
    requireThat(preparation.module.source_sha256 === digest(sourceBytes),
      `${label}/${mode}: loaded module does not bind exact caller root bytes`);
    requireThat(preparation.module.wat_sha256 === digest(watBytes),
      `${label}/${mode}: loaded module does not bind exact admitted WAT bytes`);
    report = engine.execute({ inputs, limits: selectedLimits });
    writeJson(join(directory, 'report.json'), report);
    requireThat(report.guest_requests === 0 && report.protected_operations === 0
      && report.host_requests === 0 && report.ambient_fallback_calls === 0
      && stable(report.request_trace ?? []) === '[]',
    `${label}/${mode}: pure scalar execution issued a host/protected request`);
    const name = mode === 'on' ? 'module-1-optimized.wasm' : 'module-1.wasm';
    wasmPath = join(engine.directory, name);
    requireThat(statSync(wasmPath).isFile(), `${label}/${mode}: final loaded Wasm is absent`);
    requireThat(fileHash(wasmPath) === report.module.wasm_sha256,
      `${label}/${mode}: report does not bind actual loaded Wasm bytes`);
    requireThat(report.module.optimized === (mode === 'on'), `${label}/${mode}: optimizer mode mismatch`);
    if (expected !== null) checkStack(report, expected, `${label}/${mode}`);
  } finally {
    engine.close();
  }
  const reportPath = join(directory, 'report.json');
  return {
    label, mode, subject: subject.label,
    inputs,
    limits: { ...selectedLimits },
    root_source_sha256: digest(sourceBytes),
    wat_sha256: digest(watBytes),
    wasm_sha256: fileHash(wasmPath),
    expected_stack: expected,
    observed_stack: stackValues(report),
    outcome: report.outcome,
    guest_requests: report.guest_requests,
    protected_operations: report.protected_operations,
    report_sha256: fileHash(reportPath),
    report_path: relativeOutputPath(out, reportPath),
    preparation_path: relativeOutputPath(out, join(directory, 'preparation.json')),
    inputs_path: relativeOutputPath(out, inputsPath),
    input_wat_path: relativeOutputPath(out, inputWatPath),
    source_path: relativeOutputPath(out, sourcePath),
    engine_wat_path: relativeOutputPath(out, join(engine.directory, 'module-1.wat')),
    wasm_path: relativeOutputPath(out, wasmPath),
    artifacts_directory: relativeOutputPath(out, directory),
  };
}

function compileSubject({ out, cli, env, label, rootKind, rootBytes,
  calleeBytes, importBytes, version, baseFixture }) {
  const directory = join(out, 'subjects', label);
  mkdirSync(directory, { recursive: true });
  const paths = {
    callee: join(directory, 'callee.noble'),
    import: join(directory, 'import.noble'),
    root: join(directory, rootKind === 'boundary' ? 'boundary.noble' : 'caller.noble'),
    bindings: join(directory, 'bindings.txt'),
    wat: join(directory, 'compile.wat'),
  };
  writeFileSync(paths.callee, calleeBytes);
  writeFileSync(paths.import, importBytes);
  writeFileSync(paths.root, rootBytes);
  writeFileSync(paths.bindings, baseFixture.bindingsBytes);
  const identity = subjectIdentity({ rootBytes, calleeBytes, importBytes, version,
    selected: baseFixture.subject.callee, rootKind });
  const argv = ['compile', paths.root, '--declared-modules', '--bindings', paths.bindings,
    '--module', paths.callee, '--module', paths.import];
  for (const type of ['Bool', 'I64', 'I64']) argv.push('--input-type', type);
  const commandPath = join(commandDirectory, String(++commandSerial).padStart(3, '0'));
  const compiled = run(cli, argv, { cwd: root, env, allowFailure: true, captureDirectory: commandPath });
  requireThat(compiled.status === 0 && compiled.stdoutBytes.length > 0,
    `${label}: real ` + `noble compile --declared-modules failed: ${compiled.stderr}`);
  writeFileSync(paths.wat, compiled.stdoutBytes);
  const rawCompile = {
    argv: ['compile', '<root>', '--declared-modules', '--bindings', '<bindings>',
      '--module', '<callee>', '--module', '<import>', '--input-type', 'Bool',
      '--input-type', 'I64', '--input-type', 'I64'],
    status: compiled.status,
    command_path: relativeOutputPath(out, commandPath),
    stdout_sha256: digest(compiled.stdoutBytes),
    stderr_sha256: digest(compiled.stderrBytes),
    wat_sha256: fileHash(paths.wat),
    wat_path: relativeOutputPath(out, paths.wat),
  };
  const record = {
    label, root_kind: rootKind, module: 'LinkCallee', version,
    same_artifact_internal_dispatch: true,
    compile: rawCompile,
    paths: Object.fromEntries(Object.entries(paths).map(([key, path]) => [key, relativeOutputPath(out, path)])),
    source_sha256: {
      caller_root: digest(rootBytes), callee_module: digest(calleeBytes), import: digest(importBytes),
    },
    identity,
    admitted_wat_sha256: fileHash(paths.wat),
    admitted_wat_path: relativeOutputPath(out, paths.wat),
  };
  writeJson(join(directory, 'subject.json'), record);
  return { ...record, label, rootBytes, calleeBytes, importBytes,
    paths: Object.fromEntries(Object.entries(paths).map(([key, path]) => [key, relativeOutputPath(out, path)])),
    watBytes: readFileSync(paths.wat), rootPath: paths.root };
}

function fixtureData() {
  const readFixture = name => readFileSync(repoPath(`${fixtures}/${name}`));
  const subject = JSON.parse(readFixture('subject.json'));
  requireThat(subject.schema === 'noble-linking-fixture/v1', 'unknown linking fixture schema');
  requireThat(subject.same_artifact_internal_dispatch === true, 'fixture is not a same-artifact link');
  const calleeBytes = readFixture('callee.noble');
  const importBytes = readFixture('import.noble');
  const callerBytes = readFixture('caller.noble');
  const boundaryBytes = readFixture('boundary.noble');
  requireThat(stable(subject.callee.input) === stable(['I64', 'I64'])
    && stable(subject.callee.output) === stable(['I64', 'I64']), 'unexpected split interface');
  requireThat(subject.callee.module === 'LinkCallee' && subject.callee.version === 1
    && subject.callee.export === 'split' && subject.callee.definition === 'split'
    && subject.callee.definition_slot === 0
    && subject.caller_root === 'c.split -' && subject.boundary_root === 'c.split'
    && subject.callee.semantics === '[x,y] -> [wrap64(x-y),7]'
    && subject.preserved_prefix === 'Bool'
    && callerBytes.equals(Buffer.from(`${subject.caller_root}\n`))
    && boundaryBytes.equals(Buffer.from(`${subject.boundary_root}\n`))
    && importBytes.equals(Buffer.from(`${subject.import}\n`))
    && calleeBytes.equals(Buffer.from('module LinkCallee@1 [ export split def split [ - 7 ] ]\n')),
  'fixture bytes do not implement the named same-artifact pilot');
  return {
    calleeBytes, importBytes, callerBytes, boundaryBytes,
    bindingsBytes: readFixture('bindings.txt'), subject,
  };
}

function matchingIdentity(left, right) {
  return left.identity.identity_sha256 === right.identity.identity_sha256
    && left.admitted_wat_sha256 === right.admitted_wat_sha256;
}

function runEvidence({ foundationPath, output }) {
  checkEnvironment();
  const out = resolve(output);
  requireThat(realpathSync(dirname(out)) === dirname(out),
    'linking output parent must not resolve through a symlink');
  requireThat(!out.startsWith(`${root}${sep}`) && out !== root,
    'linking output must be outside the committed source worktree');
  mkdirSync(out);
  requireThat(realpathSync(out) === out, 'output directory resolves through a symlink');
  commandDirectory = join(out, 'commands');
  mkdirSync(commandDirectory);
  const state = gitState();
  requireThat(state.status === '', `source worktree is dirty: ${state.status}`);
  const foundationBytes = readFileSync(resolve(foundationPath));
  const foundation = JSON.parse(foundationBytes);
  const proof = proofBinding(foundation, foundationBytes, state);
  writeFileSync(join(out, 'foundation-check.json'), foundationBytes);

  const initialSources = currentSourceHashes(foundation);
  const fixed = fixtureData();
  const rawVectors = vectors.map(asVectorJson);
  const toolchainSelection = selectedVendor(selection.component_sync);
  const built = buildCli({ root, out, selection, childBase, run: capturedRun });
  const toolchain = selectedTools(toolchainSelection);
  const cliSha = fileHash(built.cli);
  toolchain.cli = { path: relative(root, built.cli).split(sep).join('/'), sha256: cliSha };
  const commandArgs = [
    'verification/linking/run.mjs', '--foundation', resolve(foundationPath),
    '--output', out,
  ];
  const limits = { ...selectedLimits };
  const receipt = {
    schema: 'noble-linking-execution/v1',
    issue: 52,
    profile,
    claim_boundary: claimBoundary,
    source_commit: state.head,
    source_tree: state.tree,
    clean_source: true,
    command: commandArgs,
    artifact_directory: '.',
    proof_binding: proof,
    source_hashes: initialSources,
    toolchain,
    runtime: {
      profile: runtimeConfig.target,
      backend: 'managed-linear-memory',
      runtime_config_sha256: fileHash(repoPath(runtimeConfigPath)),
      abi_version: abi.version,
      abi_sha256: fileHash(repoPath(abiPath)),
      helper_source_hashes: Object.fromEntries(Object.entries(initialSources)
        .filter(([path]) => path.startsWith('crates/noble-cli/src/core/runtime/'))),
      runtime_assets: runtimeConfig.runtime_assets,
      limits,
      opt_modes: modes,
      node_flags: runtimeConfig.node_flags,
    },
    fixture: fixtureRecord(fixed),
    vectors: rawVectors,
    bounds,
    subjects: {},
    executions: [],
    controls: {},
    artifacts_sha256: {},
  };

  const subjects = {};
  const addSubject = options => {
    const subject = compileSubject({ out, cli: built.cli, env: built.env,
      baseFixture: fixed, ...options });
    subjects[subject.label] = subject;
    const { rootBytes, calleeBytes, importBytes, watBytes, rootPath, ...sourceBoundRecord } = subject;
    receipt.subjects[subject.label] = sourceBoundRecord;
    return subject;
  };

  const callerSubject = addSubject({ label: 'pilot-caller', rootKind: 'caller',
    rootBytes: fixed.callerBytes, calleeBytes: fixed.calleeBytes,
    importBytes: fixed.importBytes, version: fixed.subject.callee.version });
  const boundarySubject = addSubject({ label: 'callee-result-boundary', rootKind: 'boundary',
    rootBytes: fixed.boundaryBytes, calleeBytes: fixed.calleeBytes,
    importBytes: fixed.importBytes, version: fixed.subject.callee.version });

  for (const mode of modes) {
    for (const vector of vectors) {
      const callerExpected = expectedStacks(vector, 'caller');
      const callerRun = executeWat({ out, label: `pilot-caller/${vector.id}`, subject: callerSubject,
        mode, watBytes: callerSubject.watBytes, sourcePath: callerSubject.rootPath,
        inputs: inputsFor(vector), expected: callerExpected });
      const boundaryExpected = expectedStacks(vector, 'boundary');
      const boundaryRun = executeWat({ out, label: `callee-result-boundary/${vector.id}`,
        subject: boundarySubject, mode, watBytes: boundarySubject.watBytes,
        sourcePath: boundarySubject.rootPath, inputs: inputsFor(vector), expected: boundaryExpected });
      receipt.executions.push({ root: 'caller', vector: vector.id, mode, ...callerRun });
      receipt.executions.push({ root: 'boundary', vector: vector.id, mode, ...boundaryRun });
    }
  }

  const first = vectors[0];
  const wrongBodyBytes = replaceExactly(fixed.calleeBytes, Buffer.from(' - 7 '), Buffer.from(' - 8 '),
    'same-signature callee body');
  const wrongBody = addSubject({ label: 'control-wrong-body', rootKind: 'caller',
    rootBytes: fixed.callerBytes, calleeBytes: wrongBodyBytes, importBytes: fixed.importBytes,
    version: fixed.subject.callee.version });
  const wrongBodyRuns = modes.map(mode => {
    const expected = expectedStacks(first, 'caller', { constant: 8n });
    return executeWat({ out, label: `control-wrong-body/${mode}`, subject: wrongBody,
      mode, watBytes: wrongBody.watBytes, sourcePath: wrongBody.rootPath,
      inputs: inputsFor(first), expected });
  });
  const sameSignatureReuseAccepted = matchingIdentity(callerSubject, wrongBody);
  requireThat(!sameSignatureReuseAccepted, 'same-signature altered callee identity was incorrectly reusable');
  receipt.controls.same_signature_wrong_body = {
    same_input_output_interface: true,
    exact_subject_identity_reuse_accepted: sameSignatureReuseAccepted,
    identity_reuse_refused: true,
    baseline_subject: callerSubject.label,
    candidate_subject: wrongBody.label,
    expected_baseline: expectedStacks(first, 'caller'),
    runs: wrongBodyRuns,
    detected: wrongBodyRuns.every(row => stable(row.observed_stack) !== stable(expectedStacks(first, 'caller'))
      && row.outcome === 'normal'),
  };
  requireThat(receipt.controls.same_signature_wrong_body.detected,
    'same-signature wrong-body control was not observed');
  receipt.controls.same_signature_identity_reuse = {
    same_signature: true,
    reuse_accepted: sameSignatureReuseAccepted,
    refusal_reason: 'callee module bytes, selected body bytes and linked WAT differ despite the same [I64,I64]->[I64,I64] interface',
    baseline_identity_sha256: callerSubject.identity.identity_sha256,
    candidate_identity_sha256: wrongBody.identity.identity_sha256,
    detected: !sameSignatureReuseAccepted,
  };

  const reversedInputRuns = modes.map(mode => {
    const expected = expectedStacks(first, 'caller', { reverseInputs: true });
    return executeWat({ out, label: `control-reversed-inputs/${mode}`, subject: callerSubject,
      mode, watBytes: callerSubject.watBytes, sourcePath: callerSubject.rootPath,
      inputs: inputsFor(first, true), expected });
  });
  receipt.controls.reversed_inputs = {
    actual_input_order: ['Bool-prefix', 'y', 'x'],
    expected_order: ['Bool-prefix', 'x', 'y'],
    runs: reversedInputRuns,
    detected: reversedInputRuns.every(row => stable(row.observed_stack)
      !== stable(expectedStacks(first, 'caller'))),
  };
  requireThat(receipt.controls.reversed_inputs.detected, 'reversed-input control was not distinguished');

  const reversedResultBytes = replaceExactly(fixed.calleeBytes, Buffer.from(' - 7 '),
    Buffer.from(' - 7 swap '), 'reversed callee results');
  const reversedResultSubject = addSubject({ label: 'control-reversed-results', rootKind: 'boundary',
    rootBytes: fixed.boundaryBytes, calleeBytes: reversedResultBytes,
    importBytes: fixed.importBytes, version: fixed.subject.callee.version });
  const reversedResultRuns = modes.map(mode => executeWat({ out,
    label: `control-reversed-results/${mode}`, subject: reversedResultSubject, mode,
    watBytes: reversedResultSubject.watBytes, sourcePath: reversedResultSubject.rootPath,
    inputs: inputsFor(first), expected: expectedStacks(first, 'boundary', { reverseResults: true }) }));
  receipt.controls.reversed_results = {
    actual_order: ['7', 'wrap64(x-y)'],
    expected_order: ['wrap64(x-y)', '7'],
    same_input_output_interface: true,
    runs: reversedResultRuns,
    detected: reversedResultRuns.every(row => stable(row.observed_stack)
      !== stable(expectedStacks(first, 'boundary'))),
  };
  requireThat(receipt.controls.reversed_results.detected, 'reversed-result control was not distinguished');

  const enqueueIndex = Buffer.from('(i32.store (local.get $address) (local.get $index))');
  const wrongTargetWat = replaceExactly(callerSubject.watBytes, enqueueIndex,
    Buffer.from('(i32.store (local.get $address) (i32.add (local.get $index) (i32.const 1)))'),
    'continuation return-target slot');
  const wrongTargetWatPath = join(out, 'controls', 'return-target', 'wrong-target.wat');
  mkdirParent(wrongTargetWatPath);
  writeFileSync(wrongTargetWatPath, wrongTargetWat);
  const wrongTargetRuns = modes.map(mode => executeWat({ out,
    label: `control-wrong-return-target/${mode}`, subject: callerSubject, mode,
    watBytes: wrongTargetWat, sourcePath: callerSubject.rootPath,
    inputs: inputsFor(first) }));
  receipt.controls.wrong_return_target = {
    mutation: 'enqueue stores the next function-table index, not the selected continuation index',
    mutation_wat_sha256: digest(wrongTargetWat),
    mutation_wat_path: relativeOutputPath(out, wrongTargetWatPath),
    valid_wat_preparation_required: true,
    runs: wrongTargetRuns,
    detected: wrongTargetRuns.every(row => row.outcome !== 'normal'
      || stable(row.observed_stack) !== stable(expectedStacks(first, 'caller'))),
  };
  requireThat(receipt.controls.wrong_return_target.detected,
    'wrong-return-target mutation did not change or refuse execution');

  const versionCallee = Buffer.from(fixed.calleeBytes.toString('utf8').replace('LinkCallee@1', 'LinkCallee@2'));
  const versionImport = Buffer.from(fixed.importBytes.toString('utf8').replace('LinkCallee@1', 'LinkCallee@2'));
  const versionSubject = addSubject({ label: 'control-version-change', rootKind: 'caller',
    rootBytes: fixed.callerBytes, calleeBytes: versionCallee, importBytes: versionImport, version: 2 });
  const versionRuns = modes.map(mode => executeWat({ out, label: `control-version-change/${mode}`,
    subject: versionSubject, mode, watBytes: versionSubject.watBytes,
    sourcePath: versionSubject.rootPath, inputs: inputsFor(first), expected: expectedStacks(first, 'caller') }));
  receipt.controls.changed_version = {
    baseline_version: 1, candidate_version: 2,
    same_signature_and_finite_semantics: true,
    exact_identity_reuse_accepted: matchingIdentity(callerSubject, versionSubject),
    runs: versionRuns,
    detected: !matchingIdentity(callerSubject, versionSubject)
      && versionRuns.every(row => stable(row.observed_stack) === stable(expectedStacks(first, 'caller'))),
  };
  requireThat(receipt.controls.changed_version.detected, 'changed-version dependency control failed');

  const changedDependency = Buffer.concat([fixed.calleeBytes, Buffer.from('\n')]);
  const dependencySubject = addSubject({ label: 'control-dependency-bytes', rootKind: 'caller',
    rootBytes: fixed.callerBytes, calleeBytes: changedDependency, importBytes: fixed.importBytes,
    version: fixed.subject.callee.version });
  const dependencyRuns = modes.map(mode => executeWat({ out, label: `control-dependency-bytes/${mode}`,
    subject: dependencySubject, mode, watBytes: dependencySubject.watBytes,
    sourcePath: dependencySubject.rootPath, inputs: inputsFor(first), expected: expectedStacks(first, 'caller') }));
  receipt.controls.changed_dependency_bytes = {
    selected_definition_body_unchanged: dependencySubject.identity.selected_definition.body_source_sha256
      === callerSubject.identity.selected_definition.body_source_sha256,
    source_dependency_closure_changed: dependencySubject.identity.source_dependency_closure_sha256
      !== callerSubject.identity.source_dependency_closure_sha256,
    exact_identity_reuse_accepted: matchingIdentity(callerSubject, dependencySubject),
    runs: dependencyRuns,
    detected: !matchingIdentity(callerSubject, dependencySubject)
      && dependencyRuns.every(row => stable(row.observed_stack) === stable(expectedStacks(first, 'caller'))),
  };
  requireThat(receipt.controls.changed_dependency_bytes.detected,
    'changed-dependency-source control failed');

  const substitutedRuns = modes.map(mode => executeWat({ out,
    label: `control-post-check-substitution/${mode}`, subject: callerSubject, mode,
    watBytes: wrongBody.watBytes, sourcePath: callerSubject.rootPath,
    inputs: inputsFor(first), expected: expectedStacks(first, 'caller', { constant: 8n }) }));
  receipt.controls.post_check_callee_substitution = {
    checked_subject: callerSubject.label,
    substituted_artifact_subject: wrongBody.label,
    caller_source_hash_unchanged: callerSubject.source_sha256.caller_root
      === wrongBody.source_sha256.caller_root,
    same_signature: true,
    checked_wat_sha256: callerSubject.admitted_wat_sha256,
    substituted_wat_sha256: wrongBody.admitted_wat_sha256,
    checked_wasm_hashes: Object.fromEntries(receipt.executions.filter(row => row.root === 'caller'
      && row.vector === first.id).map(row => [row.mode, row.wasm_sha256])),
    substituted_runs: substitutedRuns,
    receipt_identity_rejects_substitution: callerSubject.admitted_wat_sha256 !== wrongBody.admitted_wat_sha256
      && callerSubject.identity.identity_sha256 !== wrongBody.identity.identity_sha256,
    engine_source_hash_alone_is_not_semantic_authority: true,
    detected: substitutedRuns.every(row => stable(row.observed_stack)
      === stable(expectedStacks(first, 'caller', { constant: 8n })))
      && callerSubject.admitted_wat_sha256 !== wrongBody.admitted_wat_sha256,
  };
  requireThat(receipt.controls.post_check_callee_substitution.detected,
    'post-check embedded-callee substitution control failed');

  for (const control of Object.values(receipt.controls)) {
    if (control.detected === undefined) control.detected = control.reuse_accepted === false;
    requireThat(control.detected === true, 'linking negative control was not detected');
  }
  receipt.execution_count = receipt.executions.length;
  const finalState = gitState();
  requireThat(finalState.head === state.head && finalState.tree === state.tree && finalState.status === state.status,
    'committed source changed during linking run');
  receipt.artifacts_sha256 = artifactIndex(out);
  const receiptPath = join(out, 'linking.json');
  writeJson(receiptPath, receipt);
  console.log(json({ schema: receipt.schema, receipt: receiptPath,
    source_commit: receipt.source_commit, source_tree: receipt.source_tree,
    normal_executions: receipt.executions.length,
    controls: Object.fromEntries(Object.entries(receipt.controls).map(([key, value]) => [key, value.detected])),
    artifact_count: Object.keys(receipt.artifacts_sha256).length,
    receipt_sha256: fileHash(receiptPath) }));
}

function verifySourceHashes(receipt) {
  requireThat(receipt.source_hashes && Object.keys(receipt.source_hashes).length > 0,
    'linking receipt has no source hashes');
  for (const [path, expected] of Object.entries(receipt.source_hashes)) {
    requireThat(safeRepoRelative(path), `unsafe source path in receipt: ${path}`);
    requireThat(fileHash(repoPath(path)) === expected, `source drift: ${path}`);
  }
}

function readOutputFile(out, path) {
  requireThat(typeof path === 'string' && !path.startsWith('/')
    && !path.split('/').some(part => part === '..' || part === ''), `unsafe artifact path: ${path}`);
  return join(out, ...path.split('/'));
}

function validateSubject(out, subject, fixture) {
  const subjectDirectory = `subjects/${subject.label}`;
  const expectedPaths = {
    callee: `${subjectDirectory}/callee.noble`,
    import: `${subjectDirectory}/import.noble`,
    root: `${subjectDirectory}/${subject.root_kind === 'boundary' ? 'boundary' : 'caller'}.noble`,
    bindings: `${subjectDirectory}/bindings.txt`,
    wat: `${subjectDirectory}/compile.wat`,
  };
  requireThat(stable(subject.paths) === stable(expectedPaths)
    && subject.admitted_wat_path === expectedPaths.wat,
  `${subject.label}: subject artifact paths changed`);
  const sourceBytes = Object.fromEntries(['callee', 'import', 'root'].map(key => {
    const path = readOutputFile(out, subject.paths[key]);
    return [key, readFileSync(path)];
  }));
  for (const [key, bytes] of Object.entries(sourceBytes)) {
    const expected = key === 'root' ? subject.source_sha256.caller_root
      : key === 'callee' ? subject.source_sha256.callee_module : subject.source_sha256.import;
    requireThat(digest(bytes) === expected, `${subject.label}: changed ${key} source bytes`);
  }
  let expectedCallee = fixture.calleeBytes;
  let expectedImport = fixture.importBytes;
  let expectedVersion = fixture.subject.callee.version;
  const expectedRoot = subject.root_kind === 'boundary' ? fixture.boundaryBytes : fixture.callerBytes;
  if (subject.label === 'control-wrong-body') {
    expectedCallee = replaceExactly(expectedCallee, Buffer.from(' - 7 '), Buffer.from(' - 8 '),
      'same-signature callee body');
  } else if (subject.label === 'control-reversed-results') {
    expectedCallee = replaceExactly(expectedCallee, Buffer.from(' - 7 '), Buffer.from(' - 7 swap '),
      'reversed callee results');
  } else if (subject.label === 'control-version-change') {
    expectedCallee = replaceExactly(expectedCallee, Buffer.from('LinkCallee@1'), Buffer.from('LinkCallee@2'),
      'callee version');
    expectedImport = replaceExactly(expectedImport, Buffer.from('LinkCallee@1'), Buffer.from('LinkCallee@2'),
      'import version');
    expectedVersion = 2;
  } else if (subject.label === 'control-dependency-bytes') {
    expectedCallee = Buffer.concat([expectedCallee, Buffer.from('\n')]);
  } else {
    requireThat(subject.label === 'pilot-caller' || subject.label === 'callee-result-boundary',
      `${subject.label}: unknown linked subject`);
  }
  requireThat(subject.same_artifact_internal_dispatch === true
    && subject.version === expectedVersion
    && sourceBytes.callee.equals(expectedCallee)
    && sourceBytes.import.equals(expectedImport)
    && sourceBytes.root.equals(expectedRoot)
    && subject.root_kind === (subject.label === 'callee-result-boundary' || subject.label === 'control-reversed-results'
      ? 'boundary' : 'caller'),
  `${subject.label}: source bytes do not match the declared pilot/control transformation`);
  const identity = subjectIdentity({ rootBytes: sourceBytes.root, calleeBytes: sourceBytes.callee,
    importBytes: sourceBytes.import, version: subject.version, selected: fixture.subject.callee,
    rootKind: subject.root_kind });
  requireThat(stable(identity) === stable(subject.identity), `${subject.label}: source identity mismatch`);
  const watPath = readOutputFile(out, subject.admitted_wat_path);
  const wat = readFileSync(watPath);
  requireThat(digest(wat) === subject.admitted_wat_sha256 && digest(wat) === subject.compile.wat_sha256,
    `${subject.label}: admitted WAT changed`);
  const commandDir = readOutputFile(out, subject.compile.command_path);
  const command = JSON.parse(readFileSync(join(commandDir, 'command.json')));
  const calleePath = resolve(out, subject.paths.callee);
  const importPath = resolve(out, subject.paths.import);
  const rootPath = resolve(out, subject.paths.root);
  const bindingsPath = resolve(out, subject.paths.bindings);
  const expectedArgv = ['compile', rootPath, '--declared-modules', '--bindings', bindingsPath,
    '--module', calleePath, '--module', importPath,
    '--input-type', 'Bool', '--input-type', 'I64', '--input-type', 'I64'];
  const expectedRecordedArgv = ['compile', '<root>', '--declared-modules', '--bindings', '<bindings>',
    '--module', '<callee>', '--module', '<import>', '--input-type', 'Bool',
    '--input-type', 'I64', '--input-type', 'I64'];
  requireThat(command.command === join(root, 'target/debug/noble') && command.cwd === root
    && command.exit === 0 && command.signal === null && command.error === null
    && stable(command.argv) === stable(expectedArgv)
    && subject.compile.status === 0
    && stable(subject.compile.argv) === stable(expectedRecordedArgv)
    && subject.compile.wat_path === subject.paths.wat
    && subject.admitted_wat_path === subject.paths.wat,
  `${subject.label}: compile receipt is not an explicit declared-module link`);
  const typeFlags = command.argv.flatMap((argument, index) => argument === '--input-type'
    ? [command.argv[index + 1]] : []);
  requireThat(stable(typeFlags) === stable(['Bool', 'I64', 'I64']),
    `${subject.label}: compile input interface or order changed`);
  requireThat(readFileSync(bindingsPath).equals(fixture.bindingsBytes)
    && fixture.bindingsBytes.toString('utf8').trim() === '',
  `${subject.label}: unexpected engine binding grant`);
  const capturedStdout = readFileSync(join(commandDir, 'stdout'));
  const capturedStderr = readFileSync(join(commandDir, 'stderr'));
  requireThat(digest(capturedStdout) === subject.compile.stdout_sha256
    && digest(capturedStderr) === subject.compile.stderr_sha256
    && digest(capturedStdout) === subject.admitted_wat_sha256,
  `${subject.label}: raw CLI output differs from admitted WAT`);
  const compileRecordPath = readOutputFile(out, `subjects/${subject.label}/subject.json`);
  requireThat(stable(JSON.parse(readFileSync(compileRecordPath))) === stable(subject),
    `${subject.label}: retained subject manifest differs`);
}

function validateExecution(out, row, expectedStack = null) {
  const reportPath = readOutputFile(out, row.report_path);
  const preparationPath = readOutputFile(out, row.preparation_path);
  const report = JSON.parse(readFileSync(reportPath));
  const preparation = JSON.parse(readFileSync(preparationPath));
  const retainedInputs = JSON.parse(readFileSync(readOutputFile(out, row.inputs_path)));
  const sourceBytes = readFileSync(readOutputFile(out, row.source_path));
  const inputWatBytes = readFileSync(readOutputFile(out, row.input_wat_path));
  const engineWatPath = readOutputFile(out, row.engine_wat_path);
  const wasmPath = readOutputFile(out, row.wasm_path);
  requireThat(fileHash(reportPath) === row.report_sha256, `${row.label}/${row.mode}: report bytes changed`);
  requireThat(digest(sourceBytes) === row.root_source_sha256
    && report.module.source_sha256 === row.root_source_sha256
    && preparation.module.source_sha256 === row.root_source_sha256,
  `${row.label}/${row.mode}: loaded root source identity changed`);
  requireThat(digest(inputWatBytes) === row.wat_sha256
    && fileHash(engineWatPath) === row.wat_sha256
    && report.module.wat_sha256 === row.wat_sha256
    && preparation.module.wat_sha256 === row.wat_sha256,
  `${row.label}/${row.mode}: admitted or loaded WAT bytes changed`);
  requireThat(fileHash(wasmPath) === row.wasm_sha256
    && report.module.wasm_sha256 === row.wasm_sha256
    && preparation.module.wasm_sha256 === row.wasm_sha256,
  `${row.label}/${row.mode}: actual loaded final Wasm bytes changed`);
  requireThat(report.module.optimized === (row.mode === 'on')
    && preparation.module.optimized === (row.mode === 'on'), `${row.label}/${row.mode}: optimization identity changed`);
  requireThat(preparation.outcome === 'ready', `${row.label}/${row.mode}: preparation was not ready`);
  requireThat(preparation.profile === 'Declared-Modules-v1' && report.profile === 'Declared-Modules-v1',
    `${row.label}/${row.mode}: declared-module runtime profile changed`);
  requireThat(stable(row.limits) === stable(selectedLimits),
    `${row.label}/${row.mode}: runtime limits differ from the pinned ABI bounds`);
  requireThat(row.outcome === report.outcome
    && stable(row.inputs) === stable(retainedInputs)
    && stable(row.observed_stack) === stable(stackValues(report))
    && row.guest_requests === report.guest_requests
    && row.protected_operations === report.protected_operations,
  `${row.label}/${row.mode}: run summary differs from raw runtime report`);
  if (expectedStack !== null) checkStack(report, expectedStack, `${row.label}/${row.mode} verify`);
  if (expectedStack !== null) requireThat(stable(row.expected_stack) === stable(expectedStack),
    `${row.label}/${row.mode}: recorded expected stack changed`);
  requireThat(report.guest_requests === 0 && report.protected_operations === 0
    && report.host_requests === 0 && report.ambient_fallback_calls === 0
    && stable(report.request_trace ?? []) === '[]',
  `${row.label}/${row.mode}: host/protected request appeared in a scalar run`);
  return report;
}

function verifyControls(receipt, out, fixture) {
  const subjects = receipt.subjects;
  const first = vectors[0];
  const controlNames = [
    'same_signature_wrong_body', 'same_signature_identity_reuse', 'reversed_inputs',
    'reversed_results', 'wrong_return_target', 'changed_version',
    'changed_dependency_bytes', 'post_check_callee_substitution',
  ];
  requireThat(stable(Object.keys(receipt.controls).sort()) === stable(controlNames.sort()),
    'negative-control roster changed');
  const pilot = subjects['pilot-caller'];
  const wrongBodySubject = subjects['control-wrong-body'];
  const reversedResultsSubject = subjects['control-reversed-results'];
  const versionSubject = subjects['control-version-change'];
  const dependencySubject = subjects['control-dependency-bytes'];
  requireThat(pilot && wrongBodySubject && reversedResultsSubject && versionSubject && dependencySubject,
    'required compiled control subjects are missing');
  requireThat(!matchingIdentity(pilot, wrongBodySubject)
    && receipt.controls.same_signature_identity_reuse.detected === true
    && receipt.controls.same_signature_identity_reuse.reuse_accepted === false,
  'same-signature identity reuse was not refused');
  requireThat(receipt.controls.same_signature_identity_reuse.same_signature === true
    && receipt.controls.same_signature_identity_reuse.baseline_identity_sha256
      === pilot.identity.identity_sha256
    && receipt.controls.same_signature_identity_reuse.candidate_identity_sha256
      === wrongBodySubject.identity.identity_sha256,
  'same-signature reuse record is not bound to the two exact identities');
  requireThat(receipt.controls.same_signature_wrong_body.same_input_output_interface === true
    && receipt.controls.same_signature_wrong_body.identity_reuse_refused === true
    && receipt.controls.same_signature_wrong_body.baseline_subject === pilot.label
    && receipt.controls.same_signature_wrong_body.candidate_subject === wrongBodySubject.label,
  'wrong-body control identity record changed');
  requireThat(versionSubject.version === 2 && !matchingIdentity(pilot, versionSubject)
    && receipt.controls.changed_version.baseline_version === 1
    && receipt.controls.changed_version.candidate_version === 2
    && receipt.controls.changed_version.same_signature_and_finite_semantics === true
    && receipt.controls.changed_version.exact_identity_reuse_accepted === false,
  'changed version was not an identity mismatch');
  requireThat(dependencySubject.identity.selected_definition.body_source_sha256
    === pilot.identity.selected_definition.body_source_sha256
    && dependencySubject.identity.source_dependency_closure_sha256
      !== pilot.identity.source_dependency_closure_sha256
    && !matchingIdentity(pilot, dependencySubject)
    && receipt.controls.changed_dependency_bytes.selected_definition_body_unchanged === true
    && receipt.controls.changed_dependency_bytes.source_dependency_closure_changed === true
    && receipt.controls.changed_dependency_bytes.exact_identity_reuse_accepted === false,
  'changed dependency bytes were not an identity mismatch');
  requireThat(!matchingIdentity(pilot, wrongBodySubject)
    && stable(receipt.controls.same_signature_wrong_body.expected_baseline)
      === stable(expectedStacks(first, 'caller')),
  'wrong-body source identity was not refused');

  const wrongTargetControl = receipt.controls.wrong_return_target;
  requireThat(wrongTargetControl.mutation === 'enqueue stores the next function-table index, not the selected continuation index'
    && wrongTargetControl.valid_wat_preparation_required === true
    && wrongTargetControl.mutation_wat_path === 'controls/return-target/wrong-target.wat',
  'wrong-return-target control declaration changed');
  const baseWat = readFileSync(readOutputFile(out, pilot.admitted_wat_path));
  const expectedWrongTargetWat = replaceExactly(baseWat,
    Buffer.from('(i32.store (local.get $address) (local.get $index))'),
    Buffer.from('(i32.store (local.get $address) (i32.add (local.get $index) (i32.const 1)))'),
    'continuation return-target slot');
  const faultWatPath = readOutputFile(out, wrongTargetControl.mutation_wat_path);
  requireThat(fileHash(faultWatPath) === wrongTargetControl.mutation_wat_sha256
    && digest(expectedWrongTargetWat) === wrongTargetControl.mutation_wat_sha256,
  'wrong-return-target artifact bytes changed');

  for (const mode of modes) {
    const main = receipt.executions.find(row => row.root === 'caller' && row.vector === first.id && row.mode === mode);
    requireThat(main, `missing main caller run for ${mode}`);
    validateExecution(out, main, expectedStacks(first, 'caller'));
    requireThat(main.subject === 'pilot-caller' && main.source_path === pilot.paths.root
      && main.wat_sha256 === pilot.admitted_wat_sha256
      && stable(main.inputs) === stable(inputsFor(first)), 'main caller run uses a different subject/input order');
    const wrong = receipt.controls.same_signature_wrong_body.runs.find(row => row.mode === mode);
    validateExecution(out, wrong, expectedStacks(first, 'caller', { constant: 8n }));
    requireThat(wrong.subject === wrongBodySubject.label
      && wrong.wat_sha256 === wrongBodySubject.admitted_wat_sha256
      && wrong.source_path === wrongBodySubject.paths.root
      && stable(wrong.inputs) === stable(inputsFor(first)), 'wrong-body run is not the compiled same-signature subject');
    requireThat(stable(wrong.observed_stack) !== stable(expectedStacks(first, 'caller')),
      'wrong-body control did not alter ordered outputs');
    requireThat(wrong.observed_stack.length === expectedStacks(first, 'caller').length
      && stable(wrong.observed_stack.map(([type]) => type))
        === stable(expectedStacks(first, 'caller').map(([type]) => type)),
    'wrong-body control did not retain the input/output interface');
    const reversedInput = receipt.controls.reversed_inputs.runs.find(row => row.mode === mode);
    validateExecution(out, reversedInput, expectedStacks(first, 'caller', { reverseInputs: true }));
    requireThat(reversedInput.subject === pilot.label
      && reversedInput.wat_sha256 === pilot.admitted_wat_sha256
      && stable(reversedInput.inputs) === stable(inputsFor(first, true)),
    'reversed-input control is not the caller subject with reversed operands');
    requireThat(stable(receipt.controls.reversed_inputs.actual_input_order)
        === stable(['Bool-prefix', 'y', 'x'])
      && stable(receipt.controls.reversed_inputs.expected_order) === stable(['Bool-prefix', 'x', 'y']),
    'reversed-input control order record changed');
    const reversedResult = receipt.controls.reversed_results.runs.find(row => row.mode === mode);
    validateExecution(out, reversedResult, expectedStacks(first, 'boundary', { reverseResults: true }));
    requireThat(reversedResult.subject === reversedResultsSubject.label
      && reversedResult.wat_sha256 === reversedResultsSubject.admitted_wat_sha256
      && stable(reversedResult.inputs) === stable(inputsFor(first)),
    'reversed-result control is not the compiled reversed-order subject');
    requireThat(receipt.controls.reversed_results.same_input_output_interface === true
      && stable(receipt.controls.reversed_results.actual_order) === stable(['7', 'wrap64(x-y)'])
      && stable(receipt.controls.reversed_results.expected_order)
        === stable(['wrap64(x-y)', '7']),
    'reversed-result control order record changed');
    const version = receipt.controls.changed_version.runs.find(row => row.mode === mode);
    validateExecution(out, version, expectedStacks(first, 'caller'));
    requireThat(version.subject === versionSubject.label
      && version.wat_sha256 === versionSubject.admitted_wat_sha256,
    'version control is not the compiled changed-version subject');
    const dependency = receipt.controls.changed_dependency_bytes.runs.find(row => row.mode === mode);
    validateExecution(out, dependency, expectedStacks(first, 'caller'));
    requireThat(dependency.subject === dependencySubject.label
      && dependency.wat_sha256 === dependencySubject.admitted_wat_sha256,
    'dependency control is not the compiled changed-dependency subject');
    const substitution = receipt.controls.post_check_callee_substitution.substituted_runs
      .find(row => row.mode === mode);
    validateExecution(out, substitution, expectedStacks(first, 'caller', { constant: 8n }));
    requireThat(substitution.subject === pilot.label
      && substitution.source_path === pilot.paths.root
      && substitution.wat_sha256 === wrongBodySubject.admitted_wat_sha256
      && substitution.wasm_sha256 !== receipt.controls.post_check_callee_substitution.checked_wasm_hashes[mode],
    'post-check substitution did not replace the final linked artifact');
    const returnTarget = receipt.controls.wrong_return_target.runs.find(row => row.mode === mode);
    const wrongTargetReport = validateExecution(out, returnTarget);
    requireThat(returnTarget.subject === pilot.label
      && returnTarget.wat_sha256 === wrongTargetControl.mutation_wat_sha256
      && stable(returnTarget.inputs) === stable(inputsFor(first)),
    'wrong-return-target execution did not use the mutated compiled artifact');
    requireThat(wrongTargetReport.outcome !== 'normal'
      || stable(stackValues(wrongTargetReport)) !== stable(expectedStacks(first, 'caller')),
    `wrong-return-target control did not distinguish mode ${mode}`);
  }
  requireThat(pilot.identity.selected_definition.definition_slot
    === fixture.subject.callee.definition_slot, 'selected definition slot differs from fixture model');
  const substitutionControl = receipt.controls.post_check_callee_substitution;
  const checkedWasmHashes = Object.fromEntries(receipt.executions.filter(row => row.root === 'caller'
    && row.vector === first.id).map(row => [row.mode, row.wasm_sha256]));
  requireThat(substitutionControl.checked_subject === pilot.label
    && substitutionControl.substituted_artifact_subject === wrongBodySubject.label
    && substitutionControl.caller_source_hash_unchanged === true
    && pilot.source_sha256.caller_root === wrongBodySubject.source_sha256.caller_root
    && substitutionControl.same_signature === true
    && substitutionControl.checked_wat_sha256 === pilot.admitted_wat_sha256
    && substitutionControl.substituted_wat_sha256 === wrongBodySubject.admitted_wat_sha256
    && stable(substitutionControl.checked_wasm_hashes) === stable(checkedWasmHashes)
    && substitutionControl.receipt_identity_rejects_substitution === true
    && substitutionControl.engine_source_hash_alone_is_not_semantic_authority === true,
  'post-check substitution identity record changed');
  for (const [name, control] of Object.entries(receipt.controls)) {
    requireThat(control.detected === true, `negative control missing or not detected: ${name}`);
  }
}

function verifyEvidence(receiptPath) {
  checkEnvironment();
  const requestedReceipt = resolve(receiptPath);
  const resolvedReceipt = realpathSync(requestedReceipt);
  requireThat(resolvedReceipt === requestedReceipt, 'linking receipt path must not be a symlink');
  requireThat(resolvedReceipt.endsWith(`${sep}linking.json`), 'verification expects the linking.json receipt');
  const out = dirname(resolvedReceipt);
  requireThat(realpathSync(out) === out && !out.startsWith(`${root}${sep}`) && out !== root,
    'linking receipt must be a regular external artifact, not a symlink or in-tree output');
  const actualArtifacts = artifactIndex(out);
  const receipt = JSON.parse(readFileSync(resolvedReceipt));
  requireThat(receipt.schema === 'noble-linking-execution/v1' && receipt.issue === 52,
    'linking receipt schema/issue mismatch');
  requireThat(receipt.profile === profile && stable(receipt.claim_boundary) === stable(claimBoundary)
    && receipt.artifact_directory === '.'
    && Array.isArray(receipt.command) && receipt.command.length === 5
    && receipt.command[0] === 'verification/linking/run.mjs'
    && receipt.command[1] === '--foundation'
    && typeof receipt.command[2] === 'string' && receipt.command[2].startsWith(sep)
    && receipt.command[3] === '--output' && receipt.command[4] === out,
  'linking receipt command or claim boundary changed');
  const state = gitState();
  requireThat(state.status === '' && receipt.clean_source === true
    && receipt.source_commit === state.head && receipt.source_tree === state.tree,
  'receipt does not bind the current clean committed source revision/tree');
  verifySourceHashes(receipt);
  const foundationPath = readOutputFile(out, receipt.proof_binding.receipt_path);
  const foundationBytes = readFileSync(foundationPath);
  const foundation = JSON.parse(foundationBytes);
  const proof = proofBinding(foundation, foundationBytes, state);
  requireThat(stable(proof) === stable(receipt.proof_binding), 'foundation proof receipt/theorem inventory changed');
  const fixture = fixtureData();
  requireThat(stable(receipt.fixture) === stable(fixtureRecord(fixture))
    && stable(receipt.bounds) === stable(bounds), 'fixture or finite state/ABI premises changed');
  requireThat(stable(currentSourceHashes(foundation)) === stable(receipt.source_hashes),
    'linking source hash inventory is incomplete or changed');
  requireThat(receipt.runtime.runtime_config_sha256 === fileHash(repoPath(runtimeConfigPath))
    && receipt.runtime.abi_sha256 === fileHash(repoPath(abiPath))
    && receipt.runtime.profile === runtimeConfig.target
    && receipt.runtime.backend === 'managed-linear-memory'
    && receipt.runtime.abi_version === abi.version
    && stable(receipt.runtime.node_flags) === stable(runtimeConfig.node_flags),
  'runtime or ABI revision changed');
  requireThat(stable(receipt.runtime.helper_source_hashes)
    === stable(Object.fromEntries(Object.entries(receipt.source_hashes)
      .filter(([path]) => path.startsWith('crates/noble-cli/src/core/runtime/'))))
    && stable(receipt.runtime.runtime_assets) === stable(runtimeConfig.runtime_assets),
  'runtime helper or embedded asset inventory changed');
  requireThat(stable(receipt.runtime.limits) === stable(selectedLimits)
    && stable(receipt.runtime.opt_modes) === stable(modes), 'runtime bounds or optimization roster changed');
  requireThat(stable(receipt.vectors) === stable(vectors.map(asVectorJson)),
    'fixed deterministic vector roster changed');
  const vendor = selectedVendor(selection.component_sync);
  const currentTools = selectedTools(vendor);
  for (const key of ['node', 'cargo', 'rustc', 'wasm_tools', 'wasm_opt', 'nix']) {
    requireThat(stable(receipt.toolchain[key]) === stable(currentTools[key]), `pinned tool drift: ${key}`);
  }
  requireThat(stable(receipt.toolchain.vendor) === stable(currentTools.vendor)
    && receipt.toolchain.cli.path === 'target/debug/noble', 'selected vendor or CLI path changed');
  const cliPath = join(root, receipt.toolchain.cli.path);
  requireThat(fileHash(cliPath) === receipt.toolchain.cli.sha256, 'built Noble CLI bytes changed or are unavailable');
  const wantedPaths = Object.keys(receipt.artifacts_sha256 ?? {}).sort();
  const actualPaths = Object.keys(actualArtifacts).sort();
  requireThat(stable(actualPaths) === stable(wantedPaths), 'linking artifact set differs from receipt');
  const artifactDrift = wantedPaths.filter(path => actualArtifacts[path] !== receipt.artifacts_sha256[path]);
  requireThat(artifactDrift.length === 0, `linking artifacts changed: ${artifactDrift.join(', ')}`);
  requireThat(receipt.execution_count === receipt.executions.length, 'execution count mismatch');
  requireThat(receipt.executions.length === vectors.length * modes.length * 2,
    'finite run roster is incomplete');
  const subjectNames = [
    'pilot-caller', 'callee-result-boundary', 'control-wrong-body',
    'control-reversed-results', 'control-version-change', 'control-dependency-bytes',
  ];
  requireThat(stable(Object.keys(receipt.subjects).sort()) === stable(subjectNames.sort()),
    'compiled subject roster changed');
  for (const mode of modes) {
    for (const vector of vectors) {
      const caller = receipt.executions.find(row => row.root === 'caller' && row.vector === vector.id && row.mode === mode);
      const boundary = receipt.executions.find(row => row.root === 'boundary' && row.vector === vector.id && row.mode === mode);
      requireThat(caller && boundary, `missing normal execution ${vector.id}/${mode}`);
      validateExecution(out, caller, expectedStacks(vector, 'caller'));
      validateExecution(out, boundary, expectedStacks(vector, 'boundary'));
      const subject = receipt.subjects[caller.subject];
      const boundarySource = receipt.subjects[boundary.subject];
      requireThat(caller.subject === 'pilot-caller' && subject
        && subject.identity.caller_root_sha256 === caller.root_source_sha256
        && caller.wat_sha256 === subject.admitted_wat_sha256
        && caller.source_path === subject.paths.root
        && stable(caller.inputs) === stable(inputsFor(vector)),
        `${caller.label}: caller root hash is not subject-bound`);
      requireThat(boundary.subject === 'callee-result-boundary' && boundarySource
        && boundary.wat_sha256 === boundarySource.admitted_wat_sha256
        && boundary.source_path === boundarySource.paths.root
        && stable(boundary.inputs) === stable(inputsFor(vector)),
      `${boundary.label}: two-result boundary is not subject-bound`);
    }
  }
  for (const subject of Object.values(receipt.subjects)) validateSubject(out, subject, fixture);
  verifyControls(receipt, out, fixture);
  const finalState = gitState();
  requireThat(finalState.head === state.head && finalState.tree === state.tree && finalState.status === state.status,
    'source worktree changed during verification');
  console.log(json({ schema: receipt.schema, verified: true, source_commit: state.head,
    source_tree: state.tree, normal_executions: receipt.executions.length,
    controls: Object.fromEntries(Object.entries(receipt.controls).map(([key, value]) => [key, value.detected])),
    artifact_count: actualPaths.length, receipt_sha256: fileHash(resolvedReceipt) }));
}

function main() {
  const args = process.argv.slice(2);
  if (args[0] === '--verify') {
    requireThat(args.length === 2, 'usage: run.mjs --verify RECEIPT/linking.json');
    verifyEvidence(args[1]);
    return;
  }
  requireThat(args.length === 4 && args[0] === '--foundation' && args[2] === '--output',
    'usage: run.mjs --foundation FOUNDATION_CHECK_JSON --output NEW_DIR | --verify RECEIPT/linking.json');
  runEvidence({ foundationPath: args[1], output: args[3] });
}

try {
  main();
} catch (error) {
  console.error(`[linking] FAIL ${error.message}`);
  process.exitCode = 1;
}
