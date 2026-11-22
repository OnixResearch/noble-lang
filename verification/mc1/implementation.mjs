#!/usr/bin/env bun
// Source binding + compiled theorem audit. Separate from application proof checking.
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const argument = process.argv[2];
if (!argument || argument === '--help' || process.argv.length !== 3) {
  console.log('usage: bun verification/mc1/implementation.mjs NEW_ARTIFACT_DIRECTORY');
  process.exit(argument === '--help' ? 0 : 2);
}
const artifacts = path.resolve(argument);
fs.mkdirSync(path.dirname(artifacts), { recursive: true });
fs.mkdirSync(artifacts); // Never overwrite a previous run.
const selection = JSON.parse(fs.readFileSync(path.join(root, 'policy/tool-selection.json'), 'utf8'));
const tool = (name, binary) => path.join(selection.tool_paths[name].output, 'bin', binary);
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const fileSha = file => sha(fs.readFileSync(file));

function files(directory) {
  return fs.readdirSync(path.join(root, directory), { withFileTypes: true }).flatMap(entry => {
    if (entry.name.startsWith('.') || ['target', 'node_modules'].includes(entry.name)) return [];
    const item = path.posix.join(directory, entry.name);
    if (entry.isSymbolicLink()) throw Error(`unbound source symlink: ${item}`);
    return entry.isDirectory() ? files(item) : [item];
  });
}
function snapshot() {
  const inputs = new Set([
    ...Object.keys(selection.file_sha256), 'policy/tool-selection.json',
    'verification/mc1/implementation.mjs',
    ...files('crates/noble-kernel/src'), ...files('crates/noble-contracts/src'),
    ...files('proofs/mc1'), ...files('proofs/m3/NobleKernel'),
    'proofs/m3/NobleKernel.lean',
    ...files('verification/mc1').filter(file => /\.(noble-contract|lean)$/.test(file)),
  ]);
  return Object.fromEntries([...inputs].sort().map(file => [file, fileSha(path.join(root, file))]));
}
const sourceFiles = snapshot();
for (const [file, expected] of Object.entries(selection.file_sha256)) {
  if (sourceFiles[file] !== expected) throw Error(`stale selected configuration: ${file}`);
}
for (const name of ['CHARON_EXE', 'AENEAS_EXE', 'LEAN_PATH', 'LEAN_SRC_PATH', 'LEAN_SYSROOT',
  'LAKE_HOME', 'RUSTC', 'RUSTC_WRAPPER', 'RUSTC_WORKSPACE_WRAPPER', 'RUSTFLAGS',
  'CARGO_ENCODED_RUSTFLAGS', 'RUSTUP_TOOLCHAIN', 'CHARON_IS_SYMLINK']) {
  if (process.env[name]) throw Error(`unreviewed tool override: ${name}`);
}
const environment = { ...process.env, RUSTC_WRAPPER: '',
  PATH: `${selection.tool_paths.extraction_rust.output}/bin:${selection.tool_paths.lean.output}/bin:${process.env.PATH}`,
  CARGO_TARGET_DIR: path.join(artifacts, 'cargo-target') };
