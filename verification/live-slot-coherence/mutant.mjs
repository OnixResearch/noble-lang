#!/usr/bin/env node
/**
 * Retained broken-current-map control for VC-LIVE-03 (issue #47).
 *
 * The model's `current_map_selection_duplicates_and_omits` names the defect a root pin prevents:
 * a dispatch that reads the registry as published at each selection instead of the root's own
 * map. This runner builds **two** CLIs from the same committed source — an unpatched clean copy
 * and a copy whose `dispatch` selects from the registry's current map — and runs the live-slot
 * lanes against both, so the mutation check is a retained artifact rather than a claim:
 *
 *   clean  : the coherence lane and the two-selection lane both pass;
 *   mutant : both fail. In the two-selection lane the publication lands while the root is held
 *            before its first leaf selection, so the current-map CLI runs the *republished* leaf
 *            for both selections of a root pinned on the old map: both of the pinned version's
 *            events are omitted when it emits (2 -> 0), and two events the pinned version never
 *            performs appear when only the republished version emits (0 -> 2).
 *
 * Both source copies come from `git archive <commit>` of this repository, so neither binary can
 * include an uncommitted edit. The patch text, both source copies' lane digests, both binary
 * hashes, the build recipe and every lane outcome are written to `mutant.json` outside the
 * repository; the retained copy under `verification/live-slot-coherence/` is that file.
 *
 * Usage (pinned Node; the repository tree must be clean):
 *   node verification/live-slot-coherence/mutant.mjs \
 *     --work ~/.cache/noble-iss/scratch/<slice>/mutant-run \
 *     --cargo ~/.cache/noble-iss/cargo.sh --jobs 6 [--repo .] [--target <dir>] [--node <bin>]
 *
 * This writes no conformance status and promotes no LSLOT case.
 */
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const sha = bytes => createHash('sha256').update(bytes).digest('hex');

const option = (name, fallback = null) => {
  const index = process.argv.indexOf(`--${name}`);
  if (index < 0) return fallback;
  assert.ok(index + 1 < process.argv.length, `--${name} requires a value`);
  return process.argv[index + 1];
};
const here = path.dirname(fileURLToPath(import.meta.url));
const repository = path.resolve(option('repo', path.resolve(here, '../..')));
const workOption = option('work');
assert.ok(workOption, 'usage: node mutant.mjs --work <fresh directory> [--repo .] [--cargo cargo]');
const work = path.resolve(workOption);
const cargo = option('cargo', 'cargo');
const node = option('node', process.execPath);
const jobs = option('jobs', '6');
const target = path.resolve(option('target', path.join(work, 'target')));
assert.ok(path.isAbsolute(repository), 'the repository must be an absolute path');
assert.ok(fs.existsSync(path.join(repository, 'verification', 'live-slot-coherence', 'mutant.mjs')),
  'select the Noble repository root that carries this runner');
assert.ok(!fs.existsSync(work), 'the mutant work directory must not exist yet');

const run = (command, argv, options = {}) => {
  const result = spawnSync(command, argv, { encoding: 'utf8', maxBuffer: 64 * 1024 * 1024, ...options });
  return { status: result.status, stdout: result.stdout ?? '', stderr: result.stderr ?? '' };
};
// A failing lane prints the assertion message first and its diff afterwards, so the retained
// excerpt keeps the head of the failure report rather than the tail of the value diff.
const excerpt = text => text.split('\n').filter(Boolean).slice(0, 24).join('\n');

// The control is only meaningful over committed source: the archive below must equal the review.
const status = run('git', ['-C', repository, 'status', '--porcelain']);
assert.equal(status.status, 0, `git status failed: ${status.stderr}`);
assert.equal(status.stdout.trim(), '', 'the repository tree must be clean before the mutant run');
const commit = run('git', ['-C', repository, 'rev-parse', 'HEAD']).stdout.trim();
assert.match(commit, /^[0-9a-f]{40}$/, 'select the source commit of the clean tree');

const PATCHED_FILE = 'crates/noble-cli/src/core/runtime/slot-registry.mjs';
const BEFORE = 'const selected = root.state.map.get(ref.slotId);';
const AFTER = 'const selected = this.#current.map.get(ref.slotId);';
const laneSources = [
  'crates/noble-cli/tests/live_slot_coherent_maps.mjs',
  'crates/noble-cli/tests/live_slot_two_selections.mjs',
  'crates/noble-cli/tests/live_slot_resource_ref_cli.mjs',
];
const repositoryBytes = relative => ({
  path: relative, sha256: sha(fs.readFileSync(path.join(repository, relative))),
});

