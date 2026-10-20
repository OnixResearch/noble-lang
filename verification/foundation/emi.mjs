import { mkdirSync, readFileSync, readdirSync, realpathSync, writeFileSync } from 'node:fs';
import { join, resolve, relative } from 'node:path';
import { buildCli, createEmiExecutor, digest, fileHash } from './execution.mjs';
import { generateEMI, assess, source } from './emi-model.mjs';
import { controls, wrongSelectedLowering } from './emi-controls.mjs';
import { show, requireThat, pairBinding, revivePair, evidenceMatches, headroom, discrepancy,
  createProofChecker, shrinkTriple } from './emi-evidence.mjs';

const extraSources = [
  'verification/foundation/emi.mjs', 'verification/foundation/emi-model.mjs',
  'verification/foundation/emi-shrink.mjs',
  'verification/foundation/emi-evidence.mjs', 'verification/foundation/emi-controls.mjs',
  'proofs/foundation/NobleFoundation/Branch.lean', 'proofs/foundation/inventory.json',
  'crates/noble-cli/src/core/runtime/host.mjs', 'crates/noble-cli/src/core/runtime/preamble.mjs',
  'crates/noble-cli/src/core/runtime/values.mjs', 'crates/noble-cli/src/core/runtime/protocol.mjs',
  'crates/noble-cli/src/core/runtime/engine.mjs', 'crates/noble-cli/src/core/runtime/engine-host.mjs',
  'crates/noble-cli/src/core/runtime/abi.json',
];
const requiredTheorems = ['NobleFoundation.exec_branchRef', 'NobleFoundation.branchRef_untaken',
  'NobleFoundation.branchVisits_selected'];
function artifactIndex(directory, prefix = '') {
  const rows = {};
  for (const entry of readdirSync(join(directory, prefix), { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name))) {
    const path = join(prefix, entry.name);
    if (path === 'cargo-home') continue;
    if (entry.isDirectory()) Object.assign(rows, artifactIndex(directory, path));
    else if (entry.isFile()) rows[path] = fileHash(join(directory, path));
    else throw Error(`nonregular evidence artifact: ${path}`);
  }
  return rows;
}
const admissionRow = ({ wat, ...record }) => ({ ...record, wat_sha256: wat === null ? null : digest(wat) });
const executionRow = observed => ({ outcome: observed.outcome, stack: observed.stack, trace: observed.trace,
  guest_requests: observed.guest_requests, protected_operations: observed.protected_operations,
  headroom: observed.headroom, artifact_directory: observed.directory, wasm_sha256: observed.wasm_sha256,
  wat_sha256: observed.wat_sha256, fault_injected: observed.fault_injected,
  report_sha256: fileHash(join(observed.directory, 'observation.json')) });