const commands = [];
function run(label, binary, args, cwd = root, timeout = selection.configuration.lean_timeout_seconds * 1000) {
  const result = spawnSync(binary, args, { cwd, env: environment, encoding: 'utf8',
    timeout, maxBuffer: 16 * 1024 * 1024 });
  const output = (result.stdout ?? '') + (result.stderr ?? '');
  fs.writeFileSync(path.join(artifacts, `${label}.log`), output);
  commands.push({ label, binary, args, cwd: path.relative(root, cwd) || '.',
    exit: result.status, signal: result.signal, output_sha256: sha(output) });
  if (result.error || result.status !== 0) throw Error(`${label} failed; see ${label}.log: ${result.error ?? result.status}`);
  return output;
}
function identical(actual, expected) {
  if (!fs.readFileSync(actual).equals(fs.readFileSync(expected))) {
    throw Error(`stale generated implementation: ${path.relative(root, expected)}`);
  }
}
function identicalEntry(actual, expected) {
  // The inherited entrypoint carries the documented regeneration runbook as
  // line comments. It may contain only the freshly generated import otherwise.
  const imports = file => fs.readFileSync(file, 'utf8').split('\n')
    .filter(line => line.trim() && !line.trimStart().startsWith('--')).join('\n');
  if (imports(actual) !== imports(expected)) throw Error(`changed generated entrypoint: ${expected}`);
}
// The reviewed crate-local opaque functions of the contracts subject (#97), each
// with the Lean model that replaces its body and why the model is faithful.
// The extraction must report exactly these Rust names as local and opaque.
const reviewedContractsOpaque = [
  { rust: 'noble_contracts::intrinsic::debug_form_kind',
    model: 'proofs/mc1/NobleContractImpl/FunsExternal.lean intrinsic.debug_form_kind',
    reason: 'Derived-Debug rendering of FormKind, opaque only to break the Form/FormKind trait-impl cycle Aeneas refuses; in the formatter model every write and every composed leaf Debug succeeds and returns the formatter, so the model is that successful step (as the five kernel debug helpers).' },
  { rust: 'noble_contracts::intrinsic::debug_proof_dependency',
    model: 'proofs/mc1/NobleContractImpl/FunsExternal.lean intrinsic.debug_proof_dependency',
    reason: 'Derived-Debug rendering of ProofDependency, opaque only because a transparent body formats Vec<ProofDependency> through the instance it defines; same formatter-model step.' },
  { rust: 'noble_contracts::source::editor::debug_node',
    model: 'proofs/mc1/NobleContractImpl/FunsExternal.lean source.editor.debug_node',
    reason: 'Derived-Debug rendering of editor Node, opaque only because Quotation formats nested nodes through the instance it defines; same formatter-model step.' },
  { rust: "noble_contracts::source::chunks::{noble_contracts::source::chunks::ObserverHandle<'_0>}::observe",
    model: 'proofs/mc1/NobleContractImpl/FunsExternal.lean source.chunks.ObserverHandle.observe',
    reason: 'Calls the caller-owned dyn FnMut observer, which the Rust source does not determine; the handle carries the host as a prophecy sequence of call results, so every observer is represented.' },
  { rust: "noble_contracts::source::chunks::{noble_contracts::source::chunks::RankerHandle<'_0>}::model_identity",
    model: 'proofs/mc1/NobleContractImpl/FunsExternal.lean source.chunks.RankerHandle.model_identity',
    reason: 'Calls the host dyn CandidateRanker (&self); the handle carries the answer for each scorer state.' },
  { rust: "noble_contracts::source::chunks::{noble_contracts::source::chunks::RankerHandle<'_0>}::provenance_matches",
    model: 'proofs/mc1/NobleContractImpl/FunsExternal.lean source.chunks.RankerHandle.provenance_matches',
    reason: 'Calls the host dyn CandidateRanker (&self); the handle carries the answer as a function of the scorer state and of an injective image of the RankingIdentity.' },
  { rust: "noble_contracts::source::chunks::{noble_contracts::source::chunks::RankerHandle<'_0>}::score",
    model: 'proofs/mc1/NobleContractImpl/FunsExternal.lean source.chunks.RankerHandle.score',
    reason: 'Calls the host dyn CandidateRanker (&mut self); the handle carries the prophecy sequence of replies, so every scorer is represented.' },
  { rust: 'noble_contracts::source::chunks::{noble_contracts::source::chunks::MonotonicClock}::now_ns',
    model: 'proofs/mc1/NobleContractImpl/FunsExternal.lean source.chunks.MonotonicClock.now_ns',
    reason: 'Calls the host fn() -> u64 clock, whose readings the Rust source does not determine; the handle carries the prophecy sequence of readings.' },
  { rust: 'noble_contracts::source::procedures::acceptance::{noble_contracts::source::procedures::acceptance::OwnerVerifier}::verify',
    model: 'proofs/mc1/NobleContractImpl/FunsExternal.lean source.procedures.acceptance.OwnerVerifier.verify',
    reason: "Calls the owner's dyn EvidenceVerifier (&self); the handle carries the answer as a function of injective images of the record and the claim." },
  { rust: 'noble_contracts::source::procedures::acceptance::{impl core::fmt::Debug for noble_contracts::source::procedures::acceptance::OwnerVerifier}::fmt',
    model: 'proofs/mc1/NobleContractImpl/FunsExternal.lean source.procedures.acceptance.OwnerVerifier.Insts.CoreFmtDebug.fmt',
    reason: "Formats through the owner's boxed verifier; the handle carries that Debug result under the abstract formatter." },
];
function extract(label, manifest, basename, checkedDirectory, allowedOpaque) {
  const directory = path.join(artifacts, label);
  fs.mkdirSync(directory);
  const llbc = path.join(directory, `${basename}.llbc`);
  const generated = path.join(directory, 'generated');
  fs.mkdirSync(generated);
  run(`${label}-charon`, tool('charon', 'charon'), ['cargo', ...selection.configuration.charon_args,
    '--dest-file', llbc, '--', '--manifest-path', manifest, ...selection.configuration.cargo_args],
  root, selection.configuration.charon_timeout_seconds * 1000);
  run(`${label}-aeneas`, tool('aeneas', 'aeneas'), [...selection.configuration.aeneas_args,
    '-split-files', '-gen-lib-entry', '-all-computable', '-dest', generated, llbc],
  root, selection.configuration.translator_timeout_seconds * 1000);
  const prefix = label === 'kernel' ? 'NobleKernel' : 'NobleContractImpl';
  for (const file of ['Types.lean', 'Funs.lean']) {
    identical(path.join(generated, file), path.join(root, checkedDirectory, file));
  }
  identicalEntry(path.join(generated, `${prefix}.lean`), path.join(root, `${checkedDirectory}.lean`));
  const translation = JSON.parse(fs.readFileSync(path.join(generated, 'translation.json'), 'utf8'));
  const opaque = translation.functions.filter(item => item.is_local && item.is_opaque).map(item => item.rust_name).sort();
  if (JSON.stringify(opaque) !== JSON.stringify([...allowedOpaque].sort())) {
    throw Error(`unreviewed local extraction boundaries in ${label}: ${JSON.stringify(opaque)}`);
  }
  return { llbc_sha256: fileSha(llbc), translation_sha256: fileSha(path.join(generated, 'translation.json')),
    generated: Object.fromEntries(['Types.lean', 'Funs.lean', `${prefix}.lean`]
      .map(file => [file, fileSha(path.join(generated, file))])),
    local_functions: translation.functions.filter(item => item.is_local).length,
    local_opaque_functions: opaque,
    external_functions: translation.functions.filter(item => !item.is_local)
      .map(item => ({ rust: item.rust_name, lean: item.lean_name })) };
}

