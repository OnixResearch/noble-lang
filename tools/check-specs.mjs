#!/usr/bin/env bun
// Document validation only. No Noble parsing, execution, or proof checking.
// Filesystem effects stay in loadBundle/main; validate and deriveLedger are pure.
import { readFileSync, readdirSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';
import { canonicalPath, nativeRequirementIds, legacyRequirementIds } from './cairn-specs.mjs';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const CORE_SPEC = canonicalPath('SPEC-0001.md');
const SAFETY_SPEC = canonicalPath('SAFETY.md');
const VERIFICATION_SPEC = canonicalPath('VERIFICATION.md');
const REVISION = '0.1.0-draft.5';
const CONTRACT_OUTCOMES = ['proved', 'disproved', 'unknown', 'timeout', 'unsupported', 'error', 'not-run'];
const CONTRACT_POLICY = {
  profile: 'Contracts-Draft',
  source_contracts: 'typed-compiler-inputs',
  companions: 'first-class-checked-companions',
  ordinary_programs_require_proofs: false,
  ordinary_execution_runs_proof_search: false,
};
const CALCULATOR_POLICY = {
  contract: 'CALCULATOR.md',
  lane: 'reference-application-not-language-profile',
  numeric_model: 'arbitrary-precision-integers-and-normalized-rationals',
  decimal_literals: 'exact',
  division: 'exact-rational-with-zero-error',
  core_I64_semantics: 'unchanged-wrapping',
  bootstrap_expanded: false,
  approximation: 'deferred-explicit-only',
};
const WORKER_POLICY = {
  design: 'WORKER-CONFORMANCE.md',
  lane: 'conformance-design-not-language-profile',
  new_kernel_syntax: false,
  bootstrap_expanded: false,
  swarm_runtime_claimed: false,
};
const OCTET_ADOPTION_POLICY = {
  design: 'OCTET-ADOPTION.md',
  design_source_revision: 'cf04e894e53eb0947230118a086ef6066ddba38c',
  architecture_policy: 'required-in-addition-to-deny-all',
  effect_analysis: 'resolved-Noble-contracts-not-source-token-substitution',
  authorization_witness: 'host-established-one-shot-resource',
  receipt_basis: 'validated-scoped-observation',
  new_kernel_syntax: false,
  bootstrap_expanded: false,
  Aeneas_route_replaced: false,
};
const OCTET_MILESTONE_REQUIREMENTS = {
  M1: ['VT-OCTET-01', 'VT-OCTET-02', 'VT-OCTET-03', 'EV-BIND-01', 'EV-TIER-01'],
  M5: ['H-AUTH-01', 'H-AUTH-02', 'H-AUTH-03', 'H-AUTH-04', 'H-RECEIPT-01', 'H-RECEIPT-02'],
  M6: ['DX-PROTOCOL-03'],
  MW1: ['DX-TYPE-03', 'DX-TYPE-04', 'DX-MODULE-04', 'H-AUTH-01', 'H-AUTH-02', 'H-AUTH-03', 'H-AUTH-04', 'H-RECEIPT-01', 'H-RECEIPT-02'],
  MW2: ['DX-PROTOCOL-03', 'H-AUTH-03', 'H-RECEIPT-02'],
};
const WORKER_GATES = {
  MW1: {
    depends_on: ['M4', 'M5'],
    entry_gates: ['worker-interface-checker', 'declared-schemas-and-modules', 'preparation-service', 'package-encoding', 'bounded-execution-profile'],
    required_requirements: ['K-PROG-09', 'K-PROG-10', 'K-EFFECT-05', 'K-CHECK-06', 'K-CHECK-07', 'P-ROUND-01', 'P-ROUND-02',
      'P-PACK-01', 'P-PACK-02', 'DX-MODULE-03', 'H-LIMIT-02', 'H-LIMIT-03', 'H-OUTCOME-01', 'H-OUTCOME-02'],
  },
  MW2: {
    depends_on: ['MW1', 'M6'],
    entry_gates: ['native-async-interfaces'],
    required_requirements: ['RA-ASYNC-02', 'RA-ASYNC-03', 'RA-ASYNC-04', 'RA-ASYNC-05', 'H-OUTCOME-01', 'H-OUTCOME-02'],
  },
};
const CONTRACT_ROUTES = {
  'PO-19': 'lean-and-aeneas-contract-export',
  'PO-20': 'lean-and-aeneas-companion-rules',
  'PO-21': 'lean-and-aeneas-guard-correspondence',
};
const AENEAS_POLICY = {
  implementation_language: 'Rust',
  reference_logic: 'Lean 4',
  primary_route: 'charon-aeneas-lean',
  scope: 'all-noble-owned-production-rust',
  kernel_route: 'mandatory-no-exceptions',
  verus: 'reviewed-non-kernel-exceptions-only',
};
const AENEAS_ROUTES = {
  'PO-13': 'aeneas-lean',
  'PO-14': 'aeneas-and-boundary-evidence',
  'PO-17': 'aeneas-and-backend-proof-or-translation-validation',
  'PO-18': 'aeneas-and-loader-correspondence',
  'SO-06': 'aeneas-and-wrapper-evidence',
  'SO-07': 'aeneas-and-backend-proof-or-translation-validation',
  'SO-09': 'aeneas-transitions-and-synchronization-evidence',
};
const STATES = {
  implementation: ['absent', 'partial', 'implemented', 'unsupported'],
  execution: ['not-run', 'passed', 'failed', 'timeout', 'unsupported'],
  proof: ['open', 'accepted', 'failed', 'not-applicable'],
  trust: ['unassessed', 'explicit'],
};
const KINDS = ['static', 'runtime', 'admission', 'adapter', 'identity', 'review'];
const REQUIRED_DOCUMENTS = ['SPEC-0001', 'SPEC-S001', 'SPEC-W001', 'SPEC-V001', 'IMPL-V001', 'SPEC-V002', 'SPEC-B001', 'SPEC-BE001', 'SPEC-R001', 'SPEC-EV001', 'SPEC-DX001', 'SPEC-CALC001'];
const EXECUTED = ['passed', 'failed', 'timeout'];
const ROUTES = {
  K: ['semantic-test', 'model-proof', 'review'],
  P: ['semantic-test', 'model-proof', 'review'],
  B: ['semantic-test', 'model-proof', 'review'],
  BE: ['experiment-test', 'correspondence', 'review'],
  W: ['boundary-test', 'correspondence', 'review'],
  H: ['boundary-test', 'correspondence', 'review'],
  S: ['boundary-test', 'correspondence', 'review'],
  WI: ['boundary-test', 'correspondence', 'review'],
  RA: ['boundary-test', 'correspondence', 'review'],
  C: ['policy-test', 'review'],
  EV: ['policy-test', 'review'],
  V: ['proof-or-explicit-assumption', 'implementation-test', 'review'],
  VT: ['proof-or-explicit-assumption', 'implementation-test', 'review'],
  VC: ['proof-or-explicit-assumption', 'implementation-test', 'review'],
  DX: ['semantic-test', 'policy-test', 'review'],
  CALC: ['application-test', 'benchmark-policy-test', 'review'],
};

// Document scope only. This discovery is not the compiler/source inventory.
function checkedMarkdown(file) {
  return file.endsWith('.md') && (file === 'README.md'
    || ['specs/', '.cairn/', 'verification/', 'proofs/'].some(prefix => file.startsWith(prefix)));
}

export function loadBundle(root) {
  const texts = new Map();
  const paths = new Set();
  function visit(relative) {
    for (const entry of readdirSync(path.join(root, relative), { withFileTypes: true })) {
      if (['.git', '.pi', '.octet', '.lake', 'node_modules', 'target', 'build', '.direnv'].includes(entry.name)) continue;
      const name = path.posix.join(relative, entry.name);
      if (entry.isSymbolicLink()) continue;
      paths.add(name);
      if (entry.isDirectory()) visit(name);
      else if (/\.(md|json)$/.test(name)) texts.set(name, readFileSync(path.join(root, name), 'utf8'));
    }
  }
  visit('');
  return { texts, paths };
}

function object(value) {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

function sameStrings(actual, expected) {
  return Array.isArray(actual) && actual.every(v => typeof v === 'string')
    && JSON.stringify([...actual].sort()) === JSON.stringify([...expected].sort());
}

function parse(bundle, file) {
  const text = bundle.texts.get(file);
  if (text === undefined) throw new Error(`missing file: ${file}`);
  return JSON.parse(text);
}

function requirements(bundle, family) {
  return family.normative_documents.flatMap(doc => nativeRequirementIds(bundle.texts.get(path.posix.join('specs', doc.path)) ?? '')
    .map(id => ({ id, document: doc.path, document_id: doc.id })));
}

function allCases(bundle, family) {
  return family.scenario_files.flatMap(file => parse(bundle, `specs/${file}`).cases);
}

export function deriveLedger(bundle) {
  const family = parse(bundle, 'specs/spec-family.json');
  const cases = allCases(bundle, family);
  return {
    revision: REVISION,
    generated_by: 'bun tools/check-specs.mjs --refresh-ledger',
    meaning: 'planned evidence routes and scenario links; not conformance or proof results',
    requirements: requirements(bundle, family).sort((a, b) => a.id.localeCompare(b.id, 'en')).map(req => {
      const scenarioIds = cases.filter(c => c.requirements.includes(req.id)).map(c => c.id).sort();
      return {
        ...req,
        evidence_routes: ROUTES[req.id.split('-')[0]] ?? [],
        scenarios: scenarioIds,
        test_design: scenarioIds.length ? 'scenario-described-not-executed' : 'test-design-open',
      };
    }),
  };
}

// This checks the fixture contract, not a Noble import or effect derivation.
function validateWitEffectOracle(get, check) {
  const c = get('WI-07');
  const operation = 'noble-test:math/arithmetic@1.0.0#inc';
  const expected = [
    [false, [], 'effect-reject'], [true, [], 'effect-reject'],
    [false, [operation], 'accept'], [true, [operation], 'accept'],
  ];
  check(c?.kind === 'static' && c.profile === 'Component-Sync-Bootstrap'
    && sameStrings(c.requirements, ['WI-WIT-03', 'WI-WIT-04', 'W-WIT-03', 'S-EFFECT-01', 'S-EFFECT-02'])
    && c.input?.harness === 'import-effect-check-matrix' && c.input.operation === operation
    && c.input.interface === 'I64 -- I64' && c.input.non_effect_derivations === 'valid',
  'WIT effect oracle: input and requirement bindings');
  const variants = c?.input?.variants;
  check(Array.isArray(variants) && variants.length === expected.length
    && expected.every(([reviewed, effects, outcome]) => variants.some(v => v?.reviewed_pure_contract === reviewed
      && sameStrings(v.claimed_effects, effects) && v.outcome === outcome)),
  'WIT effect oracle: every reviewed/unreviewed request bound');
  check(c?.expected?.stage === 'check' && c.expected.outcome === 'each-effect-bound-matches'
    && c.expected.reviewed_purity_removes_operation_id === false
    && c.expected.guest_requests === 0 && c.expected.protected_operations === 0,
  'WIT effect oracle: static observations');
}

// Check discriminating contract-fixture expectations only; no Noble or Lean execution.
function validateContractOracles(get, check) {
  const first = get('CONTRACT-01');
  check(first?.input?.subject === 'actual-accepted-program'
    && first.input.postcondition === 'output = wrap64(input + 1)'
    && first.expected?.outcome === 'proved-for-exact-subject'
    && first.expected.lean_kernel_check_required === true && first.expected.export_correspondence_required === true
    && first.expected.total_correctness_claimed === false && first.expected.vectors_establish_universal_proof === false,
  'contract oracle: exact increment claim');
  const vectors = first?.input?.vectors;
  const i64 = value => value?.type === 'I64' && typeof value.value === 'string' && /^-?\d+$/.test(value.value)
    && BigInt(value.value) >= -(1n << 63n) && BigInt(value.value) < (1n << 63n);
  check(Array.isArray(vectors) && vectors.some(v => v.input?.value === '9223372036854775807')
    && vectors.some(v => v.input?.value === '41')
    && vectors.every(v => i64(v.input) && i64(v.output)
      && BigInt.asIntN(64, BigInt(v.input.value) + 1n) === BigInt(v.output.value)),
  'contract oracle: wrapping arithmetic vectors');
  check(sameStrings(get('CONTRACT-02')?.input?.variants, ['captured-value', 'program-operand', 'instantiated-interface',
    'logical-definition', 'host-contract', 'semantic-revision', 'consumer-policy', 'current-environment-fact'])
    && get('CONTRACT-02')?.expected?.outcome === 'reject-each-variant-until-revalidated'
    && get('CONTRACT-02')?.expected?.new_certified_status === false
    && get('CONTRACT-02')?.expected?.old_theorem_automatically_false === false, 'contract oracle: evidence binding');
  for (const id of ['CONTRACT-03', 'CONTRACT-05']) {
    check(get(id)?.kind === 'runtime' && get(id)?.input?.compiler_service === 'disabled'
      && get(id)?.input?.proof_search === 'disabled' && get(id)?.expected?.prover_calls === 0
      && get(id)?.expected?.candidate_prepare_requests === 0, `contract oracle: runtime proof-free builders ${id}`);
  }
  check(i64(get('CONTRACT-03')?.input?.argument)
    && get('CONTRACT-03')?.input?.argument.value === '9223372036854775807'
    && i64(get('CONTRACT-03')?.expected?.output)
    && get('CONTRACT-03')?.expected?.output.value === '-9223372036854775807'
    && get('CONTRACT-03')?.expected?.claim_kind === 'partial-correctness'
    && get('CONTRACT-03')?.expected?.assumptions_dropped === false
    && get('CONTRACT-03')?.expected?.underlying_recipe_matches_plain_compose === true, 'contract oracle: composition preserves claim');
  check(get('CONTRACT-04')?.input?.ordinary_interfaces_match === true
    && get('CONTRACT-04')?.input?.implication_evidence === 'absent'
    && get('CONTRACT-04')?.input?.right_precondition === 'input >= 0'
    && i64(get('CONTRACT-04')?.input?.counterexample_initial_input)
    && get('CONTRACT-04')?.input?.counterexample_initial_input.value === '-2'
    && i64(get('CONTRACT-04')?.input?.intermediate_output)
    && get('CONTRACT-04')?.input?.intermediate_output.value === '-1'
    && get('CONTRACT-04')?.expected?.outcome === 'unresolved-implication'
    && get('CONTRACT-04')?.expected?.ordinary_composition_valid === true
    && get('CONTRACT-04')?.expected?.new_certified_status === false, 'contract oracle: missing intermediate implication');
  const instances = get('CONTRACT-05')?.input?.instances;
  check(get('CONTRACT-05')?.input?.captures_supplied_after_compilation === true
    && get('CONTRACT-05')?.expected?.program_value_identity_relation === 'different'
    && get('CONTRACT-05')?.expected?.literal_specialization_sufficient === false
    && Array.isArray(instances) && instances.length >= 2 && new Set(instances.map(i => i.capture?.value)).size >= 2
    && instances.every(i => i64(i.capture) && i64(i.argument) && i64(i.output)
      && BigInt.asIntN(64, BigInt(i.capture.value) + BigInt(i.argument.value)) === BigInt(i.output.value)), 'contract oracle: runtime family binding');
  check(sameStrings(get('CONTRACT-06')?.input?.variants, ['serialized-certified-true', 'forged-constructor-tag',
    'unknown-rule', 'cyclic-derivation', 'missing-premise', 'wrong-family-instantiation', 'exhausted-replay-budget'])
    && get('CONTRACT-06')?.expected?.outcome === 'reject-each-variant'
    && get('CONTRACT-06')?.expected?.new_certified_status === false, 'contract oracle: forged companions');
  check(get('CONTRACT-07')?.input?.precondition === 'input < 9223372036854775807'
    && get('CONTRACT-07')?.input?.guard_correspondence === 'prechecked'
    && i64(get('CONTRACT-07')?.input?.argument)
    && get('CONTRACT-07')?.input?.argument.value === '9223372036854775807'
    && get('CONTRACT-07')?.expected?.outcome === 'precondition-reject'
    && get('CONTRACT-07')?.expected?.conditional_theorem_valid === true
    && get('CONTRACT-07')?.expected?.candidate_body_started === false, 'contract oracle: false precondition');
  check(get('CONTRACT-08')?.input?.precondition === 'input < 9223372036854775807'
    && i64(get('CONTRACT-08')?.input?.argument)
    && get('CONTRACT-08')?.input?.argument.value === '9223372036854775806'
    && i64(get('CONTRACT-08')?.expected?.output)
    && get('CONTRACT-08')?.expected?.output.value === '9223372036854775807'
    && get('CONTRACT-08')?.expected?.outcome === 'certified-invocation'
    && get('CONTRACT-08')?.input?.artifact_correspondence === 'recorded-trusted-build'
    && get('CONTRACT-08')?.input?.host_authority === 'independently-checked'
    && get('CONTRACT-08')?.expected?.verified_backend_claimed === false
    && get('CONTRACT-08')?.expected?.guard_erased === false
    && get('CONTRACT-08')?.expected?.underlying_program_identity_changed === false, 'contract oracle: applicable invocation');
  const expectedAttempts = {
    'accepted-exact-proof': 'proved', 'checked-refutation': 'disproved', 'bare-solver-answer': 'unknown',
    'rejected-proof-term': 'error', 'unfinished-or-sorry-proof': 'error', 'proof-timeout': 'timeout',
    'unsupported-predicate': 'unsupported', 'no-attempt': 'not-run', 'execution-fuel-exhausted': 'unknown',
  };
  const attempts = get('CONTRACT-09')?.input?.attempts;
  check(get('CONTRACT-09')?.input?.proof_policy === 'strict-Lean'
    && Array.isArray(attempts) && sameStrings(attempts.map(a => a.case), Object.keys(expectedAttempts))
    && attempts.every(a => a.outcome === expectedAttempts[a.case] && a.release_allowed === (a.outcome === 'proved'))
    && get('CONTRACT-09')?.expected?.outcome === 'only-applicable-proved-claim-releases'
    && ['failed_proof_implies_disproof', 'solver_success_is_accepted_proof', 'fuel_exhaustion_proves_divergence']
      .every(key => get('CONTRACT-09')?.expected?.[key] === false)
    && get('CONTRACT-09')?.expected?.independent_recheck_required === true, 'contract oracle: claim outcome matrix');
  check(sameStrings(get('CONTRACT-10')?.input?.operations, ['pass-as-argument', 'return-as-result',
    'store-in-homogeneous-list', 'inspect-contract-and-evidence-reference', 'explicitly-project-program'])
    && get('CONTRACT-10')?.input?.proof_terms === 'outside-Wasm'
    && get('CONTRACT-10')?.expected?.selected_inspection_metadata_retained === true
    && get('CONTRACT-10')?.expected?.underlying_program_identity === 'unchanged'
    && get('CONTRACT-10')?.expected?.underlying_reflected_recipe === 'unchanged'
    && get('CONTRACT-10')?.expected?.host_only_metadata_sufficient === false
    && get('CONTRACT-10')?.expected?.authority_created === false
    && get('CONTRACT-10')?.expected?.implicit_evidence_fetches === 0, 'contract oracle: first-class companions');
  check(get('CONTRACT-11')?.input?.proof_evidence === 'absent'
    && get('CONTRACT-11')?.input?.proof_services === 'disabled'
    && get('CONTRACT-11')?.expected?.outcome === 'ordinary-success'
    && i64(get('CONTRACT-11')?.expected?.output) && get('CONTRACT-11')?.expected?.output.value === '42'
    && get('CONTRACT-11')?.expected?.behavioral_certification_claimed === false
    && get('CONTRACT-11')?.expected?.prover_calls === 0, 'contract oracle: ordinary execution unchanged');
  check(sameStrings(get('CONTRACT-12')?.input?.variants, ['resource-in-ghost-capture',
    'service-capability-in-pure-evidence', 'runtime-value-from-erased-ghost'])
    && get('CONTRACT-12')?.expected?.outcome === 'reject-each-variant'
    && get('CONTRACT-12')?.expected?.resource_ownership_duplicated === false, 'contract oracle: ghost eligibility');
  check(get('CONTRACT-13')?.expected?.outcome === 'correspondence-reject'
    && get('CONTRACT-13')?.expected?.candidate_body_started === false, 'contract oracle: unrelated Wasm');
  check(sameStrings(get('CONTRACT-14')?.input?.variants, ['changed-body', 'weakened-postcondition',
    'strengthened-precondition', 'rebound-logical-definition', 'unbounded-integer-model'])
    && get('CONTRACT-14')?.expected?.outcome === 'reject-each-unapproved-change'
    && get('CONTRACT-14')?.expected?.same_display_name_sufficient === false
    && get('CONTRACT-14')?.expected?.independent_expected_claim_required === true, 'contract oracle: statement export');
  const invalid = get('CONTRACT-15')?.input?.variants;
  check(Array.isArray(invalid) && sameStrings(invalid.map(v => `${v.case}:${v.diagnostic}`),
    ['unbound-contract-variable:invalid', 'predicate-type-mismatch:invalid', 'unsupported-host-protocol:unsupported'])
    && get('CONTRACT-15')?.expected?.ordinary_program_valid === true
    && get('CONTRACT-15')?.expected?.claim_silently_weakened === false
    && get('CONTRACT-15')?.expected?.required_proof_admission === 'blocked', 'contract oracle: unsupported declaration');
}

// Fixed document oracles only. This does not parse or evaluate calculator expressions.
function validateCalculatorOracles(get, check) {
  const integer = value => typeof value === 'string' && value.length <= 1024
    && /^(0|-?[1-9]\d*)$/.test(value);
  const gcd = (a, b) => { while (b !== 0n) [a, b] = [b, a % b]; return a; };
  const normalized = value => {
    if (!integer(value?.numerator) || !integer(value?.denominator)) return false;
    const n = BigInt(value.numerator), d = BigInt(value.denominator);
    return d > 0n && gcd(n < 0n ? -n : n, d) === 1n;
  };
  const exact = {
    '1 / 3 + 1 / 6': ['1', '2'], '0.1 + 0.2': ['3', '10'],
    '0.00000000000000000001': ['1', '100000000000000000000'],
    '9223372036854775807 + 1': ['9223372036854775808', '1'],
    '9007199254740993 - 9007199254740992': ['1', '1'],
    '2 + 3 * 4': ['14', '1'], '(2 + 3) * 4': ['20', '1'], '10 - 3': ['7', '1'],
    '8 / 4 / 2': ['1', '1'], '-2^2': ['-4', '1'], '(-2)^2': ['4', '1'],
    '2^3^2': ['512', '1'], '2^-3': ['1', '8'], '4^(2/2)': ['4', '1'], '0^0': ['1', '1'],
  };
  const vectors = get('CALC-01')?.input?.vectors;
  check(get('CALC-01')?.input?.numeric_mode === 'exact-rational'
    && Array.isArray(vectors) && sameStrings(vectors.map(v => v.expression), Object.keys(exact))
    && vectors.every(v => normalized(v) && v.numerator === exact[v.expression][0] && v.denominator === exact[v.expression][1])
    && get('CALC-01')?.expected?.binary_float_parsing === false
    && get('CALC-01')?.expected?.fixed_width_narrowing === false,
  'calculator oracle: exact arithmetic and precedence');
  const errors = get('CALC-03')?.input?.vectors;
  const expectedErrors = { '1 / 0': 'DivisionByZero', '0 / 0': 'DivisionByZero', '0^-1': 'DivisionByZero', '4^(1/2)': 'NonIntegerExponent' };
  check(Array.isArray(errors) && sameStrings(errors.map(v => v.expression), Object.keys(expectedErrors))
    && errors.every(v => v.category === expectedErrors[v.expression])
    && get('CALC-03')?.expected?.successful_numeric_result === false
    && get('CALC-03')?.expected?.state_unchanged === true, 'calculator oracle: numeric errors');
  const rational = get('CALC-04')?.input?.vectors;
  check(sameStrings(get('CALC-04')?.input?.paths, ['constructor', 'decoder'])
    && Array.isArray(rational) && rational.length === 5
    && sameStrings(rational.map(v => `${v.numerator}/${v.denominator}`), ['2/4', '-4/-8', '1/-2', '0/-7', '1/0'])
    && rational.every(v => {
      if (!integer(v.numerator) || !integer(v.denominator)) return false;
      if (v.denominator === '0') return v.error === 'ZeroDenominator' && v.result === undefined;
      return normalized(v.result) && v.error === undefined
        && BigInt(v.numerator) * BigInt(v.result.denominator) === BigInt(v.result.numerator) * BigInt(v.denominator);
    }) && get('CALC-04')?.expected?.invalid_rat_exposed === false, 'calculator oracle: normalized rational boundary');
  const conversions = get('CALC-05')?.input?.vectors;
  const conversionKeys = ['to-I64:9223372036854775807/1', 'to-I64:-9223372036854775808/1',
    'to-I64:9223372036854775808/1', 'to-I64:-9223372036854775809/1', 'to-I64:1/2', 'from-I64:-9223372036854775808/1'];
  check(Array.isArray(conversions) && sameStrings(conversions.map(v => `${v.direction}:${v.numerator}/${v.denominator}`), conversionKeys)
    && conversions.every(v => {
      if (!normalized(v)) return false;
      const n = BigInt(v.numerator);
      if (v.direction === 'from-I64') return v.value === v.numerator && v.denominator === '1' && v.error === undefined;
      if (v.denominator !== '1') return v.error === 'NonIntegral' && v.value === undefined;
      if (n < -(1n << 63n) || n >= (1n << 63n)) return v.error === 'OutOfRange' && v.value === undefined;
      return v.value === v.numerator && v.error === undefined;
    }) && get('CALC-05')?.input?.core_source === '9223372036854775807 1 +'
    && get('CALC-05')?.expected?.core_output?.type === 'I64'
    && get('CALC-05')?.expected?.core_output?.value === '-9223372036854775808'
    && get('CALC-05')?.expected?.implicit_conversion === false
    && get('CALC-05')?.expected?.core_semantics_changed === false, 'calculator oracle: I64 boundary');
  check(get('CALC-02')?.input?.span_units === 'utf8-bytes-half-open'
    && get('CALC-02')?.expected?.state_unchanged === true && get('CALC-02')?.expected?.silent_correction === false
    && get('CALC-02')?.expected?.calculator_core_host_requests === 0, 'calculator oracle: diagnostics');
  const session = get('CALC-06');
  check(session?.expected?.failed_submission_state_unchanged === true
    && session.expected.earlier_function_rebound === false && session.expected.calculator_core_host_requests === 0
    && session.input?.operations?.some(v => v.expression === 'f(1)' && v.expected_fraction === '11/1')
    && session.input?.operations?.some(v => v.expression === 'g(2)' && v.expected_fraction === '3/1')
    && session.input?.operations?.some(v => v.expression === 'x' && v.expected_fraction === '20/1'), 'calculator oracle: lexical session');
  check(get('CALC-07')?.expected?.rejected_definition_installed === false
    && get('CALC-07')?.expected?.evaluations_during_definition_admission === 0, 'calculator oracle: definition admission');
  check(sameStrings(get('CALC-08')?.input?.limit_categories, ['input-bytes', 'numeric-digits', 'intermediate-integer-bits',
    'syntax-depth', 'call-depth', 'evaluation-work', 'output-size'])
    && get('CALC-08')?.input?.state_control === 'definition-result-exceeds-output-budget-before-commit'
    && get('CALC-08')?.expected?.outcome === 'ResourceLimit-for-each-category'
    && get('CALC-08')?.expected?.state_unchanged === true
    && ['approximation_fallback', 'wrapped_result', 'quota_counts_as_success', 'host_timeout_is_resource_limit']
      .every(key => get('CALC-08')?.expected?.[key] === false), 'calculator oracle: bounded exact computation');
  check(get('CALC-09')?.expected?.silent_approximation === false
    && get('CALC-09')?.expected?.truncated_decimal_labeled_exact === false
    && get('CALC-09')?.input?.vectors?.some(v => v.fraction === '1/3' && v.mode === 'finite-decimal' && v.display === '1/3'),
  'calculator oracle: exact display');
  check(get('CALC-10')?.expected?.same_acceptance_boundary === true
    && get('CALC-10')?.expected?.invalid_edit_published === false
    && get('CALC-10')?.expected?.candidate_body_host_requests === 0
    && get('CALC-10')?.input?.variants?.some(v => v.case === 'stale-base-revision' && v.outcome === 'reject-before-publication'),
  'calculator oracle: authoring boundary');
  check(sameStrings(get('CALC-11')?.input?.task_families, ['exact-evaluator', 'integer-powers', 'lexical-functions', 'parser-repair', 'numeric-capacity'])
    && sameStrings(get('CALC-11')?.input?.authoring_routes, ['stack-only', 'local-names', 'structured-edits'])
    && sameStrings(get('CALC-11')?.input?.required_record_fields, ['model_version', 'task_revision', 'starting_revision', 'prompts',
      'supplied_context', 'tool_interfaces', 'budgets', 'attempts', 'patches', 'compiler_diagnostics',
      'compiler_backend_revision', 'numeric_library_revision', 'outcomes'])
    && sameStrings(get('CALC-11')?.input?.metrics, ['first_attempt_acceptance', 'final_acceptance', 'repair_attempts',
      'total_tokens', 'elapsed_time', 'regressions', 'incorrect_success_claims'])
    && get('CALC-11')?.expected?.independent_acceptance_required === true
    && ['producer_can_change_acceptance', 'failed_runs_omitted', 'universal_AI_suitability_claimed']
      .every(key => get('CALC-11')?.expected?.[key] === false), 'calculator oracle: benchmark integrity');
  check(get('CALC-12')?.expected?.shared_implementation_is_sole_oracle === false
    && get('CALC-12')?.expected?.actual_Noble_backend_required === true
    && get('CALC-12')?.expected?.tests_establish_universal_proof === false
    && sameStrings(get('CALC-12')?.input?.incorrect_implementations, ['integer-division', 'binary-float-decimal-parser',
      'I64-wrapping', 'reversed-subtraction', 'left-associated-power', 'non-normalized-rational', 'failure-mutates-session']),
  'calculator oracle: independent evidence');
  check(get('CALC-13')?.expected?.bootstrap_expanded === false
    && get('CALC-13')?.expected?.calculator_blocks_M4 === false
    && get('CALC-13')?.expected?.new_kernel_mechanisms_required === false, 'calculator oracle: application scope');
}

// Fixed worker document controls. No candidate checking, task execution, or scheduling occurs here.
function validateWorkerOracles(get, check) {
  const sameJson = (a, b) => {
    if (Array.isArray(b)) return Array.isArray(a) && a.length === b.length && b.every((v, i) => sameJson(a[i], v));
    if (object(b)) return object(a) && sameStrings(Object.keys(a), Object.keys(b))
      && Object.entries(b).every(([k, v]) => sameJson(a[k], v));
    return a === b;
  };
  const matches = (actual, expected) => object(actual)
    && Object.entries(expected).every(([key, value]) => sameJson(actual[key], value));
  const fields = (id, section, expected, label) => check(matches(get(id)?.[section], expected), `worker oracle: ${label}`);
  const matrix = (id, expected, label) => {
    const variants = get(id)?.input?.variants;
    check(Array.isArray(variants) && sameStrings(variants.map(v => v?.case), Object.keys(expected))
      && variants.every(v => matches(v, expected[v.case])), `worker oracle: ${label}`);
  };
  for (let i = 1; i <= 12; i++) {
    const id = `WORKER-${String(i).padStart(2, '0')}`;
    check(get(id)?.profile === 'Worker-Design', `worker oracle: required case ${id}`);
  }
  fields('WORKER-01', 'input', {
    candidate_supplied_after_coordinator_compilation: true, expected_interface: 'State Event -- State List<Action> ! {}',
    schemas: 'exact-resource-free-worker-v1', initial_state: { count: { type: 'I64', value: '0' }, pending: null },
    events: [{ kind: 'Submit', attempt: 'attempt-1', text: 'ping' }, { kind: 'Cancelled', attempt: 'attempt-1' }],
    shell_authority: 'independently-authorized-send-capability', capability_captured_by_worker: false,
    task_order: ['admission-commit', 'cancellation-commit', 'late-native-completion'],
    worker_service_access: 'none', preparation_service: 'explicit-before-invocation',
  }, 'generated worker setup');
  fields('WORKER-01', 'input', {
    transition_controls: [
      { case: 'submit-while-pending', count: { type: 'I64', value: '1' }, pending: 'attempt-1', event: { kind: 'Submit', attempt: 'attempt-2', text: 'ignored' }, expected_count: { type: 'I64', value: '1' }, expected_pending: 'attempt-1', expected_actions: [] },
      { case: 'cancel-wrong-attempt', count: { type: 'I64', value: '1' }, pending: 'attempt-1', event: { kind: 'Cancelled', attempt: 'attempt-2' }, expected_count: { type: 'I64', value: '1' }, expected_pending: 'attempt-1', expected_actions: [] },
      { case: 'cancel-without-pending-attempt', count: { type: 'I64', value: '1' }, pending: null, event: { kind: 'Cancelled', attempt: 'attempt-1' }, expected_count: { type: 'I64', value: '1' }, expected_pending: null, expected_actions: [] },
      { case: 'wrapping-submission-count', count: { type: 'I64', value: '9223372036854775807' }, pending: null, event: { kind: 'Submit', attempt: 'attempt-1', text: 'ping' }, expected_count: { type: 'I64', value: '-9223372036854775808' }, expected_pending: 'attempt-1', expected_actions: [{ kind: 'Send', attempt: 'attempt-1', text: 'ping' }] },
    ],
  }, 'worker transition controls');
  fields('WORKER-01', 'expected', {
    outcome: 'cancelled-without-resurrection',
    states: [{ count: { type: 'I64', value: '1' }, pending: 'attempt-1' }, { count: { type: 'I64', value: '1' }, pending: null }],
    actions: [[{ kind: 'Send', attempt: 'attempt-1', text: 'ping' }], []],
    worker_host_requests: 0, shell_operation_attempts: 1, preparation_trace_separate: true,
    owner_after_admission: 'task', state_at_cancel_ack: 'Retiring', pins_at_cancel_ack: 1,
    state_after_late_completion: 'Retired', pins_after_late_completion: 0,
    successful_result_deliveries: 0, late_worker_events: 0, owner_resurrections: 0,
    external_send_outcome_at_cancel: 'unknown', external_rollback_claimed: false,
  }, 'worker state and retirement');
  matrix('WORKER-02', {
    'reversed-state-event-stack': { outcome: 'interface-reject' }, 'extra-output-value': { outcome: 'interface-reject' },
    'same-layout-different-schema-identity': { outcome: 'schema-reject' }, 'effect-outside-dispatch-ceiling': { outcome: 'effect-reject' },
    'resource-hidden-in-worker-capture': { outcome: 'eligibility-reject' }, 'heterogeneous-list-without-adapter': { outcome: 'interface-reject' },
  }, 'interface rejection matrix');
  fields('WORKER-02', 'expected', { outcome: 'reject-each-variant', dynamic_Any_fallback: false, implicit_schema_conversion: false }, 'interface boundary');
  fields('WORKER-03', 'input', {
    operands_supplied_after_compilation: true, compiler_service: 'disabled',
    program_interfaces: ['I64 -- I64 ! {}', 'I64 -- I64 ! {test.emit}'], destination_interface: 'I64 -- I64 ! {test.emit}',
    adaptation: 'prechecked-effect-inclusion-rule', operations: ['adapt', 'store-in-list', 'select', 'duplicate', 'compose'],
  }, 'runtime interface adaptation');
  fields('WORKER-03', 'expected', {
    outcome: 'checked-common-interface', source_compiler_calls: 0, guest_requests: 0, protected_operations: 0,
    recipe_nodes_unchanged_by_widening: true, original_interface_unchanged: true, destination_witness_retained: true, duplicate_interfaces: 'same-instantiation',
    authority_created: false, identity_comparison_includes_interface_witness: true,
  }, 'effect widening invariant');
  fields('WORKER-04', 'input', {
    expected_interface: 'unchanged-instantiated-interface', context: 'same-exact-semantic-dependencies',
    captured_data: 'runtime-supplied-text', self_reference: 'retained-owner-identity', preparation_authority: 'explicit',
    export_authority: 'explicit-for-destination-and-capture', limits: 'sufficient',
    variants: ['reflect-prepare', 'export-import', 'rebound-display-name-original-dependency-retained', 'different-optimizer-same-semantics'],
  }, 'round-trip context');
  fields('WORKER-04', 'expected', {
    outcome: 'preserve-each-variant', normalized_recipe: 'same', interface_witnesses: 'same', captures: 'same',
    dependency_identities: 'same', recursive_owner: 'same', abstract_program_value_identity: 'same',
    artifact_bytes_required_identical: false, budget_outcomes_required_identical: false, unchecked_acceptance_transferred: false,
    proof_applicability_transferred: false, live_authority_transferred: false, stable_wire_encoding_claimed: false, candidate_body_requests: 0,
  }, 'round-trip invariant');
  matrix('WORKER-05', {
    'missing-exact-transitive-dependency': { outcome: 'reject' }, 'same-name-wrong-host-contract-version': { outcome: 'reject' },
    'capture-schema-witness-mismatch': { outcome: 'reject' }, 'foreign-scoped-self-owner': { outcome: 'reject' },
    'invalid-body-with-accepted-flag-and-digest': { outcome: 'reject' }, 'unrelated-executable-artifact': { outcome: 'reject' },
    'unsupported-format-revision': { outcome: 'unsupported' },
  }, 'package rejection matrix');
  fields('WORKER-05', 'input', { dependency_fetch_authority: 'absent' }, 'package retrieval authority');
  fields('WORKER-05', 'expected', {
    outcome: 'no-variant-admitted', implicit_fetches: 0, version_substitutions: 0, partial_programs_published: 0,
    signature_or_digest_bypasses_checks: false,
  }, 'package admission');
  fields('WORKER-06', 'input', {
    scope: 'supported-finite-candidate-subset',
    limit_categories: ['input-bytes', 'decoded-nodes', 'nesting', 'dependency-traversal', 'type-stack-size', 'checking-work', 'memory', 'diagnostic-output'],
    other_limits: 'sufficient', cases_per_category: ['at-limit', 'one-unit-over-limit'],
    diagnostic_control: 'invalid-candidate-at-limit-and-truncated-one-unit-over', valid_candidate_categories: 'all-except-diagnostic-output',
    nested_stage_control: 'shared-remaining-request-budget',
  }, 'admission budget matrix');
  fields('WORKER-06', 'expected', {
    outcome: 'each-boundary-matches', at_limit_valid_candidate: 'accepted', over_limit_valid_candidate: 'exhausted',
    diagnostic_limit_on_invalid_candidate: 'invalid-with-truncated-diagnostic', over_limit_partial_programs: 0,
    nested_budget_reset: false, allocation_before_limit_check: false, exhaustion_proves_invalidity: false,
  }, 'admission budget outcomes');
  matrix('WORKER-07', {
    'cyclic-witness-premises': { outcome: 'invalid' }, 'missing-premise': { outcome: 'invalid' },
    'witness-for-another-node': { outcome: 'invalid' }, 'infinite-type-equation': { outcome: 'invalid' },
    'candidate-replaces-expected-interface': { outcome: 'invalid' }, 'candidate-selects-unadmitted-environment': { outcome: 'invalid' },
    'unsupported-witness': { outcome: 'unsupported' }, 'unsupported-candidate-format': { outcome: 'unsupported' },
    'injected-checker-internal-failure': { outcome: 'internal-failure' },
  }, 'candidate witness matrix');
  fields('WORKER-07', 'input', { expected_interface_owner: 'consumer-not-producer' }, 'independent admission input');
  fields('WORKER-07', 'expected', { outcome: 'no-variant-admitted', accepted_programs: 0, prior_session_unchanged: true, fallback_checker_used: false }, 'candidate admission');
  matrix('WORKER-08', {
    'cancel-before-completion': { order: ['admit', 'cancel', 'complete', 'deliver'], terminal_state: 'Retired', result_deliveries: 0, result_owner: 'none', pins_at_cancel_ack: 1 },
    'complete-cancel-before-delivery': { order: ['admit', 'complete-with-resource-result', 'cancel', 'deliver'], terminal_state: 'Retired', result_deliveries: 0, result_owner: 'none', undelivered_result_retired: true },
    'delivery-before-cancel': { order: ['admit', 'complete', 'deliver', 'cancel'], terminal_state: 'Delivered', result_deliveries: 1, result_owner: 'receiver', late_cancel_revokes_result: false },
    'domain-error-returns-owner': { order: ['admit', 'complete-with-declared-error-owner', 'deliver'], terminal_state: 'Delivered', result_deliveries: 1, result_owner: 'receiver-error-branch', normal_typed_return: true },
    'task-quota-preflight-failure': { order: ['deny-admission'], terminal_state: 'NotAdmitted', result_deliveries: 0, result_owner: 'caller-input', protected_operations: 0 },
    'buffer-quota-preflight-failure': { order: ['deny-admission'], terminal_state: 'NotAdmitted', result_deliveries: 0, result_owner: 'caller-input', protected_operations: 0 },
    'stale-generation-completion': { order: ['admit-new-generation', 'old-generation-complete'], terminal_state: 'Pending', result_deliveries: 0, result_owner: 'task', current_generation_changed: false },
    'wrong-context-callback': { order: ['admit', 'foreign-context-complete'], terminal_state: 'Pending', result_deliveries: 0, result_owner: 'task', current_generation_changed: false },
    'duplicate-completion-after-retirement': { order: ['admit', 'cancel', 'complete', 'duplicate-complete'], terminal_state: 'Retired', result_deliveries: 0, result_owner: 'none', native_pin_releases: 1 },
    'oversized-buffered-result': { order: ['admit', 'result-exceeds-reservation', 'finish-retirement'], terminal_state: 'Retired', result_deliveries: 0, result_owner: 'none', primary_outcome: 'budget-exhaustion' },
    'trap-with-native-pin': { order: ['admit', 'trap', 'late-native-completion'], terminal_state: 'Retired', result_deliveries: 0, result_owner: 'none', primary_outcome: 'trap' },
    'delivery-before-ready': { order: ['admit', 'deliver'], terminal_state: 'Pending', result_deliveries: 0, result_owner: 'task' },
    'repeated-cancellation-during-retirement': { order: ['admit', 'cancel', 'cancel'], terminal_state: 'Retiring', result_deliveries: 0, result_owner: 'host-retirement', cleanup_restarted: false },
  }, 'async ownership matrix');
  fields('WORKER-08', 'expected', {
    outcome: 'each-order-matches', owner_resurrections: 0, double_releases: 0, unaccounted_pins: 0,
    cancellation_ack_proves_native_completion: false, automatic_retry: false, distributed_exactly_once_claimed: false,
  }, 'async ownership invariant');
  matrix('WORKER-09', {
    'finite-pure-step': { outcome: 'normal-return', typed_output: true }, 'domain-Result-error': { outcome: 'normal-return', typed_output: true },
    'pure-loop-work-limit': { outcome: 'budget-exhaustion', typed_output: false }, 'runtime-builder-allocation-limit': { outcome: 'budget-exhaustion', typed_output: false },
    'nested-invocation-budget': { outcome: 'budget-exhaustion', typed_output: false },
    'host-deadline-native-work-outstanding': { outcome: 'host-deadline', typed_output: false, native_owner: 'host-retirement' },
    'permitted-suspension': { outcome: 'suspended', typed_output: false, pending_owner: 'task' }, 'guest-trap': { outcome: 'trap', typed_output: false },
    'cancelled-invocation': { outcome: 'cancelled', typed_output: false }, 'runtime-internal-failure': { outcome: 'internal-failure', typed_output: false },
  }, 'execution outcome matrix');
  fields('WORKER-09', 'input', { accounting_revision: 'symbolic-test-profile-v1', max_guest_work_between_checks: 8 }, 'execution accounting context');
  fields('WORKER-09', 'expected', {
    outcome: 'each-outcome-distinct', observed_max_guest_work_between_checks: 8, pure_code_exempt_from_limits: false,
    nested_budget_reset: false, failed_output_exposed_as_T: false, host_deadline_is_deterministic_fuel: false,
    exhaustion_proves_divergence: false, budget_silently_increased: false, cross_backend_budget_equality_required: false,
  }, 'execution bounds');
  fields('WORKER-10', 'input', {
    declared_effects: ['test.send'], invocation: 'invocation-1', terminal_event: 'cancellation',
    operations: [
      { attempt: 'attempt-1', observations: ['requested', 'admitted', 'completed'] },
      { attempt: 'attempt-2', observations: ['requested', 'denied'] },
      { attempt: 'attempt-3', observations: ['requested', 'admitted', 'externally-unknown'] },
    ], variants: ['sufficient-observation-budget', 'observation-output-exhausted', 'diagnostic-output-exhausted'],
  }, 'operation observation setup');
  fields('WORKER-10', 'expected', {
    outcome: 'cancelled-with-prior-effects-retained',
    required_fields: ['invocation', 'program-build-context', 'execution-profile', 'primary-outcome', 'ownership-disposition',
      'operation-attempt-correlation', 'omitted-observation-scope', 'unknown-operation-outcomes'],
    primary_outcome: 'cancelled', guest_requests: 3, protected_operation_admissions: 2, completed_operations: 1,
    denied_attempt_counted_as_request: true, attempt_3_outcome: 'unknown', omission_marked_when_exhausted: true,
    prior_effects_rolled_back: false, effect_plan_counts_as_completion: false, missing_observations_imply_no_effects: false, automatic_retry: false,
  }, 'operation observation invariant');
  fields('WORKER-11', 'input', {
    milestones: ['MW1', 'MW2'], open_gates: ['worker-interface-checker', 'declared-schemas-and-modules', 'preparation-service',
      'package-encoding', 'bounded-execution-profile', 'native-async-interfaces'],
  }, 'worker entry gates');
  fields('WORKER-11', 'expected', {
    outcome: 'design-only-until-executed-evidence', new_kernel_syntax: false, bootstrap_expanded: false,
    ordinary_core_blocked: false, calculator_replaced: false, swarm_runtime_claimed: false, durable_replay_claimed: false, syndicate_replaced: false,
  }, 'worker scope');
  fields('WORKER-12', 'input', { declared_effects: ['test.code-export'], destination_authority: 'denied', program_valid: true, digest_valid: true }, 'export authority setup');
  fields('WORKER-12', 'expected', {
    outcome: 'denied', guest_requests: 1, protected_operations: 0, capture_published: false,
    capturability_grants_export_authority: false, candidate_body_requests: 0, denial_recorded: true,
  }, 'export authority invariant');
}

// Document expectations only. This does not run Octet, validate credentials, or construct live witnesses.
function validateOctetAdoptionOracles(get, check) {
  const matches = (actual, expected) => object(actual) && Object.entries(expected).every(([key, value]) =>
    Array.isArray(value) ? sameStrings(actual[key], value) : actual[key] === value);
  const fields = (id, section, expected, label) => check(matches(get(id)?.[section], expected), `Octet adoption oracle: ${label}`);
  const matrix = (id, expected, label) => {
    const rows = get(id)?.input?.variants;
    check(Array.isArray(rows) && sameStrings(rows.map(v => v?.case), Object.keys(expected))
      && rows.every(v => matches(v, expected[v.case])), `Octet adoption oracle: ${label}`);
  };
  const kinds = ['runtime', 'runtime', 'static', 'adapter', 'review', 'admission', 'admission', 'review', 'review', 'review'];
  kinds.forEach((kind, index) => {
    const id = `OCTET-${String(index + 1).padStart(2, '0')}`;
    check(get(id)?.profile === 'Octet-Adoption-Design' && get(id)?.kind === kind, `Octet adoption oracle: required case ${id}`);
  });
  fields('OCTET-01', 'input', { authority_facts: 'host-validated-independent-of-producer', plan: 'exact-operation-arguments-and-context', witness_mode: 'one-shot' }, 'authority setup');
  matrix('OCTET-01', {
    'allow-and-observed-success': { witnesses_created: 1, witness_consumptions: 1, protected_operations: 1, receipt: 'operation-success' },
    'deny': { witnesses_created: 0, witness_consumptions: 0, protected_operations: 0, receipt: 'denial' },
    'preflight-capacity-failure': { witnesses_created: 1, witness_consumptions: 0, protected_operations: 0, receipt: 'preflight-failure', caller_retains_valid_witness: true },
    'external-outcome-unknown-after-commit': { witnesses_created: 1, witness_consumptions: 1, protected_operations: 1, receipt: 'unknown', witness_restored: false },
  }, 'authority phase matrix');
  fields('OCTET-01', 'expected', { outcome: 'each-phase-distinct', pure_decision_creates_live_authority: false,
    admission_commit_before_protected_operation: true, attempt_recorded_with_commit: true, automatic_retry: false, receipt_grants_authority: false }, 'authority invariant');
  fields('OCTET-02', 'input', { declared_effects: ['test.protected-send'], variants: ['changed-operation-contract', 'changed-plan-target',
    'changed-plan-amount', 'wrong-actor', 'wrong-owner-context', 'changed-policy-revision', 'expired', 'revoked', 'quota-exhausted',
    'missing-current-facts', 'wrong-resource-kind', 'stale-generation', 'consumed-witness-replay', 'serialized-witness-tag'] }, 'witness binding matrix');
  fields('OCTET-02', 'expected', { outcome: 'deny-each-variant', requests_per_variant: 1, protected_operations: 0,
    attempts_admitted: 0, resource_resurrections: 0, missing_fact_defaults_to_allow: false, digest_alone_sufficient: false }, 'witness rejection');
  fields('OCTET-03', 'input', { variants: ['allow-data-as-resource', 'raw-id-as-resource', 'default-construct-witness',
    'public-field-construction', 'dup-live-witness', 'drop-live-witness', 'capture-live-witness', 'serialize-live-witness'] }, 'witness construction matrix');
  fields('OCTET-03', 'expected', { outcome: 'reject-each-variant', live_witnesses_created: 0, new_kernel_type_required: false }, 'witness construction invariant');
  matrix('OCTET-04', {
    'matching-approved-success-observation': { receipt: 'operation-success', trusted: true },
    'matching-approved-failure-observation': { receipt: 'operation-failure', trusted: true },
    'plan-as-success': { receipt: 'reject' }, 'witness-as-success': { receipt: 'reject' }, 'attempt-as-success': { receipt: 'reject' },
    'failed-observation-as-success': { receipt: 'reject' }, 'unknown-observation-as-success': { receipt: 'reject' },
    'missing-observation-as-success': { receipt: 'reject' }, 'different-attempt-observation': { receipt: 'reject' },
    'schema-valid-import-without-provenance': { receipt: 'untrusted-description', trusted: false },
    'cancelled-invocation-late-confirmed-external-success': { receipt: 'operation-success', trusted: true, invocation_outcome: 'cancelled', successful_task_deliveries: 0 },
  }, 'receipt constructor matrix');
  fields('OCTET-04', 'expected', { outcome: 'each-claim-scoped', owner_resurrections: 0, receipt_is_capability: false,
    receipt_is_arbitrary_behavioral_proof: false, integrity_is_authenticity: false }, 'receipt scope');
  fields('OCTET-05', 'input', { states: ['Pending', 'Ready', 'Retiring', 'Retired', 'Delivered'], events: ['Complete', 'Deliver', 'Cancel'] }, 'lifecycle domain');
  matrix('OCTET-05', {
    'all-pairs-explicit-with-invalid-results': { outcome: 'coverage-accepted' }, 'named-alternative-groups-cover-all-pairs': { outcome: 'coverage-accepted' },
    'missing-pair': { outcome: 'reject' }, 'wildcard-covers-new-event': { outcome: 'reject' }, 'guard-without-remaining-case': { outcome: 'reject' },
    'changed-schema-reuses-old-coverage': { outcome: 'reject' }, 'unsupported-transition-shape': { outcome: 'unsupported' },
  }, 'lifecycle coverage matrix');
  fields('OCTET-05', 'expected', { outcome: 'each-coverage-control-matches', constructor_pairs: 15, invalid_transition_preserves_declared_ownership: true,
    all_Noble_programs_require_FSM_profile: false, full_async_protocol_coverage_claimed: false, coverage_proves_liveness: false }, 'lifecycle scope');
  matrix('OCTET-06', {
    'valid-approved-constructor': { outcome: 'accepted' }, 'valid-explicit-unit-conversion': { outcome: 'accepted' },
    'valid-zero-budget': { outcome: 'accepted-data-with-no-work-authorized' }, 'AttemptId-as-TaskGeneration': { outcome: 'type-reject' },
    'AdmissionBudget-as-ExecutionFuel': { outcome: 'type-reject' }, 'duration-as-byte-count': { outcome: 'type-reject' },
    'primitive-alias-as-nominal-identity': { outcome: 'type-reject' }, 'unchecked-overflowing-unit-conversion': { outcome: 'invariant-reject' },
    'default-bypasses-invariant': { outcome: 'invariant-reject' }, 'decoder-bypasses-invariant': { outcome: 'invariant-reject' },
    'package-literal-forges-opaque-value': { outcome: 'invariant-reject' },
  }, 'nominal domain matrix');
  fields('OCTET-06', 'expected', { outcome: 'each-domain-control-matches', invariant_failure_defaulted_to_success: false,
    core_I64_wrapping_changed: false, general_refinement_inference_required: false }, 'nominal domain scope');
  matrix('OCTET-07', {
    'exact-resolved-pure-dependencies': { outcome: 'accepted' }, 'typed-indirect-call-with-complete-allowed-bound': { outcome: 'accepted' },
    'supported-self-recursion-with-checked-signature': { outcome: 'accepted-without-termination-claim' },
    'pure-display-name-calls-clock-under-empty-bound': { outcome: 'effect-reject' }, 'summary-erases-known-direct-effect': { outcome: 'effect-reject' },
    'wrong-provider-summary-identity': { outcome: 'reject' }, 'missing-operation-contract': { outcome: 'reject' },
    'unresolved-call-without-trusted-interface': { outcome: 'reject' }, 'closure-budget-exhausted': { outcome: 'exhausted' },
  }, 'resolved effect matrix');
  fields('OCTET-07', 'expected', { outcome: 'each-effect-control-matches', unknown_defaults_to_pure: false,
    all_runtime_program_values_must_be_enumerated: false, source_token_receipt_replaces_language_judgment: false }, 'resolved effect scope');
  matrix('OCTET-08', {
    'complete-gated-policy-and-compiler-facts': { outcome: 'scoped-policy-acceptance' },
    'deterministic-profile-without-architecture-pointer': { outcome: 'architecture-gate-unsatisfied' },
    'inventory-or-advisory-only': { outcome: 'architecture-gate-unsatisfied' }, 'stale-Nickel-export': { outcome: 'reject' },
    'undeclared-role-or-provider': { outcome: 'reject' }, 'adapter-constructed-in-domain-core': { outcome: 'reject' },
    'executor-without-witness-contract': { outcome: 'reject' }, 'receipt-constructor-without-observation': { outcome: 'reject' },
    'missing-required-compiler-facts': { outcome: 'blocking-unknown' }, 'collection-limit-exceeded': { outcome: 'blocking-unknown' },
    'uncovered-production-binary': { outcome: 'blocking-unknown' }, 'unsupported-required-target-feature-set': { outcome: 'blocking-unknown' },
    'lint-passed-architecture-failed': { outcome: 'admission-denied' }, 'finding-allowance-or-warning-budget': { outcome: 'reject' },
  }, 'architecture matrix');
  fields('OCTET-08', 'expected', { outcome: 'each-policy-control-matches', full_deny_all_replaced_by_profile: false,
    source_identity_baseline_is_finding_allowance: false, no_std_proves_purity: false, Octet_was_executed_by_document_validator: false }, 'architecture scope');
  fields('OCTET-09', 'input', { required_bindings: ['subject', 'claim', 'source-scope', 'semantic-dependencies', 'policy', 'toolchain',
    'target-features', 'command', 'inputs', 'assumptions', 'result'] }, 'evidence bindings');
  matrix('OCTET-09', {
    'same-bindings-current-policy': { outcome: 'reuse-at-original-tier' }, 'changed-source-policy-provider-or-toolchain': { outcome: 'revalidate-before-reuse' },
    'missing-binding-or-unknown-coverage': { outcome: 'block-required-claim' }, 'failed-or-omitted-required-phase': { outcome: 'block-required-claim' },
    'source-lint-result-as-formal-proof': { outcome: 'reject-tier-promotion' }, 'artifact-integrity-as-authenticity': { outcome: 'reject-tier-promotion' },
    'fixture-Charon-analysis-as-Noble-kernel-proof': { outcome: 'reject-scope-promotion' },
    'producer-replaces-artifact-and-digest': { outcome: 'independent-trust-context-still-required' },
  }, 'evidence tier matrix');
  fields('OCTET-09', 'expected', { outcome: 'each-evidence-control-matches', historical_evidence_erased: false,
    consumer_claim_selected_by_producer: false, successful_sibling_phase_hides_failure: false, typed_policy_is_executed_enforcement: false }, 'evidence scope');
  fields('OCTET-10', 'input', { design_source_revision: OCTET_ADOPTION_POLICY.design_source_revision, milestones: Object.keys(OCTET_MILESTONE_REQUIREMENTS) }, 'adoption source and gates');
  fields('OCTET-10', 'expected', { outcome: 'spec-adaptation-not-implementation', new_kernel_syntax: false, bootstrap_expanded: false,
    Rust_recursion_lint_is_Noble_recursion_ban: false, Aeneas_route_replaced: false, Verus_migration_required: false,
    Octet_implementation_imported: false, toolchain_pin_selected_by_design_reference: false, ambient_sibling_dependency_allowed: false,
    runtime_or_proof_claimed: false }, 'adoption non-claims');
}

export function validate(bundle, { ignoreLedger = false } = {}) {
  const errors = [];
  const check = (condition, message) => { if (!condition) errors.push(message); };
  let family, status, roadmap, cases;
  try {
    // Parse all active JSON, including files not referenced by the family manifest.
    for (const [file, text] of bundle.texts) {
      if (file.startsWith('specs/') && file.endsWith('.json')) JSON.parse(text);
    }
    family = parse(bundle, 'specs/spec-family.json');
    status = parse(bundle, 'specs/STATUS.json');
    roadmap = parse(bundle, 'specs/roadmap.json');
    cases = allCases(bundle, family);
    check(family.revision === REVISION && status.revision === REVISION, 'revision: family/status mismatch');
    check(family.lifecycle === '.cairn', 'Cairn authority: native lifecycle missing');
    check(roadmap.revision === REVISION, 'revision: roadmap mismatch');
    const template = parse(bundle, 'specs/verification/toolchain-lock.template.json');
    check(template.revision === REVISION, 'revision: toolchain template mismatch');
    check(template.status === 'unselected-template-not-a-lock' && template.backend_experiments?.selected_representation === null
      && template.optional_byte_view_dependency === null, 'unselected template: backend/dependency is not a selection record');
    check(status.origin === 'greenfield-user-confirmed', 'origin: greenfield decision missing');
    for (const [field, expected] of Object.entries(AENEAS_POLICY)) {
      check(status.verification_policy?.[field] === expected, `Aeneas policy: ${field}`);
    }
    check(Array.isArray(status.verification_policy?.approved_exceptions), 'Aeneas policy: exception inventory missing');
    if (!status.compiler_exists && !status.runtime_exists) {
      check(status.verification_policy?.coverage_inventory === 'absent'
        && status.verification_policy?.approved_exceptions?.length === 0, 'Aeneas policy: false greenfield coverage/exception claim');
    }
    check(template.proof?.primary_route === AENEAS_POLICY.primary_route
      && !['verus', 'verus_rustc', 'solver'].some(key => Object.hasOwn(template.proof ?? {}, key))
      && template.optional_boundary_tools?.verus === null, 'Aeneas template: Verus must remain optional and unselected');
    for (const [field, expected] of Object.entries(CONTRACT_POLICY)) {
      check(status.program_contracts?.[field] === expected, `contract policy: ${field}`);
    }
    check(sameStrings(status.program_contracts?.claim_outcomes, CONTRACT_OUTCOMES), 'contract policy: claim outcomes');
    for (const [field, expected] of Object.entries(CALCULATOR_POLICY)) {
      check(status.calculator?.[field] === expected, `calculator policy: ${field}`);
    }
    check(status.components.some(c => c.id === 'exact-calculator'), 'calculator policy: component status missing');
    if (!status.compiler_exists || !status.runtime_exists) {
      check(status.calculator?.implementation === 'absent' && status.calculator?.execution === 'not-run'
        && status.calculator?.benchmark_execution === 'not-run' && status.calculator?.proof === 'open',
      'calculator policy: false greenfield result');
    }
    for (const [field, expected] of Object.entries(WORKER_POLICY)) {
      check(status.worker_contracts?.[field] === expected, `worker policy: ${field}`);
    }
    if (!status.compiler_exists || !status.runtime_exists) {
      check(status.worker_contracts?.implementation === 'absent' && status.worker_contracts?.execution === 'not-run'
        && status.worker_contracts?.proof === 'open', 'worker policy: false greenfield result');
    }
    for (const [field, expected] of Object.entries(OCTET_ADOPTION_POLICY)) {
      check(status.octet_adoption?.[field] === expected, `Octet adoption policy: ${field}`);
    }
    if (!status.compiler_exists || !status.runtime_exists) {
      check(status.octet_adoption?.implementation === 'absent' && status.octet_adoption?.execution === 'not-run'
        && status.octet_adoption?.proof === 'open', 'Octet adoption policy: false greenfield result');
    }
    check(status.components.some(c => c.id === 'program-contracts'), 'contract policy: component status missing');
    if (!status.compiler_exists) {
      check(['concrete_grammar', 'companion_representation', 'portable_encoding']
        .every(key => status.program_contracts?.[key] === 'open'), 'contract policy: false greenfield design selection');
    }
    check(['contract_ir', 'claim_exporter', 'rule_library', 'companion_encoding', 'applicability_guards',
      'acceptance_policy', 'compatibility_run'].every(key => template.program_contracts?.[key] === null),
    'contract template: unselected fields required');
    check(status.selected_surface?.comment_prefix === '#' && status.selected_surface?.execute_word === 'run'
      && status.selected_surface?.reflection_word === 'reflect' && status.selected_surface?.legacy_aliases === false,
    'surface: selected terminology mismatch');
    const seenDocs = new Set();
    for (const doc of family.normative_documents) {
      check(!seenDocs.has(doc.id), `duplicate document: ${doc.id}`);
      seenDocs.add(doc.id);
      check(path.posix.join('specs', doc.path) === canonicalPath(doc.compatibility_path), `Cairn authority: ${doc.id}`);
      const text = bundle.texts.get(path.posix.join('specs', doc.path));
      check(typeof text === 'string' && text.includes(REVISION), `revision: ${doc.path}`);
      if (text === undefined) continue;
      if (doc.source) {
        const sourcePath = path.posix.normalize(path.posix.join('specs', doc.source));
        const source = bundle.texts.get(sourcePath);
        check(source !== undefined, `source missing: ${sourcePath}`);
        const retained = new Set(nativeRequirementIds(text));
        for (const id of legacyRequirementIds(source ?? '')) check(retained.has(id), `dropped requirement: ${id}`);
      }
    }
    for (const id of REQUIRED_DOCUMENTS) check(seenDocs.has(id), `required document missing: ${id}`);
    const reqs = requirements(bundle, family);
    const reqIds = new Set();
    for (const req of reqs) {
      check(!reqIds.has(req.id), `duplicate requirement: ${req.id}`);
      reqIds.add(req.id);
      check((ROUTES[req.id.split('-')[0]] ?? []).length > 0, `missing evidence route: ${req.id}`);
    }
    const seenCases = new Set();
    const checkGreenfield = (state, id, { compiler = true, runtime = false } = {}) => {
      if (!status.compiler_exists && compiler) {
        check(!EXECUTED.includes(state?.execution), `greenfield execution claim: ${id}`);
        check(state?.implementation !== 'implemented', `greenfield implementation claim: ${id}`);
      }
      if (!status.runtime_exists && runtime) {
        check(!EXECUTED.includes(state?.execution), `absent runtime execution claim: ${id}`);
      }
      if (!status.proof_implementation_exists) check(state?.proof !== 'accepted', `greenfield proof claim: ${id}`);
    };
    const requireEvidence = (state, evidence, id, { review = false } = {}) => {
      check(object(state), `state missing: ${id}`);
      if (!object(state)) return;
      for (const [key, values] of Object.entries(STATES)) check(values.includes(state[key]), `state ${key}: ${id}`);
      check(Array.isArray(evidence), `evidence array: ${id}`);
      if (!Array.isArray(evidence)) return;
      if (EXECUTED.includes(state.execution) || ['accepted', 'failed'].includes(state.proof) || state.trust === 'explicit') {
        check(evidence.length > 0, `evidence required: ${id}`);
      }
      if (EXECUTED.includes(state.execution)) {
        const executionKinds = review ? ['test', 'review'] : ['test'];
        check(evidence.some(e => executionKinds.includes(e?.kind) && e.result === state.execution), `execution evidence mismatch: ${id}`);
      }
      if (['accepted', 'failed'].includes(state.proof)) {
        check(evidence.some(e => ['lean-kernel', 'aeneas-lean', 'verus', 'translation-validation'].includes(e?.kind)
          && e.result === state.proof), `proof evidence mismatch: ${id}`);
      }
      for (const record of evidence) {
        check(typeof record?.claim === 'string' && record.claim.trim().length > 0, `evidence claim: ${id}`);
        check(object(record) && record.subject === id && record.revision === REVISION
          && typeof record.kind === 'string' && typeof record.result === 'string'
          && typeof record.source_revision === 'string' && record.source_revision.length > 0
          && object(record.configuration) && Object.keys(record.configuration).length > 0
          && Array.isArray(record.assumptions), `evidence binding: ${id}`);
      }
    };
    for (const file of family.scenario_files) {
      const packet = parse(bundle, `specs/${file}`);
      check(packet.schema_version === 1 && packet.revision === REVISION && typeof packet.origin === 'string', `scenario envelope: ${file}`);
      check(Array.isArray(packet.cases) && packet.cases.length > 0, `empty scenarios: ${file}`);
    }
    for (const c of cases) {
      check(typeof c.id === 'string' && !seenCases.has(c.id), `duplicate or missing case: ${c.id}`);
      seenCases.add(c.id);
      check(typeof c.profile === 'string' && KINDS.includes(c.kind), `case kind/profile: ${c.id}`);
      check(Array.isArray(c.requirements) && c.requirements.length > 0, `case references missing: ${c.id}`);
      for (const ref of c.requirements ?? []) check(reqIds.has(ref), `unresolved requirement: ${c.id} -> ${ref}`);
      check(object(c.input) && (typeof c.input.source === 'string' || typeof c.input.harness === 'string'
        || (Array.isArray(c.input.submissions) && c.input.submissions.length > 0)), `case input: ${c.id}`);
      check(object(c.expected) && typeof c.expected.stage === 'string' && typeof c.expected.outcome === 'string', `case expected: ${c.id}`);
      requireEvidence(c.state, c.evidence, c.id, { review: c.kind === 'review' });
      if (['static', 'admission'].includes(c.kind)) {
        check(c.expected?.guest_requests === 0 && c.expected?.protected_operations === 0, `rejection trace: ${c.id}`);
      }
      checkGreenfield(c.state, c.id, { compiler: c.kind !== 'review', runtime: c.kind === 'runtime' });
    }
    const runtimeComponents = new Set(['wasm-runtime', 'component-adapters', 'syndicate-profile', 'exact-calculator']);
    for (const component of status.components) {
      requireEvidence(component, component.evidence, component.id);
      checkGreenfield(component, component.id, { runtime: runtimeComponents.has(component.id) });
    }
    const get = id => cases.find(c => c.id === id);
    validateWitEffectOracle(get, check);
    const localBinding = get('DX-07');
    check(localBinding?.input?.stack_only_source === 'dup 10 +'
      && localBinding.input.binding_form === 'structured lexical binding of subtotal; return subtotal and wrap64(subtotal + 10)',
    'local binding oracle: selected arithmetic');
    const i64Stack = (stack, values) => Array.isArray(stack) && stack.length === values.length
      && stack.every((value, index) => value?.type === 'I64' && value.value === values[index]);
    check(i64Stack(localBinding?.input?.initial_stack, ['100'])
      && i64Stack(localBinding?.expected?.pure_output, ['100', '110'])
      && sameStrings(localBinding?.expected?.pure_effects, []) && localBinding?.expected?.new_kernel_operations === 0,
    'local binding oracle: expected stack');
    const effect = get('S-CASE-05');
    check(effect?.kind === 'static' && effect.expected?.outcome === 'effect-reject'
      && effect.expected?.guest_requests === 0, 'effect oracle: S-CASE-05 must reject before a request');
    check(get('S-CASE-16')?.kind === 'admission', 'effect oracle: forged-manifest admission case missing');
    for (const id of ['S-CASE-06', 'WI-08']) {
      const c = get(id);
      check(c?.expected?.guest_requests === 1 && c.expected.protected_operations === 0
        && c.input?.declared_effects?.length > 0, `authority oracle: ${id}`);
    }
    check(get('ID-03')?.expected?.definition === 'different' && get('ID-03')?.expected?.program_value === 'different', 'identity oracle: versioned operation must change semantic identity');
    check(get('ID-04')?.expected?.definition === 'same' && get('ID-04')?.expected?.build_key === 'different', 'identity oracle: adapter-only change');
    for (const id of ['CORE-03', 'CORE-05', 'CORE-09', 'ADAPT-09', 'ADAPT-10']) {
      check(get(id)?.input?.compiler_service === 'disabled' && get(id)?.expected?.candidate_prepare_requests === 0,
        `runtime builder boundary: ${id}`);
    }
    // Discriminating document oracles, not implementations of the named harnesses.
    check(get('ADAPT-01')?.expected?.implicit_union_join === false
      && get('ADAPT-01')?.expected?.outcome === 'reject-each-variant', 'branch join oracle');
    check(get('ADAPT-05')?.expected?.immutable_marker_freezes_external_storage === false
      && get('ADAPT-05')?.expected?.use_after_invalidation === false, 'buffer lifetime oracle');
    check(get('ADAPT-06')?.expected?.outcome === 'reject'
      && get('ADAPT-06')?.expected?.accepted_program_created === false, 'decoded-candidate oracle');
    check(get('ADAPT-08')?.expected?.outcome === 'reject'
      && get('ADAPT-08')?.expected?.artifact_emitted === false, 'adapter arity oracle');
    check(get('ADAPT-09')?.expected?.new_wasm_types_per_tree === 0
      && get('ADAPT-09')?.expected?.quota_is_success === false, 'dynamic composition oracle');
    check(get('ADAPT-10')?.input?.debug_metadata === 'stripped'
      && get('ADAPT-10')?.expected?.reflection_uses_addresses === false, 'retained recipe oracle');
    check(get('RA-CASE-05')?.input?.gc_cycle_after_guest_revocation === true
      && get('RA-CASE-05')?.expected?.native_pins === 1
      && get('RA-CASE-05')?.expected?.final_state === 'Retiring(call-1)', 'GC ownership oracle');
    check(get('ADAPT-11')?.expected?.internal_gc_implies_zero_copy === false
      && get('ADAPT-11')?.expected?.partial_trusted_values_published === 0
      && get('ADAPT-11')?.expected?.m3_probe_closes_m5 === false, 'component conversion oracle');
    check(get('ADAPT-12')?.expected?.two_production_backends_required === false
      && get('ADAPT-12')?.expected?.backend_selected_by_fixture === false, 'backend selection oracle');
    check(get('ADAPT-13')?.expected?.wasm_correspondence_proven === false
      && get('ADAPT-13')?.expected?.all_targets_covered === false, 'native assurance oracle');
    const snapshot = get('ADAPT-14');
    const scalars = ['before', 'amount', 'after'].map(key => snapshot?.input?.[key]);
    const validScalars = scalars.every(v => v?.type === 'I64' && typeof v.value === 'string' && /^-?\d+$/.test(v.value));
    check(validScalars, 'contract snapshot oracle: typed integer operands');
    if (validScalars) {
      const [before, amount, after] = scalars.map(v => BigInt(v.value));
      const max = (1n << 63n) - 1n;
      check([before, amount, after].every(v => v >= 0n && v <= max) && before + amount <= max
        && snapshot.input.snapshot_condition === 'after = before + amount'
        && snapshot.input.incorrect_condition === 'after = after + amount'
        && after === before + amount && after !== after + amount
        && snapshot.expected.snapshot_condition === true && snapshot.expected.incorrect_condition === false
        && snapshot.expected.universal_proof_claimed === false, 'contract snapshot oracle');
    }
    const modes = get('ADAPT-15')?.input?.variants ?? [];
    const modeKeys = modes.map(v => `${v.build}:${v.optional_proof}`).sort();
    check(JSON.stringify(modeKeys) === JSON.stringify(['debug:absent', 'debug:unrelated-valid-proof', 'release:absent', 'release:unrelated-valid-proof'])
      && modes.every(v => v.outcome === 'reject') && get('ADAPT-15')?.expected?.accepted_program_created === false,
    'mandatory admission oracle: complete rejecting build/proof matrix');
    // These are assurance-harness specifications, not executed extraction or proof results.
    check(get('VERIFY-01')?.input?.actual_rust_extracted === true
      && get('VERIFY-01')?.expected?.outcome === 'accept-named-scope-only'
      && ['whole_kernel_verified', 'whole_project_verified', 'wasm_preservation_proved']
        .every(key => get('VERIFY-01')?.expected?.[key] === false), 'Aeneas narrow-scope oracle');
    check(sameStrings(get('VERIFY-02')?.input?.variants, ['frontend-parser-body', 'backend-emitter-body',
      'runtime-transition', 'adapter-wrapper', 'cli-decision', 'macro-generated-body', 'feature-target-body'])
      && get('VERIFY-02')?.expected?.outcome === 'reject-each-variant'
      && get('VERIFY-02')?.expected?.missing_body_ignored === false, 'Aeneas source-coverage oracle');
    check(sameStrings(get('VERIFY-03')?.input?.variants, ['unsupported-Rust-construct', 'unsafe-body',
      'move-body-to-adapter-crate', 'replace-Aeneas-with-Verus', 'desired-result-axiom'])
      && get('VERIFY-03')?.expected?.outcome === 'reject-each-variant'
      && get('VERIFY-03')?.expected?.kernel_exception_accepted === false
      && get('VERIFY-03')?.expected?.moving_crate_changes_semantic_owner === false, 'Aeneas kernel-exception oracle');
    check(sameStrings(get('VERIFY-04')?.input?.required_record_fields, ['symbols', 'source_configuration', 'owner',
      'constraint_or_diagnostic', 'contract', 'assumptions', 'evidence', 'reassessment_condition'])
      && sameStrings(get('VERIFY-04')?.input?.variants, ['complete-reviewed-record', 'missing-owner',
        'missing-contract', 'whole-crate-scope', 'unreviewed-record'])
      && get('VERIFY-04')?.expected?.outcome === 'accept-complete-boundary-record-reject-other-variants'
      && ['aeneas_verified_body', 'physical_release_proved_by_plan', 'synchronization_proved_by_sequential_transition',
        'correspondence_obligation_closed_by_exception'].every(key => get('VERIFY-04')?.expected?.[key] === false),
    'Aeneas external-boundary oracle');
    check(sameStrings(get('VERIFY-05')?.input?.variants, ['proof-not-run', 'proof-failed', 'proof-timeout',
      'proof-unsupported', 'sorryAx', 'unexplained-external-model', 'stale-source-configuration',
      'hand-edited-generated-function', 'proof-of-separate-reference-only'])
      && get('VERIFY-05')?.input?.extraction === 'passed'
      && get('VERIFY-05')?.expected?.outcome === 'reject-each-variant'
      && get('VERIFY-05')?.expected?.extraction_implies_refinement === false, 'Aeneas incomplete-proof oracle');
    validateContractOracles(get, check);
    validateCalculatorOracles(get, check);
    validateWorkerOracles(get, check);
    validateOctetAdoptionOracles(get, check);
    const core = bundle.texts.get(CORE_SPEC) ?? '';
    check(core.includes('`#` begins a comment') && core.includes('**K-SYN-05.**'), 'surface: canonical lexer rule missing');
    check(!core.includes('[ drop call ]'), 'surface: legacy list example');
    check(!core.includes('No async ABI or stream surface API is frozen here.'), 'async: stale blanket ABI statement');
    for (const [file, text] of bundle.texts) {
      if (!checkedMarkdown(file)) continue;
      for (const match of text.matchAll(/\[[^\]\n]*\]\(([^)\s]+)\)/g)) {
        const target = match[1];
        if (/^https?:\/\//.test(target) || target.startsWith('#')) continue;
        const clean = decodeURIComponent(target.split('#')[0]);
        const resolved = path.posix.normalize(path.posix.join(path.posix.dirname(file), clean));
        check(!path.posix.isAbsolute(clean) && !resolved.startsWith('../') && bundle.paths.has(resolved), `broken local link: ${file} -> ${target}`);
      }
    }
    const obligations = parse(bundle, 'specs/verification/obligations.json');
    check(obligations.revision === REVISION, 'revision: obligation ledger mismatch');
    const expectedObligations = [...(bundle.texts.get(VERIFICATION_SPEC) ?? '').matchAll(/^\| ((?:PO|SO)-\d+) \|/gm)].map(m => m[1]).sort();
    check(JSON.stringify(obligations.obligations.map(o => o.id).sort()) === JSON.stringify(expectedObligations), 'obligation ledger: IDs do not match PO/SO tables');
    for (const obligation of obligations.obligations) {
      check(['open', 'accepted', 'failed'].includes(obligation.status), `obligation status: ${obligation.id}`);
      check(typeof obligation.claim === 'string' && typeof obligation.route === 'string' && Array.isArray(obligation.evidence), `obligation schema: ${obligation.id}`);
      // Project the proof ledger into the same evidence policy as scenarios and components.
      // These neutral fields do not assert implementation, execution, or assessed trust.
      requireEvidence({ implementation: 'absent', execution: 'not-run', proof: obligation.status,
        trust: 'unassessed' }, obligation.evidence, obligation.id);
      if (!status.proof_implementation_exists) check(obligation.status !== 'accepted', `greenfield obligation claim: ${obligation.id}`);
    }
    for (const [id, route] of Object.entries({ ...AENEAS_ROUTES, ...CONTRACT_ROUTES })) {
      check(obligations.obligations.find(o => o.id === id)?.route === route, `Aeneas obligation route: ${id}`);
    }
    const nodes = new Map(roadmap.milestones.map(m => [m.id, m]));
    check(nodes.size === roadmap.milestones.length, 'roadmap duplicate milestone');
    for (const node of nodes.values()) {
      check(Array.isArray(node.required_requirements ?? []), `roadmap requirement array: ${node.id}`);
      for (const ref of node.required_requirements ?? []) check(reqIds.has(ref), `roadmap requirement: ${node.id} -> ${ref}`);
    }
    for (const ref of ['VT-NATIVE-01', 'VT-NATIVE-02', 'VT-NATIVE-03']) {
      check(nodes.get('M1')?.required_requirements?.includes(ref), `native workspace gate missing: ${ref}`);
    }
    for (const [id, refs] of Object.entries({
      M1: ['VT-SCOPE-01', 'VT-SCOPE-02', 'VT-SCOPE-03', 'VT-SCOPE-04', 'VT-SCOPE-05', 'VT-CI-05', 'VT-PIN-01', 'VT-PIN-02'],
      M2: ['B-IMPL-02', 'VT-AENEAS-01', 'VT-AENEAS-04', 'VT-SCOPE-05', 'VT-CI-05'],
      M5: ['VT-SCOPE-02', 'VT-SCOPE-03'],
    })) {
      for (const ref of refs) check(nodes.get(id)?.required_requirements?.includes(ref), `Aeneas milestone gate: ${id} -> ${ref}`);
    }
    for (const [id, deps, refs] of [
      ['MC1', ['M2'], ['VC-SCOPE-02', 'VC-INPUT-01', 'VC-INPUT-02', 'VC-INPUT-03', 'VC-INPUT-04',
        'VC-LIB-01', 'VC-LIB-02', 'VC-GATE-01', 'VT-CONTRACT-01']],
      ['MC2', ['MC1', 'M4'], ['VC-SCOPE-03', 'VC-VALUE-01', 'VC-VALUE-02', 'VC-VALUE-03', 'VC-VALUE-04',
        'VC-VALUE-05', 'VC-USE-01', 'VC-LIB-03', 'VC-LIB-04', 'VC-TOOL-01', 'VC-TOOL-02', 'VC-TOOL-03',
        'VC-TOOL-04', 'VC-GATE-02', 'VC-GATE-03', 'VT-CONTRACT-02']],
    ]) {
      check(sameStrings(nodes.get(id)?.depends_on, deps) && !(nodes.get(id)?.conditional_dependencies?.length),
        `contract milestone dependencies: ${id}`);
      for (const ref of refs) check(nodes.get(id)?.required_requirements?.includes(ref), `contract milestone gate: ${id} -> ${ref}`);
    }
    check(sameStrings(nodes.get('M4')?.depends_on, ['M2', 'M3'])
      && !(nodes.get('M4')?.conditional_dependencies?.length), 'contract profile must not block ordinary core');
    for (const [id, dependencies, gates, refs] of [
      ['MA1', ['M4'], ['declarations-and-modules', 'iteration-or-recursion', 'text-processing', 'error-schemas', 'budget-accounting'],
        ['CALC-SCOPE-02', 'CALC-NUM-01', 'CALC-NUM-02', 'CALC-NUM-03', 'CALC-NUM-04', 'CALC-NUM-05', 'CALC-NUM-06', 'CALC-LIMIT-01', 'CALC-GATE-01']],
      ['MA2', ['MA1'], ['definition-command-grammar', 'expression-and-session-interfaces'],
        ['CALC-SCOPE-01', 'CALC-PARSE-01', 'CALC-PARSE-02', 'CALC-STATE-01', 'CALC-STATE-02', 'CALC-BOUND-01', 'CALC-DISPLAY-01', 'CALC-APPROX-01', 'CALC-EVIDENCE-01', 'CALC-EVIDENCE-02']],
      ['MA3', ['MA2'], ['authoring-tool-schema', 'independent-benchmark-harness', 'route-specific-language-support'],
        ['CALC-AI-01', 'CALC-AI-02', 'CALC-AI-03', 'CALC-AI-04', 'CALC-EVIDENCE-01']],
    ]) {
      check(sameStrings(nodes.get(id)?.depends_on, dependencies) && !(nodes.get(id)?.conditional_dependencies?.length),
        `calculator milestone dependencies: ${id}`);
      check(sameStrings(nodes.get(id)?.entry_gates, gates), `calculator entry gates: ${id}`);
      for (const ref of refs) check(nodes.get(id)?.required_requirements?.includes(ref), `calculator milestone gate: ${id} -> ${ref}`);
    }
    for (const [id, gate] of Object.entries(WORKER_GATES)) {
      check(sameStrings(nodes.get(id)?.depends_on, gate.depends_on) && !(nodes.get(id)?.conditional_dependencies?.length),
        `worker milestone dependencies: ${id}`);
      check(sameStrings(nodes.get(id)?.entry_gates, gate.entry_gates), `worker milestone entry gates: ${id}`);
      for (const ref of gate.required_requirements) {
        check(nodes.get(id)?.required_requirements?.includes(ref), `worker milestone requirement: ${id} -> ${ref}`);
      }
    }
    check(nodes.get('M2')?.required_requirements?.includes('B-CHECK-07'), 'worker admission limit gate: M2');
    for (const ref of ['RA-ASYNC-01', 'RA-ASYNC-02', 'RA-ASYNC-03', 'RA-ASYNC-04', 'RA-ASYNC-05']) {
      check(nodes.get('M6')?.required_requirements?.includes(ref), `worker async ownership gate: ${ref}`);
    }
    for (const [id, refs] of Object.entries(OCTET_MILESTONE_REQUIREMENTS)) {
      for (const ref of refs) check(nodes.get(id)?.required_requirements?.includes(ref), `Octet adoption milestone gate: ${id} -> ${ref}`);
    }
    const comparison = nodes.get('M3')?.comparison;
    check(object(comparison), 'backend comparison missing');
    if (object(comparison)) {
      check(JSON.stringify([...(comparison.candidates ?? [])].sort()) === JSON.stringify(['managed-linear-memory', 'wasm-gc']), 'backend comparison candidates');
      check(comparison.component_probe_blocks_m3 === false, 'backend comparison: component probe must not block M3');
      const selected = comparison.selected_backend !== null;
      check(!selected || comparison.candidates?.includes(comparison.selected_backend), 'backend comparison selection');
      check(!selected || (status.compiler_exists && status.runtime_exists && nodes.get('M3').status !== 'not-started'), 'backend selection before execution');
      check(!selected || (Array.isArray(comparison.evidence) && comparison.evidence.some(e => e.kind === 'test' && e.result === 'passed')),
        'backend selection requires an executed comparison');
      requireEvidence({ implementation: selected ? 'implemented' : 'absent', execution: selected ? 'passed' : 'not-run',
        proof: 'not-applicable', trust: 'unassessed' }, comparison.evidence, 'M3-backend-comparison');
    }
    for (const ref of ['BE-COMPARE-01', 'BE-CONFIG-01', 'BE-DYNAMIC-01', 'BE-ARITY-01', 'BE-REFLECT-01', 'BE-LIMIT-01', 'BE-REPORT-01']) {
      check(nodes.get('M3')?.required_requirements?.includes(ref), `backend requirement gate missing: ${ref}`);
    }
    const active = new Set(), done = new Set();
    function walk(id) {
      if (active.has(id)) { errors.push(`roadmap cycle: ${id}`); return; }
      if (done.has(id)) return;
      const node = nodes.get(id);
      if (!node) { errors.push(`roadmap missing dependency: ${id}`); return; }
      active.add(id);
      for (const next of [...node.depends_on, ...(node.conditional_dependencies ?? []).map(d => d.id)]) walk(next);
      active.delete(id);
      done.add(id);
    }
    for (const id of nodes.keys()) walk(id);
    check(nodes.has(status.next_milestone), 'roadmap next milestone missing');
    if (!ignoreLedger) {
      check(JSON.stringify(parse(bundle, 'specs/requirements.json')) === JSON.stringify(deriveLedger(bundle)), 'requirement ledger stale or incomplete; run --refresh-ledger after intentional changes');
    }
    return { errors, summary: { documents: family.normative_documents.length, requirements: reqIds.size, scenarios: cases.length,
      scenarios_executed: cases.filter(c => EXECUTED.includes(c.state.execution)).length,
      open_test_design: deriveLedger(bundle).requirements.filter(r => r.test_design === 'test-design-open').length,
      proof_obligations: obligations.obligations.length, completed_proofs: obligations.obligations.filter(o => o.status === 'accepted').length } };
  } catch (error) {
    errors.push(`schema/read failure: ${error.message}`);
    return { errors, summary: null };
  }
}

function clone(bundle) { return { texts: new Map(bundle.texts), paths: new Set(bundle.paths) }; }
function changeJson(bundle, file, change) {
  const value = parse(bundle, file);
  change(value);
  bundle.texts.set(file, JSON.stringify(value));
}
function refreshed(bundle) {
  bundle.texts.set('specs/requirements.json', JSON.stringify(deriveLedger(bundle)));
  bundle.paths.add('specs/requirements.json');
  return bundle;
}

function selfTest(base) {
  const results = [];
  const run = (name, mutate, expectedError) => {
    const sample = clone(base);
    mutate(sample);
    const result = validate(sample);
    const passed = expectedError ? result.errors.some(e => e.includes(expectedError)) : result.errors.length === 0;
    if (!passed) throw new Error(`self-test ${name}: ${JSON.stringify(result.errors)}`);
    results.push({ name, expected: expectedError ? 'reject' : 'accept', result: 'passed' });
  };
  run('valid-family', () => {}, null);
  run('missing-native-spec', b => b.texts.delete(CORE_SPEC), 'revision:');
  run('broken-native-link', b => b.texts.set(CORE_SPEC, b.texts.get(CORE_SPEC) + '\n[missing](absent.md)\n'), 'broken local link');
  run('legacy-authority-regression', b => changeJson(b, 'specs/spec-family.json', p => p.normative_documents[0].path = 'SPEC-0001.md'), 'Cairn authority');
  run('missing-native-lifecycle', b => changeJson(b, 'specs/spec-family.json', p => delete p.lifecycle), 'Cairn authority');
  run('malformed-json', b => b.texts.set('specs/STATUS.json', '{'), 'schema/read failure');
  run('missing-state', b => changeJson(b, 'specs/conformance/cases.json', p => delete p.cases[0].state.execution), 'state execution');
  run('unknown-requirement', b => changeJson(b, 'specs/conformance/cases.json', p => p.cases[0].requirements.push('K-MISSING-99')), 'unresolved requirement');
  run('duplicate-requirement', b => b.texts.set(CORE_SPEC, b.texts.get(CORE_SPEC) + '\n### Requirement: K-SYN-01\nr[K-SYN-01]\n\nDuplicate MUST fail.\n'), 'duplicate requirement');
  run('dropped-inherited-requirement', b => b.texts.set(SAFETY_SPEC,
    b.texts.get(SAFETY_SPEC).replaceAll('S-LANG-01', 'S-REMOVED-01')), 'dropped requirement');
  run('broken-link', b => b.texts.set('specs/README.md', b.texts.get('specs/README.md') + '\n[missing](absent.md)\n'), 'broken local link');
  run('unsafe-effect-oracle', b => changeJson(b, 'specs/conformance/safety-cases.json', p => {
    const c = p.cases.find(c => c.id === 'S-CASE-05'); c.expected.outcome = 'deny-or-static-reject'; c.expected.guest_requests = 1;
  }), 'effect oracle');
  run('WIT-purity-does-not-erase-request', b => changeJson(b, 'specs/conformance/wit-wasi-cases.json', p => {
    p.cases.find(c => c.id === 'WI-07').input.variants.find(v => v.reviewed_pure_contract && !v.claimed_effects.length).outcome = 'accept';
  }), 'WIT effect oracle');
  run('WIT-effect-matrix-must-be-complete', b => changeJson(b, 'specs/conformance/wit-wasi-cases.json', p => {
    p.cases.find(c => c.id === 'WI-07').input.variants.pop();
  }), 'WIT effect oracle');
  run('identity-version-regression', b => changeJson(b, 'specs/conformance/identity-cases.json', p => p.cases.find(c => c.id === 'ID-03').expected.definition = 'same'), 'identity oracle');
  run('legacy-list-example', b => b.texts.set(CORE_SPEC, b.texts.get(CORE_SPEC).replace('[ drop run ]', '[ drop call ]')), 'surface: legacy');
  run('greenfield-false-pass', b => {
    changeJson(b, 'specs/STATUS.json', p => p.compiler_exists = false);
    changeJson(b, 'specs/conformance/cases.json', p => p.cases[0].state.execution = 'passed');
  }, `greenfield execution claim: ${parse(base, 'specs/conformance/cases.json').cases[0].id}`);
  // The fixture pins the greenfield state itself (flag false + accepted
  // obligation) instead of depending on the bundle's current STATUS flags,
  // so the control keeps its meaning once the proof implementation exists.
  run('accepted-proof-without-source', b => {
    changeJson(b, 'specs/STATUS.json', p => p.proof_implementation_exists = false);
    changeJson(b, 'specs/verification/obligations.json', p => p.obligations[0].status = 'accepted');
  }, 'greenfield obligation');
  run('roadmap-cycle', b => changeJson(b, 'specs/roadmap.json', p => p.milestones[0].depends_on.push('M8')), 'roadmap cycle');
  run('missing-milestone', b => changeJson(b, 'specs/roadmap.json', p => p.milestones[1].depends_on.push('M99')), 'roadmap missing dependency');
  run('missing-ledger-row', b => changeJson(b, 'specs/requirements.json', p => p.requirements.pop()), 'requirement ledger');
  run('removed-normative-document', b => changeJson(b, 'specs/spec-family.json', p => p.normative_documents = p.normative_documents.filter(d => d.id !== 'SPEC-V002')), 'required document missing');
  run('duplicate-case', b => changeJson(b, 'specs/conformance/cases.json', p => p.cases.push(p.cases[0])), 'duplicate or missing case');
  run('unsupported-is-not-executed', b => changeJson(b, 'specs/conformance/cases.json', p => {
    p.cases[0].state.implementation = 'unsupported'; p.cases[0].state.execution = 'unsupported';
  }), null);
  run('added-unexecuted-scenario', b => {
    changeJson(b, 'specs/conformance/cases.json', p => p.cases.push({ ...p.cases[0], id: 'CORE-EXTRA' })); refreshed(b);
  }, null);
  run('implemented-failed-open-is-valid', b => {
    changeJson(b, 'specs/STATUS.json', p => { p.compiler_exists = true; p.runtime_exists = true; });
    changeJson(b, 'specs/conformance/cases.json', p => {
      const c = p.cases[0]; c.state.implementation = 'implemented'; c.state.execution = 'failed';
      c.evidence.push({ kind: 'test', subject: c.id, claim: `Declared checks for ${c.id}`, revision: REVISION, source_revision: 'synthetic-self-test-only', configuration: { toolchain: 'synthetic' }, result: 'failed', assumptions: [] });
    });
  }, null);
  run('mismatched-execution-evidence', b => {
    changeJson(b, 'specs/STATUS.json', p => { p.compiler_exists = true; p.runtime_exists = true; });
    changeJson(b, 'specs/conformance/cases.json', p => {
      const c = p.cases[0]; c.state.implementation = 'implemented'; c.state.execution = 'passed';
      c.evidence.push({ kind: 'test', subject: c.id, claim: `Declared checks for ${c.id}`, revision: REVISION, source_revision: 'synthetic', configuration: { toolchain: 'synthetic' }, result: 'failed', assumptions: [] });
    });
  }, 'execution evidence mismatch');
  const componentEvidence = (id, kind, result) => ({ kind, subject: id, claim: `Declared checks for ${id}`, revision: REVISION,
    source_revision: 'synthetic-self-test-only', configuration: { toolchain: 'synthetic' }, result, assumptions: [] });
  for (const execution of EXECUTED) {
    run(`component-runtime-${execution}-without-runtime`, b => changeJson(b, 'specs/STATUS.json', p => {
      p.compiler_exists = true; p.runtime_exists = false;
      const c = p.components.find(c => c.id === 'wasm-runtime');
      c.implementation = 'implemented'; c.execution = execution;
      c.evidence = [componentEvidence(c.id, 'test', execution)];
    }), 'absent runtime execution claim: wasm-runtime');
  }
  run('component-implemented-without-compiler', b => changeJson(b, 'specs/STATUS.json', p => {
    p.compiler_exists = false;
    p.components.find(c => c.id === 'core-checker').implementation = 'implemented';
  }), 'greenfield implementation claim: core-checker');
  run('component-executed-without-compiler', b => changeJson(b, 'specs/STATUS.json', p => {
    p.compiler_exists = false;
    const c = p.components.find(c => c.id === 'core-checker');
    c.execution = 'passed'; c.evidence = [componentEvidence(c.id, 'test', 'passed')];
  }), 'greenfield execution claim: core-checker');
  run('component-proof-without-proof-implementation', b => changeJson(b, 'specs/STATUS.json', p => {
    p.proof_implementation_exists = false;  // pinned by the fixture, not by the bundle
    const c = p.components.find(c => c.id === 'core-checker');
    c.proof = 'accepted'; c.evidence = [componentEvidence(c.id, 'lean-kernel', 'accepted')];
  }), 'greenfield proof claim: core-checker');
  run('component-implemented-failed-open-is-valid', b => changeJson(b, 'specs/STATUS.json', p => {
    p.compiler_exists = true; p.runtime_exists = true;
    const c = p.components.find(c => c.id === 'wasm-runtime');
    c.implementation = 'implemented'; c.execution = 'failed';
    c.evidence = [componentEvidence(c.id, 'test', 'failed')];
  }), null);
  run('component-checker-test-without-runtime-is-valid', b => {
    changeJson(b, 'specs/STATUS.json', p => {
      p.compiler_exists = true; p.runtime_exists = false;
      for (const component of p.components) component.execution = 'not-run';
      const c = p.components.find(c => c.id === 'core-checker');
      c.implementation = 'implemented'; c.execution = 'passed';
      c.evidence = [componentEvidence(c.id, 'test', 'passed')];
    });
    for (const file of parse(b, 'specs/spec-family.json').scenario_files) {
      changeJson(b, `specs/${file}`, p => {
        for (const c of p.cases) if (c.kind === 'runtime') c.state.execution = 'not-run';
      });
    }
    changeJson(b, 'specs/roadmap.json', p => {
      const m = p.milestones.find(m => m.id === 'M3');
      m.status = 'not-started'; m.comparison.selected_backend = null; m.comparison.evidence = [];
    });
  }, null);
  run('component-proof-with-implementation-is-valid', b => changeJson(b, 'specs/STATUS.json', p => {
    p.proof_implementation_exists = true;
    const c = p.components.find(c => c.id === 'core-checker');
    c.execution = 'not-run';
    c.proof = 'accepted'; c.evidence = [componentEvidence(c.id, 'lean-kernel', 'accepted')];
  }), null);
  run('local-binding-undefined-division', b => changeJson(b, 'specs/conformance/language-workflow-cases.json', p => {
    p.cases.find(c => c.id === 'DX-07').input.stack_only_source = 'dup dup 10 / +';
  }), 'local binding oracle: selected arithmetic');
  run('local-binding-wrong-output', b => changeJson(b, 'specs/conformance/language-workflow-cases.json', p => {
    p.cases.find(c => c.id === 'DX-07').expected.pure_output[1].value = '111';
  }), 'local binding oracle: expected stack');
  const adaptation = (b, id, change) => changeJson(b, 'specs/conformance/adaptation-cases.json', p => change(p.cases.find(c => c.id === id)));
  run('removed-backend-document', b => changeJson(b, 'specs/spec-family.json', p => p.normative_documents = p.normative_documents.filter(d => d.id !== 'SPEC-BE001')), 'required document missing');
  run('stale-roadmap-revision', b => changeJson(b, 'specs/roadmap.json', p => p.revision = 'obsolete'), 'revision: roadmap');
  run('stale-obligation-revision', b => changeJson(b, 'specs/verification/obligations.json', p => p.revision = 'obsolete'), 'revision: obligation');
  run('missing-native-gate', b => changeJson(b, 'specs/roadmap.json', p => {
    const m = p.milestones.find(m => m.id === 'M1'); m.required_requirements = m.required_requirements.filter(r => r !== 'VT-NATIVE-03');
  }), 'native workspace gate missing');
  run('missing-backend-candidate', b => changeJson(b, 'specs/roadmap.json', p => p.milestones.find(m => m.id === 'M3').comparison.candidates.pop()), 'backend comparison candidates');
  run('premature-backend-selection', b => changeJson(b, 'specs/roadmap.json', p => {
    const m = p.milestones.find(m => m.id === 'M3');
    m.status = 'not-started'; m.comparison.selected_backend = 'wasm-gc';
  }), 'backend selection before execution');
  run('review-only-backend-selection', b => {
    changeJson(b, 'specs/STATUS.json', p => { p.compiler_exists = true; p.runtime_exists = true; });
    changeJson(b, 'specs/roadmap.json', p => {
      const m = p.milestones.find(m => m.id === 'M3'); m.status = 'completed'; m.comparison.selected_backend = 'wasm-gc';
      m.comparison.evidence = [{ kind: 'review', subject: 'M3-backend-comparison', claim: 'M3 comparison gates', revision: REVISION,
        source_revision: 'synthetic-self-test-only', configuration: { toolchain: 'synthetic' }, result: 'passed', assumptions: [] }];
    });
  }, 'backend selection requires an executed comparison');
  run('implicit-union-join', b => adaptation(b, 'ADAPT-01', c => c.expected.implicit_union_join = true), 'branch join oracle');
  run('marker-does-not-freeze-storage', b => adaptation(b, 'ADAPT-05', c => c.expected.immutable_marker_freezes_external_storage = true), 'buffer lifetime oracle');
  run('layout-does-not-grant-acceptance', b => adaptation(b, 'ADAPT-06', c => c.expected.accepted_program_created = true), 'decoded-candidate oracle');
  run('hidden-argument-dropping', b => adaptation(b, 'ADAPT-08', c => c.expected.artifact_emitted = true), 'adapter arity oracle');
  run('per-tree-wasm-type', b => adaptation(b, 'ADAPT-09', c => c.expected.new_wasm_types_per_tree = 1), 'dynamic composition oracle');
  run('reflection-as-address', b => adaptation(b, 'ADAPT-10', c => c.expected.reflection_uses_addresses = true), 'retained recipe oracle');
  run('GC-premature-pin-release', b => changeJson(b, 'specs/conformance/resource-cases.json', p => p.cases.find(c => c.id === 'RA-CASE-05').expected.native_pins = 0), 'GC ownership oracle');
  run('false-zero-copy', b => adaptation(b, 'ADAPT-11', c => c.expected.internal_gc_implies_zero_copy = true), 'component conversion oracle');
  run('wrong-postcondition', b => adaptation(b, 'ADAPT-14', c => c.expected.incorrect_condition = true), 'contract snapshot oracle');
  run('disabled-release-guard', b => adaptation(b, 'ADAPT-15', c => c.input.variants[1].outcome = 'accept'), 'mandatory admission oracle');
  run('missing-guard-matrix-cell', b => adaptation(b, 'ADAPT-15', c => c.input.variants.pop()), 'mandatory admission oracle');
  run('template-selects-dependency', b => changeJson(b, 'specs/verification/toolchain-lock.template.json', p => p.optional_byte_view_dependency = 'zerocopy'), 'unselected template');
  const verification = (b, id, change) => changeJson(b, 'specs/conformance/verification-cases.json', p => change(p.cases.find(c => c.id === id)));
  run('Aeneas-policy-missing', b => changeJson(b, 'specs/STATUS.json', p => delete p.verification_policy), 'Aeneas policy');
  run('Aeneas-checker-only-regression', b => changeJson(b, 'specs/STATUS.json', p => p.verification_policy.scope = 'checker-only'), 'Aeneas policy: scope');
  run('Aeneas-kernel-optional-regression', b => changeJson(b, 'specs/STATUS.json', p => p.verification_policy.kernel_route = 'preferred'), 'Aeneas policy: kernel_route');
  run('Aeneas-false-coverage-inventory', b => changeJson(b, 'specs/STATUS.json', p => {
    p.compiler_exists = false;
    p.runtime_exists = false;
    p.verification_policy.coverage_inventory = 'complete';
  }), 'Aeneas policy: false greenfield');
  run('Aeneas-mandatory-Verus-regression', b => changeJson(b, 'specs/verification/toolchain-lock.template.json', p => p.proof.verus = null), 'Aeneas template');
  run('Aeneas-resource-route-regression', b => changeJson(b, 'specs/verification/obligations.json', p => p.obligations.find(o => o.id === 'PO-13').route = 'verus'), 'Aeneas obligation route');
  run('Aeneas-missing-extraction-gate', b => changeJson(b, 'specs/roadmap.json', p => {
    const m = p.milestones.find(m => m.id === 'M2'); m.required_requirements = m.required_requirements.filter(r => r !== 'VT-CI-05');
  }), 'Aeneas milestone gate');
  run('Aeneas-narrow-proof-overclaim', b => verification(b, 'VERIFY-01', c => c.expected.whole_kernel_verified = true), 'Aeneas narrow-scope oracle');
  run('Aeneas-frontend-omission', b => verification(b, 'VERIFY-02', c => c.input.variants.shift()), 'Aeneas source-coverage oracle');
  run('Aeneas-kernel-exception', b => verification(b, 'VERIFY-03', c => c.expected.kernel_exception_accepted = true), 'Aeneas kernel-exception oracle');
  run('Aeneas-plan-is-not-execution', b => verification(b, 'VERIFY-04', c => c.expected.physical_release_proved_by_plan = true), 'Aeneas external-boundary oracle');
  run('Aeneas-exception-missing-owner', b => verification(b, 'VERIFY-04', c => c.input.required_record_fields = c.input.required_record_fields.filter(f => f !== 'owner')), 'Aeneas external-boundary oracle');
  run('Aeneas-extraction-is-not-proof', b => verification(b, 'VERIFY-05', c => c.expected.extraction_implies_refinement = true), 'Aeneas incomplete-proof oracle');
  run('Aeneas-unsupported-proof-omission', b => verification(b, 'VERIFY-05', c => c.input.variants = c.input.variants.filter(v => v !== 'proof-unsupported')), 'Aeneas incomplete-proof oracle');
  const contract = (b, id, change) => changeJson(b, 'specs/conformance/contract-cases.json', p => change(p.cases.find(c => c.id === id)));
  run('contracts-host-only-regression', b => changeJson(b, 'specs/STATUS.json', p => p.program_contracts.companions = 'host-only'), 'contract policy: companions');
  run('contracts-mandatory-proof-regression', b => changeJson(b, 'specs/STATUS.json', p => p.program_contracts.ordinary_programs_require_proofs = true), 'contract policy: ordinary_programs');
  run('contracts-missing-outcome', b => changeJson(b, 'specs/STATUS.json', p => p.program_contracts.claim_outcomes.pop()), 'contract policy: claim outcomes');
  run('contracts-resource-dependency', b => changeJson(b, 'specs/roadmap.json', p => p.milestones.find(m => m.id === 'MC1').depends_on.push('M5')), 'contract milestone dependencies');
  run('contracts-missing-export-gate', b => changeJson(b, 'specs/roadmap.json', p => {
    const m = p.milestones.find(m => m.id === 'MC1'); m.required_requirements = m.required_requirements.filter(r => r !== 'VC-INPUT-03');
  }), 'contract milestone gate');
  run('contracts-unrelated-proof', b => contract(b, 'CONTRACT-01', c => c.input.subject = 'handwritten-reference-only'), 'contract oracle: exact increment');
  run('contracts-incorrect-overflow', b => contract(b, 'CONTRACT-01', c => c.input.vectors[1].output.value = '0'), 'contract oracle: wrapping');
  run('contracts-untyped-integer', b => contract(b, 'CONTRACT-01', c => c.input.vectors[0].input = '41'), 'contract oracle: wrapping');
  run('contracts-invalid-positive-guard-vector', b => contract(b, 'CONTRACT-08', c => c.input.argument.value = '9223372036854775807'), 'contract oracle: applicable invocation');
  run('contracts-stale-capture-accepted', b => contract(b, 'CONTRACT-02', c => c.expected.new_certified_status = true), 'contract oracle: evidence binding');
  run('contracts-runtime-prover', b => contract(b, 'CONTRACT-03', c => c.expected.prover_calls = 1), 'contract oracle: runtime proof-free');
  run('contracts-strength-promotion', b => contract(b, 'CONTRACT-03', c => c.expected.claim_kind = 'total-correctness'), 'contract oracle: composition');
  run('contracts-missing-implication-accepted', b => contract(b, 'CONTRACT-04', c => c.expected.new_certified_status = true), 'contract oracle: missing intermediate');
  run('contracts-literal-only-family', b => contract(b, 'CONTRACT-05', c => c.input.captures_supplied_after_compilation = false), 'contract oracle: runtime family');
  run('contracts-cyclic-evidence-omitted', b => contract(b, 'CONTRACT-06', c => c.input.variants = c.input.variants.filter(v => v !== 'cyclic-derivation')), 'contract oracle: forged companions');
  run('contracts-precondition-bypass', b => contract(b, 'CONTRACT-07', c => c.expected.candidate_body_started = true), 'contract oracle: false precondition');
  run('contracts-guard-erased', b => contract(b, 'CONTRACT-08', c => c.expected.guard_erased = true), 'contract oracle: applicable invocation');
  run('contracts-solver-success-promoted', b => contract(b, 'CONTRACT-09', c => c.input.attempts.find(a => a.case === 'bare-solver-answer').outcome = 'proved'), 'contract oracle: claim outcome matrix');
  run('contracts-timeout-releases', b => contract(b, 'CONTRACT-09', c => c.input.attempts.find(a => a.case === 'proof-timeout').release_allowed = true), 'contract oracle: claim outcome matrix');
  run('contracts-erased-inspection', b => contract(b, 'CONTRACT-10', c => c.expected.selected_inspection_metadata_retained = false), 'contract oracle: first-class companions');
  run('contracts-ordinary-run-certified', b => contract(b, 'CONTRACT-11', c => c.expected.behavioral_certification_claimed = true), 'contract oracle: ordinary execution');
  run('contracts-ghost-resource-copy', b => contract(b, 'CONTRACT-12', c => c.expected.resource_ownership_duplicated = true), 'contract oracle: ghost eligibility');
  run('contracts-unrelated-Wasm-executed', b => contract(b, 'CONTRACT-13', c => c.expected.candidate_body_started = true), 'contract oracle: unrelated Wasm');
  run('contracts-producer-selects-claim', b => contract(b, 'CONTRACT-14', c => c.expected.independent_expected_claim_required = false), 'contract oracle: statement export');
  run('contracts-unsupported-weakened', b => contract(b, 'CONTRACT-15', c => c.expected.claim_silently_weakened = true), 'contract oracle: unsupported declaration');
  const calculator = (b, id, change) => changeJson(b, 'specs/conformance/calculator-cases.json', p => change(p.cases.find(c => c.id === id)));
  run('calculator-document-removed', b => changeJson(b, 'specs/spec-family.json', p => p.normative_documents = p.normative_documents.filter(d => d.id !== 'SPEC-CALC001')), 'required document missing');
  run('calculator-truncating-policy', b => changeJson(b, 'specs/STATUS.json', p => p.calculator.division = 'integer-truncation'), 'calculator policy: division');
  run('calculator-false-benchmark-pass', b => changeJson(b, 'specs/STATUS.json', p => {
    p.runtime_exists = false; p.calculator.benchmark_execution = 'passed';
  }), 'calculator policy: false greenfield');
  run('calculator-truncated-division', b => calculator(b, 'CALC-01', c => c.input.vectors[0].numerator = '0'), 'calculator oracle: exact arithmetic');
  run('calculator-rounded-decimal', b => calculator(b, 'CALC-01', c => { c.input.vectors[1].numerator = '30000000000000004'; c.input.vectors[1].denominator = '100000000000000000'; }), 'calculator oracle: exact arithmetic');
  run('calculator-lossy-JSON-number', b => calculator(b, 'CALC-01', c => c.input.vectors[3].numerator = Number(c.input.vectors[3].numerator)), 'calculator oracle: exact arithmetic');
  run('calculator-left-associated-power', b => calculator(b, 'CALC-01', c => c.input.vectors.find(v => v.expression === '2^3^2').numerator = '64'), 'calculator oracle: exact arithmetic');
  run('calculator-missing-exact-vector', b => calculator(b, 'CALC-01', c => c.input.vectors.pop()), 'calculator oracle: exact arithmetic');
  run('calculator-zero-division-success', b => calculator(b, 'CALC-03', c => c.expected.successful_numeric_result = true), 'calculator oracle: numeric errors');
  run('calculator-negative-denominator', b => calculator(b, 'CALC-04', c => c.input.vectors[0].result = { numerator: '-1', denominator: '-2' }), 'calculator oracle: normalized rational');
  run('calculator-noncanonical-zero', b => calculator(b, 'CALC-04', c => c.input.vectors[3].result.denominator = '7'), 'calculator oracle: normalized rational');
  run('calculator-decoder-bypasses-invariant', b => calculator(b, 'CALC-04', c => c.input.paths.pop()), 'calculator oracle: normalized rational');
  run('calculator-I64-wrap-changed', b => calculator(b, 'CALC-05', c => c.expected.core_output.value = '9223372036854775808'), 'calculator oracle: I64 boundary');
  run('calculator-I64-narrowing-accepted', b => calculator(b, 'CALC-05', c => { delete c.input.vectors[2].error; c.input.vectors[2].value = '-9223372036854775808'; }), 'calculator oracle: I64 boundary');
  run('calculator-failed-definition-commits', b => calculator(b, 'CALC-06', c => c.expected.failed_submission_state_unchanged = false), 'calculator oracle: lexical session');
  run('calculator-definition-runs-body', b => calculator(b, 'CALC-07', c => c.expected.evaluations_during_definition_admission = 1), 'calculator oracle: definition admission');
  run('calculator-missing-budget-category', b => calculator(b, 'CALC-08', c => c.input.limit_categories.pop()), 'calculator oracle: bounded exact');
  run('calculator-quota-falls-back-to-float', b => calculator(b, 'CALC-08', c => c.expected.approximation_fallback = true), 'calculator oracle: bounded exact');
  run('calculator-formatting-limit-after-commit', b => calculator(b, 'CALC-08', c => c.input.state_control = 'format-after-commit'), 'calculator oracle: bounded exact');
  run('calculator-rounded-display-is-exact', b => calculator(b, 'CALC-09', c => c.expected.truncated_decimal_labeled_exact = true), 'calculator oracle: exact display');
  run('calculator-stale-edit-accepted', b => calculator(b, 'CALC-10', c => c.input.variants[0].outcome = 'accept'), 'calculator oracle: authoring boundary');
  run('calculator-producer-changes-tests', b => calculator(b, 'CALC-11', c => c.expected.producer_can_change_acceptance = true), 'calculator oracle: benchmark integrity');
  run('calculator-failed-runs-omitted', b => calculator(b, 'CALC-11', c => c.expected.failed_runs_omitted = true), 'calculator oracle: benchmark integrity');
  run('calculator-cost-metric-omitted', b => calculator(b, 'CALC-11', c => c.input.metrics = c.input.metrics.filter(m => m !== 'total_tokens')), 'calculator oracle: benchmark integrity');
  run('calculator-circular-oracle', b => calculator(b, 'CALC-12', c => c.expected.shared_implementation_is_sole_oracle = true), 'calculator oracle: independent evidence');
  run('calculator-test-is-proof', b => calculator(b, 'CALC-12', c => c.expected.tests_establish_universal_proof = true), 'calculator oracle: independent evidence');
  run('calculator-bootstrap-expanded', b => calculator(b, 'CALC-13', c => c.expected.bootstrap_expanded = true), 'calculator oracle: application scope');
  run('calculator-missing-library-gate', b => changeJson(b, 'specs/roadmap.json', p => p.milestones.find(m => m.id === 'MA1').entry_gates.pop()), 'calculator entry gates');
  run('calculator-precedes-core', b => changeJson(b, 'specs/roadmap.json', p => p.milestones.find(m => m.id === 'MA1').depends_on = ['M2']), 'calculator milestone dependencies');
  run('calculator-blocks-bootstrap', b => changeJson(b, 'specs/roadmap.json', p => p.milestones.find(m => m.id === 'M4').depends_on.push('MA1')), 'contract profile must not block ordinary core');
  const worker = (b, id, change) => changeJson(b, 'specs/conformance/worker-cases.json', p => change(p.cases.find(c => c.id === id)));
  run('worker-valid-object-key-order', b => worker(b, 'WORKER-01', c => {
    c.input.initial_state = { pending: null, count: { value: '0', type: 'I64' } };
  }), null);
  run('worker-scenarios-unregistered', b => {
    changeJson(b, 'specs/spec-family.json', p => p.scenario_files = p.scenario_files.filter(f => f !== 'conformance/worker-cases.json'));
    refreshed(b);
  }, 'worker oracle: required case');
  run('worker-false-runtime-claim', b => changeJson(b, 'specs/STATUS.json', p => {
    p.runtime_exists = false; p.worker_contracts.execution = 'passed';
  }), 'worker policy: false greenfield');
  run('worker-new-kernel-syntax', b => changeJson(b, 'specs/STATUS.json', p => p.worker_contracts.new_kernel_syntax = true), 'worker policy: new_kernel_syntax');
  run('worker-missing-package-gate', b => changeJson(b, 'specs/roadmap.json', p => p.milestones.find(m => m.id === 'MW1').entry_gates.pop()), 'worker milestone entry gates');
  run('worker-missing-async-dependency', b => changeJson(b, 'specs/roadmap.json', p => p.milestones.find(m => m.id === 'MW2').depends_on = ['MW1']), 'worker milestone dependencies');
  run('worker-missing-round-trip-requirement', b => changeJson(b, 'specs/roadmap.json', p => {
    const m = p.milestones.find(m => m.id === 'MW1'); m.required_requirements = m.required_requirements.filter(r => r !== 'P-ROUND-01');
  }), 'worker milestone requirement');
  run('worker-missing-admission-budget-gate', b => changeJson(b, 'specs/roadmap.json', p => {
    const m = p.milestones.find(m => m.id === 'M2'); m.required_requirements = m.required_requirements.filter(r => r !== 'B-CHECK-07');
  }), 'worker admission limit gate');
  run('worker-missing-async-ownership-gate', b => changeJson(b, 'specs/roadmap.json', p => {
    const m = p.milestones.find(m => m.id === 'M6'); m.required_requirements = m.required_requirements.filter(r => r !== 'RA-ASYNC-05');
  }), 'worker async ownership gate');
  for (const [name, id, section, key, value, error] of [
    ['literal-only', 'WORKER-01', 'input', 'candidate_supplied_after_coordinator_compilation', false, 'generated worker setup'],
    ['captured-capability', 'WORKER-01', 'input', 'capability_captured_by_worker', true, 'generated worker setup'],
    ['pure-worker-issues-effects', 'WORKER-01', 'expected', 'worker_host_requests', 1, 'worker state and retirement'],
    ['premature-pin-release', 'WORKER-01', 'expected', 'pins_at_cancel_ack', 0, 'worker state and retirement'],
    ['late-success-delivery', 'WORKER-01', 'expected', 'successful_result_deliveries', 1, 'worker state and retirement'],
    ['Any-fallback', 'WORKER-02', 'expected', 'dynamic_Any_fallback', true, 'interface boundary'],
    ['runtime-compiler', 'WORKER-03', 'expected', 'source_compiler_calls', 1, 'effect widening invariant'],
    ['erased-effect-witness', 'WORKER-03', 'expected', 'destination_witness_retained', false, 'effect widening invariant'],
    ['mutated-original-interface', 'WORKER-03', 'expected', 'original_interface_unchanged', false, 'effect widening invariant'],
    ['round-trip-artifact-equality', 'WORKER-04', 'expected', 'artifact_bytes_required_identical', true, 'round-trip invariant'],
    ['transferred-acceptance', 'WORKER-04', 'expected', 'unchecked_acceptance_transferred', true, 'round-trip invariant'],
    ['changed-round-trip-context', 'WORKER-04', 'input', 'context', 'new-host-contract-version', 'round-trip context'],
    ['implicit-dependency-fetch', 'WORKER-05', 'expected', 'implicit_fetches', 1, 'package admission'],
    ['digest-bypasses-admission', 'WORKER-05', 'expected', 'signature_or_digest_bypasses_checks', true, 'package admission'],
    ['budget-reset', 'WORKER-06', 'expected', 'nested_budget_reset', true, 'admission budget outcomes'],
    ['allocation-before-limit', 'WORKER-06', 'expected', 'allocation_before_limit_check', true, 'admission budget outcomes'],
    ['producer-selects-interface', 'WORKER-07', 'input', 'expected_interface_owner', 'producer', 'independent admission input'],
    ['fallback-checker', 'WORKER-07', 'expected', 'fallback_checker_used', true, 'candidate admission'],
    ['cancel-means-native-finished', 'WORKER-08', 'expected', 'cancellation_ack_proves_native_completion', true, 'async ownership invariant'],
    ['pure-code-unbounded', 'WORKER-09', 'expected', 'pure_code_exempt_from_limits', true, 'execution bounds'],
    ['wrong-interruption-bound', 'WORKER-09', 'expected', 'observed_max_guest_work_between_checks', 9, 'execution bounds'],
    ['failed-stack-returned', 'WORKER-09', 'expected', 'failed_output_exposed_as_T', true, 'execution bounds'],
    ['effects-rolled-back', 'WORKER-10', 'expected', 'prior_effects_rolled_back', true, 'operation observation invariant'],
    ['hidden-observation-truncation', 'WORKER-10', 'expected', 'omission_marked_when_exhausted', false, 'operation observation invariant'],
    ['plan-is-execution', 'WORKER-10', 'expected', 'effect_plan_counts_as_completion', true, 'operation observation invariant'],
    ['swarm-overclaim', 'WORKER-11', 'expected', 'swarm_runtime_claimed', true, 'worker scope'],
    ['unauthorized-capture-export', 'WORKER-12', 'expected', 'capture_published', true, 'export authority invariant'],
  ]) {
    run(`worker-${name}`, b => worker(b, id, c => { c[section][key] = value; }), `worker oracle: ${error}`);
  }
  run('worker-invalid-state-vector', b => worker(b, 'WORKER-01', c => c.expected.states[1].count.value = '0'), 'worker oracle: worker state and retirement');
  for (const [id, error] of [
    ['WORKER-02', 'interface rejection matrix'], ['WORKER-05', 'package rejection matrix'],
    ['WORKER-07', 'candidate witness matrix'], ['WORKER-08', 'async ownership matrix'], ['WORKER-09', 'execution outcome matrix'],
  ]) {
    run(`worker-missing-matrix-row-${id}`, b => worker(b, id, c => c.input.variants.pop()), `worker oracle: ${error}`);
  }
  run('worker-missing-budget-category', b => worker(b, 'WORKER-06', c => c.input.limit_categories.pop()), 'worker oracle: admission budget matrix');
  run('worker-missing-state-control', b => worker(b, 'WORKER-01', c => c.input.transition_controls.pop()), 'worker oracle: worker transition controls');
  run('worker-counter-overflow-regression', b => worker(b, 'WORKER-01', c => c.input.transition_controls[3].expected_count.value = '0'), 'worker oracle: worker transition controls');
  run('worker-wrong-attempt-clears-state', b => worker(b, 'WORKER-01', c => c.input.transition_controls[1].expected_pending = null), 'worker oracle: worker transition controls');
  run('worker-cancelled-buffered-result-leaks', b => worker(b, 'WORKER-08', c => {
    c.input.variants.find(v => v.case === 'complete-cancel-before-delivery').undelivered_result_retired = false;
  }), 'worker oracle: async ownership matrix');
  run('worker-revoke-already-delivered-owner', b => worker(b, 'WORKER-08', c => {
    c.input.variants.find(v => v.case === 'delivery-before-cancel').late_cancel_revokes_result = true;
  }), 'worker oracle: async ownership matrix');
  run('worker-trap-misreported-as-cancelled', b => worker(b, 'WORKER-08', c => {
    c.input.variants.find(v => v.case === 'trap-with-native-pin').primary_outcome = 'cancelled';
  }), 'worker oracle: async ownership matrix');
  run('worker-incomplete-operation-correlation', b => worker(b, 'WORKER-10', c => c.expected.required_fields.pop()), 'worker oracle: operation observation invariant');
  run('worker-missing-request-observation', b => worker(b, 'WORKER-10', c => c.input.operations.pop()), 'worker oracle: operation observation setup');
  const octet = (b, id, change) => changeJson(b, 'specs/conformance/octet-adoption-cases.json', p => change(p.cases.find(c => c.id === id)));
  run('octet-valid-matrix-order', b => octet(b, 'OCTET-01', c => c.input.variants.reverse()), null);
  run('octet-adoption-scenarios-unregistered', b => {
    changeJson(b, 'specs/spec-family.json', p => p.scenario_files = p.scenario_files.filter(f => f !== 'conformance/octet-adoption-cases.json'));
    refreshed(b);
  }, 'Octet adoption oracle: required case');
  run('octet-adoption-runtime-overclaim', b => changeJson(b, 'specs/STATUS.json', p => {
    p.runtime_exists = false; p.octet_adoption.execution = 'passed';
  }), 'Octet adoption policy: false greenfield');
  run('octet-adoption-Aeneas-replaced', b => changeJson(b, 'specs/STATUS.json', p => p.octet_adoption.Aeneas_route_replaced = true), 'Octet adoption policy: Aeneas_route_replaced');
  run('octet-adoption-lints-replace-architecture', b => changeJson(b, 'specs/STATUS.json', p => p.octet_adoption.architecture_policy = 'lint-only'), 'Octet adoption policy: architecture_policy');
  for (const [milestone, ref] of [['M1', 'VT-OCTET-01'], ['M5', 'H-AUTH-03'], ['M6', 'DX-PROTOCOL-03'], ['MW1', 'DX-TYPE-04'], ['MW2', 'H-RECEIPT-02']]) {
    run(`octet-missing-gate-${milestone}`, b => changeJson(b, 'specs/roadmap.json', p => {
      const m = p.milestones.find(m => m.id === milestone); m.required_requirements = m.required_requirements.filter(r => r !== ref);
    }), 'Octet adoption milestone gate');
  }
  for (const [name, id, section, key, value, error] of [
    ['producer-authority-facts', 'OCTET-01', 'input', 'authority_facts', 'producer-claims', 'authority setup'],
    ['pure-authority-mint', 'OCTET-01', 'expected', 'pure_decision_creates_live_authority', true, 'authority invariant'],
    ['execute-before-consume', 'OCTET-01', 'expected', 'admission_commit_before_protected_operation', false, 'authority invariant'],
    ['missing-facts-allow', 'OCTET-02', 'expected', 'missing_fact_defaults_to_allow', true, 'witness rejection'],
    ['data-mints-witness', 'OCTET-03', 'expected', 'live_witnesses_created', 1, 'witness construction invariant'],
    ['receipt-is-capability', 'OCTET-04', 'expected', 'receipt_is_capability', true, 'receipt scope'],
    ['wrong-FSM-pair-count', 'OCTET-05', 'expected', 'constructor_pairs', 14, 'lifecycle scope'],
    ['FSM-is-full-async-proof', 'OCTET-05', 'expected', 'full_async_protocol_coverage_claimed', true, 'lifecycle scope'],
    ['refinement-solver-required', 'OCTET-06', 'expected', 'general_refinement_inference_required', true, 'nominal domain scope'],
    ['unknown-call-is-pure', 'OCTET-07', 'expected', 'unknown_defaults_to_pure', true, 'resolved effect scope'],
    ['indirect-calls-need-enumeration', 'OCTET-07', 'expected', 'all_runtime_program_values_must_be_enumerated', true, 'resolved effect scope'],
    ['profile-replaces-deny-all', 'OCTET-08', 'expected', 'full_deny_all_replaced_by_profile', true, 'architecture scope'],
    ['identity-baseline-allows-findings', 'OCTET-08', 'expected', 'source_identity_baseline_is_finding_allowance', true, 'architecture scope'],
    ['phase-failure-hidden', 'OCTET-09', 'expected', 'successful_sibling_phase_hides_failure', true, 'evidence scope'],
    ['producer-selects-claim', 'OCTET-09', 'expected', 'consumer_claim_selected_by_producer', true, 'evidence scope'],
    ['Rust-lint-bans-Noble-recursion', 'OCTET-10', 'expected', 'Rust_recursion_lint_is_Noble_recursion_ban', true, 'adoption non-claims'],
    ['design-reference-selects-pin', 'OCTET-10', 'expected', 'toolchain_pin_selected_by_design_reference', true, 'adoption non-claims'],
  ]) {
    run(`octet-${name}`, b => octet(b, id, c => { c[section][key] = value; }), `Octet adoption oracle: ${error}`);
  }
  for (const [name, id, variant, key, value, error] of [
    ['denial-mints-witness', 'OCTET-01', 'deny', 'witnesses_created', 1, 'authority phase matrix'],
    ['denial-performs-operation', 'OCTET-01', 'deny', 'protected_operations', 1, 'authority phase matrix'],
    ['preflight-consumes-witness', 'OCTET-01', 'preflight-capacity-failure', 'witness_consumptions', 1, 'authority phase matrix'],
    ['unknown-restores-witness', 'OCTET-01', 'external-outcome-unknown-after-commit', 'witness_restored', true, 'authority phase matrix'],
    ['plan-becomes-success-receipt', 'OCTET-04', 'plan-as-success', 'receipt', 'operation-success', 'receipt constructor matrix'],
    ['imported-description-trusted', 'OCTET-04', 'schema-valid-import-without-provenance', 'trusted', true, 'receipt constructor matrix'],
    ['late-effect-success-uncancels-task', 'OCTET-04', 'cancelled-invocation-late-confirmed-external-success', 'invocation_outcome', 'normal-return', 'receipt constructor matrix'],
    ['wildcard-establishes-FSM-coverage', 'OCTET-05', 'wildcard-covers-new-event', 'outcome', 'coverage-accepted', 'lifecycle coverage matrix'],
    ['invalid-decoder-value-accepted', 'OCTET-06', 'decoder-bypasses-invariant', 'outcome', 'accepted', 'nominal domain matrix'],
    ['summary-erases-direct-effect', 'OCTET-07', 'summary-erases-known-direct-effect', 'outcome', 'accepted', 'resolved effect matrix'],
    ['uncovered-binary-accepted', 'OCTET-08', 'uncovered-production-binary', 'outcome', 'scoped-policy-acceptance', 'architecture matrix'],
    ['fixture-analysis-promoted', 'OCTET-09', 'fixture-Charon-analysis-as-Noble-kernel-proof', 'outcome', 'formal-proof', 'evidence tier matrix'],
  ]) {
    run(`octet-${name}`, b => octet(b, id, c => { c.input.variants.find(v => v.case === variant)[key] = value; }), `Octet adoption oracle: ${error}`);
  }
  for (const [id, error] of [['OCTET-01', 'authority phase matrix'], ['OCTET-02', 'witness binding matrix'],
    ['OCTET-03', 'witness construction matrix'], ['OCTET-04', 'receipt constructor matrix'], ['OCTET-05', 'lifecycle coverage matrix'],
    ['OCTET-06', 'nominal domain matrix'], ['OCTET-07', 'resolved effect matrix'], ['OCTET-08', 'architecture matrix'], ['OCTET-09', 'evidence tier matrix']]) {
    run(`octet-missing-control-${id}`, b => octet(b, id, c => c.input.variants.pop()), `Octet adoption oracle: ${error}`);
  }
  run('octet-missing-evidence-binding', b => octet(b, 'OCTET-09', c => c.input.required_bindings.pop()), 'Octet adoption oracle: evidence bindings');
  return results;
}

function main() {
  const args = process.argv.slice(2);
  const allowed = ['--refresh-ledger', '--self-test', '--report'];
  for (const arg of args) if (!allowed.includes(arg)) throw new Error(`unknown option: ${arg}`);
  const bundle = loadBundle(ROOT);
  if (args.includes('--refresh-ledger')) {
    const result = validate(bundle, { ignoreLedger: true });
    // The new index is the only local-link target allowed to be absent during initialization.
    const errors = result.errors.filter(e => !e.endsWith('-> requirements.json'));
    if (errors.length) throw new Error(errors.join('\n'));
    const ledger = deriveLedger(bundle);
    const text = JSON.stringify(ledger, null, 2) + '\n';
    writeFileSync(path.join(ROOT, 'specs/requirements.json'), text);
    bundle.texts.set('specs/requirements.json', text);
    bundle.paths.add('specs/requirements.json');
  }
  const result = validate(bundle);
  if (result.errors.length) throw new Error(result.errors.join('\n'));
  const tests = args.includes('--self-test') ? selfTest(bundle) : [];
  const sourcePaths = new Set(parse(bundle, 'specs/spec-family.json').normative_documents
    .filter(d => d.source).map(d => path.posix.normalize(path.posix.join('specs', d.source))));
  const report = {
    revision: REVISION,
    validator: { file: 'tools/check-specs.mjs', sha256: createHash('sha256').update(readFileSync(fileURLToPath(import.meta.url))).digest('hex'),
      adapter: { file: 'tools/cairn-specs.mjs', sha256: createHash('sha256').update(readFileSync(path.join(ROOT, 'tools/cairn-specs.mjs'))).digest('hex') } },
    lane: 'document-validation-only',
    result: 'passed',
    runner: { bun: process.versions.bun ?? null, node_api: process.versions.node },
    ...result.summary,
    self_tests: tests,
    inputs: [...bundle.texts].filter(([name]) => (name.startsWith('specs/') && name !== 'specs/VALIDATION.json')
      || checkedMarkdown(name) || sourcePaths.has(name))
      .sort(([a], [b]) => a.localeCompare(b, 'en'))
      .map(([file, text]) => ({ file, sha256: createHash('sha256').update(text).digest('hex') })),
    non_claims: ['No Noble execution', 'No proof checking', 'No Component Model conformance', 'No complete semantic coverage claim'],
  };
  if (args.includes('--report')) writeFileSync(path.join(ROOT, 'specs/VALIDATION.json'), JSON.stringify(report, null, 2) + '\n');
  console.log(JSON.stringify({ ...result.summary, lane: report.lane, result: report.result, self_tests_passed: tests.length }, null, 2));
}

if (import.meta.main) {
  try { main(); } catch (error) { console.error(`spec validation failed: ${error.message}`); process.exitCode = 1; }
}
