#!/usr/bin/env node
// Executed pin controls for the issue #62 replay lanes (unit 1 of the re-pin).
//
// `refusals.mjs`, `limits.mjs` and `binding.mjs` each refuse before spawning any tool when a pinned
// artifact changed, but until now that guarantee was only a pre-run guard: no retained receipt showed
// it firing. This driver exercises it through the real lanes, in both directions:
// - the artifact changed (a copy with one flipped byte) and the pin stayed;
// - the artifact stayed and the pin changed (one hex digit).
// Each run must exit 2 with `pin-refused class=<class>` on stderr, write no receipt, and name the
// class the lane's guard enforces: `export-pin-mismatch` for an export, `tool-pin-mismatch` for a
// checker, exporter or Lean binary. A guard that accepts the real files is checked in-process, so the
// control set cannot pass by refusing everything.
import { spawnSync } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { verifyPins } from './pins.mjs';

const argv = process.argv.slice(2);
const value = (name) => {
  const index = argv.indexOf(name);
  if (index < 0 || index + 1 >= argv.length) throw new Error(`missing ${name}`);
  return argv[index + 1];
};
const here = path.dirname(fileURLToPath(import.meta.url));
const node = process.execPath;
const leanBin = path.resolve(value('--lean-bin'));
const foundation = path.resolve(value('--foundation'));
const manifest = path.resolve(value('--manifest'));
const nanoda = path.resolve(value('--nanoda'));
const exporter = path.resolve(value('--exporter'));
const large = path.resolve(value('--large-export'));
const small = path.resolve(value('--small-export'));
const work = path.resolve(value('--work'));
const output = path.resolve(value('--output'));
const pins = {
  nanoda: value('--nanoda-sha256'),
  exporter: value('--exporter-sha256'),
  lean: value('--lean-sha256'),
  large: value('--large-sha256'),
  small: value('--small-sha256'),
};
const toolchain = argv.includes('--toolchain') ? value('--toolchain') : null;

const sha256 = (data) => crypto.createHash('sha256').update(data).digest('hex');
const fileSha256 = (file) => sha256(fs.readFileSync(file));
const childEnv = {
  HOME: process.env.HOME ?? '/', LANG: 'C.UTF-8',
  PATH: [leanBin, '/run/current-system/sw/bin', '/usr/bin', '/bin'].join(':'), TMPDIR: work,
};

if (fs.existsSync(work)) throw new Error(`work directory exists: ${work}`);
fs.mkdirSync(work, { recursive: true });

// The positive side: the guard accepts exactly the real artifacts under their real pins.
const realEntries = () => [
  { role: 'checker', file: nanoda, sha256: pins.nanoda, pinClass: 'tool-pin-mismatch' },
  { role: 'exporter', file: exporter, sha256: pins.exporter, pinClass: 'tool-pin-mismatch' },
  { role: 'lean', file: path.join(leanBin, 'lean'), sha256: pins.lean, pinClass: 'tool-pin-mismatch' },
  { role: 'large export', file: large, sha256: pins.large, pinClass: 'export-pin-mismatch' },
  { role: 'small export', file: small, sha256: pins.small, pinClass: 'export-pin-mismatch' },
];
let guardAcceptsReal = true;
try {
  verifyPins(realEntries());
} catch (error) {
  guardAcceptsReal = false;
  console.error(error.message);
}

/** One hex digit of the pin changed; the artifact is untouched. */
function changedPin(digest) {
  const last = digest.at(-1);
  return `${digest.slice(0, -1)}${last === '0' ? '1' : '0'}`;
}

/** A byte-for-byte copy with one flipped bit in the middle; the pin is untouched. */
function tamperedCopy(source, name) {
  const copy = path.join(work, name);
  fs.copyFileSync(source, copy);
  const bytes = fs.readFileSync(copy);
  bytes[Math.floor(bytes.length / 2)] ^= 0x01;
  fs.writeFileSync(copy, bytes);
  return copy;
}

const lanes = {
  limits: (options) => [
    path.join(here, 'limits.mjs'),
    '--nanoda', options.nanoda ?? nanoda,
    '--nanoda-sha256', options.nanodaPin ?? pins.nanoda,
    '--large-export', large,
    '--large-sha256', pins.large,
    '--small-export', options.small ?? small,
    '--small-sha256', options.smallPin ?? pins.small,
    '--work', path.join(work, options.id),
    '--output', path.join(work, `${options.id}.json`),
  ],
  refusals: (options) => [
    path.join(here, 'refusals.mjs'),
    '--nanoda', nanoda,
    '--nanoda-sha256', pins.nanoda,
    '--exporter', options.exporter ?? exporter,
    '--exporter-sha256', options.exporterPin ?? pins.exporter,
    '--lean-bin', leanBin,
    '--lean-sha256', pins.lean,
    '--large-export', large,
    '--large-sha256', pins.large,
    '--small-export', small,
    '--small-sha256', pins.small,
    '--work', path.join(work, options.id),
    '--output', path.join(work, `${options.id}.json`),
  ],
  binding: (options) => [
    path.join(here, 'binding.mjs'),
    '--nanoda', options.nanoda ?? nanoda,
    '--nanoda-sha256', options.nanodaPin ?? pins.nanoda,
    '--exporter', exporter,
    '--exporter-sha256', pins.exporter,
    '--lean-bin', leanBin,
    '--lean-sha256', options.leanPin ?? pins.lean,
    '--foundation', foundation,
    '--manifest', manifest,
    '--large-export', large,
    '--large-sha256', pins.large,
    '--small-export', small,
    '--small-sha256', pins.small,
    '--work', path.join(work, options.id),
    '--output', path.join(work, `${options.id}.json`),
  ],
};

