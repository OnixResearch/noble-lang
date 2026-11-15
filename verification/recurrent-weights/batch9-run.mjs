#!/usr/bin/env node
// Source-bind the separately decided recurrent-weight owner arms to one selected CPU runtime.
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdirSync, readFileSync, realpathSync, writeFileSync } from 'node:fs';
import { release } from 'node:os';
import { isAbsolute, join, relative, resolve } from 'node:path';

function argument(name) {
  const index = process.argv.indexOf(name);
  if (index < 0 || index + 1 >= process.argv.length) throw new Error(`missing ${name}`);
  return process.argv[index + 1];
}
const root = resolve(argument('--root'));
const cargo = resolve(argument('--cargo'));
const output = resolve(argument('--output'));
const work = resolve(argument('--work-dir'));
const selectedWork = resolve(argument('--selected-work-dir'));
const sha = bytes => createHash('sha256').update(bytes).digest('hex');
const outside = path => {
  const rel = relative(root, path);
  if (rel === '' || (rel !== '..' && !rel.startsWith('../') && !isAbsolute(rel))) {
    throw new Error(`output must be outside the source tree: ${path}`);
  }
};
for (const path of [output, work, selectedWork]) outside(path);
if (process.env.NOBLE_ROOT !== root || !process.env.NOBLE_TARGET ||
    !isAbsolute(process.env.NOBLE_TARGET)) throw new Error('pin source root and Cargo target');
outside(process.env.NOBLE_TARGET);
function command(executable, args, options = {}, acceptedStatuses = [0]) {
  const result = spawnSync(executable, args, {
    encoding: 'utf8', maxBuffer: 32 * 1024 * 1024, ...options,
  });
  if (result.error || !acceptedStatuses.includes(result.status)) {
    throw new Error(`${executable} ${args.join(' ')}: ${result.error || result.stderr || result.stdout}`);
  }
  return result;
}
const git = args => command('git', ['-C', root, ...args]).stdout.trim();
if (git(['status', '--porcelain']) !== '') throw new Error('receipt needs clean committed source');
const source_commit = git(['rev-parse', 'HEAD']);
const source_tree = git(['rev-parse', 'HEAD^{tree}']);
const selection = JSON.parse(readFileSync(join(root, 'policy/tool-selection.json')));
const node = selection.tool_paths.node.output + '/bin/node';
if (realpathSync(process.execPath) !== realpathSync(node) ||
    selection.verification_tools.onnx_runtime.version !== '1.23.2' ||
    selection.verification_tools.onnx_python.version !== '3.13.12') {
  throw new Error('unselected Node/ONNX/ORT tool version');
}
const runtime = selection.tool_paths.onnx_runtime;
const interpreter = selection.tool_paths.onnx_python;
const python = interpreter.output + '/bin/python';
const paths = JSON.parse(command('nix', ['path-info', '--offline', '--json-format', '1', '--json',
  runtime.output, interpreter.output]).stdout);
for (const tool of [runtime, interpreter]) {
  if (paths[tool.output]?.deriver !== tool.derivation || !paths[tool.output]?.narHash) {
    throw new Error(`selected ORT/Python output cannot be realized offline: ${tool.output}`);
  }
}
if (!command('nix-store', ['-q', '--requisites', interpreter.output]).stdout.split('\n')
    .includes(runtime.output)) throw new Error('selected Python closure lacks the selected ORT');
const previousPath = join(root, 'verification/recurrent-weights/receipts/2026-11-15/model-state-c61fac3.json');
const previousBytes = readFileSync(previousPath);
const previous = JSON.parse(previousBytes);
if (previous.schema !== 'noble.recurrent-model-state-receipt/v1' || !previous.clean_source ||
    previous.source_commit !== 'c61fac31db362c5a32a4d527eada0cd47964fb84' ||
    previous.selected_tools.onnx_runtime.output !== runtime.output ||
    previous.selected_tools.onnx_python.output !== interpreter.output ||
    command('git', ['-C', root, 'merge-base', '--is-ancestor', previous.source_commit, source_commit]).status !== 0) {
  throw new Error('previous selected checked-export/model-state receipt is not this source ancestor');
}
const selectedExport = join(selectedWork, 'checked-graphs.json');
const exportBytes = readFileSync(selectedExport);
if (sha(exportBytes) !== previous.exporter.checked_graphs_sha256 ||
    sha(exportBytes) !== 'bf84fa4454e7313a9500a4ece04d2fbc8b6f1ddcb3dfb4698a3ae19014f4b4ae') {
  throw new Error('selected checked graph export changed since prior receipt');
}
const specs = JSON.parse(exportBytes);
if (!Array.isArray(specs) || specs.map(spec => spec.program).join(',') !==
    'accumulator,stack-swap') throw new Error('wrong selected checked program roster');
