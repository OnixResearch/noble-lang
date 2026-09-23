#!/usr/bin/env bun
// Execute every canonical MC2 case. Reports, scripts, input bytes and subprocess
// outcomes are retained; a missing observation or variant is a failure.
// Usage: bun verification/mc2/gate.mjs [BINARY] [DESTINATION]
// Optional NOBLE_MC2_CORE_TEST_BINARY selects an already-built CLI test
// executable. Otherwise the one structured core-control test is run via cargo.

import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { createHash } from 'node:crypto';
import { chmod, copyFile, mkdir, mkdtemp, readFile, stat, symlink, unlink, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, isAbsolute, join, resolve } from 'node:path';
import { createInterface } from 'node:readline';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const suppliedBinary = resolve(process.argv[2] ?? process.env.NOBLE_BIN ?? join(root, 'target/debug/noble'));
const destination = process.argv[3] ? resolve(process.argv[3])
  : join(await mkdtemp(join(tmpdir(), 'noble-mc2-acceptance-')), 'acceptance.json');
await mkdir(dirname(destination), { recursive: true });
const workspace = await mkdtemp(join(dirname(destination), 'mc2-artifacts-'));
const fixtures = join(root, 'verification/mc2/contracts');
const canonicalPath = join(root, 'specs/conformance/contract-cases.json');
const canonicalBytes = await readFile(canonicalPath);
const canonical = JSON.parse(canonicalBytes);
const cases = new Map(canonical.cases.map(item => [item.id, item]));
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const frozenExecutables = [];
async function freezeExecutable(source, name) {
  const executable = join(workspace, name);
  await copyFile(source, executable);
  await chmod(executable, 0o500);
  const sha256 = hash(await readFile(executable));
  frozenExecutables.push({ supplied: source, executed: executable, sha256_at_start: sha256 });
  return executable;
}
const binary = await freezeExecutable(suppliedBinary, 'noble-executed');
let controlBinary = process.env.NOBLE_MC2_CORE_TEST_BINARY
  ? await freezeExecutable(resolve(process.env.NOBLE_MC2_CORE_TEST_BINARY), 'controls-executed')
  : null;
const proofTimeout = 600000;
const reviewed = {
  NOBLE_LEAN: '/nix/store/hbxwgcbpkyqllqzq7cidhnvhgb60jb9q-noble-lean-release-4.31.0/bin/lean',
  NOBLE_BWRAP: '/nix/store/lqndphylsxqwbwm804n473pb4sqb98sh-bubblewrap-0.11.2/bin/bwrap',
  NOBLE_PRLIMIT: '/nix/store/mqvbf0flamqaq9c3ihb496aahag897n0-util-linux-2.42.3-bin/bin/prlimit',
  NOBLE_SYSTEMD_RUN: '/nix/store/baxgs06659w4i6ggsfr56srjrb52v1h8-systemd-261.2/bin/systemd-run',
  NOBLE_CONTRACT_LIBRARY: join(root, 'proofs/mc1'),
};
const environment = { ...process.env };
for (const [key, value] of Object.entries(reviewed)) environment[key] ??= value;
const acceptance = {
  schema: 'noble-mc2-acceptance/v1', binary, supplied_binary: suppliedBinary,
  artifact_directory: workspace, frozen_executables: frozenExecutables,
  environment: Object.fromEntries([...Object.keys(reviewed), 'XDG_RUNTIME_DIR', 'NOBLE_MC2_CORE_TEST_BINARY']
    .map(key => [key, environment[key] ?? null])),
  conformance: { path: canonicalPath, sha256: hash(canonicalBytes),
    workload: canonical.cases.map(({ id, input, expected }) => ({ id, input, expected })) },
  cases: [], commands: [], inputs: {}, executables: {},
};
const retained = new Set([canonicalPath, fileURLToPath(import.meta.url)]);
const fixture = name => { const path = join(fixtures, name); retained.add(path); return path; };
const mc1 = name => { const path = join(root, 'verification/mc1', name); retained.add(path); return path; };
let serial = 0;
async function source(name, content) {
  const path = join(workspace, `${++serial}-${name}`);
  await writeFile(path, content, { flag: 'wx' });
  retained.add(path);
  return path;
}
function check(row, label, observed, expected) {
  const observation = { label, observed, expected, passed: false };
  row.assertions.push(observation);
  try { assert.deepStrictEqual(observed, expected); observation.passed = true; }
  catch (error) { observation.failure = error.message; }
  return observation.passed;
}
function processRecord(row, executable, args, options) {
  const result = { id: ++serial, executable, args, cwd: root, env: options.env ?? {},
    input: options.input ?? null, exit: null, signal: null, stdout: '', stderr: '', reports: [] };
  acceptance.commands.push(result);
  row.evidence.push({ command: result.id });
  return result;
}
async function command(row, args, options = {}) {
  const executable = options.executable ?? binary;
  const result = processRecord(row, executable, args, options);
  await new Promise(resolveCommand => {
    const child = spawn(executable, args, { cwd: root, env: { ...environment, ...options.env },
      stdio: ['pipe', 'pipe', 'pipe'] });
    const timeout = setTimeout(() => {
      result.error = 'gate subprocess deadline exceeded'; child.kill('SIGKILL');
    }, options.timeout ?? proofTimeout + 120000);
    let bytes = 0;
    for (const [stream, key] of [[child.stdout, 'stdout'], [child.stderr, 'stderr']]) {
      stream.on('data', chunk => {
        bytes += chunk.length;
        if (bytes > 67108864) { result.error = 'gate output bound exceeded'; child.kill('SIGKILL'); }
        else result[key] += chunk.toString();
      });
    }
    child.once('error', error => { result.error = String(error); });
    child.once('close', (exit, signal) => {
      clearTimeout(timeout); Object.assign(result, { exit, signal }); resolveCommand();
    });
    child.stdin.on('error', error => { result.stdin_error = String(error); });
    child.stdin.end(options.input);
  });
  check(row, `command ${result.id} completes without host failure`, result.error ?? null, null);
  check(row, `command ${result.id} exits without a signal`, result.signal, null);
  return result;
}
function jsonReport(row, result) {
  const texts = result.stdout.split('\n').filter(line => line.trim());
  check(row, `command ${result.id} emits exactly one report`, texts.length, 1);
  const report = JSON.parse(texts[0] ?? 'null');
  result.reports = [report];
  return report;
}
const decode = value => typeof value === 'string' ? JSON.parse(value) : value;
const scalar = value => ({ type: 'I64', value: String(value) });
const output = report => decode(report.output);
function scalarStack(row, label, stack) {
  check(row, `${label}: runtime stack is present`, Array.isArray(stack), true);
  if (!Array.isArray(stack)) return null;
  check(row, `${label}: scalar entries carry no cell handles`, stack.map(value => value.handle), stack.map(() => null));
  return stack.map(({ handle, ...value }) => value);
}
const diagnosticCodes = report => (report.diagnostics ?? []).map(item => item.code);
const exitFor = { proved: 0, disproved: 1, error: 2, unknown: 3, unsupported: 4, timeout: 124, 'not-run': 0 };
function quiet(row, label, report) {
  check(row, `${label}: guest requests`, report.guest_requests, 0);
  check(row, `${label}: protected operations`, report.protected_operations, 0);
}
function independentProof(row, report, label = 'proof') {
  check(row, `${label}: outcome`, report.outcome, 'proved');
  check(row, `${label}: independent kernel replay`, report.independent_recheck, true);
  check(row, `${label}: claim kind`, report.claim_kind, 'partial-correctness');
}

