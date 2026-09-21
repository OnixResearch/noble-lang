#!/usr/bin/env node
// Verification host only. The Rust CLI is the sole production shell/compiler.
// All preparation precedes the worker ready/input handshake; nothing compiles a tree.
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawn, spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { REPRESENTATIONS, OPTIMIZATIONS, CANONICAL_IDS, sha256, digest, equal, unknown,
  validateConfig, canonicalProjection, runtimeCases, aggregate, evaluateReport, runPolicyControls } from './m3-wasm-policy.mjs';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const CONFIG = 'crates/noble-cli/src/core/runtime/config.json';
const WORKER = path.join(ROOT, 'tools/m3-wasm-worker.mjs');
const fileHash = file => sha256(fs.readFileSync(file));
const writeJson = (file, value) => fs.writeFileSync(file, `${JSON.stringify(value, null, 2)}\n`, { flag: 'wx' });
const now = () => process.hrtime.bigint();
function argumentsForRun() {
  const options = { noble: process.env.NOBLE_M3_CLI, out: null };
  for (let index = 2; index < process.argv.length; index += 1) {
    const argument = process.argv[index];
    if (argument === '--help') return { help: true };
    if (!['--noble', '--out'].includes(argument) || !process.argv[index + 1]
      || process.argv[index + 1].startsWith('--')) throw Error(`invalid argument: ${argument}`);
    const key = argument.slice(2);
    if (key === 'out' && options.out !== null) throw Error('duplicate --out');
    options[key] = process.argv[++index];
  }
  return options;
}
function freshDirectory(requested) {
  if (!requested) return fs.mkdtempSync(path.join(os.tmpdir(), 'noble-m3-wasm-'));
  const destination = path.resolve(requested);
  fs.mkdirSync(path.dirname(destination), { recursive: true });
  fs.mkdirSync(destination); // Never replace/reuse a run, even an empty one.
  return destination;
}
function sourceSnapshot(config) {
  const names = new Set([...config.source_files, ...config.runtime_assets]);
  function visit(relative) {
    for (const entry of fs.readdirSync(path.join(ROOT, relative), { withFileTypes: true })) {
      const name = path.posix.join(relative, entry.name);
      if (entry.isSymbolicLink()) throw Error(`unbound source symlink: ${name}`);
      if (entry.isDirectory()) visit(name);
      else if (entry.isFile()) names.add(name);
    }
  }
  for (const directory of config.source_trees) visit(directory);
  const actualAssets = fs.readdirSync(path.join(ROOT, 'crates/noble-wasm/runtime'))
    .filter(file => file.endsWith('.wat')).map(file => `crates/noble-wasm/runtime/${file}`).sort();
  if (!equal(actualAssets, [...config.runtime_assets].sort())) throw Error('owned runtime WAT asset declarations are incomplete');
  const files = Object.fromEntries([...names].sort().map(name => {
    const absolute = path.join(ROOT, name);
    if (fs.lstatSync(absolute).isSymbolicLink()) throw Error(`source identity must not be a symlink: ${name}`);
    const bytes = fs.readFileSync(absolute);
    return [name, { bytes: bytes.length, sha256: sha256(bytes) }];
  }));
  return { classification: 'byte-manifest-not-compiler-inventory', compiler_inventory_claim: false,
    source_revision_kind: 'content-sha256-not-inferred-git-commit', sha256: digest(files), files,
    runtime_assets: config.runtime_assets.map(name => ({ path: name, ...files[name],
      classification: 'owned-runtime-source-outside-rust-extraction', proof_status: 'open',
      extraction_receipt_claim: false })) };
}
function workloadSnapshot(config) {
  const records = [];
  const sourceFiles = [];
  for (const [file, ids] of Object.entries(config.canonical_cases)) {
    const bytes = fs.readFileSync(path.join(ROOT, file));
    const parsed = JSON.parse(bytes);
    for (const id of ids) {
      const matches = parsed.cases.filter(row => row.id === id);
      if (matches.length !== 1) throw Error(`canonical case missing/duplicated in ${file}: ${id}`);
      records.push(matches[0]);
    }
    sourceFiles.push({ path: file, byte_sha256_at_read: sha256(bytes),
      identity_for_workload: 'selected-id-input-expected-projection-only; state/evidence-not-workload' });
  }
  const projection = canonicalProjection(records);
  const matrix = runtimeCases(projection);
  return { sha256: digest(projection), projection, canonical_ids: CANONICAL_IDS,
    runtime_case_ids: matrix.map(row => row.id), source_files: sourceFiles,
    excluded_from_workload_hash: ['state', 'evidence', 'requirements', 'document formatting'],
    oracle: { arithmetic: 'independent-JavaScript-BigInt.asIntN(64)-left-fold',
      recipes: 'exact-ordered-literal-and-definition-atom-comparison-not-hash-only',
      builtin_definition_mapping: { 4: 'builtin:i64.add', 6: 'builtin:i64.mul' },
      mapping_source: 'crates/noble-kernel/src/contracts/bootstrap/data.rs' } };
}