fs.mkdirSync(work, { recursive: true, mode: 0o700 });
const cleanSource = path.join(work, 'clean');
const mutantSource = path.join(work, 'mutant');
const archive = path.join(work, `source-${commit}.tar`);
const archived = run('git', ['-C', repository, 'archive', '--format=tar', `--output=${archive}`, commit]);
assert.equal(archived.status, 0, `git archive failed: ${archived.stderr}`);
for (const destination of [cleanSource, mutantSource]) {
  fs.mkdirSync(destination, { recursive: true });
  const extracted = run('tar', ['-xf', archive, '-C', destination]);
  assert.equal(extracted.status, 0, `tar extraction failed: ${extracted.stderr}`);
}
fs.rmSync(archive);

const patchedPath = path.join(mutantSource, PATCHED_FILE);
const original = fs.readFileSync(patchedPath, 'utf8');
assert.equal(original.split(BEFORE).length - 1, 1,
  'the pinned-map dispatch selection must occur exactly once in the reviewed source');
const diff = [
  `--- a/${PATCHED_FILE}`,
  `+++ b/${PATCHED_FILE}`,
  `@@ ${BEFORE} @@`,
  `-${BEFORE}`,
  `+${AFTER}`,
  '',
].join('\n');
fs.writeFileSync(patchedPath, original.replace(BEFORE, AFTER), { mode: 0o644 });
const cleanBytes = fs.readFileSync(path.join(cleanSource, PATCHED_FILE));
assert.equal(sha(cleanBytes), sha(fs.readFileSync(path.join(repository, PATCHED_FILE))),
  'the archived clean copy must equal the reviewed file');

const build = (root, which) => {
  const started = Date.now();
  const result = run(cargo, ['build', '-p', 'noble-cli', '--bin', 'noble', '--offline', '--locked'], {
    env: { ...process.env, NOBLE_ROOT: root, NOBLE_TARGET: path.join(target, which),
      CARGO_BUILD_JOBS: jobs },
  });
  assert.equal(result.status, 0, `clean source build ${which} failed: ${excerpt(result.stderr)}`);
  const binary = path.join(target, which, 'debug', 'noble');
  assert.ok(fs.existsSync(binary), `build ${which} produced no selected CLI`);
  return { binary, sha256: sha(fs.readFileSync(binary)), seconds: Math.round((Date.now() - started) / 1000) };
};
const clean = build(cleanSource, 'clean');
const mutant = build(mutantSource, 'mutant');
assert.notEqual(mutant.sha256, clean.sha256,
  'the mutated dispatch must change the selected CLI binary');

const laneEnv = { PATH: process.env.PATH, HOME: process.env.HOME };
// The selected-CLI helper materialises one private authority directory per session inside the
// reviewed tests directory and removes it on close; a lane that fails early can leave one behind,
// so the run removes exactly the directories it created and re-checks the tree afterwards.
const testsDirectory = path.join(repository, 'crates/noble-cli/tests');
const fixturePrefix = 'live_slot_resource_ref_authority-';
const existingFixtures = new Set(fs.readdirSync(testsDirectory).filter(name => name.startsWith(fixturePrefix)));
const removeCreatedFixtures = () => {
  for (const name of fs.readdirSync(testsDirectory)) {
    if (name.startsWith(fixturePrefix) && !existingFixtures.has(name)) {
      fs.rmSync(path.join(testsDirectory, name), { recursive: true, force: true });
    }
  }
};
const runLane = (binary, lane, direction = null) => {
  const argv = [path.join(repository, 'crates/noble-cli/tests', lane), binary];
  if (direction !== null) argv.push(direction);
  const result = run(node, argv, { env: laneEnv });
  const lines = result.stdout.trim().split('\n').filter(Boolean);
  let observed = null;
  try { observed = lines.length ? JSON.parse(lines.at(-1)) : null; } catch { observed = null; }
  return { lane, direction, status: result.status, observed,
    failure: result.status === 0 ? null : excerpt(result.stderr), stderr: result.stderr };
};

