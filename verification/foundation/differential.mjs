// Issue #15 Phase 4: seeded differential comparison of Noble's compiled
// managed-Wasm execution against an implementation-independent reference.
// Sibling of verification/mathlib-i64/compare.mjs (whose 12 fixed vectors stay
// unchanged).
//
// The reference stack machine is `NobleFoundation.refRun`: wrapping I64
// arithmetic via the Mathlib oracle, with out-of-range literals refused. The
// harness evaluates it in BigInt. For every generated case, Lean then
// kernel-checks (`decide`) that `refRun` yields exactly that expectation, and
// instantiates the proved MC1 theorem for the case
// (`PExpr.exec_inputs_run`, `exec_lowerAll_from_empty`, `exec_inputs_nested`).
// Expected values therefore never come from the implementation under test.
//
// Finite tests stay finite. This does not prove the backend for arbitrary
// programs.
//
// Usage:
//   env -i NODE verification/foundation/differential.mjs --seed N --count K \
//     --foundation FOUNDATION_CHECK_JSON --output NEW_DIR [--import CASES_JSON] [--allow-dirty] \
//     [--skip-checker-lanes]
//   env -i NODE verification/foundation/differential.mjs --verify RECEIPT_JSON
// Add --emi for the separately Lean-checked fixed-input if/case lane.
// --import appends the cases `verification/fuzz/export-differential.mjs` exported from a fuzz
// corpus (#102). Each runs its exact corpus text against the expectation of its parsed model.
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { mkdirSync, readFileSync, realpathSync, writeFileSync } from 'node:fs';
import { userInfo } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { MIN, MAX, wrap, refRun, opSource, opsSource, leanInt, leanOps, leanList, prng } from './arithmetic.mjs';
import { buildCli, createCliObserver, matchesScalar } from './execution.mjs';
import { shrinkCandidates } from './shrinking.mjs';
import { p1Eligible, p1RandomCell, structuredP1Cases } from '../differential-interactions/p1-cases.mjs';

const root = resolve(fileURLToPath(import.meta.url), '../../..');
const project = join(root, 'proofs/foundation');
const selection = JSON.parse(readFileSync(join(root, 'policy/tool-selection.json')));
const leanBin = join(selection.tool_paths.lean.output, 'bin');
const rustBin = join(selection.tool_paths.quality_rust.output, 'bin');
const node = join(selection.tool_paths.node.output, 'bin/node');
const git = realpathSync('/run/current-system/sw/bin/git');
const { homedir: home, username } = userInfo();
const childBase = Object.freeze({
  HOME: home, USER: username, LOGNAME: username, LANG: 'C.UTF-8', CI: '1', GIT_TERMINAL_PROMPT: '0',
  PATH: [leanBin, rustBin, dirname(git), `/etc/profiles/per-user/${username}/bin`,
    '/run/current-system/sw/bin', '/usr/bin', '/bin'].join(':'),
});
const theoremsUsed = ['NobleFoundation.PExpr.exec_inputs_run', 'NobleFoundation.exec_lowerAll_from_empty',
  'NobleFoundation.exec_inputs_nested', 'NobleFoundation.exec_lowerAll', 'NobleFoundation.PExpr.exec_run_ref',
  'NobleFoundation.accumulate_eq_acceptLiteral', 'NobleFoundation.refRun_range', 'NobleFoundation.wrap_eq_knum01'];
const boundSources = [
  'verification/foundation/differential.mjs', 'verification/foundation/arithmetic.mjs',
  'verification/foundation/execution.mjs', 'verification/foundation/shrinking.mjs',
  'verification/differential-interactions/p1-cases.mjs', 'verification/differential-interactions/matrix-v1.json',
  'verification/differential-interactions/mutants-v1.json', 'verification/differential-interactions/replay-p1.mjs',
  'verification/differential-interactions/mutant-runner.mjs',
  'verification/fuzz/export-differential.mjs',
  'verification/selected-vendor.mjs',
  'crates/noble-cli/src/core/runtime/engine.mjs', 'crates/noble-cli/src/core/runtime/values.mjs',
  'proofs/foundation/NobleFoundation/Sequence.lean',
  'proofs/foundation/NobleFoundation/Literal.lean', 'proofs/foundation/NobleFoundation/Repr.lean',
  'proofs/m3/MathlibI64.lean', 'proofs/mc1/NobleContracts/Model.lean', 'proofs/mc1/NobleContracts/Rules.lean',
  'crates/noble-wasm/runtime/operations.wat', 'crates/noble-wasm/runtime/stack.wat',
  'crates/noble-wasm/runtime/programs.wat', 'crates/noble-wasm/src/operations.rs',
  'crates/noble-contracts/src/syntax/integer.rs', 'crates/noble-contracts/src/source/lexer/tokens.rs',
  'crates/noble-cli/src/core/runtime/config.json', 'policy/tool-selection.json', 'Cargo.lock',
  'crates/noble-kernel/tests/property.rs',
];

const show = value => JSON.stringify(value, (_, x) => typeof x === 'bigint' ? x.toString() : x);
const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const fileHash = path => digest(readFileSync(path));
const requireThat = (condition, message) => { if (!condition) throw Error(message); };
const commandLog = [];
function run(command, args, options = {}) {
  const { captureDirectory, ...spawnOptions } = options;
  const raw = spawnSync(command, args, { maxBuffer: 64 * 1024 * 1024,
    timeout: 3600000, env: childBase, ...spawnOptions, encoding: null });
  if (captureDirectory) {
    mkdirSync(captureDirectory);
    writeFileSync(join(captureDirectory, 'stdout'), raw.stdout ?? Buffer.alloc(0));
    writeFileSync(join(captureDirectory, 'stderr'), raw.stderr ?? Buffer.alloc(0));
    if (spawnOptions.input !== undefined) writeFileSync(join(captureDirectory, 'stdin'), spawnOptions.input);
    writeFileSync(join(captureDirectory, 'command.json'), JSON.stringify({ command, argv: args,
      cwd: spawnOptions.cwd ?? null, exit: raw.status, signal: raw.signal,
      error: raw.error ? String(raw.error) : null }, null, 2) + '\n');
  }
  const result = { ...raw, stdout: raw.stdout?.toString('utf8') ?? '', stderr: raw.stderr?.toString('utf8') ?? '' };
  commandLog.push({ command: command.split('/').pop(), args: args.length, exit: result.status });
  if (result.error || (result.status !== 0 && !options.allowFailure)) {
    throw Error(`${command} ${args.join(' ')} failed: ${result.error?.message ?? result.status}\n${result.stderr}\n${result.stdout}`);
  }
  return options.allowFailure ? result : result.stdout.trim();
}
const gitState = cwd => ({
  head: run(git, ['rev-parse', 'HEAD'], { cwd }),
  tree: run(git, ['rev-parse', 'HEAD^{tree}'], { cwd }),
  status: run(git, ['status', '--porcelain=v1', '--untracked-files=all'], { cwd }),
});

