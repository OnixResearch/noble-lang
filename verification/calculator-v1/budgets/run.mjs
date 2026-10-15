#!/usr/bin/env node
// CALC-08 budget bracket runner. Each plan session is one `noble calculator`
// process (one stdin) run against an exact prebuilt binary, optionally inside
// a memory-capped systemd user scope. Raw stdin/stdout/stderr and timestamped
// output events are retained in a fresh directory outside the repository.
// The receipt records observations only; it never changes a canonical case.
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { spawn, spawnSync } from 'node:child_process';
import { expectations } from './check.mjs';

const usage = 'usage: run.mjs --binary FILE --binary-sha256 HEX --source-id TEXT --out NEW_DIR '
  + '[--plan FILE] [--only ID,...] [--concurrency N] [--memory-max SIZE] [--timeout-s N]';
const options = { plan: new URL('plan.json', import.meta.url).pathname, concurrency: 4,
  'memory-max': '2G', 'timeout-s': 7200, only: null };
for (let at = 2; at < process.argv.length; at += 2) {
  const key = process.argv[at].replace(/^--/, '');
  if (!(key in options) && !['binary', 'binary-sha256', 'source-id', 'out'].includes(key)) {
    console.error(usage);
    process.exit(2);
  }
  options[key] = process.argv[at + 1];
}
for (const key of ['binary', 'binary-sha256', 'source-id', 'out']) {
  if (!options[key]) {
    console.error(usage);
    process.exit(2);
  }
}
const sha256 = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const binary = fs.realpathSync(options.binary);
const binaryHash = sha256(fs.readFileSync(binary));
if (binaryHash !== options['binary-sha256']) {
  console.error(`binary sha256 ${binaryHash} does not match --binary-sha256`);
  process.exit(2);
}
const out = path.resolve(options.out);
const repo = path.resolve(new URL('../../..', import.meta.url).pathname);
if (out === repo || out.startsWith(`${repo}${path.sep}`)) {
  console.error('--out must be outside the repository');
  process.exit(2);
}
fs.mkdirSync(out, { recursive: false });
const planBytes = fs.readFileSync(options.plan);
const plan = JSON.parse(planBytes);
const only = options.only ? new Set(options.only.split(',')) : null;
const sessions = plan.sessions.filter(session => !only || only.has(session.id));
const timeoutMs = Number(options['timeout-s']) * 1000;
const git = args => spawnSync('git', ['-C', repo, ...args], { encoding: 'utf8' }).stdout.trim();
const time = fs.existsSync('/run/current-system/sw/bin/time') ? '/run/current-system/sw/bin/time' : null;

const lineOf = text => Buffer.from(text, 'utf8');
const stdinOf = session => Buffer.concat(session.lines.map(line => {
  if (typeof line === 'string') return Buffer.concat([lineOf(line), Buffer.from('\n')]);
  // {"repeat": "1", "count": N}: a long generated line, recorded by length and hash.
  return Buffer.concat([lineOf(line.repeat.repeat(line.count)), Buffer.from('\n')]);
}));