const modelFiles = ['accumulator-scan.onnx', 'stack-swap-scan.onnx'];
const expectedModels = ['a797dcd849805991b43771cf49e9e30a232b324963839d8b76af18dfceb795c8',
  'db793d0d78ee729457f5af5f3476dbff12714c698dc5ba25a9d08d2f5dfbd9ea'];
for (const [index, file] of modelFiles.entries()) {
  if (sha(readFileSync(join(selectedWork, file))) !== expectedModels[index] ||
      previous.model_state.results[index].model_sha256 !== expectedModels[index]) {
    throw new Error(`selected model bytes do not match earlier receipt: ${file}`);
  }
}
mkdirSync(work, { mode: 0o700, recursive: true });
const pyEnv = { HOME: work, TMPDIR: work, PATH: '/usr/bin:/bin', PYTHONNOUSERSITE: '1',
  HTTP_PROXY: 'http://127.0.0.1:1', HTTPS_PROXY: 'http://127.0.0.1:1', NO_PROXY: '*' };
const commands = [];
function pythonReport(script, args, allowMeasuredNoGo = false) {
  const argv = ['-I', join(root, 'verification/recurrent-weights', script), ...args];
  const result = command(python, argv, { cwd: root, env: pyEnv }, allowMeasuredNoGo ? [0, 1] : [0]);
  const lines = result.stdout.trim().split('\n');
  if (lines.length !== 1) throw new Error(`${script} did not produce exactly one JSON report`);
  const report = JSON.parse(lines[0]);
  if (result.status === 1 && report.verdict !== 'NO-GO') {
    throw new Error(`${script} exited nonzero without an explicit measured NO-GO`);
  }
  commands.push({ command: [python, ...argv], exit_code: result.status, report_sha256: sha(result.stdout) });
  return report;
}
const llm = pythonReport('llm-author-run.py', [
  '--prompt', join(root, 'verification/recurrent-weights/llm-author-prompt.txt'),
  '--transcript', join(root, 'verification/recurrent-weights/llm-author-transcript.json'),
  '--compiled-receipt', previousPath, '--output-dir', join(work, 'llm')]);
if (llm.schema !== 'noble-recurrent-llm-author-arm/v1' || llm.decision !== 'GO' ||
    llm.llm_authored.passed !== 16 || llm.llm_authored.kind !== 'ACCEPT' ||
    Object.values(llm.controls).length !== 4 ||
    !Object.values(llm.controls).every(control => control.kind === 'REFUSE')) {
  throw new Error('one-shot LLM author bar or adversarial controls not met');
}
const classifier = pythonReport('sparse-classifier.py', ['--output-dir', join(work, 'classifier')]);
if (classifier.schema !== 'noble.recurrent-sparse-classifier/v1' || classifier.decision !== 'GO' ||
    classifier.checker_verdict.kind !== 'ACCEPT' || classifier.checker_verdict.tested !== 19531 ||
    classifier.checker_verdict.passed !== 19531 || classifier.behavioral_accuracy.correct !== 16 ||
    classifier.behavioral_accuracy.total !== 16 ||
    classifier.model.nonzero_initializer_elements >= 400 ||
    Object.values(classifier.controls).length !== 7 ||
    !Object.values(classifier.controls).every(control => control.kind === 'REFUSE') ||
    Object.values(classifier.input_controls).length !== 8 ||
    !Object.values(classifier.input_controls).every(control => control.kind === 'REFUSE')) {
  throw new Error('independent sparse classifier bar or adversarial controls not met');
}
const lens = pythonReport('neuron-lens.py', ['--export', selectedExport]);
if (lens.schema !== 'noble.recurrent-neuron-lens/v1' ||
    lens.source_export_sha256 !== sha(exportBytes) || lens.results.length !== 2 ||
    lens.results.some((row, index) => row.program !== specs[index].program ||
      row.corpus.cases !== 16 || row.corpus.local_and_selected_ort_source_matches !== 16 ||
      !['GO', 'NO-GO'].includes(row.decision.verdict))) {
  throw new Error('lens audit omitted its predeclared unit map, runtime cases or verdict');
}
const tableArgs = ['test', '-p', 'noble-ngraph', '--test', 'layout_differential', '--offline',
  '--locked', '--', 'recurrent::table_read::checked_index_tiny_near_misses_and_refusals'];
const tableTest = command(cargo, tableArgs, { cwd: root, env: process.env });
if (!tableTest.stdout.includes('test result: ok. 1 passed; 0 failed')) {
  throw new Error('checked F32Index/source/ref test did not pass');
}
commands.push({ command: [cargo, ...tableArgs], exit_code: 0, stdout_sha256: sha(tableTest.stdout) });
const exportArgs = ['test', '-p', 'noble-ngraph', '--test', 'layout_differential', '--offline',
  '--locked', '--', '--ignored', '--nocapture', 'recurrent::table_read::table_read_export'];