export async function runEmi({ option, args, root, project, selection, childBase, run: baseRun,
  gitState, leanBin, node, boundSources }) {
  if (option('--verify')) {
    try {
      const path = resolve(option('--verify')), receipt = JSON.parse(readFileSync(path));
      const state = gitState(root);
      const drift = Object.entries(receipt.sources_sha256 ?? {}).filter(([p, h]) => fileHash(join(root, p)) !== h).map(([p]) => p);
      const artifactDrift = Object.entries(receipt.artifacts_sha256 ?? {})
        .filter(([p, h]) => fileHash(join(receipt.artifact_directory, p)) !== h).map(([p]) => p);
      const ok = receipt.schema === 'noble-foundation-emi/v1' && state.status === '' &&
        receipt.source_tree === state.tree && receipt.all_matched === true &&
        Object.keys(receipt.sources_sha256 ?? {}).length > 0 && Object.keys(receipt.artifacts_sha256 ?? {}).length > 0 &&
        receipt.controls.every(control => control.detected) && drift.length === 0 && artifactDrift.length === 0;
      console.log(show({ verified: ok, clean: state.status === '', receipt_tree: receipt.source_tree,
        current_tree: state.tree, drift, artifact_drift: artifactDrift }));
      process.exitCode = ok ? 0 : 1;
    } catch (error) { console.error(`[emi] FAIL ${error.message}`); process.exitCode = 1; }
    return;
  }
  const count = Number(option('--count') ?? '16'), seed = BigInt(option('--seed') ?? '53');
  const foundationPath = option('--foundation'), output = option('--output'), replayPath = option('--replay');
  requireThat(output && foundationPath && Number.isInteger(count) && count >= 16 && count <= 64,
    'usage: differential.mjs --emi --seed N --count 16..64 --foundation RECEIPT --output NEW_DIR [--allow-dirty] [--replay MINIMUM_JSON]');
  const out = resolve(output);
  mkdirSync(out);
  const commandDirectory = join(out, 'commands');
  mkdirSync(commandDirectory);
  let commandCount = 0;
  const run = (command, argv, options = {}) => baseRun(command, argv,
    { ...options, captureDirectory: join(commandDirectory, String(++commandCount)) });
  const receipt = { schema: 'noble-foundation-emi/v1', seed: String(seed), requested_positive_pairs: count,
    profile: 'Core-Bootstrap / managed-linear-memory', fragment: 'fixed-input scalar if and I64 sum case with pure AOp bodies',
    opt_modes: ['off', 'on'], universal_equivalence: false, compiler_theorem: false,
    artifact_directory: out, cases: [], controls: [], discrepancies: [] };
  try {
    requireThat(realpathSync(process.execPath) === realpathSync(node), 'use the policy-selected Node');
    const initial = gitState(root), allowDirty = args.includes('--allow-dirty');
    requireThat(allowDirty || initial.status === '', 'Noble source worktree is dirty');
    Object.assign(receipt, { source_commit: initial.head, source_tree: initial.tree, clean_source: initial.status === '' });
    const foundation = JSON.parse(readFileSync(foundationPath));
    const verifyFoundation = (value, state) => {
      requireThat(value.schema === 'noble-foundation-check/v1' && value.source_tree === state.tree,
        'stale-proof-subject: foundation source tree mismatch');
      requireThat(Object.entries(value.sources_sha256).every(([p, hash]) => fileHash(join(root, p)) === hash),
        'stale-proof-subject: foundation model/source hash mismatch');
      requireThat(requiredTheorems.every(name => value.theorems.some(t => t.name === name && t.status === 'proved')),
        'foundation receipt is missing the branch theorems');
    };
    verifyFoundation(foundation, initial);
    receipt.foundation_receipt_sha256 = fileHash(foundationPath);
    receipt.theorems = foundation.theorems.filter(t => requiredTheorems.includes(t.name));
    const runtimeConfig = JSON.parse(readFileSync(join(root, 'crates/noble-cli/src/core/runtime/config.json')));
    const abi = JSON.parse(readFileSync(join(root, 'crates/noble-cli/src/core/runtime/abi.json')));
    const limits = Object.fromEntries(['allocation_bytes', 'recipe_leaves', 'program_depth', 'operand_bytes', 'continuation_bytes', 'steps']
      .map(key => [key, abi.limits_maximum[key]]));
    receipt.bounds = { runtime: limits, source_bytes: 65536, generated_pairs: count,
      shrink_attempts: 64, required_free_fraction: 'strictly more than one half of every selected runtime bound' };
    const sourcePaths = [...new Set([...boundSources, ...extraSources, ...runtimeConfig.runtime_assets])];
    receipt.sources_sha256 = Object.fromEntries(sourcePaths.map(path => [path, fileHash(join(root, path))]));
    const built = buildCli({ root, out, selection, childBase, run });
    receipt.toolchain = { cli_sha256: fileHash(built.cli), lean: foundation.lean,
      lean_sha256: foundation.lean_sha256, mathlib_commit: foundation.mathlib_commit,
      node: { path: node, sha256: fileHash(node), version: process.versions.node, v8: process.versions.v8, flags: process.execArgv },
      wasm_tools: { path: runtimeConfig.tools.wasm_tools.path, sha256: fileHash(runtimeConfig.tools.wasm_tools.path) },
      wasm_opt: { path: runtimeConfig.tools.wasm_opt.path, sha256: fileHash(runtimeConfig.tools.wasm_opt.path) },
      vendor: built.vendor };
    const executor = createEmiExecutor({ root, out, ...built, run, runtimeConfig, abi, limits });
    const checkProof = createProofChecker({ out, root, project, leanBin, childBase, run,
      foundationHash: receipt.foundation_receipt_sha256 });
    const proofChecks = [];
    const prove = (pair, label) => {
      const evidence = checkProof(pair, label);
      proofChecks.push(evidence);
      return evidence;
    };
    const eligible = [];
    const observeAdmitted = (pair, admissions, evidence, mode, fault) => {
      requireThat(evidenceMatches(pair, evidence), 'stale-subject: execution refused stale reference evidence');
      const original = executor.execute(admissions.original, mode);
      const variant = executor.execute(admissions.variant, mode, fault ? { mutateWat: wrongSelectedLowering } : {});
      original.headroom = headroom(pair, 'original', original, limits);
      variant.headroom = headroom(pair, 'variant', variant, limits);
      const expected = assess(pair).expected;
      return { original, variant, discrepancy: discrepancy(expected, original, pair.prefix.length) ??
        discrepancy(expected, variant, pair.prefix.length) };
    };
    const evaluate = (pair, label, mode, fault) => {
      const reference = assess(pair);
      const admissions = Object.fromEntries(['original', 'variant'].map(member =>
        [member, executor.admit(source(pair, member), `${label}-${member}`)]));
      if (!reference.eligible || !admissions.original.admitted || !admissions.variant.admitted)
        return { admitted: false, reference, admissions };
      const evidence = prove(pair, label);
      const observed = observeAdmitted(pair, admissions, evidence, mode, fault);
      return { admitted: true, admissions, evidence, observations: observed, discrepancy: observed.discrepancy };
    };
    const replay = replayPath ? JSON.parse(readFileSync(resolve(replayPath))) : null;
    if (replay) requireThat(replay.schema === 'noble-foundation-emi-replay/v1', 'invalid replay schema');
    const pairs = replay ? [revivePair(replay.pair)] : generateEMI(seed, count);
    requireThat(show(pairs) === show(replay ? [revivePair(replay.pair)] : generateEMI(seed, count)), 'generation replay differs');
    receipt.generated_pairs_sha256 = digest(show(pairs));
    for (const [index, pair] of pairs.entries()) {
      const reference = assess(pair);
      const admissions = Object.fromEntries(['original', 'variant'].map(member =>
        [member, executor.admit(source(pair, member), `pair-${index}-${member}`)]));
      const entry = { index, kind: pair.kind, selected: pair.selected, transformation: pair.transformation,
        pair, subject: pairBinding(pair), model_eligibility: reference,
        admission: Object.fromEntries(Object.entries(admissions).map(([key, value]) => [key, admissionRow(value)])), executions: [] };
      receipt.cases.push(entry);
      if (!reference.eligible || !admissions.original.admitted || !admissions.variant.admitted) {
        entry.classification = reference.eligible ? 'frontend-refused' : 'model-ineligible';
        continue;
      }
      entry.classification = 'eligible';
      const evidence = prove(pair, `pair-${index}`);
      entry.evidence = evidence;
      const observations = {};
      for (const mode of ['off', 'on']) {
        const observed = observeAdmitted(pair, admissions, evidence, mode, false);
        observations[mode] = observed;
        entry.executions.push({ mode, original: executionRow(observed.original), variant: executionRow(observed.variant),
          discrepancy: observed.discrepancy });
        if (observed.discrepancy) {
          const shrunk = shrinkTriple(pair, { mode, wanted: observed.discrepancy,
            evaluate: (candidate, label, opt) => evaluate(candidate, `failure-${index}-${mode}-${label}`, opt, false) });
          const filename = `minimal-${index}-${mode}.json`;
          writeFileSync(join(out, filename), show({ schema: 'noble-foundation-emi-replay/v1', pair: shrunk.pair,
            mode, fault: null, discrepancy: observed.discrepancy }) + '\n');
          receipt.discrepancies.push({ index, mode, kind: observed.discrepancy, shrink: shrunk, replay: filename });
        }
      }
      eligible.push({ pair, reference, admissions, evidence, observations });
    }
    requireThat(eligible.length > 0, 'no independently admitted EMI pairs');
    if (replay) {
      const result = evaluate(eligible[0].pair, 'replay', replay.mode, replay.fault === 'wrong-selected-branch-lowering');
      receipt.controls = [{ id: 'replay-discrepancy', detected: result.discrepancy === replay.discrepancy,
        observed_discrepancy: result.discrepancy, expected_discrepancy: replay.discrepancy,
        evidence: result.evidence, observed: executionRow(result.observations.variant) }];
    } else {
      receipt.controls = controls({ eligible, executor, evaluate, verifyFoundation, foundation, initial, out });
    }
    const countBy = (field, rows = receipt.cases) => Object.fromEntries([...new Set(rows.map(field))].sort()
      .map(key => [key, rows.filter(entry => field(entry) === key).length]));
    const eligibleCases = receipt.cases.filter(entry => entry.classification === 'eligible');
    receipt.counts = { generated_pairs: pairs.length, eligible: eligible.length, refused: pairs.length - eligible.length,
      branch_kinds: countBy(entry => entry.kind), selected_branches: countBy(entry => `${entry.kind}/${entry.selected}`),
      transformations: countBy(entry => entry.transformation.kind), eligibility: countBy(entry => entry.classification),
      eligible_branch_kinds: countBy(entry => entry.kind, eligibleCases),
      eligible_transformations: countBy(entry => entry.transformation.kind, eligibleCases),
      opt_modes: Object.fromEntries(['off', 'on'].map(mode => [mode, eligible.length * 2])),
      nominal_kernel_checked_subjects: eligible.length * 2,
      kernel_checked_subjects: proofChecks.length * 2, controls: receipt.controls.length };
    receipt.proof_checks = proofChecks;
    receipt.all_matched = receipt.discrepancies.length === 0 && receipt.controls.every(control => control.detected);
    const final = gitState(root);
    requireThat(show(final) === show(initial), 'Noble source state changed during EMI comparison');
    requireThat(Object.entries(receipt.sources_sha256).every(([p, hash]) => fileHash(join(root, p)) === hash),
      'bound source/model bytes changed during EMI comparison');
    requireThat(fileHash(built.cli) === receipt.toolchain.cli_sha256, 'compiled CLI bytes changed during EMI comparison');
    receipt.command_count = commandCount;
    receipt.claims = { branch_reference_and_prefix: 'Lean-kernel-checked MC1 model',
      per_subject_expectations: 'Lean-kernel-checked decide and exec_branchRef instantiation',
      frontend_kernel_and_Wasm_execution: 'finite supporting evidence',
      Rust_renderer_and_lowering_refinement: 'open', universal_input_equivalence: 'not claimed',
      quota_sensitive_or_identity_observations: 'excluded',
      mathlib_oleans: foundation.mathlib_oleans, receipt_authenticity: 'not claimed' };
    receipt.artifacts_sha256 = artifactIndex(out);
    writeFileSync(join(out, 'emi.json'), show(receipt) + '\n');
    console.log(show({ ok: receipt.all_matched, receipt: join(out, 'emi.json'), source_commit: initial.head,
      source_tree: initial.tree, clean_source: receipt.clean_source, seed: String(seed), counts: receipt.counts,
      all_matched: receipt.all_matched, controls: receipt.controls.map(control => `${control.id}:${control.detected}`) }));
    process.exitCode = receipt.all_matched ? 0 : 1;
  } catch (error) {
    writeFileSync(join(out, 'emi-failure.json'), show({ ...receipt, error: error.message }) + '\n');
    console.error(`[emi] FAIL ${error.stack ?? error.message}`);
    process.exitCode = 1;
  }
}
