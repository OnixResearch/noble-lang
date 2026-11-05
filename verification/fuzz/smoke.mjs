// cargo-fuzz smoke run for `crates/noble-wasm/fuzz` (#87). Builds the four libFuzzer targets with
// the pinned nightly Rust and GCC named by `policy/tool-selection.json`, seeds every target from the
// tracked `.noble`/`.wit` fixtures under the default budget and deliberately tiny ones, runs each
// for a fixed time and retains a receipt of the source, toolchain, dependency lock and per-target
// libFuzzer statistics. Dynamic evidence only: a clean run shows that these executions found no
// property violation, not that no input violates a property.
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import {
  existsSync, mkdirSync, readdirSync, readFileSync, realpathSync, writeFileSync,
} from 'node:fs';
import { dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(fileURLToPath(import.meta.url), '../../..');
const crate = join(root, 'crates/noble-wasm/fuzz');
const targets = ['prepare', 'source', 'prepare_to_wasm', 'wit_world'];
const triple = 'x86_64-unknown-linux-gnu';
const headers = [[3, 0, 0, 0], [0, 255, 63, 7], [16, 128, 32, 4], [2, 40, 10, 3]];
const multiUnit = [
  ['verification/linking/fixtures/callee.noble', 'verification/linking/fixtures/import.noble',
    'verification/linking/fixtures/caller.noble'],
  ['crates/noble-contracts/src/fixtures/result.noble', 'crates/noble-contracts/src/fixtures/calc-errors.noble'],
  ['crates/noble-cli/tests/fixtures/worker-data.noble', 'crates/noble-cli/tests/fixtures/worker-transition.noble'],
];
const rustflags = [
  '-Cpasses=sancov-module', '-Cllvm-args=-sanitizer-coverage-level=4',
  '-Cllvm-args=-sanitizer-coverage-inline-8bit-counters', '-Cllvm-args=-sanitizer-coverage-pc-table',
  '-Cllvm-args=-sanitizer-coverage-trace-compares', '--cfg', 'fuzzing', '-Cdebug-assertions',
  '-Coverflow-checks',
].join(' ');
const requireThat = (condition, message) => { if (!condition) throw Error(message); };
const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const commands = [];
let output;

function parseArguments(argv) {
  const options = { seconds: 60, allowDirty: false, output: null, targets: [] };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === '--allow-dirty') options.allowDirty = true;
    else if (argument === '--output') options.output = argv[(index += 1)];
    else if (argument === '--seconds') options.seconds = Number(argv[(index += 1)]);
    else if (argument === '--target') options.targets.push(argv[(index += 1)]);
    else throw Error(`usage: SELECTED_NODE verification/fuzz/smoke.mjs --output NEW_EXTERNAL_DIRECTORY [--seconds N] [--target NAME]... [--allow-dirty]; unknown ${argument}`);
  }
  requireThat(options.output, 'usage: --output NEW_EXTERNAL_DIRECTORY is required');
  requireThat(Number.isSafeInteger(options.seconds) && options.seconds > 0 && options.seconds <= 3600,
    '--seconds must be an integer from 1 to 3600');
  if (options.targets.length === 0) options.targets = [...targets];
  requireThat(options.targets.every(name => targets.includes(name)), `--target must be one of ${targets.join(', ')}`);
  return options;
}

function checkEnvironment(selectedNode) {
  const forbidden = Object.keys(process.env).filter(key =>
    /^(?:CARGO_|RUST|LD_|DYLD_|NIX_|NODE(?:_|$)|LIBFUZZER|ASAN_|UBSAN_|CC$|CXX$|CFLAGS|CXXFLAGS|GIT_)/.test(key)
    && process.env[key] !== '');
  requireThat(forbidden.length === 0, `unsafe caller environment: ${forbidden.sort().join(', ')}`);
  requireThat(realpathSync(process.execPath) === realpathSync(selectedNode),
    `run this script with the selected Node executable ${selectedNode}`);
  requireThat(process.execArgv.length === 0, 'unreviewed Node runtime flags are not accepted');
}

function run(label, command, argv, { cwd = root, env, allowFailure = false, timeout = 3_600_000 } = {}) {
  const started = Date.now();
  const observed = spawnSync(command, argv, {
    cwd, env, timeout, maxBuffer: 256 * 1024 * 1024, encoding: 'utf8',
  });
  const record = {
    label, command, argv, exit: observed.status, signal: observed.signal,
    milliseconds: Date.now() - started,
  };
  commands.push(record);
  const directory = join(output, 'commands', String(commands.length).padStart(3, '0'));
  mkdirSync(directory, { recursive: true });
  writeFileSync(join(directory, 'stdout'), observed.stdout ?? '');
  writeFileSync(join(directory, 'stderr'), observed.stderr ?? '');
  writeFileSync(join(directory, 'command.json'), `${JSON.stringify(record, null, 2)}\n`);
  if (!allowFailure) {
    requireThat(observed.status === 0,
      `${label} failed: ${observed.status ?? observed.signal}\n${(observed.stderr ?? '').slice(-4000)}`);
  }
  return { ...observed, record };
}

