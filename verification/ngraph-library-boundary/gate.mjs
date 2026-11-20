#!/usr/bin/env node
// NGRAPH-CASE-02: compiler-derived capability boundary for the noble-ngraph library.
//
// The static crate-boundary scan in verification/ngraph-purity is supporting
// evidence only: `#![no_std]` is a crate/build constraint, not a transitive
// purity guarantee. This lane evaluates the selected pinned Octet run's own
// compiler-derived artifacts instead:
//
//   cargo-octet check --workspace --output-format json --artifact-dir OUT \
//     -- --all-targets --features live-test-barrier,safety-observe --offline
//
// It refuses a dirty source tree, runs that exact row on the committed tree,
// replays the artifact with `cargo-octet artifact verify`, and evaluates the
// project architecture receipt, the Cargo graph, the Compiler Architecture IR
// and the compiler coverage against the case's expected fields. It also runs
// one negative control: a scratch copy of the same commit whose noble-ngraph
// library gains a forbidden environment capability through a `std` alias. The
// control must be refused by the same predicates that the positive tree
// passes, so a vacuous evaluator cannot report the positive result.
//
// The runner writes its receipt only when asked (--receipt FILE) and never
// overwrites an existing file. Raw Octet artifacts, logs and the control tree
// stay in the external output directory; the receipt records their sha256.
//
// This lane establishes bounded compiler-derived facts for the selected
// Octet run. It is not a proof, not whole-program authenticity, and not a
// runtime observation of the library.
import crypto from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const usage = 'usage: node gate.mjs REPO_ROOT NEW_OUTPUT_DIRECTORY OCTET_OUTPUT_DIRECTORY [--receipt FILE] [--jobs N]';
const caseId = 'NGRAPH-CASE-02';
const packageName = 'noble-ngraph';
const forbiddenFamilies = ['filesystem', 'process', 'environment', 'time', 'concurrency', 'observability', 'unsafe-code'];
const identityTraits = [
  'noble_ngraph::identity::IdentityHasher',
  'noble_ngraph::identity::IdentityScheme',
];
const forbiddenHasherNeedles = ['DefaultHasher', 'RandomState', 'SipHasher', 'std::collections::hash_map', 'std::hash::'];
const octetArgs = [
  'check', '--workspace', '--output-format', 'json',
  '--', '--all-targets', '--features', 'live-test-barrier,safety-observe', '--offline',
];
const controlMutation = [
  '',
  '// NGRAPH-CASE-02 negative control: not part of the verified tree.',
  'extern crate std as boundary_control_std;',
  '',
  '#[doc(hidden)]',
  'pub fn boundary_control_probe() -> Option<boundary_control_std::ffi::OsString> {',
  '    boundary_control_std::env::var_os("NOBLE_NGRAPH_BOUNDARY_CONTROL")',
  '}',
  '',
].join('\n');

const sha256 = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const fileSha256 = file => sha256(fs.readFileSync(file));
const exists = file => {
  try { fs.statSync(file); return true; } catch { return false; }
};
const sorted = values => [...values].sort();

function fail(message) {
  process.stderr.write(`${message}\n${usage}\n`);
  process.exit(2);
}

function parseArguments(argv) {
  const positional = [];
  const options = { receipt: null, jobs: 6 };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === '--receipt') {
      options.receipt = argv[index + 1];
      if (!options.receipt) fail('--receipt needs a file');
      index += 1;
    } else if (argument === '--jobs') {
      options.jobs = Number(argv[index + 1]);
      if (!Number.isInteger(options.jobs) || options.jobs < 1 || options.jobs > 16) fail('--jobs must be 1..16');
      index += 1;
    } else if (argument.startsWith('--')) {
      fail(`unknown option: ${argument}`);
    } else {
      positional.push(argument);
    }
  }
  if (positional.length !== 3) fail('expected REPO_ROOT, NEW_OUTPUT_DIRECTORY and OCTET_OUTPUT_DIRECTORY');
  const [root, output, octetOutput] = positional.map(value => path.resolve(value));
  if (!fs.existsSync(root)) fail(`repository root missing: ${root}`);
  if (exists(output)) fail(`output directory already exists: ${output}`);
  const outputIsInsideRoot = path.relative(root, output) === '' || !path.relative(root, output).startsWith('..');
  if (outputIsInsideRoot) fail(`output directory must stay outside the repository: ${output}`);
  return { root, output, octetOutput, ...options };
}

