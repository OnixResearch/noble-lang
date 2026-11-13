#!/usr/bin/env node
// Shared candidate execution over `ngraph-i64/0` (issue #117).
//
//   NODE verification/shared-candidates/run.mjs verify  --binary NOBLE --work DIR
//   NODE verification/shared-candidates/run.mjs measure --binary NOBLE --work DIR --receipt FILE
//
// `verify` generates the frozen candidate families, extracts them through
// `noble ngraph extract`, evaluates every arm through `noble ngraph eval`, and
// checks the results against the independent oracle of `oracle.mjs`, the
// normative reference interpreter and each other. It runs the negative
// controls too. It writes no receipt and may run on a dirty tree.
//
// `measure` requires a clean tree, repeats `verify`'s correctness gates first,
// then times the arms and writes the receipt. The receipt records the source
// revision, the CLI digest and the criteria digest, so a reader can see that
// the frozen thresholds of `criteria.json` predate the numbers.
//
// Every program this runner measures is generated from `criteria.json`; the
// runner contains no candidate program of its own. Raw graphs, vector files and
// per-invocation timings stay in `--work`, which must be outside the repo.
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

import {
  candidateCount,
  candidatePrograms,
  evalOutputs,
  firstDisagreement,
  mergedProgram,
  parseSource,
  prefixProgram,
  rows as oracleRows,
  run as oracleRun,
} from './oracle.mjs';

