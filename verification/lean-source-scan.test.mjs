// The m3 gates' proof-hole scan: hits real escape hatches, ignores comments,
// strings and characters, and cannot be fooled by a document that merely
// mentions the construct.
import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { maskSource, scanText } from './lean-source-scan.mjs';

const constructs = text => scanText(text).map(hit => hit.construct);

test('a real proof hole is found with its line number', () => {
  const hits = scanText('theorem a : True := trivial\n\ntheorem b : False := by\n  sorry\n');
  assert.deepEqual(hits.map(hit => [hit.line, hit.construct]), [[4, 'sorry']]);
});

test('admit, a local axiom, debug.skipKernelTC and addDecl are all hits', () => {
  assert.deepEqual(constructs('theorem a : False := by admit\n'), ['admit']);
  assert.deepEqual(constructs('axiom bad : False\n'), ['axiom declaration']);
  assert.deepEqual(constructs('set_option debug.skipKernelTC true in\ntheorem a : True := trivial\n'),
    ['debug.skipKernelTC']);
  assert.deepEqual(constructs('run_cmd addDecl (Declaration.thmDecl {})\n'), ['addDecl']);
});

test('comments never produce a hit, including nested block comments', () => {
  assert.deepEqual(constructs('-- theorem a : False := by sorry\n'), []);
  assert.deepEqual(constructs('/- sorry -/\n'), []);
  assert.deepEqual(constructs('/- outer /- admit -/ axiom bad : False -/\ntheorem t : True := trivial\n'), []);
  assert.deepEqual(constructs('theorem t : True := trivial -- addDecl in prose\n'), []);
});

test('string and character literals never produce a hit', () => {
  assert.deepEqual(constructs('def message := "sorry, admit, axiom, addDecl"\n'), []);
  assert.deepEqual(constructs('def quoted := r#"debug.skipKernelTC"#\n'), []);
  assert.deepEqual(constructs("def carriage := '\\n'\ndef quote := '\\''\n"), []);
});

test('identifiers that merely contain a construct do not match', () => {
  assert.deepEqual(constructs('theorem admitted_results_agree : True := trivial\n'), []);
  assert.deepEqual(constructs('def sorryFree := 1\n'), []);
});

test('a prime in an identifier does not swallow the rest of the line', () => {
  assert.deepEqual(constructs("theorem t' : True := trivial -- note\n"), []);
  assert.deepEqual(constructs("def h' := 1\ntheorem u : False := by sorry\n"), ['sorry']);
  assert.deepEqual(maskSource("theorem t' : True := trivial\n").trim(), "theorem t' : True := trivial");
});

test('an interpolated string is masked whole; only code outside it is scanned', () => {
  assert.deepEqual(constructs('def s := s!"a sorry {1}"\n'), []);
  assert.deepEqual(constructs('def s := s!"{x}" \ndef x := by sorry\n'), ['sorry']);
});

test('--only restricts the scan to the named construct', () => {
  const text = 'axiom bad : False\ntheorem t : False := by sorry\n';
  assert.deepEqual(scanText(text, ['sorry']).map(hit => hit.construct), ['sorry']);
  assert.deepEqual(scanText(text, ['addDecl']), []);
  assert.deepEqual(scanText(text).map(hit => hit.construct), ['axiom declaration', 'sorry']);
});

test('the CLI scans a tree without following symlinks or entering .lake', () => {
  const dir = mkdtempSync(join(tmpdir(), 'noble-scan-'));
  mkdirSync(join(dir, '.lake', 'build'), { recursive: true });
  mkdirSync(join(dir, 'NobleM2'), { recursive: true });
  writeFileSync(join(dir, '.lake', 'build', 'Generated.lean'), 'theorem t : False := by sorry\n');
  writeFileSync(join(dir, 'NobleM2', 'Real.lean'), 'theorem u : True := trivial\n');
  writeFileSync(join(dir, 'NobleM2', 'Hole.lean'), 'theorem v : False := by admit\n');
  symlinkSync('/nonexistent/noble-target', join(dir, 'NobleM2', 'Broken.lean'));
  const observed = spawnSync(process.execPath,
    [join(dirname(fileURLToPath(import.meta.url)), 'lean-source-scan.mjs'), dir], { encoding: 'utf8' });
  assert.equal(observed.status, 1, observed.stderr);
  assert.match(observed.stderr, /NobleM2\/Hole\.lean:1: forbidden construct 'admit'/);
  assert.doesNotMatch(observed.stderr, /\.lake/);
  assert.doesNotMatch(observed.stderr, /Broken\.lean/);
  const quiet = spawnSync(process.execPath,
    [join(dirname(fileURLToPath(import.meta.url)), 'lean-source-scan.mjs'), '--only', 'sorry', dir],
    { encoding: 'utf8' });
  assert.equal(quiet.status, 0, quiet.stderr);
});

test('an unknown --only name and an unknown option are usage errors', () => {
  const script = join(dirname(fileURLToPath(import.meta.url)), 'lean-source-scan.mjs');
  assert.equal(spawnSync(process.execPath, [script, '--only', 'nonsense', '.'], { encoding: 'utf8' }).status, 2);
  assert.equal(spawnSync(process.execPath, [script, '--bogus', '.'], { encoding: 'utf8' }).status, 2);
  assert.equal(spawnSync(process.execPath, [script], { encoding: 'utf8' }).status, 2);
});
