#!/usr/bin/env node
// Producer-independent replay of the foundation's compiled Lean declarations, with an artifact
// integrity step in front of it (issue #62).
//
// The measured single-byte control in `receipts/2026-10-26/` showed that `.olean` loading detects
// header/parse failure rather than byte integrity, and that `leanchecker` does not consult the
// `.olean.hash` file. So this lane refuses BEFORE replaying when the retained `.olean` set does not
// match the frozen digest manifest: a corrupted or substituted artifact is an `artifact-integrity`
// refusal, never a silent acceptance.
//
// The digest set is exactly what `lake env` puts on `LEAN_PATH` for the foundation, because replay
// resolves imports through that same search path.
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
//  and  ship with the pinned toolchain rather than with the lane's process PATH.
const leanBin = path.resolve(root, value('--lean-bin',
  '/nix/store/hbxwgcbpkyqllqzq7cidhnvhgb60jb9q-noble-lean-release-4.31.0/bin'));
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

/// The exact search path replay resolves through, minus entries that are not this project's.
function searchPath() {
  return lake('env', 'printenv', 'LEAN_PATH').split(':').filter((entry) => entry.length > 0)
    .map((entry) => path.resolve(entry))
    .filter((entry) => entry.startsWith(root))
    // `lake env` lists every declared package path, including ones this project never built. A path
    // with no artifacts contributes nothing to the replay and is not frozen.
    .filter((entry) => fs.existsSync(entry));
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

function digest(file) {
  return crypto.createHash('sha256').update(fs.readFileSync(path.join(root, file))).digest('hex');
}

function digestFrom(file) {
  return crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
}

function freeze() {
  const directories = searchPath();
  const files = directories.flatMap((directory) => oleanFiles(directory));
  const total = files.reduce((sum, full) => sum + fs.statSync(full).size, 0);
  const header = [
    '# noble-olean-integrity/v1',
    '# Frozen digest manifest for the exact `.olean` set the producer-independent replay resolves',
    '# through `lake env`\'s LEAN_PATH. One line per artifact: <sha256> <bytes> <path>.',
    '#',
    '# The checker does not consult `.olean.hash`, so this manifest is the digest step the replay',
    '# depends on: a corrupted, substituted, removed or extra artifact is an `artifact-integrity`',
    '# refusal before any checker process runs, never a silent acceptance.',
    `schema noble-olean-integrity/v1`,
    `project ${path.relative(root, project)}`,
    `module ${moduleName}`,
    `file-count ${files.length}`,
    `total-bytes ${total}`,
    ...directories.map((entry) => `search-path ${path.relative(root, entry)}`),
  ];
  const body = files.map((full) =>
    `${digestFrom(full)} ${fs.statSync(full).size} ${path.relative(root, full)}`);
  fs.writeFileSync(manifestPath, `${[...header, ...body].join('\n')}\n`);
  return { outcome: 'frozen', manifest: path.relative(root, manifestPath),
    file_count: files.length, total_bytes: total };
}

function readManifest() {
  const lines = fs.readFileSync(manifestPath, 'utf8').split('\n').map((line) => line.trim())
    .filter((line) => line.length > 0);
  const schema = lines.find((line) => line.startsWith('schema '))?.slice('schema '.length);
  const directories = lines
    .filter((line) => line.startsWith('search-path '))
    .map((line) => line.slice('search-path '.length));
  const entries = [];
  for (const line of lines) {
    if (line.startsWith('#')) {
      continue;
    }
    const parts = line.split(' ');
    if (parts.length !== 3 || !/^[0-9a-f]{64}$/.test(parts[0])) {
      continue;
    }
    entries.push({ sha256: parts[0], path: parts[2] });
  }
  return { schema, directories, entries };
}

/// Verify a search path against the manifest. Returns a refusal class instead of throwing, so controls
/// can assert the class and that no checker process ran.
function verifyAgainstManifest(directories) {
  const manifest = readManifest();
  if (manifest.schema !== 'noble-olean-integrity/v1') {
    return { class: 'artifact-integrity', detail: 'manifest schema mismatch', checker_invoked: false };
  }
  if (directories.length !== manifest.directories.length) {
    return { class: 'artifact-integrity', detail: 'search-path arity mismatch', checker_invoked: false };
  }
  // Presented paths are named the way the manifest names them, so a copy of the search path is
  // verified against the same rows as the retained tree.
  const presented = directories.map((directory, index) =>
    oleanPaths(directory, manifest.directories[index]));
  const present = new Set(presented.flat());
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
    const actual = path.join(directories[index], path.relative(manifest.directories[index], entry.path));
    if (digestFrom(actual) !== entry.sha256) {
      return { class: 'artifact-integrity', detail: `digest mismatch ${entry.path}`, checker_invoked: false };
    }
  }
  return { class: 'artifact-intact', detail: `${manifest.entries.length} files`, checker_invoked: false };
}

