import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { root, sha256, fileHash, Recorder } from './record.mjs';
import { scenarios } from './fixtures.mjs';

export const executedCases = ['CONTRACT-17', 'CONTRACT-18', 'CONTRACT-19',
  'CONTRACT-21', 'CONTRACT-22', 'CONTRACT-25'];
const reviewCase = 'CONTRACT-24';
const blockedCases = ['CONTRACT-20', 'CONTRACT-23'];
const canonicalFile = 'specs/conformance/contract-cases.json';

function walk(directory, files) {
  for (const entry of fs.readdirSync(path.join(root, directory), { withFileTypes: true })
    .sort((left, right) => left.name.localeCompare(right.name))) {
    if (entry.name === '.lake' || entry.name === 'target'
      || (directory === 'verification/intrinsic-proofs'
        && (entry.name === 'acceptance.json' || entry.name === 'source-bridge-acceptance.json'))) continue;
    const name = `${directory}/${entry.name}`;
    if (entry.isDirectory()) walk(name, files);
    else {
      assert.ok(entry.isFile(), `source inventory contains non-file: ${name}`);
      files.add(name);
    }
  }
}

export function preparePlan(record, scenarios) {
  const sourcePaths = new Set([
    'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', 'flake.lock', 'flake.nix',
    'policy/tool-selection.json', 'verification/m6/pins.json',
    'nix/reviewed-vendor.nix', 'verification/selected-vendor.mjs', canonicalFile,
    '.cairn/README.md', '.cairn/changes/intrinsic-noble-proofs/design.md',
    '.cairn/changes/intrinsic-noble-proofs/tasks.md',
    '.cairn/specs/language/spec.md', '.cairn/specs/program-contracts/spec.md',
    '.cairn/specs/verification/spec.md', '.cairn/specs/developer-experience/spec.md',
    'verification/mc1/increment.noble-contract', 'verification/mc1/intrinsic-feasibility.mjs',
    'verification/mc1/intrinsic-feasibility-proof.lean',
    'verification/mc2/contracts/q-one.noble',
    'verification/mc2/contracts/increment.contract',
    'verification/mc2/contracts/increment.proof.lean',
  ]);
  for (const tree of [
    'crates/noble-contracts', 'crates/noble-kernel', 'crates/noble-cli', 'crates/noble-wasm',
    'proofs/mc1', 'verification/intrinsic-proofs', 'verification/declared-modules-v1/peer',
  ]) walk(tree, sourcePaths);
  const sources = Object.fromEntries([...sourcePaths].sort().map(name => [name, record.source(name)]));
  const canonical = JSON.parse(fs.readFileSync(path.join(root, canonicalFile)));
  const byId = new Map(canonical.cases.map(row => [row.id, row]));
  assert.equal(byId.size, canonical.cases.length, 'duplicate canonical case IDs');
  const selection = [...executedCases, reviewCase, ...blockedCases, 'CONTRACT-16'];
  const cases = Object.fromEntries(selection.map(id => {
    const row = byId.get(id);
    assert.ok(row, `missing canonical ${id}`);
    if (id !== 'CONTRACT-16') assert.equal(row.profile, 'Intrinsic-Proofs-Draft');
    return [id, { kind: row.kind, input: row.input, expected: row.expected }];
  }));
  assert.deepEqual(executedCases.map(id => cases[id].input.harness), [
    'actual-noble-module-predicative-type-universal',
    'noble-source-increment-proof-to-exact-mc1-claim',
    'reject-forged-proof-source-and-lexer-confusion',
    'bounded-total-proof-checking',
    'proof-erasure-and-proof-free-ordinary-program',
    'dependent-equality-elimination-and-source-scope',
  ], 'changed canonical intrinsic workload');
  const selected = JSON.parse(fs.readFileSync(path.join(root, 'policy/tool-selection.json')));
  const selectedNode = fs.realpathSync(path.join(selected.tool_paths.node.output, 'bin/node'));
  assert.equal(fs.realpathSync(process.execPath), selectedNode, 'selected Node required');
  assert.deepEqual(process.execArgv, [], 'unreviewed Node flags');
  const toolPaths = {
    node: selectedNode,
    'wasm-tools': path.join(selected.tool_paths.wasm_tools.output, 'bin/wasm-tools'),
    cargo: path.join(selected.tool_paths.quality_rust.output, 'bin/cargo'),
    rustc: path.join(selected.tool_paths.quality_rust.output, 'bin/rustc'),
    lean: path.join(selected.tool_paths.lean.output, 'bin/lean'),
  };
  const tools = Object.fromEntries(Object.entries(toolPaths).map(([name, file]) => {
    const frozen = record.freeze(name, file);
    return [name, { file: record.relative(frozen), sha256: fileHash(frozen) }];
  }));
  assert.deepEqual(scenarios.map(item => item.id), executedCases,
    'all and only selected executable cases must be planned');
  const baselineVariants = new Map([
    ['CONTRACT-17', 'positive-identity-and-instantiation'],
    ['CONTRACT-18', 'exact-source-proof'],
    ['CONTRACT-19', null],
    ['CONTRACT-21', 'well-formed-small-polymorphic-proof'],
    ['CONTRACT-22', 'same-accepted-program-with-and-without-intrinsic-proof'],
    ['CONTRACT-25', null],
  ]);
  const inputs = {};
  for (const item of scenarios) {
    const row = cases[item.id];
    assert.ok(item.baseline && Buffer.isBuffer(item.baseline), `${item.id}: real baseline source bytes`);
    assert.ok(item.variants && typeof item.variants === 'object', `${item.id}: variant workload`);
    if (['CONTRACT-17', 'CONTRACT-18', 'CONTRACT-25'].includes(item.id))
      assert.equal(item.baseline.toString('utf8').trimEnd(), row.input.source,
        `${item.id}: baseline must be the exact canonical .noble source`);
    if (item.id === 'CONTRACT-19')
      assert.equal(item.variants['false-Eq']?.toString('utf8').trimEnd(), row.input.source,
        `${item.id}: false equality must be the exact canonical hostile source`);
    assert.deepEqual(Object.keys(item.variants),
      row.input.variants.filter(name => name !== baselineVariants.get(item.id)),
      `${item.id}: variants diverge from canonical input`);
    const freeze = (name, bytes) => {
      assert.ok(Buffer.isBuffer(bytes) && bytes.length > 0, `${item.id}/${name}: source not provided`);
      const file = record.retain(`plan/inputs/${item.id}/${name}.noble`, bytes);
      return { file: record.relative(file), sha256: fileHash(file), bytes: bytes.length };
    };
    inputs[item.id] = { baseline: freeze('baseline', item.baseline), variants: {}, extras: {} };
    for (const [variant, bytes] of Object.entries(item.variants))
      inputs[item.id].variants[variant] = freeze(variant, bytes);
    for (const [name, bytes] of Object.entries(item.extras ?? {})) {
      assert.ok(!row.input.variants.includes(name), `${item.id}: supplemental source shadows variant`);
      inputs[item.id].extras[name] = freeze(name, bytes);
    }
  }
  const workload = Object.fromEntries(selection.map(id => [id, cases[id]]));
  const emptyBinding = record.retain('plan/inputs/bindings/empty.txt', Buffer.alloc(0));
  const positive = {
    'CONTRACT-17': ['polymorphic-reflexivity', ...cases['CONTRACT-17'].input.positive_type_instances],
    'CONTRACT-18': ['exact-source-proof'],
    'CONTRACT-19': ['independent-valid-polymorphic-proof-before-hostiles'],
    'CONTRACT-21': ['well-formed-small-polymorphic-proof'],
    'CONTRACT-22': ['proof-free-compiled-program', 'proof-bearing-compiled-program',
      'proof-only-edited-compiled-program'],
    'CONTRACT-25': ['polymorphic-reflexivity', 'apply-proof-to-valid-List-I64-code',
      'subst-with-typed-motive-and-checked-premise', 'imported-immutable-use'],
  };
  const extraHostiles = {
    'CONTRACT-17': cases['CONTRACT-17'].input.negative_type_instances,
  };
  const nonHostileVariants = new Set(['proof-only-change', 'explicit-MC2-evidence-inspection',
    'ordinary-quote-compose-run-reflect', 'apply-proof-to-valid-List-I64-code',
    'subst-with-typed-motive-and-checked-premise']);
  const caseLedger = Object.fromEntries(executedCases.map(id => [id, {
    required_positive_baselines: positive[id], required_positive_count: positive[id].length,
    required_hostile_variants: [...cases[id].input.variants.filter(name =>
      name !== baselineVariants.get(id) && !nonHostileVariants.has(name)),
    ...(extraHostiles[id] ?? [])],
    required_hostile_count: cases[id].input.variants.filter(name =>
      name !== baselineVariants.get(id) && !nonHostileVariants.has(name)).length
      + (extraHostiles[id]?.length ?? 0),
    executed_positive_baselines: 0, executed_hostile_variants: 0,
  }]));
  const plan = {
    schema: 'noble-intrinsic-proofs-fixture-plan/v1', status: 'planned-not-executed',
    acceptance_claim: false, source_root: root, output_directory: record.output,
    sources, tools, canonical: { file: canonicalFile, sha256: sources[canonicalFile].sha256,
      workload_sha256: sha256(Buffer.from(JSON.stringify(workload))), cases }, inputs,
    bindings: { empty: { file: record.relative(emptyBinding), sha256: fileHash(emptyBinding),
      bytes: 0 } },
    required_execution: executedCases, case_ledger: caseLedger, review: reviewCase,
    blocked_release: { cases: blockedCases, depends_on: 'CONTRACT-16/VC-OWNER-01',
      reason: 'this gate does not perform independently owner-frozen law release or validate its external acceptance receipt' },
    receipt_requirements: {
      each_variant: ['actual-module-source', 'baseline-before-hostile', 'frozen-CLI',
        'raw-stdout-stderr-exit', 'stage-outcome', 'zero-guest-and-protected-work-on-refusal'],
      strict_proof: ['independently-derived-exact-claim', 'pinned-reviewed-rule-and-consumer',
        'bounded-checker', 'strict-axioms-and-dependency-audit'],
      erasure: ['real-compiled-wasm-off-and-on', 'wasm-tools-validate-and-print',
        'unchanged-executable-recipe-and-definition-identity', 'ordinary-stack-and-request-trace',
        'snapshot-survives-rejected-proof'],
      nonclaims: ['source-to-Lean-soundness-not-proved-by-finite-gate',
        'Rust-refinement-not-proved-by-execution', 'owner-law-release-not-admitted'],
    },
  };
  const manifest = record.retain('plan/plan.json', Buffer.from(JSON.stringify(plan, null, 2) + '\n'));
  record.receipt.plan = { file: record.relative(manifest), sha256: fileHash(manifest),
    selected_cases: executedCases, review_case: reviewCase, blocked_release_cases: blockedCases };
  return plan;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  assert.equal(process.argv.length, 3,
    'usage: SELECTED_NODE verification/intrinsic-proofs/plan.mjs NEW_EXTERNAL_DIRECTORY');
  const record = new Recorder(process.argv[2]);
  const plan = preparePlan(record, scenarios());
  console.log(JSON.stringify({ schema: plan.schema, status: plan.status,
    acceptance_claim: plan.acceptance_claim, manifest: path.join(record.output, record.receipt.plan.file),
    selected_cases: plan.required_execution, source_files: Object.keys(plan.sources).length,
    positive_baseline_counts: Object.fromEntries(Object.entries(plan.case_ledger)
      .map(([id, row]) => [id, row.required_positive_count])),
    hostile_variant_counts: Object.fromEntries(Object.entries(plan.case_ledger)
      .map(([id, row]) => [id, row.required_hostile_count])),
    variant_counts: Object.fromEntries(Object.entries(plan.inputs)
      .map(([id, row]) => [id, Object.keys(row.variants).length])) }));
}
