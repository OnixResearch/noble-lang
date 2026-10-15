// Issue #15 Phase 3 smallest end-to-end slice: one arithmetic-plus-composition
// subject, `[ quote [ 1 + ] compose run ]`, chained from its Lean
// specification (proofs/foundation, NobleFoundation.Slice) to the exact Wasm
// bytes that `noble admit-artifact` instantiates.
//
// This joins the two existing routes that #10's PAA-1 design composes:
//   Route B  `noble build --require-proof` (checked evidence bound to source)
//   Route A  `noble admit-artifact`        (host rebuild + final-byte equality)
// It is NOT the PAA-1 envelope implementation, and it is NOT translation
// validation. Every edge without a proof is labelled in the receipt.
//
// Usage (selected Node, empty environment except XDG_RUNTIME_DIR for the
// sandboxed Lean consumer):
//   env -i XDG_RUNTIME_DIR=$XDG_RUNTIME_DIR NODE verification/foundation/slice.mjs \
//     --foundation FOUNDATION_CHECK_JSON --output NEW_DIR [--allow-dirty]
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, readdirSync, readFileSync, realpathSync, writeFileSync } from 'node:fs';
import { userInfo } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(fileURLToPath(import.meta.url), '../../..');
const sliceDir = join(root, 'verification/foundation/slice');
const project = join(root, 'proofs/foundation');
const selection = JSON.parse(readFileSync(join(root, 'policy/tool-selection.json')));
const leanBin = join(selection.tool_paths.lean.output, 'bin');
const rustBin = join(selection.tool_paths.quality_rust.output, 'bin');
const node = join(selection.tool_paths.node.output, 'bin/node');
const wasmTools = join(selection.tool_paths.wasm_tools.output, 'bin/wasm-tools');
const git = realpathSync('/run/current-system/sw/bin/git');
const { homedir: home, username } = userInfo();
// Sandbox inputs of the Route B consumer (verification/mc2/gate.mjs `reviewed`).
// The reviewed bubblewrap 0.11.2 path is used when present; otherwise the
// fallback is recorded as a deviation in the receipt.
const reviewedBwrap = '/nix/store/lqndphylsxqwbwm804n473pb4sqb98sh-bubblewrap-0.11.2/bin/bwrap';
const fallbackBwrap = '/nix/store/y0ra9qr3rz81d9wl7dfrldv3j25dc98q-bubblewrap-0.12.0/bin/bwrap';
const consumer = {
  NOBLE_LEAN: join(leanBin, 'lean'),
  NOBLE_BWRAP: existsSync(reviewedBwrap) ? reviewedBwrap : fallbackBwrap,
  NOBLE_PRLIMIT: '/nix/store/mqvbf0flamqaq9c3ihb496aahag897n0-util-linux-2.42.3-bin/bin/prlimit',
  NOBLE_SYSTEMD_RUN: '/nix/store/baxgs06659w4i6ggsfr56srjrb52v1h8-systemd-261.2/bin/systemd-run',
  NOBLE_CONTRACT_LIBRARY: join(root, 'proofs/mc1'),
};
const childBase = Object.freeze({
  HOME: home, USER: username, LOGNAME: username, LANG: 'C.UTF-8', CI: '1', GIT_TERMINAL_PROMPT: '0',
  PATH: [leanBin, rustBin, dirname(git), `/etc/profiles/per-user/${username}/bin`,
    '/run/current-system/sw/bin', '/usr/bin', '/bin'].join(':'),
});
const sliceTheorems = ['NobleFoundation.slice_body_exec', 'NobleFoundation.slice_body_signed',
  'NobleFoundation.slice_root_inert', 'NobleFoundation.slice_driver', 'NobleFoundation.slice_body_typed',
  'NobleFoundation.slice_exported_claim', 'NobleFoundation.slice_examples',
  'NobleFoundation.helper_refines_step', 'NobleFoundation.helper_fails_when_stuck',
  'NobleFoundation.swapped_helper_refuted', 'NobleFoundation.sub_operand_order'];