const runSession = session => new Promise(resolve => {
  const directory = path.join(out, 'sessions', session.id.replaceAll('/', '__'));
  fs.mkdirSync(directory, { recursive: true });
  const stdin = stdinOf(session);
  fs.writeFileSync(path.join(directory, 'stdin'), stdin);
  const calculator = [binary, 'calculator', ...session.flags];
  const timed = time ? [time, '-f', '%e %M', '-o', path.join(directory, 'time'), ...calculator] : calculator;
  const memoryMax = session.memory_max ?? options['memory-max'];
  const argv = memoryMax
    ? ['systemd-run', '--user', '--scope', '-q', '-p', `MemoryMax=${memoryMax}`, '-p', 'MemorySwapMax=0', '--', ...timed]
    : timed;
  const events = [];
  const stdout = [];
  const stderr = [];
  const started = process.hrtime.bigint();
  const at = () => Number(process.hrtime.bigint() - started) / 1e6;
  const child = spawn(argv[0], argv.slice(1), { stdio: ['pipe', 'pipe', 'pipe'], detached: true,
    env: { PATH: process.env.PATH, HOME: process.env.HOME, TMPDIR: process.env.TMPDIR ?? '/tmp',
      XDG_RUNTIME_DIR: process.env.XDG_RUNTIME_DIR ?? '', DBUS_SESSION_BUS_ADDRESS: process.env.DBUS_SESSION_BUS_ADDRESS ?? '' } });
  const timeout = session.timeout_s ? session.timeout_s * 1000 : timeoutMs;
  let timedOut = false;
  const timer = setTimeout(() => {
    timedOut = true;
    try { process.kill(-child.pid, 'SIGKILL'); } catch { /* gone */ }
  }, timeout);
  let killer = null;
  if (session.kill_worker_after_stdout_lines) {
    // Native-failure control: SIGKILL the calculator's Node Wasm worker once
    // the given number of stdout lines has appeared.
    killer = () => {
      const lines = Buffer.concat(stdout).toString('utf8').split('\n').length - 1;
      if (lines < session.kill_worker_after_stdout_lines || killer.done) return;
      killer.done = true;
      const workers = spawnSync('pgrep', ['-g', String(child.pid), '-f', 'node'], { encoding: 'utf8' })
        .stdout.split('\n').filter(Boolean);
      events.push({ ms: at(), stream: 'control', text: `SIGKILL worker ${workers.join(',')}` });
      for (const pid of workers) try { process.kill(Number(pid), 'SIGKILL'); } catch { /* gone */ }
    };
  }
  const record = (stream, sink) => chunk => {
    sink.push(chunk);
    events.push({ ms: at(), stream, text: chunk.toString('utf8') });
    if (killer && stream === 'stdout') killer();
  };
  child.stdout.on('data', record('stdout', stdout));
  child.stderr.on('data', record('stderr', stderr));
  child.stdin.on('error', () => {});
  child.stdin.end(stdin);
  child.on('exit', (status, signal) => {
    clearTimeout(timer);
    const wall = at();
    try { process.kill(-child.pid, 'SIGKILL'); } catch { /* group empty */ }
    // Let the pipes drain before recording.
    setTimeout(() => {
      fs.writeFileSync(path.join(directory, 'stdout'), Buffer.concat(stdout));
      fs.writeFileSync(path.join(directory, 'stderr'), Buffer.concat(stderr));
      fs.writeFileSync(path.join(directory, 'events.json'), `${JSON.stringify(events, null, 1)}\n`);
      let timing = null;
      try {
        const [seconds, kib] = fs.readFileSync(path.join(directory, 'time'), 'utf8').trim().split('\n').pop().split(' ');
        // GNU time reports the noble process tree's peak RSS (waited Node worker included).
        if (/^\d+(\.\d+)? \d+$/.test(`${seconds} ${kib}`))
          timing = { time_wall_s: Number(seconds), max_rss_kib: Number(kib) };
      } catch { /* killed before time wrote */ }
      const result = { id: session.id, category: session.category, argv, memory_max: memoryMax,
        timeout_s: timeout / 1000, stdin_sha256: sha256(stdin), stdin_bytes: stdin.length,
        lines: session.lines.map(line => typeof line === 'string' ? line
          : { repeat: line.repeat, count: line.count }),
        status, signal, timed_out: timedOut, wall_ms: wall, timing,
        stdout: Buffer.concat(stdout).toString('utf8'), stderr: Buffer.concat(stderr).toString('utf8'),
        events: events.map(event => ({ ms: Math.round(event.ms), stream: event.stream,
          text: event.text.length > 300 ? `${event.text.slice(0, 300)}…` : event.text })) };
      fs.writeFileSync(path.join(directory, 'result.json'), `${JSON.stringify(result, null, 2)}\n`);
      fs.appendFileSync(path.join(out, 'progress.jsonl'), `${JSON.stringify({ id: session.id, status, signal, timed_out: timedOut, wall_ms: wall })}\n`);
      resolve(result);
    }, 200);
  });
});

const receipt = {
  schema: 'noble-calculator-budget-brackets/v1',
  status: 'implementation-observation-not-case-evidence',
  repository: { head: git(['rev-parse', 'HEAD']), tracked_changes: git(['status', '--porcelain', '--untracked-files=no']) },
  plan_sha256: sha256(planBytes),
  runner_sha256: sha256(fs.readFileSync(new URL(import.meta.url))),
  binary: { path: binary, sha256: binaryHash, source_id: options['source-id'] },
  node: process.version,
  configuration: { concurrency: Number(options.concurrency), memory_max: options['memory-max'], timeout_s: timeoutMs / 1000 },
  non_claims: plan.non_claims,
  sessions: [],
};
const queue = [...sessions];
const workers = Array.from({ length: Number(options.concurrency) }, async () => {
  while (queue.length > 0) {
    const session = queue.shift();
    const result = await runSession(session);
    receipt.sessions.push({ ...result, check: expectations(session, result) });
    fs.writeFileSync(path.join(out, 'receipt.partial.json'), `${JSON.stringify(receipt, null, 2)}\n`);
  }
});
try {
  await Promise.all(workers);
  receipt.sessions.sort((a, b) => a.id.localeCompare(b.id));
  fs.writeFileSync(path.join(out, 'receipt.json'), `${JSON.stringify(receipt, null, 2)}\n`);
  fs.writeFileSync(path.join(out, 'ALL_DONE'), '');
} catch (error) {
  fs.writeFileSync(path.join(out, 'FAILED'), `${error.stack ?? error}\n`);
  process.exit(1);
}
