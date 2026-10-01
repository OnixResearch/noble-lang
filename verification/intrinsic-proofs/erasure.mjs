import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileHash, sha256 } from './record.mjs';

const id = 'CONTRACT-22';
const modules = [
  ['proof-free-compiled-program', 'without-proof', 'extras'],
  ['proof-bearing-compiled-program', 'baseline', 'baseline'],
  ['proof-only-edited-compiled-program', 'proof-only-change', 'variants'],
];
const executableSubject = contract => ({
  definition: contract.definition,
  definition_identity: contract.definition_identity,
  definition_source_sha256: contract.definition_source_sha256,
  accepted_recipe: contract.accepted_recipe,
  generated_statement: contract.generated_statement,
});
const withoutProofTools = {
  NOBLE_LEAN: '/nonexistent/intrinsic-proof-services-disabled',
  NOBLE_CONTRACT_LIBRARY: '/nonexistent/intrinsic-proof-library-disabled',
};

function file(context, name, group = 'extras') {
  return context.runtime.asset(id, name, group);
}

function inspected(context, label, run) {
  const wasm = context.runtime.checkedWasm(label, run);
  for (const item of wasm) {
    const original = run.artifacts.find(artifact => artifact.file === item.file);
    assert.ok(original && original.bytes > 0, `${label}: empty compiled Wasm`);
    const printedCommand = context.record.receipt.commands[item.inspection_command - 1];
    assert.equal(printedCommand.id, item.inspection_command,
      `${label}: missing actual selected wasm-tools inspection`);
    const text = fs.readFileSync(path.join(context.record.output, printedCommand.stdout), 'utf8');
    assert.doesNotMatch(text, /increment-correct|poly-refl|MC1Proof|lean-exact|proof-service/iu,
      `${label}: intrinsic proof escaped into compiled Wasm`);
  }
  return wasm;
}

function exactRecipe(context, frame, label) {
  context.runtime.invocation(frame, null, label.optimization, label.name);
  assert.equal(frame.stack.length, 1, `${label.name}: wrong reflected value count`);
  const observed = frame.stack[0];
  assert.equal(observed.type, 'Syntax', `${label.name}: reflection did not return inert Syntax`);
  assert.ok(Array.isArray(observed.recipe) && observed.recipe.length === 1,
    `${label.name}: reflected named executable recipe missing`);
  const identity = observed.recipe[0].invoke;
  assert.match(identity, /^definition:[0-9]+$/u,
    `${label.name}: recipe omitted actual resolved DefinitionId`);
  assert.ok(Array.isArray(observed.witnesses), `${label.name}: typed recipe witness omitted`);
  return { definition_identity: identity,
    recipe: observed.recipe, witnesses: observed.witnesses };
}

function normal(context, run, mode, label, source, proofFree) {
  assert.equal(run.status, 0, `${label}: real framed runtime did not complete`);
  assert.equal(run.reports.length, 4, `${label}: incomplete real runtime frames`);
  const [registered, imported, number, wrapped] = run.reports;
  context.runtime.linked(registered, 'arithmetic', 1, `${label}:module`);
  context.runtime.linked(imported, 'arithmetic', 1, `${label}:import`);
  context.runtime.invocation(number, ['42'], mode, `${label}:41→42`);
  context.runtime.invocation(wrapped, ['42', '-9223372036854775808'],
    mode, `${label}:max→min`);
  assert.equal(number.runtime_prover_calls, 0, `${label}: runtime invoked proof checker`);
  assert.equal(wrapped.runtime_prover_calls, 0, `${label}: runtime invoked proof checker`);
  if (proofFree) assert.equal(registered.intrinsic_proof, null,
    `${label}: proof-free source unexpectedly carries proof evidence`);
  else {
    assert.equal(registered.intrinsic_proof?.outcome, 'proved',
      `${label}: proof-bearing module lacks independent checker evidence`);
    assert.equal(registered.intrinsic_proof?.source_sha256, fileHash(source),
      `${label}: accepted proof evidence refers to other module bytes`);
    assert.ok(registered.intrinsic_proof?.proofs?.length >= 1,
      `${label}: proof-bearing module contains no strict checked proofs`);
  }
  const wasm = inspected(context, label, run);
  const definition = registered.contracts?.find(item =>
    item.name === 'increment-law' && item.definition === 'increment');
  assert.ok(definition && typeof definition.definition_identity === 'string'
    && typeof definition.accepted_recipe === 'string',
  `${label}: no actual accepted module definition identity/recipe`);
  assert.equal(definition.module_source_sha256, fileHash(source),
    `${label}: contract metadata was not bound to submitted module bytes`);
  return { command: run.command, wasm, reports: [registered, imported, number, wrapped],
    definition };
}

