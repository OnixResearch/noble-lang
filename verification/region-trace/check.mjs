// Lean-kernel check for the region-trace relation pilot (`.cairn/changes/region-trace-relation`,
// WIT-TRACE-01). Builds `proofs/m5/RegionTrace.lean` and the generated runtime fixtures
// `proofs/m5/RegionTraceRuntime.lean` in an isolated project with the pinned Lean toolchain and
// audits every inventoried theorem's statement and transitive axioms. The runtime fixtures must be
// exactly what `runtime-fixtures.mjs` generates from the retained effect-protocol receipt.
// This is a model theorem check: no emitted-step correspondence, no Wasmtime claim.
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdirSync, readFileSync, realpathSync, writeFileSync } from 'node:fs';
import { userInfo } from 'node:os';
import { basename, dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  generate, inventoryFile as runtimeInventoryFile, leanFile as runtimeFile, observation, observe, receiptFile,
} from './runtime-fixtures.mjs';

const root = resolve(fileURLToPath(import.meta.url), '../../..');
const selection = JSON.parse(readFileSync(join(root, 'policy/tool-selection.json')));
const node = join(selection.tool_paths.node.output, 'bin/node');
const leanBin = join(selection.tool_paths.lean.output, 'bin');
const git = realpathSync('/run/current-system/sw/bin/git');
const { homedir: home, username } = userInfo();
const childEnv = {
  HOME: home, USER: username, LOGNAME: username, LANG: 'C.UTF-8', CI: '1',
  PATH: [leanBin, dirname(git), '/run/current-system/sw/bin', '/usr/bin', '/bin'].join(':'),
};
const requireThat = (condition, message) => { if (!condition) throw Error(message); };
const codeFile = 'proofs/m5/RegionTrace.lean';
const inventoryFile = 'proofs/m5/region-trace-inventory.json';
const cleanupEvidenceFile = 'verification/host-locality/evidence-1bf39a52.json';
const codeFiles = [codeFile, runtimeFile];
const standardAxioms = new Set(['propext', 'Classical.choice', 'Quot.sound']);
let output;
const commands = [];
function run(command, argv, cwd = root) {
  const observed = spawnSync(command, argv, {
    cwd, env: { ...childEnv, TMPDIR: output }, timeout: 3_600_000,
    maxBuffer: 64 * 1024 * 1024, encoding: 'utf8',
  });
  commands.push({ command, argv, exit: observed.status });
  requireThat(observed.status === 0, `${command} ${argv.join(' ')} failed: ${observed.status}\n${observed.stderr ?? ''}`);
  return observed.stdout ?? '';
}
function checkDeclarations(inventory) {
  const names = inventory.theorems.map(row => row.name);
  requireThat(names.length > 0 && new Set(names).size === names.length, 'empty/duplicate theorem inventory');
  const declared = [];
  for (const file of codeFiles) {
    const text = readFileSync(join(root, file), 'utf8');
    const code = text.replace(/\/-[\s\S]*?-\//g, '').replace(/--.*$/gm, '');
    requireThat(!/\b(?:sorry|admit|native_decide|implemented_by|unsafe|opaque|partial)\b/.test(code),
      `${file}: forbidden proof or evaluation escape hatch`);
    requireThat(!/@\[\s*extern\b/.test(code), `${file}: external implementation attribute`);
    requireThat(!/^\s*axiom\s/m.test(code), `${file}: declares an axiom`);
    let namespace = '';
    for (const line of code.split('\n')) {
      const opened = line.match(/^namespace\s+(\S+)/);
      if (opened) namespace = opened[1];
      const theorem = line.match(/^\s*(?:@\[[^\]]*\]\s*)?(?:theorem|lemma)\s+(\S+)/);
      if (theorem) declared.push(`${namespace}.${theorem[1]}`);
    }
  }
  requireThat(names.every(name => name.startsWith('RegionTrace.')), 'unrelated theorem in inventory');
  requireThat(names.length === declared.length && names.every(name => declared.includes(name)),
    `declaration/inventory mismatch: declared=${declared.join(',')} inventoried=${names.join(',')}`);
  return names;
}
function auditTheorems(project, inventory, names) {
  const auditFile = join(output, 'Audit.lean');
  const lines = ['import RegionTrace', 'import RegionTraceRuntime', ''];
  const positions = {};
  for (const name of names) {
    positions[lines.length + 1] = { name, kind: 'statement' }; lines.push(`#check @${name}`);
    positions[lines.length + 1] = { name, kind: 'axioms' }; lines.push(`#print axioms ${name}`);
  }
  writeFileSync(auditFile, lines.join('\n') + '\n');
  const outputText = run(join(leanBin, 'lake'), ['env', 'lean', '--json', auditFile], project);
  const records = {};
  for (const line of outputText.split('\n').filter(Boolean)) {
    const message = JSON.parse(line);
    requireThat(message.severity === 'information', `Lean audit diagnostic: ${line}`);
    const slot = positions[message.pos.line];
    requireThat(slot, `unexpected Lean audit output at ${message.pos.line}`);
    (records[slot.name] ??= {})[slot.kind] = message.data.trim();
  }
  return names.map(name => {
    const row = records[name];
    const metadata = inventory.theorems.find(item => item.name === name);
    requireThat(row?.statement && row?.axioms, `missing compiled statement/axioms: ${name}`);
    const match = row.axioms.match(/depends on axioms: \[([\s\S]*)\]/);
    const axioms = /does not depend on any axioms/.test(row.axioms) ? [] :
      (requireThat(match, `unparsable axiom report: ${name}`), match[1].split(',').map(value => value.trim()).filter(Boolean));
    const allowed = new Set(metadata.allowed_standard_axioms ?? standardAxioms);
    requireThat(axioms.every(axiom => allowed.has(axiom)), `disallowed transitive axiom: ${name}: ${axioms.join(', ')}`);
    return { ...metadata, status: 'proved', statement: row.statement.replace(/\s+/g, ' '), axioms };
  });
}

// WIT-TRACE-06: the revisions the recorded observations came from, each labelled with what this
// replay actually exercised. A component is bound by the byte strings the observation receipt
// retains, so re-hashing it is a real test; the helper and engine are bound by versions that must
// agree with the pinned tool selection and the Cargo.lock entry. Every binding keeps
// `correspondence: open`: the route is named, no emitted-step correspondence is proved.
function digest64(bytes) {
  return createHash('sha256').update(bytes).digest('hex');
}
function lockVersions(text, name) {
  return text.split('[[package]]')
    .map(block => ({
      name: block.match(/^\s*name = "([^"]+)"/m)?.[1],
      version: block.match(/^\s*version = "([^"]+)"/m)?.[1],
    }))
    .filter(entry => entry.name === name && entry.version !== undefined)
    .map(entry => entry.version);
}
function resolveBindings({
  receipt = observation(),
  lock = readFileSync(join(root, 'Cargo.lock'), 'utf8'),
  policy = selection.component_sync,
} = {}) {
  const wasmtime = lockVersions(lock, 'wasmtime');
  requireThat(wasmtime.length === 1, `Cargo.lock names ${wasmtime.length} wasmtime entries`);
  requireThat(receipt.toolchain.wasmtime === wasmtime[0],
    `engine: the receipt names wasmtime ${receipt.toolchain.wasmtime}, Cargo.lock ${wasmtime[0]}`);
  requireThat(policy.wasmtime_version === wasmtime[0],
    `engine: the pinned selection names wasmtime ${policy.wasmtime_version}, Cargo.lock ${wasmtime[0]}`);
  requireThat(receipt.toolchain.wasm_tools === `wasm-tools ${policy.wasm_tools_version}`,
    `helper: the receipt names ${receipt.toolchain.wasm_tools}, the pinned selection ${policy.wasm_tools_version}`);
  const components = {};
  for (const component of receipt.components) {
    const bytes = Buffer.from(component.base64, 'base64');
    requireThat(bytes.toString('base64') === component.base64,
      `component ${component.path}: the retained base64 is not canonical`);
    requireThat(digest64(bytes) === component.sha256,
      `component ${component.path}: the retained bytes do not hash to its recorded sha256`);
    components[component.path] = component.sha256;
  }
  requireThat(Object.keys(components).length === receipt.components.length,
    'a component path appears twice in the observation receipt');
  requireThat(/^[a-f0-9]{64}$/.test(receipt.toolchain.cli_sha256),
    'helper: the receipt does not carry a SHA-256 CLI binding');
  return {
    component: {
      status: 'tested',
      correspondence: 'open',
      components,
      verified: 'every component byte string the observation receipt retains hashes to its recorded sha256, and the artifact map agrees with each component record. The emitted component is not a subject of these theorems',
    },
    helper: {
      status: 'tested',
      correspondence: 'open',
      cli_sha256: receipt.toolchain.cli_sha256,
      wasm_tools_version: receipt.toolchain.wasm_tools,
      vendor_nar_hash: receipt.toolchain.vendor_nar_hash,
      verified: 'the wasm-tools that assembled the hostile guests equals the pinned tool selection. The CLI is the one the observation receipt recorded by hash; it is not re-hashed here',
    },
    engine: {
      status: 'tested',
      correspondence: 'open',
      wasmtime_version: receipt.toolchain.wasmtime,
      cargo_lock_version: wasmtime[0],
      tool_selection_version: policy.wasmtime_version,
      verified: 'the Wasmtime version the receipt names agrees with the Cargo.lock entry and the pinned tool selection',
    },
  };
}