// Drive one operation at a time. This allows real tool/library access to be
// revoked after admission and avoids relying on incidental guest stack indexes.
async function withSession(row, action, options = {}) {
  const directory = await mkdtemp(join(workspace, 'session-'));
  const lean = join(directory, 'lean'), library = join(directory, 'library');
  await symlink(environment.NOBLE_LEAN, lean);
  await symlink(environment.NOBLE_CONTRACT_LIBRARY, library);
  const args = ['companions', '--timeout-ms', String(proofTimeout), '--opt', options.optimized ? 'on' : 'off'];
  const overrides = { NOBLE_LEAN: lean, NOBLE_CONTRACT_LIBRARY: library };
  const result = processRecord(row, binary, args, { env: overrides, input: '' });
  result.operations = [];
  const child = spawn(binary, args, { cwd: root, env: { ...environment, ...overrides },
    stdio: ['pipe', 'pipe', 'pipe'] });
  let pending = null, closed = false, bytes = 0;
  const timeout = setTimeout(() => {
    result.error = 'companion session deadline exceeded'; child.kill('SIGKILL');
  }, 8 * (proofTimeout + 120000));
  const ended = new Promise(resolveEnd => {
    child.once('error', error => { result.error = String(error); });
    child.once('close', (exit, signal) => {
      closed = true; clearTimeout(timeout); Object.assign(result, { exit, signal });
      if (pending) { pending.reject(new Error(result.error ?? 'session closed before its report')); pending = null; }
      resolveEnd();
    });
  });
  child.stdout.on('data', chunk => {
    bytes += chunk.length;
    if (bytes > 67108864) { result.error = 'session output bound exceeded'; child.kill('SIGKILL'); }
    else result.stdout += chunk.toString();
  });
  child.stderr.on('data', chunk => { result.stderr += chunk.toString(); });
  child.stdin.on('error', error => { result.stdin_error = String(error); });
  const lines = createInterface({ input: child.stdout });
  lines.on('line', text => {
    try {
      const report = JSON.parse(text);
      result.reports.push(report);
      if (!pending) throw new Error('unsolicited companion report');
      const request = pending; pending = null;
      request.resolve(report);
    } catch (error) {
      result.error = String(error);
      if (pending) { pending.reject(error); pending = null; }
      child.kill('SIGKILL');
    }
  });
  const session = {
    result, proof_services_disabled: false,
    async call(line) {
      assert.equal(closed, false, 'cannot use an ended companion session');
      assert.equal(pending, null, 'only one companion request may be outstanding');
      result.input += `${line}\n`;
      const report = await new Promise((resolveReply, reject) => {
        pending = { resolve: resolveReply, reject };
        child.stdin.write(`${line}\n`);
      });
      result.operations.push({ input: line, report });
      check(row, `${line}: report schema`, report.schema, 'noble-companions-report/v1');
      check(row, `${line}: operation`, report.operation, line.split(' ')[0]);
      return report;
    },
    async disableProofServices() {
      await unlink(lean); await unlink(library);
      const absent = [];
      for (const path of [lean, library]) {
        try { await stat(path); absent.push(false); }
        catch (error) { absent.push(error.code === 'ENOENT'); }
      }
      check(row, 'selected proof tool and rule library really removed', absent, [true, true]);
      result.proof_service_revocation = { after_report: result.reports.length, paths: [lean, library], absent };
      session.proof_services_disabled = absent.every(Boolean);
    },
    async proveServicesUnavailable() {
      const refusal = await session.call(`contract ${fixture('increment.contract')} ${fixture('increment.proof.lean')}`);
      check(row, 'disabled proof service refuses an explicit later proof attempt', refusal.outcome, 'unsupported');
      check(row, 'disabled proof service cannot certify a claim', refusal.evidence_reference ?? null, null);
      quiet(row, 'disabled proof service probe', refusal);
    },
    async slot(type, fromEnd = 0) {
      const report = await session.call('stack');
      const indexes = report.stack.flatMap((entry, index) => entry.type === type ? [index] : []);
      assert.ok(indexes.length > fromEnd, `no live ${type} slot ${fromEnd}`);
      return indexes[indexes.length - 1 - fromEnd];
    },
  };
  try {
    const initial = await session.call('stack');
    check(row, 'initial uncompiled session remains usable', initial.outcome, 'reported');
    check(row, 'initial uncompiled session has an empty stack', initial.stack, []);
    return await action(session);
  }
  finally {
    child.stdin.end(); await ended; lines.close();
    check(row, 'companion script exit', result.exit, 0);
    check(row, 'companion script signal', result.signal, null);
    check(row, 'companion script host error', result.error ?? null, null);
    check(row, 'one complete report for every submitted operation', result.reports.length, result.operations.length);
  }
}
async function admit(row, session, contract = 'increment.contract', proof = 'increment.proof.lean') {
  const report = await session.call(`contract ${fixture(contract)} ${fixture(proof)}`);
  independentProof(row, report, contract);
  check(row, `${contract}: exact evidence class`, report.evidence_class, 'lean-exact');
  return report;
}
async function certify(row, session, index, contractIndex) {
  const report = await session.call(`certify ${index}${contractIndex === undefined ? '' : ` ${contractIndex}`}`);
  check(row, 'exact subject certification', report.outcome, 'certified');
  return report;
}
async function runCase(id, action) {
  const item = cases.get(id);
  const row = { case: id, name: item?.input.harness, input: item?.input, expected: item?.expected,
    observed: null, passed: false, assertions: [], evidence: [] };
  acceptance.cases.push(row);
  try {
    assert.ok(item, `canonical case ${id} is absent`);
    row.observed = await action(row, item);
    check(row, 'complete canonical expected object', row.observed, item.expected);
  } catch (error) { row.failure = error.stack ?? String(error); }
  row.passed = !row.failure && row.assertions.length > 0 && row.assertions.every(item => item.passed);
  await writeFile(join(workspace, `${id}.json`), `${JSON.stringify({
    case: row,
    commands: row.evidence.map(item => acceptance.commands.find(command => command.id === item.command)),
  }, null, 2)}\n`);
  console.log(`${row.passed ? 'PASS' : 'FAIL'} ${id}${row.failure ? `: ${row.failure.split('\n')[0]}` : ''}`);
}
function variantSet(row, variants) {
  row.variants = variants;
  const names = variants.map(item => item.variant);
  const expected = row.input.variants.map(item => typeof item === 'string' ? item : item.case);
  check(row, 'exact canonical variant set (including multiplicity)', [...names].sort(), [...expected].sort());
  return names.length === expected.length && new Set(names).size === expected.length;
}

