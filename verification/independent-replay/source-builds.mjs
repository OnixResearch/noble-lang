#!/usr/bin/env node
// Issue #62: source-bound, separate builds of all five required external checkers.
// Run on a committed tree with a new --work directory outside the repository. The receipt and
// raw stdout/stderr/export bytes stay there until the caller deliberately retains them.
import { spawn, spawnSync } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { inspectExport, foreignAxioms } from './ndjson.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, '../..');
const args = process.argv.slice(2);
function value(flag) {
  const i = args.indexOf(flag);
  if (i < 0 || !args[i + 1] || args[i + 1].startsWith('--')) {
    throw new Error(`required argument ${flag} is missing`);
  }
  return args[i + 1];
}
const work = path.resolve(value('--work'));
const rustBin = path.resolve(value('--rust-bin'));
const ccBin = path.resolve(value('--cc-bin'));
const nixBin = path.resolve(value('--nix'));
const gitBin = path.resolve(value('--git'));
const workWithinRoot = path.relative(root, work);
if (workWithinRoot === '' || (!workWithinRoot.startsWith(`..${path.sep}`)
    && workWithinRoot !== '..' && !path.isAbsolute(workWithinRoot))) {
  throw new Error('--work must be outside the source repository');
}
const sha256 = (bytes) => crypto.createHash('sha256').update(bytes).digest('hex');
const hashFile = (file) => sha256(fs.readFileSync(file));
const sourcesPath = path.join(here, 'source-provenance.json');
const checkersPath = path.join(here, 'checkers.json');
const criteriaPath = path.join(here, 'batch9-criteria.json');
const derivationPath = path.join(here, 'ocaml4lean.nix');
const sources = JSON.parse(fs.readFileSync(sourcesPath, 'utf8'));
const pins = JSON.parse(fs.readFileSync(checkersPath, 'utf8'));
const criteria = JSON.parse(fs.readFileSync(criteriaPath, 'utf8'));
const leanBin = path.join(pins.toolchain.output, 'bin');
const required = criteria.items.source_builds.required_profiles;
if (sources.schema !== 'noble-independent-replay-source-provenance/v1'
    || pins.toolchain.lean !== criteria.selected_lean
    || sources.source_of_release_pins.revision !== pins.toolchain.githash
    || sources.source_of_release_pins.path !== 'src/CMakeLists.txt'
    || pins.export_format.version !== criteria.selected_export_format
    || JSON.stringify(sources.sources.map((entry) => entry.id)) !== JSON.stringify(required)
    || JSON.stringify(pins.checkers.map((entry) => entry.id)) !== JSON.stringify(required)
    || sources.selected_export.toolchain !== criteria.selected_lean
    || sources.selected_export.format !== criteria.selected_export_format
    || sources.source_build_admission_erratum?.prior_source_commit
      !== '3ddce2d0da2357cae300e8018474109cac6677b9'
    || sources.source_build_admission_erratum?.prior_receipt_sha256
      !== 'dd80be206796580182718a6e906eb08239ae5bf157189812669a5c021d0ea74e') {
  throw new Error('frozen criteria, source manifest and configured required checkers disagree');
}
if (fs.existsSync(work)) throw new Error(`work directory already exists: ${work}`);
fs.mkdirSync(path.join(work, 'commands'), { recursive: true, mode: 0o700 });
fs.mkdirSync(path.join(work, 'sources'), { mode: 0o700 });
const receiptPath = path.join(work, 'source-builds.json');
const receipt = {
  schema: 'noble-independent-replay-source-builds/v1',
  outcome: 'NO-GO',
  source_commit: null,
  selected_profile: { lean: criteria.selected_lean, export: criteria.selected_export_format,
    module: sources.selected_export.module, root: sources.selected_export.root },
  inputs: Object.fromEntries([
    ['criteria', criteriaPath], ['checkers', checkersPath], ['source_provenance', sourcesPath],
    ['runner', fileURLToPath(import.meta.url)], ['ocaml_derivation', derivationPath],
  ].map(([key, file]) => [key, { path: path.relative(root, file), sha256: hashFile(file) }])),
  tools: {},
  exports: null,
  commands: [],
  required_checkers: required,
  checkers: [],
  error: null,
  non_claims: [
    'A source-built binary need not be byte-identical to a release-bundled binary; both digests are recorded.',
    'Agreement with Lean or another external checker does not prove implementation correctness or equivalence.',
    'A parse failure, unsupported input, timeout, missing input, or resource error is not semantic rejection.',
  ],
};
const save = () => fs.writeFileSync(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`, { mode: 0o600 });
const commandRecord = ({ stdout_text, stderr_text, ...record }) => record;
let commandNumber = 0;
const baseEnv = {
  HOME: process.env.HOME ?? '/',
  USER: process.env.USER ?? '',
  ...(process.env.NIX_CONFIG ? { NIX_CONFIG: process.env.NIX_CONFIG } : {}),
  ...(process.env.SSL_CERT_FILE ? { SSL_CERT_FILE: process.env.SSL_CERT_FILE } : {}),
  ...(process.env.NIX_SSL_CERT_FILE ? { NIX_SSL_CERT_FILE: process.env.NIX_SSL_CERT_FILE } : {}),
  ...(process.env.GIT_SSL_CAINFO ? { GIT_SSL_CAINFO: process.env.GIT_SSL_CAINFO } : {}),
  LANG: 'C.UTF-8',
  PATH: [rustBin, ccBin, leanBin, '/run/current-system/sw/bin', '/usr/bin', '/bin'].join(':'),
  TMPDIR: work,
  RUSTC: path.join(rustBin, 'rustc'),
  CC: path.join(ccBin, 'cc'),
  CXX: path.join(ccBin, 'c++'),
  RUSTC_WRAPPER: '',
  RUSTC_WORKSPACE_WRAPPER: '',
  CARGO_BUILD_RUSTC_WRAPPER: '',
  CARGO_BUILD_JOBS: '4',
  CARGO_INCREMENTAL: '0',
  CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER: path.join(ccBin, 'cc'),
  CARGO_HOME: path.join(work, 'cargo-home'),
  LAKE_CACHE_DIR: path.join(work, 'lake-cache'),
};
receipt.build_environment = {
  LANG: baseEnv.LANG, PATH: baseEnv.PATH, TMPDIR: baseEnv.TMPDIR, RUSTC: baseEnv.RUSTC,
  CC: baseEnv.CC, CXX: baseEnv.CXX, CARGO_HOME: baseEnv.CARGO_HOME,
  CARGO_BUILD_JOBS: baseEnv.CARGO_BUILD_JOBS, CARGO_INCREMENTAL: baseEnv.CARGO_INCREMENTAL,
  LAKE_CACHE_DIR: baseEnv.LAKE_CACHE_DIR,
  nix_config_sha256: baseEnv.NIX_CONFIG ? sha256(baseEnv.NIX_CONFIG) : null,
};

// Keep raw child stdout and stderr, including the failing command. No shell evaluation or truncated
// captured buffer; timeout kills the process group, not merely its top-level cargo/lake process.
async function run(label, bin, argv, cwd = work, timeoutMs = 300_000, extraEnv = {}) {
  const prefix = `${String(++commandNumber).padStart(3, '0')}-${label.replace(/[^a-z0-9-]/giu, '-')}`;
  const outPath = path.join(work, 'commands', `${prefix}.stdout`);
  const errPath = path.join(work, 'commands', `${prefix}.stderr`);
  const out = fs.createWriteStream(outPath, { flags: 'wx', mode: 0o600 });
  const err = fs.createWriteStream(errPath, { flags: 'wx', mode: 0o600 });
  const start = process.hrtime.bigint();
  let result;
  try {
    result = await new Promise((resolve) => {
      const child = spawn(bin, argv, { cwd, env: { ...baseEnv, ...extraEnv },
        stdio: ['ignore', 'pipe', 'pipe'], detached: true });
      let timedOut = false;
      const timer = setTimeout(() => {
        timedOut = true;
        try { process.kill(-child.pid, 'SIGKILL'); } catch { /* spawn failure already recorded */ }
      }, timeoutMs);
      child.stdout?.pipe(out, { end: false });
      child.stderr?.pipe(err, { end: false });
      let launchError = null;
      child.once('error', (e) => { launchError = e.message; });
      child.once('close', (code, signal) => {
        clearTimeout(timer);
        resolve({ exit_code: code, signal, timed_out: timedOut, launch_error: launchError });
      });
    });
  } finally {
    await Promise.all([new Promise((resolve) => out.end(resolve)),
      new Promise((resolve) => err.end(resolve))]);
  }
  const row = {
    label, argv: [bin, ...argv], cwd, exit_code: result.exit_code, signal: result.signal,
    timed_out: result.timed_out, launch_error: result.launch_error,
    elapsed_ms: Number((process.hrtime.bigint() - start) / 1_000_000n),
    stdout: { path: path.relative(work, outPath), sha256: hashFile(outPath) },
    stderr: { path: path.relative(work, errPath), sha256: hashFile(errPath) },
  };
  receipt.commands.push(row);
  save();
  return { ...row, stdout_text: fs.readFileSync(outPath, 'utf8'),
    stderr_text: fs.readFileSync(errPath, 'utf8') };
}
function must(result, failureClass) {
  if (result.exit_code !== 0 || result.signal || result.launch_error || result.timed_out) {
    const error = new Error(`${result.label}: exit=${result.exit_code} signal=${result.signal} `
      + `timeout=${result.timed_out} launch=${result.launch_error}; see ${result.stderr.path}`);
    error.outcomeClass = result.timed_out ? 'timeout' : failureClass;
    throw error;
  }
  return result.stdout_text.trim();
}
function input(file, expectedHash) {
  if (!fs.existsSync(file) || !fs.statSync(file).isFile()) {
    const error = new Error(`input missing or not a regular file: ${file}`);
    error.outcomeClass = 'missing-input';
    throw error;
  }
  const hash = hashFile(file);
  if (expectedHash !== null && hash !== expectedHash) {
    const error = new Error(`tool-pin-mismatch ${file}: ${hash} != ${expectedHash}`);
    error.outcomeClass = 'tool-pin-mismatch';
    throw error;
  }
  return { path: file, sha256: hash, bytes: fs.statSync(file).size };
}
function manifestSha256(directory, tracked) {
  const hash = crypto.createHash('sha256');
  for (const relative of tracked) {
    const file = path.join(directory, relative);
    const stat = fs.lstatSync(file);
    if (stat.isDirectory()) {
      const link = spawnSync(gitBin, ['-C', directory, 'ls-files', '--stage', '--', relative],
        { encoding: 'utf8', timeout: 10_000 });
      const match = /^160000 ([0-9a-f]{40}) 0\t[^\n]+\n?$/u.exec(link.stdout ?? '');
      if (link.status !== 0 || !match) throw new Error(`unexpected source directory: ${relative}`);
      hash.update(`gitlink\0${relative}\0${match[1]}\n`);
      continue;
    }
    if (!stat.isFile() && !stat.isSymbolicLink()) throw new Error(`unexpected source entry: ${relative}`);
    const contents = stat.isSymbolicLink() ? Buffer.from(fs.readlinkSync(file)) : fs.readFileSync(file);
    hash.update(`${stat.isSymbolicLink() ? 'link' : 'file'}\0${relative}\0${sha256(contents)}\n`);
  }
  return hash.digest('hex');
}
async function checkout(source, dir, label) {
  must(await run(`${label}-init`, gitBin, ['init', '--quiet', dir]), 'source-unavailable');
  must(await run(`${label}-remote`, gitBin, ['-C', dir, 'remote', 'add', 'origin', source.repository]), 'source-unavailable');
  must(await run(`${label}-fetch`, gitBin, ['-C', dir, 'fetch', '--no-tags', '--depth=1', 'origin', source.revision],
    work, 300_000), 'source-unavailable');
  must(await run(`${label}-checkout`, gitBin, ['-C', dir, 'checkout', '--quiet', '--detach', 'FETCH_HEAD']), 'source-unavailable');
  const revision = must(await run(`${label}-revision`, gitBin, ['-C', dir, 'rev-parse', 'HEAD']), 'source-mismatch');
  const tree = must(await run(`${label}-tree`, gitBin, ['-C', dir, 'rev-parse', 'HEAD^{tree}']), 'source-mismatch');
  if (revision !== source.revision || tree !== source.source_tree_git_sha1) {
    const error = new Error(`${label}: expected commit/tree ${source.revision}/${source.source_tree_git_sha1}, got ${revision}/${tree}`);
    error.outcomeClass = 'source-mismatch';
    throw error;
  }
  const files = must(await run(`${label}-files`, gitBin, ['-C', dir, 'ls-files']), 'source-mismatch')
    .split('\n').filter(Boolean).sort();
  const record = {
    repository: source.repository, commit: revision, source_tree_git_sha1: tree,
    tracked_files: files.length, tracked_files_sha256: manifestSha256(dir, files),
  };
  if (source.license_file) {
    const license = fs.readFileSync(path.join(dir, source.license_file), 'utf8');
    if (source.license !== 'Apache-2.0' || !/Apache License[\s\S]{0,100}Version 2\.0/u.test(license)) {
      const error = new Error(`${label}: license bytes do not substantiate ${source.license}`);
      error.outcomeClass = 'license-mismatch';
      throw error;
    }
    Object.assign(record, { license: source.license, license_path: source.license_file,
      license_sha256: sha256(license) });
  }
  return record;
}
function patchLean4Lean(directory) {
  const file = path.join(directory, 'lakefile.toml');
  const before = fs.readFileSync(file, 'utf8');
  const needle = '[[lean_exe]]\n';
  if (before.split(needle).length !== 2 || before.includes('initialize_minimal')) {
    const error = new Error('lean4lean patch target is absent, ambiguous or already patched');
    error.outcomeClass = 'source-mismatch';
    throw error;
  }
  const after = before.replace(needle, `${needle}moreLinkArgs = ["-linitialize_minimal"]\n`);
  fs.writeFileSync(file, after);
  return { path: 'lakefile.toml', before_sha256: sha256(before), after_sha256: sha256(after),
    method: 'Lean 4.35.0 script/lean4lean-minimal-init.sh first [[lean_exe]] insertion' };
}
function deriveSemanticNegative(positive, negative) {
  const lines = fs.readFileSync(positive, 'utf8').split('\n');
  const names = new Map([[0, '']]);
  const rootName = sources.selected_export.root;
  let hit = null;
  for (let i = 0; i < lines.length; i++) {
    if (!lines[i]) continue;
    const record = JSON.parse(lines[i]);
    if (record.str && 'in' in record) {
      const pre = names.get(record.str.pre);
      if (pre === undefined) throw new Error(`source export name index ${record.str.pre} is undefined`);
      names.set(record.in, pre ? `${pre}.${record.str.str}` : record.str.str);
    }
    if (record.num && 'in' in record) {
      const pre = names.get(record.num.pre);
      if (pre === undefined) throw new Error(`source export name index ${record.num.pre} is undefined`);
      names.set(record.in, pre ? `${pre}.${record.num.i}` : String(record.num.i));
    }
    if (record.thm && names.get(record.thm.name) === rootName) {
      if (hit !== null || record.thm.type === record.thm.value) {
        throw new Error(`${rootName}: theorem proof mutation is ambiguous or vacuous`);
      }
      hit = { line: i, before_value: record.thm.value, type: record.thm.type };
      record.thm.value = record.thm.type;
      lines[i] = JSON.stringify(record);
    }
  }
  if (hit === null) throw new Error(`${rootName}: unique theorem missing from export`);
  fs.writeFileSync(negative, lines.join('\n'), { flag: 'wx', mode: 0o600 });
  return hit;
}
function inspect(file) {
  const exportInfo = inspectExport(file);
  if (exportInfo.format !== criteria.selected_export_format
      || !exportInfo.declared.has(sources.selected_export.root)
      || foreignAxioms(exportInfo).length > 0) {
    throw new Error(`export format, required root or allowed axioms invalid: ${file}`);
  }
  return { ...input(file, null), format: exportInfo.format, records: exportInfo.records,
    declared: exportInfo.declared.size, axioms: exportInfo.axioms };
}
function checkerCommand(checker, binary, exportFile, label) {
  if (checker.run.kind === 'nanoda-config') {
    const file = path.join(work, `${label}.nanoda.json`);
    fs.writeFileSync(file, `${JSON.stringify({ use_stdin: false, export_file_path: exportFile,
      permitted_axioms: criteria.allowed_axioms, unpermitted_axiom_hard_error: true,
      num_threads: checker.run.threads, nat_extension: true, string_extension: true,
      print_axioms: false, print_success_message: true }, null, 2)}\n`, { flag: 'wx', mode: 0o600 });
    return { argv: [binary, file], config: input(file, null) };
  }
  return { argv: [binary, ...checker.run.argv.map((part) => part === 'FILE' ? exportFile : part)], config: null };
}
function classify(result, checker, positive) {
  const text = `${result.stdout_text}\n${result.stderr_text}`;
  if (result.timed_out) return { outcome: 'timeout' };
  if (result.launch_error || result.signal || result.exit_code === null) {
    return { outcome: /ENOMEM|out of memory|allocat/iu.test(text) ? 'resource-exhausted' : 'checker-error' };
  }
  if (result.exit_code === 2 || /unsupported|not supported|declined/iu.test(text)) return { outcome: 'unsupported' };
  if (/out of memory|cannot allocate|allocation failed|resource temporarily unavailable|ENOMEM/iu.test(text)) {
    return { outcome: 'resource-exhausted' };
  }
  if (/\bparse\b.{0,60}(?:error|fail|invalid)|\b(?:json|ndjson)\b.{0,60}(?:error|fail|invalid)|\bexpected .{1,80} after\b/iu.test(text)) {
    return { outcome: 'parse-failure' };
  }
  if (result.exit_code === 0) {
    const count = Number(new RegExp(checker.accept_pattern, 'u').exec(result.stdout_text)?.[1] ?? 0);
    return count > 0 ? { outcome: 'accepted', checked_declarations: count } : { outcome: 'checker-error' };
  }
  if (/\bunpermitted axiom\b/iu.test(text)) return { outcome: 'forbidden-axiom' };
  const rejection = /mismatch|invalid:|rejected|assertion failed|\(kernel\)/iu.test(text);
  return { outcome: rejection ? 'kernel-rejection' : 'checker-error',
    rejection_shaped: rejection, selected_negative: !positive };
}

async function main() {
  const gitStatus = must(await run('source-status', gitBin, ['-C', root, 'status', '--porcelain', '--untracked-files=all']), 'source-dirty');
  receipt.source_commit = must(await run('source-commit', gitBin, ['-C', root, 'rev-parse', 'HEAD']), 'source-unavailable');
  if (gitStatus) {
    const error = new Error('source checkout is dirty; source-build results would not bind a committed cohort');
    error.outcomeClass = 'source-dirty';
    throw error;
  }
  receipt.tools.node = input(process.execPath, null);
  for (const [name, file, expected] of [
    ['lean', 'lean', pins.toolchain.lean_sha256],
    ['lake', 'lake', pins.toolchain.lake_sha256],
    ['leanexport', 'leanexport', pins.toolchain.leanexport_sha256],
  ]) receipt.tools[name] = input(path.join(leanBin, file), expected);
  const leanVersion = must(await run('lean-version', path.join(leanBin, 'lean'), ['--version']), 'tool-error');
  if (!leanVersion.includes('4.35.0')) throw new Error(`Lean version does not match selected toolchain: ${leanVersion}`);
  receipt.tools.lean.version = leanVersion;
  const positive = path.join(work, 'positive.ndjson');
  const exportRun = await run('positive-export', path.join(leanBin, 'leanexport'),
    [sources.selected_export.module, '--', sources.selected_export.root], work, 900_000);
  must(exportRun, 'export-failure');
  fs.copyFileSync(path.join(work, exportRun.stdout.path), positive, fs.constants.COPYFILE_EXCL);
  const negative = path.join(work, 'negative.ndjson');
  const mutation = deriveSemanticNegative(positive, negative);
  receipt.exports = { positive: inspect(positive), negative: inspect(negative), mutation };
  if (receipt.exports.positive.sha256 === receipt.exports.negative.sha256
      || receipt.exports.positive.declared !== receipt.exports.negative.declared
      || JSON.stringify(receipt.exports.positive.axioms) !== JSON.stringify(receipt.exports.negative.axioms)) {
    throw new Error('negative export changed declarations/axioms unexpectedly or did not change bytes');
  }
  save();
  for (const source of sources.sources) {
    const checker = pins.checkers.find((item) => item.id === source.id);
    const row = { id: source.id, outcome: 'NO-GO', outcome_class: null,
      pinned_release_binary_sha256: checker.sha256, bundled_binary: null,
      source: null, dependencies: [], patch: null,
      build: null, binary: null, runs: [], detail: null };
    receipt.checkers.push(row);
    save();
    try {
      if (!checker.binary_arg) {
        row.bundled_binary = input(path.join(leanBin, checker.binary), checker.sha256);
      }
      const directory = path.join(work, 'sources', source.id);
      row.source = await checkout(source, directory, source.id);
      if (source.id === 'lean4lean') {
        const lock = JSON.parse(fs.readFileSync(path.join(directory, 'lake-manifest.json'), 'utf8'));
        for (const dependency of source.dependencies) {
          const locked = lock.packages.find((item) => item.name === dependency.name);
          if (!locked || locked.url !== dependency.repository || locked.rev !== dependency.revision) {
            const error = new Error(`lean4lean lockfile disagrees with pinned ${dependency.name}`);
            error.outcomeClass = 'dependency-mismatch';
            throw error;
          }
          const depDir = path.join(directory, '.lake', 'packages', dependency.name);
          fs.mkdirSync(path.dirname(depDir), { recursive: true, mode: 0o700 });
          row.dependencies.push(await checkout(dependency, depDir, `${source.id}-${dependency.name}`));
        }
        row.patch = patchLean4Lean(directory);
      }
      let binary;
      if (source.id === 'ocaml4lean') {
        const recipe = fs.readFileSync(derivationPath, 'utf8');
        if (![source.revision, source.source_nar_hash, source.nixpkgs_revision,
          source.nixpkgs_nar_hash].every((text) => recipe.includes(text))) {
          const error = new Error('OCaml Nix derivation differs from pinned source/compiler input revisions');
          error.outcomeClass = 'build-recipe-mismatch';
          throw error;
        }
        const buildRun = await run(`${source.id}-nix-build`, nixBin,
          ['build', '--no-link', '--print-out-paths', '--file', derivationPath], root, 2_700_000);
        row.build = commandRecord(buildRun);
        const storeOutput = must(buildRun, 'build-failed').split('\n').filter(Boolean);
        if (storeOutput.length !== 1 || !storeOutput[0].startsWith('/nix/store/')) {
          const error = new Error('OCaml build returned no single immutable Nix output path');
          error.outcomeClass = 'build-error';
          throw error;
        }
        row.compiler = { nixpkgs_revision: source.nixpkgs_revision,
          nixpkgs_nar_hash: source.nixpkgs_nar_hash,
          derivation_sha256: hashFile(derivationPath), output: storeOutput[0] };
        const compilerExpression = `let pkgs = import (builtins.fetchTree {
          type = "github"; owner = "NixOS"; repo = "nixpkgs";
          rev = "${source.nixpkgs_revision}"; narHash = "${source.nixpkgs_nar_hash}";
        }) { system = "x86_64-linux"; }; in "\${pkgs.ocaml}/bin/ocamlopt"`;
        const compilerPath = must(await run(`${source.id}-compiler-path`, nixBin,
          ['eval', '--raw', '--expr', compilerExpression], root), 'tool-error');
        row.compiler.ocamlopt = input(compilerPath, null);
        row.compiler.version = must(await run(`${source.id}-compiler-version`,
          compilerPath, ['-version']), 'tool-error');
        binary = path.join(storeOutput[0], source.binary);
      } else {
        if (source.compiler === 'repo-rust-toolchain') {
          const rustc = input(path.join(rustBin, 'rustc'), null);
          const cargo = input(path.join(rustBin, 'cargo'), null);
          const cc = input(path.join(ccBin, 'cc'), null);
          const version = must(await run(`${source.id}-rustc-version`, rustc.path,
            ['--version', '--verbose']), 'tool-error');
          const channel = fs.readFileSync(path.join(root, 'rust-toolchain.toml'), 'utf8')
            .match(/channel\s*=\s*"([^"]+)"/u)?.[1];
          if (channel !== 'nightly-2026-03-21' || !version.includes('1.96.0-nightly')) {
            const error = new Error(`${source.id}: compiler revision differs from repository Rust pin ${channel}: ${version}`);
            error.outcomeClass = 'tool-pin-mismatch';
            throw error;
          }
          row.compiler = { channel, rustc: { ...rustc, version }, cargo, cc,
            cargo_version: must(await run(`${source.id}-cargo-version`, cargo.path, ['--version']), 'tool-error'),
            cc_version: must(await run(`${source.id}-cc-version`, cc.path, ['--version']), 'tool-error') };
        } else {
          row.compiler = { lean: receipt.tools.lean, lake: receipt.tools.lake,
            source_declared_toolchain: source.declared_lean_toolchain };
        }
        const executable = source.compiler === 'repo-rust-toolchain'
          ? path.join(rustBin, source.build[0]) : path.join(leanBin, source.build[0]);
        const buildRun = await run(`${source.id}-build`, executable, source.build.slice(1), directory, 2_700_000);
        row.build = commandRecord(buildRun);
        must(buildRun, 'build-incompatible');
        binary = path.join(directory, source.binary);
      }
      // Lake can update a manifest and Cargo can run build scripts: refuse if tracked source moved.
      const tracked = must(await run(`${source.id}-tracked-after-build`, gitBin,
        ['-C', directory, 'status', '--porcelain', '--untracked-files=no']), 'source-mismatch');
      if (tracked !== (source.id === 'lean4lean' ? 'M lakefile.toml' : '')) {
        const error = new Error(`${source.id}: tracked source changed during build: ${tracked}`);
        error.outcomeClass = 'source-mismatch';
        throw error;
      }
      if (row.patch && hashFile(path.join(directory, row.patch.path)) !== row.patch.after_sha256) {
        const error = new Error('lean4lean source patch changed during build');
        error.outcomeClass = 'source-mismatch';
        throw error;
      }
      if (!fs.existsSync(binary)) {
        const error = new Error(`${source.id}: build exited zero without the expected executable ${binary}`);
        error.outcomeClass = 'build-output-missing';
        throw error;
      }
      row.binary = input(binary, null);
      row.binary.release_digest_match = row.binary.sha256 === checker.sha256;
      if (source.id === 'ocaml4lean' && !row.binary.release_digest_match) {
        const error = new Error('OCaml source-built executable digest differs from the configured source-built checker pin');
        error.outcomeClass = 'binary-pin-mismatch';
        throw error;
      }
      for (const [kind, file] of [['positive', positive], ['negative', negative]]) {
        const sourceHash = receipt.exports[kind].sha256;
        if (hashFile(file) !== sourceHash || hashFile(binary) !== row.binary.sha256) {
          const error = new Error(`${source.id}: export or executable changed before ${kind} replay`);
          error.outcomeClass = 'input-mismatch';
          throw error;
        }
        const command = checkerCommand(checker, binary, file, `${source.id}-${kind}`);
        const result = await run(`${source.id}-${kind}-replay`, command.argv[0], command.argv.slice(1),
          work, 900_000);
        const verdict = classify(result, checker, kind === 'positive');
        row.runs.push({ kind, export_sha256: sourceHash, executable_sha256: row.binary.sha256,
          config: command.config, command: commandRecord(result), ...verdict });
        if (hashFile(file) !== sourceHash || hashFile(binary) !== row.binary.sha256) {
          const error = new Error(`${source.id}: export or executable changed during ${kind} replay`);
          error.outcomeClass = 'input-mismatch';
          throw error;
        }
      }
      const [positiveRun, negativeRun] = row.runs;
      if (positiveRun.outcome !== 'accepted' || negativeRun.outcome !== 'kernel-rejection') {
        const error = new Error(`${source.id}: expected positive=accepted and negative=kernel-rejection; got ${positiveRun.outcome} / ${negativeRun.outcome}`);
        error.outcomeClass = 'replay-incompatible';
        throw error;
      }
      row.outcome = 'GO';
      row.outcome_class = row.binary.release_digest_match
        ? 'source-built-byte-identical-and-replay-compatible'
        : 'source-built-replay-compatible-binary-differs-from-release';
    } catch (error) {
      row.outcome = 'NO-GO';
      row.outcome_class = error.outcomeClass ?? 'source-build-error';
      row.detail = String(error);
    }
    save();
  }
  receipt.outcome = receipt.checkers.length === required.length
      && receipt.checkers.every((row) => row.outcome === 'GO') ? 'GO' : 'NO-GO';
}
try {
  await main();
} catch (error) {
  receipt.error = { outcome_class: error.outcomeClass ?? 'preflight-error', detail: String(error) };
  receipt.outcome = 'NO-GO';
  for (const id of required.slice(receipt.checkers.length)) {
    receipt.checkers.push({ id, outcome: 'NO-GO', outcome_class: receipt.error.outcome_class,
      detail: `source-build preflight stopped before ${id}: ${receipt.error.detail}` });
  }
} finally {
  save();
  console.log(`source-builds outcome=${receipt.outcome} receipt=${receiptPath}`);
}
if (receipt.outcome !== 'GO') process.exitCode = 1;
