import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';

const [cli, peer, fixtures, output] = process.argv.slice(2);
assert.ok(cli && peer && fixtures && output,
  'usage: node verification/worker-send/test.mjs NOBLE PEER FIXTURE_DIR NEW_OUTPUT_DIR');
fs.mkdirSync(output, { recursive: false, mode: 0o700 });
const wit = path.resolve('crates/noble-wasm/wit/worker-send.wit');
const source = path.resolve('crates/noble-cli/tests/fixtures/worker-send-dispatch.noble');
const componentDir = path.join(output, 'component');
// One component carries both exports: the cancelling guest program backs
// dispatch, the explicit asynchronous finish bridges dispatch-result. The
// selected mode decides which export the peer invokes.
const finishSource = path.resolve('crates/noble-cli/tests/fixtures/worker-send-finish.noble');
const compiled = spawnSync(cli, ['component', 'compile', wit, 'worker-shell', componentDir,
  `dispatch=${source}`, `dispatch-result=${finishSource}`],
{ encoding: 'utf8', timeout: 120_000 });
assert.equal(compiled.status, 0, compiled.stderr || compiled.stdout);
const compilation = JSON.parse(compiled.stdout);
assert.equal(compilation.outcome, 'compiled');
assert.equal(compilation.component_emitted, true);
const component = path.join(componentDir, 'component.wasm');
const componentSha256 = createHash('sha256').update(fs.readFileSync(component)).digest('hex');
const compiledComponent = { path: component, sha256: componentSha256 };
const startTrap = path.resolve('verification/worker-send/start-trap.wat');
const startTrapComponent = {
  path: startTrap,
  sha256: createHash('sha256').update(fs.readFileSync(startTrap)).digest('hex'),
};
const missingDispatch = path.resolve('verification/worker-send/start-missing-dispatch.wat');
const missingDispatchComponent = {
  path: missingDispatch,
  sha256: createHash('sha256').update(fs.readFileSync(missingDispatch)).digest('hex'),
};
const selected = name => path.join(fixtures, name);
const worker = JSON.parse(fs.readFileSync(selected('package.json'), 'utf8'));
const selectedSourceSha256 = createHash('sha256').update(worker.root.source_utf8).digest('hex');
const grant = (name, { allow = true, attempt = '1', expires_at = 20,
  source_sha256 = selectedSourceSha256 } = {}) => {
  const bytes = {
    allow, attempt, expires_at,
    format: 'noble-worker-send-grant/v1',
    operation: 'noble-test:worker-send/host@1.0.0#begin-send',
    source_sha256, text: 'ping',
  };
  const file = path.join(output, `${name}.json`);
  fs.writeFileSync(file, JSON.stringify(bytes), { flag: 'wx', mode: 0o600 });
  return file;
};
const approvedGrant = grant('approved');
const deniedGrant = grant('denied', { allow: false });
const wrongAttemptGrant = grant('wrong-attempt', { attempt: '2' });
const expiredGrant = grant('expired', { expires_at: 8 });
const mismatchedSource = grant('mismatched-source', {
  source_sha256: `${selectedSourceSha256[0] === '0' ? '1' : '0'}${selectedSourceSha256.slice(1)}`,
});
const generated = JSON.parse(fs.readFileSync(selected('submit.json'), 'utf8'));
generated.value.value.right.value = '{"grant":"candidate-generated","allow":true}';
const generatedEvent = path.join(output, 'candidate-grant-event.json');
fs.writeFileSync(generatedEvent, JSON.stringify(generated), { flag: 'wx', mode: 0o600 });

