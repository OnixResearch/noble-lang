import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { receiptsBindingModel } from '../foundation/model-receipts.mjs';

export const MODEL_FILES = [
  'proofs/foundation/NobleFoundation/Binary32.lean',
  'proofs/foundation/NobleFoundation/Binary32Expectations.lean',
];
export const MODEL_NAMESPACE = 'NobleFoundation.Binary32';
export const LANE_FILE = 'crates/noble-ngraph-experiment/tests/b2_differential.rs';
export const PROPOSAL_FILE = 'verification/binary32/proposals.json';
export const PROPOSER_FILE = 'verification/binary32/propose.py';
export const RECEIPT_FILES = [
  'verification/binary32/replay.json',
  'verification/binary32/shrinker.json',
];

export function sha256(root, file) {
  return crypto.createHash('sha256').update(fs.readFileSync(path.join(root, file))).digest('hex');
}

export function plain(argv, cwd) {
  const result = spawnSync(argv[0], argv.slice(1), { cwd, encoding: 'utf8' });
  if (result.status !== 0) throw new Error(`${argv.join(' ')} failed: ${result.stderr}`);
  return result.stdout ?? '';
}

export function runDev(argv, cwd) {
  const result = spawnSync('nix', ['develop', '--command', ...argv], {
    cwd, encoding: 'utf8', timeout: 1800_000, maxBuffer: 64 * 1024 * 1024,
  });
  if (result.status !== 0) {
    const text = `${result.stdout ?? ''}${result.stderr ?? ''}`;
    throw new Error(`command failed (${result.status}): ${argv.join(' ')}\n${text.slice(-2000)}`);
  }
  return result.stdout ?? '';
}

export function proposals(root) {
  const packet = JSON.parse(fs.readFileSync(path.join(root, PROPOSAL_FILE), 'utf8'));
  if (packet.schema !== 'noble-binary32-external-proposals/v1' || !Array.isArray(packet.proposals)) {
    throw new Error('invalid binary32 proposal packet');
  }
  return packet.proposals;
}

function proposalKey(proposal) {
  return JSON.stringify([proposal.operation, proposal.left, proposal.right, proposal.expected]);
}

export function retained(root) {
  const source = fs.readFileSync(path.join(root, MODEL_FILES[1]), 'utf8');
  const marker = source.match(/def retained\s*:[^=]+:=\s*\[([\s\S]*?)\n\]/);
  if (!marker) throw new Error('could not locate Binary32.Expectations.retained');
  const entries = [...marker[1].matchAll(
    /^\s*\("(add|sub|mul)",\s*(0x[0-9A-Fa-f]+),\s*(0x[0-9A-Fa-f]+),\s*(0x[0-9A-Fa-f]+)\),?\s*(?:--.*)?$/gm,
  )].map((match) => ({
    operation: match[1],
    left: Number.parseInt(match[2], 16),
    right: Number.parseInt(match[3], 16),
    expected: Number.parseInt(match[4], 16),
  }));
  const sourceEntries = marker[1].split('\n').filter((line) => line.includes('(\"'));
  if (entries.length !== sourceEntries.length) {
    throw new Error(`unparsed retained expectation line (${entries.length}/${sourceEntries.length})`);
  }
  return entries;
}

export function binding(root) {
  const packet = proposals(root);
  const lean = retained(root);
  const packetKeys = packet.map(proposalKey).sort();
  const leanKeys = lean.map(proposalKey).sort();
  if (JSON.stringify(packetKeys) !== JSON.stringify(leanKeys)) {
    throw new Error('proposals.json and Lean retained expectations differ');
  }
  return {
    proposals_sha256: sha256(root, PROPOSAL_FILE),
    proposer_sha256: sha256(root, PROPOSER_FILE),
    retained_expectations: packet.length,
    lean_authority: receiptsBindingModel(root, MODEL_FILES, MODEL_NAMESPACE),
  };
}

export function targetMetadata(toolchain, environment) {
  const host = toolchain.match(/^host:\s*(\S+)$/m)?.[1];
  if (!host) throw new Error('rustc -vV did not report a host target');
  return {
    target_triple: environment.CARGO_BUILD_TARGET || host,
    rustc_version: toolchain,
    fp_configuration: {
      rustflags: environment.RUSTFLAGS || '',
      cargo_encoded_rustflags: environment.CARGO_ENCODED_RUSTFLAGS || '',
      cargo_incremental: environment.CARGO_INCREMENTAL || '0',
    },
  };
}

function sameJson(left, right) {
  return JSON.stringify(left) === JSON.stringify(right);
}

function verifySourceIdentity(root, receipt) {
  if (receipt.clean_source !== true) throw new Error('receipt is not marked clean_source');
  const head = plain(['git', 'rev-parse', 'HEAD'], root).trim();
  const status = plain(['git', 'status', '--porcelain', '--untracked-files=no'], root).trim();
  if (status) throw new Error(`current tracked tree is dirty: ${status}`);
  const sourceTree = plain(['git', 'rev-parse', `${receipt.source_commit}^{tree}`], root).trim();
  if (sourceTree !== receipt.source_tree) throw new Error('receipt source_commit does not name source_tree');
  if (plain(['git', 'merge-base', '--is-ancestor', receipt.source_commit, head], root) === undefined) {
    // `plain` throws for a non-zero status; this branch is intentionally unreachable.
    throw new Error('receipt source commit is not an ancestor of HEAD');
  }
  const changed = plain(['git', 'diff', '--name-only', `${receipt.source_commit}..${head}`], root)
    .split('\n').filter(Boolean);
  const unexpected = changed.filter((file) => !RECEIPT_FILES.includes(file));
  if (unexpected.length) throw new Error(`source changed after receipt: ${unexpected.join(', ')}`);
}

export function verifyCommon(root, receipt, schema) {
  if (receipt.schema !== schema) throw new Error(`receipt schema mismatch: ${receipt.schema}`);
  verifySourceIdentity(root, receipt);
  const toolchain = runDev(['rustc', '-vV'], root).trim();
  const target = targetMetadata(toolchain, process.env);
  if (!sameJson(receipt.target, target)) throw new Error('floating-point target metadata mismatch');
  const expected = binding(root);
  if (receipt.lane_sha256 !== sha256(root, LANE_FILE)) throw new Error('lane digest mismatch');
  if (receipt.proposals_sha256 !== expected.proposals_sha256) throw new Error('proposals digest mismatch');
  if (receipt.proposer_sha256 !== expected.proposer_sha256) throw new Error('proposer digest mismatch');
  if (receipt.retained_expectations !== expected.retained_expectations) {
    throw new Error('retained expectation count mismatch');
  }
  if (!sameJson(receipt.lean_authority, expected.lean_authority)) {
    throw new Error('Lean authority membership or model digest mismatch');
  }
  return { target, expected };
}
