import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

// An independent owner supplies the source-bound CLI binary and its digest.
const [binary, expectedDigest, output, buildEvidence] = process.argv.slice(2);
assert.match(expectedDigest ?? '', /^[0-9a-f]{64}$/);
assert.ok(binary && output && buildEvidence && !fs.existsSync(output));
const root = path.resolve(import.meta.dirname, '../..');
const digest = bytes => createHash('sha256').update(bytes).digest('hex');
assert.equal(digest(fs.readFileSync(binary)), expectedDigest);
const buildBytes = fs.readFileSync(path.join(buildEvidence, 'build.json'));
const manifestBytes = fs.readFileSync(path.join(buildEvidence, 'source-manifest.json'));
const build = JSON.parse(buildBytes);
const manifest = JSON.parse(manifestBytes);
assert.equal(build.status, 0);
assert.equal(build.binary_sha256, expectedDigest);
assert.equal(build.source_manifest_sha256, digest(manifestBytes));
assert.equal(manifest.files.length, build.source_files);
for (const entry of manifest.files) {
  const sourcePath = path.join(root, entry.path);
  const actualKind = fs.lstatSync(sourcePath).isSymbolicLink() ? 'symlink' : 'file';
  assert.equal(actualKind, entry.kind, `build input kind changed: ${entry.path}`);
  const source = actualKind === 'symlink'
    ? Buffer.from(fs.readlinkSync(sourcePath)) : fs.readFileSync(sourcePath);
  assert.equal(source.length, entry.size, `build input changed: ${entry.path}`);
  assert.equal(digest(source), entry.sha256, `build input changed: ${entry.path}`);
}
const resultModule = fs.readFileSync(path.join(root, 'crates/noble-contracts/src/fixtures/result.noble'));
const digitModule = fs.readFileSync(new URL('./decimal-digit.noble', import.meta.url));
const laterDigitModule = Buffer.from('module decimal_digit@2 [ signature digit forall<S:stack> [ S I64 -- S I64 Bool ! pure ] export digit def digit [ drop 0 false ] ]');
const foldModule = fs.readFileSync(new URL('./decimal-fold.noble', import.meta.url));
const localModule = fs.readFileSync(new URL('./local-bindings.noble', import.meta.url));
const cases = [
  ['0', '0'], ['9', '9'], ['12345', '12345'],
  ['0009', '9'], ['999999999999999999', '999999999999999999'],
  ['', 'InvalidDigit'], ['1a', 'InvalidDigit'], ['é', 'InvalidDigit'],
  ['12é', 'InvalidDigit'], ['1234567890123456789', 'OutOfRange'],
];
const frame = bytes => Buffer.concat([Buffer.from(`${bytes.length}\n`), bytes]);
const record = { schema: 'noble-calculator-fold-precursor-local/v1',
  scope: 'bounded wrapping-I64 fold and finite lexical checked-stack sugar, not exact BigInt or CALC acceptance',
  binary_sha256: expectedDigest, result_sha256: digest(resultModule),
  build_sha256: digest(buildBytes), source_manifest_sha256: digest(manifestBytes),
  digit_sha256: digest(digitModule),
  later_digit_sha256: digest(laterDigitModule),
  module_sha256: digest(foldModule),
  local_module_sha256: digest(localModule),
  gate_sha256: digest(fs.readFileSync(new URL(import.meta.url))), runs: [] };
