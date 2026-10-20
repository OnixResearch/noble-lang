// Frozen DX-04 workload and exact source/tool inventory. Planning runs no guest.
// Production source spelling is supplied by the acceptance gate only after its
// versioned module and source-signature grammar have been reviewed.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { root, sha256, fileHash } from './record.mjs';

const caseFile = 'specs/conformance/developer-experience-cases.json';
const libraryFile = 'crates/noble-contracts/src/fixtures/result.noble';
const sourceFiles = [
  'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', 'flake.lock', 'flake.nix',
  'policy/tool-selection.json', 'verification/m6/pins.json',
  '.cairn/specs/developer-experience/spec.md',
  '.cairn/changes/bend-qcue-scoped-adaptations/design.md',
  '.cairn/changes/bend-qcue-scoped-adaptations/tasks.md', caseFile,
];
const sourceTrees = [
  'crates/noble-kernel', 'crates/noble-contracts', 'crates/noble-wasm',
  'crates/noble-cli', 'crates/noble-syndicate', 'verification/result-library',
];

function walk(directory, files) {
  for (const entry of fs.readdirSync(path.join(root, directory), { withFileTypes: true })
    .sort((left, right) => left.name.localeCompare(right.name))) {
    const relative = `${directory}/${entry.name}`;
    if (entry.isDirectory()) walk(relative, files);
    else {
      assert.ok(entry.isFile(), `source inventory cannot contain symlink or special file: ${relative}`);
      files.add(relative);
    }
  }
}

function canonical() {
  const document = JSON.parse(fs.readFileSync(path.join(root, caseFile)));
  const [row] = document.cases.filter(item => item.id === 'DX-04');
  assert.equal(document.cases.filter(item => item.id === 'DX-04').length, 1,
    'canonical DX-04 case must be unique');
  assert.equal(row.profile, 'Result-Library-Draft');
  assert.equal(row.kind, 'runtime');
  assert.deepEqual(row.requirements, ['DX-RESULT-01', 'DX-RESULT-02']);
  assert.equal(row.input.harness, 'ordinary-result-combinators');
  assert.deepEqual(row.input.operations, ['map-success', 'map-error', 'chain', 'recover']);
  assert.deepEqual(row.input.variants, ['success', 'error']);
  assert.equal(row.input.callbacks, 'distinct-trace-markers-and-declared-effects');
  const paths = row.input.result_paths;
  assert.ok(Array.isArray(paths) && paths.length === 8, 'all eight Result paths required');
  assert.deepEqual(paths.map(item => [item.operation, item.input]), [
    ['map-success', 'Ok(2)'], ['map-success', 'Err(fault)'],
    ['map-error', 'Ok(2)'], ['map-error', 'Err(fault)'],
    ['chain', 'Ok(2)'], ['chain', 'Err(fault)'],
    ['recover', 'Ok(2)'], ['recover', 'Err(fault)'],
  ], 'canonical operation/arm order changed');
  assert.deepEqual(paths.map(item => [item.callback_result ?? null, item.output]), [
    ['3', 'Ok(3)'], [null, 'Err(fault)'],
    [null, 'Ok(2)'], ['mapped', 'Err(mapped)'],
    ['Err(fault)', 'Err(fault)'], [null, 'Err(fault)'],
    [null, 'Ok(2)'], ['Ok(3)', 'Ok(3)'],
  ], 'canonical callback result or observed output changed');
  for (const item of paths) {
    assert.ok(typeof item.output === 'string' && typeof item.callback_count === 'number',
      'each path must declare output and callback count');
    assert.equal(item.callback_count, item.input.startsWith('Ok(')
      ? Number(['map-success', 'chain'].includes(item.operation))
      : Number(['map-error', 'recover'].includes(item.operation)),
    'selected-arm callback count changed');
  }
  assert.deepEqual(row.input.negative_variants, [
    'resource-in-unselected-arm-implicitly-discarded',
    'resource-in-error-arm-implicitly-discarded',
    'incompatible-ordered-callback-stack',
  ]);
  assert.deepEqual(row.input.bypassed_effectful_callback, {
    operation: 'map-success', input: 'Err(fault)', callback_effect: 'test.emit',
    expected: 'accepted-and-run-without-callback-request',
  });
  assert.deepEqual(row.expected.bypassed_effectful_callback, {
    static: 'accept', runtime_output: 'Err(fault)', guest_requests: 0,
  });
  assert.equal(row.expected.stage, 'check-and-run');
  assert.equal(row.expected.outcome, 'each-result-path-and-negative-variant-matches');
  assert.deepEqual(row.expected.callback_selection, {
    'map-success': 'success-only', 'map-error': 'error-only',
    chain: 'success-only', recover: 'error-only',
  });
  assert.equal(row.expected.callback_trace, 'one-marker-on-selected-arm-zero-on-bypass');
  assert.equal(row.expected.negative_variants, 'static-rejection-with-zero-guest-requests');
  assert.equal(row.expected.effects_conservatively_include_callback, true);
  assert.equal(row.expected.resource_positive_runtime_claim, false);
  assert.equal(row.expected.new_expression_forms, 0);
  return { file: caseFile, source_sha256: fileHash(path.join(root, caseFile)),
    workload: { id: row.id, profile: row.profile, kind: row.kind,
      requirements: row.requirements, input: row.input, expected: row.expected },
    revision: document.revision };
}