// Parse the actual binary framing, not WAT lengths or optimizer estimates. The
// 8-byte header and each section's id/LEB framing are accounted separately.
export function sectionAccounting(bytes) {
  if (bytes.length < 8 || !bytes.subarray(0, 8).equals(Buffer.from([0, 97, 115, 109, 1, 0, 0, 0]))) {
    throw Error('expected a version-1 core Wasm module');
  }
  function uleb(offset, end = bytes.length) {
    let value = 0;
    let shift = 0;
    const start = offset;
    for (let count = 0; count < 5; count += 1) {
      if (offset >= end) throw Error('truncated section length');
      const byte = bytes[offset++];
      if (count === 4 && (byte & 0xf0) !== 0) throw Error('section length overflows u32');
      value += (byte & 127) * 2 ** shift;
      if ((byte & 128) === 0) return { value, next: offset, bytes: offset - start };
      shift += 7;
    }
    throw Error('invalid section length');
  }
  const names = ['custom', 'type', 'import', 'function', 'table', 'memory', 'global', 'export',
    'start', 'element', 'code', 'data', 'data-count', 'tag'];
  const entries = [];
  let offset = 8;
  while (offset < bytes.length) {
    const start = offset;
    const id = bytes[offset++];
    const size = uleb(offset);
    offset = size.next;
    const end = offset + size.value;
    if (end > bytes.length || id > 13) throw Error('malformed Wasm section framing');
    let customName = null;
    if (id === 0) {
      const nameLength = uleb(offset, end);
      if (nameLength.next + nameLength.value > end) throw Error('truncated custom section name');
      customName = new TextDecoder('utf-8', { fatal: true })
        .decode(bytes.subarray(nameLength.next, nameLength.next + nameLength.value));
    }
    entries.push({ id, name: names[id], custom_name: customName, file_offset: start,
      payload_bytes: size.value, framing_bytes: offset - start, total_bytes: end - start,
      payload_sha256: sha256(bytes.subarray(offset, end)) });
    offset = end;
  }
  const types = entries.filter(section => section.id === 1);
  if (types.length !== 1) throw Error('expected one actual type section');
  return { header_bytes: 8, framing_bytes: entries.reduce((sum, entry) => sum + entry.framing_bytes, 0),
    payload_bytes: entries.reduce((sum, entry) => sum + entry.payload_bytes, 0), total_bytes: bytes.length,
    entries, type_section_sha256: types[0].payload_sha256 };
}