const root = fs.realpathSync(path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..'));
const LANE = 'verification/shared-candidates';
const CRITERIA = `${LANE}/criteria.json`;
const RECEIPT_SCHEMA = 'noble-shared-candidates-receipt/v1';
const CRITERIA_SCHEMA = 'noble-shared-candidates-criteria/v1';
const REPORT_SCHEMA = 'noble-ngraph-report/v1';
const VECTORS_SCHEMA = 'noble-ngraph-vectors/0';
const GIT = process.env.NOBLE_GIT ?? 'git';
const TIME = '/run/current-system/sw/bin/time';
const TIME_FORMAT = '%U %S %M';
const USAGE = `usage: NODE ${LANE}/run.mjs verify  --binary NOBLE --work DIR
       NODE ${LANE}/run.mjs measure --binary NOBLE --work DIR --receipt FILE`;

// ---------------------------------------------------------------------------
// Arguments and small helpers
// ---------------------------------------------------------------------------

function fail(message) {
  process.stderr.write(`${LANE}: ${message}\n`);
  process.exit(2);
}

function parseArguments(argv) {
  const mode = argv[0];
  if (mode !== 'verify' && mode !== 'measure') fail(USAGE);
  const options = { mode };
  for (let i = 1; i < argv.length; i += 1) {
    const flag = argv[i];
    const value = argv[i + 1];
    if (!['--binary', '--work', '--receipt'].includes(flag)) fail(`unknown flag ${flag}\n${USAGE}`);
    if (value === undefined) fail(`${flag} needs a value\n${USAGE}`);
    options[flag.slice(2)] = value;
    i += 1;
  }
  if (options.binary === undefined) fail(`--binary is required\n${USAGE}`);
  if (options.work === undefined) fail(`--work is required\n${USAGE}`);
  if (mode === 'measure' && options.receipt === undefined) fail(`--receipt is required\n${USAGE}`);
  return options;
}

function realOrResolved(p) {
  try {
    return fs.realpathSync(p);
  } catch {
    return path.resolve(p);
  }
}

function assertOutsideRepo(p, what) {
  const real = realOrResolved(p);
  if (real === root || real.startsWith(root + path.sep)) {
    fail(`${what} ${p} is inside the repository; raw outputs and receipts must stay outside it`);
  }
  return real;
}

function sha256(bytes) {
  return crypto.createHash('sha256').update(bytes).digest('hex');
}

function sha256File(file) {
  return sha256(fs.readFileSync(file));
}

function git(args) {
  const result = spawnSync(GIT, args, { cwd: root, encoding: 'utf8' });
  if (result.status !== 0) {
    fail(`git ${args.join(' ')} failed: ${(result.stderr ?? '').trim()}`);
  }
  return result.stdout;
}

function median(values) {
  const sorted = [...values].sort((a, b) => a - b);
  const mid = Math.floor(sorted.length / 2);
  return sorted.length % 2 === 0 ? (sorted[mid - 1] + sorted[mid]) / 2 : sorted[mid];
}

function quartile(values) {
  const sorted = [...values].sort((a, b) => a - b);
  const at = (q) => {
    const position = (sorted.length - 1) * q;
    const lower = Math.floor(position);
    const upper = Math.min(lower + 1, sorted.length - 1);
    return sorted[lower] + (sorted[upper] - sorted[lower]) * (position - lower);
  };
  return at(0.75) - at(0.25);
}

function round(value, places = 3) {
  const scale = 10 ** places;
  return Math.round(value * scale) / scale;
}

// ---------------------------------------------------------------------------
// The frozen criteria
// ---------------------------------------------------------------------------

function loadCriteria() {
  const text = fs.readFileSync(path.join(root, CRITERIA), 'utf8');
  const criteria = JSON.parse(text);
  assert.equal(criteria.schema, CRITERIA_SCHEMA, 'criteria schema');
  assert.equal(criteria.profile, 'ngraph-i64/0', 'criteria profile');
  assert.equal(criteria.inputs, 'i64', 'criteria inputs');
  assert.ok(Array.isArray(criteria.families) && criteria.families.length === 2, 'two families');
  const names = criteria.families.map((f) => f.name);
  assert.deepEqual(names, ['shared-prefix', 'distinct-prefix'], 'frozen family names');
  const armNames = criteria.arms.map((a) => a.name);
  assert.deepEqual(armNames, ['separate', 'merged', 'merged-duplicated', 'floor'], 'frozen arms');
  for (const family of criteria.families) {
    const programs = family.sharing === 'shared' ? family.tails : family.fragments;
    assert.equal(programs.length, 12, `${family.name}: twelve candidates`);
  }
  assert.ok(criteria.timing.warm_rounds >= 5, 'enough warm rounds');
  return { criteria, text };
}


// ---------------------------------------------------------------------------
// Running the CLI
// ---------------------------------------------------------------------------

function runOnce(binary, args) {
  const result = spawnSync(binary, args, { encoding: 'utf8', maxBuffer: 256 * 1024 * 1024 });
  if (result.error) fail(`running ${binary}: ${result.error.message}`);
  return { status: result.status, stdout: result.stdout, stderr: result.stderr };
}

/** Runs one invocation under GNU time and reports wall, CPU and peak RSS. */
function runMeasured(binary, args, work, tag) {
  const flagFile = path.join(work, `time-${tag}.txt`);
  const started = process.hrtime.bigint();
  const result = spawnSync(
    TIME,
    ['-f', TIME_FORMAT, '-o', flagFile, '--', binary, ...args],
    { encoding: 'utf8', maxBuffer: 256 * 1024 * 1024 },
  );
  const wallMs = Number(process.hrtime.bigint() - started) / 1e6;
  if (result.error) fail(`running ${binary}: ${result.error.message}`);
  let cpuMs = 0;
  let rssKib = 0;
  try {
    const [user, system, maxRss] = fs.readFileSync(flagFile, 'utf8').trim().split(/\s+/);
    cpuMs = (Number(user) + Number(system)) * 1000;
    rssKib = Number(maxRss);
  } catch {
    fail(`GNU time wrote no measurement for ${tag}`);
  }
  return { wallMs, cpuMs, rssKib, status: result.status, stdout: result.stdout, stderr: result.stderr };
}

function reportLine(stdout, command) {
  const lines = stdout
    .split('\n')
    .filter((l) => l.trim().length > 0)
    .map((l) => {
      try {
        return JSON.parse(l);
      } catch {
        return fail(`${command}: report line is not JSON: ${l.slice(0, 160)}`);
      }
    });
  const header = lines.find((l) => l.schema === REPORT_SCHEMA);
  if (header === undefined) fail(`${command}: no ${REPORT_SCHEMA} line in the output`);
  const refusal = lines.find((l) => l.outcome !== 'normal');
  return { header, refusal, lines };
}

function extract(binary, work, label, source) {
  const sourcePath = path.join(work, `${label}.noble`);
  const graphPath = path.join(work, `${label}.ngraph`);
  fs.rmSync(graphPath, { force: true });
  fs.writeFileSync(sourcePath, source);
  const { status, stdout } = runOnce(binary, ['ngraph', 'extract', sourcePath, '--inputs', 'i64', '--out', graphPath]);
  const { header, refusal } = reportLine(stdout, 'extract');
  if (status !== 0 || refusal !== undefined || header.outcome !== 'normal') {
    fail(`extract ${label} failed: exit ${status} ${refusal?.code ?? ''} ${refusal?.detail ?? ''}`);
  }
  if (!fs.existsSync(graphPath)) fail(`extract ${label} published no graph`);
  return {
    source: sourcePath,
    graph: graphPath,
    report: header,
    bytes: fs.readFileSync(graphPath),
  };
}

function vectorsText(values) {
  const lines = [VECTORS_SCHEMA, 'types i64'];
  for (const value of values) lines.push(`v ${value.toString()}`);
  return `${lines.join('\n')}\n`;
}

function evaluate(binary, work, label, graphPath, values, extra = []) {
  const vectorsPath = path.join(work, `${label}.vectors`);
  fs.writeFileSync(vectorsPath, vectorsText(values));
  const { status, stdout } = runOnce(binary, ['ngraph', 'eval', graphPath, vectorsPath, ...extra]);
  if (status !== 0) fail(`eval ${label} exited ${status}`);
  return { vectorsPath, outputs: evalOutputs(stdout, `eval ${label}`), stdout };
}

// ---------------------------------------------------------------------------
// Gates
// ---------------------------------------------------------------------------

/** Every candidate's expected single value, per frozen row. */
function oracleCandidates(family, rowValues) {
  return candidatePrograms(family).map((program) => {
    const parsed = parseSource(program.source);
    const values = rowValues.map((x) => {
      const stack = oracleRun(parsed.root, parsed.definitions, [x]);
      assert.equal(stack.length, 1, `${family.name}/${program.name}: one output in the oracle`);
      return stack[0];
    });
    return { program, values };
  });
}

function oracleMerged(family, duplicated, rowValues) {
  const program = mergedProgram(family, duplicated);
  const parsed = parseSource(program.source);
  const expected = candidateCount(family);
  return rowValues.map((x) => {
    const stack = oracleRun(parsed.root, parsed.definitions, [x]);
    assert.equal(stack.length, expected, `${family.name}/${program.name}: ${expected} outputs in the oracle`);
    return stack;
  });
}

function verifyFamily(binary, work, criteria, family, rowValues) {
  const gates = [];
  const gate = (name, detail) => {
    gates.push({ gate: name, holds: true, detail });
  };

  const candidateValues = oracleCandidates(family, rowValues);
  const sharedOracle = oracleMerged(family, false, rowValues);
  const duplicatedOracle = oracleMerged(family, true, rowValues);

  // The oracle's own construction: the merged stack must equal the candidates.
  for (let row = 0; row < rowValues.length; row += 1) {
    const disagreement = firstDisagreement(
      [sharedOracle[row]],
      [candidateValues.map((c) => c.values[row])],
      `${family.name} oracle`,
    );
    assert.equal(disagreement, null, disagreement?.detail);
  }
  const duplicatedAgrees = firstDisagreement(duplicatedOracle, sharedOracle, `${family.name} oracle`);
  assert.equal(duplicatedAgrees, null, `oracle: duplicated form differs: ${duplicatedAgrees?.detail}`);
  gate('oracle-construction', 'the oracle gives every candidate the same value in the merged and separate programs');

  // Structural extraction.
  const candidates = candidateValues.map(({ program }) => extract(binary, work, `${family.name}-${program.name}`, program.source));
  const mergedInfo = mergedProgram(family, false);
  const duplicatedInfo = mergedProgram(family, true);
  const merged = extract(binary, work, `${family.name}-merged`, mergedInfo.source);
  const duplicated = extract(binary, work, `${family.name}-merged-duplicated`, duplicatedInfo.source);
  const count = candidateCount(family);

  for (const [label, info] of [[mergedInfo.name, merged], ['merged-duplicated', duplicated]]) {
    assert.equal(info.report.outputs.length, count, `${family.name}/${label}: ${count} outputs`);
    assert.equal(info.report.inputs.length, 1, `${family.name}/${label}: one input`);
  }
  candidates.forEach((info, index) => {
    assert.equal(info.report.outputs.length, 1, `${family.name}/${candidates[index].report.graph}: one output`);
  });
  gate('interface', `${count} candidate outputs and one input in every merged graph`);

  if (family.sharing === 'shared') {
    const prefix = extract(binary, work, `${family.name}-${prefixProgram(family).name}`, prefixProgram(family).source);
    const prefixNodes = prefix.report.live_nodes;
    const tailNodes = candidates.reduce((sum, info) => sum + (info.report.live_nodes - prefixNodes), 0);
    const expectedMerged = prefixNodes + tailNodes;
    const expectedDuplicated = prefixNodes + (count - 1) * (prefixNodes - 1) + tailNodes;
    assert.equal(merged.report.live_nodes, expectedMerged,
      `${family.name}: merged has ${merged.report.live_nodes} live nodes, the shared construction predicts ${expectedMerged}`);
    assert.equal(duplicated.report.live_nodes, expectedDuplicated,
      `${family.name}: duplicated has ${duplicated.report.live_nodes} live nodes, the recomputing construction predicts ${expectedDuplicated}`);
    gate('structure',
      `shared ${merged.report.live_nodes} live nodes = prefix ${prefixNodes} + tails ${tailNodes}; ` +
      `recomputed ${duplicated.report.live_nodes} live nodes = prefix + ${count - 1} further prefix copies + tails`);
  } else {
    const expectedMerged = 1 + candidates.reduce((sum, info) => sum + (info.report.live_nodes - 1), 0);
    assert.equal(merged.report.live_nodes, expectedMerged,
      `${family.name}: merged has ${merged.report.live_nodes} live nodes, the concatenation predicts ${expectedMerged}`);
    assert.equal(duplicated.report.live_nodes, merged.report.live_nodes,
      `${family.name}: the two merged forms are the same program`);
    gate('structure', `no candidate shares a value: merged ${merged.report.live_nodes} live nodes = the input plus every candidate's own nodes`);
  }

  // Evaluation: every arm, every row.
  const separate = candidates.map((info, index) => ({
    name: `${family.name}/candidate-${index}`,
    outputs: evaluate(binary, work, `${family.name}-candidate-${index}`, info.graph, rowValues).outputs,
  }));
  separate.forEach((arm, index) => {
    const disagreement = firstDisagreement(arm.outputs, candidateValues[index].values.map((v) => [v]), `separate ${arm.name}`);
    assert.equal(disagreement, null, `separate ${arm.name}: ${disagreement?.detail}`);
  });
  gate('separate-oracle', 'every candidate graph reproduces the oracle value on every row');

  const mergedRun = evaluate(binary, work, `${family.name}-merged`, merged.graph, rowValues);
  const duplicatedRun = evaluate(binary, work, `${family.name}-merged-duplicated`, duplicated.graph, rowValues);
  const mergedDisagreement = firstDisagreement(mergedRun.outputs, sharedOracle, `${family.name}/merged`);
  assert.equal(mergedDisagreement, null, `merged: ${mergedDisagreement?.detail}`);
  const duplicatedDisagreement = firstDisagreement(duplicatedRun.outputs, duplicatedOracle, `${family.name}/merged-duplicated`);
  assert.equal(duplicatedDisagreement, null, `merged-duplicated: ${duplicatedDisagreement?.detail}`);
  const crossDisagreement = firstDisagreement(mergedRun.outputs, duplicatedRun.outputs, `${family.name} merged vs duplicated`);
  assert.equal(crossDisagreement, null, `merged vs duplicated: ${crossDisagreement?.detail}`);
  gate('merged-oracle', 'the merged graph and its recomputing twin reproduce the oracle on every row, and each other');

  // The normative reference interpreter over a frozen prefix of the rows.
  const referenceRows = criteria.timing.reference_rows;
  const referenceRun = evaluate(binary, work, `${family.name}-reference`, merged.graph, rowValues.slice(0, referenceRows), ['--reference']);
  const referenceDisagreement = firstDisagreement(referenceRun.outputs, sharedOracle.slice(0, referenceRows), `${family.name}/reference`);
  assert.equal(referenceDisagreement, null, `reference: ${referenceDisagreement?.detail}`);
  gate('reference', `the reference interpreter agrees with the oracle on the first ${referenceRows} rows`);

  // Determinism: a second extraction and a second evaluation of one program.
  const again = extract(binary, work, `${family.name}-merged-again`, mergedInfo.source);
  assert.ok(again.bytes.equals(merged.bytes), `${family.name}: a second extraction differs byte for byte`);
  const evalAgain = runOnce(binary, ['ngraph', 'eval', merged.graph, referenceRun.vectorsPath]);
  const evalAgainTwice = runOnce(binary, ['ngraph', 'eval', merged.graph, referenceRun.vectorsPath]);
  assert.equal(evalAgain.status, 0, `${family.name}: the repeated evaluation exited ${evalAgain.status}`);
  assert.equal(evalAgain.stdout, evalAgainTwice.stdout, `${family.name}: a repeated evaluation differs`);
  gate('determinism', 'the extraction is byte-identical and the evaluation text is identical when repeated');

  return {
    family,
    gates,
    candidates,
    merged,
    duplicated,
    mergedInfo,
    duplicatedInfo,
    mergedRun,
    duplicatedRun,
    separate,
    rowValues,
  };
}

// ---------------------------------------------------------------------------
// Negative controls
// ---------------------------------------------------------------------------

function expectRefusal(binary, work, control, command, expected) {
  const { status, stdout } = runOnce(binary, command);
  const { header, refusal } = reportLine(stdout, control);
  const observed = {
    exit: status,
    outcome: refusal === undefined ? header.outcome : refusal.outcome,
    code: refusal === undefined ? null : refusal.code,
  };
  if (observed.exit !== expected.exit || observed.code !== expected.code) {
    fail(`${control}: expected exit ${expected.exit} ${expected.code}, observed exit ${observed.exit} ${observed.code}`);
  }
  return { control, expected: `${expected.exit} ${expected.code}`, observed };
}

function controls(binary, work, criteria, familyState) {
  const result = [];

  // 1. A merged graph whose candidates are wired to the wrong outputs is
  //    admitted by the CLI (it cannot know the intended programme) and must be
  //    caught by the oracle comparison alone.
  const swapped = familyState.family;
  const permutedFamily = { ...swapped, tails: [swapped.tails[1], swapped.tails[0], ...swapped.tails.slice(2)] };
  const wrongSource = mergedProgram(permutedFamily, false).source;
  const wrongGraph = extract(binary, work, 'control-permuted', wrongSource);
  const wrongRun = evaluate(binary, work, 'control-permuted', wrongGraph.graph, familyState.rowValues.slice(0, 8));
  const wrongOracle = oracleMerged(swapped, false, familyState.rowValues.slice(0, 8));
  const caught = firstDisagreement(wrongRun.outputs, wrongOracle, 'control/permuted');
  assert.notEqual(caught, null, 'the permutation control must disagree with the oracle');
  result.push({
    control: 'permuted-outputs',
    expected: 'the CLI admits the graph and the oracle comparison refuses it',
    observed: { exit: 0, caught: caught.detail },
  });

  // 2. A merged graph with one candidate missing has the wrong arity.
  const truncatedFamily = { ...swapped, tails: swapped.tails.slice(0, -1) };
  const truncated = extract(binary, work, 'control-truncated', mergedProgram(truncatedFamily, false).source);
  assert.notEqual(truncated.report.outputs.length, candidateCount(swapped), 'the truncated graph has fewer outputs');
  result.push({
    control: 'truncated-family',
    expected: `${candidateCount(swapped)} outputs; the truncated graph has fewer and is refused by the interface gate`,
    observed: { exit: 0, outputs: truncated.report.outputs.length },
  });

  // 3. Extraction refusals: each construct outside `ngraph-i64/0` keeps its
  //    code, its exit class and publishes nothing.
  const refusals = [
    ['control-if', 'true [ 1 + ] [ 2 + ] if\n', 4, 'E-CONTROL'],
    ['control-effect', '"hi" test.emit 1 +\n', 4, 'E-EFFECT'],
    ['control-program-output', 'drop [ 1 + ]\n', 4, 'E-PROGRAM-OUTPUT'],
    ['control-unbound', '1 nosuchword +\n', 2, 'E-SUBJECT'],
  ];
  for (const [name, text, exit, code] of refusals) {
    const sourcePath = path.join(work, `${name}.noble`);
    const graphPath = path.join(work, `${name}.ngraph`);
    fs.rmSync(graphPath, { force: true });
    fs.writeFileSync(sourcePath, text);
    const entry = expectRefusal(binary, work, name, ['ngraph', 'extract', sourcePath, '--inputs', 'i64', '--out', graphPath], { exit, code });
    entry.observed.published = fs.existsSync(graphPath);
    assert.equal(entry.observed.published, false, `${name} published a graph`);
    result.push(entry);
  }

  // 4. Profile and budget refusals over a valid source.
  const validSource = familyState.candidates[0].source;
  result.push(expectRefusal(binary, work, 'control-profile',
    ['ngraph', 'extract', validSource, '--inputs', 'i64', '--out', path.join(work, 'control-profile.ngraph'), '--profile', 'ngraph-f32/0'],
    { exit: 2, code: 'E-PROFILE' }));
  result.push(expectRefusal(binary, work, 'control-raised-budget',
    ['ngraph', 'extract', validSource, '--inputs', 'i64', '--out', path.join(work, 'control-budget.ngraph'), '--budget-work', '4194305'],
    { exit: 2, code: 'E-PROFILE' }));

  // 5. A tampered merged graph: an unknown operation and a dangling operand.
  const mergedText = fs.readFileSync(familyState.merged.graph, 'utf8');
  const unknownOp = path.join(work, 'control-unknown-op.ngraph');
  fs.writeFileSync(unknownOp, mergedText.replace(/= i64\.add /, '= i64.div '));
  result.push(expectRefusal(binary, work, 'control-unknown-op', ['ngraph', 'check', unknownOp], { exit: 2, code: 'E-GRAPH-MALFORMED' }));
  const dangling = path.join(work, 'control-dangling-operand.ngraph');
  fs.writeFileSync(dangling, mergedText.replace(/= i64\.add (%\d+) (%\d+)/, '= i64.add $1 %99'));
  result.push(expectRefusal(binary, work, 'control-dangling-operand', ['ngraph', 'check', dangling], { exit: 2, code: 'E-GRAPH-MALFORMED' }));

  // 6. Vector-file refusals: the wrong interface and a missing `types` line.
  const wrongTypes = path.join(work, 'control-wrong-types.vectors');
  fs.writeFileSync(wrongTypes, `${VECTORS_SCHEMA}\ntypes i64 i64\nv 5 4\n`);
  result.push(expectRefusal(binary, work, 'control-wrong-types', ['ngraph', 'eval', familyState.merged.graph, wrongTypes], { exit: 2, code: 'E-INTERFACE' }));
  const noTypes = path.join(work, 'control-no-types.vectors');
  fs.writeFileSync(noTypes, `${VECTORS_SCHEMA}\nv 5\n`);
  result.push(expectRefusal(binary, work, 'control-no-types', ['ngraph', 'eval', familyState.merged.graph, noTypes], { exit: 2, code: 'E-FORMAT' }));

  // 7. The backend gate: no interaction-net arm exists, so nothing is being
  //    preserved here and the net and labelled-SUP arms stay pending #37/#105.
  const noNet = runOnce(binary, ['ngraph', 'net', 'x']);
  assert.equal(noNet.status, 2, 'noble ngraph net is a usage error');
  assert.match(noNet.stderr, /expected check, extract, eval, train or verify/, 'the dispatch names its arms');
  assert.doesNotMatch(noNet.stderr, /\bnet\b[^\n]*GRAPH/, 'the usage text mentions no net arm');
  const ngraphCargo = fs.readFileSync(path.join(root, 'crates/noble-ngraph/Cargo.toml'), 'utf8');
  assert.doesNotMatch(ngraphCargo, /^\[features\]/m, 'noble-ngraph declares no features');
  result.push({
    control: 'backend-gate',
    expected: 'no net arm: usage exits 2 and names no net subcommand; `noble-ngraph` has no feature gate to preserve',
    observed: { exit: noNet.status, features: false },
  });

  assert.ok(criteria !== undefined);
  return result;
}

// ---------------------------------------------------------------------------
// Measurement
// ---------------------------------------------------------------------------

function armCommands(familyState) {
  return {
    separate: familyState.candidates.map((info) => ['ngraph', 'eval', info.graph, familyState.mergedRun.vectorsPath]),
    merged: [['ngraph', 'eval', familyState.merged.graph, familyState.mergedRun.vectorsPath]],
    'merged-duplicated': [['ngraph', 'eval', familyState.duplicated.graph, familyState.duplicatedRun.vectorsPath]],
    floor: [['ngraph', 'eval', familyState.merged.graph, familyState.floorVectors]],
  };
}

function measureFamily(binary, work, criteria, familyState) {
  const commands = armCommands(familyState);
  const arms = criteria.arms.map((a) => a.name);
  const rounds = criteria.timing.warmup_rounds + criteria.timing.warm_rounds;
  const series = Object.fromEntries(arms.map((arm) => [arm, []]));
  for (let turn = 0; turn < rounds; turn += 1) {
    const rotation = turn % arms.length;
    const order = arms.slice(rotation).concat(arms.slice(0, rotation));
    for (const arm of order) {
      let wallMs = 0;
      let cpuMs = 0;
      let rssKib = 0;
      commands[arm].forEach((args, index) => {
        const run = runMeasured(binary, args, work, `${familyState.family.name}-${arm}-${turn}-${index}`);
        if (run.status !== 0) fail(`${familyState.family.name}/${arm} exited ${run.status}`);
        wallMs += run.wallMs;
        cpuMs += run.cpuMs;
        rssKib = Math.max(rssKib, run.rssKib);
      });
      series[arm].push({ round: turn, wall_ms: round(wallMs), cpu_ms: round(cpuMs), rss_kib: rssKib });
    }
  }
  const warm = (arm) => series[arm].slice(criteria.timing.warmup_rounds);
  const summary = {};
  for (const arm of arms) {
    const walls = warm(arm).map((r) => r.wall_ms);
    const cpus = warm(arm).map((r) => r.cpu_ms);
    summary[arm] = {
      first_wall_ms: series[arm][0].wall_ms,
      warm_median_wall_ms: round(median(walls)),
      warm_quartile_wall_ms: round(quartile(walls)),
      warm_min_wall_ms: round(Math.min(...walls)),
      warm_median_cpu_ms: round(median(cpus)),
      warm_median_rss_kib: round(median(warm(arm).map((r) => r.rss_kib)), 1),
      warm_max_rss_kib: Math.max(...warm(arm).map((r) => r.rss_kib)),
      first_warm_rss_kib: warm(arm)[0].rss_kib,
      second_warm_rss_kib: Math.min(warm(arm).length, 2) === 2 ? warm(arm)[1].rss_kib : null,
      processes_per_round: commands[arm].length,
    };
  }
  return { series, summary };
}

function measureExtraction(binary, work, criteria, familyState) {
  const programs = [
    ...familyState.candidates.map((info, index) => [`candidate-${index}`, info]),
    ['merged', familyState.merged],
    ['merged-duplicated', familyState.duplicated],
  ];
  const turns = 3;
  const series = [];
  for (let turn = 0; turn < turns; turn += 1) {
    let wallMs = 0;
    let rssKib = 0;
    const perProgram = {};
    for (const [label, info] of programs) {
      const out = path.join(work, `extract-timing-${label}.ngraph`);
      fs.rmSync(out, { force: true });
      const run = runMeasured(binary, ['ngraph', 'extract', info.source, '--inputs', 'i64', '--out', out], work, `extract-${label}-${turn}`);
      if (run.status !== 0) fail(`extract ${label} exited ${run.status} during the extraction measurement`);
      wallMs += run.wallMs;
      rssKib = Math.max(rssKib, run.rssKib);
      perProgram[label] = round(run.wallMs);
    }
    series.push({ round: turn, wall_ms: round(wallMs), rss_kib: rssKib, per_program_ms: perProgram });
  }
  return {
    invocations_per_round: programs.length,
    median_wall_ms: round(median(series.map((s) => s.wall_ms))),
    max_rss_kib: Math.max(...series.map((s) => s.rss_kib)),
    series,
    note: 'reported only; extraction is producer work and is not part of any evaluation arm',
  };
}

// ---------------------------------------------------------------------------
// The receipt
// ---------------------------------------------------------------------------

function structure(familyState) {
  const family = familyState.family;
  const nodes = familyState.candidates.map((info) => info.report.live_nodes);
  return {
    name: family.name,
    sharing: family.sharing,
    why: family.why,
    candidates: candidateCount(family),
    rows: familyState.rowValues.length,
    live_nodes: {
      candidate_min: Math.min(...nodes),
      candidate_max: Math.max(...nodes),
      candidate_sum: nodes.reduce((a, b) => a + b, 0),
      merged: familyState.merged.report.live_nodes,
      merged_duplicated: familyState.duplicated.report.live_nodes,
    },
    extraction: {
      merged: {
        graph: familyState.merged.report.graph,
        work_units: familyState.merged.report.work_units,
        expanded_nodes: familyState.merged.report.expanded_nodes,
        pruned_nodes: familyState.merged.report.pruned_nodes,
        layers: familyState.merged.report.layers,
      },
      duplicated: {
        graph: familyState.duplicated.report.graph,
        work_units: familyState.duplicated.report.work_units,
        expanded_nodes: familyState.duplicated.report.expanded_nodes,
        pruned_nodes: familyState.duplicated.report.pruned_nodes,
        layers: familyState.duplicated.report.layers,
      },
    },
    graph_sha256: {
      merged: sha256(familyState.merged.bytes),
      merged_duplicated: sha256(familyState.duplicated.bytes),
    },
  };
}

function decide(criteria, families) {
  const thresholds = criteria.thresholds;
  const shared = families.find((f) => f.family.name === 'shared-prefix');
  const distinct = families.find((f) => f.family.name === 'distinct-prefix');
  const ratio = (a, b) => (b === 0 ? null : a / b);
  const d1 = ratio(shared.measurement.summary.merged.warm_median_wall_ms, shared.measurement.summary.separate.warm_median_wall_ms);
  const d2 = ratio(shared.measurement.summary.merged.warm_median_wall_ms, shared.measurement.summary['merged-duplicated'].warm_median_wall_ms);
  const d3Pairs = distinct.measurement.summary;
  const d3 = Math.abs(d3Pairs.merged.warm_median_wall_ms - d3Pairs['merged-duplicated'].warm_median_wall_ms)
    / Math.max(d3Pairs.merged.warm_median_wall_ms, d3Pairs['merged-duplicated'].warm_median_wall_ms);
  const growth = [];
  for (const state of families) {
    for (const [arm, values] of Object.entries(state.measurement.summary)) {
      if (values.second_warm_rss_kib === null) continue;
      growth.push({
        family: state.family.name,
        arm,
        first_warm_rss_kib: values.first_warm_rss_kib,
        second_warm_rss_kib: values.second_warm_rss_kib,
        growth_kib: values.second_warm_rss_kib - values.first_warm_rss_kib,
      });
    }
  }
  const d4 = growth.every((g) => g.growth_kib <= thresholds.d4_rss_growth_kib_max);
  const gatesHold = families.every((state) => state.gates.every((g) => g.holds));
  const d1Holds = d1 !== null && d1 <= thresholds.d1_separate_wall_ratio_max;
  const d2Holds = d2 !== null && d2 <= thresholds.d2_sharing_wall_ratio_max;
  const d3Holds = d3 <= thresholds.d3_noise_band;
  const go = gatesHold && d1Holds && d2Holds && d3Holds && d4;
  let statement;
  if (!gatesHold) {
    statement = 'NO-GO: a correctness gate failed; the timing numbers are not interpretable.';
  } else if (!d3Holds) {
    statement = 'INVALID: the identical-program control on `distinct-prefix` exceeded the noise band, so d2 cannot be read.';
  } else if (!d4) {
    statement = 'NO-GO: an arm grew its peak resident set beyond the declared bound.';
  } else if (d1Holds && d2Holds) {
    statement = 'GO: merged execution beats separate evaluation, and sharing one value is measurably cheaper than recomputing it inside one process, on `shared-prefix` at the frozen size. This is not an optimal-runtime claim.';
  } else if (d1Holds) {
    statement = 'PROCESS-AMORTISATION-ONLY: merged execution is cheaper than separate evaluation, but the sharing saving is inside the noise band. No common-subexpression-elimination claim.';
  } else {
    statement = 'NO-GO: merged execution is not cheaper than separate evaluation at the frozen size.';
  }
  return {
    gates_hold: gatesHold,
    d1: {
      quantity: 'merged warm median wall / separate warm median wall on shared-prefix',
      ratio: d1 === null ? null : round(d1, 4),
      threshold: `<= ${thresholds.d1_separate_wall_ratio_max}`,
      holds: d1Holds,
    },
    d2: {
      quantity: 'merged warm median wall / merged-duplicated warm median wall on shared-prefix',
      ratio: d2 === null ? null : round(d2, 4),
      threshold: `<= ${thresholds.d2_sharing_wall_ratio_max}`,
      holds: d2Holds,
    },
    d3: {
      quantity: 'relative difference of the two identical merged programs on distinct-prefix',
      value: round(d3, 4),
      threshold: `<= ${thresholds.d3_noise_band}`,
      holds: d3Holds,
    },
    d4: {
      quantity: 'peak resident growth from the first to the second warm round, every arm',
      threshold: `<= ${thresholds.d4_rss_growth_kib_max} KiB`,
      holds: d4,
      rows: growth,
    },
    go,
    statement,
  };
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

function main() {
  const options = parseArguments(process.argv.slice(2));
  const binary = realOrResolved(options.binary);
  if (!fs.existsSync(binary)) fail(`--binary ${options.binary} does not exist`);
  const work = assertOutsideRepo(options.work, '--work');
  fs.mkdirSync(work, { recursive: true });
  if (options.receipt !== undefined) assertOutsideRepo(options.receipt, '--receipt');
  if (!fs.existsSync(TIME)) fail(`${TIME} (GNU time) is required for the memory and CPU instruments`);

  const { criteria, text: criteriaText } = loadCriteria();
  const rowValues = oracleRows(criteria.vectors);
  fs.writeFileSync(path.join(work, 'rows.vectors'), vectorsText(rowValues));

  const revision = git(['rev-parse', 'HEAD']).trim();
  if (options.mode === 'measure') {
    const dirty = git(['status', '--porcelain']).trim();
    if (dirty.length !== 0) fail(`the tree is not clean, so a receipt would not bind its sources:\n${dirty}`);
  }

  const families = criteria.families.map((family) => verifyFamily(binary, work, criteria, family, rowValues));

  // The floor arm needs a one-row vector file for the merged graph.
  for (const state of families) {
    const floorVectors = path.join(work, `${state.family.name}-floor.vectors`);
    fs.writeFileSync(floorVectors, vectorsText(rowValues.slice(0, 1)));
    state.floorVectors = floorVectors;
  }

  const controlResults = controls(binary, work, criteria, families[0]);

  process.stdout.write(`${LANE}: gates passed for ${families.length} families, ${controlResults.length} controls held\n`);
  if (options.mode === 'verify') return;

  for (const state of families) {
    state.measurement = measureFamily(binary, work, criteria, state);
    state.extraction_timing = measureExtraction(binary, work, criteria, state);
  }

  const receipt = {
    schema: RECEIPT_SCHEMA,
    issue: 117,
    produced: new Date().toISOString(),
    source_revision: revision,
    clean_tree: true,
    criteria: {
      path: CRITERIA,
      sha256: sha256(Buffer.from(criteriaText, 'utf8')),
      frozen: criteria.frozen,
      committed_blob: git(['rev-parse', `HEAD:${CRITERIA}`]).trim(),
    },
    binary: {
      path: binary,
      sha256: sha256File(binary),
      arms: runOnce(binary, ['ngraph']).stderr.split('\n')[0].trim(),
    },
    instrument: {
      wall: 'process.hrtime.bigint around spawnSync',
      cpu_and_rss: `${TIME} -f '${TIME_FORMAT}'`,
      time_version: runOnce(TIME, ['--version']).stdout.split('\n')[0],
      node: process.version,
      node_path: process.execPath,
      rows: criteria.vectors.rows,
      row_source: `boundary rows ${criteria.vectors.boundary.length} then splitmix64 seed ${criteria.vectors.seed} in [0, ${criteria.vectors.range}]`,
      rows_sha256: sha256(Buffer.from(vectorsText(rowValues), 'utf8')),
      rounds: criteria.timing.warm_rounds,
      warmup_rounds: criteria.timing.warmup_rounds,
      cold: criteria.timing.cold,
    },
    host: {
      kernel: os.release(),
      arch: os.arch(),
      cpus: os.cpus().length,
      load1: round(os.loadavg()[0], 2),
      note: 'shared host: medians of interleaved rounds, no CPU pinning and no cache drop',
    },
    families: families.map((state) => ({
      ...structure(state),
      gates: state.gates,
      measurement: state.measurement,
      extraction_timing: state.extraction_timing,
    })),
    controls: controlResults,
    decision: decide(criteria, families),
    not_measured: criteria.not_measured,
  };

  fs.writeFileSync(options.receipt, `${JSON.stringify(receipt, null, 2)}\n`);
  process.stdout.write(`${LANE}: receipt written to ${options.receipt}\n`);
  process.stdout.write(`${LANE}: ${receipt.decision.statement}\n`);
  if (!receipt.decision.gates_hold) process.exit(1);
}

try {
  main();
} catch (error) {
  process.stderr.write(`${LANE}: ${error instanceof Error ? error.message : String(error)}\n`);
  if (error instanceof Error && error.stack !== undefined) {
    process.stderr.write(`${error.stack.split('\n').slice(1, 4).join('\n')}\n`);
  }
  process.exit(1);
}