function gitState(git) {
  const capture = argv => run(`git ${argv[0]}`, git, argv, { env: { PATH: dirname(git), HOME: output } }).stdout;
  return {
    commit: capture(['rev-parse', 'HEAD']).trim(),
    tree: capture(['rev-parse', 'HEAD^{tree}']).trim(),
    status: capture(['status', '--porcelain', '--untracked-files=all']),
  };
}

function trackedFixtures(git) {
  const listed = run('git ls-files fixtures', git, ['ls-files', '-z', '--', '*.noble', '*.wit'],
    { env: { PATH: dirname(git), HOME: output } }).stdout;
  return listed.split('\0').filter(Boolean).sort();
}

function writeSeed(directory, header, select, body) {
  const data = Buffer.concat([Buffer.from([...header, select]), body]);
  const name = digest(data).slice(0, 40);
  writeFileSync(join(directory, name), data);
  return name;
}

function seedCorpora(files) {
  const seeds = {};
  for (const target of targets) {
    const directory = join(output, 'corpus', target);
    mkdirSync(directory, { recursive: true });
    const names = new Set();
    const selects = target === 'source' ? [0, 1, 2, 3] : [0];
    const subjects = files.filter(file => (target === 'wit_world') === file.endsWith('.wit'));
    for (const file of subjects) {
      const body = readFileSync(join(root, file));
      for (const header of headers) {
        for (const select of selects) names.add(writeSeed(directory, header, select, body));
      }
    }
    if (target === 'source') {
      for (const group of multiUnit) {
        const body = Buffer.concat(group.flatMap((file, index) =>
          (index === 0 ? [] : [Buffer.from([0])]).concat([readFileSync(join(root, file))])));
        for (const header of headers) {
          for (const select of [0, 2]) names.add(writeSeed(directory, header, select, body));
        }
      }
    }
    const regressions = join(crate, 'regressions', target);
    if (existsSync(regressions)) {
      for (const entry of readdirSync(regressions).sort()) {
        const data = readFileSync(join(regressions, entry));
        const name = digest(data).slice(0, 40);
        writeFileSync(join(directory, name), data);
        names.add(name);
      }
    }
    seeds[target] = { files: subjects.length, inputs: names.size, digest: digest(Buffer.from([...names].sort().join('\n'))) };
  }
  return seeds;
}

