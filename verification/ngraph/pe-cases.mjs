#!/usr/bin/env node
// Execute the eighteen exact Numeric-Graph PE cases against a committed source tree.
// Usage: NOBLE_CARGO=<absolute pinned cargo executable> node verification/ngraph/pe-cases.mjs --out <new external directory>
// Each selected integration test exercises the production CLI or library and emits one
// PE_CASE_OBSERVATION JSON line only after its assertions. A Cargo build without selected
// tests, a missing variant, or an unobserved expectation is never an accepted case.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = fs.realpathSync(path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..'));
const caseFile = 'specs/conformance/numeric-graph-pe-cases.json';
const measurementFile = 'verification/ngraph/pe-2026-11-13.json';
const sourceScope = ['crates', 'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml',
  'policy/tool-selection.json',
  caseFile, measurementFile, 'verification/ngraph/pe-cases.mjs'];
const harnesses = [
  'ngraph-specialize-admitted-slice', 'ngraph-specialize-refused-slice',
  'ngraph-specialize-non-prefix-map', 'ngraph-specialize-static-arity',
  'ngraph-specialize-fold-whitelist', 'ngraph-specialize-wraparound',
  'ngraph-specialize-differential', 'ngraph-specialize-identity',
  'ngraph-specialize-budget-ladder', 'ngraph-specialize-measurement-review',
  'ngraph-specialize-source-readmission', 'ngraph-specialize-zero-host-requests',
  'ngraph-specialize-derivation-recompute', 'ngraph-specialize-cache-key-inputs',
  'ngraph-specialize-source-full-tail', 'ngraph-specialize-source-closure-identity',
  'ngraph-specialize-source-refusals', 'ngraph-specialize-source-closure-refusal',
];
const sha256 = bytes => createHash('sha256').update(bytes).digest('hex');
const digest = file => sha256(fs.readFileSync(file));
const insist = (value, message) => assert.ok(value, message);
const relative = file => path.join(root, file);

function git(...args) {
  const command = spawnSync('git', args, { cwd: root, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 });
  assert.equal(command.status, 0, `git ${args.join(' ')}: ${command.stderr || command.error}`);
  return command.stdout;
}

function snapshot() {
  assert.equal(git('status', '--porcelain', '--untracked-files=all', '--', ...sourceScope), '',
    'scoped source must be clean and committed');
  const files = git('ls-files', '-z', '--', ...sourceScope).split('\0').filter(Boolean).sort();
  for (const required of [caseFile, measurementFile, 'verification/ngraph/pe-cases.mjs',
    'crates/noble-cli/tests/ngraph.rs', 'crates/noble-cli/tests/ngraph/pe/cases.rs']) {
    insist(files.includes(required), `missing committed source: ${required}`);
  }
  return Object.fromEntries(files.map(file => [file, { sha256: digest(relative(file)) }]));
}

function canonicalCases() {
  const packet = JSON.parse(fs.readFileSync(relative(caseFile), 'utf8'));
  assert.equal(packet.schema_version, 1);
  assert.equal(packet.origin, 'partial-evaluation-v1');
  assert.equal(packet.cases.length, harnesses.length, 'the runner covers exactly eighteen registered cases');
  return packet.cases.map((row, index) => {
    const id = `NGRAPH-PE-CASE-${String(index + 1).padStart(2, '0')}`;
    assert.equal(row.id, id);
    assert.equal(row.profile, 'Numeric-Graph-v1');
    assert.equal(row.input.harness, harnesses[index], `${id}: harness changed`);
    insist(row.state?.implementation === 'implemented', `${id}: no implemented subject`);
    const { profile, kind, requirements, input, expected } = row;
    return { id, profile, kind, requirements, input, expected };
  });
}

function variantArrays(input) {
  return Object.fromEntries(Object.entries(input).filter(([, value]) => Array.isArray(value)));
}

