import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { auditSources, modulePaths, DEFAULT_RETAINED, OPTED_RETAINED,
  MAX_SOURCE, RetainedNamespace, selectedCompilerReceipt } from './retention.mjs';

// The default gate executes the real Noble/Wasm guest through the thin shell.
// --vectors-only checks source contracts without executing or accepting a guest.
// A shell failure, compiler refusal, timeout or missing output never passes.
const sourceOnly = process.argv[2] === '--vectors-only';
const [binary, numericModules, languageRoot, retainedArgs] = sourceOnly
  ? [null, process.argv[4] ?? null, process.argv[3], process.argv.slice(5)]
  : [...process.argv.slice(2, 5), process.argv.slice(5)];
const opted = (retainedArgs.length === 2 && sourceOnly || retainedArgs.length === 4) &&
  retainedArgs[0] === '--retained-source-bytes' &&
  retainedArgs[1] === String(OPTED_RETAINED) &&
  (retainedArgs.length === 2 || retainedArgs[2] === '--compiled-retention-receipt' &&
    retainedArgs[3].length > 0);
assert.ok(retainedArgs.length === 0 || opted, 'only the explicit tested 262144-byte opt-in is supported');
assert.ok(languageRoot && (sourceOnly || binary && numericModules),
  'usage: gate.mjs NOBLE_BINARY NUMERIC_MODULE_DIR LANGUAGE_ROOT [--retained-source-bytes 262144 --compiled-retention-receipt MA1_VERIFICATION_JSON] | gate.mjs --vectors-only LANGUAGE_ROOT [NUMERIC_MODULE_DIR [--retained-source-bytes 262144]]');
const here = import.meta.dirname;
const retainedBound = opted ? OPTED_RETAINED : DEFAULT_RETAINED;
const sourceAudit = numericModules
  ? auditSources(modulePaths(languageRoot, numericModules, here), retainedBound) : null;
const selectedReceipt = !sourceOnly && opted
  ? selectedCompilerReceipt(binary, retainedArgs[3], numericModules, languageRoot, sourceAudit) : null;
// Exercise the same per-commit namespace model used by the shell with exact
// original outside sources. A rebind retains another framed copy for every
// replayed definition; a failed second replay cannot advance its ledger.
function checkReplayAccounting(audit) {
  const namespace = new RetainedNamespace(retainedBound);
  for (const { source } of audit.modules) namespace.commit(namespace.planModule(source));
  const sourceBytes = Math.floor((retainedBound - audit.module_bytes) / 3) - 4;
  assert.ok(sourceBytes > 0 && sourceBytes <= MAX_SOURCE,
    'selected source graph leaves no three-definition/replay quota control');
  const definition = (name, padding) => {
    const prefix = Buffer.from(`def ${name} [ 1 ]\n#`);
    assert.ok(prefix.length <= sourceBytes, 'quota fixture definition cannot fit');
    return Buffer.concat([prefix, Buffer.alloc(sourceBytes - prefix.length, padding)]);
  };
  const first = definition('alpha', 0x61), second = definition('beta', 0x62);
  namespace.commit(namespace.planDefinition(first));
  namespace.commit(namespace.planDefinition(second));
  namespace.commit(namespace.planReplay([1]));
  const accepted = auditSources(modulePaths(languageRoot, numericModules, here),
    retainedBound, [first, second, second]);
  assert.equal(namespace.retainedTotalBytes, accepted.retained_total_bytes);
  assert.equal(namespace.outsideHistoryBytes, 3 * (sourceBytes + 4));
  const before = [namespace.moduleBytes, namespace.outsideHistoryBytes,
    namespace.definitionCount, namespace.retainedTotalBytes];
  namespace.checkExpression(Buffer.from('calc_session@1.empty'));
  assert.throws(() => namespace.planDefinition(Buffer.from('def overflow [ 1 ]')),
    /retained source preflight refused/);
  assert.throws(() => namespace.planReplay([0]), /retained source preflight refused/);
  assert.deepEqual([namespace.moduleBytes, namespace.outsideHistoryBytes,
    namespace.definitionCount, namespace.retainedTotalBytes], before,
  'quota refusal mutated retained namespace history');
  assert.throws(() => namespace.planReplay([99]), /unknown retained definition/);
  return { accepted_total_bytes: accepted.retained_total_bytes,
    replay_refused_before_bytes: accepted.retained_total_bytes + first.length + 4,
    refusal_preserved_retained_total_bytes: namespace.retainedTotalBytes };
}
const replayAccounting = sourceAudit && retainedBound === OPTED_RETAINED
  ? checkReplayAccounting(sourceAudit) : null;
