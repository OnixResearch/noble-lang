// Source-bound S-CASE-16 gate. Invoke with the freshly built CLI binary:
//   pinned-node verification/scase16/gate.mjs target/debug/noble [receipt-path]
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const cli = path.resolve(process.argv[2] ?? 'target/debug/noble');
const selected = JSON.parse(fs.readFileSync(path.join(root, 'crates/noble-cli/src/core/runtime/config.json'), 'utf8'));
const abi = JSON.parse(fs.readFileSync(path.join(root, 'crates/noble-cli/src/core/runtime/abi.json'), 'utf8'));
const { CoreEngine } = await import(path.join(root, 'crates/noble-cli/src/core/runtime/host.mjs'));
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const tmp = fs.mkdtempSync(path.join(process.env.TMPDIR ?? os.tmpdir(), 'noble-scase16-'));
const counts = { refusals: 0, executions: 0, unchanged_host: 0 };
const invoke = (binary, args, success = true) => {
  const result = spawnSync(binary, args, { encoding: 'utf8', env: {}, timeout: 60000, maxBuffer: 4 * 1024 * 1024 });
  if (result.error) throw result.error;
  assert.equal(result.status === 0, success, `${binary} ${args.join(' ')}: ${result.stderr}\n${result.stdout}`);
  return result.stdout;
};
const put = (name, bytes) => {
  const file = path.join(tmp, name);
  fs.writeFileSync(file, bytes);
  return file;
};
const manifest = (name, claims, other = {}) => put(`${name}.json`, JSON.stringify({ claimed_effects: claims, ...other }));
const source = (name, text) => put(`${name}.noble`, text);
const emit = (name, file, optimized = false, outcome = 'normal') => {
  const dir = path.join(tmp, name);
  const report = JSON.parse(invoke(cli, ['run', file, '--opt', optimized ? 'on' : 'off', '--emit', dir],
    outcome === 'normal'));
  assert.equal(report.outcome, outcome);
  return path.join(dir, 'engine', `module-1${optimized ? '-optimized' : ''}.wasm`);
};
const admit = (wasm, claims, { selectedSource, allow, optimized = false } = {}) => {
  const args = ['admit-artifact', wasm, '--effects', claims, '--opt', optimized ? 'on' : 'off'];
  if (selectedSource) args.push('--source', selectedSource);
  if (allow !== undefined) args.push('--allow-effects', allow);
  const result = spawnSync(cli, args, { encoding: 'utf8', env: {}, timeout: 60000, maxBuffer: 4 * 1024 * 1024 });
  if (result.error) throw result.error;
  const report = JSON.parse(result.stdout);
  assert.equal(result.status === 0, report.outcome === 'normal', result.stderr);
  return report;
};
const rejected = (report, outcome, imports) => {
  assert.equal(report.stage, 'admission');
  assert.equal(report.outcome, outcome);
  if (imports) assert.deepEqual(report.actual_imports, imports);
  assert.equal(report.guest_requests, 0);
  assert.equal(report.protected_operations, 0);
  counts.refusals++;
};
const accepted = (report, imports, requests, trace) => {
  assert.equal(report.stage, 'wasm');
  assert.equal(report.outcome, 'normal');
  assert.equal(report.admission.correspondence, 'byte-exact');
  assert.deepEqual(report.admission.actual_imports, imports);
  assert.equal(report.guest_requests, requests);
  assert.deepEqual(report.request_trace, trace);
  assert.equal(report.protected_operations, 0);
  counts.executions++;
};
const imports = file => WebAssembly.Module.imports(new WebAssembly.Module(fs.readFileSync(file)))
  .filter(entry => entry.kind === 'function').map(entry => `${entry.module}.${entry.name}`);
