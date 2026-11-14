// Finite Plausible pilot and fresh-process replay, not a proof tactic or theorem source.
// Run on a clean commit with a foundation receipt for that same source tree.
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { mkdirSync, readFileSync, realpathSync, writeFileSync } from 'node:fs';
import { userInfo } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createCliObserver } from '../../foundation/execution.mjs';
import { shrinkCandidates } from '../../foundation/shrinking.mjs';
import { invalidP1Case, p1Eligible, p1Expected, p1RandomCell, renderP1Case } from '../p1-cases.mjs';

const root = resolve(fileURLToPath(import.meta.url), '../../../..');
const lane = join(root, 'verification/differential-interactions/plausible');
const project = join(root, 'proofs/foundation');
const policy = JSON.parse(readFileSync(join(root, 'policy/tool-selection.json')));
const protocolPath = join(lane, 'matrix-v1.json');
const protocol = JSON.parse(readFileSync(protocolPath));
const originalMatrix = JSON.parse(readFileSync(join(root, protocol.source_matrix)));
const bank = JSON.parse(readFileSync(join(root, 'verification/differential-interactions/mutants-v1.json')));
const node = join(policy.tool_paths.node.output, 'bin/node');
const lake = join(policy.tool_paths.lean.output, 'bin/lake');
const git = realpathSync('/run/current-system/sw/bin/git');
const { homedir: home, username } = userInfo();
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const fileHash = path => hash(readFileSync(path));
const requireThat = (ok, message) => { if (!ok) throw Error(message); };
const show = value => JSON.stringify(value, (_, x) => typeof x === 'bigint' ? String(x) : x);
const revive = value => Array.isArray(value) ? value.map(revive) : value && typeof value === 'object'
  ? Object.fromEntries(Object.entries(value).map(([key, v]) => [key, key === 'push' ? BigInt(v) : revive(v)])) : value;
const args = process.argv.slice(2);
const option = name => { const at = args.indexOf(name); return at < 0 ? undefined : args[at + 1]; };
const output = option('--output') && resolve(option('--output'));
const target = option('--target-dir') && resolve(option('--target-dir'));
const replayPath = option('--replay') && resolve(option('--replay'));
const foundation = option('--foundation') && resolve(option('--foundation'));
const env = { HOME: home, USER: username, LOGNAME: username, LANG: 'C.UTF-8', CI: '1',
  PATH: [join(policy.tool_paths.lean.output, 'bin'),
    join(policy.tool_paths.quality_rust.output, 'bin'), dirname(git),
    '/run/current-system/sw/bin', '/usr/bin', '/bin'].join(':') };
function run(command, argv, options = {}) {
  const result = spawnSync(command, argv, { cwd: root, env, encoding: 'utf8',
    maxBuffer: 128 * 1024 * 1024, timeout: 3600000, ...options });
  if (result.error || (result.status !== 0 && !options.allowFailure))
    throw Error(`${command} ${argv.join(' ')}: ${result.error ?? result.status}\n${result.stderr}\n${result.stdout}`);
  return result;
}
const gitValue = argv => run(git, argv).stdout.trim();
const observer = (directory, cli) => createCliObserver({ root, out: directory, cli, env: { ...env,
  TMPDIR: directory }, run });
