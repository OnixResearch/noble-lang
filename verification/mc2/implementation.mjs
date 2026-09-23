#!/usr/bin/env node
// Rebuild the source-bound CLI, run all declared MC2 controls, and require a
// fresh whole-crate Charon/Aeneas/Lean check against the reviewed extraction lock.
// Receipts are outputs, never inputs to their own source fingerprint.
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const [argument, ...extra] = process.argv.slice(2);
if (!argument || argument === '--help' || extra.length) {
  console.log('usage: SELECTED_NODE verification/mc2/implementation.mjs NEW_ARTIFACT_DIRECTORY');
  process.exit(argument === '--help' ? 0 : 2);
}
const artifacts = path.resolve(argument);
if (artifacts === root || artifacts.startsWith(`${root}${path.sep}`)) throw Error('artifacts must be outside the repository');
fs.mkdirSync(path.dirname(artifacts), { recursive: true });
fs.mkdirSync(artifacts);
const json = file => JSON.parse(fs.readFileSync(file, 'utf8'));
const selection = json(path.join(root, 'policy/tool-selection.json'));
const pinned = name => selection.tool_paths[name].output;
const binary = (name, tool) => path.join(pinned(name), 'bin', tool);
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const fileSha = file => sha(fs.readFileSync(file));
const save = (name, value) => fs.writeFileSync(path.join(artifacts, name), JSON.stringify(value, null, 2) + '\n');
if (fs.realpathSync(process.execPath) !== fs.realpathSync(binary('node', 'node'))) {
  throw Error('invoke this gate with the selected Node executable');
}
function files(directory) {
  return fs.readdirSync(path.join(root, directory), { withFileTypes: true }).flatMap(entry => {
    if (['.git', '.lake', '.lake-packages', 'target', 'node_modules'].includes(entry.name)) return [];
    const item = path.posix.join(directory, entry.name);
    if (entry.isSymbolicLink()) throw Error(`unbound source symlink: ${item}`);
    if (entry.isDirectory()) return files(item);
    if (!entry.isFile()) throw Error(`unbound non-file input: ${item}`);
    return [item];
  });
}
function snapshot() {
  const inputs = new Set([
    'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', 'dylint.toml',
    ...Object.keys(selection.file_sha256), 'policy/tool-selection.json',
    ...files('policy').filter(file => /\.(json|ncl)$/.test(file)),
    ...files('nix').filter(file => /\.(nix|patch)$/.test(file)),
    ...files('crates'),
    ...files('proofs').filter(file => /\.(lean|toml|json)$/.test(file) || file.endsWith('/lean-toolchain')),
    ...files('verification/mc2').filter(file => /\.(mjs|lean|contract|noble)$/.test(file)),
    ...files('verification/mc1').filter(file => /\.(mjs|noble-contract|lean)$/.test(file)),
    ...files('verification/m4').filter(file => file.endsWith('.mjs')),
    'verification/m4/extraction-lock.json', 'verification/m4/inherited-boundaries.json',
  ]);
  return Object.fromEntries([...inputs].sort().map(file => [file, fileSha(path.join(root, file))]));
}
// Status/evidence annotations in the canonical file are documentation outputs;
// every case's id, input and expected result remain immutable gate inputs.
const workload = () => json(path.join(root, 'specs/conformance/contract-cases.json')).cases
  .map(({ id, input, expected }) => ({ id, input, expected }));