const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const fileHash = path => digest(readFileSync(path));
const requireThat = (condition, message) => { if (!condition) throw Error(message); };
const commands = [];
function run(command, args, options = {}) {
  const result = spawnSync(command, args, { encoding: 'utf8', maxBuffer: 64 * 1024 * 1024,
    timeout: 1800000, env: childBase, ...options });
  commands.push({ command, args, exit: result.status });
  if (result.error || (result.status !== 0 && !options.allowFailure)) {
    throw Error(`${command} ${args.join(' ')} failed: ${result.error?.message ?? result.status}\n${result.stderr}\n${result.stdout}`);
  }
  return options.allowFailure ? result : result.stdout.trim();
}
const json = text => JSON.parse(text);

const args = process.argv.slice(2);
const allowDirty = args.includes('--allow-dirty');
const option = name => { const i = args.indexOf(name); return i >= 0 ? args[i + 1] : undefined; };
const foundationPath = option('--foundation');
const outputArg = option('--output');
requireThat(foundationPath && outputArg,
  'usage: slice.mjs --foundation FOUNDATION_CHECK_JSON --output NEW_DIR [--allow-dirty]');
const forbidden = Object.keys(process.env).filter(key =>
  /^(?:LEAN(?:_|$)|LAKE(?:_|$)|NODE(?:_|$)|BASH(?:_|$)|ENV$|LD_|DYLD_|CARGO_|RUST|GIT_|NIX_|NOBLE_)/.test(key)
  && process.env[key] !== '');
if (forbidden.length) {
  console.error(`[slice] FAIL unsafe caller environment: ${forbidden.sort().join(', ')}`);
  process.exit(1);
}
requireThat(process.env.XDG_RUNTIME_DIR, 'XDG_RUNTIME_DIR is required by the sandboxed Lean consumer');
const out = resolve(outputArg);
mkdirSync(out);

/** Text of one top-level `(func $name …)` in a runtime helper file. */
function helperFunction(text, name) {
  const start = text.indexOf(`(func ${name} `) >= 0 ? text.indexOf(`(func ${name} `) : text.indexOf(`(func ${name}\n`);
  requireThat(start >= 0, `helper ${name} not found`);
  const next = text.indexOf('\n(func ', start + 1);
  return text.slice(start, next < 0 ? text.length : next + 1);
}
/** Pinned transcription check (refusal: helper-transcription-stale). */
function checkTranscription(read) {
  const pin = json(readFileSync(join(sliceDir, 'helper-transcription.json')));
  const observed = {};
  for (const [file, names] of Object.entries(pin.files)) {
    const text = read(file);
    for (const [name, expected] of Object.entries(names)) {
      const actual = digest(helperFunction(text, name));
      observed[`${file}#${name}`] = actual;
      if (actual !== expected) return { accepted: false, refusal: 'helper-transcription-stale', at: `${file}#${name}`, observed };
    }
  }
  return { accepted: true, observed };
}
/** Stale-proof-subject check of a foundation-check receipt (refusal: stale-proof-subject). */
function checkFoundation(receipt, tree) {
  if (receipt.schema !== 'noble-foundation-check/v1') return { accepted: false, refusal: 'invalid-foundation-receipt' };
  if (receipt.source_tree !== tree) return { accepted: false, refusal: 'stale-proof-subject' };
  if (!allowDirty && receipt.clean_source !== true) return { accepted: false, refusal: 'dirty-proof-subject' };
  for (const [path, hash] of Object.entries(receipt.sources_sha256)) {
    if (fileHash(join(root, path)) !== hash) return { accepted: false, refusal: 'stale-proof-subject', at: path };
  }
  const allowed = new Set(receipt.allowed_axioms);
  for (const name of sliceTheorems) {
    const theorem = receipt.theorems.find(t => t.name === name);
    if (!theorem || theorem.status !== 'proved' || theorem.axioms.some(a => !allowed.has(a))) {
      return { accepted: false, refusal: 'missing-theorem', at: name };
    }
  }
  return { accepted: true };
}
const wrap = value => BigInt.asIntN(64, value);

const receipt = { schema: 'noble-foundation-slice/v1', subject: '[ quote [ 1 + ] compose run ]',
  profile: 'MC2-pure-quotation@1 shape (one closed quotation root, no definitions/resources)',
  route: 'Route B (build --require-proof) + Route A (admit-artifact); PAA-1 envelope not implemented',
  not_translation_validation: true, universal_backend_proof: false };