function mismatch(expected, observed) {
  if (observed.outcome !== 'normal') return `unexpected-${observed.outcome}`;
  if (show(expected.stack) === show(observed.stack)) return null;
  const prefixLength = expected.stack.length - 1;
  if (prefixLength > 0 && show(expected.stack.slice(0, prefixLength)) !==
    show(observed.stack.slice(0, prefixLength))) return 'ordered-prefix-output-mismatch';
  if (expected.stack.length !== observed.stack.length) return 'ordered-output-shape-mismatch';
  return 'ordered-arithmetic-output-mismatch';
}
function observeFault(model, source, mode, observe, tag) {
  const expected = p1Expected(model);
  requireThat(expected.outcome === 'normal' && source === renderP1Case(model),
    'a candidate lost independent reference admission or source/model byte identity');
  const actual = observe({ ...model, index: tag, source }, mode);
  const executed = actual.outcome === 'normal' && actual.metrics.execution_steps > 0;
  requireThat(executed, 'candidate was refused or did not execute');
  requireThat(show(actual.stack) === show(expected.stack), 'clean CLI differs from independent reference');
  // The same observable fault as the retained p1-reversed-observed-stack bank entry.
  // This is a TEST-SIDE injected output fault, not a changed implementation binary.
  const faulty = { ...actual, stack: [...actual.stack].reverse() };
  return { expected, observed: { outcome: actual.outcome, stack: actual.stack,
    execution_steps: actual.metrics.execution_steps, wasm_sha256: actual.wasm_sha256 },
    injected: { stack: faulty.stack, failure_identity: mismatch(expected, faulty) }, executed };
}
function witness(model, source, mode, observe, tag) {
  return { source, source_sha256: hash(Buffer.from(source)), source_size_bytes: Buffer.byteLength(source),
    model: JSON.parse(show(model)), mode, ...observeFault(model, source, mode, observe, tag) };
}
function replay(receipt, directory, cli) {
  requireThat(receipt.schema === 'noble-plausible-pilot/v1' && receipt.source_tree === gitValue(['rev-parse', 'HEAD^{tree}']) &&
    receipt.toolchain.cli_sha256 === fileHash(cli) && receipt.protocol_sha256 === fileHash(protocolPath),
  'source/protocol/executable identity differs from retained witness');
  const observe = observer(directory, cli);
  const results = [receipt.mutant_control.original, receipt.mutant_control.minimized].map((entry, index) => {
    requireThat(hash(Buffer.from(entry.source)) === entry.source_sha256, 'retained case bytes changed');
    const model = revive(entry.model);
    requireThat(model.interaction?.cell === receipt.mutant_control.cell && p1Eligible(model),
      'retained case lost its frozen interaction or premise');
    const result = witness(model, entry.source, entry.mode, observe, `replay-${index}`);
    requireThat(result.injected.failure_identity === receipt.mutant_control.failure_identity &&
      show(result.expected) === show(entry.expected) && show(result.observed.stack) === show(entry.observed.stack),
    'replayed case did not reproduce its original attributed fault and baseline');
    return { label: index ? 'minimized' : 'original', source_sha256: entry.source_sha256,
      source_size_bytes: entry.source_size_bytes, failure_identity: result.injected.failure_identity,
      execution_steps: result.observed.execution_steps, matched_clean_reference: true };
  });
  const result = { schema: 'noble-plausible-replay/v1', source_commit: receipt.source_commit,
    source_tree: receipt.source_tree, cli_sha256: fileHash(cli), generator_called: false,
    fresh_process: true, replayed: results };
  writeFileSync(join(directory, 'replay.json'), `${JSON.stringify(result, null, 2)}\n`);
  return result;
}

requireThat(output && target && (replayPath ? !foundation : foundation),
  'usage: run.mjs --foundation CHECK_JSON --output NEW_DIR --target-dir TARGET | --replay PILOT_JSON --output NEW_DIR --target-dir TARGET');