function report(run, label) {
  insist(run && typeof run === 'object' && !Array.isArray(run), `${label}: missing run`);
  insist(typeof run.name === 'string' && run.name.length > 0, `${label}: run has no scenario`);
  insist(['cli', 'library', 'review'].includes(run.kind), `${label}: missing witness kind`);
  if (run.kind === 'cli') insist(Number.isInteger(run.exit), `${label}: missing CLI exit`);
  else assert.equal(run.exit, null, `${label}: only CLI runs have exits`);
  if (run.kind !== 'review') {
    insist(typeof run.stage === 'string' && run.stage.length > 0, `${label}: missing raw stage`);
    insist(typeof run.outcome === 'string' && run.outcome.length > 0, `${label}: missing raw outcome`);
    insist(run.code === null || typeof run.code === 'string', `${label}: missing raw code`);
    insist(Number.isInteger(run.guest_requests) && run.guest_requests >= 0, `${label}: missing host counter`);
    insist(Number.isInteger(run.protected_operations) && run.protected_operations >= 0,
      `${label}: missing protected-operation counter`);
    insist(typeof run.published_output === 'boolean', `${label}: missing publication observation`);
  }
}

function refusal(run, code, label) {
  report(run, label);
  insist(run.kind !== 'review', `${label}: cannot refuse through a review`);
  assert.equal(run.code, code, `${label}: diagnostic`);
  insist(run.outcome !== 'normal', `${label}: refusal was normal`);
  const unsupported = new Set(['E-AGGREGATE', 'E-EFFECT', 'E-RESOURCE', 'E-RECURSION',
    'E-REFLECT', 'E-DYNAMIC-RUN']);
  const budget = code.startsWith('E-BUDGET-');
  const expectedExit = budget ? 5 : unsupported.has(code) ? 4 : 2;
  if (run.kind === 'cli') {
    assert.equal(run.exit, expectedExit, `${label}: refusal exit`);
    assert.equal(run.outcome, budget ? 'exhausted' : unsupported.has(code) ? 'unsupported' : 'reject',
      `${label}: refusal outcome`);
  } else assert.equal(run.exit, null, `${label}: library-only refusal`);
  assert.equal(run.guest_requests, 0, `${label}: guest requests`);
  assert.equal(run.protected_operations, 0, `${label}: protected operations`);
  assert.equal(run.published_output, false, `${label}: refused path published`);
}

function normal(run, label) {
  report(run, label);
  insist(run.kind !== 'review', `${label}: review is not execution`);
  assert.equal(run.outcome, 'normal', `${label}: normal observation`);
  assert.equal(run.code, null, `${label}: normal path has a diagnostic`);
  assert.equal(run.exit, run.kind === 'cli' ? 0 : null, `${label}: normal exit`);
}

function checked(checks, key, value, id) {
  assert.deepEqual(checks[key], value, `${id}: ${key} was not observed as registered`);
}

function namedRuns(observed, names, id) {
  const byName = new Map(observed.runs.map(run => [run.name, run]));
  for (const name of names) insist(byName.has(name), `${id}: missing executed scenario ${name}`);
  return byName;
}

function measurementReview(checks, expected) {
  assert.equal(checks.measurement_file, measurementFile);
  const measurement = JSON.parse(fs.readFileSync(relative(measurementFile), 'utf8'));
  assert.equal(measurement.schema, 'noble-pe-measurement/v1');
  assert.equal(measurement.clean_source, true);
  assert.equal(measurement.issue, 32);
  insist(Array.isArray(measurement.records) && measurement.records.length > 0,
    'CASE-10: no measured original/residual records');
  assert.equal(checks.measurement_records, measurement.records.length,
    'CASE-10: marker record count differs from retained measurement');
  const sides = ['l3_original', 'l3_residual', 'l4_original', 'l4_residual'];
  for (const [index, record] of measurement.records.entries()) {
    for (const key of ['original_nodes', 'residual_nodes', 'folded_nodes', 'pruned_nodes',
      'retained_original', 'retained_residual', 'vectors', 'warm_vectors']) {
      insist(Number.isSafeInteger(record[key]) && record[key] >= 0, `CASE-10 record ${index}: ${key}`);
    }
    insist(record.original_nodes > 0 && record.residual_nodes > 0 && record.vectors > 0 &&
      record.warm_vectors > 0 && record.retained_original > 0 && record.retained_residual > 0,
    `CASE-10 record ${index}: empty side`);
    for (const key of sides) {
      insist(Number.isSafeInteger(record[key]) && record[key] > 0, `CASE-10 record ${index}: ${key} visits`);
      for (const timer of ['cold_eval_ns', 'warm_eval_ns_total']) {
        insist(Number.isSafeInteger(record[timer]?.[key]) && record[timer][key] > 0,
          `CASE-10 record ${index}: ${timer}.${key}`);
      }
    }
  }
  checked(checks, 'fields', expected.fields, 'CASE-10');
  checked(checks, 'equal_fuel_or_normalized_figure', false, 'CASE-10');
  return { file: measurementFile, sha256: digest(relative(measurementFile)),
    measured_source_revision: measurement.source_revision, records: measurement.records.length,
    review: 'historical measurement record, not a new measurement at tested_commit' };
}