let coreRun;
async function coreControls(row, id) {
  if (!coreRun) {
    retained.add(join(root, 'crates/noble-cli/src/core/companions/controls.rs'));
    const receipt = join(workspace, 'core-controls.json');
    const test = 'core::companions::controls::exercise';
    if (controlBinary === null) {
      const built = await command(row, ['test', '-p', 'noble-cli', '--bin', 'noble', '--no-run', '--message-format=json'], {
        executable: process.env.CARGO ?? 'cargo', timeout: 3600000,
        env: { CARGO_TARGET_DIR: join(workspace, 'cargo-target') },
      });
      check(row, 'structured control compilation succeeds', built.exit, 0);
      built.reports = built.stdout.split('\n').filter(line => line.trim()).map(line => JSON.parse(line));
      const artifacts = built.reports.filter(report => report.reason === 'compiler-artifact'
        && report.target?.name === 'noble' && report.profile?.test === true && report.executable);
      assert.equal(artifacts.length, 1, 'cargo must identify exactly one CLI unit-test executable');
      controlBinary = await freezeExecutable(artifacts[0].executable, 'controls-executed');
    }
    const run = await command(row, [test, '--exact', '--nocapture'], { executable: controlBinary, timeout: 3600000,
      env: { NOBLE_MC2_CONTROL_RECEIPT: receipt, NOBLE_MC2_CHECKER_BINARY: binary } });
    coreRun = { run, receipt: null, error: null };
    try {
      coreRun.receipt = JSON.parse(await readFile(receipt, 'utf8'));
      retained.add(receipt);
      for (const checked of coreRun.receipt.checker_runs ?? []) {
        const report = JSON.parse(checked.report_json);
        checked.report = report;
        if (report.source_path) {
          retained.add(report.source_path);
          retained.add(join(dirname(report.source_path), 'proof.lean'));
        }
      }
      acceptance.core_controls = { path: receipt, receipt: coreRun.receipt };
    }
    catch (error) { coreRun.error = String(error); }
  } else row.evidence.push({ command: coreRun.run.id });
  check(row, 'focused structured Rust harness exit', coreRun.run.exit, 0);
  check(row, 'fresh core receipt exists', coreRun.error, null);
  check(row, 'core receipt schema', coreRun.receipt?.schema, 'noble-mc2-core-controls/v1');
  const controls = coreRun.receipt?.controls?.filter(control => control.case === id) ?? [];
  variantSet(row, controls);
  return controls;
}
function coreRefusal(row, control, expectedDiagnostic) {
  const label = control.variant;
  check(row, `${label}: core tier has no guest execution capability`, control.execution_capability, 'none');
  check(row, `${label}: exact refusal`, control.observed?.diagnostic, expectedDiagnostic);
  check(row, `${label}: no certified result`, control.observed?.new_certified_status, false);
  check(row, `${label}: actual API inputs retained`, !!control.input && Object.keys(control.input).length > 0, true);
}
function coreBaseline(row, control) {
  const baseline = coreRun.receipt?.checker_runs?.[control.baseline?.checker_report];
  check(row, `${control.variant}: actual independent baseline report retained`, baseline?.report?.schema, 'noble-mc1-report/v1');
  check(row, `${control.variant}: actual baseline process exit`, baseline?.exit, 0);
  check(row, `${control.variant}: actual baseline proof outcome`, baseline?.report?.outcome, 'proved');
  check(row, `${control.variant}: actual independent kernel check`, baseline?.report?.independent_recheck, true);
  check(row, `${control.variant}: actual declaration data retained`, typeof baseline?.report?.proof_declarations, 'string');
}

await runCase('CONTRACT-01', async (row, item) => {
  const verify = await command(row, ['verify', fixture('increment.contract'), '--proof', fixture('increment.proof.lean'), '--timeout-ms', String(proofTimeout)]);
  const proof = jsonReport(row, verify);
  check(row, 'exact proof process exit', verify.exit, 0);
  independentProof(row, proof);
  check(row, 'independently consumed declarations retained', typeof proof.proof_declarations, 'string');
  const result = await withSession(row, async session => {
    await session.call(`submit ${fixture('q-one.noble')}`);
    const admitted = await admit(row, session);
    const bound = await certify(row, session, 0);
    const metadata = decode((await session.call(`inspect ${await session.slot('Certified')}`)).detail);
    check(row, 'companion retains partial-correctness claim kind', metadata.contract.claim_kind, 1);
    await session.disableProofServices();
    for (const vector of item.input.vectors) {
      const program = await source('increment-vector.noble', `${vector.input.value} [ 1 + ] run\n`);
      const before = await session.call('stack');
      const report = await session.call(`plain ${program}`);
      check(row, `vector ${vector.input.value}: normal return`, report.outcome, 'normal');
      check(row, `vector ${vector.input.value}: exact output`, output(report), vector.output);
      check(row, `vector ${vector.input.value}: entire stack tail unchanged`, report.stack.slice(0, -1), before.stack);
    }
    await session.proveServicesUnavailable();
    return { admitted, bound, metadata };
  });
  return { stage: 'contract-proof', outcome: result.bound.outcome === 'certified' && proof.outcome === 'proved' ? 'proved-for-exact-subject' : proof.outcome,
    lean_kernel_check_required: proof.independent_recheck && result.admitted.independent_recheck,
    total_correctness_claimed: proof.claim_kind !== 'partial-correctness',
    vectors_establish_universal_proof: result.admitted.evidence_class !== 'lean-exact',
    export_correspondence_required: result.bound.contract_reference === result.admitted.contract_reference
      && result.bound.evidence_reference === result.admitted.evidence_reference };
});