const tree = (directory, collected = []) => {
  for (const entry of fs.readdirSync(path.join(root, directory), { withFileTypes: true })) {
    const file = path.join(directory, entry.name);
    if (entry.isDirectory()) tree(file, collected);
    else if (entry.isFile()) collected.push(file);
  }
  return collected;
};
const raw = (name, wat) => {
  const input = put(`${name}.wat`, wat);
  const wasm = path.join(tmp, `${name}.wasm`);
  invoke(selected.tools.wasm_tools.path, ['parse', input, '-o', wasm]);
  invoke(selected.tools.wasm_tools.path, ['validate', wasm]);
  return wasm;
};
const hostUnchanged = (file, claims, expected) => {
  const engine = new CoreEngine(selected, abi);
  const beforeMemory = Buffer.from(new Uint8Array(engine.memory.buffer).subarray(0, 16));
  const beforeTable = engine.table.get(0);
  const report = engine.admit(fs.readFileSync(file), Buffer.alloc(0), Buffer.alloc(0), claims, claims, false);
  rejected(report, expected);
  assert.deepEqual(Buffer.from(new Uint8Array(engine.memory.buffer).subarray(0, 16)), beforeMemory);
  assert.equal(engine.table.get(0), beforeTable);
  assert.deepEqual(engine.trace, []);
  assert.equal(engine.protectedOperations, 0);
  engine.close();
  counts.unchanged_host++;
};

