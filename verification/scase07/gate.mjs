#!/usr/bin/env node
// Source-bound S-CASE-07 gate for the existing Core-only `noble admit-artifact`
// route. It follows the accepted S-CASE-16 admission-gate pattern and the
// immutable dx06-current-source inventory/command helpers, builds the production
// CLI from the frozen source with the pinned offline toolchain, records every
// external command and never edits or promotes the canonical case. Run with the
// runtime-selected Node:
//   <selected node> verification/scase07/gate.mjs NEW_EXTERNAL_DIRECTORY
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {root, sha, read, sourceInventory, sourceTrees, buildFiles, revision, frozen,
  freshOutput, commandsAt, verifyRecordedCommands} from '../dx06-current-source/source.mjs';

assert.equal(process.argv.length, 3,
  'usage: <selected node> verification/scase07/gate.mjs NEW_EXTERNAL_DIRECTORY');
const runtimeSelection = JSON.parse(read('crates/noble-cli/src/core/runtime/config.json'));
const abi = JSON.parse(read('crates/noble-cli/src/core/runtime/abi.json'));
assert.equal(fs.realpathSync(process.execPath), fs.realpathSync(runtimeSelection.tools.node.path),
  'run the gate with the runtime-selected Node');

// The exact canonical design, still unexecuted and unpromoted.
const caseFile = 'specs/conformance/safety-cases.json';
const caseText = read(caseFile).toString('utf8');
const caseRows = JSON.parse(caseText).cases;
const design = caseRows.find(row => row.id === 'S-CASE-07');
assert.deepEqual(Object.keys(design),
  ['id', 'profile', 'kind', 'requirements', 'input', 'expected', 'state', 'evidence']);
assert.equal(design.profile, 'Wasm-Draft');
assert.equal(design.kind, 'admission');
assert.deepEqual(design.requirements, ['S-TYPE-02', 'S-HOST-03']);
assert.deepEqual(design.input, {harness:'forged-artifact-metadata', wasm_valid:true,
  recipe_matches_artifact:false, trusted_build:false, translation_validation:false});
assert.deepEqual(design.expected, {stage:'admission', outcome:'correspondence-reject',
  guest_requests:0, protected_operations:0});
assert.deepEqual(design.state, {implementation:'absent', execution:'not-run', proof:'open', trust:'unassessed'});
assert.deepEqual(design.evidence, []);
const caseLines = caseText.split('\n').filter(line => line.includes('"id":"S-CASE-07"'));
assert.equal(caseLines.length, 1, 'S-CASE-07 occupies exactly one canonical line');
assert.equal(caseLines[0], `    ${JSON.stringify(design)},`, 'S-CASE-07 line is its compact JSON row');

// Earlier source-bound receipts are immutable inputs, never rewritten here.
const historical = {};
for (const row of caseRows.filter(item => item.id === 'S-CASE-16' || item.id === 'S-CASE-02')) {
  for (const evidence of row.evidence) {
    const receipt = path.join('specs/conformance', evidence.configuration.receipt);
    assert.equal(sha(read(receipt)), evidence.configuration.receipt_sha256, `historical receipt changed: ${receipt}`);
    historical[receipt] = evidence.configuration.receipt_sha256;
  }
}
assert.deepEqual(Object.keys(historical).sort(), ['verification/scase02/acceptance.json',
  'verification/scase02/corrected-acceptance.json', 'verification/scase16/acceptance.json']);

// The accepted inventory: production CLI trees and build inputs (including the
// runtime-selected Node/wasm-tools/wasm-opt configuration), plus this case's
// governing documents and the immutable helpers and receipts this gate reuses.
const change = '.cairn/changes/safety-artifact-correspondence';
const sources = sourceInventory([...new Set([...buildFiles, caseFile,
  '.cairn/specs/safety/spec.md', 'specs/SAFETY.md', `${change}/metadata.json`, `${change}/proposal.md`,
  `${change}/design.md`, `${change}/specs/safety/spec.md`, 'verification/scase07/gate.mjs',
  'verification/dx06-current-source/source.mjs', 'verification/scase16/gate.mjs', ...Object.keys(historical)])],
sourceTrees);
const sourceRevision = revision(sources);

