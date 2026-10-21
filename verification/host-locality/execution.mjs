// Build the current bounded-region CLI offline and execute the selected WIT guest.
// Mutated core modules are fault-injection controls over the emitted component,
// not alternate production adapters or proofs of general Wasmtime refinement.
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import {
  lstatSync, mkdirSync, readFileSync, readdirSync, realpathSync, writeFileSync,
} from 'node:fs';
import {
  basename, dirname, join, relative, resolve, sep,
} from 'node:path';
import { fileURLToPath } from 'node:url';
import { selectedVendor } from '../selected-vendor.mjs';

const root = resolve(fileURLToPath(import.meta.url), '../../..');
const witRelative = 'crates/noble-wasm/wit/bounded-region.wit';
const worldName = 'bounded';
const worldId = 'noble-test:bounded-region/bounded@1.0.0';
const packageId = 'noble-test:bounded-region@1.0.0';
const fixtureRelative = 'verification/host-locality/fixtures/read-release.noble';
const json = value => `${JSON.stringify(value, null, 2)}\n`;
const stable = value => JSON.stringify(value);
const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const fileHash = path => digest(readFileSync(path));
const requireThat = (condition, message) => { if (!condition) throw Error(message); };
const writeJson = (path, value) => writeFileSync(path, json(value));
const relativeOutputPath = (out, path) => relative(out, path).split(sep).join('/');

let out = null;
let baseEnv = null;
let commands = [];
let cases = [];
let controls = [];
let commandNumber = 0;

function run(label, executable, argv, options = {}) {
  const { allowFailure = false, env = baseEnv, cwd = root, ...spawnOptions } = options;
  const id = String(++commandNumber).padStart(3, '0');
  const captureDirectory = join(out, 'commands', id);
  mkdirSync(captureDirectory);
  let executablePath = executable;
  let executableSha256 = null;
  try {
    executablePath = realpathSync(executable);
    executableSha256 = fileHash(executablePath);
  } catch {}
  const raw = spawnSync(executable, argv, {
    cwd, env, encoding: null, maxBuffer: 64 * 1024 * 1024, timeout: 3_600_000,
    ...spawnOptions,
  });
  const stdoutBytes = raw.stdout ?? Buffer.alloc(0);
  const stderrBytes = raw.stderr ?? Buffer.alloc(0);
  const stdoutPath = join(captureDirectory, 'stdout');
  const stderrPath = join(captureDirectory, 'stderr');
  writeFileSync(stdoutPath, stdoutBytes);
  writeFileSync(stderrPath, stderrBytes);
  const record = {
    id, label, executable: executablePath, executable_sha256: executableSha256,
    argv, cwd, exit_status: raw.status, signal: raw.signal,
    spawn_error: raw.error ? String(raw.error) : null,
    stdout_sha256: digest(stdoutBytes), stderr_sha256: digest(stderrBytes),
    stdout_bytes: stdoutBytes.length, stderr_bytes: stderrBytes.length,
    stdout_path: relativeOutputPath(out, stdoutPath),
    stderr_path: relativeOutputPath(out, stderrPath),
    command_path: relativeOutputPath(out, join(captureDirectory, 'command.json')),
    environment_sha256: digest(Buffer.from(stable(Object.entries(env ?? {}).sort(([a], [b]) => a.localeCompare(b))))),
  };
  writeJson(join(captureDirectory, 'command.json'), record);
  commands.push(record);
  const result = {
    ...raw, stdoutBytes, stderrBytes,
    stdout: stdoutBytes.toString('utf8'), stderr: stderrBytes.toString('utf8'), record,
  };
  if (result.error || (result.status !== 0 && !allowFailure)) {
    throw Error(`${label} failed (${result.error?.message ?? result.status ?? result.signal}): ${result.stderr}`);
  }
  return result;
}

function gitState(git) {
  const head = run('source-head', git, ['rev-parse', 'HEAD']).stdout.trim();
  const tree = run('source-tree', git, ['rev-parse', 'HEAD^{tree}']).stdout.trim();
  const status = run('source-status', git,
    ['status', '--porcelain=v1', '--untracked-files=all']).stdout;
  return { head, tree, status };
}

function sourceHashes(git) {
  const result = run('source-path-inventory', git, [
    'ls-files', '-z', '--cached', '--others', '--exclude-standard', '--',
    'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', '.cargo', 'crates',
    'policy/tool-selection.json', 'verification/selected-vendor.mjs',
    'verification/host-locality',
  ]);
  const paths = result.stdoutBytes.toString('utf8').split('\0').filter(Boolean).sort();
  requireThat(paths.includes(fixtureRelative), 'source inventory omits the selected guest fixture');
  requireThat(paths.includes('verification/host-locality/execution.mjs'),
    'source inventory omits this execution runner');
  const hashes = {};
  for (const path of paths) {
    const absolute = join(root, path);
    requireThat(lstatSync(absolute).isFile(), `nonregular source input: ${path}`);
    hashes[path] = fileHash(absolute);
  }
  return hashes;
}

