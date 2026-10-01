import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileHash } from './record.mjs';

export class ProofRunner {
  constructor(record, plan, cli, environment) {
    this.record = record;
    this.plan = plan;
    this.cli = cli;
    this.environment = environment;
  }

  asset(caseId, name, group = 'variants') {
    const entry = group === 'baseline'
      ? this.plan.inputs[caseId]?.baseline
      : this.plan.inputs[caseId]?.[group]?.[name];
    assert.ok(entry, `unreviewed proof source ${caseId}/${group}/${name}`);
    const file = path.join(this.record.output, entry.file);
    assert.equal(fileHash(file), entry.sha256, `${caseId}/${name}: modified source`);
    return file;
  }

  verify(label, source, { modules = [], timeoutMs = 600_000,
    flags = [], environment = {} } = {}) {
    assert.ok(fs.lstatSync(source).isFile(), `${label}: actual source file required`);
    this.record.watch(source);
    assert.ok(Array.isArray(modules) && modules.every(file => fs.lstatSync(file).isFile()),
      `${label}: dependencies must be real module files`);
    modules.forEach(file => this.record.watch(file));
    assert.ok(Number.isSafeInteger(timeoutMs) && timeoutMs > 0 && timeoutMs <= 600_000,
      `${label}: unreviewed proof timeout`);
    const args = ['verify-module', source,
      ...modules.flatMap(file => ['--module', file]), '--timeout-ms', String(timeoutMs), ...flags];
    const run = this.record.command(label, this.cli, args,
      { env: { ...this.environment, ...environment }, timeout: timeoutMs + 60_000 });
    const text = run.bytes.toString('utf8').trim();
    assert.ok(text && text.startsWith('{') && text.endsWith('}'),
      `${label}: no actual production JSON proof report`);
    const report = JSON.parse(text);
    assert.equal(report.schema, 'noble-intrinsic-module-report/v1',
      `${label}: wrong independent source-proof report`);
    assert.equal(report.profile, 'Intrinsic-Proofs-Draft', `${label}: wrong proof profile`);
    assert.equal(report.command, 'verify-module', `${label}: wrong proof command`);
    assert.ok(typeof report.outcome === 'string', `${label}: missing proof outcome`);
    return { command: run.id, exit: run.status, report };
  }
}
