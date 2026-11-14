// Differentially drive production-emitted I64 modules through Talos and pinned Node/V8.
// The input set is copied verbatim from mathlib-i64/compare.mjs and linking/run.mjs.
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { createReadStream, mkdirSync, readFileSync, realpathSync, writeFileSync } from 'node:fs';
import { dirname, isAbsolute, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const lane = join(root, 'verification/wasm-refinement-pilot');
const abiPath = join(root, 'crates/noble-cli/src/core/runtime/abi.json');
const selection = JSON.parse(readFileSync(join(root, 'policy/tool-selection.json'), 'utf8'));
const nodePath = join(selection.tool_paths.node.output, 'bin/node');
const wasmTools = join(selection.tool_paths.wasm_tools.output, 'bin/wasm-tools');
const git = realpathSync('/run/current-system/sw/bin/git');
const fail = message => { throw new Error(message); };
const requireThat = (condition, message) => { if (!condition) fail(message); };
const sha256 = bytes => createHash('sha256').update(bytes).digest('hex');
const abi = JSON.parse(readFileSync(abiPath, 'utf8'));

async function hashFile(path) {
  const hash = createHash('sha256');
  for await (const chunk of createReadStream(path)) hash.update(chunk);
  return hash.digest('hex');
}

function run(command, args, cwd = root) {
  const result = spawnSync(command, args, { cwd, env: {}, encoding: null, maxBuffer: 128 * 1024 * 1024 });
  requireThat(result.error === undefined, `${command} ${args[0]}: ${result.error}`);
  requireThat(result.status === 0, `${command} ${args[0]} failed: ${result.stderr.toString('utf8')}`);
  return result.stdout;
}

const compareVectors = [
  ['add-max-wrap', '9223372036854775807', '1', 'add'],
  ['add-min-negative', '-9223372036854775808', '-1', 'add'],
  ['add-two-negatives', '-7', '-13', 'add'],
  ['add-zero', '0', '0', 'add'],
  ['sub-min-wrap', '-9223372036854775808', '1', 'sub'],
  ['sub-max-negative', '9223372036854775807', '-1', 'sub'],
  ['sub-order', '3', '8', 'sub'],
  ['sub-negative', '-7', '-13', 'sub'],
  ['mul-min-negative', '-9223372036854775808', '-1', 'mul'],
  ['mul-max-double', '9223372036854775807', '2', 'mul'],
  ['mul-negatives', '-7', '-13', 'mul'],
  ['mul-zero', '-9223372036854775808', '0', 'mul'],
];
const linkingVectors = [
  { id: 'small-positive', prefix: true, x: '19', y: '4' },
  { id: 'negative', prefix: false, x: '-3', y: '8' },
  { id: 'equal-operands', prefix: true, x: '42', y: '42' },
  { id: 'i64-maximum-wrap', prefix: false, x: '9223372036854775807', y: '-1' },
  { id: 'i64-minimum-wrap', prefix: true, x: '-9223372036854775808', y: '1' },
  { id: 'wide-noncommutative', prefix: false, x: '4611686018427387921', y: '-4611686018427387899' },
];
const sourceOperations = {
  add: { source: '+', types: ['I64', 'I64'] },
  sub: { source: '-', types: ['I64', 'I64'] },
  mul: { source: '*', types: ['I64', 'I64'] },
  linking: { source: '- 7 -', types: ['Bool', 'I64', 'I64'] },
};
const arithmetic = {
  add: (x, y) => x + y,
  sub: (x, y) => x - y,
  mul: (x, y) => x * y,
  linking: (x, y) => x - y - 7n,
};

function makeV8Instance(bytes) {
  const module = new WebAssembly.Module(bytes);
  const imports = Object.create(null);
  const namespace = Object.create(null);
  namespace[abi.memory.name] = new WebAssembly.Memory({ initial: abi.memory.initial, maximum: abi.memory.maximum });
  namespace[abi.table.name] = new WebAssembly.Table({
    element: abi.table.element === 'anyfunc' ? 'anyfunc' : abi.table.element,
    initial: abi.table.initial,
    maximum: abi.table.maximum,
  });
  for (const global of abi.globals) {
    const value = global.type === 'i64' ? BigInt(global.value) : global.value;
    namespace[global.name] = new WebAssembly.Global({ value: global.type, mutable: true }, value);
  }
  imports[abi.module] = namespace;
  const importRows = WebAssembly.Module.imports(module);
  requireThat(importRows.length === 27 && importRows.every(row => row.module === abi.module),
    `expected the exact 27-import Core-Bootstrap ABI, found ${importRows.length}`);
  const instance = new WebAssembly.Instance(module, imports);
  return instance.exports;
}

function callStatus(name, value) {
  requireThat(value === 0, `${name} returned status ${String(value)}`);
}

function runV8(bytes, xText, yText, prefix = null) {
  const e = makeV8Instance(bytes);
  const limits = abi.limits_maximum;
  callStatus('begin', e.begin(limits.allocation_bytes, limits.recipe_leaves, limits.program_depth,
    limits.operand_bytes, limits.continuation_bytes, limits.steps));
  if (prefix !== null) callStatus('push_bool', e.push_bool(prefix ? 1 : 0));
  callStatus('push_i64(x)', e.push_i64(BigInt(xText)));
  callStatus('push_i64(y)', e.push_i64(BigInt(yText)));
  callStatus('submit', e.submit());
  const resultIndex = prefix === null ? 0 : 1;
  const expectedLength = prefix === null ? 1 : 2;
  requireThat(e.stack_length() === expectedLength, `V8 result stack length was ${e.stack_length()}`);
  if (prefix !== null) {
    requireThat(e.stack_kind(0) === 2, 'V8 did not preserve the Bool prefix');
    requireThat(BigInt.asUintN(64, e.stack_value(0)) === (prefix ? 1n : 0n), 'V8 Bool prefix changed');
  }
  requireThat(e.stack_kind(resultIndex) === 1, 'V8 result cell is not I64');
  return BigInt.asUintN(64, e.stack_value(resultIndex));
}

function runTalos(runner, watPath, xText, yText, prefix = null) {
  const args = prefix === null ? [watPath, xText, yText] : [watPath, String(prefix), xText, yText];
  const result = spawnSync(runner, args, { cwd: root, env: {}, encoding: 'utf8', maxBuffer: 1024 * 1024 });
  requireThat(result.error === undefined, `pilot-runner: ${result.error}`);
  requireThat(result.status === 0, `pilot-runner failed for ${watPath} ${args.slice(1).join(' ')}: ${result.stderr}`);
  const match = result.stdout.match(/^PILOT_RESULT ([0-9]+)\s*$/m);
  requireThat(match, `pilot-runner result missing: ${result.stdout}`);
  return BigInt(match[1]);
}

async function main() {
  requireThat(process.argv.length === 10 && process.argv[2] === '--work-dir' && process.argv[4] === '--noble' &&
      process.argv[6] === '--runner' && process.argv[8] === '--output',
    'usage: differential.mjs --work-dir ABS_PATH --noble ABS_PATH --runner ABS_PATH --output NEW_DIR_OUTSIDE_REPOSITORY');
  requireThat(realpathSync(process.execPath) === realpathSync(nodePath), 'process is not the policy-selected Node executable');
  const forbidden = Object.keys(process.env).filter(key => process.env[key] !== '');
  requireThat(forbidden.length === 0, `refusing ambient environment variables: ${forbidden.join(', ')}`);
  const workDir = resolve(process.argv[3]);
  requireThat(isAbsolute(process.argv[3]) && workDir !== root && !workDir.startsWith(`${root}/`),
    '--work-dir must be an absolute path outside the repository');
  const prepared = JSON.parse(readFileSync(join(workDir, 'talos-source.json'), 'utf8'));
  requireThat(prepared.schema === 'wasm-pilot-talos-preparation/v1' &&
    prepared.upstream_commit === 'd7c84ac660757953b752bdfc9cb8dae3dab623ed' && /^[0-9a-f]{40}$/.test(prepared.upstream_tree),
    'work directory does not contain the required pinned Talos preparation receipt');
  const project = join(workDir, 'interpreter');
  const noble = realpathSync(process.argv[5]);
  const runner = realpathSync(process.argv[7]);
  requireThat(runner === realpathSync(join(workDir, 'pilot-runner')),
    'runner must be the wrapper generated for the prepared work directory');
  const output = resolve(process.argv[9]);
  requireThat(output !== root && !output.startsWith(`${root}/`), 'output directory must be outside the repository');
  const sourceCommit = run(git, ['rev-parse', 'HEAD']).toString('utf8').trim();
  const status = run(git, ['status', '--porcelain']).toString('utf8').trim();
  requireThat(status === '', `receipts require a clean committed source tree; status=${JSON.stringify(status)}`);
  requireThat(prepared.source_commit === sourceCommit, 'prepared project belongs to a different Noble source commit');
  mkdirSync(output);
  const work = join(output, 'work');
  mkdirSync(work);
  const compilerSha256 = await hashFile(noble);
  const runnerSha256 = await hashFile(runner);
  requireThat(prepared.runner_wrapper_sha256 === runnerSha256,
    'runner wrapper differs from the prepared workdir receipt');
  const subjectMul = readFileSync(join(project, 'Interpreter/WasmPilot/SubjectMul.lean'), 'utf8');
  const boundMulSha = subjectMul.match(/def artifactSha256 : String := "([0-9a-f]{64})"/)?.[1];
  requireThat(boundMulSha, 'SubjectMul.lean has no artifact digest binding');
  const compiled = new Map();
  for (const [name, spec] of Object.entries(sourceOperations)) {
    const sourcePath = name === 'mul' ? join(lane, 'mul.noble') :
      name === 'linking' ? join(lane, 'linking-diff.noble') : join(work, `${name}.noble`);
    if (name !== 'mul' && name !== 'linking') writeFileSync(sourcePath, spec.source, { flag: 'wx' });
    const args = ['compile', sourcePath, ...spec.types.flatMap(type => ['--input-type', type])];
    const printed = run(noble, args);
    const watPath = join(work, `${name}.wat`);
    const wasmPath = join(work, `${name}.wasm`);
    writeFileSync(watPath, printed, { flag: 'wx' });
    run(wasmTools, ['parse', watPath, '-o', wasmPath], work);
    run(wasmTools, ['validate', wasmPath], work);
    const bytes = readFileSync(wasmPath);
    const roundTripPath = join(work, `${name}-roundtrip.wasm`);
    run(wasmTools, ['parse', watPath, '-o', roundTripPath], work);
    const roundTrip = readFileSync(roundTripPath);
    requireThat(roundTrip.equals(bytes), `${name}: wasm-tools round-trip changed the bytes`);
    const artifactSha256 = sha256(bytes);
    if (name === 'mul') requireThat(artifactSha256 === boundMulSha,
      `x y * artifact digest ${artifactSha256} differs from SubjectMul ${boundMulSha}`);
    compiled.set(name, { bytes, watPath, artifactSha256, textSha256: sha256(printed), sourceSha256: sha256(readFileSync(sourcePath)) });
  }
  const rows = [];
  for (const [id, x, y, operation] of compareVectors) {
    const module = compiled.get(operation);
    const expected = BigInt.asUintN(64, arithmetic[operation](BigInt(x), BigInt(y)));
    const v8 = runV8(module.bytes, x, y);
    const talos = runTalos(runner, module.watPath, x, y);
    requireThat(v8 === expected, `${id}: V8=${v8} expected=${expected}`);
    requireThat(talos === expected, `${id}: Talos=${talos} expected=${expected}`);
    rows.push({ suite: 'mathlib-i64/compare.mjs', id, operation, x, y, expected: String(expected), v8: String(v8), talos: String(talos), artifact_sha256: module.artifactSha256 });
  }
  for (const vector of linkingVectors) {
    const module = compiled.get('linking');
    const expected = BigInt.asUintN(64, arithmetic.linking(BigInt(vector.x), BigInt(vector.y)));
    const v8 = runV8(module.bytes, vector.x, vector.y, vector.prefix);
    const talos = runTalos(runner, module.watPath, vector.x, vector.y, vector.prefix);
    requireThat(v8 === expected, `${vector.id}: V8=${v8} expected=${expected}`);
    requireThat(talos === expected, `${vector.id}: Talos=${talos} expected=${expected}`);
    rows.push({ suite: 'linking/run.mjs arithmetic projection', id: vector.id, operation: 'x y - 7 -', prefix_bool: vector.prefix, x: vector.x, y: vector.y, expected: String(expected), v8: String(v8), talos: String(talos), artifact_sha256: module.artifactSha256 });
  }
  const wrongModule = compiled.get('add');
  const wrongX = '2';
  const wrongY = '3';
  const claimedMultiply = BigInt.asUintN(64, arithmetic.mul(BigInt(wrongX), BigInt(wrongY)));
  const wrongSourceResult = BigInt.asUintN(64, arithmetic.add(BigInt(wrongX), BigInt(wrongY)));
  const wrongV8 = runV8(wrongModule.bytes, wrongX, wrongY);
  const wrongTalos = runTalos(runner, wrongModule.watPath, wrongX, wrongY);
  requireThat(sha256(wrongModule.bytes) === wrongModule.artifactSha256,
    'wrong-operation control digest is not bound to the exact emitted bytes');
  requireThat(wrongV8 === wrongSourceResult && wrongTalos === wrongSourceResult,
    `correct-digest + artifact: V8=${wrongV8} Talos=${wrongTalos} expected source result=${wrongSourceResult}`);
  requireThat(wrongV8 !== claimedMultiply && wrongTalos !== claimedMultiply,
    `correct-digest + artifact unexpectedly matches * claim result ${claimedMultiply}`);
  const wrongArtifactControl = {
    claim: 'x y *',
    emitted_operation: '+',
    source_sha256: wrongModule.sourceSha256,
    artifact_sha256: wrongModule.artifactSha256,
    artifact_digest_is_sha256_of_bytes: true,
    x: wrongX,
    y: wrongY,
    claimed_multiply_result: String(claimedMultiply),
    emitted_add_result: String(wrongSourceResult),
    v8_result: String(wrongV8),
    talos_result: String(wrongTalos),
    kernel_counterexample: 'Interpreter.WasmPilot.model_mul_vs_add_2_3',
    theorem_scope: 'MC1 proves the multiply and add source-model results differ; no theorem yet connects these artifact executions to MC1.',
  };
  rows.push({
    suite: 'correct-digest wrong-artifact control',
    id: 'plus-module-is-wrong-for-multiply-at-2-3',
    operation: '+',
    x: wrongX,
    y: wrongY,
    expected: String(claimedMultiply),
    v8: String(wrongV8),
    talos: String(wrongTalos),
    artifact_sha256: wrongModule.artifactSha256,
    outcome: 'detected-wrong-result',
  });
  const receipt = {
    schema: 'wasm-pilot-differential/v1',
    source_commit: sourceCommit,
    source_worktree_clean: true,
    talos_source_commit: prepared.upstream_commit,
    talos_source_tree: prepared.upstream_tree,
    node_path: realpathSync(process.execPath),
    node_version: process.version,
    v8_version: process.versions.v8,
    wasm_tools_path: wasmTools,
    wasm_tools_version: run(wasmTools, ['--version']).toString('utf8').trim(),
    compiler_path: noble,
    compiler_sha256: compilerSha256,
    talos_runner_path: runner,
    talos_runner_sha256: runnerSha256,
    subject_mul_sha256: boundMulSha,
    wrong_artifact_control: wrongArtifactControl,
    cases: rows,
    passed_cases: rows.length,
    failed_cases: 0,
    scope: 'Core-Bootstrap arithmetic modules only; linking/run vectors exercise the retained Bool prefix and arithmetic projection, not the full declared-module linker.',
  };
  writeFileSync(join(output, 'differential.json'), `${JSON.stringify(receipt, null, 2)}\n`, { flag: 'wx' });
  process.stdout.write(`[wasm-pilot-differential] PASS cases=${rows.length} commit=${sourceCommit} v8=${process.versions.v8}\n`);
}

main().catch(error => { process.stderr.write(`${error.message}\n`); process.exitCode = 1; });