let cleanCoherence;
let cleanTwoSelections;
let mutantCoherence;
let mutantDirections;
try {
  cleanCoherence = runLane(clean.binary, 'live_slot_coherent_maps.mjs');
  assert.equal(cleanCoherence.status, 0,
    `the unpatched CLI must pass the coherence lane: ${cleanCoherence.failure}`);
  cleanTwoSelections = runLane(clean.binary, 'live_slot_two_selections.mjs');
  assert.equal(cleanTwoSelections.status, 0,
    `the unpatched CLI must pass the two-selection lane: ${cleanTwoSelections.failure}`);
  for (const variant of cleanTwoSelections.observed.variants) {
    assert.equal(variant.heldCounts.effects, variant.expectedHeldEffects,
      `the unpatched held root must perform ${variant.expectedHeldEffects} events (${variant.variant})`);
    assert.equal(variant.heldPrograms.length, 1,
      `the unpatched held root must keep one pinned Program (${variant.variant})`);
  }

  mutantCoherence = runLane(mutant.binary, 'live_slot_coherent_maps.mjs');
  assert.notEqual(mutantCoherence.status, 0,
    'the current-map CLI must fail the coherence lane it previously passed');
  assert.match(mutantCoherence.stderr,
    /held root must dispatch to and emit from the version it was pinned on/,
    `the coherence failure must be the pinned-version assertion: ${mutantCoherence.failure}`);

  mutantDirections = ['emitting-old', 'inert-old'].map(direction =>
    runLane(mutant.binary, 'live_slot_two_selections.mjs', direction));
  // Measured, not assumed: the root is pinned at the dispatcher selection and held at its first
  // leaf selection, the publication lands during the hold, and the current-map CLI then selects the
  // republished leaf both times. Its event count is therefore the *new* version's, in both directions.
  const expected = { 'emitting-old': { pinned: 2, mutant: 0 }, 'inert-old': { pinned: 0, mutant: 2 } };
  for (const row of mutantDirections) {
    assert.notEqual(row.status, 0, `the current-map CLI must fail the two-selection lane (${row.direction})`);
    assert.match(row.stderr, /the held root must perform the pinned version's event exactly \d+ times/,
      `the two-selection failure must be the pinned-event-count assertion: ${row.failure}`);
    assert.ok(row.observed?.variants?.length === 1,
      `the mutant run must report its measured variant (${row.direction})`);
    const [measured] = row.observed.variants;
    assert.equal(measured.heldCounts.leafSelections, 2,
      'both selections still happen; only their source map changes');
    assert.equal(measured.heldPrograms.length, 1,
      'the mutant selects one version for both selections of the held root');
    assert.deepEqual(measured.heldPrograms, measured.nextPrograms,
      'the version the mutant ran in the held root is the republished one, not the pinned one');
    assert.equal(measured.heldCounts.effects, expected[row.direction].mutant,
      `the mutant must perform the republished version's ${expected[row.direction].mutant} events (${row.direction})`);
    assert.equal(measured.expectedHeldEffects, expected[row.direction].pinned);
    assert.notEqual(measured.heldCounts.effects, measured.expectedHeldEffects,
      'the mutant must differ from the pinned count in the same measurement');
  }
} finally {
  removeCreatedFixtures();
}
const finalStatus = run('git', ['-C', repository, 'status', '--porcelain']);
assert.equal(finalStatus.stdout.trim(), '', 'the lane runs must not leave the reviewed tree dirty');

const receipt = {
  schema: 'noble-live-slot-current-map-mutant/v1',
  claim: 'one committed-source patch makes both retained lanes fail; the unpatched source passes them. Not a proof, not a correspondence claim, not a case promotion.',
  source_commit: commit,
  patch: {
    file: PATCHED_FILE,
    what: 'dispatch selects from the registry\'s current map instead of the root\'s pinned map',
    before: BEFORE, after: AFTER, diff, sha256: sha(diff),
    only_difference: 'the clean and mutated source trees are the same git archive plus this one substitution',
  },
  build: {
    recipe: ['cargo', 'build', '-p', 'noble-cli', '--bin', 'noble', '--offline', '--locked'],
    cargo, jobs,
    clean: { seconds: clean.seconds, binary_sha256: clean.sha256 },
    mutant: { seconds: mutant.seconds, binary_sha256: mutant.sha256 },
    target_root: target,
  },
  lanes: {
    digests: laneSources.map(repositoryBytes),
    node,
    clean: { coherent_maps: cleanCoherence, two_selections: cleanTwoSelections },
    mutant: { coherent_maps: mutantCoherence, two_selections: mutantDirections },
  },
  observed: {
    clean_held_events: cleanTwoSelections.observed.variants.map(variant =>
      ({ variant: variant.variant, held: variant.heldCounts.effects, next: variant.nextCounts.effects })),
    mutant_held_events: mutantDirections.map(row =>
      ({ variant: row.observed.variants[0].variant,
        held: row.observed.variants[0].heldCounts.effects,
        pinned_expected: row.observed.variants[0].expectedHeldEffects,
        held_programs: row.observed.variants[0].heldPrograms.length,
        held_ran_republished: JSON.stringify(row.observed.variants[0].heldPrograms)
          === JSON.stringify(row.observed.variants[0].nextPrograms) })),
    omitted_event: 'emitting-old: the pinned version emits twice in the held root; the current-map CLI ran the republished inert version for both selections and emitted 0 times',
    extra_event: 'inert-old: the pinned version never emits; the current-map CLI ran the republished emitting version for both selections and emitted 2 times',
  },
  does_not_establish: [
    'that the model theorems hold (they are kernel-checked separately)',
    'any correspondence between NobleFoundation/LiveDispatch.lean and slot-registry.mjs',
    'anything about a differently built CLI or a different fixture',
  ],
};
const output = path.join(work, 'mutant.json');
fs.writeFileSync(output, JSON.stringify(receipt, null, 2) + '\n', { mode: 0o600 });
process.stdout.write(`${JSON.stringify({ mutant: output, source_commit: commit,
  clean_binary: clean.sha256, mutant_binary: mutant.sha256,
  clean_events: receipt.observed.clean_held_events,
  mutant_events: receipt.observed.mutant_held_events })}\n`);