function spawn(argv, options) {
  const started = process.hrtime.bigint();
  const result = spawnSync(argv[0], argv.slice(1), {
    encoding: 'buffer',
    maxBuffer: 1024 * 1024 * 1024,
    timeout: options.timeout ?? 3_600_000,
    cwd: options.cwd,
    env: options.env,
  });
  const durationMs = Number((process.hrtime.bigint() - started) / 1_000_000n);
  return {
    status: result.status,
    signal: result.signal,
    stdout: result.stdout ?? Buffer.alloc(0),
    stderr: result.stderr ?? Buffer.alloc(0),
    duration_ms: durationMs,
    error: result.error?.message ?? null,
  };
}

function git(root, args) {
  const result = spawn([process.env.NOBLE_GIT ?? 'git', '-C', root, ...args], { env: process.env });
  if (result.error || result.status !== 0) {
    throw new Error(`git ${args.join(' ')} failed: ${result.error ?? result.stderr.toString().trim()}`);
  }
  return result.stdout.toString().trim();
}

function cleanSourceIdentity(root) {
  const status = git(root, ['status', '--porcelain']);
  if (status !== '') throw new Error(`source tree is not clean:\n${status}`);
  const listing = git(root, ['ls-tree', '-r', 'HEAD']).split('\n').filter(Boolean);
  const manifest = listing.map(line => {
    const tab = line.indexOf('\t');
    return `${line.slice(tab + 1)}\0${line.slice(0, tab).split(' ')[2]}`;
  });
  return {
    commit: git(root, ['rev-parse', 'HEAD']),
    tree: git(root, ['rev-parse', 'HEAD^{tree}']),
    branch: git(root, ['branch', '--show-current']) || null,
    status: 'clean',
    tracked_files: listing.length,
    git_manifest_sha256: sha256(manifest.join('\n')),
  };
}

function resolveSelectedTool(root, selection, key) {
  const entry = selection.tool_paths?.[key];
  if (!entry?.output) throw new Error(`policy/tool-selection.json has no tool_paths.${key}.output`);
  const selected = path.resolve(entry.output);
  if (!fs.existsSync(selected)) throw new Error(`selected ${key} output is missing: ${selected}`);
  return selected;
}

function policyPath(root) {
  return path.join(root, 'policy/tool-selection.json');
}

function setupEnvironment(output, octetBinary, rustOutput, ccDirectory, vendor, jobs) {
  const cargoHome = path.join(output, 'cargo-home');
  fs.mkdirSync(cargoHome, { recursive: true });
  const target = path.join(output, 'target');
  const tmp = path.join(output, 'tmp');
  fs.mkdirSync(tmp, { recursive: true });
  fs.writeFileSync(path.join(cargoHome, 'config.toml'), [
    '[source.crates-io]',
    'replace-with = "reviewed-vendor"',
    '[source.reviewed-vendor]',
    `directory = ${JSON.stringify(path.join(vendor, 'source-registry-0'))}`,
    '[net]',
    'offline = true',
    '',
  ].join('\n'));
  const environment = {
    HOME: os.homedir(),
    USER: os.userInfo().username,
    LOGNAME: os.userInfo().username,
    LANG: 'C.UTF-8',
    PATH: sorted(new Set([
      path.dirname(octetBinary),
      path.join(rustOutput, 'bin'),
      ccDirectory,
      path.dirname(process.execPath),
      '/run/current-system/sw/bin',
    ])).join(':'),
    CARGO_HOME: cargoHome,
    CARGO_TARGET_DIR: target,
    CARGO_NET_OFFLINE: 'true',
    CARGO_BUILD_JOBS: String(jobs),
    CARGO_INCREMENTAL: '0',
    TMPDIR: tmp,
    RUSTC_WRAPPER: '',
    RUSTC_WORKSPACE_WRAPPER: '',
    CARGO_BUILD_RUSTC_WRAPPER: '',
    RUSTFLAGS: '',
    CARGO_ENCODED_RUSTFLAGS: '',
    CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS: '',
    CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER: path.join(ccDirectory, 'cc'),
  };
  return { environment, cargoHome, target, tmp };
}

