import assert from 'node:assert/strict';
import { execFileSync, spawnSync } from 'node:child_process';
import { after, test } from 'node:test';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs';

import { dirname, join, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { bisectRegression, classifyChanges, compareDirectToManifest, compareManifests,
  dependencyToolchainMismatches, directDependencies, linearScan } from './core.mjs';
import { requiredDifferentialControlIds, verifyRequiredDifferentialControls }
  from '../foundation/control-ids.mjs';
import { buildCacheInventory, compareBuildCaches, planQualification, profileDefinitions,
  replayEnvironment, requestCancellation, runProcess, seedProjectCaches } from './qualify.mjs';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const policy = JSON.parse(readFileSync(join(root, 'policy/tool-selection.json'), 'utf8'));
const leanBin = join(policy.tool_paths.lean.output, 'bin');
const scratchBase = process.env.NOBLE_QUALIFY_TEST_TMP;
assert.ok(scratchBase, 'set NOBLE_QUALIFY_TEST_TMP to a new private directory outside the repository');
assert.ok(existsSync(scratchBase), `test scratch directory must already exist: ${scratchBase}`);
assert.equal(statSync(scratchBase).mode & 0o077, 0, 'test scratch directory must be private (0700 or stricter)');
const testRoot = mkdtempSync(join(scratchBase, 'qualification-tests-'));
after(() => rmSync(testRoot, { recursive: true, force: true }));

const git = process.env.GIT ?? '/run/current-system/sw/bin/git';
function gitRun(cwd, ...args) {
  return execFileSync(git, args, { cwd, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] }).trim();
}
function initRepo(path) {
  mkdirSync(path, { recursive: true });
  gitRun(path, 'init', '-q', '--initial-branch=main');
  gitRun(path, 'config', 'user.name', 'qualification-test');
  gitRun(path, 'config', 'user.email', 'qualification-test@example.invalid');
}
function commit(path, message) {
  gitRun(path, 'add', '-A');
  gitRun(path, 'commit', '-qm', message);
  return gitRun(path, 'rev-parse', 'HEAD');
}
function write(path, relative, text) {
  const target = join(path, relative);
  mkdirSync(dirname(target), { recursive: true });
  writeFileSync(target, text);
}
function localState(path) {
  return { head: gitRun(path, 'rev-parse', 'HEAD'), tree: gitRun(path, 'rev-parse', 'HEAD^{tree}'),
    status: gitRun(path, 'status', '--porcelain=v1', '--untracked-files=all') };
}
const lakeEnv = {
  HOME: testRoot, USER: process.env.USER ?? 'qualification-test', LANG: 'C.UTF-8',
  PATH: [leanBin, '/run/current-system/sw/bin', '/usr/bin', '/bin'].join(':'),
  GIT_TERMINAL_PROMPT: '0',
};

// A deterministic sequence of immutable Git revisions supplies the bisection and non-monotonic controls.
const history = join(testRoot, 'regression-history');
initRepo(history);
const historyRevisions = [];
for (const [index, outcome] of ['pass', 'pass', 'pass', 'fail', 'pass', 'fail', 'fail'].entries()) {
  write(history, 'probe.json', JSON.stringify({ index, outcome, toolchain: 'leanprover/lean4:v4.35.0' }) + '\n');
  historyRevisions.push(commit(history, `probe-${index}-${outcome}`));
}
const historyOutcome = revision => JSON.parse(execFileSync(git, ['show', `${revision}:probe.json`], { cwd: history, encoding: 'utf8' }));

test('Lakefile dependency plan is data-only and records manifest omissions', () => {
  const lakefile = [
    'name = "root"',
    '[[require]]',
    'name = "tiny"',
    'git = { url = "file:///fixture/tiny", rev = "1111111111111111111111111111111111111111" }',
    '',
  ].join('\n');
  const direct = directDependencies(lakefile, 'toml');
  const comparison = compareDirectToManifest(direct, { packages: [] });
  assert.equal(direct.evaluator, 'toml-data-parser');
  assert.equal(direct.entries[0].url, 'file:///fixture/tiny');
  assert.equal(direct.entries[0].rev, '1111111111111111111111111111111111111111');
  assert.equal(direct.complete, true);
  assert.deepEqual(comparison.discrepancies.map(item => item.kind), ['lakefile-dependency-not-in-manifest']);
  assert.equal(comparison.discrepancies[0].name, 'tiny');
  assert.equal(directDependencies('name = "root"\n[[require]]\nname = "unresolved"\n', 'toml').complete, false);
});

