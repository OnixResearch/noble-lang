// Evidence-level binding for the replayable-certificate lane (issue #42).
//
// The certificate lane has two levels of "stale model" control:
//
// * the certificate itself binds `CERT_REVISION` and `MODEL_REVISION`, and `cert::check` refuses a
//   stale token (`Refusal::StaleRevision`, `Refusal::StaleModel`), mirrored in Lean by
//   `Certificate.stale_*` and `CertificateCodec.stale_*_token_refused`;
// * this file binds the *evidence*: the exact bytes of the Rust implementation and of the three
//   Lean modules (format, codec, generated corpus) that describe it, and requires a clean-source
//   foundation receipt that hashes those module bytes and carries the named theorems.
//
// A model change without a retained receipt refreshes nothing: `--verify` refuses, naming the
// missing receipt. An implementation change without a re-written binding refuses too. Neither
// refusal is a correctness claim; they say the evidence no longer describes the bytes.
//
// usage:
//   node verification/ngraph/cert-binding.mjs --write
//   node verification/ngraph/cert-binding.mjs --verify [--receipt <foundation-check.json | directory>]
import fs from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { receiptsBindingModel } from '../foundation/model-receipts.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const BINDING = 'verification/ngraph/cert-binding.json';
const SCHEMA = 'noble-cert-binding/v1';
const MODEL_NAMESPACE = 'NobleFoundation.Certificate';

// The three Lean modules the certificate lane's claims live in: the `ngraph-cert/0` format, its
// token codec and the generated corpus of the real Rust fixture.
const MODEL_FILES = Object.freeze([
  'proofs/foundation/NobleFoundation/Certificate.lean',
  'proofs/foundation/NobleFoundation/CertificateCodec.lean',
  'proofs/foundation/NobleFoundation/CertificateCorpus.lean',
]);

// The Rust bytes the model describes, and the fixture and generator the corpus is written from.
const IMPLEMENTATION_FILES = Object.freeze([
  'crates/noble-ngraph/src/cert.rs',
  'crates/noble-ngraph/tests/cert/differential.rs',
  'crates/noble-ngraph/tests/cert/corpus.rs',
]);

// Theorems a receipt covering this lane must retain: the replay endpoint theorem and its
// rule-theorem consequence, the model-revision refusals in both the format and codec models, the
// codec round trip, and the kernel-checked statements about this exact fixture and its tokens.
const REQUIRED_THEOREMS = Object.freeze([
  'NobleFoundation.Certificate.accepted_endpoint_equivalence',
  'NobleFoundation.Certificate.accepted_fold_step_denotes',
  'NobleFoundation.Certificate.stale_revision_refused',
  'NobleFoundation.Certificate.stale_model_refused',
  'NobleFoundation.Certificate.stale_model_kills_acceptance',
  'NobleFoundation.CertificateCodec.decode_encode_replays',
  'NobleFoundation.CertificateCodec.acceptance_preserved_by_codec',
  'NobleFoundation.CertificateCodec.stale_revision_token_refused',
  'NobleFoundation.CertificateCodec.stale_model_token_refused',
  'NobleFoundation.Certificate.Corpus.wire_envelope_decodes',
  'NobleFoundation.Certificate.Corpus.derivation_is_accepted',
  'NobleFoundation.Certificate.Corpus.replay_matches_the_rust_result',
  'NobleFoundation.Certificate.Corpus.reference_outputs_at_zero',
  'NobleFoundation.Certificate.Corpus.reference_outputs_at_boundary',
  'NobleFoundation.Certificate.Corpus.stale_model_token_refused',
  'NobleFoundation.Certificate.Corpus.stale_revision_token_refused',
]);

const sha256 = (file) => createHash('sha256').update(fs.readFileSync(path.join(root, file))).digest('hex');

function requireThat(condition, message) {
  if (!condition) throw Error(message);
}

function digests(files) {
  return Object.fromEntries(files.map((file) => [file, sha256(file)]));
}

function gitHead() {
  const result = spawnSync('git', ['rev-parse', 'HEAD'], {
    cwd: root,
    encoding: 'utf8',
    env: { PATH: '/run/current-system/sw/bin:/usr/bin:/bin', GIT_CONFIG_NOSYSTEM: '1',
      GIT_CONFIG_GLOBAL: '/dev/null', GIT_TERMINAL_PROMPT: '0' },
  });
  requireThat(result.status === 0, `git rev-parse HEAD failed: ${result.stderr ?? result.status}`);
  return result.stdout.trim();
}

function readReceiptFile(file) {
  const receipt = JSON.parse(fs.readFileSync(file, 'utf8'));
  requireThat(receipt.schema === 'noble-foundation-check/v1', `${file} is not a foundation receipt`);
  requireThat(receipt.clean_source === true, `${file} is not a clean-source receipt`);
  return receipt;
}