function runOctet({ source, artifactDirectory, output, octetBinary, environment }) {
  fs.mkdirSync(output, { recursive: true });
  fs.mkdirSync(artifactDirectory, { recursive: true });
  const argv = [octetBinary, ...octetArgs, ...['--artifact-dir', artifactDirectory]];
  const result = spawn(argv, { cwd: source, env: environment });
  fs.writeFileSync(path.join(output, 'octet.stdout.json'), result.stdout);
  fs.writeFileSync(path.join(output, 'octet.stderr.log'), result.stderr);
  let parsed = null;
  try { parsed = JSON.parse(result.stdout.toString()); } catch { parsed = null; }
  const verification = spawn([octetBinary, 'artifact', 'verify', '--artifact-dir', artifactDirectory,
    '--output-format', 'json'], { cwd: source, env: environment });
  fs.writeFileSync(path.join(output, 'artifact-verify.stdout.json'), verification.stdout);
  fs.writeFileSync(path.join(output, 'artifact-verify.stderr.log'), verification.stderr);
  let verificationReport = null;
  try { verificationReport = JSON.parse(verification.stdout.toString()); } catch { verificationReport = null; }
  return {
    argv,
    result,
    parsed,
    verification,
    verificationReport,
  };
}

function readArtifact(directory) {
  const files = fs.readdirSync(directory, { withFileTypes: true })
    .filter(entry => entry.isFile())
    .map(entry => entry.name)
    .sort();
  const inventory = files.map(name => {
    const file = path.join(directory, name);
    return { name, bytes: fs.statSync(file).size, sha256: fileSha256(file) };
  });
  const parsed = new Map();
  const json = name => {
    if (parsed.has(name)) return parsed.get(name);
    if (!files.includes(name)) throw new Error(`artifact file missing: ${name}`);
    const value = JSON.parse(fs.readFileSync(path.join(directory, name), 'utf8'));
    parsed.set(name, value);
    return value;
  };
  return {
    files: inventory,
    total_bytes: inventory.reduce((sum, file) => sum + file.bytes, 0),
    json,
  };
}