async function run(options) {
  const out = freshDirectory(options.out);
  fs.mkdirSync(path.join(out, 'logs'));
  let configBytes;
  let config;
  try {
    configBytes = fs.readFileSync(path.join(ROOT, CONFIG));
    config = JSON.parse(configBytes);
  } catch (error) {
    writeJson(path.join(out, 'report.json'), { schema_version: 1, result: 'failed', selected_representation: null,
      stage: 'configuration-read', output_directory: out, error: String(error),
      configuration_bytes_sha256: configBytes ? sha256(configBytes) : null });
    console.error(`M3-WASM configuration failed: ${path.join(out, 'report.json')}`);
    process.exitCode = 1;
    return;
  }
  const report = { schema_version: 1, scope: 'm3-resource-free-program-representation-comparison',
    started_at: new Date().toISOString(), output_directory: out, selected_representation: null,
    proof_status: 'open', configuration: config, configuration_sha256: digest(config),
    configuration_bytes_sha256: sha256(configBytes), source: null, workload: null, tools: {}, commands: [],
    integrity: { unchanged: false }, adapt08: null, adapt12: null,
    orchestrator: { executable: process.execPath, node: process.versions.node,
      v8: process.versions.v8, bun: process.versions.bun ?? null, platform: process.platform, arch: process.arch,
      os_release: os.release(), machine: os.machine(), cpu_model: os.cpus()[0]?.model ?? null,
      logical_cpus: os.cpus().length, total_host_memory_bytes: os.totalmem(),
      measurements_are_taken_in_separate_pinned_node_workers: true },
    configurations: REPRESENTATIONS.flatMap(representation => OPTIMIZATIONS.map(optimization => ({
      id: `${representation}/${optimization}`, representation, optimization, outcome: 'not-run',
      measured_success: false, worker: null }))), failures: [],
    trust_boundaries: [
      'Rust CLI preparation, checked lowering, and owned WAT runtime source are byte-bound; this harness proves no compiler/backend correspondence theorem.',
      'Owned WAT fragments are executable runtime source outside Rust extraction, with proof status open, not compiler-inventoried Rust definitions.',
      'Pinned wasm-tools, Binaryen, Node/V8, the OS, hardware, and this JavaScript oracle/measurement harness are explicit trusted tools.',
      'The supplied native CLI executable is byte-bound; correspondence between that executable and source is not established by this runner.',
      'Empty Wasm imports prevent guest host requests/compiler callbacks; process isolation is an experimental execution boundary, not a security sandbox proof.',
      'Logical 48-byte cell charges and cleared roots do not establish physical GC size, prompt reclamation, engine-only peak memory, resource ownership, ABI, or component conformance.',
      'Only a fixed checked pure vocabulary/interface set is compiled; this is not a general M4 source compiler, production JavaScript shell, or M5 resource/ABI completion.'
    ], performance_claim: false };
  let commandSequence = 0;
  const relative = absolute => path.relative(out, absolute).split(path.sep).join('/');
  function artifact(absolute) {
    const bytes = fs.readFileSync(absolute);
    return { path: relative(absolute), bytes: bytes.length, sha256: sha256(bytes) };
  }
  function command(label, binary, args) {
    const prefix = `${String(commandSequence++).padStart(3, '0')}-${label}`;
    const stdoutFile = path.join(out, 'logs', `${prefix}.stdout`);
    const stderrFile = path.join(out, 'logs', `${prefix}.stderr`);
    const started = now();
    const result = spawnSync(binary, args, { cwd: ROOT, env: process.env, timeout: config.timing.command_timeout_ms,
      maxBuffer: 32 * 1024 * 1024, windowsHide: true });
    const ns = Number(now() - started);
    fs.writeFileSync(stdoutFile, result.stdout ?? Buffer.alloc(0), { flag: 'wx' });
    fs.writeFileSync(stderrFile, result.stderr ?? Buffer.alloc(0), { flag: 'wx' });
    const receipt = { label, binary, args, elapsed_ns: ns, exit: result.status, signal: result.signal,
      error: result.error ? String(result.error) : null, stdout: artifact(stdoutFile), stderr: artifact(stderrFile) };
    report.commands.push(receipt);
    if (result.error || result.status !== 0) {
      const error = Error(`${label}: ${result.error ?? `exit ${result.status}, signal ${result.signal}`}`);
      error.receipt = receipt;
      throw error;
    }
    return { stdout: result.stdout ?? Buffer.alloc(0), receipt, ns };
  }
  function block(row, reason, evidence, partial = null) {
    row.outcome = 'blocked'; row.measured_success = false; row.worker = null;
    row.measurements = unknown('No complete measured execution; partial artifacts are retained separately, not successful performance evidence.');
    row.blocker = { reason, evidence };
    if (partial) row.partial_worker_observations = partial;
  }
  function errorEvidence(label, error) {
    const file = path.join(out, `${label}-failure.json`);
    writeJson(file, { message: error.message ?? String(error), stack: error.stack ?? null,
      command: error.receipt ?? null });
    return [artifact(file), ...(error.receipt ? [error.receipt.stdout, error.receipt.stderr] : [])];
  }
  function selectedTool(name, environment) {
    const pinned = config.tools[name];
    const requested = process.env[environment] ?? pinned.path;
    if (!path.isAbsolute(requested)) throw Error(`${environment} must select an absolute pinned executable`);
    const resolved = fs.realpathSync(requested);
    const expected = fs.realpathSync(pinned.path);
    if (resolved !== expected) throw Error(`${environment} differs from pinned executable: ${requested}`);
    return { requested_path: requested, pinned_path: pinned.path, realpath: resolved,
      sha256: fileHash(resolved), version: pinned.version, pin_matched: true };
  }
  function moduleRecord(file) {
    const bytes = fs.readFileSync(file);
    const sections = sectionAccounting(bytes);
    return { path: relative(file), bytes: bytes.length, sha256: sha256(bytes), sections,
      type_section_sha256: sections.type_section_sha256,
      debug_metadata_stripped: !sections.entries.some(section => section.id === 0
        && (section.custom_name === 'name' || section.custom_name?.startsWith('.debug'))),
      semantic_recipe_storage: 'retained-runtime-data-not-debug-metadata' };
  }
  const preparations = new Map();
  let featureProbes;
  try {
    validateConfig(config);
    for (const variable of ['NODE_OPTIONS', 'LD_PRELOAD', 'LD_LIBRARY_PATH']) {
      if (Object.hasOwn(process.env, variable)) throw Error(`unreviewed execution override: ${variable}`);
    }
    if (!options.noble) throw Error('select the built Rust CLI with --noble PATH or NOBLE_M3_CLI');
    report.source = sourceSnapshot(config);
    report.workload = workloadSnapshot(config);
    writeJson(path.join(out, 'source-manifest.json'), report.source);
    writeJson(path.join(out, 'workload.json'), report.workload);
    fs.writeFileSync(path.join(out, 'configuration.json'), configBytes, { flag: 'wx' });
    for (const [name, environment] of [['node', 'NOBLE_M3_NODE'], ['wasm_tools', 'NOBLE_M3_WASM_TOOLS'],
      ['wasm_opt', 'NOBLE_M3_WASM_OPT']]) report.tools[name] = selectedTool(name, environment);
    const cli = fs.realpathSync(path.resolve(options.noble));
    report.tools.noble = { requested_path: options.noble, realpath: cli, sha256: fileHash(cli),
      source_correspondence: 'not-established-by-this-harness' };
    const lock = JSON.parse(fs.readFileSync(path.join(ROOT, 'flake.lock'), 'utf8'));
    const locked = lock.nodes[config.nixpkgs.lock_node]?.locked;
    if (locked?.rev !== config.nixpkgs.revision || locked?.narHash !== config.nixpkgs.nar_hash) {
      throw Error('selected nixpkgs source differs from flake.lock');
    }
    report.tools.nixpkgs = { ...config.nixpkgs, lock_sha256: report.source.files['flake.lock'].sha256 };
    const node = report.tools.node.realpath;
    const wasmTools = report.tools.wasm_tools.realpath;
    const wasmOpt = report.tools.wasm_opt.realpath;
    const nodeVersion = command('node-version', node, ['-e',
      'process.stdout.write(JSON.stringify({node:process.versions.node,v8:process.versions.v8}))']);
    const observedNode = JSON.parse(nodeVersion.stdout.toString('utf8'));
    if (observedNode.node !== config.tools.node.version || observedNode.v8 !== config.tools.node.v8) {
      throw Error('selected Node/V8 version mismatch');
    }
    report.tools.node.v8 = observedNode.v8;
    report.tools.node.version_receipt = nodeVersion.receipt;
    const toolsVersion = command('wasm-tools-version', wasmTools, ['--version']);
    const optVersion = command('wasm-opt-version', wasmOpt, ['--version']);
    const observedTools = /^wasm-tools\s+(\S+)/.exec(toolsVersion.stdout.toString('utf8').trim())?.[1];
    const observedOpt = /^wasm-opt version\s+(\S+)/.exec(optVersion.stdout.toString('utf8').trim())?.[1];
    if (observedTools !== config.tools.wasm_tools.version || observedOpt !== config.tools.wasm_opt.version) {
      throw Error('assembler/optimizer version mismatch');
    }
    report.tools.wasm_tools.version_receipt = toolsVersion.receipt;
    report.tools.wasm_opt.version_receipt = optVersion.receipt;
    try {
      const rejected = command('adapt08-reject-owner-slot', cli, ['wasm-experiment', 'reject-owner-slot']);
      const observed = JSON.parse(rejected.stdout.toString('utf8'));
      const expected = report.workload.projection.find(row => row.id === 'ADAPT-08').expected;
      report.adapt08 = { correctness: equal(observed, expected) ? 'passed' : 'failed', observed,
        exit: rejected.receipt.exit, stdout_sha256: rejected.receipt.stdout.sha256,
        representations: REPRESENTATIONS, command: rejected.receipt };
    } catch (error) {
      report.adapt08 = { correctness: 'failed', error: String(error), evidence: errorEvidence('adapt08', error) };
      report.failures.push({ stage: 'ADAPT-08', message: String(error) });
    }
    const featureDirectory = path.join(out, 'feature-probes');
    fs.mkdirSync(featureDirectory);
    const probes = [
      ['gc', '(module (type $box (struct (field i32))) (func (export "probe") (result i32) i32.const 7 struct.new $box struct.get $box 0))', 7],
      ['function_references', '(module (type $sig (func (result i32))) (func $f (type $sig) i32.const 7) (elem declare func $f) (func (export "probe") (result i32) ref.func $f call_ref $sig))', 7],
      ['multi_value', '(module (func (export "probe") (result i32 i32) i32.const 1 i32.const 2))', [1, 2]],
      ['tail_call', '(module (func $f (result i32) i32.const 7) (func (export "probe") (result i32) return_call $f))', 7],
      ['component', '(component)', null],
    ];
    featureProbes = probes.map(([name, wat, expected]) => {
      const source = path.join(featureDirectory, `${name}.wat`);
      const binary = path.join(featureDirectory, `${name}.wasm`);
      fs.writeFileSync(source, `${wat}\n`, { flag: 'wx' });
      command(`feature-${name}-assembly`, wasmTools, ['parse', source, '-o', binary]);
      return { name, path: binary, sha256: fileHash(binary), source: artifact(source), expected };
    });
    report.feature_probe_assets = featureProbes.map(probe => ({ ...probe, path: relative(probe.path) }));

    // Repeat the entire preparation pipeline before any runtime inputs are sent.
    // Each repetition's source/module/log is retained; nondeterminism fails closed.
    for (const representation of REPRESENTATIONS) {
      const directory = path.join(out, representation);
      fs.mkdirSync(directory);
      const preparation = { cli: [], assembly: [], optimization: [], samples: [],
        shared_by_optimization_modes: true, all_requests_before_worker_runtime_inputs: true };
      let reference = null;
      try {
        for (const role of ['warmup', 'sample']) {
          const count = role === 'warmup' ? config.timing.warmups : config.timing.samples;
          for (let index = 0; index < count; index += 1) {
            const label = `${representation}-${role}-${index}`;
            const prefix = path.join(directory, `${role}-${index}`);
            const emitted = command(`${label}-cli`, cli, ['wasm-experiment', representation]);
            const watFile = `${prefix}.wat`;
            const rawFile = `${prefix}-off.wasm`;
            const optimizedFile = `${prefix}-on.wasm`;
            fs.writeFileSync(watFile, emitted.stdout, { flag: 'wx' });
            preparation.cli.push({ role, index, ns: emitted.ns, command: emitted.receipt.label });
            const assembled = command(`${label}-assembly`, wasmTools, ['parse', watFile, '-o', rawFile]);
            preparation.assembly.push({ role, index, ns: assembled.ns, command: assembled.receipt.label });
            const optimized = command(`${label}-optimization`, wasmOpt,
              [rawFile, ...config.optimizer_flags, '-o', optimizedFile]);
            preparation.optimization.push({ role, index, ns: optimized.ns, command: optimized.receipt.label });
            const current = { wat: artifact(watFile), off: moduleRecord(rawFile), on: moduleRecord(optimizedFile) };
            preparation.samples.push({ role, index, ...current });
            if (reference && ['wat', 'off', 'on'].some(key => reference[key].sha256 !== current[key].sha256)) {
              throw Error('preparation emitted nondeterministic bytes for the same checked vocabulary');
            }
            if (!reference) reference = current;
          }
        }
        for (const phase of ['cli', 'assembly', 'optimization']) {
          preparation[`${phase}_aggregate_ns`] = aggregate(preparation[phase].filter(row => row.role === 'sample').map(row => row.ns));
        }
        const selected = preparation.samples.find(row => row.role === 'sample' && row.index === 0);
        for (const mode of OPTIMIZATIONS) command(`${representation}-${mode}-validate`, wasmTools,
          ['validate', path.join(out, selected[mode].path)]);
        preparations.set(representation, { preparation, selected });
        writeJson(path.join(directory, 'preparation.json'), preparation);
      } catch (error) {
        const evidence = errorEvidence(`${representation}-preparation`, error);
        writeJson(path.join(directory, 'partial-preparation.json'), preparation);
        for (const row of report.configurations.filter(row => row.representation === representation)) {
          block(row, `preparation failed: ${error.message}`, evidence);
          row.preparation = preparation;
        }
        report.failures.push({ stage: `${representation}/preparation`, message: String(error) });
      }
    }
    for (const row of report.configurations) {
      const prepared = preparations.get(row.representation);
      if (!prepared) continue;
      row.preparation = prepared.preparation;
      row.module = prepared.selected[row.optimization];
      row.emitted_wat = prepared.selected.wat;
      const directory = path.join(out, row.representation, row.optimization);
      fs.mkdirSync(directory);
      const jobFile = path.join(directory, 'job.json');
      const job = { configuration: config, representation: row.representation, optimization: row.optimization,
        module: { ...row.module, path: path.join(out, row.module.path) }, feature_probes: featureProbes };
      writeJson(jobFile, job);
      const result = await executeWorker(node, config.node_flags, jobFile, directory, row.module, report.workload, config);
      row.worker_artifacts = result.artifacts.map(artifact);
      row.worker_process = result.process;
      if (!result.report || result.error) {
        block(row, result.error ?? 'worker terminated without a complete result', row.worker_artifacts,
          { ready: result.ready, cases: result.cases, fatal: result.fatal });
        report.failures.push({ stage: row.id, message: row.blocker.reason });
        continue;
      }
      row.outcome = 'executed';
      row.worker = result.report;
      row.measured_success = result.report.correctness === 'passed';
      row.protocol = result.protocol;
      if (!row.measured_success) report.failures.push({ stage: row.id, message: 'retained correctness failures; see worker observations' });
    }
    const reboundSource = sourceSnapshot(config);
    const reboundWorkload = workloadSnapshot(config);
    const toolsUnchanged = ['node', 'wasm_tools', 'wasm_opt', 'noble']
      .every(name => fileHash(report.tools[name].realpath) === report.tools[name].sha256);
    report.integrity = { unchanged: equal(report.source.files, reboundSource.files)
      && report.workload.sha256 === reboundWorkload.sha256 && toolsUnchanged,
    rebound_source_sha256: reboundSource.sha256, rebound_workload_sha256: reboundWorkload.sha256,
    tool_executable_bytes_unchanged: toolsUnchanged,
    canonical_state_evidence_edits_do_not_change_workload_identity: true };
    const policyDirectory = path.join(out, 'policy-controls');
    fs.mkdirSync(policyDirectory);
    report.adapt12 = runPolicyControls(report, (index, fixture) => {
      const file = path.join(policyDirectory, `adapt12-${index}.json`);
      writeJson(file, fixture);
      return artifact(file);
    });
  } catch (error) {
    const evidence = errorEvidence('run', error);
    report.failures.push({ stage: 'configuration-or-host', message: String(error), evidence });
    for (const row of report.configurations.filter(row => row.outcome === 'not-run')) {
      block(row, `run prerequisite failed: ${error.message}`, evidence);
    }
  }
  report.finished_at = new Date().toISOString();
  report.policy = evaluateReport(report);
  report.result = report.policy.eligible_for_selection ? 'eligible-for-selection' : 'failed';
  report.artifact_manifest = listArtifacts(out).map(artifact);
  // The report itself is not self-hashed. Every retained input/module/log/job/result
  // that precedes it is byte-bound in the manifest; failed observations are included.
  writeJson(path.join(out, 'report.json'), report);
  console.log(`M3-WASM ${report.result}: ${path.join(out, 'report.json')}`);
  if (!report.policy.eligible_for_selection) process.exitCode = 1;
}

