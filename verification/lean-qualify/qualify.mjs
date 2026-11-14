#!/usr/bin/env node
import { spawn, spawnSync } from 'node:child_process';
import { existsSync, lstatSync, mkdirSync, readFileSync, realpathSync,
  readdirSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { homedir, userInfo } from 'node:os';
import { basename, dirname, join, relative, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { canonical, classifyChanges, compareDirectToManifest, compareManifests,
  dependencyToolchainMismatches, directDependencies, digest, manifestPackages, sourceRevision, bisectRegression,
  linearScan } from './core.mjs';

const repo = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const defaultGit = '/run/current-system/sw/bin/git';
const git = existsSync(defaultGit) ? realpathSync(defaultGit) : 'git';
const profileDefinitions = Object.freeze({
  'core-bootstrap': {
    name: 'core-bootstrap',
    projects: ['proofs/m3', 'proofs/foundation'],
    proofSources: ['proofs/mc1'],
    requiredCheckers: ['verification/ci/lean-proofs.sh', 'verification/foundation/differential.mjs'],
    requiredTools: ['lean', 'node', 'quality_rust', 'binaryen', 'wasm_tools'],
    explanation: 'M3 build/proof gate, foundation build/inventory/axiom audit, and seeded foundation differential replay controls.',
  },
  'mc1-export': {
    name: 'mc1-export',
    projects: ['proofs/mc1'],
    proofSources: ['proofs/mc1'],
    requiredCheckers: ['verification/mc1/gate.mjs'],
    requiredTools: ['lean', 'node', 'quality_rust'],
    unavailable: 'The MC1 export/checker runner is Bun-driven and Bun has no tool-selection pin in policy/tool-selection.json; it cannot be treated as an optional pass after Lake succeeds.',
  },
});

class QualificationError extends Error {
  constructor(code, message, stage = null) { super(message); this.code = code; this.stage = stage; }
}
class Cancelled extends Error { constructor() { super('cancelled by SIGINT'); this.code = 'CANCELLED'; } }
let cancelled = false;
let activeChild = null;
const onSigint = () => {
  cancelled = true;
  if (activeChild?.pid) { try { process.kill(-activeChild.pid, 'SIGTERM'); } catch {} }
};
process.on('SIGINT', onSigint);
export function requestCancellation() { onSigint(); }

function assert(condition, message, code = 'INVALID_INPUT') {
  if (!condition) throw new QualificationError(code, message);
}
function cleanPath(path) { return resolve(path); }
function safeOutput(path, root) {
  const absolute = cleanPath(path);
  assert(absolute !== root && !absolute.startsWith(`${root}${sep}`), 'output must be outside the repository');
  assert(!existsSync(absolute), `output already exists: ${absolute}`);
  mkdirSync(absolute, { recursive: true });
  return absolute;
}
function safeScratch(path, root) {
  const absolute = cleanPath(path);
  assert(absolute !== root && !absolute.startsWith(`${root}${sep}`), 'scratch must be outside the repository');
  if (existsSync(absolute)) {
    assert(lstatSync(absolute).isDirectory(), `scratch is not a directory: ${absolute}`);
    assert(readdirSync(absolute).length === 0, `scratch must be empty: ${absolute}`);
  } else mkdirSync(absolute, { recursive: true });
  return absolute;
}

function sync(command, args, options = {}) {
  const result = spawnSync(command, args, { cwd: options.cwd ?? repo, encoding: null,
    maxBuffer: 128 * 1024 * 1024, env: options.env ?? process.env });
  if (result.error || (result.status !== 0 && !options.allowFailure)) {
    const stderr = result.stderr?.toString('utf8') ?? '';
    throw new QualificationError('TOOL_ERROR', `${basename(command)} ${args.join(' ')} failed (${result.error?.message ?? result.status}): ${stderr}`);
  }
  return options.bytes ? result.stdout ?? Buffer.alloc(0) : (result.stdout?.toString('utf8') ?? '').trimEnd();
}
function gitText(args, cwd = repo) { return sync(git, args, { cwd }); }
function gitBytes(args, cwd = repo) { return sync(git, args, { cwd, bytes: true }); }
function gitState(cwd = repo) {
  return { head: gitText(['rev-parse', 'HEAD'], cwd), tree: gitText(['rev-parse', 'HEAD^{tree}'], cwd),
    status: gitText(['status', '--porcelain=v1', '--untracked-files=all'], cwd) };
}
function commitExists(revision) {
  return spawnSync(git, ['cat-file', '-e', `${revision}^{commit}`], { cwd: repo }).status === 0;
}
function gitFile(revision, path, cwd = repo) {
  const result = spawnSync(git, ['show', `${revision}:${path}`], { cwd, encoding: null, maxBuffer: 128 * 1024 * 1024 });
  if (result.error || result.status !== 0) return null;
  return result.stdout;
}
function readJsonBuffer(buffer, label) {
  try { return JSON.parse(buffer.toString('utf8')); }
  catch (error) { throw new QualificationError('INVALID_REVISION_SET', `${label} is not valid JSON: ${error.message}`); }
}
function commitTree(revision) { return gitText(['rev-parse', `${revision}^{tree}`]); }

function affectedPath(path, profile) {
  return [...profile.projects, ...(profile.proofSources ?? [])].some(project => path === project || path.startsWith(`${project}/`)) ||
    path === 'proofs/inets/UPSTREAM.json' || path.startsWith('proofs/inets/') ||
    path === 'policy/tool-selection.json' || path === 'policy/lean-inputs.json' ||
    path.startsWith('verification/lean-qualify/') || [
      'verification/ci/lean-proofs.sh', 'verification/m3-proof-gate.sh',
      'verification/lean-source-scan.mjs', 'verification/foundation/check.mjs',
      'verification/foundation/claims.mjs', 'verification/foundation/differential.mjs',
      'verification/foundation/arithmetic.mjs', 'verification/foundation/execution.mjs',
      'verification/foundation/emi-controls.mjs', 'verification/mc1/gate.mjs',
    ].includes(path);
}

function projectSnapshot(revision, project) {
  const manifestBytes = gitFile(revision, `${project}/lake-manifest.json`);
  const lakefileToml = gitFile(revision, `${project}/lakefile.toml`);
  const lakefileLean = gitFile(revision, `${project}/lakefile.lean`);
  const toolchain = gitFile(revision, `${project}/lean-toolchain`);
  if (!manifestBytes || !toolchain || (!lakefileToml && !lakefileLean)) {
    throw new QualificationError('PROFILE_INPUT_MISSING', `${project} lacks a Lake manifest, lakefile, or Lean toolchain`);
  }
  const manifest = readJsonBuffer(manifestBytes, `${project}/lake-manifest.json`);
  const lakefile = lakefileToml ?? lakefileLean;
  const direct = directDependencies(lakefile.toString('utf8'), lakefileToml ? 'toml' : 'lean');
  return { project, manifest_path: `${project}/lake-manifest.json`,
    manifest_sha256: digest(manifestBytes), lakefile_path: lakefileToml ? `${project}/lakefile.toml` : `${project}/lakefile.lean`,
    lakefile_sha256: digest(lakefile), toolchain: toolchain.toString('utf8').trim(),
    toolchain_sha256: digest(toolchain), manifest, direct, direct_comparison: compareDirectToManifest(direct, manifest),
    packages: manifestPackages(manifest) };
}

function packageCheckout(project, name) {
  const path = join(repo, project, '.lake/packages', name);
  if (!existsSync(path)) return null;
  const topResult = spawnSync(git, ['rev-parse', '--show-toplevel'], { cwd: path, encoding: 'utf8' });
  if (topResult.status !== 0) return null;
  return { path, top: resolve(topResult.stdout.trim()) };
}

function packageConfigSnapshot(project, pkg) {
  const checkout = packageCheckout(project, pkg.name);
  if (!checkout) return { package: pkg.name, available: false, reason: 'package checkout missing from local cache' };
  const revisionAvailable = spawnSync(git, ['cat-file', '-e', `${pkg.rev}^{commit}`], { cwd: checkout.top });
  if (revisionAvailable.status !== 0) return { package: pkg.name, available: false,
    path: checkout.path, reason: `pinned commit ${pkg.rev} is absent from local package object store` };
  const state = { head: gitText(['rev-parse', 'HEAD'], checkout.top), tree: gitText(['rev-parse', `${pkg.rev}^{tree}`], checkout.top),
    status: gitText(['status', '--porcelain=v1', '--untracked-files=all'], checkout.top) };
  const configPath = [pkg.subDir, pkg.configFile].filter(Boolean).join('/');
  const config = gitFile(pkg.rev, configPath, checkout.top);
  const manifestPath = [pkg.subDir, pkg.manifestFile].filter(Boolean).join('/');
  const nestedManifest = pkg.manifestFile ? gitFile(pkg.rev, manifestPath, checkout.top) : null;
  const toolchain = gitFile(pkg.rev, [pkg.subDir, 'lean-toolchain'].filter(Boolean).join('/'), checkout.top);
  const parsedDirect = config ? directDependencies(config.toString('utf8'),
    configPath.endsWith('.toml') ? 'toml' : configPath.endsWith('.lean') ? 'lean' : 'unknown') : null;
  return { package: pkg.name, available: true, path: checkout.path, repository: checkout.top,
    revision: pkg.rev, cached_head: state.head, cached_tree: state.tree, cached_status: state.status,
    config_path: configPath, config_sha256: config ? digest(config) : null,
    nested_manifest_path: manifestPath, nested_manifest_sha256: nestedManifest ? digest(nestedManifest) : null,
    toolchain: toolchain?.toString('utf8').trim() ?? null,
    direct: parsedDirect, direct_dependencies_not_in_root_manifest: parsedDirect
      ? parsedDirect.entries.filter(item => !pkgMap(pkg).has(item.name)) : [] };
}
function pkgMap(pkg) { return new Map((pkg.packages ?? []).map(item => [item.name, item])); }

function snapshotRevision(revision, profile) {
  const names = gitText(['ls-tree', '-r', '--name-only', revision]).split('\n').filter(Boolean);
  const files = {};
  for (const path of names.filter(item => affectedPath(item, profile))) {
    const bytes = gitFile(revision, path);
    if (bytes) files[path] = digest(bytes);
  }
  const projects = Object.fromEntries(profile.projects.map(project => [project, projectSnapshot(revision, project)]));
  const package_sources = [];
  for (const [project, data] of Object.entries(projects)) {
    for (const pkg of data.packages ?? []) package_sources.push({ project,
      ...packageConfigSnapshot(project, { ...pkg, packages: data.packages }) });
  }
  const policyBytes = gitFile(revision, 'policy/tool-selection.json');
  if (!policyBytes) throw new QualificationError('TOOL_POLICY_MISSING', 'policy/tool-selection.json is missing');
  const policy = readJsonBuffer(policyBytes, 'policy/tool-selection.json');
  const toolSelection = {
    schema: policy.schema_version ?? null,
    lean: policy.lean ? { toolchain: policy.lean.toolchain, version: policy.lean.version,
      revision: policy.lean.rev, output: policy.tool_paths?.lean?.output } : null,
    node: { output: policy.tool_paths?.node?.output,
      version: policy.verification_tools?.node?.version ?? null },
    quality_rust: { output: policy.tool_paths?.quality_rust?.output },
    binaryen: { output: policy.tool_paths?.binaryen?.output },
    wasm_tools: { output: policy.tool_paths?.wasm_tools?.output },
  };
  return { revision, tree: commitTree(revision), files, projects, package_sources,
    tool_policy: { sha256: digest(policyBytes), selected: toolSelection } };
}

function changedPaths(baseline, proposal) {
  const raw = gitText(['diff', '--no-ext-diff', '--name-only', baseline, proposal]);
  return raw.split('\n').filter(Boolean).sort();
}
function makePatch(baseline, proposal) {
  return gitBytes(['diff', '--no-ext-diff', '--binary', baseline, proposal]);
}
function sourceDeltas(baseline, proposal, baselineSnapshot, proposalSnapshot, output) {
  const byProject = new Map();
  for (const snap of [baselineSnapshot, proposalSnapshot]) {
    for (const pkg of snap.package_sources) {
      const key = `${pkg.project}:${pkg.package}`;
      const rows = byProject.get(key) ?? [];
      rows.push(pkg); byProject.set(key, rows);
    }
  }
  const files = [];
  const packagesChanged = [];
  for (const project of Object.keys(baselineSnapshot.projects)) {
    const before = new Map(baselineSnapshot.projects[project].packages.map(pkg => [pkg.name, pkg]));
    const after = new Map(proposalSnapshot.projects[project].packages.map(pkg => [pkg.name, pkg]));
    for (const name of [...new Set([...before.keys(), ...after.keys()])].sort()) {
      const oldPkg = before.get(name), newPkg = after.get(name);
      if (!oldPkg || !newPkg || oldPkg.rev === newPkg.rev) continue;
      const oldSource = byProject.get(`${project}:${name}`)?.find(row => row.revision === oldPkg.rev && row.available);
      const newSource = byProject.get(`${project}:${name}`)?.find(row => row.revision === newPkg.rev && row.available);
      if (!oldSource || !newSource || oldSource.repository !== newSource.repository) {
        packagesChanged.push({ project, name, old_revision: oldPkg.rev, new_revision: newPkg.rev,
          diff: null, error: 'immutable source diff unavailable locally; fetch the revisions into a reviewed local mirror before qualification' });
        continue;
      }
      const patch = sync(git, ['diff', '--no-ext-diff', '--binary', oldPkg.rev, newPkg.rev], { cwd: oldSource.repository, bytes: true });
      const filename = `package-diffs/${project.replaceAll('/', '-')}-${name}.patch`;
      mkdirSync(join(output, 'package-diffs'), { recursive: true });
      writeFileSync(join(output, filename), patch);
      const record = { project, name, old_revision: oldPkg.rev, new_revision: newPkg.rev,
        file: filename, sha256: digest(patch), bytes: patch.length };
      packagesChanged.push(record); files.push(record);
    }
  }
  return { files, packagesChanged };
}

function revisionSet(revision, profileName) {
  sourceRevision(revision);
  if (!commitExists(revision)) throw new QualificationError('REVISION_MISSING', `unknown immutable commit ${revision}`);
  return { revision: gitText(['rev-parse', `${revision}^{commit}`]), profile: profileName };
}

function planQualification({ baseline, proposal, profileName, output }) {
  const profile = profileDefinitions[profileName];
  if (!profile) throw new QualificationError('PROFILE_UNAVAILABLE', `consumer profile '${profileName}' is unavailable; choose a profile with an explicit required-checker roster`);
  const current = gitState(repo);
  assert(current.status === '', `starting repository must be clean; found ${current.status}`, 'DIRTY_SOURCE');
  const base = revisionSet(baseline, profileName), next = revisionSet(proposal, profileName);
  const baseSnapshot = snapshotRevision(base.revision, profile), proposalSnapshot = snapshotRevision(next.revision, profile);
  const patch = makePatch(base.revision, next.revision);
  const patchPath = join(output, 'candidate.patch');
  writeFileSync(patchPath, patch);
  const paths = changedPaths(base.revision, next.revision);
  const classes = classifyChanges(paths);
  const classPatches = {};
  mkdirSync(join(output, 'change-classes'), { recursive: true });
  for (const [name, key] of Object.entries({ pins: 'proposed_pin_changes', repairs: 'proof_only_repairs',
    semantics: 'semantic_changes', verifier_policy: 'verifier_policy_changes' })) {
    const classPaths = classes[key];
    const bytes = classPaths.length
      ? gitBytes(['diff', '--no-ext-diff', '--binary', base.revision, next.revision, '--', ...classPaths])
      : Buffer.alloc(0);
    const path = `change-classes/${name}.patch`;
    writeFileSync(join(output, path), bytes);
    classPatches[name] = { file: path, sha256: digest(bytes), bytes: bytes.length, paths: classPaths };
  }
  const deltas = sourceDeltas(base.revision, next.revision, baseSnapshot, proposalSnapshot, output);
  const directDiscrepancies = [];
  for (const [side, snapshot] of [['baseline', baseSnapshot], ['proposal', proposalSnapshot]]) {
    for (const [project, value] of Object.entries(snapshot.projects)) {
      directDiscrepancies.push(...value.direct_comparison.discrepancies.map(item => ({ side, project, ...item })));
    }
    for (const row of snapshot.package_sources) {
      for (const item of row.direct_dependencies_not_in_root_manifest ?? []) directDiscrepancies.push({
        side, project: row.project, package: row.package, ...item,
        kind: 'upstream-lakefile-dependency-not-in-manifest', source: 'pre-update-plan', scope: 'upstream',
      });
    }
  }
  const toolchainDiscrepancies = [...dependencyToolchainMismatches(baseSnapshot, 'baseline'),
    ...dependencyToolchainMismatches(proposalSnapshot, 'proposal')];
  const changedPackageSourcesUnavailable = deltas.packagesChanged.filter(row => row.error);
  const incompleteDependencyPlans = [];
  for (const [side, snapshot] of [['baseline', baseSnapshot], ['proposal', proposalSnapshot]]) {
    for (const [project, row] of Object.entries(snapshot.projects)) {
      if (!row.direct.complete) incompleteDependencyPlans.push({ side, project, config_path: row.lakefile_path,
        evaluator: row.direct.evaluator, scope: 'consumer' });
    }
    for (const row of snapshot.package_sources) {
      if (row.available && row.direct && !row.direct.complete) incompleteDependencyPlans.push({
        side, project: row.project, package: row.package, config_path: row.config_path, evaluator: row.direct.evaluator,
        scope: 'upstream',
      });
    }
  }
  const patchSha = digest(patch);
  const snapshotSha = digest(Buffer.from(canonical({ baseline: baseSnapshot, proposal: proposalSnapshot })));
  const packageDiffsSha = deltas.files.map(item => `${item.file}:${item.sha256}`).sort().join('\n');
  const reviewSha = digest(Buffer.from(`${patchSha}\n${snapshotSha}\n${packageDiffsSha}\n`));
  const plan = {
    schema: 'noble-lean-qualification-plan/v1', created_at: new Date().toISOString(),
    repository: repo, invoking_source: current,
    profile: { name: profileName, explanation: profile.explanation, projects: profile.projects,
      proof_sources: profile.proofSources ?? [], required_checkers: profile.requiredCheckers,
      required_tools: profile.requiredTools },
    baseline: { ...base, snapshot: baseSnapshot }, proposal: { ...next, snapshot: proposalSnapshot },
    source_diff: { file: 'candidate.patch', sha256: patchSha, bytes: patch.length, changed_paths: paths,
      classes, class_patches: classPatches },
    package_source_diffs: deltas.packagesChanged,
    pre_update_plan: { evaluator: 'native data-only Lakefile parser; no Lakefile or package code executed',
      limitations: [
        'Dynamic Lean lakefile declarations and dependencies introduced only by a newly resolved upstream package cannot be predicted by the static pre-update plan.',
        'After the approved source diff, Lake resolution is run in the disposable proposal worktree and its complete manifest is compared exactly with this prediction.',
      ], discrepancies: directDiscrepancies,
      toolchain_discrepancies: toolchainDiscrepancies,
      incomplete_upstream_configs: incompleteDependencyPlans },
    review: { required: true, sha256: reviewSha, approval_flag: '--approve-diff',
      approval_covers: ['candidate.patch', 'external package source diffs', 'baseline/proposal snapshots'] },
    blockers: [
      ...(profile.unavailable ? [{ code: 'REQUIRED_CHECKER_UNAVAILABLE', message: profile.unavailable }] : []),
      ...changedPackageSourcesUnavailable.map(row => ({ code: 'PACKAGE_DIFF_UNAVAILABLE', ...row })),
      ...directDiscrepancies.filter(row => row.scope !== 'upstream')
        .map(row => ({ code: 'PRE_UPDATE_DISCREPANCY', ...row })),
      ...toolchainDiscrepancies.map(row => ({ code: 'INCOMPATIBLE_LEAN_LIBRARY', ...row,
        message: `${row.package} selects ${row.dependency_toolchain}, but its consumer selects ${row.consumer_toolchain}` })),
      ...incompleteDependencyPlans.filter(row => row.scope !== 'upstream')
        .map(row => ({ code: 'DEPENDENCY_PLAN_INCOMPLETE', ...row,
        message: 'The data-only planner could not account for every require declaration; no candidate run is authorized' })),
    ],
    no_op: base.revision === next.revision,
    projects_requiring_resolution: profile.projects.filter(project =>
      baseSnapshot.projects[project].manifest_sha256 !== proposalSnapshot.projects[project].manifest_sha256 ||
      baseSnapshot.projects[project].lakefile_sha256 !== proposalSnapshot.projects[project].lakefile_sha256),
    policy: { adapters: 'native implementation; paranoid-lake-update and hopscotch are not used',
      localization: 'native monotone bisection plus a complete linear scan; bisection assumes a good start and monotonic outcomes' },
  };
  plan.qualification_status = plan.blockers.length ? 'unqualified' : 'awaiting-reviewed-source-diff';
  plan.plan_sha256 = digest(Buffer.from(canonical(plan)));
  writeFileSync(join(output, 'plan.json'), `${JSON.stringify(plan, null, 2)}\n`);
  return plan;
}

function checkRootState(expected) {
  const actual = gitState(repo);
  if (canonical(actual) !== canonical(expected)) throw new QualificationError('SOURCE_CHANGED',
    `starting source/evidence changed after plan: expected ${canonical(expected)}, observed ${canonical(actual)}`);
  return actual;
}

function selectedPolicy(root, revision) {
  const bytes = readFileSync(join(root, 'policy/tool-selection.json'));
  const policy = JSON.parse(bytes);
  const selected = policy.tool_paths ?? {};
  const required = ['lean', 'node'];
  for (const name of required) {
    const output = selected[name]?.output;
    if (!output || !existsSync(join(output, 'bin', name === 'node' ? 'node' : 'lean')))
      throw new QualificationError('TOOL_UNAVAILABLE', `required policy-selected ${name} is unavailable at ${output ?? '(no pinned output)'}`);
  }
  const toolchain = readFileSync(join(root, 'proofs/m3/lean-toolchain'), 'utf8').trim();
  const foundationToolchain = readFileSync(join(root, 'proofs/foundation/lean-toolchain'), 'utf8').trim();
  if (toolchain !== foundationToolchain) throw new QualificationError('TOOLCHAIN_MISMATCH',
    `proofs/m3 selects ${toolchain}, proofs/foundation selects ${foundationToolchain}`);
  const leanVersion = policy.lean?.version;
  if (!leanVersion || !toolchain.includes(leanVersion)) throw new QualificationError('TOOLCHAIN_MISMATCH',
    `proof package toolchain ${toolchain} disagrees with selected Lean policy version ${leanVersion ?? '(missing)'}`);
  return { policy, lean: join(selected.lean.output, 'bin/lean'), lake: join(selected.lean.output, 'bin/lake'),
    node: join(selected.node.output, 'bin/node'), toolchain, leanVersion,
    rust: selected.quality_rust?.output ? join(selected.quality_rust.output, 'bin') : null,
    binaryen: selected.binaryen?.output ? join(selected.binaryen.output, 'bin') : null,
    wasmTools: selected.wasm_tools?.output ? join(selected.wasm_tools.output, 'bin') : null };
}

function childEnvironment(tools, output) {
  const { homedir: home, username } = userInfo();
  const paths = [dirname(tools.lean), dirname(tools.node), tools.rust, tools.binaryen, tools.wasmTools,
    '/run/current-system/sw/bin', `/etc/profiles/per-user/${username}/bin`, '/usr/bin', '/bin'].filter(Boolean);
  return { HOME: home ?? homedir(), USER: username, LOGNAME: username, LANG: 'C.UTF-8', CI: '1',
    GIT_TERMINAL_PROMPT: '0', PATH: [...new Set(paths)].join(':'), TMPDIR: join(output, 'tmp'),
    NOBLE_TARGET: '/dev/shm/noble-iss/target/qualify-126', CARGO_TARGET_DIR: '/dev/shm/noble-iss/target/qualify-126',
    RUSTC_WRAPPER: '', RUSTFLAGS: '', LEAN_ABORT_ON_PANIC: '1' };
}

function capturePackageState(path) {
  const top = sync(git, ['rev-parse', '--show-toplevel'], { cwd: path });
  return { head: sync(git, ['rev-parse', 'HEAD'], { cwd: top }),
    tree: sync(git, ['rev-parse', 'HEAD^{tree}'], { cwd: top }),
    status: sync(git, ['status', '--porcelain=v1', '--untracked-files=all'], { cwd: top }) };
}

function seedProjectCaches(candidate, sourceRoot, project, scratch, cacheRows) {
  const sourceProject = join(sourceRoot, project), candidateProject = join(candidate, project);
  const sourceLake = join(sourceProject, '.lake'), targetLake = join(candidateProject, '.lake');
  mkdirSync(targetLake, { recursive: true });
  const sourceBuild = join(sourceLake, 'build');
  if (existsSync(sourceBuild)) {
    const targetBuild = join(targetLake, 'build');
    const copied = spawnSync('/run/current-system/sw/bin/cp', ['-a', '--reflink=always', sourceBuild, targetBuild], {
      encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'],
    });
    if (copied.status !== 0) throw new QualificationError('CACHE_ISOLATION_UNAVAILABLE',
      `could not create copy-on-write Lean cache for ${project}; refusing an expensive/unsafe ordinary copy: ${copied.error?.message ?? (copied.stderr ?? '').trim()}`);
  }
  const packagesDir = join(targetLake, 'packages');
  mkdirSync(packagesDir, { recursive: true });
  const manifest = JSON.parse(readFileSync(join(candidateProject, 'lake-manifest.json'), 'utf8'));
  for (const pkg of manifest.packages ?? []) {
    const sourcePkg = join(sourceLake, 'packages', pkg.name);
    if (!existsSync(sourcePkg)) throw new QualificationError('PACKAGE_CACHE_UNAVAILABLE',
      `${project}/${pkg.name} has no locally retained package source; this profile cannot resolve or rebuild it offline`);
    const top = sync(git, ['rev-parse', '--show-toplevel'], { cwd: sourcePkg });
    const cachedRev = sync(git, ['rev-parse', 'HEAD'], { cwd: top });
    const cachedStatus = sync(git, ['status', '--porcelain=v1', '--untracked-files=all'], { cwd: top });
    let target = sourcePkg;
    if (cachedRev !== pkg.rev || cachedStatus !== '') {
      const object = spawnSync(git, ['cat-file', '-e', `${pkg.rev}^{commit}`], { cwd: top });
      if (object.status !== 0) throw new QualificationError('PACKAGE_REVISION_UNAVAILABLE',
        `${project}/${pkg.name} requires ${pkg.rev}, but the local package object store does not contain it; fetch and review that immutable source before qualification`);
      if (['aeneas', 'mathlib'].includes(pkg.name.toLowerCase())) throw new QualificationError('HEAVY_PACKAGE_UPDATE_REFUSED',
        `${pkg.name} changed from cached ${cachedRev} to ${pkg.rev}; candidate Mathlib/Aeneas rebuilds are forbidden by this qualification lane`);
      target = join(scratch, 'package-checkouts', project.replaceAll('/', '-'), pkg.name);
      mkdirSync(dirname(target), { recursive: true });
      const clone = sync(git, ['clone', '--shared', '--no-checkout', top, target], { cwd: repo, allowFailure: true });
      if (!existsSync(join(target, '.git'))) throw new QualificationError('PACKAGE_CHECKOUT_FAILED',
        `could not create isolated checkout for ${project}/${pkg.name}: ${clone}`);
      sync(git, ['checkout', '--detach', pkg.rev], { cwd: target });
    }
    const packageToolchainPath = join(target, pkg.subDir ?? '', 'lean-toolchain');
    if (existsSync(packageToolchainPath)) {
      const packageToolchain = readFileSync(packageToolchainPath, 'utf8').trim();
      const projectToolchain = readFileSync(join(candidateProject, 'lean-toolchain'), 'utf8').trim();
      if (packageToolchain !== projectToolchain) throw new QualificationError('DEPENDENCY_TOOLCHAIN_MISMATCH',
        `${project}/${pkg.name} selects ${packageToolchain}, but the consumer proof package selects ${projectToolchain}`);
    }
    const link = join(packagesDir, pkg.name);
    if (existsSync(link)) throw new QualificationError('CACHE_SETUP_FAILED', `unexpected candidate package path: ${link}`);
    symlinkSync(target, link, 'dir');
    if (target === sourcePkg) cacheRows.push({ project, package: pkg.name, path: sourcePkg,
      state: capturePackageState(sourcePkg) });
  }
}

async function runProcess(command, args, { cwd, env, logPath, timeoutMs = 3600000 } = {}) {
  if (cancelled) throw new Cancelled();
  mkdirSync(dirname(logPath), { recursive: true });
  const child = spawn(command, args, { cwd, env, detached: true, stdio: ['ignore', 'pipe', 'pipe'] });
  activeChild = child;
  const stdout = [], stderr = [];
  let size = 0, timedOut = false;
  const timer = setTimeout(() => { timedOut = true; try { process.kill(-child.pid, 'SIGTERM'); } catch {} }, timeoutMs);
  const collect = (chunks, stream) => stream.on('data', chunk => {
    size += chunk.length;
    if (size > 128 * 1024 * 1024) { try { process.kill(-child.pid, 'SIGTERM'); } catch {} }
    else chunks.push(chunk);
  });
  collect(stdout, child.stdout); collect(stderr, child.stderr);
  const result = await new Promise((resolveResult, reject) => {
    child.once('error', reject);
    child.once('close', (code, signal) => resolveResult({ code, signal }));
  }).finally(() => { clearTimeout(timer); activeChild = null; });
  const outBytes = Buffer.concat(stdout), errBytes = Buffer.concat(stderr);
  writeFileSync(logPath, Buffer.concat([Buffer.from(`$ ${command} ${args.join(' ')}\n`), outBytes,
    Buffer.from('\n--- stderr ---\n'), errBytes]));
  if (cancelled) throw new Cancelled();
  if (timedOut) return { status: 124, signal: result.signal, stdout: outBytes.toString(), stderr: errBytes.toString(), timed_out: true };
  if (size > 128 * 1024 * 1024) return { status: 125, signal: result.signal, stdout: '', stderr: 'output cap exceeded', output_capped: true };
  return { status: result.code ?? (result.signal ? 128 : 1), signal: result.signal,
    stdout: outBytes.toString('utf8'), stderr: errBytes.toString('utf8') };
}

async function stage(command, args, options, record) {
  const result = await runProcess(command, args, options);
  record.exit = result.status; record.signal = result.signal ?? null;
  record.stdout_sha256 = digest(Buffer.from(result.stdout)); record.stderr_sha256 = digest(Buffer.from(result.stderr));
  record.log = relative(options.outputRoot, options.logPath);
  record.timed_out = result.timed_out ?? false;
  return result;
}

function assertRequiredTools(profile, tools) {
  for (const name of profile.requiredTools) {
    if (name === 'lean' || name === 'node') continue;
    if (!tools.policy.tool_paths?.[name]?.output || !existsSync(tools.policy.tool_paths[name].output)) {
      throw new QualificationError('REQUIRED_CHECKER_UNAVAILABLE',
        `required ${name} tool for profile is not available from policy/tool-selection.json; qualification cannot continue after a successful Lake build`);
    }
  }
  for (const checker of profile.requiredCheckers) {
    if (!existsSync(join(repo, checker))) throw new QualificationError('REQUIRED_CHECKER_UNAVAILABLE',
      `required checker ${checker} is missing; Lake success cannot make it optional`);
  }
}

async function runOneRevision({ revision, side, plan, output, scratch, profileName }) {
  const profile = profileDefinitions[profileName];
  const worktree = join(scratch, `worktree-${side}-${revision.slice(0, 12)}-${Date.now()}`);
  const worktrees = [];
  const sharedPackageStates = [];
  const row = { side, revision, tree: null, toolchain: null, outcome: 'fail', stages: [], diagnostics: [] };
  let worktreeAdded = false;
  try {
    sync(git, ['worktree', 'add', '--detach', worktree, revision]);
    worktreeAdded = true; worktrees.push(worktree);
    row.tree = gitText(['rev-parse', `${revision}^{tree}`]);
    row.toolchain = readFileSync(join(worktree, 'proofs/m3/lean-toolchain'), 'utf8').trim();
    const tools = selectedPolicy(worktree, revision);
    row.actual_toolchain = tools.toolchain;
    assertRequiredTools(profile, tools);
    const env = childEnvironment(tools, output);
    mkdirSync(env.TMPDIR, { recursive: true });
    mkdirSync(env.NOBLE_TARGET, { recursive: true });
    for (const project of profile.projects) seedProjectCaches(worktree, repo, project, scratch, sharedPackageStates);

    const leanVersion = await runProcess(tools.lean, ['--version'], { cwd: worktree, env,
      logPath: join(output, 'stages', `${side}-lean-version.log`) });
    const nodeVersion = await runProcess(tools.node, ['--version'], { cwd: worktree, env,
      logPath: join(output, 'stages', `${side}-node-version.log`) });
    row.executables = { lean: leanVersion.stdout.trim(), node: nodeVersion.stdout.trim() };
    if (leanVersion.status !== 0 || !leanVersion.stdout.includes(`version ${tools.leanVersion}`))
      throw new QualificationError('TOOLCHAIN_MISMATCH',
        `policy-selected Lean reports '${leanVersion.stdout.trim()}', expected ${tools.leanVersion}`);
    const requiredNodeVersion = tools.policy.verification_tools?.node?.version;
    if (nodeVersion.status !== 0 || (requiredNodeVersion && !nodeVersion.stdout.includes(requiredNodeVersion)))
      throw new QualificationError('REQUIRED_CHECKER_UNAVAILABLE',
        `policy-selected Node reports '${nodeVersion.stdout.trim()}', expected ${requiredNodeVersion ?? 'the tool-selection policy version'}`);

    for (const project of profile.projects) {
      let resolutionMethod = 'locked-manifest-exact-comparison';
      if (side === 'proposal' && plan.projects_requiring_resolution.includes(project)) {
        resolutionMethod = 'lake-update-after-reviewed-source-diff';
        const record = { name: `lake-resolve:${project}`, command: [tools.lake, 'update'], cwd: join(worktree, project), required: true };
        const result = await stage(tools.lake, ['update'], { cwd: join(worktree, project), env,
          logPath: join(output, 'stages', `${side}-${project.replaceAll('/', '-')}-lake-update.log`), outputRoot: output }, record);
        row.stages.push(record);
        if (result.status !== 0) throw new QualificationError('RESOLUTION_FAILED',
          `${project} Lake resolution failed with ${result.status}: ${result.stderr.slice(-2500)}`, record.name);
      }
      const actualBytes = readFileSync(join(worktree, project, 'lake-manifest.json'));
      const expected = JSON.parse(gitBytes(['show', `${revision}:${project}/lake-manifest.json`], repo).toString('utf8'));
      const actual = JSON.parse(actualBytes.toString('utf8'));
      const comparison = compareManifests(expected, actual);
      row.manifests ??= {};
      row.manifests[project] = { resolution_method: resolutionMethod,
        predicted_sha256: digest(Buffer.from(canonical(expected))), actual_sha256: digest(actualBytes),
        equal: comparison.equal, discrepancies: comparison.discrepancies };
      const resolutionPatch = gitBytes(['diff', '--no-ext-diff', '--binary', 'HEAD', '--', `${project}/lake-manifest.json`], worktree);
      const resolutionPath = join(output, `${side}-${project.replaceAll('/', '-')}-resolution.patch`);
      writeFileSync(resolutionPath, resolutionPatch);
      row.manifests[project].resolution_patch = { path: relative(output, resolutionPath),
        sha256: digest(resolutionPatch), bytes: resolutionPatch.length };
      if (!comparison.equal) throw new QualificationError('MANIFEST_DISCREPANCY',
        `${project} post-resolution manifest differs from the reviewed pin prediction: ${JSON.stringify(comparison.discrepancies)}`, record.name);
    }

    for (const checker of profile.requiredCheckers) {
      const path = join(worktree, checker);
      if (!existsSync(path)) throw new QualificationError('REQUIRED_CHECKER_UNAVAILABLE',
        `required checker ${checker} is unavailable in ${revision}; qualification cannot continue after a successful Lake build`);
    }
    const proofDir = join(output, side, 'lean-proofs');
    const proofRecord = { name: 'lean-proof-suite', command: ['bash', join(worktree, 'verification/ci/lean-proofs.sh'), proofDir],
      cwd: worktree, required: true };
    const proofResult = await stage('bash', proofRecord.command.slice(1), { cwd: worktree, env,
      logPath: join(output, 'stages', `${side}-lean-proofs.log`), outputRoot: output }, proofRecord);
    row.stages.push(proofRecord);
    if (proofResult.status !== 0) throw new QualificationError('PROOF_GATE_FAILED',
      `required proof/inventory/axiom gate failed (${proofResult.status}): ${proofResult.stderr.slice(-2500)}${proofResult.stdout.slice(-2500)}`,
      proofRecord.name);
    const foundationReceipt = join(proofDir, 'foundation', 'foundation-check.json');
    if (!existsSync(foundationReceipt)) throw new QualificationError('REQUIRED_CHECKER_UNAVAILABLE',
      'lean-proofs completed without foundation-check.json; theorem-inventory/axiom checker is required');

    const replayOut = join(output, side, 'foundation-differential');
    const differential = join(worktree, 'verification/foundation/differential.mjs');
    const replayArgs = [differential, '--seed', '15', '--count', '80', '--foundation', foundationReceipt,
      '--output', replayOut, '--target-dir', env.NOBLE_TARGET];
    const replayRecord = { name: 'foundation-differential', command: [tools.node, ...replayArgs], cwd: worktree, required: true };
    const replayResult = await stage(tools.node, replayArgs, { cwd: worktree, env,
      logPath: join(output, 'stages', `${side}-foundation-differential.log`), outputRoot: output }, replayRecord);
    row.stages.push(replayRecord);
    if (replayResult.status !== 0) throw new QualificationError('REPLAY_CONTROL_FAILED',
      `required differential/replay controls failed (${replayResult.status}): ${replayResult.stderr.slice(-2500)}${replayResult.stdout.slice(-2500)}`,
      replayRecord.name);
    const receipt = join(replayOut, 'differential.json');
    if (!existsSync(receipt)) throw new QualificationError('REQUIRED_CHECKER_UNAVAILABLE',
      'differential checker succeeded without a retained receipt');
    const verifyArgs = [differential, '--verify', receipt];
    const verifyRecord = { name: 'foundation-differential-verify', command: [tools.node, ...verifyArgs], cwd: worktree, required: true };
    const verifyResult = await stage(tools.node, verifyArgs, { cwd: worktree, env,
      logPath: join(output, 'stages', `${side}-foundation-differential-verify.log`), outputRoot: output }, verifyRecord);
    row.stages.push(verifyRecord);
    if (verifyResult.status !== 0) throw new QualificationError('REPLAY_CONTROL_FAILED',
      `differential receipt verification failed (${verifyResult.status}): ${verifyResult.stderr.slice(-2500)}`,
      verifyRecord.name);

    row.outcome = 'pass';
    row.proof_receipt = relative(output, foundationReceipt);
    row.replay_receipt = relative(output, receipt);
    row.replay_verified = true;
  } catch (error) {
    row.outcome = 'fail';
    row.failure = { code: error.code ?? 'TOOL_ERROR', stage: error.stage ?? null, message: error.message };
    if (error instanceof Cancelled) throw error;
    if (worktreeAdded) row.diagnostics.push({ code: row.failure.code, message: row.failure.message });
  } finally {
    for (const item of sharedPackageStates) {
      const after = capturePackageState(item.path);
      if (canonical(after) !== canonical(item.state)) {
        row.outcome = 'fail';
        row.failure = { code: 'SHARED_PACKAGE_MUTATED', message: `${item.project}/${item.package} package cache changed` };
      }
    }
    for (const project of profile.projects) {
      const link = join(worktree, project, '.lake/packages');
      if (existsSync(link) && lstatSync(link).isDirectory()) rmSync(link, { recursive: true, force: true });
    }
    for (const item of worktrees.reverse()) {
      sync(git, ['worktree', 'remove', '--force', item], { cwd: repo, allowFailure: true });
      rmSync(item, { recursive: true, force: true });
    }
  }
  return row;
}

function verifyReviewPlan(plan, approval, planPath) {
  if (plan.schema !== 'noble-lean-qualification-plan/v1') throw new QualificationError('INVALID_PLAN', 'unknown plan schema');
  if (plan.repository !== repo) throw new QualificationError('INVALID_PLAN', 'plan belongs to a different repository checkout');
  const expectedHash = plan.review?.sha256;
  if (!expectedHash || approval !== expectedHash) throw new QualificationError('REVIEW_NOT_APPROVED',
    `source review is required: inspect candidate.patch and package diffs, then pass --approve-diff ${expectedHash ?? '<missing>'}`);
  if (plan.blockers.length) throw new QualificationError('PLAN_BLOCKED',
    `pre-update plan has blockers and cannot be qualified: ${JSON.stringify(plan.blockers)}`);
  const state = checkRootState(plan.invoking_source);
  const baseline = snapshotRevision(plan.baseline.revision, profileDefinitions[plan.profile.name]);
  const proposal = snapshotRevision(plan.proposal.revision, profileDefinitions[plan.profile.name]);
  const snapshotSha = digest(Buffer.from(canonical({ baseline, proposal })));
  if (canonical(baseline) !== canonical(plan.baseline.snapshot) || canonical(proposal) !== canonical(plan.proposal.snapshot))
    throw new QualificationError('STALE_PLAN', 'proof packages, tool policy, checker profile, or vendored reference changed after plan');
  const planDir = dirname(resolve(planPath));
  const patchPath = join(planDir, plan.source_diff.file);
  if (!existsSync(patchPath) || digest(readFileSync(patchPath)) !== plan.source_diff.sha256)
    throw new QualificationError('STALE_PLAN', 'candidate patch bytes changed after review');
  for (const item of plan.package_source_diffs ?? []) {
    if (!item.file) continue;
    const packagePatch = join(planDir, item.file);
    if (!existsSync(packagePatch) || digest(readFileSync(packagePatch)) !== item.sha256)
      throw new QualificationError('STALE_PLAN', `package source diff changed after review: ${item.file}`);
  }
  for (const item of Object.values(plan.source_diff.class_patches ?? {})) {
    const classPatch = join(planDir, item.file);
    if (!existsSync(classPatch) || digest(readFileSync(classPatch)) !== item.sha256)
      throw new QualificationError('STALE_PLAN', `classified patch changed after review: ${item.file}`);
  }
  const packageDiffSha = (plan.package_source_diffs ?? []).filter(item => item.file)
    .map(item => `${item.file}:${item.sha256}`).sort().join('\n');
  const actualReviewSha = digest(Buffer.from(`${plan.source_diff.sha256}\n${snapshotSha}\n${packageDiffSha}\n`));
  if (actualReviewSha !== expectedHash) throw new QualificationError('STALE_PLAN', 'review fingerprint does not match the retained source snapshots and diffs');
  if (state.head !== plan.invoking_source.head || state.tree !== plan.invoking_source.tree || state.status !== '')
    throw new QualificationError('STALE_PLAN', 'repository changed after the reviewed plan was produced');
  return true;
}

async function runQualification({ planPath, approval, output, scratch }) {
  const plan = readJsonBuffer(readFileSync(planPath), 'qualification plan');
  const profile = profileDefinitions[plan.profile?.name];
  const initial = gitState(repo);
  const report = { schema: 'noble-lean-qualification/v1', plan: resolve(planPath), plan_sha256: digest(readFileSync(planPath)),
    source_revision: plan.proposal.revision, source_tree: plan.proposal.snapshot.tree, profile: plan.profile,
    candidate_patch: { path: plan.source_diff.file, sha256: plan.source_diff.sha256 },
    candidate_change_classes: plan.source_diff.classes,
    proposed_pin_changes: plan.source_diff.classes.proposed_pin_changes,
    proof_only_repairs: plan.source_diff.classes.proof_only_repairs,
    semantic_changes: plan.source_diff.classes.semantic_changes,
    verifier_policy_changes: plan.source_diff.classes.verifier_policy_changes,
    pre_update_plan: plan.pre_update_plan, policy: plan.policy,
    baseline: null, proposal: null, localization: { status: 'not-requested', probes: [] },
    source_preservation: { before: initial, after: null, unchanged: false }, qualification_status: 'unqualified' };
  let baselineResult;
  try {
    if (!profile || profile.unavailable) throw new QualificationError('PROFILE_UNAVAILABLE',
      profile?.unavailable ?? `consumer profile '${plan.profile?.name}' is unavailable`);
    verifyReviewPlan(plan, approval, planPath);
    baselineResult = await runOneRevision({ revision: plan.baseline.revision, side: 'baseline', plan,
      output, scratch, profileName: profile.name });
    report.baseline = baselineResult;
    if (baselineResult.outcome !== 'pass') throw new QualificationError('BASELINE_UNVERIFIED',
      `baseline failed required verification (${baselineResult.failure?.code ?? 'unknown'}): ${baselineResult.failure?.message ?? ''}`);
    if (plan.no_op) {
      report.proposal = { ...baselineResult, side: 'proposal', reused_identical_revision: true };
    } else {
      report.proposal = await runOneRevision({ revision: plan.proposal.revision, side: 'proposal', plan,
        output, scratch, profileName: profile.name });
    }
    if (report.proposal.outcome !== 'pass') {
      report.diagnostics = [report.proposal.failure ?? { code: 'PROPOSAL_FAILED', message: 'proposal did not pass all required checks' }];
    } else {
      report.qualification_status = plan.no_op ? 'qualified-no-change' : 'qualified';
    }
  } catch (error) {
    report.diagnostics = [...(report.diagnostics ?? []), { code: error.code ?? 'TOOL_ERROR',
      stage: error.stage ?? null, message: error.message }];
    if (error instanceof Cancelled) report.qualification_status = 'cancelled';
  } finally {
    try { report.source_preservation.after = gitState(repo); }
    catch (error) { report.source_preservation.after_error = String(error); }
    report.source_preservation.unchanged = canonical(report.source_preservation.before) ===
      canonical(report.source_preservation.after);
    if (!report.source_preservation.unchanged) {
      report.qualification_status = 'unqualified';
      report.diagnostics = [...(report.diagnostics ?? []), { code: 'SOURCE_MUTATED',
        message: 'starting source tree changed during qualification' }];
    }
    report.completed_at = new Date().toISOString();
    report.report_sha256 = digest(Buffer.from(canonical(report)));
    writeFileSync(join(output, 'qualification.json'), `${JSON.stringify(report, null, 2)}\n`);
  }
  return report;
}

function parseArgs(args) {
  const operation = args.shift();
  const parsed = { operation };
  while (args.length) {
    const key = args.shift();
    if (!key.startsWith('--')) throw new QualificationError('USAGE', `unexpected argument ${key}`);
    const name = key.slice(2);
    if (name === 'help') { parsed.help = true; continue; }
    const value = args.shift();
    if (value === undefined || value.startsWith('--')) throw new QualificationError('USAGE', `${key} requires a value`);
    parsed[name] = value;
  }
  return parsed;
}
function usage() {
  return [
    'Usage:',
    '  node verification/lean-qualify/qualify.mjs plan --baseline <full-commit-sha> --proposal <full-commit-sha> --profile core-bootstrap --output <new-dir> --scratch <empty-dir>',
    '  node verification/lean-qualify/qualify.mjs run --plan <plan.json> --approve-diff <review-sha256> --output <new-dir> --scratch <empty-dir>',
    '  node verification/lean-qualify/qualify.mjs localize --baseline <full-commit-sha> --proposal <full-commit-sha> --history <json-array-of-commit-shas> --mode bisect|linear --plan <approved-plan.json> --approve-diff <review-sha256> --output <new-dir> --scratch <empty-dir>',
    '  node --test verification/lean-qualify/qualify.test.mjs',
    'Profile core-bootstrap is required and includes the existing M3/foundation proof+inventory+axiom gate and the seeded differential replay with receipt verification.',
  ].join('\n');
}

async function localize({ args, output, scratch }) {
  const plan = readJsonBuffer(readFileSync(args.plan), 'qualification plan');
  verifyReviewPlan(plan, args['approve-diff'], args.plan);
  const history = JSON.parse(readFileSync(args.history, 'utf8'));
  assert(Array.isArray(history) && history.length >= 2, 'history must be an ordered JSON array of full immutable commit SHAs');
  const revisions = history.map(sourceRevision);
  assert(revisions[0] === plan.baseline.revision && revisions.at(-1) === plan.proposal.revision,
    'history endpoints must equal the verified baseline and failed proposal');
  for (let index = 1; index < revisions.length; index += 1) {
    const ancestor = spawnSync(git, ['merge-base', '--is-ancestor', revisions[index - 1], revisions[index]], { cwd: repo });
    if (ancestor.status !== 0) throw new QualificationError('INVALID_HISTORY',
      `${revisions[index - 1]} is not an ancestor of ${revisions[index]}`);
  }
  const cache = new Map();
  const probe = async revision => {
    if (!cache.has(revision)) {
      const result = await runOneRevision({ revision, side: `probe-${revision.slice(0, 12)}`, plan,
        output: join(output, 'probes', revision), scratch, profileName: plan.profile.name });
      cache.set(revision, { outcome: result.outcome, toolchain: result.toolchain,
        details: result.failure ?? { receipts: [result.proof_receipt, result.replay_receipt] } });
    }
    return cache.get(revision);
  };
  const mode = args.mode;
  if (mode === 'bisect') return await bisectRegression(revisions, probe);
  if (mode === 'linear') return await linearScan(revisions, probe);
  throw new QualificationError('USAGE', 'localize --mode must be bisect or linear');
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  if (args.help || args.operation === 'help' || args.operation === undefined) {
    console.log(usage()); return 0;
  }
  if (args.operation === 'plan') {
    for (const name of ['baseline', 'proposal', 'profile', 'output', 'scratch']) assert(args[name], `plan requires --${name}`);
    const output = safeOutput(args.output, repo), scratch = safeScratch(args.scratch, repo);
    try {
      const plan = planQualification({ baseline: args.baseline, proposal: args.proposal,
        profileName: args.profile, output });
      writeFileSync(join(output, 'summary.txt'), `plan=${plan.plan_sha256}\nreview=${plan.review.sha256}\nstatus=${plan.qualification_status}\n`);
      console.log(JSON.stringify({ plan: join(output, 'plan.json'), patch: join(output, 'candidate.patch'),
        review_sha256: plan.review.sha256, blockers: plan.blockers, status: plan.qualification_status }, null, 2));
      return plan.blockers.length ? 2 : 0;
    } finally { if (existsSync(scratch)) rmSync(scratch, { recursive: true, force: true }); }
  }
  if (args.operation === 'run') {
    for (const name of ['plan', 'approve-diff', 'output', 'scratch']) assert(args[name], `run requires --${name}`);
    const output = safeOutput(args.output, repo), scratch = safeScratch(args.scratch, repo);
    try {
      const report = await runQualification({ planPath: args.plan, approval: args['approve-diff'], output, scratch });
      console.log(JSON.stringify({ report: join(output, 'qualification.json'), status: report.qualification_status,
        source_unchanged: report.source_preservation.unchanged }, null, 2));
      return report.qualification_status.startsWith('qualified') ? 0 : 2;
    } finally { if (existsSync(scratch)) rmSync(scratch, { recursive: true, force: true }); }
  }
  if (args.operation === 'localize') {
    for (const name of ['baseline', 'proposal', 'history', 'mode', 'plan', 'approve-diff', 'output', 'scratch']) assert(args[name], `localize requires --${name}`);
    const output = safeOutput(args.output, repo), scratch = safeScratch(args.scratch, repo);
    try {
      const result = await localize({ args, output, scratch });
      writeFileSync(join(output, 'localization.json'), `${JSON.stringify(result, null, 2)}\n`);
      console.log(JSON.stringify({ localization: join(output, 'localization.json'), ...result }, null, 2));
      return result.qualified ? 0 : 2;
    } finally { if (existsSync(scratch)) rmSync(scratch, { recursive: true, force: true }); }
  }
  throw new QualificationError('USAGE', `unknown operation '${args.operation}'`);
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main().then(code => { process.exitCode = code; }).catch(error => {
    const message = `[lean-qualify] ${error.code ?? 'ERROR'}: ${error.message}`;
    console.error(message);
    if (!(error instanceof QualificationError) && !(error instanceof Cancelled)) console.error(error.stack);
    process.exitCode = error.code === 'CANCELLED' ? 130 : 2;
  }).finally(() => process.removeListener('SIGINT', onSigint));
}

export { profileDefinitions, snapshotRevision, planQualification, runQualification, runOneRevision,
  verifyReviewPlan, localize, runProcess };