try {
  const versions = {
    rust: run('rust-version', tool('extraction_rust', 'rustc'), ['--version', '--verbose']),
    charon: run('charon-version', tool('charon', 'charon'), ['version']),
    aeneas: run('aeneas-version', tool('aeneas', 'aeneas'), ['-version']),
    lean: run('lean-version', tool('lean', 'lean'), ['--version']),
  };
  const kernel = extract('kernel', 'crates/noble-kernel/Cargo.toml', 'noble_kernel',
    'proofs/m3/NobleKernel', [
      'noble_kernel::types::impls::clone_stack', 'noble_kernel::types::impls::debug_stack',
      'noble_kernel::shapes::impls::clone_parts', 'noble_kernel::shapes::impls::debug_parts',
      'noble_kernel::shapes::impls::debug_slots',
    ]);
  const contracts = extract('contracts', 'crates/noble-contracts/Cargo.toml', 'noble_contract_impl',
    'proofs/mc1/NobleContractImpl', reviewedContractsOpaque.map(entry => entry.rust));
  const proofRoot = path.join(root, 'proofs/mc1');
  // Build the `NobleContracts` library and the extracted `NobleContractImpl`
  // modules; neither verdict may depend on pre-existing object files. The
  // library's globs (`lakefile.toml`) cover every `NobleContracts` submodule,
  // including `NobleContracts.Refutation`, which is outside the root import
  // closure, so no explicit submodule targets are needed here.
  run('lean-build', tool('lean', 'lake'), ['build', 'NobleContracts',
    'NobleContractImpl', 'NobleContractImpl.Projection', 'NobleContractImpl.Statement'], proofRoot);
  // This CLI-embedded witness is a standalone file, not a lake library target.
  run('intrinsic-type-witness', tool('lean', 'lake'), ['env', 'lean', 'IntrinsicTypeWitness.lean'], proofRoot);
  const library = run('contract-library', tool('lean', 'lake'), ['env', 'lean', 'ContractGate.lean'], proofRoot);
  // The contract library grew with NamedV3 (T1/T6), the specification-quality module
  // (#65), the submission-refusal witnesses for #65 and the recheck-during-shrinking lane
  // for #65's residual, then with the 23 `wire@1` law theorems of the Wire module, the
  // ten PAUTO-FO-4 refutation-template theorems (the template, its two instances, and their
  // evaluation and counterexample lemmas), and NamedV3 T2-T5 (entry embedding, call frame,
  // MC1 claim lift, call chains) together with the six reviewed rule identities
  // (`List.append_assoc`, `List.nil_append`, `List.append_nil`, `BitVec.add_assoc`,
  // `BitVec.add_comm`, `BitVec.mul_add`) (124), and then with the 31 semantic-merge
  // relation/termination theorems of `NobleContracts.Merge`, so the strict verdict now
  // pins 155 required theorems. The exact string keeps the verdict non-substitutable; a
  // merged branch that also adds pins must sum its additions here rather than read the
  // gate's printed count. The library's globs build `NobleContracts.Merge` too.
  if (!library.includes('MC1-CONTRACT-LIBRARY: 155 required theorems; strict axioms accepted')) {
    throw Error('missing strict contract-library verdict');
  }
  const audit = run('implementation-audit', tool('lean', 'lake'), ['env', 'lean', 'ImplementationGate.lean'], proofRoot);
  const records = prefix => audit.split('\n').filter(line => line.startsWith(prefix))
    .map(line => JSON.parse(line.slice(prefix.length)));
  const [implementation, ...extra] = records('MC1-IMPLEMENTATION ');
  const statements = records('MC1-SOURCE-EQUATION ');
  const literalSize = records('MC1-LITERAL-SIZE ');
  // 31 bridge types (the #97 port added GenericVariantDecl, the three bound
  // registrations and LiveSite) with both round trips, plus nine named theorems.
  if (!implementation || extra.length || implementation.strict_theorems !== 71 || statements.length !== 6) {
    throw Error('incomplete compiled implementation theorem audit');
  }
  // Only the linked kernel checker reaches the generated literal-size axioms; they
  // are reported apart from the strict axioms, and the in-gate controls refused
  // the forged type and the foreign owner.
  if (literalSize.some(record => record.theorem !== 'noble_contracts.KernelBridge.check_from_inputs') ||
      JSON.stringify(literalSize.flatMap(record => record.axioms).sort()) !==
        JSON.stringify([...implementation.literal_size_axioms].sort()) ||
      !implementation.literal_size_controls?.refused_contracts_owner) {
    throw Error('unreviewed generated literal-size admission');
  }
  if (JSON.stringify(snapshot()) !== JSON.stringify(sourceFiles)) throw Error('source changed during proof gate');
  const evidence = { schema_version: 1, result: 'passed', scope: 'mc1-statement-export-v1',
    source_revision: `sha256:${sha(JSON.stringify(sourceFiles))}`, source_files: sourceFiles,
    versions, commands, kernel, contracts, local_opaque_reviews: reviewedContractsOpaque,
    implementation, literal_size: literalSize, statements,
    assumptions: [
      'Pinned Charon/Aeneas translation and handwritten standard-library models are explicit trust boundaries.',
      'Inherited local opaque copy/format helpers are exactly the five enumerated kernel boundaries.',
      'The contracts subject has exactly the ten reviewed local opaque functions: three derived-Debug helpers modelled in the formatter abstraction, and seven host plug-in calls answered by host behaviour the handle carries (prophecy sequences for &mut calls, functions of injective argument images for &self calls).',
      'Allocation identity, spare capacity, allocator failure, and formatting layout are abstracted by the inherited models.',
      'Models without a faithful value fail rather than guess: core::mem::size_of, Cell::set (shared mutation) and Arc::ptr_eq (allocation identity); no ok result is derived through them.',
      'Generated string-bound proofs and six closed source equations use the separately listed native-evaluation axioms.',
      'The 71 universal bridge/projection theorems use strict standard axioms, except that check_from_inputs also uses the separately listed generated literal-size axioms of the linked kernel checker; the 43 application/rule theorems use strict standard axioms only.',
      'This is a named source-export fragment, not universal frontend/exporter or runtime correctness.',
    ] };
  fs.writeFileSync(path.join(artifacts, 'implementation.json'), JSON.stringify(evidence, null, 2) + '\n');
  console.log(`MC1-IMPLEMENTATION-GATE passed: ${evidence.source_revision}`);
} catch (error) {
  fs.writeFileSync(path.join(artifacts, 'failure.json'), JSON.stringify({ result: 'failed',
    message: String(error), commands }, null, 2) + '\n');
  console.error(String(error));
  process.exitCode = 1;
}
