#!/usr/bin/env node
// Produces exact, source-frozen test inputs only. A plan is NOT acceptance evidence.
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = fs.realpathSync(path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..'));
const selected = new Map([
  ['specs/conformance/developer-experience-cases.json', ['DX-03']],
  ['specs/conformance/language-workflow-cases.json', ['DX-08', 'DX-09']],
]);
const variantNames = new Map([
  ['DX-03', [
    'UserId-to-OrderId-same-I64-layout', 'UserId-to-UserId',
    'private-constructor-from-outside-module', 'private-arm-match-from-outside-module',
    'variant-elimination-missing-constructor',
    'variant-elimination-all-constructors-compatible-stacks',
    'opaque-resource-wrapper-dup-drop-or-capture',
  ]],
  ['DX-08', [
    'matching-declared-operation', 'missing', 'wrong-input-type',
    'incompatible-effect-bound', 'import-time-host-initializer',
    'non-ASCII-declaration-name',
  ]],
]);
const outcomes = new Map([
  ['DX-03', ['reject', 'accept', 'reject', 'reject', 'reject', 'accept',
    'reject-each-operation']],
  ['DX-08', ['link-without-execution', 'reject', 'reject', 'reject',
    'reject-implicit-execution', 'reject-before-link']],
]);
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const fileHash = file => hash(fs.readFileSync(file));
const sourceUnits = {
  schema: `module ledger@1 [
  opaque UserId I64 private
  opaque OrderId I64 public
  variant Status Ready I64 public Failed Text private
  opaque CounterOwner Resource<test.counter> private
  export UserId
  export OrderId
  export OrderId.new
  export OrderId.into
  export Status
  export Status.Ready
  export CounterOwner
  export make_user
  export echo_user
  export reveal_user
  export make_ready
  export inspect_status
  def make_user [ 7 UserId.new ]
  def echo_user [ UserId.into UserId.new ]
  def reveal_user [ UserId.into ]
  def make_ready [ 3 Status.Ready ]
  def inspect_status [ [ 1 + ] [ drop 0 ] Status.match ]
]\n`,
  schema_v2: `module ledger@2 [
  opaque UserId I64 private
  opaque OrderId I64 public
  variant Status Ready I64 public Failed Text private
  opaque CounterOwner Resource<test.counter> private
  export UserId
  export OrderId
  export OrderId.new
  export OrderId.into
  export Status
  export Status.Ready
  export CounterOwner
  export make_user
  export echo_user
  export reveal_user
  export make_ready
  export inspect_status
  def make_user [ 7 UserId.new ]
  def echo_user [ UserId.into UserId.new ]
  def reveal_user [ UserId.into ]
  def make_ready [ 3 Status.Ready ]
  def inspect_status [ [ 1 + ] [ drop 0 ] Status.match ]
]\n`,
  incomplete_match_module: `module ledger@1 [
  variant Status Ready I64 public Failed Text private
  export Status
  def invalid [ 3 Status.Ready [ 1 + ] Status.match ]
]\n`,
  public_variant: `module public_variant@1 [
  variant Status Ready I64 public Failed Text public
  export Status
  export Status.Ready
  export Status.Failed
]\n`,
  first_named_module: `module ledger@1 [
  export handle
  def helper [ 1 ]
  def handle [ helper ]
]\n`,
  ledger_A: `module ledger@1 [
  require emit Text -- ! test.emit
  export handle
  def handle [ "A" emit ]
]\n`,
  ledger_B: `module ledger@2 [
  require emit Text -- ! test.emit
  export handle
  def handle [ "B" emit ]
]\n`,
  ambient_escape_module: `module ledger@1 [
  require emit Text -- ! test.emit
  export handle
  def handle [ "A" test.emit ]
]\n`,
  utf8_text_module: `module ledger@1 [
  require emit Text -- ! test.emit
  export handle
  def handle [ "café" emit ]
]\n`,
  non_ascii_identifier: `module ledgér@1 [
  require emit Text -- ! test.emit
  export handle
  def handle [ "A" emit ]
]\n`,
  executable_initializer: `module ledger@1 [
  require emit Text -- ! test.emit
  export handle
  def handle [ "A" emit ]
  "must-not-run-at-link" emit
]\n`,
  import_schema: 'import ledger@1 as account\n',
  import_schema_v2: 'import ledger@2 as next\n',
  import_public_variant: 'import public_variant@1 as visible\n',
  import_ledger_A: 'import ledger@1 as account\n',
  import_ledger_B: 'import ledger@2 as account\n',
  invoke_named: 'account.handle\n',
  mismatched_nominal: 'ledger@1.make_user ledger@1.OrderId.into\n',
  equal_nominal: 'ledger@1.make_user ledger@1.echo_user\n',
  private_construction: '7 ledger@1.UserId.new\n',
  outsider_match: 'ledger@1.make_ready [ 1 + ] [ drop 0 ] ledger@1.Status.match\n',
  static_complete_match: 'ledger@1.make_ready ledger@1.inspect_status\n',
  complete_match: 'account.make_ready account.inspect_status\n',
  public_match: '3 visible.Status.Ready [ 1 + ] [ drop 0 ] visible.Status.match\n',
  resource_dup: 'dup\n',
  resource_drop: 'drop\n',
  resource_capture: 'quote\n',
  ambient_literal: '"A"\n',
  cross_module_value: 'account.make_user account.reveal_user\n',
  distinct_version: 'ledger@1.make_user ledger@2.echo_user\n',
  unused_expression: '1\n',
  retain_A: 'def retained [ account.handle ]\n',
  execute_saved_program: 'retained\n',
  invoke_emit: 'account.handle\n',
};
const binding = Object.freeze({
  empty: '',
  A: 'bind ledger@1 test.emit version-A Text -- ! test.emit allow\n',
  denied_A_and_B: 'bind ledger@1 test.emit version-A Text -- ! test.emit deny\n'
    + 'bind ledger@2 test.emit version-B Text -- ! test.emit allow\n',
  wrong_input: 'bind ledger@1 test.emit version-A I64 -- ! test.emit allow\n',
  wrong_effect: 'bind ledger@1 test.emit version-A Text -- ! test.abort allow\n',
});

function fixtures() {
  const check = (variant, program, extras = {}) => ({ case_id: 'DX-03', variant,
    steps: [{ action: 'register-source', source: 'schema' },
      { action: 'link-only', source: 'import_schema' },
      { action: 'kernel-check-only', source: program, ...extras }] });
  const link = (variant, adapter, source = 'ledger_A') => ({ case_id: 'DX-08', variant,
    steps: [{ action: 'register-source', source },
      { action: 'bind-host-adapter', binding: adapter },
      { action: 'link-only', source: 'import_ledger_A' }] });
  return [
    check('UserId-to-OrderId-same-I64-layout', 'mismatched_nominal'),
    check('UserId-to-UserId', 'equal_nominal'),
    check('private-constructor-from-outside-module', 'private_construction',
      { additional_kernel_control: 'forged-private-constructor-owner-candidate' }),
    check('private-arm-match-from-outside-module', 'outsider_match'),
    { case_id: 'DX-03', variant: 'variant-elimination-missing-constructor',
      steps: [{ action: 'kernel-check-only', source: 'incomplete_match_module',
        obligation: 'owner module match is visible; refuse specifically because second arm is absent' }] },
    check('variant-elimination-all-constructors-compatible-stacks', 'static_complete_match'),
    ...['dup', 'drop', 'capture'].map(operation => check(
      'opaque-resource-wrapper-dup-drop-or-capture', `resource_${operation}`,
      { resource_operation: operation, fixture_resource: 'test.counter',
        input_stack: ['ledger@1.CounterOwner'],
        admission: 'source-declared-nominal-type-via-independent-kernel-checker; never inject a resource into Wasm' })),
    link('matching-declared-operation', 'A'),
    link('missing', 'empty'),
    link('wrong-input-type', 'wrong_input'),
    link('incompatible-effect-bound', 'wrong_effect'),
    link('import-time-host-initializer', 'A', 'executable_initializer'),
    link('non-ASCII-declaration-name', 'A', 'non_ascii_identifier'),
    { case_id: 'DX-09', variant: 'A-denied-after-B-alias-rebind', steps: [
      { action: 'bind-host-adapter', binding: 'denied_A_and_B' },
      { action: 'register-source', source: 'ledger_A' },
      { action: 'register-source', source: 'ledger_B' },
      { action: 'link-only', source: 'import_ledger_A' },
      { action: 'compile-and-retain-program', source: 'retain_A',
        obligation: 'retain compiled definition resolved to version-A, not mutable account spelling' },
      { action: 'rebind-display-name', source: 'import_ledger_B' },
      { action: 'explicit-compiled-invocation', source: 'execute_saved_program',
        obligation: 'observe one denied guest call of A, test.emit effect, zero protected work and zero calls of B' },
    ] },
    { case_id: 'supplemental', variant: 'compiled-nominal-match', steps: [
      { action: 'register-source', source: 'schema' }, { action: 'link-only', source: 'import_schema' },
      { action: 'explicit-compiled-invocation', source: 'complete_match', output: [{ type: 'I64', value: '4' }] },
    ] },
    { case_id: 'supplemental', variant: 'first-dynamic-definition-23-compiled', steps: [
      { action: 'register-source', source: 'first_named_module' },
      { action: 'link-only', source: 'import_ledger_A' },
      { action: 'explicit-compiled-invocation', source: 'invoke_named',
        output: [{ type: 'I64', value: '1' }] },
    ] },
    { case_id: 'supplemental', variant: 'cross-module-exported-nominal-value', steps: [
      { action: 'register-source', source: 'schema' }, { action: 'link-only', source: 'import_schema' },
      { action: 'explicit-compiled-invocation', source: 'cross_module_value', output: [{ type: 'I64', value: '7' }] },
    ] },
    { case_id: 'supplemental', variant: 'positive-compiled-host-invocation', steps: [
      { action: 'register-source', source: 'ledger_A' }, { action: 'bind-host-adapter', binding: 'A' },
      { action: 'link-only', source: 'import_ledger_A' },
      { action: 'explicit-compiled-invocation', source: 'invoke_emit',
        request: { adapter_id: 'version-A', effect: 'test.emit', text: 'A' } },
    ] },
    { case_id: 'supplemental', variant: 'rebound-version-B-positive-control', steps: [
      { action: 'register-source', source: 'ledger_A' },
      { action: 'register-source', source: 'ledger_B' },
      { action: 'bind-host-adapter', binding: 'denied_A_and_B' },
      { action: 'link-only', source: 'import_ledger_A' },
      { action: 'rebind-display-name', source: 'import_ledger_B' },
      { action: 'explicit-compiled-invocation', source: 'invoke_emit',
        request: { adapter_id: 'version-B', effect: 'test.emit', text: 'B' } },
    ] },
    { case_id: 'supplemental', variant: 'distinct-source-module-version-identity', steps: [
      { action: 'register-source', source: 'schema' }, { action: 'register-source', source: 'schema_v2' },
      { action: 'link-only', source: 'import_schema' }, { action: 'link-only', source: 'import_schema_v2' },
      { action: 'kernel-check-only', source: 'distinct_version' },
    ] },
    { case_id: 'supplemental', variant: 'stale-registry-snapshot', steps: [
      { action: 'register-source', source: 'schema' },
      { action: 'prepare-before-rebind', source: 'import_schema',
        obligation: 'retain checked immutable registry identity and attempted transaction' },
      { action: 'change-registry-or-alias', source: 'schema_v2' },
      { action: 'commit-stale-preparation', obligation: 'refuse stale publication, zero requests and unchanged authority' },
    ] },
    { case_id: 'supplemental', variant: 'hostile-kernel-candidates', steps: [
      { action: 'register-source', source: 'schema' },
      { action: 'independent-kernel-adversarial-check', candidates: [
        'forged-private-constructor-owner', 'forged-private-arm-match',
        'forged-same-layout-nominal', 'forged-resource-free-schema', 'raw-sum-to-nominal',
        'wrong-effect-witness', 'forged-bound-adapter-contract', 'stale-module-identity',
        'ambient-test-emit-bypass',
      ] },
    ] },
    { case_id: 'supplemental', variant: 'all-public-variant-external-match-executes', steps: [
      { action: 'register-source', source: 'public_variant' },
      { action: 'link-only', source: 'import_public_variant' },
      { action: 'explicit-compiled-invocation', source: 'public_match',
        output: [{ type: 'I64', value: '4' }] },
    ] },
    { case_id: 'supplemental', variant: 'UTF8-Text-literal-compiled-guest', steps: [
      { action: 'register-source', source: 'utf8_text_module' },
      { action: 'bind-host-adapter', binding: 'A' },
      { action: 'link-only', source: 'import_ledger_A' },
      { action: 'explicit-compiled-invocation', source: 'invoke_emit',
        request: { adapter_id: 'version-A', effect: 'test.emit', text: 'café' } },
    ] },
    { case_id: 'supplemental', variant: 'ambient-test-emit-bypass-refused', steps: [
      { action: 'register-source', source: 'ambient_escape_module' },
      { action: 'bind-host-adapter', binding: 'A' },
      { action: 'link-only', source: 'import_ledger_A' },
      { action: 'require-static-refusal',
        obligation: 'no legacy test.emit word or ambient host fallback in declared module body' },
    ] },
  ];
}

function validateCanonical(documents, planned) {
  assert.equal(sourceUnits.non_ascii_identifier,
    sourceUnits.ledger_A.replace('ledger@1', 'ledgér@1'),
    'non-ASCII control must differ from the valid module only in its identifier bytes');
  const cases = new Map();
  for (const [file, ids] of selected) {
    const doc = documents.get(file);
    assert.equal(new Set(doc.cases.map(item => item.id)).size, doc.cases.length,
      `${file}: duplicate canonical case IDs`);
    for (const id of ids) {
      const entries = doc.cases.filter(item => item.id === id);
      assert.equal(entries.length, 1, `${file}: missing/duplicate ${id}`);
      const row = entries[0];
      assert.equal(row.profile, 'Declared-Modules-v1');
      assert.equal(row.state.execution, 'not-run', `${id}: do not recycle prior receipts`);
      cases.set(id, row);
    }
  }
  const dx03 = cases.get('DX-03'), dx08 = cases.get('DX-08'), dx09 = cases.get('DX-09');
  for (const declaration of ['opaque UserId I64 private', 'opaque OrderId I64 public',
    'variant Status Ready I64 public Failed Text private',
    'opaque CounterOwner Resource<test.counter> private'])
    assert.ok(dx03.input.source_contract.includes(declaration), `DX-03 lost ${declaration}`);
  assert.ok(dx08.input.source_contract.includes('require emit Text -- ! test.emit'));
  assert.equal(dx08.input.required_operation, 'test.emit : Text -- ! {test.emit}');
  assert.equal(dx03.input.harness, 'real-source-module-declarations-through-independent-kernel-acceptance');
  assert.equal(dx08.input.harness, 'real-source-module-import-and-explicit-linking-with-no-guest-execution');
  assert.equal(dx09.input.harness, 'compiled-noble-wasm-linked-module-authority-and-resolution');
  assert.deepEqual([dx03.expected.stage, dx08.expected.stage, dx09.expected.stage],
    ['static', 'link-admission', 'runtime']);
  for (const [id, field] of [['DX-03', 'case'], ['DX-08', 'binding']]) {
    const actual = cases.get(id).input.variants;
    assert.deepEqual(actual.map(row => row[field]), variantNames.get(id), `${id}: canonical variants changed`);
    assert.deepEqual(actual.map(row => row.outcome), outcomes.get(id), `${id}: canonical outcomes changed`);
    const expectedCounts = id === 'DX-03' ? { 'opaque-resource-wrapper-dup-drop-or-capture': 3 } : {};
    for (const variant of variantNames.get(id)) {
      const count = planned.filter(item => item.case_id === id && item.variant === variant).length;
      assert.equal(count, expectedCounts[variant] ?? 1, `${id}/${variant}: omitted or duplicated`);
    }
    assert.equal(planned.filter(item => item.case_id === id).length,
      variantNames.get(id).length + (id === 'DX-03' ? 2 : 0));
  }
  for (const key of ['guest_requests', 'protected_operations']) {
    assert.equal(dx03.expected[key], 0);
    assert.equal(dx08.expected[key], 0);
  }
  for (const key of ['host_requests', 'ambient_fallback_calls']) assert.equal(dx08.expected[key], 0);
  assert.equal(dx08.expected.acquired_authority, false);
  assert.equal(dx09.input.original_binding, 'version-A');
  assert.equal(dx09.input.rebound_display_name, 'version-B');
  assert.deepEqual(dx09.input.declared_effects, ['test.emit']);
  assert.equal(dx09.expected.outcome, 'denied-request-for-version-A');
  assert.equal(dx09.expected.guest_requests, 1);
  assert.equal(dx09.expected.protected_operations, 0);
  assert.equal(dx09.expected.version_B_invoked, false);
  assert.equal(dx09.expected.effect_request_recorded, true);
  assert.equal(planned.filter(item => item.case_id === 'DX-09').length, 1);
  const known = new Set([...cases.keys(), 'supplemental']);
  for (const item of planned) {
    assert.ok(known.has(item.case_id), `unreviewed case ${item.case_id}`);
    for (const step of item.steps) {
      if ('source' in step) assert.ok(Object.hasOwn(sourceUnits, step.source), `${item.variant}: missing source`);
      if ('binding' in step && step.binding !== null)
        assert.ok(Object.hasOwn(binding, step.binding), `${item.variant}: missing binding`);
    }
  }
  assert.equal(new Set(planned.map(item => `${item.case_id}/${item.variant}/${item.steps.find(step => step.resource_operation)?.resource_operation ?? ''}`)).size,
    planned.length, 'duplicate planned scenario');
  return cases;
}

export function preparePlan(artifacts) {
  const target = path.resolve(artifacts);
  assert.ok(target !== root && !target.startsWith(`${root}${path.sep}`), 'output must be outside repository');
  assert.ok(!fs.existsSync(target), 'new external output directory required');
  assert.equal(fs.realpathSync(path.dirname(target)), path.dirname(target), 'output parent must not be a symlink');
  fs.mkdirSync(target);
  const retained = {}, watched = [], sources = {};
  const retain = (name, bytes, mode = 0o400) => {
    const dest = path.join(target, name);
    fs.mkdirSync(path.dirname(dest), { recursive: true });
    fs.writeFileSync(dest, bytes, { flag: 'wx', mode });
    const content = fs.readFileSync(dest);
    assert.equal(hash(content), hash(bytes));
    retained[name] = { sha256: hash(content), bytes: content.length };
    watched.push({ file: dest, sha256: hash(content) });
    return name;
  };
  const source = name => {
    const original = path.join(root, name);
    assert.ok(fs.lstatSync(original).isFile(), `not a plain source file: ${name}`);
    const bytes = fs.readFileSync(original);
    const snapshot = retain(`sources/${name}`, bytes);
    sources[name] = { sha256: hash(bytes), bytes: bytes.length, snapshot };
    watched.push({ file: original, sha256: hash(bytes) });
    return bytes;
  };
  const tree = directory => {
    for (const entry of fs.readdirSync(path.join(root, directory), { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name))) {
      const name = `${directory}/${entry.name}`;
      if (entry.isDirectory()) tree(name);
      else if (entry.isFile()) source(name);
      else throw Error(`unreviewed non-file source entry: ${name}`);
    }
  };
  const documents = new Map();
  for (const file of selected.keys()) documents.set(file, JSON.parse(source(file)));
  source('.cairn/changes/2026-09-26-declared-modules-v1/design.md');
  for (const file of ['Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', 'flake.lock',
    'policy/tool-selection.json', 'crates/noble-contracts/Cargo.toml',
    'crates/noble-kernel/Cargo.toml', 'crates/noble-wasm/Cargo.toml',
    'crates/noble-cli/Cargo.toml', 'verification/declared-modules-v1/plan.mjs',
    'verification/declared-modules-v1/gate.mjs']) source(file);
  for (const directory of ['crates/noble-contracts/src', 'crates/noble-kernel/src',
    'crates/noble-wasm/src', 'crates/noble-wasm/runtime', 'crates/noble-cli/src',
    'verification/declared-modules-v1/peer']) tree(directory);
  const planned = fixtures();
  const cases = validateCanonical(documents, planned);
  const programBytes = Object.fromEntries(Object.entries(sourceUnits).map(([name, text]) => {
    const file = retain(`inputs/source/${name}.noble`, Buffer.from(text, 'utf8'));
    return [name, { file, sha256: retained[file].sha256, bytes: retained[file].bytes }];
  }));
  const bindings = Object.fromEntries(Object.entries(binding).map(([name, lines]) => {
    const file = retain(`inputs/bindings/${name}.txt`, Buffer.from(lines, 'utf8'));
    return [name, { file, sha256: retained[file].sha256, bytes: retained[file].bytes }];
  }));
  const projection = Object.fromEntries([...selected].map(([file, ids]) => [file, {
    source_sha256: sources[file].sha256,
    complete_selected_workload: ids.map(id => {
      const row = cases.get(id);
      return { id, input: row.input, expected: row.expected };
    }),
  }]));
  for (const [file, item] of Object.entries(projection)) item.workload_sha256 = hash(Buffer.from(JSON.stringify(item.complete_selected_workload)));
  const configFile = 'crates/noble-cli/src/core/runtime/config.json';
  const config = JSON.parse(fs.readFileSync(path.join(target, sources[configFile].snapshot)));
  const selectedNode = fs.realpathSync(config.tools.node.path);
  assert.equal(fs.realpathSync(process.execPath), selectedNode, 'execute planner with selected Node');
  const tools = Object.fromEntries([
    ['node', selectedNode], ['wasm_tools', config.tools.wasm_tools.path],
    ['wasm_opt', config.tools.wasm_opt.path],
  ].map(([name, namePath]) => {
    const real = fs.realpathSync(namePath);
    assert.ok(fs.statSync(real).isFile(), `${name}: missing selected tool`);
    const frozen = retain(`executables/${name}`, fs.readFileSync(real), 0o500);
    watched.push({ file: real, sha256: retained[frozen].sha256 });
    return [name, { path: real, frozen, sha256: retained[frozen].sha256 }];
  }));
  const plan = {
    schema: 'noble-declared-modules-fixture-plan/v1', status: 'planned-not-executed',
    acceptance_claim: false, source_root: root, output_directory: target,
    profile: 'Declared-Modules-v1', profile_contract: sources['.cairn/changes/2026-09-26-declared-modules-v1/design.md'],
    sources, tools, canonical: projection, program_bytes: programBytes, bindings,
    scenarios: planned, receipt_requirements: {
      per_command: ['exact-argv', 'stdin-raw-bytes', 'stdout-raw-bytes', 'stderr-raw-bytes', 'process-status',
        'wasm-raw-bytes', 'selected-tool-hashes'],
      per_link: ['exact-module-version-and-schema-identity', 'binding-contract-and-immutable-adapter-id',
        'guest-requests-zero', 'host-requests-zero', 'protected-operations-zero',
        'ambient-fallback-zero', 'acquired-authority-false'],
      per_runtime: ['kernel-admission-result', 'compiled-wasm-identity', 'guest-output',
        'request-and-effect-trace', 'host-authorization-trace', 'adapter-A-count', 'adapter-B-count',
        'protected-operation-count'],
      denial: ['version-A-after-version-B-rebind', 'exactly-one-real-guest-request',
        'denied-before-protected-work', 'test.emit-effect-recorded', 'no-version-B-call'],
    },
    source_integrity: 'checked-after-planning', proof: 'open', commands: [], observations: [],
  };
  for (const entry of watched) assert.equal(fileHash(entry.file), entry.sha256,
    `source or retained asset changed during planning: ${entry.file}`);
  retain('plan.json', JSON.stringify(plan, null, 2) + '\n');
  return { schema: plan.schema, status: plan.status, manifest: path.join(target, 'plan.json'),
    planned_scenarios: planned.length, canonical_cases: [...cases.keys()],
    resource_operation_controls: planned.filter(item => item.steps.some(step => step.resource_operation)).length };
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  assert.equal(process.argv.length, 3, 'usage: SELECTED_NODE verification/declared-modules-v1/plan.mjs NEW_EXTERNAL_DIRECTORY');
  console.log(JSON.stringify(preparePlan(process.argv[2])));
}