// A stale binding must fail the replay (WIT-TRACE-06). Each control mutates one input of the real
// resolution and requires a refusal; a control that resolves is a failure, not a warning.
function staleBindingControls(real = observation()) {
  const lock = readFileSync(join(root, 'Cargo.lock'), 'utf8');
  const copy = () => JSON.parse(JSON.stringify(real));
  const controls = [];
  const refuse = (id, { receipt = real, lockText = lock, policy = selection.component_sync }) => {
    let refused = false;
    try {
      resolveBindings({ receipt, lock: lockText, policy });
    } catch {
      refused = true;
    }
    requireThat(refused, `${id}: a stale binding did not fail the replay`);
    controls.push({ id, expected: 'refused', observed: 'refused' });
  };
  const staleDigest = copy();
  staleDigest.components[0].sha256 =
    (staleDigest.components[0].sha256[0] === '0' ? '1' : '0') + staleDigest.components[0].sha256.slice(1);
  refuse('stale-component-digest', { receipt: staleDigest });
  const staleBytes = copy();
  const bytes = staleBytes.components[0].base64;
  staleBytes.components[0].base64 = (bytes[0] === 'A' ? 'B' : 'A') + bytes.slice(1);
  refuse('stale-component-bytes', { receipt: staleBytes });
  const staleHelper = copy();
  staleHelper.toolchain.wasm_tools = 'wasm-tools 1.245.2';
  refuse('stale-helper-receipt-wasm-tools', { receipt: staleHelper });
  refuse('stale-helper-wasm-tools',
    { policy: { ...selection.component_sync, wasm_tools_version: '1.245.2' } });
  const staleCliHash = copy();
  staleCliHash.toolchain.cli_sha256 =
    (staleCliHash.toolchain.cli_sha256[0] === '0' ? '1' : '0') + staleCliHash.toolchain.cli_sha256.slice(1);
  const receiptBytes = readFileSync(join(root, receiptFile), 'utf8');
  const cliBinding = receiptBytes.match(/("cli_sha256"\s*:\s*")([a-f0-9]{64})(")/);
  requireThat(cliBinding?.[2] === real.toolchain.cli_sha256,
    'the retained effect-protocol receipt does not contain the bound CLI hash');
  const staleCliBytes = Buffer.from(receiptBytes.replace(cliBinding[0],
    `${cliBinding[1]}${staleCliHash.toolchain.cli_sha256}${cliBinding[3]}`));
  let staleCliRefused = false;
  try {
    observation(staleCliBytes);
  } catch {
    staleCliRefused = true;
  }
  requireThat(staleCliRefused, 'stale-observation-receipt: a modified receipt did not fail its pinned digest');
  controls.push({ id: 'stale-observation-receipt', expected: 'refused', observed: 'refused' });
  const staleEngine = copy();
  staleEngine.toolchain.wasmtime = '40.0.3';
  refuse('stale-engine-receipt-version', { receipt: staleEngine });
  refuse('stale-engine-selection',
    { policy: { ...selection.component_sync, wasmtime_version: '40.0.3' } });
  refuse('stale-engine-cargo-lock', {
    lockText: lock.replace(/name = "wasmtime"\nversion = "40\.0\.2"/,
      'name = "wasmtime"\nversion = "40.0.3"'),
  });
  return controls;
}
function refusedRetirementFixtureControl() {
  const observed = observe({
    id: 'EPROTO-CASE-01', variant: 'synthetic-cleanup-refusal',
    observed: {
      exit_class: 'trap', events_truncated: false,
      events: [
        { kind: 'request', operation: 'host.retire', identity: 1 },
        { kind: 'response', result: 'refused', reason: 'invalid-handle' },
      ],
    },
  });
  requireThat(observed.length === 2 && observed[0].type === 'exit' && observed[0].cls === 'trap'
    && observed[1].type === 'retirementRefused' && observed[1].identity === '1'
    && observed[1].reason === 'invalid-handle',
  'runtime-fixtures did not retain refused host retirement as cleanup failure without a commit');
  return { id: 'synthetic-refused-host-retirement', expected: 'cleanup-refusal-without-commit',
    observed: 'cleanup-refusal-without-commit' };
}
const argv = process.argv.slice(2);
const allowDirty = argv.includes('--allow-dirty');
if (allowDirty) argv.splice(argv.indexOf('--allow-dirty'), 1);
requireThat(argv.length === 2 && argv[0] === '--output',
  'usage: SELECTED_NODE verification/region-trace/check.mjs [--allow-dirty] --output NEW_EXTERNAL_DIRECTORY');