function verifyObservation(c, observation) {
  const { id, input, expected } = c;
  assert.equal(observation?.id, id, `${id}: wrong case marker`);
  assert.equal(observation.harness, input.harness, `${id}: wrong harness marker`);
  assert.deepEqual(observation.variants, variantArrays(input), `${id}: missing or changed canonical variants`);
  const observed = observation.observed;
  insist(observed && Array.isArray(observed.runs) && observed.runs.length > 0,
    `${id}: no executed scenario observations`);
  insist(observed.checks && typeof observed.checks === 'object' && !Array.isArray(observed.checks),
    `${id}: no asserted case observations`);
  const names = observed.runs.map(run => run.name);
  assert.equal(new Set(names).size, names.length, `${id}: repeated run label`);
  for (const run of observed.runs) {
    report(run, id);
    if (run.kind !== 'review') {
      assert.equal(run.guest_requests, 0, `${id}: specialized path requested a guest`);
      assert.equal(run.protected_operations, 0, `${id}: specialized path used a protected operation`);
    }
  }
  const checks = observed.checks;
  let measurement = null;
  switch (id) {
    case 'NGRAPH-PE-CASE-01':
      namedRuns(observed, input.graphs, id);
      insist(observed.runs.some(run => run.kind === 'cli' && run.stage === 'specialization' &&
        run.outcome === 'normal' && run.published_output), `${id}: no checked residual publication`);
      insist(Number.isInteger(checks.checked_residuals) && checks.checked_residuals > 0,
        `${id}: no independently checked residual`);
      checked(checks, 'report_fields', expected.report_fields, id);
      checked(checks, 'static_map_verified', true, id);
      break;
    case 'NGRAPH-PE-CASE-02': {
      const byName = namedRuns(observed, input.variants, id);
      input.variants.forEach((name, index) => refusal(byName.get(name), expected.codes_by_variant[index], name));
      checked(checks, 'residual_nodes_created', 0, id);
      checked(checks, 'published_output', false, id);
      break;
    }
    case 'NGRAPH-PE-CASE-03':
      insist(observed.runs.some(run => run.outcome === 'normal'), `${id}: no evaluation`);
      checked(checks, 'dynamic_rank_correct', true, id);
      checked(checks, 'prefix_control_disagrees', true, id);
      break;
    case 'NGRAPH-PE-CASE-04':
      for (const [name, run] of namedRuns(observed, input.map_lengths, id)) {
        refusal(run, expected.code, name);
      }
      checked(checks, 'residual_created', false, id);
      break;
    case 'NGRAPH-PE-CASE-05':
      namedRuns(observed, input.nodes, id);
      checked(checks, 'only_static_whitelist_folded', true, id);
      checked(checks, 'other_node_operation_order_and_operand_positions_unchanged', true, id);
      break;
    case 'NGRAPH-PE-CASE-06':
      namedRuns(observed, input.expressions, id);
      checked(checks, 'add_result', expected.add_result, id);
      checked(checks, 'multiply_result', expected.multiply_result, id);
      checked(checks, 'reference_agrees', true, id);
      break;
    case 'NGRAPH-PE-CASE-07':
      namedRuns(observed, input.graphs, id);
      checked(checks, 'evaluators', expected.evaluators, id);
      checked(checks, 'record_compared_vectors_and_graphs', true, id);
      checked(checks, 'observations_agree', true, id);
      insist(Number.isInteger(checks.compared_vectors) && checks.compared_vectors > 0 &&
        Number.isInteger(checks.compared_graphs) && checks.compared_graphs > 0,
      `${id}: no compared vectors and graphs`);
      break;
    case 'NGRAPH-PE-CASE-08':
      checked(checks, 'fresh_subject', true, id);
      checked(checks, 'fresh_graph', true, id);
      checked(checks, 'residual_closure_matches_original', true, id);
      checked(checks, 'record_binds_both_identities', true, id);
      break;
    case 'NGRAPH-PE-CASE-09': {
      const byName = namedRuns(observed, [...input.residual_node_caps, ...input.variant_caps], id);
      for (const [variants, codes] of [[input.residual_node_caps, expected.node_cap_codes],
        [input.variant_caps, expected.variant_cap_codes]]) {
        variants.forEach((name, index) => {
          if (codes[index] === 'normal') normal(byName.get(name), name);
          else refusal(byName.get(name), codes[index], name);
        });
      }
      break;
    }
    case 'NGRAPH-PE-CASE-10':
      insist(observed.runs.some(run => run.kind === 'review'), `${id}: missing record review`);
      measurement = measurementReview(checks, expected);
      break;
    case 'NGRAPH-PE-CASE-11':
      checked(checks, 'readmitted_checked', true, id);
      checked(checks, 'observations_agree', true, id);
      insist(observed.runs.some(run => run.outcome === 'normal'), `${id}: no ordinary execution`);
      break;
    case 'NGRAPH-PE-CASE-12': {
      const byName = namedRuns(observed, input.subjects, id);
      input.subjects.forEach((name, index) => refusal(byName.get(name), expected.codes[index], name));
      for (const key of ['graph_created', 'residual_created', 'published_output']) {
        checked(checks, key, false, id);
      }
      break;
    }
    case 'NGRAPH-PE-CASE-13':
      namedRuns(observed, input.mutations, id);
      checked(checks, 'all_recorded_identities_match', true, id);
      checked(checks, 'each_mutated_record_reproduces_residual_identity', false, id);
      break;
    case 'NGRAPH-PE-CASE-14':
      namedRuns(observed, input.paired_differences, id);
      for (const key of ['residual_identities_distinct', 'binding_fields_distinct']) {
        checked(checks, key, true, id);
      }
      break;
    case 'NGRAPH-PE-CASE-15':
      assert.deepEqual([...names].sort(),
        input.tail_types.flatMap(tail => input.static_maps.map(map => `${tail}/${map}`)).sort(),
        `${id}: not every tail type/static-map pair was executed`);
      checked(checks, 'tested_tail_types', input.tail_types, id);
      checked(checks, 'tested_static_maps', input.static_maps, id);
      checked(checks, 'ordered_typed_stacks_agree', true, id);
      checked(checks, 'opaque_tail_positions_unchanged', true, id);
      checked(checks, 'ordinary_paths_selectable', true, id);
      insist(observed.runs.some(run => run.outcome === 'normal'), `${id}: no ordinary execution`);
      break;
    case 'NGRAPH-PE-CASE-16':
      namedRuns(observed, input.subjects, id);
      checked(checks, 'exact_original_closure', true, id);
      checked(checks, 'empty_residual_closure', true, id);
      checked(checks, 'fresh_identity', true, id);
      checked(checks, 'changed_definition_or_stale_pin_publishes', false, id);
      break;
    case 'NGRAPH-PE-CASE-17': {
      const byName = namedRuns(observed, input.variants, id);
      input.variants.forEach((name, index) => refusal(byName.get(name), expected.codes[index], name));
      checked(checks, 'residual_source_published', false, id);
      checked(checks, 'graph_published', false, id);
      break;
    }
    case 'NGRAPH-PE-CASE-18':
      insist(observed.runs.some(run => run.code === 'E-CLOSURE'), `${id}: no closure refusal`);
      for (const run of observed.runs) refusal(run, 'E-CLOSURE', id);
      checked(checks, 'extraction_started', false, id);
      checked(checks, 'published_output', false, id);
      break;
    default:
      throw new Error(`no validator for ${id}`);
  }
  return measurement;
}