const auditReceipt = audit => ({
  default_retained_source_bytes: audit.default_retained_source_bytes,
  selected_retained_source_bytes: audit.selected_retained_source_bytes,
  per_source_bytes: audit.per_source_bytes,
  module_bytes: audit.module_bytes,
  outside_history_bytes: audit.outside_history_bytes,
  outside_history_prefix_bytes_per_definition: audit.outside_history_prefix_bytes_per_definition,
  retained_total_bytes: audit.retained_total_bytes,
  modules: audit.modules.map(({ file, name, depends_on, bytes, source_sha256 }) =>
    ({ file, name, depends_on, bytes, source_sha256 })),
});
async function digestFile(file) {
  const hash = createHash('sha256');
  for await (const chunk of fs.createReadStream(file)) hash.update(chunk);
  return hash.digest('hex');
}
async function sourceRevision(directory, include) {
  const hash = createHash('sha256');
  for (const name of fs.readdirSync(directory).filter(include).sort()) {
    hash.update(name).update('\0');
    hash.update(await digestFile(path.join(directory, name)));
  }
  return hash.digest('hex');
}
const vectors = JSON.parse(fs.readFileSync(path.join(here, 'cases.json')));
const capabilityFixture = path.join(here, '../../crates/noble-contracts/tests/calculator_capability.rs');
const capabilityFixtureSha256 = await digestFile(capabilityFixture);
const canonicalPath = path.join(here, '../../specs/conformance/calculator-cases.json');
const selectedCanonicalPath = path.join(languageRoot, 'specs/conformance/calculator-cases.json');
assert.equal(await digestFile(canonicalPath), await digestFile(selectedCanonicalPath),
  'calculator conformance contract differs from selected language source');
const canonical = JSON.parse(fs.readFileSync(selectedCanonicalPath));
assert.equal(vectors.source_revision, '0ea10d3f59dd7e5d878862869a64282283052f15');
const required = Object.fromEntries(canonical.cases.map(item => [item.id, item]));
for (const item of required['CALC-01'].input.vectors) {
  assert.deepEqual(vectors.expressions.find(test => test.input === item.expression)?.fraction,
    [item.numerator, item.denominator], `canonical CALC-01 missing: ${item.expression}`);
}
for (const item of required['CALC-02'].input.vectors) {
  const actual = vectors.errors.find(test => test.input === item.expression);
  assert.equal(actual?.category, item.category, `canonical CALC-02 missing: ${item.expression}`);
  assert.deepEqual(actual?.span, item.span, `canonical CALC-02 span: ${item.expression}`);
}
for (const item of required['CALC-03'].input.vectors)
  assert.equal(vectors.errors.find(test => test.input === item.expression)?.category,
    item.category, `canonical CALC-03 missing: ${item.expression}`);
