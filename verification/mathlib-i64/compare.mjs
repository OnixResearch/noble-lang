// Finite differential check of Noble's compiled, managed-Wasm I64 arithmetic
// against an independently compiled Mathlib ZMod signed-representative model.
// Run with the Node selected by policy/tool-selection.json (see README).
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(fileURLToPath(import.meta.url), '../../..');
const proof = join(root, 'proofs/m3');
const selection = JSON.parse(readFileSync(join(root, 'policy/tool-selection.json')));
const leanBin = join(selection.tool_paths.lean.output, 'bin');
const rustBin = join(selection.tool_paths.quality_rust.output, 'bin');
const node = join(selection.tool_paths.node.output, 'bin/node');
const mathlibRevision = 'fabf563a7c95a166b8d7b6efca11c8b4dc9d911f';
const operations = { add: '+', sub: '-', mul: '*' };
const vectors = [
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

const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const fileHash = path => digest(readFileSync(path));
const requireThat = (condition, message) => { if (!condition) throw Error(message); };
function run(command, args, options = {}) {
  const result = spawnSync(command, args, { encoding: 'utf8', maxBuffer: 8 * 1024 * 1024,
    timeout: 180000, ...options });
  if (result.error || result.status !== 0) {
    throw Error(`${command} ${args.join(' ')} failed: ${result.error?.message ?? result.status}\n${result.stderr}\n${result.stdout}`);
  }
  return result.stdout.trim();
}

requireThat(process.argv.length === 2 || (process.argv.length === 4 && process.argv[2] === '--output'),
  'usage: compare.mjs [--output NEW_DIR]');
const retained = process.argv.length === 4;
const scratch = retained ? resolve(process.argv[3]) : mkdtempSync(join(tmpdir(), 'noble-mathlib-i64-'));
if (retained) mkdirSync(scratch);
try {
  requireThat(realpathSync(process.execPath) === realpathSync(node), `use selected Node: ${node}`);
  const manifest = JSON.parse(readFileSync(join(proof, 'lake-manifest.json')));
  const mathlib = manifest.packages.find(pkg => pkg.name === 'mathlib');
  requireThat(mathlib?.url === 'https://github.com/leanprover-community/mathlib4.git' &&
    mathlib.rev === mathlibRevision && mathlib.inherited === true, 'Mathlib manifest pin differs');
  requireThat(readFileSync(join(proof, 'lean-toolchain'), 'utf8').trim() === 'leanprover/lean4:v4.31.0',
    'Lean toolchain pin differs');
  const mathlibTree = join(proof, '.lake/packages/mathlib');
  requireThat(run('git', ['rev-parse', 'HEAD'], { cwd: mathlibTree }) === mathlibRevision,
    'checked-out Mathlib differs from manifest');
  const commit = run('git', ['rev-parse', 'HEAD'], { cwd: root });
  const lean = join(leanBin, 'lean');
  const lake = join(leanBin, 'lake');
  const version = run(lean, ['--version']);
  requireThat(version.includes('version 4.31.0') &&
    version.includes('68218e876d2a38b1985b8590fff244a83c321783'), 'wrong Lean executable');
  const env = { ...process.env, PATH: `${leanBin}:${process.env.PATH}`, CI: '1',
    TMPDIR: scratch, RUSTC_WRAPPER: '', CARGO_TARGET_DIR: join(root, 'target') };
  run(lake, ['env', 'lean', '-DmaxHeartbeats=1000000', 'MathlibI64.lean'], { cwd: proof, env });
  const model = readFileSync(join(proof, 'MathlibI64.lean'), 'utf8');
  // Only decimal *inputs* and operator names cross this boundary; the
  // expected signed values are computed by Mathlib ZMod inside Lean.
  const oracle = join(scratch, 'Oracle.lean');
  writeFileSync(oracle, model + '\n' + vectors.map(([_, left, right, op]) =>
    `#eval MathlibI64.wrap ((${left} : ℤ) ${operations[op]} (${right} : ℤ))`
  ).join('\n') + '\n');
  const expected = run(lake, ['env', 'lean', '-DmaxHeartbeats=1000000', oracle],
    { cwd: proof, env }).split('\n');
  requireThat(expected.length === vectors.length && expected.every(x => /^-?(0|[1-9][0-9]*)$/.test(x)),
    `invalid Lean oracle output: ${expected.join(', ')}`);

  const cargo = join(rustBin, 'cargo');
  run(cargo, ['build', '--locked', '--offline', '-j', '4', '-p', 'noble-cli'],
    { cwd: root, env: { ...env, PATH: `${rustBin}:${env.PATH}` }, timeout: 900000 });
  const binary = join(root, 'target/debug/noble');
  const toolsExpected = {
    node,
    wasm_tools: join(selection.tool_paths.wasm_tools.output, 'bin/wasm-tools'),
    wasm_opt: join(selection.tool_paths.binaryen.output, 'bin/wasm-opt'),
  };
  function compare(index, mutant = false) {
    const [name, left, right, op] = vectors[index];
    const word = mutant ? '-' : operations[op];
    const source = `${left} ${right} ${word}\n`;
    const sourceFile = join(scratch, `${name}${mutant ? '-mutant' : ''}.noble`);
    const emit = join(scratch, `${name}${mutant ? '-mutant' : ''}-artifacts`);
    writeFileSync(sourceFile, source);
    const report = JSON.parse(run(binary, ['run', sourceFile, '--opt', 'off', '--emit', emit],
      { cwd: root, env }));
    requireThat(report.schema === 'noble-core-report/v1' && report.profile === 'Core-Bootstrap' &&
      report.backend === 'managed-linear-memory' && report.stage === 'wasm' &&
      report.outcome === 'normal' && report.status === 0 && report.module?.optimized === false &&
      report.stack?.length === 1 && report.stack[0]?.type === 'I64' &&
      report.guest_requests === 0, `${name}: not a successful pure Noble Wasm execution`);
    const wasm = readFileSync(join(emit, 'engine/module-1.wasm'));
    requireThat(WebAssembly.validate(wasm), `${name}: emitted Wasm invalid`);
    requireThat(report.module.wasm_sha256 === digest(wasm) &&
      report.module.source_sha256 === digest(Buffer.from(source)) &&
      report.module.wat_sha256 === fileHash(join(emit, 'engine/module-1.wat')) &&
      readFileSync(join(emit, 'engine/module-1.noble'), 'utf8') === source,
    `${name}: emitted executed module/source does not bind the report`);
    const tools = JSON.parse(readFileSync(join(emit, 'engine/tools.json')));
    for (const [key, path] of Object.entries(toolsExpected)) {
      requireThat(tools[key]?.path === path && tools[key].sha256 === fileHash(path),
        `${name}: ${key} not selected or hash differs`);
    }
    return { name, expected: expected[index], actual: report.stack[0].value,
      wasm_sha256: report.module.wasm_sha256, matched: report.stack[0].value === expected[index] };
  }
  const results = vectors.map((_, index) => compare(index));
  requireThat(results.every(result => result.matched),
    `Mathlib mismatch: ${JSON.stringify(results.filter(result => !result.matched))}`);
  const wrong = compare(0, true); // Deliberately use subtraction for max + 1.
  requireThat(!wrong.matched, 'seeded wrong arithmetic escaped comparator');
  const sourceHashes = Object.fromEntries([
    'Cargo.lock', 'crates/noble-cli/src/core/worker.rs',
    'crates/noble-cli/src/core/runtime/config.json',
    'crates/noble-wasm/src/component/lower/operations.rs',
    'crates/noble-wasm/runtime/operations.wat',
    'proofs/m3/MathlibI64.lean', 'proofs/m3/lake-manifest.json',
    'proofs/m3/lakefile.toml', 'proofs/m3/lean-toolchain',
    'policy/tool-selection.json', 'verification/mathlib-i64/compare.mjs',
  ].map(path => [path, fileHash(join(root, path))]));
  const result = { scope: 'finite-implemented-pure-I64-Wasm-only',
    source_commit: commit, lean: version, mathlib_commit: mathlibRevision,
    cli_sha256: fileHash(binary), sources_sha256: sourceHashes,
    cases: results, seeded_wrong_arithmetic_rejected: { ...wrong, matched: false },
    universal_backend_proof: false, exact_calculator_verified: false,
    artifact_directory: retained ? scratch : null };
  if (retained) writeFileSync(join(scratch, 'comparison.json'), JSON.stringify(result, null, 2) + '\n');
  console.log(JSON.stringify(result, null, 2));
} catch (error) {
  console.error(`[mathlib-i64] FAIL ${error.message}`);
  process.exitCode = 1;
} finally {
  if (!retained) rmSync(scratch, { recursive: true, force: true });
}