try {
  requireThat(realpathSync(process.execPath) === realpathSync(node), `use selected Node: ${node}`);
  const gitState = cwd => ({
    head: run(git, ['rev-parse', 'HEAD'], { cwd }),
    tree: run(git, ['rev-parse', 'HEAD^{tree}'], { cwd }),
    status: run(git, ['status', '--porcelain=v1', '--untracked-files=all'], { cwd }),
  });
  const initial = gitState(root);
  requireThat(allowDirty || initial.status === '', `Noble source worktree dirty: ${initial.status}`);
  Object.assign(receipt, { source_commit: initial.head, source_tree: initial.tree, clean_source: initial.status === '' });

  // E1 + stale proof subject: the foundation receipt must be for this tree.
  const foundation = json(readFileSync(foundationPath));
  const fcheck = checkFoundation(foundation, initial.tree);
  requireThat(fcheck.accepted, `foundation receipt refused: ${fcheck.refusal} ${fcheck.at ?? ''}`);
  receipt.foundation = { receipt_sha256: fileHash(foundationPath), lean: foundation.lean,
    mathlib_commit: foundation.mathlib_commit,
    theorems: foundation.theorems.filter(t => sliceTheorems.includes(t.name))
      .map(({ name, statement, axioms }) => ({ name, statement, axioms })) };

  // Tools and helpers (PAA-1 `toolchain` field, recorded; pins compared).
  const env = { ...childBase, TMPDIR: out, CARGO_HOME: join(home, '.cargo'), RUSTC_WRAPPER: '',
    CARGO_TARGET_DIR: join(root, 'target') };
  run(join(rustBin, 'cargo'), ['build', '--locked', '--offline', '-j', '4', '-p', 'noble-cli'],
    { cwd: root, env, timeout: 3600000 });
  const cli = join(root, 'target/debug/noble');
  const config = json(readFileSync(join(root, 'crates/noble-cli/src/core/runtime/config.json')));
  const pinned = { node, wasm_tools: wasmTools, wasm_opt: join(selection.tool_paths.binaryen.output, 'bin/wasm-opt') };
  const runtimeDir = join(root, 'crates/noble-wasm/runtime');
  receipt.toolchain = {
    cli_sha256: fileHash(cli),
    tools: Object.fromEntries(Object.entries(pinned).map(([k, p]) => [k, { path: p, sha256: fileHash(p) }])),
    runtime_helpers_sha256: Object.fromEntries(readdirSync(runtimeDir).filter(f => f.endsWith('.wat')).sort()
      .map(f => [f, fileHash(join(runtimeDir, f))])),
    consumer_sandbox: Object.fromEntries(Object.entries(consumer).map(([k, p]) => [k, p])),
    consumer_deviation: consumer.NOBLE_BWRAP === reviewedBwrap ? null
      : `reviewed bubblewrap 0.11.2 absent; used ${consumer.NOBLE_BWRAP}`,
    optimizer_flags: config.optimizer ?? config.tools?.wasm_opt ?? null,
  };
  const transcription = checkTranscription(file => readFileSync(join(root, file), 'utf8'));
  requireThat(transcription.accepted, `runtime helper changed under the Lean transcription: ${transcription.at}`);
  receipt.helper_transcription = transcription.observed;

  // E2: frozen law -> regenerated MC1 statement (exact bytes).
  const contract = join(sliceDir, 'compose-increment.contract');
  const explained = json(run(cli, ['explain-proof', contract], { cwd: root, env }));
  const expectedStatement = readFileSync(join(sliceDir, 'expected-statement.lean'), 'utf8');
  requireThat(explained.generated_statement === expectedStatement, 'law-mismatch: regenerated statement differs from frozen law');
  receipt.law = { contract_sha256: fileHash(contract), statement_sha256: digest(expectedStatement) };

  // E3: the regenerated statement's program IS the foundation subject, and the
  // Mathlib foundation proves the regenerated claim (kernel-checked here).
  const imports = expectedStatement.split('\n').filter(l => l.startsWith('import '));
  const body = expectedStatement.split('\n').filter(l => !l.startsWith('import ')).join('\n');
  const bridge = join(out, 'Bridge.lean');
  writeFileSync(bridge, ['import NobleFoundation.Slice', ...imports, body,
    'open NobleContracts in',
    'theorem bridge_program : MC1Obligation.program = NobleFoundation.sliceBody := rfl',
    'theorem bridge_interface : MC1Obligation.inputTypes = [.i64] ∧ MC1Obligation.outputTypes = [.i64] ∧',
    '    MC1Obligation.paramTypes = [] := ⟨rfl, rfl, rfl⟩',
    'open NobleContracts in',
    'theorem bridge_claim : MC1Obligation.claim := by',
    '  intro tail before params hi hp hpre final he',
    '  obtain ⟨n, rfl⟩ := (stackTyped_i64_iff before).mp hi',
    '  refine ⟨[.i64 (n + 1)], (NobleFoundation.slice_body_exec tail n final).mp he, .cons .i64 .nil, ?_⟩',
    '  simp [Holds, evaluate, MC1Obligation.postcondition, MC1Obligation.expression_1,',
    '    MC1Obligation.expression_2, MC1Obligation.expression_3, MC1Obligation.expression_4,',
    '    MC1Obligation.expression_5]',
    '#print axioms bridge_claim', ''].join('\n'));
  const lake = join(leanBin, 'lake');
  const bridgeOut = run(lake, ['env', 'lean', bridge], { cwd: project, env: { ...childBase, TMPDIR: out } });
  requireThat(/bridge_claim' depends on axioms: \[propext, Classical.choice, Quot.sound\]|bridge_claim' depends on axioms: \[[A-Za-z., ]*\]/.test(bridgeOut)
    && !/sorryAx|ofReduceBool/.test(bridgeOut), `bridge axioms: ${bridgeOut}`);
  receipt.bridge = { lean_sha256: fileHash(bridge), axioms: bridgeOut };

  // E4: Route B — independent sandboxed recheck of the self-contained MC1
  // proof, bound by the core to the exact subject source.
  const subject = join(sliceDir, 'subject.noble');
  const proof = join(sliceDir, 'compose-increment.proof.lean');
  const routeB = (source, label) => {
    const dest = join(out, `route-b-${label}`);
    const result = run(cli, ['build', '--require-proof', source, '--contract', contract, '--proof', proof,
      '--timeout-ms', '600000', '--out', dest], { cwd: root, env: { ...env, ...consumer,
      XDG_RUNTIME_DIR: process.env.XDG_RUNTIME_DIR }, allowFailure: true });
    const report = json(readFileSync(join(dest, 'report.json')));
    return { exit: result.status, outcome: report.outcome, allowed: report.release?.allowed,
      reason: report.release?.reason, subject_identity: report.release?.core_subject_identity,
      evidence: report.evidence, refusals: report.release?.refusals, diagnostics: report.diagnostics };
  };
  const proved = routeB(subject, 'subject');
  requireThat(proved.outcome === 'proved' && proved.allowed === true && proved.evidence?.independent_recheck === true,
    `Route B did not release the subject: ${JSON.stringify(proved)}`);
  receipt.route_b = proved;

  // E5-E9: Route A per optimization mode — consumer rebuild, final-byte
  // equality, instantiation of the compared buffer.
  const empty = join(out, 'claims-empty.json');
  writeFileSync(empty, JSON.stringify({ claimed_effects: [] }));
  const emit = (source, mode, label) => {
    const dir = join(out, `${label}-${mode}`);
    const report = json(run(cli, ['run', source, '--opt', mode, '--emit', dir], { cwd: root, env }));
    const file = join(dir, 'engine', mode === 'on' ? 'module-1-optimized.wasm' : 'module-1.wasm');
    requireThat(report.module?.wasm_sha256 === fileHash(file), `${label}/${mode}: emitted bytes do not match report`);
    return { report, file, dir };
  };
  const admit = (candidate, source, mode) => {
    const result = run(cli, ['admit-artifact', candidate, '--effects', empty, '--source', source, '--opt', mode],
      { cwd: root, env, allowFailure: true });
    return { exit: result.status, report: json(result.stdout) };
  };
  receipt.route_a = {};
  for (const mode of ['off', 'on']) {
    const candidate = emit(subject, mode, 'subject');
    const admitted = admit(candidate.file, subject, mode);
    const r = admitted.report;
    requireThat(r.outcome === 'normal' && r.admission?.correspondence === 'byte-exact' &&
      r.module?.wasm_sha256 === fileHash(candidate.file) && r.module?.optimized === (mode === 'on') &&
      r.guest_requests === 0 && r.stack?.length === 1 && r.stack[0].type === 'Program',
    `Route A ${mode}: subject not admitted: ${JSON.stringify(r).slice(0, 400)}`);
    const recipe = r.stack[0].recipe.map(step => step.invoke ?? (step.quotation ? 'quotation' : '?'));
    requireThat(JSON.stringify(recipe) === JSON.stringify(['builtin:quote', 'quotation', 'builtin:compose', 'builtin:run']) &&
      JSON.stringify(r.stack[0].recipe[1].quotation) === JSON.stringify([{ literal: { type: 'I64', value: '1' } }, { invoke: 'builtin:i64.add' }]),
    `Route A ${mode}: reflected recipe differs from sliceBody`);
    receipt.route_a[mode] = { wasm_sha256: r.module.wasm_sha256, wasm_bytes: readFileSync(candidate.file).length,
      wat_sha256: r.module.wat_sha256, source_sha256: r.module.source_sha256, correspondence: r.admission.correspondence,
      interface: r.stack[0].interface, recipe };
  }

  // E11: executions through admission. Expected values are computed with
  // BigInt and then kernel-checked against the Mathlib oracle by Lean.
  const drivers = [
    { name: 'driver-41', prefix: [], x: 41n },
    { name: 'driver-max', prefix: [], x: 9223372036854775807n },
    { name: 'driver-min', prefix: [], x: -9223372036854775808n },
    { name: 'driver-prefix', prefix: [['Bool', 'true'], ['I64', '-3']], x: 0n },
  ];
  const expectedLean = join(out, 'Expected.lean');
  writeFileSync(expectedLean, ['import MathlibI64', ...drivers.map(d =>
    `example : MathlibI64.wrap ((${d.x} : ℤ) + 1) = ${wrap(d.x + 1n)} := by decide`), ''].join('\n'));
  run(lake, ['env', 'lean', expectedLean], { cwd: project, env: { ...childBase, TMPDIR: out } });
  receipt.executions = [];
  for (const d of drivers) {
    const text = [...d.prefix.map(([, v]) => v), String(d.x), '[ quote [ 1 + ] compose run ] run'].join(' ') + '\n';
    const source = join(out, `${d.name}.noble`);
    writeFileSync(source, text);
    for (const mode of ['off', 'on']) {
      const candidate = emit(source, mode, d.name);
      const r = admit(candidate.file, source, mode).report;
      const stack = (r.stack ?? []).map(v => [v.type, String(v.value)]);
      const expected = [...d.prefix, ['I64', String(wrap(d.x + 1n))]];
      requireThat(r.outcome === 'normal' && r.admission?.correspondence === 'byte-exact' &&
        JSON.stringify(stack) === JSON.stringify(expected) && r.request_trace?.length === 0,
      `${d.name}/${mode}: ${JSON.stringify(stack)} != ${JSON.stringify(expected)}`);
      receipt.executions.push({ name: d.name, mode, source_sha256: digest(text), wasm_sha256: r.module.wasm_sha256,
        stack, expected, lean_checked_expected: true, matched: true });
    }
  }
  receipt.expected_lean_sha256 = fileHash(expectedLean);

  // Refusal controls (ACORR-* ids from #10 where they correspond).
  const controls = [];
  const control = (id, acorr, intended, outcome, ok) => {
    controls.push({ id, acorr, intended, outcome, refused_for_intended_reason: ok });
    requireThat(ok, `control ${id} did not refuse for its intended reason: ${JSON.stringify(outcome)}`);
  };
  // Wrong body: proof-bound Route B on [ quote [ 2 + ] compose run ].
  const wrongSource = join(out, 'wrong-body.noble');
  writeFileSync(wrongSource, '[ quote [ 2 + ] compose run ]\n');
  const wrongB = routeB(wrongSource, 'wrong-body');
  control('wrong-body-proof', 'ACORR-CASE-03', 'valid proof of [1 +] law must not release [2 +] body',
    { outcome: wrongB.outcome, allowed: wrongB.allowed, reason: wrongB.reason }, wrongB.allowed !== true && wrongB.outcome !== 'proved');
  // Wrong body bytes admitted against the correct source.
  const wrongBytes = admit(emit(wrongSource, 'off', 'wrong-body').file, subject, 'off');
  control('wrong-body-bytes', 'ACORR-CASE-03', 'bytes of [2 +] body against SOURCE [1 +]', wrongBytes.report,
    wrongBytes.exit !== 0 && wrongBytes.report.outcome === 'correspondence-reject');
  // Optimization-mode mismatch.
  const optOn = emit(subject, 'on', 'opt-mismatch');
  const mismatch = admit(optOn.file, subject, 'off');
  control('opt-mode-mismatch', 'S-CASE-07 option mismatch / PAA-1 build.opt', 'opt-on bytes admitted under consumer opt off',
    mismatch.report, mismatch.exit !== 0 && mismatch.report.outcome === 'correspondence-reject');
  // Changed runtime helper with reversed operand order: $op_sub computes right - left.
  const subSource = join(out, 'sub-order.noble');
  writeFileSync(subSource, '3 8 -\n');
  const subEmit = emit(subSource, 'off', 'sub-order');
  const wat = readFileSync(join(subEmit.dir, 'engine/module-1.wat'), 'utf8');
  const original = '(i64.sub (local.get $left) (local.get $right))';
  requireThat(wat.split(original).length === 2, 'expected exactly one $op_sub body in emitted WAT');
  const swappedWat = join(out, 'sub-order-swapped-helper.wat');
  writeFileSync(swappedWat, wat.replace(original, '(i64.sub (local.get $right) (local.get $left))'));
  const swappedWasm = join(out, 'sub-order-swapped-helper.wasm');
  run(wasmTools, ['parse', swappedWat, '-o', swappedWasm], { env });
  const swapped = admit(swappedWasm, subSource, 'off');
  control('reversed-operand-helper', 'ACORR-CASE-08', '$op_sub with swapped pops (changed helper / operand order)',
    swapped.report, swapped.exit !== 0 && swapped.report.outcome === 'correspondence-reject');
  // The same helper change is refused by the Lean transcription pin.
  const mutatedTranscription = checkTranscription(file => {
    const text = readFileSync(join(root, file), 'utf8');
    return file.endsWith('operations.wat') ? text.replace(original, '(i64.sub (local.get $right) (local.get $left))') : text;
  });
  control('helper-transcription-pin', null, 'Lean helper model no longer matches the runtime helper text',
    mutatedTranscription, !mutatedTranscription.accepted && mutatedTranscription.refusal === 'helper-transcription-stale');
  // Code/recipe divergence: change only an interface/recipe data string.
  const subjectWat = readFileSync(join(out, 'subject-off/engine/module-1.wat'), 'utf8');
  const recipeData = '(data $d2 "\\5b\\49\\36\\34\\5d")';
  requireThat(subjectWat.includes(recipeData), 'expected recipe data segment $d2 "[I64]"');
  const divergentWat = join(out, 'recipe-divergence.wat');
  writeFileSync(divergentWat, subjectWat.replace(recipeData, '(data $d2 "\\5b\\49\\36\\35\\5d")'));
  const divergentWasm = join(out, 'recipe-divergence.wasm');
  run(wasmTools, ['parse', divergentWat, '-o', divergentWasm], { env });
  const divergent = admit(divergentWasm, subject, 'off');
  control('code-recipe-divergence', 'ACORR-CASE-06', 'recipe/interface data changed, code unchanged',
    divergent.report, divergent.exit !== 0 && divergent.report.outcome === 'correspondence-reject');
  // Stale proof subject: a foundation receipt for another tree.
  const stale = checkFoundation({ ...foundation, source_tree: '0'.repeat(40) }, initial.tree);
  control('stale-proof-subject', 'ACORR-CASE-04 (stale version)', 'foundation receipt from another source tree',
    stale, !stale.accepted && stale.refusal === 'stale-proof-subject');
  // Frozen-law substitution: a weaker regenerated statement must not match.
  const weakened = expectedStatement.replace('def expression_4 : Term := .add expression_2 expression_3',
    'def expression_4 : Term := expression_1');
  control('law-substitution', 'ACORR-CASE-05', 'weakened statement differs from the frozen law',
    { equal: weakened === expectedStatement }, weakened !== expectedStatement);
  controls.push({ id: 'post-check-replacement', acorr: 'ACORR-CASE-09', refused_for_intended_reason: null,
    status: 'not-executed', reason: 'admit-artifact exposes no hook between comparison and instantiation; PAA-1 load hash is not implemented' });
  receipt.controls = controls;

  const final = gitState(root);
  requireThat(final.head === initial.head && final.tree === initial.tree && final.status === initial.status,
    'Noble source worktree changed during the slice');
  receipt.sources_sha256 = Object.fromEntries([
    'verification/foundation/slice.mjs', 'verification/foundation/slice/subject.noble',
    'verification/foundation/slice/compose-increment.contract', 'verification/foundation/slice/compose-increment.proof.lean',
    'verification/foundation/slice/expected-statement.lean', 'verification/foundation/slice/helper-transcription.json',
    'proofs/foundation/NobleFoundation/Slice.lean', 'proofs/foundation/NobleFoundation/Helper.lean',
    'crates/noble-wasm/src/operations.rs', 'crates/noble-cli/src/core/runtime/config.json', 'policy/tool-selection.json',
  ].map(p => [p, fileHash(join(root, p))]));
  receipt.chain = [
    { edge: 'E1 Lean specification (MC1 model, Mathlib oracle)', status: 'proved', evidence: 'foundation receipt theorems' },
    { edge: 'E2 frozen contract law -> regenerated MC1 statement bytes', status: 'checked', evidence: 'byte equality with expected-statement.lean; exporter itself assumed (PO-16/PO-19)' },
    { edge: 'E3 regenerated statement program = NobleFoundation.sliceBody; foundation proves the regenerated claim', status: 'proved', evidence: 'Bridge.lean kernel rfl + bridge_claim' },
    { edge: 'E4 source -> subject binding with independently rechecked evidence', status: 'checked', evidence: 'Route B outcome proved; bind_artifact is trusted Rust (O6/PO-16 open)' },
    { edge: 'E5 noble-wasm lowering source -> WAT', status: 'open', evidence: 'PO-17 / PAA-1 O1; trusted compiler' },
    { edge: 'E6 runtime helpers $op_add/$op_sub/$op_mul (+ stack helpers)', status: 'proved-over-transcription', evidence: 'helper_refines_step/helper_fails_when_stuck; transcription text pinned (checked), transcription fidelity assumed; $op_quote_typed/$op_compose/$op_run open' },
    { edge: 'E7 wasm-tools assembly WAT -> Wasm', status: 'assumed', evidence: 'pinned path+hash; validation is format/type only' },
    { edge: 'E8 wasm-opt -O2 (opt on)', status: 'assumed', evidence: 'residual trusted optimizer (PAA-1 O2 open); separate per-mode receipts' },
    { edge: 'E9 candidate bytes = consumer rebuild', status: 'checked', evidence: 'admit-artifact byte-exact per mode (identity, not semantics)' },
    { edge: 'E10 instantiate compared buffer / loader / V8', status: 'assumed', evidence: 'PO-18 / PAA-1 O3 open; post-check replacement control not executed' },
    { edge: 'E11 executed observations equal Lean-checked expectations', status: 'tested', evidence: 'drivers off/on, finite' },
    { edge: 'E12 single PAA-1 envelope joining Route A and Route B', status: 'open', evidence: '#10 design only; this runner joins them over the same consumer-selected source but in an unreviewed script' },
  ];
  receipt.commands = commands.length;
  writeFileSync(join(out, 'slice-receipt.json'), JSON.stringify(receipt, null, 2) + '\n');
  console.log(JSON.stringify({ ok: true, receipt: join(out, 'slice-receipt.json'), source_commit: receipt.source_commit,
    route_a: receipt.route_a, route_b: { outcome: proved.outcome, subject_identity: proved.subject_identity },
    executions: receipt.executions.length, controls: controls.map(c => `${c.id}:${c.refused_for_intended_reason}`) }, null, 2));
} catch (error) {
  writeFileSync(join(out, 'slice-failure.json'), JSON.stringify({ ...receipt, error: error.message }, null, 2) + '\n');
  console.error(`[slice] FAIL ${error.message}`);
  process.exitCode = 1;
}