function run(name, mode, submit, selectedGrant, expectedStatus = 0,
  selectedComponent = compiledComponent) {
  const invoked = spawnSync(peer, [selectedComponent.path, cli, selected('package.json'),
    selected('schema.noble'), selected('state.json'), submit, selected('cancel.json'),
    selectedGrant, mode], { encoding: 'utf8', timeout: 120_000 });
  assert.equal(invoked.status, expectedStatus, `${name}: ${invoked.stderr}\n${invoked.stdout}`);
  const report = JSON.parse(invoked.stdout);
  fs.writeFileSync(path.join(output, `${name.replaceAll(' ', '-')}.report.json`),
    invoked.stdout, { flag: 'wx', mode: 0o600 });
  assert.equal(report.schema, 'noble-worker-send-wit-integration/v1', name);
  assert.equal(report.component_sha256, selectedComponent.sha256,
    `${name}: peer did not report the exact selected component bytes`);
  assert.equal(report.first_guest.outcome, 'normal', name);
  assert.equal(report.cancel_guest.outcome, 'normal', name);
  assert.equal(report.source_sha256, selectedSourceSha256,
    `${name}: host selected a different checked guest source`);
  assert.equal(report.first_guest.source_sha256, selectedSourceSha256,
    `${name}: Submit did not execute the selected checked source`);
  assert.equal(report.cancel_guest.source_sha256, selectedSourceSha256,
    `${name}: Cancelled did not execute the same checked source`);
  return report;
}
const allowed = run('authorized socket send', 'cancel-with-native-outstanding',
  selected('submit.json'), approvedGrant);
assert.equal(allowed.outcome, 'cancelled-retired');
assert.equal(allowed.host.receiver_datagrams, 1);
assert.equal(allowed.host.native_send_bytes, 12);
assert.equal(allowed.host.protected_operation_attempts, 1);
assert.equal(allowed.host.pins_at_cancel, 1);
assert.equal(allowed.host.late_completion_retired, true);
assert.equal(allowed.host.late_task_outcome, 'success');
assert.equal(allowed.host.task_retired, 1);
assert.equal(allowed.host.resource_retired, 1);
assert.equal(allowed.host.task_outstanding_pins, 0);
assert.equal(allowed.host.resource_native_pins, 0);
assert.equal(allowed.host.task_reserved_bytes, 0);
assert.equal(allowed.host.successful_result_deliveries, 0);
assert.equal(allowed.native_pins_at_guest_exit, 1);
assert.equal(Object.hasOwn(allowed, 'native_completion_diagnostic'), false);
const cancelled = run('cancel before native send', 'cancel-before-delivery',
  selected('submit.json'), approvedGrant);
assert.equal(cancelled.outcome, 'cancelled-retired');
assert.equal(cancelled.host.receiver_datagrams, 0);
assert.equal(cancelled.host.native_send_bytes, 0);
assert.equal(cancelled.host.protected_operation_attempts, 1);
assert.equal(cancelled.host.pins_at_cancel, 1);
assert.equal(cancelled.native_pins_at_guest_exit, 1);
assert.equal(cancelled.host.task_retired, 1);
assert.equal(cancelled.host.resource_retired, 1);
assert.equal(cancelled.host.late_completion_retired, true);
assert.equal(cancelled.host.late_task_outcome, 'domain-error');
assert.equal(cancelled.host.task_reserved_bytes, 0);
assert.equal(cancelled.host.successful_result_deliveries, 0);
assert.equal(cancelled.host.native_error, null);
assert.equal(Object.hasOwn(cancelled, 'native_completion_diagnostic'), false);
const startTrapReport = run('core start traps after send admission', 'cancel-before-delivery',
  selected('submit.json'), approvedGrant, 2, startTrapComponent);
assert.equal(startTrapReport.outcome, 'internal-failure');
assert.match(startTrapReport.guest_component_error, /unreachable|trap/i);
assert.equal(startTrapReport.future_consumed, false);
assert.equal(startTrapReport.native_pins_at_guest_exit, 1);
assert.equal(startTrapReport.host.protected_operation_attempts, 1);
assert.equal(startTrapReport.host.pins_at_cancel, 1);
assert.equal(startTrapReport.host.task_retired, 1);
assert.equal(startTrapReport.host.resource_retired, 1);
assert.equal(startTrapReport.host.late_completion_retired, true);
assert.equal(startTrapReport.host.late_task_outcome, 'domain-error');
assert.equal(startTrapReport.host.task_outstanding_pins, 0);
assert.equal(startTrapReport.host.resource_native_pins, 0);
assert.equal(startTrapReport.host.task_reserved_bytes, 0);
assert.equal(startTrapReport.host.native_send_bytes, 0);
assert.equal(startTrapReport.host.receiver_datagrams, 0);
assert.equal(startTrapReport.host.successful_result_deliveries, 0);
assert.deepEqual(startTrapReport.host.cleanup_failures, []);
assert.equal(Object.hasOwn(startTrapReport, 'abort_diagnostic'), false);
assert.equal(Object.hasOwn(startTrapReport, 'native_completion_diagnostic'), false);
const lookupFailure = run('core start sends before typed lookup failure', 'cancel-before-delivery',
  selected('submit.json'), approvedGrant, 2, missingDispatchComponent);
