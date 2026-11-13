import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { mkdirSync, readFileSync, readdirSync, realpathSync, writeFileSync, existsSync } from 'node:fs';
import { basename, dirname, isAbsolute, join, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(fileURLToPath(import.meta.url), '../../..');
const modulePath = 'proofs/foundation/NobleFoundation/TaskScope.lean';
const corpusPath = 'proofs/foundation/NobleFoundation/TaskScopeCorpus.lean';
const foundationReportPath = 'foundation/foundation-check.json';
const foundationProofFiles = [
  'proofs/foundation/NobleFoundation.lean',
  'proofs/foundation/README.md',
  'proofs/foundation/inventory.json',
];
const testPaths = [
  'crates/noble-kernel/tests/m6-interleaving.rs',
  'crates/noble-kernel/tests/m6_interleaving/support.rs',
  'crates/noble-kernel/tests/m6_interleaving/capacity.rs',
  'crates/noble-kernel/tests/m6_interleaving/scope/manager.rs',
  'crates/noble-kernel/tests/m6_interleaving/scope/lifecycle.rs',
  'crates/noble-kernel/tests/m6_interleaving/scope/controls/drop.rs',
  'crates/noble-kernel/tests/m6_interleaving/scope/registry.rs',
  'crates/noble-kernel/tests/m6_interleaving/scope/support.rs',
  'crates/noble-kernel/tests/m6_interleaving/scope/traces.rs',
  'crates/noble-kernel/tests/m6_interleaving/scope/corpus/retirement.rs',
  'crates/noble-kernel/tests/m6_interleaving/scope/controls.rs',
  'crates/noble-kernel/tests/m6_interleaving/scope/controls/ownership.rs',
  'crates/noble-kernel/tests/m6_interleaving/scope/controls/ownership/cleanup.rs',
  'crates/noble-kernel/tests/m6_interleaving/scope/controls/foreign.rs',
  'crates/noble-kernel/tests/m6_interleaving/scope/corpus.rs',
  'crates/noble-kernel/tests/m6_interleaving/scope/corpus/completion.rs',
  'crates/noble-kernel/tests/m6_interleaving/scope/corpus/sequence.rs',
  'crates/noble-kernel/tests/m6_interleaving/scope/corpus/cancel.rs',
  'crates/noble-kernel/tests/m6_interleaving/scope/corpus/emission.rs',
];
const changeFiles = [
  'proposal.md', 'design.md', 'tasks.md', 'metadata.json', 'specs/resource-adapters/spec.md',
];
const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const requireThat = (condition, message) => { if (!condition) throw Error(message); };

function command(executable, argv, env) {
  const raw = spawnSync(executable, argv, {
    cwd: root, env, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024,
  });
  requireThat(!raw.error && raw.status === 0,
    `${basename(executable)} ${argv.join(' ')} failed (status ${raw.status}):\n${raw.stderr}\n${raw.stdout}`);
  const combined = `${raw.stdout}\n${raw.stderr}`;
  const resultLines = combined.split(/\r?\n/)
    .filter(line => /test result:|Finished `|lean-proofs|proofs passed|theorems|PASS|error:|warning:/i.test(line))
    .slice(-40);
  return {
    argv: [executable, ...argv],
    exit_code: raw.status,
    result_lines: resultLines,
    stdout_sha256: digest(raw.stdout),
    stderr_sha256: digest(raw.stderr),
  };
}

function outputDirectory(requested) {
  const resolved = resolve(requested);
  const destination = join(realpathSync(dirname(resolved)), basename(resolved));
  const realRoot = realpathSync(root);
  requireThat(destination !== realRoot && !destination.startsWith(`${realRoot}${sep}`),
    'the output directory must be outside the worktree');
  mkdirSync(destination);
  mkdirSync(join(destination, 'lean-proofs'));
  return destination;
}

function findChangeDirectory() {
  const active = join(root, '.cairn/changes/m6-task-scope');
  if (existsSync(active)) return active;
  const archiveRoot = join(root, '.cairn/archive');
  const archived = readdirSync(archiveRoot)
    .filter(entry => entry.endsWith('-m6-task-scope'))
    .map(entry => join(archiveRoot, entry));
  requireThat(archived.length === 1, `expected one active or archived m6-task-scope package, found ${archived.length}`);
  return archived[0];
}

function bounds() {
  const source = readFileSync(join(root, modulePath), 'utf8');
  const adapter = readFileSync(join(root, 'crates/noble-kernel/tests/m6_interleaving/scope/manager.rs'), 'utf8');
  const alphabetBody = source.match(/def scopeAlphabet : List Step :=\s*\[([\s\S]*?)\]/)?.[1];
  const depth = Number(source.match(/def scopeDepth : Nat := (\d+)/)?.[1]);
  const sequenceCount = Number(source.match(/theorem scope_bounded_sequence_count : scopeBoundedSequenceCount = (\d+)/)?.[1]);
  const children = Number(adapter.match(/const MAX_CHILDREN: usize = (\d+);/)?.[1]);
  requireThat(alphabetBody && Number.isSafeInteger(depth) && Number.isSafeInteger(sequenceCount)
    && Number.isSafeInteger(children), 'could not extract the declared model and adapter bounds');
  const alphabet = [...alphabetBody.matchAll(/\.([A-Za-z]+)\s+([A-Za-z0-9]+)/g)]
    .map(match => `${match[1]} ${match[2]}`);
  requireThat(children === 2 && alphabet.length === 15 && depth === 3 && sequenceCount === 3616,
    `unexpected scope bounds: children=${children}, alphabet=${alphabet.length}, depth=${depth}, sequences=${sequenceCount}`);
  return { children, alphabet, depth, bounded_sequences_including_empty: sequenceCount };
}

function main() {
  const [requested] = process.argv.slice(2);
  requireThat(requested,
    'usage: POLICY_SELECTED_NODE verification/task-scope/run.mjs NEW_EXTERNAL_DIRECTORY');
  const selection = JSON.parse(readFileSync(join(root, 'policy/tool-selection.json'), 'utf8'));
  const selectedNode = join(selection.tool_paths.node.output, 'bin/node');
  requireThat(realpathSync(process.execPath) === realpathSync(selectedNode),
    'run this script with the policy-selected Node');
  const cargoRunner = process.env.NOBLE_CARGO_RUNNER;
  const proofRunner = process.env.NOBLE_PROOF_RUNNER;
  const targetDirectory = process.env.NOBLE_TARGET;
  const temporaryDirectory = process.env.TMPDIR;
  requireThat(cargoRunner && isAbsolute(cargoRunner),
    'NOBLE_CARGO_RUNNER must name the approved Cargo command');
  requireThat(proofRunner && isAbsolute(proofRunner),
    'NOBLE_PROOF_RUNNER must name the approved Lean proof command');
  requireThat(targetDirectory && isAbsolute(targetDirectory),
    'NOBLE_TARGET must be an absolute external build directory');
  requireThat(temporaryDirectory && isAbsolute(temporaryDirectory),
    'TMPDIR must be an absolute private temporary directory');
  const out = outputDirectory(requested);
  const realRoot = realpathSync(root);
  const realTargetParent = realpathSync(dirname(targetDirectory));
  const resolvedTarget = join(realTargetParent, basename(targetDirectory));
  requireThat(resolvedTarget !== realRoot && !resolvedTarget.startsWith(`${realRoot}${sep}`),
    'NOBLE_TARGET must be outside the worktree');
  const rustEnv = {
    ...process.env,
    NOBLE_ROOT: root,
    NOBLE_TARGET: resolvedTarget,
    TMPDIR: temporaryDirectory,
    CARGO_BUILD_JOBS: '4',
  };
  const leanEnv = { ...process.env, NOBLE_ROOT: root, TMPDIR: temporaryDirectory };
  const gitOutput = argv => {
    const raw = spawnSync('git', argv, { cwd: root, encoding: 'utf8' });
    requireThat(!raw.error && raw.status === 0, `git ${argv.join(' ')} failed: ${raw.stderr}`);
    return raw.stdout;
  };
  const state = () => ({
    commit: gitOutput(['rev-parse', 'HEAD']).trim(),
    tree: gitOutput(['rev-parse', 'HEAD^{tree}']).trim(),
    status: gitOutput(['status', '--porcelain=v1', '--untracked-files=all']),
  });
  const before = state();
  requireThat(before.status === '', `the source worktree is dirty:\n${before.status}`);
  const changeDirectory = findChangeDirectory();
  const changeRelative = changeDirectory.slice(root.length + 1);
  const packageHashes = Object.fromEntries(changeFiles.map(file => [
    file, digest(readFileSync(join(changeDirectory, file))),
  ]));
  const changeRevision = digest(changeFiles.map(file => `${file}\0${packageHashes[file]}`).join('\n'));
  const declaredBounds = bounds();
  const cargoCommands = [
    command(cargoRunner, ['test', '-p', 'noble-kernel', '--test', 'm6-interleaving', '--test', 'm6-async', '--offline', '--locked'], rustEnv),
    command(cargoRunner, ['test', '-p', 'noble-cli', '--bin', 'noble', '--all-features', '--offline', '--locked', '--', 'worker_program_guest'], rustEnv),
  ];
  const leanCommand = command(proofRunner, [
    'lean', '3', 'verification/ci/lean-proofs.sh', '--allow-dirty', join(out, 'lean-proofs'),
  ], leanEnv);
  const foundationReport = JSON.parse(readFileSync(join(out, 'lean-proofs', foundationReportPath), 'utf8'));
  requireThat(foundationReport.clean_source === true
    && foundationReport.source_commit === before.commit
    && foundationReport.source_tree === before.tree,
  'the foundation proof report is not bound to this clean source commit and tree');
  requireThat(JSON.stringify(state()) === JSON.stringify(before), 'the source changed during the evidence run');
  const boundFiles = [modulePath, corpusPath, ...foundationProofFiles, ...testPaths,
    'verification/task-scope/README.md', 'verification/task-scope/run.mjs',
    ...changeFiles.map(file => `${changeRelative}/${file}`)];
  const receipt = {
    schema: 'noble-task-scope-receipt/v1',
    issue: 115,
    clean_source: true,
    source_commit: before.commit,
    source_tree: before.tree,
    change_package: {
      path: changeRelative,
      revision_sha256: changeRevision,
      files_sha256: packageHashes,
    },
    foundation_module: {
      model: modulePath,
      generated_adapter_corpus: corpusPath,
      files_sha256: Object.fromEntries([modulePath, corpusPath, ...foundationProofFiles].map(file => [
        file, digest(readFileSync(join(root, file))),
      ])),
      proof_receipt: {
        path: foundationReportPath,
        sha256: digest(readFileSync(join(out, 'lean-proofs', foundationReportPath))),
        source_commit: foundationReport.source_commit,
        source_tree: foundationReport.source_tree,
        theorem_count: foundationReport.theorem_count,
        lean: foundationReport.lean,
        lean_sha256: foundationReport.lean_sha256,
        allowed_axioms: foundationReport.allowed_axioms,
      },
    },
    declared_bounds: declaredBounds,
    evidence: {
      model_proofs: {
        status: 'passed',
        command: leanCommand,
        scope: 'The whole-tree pinned Lean proof lane passed; TaskScope and TaskScopeCorpus are included in the foundation project.',
      },
      adapter_runtime: {
        status: 'passed',
        commands: cargoCommands,
      },
    },
    host_assumptions: [
      'One serialized host owner and unique host-issued scope identities.',
      'The host authenticates native-stop observation and keeps the retirement registry alive to perform inherited cleanup.',
      'The adapter exercises finite traces and the existing M6 Table; it does not provide production ExecutionScope integration.',
    ],
    non_claims: [
      'No Rust-to-Lean refinement theorem or universal correspondence is established.',
      'No nesting, more than two children, fairness, liveness, native authenticity, eventual native completion, or cleanup after host death is claimed.',
      'The existing ExecutionScope tests are regressions only, not an implementation of task groups.',
    ],
    files_sha256: Object.fromEntries(boundFiles.map(file => [file, digest(readFileSync(join(root, file)))])),
  };
  const receiptPath = join(out, 'receipt.json');
  writeFileSync(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`, { flag: 'wx' });
  console.log(JSON.stringify({
    result: 'passed', receipt: receiptPath, source_commit: before.commit,
    change_revision_sha256: changeRevision, bounds: declaredBounds,
    runtime_commands: cargoCommands.length, lean_lane_exit_code: leanCommand.exit_code,
    receipt_sha256: digest(readFileSync(receiptPath)),
  }));
}

try {
  main();
} catch (error) {
  console.error(`[task-scope] FAIL ${error.message}`);
  process.exitCode = 1;
}