await runCase('CONTRACT-02', async row => {
  const controls = await coreControls(row, row.case);
  const reasons = {
    'captured-value': 'mismatched-subject', 'program-operand': 'mismatched-subject',
    'instantiated-interface': 'mismatched-subject', 'logical-definition': 'mismatched-claim',
    'host-contract': 'stale-context', 'semantic-revision': 'stale-context',
    'consumer-policy': 'stale-context', 'current-environment-fact': 'stale-context',
  };
  for (const control of controls) {
    coreRefusal(row, control, reasons[control.variant]);
    coreBaseline(row, control);
    check(row, `${control.variant}: original proof independently accepted`, control.baseline?.independent_recheck, true);
    check(row, `${control.variant}: original subject accepted`, control.baseline?.accepted, true);
    if (['captured-value', 'host-contract', 'semantic-revision', 'consumer-policy', 'current-environment-fact'].includes(control.variant)) {
      check(row, `${control.variant}: explicit revalidation restores applicability`, control.observed?.revalidated, true);
    }
    if (control.variant === 'semantic-revision') {
      check(row, 'old-model proof cannot revalidate unsupported current semantics',
        control.observed?.revalidation_while_unsupported, 'unsupported-semantic-revision');
    }
  }
  const rejected = controls.length === 8 && controls.every(control => control.observed?.new_certified_status === false);
  return { stage: 'evidence-admission', outcome: rejected ? 'reject-each-variant-until-revalidated' : 'incomplete-binding-refusals',
    new_certified_status: controls.some(control => control.observed?.new_certified_status !== false),
    old_theorem_automatically_false: controls.some(control => control.baseline?.accepted !== true),
    guest_requests: controls.every(control => control.execution_capability === 'none') ? 0 : null,
    protected_operations: controls.every(control => control.execution_capability === 'none') ? 0 : null };
});

await runCase('CONTRACT-03', async (row, item) => withSession(row, async session => {
  await session.call(`submit ${fixture('q-one.noble')}`);
  await session.call(`submit ${fixture('q-one.noble')}`);
  await admit(row, session);
  await certify(row, session, 0);
  const left = await session.slot('Certified');
  await certify(row, session, 1);
  const right = await session.slot('Certified');
  await session.disableProofServices();
  const composed = await session.call(`compose ${left} ${right} ${item.input.argument.value}`);
  check(row, 'composed wrapping result', output(composed), item.expected.output);
  check(row, 'derived increment theorem parameter', composed.increment_by, '2');
  await session.proveServicesUnavailable();
  return { stage: 'normal-return', outcome: composed.outcome, output: output(composed),
    postcondition: `output = wrap64(input + ${composed.increment_by})`, claim_kind: composed.claim_kind,
    prover_calls: composed.prover_calls, candidate_prepare_requests: composed.candidate_prepare_requests,
    assumptions_dropped: composed.assumptions_dropped,
    underlying_recipe_matches_plain_compose: composed.underlying_recipe_matches_plain_compose };
}));

await runCase('CONTRACT-04', async (row, item) => withSession(row, async session => {
  await session.call(`submit ${fixture('q-one.noble')}`);
  await session.call(`submit ${fixture('q-one.noble')}`);
  const leftContract = await admit(row, session);
  await certify(row, session, 0, leftContract.contract_reference);
  const left = await session.slot('Certified');
  const rightContract = await admit(row, session, 'nonnegative.contract', 'nonnegative.proof.lean');
  await certify(row, session, 1, rightContract.contract_reference);
  const right = await session.slot('Certified');
  const before = await session.call('stack');
  await session.disableProofServices();
  const composed = await session.call(`compose ${left} ${right} ${item.input.counterexample_initial_input.value}`);
  check(row, 'missing implication has its own refusal', composed.diagnostic?.code, 'unresolved-implication');
  const after = await session.call('stack');
  check(row, 'failed composition mints no companion', after.stack.filter(entry => entry.type === 'Certified').length,
    before.stack.filter(entry => entry.type === 'Certified').length);
  const intermediate = await source('negative-intermediate.noble', '-2 [ 1 + ] run\n');
  const ordinary = await source('ordinary-composition.noble', '-2 [ 1 + ] [ 1 + ] compose run\n');
  const first = await session.call(`plain ${intermediate}`);
  check(row, 'canonical counterexample intermediate result', output(first), item.input.intermediate_output);
  check(row, 'right input precondition is false at intermediate result', BigInt(output(first).value) >= 0n, false);
  const plain = await session.call(`plain ${ordinary}`);
  check(row, 'ordinary matching-interface composition runs', plain.outcome, 'normal');
  check(row, 'ordinary composition counterexample output', output(plain), scalar(0));
  await session.proveServicesUnavailable();
  return { stage: 'companion-composition', outcome: composed.outcome,
    ordinary_composition_valid: composed.ordinary_composition_valid && plain.outcome === 'normal',
    new_certified_status: composed.new_certified_status, guest_requests: composed.guest_requests,
    protected_operations: composed.protected_operations };
}));

await runCase('CONTRACT-05', async (row, item) => withSession(row, async session => {
  await session.call(`submit ${fixture('builder.noble')}`);
  const family = await session.call(`contract ${mc1('family.noble-contract')} ${mc1('family-proof.lean')}`);
  independentProof(row, family, 'universally quantified runtime builder family');
  await certify(row, session, 0);
  const builder = await session.slot('Certified');
  await session.disableProofServices();
  const instances = [];
  for (const instance of item.input.instances) {
    const report = await session.call(`instantiate ${builder} ${instance.capture.value} ${instance.argument.value}`);
    instances.push(report);
    check(row, `capture ${instance.capture.value}: certified instance`, report.outcome, 'each-instance-bound-and-certified');
    check(row, `capture ${instance.capture.value}: exact argument`, report.argument, instance.argument.value);
    check(row, `capture ${instance.capture.value}: retained capture`, report.capture, instance.capture.value);
    check(row, `capture ${instance.capture.value}: exact output`, output(report), instance.output);
  }
  check(row, 'all canonical runtime captures executed', instances.length, item.input.instances.length);
  await session.proveServicesUnavailable();
  return { stage: 'normal-return', outcome: instances.every(report => report.outcome === 'each-instance-bound-and-certified') ? 'each-instance-bound-and-certified' : 'instance-failure',
    same_compiled_builder: instances.every(report => report.same_compiled_builder === true)
      && new Set(instances.map(report => report.builder_handle)).size === 1,
    program_value_identity_relation: new Set(instances.map(report => report.program_identity)).size === instances.length ? 'different' : 'same',
    prover_calls: instances.reduce((sum, report) => sum + report.prover_calls, 0),
    candidate_prepare_requests: instances.reduce((sum, report) => sum + report.candidate_prepare_requests, 0),
    literal_specialization_sufficient: instances.some(report => report.literal_specialization_sufficient !== false) };
}));