assert.equal(lookupFailure.outcome, 'internal-failure');
assert.match(lookupFailure.guest_component_error, /dispatch/);
assert.equal(lookupFailure.future_consumed, false);
assert.equal(lookupFailure.native_pins_at_guest_exit, 1);
assert.equal(lookupFailure.host.protected_operation_attempts, 1);
assert.equal(lookupFailure.host.pins_at_cancel, 1);
assert.equal(lookupFailure.host.task_retired, 1);
assert.equal(lookupFailure.host.resource_retired, 1);
assert.equal(lookupFailure.host.late_completion_retired, true);
assert.equal(lookupFailure.host.late_task_outcome, 'domain-error');
assert.equal(lookupFailure.host.task_outstanding_pins, 0);
assert.equal(lookupFailure.host.resource_native_pins, 0);
assert.equal(lookupFailure.host.task_reserved_bytes, 0);
assert.equal(lookupFailure.host.native_send_bytes, 0);
assert.equal(lookupFailure.host.receiver_datagrams, 0);
assert.equal(lookupFailure.host.successful_result_deliveries, 0);
assert.deepEqual(lookupFailure.host.cleanup_failures, []);
assert.equal(Object.hasOwn(lookupFailure, 'abort_diagnostic'), false);
assert.equal(Object.hasOwn(lookupFailure, 'native_completion_diagnostic'), false);
for (const [name, mode, diagnostic] of [
  ['injected receiver error', 'inject-receive-error', /native receiver read failed/],
  ['injected receiver corruption', 'inject-receive-corruption',
    /native receiver did not observe exact authorized Send/],
]) {
  // Both modes perform the actual selected native Send. The test host alone
  // consumes or corrupts its receipt after join, not the guest or the wire.
  const fault = run(name, mode, selected('submit.json'), approvedGrant, 2);
  assert.equal(fault.outcome, 'native-validation-failure-retired', name);
  assert.equal(fault.future_consumed, true, name);
  assert.equal(fault.native_pins_at_guest_exit, 1, name);
  assert.equal(fault.host.receiver_datagrams, 1, name);
  assert.equal(fault.host.native_send_bytes, 12, name);
  assert.equal(fault.host.protected_operation_attempts, 1, name);
  assert.equal(fault.host.pins_at_cancel, 1, name);
  assert.equal(fault.host.late_completion_retired, true, name);
  assert.equal(fault.host.late_task_outcome, 'domain-error', name);
  assert.equal(fault.host.task_retired, 1, name);
  assert.equal(fault.host.resource_retired, 1, name);
  assert.equal(fault.host.task_outstanding_pins, 0, name);
  assert.equal(fault.host.resource_native_pins, 0, name);
  assert.equal(fault.host.task_reserved_bytes, 0, name);
  assert.equal(fault.host.successful_result_deliveries, 0, name);
  assert.equal(fault.host.native_error, null, name);
  assert.match(fault.native_completion_diagnostic, diagnostic, name);
}
const producerPanic = run('native producer panic after send', 'inject-producer-panic',
  selected('submit.json'), approvedGrant, 2);
assert.equal(producerPanic.outcome, 'native-validation-failure-retired');
assert.equal(producerPanic.future_consumed, true);
assert.equal(producerPanic.guest_component_error, null);
assert.equal(producerPanic.native_pins_at_guest_exit, 1);
assert.equal(producerPanic.host.native_send_bytes, 12);
assert.equal(producerPanic.host.receiver_datagrams, 1);
assert.equal(producerPanic.host.native_error, 'native Send producer panicked');
assert.equal(producerPanic.host.protected_operation_attempts, 1);
assert.equal(producerPanic.host.successful_result_deliveries, 0);
assert.equal(producerPanic.host.pins_at_cancel, 1);
assert.equal(producerPanic.host.late_completion_retired, true);
assert.equal(producerPanic.host.late_task_outcome, 'domain-error');
assert.equal(producerPanic.host.task_retired, 1);
assert.equal(producerPanic.host.resource_retired, 1);
assert.equal(producerPanic.host.task_outstanding_pins, 0);
assert.equal(producerPanic.host.resource_native_pins, 0);
assert.equal(producerPanic.host.task_reserved_bytes, 0);
assert.match(producerPanic.native_completion_diagnostic,
  /failed native Send unexpectedly reached receiver/);
