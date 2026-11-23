#!/usr/bin/env node
// Allocation-strategy baselines (r[ALLOC-EV-01], r[ALLOC-OBS-01]).
//
// Runs the design's W1-W8 workloads against the strategies each profile actually
// uses, records the design's controls (pinned tools, representation, declared
// limits, warmups, repetitions, host load), retains every raw sample, and writes a
// summary receipt bound to the source revision and CLI bytes it ran with.
//
//   node verification/allocation-baselines/run.mjs --cli BIN --out NEW_DIR
//        [--samples N] [--warmups N] [--only W1,W2] [--allow-dirty]
//
// A clean tracked tree is required for a receipt; --allow-dirty is for development
// and is recorded in the receipt.
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

import {
  abi, config, decisive, DECISION_EFFECT, freshDirectory, git, here, hostFacts, median,
  pressure, resolveWasmTools, root, sha256, sourceBinding, summarize, toolBinding, writeJson,
} from './core.mjs';
import {
  ComponentDriver, ExperimentDriver, OrdinaryDriver, calculatorSession, compileSource,
  lineSession, slotInteractive, slotSession, workerDeathProbe,
} from './drivers.mjs';
import {
  calculatorLines, definitionSessionLines, leafCounts, mixedLifetimeLines, repeatedRootLines,
  shortRoot, slotSources, stepExhaustionSource, trapSource, treeBuildLines, treeBuildShape,
  treeExpression, treeOracle, treeShapes,
} from './workloads.mjs';

function argumentsForRun() {
  const options = { cli: process.env.NOBLE_ALLOC_CLI ?? null, out: null, samples: 30,
    warmups: 3, only: null, allowDirty: false, timeoutMs: 900000 };
  for (let index = 2; index < process.argv.length; index += 1) {
    const argument = process.argv[index];
    if (argument === '--allow-dirty') { options.allowDirty = true; continue; }
    if (!['--cli', '--out', '--samples', '--warmups', '--only', '--timeout-ms'].includes(argument)) {
      throw Error(`invalid argument: ${argument}`);
    }
    const value = process.argv[++index];
    if (value === undefined) throw Error(`missing value for ${argument}`);
    if (argument === '--cli') options.cli = value;
    if (argument === '--out') options.out = value;
    if (argument === '--samples') options.samples = Number(value);
    if (argument === '--warmups') options.warmups = Number(value);
    if (argument === '--only') options.only = new Set(value.split(','));
    if (argument === '--timeout-ms') options.timeoutMs = Number(value);
  }
  if (!options.cli || !options.out) throw Error('usage: run.mjs --cli BIN --out NEW_DIR');
  if (!Number.isInteger(options.samples) || options.samples < 1) throw Error('invalid --samples');
  if (!Number.isInteger(options.warmups) || options.warmups < 0) throw Error('invalid --warmups');
  return options;
}

const options = argumentsForRun();
const out = freshDirectory(options.out);
const scratch = fs.mkdtempSync(path.join(process.env.TMPDIR ?? os.tmpdir(), 'alloc-baselines-'));
const cli = fs.realpathSync(path.resolve(options.cli));
const binding = sourceBinding();
if (!binding.clean && !options.allowDirty) {
  throw Error(`the receipt binds a clean tracked tree; status:\n${binding.clean_status}`);
}
const tools = toolBinding(cli);
if (tools.node.version !== config.tools.node.version || tools.node.v8 !== config.tools.node.v8) {
  throw Error(`Node/V8 differs from the pinned runtime: ${tools.node.version}/${tools.node.v8}`);
}
process.env.NOBLE_WASM_TOOLS ??= resolveWasmTools();
const host = hostFacts();

const samplesFile = path.join(out, 'samples.jsonl');
const samples = [];
const points = [];
const failures = [];

function record(entry) {
  samples.push(entry);
  fs.appendFileSync(samplesFile, `${JSON.stringify(entry)}\n`);
}

// One point = one (strategy, workload, shape, size) cell of the design's matrix.
// `run` performs exactly one repetition and returns the guest observation.
function definePoint(id, fields) {
  points.push({ id, ...fields });
}

async function runPoint(point) {
  const sampleCount = point.samples ?? options.samples;
  const warmups = point.warmups ?? options.warmups;
  let attempt = 0;
  const attempts = [];
  while (true) {
    attempt += 1;
    const entries = [];
    let required = 0;
    for (const role of ['warmup', 'sample']) {
      const count = role === 'warmup' ? warmups : sampleCount;
      for (let index = 0; index < count; index += 1) {
        if (options.only && !options.only.has(point.workload)) break;
        const before = pressure();
        const started = process.hrtime.bigint();
        let result;
        let error = null;
        try {
          result = await point.run({ role, index });
        } catch (caught) {
          error = `${caught.name}: ${caught.message}`;
          result = null;
        }
        const wall = Number(process.hrtime.bigint() - started);
        const entry = {
          point: point.id, strategy: point.strategy, workload: point.workload,
          shape: point.shape ?? null, size: point.size ?? null, unit: point.unit,
          includes_compilation: point.includesCompilation ?? false,
          attempt, role, index, wall_ns: wall, outcome: result?.outcome ?? 'harness-error',
          error, units: result?.units ?? null, metrics: result?.metrics ?? null,
          observation: result?.observation ?? null, pressure_before: before,
          pressure_after: pressure(),
        };
        record(entry);
        entries.push(entry);
        if (role === 'sample') required += 1;
        if (error) break;
      }
    }
    const sampleEntries = entries.filter(entry => entry.role === 'sample' && !entry.error);
    const wallValues = sampleEntries.map(entry => entry.wall_ns);
    const unitMedians = sampleEntries.map(entry => entry.units?.median ?? entry.wall_ns);
    const summary = {
      point: point.id, strategy: point.strategy, workload: point.workload,
      shape: point.shape ?? null, size: point.size ?? null, unit: point.unit,
      includes_compilation: point.includesCompilation ?? false,
      attempt, planned_samples: sampleCount, observed_samples: sampleEntries.length,
      errors: entries.filter(entry => entry.error).map(entry => entry.error),
      wall: summarize(wallValues), unit: summarize(unitMedians),
      metrics: sampleEntries.map(entry => entry.metrics).filter(Boolean).at(-1) ?? null,
    };
    summary.decisive_at_20pct = decisive(summary.unit?.ci95);
    summary.noise = summary.unit?.relative_mad ?? null;
    attempts.push(summary);
    const noisy = summary.observed_samples < sampleCount || summary.errors.length
      || (summary.noise !== null && summary.noise > 0.1);
    if (!noisy || attempt >= 2 || point.no_retry) return attempts;
    // At most one re-run (2 attempts total) after a noisy attempt; retain both, never retune a threshold.
  }
}

