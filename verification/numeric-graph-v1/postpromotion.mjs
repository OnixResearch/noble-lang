#!/usr/bin/env node
// Bounded Numeric-Graph-v1 lifecycle verification. No build, historical review,
// gate, proof, sync, archive, commit or publication is executed by this runner.
// SELECTED_NODE postpromotion.mjs plan|run --anchor COMMIT
//   --acceptance ORIGINAL_EXTERNAL_ACCEPTANCE_JSON
//   --review ORIGINAL_EXTERNAL_REVIEW_RECEIPT_JSON --out NEW_EXTERNAL_DIR
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { parseArgs } from 'node:util';

const root = fs.realpathSync(path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..'));
const PROTECTED_ROOTS = ['/home/brittonr/git/noble', '/home/brittonr/.cache/noble-issues/wt/ngraph-phase7'];
const ANCHOR = '16fd5369a6625d1d166a25190bbb02532832432a';
const PUBLISHED = '0eb5c0478ebf3419de6f94b8144c2ac7520356ca';
const TESTED = 'bc10df848ed81d53e849c440c1f639979e40d38d';
const SOURCE = 'sha256:2f4ebb1dc161687fc9844e9ab69cb1f992caf27bd6fd6ecd83aa0ff9d30f6f9f';
const REVISION = '0.1.0-draft.5';
const RUNNER = 'verification/numeric-graph-v1/postpromotion.mjs';
const ACTIVE = '.cairn/changes/numeric-graph-v1';
const OLD_CASES = `${ACTIVE}/cases/numeric-graph-cases.json`;
const CASES = 'specs/conformance/numeric-graph-cases.json';
const V = 'verification/numeric-graph-v1/';
const PASS = ['01', '06', '07', '08', '10', '13', '14', '15'].map(n => `NGRAPH-CASE-${n}`);
const HOLD = ['02', '03', '04', '05', '09', '11', '12'].map(n => `NGRAPH-CASE-${n}`);
const ALL_CASES = Array.from({ length: 15 }, (_, i) => `NGRAPH-CASE-${String(i + 1).padStart(2, '0')}`);
const HELD_STATE = { implementation: 'absent', execution: 'not-run', proof: 'open', trust: 'unassessed' };
const PASSED_STATE = { implementation: 'implemented', execution: 'passed', proof: 'open', trust: 'explicit' };
const BUILD_SCOPE = ['crates', 'Cargo.toml', 'Cargo.lock', 'policy', 'nix', 'flake.nix', 'flake.lock', 'dylint.toml'];
const ALLOWLIST = [
  'README.md', 'verification/main-gates/README.md', `${V}README.md`, RUNNER,
  `${ACTIVE}/design.md`, `${ACTIVE}/tasks.md`,
  ...['language', 'developer-experience', 'evidence'].map(id => `${ACTIVE}/specs/${id}/spec.md`),
  OLD_CASES, CASES, 'specs/spec-family.json', 'specs/STATUS.json', 'specs/ROADMAP.md',
  'specs/requirements.json', 'specs/VALIDATION.json',
  ...['language', 'developer-experience', 'evidence'].map(id => `.cairn/specs/${id}/spec.md`),
  ...['safety', 'verification', 'backend-experiments'].map(id => `.cairn/specs/${id}/spec.md`),
  'specs/SPEC-0001.md', 'specs/DEVELOPER-EXPERIENCE.md', 'specs/EVIDENCE.md',
].sort();
const allowed = new Set(ALLOWLIST);
// These are hard pins as well as anchor comparisons; an allowlist cannot turn
// immutable source evidence into a document that may be rewritten.
const IMMUTABLE = {
  [`${V}acceptance.json`]: 'cace59bc617e30b2a763b0298402e613115823eeec79aeae21d746571c5273a2',
  [`${V}build-provenance.json`]: 'b7d618b65fc84f57d2a8cee6779911b8d043b6e2d5127b7639dd6fad6cd76f7b',
  [`${V}prepromotion-cases.json`]: '96150208b0f6984919a7016ef4f2b825657e8a4ab809eaaf537fd11dd654b545',
  [`${V}review.json`]: '3596ce7ac2cc8bdda428bf4d30b8438ccd17b71976ef151ca9f40afdb590b563',
  [`${V}review.mjs`]: 'd4d6f5f11d81491cbabf354309d659bb5591987b1118213d187c5fa2bc54a73c',
  [`${V}review-receipt.json`]: '1d6a67e11b5142d35b971ac9645e0328d2944eeee1fd28e87c5a63b6586c37b0',
  [`${V}gate-deltas.json`]: '7812550ca0d19f545a49ae33b53055c66a32ade16505cf39079d7fd98d738050',
  'verification/main-gates/receipt.json': '4e0b3536e285a8867dc0ca64993f75c872da2e1bee0ceae1e68bb577de19708d',
};
const ORIGINAL_ACCEPTANCE = '/home/brittonr/.cache/noble-issues/proof/ngraph-acceptance-20261016-final/acceptance/acceptance.json';
const ORIGINAL_REVIEW = '/home/brittonr/.cache/noble-issues/proof/ngraph-independent-claims-20261016-bound/review-receipt.json';
const RUNTIME = '/home/brittonr/.cache/noble-issues/proof/ngraph-independent-claims-20261016-core/runtime.json';
const git = '/run/current-system/sw/bin/git';
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const stable = value => Array.isArray(value) ? value.map(stable) : value && typeof value === 'object'
  ? Object.fromEntries(Object.keys(value).sort().map(key => [key, stable(value[key])])) : value;
const canonical = value => JSON.stringify(stable(value));
const digest = value => sha(Buffer.from(canonical(value)));
const file = name => path.join(root, name);
const split = value => value.toString().split('\0').filter(Boolean);
const within = (child, parent) => child === parent || child.startsWith(`${parent}${path.sep}`);
const inScope = name => BUILD_SCOPE.some(scope => name === scope || name.startsWith(`${scope}/`));
const externalFiles = new Map();

function relative(name) {
  assert.equal(typeof name, 'string');
  assert.ok(name && !path.isAbsolute(name) && !/[\0\r\n\\]/.test(name)
    && name.split('/').every(part => part && part !== '.' && part !== '..'), `unsafe relative path: ${name}`);
  return name;
}
function plainBytes(name) {
  const stat = fs.lstatSync(name);
  assert.ok(stat.isFile(), `plain file required: ${name}`);
  assert.equal(fs.realpathSync(name), path.resolve(name), `symlinked file or parent: ${name}`);
  const bytes = fs.readFileSync(name);
  assert.equal(bytes.length, stat.size, `file changed while reading: ${name}`);
  return bytes;
}
function load(name) { return JSON.parse(plainBytes(name)); }
function gitCall(args) {
  const result = spawnSync(git, ['-c', 'core.fsmonitor=false', '-c', 'core.untrackedCache=false', '-C', root, ...args], {
    encoding: null, maxBuffer: 64 * 1024 * 1024, timeout: 30_000,
    env: { PATH: '/run/current-system/sw/bin', LANG: 'C', LC_ALL: 'C',
      GIT_OPTIONAL_LOCKS: '0', GIT_CONFIG_NOSYSTEM: '1', GIT_CONFIG_GLOBAL: '/dev/null', GIT_NO_REPLACE_OBJECTS: '1' },
  });
  assert.equal(result.error, undefined, `git ${args[0]}: launch failure`);
  assert.equal(result.signal, null, `git ${args[0]}: terminated`);
  assert.equal(result.status, 0, `git ${args[0]}: ${result.stderr}`);
  return result.stdout;
}
const gitText = args => gitCall(args).toString().trim();
const anchorBytes = name => gitCall(['show', `${ANCHOR}:${relative(name)}`]);
function tree(revision, scope = []) {
  return split(gitCall(['ls-tree', '-rz', '--full-tree', revision, '--', ...scope])).map(entry => {
    const match = /^(\d+) blob ([0-9a-f]+)\t(.+)$/s.exec(entry);
    assert.ok(match, `unsupported anchor tree entry: ${entry}`);
    return { path: relative(match[3]), mode: match[1], oid: match[2] };
  }).sort((a, b) => a.path < b.path ? -1 : a.path > b.path ? 1 : 0);
}
function worktreeBytes(name) {
  const target = file(relative(name)), stat = fs.lstatSync(target);
  if (stat.isSymbolicLink()) return { bytes: Buffer.from(fs.readlinkSync(target)), kind: 'symlink', mode: '120000' };
  assert.ok(stat.isFile(), `unsupported worktree entry: ${name}`);
  assert.equal(fs.realpathSync(path.dirname(target)), path.dirname(target), `symlinked worktree parent: ${name}`);
  return { bytes: fs.readFileSync(target), kind: 'file', mode: stat.mode & 0o111 ? '100755' : '100644' };
}
function boundExternal(name, expected, size) {
  assert.match(expected, /^[0-9a-f]{64}$/, `missing expected SHA-256: ${name}`);
  const absolute = path.resolve(name), bytes = plainBytes(absolute), actual = sha(bytes);
  assert.equal(actual, expected, `external evidence changed: ${absolute}`);
  if (size !== undefined) assert.equal(bytes.length, size, `external evidence size changed: ${absolute}`);
  const row = { path: absolute, sha256: actual, bytes: bytes.length };
  if (externalFiles.has(absolute)) assert.deepEqual(row, externalFiles.get(absolute), `evidence drift: ${absolute}`);
  externalFiles.set(absolute, row);
  return bytes;
}
function artifact(base, name, row) {
  const target = path.join(base, relative(name));
  assert.ok(within(target, base), `artifact escaped packet: ${name}`);
  return boundExternal(target, row.sha256, row.bytes);
}
function walkPlain(base, prefix = '') {
  const result = [];
  for (const entry of fs.readdirSync(path.join(base, prefix), { withFileTypes: true })) {
    const name = prefix ? `${prefix}/${entry.name}` : entry.name;
    relative(name);
    if (entry.isDirectory()) result.push(...walkPlain(base, name));
    else { assert.ok(entry.isFile(), `non-plain packet artifact: ${path.join(base, name)}`); result.push(name); }
  }
  return result.sort();
}
function captureSource() {
  const rawIndex = gitCall(['ls-files', '--stage', '-z']);
  const objectFormat = gitText(['rev-parse', '--show-object-format']);
  assert.ok(['sha1', 'sha256'].includes(objectFormat), 'unsupported Git object format');
  const rows = split(rawIndex).map(entry => {
    const match = /^(\d+) ([0-9a-f]+) ([0-3])\t(.+)$/s.exec(entry);
    assert.ok(match && match[3] === '0', `unmerged or malformed index row: ${entry}`);
    const name = relative(match[4]), row = { path: name, index: { mode: match[1], oid: match[2] } };
    try {
      const current = worktreeBytes(name);
      Object.assign(row, { kind: current.kind, worktree_mode: current.mode,
        worktree_sha256: sha(current.bytes), worktree_bytes: current.bytes.length,
        worktree_oid: crypto.createHash(objectFormat).update(`blob ${current.bytes.length}\0`).update(current.bytes).digest('hex') });
    } catch (error) {
      if (error.code !== 'ENOENT') throw error;
      row.missing = true;
    }
    return row;
  }).sort((a, b) => a.path < b.path ? -1 : a.path > b.path ? 1 : 0);
  assert.equal(new Set(rows.map(row => row.path)).size, rows.length, 'duplicate staged paths');
  const untracked = split(gitCall(['ls-files', '--others', '-z'])).sort(); // Includes ignored files: no blanket exemption.
  const untrackedRows = untracked.map(name => {
    const current = worktreeBytes(name);
    return { path: name, kind: current.kind, worktree_mode: current.mode,
      worktree_sha256: sha(current.bytes), worktree_bytes: current.bytes.length };
  });
  return { head: gitText(['rev-parse', 'HEAD']), branch: gitText(['branch', '--show-current']),
    object_format: objectFormat, index_manifest_sha256: sha(rawIndex),
    index_flags: split(gitCall(['ls-files', '-v', '-z'])), rows, untracked: untrackedRows,
    status: split(gitCall(['status', '--porcelain=v1', '-z', '--untracked-files=all'])),
    staged_changes: split(gitCall(['diff', '--cached', '--no-ext-diff', '--no-renames', '--name-only', '-z', ANCHOR])),
    unstaged_changes: split(gitCall(['diff', '--no-ext-diff', '--no-renames', '--name-only', '-z'])) };
}
function changedPaths(source, anchorRows) {
  const before = new Map(anchorRows.map(row => [row.path, row]));
  const after = new Map(source.rows.map(row => [row.path, row]));
  return [...new Set([
    ...source.staged_changes, ...source.unstaged_changes, ...source.untracked.map(row => row.path),
    ...anchorRows.filter(row => !after.has(row.path)).map(row => row.path),
    ...source.rows.filter(row => {
      const old = before.get(row.path);
      return !old || row.missing || row.index.mode !== old.mode || row.index.oid !== old.oid
        || row.worktree_mode !== old.mode || row.worktree_oid !== old.oid;
    }).map(row => row.path),
  ])].sort();
}
function requireSubset(actual, expected, label) {
  assert.deepEqual(actual.filter(name => !expected.has(name)), [], label);
}
function requirementBlocks(markdown) {
  const starts = [];
  let fence;
  let offset = 0;
  for (const line of markdown.split('\n')) {
    const marker = line.match(/^\s*(`{3,}|~{3,})(.*)$/);
    if (fence) { if (marker && marker[1][0] === fence[0] && marker[1].length >= fence.length && !marker[2].trim()) fence = undefined; }
    else if (marker) fence = marker[1];
    else {
      const heading = /^### Requirement: ([A-Z][A-Z0-9-]*-\d+)$/.exec(line);
      if (heading) starts.push({ id: heading[1], offset });
    }
    offset += line.length + 1;
  }
  assert.equal(fence, undefined, 'unclosed requirement fence');
  return { prefix: markdown.slice(0, starts[0]?.offset ?? markdown.length),
    rows: starts.map((start, i) => ({ id: start.id, text: markdown.slice(start.offset, starts[i + 1]?.offset) })) };
}
const withoutGenerated = text => text.replace(/\n?<!-- cairn:scenario-links:start -->[\s\S]*?<!-- cairn:scenario-links:end -->\n?/g, '').trim();
const withoutScenarioContents = text => text.replace(/(<!-- cairn:scenario-links:start -->)[\s\S]*?(<!-- cairn:scenario-links:end -->)/g, '$1$2');
const normativeHead = text => withoutGenerated(text).split(/^#### Scenario:/m)[0].trim();

async function main() {
  const [mode, ...args] = process.argv.slice(2);
  assert.ok(['plan', 'run'].includes(mode), 'usage: SELECTED_NODE postpromotion.mjs plan|run --anchor COMMIT --acceptance ORIGINAL_EXTERNAL_ACCEPTANCE_JSON --review ORIGINAL_EXTERNAL_REVIEW_RECEIPT_JSON --out NEW_EXTERNAL_DIR');
  const { values } = parseArgs({ args, options: Object.fromEntries(['anchor', 'acceptance', 'review', 'out'].map(key => [key, { type: 'string' }])), strict: true });
  for (const key of ['anchor', 'acceptance', 'review', 'out']) {
    assert.ok(values[key], `--${key} is required`);
    assert.equal(args.filter(arg => arg === `--${key}` || arg.startsWith(`--${key}=`)).length, 1, `duplicate --${key}`);
  }
  const out = path.resolve(values.out);
  assert.ok(!fs.existsSync(out), 'new external output directory required, including in plan mode');
  assert.equal(fs.realpathSync(path.dirname(out)), path.dirname(out), 'output parent must exist and cannot be symlinked');
  const forbiddenOutputs = [root, ...PROTECTED_ROOTS,
    '/home/brittonr/.cache/noble-issues/proof/ngraph-acceptance-20261016-final', path.dirname(ORIGINAL_REVIEW), path.dirname(RUNTIME)];
  for (const forbidden of forbiddenOutputs) assert.ok(!within(out, forbidden), `output would modify protected source/evidence: ${forbidden}`);
  // All registered worktrees are read-only to output publication, not only this one.
  const registeredWorktrees = gitCall(['worktree', 'list', '--porcelain']).toString().split('\n')
    .filter(line => line.startsWith('worktree ')).map(line => line.slice(9));
  for (const registered of registeredWorktrees)
    assert.ok(!within(out, registered), 'external output cannot be in any registered worktree');

  const report = { schema: 'noble-numeric-graph-v1-postpromotion/v1', phase: 'bounded-post-promotion-lifecycle',
    result: 'failed', recorded_at: new Date().toISOString(), root, branch: gitText(['branch', '--show-current']), anchor: values.anchor, artifact_directory: out,
    source_revision: SOURCE, reviewed_allowlist: ALLOWLIST, changed_paths: [], checks: [],
    failures: [], integrity_failures: [], ng_po_discharged: [],
    non_claims: ['Finite receipt/source/document verification on one pinned x86_64 Linux platform, not new execution acceptance.',
      'No historical review, build, Rust test or gate is rerun. Historical full14 remains10PASS4knownFAIL.',
      'No proof, performance, speedup, GPU, backend-selection or cross-platform claim.',
      'No archive, commit, push, posting or original-worktree mutation. Canonical sync is performed by the caller, not this runner.'] };
  const check = async (label, action, integrity = true) => {
    try { const value = await action(); report.checks.push({ check: label, result: 'passed' }); return value; }
    catch (error) {
      const row = { check: label, error: String(error.message ?? error) };
      (integrity ? report.integrity_failures : report.failures).push(row);
      report.checks.push({ check: label, result: 'failed' });
      return undefined;
    }
  };
  let outputCreated = false, source, anchorRows, projection;
  const outputFiles = [];
  function save(name, bytes) {
    relative(name);
    const target = path.join(out, name);
    fs.mkdirSync(path.dirname(target), { recursive: true, mode: 0o700 });
    fs.writeFileSync(target, bytes, { flag: 'wx', mode: 0o444 });
    const row = { path: name, sha256: sha(bytes), bytes: Buffer.byteLength(bytes) };
    outputFiles.push(row);
    return row;
  }
  function saveJSON(name, value) { return save(name, Buffer.from(JSON.stringify(value, null, 2) + '\n')); }
  function sealDirectory(name) {
    for (const entry of fs.readdirSync(name, { withFileTypes: true })) {
      const child = path.join(name, entry.name);
      if (entry.isDirectory()) sealDirectory(child);
      else { assert.ok(entry.isFile(), `non-file output: ${child}`); fs.chmodSync(child, 0o444); }
    }
    fs.chmodSync(name, 0o555);
  }
  try {
    if (mode === 'run') { fs.mkdirSync(out, { mode: 0o700 }); outputCreated = true; }
    const identity = await check('registered worktree exact anchor and policy-selected Node', () => {
      assert.ok(registeredWorktrees.includes(root), 'runner must belong to a registered worktree');
      assert.ok(!PROTECTED_ROOTS.some(protectedRoot => within(root, protectedRoot)), 'original checkout and publication worktree are excluded');
      assert.equal(values.anchor, ANCHOR, 'the independently audited exact anchor is mandatory');
      assert.equal(gitText(['rev-parse', '--verify', `${values.anchor}^{commit}`]), ANCHOR);
      assert.equal(gitText(['rev-parse', '--show-toplevel']), root);
      assert.equal(gitText(['rev-parse', 'HEAD']), ANCHOR, 'worktree must remain uncommitted at the audit anchor');
      assert.deepEqual(process.execArgv, [], 'Node execution overrides are not allowed');
      assert.ok(!process.env.NODE_OPTIONS, 'NODE_OPTIONS is not allowed');
      for (const key of ['GIT_DIR', 'GIT_WORK_TREE', 'GIT_INDEX_FILE', 'GIT_COMMON_DIR', 'GIT_OBJECT_DIRECTORY', 'GIT_ALTERNATE_OBJECT_DIRECTORIES', 'GIT_NAMESPACE'])
        assert.ok(!process.env[key], `${key} is not allowed`);
      assert.equal(process.platform, 'linux'); assert.equal(process.arch, 'x64');
      const selectedBytes = anchorBytes('policy/tool-selection.json');
      assert.deepEqual(plainBytes(file('policy/tool-selection.json')), selectedBytes, 'tool-selection policy changed');
      const selected = JSON.parse(selectedBytes);
      const selectedNode = fs.realpathSync(path.join(selected.tool_paths.node.output, 'bin/node'));
      assert.equal(fs.realpathSync(process.execPath), selectedNode, 'policy-selected Node required');
      return { path: selectedNode, sha256: sha(plainBytes(selectedNode)), version: process.version };
    });
    if (!identity) throw new Error('source identity prerequisite failed');
    report.node = identity;
    anchorRows = tree(ANCHOR);
    source = captureSource();
    report.changed_paths = changedPaths(source, anchorRows);
    report.source = { anchor_tree: gitText(['rev-parse', `${ANCHOR}^{tree}`]),
      staged_files: source.rows.length, source_snapshot_sha256: digest(source),
      runner: { path: RUNNER, sha256: sha(plainBytes(file(RUNNER))) } };
    await check('whole staged/worktree identity and exact changed-path allowlist', () => {
      assert.equal(source.head, ANCHOR); assert.equal(source.branch, report.branch);
      requireSubset(report.changed_paths, allowed, 'unreviewed changed paths');
      assert.deepEqual(source.untracked, [], 'all files must be staged; ignored/untracked files are not exempt');
      assert.deepEqual(source.unstaged_changes, [], 'unstaged worktree changes');
      for (const row of source.rows) {
        assert.ok(!row.missing, `tracked file missing: ${row.path}`);
        assert.equal(row.worktree_mode, row.index.mode, `staged/worktree mode mismatch: ${row.path}`);
        assert.equal(row.worktree_oid, row.index.oid, `staged/worktree bytes mismatch: ${row.path}`);
      }
      const before = new Map(anchorRows.map(row => [row.path, row]));
      const after = new Map(source.rows.map(row => [row.path, row]));
      for (const old of anchorRows) {
        if (allowed.has(old.path)) continue;
        const row = after.get(old.path);
        assert.ok(row, `unreviewed tracked deletion: ${old.path}`);
        assert.deepEqual(row.index, { mode: old.mode, oid: old.oid }, `unreviewed staged source change: ${old.path}`);
        assert.equal(row.worktree_oid, old.oid, `unreviewed worktree source change: ${old.path}`);
        assert.equal(row.worktree_mode, old.mode, `unreviewed source mode change: ${old.path}`);
      }
      for (const row of source.rows.filter(row => allowed.has(row.path))) {
        assert.equal(row.kind, 'file', `reviewed documents cannot be symlinks: ${row.path}`);
        assert.equal(row.index.mode, before.get(row.path)?.mode ?? (row.path === RUNNER ? '100755' : '100644'), `unreviewed document mode: ${row.path}`);
      }
      for (const name of [CASES, RUNNER, `${ACTIVE}/metadata.json`, `${ACTIVE}/proposal.md`, `${ACTIVE}/tasks.md`])
        assert.ok(after.has(name), `required active/promoted path not staged: ${name}`);
      assert.ok(!after.has(OLD_CASES) && !fs.existsSync(file(OLD_CASES)), 'old case path must be deleted, not copied');
      for (const immutable of Object.keys(IMMUTABLE)) assert.ok(!allowed.has(immutable));
      assert.ok(ALLOWLIST.every(name => !inScope(name)), 'reviewed allowlist overlaps immutable build inputs');
    });
    await check('existing gitSourceProjection frozen source binding', async () => {
      const helper = 'verification/m8/source-projection.mjs';
      assert.deepEqual(plainBytes(file(helper)), anchorBytes(helper), 'source projection helper changed');
      const { gitSourceProjection } = await import(pathToFileURL(file(helper)).href);
      projection = gitSourceProjection(root, source.rows.map(row => row.path), true);
      for (const row of projection.rows) {
        const own = source.rows.find(item => item.path === row.path);
        assert.deepEqual(row.index, own.index);
        assert.equal(row.worktree_sha256, own.worktree_sha256);
        assert.equal(row.worktree_mode, own.worktree_mode);
      }
    });

    const packets = await check('anchor-pinned immutable packets and original external receipts', () => {
      const documents = {};
      for (const [name, expected] of Object.entries(IMMUTABLE)) {
        const original = anchorBytes(name), current = plainBytes(file(name));
        assert.equal(sha(original), expected, `unexpected anchor packet: ${name}`);
        assert.deepEqual(current, original, `immutable packet changed: ${name}`);
        if (name.endsWith('.json')) documents[name] = JSON.parse(original);
      }
      assert.equal(path.resolve(values.acceptance), ORIGINAL_ACCEPTANCE, 'original external acceptance receipt required');
      assert.equal(path.resolve(values.review), ORIGINAL_REVIEW, 'original external review receipt required');
      boundExternal(ORIGINAL_ACCEPTANCE, IMMUTABLE[`${V}acceptance.json`]);
      boundExternal(ORIGINAL_REVIEW, IMMUTABLE[`${V}review-receipt.json`]);
      return { acceptance: documents[`${V}acceptance.json`], review: documents[`${V}review.json`],
        receipt: documents[`${V}review-receipt.json`], provenance: documents[`${V}build-provenance.json`],
        custody: documents[`${V}prepromotion-cases.json`], gates: documents['verification/main-gates/receipt.json'],
        deltas: documents[`${V}gate-deltas.json`] };
    });
    assert.ok(packets, 'immutable packet prerequisites failed');
    const { acceptance: acceptance, review, receipt, provenance, custody, gates, deltas } = packets;
    report.immutable_packets = Object.entries(IMMUTABLE).map(([name, sha256]) => ({ path: name, sha256 }));
    await check('acceptance and review verdict/source bindings', () => {
      assert.equal(acceptance.schema, 'noble-numeric-graph-v1-acceptance/v1');
      assert.equal(acceptance.result, 'passed'); assert.deepEqual(acceptance.failures, []); assert.deepEqual(acceptance.integrity_failures, []);
      assert.equal(acceptance.source_revision, SOURCE); assert.equal(acceptance.source.frozen, true);
      assert.equal(acceptance.source.head, PUBLISHED); assert.equal(acceptance.tested_commit, TESTED);
      assert.equal(acceptance.source.built_sources_equal_tested_commit, true);
      assert.deepEqual(acceptance.source.built_sources_differing, []);
      assert.equal(`sha256:${acceptance.source.projection.sha256}`, SOURCE);
      for (const [name, expected] of Object.entries(acceptance.source.files))
        assert.equal(sha(anchorBytes(name)), expected, `accepted source differs at audit anchor: ${name}`);
      assert.equal(receipt.schema, 'noble-numeric-graph-v1-review-receipt/v1');
      assert.equal(receipt.result, 'passed'); assert.deepEqual(receipt.failures, []);
      assert.equal(receipt.source_revision, SOURCE); assert.equal(receipt.reviewed_publication, PUBLISHED); assert.equal(receipt.tested_commit, TESTED);
      assert.equal(receipt.acceptance.sha256, IMMUTABLE[`${V}acceptance.json`]);
      assert.equal(receipt.review.sha256, IMMUTABLE[`${V}review.json`]);
      assert.equal(receipt.runner.sha256, IMMUTABLE[`${V}review.mjs`]);
      assert.equal(identity.sha256, receipt.tools.node.sha256, 'selected Node binary differs from independent review');
      assert.equal(identity.sha256, acceptance.tools.node.sha256, 'selected Node binary differs from original acceptance');
      assert.equal(receipt.build_provenance.sha256, IMMUTABLE[`${V}build-provenance.json`]);
      assert.equal(receipt.prepromotion_cases.sha256, IMMUTABLE[`${V}prepromotion-cases.json`]);
      assert.equal(receipt.external_output, path.dirname(ORIGINAL_REVIEW));
      assert.deepEqual(receipt.verdicts, { case01: 'passed', case15: 'passed', passed: PASS, held: HOLD });
      assert.deepEqual(review.final_recommendation.passed, PASS); assert.deepEqual(review.final_recommendation.held, HOLD);
      assert.equal(review.case01.verdict, 'passed'); assert.equal(review.case15.verdict, 'passed'); assert.equal(review.case08.verdict, 'passed');
      assert.deepEqual(review.ng_po_discharged, []); assert.deepEqual(receipt.ng_po_discharged, []);
      assert.ok(Object.values(review.scope).every(value => value === false), 'immutable review has unexpected authority scope');
      assert.equal(review.acceptance.source_revision, SOURCE);
      assert.equal(provenance.acceptance.source_revision, SOURCE);
      for (const [name, row] of Object.entries(review.reviewed_sources)) {
        const old = anchorBytes(name), hash = sha(old);
        assert.ok([row.sha256, row.schema_only_sha256].includes(hash), `reviewed source at anchor differs: ${name}`);
        if (hash === row.sha256) assert.equal(old.length, row.bytes, `reviewed anchor size: ${name}`);
      }
      // The reviewed README is deliberately read from the anchor, never from
      // its post-promotion replacement. Do not execute historical review.mjs.
      report.reviewed_anchor_sources = Object.entries(review.reviewed_sources).map(([name]) => ({ path: name, sha256: sha(anchorBytes(name)) }));
      for (const [key, id] of [['case01', 'NGRAPH-CASE-01'], ['case15', 'NGRAPH-CASE-15']]) {
        const row = custody.cases.find(item => item.case_id === id).record;
        for (const field of review[key].fields) {
          assert.equal(field.verdict, 'passed');
          assert.deepEqual(field.path.reduce((value, key) => value?.[key], row.expected), field.expected, `${id}: reviewed expected field changed`);
        }
      }
    });
    await check('all1245 original acceptance raw artifact hashes', () => {
      const raw = acceptance.external_raw_output;
      assert.equal(path.join(raw.directory, raw.receipt_copy), ORIGINAL_ACCEPTANCE);
      assert.deepEqual(raw, provenance.acceptance.raw);
      assert.equal(raw.files, 1245); assert.equal(receipt.acceptance.checked_raw_files, 1245);
      assert.equal(raw.manifest_sha256, receipt.acceptance.manifest_sha256);
      const manifest = JSON.parse(boundExternal(path.join(raw.directory, relative(raw.manifest)), raw.manifest_sha256));
      assert.equal(Object.keys(manifest).length, 1245);
      let total = 0;
      for (const [name, row] of Object.entries(manifest)) { artifact(raw.directory, name, row); total += row.bytes; }
      assert.equal(total, raw.bytes);
      assert.deepEqual(walkPlain(raw.directory), [...Object.keys(manifest), raw.manifest, raw.receipt_copy].sort(), 'unexpected or missing original raw artifacts');
      const plan = JSON.parse(artifact(raw.directory, acceptance.plan.file, { sha256: acceptance.plan.sha256 }));
      assert.equal(plan.cases.workload.length, 15);
      assert.deepEqual(plan.cases.workload.map(row => row.id).sort(), ALL_CASES);
      assert.equal(acceptance.plan.case_file_sha256, custody.prepromotion_case_file_sha256);
      const pairs = new Map();
      for (const command of acceptance.commands) {
        artifact(raw.directory, command.stdout, { sha256: command.stdout_sha256, bytes: command.stdout_bytes });
        artifact(raw.directory, command.stderr, { sha256: command.stderr_sha256, bytes: command.stderr_bytes });
        assert.equal(command.signal, null); assert.equal(command.error, null);
        if (!command.repetition) continue;
        const key = canonical([command.scenario, command.step]), row = pairs.get(key) ?? {};
        assert.ok(!row[command.repetition], `duplicate repetition: ${key}`);
        row[command.repetition] = command; pairs.set(key, row);
      }
      assert.equal(pairs.size, 100);
      for (const [key, pair] of pairs) {
        assert.ok(pair[1] && pair[2], `missing repetition: ${key}`);
        for (const field of ['stdout_sha256', 'stderr_sha256', 'exit']) assert.equal(pair[1][field], pair[2][field], `${key}: ${field}`);
      }
      assert.equal(acceptance.determinism.result, 'passed'); assert.deepEqual(acceptance.determinism.mismatches, []);
      assert.equal(acceptance.core_identity.rows.length, 31); assert.equal(acceptance.core_identity.result, 'passed');
      assert.ok(acceptance.core_identity.rows.every(row => row.tested_expectation === 'passed' && Object.values(row.identical).every(value => value === true)));
      boundExternal(path.join(path.dirname(ORIGINAL_REVIEW), 'help.stdout'), receipt.observed.help.stdout_sha256);
      boundExternal(path.join(path.dirname(ORIGINAL_REVIEW), 'help.stderr'), receipt.observed.help.stderr_sha256);
      report.acceptance_raw = { directory: raw.directory, manifest_sha256: raw.manifest_sha256, files: 1245, bytes: total, repeated_command_pairs: pairs.size };
    });
    await check('fresh binaries, distinct build logs, A4 and case custody', () => {
      assert.equal(provenance.schema, 'noble-numeric-graph-v1-build-provenance/v1');
      assert.equal(provenance.actual_release_build_revision, PUBLISHED); assert.equal(provenance.tested_gate_revision, TESTED);
      for (const [side, tool] of [['current', acceptance.tools.noble], ['base', acceptance.tools.noble_base]]) {
        const built = provenance.builds[side];
        assert.equal(built.sha256, tool.sha256); assert.equal(built.exit, 0);
        assert.notEqual(built.sha256, built.log_sha256, 'binary and build-log hashes must not be conflated');
        boundExternal(built.file, built.sha256, built.bytes);
        boundExternal(built.log, built.log_sha256);
        artifact(acceptance.external_raw_output.directory, tool.frozen, { sha256: tool.sha256, bytes: tool.bytes });
      }
      assert.equal(receipt.tools.binary.sha256, acceptance.tools.noble.sha256);
      boundExternal(receipt.tools.binary.file, receipt.tools.binary.sha256);
      assert.equal(provenance.soft_float.sha256, acceptance.soft_float.sha256);
      assert.equal(provenance.soft_float.exit, 0); assert.equal(provenance.soft_float.passed, 1); assert.equal(provenance.soft_float.failed, 0);
      const soft = boundExternal(provenance.soft_float.log, provenance.soft_float.sha256, acceptance.soft_float.bytes).toString();
      assert.equal(soft.split('\n').filter(line => /^test .*ten_million_bit_patterns_match_std \.\.\. ok$/.test(line)).length, 1, 'missing original A4 success row');
      for (const key of ['plan_command', 'run_command']) {
        const row = provenance.acceptance[key]; assert.equal(row.exit, 0); boundExternal(row.log, row.sha256);
      }
      const completion = provenance.schema_completion;
      const original = JSON.parse(boundExternal(completion.original, completion.original_sha256));
      const schema = JSON.parse(boundExternal(completion.record, completion.record_sha256));
      assert.equal(schema.original_sha256, completion.original_sha256);
      assert.equal(schema.prepromotion_case_sha256, custody.prepromotion_case_file_sha256);
      assert.equal(sha(anchorBytes(OLD_CASES)), custody.prepromotion_case_file_sha256);
      assert.equal(custody.schema, 'noble-ngraph-prepromotion-case-records/v1');
      assert.equal(custody.cases.length, 15);
      assert.deepEqual(custody.cases.map(row => row.case_id).sort(), ALL_CASES);
      const unpromoted = JSON.parse(anchorBytes(OLD_CASES));
      for (const item of custody.cases) {
        assert.equal(digest(item.record), item.prepromotion_case_sha256, `${item.case_id}: canonical custody hash`);
        assert.deepEqual(item.record, unpromoted.cases.find(row => row.id === item.case_id));
        assert.deepEqual(item.record.state, HELD_STATE); assert.deepEqual(item.record.evidence, []);
        const old = original.cases.find(row => row.id === item.case_id);
        for (const key of ['id', 'profile', 'kind', 'requirements', 'input', 'state', 'evidence']) assert.deepEqual(item.record[key], old[key]);
      }
    });
    await check('complete707 published/tested/anchor/staged/worktree build projection', () => {
      const equivalence = provenance.source_equivalence;
      assert.deepEqual(equivalence.scope, BUILD_SCOPE); assert.equal(equivalence.files, 707); assert.equal(equivalence.manifests_byte_identical, true);
      const published = gitCall(['ls-tree', '-r', PUBLISHED, '--', ...equivalence.scope]);
      const tested = gitCall(['ls-tree', '-r', TESTED, '--', ...equivalence.scope]);
      const anchored = gitCall(['ls-tree', '-r', ANCHOR, '--', ...equivalence.scope]);
      assert.deepEqual(published, tested, 'full build/policy manifests differ between published and tested revisions');
      assert.deepEqual(published, anchored, 'anchor changed the complete accepted build inputs');
      assert.equal(sha(published), equivalence.git_tree_manifest_sha256);
      const expected = tree(PUBLISHED, equivalence.scope), current = source.rows.filter(row => inScope(row.path));
      assert.equal(expected.length, 707);
      assert.deepEqual(current.map(row => row.path), expected.map(row => row.path), 'new or deleted production/build/policy files');
      for (const [index, row] of current.entries()) {
        const old = expected[index];
        assert.deepEqual(row.index, { mode: old.mode, oid: old.oid }, `full707 staged drift: ${row.path}`);
        assert.equal(row.worktree_oid, old.oid, `full707 worktree drift: ${row.path}`);
        assert.equal(row.worktree_mode, old.mode, `full707 mode drift: ${row.path}`);
      }
      report.full_build_projection = { scope: equivalence.scope, files: 707,
        git_tree_manifest_sha256: sha(published), published: PUBLISHED, tested: TESTED,
        rows: current.map(row => ({ path: row.path, mode: row.index.mode, oid: row.index.oid, sha256: row.worktree_sha256 })) };
      if (mode === 'run') report.full_build_manifest = save('build-source-manifest.txt', published);
    });
    let runtime;
    await check('review packet and all246 supplemental runtime artifacts', () => {
      assert.equal(receipt.runtime.file, RUNTIME); assert.equal(review.runtime.path, RUNTIME);
      assert.equal(receipt.runtime.sha256, review.runtime.sha256);
      runtime = JSON.parse(boundExternal(RUNTIME, receipt.runtime.sha256, review.runtime.bytes));
      assert.equal(runtime.schema, 'noble-ngraph-independent-core-runtime/v1'); assert.equal(runtime.result, 'passed'); assert.deepEqual(runtime.failures, []);
      assert.equal(runtime.acceptance.sha256, IMMUTABLE[`${V}acceptance.json`]);
      assert.equal(runtime.driver.sha256, sha(anchorBytes('verification/m4/gate.mjs')));
      for (const tool of Object.values(runtime.tools)) boundExternal(tool.path, tool.sha256);
      assert.equal(receipt.runtime.checked_artifact_files, 246); assert.equal(receipt.runtime.extra_runtime_inputs_per_binary, 12);
      let artifacts = 0;
      for (const side of ['base', 'tested']) {
        const data = runtime.sides[side], dir = path.join(path.dirname(RUNTIME), side);
        const expectedBinary = side === 'base' ? acceptance.tools.noble_base : acceptance.tools.noble;
        assert.equal(data.binary.sha256, expectedBinary.sha256); boundExternal(data.binary.path, data.binary.sha256);
        for (const [name, row] of Object.entries(data.artifacts)) { artifact(dir, name, row); artifacts++; }
        assert.deepEqual(walkPlain(dir), Object.keys(data.artifacts).sort(), `${side}: unexpected runtime artifact`);
        for (const row of data.commands) {
          assert.equal(row.status, 0); assert.equal(row.signal, null); assert.equal(row.error, null);
          artifact(dir, row.stdout, { sha256: row.stdout_sha256 }); artifact(dir, row.stderr, { sha256: row.stderr_sha256 });
        }
        assert.equal(data.compiled_artifacts.length, 30);
        for (const row of data.compiled_artifacts) artifact(dir, row.name, row);
        assert.deepEqual(Object.keys(data.observations).sort(), ['CORE-03', 'CORE-05', 'CORE-09']);
        for (const [id, count] of [['CORE-03', 6], ['CORE-05', 4], ['CORE-09', 2]]) {
          assert.equal(data.observations[id].length, count);
          assert.equal(data.observations[id].filter(row => row.optimization === 'off').length, count / 2);
          assert.equal(data.observations[id].filter(row => row.optimization === 'on').length, count / 2);
          assert.ok(data.observations[id].every(row => row.compiler_service === 'absent' && row.stage === 'wasm'));
        }
      }
      assert.equal(artifacts, 246);
      for (const id of ['CORE-03', 'CORE-05', 'CORE-09'])
        assert.deepEqual(runtime.sides.base.observations[id], runtime.sides.tested.observations[id], `${id}: runtime observations differ`);
      assert.equal(runtime.compiled_artifact_comparison.result, 'passed');
      const compiled = side => runtime.sides[side].compiled_artifacts.map(row => canonical([path.basename(row.name), row.sha256, row.bytes])).sort();
      assert.deepEqual(compiled('base'), compiled('tested'));
      report.supplemental_runtime = { path: RUNTIME, sha256: receipt.runtime.sha256, artifacts,
        inputs_per_binary: 12, observations: { 'CORE-03': 6, 'CORE-05': 4, 'CORE-09': 2 } };
    });
    await check('immutable historical full14 red gates and raw logs', () => {
      assert.equal(gates.tested_commit, TESTED); assert.equal(gates.flake_checks.length, 14);
      assert.equal(gates.flake_checks.filter(row => row.result === 'pass').length, 10);
      assert.deepEqual(gates.flake_checks.filter(row => row.result === 'fail').map(row => row.check).sort(),
        ['boundary-controls', 'native-assurance', 'octet', 'source-coverage']);
      for (const gate of gates.flake_checks) {
        assert.equal(gate.exit, gate.result === 'pass' ? 0 : 1);
        boundExternal(gate.log, gate.log_sha256);
      }
      assert.equal(acceptance.library_tests.gate_receipt.sha256, IMMUTABLE['verification/main-gates/receipt.json']);
      assert.deepEqual(provenance.gate_binding, acceptance.library_tests);
      const rust = acceptance.library_tests.rust_tests;
      assert.equal(rust.commit, TESTED); assert.equal(rust.result, 'pass'); assert.equal(rust.exit, 0);
      assert.equal(rust.log_sha256, rust.observed_log_sha256);
      boundExternal(rust.log, rust.log_sha256);
      for (const test of acceptance.library_tests.tests) {
        assert.equal(test.result, 'passed'); assert.equal(test.ok_lines, 1); assert.equal(test.definitions_at_tested_commit, 1); assert.deepEqual(test.other_lines, []);
      }
      function bindRows(value) {
        if (!value || typeof value !== 'object') return;
        if (typeof value.path === 'string' && typeof value.sha256 === 'string') boundExternal(value.path, value.sha256, value.bytes);
        for (const item of Object.values(value)) if (item && typeof item === 'object') bindRows(item);
      }
      bindRows(deltas);
      assert.equal(deltas.architecture_findings.total, receipt.gate_deltas.architecture_findings);
      assert.equal(deltas.tigerstyle_static_lint.total, receipt.gate_deltas.warning_findings);
      report.historical_gates = { receipt_sha256: IMMUTABLE['verification/main-gates/receipt.json'], tested_commit: TESTED,
        total: 14, passed: 10, failed: 4, result: 'red', rerun: false };
    });

    const family = load(file('specs/spec-family.json'));
    const caseDocument = load(file(CASES));
    await check('single canonical case registration and exact8/7 promotions', () => {
      assert.equal(caseDocument.revision, REVISION); assert.equal(caseDocument.cases.length, 15);
      assert.deepEqual(caseDocument.cases.map(row => row.id), ALL_CASES);
      assert.equal(family.scenario_files.filter(name => name === 'conformance/numeric-graph-cases.json').length, 1);
      const oldFamily = JSON.parse(anchorBytes('specs/spec-family.json'));
      assert.deepEqual({ ...family, scenario_files: family.scenario_files.filter(name => name !== 'conformance/numeric-graph-cases.json') }, oldFamily,
        'unrelated family registration changed');
      const registered = family.scenario_files.flatMap(name => load(file(`specs/${relative(name)}`)).cases);
      for (const id of ALL_CASES) assert.equal(registered.filter(row => row.id === id).length, 1, `duplicate or missing registered case: ${id}`);
      for (const current of caseDocument.cases) {
        const item = custody.cases.find(row => row.case_id === current.id);
        const reconstructed = { ...current, state: item.record.state, evidence: item.record.evidence };
        assert.deepEqual(reconstructed, item.record, `${current.id}: input/expected/kind/profile/requirements or extra fields changed`);
        assert.equal(digest(reconstructed), item.prepromotion_case_sha256, `${current.id}: reconstructed custody record hash`);
        if (HOLD.includes(current.id)) {
          assert.deepEqual(current, item.record, `${current.id}: held row changed`);
          continue;
        }
        assert.deepEqual(current.state, PASSED_STATE, `${current.id}: invalid promoted state`);
        const accepted = acceptance.cases.find(row => row.case_id === current.id);
        assert.equal(accepted.result, 'passed'); assert.deepEqual(accepted.accounting_problems, []);
        const reviewCase = ['NGRAPH-CASE-01', 'NGRAPH-CASE-15'].includes(current.id);
        assert.deepEqual(current.evidence.map(row => row.kind).sort(), reviewCase ? ['review', 'test'] : ['test']);
        for (const evidence of current.evidence) {
          assert.equal(evidence.subject, current.id); assert.equal(evidence.revision, REVISION);
          assert.equal(evidence.result, 'passed'); assert.equal(evidence.source_revision, SOURCE);
          assert.ok(typeof evidence.claim === 'string' && evidence.claim.trim(), `${current.id}: missing explicit claim`);
          assert.ok(Array.isArray(evidence.assumptions) && evidence.assumptions.every(row => typeof row === 'string'));
          const reviewEvidence = evidence.kind === 'review';
          const config = evidence.configuration;
          const expected = { profile: 'Numeric-Graph-v1', case_id: current.id,
            receipt: `../../${V}${reviewEvidence ? 'review-receipt.json' : 'acceptance.json'}`,
            receipt_sha256: IMMUTABLE[`${V}${reviewEvidence ? 'review-receipt.json' : 'acceptance.json'}`],
            binary_sha256: acceptance.tools.noble.sha256,
            gate_receipt: '../../verification/main-gates/receipt.json', gate_receipt_sha256: IMMUTABLE['verification/main-gates/receipt.json'],
            rust_tests_log_sha256: acceptance.library_tests.rust_tests.log_sha256,
            tests: accepted.library_tests.map(row => row.name),
            variants: [...new Set(acceptance.commands.filter(row => row.case_id === current.id && row.repetition === 1).map(row => row.variant))],
            external_raw_output: acceptance.external_raw_output.directory,
            prepromotion_case_sha256: item.prepromotion_case_sha256, prepromotion_case_file_sha256: custody.prepromotion_case_file_sha256,
            prepromotion_cases: `../../${V}prepromotion-cases.json`, prepromotion_cases_sha256: IMMUTABLE[`${V}prepromotion-cases.json`],
            build_provenance: `../../${V}build-provenance.json`, build_provenance_sha256: IMMUTABLE[`${V}build-provenance.json`],
            published_build_revision: PUBLISHED, tested_gate_revision: TESTED,
            review_receipt: `../../${V}review-receipt.json`, review_receipt_sha256: IMMUTABLE[`${V}review-receipt.json`],
            review: `../../${V}review.json`, review_sha256: IMMUTABLE[`${V}review.json`], review_external_output: receipt.external_output };
          if (reviewCase) Object.assign(expected, { supplemental_runtime: RUNTIME,
            supplemental_runtime_sha256: receipt.runtime.sha256, supplemental_runtime_inputs_per_binary: 12 });
          if (reviewEvidence) Object.assign(expected, { acceptance_receipt: `../../${V}acceptance.json`, acceptance_receipt_sha256: IMMUTABLE[`${V}acceptance.json`] });
          if (current.id === 'NGRAPH-CASE-08') expected.node_zero_evidence = review.case08.node_zero;
          if (current.id === 'NGRAPH-CASE-14') expected.soft_float_log = { path: provenance.soft_float.log, sha256: provenance.soft_float.sha256 };
          assert.deepEqual(config, expected, `${current.id}/${evidence.kind}: evidence packet/variant/record binding differs`);
          for (const key of ['receipt', 'gate_receipt', 'prepromotion_cases', 'build_provenance', 'review_receipt', 'review', ...(reviewEvidence ? ['acceptance_receipt'] : [])])
            assert.equal(sha(plainBytes(path.resolve(path.dirname(file(CASES)), config[key]))), config[`${key}_sha256`], `${current.id}: ${key} link/hash`);
        }
      }
      assert.equal(caseDocument.cases.filter(row => row.state.execution === 'passed').length, 8);
      assert.equal(caseDocument.cases.filter(row => row.state.execution === 'not-run').length, 7);
      const node = review.case08.node_zero;
      assert.equal(node.expected, 0); assert.equal(node.gate_test, 'refusals_before_evaluation_evaluate_no_node');
      assert.equal(node.file, 'crates/noble-ngraph/tests/ir_check.rs');
      assert.equal(node.evidence_kind, 'direct library counter observations, not CLI counter');
      assert.deepEqual(node.mapping.map(row => row.case_variant), ['input violating an input-validated range fact', 'types line i64 for an f32 graph', 'graph with param slots and no --snapshot']);
      const library = acceptance.library_tests.tests.find(row => row.name === node.gate_test);
      assert.equal(library.file, node.file); assert.equal(library.result, 'passed');
      report.cases = { registered: 15, passed: PASS, not_run: HOLD, proof: 'open', record_hashes_reconstructed: 15 };
    }, false);

    await check('native canonical sync, all13 generated views and ledger parity', async () => {
      for (const helper of ['tools/cairn-specs.mjs', 'tools/check-specs.mjs'])
        assert.deepEqual(plainBytes(file(helper)), anchorBytes(helper), `renderer/ledger helper changed: ${helper}`);
      const { view, regenerate, nativeRequirementIds, toLegacy } = await import(pathToFileURL(file('tools/cairn-specs.mjs')).href);
      const { deriveLedger, loadBundle } = await import(pathToFileURL(file('tools/check-specs.mjs')).href);
      const registered = family.scenario_files.flatMap(name => load(file(`specs/${relative(name)}`)).cases.map(row => ({ ...row, file: name })));
      const documents = [
        ['language', 'SPEC-0001.md', ['NGRAPH-SCOPE-01']],
        ['developer-experience', 'DEVELOPER-EXPERIENCE.md', ['NGRAPH-CLI-01', 'NGRAPH-FORMAT-01']],
        ['evidence', 'EVIDENCE.md', ['NGRAPH-SNAP-02']],
      ];
      for (const [id, compatibility, modified] of documents) {
        const name = `.cairn/specs/${id}/spec.md`, current = plainBytes(file(name)).toString(), before = anchorBytes(name).toString();
        const deltaName = `${ACTIVE}/specs/${id}/spec.md`;
        const delta = plainBytes(file(deltaName)).toString();
        const originalDelta = requirementBlocks(anchorBytes(deltaName).toString());
        const deltaBlocks = requirementBlocks(delta), native = requirementBlocks(current), prior = requirementBlocks(before);
        assert.deepEqual(deltaBlocks.rows.map(row => row.id), originalDelta.rows.map(row => row.id));
        for (const row of originalDelta.rows) {
          const actual = deltaBlocks.rows.find(item => item.id === row.id);
          assert.equal(normativeHead(actual.text), normativeHead(row.text), `approved active contract changed: ${row.id}`);
          // Cairn may retain an optional matching legacy ID label. Compare
          // through the pinned renderer rather than treating that label as prose.
          assert.equal(toLegacy(normativeHead(native.rows.find(item => item.id === row.id)?.text ?? ''), compatibility),
            toLegacy(normativeHead(row.text), compatibility), `canonical sync missing or altered: ${row.id}`);
        }
        assert.equal(native.prefix, prior.prefix, `unrelated canonical header changed: ${name}`);
        assert.deepEqual(nativeRequirementIds(current).sort(), [...new Set([...nativeRequirementIds(before), ...modified])].sort(), `unexpected canonical requirement set: ${name}`);
        for (const row of prior.rows.filter(row => !modified.includes(row.id)))
          assert.equal(withoutGenerated(native.rows.find(item => item.id === row.id)?.text ?? ''), withoutGenerated(row.text), `unrelated canonical requirement changed: ${row.id}`);
        assert.equal(regenerate(current, compatibility, registered), current, `canonical scenario links stale: ${name}`);
        assert.equal(plainBytes(file(`specs/${compatibility}`)).toString(), view(current, compatibility), `canonical/generated renderer drift: ${compatibility}`);
      }
      const scenarioOnly = ['safety', 'verification', 'backend-experiments'].map(id => `.cairn/specs/${id}/spec.md`);
      for (const name of scenarioOnly) {
        assert.equal(withoutScenarioContents(plainBytes(file(name)).toString()),
          withoutScenarioContents(anchorBytes(name).toString()), `scenario-only canonical normative bytes changed: ${name}`);
      }
      assert.equal(family.normative_documents.length, 13);
      for (const document of family.normative_documents) {
        const name = path.posix.normalize(path.posix.join('specs', document.path));
        const current = plainBytes(file(name)).toString();
        assert.equal(regenerate(current, document.compatibility_path, registered), current, `all13 canonical scenario links stale: ${name}`);
        assert.equal(plainBytes(file(`specs/${document.compatibility_path}`)).toString(),
          view(current, document.compatibility_path), `all13 canonical/generated renderer drift: ${document.compatibility_path}`);
      }
      const bundle = loadBundle(root), ledger = deriveLedger(bundle);
      assert.deepEqual(load(file('specs/requirements.json')), ledger, 'generated requirement ledger differs from canonical requirements/cases');
      const validation = load(file('specs/VALIDATION.json'));
      assert.equal(validation.result, 'passed'); assert.equal(validation.lane, 'document-validation-only'); assert.equal(validation.revision, REVISION);
      assert.equal(validation.validator.sha256, sha(plainBytes(file(validation.validator.file))));
      assert.equal(validation.validator.adapter.sha256, sha(plainBytes(file(validation.validator.adapter.file))));
      assert.equal(validation.documents, family.normative_documents.length); assert.equal(validation.requirements, ledger.requirements.length);
      assert.equal(validation.scenarios, registered.length);
      assert.equal(validation.scenarios_executed, registered.filter(row => ['passed', 'failed', 'timeout'].includes(row.state.execution)).length);
      assert.equal(validation.open_test_design, ledger.requirements.filter(row => row.test_design === 'test-design-open').length);
      const obligations = load(file('specs/verification/obligations.json'));
      assert.equal(validation.proof_obligations, obligations.obligations.length);
      assert.equal(validation.completed_proofs, obligations.obligations.filter(row => row.status === 'accepted').length);
      // Use the validator's existing input-selection convention without running
      // validate(), self-tests, or refreshing any report in this verifier.
      const sources = new Set(family.normative_documents.filter(row => row.source).map(row => path.posix.normalize(path.posix.join('specs', row.source))));
      const checkedMarkdown = name => name.endsWith('.md') && (name === 'README.md' || ['specs/', '.cairn/', 'verification/', 'proofs/'].some(prefix => name.startsWith(prefix)));
      const inputs = [...bundle.texts].filter(([name]) => (name.startsWith('specs/') && name !== 'specs/VALIDATION.json') || checkedMarkdown(name) || sources.has(name))
        .sort(([a], [b]) => a.localeCompare(b, 'en')).map(([name, text]) => ({ file: name, sha256: sha(Buffer.from(text)) }));
      assert.deepEqual(validation.inputs, inputs, 'document validation receipt has stale or weakened inputs');
      report.canonical = { sync: true, views: family.normative_documents.map(row => `specs/${row.compatibility_path}`),
        normative_delta_requirements: documents.flatMap(([, , ids]) => ids), scenario_regions_only: scenarioOnly,
        ledger: 'exact-renderer-derived', validation: 'document-only' };
    }, false);
    await check('STATUS/ROADMAP support boundaries and unchanged proof/CALC records', () => {
      const status = load(file('specs/STATUS.json')), oldStatus = JSON.parse(anchorBytes('specs/STATUS.json'));
      const numeric = status.numeric_graph;
      assert.ok(numeric, 'STATUS numeric_graph supporting record required');
      assert.equal(numeric.profile, 'Numeric-Graph-v1'); assert.equal(numeric.implementation, 'partial'); assert.equal(numeric.execution, 'passed');
      assert.deepEqual(numeric.sub_profiles, ['ngraph-i64/0', 'ngraph-f32/0']);
      assert.equal(numeric.selection, 'explicit-opt-in-only'); assert.equal(numeric.change, `../${ACTIVE}/`);
      assert.equal(numeric.scenario_file, 'conformance/numeric-graph-cases.json');
      assert.deepEqual(numeric.passed_cases, PASS); assert.deepEqual(numeric.held_cases, HOLD);
      assert.deepEqual(numeric.held_state, { ...HELD_STATE, evidence: [] });
      assert.deepEqual(Object.keys(numeric.held_gaps).sort(), HOLD);
      assert.ok(Object.values(numeric.held_gaps).every(value => typeof value === 'string' && value.trim()), 'held case gaps must remain explicit');
      assert.equal(numeric.proof, 'open'); assert.equal(numeric.trust, 'explicit'); assert.equal(numeric.source_revision, SOURCE);
      assert.equal(numeric.execution_record, `../${V}acceptance.json`); assert.equal(numeric.acceptance_sha256, IMMUTABLE[`${V}acceptance.json`]);
      assert.equal(numeric.review_record, `../${V}review.json`); assert.equal(numeric.review_sha256, IMMUTABLE[`${V}review.json`]);
      assert.equal(numeric.review_receipt, `../${V}review-receipt.json`);
      assert.equal(numeric.build_provenance_record, `../${V}build-provenance.json`);
      assert.equal(numeric.build_provenance_sha256, IMMUTABLE[`${V}build-provenance.json`]);
      assert.equal(numeric.prepromotion_cases_record, `../${V}prepromotion-cases.json`);
      assert.equal(numeric.prepromotion_cases_sha256, IMMUTABLE[`${V}prepromotion-cases.json`]);
      assert.equal(numeric.binary_sha256, acceptance.tools.noble.sha256);
      assert.equal(numeric.published_build_revision, PUBLISHED); assert.equal(numeric.tested_gate_revision, TESTED);
      assert.equal(numeric.runtime_record, RUNTIME); assert.equal(numeric.runtime_sha256, receipt.runtime.sha256);
      assert.equal(numeric.global_gate_record, '../verification/main-gates/receipt.json');
      assert.equal(numeric.change_status, 'active'); assert.equal(numeric.archived, false); assert.equal(numeric.archive_authorized, false);
      assert.deepEqual(numeric.ng_po_discharged, []);
      assert.deepEqual(numeric.proof_obligations_open, ['A', 'B', 'C', 'D', 'E', 'F', 'G', 'H'].map(id => `NG-PO-${id}`));
      const stripNumeric = value => {
        const copy = structuredClone(value); delete copy.numeric_graph;
        if (copy.profile_status) delete copy.profile_status.numeric_graph;
        return copy;
      };
      assert.deepEqual(stripNumeric(status), stripNumeric(oldStatus), 'unrelated STATUS claims changed');
      assert.equal(typeof status.profile_status?.numeric_graph, 'string');
      assert.ok(fs.statSync(file(ACTIVE)).isDirectory(), 'active change must remain present; archive is not authorized');
      const roadmap = plainBytes(file('specs/ROADMAP.md')).toString();
      const heading = '## Numeric-Graph-v1 (bounded active opt-in profile)';
      assert.equal(roadmap.split(heading).length, 2, 'single supporting Numeric-Graph roadmap section required');
      const section = roadmap.split(heading)[1].split(/^## /m)[0];
      for (const destination of ['conformance/numeric-graph-cases.json', `../${V}acceptance.json`, `../${V}review-receipt.json`])
        assert.ok(section.includes(`](${destination})`), `ROADMAP missing supporting evidence link: ${destination}`);
      const start = roadmap.indexOf(heading), end = roadmap.indexOf('\n## ', start + heading.length);
      assert.equal(roadmap.slice(0, start) + (end < 0 ? '' : roadmap.slice(end + 1)), anchorBytes('specs/ROADMAP.md').toString(),
        'unrelated ROADMAP or CALC milestone content changed');
      report.unchanged_proof_calc = ['specs/verification/obligations.json', 'specs/conformance/calculator-cases.json', '.cairn/specs/calculator/spec.md', 'specs/CALCULATOR.md'].map(name => {
        const original = anchorBytes(name), current = plainBytes(file(name));
        assert.deepEqual(current, original, `proof/CALC bytes changed: ${name}`);
        return { path: name, sha256: sha(current), bytes: current.length };
      });
    }, false);

    if (mode === 'run') {
      await check('external cold-review snapshots and complete provenance', () => {
        const before = new Map(anchorRows.map(row => [row.path, row]));
        const after = new Map(source.rows.map(row => [row.path, row]));
        report.changed_documents = report.changed_paths.map(name => {
          // Even a failed hostile-control run may not export unrelated source.
          if (!allowed.has(name)) return { path: name, snapshot: 'refused-outside-reviewed-allowlist' };
          const old = before.get(name), current = after.get(name);
          const row = { path: name, change: !old ? 'added' : !current ? 'deleted' : 'modified' };
          if (old) row.before = { ...save(`documents/before/${name}`, anchorBytes(name)), mode: old.mode, oid: old.oid };
          if (current && !current.missing) {
            const data = worktreeBytes(name);
            assert.equal(sha(data.bytes), current.worktree_sha256, `snapshot worktree drift: ${name}`);
            assert.equal(data.mode, current.worktree_mode);
            assert.equal(data.kind, 'file', `document snapshot must be plain: ${name}`);
            row.after = { ...save(`documents/after/${name}`, data.bytes), mode: current.worktree_mode, staged: current.index };
          }
          return row;
        });
        report.packet_snapshots = Object.entries(IMMUTABLE).map(([name, expected]) => {
          const bytes = anchorBytes(name); assert.equal(sha(bytes), expected);
          return { original: name, ...save(`packets/${name}`, bytes) };
        });
        report.packet_snapshots.push({ original: RUNTIME, ...save('packets/runtime.json', boundExternal(RUNTIME, receipt.runtime.sha256)) });
        const provenanceSnapshot = { schema: 'noble-numeric-graph-v1-postpromotion-source-snapshot/v1', complete: true,
          root, branch: source.branch, anchor: ANCHOR, anchor_tree: report.source.anchor_tree,
          anchor_rows: anchorRows, source, git_projection: projection ?? null,
          runner: report.source.runner, immutable_packets: report.immutable_packets,
          original_acceptance: { path: ORIGINAL_ACCEPTANCE, sha256: IMMUTABLE[`${V}acceptance.json`] },
          original_review: { path: ORIGINAL_REVIEW, sha256: IMMUTABLE[`${V}review-receipt.json`] },
          full_build_projection: report.full_build_projection ?? null,
          reviewed_allowlist: ALLOWLIST, changed_paths: report.changed_paths,
          changed_documents: report.changed_documents,
          external_evidence: [...externalFiles.values()].sort((a, b) => a.path.localeCompare(b.path, 'en')) };
        report.source_snapshot = saveJSON('source-snapshot.json', provenanceSnapshot);
        report.source.snapshot_revision = `sha256:${digest(provenanceSnapshot)}`;
      });
    }
    await check('terminal frozen source, runner and original packet identities', async () => {
      assert.deepEqual(captureSource(), source, 'whole index/worktree source changed during verification');
      assert.deepEqual(tree(ANCHOR), anchorRows, 'anchor tree identity changed');
      assert.equal(sha(plainBytes(file(RUNNER))), report.source.runner.sha256, 'runner changed during verification');
      if (projection) {
        const { gitSourceProjection } = await import(pathToFileURL(file('verification/m8/source-projection.mjs')).href);
        assert.deepEqual(gitSourceProjection(root, source.rows.map(row => row.path), true), projection, 'gitSourceProjection changed during verification');
      }
      for (const row of externalFiles.values()) boundExternal(row.path, row.sha256, row.bytes);
      for (const row of outputFiles) assert.equal(sha(plainBytes(path.join(out, row.path))), row.sha256, `external snapshot changed: ${row.path}`);
    });
  } catch (error) {
    report.integrity_failures.push({ check: 'required verification prerequisites', error: String(error.message ?? error) });
  }
  if (outputCreated && !report.source_snapshot) {
    // Failed prerequisite controls still receive a durable source observation,
    // never a fabricated complete snapshot or a passing receipt.
    report.source_snapshot = saveJSON('source-snapshot.json', {
      schema: 'noble-numeric-graph-v1-postpromotion-source-snapshot/v1', complete: false,
      root, anchor: values.anchor, anchor_rows: anchorRows ?? null, source: source ?? null,
      git_projection: projection ?? null, runner: report.source?.runner ?? null,
      immutable_packets: Object.entries(IMMUTABLE).map(([name, sha256]) => ({ path: name, sha256 })),
      reviewed_allowlist: ALLOWLIST, changed_paths: report.changed_paths,
      external_evidence: [...externalFiles.values()], integrity_failures: report.integrity_failures,
    });
  }
  report.result = report.failures.length || report.integrity_failures.length ? 'failed' : 'passed';
  report.prerequisites = [...report.failures, ...report.integrity_failures];
  report.ready = report.result === 'passed';
  if (mode === 'plan') {
    report.schema = 'noble-numeric-graph-v1-postpromotion-plan/v1';
    report.planned_result = report.ready ? 'ready' : 'blocked';
    console.log(JSON.stringify(report, null, 2));
    return;
  }
  if (outputCreated) {
    report.outputs = outputFiles;
    // Write once only, after terminal source checks. It cannot be used as an
    // input or self-excluded in-repository receipt in a later source snapshot.
    fs.writeFileSync(path.join(out, 'receipt.json'), JSON.stringify(report, null, 2) + '\n', { flag: 'wx', mode: 0o444 });
    sealDirectory(out);
  }
  console.log(JSON.stringify(report, null, 2));
  if (!report.ready) process.exitCode = 1;
}

try { await main(); }
catch (error) {
  console.error(JSON.stringify({ schema: 'noble-numeric-graph-v1-postpromotion-error/v1', result: 'failed',
    failures: [], integrity_failures: [{ check: 'CLI/output safety', error: String(error.message ?? error) }] }, null, 2));
  process.exitCode = 1;
}
