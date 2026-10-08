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
const compiled = spawnSync(cli, ['component', 'compile', wit, 'worker-shell', componentDir,
  `dispatch=${source}`], { encoding: 'utf8', timeout: 120_000 });
assert.equal(compiled.status, 0, compiled.stderr || compiled.stdout);
const compilation = JSON.parse(compiled.stdout);
assert.equal(compilation.outcome, 'compiled');
assert.equal(compilation.component_emitted, true);
const component = path.join(componentDir, 'component.wasm');
const componentSha256 = createHash('sha256').update(fs.readFileSync(component)).digest('hex');
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

function run(name, mode, submit, selectedGrant, expectedStatus = 0) {
  const invoked = spawnSync(peer, [component, cli, selected('package.json'),
    selected('schema.noble'), selected('state.json'), submit, selected('cancel.json'),
    selectedGrant, mode], { encoding: 'utf8', timeout: 120_000 });
  assert.equal(invoked.status, expectedStatus, `${name}: ${invoked.stderr}\n${invoked.stdout}`);
  const report = JSON.parse(invoked.stdout);
  fs.writeFileSync(path.join(output, `${name.replaceAll(' ', '-')}.report.json`),
    invoked.stdout, { flag: 'wx', mode: 0o600 });
  assert.equal(report.schema, 'noble-worker-send-wit-integration/v1', name);
  assert.equal(report.component_sha256, componentSha256,
    `${name}: peer did not report the exact selected component bytes`);
  assert.equal(report.first_guest.outcome, 'normal', name);
  assert.equal(report.cancel_guest.outcome, 'normal', name);
  assert.equal(report.source_sha256, selectedSourceSha256,
    `${name}: host selected a different checked guest source`);
  assert.equal(report.first_guest.source_sha256, selectedSourceSha256,
    `${name}: Submit did not execute the selected checked source`);
  assert.equal(report.cancel_guest.source_sha256, selectedSourceSha256,
    `${name}: Cancelled did not execute the same checked source`);
  assert.equal(report.host.external_send_outcome, 'not-attempted', name);
  assert.equal(report.host.external_rollback_claimed, false, name);
  return report;
}
const allowed = run('authorized socket send', 'send-then-cancel', selected('submit.json'), approvedGrant);
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
const cancelled = run('cancel before native send', 'cancel-before-send', selected('submit.json'), approvedGrant);
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
  const refused = run(name, 'send-then-cancel', submit, selection);
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
  selected('cancel.json'), mismatchedSource, 'send-then-cancel'],
{ encoding: 'utf8', timeout: 120_000 });
assert.equal(wrongSource.status, 2, wrongSource.stderr || wrongSource.stdout);
const wrongReport = JSON.parse(wrongSource.stdout);
fs.writeFileSync(path.join(output, 'mismatched-source-grant.report.json'),
  wrongSource.stdout, { flag: 'wx', mode: 0o600 });
assert.equal(wrongReport.schema, 'noble-worker-send-wit-integration/v1');
assert.equal(wrongReport.outcome, 'internal-failure');
assert.equal(Object.hasOwn(wrongReport, 'host'), false,
  'a mismatched worker source grant must be refused before a native host exists');
console.log(JSON.stringify({ schema: 'noble-worker-send-wit-smoke/v1',
  world: 'noble-test:worker-send/worker-shell@1.0.0',
  scope: 'local-socket-only-not-canonical-WORKER-01',
  cases: ['authorized socket send', 'cancel before native send',
    'injected receiver error', 'injected receiver corruption',
    'native producer panic after send', 'rejected native cancellation',
    'unwound native cancellation', 'buffer quota preflight',
    'no-grant', 'mismatched-attempt-grant', 'expired-grant', 'candidate-generated-grant',
    'mismatched-source-grant'] }));