function execute(executable, args, out, name, environment = process.env) {
  const result = spawnSync(executable, args, {
    cwd: root, env: environment, encoding: 'buffer', timeout: 3_600_000, maxBuffer: 256 * 1024 * 1024,
  });
  const stdout = result.stdout ?? Buffer.alloc(0);
  const stderr = result.stderr ?? Buffer.alloc(0);
  fs.writeFileSync(path.join(out, 'raw', `${name}.stdout`), stdout, { flag: 'wx' });
  fs.writeFileSync(path.join(out, 'raw', `${name}.stderr`), stderr, { flag: 'wx' });
  return { command: [executable, ...args], exit: result.status, signal: result.signal,
    error: result.error?.message ?? null, stdout_file: `raw/${name}.stdout`,
    stderr_file: `raw/${name}.stderr`, stdout_sha256: sha256(stdout), stderr_sha256: sha256(stderr),
    stdout: stdout.toString('utf8') };
}

function testArtifact(build, target) {
  const artifacts = build.stdout.split(/\r?\n/).filter(Boolean).flatMap(line => {
    try { return [JSON.parse(line)]; } catch { return []; }
  }).filter(entry => entry.reason === 'compiler-artifact' && entry.executable);
  const tests = [...new Set(artifacts.filter(entry => entry.target?.name === target &&
    entry.target?.kind?.includes('test')).map(entry => entry.executable))];
  assert.equal(tests.length, 1, `Cargo did not produce one ${target} integration-test executable`);
  return { executable: fs.realpathSync(tests[0]), artifacts };
}

