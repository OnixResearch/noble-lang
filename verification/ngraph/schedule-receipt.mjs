#!/usr/bin/env node
// Issue #43, item 6: a source-bound receipt for the real packed artifacts the Lean kernel validates.
//
// The generated `PackedScheduleCorpus.lean` and `PackedScheduleGuardCorpus.lean` files are pinned
// to the declarations and streams `Compiled::new` emits. This lane writes, for each artifact:
// - the graph and packed artifact, by digest of exactly the terms the kernel checked;
// - the exporter, checker and interpreter revisions;
// - the accepted claim, the semantic theorem, the refused mutations and the agreeing executions, with
//   the statements and axioms a clean-source foundation receipt recorded for them;
// - the premises the claim rests on and the edges that stay trusted or open.
//
// The foundation receipt must come from a commit whose corpus and checker model are byte-identical to
// this checkout's, so its statements are about these artifacts.
//
// Usage: node verification/ngraph/schedule-receipt.mjs --foundation-receipt <receipt dir> --output <new file>
import { spawnSync } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const argv = process.argv.slice(2);
const value = (name) => {
  const index = argv.indexOf(name);
  if (index < 0 || index + 1 >= argv.length) throw new Error(`missing ${name}`);
  return argv[index + 1];
};
const root = path.resolve(fileURLToPath(import.meta.url), '../../..');
const foundationDir = path.resolve(value('--foundation-receipt'));
const output = path.resolve(value('--output'));
const requireThat = (condition, message) => { if (!condition) throw new Error(message); };
const sha256 = (data) => crypto.createHash('sha256').update(data).digest('hex');
const git = (args) => {
  const result = spawnSync('git', args, { cwd: root, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 });
  requireThat(result.status === 0, `git ${args.join(' ')} failed: ${result.stderr}`);
  return result.stdout;
};
const read = (file) => fs.readFileSync(path.join(root, file));
const digests = (files) => Object.fromEntries(files.map((file) => [file, sha256(read(file))]));

const CORPUS_FILES = [
  'proofs/foundation/NobleFoundation/PackedScheduleCorpus.lean',
  'proofs/foundation/NobleFoundation/PackedScheduleGuardCorpus.lean',
];
const MODEL = ['proofs/foundation/NobleFoundation/PackedSchedule.lean',
  'proofs/foundation/NobleFoundation/PackedScheduleGuard.lean',
  'proofs/foundation/NobleFoundation/FirstProjection.lean'];
const EXPORTER = ['crates/noble-ngraph/tests/schedule_lean_dir/corpus.rs',
  'crates/noble-ngraph/tests/schedule_lean_dir/corpus_guard.rs',
  'crates/noble-ngraph/tests/schedule_lean_dir/corpus_shared.rs',
  'crates/noble-ngraph/tests/schedule_lean_dir/emit.rs',
  'crates/noble-ngraph/tests/schedule_lean_dir/emit_guard.rs',
  'crates/noble-ngraph/tests/common/mod.rs',
  'crates/noble-ngraph/tests/schedule_correspondence.rs'];
const PACKED_MACHINE = ['crates/noble-ngraph/src/compiled.rs', 'crates/noble-ngraph/src/schedule.rs', 'crates/noble-ngraph/src/layout.rs'];
const REFERENCE = ['crates/noble-ngraph/src/eval.rs', 'crates/noble-ngraph/src/sem.rs', 'crates/noble-ngraph/src/check.rs'];
const NAMESPACE = 'NobleFoundation.PackedScheduleCorpus.';
const EXPORT_TAG = 'ngraph-packed-export/0';

// ---------- source ----------
const source = {
  commit: git(['rev-parse', 'HEAD']).trim(),
  tree: git(['rev-parse', 'HEAD^{tree}']).trim(),
  status: git(['status', '--porcelain=v1', '--untracked-files=all']).trim(),
};
requireThat(source.status === '', `the source worktree is dirty:\n${source.status}`);
requireThat(!output.startsWith(`${root}${path.sep}`), 'the output must be outside the worktree');