requireThat(realpathSync(process.execPath) === realpathSync(node), `use selected Node ${node}`);
requireThat(args.length === 6, 'unexpected pilot arguments');
requireThat(protocol.schema === 'noble-plausible-interaction-pilot/v1' &&
  originalMatrix.profiles[0].selected_axes.quotation_composition.includes('composed') &&
  protocol.selected_cells.length === 4 && protocol.cases_per_arm === 16,
'frozen P1 protocol is absent or incompatible with source matrix');
mkdirSync(output, { recursive: false });
const outputTmp = join(output, 'tmp');
mkdirSync(outputTmp);
const childEnv = { ...env, TMPDIR: outputTmp };
const cli = join(target, 'debug/noble');
const clean = gitValue(['status', '--porcelain=v1', '--untracked-files=all']);
requireThat(clean === '', `pilot source worktree dirty: ${clean}`);
if (replayPath) {
  const result = replay(JSON.parse(readFileSync(replayPath)), output, cli);
  console.log(JSON.stringify({ ok: true, replayed: result.replayed.length, generator_called: false }));
} else {
  const initialCommit = gitValue(['rev-parse', 'HEAD']);
  const initialTree = gitValue(['rev-parse', 'HEAD^{tree}']);
  const proof = JSON.parse(readFileSync(foundation));
  requireThat(proof.source_tree === initialTree && proof.schema === 'noble-foundation-check/v1',
    'foundation expectation receipt is stale');
  const driverPath = join(lane, 'Driver.lean');
  const generated = run(lake, ['env', 'lean', '--run', driverPath, '--seed', String(protocol.seed),
    '--count', String(protocol.cases_per_arm), '--size', String(protocol.size)],
  { cwd: project, env: childEnv });
  writeFileSync(join(output, 'samples.jsonl'), generated.stdout);
  writeFileSync(join(output, 'generator.stderr'), generated.stderr);
  const rows = generated.stdout.trim().split('\n').map(line => JSON.parse(line));
  requireThat(rows.length === protocol.cases_per_arm && rows.every(row => row.positive_control &&
    row.false_property_refuted && row.index >= 0), 'Plausible generation controls or count failed');
  const cellCounts = new Map();
  const cases = rows.map(row => {
    const model = revive(row.model);
    requireThat(model.interaction?.cell === protocol.selected_cells[row.cell_index] &&
      p1Eligible(model) && renderP1Case(model) === row.source, 'sample did not preserve its declared typed case and exact source');
    cellCounts.set(model.interaction.cell, (cellCounts.get(model.interaction.cell) ?? 0) + 1);
    return { ...model, origin: { unit: 'issue125-plausible-v1',
      text: row.source, sha256: hash(Buffer.from(row.source)) } };
  });
  requireThat(protocol.selected_cells.every(cell => cellCounts.get(cell) === 4),
    'generated distribution differs from four cases per frozen interaction');
  const importPath = join(output, 'import.json');
  writeFileSync(importPath, `${show({ schema: 'noble-fuzz-differential-cases/v1',
    corpus: 'issue125-plausible-p1-v1', counts: { seed: protocol.seed, size: protocol.size,
      cases_per_arm: protocol.cases_per_arm }, cases, invalid_cases: [invalidP1Case()] })}\n`);
  const comparisonDir = join(output, 'comparison');
  const compare = run(node, [join(root, 'verification/foundation/differential.mjs'),
    '--seed', String(protocol.seed), '--count', String(protocol.cases_per_arm),
    '--foundation', foundation, '--output', comparisonDir, '--import', importPath,
    '--target-dir', target, '--skip-checker-lanes'], { env: childEnv });
  writeFileSync(join(output, 'comparison.stdout'), compare.stdout);
  writeFileSync(join(output, 'comparison.stderr'), compare.stderr);
  const comparison = JSON.parse(readFileSync(join(comparisonDir, 'differential.json')));
  requireThat(comparison.all_matched && comparison.clean_source && comparison.interaction_matrix.reached === 4 &&
    comparison.controls.length === 7 && comparison.controls.every(c => c.demonstrably_fails) &&
    comparison.p1_invalid_control.refused && comparison.generator_comparison.same_compute_seed_budget.equal_case_run_budget,
  'existing admitted comparison, oracle controls or budget failed');
  const generatedRows = comparison.cases.filter(row => row.corpus_unit?.unit === 'issue125-plausible-v1');
  requireThat(generatedRows.length === 32 && generatedRows.every(row => row.matched &&
    row.interaction_witness?.positive_execution_steps && row.interaction_witness?.property_engaged),
  'a generated case was not actually executed and checked in both modes');
  const current = comparison.generator_comparison.current_generator.reached_cells
    .filter(cell => protocol.selected_cells.includes(cell));
  const selected = comparison.interaction_matrix.cells.filter(cell => cell.reached).map(cell => cell.cell);
  const targetCase = cases.filter(c => c.interaction.cell === protocol.selected_cells[0])
    .reduce((a, b) => a.origin.text.length >= b.origin.text.length ? a : b);
  const targetModel = { ...targetCase }; delete targetModel.origin;
  const observe = observer(join(output, 'shrink'), cli);
  mkdirSync(join(output, 'shrink'));
  const mode = 'off';
  const original = witness(targetModel, targetCase.origin.text, mode, observe, 'original');
  const identity = original.injected.failure_identity;
  requireThat(identity === 'ordered-prefix-output-mismatch', 'the selected semantic output mutant survived');
  const controls = {
    profile_changed_rejected: !p1Eligible({ ...targetModel,
      interaction: { ...targetModel.interaction, profile: 'P2-ngraph-f32' } }),
    interaction_removed_rejected: !p1Eligible({ ...targetModel, ops: [{ push: 2n }, { push: 3n }, 'add'] }),
    required_prefix_dropped_rejected: !p1Eligible({ ...targetModel, prefix: targetModel.prefix.slice(0, 1) }),
    earlier_refusal_substitution_rejected: mismatch(original.expected, { outcome: 'reject' }) !== identity,
    unrelated_failure_substitution_rejected: mismatch(original.expected,
      { outcome: 'normal', stack: original.expected.stack.map((entry, index) =>
        index === original.expected.stack.length - 1 ? ['I64', '999'] : entry) }) !== identity,
  };
  requireThat(Object.values(controls).every(Boolean), 'the shrink substitution controls accepted an invalid candidate');
  let minimized = original;
  let currentModel = targetModel;
  let attempts = 0;
  const rejected = { reference_refusal: 0, interaction_or_premise_removed: 0,
    different_failure: 0, not_smaller: 0 };
  let localMinimum = false;
  while (attempts < 48) {
    let improved = false;
    for (const candidate of shrinkCandidates(currentModel)) {
      if (attempts >= 48) break;
      attempts++;
      const source = renderP1Case(candidate);
      if (!p1Eligible(candidate)) { rejected.interaction_or_premise_removed++; continue; }
      if (p1Expected(candidate).outcome !== 'normal') { rejected.reference_refusal++; continue; }
      if (Buffer.byteLength(source) >= minimized.source_size_bytes) { rejected.not_smaller++; continue; }
      const result = witness(candidate, source, mode, observe, `shrink-${attempts}`);
      if (result.injected.failure_identity !== identity) { rejected.different_failure++; continue; }
      minimized = result; currentModel = candidate; improved = true; break;
    }
    if (!improved) { localMinimum = true; break; }
  }
  const mutant = bank.mutants.find(m => m.id === 'p1-reversed-observed-stack');
  requireThat(mutant?.cell === protocol.selected_cells[0] &&
    mutant.after.includes('}).reverse();'), 'retained semantic-mutant bank no longer matches this fault');
  const outcome = selected.length === 0 ? 'no-go' : selected.length - current.length >= 2 ? 'adopt' : 'retain-current';
  const receipt = { schema: 'noble-plausible-pilot/v1', source_commit: initialCommit, source_tree: initialTree,
    protocol_sha256: fileHash(protocolPath), source_matrix_sha256: fileHash(join(root, protocol.source_matrix)),
    source_paths_sha256: { [join('verification/differential-interactions/plausible', 'Driver.lean')]: fileHash(driverPath),
      [join('verification/differential-interactions/plausible', 'run.mjs')]: fileHash(join(lane, 'run.mjs')),
      'verification/foundation/differential.mjs': fileHash(join(root, 'verification/foundation/differential.mjs')) },
    toolchain: { lean: proof.lean, plausible_commit: JSON.parse(readFileSync(join(project, 'lake-manifest.json')))
      .packages.find(p => p.name === 'plausible').rev, node, node_sha256: fileHash(node),
      cli_sha256: fileHash(cli), foundation_receipt_sha256: fileHash(foundation) },
    seed: protocol.seed, generator_configuration: { size: protocol.size,
      cases_per_arm: protocol.cases_per_arm, allocation: protocol.plausible_allocation },
    comparison_receipt_sha256: fileHash(join(comparisonDir, 'differential.json')),
    imported_cases_sha256: fileHash(importPath), sample_bytes_sha256: fileHash(join(output, 'samples.jsonl')),
    generated_cases: cases.map(c => ({ cell: c.interaction.cell, source: c.origin.text,
      source_sha256: c.origin.sha256, model: JSON.parse(show({ ...c, origin: undefined })) })),
    comparison: { selected_cells: protocol.selected_cells, reached_cells: selected,
      current_reached_selected: current, current_reached_all: comparison.generator_comparison.current_generator.reached_cells,
      accepted: generatedRows.filter(row => row.observed.outcome === 'normal').length,
      refused: generatedRows.filter(row => row.observed.outcome === 'reject').length,
      exhausted: generatedRows.filter(row => row.observed.outcome === 'runtime-exhausted').length,
      unsupported: protocol.unsupported, fixed_invalid_refused: comparison.p1_invalid_control.refused,
      source_case_runs_per_arm: 16, modes: ['off', 'on'],
      distinct_attributed_failures_current: comparison.generator_comparison.current_generator.distinct_attributed_failures,
      distinct_attributed_failures_plausible: comparison.generator_comparison.structured_extension.distinct_attributed_failures },
    controls: { lean_positive_and_false_property: true, oracle_controls: comparison.controls,
      shrinking: controls, mutant_bank_id: mutant.id, mutant_injection_kind: 'test-side output reversal, not a candidate implementation build',
      earlier_implementation_mutant_evidence: 'verification/differential-interactions/receipts/756ccc9/campaign.json' },
    mutant_control: { cell: mutant.cell, failure_identity: identity, original, minimized,
      shrink_work: { attempts, budget: 48, rejected,
        termination_reason: localMinimum ? 'local-minimum-under-eligible-shrinks' : 'attempt-budget-exhausted',
        admission_and_identity_rechecked: true, globally_minimal: false } },
    decision: { outcome, rule: protocol.decision[outcome.replace('-', '_')] ?? protocol.decision.retain_current,
      proof_gate_control_pending: true, fresh_replay_pending: true } };
  requireThat(gitValue(['rev-parse', 'HEAD']) === initialCommit &&
    gitValue(['status', '--porcelain=v1', '--untracked-files=all']) === '',
  'pilot source changed during the clean-commit campaign');
  writeFileSync(join(output, 'pilot.json'), `${JSON.stringify(receipt, null, 2)}\n`);
  console.log(JSON.stringify({ ok: true, receipt: join(output, 'pilot.json'),
    selected: selected.length, current: current.length, decision: outcome,
    original_bytes: original.source_size_bytes, minimized_bytes: minimized.source_size_bytes,
    shrink_attempts: attempts }));
}