await runCase('CONTRACT-06', async row => {
  const controls = await coreControls(row, row.case);
  const reasons = { 'serialized-certified-true': 'forged-status', 'forged-constructor-tag': 'unsupported-evidence-class',
    'unknown-rule': 'unknown-rule', 'cyclic-derivation': 'cyclic-derivation', 'missing-premise': 'missing-premise',
    'wrong-family-instantiation': 'wrong-instantiation', 'exhausted-replay-budget': 'exhausted-replay' };
  for (const control of controls) {
    coreRefusal(row, control, reasons[control.variant]);
    coreBaseline(row, control);
  }
  return { stage: 'evidence-admission', outcome: controls.length === 7 && controls.every(control => control.observed?.new_certified_status === false) ? 'reject-each-variant' : 'incomplete-forgery-refusals',
    new_certified_status: controls.some(control => control.observed?.new_certified_status !== false),
    guest_requests: controls.every(control => control.execution_capability === 'none') ? 0 : null,
    protected_operations: controls.every(control => control.execution_capability === 'none') ? 0 : null };
});

for (const id of ['CONTRACT-07', 'CONTRACT-08']) await runCase(id, async (row, item) => withSession(row, async session => {
  await session.call(`submit ${fixture('q-one.noble')}`);
  const proof = await admit(row, session, 'guarded.contract', 'guarded.proof.lean');
  const certified = await certify(row, session, 0);
  const slot = await session.slot('Certified');
  const before = decode((await session.call(`inspect ${slot}`)).detail);
  await session.disableProofServices();
  const invoked = await session.call(`invoke ${slot} ${item.input.argument.value}`);
  const after = decode((await session.call(`inspect ${slot}`)).detail);
  check(row, 'guard invocation preserves exact companion payload', after, before);
  check(row, 'guard invocation uses retained subject handle', invoked.subject_handle, certified.subject_handle);
  check(row, 'guard wrapper executed', invoked.wrapper_executions, 1);
  check(row, 'invocation retains trusted artifact correspondence', invoked.artifact_correspondence, 'recorded-trusted-build');
  check(row, 'host authority is checked separately from proof', invoked.host_authority, 'independently-checked-resource-free');
  await session.proveServicesUnavailable();
  if (id === 'CONTRACT-07') return { stage: 'applicability', outcome: invoked.outcome,
    conditional_theorem_valid: proof.independent_recheck && invoked.conditional_theorem_valid,
    candidate_body_started: invoked.candidate_body_started, guest_requests: invoked.guest_requests,
    protected_operations: invoked.protected_operations };
  check(row, 'true guard enters candidate body', invoked.candidate_body_started, true);
  return { stage: 'normal-return', outcome: invoked.outcome, output: output(invoked), prover_calls: invoked.prover_calls,
    verified_backend_claimed: invoked.verified_backend_claimed, guard_erased: invoked.guard_erased,
    underlying_program_identity_changed: invoked.underlying_program_identity_changed };
}));

async function requiredBuild(row, name, sourcePath, contract, proofArgs = [], options = {}) {
  const emitted = options.destination ?? join(workspace, `${++serial}-${name}-build`);
  const args = ['build', '--require-proof', sourcePath, '--contract', contract, ...proofArgs,
    '--timeout-ms', String(options.proofTimeout ?? proofTimeout), '--out', emitted];
  const run = await command(row, args, { env: options.env });
  const report = jsonReport(row, run);
  check(row, `${name}: outcome exit mapping`, run.exit, exitFor[report.outcome]);
  check(row, `${name}: build report schema`, report.schema, 'noble-mc2-build/v1');
  return { run, report, emitted };
}
async function runtimeFuel(row) {
  const path = fixture('vector-41.noble');
  const compiled = await command(row, ['compile', path]);
  check(row, 'fuel witness compiles before execution', compiled.exit, 0);
  const configPath = join(root, 'crates/noble-cli/src/core/runtime/config.json');
  const abiPath = join(root, 'crates/noble-cli/src/core/runtime/abi.json');
  retained.add(configPath); retained.add(abiPath);
  const config = JSON.parse(await readFile(configPath));
  const wat = await source('fuel-runtime.wat', compiled.stdout);
  const adapter = join(root, 'verification/mc2/runtime-controls.mjs');
  retained.add(adapter);
  const run = await command(row, [...config.node_flags, adapter, wat, path, workspace], { executable: config.tools.node.path });
  const receipt = jsonReport(row, run);
  check(row, 'runtime fuel adapter process exit', run.exit, 0);
  check(row, 'runtime fuel adapter schema', receipt.schema, 'noble-mc2-runtime-fuel/v1');
  check(row, 'same compiled module normally returns before fuel control', receipt.baseline?.outcome, 'normal');
  check(row, 'same compiled module has the expected normal result',
    scalarStack(row, 'fuel baseline', receipt.baseline?.stack), [scalar(42)]);
  const observation = { operation: 'CoreEngine.execute', input: receipt.input,
    limits: receipt.limits, command: run.id, baseline: receipt.baseline, observed: receipt.exhausted };
  row.runtime_fuel = observation;
  check(row, 'fuel witness reaches Wasm execution', observation.observed.stage, 'wasm');
  check(row, 'fuel witness really exhausts runtime fuel', observation.observed.outcome, 'runtime-exhausted');
  check(row, 'fuel witness is not a static byte-limit failure', observation.observed.quota_reason, 'execution work');
  quiet(row, 'runtime fuel witness', observation.observed);
  return observation;
}
await runCase('CONTRACT-09', async (row, item) => {
  const bare = await source('bare-solver-answer.lean', '');
  const exhausted = await runtimeFuel(row);
  const exhaustedSource = await source('execution-fuel-no-declaration.lean', '');
  row.exhausted_attempt = {
    runtime_observation: exhausted,
    offered_declaration: exhaustedSource,
    evidence_class: 'unsuccessful-runtime-attempt-not-a-Lean-declaration',
  };
  const attempts = {
    'accepted-exact-proof': { contract: fixture('increment.contract'), args: ['--proof', fixture('increment.proof.lean')] },
    'checked-refutation': { contract: mc1('monotonic.noble-contract'), args: ['--refutation', mc1('monotonic-refutation.lean')] },
    'bare-solver-answer': { contract: fixture('increment.contract'), args: ['--proof', bare] },
    'rejected-proof-term': { contract: fixture('increment.contract'), args: ['--proof', fixture('composed.proof.lean')] },
    'unfinished-or-sorry-proof': { contract: fixture('increment.contract'), args: ['--proof', fixture('sorry.proof.lean')] },
    'proof-timeout': { contract: fixture('increment.contract'), args: ['--proof', fixture('increment.proof.lean')], proofTimeout: 1 },
    'unsupported-predicate': { contract: fixture('unsupported-host.contract'), args: ['--proof', fixture('increment.proof.lean')] },
    'no-attempt': { contract: fixture('increment.contract'), args: [] },
    'execution-fuel-exhausted': { contract: fixture('increment.contract'), args: ['--proof', exhaustedSource] },
  };
  check(row, 'exact canonical attempt set', Object.keys(attempts).sort(), item.input.attempts.map(attempt => attempt.case).sort());
  row.attempts = [];
  for (const expected of item.input.attempts) {
    const attempt = attempts[expected.case];
    const { run, report } = await requiredBuild(row, expected.case, fixture('q-one.noble'), attempt.contract, attempt.args, attempt);
    row.attempts.push({ case: expected.case, report, command: run.id });
    check(row, `${expected.case}: distinct proof outcome`, report.outcome, expected.outcome);
    check(row, `${expected.case}: release policy`, report.release?.allowed, expected.release_allowed);
    check(row, `${expected.case}: exact process exit`, run.exit, exitFor[expected.outcome]);
    if (['proved', 'disproved'].includes(expected.outcome)) check(row, `${expected.case}: independent recheck`, report.evidence?.independent_recheck, true);
  }
  const invalid = await source('invalid-artifact.noble', 'true 1 +\n');
  const occupied = await mkdtemp(join(workspace, 'occupied-build-destination-'));
  row.build_controls = [];
  for (const [name, sourcePath, options] of [
    ['static-invalid-artifact', invalid, {}],
    ['occupied-artifact-destination', fixture('q-one.noble'), { destination: occupied }],
  ]) {
    const result = await requiredBuild(row, name, sourcePath, fixture('increment.contract'), ['--proof', fixture('increment.proof.lean')], options);
    row.build_controls.push({ name, report: result.report, command: result.run.id });
    check(row, `${name}: valid proof cannot release unavailable artifact`, result.report.release?.allowed, false);
    check(row, `${name}: artifact absence diagnosed`, diagnosticCodes(result.report).length > 0, true);
  }
  const reports = Object.fromEntries(row.attempts.map(attempt => [attempt.case, attempt.report]));
  const matched = item.input.attempts.every(attempt => reports[attempt.case].outcome === attempt.outcome
    && reports[attempt.case].release?.allowed === attempt.release_allowed);
  return { stage: 'proof-required-build', outcome: matched ? 'only-applicable-proved-claim-releases' : 'proof-outcome-mismatch',
    failed_proof_implies_disproof: reports['rejected-proof-term'].outcome === 'disproved',
    solver_success_is_accepted_proof: reports['bare-solver-answer'].release.allowed,
    fuel_exhaustion_proves_divergence: reports['execution-fuel-exhausted'].outcome === 'disproved',
    independent_recheck_required: reports['accepted-exact-proof'].evidence.independent_recheck
      && !reports['rejected-proof-term'].release.allowed };
});