const tableStdout = command(cargo, exportArgs, { cwd: root, env: process.env }).stdout;
const prefix = 'RECURRENT_TABLE_EXPORT\t';
const rows = tableStdout.split('\n').filter(line => line.startsWith(prefix));
if (rows.length !== 1 || !tableStdout.includes('test result: ok. 1 passed; 0 failed')) {
  throw new Error('checked table exporter did not emit exactly one successful admitted graph');
}
const tableExport = join(work, 'checked-table.json');
writeFileSync(tableExport, JSON.stringify(JSON.parse(rows[0].slice(prefix.length))) + '\n',
  { flag: 'wx', mode: 0o600 });
commands.push({ command: [cargo, ...exportArgs], exit_code: 0, stdout_sha256: sha(tableStdout) });
const table = pythonReport('table-read.py', ['--export', tableExport,
  '--output-dir', join(work, 'table')]);
if (table.schema !== 'noble.recurrent-table-read/v1' || table.decision !== 'GO' ||
    table.coverage.tiny.transitions !== 17 || table.coverage.near_miss.pairs !== 10 ||
    table.coverage.near_miss.misroutes !== 0 ||
    table.coverage.free_running.transitions !== 64 ||
    table.coverage.free_running.host_calls !== 64 ||
    table.controls.length !== 36 || !table.controls.every(item => item.refused) ||
    !table.seeded_fault.first_counterexample) {
  throw new Error('checked F32Index table pilot omitted required coverage/refusals');
}
const scratch = modelFiles.map((file, index) => pythonReport('peak-scratch.py', [
  '--program', specs[index].program, '--model', join(selectedWork, file),
  '--model-sha256', expectedModels[index], '--export', selectedExport,
  '--export-sha256', sha(exportBytes)], true));
if (scratch.some((report, index) => report.schema !== 'noble.recurrent-peak-scratch/v1' ||
    report.program !== specs[index].program || report.output_match?.mismatches !== 0 ||
    report.output_match?.checked_steps !== 1000 ||
    !['MEASURED', 'NO-GO'].includes(report.verdict) ||
    report.profiler?.nonperturbing_control !== true)) {
  throw new Error('native ORT arena run omitted source correspondence, idle control, or explicit verdict');
}
const criterion = 'verification/recurrent-weights/batch9-criteria.txt';
const sourcePaths = ['llm-author-prompt.txt', 'llm-author-transcript.json', 'llm-author-run.py',
  'sparse-classifier.py', 'neuron-lens.py', 'peak-scratch.py', 'table-read.py', 'batch9-run.mjs'];
const receipt = {
  schema: 'noble.recurrent-batch9-receipt/v1', source_commit, source_tree, clean_source: true,
  criterion, criterion_sha256: sha(readFileSync(join(root, criterion))),
  host: { platform: process.platform, architecture: process.arch, kernel_release: release(),
    node: process.version },
  selected_tools: { source_nixpkgs: selection.sources.nixpkgs,
    onnx_runtime: { ...runtime, narHash: paths[runtime.output].narHash },
    onnx_python: { ...interpreter, narHash: paths[interpreter.output].narHash },
    python, execution_offline: true },
  source_files: Object.fromEntries(sourcePaths.map(path => [path,
    sha(readFileSync(join(root, 'verification/recurrent-weights', path)))])),
  predecessor: { receipt_sha256: sha(previousBytes), source_commit: previous.source_commit,
    selected_checked_graphs_sha256: sha(exportBytes),
    selected_models_sha256: expectedModels },
  commands, llm, classifier, lens, table,
  table_export_sha256: sha(readFileSync(tableExport)),
  scratch,
  coverage: { dense: 'none', sparse: 'independent authored classifier only; not compiled sparse runtime',
    stock: 'two selected Scan programs and checked bounded F32Index table; not optimized long deployment' },
  decision: { llm: llm.decision, classifier: classifier.decision,
    lens: lens.decision, table: table.decision,
    scratch: scratch.map(row => ({ program: row.program, verdict: row.verdict,
      reason: row.reason, net_run_high_water_bytes: row.arena.net_run_high_water_bytes })) },
};
writeFileSync(output, JSON.stringify(receipt, null, 2) + '\n', { flag: 'wx', mode: 0o600 });
console.log(`batch9: table=${table.decision}; LLM=${llm.decision}; sparse=${classifier.decision}; ` +
  `lens=${lens.decision}; scratch=${scratch.map(row => row.verdict).join(',')}; ` +
  `receipt=${output}`);
