import assert from 'node:assert/strict';
import { polymorphicCase, exactMc1Case, forgedSourceCase,
  boundedCase, dependentCase } from './proof-cases.mjs';
import { runKernelPeer } from './kernel-peer.mjs';
import { erasureCase } from './erasure.mjs';

function keep(context, observed) {
  const ledger = context.record.receipt.case_ledger[observed.id];
  assert.ok(ledger, `unplanned conformance execution ${observed.id}`);
  if (!observed.passed) context.record.receipt.failures.push({
    case_id: observed.id,
    error: observed.details.internal_checker_failure_gap
      ?? 'selected canonical case has an unobserved or refused variant',
  });
  assert.deepEqual(observed.positive.map(row => row.name).sort(),
    ledger.required_positive_baselines.slice().sort(),
    `${observed.id}: missing real positive baseline`);
  assert.deepEqual(observed.hostiles.map(row => row.name).sort(),
    ledger.required_hostile_variants.slice().sort(),
    `${observed.id}: missing hostile source variant`);
  ledger.executed_positive_baselines = observed.positive.length;
  ledger.executed_hostile_variants = observed.hostiles.filter(row =>
    Number.isSafeInteger(row.command)).length;
  context.record.receipt.cases.push(observed);
}

function review(context) {
  const case24 = context.plan.canonical.cases['CONTRACT-24'];
  assert.equal(case24.kind, 'review');
  const real = context.record.receipt.cases;
  const accepted = name => real.some(row => row.positive.some(item => item.name === name
    && context.record.receipt.commands[item.command - 1]?.id === item.command));
  assert.ok(accepted('polymorphic-reflexivity') && accepted('exact-source-proof'),
    'review lacks actual independently checked source proof claims');
  const labels = {
    'exact-strict-Lean-claim-check': 'checked-finite-exact-claim-only',
    'valid-Lean-proof-from-broken-Noble-translation': 'open-no-source-to-Lean-correctness-theorem',
    'Aeneas-extraction-only': 'does-not-typecheck-Noble-proof-source',
    'actual-Rust-checker-refinement': 'open-no-Rust-refinement-theorem',
    'stale-reviewed-rule': 'current-model-source-hash-checked-by-gate-not-formal-correspondence',
    'foreign-Aeneas-external-model': 'open-no-foreign-model-equivalence-proof',
    'unsound-extracted-model': 'open-no-extraction-soundness-proof',
    'producer-only-status': 'not-accepted-independent-Lean-verdict-required',
  };
  assert.deepEqual(Object.keys(labels), case24.input.variants,
    'assurance review omitted a canonical variant');
  return { id: 'CONTRACT-24', kind: 'review', outcome: 'separate-claims-labeled',
    proof: 'open', implementation_acceptance_promoted: false,
    lean_success_proves_translation: false, aeneas_typechecks_noble_proof: false,
    labels, observed_strict_lean_case_ids: ['CONTRACT-17', 'CONTRACT-18'],
    source_bound_revision: context.record.receipt.source_revision,
    reviewed_import_closure_revision: context.revisions.model,
    source_checker_and_consumer_revision: context.revisions.checker,
    named_call_source_to_accepted_recipe_correspondence: 'open-no-host-MC1-admission',
    reviewer_lane: 'human-independent-formal-assurance-required' };
}

export async function executeCases(context) {
  keep(context, polymorphicCase(context));
  keep(context, exactMc1Case(context));
  keep(context, forgedSourceCase(context));
  const peer = runKernelPeer(context);
  keep(context, boundedCase(context, peer));
  keep(context, erasureCase(context));
  keep(context, dependentCase(context, peer));
  context.record.receipt.review = review(context);
  assert.deepEqual(context.record.receipt.blocked_release_cases,
    context.plan.blocked_release.cases,
    'intrinsic fixture cannot silently promote independently owner-frozen law release');
}
