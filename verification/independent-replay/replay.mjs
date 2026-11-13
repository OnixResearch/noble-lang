#!/usr/bin/env node
// Producer-independent replay of the foundation's compiled Lean declarations, with an artifact
// integrity step in front of it (issue #62).
//
// The measured single-byte control in `receipts/2026-10-26/` showed that `.olean` loading detects
// header/parse failure rather than byte integrity, and that `leanchecker` does not consult the
// `.olean.hash` file. So this lane refuses BEFORE replaying when the replayed `.olean` set does not
// match the frozen digest manifest: a corrupted or substituted artifact is an `artifact-integrity`
// refusal, never a silent acceptance.
//
// The digest set is exactly what `lake env` puts on `LEAN_PATH` for the foundation, because replay
// resolves imports through that same search path. Directories inside the repository are frozen file
// by file. Shared `.lake/packages` dependencies and the pinned toolchain's own `lib/lean` are frozen
// as one aggregate per directory — count, total bytes and a Merkle digest over sorted
// `<sha256> <bytes> <relative path>` rows — because they are read-only dependency closures the lane
// never builds or mutates.
//
// The lane runs the policy-selected Lean 4.35.0 release by default; the toolchain's `leanchecker` is
// the producer-independent replay of issue #62 (it is Lean's own kernel, so it is not an independent
// implementation, which is why `multi.mjs` runs the separately written checkers over exports).
//
// Modes:
//   --freeze                 write the digest manifest for the search path as it stands
//   (default)                verify the search path against the manifest, then replay `leanchecker`
//   --controls               exercise corruption controls against a copy of the search path
import crypto from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

const root = process.cwd();
const argv = process.argv.slice(2);
const flag = (name) => argv.includes(name);
const value = (name, fallback) => {
  const index = argv.indexOf(name);
  return index >= 0 ? argv[index + 1] : fallback;
};

const project = path.resolve(root, value('--project', 'proofs/foundation'));
const manifestPath = path.resolve(root, value('--manifest',
  'verification/independent-replay/olean-integrity.txt'));
const moduleName = value('--module', 'NobleFoundation');
// `lake` and `leanchecker` ship with the pinned toolchain rather than with the lane's process PATH.
const leanBin = path.resolve(root, value('--lean-bin',
  '/nix/store/kwvw84h9r7w0q40d3cm5xgfz0rpv7yzc-noble-lean-release-4.35.0/bin'));
const lakeBin = path.join(leanBin, 'lake');

function lake(...args) {
  const run = spawnSync(lakeBin, args, {
    cwd: project, encoding: 'utf8', timeout: 1800_000,
    env: { ...process.env, HOME: os.homedir(),
      PATH: leanBin + ':' + (process.env.PATH ?? '') },
  });
  const text = `${run.stdout ?? ''}${run.stderr ?? ''}`;
  if (run.status !== 0) {
    throw new Error(`lake ${args.join(' ')} failed (${run.status}): ${text.slice(-2000)}`);
  }
  return text;
}

/// Every existing directory `lake env` puts on `LEAN_PATH`, and whether the lane treats it as internal.
function searchPath() {
  const sharedPackages = `${path.join(project, '.lake', 'packages')}${path.sep}`;
  return lake('env', 'printenv', 'LEAN_PATH').split(':').map((entry) => entry.trim())
    .filter((entry) => entry.length > 0)
    .map((entry) => path.resolve(entry))
    .filter((entry) => fs.existsSync(entry))
    .map((entry) => ({ path: entry,
      internal: (entry === root || entry.startsWith(`${root}${path.sep}`))
        && !entry.startsWith(sharedPackages) }));
}

/// Stable name for a directory outside the repository: the dependency path after `.lake/packages`,
/// else the directory itself (the toolchain's store path, which pins what it names).
function labelOf(directory) {
  const marker = `${path.sep}.lake${path.sep}packages${path.sep}`;
  const index = directory.indexOf(marker);
  if (index >= 0) return `packages/${directory.slice(index + marker.length)}`;
  return directory;
}

/// Absolute paths of every `.olean` under `directory`, sorted.
function oleanFiles(directory) {
  const found = [];
  const walk = (current) => {
    for (const entry of fs.readdirSync(current, { withFileTypes: true })) {
      const full = path.join(current, entry.name);
      if (entry.isDirectory()) {
        walk(full);
      } else if (entry.isFile() && entry.name.endsWith('.olean')) {
        found.push(full);
      }
    }
  };
  walk(directory);
  return found.sort();
}

/// The same files named the way a manifest names them, relative to one search-path entry.
function oleanPaths(directory, naming) {
  return oleanFiles(directory).map((full) => path.join(naming, path.relative(directory, full)));
}

function digestFrom(file) {
  return crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
}