for (const [index, item] of required['CALC-06'].input.operations.entries()) {
  const actual = vectors.session[index];
  const op = { 'define-variable': 'var', 'define-function': 'function',
    evaluate: 'eval' }[item.operation];
  assert.equal(actual?.op, op, `canonical CALC-06 operation ${index}`);
  for (const key of ['name', 'expression', 'parameters', 'body'])
    if (key in item) assert.deepEqual(actual[key], item[key],
      `canonical CALC-06 ${key} in operation ${index}`);
  if (item.expected_fraction)
    assert.deepEqual(actual.fraction, item.expected_fraction.split('/'),
      `canonical CALC-06 fraction in operation ${index}`);
  if (item.expected_error)
    assert.equal(actual.category, item.expected_error,
      `canonical CALC-06 error in operation ${index}`);
}
assert.deepEqual(vectors.open_controls.map(item => [item.id, item.status]),
  [['CALC-07/resource-capability-capture', 'PENDING-SELECTED-CHECKER']]);
assert.equal(vectors.open_controls[0].fixture,
  'crates/noble-contracts/tests/calculator_capability.rs');
assert.deepEqual(vectors.external_library_controls.map(item => [item.id, item.status]),
  [['CALC-04', 'OPEN'], ['CALC-05', 'OPEN']],
  'MA1 numeric acceptance cannot be inferred from application vectors');
for (const control of vectors.open_controls)
  assert.ok(required['CALC-07'].input.variants.some(item =>
    control.id === `CALC-07/${item.case}`),
  `open control is not a canonical CALC-07 variant: ${control.id}`);
const supportedDefinitions = {
  'unused-body-with-unknown-name': item => item.op === 'function' &&
    item.body === 'unknown' && item.category === 'UnknownName',
  'duplicate-parameter-name': item => item.op === 'function' &&
    Array.isArray(item.parameters) &&
    new Set(item.parameters).size !== item.parameters.length &&
    item.category === 'DefinitionError',
  'self-recursive-definition': item => item.op === 'function' &&
    item.body === 'f(z)' && item.category === 'DefinitionError',
  'cyclic-imported-definition-graph': item => item.op === 'import-fixture' &&
    item.category === 'DefinitionError' &&
    item.definitions?.some(def => def.name === 'cycle_left' &&
      def.body === 'cycle_right()') &&
    item.definitions?.some(def => def.name === 'cycle_right' &&
      def.body === 'cycle_left()'),
  'zero-argument-function-body-1-divided-by-0': item => item.op === 'function' &&
    item.parameters?.length === 0 && item.body === '1/0' && item.display === 'defined',
  'call-that-zero-argument-function': item => item.op === 'eval' &&
    item.expression === 'lazy()' && item.category === 'DivisionByZero',
};
for (const item of required['CALC-07'].input.variants) {
  if (vectors.open_controls.some(control => control.id === `CALC-07/${item.case}`)) continue;
  assert.ok(supportedDefinitions[item.case] &&
    [...vectors.session, ...vectors.import_fixtures].some(supportedDefinitions[item.case]),
  `canonical CALC-07 supported variant absent: ${item.case}`);
}
assert.ok(vectors.import_fixtures.some(test => test.op === 'import-fixture' &&
  test.category === 'DefinitionError' && test.state_unchanged === true),
  'cyclic imported graph has no transactional rejection');
assert.deepEqual(vectors.import_fixtures.slice(1, 4).map(test =>
  [test.op, test.expression, test.category ?? test.fraction]),
  [['eval', 'cycle_left()', 'UnknownName'],
    ['eval', 'cycle_right()', 'UnknownName'],
    ['eval', 'f(1)', ['101', '1']]],
  'rejected graph must leave both names absent and earlier f callable');
assert.ok(vectors.import_fixtures.some(test => test.op === 'import-fixture' &&
  test.definitions?.some(def => def.name === 'bridge' && def.body === 'f(n)') &&
  test.definitions?.some(def => def.name === 'imported' && def.body === 'bridge(n)')),
  'import graph has no positive bound-identity control');
assert.ok(vectors.session.some(item => item.op === 'var' && item.budget?.output === 1 &&
  item.category === 'ResourceLimit' && item.limit === 'OutputSize' && item.state_unchanged),
  'canonical CALC-08 definition-result output limit absent');
const limitKinds = new Set([...vectors.limits, ...vectors.session]
  .filter(item => item.category === 'ResourceLimit').map(item => item.limit));
