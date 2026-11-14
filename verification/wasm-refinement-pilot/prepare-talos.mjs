// Fetch the pinned Talos commit and prepare a disposable, isolated interpreter project.
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const lane = path.join(root, 'verification/wasm-refinement-pilot');
const pin = 'd7c84ac660757953b752bdfc9cb8dae3dab623ed';
const upstreamUrl = 'https://github.com/cajal-technologies/talos.git';
const patchNames = [
  'talos-d7c84ac-decoder.patch',
  'talos-d7c84ac-memory-init.patch',
  'talos-d7c84ac-lakefile.patch',
];
const git = fs.realpathSync('/run/current-system/sw/bin/git');
// Keep the applet symlink name: uutils coreutils dispatches on argv[0].
const cp = '/run/current-system/sw/bin/cp';
const sha256 = bytes => createHash('sha256').update(bytes).digest('hex');
const fail = message => { throw new Error(message); };
const requireThat = (condition, message) => { if (!condition) fail(message); };

function run(command, args, cwd, env) {
  const result = spawnSync(command, args, {
    cwd,
    env,
    encoding: 'utf8',
    maxBuffer: 16 * 1024 * 1024,
  });
  requireThat(result.error === undefined, `${path.basename(command)} ${args[0] ?? ''}: ${result.error}`);
  requireThat(result.status === 0,
    `${path.basename(command)} ${args[0] ?? ''} failed (${result.status}): ${result.stderr}`);
  return result.stdout.trim();
}

function copyReflink(source, destination, cwd, env) {
  run(cp, ['-a', '--reflink=always', source, destination], cwd, env);
}

function shellQuote(value) {
  return `'${value.replaceAll("'", "'\\''")}'`;
}

function parseArgs() {
  const args = process.argv.slice(2);
  requireThat(args.length === 2 && args[0] === '--work-dir' && args[1] !== '',
    'usage: prepare-talos.mjs --work-dir ABSOLUTE_NEW_DIR_OUTSIDE_REPOSITORY');
  requireThat(path.isAbsolute(args[1]), '--work-dir must be absolute');
  return path.resolve(args[1]);
}