/** A receipt named explicitly on the command line: a `foundation-check.json`, or its directory. */
function explicitReceipt(target) {
  const file = fs.statSync(target).isDirectory() ? path.join(target, 'foundation-check.json') : target;
  const receipt = readReceiptFile(file);
  const current = digests(MODEL_FILES);
  for (const module of MODEL_FILES) {
    requireThat(receipt.sources_sha256?.[module] === current[module],
      `receipt ${file} does not hash the current ${module}`);
  }
  return { receipt: file, source_commit: receipt.source_commit, theorems: receipt.theorems ?? [] };
}

/** The retained receipts that bind these exact model bytes, through the repository's helper. */
function retainedReceipts() {
  const binding = receiptsBindingModel(root, MODEL_FILES, MODEL_NAMESPACE);
  const receipts = binding.receipts.map((entry) => {
    const receipt = readReceiptFile(path.join(root, entry.receipt));
    return { receipt: entry.receipt, source_commit: entry.source_commit, theorems: receipt.theorems ?? [] };
  });
  return { digest: binding.model_files_sha256, receipts };
}

function verify(options) {
  requireThat(fs.existsSync(path.join(root, BINDING)), `missing ${BINDING}; run --write`);
  const binding = JSON.parse(fs.readFileSync(path.join(root, BINDING), 'utf8'));
  requireThat(binding.schema === SCHEMA, `${BINDING} schema is ${binding.schema}`);
  requireThat(typeof binding.source_commit === 'string' && binding.source_commit.length > 0,
    `${BINDING} names no source commit`);
  const current = digests(IMPLEMENTATION_FILES);
  for (const file of IMPLEMENTATION_FILES) {
    requireThat(binding.files?.[file] === current[file],
      `${file} changed since ${BINDING} was written; the bound evidence describes other bytes`);
  }
  const modeled = options.receipt === null ? retainedReceipts() : { receipts: [explicitReceipt(options.receipt)] };
  const modelDigests = digests(MODEL_FILES);
  for (const receipt of modeled.receipts) {
    const names = new Set(receipt.theorems.map((theorem) => theorem.name));
    const missing = REQUIRED_THEOREMS.filter((name) => !names.has(name));
    requireThat(missing.length === 0,
      `receipt ${receipt.receipt} lacks ${missing.join(', ')}; regenerate the foundation receipt`);
    for (const theorem of receipt.theorems.filter((entry) => REQUIRED_THEOREMS.includes(entry.name))) {
      requireThat(theorem.status === 'proved', `${theorem.name} is ${theorem.status}`);
    }
  }
  return {
    schema: SCHEMA,
    binding_source_commit: binding.source_commit,
    implementation_sha256: current,
    model_sha256: modelDigests,
    receipts: modeled.receipts.map((receipt) => receipt.receipt),
    required_theorems: REQUIRED_THEOREMS.length,
  };
}

function write() {
  const record = {
    schema: SCHEMA,
    source_commit: gitHead(),
    note: 'Binds the Rust certificate bytes to the Lean format, codec and generated corpus modules. '
      + '`node verification/ngraph/cert-binding.mjs --verify` additionally requires a clean-source '
      + 'foundation receipt that hashes the current module bytes and retains the named theorems.',
    files: digests(IMPLEMENTATION_FILES),
    model_files: MODEL_FILES,
  };
  fs.writeFileSync(path.join(root, BINDING), `${JSON.stringify(record, null, 2)}\n`);
  return { schema: SCHEMA, source_commit: record.source_commit, files: Object.keys(record.files) };
}

function parseArgs(argv) {
  if (argv.length === 1 && argv[0] === '--write') return { mode: 'write', receipt: null };
  if (argv[0] === '--verify') {
    const index = argv.indexOf('--receipt');
    if (index < 0) {
      requireThat(argv.length === 1, `unknown arguments: ${argv.slice(1).join(' ')}`);
      return { mode: 'verify', receipt: null };
    }
    requireThat(argv.length === 3 && index === 1, 'usage: --verify [--receipt <file-or-directory>]');
    return { mode: 'verify', receipt: path.resolve(argv[2]) };
  }
  throw Error('usage: node verification/ngraph/cert-binding.mjs --write | --verify [--receipt <file-or-directory>]');
}

try {
  const options = parseArgs(process.argv.slice(2));
  const result = options.mode === 'write' ? write() : verify(options);
  console.log(JSON.stringify(result, null, 2));
} catch (error) {
  console.error(`[cert-binding] FAIL ${error.message}`);
  process.exitCode = 1;
}