const controls = [
  { id: 'export-changed-limits', lane: 'limits', expected: 'export-pin-mismatch',
    changes: 'the small export is a byte-flipped copy; its pin is unchanged',
    options: { small: tamperedCopy(small, 'small-tampered.ndjson') } },
  { id: 'export-pin-changed-limits', lane: 'limits', expected: 'export-pin-mismatch',
    changes: 'the small export is unchanged; one hex digit of its pin changed',
    options: { smallPin: changedPin(pins.small) } },
  { id: 'tool-pin-changed-refusals', lane: 'refusals', expected: 'tool-pin-mismatch',
    changes: 'the exporter binary is unchanged; one hex digit of its pin changed',
    options: { exporterPin: changedPin(pins.exporter) } },
  { id: 'tool-changed-binding', lane: 'binding', expected: 'tool-pin-mismatch',
    changes: 'the checker is a byte-flipped copy; its pin is unchanged',
    options: { nanoda: tamperedCopy(nanoda, 'nanoda-tampered') } },
  { id: 'lean-pin-changed-binding', lane: 'binding', expected: 'tool-pin-mismatch',
    changes: 'the Lean binary is unchanged; one hex digit of its pin changed',
    options: { leanPin: changedPin(pins.lean) } },
];

const classify = (stderr) => {
  const match = stderr.match(/pin-refused class=(\S+)/u);
  return match ? match[1] : null;
};

const entries = controls.map((control) => {
  const laneArgv = lanes[control.lane](control.options);
  const receiptPath = path.join(work, `${control.id}.json`);
  const started = process.hrtime.bigint();
  const result = spawnSync(node, laneArgv, {
    env: childEnv, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024, timeout: 1_800_000, killSignal: 'SIGKILL',
  });
  const elapsed = Number((process.hrtime.bigint() - started) / 1_000_000n);
  const stderr = (result.stderr ?? '').trim();
  return {
    ...control,
    lane_argv: laneArgv.slice(1).map((part) => part.replace(work, '$WORK')),
    exit_code: result.status,
    signal: result.signal,
    elapsed_ms: elapsed,
    classified: classify(stderr),
    receipt_written: fs.existsSync(receiptPath),
    stderr_first_line: stderr.split('\n')[0]?.slice(0, 300) ?? '',
  };
});

const checks = {
  guard_accepts_the_real_artifacts: guardAcceptsReal,
  every_control_refused_with_exit_2: entries.every((entry) => entry.exit_code === 2 && !entry.signal),
  every_control_in_its_expected_class: entries.every((entry) => entry.classified === entry.expected),
  no_control_wrote_a_receipt: entries.every((entry) => !entry.receipt_written),
  both_artifact_and_pin_changes_are_covered:
    entries.some((entry) => entry.changes.includes('byte-flipped'))
    && entries.some((entry) => entry.changes.includes('one hex digit')),
};
const holds = Object.values(checks).every(Boolean);

const host = spawnSync('uname', ['-srm'], { encoding: 'utf8' }).stdout.trim();
const source = spawnSync('git', ['rev-parse', 'HEAD'], { cwd: process.cwd(), encoding: 'utf8' });
if (source.status !== 0 || source.stdout.trim() === '') {
  throw new Error(`cannot identify receipt source commit: ${source.stderr.trim()}`);
}
const receipt = {
  schema: 'noble-independent-replay-pin-controls/v1',
  measured_on: new Date().toISOString().slice(0, 10),
  host,
  source_commit: source.stdout.trim(),
  toolchain,
  assertion: 'a changed export, a changed tool binary and a changed pin each end in a named pin refusal through the real lane, before any tool runs and without writing a receipt, while the guard accepts the real artifacts under their real pins',
  holds,
  checks,
  harness: {
    path: 'verification/independent-replay/pin-controls.mjs',
    sha256: fileSha256(fileURLToPath(import.meta.url)),
    guard: { path: 'verification/independent-replay/pins.mjs', sha256: fileSha256(path.join(here, 'pins.mjs')) },
  },
  controls: entries,
  pins: {
    nanoda: pins.nanoda, exporter: pins.exporter, lean: pins.lean,
    large: pins.large, small: pins.small,
  },
  non_claims: [
    'The controls show the guard refuses; they say nothing about the lanes\' checker verdicts.',
    'A changed pin argument and a changed artifact are exercised; a compromised guard itself is not.',
    'No CI, flake, policy or gate change; promotion is an owner decision.',
  ],
};
fs.mkdirSync(path.dirname(output), { recursive: true });
fs.writeFileSync(output, `${JSON.stringify(receipt, null, 2)}\n`);
console.log(JSON.stringify({ holds, checks, controls: entries.map((entry) => `${entry.id}:${entry.classified}`) }));
process.exitCode = holds ? 0 : 1;
