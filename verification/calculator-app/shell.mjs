#!/usr/bin/env node
import { spawn } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import readline from 'node:readline';
import { TextDecoder } from 'node:util';
import { createHash } from 'node:crypto';
import { nominal, pair, result, variantIndex, rational, displayExact, diagnostic } from './display.mjs';
import { literal } from './literal.mjs';
import { boundedLines } from './transport.mjs';
import { auditSources, modulePaths, DEFAULT_RETAINED, OPTED_RETAINED,
  MAX_SOURCE, MAX_COMMAND_BYTES, RetainedNamespace,
  selectedCompilerReceipt, verifyRetainedSourceFlag } from './retention.mjs';

// Invocation: node shell.mjs NOBLE_BINARY NUMERIC_MODULE_DIR LANGUAGE_ROOT
//   [--retained-source-bytes 262144 --compiled-retention-receipt MA1_VERIFICATION_JSON]
// stdin: one JSON object per line: {op:"eval",expression},
// {op:"var",name,expression}, or {op:"function",name,parameters,body}.
// The shell neither tokenizes expressions nor performs calculator arithmetic.
const [binary, numericDir, languageRoot, ...providedArgs] = process.argv.slice(2);
const retainedArgs = [...providedArgs];
const verifyState = retainedArgs.at(-1) === '--verify-state';
if (verifyState) retainedArgs.pop();
const fixtureMode = retainedArgs.at(-1) === '--definition-fixtures';
if (fixtureMode) retainedArgs.pop();
const opted = retainedArgs.length === 4 &&
  retainedArgs[0] === '--retained-source-bytes' &&
  retainedArgs[1] === String(OPTED_RETAINED) &&
  retainedArgs[2] === '--compiled-retention-receipt' &&
  retainedArgs[3].length > 0;
if (!binary || !numericDir || !languageRoot || retainedArgs.length && !opted) {
  console.error('usage: shell.mjs NOBLE_BINARY NUMERIC_MODULE_DIR LANGUAGE_ROOT [--retained-source-bytes 262144 --compiled-retention-receipt MA1_VERIFICATION_JSON]');
  process.exit(2);
}
const here = import.meta.dirname;
const output = value => process.stdout.write(`${JSON.stringify(value)}\n`);
const stateDigest = value => createHash('sha256').update(JSON.stringify(value)).digest('hex');
const sourceDigest = bytes => createHash('sha256').update(bytes).digest('hex');
const frame = source => {
  const bytes = Buffer.isBuffer(source) ? source : Buffer.from(source);
  return Buffer.concat([Buffer.from(`${bytes.length}\n`), bytes]);
};
const audited = auditSources(modulePaths(languageRoot, numericDir, here),
  opted ? OPTED_RETAINED : DEFAULT_RETAINED);
const namespace = new RetainedNamespace(opted ? OPTED_RETAINED : DEFAULT_RETAINED);
const selected = JSON.parse(fs.readFileSync(path.join(languageRoot, 'policy/tool-selection.json')));
const tools = selected.tool_paths;
const toolPath = ['node', 'wasm_tools', 'binaryen']
  .map(name => path.join(tools[name].output, 'bin')).join(path.delimiter);
const guestEnv = { PATH: toolPath, LANG: 'C', LC_ALL: 'C',
  TMPDIR: process.env.TMPDIR ?? '/tmp' };
const selectedReceipt = opted
  ? selectedCompilerReceipt(binary, retainedArgs[3], numericDir, languageRoot, audited) : null;