function requireCliReport(reports, index) {
  const line = reports[index];
  if (!line) throw Error(`missing report ${index + 1}`);
  return JSON.parse(line);
}

function compileOnce(key, source) {
  const file = path.join(scratch, `${key}.wat`);
  if (!fs.existsSync(file)) fs.writeFileSync(file, compileSource(cli, source));
  return fs.readFileSync(file);
}

function makeEngine() {
  return new OrdinaryDriver(cli, scratch);
}

// Source tree lines are compiled by the shipped session itself: only the session
// frontend owns the retained stack and definition namespace.
const sessionDriver = () => ({ command: cli, args: ['session'] });
const liveDriver = (extraArgs = []) => ({ command: cli, args: ['live', 'repl', ...extraArgs] });

function sourceSessionResult(session, expected) {
  const reports = session.reports.map(JSON.parse);
  const last = reports.at(-1) ?? null;
  const observed = last?.stack?.map(entry => entry.value ?? entry.kind) ?? [];
  const okay = expected === undefined || JSON.stringify(observed) === JSON.stringify(expected);
  const withMetrics = reports.filter(row => row.metrics);
  const maxOf = key => withMetrics.length ? Math.max(...withMetrics.map(row => row.metrics[key] ?? 0)) : null;
  const sumOf = key => withMetrics.length ? withMetrics.reduce((sum, row) => sum + (row.metrics[key] ?? 0), 0) : null;
  return {
    outcome: last?.outcome ?? 'missing-report',
    metrics: withMetrics.length ? {
      allocation_peak_bytes: maxOf('allocation_peak_bytes'),
      allocated_total_bytes: sumOf('allocated_total_bytes'),
      released_total_bytes: sumOf('released_total_bytes'),
      retained_before_begin_bytes: maxOf('retained_before_begin_bytes'),
      committed_bytes: maxOf('memory_bytes'),
      operand_peak_bytes: maxOf('operand_peak_bytes'),
      continuation_peak_bytes: maxOf('continuation_peak_bytes'),
      execution_steps: sumOf('execution_steps'),
      quote_invocations: sumOf('quote_invocations'),
      reports_with_metrics: withMetrics.length,
    } : null,
    units: summarize(session.latencies),
    observation: { reports: reports.length, last_outcome: last?.outcome ?? null,
      last_quota_reason: last?.quota_reason ?? null,
      stack: observed, expected_value_matches: okay, exit: session.exit,
      stderr_tail: session.stderr.slice(-400),
      report_metrics: reports.map(row => row.metrics ?? null) },
  };
}

