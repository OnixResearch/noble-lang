#!/usr/bin/env node
// Bounded observation of the prebuilt production compiler and real Wasm child.
// This is not source-bound acceptance, MA1, or CALC-08 evidence.
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const root = fs.realpathSync(path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..'));
const digest = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const hash = file => digest(fs.readFileSync(file));
const sourceNames = [
  'crates/noble-contracts/src/fixtures/result.noble',
  'crates/noble-contracts/src/fixtures/calc-errors.noble',
];
assert.equal(process.argv.length, 6,
  'usage: node verification/calculator-errors/gate.mjs PREBUILT_NOBLE EXPECTED_BINARY_SHA256 LIVE_OWNER_SOURCE_ID FRESH_EXTERNAL_DIR');
const binary = path.resolve(process.argv[2]);
const expectedBinary = process.argv[3];
const sourceIdentity = process.argv[4];
const output = path.resolve(process.argv[5]);
assert.match(expectedBinary, /^[0-9a-f]{64}$/, 'live owner must supply prebuilt binary SHA-256');
assert.ok(sourceIdentity.length > 0 && sourceIdentity.length <= 256,
  'live owner must supply associated build-source identity');
assert.ok(output !== root && !output.startsWith(`${root}${path.sep}`),
  'receipt/artifacts must stay outside the repository');
assert.ok(!fs.existsSync(output), 'receipt directory must be fresh');
assert.equal(fs.realpathSync(path.dirname(output)), path.dirname(output),
  'receipt parent must not be a symlink');
fs.mkdirSync(output);
const receipt = {
  schema: 'noble-calculator-error-schema-observation/v1', result: 'running',
  mode: 'bounded-live-worker-prebuilt-observation',
  claim: 'finite-prebuilt-compiled-error-schema-observation-only',
  non_claims: ['source-bound acceptance', 'MA1', 'CALC-08', 'calculator',
    'numeric-budget-accounting', 'proof'],
  sources: {}, executable: null, commands: [], cases: [], error: null,
};
const retain = (name, bytes) => {
  const file = path.join(output, name);
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, bytes, { flag: 'wx', mode: 0o400 });
  return { file: name, sha256: hash(file), bytes: bytes.length };
};
const recordFile = path.join(output, 'acceptance.json');
const command = (label, args, input = Buffer.alloc(0)) => {
  const index = receipt.commands.length + 1;
  const prefix = `commands/${String(index).padStart(3, '0')}-${label}`;
  const stdin = retain(`${prefix}.stdin`, input);
  const run = spawnSync(binary, args, {
    cwd: root, input, encoding: null, timeout: 180_000, killSignal: 'SIGKILL',
    maxBuffer: 32 * 1024 * 1024,
    env: { PATH: '/nonexistent', LANG: 'C', LC_ALL: 'C', RUST_BACKTRACE: '1' },
  });
  const stdout = retain(`${prefix}.stdout`, run.stdout ?? Buffer.alloc(0));
  const stderr = retain(`${prefix}.stderr`, run.stderr ?? Buffer.alloc(0));
  receipt.commands.push({ label, argv: [binary, ...args], cwd: root,
    binary_sha256: hash(binary), stdin, stdout, stderr,
    status: run.status, signal: run.signal, error: run.error ? String(run.error) : null });
  assert.equal(run.signal, null, `${label}: child timed out or was killed`);
  assert.equal(run.error, undefined, `${label}: child could not launch`);
  return { status: run.status, bytes: run.stdout ?? Buffer.alloc(0) };
};
const parseReports = (run, count) => {
  const lines = run.bytes.toString('utf8').split('\n').filter(Boolean);
  assert.equal(lines.length, count, 'unexpected number of source reports');
  return lines.map(line => {
    const report = JSON.parse(line);
    assert.equal(report.schema, 'noble-core-report/v1');
    assert.equal(report.profile, 'Declared-Modules-v1');
    return report;
  });
};
const quiet = report => {
  for (const field of ['guest_requests', 'host_requests', 'protected_operations',
    'candidate_prepare_requests', 'ambient_fallback_calls']) {
    // Static diagnostics and runtime reports both contain these counters.
    assert.equal(report[field], 0, `${field}: unexpected guest or host activity`);
  }
  assert.equal(report.acquired_authority, false);
};
const nominal = (value, module, ordinal, variant) => {
  assert.equal(value?.type, 'Nominal');
  assert.equal(value.module, module);
  assert.equal(value.ordinal, ordinal);
  assert.equal(value.shape, 'variant');
  assert.equal(value.variant, variant);
  return value.value;
};
const compiled = (report, sourceSha) => {
  assert.match(report.module?.wasm_sha256, /^[0-9a-f]{64}$/);
  assert.match(report.module?.wat_sha256, /^[0-9a-f]{64}$/);
  assert.equal(report.module.source_sha256, sourceSha);
  assert.ok(report.module.compilations > 0, 'not a compiled Wasm child');
  return { wasm_sha256: report.module.wasm_sha256, wat_sha256: report.module.wat_sha256 };
};

