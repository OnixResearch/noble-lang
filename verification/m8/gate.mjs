#!/usr/bin/env node
// Source-frozen M8 conformance: independently compile two Noble components,
// link through the separately built typed Wasmtime peer, and check complete
// canonical positive/hostile workloads against production monitor observations.
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const root = fs.realpathSync(path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..'));
const args = process.argv.slice(2);
assert.equal(args.length, 4, 'usage: SELECTED_NODE verification/m8/gate.mjs CLI M8_PEER WASM_TOOLS NEW_EXTERNAL_DIRECTORY');
const artifacts = path.resolve(args[3]);
assert.ok(artifacts !== root && !artifacts.startsWith(`${root}${path.sep}`), 'artifacts outside repository required');
assert.ok(!fs.existsSync(artifacts), 'fresh acceptance directory required');
assert.equal(fs.realpathSync(path.dirname(artifacts)), path.dirname(artifacts), 'artifact parent cannot be a symlink');
fs.mkdirSync(artifacts);
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const hash = file => sha(fs.readFileSync(file));
const relative = file => path.relative(artifacts, file).split(path.sep).join('/');
const selected = {
  'specs/conformance/safety-cases.json': ['S-CASE-18', 'S-CASE-19'],
  'specs/conformance/wit-wasi-cases.json': ['WI-19', 'WI-20'],
};
const receipt = { schema: 'noble-m8-acceptance/v1', profile: 'Choreography-Service-M8',
  result: 'running', source_root: root, artifact_directory: artifacts, sources: {}, executables: {},
  inputs: {}, retained: {}, commands: [], components: {}, cases: [], integrity_failures: [],
  assumptions: [
    'The supplied binaries, pinned Wasmtime/Node/wasm-tools and OS are trusted execution tools; recorded hashes are provenance, not semantics.',
    'The separately compiled Noble components are linked by an independent typed peer; every M8 import is checked through production choreography policy and then M7 host dataspace policy.',
    'Authentic host-assigned role/facet bindings, actual Store isolation, trap authenticity and physical cleanup remain host/engine assumptions.',
    'This selected local synchronous profile does not imply general choreography, fairness, async scheduling, transport, durability, or universal source-to-Wasm refinement.',
  ] };
const watched = [];
const cases = new Map();
let serial = 0;
function retain(name, bytes, mode = 0o400) {
  const file = path.join(artifacts, name);
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, bytes, { flag: 'wx', mode });
  receipt.retained[name] = { sha256: hash(file), bytes: fs.statSync(file).size };
  watched.push({ file, sha256: hash(file) });
  return file;
}
function source(file) {
  const original = path.join(root, file);
  assert.ok(fs.lstatSync(original).isFile(), `missing plain source ${file}`);
  const bytes = fs.readFileSync(original);
  const frozen = retain(`sources/${file}`, bytes);
  receipt.sources[file] = { sha256: sha(bytes), bytes: bytes.length, snapshot: relative(frozen) };
  watched.push({ file: original, sha256: sha(bytes) });
  return frozen;
}
function tree(directory) {
  for (const item of fs.readdirSync(path.join(root, directory), { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name))) {
    const file = `${directory}/${item.name}`;
    if (item.isDirectory()) tree(file);
    else if (item.isFile()) source(file);
    else throw Error(`unreviewed source entry: ${file}`);
  }
}
function freeze(file, name, executable = false) {
  const original = fs.realpathSync(path.resolve(file));
  assert.ok(fs.statSync(original).isFile(), `not a file: ${original}`);
  const bytes = fs.readFileSync(original);
  const frozen = retain(name, bytes, executable ? 0o500 : 0o400);
  watched.push({ file: original, sha256: sha(bytes) });
  return { original, frozen, sha256: sha(bytes), bytes: bytes.length };
}
function command(label, executable, argv, timeout = 180_000) {
  const id = ++serial;
  const stem = `commands/${String(id).padStart(4, '0')}-${label.replace(/[^A-Za-z0-9_.-]/g, '-')}`;
  const result = spawnSync(executable, argv, { cwd: root, encoding: 'utf8', timeout,
    killSignal: 'SIGKILL', maxBuffer: 32 * 1024 * 1024,
    env: { PATH: '/nonexistent', LANG: 'C', LC_ALL: 'C', RUST_BACKTRACE: '1' } });
  const stdout = retain(`${stem}.stdout`, result.stdout ?? '');
  const stderr = retain(`${stem}.stderr`, result.stderr ?? '');
  const entry = { id, label, executable: relative(executable), executable_sha256: hash(executable),
    arguments: argv, cwd: root, timeout_ms: timeout, stdout: relative(stdout), stderr: relative(stderr),
    status: result.status, signal: result.signal, error: result.error ? String(result.error) : null };
  receipt.commands.push(entry);
  assert.equal(entry.error, null, `${label}: launch failure`);
  assert.equal(entry.signal, null, `${label}: command terminated`);
  assert.equal(entry.status, 0, `${label}: command failed, see ${entry.stderr}`);
  return { id, stdout: result.stdout ?? '', stderr: result.stderr ?? '' };
}
function json(run) {
  const lines = run.stdout.split('\n').filter(line => line.trim());
  assert.equal(lines.length, 1, `command ${run.id} must emit one JSON object`);
  return JSON.parse(lines[0]);
}
function bindCases() {
  for (const [file, ids] of Object.entries(selected)) {
    const bytes = fs.readFileSync(path.join(artifacts, `sources/${file}`));
    const document = JSON.parse(bytes);
    assert.equal(new Set(document.cases.map(row => row.id)).size, document.cases.length);
    const workload = ids.map(id => {
      const item = document.cases.find(row => row.id === id);
      assert.ok(item && item.profile === receipt.profile, `missing selected case: ${id}`);
      assert.equal(item.state?.execution, 'not-run', 'M8 case cannot be promoted before fresh gate');
      cases.set(id, item);
      return { id, input: item.input, expected: item.expected };
    });
    receipt.inputs[file] = { sha256: sha(bytes), workload_sha256: sha(Buffer.from(JSON.stringify(workload))),
      workload, interpretation: 'complete exact canonical selected case inputs/expectations; metadata is not execution' };
  }
}
const exportNames = ['observer', 'publish-and-trap', 'publisher', 'withdraw'];
const publisherBodies = { publisher: 'dataspace.publish', observer: 'dataspace.observe',
  withdraw: 'dataspace.retract', 'publish-and-trap': 'dataspace.publish dataspace.fail' };