for (const [name, mode, failure] of [
  ['rejected native cancellation', 'inject-cancel-error', /WrongContext/],
  ['unwound native cancellation', 'inject-cancel-panic', /host Send cancellation panicked/],
]) {
  const fault = run(name, mode, selected('submit.json'), approvedGrant, 2);
  assert.equal(fault.outcome, 'internal-failure', name);
  assert.match(fault.guest_component_error, failure, name);
  assert.equal(fault.future_consumed, true, name);
  assert.equal(fault.native_pins_at_guest_exit, 1, name);
  assert.equal(fault.host.receiver_datagrams, 1, name);
  assert.equal(fault.host.native_send_bytes, 12, name);
  assert.equal(fault.host.protected_operation_attempts, 1, name);
  assert.equal(fault.host.successful_result_deliveries, 0, name);
  assert.equal(fault.host.late_completion_retired, true, name);
  assert.equal(fault.host.late_task_outcome, 'success', name);
  assert.equal(fault.host.task_retired, 1, name);
  assert.equal(fault.host.resource_retired, 1, name);
  assert.equal(fault.host.task_outstanding_pins, 0, name);
  assert.equal(fault.host.resource_native_pins, 0, name);
  assert.equal(fault.host.task_reserved_bytes, 0, name);
  assert.equal(Object.hasOwn(fault, 'native_completion_diagnostic'), false, name);
}
const fallbackAuthority = run('failed fallback authority retirement',
  'inject-fallback-authority-error', selected('submit.json'), approvedGrant, 2);
assert.equal(fallbackAuthority.outcome, 'internal-failure');
assert.match(fallbackAuthority.guest_component_error, /InvocationFinished/);
assert.equal(fallbackAuthority.host.cleanup_failures.length, 1);
assert.match(fallbackAuthority.host.cleanup_failures[0],
  /authority retirement \(Cancelled\).*InvocationFinished/);
assert.equal(fallbackAuthority.future_consumed, true);
assert.equal(fallbackAuthority.native_pins_at_guest_exit, 1);
assert.equal(fallbackAuthority.host.protected_operation_attempts, 1);
assert.equal(fallbackAuthority.host.pins_at_cancel, 1);
assert.equal(fallbackAuthority.host.task_retired, 1);
assert.equal(fallbackAuthority.host.resource_retired, 1);
assert.equal(fallbackAuthority.host.late_completion_retired, true);
assert.equal(fallbackAuthority.host.late_task_outcome, 'domain-error');
assert.equal(fallbackAuthority.host.task_outstanding_pins, 0);
assert.equal(fallbackAuthority.host.resource_native_pins, 0);
assert.equal(fallbackAuthority.host.task_reserved_bytes, 0);
assert.equal(fallbackAuthority.host.native_send_bytes, 0);
assert.equal(fallbackAuthority.host.receiver_datagrams, 0);
assert.equal(fallbackAuthority.host.successful_result_deliveries, 0);
assert.equal(Object.hasOwn(fallbackAuthority, 'native_completion_diagnostic'), false);
const bufferQuota = run('buffer quota preflight', 'buffer-quota-preflight',
  selected('submit.json'), approvedGrant, 2);
assert.equal(bufferQuota.outcome, 'quota-denied');
assert.match(bufferQuota.guest_component_error,
  /task buffer reservation denied before Send admission/);