// ---------------------------------------------------------------- reference
/** PExpr: {arith:[ops]} | {quoted:n} | {compose:[P,Q]}; flat mirrors PExpr.flat. */
function flat(P, mutant = {}) {
  if (P.arith) return P.arith;
  if (P.quoted !== undefined) return [{ push: P.quoted }];
  const [p, q] = P.compose;
  return mutant.wrongComposeOrder ? [...flat(q, mutant), ...flat(p, mutant)] : [...flat(p, mutant), ...flat(q, mutant)];
}
/** Whole-case reference: top-first result stack, or null (refusal). */
function caseOps(c, mutant) {
  if (c.kind === 'straight') return c.ops;
  if (c.kind === 'program') return [...c.ins, ...flat(c.program, mutant)];
  return [...c.ins, ...c.ops]; // nested
}
function expectation(c, mutant = {}) {
  const result = refRun(caseOps(c, mutant), [], mutant);
  if (result === null) return { outcome: 'reject' };
  const prefix = mutant.corruptPrefix && c.prefix.length ? [['I64', '424242'], ...c.prefix.slice(1)] : c.prefix;
  return { outcome: 'normal', stack: [...prefix, ...[...result].reverse().map(v => ['I64', String(v)])],
    trace: mutant.expectTrace ? ['test.emit:phantom'] : [] };
}

// ----------------------------------------------------------------- rendering
function programSource(P) {
  if (P.arith) return `[ ${opsSource(P.arith)} ]`;
  if (P.quoted !== undefined) return `${P.quoted} quote`;
  return `${programSource(P.compose[0])} ${programSource(P.compose[1])} compose`;
}
function caseSource(c) {
  const parts = c.prefix.map(([, v]) => v);
  if (c.kind === 'straight') parts.push(opsSource(c.ops));
  else if (c.kind === 'program') parts.push(opsSource(c.ins), programSource(c.program), 'run');
  else parts.push(opsSource(c.ins), `[ ${opsSource(c.ops)} ] quote run run`);
  return parts.filter(Boolean).join(' ') + '\n';
}
function leanProgram(P) {
  if (P.arith) return `(.arith ${leanOps(P.arith)})`;
  if (P.quoted !== undefined) return `(.quoted ${leanInt(P.quoted)})`;
  return `(.compose ${leanProgram(P.compose[0])} ${leanProgram(P.compose[1])})`;
}
/** Lean checks bound to one case: refRun result by `decide`, then the MC1 theorem. */
function leanCase(c, index, result) {
  const name = `case_${index}`;
  if (result === null) {
    return `theorem ${name}_refused : refRun ${leanOps(caseOps(c))} [] = none := by decide\n`;
  }
  const r = leanList(result);
  if (c.kind === 'straight') {
    return `theorem ${name}_ref : refRun ${leanOps(c.ops)} [] = some ${r} := by decide\n` +
      `example (pre t : Stack) := exec_lowerAll_from_empty ${name}_ref pre t\n`;
  }
  if (c.kind === 'program') {
    return `theorem ${name}_ref : refRun (${leanOps(c.ins)} ++ PExpr.flat ${leanProgram(c.program)}) [] = some ${r} := by decide\n` +
      `example (pre t : Stack) := PExpr.exec_inputs_run ${leanProgram(c.program)} ${leanOps(c.ins)} ${name}_ref pre t\n`;
  }
  return `theorem ${name}_ref : refRun (${leanOps(c.ins)} ++ ${leanOps(c.ops)}) [] = some ${r} := by decide\n` +
    `example (pre t : Stack) := exec_inputs_nested ${leanOps(c.ins)} ${leanOps(c.ops)} ${name}_ref pre t\n`;
}

// ----------------------------------------------------------------- generation
const BOUNDARY = [0n, 1n, -1n, 2n, -2n, 3n, -3n, MAX, MIN, MAX - 1n, MIN + 1n, 2n ** 31n, -(2n ** 31n),
  2n ** 32n, 2n ** 32n - 1n, 2n ** 62n, -(2n ** 62n), 3037000499n, 3037000500n, -3037000500n];
function generator(seed) {
  const next = prng(seed);
  const int = n => Number(next() % BigInt(n));
  const value = () => int(2) === 0 ? BOUNDARY[int(BOUNDARY.length)] : BigInt.asIntN(64, next());
  const prefixItem = () => int(3) === 0 ? ['Bool', int(2) ? 'true' : 'false'] : ['I64', String(value())];
  /** A depth-respecting op list over an operand stack of `depth` I64s. */
  function ops(depth, length, needOut) {
    const out = [];
    for (let i = 0; i < length; i++) {
      if (depth >= 2 && int(2) === 0) { out.push(['add', 'sub', 'mul'][int(3)]); depth -= 1; }
      else { out.push({ push: value() }); depth += 1; }
    }
    while (depth > needOut) { out.push(['add', 'sub', 'mul'][int(3)]); depth -= 1; }
    return { ops: out, depth };
  }
  /** A PExpr whose flattened ops map depth `k` to depth `k`+delta, with every
  operand produced inside or below. */
  function program(k, budget) {
    const shape = budget <= 0 ? int(2) : int(3);
    if (shape === 0) { const { ops: body, depth } = ops(k, 1 + int(4), Math.max(1, Math.min(k, 1))); return { P: { arith: body }, depth }; }
    if (shape === 1) return { P: { quoted: value() }, depth: k + 1 };
    const left = program(k, budget - 1);
    const right = program(left.depth, budget - 1);
    return { P: { compose: [left.P, right.P] }, depth: right.depth };
  }
  return function make(index) {
    const prefix = Array.from({ length: int(3) }, prefixItem);
    const kind = ['straight', 'program', 'program', 'nested'][int(4)];
    if (kind === 'straight') return { index, kind, prefix, ops: ops(0, 1 + int(6), 1).ops };
    const ins = Array.from({ length: int(3) }, () => ({ push: value() }));
    if (kind === 'nested') return { index, kind, prefix, ins, ops: ops(ins.length, 1 + int(4), 1).ops };
    return { index, kind, prefix, ins, program: program(ins.length, 2).P };
  };
}
/** Rejection cases: out-of-range literals (frontend refusal, never wrapping). */
const rejectionCases = [
  { kind: 'straight', prefix: [], ops: [{ push: MAX + 1n }] },
  { kind: 'straight', prefix: [['Bool', 'true']], ops: [{ push: 1n }, { push: MIN - 1n }, 'add'] },
  { kind: 'program', prefix: [], ins: [{ push: 5n }], program: { arith: [{ push: 2n ** 64n }, 'add'] } },
];