await runCase('CONTRACT-10', async (row, item) => withSession(row, async session => {
  await session.call(`submit ${fixture('q-one.noble')}`);
  const proof = await admit(row, session);
  const bound = await certify(row, session, 0);
  const index = await session.slot('Certified');
  const original = decode((await session.call(`inspect ${index}`)).detail);
  check(row, 'guest identity is the identity returned by core binding', original.identity,
    BigInt(`0x${bound.subject_identity}`).toString());
  check(row, 'guest carries retained contract metadata', original.contract.type, 'Contract');
  check(row, 'guest carries retained evidence reference', original.evidence.type, 'Evidence');
  check(row, 'guest contract reference matches independently accepted claim', original.contract.index, proof.contract_reference);
  check(row, 'guest evidence reference matches independently accepted evidence', original.evidence.index, proof.evidence_reference);
  check(row, 'guest evidence references same exact contract', original.evidence.contract, original.contract);
  check(row, 'guest companion contains descriptors, not proof terms', Object.keys(original).sort(),
    ['contract', 'evidence', 'identity', 'subject', 'type']);
  check(row, 'guest evidence carries no proof term or live capability', Object.keys(original.evidence).sort(),
    ['class', 'contract', 'index', 'ruleset', 'type']);
  check(row, 'reflection has no source/debug metadata', Object.keys(original.subject).sort(),
    ['interface', 'recipe', 'type', 'witnesses']);
  await session.disableProofServices();
  const before = await session.call('stack');
  const transport = await session.call(`submit ${fixture('pass-return.noble')}`);
  check(row, 'compiled program receives and returns companion', transport.outcome, 'normal');
  check(row, 'returned companion preserves the entire guest payload', output(transport), original);
  check(row, 'pass-return keeps the complete surrounding stack', transport.stack, before.stack);
  const transportedModule = decode(transport.module);
  check(row, 'pass-return uses the selected debug-stripping optimizer', transportedModule.optimized, true);
  check(row, 'compiled pass-return module binds the actual source', transportedModule.source_sha256,
    hash(await readFile(fixture('pass-return.noble'))));
  const duplicated = await session.call(`submit ${fixture('dup.noble')}`);
  check(row, 'immutable companion can be duplicated without authority', output(duplicated), original);
  const listed = await session.call(`submit ${fixture('list.noble')}`);
  const list = output(listed);
  check(row, 'homogeneous list contains the exact companion payload', list, { type: 'List', value: [original] });
  const listModule = decode(listed.module);
  check(row, 'list transport uses the selected debug-stripping optimizer', listModule.optimized, true);
  check(row, 'compiled list module binds the actual source', listModule.source_sha256,
    hash(await readFile(fixture('list.noble'))));
  const retainedCompanion = await session.slot('Certified');
  const aggregateReplay = await session.call(`compose ${retainedCompanion} ${retainedCompanion} 41`);
  check(row, 'certified replay works with a homogeneous companion-list tail', aggregateReplay.outcome, 'certified-composition');
  check(row, 'replayed composition with aggregate tail returns exact output', output(aggregateReplay), scalar(43));
  const restored = await session.call('stack');
  check(row, 'replay preserves every pre-existing aggregate and cell handle',
    restored.stack.slice(0, listed.stack.length), listed.stack);
  row.aggregate_tail_regression = { before: listed.stack, operation: aggregateReplay, after: restored.stack };
  const inspected = await session.call(`inspect ${retainedCompanion}`);
  const projected = await session.call(`project ${retainedCompanion}`);
  check(row, 'transported inspection still carries contract and evidence', decode(inspected.detail), original);
  check(row, 'explicit projection returns the original program', projected.subject_handle, bound.subject_handle);
  check(row, 'explicit projection preserves underlying identity', projected.subject_identity, original.identity);
  row.operations = [
    { operation: 'pass-as-argument', program: fixture('pass-return.noble'), input_stack: before.stack, report: transport },
    { operation: 'return-as-result', program: fixture('pass-return.noble'), report: transport },
    { operation: 'store-in-homogeneous-list', program: fixture('list.noble'), report: listed },
    { operation: 'inspect-contract-and-evidence-reference', report: inspected },
    { operation: 'explicitly-project-program', report: projected },
  ];
  check(row, 'every canonical transport operation executed', row.operations.map(entry => entry.operation), item.input.operations);
  await session.proveServicesUnavailable();
  const runtimeReports = [transport, duplicated, listed, aggregateReplay, inspected, projected];
  return { stage: 'normal-return', outcome: list?.value?.[0]?.type === 'Certified' && projected.outcome === 'projected' ? 'all-operations-preserve-companion-contract' : 'transport-failure',
    underlying_program_identity: list.value[0].identity === original.identity && projected.identity_unchanged ? 'unchanged' : 'changed',
    underlying_reflected_recipe: JSON.stringify(list.value[0].subject) === JSON.stringify(original.subject) ? 'unchanged' : 'changed',
    selected_inspection_metadata_retained: decode(inspected.detail).contract.index === proof.contract_reference
      && decode(inspected.detail).evidence.index === proof.evidence_reference,
    prover_calls: runtimeReports.reduce((sum, report) => sum + report.prover_calls, 0),
    implicit_evidence_fetches: inspected.implicit_evidence_fetches,
    host_only_metadata_sufficient: list.value[0].contract === undefined || list.value[0].evidence === undefined,
    authority_created: inspected.authority_created };
}, { optimized: true }));

