#!/usr/bin/env node
// Binds independently checked production traces to a clean source and exact tools.
// Usage: node verification/m7/trace130/run.mjs NEW_EXTERNAL_DIR CARGO_EXECUTABLE TARGET_DIR RUSTC_EXECUTABLE
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = fs.realpathSync(path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../..'));
const [directory, cargoArgument, targetArgument, rustcArgument] = process.argv.slice(2);
assert.ok(directory && cargoArgument && targetArgument && rustcArgument,
  'usage: node verification/m7/trace130/run.mjs NEW_EXTERNAL_DIR CARGO_EXECUTABLE TARGET_DIR RUSTC_EXECUTABLE');
const out = path.resolve(directory);
assert.ok(!out.startsWith(`${root}${path.sep}`) && out !== root, 'evidence must be outside source');
assert.ok(!fs.existsSync(out), 'evidence directory must be fresh');
const cargo = fs.realpathSync(cargoArgument);
const rustc = fs.realpathSync(rustcArgument);
const target = path.resolve(targetArgument);
assert.ok(path.isAbsolute(targetArgument), 'target must be an absolute private directory');
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const hashFile = file => hash(fs.readFileSync(file));
const exec = (executable, args, options = {}) => spawnSync(executable, args, {
  cwd: root, encoding: 'utf8', maxBuffer: 128 * 1024 * 1024,
  env: { ...process.env, NOBLE_ROOT: root, NOBLE_TARGET: target }, ...options,
});
const git = (...args) => {
  const result = exec('git', args);
  assert.equal(result.status, 0, `git ${args.join(' ')} failed: ${result.stderr}`);
  return result.stdout.trim();
};
assert.equal(git('status', '--porcelain'), '', 'receipt producer requires clean committed tree');
const source = git('rev-parse', 'HEAD');
const sourcePaths = [
  'crates/noble-kernel/src/dataspace/mod.rs',
  'crates/noble-kernel/src/dataspace/table.rs',
  'crates/noble-kernel/src/dataspace/table/actions.rs',
  'crates/noble-kernel/tests/m7-dataspace.rs',
  'crates/noble-kernel/tests/m7-dataspace/trace130.rs',
  'crates/noble-kernel/tests/m7-dataspace/trace130/reference.rs',
  'crates/noble-kernel/tests/m7-dataspace/trace130/reference/steps.rs',
  'crates/noble-kernel/tests/m7-dataspace/trace130/generated.rs',
  'crates/noble-kernel/tests/m7-dataspace/trace130/generated/boundary.rs',
  'crates/noble-kernel/tests/m7-dataspace/trace130/faults.rs',
  'crates/noble-syndicate/src/lib.rs',
  'crates/noble-syndicate/tests/service.rs',
  'crates/noble-syndicate/tests/service/trace130.rs',
  'verification/m7/trace130/profile.md',
  'verification/m7/trace130/run.mjs',
  'verification/m7/acceptance.json',
  'verification/m7/evidence.json',
];
const sources = Object.fromEntries(sourcePaths.map(file => [file, hashFile(path.join(root, file))]));
fs.mkdirSync(out);
const retain = (file, data) => {
  const bytes = typeof data === 'string' ? data : JSON.stringify(data, null, 2) + '\n';
  fs.writeFileSync(path.join(out, file), bytes, { flag: 'wx' });
  return { path: file, sha256: hash(bytes), bytes: Buffer.byteLength(bytes) };
};
const commands = [];
function run(label, crate, test, filter) {
  const args = ['test', '-p', crate, '--test', test, '--offline', '--locked', filter,
    '--', '--nocapture', '--test-threads=1'];
  const result = exec(cargo, args);
  const stdout = retain(`${label}.stdout`, result.stdout ?? '');
  const stderr = retain(`${label}.stderr`, result.stderr ?? '');
  commands.push({ label, args, status: result.status, signal: result.signal, error: String(result.error ?? ''), stdout, stderr });
  return result.stdout ?? '';
}
const kernel = run('kernel', 'noble-kernel', 'm7-dataspace', 'trace130');
const host = run('host', 'noble-syndicate', 'service', 'trace130');
function lines(output, marker, fields) {
  return output.split(/\r?\n/).filter(line => line.includes(`${marker}\t`)).map(line => {
    // libtest may print its "test ... " prefix on the first nocapture row.
    const columns = line.slice(line.indexOf(`${marker}\t`)).split('\t');
    assert.equal(columns.length, fields, `malformed ${marker} receipt: ${line}`);
    return columns;
  });
}
function traceRow([, id, limits, history, expected, actual]) {
  const actions = history ? history.split(';') : [];
  const expectedTrace = expected ? expected.split(';') : [];
  const actualTrace = actual ? actual.split(';') : [];
  assert.equal(expectedTrace.length, actions.length, `expected steps for ${id}`);
  assert.equal(actualTrace.length, actions.length, `actual steps for ${id}`);
  return { id, limits, actions, expected: expectedTrace, actual: actualTrace,
    agrees: expected === actual };
}
const histories = lines(kernel, 'M7TRACE', 6).map(traceRow);
const faults = lines(kernel, 'M7FAULT', 10).map(([, name, trigger, limits, full, reduced, fullExpected, fullActual, reducedExpected, reducedActual]) => ({
  name, trigger, limits, full: { actions: full.split(';'), expected: fullExpected.split(';'), actual: fullActual.split(';') },
  reduced: { actions: reduced.split(';'), expected: reducedExpected.split(';'), actual: reducedActual.split(';') },
}));
const hosts = lines(host, 'M7HOST', 4).map(([, action, expected, actual]) => ({ action, expected, actual }));
const historiesFile = retain('histories.jsonl', histories.map(row => JSON.stringify(row)).join('\n') + '\n');
const faultsFile = retain('faults.jsonl', faults.map(row => JSON.stringify(row)).join('\n') + '\n');
const hostFile = retain('host.jsonl', hosts.map(row => JSON.stringify(row)).join('\n') + '\n');
const version = (executable, args) => {
  const result = exec(executable, args);
  assert.equal(result.status, 0, `version invocation failed: ${result.stderr}`);
  return result.stdout.trim();
};
const summary = {
  schema: 'noble-m7-trace130/v1', source_commit: source, dirty: false,
  sources, tools: {
    cargo: { path: cargo, sha256: hashFile(cargo), version: version(cargo, ['--version']) },
    rustc: { path: rustc, sha256: hashFile(rustc), version: version(rustc, ['-Vv']) },
    node: { path: process.execPath, sha256: hashFile(process.execPath), version: process.version },
  },
  target, commands, bounds: {
    enumeration: { alphabet: 6, lengths: [0, 1, 2, 3, 4], expected: 1555,
      limits: { facets: 3, assertions: 2, interests: 1, events: 8 } },
    seeded: { seeds: ['00000000130a1301', '00000000130b1302', '00000000130c1303'],
      historiesPerSeed: 96, generatedSteps: 16,
      limits: { facets: 8, assertions: 8, interests: 8, events: 32 } },
  },
  counts: { enumeration: histories.filter(row => row.id.startsWith('enum/')).length,
    seeded: histories.filter(row => row.id.startsWith('seed/')).length,
    named: histories.filter(row => row.id.startsWith('boundary/')).length,
    baselineFaultControls: histories.filter(row => row.id.startsWith('fault-control/')).length,
    faults: faults.length, hostSteps: hosts.length,
    disagreements: histories.filter(row => !row.agrees).length },
  artifacts: { histories: historiesFile, faults: faultsFile, host: hostFile },
  priorCompiledAbnormalExit: {
    source: 'verification/m7/acceptance.json', sha256: sources['verification/m7/acceptance.json'],
    scope: 'Historical compiled two-component M7 acceptance only; not freshly executed by this lane.',
  },
  claims: {
    finite_comparison: 'Generated Table histories and selected Profile host calls only',
    new_model_theorem: 'none', whole_table_extraction_or_refinement: 'none',
    host_assumptions: 'Host-issued identities, serialized calls, drop execution, engine and OS fidelity',
    unproved_edges: 'Whole-Table universal refinement, SAM turn semantics, async delivery, scheduler and transport',
  },
};
const good = commands.every(command => command.status === 0 && !command.signal && !command.error)
  && summary.counts.enumeration === 1555 && summary.counts.seeded === 288
  && summary.counts.named === 8 && summary.counts.baselineFaultControls === 5
  && summary.counts.faults === 5 && summary.counts.hostSteps >= 12
  && summary.counts.disagreements === 0
  && faults.every(row => row.trigger.length > 0 && row.full.expected.join(';') !== row.full.actual.join(';')
    && row.reduced.expected.join(';') !== row.reduced.actual.join(';')
    && row.reduced.actions.length <= row.full.actions.length);
summary.result = good ? 'passed' : 'failed';
retain('receipt.json', summary);
console.log(`m7trace130 result=${summary.result} enum=${summary.counts.enumeration} seed=${summary.counts.seeded} named=${summary.counts.named} faults=${summary.counts.faults} host=${summary.counts.hostSteps} disagreements=${summary.counts.disagreements}`);
assert.ok(good, `comparison failed; retained ${out}/receipt.json`);
