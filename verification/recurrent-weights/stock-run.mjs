#!/usr/bin/env node
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { release } from 'node:os';
import { isAbsolute, join, relative, resolve } from 'node:path';

function argument(name) {
  const index = process.argv.indexOf(name);
  if (index < 0 || index + 1 >= process.argv.length) throw new Error(`missing ${name}`);
  return process.argv[index + 1];
}
function exec(command, args, options = {}) {
  const result = spawnSync(command, args, { encoding: 'utf8', maxBuffer: 32 * 1024 * 1024, ...options });
  if (result.error || result.status !== 0) {
    throw new Error(`${command} ${args.join(' ')}: ${result.error || result.stderr || result.stdout}`);
  }
  return result.stdout;
}
const sha = bytes => createHash('sha256').update(bytes).digest('hex');
const root = resolve(argument('--root'));
const cargo = resolve(argument('--cargo'));
const output = resolve(argument('--output'));
const work = resolve(argument('--work-dir'));
const outside = path => {
  const rel = relative(root, path);
  if (rel === '' || (rel !== '..' && !rel.startsWith('../') && !isAbsolute(rel))) {
    throw new Error(`runner output must be outside the source worktree: ${path}`);
  }
};
outside(output);
outside(work);
if (process.env.NOBLE_ROOT !== root || !process.env.NOBLE_TARGET ||
    !isAbsolute(process.env.NOBLE_TARGET)) {
  throw new Error('pin the source root and outside-Cargo target before producing a receipt');
}
outside(process.env.NOBLE_TARGET);
const git = args => exec('git', ['-C', root, ...args]).trim();
if (git(['status', '--porcelain']) !== '') throw new Error('stock receipt requires clean committed source');
const source_commit = git(['rev-parse', 'HEAD']);
const source_tree = git(['rev-parse', 'HEAD^{tree}']);
const selection = JSON.parse(readFileSync(join(root, 'policy/tool-selection.json'), 'utf8'));
const node = selection.tool_paths.node.output + '/bin/node';
if (process.execPath !== node) throw new Error(`unselected Node interpreter: ${process.execPath}`);
const runtime = selection.tool_paths.onnx_runtime;
const interpreter = selection.tool_paths.onnx_python;
if (selection.verification_tools.onnx_runtime.version !== '1.23.2' ||
    selection.verification_tools.onnx_python.version !== '3.13.12') {
  throw new Error('unselected stock-runtime policy version');
}
const paths = JSON.parse(exec('nix', ['path-info', '--offline', '--json-format', '1', '--json',
  runtime.output, interpreter.output]));
for (const tool of [runtime, interpreter]) {
  if (paths[tool.output]?.deriver !== tool.derivation || !paths[tool.output]?.narHash) {
    throw new Error(`selected runtime output/deriver/hash unavailable offline: ${tool.output}`);
  }
}
const closure = exec('nix-store', ['-q', '--requisites', interpreter.output]);
if (!closure.split('\n').includes(runtime.output)) {
  throw new Error('selected Python runtime closure does not contain the selected ORT binary');
}
mkdirSync(work, { recursive: true, mode: 0o700 });
const testArgs = ['test', '-p', 'noble-ngraph', '--test', 'layout_differential',
  '--offline', '--locked', '--', '--ignored', '--nocapture', 'stock_runtime_export'];
const testStdout = exec(cargo, testArgs, { cwd: root, env: process.env });
const rows = testStdout.split('\n').filter(line => line.startsWith('RECURRENT_STOCK_EXPORT\t'));
if (rows.length !== 2 || !testStdout.includes('test result: ok. 1 passed; 0 failed')) {
  throw new Error(`checked graph export omitted an artifact or successful test: ${rows.length}`);
}
const specs = rows.map(line => JSON.parse(line.slice('RECURRENT_STOCK_EXPORT\t'.length)));
const exportPath = join(work, 'checked-graphs.json');
writeFileSync(exportPath, JSON.stringify(specs) + '\n', { flag: 'wx', mode: 0o600 });
const python = interpreter.output + '/bin/python';
const pythonArgs = ['-I', join(root, 'verification/recurrent-weights/stock-runtime.py'),
  '--export', exportPath, '--output-dir', work];
const pythonStdout = exec(python, pythonArgs, {
  cwd: root,
  env: { HOME: work, TMPDIR: work, PATH: '/usr/bin:/bin', PYTHONNOUSERSITE: '1',
    HTTP_PROXY: 'http://127.0.0.1:1', HTTPS_PROXY: 'http://127.0.0.1:1', NO_PROXY: '*' },
});
const lines = pythonStdout.trim().split('\n');
if (lines.length !== 1) throw new Error('expected one stock-runtime JSON result');
const report = JSON.parse(lines[0]);
if (report.schema !== 'noble.recurrent-stock-runtime/v1' || report.results.length !== 2 ||
    report.results.some(result => result.provider.join(',') !== 'CPUExecutionProvider' ||
      result.refusals.length !== 24 || !result.refusals.every(control => control.refused) ||
      result.coverage.tiny.transitions !== 16 ||
      result.coverage.teacher_forced.transitions !== 1000 ||
      result.coverage.free_running.transitions !== 1000)) {
  throw new Error('stock result omitted a required coverage or refusal');
}
const receipt = {
  schema: 'noble.recurrent-stock-receipt/v1', source_commit, source_tree,
  clean_source: true, criterion: 'verification/recurrent-weights/stock-runtime-criteria.txt',
  criterion_sha256: sha(readFileSync(join(root, 'verification/recurrent-weights/stock-runtime-criteria.txt'))),
  host: { platform: process.platform, architecture: process.arch, kernel_release: release(), node: process.version },
  selected_tools: { source_nixpkgs: selection.sources.nixpkgs,
    onnx_runtime: { ...runtime, narHash: paths[runtime.output].narHash },
    onnx_python: { ...interpreter, narHash: paths[interpreter.output].narHash },
    python: python, execution_offline: true },
  exporter: { command: [cargo, ...testArgs], test_result: '1 passed; 0 failed',
    stdout_sha256: sha(Buffer.from(testStdout)), checked_graphs_sha256: sha(readFileSync(exportPath)),
    checked_graphs_path: exportPath },
  stock: { command: [python, ...pythonArgs], exit_code: 0, ...report },
  note: 'Separate bounded ONNX stock CPU execution, not dense/sparse optimized model coverage or a hybrid serving checkpoint.',
};
writeFileSync(output, JSON.stringify(receipt, null, 2) + '\n', { flag: 'wx', mode: 0o600 });
console.log(`stock-runtime: passed; programs=${report.results.length}; ` +
  `stock-transitions=${report.results.reduce((sum, row) => sum +
    Object.values(row.coverage).reduce((n, group) => n + group.transitions, 0), 0)}; ` +
  `refusals=${report.results.reduce((sum, row) => sum + row.refusals.length, 0)}; ` +
  `receipt=${output}`);