await runCase('CONTRACT-11', async (row, item) => {
  const sourcePath = await source('ordinary-without-contract.noble', `${item.input.source}\n`);
  const run = await command(row, ['run', sourcePath], { env: { NOBLE_LEAN: '/nonexistent/proof-services-disabled', NOBLE_CONTRACT_LIBRARY: '/nonexistent/proof-library-disabled' } });
  const report = jsonReport(row, run);
  check(row, 'ordinary program process exit', run.exit, 0);
  check(row, 'ordinary program enters Wasm normally', report.stage, 'wasm');
  const plain = await withSession(row, async session => {
    await session.disableProofServices();
    const result = await session.call(`plain ${sourcePath}`);
    await session.proveServicesUnavailable();
    return result;
  });
  check(row, 'ordinary CLI result', scalarStack(row, 'ordinary result', report.stack), [item.expected.output]);
  return { stage: 'normal-return', outcome: report.outcome === 'normal' && plain.outcome === 'normal' ? 'ordinary-success' : report.outcome,
    output: output(plain), behavioral_certification_claimed: plain.behavioral_certification_claimed,
    prover_calls: plain.prover_calls, candidate_prepare_requests: plain.candidate_prepare_requests };
});

await runCase('CONTRACT-12', async row => {
  const controls = await coreControls(row, row.case);
  const reasons = { 'resource-in-ghost-capture': 'unsupported',
    'service-capability-in-pure-evidence': 'live-capability-in-evidence', 'runtime-value-from-erased-ghost': 'invalid' };
  for (const control of controls) {
    coreRefusal(row, control, reasons[control.variant]);
    check(row, `${control.variant}: ownership cannot be duplicated`, control.observed?.resource_ownership_duplicated, false);
  }
  return { stage: 'contract-and-eligibility-check', outcome: controls.length === 3 && controls.every(control => control.observed?.new_certified_status === false) ? 'reject-each-variant' : 'incomplete-eligibility-refusals',
    resource_ownership_duplicated: controls.some(control => control.observed?.resource_ownership_duplicated !== false),
    guest_requests: controls.every(control => control.execution_capability === 'none') ? 0 : null,
    protected_operations: controls.every(control => control.execution_capability === 'none') ? 0 : null };
});

await runCase('CONTRACT-13', async row => {
  const result = await requiredBuild(row, 'unrelated-same-interface-artifact', fixture('q-two.noble'), fixture('increment.contract'), ['--proof', fixture('increment.proof.lean')]);
  const report = result.report;
  check(row, 'unrelated artifact cannot be released', report.release?.allowed, false);
  check(row, 'unrelated source still has a valid independent increment proof', report.evidence?.independent_recheck, true);
  check(row, 'subject-to-artifact mismatch has exact refusal', diagnosticCodes(report).includes('correspondence-mismatch'), true);
  return { stage: 'artifact-admission', outcome: diagnosticCodes(report).includes('correspondence-mismatch') ? 'correspondence-reject' : report.outcome,
    candidate_body_started: report.candidate_body_started, guest_requests: report.guest_requests,
    protected_operations: report.protected_operations };
});

