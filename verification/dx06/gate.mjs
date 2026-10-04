#!/usr/bin/env node
// Runs production CLI/Wasm for the six finite test.clock controls. The output
// is a local smoke record, NOT an immutable source-bound acceptance receipt.
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { CoreEngine } from '../../crates/noble-cli/src/core/runtime/host.mjs';

const root = fs.realpathSync(path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..'));
assert.equal(process.argv.length, 4, 'usage: SELECTED_NODE verification/dx06/gate.mjs NOBLE_BINARY NEW_EXTERNAL_DIRECTORY');
const cli = fs.realpathSync(process.argv[2]);
const output = path.resolve(process.argv[3]);
assert.ok(output !== root && !output.startsWith(`${root}${path.sep}`), 'external output required');
assert.ok(!fs.existsSync(output), 'fresh output directory required');
fs.mkdirSync(output);
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const frozen = path.join(output, 'executables/noble');
fs.mkdirSync(path.dirname(frozen));
fs.copyFileSync(cli, frozen, fs.constants.COPYFILE_EXCL);
fs.chmodSync(frozen, 0o500);
const cliSha256 = sha(fs.readFileSync(cli));
assert.equal(sha(fs.readFileSync(frozen)), cliSha256);
function inventory(folder, base) {
  const files = {};
  function visit(directory) {
    for (const item of fs.readdirSync(directory, { withFileTypes: true })
      .sort((left, right) => left.name.localeCompare(right.name))) {
      const file = path.join(directory, item.name);
      if (item.isDirectory()) visit(file);
      else {
        assert.ok(item.isFile(), `symlink or special file is not a frozen source: ${file}`);
        const relative = path.relative(base, file).split(path.sep).join('/');
        const bytes = fs.readFileSync(file);
        files[relative] = { sha256: sha(bytes), bytes: bytes.length };
      }
    }
  }
  visit(folder);
  return files;
}
function sources() {
  const selected = [
    'crates/noble-kernel/src', 'crates/noble-contracts/src',
    'crates/noble-wasm/src', 'crates/noble-cli/src',
    'verification/dx06/gate.mjs', 'README.md',
  ];
  return Object.assign({}, ...selected.map(item => {
    const file = path.join(root, item);
    return fs.statSync(file).isDirectory()
      ? inventory(file, root)
      : { [item]: { sha256: sha(fs.readFileSync(file)), bytes: fs.statSync(file).size } };
  }));
}
const sourceInventory = sources();
const sourceRevision = sha(Buffer.from(JSON.stringify(sourceInventory)));
const prePromotionCaseSha256 = sha(fs.readFileSync(
  path.join(root, 'specs/conformance/developer-experience-cases.json')));
const source = {
  module: 'module clock@1 [\n  require now -- I64 ! test.clock\n  export tick\n  def tick [ now ]\n]\n',
  import: 'import clock@1 as c\n',
  call: 'c.tick\n',
  twice: 'c.tick c.tick\n',
  reflect: '[ c.tick ]\n',
};
const manifest = {
  allow: 'bind clock@1 test.clock version-1 -- I64 ! test.clock allow script 42\n',
  deny: 'bind clock@1 test.clock version-1 -- I64 ! test.clock deny script 42\n',
  missing: '',
  incompatible: 'bind clock@1 test.clock version-1 I64 -- I64 ! test.clock allow script 42\n',
};
const fixture = (kind, name, text) => {
  const file = path.join(output, `${kind}-${name}.txt`);
  fs.writeFileSync(file, text, { flag: 'wx' });
  return file;
};
const sourceFiles = Object.fromEntries(Object.entries(source).map(([name, text]) =>
  [name, fixture('source', name, text)]));
const manifests = Object.fromEntries(Object.entries(manifest).map(([name, text]) =>
  [name, fixture('manifest', name, text)]));
const framed = names => Buffer.concat(names.flatMap(name => {
  const bytes = fs.readFileSync(sourceFiles[name]);
  return [Buffer.from(`${bytes.length}\n`), bytes];
}));
const commands = [];
function launch(label, binding, names, expectedStatus, emit = false) {
  const emission = path.join(output, `${label}-emission`);
  const args = ['session', '--framed', '--declared-modules', '--bindings', manifests[binding],
    ...(emit ? ['--emit', emission] : [])];
  const result = spawnSync(frozen, args, { cwd: root, input: framed(names), encoding: 'utf8',
    timeout: 180_000, maxBuffer: 8 * 1024 * 1024 });
  fs.writeFileSync(path.join(output, `${label}.stdout`), result.stdout ?? '', { flag: 'wx' });
  fs.writeFileSync(path.join(output, `${label}.stderr`), result.stderr ?? '', { flag: 'wx' });
  assert.equal(result.error, undefined, `${label}: launch failure`);
  assert.equal(result.status, expectedStatus, `${label}: unexpected CLI status: ${result.stderr}`);
  const reports = result.stdout.trim().split('\n').map(line => JSON.parse(line));
  assert.equal(reports.length, names.length, `${label}: one report per source unit`);
  for (const report of reports) {
    assert.equal(report.profile, 'Declared-Modules-v1');
    assert.equal(report.schema, 'noble-core-report/v1');
    assert.equal(report.ambient_fallback_calls, 0);
  }
  commands.push({ label, argv: args, status: result.status,
    stdout_sha256: sha(result.stdout), stderr_sha256: sha(result.stderr) });
  return { reports, emission };
}
function quiet(report) {
  assert.equal(report.guest_requests, 0);
  assert.equal(report.host_requests, 0);
  assert.equal(report.protected_operations, 0);
  assert.equal(report.ambient_fallback_calls, 0);
  assert.equal(report.acquired_authority, false);
}
function runtime(report, count, protectedCount) {
  assert.equal(report.stage, 'wasm');
  assert.equal(report.host_requests, count);
  assert.equal(report.guest_requests, count);
  assert.equal(report.protected_operations, protectedCount);
  assert.equal(report.request_trace.length, count);
  assert.equal(report.request_trace_complete, true);
  assert.equal(report.effect_requests_unrecorded, 0);
  assert.deepEqual(report.effect_requests, Array(count).fill('test.clock'));
  assert.equal(report.declared_effect_preserved, true);
  assert.equal(report.real_host_fallback_calls, 0);
  assert.equal(report.real_host_evidence, false);
  assert.equal(report.acquired_authority, false);
  assert.equal(report.native_trap, null);
  assert.deepEqual(report.scripted_inputs[0].values, ['42']);
  assert.equal(report.substitutions[0].operation, 'test.clock');
  assert.equal(report.adapter_versions[0].version, 1);
  assert.equal(report.adapter_versions[0].adapter_identity, 'version-1');
  assert.equal(report.module.imports.some(item => item.name === 'test_clock_bound'), true);
  assert.equal(report.module.imports.some(item => item.name === 'test_emit_bound'), false);
}
const positive = launch('matching-interface', 'allow', ['module', 'import', 'call'], 0, true);
for (const report of positive.reports.slice(0, 2)) {
  quiet(report);
  assert.equal(report.outcome, 'linked');
}
const accepted = positive.reports[2];
runtime(accepted, 1, 1);
assert.equal(accepted.outcome, 'normal');
assert.deepEqual(accepted.stack.map(value => [value.type, value.value]), [['I64', '42']]);
assert.equal(accepted.request_trace[0].decision, 'allow');
assert.equal(accepted.request_trace[0].value, '42');
const wasm = path.join(positive.emission, 'engine/module-3.wasm');
const wat = path.join(positive.emission, 'engine/module-3.wat');
assert.equal(sha(fs.readFileSync(wasm)), accepted.module.wasm_sha256);
const selection = JSON.parse(fs.readFileSync(path.join(root, 'crates/noble-cli/src/core/runtime/config.json')));
const validation = spawnSync(selection.tools.wasm_tools.path, ['validate', wasm],
  { encoding: 'utf8', timeout: 30_000 });
assert.equal(validation.status, 0, `selected wasm-tools refused compiled Wasm: ${validation.stderr}`);
const reflection = launch('latent-effect', 'allow', ['module', 'import', 'reflect'], 0).reports[2];
runtime(reflection, 0, 0);
assert.equal(reflection.outcome, 'normal');
assert.equal(reflection.stack[0].type, 'Program');
assert.deepEqual(reflection.stack[0].interface.effects, ['test.clock']);
assert.deepEqual(reflection.stack[0].witnesses[0].effects, ['test.clock']);

const denied = launch('denied', 'deny', ['module', 'import', 'call'], 1).reports;
for (const report of denied.slice(0, 2)) quiet(report);
runtime(denied[2], 1, 0);
assert.equal(denied[2].outcome, 'trap');
assert.equal(denied[2].request_trace[0].decision, 'deny');
assert.equal(denied[2].request_trace[0].reason, 'policy-denied');

const missing = launch('missing-mapping', 'missing', ['module'], 2).reports[0];
quiet(missing);
assert.equal(missing.stage, 'link');
assert.equal(missing.outcome, 'reject');
assert.match(missing.diagnostic, /missing bound adapter/);

const incompatibleFile = manifests.incompatible;
const invalid = spawnSync(frozen, ['compile', sourceFiles.call, '--declared-modules',
  '--bindings', incompatibleFile, '--module', sourceFiles.module],
{ cwd: root, encoding: 'utf8', timeout: 180_000 });
fs.writeFileSync(path.join(output, 'incompatible.stdout'), invalid.stdout, { flag: 'wx' });
fs.writeFileSync(path.join(output, 'incompatible.stderr'), invalid.stderr, { flag: 'wx' });
assert.equal(invalid.status, 2);
const incompatible = JSON.parse(invalid.stdout.trim());
assert.equal(incompatible.stage, 'binding');
assert.equal(incompatible.outcome, 'invalid-input');
assert.match(incompatible.diagnostic, /test.clock binding requires/);
assert.equal(incompatible.guest_requests, 0);
assert.equal(incompatible.protected_operations, 0);
commands.push({ label: 'incompatible-interface', status: invalid.status,
  stdout_sha256: sha(invalid.stdout), stderr_sha256: sha(invalid.stderr) });

const exhausted = launch('script-exhausted', 'allow', ['module', 'import', 'twice'], 1).reports;
for (const report of exhausted.slice(0, 2)) quiet(report);
runtime(exhausted[2], 2, 1);
assert.equal(exhausted[2].outcome, 'trap');
assert.deepEqual(exhausted[2].request_trace.map(item => [item.decision, item.reason ?? null]),
  [['allow', null], ['deny', 'script-exhausted']]);

// Adversarial control only: alter the accepted WAT at the host import call to
// request test.emit using the clock slot. This is not an accepted guest source
// or a substitute for independent frontend/backend acceptance.
const runtimeRoot = path.join(root, 'crates/noble-cli/src/core/runtime');
const abi = JSON.parse(fs.readFileSync(path.join(runtimeRoot, 'abi.json')));
const extension = fs.readFileSync(path.join(runtimeRoot, 'declared-abi.json'), 'utf8');
const binding = { slot: 0, module: 'clock', version: 1, adapter: 'version-1',
  operation: 'test.clock', allowed: true, script: ['42'],
  dispatch: [{ decision: 'allow', value: '42' }, { decision: 'script-exhausted', value: null }] };
const engine = new CoreEngine(selection, abi,
  { declared_modules: true, declared_extension: extension, bindings: [binding] });
let unexpected;
try {
  let modified = fs.readFileSync(wat, 'utf8');
  const originalImport = '(import "noble" "test_clock_bound" (func $host_clock_bound (param i32) (result i32 i64)))';
  const originalCall = '(call $host_clock_bound (local.get $slot))';
  assert.ok(modified.includes(originalImport) && modified.includes(originalCall));
  modified = modified.replace(originalImport, `${originalImport}\n(import "noble" "test_emit_bound" (func $host_unexpected (param i32 i32 i32) (result i32)))`)
    .replace(originalCall, '(call $host_unexpected (i32.const 0) (i32.const 0) (local.get $slot))\n (i64.const 0)');
  fs.writeFileSync(path.join(output, 'unexpected-operation.wat'), modified, { flag: 'wx' });
  engine.prepare(Buffer.from(modified), fs.readFileSync(sourceFiles.call));
  unexpected = engine.execute();
  assert.equal(unexpected.module.imports.some(item => item.name === 'test_emit_bound'), true);
  assert.equal(unexpected.outcome, 'trap');
  assert.equal(unexpected.status, 5);
  assert.equal(unexpected.host_requests, 1);
  assert.equal(unexpected.protected_operations, 0);
  assert.equal(unexpected.request_trace[0].reason, 'unexpected-operation');
  assert.equal(unexpected.request_trace[0].decision, 'deny');
  assert.equal(unexpected.real_host_fallback_calls, 0);
  assert.equal(unexpected.real_host_evidence, false);
} finally { engine.close(); }
assert.deepEqual(sources(), sourceInventory, 'production source changed during the DX-06 smoke');
assert.equal(sha(fs.readFileSync(cli)), cliSha256, 'production binary changed during the DX-06 smoke');
const retained = inventory(output, output);
const summary = { case_id: 'DX-06', profile: 'Test-Host-Draft', result: 'six-controls-pass',
  proof: 'open', real_host_evidence: false, real_host_fallback_calls: 0,
  source_root: root, source_revision: `sha256:${sourceRevision}`,
  source_files: sourceInventory, pre_promotion_case_sha256: prePromotionCaseSha256,
  production_cli_sha256: cliSha256, frozen_cli: 'executables/noble',
  selected_node_sha256: sha(fs.readFileSync(process.execPath)),
  retained,
  compiled_wasm_sha256: sha(fs.readFileSync(wasm)), commands,
  latent_effect_witness: reflection.stack[0].interface.effects,
  controls: {
    'authorized-matching-interface': { outcome: accepted.outcome, value: accepted.stack[0].value },
    denied: { outcome: denied[2].outcome, request: denied[2].request_trace[0] },
    'missing-mapping': { outcome: missing.outcome, diagnostic: missing.diagnostic },
    'incompatible-interface': { outcome: incompatible.outcome, diagnostic: incompatible.diagnostic },
    'second-request-script-exhausted': { outcome: exhausted[2].outcome,
      requests: exhausted[2].request_trace },
    'unexpected-operation': { outcome: unexpected.outcome, request: unexpected.request_trace[0] },
  } };
fs.writeFileSync(path.join(output, 'smoke.json'), JSON.stringify(summary, null, 2) + '\n', { flag: 'wx' });
console.log(JSON.stringify(summary));