const canonicalLimitNames = { 'input-bytes': 'InputBytes', 'numeric-digits': 'NumericDigits',
  'intermediate-integer-bits': 'IntermediateIntegerBits', 'syntax-depth': 'SyntaxDepth',
  'call-depth': 'CallDepth', 'evaluation-work': 'EvaluationWork',
  'output-size': 'OutputSize' };
for (const name of required['CALC-08'].input.limit_categories)
  assert.ok(limitKinds.has(canonicalLimitNames[name]), `canonical CALC-08 missing: ${name}`);
const isolatedBudgetKeys = { InputBytes: 'input', NumericDigits: 'digits',
  IntermediateIntegerBits: 'bits', SyntaxDepth: 'syntax',
  CallDepth: 'calls', EvaluationWork: 'work', OutputSize: 'output' };
for (const test of vectors.limits) {
  assert.equal(test.state_unchanged, true, `CALC-08 state control absent: ${test.limit}`);
  assert.deepEqual(Object.keys(test.budget), [isolatedBudgetKeys[test.limit]],
    `CALC-08 ${test.limit} constrains more than its own cap`);
}
assert.ok(vectors.session.some(test => test.expression === 'lazy()' &&
  test.limit === 'CallDepth' && test.state_unchanged === true &&
  Object.keys(test.budget ?? {}).length === 1 && test.budget.calls === 0),
  'CALC-08 independent call-depth witness absent');
