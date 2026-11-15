#!/usr/bin/env node
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const CRITERIA = 'verification/procedure-library/criteria-producer-targets-v3.json';
const MANIFEST = 'verification/procedure-library/producer-targets-v3.txt';
const PRODUCER = 'verification/procedure-library/receipts/producer-targets-v3.json';
const ACQUISITION = 'verification/synthesis/receipts/2026-11-13/synthesis-v2.json';
const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const read = relative => fs.readFileSync(path.join(ROOT, relative));

function flags() {
  const args = process.argv.slice(2);
  assert.equal(args.length, 8, 'usage: producer-transfer.mjs --cargo ABS --target ABS --output ABS --raw ABS');
  const result = {};
  for (let at = 0; at < args.length; at += 2) {
    assert.ok(['--cargo', '--target', '--output', '--raw'].includes(args[at]) && !(args[at] in result),
      `unknown or repeated argument ${args[at]}`);
    assert.ok(path.isAbsolute(args[at + 1]), `${args[at]} must be absolute`);
    result[args[at]] = args[at + 1];
  }
  for (const item of ['--output', '--raw']) {
    assert.ok(!fs.existsSync(result[item]), `${item} must not exist`);
    assert.ok(!path.resolve(result[item]).startsWith(ROOT + path.sep), `${item} must be outside the source tree`);
  }
  return result;
}

function git(args) {
  const output = spawnSync('git', args, { cwd: ROOT, encoding: 'utf8' });
  assert.equal(output.status, 0, `git ${args.join(' ')}: ${output.stderr}`);
  return output.stdout.trim();
}

function main() {
  const options = flags();
  const sourceCommit = git(['rev-parse', 'HEAD']);
  const sourceTree = git(['rev-parse', 'HEAD^{tree}']);
  assert.equal(git(['status', '--porcelain', '--untracked-files=all']), '', 'source tree must be clean');
  const criteriaBytes = read(CRITERIA);
  const criteria = JSON.parse(criteriaBytes);
  assert.equal(criteria.schema, 'noble-procedure-library-producer-targets-criteria/3');
  assert.equal(criteria.producer.reference_derivation.source_sha256, digest(read(ACQUISITION)),
    'the frozen v3 reference derivation no longer binds the retained #116 source');
  assert.equal(criteria.producer.manifest, MANIFEST);
  const producerBytes = read(PRODUCER);
  const producer = JSON.parse(producerBytes);
  assert.equal(producer.schema, 'noble-synthesis-receipt/v1');
  assert.equal(producer.source.manifest, MANIFEST);
  assert.equal(producer.source.manifest_sha256, digest(read(MANIFEST)), 'producer manifest binding drifted');
  assert.equal(producer.source.clean, true, 'producer receipt was not source-bound');
  const testName = 'procedures::transfer::tests::producer_generated_heldout_measurement_once';
  const command = [
    'test', '-p', 'noble-contracts', '--test', 'editor', testName, '--offline', '--locked',
    '--', '--ignored', '--exact', '--nocapture',
  ];
  const rawFd = fs.openSync(options['--raw'], 'wx', 0o600);
  let execution;
  try {
    execution = spawnSync(options['--cargo'], command, {
      cwd: ROOT,
      env: { ...process.env, NOBLE_ROOT: ROOT, NOBLE_TARGET: options['--target'] },
      stdio: ['ignore', rawFd, rawFd],
      timeout: 30 * 60 * 1000,
    });
  } finally {
    fs.closeSync(rawFd);
  }
  const raw = fs.readFileSync(options['--raw']);
  const markers = [...raw.toString('utf8').matchAll(/^PRODUCER_TRANSFER_JSON=(\{.*\})$/gm)];
  const report = markers.length === 1 ? JSON.parse(markers[0][1]) : null;
  const decision = execution.status === 0 && report?.decision === 'GO' ? 'GO' : 'NO-GO';
  const receipt = {
    schema: 'noble-procedure-library-producer-transfer-receipt/1',
    source_commit: sourceCommit,
    source_tree: sourceTree,
    clean_source: true,
    criteria: CRITERIA,
    criteria_sha256: digest(criteriaBytes),
    producer_receipt: PRODUCER,
    producer_sha256: digest(producerBytes),
    producer_source_commit: producer.source.commit,
    command: { binary: options['--cargo'], args: command, target: options['--target'] },
    raw_sha256: digest(raw),
    exit_status: execution.status,
    execution_error: execution.error?.message ?? null,
    report,
    decision,
    non_claims: [
      'A producer candidate is a finite target oracle only where its generated body agrees with the declared reference; the source library never stores that body.',
      'The deliberate proposer and teacher requests run in an isolated negative-control library, not inside scored transfer arms.',
      'This one-shot receipt cannot revise the preceding frozen criterion or receipt.',
    ],
  };
  fs.writeFileSync(options['--output'], `${JSON.stringify(receipt, null, 2)}\n`, { flag: 'wx', mode: 0o600 });
  process.stdout.write(`${JSON.stringify({ source_commit: sourceCommit, criteria_sha256: receipt.criteria_sha256,
    producer_sha256: receipt.producer_sha256, raw_sha256: receipt.raw_sha256,
    exit_status: execution.status, decision, output: options['--output'] })}\n`);
  if (decision !== 'GO') process.exitCode = 1;
}

main();