function named(context, source, mode, label, proofFree) {
  const run = context.runtime.session(label,
    [source, file(context, 'import'), file(context, 'named-reflect')], mode,
    { environment: proofFree ? withoutProofTools : {} });
  assert.equal(run.status, 0, `${label}: actual named reflection failed`);
  context.runtime.linked(run.reports[0], 'arithmetic', 1, `${label}:module`);
  context.runtime.linked(run.reports[1], 'arithmetic', 1, `${label}:import`);
  const reflected = exactRecipe(context, run.reports[2],
    { optimization: mode, name: `${label}:named-reflect` });
  const wasm = inspected(context, label, run);
  return { command: run.command, reflected, wasm };
}

function ordinary(context, mode) {
  const source = file(context, 'baseline', 'baseline');
  const output = {};
  for (const [name, expected] of [['compose-run', ['42']], ['reflect', null]]) {
    const label = `C22-${name}-${mode}`;
    const run = context.runtime.session(label,
      [source, file(context, 'import'), file(context, name)], mode);
    assert.equal(run.status, 0, `${label}: ordinary quotation/compose/run failed`);
    context.runtime.linked(run.reports[0], 'arithmetic', 1, `${label}:module`);
    context.runtime.linked(run.reports[1], 'arithmetic', 1, `${label}:import`);
    if (expected) context.runtime.invocation(run.reports[2], expected, mode, label);
    else {
      context.runtime.invocation(run.reports[2], null, mode, label);
      assert.equal(run.reports[2].stack.length, 1);
      assert.equal(run.reports[2].stack[0].type, 'Syntax');
      assert.deepEqual(run.reports[2].stack[0].recipe.map(atom => atom.literal?.value
        ?? atom.invoke), ['1', 'builtin:i64.add'], `${label}: incorrect reflected ordinary recipe`);
    }
    output[name] = { command: run.command, wasm: inspected(context, label, run),
      stack: run.reports[2].stack };
  }
  return output;
}

function snapshot(context, mode) {
  const label = `C22-rejected-proof-preserves-snapshot-${mode}`;
  const run = context.runtime.session(label, [file(context, 'baseline', 'baseline'),
    file(context, 'import'), file(context,
      'proof-rejection-while-prior-module-snapshot-remains', 'variants'),
    file(context, 'import-rejected-v2'), file(context, 'invoke-41')], mode);
  assert.notEqual(run.status, 0, `${label}: hostile proof did not refuse publication`);
  assert.equal(run.reports.length, 5, `${label}: incomplete before/after frames`);
  const old = context.runtime.linked(run.reports[0], 'arithmetic', 1, `${label}:original`);
  context.runtime.linked(run.reports[1], 'arithmetic', 1, `${label}:original-import`);
  for (const index of [2, 3]) {
    const report = run.reports[index];
    context.runtime.quiet(report, `${label}:${index}`);
    assert.notEqual(report.outcome, 'linked', `${label}: rejected proof entered namespace`);
    assert.equal(report.prior_namespace, 'unchanged',
      `${label}: failed proof/import mutated live namespace`);
    assert.equal(report.resolved_module?.version ?? null, null,
      `${label}: rejected module is visible to imports`);
  }
  context.runtime.invocation(run.reports[4], ['42'], mode,
    `${label}:actual-old-snapshot-invocation`);
  const wasm = inspected(context, label, run);
  return { command: run.command, retained_identity: old,
    rejection: run.reports.slice(2, 4).map(report => ({ stage: report.stage,
      outcome: report.outcome, diagnostic: report.diagnostic,
      prior_namespace: report.prior_namespace })),
    old_stack: run.reports[4].stack, wasm };
}