// ---------- the foundation receipt the claim comes from ----------
const foundation = JSON.parse(fs.readFileSync(path.join(foundationDir, 'foundation-check.json'), 'utf8'));
requireThat(foundation.clean_source === true && foundation.allow_dirty === false, 'the foundation receipt is not from a clean source');
requireThat(spawnSync('git', ['merge-base', '--is-ancestor', foundation.source_commit, 'HEAD'], { cwd: root }).status === 0,
  'the foundation receipt is not from an ancestor of this checkout');
for (const file of [...CORPUS_FILES, ...MODEL]) {
  requireThat(git(['show', `${foundation.source_commit}:${file}`]) === read(file).toString('utf8'),
    `${file} changed since the foundation receipt's commit`);
}
const proved = Object.fromEntries(foundation.theorems.filter((row) => row.name.startsWith(NAMESPACE)).map((row) => [row.name, row]));
requireThat(Object.values(proved).every((row) => row.status === 'proved'), 'a corpus theorem is not proved');
const allowed = new Set(foundation.allowed_axioms);

// ---------- the artifacts, as the kernel checked them ----------
const corpus = CORPUS_FILES.map((file) => read(file).toString('utf8')).join('\n');
requireThat(corpus.includes(`def packedExportTag : String := "${EXPORT_TAG}"`),
  `the Lean corpus does not declare export tag ${EXPORT_TAG}`);
