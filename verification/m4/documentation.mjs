import assert from 'node:assert/strict';
import { digest, stable } from '../../tools/m3-wasm-policy.mjs';

const PUBLICATION = 'specs/conformance/language-workflow-cases.json#DX-12';
const HOST_IMPORTS = new Map([['noble:test_emit', 'test.emit'], ['noble:test_abort', 'test.abort']]);
const LIMIT_KEYS = ['allocation_bytes', 'recipe_leaves', 'program_depth', 'operand_bytes', 'continuation_bytes', 'steps'];
const CONTROL_OUTCOMES = {
  'compiler-absent': 'not-run',
  'runtime-example-only-typechecked': 'runtime-not-run',
  'unsupported-example': 'unsupported-visible-in-coverage',
  'mismatched-output': 'failed-visible-in-coverage',
  'execution-timeout': 'timeout-visible-in-coverage',
  'effectful-example-without-explicit-test-host': 'reject-run-no-ambient-credentials',
};
const same = (left, right) => stable(left) === stable(right);

function quiet(report) {
  return report.guest_requests === 0 && report.protected_operations === 0
    && report.candidate_prepare_requests === 0;
}

function runtimeMatches(report, expected) {
  return report.stage === 'wasm' && report.outcome === 'normal' && report.status === 0
    && same(report.stack, [expected]) && quiet(report) && same(report.request_trace, []);
}

// CoreEngine's imports are a conservative authority bound, not the program's
// inferred latent effect set. Its shared runtime imports both test functions
// even for pure modules. Requiring mappings for the entire bound is deliberately
// stricter than trusting example declarations or scanning source for host words.
function preparedHostCheck(compiled, host, declaredHosts) {
  assert.ok(compiled.module instanceof WebAssembly.Module, 'documentation needs an actual compiled Wasm module');
  assert.ok(Number.isSafeInteger(compiled.engine.compilations) && compiled.engine.compilations > 0,
    'documentation preparation must actually compile Wasm');
  const imports = WebAssembly.Module.imports(compiled.module);
  const functions = imports.filter(entry => entry.kind === 'function');
  const unknown = functions.filter(entry => !HOST_IMPORTS.has(`${entry.module}:${entry.name}`));
  const authorities = functions.map(entry => HOST_IMPORTS.get(`${entry.module}:${entry.name}`)).filter(Boolean);
  const required = [...new Set([...declaredHosts, ...authorities])].sort();
  const missing = required.filter(name => !Object.hasOwn(host.mappings, name));
  const unsafe = required.filter(name => Object.hasOwn(host.mappings, name)
    && host.mappings[name]?.resource_free !== true);
  const mismatched = functions.filter(entry => {
    const name = HOST_IMPORTS.get(`${entry.module}:${entry.name}`);
    if (!name || !Object.hasOwn(host.mappings, name)) return false;
    const mapping = host.mappings[name];
    return mapping.module !== entry.module || mapping.import !== entry.name
      || mapping.effect_id !== (name === 'test.emit' ? 0 : 1);
  });
  const trace = compiled.engine.trace.slice();
  assert.equal(trace.length, 0, 'module preparation must not issue guest requests');
  return {
    basis: 'actual-WebAssembly.Module.imports; conservative-host-authority-superset-not-an-inferred-effect-set',
    imports, compiled_host_authorities: authorities, declared_required_hosts: declaredHosts,
    missing_mappings: missing, unsafe_mappings: unsafe, unknown_function_imports: unknown,
    mismatched_import_mappings: mismatched,
    ambient_credentials_allowed: host.ambient_credentials,
    allowed: host.ambient_credentials === false && missing.length === 0 && unsafe.length === 0
      && unknown.length === 0 && mismatched.length === 0,
    preparation_request_trace: trace,
  };
}

function validateHost(host) {
  assert.ok(host && typeof host === 'object', 'an explicit documentation test host is required');
  assert.equal(host.ambient_credentials, false, 'documentation may not expose ambient credentials');
  assert.ok(host.mappings && typeof host.mappings === 'object' && !Array.isArray(host.mappings));
  // The supplied resource-free CoreEngine adapter has this hard trace bound.
  // Do not present an unenforced, smaller documentary budget as a runtime limit.
  assert.equal(host.guest_request_budget, 4096, 'bind the real CoreEngine request bound');
  assert.ok(host.limits && typeof host.limits === 'object');
  assert.deepEqual(Object.keys(host.limits).sort(), LIMIT_KEYS.slice().sort());
  for (const key of LIMIT_KEYS) {
    assert.ok(Number.isSafeInteger(host.limits[key]) && host.limits[key] > 0, `finite positive runtime limit required: ${key}`);
  }
}