function evaluate({ artifactDirectory, run, artifact, source, policy }) {
  const checks = [];
  const check = (name, claim, ok, observed) => checks.push({ name, claim, ok: Boolean(ok), observed });
  const isObject = value => value !== null && typeof value === 'object';

  check('octet-process',
    'The selected Octet row completed the Cargo process successfully for the committed tree.',
    run.result.error === null && run.result.signal === null
      && isObject(run.parsed?.phases)
      && run.parsed.phases.lint?.status !== 'integration-failure',
    { exit: run.result.status, signal: run.result.signal, error: run.result.error,
      lint_status: run.parsed?.phases?.lint?.status ?? null,
      lint_findings: run.parsed?.phases?.lint?.finding_count ?? null });

  const replay = run.verificationReport;
  check('artifact-replay',
    '`cargo-octet artifact verify` replays the stored artifact with no diagnostics.',
    run.verification.status === 0 && replay?.status === 'valid'
      && Array.isArray(replay.diagnostics) && replay.diagnostics.length === 0
      && Array.isArray(replay.artifacts) && replay.artifacts.length > 0,
    { exit: run.verification.status, status: replay?.status ?? null,
      artifacts: Array.isArray(replay?.artifacts) ? replay.artifacts.length : null,
      diagnostics: Array.isArray(replay?.diagnostics) ? replay.diagnostics.length : null });

  let receipt = null;
  let graph = null;
  let coverage = null;
  let compilerReceipt = null;
  let ir = null;
  try {
    receipt = artifact.json('project-architecture-receipt.json');
    graph = artifact.json('project-architecture-cargo-graph.json');
    coverage = artifact.json('compiler-architecture-coverage.json');
    compilerReceipt = artifact.json('compiler-architecture-receipt.json');
    ir = artifact.json('compiler-architecture-ir.json');
  } catch (error) {
    check('artifact-schema', 'Every required Octet artifact file is present and parses as JSON.', false,
      { error: error.message, files: artifact.files.map(file => file.name) });
    return { checks, receipt: null, graph: null, coverage: null, compilerReceipt: null, ir: null };
  }
  check('artifact-schema', 'Every required Octet artifact file is present and parses as JSON.', true,
    { files: artifact.files.map(file => file.name) });

  check('collection-complete',
    'Compiler collection covered the exact expected unit roster with no missing, unexpected, duplicate or invalid shards.',
    coverage.status === 'complete'
      && Array.isArray(coverage.missing_unit_ids) && coverage.missing_unit_ids.length === 0
      && Array.isArray(coverage.unexpected_unit_ids) && coverage.unexpected_unit_ids.length === 0
      && Array.isArray(coverage.duplicate_unit_ids) && coverage.duplicate_unit_ids.length === 0
      && Array.isArray(coverage.invalid_shard_ids) && coverage.invalid_shard_ids.length === 0
      && compilerReceipt.status === 'complete'
      && ir.schema_version === 'octet-compiler-architecture-ir/v2',
    { coverage_status: coverage.status, expected_units: coverage.expected_unit_ids?.length ?? null,
      observed_units: coverage.observed_unit_ids?.length ?? null,
      missing: coverage.missing_unit_ids, unexpected: coverage.unexpected_unit_ids,
      duplicate: coverage.duplicate_unit_ids, invalid_shards: coverage.invalid_shard_ids,
      collection_receipt_status: compilerReceipt.status, ir_schema: ir.schema_version });

  const policyBlake3 = policy.policy_blake3;
  check('receipt-policy-binding',
    'The architecture receipt binds the committed exported architecture policy and Cargo graph identity.',
    receipt.schema_version === 'octet-project-architecture-receipt/v2'
      && receipt.mode === 'gate'
      && receipt.project_id === 'noble'
      && receipt.policy_blake3 === policyBlake3
      && receipt.cargo_graph_id === graph.cargo_graph_id
      && receipt.architecture_ir_id === ir.architecture_ir_id
      && receipt.coverage_id === coverage.coverage_id
      && receipt.compiler_receipt_id === compilerReceipt.receipt_id,
    { schema: receipt.schema_version, mode: receipt.mode, project: receipt.project_id,
      policy_blake3: receipt.policy_blake3, expected_policy_blake3: policyBlake3,
      cargo_graph_binding: receipt.cargo_graph_id === graph.cargo_graph_id,
      ir_binding: receipt.architecture_ir_id === ir.architecture_ir_id,
      coverage_binding: receipt.coverage_id === coverage.coverage_id,
      collection_binding: receipt.compiler_receipt_id === compilerReceipt.receipt_id });

  check('architecture-clean',
    'The selected architecture gate completed clean with no findings, no waived findings and no waivers applied; every retained compiler unknown is classified by the reviewed policy.',
    receipt.status === 'clean'
      && Array.isArray(receipt.findings) && receipt.findings.length === 0
      && Array.isArray(receipt.applied_waiver_ids) && receipt.applied_waiver_ids.length === 0
      && Array.isArray(receipt.unknown_ids)
      && run.parsed?.phases?.architecture?.status === 'clean',
    { status: receipt.status, findings: receipt.findings, applied_waivers: receipt.applied_waiver_ids,
      retained_unknown_ids: receipt.unknown_ids.length,
      phase_status: run.parsed?.phases?.architecture?.status ?? null });

  const memberships = Array.isArray(ir.unit_memberships) ? ir.unit_memberships : [];
  const factById = new Map((ir.facts ?? []).map(fact => [fact.fact_id, fact]));
  const unknownById = new Map((ir.unknowns ?? []).map(unknown => [unknown.unknown_id, unknown]));
  const membershipByUnit = new Map(memberships.map(membership => [membership.unit_id, membership]));
  const units = (ir.units ?? []).filter(unit => unit.package_name === packageName);
  const ownedFacts = unit => {
    const membership = membershipByUnit.get(unit.unit_id);
    return (membership?.fact_ids ?? []).map(id => factById.get(id)).filter(Boolean);
  };
  const ownedUnknowns = unit => {
    const membership = membershipByUnit.get(unit.unit_id);
    return (membership?.unknown_ids ?? []).map(id => unknownById.get(id)).filter(Boolean);
  };

  check('ngraph-unit-roster',
    'The compiler observed noble-ngraph units, each with the policy role for its source root.',
    units.length > 0
      && units.some(unit => unit.source_path === `crates/${packageName}/src/lib.rs` && unit.role === 'domain-core')
      && units.every(unit => ['domain-core', 'test'].includes(unit.role))
      && units.every(unit => (unit.role === 'domain-core') === unit.source_path.startsWith(`crates/${packageName}/src/`)),
    { units: units.length, roster: units.map(unit => ({
      unit_id: unit.unit_id, target_kind: unit.target_kind, source_path: unit.source_path,
      role: unit.role, features: unit.features, default_features: unit.default_features,
    })) });

  check('selection-configuration',
    'Every observed unit selected the single declared target/feature row of the architecture policy.',
    units.length > 0
      && units.every(unit => unit.target_triple === 'x86_64-unknown-linux-gnu')
      && units.every(unit => unit.default_features === true)
      && units.every(unit => unit.features.length === 0
        || sorted(unit.features).join(',') === sorted(['live-test-barrier', 'safety-observe']).join(',')),
    { target: sorted(new Set(units.map(unit => unit.target_triple))),
      feature_sets: [...new Set(units.map(unit => sorted(unit.features).join(',')))],
      default_features: [...new Set(units.map(unit => unit.default_features))] });

  const noStdFacts = units.map(unit => {
    const predicate = `crate:no-std=${packageName}`;
    const active = ownedFacts(unit).filter(fact => fact.kind === 'configuration'
      && fact.predicate === predicate && fact.active === true);
    return { unit_id: unit.unit_id, source_path: unit.source_path, role: unit.role, facts: active.length };
  });
  check('no-std-compiler-confirmed',
    'Every observed noble-ngraph unit owns an active compiler configuration fact for `crate:no-std=noble-ngraph`.',
    units.length > 0 && noStdFacts.every(entry => entry.facts > 0),
    { predicate: `crate:no-std=${packageName}`, units: noStdFacts });

  const declaredPackages = new Set(graph.selected_packages ?? []);
  const ngraphEdges = (graph.edges ?? []).filter(edge => edge.caller_package === packageName);
  const normalEdges = ngraphEdges.filter(edge => edge.edge_kind === 'normal');
  const normalCallees = sorted(new Set(normalEdges.map(edge => edge.callee_package)));
  check('normal-dependencies',
    'The resolved Cargo graph contains exactly the two declared Noble normal edges out of noble-ngraph and no build edge.',
    ngraphEdges.length === normalEdges.length
      && normalCallees.join(',') === sorted(['noble-contracts', 'noble-kernel']).join(',')
      && normalEdges.every(edge => declaredPackages.has(edge.callee_package)),
    { edges: ngraphEdges, normal_callees: normalCallees,
      declared_packages: sorted(declaredPackages) });

  const manifest = fs.readFileSync(path.join(source, `crates/${packageName}/Cargo.toml`));
  const manifestText = manifest.toString();
  const manifestSections = {};
  let section = null;
  for (const line of manifestText.split('\n')) {
    const header = line.match(/^\[([^\]]+)\]$/);
    if (header) { section = header[1]; manifestSections[section] = manifestSections[section] ?? []; continue; }
    if (section && line.trim() && !line.trim().startsWith('#')) manifestSections[section].push(line.trim());
  }
  const manifestNormal = sorted((manifestSections.dependencies ?? []).map(line => line.split('=')[0].trim()).filter(Boolean));
  const thirdParty = ngraphEdges.filter(edge => !declaredPackages.has(edge.callee_package));
  check('manifest-dependencies',
    'The committed manifest declares exactly noble-contracts and noble-kernel as normal dependencies, with no build or third-party dependency.',
    manifestNormal.join(',') === sorted(['noble-contracts', 'noble-kernel']).join(',')
      && manifestSections['build-dependencies'] === undefined
      && thirdParty.length === 0,
    { manifest_sha256: sha256(manifest), normal: manifestNormal,
      build_dependencies: manifestSections['build-dependencies'] ?? null,
      third_party_edges: thirdParty });

  const effects = units.flatMap(unit => ownedFacts(unit)
    .filter(fact => fact.kind === 'effect')
    .map(fact => ({ unit_id: unit.unit_id, family: fact.capability_family, primitive: fact.primitive_definition })));
  const familyCounts = Object.fromEntries(forbiddenFamilies.map(family => [family, effects.filter(effect => effect.family === family).length]));
  const observedFamilies = sorted(new Set(effects.map(effect => effect.family)));
  const authorizedFactIds = new Set(receipt.authorized_effect_fact_ids ?? []);
  const authorizedOwned = units.flatMap(unit => ownedFacts(unit)
    .filter(fact => fact.kind === 'effect' && authorizedFactIds.has(fact.fact_id))
    .map(fact => fact.fact_id));
  check('forbidden-capability-uses-zero',
    'No compiler effect fact owned by a noble-ngraph unit uses any of the seven forbidden capability families, and none is an authorized outbound effect.',
    forbiddenFamilies.every(family => familyCounts[family] === 0) && authorizedOwned.length === 0,
    { families: familyCounts, observed_families: observedFamilies},
  );

  const ngraphUnknowns = units.flatMap(unit => ownedUnknowns(unit)
    .map(unknown => ({ unit_id: unit.unit_id, unknown_id: unknown.unknown_id, kind: unknown.kind,
      target_identity: unknown.target_identity, detail: unknown.detail })));
  const unclassified = ngraphUnknowns.filter(unknown => !['ambiguous-trait-dispatch', 'dynamic-dispatch'].includes(unknown.kind)
    || !identityTraits.some(trait => String(unknown.target_identity ?? '').startsWith(trait)));
  const requiredUnknownFindings = (receipt.findings ?? []).filter(finding => finding.code === 'required-compiler-unknown');
  check('required-unknowns-zero',
    'Every compiler unknown owned by a noble-ngraph unit is an identity-trait dynamic-dispatch unknown declared by the reviewed policy; otherwise there are none.',
    unclassified.length === 0 && requiredUnknownFindings.length === 0,
    { unknowns: ngraphUnknowns, unclassified, required_unknown_findings: requiredUnknownFindings.length,
      retained_unknown_ids: receipt.unknown_ids.length },
  );

  const ngraphText = units.flatMap(unit => ownedFacts(unit)).map(fact => JSON.stringify(fact)).join('\n');
  const traitFacts = units.flatMap(unit => ownedFacts(unit))
    .filter(fact => fact.kind === 'trait' && identityTraits.includes(fact.qualified_path));
  const traitCalls = units.flatMap(unit => ownedFacts(unit))
    .filter(fact => fact.kind === 'call' && identityTraits.some(trait =>
      String(fact.resolved_definition ?? '').startsWith(trait)));
  const traitImplementations = units.flatMap(unit => ownedFacts(unit))
    .filter(fact => fact.kind === 'implementation' && identityTraits.some(trait => fact.trait_definition === trait));
  const hasherHits = forbiddenHasherNeedles.filter(needle => ngraphText.includes(needle));
  check('identity-hash-injected',
    'The compiled noble-ngraph facts declare both injected identity traits, reach them through resolved trait calls or implementations, and select no standard hasher.',
    traitFacts.length >= identityTraits.length
      && (traitCalls.length > 0 || traitImplementations.length > 0)
      && hasherHits.length === 0,
    { traits: sorted(new Set(traitFacts.map(fact => fact.qualified_path))),
      resolved_trait_calls: sorted(new Set(traitCalls.map(fact => fact.resolved_definition))),
      implementations: traitImplementations.length,
      forbidden_hasher_needles: hasherHits });

  return { checks, receipt, graph, coverage, compilerReceipt, ir };
}

