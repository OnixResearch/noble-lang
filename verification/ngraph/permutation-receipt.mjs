#!/usr/bin/env node
// Source-bound receipt for the separate #129 permutation profile. Read a clean committed tree,
// decode its retained real binary independently of the Rust codec, and bind every replay subject
// to the kernel-audited actual corpus. This does not claim a Rust/Lean refinement theorem.
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(fileURLToPath(import.meta.url), '../../..');
const argv = process.argv.slice(2);
const value = (key) => {
  const index = argv.indexOf(key);
  if (index < 0 || index + 1 >= argv.length) throw Error(`missing ${key}`);
  return argv[index + 1];
};
const assert = (yes, message) => { if (!yes) throw Error(message); };
const sha = (bytes) => createHash('sha256').update(bytes).digest('hex');
const git = (...args) => {
  const result = spawnSync('git', args, { cwd: root, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 });
  assert(result.status === 0, `git ${args.join(' ')}: ${result.stderr}`);
  return result.stdout.trim();
};
const file = (name) => readFileSync(path.join(root, name));
const sourcePaths = [
  'crates/noble-ngraph/src/permutation.rs',
  'crates/noble-ngraph/src/compiled.rs',
  'crates/noble-ngraph/src/layout.rs',
  'crates/noble-ngraph/src/check.rs',
  'crates/noble-ngraph/src/ir.rs',
  'crates/noble-ngraph/src/sem.rs',
  'crates/noble-ngraph/src/eval.rs',
  'crates/noble-ngraph/tests/schedule_correspondence_dir/permutation.rs',
  'crates/noble-ngraph/tests/schedule_correspondence_dir/permutation_export.rs',
  'proofs/foundation/NobleFoundation/PackedPermutation.lean',
  'proofs/foundation/NobleFoundation/PackedPermutationCorpus.lean',
  'proofs/foundation/NobleFoundation/PackedSchedule.lean',
  'proofs/foundation/NobleFoundation/FirstProjection.lean',
  'proofs/foundation/inventory.json',
  'proofs/foundation/NobleFoundation.lean',
];
const corpus = 'proofs/foundation/NobleFoundation/PackedPermutationCorpus.lean';
const binary = 'verification/ngraph/permutation-noncanonical-0.bin';
const output = path.resolve(value('--output'));
assert(!output.startsWith(`${root}${path.sep}`), 'output must be outside the repository');
assert(git('status', '--porcelain=v1', '--untracked-files=all') === '', 'a clean committed tree is required');
const sourceCommit = git('rev-parse', 'HEAD');
const foundation = JSON.parse(readFileSync(path.resolve(value('--foundation-receipt'))));
assert(foundation.clean_source === true && foundation.allow_dirty === false,
  'the foundation proof receipt must be from clean source');
assert(spawnSync('git', ['merge-base', '--is-ancestor', foundation.source_commit, 'HEAD'], { cwd: root }).status === 0,
  'the proof receipt must be on this source ancestry');
for (const name of sourcePaths.filter((item) => item.endsWith('.lean'))) {
  const atProof = spawnSync('git', ['show', `${foundation.source_commit}:${name}`],
    { cwd: root, maxBuffer: 64 * 1024 * 1024 });
  assert(atProof.status === 0 && Buffer.compare(atProof.stdout, file(name)) === 0,
    `proof subject changed: ${name}`);
}

// Decode the actual retained byte export. All lengths are checked against remaining bytes and
// the total file is capped before any allocation; each field retains its exact byte slice.
const bytes = file(binary);
assert(bytes.length <= 32 * 1024 * 1024, 'retained export exhausted its default byte cap');
let at = 0;
const take = (length) => {
  assert(Number.isSafeInteger(length) && length >= 0 && length <= bytes.length - at,
    'malformed binary export');
  const slice = bytes.subarray(at, at + length);
  at += length;
  return slice;
};
const u32 = () => take(4).readUInt32LE(0);
const field = (read) => { const start = at; const parsed = read(); return { parsed, bytes: bytes.subarray(start, at) }; };
const text = () => {
  const raw = take(u32());
  const decoded = new TextDecoder('utf-8', { fatal: true }).decode(raw);
  assert(Buffer.compare(Buffer.from(decoded, 'utf-8'), raw) === 0, 'noncanonical text');
  return decoded;
};
const table = (width) => {
  const length = u32();
  assert(length <= Math.floor((bytes.length - at) / width), 'malformed table length');
  return take(length * width);
};
assert(take(8).toString('ascii') === 'NPRM0000', 'unsupported binary export');
const profile = field(text), semantic = field(text), checker = field(text), model = field(text), exportRev = field(text);
const layoutRev = field(text), compilerRev = field(text);
const graphId = field(() => take(32).toString('hex'));
const layoutKey = field(() => take(32).toString('hex'));
const layoutStart = layoutRev.bytes.byteOffset - bytes.byteOffset;
const registers = field(() => { const count = u32(); return { count, values: table(4) }; });
table(12);
table(8);
table(8);
table(4);
const stream = field(() => {
  const count = u32();
  assert(count <= Math.floor((bytes.length - at) / 13), 'malformed instruction count');
  const instructions = take(count * 13);
  for (let offset = 0; offset < instructions.length; offset += 13) {
    assert(instructions[offset] <= 3, 'unsupported instruction opcode');
  }
  return { count, instructions };
});
const witness = field(() => table(4));
const outRegs = field(() => table(4));
assert(at === bytes.length, 'trailing bytes in export');
const source = file('crates/noble-ngraph/src/permutation.rs').toString('utf8');
const constant = (name) => {
  const match = new RegExp(`pub const ${name}: &str = "([^"]+)";`).exec(source);
  assert(match, `missing Rust revision ${name}`);
  return match[1];
};
for (const [actual, expected] of [
  [profile.parsed, constant('PROFILE')],
  [semantic.parsed, /pub const SEMANTIC_REVISION: &str = "([^"]+)";/.exec(file('crates/noble-ngraph/src/ir.rs').toString('utf8'))?.[1]],
  [checker.parsed, constant('CHECKER_REVISION')],
  [model.parsed, constant('MODEL_REVISION')],
  [exportRev.parsed, constant('EXPORT_REVISION')],
  [layoutRev.parsed, /pub const LAYOUT_REVISION: &str = "([^"]+)";/.exec(file('crates/noble-ngraph/src/layout.rs').toString('utf8'))?.[1]],
  [compilerRev.parsed, /pub const COMPILER_REVISION: &str = "([^"]+)";/.exec(file('crates/noble-ngraph/src/compiled.rs').toString('utf8'))?.[1]],
]) assert(actual === expected, `stale revision: ${actual} != ${expected}`);