requireThat(realpathSync(process.execPath) === realpathSync(node), `use selected Node: ${node}`);
const forbidden = Object.keys(process.env).filter(key =>
  /^(?:LEAN(?:_|$)|LAKE(?:_|$)|BASH(?:_|$)|GIT_|NIX_|ELAN)/.test(key) && process.env[key] !== '');
requireThat(forbidden.length === 0, `unsafe caller environment: ${forbidden.join(', ')}`);
output = resolve(argv[1]);
requireThat(output !== root && !output.startsWith(`${root}/`), 'output must be external');
mkdirSync(output);
const initial = {
  commit: run(git, ['rev-parse', 'HEAD']).trim(),
  tree: run(git, ['rev-parse', 'HEAD^{tree}']).trim(),
  status: run(git, ['status', '--porcelain=v1', '--untracked-files=all']).trim(),
};
requireThat(allowDirty || initial.status === '', `source worktree dirty: ${initial.status}`);
const generated = generate();
requireThat(readFileSync(join(root, runtimeFile), 'utf8') === generated.lean,
  `${runtimeFile} differs from what runtime-fixtures.mjs generates from ${receiptFile}`);
requireThat(readFileSync(join(root, runtimeInventoryFile), 'utf8') === generated.inventory,
  `${runtimeInventoryFile} differs from what runtime-fixtures.mjs generates`);
