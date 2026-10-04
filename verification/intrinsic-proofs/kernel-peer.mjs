import assert from 'node:assert/strict';
import path from 'node:path';
import { fileHash } from './record.mjs';
import { accepted } from './proof-cases.mjs';

export function runKernelPeer(context) {
  const id = 'CONTRACT-25', from = (name, caseId = id) => {
    const entry = context.plan.inputs[caseId].extras[name];
    assert.ok(entry, `${caseId}: peer lacks source ${name}`);
    const file = context.proof.asset(caseId, name, 'extras');
    return { entry, file };
  };
  const sources = {
    exported_v1: from('accepted-exported-proof'),
    exported_v2: from('rebound-exported-proof'),
    import_v1: from('import-version-1'),
    import_v2: from('import-version-2'),
    consumer: from('accepted-imported-use'),
    private_v1: from('private-proof'),
    untrusted_axiom: from('untrusted-axiom-module', 'CONTRACT-21'),
  };
  const verification = {};
  for (const [key, name] of Object.entries({ exported_v1: 'poly-refl',
    exported_v2: 'poly-refl', private_v1: 'poly-refl' })) {
    const run = context.proof.verify(`peer-${key}`, sources[key].file);
    accepted(run, sources[key].entry, name, `peer-${key}`, context);
    verification[key] = run;
  }
  const consumer = context.proof.verify('peer-consumer-v2', sources.consumer.file,
    { modules: [sources.exported_v2.file, sources.import_v2.file] });
  accepted(consumer, sources.consumer.entry, 'imported-list', 'peer-consumer-v2', context);
  verification.consumer_v2 = consumer;
  const verdicts = Object.fromEntries(Object.entries(verification).map(([name, row]) => {
    const command = context.record.receipt.commands[row.command - 1];
    assert.equal(command.id, row.command, 'reported proof observation not retained');
    return [name, path.join(context.record.output, command.stdout)];
  }));
  const workload = {
    schema: 'noble-intrinsic-proofs-peer-workload/v1',
    sources: Object.fromEntries(Object.entries(sources).map(([key, row]) => [key, row.file])),
    source_sha256: Object.fromEntries(Object.entries(sources).map(([key, row]) =>
      [key, row.entry.sha256])), verdicts,
    verdict_commands: Object.fromEntries(Object.entries(verification).map(([key, row]) =>
      [key, row.command])),
  };
  const workloadFile = context.record.retain('peer/workload.json',
    Buffer.from(JSON.stringify(workload, null, 2) + '\n'));
  const run = context.record.command('independent-production-module-session-peer',
    context.tools.peer, [workloadFile], { env: context.environment, timeout: 300_000 });
  assert.equal(run.status, 0, 'production ModuleSession rejected independently checked proof workflow');
  const observed = JSON.parse(run.bytes.toString('utf8'));
  assert.equal(observed.schema, 'noble-intrinsic-proofs-kernel-peer/v1');
  assert.equal(observed.result, 'passed');
  assert.equal(observed.workload_sha256, fileHash(workloadFile));
  assert.equal(observed.stale?.guest_requests, 0);
  assert.equal(observed.stale?.protected_operations, 0);
  assert.equal(observed.visibility?.guest_requests, 0);
  assert.equal(observed.visibility?.protected_operations, 0);
  assert.equal(observed.transitive?.guest_requests, 0);
  assert.equal(observed.transitive?.protected_operations, 0);
  assert.equal(observed.transitive?.forged_nested_dependency_rejected, true,
    'forged transitive axiom reached admission');
  assert.equal(observed.stale?.old_dependency?.source_sha256, sources.exported_v1.entry.sha256,
    'stale proof was not bound to original owner source');
  assert.equal(observed.stale?.new_dependency?.source_sha256, sources.exported_v2.entry.sha256,
    'fresh proof did not bind rebound source bytes');
  assert.notEqual(sources.exported_v1.entry.sha256, sources.exported_v2.entry.sha256,
    'old and new proof owner have identical bytes');
  return { ...observed, command: run.id,
    strict_verdict_commands: Object.fromEntries(Object.entries(verification)
      .map(([key, row]) => [key, row.command])) };
}
