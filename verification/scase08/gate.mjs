#!/usr/bin/env node
// Finite compiled-component callback matrix. This gate freezes the absent
// canonical case and writes a new external receipt; it never promotes the case.
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import {spawnSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';

const root = fs.realpathSync(path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..'));
const sha = value => crypto.createHash('sha256').update(value).digest('hex');
const read = file => fs.readFileSync(path.join(root, file));
const caseFile = 'specs/conformance/safety-cases.json';
const caseBytes = read(caseFile);
const cases = JSON.parse(caseBytes).cases;
assert.equal(cases.filter(row => row.id === 'S-CASE-08').length, 1,
  'exactly one canonical S-CASE-08 design required');
const design = cases.find(row => row.id === 'S-CASE-08');
assert.deepEqual(design.input, {
  harness:'callback-handle-matrix', mutations:['stale-generation','wrong-kind','wrong-context'],
});
assert.deepEqual(design.expected, {
  stage:'callback', outcome:'invalid-result', trusted_values_created:0,
});
assert.deepEqual(design.state, {
  implementation:'absent', execution:'not-run', proof:'open', trust:'unassessed',
});
assert.deepEqual(design.evidence, []);
assert.equal(design.profile, 'Resources-Draft');
assert.equal(design.kind, 'adapter');
assert.deepEqual(design.requirements, ['S-HOST-01','S-HOST-02','S-MEM-02']);
assert.deepEqual(caseBytes.toString('utf8').split('\n')
  .filter(line => line.includes('"id":"S-CASE-08"')),
  [`    ${JSON.stringify(design)},`], 'canonical absent row must retain exact bytes');

assert.equal(process.argv.length, 3,
  'usage: <selected node> verification/scase08/gate.mjs NEW_EXTERNAL_DIRECTORY');
const selected = JSON.parse(read('policy/tool-selection.json'));
const runtime = JSON.parse(read('crates/noble-cli/src/core/runtime/config.json'));
const node = path.join(selected.tool_paths.node.output, 'bin/node');
const wasmTools = path.join(selected.tool_paths.wasm_tools.output, 'bin/wasm-tools');
assert.equal(fs.realpathSync(process.execPath), fs.realpathSync(node), 'selected Node required');
assert.equal(fs.realpathSync(runtime.tools.node.path), fs.realpathSync(node),
  'selected Node must match production runtime');
assert.equal(fs.realpathSync(runtime.tools.wasm_tools.path), fs.realpathSync(wasmTools),
  'selected wasm-tools must match production compiler');
const output = path.resolve(process.argv[2]);
const external = destination => destination !== root && !destination.startsWith(`${root}/`);
assert.ok(external(output), 'external output required');
assert.ok(external(fs.realpathSync(path.dirname(output))), 'external output parent required');
fs.mkdirSync(output); // Existing directories/receipts must never be overwritten.
assert.ok(external(fs.realpathSync(output)), 'external output symlink forbidden');
fs.writeFileSync(path.join(output, 'prepromotion-safety-cases.json'), caseBytes, {flag:'wx'});

// The canonical evidence identifies old receipts independently of our gate.
// Retain every accepted observation, including the active S-CASE-04/06 paths.
const priorReceiptSha256 = {
  'verification/scase02/acceptance.json':'23f2b73aff9ee4dfa0bbd7d613dba3e569caa316409a49abe9f7884eddcec0b1',
  'verification/scase02/corrected-acceptance.json':'3588522e32149a490ea0ec8b7c33abbcb229150b91e24a31d164a17b60a740d0',
  'verification/scase04/acceptance.json':'16ba34b6672486271eaef6bc22e7fd86c476374fd4f46acb745e66c2fc87f8be',
  'verification/scase06/acceptance.json':'5fed58285741886c1c019dc07878e7b4c8d138d9110d240b78a0c25de7d299d8',
  'verification/scase06/corrected-acceptance.json':'304fad0aced504a9f241c2f0f05be4dc5d221e8e88caa97c36516079f6b428f9',
  'verification/scase06/final-acceptance.json':'6943c1a03163ebe5c7e98c1cdc4187cfa02a8c7007c57702e7f241d5679ab418',
  'verification/scase07/acceptance.json':'6bb9ee9bf3d9ed1077818426d29a1bb9895fe1733e1d8bbb3041e3d7fa2013d5',
  'verification/scase16/acceptance.json':'7534cfc6501b223e80e04ed0042fdf21fead5ffe246e4528b1358e8d605457d5',
  'verification/scase02-current-source/acceptance.json':'dd36ff5c04808c16461ab34a1c905a73e942e73a63e889fa1ee9647bbca72046',
  'verification/scase02-current-source/dx06-acceptance.json':'d092350b77ae605dc182e1b46d60cf5dd3f2737e447b27b3fccd99f0c8bbfb0f',
  'verification/scase04-current-source/acceptance.json':'850f455e59f41be52e2ae1b7996473825af80d00653517c720b0d4c0aafcd2c3',
  'verification/scase04-current-source/dx06-acceptance.json':'cc8abef6995e4ec74dac74bab46789d55014199a8f4afa20c938946284928146',
  'verification/scase06-current-source/acceptance.json':'aeaaba6afa2c2a7f4e18ea7704b4fc46b2c6ba029f42eb4779f1d71c15686da1',
  'verification/scase06-current-source/dx06-acceptance.json':'8c8ea0587ad033ba433fdfcaaff0a7a1cefd75b8d9e1c4dc54193a77625e22ff',
  'verification/scase07-current-source/acceptance.json':'b20e176c5cbe4ca7af997771b58ad569e76baae783bb3c7a36f7fdef7292fbc1',
  'verification/scase07-current-source/dx06-acceptance.json':'3b9e726f1d9204655b72289228c3321d158de336381130b66ef8228a26af7509',
};
const historical = {};
const historicalGates = {};
for (const [id, count] of [
  ['S-CASE-02',2], ['S-CASE-04',1], ['S-CASE-06',3],
  ['S-CASE-07',1], ['S-CASE-16',1],
]) {
  const row = cases.find(item => item.id === id);
  assert.equal(row.evidence.length, count, `${id} historical evidence count`);
  for (const evidence of row.evidence) {
    assert.equal(evidence.result, 'passed', `${id} historical result`);
    const configuration = evidence.configuration;
    const resolve = relative => {
      const file = path.resolve(root, 'specs/conformance', relative);
      assert.ok(file.startsWith(`${root}/verification/`), `${id} receipt/gate outside verification`);
      assert.equal(fs.realpathSync(file), file, `${id} receipt/gate symlink`);
      return path.relative(root, file).split(path.sep).join('/');
    };
    const receipt = resolve(configuration.receipt);
    assert.equal(configuration.receipt_sha256, priorReceiptSha256[receipt],
      `${id} historical receipt identity: ${receipt}`);
    assert.equal(sha(read(receipt)), configuration.receipt_sha256,
      `${id} historical receipt changed: ${receipt}`);
    historical[receipt] = configuration.receipt_sha256;
    if (configuration.gate_sha256) {
      const gate = resolve(configuration.gate);
      assert.equal(sha(read(gate)), configuration.gate_sha256,
        `${id} historical gate changed: ${gate}`);
      historicalGates[gate] = configuration.gate_sha256;
    }
  }
}
for (const id of ['scase02','scase04','scase06','scase07']) {
  for (const name of ['acceptance.json','dx06-acceptance.json']) {
    const file = `verification/${id}-current-source/${name}`;
    const result = JSON.parse(read(file));
    assert.equal(result.result, 'passed', `${file} previously accepted`);
    assert.equal(sha(read(file)), priorReceiptSha256[file],
      `${file} historical receipt changed`);
    historical[file] = priorReceiptSha256[file];
  }
}
assert.deepEqual(Object.keys(historical).sort(), Object.keys(priorReceiptSha256).sort(),
  'complete pinned historical receipt set');

const sources = {};
function add(file) {
  const full = path.join(root, file);
  const stat = fs.lstatSync(full);
  assert.ok(stat.isFile() && !stat.isSymbolicLink(), `unbound source file ${file}`);
  sources[file] = sha(fs.readFileSync(full));
}
function tree(directory) {
  for (const entry of fs.readdirSync(path.join(root, directory), {withFileTypes:true})
    .sort((a,b) => a.name.localeCompare(b.name))) {
    const file = `${directory}/${entry.name}`;
    assert.equal(entry.isSymbolicLink(), false, `unbound source symlink ${file}`);
    if (entry.isDirectory()) tree(file);
    else if (entry.isFile()) add(file);
    else throw Error(`unbound source type ${file}`);
  }
}
for (const directory of [
  'crates/noble-kernel/src','crates/noble-contracts/src',
  'crates/noble-wasm/src','crates/noble-wasm/runtime','crates/noble-wasm/wit',
  'crates/noble-cli/src','verification/scase08',
  '.cairn/changes/safety-callback-returned-owner',
]) tree(directory);
for (const file of [
  'Cargo.toml','Cargo.lock','rust-toolchain.toml',
  'crates/noble-kernel/Cargo.toml','crates/noble-contracts/Cargo.toml',
  'crates/noble-wasm/Cargo.toml','crates/noble-cli/Cargo.toml',
  '.cairn/specs/safety/spec.md','.cairn/specs/resource-adapters/spec.md',
  'specs/SAFETY.md',caseFile,'policy/tool-selection.json',
]) add(file);
assert.equal(sha(caseBytes), sources[caseFile],
  'canonical case changed between design check and source inventory');
const tasksFile = '.cairn/changes/safety-callback-returned-owner/tasks.md';
const tasksBytes = read(tasksFile);
assert.equal(sha(tasksBytes), sources[tasksFile], 'change tasks changed during source inventory');
const sourceRevision = `sha256:${sha(JSON.stringify(Object.entries(sources).sort()))}`;

const rust = path.join(selected.tool_paths.quality_rust.output, 'bin');
const temp = path.join(output, 'tmp');
fs.mkdirSync(temp);
const environment = {...process.env,
  PATH:`${rust}:${selected.component_sync.linker_bin}:${selected.tool_paths.node.output}/bin:${process.env.PATH}`,
  TMPDIR:temp, RUSTC_WRAPPER:'', RUSTC_WORKSPACE_WRAPPER:'',
  RUSTFLAGS:'', CARGO_ENCODED_RUSTFLAGS:'',
  CARGO_TARGET_DIR:path.join(output, 'target'), NIX_CONFIG:'min-free = 0'};
const commands = [];
function run(label, executable, args, expectedStatus=0) {
  const response = spawnSync(executable, args, {cwd:root,env:environment,
    timeout:900_000,maxBuffer:16*1024*1024});
  const stdout = response.stdout ?? Buffer.alloc(0);
  const stderr = response.stderr ?? Buffer.alloc(0);
  const stem = `${String(commands.length).padStart(2,'0')}-${label}`;
  fs.writeFileSync(path.join(output, `${stem}.stdout`), stdout, {flag:'wx'});
  fs.writeFileSync(path.join(output, `${stem}.stderr`), stderr, {flag:'wx'});
  commands.push({label,executable,executable_sha256:sha(fs.readFileSync(executable)),args,
    status:response.status,signal:response.signal,error:response.error?.message??null,
    stdout:`${stem}.stdout`,stderr:`${stem}.stderr`,
    stdout_sha256:sha(stdout),stderr_sha256:sha(stderr)});
  assert.equal(response.error, undefined, `${label} launch`);
  assert.equal(response.signal, null, `${label} signal`);
  assert.equal(response.status, expectedStatus, `${label}: ${stderr.toString('utf8')}`);
  return {stdout:stdout.toString('utf8'),stderr:stderr.toString('utf8')};
}

run('production-build', path.join(rust, 'cargo'),
  ['build','-p','noble-cli','--locked','--offline','-j','4']);
const cli = path.join(environment.CARGO_TARGET_DIR, 'debug/noble');
const wit = 'crates/noble-wasm/wit/callback-owner.wit';
const world = 'bounded';
const exportSource = `check=${path.join(root, 'verification/scase08/guest.noble')}`;
const bindings = JSON.parse(run('typed-bindings', cli,
  ['component','bindings',wit,world]).stdout);
assert.equal(bindings.world, 'noble-test:callback-owner/bounded@1.0.0');
assert.deepEqual(bindings.imports.map(row => row.word), ['tokens.issue','tokens.consume']);
assert.deepEqual(bindings.exports.map(row => row.word), ['check']);
assert.equal(bindings.resources.length, 1);
assert.equal(bindings.resources[0].identity, 'noble-test:callback-owner/tokens@1.0.0#token');
assert.equal(bindings.resources[0].data, false);
assert.equal(bindings.resources[0].capture, false);
const compiledDirectory = path.join(output, 'component');
const compiled = JSON.parse(run('compile-guest', cli,
  ['component','compile',wit,world,compiledDirectory,exportSource]).stdout);
assert.equal(compiled.outcome, 'compiled');
assert.equal(compiled.independent_kernel_check, true);
assert.equal(compiled.component_emitted, true);
const component = path.join(compiledDirectory, 'component.wasm');
const componentBytes = fs.readFileSync(component);
const componentSha = sha(componentBytes);
run('validate-component', wasmTools, ['validate', component]);

const observations = [];
function invoke(mode, expected) {
  const report = JSON.parse(run(mode, cli,
    ['component','callback-owner',component,mode,wit,world,exportSource]).stdout);
  assert.equal(report.schema, 'noble-callback-owner/v1', `${mode}.schema`);
  assert.equal(report.stage, 'callback', `${mode}.stage`);
  assert.equal(report.released_owners, 1, `${mode}.released_owners`);
  assert.equal(report.fixture_releases, mode === 'authentic' ? 0 : 1,
    `${mode}.fixture_releases`);
  assert.equal(report.live_owners, 0, `${mode}.live_owners`);
  assert.equal(report.native_pins, 0, `${mode}.native_pins`);
  for (const [key,value] of Object.entries(expected))
    assert.deepEqual(report[key], value, `${mode}.${key}`);
  observations.push({mode,report});
  return report;
}
invoke('authentic', {
  outcome:'accepted',guest_value:42,callback_rejection:null,guest_requests:2,
  trusted_values_created:1,protected_operations:1,
});
for (const [mode, rejection] of [
  ['stale-generation','wrong-generation'],
  ['wrong-kind','wrong-kind'],
  ['wrong-context','wrong-context'],
]) {
  invoke(mode, {
    ...design.expected,guest_value:null,callback_rejection:rejection,
    guest_requests:1,protected_operations:0,
  });
}
assert.deepEqual(observations.slice(1).map(row => row.mode), design.input.mutations);

// A valid but byte-distinct Wasm component must be refused by independent
// host-selected WIT/world/export source correspondence before any guest call.
const spoof = path.join(output, 'spoof-custom-section.wasm');
const spoofBytes = Buffer.concat([componentBytes, Buffer.from([0x00,0x01,0x00])]);
assert.notEqual(sha(spoofBytes), componentSha);
fs.writeFileSync(spoof, spoofBytes, {flag:'wx'});
run('validate-spoof', wasmTools, ['validate', spoof]);
const refusal = run('spoof-pre-guest-refusal', cli,
  ['component','callback-owner',spoof,'authentic',wit,world,exportSource], 2);
assert.equal(refusal.stdout, '', 'spoof must not produce a guest report');
assert.match(refusal.stderr,
  /component differs from independent host-selected Noble source recipe/);
const controls = run('retained-table-and-cross-store-controls', path.join(rust, 'cargo'),
  ['test','-p','noble-cli','--locked','--offline','--bin','noble','-j','4',
    'component::callback_owner::tests','--','--test-threads','1']).stdout;
assert.match(controls, /retained_real_owners_are_refused_at_callback_ingress \.\.\. ok/);
assert.match(controls, /callback_preflight_does_not_accept_missing_owner_right \.\.\. ok/);
assert.match(controls, /live_stores_with_equal_slots_and_generations_never_share_identity \.\.\. ok/);
assert.match(controls, /3 passed; 0 failed/);
assert.equal(sha(fs.readFileSync(component)), componentSha, 'component changed during invocations');
assert.equal(sha(fs.readFileSync(path.join(output, 'prepromotion-safety-cases.json'))),
  sources[caseFile], 'external canonical snapshot changed');
for (const [file,digest] of Object.entries(sources))
  assert.equal(sha(read(file)), digest, `source changed during gate: ${file}`);
for (const [file,digest] of Object.entries(historical))
  assert.equal(sha(read(file)), digest, `historical receipt changed during gate: ${file}`);
for (const [file,digest] of Object.entries(historicalGates))
  assert.equal(sha(read(file)), digest, `historical gate changed during gate: ${file}`);
for (const command of commands) {
  assert.equal(sha(fs.readFileSync(command.executable)), command.executable_sha256,
    `executable changed: ${command.label}`);
  for (const channel of ['stdout','stderr'])
    assert.equal(sha(fs.readFileSync(path.join(output,command[channel]))),
      command[`${channel}_sha256`], `raw output changed: ${command.label}.${channel}`);
}
const receipt = {
  schema:'noble-scase08-callback-owner/v1',kind:'test',result:'passed',
  failures:[],integrity_failures:[],
  case:{id:design.id,input:design.input,expected:design.expected},
  source_revision:sourceRevision,source_sha256:sources,
  prepromotion_case_sha256:sources[caseFile],
  prepromotion_tasks_sha256:sources[tasksFile],
  historical_receipt_sha256:historical,historical_gate_sha256:historicalGates,
  selected_rust:selected.tool_paths.quality_rust,selected_node:selected.tool_paths.node,
  selected_wasm_tools:selected.tool_paths.wasm_tools,
  selected_wasmtime:selected.component_sync.wasmtime_version,
  guest_source_sha256:sources['verification/scase08/guest.noble'],
  component_sha256:componentSha,spoof_component_sha256:sha(spoofBytes),
  binary_sha256:sha(fs.readFileSync(cli)),commands,observations,
  spoof_refusal:{status:2,stdout_sha256:sha(refusal.stdout),stderr_sha256:sha(refusal.stderr)},
  claim:'The exact compiled Noble guest invokes host-owned issue and consume callbacks. The production host accepts one authentic result and rejects each canonical stale-generation, wrong-kind and wrong-context callback result before constructing a trusted value or performing the protected operation. An independently validated byte-distinct component is refused before guest execution by the host-selected WIT/world/source recipe.',
  assumptions:[
    'The host, not the guest, selects each callback mutation and retains the owner, token table, generation, kind, context and independently selected source recipe.',
    'Selected Rust, Noble compiler, Wasmtime, wasm-tools and Canonical ABI are trusted for these finite results. Universal handle non-forgeability, native memory safety and backend refinement remain open.',
    'Earlier case and current-source acceptance receipts remain immutable evidence for their original source revisions, not automatic proof for this source revision.',
  ],
};
const receiptFile = path.join(output, 'acceptance.json');
fs.writeFileSync(receiptFile, `${JSON.stringify(receipt,null,2)}\n`, {flag:'wx'});
console.log(JSON.stringify({result:'passed',receipt:receiptFile,receipt_sha256:sha(fs.readFileSync(receiptFile)),
  source_revision:sourceRevision,component_sha256:componentSha,cases:observations.length,
  commands:commands.length}));
