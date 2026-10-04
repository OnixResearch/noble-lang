import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileHash } from './record.mjs';

export class Runtime {
  constructor(record, plan, cli, wasmTools, environment) {
    this.record = record;
    this.plan = plan;
    this.cli = cli;
    this.wasmTools = wasmTools;
    this.environment = environment;
  }

  asset(caseId, variant, group = 'variants') {
    const entry = group === 'baseline'
      ? this.plan.inputs[caseId]?.baseline
      : this.plan.inputs[caseId]?.[group]?.[variant];
    assert.ok(entry, `not a frozen source: ${caseId}/${group}/${variant}`);
    const file = path.join(this.record.output, entry.file);
    assert.equal(fileHash(file), entry.sha256, `${caseId}/${variant}: frozen source changed`);
    return file;
  }

  binding() {
    const entry = this.plan.bindings.empty;
    const file = path.join(this.record.output, entry.file);
    assert.equal(fileHash(file), entry.sha256, 'frozen host bindings changed');
    return file;
  }

  session(label, files, optimization, { environment = {} } = {}) {
    assert.ok(['off', 'on'].includes(optimization), 'unreviewed Wasm optimizer mode');
    const directory = path.join(this.record.output, 'emission', label);
    assert.ok(!fs.existsSync(directory), `${label}: emission must be fresh`);
    fs.mkdirSync(path.dirname(directory), { recursive: true });
    const args = ['session', '--framed', '--declared-modules', '--bindings', this.binding(),
      '--opt', optimization, '--emit', directory];
    const run = this.record.command(label, this.cli, args, {
      input: this.record.framed(files), env: { ...this.environment, ...environment },
      timeout: 900_000,
    });
    const lines = run.bytes.toString('utf8').trimEnd().split('\n');
    assert.equal(lines.length, files.length, `${label}: wrong actual framed report count`);
    const reports = lines.map((line, index) => {
      const report = JSON.parse(line);
      assert.equal(report.schema, 'noble-core-report/v1', `${label}/${index}: wrong CLI report`);
      assert.equal(report.profile, 'Declared-Modules-v1', `${label}/${index}: wrong module runtime`);
      return report;
    });
    return { command: run.id, status: run.status, reports,
      artifacts: this.record.emission(label, directory, files) };
  }

  quiet(report, label) {
    for (const key of ['guest_requests', 'host_requests', 'protected_operations',
      'ambient_fallback_calls', 'candidate_prepare_requests'])
      assert.equal(report[key], 0, `${label}: ${key} during proof/link admission`);
    assert.equal(report.acquired_authority, false, `${label}: source proof acquired authority`);
    assert.equal(report.prior_stack, 'unchanged', `${label}: declaration mutated guest stack`);
  }

  linked(report, name, version, label) {
    this.quiet(report, label);
    assert.equal(report.stage, 'link', `${label}: no module link report`);
    assert.equal(report.outcome, 'linked', `${label}: module was not published`);
    assert.equal(report.resolved_module?.name, name, `${label}: wrong module name`);
    assert.equal(report.resolved_module?.version, version, `${label}: wrong version`);
    assert.match(report.resolved_module?.identity, /^[0-9]+$/u,
      `${label}: no exact decimal module identity`);
    return report.resolved_module.identity;
  }

  invocation(report, values, optimization, label) {
    assert.equal(report.stage, 'wasm', `${label}: no actual compiled guest invocation`);
    assert.equal(report.outcome, 'normal', `${label}: guest failed`);
    assert.equal(report.status, 0, `${label}: guest returned nonzero`);
    assert.equal(report.module?.optimized, optimization === 'on',
      `${label}: wrong Wasm optimizer mode`);
    assert.ok(Array.isArray(report.stack), `${label}: no guest output stack`);
    if (values !== null)
      assert.deepEqual(report.stack.map(value => [value.type, value.value]),
        values.map(value => ['I64', value]), `${label}: wrong ordered compiled output`);
    for (const key of ['guest_requests', 'host_requests', 'protected_operations',
      'ambient_fallback_calls']) assert.equal(report[key], 0, `${label}: protected effect`);
    assert.deepEqual(report.effect_requests, [], `${label}: unexpected effect request`);
    assert.deepEqual(report.request_trace, [], `${label}: unexpected host request trace`);
    assert.equal(report.request_trace_complete, true, `${label}: truncated host trace`);
    assert.equal(report.effect_requests_unrecorded, 0, `${label}: missing effect observations`);
    assert.equal(report.runtime_prover_calls, 0,
      `${label}: compiled guest invoked intrinsic proof service`);
    assert.equal(report.acquired_authority, false, `${label}: proof supplied runtime authority`);
    assert.equal(report.native_trap, null, `${label}: guest trapped`);
  }

  checkedWasm(label, observed) {
    const files = this.record.wasm(label, this.wasmTools, observed.artifacts);
    return files.map(item => ({ file: item.file, sha256: item.sha256,
      validation_command: item.validate_command, inspection_command: item.print_command }));
  }
}