function main() {
  requireThat(Object.keys(process.env).length === 0, 'run with env -i; ambient environment is refused');
  const selection = JSON.parse(fs.readFileSync(path.join(root, 'policy/tool-selection.json'), 'utf8'));
  const node = path.join(selection.tool_paths.node.output, 'bin/node');
  requireThat(fs.realpathSync(process.execPath) === fs.realpathSync(node),
    'process is not the policy-selected Node executable');

  const workDir = parseArgs();
  requireThat(workDir !== root && !workDir.startsWith(`${root}${path.sep}`),
    '--work-dir must be outside the repository');
  requireThat(!fs.existsSync(workDir), '--work-dir must be a new directory');
  fs.mkdirSync(workDir, { recursive: false });
  const env = {
    HOME: workDir,
    TMPDIR: workDir,
    LANG: 'C.UTF-8',
    PATH: '/run/current-system/sw/bin:/usr/bin:/bin',
    GIT_TERMINAL_PROMPT: '0',
  };

  const sourceRepo = path.join(workDir, 'talos-source');
  fs.mkdirSync(sourceRepo);
  run(git, ['init', '--quiet'], sourceRepo, env);
  run(git, ['remote', 'add', 'origin', upstreamUrl], sourceRepo, env);
  run(git, ['fetch', '--depth=1', 'origin', pin], sourceRepo, env);
  run(git, ['checkout', '--quiet', '--detach', 'FETCH_HEAD'], sourceRepo, env);
  const upstreamCommit = run(git, ['rev-parse', 'HEAD'], sourceRepo, env);
  requireThat(upstreamCommit === pin, `Talos commit ${upstreamCommit} differs from required ${pin}`);
  const upstreamTree = run(git, ['rev-parse', 'HEAD^{tree}'], sourceRepo, env);
  requireThat(/^[0-9a-f]{40}$/.test(upstreamTree), `invalid Talos tree hash ${upstreamTree}`);

  const sourceInterpreter = path.join(sourceRepo, 'interpreter');
  const project = path.join(workDir, 'interpreter');
  requireThat(fs.existsSync(sourceInterpreter), 'pinned Talos checkout has no interpreter directory');
  run(cp, ['-a', sourceInterpreter, project], workDir, env);

  // The destination is a scratch-only Git work tree so `git apply` can check
  // each path after the upstream interpreter has been copied out of the clone.
  run(git, ['init', '--quiet'], workDir, env);
  const patchDir = path.join(lane, 'lean/patches');
  const patches = [];
  for (const name of patchNames) {
    const patch = path.join(patchDir, name);
    const digest = sha256(fs.readFileSync(patch));
    run(git, ['apply', '--check', '-p2', '--directory=interpreter', patch], workDir, env);
    run(git, ['apply', '-p2', '--directory=interpreter', patch], workDir, env);
    patches.push({ path: `verification/wasm-refinement-pilot/lean/patches/${name}`, sha256: digest });
  }

  const overlayRoot = path.join(lane, 'lean/overlay');
  const overlays = [
    {
      source: path.join(overlayRoot, 'Interpreter/WasmPilot/Check.lean'),
      destination: path.join(project, 'Interpreter/WasmPilot/Check.lean'),
      path: 'verification/wasm-refinement-pilot/lean/overlay/Interpreter/WasmPilot/Check.lean',
    },
    {
      source: path.join(overlayRoot, 'Interpreter/WasmPilot/Runner.lean'),
      destination: path.join(project, 'Interpreter/WasmPilot/Runner.lean'),
      path: 'verification/wasm-refinement-pilot/lean/overlay/Interpreter/WasmPilot/Runner.lean',
    },
    {
      source: path.join(root, 'proofs/mc1/NobleContracts/Model.lean'),
      destination: path.join(project, 'NobleContracts/Model.lean'),
      path: 'proofs/mc1/NobleContracts/Model.lean',
    },
  ];
  const overlayRows = [];
  for (const item of overlays) {
    fs.mkdirSync(path.dirname(item.destination), { recursive: true });
    fs.copyFileSync(item.source, item.destination, fs.constants.COPYFILE_EXCL);
    overlayRows.push({ path: item.path, sha256: sha256(fs.readFileSync(item.source)) });
  }

  const fixtureSource = path.join(lane, 'lean/interpreter/vendor/testsuite');
  const fixtureDestination = path.join(project, 'vendor/testsuite');
  fs.mkdirSync(path.dirname(fixtureDestination), { recursive: true });
  run(cp, ['-a', fixtureSource, fixtureDestination], workDir, env);

  const talosManifestPath = path.join(project, 'lake-manifest.json');
  const foundationManifestPath = path.join(root, 'proofs/foundation/lake-manifest.json');
  const talosManifest = JSON.parse(fs.readFileSync(talosManifestPath, 'utf8'));
  const foundationManifest = JSON.parse(fs.readFileSync(foundationManifestPath, 'utf8'));
  const foundationPackagesByName = new Map(foundationManifest.packages.map(item => [item.name, item]));
  const foundationPackages = path.join(root, 'proofs/foundation/.lake/packages');
  const targetPackages = path.join(workDir, '.lake/packages');
  fs.mkdirSync(targetPackages, { recursive: true });
  const packageRows = [];
  for (const item of talosManifest.packages) {
    const foundationItem = foundationPackagesByName.get(item.name);
    requireThat(foundationItem?.rev === item.rev,
      `${item.name} revision mismatch: Talos=${item.rev} foundation=${foundationItem?.rev ?? '(missing)'}`);
    const source = path.join(foundationPackages, item.name);
    const destination = path.join(targetPackages, item.name);
    requireThat(fs.existsSync(source), `prebuilt foundation package ${item.name} is missing`);
    const cachedRevision = run(git, ['-C', source, 'rev-parse', 'HEAD'], root, env);
    requireThat(cachedRevision === item.rev,
      `${item.name} cache revision ${cachedRevision} differs from Talos pin ${item.rev}`);
    copyReflink(source, destination, workDir, env);
    packageRows.push({ name: item.name, rev: item.rev, reflink_copied: true });
  }

  const talosToolchain = fs.readFileSync(path.join(project, 'lean-toolchain'), 'utf8').trim();
  const foundationToolchain = fs.readFileSync(path.join(root, 'proofs/foundation/lean-toolchain'), 'utf8').trim();
  requireThat(talosToolchain === foundationToolchain && /(?:^|:)v4\.35\.0$/.test(talosToolchain),
    `Lean toolchain mismatch: Talos=${talosToolchain} foundation=${foundationToolchain}`);

  const leanBin = path.join(selection.tool_paths.lean.output, 'bin');
  const lake = path.join(leanBin, 'lake');
  const lean = path.join(leanBin, 'lean');
  const runnerPath = path.join(workDir, 'pilot-runner');
  const runnerEnvPath = `${leanBin}:/run/current-system/sw/bin:/usr/bin:/bin`;
  const runnerWrapper = [
    '#!/run/current-system/sw/bin/bash',
    'set -euo pipefail',
    `cd ${shellQuote(project)}`,
    `export HOME=${shellQuote(workDir)}`,
    `export TMPDIR=${shellQuote(workDir)}`,
    'export LANG=C.UTF-8',
    `export PATH=${shellQuote(runnerEnvPath)}`,
    `exec ${shellQuote(lake)} env env PATH=${shellQuote(runnerEnvPath)} ${shellQuote(lean)} --run Interpreter/WasmPilot/Runner.lean "$@"`,
    '',
  ].join('\n');
  fs.writeFileSync(runnerPath, runnerWrapper, { flag: 'wx' });
  fs.chmodSync(runnerPath, 0o755);

  const sourceCommit = run(git, ['rev-parse', 'HEAD'], root, env);
  const sourceStatus = run(git, ['status', '--porcelain'], root, env);
  requireThat(sourceStatus === '', `receipts require a clean committed source tree: ${JSON.stringify(sourceStatus)}`);
  const receipt = {
    schema: 'wasm-pilot-talos-preparation/v1',
    issue: 82,
    source_commit: sourceCommit,
    source_worktree_clean: true,
    upstream_repository: upstreamUrl,
    upstream_commit: upstreamCommit,
    upstream_tree: upstreamTree,
    project: path.join(workDir, 'interpreter'),
    runner_wrapper: runnerPath,
    runner_wrapper_sha256: sha256(fs.readFileSync(runnerPath)),
    lean_toolchain: talosToolchain,
    patches,
    overlays: overlayRows,
    spec_fixture_manifest_sha256: sha256(fs.readFileSync(path.join(fixtureSource, 'SOURCE.json'))),
    packages: packageRows,
    package_source: 'proofs/foundation/.lake/packages',
    package_copy_method: 'cp -a --reflink=always; no package fetch or Mathlib rebuild',
  };
  fs.writeFileSync(path.join(workDir, 'talos-source.json'), `${JSON.stringify(receipt, null, 2)}\n`, { flag: 'wx' });
  process.stdout.write(`[wasm-pilot-prepare] PASS commit=${upstreamCommit} tree=${upstreamTree} packages=${packageRows.length}\n`);
}

try { main(); }
catch (error) {
  process.stderr.write(`[wasm-pilot-prepare] FAIL ${error.message}\n`);
  process.exitCode = 1;
}