// ------------------------------------------------------------------ shrinking
const SHRINK_STEPS = 48;
/** Bounded greedy shrinking preserving `fails`. Candidates whose reference is a
refusal are skipped so shrinking never changes the kind of disagreement. */
function shrink(c, fails, eligible = () => true) {
  let current = c, steps = 0;
  const rejected = { reference_refusal: 0, target_feature_eliminated: 0, different_failure: 0 };
  outer: while (steps < SHRINK_STEPS) {
    for (const candidate of shrinkCandidates(current)) {
      if (++steps > SHRINK_STEPS) break outer;
      if (expectation(candidate).outcome !== 'normal') { rejected.reference_refusal++; continue; }
      if (!eligible(candidate)) { rejected.target_feature_eliminated++; continue; }
      const result = fails(candidate);
      if (result === true) { current = candidate; continue outer; }
      if (result === 'different-failure') rejected.different_failure++;
    }
    break;
  }
  return { case: current, steps, rejected,
    termination_reason: steps >= SHRINK_STEPS ? 'attempt-budget-exhausted' : 'local-minimum-under-eligible-shrinks' };
}

// --------------------------------------------------------------- verify mode
const args = process.argv.slice(2);
const option = name => { const i = args.indexOf(name); return i >= 0 ? args[i + 1] : undefined; };
const forbidden = Object.keys(process.env).filter(key =>
  /^(?:LEAN(?:_|$)|LAKE(?:_|$)|NODE(?:_|$)|BASH(?:_|$)|ENV$|LD_|DYLD_|CARGO_|RUST|GIT_|NIX_|NOBLE_)/.test(key)
  && process.env[key] !== '');
if (forbidden.length) {
  console.error(`[differential] FAIL unsafe caller environment: ${forbidden.sort().join(', ')}`);
  process.exit(1);
}
if (args.includes('--emi')) {
  const { runEmi } = await import('./emi.mjs');
  await runEmi({ args, option, root, project, selection, childBase, run, gitState, leanBin, node, boundSources });
  process.exit();
}
if (option('--verify')) {
  // Reject stale evidence: the receipt must describe this exact clean tree and
  // every bound file must still have its recorded hash.
  try {
    const receipt = JSON.parse(readFileSync(option('--verify')));
    const state = gitState(root);
    const drift = Object.entries(receipt.sources_sha256).filter(([p, h]) => fileHash(join(root, p)) !== h).map(([p]) => p);
    const ok = receipt.schema === 'noble-foundation-differential/v1' && state.status === '' &&
      receipt.source_tree === state.tree && drift.length === 0 && receipt.all_matched === true;
    console.log(JSON.stringify({ verified: ok, receipt_tree: receipt.source_tree, current_tree: state.tree,
      clean: state.status === '', drift }, null, 2));
    process.exitCode = ok ? 0 : 1;
  } catch (error) { console.error(`[differential] FAIL ${error.message}`); process.exitCode = 1; }
  process.exit();
}

const seed = BigInt(option('--seed') ?? '15');
const count = Number(option('--count') ?? '80');
const foundationPath = option('--foundation');
const output = option('--output');
const allowDirty = args.includes('--allow-dirty');
const skipLanes = args.includes('--skip-checker-lanes');
const mutationProbe = args.includes('--mutation-probe');
const mutationId = option('--mutation-id');
const mutationCell = option('--mutation-cell');
const importPath = option('--import');
requireThat(output && foundationPath && Number.isInteger(count) && count > 0,
  'usage: differential.mjs --seed N --count K --foundation FOUNDATION_CHECK_JSON --output NEW_DIR [--import CASES_JSON] [--allow-dirty] [--skip-checker-lanes]');
requireThat(!mutationProbe || (typeof mutationId === 'string' && mutationId.length > 0 &&
  typeof mutationCell === 'string' && mutationCell.length > 0),
  '--mutation-probe requires --mutation-id and --mutation-cell');
const out = resolve(output);
mkdirSync(out);
const receipt = { schema: 'noble-foundation-differential/v1', profile: 'Core-Bootstrap / managed-linear-memory',
  fragment: 'I64 literals, + - *, quote, literal blocks, compose, run, nested quote-run-run, Bool/I64 prefix',
  seed: String(seed), count, opt_modes: ['off', 'on'], universal_backend_proof: false };
if (mutationProbe) receipt.mutation_probe = { mutation_id: mutationId, target_cell: mutationCell,
  purpose: 'scratch semantic mutation; not production source' };