function summaryOf(evaluation) {
  const failed = evaluation.checks.filter(check => !check.ok);
  return {
    verdict: failed.length === 0 ? 'passed' : 'refused',
    checks: evaluation.checks.length,
    failed: failed.map(check => check.name),
  };
}

function main() {
  const options = parseArguments(process.argv.slice(2));
  const { root } = options;
  const source = cleanSourceIdentity(root);
  const selection = JSON.parse(fs.readFileSync(policyPath(root), 'utf8'));
  const architectureJson = fs.readFileSync(path.join(root, 'policy/architecture.json'));
  const architectureManifest = JSON.parse(fs.readFileSync(path.join(root, 'policy/architecture.manifest.json'), 'utf8'));
  const exportEntry = (architectureManifest.exports ?? []).find(entry => entry.output?.path === 'policy/architecture.json');
  const policy = {
    repository: 'OnixResearch/noble-lang',
    policy_blake3: sha256(architectureJson),
    manifest_output_blake3: exportEntry?.output?.blake3 ?? null,
    manifest_source_blake3: exportEntry?.source?.blake3 ?? null,
    manifest_sha256: fileSha256(path.join(root, 'policy/architecture.manifest.json')),
    architecture_json_sha256: sha256(architectureJson),
    core_no_std: JSON.parse(fs.readFileSync(path.join(root, 'policy/architecture.json'), 'utf8')).core_no_std,
  };
  if (exportEntry === undefined) throw new Error('policy/architecture.manifest.json does not export policy/architecture.json');
  if (exportEntry.output.blake3 !== policy.policy_blake3) {
    throw new Error(`policy/architecture.json is not the exported policy: manifest ${exportEntry.output.blake3} != ${policy.policy_blake3}`);
  }
  if (!policy.core_no_std?.some(obligation => obligation.scope === packageName)) {
    throw new Error(`policy/architecture.json does not declare the ${packageName} no_std obligation`);
  }

  const octetOutput = options.octetOutput;
  const pinnedOctet = resolveSelectedTool(root, selection, 'octet');
  const octetBinary = [octetOutput, pinnedOctet]
    .flatMap(base => [path.join(base, 'bin/cargo-octet'), path.join(base, 'cargo-octet')])
    .find(exists);
  if (!octetBinary) throw new Error(`no cargo-octet binary under ${octetOutput}`);
  if (!path.resolve(octetBinary).startsWith(`${pinnedOctet}/`)) {
    throw new Error(`octet binary ${octetBinary} is not inside the selected tool_paths.octet.output ${pinnedOctet}`);
  }
  const rustOutput = resolveSelectedTool(root, selection, 'quality_rust');
  const ccDirectory = selection.component_sync?.linker_bin;
  if (!ccDirectory || !exists(path.join(ccDirectory, 'cc'))) throw new Error(`selected linker_bin lacks cc: ${ccDirectory}`);
  const vendor = selection.component_sync?.vendor;
  if (!vendor || !exists(path.join(vendor, 'source-registry-0'))) throw new Error(`selected vendor lacks source-registry-0: ${vendor}`);

  fs.mkdirSync(options.output, { recursive: true });
  const { environment } = setupEnvironment(options.output, octetBinary, rustOutput, ccDirectory, vendor, options.jobs);

  const positiveDirectory = path.join(options.output, 'artifact-positive');
  const positiveRun = runOctet({ source: root, artifactDirectory: positiveDirectory, output: options.output,
    octetBinary, environment });
  const positiveArtifact = readArtifact(positiveDirectory);
  const positive = evaluate({ artifactDirectory: positiveDirectory, run: positiveRun, artifact: positiveArtifact,
    source: root, policy });
  const positiveSummary = summaryOf(positive);

  // Negative control: the same commit, exported with `git archive`, whose
  // noble-ngraph library gains a forbidden environment capability through an
  // aliased `std` crate. The predicates must refuse it for that reason.
  const controlSource = path.join(options.output, 'control-source');
  fs.mkdirSync(controlSource, { recursive: true });
  const archive = path.join(options.output, 'control-source.tar');
  const archiveRun = spawn(['git', '-C', root, 'archive', '--format=tar', '-o', archive, source.commit], { env: process.env });
  if (archiveRun.status !== 0) throw new Error(`git archive failed: ${archiveRun.stderr.toString()}`);
  const extract = spawn(['tar', '-xf', archive, '-C', controlSource], { env: process.env });
  if (extract.status !== 0) throw new Error(`tar extraction failed: ${extract.stderr.toString()}`);
  fs.rmSync(archive);
  const libFile = path.join(controlSource, `crates/${packageName}/src/lib.rs`);
  fs.writeFileSync(libFile, `${fs.readFileSync(libFile).toString()}${controlMutation}`);
  const controlLibSha256 = fileSha256(libFile);
  const controlEnvironment = { ...environment,
    CARGO_TARGET_DIR: path.join(options.output, 'control-target'),
    TMPDIR: path.join(options.output, 'control-tmp') };
  fs.mkdirSync(controlEnvironment.TMPDIR, { recursive: true });
  const controlDirectory = path.join(options.output, 'artifact-control');
  const controlRun = runOctet({ source: controlSource, artifactDirectory: controlDirectory,
    output: path.join(options.output, 'control-logs'), octetBinary, environment: controlEnvironment });
  let controlEvaluation = null;
  let controlArtifact = null;
  try {
    controlArtifact = readArtifact(controlDirectory);
    controlEvaluation = evaluate({ artifactDirectory: controlDirectory, run: controlRun, artifact: controlArtifact,
      source: controlSource, policy });
  } catch (error) {
    controlEvaluation = { checks: [{ name: 'artifact-schema', claim: 'Control artifact is present.', ok: false,
      observed: { error: error.message } }], receipt: null };
  }
  const controlSummary = summaryOf(controlEvaluation);
  const controlNamed = controlEvaluation.checks.filter(check => !check.ok).map(check => check.name);
  const controlEffectDetected = (() => {
    const receipt = controlEvaluation.receipt;
    if (!receipt) return false;
    const findings = (receipt.findings ?? []).filter(finding => finding.code === 'domain-core-effect'
      || finding.code === 'required-compiler-unknown' || finding.code === 'no-std-obligation');
    const evaluation = controlEvaluation.checks.find(check => check.name === 'forbidden-capability-uses-zero');
    const environment = evaluation?.observed?.families?.environment ?? 0;
    return findings.length > 0 || environment > 0;
  })();
  const controlVerdict = controlSummary.verdict === 'refused' && controlEffectDetected ? 'refused' : 'insufficient';

  const runner = fileSha256(fileURLToPath(import.meta.url));
  const receipt = {
    schema: 'noble-ngraph-library-boundary-receipt/v1',
    case: caseId,
    subject: packageName,
    assertion: 'compiler-derived capability boundary for noble-ngraph from the selected pinned Octet run, with one refused negative control; not a proof, not runtime observation, and not whole-program authenticity',
    verdict: positiveSummary.verdict === 'passed' && controlVerdict === 'refused' ? 'passed' : 'refused',
    source,
    tools: {
      octet: { output: pinnedOctet, binary: octetBinary, binary_sha256: fileSha256(octetBinary) },
      quality_rust: rustOutput,
      linker_bin: ccDirectory,
      vendor,
      node: { path: process.execPath, sha256: fileSha256(process.execPath), version: process.version },
      runner: { path: 'verification/ngraph-library-boundary/gate.mjs', sha256: runner },
    },
    inputs: {
      'policy/architecture.json': policy.architecture_json_sha256,
      'policy/architecture.manifest.json': policy.manifest_sha256,
      policy_blake3: policy.policy_blake3,
      'crates/noble-ngraph/Cargo.toml': sha256(fs.readFileSync(path.join(root, `crates/${packageName}/Cargo.toml`))),
      'Cargo.lock': fileSha256(path.join(root, 'Cargo.lock')),
      'Cargo.toml': fileSha256(path.join(root, 'Cargo.toml')),
      'dylint.toml': fileSha256(path.join(root, 'dylint.toml')),
      'policy/tool-selection.json': fileSha256(policyPath(root)),
    },
    command: {
      argv: [octetBinary, ...octetArgs, '--artifact-dir', positiveDirectory],
      cwd: root,
      environment,
      exit: positiveRun.result.status,
      signal: positiveRun.result.signal,
      duration_ms: positiveRun.result.duration_ms,
      stdout_sha256: sha256(positiveRun.result.stdout),
      stderr_sha256: sha256(positiveRun.result.stderr),
      stdout_bytes: positiveRun.result.stdout.length,
      stderr_bytes: positiveRun.result.stderr.length,
    },
    artifact: {
      directory: positiveDirectory,
      files: positiveArtifact.files,
      total_bytes: positiveArtifact.total_bytes,
      replay: { exit: positiveRun.verification.status,
        stdout_sha256: sha256(positiveRun.verification.stdout),
        status: positiveRun.verificationReport?.status ?? null },
    },
    evaluation: { ...positiveSummary, checks: positive.checks },
    control: {
      method: 'git archive of the verified commit, then an aliased `extern crate std` environment probe appended to crates/noble-ngraph/src/lib.rs',
      commit: source.commit,
      source_directory: controlSource,
      mutation: { file: `crates/${packageName}/src/lib.rs`, appended: controlMutation, sha256: controlLibSha256 },
      command: { argv: [octetBinary, ...octetArgs, '--artifact-dir', controlDirectory],
        exit: controlRun.result.status, signal: controlRun.result.signal,
        duration_ms: controlRun.result.duration_ms,
        stdout_sha256: sha256(controlRun.result.stdout),
        stderr_sha256: sha256(controlRun.result.stderr),
        stdout_bytes: controlRun.result.stdout.length,
        stderr_bytes: controlRun.result.stderr.length },
      artifact: controlArtifact ? { directory: controlDirectory, files: controlArtifact.files,
        total_bytes: controlArtifact.total_bytes } : { directory: controlDirectory, files: [] },
      verdict: controlVerdict,
      summary: { ...controlSummary, failed: controlNamed },
      checks: controlEvaluation.checks,
    },
    non_claims: [
      'Bounded compiler-derived facts for the selected Octet run only; not a proof or runtime observation.',
      'The architecture policy is an explicitly reviewed input; its role, provider, capability and no_std classifications are premises, not compiler theorems.',
      'Identity scheme implementations in composition roots, native code, the operating system and the hash function implementation retain separate assumptions.',
      'The negative control mutates only the scratch copy; the verified tree is unchanged and its commit/tree identities are recorded.',
    ],
  };

  const serialized = `${JSON.stringify(receipt, null, 2)}\n`;
  fs.writeFileSync(path.join(options.output, 'receipt.json'), serialized);
  if (options.receipt) {
    const destination = path.resolve(options.receipt);
    if (fs.existsSync(destination)) throw new Error(`refusing to overwrite receipt: ${destination}`);
    fs.writeFileSync(destination, serialized, { flag: 'wx' });
  }
  process.stdout.write(`${JSON.stringify({
    verdict: receipt.verdict,
    case: caseId,
    positive: positiveSummary,
    control: { verdict: controlVerdict, failed: controlNamed },
    checks: positiveSummary.checks,
    artifact: positiveDirectory,
    receipt: options.receipt ? path.resolve(options.receipt) : path.join(options.output, 'receipt.json'),
  })}\n`);
  process.exitCode = receipt.verdict === 'passed' ? 0 : 1;
}

try {
  main();
} catch (error) {
  process.stderr.write(`ngraph-library-boundary: ${error.message}\n`);
  process.exitCode = 2;
}