const prefix = 'NobleFoundation.PackedPermutationCorpus.';
const proved = foundation.theorems.filter((row) => row.name.startsWith(prefix));
const names = new Set(proved.map((row) => row.name));
assert(names.has(`${prefix}first_validated`) && names.has(`${prefix}first_sound`),
  'kernel proof receipt does not contain the noncanonical artifact and soundness');
assert(proved.every((row) => row.status === 'proved' && row.axioms.every((axiom) => foundation.allowed_axioms.includes(axiom))),
  'nonstandard axiom or unproved corpus theorem');
const corpusText = file(corpus).toString('utf8');
assert(corpusText.includes('def firstWitness') && corpusText.includes('theorem first_sound'),
  'the bound noncanonical model corpus is absent');
const nodesDeclaration = corpusText.match(/^def nodes : List Node :=[\s\S]*?(?=\ndef regs)/m)?.[0];
assert(nodesDeclaration, 'the concrete model graph nodes declaration is absent');
const measurePath = argv.includes('--measurement-file') ? path.resolve(value('--measurement-file')) : null;
let measurement = null;
if (measurePath) {
  const line = readFileSync(measurePath, 'utf8').split('\n').find((entry) => entry.startsWith('PERM_MEASURE '));
  assert(line, 'measurement file has no executed PERM_MEASURE result');
  measurement = JSON.parse(line.slice('PERM_MEASURE '.length));
  assert(measurement.witness_bytes === witness.parsed.length, 'measurement witness size differs from artifact');
}
const receipt = {
  schema: 'noble-packed-permutation-receipt/v1', issue: 129,
  source_commit: sourceCommit, source_tree: git('rev-parse', 'HEAD^{tree}'), clean_source: true,
  profile: profile.parsed, graph_logical_id: graphId.parsed,
  replay_subjects: {
    graph: { logical_id: graphId.parsed, model_nodes_sha256: sha(nodesDeclaration) },
    stream: { instructions: stream.parsed.count, sha256: sha(stream.bytes) },
    witness: { bytes: witness.parsed.length, sha256: sha(witness.bytes) },
    register_output_map: { register_count: registers.parsed.count, registers_sha256: sha(registers.bytes), output_sha256: sha(outRegs.bytes) },
    layout: { key: layoutKey.parsed, declaration_sha256: sha(bytes.subarray(layoutStart, stream.bytes.byteOffset - bytes.byteOffset)) },
    profile: { profile: profile.parsed, semantic: semantic.parsed, sha256: sha(Buffer.concat([profile.bytes, semantic.bytes])) },
    revisions: { checker: checker.parsed, model: model.parsed, export: exportRev.parsed,
      compiler: compilerRev.parsed, layout: layoutRev.parsed,
      sha256: sha(Buffer.concat([checker.bytes, model.bytes, exportRev.bytes, layoutRev.bytes, compilerRev.bytes])) },
  },
  export: { path: binary, bytes: bytes.length, sha256: sha(bytes), codec_sha256: sha(source) },
  corpus: { path: corpus, sha256: sha(file(corpus)), theorem_count: proved.length,
    validated: proved.find((row) => row.name === `${prefix}first_validated`),
    sound: proved.find((row) => row.name === `${prefix}first_sound`),
    transitive_axioms: Object.fromEntries(proved.map((row) => [row.name, row.axioms])) },
  foundation: { source_commit: foundation.source_commit, source_tree: foundation.source_tree,
    lean_sha256: foundation.lean_sha256, allowed_axioms: foundation.allowed_axioms },
  sources_sha256: Object.fromEntries(sourcePaths.map((name) => [name, sha(file(name))])),
  measurement,
  evidence: 'Rust freshness/differential/control tests and the Lean kernel check are separate; this receipt only binds their subjects and the retained proof statements.',
  open_edges: ['No arbitrary Rust executor or runtime correspondence theorem.',
    'Lean receives generated terms, not a verified byte decoder refinement.',
    'No guards, F32, register reuse, cross-layer motion, low-fuel/cancellation equivalence, cache or native-code theorem.'],
};
writeFileSync(output, `${JSON.stringify(receipt, null, 2)}\n`, { flag: 'wx' });
console.log(JSON.stringify({ result: 'passed', source_commit: sourceCommit, export_sha256: sha(bytes),
  theorems: proved.length, receipt_sha256: sha(readFileSync(output)) }));