for (const item of required['CALC-09'].input.vectors) {
  assert.ok(item.mode === 'default' || item.mode === 'finite-decimal',
    `CALC-09 mode not represented: ${item.mode}`);
  assert.match(item.fraction, /^-?(?:0|[1-9]\d*)\/[1-9]\d*$/,
    `CALC-09 exact fraction is not a decimal-free guest expression: ${item.fraction}`);
  assert.equal(typeof item.display, 'string', `CALC-09 display absent: ${item.fraction}`);
}
if (sourceOnly) {
  console.log(JSON.stringify({ outcome: 'source-vectors-reviewed',
    execution: 'not-run', canonical_sha256: await digestFile(selectedCanonicalPath),
    capability_checker_fixture_sha256: capabilityFixtureSha256,
    retained_replay_accounting: replayAccounting,
    ...(sourceAudit ? { retained_sources: auditReceipt(sourceAudit),
      compiled_flag_tested: false } : {}),
    open_controls: vectors.open_controls,
    external_library_controls: vectors.external_library_controls }));
  process.exit(0);
}
async function runCapabilityChecker() {
  const buildPath = path.join(path.dirname(path.dirname(path.dirname(fs.realpathSync(binary)))),
    'build.json');
  const buildBytes = fs.readFileSync(buildPath);
  assert.equal(createHash('sha256').update(buildBytes).digest('hex'),
    selectedReceipt.selected_cli_build_sha256, 'selected Cargo recipe changed before checker');
  const build = JSON.parse(buildBytes);
  const cargo = build.tools.cargo.path;
  const vendor = build.tools.vendor.path;
  const packageName = 'noble-calculator-checked-boundary';
  const temporary = fs.mkdtempSync(path.join(process.env.TMPDIR ?? os.tmpdir(),
    'noble-calculator-checker-'));
  try {
    for (const name of ['home', 'cargo', 'target', 'tmp']) fs.mkdirSync(path.join(temporary, name));
    const manifest = [
      '[package]', `name = "${packageName}"`, 'version = "0.0.0"',
      'edition = "2021"', 'publish = false', '',
      '[dependencies]',
      `noble-contracts = { path = ${JSON.stringify(path.join(languageRoot, 'crates/noble-contracts'))} }`,
      `noble-kernel = { path = ${JSON.stringify(path.join(languageRoot, 'crates/noble-kernel'))} }`,
      '', '[[test]]', 'name = "calculator_capability"',
      `path = ${JSON.stringify(capabilityFixture)}`, '',
    ].join('\n');
    const manifestPath = path.join(temporary, 'Cargo.toml');
    fs.writeFileSync(manifestPath, manifest, { flag: 'wx', mode: 0o400 });
    const env = { ...build.commands[0].env,
      HOME: path.join(temporary, 'home'), CARGO_HOME: path.join(temporary, 'cargo'),
      CARGO_TARGET_DIR: path.join(temporary, 'target'), TMPDIR: path.join(temporary, 'tmp'),
      NOBLE_CALCULATOR_SELECTED_ROOT: fs.realpathSync(languageRoot),
      NOBLE_CALCULATOR_NUMERIC_DIR: fs.realpathSync(numericModules),
      NOBLE_CALCULATOR_APP_DIR: fs.realpathSync(here),
    };
    assert.equal(fs.realpathSync(cargo),
      fs.realpathSync(path.join(JSON.parse(fs.readFileSync(
        path.join(languageRoot, 'policy/tool-selection.json'))).tool_paths.quality_rust.output,
      'bin/cargo')), 'capability checker did not select reviewed Cargo');
    const config = [
      '--config', 'source.crates-io.replace-with="m6-vendor"',
      '--config', `source.m6-vendor.directory=${JSON.stringify(vendor)}`,
    ];
    const invoke = (args, label) => {
      const run = spawnSync(cargo, args, {
        cwd: languageRoot, env, encoding: 'utf8',
        timeout: 3_600_000, maxBuffer: 16 * 1024 * 1024,
      });
      assert.equal(run.error, undefined, `${label}: ${run.error}`);
      assert.equal(run.signal, null, `${label}: ${run.signal}`);
      assert.equal(run.status, 0, `${label} failed:\n${run.stderr}`);
      return run;
    };
    invoke(['generate-lockfile', '--manifest-path', manifestPath, '--offline', ...config],
      'selected capability checker lock');
    const lockSha256 = fs.existsSync(path.join(temporary, 'Cargo.lock'))
      ? await digestFile(path.join(temporary, 'Cargo.lock')) : null;
    assert.match(lockSha256, /^[0-9a-f]{64}$/, 'selected checker lockfile absent');
    const checked = invoke(['test', '--manifest-path', manifestPath, '--offline', '--locked',
      ...config, '--test', 'calculator_capability', '--',
      '--exact', 'checked_calculator_capture_and_rebind_retention', '--nocapture'],
    'selected Resource capture and quota checker');
    assert.match(checked.stdout, /running 1 test[\s\S]*1 passed; 0 failed/,
      'selected checker did not execute and pass its one real fixture');
    return { outcome: 'passed',
      fixture_sha256: capabilityFixtureSha256,
      selected_cli_build_sha256: selectedReceipt.selected_cli_build_sha256,
      selected_cargo_sha256: build.tools.cargo.sha256,
      selected_rustc_sha256: build.tools.rustc.sha256,
      manifest_sha256: createHash('sha256').update(manifest).digest('hex'),
      lock_sha256: lockSha256,
      stdout_sha256: createHash('sha256').update(checked.stdout).digest('hex'),
      stderr_sha256: createHash('sha256').update(checked.stderr).digest('hex') };
  } finally {
    fs.rmSync(temporary, { recursive: true, force: true });
  }
}
const capabilityChecker = await runCapabilityChecker();
const commands = [
  ...vectors.expressions.map(test => ({ op: 'eval', expression: test.input })),
  ...vectors.errors.map(test => ({ op: 'eval', expression: test.input })),
  ...vectors.session.map(test => {
    const { fraction, category, limit, span, state_unchanged, display, ...command } = test;
    return command;
  }),
  ...vectors.limits.map(test => ({ op: 'eval', expression: test.expression, budget: test.budget })),
  { op: 'eval', expression: 'x' }, // output-limit failure must retain x=20
  ...required['CALC-09'].input.vectors.map(item => ({
    op: 'eval', expression: item.fraction,
    ...(item.mode === 'finite-decimal' ? { display: 'finite-decimal' } : {}),
  })),
  ...vectors.import_fixtures.map(test => {
    const { fraction, category, state_unchanged, display, ...command } = test;
    return command;
  }),
];
const compilerBinarySha256 = await digestFile(binary);
const numericRevisionSha256 = await sourceRevision(numericModules,
  name => name.endsWith('.noble'));