assert.equal(bufferQuota.future_consumed, true);
assert.equal(bufferQuota.native_pins_at_guest_exit, 0);
assert.equal(bufferQuota.host.quota_refusal, 'byte-capacity');
assert.equal(bufferQuota.host.task_retired, 0);
assert.equal(bufferQuota.host.task_outstanding_pins, 0);
assert.equal(bufferQuota.host.task_reserved_bytes, 0);
assert.equal(bufferQuota.host.resource_retired, 1);
assert.equal(bufferQuota.host.resource_native_pins, 0);
assert.equal(bufferQuota.host.protected_operation_attempts, 0);
assert.equal(bufferQuota.host.successful_result_deliveries, 0);
assert.equal(bufferQuota.host.native_send_bytes, 0);
assert.equal(bufferQuota.host.receiver_datagrams, 0);
assert.equal(bufferQuota.host.late_task_outcome, null);
assert.equal(Object.hasOwn(bufferQuota, 'native_completion_diagnostic'), false);
for (const [name, submit, selection] of [
  ['no-grant', selected('submit.json'), deniedGrant],
  ['mismatched-attempt-grant', selected('submit.json'), wrongAttemptGrant],
  ['expired-grant', selected('submit.json'), expiredGrant],
  ['candidate-generated-grant', generatedEvent, approvedGrant],
]) {
  const refused = run(name, 'cancel-before-delivery', submit, selection);
  assert.equal(refused.outcome, 'denied', name);
  assert.equal(refused.host.receiver_datagrams, 0, name);
  assert.equal(refused.host.native_send_bytes, 0, name);
  assert.equal(refused.host.protected_operation_attempts, 0, name);
  assert.equal(refused.host.resource_retired, 1, name);
  assert.equal(refused.host.task_outstanding_pins, 0, name);
  assert.equal(refused.host.resource_native_pins, 0, name);
  assert.equal(refused.host.task_reserved_bytes, 0, name);
}
const wrongSource = spawnSync(peer, [component, cli, selected('package.json'),
  selected('schema.noble'), selected('state.json'), selected('submit.json'),
  selected('cancel.json'), mismatchedSource, 'cancel-before-delivery'],
{ encoding: 'utf8', timeout: 120_000 });
assert.equal(wrongSource.status, 2, wrongSource.stderr || wrongSource.stdout);
const wrongReport = JSON.parse(wrongSource.stdout);
fs.writeFileSync(path.join(output, 'mismatched-source-grant.report.json'),
  wrongSource.stdout, { flag: 'wx', mode: 0o600 });
assert.equal(wrongReport.schema, 'noble-worker-send-wit-integration/v1');
assert.equal(wrongReport.outcome, 'internal-failure');
assert.equal(Object.hasOwn(wrongReport, 'host'), false,
  'a mismatched worker source grant must be refused before a native host exists');

// --- native-async delivery, suspension and deadline rows -------------------
function runDelivery(name, mode, selectedGrant, expectedStatus = 0) {
  return run(name, mode, selected('submit.json'), selectedGrant, expectedStatus);
}
const delivered = runDelivery('delivered once through guest finish', 'delivery-before-cancel',
  approvedGrant);
assert.equal(delivered.outcome, 'delivered-once');
assert.deepEqual(delivered.host.lifecycle_sequence,
  ['submit-worker-event', 'admission-commit', 'native-handoff', 'result-delivery',
    'invocation-delivery', 'duplicate-cancellation']);
assert.equal(delivered.future_consumed, true);
assert.equal(delivered.native_pins_at_guest_exit, 0);
assert.equal(delivered.handle_disposition, 'finished');
// Delivery is one-shot: exactly one native Send, one guest delivery and one
// returned caller owner, and no resurrection or late worker event.
assert.equal(delivered.host.native_send_bytes, 12);
assert.equal(delivered.host.receiver_datagrams, 1);
assert.equal(delivered.host.task_deliveries, 1);
assert.equal(delivered.host.successful_result_deliveries, 1);
assert.equal(delivered.host.owner_returns, 1);
assert.equal(delivered.host.owner_resurrections, 0);
assert.equal(delivered.host.late_worker_events, 0);
assert.equal(delivered.host.late_completion_retired, false);
assert.equal(delivered.host.task_outstanding_pins, 0);
assert.equal(delivered.host.resource_native_pins, 0);
assert.equal(delivered.host.task_reserved_bytes, 0);
assert.equal(delivered.host.resource_retired, 0);
assert.equal(delivered.host.authority_invocation, 'Succeeded');
assert.equal(delivered.host.operation_attempt_outcome, 'OperationSuccess');
assert.deepEqual(delivered.host.delivered_result, { ok: 1 });
assert.deepEqual(delivered.host.receipts.map((receipt) => receipt.claim),
  ['InvocationSuccess']);