// A top-level `def NAME … := …` block, up to the next blank line.
function definition(name) {
  const match = new RegExp(`^def ${name} [^\\n]*(?:\\n(?!\\n)[^\\n]*)*`, 'm').exec(corpus);
  requireThat(match, `missing definition ${name}`);
  return match[0];
}
function theorem(name) {
  const row = proved[`${NAMESPACE}${name}`];
  requireThat(row, `the foundation receipt has no ${name}`);
  requireThat(row.axioms.every((axiom) => allowed.has(axiom)), `${name} uses an axiom outside the allow-list`);
  return { name: row.name, statement: row.statement, axioms: row.axioms };
}
const artifactNames = [...corpus.matchAll(/^\/-! ## `([A-Za-z]+)`: (.*) -\/$/gm)].map((match) => ({ name: match[1], doc: match[2] }));
requireThat(artifactNames.length > 0, 'no artifact in the corpus');
const artifacts = artifactNames.map(({ name, doc }) => {
  const graphTerms = [definition(`${name}Nodes`), definition(`${name}Graph`)];
  const packedTerms = ['Regs', 'Order', 'Layers', 'FailSuffixMin', 'LayerGuardOrder', 'Instrs', 'Outputs', 'OutRegs']
    .map((suffix) => definition(`${name}${suffix}`));
  const theorems = Object.keys(proved).filter((key) => key.startsWith(`${NAMESPACE}${name}_`)).map((key) => key.slice(NAMESPACE.length));
  const guarded = theorems.includes(`${name}_layered_sound`);
  const artifact = {
    name, doc,
    graph: { definitions: [`${name}Nodes`, `${name}Graph`], sha256: sha256(graphTerms.join('\n')) },
    packed_artifact: {
      definitions: ['Regs', 'Order', 'Layers', 'FailSuffixMin', 'LayerGuardOrder', 'Instrs', 'Outputs', 'OutRegs']
        .map((suffix) => `${name}${suffix}`),
      sha256: sha256(packedTerms.join('\n')),
    },
    claim: theorem(`${name}_validated`),
    semantics: theorem(`${name}_sound`),
    well_formed: theorem(`${name}_wellFormed`),
    refused_mutations: theorems.filter((key) => key.endsWith('_refused')).map((key) => theorem(key)),
    agreeing_executions: theorems.filter((key) => /_execution_\d+_agrees$/.test(key)).map((key) => theorem(key)),
  };
  if (guarded) {
    artifact.layered_semantics = theorem(`${name}_layered_sound`);
    artifact.suffix_table = theorem(`${name}_suffix_table_matches`);
    artifact.guard_order_witness = theorem(`${name}_layer_guard_witness`);
  }
  return artifact;
});
const covered = new Set(artifacts.flatMap((artifact) => [artifact.claim, artifact.semantics, artifact.well_formed,
  ...artifact.refused_mutations, ...artifact.agreeing_executions, artifact.layered_semantics, artifact.suffix_table,
  artifact.guard_order_witness].filter(Boolean).map((row) => row.name)));
requireThat(covered.size === Object.keys(proved).length, 'a corpus theorem belongs to no artifact');

// ---------- revisions ----------
const constant = (file, name) => {
  const match = new RegExp(`pub const ${name}: &str = "([^"]+)";`).exec(read(file).toString('utf8'));
  requireThat(match, `missing ${name} in ${file}`);
  return match[1];
};
const revisions = {
  exporter: { files_sha256: digests(EXPORTER), generated_sha256: sha256(corpus) },
  checker: { model_sha256: digests(MODEL), lean: foundation.lean, lean_sha256: foundation.lean_sha256 },
  interpreter: {
    packed_machine: {
      compiler_revision: constant('crates/noble-ngraph/src/compiled.rs', 'COMPILER_REVISION'),
      layout_revision: constant('crates/noble-ngraph/src/layout.rs', 'LAYOUT_REVISION'),
      files_sha256: digests(PACKED_MACHINE),
    },
    reference: { files_sha256: digests(REFERENCE) },
  },
};

const receipt = {
  schema: 'noble-packed-schedule-receipt/v2',
  export_tag: EXPORT_TAG,
  issue: 43,
  source_commit: source.commit, source_tree: source.tree, clean_source: true,
  corpus: {
    files: CORPUS_FILES,
    files_sha256: digests(CORPUS_FILES),
    sha256: sha256(corpus),
    pinned_by: 'the generated-corpus freshness tests in crates/noble-ngraph/tests/schedule_lean_dir/',
  },
  foundation_receipt: {
    path: path.relative(root, foundationDir), source_commit: foundation.source_commit,
    corpus_and_model_identical: true, theorems: Object.keys(proved).length,
  },
  artifacts,
  revisions,
  premises: [
    'The exporter is untrusted: it claims which node each instruction computes, and validate refuses a wrong claim.',
    'The preload is trusted: the packed machine loads the slot and constant tables into their registers before the first layer, written as setInput/setConst at their packed positions.',
    'Reading checked noble-ngraph nodes as model nodes is trusted.',
    'Each unguarded execution row is the Rust packed machine\'s output at a boundary input, checked against the Rust reference interpreter before it is written; the kernel evaluates run to the same outputs.',
    'Each guarded execution row is the Rust packed machine\'s observation at a boundary input, checked against the Rust reference interpreter before it is written; the kernel evaluates machineObs to that observation. The guarded artifact separately proves layer-order failure handling and the exact exported failure-suffix table.',
  ],
  open_edges: [
    'No theorem relates the Rust packed executor to the Lean machines: per-artifact validation does not prove the executor runs every stream as the model does.',
    'No decoder: an artifact enters Lean as generated text, not as decoded bytes, so no byte-level layout or stream format is bound.',
    'F32 schedules remain outside the validated slice; #38 is promoted and the follow-up issue is ready to file.',
    'Validation is per artifact; nothing is claimed about the scheduler on other graphs, the host or the engine.',
  ],
};
fs.writeFileSync(output, `${JSON.stringify(receipt, null, 2)}\n`, { flag: 'wx' });
console.log(JSON.stringify({ result: 'passed', receipt: output, source_commit: source.commit, artifacts: artifacts.length,
  theorems: covered.size, foundation_commit: foundation.source_commit, receipt_sha256: sha256(fs.readFileSync(output)) }));