fs.mkdirSync(output);
fs.copyFileSync(binary, path.join(output, 'noble'), fs.constants.COPYFILE_EXCL);
for (const file of ['build.json', 'build.stdout', 'build.stderr', 'source-manifest.json']) {
  fs.copyFileSync(path.join(buildEvidence, file), path.join(output, file), fs.constants.COPYFILE_EXCL);
}
for (const [name, bytes] of [['result.noble', resultModule],
  ['decimal-digit.noble', digitModule], ['decimal-fold.noble', foldModule],
  ['decimal-digit-2.noble', laterDigitModule], ['local-bindings.noble', localModule]]) {
  fs.writeFileSync(path.join(output, name), bytes, { flag: 'wx' });
}
const run = (name, args, input) => {
  const artifactDir = /^(fold-|cursor-|less-|local-|rebind-|signed-tail-|digit-version-|same-session-|recover-source-)/.test(name)
    ? path.join(output, `${name}.artifacts`) : null;
  if (artifactDir) args = [...args, '--emit', artifactDir];
  const execution = spawnSync(binary, args, { cwd: root, input, encoding: null,
    timeout: 180_000, killSignal: 'SIGKILL', maxBuffer: 8 * 1024 * 1024,
    env: { PATH: '/nonexistent', LANG: 'C', LC_ALL: 'C', RUST_BACKTRACE: '1' } });
  for (const [suffix, bytes] of [['stdin', input], ['stdout', execution.stdout ?? Buffer.alloc(0)],
    ['stderr', execution.stderr ?? Buffer.alloc(0)]]) {
    fs.writeFileSync(path.join(output, `${name}.${suffix}`), bytes, { flag: 'wx' });
  }
  const item = { name, args, input_sha256: digest(input),
    stdout_sha256: digest(execution.stdout ?? Buffer.alloc(0)),
    stderr_sha256: digest(execution.stderr ?? Buffer.alloc(0)),
    status: execution.status, signal: execution.signal, error: String(execution.error ?? '') };
  record.runs.push(item);
  assert.equal(execution.error, undefined, `${name}: child spawn failed`);
  assert.equal(execution.signal, null, `${name}: child timed out`);
  const reports = (execution.stdout ?? Buffer.alloc(0)).toString('utf8').trim()
    .split('\n').filter(Boolean).map(line => JSON.parse(line));
  if (artifactDir && fs.existsSync(path.join(artifactDir, 'engine'))) {
    item.guest_artifacts = reports.filter(entry => entry.module).map(report => {
      const stem = report.module.stem;
      const watFile = path.join(artifactDir, 'engine', `${stem}.wat`);
      const wasmFile = path.join(artifactDir, 'engine',
        `${stem}${report.module.optimized ? '-optimized' : ''}.wasm`);
      const sourceFile = path.join(artifactDir, 'engine', `${stem}.noble`);
      const wat = fs.readFileSync(watFile);
      const wasm = fs.readFileSync(wasmFile);
      const source = fs.readFileSync(sourceFile);
      assert.equal(digest(wat), report.module.wat_sha256);
      assert.equal(digest(wasm), report.module.wasm_sha256);
      assert.equal(digest(source), report.module.source_sha256);
      return { wasm_sha256: digest(wasm), wat_sha256: digest(wat),
        source_sha256: digest(source), wasm_file: path.relative(output, wasmFile),
        wat_file: path.relative(output, watFile), source_file: path.relative(output, sourceFile) };
    });
  }
  return { status: execution.status, reports };
};
const flags = ['--framed', '--declared-modules', '--text-byte-cursor',
  '--signed-self-recursion'];