function outputDirectory(requested) {
  const resolved = resolve(requested);
  const parent = realpathSync(dirname(resolved));
  const destination = join(parent, basename(resolved));
  requireThat(destination !== parent, 'NEW_EXTERNAL_DIRECTORY must name a directory');
  try {
    lstatSync(destination);
    throw Error(`NEW_EXTERNAL_DIRECTORY already exists: ${destination}`);
  } catch (error) {
    if (error.code !== 'ENOENT') throw error;
  }
  const realRoot = realpathSync(root);
  requireThat(destination !== realRoot && !destination.startsWith(`${realRoot}${sep}`),
    'NEW_EXTERNAL_DIRECTORY must be outside the source worktree');
  mkdirSync(destination);
  for (const child of ['commands', 'home', 'tmp']) mkdirSync(join(destination, child));
  return destination;
}

function checkEnvironment(selection, selectedNode) {
  const forbidden = Object.keys(process.env).filter(key => {
    if (key === 'GIT_TERMINAL_PROMPT') return process.env[key] !== '0';
    return /^(?:LEAN(?:_|$)|LAKE(?:_|$)|NODE(?:_|$)|BASH(?:_|$)|ENV$|LD_|DYLD_|GIT_|NIX_|ELAN|CARGO_|RUST|NOBLE_)/
      .test(key) && process.env[key] !== '';
  });
  requireThat(forbidden.length === 0, `unsafe caller environment: ${forbidden.sort().join(', ')}`);
  requireThat(realpathSync(process.execPath) === realpathSync(selectedNode),
    `run this script with the selected Node executable ${selectedNode}`);
  requireThat(process.execArgv.length === 0, 'unreviewed Node runtime flags are not accepted');
  requireThat(realpathSync(selection.tool_paths.node.output + '/bin/node') === realpathSync(selectedNode),
    'selected Node differs from policy tool selection');
}

function parseJson(text, label) {
  try { return JSON.parse(text); } catch (error) {
    throw Error(`${label} did not emit JSON: ${error.message}`);
  }
}

function selectedFiles(rootPath) {
  const files = {};
  function walk(directory, prefix = '') {
    for (const entry of readdirSync(directory, { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name))) {
      const rel = prefix ? `${prefix}/${entry.name}` : entry.name;
      const path = join(directory, entry.name);
      if (entry.isDirectory()) {
        if (rel === 'cargo-target') continue;
        walk(path, rel);
      } else if (entry.isFile()) files[rel] = fileHash(path);
      else throw Error(`unexpected nonregular output artifact: ${rel}`);
    }
  }
  walk(rootPath);
  delete files['execution.json'];
  return files;
}

function compiledComponent({ out, cli, cliEnv, witBytes, guestBytes }) {
  const guestPath = join(out, 'guest', 'read-release.noble');
  const componentDirectory = join(out, 'component');
  mkdirSync(join(out, 'guest'));
  mkdirSync(componentDirectory);
  writeFileSync(guestPath, guestBytes, { flag: 'wx' });
  writeFileSync(join(out, 'selected-bounded-region.wit'), witBytes, { flag: 'wx' });

  const bindingsRun = run('component-bindings', cli,
    ['component', 'bindings', witRelative, worldName], { env: cliEnv });
  requireThat(bindingsRun.status === 0, `component bindings failed: ${bindingsRun.stderr}`);
  const bindings = parseJson(bindingsRun.stdout, 'component bindings');
  requireThat(bindings.world === worldId, `selected WIT world mismatch: ${bindings.world}`);
  requireThat(stable((bindings.imports ?? []).map(row => row.word))
    === stable(['region.read', 'regions.release']), 'selected imported operations changed');
  requireThat(stable((bindings.exports ?? []).map(row => row.word)) === stable(['read-region']),
    'selected WIT export changed');

  const componentDirectoryPath = join(componentDirectory, 'read-release');
  const compileRun = run('compile-selected-guest', cli, [
    'component', 'compile', witRelative, worldName, componentDirectoryPath,
    `read-region=${guestPath}`,
  ], { env: cliEnv, allowFailure: true });
  requireThat(compileRun.status === 0, `real CLI component compile failed: ${compileRun.stderr}`);
  const compileReport = parseJson(compileRun.stdout, 'component compile');
  requireThat(compileReport.outcome === 'compiled' && compileReport.component_emitted === true
    && compileReport.independent_kernel_check === true,
  'real CLI did not report an independently checked emitted component');
  const componentPath = join(componentDirectoryPath, 'component.wasm');
  const corePath = join(componentDirectoryPath, 'core.wasm');
  const watPath = join(componentDirectoryPath, 'module.wat');
  for (const path of [componentPath, corePath, watPath]) {
    requireThat(lstatSync(path).isFile(), `compiled artifact is absent: ${relativeOutputPath(out, path)}`);
  }
  return {
    bindings,
    bindings_command: bindingsRun.record.id,
    compile_command: compileRun.record.id,
    compile_report: compileReport,
    guest: {
      path: relativeOutputPath(out, guestPath), sha256: fileHash(guestPath), byte_length: guestBytes.length,
    },
    wit: {
      path: relativeOutputPath(out, join(out, 'selected-bounded-region.wit')),
      source_path: witRelative, sha256: digest(witBytes), byte_length: witBytes.length,
    },
    component: {
      path: relativeOutputPath(out, componentPath), sha256: fileHash(componentPath),
      byte_length: readFileSync(componentPath).length,
    },
    core: {
      path: relativeOutputPath(out, corePath), sha256: fileHash(corePath),
      byte_length: readFileSync(corePath).length,
    },
    wat: {
      path: relativeOutputPath(out, watPath), sha256: fileHash(watPath),
      byte_length: readFileSync(watPath).length,
    },
    directory: componentDirectoryPath,
  };
}

