#!/usr/bin/env node
// Read-only repository inputs, fresh external artifacts, actual compiled Noble
// components and independent dynamic Wasmtime Component Model Val conversion.
// A passing run is bounded execution evidence, never a compiler/engine proof.
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const root = fs.realpathSync(path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..'));
const args = process.argv.slice(2);
assert.equal(args.length, 4, 'usage: NODE verification/m7/gate.mjs CLI PEER WASM_TOOLS NEW_EXTERNAL_ARTIFACT_DIR');
const artifacts = path.resolve(args[3]);
assert.ok(artifacts !== root && !artifacts.startsWith(`${root}${path.sep}`), 'artifacts must be outside the repository');
assert.ok(!fs.existsSync(artifacts), 'acceptance directory must be fresh');
assert.equal(fs.realpathSync(path.dirname(artifacts)), path.dirname(artifacts), 'artifact parent must not be a symlink');
fs.mkdirSync(artifacts);
const sha256 = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const relative = file => path.relative(artifacts, file).split(path.sep).join('/');
const receipt = {
  schema: 'noble-m7-acceptance/v1', profile: 'Syndicate-Sync-Service-M7', result: 'running',
  source_root: root, artifact_directory: artifacts,
  engine: { executable: process.execPath, version: process.version },
  sources: {}, executables: {}, inputs: {}, retained: {}, commands: [], components: {}, cases: [], controls: [],
  integrity_failures: [],
  assumptions: [
    'The supplied CLI/peer/wasm-tools binaries and Node/Wasmtime/OS are trusted execution tooling; their captured hashes do not prove correspondence to source.',
    'The independent Rust peer performs dynamic Wasmtime Component Model linking and typed Val conversion while reusing Noble production dataspace and Preserves decisions.',
    'Host-granted facet rights, serialized access, actual Store isolation, trap authenticity and physical cleanup remain separate host/engine assumptions.',
    'This selected synchronous service proves neither full Syndicate/Preserves/WASI nor native async, fairness, distributed delivery, or universal source-to-Wasm refinement.',
  ],
};
let serial = 0;
const monitored = [];
const hashFile = file => sha256(fs.readFileSync(file));
function retain(name, bytes, mode = 0o400) {
  const file = path.join(artifacts, name);
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, bytes, { flag: 'wx', mode });
  const actual = fs.readFileSync(file);
  receipt.retained[name] = { sha256: sha256(actual), bytes: actual.length };
  monitored.push({ file, sha256: receipt.retained[name].sha256 });
  return file;
}
function freeze(file, name, executable = false) {
  const original = fs.realpathSync(path.resolve(file));
  assert.ok(fs.statSync(original).isFile(), `not a file: ${original}`);
  const bytes = fs.readFileSync(original);
  const frozen = retain(name, bytes, executable ? 0o500 : 0o400);
  monitored.push({ file: original, sha256: sha256(bytes) });
  return { original, frozen, sha256: sha256(bytes), bytes: bytes.length };
}
function source(file) {
  const value = freeze(path.join(root, file), `sources/${file}`);
  receipt.sources[file] = { sha256: value.sha256, snapshot: relative(value.frozen), bytes: value.bytes };
  return value.frozen;
}
function sourceTree(directory) {
  for (const item of fs.readdirSync(path.join(root, directory), { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name))) {
    if (['target', '.git', 'node_modules'].includes(item.name)) continue;
    const file = `${directory}/${item.name}`;
    if (item.isDirectory()) sourceTree(file);
    else if (item.isFile()) source(file);
    else throw Error(`unreviewed source entry: ${file}`);
  }
}
function command(label, executable, argv, timeout = 180_000) {
  const id = ++serial;
  const stem = `commands/${String(id).padStart(4, '0')}-${label.replace(/[^A-Za-z0-9_.-]/g, '-')}`;
  const result = spawnSync(executable, argv, {
    cwd: root, encoding: 'utf8', timeout, killSignal: 'SIGKILL', maxBuffer: 32 * 1024 * 1024,
    env: { PATH: '/nonexistent', LANG: 'C', LC_ALL: 'C', RUST_BACKTRACE: '1' },
  });
  const stdout = retain(`${stem}.stdout`, result.stdout ?? '');
  const stderr = retain(`${stem}.stderr`, result.stderr ?? '');
  const entry = { id, label, executable: relative(executable), executable_sha256: hashFile(executable),
    arguments: argv, cwd: root, timeout_ms: timeout, stdout: relative(stdout), stderr: relative(stderr),
    status: result.status, signal: result.signal, error: result.error ? String(result.error) : null };
  receipt.commands.push(entry);
  assert.equal(entry.error, null, `command ${id} host failure`);
  assert.equal(entry.signal, null, `command ${id} terminated by signal`);
  assert.equal(entry.status, 0, `command ${id} failed; retained stderr: ${entry.stderr}`);
  return { id, stdout: result.stdout ?? '', stderr: result.stderr ?? '' };
}
function json(run) {
  const lines = run.stdout.split('\n').filter(line => line.trim());
  assert.equal(lines.length, 1, `command ${run.id} must print exactly one JSON object`);
  return JSON.parse(lines[0]);
}
function record(row, run, detail) { row.evidence.push({ command: run.id, ...(detail === undefined ? {} : { detail }) }); }
const selected = {
  'specs/conformance/safety-cases.json': ['S-CASE-09', 'S-CASE-10', 'S-CASE-11', 'S-CASE-12', 'S-CASE-17'],
  'specs/conformance/wit-wasi-cases.json': ['WI-14', 'WI-18'],
};
const cases = new Map();
function bindCases() {
  for (const [file, ids] of Object.entries(selected)) {
    const bytes = fs.readFileSync(path.join(artifacts, `sources/${file}`));
    const document = JSON.parse(bytes);
    assert.equal(document.revision, '0.1.0-draft.5');
    assert.equal(new Set(document.cases.map(row => row.id)).size, document.cases.length);
    const workload = ids.map(id => {
      const item = document.cases.find(row => row.id === id);
      assert.ok(item, `missing declared case ${id}`);
      cases.set(id, item);
      return { id, input: item.input, expected: item.expected };
    });
    receipt.inputs[file] = { sha256: sha256(bytes), workload_sha256: sha256(Buffer.from(JSON.stringify(workload))),
      workload, interpretation: 'complete selected id/input/expected projection; metadata does not execute cases' };
  }
}
const exportNames = ['observer', 'publish-and-trap', 'publisher', 'withdraw'];
const publisherBodies = {
  publisher: 'dataspace.publish', observer: 'dataspace.observe', withdraw: 'dataspace.retract',
  'publish-and-trap': 'dataspace.publish dataspace.fail',
};
const subscriberBodies = { ...publisherBodies, withdraw: 'dataspace.retract drop false' };
function compile(label, cli, wasmTools, witPath, wit, bodies) {
  const exports = Object.entries(bodies).map(([name, body]) => ({ name, body, file: retain(`inputs/${label}-${name}.noble`, body) }));
  const directory = path.join(artifacts, 'components', label);
  fs.mkdirSync(path.dirname(directory), { recursive: true });
  const run = command(`${label}-compile`, cli, ['component', 'compile', witPath, 'service', directory,
    ...exports.map(item => `${item.name}=${item.file}`)]);
  const report = json(run);
  assert.equal(report.schema, 'noble-component/v1');
  assert.equal(report.outcome, 'compiled');
  assert.equal(report.component_emitted, true);
  assert.equal(report.independent_kernel_check, true);
  assert.equal(report.abi, 'wasm-tools-1.245.1-sync-memory32-utf8');
  assert.deepEqual(fs.readdirSync(directory).sort(), ['component.wasm', 'core.wasm', 'module.wat', 'report.json', 'world.wit',
    ...exports.map((_, index) => `export-${index}.noble`)].sort());
  assert.deepEqual(fs.readFileSync(path.join(directory, 'world.wit')), wit);
  assert.deepEqual(JSON.parse(fs.readFileSync(path.join(directory, 'report.json'))), report);
  exports.forEach((item, index) => assert.deepEqual(fs.readFileSync(path.join(directory, `export-${index}.noble`)), Buffer.from(item.body)));
  for (const name of fs.readdirSync(directory)) {
    const file = path.join(directory, name);
    assert.ok(fs.lstatSync(file).isFile());
    const bytes = fs.readFileSync(file);
    fs.chmodSync(file, 0o400);
    receipt.retained[relative(file)] = { sha256: sha256(bytes), bytes: bytes.length };
    monitored.push({ file, sha256: sha256(bytes) });
  }
  const component = path.join(directory, 'component.wasm');
  const validated = command(`${label}-wasm-validate`, wasmTools, ['validate', component]);
  const inspected = command(`${label}-wit-inspect`, wasmTools, ['component', 'wit', component]);
  assert.match(inspected.stdout, /noble:syndicate(?:\/dataspace)?@1\.0\.0/);
  const found = [...inspected.stdout.matchAll(/^\s*export\s+([^\s:]+)\s*:/gm)].map(match => match[1]).sort();
  assert.deepEqual(found, exportNames, 'independently inspected component exports must match selected world');
  const effects = report.resolved_imports ?? report.imports ?? null;
  receipt.components[label] = { artifact: relative(component), sha256: hashFile(component), compile: run.id,
    validation: validated.id, inspection: inspected.id, source_exports: exports.map(item => ({ name: item.name,
      sha256: hashFile(item.file) })), resolved_imports: effects };
  return component;
}
function caseRow(id, action) {
  const item = cases.get(id);
  assert.ok(item, `missing canonical case ${id}`);
  const row = { id, input: item.input, expected: item.expected, result: 'running', evidence: [], variants: [] };
  receipt.cases.push(row);
  try { action(row, item); assert.ok(row.evidence.length || row.variants.length, `${id} had no observations`); row.result = 'passed'; }
  catch (error) { row.result = 'failed'; row.failure = String(error.stack ?? error); }
  return row;
}
function variants(row, items, declared, field, action) {
  assert.ok(Array.isArray(items), `${row.id}.${field} not independently reported`);
  assert.deepEqual(items.map(item => item.name).sort(), declared.slice().sort(), `${row.id}: missing, duplicate or invented ${field} variants`);
  for (const name of declared) {
    const observed = items.find(item => item.name === name);
    const variant = { name, observed, result: 'running' };
    row.variants.push(variant);
    try { action(observed, name); variant.result = 'passed'; }
    catch (error) { variant.result = 'failed'; throw error; }
  }
}
function checkedRejection(observation, { typed = false, assertions = false, rights = false } = {}) {
  assert.equal(observation.outcome, 'reject');
  if (typed) assert.equal(observation.typed_protocol_values_created, 0);
  if (assertions) assert.equal(observation.assertions_published, 0);
  if (rights) assert.equal(observation.facet_rights_created, 0);
}
function verifyPeer(report, run, publisher, subscriber) {
  assert.equal(report.schema, 'noble-m7-peer/v1');
  assert.equal(report.outcome, 'passed');
  assert.equal(report.engine, 'wasmtime-40.0.2');
  assert.deepEqual(report.components, { publisher_sha256: hashFile(publisher), subscriber_sha256: hashFile(subscriber),
    distinct: true, threads_enabled: false, shared_memory_enabled: false });
  assert.equal(report.compiled.separate_stores, true);
  assert.equal(report.compiled.trap_observed, true);
  assert.equal(report.compiled.calls?.filter(item => item.role === 'publisher').length, 5,
    'all five publisher/withdraw/trap calls must execute on the compiled publisher');
  assert.equal(report.compiled.calls?.filter(item => item.role === 'subscriber').length, 8,
    'all eight membership queries must execute on the compiled subscriber');
  assert.equal(report.compiled.published_before_trap, true);
  assert.equal(report.compiled.trap_import_started, true);
  assert.ok(report.controls && typeof report.controls === 'object');
  assert.ok(Array.isArray(report.compiled.events));
  assert.deepEqual(report.compiled.events.map(({ name, ready, kind }) => ({ name, ready, kind })),
    cases.get('S-CASE-17').expected.typed_observer_events);
  assert.ok(report.compiled.events.every(event => typeof event.observer === 'string' && event.observer.length > 0));
  assert.equal(new Set(report.compiled.events.map(event => event.observer)).size, 1,
    'all six add/remove notifications must target the same surviving subscriber facet');
  const declaredSteps = cases.get('S-CASE-17').input.steps.map(item => ({
    role: item.call === 'observer' ? 'subscriber' : 'publisher', ...item,
  }));
  assert.deepEqual(report.compiled.calls, declaredSteps,
    'every declared call/argument/result/trap must come from the matching compiled participant');
  const operations = cases.get('WI-18').input.imports;
  assert.deepEqual(report.compiled.publisher_requests, [
    operations[0], operations[2], operations[0], operations[2], operations[0], operations[3],
  ], 'exact versioned import effects, including the trap import, must match actual compiled publisher calls');
  assert.deepEqual(report.compiled.subscriber_requests, Array(8).fill(operations[1]),
    'all subscriber effects must be exact versioned observation requests');
  assert.deepEqual(report.compiled.remaining, { facets: 0, assertions: 0, interests: 0 });
  assert.deepEqual(report.remaining, { facets: 0, assertions: 0, interests: 0 });
  receipt.peer = { command: run.id, schema: report.schema, engine: report.engine, components: report.components };
}
try {
  for (const file of ['verification/m7/gate.mjs', 'crates/noble-wasm/wit/syndicate.wit',
    'crates/noble-syndicate/src/lib.rs',
    '.cairn/specs/safety/spec.md', '.cairn/specs/wit-wasi/spec.md',
    ...Object.keys(selected), 'verification/m7/peer/Cargo.toml']) source(file);
  sourceTree('crates/noble-kernel/src/dataspace');
  sourceTree('verification/m7/peer/src');
  bindCases();
  for (const [name, supplied] of [['cli', args[0]], ['peer', args[1]], ['wasm_tools', args[2]], ['node', process.execPath]]) {
    receipt.executables[name] = freeze(supplied, `executables/${name}`, true);
  }
  const cli = receipt.executables.cli.frozen;
  const peer = receipt.executables.peer.frozen;
  const wasmTools = receipt.executables.wasm_tools.frozen;
  const wit = fs.readFileSync(path.join(artifacts, 'sources/crates/noble-wasm/wit/syndicate.wit'));
  const witPath = retain('inputs/selected-service.wit', wit);
  const publisher = compile('publisher', cli, wasmTools, witPath, wit, publisherBodies);
  const subscriber = compile('subscriber', cli, wasmTools, witPath, wit, subscriberBodies);
  assert.notEqual(hashFile(publisher), hashFile(subscriber), 'two independently compiled participant artifacts must be semantically distinct');
  const sharedBytes = Buffer.from(cases.get('S-CASE-11').input.shared_memory_core_wasm_hex, 'hex');
  assert.equal(sharedBytes.toString('hex'), cases.get('S-CASE-11').input.shared_memory_core_wasm_hex,
    'selected shared-memory module must be valid hex');
  const sharedMemory = retain('inputs/shared-memory-core.wasm', sharedBytes);
  const sharedValidation = command('shared-memory-wasm-validate', wasmTools, ['validate', sharedMemory]);
  receipt.controls.push({ name: 'actual-shared-memory-module-validated', command: sharedValidation.id,
    input: relative(sharedMemory), sha256: hashFile(sharedMemory) });
  const run = command('independent-wasmtime-peer', peer, [publisher, subscriber, 'scenario', sharedMemory]);
  const report = json(run);
  verifyPeer(report, run, publisher, subscriber);
  caseRow('S-CASE-09', row => {
    variants(row, report.controls.bounds, row.input.mutations, 'bounds', observation => checkedRejection(observation, { typed: true, rights: true }));
    record(row, run, 'production bounded Preserves decoder called for every declared malformed input');
  });
  caseRow('S-CASE-10', row => {
    variants(row, report.controls.schema, ['ready-not-bool', ...row.input.additional_mutations], 'schema',
      observation => checkedRejection(observation, { typed: true, assertions: true }));
    record(row, run, 'production schema rejection before assertion publication');
  });
  caseRow('S-CASE-11', row => {
    const observation = report.controls.admission;
    assert.equal(observation.input_sha256, hashFile(sharedMemory));
    assert.equal(observation.inspected_shared_memory, row.expected.inspected_shared_memory);
    assert.equal(observation.loader_rejected, row.expected.selected_engine_loader_rejected,
      'selected engine must refuse the actual shared-memory Wasm');
    assert.equal(observation.selected_engine_threads_enabled, false);
    assert.equal(observation.selected_engine_shared_memory_enabled, false);
    assert.ok(typeof observation.loader_error === 'string' && observation.loader_error.length > 0);
    assert.equal(observation.requested_shared_mutable_noble_memory, row.input.requested_shared_mutable_noble_memory);
    assert.equal(observation.stage, row.expected.stage);
    assert.equal(observation.outcome, row.expected.outcome);
    assert.equal(observation.facet_rights_created, row.expected.facet_rights_created);
    assert.equal(observation.guest_requests, row.expected.guest_requests);
    assert.equal(observation.protected_operations, row.expected.protected_operations);
    assert.deepEqual(observation.counts_after, observation.counts_before,
      'shared-memory refusal cannot grant a facet or mutate accepted dataspace state');
    record(row, run, 'gate-supplied shared Wasm validated, memory section inspected by peer, production profile refused before facet/guest work');
  });
  caseRow('S-CASE-12', row => {
    const observed = report.controls.cleanup;
    assert.equal(observed.owned_assertions_before, row.input.owned_assertions);
    assert.equal(observed.owned_interests_before, row.input.owned_interests);
    assert.equal(observed.child_scopes_before, row.input.child_scopes);
    assert.equal(observed.actor_trap, true);
    assert.equal(observed.compiled_publish_and_trap, true);
    assert.equal(observed.surviving_observer_removal, true);
    assert.equal(observed.outcome, row.expected.outcome);
    for (const key of ['remaining_scope_assertions', 'remaining_scope_interests', 'live_child_scopes', 'duplicate_retractions']) {
      assert.equal(observed[key], row.expected[key]);
    }
    record(row, run, 'production recursive retirement with child and interest, distinct from compiled trap path');
  });
  caseRow('S-CASE-17', row => {
    assert.equal(report.compiled.subscriber_still_active_before_close, row.expected.subscriber_still_active);
    assert.equal(report.compiled.publisher_trap_reported_success, row.expected.publisher_trap_reported_success);
    assert.equal(report.controls.admission.inspected_shared_memory, true);
    assert.equal(report.controls.capability.attempted_publish_without_right, 'denied');
    assert.equal(row.expected.host_bound_facet_rights, true);
    assert.equal(row.expected.remaining_publisher_assertions, 0);
    assert.equal(row.input.steps.length, 13);
    record(row, run, 'compiled two-participant exact membership and typed add/remove/trap observations');
  });
  caseRow('WI-14', row => {
    const observed = report.controls.capability;
    assert.equal(observed.outcome, row.expected.outcome);
    assert.deepEqual(observed.payload, row.input.payload);
    for (const key of ['valid_service_remains_data_only', 'invalid_extension_decoded',
      'live_resources_created', 'facet_rights_created', 'protected_operations']) assert.equal(observed[key], row.expected[key]);
    assert.equal(observed.attempted_publish_without_right, 'denied');
    record(row, run, 'actual production adapter refuses wire-to-resource and wire-to-facet authority');
  });
  caseRow('WI-18', row => {
    assert.deepEqual(report.compiled.roundtrip.map(item => ({ name: item.name, ready: item.ready, canonical_text: item.canonical_text })),
      row.input.service_values);
    assert.deepEqual(report.compiled.roundtrip, report.controls.roundtrip);
    assert.deepEqual(report.compiled.roundtrip.map(item => item.request_ordinal), [1, 3]);
    for (const item of report.compiled.roundtrip) {
      assert.equal(item.outcome, 'roundtrip');
      assert.equal(item.compiled_import, true);
      assert.equal(item.operation, row.input.imports[0]);
    }
    variants(row, report.controls.boundary, row.input.malformed_or_forged_controls, 'boundary',
      observation => {
        assert.equal(observation.outcome, 'reject');
        assert.equal(observation.guest_requests, 0);
        assert.equal(observation.protected_operations, 0);
        assert.equal(observation.facet_rights_created, 0);
        assert.equal(observation.stage, ['wrong-wit-version', 'wrong-signature'].includes(observation.name)
          ? 'link' : { 'invalid-preserves-tag': 'decode', 'invalid-preserves-schema': 'schema',
            'resource-shaped-payload': 'protocol-adapter' }[observation.name]);
        assert.ok(typeof observation.error === 'string' && observation.error.length > 0);
        if (observation.stage !== 'link') assert.equal(observation.typed_protocol_values_created, 0);
      });
    record(row, run, 'independently validated WIT shape, actual Noble component calls, production Preserves roundtrip and hostile boundary probes');
  });
  const transition = report.controls.transition;
  assert.equal(transition.outcome, 'replace-atomic');
  assert.deepEqual(transition.events, [
    { name: 'clock', ready: true, kind: 'add' },
    { name: 'clock', ready: true, kind: 'remove' },
    { name: 'clock', ready: false, kind: 'add' },
  ]);
  assert.equal(transition.idempotent_replay_events, 0);
  assert.deepEqual(transition.remaining, [0, 0, 0]);
  receipt.controls.push({ name: 'production-replacement-and-idempotence', command: run.id,
    scope: 'local production decisions, separate from compiled two-participant execution', observed: transition });
} catch (error) {
  receipt.failure = String(error.stack ?? error);
} finally {
  for (const binding of monitored) {
    try { assert.equal(hashFile(binding.file), binding.sha256); }
    catch (error) { receipt.integrity_failures.push({ file: binding.file, failure: String(error) }); }
  }
  const expected = Object.values(selected).flat().sort();
  const actual = receipt.cases.map(row => row.id).sort();
  receipt.summary = { required_cases: expected.length, executed_cases: actual.length,
    passed_cases: receipt.cases.filter(row => row.result === 'passed').length,
    failed_cases: receipt.cases.filter(row => row.result === 'failed').map(row => row.id),
    missing_cases: expected.filter(id => !actual.includes(id)),
    variant_count: receipt.cases.reduce((sum, row) => sum + row.variants.length, 0),
    commands: receipt.commands.length, integrity_failures: receipt.integrity_failures.length };
  receipt.result = !receipt.failure && !receipt.integrity_failures.length
    && JSON.stringify(expected) === JSON.stringify(actual)
    && receipt.cases.every(row => row.result === 'passed' && row.variants.every(item => item.result === 'passed'))
    ? 'passed' : 'failed';
  fs.writeFileSync(path.join(artifacts, 'report.json'), JSON.stringify(receipt, null, 2) + '\n', { flag: 'wx', mode: 0o400 });
  console.log(JSON.stringify({ schema: receipt.schema, result: receipt.result, report: path.join(artifacts, 'report.json'), summary: receipt.summary }));
  process.exitCode = receipt.result === 'passed' ? 0 : 1;
}