const inventory = {
  theorems: [inventoryFile, runtimeInventoryFile]
    .flatMap(file => JSON.parse(readFileSync(join(root, file))).theorems),
};
const names = checkDeclarations(inventory);
const version = run(join(leanBin, 'lean'), ['--version']).trim();
requireThat(version.includes('version 4.35.0') && version.includes('eb6cafb7290e8bd5f2937522d8d804f58f02bafc'),
  'wrong pinned Lean executable');
const project = join(output, 'project');
mkdirSync(project);
writeFileSync(join(project, 'RegionTrace.lean'), readFileSync(join(root, codeFile)));
writeFileSync(join(project, 'RegionTraceRuntime.lean'), readFileSync(join(root, runtimeFile)));
writeFileSync(join(project, 'lean-toolchain'), readFileSync(join(root, 'proofs/m5/lean-toolchain')));
writeFileSync(join(project, 'lakefile.toml'),
  ['name = "noble_region_trace"', 'defaultTargets = ["RegionTrace", "RegionTraceRuntime"]', '',
   '[[lean_lib]]', 'name = "RegionTrace"', '', '[[lean_lib]]', 'name = "RegionTraceRuntime"', ''].join('\n'));
run(join(leanBin, 'lake'), ['build', 'RegionTrace', 'RegionTraceRuntime'], project);
const theorems = auditTheorems(project, inventory, names);
const final = {
  commit: run(git, ['rev-parse', 'HEAD']).trim(),
  tree: run(git, ['rev-parse', 'HEAD^{tree}']).trim(),
  status: run(git, ['status', '--porcelain=v1', '--untracked-files=all']).trim(),
};
requireThat(final.status === initial.status && final.commit === initial.commit, 'source tree changed during proof check');
const observed = observation();
const evidence = resolveBindings({ receipt: observed });
const bindingControls = staleBindingControls(observed);
const fixtureControls = [refusedRetirementFixtureControl()];
const bindings = {
  // WIT-TRACE-06: every revision this pilot's verdicts depend on is bound. `model` and
  // `observations` were already bound; `component`, `helper` and `engine` say what was exercised.
  model: {
    status: 'tested',
    theorem_status: 'proved',
    lean: version,
    lean_sha256: spawnSync('sha256sum', [join(leanBin, 'lean')], { encoding: 'utf8' }).stdout.split(' ')[0],
    sources_sha256: Object.fromEntries([codeFile, inventoryFile, runtimeFile, runtimeInventoryFile]
      .map(file => [file, spawnSync('sha256sum', [join(root, file)], { encoding: 'utf8' }).stdout.split(' ')[0]])),
  },
  // The runtime fixtures are generated from this retained receipt of real executions.
  observations: {
    status: 'tested',
    correspondence: 'open',
    receipt: receiptFile,
    receipt_sha256: observed.receipt_sha256,
    source_commit: observed.source.commit,
    source_tree: observed.source.tree,
  },
  ...evidence,
  correspondence_route: [
    'Canonical ABI lift/lower of a guest request -> importCrossing plus the host decision; its lowered result -> returned.',
    'Host::read / Host::release / Host::abnormal_exit_cleanup -> hostDecision, protectedCopy and cleanup.',
    'A Wasmtime Trap (OutOfFuel included) or a host cancellation -> abnormalExit with the class the report names; a result outside the schema -> abnormalExit outsideModel; a refused retirement -> cleanupFailure, committing nothing.',
    'The route is named, not proved. PO-17/PO-18 cover emitted-component evidence, and #10 owns any real lowering correspondence.',
  ],
};
const claims = {
  model_theorems: names.length,
  model_theorems_status: 'proved',
  implementation_theorems: 0,
  runtime_fixtures: {
    status: 'tested', related: generated.related, related_under_registration: generated.registered,
  },
  correspondence: {
    status: 'open',
    claim: 'none proved: the route is named and the revisions are bound, no emitted-step correspondence is proved',
  },
  liveness: { status: 'open', claim: 'none' },
  cancellation: {
    status: 'tested',
    claim: 'the host reports the cancelled exit class and the projection erases it; no general cancellation API or liveness property is claimed',
  },
  cleanup_failure: {
    host_method_status: 'tested',
    guest_path_status: 'open',
    evidence: `${cleanupEvidenceFile}: cleanup-release-refusal-retains-owner`,
    evidence_sha256: digest64(readFileSync(join(root, cleanupEvidenceFile))),
  },
  assumptions: [
    {
      name: 'host-identity-supply', status: 'assumed',
      claim: 'the host supplies the exact identity that authorized each protected commit',
    },
    {
      name: 'request-response-pairing', status: 'assumed',
      claim: 'a response in an execution answers the request whose operation the target model pairs with it',
    },
    {
      name: 'fuel-accounting', status: 'assumed',
      claim: 'an exhaustion observation cuts the target trace at its recorded committed prefix; it does not authorize dropping a prior protected commit',
    },
  ],
  open_items: [
    { name: 'emitted-step-correspondence', status: 'open', owner: '#10' },
    {
      name: 'guest-reachable-cleanup-refusal', status: 'open',
      claim: 'the refused-retirement path is retained at the host method but cannot be induced through the guest interface',
    },
    { name: 'liveness', status: 'open' },
    { name: 'general-cancellation-api', status: 'open' },
  ],
};
const receipt = {
  schema: 'noble-region-trace-proof/v1', result: 'passed',
  source_commit: initial.commit, source_tree: initial.tree, clean_source: initial.status === '',
  allow_dirty: allowDirty, lean: version,
  bindings, claims,
  sources_sha256: Object.fromEntries([codeFile, inventoryFile, runtimeFile, runtimeInventoryFile,
    'verification/region-trace/check.mjs', 'verification/region-trace/runtime-fixtures.mjs',
    cleanupEvidenceFile, receiptFile].map(file => [file,
      spawnSync('sha256sum', [join(root, file)], { encoding: 'utf8' }).stdout.split(' ')[0]])),
  theorem_count: names.length, theorems, binding_controls: bindingControls,
  fixture_controls: fixtureControls, commands,
  implementation_theorem: false,
  scope: ['.cairn/changes/region-trace-relation WIT-TRACE-01 model: source alphabet, target ABI alphabet and the relation.',
    'The target alphabet is a handwritten machine. No emitted ABI step correspondence, no Wasmtime/Canonical ABI claim,',
    'no Rust host or lowering claim, and no liveness claim. The component, helper and engine bindings record',
    'the revisions the observed traces came from and label them tested; they are not a correspondence.',
    'The host records its own ordered trace with',
    'the exact retained identity, and pairing is stated over projected source traces rather than proved',
    'for the emitted component own steps; the observed-trace lowering maps an observation, not a',
    'correspondence. The runtime fixtures are finite tests of the traces in the retained effect-protocol',
    'receipt: each recorded observation, its exit class included, lowers to a trace the relation accepts',
    'or, for a host retirement before any request, accepts under the registration the host report names',
    '(relatedFrom).'],
};
writeFileSync(join(output, 'receipt.json'), JSON.stringify(receipt, null, 2) + '\n');
process.stdout.write(`REGION-TRACE: ${names.length} model theorems; strict axioms accepted\n`);