test('exact manifest comparison names newly resolved upstream dependencies', () => {
  const predicted = { packages: [{ name: 'tiny', url: 'file:///tiny', rev: '1', type: 'git' }] };
  const actual = { packages: [...predicted.packages,
    { name: 'hidden', url: 'file:///hidden', rev: '2', type: 'git' }] };
  const comparison = compareManifests(predicted, actual);
  assert.equal(comparison.equal, false);
  assert.ok(comparison.discrepancies.some(item => item.kind === 'unpredicted-dependency' && item.name === 'hidden'));
});

test('no-op planning retains empty source changes and complete consumer pins', () => {
  const revision = gitRun(root, 'rev-parse', 'b7af712703ba968a9449772ac87b36d28039d636');
  const output = join(testRoot, 'no-op-plan');
  mkdirSync(output);
  const plan = planQualification({ baseline: revision, proposal: revision,
    profileName: 'core-bootstrap', output });
  assert.equal(plan.no_op, true);
  assert.deepEqual(plan.source_diff.changed_paths, []);
  assert.deepEqual(plan.source_diff.classes, { proposed_pin_changes: [], proof_only_repairs: [],
    semantic_changes: [], verifier_policy_changes: [] });
  assert.equal(plan.profile.name, 'core-bootstrap');
  assert.ok(plan.profile.projects.includes('proofs/m3'));
  assert.ok(plan.profile.projects.includes('proofs/foundation'));
});

test('incompatible Lean/library pairs are identified before any build', () => {
  const mismatches = dependencyToolchainMismatches({
    projects: { 'proofs/m3': { toolchain: 'leanprover/lean4:v4.35.0' } },
    package_sources: [{ project: 'proofs/m3', package: 'mathlib', toolchain: 'leanprover/lean4:v4.36.0-rc2' }],
  }, 'proposal');
  assert.deepEqual(mismatches, [{ side: 'proposal', project: 'proofs/m3', package: 'mathlib',
    consumer_toolchain: 'leanprover/lean4:v4.35.0', dependency_toolchain: 'leanprover/lean4:v4.36.0-rc2',
    kind: 'lean-library-toolchain-mismatch' }]);
});

