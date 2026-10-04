#!/usr/bin/env node
// Prepromotion, source-bound S-CASE-04 observation; never edits the canonical case.
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import {spawnSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
assert.equal(process.argv.length, 3,
  'usage: node verification/scase04/gate.mjs NEW_EXTERNAL_DIRECTORY');
const output = path.resolve(process.argv[2]);
assert.ok(output !== root && !output.startsWith(`${root}${path.sep}`),
  'prepromotion output must be outside the repository');
const parent = fs.realpathSync(path.dirname(output));
assert.ok(parent !== root && !parent.startsWith(`${root}${path.sep}`),
  'prepromotion output parent must resolve outside the repository');
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const read = file => fs.readFileSync(path.join(root, file));
const caseFile = 'specs/conformance/safety-cases.json';
const safetySpec = '.cairn/specs/safety/spec.md';
const changeTasks = '.cairn/changes/safety-recursive-resource-eligibility/tasks.md';
const design = JSON.parse(read(caseFile)).cases.find(row => row.id === 'S-CASE-04');
assert.ok(design, 'the canonical S-CASE-04 design is required');
assert.equal(design.profile, 'Resources-Draft');
assert.equal(design.kind, 'static');
assert.deepEqual(design.requirements, ['S-RES-01', 'S-RES-04']);
assert.deepEqual(design.input, {
  harness: 'recursive-resource-eligibility',
  input_types: ['Resource<R>', 'Pair<Text,Resource<R>>',
    'Sum<Resource<R>,I64>', 'List<Resource<R>>'],
  operations: ['dup', 'drop', 'quote', 'generic-serialization'],
  selected_sum_branch: 'I64',
});
assert.deepEqual(design.expected, {
  stage: 'check', outcome: 'eligibility-reject',
  guest_requests: 0, protected_operations: 0,
});
assert.deepEqual(design.state, {
  implementation: 'absent', execution: 'not-run', proof: 'open', trust: 'unassessed',
});
assert.deepEqual(design.evidence, [], 'this gate must run before canonical promotion');
assert.ok(!fs.existsSync(path.join(root, 'verification/scase04/acceptance.json')),
  'the S-CASE-04 acceptance receipt must not predate this gate');

const selected = JSON.parse(read('policy/tool-selection.json'));
const rust = path.join(selected.tool_paths.quality_rust.output, 'bin');
const node = path.join(selected.tool_paths.node.output, 'bin/node');
assert.equal(fs.realpathSync(process.execPath), fs.realpathSync(node), 'selected Node required');
const sourceRoots = ['crates/noble-kernel/src', 'crates/noble-contracts/src',
  'crates/noble-wasm/src', 'crates/noble-cli/src',
  'crates/noble-wasm/wit', 'verification/scase04'];
const changeRoot = '.cairn/changes/safety-recursive-resource-eligibility';
const fixedSources = ['Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml',
  'crates/noble-kernel/Cargo.toml', 'crates/noble-contracts/Cargo.toml',
  'crates/noble-wasm/Cargo.toml', 'crates/noble-cli/Cargo.toml',
  safetySpec, caseFile, 'policy/tool-selection.json'];
function inventory() {
  const result = {};
  function file(name) {
    const status = fs.lstatSync(path.join(root, name));
    assert.ok(status.isFile() && !status.isSymbolicLink(), `unbound source file: ${name}`);
    result[name] = sha(read(name));
  }
  function tree(directory) {
    const status = fs.lstatSync(path.join(root, directory));
    assert.ok(status.isDirectory() && !status.isSymbolicLink(),
      `unbound source directory: ${directory}`);
    for (const item of fs.readdirSync(path.join(root, directory), {withFileTypes:true})
      .sort((a,b) => a.name.localeCompare(b.name))) {
      const name = `${directory}/${item.name}`;
      assert.equal(item.isSymbolicLink(), false, `unbound source symlink: ${name}`);
      if (item.isDirectory()) tree(name);
      else if (item.isFile()) file(name);
      else assert.fail(`unbound source entry: ${name}`);
    }
  }
  for (const directory of sourceRoots) tree(directory);
  if (fs.existsSync(path.join(root, changeRoot))) tree(changeRoot);
  for (const name of fixedSources) file(name);
  assert.ok(Object.hasOwn(result, 'verification/scase04/check.rs'),
    'the independently compiled typed checker must be in the source inventory');
  assert.ok(Object.hasOwn(result, 'verification/scase04/gate.mjs'),
    'the running gate must be in the source inventory');
  assert.ok(Object.hasOwn(result, changeTasks),
    'the native change tasks must be in the prepromotion source inventory');
  return Object.fromEntries(Object.entries(result).sort());
}
function historicalReceipts() {
  const result = {};
  function tree(directory) {
    for (const item of fs.readdirSync(path.join(root, directory), {withFileTypes:true})
      .sort((a,b) => a.name.localeCompare(b.name))) {
      const name = `${directory}/${item.name}`;
      assert.equal(item.isSymbolicLink(), false, `unbound historical entry: ${name}`);
      if (item.isDirectory()) tree(name);
      else if (item.isFile() &&
        (item.name === 'acceptance.json' || item.name.endsWith('-acceptance.json')) &&
        name !== 'verification/scase04/acceptance.json') result[name] = sha(read(name));
    }
  }
  tree('verification');
  return Object.fromEntries(Object.entries(result).sort());
}
const sources = inventory();
const historical = historicalReceipts();
fs.mkdirSync(output); // NEW_DIR: an existing run can never overwrite its receipt.
fs.mkdirSync(path.join(output, 'inputs'));
fs.mkdirSync(path.join(output, 'tmp'));
function external(relative, bytes) {
  const destination = path.join(output, relative);
  fs.writeFileSync(destination, bytes, {flag:'wx'});
  return {path:relative, sha256:sha(fs.readFileSync(destination))};
}
const prepromotion_inputs = {
  case: external('prepromotion-safety-cases.json', read(caseFile)),
  safety_spec: external('prepromotion-safety-spec.md', read(safetySpec)),
  change_tasks: external('prepromotion-change-tasks.md', read(changeTasks)),
};
assert.equal(prepromotion_inputs.case.sha256, sources[caseFile]);
assert.equal(prepromotion_inputs.safety_spec.sha256, sources[safetySpec]);
assert.equal(prepromotion_inputs.change_tasks.sha256, sources[changeTasks]);

const witBytes = `package noble-test:recursive-resource@1.0.0;

interface counters {
  resource counter;
  open: func() -> own<counter>;
  close: func(value: own<counter>) -> s64;
}

world demo {
  import counters;
  export check: func() -> s64;
}
`;
const inputs = {};
function input(name, contents, extension = 'noble') {
  const relative = `inputs/${name}.${extension}`;
  inputs[relative] = external(relative, contents);
  return path.join(output, relative);
}
const wit = input('recursive-resource', witBytes, 'wit');
const prefixes = [
  {type:'Resource<R>', actual_type:'Resource(ResourceKind(1))', source:'counters.open'},
  {type:'Pair<Text,Resource<R>>', actual_type:'Pair(Text, Resource(ResourceKind(1)))',
    source:'"tag" counters.open pair'},
  {type:'Sum<Resource<R>,I64>', actual_type:'Sum(Resource(ResourceKind(1)), I64)',
    source:'false [ counters.open inl ] [ 7 inr ] if'},
  {type:'List<Resource<R>>', actual_type:'List(Resource(ResourceKind(1)))',
    source:'counters.open nil cons'},
];
const operations = [
  {operation:'dup', word:'dup', suffix:'dup'},
  {operation:'drop', word:'drop', suffix:'drop'},
  {operation:'quote', word:'quote', suffix:'quote'},
  {operation:'generic-serialization', word:'quote', suffix:'quote reflect'},
];
const negativeInputs = prefixes.flatMap((prefix, at) =>
  operations.map((operation, index) => ({
    ...prefix, ...operation, source:`${prefix.source} ${operation.suffix}`,
    file:input(`negative-${at}-${index}`, `${prefix.source} ${operation.suffix}`),
  })));
assert.equal(negativeInputs.length, 16);
const sumSource = `${prefixes[2].source} [ counters.close ] [ dup drop ] case`;
const sumFile = input('positive-sum-typed', sumSource);
const resourceSource = 'counters.open counters.close';
const pairSource = '"tag" counters.open pair unpair counters.close swap drop';
const dataSource = '7 dup drop';
const dataReflectSource = '7 true pair quote reflect';
const positives = [
  {label:'resource', source:resourceSource, file:input('positive-resource', `${resourceSource}\n`)},
  {label:'pair', source:pairSource, file:input('positive-pair', `${pairSource}\n`)},
  {label:'data', source:dataSource, file:input('positive-data', `${dataSource}\n`)},
];
const unsupportedWit =
  'package noble-test:recursive-resource@1.0.0; world demo { export check: func() -> u32; }';
const badWit = input('malformed-wit', 'not-wit\n', 'wit');
const unsupportedWitFile = input('unsupported-wit', unsupportedWit, 'wit');
const unboundSource = input('unbound-word', '7 not-a-word\n');
const illTypedSource = input('ill-typed', 'true 1 +\n');

const environment = {...process.env,
  PATH:`${rust}:${selected.component_sync.linker_bin}:${selected.tool_paths.node.output}/bin:${process.env.PATH}`,
  TMPDIR:path.join(output, 'tmp'), RUSTC_WRAPPER:'', RUSTC_WORKSPACE_WRAPPER:'',
  RUSTFLAGS:'', CARGO_ENCODED_RUSTFLAGS:'', CARGO_TARGET_DIR:path.join(output, 'target'),
  NIX_CONFIG:'min-free = 0'};
const commands = [];
function run(label, executable, args, expectedStatus = 0) {
  const process = spawnSync(executable, args, {
    cwd:root, env:environment, timeout:900_000, maxBuffer:16 * 1024 * 1024,
  });
  const stem = `${String(commands.length).padStart(2,'0')}-${label}`;
  const stdout = process.stdout ?? Buffer.alloc(0);
  const stderr = process.stderr ?? Buffer.alloc(0);
  external(`${stem}.stdout`, stdout);
  external(`${stem}.stderr`, stderr);
  const row = {label, executable, executable_sha256:sha(fs.readFileSync(executable)), args,
    status:process.status, signal:process.signal, error:process.error?.message ?? null,
    stdout:`${stem}.stdout`, stderr:`${stem}.stderr`,
    stdout_sha256:sha(stdout), stderr_sha256:sha(stderr)};
  commands.push(row);
  assert.equal(row.error, null, `${label}: process failed to spawn`);
  assert.equal(row.signal, null, `${label}: process signaled`);
  assert.equal(row.status, expectedStatus,
    `${label}: exit ${row.status} instead of ${expectedStatus}: ${stderr.toString('utf8')}`);
  return stdout.toString('utf8');
}
function json(label, executable, args, exit = 0) {
  return JSON.parse(run(label, executable, args, exit).trim());
}
const cargoOutput = run('production-build', path.join(rust, 'cargo'),
  ['build', '-p', 'noble-cli', '-p', 'noble-contracts', '--locked', '--offline', '-j', '4',
    '--message-format=json']);
function artifact(name, kind) {
  const matches = cargoOutput.split('\n').filter(line => line.startsWith('{'))
    .map(line => JSON.parse(line))
    .filter(row => row.reason === 'compiler-artifact' &&
      row.target.name === name && row.target.kind.includes(kind));
  const files = [...new Set(matches.flatMap(row => kind === 'bin' ? [row.executable] :
    row.filenames.filter(file => file.endsWith('.rlib'))).filter(Boolean))];
  assert.equal(files.length, 1, `expected exactly one compiled ${name}/${kind}`);
  assert.ok(files[0].startsWith(`${environment.CARGO_TARGET_DIR}${path.sep}`),
    `${name}/${kind} was not built in the external target directory`);
  return files[0];
}
const cli = artifact('noble', 'bin');
const checker = path.join(output, 'scase04-typed-check');
run('typed-peer-build', path.join(rust, 'rustc'), ['--edition=2021', '--crate-name',
  'scase04_typed_check', 'verification/scase04/check.rs',
  '--extern', `noble_contracts=${artifact('noble_contracts', 'lib')}`,
  '--extern', `noble_kernel=${artifact('noble_kernel', 'lib')}`,
  '-L', `dependency=${path.join(environment.CARGO_TARGET_DIR, 'debug/deps')}`,
  '-o', checker]);
const typed = json('independent-typed-peer', checker,
  [wit, ...negativeInputs.map(row => row.file), sumFile]);
assert.equal(typed.schema, 'noble-scase04-typed/v1');
assert.equal(typed.case_id, design.id);
assert.equal(typed.resource_kind, 1);
assert.deepEqual(typed.prefixes, prefixes, 'all four complete typed prefix shapes are required');
assert.deepEqual(typed.negatives, negativeInputs.map(row => ({
  type:row.type, operation:row.operation, word:row.word,
  actual_stack:`S ${row.actual_type}`, constraint:'eligibility:Data',
})), 'all 16 kernel eligibility diagnostics must identify the full value type and word');
assert.deepEqual(typed.positive_sum, {source:sumSource, output_type:'I64', lowered:false});
assert.deepEqual(Object.keys(typed.controls).sort(), [
  'wit_unsupported', 'source_unbound', 'source_type',
  'resource_open_close', 'resource_pair', 'data_quote_reflect', 'list_constructor',
].sort());
for (const [name, stage, kind, source] of [
  ['wit_unsupported', 'wit', 'unsupported', unsupportedWit],
  ['source_unbound', 'resolve', 'invalid', '7 not-a-word'],
  ['source_type', 'check', 'invalid', 'true 1 +'],
]) {
  const row = typed.controls[name];
  assert.equal(row.stage, stage, `${name}.stage`);
  assert.equal(row.kind, kind, `${name}.kind`);
  assert.equal(row.eligibility_join, false, `${name} must not be an eligibility rejection`);
  assert.equal(row.source.trim(), source.trim(), `${name}.source`);
}
for (const [name, source, outputType] of [
  ['resource_open_close', resourceSource, 'I64'],
  ['resource_pair', pairSource, 'I64'],
  ['data_quote_reflect', dataReflectSource, 'Syntax'],
  ['list_constructor', prefixes[3].source, prefixes[3].actual_type],
]) {
  assert.equal(typed.controls[name].source, source, `${name}.source`);
  assert.equal(typed.controls[name].output_type, outputType, `${name}.output_type`);
}
assert.equal(typed.controls.list_constructor.lowered, false);

const binding = json('real-resource-bindings', cli, ['component', 'bindings', wit, 'demo']);
assert.equal(binding.schema, 'noble-component/v1');
assert.equal(binding.outcome, 'typed-bindings');
assert.equal(binding.world, 'noble-test:recursive-resource/demo@1.0.0');
assert.equal(binding.component_emitted, false);
assert.deepEqual(binding.resources, [{
  identity:'noble-test:recursive-resource/counters@1.0.0#counter',
  kind:1, data:false, capture:false,
}]);
assert.deepEqual(binding.imports.map(row => row.word), ['counters.open', 'counters.close']);
assert.deepEqual(binding.imports.map(row => [row.input_types, row.output_types]), [
  [[], [prefixes[0].actual_type]], [[prefixes[0].actual_type], ['I64']],
]);
assert.deepEqual(binding.imports.map(row => [row.wit_parameters, row.wit_results]), [
  [[], ['Own(ResourceKind(1))']], [['Own(ResourceKind(1))'], ['S64']],
]);
assert.deepEqual(binding.exports.map(row => [row.word, row.input_types, row.output_types]),
  [['check', [], ['I64']]]);

const matrix = [];
for (const [index, row] of negativeInputs.entries()) {
  const directory = path.join(output, `negative-component-${index}`);
  const observed = json(`eligibility-${index}`, cli,
    ['component', 'compile', wit, 'demo', directory, `check=${row.file}`], 2);
  assert.equal(observed.schema, 'noble-component/v1');
  assert.equal(observed.outcome, 'error');
  assert.equal(observed.component_emitted, false);
  assert.equal(observed.diagnostic?.code, 'component-check');
  assert.equal(observed.diagnostic.message,
    `Export: kernel rejected a non-capturable value; word ${row.word}; required S Data; actual S ${row.actual_type}`,
    `eligibility-${index}: incorrect word, reason, or partial type`);
  assert.equal(fs.existsSync(directory), false, 'static refusal must precede component emission');
  matrix.push({type:row.type, operation:row.operation, word:row.word, source:row.source,
    source_sha256:sha(fs.readFileSync(row.file)), actual_type:row.actual_type, observed});
}
const controls = {typed:typed.controls, bindings:binding, positive_components:[], unrelated:[]};
for (const row of positives) {
  const directory = path.join(output, `positive-component-${row.label}`);
  const observed = json(`positive-${row.label}`, cli,
    ['component', 'compile', wit, 'demo', directory, `check=${row.file}`]);
  assert.equal(observed.schema, 'noble-component/v1');
  assert.equal(observed.outcome, 'compiled');
  assert.equal(observed.component_emitted, true);
  assert.equal(observed.independent_kernel_check, true);
  assert.equal(observed.world, binding.world);
  assert.deepEqual(fs.readFileSync(path.join(directory, 'world.wit')), Buffer.from(witBytes));
  assert.deepEqual(fs.readFileSync(path.join(directory, 'export-0.noble')),
    fs.readFileSync(row.file));
  assert.deepEqual(JSON.parse(fs.readFileSync(path.join(directory, 'report.json'))), observed);
  const component = path.join(directory, 'component.wasm');
  assert.ok(fs.statSync(component).size > 0, `${row.label}: no component bytes`);
  controls.positive_components.push({label:row.label, source:row.source, observed,
    component_sha256:sha(fs.readFileSync(component))});
}
for (const [label, witPath, sourcePath, exit, message] of [
  ['wit-malformed', badWit, positives[0].file, 2, /^Wit: unexpected WIT token$/],
  ['wit-unsupported-type', unsupportedWitFile, positives[0].file, 4, /^Wit: /],
  ['unbound-word', wit, unboundSource, 2, /^Export: .*unbound word/],
  ['unrelated-type', wit, illTypedSource, 2, /^Export: .*stack\/program join/],
]) {
  const directory = path.join(output, `unrelated-component-${label}`);
  const observed = json(`control-${label}`, cli,
    ['component', 'compile', witPath, 'demo', directory, `check=${sourcePath}`], exit);
  assert.equal(observed.schema, 'noble-component/v1');
  assert.equal(observed.component_emitted, false);
  assert.equal(observed.diagnostic?.code, 'component-check');
  assert.match(observed.diagnostic.message, message);
  assert.doesNotMatch(observed.diagnostic.message, /kernel rejected a non-capturable value/);
  assert.equal(observed.outcome, exit === 4 ? 'unsupported' : 'error');
  assert.equal(fs.existsSync(directory), false);
  controls.unrelated.push({label, source:fs.readFileSync(sourcePath, 'utf8').trim(),
    wit_sha256:sha(fs.readFileSync(witPath)), observed});
}
assert.deepEqual(inventory(), sources, 'producer source inventory changed during gate');
assert.deepEqual(historicalReceipts(), historical,
  'a historical acceptance receipt changed during gate');
for (const [name, row] of Object.entries({...inputs, ...prepromotion_inputs}))
  assert.equal(sha(fs.readFileSync(path.join(output, row.path))), row.sha256,
    `external input changed during gate: ${name}`);

const canonical_mapping = {
  stage:design.expected.stage, outcome:design.expected.outcome,
  cli_stage:'component-check', cli_outcome:'error',
  guest_requests:0, protected_operations:0,
  basis:'typed-eligibility-before-component-emission; no guest or runtime was invoked',
};
const receipt = {
  schema:'noble-scase04-recursive-resource-eligibility/v1', kind:'test', result:'passed',
  failures:[], integrity_failures:[],
  case:{id:design.id, input:design.input, expected:design.expected},
  canonical_mapping, external_raw_output:output, prepromotion_inputs,
  source_revision:`sha256:${sha(JSON.stringify(Object.entries(sources)))}`,
  source_sha256:sources, historical_receipt_sha256:historical,
  prepromotion_case_sha256:sources[caseFile],
  wit_sha256:inputs['inputs/recursive-resource.wit'].sha256, input_sha256:inputs,
  selected_rust:selected.tool_paths.quality_rust, selected_node:selected.tool_paths.node,
  binary_sha256:sha(fs.readFileSync(cli)), checker_sha256:sha(fs.readFileSync(checker)),
  commands, typed, matrix, controls,
  claim:'All 16 recursive ResourceKind(1) values are independently kernel-rejected for Data eligibility and rejected by production component compile before component publication, with complete diagnostic types. Three resource/Pair/Data positive components compile. The selected Sum and List positive prefixes are only typed; neither is claimed to have compiled or run.',
  assumptions:[
    'The fixture is an independently written real WIT interface; its kind-1 resource is registered by the parsed selected world, not by the bootstrap ResourceKind(0) fixture.',
    'The canonical check/eligibility-reject classification maps the component-check error and the independently observed kernel eligibility diagnostics; it is not a literal CLI outcome string.',
    'Zero guest requests and protected operations follow from static rejection before component emission and from never invoking a guest; these are not runtime counters or a host safety proof.',
    'The selected Rust toolchain, Node, Noble compiler, and typed checker are trusted for this finite observation. Sum component lowering, List-to-WIT export, general resource soundness, runtime owner law, and universal proof remain open.',
  ],
};
const receiptPath = path.join(output, 'acceptance.json');
fs.writeFileSync(receiptPath, `${JSON.stringify(receipt,null,2)}\n`, {flag:'wx'});
fs.chmodSync(receiptPath, 0o444);
console.log(JSON.stringify({result:'passed', receipt:receiptPath,
  receipt_sha256:sha(fs.readFileSync(receiptPath)),
  source_revision:receipt.source_revision, observations:matrix.length,
  commands:commands.length}));