function mc2Inspection(context) {
  const frozen = name => {
    const entry = context.plan.sources[`verification/mc2/contracts/${name}`];
    const selected = path.join(context.record.output, entry.snapshot);
    assert.equal(fileHash(selected), entry.sha256,
      `MC2 actual proof input changed: ${name}`);
    return selected;
  };
  const script = ['stack', `submit ${frozen('q-one.noble')}`,
    `contract ${frozen('increment.contract')} ${frozen('increment.proof.lean')}`,
    'certify 0', 'inspect 3', 'stack'];
  const input = Buffer.from(script.join('\n') + '\n');
  const observed = context.record.command('C22-explicit-MC2-evidence-inspection', context.tools.cli,
    ['companions', '--timeout-ms', '600000', '--opt', 'on'],
    { input, env: context.environment, timeout: 900_000 });
  assert.equal(observed.status, 0, 'actual MC2 companion evidence inspection failed');
  const reports = observed.bytes.toString('utf8').trimEnd().split('\n').map(JSON.parse);
  assert.equal(reports.length, script.length, 'missing companion operation response');
  reports.forEach((report, index) => {
    assert.equal(report.schema, 'noble-companions-report/v1');
    assert.equal(report.operation, script[index].split(' ')[0]);
    if (report.guest_requests !== undefined)
      assert.equal(report.guest_requests, 0, 'MC2 inspection requested guest effects');
    if (report.protected_operations !== undefined)
      assert.equal(report.protected_operations, 0, 'MC2 inspection requested protected effect');
  });
  assert.deepEqual(reports.map(report => report.outcome),
    ['reported', 'normal', 'proved', 'certified', 'inspected', 'reported'],
    'MC2 returned a different companion operation outcome');
  assert.deepEqual(reports[0].stack, [], 'MC2 inspection did not begin in fresh session');
  assert.equal(reports[2].independent_recheck, true, 'MC2 evidence was producer-only');
  assert.equal(reports[3].contract_reference, reports[2].contract_reference,
    'MC2 certification used a different contract');
  assert.equal(reports[3].evidence_reference, reports[2].evidence_reference,
    'MC2 certification used different evidence');
  assert.equal(reports[3].prover_calls, 0, 'MC2 certification re-ran the proof service');
  assert.equal(reports[4].prover_calls, 0, 'MC2 inspection ran the proof service');
  const inspection = typeof reports[4].detail === 'string'
    ? JSON.parse(reports[4].detail) : reports[4].detail;
  assert.equal(reports[4].type, 'Certified', 'inspected handle is not certified');
  assert.equal(inspection.type, 'Certified', 'inspected value is not certified metadata');
  assert.equal(inspection.contract.type, 'Contract');
  assert.equal(inspection.evidence.type, 'Evidence');
  assert.equal(inspection.evidence.index, reports[2].evidence_reference,
    'inspected metadata did not retain the checked evidence reference');
  assert.deepEqual(Object.keys(inspection.evidence).sort(),
    ['class', 'contract', 'index', 'ruleset', 'type'],
    'guest-side evidence contains proof term/capability rather than metadata');
  assert.equal(reports[4].implicit_evidence_fetches, 0,
    'explicit inspection caused an implicit evidence fetch');
  assert.equal(reports[4].authority_created, false,
    'inspection conferred host authority');
  assert.deepEqual(reports[5].stack.map(item => item.type),
    ['Program', 'Contract', 'Evidence', 'Certified'],
    'visible MC2 stack did not preserve separate program and proof metadata');
  assert.equal(reports[5].stack[3].handle, reports[3].certified_handle,
    'visible stack substituted another certified handle');
  assert.deepEqual(JSON.parse(reports[5].stack[3].detail), inspection,
    'visible stack did not retain the explicitly inspected evidence metadata');
  return { command: observed.id, operations: script,
    proof_evidence_class: reports[2].evidence_class,
    inspection_sha256: sha256(Buffer.from(JSON.stringify(inspection))),
    explicit_reference: inspection.evidence.index,
    unreported_request_counters: reports.filter(report =>
      report.guest_requests === undefined || report.protected_operations === undefined)
      .map(report => report.operation) };
}