function listArtifacts(directory) {
  return fs.readdirSync(directory, { withFileTypes: true }).flatMap(entry => {
    const file = path.join(directory, entry.name);
    return entry.isDirectory() ? listArtifacts(file) : [file];
  }).sort();
}

async function executeWorker(node, flags, jobFile, directory, module, workload, config) {
  const stdoutFile = path.join(directory, 'worker.jsonl');
  const stderrFile = path.join(directory, 'worker.stderr');
  const outputFile = path.join(directory, 'worker-result.json');
  const inputFile = path.join(directory, 'post-compilation-input.json');
  const stdout = fs.openSync(stdoutFile, 'wx');
  const stderr = fs.openSync(stderrFile, 'wx');
  const started = now();
  const received = { report: null, ready: null, cases: [], fatal: null, error: null,
    protocol: { ready_observed: false, inputs_sent_after_ready: false, module_sha256_before_inputs: module.sha256 },
    artifacts: [stdoutFile, stderrFile] };
  const child = spawn(node, [...flags, WORKER, jobFile], { cwd: ROOT, env: process.env,
    stdio: ['pipe', 'pipe', 'pipe'], windowsHide: true });
  let pending = '';
  let totalBytes = 0;
  let stderrBytes = 0;
  let inputsSent = false;
  let timeout;
  function stop(message) {
    if (!received.error) received.error = message;
    child.kill('SIGKILL');
  }
  const completion = new Promise(resolve => {
    child.on('error', error => { received.error = String(error); });
    child.stdin.on('error', error => { if (!received.report) stop(`worker input pipe: ${error.message}`); });
    child.stdout.on('data', chunk => {
      fs.writeSync(stdout, chunk);
      totalBytes += chunk.length;
      if (totalBytes > 128 * 1024 * 1024) { stop('worker output exceeded retained evidence bound'); return; }
      pending += chunk.toString('utf8');
      let newline;
      while ((newline = pending.indexOf('\n')) !== -1) {
        const line = pending.slice(0, newline);
        pending = pending.slice(newline + 1);
        if (!line) continue;
        try {
          const message = JSON.parse(line);
          if (message.type === 'ready') {
            if (received.ready || inputsSent || message.module_sha256 !== module.sha256
              || message.type_section_sha256 !== module.type_section_sha256 || !equal(message.imports, [])
              || message.node?.version !== config.tools.node.version || message.node?.v8 !== config.tools.node.v8
              || !equal(message.node?.flags, flags)) throw Error('invalid/repeated ready receipt');
            received.ready = message;
            received.protocol.ready_observed = true;
            received.protocol.ready_after_spawn_ns = Number(now() - started);
            const input = { kind: 'run', workload_sha256: workload.sha256, projection: workload.projection };
            writeJson(inputFile, input);
            received.artifacts.push(inputFile);
            inputsSent = true;
            received.protocol.inputs_sent_after_ready = true;
            received.protocol.input_sent_after_spawn_ns = Number(now() - started);
            received.protocol.input_sha256 = fileHash(inputFile);
            child.stdin.end(`${JSON.stringify(input)}\n`);
          } else if (message.type === 'case') {
            if (!inputsSent || received.report) throw Error('case outside post-compilation execution phase');
            received.cases.push(message.case);
          } else if (message.type === 'result') {
            if (!inputsSent || received.report) throw Error('result outside execution phase');
            received.report = message.report;
          } else if (message.type === 'fatal') received.fatal = message.error;
          else throw Error('unknown worker protocol message');
        } catch (error) { stop(`worker protocol failure: ${error.message}`); }
      }
    });
    child.stderr.on('data', chunk => {
      fs.writeSync(stderr, chunk);
      stderrBytes += chunk.length;
      if (stderrBytes > 16 * 1024 * 1024) stop('worker stderr exceeded retained evidence bound');
    });
    child.on('close', (exit, signal) => {
      clearTimeout(timeout);
      fs.closeSync(stdout); fs.closeSync(stderr);
      if (pending.trim()) received.error ??= 'worker ended with an incomplete JSON record';
      if (received.fatal) received.error ??= `worker fatal error: ${received.fatal.message}`;
      if (received.report && (signal || exit !== (received.report.correctness === 'passed' ? 0 : 1))) {
        received.error ??= `worker result/exit mismatch: ${exit}/${signal}`;
      }
      if (!received.report) received.error ??= `worker did not finish: exit=${exit}, signal=${signal}`;
      if (received.report && !equal(received.report.cases, received.cases)) received.error ??= 'worker dropped or rewrote streamed cases';
      received.process = { executable: node, flags, pid: child.pid ?? null, exit, signal,
        elapsed_ns: Number(now() - started), timeout_ms: config.timing.worker_timeout_ms,
        isolated_engine_heap_peak_bytes: unknown('Process RSS includes the host and is not engine-only memory.') };
      writeJson(outputFile, { report: received.report, protocol: received.protocol, process: received.process,
        error: received.error, fatal: received.fatal });
      received.artifacts.push(outputFile);
      resolve(received);
    });
  });
  timeout = setTimeout(() => stop('worker timeout; no quota/normal outcome inferred'), config.timing.worker_timeout_ms);
  return completion;
}

try {
  const options = argumentsForRun();
  if (options.help) console.log('usage: node tools/m3-wasm.mjs [--noble PATH] [--out NEW_DIRECTORY]\nEnvironment: NOBLE_M3_CLI, NOBLE_M3_NODE, NOBLE_M3_WASM_TOOLS, NOBLE_M3_WASM_OPT');
  else await run(options);
} catch (error) {
  console.error(`M3-WASM failed before report creation: ${error.stack ?? error}`);
  process.exitCode = 1;
}