const applicationRevisionSha256 = await sourceRevision(here,
  name => /\.(noble|mjs|json)$/.test(name));
const run = spawnSync(process.execPath,
  [path.join(here, 'shell.mjs'), binary, numericModules, languageRoot,
    ...(opted ? ['--retained-source-bytes', String(OPTED_RETAINED),
      '--compiled-retention-receipt', retainedArgs[3]] : []),
    '--definition-fixtures', '--verify-state'], {
    input: commands.map(command => JSON.stringify(command)).join('\n') + '\n',
    encoding: 'utf8', timeout: 180000, maxBuffer: 32 * 1024 * 1024,
  });
assert.equal(run.error, undefined, `guest execution error: ${run.error}`);
assert.equal(run.signal, null, `guest execution timed out: ${run.stderr}`);
assert.equal(run.status, 0, `guest shell/compiler refused:\n${run.stderr}`);
const shellReceipts = run.stderr.split('\n')
  .filter(line => line.startsWith('calculator-source-receipt '))
  .map(line => JSON.parse(line.slice('calculator-source-receipt '.length)));
assert.equal(shellReceipts.length, 1, 'guest shell omitted its source-bound retention receipt');
const { compiled_flag_tested: flagTested, flag_proof: flagProof,
  selected_compiled_receipt: shellSelectedReceipt,
  initial_state_sha256: initialStateSha, ...linkedSources } = shellReceipts[0];
assert.deepEqual(linkedSources, auditReceipt(sourceAudit),
  'linked exact module sources differ from source preflight');
assert.match(initialStateSha, /^[0-9a-f]{64}$/, 'guest initial environment identity absent');
assert.deepEqual(shellSelectedReceipt, selectedReceipt,
  'selected compiler receipt differed between gate and linked shell');
assert.equal(flagTested, opted, 'selected binary retention flag was not tested as requested');
assert.equal(opted ? flagProof?.default_refused_above_bytes : flagProof,
  opted ? DEFAULT_RETAINED : null, 'default retained-source refusal was not preserved');
assert.equal(opted ? flagProof?.per_source_refused_above_bytes : null,
  opted ? 65_536 : null, 'per-source refusal was not preserved');
assert.deepEqual(auditReceipt(auditSources(
  modulePaths(languageRoot, numericModules, here), retainedBound)), auditReceipt(sourceAudit),
  'exact module sources changed during guest execution');
assert.equal(await digestFile(binary), compilerBinarySha256,
  'compiler binary changed during guest execution');
assert.equal(await sourceRevision(numericModules,
  name => name.endsWith('.noble')), numericRevisionSha256,
  'numeric guest source changed during execution');
assert.equal(await sourceRevision(here,
  name => /\.(noble|mjs|json)$/.test(name)), applicationRevisionSha256,
  'application guest source changed during execution');
assert.equal(await digestFile(capabilityFixture), capabilityFixtureSha256,
  'typed capability checker fixture changed during guest execution');