test('Lake update discovers a dependency added by an upstream package outside the pinned manifest', () => {
  const fixture = join(testRoot, 'lake-resolution');
  const hidden = join(fixture, 'hidden');
  const tiny = join(fixture, 'tiny');
  const consumer = join(fixture, 'consumer');
  for (const path of [hidden, tiny, consumer]) initRepo(path);

  write(hidden, 'lakefile.toml', 'name = "hidden"\n[[lean_lib]]\nname = "Hidden"\n');
  const hiddenRevision = commit(hidden, 'hidden baseline');
  const hiddenUrl = pathToFileURL(hidden).href;
  write(tiny, 'lakefile.toml', 'name = "tiny"\n[[lean_lib]]\nname = "Tiny"\n');
  write(tiny, 'Tiny.lean', 'def tinyValue : Nat := 1\n');
  const tinyOld = commit(tiny, 'tiny baseline');
  const tinyUrl = pathToFileURL(tiny).href;
  write(consumer, 'lean-toolchain', 'leanprover/lean4:v4.35.0\n');
  write(consumer, '.gitignore', '/.lake/\n');
  write(consumer, 'Fixture.lean', 'import Tiny\nimport Fixture.Independent\ndef fixtureValue : Nat := tinyValue + independentValue\n');
  write(consumer, 'Fixture/Independent.lean', 'def independentValue : Nat := 3\n');
  const writeConsumerLakefile = revision => write(consumer, 'lakefile.toml', [
    'name = "qualification-consumer"',
    'defaultTargets = ["Fixture"]',
    '[[require]]',
    'name = "tiny"',
    `git = { url = "${tinyUrl}" }`,
    `rev = "${revision}"`,
    '[[lean_lib]]',
    'name = "Fixture"',
    '',
  ].join('\n'));
  writeConsumerLakefile(tinyOld);
  commit(consumer, 'consumer pins old tiny');
  const lake = join(leanBin, 'lake');
  const first = spawnSync(lake, ['update'], { cwd: consumer, env: lakeEnv, encoding: 'utf8' });
  assert.equal(first.status, 0, `initial synthetic Lake resolution failed: ${first.stderr}`);
  const predicted = JSON.parse(readFileSync(join(consumer, 'lake-manifest.json'), 'utf8'));
  const initialBuild = spawnSync(lake, ['build', 'Fixture'], { cwd: consumer, env: lakeEnv, encoding: 'utf8' });
  assert.equal(initialBuild.status, 0, `initial synthetic Lean build failed: ${initialBuild.stderr}`);
  const consumerOutputs = join(consumer, '.lake/build/lib/lean');
  const tinyOutput = join(consumer, '.lake/packages/tiny/.lake/build/lib/lean/Tiny.olean');
  const tinyBefore = readFileSync(tinyOutput);
  const fixtureBefore = readFileSync(join(consumerOutputs, 'Fixture.olean'));
  const independentOutput = join(consumerOutputs, 'Fixture/Independent.olean');
  const independentBefore = readFileSync(independentOutput);
  const independentMtime = statSync(independentOutput).mtimeMs;

  write(tiny, 'lakefile.toml', [
    'name = "tiny"',
    '[[require]]',
    'name = "hidden"',
    `git = { url = "${hiddenUrl}" }`,
    `rev = "${hiddenRevision}"`,
    '[[lean_lib]]',
    'name = "Tiny"',
    '',
  ].join('\n'));
  write(tiny, 'Tiny.lean', 'def tinyValue : Nat := 2\n');
  const tinyNew = commit(tiny, 'add hidden dependency outside old manifest');
  writeConsumerLakefile(tinyNew);
  const second = spawnSync(lake, ['update'], { cwd: consumer, env: lakeEnv, encoding: 'utf8' });
  assert.equal(second.status, 0, `changed synthetic Lake resolution failed: ${second.stderr}`);
  const actual = JSON.parse(readFileSync(join(consumer, 'lake-manifest.json'), 'utf8'));
  const comparison = compareManifests(predicted, actual);
  assert.equal(comparison.equal, false);
  assert.ok(comparison.discrepancies.some(item => item.kind === 'unpredicted-dependency' && item.name === 'hidden'));
  const build = spawnSync(lake, ['build', 'Fixture'], { cwd: consumer, env: lakeEnv, encoding: 'utf8' });
  assert.equal(build.status, 0, `compatible synthetic Lean package update failed to build: ${build.stderr}`);
  assert.notDeepEqual(readFileSync(tinyOutput), tinyBefore, 'changed dependency module was not rebuilt');
  assert.notDeepEqual(readFileSync(join(consumerOutputs, 'Fixture.olean')), fixtureBefore,
    'dependent consumer module was not rebuilt');
  assert.deepEqual(readFileSync(independentOutput), independentBefore,
    'unrelated consumer module output changed during the compatible update');
  assert.equal(statSync(independentOutput).mtimeMs, independentMtime,
    'Lake rebuilt the unrelated consumer module');
  const status = localState(consumer).status;
  assert.match(status, /lakefile\.toml/);
  assert.match(status, /lake-manifest\.json/);
  assert.doesNotMatch(status, /Fixture\.lean|\.lake\//);
});

test('native bisection verifies a good baseline and localizes the seeded mid-range regression', async () => {
  const probed = [];
  const result = await bisectRegression(historyRevisions.slice(0, 4), async revision => {
    probed.push(revision);
    const source = historyOutcome(revision);
    return { outcome: source.outcome, toolchain: source.toolchain, details: { commit: revision } };
  });
  assert.equal(result.qualified, true);
  assert.deepEqual(result.boundary, { last_good: historyRevisions[2], first_bad: historyRevisions[3] });
  assert.equal(probed[0], historyRevisions[0]);
  assert.ok(result.probes.every(row => row.toolchain === 'leanprover/lean4:v4.35.0'));
  assert.deepEqual(new Set(probed), new Set(result.probes.map(row => row.revision)));
});

test('fail-then-pass sequence exposes bisection limits and linear scan certifies every revision', async () => {
  const outcomes = new Map(historyRevisions.map(revision => [revision, historyOutcome(revision).outcome]));
  const bisection = await bisectRegression(historyRevisions, async revision => ({
    outcome: outcomes.get(revision), toolchain: 'leanprover/lean4:v4.35.0' }));
  assert.equal(bisection.qualified, true);
  assert.ok(!bisection.probes.some(row => row.revision === historyRevisions[4]));
  const linear = await linearScan(historyRevisions, async revision => ({ outcome: outcomes.get(revision),
    toolchain: 'leanprover/lean4:v4.35.0' }));
  assert.equal(linear.qualified, true);
  assert.deepEqual(linear.probes.map(row => row.revision), historyRevisions);
  assert.ok(linear.transitions.some(row => row.from === 'fail' && row.to === 'pass'));
});

test('candidate change classification keeps pins, semantics, and verifier policy separate', () => {
  const classes = classifyChanges([
    'proofs/inets/UPSTREAM.json',
    'proofs/foundation/NobleFoundation/Arithmetic.lean',
    'proofs/foundation/inventory.json',
    'policy/tool-selection.json',
    'verification/lean-qualify/qualify.mjs',
  ]);
  assert.deepEqual(classes.proposed_pin_changes, ['proofs/inets/UPSTREAM.json']);
  assert.deepEqual(classes.semantic_changes, ['proofs/foundation/NobleFoundation/Arithmetic.lean']);
  assert.deepEqual(classes.verifier_policy_changes, [
    'policy/tool-selection.json', 'proofs/foundation/inventory.json', 'verification/lean-qualify/qualify.mjs']);
  assert.deepEqual(classes.proof_only_repairs, []);
});

test('exporter profile refuses qualification without an explicitly pinned required checker', () => {
  assert.match(profileDefinitions['mc1-export'].unavailable,
    /Bun.*no tool-selection pin.*cannot be treated as an optional pass/);
  const revision = gitRun(root, 'rev-parse', 'b7af712703ba968a9449772ac87b36d28039d636');
  const output = join(testRoot, 'unavailable-export-plan');
  mkdirSync(output);
  const plan = planQualification({ baseline: revision, proposal: revision,
    profileName: 'mc1-export', output });
  assert.equal(plan.qualification_status, 'unqualified');
  assert.ok(plan.blockers.some(item => item.code === 'REQUIRED_CHECKER_UNAVAILABLE' && /Bun/.test(item.message)));
});

test('core profile keeps all existing proof, inventory, axiom, and replay checks mandatory', () => {
  assert.deepEqual(profileDefinitions['core-bootstrap'].requiredCheckers,
    ['verification/ci/lean-proofs.sh', 'verification/foundation/differential.mjs']);
});

test('foundation compatibility profile does not build M3/Aeneas but retains its pin evidence', () => {
  const profile = profileDefinitions['inets-foundation'];
  assert.deepEqual(profile.projects, ['proofs/foundation']);
  assert.deepEqual(profile.companionProjects, ['proofs/m3']);
  assert.equal(profile.proofMode, 'foundation-only');
  assert.ok(!profile.projects.includes('proofs/m3'));
  assert.deepEqual(profile.requiredCheckers,
    ['verification/foundation/check.mjs', 'verification/foundation/differential.mjs']);
  assert.match(profile.explanation, /rebuilding Aeneas is forbidden/);
});

test('differential replay process receives a sanitized environment', () => {
  const replay = replayEnvironment({ HOME: '/home/test', USER: 'test', LOGNAME: 'test', LANG: 'C.UTF-8',
    CI: '1', PATH: '/pinned/bin', TMPDIR: '/scratch', NOBLE_TARGET: '/dev/shm/target',
    CARGO_TARGET_DIR: '/dev/shm/target', GIT_TERMINAL_PROMPT: '0', LEAN_ABORT_ON_PANIC: '1' });
  assert.deepEqual(replay, { HOME: '/home/test', USER: 'test', LOGNAME: 'test', LANG: 'C.UTF-8',
    CI: '1', PATH: '/pinned/bin', TMPDIR: '/scratch' });
});

test('differential gate rejects a missing required negative control', () => {
  const controls = requiredDifferentialControlIds.map(id => ({ id, demonstrably_fails: true }));
  assert.deepEqual(verifyRequiredDifferentialControls(controls).required_control_ids,
    requiredDifferentialControlIds);
  assert.throws(() => verifyRequiredDifferentialControls(controls.filter(control =>
    control.id !== 'trace-mismatch')), /missing or invalid required negative controls.*trace-mismatch/);
});

test('foundation gate self-test refuses a missing theorem root and an extra axiom', () => {
  const node = join(policy.tool_paths.node.output, 'bin/node');
  const result = spawnSync(node, [join(root, 'verification/foundation/check.mjs'), '--self-test'], {
    cwd: root, env: lakeEnv, encoding: 'utf8',
  });
  assert.equal(result.status, 0, result.stderr);
  const report = JSON.parse(result.stdout);
  assert.equal(report.self_test, true);
  for (const caseName of ['extra axiom declaration is refused',
    'missing module removes its inventoried theorem root',
    'changed theorem root does not satisfy the inventory']) {
    assert.ok(report.cases.some(item => item.case === caseName && item.refused), `missing control ${caseName}`);
  }
});

test('Lake rejects a stale compiled artifact and a missing imported module', () => {
  const fixture = join(testRoot, 'stale-lean-artifact');
  const childSource = join(fixture, 'Mini/Child.lean');
  const childArtifact = join(fixture, '.lake/build/lib/lean/Mini/Child.olean');
  write(fixture, 'lean-toolchain', 'leanprover/lean4:v4.35.0\n');
  write(fixture, 'lakefile.toml', 'name = "stale-fixture"\ndefaultTargets = ["Mini"]\n[[lean_lib]]\nname = "Mini"\n');
  write(fixture, 'Mini.lean', 'import Mini.Child\nnamespace Mini\ntheorem root : True := child\nend Mini\n');
  const child = 'namespace Mini\ntheorem child : True := trivial\nend Mini\n';
  write(fixture, 'Mini/Child.lean', child);
  const lake = join(leanBin, 'lake');
  const build = () => spawnSync(lake, ['build', 'Mini'], { cwd: fixture, env: lakeEnv, encoding: 'utf8' });
  const initial = build();
  assert.equal(initial.status, 0, `initial synthetic proof build failed: ${initial.stderr}`);
  assert.equal(existsSync(childArtifact), true, 'fixture did not produce the compiled child module');

  writeFileSync(childSource, 'namespace Mini\ntheorem child : False := by trivial\nend Mini\n');
  assert.equal(existsSync(childArtifact), true, 'invalid source mutation did not leave a stale compiled artifact');
  const stale = build();
  assert.notEqual(stale.status, 0, 'Lake accepted the stale compiled artifact after its source changed');

  writeFileSync(childSource, child);
  const restored = build();
  assert.equal(restored.status, 0, `restored synthetic proof build failed: ${restored.stderr}`);
  rmSync(childSource);
  const missing = build();
  assert.notEqual(missing.status, 0, 'Lake accepted a missing imported source module from cached outputs');
});

test('package build cache copies are private and cache changes are reported', () => {
  const sourceRoot = join(testRoot, 'private-cache-source');
  const candidateRoot = join(testRoot, 'private-cache-candidate');
  const sourceProject = join(sourceRoot, 'proofs/foundation');
  const candidateProject = join(candidateRoot, 'proofs/foundation');
  const sourcePackage = join(sourceProject, '.lake/packages/tiny');
  const sourcePackageBuild = join(sourcePackage, '.lake/build/lib/lean');
  mkdirSync(sourcePackageBuild, { recursive: true });
  initRepo(sourcePackage);
  write(sourcePackage, '.gitignore', '/.lake/\n');
  write(sourcePackage, 'lean-toolchain', 'leanprover/lean4:v4.35.0\n');
  write(sourcePackage, 'lakefile.toml', 'name = "tiny"\n[[lean_lib]]\nname = "Tiny"\n');
  write(sourcePackage, 'Tiny.lean', 'def tinyValue : Nat := 1\n');
  const packageRevision = commit(sourcePackage, 'fixture package');
  const sourceArtifact = join(sourcePackageBuild, 'Tiny.olean');
  writeFileSync(sourceArtifact, 'original-cache-bytes');

  const manifest = JSON.stringify({ packages: [{ name: 'tiny', type: 'path', dir: 'tiny',
    subDir: '.', rev: packageRevision }] }) + '\n';
  for (const path of [sourceProject, candidateProject]) {
    mkdirSync(join(path, '.lake'), { recursive: true });
    write(path, 'lean-toolchain', 'leanprover/lean4:v4.35.0\n');
    write(path, 'lake-manifest.json', manifest);
  }
  const candidateRootBuild = join(candidateProject, '.lake/build');
  mkdirSync(candidateRootBuild, { recursive: true });
  writeFileSync(join(candidateRootBuild, 'Foundation.olean'), 'foundation-cache');
  const sourceState = localState(sourcePackage);
  const heavyCachePaths = [];
  seedProjectCaches(candidateRoot, sourceRoot, 'proofs/foundation', heavyCachePaths);

  const candidatePackage = join(candidateProject, '.lake/packages/tiny');
  const candidateArtifact = join(candidatePackage, '.lake/build/lib/lean/Tiny.olean');
  assert.ok(existsSync(candidateArtifact));
  assert.notEqual(candidatePackage, sourcePackage);
  assert.equal(readFileSync(candidateArtifact, 'utf8'), 'original-cache-bytes');
  writeFileSync(candidateArtifact, 'candidate-rebuilt-bytes');
  assert.equal(readFileSync(sourceArtifact, 'utf8'), 'original-cache-bytes');
  assert.deepEqual(localState(sourcePackage), sourceState);

  const before = [{ kind: 'dependency-package', project: 'proofs/foundation', package: 'tiny',
    revision: packageRevision, cache: buildCacheInventory(join(candidatePackage, '.lake/build')) }];
  writeFileSync(join(candidatePackage, '.lake/build/lib/lean/Tiny.olean'), 'changed');
  writeFileSync(join(candidatePackage, '.lake/build/lib/lean/Added.olean'), 'new');
  const after = [{ ...before[0], cache: buildCacheInventory(join(candidatePackage, '.lake/build')) }];
  const changes = compareBuildCaches(before, after);
  assert.equal(changes[0].changed, true);
  assert.equal(changes[0].changed_files, 2);
  assert.equal(changes[0].rebuilt_bytes, Buffer.byteLength('changed') + Buffer.byteLength('new'));
  assert.equal(changes[0].before_cache_sha256 === changes[0].after_cache_sha256, false);
  const unchanged = compareBuildCaches(before, before);
  assert.equal(unchanged[0].changed, false);
  assert.equal(heavyCachePaths.length, 0);
});

test('SIGINT cancellation terminates an active child and leaves the fixture tree byte-identical', async () => {
  const before = localState(history);
  const logPath = join(testRoot, 'cancel-child.log');
  const running = runProcess(process.execPath, ['-e', 'process.stdout.write("started\\n");setInterval(()=>{},1000)'], {
    cwd: history, env: { ...process.env }, logPath,
  });
  setTimeout(requestCancellation, 100);
  await assert.rejects(running, /cancelled by SIGINT/);
  assert.equal(existsSync(logPath), true);
  assert.deepEqual(localState(history), before);
});