function faultWAT(watBytes, kind) {
  const text = watBytes.toString('utf8');
  const lines = text.split('\n');
  const importRows = lines.map((line, index) => ({ line, index }))
    .filter(row => /^\s*\(import\b/.test(row.line) && row.line
      .replace(/\\([0-9a-f]{2})/gi, (_, hex) => String.fromCharCode(Number.parseInt(hex, 16)))
      .includes('"[method]region.read"'));
  requireThat(importRows.length === 1, 'could not uniquely identify the emitted borrowed-read import');
  const importFunction = importRows[0].line.match(/\(func\s+(\$i[0-9]+)/)?.[1];
  requireThat(importFunction, 'emitted borrowed-read import has no core-Wasm function name');
  const calls = lines.map((line, index) => ({ line, index }))
    .filter(row => row.line.trim() === `call ${importFunction}`);
  requireThat(calls.length === 1, 'guest does not have exactly one selected region.read call');
  const callIndex = calls[0].index;
  let functionStart = callIndex;
  while (functionStart >= 0 && !/^\s*\(func\b/.test(lines[functionStart])) functionStart -= 1;
  requireThat(functionStart >= 0, 'cannot locate emitted export function containing region.read');
  let resultArea = null;
  for (let index = functionStart + 1; index < callIndex; index += 1) {
    const match = lines[index].trim().match(/^local\.get\s+(\$[^\s()]+|[0-9]+)$/);
    if (match) resultArea = match[1];
  }
  requireThat(resultArea, 'emitted multi-lane region result has no local result-area pointer');
  const indentation = lines[callIndex].match(/^\s*/)[0];
  let injected;
  if (kind === 'trap-after-read') injected = ['unreachable'];
  else if (kind === 'malformed-result-tag') {
    injected = [`local.get ${resultArea}`, 'i32.const 2', 'i32.store8'];
  } else if (kind === 'malformed-result-length') {
    injected = [`local.get ${resultArea}`, 'i32.const -1', 'i32.store offset=8'];
  } else throw Error(`unknown WAT fault: ${kind}`);
  lines.splice(callIndex + 1, 0, ...injected.map(line => `${indentation}${line}`));
  const result = Buffer.from(lines.join('\n'));
  return {
    bytes: result,
    fault: {
      kind, imported_function: importFunction, insertion_line: callIndex + 1,
      result_area_local: resultArea,
      instructions: injected,
      boundary: 'inserted immediately after the actual emitted region.read host import call, before normal lowering continues',
    },
  };
}

function assembleFaultComponent({ out, label, kind, sourceWAT, witBytes, wasmTools }) {
  const directory = join(out, 'faults', label);
  mkdirSync(directory, { recursive: true });
  const watPath = join(directory, 'module.wat');
  const witPath = join(directory, 'world.wit');
  writeFileSync(witPath, witBytes, { flag: 'wx' });
  const mutated = faultWAT(sourceWAT, kind);
  writeFileSync(watPath, mutated.bytes, { flag: 'wx' });
  const parse = run(`wasm-tools-parse-${label}`, wasmTools,
    ['parse', 'module.wat', '-o', 'core.wasm'], { cwd: directory, allowFailure: true });
  requireThat(parse.status === 0, `${label}: mutated emitted core WAT did not parse: ${parse.stderr}`);
  const embed = run(`wasm-tools-embed-${label}`, wasmTools, [
    'component', 'embed', '--world', worldName, '--encoding', 'utf8',
    'world.wit', 'core.wasm', '-o', 'embedded.wasm',
  ], { cwd: directory, allowFailure: true });
  requireThat(embed.status === 0, `${label}: component WIT embedding failed: ${embed.stderr}`);
  const component = run(`wasm-tools-component-${label}`, wasmTools,
    ['component', 'new', 'embedded.wasm', '-o', 'component.wasm'],
    { cwd: directory, allowFailure: true });
  requireThat(component.status === 0, `${label}: component assembly failed: ${component.stderr}`);
  const validate = run(`wasm-tools-validate-${label}`, wasmTools,
    ['validate', 'component.wasm'], { cwd: directory, allowFailure: true });
  requireThat(validate.status === 0, `${label}: assembled fault component is invalid: ${validate.stderr}`);
  const fileRecord = name => {
    const path = join(directory, name);
    return { path: relativeOutputPath(out, path), sha256: fileHash(path), byte_length: readFileSync(path).length };
  };
  return {
    fault: mutated.fault,
    artifacts: {
      wat: fileRecord('module.wat'), core: fileRecord('core.wasm'),
      embedded_core: fileRecord('embedded.wasm'), component: fileRecord('component.wasm'),
    },
    component_path: join(directory, 'component.wasm'),
    assembly_commands: [parse.record.id, embed.record.id, component.record.id, validate.record.id],
  };
}

function checkReport(report, label, expected) {
  requireThat(report.schema === 'noble-bounded-region/v1' && report.stage === 'adapter',
    `${label}: unexpected production adapter report`);
  for (const [key, value] of Object.entries(expected)) {
    requireThat(stable(report[key]) === stable(value),
      `${label}.${key}: expected ${stable(value)}, received ${stable(report[key])}`);
  }
  requireThat(report.native_memory_access === false, `${label}: adapter reported native-memory access`);
  requireThat(report.released_owners === 1 && report.live_owners === 0 && report.native_pins === 0,
    `${label}: expected one successful cleanup and no live owner/pin`);
}

function invokeCase({ out, cli, cliEnv, label, componentPath, componentSha256, bufferHex,
  offset, length, expected, kind }) {
  const result = run(`read-region-${label}`, cli, [
    'component', 'read-region', componentPath, bufferHex, String(offset), String(length),
  ], { env: cliEnv, allowFailure: true });
  requireThat(result.status === 0, `${label}: production read-region CLI failed: ${result.stderr}`);
  const report = parseJson(result.stdout, `read-region ${label}`);
  checkReport(report, label, expected);
  const row = {
    id: label, kind, component_sha256: componentSha256,
    input: { host_buffer_hex: bufferHex, offset: String(offset), length: String(length) },
    expected, observed: report, command_id: result.record.id,
  };
  cases.push(row);
  return row;
}

function checkSourceIntegrity(git, expectedState, expectedHashes) {
  const state = gitState(git);
  const hashes = sourceHashes(git);
  requireThat(stable(state) === stable(expectedState), 'source commit/tree/status changed during execution');
  requireThat(stable(hashes) === stable(expectedHashes), 'source files changed during execution');
  return { state, hashes };
}

function main() {
  const args = process.argv.slice(2);
  requireThat(args.length === 1 || (args.length === 2 && args[1] === '--allow-dirty'),
    'usage: SELECTED_NODE verification/host-locality/execution.mjs NEW_EXTERNAL_DIRECTORY [--allow-dirty]');
  const allowDirty = args[1] === '--allow-dirty';
  const selection = JSON.parse(readFileSync(join(root, 'policy/tool-selection.json')));
  const selectedNode = join(selection.tool_paths.node.output, 'bin/node');
  checkEnvironment(selection, selectedNode);
  out = outputDirectory(args[0]);
  const rustBin = join(selection.tool_paths.quality_rust.output, 'bin');
  const cargo = join(rustBin, 'cargo');
  const rustc = join(rustBin, 'rustc');
  const wasmTools = join(selection.tool_paths.wasm_tools.output, 'bin/wasm-tools');
  const linker = join(selection.component_sync.linker_bin, 'cc');
  const git = realpathSync('/run/current-system/sw/bin/git');
  const nix = realpathSync('/run/current-system/sw/bin/nix');
  baseEnv = {
    HOME: join(out, 'home'), USER: 'noble-host-locality', LOGNAME: 'noble-host-locality',
    LANG: 'C.UTF-8', LC_ALL: 'C.UTF-8', CI: '1', GIT_TERMINAL_PROMPT: '0',
    GIT_CONFIG_NOSYSTEM: '1', GIT_CONFIG_GLOBAL: '/dev/null',
    PATH: [rustBin, selection.component_sync.linker_bin,
      '/run/current-system/sw/bin', '/usr/bin', '/bin'].join(':'),
    TMPDIR: join(out, 'tmp'),
  };

  const initialState = gitState(git);
  const hashes = sourceHashes(git);
  const cleanSource = initialState.status === '';
  requireThat(cleanSource || allowDirty,
    'source worktree is dirty; re-run with --allow-dirty to bind the current source snapshot');
  const sourceRevisionSha256 = digest(Buffer.from(stable(Object.entries(hashes))));

  const vendor = selectedVendor(selection.component_sync);
  const nodeVersion = run('selected-node-version', selectedNode, ['--version']).stdout.trim();
  requireThat(nodeVersion === `v${process.versions.node}`, 'selected Node runtime version changed');
  const cargoVersion = run('selected-cargo-version', cargo, ['--version']).stdout.trim();
  const rustcVersion = run('selected-rustc-version', rustc, ['--version']).stdout.trim();
  const wasmToolsVersion = run('selected-wasm-tools-version', wasmTools, ['--version']).stdout.trim();
  requireThat(wasmToolsVersion === `wasm-tools ${selection.component_sync.wasm_tools_version}`,
    'selected wasm-tools version differs from policy');

  const cargoHome = join(out, 'cargo-home');
  const targetRoot = join(root, 'target');
  mkdirSync(targetRoot, { recursive: true });
  const target = realpathSync(targetRoot);
  mkdirSync(cargoHome);
  writeFileSync(join(cargoHome, 'config.toml'),
    '[source.crates-io]\nreplace-with = "reviewed-vendor"\n[source.reviewed-vendor]\n' +
    `directory = ${JSON.stringify(vendor.directory)}\n[net]\noffline = true\n`, { flag: 'wx' });
  const cliEnv = {
    ...baseEnv,
    CARGO_HOME: cargoHome, CARGO_TARGET_DIR: target, CARGO_NET_OFFLINE: 'true',
    CARGO_BUILD_JOBS: '4', CARGO_INCREMENTAL: '0',
    RUSTC: rustc, RUSTDOC: join(rustBin, 'rustdoc'),
    RUSTC_WRAPPER: '', RUSTC_WORKSPACE_WRAPPER: '', CARGO_BUILD_RUSTC_WRAPPER: '',
    RUSTFLAGS: '', CARGO_ENCODED_RUSTFLAGS: '',
    CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS: '',
    CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER: linker,
  };
  const build = run('source-cli-build', cargo,
    ['build', '--locked', '--offline', '-j', '4', '-p', 'noble-cli'],
    { cwd: root, env: cliEnv, allowFailure: true });
  requireThat(build.status === 0, `source-built noble-cli failed: ${build.stderr}`);
  const cli = join(target, 'debug/noble');
  requireThat(lstatSync(cli).isFile(), 'source-built CLI is absent from the selected target');
  const cliSha256 = fileHash(cli);
  const toolchain = {
    selected_node: { path: realpathSync(selectedNode), sha256: fileHash(selectedNode), version: nodeVersion },
    cargo: { path: realpathSync(cargo), sha256: fileHash(cargo), version: cargoVersion },
    rustc: { path: realpathSync(rustc), sha256: fileHash(rustc), version: rustcVersion,
      selection: selection.configuration.quality_rust },
    linker: { path: realpathSync(linker), sha256: fileHash(linker) },
    wasm_tools: { path: realpathSync(wasmTools), sha256: fileHash(wasmTools), version: wasmToolsVersion },
    nix: { path: nix, sha256: fileHash(nix) },
    vendor: { directory: vendor.directory, nar_hash: vendor.narHash },
    wasmtime: { version: selection.component_sync.wasmtime_version,
      source: selection.component_sync.wasmtime_source,
      source_nar_hash: selection.component_sync.wasmtime_source_nar_hash },
    cli: { path: cli, sha256: cliSha256 },
  };

  const witBytes = readFileSync(join(root, witRelative));
  const guestBytes = readFileSync(join(root, fixtureRelative));
  const compiled = compiledComponent({ out, cli, cliEnv, witBytes, guestBytes });
  const baseComponentPath = join(compiled.directory, 'component.wasm');
  const region = '000102';
  const normalCases = [
    { id: 'in-bounds-read-and-consuming-release', kind: 'normal-success', offset: 1, length: 2,
      expected: { outcome: 'read', bytes_hex: '0102', error: null, protected_operations: 1, region_reads: 1 } },
    { id: 'empty-read-at-end-and-consuming-release', kind: 'normal-success-boundary', offset: 3, length: 0,
      expected: { outcome: 'read', bytes_hex: '', error: null, protected_operations: 1, region_reads: 1 } },
    { id: 'partial-overrun-ordinary-error', kind: 'ordinary-bounds-error', offset: 2, length: 2,
      expected: { outcome: 'bounds-reject', bytes_hex: null, error: 'bounds-reject', protected_operations: 0, region_reads: 0 } },
    { id: 'zero-length-outside-ordinary-error', kind: 'ordinary-bounds-error', offset: 4, length: 0,
      expected: { outcome: 'bounds-reject', bytes_hex: null, error: 'bounds-reject', protected_operations: 0, region_reads: 0 } },
    { id: 'negative-offset-ordinary-error', kind: 'ordinary-bounds-error', offset: -1, length: 1,
      expected: { outcome: 'bounds-reject', bytes_hex: null, error: 'bounds-reject', protected_operations: 0, region_reads: 0 } },
    { id: 'negative-length-ordinary-error', kind: 'ordinary-bounds-error', offset: 0, length: -1,
      expected: { outcome: 'bounds-reject', bytes_hex: null, error: 'bounds-reject', protected_operations: 0, region_reads: 0 } },
    { id: 'checked-add-overflow-ordinary-error', kind: 'ordinary-bounds-error',
      offset: '9223372036854775807', length: 1,
      expected: { outcome: 'bounds-reject', bytes_hex: null, error: 'bounds-reject', protected_operations: 0, region_reads: 0 } },
  ];
  for (const scenario of normalCases) {
    invokeCase({ out, cli, cliEnv, label: scenario.id, componentPath: baseComponentPath,
      componentSha256: compiled.component.sha256, bufferHex: region,
      offset: scenario.offset, length: scenario.length, expected: scenario.expected, kind: scenario.kind });
  }

  const sourceWAT = readFileSync(join(compiled.directory, 'module.wat'));
  const faultSpecs = [
    { id: 'guest-trap-after-host-read', kind: 'trap-after-read', observed: {
      outcome: 'adapter-trap', bytes_hex: null, protected_operations: 1, region_reads: 1,
    } },
    { id: 'malformed-result-discriminant', kind: 'malformed-result-tag', observed: {
      outcome: 'adapter-trap', bytes_hex: null, protected_operations: 1, region_reads: 1,
    } },
    { id: 'malformed-result-list-length', kind: 'malformed-result-length', observed: {
      outcome: 'adapter-trap', bytes_hex: null, protected_operations: 1, region_reads: 1,
    } },
  ];
  const faultComponents = {};
  for (const spec of faultSpecs) {
    const fault = assembleFaultComponent({ out, label: spec.id, kind: spec.kind, sourceWAT, witBytes, wasmTools });
    const row = invokeCase({ out, cli, cliEnv, label: spec.id, componentPath: fault.component_path,
      componentSha256: fault.artifacts.component.sha256, bufferHex: region, offset: 1, length: 2,
      expected: spec.observed, kind: 'emitted-core-wasm-fault-control' });
    requireThat(typeof row.observed.error === 'string' && row.observed.error.length > 0,
      `${spec.id}: trap report did not retain a diagnostic`);
    faultComponents[spec.id] = { ...fault, execution_case: row.id };
  }

  const hostTests = run('direct-production-host-controls', cargo, [
    'test', '--locked', '--offline', '-j', '4', '-p', 'noble-cli', '--bin', 'noble',
    'component::bounded_region::tests', '--', '--test-threads=1',
  ], { cwd: root, env: cliEnv, allowFailure: true });
  requireThat(hostTests.status === 0, `direct production Host controls failed: ${hostTests.stderr}`);
  const hostTestNames = [
    'invalid_claims_and_boundaries_do_not_read_or_consume_region',
    'end_boundary_is_empty_success_and_zero_length_outside_refuses',
    'same_slot_and_generation_from_another_live_invocation_cannot_cross',
    'double_release_does_not_retire_the_owner_twice',
    'failed_cleanup_release_retains_owner_for_recovery',
  ];
  for (const name of hostTestNames) {
    requireThat(new RegExp(`test component::bounded_region::tests::${name}\\s+\\.\\.\\. ok`).test(hostTests.stdout),
      `direct Host test did not report success: ${name}`);
  }
  requireThat(/test result: ok\. 5 passed; 0 failed/.test(hostTests.stdout),
    'direct Host control count differs from the receipt roster');
  const hostTestCommand = hostTests.record.id;
  const hostTestControls = [
    ['wrong-table-id', 'Host::read', 'invalid_claims_and_boundaries_do_not_read_or_consume_region',
      'A copied Handle with only TableId changed is refused before any protected read.'],
    ['wrong-context', 'Host::read', 'invalid_claims_and_boundaries_do_not_read_or_consume_region',
      'A copied Handle with only Context changed is refused before any protected read.'],
    ['wrong-generation', 'Host::read', 'invalid_claims_and_boundaries_do_not_read_or_consume_region',
      'A copied Handle with only generation changed is refused before any protected read.'],
    ['wrong-kind', 'Host::read', 'invalid_claims_and_boundaries_do_not_read_or_consume_region',
      'A copied Handle with only kind changed is refused before any protected read.'],
    ['wrong-rights', 'Host::read', 'invalid_claims_and_boundaries_do_not_read_or_consume_region',
      'A copied Handle with only rights changed is refused before any protected read.'],
    ['wrong-resource-representation', 'Host::read', 'invalid_claims_and_boundaries_do_not_read_or_consume_region',
      'A noncanonical resource representation is refused before any protected read.'],
    ['foreign-invocation-owner-same-slot-generation', 'Host::read',
      'same_slot_and_generation_from_another_live_invocation_cannot_cross',
      'A live owner from another production Host invocation has the same slot/generation but a different table/context and cannot authorize bytes.'],
    ['borrowed-owner-restored-before-consuming-release', 'Host::read / Host::release',
      'invalid_claims_and_boundaries_do_not_read_or_consume_region',
      'After a successful borrowed read the original owner handle remains live and is then consumed by release.'],
    ['double-retirement-refused', 'Host::release', 'double_release_does_not_retire_the_owner_twice',
      'The first release retires once; a second release errors without incrementing releases or recreating a live owner.'],
    ['cleanup-release-refusal-retains-owner', 'Host::release', 'failed_cleanup_release_retains_owner_for_recovery',
      'Test-only corrupted Host.context (not guest-reachable) makes the production release method fail; the owner is returned live and a corrected retry succeeds. This is not an injected public CLI post-return failure.'],
  ];
  for (const [id, method, test, observation] of hostTestControls) {
    controls.push({ id, status: 'passed', class: 'actual-production-host-method-test', method, test,
      command_id: hostTestCommand, observation });
  }
  for (const spec of faultSpecs) {
    const abnormalObservation = spec.kind === 'trap-after-read'
      ? 'The exact emitted core Wasm traps on an inserted unreachable immediately after the real region.read import call. Wasmtime returns adapter-trap; no subsequent guest instruction/normal continuation runs, while the host report retains the read and shows owner/pin cleanup. released_owners cannot attribute guest release versus host cleanup.'
      : 'The exact emitted core Wasm corrupts the selected result-area discriminant/length after a real host response; Wasmtime reports adapter-trap before normal guest continuation, while the report retains the read and shows owner/pin cleanup. The malformed bytes were not returned by the host Val.';
    controls.push({ id: spec.id, status: 'passed', class: 'actual-emitted-core-wasm-fault',
      case_id: spec.id, fault: faultComponents[spec.id].fault,
      guest_continuation: 'not-resumed-after-trap', observation: abnormalObservation });
  }
  controls.push({
    id: 'unrelated-logical-owner-frame-mutation', status: 'pure-model-only',
    class: 'model-control-not-production-host-api',
    observation: 'The selected production adapter deliberately has one owner per Host invocation and no multi-owner frame API. A distinct concurrently live Host invocation is tested directly, but unrelated-owner/frame mutation is not represented by this production API and is not claimed as execution evidence.',
  });
  controls.push({
    id: 'early-borrow-restoration-before-result-validation', status: 'model-only',
    class: 'bad-boundary-transition-for-proof-binding',
    transition: 'Restore the borrowed owner into the guest frame before validating the imported result tag and payload.',
    expected: 'Reject this transition; result validation must precede continuation-state restoration.',
    execution_evidence: false,
    observation: 'The Lean boundary model/audit owns this negative transition. In calls.rs, restore_borrows only pushes saved Values onto compiler State.stack and emits no runtime instruction; a source-order move may therefore produce identical Wasm. The actual malformed-result mutants exercise runtime validation/trap cleanup but cannot detect this compile-time ordering mutation. No production telemetry/API was added.',
  });
  controls.push({
    id: 'public-adapter-cleanup-failure-report-path', status: 'not-exercised',
    class: 'cleanup-path-instrumentation-limit',
    observation: 'A direct Host::release failure and successful retry are exercised. The public read-region post-return cleanup-failure process path cannot be induced from the selected guest/host interface without a production fault-injection hook; no claim is made about that CLI path. Trap cleanup success is observed separately.',
  });

  const finalStateAndHashes = checkSourceIntegrity(git, initialState, hashes);
  requireThat(fileHash(cli) === cliSha256, 'the source-built CLI bytes changed during the run');
  const hostSourcePaths = Object.keys(hashes).filter(path => path === 'crates/noble-cli/src/component/bounded_region.rs'
    || path === 'crates/noble-cli/src/component/identity.rs'
    || path.startsWith('crates/noble-kernel/src/resources/'));
  const loweringSourcePaths = Object.keys(hashes).filter(path => path.startsWith('crates/noble-wasm/src/component/')
    || path === witRelative);
  const runtimeSourcePaths = Object.keys(hashes).filter(path => path.startsWith('crates/noble-cli/src/component/')
    || path === 'crates/noble-cli/src/component.rs');
  const artifactHashes = selectedFiles(out);
  const sourceHashesForHost = Object.fromEntries(hostSourcePaths.map(path => [path, hashes[path]]));
  const receipt = {
    schema: 'noble-host-locality-execution/v1', issue: 51,
    source_commit: initialState.head, source_tree: initialState.tree,
    clean_source: cleanSource, allow_dirty: allowDirty,
    source_status_before: initialState.status,
    source_worktree_revision_sha256: sourceRevisionSha256,
    source_hashes_revision_recheck: digest(Buffer.from(stable(Object.entries(finalStateAndHashes.hashes)))),
    sources_sha256: hashes,
    selected_wit_abi: {
      package: packageId, world: compiled.bindings.world,
      canonical_abi: selection.component_sync.canonical_abi,
      revision: 'bounded-region WIT 1.0.0 / memory32 synchronous Canonical ABI',
      wit_path: witRelative, wit_sha256: digest(witBytes),
      imports: compiled.bindings.imports.map(row => row.word),
      exports: compiled.bindings.exports.map(row => row.word),
      wasmtime_version: selection.component_sync.wasmtime_version,
      wasm_tools_version: selection.component_sync.wasm_tools_version,
      bindings_command_id: compiled.bindings_command,
    },
    toolchain,
    build: {
      fresh_external_target: false, target_path: target,
      cargo_home_path: relativeOutputPath(out, cargoHome),
      cargo_config_sha256: fileHash(join(cargoHome, 'config.toml')),
      command_id: build.record.id, cli_sha256: cliSha256,
      offline_vendor_nar_hash: vendor.narHash,
    },
    host_runtime_source_hashes: {
      host: sourceHashesForHost,
      lowering: Object.fromEntries(loweringSourcePaths.map(path => [path, hashes[path]])),
      runtime: Object.fromEntries(runtimeSourcePaths.map(path => [path, hashes[path]])),
      workspace_and_lock: Object.fromEntries(Object.entries(hashes).filter(([path]) =>
        path === 'Cargo.toml' || path === 'Cargo.lock' || path === 'crates/noble-cli/Cargo.toml'
        || path === 'crates/noble-wasm/Cargo.toml' || path === 'crates/noble-kernel/Cargo.toml')),
    },
    guest: compiled.guest,
    component: compiled.component,
    core: compiled.core,
    emitted_wat: compiled.wat,
    fault_components: faultComponents,
    cases,
    controls,
    commands,
    artifacts_sha256: artifactHashes,
    artifact_count: Object.keys(artifactHashes).length,
    failures: [],
    assumptions: [
      'Every normal read and ordinary bounds result is a fresh public CLI invocation over the actual source-built noble-cli, selected WIT guest and Wasmtime host adapter; the host selects immutable bytes and constructs one owner per invocation.',
      'The Host::read and Host::release unit controls call the private production methods directly. They do not model Wasmtime resource authenticity or a multi-owner production API.',
      'Trap and malformed-result controls start from the exact module.wat emitted for the selected guest, alter only its emitted core-Wasm instructions after the actual region.read import call, reassemble it with selected wasm-tools, and execute it through the same public CLI/Wasmtime adapter.',
      'Malformed result tag/length controls corrupt guest result-area memory after Wasmtime has supplied a valid typed host response. They test emitted lowering validation and cleanup on those finite mutated artifacts, not an adversarial host returning a malformed Val.',
      'The current noble-bounded-region/v1 report conflates guest release, destructor release and host post-return cleanup in released_owners. Normal guest source includes consuming release, but the report cannot attribute the one count to that source operation rather than cleanup.',
      'The adapter report records no ordered event trace. The actual trap fault is placed immediately after the selected host read call; no guest continuation or guest restoration is inferred after the trap.',
      'Direct Host::release refusal/recovery is observed separately from the public CLI post-return cleanup-failure path, which remains unexercised without a production fault-injection seam.',
      'Finite execution, Rust unit controls and artifacts do not prove general Canonical ABI, Noble lowering, host/runtime, Wasmtime, or Lean-model refinement. Borrow-restoration/result-validation event order and unrelated logical frame mutation remain unobserved or model-only as identified in controls.',
    ],
    open_correspondence: [
      'Rust Host/table decisions to the extracted M5 table model beyond the finite direct Host tests.',
      'Wasmtime Canonical ABI resource authenticity and borrow lifetime to the typed Noble owner model.',
      'Malformed host Val production: no public host mutation seam exists; current fault mutants instead corrupt returned core memory after a valid response.',
      'Compile-time borrowed-owner restoration ordering has no separate runtime event in this ABI; a causal execution distinction would require an existing lowering harness or approved instrumentation.',
      'Guest-versus-host release attribution, because noble-bounded-region/v1 exposes only a single released_owners counter.',
      'Public adapter post-return cleanup failure, process exit/report behavior, and cleanup after process crash.',
      'The selected profile has one owner per Host invocation; unrelated-owner/frame mutation does not correspond to a production multi-owner API.',
    ],
  };
  const receiptPath = join(out, 'execution.json');
  writeFileSync(receiptPath, json(receipt), { flag: 'wx' });
  console.log(JSON.stringify({
    result: 'passed', receipt: receiptPath, receipt_sha256: fileHash(receiptPath),
    source_commit: receipt.source_commit, source_tree: receipt.source_tree,
    clean_source: cleanSource, cases: cases.length, controls_passed: controls.filter(row => row.status === 'passed').length,
    controls_limited: controls.filter(row => row.status !== 'passed').length,
    commands: commands.length, artifacts: receipt.artifact_count, failures: [],
  }));
}

function lstatExists(path) {
  try { lstatSync(path); return true; } catch (error) {
    if (error.code === 'ENOENT') return false;
    throw error;
  }
}

try {
  main();
} catch (error) {
  const summary = {
    result: 'failed', receipt: out ? join(out, 'execution.json') : null,
    receipt_written: Boolean(out && lstatExists(join(out, 'execution.json'))),
    cases: cases.length, controls_passed: controls.filter(row => row.status === 'passed').length,
    controls_limited: controls.filter(row => row.status !== 'passed').length,
    commands: commands.length, failures: [error.message],
  };
  console.error(`[host-locality] FAIL ${error.message}`);
  console.log(JSON.stringify(summary));
  process.exitCode = 1;
}