function replay(directories) {
  const integrity = verifyAgainstManifest(directories);
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

function replayAgainst(directories) {
  const integrity = verifyAgainstManifest(directories);
  return { integrity: integrity.class, detail: integrity.detail, checker_invoked: integrity.checker_invoked };
}

/// Copy each retained search-path directory, damage one artifact in the copy, and require the
/// integrity step to refuse before any checker process runs. The untouched copy is verified too, so
/// the control set cannot pass by refusing everything.
function controls() {
  const temporary = fs.mkdtempSync(path.join(os.tmpdir(), 'noble-olean-controls-'));
  const directories = searchPath();
  const copies = directories.map((directory, index) => {
    const copy = path.join(temporary, `path-${index}`);
    fs.cpSync(directory, copy, { recursive: true });
    return copy;
  });
  const intact = replayAgainst(copies);
  if (intact.integrity !== 'artifact-intact') {
    throw new Error(`the untouched copy is not intact: ${intact.detail}`);
  }
  const flipTarget = copies.flatMap(oleanFiles)
    .find((rel) => rel.endsWith('NobleFoundation.olean'));
  if (flipTarget === undefined) {
    throw new Error('the copy has no NobleFoundation olean to control against');
  }
  const bytes = fs.readFileSync(flipTarget);
  const position = Math.floor(bytes.length / 2);
  bytes[position] ^= 0x01;
  fs.writeFileSync(flipTarget, bytes);
  const flipped = replayAgainst(copies);
  const removeTarget = copies.flatMap(oleanFiles)[0];
  fs.rmSync(removeTarget);
  const removed = replayAgainst(copies);
  // Rebuild the copy so an extra artifact is observed as *extra* rather than behind the removal.
  directories.forEach((directory, index) => {
    fs.rmSync(copies[index], { recursive: true, force: true });
    fs.cpSync(directory, copies[index], { recursive: true });
  });
  fs.writeFileSync(path.join(copies[0], 'Extra.olean'), 'not an olean');
  const extra = replayAgainst(copies);
  fs.rmSync(temporary, { recursive: true, force: true });
  return {
    intact,
    controls: [
      { name: 'single-byte-flip', target: path.relative(root, flipTarget), position, result: flipped },
      { name: 'removed-artifact', target: path.relative(root, removeTarget), result: removed },
      { name: 'unexpected-artifact', result: extra },
    ],
  };
}

if (flag('--freeze')) {
  console.log(JSON.stringify(freeze()));
} else if (flag('--controls')) {
  console.log(JSON.stringify(controls(), null, 2));
} else {
  console.log(JSON.stringify({
    ...replay(searchPath()),
    source_commit: spawnSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).stdout.trim(),
    project: path.relative(root, project),
    module: moduleName,
    manifest: path.relative(root, manifestPath),
    host: `${os.type()} ${os.release()} ${os.arch()}`,
  }));
}