const flagProof = opted ? verifyRetainedSourceFlag(binary, languageRoot, guestEnv) : null;
const guest = spawn(binary, ['session', '--framed', '--declared-modules',
  '--text-byte-cursor', '--signed-self-recursion',
  ...(opted ? ['--retained-source-bytes', String(OPTED_RETAINED)] : [])], {
  cwd: languageRoot, stdio: ['pipe', 'pipe', 'pipe'],
  env: guestEnv,
});
let stderr = '';
guest.stderr.on('data', chunk => { stderr = (stderr + chunk.toString('utf8')).slice(-8192); });
const reports = readline.createInterface({ input: guest.stdout, crlfDelay: Infinity })[Symbol.asyncIterator]();
async function submit(source, modulePlan = null) {
  const bytes = Buffer.isBuffer(source) ? source : Buffer.from(source);
  if (bytes.length > MAX_SOURCE) throw Error('guest submission source exceeds compiler bound');
  if (modulePlan) {
    if (modulePlan.kind !== 'module' || !modulePlan.sources[0].equals(bytes))
      throw Error('linked source differs from retained namespace plan');
  } else namespace.checkExpression(bytes);
  if (!guest.stdin.write(frame(source))) await new Promise(resolve => guest.stdin.once('drain', resolve));
  const next = await reports.next();
  if (next.done) throw Error(`Noble session stopped: ${stderr}`);
  if (Buffer.byteLength(next.value) > 8 * 1024 * 1024)
    throw Error('guest observation exceeds shell bound');
  const report = JSON.parse(next.value);
  if (report.host_requests !== 0 || report.guest_requests !== 0 ||
      report.acquired_authority !== false || report.outcome === 'trap' ||
      report.module?.imports?.some(entry => entry.kind === 'function')) {
    throw Error(`Noble authority/runtime refusal: ${JSON.stringify(report.diagnostic)}`);
  }
  return report;
}
function executed(report) {
  if (report.stage !== 'wasm' || report.outcome !== 'normal' ||
      !/^[0-9a-f]{64}$/.test(report.module?.wasm_sha256 ?? ''))
    throw Error('Noble guest lacks a normal source-compiled Wasm observation');
}
function guestFrame(role, source, report) {
  executed(report);
  const bytes = Buffer.from(source);
  if (report.module.source_sha256 !== sourceDigest(bytes) ||
      report.backend !== 'managed-linear-memory')
    throw Error('Noble guest did not execute the exact submitted source in managed Wasm');
  const metrics = report.metrics;
  if (metrics?.status !== 0 || metrics.quota_reason !== 0)
    throw Error('Noble managed-Wasm execution lacks normal physical metrics');
  for (const [name, ceiling] of [
    ['execution_steps', 100000], ['allocation_peak_bytes', 196608],
    ['continuation_peak_bytes', 4096]]) {
    if (!Number.isSafeInteger(metrics[name]) || metrics[name] < 0 || metrics[name] > ceiling)
      throw Error(`Noble managed-Wasm ${name} is absent or exceeds its physical ceiling`);
  }
  return { role, source_bytes: bytes.length, source_sha256: sourceDigest(bytes),
    wasm_sha256: report.module.wasm_sha256, backend: report.backend,
    metrics: { status: metrics.status, quota_reason: metrics.quota_reason,
      execution_steps: metrics.execution_steps,
      allocation_peak_bytes: metrics.allocation_peak_bytes,
      continuation_peak_bytes: metrics.continuation_peak_bytes } };
}
function fields(input, keys) {
  if (!input || typeof input !== 'object' || Array.isArray(input) ||
      Object.keys(input).some(key => !keys.includes(key))) throw Error('invalid command fields');
  for (const key of keys.filter(key => key !== 'display' && key !== 'budget'))
    if (!(key in input)) throw Error(`missing ${key}`);
}
const limits = { input: 32768, digits: 1024, bits: 4096, syntax: 64,
  calls: 256, work: 5000000, output: 8192 };
