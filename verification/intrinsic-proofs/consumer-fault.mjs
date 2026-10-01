#!/usr/bin/env node
// Exercise a real pinned Lean consumer process failure, not a replacement checker.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { spawn, spawnSync } from 'node:child_process';
import { setTimeout as delay } from 'node:timers/promises';
import { fileHash, root } from './record.mjs';

const POLL_MS = 10;
const WATCH_MS = 120_000;
const STREAM_LIMIT = 1_048_576;
const CONSUMER = '/consumer/IntrinsicConsumer.lean';

function observation(pid) {
  try {
    return { pid, exe: fs.realpathSync(`/proc/${pid}/exe`),
      argv: fs.readFileSync(`/proc/${pid}/cmdline`).toString('utf8')
        .split('\0').filter(Boolean) };
  } catch (error) {
    if (['ENOENT', 'ESRCH', 'EACCES'].includes(error.code)) return null;
    throw error;
  }
}

function descendants(owner) {
  const queue = [owner], seen = new Set(queue), result = [];
  for (let at = 0; at < queue.length; at++) {
    const pid = queue[at];
    let threads;
    try { threads = fs.readdirSync(`/proc/${pid}/task`); }
    catch (error) {
      if (['ENOENT', 'ESRCH'].includes(error.code)) continue;
      throw error;
    }
    for (const thread of threads) {
      let children;
      try { children = fs.readFileSync(`/proc/${pid}/task/${thread}/children`, 'utf8'); }
      catch (error) {
        if (['ENOENT', 'ESRCH'].includes(error.code)) continue;
        throw error;
      }
      for (const value of children.trim().split(/\s+/u)) {
        if (!/^\d+$/u.test(value)) continue;
        const child = Number(value);
        if (seen.has(child)) continue;
        seen.add(child);
        assert.ok(seen.size <= 128, 'unexpectedly large CLI process tree');
        queue.push(child);
        result.push({ pid: child, parent_pid: pid });
      }
    }
  }
  return result;
}

function capture(stream, label) {
  const chunks = [];
  let size = 0;
  stream.on('data', chunk => {
    size += chunk.length;
    if (size <= STREAM_LIMIT) chunks.push(chunk);
  });
  return { bytes: () => {
    assert.ok(size <= STREAM_LIMIT, `${label}: unbounded child output`);
    return Buffer.concat(chunks);
  } };
}

async function run() {
  assert.equal(process.argv.length, 6,
    'usage: SELECTED_NODE consumer-fault.mjs FROZEN_CLI PINNED_LEAN FROZEN_SOURCE PROOF_TIMEOUT_MS');
  const [selectedCli, selectedLean, source, timeoutText] = process.argv.slice(2);
  const cli = fs.realpathSync(selectedCli), lean = fs.realpathSync(selectedLean);
  assert.ok(fs.statSync(source).isFile(), 'actual frozen .noble source required');
  const timeout = Number(timeoutText);
  assert.ok(Number.isSafeInteger(timeout) && timeout > WATCH_MS && timeout <= 600_000,
    'bounded real consumer deadline required');
  const version = spawnSync(lean, ['--version'], { cwd: root, env: process.env,
    encoding: null, timeout: 30_000, maxBuffer: 4096 });
  assert.equal(version.status, 0, 'selected real Lean version probe failed');
  assert.equal(version.signal, null, 'selected Lean version probe was killed');
  assert.match(version.stdout.toString('utf8'), /Lean \(version 4\.31\.0,.*68218e876d2a38b1985b8590fff244a83c321783/u,
    'selected real Lean version/commit differs from the pinned checker');
  const argv = ['verify-module', source, '--timeout-ms', timeoutText];
  const child = spawn(cli, argv, { cwd: root, env: process.env,
    stdio: ['ignore', 'pipe', 'pipe'] });
  const stdout = capture(child.stdout, 'CLI stdout');
  const stderr = capture(child.stderr, 'CLI stderr');
  let exited = false;
  const completed = new Promise((resolve, reject) => {
    child.once('error', reject);
    child.once('close', (status, signal) => { exited = true; resolve({ status, signal }); });
  });
  const started = Date.now();
  let target = null;
  let failure = null;
  while (Date.now() - started < WATCH_MS && !exited && !target) {
    const owner = observation(child.pid);
    if (owner && owner.exe === cli && owner.argv[0] === cli
      && owner.argv[1] === 'verify-module' && owner.argv[2] === source) {
      const tree = descendants(child.pid);
      const parents = new Map(tree.map(item => [item.pid, item.parent_pid]));
      const matches = tree.flatMap(({ pid, parent_pid }) => {
        const candidate = observation(pid);
        return candidate?.exe === lean && candidate.argv.includes(CONSUMER)
          ? [{ ...candidate, parent_pid }] : [];
      });
      if (matches.length > 1) { failure = 'more than one actual pinned consumer child'; break; }
      if (matches.length === 1) {
        const candidate = matches[0], current = observation(candidate.pid);
        if (current?.exe === lean && current.argv.includes(CONSUMER)
          && descendants(child.pid).some(item => item.pid === candidate.pid)) {
          const ancestry = [candidate.pid];
          while (ancestry.at(-1) !== child.pid && ancestry.length <= 128) {
            const parent = parents.get(ancestry.at(-1));
            if (!parent) break;
            ancestry.push(parent);
          }
          assert.equal(ancestry.at(-1), child.pid,
            'pinned consumer is not a descendant of this exact CLI process');
          try {
            process.kill(candidate.pid, 'SIGKILL');
            target = { ...candidate, ancestry, signal: 'SIGKILL' };
          } catch (error) {
            if (error.code !== 'ESRCH') throw error;
          }
        }
      }
    }
    if (!target) await delay(POLL_MS);
  }
  if (!target && !exited) child.kill('SIGKILL');
  const terminal = await completed;
  return {
    schema: 'noble-real-lean-consumer-fault/v1',
    result: target ? 'consumer-process-killed' : 'unwitnessed',
    error: target ? null : failure ?? 'no exact pinned Lean IntrinsicConsumer.lean descendant before bounded CLI exit/deadline',
    selected_version: { status: version.status,
      stdout_base64: version.stdout.toString('base64'),
      stderr_base64: version.stderr.toString('base64') },
    target, monitor: { interval_ms: POLL_MS, deadline_ms: WATCH_MS,
      observed_ms: Date.now() - started },
    cli: { pid: child.pid, executable: cli, sha256: fileHash(cli),
      argv, source_sha256: fileHash(source), status: terminal.status,
      signal: terminal.signal, stdout_base64: stdout.bytes().toString('base64'),
      stderr_base64: stderr.bytes().toString('base64') },
  };
}

try {
  const observed = await run();
  console.log(JSON.stringify(observed));
  if (observed.result !== 'consumer-process-killed') process.exitCode = 1;
} catch (error) {
  console.log(JSON.stringify({ schema: 'noble-real-lean-consumer-fault/v1',
    result: 'unwitnessed', error: String(error.stack ?? error) }));
  process.exitCode = 1;
}