function inputFiles(record, directory, entries, extension) {
  assert.ok(Object.keys(entries).length > 0, `${directory}: real fixture inputs required`);
  const retained = {};
  for (const name of Object.keys(entries).sort()) {
    assert.match(name, /^[A-Za-z0-9_-]+$/, `${directory}: unsafe input name`);
    const bytes = entries[name];
    assert.ok(Buffer.isBuffer(bytes), `${directory}/${name}: input must be exact bytes`);
    const file = record.retain(`plan/inputs/${directory}/${name}.${extension}`, bytes);
    retained[name] = { file: record.relative(file), sha256: fileHash(file), bytes: bytes.length };
  }
  return retained;
}

export function preparePlan(record, { moduleSource, programs, bindings }) {
  assert.equal(moduleSource, libraryFile,
    'acceptance must run the exact production versioned Result source file');
  const file = path.join(root, moduleSource);
  assert.ok(fs.lstatSync(file).isFile(), 'selected library source must be a plain source file');
  const selected = canonical();
  const files = new Set(sourceFiles);
  for (const directory of sourceTrees) walk(directory, files);
  assert.ok(files.has(moduleSource), 'library source omitted from source-bound inventory');
  const sources = Object.fromEntries([...files].sort().map(name => [name, record.source(name)]));
  const moduleBytes = fs.readFileSync(file);
  assert.equal(fileHash(file), sources[moduleSource].sha256,
    'versioned library bytes changed during plan');
  assert.ok(!Object.hasOwn(programs, 'result_module'),
    'library fixture bytes must come only from the production source file');
  const sourceUnits = inputFiles(record, 'source', { ...programs, result_module: moduleBytes }, 'noble');
  const bindingFiles = inputFiles(record, 'bindings', bindings, 'txt');
  const runtime = JSON.parse(fs.readFileSync(path.join(root, 'crates/noble-cli/src/core/runtime/config.json')));
  assert.equal(fs.realpathSync(process.execPath), fs.realpathSync(runtime.tools.node.path),
    'run source-bound acceptance with selected Node');
  assert.deepEqual(process.execArgv, [], 'unreviewed acceptance Node flags');
  const tools = {
    node: record.freeze('node', runtime.tools.node.path),
    wasm_tools: record.freeze('wasm-tools', runtime.tools.wasm_tools.path),
    wasm_opt: record.freeze('wasm-opt', runtime.tools.wasm_opt.path),
  };
  const canonicalWorkload = selected.workload;
  const plan = {
    schema: 'noble-result-library-fixture-plan/v1', status: 'planned-not-executed',
    acceptance_claim: false, source_root: root, output_directory: record.output,
    module_source: moduleSource, module_source_sha256: sources[moduleSource].sha256,
    sources, tools: Object.fromEntries(Object.entries(tools).map(([name, frozen]) =>
      [name, { file: record.relative(frozen), sha256: fileHash(frozen) }])),
    canonical: { file: selected.file, revision: selected.revision,
      source_sha256: selected.source_sha256,
      workload_sha256: sha256(Buffer.from(JSON.stringify(canonicalWorkload))),
      workload: canonicalWorkload },
    program_bytes: sourceUnits, bindings: bindingFiles,
    receipt_requirements: {
      per_command: ['exact-argv', 'stdin-raw-bytes', 'stdout-raw-bytes', 'stderr-raw-bytes',
        'process-status', 'frozen-tool-hashes'],
      per_execution: ['actual-compiled-wasm', 'checked-module-version', 'exact-ordered-result',
        'selected-callback-marker', 'request-trace', 'host-authorization', 'static-effect-bound'],
      per_negative: ['static-rejection', 'guest-requests-zero', 'host-requests-zero'],
      independent_peer: ['source-derived-valid-baseline', 'hostile-env-mutation-refusals'],
    },
  };
  const manifest = record.retain('plan/plan.json', Buffer.from(JSON.stringify(plan, null, 2) + '\n'));
  record.receipt.plan = { file: record.relative(manifest), sha256: fileHash(manifest),
    selected_cases: ['DX-04'], result_paths: 8, negative_variants: 3 };
  return plan;
}

export function bindRevision(record, plan) {
  assert.equal(plan.schema, 'noble-result-library-fixture-plan/v1');
  assert.equal(plan.status, 'planned-not-executed');
  assert.equal(plan.acceptance_claim, false);
  assert.ok(record.receipt.tools.noble && record.receipt.tools['kernel-peer'],
    'revision requires separately frozen production CLI and independent peer');
  assert.equal(fileHash(path.join(record.output, record.receipt.plan.file)),
    record.receipt.plan.sha256, 'source-bound plan changed before execution');
  const digests = entries => Object.fromEntries(Object.entries(entries).sort()
    .map(([name, entry]) => [name, entry.sha256]));
  const inputs = {
    reviewed_source: digests(plan.sources),
    source_fixtures: digests(plan.program_bytes),
    host_bindings: digests(plan.bindings),
    selected_tools: digests(plan.tools),
    compiled_executables: digests(record.receipt.tools),
    canonical_workload: {
      file: plan.canonical.file,
      source_sha256: plan.canonical.source_sha256,
      workload_sha256: plan.canonical.workload_sha256,
    },
  };
  record.receipt.revision_inputs = inputs;
  record.receipt.source_revision = `sha256:${sha256(Buffer.from(JSON.stringify(inputs)))}`;
  return record.receipt.source_revision;
}
