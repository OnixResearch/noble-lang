// Record a source-pinned run of the retained WebAssembly core spec fixtures.
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, realpathSync, writeFileSync } from 'node:fs';
import { dirname, isAbsolute, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const selection = JSON.parse(readFileSync(join(root, 'policy/tool-selection.json'), 'utf8'));
const leanBin = join(selection.tool_paths.lean.output, 'bin');
const node = join(selection.tool_paths.node.output, 'bin/node');
const wasmTools = join(selection.tool_paths.wasm_tools.output, 'bin/wasm-tools');
const git = realpathSync('/run/current-system/sw/bin/git');
const fail = message => { throw new Error(message); };
const requireThat = (condition, message) => { if (!condition) fail(message); };
const sha256 = bytes => createHash('sha256').update(bytes).digest('hex');
const gitBlobSha1 = bytes => createHash('sha1').update(`blob ${bytes.length}\0`).update(bytes).digest('hex');

function run(command, args, cwd, env = {}) {
  const result = spawnSync(command, args, { cwd, env, encoding: 'utf8', maxBuffer: 256 * 1024 * 1024 });
  requireThat(result.error === undefined, `${command} ${args[0] ?? ''}: ${result.error}`);
  return result;
}

function main() {
  requireThat(process.argv.length === 6 && process.argv[2] === '--work-dir' && process.argv[3] !== '' &&
      process.argv[4] === '--output' && process.argv[5] !== '',
    'usage: spec-tests.mjs --work-dir ABS_PATH --output NEW_DIR_OUTSIDE_REPOSITORY');
  requireThat(realpathSync(process.execPath) === realpathSync(node), 'process is not the policy-selected Node executable');
  const forbidden = Object.keys(process.env).filter(key => process.env[key] !== '');
  requireThat(forbidden.length === 0, `refusing ambient environment variables: ${forbidden.join(', ')}`);
  const workDir = resolve(process.argv[3]);
  requireThat(isAbsolute(process.argv[3]) && workDir !== root && !workDir.startsWith(`${root}/`),
    '--work-dir must be an absolute path outside the repository');
  const project = join(workDir, 'interpreter');
  const testsuite = join(project, 'vendor/testsuite');
  const prepared = JSON.parse(readFileSync(join(workDir, 'talos-source.json'), 'utf8'));
  requireThat(prepared.schema === 'wasm-pilot-talos-preparation/v1' &&
    prepared.upstream_commit === 'd7c84ac660757953b752bdfc9cb8dae3dab623ed' && /^[0-9a-f]{40}$/.test(prepared.upstream_tree),
    'work directory does not contain the required pinned Talos preparation receipt');
  const source = JSON.parse(readFileSync(join(testsuite, 'SOURCE.json'), 'utf8'));
  requireThat(source.schema === 'wasm-pilot-spec-tests/v1' && /^[0-9a-f]{40}$/.test(source.commit),
    'invalid pinned spec fixture manifest');

  const output = resolve(process.argv[5]);
  requireThat(output !== root && !output.startsWith(`${root}/`), 'output directory must be outside the repository');
  requireThat(!existsSync(output), 'output directory already exists');
  const sourceCommit = run(git, ['rev-parse', 'HEAD'], root).stdout.trim();
  const status = run(git, ['status', '--porcelain'], root).stdout.trim();
  requireThat(status === '', `receipts require a clean committed source tree; status=${JSON.stringify(status)}`);
  requireThat(prepared.source_commit === sourceCommit, 'prepared project belongs to a different Noble source commit');

  for (const item of source.files) {
    requireThat(item.local_path === item.local_path.split('/').at(-1), `fixture path is not a basename: ${item.local_path}`);
    const bytes = readFileSync(join(testsuite, item.local_path));
    requireThat(sha256(bytes) === item.sha256, `${item.local_path}: SHA-256 differs from SOURCE.json`);
    requireThat(gitBlobSha1(bytes) === item.git_blob_sha1, `${item.local_path}: Git blob SHA-1 differs from SOURCE.json`);
  }
  const license = readFileSync(join(testsuite, 'LICENSE'));
  requireThat(sha256(license) === source.license.sha256, 'testsuite LICENSE SHA-256 differs from SOURCE.json');
  requireThat(gitBlobSha1(license) === source.license.git_blob_sha1, 'testsuite LICENSE Git blob SHA-1 differs from SOURCE.json');

  mkdirSync(output);
  const wasmToolsBin = dirname(wasmTools);
  const path = [wasmToolsBin, leanBin, '/run/current-system/sw/bin', '/usr/bin', '/bin'].join(':');
  const env = {
    HOME: output,
    TMPDIR: output,
    LANG: 'C.UTF-8',
    PATH: [leanBin, '/run/current-system/sw/bin', '/usr/bin', '/bin'].join(':'),
  };
  const lake = join(leanBin, 'lake');
  const lean = join(leanBin, 'lean');
  const args = ['env', 'env', `PATH=${path}`, lean, '--run', 'Interpreter/Testsuite.lean', '--json'];
  const result = run(lake, args, project, env);
  requireThat(result.status === 0 || result.status === 1,
    `testsuite setup/execution failed with exit ${result.status}: ${result.stderr}`);
  let rows;
  try { rows = JSON.parse(result.stdout); }
  catch (error) { fail(`testsuite output was not JSON: ${error}`); }
  requireThat(Array.isArray(rows) && rows.length > 0, 'testsuite returned no command results');
  const expectedFiles = new Set(source.files.map(item => item.local_path));
  const observedFiles = new Set(rows.map(row => row.file));
  requireThat(expectedFiles.size === observedFiles.size && [...expectedFiles].every(file => observedFiles.has(file)),
    `testsuite file set differs from SOURCE.json: ${JSON.stringify([...observedFiles])}`);
  requireThat(!rows.some(row => row.kind === 'file_error'), 'testsuite could not load a pinned fixture');
  const outcomes = rows.reduce((counts, row) => {
    counts[row.outcome] = (counts[row.outcome] ?? 0) + 1;
    return counts;
  }, {});
  const failed = rows.filter(row => ['fail', 'interpreter_error', 'out_of_fuel', 'error'].includes(row.outcome));
  const receipt = {
    schema: 'wasm-pilot-spec-test-run/v1',
    issue: 82,
    source_commit: sourceCommit,
    source_worktree_clean: true,
    talos_source_commit: prepared.upstream_commit,
    talos_source_tree: prepared.upstream_tree,
    spec_repository: source.repository,
    spec_commit: source.commit,
    fixture_manifest: join(testsuite, 'SOURCE.json'),
    fixtures: source.files.map(({ local_path, upstream_path, git_blob_sha1, sha256 }) => ({ local_path, upstream_path, git_blob_sha1, sha256 })),
    lean_path: lean,
    lean_version: run(lean, ['--version'], project, env).stdout.trim(),
    wasm_tools_path: wasmTools,
    wasm_tools_version: run(wasmTools, ['--version'], project, env).stdout.trim(),
    command: [lake, ...args],
    exit_code: result.status,
    command_count: rows.length,
    outcomes,
    failed_cases: failed,
    stdout_sha256: sha256(Buffer.from(result.stdout, 'utf8')),
    stderr: result.stderr,
    interpretation: failed.length === 0 ? 'all recorded commands passed' : 'failed spec assertions are Talos/spec discrepancies, not evidence of a Noble miscompilation',
  };
  writeFileSync(join(output, 'spec-tests.json'), `${JSON.stringify(receipt, null, 2)}\n`, { flag: 'wx' });
  const tag = failed.length === 0 ? 'PASS' : 'FAIL';
  process.stdout.write(`[wasm-pilot-spec-tests] ${tag} fixtures=${expectedFiles.size} commands=${rows.length} failed=${failed.length} commit=${sourceCommit}\n`);
  if (failed.length !== 0) process.exitCode = 1;
}

try { main(); }
catch (error) { process.stderr.write(`[wasm-pilot-spec-tests] ERROR ${error.message}\n`); process.exitCode = 2; }
