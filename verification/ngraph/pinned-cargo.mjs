// The pinned toolchain and archived-source plumbing shared by the ngraph receipt runners.
//
// Every runner in this directory that produces a source-bound receipt must build the *committed*
// source with the *policy-selected* Rust toolchain against the reviewed offline vendor, not with
// whatever `cargo` is on PATH. This module holds that one copy of the setup so the runners cannot
// drift into different build routes:
//
//   const tools = pinnedCargo({ root, evidence: output });
//   archiveWorkspace({ root, commit, destination: path.join(output, 'source') });
//   const run = runCargo({ tools, source, args, target: path.join(output, 'target') });
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { selectedVendor } from '../selected-vendor.mjs';

/// The system git and tar the runners use, resolved to store paths so PATH cannot substitute them.
export function systemBinary(name) {
  const file = fs.realpathSync(`/run/current-system/sw/bin/${name}`);
  return file;
}

/// One git invocation against `root` with no user or system configuration.
export function gitText(root, argv, { encoding = 'utf8' } = {}) {
  const git = systemBinary('git');
  const result = spawnSync(git, argv, {
    cwd: root, encoding, maxBuffer: 1024 * 1024 * 1024,
    env: { PATH: path.dirname(git), HOME: '/nonexistent', GIT_CONFIG_NOSYSTEM: '1', GIT_CONFIG_GLOBAL: '/dev/null' },
  });
  if (result.status !== 0) throw new Error(`git ${argv.join(' ')} failed: ${result.stderr}`);
  return result.stdout;
}

/// Extracts `commit`'s Cargo workspace (manifest, lockfile, crates) into `destination`.
export function archiveWorkspace({ root, commit, destination, members = ['Cargo.toml', 'Cargo.lock', 'crates'] }) {
  fs.mkdirSync(destination, { recursive: true });
  const file = `${destination}.tar`;
  gitText(root, ['archive', '--format=tar', `--output=${file}`, commit, ...members]);
  const result = spawnSync(systemBinary('tar'), ['-xf', file, '-C', destination], { encoding: 'utf8' });
  if (result.status !== 0) throw new Error(`tar failed: ${result.stderr}`);
  fs.rmSync(file);
}

/// The policy-selected cargo, its environment, and the reviewed vendor it resolves through.
///
/// `evidence` must be outside the source tree: the cargo home, the private HOME and TMPDIR all live
/// under it so nothing in the repository is written by the build.
export function pinnedCargo({ root, evidence, jobs = 4 }) {
  const selection = JSON.parse(fs.readFileSync(path.join(root, 'policy/tool-selection.json'), 'utf8'));
  const rustBin = path.join(selection.tool_paths.quality_rust.output, 'bin');
  const linkerBin = selection.component_sync.linker_bin;
  const vendor = selectedVendor(selection.component_sync);
  const cargoHome = path.join(evidence, 'cargo-home');
  const home = path.join(evidence, 'home');
  fs.mkdirSync(cargoHome, { recursive: true });
  fs.mkdirSync(home, { recursive: true });
  fs.writeFileSync(path.join(cargoHome, 'config.toml'),
    `[source.crates-io]\nreplace-with = "reviewed-vendor"\n[source.reviewed-vendor]\ndirectory = ${JSON.stringify(vendor.directory)}\n[net]\noffline = true\n`,
    { flag: 'wx' });
  const environment = {
    HOME: home, USER: 'noble-ngraph-receipt', LOGNAME: 'noble-ngraph-receipt', LANG: 'C.UTF-8', LC_ALL: 'C.UTF-8',
    CI: '1', PATH: [rustBin, linkerBin, '/run/current-system/sw/bin', '/usr/bin', '/bin'].join(':'), TMPDIR: home,
    CARGO_HOME: cargoHome, CARGO_NET_OFFLINE: 'true', CARGO_BUILD_JOBS: String(jobs), CARGO_INCREMENTAL: '0',
    RUSTC: path.join(rustBin, 'rustc'), RUSTDOC: path.join(rustBin, 'rustdoc'), RUSTC_WRAPPER: '',
    RUSTC_WORKSPACE_WRAPPER: '', CARGO_BUILD_RUSTC_WRAPPER: '', RUSTFLAGS: '', CARGO_ENCODED_RUSTFLAGS: '',
    CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS: '', CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER: path.join(linkerBin, 'cc'),
    CARGO_TERM_COLOR: 'never',
  };
  const version = spawnSync(path.join(rustBin, 'rustc'), ['--version'], { env: environment, encoding: 'utf8' }).stdout.trim();
  return { cargo: path.join(rustBin, 'cargo'), rustBin, environment, vendor, version };
}

/// Runs cargo in `source` with the target directory and environment `tools` pins.
export function runCargo({ tools, source, args, target, timeout = 3_600_000, environment = {} }) {
  const result = spawnSync(tools.cargo, args, {
    cwd: source, env: { ...tools.environment, ...environment, CARGO_TARGET_DIR: target }, encoding: 'utf8',
    timeout, maxBuffer: 256 * 1024 * 1024,
  });
  return {
    status: result.status ?? (result.signal ? -1 : 0),
    signal: result.signal ?? null,
    timedOut: result.error?.code === 'ETIMEDOUT',
    stdout: result.stdout ?? '',
    stderr: result.stderr ?? '',
  };
}