function sha256(text) {
  return crypto.createHash('sha256').update(text).digest('hex');
}

/// One digest per directory outside the repository: count, total bytes and a Merkle digest over the
/// sorted per-file rows, so a changed, added or removed artifact changes the aggregate.
function aggregate(directory) {
  const rows = [];
  let bytes = 0;
  for (const full of oleanFiles(directory)) {
    const size = fs.statSync(full).size;
    bytes += size;
    rows.push(`${digestFrom(full)} ${size} ${path.relative(directory, full)}`);
  }
  rows.sort();
  return { files: rows.length, bytes, merkle: sha256(rows.join('\n')) };
}

function freeze() {
  const directories = searchPath();
  const internal = directories.filter((entry) => entry.internal);
  const external = directories.filter((entry) => !entry.internal);
  const files = internal.flatMap((entry) => oleanFiles(entry.path));
  const total = files.reduce((sum, full) => sum + fs.statSync(full).size, 0);
  const externals = external.map((entry) => ({
    label: labelOf(entry.path), path: entry.path, ...aggregate(entry.path),
  }));
  const header = [
    '# noble-olean-integrity/v2',
    '# Frozen digest manifest for the exact `.olean` set the producer-independent replay resolves',
    '# through `lake env`\'s LEAN_PATH.',
    '#',
    '# Directories inside the repository are frozen file by file: <sha256> <bytes> <path>.',
    '# Shared dependency paths and directories outside it are frozen as one aggregate per directory:',
    '#   external-path <label> <files> <bytes> <merkle sha256>',
    '# where the Merkle digest is over the sorted rows "<sha256> <bytes> <path relative to the file>".',
    '# Shared `.lake/packages` dependency artifacts and the pinned toolchain itself are',
    '# aggregated because the lane never builds or mutates them; it only replays through them.',
    '#',
    '# The checker does not consult `.olean.hash`, so this manifest is the digest step the replay',
    '# depends on: a corrupted, substituted, removed or extra artifact is an `artifact-integrity`',
    '# refusal before any checker process runs, never a silent acceptance.',
    'schema noble-olean-integrity/v2',
    `project ${path.relative(root, project)}`,
    `module ${moduleName}`,
    `file-count ${files.length}`,
    `total-bytes ${total}`,
    ...internal.map((entry) => `search-path ${path.relative(root, entry.path)}`),
    ...externals.map((entry) => `external-path ${entry.label} ${entry.files} ${entry.bytes} ${entry.merkle}`),
  ];
  const body = files.map((full) =>
    `${digestFrom(full)} ${fs.statSync(full).size} ${path.relative(root, full)}`);
  fs.writeFileSync(manifestPath, `${[...header, ...body].join('\n')}\n`);
  return {
    outcome: 'frozen', manifest: path.relative(root, manifestPath),
    file_count: files.length, total_bytes: total,
    internal: internal.map((entry) => path.relative(root, entry.path)),
    external: externals.map(({ label, files: count, bytes, merkle }) => ({ label, files: count, bytes, merkle })),
  };
}

function readManifest() {
  const lines = fs.readFileSync(manifestPath, 'utf8').split('\n').map((line) => line.trim())
    .filter((line) => line.length > 0 && !line.startsWith('#'));
  const schema = lines.find((line) => line.startsWith('schema '))?.slice('schema '.length);
  const directories = lines
    .filter((line) => line.startsWith('search-path '))
    .map((line) => line.slice('search-path '.length));
  const externals = lines
    .filter((line) => line.startsWith('external-path '))
    .map((line) => {
      const [, label, files, bytes, merkle] = line.split(' ');
      return { label, files: Number(files), bytes: Number(bytes), merkle };
    });
  const entries = [];
  for (const line of lines) {
    const parts = line.split(' ');
    if (parts.length !== 3 || !/^[0-9a-f]{64}$/.test(parts[0])) {
      continue;
    }
    entries.push({ sha256: parts[0], path: parts[2] });
  }
  return { schema, directories, externals, entries };
}

