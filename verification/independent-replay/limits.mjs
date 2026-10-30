#!/usr/bin/env node
// Issue #62 limit controls for the independent-kernel pilot.
//
// Runs the pinned nanoda checker on a large pinned export and a small retained one: unbounded, under a
// wall-clock bound well below the large run's measured time, and under an address-space bound found
// from the small export. Each run is classified from its exit status, signal and output alone.
//
// The control holds only when the large export is accepted unbounded and refused under each bound,
// while the small export is accepted under the same bounds, so a refusal is the bound acting on
// checker work rather than a bound that refuses everything. A bound that does not bind, or a bounded
// run classified as accepted, fails the control. This is pilot evidence, not a gate.
import { spawnSync } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';

const argv = process.argv.slice(2);
const value = (name) => {
  const index = argv.indexOf(name);
  if (index < 0 || index + 1 >= argv.length) throw new Error(`missing ${name}`);
  return argv[index + 1];
};
const nanoda = path.resolve(value('--nanoda'));
const large = path.resolve(value('--large-export'));
const small = path.resolve(value('--small-export'));
const work = path.resolve(value('--work'));
const output = path.resolve(value('--output'));
const pins = {
  nanoda: value('--nanoda-sha256'),
  large: value('--large-sha256'),
  small: value('--small-sha256'),
};

const sha256 = (file) => crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
// Refuse before any checker process runs when a pinned input changed.
for (const [name, file] of [['nanoda', nanoda], ['large', large], ['small', small]]) {
  const actual = sha256(file);
  if (actual !== pins[name]) throw new Error(`${name} digest ${actual} differs from the pin ${pins[name]}`);
}

fs.mkdirSync(work, { recursive: true });
function config(exportPath) {
  const file = path.join(work, `nanoda-limits-${path.basename(exportPath, '.ndjson')}.json`);
  fs.writeFileSync(file, `${JSON.stringify({
    use_stdin: false,
    export_file_path: exportPath,
    permitted_axioms: ['propext', 'Classical.choice', 'Quot.sound'],
    unpermitted_axiom_hard_error: true,
    num_threads: 1,
    nat_extension: true,
    string_extension: true,
    print_axioms: false,
    print_success_message: true,
  }, null, 2)}\n`);
  return file;
}
const configs = { large: config(large), small: config(small) };

function classify(result) {
  if (result.error?.code === 'ETIMEDOUT') return 'timeout';
  const text = `${result.stdout ?? ''}\n${result.stderr ?? ''}`;
  if (result.status === 0 && /Checked \d+ declarations with no errors/u.test(result.stdout ?? '')) {
    return 'accepted';
  }
  if (result.signal === 'SIGABRT' || result.status === 134
      || /memory allocation of \d+ bytes failed|Cannot allocate memory|out of memory/iu.test(text)) {
    return 'resource-exhausted';
  }
  return 'refused-other';
}

function run(name, subject, { timeoutMs = 0, addressSpaceBytes = 0 } = {}) {
  const argvRun = addressSpaceBytes > 0
    ? ['prlimit', `--as=${addressSpaceBytes}`, '--', nanoda, configs[subject]]
    : [nanoda, configs[subject]];
  const started = process.hrtime.bigint();
  const result = spawnSync(argvRun[0], argvRun.slice(1), {
    encoding: 'utf8',
    maxBuffer: 64 * 1024 * 1024,
    timeout: timeoutMs > 0 ? timeoutMs : 1_800_000,
    killSignal: 'SIGKILL',
  });
  const elapsedMs = Number((process.hrtime.bigint() - started) / 1_000_000n);
  return {
    name,
    subject,
    timeout_ms: timeoutMs || null,
    address_space_bytes: addressSpaceBytes || null,
    exit_code: result.status,
    signal: result.signal,
    elapsed_ms: elapsedMs,
    classification: classify(result),
    stdout: (result.stdout ?? '').trim().slice(0, 300),
    stderr_excerpt: (result.stderr ?? '').trim().slice(-300),
  };
}