// The same hostile-exporter seam used by MC1: a successful untrusted module
// initializer supplies raw declarations. Only independent consumer replay can
// accept them. Each mutation below has its own payload, subprocess and refusal.
const constant = (name, levels = []) => ['const', name.split('.'), levels];
const apply = (fn, ...args) => args.reduce((expression, argument) => ['app', expression, argument], fn);
const natural = value => ['nat', String(value)];
const integer = value => apply(constant('Int.ofNat'), natural(value));
const term = (name, ...args) => apply(constant(`NobleContracts.Term.${name}`), ...args);
function injectedWire(payload) {
  return `import Lean\nimport MC1Obligation\ninitialize do\n  if ← System.FilePath.pathExists "/out/MC1Proof.json" then\n    IO.FS.writeFile "/out/MC1Proof.json" ${JSON.stringify(payload)}\n    IO.Process.exit 0\nnamespace MC1Proof\ntheorem proof : True := True.intro\nend MC1Proof\n`;
}
await runCase('CONTRACT-14', async row => {
  const baseline = await command(row, ['verify', fixture('increment.contract'), '--proof', fixture('increment.proof.lean'), '--timeout-ms', String(proofTimeout)]);
  const proof = jsonReport(row, baseline);
  independentProof(row, proof, 'statement-mutation baseline');
  check(row, 'statement-mutation baseline exits successfully', baseline.exit, 0);
  const wire = JSON.parse(proof.proof_declarations);
  const positive = await source('hostile-exporter-valid-wire.lean', injectedWire(proof.proof_declarations));
  const positiveBuild = await requiredBuild(row, 'hostile-exporter-valid-wire', fixture('q-one.noble'), fixture('increment.contract'), ['--proof', positive]);
  check(row, 'hostile exporter transport itself is not rejected', positiveBuild.report.release.allowed, true);
  check(row, 'valid wire is independently rechecked', positiveBuild.report.evidence.independent_recheck, true);
  const eqInt = (input, out) => apply(constant('Eq', [['succ', ['zero']]]), constant('Int'), ['bvar', out],
    apply(constant('Int.add'), ['bvar', input], integer(1)));
  const unboundedClaim = ['forall', ['input'], constant('Int'),
    ['forall', ['output'], constant('Int'),
      ['forall', ['execution'], eqInt(1, 0), eqInt(2, 1), 'default'], 'default'], 'default'];
  const mutations = [
    ['changed-body', 'MC1Obligation.node_0', constant('NobleContracts.Op'),
      apply(constant('NobleContracts.Op.lit'), apply(constant('NobleContracts.Value.i64'),
        apply(constant('BitVec.ofInt'), natural(64), integer(2))))],
    ['weakened-postcondition', 'MC1Obligation.postcondition', constant('NobleContracts.Term'), term('bool', constant('Bool.true'))],
    ['strengthened-precondition', 'MC1Obligation.precondition', constant('NobleContracts.Term'),
      term('le', term('i64', integer(0)), term('input', natural(0)))],
    ['rebound-logical-definition', 'MC1Obligation.expression_4', constant('NobleContracts.Term'), term('i64', integer(0))],
    ['unbounded-integer-model', 'MC1Obligation.claim', ['sort', ['zero']], unboundedClaim],
  ];
  const variants = [];
  for (const [variant, name, type, value] of mutations) {
    const mutated = structuredClone(wire);
    const declaration = { kind: 'definition', name: name.split('.'), levels: [], type, value };
    mutated.declarations.push(declaration);
    const encoded = JSON.stringify(mutated);
    const path = await source(`${variant}.lean`, injectedWire(encoded));
    const result = await requiredBuild(row, variant, fixture('q-one.noble'), fixture('increment.contract'), ['--proof', path]);
    const report = result.report;
    const recheck = report.diagnostics.find(item => item.code === 'independent-recheck-rejected');
    variants.push({ variant, input: { trusted_contract: fixture('increment.contract'), producer_proof: path,
      changed_declaration: declaration, wire_sha256: hash(encoded), root: mutated.root }, command: result.run.id, report });
    check(row, `${variant}: actual declaration payload differs`, encoded === proof.proof_declarations, false);
    check(row, `${variant}: display/root name kept unchanged`, mutated.root, wire.root);
    check(row, `${variant}: consumer rejects changed trusted definition`, report.outcome, 'error');
    check(row, `${variant}: exact independent consumer refusal`, recheck?.code, 'independent-recheck-rejected');
    check(row, `${variant}: independent consumer reports protected name`, recheck?.details?.stderr?.includes(`MC1_DUPLICATE_DECLARATION: duplicate or trusted name ${name}`), true);
    check(row, `${variant}: no release`, report.release?.allowed, false);
    quiet(row, variant, report);
  }
  variantSet(row, variants);
  check(row, 'five genuinely distinct hostile declaration payloads', new Set(variants.map(variant => variant.input.wire_sha256)).size, 5);
  return { stage: 'statement-admission', outcome: variants.every(variant => variant.report.outcome === 'error' && !variant.report.release.allowed) ? 'reject-each-unapproved-change' : 'statement-mutation-accepted',
    same_display_name_sufficient: variants.some(variant => variant.report.release.allowed),
    independent_expected_claim_required: variants.every(variant => diagnosticCodes(variant.report).includes('independent-recheck-rejected')),
    guest_requests: variants.reduce((sum, variant) => sum + variant.report.guest_requests, 0),
    protected_operations: variants.reduce((sum, variant) => sum + variant.report.protected_operations, 0) };
});

await runCase('CONTRACT-15', async (row, item) => {
  const ordinary = await command(row, ['run', fixture('vector-41.noble')]);
  const ordinaryReport = jsonReport(row, ordinary);
  check(row, 'contract rejection leaves ordinary program valid',
    scalarStack(row, 'ordinary valid source', ordinaryReport.stack), [scalar(42)]);
  check(row, 'ordinary program exit', ordinary.exit, 0);
  const files = { 'unbound-contract-variable': 'unbound-variable.contract', 'predicate-type-mismatch': 'predicate-mismatch.contract',
    'unsupported-host-protocol': 'unsupported-host.contract' };
  const variants = [];
  for (const variant of item.input.variants) {
    const result = await requiredBuild(row, variant.case, fixture('q-one.noble'), fixture(files[variant.case]), ['--proof', fixture('increment.proof.lean')]);
    variants.push({ variant: variant.case, report: result.report, command: result.run.id });
    check(row, `${variant.case}: required admission blocked`, result.report.release?.allowed, false);
    check(row, `${variant.case}: diagnostic classification`, diagnosticCodes(result.report).includes(`contract-${variant.diagnostic}`), true);
    check(row, `${variant.case}: proof outcome`, result.report.outcome, variant.diagnostic === 'invalid' ? 'error' : 'unsupported');
    quiet(row, variant.case, result.report);
  }
  variantSet(row, variants);
  return { stage: 'contract-elaboration', outcome: variants.every(variant => !variant.report.release.allowed) ? 'reject-contract-only' : 'invalid-contract-released',
    ordinary_program_valid: ordinaryReport.outcome === 'normal',
    claim_silently_weakened: variants.some(variant => variant.report.outcome === 'proved'),
    required_proof_admission: variants.every(variant => variant.report.release.allowed === false) ? 'blocked' : 'released',
    guest_requests: variants.reduce((sum, variant) => sum + variant.report.guest_requests, 0),
    protected_operations: variants.reduce((sum, variant) => sum + variant.report.protected_operations, 0) };
});

// Never shrink a summary to whichever cases happened to produce reports.
const expectedIds = canonical.cases.map(item => item.id).sort();
const actualIds = acceptance.cases.map(item => item.case).sort();
let complete = cases.size === canonical.cases.length;
try { assert.deepStrictEqual(actualIds, expectedIds); } catch { complete = false; }
for (const path of retained) {
  try { const bytes = await readFile(path); acceptance.inputs[path] = { sha256: hash(bytes), utf8: bytes.toString('utf8') }; }
  catch (error) { acceptance.inputs[path] = { error: String(error) }; complete = false; }
}
for (const executable of new Set(acceptance.commands.map(command => command.executable).filter(isAbsolute))) {
  try { acceptance.executables[executable] = { sha256: hash(await readFile(executable)) }; }
  catch (error) { acceptance.executables[executable] = { error: String(error) }; complete = false; }
}
for (const executable of frozenExecutables) {
  executable.sha256_at_end = acceptance.executables[executable.executed]?.sha256 ?? null;
  executable.unchanged = executable.sha256_at_end === executable.sha256_at_start;
  if (!executable.unchanged) complete = false;
}
try { acceptance.binary_sha256 = hash(await readFile(binary)); }
catch (error) { acceptance.binary_error = String(error); complete = false; }
acceptance.gate_sha256 = acceptance.inputs[fileURLToPath(import.meta.url)]?.sha256;
acceptance.summary = { cases: canonical.cases.length, executed: acceptance.cases.length,
  passed: acceptance.cases.filter(row => row.passed).length,
  failed: acceptance.cases.filter(row => !row.passed).map(row => row.case), complete };
acceptance.passed = complete && acceptance.summary.passed === canonical.cases.length;
await writeFile(destination, `${JSON.stringify(acceptance, null, 2)}\n`);
console.log(`${acceptance.summary.passed}/${canonical.cases.length} MC2 cases passed; ${destination}`);
process.exitCode = acceptance.passed ? 0 : 1;
