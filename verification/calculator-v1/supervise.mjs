#!/usr/bin/env node
// Per-vector supervisor started by acceptance.mjs, optionally inside a
// memory-capped systemd scope. It runs exactly one `noble calculator` process
// group, retains raw stdout/stderr, enforces the wall-clock timeout with
// SIGKILL on the whole group, samples per-process peak RSS from /proc, and on
// exit reads its own cgroup's memory.peak and memory.events. It never
// interprets calculator output.
import fs from 'node:fs';
import { spawn } from 'node:child_process';

const spec = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'));
const result = {
  schema: 'noble-calculator-supervisor/v1', pid: null, status: null, signal: null,
  timed_out: false, launch_error: null, stdin_error: null, wall_ms: null,
  started_at: null, group_survivors_killed: false,
  memory: { sample_ms: spec.sample_ms, samples: 0, processes: {}, peak_tree_rss_kib: 0, cgroup: null },
};

const kib = (status, field) => {
  const match = new RegExp(`^${field}:\\s+(\\d+) kB$`, 'm').exec(status);
  return match ? Number(match[1]) : null;
};
const children = pid => {
  try {
    return fs.readFileSync(`/proc/${pid}/task/${pid}/children`, 'utf8').split(' ').filter(Boolean).map(Number);
  } catch {
    return [];
  }
};
const sample = root => {
  const stack = [root];
  let tree = 0;
  while (stack.length > 0) {
    const pid = stack.pop();
    let status;
    try {
      status = fs.readFileSync(`/proc/${pid}/status`, 'utf8');
    } catch {
      continue;
    }
    const name = /^Name:\s+(.*)$/m.exec(status)?.[1] ?? '?';
    const hwm = kib(status, 'VmHWM');
    const rss = kib(status, 'VmRSS');
    const entry = result.memory.processes[pid] ?? { name, peak_rss_kib: 0 };
    if (hwm !== null) entry.peak_rss_kib = Math.max(entry.peak_rss_kib, hwm);
    result.memory.processes[pid] = entry;
    tree += rss ?? 0;
    stack.push(...children(pid));
  }
  result.memory.samples += 1;
  result.memory.peak_tree_rss_kib = Math.max(result.memory.peak_tree_rss_kib, tree);
};
const ownCgroup = () => {
  const line = fs.readFileSync('/proc/self/cgroup', 'utf8').split('\n').find(text => text.startsWith('0::'));
  return line ? `/sys/fs/cgroup${line.slice(3)}` : null;
};
const readCgroup = directory => {
  const read = name => {
    try {
      return fs.readFileSync(`${directory}/${name}`, 'utf8').trim();
    } catch {
      return null;
    }
  };
  const events = Object.fromEntries((read('memory.events') ?? '').split('\n').filter(Boolean)
    .map(line => line.split(' ')).map(([key, value]) => [key, Number(value)]));
  const peak = read('memory.peak');
  return { path: directory, memory_max: read('memory.max'), memory_swap_max: read('memory.swap.max'),
    memory_peak_bytes: peak === null ? null : Number(peak), memory_events: events };
};

const out = fs.openSync(spec.stdout_file, 'wx', 0o600);
const err = fs.openSync(spec.stderr_file, 'wx', 0o600);
const input = fs.readFileSync(spec.stdin_file);
const finish = () => {
  fs.closeSync(out);
  fs.closeSync(err);
  if (spec.cgroup) result.memory.cgroup = readCgroup(ownCgroup());
  fs.writeFileSync(spec.result_file, `${JSON.stringify(result, null, 2)}\n`, { flag: 'wx', mode: 0o600 });
};

const started = process.hrtime.bigint();
result.started_at = new Date().toISOString();
const child = spawn(spec.argv[0], spec.argv.slice(1), {
  cwd: spec.cwd, env: spec.env, stdio: ['pipe', out, err], detached: true,
});
let timer = null;
let sampler = null;
child.on('error', error => {
  result.launch_error = String(error);
  result.wall_ms = Number(process.hrtime.bigint() - started) / 1e6;
  clearTimeout(timer);
  clearInterval(sampler);
  finish();
});
if (child.pid !== undefined) {
  result.pid = child.pid;
  child.stdin.on('error', error => { result.stdin_error = String(error); });
  child.stdin.end(input);
  timer = setTimeout(() => {
    result.timed_out = true;
    try { process.kill(-child.pid, 'SIGKILL'); } catch { /* already gone */ }
  }, spec.timeout_ms);
  sample(child.pid);
  sampler = setInterval(() => sample(child.pid), spec.sample_ms);
  child.on('exit', (status, signal) => {
    result.wall_ms = Number(process.hrtime.bigint() - started) / 1e6;
    result.status = status;
    result.signal = signal;
    clearTimeout(timer);
    clearInterval(sampler);
    // The worker Node child shares the group; nothing may outlive the vector.
    try {
      process.kill(-child.pid, 0);
      process.kill(-child.pid, 'SIGKILL');
      result.group_survivors_killed = true;
    } catch { /* group empty */ }
    finish();
  });
}