const runs = [];
const unbounded = run('unbounded', 'large');
runs.push(unbounded);
runs.push(run('unbounded', 'small'));

// The wall-clock bound is one tenth of the large export's unbounded time.
const timeoutMs = Math.max(1, Math.floor(unbounded.elapsed_ms / 10));
runs.push(run('time-bound', 'large', { timeoutMs }));
runs.push(run('time-bound', 'small', { timeoutMs }));

// The address-space bound is the smallest candidate under which the small export is accepted.
const candidates = [4, 6, 8, 12, 16, 24, 32, 48, 64, 96, 128].map((mib) => mib * 1024 * 1024);
let addressSpaceBytes = 0;
const search = [];
for (const bytes of candidates) {
  const probe = run('address-space-probe', 'small', { addressSpaceBytes: bytes });
  search.push({ address_space_bytes: bytes, classification: probe.classification, exit_code: probe.exit_code });
  if (probe.classification === 'accepted') {
    addressSpaceBytes = bytes;
    runs.push({ ...probe, name: 'address-space-bound' });
    break;
  }
}
if (addressSpaceBytes > 0) runs.push(run('address-space-bound', 'large', { addressSpaceBytes }));

const find = (name, subject) => runs.find((entry) => entry.name === name && entry.subject === subject);
const checks = {
  large_unbounded_accepted: find('unbounded', 'large')?.classification === 'accepted',
  small_unbounded_accepted: find('unbounded', 'small')?.classification === 'accepted',
  large_time_bound_refused: find('time-bound', 'large')?.classification === 'timeout',
  small_time_bound_accepted: find('time-bound', 'small')?.classification === 'accepted',
  address_space_bound_found: addressSpaceBytes > 0,
  large_address_space_bound_refused: ['resource-exhausted', 'refused-other']
    .includes(find('address-space-bound', 'large')?.classification),
  no_bounded_large_run_accepted: runs
    .filter((entry) => entry.subject === 'large' && (entry.timeout_ms || entry.address_space_bytes))
    .every((entry) => entry.classification !== 'accepted'),
};
const holds = Object.values(checks).every(Boolean);

const host = spawnSync('uname', ['-srm'], { encoding: 'utf8' }).stdout.trim();
const receipt = {
  schema: 'noble-independent-replay-limits/v1',
  measured_on: new Date().toISOString().slice(0, 10),
  host,
  assertion: 'under a wall-clock bound and an address-space bound the pinned nanoda checker run on the pinned large export ends in a refusal class, never acceptance, while the small retained export is accepted under the same bounds',
  holds,
  checks,
  tools: { nanoda: { binary_sha256: pins.nanoda, num_threads: 1 } },
  exports: {
    large: { root: 'NobleFoundation.toInt_add', sha256: pins.large, size_bytes: fs.statSync(large).size, retained: false },
    small: {
      root: 'NobleFoundation.LinkBoundary.vertical_simulation_compose',
      sha256: pins.small,
      size_bytes: fs.statSync(small).size,
      retained: true,
      path: 'verification/independent-replay/receipts/2026-10-29/vertical_simulation_compose.ndjson',
    },
  },
  bounds: { timeout_ms: timeoutMs, address_space_bytes: addressSpaceBytes || null, address_space_search: search },
  runs,
  non_claims: [
    'Pilot evidence on one host; the bounds are chosen from this run, not a portable resource contract.',
    'A refusal under a bound says nothing about nanoda correctness, completeness or equivalence to the Lean kernel.',
    'Unsupported-construct and export-failure refusal classes are not exercised here.',
    'No CI, flake, policy or gate change; promotion is an owner decision.',
  ],
};
fs.mkdirSync(path.dirname(output), { recursive: true });
fs.writeFileSync(output, `${JSON.stringify(receipt, null, 2)}\n`);
console.log(JSON.stringify({ holds, checks, bounds: receipt.bounds }));
process.exitCode = holds ? 0 : 1;