try {
  assert.equal(hash(binary), expectedBinary, 'prebuilt CLI differs from live owner binary SHA-256');
  for (const name of sourceNames) {
    const bytes = fs.readFileSync(path.join(root, name));
    const snapshot = retain(`sources/${path.basename(name)}`, bytes);
    receipt.sources[name] = snapshot;
  }
  receipt.executable = {
    file: binary, sha256: expectedBinary,
    source_identity: sourceIdentity,
    source_identity_attestation: 'supplied by live owner; not independently source-bound by this gate',
  };
  const binding = retain('inputs/empty-bindings.txt', Buffer.alloc(0));
  const result = path.join(output, receipt.sources[sourceNames[0]].file);
  const errors = path.join(output, receipt.sources[sourceNames[1]].file);
  const empty = path.join(output, binding.file);
  const inputs = [result, errors];
  const cases = [
    { name: 'resource-input-bytes',
      source: 'unit calc_errors@1.LimitIO.InputBytes calc_errors@1.LimitPrimary.IO calc_errors@1.LimitCategory.Primary calc_errors@1.NumericError.ResourceLimit result@1.Result.Err dup [ 1 + ] [ drop 0 ] result@1.Result.match\n',
      arms: [[10, 'left'], [5, 'left'], [4, 'left'], [0, 'left']] },
    { name: 'domain-zero-denominator',
      source: 'unit calc_errors@1.DomainArithmetic.ZeroDenominator calc_errors@1.DomainExtended.Arithmetic calc_errors@1.DomainError.Extended calc_errors@1.NumericError.Domain result@1.Result.Err dup [ 1 + ] [ drop 0 ] result@1.Result.match\n',
      arms: [[10, 'right'], [9, 'right'], [8, 'right'], [6, 'left']] },
  ];
  for (const opt of ['off', 'on']) for (const item of cases) {
    const label = `${item.name}-${opt}`;
    const expression = retain(`inputs/${label}.noble`, Buffer.from(item.source));
    const source = path.join(output, expression.file);
    const frames = [...inputs, source];
    const input = Buffer.concat(frames.flatMap(file => {
      const bytes = fs.readFileSync(file);
      return [Buffer.from(`${bytes.length}\n`), bytes];
    }));
    const run = command(label, ['session', '--framed', '--declared-modules', '--bindings',
      empty, '--opt', opt], input);
    assert.equal(run.status, 0, `${label}: compiled child failed`);
    const reports = parseReports(run, 3);
    for (const [index, module] of ['result', 'calc_errors'].entries()) {
      assert.equal(reports[index].stage, 'link');
      assert.equal(reports[index].outcome, 'linked');
      assert.equal(reports[index].resolved_module.name, module);
      assert.equal(reports[index].resolved_module.version, 1);
      quiet(reports[index]);
    }
    const report = reports[2];
    assert.equal(report.stage, 'wasm');
    assert.equal(report.outcome, 'normal');
    assert.equal(report.status, 0);
    assert.equal(report.session_state, 'retained');
    assert.equal(report.module.optimized, opt === 'on');
    quiet(report);
    assert.equal(report.native_trap, null);
    assert.deepEqual(report.request_trace, []);
    assert.equal(report.stack.length, 2);
    assert.equal(report.stack[1].type, 'I64');
    assert.equal(report.stack[1].value, '0', 'the actual Err branch did not run');
    const resultIdentity = reports[0].resolved_module.identity;
    const errorIdentity = reports[1].resolved_module.identity;
    let value = nominal(report.stack[0], resultIdentity, 0, 'right');
    for (const [ordinal, arm] of item.arms) value = nominal(value, errorIdentity, ordinal, arm);
    assert.equal(value.type, 'Unit');
    assert.equal(value.value, null);
    receipt.cases.push({ name: label, result: 'passed', result_module: resultIdentity,
      error_module: errorIdentity, source_sha256: expression.sha256,
      compiled: compiled(report, expression.sha256) });
  }
  const errorsMatrix = [
    { name: 'input-bytes', words: 'LimitIO.InputBytes LimitPrimary.IO LimitCategory.Primary NumericError.ResourceLimit',
      arms: [[10, 'left'], [5, 'left'], [4, 'left'], [0, 'left']] },
    { name: 'output-size', words: 'LimitIO.OutputSize LimitPrimary.IO LimitCategory.Primary NumericError.ResourceLimit',
      arms: [[10, 'left'], [5, 'left'], [4, 'left'], [0, 'right']] },
    { name: 'numeric-digits', words: 'LimitNumeric.NumericDigits LimitPrimary.Numeric LimitCategory.Primary NumericError.ResourceLimit',
      arms: [[10, 'left'], [5, 'left'], [4, 'right'], [1, 'left']] },
    { name: 'intermediate-integer-bits', words: 'LimitNumeric.IntermediateIntegerBits LimitPrimary.Numeric LimitCategory.Primary NumericError.ResourceLimit',
      arms: [[10, 'left'], [5, 'left'], [4, 'right'], [1, 'right']] },
    { name: 'evaluation-work', words: 'LimitRemaining.EvaluationWork LimitCategory.Remaining NumericError.ResourceLimit',
      arms: [[10, 'left'], [5, 'right'], [3, 'left']] },
    { name: 'syntax-depth', words: 'LimitDepth.SyntaxDepth LimitRemaining.NestedDepth LimitCategory.Remaining NumericError.ResourceLimit',
      arms: [[10, 'left'], [5, 'right'], [3, 'right'], [2, 'left']] },
    { name: 'call-depth', words: 'LimitDepth.CallDepth LimitRemaining.NestedDepth LimitCategory.Remaining NumericError.ResourceLimit',
      arms: [[10, 'left'], [5, 'right'], [3, 'right'], [2, 'right']] },
    { name: 'zero-denominator', words: 'DomainArithmetic.ZeroDenominator DomainExtended.Arithmetic DomainError.Extended NumericError.Domain',
      arms: [[10, 'right'], [9, 'right'], [8, 'right'], [6, 'left']] },
    { name: 'division-by-zero', words: 'DomainArithmetic.DivisionByZero DomainExtended.Arithmetic DomainError.Extended NumericError.Domain',
      arms: [[10, 'right'], [9, 'right'], [8, 'right'], [6, 'right']] },
    { name: 'nonintegral', words: 'DomainConversion.NonIntegral DomainError.Conversion NumericError.Domain',
      arms: [[10, 'right'], [9, 'left'], [7, 'left']] },
    { name: 'out-of-range', words: 'DomainConversion.OutOfRange DomainError.Conversion NumericError.Domain',
      arms: [[10, 'right'], [9, 'left'], [7, 'right']] },
    { name: 'noninteger-exponent', words: 'DomainExtended.NonIntegerExponent DomainError.Extended NumericError.Domain',
      arms: [[10, 'right'], [9, 'right'], [8, 'left']] },
  ];
  const matrixFrames = errorsMatrix.map((item, index) => {
    const bytes = Buffer.from(`${index ? 'drop ' : ''}unit ${item.words.split(' ').map(word => `calc_errors@1.${word}`).join(' ')}\n`);
    const input = retain(`inputs/matrix-${item.name}.noble`, bytes);
    return { ...item, input, file: path.join(output, input.file) };
  });
  const framedMatrix = Buffer.concat([...inputs, ...matrixFrames.map(item => item.file)]
    .flatMap(file => {
      const bytes = fs.readFileSync(file);
      return [Buffer.from(`${bytes.length}\n`), bytes];
    }));
  const matrix = command('all-categories-and-errors', ['session', '--framed',
    '--declared-modules', '--bindings', empty, '--opt', 'off'], framedMatrix);
  assert.equal(matrix.status, 0, 'category/error matrix child failed');
  const matrixReports = parseReports(matrix, 2 + matrixFrames.length);
  for (const [index, module] of ['result', 'calc_errors'].entries()) {
    assert.equal(matrixReports[index].stage, 'link');
    assert.equal(matrixReports[index].outcome, 'linked');
    assert.equal(matrixReports[index].resolved_module.name, module);
    quiet(matrixReports[index]);
  }
  const categoryModule = matrixReports[1].resolved_module.identity;
  for (const [index, item] of matrixFrames.entries()) {
    const report = matrixReports[index + 2];
    assert.equal(report.stage, 'wasm');
    assert.equal(report.outcome, 'normal');
    assert.equal(report.status, 0);
    quiet(report);
    assert.equal(report.stack.length, 1);
    let value = report.stack[0];
    for (const [ordinal, arm] of item.arms) value = nominal(value, categoryModule, ordinal, arm);
    assert.equal(value.type, 'Unit');
    assert.equal(value.value, null);
    receipt.cases.push({ name: item.name, result: 'passed', source_sha256: item.input.sha256,
      compiled: compiled(report, item.input.sha256), error_module: categoryModule });
  }
  const invalid = retain('inputs/wrong-category.noble', Buffer.from(
    'unit calc_errors@1.DomainArithmetic.DivisionByZero calc_errors@1.NumericError.ResourceLimit\n'));
  const refused = command('wrong-category', ['compile', path.join(output, invalid.file),
    '--declared-modules', '--bindings', empty, '--module', result, '--module', errors]);
  assert.notEqual(refused.status, 0, 'wrong nominal category was accepted');
  const [negative] = parseReports(refused, 1);
  assert.ok(['check', 'acceptance'].includes(negative.stage), 'not a checked type refusal');
  assert.ok(['reject', 'type-reject'].includes(negative.outcome), 'not a static type refusal');
  quiet(negative);
  receipt.cases.push({ name: 'wrong-category', result: 'static-refusal',
    stage: negative.stage, outcome: negative.outcome, source_sha256: invalid.sha256 });
  const unknown = retain('inputs/unknown-error.noble',
    Buffer.from('unit calc_errors@1.DomainArithmetic.UnknownError\n'));
  const missing = command('unknown-error', ['compile', path.join(output, unknown.file),
    '--declared-modules', '--bindings', empty, '--module', result, '--module', errors]);
  assert.notEqual(missing.status, 0, 'unknown numeric error was accepted');
  const [missingReport] = parseReports(missing, 1);
  assert.equal(missingReport.stage, 'resolve');
  assert.equal(missingReport.outcome, 'unbound-word');
  quiet(missingReport);
  receipt.cases.push({ name: 'unknown-error', result: 'static-refusal',
    stage: missingReport.stage, outcome: missingReport.outcome, source_sha256: unknown.sha256 });
  assert.equal(hash(binary), receipt.executable.sha256, 'CLI changed during acceptance');
  for (const [name, entry] of Object.entries(receipt.sources)) {
    assert.equal(hash(path.join(root, name)), entry.sha256, `source changed during observation: ${name}`);
  }
  receipt.result = 'observed';
} catch (error) {
  receipt.result = 'failed';
  receipt.error = String(error?.stack ?? error);
  process.exitCode = 1;
} finally {
  fs.writeFileSync(recordFile, JSON.stringify(receipt, null, 2) + '\n', { flag: 'wx', mode: 0o400 });
  console.log(JSON.stringify({ result: receipt.result, receipt: recordFile,
    cases: receipt.cases.map(item => item.name), error: receipt.error }));
}