const replies = run.stdout.trim().split('\n').map(line => JSON.parse(line));
assert.equal(replies.length, commands.length, 'one guest outcome per submitted command');
let priorStateSha = initialStateSha;
let priorHistoryBytes = sourceAudit.outside_history_bytes;
for (const [index, reply] of replies.entries()) {
  assert.match(reply.state_sha256, /^[0-9a-f]{64}$/,
    `missing guest environment identity after command ${index}`);
  if (!reply.accepted)
    assert.equal(reply.state_sha256, priorStateSha,
      `failed command ${index} changed the prior guest environment`);
  assert.equal(reply.namespace_outside_history_bytes, priorHistoryBytes,
    `calculator expression ${index} unexpectedly changed compiler outside-definition history`);
  assert.equal(reply.namespace_retained_total_bytes,
    sourceAudit.module_bytes + reply.namespace_outside_history_bytes,
    `calculator expression ${index} retained namespace differs from linked modules and history`);
  assert.ok(reply.namespace_retained_total_bytes <= retainedBound,
    `calculator expression ${index} exceeded selected retained-source quota`);
  priorHistoryBytes = reply.namespace_outside_history_bytes;
  priorStateSha = reply.state_sha256;
}
let cursor = 0;
for (const test of vectors.expressions) {
  const actual = replies[cursor++];
  assert.equal(actual.accepted, true, test.input);
  assert.deepEqual(actual.fraction, test.fraction, test.input);
}
for (const test of vectors.errors) {
  const actual = replies[cursor++];
  assert.equal(actual.accepted, undefined, test.input);
  assert.equal(actual.error?.category, test.category, test.input);
  assert.deepEqual(actual.error?.span, test.span, test.input);
  assert.equal(actual.error?.provenance, 'unavailable', test.input);
}
for (const test of vectors.session) {
  const actual = replies[cursor++];
  if (test.category) {
    assert.equal(actual.accepted, undefined, JSON.stringify(test));
    assert.equal(actual.error?.category, test.category, JSON.stringify(test));
    if (test.limit) assert.equal(actual.error?.limit, test.limit, JSON.stringify(test));
    if (test.span) assert.deepEqual(actual.error?.span, test.span);
    continue;
  }
  assert.equal(actual.accepted, true, JSON.stringify(test));
  if (test.fraction) assert.deepEqual(actual.fraction, test.fraction);
  if (test.display) assert.equal(actual.display, test.display);
}
for (const test of vectors.limits) {
  const actual = replies[cursor++];
  assert.equal(actual.accepted, undefined, JSON.stringify(test));
  assert.equal(actual.error?.category, test.category, JSON.stringify(test));
  assert.equal(actual.error?.limit, test.limit, JSON.stringify(test));
}
assert.deepEqual(replies[cursor++].fraction, ['20', '1'], 'output failure changed x');
for (const item of required['CALC-09'].input.vectors) {
  const actual = replies[cursor++];
  assert.equal(actual.display, item.display, `canonical CALC-09 display ${item.fraction}`);
  assert.deepEqual(actual.fraction, item.fraction.split('/'),
    `canonical CALC-09 exact value ${item.fraction}`);
}
for (const test of vectors.import_fixtures) {
  const actual = replies[cursor++];
  if (test.category) {
    assert.equal(actual.accepted, undefined, JSON.stringify(test));
    assert.equal(actual.error?.category, test.category, JSON.stringify(test));
  } else {
    assert.equal(actual.accepted, true, JSON.stringify(test));
    if (test.display) assert.equal(actual.display, test.display);
    if (test.fraction) assert.deepEqual(actual.fraction, test.fraction);
  }
}
console.log(JSON.stringify({ outcome: 'supported-guest-vectors-passed',
  vectors: replies.length, compiler_binary_sha256: compilerBinarySha256,
  numeric_revision_sha256: numericRevisionSha256,
  application_revision_sha256: applicationRevisionSha256,
  capability_checker_fixture_sha256: capabilityFixtureSha256,
  capability_checker: capabilityChecker,
  retained_sources: auditReceipt(sourceAudit),
  selected_compiled_receipt: selectedReceipt,
  initial_state_sha256: initialStateSha, final_state_sha256: priorStateSha,
  compiled_flag_tested: flagTested, flag_proof: flagProof,
  retained_replay_accounting: replayAccounting,
  final_outside_history_bytes: priorHistoryBytes,
  open_controls: vectors.open_controls,
  external_library_controls: vectors.external_library_controls }));
