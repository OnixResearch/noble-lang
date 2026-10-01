// Test programs are ordinary declared-module source. The production library
// itself is always loaded byte-for-byte from crates/noble-contracts/src/fixtures/result.noble.
const helper = `module result_cases@1 [
  signature make_ok forall<S:stack> [ S -- S result@1.Result<I64,Text> ! pure ]
  export make_ok
  def make_ok [ 2 result@1.Result.Ok ]
  signature make_err forall<S:stack> [ S -- S result@1.Result<I64,Text> ! pure ]
  export make_err
  def make_err [ "fault" result@1.Result.Err ]
  signature make_three forall<S:stack> [ S -- S result@1.Result<I64,Text> ! pure ]
  export make_three
  def make_three [ 3 result@1.Result.Ok ]
  signature make_text_ok forall<S:stack> [ S -- S result@1.Result<Text,I64> ! pure ]
  export make_text_ok
  def make_text_ok [ "two" result@1.Result.Ok ]
  signature make_text_err forall<S:stack> [ S -- S result@1.Result<Text,I64> ! pure ]
  export make_text_err
  def make_text_err [ 9 result@1.Result.Err ]
]\n`;

const program = `module program_cases@1 [
  signature make_program forall<S:stack> [ S -- S result@1.Result<Program<I64,I64,pure>,Text> ! pure ]
  export make_program
  def make_program [ [ 1 + ] result@1.Result.Ok ]
]\n`;

const trace = `module result_host@1 [
  require emit Text -- ! test.emit
  export mark
  def mark [ emit ]
]\n`;

const syntax = `module syntax_cases@1 [
  signature make_syntax forall<S:stack> [ S -- S result@1.Result<I64,Syntax> ! pure ]
  export make_syntax
  def make_syntax [ [ 1 + ] reflect result@1.Result.Err ]
]\n`;

const resource = `module result_resource@1 [
  opaque CounterOwner Resource<test.counter> private
  export CounterOwner
]\n`;

const monomorphic = `module mono_data@1 [ variant State Job Program<I64,I64,pure> public Token Syntax public export State export run_job export run_token def run_job [ 5 [ 1 + ] State.Job [ run ] [ drop ] State.match ] def run_token [ [ 1 + ] reflect State.Token [ drop 11 ] [ drop 22 ] State.match ] ]\n`;

const inertResource = `module inert_resource@1 [
  signature make_inert forall<S:stack> [ S -- S result@1.Result<Program<Resource<test.counter>,Resource<test.counter>,test.emit>,Text> ! pure ]
  export make_inert
  def make_inert [ [ "unused" result_host@1.mark ] result@1.Result.Ok ]
]\n`;

const marker = (operation, arm) => `dx04-${operation}-${arm}`;
const effect = (operation, arm, body) =>
  `[ "${marker(operation, arm)}" result_host@1.mark ${body} ]`;
const cases = [
  { operation: 'map-success', input: 'Ok(2)', constructor: 'make_ok', word: 'map_ok',
    callback: effect('map-success', 'success', '1 +'), selected: true },
  { operation: 'map-success', input: 'Err(fault)', constructor: 'make_err', word: 'map_ok',
    callback: effect('map-success', 'error', '1 +'), selected: false },
  { operation: 'map-error', input: 'Ok(2)', constructor: 'make_ok', word: 'map_error',
    callback: effect('map-error', 'success', 'drop "mapped"'), selected: false },
  { operation: 'map-error', input: 'Err(fault)', constructor: 'make_err', word: 'map_error',
    callback: effect('map-error', 'error', 'drop "mapped"'), selected: true },
  { operation: 'chain', input: 'Ok(2)', constructor: 'make_ok', word: 'and_then',
    callback: effect('chain', 'success', 'drop result_cases@1.make_err'), selected: true },
  { operation: 'chain', input: 'Err(fault)', constructor: 'make_err', word: 'and_then',
    callback: effect('chain', 'error', 'drop result_cases@1.make_err'), selected: false },
  { operation: 'recover', input: 'Ok(2)', constructor: 'make_ok', word: 'or_else',
    callback: effect('recover', 'success', 'drop result_cases@1.make_three'), selected: false },
  { operation: 'recover', input: 'Err(fault)', constructor: 'make_err', word: 'or_else',
    callback: effect('recover', 'error', 'drop result_cases@1.make_three'), selected: true },
].map((item, index) => ({ ...item, index, source:
  `result_cases@1.${item.constructor} ${item.callback} result@1.${item.word}\n`,
  marker: marker(item.operation, item.input.startsWith('Ok(') ? 'success' : 'error') }));

export const librarySource = 'crates/noble-contracts/src/fixtures/result.noble';
export const hostBinding = { module: 'result_host', version: 1, adapter_identity: 'result-trace-v1' };
export const programs = Object.freeze({
  trace_module: trace,
  helper_module: helper,
  program_module: program,
  syntax_module: syntax,
  resource_module: resource,
  monomorphic_module: monomorphic,
  inert_resource_module: inertResource,
  pure_baseline: 'result_cases@1.make_ok [ 1 + ] result@1.map_ok\n',
  effectful_bypass: cases[1].source,
  second_success: `result_cases@1.make_text_ok ${effect('second', 'success', 'drop "three"')} result@1.map_ok\n`,
  second_error: `result_cases@1.make_text_err ${effect('second', 'error', 'drop 10')} result@1.map_error\n`,
  program_value: 'program_cases@1.make_program\n',
  program_run: 'program_cases@1.make_program [ 2 swap run ] result@1.map_ok\n',
  syntax_value: 'syntax_cases@1.make_syntax\n',
  syntax_match: 'syntax_cases@1.make_syntax [ drop 11 ] [ drop 22 ] result@1.Result.match\n',
  monomorphic_program: 'mono_data@1.run_job\n',
  monomorphic_syntax: 'mono_data@1.run_token\n',
  inert_resource_value: 'inert_resource@1.make_inert\n',
  inert_resource_match: 'inert_resource@1.make_inert [ drop 11 ] [ drop 22 ] result@1.Result.match\n',
  resource_success: '[ 1 + ] result@1.map_ok\n',
  resource_error: '[ 1 + ] result@1.map_ok\n',
  invalid_callback_stack: 'result_cases@1.make_ok [ drop 3 "extra" ] result@1.map_ok\n',
  resource_capture: 'quote\n',
  ...Object.fromEntries(cases.map(item => [`canonical_${item.index}`, item.source])),
});
export const bindings = Object.freeze({
  empty: '',
  trace: `bind result_host@1 test.emit ${hostBinding.adapter_identity} Text -- ! test.emit allow\n`,
});
export const canonicalCases = Object.freeze(cases);