export function erasureCase(context) {
  const positive = [], hostiles = [], modes = {};
  for (const mode of ['off', 'on']) {
    const output = {};
    for (const [label, name, group] of modules) {
      const title = `C22-${label}-${mode}`;
      const source = file(context, name, group);
      const observation = context.runtime.session(title,
        [source, file(context, 'import'), file(context, 'invoke-41'),
          file(context, 'invoke-wrap')], mode,
        { environment: label === 'proof-free-compiled-program' ? withoutProofTools : {} });
      output[label] = {
        ...normal(context, observation, mode, title, source,
          label === 'proof-free-compiled-program'),
        named: named(context, source, mode, `C22-${label}-named-${mode}`,
          label === 'proof-free-compiled-program'),
      };
      assert.equal(output[label].named.reflected.definition_identity,
        `definition:${output[label].definition.definition_identity}`,
        `${title}: host contract metadata and actual compiled DefinitionId disagree`);
    }
    const baseline = output['proof-free-compiled-program'];
    for (const [label] of modules.slice(1)) {
      const observed = output[label];
      assert.deepEqual(observed.wasm.map(item => item.sha256),
        baseline.wasm.map(item => item.sha256),
        `${mode}: proof-only difference changed compiled Wasm bytes`);
      assert.deepEqual(observed.named.reflected, baseline.named.reflected,
        `${mode}: proof changed compiled executable DefinitionId/recipe`);
      assert.deepEqual(observed.reports.slice(2).map(report => report.stack),
        baseline.reports.slice(2).map(report => report.stack),
        `${mode}: proof changed ordinary invocation observations`);
      assert.deepEqual(executableSubject(observed.definition),
        executableSubject(baseline.definition),
        `${mode}: proof changed accepted subject identity/recipe`);
      assert.notEqual(observed.definition.module_source_sha256,
        baseline.definition.module_source_sha256,
        `${mode}: proof-bearing source falsely reused proof-free module identity`);
    }
    for (const [label] of modules) if (mode === 'off') {
      const row = output[label];
      positive.push({ name: label, command: row.command, outcome: 'normal',
        proof_mode: label === 'proof-free-compiled-program' ? 'none' : 'independent-Lean',
        definition: row.named.reflected, baseline_wasm: row.wasm });
    }
    const surviving = snapshot(context, mode);
    if (mode === 'off') hostiles.push({ name: 'proof-rejection-while-prior-module-snapshot-remains',
      command: surviving.command, outcome: 'rejected-with-old-definition-retained',
      prior_identity: surviving.retained_identity });
    modes[mode] = { ordinary: ordinary(context, mode),
      baseline_proof_variants: output, refused: surviving };
  }
  const mc2 = mc2Inspection(context);
  return { id, passed: true, positive, hostiles,
    details: { modes, explicit_MC2_evidence_inspection: mc2,
      ordinary_quote_compose_run_reflect: ['reflect', 'compose-run'],
      proof_artifact_metadata_separate: true,
      release_owner_law_proved: false } };
}