const localFlags = flags.filter(flag => flag !== '--signed-self-recursion');
for (const opt of ['off', 'on']) {
  for (const [index, [text, expected]] of cases.entries()) {
    const input = Buffer.concat([resultModule, digitModule, foldModule,
      Buffer.from(`${JSON.stringify(text)} 0 0 decimal_fold@1.fold`)].map(frame));
    const { status, reports } = run(`fold-${opt}-${index}`, ['session', ...flags, '--opt', opt], input);
    assert.equal(status, 0);
    assert.equal(reports.length, 4);
    for (const report of reports) {
      assert.equal(report.profile, 'Signed-Declared-Text-v1');
      assert.equal(report.host_requests, 0);
      assert.equal(report.guest_requests, 0);
      assert.equal(report.ambient_fallback_calls, 0);
      assert.equal(report.acquired_authority, false);
    }
    assert.deepEqual(reports.slice(0, 3).map(report => report.outcome), ['linked', 'linked', 'linked']);
    const report = reports[3];
    assert.equal(report.outcome, 'normal', `${opt}/${index}: ${report.diagnostic}`);
    assert.equal(report.native_trap, null);
    assert.deepEqual(report.request_trace, []);
    assert.match(report.module?.wasm_sha256, /^[0-9a-f]{64}$/);
    assert.ok(Array.isArray(report.module.imports));
    assert.ok(report.module.imports.every(entry => entry.kind !== 'function'),
      'pure calculator module imported a host callback');
    const result = report.stack.at(-1);
    assert.equal(result.type, 'Nominal');
    assert.equal(result.module, reports[0].resolved_module.identity);
    assert.equal(result.ordinal, 0);
    assert.equal(result.variant, expected === 'InvalidDigit' || expected === 'OutOfRange' ? 'right' : 'left');
    if (/^[0-9]/.test(expected)) {
      assert.equal(result.value.type, 'I64');
      assert.equal(result.value.value, expected);
    } else {
      assert.equal(result.value.module, reports[2].resolved_module.identity);
      assert.equal(result.value.ordinal, 0);
      assert.equal(result.value.variant, expected === 'InvalidDigit' ? 'left' : 'right');
    }
  }
  const changed = run(`digit-version-${opt}`, ['session', ...flags, '--opt', opt],
    Buffer.concat([resultModule, digitModule, foldModule, laterDigitModule,
      Buffer.from('"17" 0 0 decimal_fold@1.fold')].map(frame)));
  assert.equal(changed.status, 0);
  assert.equal(changed.reports.length, 5);
  const later = changed.reports[3];
  const rebound = changed.reports[4];
  assert.equal(later.outcome, 'linked');
  assert.notEqual(later.resolved_module.identity, changed.reports[1].resolved_module.identity);
  assert.equal(rebound.outcome, 'normal');
  assert.equal(rebound.stack.at(-1).variant, 'left');
  assert.equal(rebound.stack.at(-1).value.value, '17',
    'compiled fold rebound its exact decimal_digit@1 callee to later @2');
  assert.deepEqual(rebound.request_trace, []);
  assert.ok(rebound.module.imports.every(entry => entry.kind !== 'function'));
  const cursor = [
    ['', 0, -1], ['é', 0, 195], ['é', 1, 169], ['é', 2, -1],
    ['é', -1, -2], ['é', 3, -3], ['7', 0, 55], ['7', 1, -1],
  ];
  for (const [index, [text, offset, value]] of cursor.entries()) {
    const byteRun = run(`cursor-${opt}-${index}`, ['session', ...flags, '--opt', opt],
      frame(Buffer.from(`${JSON.stringify(text)} ${offset} text.byte`)));
    assert.equal(byteRun.status, 0);
    assert.equal(byteRun.reports.length, 1);
    const report = byteRun.reports[0];
    assert.equal(report.profile, 'Signed-Declared-Text-v1');
    assert.equal(report.outcome, 'normal');
    assert.equal(report.host_requests, 0);
    assert.equal(report.guest_requests, 0);
    assert.equal(report.native_trap, null);
    assert.deepEqual(report.stack.map(item => [item.type, item.value]),
      [['Text', text], ['I64', String(value)]]);
    assert.ok(report.module.imports.every(entry => entry.kind !== 'function'));
  }
  for (const [index, [left, right, expected]] of [
    ['-1', '0', true],
    ['0', '-1', false],
    ['-9223372036854775808', '9223372036854775807', true],
    ['9223372036854775807', '-9223372036854775808', false],
    ['-1', '-1', false],
  ].entries()) {
    const comparison = run(`less-${opt}-${index}`, ['session', ...flags, '--opt', opt],
      frame(Buffer.from(`${left} ${right} <`)));
    assert.equal(comparison.status, 0);
    assert.equal(comparison.reports.length, 1);
    const report = comparison.reports[0];
    assert.equal(report.profile, 'Signed-Declared-Text-v1');
    assert.equal(report.outcome, 'normal', `signed comparison ${left} < ${right}`);
    assert.deepEqual(report.stack.map(({ type, value }) => ({ type, value })),
      [{ type: 'Bool', value: expected }]);
    assert.deepEqual(report.request_trace, []);
    assert.ok(report.module.imports.every(entry => entry.kind !== 'function'));
    assert.equal(report.native_trap, null);
    const wat = fs.readFileSync(path.join(output, `less-${opt}-${index}.artifacts`,
      'engine', `${report.module.stem}.wat`), 'utf8');
    assert.match(wat, /\bi64\.lt_s\b/);
  }
}
for (const opt of ['off', 'on']) {
  const owner = Buffer.from(
    'module countdown@1 [ signature go forall<S:stack> [ S I64 -- S I64 ! pure ] export go def go [ dup [ let n n 0 = ] dip swap [ ] [ 1 - go ] if ] ]');
  const observed = run(`signed-tail-${opt}-lexical-over-512`,
    ['session', ...flags, '--opt', opt],
    Buffer.concat([owner, Buffer.from('600 countdown@1.go')].map(frame)));
  assert.equal(observed.status, 0, `signed lexical tail recursion/${opt}`);
  assert.deepEqual(observed.reports.map(report => report.outcome), ['linked', 'normal']);
  assert.deepEqual(observed.reports[1].stack.map(({ type, value }) => ({ type, value })),
    [{ type: 'I64', value: '0' }]);
  assert.ok(observed.reports[1].metrics.execution_steps > 512,
    'guest must perform more than 512 real checked recursive steps');
  assert.ok(observed.reports[1].metrics.continuation_peak_bytes <= 512,
    'signed tail self must not retain one continuation per recursive step');
  for (const report of observed.reports) {
    assert.equal(report.host_requests, 0);
    assert.equal(report.guest_requests, 0);
    assert.equal(report.acquired_authority, false);
    if (report.module) {
      assert.equal(report.native_trap, null);
      assert.ok(report.module.imports.every(entry => entry.kind !== 'function'));
    }
  }
}
for (const opt of ['off', 'on']) {
  for (const [name, source, expected] of [
    ['add', '5 6 local_math@1.add', '11'],
    ['stack-add', '5 6 local_math@1.stack_add', '11'],
    ['shadow-global', '5 local_math@1.shadow_global', '5'],
    ['capture', '5 local_math@1.capture', '5'],
    ['capture-pair', '5 6 local_math@1.capture_pair', '11'],
    ['ordered-three', '1 2 3 local_math@1.ordered_three', '6'],
    ['shadow', '5 local_math@1.shadow', '2'],
    ['root', '1 let x 2 let y x y +', '3'],
  ]) {
    const input = Buffer.concat([localModule, Buffer.from(source)].map(frame));
    const result = run(`local-${opt}-${name}`, ['session', ...localFlags, '--opt', opt], input);
    assert.equal(result.status, 0, `local ${name}/${opt}`);
    assert.deepEqual(result.reports.map(item => item.outcome), ['linked', 'normal']);
    for (const report of result.reports) {
      assert.equal(report.profile, 'Declared-Text-v1');
      assert.equal(report.host_requests, 0);
      assert.equal(report.guest_requests, 0);
      assert.equal(report.acquired_authority, false);
    }
    const execution = result.reports[1];
    assert.deepEqual(execution.stack.map(({ type, value }) => ({ type, value })),
      [{ type: 'I64', value: expected }]);
    assert.deepEqual(execution.request_trace, []);
    assert.equal(execution.native_trap, null);
    assert.ok(execution.module.imports.every(entry => entry.kind !== 'function'));
  }
  const reflected = run(`local-${opt}-capture-recipe`,
    ['session', ...localFlags, '--opt', opt],
    Buffer.concat([localModule, Buffer.from('5 let value [ value ] reflect')].map(frame)));
  assert.equal(reflected.status, 0, `lexical capture reflection/${opt}`);
  assert.deepEqual(reflected.reports.map(item => item.outcome), ['linked', 'normal']);
  assert.equal(reflected.reports[1].stack.length, 1);
  const syntax = reflected.reports[1].stack[0];
  assert.equal(syntax.type, 'Syntax');
  assert.deepEqual(syntax.recipe[0], { literal: { type: 'I64', value: '5' } });
  assert.ok(syntax.recipe.some(entry => entry.invoke === 'builtin:dup')
    && syntax.recipe.some(entry => entry.invoke === 'builtin:drop'),
  'capture recipe must show the checked local projection and scope discard');
  assert.equal(reflected.reports[1].host_requests, 0);
  assert.deepEqual(reflected.reports[1].request_trace, []);
  const selectedRecipe = run(`local-${opt}-selected-builtin-recipe`,
    ['session', ...localFlags, '--opt', opt],
    Buffer.concat([localModule,
      Buffer.from('5 let x [ x -1 < ] reflect [ "x" 0 text.byte ] reflect')].map(frame)));
  assert.equal(selectedRecipe.status, 0, `combined-profile builtin reflection/${opt}`);
  const reflectedStack = selectedRecipe.reports.at(-1).stack;
  assert.deepEqual(reflectedStack.map(entry => entry.type), ['Syntax', 'Syntax']);
  assert.match(JSON.stringify(reflectedStack[0].recipe), /"invoke":"builtin:<"/);
  assert.match(JSON.stringify(reflectedStack[1].recipe), /"invoke":"builtin:text.byte"/);
  assert.equal(selectedRecipe.reports.at(-1).host_requests, 0);
  const failed = run(`local-recover-${opt}`, ['session', ...localFlags, '--opt', opt],
    Buffer.concat([Buffer.from('1 let x unknown'), Buffer.from('1 let x x')].map(frame)));
  assert.notEqual(failed.status, 0);
  assert.deepEqual(failed.reports.map(item => item.outcome), ['unbound-word', 'normal']);
  assert.equal(failed.reports[0].prior_stack, 'unchanged');
  assert.equal(failed.reports[0].prior_namespace, 'unchanged');
  assert.equal(failed.reports[1].stack.at(-1).value, '1');
}
for (const opt of ['off', 'on']) {
  const first = Buffer.from('module m@1 [ export addone def addone [ 1 + ] ]');
  const second = Buffer.from('module m@2 [ export addone def addone [ 2 + ] ]');
  const source = [first, second, Buffer.from('import m@1 as m'),
    Buffer.from('import m@1 as other'),
    Buffer.from('def once [ let x x m.addone ]'), Buffer.from('def twice [ once once ]'),
    Buffer.from('[ once ]'), Buffer.from('import m@2 as m'),
    Buffer.from('20 swap run'), Buffer.from('drop'),
    Buffer.from('20 other.addone'), Buffer.from('drop'), Buffer.from('20 twice')];
  const observed = run(`rebind-${opt}-retained-old-and-current-new`,
    ['session', ...localFlags, '--opt', opt], Buffer.concat(source.map(frame)));
  assert.equal(observed.status, 0, `checked transitive import rebind/${opt}`);
  assert.deepEqual(observed.reports.map(report => report.outcome),
    ['linked', 'linked', 'linked', 'linked', 'defined', 'defined',
      'normal', 'linked', 'normal', 'normal', 'normal', 'normal', 'normal']);
  assert.equal(observed.reports[8].stack.at(-1).value, '21',
    'old captured Program must retain old immutable once owner');
  assert.deepEqual(observed.reports[9].stack, []);
  assert.equal(observed.reports[10].stack.at(-1).value, '21',
    'unchanged second alias must retain its own old immutable module snapshot');
  assert.deepEqual(observed.reports[11].stack, []);
  assert.equal(observed.reports[12].stack.at(-1).value, '24',
    'new twice must transitively invoke newly checked once owner');
  for (const report of observed.reports) {
    assert.equal(report.host_requests, 0);
    assert.equal(report.guest_requests, 0);
    assert.equal(report.acquired_authority, false);
    if (report.module) {
      assert.equal(report.native_trap, null);
      assert.ok(report.module.imports.every(entry => entry.kind !== 'function'));
    }
  }
  const incompatible = run(`rebind-${opt}-incompatible-then-recover`,
    ['session', ...localFlags, '--opt', opt],
    Buffer.concat([first,
      Buffer.from('module m@2 [ export addone def addone [ drop true ] ]'),
      Buffer.from('import m@1 as m'), Buffer.from('def once [ m.addone ]'),
      Buffer.from('import m@2 as m'), Buffer.from('20 once')].map(frame)));
  assert.notEqual(incompatible.status, 0);
  assert.deepEqual(incompatible.reports.slice(0, 4).map(report => report.outcome),
    ['linked', 'linked', 'linked', 'defined']);
  assert.ok(['check', 'acceptance'].includes(incompatible.reports[4].stage));
  assert.equal(incompatible.reports[4].prior_namespace, 'unchanged');
  assert.equal(incompatible.reports[5].outcome, 'normal');
  assert.equal(incompatible.reports[5].stack.at(-1).value, '21');
}
for (const opt of ['off', 'on']) {
  const source = [resultModule, digitModule, foldModule,
    Buffer.from('"0" 0 0 decimal_fold@1.fold'),
    Buffer.from('drop "9" 0 0 decimal_fold@1.fold'),
    Buffer.from('drop "1a" 0 0 decimal_fold@1.fold'),
    Buffer.from('drop "12" 0 0 decimal_fold@1.fold')];
  const repeated = run(`same-session-${opt}`, ['session', ...flags, '--opt', opt],
    Buffer.concat(source.map(frame)));
  assert.equal(repeated.status, 0);
  assert.deepEqual(repeated.reports.slice(0, 3).map(report => report.outcome),
    ['linked', 'linked', 'linked']);
  assert.equal(repeated.reports.length, 7);
  for (const [index, expected] of ['0', '9', 'InvalidDigit', '12'].entries()) {
    const report = repeated.reports[index + 3];
    assert.equal(report.outcome, 'normal', `same-session-${opt}/${index}`);
    assert.equal(report.host_requests, 0);
    assert.equal(report.guest_requests, 0);
    assert.deepEqual(report.request_trace, []);
    assert.equal(report.native_trap, null);
    assert.ok(report.module.imports.every(entry => entry.kind !== 'function'));
    const value = report.stack.at(-1);
    assert.equal(value.module, repeated.reports[0].resolved_module.identity);
    assert.equal(value.variant, expected === 'InvalidDigit' ? 'right' : 'left');
    if (expected === 'InvalidDigit') {
      assert.equal(value.value.module, repeated.reports[2].resolved_module.identity);
      assert.equal(value.value.variant, 'left');
    } else {
      assert.equal(value.value.value, expected);
    }
  }
  const rejectedSource = [resultModule, digitModule, foldModule,
    Buffer.from('"0" 0 0 decimal_fold@1.fold'),
    Buffer.from('drop absent@1.fold'),
    Buffer.from('drop "12" 0 0 decimal_fold@1.fold')];
  const recovered = run(`recover-source-${opt}`, ['session', ...flags, '--opt', opt],
    Buffer.concat(rejectedSource.map(frame)));
  assert.notEqual(recovered.status, 0, 'source refusal must remain a visible process failure');
  assert.equal(recovered.reports.length, 6);
  const refusal = recovered.reports[4];
  assert.equal(refusal.outcome, 'unbound-word');
  assert.equal(refusal.stage, 'resolve');
  assert.equal(refusal.prior_stack, 'unchanged');
  assert.equal(refusal.prior_namespace, 'unchanged');
  assert.equal(refusal.host_requests, 0);
  assert.equal(refusal.guest_requests, 0);
  const after = recovered.reports[5];
  assert.equal(after.outcome, 'normal', `post-refusal valid ${opt}`);
  assert.equal(after.stack.at(-1).value.value, '12');
  assert.equal(after.host_requests, 0);
  assert.deepEqual(after.request_trace, []);
  assert.ok(after.module.imports.every(entry => entry.kind !== 'function'));
}
const negative = [
  ['no-signed', ['--declared-modules', '--text-byte-cursor']],
  ['ordinary-declared', ['--declared-modules', '--bindings', '/dev/null']],
  ['cursor-declared', ['--declared-modules', '--signed-self-recursion',
    '--text-byte-cursor', '--bindings', '/dev/null']],
  ['duplicate-cursor', [...flags, '--text-byte-cursor']],
  ['no-declared', ['--text-byte-cursor', '--signed-self-recursion']],
  ['ordinary-less', ['--declared-modules', '--bindings', '/dev/null'],
    frame(Buffer.from('-1 0 <'))],
  ['standalone-cursor-less', ['--text-byte-cursor'],
    frame(Buffer.from('-1 0 <'))],
  ['ordinary-locals', ['--declared-modules', '--bindings', '/dev/null'],
    frame(localModule)],
  ['duplicate-local', localFlags.filter(flag => flag !== '--framed'),
    frame(Buffer.from('module invalid@1 [ signature f forall<S:stack> [ S I64 I64 -- S I64 ! pure ] export f def f [ let x let x x ] ]'))],
  ['missing-local-name', localFlags.filter(flag => flag !== '--framed'),
    frame(Buffer.from('1 let'))],
];
const plainCursorModule = Buffer.from('module cursor@1 [ signature read forall<S:stack> [ S Text I64 -- S Text I64 ! pure ] export read def read [ text.byte ] ]');
const withoutFramed = flags.filter(flag => flag !== '--framed');
negative.push(
  ['unsigned-fold', withoutFramed, Buffer.concat([
    frame(resultModule),
    frame(digitModule),
    frame(Buffer.from(foldModule.toString('utf8').replace(
      'signature fold forall<S:stack>', 'signature other forall<S:stack>'))),
  ])],
  ['wrong-digit-version', withoutFramed, Buffer.concat([
    frame(resultModule), frame(digitModule),
    frame(Buffer.from(foldModule.toString('utf8').replace(
      'decimal_digit@1.digit', 'decimal_digit@2.digit'))),
  ])],
  ['duplicate-digit-owner', withoutFramed, Buffer.concat([
    frame(resultModule), frame(digitModule), frame(foldModule), frame(digitModule),
  ])],
);
for (const [name, args, source] of negative) {
  const reports = run(name, ['session', '--framed', ...args],
    source ?? frame(name === 'ordinary-declared' ? plainCursorModule : foldModule));
  assert.notEqual(reports.status, 0, `${name} must reject before executing`);
  const refusal = reports.reports.at(-1);
  if (refusal.stage === 'arguments') {
    assert.equal(refusal.outcome, 'invalid-input');
    assert.equal(refusal.prior_session, 'unchanged');
  } else if (name === 'standalone-cursor-less') {
    assert.equal(refusal.stage, 'resolve');
    assert.equal(refusal.outcome, 'unbound-word');
    assert.equal(refusal.prior_stack, 'unchanged');
  } else {
    assert.equal(refusal.host_requests, 0);
  }
  assert.equal(refusal.guest_requests, 0);
  if (name === 'ordinary-declared') {
    assert.equal(refusal.stage, 'resolve');
    assert.equal(refusal.outcome, 'unbound-word');
    assert.equal(refusal.prior_namespace, 'unchanged');
  }
  if (['ordinary-locals', 'duplicate-local', 'missing-local-name'].includes(name)) {
    assert.equal(refusal.stage, 'resolve');
    assert.equal(refusal.prior_namespace, 'unchanged');
  }
  if (name === 'unsigned-fold' || name === 'wrong-digit-version' ||
      name === 'duplicate-digit-owner') {
    assert.equal(refusal.prior_namespace, 'unchanged');
  }
}
fs.writeFileSync(path.join(output, 'run.json'), `${JSON.stringify(record, null, 2)}\n`, { flag: 'wx' });
console.log(JSON.stringify({ outcome: 'wrapping-I64-precursor-observed', runs: record.runs.length,
  binary_sha256: expectedDigest, raw: output, exact_bigint: false }));