const sourceFiles = snapshot();
const declaredWorkload = workload();
const commands = [];
const environment = {
  ...process.env,
  PATH: `${pinned('quality_rust')}/bin:${pinned('lean')}/bin:${process.env.PATH}`,
  RUSTC: binary('quality_rust', 'rustc'),
  RUSTDOC: binary('quality_rust', 'rustdoc'),
  RUSTC_WRAPPER: '',
  RUSTC_WORKSPACE_WRAPPER: '',
  CARGO_TARGET_DIR: path.join(artifacts, 'cargo-target'),
  NOBLE_LEAN: binary('lean', 'lean'),
  NOBLE_CONTRACT_LIBRARY: path.join(root, 'proofs/mc1'),
};
async function run(label, executable, args, cwd = root, overrides = {}) {
  const log = `${label}.log`;
  const fd = fs.openSync(path.join(artifacts, log), 'wx');
  const command = { label, binary: executable, args, cwd: path.relative(root, cwd) || '.',
    environment: overrides, log, started_at: new Date().toISOString() };
  commands.push(command);
  save('commands.json', commands);
  try {
    await new Promise((resolve, reject) => {
      const child = spawn(executable, args, { cwd, env: { ...environment, ...overrides }, stdio: ['ignore', fd, fd] });
      child.once('error', reject);
      child.once('close', (exit, signal) => {
        Object.assign(command, { exit, signal });
        if (exit === 0 && signal === null) resolve();
        else reject(Error(`${label} failed: ${exit ?? signal}; see ${log}`));
      });
    });
  } finally {
    fs.closeSync(fd);
    command.finished_at = new Date().toISOString();
    command.output_sha256 = fileSha(path.join(artifacts, log));
    save('commands.json', commands);
  }
  return fs.readFileSync(path.join(artifacts, log), 'utf8');
}
try {
  const versions = {};
  for (const [name, executable, args] of [
    ['quality_rust', binary('quality_rust', 'rustc'), ['--version', '--verbose']],
    ['lean', binary('lean', 'lean'), ['--version']],
    ['lake', binary('lean', 'lake'), ['--version']],
    ['node', process.execPath, ['--version']],
  ]) versions[name] = (await run(`${name}-version`, executable, args)).trim();

  await run('cli-build', binary('quality_rust', 'cargo'), ['build', '-p', 'noble-cli', '--locked', '--offline']);
  const cli = path.join(environment.CARGO_TARGET_DIR, 'debug', 'noble');
  environment.NOBLE_MC2_CHECKER_BINARY = cli;
  await run('consumer-library-build', binary('lean', 'lake'), ['build', 'NobleContracts', 'NobleContractImpl'], path.join(root, 'proofs/mc1'));
  await run('proof-build', binary('lean', 'lake'), ['build'], path.join(root, 'proofs/mc2'));
  const proof = await run('proof-gate', binary('lean', 'lake'), ['env', 'lean', 'MC2Gate.lean'], path.join(root, 'proofs/mc2'));
  const verdictLine = proof.split('\n').find(line => line.startsWith('MC2-GATE '));
  if (!verdictLine) throw Error('missing MC2-GATE verdict');
  const verdict = JSON.parse(verdictLine.slice('MC2-GATE '.length));
  if (verdict.result !== 'passed' || verdict.required_theorems !== 35 || verdict.required_definitions !== 6
    || verdict.guard_templates !== 3 || verdict.extracted_correspondence !== 'separate-required-MC2ExtractionGate') {
    throw Error(`unexpected semantic certificate verdict: ${verdictLine}`);
  }
  await run('workspace-tests', binary('quality_rust', 'cargo'), ['test', '--workspace', '--locked', '--offline']);

  const acceptancePath = path.join(artifacts, 'acceptance.json');
  const extractionDirectory = path.join(artifacts, 'extraction-check');
  // Independent read-only runs over a settled source tree. Extraction compiles
  // only its private snapshot; acceptance uses the freshly rebuilt CLI.
  const results = await Promise.allSettled([
    run('acceptance', process.execPath, [path.join(root, 'verification/mc2/gate.mjs'), cli, acceptancePath]),
    // The extraction gate selects its own compiler and private target. Do not
    // pass this caller's quality-build settings as forbidden tool overrides.
    run('extraction-check', process.execPath,
      [path.join(root, 'verification/m4/implementation.mjs'), 'check', extractionDirectory],
      root, { RUSTC: '', RUSTDOC: '', CARGO_TARGET_DIR: '' }),
  ]);
  for (const result of results) if (result.status === 'rejected') throw result.reason;
  const acceptance = json(acceptancePath);
  const expectedIds = declaredWorkload.map(item => item.id).sort();
  const actualIds = acceptance.cases.map(item => item.case).sort();
  if (expectedIds.length !== 15 || JSON.stringify(actualIds) !== JSON.stringify(expectedIds)
    || acceptance.cases.some(item => item.passed !== true)) throw Error('canonical acceptance case set did not pass');
  if (JSON.stringify(acceptance.conformance.workload) !== JSON.stringify(declaredWorkload)) throw Error('acceptance workload changed');

  const extractionPath = path.join(extractionDirectory, 'implementation.json');
  const extraction = json(extractionPath);
  if (extraction.mode !== 'check' || extraction.result !== 'passed'
    || extraction.mc2?.coverage?.schema !== 'mc2-compiled-coverage/v2'
    || extraction.mc2?.coverage?.host_observation?.classification !== 'authentic-sound-host-check-observation'
    || extraction.mc2?.correspondence?.result !== 'passed') throw Error('fresh MC2 extraction/accounting did not pass');
  for (const [file, hash] of Object.entries(extraction.source_files ?? {})) {
    if (fileSha(path.join(root, file)) !== hash) throw Error(`extraction source is stale: ${file}`);
  }
  if (!Object.keys(extraction.source_files ?? {}).length) throw Error('extraction source binding absent');
  if (JSON.stringify(snapshot()) !== JSON.stringify(sourceFiles)
    || JSON.stringify(workload()) !== JSON.stringify(declaredWorkload)) throw Error('source/workload changed during the run');

  const evidence = {
    schema: 'mc2-implementation-evidence/v1', scope: 'first-class-certified-programs', mode: 'check', result: 'passed',
    source_revision: `sha256:${sha(JSON.stringify(sourceFiles))}`, source_files: sourceFiles,
    conformance_workload_sha256: sha(JSON.stringify(declaredWorkload)),
    binary: { path: cli, sha256: fileSha(cli) }, versions, commands, proof_gate: verdict,
    acceptance: { path: acceptancePath, sha256: fileSha(acceptancePath), schema: acceptance.schema,
      cases: acceptance.cases.map(({ case: id, passed }) => ({ case: id, passed })) },
    extraction: { status: 'passed', path: extractionPath, sha256: fileSha(extractionPath),
      source_revision: extraction.source_revision, mc2: extraction.mc2 },
    assumptions: [
      'Pinned compiler, translator, Lean kernel, Wasm assembler/optimizer/engine, sandbox and host OS remain explicit trusted tooling boundaries.',
      'CheckObservation records are an explicit authentic-and-sound host-check boundary: the source-bound CLI runs the isolated independent Lean consumer over the exact statement and submitted bytes. Arbitrary host Rust can construct observations; constructor availability does not prove their authenticity or soundness.',
      'Strict semantic certificate theorems and extracted finite-rule/template/release correspondences are separate audited scopes, not a universal CLI/parser/backend refinement theorem.',
      'Actual execution and hostile admission/replay controls establish only the exact canonical workload retained in acceptance.json.',
      'Admitted declarations and replay derivations carry partial correctness, never total correctness, host authority or execution fuel guarantees.',
    ],
  };
  save('implementation.json', evidence);
  console.log(`MC2-IMPLEMENTATION-GATE passed: ${evidence.source_revision}`);
} catch (error) {
  save('failure.json', { result: 'failed', message: String(error), commands });
  console.error(String(error));
  process.exitCode = 1;
}
