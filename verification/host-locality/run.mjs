// Issue #51 orchestration: keep Lean model proof, actual execution and identity
// verification separate. A matching receipt is not wrapper/lowering refinement.
import { spawnSync } from 'node:child_process';
import { mkdirSync, readFileSync, realpathSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { digest, fileHash } from '../foundation/execution.mjs';

const root = resolve(fileURLToPath(import.meta.url), '../../..');
const selection = JSON.parse(readFileSync(join(root, 'policy/tool-selection.json')));
const node = join(selection.tool_paths.node.output, 'bin/node');
const git = realpathSync('/run/current-system/sw/bin/git');
const requireThat = (condition, message) => { if (!condition) throw Error(message); };
const json = value => JSON.stringify(value, null, 2) + '\n';
const pureEnv = { PATH: `${dirname(git)}:/run/current-system/sw/bin:/usr/bin:/bin`, GIT_TERMINAL_PROMPT: '0' };
let output;
let serial = 0;
function run(command, argv, { capture = true, allowFailure = false, env = {} } = {}) {
  const observed = spawnSync(command, argv, {
    cwd: root, env, timeout: 3_600_000, maxBuffer: 64 * 1024 * 1024, encoding: null,
  });
  const stdout = observed.stdout ?? Buffer.alloc(0);
  const stderr = observed.stderr ?? Buffer.alloc(0);
  if (capture) {
    const directory = join(output, 'commands', String(++serial).padStart(3, '0'));
    mkdirSync(directory);
    writeFileSync(join(directory, 'stdout'), stdout);
    writeFileSync(join(directory, 'stderr'), stderr);
    writeFileSync(join(directory, 'command.json'), json({
      command, executable_sha256: fileHash(command), argv, cwd: root, exit: observed.status,
      signal: observed.signal, error: observed.error?.message ?? null,
      stdout_sha256: digest(stdout), stderr_sha256: digest(stderr),
    }));
  }
  requireThat(!observed.error && observed.signal === null && (allowFailure || observed.status === 0),
    `${command} ${argv.join(' ')} failed: ${observed.error?.message ?? observed.status}\n${stderr}\n${stdout}`);
  return { exit: observed.status, stdout: stdout.toString('utf8'), stderr: stderr.toString('utf8') };
}
function state() {
  const invoke = argv => run(git, argv, { capture: false, env: pureEnv }).stdout.trim();
  return { source_commit: invoke(['rev-parse', 'HEAD']), source_tree: invoke(['rev-parse', 'HEAD^{tree}']),
    status: invoke(['status', '--porcelain=v1', '--untracked-files=all']) };
}
function artifactPath(directory, file) {
  const path = resolve(directory, file);
  requireThat(path !== directory && path.startsWith(`${directory}${sep}`), `artifact outside receipt directory: ${file}`);
  return path;
}
function verify(receiptPath) {
  const receipt = JSON.parse(readFileSync(receiptPath));
  requireThat(receipt.schema === 'noble-host-locality/v1', 'wrong locality receipt schema');
  const current = state();
  requireThat(current.status === '' && receipt.clean_source === true &&
    current.source_commit === receipt.source_commit && current.source_tree === receipt.source_tree,
    `stale locality receipt: current=${current.source_commit}/${current.source_tree}, receipt=${receipt.source_commit}/${receipt.source_tree}`);
  for (const [file, expected] of Object.entries(receipt.sources_sha256)) {
    requireThat(fileHash(artifactPath(root, file)) === expected, `source changed: ${file}`);
  }
  const directory = dirname(receiptPath);
  for (const [file, expected] of Object.entries(receipt.artifacts_sha256)) {
    requireThat(fileHash(artifactPath(directory, file)) === expected, `retained artifact changed: ${file}`);
  }
  for (const name of ['proof', 'execution']) {
    const binding = receipt.receipts[name];
    const inputPath = artifactPath(directory, binding.path);
    requireThat(fileHash(inputPath) === binding.sha256 && receipt.artifacts_sha256[binding.path] === binding.sha256,
      `${name} primary receipt digest differs`);
    const input = JSON.parse(readFileSync(inputPath));
    requireThat(input.source_commit === receipt.source_commit && input.source_tree === receipt.source_tree && input.clean_source === true,
      `${name} receipt names a different source`);
  }
  return { verified: true, source_commit: receipt.source_commit, source_tree: receipt.source_tree,
    boundary: 'source/receipt/artifact identity only; not a fresh Lean proof replay or Rust/Wasmtime refinement' };
}
function sourceBindings(proof, execution) {
  const combined = {};
  for (const sources of [proof.sources_sha256, execution.sources_sha256]) {
    requireThat(sources && typeof sources === 'object' && !Array.isArray(sources), 'missing source bindings');
    for (const [file, hash] of Object.entries(sources)) {
      requireThat(combined[file] === undefined || combined[file] === hash, `cross-receipt source mismatch: ${file}`);
      combined[file] = hash;
    }
  }
  for (const file of ['verification/host-locality/run.mjs', 'verification/host-locality/check.mjs',
    'verification/host-locality/execution.mjs', 'verification/host-locality/model-inputs.json']) {
    combined[file] = fileHash(join(root, file));
  }
  for (const [file, hash] of Object.entries(combined)) {
    requireThat(fileHash(artifactPath(root, file)) === hash, `source drift after execution: ${file}`);
  }
  return combined;
}

try {
  requireThat(realpathSync(process.execPath) === realpathSync(node), `use selected Node: ${node}`);
  const argv = process.argv.slice(2);
  if (argv.length === 2 && argv[0] === '--verify') {
    console.log(json(verify(resolve(argv[1]))));
  } else {
    const allowDirty = argv.includes('--allow-dirty');
    if (allowDirty) argv.splice(argv.indexOf('--allow-dirty'), 1);
    requireThat(argv.length === 2 && argv[0] === '--output',
      'usage: SELECTED_NODE verification/host-locality/run.mjs [--allow-dirty] --output NEW_EXTERNAL_DIRECTORY | --verify RECEIPT');
    output = resolve(argv[1]);
    requireThat(output !== root && !output.startsWith(`${root}/`), 'output must be external');
    mkdirSync(output); mkdirSync(join(output, 'commands'));
    const initial = state();
    requireThat(allowDirty || initial.status === '', `source worktree dirty: ${initial.status}`);
    const proofDirectory = join(output, 'proof');
    const executionDirectory = join(output, 'execution');
    run(node, ['verification/host-locality/check.mjs', ...(allowDirty ? ['--allow-dirty'] : []), '--output', proofDirectory]);
    run(node, ['verification/host-locality/execution.mjs', executionDirectory, ...(allowDirty ? ['--allow-dirty'] : [])]);
    const proofFile = join(proofDirectory, 'proof.json');
    const executionFile = join(executionDirectory, 'execution.json');
    const proof = JSON.parse(readFileSync(proofFile));
    const execution = JSON.parse(readFileSync(executionFile));
    requireThat(proof.schema === 'noble-host-locality-proof/v1', 'wrong proof receipt schema');
    for (const [name, input] of [['proof', proof], ['execution', execution]]) {
      requireThat(input.source_commit === initial.source_commit && input.source_tree === initial.source_tree &&
        input.clean_source === (initial.status === ''), `${name} source binding differs`);
    }
    const sources = sourceBindings(proof, execution);
    requireThat(JSON.stringify(state()) === JSON.stringify(initial), 'source worktree changed during the pilot');
    const artifacts = { 'proof/proof.json': fileHash(proofFile), 'execution/execution.json': fileHash(executionFile) };
    for (const [name, input, directory] of [['proof', proof, proofDirectory], ['execution', execution, executionDirectory]]) {
      requireThat(input.artifacts_sha256 && typeof input.artifacts_sha256 === 'object', `missing ${name} artifact bindings`);
      for (const [file, hash] of Object.entries(input.artifacts_sha256)) {
        requireThat(fileHash(artifactPath(directory, file)) === hash, `${name} artifact changed: ${file}`);
        artifacts[`${name}/${file}`] = hash;
      }
    }
    const receipt = {
      schema: 'noble-host-locality/v1', issue: 51,
      source_commit: initial.source_commit, source_tree: initial.source_tree, clean_source: initial.status === '',
      receipts: { proof: { path: 'proof/proof.json', sha256: fileHash(proofFile) },
        execution: { path: 'execution/execution.json', sha256: fileHash(executionFile) } },
      sources_sha256: sources, artifacts_sha256: artifacts,
      proof: { theorem_count: proof.theorem_count, reused_m5_theorem_count: proof.reused_m5_theorem_count,
        transitive_axioms: [...new Set(proof.theorems.flatMap(row => row.axioms))].sort(), implementation_theorem: false },
      execution: { case_count: execution.cases.length, controls: execution.controls },
      coverage: {
        model: 'proved bounded adapter/borrow/locality model and reused M5 table lemmas; actual source/hash bindings are in the separate proof receipt',
        wrapper_refinement: 'open: the Wasmtime/anyhow-using CLI wrapper is not the extracted M5 table decision function',
        lowering_refinement: 'open: emitted-component execution and causal controls are finite evidence, not a universal lowering theorem',
        trusted_host: 'explicit locality, response and host-alive premises; no arbitrary malicious-host isolation theorem',
        cleanup: 'selection, invocation and successful retirement are distinct; model permits failure and never invents guest continuation',
      },
      assumptions: [...proof.assumptions, 'Pinned Rust, selected Wasmtime/Canonical ABI and native host process behavior are trusted for finite observations.'],
      artifact_directory: output, command_count: serial,
    };
    writeFileSync(join(output, 'locality.json'), json(receipt));
    console.log(json({ receipt: join(output, 'locality.json'), source_commit: initial.source_commit,
      clean_source: receipt.clean_source, model_theorems: proof.theorem_count,
      reused_m5_theorems: proof.reused_m5_theorem_count, cases: execution.cases.length,
      identity_only_verification: 'run --verify on the same clean source tree' }));
  }
} catch (error) {
  console.error(`[host-locality] FAIL ${error.stack ?? error}`);
  process.exitCode = 1;
}