try {
  assert.equal(fs.realpathSync(process.execPath), fs.realpathSync(selected.tools.node.path));
  const canonical = JSON.parse(fs.readFileSync(path.join(root, 'specs/conformance/safety-cases.json'), 'utf8'));
  const case16 = canonical.cases.find(row => row.id === 'S-CASE-16');
  assert.deepEqual(case16.input.actual_imports, ['test.emit']);
  assert.deepEqual(case16.input.claimed_effects, []);
  assert.equal(case16.input.trusted_correspondence, false);
  assert.equal(case16.expected.stage, 'admission');
  assert.equal(case16.expected.outcome, 'effect-manifest-reject');

  const pure = source('pure', '1 2 +\n');
  const emitSource = source('emit', '"audit" test.emit\n');
  const latent = source('latent', '["audit" test.emit]\n');
  const abortSource = source('abort', 'test.abort\n');
  const different = source('different', '2 2 +\n');
  const pureWasm = emit('pure-build', pure);
  const emitWasm = emit('emit-build', emitSource);
  const latentWasm = emit('latent-build', latent);
  const abortWasm = emit('abort-build', abortSource, false, 'trap');
  assert.deepEqual(imports(pureWasm), []);
  assert.deepEqual(imports(emitWasm), ['noble.test_emit']);
  assert.deepEqual(imports(latentWasm), ['noble.test_emit']);
  assert.deepEqual(imports(abortWasm), ['noble.test_abort']);
  const empty = manifest('forged-empty', [], { trusted_correspondence: true, allowed: true,
    source: emitSource, digest: hash(fs.readFileSync(emitWasm)) });
  const claimEmit = manifest('emit-effect', ['test.emit'], { trusted_correspondence: true, allowed: true });
  const claimAbort = manifest('abort-effect', ['test.abort']);
  rejected(admit(emitWasm, empty, { selectedSource: emitSource, allow: 'test.emit' }),
    'effect-manifest-reject', ['test.emit']);
  rejected(admit(latentWasm, empty, { selectedSource: latent, allow: 'test.emit' }),
    'effect-manifest-reject', ['test.emit']);
  rejected(admit(emitWasm, claimEmit, { selectedSource: emitSource }), 'effect-policy-reject', ['test.emit']);
  rejected(admit(abortWasm, claimAbort, { selectedSource: abortSource }), 'effect-policy-reject', ['test.abort']);
  rejected(admit(abortWasm, empty, { selectedSource: abortSource, allow: 'test.abort' }),
    'effect-manifest-reject', ['test.abort']);
  const candidateSource = manifest('candidate-source', ['test.emit'],
    { source: emitSource, digest: hash(fs.readFileSync(emitWasm)), trusted_correspondence: true });
  rejected(admit(emitWasm, candidateSource, { allow: 'test.emit' }),
    'correspondence-reject', ['test.emit']);
  rejected(admit(emitWasm, claimEmit, { selectedSource: different, allow: 'test.emit' }),
    'correspondence-reject', ['test.emit']);
  const changed = put('changed-valid.wasm', Buffer.concat([fs.readFileSync(pureWasm), Buffer.from([0, 1, 0])]));
  invoke(selected.tools.wasm_tools.path, ['validate', changed]);
  const matchingCandidateSource = manifest('matching-candidate-source', [], {
    source: pure, digest: hash(fs.readFileSync(changed)), trusted_correspondence: true, allowed: true,
  });
  rejected(admit(changed, matchingCandidateSource, { selectedSource: pure }), 'correspondence-reject', []);
  rejected(admit(pureWasm, empty, { selectedSource: pure, optimized: true }), 'correspondence-reject', []);
  rejected(admit(put('malformed.wasm', Buffer.from([0, 97, 115, 109])), empty), 'invalid-artifact');
  const unknown = raw('unknown-import', '(module (import "evil" "test_emit" (func)))');
  rejected(admit(unknown, empty), 'invalid-artifact');
  const kindCollision = raw('wrong-kind', '(module (import "noble" "test_emit" (global i32)))');
  rejected(admit(kindCollision, empty), 'invalid-artifact');
  const signatureCollision = raw('wrong-signature',
    '(module (import "noble" "test_emit" (func (param i64) (result i64))))');
  rejected(admit(signatureCollision, claimEmit, { selectedSource: emitSource, allow: 'test.emit' }),
    'correspondence-reject', ['test.emit']);
  const duplicateImport = raw('duplicate-import',
    '(module (import "noble" "test_emit" (func (param i32 i32) (result i32))) (import "noble" "test_emit" (func (param i32 i32) (result i32))))');
  rejected(admit(duplicateImport, claimEmit, { selectedSource: emitSource, allow: 'test.emit' }),
    'invalid-artifact');
  const abort = raw('forged-abort', '(module (import "noble" "test_abort" (func (result i32))))');
  rejected(admit(abort, empty), 'effect-manifest-reject', ['test.abort']);
  const bound = raw('forged-bound', '(module (import "noble" "test_clock_bound" (func (param i32) (result i32 i64))))');
  rejected(admit(bound, manifest('clock', ['test.clock']), { allow: 'test.clock' }), 'invalid-artifact');
  const start = raw('raw-start', '(module (import "noble" "test_emit" (func $emit (param i32 i32) (result i32))) (func $start (drop (call $emit (i32.const 0) (i32.const 0)))) (start $start))');
  hostUnchanged(start, ['test.emit'], 'invalid-artifact');
  const data = raw('active-data', '(module (import "noble" "memory" (memory 16 16)) (data (i32.const 0) "MUTATE"))');
  hostUnchanged(data, [], 'correspondence-reject');
  const element = raw('active-element', '(module (import "noble" "table" (table 16384 16384 funcref)) (func $evil) (elem (i32.const 0) func $evil))');
  hostUnchanged(element, [], 'correspondence-reject');
  const engine = new CoreEngine(selected, abi);
  const pureWat = fs.readFileSync(path.join(tmp, 'pure-build', 'engine', 'module-1.wat'));
  engine.prepare(pureWat, fs.readFileSync(pure));
  rejected(engine.admit(fs.readFileSync(emitWasm), Buffer.alloc(0), Buffer.alloc(0),
    ['test.emit'], ['test.emit'], false), 'stale-session-reject');
  engine.close();
  assert.throws(() => {
    const direct = new CoreEngine(selected, abi);
    try { direct.install(new WebAssembly.Module(fs.readFileSync(pureWasm))); }
    finally { direct.close(); }
  }, /raw module installation is not artifact admission/);
  assert.throws(() => new CoreEngine({
    ...selected, tools: { ...selected.tools,
      wasm_tools: { ...selected.tools.wasm_tools, version: 'not-the-pinned-version' } },
  }, abi), /incompatible wasm_tools/);
  rejected(admit(emitWasm, manifest('duplicate-effect', ['test.emit', 'test.emit'])), 'invalid-manifest');
  rejected(admit(emitWasm, manifest('unknown-effect', ['not.an.effect'])), 'invalid-manifest');
  const malformedManifest = put('unknown-field.json', '{"claimed_effects":[],"unknown":true}');
  rejected(admit(emitWasm, malformedManifest), 'invalid-manifest');
  const pureReport = admit(pureWasm, empty, { selectedSource: pure });
  accepted(pureReport, [], 0, []);
  assert.deepEqual(pureReport.stack.map(value => [value.type, value.value]), [['I64', '3']]);
  accepted(admit(emitWasm, claimEmit, { selectedSource: emitSource, allow: 'test.emit' }),
    ['test.emit'], 1, ['test.emit:audit']);
  accepted(admit(latentWasm, claimEmit, { selectedSource: latent, allow: 'test.emit' }),
    ['test.emit'], 0, []);
  const abortReport = admit(abortWasm, claimAbort, { selectedSource: abortSource, allow: 'test.abort' });
  assert.equal(abortReport.stage, 'wasm');
  assert.equal(abortReport.outcome, 'trap');
  assert.deepEqual(abortReport.admission.actual_imports, ['test.abort']);
  assert.deepEqual(abortReport.request_trace, ['test.abort']);
  assert.equal(abortReport.guest_requests, 1);
  assert.equal(abortReport.protected_operations, 0);
  counts.executions++;
  const optimized = emit('emit-optimized', emitSource, true);
  accepted(admit(optimized, claimEmit, { selectedSource: emitSource, allow: 'test.emit', optimized: true }),
    ['test.emit'], 1, ['test.emit:audit']);

  const sourceFiles = [
    'crates/noble-cli/src/core/artifact.rs', 'crates/noble-cli/src/core.rs',
    'crates/noble-cli/src/main.rs', 'crates/noble-cli/src/core/worker.rs',
    'crates/noble-cli/src/core/worker/configuration.rs',
    'crates/noble-cli/src/core/runtime/engine.mjs',
    'crates/noble-cli/src/core/runtime/protocol.mjs',
    'crates/noble-cli/src/core/runtime/config.json',
    'crates/noble-cli/src/core/runtime/abi.json',
    'crates/noble-wasm/src/source/plan.rs',
    'crates/noble-wasm/src/source/emit/imports.rs',
    'crates/noble-wasm/runtime/source.wat',
    'verification/scase16/gate.mjs',
  ];
  const sourceTree = [
    ...tree('crates/noble-cli/src'), ...tree('crates/noble-wasm/src'),
    ...tree('crates/noble-wasm/runtime'), ...tree('crates/noble-contracts/src'),
    ...tree('crates/noble-kernel/src'),
  ].sort();
  const evidence = {
    schema: 'noble-scase16-source-bound-acceptance/v1', case: 'S-CASE-16',
    observed: counts, canonical: { stage: case16.expected.stage, outcome: case16.expected.outcome,
      actual_imports: case16.input.actual_imports, claimed_effects: case16.input.claimed_effects,
      case_sha256: hash(JSON.stringify([case16.id, case16.profile, case16.kind,
        case16.requirements, case16.input, case16.expected])) },
    final_artifacts: { pure_sha256: hash(fs.readFileSync(pureWasm)), emit_sha256: hash(fs.readFileSync(emitWasm)),
      latent_sha256: hash(fs.readFileSync(latentWasm)), abort_sha256: hash(fs.readFileSync(abortWasm)),
      optimized_emit_sha256: hash(fs.readFileSync(optimized)) },
    tool_sha256: { node: hash(fs.readFileSync(process.execPath)),
      wasm_tools: hash(fs.readFileSync(selected.tools.wasm_tools.path)),
      wasm_opt: hash(fs.readFileSync(selected.tools.wasm_opt.path)), cli: hash(fs.readFileSync(cli)) },
    source_sha256: Object.fromEntries(sourceFiles.map(file => [file, hash(fs.readFileSync(path.join(root, file)))])),
    source_tree_sha256: hash(sourceTree.map(file => `${file}\0${hash(fs.readFileSync(path.join(root, file)))}\n`).join('')),
    source_tree_files: sourceTree.length,
  };
  if (process.argv[3]) fs.writeFileSync(path.resolve(process.argv[3]), `${JSON.stringify(evidence, null, 2)}\n`, { flag: 'wx' });
  console.log(JSON.stringify(evidence));
} finally {
  fs.rmSync(tmp, { recursive: true, force: true });
}