assert.equal(delivered.host.receipts[0].scope, 'InvocationDelivery');
assert.equal(delivered.host.receipts[0].invocation_outcome, 'Succeeded');
// Every boundary is a live snapshot of the retained tables, not a constant.
for (const boundary of delivered.host.boundaries) {
  assert.ok(boundary.tasks && boundary.resources && boundary.authority, boundary.boundary);
  assert.equal(Object.hasOwn(boundary, 'external_send_outcome'), false);
  assert.equal(Object.hasOwn(boundary, 'external_rollback_claimed'), false);
}
assert.equal(delivered.host.boundaries.at(-1).boundary, 'duplicate-cancellation');
assert.equal(delivered.host.boundaries.at(-1).task.state, 'Delivered');
assert.equal(delivered.host.boundaries.at(-1).resources.live, 1);
assert.equal(delivered.host.boundaries.at(-1).owner_with_caller, true);
assert.equal(delivered.host.boundaries.at(0).boundary, 'admission');
assert.equal(delivered.host.boundaries.at(0).task.state, 'Pending');
assert.equal(delivered.host.boundaries.at(0).tasks.reserved_bytes, 3 * 12 + 256);
assert.deepEqual(delivered.host.cleanup_failures, []);
for (const diagnostic of ['native_completion_diagnostic', 'abort_diagnostic',
  'store_drop_diagnostic', 'release_diagnostic', 'driver_diagnostic',
  'invocation_diagnostic', 'duplicate_cancellation_diagnostic']) {
  assert.equal(Object.hasOwn(delivered, diagnostic), false, diagnostic);
}
fs.writeFileSync(path.join(output, 'delivered.json'), JSON.stringify(delivered, null, 1));

const domainError = runDelivery('domain error stays a domain error', 'delivery-domain-error',
  approvedGrant);
assert.equal(domainError.outcome, 'domain-error-delivered');
assert.equal(domainError.handle_disposition, 'finished');
assert.equal(domainError.future_consumed, true);
// The withheld pre-send path issues no datagram but still delivers the domain
// error exactly once, with a failed invocation and no success receipt.
assert.equal(domainError.host.native_send_bytes, 0);
assert.equal(domainError.host.receiver_datagrams, 0);
assert.equal(domainError.host.task_deliveries, 1);
assert.equal(domainError.host.successful_result_deliveries, 0);
assert.equal(domainError.host.owner_returns, 1);
assert.equal(domainError.host.task_outstanding_pins, 0);
assert.equal(domainError.host.resource_native_pins, 0);
assert.equal(domainError.host.resource_retired, 0);
assert.equal(domainError.host.authority_invocation, 'Failed');
assert.equal(domainError.host.operation_attempt_outcome, 'OperationFailure');
assert.deepEqual(domainError.host.delivered_result, { error: 'send-refused' });
assert.equal(domainError.host.native_error, 'send-refused');
assert.deepEqual(domainError.host.receipts, []);
assert.deepEqual(domainError.host.cleanup_failures, []);
assert.equal(Object.hasOwn(domainError, 'invocation_diagnostic'), false);
fs.writeFileSync(path.join(output, 'domain-error.json'), JSON.stringify(domainError, null, 1));

const suspended = runDelivery('suspended awaiting withheld result', 'suspend-awaiting-send',
  approvedGrant);
assert.equal(suspended.outcome, 'suspended-retired');
assert.equal(suspended.handle_disposition, 'finished');
assert.equal(suspended.future_consumed, true);
assert.deepEqual(suspended.host.lifecycle_sequence,
  ['submit-worker-event', 'admission-commit', 'native-handoff', 'suspension',
    'suspension-retirement', 'late-native-completion', 'retirement-finished']);
