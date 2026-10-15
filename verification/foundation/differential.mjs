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
//     --foundation FOUNDATION_CHECK_JSON --output NEW_DIR [--allow-dirty] [--skip-checker-lanes]
//   env -i NODE verification/foundation/differential.mjs --verify RECEIPT_JSON
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { mkdirSync, readFileSync, realpathSync, writeFileSync } from 'node:fs';
import { userInfo } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

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
  'verification/foundation/differential.mjs', 'proofs/foundation/NobleFoundation/Sequence.lean',
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
  const result = spawnSync(command, args, { encoding: 'utf8', maxBuffer: 64 * 1024 * 1024,
    timeout: 3600000, env: childBase, ...options });
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
const MIN = -(2n ** 63n), MAX = 2n ** 63n - 1n;
const wrap = z => BigInt.asIntN(64, z); // K-NUM-01 = MathlibI64.wrap (wrap_eq_knum01)
const inRange = n => n >= MIN && n <= MAX;
/** AOp: {push:n} | 'add' | 'sub' | 'mul'. Mirrors NobleFoundation.AOp.ref (top-first). */
function refStep(op, s, mutant = {}) {
  if (typeof op === 'object') return inRange(op.push) ? [op.push, ...s] : null;
  if (s.length < 2) return null;
  const [y, x, ...rest] = s;
  const value = op === 'add' ? x + y : op === 'mul' ? x * y : (mutant.reversedSub ? y - x : x - y);
  return [wrap(value), ...rest];
}
function refRun(ops, s, mutant) {
  for (const op of ops) { s = refStep(op, s, mutant); if (!s) return null; }
  return s;
}
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
const opSource = op => typeof op === 'object' ? String(op.push) : { add: '+', sub: '-', mul: '*' }[op];
const opsSource = ops => ops.map(opSource).join(' ');
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
const leanInt = n => n < 0n ? `(${n})` : `${n}`;
const leanOp = op => typeof op === 'object' ? `.push ${leanInt(op.push)}` : `.${op}`;
const leanOps = ops => `[${ops.map(leanOp).join(', ')}]`;
function leanProgram(P) {
  if (P.arith) return `(.arith ${leanOps(P.arith)})`;
  if (P.quoted !== undefined) return `(.quoted ${leanInt(P.quoted)})`;
  return `(.compose ${leanProgram(P.compose[0])} ${leanProgram(P.compose[1])})`;
}
const leanList = values => `[${values.map(leanInt).join(', ')}]`;
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
function prng(seed) { // splitmix64
  let state = BigInt.asUintN(64, BigInt(seed));
  return () => {
    state = BigInt.asUintN(64, state + 0x9e3779b97f4a7c15n);
    let z = state;
    z = BigInt.asUintN(64, (z ^ (z >> 30n)) * 0xbf58476d1ce4e5b9n);
    z = BigInt.asUintN(64, (z ^ (z >> 27n)) * 0x94d049bb133111ebn);
    return z ^ (z >> 31n);
  };
}
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
function* simplerValues(n) { for (const v of [0n, 1n, -1n, n / 2n]) if (v !== n) yield v; }
function* shrinkCandidates(c) {
  for (let i = 0; i < c.prefix.length; i++) yield { ...c, prefix: c.prefix.filter((_, j) => j !== i) };
  const listShrinks = function* (key) {
    const list = c[key]; if (!list) return;
    for (let i = 0; i < list.length; i++) yield { ...c, [key]: list.filter((_, j) => j !== i) };
    for (let i = 0; i < list.length; i++) if (typeof list[i] === 'object') {
      for (const v of simplerValues(list[i].push)) yield { ...c, [key]: list.map((op, j) => j === i ? { push: v } : op) };
    }
  };
  yield* listShrinks('ops'); yield* listShrinks('ins');
  if (c.program?.compose) { yield { ...c, program: c.program.compose[0] }; yield { ...c, program: c.program.compose[1] }; }
  if (c.program?.arith) {
    const body = c.program.arith;
    for (let i = 0; i < body.length; i++) yield { ...c, program: { arith: body.filter((_, j) => j !== i) } };
  }
}
/** Bounded greedy shrinking preserving `fails`. Candidates whose reference is a
refusal are skipped so shrinking never changes the kind of disagreement. */
function shrink(c, fails) {
  let current = c, steps = 0;
  outer: while (steps < SHRINK_STEPS) {
    for (const candidate of shrinkCandidates(current)) {
      if (expectation(candidate).outcome !== 'normal') continue; // invalid: not executed, not counted
      if (++steps > SHRINK_STEPS) break outer;
      if (fails(candidate)) { current = candidate; continue outer; }
    }
    break;
  }
  return { case: current, steps };
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
requireThat(output && foundationPath && Number.isInteger(count) && count > 0,
  'usage: differential.mjs --seed N --count K --foundation FOUNDATION_CHECK_JSON --output NEW_DIR [--allow-dirty] [--skip-checker-lanes]');
const out = resolve(output);
mkdirSync(out);
const receipt = { schema: 'noble-foundation-differential/v1', profile: 'Core-Bootstrap / managed-linear-memory',
  fragment: 'I64 literals, + - *, quote, literal blocks, compose, run, nested quote-run-run, Bool/I64 prefix',
  seed: String(seed), count, opt_modes: ['off', 'on'], universal_backend_proof: false };
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
  const env = { ...childBase, TMPDIR: out, CARGO_HOME: join(home, '.cargo'), RUSTC_WRAPPER: '',
    CARGO_TARGET_DIR: join(root, 'target') };
  run(join(rustBin, 'cargo'), ['build', '--locked', '--offline', '-j', '4', '-p', 'noble-cli'],
    { cwd: root, env, timeout: 3600000 });
  const cli = join(root, 'target/debug/noble');
  const tools = { node, wasm_tools: join(selection.tool_paths.wasm_tools.output, 'bin/wasm-tools'),
    wasm_opt: join(selection.tool_paths.binaryen.output, 'bin/wasm-opt') };
  receipt.toolchain = { cli_sha256: fileHash(cli), lean: foundation.lean, mathlib_commit: foundation.mathlib_commit,
    tools: Object.fromEntries(Object.entries(tools).map(([k, p]) => [k, { path: p, sha256: fileHash(p) }])) };

  // Generate cases and their references.
  const make = generator(seed);
  const cases = [...Array.from({ length: count }, (_, i) => make(i)),
    ...rejectionCases.map((c, i) => ({ ...c, index: count + i }))];
  for (const c of cases) { c.source = caseSource(c); c.expected = expectation(c); }
  requireThat(cases.slice(0, count).every(c => c.expected.outcome === 'normal'), 'generator produced an unexpected refusal');

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
  let serial = 0;
  const observe = (c, mode) => {
    const tag = `${c.index}-${mode}-${++serial}`;
    const dir = join(out, `case-${tag}`);
    const file = join(out, `case-${tag}.noble`);
    writeFileSync(file, c.source);
    const result = run(cli, ['run', file, '--opt', mode, '--emit', dir], { cwd: root, env, allowFailure: true });
    const report = JSON.parse(result.stdout);
    const obs = { outcome: report.outcome, stage: report.stage, diagnostic: report.diagnostic ?? null,
      guest_requests: report.guest_requests, trace: report.request_trace ?? [] };
    if (report.outcome === 'normal') {
      requireThat(report.module?.optimized === (mode === 'on'), `case ${c.index}/${mode}: optimization flag mismatch`);
      const wasm = join(dir, 'engine', mode === 'on' ? 'module-1-optimized.wasm' : 'module-1.wasm');
      requireThat(report.module.wasm_sha256 === fileHash(wasm) &&
        report.module.source_sha256 === digest(Buffer.from(c.source)), `case ${c.index}/${mode}: emitted bytes do not bind report`);
      obs.wasm_sha256 = report.module.wasm_sha256;
      obs.stack = report.stack.map(v => [v.type, String(v.value)]);
    }
    return obs;
  };
  const decodeSigned = v => v;
  const decodeUnsigned = ([type, v]) => type === 'I64' ? [type, String(BigInt.asUintN(64, BigInt(v)))] : [type, v];
  const agrees = (expected, obs, decode = x => x) => expected.outcome === 'normal'
    ? obs.outcome === 'normal' && obs.guest_requests === 0 &&
      JSON.stringify(obs.trace) === JSON.stringify(expected.trace) &&
      JSON.stringify(obs.stack.map(decode)) === JSON.stringify(expected.stack)
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
        expected: c.expected, observed: { outcome: obs.outcome, stack: obs.stack, diagnostic: obs.diagnostic },
        wasm_sha256: obs.wasm_sha256 ?? null, lean_theorem: `case_${c.index}_ref`, matched };
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
  requireThat(receipt.all_matched, `mismatch: ${show(receipt.cases.filter(c => !c.matched)).slice(0, 2000)}`);
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
  };

  // Mutation controls: each must disagree with the observations, and only for
  // its intended reason.
  const controls = [];
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
  const refusalFailing = cases.filter(c => c.expected.outcome === 'reject')
    .filter(c => !agrees(wrappedExpectation(c), observations.get(`${c.index}/off`)));
  controls.push({ id: 'literal-wrapping-instead-of-refusal', intended: 'out-of-range literal assumed to wrap',
    detected_cases: refusalFailing.length, demonstrably_fails: refusalFailing.length === rejectionCases.length });
  requireThat(refusalFailing.length === rejectionCases.length, 'refusal control did not fail');
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

  // Checker property lanes (acceptance coverage, not execution semantics).
  if (!skipLanes) {
    const lanes = run(join(rustBin, 'cargo'), ['test', '--locked', '--offline', '-j', '4', '-p', 'noble-kernel',
      '--test', 'property'], { cwd: root, env, timeout: 3600000, allowFailure: true });
    const summary = (lanes.stdout + lanes.stderr).split('\n').filter(l => /^test |test result/.test(l));
    receipt.checker_property_lanes = { command: 'cargo test --locked --offline -p noble-kernel --test property',
      exit: lanes.status, summary, generated_oracle_candidates: 1000, malformed_candidates: 200,
      coverage_class: 'checker acceptance (kernel), NOT execution semantics' };
    requireThat(lanes.status === 0, 'checker property lanes failed');
  } else {
    receipt.checker_property_lanes = { skipped: true };
  }

  const final = gitState(root);
  requireThat(final.head === initial.head && final.tree === initial.tree && final.status === initial.status,
    'Noble source worktree changed during comparison');
  receipt.sources_sha256 = Object.fromEntries(boundSources.map(p => [p, fileHash(join(root, p))]));
  receipt.claims = [
    { claim: 'refRun semantics equal MC1 Exec for the fragment (any prefix)', status: 'proved', theorems: theoremsUsed.slice(0, 5) },
    { claim: 'per-case expectations equal refRun (kernel decide) and per-case MC1 theorems', status: 'proved', count: cases.length },
    { claim: 'compiled managed-Wasm execution matches, opt off and on', status: 'tested', count: receipt.cases.length },
    { claim: 'out-of-range literals refused by the frontend', status: 'tested', count: rejectionCases.length },
    { claim: 'Rust frontend refines the transcribed literal accumulator', status: 'open' },
    { claim: 'noble-wasm lowering, helpers, wasm-tools, wasm-opt, V8', status: 'assumed' },
    { claim: 'universal backend correctness', status: 'open' },
    { claim: 'Mathlib .olean imports', status: 'assumed', note: 'downloaded cache, not rebuilt' },
  ];
  receipt.commands = commandLog.length;
  writeFileSync(join(out, 'differential.json'), JSON.stringify(receipt, null, 2) + '\n');
  console.log(JSON.stringify({ ok: true, receipt: join(out, 'differential.json'), source_commit: receipt.source_commit,
    seed: receipt.seed, cases: receipt.cases.length, all_matched: receipt.all_matched,
    opt_on_off_same_stack: receipt.opt_on_off_same_stack, coverage: receipt.coverage,
    controls: controls.map(c => `${c.id}:${c.demonstrably_fails}`),
    checker_property_lanes: receipt.checker_property_lanes.summary ?? 'skipped' }, null, 2));
} catch (error) {
  writeFileSync(join(out, 'differential-failure.json'), JSON.stringify({ ...receipt, error: error.message }, null, 2) + '\n');
  console.error(`[differential] FAIL ${error.message}`);
  process.exitCode = 1;
}