function budgetSource(budget) {
  const supplied = { ...limits, ...budget };
  if (!budget || typeof budget !== 'object' || Array.isArray(budget)) throw Error('invalid budget');
  if (Object.keys(budget).some(key => !(key in limits))) throw Error('unknown budget field');
  const values = Object.keys(limits).map(key => supplied[key]);
  if (values.some((v, i) => !Number.isSafeInteger(v) || v < 0 || v > Object.values(limits)[i]))
    throw Error('budget exceeds shell implementation ceiling');
  return `${values.join(' ')} numeric_budget@1.make`;
}
function parameterList(values) {
  if (!Array.isArray(values) || values.length > 32) throw Error('invalid parameter list');
  return values.slice().reverse().reduce((program, value) =>
    `${program} ${literal(value)} swap cons`, 'nil');
}
function importedDefinitions(values) {
  if (!Array.isArray(values) || values.length === 0 || values.length > 16)
    throw Error('invalid imported definition graph');
  return values.slice().reverse().reduce((program, value) => {
    fields(value, ['name', 'parameters', 'body']);
    return `${program} ${literal(value.name)} ${parameterList(value.parameters)} ${literal(value.body)} pair pair swap cons`;
  }, 'nil');
}
function proposedReply(report) {
  if (report.outcome !== 'normal' || report.stack?.length !== 1) throw Error('guest stage refused');
  const [, staged] = pair(nominal(report.stack[0]));
  const outcome = result(staged);
  if (!outcome.ok) return { error: diagnostic(outcome.payload) };
  const [candidate] = pair(outcome.payload);
  const [, reply] = pair(candidate);
  const [kind, payload] = variantIndex(reply);
  if (kind === 1) return { text: 'defined', definition: true };
  if (kind !== 0) throw Error('unexpected guest reply');
  return { fraction: rational(payload) };
}
async function main() {
  for (const { file, source, source_sha256 } of audited.modules) {
    const plan = namespace.planModule(source);
    const report = await submit(source, plan);
    if (report.outcome !== 'linked' || report.module?.source_sha256 !== source_sha256)
      throw Error(`guest module link/source identity refused: ${file}: ${JSON.stringify(report.diagnostic)}`);
    namespace.commit(plan);
  }
  if (namespace.moduleBytes !== audited.module_bytes ||
      namespace.outsideHistoryBytes !== audited.outside_history_bytes)
    throw Error('linked namespace retention differs from exact source audit');
  const initial = await submit('calc_session@1.empty');
  const initialFrame = guestFrame('empty', 'calc_session@1.empty', initial);
  if (initial.outcome !== 'normal' || initial.stack?.length !== 1) throw Error('guest state initialization failed');
  let stateSha = verifyState ? stateDigest(initial.stack[0]) : null;
  console.error(`calculator-source-receipt ${JSON.stringify({
    ...audited, modules: audited.modules.map(({ file, name, depends_on, bytes, source_sha256 }) =>
      ({ file, name, depends_on, bytes, source_sha256 })),
    compiled_flag_tested: flagProof !== null, flag_proof: flagProof,
    selected_compiled_receipt: selectedReceipt,
    initial_guest_frame: initialFrame,
    initial_state_sha256: stateSha,
  })}`);
  const reply = value => output(verifyState ? {
    ...value, state_sha256: stateSha,
    guest_frames: frames,
    namespace_retained_total_bytes: namespace.retainedTotalBytes,
    namespace_outside_history_bytes: namespace.outsideHistoryBytes,
  } : value);
  let previous = false, definitions = 0, storedBytes = 0;
  let frames = [];
  for await (const bytes of boundedLines(process.stdin, MAX_COMMAND_BYTES)) {
    frames = [];
    if (bytes === null) {
      reply({ error: 'InvalidCommand', message: 'command line exceeds transport bound' });
      continue;
    }
    let line;
    try { line = new TextDecoder('utf-8', { fatal: true }).decode(bytes); }
    catch {
      reply({ error: { category: 'SyntaxError', span: [0, bytes.length], provenance: 'unavailable' } });
      continue;
    }
    let command;
    try { command = JSON.parse(line); } catch { reply({ error: 'InvalidCommand' }); continue; }
    let source, isDefinition = false, ownBytes = 0, definitionCount = 0;
    try {
      if (command?.display !== undefined &&
          command.display !== 'fraction' && command.display !== 'finite-decimal')
        throw Error('unknown display mode');
      const prefix = previous ? 'drop ' : '';
      if (command.op === 'eval') {
        fields(command, ['op', 'expression', 'display', 'budget']);
        source = `${prefix}${literal(command.expression)} ${budgetSource(command.budget ?? {})} calc_session@1.stage_evaluate_budgeted`;
      } else if (command.op === 'var') {
        fields(command, ['op', 'name', 'expression', 'display', 'budget']);
        source = `${prefix}${literal(command.name)} ${literal(command.expression)} ${budgetSource(command.budget ?? {})} calc_session@1.stage_variable_budgeted`;
        ownBytes = Buffer.byteLength(command.name) + Buffer.byteLength(command.expression);
        isDefinition = true;
        definitionCount = 1;
      } else if (command.op === 'function') {
        fields(command, ['op', 'name', 'parameters', 'body', 'display', 'budget']);
        source = `${prefix}${literal(command.name)} ${parameterList(command.parameters)} ${literal(command.body)} ${budgetSource(command.budget ?? {})} calc_session@1.stage_function_budgeted`;
        ownBytes = [command.name, ...command.parameters, command.body]
          .reduce((sum, value) => sum + Buffer.byteLength(value), 0);
        isDefinition = true;
        definitionCount = 1;
      } else if (fixtureMode && command.op === 'import-fixture') {
        fields(command, ['op', 'definitions', 'budget']);
        source = `${prefix}${importedDefinitions(command.definitions)} ${budgetSource(command.budget ?? {})} calc_session@1.stage_imported_budgeted`;
        ownBytes = command.definitions.reduce((sum, item) =>
          sum + [item.name, ...item.parameters, item.body]
            .reduce((bytes, text) => bytes + Buffer.byteLength(text), 0), 0);
        isDefinition = true;
        definitionCount = command.definitions.length;
      } else throw Error('unknown command');
      if (isDefinition && (definitions + definitionCount > 32 || storedBytes + ownBytes > 16384))
        throw Error('session storage limit');
    } catch (error) { reply({ error: 'InvalidCommand', message: error.message }); continue; }
    const staged = await submit(source);
    frames.push(guestFrame('stage', source, staged));
    let candidate, text;
    try {
      candidate = proposedReply(staged);
      text = candidate.error ? '' : candidate.definition ? candidate.text :
        displayExact(...candidate.fraction, command.display);
    } catch (error) {
      const rollbackSource = 'calc_session@1.rollback';
      const restored = await submit(rollbackSource);
      frames.push(guestFrame('rollback', rollbackSource, restored));
      if (restored.outcome !== 'normal' || restored.stack?.length !== 2 ||
          restored.stack[1].type !== 'Unit')
        throw Error('guest rollback refused after formatter failure');
      if (verifyState && stateDigest(restored.stack[0]) !== stateSha)
        throw Error('guest rollback changed the prior environment');
      previous = true;
      reply({ error: { category: 'FormatterFailure', provenance: 'unavailable' } });
      continue;
    }
    // The output count is charged by the Noble guest before publication. No
    // new session state is selected if formatting or the charge fails.
    const commitSource = `${Buffer.byteLength(text)} calc_session@1.commit`;
    const committed = await submit(commitSource);
    frames.push(guestFrame('commit', commitSource, committed));
    if (committed.outcome !== 'normal' || committed.stack?.length !== 2)
      throw Error('guest commit refused; previous state cannot be assumed');
    previous = true;
    const outcome = result(committed.stack[1]);
    const observedState = verifyState ? stateDigest(committed.stack[0]) : null;
    if (!outcome.ok) {
      if (verifyState && observedState !== stateSha)
        throw Error('failed guest submission changed the prior environment');
      reply({ error: diagnostic(outcome.payload) });
      continue;
    }
    if (candidate.error) throw Error('guest committed a failed proposal');
    stateSha = observedState;
    if (isDefinition) { definitions += definitionCount; storedBytes += ownBytes; }
    reply(candidate.definition ? { accepted: true, display: text } :
      { accepted: true, display: text,
        fraction: candidate.fraction.map(v => String(v)) });
  }
  guest.stdin.end();
  const exit = await new Promise(resolve =>
    guest.once('close', (code, signal) => resolve({ code, signal })));
  if (exit.code !== 0 || exit.signal)
    throw Error(`Noble session exited abnormally: ${JSON.stringify(exit)}: ${stderr}`);
  if (!(await reports.next()).done)
    throw Error('Noble session emitted an unmatched guest report');
}
main().catch(error => {
  console.error(`calculator shell failed closed: ${error.stack ?? error}; guest stderr: ${stderr}`);
  guest.kill();
  process.exitCode = 1;
});