export function runDocumentationWorkflow(context) {
  const { fixture, configuration, save, compile, runSource, timedRun, compilerAvailable } = context;
  assert.equal(fixture.id, 'DX-12');
  assert.equal(fixture.input.harness, 'compiler-backed-documentation');
  assert.equal(fixture.input.variants.length, 8, 'every published DX-12 variant must be accounted');
  assert.equal(configuration && typeof configuration, 'object');
  assert.equal(typeof save, 'function');
  assert.equal(typeof compile, 'function');
  assert.equal(typeof runSource, 'function');
  assert.equal(typeof timedRun, 'function');
  assert.equal(typeof compilerAvailable, 'boolean');
  const host = configuration.test_host;
  validateHost(host);
  const variants = fixture.input.variants;
  const runtime = variants[0], rejection = variants[1];
  assert.equal(runtime.source, '41 [ 1 + ] run');
  assert.equal(runtime.mode, 'runtime');
  assert.deepEqual(runtime.expected_value, { type: 'I64', value: '42' });
  assert.equal(rejection.source, '1 true +');
  assert.equal(rejection.mode, 'static-rejection');
  assert.equal(rejection.expected_diagnostic, 'word-input-type');
  assert.equal(rejection.guest_requests, 0);
  assert.deepEqual(variants.slice(2).map(({ case: id, outcome }) => [id, outcome]), Object.entries(CONTROL_OUTCOMES));
  assert.equal(fixture.expected.actual_compiler_required, true);
  assert.equal(fixture.expected.runtime_backend_required_for_runtime_claim, true);
  assert.equal(fixture.expected.rendered_text_match_sufficient, false);
  assert.equal(fixture.expected.example_pass_is_proof, false);

  const configurationDigest = digest(configuration);
  const receipt = {
    schema: 'noble-m4-documentation-workflow/v1', result: 'running',
    fixture: { id: fixture.id, input: fixture.input, expected: fixture.expected },
    fixture_sha256: digest({ id: fixture.id, input: fixture.input, expected: fixture.expected }),
    configuration, configuration_sha256: configurationDigest,
    scope: {
      publication: PUBLICATION,
      source_selection: 'the two exact source-bearing DX-12 variants; six separately labelled hostile harness controls',
      other_documentation: 'illustrative-or-unexecuted-by-this-workflow; not scanned and not claimed as runtime evidence',
      actual_compiler_required: true, runtime_backend: 'managed-linear-memory',
      rendered_text_matching_used: false, passing_examples_are_proofs: false,
    },
    examples: [], controls: [], coverage: [],
    non_claims: ['universal compiler correctness', 'universal backend refinement', 'all documentation executed',
      'live resources', 'ambient-credential hosts', 'guest-body entry established by a process timeout'],
  };

  function row(index, source, mode, expectation, fault, observe) {
    const variant = variants[index];
    const isControl = index >= 2;
    const record = {
      id: isControl ? variant.case : `${fixture.id}/${variant.mode}`,
      fixture_variant_index: index, fixture_variant: variant,
      role: isControl ? 'hostile-harness-control' : 'published-source-example',
      fault_injection: fault,
      replay: { publication: PUBLICATION, source, mode, expected: expectation, inputs: [],
        optimization: 'off', configuration_sha256: configurationDigest, test_host: host },
      expected_classification: isControl ? variant.outcome : 'passed',
      activity: { compiler_calls: 0, run_source_calls: 0, timed_run_calls: 0, direct_backend_execute_calls: 0 },
      classification: 'harness-error', status: 'harness-error', expectation_matched: false,
    };
    record.replay_sha256 = digest({ replay: record.replay, configuration });
    try {
      observe(record);
    } catch (error) {
      record.status = 'harness-error';
      record.classification = 'harness-error';
      record.expectation_matched = false;
      record.error = { name: error.name ?? 'Error', message: String(error.message ?? error), stack: error.stack ?? null };
    }
    record.harness_result = record.expectation_matched ? 'passed' : 'failed';
    (isControl ? receipt.controls : receipt.examples).push(record);
    receipt.coverage.push({ id: record.id, fixture_variant_index: index, role: record.role,
      status: record.status, classification: record.classification, expected_classification: record.expected_classification,
      harness_result: record.harness_result });
  }

  function unavailable(record, available) {
    record.compiler_available = available;
    if (available) return false;
    record.status = 'not-run'; record.classification = 'not-run';
    record.execution = { compiler_invoked: false, backend_executed: false, fallback_used: false,
      reason: 'selected actual compiler unavailable; no source interpreter or rendered-text fallback' };
    record.expectation_matched = record.expected_classification === record.classification;
    return true;
  }

  function withPrepared(record, selectedHost, declaredHosts, action) {
    record.replay.test_host = selectedHost;
    record.replay_sha256 = digest({ replay: record.replay, configuration });
    record.activity.compiler_calls += 1;
    const compiled = compile(`DX-12-${record.id.replaceAll('/', '-')}`, record.replay.source, [], 'off');
    try {
      record.preparation = { module: compiled.metadata, compilations: compiled.engine.compilations };
      record.host_preflight = preparedHostCheck(compiled, selectedHost, declaredHosts);
      for (const key of LIMIT_KEYS) {
        assert.equal(selectedHost.limits[key], compiled.engine.abi.limits_maximum[key],
          `direct execution and the CLI deadline run must bind the same actual ABI limit: ${key}`);
      }
      action(compiled);
    } finally {
      record.preparation.final_request_trace = compiled.engine.trace.slice();
      compiled.engine.close();
      record.preparation.engine_closed = true;
    }
  }

  function execute(record, compiled, expected) {
    assert.equal(record.host_preflight.allowed, true, 'compiled host authority exceeds explicit test-host mappings');
    const before = compiled.engine.compilations;
    record.activity.direct_backend_execute_calls += 1;
    const actual = compiled.engine.execute({ inputs: [], limits: record.replay.test_host.limits });
    record.observed = actual;
    record.preparation.compilations_after_execution = compiled.engine.compilations;
    assert.equal(compiled.engine.compilations, before, 'documentation execution must use the already compiled module');
    assert.ok(actual.guest_requests <= record.replay.test_host.guest_request_budget);
    const matched = runtimeMatches(actual, expected);
    record.comparison = { expected_stack: [expected], actual_stack: actual.stack, matched };
    record.status = matched ? 'passed' : 'failed';
    record.classification = matched ? 'passed' : 'failed-visible-in-coverage';
    record.expectation_matched = record.classification === record.expected_classification;
  }

  row(0, runtime.source, runtime.mode, runtime, null, record => {
    if (unavailable(record, compilerAvailable)) return;
    withPrepared(record, host, [], compiled => execute(record, compiled, runtime.expected_value));
  });

  row(1, rejection.source, rejection.mode, rejection, null, record => {
    if (unavailable(record, compilerAvailable)) return;
    record.activity.run_source_calls += 1;
    const actual = runSource('DX-12-static-rejection', rejection.source, 'off');
    record.observed = actual;
    // The current machine interface exposes check/type-reject, not a separate
    // word-input-type enum. Map that category only for this exact published
    // input and offending '+' span; never pin diagnostic prose.
    const categoryMatched = actual.stage === 'check' && actual.outcome === 'type-reject'
      && same(actual.source_span, { start: 7, end: 8 });
    record.diagnostic_mapping = { declared_category: rejection.expected_diagnostic,
      machine_category: { stage: actual.stage, outcome: actual.outcome },
      machine_interface: 'crates/noble-cli/src/core/output.rs: source_outcome',
      mapping_scope: 'exact DX-12 static input at the + word; not a general equivalence of all type-reject diagnostics',
      source_span: actual.source_span, prose_asserted: false, matched: categoryMatched };
    const matched = categoryMatched && actual.process_exit === 2 && quiet(actual);
    record.status = matched ? 'passed' : 'failed';
    record.classification = matched ? 'passed' : 'failed-visible-in-coverage';
    record.expectation_matched = matched;
  });

  row(2, runtime.source, runtime.mode, runtime,
    { target: 'compiler-availability', value: false, execution_results_injected: false }, record => {
      unavailable(record, false);
      record.expectation_matched &&= Object.values(record.activity).every(count => count === 0);
    });

  row(3, runtime.source, runtime.mode, runtime,
    { target: 'runtime-execution-step', value: 'deliberately omitted after real compilation', execution_results_injected: false }, record => {
      if (unavailable(record, compilerAvailable)) return;
      withPrepared(record, host, [], () => {
        assert.equal(record.host_preflight.allowed, true);
        record.status = 'runtime-not-run'; record.classification = 'runtime-not-run';
        record.execution = { static_acceptance: 'actual compiler accepted and Wasm prepared', backend_executed: false,
          runtime_claim: false, reason: 'compilation/typechecking alone cannot establish runtime observations' };
        record.expectation_matched = record.classification === record.expected_classification
          && record.activity.direct_backend_execute_calls === 0;
      });
    });

  row(4, 'import', 'runtime', { resolver_stage: 'resolve', outcome: 'unsupported', guest_requests: 0 },
    { target: 'source-example', value: 'unsupported module import', execution_results_injected: false }, record => {
      if (unavailable(record, compilerAvailable)) return;
      record.activity.run_source_calls += 1;
      const actual = runSource('DX-12-unsupported-example', record.replay.source, 'off');
      record.observed = actual;
      const unsupported = actual.stage === 'resolve' && actual.outcome === 'unsupported'
        && actual.process_exit === 4 && quiet(actual);
      record.status = unsupported ? 'unsupported' : 'failed';
      record.classification = unsupported ? 'unsupported-visible-in-coverage' : 'unexpected-unsupported-control-result';
      record.expectation_matched = record.classification === record.expected_classification;
    });

  const wrongValue = { type: 'I64', value: '43' };
  row(5, runtime.source, runtime.mode, { ...runtime, expected_value: wrongValue },
    { target: 'expected-output', original: runtime.expected_value, injected: wrongValue, execution_results_injected: false }, record => {
      if (unavailable(record, compilerAvailable)) return;
      withPrepared(record, host, [], compiled => {
        execute(record, compiled, wrongValue);
        record.original_expectation_matched = runtimeMatches(record.observed, runtime.expected_value);
        record.expectation_matched &&= record.original_expectation_matched;
      });
    });

  const deadlineMs = 1;
  row(6, runtime.source, runtime.mode, runtime,
    { target: 'whole-run-command-deadline-ms', value: deadlineMs, execution_results_injected: false }, record => {
      if (unavailable(record, compilerAvailable)) return;
      // A separate successful preparation establishes host admission for this
      // exact source before launching the deadline-bound real run command.
      withPrepared(record, host, [], () => assert.equal(record.host_preflight.allowed, true));
      record.activity.timed_run_calls += 1;
      const actual = timedRun('DX-12-execution-timeout', record.replay.source, deadlineMs);
      record.observed = actual;
      record.deadline = { milliseconds: deadlineMs, scope: 'complete actual noble run process, including preparation',
        guest_body_entry: 'not-established-by-process-timeout',
        runtime_reports_observed: actual.reports.filter(report => report.stage === 'wasm') };
      record.status = actual.timed_out === true ? 'timeout' : 'failed';
      record.classification = actual.timed_out === true ? 'timeout-visible-in-coverage' : 'deadline-did-not-time-out';
      record.expectation_matched = record.classification === record.expected_classification;
    });

  const hostless = { ...host, mappings: {} };
  row(7, '"documentation audit" test.emit', 'runtime',
    { required_hosts: ['test.emit'], admission: 'reject-before-execute', guest_requests: 0, protected_operations: 0 },
    { target: 'explicit-test-host-mappings', original: host.mappings, injected: {}, execution_results_injected: false }, record => {
      if (unavailable(record, compilerAvailable)) return;
      withPrepared(record, hostless, ['test.emit'], compiled => {
        const refused = !record.host_preflight.allowed
          && record.host_preflight.missing_mappings.includes('test.emit')
          && record.host_preflight.compiled_host_authorities.includes('test.emit')
          && compiled.engine.trace.length === 0 && record.activity.direct_backend_execute_calls === 0;
        record.status = refused ? 'rejected' : 'failed';
        record.classification = refused ? 'reject-run-no-ambient-credentials' : 'hostless-admission-control-failed';
        record.execution = { backend_executed: false, fallback_used: false, ambient_credentials_used: false,
          guest_request_observation: 'actual prepared engine trace; candidate body never submitted',
          request_trace: compiled.engine.trace.slice(), guest_requests: compiled.engine.trace.length };
        record.expectation_matched = record.classification === record.expected_classification;
      });
    });

  receipt.coverage_summary = { expected_variants: variants.length, retained_variants: receipt.coverage.length,
    omitted_variants: variants.length - receipt.coverage.length,
    by_status: Object.fromEntries([...new Set(receipt.coverage.map(item => item.status))]
      .map(status => [status, receipt.coverage.filter(item => item.status === status).length])),
    hostile_controls_are_not_production_failures: true };
  receipt.result = receipt.coverage.length === variants.length
    && receipt.examples.every(record => record.expectation_matched)
    && receipt.controls.every(record => record.expectation_matched) ? 'passed' : 'failed';
  save('documentation-workflow.json', receipt);
  return receipt;
}