/// Verify a search path against the manifest. Returns a refusal class instead of throwing, so controls
/// can assert the class and that no checker process ran.
function verifyAgainstManifest(presented) {
  const manifest = readManifest();
  if (manifest.schema !== 'noble-olean-integrity/v2') {
    return { class: 'artifact-integrity', detail: 'manifest schema mismatch', checker_invoked: false };
  }
  const internal = presented.filter((entry) => entry.internal);
  const external = presented.filter((entry) => !entry.internal);
  if (internal.length !== manifest.directories.length) {
    return { class: 'artifact-integrity', detail: 'search-path arity mismatch', checker_invoked: false };
  }
  const externalLabels = external.map((entry) => entry.label ?? labelOf(entry.path));
  const expectedLabels = manifest.externals.map((entry) => entry.label);
  if (externalLabels.length !== expectedLabels.length
      || externalLabels.some((label, index) => label !== expectedLabels[index])) {
    return { class: 'artifact-integrity', detail: 'external search-path set mismatch', checker_invoked: false };
  }
  // Presented paths are named the way the manifest names them, so a copy of the search path is
  // verified against the same rows as the retained tree.
  const presentedPaths = internal.map((entry, index) =>
    oleanPaths(entry.path, manifest.directories[index]));
  const present = new Set(presentedPaths.flat());
  const expected = new Set(manifest.entries.map((entry) => entry.path));
  const missing = [...expected].filter((rel) => !present.has(rel));
  if (missing.length > 0) {
    return { class: 'artifact-integrity', detail: `missing ${missing[0]}`, checker_invoked: false };
  }
  const extra = [...present].filter((rel) => !expected.has(rel));
  if (extra.length > 0) {
    return { class: 'artifact-integrity', detail: `unexpected ${extra[0]}`, checker_invoked: false };
  }
  for (const entry of manifest.entries) {
    const index = manifest.directories.findIndex((directory) =>
      entry.path === directory || entry.path.startsWith(`${directory}${path.sep}`));
    if (index < 0) {
      return { class: 'artifact-integrity', detail: `outside the search path ${entry.path}`,
        checker_invoked: false };
    }
    const actual = path.join(internal[index].path, path.relative(manifest.directories[index], entry.path));
    if (digestFrom(actual) !== entry.sha256) {
      return { class: 'artifact-integrity', detail: `digest mismatch ${entry.path}`, checker_invoked: false };
    }
  }
  for (const [index, expectedEntry] of manifest.externals.entries()) {
    const actual = aggregate(external[index].path);
    if (actual.files !== expectedEntry.files || actual.bytes !== expectedEntry.bytes
        || actual.merkle !== expectedEntry.merkle) {
      return { class: 'artifact-integrity',
        detail: `aggregate mismatch ${expectedEntry.label} (${actual.files} files, ${actual.bytes} bytes, ${actual.merkle})`,
        checker_invoked: false };
    }
  }
  return { class: 'artifact-intact',
    detail: `${manifest.entries.length} files in ${manifest.directories.length} directories, ${manifest.externals.length} aggregates`,
    checker_invoked: false };
}

function replay(presented) {
  const integrity = verifyAgainstManifest(presented);
  if (integrity.class !== 'artifact-intact') {
    return { ...integrity, outcome: 'refused' };
  }
  const started = Date.now();
  const run = spawnSync(lakeBin, ['env', 'leanchecker', moduleName], {
    cwd: project, encoding: 'utf8', timeout: 1800_000,
    env: { ...process.env, HOME: os.homedir(),
      PATH: leanBin + ':' + (process.env.PATH ?? '') },
  });
  const elapsed = Date.now() - started;
  const text = `${run.stdout ?? ''}${run.stderr ?? ''}`.trim();
  if (run.error?.code === 'ETIMEDOUT') {
    return { outcome: 'refused', class: 'timeout', checker_invoked: true, elapsed_ms: elapsed };
  }
  if (run.status !== 0) {
    return { outcome: 'refused', class: 'replay-rejected', checker_invoked: true,
      status: run.status, elapsed_ms: elapsed, log_tail: text.slice(-2000) };
  }
  return { outcome: 'kernel-accepted', class: 'replay-accepted', checker_invoked: true,
    status: run.status, elapsed_ms: elapsed, silent: text === '', log_tail: text.slice(-2000) };
}

function replayAgainst(presented) {
  const integrity = verifyAgainstManifest(presented);
  return { integrity: integrity.class, detail: integrity.detail, checker_invoked: integrity.checker_invoked };
}