function finalStats(stderr) {
  const stats = {};
  for (const match of stderr.matchAll(/^stat::([a-z_]+):\s+(\d+)$/gm)) stats[match[1]] = Number(match[2]);
  const done = [...stderr.matchAll(/^#\d+\s+DONE\s+cov: (\d+) ft: (\d+) corp: (\d+)\//gm)].at(-1);
  if (done) {
    stats.coverage_edges = Number(done[1]);
    stats.features = Number(done[2]);
    stats.corpus_units = Number(done[3]);
  }
  return stats;
}

function main() {
  const options = parseArguments(process.argv.slice(2));
  const selection = JSON.parse(readFileSync(join(root, 'policy/tool-selection.json'), 'utf8'));
  const selectedNode = join(selection.tool_paths.node.output, 'bin/node');
  checkEnvironment(selectedNode);
  output = resolve(options.output);
  requireThat(relative(root, output).startsWith('..'), 'output directory must be outside the source tree');
  requireThat(!existsSync(output), 'output directory must not exist yet');
  mkdirSync(output, { recursive: true });
  const rustBin = join(selection.tool_paths.quality_rust.output, 'bin');
  const cargo = join(rustBin, 'cargo');
  const rustc = join(rustBin, 'rustc');
  const gccBin = selection.component_sync.linker_bin;
  const git = realpathSync('/run/current-system/sw/bin/git');

  const initial = gitState(git);
  requireThat(initial.status === '' || options.allowDirty,
    'source worktree is dirty; re-run with --allow-dirty to bind the current source snapshot');
  const lock = readFileSync(join(crate, 'Cargo.lock'), 'utf8');
  const libfuzzer = lock.match(/name = "libfuzzer-sys"\nversion = "([^"]+)"\nsource = "[^"]+"\nchecksum = "([0-9a-f]+)"/);
  requireThat(libfuzzer, 'fuzz Cargo.lock does not pin a registry libfuzzer-sys with a checksum');

  const home = join(output, 'home');
  mkdirSync(home);
  const buildEnv = {
    HOME: home, USER: 'noble-fuzz', LOGNAME: 'noble-fuzz', LANG: 'C.UTF-8', LC_ALL: 'C.UTF-8',
    PATH: [rustBin, gccBin, '/run/current-system/sw/bin', '/usr/bin', '/bin'].join(':'),
    TMPDIR: home, CARGO_HOME: join(output, 'cargo-home'), CARGO_TARGET_DIR: join(output, 'cargo-target'),
    CARGO_BUILD_JOBS: '8', CARGO_INCREMENTAL: '0', RUSTC: rustc, RUSTC_WRAPPER: '',
    CARGO_BUILD_RUSTC_WRAPPER: '', CC: join(gccBin, 'cc'), CXX: join(gccBin, 'c++'),
    CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER: join(gccBin, 'cc'), RUSTFLAGS: rustflags,
  };
  const rustcVersion = run('rustc --version', rustc, ['--version'], { env: buildEnv }).stdout.trim();
  run('fuzz build', cargo, ['build', '--release', '--locked', '--bins', '--target', triple],
    { cwd: crate, env: buildEnv });

  const seeds = seedCorpora(trackedFixtures(git));
  const results = [];
  for (const target of options.targets) {
    const binary = join(output, 'cargo-target', triple, 'release', target);
    const artifacts = join(output, 'artifacts', target);
    mkdirSync(artifacts, { recursive: true });
    const fuzz = run(`fuzz ${target}`, binary, [
      `-max_total_time=${options.seconds}`, '-timeout=5', '-rss_limit_mb=1024', '-print_final_stats=1',
      `-dict=${join(crate, 'noble.dict')}`, `-artifact_prefix=${artifacts}/`, join(output, 'corpus', target),
    ], { cwd: output, env: { HOME: home, PATH: '/run/current-system/sw/bin:/usr/bin:/bin', LANG: 'C.UTF-8' },
      allowFailure: true, timeout: (options.seconds + 300) * 1000 });
    const findings = readdirSync(artifacts).sort().map(name => ({
      name, sha256: digest(readFileSync(join(artifacts, name))),
    }));
    results.push({
      target, binary_sha256: digest(readFileSync(binary)), exit: fuzz.status, signal: fuzz.signal,
      seconds: options.seconds, seeds: seeds[target], stats: finalStats(fuzz.stderr ?? ''), findings,
      command: fuzz.record.label,
    });
  }
  const final = gitState(git);
  requireThat(final.commit === initial.commit && final.tree === initial.tree && final.status === initial.status,
    'source commit, tree or status changed during the run');
  const passed = results.every(row => row.exit === 0 && row.findings.length === 0);
  const receipt = {
    schema: 'noble-fuzz-smoke/v1', result: passed ? 'passed' : 'failed',
    source_commit: initial.commit, source_tree: initial.tree, clean_source: initial.status === '',
    allow_dirty: options.allowDirty,
    toolchain: {
      rustc: { path: realpathSync(rustc), version: rustcVersion },
      cargo: { path: realpathSync(cargo) },
      cxx: { path: realpathSync(join(gccBin, 'c++')) },
      node: { path: realpathSync(process.execPath), version: process.version },
    },
    dependencies: {
      lock_sha256: digest(Buffer.from(lock)),
      libfuzzer_sys: { version: libfuzzer[1], checksum: libfuzzer[2] },
    },
    rustflags,
    libfuzzer_options: ['-timeout=5', '-rss_limit_mb=1024', '-dict=crates/noble-wasm/fuzz/noble.dict'],
    oracle_sha256: digest(readFileSync(join(root, 'crates/noble-wasm/tests/fuzz/oracles.rs'))),
    targets: results,
    commands: commands.map(({ label, exit, milliseconds }) => ({ label, exit, milliseconds })),
    claims: {
      kind: 'dynamic evidence: coverage-guided fuzzing for a fixed time',
      properties: ['no panic', 'no Internal frontend diagnostic', 'no Defective or Invalid backend outcome for a prepared contract',
        'deterministic outcomes', 'non-exhausted outcomes reproduced under the default budget',
        'emitted WAT parses and the module validates'],
      non_claims: ['A clean run is not a proof that no input violates a property.',
        'The oracles check outcome consistency, not that any accepted program is correct.',
        'Peak RSS is libFuzzer getrusage, which a launching process can inflate.'],
    },
  };
  writeFileSync(join(output, 'receipt.json'), `${JSON.stringify(receipt, null, 2)}\n`);
  process.stdout.write(`${JSON.stringify({
    result: receipt.result, receipt: join(output, 'receipt.json'),
    targets: results.map(row => ({ target: row.target, exit: row.exit, runs: row.stats.number_of_executed_units,
      findings: row.findings.length })),
  })}\n`);
  process.exitCode = passed ? 0 : 1;
}

try {
  main();
} catch (error) {
  console.error(`[fuzz-smoke] FAIL ${error.message}`);
  process.exitCode = 1;
}
