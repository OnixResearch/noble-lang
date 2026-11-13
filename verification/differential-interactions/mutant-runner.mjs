// Execute every frozen issue-125 mutant in place in one detached scratch worktree and one target.
// The caller creates that single linked worktree; this runner never clones or copies the repository.
// Usage: SELECTED_NODE verification/differential-interactions/mutant-runner.mjs \
//   --root SCRATCH_WORKTREE --output NEW_EXTERNAL_DIR --target-dir ONE_TARGET \
//   --cargo PINNED_CARGO_WRAPPER --node SELECTED_NODE --foundation FOUNDATION_CHECK_JSON \
//   --corpus MINIMISED_SOURCE_CORPUS
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, readdirSync, realpathSync, statSync, unlinkSync, writeFileSync } from 'node:fs';
import { userInfo } from 'node:os';
import { dirname, join, relative, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';

const scriptRoot = resolve(fileURLToPath(import.meta.url), '../../..');
const policy = JSON.parse(readFileSync(join(scriptRoot, 'policy/tool-selection.json'), 'utf8'));
const selectedNode = join(policy.tool_paths.node.output, 'bin/node');
const selectedGit = realpathSync('/run/current-system/sw/bin/git');
const { homedir, username } = userInfo();
const args = process.argv.slice(2);
const option = name => { const at = args.indexOf(name); return at < 0 ? undefined : args[at + 1]; };
const rootArg = option('--root');
const outputArg = option('--output');
const targetArg = option('--target-dir');
const cargoArg = option('--cargo');
const nodeArg = option('--node');
const foundationArg = option('--foundation');
const corpusArg = option('--corpus');
const requireThat = (condition, message) => { if (!condition) throw Error(message); };
const invalidMutant = message => Object.assign(new Error(message),
  { mutationOutcome: 'invalid-or-equivalent' });
const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const hashFile = path => digest(readFileSync(path));
const root = rootArg && resolve(rootArg);
const output = outputArg && resolve(outputArg);
const targetDir = targetArg && resolve(targetArg);
const cargo = cargoArg && resolve(cargoArg);
const node = nodeArg && resolve(nodeArg);
const foundation = foundationArg && resolve(foundationArg);
const corpus = corpusArg && resolve(corpusArg);
const git = (argv, cwd = root) => {
  const raw = spawnSync(selectedGit, argv, { cwd, encoding: 'utf8', timeout: 60000,
    env: { HOME: homedir, USER: username, LOGNAME: username, LANG: 'C.UTF-8', PATH: process.env.PATH } });
  if (raw.error || raw.status !== 0) throw Error(`git ${argv.join(' ')} failed: ${raw.stderr ?? raw.error}`);
  return raw.stdout.trim();
};
const excludedFrom = (base, path) => {
  const part = relative(base, path);
  return part === '' || (!part.startsWith(`..${sep}`) && part !== '..' && !part.startsWith(sep));
};
const writeProcess = (directory, command, argv, result) => {
  mkdirSync(directory, { recursive: true });
  writeFileSync(join(directory, 'stdout'), result.stdout ?? '');
  writeFileSync(join(directory, 'stderr'), result.stderr ?? '');
  writeFileSync(join(directory, 'command.json'), `${JSON.stringify({ command, argv,
    status: result.status, signal: result.signal, error: result.error ? String(result.error) : null }, null, 2)}\n`);
};
const selectedEnv = tmp => ({ HOME: homedir, USER: username, LOGNAME: username, LANG: 'C.UTF-8', CI: '1',
  PATH: [dirname(selectedGit), '/run/current-system/sw/bin', '/usr/bin', '/bin'].join(':'), TMPDIR: tmp });
const runNode = (argv, label, tmp) => {
  const result = spawnSync(node, argv, { cwd: root, env: selectedEnv(tmp), encoding: 'utf8',
    maxBuffer: 128 * 1024 * 1024, timeout: 3600000 });
  writeProcess(join(output, 'commands', label), node, argv, result);
  return result;
};
const cargoEnv = (tmp, mutation = {}) => ({ ...selectedEnv(tmp), NOBLE_ROOT: root, NOBLE_TARGET: targetDir,
  ...mutation });
const runCargo = (argv, label, tmp, mutation = {}) => {
  const result = spawnSync(cargo, argv, { cwd: root, env: cargoEnv(tmp, mutation), encoding: 'utf8',
    maxBuffer: 128 * 1024 * 1024, timeout: 3600000 });
  writeProcess(join(output, 'commands', label), cargo, argv, result);
  return result;
};
const markerJson = (text, marker) => {
  const line = text.split('\n').map(row => row.trim()).find(row => row.startsWith(marker));
  return line ? JSON.parse(line.slice(marker.length)) : null;
};
const apply = (mutant, action) => {
  const path = join(root, mutant.source_path);
  if (!excludedFrom(root, path) || !existsSync(path)) {
    throw invalidMutant(`${mutant.id}: mutation source is absent or outside the scratch worktree`);
  }
  const original = readFileSync(path);
  const source = original.toString('utf8');
  const occurrences = source.split(mutant.before).length - 1;
  if (occurrences !== 1) {
    throw invalidMutant(`${mutant.id}: expected one exact mutation site, found ${occurrences}`);
  }
  const changed = source.replace(mutant.before, mutant.after);
  if (changed === source || changed.includes(mutant.before)) {
    throw invalidMutant(`${mutant.id}: replacement is unchanged or not unique`);
  }
  let result;
  try {
    writeFileSync(path, changed);
    result = action({ path, beforeSha256: digest(original), afterSha256: hashFile(path), original });
  } finally {
    writeFileSync(path, original);
    requireThat(hashFile(path) === digest(original), `${mutant.id}: source restoration failed`);
    requireThat(git(['status', '--porcelain=v1', '--untracked-files=all']) === '',
      `${mutant.id}: scratch worktree did not restore cleanly`);
  }
  return result;
};
const p1MismatchOutcome = result => result?.mutation_probe?.witness?.outcome ?? 'incomplete';

try {
  requireThat(args.length === 14 && root && output && targetDir && cargo && node && foundation && corpus,
    'usage: mutant-runner.mjs --root SCRATCH_WORKTREE --output NEW_DIR --target-dir TARGET --cargo WRAPPER --node NODE --foundation CHECK_JSON --corpus CORPUS_DIR');
  requireThat(realpathSync(process.execPath) === realpathSync(selectedNode), `runner must use selected Node ${selectedNode}`);
  requireThat(realpathSync(node) === realpathSync(selectedNode), 'the --node argument differs from the selected Node');
  requireThat(realpathSync(git(['rev-parse', '--show-toplevel'])) === realpathSync(root),
    '--root is not this Git worktree root');
  requireThat(statSync(join(root, '.git')).isFile() && git(['branch', '--show-current']) === '',
    'mutants require one detached linked scratch worktree, never the branch or primary checkout');
  requireThat(git(['status', '--porcelain=v1', '--untracked-files=all']) === '',
    'scratch worktree must be clean before the campaign');
  requireThat(!existsSync(output), 'campaign output directory must be new');
  requireThat(!excludedFrom(root, output) && !excludedFrom(root, targetDir),
    'campaign output and the single target must be outside the scratch worktree');
  requireThat(realpathSync(corpus) && statSync(corpus).isDirectory(), 'the minimized source corpus must be a directory');
  requireThat(existsSync(foundation), 'the clean foundation-check receipt does not exist');
  mkdirSync(output);
  const tmp = join(output, 'tmp');
  mkdirSync(tmp);
  const targetTree = git(['rev-parse', 'HEAD^{tree}']);
  const sourceCommit = git(['rev-parse', 'HEAD']);
  const matrixPath = join(root, 'verification/differential-interactions/matrix-v1.json');
  const mutantPath = join(root, 'verification/differential-interactions/mutants-v1.json');
  const matrix = JSON.parse(readFileSync(matrixPath, 'utf8'));
  const bank = JSON.parse(readFileSync(mutantPath, 'utf8'));
  requireThat(matrix.schema === 'noble-differential-interactions-matrix/v1' &&
    matrix.frozen_before_campaign === true && bank.schema === 'noble-differential-interactions-mutants/v1',
  'matrix or mutant bank is not frozen in the expected version');
  const mutants = bank.mutants;
  requireThat(mutants.length === 8 && mutants.every(mutant => mutant.before && mutant.after && mutant.cell),
    'the frozen matrix-v1 mutant bank is incomplete');
  const nodeVersion = spawnSync(node, ['--version'], { encoding: 'utf8', env: selectedEnv(tmp) });
  requireThat(nodeVersion.status === 0, 'selected Node version could not be read');
  const rustc = join(policy.tool_paths.quality_rust.output, 'bin/rustc');
  const rustcVersion = spawnSync(rustc, ['--version', '--verbose'],
    { encoding: 'utf8', env: selectedEnv(tmp) });
  requireThat(rustcVersion.status === 0, 'selected rustc identity could not be read');
  const casesPath = join(output, 'p1-cases.json');
  const exportResult = runNode([join(root, 'verification/fuzz/export-differential.mjs'),
    '--corpus', corpus, '--output', casesPath], 'p1-export', tmp);
  requireThat(exportResult.status === 0, `P1 corpus export failed: ${exportResult.stderr}`);

  const p1BaseDir = join(output, 'p1-baseline');
  mkdirSync(p1BaseDir);
  const p1BaseResult = runNode([join(root, 'verification/foundation/differential.mjs'),
    '--seed', '125', '--count', '18', '--foundation', foundation, '--output', p1BaseDir,
    '--import', casesPath, '--target-dir', targetDir, '--skip-checker-lanes'], 'p1-baseline', tmp);
  requireThat(p1BaseResult.status === 0, `P1 baseline failed: ${p1BaseResult.stderr}`);
  const p1BaselinePath = join(p1BaseDir, 'differential.json');
  const p1Baseline = JSON.parse(readFileSync(p1BaselinePath, 'utf8'));
  requireThat(p1Baseline.clean_source && p1Baseline.all_matched === true &&
    p1Baseline.controls?.length === 7 && p1Baseline.controls.every(control => control.demonstrably_fails) &&
    p1Baseline.interaction_matrix?.reached === 18 && p1Baseline.interaction_matrix?.selected === 18,
  'P1 clean baseline did not reach all frozen cells and existing controls');

  const p2Command = ['test', '--locked', '--offline', '-p', 'noble-ngraph-experiment',
    '--test', 'b2_differential', 'issue125_structured_interaction_matrix', '--', '--nocapture'];
  const p2BaseResult = runCargo(p2Command, 'p2-baseline', tmp);
  requireThat(p2BaseResult.status === 0, `P2 baseline failed: ${p2BaseResult.stderr}`);
  const p2Baseline = markerJson(p2BaseResult.stdout, 'ISSUE125_INTERACTION_RECEIPT=');
  requireThat(p2Baseline?.reached === 24 && p2Baseline?.selected === 24,
    'P2 clean baseline did not reach all frozen cells');
  const p2BaselineHash = p2Baseline.test_executable_blake3;

  const p1Mutants = [];
  const p2Mutants = [];
  let allKilled = true;
  for (const mutant of mutants) {
    const mutantLabel = mutant.id.replaceAll(/[^A-Za-z0-9_-]/g, '_');
    if (mutant.profile === 'P1-core-source-wasm') {
      const mutantDir = join(output, 'p1-mutants', mutantLabel);
      mkdirSync(mutantDir, { recursive: true });
      let entry;
      try {
        entry = apply(mutant, ({ afterSha256 }) => {
        const verify = runNode([join(root, 'verification/foundation/differential.mjs'), '--verify', p1BaselinePath],
          `${mutantLabel}-historical-check`, tmp);
        const stale = JSON.parse(verify.stdout);
        const verified = stale?.verified === true;
        const candidateOut = join(mutantDir, 'run');
        mkdirSync(mutantDir, { recursive: true });
        const mutationResult = runNode([join(root, 'verification/foundation/differential.mjs'),
          '--seed', '125', '--count', '18', '--foundation', foundation, '--output', candidateOut,
          '--import', casesPath, '--target-dir', targetDir, '--allow-dirty', '--skip-checker-lanes',
          '--mutation-probe', '--mutation-id', mutant.id, '--mutation-cell', mutant.cell],
        `${mutantLabel}-mutation`, tmp);
        const receiptPath = join(candidateOut, 'differential.json');
        const failurePath = join(candidateOut, 'differential-failure.json');
        const failure = existsSync(receiptPath) ? null : existsSync(failurePath)
          ? JSON.parse(readFileSync(failurePath, 'utf8')) : null;
        const receipt = existsSync(receiptPath) ? JSON.parse(readFileSync(receiptPath, 'utf8')) : null;
        const witness = receipt?.mutation_probe?.witness;
        let replay = null;
        if (witness?.outcome === 'killed') {
          const witnessPath = join(candidateOut, 'witness.json');
          writeFileSync(witnessPath, `${JSON.stringify({ ...witness, mutation_id: mutant.id,
            target_cell: mutant.cell }, null, 2)}\n`);
          const replayOut = join(mutantDir, 'fresh-replay');
          const replayResult = runNode([join(root, 'verification/differential-interactions/replay-p1.mjs'),
            '--witness', witnessPath, '--output', replayOut, '--target-dir', targetDir],
          `${mutantLabel}-fresh-replay`, tmp);
          if (replayResult.status === 0) replay = JSON.parse(readFileSync(join(replayOut, 'replay-p1.json'), 'utf8'));
        }
        const baselineHash = p1Baseline.sources_sha256[mutant.source_path];
        const candidateHash = receipt?.sources_sha256?.[mutant.source_path] ?? afterSha256;
        const changed = candidateHash !== baselineHash;
        const candidateCliHash = receipt?.mutation_probe?.witness?.candidate_cli_sha256 ?? null;
        const outcome = p1MismatchOutcome(receipt);
        const binaryEquivalent = candidateCliHash !== null && candidateCliHash === p1Baseline.toolchain.cli_sha256;
        const result = { id: mutant.id, profile: mutant.profile, family: mutant.family, cell: mutant.cell,
          mutation: mutant,
          outcome: binaryEquivalent ? 'invalid-or-equivalent'
            : outcome === 'killed' && mutationResult.status === 0 && changed && verify.status !== 0 && !verified
              ? 'killed' : outcome === 'divergence-unreached' ? 'divergence-unreached'
                : outcome === 'executed-survived' ? 'executed-survived' : 'incomplete',
          command_exit: mutationResult.status, clean_source_commit: sourceCommit,
          source_path: mutant.source_path, baseline_source_sha256: baselineHash,
          mutant_source_sha256: candidateHash, source_bytes_changed: changed,
          candidate_cli_sha256: candidateCliHash,
          failure_identity: witness?.failure_identity ?? null,
          witness_sizes_bytes: witness?.witness_sizes_bytes ?? null,
          shrink_work: witness?.shrink_work ?? null,
          mutation_probe_witness: witness ?? null,
          historical_receipt_refused: verify.status !== 0 && !verified,
          historical_check_exit: verify.status, historical_check_json: stale,
          replay: replay && { source_commit: replay.source_commit, candidate_cli_sha256: replay.candidate_cli_sha256,
            replayed: replay.replayed, corpus_regenerated: replay.corpus_regenerated },
          error: failure?.error ?? (mutationResult.status !== 0 ? mutationResult.stderr : null) };
        allKilled &&= result.outcome === 'killed' && replay !== null &&
          replay.replayed.length === 2 && replay.replayed.every(item => item.positive_execution_steps);
        return result;
        });
      } catch (error) {
        if (error?.mutationOutcome !== 'invalid-or-equivalent') throw error;
        const sourcePath = resolve(root, mutant.source_path);
        const sourceSha256 = excludedFrom(root, sourcePath) && existsSync(sourcePath)
          ? hashFile(sourcePath) : null;
        allKilled = false;
        entry = { id: mutant.id, profile: mutant.profile, family: mutant.family, cell: mutant.cell,
          mutation: mutant,
          outcome: 'invalid-or-equivalent', command_exit: null, clean_source_commit: sourceCommit,
          source_path: mutant.source_path, baseline_source_sha256: sourceSha256, mutant_source_sha256: null,
          source_bytes_changed: false, error: error.message };
      }
      p1Mutants.push(entry);
    } else if (mutant.profile === 'P2-ngraph-f32') {
      const mutantDir = join(output, 'p2-mutants', mutantLabel);
      mkdirSync(mutantDir, { recursive: true });
      const replayPath = join(root, 'verification/differential-interactions/replay.json');
      requireThat(!existsSync(replayPath), 'temporary P2 replay file already exists in scratch worktree');
      let entry;
      try {
        try {
          entry = apply(mutant, ({ beforeSha256, afterSha256 }) => {
        const mutationResult = runCargo(p2Command, `${mutantLabel}-mutation`, tmp,
          { NOBLE_ISSUE125_MUTATION_ID: mutant.id, NOBLE_ISSUE125_MUTATION_CELL: mutant.cell });
        const receipt = markerJson(mutationResult.stdout, 'ISSUE125_INTERACTION_RECEIPT=');
        const probe = receipt?.mutation_probe;
        let replay = null;
        if (probe?.outcome === 'killed') {
          const target = receipt.cells.find(cell => cell.cell === mutant.cell);
          requireThat(target && target.minimized_witness?.input_bits,
            `${mutant.id}: killed receipt omitted minimized inputs`);
          const replayData = {
            schema: 'noble-differential-interactions-p2-replay/v1', replay_mode: 'semantic-mutant',
            mutation_id: mutant.id, cell: mutant.cell, interaction_family: target.interaction_family,
            fp_class: target.fp_class, graph_id: target.graph_id, graph_text: target.graph_text,
            graph_text_blake3: target.graph_text_blake3, input_bits: target.input_bits,
            selected_branch: target.selected_branch,
            selected_branch_input_slot: target.selected_branch_input_slot,
            selected_branch_input_bits: target.selected_branch_input_bits,
            index_input_slot: target.index_input_slot, index_input_bits: target.index_input_bits,
            selected_table_index: target.selected_table_index, selected_table_node: target.selected_table_node,
            original_input_bits: target.input_bits, minimized_input_bits: target.minimized_witness.input_bits,
            expected_output_bits: target.expected_output_bits, expected_mismatch_class: target.mismatch_class,
            failure_identity: probe.failure_identity, snapshot: null,
            candidate_test_executable_blake3: probe.candidate_test_executable_blake3,
            historical_test_executable_blake3: p2BaselineHash,
            candidate_source_sha256: afterSha256, baseline_source_sha256: beforeSha256,
            corpus_regenerated: false,
          };
          try {
            writeFileSync(replayPath, `${JSON.stringify(replayData, null, 2)}\n`);
            const fresh = runCargo(['test', '--locked', '--offline', '-p', 'noble-ngraph-experiment',
              '--test', 'b2_differential', 'issue125_replay_retained_witness', '--', '--ignored', '--nocapture'],
            `${mutantLabel}-fresh-replay`, tmp);
            if (fresh.status === 0) replay = markerJson(fresh.stdout, 'ISSUE125_REPLAY=');
            else replay = { error: fresh.stderr, exit: fresh.status };
          } finally {
            if (existsSync(replayPath)) unlinkSync(replayPath);
          }
        }
        const candidateHash = probe?.candidate_test_executable_blake3 ?? null;
        const sourceChanged = afterSha256 !== beforeSha256;
        const binaryEquivalent = candidateHash !== null && candidateHash === p2BaselineHash;
        const outcome = binaryEquivalent ? 'invalid-or-equivalent'
          : probe?.outcome === 'killed' && mutationResult.status === 0 && sourceChanged ? 'killed'
            : sourceChanged && (probe?.outcome === 'divergence-unreached' || probe?.outcome === 'executed-survived')
              ? probe.outcome : 'incomplete';
        const result = { id: mutant.id, profile: mutant.profile, family: mutant.family, cell: mutant.cell,
          mutation: mutant,
          outcome,
          command_exit: mutationResult.status, clean_source_commit: sourceCommit,
          source_path: mutant.source_path, baseline_source_sha256: beforeSha256,
          mutant_source_sha256: afterSha256, source_bytes_changed: afterSha256 !== beforeSha256,
          test_executable_blake3: candidateHash,
          historical_test_executable_blake3: p2BaselineHash,
          historical_executable_refused: probe?.candidate_test_executable_blake3 !== p2BaselineHash,
          failure_identity: probe?.failure_identity ?? null,
          mutation_probe: probe ?? null,
          original_input_bits: receipt?.cells.find(cell => cell.cell === mutant.cell)?.input_bits ?? null,
          minimized_witness: receipt?.cells.find(cell => cell.cell === mutant.cell)?.minimized_witness ?? null,
          witness_sizes_bytes: probe ? { original: probe.original_witness_size_bytes,
            minimized: probe.minimized_witness_size_bytes } : null,
          shrink_work: probe?.shrink_work ?? null,
          replay: replay && { fresh_process: replay.fresh_process_replay === true,
            replayed: replay.replayed ?? null, historical_executable_refused: replay.historical_executable_refused ?? false },
          error: mutationResult.status !== 0 ? mutationResult.stderr : null };
        allKilled &&= result.outcome === 'killed' && replay?.fresh_process_replay === true &&
          replay?.historical_executable_refused === true && replay?.replayed?.length === 2;
          return result;
          });
        } catch (error) {
          if (error?.mutationOutcome !== 'invalid-or-equivalent') throw error;
          const sourcePath = resolve(root, mutant.source_path);
          const sourceSha256 = excludedFrom(root, sourcePath) && existsSync(sourcePath)
            ? hashFile(sourcePath) : null;
          allKilled = false;
          entry = { id: mutant.id, profile: mutant.profile, family: mutant.family, cell: mutant.cell,
            mutation: mutant,
            outcome: 'invalid-or-equivalent', command_exit: null, clean_source_commit: sourceCommit,
            source_path: mutant.source_path, baseline_source_sha256: sourceSha256, mutant_source_sha256: null,
            source_bytes_changed: false, historical_test_executable_blake3: p2BaselineHash,
            error: error.message };
        }
      } finally {
        if (existsSync(replayPath)) unlinkSync(replayPath);
      }
      p2Mutants.push(entry);
    } else {
      throw Error(`${mutant.id}: unsupported mutant profile ${mutant.profile}`);
    }
  }
  requireThat(git(['status', '--porcelain=v1', '--untracked-files=all']) === '',
    'scratch worktree is not clean after the mutant bank');
  const report = {
    schema: 'noble-differential-interactions-campaign/v1',
    source_commit: sourceCommit,
    source_tree: targetTree,
    clean_baseline: true,
    scratch_worktree_is_detached_linked_worktree: true,
    one_scratch_worktree: true,
    one_target_directory: targetDir,
    toolchain: { node: { path: node, version: nodeVersion.stdout.trim(), sha256: hashFile(node) },
      rustc: { path: rustc, version: rustcVersion.stdout.trim(), sha256: hashFile(rustc) },
      cargo_command: { path: cargo, sha256: hashFile(cargo) },
      selection_sha256: hashFile(join(root, 'policy/tool-selection.json')) },
    matrix: { path: 'verification/differential-interactions/matrix-v1.json', sha256: hashFile(matrixPath),
      p1_seed: 125, p1_cases_per_mode: 18, p1_selected: 18, p2_selected: 24 },
    mutant_bank: { path: 'verification/differential-interactions/mutants-v1.json', sha256: hashFile(mutantPath),
      count: mutants.length },
    baseline_receipts: { p1: p1Baseline, p2: p2Baseline },
    command_runs: readdirSync(join(output, 'commands')).sort().map(label => ({ label,
        ...JSON.parse(readFileSync(join(output, 'commands', label, 'command.json'), 'utf8')) })),
    inputs: { foundation_receipt: foundation, corpus: corpus, exported_cases: casesPath },
    baselines: { p1: { receipt: p1BaselinePath, source_tree: p1Baseline.source_tree,
        source_commit: p1Baseline.source_commit, clean_source: p1Baseline.clean_source,
        foundation_receipt_sha256: p1Baseline.foundation_receipt_sha256,
        imported_cases_sha256: p1Baseline.imported.cases_sha256,
        cli_sha256: p1Baseline.toolchain.cli_sha256, toolchain: p1Baseline.toolchain,
        opt_modes: p1Baseline.opt_modes, seed: p1Baseline.seed, count: p1Baseline.count,
        matrix_sha256: p1Baseline.interaction_matrix.matrix_sha256,
        selected: p1Baseline.interaction_matrix.selected, reached: p1Baseline.interaction_matrix.reached,
        generator_comparison: p1Baseline.generator_comparison },
      p2: { matrix_sha256: p2Baseline.matrix_sha256,
        test_executable: p2Baseline.test_executable,
        test_executable_blake3: p2BaselineHash, selected: p2Baseline.selected,
        reached: p2Baseline.reached, generator_comparison: p2Baseline.generator_comparison } },
    mutants: [...p1Mutants, ...p2Mutants],
    outcome_classes: matrix.outcome_classes,
    all_selected_mutants_killed_and_replayed: allKilled,
    scratch_restored_cleanly: true,
  };
  writeFileSync(join(output, 'campaign.json'), `${JSON.stringify(report, null, 2)}\n`);
  process.stdout.write(`${JSON.stringify({ ok: allKilled, report: join(output, 'campaign.json'),
    source_commit: sourceCommit, p1_mutants: p1Mutants.map(({ id, outcome }) => ({ id, outcome })),
    p2_mutants: p2Mutants.map(({ id, outcome }) => ({ id, outcome })),
    scratch_restored_cleanly: true }, null, 2)}\n`);
  if (!allKilled) process.exitCode = 1;
} catch (error) {
  console.error(`[issue125-mutants] FAIL ${error.message}`);
  process.exitCode = 1;
}