// Pinned offline build through the accepted command recorder.
const artifacts = freshOutput(process.argv[2]);
const work = path.join(artifacts, 'work');
fs.mkdirSync(work);
process.env.TMPDIR = path.join(artifacts, 'tmp');
const {run, artifact, commands, environment, rust} = commandsAt(artifacts);
// The accepted recorder's sibling cargo-home has no offline crates.io sources;
// like the accepted four-crate regression, read the populated user registry.
const cargoHome = process.env.CARGO_HOME ?? path.join(os.homedir(), '.cargo');
assert.ok(fs.statSync(path.join(cargoHome, 'registry/index')).isDirectory(), 'populated offline Cargo registry');
environment.CARGO_HOME = cargoHome;
const built = run('production-build', path.join(rust, 'cargo'),
  ['build', '-p', 'noble-cli', '--locked', '--offline', '-j', '4', '--message-format=json']);
const cli = artifact(built, 'noble', 'bin');
const wasmTools = runtimeSelection.tools.wasm_tools.path;

// Fixtures: the host-selected true program and a different real program.
const put = (name, value) => {
  const file = path.join(work, name);
  fs.writeFileSync(file, value, {flag:'wx'});
  return file;
};
const pureSource = put('pure.noble', '1 2 +\n');
const otherSource = put('other.noble', '2 2 +\n');
const stackOf = report => report.stack.map(value => [value.type, value.value]);
function emit(label, file, expected) {
  const directory = path.join(artifacts, label);
  const report = JSON.parse(run(label, cli, ['run', file, '--opt', 'off', '--emit', directory]));
  assert.equal(report.stage, 'wasm');
  assert.equal(report.outcome, 'normal');
  assert.deepEqual(stackOf(report), [['I64', expected]]);
  assert.equal(report.guest_requests, 0);
  assert.equal(report.protected_operations, 0);
  return {wasm:path.join(directory, 'engine/module-1.wasm'), wat:path.join(directory, 'engine/module-1.wat')};
}
const pure = emit('emit-pure', pureSource, '3');
const other = emit('emit-other', otherSource, '4');
const trueBytes = fs.readFileSync(pure.wasm);
const otherBytes = fs.readFileSync(other.wasm);
assert.equal(trueBytes.equals(otherBytes), false, 'different programs must produce different artifacts');
const pureWat = fs.readFileSync(pure.wat, 'utf8');
const otherWat = fs.readFileSync(other.wat, 'utf8');
const pureLines = pureWat.split('\n');
const otherLines = otherWat.split('\n');
assert.equal(pureLines.length, otherLines.length);
const watDifferences = pureLines.flatMap((line, index) => line === otherLines[index] ? []
  : [{line:index + 1, pure:line, other:otherLines[index]}]);