async function main() {
  const started = new Date().toISOString();
  writeJson(path.join(out, 'binding.json'), { source: binding, tools, host });

  // W1: composition trees. A1/A3 use the shipped session/live frontend, A5/A6 the
  // selected M3 build_tree harness. A1/A3 one-expression builds are bounded by the
  // source work budget (16 leaves); larger trees are built across submissions. The
  // recipe-leaf quota binds A1/A3 at 144 leaves, which is recorded as the boundary.
  const frontendTrees = [{ leaves: 8 }, { leaves: 16 }, { leaves: 32 }, { leaves: 128 },
    { leaves: 144, quota: true }];
  for (const shape of ['left', 'right', 'balanced']) {
    for (const { leaves, quota } of frontendTrees) {
      const oracle = treeOracle(leaves);
      const expected = quota ? null : oracle.value;
      if (leaves <= 16) {
        const wat = compileOnce(`a1-${shape}-${leaves}`, treeExpression(shape, leaves));
        definePoint(`W1/A1/${shape}/${leaves}`, {
          strategy: 'A1', workload: 'W1', shape, size: leaves, unit: 'root',
          run: async () => {
            const driver = makeEngine();
            const session = await driver.session([wat]);
            const record = session.records.at(-1);
            if (!record?.report) return { outcome: 'refused', observation: session.records };
            const value = record.report.stack?.[0]?.value;
            return { outcome: record.report.outcome, metrics: record.metrics,
              units: { median: record.execute_ns, p99: record.execute_ns, count: 1 },
              observation: { stack: record.report.stack, expected,
                value_matches: quota ? record.report.outcome !== 'normal' : value === expected,
                prepare_ns: record.prepare_ns } };
          },
        });
      } else {
        const lines = treeBuildLines(shape, leaves);
        definePoint(`W1/A1/${shape}/${leaves}`, {
          strategy: 'A1', workload: 'W1', shape, size: leaves, unit: 'submission',
          includesCompilation: true,
          run: async () => {
            const session = await lineSession(sessionDriver(), lines,
              { timeoutMs: options.timeoutMs, allowEarlyExit: quota });
            return sourceSessionResult(session, quota ? undefined : [expected]);
          },
        });
      }
      if (leaves <= 16) {
        definePoint(`W1/A3/${shape}/${leaves}`, {
          strategy: 'A3', workload: 'W1', shape, size: leaves, unit: 'submission',
          includesCompilation: true,
          run: async () => {
            const session = await lineSession(liveDriver(), [treeExpression(shape, leaves)],
              { timeoutMs: options.timeoutMs });
            return sourceSessionResult(session, quota ? undefined : [expected]);
          },
        });
      } else {
        const lines = treeBuildLines(shape, leaves);
        definePoint(`W1/A3/${shape}/${leaves}`, {
          strategy: 'A3', workload: 'W1', shape, size: leaves, unit: 'submission',
          includesCompilation: true,
          run: async () => {
            const session = await lineSession(liveDriver(), lines,
              { timeoutMs: options.timeoutMs, allowEarlyExit: quota });
            return sourceSessionResult(session, quota ? undefined : [expected]);
          },
        });
      }
    }
    for (const { leaves, quota } of [{ leaves: 8 }, { leaves: 32 }, { leaves: 255 },
      { leaves: 256 }, { leaves: 257, quota: true }]) {
      const oracle = treeOracle(leaves);
      const expected = quota ? null : oracle.value;
      for (const strategy of ['A5', 'A6']) {
        definePoint(`W1/${strategy}/${shape}/${leaves}`, {
          strategy, workload: 'W1', shape, size: leaves, unit: 'root',
          run: async () => {
            const driver = strategy === 'A5' ? drivers.a5 : drivers.a6;
            const result = driver.root({ build: { kind: 'tree', shape: treeShapes[shape], leaves },
              invoke: { kind: 'invoke', value: '20' } });
            const normal = result.status === 0;
            return { outcome: normal ? 'normal' : 'refused',
              metrics: result.snapshots[result.phase ?? 'invocation'] ?? result.snapshots.begin,
              units: { median: result.timings_ns.invocation ?? result.timings_ns.construction, count: 1 },
              observation: { status: result.status, phase: result.phase, output: result.output,
                expected, value_matches: quota ? !normal : result.output[0] === expected,
                cleanup: result.cleanup, timings_ns: result.timings_ns, error: result.engine_error } };
          },
        });
      }
    }
  }

  // A4 W1: one install and one invoke per sample (a single checked source).
  for (const leaves of [8, 16]) {
    const source = treeExpression('left', leaves);
    const oracle = treeOracle(leaves);
    definePoint(`W1/A4/left/${leaves}`, {
      strategy: 'A4', workload: 'W1', shape: 'left', size: leaves, unit: 'invoke',
      run: async () => {
        const directory = fs.mkdtempSync(path.join(scratch, 'slot-'));
        const authority = path.join(directory, 'authority.json');
        fs.writeFileSync(authority, JSON.stringify({ owner: 'alloc-baseline-host', quota: 4,
          effects: [], resources: [], slots: [], grants: [],
          sources: [{ id: 'tree', sha256: sha256(Buffer.from(source)) }] }));
        try {
          const session = await slotSession(cli, authority, [
            { operation: 'install', id: 'tree', source, inputs: [] },
            { operation: 'invoke', id: 'tree', inputs: [], refs: [] },
          ], { timeoutMs: options.timeoutMs });
          const installed = session.rows[1];
          const invoked = session.rows[2];
          return { outcome: invoked?.outcome ?? 'missing', metrics: invoked?.allocation ?? null,
            units: { median: session.latencies[1], count: 1 },
            observation: { installed, invoked, expected: oracle.value,
              value_matches: invoked?.stack?.[0]?.value === oracle.value, exit: session.exit } };
        } finally {
          fs.rmSync(directory, { recursive: true, force: true });
        }
      },
    });
  }

  // W2: repeated short roots. The ordinary and live profiles never reclaim a
  // root's cells, so a persistent session exhausts the declared allocation limit;
  // the sampled point stays below that bound and the full-size point records it.
  definePoint('W2/A1/500', {
    strategy: 'A1', workload: 'W2', size: 500, unit: 'submission', includesCompilation: true,
    run: async () => {
      const session = await lineSession(sessionDriver(), repeatedRootLines(500).map(line => `${line} drop`),
        { timeoutMs: options.timeoutMs });
      return sourceSessionResult(session, []);
    },
  });
  definePoint('W2/A1/10000', {
    strategy: 'A1', workload: 'W2', size: 10000, unit: 'submission', includesCompilation: true,
    samples: 1, warmups: 0,
    run: async () => {
      const session = await lineSession(sessionDriver(), repeatedRootLines(10000).map(line => `${line} drop`),
        { timeoutMs: options.timeoutMs, allowEarlyExit: true });
      return sourceSessionResult(session, undefined);
    },
  });
  definePoint('W2/A3/500', {
    strategy: 'A3', workload: 'W2', size: 500, unit: 'submission', includesCompilation: true,
    samples: 5,
    run: async () => {
      const session = await lineSession(liveDriver(), repeatedRootLines(500).map(line => `${line} drop`),
        { timeoutMs: options.timeoutMs });
      return sourceSessionResult(session, []);
    },
  });
  definePoint('W2/A3/10000', {
    strategy: 'A3', workload: 'W2', size: 10000, unit: 'submission', includesCompilation: true,
    samples: 1, warmups: 0,
    run: async () => {
      const session = await lineSession(liveDriver(), repeatedRootLines(10000).map(line => `${line} drop`),
        { timeoutMs: options.timeoutMs, allowEarlyExit: true });
      return sourceSessionResult(session, undefined);
    },
  });
  for (const strategy of ['A5', 'A6']) {
    definePoint(`W2/${strategy}/10000`, {
      strategy, workload: 'W2', size: 10000, unit: 'root',
      run: async () => {
        const driver = strategy === 'A5' ? drivers.a5 : drivers.a6;
        const result = driver.repeatedRoots(10000);
        return { outcome: [...new Set(result.ports.map(port => port.status))].join(','),
          metrics: result.metrics.at(-1), units: summarize(result.latencies),
          observation: { roots: result.latencies.length,
            first: result.ports[0], last: result.ports.at(-1) } };
      },
    });
  }
  {
    const driver = new ComponentDriver(path.join(root, 'crates/noble-wasm/src/component/runtime.wat'),
      path.join(scratch, 'a7.wasm'));
    definePoint('W2/A7/10000', {
      strategy: 'A7', workload: 'W2', size: 10000, unit: 'root',
      run: async () => {
        const result = driver.repeatedRoots(10000);
        return { outcome: 'normal', metrics: result.metrics.at(-1),
          units: summarize(result.latencies),
          observation: { roots: result.latencies.length, first: result.metrics[0],
            last: result.metrics.at(-1) } };
      },
    });
  }
  {
    const source = slotSources.short;
    definePoint('W2/A4/10000', {
      strategy: 'A4', workload: 'W2', size: 10000, unit: 'invoke',
      run: async () => {
        const directory = fs.mkdtempSync(path.join(scratch, 'slot-'));
        const authority = path.join(directory, 'authority.json');
        fs.writeFileSync(authority, JSON.stringify({ owner: 'alloc-baseline-host', quota: 4,
          effects: [], resources: [], slots: [], grants: [],
          sources: [{ id: 'short', sha256: sha256(Buffer.from(source)) }] }));
        try {
          const operations = [{ operation: 'install', id: 'short', source, inputs: [] },
            ...Array.from({ length: 10000 }, () => ({ operation: 'invoke', id: 'short', inputs: [], refs: [] }))];
          const session = await slotSession(cli, authority, operations, { timeoutMs: options.timeoutMs });
          const invokes = session.rows.slice(2);
          return { outcome: [...new Set(invokes.map(row => row.outcome))].join(','),
            metrics: null, units: summarize(session.latencies.slice(1)),
            observation: { invokes: invokes.length, first: invokes[0], last: invokes.at(-1),
              exit: session.exit } };
        } finally {
          fs.rmSync(directory, { recursive: true, force: true });
        }
      },
    });
  }

  // W3: saved Programs released in seeded random orders (A4 only).
  for (const seed of [1, 2, 3, 4, 5]) {
    definePoint(`W3/A4/64/seed-${seed}`, {
      strategy: 'A4', workload: 'W3', size: 64, unit: 'release-order', seed,
      run: async () => {
        const source = slotSources.quoted;
        const directory = fs.mkdtempSync(path.join(scratch, 'slot-'));
        const authority = path.join(directory, 'authority.json');
        fs.writeFileSync(authority, JSON.stringify({ owner: 'alloc-baseline-host', quota: 8,
          effects: [], resources: [], slots: [], grants: [],
          sources: [{ id: 'quoted', sha256: sha256(Buffer.from(source)) }] }));
        try {
          let owners = [];
          let order = [];
          const session = await slotInteractive(cli, authority, async live => {
            await live.issue({ operation: 'install', id: 'quoted', source, inputs: ['I64'] });
            for (let index = 0; index < 64; index += 1) {
              const row = await live.issue({ operation: 'invoke', id: 'quoted',
                inputs: [{ kind: 'i64', value: String(index) }], refs: [] });
              const owner = row.stack?.at(-1)?.owner;
              if (owner) owners.push(owner);
            }
            order = [...owners];
            let state = seed;
            for (let index = order.length - 1; index > 0; index -= 1) {
              state = (state * 1103515245 + 12345) & 0x7fffffff;
              const swap = state % (index + 1);
              [order[index], order[swap]] = [order[swap], order[index]];
            }
            for (const owner of order) await live.issue({ operation: 'release-program', owner });
          }, { timeoutMs: options.timeoutMs });
          const releases = session.rows.slice(66).map(row => row.outcome);
          return { outcome: [...new Set(releases)].join(','),
            metrics: null, units: summarize(session.latencies.slice(65)),
            observation: { owners_observed: owners.length, owners_distinct: new Set(owners).size,
              released: releases.length, release_outcomes: [...new Set(releases)],
              order_sha256: sha256(Buffer.from(order.join(','))), exit: session.exit } };
        } finally {
          fs.rmSync(directory, { recursive: true, force: true });
        }
      },
    });
  }

  // W4: long sessions with definitions and replacements. The session/live frontends
  // own definition state, so compilation is part of the measured unit.
  definePoint('W4/A1/1000', {
    strategy: 'A1', workload: 'W4', size: 1000, unit: 'submission', includesCompilation: true,
    samples: 10,
    run: async () => {
      const session = await lineSession(sessionDriver(), definitionSessionLines(1000),
        { timeoutMs: options.timeoutMs, allowEarlyExit: true });
      return sourceSessionResult(session, undefined);
    },
  });
  definePoint('W4/A3/1000', {
    strategy: 'A3', workload: 'W4', size: 1000, unit: 'submission', includesCompilation: true,
    samples: 5,
    run: async () => {
      const session = await lineSession(liveDriver(), definitionSessionLines(1000),
        { timeoutMs: options.timeoutMs, allowEarlyExit: true });
      return sourceSessionResult(session, undefined);
    },
  });
  {
    const source = slotSources.short;
    definePoint('W4/A4/1000', {
      strategy: 'A4', workload: 'W4', size: 1000, unit: 'invoke',
      run: async () => {
        const directory = fs.mkdtempSync(path.join(scratch, 'slot-'));
        const authority = path.join(directory, 'authority.json');
        const cycles = 20;
        const digest = sha256(Buffer.from(source));
        const sources = Array.from({ length: cycles }, (_, index) => ({ id: `s${index}`, sha256: digest }));
        const grants = Array.from({ length: cycles }, (_, index) => ({
          operation: 'discard', slotId: `s${index}`, allowed: true,
        }));
        fs.writeFileSync(authority, JSON.stringify({ owner: 'alloc-baseline-host', quota: 4,
          effects: [], resources: [], slots: [], grants, sources }));
        try {
          const operations = [];
          for (let cycle = 0; cycle < cycles; cycle += 1) {
            const id = `s${cycle}`;
            operations.push({ operation: 'install', id, source, inputs: [] });
            for (let step = 0; step < 49; step += 1) {
              operations.push({ operation: 'invoke', id, inputs: [], refs: [] });
            }
            operations.push({ operation: 'discard', id });
          }
          const session = await slotSession(cli, authority, operations, { timeoutMs: options.timeoutMs });
          return { outcome: [...new Set(session.rows.slice(1).map(row => row.outcome))].join(','),
            metrics: null, units: summarize(session.latencies),
            observation: { operations: session.rows.length - 1, cycles, exit: session.exit } };
        } finally {
          fs.rmSync(directory, { recursive: true, force: true });
        }
      },
    });
  }
  definePoint('W4/A2/40', {
    strategy: 'A2', workload: 'W4', size: 40, unit: 'line', includesCompilation: true,
    samples: 1, warmups: 0,
    run: async () => {
      const lines = calculatorLines();
      const session = await calculatorSession(cli, lines);
      return { outcome: session.exit.code === 0 && session.stdout.length >= lines.length / 2 ? 'normal' : 'refused',
        metrics: null,
        units: { median: session.wall_ns / Math.max(1, lines.length), count: lines.length },
        observation: { lines: lines.length, stdout_lines: session.stdout.length,
          stdout: session.stdout.slice(-20), exit: session.exit,
          stderr_tail: session.stderr.slice(-400), wall_ns: session.wall_ns } };
    },
  });

  // W5: mixed lifetimes.
  definePoint('W5/A1/100', {
    strategy: 'A1', workload: 'W5', size: 100, unit: 'submission', includesCompilation: true,
    run: async () => {
      const session = await lineSession(sessionDriver(), mixedLifetimeLines(100),
        { timeoutMs: options.timeoutMs });
      return sourceSessionResult(session, undefined);
    },
  });
  definePoint('W5/A3/100', {
    strategy: 'A3', workload: 'W5', size: 100, unit: 'submission', includesCompilation: true,
    run: async () => {
      const session = await lineSession(liveDriver(), mixedLifetimeLines(100),
        { timeoutMs: options.timeoutMs });
      return sourceSessionResult(session, undefined);
    },
  });
  definePoint('W5/A4/100', {
    strategy: 'A4', workload: 'W5', size: 100, unit: 'invoke',
    run: async () => {
      const directory = fs.mkdtempSync(path.join(scratch, 'slot-'));
      const authority = path.join(directory, 'authority.json');
      fs.writeFileSync(authority, JSON.stringify({ owner: 'alloc-baseline-host', quota: 8,
        effects: [], resources: [], slots: [], grants: [],
        sources: [
          { id: 'quoted', sha256: sha256(Buffer.from(slotSources.quoted)) },
          { id: 'short', sha256: sha256(Buffer.from(slotSources.short)) },
        ] }));
      try {
        const retained = [];
        let scratchCount = 0;
        let released = 0;
        const session = await slotInteractive(cli, authority, async live => {
          await live.issue({ operation: 'install', id: 'quoted', source: slotSources.quoted, inputs: ['I64'] });
          await live.issue({ operation: 'install', id: 'short', source: slotSources.short, inputs: [] });
          for (let index = 0; index < 33; index += 1) {
            const saved = await live.issue({ operation: 'invoke', id: 'quoted',
              inputs: [{ kind: 'i64', value: String(index) }], refs: [] });
            const owner = saved.stack?.at(-1)?.owner;
            if (owner) retained.push(owner);
            await live.issue({ operation: 'invoke', id: 'short', inputs: [], refs: [] });
            scratchCount += 1;
            if (retained.length > 4) {
              const target = retained.shift();
              const res = await live.issue({ operation: 'release-program', owner: target });
              if (res.outcome === 'program-released') released += 1;
            }
          }
          await live.issue({ operation: 'invoke', id: 'short', inputs: [], refs: [] });
          scratchCount += 1;
          while (retained.length) {
            const target = retained.shift();
            const res = await live.issue({ operation: 'release-program', owner: target });
            if (res.outcome === 'program-released') released += 1;
          }
        }, { timeoutMs: options.timeoutMs });
        const opRows = session.rows.slice(3);
        return { outcome: [...new Set(opRows.map(row => row.outcome))].join(','),
          metrics: null, units: summarize(session.latencies.slice(2)),
          observation: { operations: opRows.length, scratch: scratchCount, released,
            exit: session.exit } };
      } finally {
        fs.rmSync(directory, { recursive: true, force: true });
      }
    },
  });
  for (const strategy of ['A5', 'A6']) {
    definePoint(`W5/${strategy}/100`, {
      strategy, workload: 'W5', size: 100, unit: 'root',
      run: async () => {
        const driver = strategy === 'A5' ? drivers.a5 : drivers.a6;
        const result = driver.mixedLifetime(100);
        return { outcome: [...new Set(result.observations.map(row => row.status))].join(','),
          metrics: result.metrics.at(-1), units: summarize(result.latencies),
          observation: { roots: result.latencies.length, first: result.observations[0],
            last: result.observations.at(-1) } };
      },
    });
  }
  definePoint('W5/A7/100', {
    strategy: 'A7', workload: 'W5', size: 100, unit: 'root',
    run: async () => {
      const result = drivers.a7.mixedLifetime(100);
      return { outcome: 'normal', metrics: result.metrics.at(-1),
        units: summarize(result.latencies),
        observation: { roots: result.latencies.length, first: result.observations[0],
          last: result.observations.at(-1) } };
    },
  });

  // W6: quota exhaustion. The guest classes are read from the runtime metric; the
  // host-level F4 sample is a process death outside guest accounting.
  {
    const wat = compileOnce('a1-quota', treeExpression('left', 16));
    definePoint('W6/A1/F1', {
      strategy: 'A1', workload: 'W6', size: 'allocation-bytes', unit: 'root',
      run: async () => {
        const driver = makeEngine();
        const session = await driver.session([wat]);
        const peak = session.records.at(-1)?.metrics?.allocation_peak_bytes;
        const engineWrite = async limit => {
          const Engine = (await import(path.join(root, 'crates/noble-cli/src/core/runtime/host.mjs'))).CoreEngine;
          const directory = fs.mkdtempSync(path.join(scratch, 'a1-q-'));
          fs.writeFileSync(path.join(directory, 'module.wat'), wat);
          const dirEngine = new Engine(config, abi, { artifacts: directory });
          try {
            const prepared = dirEngine.prepare(wat, Buffer.alloc(0), 1);
            if (prepared.outcome !== 'ready') return { outcome: 'prepare-refused' };
            const report = dirEngine.execute({ inputs: [], limits: { allocation_bytes: limit } });
            return { outcome: report.outcome, status: report.status,
              quota_reason: report.quota_reason, metrics: report.metrics };
          } finally {
            dirEngine.close();
            fs.rmSync(directory, { recursive: true, force: true });
          }
        };
        const at = await engineWrite(peak);
        const below = await engineWrite(peak - 1);
        return { outcome: at.outcome, metrics: below.metrics,
          units: { median: 0, count: 1 },
          observation: { peak, at, below,
            distinct: [at.outcome, below.outcome] } };
      },
    });
  }
  for (const strategy of ['A5', 'A6']) {
    definePoint(`W6/${strategy}/F1`, {
      strategy, workload: 'W6', size: 'allocation-bytes', unit: 'root',
      run: async () => {
        const driver = strategy === 'A5' ? drivers.a5 : drivers.a6;
        const calibration = driver.root({ build: { kind: 'tree', shape: 0, leaves: 16 },
          invoke: { kind: 'invoke', value: '20' } });
        const peak = calibration.snapshots.invocation?.allocation_peak_bytes
          ?? calibration.snapshots.construction?.allocation_peak_bytes;
        const at = driver.root({ build: { kind: 'tree', shape: 0, leaves: 16 },
          invoke: { kind: 'invoke', value: '20' }, limits: { allocation_bytes: peak } });
        const below = driver.root({ build: { kind: 'tree', shape: 0, leaves: 16 },
          invoke: { kind: 'invoke', value: '20' }, limits: { allocation_bytes: peak - 1 } });
        const atSnapshot = at.phase ? at.snapshots[at.phase] : (at.snapshots.invocation ?? at.snapshots.construction);
        const belowSnapshot = below.phase ? below.snapshots[below.phase] : (below.snapshots.invocation ?? below.snapshots.construction);
        return { outcome: at.status === 0 && below.status !== 0 ? 'distinct' : 'unexpected',
          metrics: belowSnapshot ?? null,
          units: { median: below.timings_ns.construction, count: 1 },
          observation: { peak,
            at: { status: at.status, phase: at.phase, reason: atSnapshot?.quota_reason ?? null },
            below: { status: below.status, phase: below.phase, reason: belowSnapshot?.quota_reason ?? null } } };
      },
    });
  }
  definePoint('W6/A7/F1', {
    strategy: 'A7', workload: 'W6', size: 'allocation-bytes', unit: 'root',
    run: async () => {
      const at = drivers.a7.quota(64, 64);
      const below = drivers.a7.quota(63, 64);
      return { outcome: at.outcome === 'normal' && below.outcome === 'trap' ? 'distinct' : 'unexpected',
        metrics: below.metrics, units: { median: 0, count: 1 },
        observation: { at, below } };
    },
  });
  definePoint('W6/host/F4', {
    strategy: 'A9', workload: 'W6', size: 'host-process-death', unit: 'process',
    samples: 3, warmups: 0,
    run: async () => {
      const result = workerDeathProbe(64);
      const dead = result.signal !== null || result.exit !== 0;
      return { outcome: dead ? 'process-death' : 'survived', metrics: null,
        units: { median: 0, count: 1 },
        observation: { ...result, class: dead ? 'F4-engine/host-OOM' : 'no-failure',
          injection: 'ulimit -v 262144 (address-space bound); not a cgroup limit' } };
    },
  });

  // W7: abnormal exits and cleanup. A1 and A3 trap after an ordered effect prefix;
  // A5/A6 exhaust dispatch steps mid-construction; A7 traps on a bad range.
  {
    const wat = compileOnce('a1-trap', trapSource);
    definePoint('W7/A1/trap', {
      strategy: 'A1', workload: 'W7', size: 'test.abort', unit: 'root',
      run: async () => {
        const driver = makeEngine();
        const session = await driver.session([wat]);
        const last = session.records.at(-1);
        return { outcome: last?.report?.outcome ?? 'missing', metrics: last?.metrics ?? null,
          units: { median: last?.execute_ns ?? 0, count: 1 },
          observation: { status: last?.report?.status, request_trace: last?.report?.request_trace,
            session_state: last?.report?.session_state } };
      },
    });
    definePoint('W7/A3/trap', {
      strategy: 'A3', workload: 'W7', size: 'test.abort', unit: 'submission',
      includesCompilation: true,
      run: async () => {
        const session = await lineSession(
          liveDriver(['--allow-effects', 'test.emit,test.abort']),
          [trapSource],
          { timeoutMs: options.timeoutMs, allowEarlyExit: true },
        );
        return sourceSessionResult(session, undefined);
      },
    });
  }
  for (const strategy of ['A5', 'A6']) {
    definePoint(`W7/${strategy}/steps`, {
      strategy, workload: 'W7', size: 'dispatch-steps', unit: 'root',
      run: async () => {
        const driver = strategy === 'A5' ? drivers.a5 : drivers.a6;
        const result = driver.root({ build: { kind: 'tree', shape: 0, leaves: 256 },
          invoke: { kind: 'invoke', value: '20' },
          limits: { dispatch_steps: 50 } });
        const snap = result.phase ? result.snapshots[result.phase] : null;
        return { outcome: result.status === 0 ? 'normal' : 'exhausted', metrics: result.cleanup.metrics,
          units: { median: result.timings_ns.cleanup, count: 1 },
          observation: { status: result.status, phase: result.phase,
            reason: snap?.quota_reason ?? null,
            cleanup: result.cleanup.returned_live_bytes, error: result.engine_error } };
      },
    });
  }
  definePoint('W7/A7/range', {
    strategy: 'A7', workload: 'W7', size: 'bad-range', unit: 'root',
    run: async () => {
      const result = drivers.a7.trapOnRange();
      return { outcome: result.outcome, metrics: result.metrics, units: { median: 0, count: 1 },
        observation: result };
    },
  });
  {
    const source = slotSources.stepped;
    definePoint('W7/A4/steps', {
      strategy: 'A4', workload: 'W7', size: 'dispatch-steps', unit: 'invoke',
      run: async () => {
        const directory = fs.mkdtempSync(path.join(scratch, 'slot-'));
        const authority = path.join(directory, 'authority.json');
        fs.writeFileSync(authority, JSON.stringify({ owner: 'alloc-baseline-host', quota: 4,
          effects: [], resources: [], slots: [], grants: [],
          sources: [{ id: 'stepped', sha256: sha256(Buffer.from(source)) }] }));
        try {
          const session = await slotSession(cli, authority, [
            { operation: 'install', id: 'stepped', source, inputs: [] },
            { operation: 'invoke', id: 'stepped', inputs: [], refs: [] },
          ], { timeoutMs: options.timeoutMs });
          return { outcome: session.rows[2]?.outcome ?? 'missing', metrics: session.rows[2]?.allocation ?? null,
            units: { median: session.latencies[1], count: 1 },
            observation: { installed: session.rows[1], invoked: session.rows[2], exit: session.exit } };
        } finally {
          fs.rmSync(directory, { recursive: true, force: true });
        }
      },
    });
  }

  // W8: Calculator-v1 multi-definition session.
  definePoint('W8/A2/40', {
    strategy: 'A2', workload: 'W8', size: 40, unit: 'line', includesCompilation: true,
    samples: 1, warmups: 0,
    run: async () => {
      const session = await calculatorSession(cli, calculatorLines());
      const expected = calculatorLines().length;
      return { outcome: session.exit.code === 0 && session.stdout.length >= expected / 2 ? 'normal' : 'refused',
        metrics: null, units: { median: session.wall_ns / Math.max(1, session.stdout.length),
          count: session.stdout.length },
        observation: { lines: expected, stdout_lines: session.stdout.length,
          stdout: session.stdout.slice(-20), exit: session.exit,
          stderr_tail: session.stderr.slice(-600), wall_ns: session.wall_ns } };
    },
  });

  const summaries = [];
  for (const point of points) {
    // The --only filter selects workloads for development runs.
    if (options.only && !options.only.has(point.workload)) continue;
    process.stderr.write(`[alloc] ${point.id}\n`);
    const attempts = await runPoint(point);
    summaries.push(...attempts);
  }

  const finished = new Date().toISOString();
  const rebinding = sourceBinding();
  const toolsUnchanged = sha256(fs.readFileSync(cli)) === tools.noble.sha256;
  const receipt = {
    schema: 'noble-allocation-baselines/v1',
    change: '.cairn/changes/allocation-strategy-contract',
    requirements: ['ALLOC-EV-01', 'ALLOC-OBS-01'],
    started_at: started,
    finished_at: finished,
    source: binding,
    source_rebound: { ...rebinding, unchanged: binding.commit === rebinding.commit
      && binding.tree === rebinding.tree && binding.clean, cli_unchanged: toolsUnchanged },
    controls: {
      logical_cell_bytes: config.logical_cell_bytes,
      cell_capacity: config.cell_capacity,
      declared_limits: config.limits,
      maximum_limits: config.maximum_limits,
      accounting_revision: 'pre-F1-F4-split (box 1 baseline): quota reason 1 covers F1/F2/F3',
      node_flags: ['--no-liftoff', '--no-wasm-lazy-compilation', '--no-wasm-tier-up',
        '--stack-size=1024', '--max-old-space-size=256'],
      pinned_node: config.tools.node,
      pinned_wasm_tools: config.tools.wasm_tools,
      warmups: options.warmups,
      samples: options.samples,
      clock: 'process.hrtime.bigint',
      cpu_affinity_recorded_not_changed: host.affinity_cpus,
      host_load_recorded_with_every_sample: true,
      retry_policy: 'at most one re-run (2 attempts total) when relative MAD > 0.1 or a sample failed; every attempt retained',
      decision_effect_threshold: DECISION_EFFECT,
    },
    tools,
    host,
    points: summaries,
    raw_samples: { path: path.basename(samplesFile), sha256: sha256(fs.readFileSync(samplesFile)),
      count: samples.length },
    failures,
    applicability: {
      'W1': ['A1', 'A3', 'A4', 'A5', 'A6'],
      'W2': ['A1', 'A3', 'A4', 'A5', 'A6', 'A7'],
      'W3': ['A4'],
      'W4': ['A1', 'A2', 'A3', 'A4'],
      'W5': ['A1', 'A3', 'A4', 'A5', 'A6', 'A7'],
      'W6': ['A1', 'A5', 'A6', 'A7', 'A9'],
      'W7': ['A1', 'A3', 'A4', 'A5', 'A6', 'A7'],
      'W8': ['A2'],
      reasons: {
        'W1/A7': 'the component ABI has no Program, compose or run operation',
        'W1/A2': 'Calculator-v1 has no Program values or runtime composition trees',
        'W2/A2': 'every Calculator-v1 line replays retained definitions; W8 records the bounded session',
        'W3/A1': 'A1 has no release operation (never reclaims within a session)',
        'W3/A3': 'in-process live has no individual release; only whole-instance restore',
        'W3/A5': 'M3 linear reclaims only at cleanup to the module baseline',
        'W3/A6': 'the M3 experiment exposes no per-root release; GC reclamation is unobservable',
        'W3/A7': 'component cleanup resets the whole bump heap; no saved Program owners exist',
        'W3/A2': 'Calculator-v1 compacts at boundaries; it has no saved/shared Program owners',
        'W4/A5': 'the M3 experiment has no source definition or replacement namespace',
        'W4/A6': 'the M3 experiment has no source definition or replacement namespace',
        'W4/A7': 'the component ABI has no source session or definitions',
        'W5/A2': 'Calculator-v1 has no saved captures; Text allocations are measured in W8',
        'W6/A2': 'unexecuted: Calculator-v1 exposes no per-session allocation-limit selection',
        'W6/A3': 'the live REPL exposes no per-session allocation-limit selection',
        'W6/A4': 'F2 live-slot pool exhaustion is exercised by cases.mjs on the live-slot accounting/storage fragments',
      },
    },
    notes: [
      'W1 A1/A3 use the shipped session/live frontend; A5/A6 use the selected M3 build_tree harness; A4 installs one checked source and invokes it.',
      'Timing units are per point: guest root (in-process exports), submission (CLI line), invoke (live-slot command), process (worker death). Points with includes_compilation=true time the shipped frontend too and are not comparable to guest-only roots.',
      'W2 A1 and A3 additionally run one full 10,000-root shipped session (stopping at the 513-submission allocation bound); the sampled distribution uses 500 roots to stay below that bound while measuring per-submission latency.',
      'W6/A4 F2 (pool full) and the lowered-memory F3 harness are executed by cases.mjs; box 1 records the pre-split quota reason.',
      'No physical-return claim: released_total_bytes is logical accounting only.',
    ],
  };
  writeJson(path.join(out, 'receipt.json'), receipt);
  const lines = [
    `source ${binding.commit} clean=${binding.clean}`,
    `cli ${tools.noble.sha256} node ${tools.node.version} v8 ${tools.node.v8}`,
    `samples ${samples.length} points ${summaries.length}`,
    ...summaries.map(summary => `${summary.point}: n=${summary.observed_samples} `
      + `unit_median_ns=${summary.unit?.median ?? 'n/a'} p99=${summary.unit?.p99 ?? 'n/a'} `
      + `ci=[${summary.unit?.ci95?.low ?? 'n/a'},${summary.unit?.ci95?.high ?? 'n/a'}] `
      + `noise=${summary.noise === null ? 'n/a' : summary.noise.toFixed(3)} `
      + `decisive20=${summary.decisive_at_20pct}`),
  ];
  fs.writeFileSync(path.join(out, 'summary.txt'), `${lines.join('\n')}\n`);
  process.stdout.write(`ALLOC-BASELINES ${path.join(out, 'receipt.json')}\n`);
  if (summaries.some(summary => summary.errors.length)) process.exitCode = 1;
}

