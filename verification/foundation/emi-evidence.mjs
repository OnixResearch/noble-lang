import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join, relative } from 'node:path';
import { digest, fileHash, matchesScalar } from './execution.mjs';
import { assess, source, leanEvidence } from './emi-model.mjs';
import { complexity, shrinkCandidates } from './emi-shrink.mjs';

export const show = value => JSON.stringify(value, (_, x) => typeof x === 'bigint' ? String(x) : x, 2);
export const requireThat = (condition, message) => { if (!condition) throw Error(message); };
export const pairBinding = pair => ({
  original_sha256: digest(source(pair, 'original')),
  variant_sha256: digest(source(pair, 'variant')),
  fixed_input_sha256: digest(show({ selected: pair.selected, input: pair.input, prefix: pair.prefix })),
  model_pair_sha256: digest(show(pair)),
});
export function evidenceMatches(pair, evidence) {
  return evidence.kernel_checked === true && show(pairBinding(pair)) === show(evidence.subject);
}
export function revivePair(pair) {
  return { ...pair, input: pair.input.map(BigInt),
    original: pair.original.map(ops => ops.map(op => op?.push !== undefined ? { push: BigInt(op.push) } : op)),
    variant: pair.variant.map(ops => ops.map(op => op?.push !== undefined ? { push: BigInt(op.push) } : op)) };
}
export function headroom(pair, member, observed, limits) {
  const metrics = observed.metrics ?? {};
  // Reading the scalar output stack uses the runtime's reflection counter;
  // this is host observation work, not a guest `reflect` operation.
  const measuredLimits = { ...limits, reflection_steps: limits.steps };
  const used = {
    allocation_bytes: metrics.allocation_peak_bytes,
    operand_bytes: metrics.operand_peak_bytes,
    continuation_bytes: metrics.continuation_peak_bytes,
    steps: metrics.execution_steps,
    reflection_steps: metrics.reflection_steps,
    recipe_leaves: pair.prefix.length + pair.input.length + 4 + pair[member].flat().length,
    program_depth: 2,
  };
  const remaining = Object.fromEntries(Object.entries(used).map(([key, value]) => [key, measuredLimits[key] - value]));
  return { limits: measuredLimits, used, remaining,
    sufficient: observed.outcome === 'normal' &&
      Object.entries(used).every(([key, value]) => Number.isSafeInteger(value) && value >= 0 && value * 2 < measuredLimits[key]) };
}
export function discrepancy(expected, observed, prefixLength) {
  if (observed.outcome === 'runtime-exhausted') return 'exhaustion';
  if (observed.outcome !== 'normal') return 'outcome';
  if (observed.headroom && !observed.headroom.sufficient) return 'headroom';
  if (observed.guest_requests !== 0 || observed.protected_operations !== 0 ||
      !Array.isArray(observed.trace) || show(observed.trace) !== show(expected.trace)) return 'trace';
  if (!Array.isArray(observed.stack) || observed.stack.length !== expected.stack.length) return 'stack-shape';
  if (show(observed.stack.slice(0, prefixLength)) !== show(expected.stack.slice(0, prefixLength))) return 'prefix';
  return matchesScalar(expected, observed) ? null : 'ordered-output';
}

export function createProofChecker({ out, root, project, leanBin, childBase, run, foundationHash }) {
  const proofs = join(out, 'proofs');
  mkdirSync(proofs);
  let serial = 0;
  return (pair, label) => {
    const identifier = `emi_${++serial}`;
    const text = 'import NobleFoundation.Branch\nopen NobleFoundation NobleContracts\n\n' + leanEvidence(pair, identifier);
    const file = join(proofs, `${label}.lean`);
    writeFileSync(file, text);
    const argv = ['env', 'lean', '--json', file];
    const result = run(join(leanBin, 'lake'), argv,
      { cwd: project, env: { ...childBase, TMPDIR: out }, allowFailure: true, timeout: 120000 });
    writeFileSync(`${file}.stdout`, result.stdout);
    writeFileSync(`${file}.stderr`, result.stderr);
    const diagnostics = result.stdout.split('\n').filter(Boolean).map(line => JSON.parse(line));
    const ok = result.status === 0 && diagnostics.every(message => message.severity === 'information');
    const evidence = { subject: pairBinding(pair), kernel_checked: ok,
      foundation_receipt_sha256: foundationHash, lean_file: relative(out, file), lean_sha256: fileHash(file),
      stdout_sha256: digest(result.stdout), stderr_sha256: digest(result.stderr), exit: result.status,
      argv, model_root: root, model_theorem: 'NobleFoundation.exec_branchRef' };
    writeFileSync(`${file}.json`, show(evidence) + '\n');
    requireThat(ok, `Lean rejected ${label}:\n${result.stdout}\n${result.stderr}`);
    requireThat(evidenceMatches(pair, evidence), 'stale-subject: proof does not bind the current pair');
    return evidence;
  };
}

// Shrinking never trusts the model gate alone. Every eligible candidate is
// re-admitted, kernel-checked, and executed before its discrepancy is retained.
export function shrinkTriple(initial, { evaluate, mode, wanted, maximum = 64 }) {
  let current = initial, attempts = 0, accepted = 0;
  const outcomes = { model_refused: 0, admission_refused: 0, class_changed: 0, retained: 0 };
  const history = [];
  let exhausted = false;
  outer: for (;;) {
    let changed = false;
    for (const candidate of shrinkCandidates(current)) {
      if (complexity(candidate) >= complexity(current)) continue;
      if (!assess(candidate).eligible) { outcomes.model_refused++; continue; }
      if (attempts >= maximum) { exhausted = true; break outer; }
      const result = evaluate(candidate, `shrink-${++attempts}`, mode);
      if (!result.admitted) { outcomes.admission_refused++; continue; }
      if (result.discrepancy !== wanted) { outcomes.class_changed++; continue; }
      history.push({ before: complexity(current), after: complexity(candidate),
        mutation_site_before: current.transformation.site, mutation_site_after: candidate.transformation.site,
        evidence: result.evidence, discrepancy: result.discrepancy });
      current = candidate; accepted++; outcomes.retained++; changed = true; break;
    }
    if (!changed) break;
  }
  return { pair: current, mode, discrepancy: wanted, attempts, accepted, outcomes, history,
    local_minimal: !exhausted, attempt_bound: maximum, bound_exhausted: exhausted };
}