/// Copy each internal search-path directory, damage one artifact in a copy, and require the integrity
/// step to refuse before any checker process runs. The external directories are not copied: they are
/// read-only dependency closures, so the control damages a copy of the smallest one instead. The
/// untouched copy is verified too, so the control set cannot pass by refusing everything.
function controls() {
  const temporary = fs.mkdtempSync(path.join(os.tmpdir(), 'noble-olean-controls-'));
  const directories = searchPath();
  const internal = directories.filter((entry) => entry.internal);
  const external = directories.filter((entry) => !entry.internal);
  const copies = internal.map((entry, index) => {
    const copy = path.join(temporary, `path-${index}`);
    fs.cpSync(entry.path, copy, { recursive: true });
    return { path: copy, internal: true };
  });
  const presented = [...copies, ...external];
  const intact = replayAgainst(presented);
  if (intact.integrity !== 'artifact-intact') {
    throw new Error(`the untouched copy is not intact: ${intact.detail}`);
  }
  const flipTarget = copies.flatMap((entry) => oleanFiles(entry.path))
    .find((rel) => rel.endsWith('NobleFoundation.olean'));
  if (flipTarget === undefined) {
    throw new Error('the copy has no NobleFoundation olean to control against');
  }
  const bytes = fs.readFileSync(flipTarget);
  const position = Math.floor(bytes.length / 2);
  bytes[position] ^= 0x01;
  fs.writeFileSync(flipTarget, bytes);
  const flipped = replayAgainst(presented);
  const removeTarget = copies.flatMap((entry) => oleanFiles(entry.path))[0];
  fs.rmSync(removeTarget);
  const removed = replayAgainst(presented);
  // Rebuild the copy so an extra artifact is observed as *extra* rather than behind the removal.
  internal.forEach((entry, index) => {
    fs.rmSync(copies[index].path, { recursive: true, force: true });
    fs.cpSync(entry.path, copies[index].path, { recursive: true });
  });
  fs.writeFileSync(path.join(copies[0].path, 'Extra.olean'), 'not an olean');
  const extra = replayAgainst(presented);
  fs.rmSync(path.join(copies[0].path, 'Extra.olean'));
  // Smallest external directory, copied and damaged: the aggregate digest must catch it.
  const smallest = external
    .map((entry) => ({ entry, snapshot: aggregate(entry.path) }))
    .filter(({ snapshot }) => snapshot.files > 0)
    .sort((left, right) => left.snapshot.bytes - right.snapshot.bytes)[0];
  let externalControl = { name: 'external-aggregate-flip', skipped: 'no external directory' };
  if (smallest !== undefined) {
    const copy = path.join(temporary, 'external');
    fs.cpSync(smallest.entry.path, copy, { recursive: true });
    const target = oleanFiles(copy)[0];
    if (target !== undefined) {
      const damaged = fs.readFileSync(target);
      damaged[Math.floor(damaged.length / 2)] ^= 0x01;
      fs.writeFileSync(target, damaged);
    }
    const target2 = oleanFiles(copy).at(-1);
    if (target2 !== undefined && target2 !== target) fs.rmSync(target2);
    const damagedPresented = [...copies.filter((entry) => fs.existsSync(entry.path)), ...external]
      .map((entry) => (entry.path === smallest.entry.path
        ? { path: copy, internal: false, label: labelOf(smallest.entry.path) }
        : entry));
    externalControl = {
      name: 'external-aggregate-flip',
      target: labelOf(smallest.entry.path),
      result: replayAgainst(damagedPresented),
    };
  }
  fs.rmSync(temporary, { recursive: true, force: true });
  const checks = {
    intact_copy_accepted:
      intact.integrity === 'artifact-intact' && intact.checker_invoked === false,
    single_byte_flip_refused:
      flipped.integrity === 'artifact-integrity' && flipped.checker_invoked === false,
    removed_artifact_refused:
      removed.integrity === 'artifact-integrity' && removed.checker_invoked === false,
    extra_artifact_refused:
      extra.integrity === 'artifact-integrity' && extra.checker_invoked === false,
    external_aggregate_flip_refused:
      externalControl.result?.integrity === 'artifact-integrity'
        && externalControl.result.checker_invoked === false,
  };
  return {
    holds: Object.values(checks).every(Boolean),
    checks,
    intact,
    coverage: {
      internal: internal.map((entry) => path.relative(root, entry.path)),
      external: external.map((entry) => ({ label: labelOf(entry.path), path: entry.path })),
    },
    controls: [
      { name: 'single-byte-flip', target: path.basename(flipTarget), position, result: flipped },
      { name: 'removed-artifact', target: removeTarget === undefined ? null : path.basename(removeTarget), result: removed },
      { name: 'unexpected-artifact', result: extra },
      externalControl,
    ],
  };
}

if (flag('--freeze')) {
  console.log(JSON.stringify(freeze()));
} else if (flag('--controls')) {
  const result = controls();
  console.log(JSON.stringify({
    ...result,
    source_commit: spawnSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).stdout.trim(),
    project: path.relative(root, project),
    manifest: path.relative(root, manifestPath),
    host: `${os.type()} ${os.release()} ${os.arch()}`,
  }, null, 2));
  process.exitCode = result.holds ? 0 : 1;
} else {
  const result = replay(searchPath());
  console.log(JSON.stringify({
    ...result,
    source_commit: spawnSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).stdout.trim(),
    project: path.relative(root, project),
    module: moduleName,
    manifest: path.relative(root, manifestPath),
    host: `${os.type()} ${os.release()} ${os.arch()}`,
  }));
  process.exitCode = result.outcome === 'kernel-accepted' ? 0 : 1;
}