const codeTarget = '(call $push_i64 (i64.const 1))';
const codeReplacement = '(call $push_i64 (i64.const 2))';
assert.equal(watDifferences.length, 2, 'the programs differ exactly in the literal push and its recipe atom');
assert.equal(watDifferences[0].pure, codeTarget);
assert.equal(watDifferences[0].other, codeReplacement);
assert.match(watDifferences[1].pure, /\(call \$new \(i32\.const 8\) \(i64\.const 1\)/);
assert.match(watDifferences[1].other, /\(call \$new \(i32\.const 8\) \(i64\.const 2\)/);

// Every candidate is first validated by pinned wasm-tools and compiled by V8.
const reassembled = path.join(work, 'pure-reassembled.wasm');
run('reassemble-true-wat', wasmTools, ['parse', pure.wat, '-o', reassembled]);
assert.ok(fs.readFileSync(reassembled).equals(trueBytes), 'selected WAT reassembles to the emitted artifact');
const candidates = {};
function candidate(label, file) {
  run(`validate-${label}`, wasmTools, ['validate', file]);
  const content = fs.readFileSync(file);
  const module = new WebAssembly.Module(content);
  const functions = WebAssembly.Module.imports(module).filter(entry => entry.kind === 'function')
    .map(entry => `${entry.module}.${entry.name}`);
  assert.deepEqual(functions, [], `${label} imports no effect function`);
  candidates[label] = {file:path.relative(artifacts, file), sha256:sha(content), bytes:content.length,
    wasm_tools_valid:true, v8_compiled:true, function_imports:functions};
  return content;
}
candidate('true-1-2', pure.wasm);
candidate('other-2-2', other.wasm);
const appended = put('appended-custom-section.wasm', Buffer.concat([trueBytes, Buffer.from([0x00, 0x01, 0x00])]));
const appendedBytes = candidate('inert-custom-section', appended);

// Semantic mutation only when its exact code target is unique and the result
// actually executes; the recipe atom keeps the original literal.
const occurrences = pureWat.split(codeTarget).length - 1;
let mutation = {included:false, target:codeTarget, occurrences};
let mutatedFile = null;
let mutatedBytes = null;
let mutatedWat = null;
if (occurrences === 1) {
  mutatedWat = pureWat.replace(codeTarget, codeReplacement);
  const mutatedWatFile = put('mutated-code-push.wat', mutatedWat);
  mutatedFile = path.join(work, 'mutated-code-push.wasm');
  run('assemble-mutated', wasmTools, ['parse', mutatedWatFile, '-o', mutatedFile]);
  mutatedBytes = candidate('semantic-code-mutation', mutatedFile);
  const mutatedLines = mutatedWat.split('\n');
  const changed = pureLines.flatMap((line, index) => line === mutatedLines[index] ? [] : [index + 1]);
  assert.deepEqual(changed, [watDifferences[0].line], 'only the literal code push changed');
  assert.equal(mutatedLines[watDifferences[1].line - 1], watDifferences[1].pure, 'recipe atom still records 1');
  mutation = {included:true, target:codeTarget, replacement:codeReplacement, occurrences,
    changed_wat_line:watDifferences[0].line, unchanged_recipe_atom_line:watDifferences[1].line,
    wat_sha256:sha(mutatedWat), wasm_sha256:sha(mutatedBytes)};
}

// Candidate metadata: a correct empty effect claim plus forged correspondence hints.
const manifest = (name, value) => put(`${name}.json`, JSON.stringify(value));
const forged = (name, content) => manifest(`forged-${name}`, {claimed_effects:[], source:pureSource,
  digest:sha(content), trusted_correspondence:true, allowed:true});
const plain = manifest('plain-empty-claim', {claimed_effects:[]});
const forgedOther = forged('other-2-2', otherBytes);
const forgedAppended = forged('inert-custom-section', appendedBytes);
const forgedMutated = mutatedBytes ? forged('semantic-code-mutation', mutatedBytes) : null;
const trustedBuildField = manifest('trusted-build-field', {claimed_effects:[], trusted_build:true});
const translationField = manifest('translation-validation-field', {claimed_effects:[], translation_validation:true});

const rows = [];
function admit(label, role, args, status) {
  const report = JSON.parse(run(label, cli, ['admit-artifact', ...args], status));
  rows.push({label, role, args:args.map(arg => arg.startsWith(artifacts) ? path.relative(artifacts, arg) : arg),
    exit:status, report});
  return report;
}
const correspondence = {schema:'noble-artifact-admission/v1', profile:'Wasm-Draft', stage:'admission',
  outcome:'correspondence-reject', diagnostic:'final artifact bytes differ from independent host-selected compilation',
  actual_imports:[], claimed_effects:[], allowed_effects:[], guest_requests:0, protected_operations:0};
const canonical = admit('canonical-scase07', 'canonical',
  [other.wasm, '--effects', forgedOther, '--source', pureSource, '--opt', 'off'], 2);
assert.deepEqual(canonical, correspondence);
for (const [field, value] of Object.entries(design.expected)) assert.deepEqual(canonical[field], value, `S-CASE-07.${field}`);
assert.deepEqual(admit('canonical-without-forged-hints', 'metadata-independence',
  [other.wasm, '--effects', plain, '--source', pureSource, '--opt', 'off'], 2), correspondence);
function executed(report, value) {
  assert.equal(report.stage, 'wasm');
  assert.equal(report.outcome, 'normal');
  assert.deepEqual(stackOf(report), [['I64', value]]);
  assert.equal(report.guest_requests, 0);
  assert.equal(report.protected_operations, 0);
  assert.deepEqual(report.request_trace, []);
  assert.deepEqual(report.admission, {actual_imports:[], claimed_effects:[], allowed_effects:[],
    correspondence:'byte-exact', optimized:false});
}
executed(admit('positive-true-source', 'positive',
  [pure.wasm, '--effects', plain, '--source', pureSource, '--opt', 'off'], 0), '3');
executed(admit('differential-actual-source', 'positive',
  [other.wasm, '--effects', forgedOther, '--source', otherSource, '--opt', 'off'], 0), '4');
assert.deepEqual(admit('inert-custom-section-append', 'supplemental-refusal',
  [appended, '--effects', forgedAppended, '--source', pureSource, '--opt', 'off'], 2), correspondence);
if (mutatedBytes) {
  assert.deepEqual(admit('semantic-code-mutation-host-1-2', 'supplemental-refusal',
    [mutatedFile, '--effects', forgedMutated, '--source', pureSource, '--opt', 'off'], 2), correspondence);
  assert.deepEqual(admit('semantic-code-mutation-host-2-2', 'supplemental-refusal',
    [mutatedFile, '--effects', forgedMutated, '--source', otherSource, '--opt', 'off'], 2), correspondence);
}
assert.deepEqual(admit('control-no-host-source', 'labeled-control-not-canonical',
  [other.wasm, '--effects', forgedOther, '--opt', 'off'], 2),
  {...correspondence, diagnostic:'host-selected source is required'});
assert.deepEqual(admit('control-option-mismatch', 'labeled-control-not-canonical',
  [pure.wasm, '--effects', plain, '--source', pureSource, '--opt', 'on'], 2), correspondence);
const unknownField = {schema:'noble-artifact-admission/v1', profile:'Wasm-Draft', stage:'admission',
  outcome:'invalid-manifest', diagnostic:'unknown candidate manifest field', guest_requests:0, protected_operations:0};
assert.deepEqual(admit('control-trusted-build-field', 'structural-absence-probe',
  [other.wasm, '--effects', trustedBuildField, '--source', pureSource, '--opt', 'off'], 2), unknownField);
assert.deepEqual(admit('control-translation-validation-field', 'structural-absence-probe',
  [other.wasm, '--effects', translationField, '--source', pureSource, '--opt', 'off'], 2), unknownField);
const usage = admit('control-trusted-build-flag', 'structural-absence-probe',
  [other.wasm, '--effects', forgedOther, '--source', pureSource, '--trusted-build', 'true', '--opt', 'off'], 2);
assert.equal(usage.stage, 'admission');
assert.equal(usage.outcome, 'invalid-input');
assert.match(usage.diagnostic, /^usage: noble admit-artifact /);

// In-process corroboration of the refusal counters with a fresh production host.
const {CoreEngine} = await import(path.join(root, 'crates/noble-cli/src/core/runtime/host.mjs'));
const fresh = new CoreEngine(runtimeSelection, abi);
const memoryBefore = Buffer.from(new Uint8Array(fresh.memory.buffer));
const tableBefore = Array.from({length:fresh.table.length}, (_, index) => fresh.table.get(index));
const direct = fresh.admit(otherBytes, Buffer.from(pureWat), fs.readFileSync(pureSource), [], [], true);
assert.deepEqual(direct, correspondence);
const corroboration = {report:direct, trace:[...fresh.trace], protected_operations:fresh.protectedOperations,
  compilations:fresh.compilations, pending:fresh.pending, instance_retained:fresh.last !== undefined,
  poisoned:fresh.poisoned,
  memory_unchanged:memoryBefore.equals(Buffer.from(new Uint8Array(fresh.memory.buffer))),
  table_unchanged:tableBefore.every((entry, index) => fresh.table.get(index) === entry),
  memory_bytes:memoryBefore.length, table_entries:tableBefore.length};
fresh.close();
assert.deepEqual(corroboration.trace, []);
assert.equal(corroboration.protected_operations, 0);
assert.equal(corroboration.compilations, 0);
assert.equal(corroboration.pending, null);
assert.equal(corroboration.instance_retained, false);
assert.equal(corroboration.poisoned, false);
assert.equal(corroboration.memory_unchanged, true);
assert.equal(corroboration.table_unchanged, true);

// S-TYPE-02: a raw module is never an installation capability.
const rawHost = new CoreEngine(runtimeSelection, abi);
let rawInstall = null;
try {
  assert.throws(() => rawHost.install(new WebAssembly.Module(otherBytes)), error => {
    rawInstall = error.message;
    return /raw module installation is not artifact admission/.test(error.message);
  });
} finally {
  rawHost.close();
}

// Harness-only executability oracle for the mutation; never an admission route.
let oracle = null;
if (mutatedBytes) {
  const harness = new CoreEngine(runtimeSelection, abi);
  try {
    const ready = harness.prepare(Buffer.from(mutatedWat), fs.readFileSync(pureSource));
    assert.equal(ready.outcome, 'ready');
    const report = harness.execute();
    assert.equal(report.outcome, 'normal');
    assert.deepEqual(stackOf(report), [['I64', '4']]);
    oracle = {route:'in-process CoreEngine.prepare/execute harness; not admission', outcome:report.outcome,
      stack:stackOf(report), guest_requests:report.guest_requests};
  } finally {
    harness.close();
  }
}

frozen(sources);
verifyRecordedCommands(commands, artifacts);
const counts = {canonical:1,
  refusals:rows.filter(row => row.report.outcome === 'correspondence-reject').length,
  executions:rows.filter(row => row.report.outcome === 'normal').length,
  structural_absence_probes:rows.filter(row => row.role === 'structural-absence-probe').length,
  commands:commands.length};
const receipt = {schema:'noble-scase07-artifact-correspondence/v1', kind:'test', result:'passed',
  case:{id:design.id, profile:design.profile, kind:design.kind, requirements:design.requirements,
    input:design.input, expected:design.expected},
  prepromotion_case_line_sha256:sha(caseLines[0]),
  claim:'On the selected Core-only noble admit-artifact route, a freshly built CLI refuses a pinned-wasm-tools-valid, V8-compilable Core artifact compiled from 2 2 + when the host selects 1 2 + source with --opt off, despite a correct empty effect claim and forged source/digest/trusted_correspondence/allowed metadata: admission/correspondence-reject before instantiation with zero guest requests and protected operations. The same bytes are admitted only under their actual host-selected 2 2 + source (I64 4); the unmodified 1 2 + artifact executes to I64 3; a valid inert custom-section append and a uniquely targeted executable code mutation also refuse.',
  route:'noble admit-artifact WASM --effects CLAIMS_JSON --source HOST_SOURCE --opt off',
  host_selection:{source:'1 2 +', source_sha256:sha(fs.readFileSync(pureSource)), optimization:'off', allowed_effects:[]},
  structural_absences:{
    trusted_build:'No host flag or candidate field exists on this route; a trusted_build manifest field is invalid-manifest and a flag is usage invalid-input. Candidate trusted_correspondence/source/digest/allowed are parsed and discarded, and the refusal is identical without them.',
    translation_validation:'No translation-validation input or engine path exists on this route; a translation_validation manifest field is invalid-manifest. Correspondence is only exact final-byte equality with the host pinned compilation.'},
  external_raw_output:artifacts,
  cargo_home:{path:cargoHome, reason:'offline crates.io sources for the pinned build; the recorder sibling cargo-home lacks addr2line'},
  source_revision:sourceRevision, source_sha256:sources, prepromotion_case_sha256:sources[caseFile],
  historical_receipts_sha256:historical, binary_sha256:sha(fs.readFileSync(cli)),
  tools:{node:{path:runtimeSelection.tools.node.path, sha256:sha(fs.readFileSync(process.execPath))},
    wasm_tools:{path:wasmTools, sha256:sha(fs.readFileSync(wasmTools))},
    wasm_opt:{path:runtimeSelection.tools.wasm_opt.path,
      sha256:sha(fs.readFileSync(runtimeSelection.tools.wasm_opt.path))},
    rust_bin:rust},
  fixtures:{pure_source_sha256:sha(fs.readFileSync(pureSource)), other_source_sha256:sha(fs.readFileSync(otherSource)),
    true_wat_sha256:sha(pureWat), other_wat_sha256:sha(otherWat), wat_differences:watDifferences, candidates, mutation},
  rows, corroboration, raw_install_refusal:rawInstall, executability_oracle:oracle, counts, commands,
  assumptions:[
    'Correspondence means exact byte equality between the submitted final Wasm and the host own compilation of independently accepted host-selected source under host-selected options, using the recorded pinned frontend/kernel/backend, wasm-tools and wasm-opt. No trusted-build attestation, translation validation, signature or proof-object input exists or is inferred from candidate metadata.',
    'The pinned Node/V8, wasm-tools, Rust toolchain, Core ABI and production host are trusted finite boundaries at the recorded identities. Refusal reports construct zero counters; a fresh in-process host corroborates no trace, protected operation, compilation, instance, memory or table change for the canonical bytes.',
    'The mutation executability oracle uses the engine internal compilation path in-process and is not an admission route or a safety claim about the mutated artifact.',
    'Historical S-CASE-16, S-CASE-02, DX-06 and six-case receipts remain immutable source-scoped evidence; this receipt does not extend them, and later source requires new replays.'],
  non_claims:['backend lowering or artifact-loading correctness (PO-17/PO-18)', 'cross-host provenance',
    'signature or proof-object admission', 'declared-binding or component artifact admission', 'general Wasm loading',
    'Rust/Lean refinement of the admission route']};
fs.writeFileSync(path.join(artifacts, 'acceptance.json'), `${JSON.stringify(receipt, null, 2)}\n`, {flag:'wx'});
console.log(JSON.stringify({result:'passed', receipt:path.join(artifacts, 'acceptance.json'),
  source_revision:sourceRevision, binary_sha256:receipt.binary_sha256, counts}));