const subscriberBodies = { ...publisherBodies, withdraw: 'dataspace.retract drop false' };
function compile(label, cli, wasmTools, witPath, wit, bodies, version = '1.0.0') {
  const exports = Object.entries(bodies).map(([name, body]) => ({ name, body,
    file: retain(`inputs/${label}-${name}.noble`, body) }));
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
  assert.deepEqual(fs.readdirSync(directory).sort(), ['component.wasm', 'core.wasm', 'module.wat',
    'report.json', 'world.wit', ...exports.map((_, index) => `export-${index}.noble`)].sort());
  assert.deepEqual(fs.readFileSync(path.join(directory, 'world.wit')), wit);
  assert.deepEqual(JSON.parse(fs.readFileSync(path.join(directory, 'report.json'))), report);
  exports.forEach((item, index) => assert.deepEqual(fs.readFileSync(path.join(directory, `export-${index}.noble`)), Buffer.from(item.body)));
  for (const name of fs.readdirSync(directory)) {
    const file = path.join(directory, name);
    assert.ok(fs.lstatSync(file).isFile());
    const bytes = fs.readFileSync(file);
    fs.chmodSync(file, 0o400);
    receipt.retained[relative(file)] = { sha256: sha(bytes), bytes: bytes.length };
    watched.push({ file, sha256: sha(bytes) });
  }
  const component = path.join(directory, 'component.wasm');
  const validated = command(`${label}-wasm-validate`, wasmTools, ['validate', component]);
  const inspected = command(`${label}-wit-inspect`, wasmTools, ['component', 'wit', component]);
  assert.ok(inspected.stdout.split('\n').some(line =>
    line.trim() === `import noble:syndicate/dataspace@${version};`),
  `${label}: actual component import identity differs from submitted WIT version`);
  const found = [...inspected.stdout.matchAll(/^\s*export\s+([^\s:]+)\s*:/gm)].map(match => match[1]).sort();
  assert.deepEqual(found, exportNames, 'independently inspected exports differ from unchanged M7 WIT');
  receipt.components[label] = { artifact: relative(component), sha256: hash(component),
    compile: run.id, validation: validated.id, inspection: inspected.id,
    source_exports: exports.map(item => ({ name: item.name, sha256: hash(item.file) })),
    resolved_imports: report.resolved_imports ?? report.imports ?? null };
  return component;
}
function byteVariants(input) {
  const declared = input.pre_session_mutations.filter(name => name !== 'nonempty-dataspace');
  assert.equal(declared.length, 14, 'exact fourteen hostile descriptor byte variants required');
  assert.deepEqual(Object.keys(input.descriptor_byte_variants).sort(), declared.slice().sort());
  const rows = declared.map(name => {
    const value = input.descriptor_byte_variants[name];
    let bytes;
    if (value.utf8 !== undefined) bytes = Buffer.from(value.utf8, 'utf8');
    else if (value.insert_hex !== undefined) {
      assert.equal(value.prefix_utf8.length > 0 && value.suffix_utf8.length > 0, true);
      bytes = Buffer.concat([Buffer.from(value.prefix_utf8), Buffer.from(value.insert_hex, 'hex'), Buffer.from(value.suffix_utf8)]);
    } else if (value.append_ascii_space_count !== undefined) {
      assert.equal(value.prefix, 'baseline_utf8');
      bytes = Buffer.concat([Buffer.from(input.baseline_utf8), Buffer.alloc(value.append_ascii_space_count, 0x20)]);
      assert.equal(bytes.length, value.total_bytes);
    } else {
      assert.equal(value.prefix, 'baseline_utf8');
      bytes = Buffer.from(input.baseline_utf8 + value.append_utf8);
    }
    const file = retain(`inputs/descriptor-variants/${name}.json`, bytes);
    return { name, file, sha256: hash(file), bytes: bytes.length };
  });
  assert.equal(rows.find(row => row.name === '513-byte-input').bytes, 513);
  assert.equal(rows.find(row => row.name === 'invalid-utf8').bytes > 0, true);
  return rows;
}
function strictJsonEdges(input) {
  const baseline = input.baseline_utf8;
  const replace = (oldText, next) => {
    assert.ok(baseline.includes(oldText), `strict JSON edge is not derived from canonical descriptor: ${oldText}`);
    return baseline.replace(oldText, next);
  };
  const cases = [
    {
      name: 'valid-escaped-ascii-keys-and-values', outcome: 'admit',
      utf8: replace('"protocol"', String.raw`"p\u0072otocol"`)
        .replace('"rounds"', String.raw`"rou\u006eds"`)
        .replace('"name":"clock"', String.raw`"n\u0061me":"cl\u006fck"`)
        .replace('@1.0.0', String.raw`\u00401.0.0`),
    },
    { name: 'decoded-duplicate-root-key', outcome: 'reject', error: 'Descriptor',
      utf8: replace('{"protocol":', String.raw`{"p\u0072otocol":"noble:choreography/service@1.0.0","protocol":`) },
    { name: 'decoded-duplicate-round-key', outcome: 'reject', error: 'Descriptor',
      utf8: replace('"name":"clock"', String.raw`"n\u0061me":"clock","name":"clock"`) },
    { name: 'valid-surrogate-pair-then-protocol-refusal', outcome: 'reject', error: 'Protocol',
      utf8: replace('@1.0.0', String.raw`@\uD83D\uDE001.0.0`) },
    { name: 'unpaired-high-surrogate', outcome: 'reject', error: 'Descriptor',
      utf8: replace('@1.0.0', String.raw`@\uD8001.0.0`) },
    { name: 'unpaired-low-surrogate', outcome: 'reject', error: 'Descriptor',
      utf8: replace('@1.0.0', String.raw`@\uDC001.0.0`) },
    { name: 'invalid-unicode-hex', outcome: 'reject', error: 'Descriptor',
      utf8: replace('@1.0.0', String.raw`@\u00G01.0.0`) },
    { name: 'invalid-json-escape', outcome: 'reject', error: 'Descriptor',
      utf8: replace('@1.0.0', String.raw`@\q1.0.0`) },
    { name: 'unescaped-control-newline', outcome: 'reject', error: 'Descriptor',
      utf8: replace('@1.0.0', '@\n1.0.0') },
    { name: 'unescaped-control-nul', outcome: 'reject', error: 'Descriptor',
      utf8: replace('@1.0.0', '@\0' + '1.0.0') },
  ];
  assert.deepEqual(JSON.parse(cases[0].utf8), input.baseline,
    'positive escape fixture must decode to the exact canonical descriptor');
  assert.equal(JSON.parse(cases[3].utf8).protocol.includes('😀'), true,
    'valid surrogate pair fixture must reach typed protocol comparison');
  // JavaScript's JSON.parse permits lone UTF-16 surrogates, but the
  // production descriptor parser requires a valid Unicode scalar.
  assert.ok(JSON.parse(cases[4].utf8).protocol.includes('\uD800'));
  assert.ok(JSON.parse(cases[5].utf8).protocol.includes('\uDC00'));
  for (const item of cases.slice(6)) assert.throws(() => JSON.parse(item.utf8),
    `${item.name}: must be malformed JSON, not just an invalid service value`);
  return cases.map(({ name, outcome, error, utf8 }) => {
    const bytes = Buffer.from(utf8, 'utf8');
    assert.ok(bytes.length <= input.max_bytes, `${name}: fixture exceeds bound`);
    const file = retain(`inputs/strict-json/${name}.json`, bytes);
    return { name, outcome, error: error ?? null, file, sha256: hash(file), bytes: bytes.length };
  });
}
function caseRow(id, run, action) {
  const item = cases.get(id);
  assert.ok(item, `missing declared case ${id}`);
  const row = { id, input: item.input, expected: item.expected, result: 'running', evidence: [{ command: run.id }], variants: [] };
  receipt.cases.push(row);
  try { action(row, item); row.result = 'passed'; }
  catch (error) { row.result = 'failed'; row.failure = String(error.stack ?? error); }
}
function variants(row, observed, declared, check) {
  assert.ok(Array.isArray(observed), `${row.id}: no independently reported hostile observations`);
  assert.deepEqual(observed.map(item => item.name).sort(), declared.slice().sort(), `${row.id}: incomplete/duplicate hostile observations`);
  for (const name of declared) {
    const item = observed.find(entry => entry.name === name);
    const result = { name, observed: item, result: 'running' };
    row.variants.push(result);
    try { check(item, name); result.result = 'passed'; }
    catch (error) { result.result = 'failed'; throw error; }
  }
}
function preSessionControl(item, name, expectation, bytes) {
  assert.equal(item.outcome, 'reject');
  assert.equal(item.typed_descriptors_created, expectation.typed_descriptors_created);
  assert.equal(item.facet_rights_created, expectation.facet_rights_created);
  assert.equal(item.guest_requests, expectation.guest_requests);
  assert.equal(item.protected_operations, expectation.protected_operations);
  assert.deepEqual(item.before, item.after, `${name}: pre-session refusal changed dataspace`);
  if (name !== 'nonempty-dataspace') {
    assert.equal(item.input_sha256, bytes.get(name).sha256, `${name}: peer did not use gate-provided bytes`);
    const stage = expectation.byte_variant_rejection_stage[name];
    if (stage) assert.equal(item.stage, stage, `${name}: wrong parser refusal stage`);
  }
  assert.equal(typeof item.error, 'string');
  assert.ok(item.error.length > 0);
}
function preExportControl(item, expectation, name) {
  assert.equal(item.outcome, 'reject');
  assert.equal(item.guest_invoked, false);
  if (name === 'wrong-wit-version' || name === 'wrong-wit-signature') {
    const label = name === 'wrong-wit-version' ? 'wrong-version' : 'wrong-signature';
    assert.equal(item.stage, 'link', `${name}: malformed compiled artifact not rejected at typed linking`);
    assert.equal(item.component_sha256, receipt.components[label].sha256);
    assert.equal(item.facet_rights_created, 0);
    assert.equal(typeof item.error, 'string');
    assert.ok(item.error.length > 0);
  } else {
    const attempts = {
      'wrong-role': ['publisher', 'observer', 'clock', false],
      'wrong-component-role': ['publisher', 'observer', 'clock', false],
      'out-of-order': ['publisher', 'publisher', 'clock', false],
      'replayed-step': ['subscriber', 'observer', 'clock', false],
      'replayed-export': ['subscriber', 'observer', 'clock', false],
      'wrong-export-publisher-instead-of-publish-and-trap': ['publisher', 'publisher', 'alarm', true],
      'wrong-export-at-trap-step': ['publisher', 'publisher', 'alarm', true],
      'wrong-name': ['subscriber', 'observer', 'alarm', false],
      'wrong-ready': ['subscriber', 'observer', 'clock', true],
      'foreign-facet': ['subscriber', 'observer', 'clock', false],
      'retired-facet': ['publisher', 'publisher', 'alarm', true],
      'forged-rights': ['subscriber', 'publisher', 'clock', false],
      'false-ready-mistaken-for-absence': ['subscriber', 'observer', 'clock', true],
      'false-ready-pair-confused-with-absence': ['subscriber', 'observer', 'clock', true],
    };
    const expected = attempts[name];
    assert.ok(expected, `unreviewed pre-export mutation ${name}`);
    assert.equal(item.stage, 'pre-export');
    assert.deepEqual([item.attempt.role, item.attempt.export, item.attempt.name, item.attempt.ready], expected,
      `${name}: actual attempted export/arguments differ`);
    if (name !== 'foreign-facet')
      assert.equal(item.attempt.facet, item.before.rights[item.attempt.role].facet,
        `${name}: attempted facet differs from host-issued role token`);
    assert.match(item.error, /Denied/);
    if (name === 'retired-facet') {
      assert.equal(item.before.cursor, 7);
      assert.equal(item.before.dataspace.facets, 1);
    }
    if (name === 'false-ready-mistaken-for-absence' || name === 'false-ready-pair-confused-with-absence') {
      assert.equal(item.before.cursor, 2);
      assert.equal(item.before.dataspace.assertions, 1);
      assert.ok(item.before.events.some(event => event.kind === 'add'
        && event.name === 'clock' && event.ready === false));
    }
  }
  for (const field of ['cursor', 'dataspace', 'events', 'rights', 'guest_requests', 'protected_operations'])
    assert.deepEqual(item.before[field], item.after[field], `pre-export refusal changed ${field}`);
  for (const [key, value] of Object.entries(expectation)) if (key.endsWith('_unchanged')) assert.equal(value, true);
}
function inFlightControl(item, expectation, name) {
  const missingFail = name === 'missing-fail-after-trap-publication'
    || name === 'missing-fail-after-publish-and-trap-publication';
  const wrongIdentity = name === 'wrong-import-identity' || name === 'wrong-import-after-admitted-export';
  assert.ok(missingFail || wrongIdentity || name === 'wrong-import-arguments', `unreviewed in-flight mutation ${name}`);
  assert.equal(item.outcome, 'reject');
  assert.equal(item.before.cursor, missingFail ? 6 : 3);
  assert.equal(item.before.pending, false);
  assert.equal(item.before_finish.pending, true);
  assert.equal(item.before_finish.cursor, item.before.cursor);
  assert.equal(item.after.pending, false);
  assert.equal(item.before.dataspace.facets, 2);
  assert.equal(item.after.dataspace.facets, 1);
  assert.equal(item.before.dataspace.assertions, missingFail ? 0 : 1);
  assert.equal(item.before_finish.dataspace.assertions, 1);
  assert.equal(item.after.dataspace.assertions, 0);
  assert.equal(item.owned_assertion_before, true);
  assert.equal(item.unauthorized_m7_operations, 0);
  assert.equal(item.unauthorized_protected_operations, 0);
  assert.equal(item.before.cursor, item.after.cursor);
  const expectedAuthorized = missingFail ? 1 : 0;
  assert.equal(item.after.protected_operations - item.before.protected_operations, expectedAuthorized);
  assert.equal(item.after.guest_requests - item.before.guest_requests, expectedAuthorized);
  assert.equal(item.after.refused_imports - item.before.refused_imports, missingFail ? 0 : 1);
  assert.equal(item.imports.length, 1, `${name}: an undeclared guest import was observed`);
  const call = item.imports[0];
  const operation = missingFail || wrongIdentity ? 'publish' : 'retract';
  assert.equal(call.operation, operation);
  assert.equal(call.identity, `noble:syndicate/dataspace@1.0.0#${operation}`);
  assert.equal(call.role, 'publisher');
  assert.equal(call.monitored, true);
  assert.equal(call.name, missingFail || wrongIdentity ? (missingFail ? 'alarm' : 'clock') : 'other');
  if (operation === 'publish') assert.equal(call.ready, true);
  if (missingFail) {
    assert.equal(call.accepted, true);
    assert.equal(call.result, true);
    assert.equal(item.guest_invocation_error, null);
    assert.match(item.monitor_finalization_error, /Invocation/);
    assert.equal(item.before_finish.protected_operations - item.before.protected_operations, 1);
  } else {
    assert.equal(call.accepted, false);
    assert.equal(call.error, 'ImportRefused');
    assert.match(item.guest_invocation_error, /ImportRefused/);
    assert.match(item.monitor_finalization_error, /Invocation/);
    assert.equal(item.before_finish.protected_operations, item.before.protected_operations);
  }
  assert.equal(item.publisher_retired, true);
  assert.equal(item.previously_owned_assertions_retracted, true);
  const added = item.after.events.slice(item.before.events.length);
  const pair = missingFail ? ['alarm', true] : ['clock', false];
  assert.deepEqual(added.map(({ kind, name: service, ready }) => [kind, service, ready]),
    missingFail ? [['add', ...pair], ['remove', ...pair]] : [['remove', ...pair]],
    `${name}: unexpected typed add/removal or illicit operation`);
  assert.deepEqual(item.typed_removal, added.at(-1));
  assert.equal(item.reported_success, false);
  assert.equal(item.choreography_completed, false);
  assert.equal(expectation.mandatory_trap_cleanup_permitted ?? expectation.facet_retired_on_protocol_failure, true);
}
try {
  for (const file of ['verification/m8/gate.mjs', 'verification/m8/build.mjs', 'verification/m8/assurance.mjs',
    'verification/m8/source-projection.mjs', 'verification/m8/final-documents.mjs',
    'crates/noble-wasm/wit/syndicate.wit', 'crates/noble-syndicate/Cargo.toml',
    '.cairn/specs/safety/spec.md', '.cairn/specs/wit-wasi/spec.md',
    ...Object.keys(selected), 'verification/m8/peer/Cargo.toml',
    'policy/architecture.ncl', 'policy/source-inventory.json', 'policy/tool-selection.json',
    'verification/m4/extraction-lock.json']) source(file);
  tree('crates/noble-kernel/src/dataspace');
  tree('crates/noble-syndicate/src');
  tree('verification/m8/peer/src');
  tree('.cairn/changes/m8-choreography-projection');
  bindCases();
  for (const [name, supplied] of [['cli', args[0]], ['peer', args[1]], ['wasm_tools', args[2]], ['node', process.execPath]])
    receipt.executables[name] = freeze(supplied, `executables/${name}`, true);
  const cli = receipt.executables.cli.frozen, peer = receipt.executables.peer.frozen;
  const wasmTools = receipt.executables.wasm_tools.frozen;
  const wit = fs.readFileSync(path.join(artifacts, 'sources/crates/noble-wasm/wit/syndicate.wit'));
  const witPath = retain('inputs/selected-syndicate.wit', wit);
  const publisher = compile('publisher', cli, wasmTools, witPath, wit, publisherBodies);
  const subscriber = compile('subscriber', cli, wasmTools, witPath, wit, subscriberBodies);
  assert.notEqual(hash(publisher), hash(subscriber), 'two separately compiled participant components must differ');
  const wrongVersionWit = Buffer.from(wit.toString('utf8').replace(
    'package noble:syndicate@1.0.0;', 'package noble:syndicate@1.0.1;'));
  assert.notDeepEqual(wrongVersionWit, wit, 'version control must alter the real WIT document');
  const wrongVersionPath = retain('inputs/wrong-version.wit', wrongVersionWit);
  const wrongVersion = compile('wrong-version', cli, wasmTools, wrongVersionPath,
    wrongVersionWit, publisherBodies, '1.0.1');
  const wrongSignatureWit = Buffer.from(wit.toString('utf8').replace(
    'publish: func(name: string, ready: bool) -> bool;',
    'publish: func(name: s64, ready: bool) -> bool;'));
  assert.notDeepEqual(wrongSignatureWit, wit, 'signature control must alter the real WIT document');
  const wrongSignaturePath = retain('inputs/wrong-signature.wit', wrongSignatureWit);
  const wrongSignatureBodies = { ...publisherBodies,
    publisher: 'drop drop false', 'publish-and-trap': 'drop drop false' };
  const wrongSignature = compile('wrong-signature', cli, wasmTools, wrongSignaturePath,
    wrongSignatureWit, wrongSignatureBodies);
  const badImportBodies = { ...publisherBodies, withdraw: 'true dataspace.publish' };
  const badIdentity = compile('bad-import-identity', cli, wasmTools, witPath, wit, badImportBodies);
  const badArgs = compile('bad-import-arguments', cli, wasmTools, witPath, wit,
    { ...publisherBodies, withdraw: 'drop "other" dataspace.retract' });
  const missingFail = compile('missing-fail', cli, wasmTools, witPath, wit,
    { ...publisherBodies, 'publish-and-trap': 'dataspace.publish' });
  const positive = cases.get('S-CASE-18').input;
  const runtime = cases.get('WI-19').input;
  const hostile = cases.get('S-CASE-19').input;
  assert.equal(positive.descriptor_utf8, runtime.descriptor_utf8);
  assert.equal(positive.descriptor_utf8, hostile.baseline_utf8);
  assert.deepEqual(positive.descriptor, runtime.descriptor);
  assert.deepEqual(positive.descriptor, hostile.baseline);
  const descriptor = retain('inputs/selected-descriptor.json', Buffer.from(positive.descriptor_utf8));
  assert.equal(fs.statSync(descriptor).size, positive.descriptor_bytes);
  assert.equal(positive.descriptor_bytes, hostile.baseline_bytes);
  const hostileBytes = byteVariants(hostile);
  const jsonEdges = strictJsonEdges(hostile);
  const oneRoundBytes = Buffer.from(JSON.stringify({
    protocol: hostile.baseline.protocol, rounds: hostile.baseline.rounds.slice(0, 1),
  }));
  assert.ok(oneRoundBytes.length <= hostile.max_bytes);
  const oneRound = retain('inputs/one-round-reservation.json', oneRoundBytes);
  const components = Object.fromEntries([
    ['wrong-wit-version', wrongVersion], ['wrong-wit-signature', wrongSignature],
    ['wrong-import-identity', badIdentity], ['wrong-import-arguments', badArgs],
    ['missing-fail-after-trap-publication', missingFail],
  ].map(([name, file]) => [name, { artifact: file, sha256: hash(file) }]));
  const workload = { schema: 'noble-m8-peer-workload/v1', descriptor,
    descriptor_sha256: hash(descriptor), variants: hostileBytes, strict_json_edges: jsonEdges,
    one_round: { file: oneRound, sha256: hash(oneRound), bytes: oneRoundBytes.length },
    components,
    cases: Object.fromEntries([...cases].map(([id, item]) => [id, { input: item.input, expected: item.expected }])) };
  const workloadFile = retain('inputs/selected-workload.json', JSON.stringify(workload, null, 2) + '\n');
  receipt.inputs.workload = { file: relative(workloadFile), sha256: hash(workloadFile),
    descriptor_sha256: hash(descriptor), components,
    variants: hostileBytes.map(({ name, sha256, bytes }) => ({ name, sha256, bytes })),
    strict_json_edges: jsonEdges.map(({ name, outcome, error, sha256, bytes }) =>
      ({ name, outcome, error, sha256, bytes })),
    one_round: { sha256: hash(oneRound), bytes: oneRoundBytes.length } };
  const run = command('independent-monitored-wasmtime-peer', peer,
    [publisher, subscriber, 'scenario', workloadFile], 300_000);
  const report = json(run);
  assert.equal(report.schema, 'noble-m8-peer/v1');
  assert.equal(report.outcome, 'passed');
  assert.equal(report.engine, 'wasmtime-40.0.2');
  assert.deepEqual(report.components, { publisher_sha256: hash(publisher), subscriber_sha256: hash(subscriber),
    distinct: true, separate_stores: true, threads_enabled: false, shared_memory_enabled: false });
  assert.equal(report.workload_sha256, hash(workloadFile));
  receipt.peer = { command: run.id, engine: report.engine, schema: report.schema, components: report.components };
  caseRow('S-CASE-18', run, (row, item) => {
    const projection = report.projection;
    assert.equal(projection.descriptor_sha256, hash(descriptor));
    assert.equal(projection.stage, item.expected.stage);
    assert.equal(projection.outcome, item.expected.outcome);
    assert.equal(projection.guest_requests, item.expected.guest_requests);
    assert.equal(projection.protected_operations, item.expected.protected_operations);
    assert.deepEqual(projection.global_actions, item.expected.global_actions);
    assert.deepEqual(projection.publisher_steps, item.expected.publisher_steps);
    assert.deepEqual(projection.subscriber_steps, item.expected.subscriber_steps);
    assert.equal(projection.descriptor_grants_authority, item.expected.descriptor_grants_authority);
    assert.equal(projection.new_wit_world_created, item.expected.new_wit_world_created);
  });
  caseRow('WI-19', run, (row, item) => {
    const compiled = report.compiled;
    assert.deepEqual(compiled.calls, report.projection.global_actions,
      'actual linked component calls must implement every projected action in exact order');
    assert.deepEqual(compiled.calls.map(call => call.step), item.input.dispatch_steps);
    const expectedImports = report.projection.global_actions.flatMap(action => {
      const operation = { publisher: 'publish', observer: 'observe',
        withdraw: 'retract', 'publish-and-trap': 'publish' }[action.export];
      assert.ok(operation, `unreviewed WIT export ${action.export}`);
      const call = { operation, identity: `noble:syndicate/dataspace@1.0.0#${operation}`,
        role: action.role, name: action.name, monitored: true, accepted: true,
        result: action.export === 'publish-and-trap' ? true : action.result };
      if (action.ready !== undefined && operation !== 'retract') call.ready = action.ready;
      const rows = [{ step: action.step, call }];
      if (action.export === 'publish-and-trap')
        rows.push({ step: action.step, call: { operation: 'fail',
          identity: 'noble:syndicate/dataspace@1.0.0#fail', role: action.role,
          monitored: true, value: true, accepted: false, error: 'GuestFail' } });
      return rows;
    });
    assert.deepEqual(compiled.imports, expectedImports,
      'actual monitored import identity, arguments, effects, order and guest-fail result must match every export');
    assert.match(compiled.trap_guest_error, /GuestFail/,
      'terminal trap must carry the actual monitored guest-fail import error');
    assert.deepEqual(compiled.publisher_exports, item.expected.publisher_exports);
    assert.deepEqual(compiled.subscriber_exports, item.expected.subscriber_exports);
    assert.deepEqual(compiled.observer_membership, item.expected.observer_membership);
    assert.deepEqual(compiled.events.map(({ kind, name, ready }) => ({ kind, name, ready })), item.expected.typed_events);
    assert.equal(new Set(compiled.events.map(event => event.observer)).size, 1,
      'all typed events must target one surviving subscriber facet');
    assert.deepEqual(compiled.trap_import_sequence, item.expected.trap_import_sequence);
    for (const key of ['trap_publish_import_succeeded', 'trap_fail_import_observed', 'compiled_invocation_error_observed',
      'compiled_trap_observed', 'subscriber_observes_absence_after_trap', 'subscriber_still_active',
      'false_readiness_is_distinct_from_absence']) assert.equal(compiled[key], item.expected[key], `${key}: trap/host observation`);
    assert.equal(compiled.publisher_trap_reported_success, false);
    assert.equal(compiled.remaining_publisher_assertions, 0);
    assert.equal(compiled.remaining.facets, 0);
    assert.equal(compiled.remaining.assertions, 0);
    assert.equal(compiled.remaining.interests, 0);
    assert.equal(compiled.all_imports_use_production_monitor, true);
    assert.equal(compiled.guest_facet_authority_from_descriptor, false);
    assert.equal(compiled.new_wit_world_created, false);
  });
  const hostileMap = new Map(hostileBytes.map(item => [item.name, item]));
  caseRow('S-CASE-19', run, (row, item) => {
    variants(row, report.controls.pre_session, item.input.pre_session_mutations,
      (observed, name) => preSessionControl(observed, name, item.expected.pre_session, hostileMap));
    assert.deepEqual(report.controls.strict_json_edges.map(entry => entry.name),
      jsonEdges.map(entry => entry.name), 'all supplemental strict JSON controls must execute in order');
    for (const [index, edge] of jsonEdges.entries()) {
      const observed = report.controls.strict_json_edges[index];
      assert.equal(observed.input_sha256, edge.sha256, `${edge.name}: wrong submitted bytes`);
      assert.equal(observed.outcome, edge.outcome, `${edge.name}: wrong parser verdict`);
      assert.deepEqual(observed.before, observed.after, `${edge.name}: descriptor edge changed clean profile`);
      assert.equal(observed.facet_rights_created, edge.outcome === 'admit' ? 2 : 0,
        `${edge.name}: wrong pre-facet admission effect`);
      assert.equal(observed.facet_rights_remaining, 0, `${edge.name}: edge retained a facet`);
      if (edge.outcome === 'admit')
        assert.deepEqual(observed.global_actions, report.projection.global_actions,
          `${edge.name}: escaped ASCII changed projected semantics`);
      else assert.equal(observed.error, edge.error, `${edge.name}: wrong syntax/value refusal stage`);
    }
    row.strict_json_edges = jsonEdges.length;
    const capacity = report.controls.one_round_reservations;
    assert.deepEqual(capacity.map(entry => entry.name),
      ['one-round-interests-one', 'one-round-events-two', 'one-round-full-reservation']);
    for (const [index, edge] of capacity.entries()) {
      assert.equal(edge.descriptor_sha256, hash(oneRound));
      assert.equal(edge.projected_actions, 5);
      assert.deepEqual(edge.before, [0, 0, 0]);
      assert.deepEqual(edge.after, [0, 0, 0], 'one-round reservation leaked facet rights');
      assert.equal(edge.limits.interests, index === 0 ? 1 : 2);
      assert.equal(edge.limits.events, index === 1 ? 2 : 4);
      assert.equal(edge.outcome, index === 2 ? 'admit' : 'reject');
      assert.equal(edge.error, index === 2 ? null : 'Capacity');
      assert.equal(edge.facets_admitted_before_cleanup, index === 2 ? 2 : 0);
    }
    row.one_round_reservations = capacity.length;
    variants(row, report.controls.pre_export, item.input.pre_export_mutations,
      (observed, name) => preExportControl(observed, item.expected.pre_export, name));
    variants(row, report.controls.in_flight, item.input.in_flight_mutations,
      (observed, name) => inFlightControl(observed, item.expected.in_flight, name));
  });
  caseRow('WI-20', run, (row, item) => {
    variants(row, report.controls.boundary_pre_export, item.input.pre_export_mutations,
      (observed, name) => preExportControl(observed, item.expected.pre_export, name));
    variants(row, report.controls.boundary_post_start, item.input.post_start_mutations,
      (observed, name) => inFlightControl(observed, item.expected.post_start, name));
    assert.equal(report.controls.direct_m7_profile_bypass, item.expected.direct_m7_profile_bypass);
  });
} catch (error) { receipt.failure = String(error.stack ?? error); }
finally {
  for (const entry of watched) try { assert.equal(hash(entry.file), entry.sha256); }
  catch (error) { receipt.integrity_failures.push({ file: entry.file, failure: String(error) }); }
  const required = Object.values(selected).flat().sort();
  const actual = receipt.cases.map(row => row.id).sort();
  receipt.summary = { required_cases: required.length, executed_cases: actual.length,
    passed_cases: receipt.cases.filter(row => row.result === 'passed').length,
    failed_cases: receipt.cases.filter(row => row.result === 'failed').map(row => row.id),
    missing_cases: required.filter(id => !actual.includes(id)),
    variants: receipt.cases.reduce((sum, row) => sum + row.variants.length, 0),
    strict_json_edges: receipt.cases.find(row => row.id === 'S-CASE-19')?.strict_json_edges ?? 0,
    one_round_reservations: receipt.cases.find(row => row.id === 'S-CASE-19')?.one_round_reservations ?? 0,
    commands: receipt.commands.length, integrity_failures: receipt.integrity_failures.length };
  receipt.result = !receipt.failure && !receipt.integrity_failures.length
    && JSON.stringify(required) === JSON.stringify(actual)
    && receipt.cases.every(row => row.result === 'passed' && row.variants.every(variant => variant.result === 'passed'))
    ? 'passed' : 'failed';
  fs.writeFileSync(path.join(artifacts, 'report.json'), JSON.stringify(receipt, null, 2) + '\n', { flag: 'wx', mode: 0o400 });
  console.log(JSON.stringify({ schema: receipt.schema, result: receipt.result,
    report: path.join(artifacts, 'report.json'), summary: receipt.summary }));
  process.exitCode = receipt.result === 'passed' ? 0 : 1;
}