const drivers = {
  get a5() {
    if (!this._a5) {
      const watFile = path.join(scratch, 'a5.wat');
      const run = spawnSync(cli, ['wasm-experiment', 'managed-linear-memory'], { encoding: 'buffer', maxBuffer: 64 * 1024 * 1024 });
      if (run.status !== 0) throw Error(`wasm-experiment managed-linear-memory failed: ${run.status}`);
      fs.writeFileSync(watFile, run.stdout);
      this._a5 = new ExperimentDriver('A5', watFile, path.join(scratch, 'a5.wasm'));
    }
    return this._a5;
  },
  get a6() {
    if (!this._a6) {
      const watFile = path.join(scratch, 'a6.wat');
      const run = spawnSync(cli, ['wasm-experiment', 'wasm-gc'], { encoding: 'buffer', maxBuffer: 64 * 1024 * 1024 });
      if (run.status !== 0) throw Error(`wasm-experiment wasm-gc failed: ${run.status}`);
      fs.writeFileSync(watFile, run.stdout);
      this._a6 = new ExperimentDriver('A6', watFile, path.join(scratch, 'a6.wasm'));
    }
    return this._a6;
  },
  get a7() {
    if (!this._a7) {
      this._a7 = new ComponentDriver(path.join(root, 'crates/noble-wasm/src/component/runtime.wat'),
        path.join(scratch, 'a7.wasm'));
    }
    return this._a7;
  },
};

try {
  await main();
} catch (error) {
  console.error(`ALLOC-BASELINES failed: ${error.stack ?? error}`);
  process.exitCode = 1;
} finally {
  fs.rmSync(scratch, { recursive: true, force: true });
}