// The pending owner was visible while native work was still pinned, and the
// late native result never crossed the withheld boundary.
assert.equal(suspended.native_pins_at_guest_exit, 1);
const suspension = suspended.host.boundaries.find((item) => item.boundary === 'suspension');
assert.equal(suspension.task.state, 'Pending');
assert.equal(suspension.task.pins, 1);
assert.equal(suspension.owner_with_caller, false);
assert.equal(suspension.resource.state, 'Busy');
assert.equal(suspension.resources.native_pins, 1);
assert.equal(suspension.authority.invocation, 'Pending');
assert.equal(suspended.host.task_deliveries, 0);
assert.equal(suspended.host.successful_result_deliveries, 0);
assert.equal(suspended.host.late_completion_retired, true);
assert.equal(suspended.host.owner_returns, 0);
assert.equal(suspended.host.owner_resurrections, 0);
assert.equal(suspended.host.task_outstanding_pins, 0);
assert.equal(suspended.host.resource_native_pins, 0);
assert.equal(suspended.host.resource_retired, 1);
assert.equal(suspended.host.authority_invocation, 'Cancelled');
assert.deepEqual(suspended.host.receipts.map((receipt) => receipt.invocation_outcome),
  ['Cancelled']);
assert.equal(suspended.host.receipts[0].claim, 'OperationSuccess');
assert.deepEqual(suspended.host.delivered_result, null);
assert.deepEqual(suspended.host.cleanup_failures, []);
assert.equal(Object.hasOwn(suspended, 'driver_diagnostic'), false);
fs.writeFileSync(path.join(output, 'suspended.json'), JSON.stringify(suspended, null, 1));

const deadline = runDelivery('host deadline retires outstanding native work',
  'deadline-with-native-outstanding', approvedGrant);
assert.equal(deadline.outcome, 'host-deadline-retired');
assert.equal(deadline.handle_disposition, 'finished');
assert.equal(deadline.future_consumed, true);
assert.deepEqual(deadline.host.lifecycle_sequence,
  ['submit-worker-event', 'admission-commit', 'native-handoff', 'deadline-ack',
    'late-native-completion', 'retirement-finished', 'deadline-retirement']);
assert.equal(deadline.native_pins_at_guest_exit, 1);
const acknowledgement = deadline.host.boundaries.find((item) => item.boundary === 'deadline-ack');
assert.equal(acknowledgement.task.state, 'Retiring');
assert.equal(acknowledgement.task.failure, 'Deadline');
assert.equal(acknowledgement.task.pins, 1);
assert.equal(acknowledgement.resources.native_pins, 1);
assert.equal(deadline.host.deadline_diagnostic,
  'bounded driver deadline elapsed with native work outstanding');
// The host retirement is only complete after the native work is joined.
const retirement = deadline.host.boundaries.find((item) => item.boundary === 'deadline-retirement');
assert.equal(retirement.task.state, 'Retired');
assert.equal(retirement.task.pins, 0);
assert.equal(retirement.resources.retired, 1);
assert.equal(retirement.authority.invocation, 'Failed');
assert.equal(deadline.host.authority_invocation, 'Failed');
assert.equal(deadline.host.late_completion_retired, true);
assert.equal(deadline.host.task_deliveries, 0);
assert.equal(deadline.host.successful_result_deliveries, 0);
assert.equal(deadline.host.owner_returns, 0);
assert.equal(deadline.host.owner_resurrections, 0);
assert.equal(deadline.host.task_outstanding_pins, 0);
assert.equal(deadline.host.resource_native_pins, 0);
assert.equal(deadline.host.task_reserved_bytes, 0);
assert.deepEqual(deadline.host.receipts.map((receipt) => receipt.invocation_outcome), ['Failed']);
assert.deepEqual(deadline.host.delivered_result, null);
assert.deepEqual(deadline.host.cleanup_failures, []);
assert.equal(Object.hasOwn(deadline, 'driver_diagnostic'), false);
fs.writeFileSync(path.join(output, 'deadline.json'), JSON.stringify(deadline, null, 1));
console.log(JSON.stringify({ schema: 'noble-worker-send-wit-smoke/v1',
  world: 'noble-test:worker-send/worker-shell@1.0.0',
  scope: 'local-socket-only-not-canonical-WORKER-01',
  cases: ['authorized socket send', 'cancel before native send',
    'core start traps after send admission', 'core start sends before typed lookup failure',
    'injected receiver error', 'injected receiver corruption',
    'native producer panic after send', 'rejected native cancellation',
    'unwound native cancellation', 'failed fallback authority retirement',
    'buffer quota preflight',
    'no-grant', 'mismatched-attempt-grant', 'expired-grant', 'candidate-generated-grant',
    'mismatched-source-grant', 'delivered once through guest finish',
    'domain error stays a domain error', 'suspended awaiting withheld result',
    'host deadline retires outstanding native work'] }));