function onePassed(command, name) {
  insist(command.exit === 0 && command.signal === null && command.error === null,
    `${name}: test executable failed (${command.exit ?? command.signal ?? command.error})`);
  insist(/test result: ok\. 1 passed; 0 failed; 0 ignored; 0 measured; \d+ filtered out/.test(command.stdout),
    `${name}: not exactly one selected passing test`);
}

function main() {
  assert.deepEqual(process.argv.slice(2, 3), ['--out'],
    'usage: NOBLE_CARGO=<absolute executable> node verification/ngraph/pe-cases.mjs --out <new external directory>');
  assert.equal(process.argv.length, 4, 'expected exactly --out and one destination');
  const cargo = process.env.NOBLE_CARGO;
  insist(cargo && path.isAbsolute(cargo), 'NOBLE_CARGO must name an absolute pinned executable');
  const pinnedCargo = fs.realpathSync(cargo);
  fs.accessSync(pinnedCargo, fs.constants.X_OK);
  const cargoDigest = digest(pinnedCargo);
  const output = path.resolve(process.argv[3]);
  insist(!fs.existsSync(output), 'receipt output directory must not already exist');
  const parent = fs.realpathSync(path.dirname(output));
  insist(parent !== root && !parent.startsWith(`${root}${path.sep}`),
    'receipt output must be outside the source worktree, including symlinked parents');
  const cases = canonicalCases();
  const before = snapshot();
  const testedCommit = git('rev-parse', 'HEAD').trim();
  const sourceTree = git('rev-parse', 'HEAD^{tree}').trim();
  fs.mkdirSync(output, { mode: 0o700 });
  fs.mkdirSync(path.join(output, 'raw'), { mode: 0o700 });
  fs.mkdirSync(path.join(output, 'tmp'), { mode: 0o700 });
  const errors = [];
  const rows = [];
  const commands = [];
  let testExecutable = null;
  let testDigest = null;
  let differentialExecutable = null;
  let differentialDigest = null;
  let refusalExecutable = null;
  let refusalDigest = null;
  const auxiliary = [];
  let cliExecutable = null;
  let cliDigest = null;
  const buildArgs = ['test', '--offline', '--locked', '-p', 'noble-cli', '--test', 'ngraph',
    '--no-run', '--message-format=json'];
  const build = execute(pinnedCargo, buildArgs, output, 'build',
    { ...process.env, NOBLE_ROOT: root, TMPDIR: path.join(output, 'tmp') });
  commands.push({ ...build, stdout: undefined });
  try {
    insist(build.error === null && build.exit === 0 && build.signal === null,
      `pinned Cargo build failed (${build.exit ?? build.signal ?? build.error})`);
    const { executable } = testArtifact(build, 'ngraph');
    testExecutable = executable;
    testDigest = digest(testExecutable);
    // Cargo's CARGO_BIN_EXE_noble path used by ngraph.rs is target/<profile>/noble.
    // A bin compiled as an additional test harness under deps/ is not that CLI.
    const sibling = path.resolve(path.dirname(testExecutable), '..', 'noble');
    insist(fs.existsSync(sibling), 'Cargo did not provide the production noble CLI');
    cliExecutable = fs.realpathSync(sibling);
    cliDigest = digest(cliExecutable);
    const differentialBuild = execute(pinnedCargo,
      ['test', '--offline', '--locked', '-p', 'noble-ngraph-experiment', '--test', 'extract_b2',
        '--no-run', '--message-format=json'], output, 'build-differential',
      { ...process.env, NOBLE_ROOT: root, TMPDIR: path.join(output, 'tmp') });
    commands.push({ ...differentialBuild, stdout: undefined });
    insist(differentialBuild.error === null && differentialBuild.exit === 0 && differentialBuild.signal === null,
      `differential test build failed (${differentialBuild.exit ?? differentialBuild.signal ?? differentialBuild.error})`);
    differentialExecutable = testArtifact(differentialBuild, 'extract_b2').executable;
    differentialDigest = digest(differentialExecutable);
    for (const name of ['model_fixtures_agree_across_residual_and_readmitted_graph',
      'seeded_core_programs_agree_across_residual_and_readmitted_graph']) {
      const test = `specialization::differential::${name}`;
      const args = ['--exact', test, '--nocapture', '--test-threads=1', '--color=never'];
      const command = execute(differentialExecutable, args, output, name,
        { ...process.env, TMPDIR: path.join(output, 'tmp') });
      commands.push({ ...command, stdout: undefined });
      onePassed(command, test);
      auxiliary.push({ test, command: command.command, exit: command.exit,
        cases: name.startsWith('model_') ? ['NGRAPH-PE-CASE-07', 'NGRAPH-PE-CASE-11']
          : ['NGRAPH-PE-CASE-07'],
        raw: { stdout: command.stdout_file, stderr: command.stderr_file,
          stdout_sha256: command.stdout_sha256, stderr_sha256: command.stderr_sha256 } });
    }
    const refusalBuild = execute(pinnedCargo,
      ['test', '--offline', '--locked', '-p', 'noble-ngraph-experiment', '--test', 'extract_b3',
        '--no-run', '--message-format=json'], output, 'build-refusals',
      { ...process.env, NOBLE_ROOT: root, TMPDIR: path.join(output, 'tmp') });
    commands.push({ ...refusalBuild, stdout: undefined });
    insist(refusalBuild.error === null && refusalBuild.exit === 0 && refusalBuild.signal === null,
      `source-refusal test build failed (${refusalBuild.exit ?? refusalBuild.signal ?? refusalBuild.error})`);
    refusalExecutable = testArtifact(refusalBuild, 'extract_b3').executable;
    refusalDigest = digest(refusalExecutable);
    const refusalTests = [
      ['pe_refusals::program_rejects_untrusted_subject_closure_and_binding_errors', 'NGRAPH-PE-CASE-18'],
      ['pe_refusals::program_refuses_effect_resource_reflection_dynamic_run_and_recursion', 'NGRAPH-PE-CASE-12'],
      ['pe_refusals::program_refuses_pre_pruning_explosion_without_constructing_a_large_chain', 'NGRAPH-PE-CASE-09'],
      ['pe_budgets::program_budget_charges_have_inclusive_observed_boundaries', 'NGRAPH-PE-CASE-09'],
      ['pe_budgets::unreachable_profile_budget_classes_are_lower_only_not_faux_exhaustion', 'NGRAPH-PE-CASE-09'],
    ];
    for (const [test, caseId] of refusalTests) {
      const args = ['--exact', test, '--nocapture', '--test-threads=1', '--color=never'];
      const command = execute(refusalExecutable, args, output, test.replaceAll('::', '-'),
        { ...process.env, TMPDIR: path.join(output, 'tmp') });
      commands.push({ ...command, stdout: undefined });
      onePassed(command, test);
      auxiliary.push({ test, cases: [caseId], command: command.command, exit: command.exit,
        raw: { stdout: command.stdout_file, stderr: command.stderr_file,
          stdout_sha256: command.stdout_sha256, stderr_sha256: command.stderr_sha256 } });
    }
    for (const [index, c] of cases.entries()) {
      const test = `pe_cases::case_${String(index + 1).padStart(2, '0')}`;
      const args = ['--exact', test, '--nocapture', '--test-threads=1', '--color=never'];
      const command = execute(testExecutable, args, output, `case-${String(index + 1).padStart(2, '0')}`,
        { ...process.env, TMPDIR: path.join(output, 'tmp') });
      commands.push({ ...command, stdout: undefined });
      if (command.error && command.exit === null && command.signal === null) {
        errors.push(`${c.id}: test executable did not start: ${command.error}`);
        continue;
      }
      const row = { id: c.id, test, harness: c.input.harness,
        case_projection_sha256: sha256(JSON.stringify(c)), variants: variantArrays(c.input),
        command: command.command, raw: { stdout: command.stdout_file, stderr: command.stderr_file,
          stdout_sha256: command.stdout_sha256, stderr_sha256: command.stderr_sha256 },
        exit: command.exit, signal: command.signal, result: 'failed', failures: [], observation: null };
      rows.push(row);
      try {
        onePassed(command, test);
        const markers = [...command.stdout.matchAll(/PE_CASE_OBSERVATION:([^\r\n]*)/g)];
        assert.equal(markers.length, 1, `${test}: expected exactly one observation marker`);
        row.observation = JSON.parse(markers[0][1]);
        row.measurement_review = verifyObservation(c, row.observation);
        row.auxiliary_tests = auxiliary.filter(aux => aux.cases.includes(c.id));
        const requiredAuxiliary = { 'NGRAPH-PE-CASE-07': 2, 'NGRAPH-PE-CASE-09': 3,
          'NGRAPH-PE-CASE-11': 1, 'NGRAPH-PE-CASE-12': 1, 'NGRAPH-PE-CASE-18': 1 }[c.id] ?? 0;
        assert.equal(row.auxiliary_tests.length, requiredAuxiliary,
          `${c.id}: required independent execution witness missing`);
        if (c.id === 'NGRAPH-PE-CASE-12') {
          row.producer_impossible_boundary = 'resource/recursion library controls have no guest-construction API';
        }
        row.result = 'passed';
      } catch (error) {
        row.failures.push(error.message);
        errors.push(`${c.id}: ${error.message}`);
      }
    }
  } catch (error) {
    errors.push(error.message);
  }
  try {
    assert.equal(git('rev-parse', 'HEAD').trim(), testedCommit, 'HEAD moved during execution');
    assert.deepEqual(snapshot(), before, 'scoped source changed during execution');
    assert.equal(digest(pinnedCargo), cargoDigest, 'pinned cargo executable changed during execution');
    if (testExecutable) assert.equal(digest(testExecutable), testDigest, 'CLI test executable changed during execution');
    if (differentialExecutable) assert.equal(digest(differentialExecutable), differentialDigest,
      'differential test executable changed during execution');
    if (refusalExecutable) assert.equal(digest(refusalExecutable), refusalDigest,
      'source-refusal test executable changed during execution');
    if (cliExecutable) assert.equal(digest(cliExecutable), cliDigest, 'production CLI changed during execution');
  } catch (error) {
    errors.push(error.message);
  }
  insist(rows.length === 0 || new Set(rows.map(row => row.id)).size === rows.length,
    'duplicate case execution row');
  const sourceRevision = `sha256:${sha256(JSON.stringify(before))}`;
  const receipt = {
    schema: 'noble-ngraph-pe-cases-execution/v1', result: errors.length === 0 && rows.length === 18 ? 'passed' : 'failed',
    failures: errors, tested_commit: testedCommit, source_tree: sourceTree,
    source_scope: sourceScope, source_revision: sourceRevision,
    source_revision_algorithm: 'sha256(JSON.stringify(source_files))',
    source_manifest_sha256: sha256(JSON.stringify(before)), source_files: before,
    canonical_case_file: caseFile, canonical_case_file_sha256: before[caseFile].sha256,
    case_projections: Object.fromEntries(cases.map(c => [c.id, sha256(JSON.stringify(c))])),
    toolchain: { cargo: { path: pinnedCargo, sha256: cargoDigest },
      test_executable: testExecutable && { path: testExecutable, sha256: testDigest },
      differential_executable: differentialExecutable &&
        { path: differentialExecutable, sha256: differentialDigest },
      refusal_executable: refusalExecutable &&
        { path: refusalExecutable, sha256: refusalDigest },
      cli_executable: cliExecutable && { path: cliExecutable, sha256: cliDigest } },
    commands, auxiliary_tests: auxiliary, cases: rows,
    non_claims: [
      'Finite executed production CLI/library test observations, not a Rust-to-model refinement proof.',
      'CASE-10 reviews the retained, source-bound historical measurement record; this run does not remeasure it.',
      'A refused producer-impossible library input is not asserted to have been produced by the public CLI.',
    ],
  };
  const receiptFile = path.join(output, 'acceptance.json');
  fs.writeFileSync(receiptFile, `${JSON.stringify(receipt, null, 2)}\n`, { flag: 'wx' });
  console.log(JSON.stringify({ result: receipt.result, tested_commit: testedCommit,
    source_revision: sourceRevision, executed_cases: rows.length,
    passed_cases: rows.filter(row => row.result === 'passed').length,
    failures: errors.length, receipt: receiptFile, receipt_sha256: digest(receiptFile) }));
  if (receipt.result !== 'passed') process.exitCode = 1;
}

main();