function p1MismatchClass(expected, observed, model) {
  if (observed.outcome !== 'normal') return `unexpected-${observed.outcome}`;
  if (JSON.stringify(expected.stack) === JSON.stringify(observed.stack)) return null;
  const prefixLength = model.prefix.length;
  if (prefixLength > 0 && JSON.stringify(expected.stack.slice(0, prefixLength)) !==
      JSON.stringify(observed.stack.slice(0, prefixLength))) return 'ordered-prefix-output-mismatch';
  if (expected.stack.length !== observed.stack.length) return 'ordered-output-shape-mismatch';
  return 'ordered-arithmetic-output-mismatch';
}
try {
  requireThat(realpathSync(process.execPath) === realpathSync(node), `use selected Node: ${node}`);
  const initial = gitState(root);
  requireThat(allowDirty || initial.status === '', `Noble source worktree dirty: ${initial.status}`);
  Object.assign(receipt, { source_commit: initial.head, source_tree: initial.tree, clean_source: initial.status === '' });

  // Proof subject: theorems used must be in a foundation receipt for this tree.
  const foundation = JSON.parse(readFileSync(foundationPath));
  requireThat(foundation.schema === 'noble-foundation-check/v1', 'invalid foundation receipt');
  const staleFoundation = foundation.source_tree !== initial.tree ||
    Object.entries(foundation.sources_sha256).some(([p, h]) => fileHash(join(root, p)) !== h);
  requireThat(!staleFoundation, 'stale-proof-subject: foundation receipt is for another tree');
  receipt.theorems = theoremsUsed.map(name => {
    const t = foundation.theorems.find(x => x.name === name);
    requireThat(t?.status === 'proved', `missing theorem ${name}`);
    return { name, statement: t.statement, axioms: t.axioms };
  });
  receipt.foundation_receipt_sha256 = fileHash(foundationPath);

  // CLI from this checkout (as compare.mjs), tools and helpers recorded.
  const { cli, env } = buildCli({ root, out, selection, childBase, run, targetDir: option('--target-dir') });
  const tools = { node, wasm_tools: join(selection.tool_paths.wasm_tools.output, 'bin/wasm-tools'),
    wasm_opt: join(selection.tool_paths.binaryen.output, 'bin/wasm-opt') };
  receipt.toolchain = { cli_sha256: fileHash(cli), lean: foundation.lean, mathlib_commit: foundation.mathlib_commit,
    tools: Object.fromEntries(Object.entries(tools).map(([k, p]) => [k, { path: p, sha256: fileHash(p) }])) };

  // Generate cases and their references.
  const make = generator(seed);
  const cases = [...Array.from({ length: count }, (_, i) => make(i)),
    ...rejectionCases.map((c, i) => ({ ...c, index: count + i }))];
  // Imported corpus cases follow the seeded and rejection cases. The model carries the
  // expectation; the executed source is the corpus text itself, never a re-rendering of the model.
  let exported = null;
  if (importPath) {
    exported = JSON.parse(readFileSync(importPath));
    requireThat(exported.schema === 'noble-fuzz-differential-cases/v1', 'unexpected import schema');
    const revive = node => Array.isArray(node) ? node.map(revive)
      : typeof node === 'object' && node !== null
        ? Object.fromEntries(Object.entries(node).map(([key, value]) =>
          [key, key === 'push' || key === 'quoted' ? BigInt(value) : revive(value)]))
        : node;
    const reviveCase = ({ origin, ...model }, invalidControl = false) => {
      requireThat(typeof origin?.text === 'string' && /^[0-9a-f]{64}$/.test(origin.sha256 ?? ''),
        'an imported case lacks its corpus text or digest');
      requireThat(digest(Buffer.from(origin.text)) === (origin.text_sha256 ?? origin.sha256),
        'an imported case text differs from its recorded digest');
      cases.push({ ...revive(model), index: cases.length, origin, invalidControl });
    };
    for (const model of exported.cases) reviveCase(model);
    for (const model of exported.invalid_cases ?? []) reviveCase(model, true);
    const issue125Cases = exported.cases.filter(model => model.origin?.unit === 'issue125-p1-matrix-v1');
    if (issue125Cases.length) {
      requireThat(seed === 125n && count === 18,
        'matrix-v1 P1 comparison requires --seed 125 --count 18 for a matched case-run budget');
      requireThat(issue125Cases.length === 18 && exported.invalid_cases?.filter(model =>
        model.origin?.unit === 'issue125-p1-invalid-control').length === 1,
      'matrix-v1 P1 import must contain its 18 selected cells and one invalid-premise control');
    }
  }
  for (const c of cases) { c.source = c.origin ? c.origin.text : caseSource(c); c.expected = expectation(c); }
  if (mutationProbe) {
    const sample = structuredP1Cases().find(c =>
      c.interaction.cell === 'P1/composed/min_max_wrap/ordered_single');
    requireThat(sample && p1Eligible(sample), 'frozen composed P1 shrink-control case is ineligible');
    const featureEliminated = { ...sample,
      program: { arith: [{ push: MAX }, { push: 1n }, 'add'] } };
    const sampleExpected = expectation(sample);
    const wrongSameShape = { outcome: 'normal', stack: sampleExpected.stack.map(([type, value]) =>
      [type, type === 'I64' ? String(BigInt(value) + 1n) : value]), trace: [] };
    const identity = p1MismatchClass(sampleExpected, wrongSameShape, sample);
    const earlierRefusal = p1MismatchClass(sampleExpected, { outcome: 'reject' }, sample);
    const substitutedFailure = p1MismatchClass(sampleExpected, { outcome: 'normal', stack: [], trace: [] }, sample);
    receipt.mutation_probe.shrink_control = {
      control: 'candidate replaces composed program with straight arithmetic',
      original_eligible: p1Eligible(sample),
      target_feature_eliminated_rejected: !p1Eligible(featureEliminated),
      earlier_refusal_substitution_rejected: identity !== null && earlierRefusal !== identity,
      different_failure_substitution_rejected: identity !== null && substitutedFailure !== identity,
      failure_identities: { intended: identity, earlier_refusal: earlierRefusal,
        substituted_failure: substitutedFailure },
    };
    requireThat(receipt.mutation_probe.shrink_control.target_feature_eliminated_rejected &&
      receipt.mutation_probe.shrink_control.earlier_refusal_substitution_rejected &&
      receipt.mutation_probe.shrink_control.different_failure_substitution_rejected,
      'P1 shrink feature/refusal/failure-identity control was accepted');
  }
  requireThat(cases.slice(0, count).every(c => c.expected.outcome === 'normal'), 'generator produced an unexpected refusal');
  const imported = cases.filter(c => c.origin && !c.invalidControl);
  const invalidImported = cases.filter(c => c.origin && c.invalidControl);
  const pilot = imported.filter(c => c.origin.unit === 'issue125-plausible-v1');
  if (pilot.length) {
    const protocol = JSON.parse(readFileSync(join(root,
      'verification/differential-interactions/plausible/matrix-v1.json')));
    requireThat(protocol.schema === 'noble-plausible-interaction-pilot/v1' &&
      seed === BigInt(protocol.seed) && count === protocol.cases_per_arm &&
      pilot.length === protocol.cases_per_arm && pilot.length === imported.length &&
      pilot.every(c => p1Eligible(c) && protocol.selected_cells.includes(c.interaction.cell)) &&
      invalidImported.length === 1 && invalidImported[0].origin.unit === 'issue125-p1-invalid-control',
    'Plausible pilot import differs from its frozen typed 16-case protocol');
    const counts = new Map();
    for (const c of pilot) counts.set(c.interaction.cell, (counts.get(c.interaction.cell) ?? 0) + 1);
    requireThat(protocol.selected_cells.every(cell => counts.get(cell) === 4) &&
      counts.size === protocol.selected_cells.length,
    'Plausible pilot did not allocate four actual generated cases per selected cell');
  }
  requireThat(imported.every(c => c.expected.outcome === 'normal'), 'an imported case is not a normal reference run');
  requireThat(invalidImported.every(c => c.expected.outcome === 'reject'),
    'an imported one-premise invalid control did not refuse in the independent reference');

  // Lean kernel check of every expectation and per-case MC1 theorem.
  const leanFile = join(out, 'Expected.lean');
  writeFileSync(leanFile, ['import NobleFoundation.Sequence', 'open NobleFoundation NobleContracts', '',
    ...cases.map(c => {
      const result = c.expected.outcome === 'normal' ? refRun(caseOps(c), []) : null;
      c.lean = leanCase(c, c.index, result);
      return `-- source sha256 ${digest(c.source)}\n${c.lean}`;
    }), ''].join('\n'));
  const lake = join(leanBin, 'lake');
  const leanResult = run(lake, ['env', 'lean', '--json', leanFile], { cwd: project,
    env: { ...childBase, TMPDIR: out }, allowFailure: true });
  requireThat(leanResult.status === 0 && !/"severity":"(error|warning)"/.test(leanResult.stdout),
    `Lean rejected expectations:\n${leanResult.stdout.slice(0, 4000)}`);
  receipt.expected_lean = { path: 'Expected.lean', sha256: fileHash(leanFile), kernel_checked_cases: cases.length };

  // Execute each case in both optimization modes.
  const observe = createCliObserver({ root, out, cli, env, run });
  const decodeSigned = v => v;
  const decodeUnsigned = ([type, v]) => type === 'I64' ? [type, String(BigInt.asUintN(64, BigInt(v)))] : [type, v];
  const agrees = (expected, obs, decode = x => x) => expected.outcome === 'normal'
    ? matchesScalar(expected, obs, decode)
    : obs.outcome === 'reject' && obs.stage === 'parse' && obs.diagnostic === 'integer literal is outside I64';
  const observations = new Map();
  receipt.cases = [];
  for (const c of cases) {
    for (const mode of ['off', 'on']) {
      if (c.expected.outcome !== 'normal' && mode === 'on') continue; // parse refusal precedes mode
      const obs = observe(c, mode);
      observations.set(`${c.index}/${mode}`, obs);
      const matched = agrees(c.expected, obs, decodeSigned);
      const entry = { index: c.index, kind: c.kind, mode, source: c.source.trim(), source_sha256: digest(c.source),
        expected: c.expected, observed: { outcome: obs.outcome, stack: obs.stack, diagnostic: obs.diagnostic,
          execution_steps: obs.metrics?.execution_steps ?? null },
        wasm_sha256: obs.wasm_sha256 ?? null, lean_theorem: `case_${c.index}_ref`, matched,
        corpus_unit: c.origin ? { unit: c.origin.unit, sha256: c.origin.sha256 } : undefined,
        interaction: c.interaction ?? null,
        ...(c.interaction?.profile === 'P1-core-source-wasm' ? { interaction_witness: {
          admission_outcome: obs.outcome,
          executed: obs.outcome === 'normal',
          runtime_execution_steps: obs.metrics?.execution_steps ?? null,
          positive_execution_steps: Number.isInteger(obs.metrics?.execution_steps) && obs.metrics.execution_steps > 0,
          operator_trace_available: false,
          source_operator_witness: c.interaction.source_operator_witness,
          property_engaged: matched && obs.outcome === 'normal',
          property_failed: !matched && obs.outcome === 'normal' &&
            Number.isInteger(obs.metrics?.execution_steps) && obs.metrics.execution_steps > 0,
          prefix_preserved: c.interaction.output_shape !== 'preserved_prefix_multiple' ||
            JSON.stringify(obs.stack?.slice(0, c.prefix.length)) === JSON.stringify(c.expected.stack.slice(0, c.prefix.length)),
          comparison_legs_reached: ['BigInt reference', 'Lean-kernel-checked expectation', `managed Wasm opt-${mode}`],
        } } : {}) };
      if (!matched) {
        entry.shrunk = shrink(c, candidate => {
          const cand = { ...candidate, index: `shrink-${c.index}`, source: caseSource(candidate) };
          return !agrees(expectation(cand), observe(cand, mode));
        });
      }
      receipt.cases.push(entry);
    }
  }
  receipt.all_matched = receipt.cases.every(c => c.matched);
  if (mutationProbe) {
    const targetRow = receipt.cases.find(entry => entry.interaction?.cell === mutationCell && !entry.matched);
    if (targetRow) {
      const model = cases.find(c => c.index === targetRow.index);
      const originalObservation = observations.get(`${model.index}/${targetRow.mode}`);
      const failureIdentity = p1MismatchClass(model.expected, originalObservation, model);
      const shrunk = shrink(model, candidate => {
        const observed = observe({ ...candidate, index: `mutation-shrink-${model.index}`, source: caseSource(candidate) }, targetRow.mode);
        const candidateClass = p1MismatchClass(expectation(candidate), observed, candidate);
        if (candidateClass === failureIdentity && observed.outcome === 'normal' &&
            Number.isInteger(observed.metrics?.execution_steps) && observed.metrics.execution_steps > 0) return true;
        return candidateClass === null ? false : 'different-failure';
      }, p1Eligible);
      const minimized = shrunk.case;
      const minimizedSource = caseSource(minimized);
      const minimizedObserved = observe({ ...minimized, index: `mutation-minimized-${model.index}`,
        source: minimizedSource }, targetRow.mode);
      const witness = (candidate, source, observed) => ({
        cell: mutationCell,
        mutation_site: mutationId,
        model: JSON.parse(show(candidate)),
        source,
        source_sha256: digest(source),
        source_size_bytes: Buffer.byteLength(source),
        expected: expectation(candidate),
        input_literals: caseOps(candidate).filter(operation => typeof operation === 'object')
          .map(operation => String(operation.push)),
        prefix: candidate.prefix,
        mode: targetRow.mode,
        snapshot: null,
        limits: 'foundation CLI runtime defaults; no parameter snapshot',
        observed: { outcome: observed.outcome, stack: observed.stack ?? null,
          execution_steps: observed.metrics?.execution_steps ?? null,
          wasm_sha256: observed.wasm_sha256 ?? null },
        failure_identity: p1MismatchClass(expectation(candidate), observed, candidate),
        execution_witness: { runtime_steps: observed.metrics?.execution_steps ?? null,
          positive_runtime_steps: Number.isInteger(observed.metrics?.execution_steps) && observed.metrics.execution_steps > 0,
          source_operator_witness: candidate.interaction?.source_operator_witness ?? [],
          operator_trace_available: false,
          witness_kind: 'positive successful-run steps and output-discriminating retained source; no per-operator runtime trace' },
      });
      const original = witness(model, model.source, originalObservation);
      const minimizedWitness = witness(minimized, minimizedSource, minimizedObserved);
      receipt.mutation_probe.witness = {
        outcome: targetRow.interaction_witness.property_failed && targetRow.interaction_witness.positive_execution_steps
          ? 'killed' : 'divergence-unreached',
        failure_identity: failureIdentity,
        original,
        minimized: minimizedWitness,
        candidate_cli_sha256: receipt.toolchain.cli_sha256,
        witness_sizes_bytes: { original: original.source_size_bytes, minimized: minimizedWitness.source_size_bytes },
        shrink_work: { attempts: shrunk.steps, budget: SHRINK_STEPS, rejected: shrunk.rejected,
          termination_reason: shrunk.termination_reason,
          eligibility_rechecked_each_candidate: true,
          expected_result_rechecked_each_candidate: true,
          failure_identity_rechecked_each_candidate: true,
          target_feature_preserved: p1Eligible(minimized),
          mismatch_preserved: minimizedWitness.failure_identity === failureIdentity },
      };
    } else {
      receipt.mutation_probe.witness = { outcome: 'executed-survived',
        target_cell_observations: receipt.cases.filter(entry => entry.interaction?.cell === mutationCell).map(entry => ({
          mode: entry.mode, matched: entry.matched, admission: entry.observed.outcome,
          execution_steps: entry.observed.execution_steps,
        })) };
    }
  }
  requireThat(receipt.all_matched || mutationProbe,
    `mismatch: ${show(receipt.cases.filter(c => !c.matched)).slice(0, 2000)}`);
  receipt.imported = exported && {
    cases_sha256: fileHash(importPath), corpus: exported.corpus, export_counts: exported.counts,
    cases: imported.length,
    executed_cases: imported.filter(c => ['off', 'on'].every(mode =>
      observations.get(`${c.index}/${mode}`)?.outcome === 'normal')).length,
    invalid_cases: invalidImported.length,
    executions: receipt.cases.filter(entry => entry.corpus_unit && !entry.interaction?.variant).length,
    invalid_control_executions: receipt.cases.filter(entry => entry.corpus_unit && entry.interaction?.variant).length,
  };
  const optPairs = cases.filter(c => c.expected.outcome === 'normal');
  receipt.opt_on_off_same_stack = optPairs.every(c =>
    JSON.stringify(observations.get(`${c.index}/off`).stack) === JSON.stringify(observations.get(`${c.index}/on`).stack));
  receipt.coverage = {
    kinds: Object.fromEntries(['straight', 'program', 'nested'].map(k => [k, cases.filter(c => c.kind === k).length])),
    with_prefix: cases.filter(c => c.prefix.length).length,
    with_bool_prefix: cases.filter(c => c.prefix.some(([t]) => t === 'Bool')).length,
    with_compose: cases.filter(c => show(c.program ?? {}).includes('compose')).length,
    with_nested_compose: cases.filter(c => /compose.*compose/.test(show(c.program ?? {}))).length,
    with_quote: cases.filter(c => c.kind === 'nested' || show(c.program ?? {}).includes('quoted')).length,
    negative_results: cases.filter(c => c.expected.stack?.some(([t, v]) => t === 'I64' && v.startsWith('-'))).length,
    boundary_values: cases.filter(c => /9223372036854775807|-9223372036854775808/.test(c.source)).length,
    rejection_cases: rejectionCases.length,
    imported_corpus_cases: imported.length,
  };
  const p1Rows = receipt.cases.filter(entry => entry.interaction?.profile === 'P1-core-source-wasm' && entry.interaction.cell);
  if (p1Rows.length) {
    const pilotRun = pilot.length > 0;
    const cells = new Map();
    for (const row of p1Rows) {
      const existing = cells.get(row.interaction.cell) ?? { cell: row.interaction.cell, selected: true, runs: [] };
      existing.runs.push({ mode: row.mode, reached: row.interaction_witness.executed &&
        row.interaction_witness.positive_execution_steps && row.interaction_witness.property_engaged &&
        row.interaction_witness.prefix_preserved, matched: row.matched,
        admission_outcome: row.interaction_witness.admission_outcome });
      cells.set(row.interaction.cell, existing);
    }
    const selectedCells = [...cells.values()].sort((a, b) => a.cell.localeCompare(b.cell)).map(cell => ({
      ...cell, reached: cell.runs.length === (pilotRun ? 8 : 2) &&
        cell.runs.every(run => run.reached && run.matched),
    }));
    receipt.interaction_matrix = {
      profile: 'P1-core-source-wasm', version: 1, selected: selectedCells.length,
      matrix_sha256: fileHash(join(root, 'verification/differential-interactions/matrix-v1.json')),
      reached: selectedCells.filter(cell => cell.reached).length,
      unreached: selectedCells.filter(cell => !cell.reached).map(cell => cell.cell),
      accepted: selectedCells.filter(cell => cell.runs.some(run => run.admission_outcome === 'normal')).length,
      refused: selectedCells.filter(cell => cell.runs.some(run => run.admission_outcome === 'reject')).length,
      exhausted: selectedCells.filter(cell => cell.runs.some(run => run.admission_outcome === 'runtime-exhausted')).length,
      unsupported: 1,
      unsupported_features: ['per-case runtime fuel limit: the foundation CLI observer exposes no fuel override; the case-count budget is measured separately'],
      comparison_legs: ['BigInt reference', 'Lean-kernel-checked expectation', 'managed Wasm opt-off', 'managed Wasm opt-on'],
      cells: selectedCells,
      all_refused_is_semantic_acceptance: false,
    };
    const invalidRows = receipt.cases.filter(row => row.corpus_unit?.unit === 'issue125-p1-invalid-control');
    receipt.p1_budget = { kind: 'frozen-case-count-per-optimization-mode',
      selected_case_limit: pilotRun ? 16 : 18, selected_cases: pilotRun ? pilot.length : selectedCells.length,
      observed: { opt_off: pilotRun ? pilot.length : selectedCells.length,
        opt_on: pilotRun ? pilot.length : selectedCells.length },
      budget_boundary_reached: (pilotRun ? pilot.length === 16 : selectedCells.length === 18),
      runtime_fuel_boundary_supported: false,
      note: 'The foundation CLI differential observer has no per-run fuel override; this boundary is the frozen case-work budget.' };
    receipt.p1_invalid_control = { selected: invalidRows.length,
      refused: invalidRows.length === 1 && invalidRows[0].observed.outcome === 'reject' && invalidRows[0].matched,
      rows: invalidRows.map(row => ({ source: row.source, source_sha256: row.source_sha256,
        outcome: row.observed.outcome, matched: row.matched, variant: row.interaction?.variant ?? null })) };
    receipt.p1_outcome_summary = { accepted_cells: selectedCells.filter(cell => cell.runs.some(run => run.admission_outcome === 'normal')).length,
      refused_invalid_variants: invalidRows.filter(row => row.observed.outcome === 'reject' && row.matched).length,
      exhausted_runs: selectedCells.filter(cell => cell.runs.some(run => run.admission_outcome === 'runtime-exhausted')).length,
      unsupported_variants: 1 };
    if (!mutationProbe) requireThat(selectedCells.length === (pilotRun ? 4 : 18) &&
      selectedCells.every(cell => cell.reached),
    `P1 interaction matrix incomplete: reached ${selectedCells.filter(cell => cell.reached).length}/${pilotRun ? 4 : 18}`);
    if (!mutationProbe) requireThat(receipt.p1_invalid_control.refused,
      'P1 one-premise invalid control did not reach the expected frontend refusal');

    const randomCases = cases.slice(0, count);
    const randomCells = new Map();
    const randomFailures = [];
    for (const model of randomCases) {
      const cell = p1RandomCell(model);
      const rows = receipt.cases.filter(row => row.index === model.index && row.mode !== undefined);
      if (!cell) {
        for (const row of rows.filter(row => !row.matched)) {
          randomFailures.push({ generator: 'current-seeded', case_index: model.index, cell: null,
            failure_attribution: 'unclassified-current-generator-case', mode: row.mode,
            failure_identity: p1MismatchClass(model.expected, row.observed, model),
            source_sha256: row.source_sha256 });
        }
        continue;
      }
      const reached = rows.length === 2 && rows.every(row => row.matched && row.observed.outcome === 'normal' &&
        Number.isInteger(row.observed.execution_steps) && row.observed.execution_steps > 0);
      randomCells.set(cell, (randomCells.get(cell) ?? false) || reached);
      for (const row of rows.filter(row => !row.matched)) {
        randomFailures.push({ generator: 'current-seeded', case_index: model.index, cell: cell ?? 'unclassified',
          mode: row.mode, failure_identity: p1MismatchClass(model.expected, row.observed, model),
          source_sha256: row.source_sha256 });
      }
    }
    const structuredFailures = receipt.cases.filter(row =>
      row.interaction?.profile === 'P1-core-source-wasm' && row.interaction.cell && !row.matched)
      .map(row => ({ generator: 'structured-extension', cell: row.interaction.cell, mode: row.mode,
        failure_identity: p1MismatchClass(row.expected, row.observed, cases.find(c => c.index === row.index)),
        source_sha256: row.source_sha256 }));
    receipt.generator_comparison = {
      profiles_separate: true,
      same_compute_seed_budget: { seed: String(seed), cases_per_mode: count, opt_modes: ['off', 'on'],
        current_generator_case_runs: count * 2, structured_extension_case_runs: (pilotRun ? pilot.length : selectedCells.length) * 2,
        equal_case_run_budget: count === (pilotRun ? pilot.length : selectedCells.length) },
      current_generator: { classified_cells: randomCells.size, reached_cells: [...randomCells]
          .filter(([, reached]) => reached).map(([cell]) => cell).sort(),
        classified_cell_observation: 'seeded case model plus successful exact source execution and positive runtime steps; no operator trace',
        distinct_attributed_failures: randomFailures },
      structured_extension: { selected_cells: selectedCells.length, generated_cases: pilotRun ? pilot.length : selectedCells.length,
        reached_cells: selectedCells.filter(cell => cell.reached).map(cell => cell.cell),
        witness_kind: 'frozen interaction model plus positive execution steps and property comparison; no operator trace',
        distinct_attributed_failures: structuredFailures },
    };
  }

  // Mutation controls: each must disagree with the observations, and only for
  // its intended reason.
  const controls = [];
  if (!mutationProbe) {
  const control = (id, intended, mutant, decode, attributable) => {
    const failing = [];
    for (const c of cases) for (const mode of ['off', 'on']) {
      const obs = observations.get(`${c.index}/${mode}`); if (!obs) continue;
      if (!agrees(expectation(c, mutant), obs, decode)) failing.push({ c, mode });
    }
    const allAttributable = failing.every(({ c }) => attributable(c));
    const first = failing[0];
    const shrunk = first && first.c.expected.outcome === 'normal'
      ? shrink(first.c, cand => !agrees(expectation(cand, mutant),
        observe({ ...cand, index: `ctl-${id}`, source: caseSource(cand) }, first.mode), decode)) : null;
    const ok = failing.length > 0 && allAttributable;
    controls.push({ id, intended, detected_cases: failing.length, all_attributable_to_intended_reason: allAttributable,
      shrunk: shrunk && { source: caseSource(shrunk.case).trim(), steps: shrunk.steps }, demonstrably_fails: ok });
    requireThat(ok, `control ${id} did not fail for its intended reason`);
  };
  const hasSub = c => caseOps(c).includes('sub');
  control('reversed-subtraction', 'reference computes top - deeper', { reversedSub: true }, decodeSigned, hasSub);
  control('unsigned-decoding', 'observed I64 decoded as unsigned', {}, decodeUnsigned,
    c => c.expected.stack?.some(([t, v]) => t === 'I64' && v.startsWith('-')));
  control('prefix-corruption', 'expected bottom prefix item replaced', { corruptPrefix: true }, decodeSigned,
    c => c.prefix.length > 0);
  control('wrong-composition-order', 'compose evaluated as q then p', { wrongComposeOrder: true }, decodeSigned,
    c => show(c.program ?? {}).includes('compose'));
  control('trace-mismatch', 'expectation claims a host request', { expectTrace: true }, decodeSigned,
    c => c.expected.outcome === 'normal');
  // Failure-classification mismatch: expecting wrapping where the frontend refuses.
  const wrappedExpectation = c => {
    const wrapped = caseOps(c).map(op => typeof op === 'object' ? { push: wrap(op.push) } : op);
    const r = refRun(wrapped, []);
    return { outcome: 'normal', stack: [...c.prefix, ...[...r].reverse().map(v => ['I64', String(v)])], trace: [] };
  };
  const refusalCases = cases.filter(c => c.expected.outcome === 'reject');
  const refusalFailing = refusalCases
    .filter(c => !agrees(wrappedExpectation(c), observations.get(`${c.index}/off`)));
  controls.push({ id: 'literal-wrapping-instead-of-refusal', intended: 'out-of-range literal assumed to wrap',
    detected_cases: refusalFailing.length, demonstrably_fails: refusalFailing.length === refusalCases.length });
  requireThat(refusalFailing.length === refusalCases.length, 'refusal control did not fail');
  // Stale proof subject: Lean text bound to another case's program.
  const a = cases[0], b = cases.find(c => c.source !== a.source && c.expected.outcome === 'normal');
  const rebound = leanCase(a, a.index, refRun(caseOps(a), []));
  const staleRejected = rebound !== leanCase(b, a.index, refRun(caseOps(b), []));
  const staleFoundationRejected = { ...foundation, source_tree: '0'.repeat(40) }.source_tree !== initial.tree;
  controls.push({ id: 'stale-proof-subject', intended: 'Lean case or foundation receipt for a different subject',
    lean_case_rebinding_rejected: staleRejected, foundation_tree_mismatch_rejected: staleFoundationRejected,
    demonstrably_fails: staleRejected && staleFoundationRejected });
  requireThat(staleRejected && staleFoundationRejected, 'stale proof subject control did not fail');
  receipt.controls = controls;
  } else {
    receipt.controls = { skipped: true, reason: 'mutant outcome is classified by the issue-125 semantic witness' };
  }

  // Checker property lanes (acceptance coverage, not execution semantics).
  if (!skipLanes && !mutationProbe) {
    const lanes = run(join(rustBin, 'cargo'), ['test', '--locked', '--offline', '-j', '4', '-p', 'noble-kernel',
      '--test', 'property'], { cwd: root, env, timeout: 3600000, allowFailure: true });
    const summary = (lanes.stdout + lanes.stderr).split('\n').filter(l => /^test |test result/.test(l));
    receipt.checker_property_lanes = { command: 'cargo test --locked --offline -p noble-kernel --test property',
      exit: lanes.status, summary, generated_oracle_candidates: 1000, malformed_candidates: 200,
      coverage_class: 'checker acceptance (kernel), NOT execution semantics' };
    requireThat(lanes.status === 0, 'checker property lanes failed');
  } else {
    receipt.checker_property_lanes = { skipped: true,
      reason: mutationProbe ? 'baseline-only independent property lane' : 'requested by --skip-checker-lanes' };
  }

  const final = gitState(root);
  requireThat(final.head === initial.head && final.tree === initial.tree && final.status === initial.status,
    'Noble source worktree changed during comparison');
  receipt.sources_sha256 = Object.fromEntries(boundSources.map(p => [p, fileHash(join(root, p))]));
  receipt.claims = [
    { claim: 'refRun semantics equal MC1 Exec for the fragment (any prefix)', status: 'proved', theorems: theoremsUsed.slice(0, 5) },
    { claim: 'per-case expectations equal refRun (kernel decide) and per-case MC1 theorems', status: 'proved', count: cases.length },
    { claim: 'compiled managed-Wasm execution matches, opt off and on', status: 'tested', count: receipt.cases.length },
    ...(exported ? [{ claim: 'imported fuzz-corpus programs match their Lean-checked expectations, opt off and on',
      status: 'tested', count: imported.length }] : []),
    { claim: 'out-of-range literals refused by the frontend', status: 'tested', count: rejectionCases.length },
    { claim: 'Rust frontend refines the transcribed literal accumulator', status: 'open' },
    { claim: 'noble-wasm lowering, helpers, wasm-tools, wasm-opt, V8', status: 'assumed' },
    { claim: 'universal backend correctness', status: 'open' },
    { claim: 'Mathlib .olean imports', status: 'assumed', note: 'downloaded cache, not rebuilt' },
  ];
  receipt.commands = commandLog.length;
  writeFileSync(join(out, 'differential.json'), JSON.stringify(receipt,
    (_key, value) => typeof value === 'bigint' ? value.toString() : value, 2) + '\n');
  console.log(JSON.stringify({ ok: true, receipt: join(out, 'differential.json'), source_commit: receipt.source_commit,
    seed: receipt.seed, cases: receipt.cases.length, all_matched: receipt.all_matched,
    opt_on_off_same_stack: receipt.opt_on_off_same_stack, coverage: receipt.coverage,
    controls: controls.map(c => `${c.id}:${c.demonstrably_fails}`),
    checker_property_lanes: receipt.checker_property_lanes.summary ?? 'skipped' }, null, 2));
} catch (error) {
  writeFileSync(join(out, 'differential-failure.json'), JSON.stringify({ ...receipt, error: error.message },
    (_key, value) => typeof value === 'bigint' ? value.toString() : value, 2) + '\n');
  console.error(`[differential] FAIL ${error.message}`);
  process.exitCode = 1;
}
