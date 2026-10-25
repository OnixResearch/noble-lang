#!/usr/bin/env node
// Numeric-Graph-v1 acceptance runner (`.cairn/changes/numeric-graph-v1`, issue #14).
//
//   NODE verification/numeric-graph-v1/acceptance.mjs plan NEW_EXTERNAL_DIR [--cases FILE]
//   NODE verification/numeric-graph-v1/acceptance.mjs run PLANNED_DIR --binary NOBLE
//     --base-binary BASE_NOBLE --base-commit SHA --gate-receipt GATE_RECEIPT
//     --tested-commit SHA --soft-float-log LOG [--binary-sha256 HEX]
//     [--base-binary-sha256 HEX] [--receipt FILE]
//
// NODE must be the policy-selected Node (`policy/tool-selection.json`), with no
// Node flags. `plan` freezes the case file, the committed inputs and every
// command; it runs nothing and is not acceptance evidence. `run` executes the
// exact inputs of NGRAPH-CASE-03..05, -07, -08 and -10..13 twice (the
// NGRAPH-CASE-15 determinism check) against a frozen copy of `--binary`, runs
// the CORE-01..18 workload on frozen copies of `--base-binary` and `--binary`
// (NGRAPH-CASE-01), binds the library cases to the gate `rust-tests` log and
// the soft-float log, and writes the receipt. Raw outputs stay in the external
// directory, bound by sha256. Fields that `noble ngraph` reports do not carry
// are recorded as not observable in the CLI, with the library tests that cover
// them. This runner never edits case state or evidence.
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { parseArgs } from 'node:util';
import { gitSourceProjection } from '../m8/source-projection.mjs';

const root = fs.realpathSync(path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..'));
const RUNNER = 'verification/numeric-graph-v1/acceptance.mjs';
const INPUTS = 'verification/numeric-graph-v1/inputs';
const RECEIPT = 'verification/numeric-graph-v1/acceptance.json';
const CASE_FILES = ['specs/conformance/numeric-graph-cases.json',
  '.cairn/changes/numeric-graph-v1/cases/numeric-graph-cases.json'];
const CORE_CASES = 'specs/conformance/cases.json';
const TOOL_SELECTION = 'policy/tool-selection.json';
const GIT = '/run/current-system/sw/bin/git';
const SCHEMA = 'noble-numeric-graph-v1-acceptance/v1';
const PLAN_SCHEMA = 'noble-numeric-graph-v1-plan/v1';
const PROFILE = 'Numeric-Graph-v1';
const REPORT = 'noble-ngraph-report/v1';
const REPETITIONS = 2;
const TIMEOUT_MS = 180_000;
const OVERSIZED_BYTES = 64 * 1024 * 1024 + 1;
const USAGE = `usage: NODE ${RUNNER} plan NEW_EXTERNAL_DIR [--cases FILE]
       NODE ${RUNNER} run PLANNED_DIR --binary NOBLE --base-binary BASE_NOBLE --base-commit SHA
         --gate-receipt FILE --tested-commit SHA --soft-float-log FILE
         [--binary-sha256 HEX] [--base-binary-sha256 HEX] [--receipt FILE]`;

// Source projection (`source_revision`): the numeric-graph sources, the CLI
// dispatch, the runner, its inputs and the case files it reads.
const SOURCE_FILES = ['Cargo.toml', 'Cargo.lock', 'crates/noble-cli/src/main.rs',
  'crates/noble-cli/src/calculator.rs', 'crates/noble-cli/src/ngraph.rs',
  'verification/m8/source-projection.mjs', RUNNER, CORE_CASES];
const SOURCE_TREES = ['crates/noble-ngraph', 'crates/noble-cli/src/ngraph',
  'crates/noble-ngraph-experiment', INPUTS];
// Projected sources the tested binary is built from: their bytes must equal
// the blobs of `--tested-commit`.
const built = file => file.startsWith('crates/') || file === 'Cargo.toml' || file === 'Cargo.lock';

// Committed inputs (verification/numeric-graph-v1/inputs): exact bytes and origin.
const FIXTURES = {
  'f1.ngraph': ['d40119550aeab5a1e8130917271e3e1b1e5ea35648ad11ce982bba2cf6f3b529',
    'F1: noble_ngraph::format::graph::print(&noble_ngraph_experiment::fixtures::f1())'],
  'f2.ngraph': ['fdada9074cb9c2e17143e410dbf99d83ce545c67d8ccfa4debb8aa810aa10b1d',
    'F2: format::graph::print(&fixtures::f2())'],
  'mse.ngraph': ['8df71aa98e1b6e8b07222e762f56f1c29c2b3fe0dcfeb42dd07515c9593a63d6',
    'MSE point loss: format::graph::print(&fixtures::mse_loss())'],
  'b5.points': ['ebcfe6556f665f4aa5e5da1fbe8777a4eafc2dbae17c793aa79e399938b7a987',
    'B5 64-point set (data seed 0x1414da7a): byte copy of crates/noble-cli/tests/ngraph/b5.points'],
  'b5-changed.points': ['b1eee0b5e6b67e77ba39f4c9c71383a61da6088e00925a8470561615b4b0c9e1',
    'b5.points with the first point target 0xbe0da150 changed to 0xbe0da151'],
  'f1.vectors': ['27f238823491c777264a7858a7ab7ddeba5880095329032857d9c015709f8bee',
    'four f32 vectors for F1'],
  'f1-fact.ngraph': ['ba19311e4d0d4cd8c4d3d28e6468b53dff02f4bff05b0feb5ff1538b8315093a',
    'F1 plus the input-validated fact `fact %0 range 0xbf800000 0x3f800000 input-validated`'],
  'f1-missing-schema.ngraph': ['967cdaf24341e1f09fb71029d097bcfda63c0632b0b03cfc5937597a08996b5e',
    'F1 without its schema line'],
  'f1-unknown-op.ngraph': ['fbad27bd56a6c4500ce35d75900ea4b8b347fe689353618638f30bb97d4d39f8',
    'F1 with `%56 = f32.add` replaced by the unknown op `f32.div`'],
  'f1-missing-subject.ngraph': ['bf8525a35e0cb00c131a73d61cd11c3d00ec0f05171f70563dbe8d8b450f18de',
    'F1 without its subject line'],
  'f1-odd-evidence.ngraph': ['b12033b22dbfca50d92c5439e44560aa253035f1ec429db05737902d62ba37f0',
    'F1 plus `fact %56 finite evidence abc` (odd-length evidence hex)'],
  'f1-inexact-const.ngraph': ['d2dbb9a59de9d7b1338ae1d090827f4760e41ad5689a6a76822a099fd769c774',
    'F1 plus `%57 = const f32 0.1`, `%58 = f32.add %56 %57`, `out %58`'],
  'f1-asserted-range.ngraph': ['5e7d9cbc8d80ef10492e84392dd85664e194cf90e433cc229a12dbf8d1866d0e',
    'F1 plus the caller-asserted fact `fact %56 range 0xbf800000 0x3f800000 asserted`'],
  'f2-loss.ngraph': ['f1acedc4a79e04935dcf1538f3c1dfd6e1dcd79eb9577476df7c15372b313108',
    'squared-error loss for F2: two model outputs, two targets, one f32 output'],
  'f2.points': ['557b76a2d1bd9ef59dd0004278527acee85efae8fc17f48f0bc33ab20cbd66f5',
    'three F2 training points'],
  'guard.ngraph': ['e45f4c05b361e025241832fcd64dfc0c71f7db928063c6ae514bd11db2c53767',
    'GUARD_GRAPH of crates/noble-cli/tests/ngraph.rs: `guard %0 bit`, input-validated %1 in [0, 1]'],
  'guard-trap.vectors': ['37be7fff1ae3e361fafcc182fd63b876fedc935eb3cfd12a44647dc03a30ca81',
    '0.5 held in the guarded bit'],
  'guard-reject.vectors': ['841f8da85b9bb77a7746280a1085b9d6ad6e8f8c352c0b5c99a8f880d5b69fbf',
    'x1 = 2 outside the input-validated range'],
  'guard-i64-types.vectors': ['5324533a1f185414b8d406859eb38abec0fffe6f097a87f7b9e13eb757e80944',
    'types line `i64 i64` for the f32 guard graph'],
  'guard-mixed.vectors': ['673c22afe6fa4e4e268354108d46d9ce8ace3ef710aa4bd8211c0c7ed181ea75',
    'a normal vector, a guard failure and an E-FACT-INPUT vector'],
  'i1.noble': ['5d364fd3341ceb103c93d11f066d9311cbab862b1b8ce439cbb0683f642977ac', 'I1 submissions'],
  'i2.noble': ['1876cd2b3af98c3f2ee38dce94b09e175834e0f5df47e9a8f77b7c86cc897fdb', 'I2 submissions'],
  'i3.noble': ['73f5e97a991c33f85bf8ac1f39ce02393b1553ed979737862d4e01b778d0194d', 'I3 submissions'],
  'i4.noble': ['e106b6ac16ca5d6aa462760d5995324a11ab7800e286fe62909dc5b249caed8b', 'I4 submissions'],
  'effect.noble': ['1b64d46ab4feea6f00e5478ec5af6d4b2316ad87181321f76489562d70a33b6b',
    'test.emit refusal source'],
  'control.noble': ['d573baa61acafae9b8c79e0de5858deb21c32099f306dd73240cb9a024f5f30a',
    'if refusal source'],
  'program-output.noble': ['ba8ed4bc2359f4b565cdd5be9af96a586c9e182a5e052958a3405d6419895ad5',
    'Program-output refusal source'],
  'unbound.noble': ['2eec4a88b23562a1cd1fe89ba4023d3f336e93a5c1df2375c9d4b7bf715966d4',
    'frontend refusal source: the unbound word `nosuchword`'],
  'i1.vectors': ['0a1c461815ec7c17dde07562ff37f5665fd924f757232f04597d435993fda85e',
    'I1 vector (5, 4)'],
  'b2-reassociation.ngraph': ['54b5acea3257f0c32bf685fe65f40711bd0d61f10ebcd853824199b04b58be2d',
    'format::graph::print of fixtures::b2_vectors()[0] (reassociation)'],
  'b2-reassociation.vectors': ['cb4fd4827888a1e7be06e755fd293339c8bc50e271a98e7f2cf4276a07ba961a',
    'b2_vectors()[0] inputs (2^24, 1, 1)'],
  'b2-fma-probe.ngraph': ['9558aadbe1c412ef095759f3e56a594fe6cad743dd70492c09cbf5c184267a93',
    'format::graph::print of fixtures::b2_vectors()[2] (FMA detection)'],
  'b2-fma-probe.vectors': ['fa9bb152c6393931999fa27d2bc7a70ea531003283f98a76c812a8a1e47ab98d',
    'b2_vectors()[2] inputs a = b = 1+2^-12, c = -(1+2^-11)'],
  'b2-lone-negative-zero.ngraph': ['e661dcf875cb0d5708f7e4d9e4bd5c0d446689cd72f197eabd710f5da1faac35',
    'format::graph::print of fixtures::b2_vectors()[4] (lone -0 term)'],
  'b2-lone-negative-zero.vectors': ['ea6c250c99d258d90c2415f37587a0aa4313c3ab356ac7c8d35d39c94e795fde',
    'b2_vectors()[4] input -0'],
  'b2-relu-negative-zero.ngraph': ['dedaa176c2c3e5ac19b57243a2d5ba1d6481878d3d9c4bd3d0cd9cd311512d90',
    'format::graph::print of fixtures::b2_vectors()[5] (relu(-0))'],
  'b2-relu-negative-zero.vectors': ['ea6c250c99d258d90c2415f37587a0aa4313c3ab356ac7c8d35d39c94e795fde',
    'b2_vectors()[5] input -0'],
  'b2-reglu-negative-one-zero.ngraph': ['3cb6b034685c4d26d1f7f12b9234359d35fe17a535681bf25a02b24a9da283be',
    'format::graph::print of fixtures::b2_vectors()[6] (reglu(-1, 0))'],
  'b2-reglu-negative-one-zero.vectors': ['90d23be95a6982b7d1011f1703c2dc651dfafb3fbe1cefaa2c8af8bf7ea4f28a',
    'b2_vectors()[6] inputs (-1, 0)'],
  'b2-overflow.ngraph': ['9158019797d97dfdaca6216ee1fe7d6f6210bf4ed5c6de57c5fdfbfe043b64f1',
    'format::graph::print of fixtures::b2_vectors()[8] (2^127 * 2)'],
  'b2-overflow.vectors': ['5ea26efa40c57631788861502cd986d8ad7167bf2189d06d9c1d6dc24e461bed',
    'b2_vectors()[8] inputs (2^127, 2)'],
  'b2-reglu-infinity-zero.ngraph': ['442168bb1807b9cde7e51a05aa47341de47bfca9f909cb949d0cacdb94469981',
    'format::graph::print of fixtures::b2_vectors()[9] (reglu(inf, 0))'],
  'b2-reglu-infinity-zero.vectors': ['f6c619d1a3eedc3e08d23562a81f4aaff23dc4e2e46568a6ee0bfe0a0a3d0c85',
    'b2_vectors()[9] inputs (inf, 0)'],
  'b2-subnormal.ngraph': ['ead039012daff9fc03a34f1ba11b9701a65d7036deb2c103bf7915c9c227bdb0',
    'format::graph::print of fixtures::b2_vectors()[11] (2^-126 * 0.5)'],
  'b2-subnormal.vectors': ['07105355d0c189b3eaf385789aa42259905879696d3866d2beac52c4d5d30a5a',
    'b2_vectors()[11] inputs (2^-126, 0.5)'],
  'model.ngraph': ['c1bdec6486c228a49788823fded2de956857298ee3d9cc503049d0eb724967ae',
    'MODEL of crates/noble-cli/tests/ngraph.rs: y = rn(rn(w x) + b), x in [-1, 1]'],
  'other.ngraph': ['c69aac991fbed32728c1f2c0dfc1871fc1dedc2cfb42cc412c8076cfe26d651b',
    'MODEL under the subject cli-train-model-other (another graph, same layout)'],
  'loss.ngraph': ['8f3e867cc5eac764a22c549129a406478fce7402f2a1f68c6fde64dcc8d91df7',
    'LOSS of crates/noble-cli/tests/ngraph.rs (squared error)'],
  'loss-short.ngraph': ['e7baae2609f810c17a5ce1de22698ebfca30924245f3f155bd43708804d93a44',
    'LOSS_SHORT: a loss graph with one input too few'],
  'data.points': ['d68d455e87efc62f0613df80ba0d2edb5e9f1b28e053062a55bf078e04ea7ac7',
    'DATA: targets on y = 2x + 1'],
  'infinite.points': ['eead6f0ed68908a3fafd5654e9e8903239c1c0190bb409262a81ed2c4ec02ce1',
    'DATA_INFINITE_TARGET: a point with an infinite target'],
  'step.ngraph': ['7f2f9b290fe85897b305a8679d969c2fb9aeb12cbd680340c18afd8c2aa607d2',
    'STEP_MODEL: param 1 reaches `%4 = f32.step` (guarded by %3)'],
  'step.points': ['b07cd016540a16d90f0943747726d8915bfcdcc8af0cecc34eaeb924c286fb5d',
    'STEP_DATA: every input 0'],
  'step-nan.points': ['0879382bc45e7187b3627c2a6e9cac1f791df9ac319f903371f75825a1de3055',
    'STEP_DATA_NAN: a NaN input fails the guard under any params'],
  'i64.ngraph': ['be8d78d6ea163bb727a89960068814354483e161b4fa3ab7b234f28f52775641',
    'I64_MODEL: an ngraph-i64/0 model'],
  'i64.points': ['052d86ea661e3d8a7777e62a66a17efeff5d471ddc43a158facac7ddc3ad7cdd',
    'I64_DATA for the i64 model and LOSS'],
};

// Case-file input variant -> expected key, scenario step id and arguments.
const CHECK_REFUSALS = [
  ['missing schema line', 'missing-schema', ['f1-missing-schema.ngraph']],
  ['unknown op', 'unknown-op', ['f1-unknown-op.ngraph']],
  ['missing subject line', 'missing-subject', ['f1-missing-subject.ngraph']],
  ['odd-length evidence hex', 'odd-evidence', ['f1-odd-evidence.ngraph']],
  ['const f32 0.1', 'inexact-const', ['f1-inexact-const.ngraph']],
  ['caller-asserted range fact', 'asserted-range', ['f1-asserted-range.ngraph']],
  ['--budget-live-nodes 4 on F1', 'live-nodes-4', ['f1.ngraph', '--budget-live-nodes', '4']],
  ['--budget-live-nodes 262145', 'live-nodes-raised', ['f1.ngraph', '--budget-live-nodes', '262145']],
  ['graph file of 64 MiB + 1 byte', 'oversized', ['f1-oversized.ngraph']],
  ['missing file', 'missing-file', ['missing.ngraph']],
  ['unknown flag', 'unknown-flag', ['f1.ngraph', '--bogus']],
];
const EXTRACT_REFUSALS = [
  ['test.emit', 'test-emit', 'effect.noble'],
  ['if', 'if', 'control.noble'],
  ['program output', 'program-output', 'program-output.noble'],
  ['frontend refusal', 'frontend-refusal', 'unbound.noble'],
  ['f32 profile', 'f32-profile', 'i1.noble'],
  ['expansion', 'expansion', 'i3.noble'],
];
const PROBES = [
  ['I1', 'I1', 'i1', 'i1.ngraph', 'i1.vectors'],
  ['reassociation affine(-0; 2^24, 1, 1)', 'reassociation', 'reassociation',
    'b2-reassociation.ngraph', 'b2-reassociation.vectors'],
  ['FMA probe a=b=1+2^-12, c=-(1+2^-11)', 'FMA probe', 'fma-probe', 'b2-fma-probe.ngraph',
    'b2-fma-probe.vectors'],
  ['lone -0 term', 'lone -0 term', 'lone-negative-zero', 'b2-lone-negative-zero.ngraph',
    'b2-lone-negative-zero.vectors'],
  ['relu(-0)', 'relu(-0)', 'relu-negative-zero', 'b2-relu-negative-zero.ngraph',
    'b2-relu-negative-zero.vectors'],
  ['reglu(-1, 0)', 'reglu(-1, 0)', 'reglu-negative-one-zero', 'b2-reglu-negative-one-zero.ngraph',
    'b2-reglu-negative-one-zero.vectors'],
  ['2^127 * 2', '2^127 * 2', 'overflow', 'b2-overflow.ngraph', 'b2-overflow.vectors'],
  ['reglu(inf, 0)', 'reglu(inf, 0)', 'reglu-infinity-zero', 'b2-reglu-infinity-zero.ngraph',
    'b2-reglu-infinity-zero.vectors'],
  ['2^-126 * 0.5', '2^-126 * 0.5', 'subnormal', 'b2-subnormal.ngraph', 'b2-subnormal.vectors'],
];
const OUTCOMES = [
  ['0.5 held in a guarded bit', '0.5 held in a guarded bit', 'trap',
    ['guard.ngraph', 'guard-trap.vectors']],
  ['input violating an input-validated range fact', 'input violating an input-validated range fact',
    'fact-input', ['guard.ngraph', 'guard-reject.vectors']],
  ['types line i64 for an f32 graph', 'types line i64 for an f32 graph', 'interface',
    ['guard.ngraph', 'guard-i64-types.vectors']],
  ['graph with param slots and no --snapshot', 'graph with param slots and no --snapshot',
    'no-snapshot', ['f1.ngraph', 'f1.vectors']],
  ['--fuel 1 on F1', '--fuel 1 on F1', 'fuel',
    ['f1.ngraph', 'f1.vectors', '--snapshot', 'f1-s0.snapshot', '--fuel', '1']],
  ['a normal vector followed by a guard failure and an E-FACT-INPUT vector', 'mixed file', 'mixed',
    ['guard.ngraph', 'guard-mixed.vectors']],
];
const FOLLOW_UPS = [
  'train again to a second path',
  'train --from the first snapshot for 10 steps',
  "train to the first snapshot's path",
  'eval F1 with the first snapshot before and after the follow-ups',
];
const STEP_OPT_IN = '%4:boolean=straight-through';
const TRAIN_REFUSALS = [
  ['param reaching a step node without --opt-in', 'no opt-in', 'no-opt-in',
    ['step.ngraph', 'loss.ngraph', 'step.points', 2, []]],
  ['the same with --opt-in %N:boolean=straight-through', 'with opt-in', 'with-opt-in',
    ['step.ngraph', 'loss.ngraph', 'step.points', 2, ['--opt-in', STEP_OPT_IN]]],
  ['an ngraph-i64/0 model', 'i64 model', 'i64-model', ['i64.ngraph', 'loss.ngraph', 'i64.points', 2, []]],
  ['a data point with an infinite target', 'infinite target', 'infinite-target',
    ['model.ngraph', 'loss.ngraph', 'infinite.points', 1, []]],
  ['--steps 1000001', 'steps', 'steps', ['model.ngraph', 'loss.ngraph', 'data.points', 1_000_001, []]],
  ['--from a snapshot of another graph', 'foreign parent', 'foreign-parent',
    ['model.ngraph', 'loss.ngraph', 'data.points', 3, ['--from', 'other-s0.snapshot']]],
  ['a data point failing a guard under the initial params', 'guard-failing point', 'guard-failing-point',
    ['step.ngraph', 'loss.ngraph', 'step-nan.points', 2, ['--opt-in', STEP_OPT_IN]]],
  ['a loss graph with one input too few', 'loss arity', 'loss-arity',
    ['model.ngraph', 'loss-short.ngraph', 'data.points', 3, []]],
  ['missing --policy', 'missing --policy', 'missing-policy',
    ['model.ngraph', 'loss.ngraph', 'data.points', 3, []]],
];
const BINDINGS = [
  ['one param byte flipped', 'param byte', 'param-byte', 'tampered-param.snapshot'],
  ['one provenance field changed', 'provenance field', 'provenance-field', 'tampered-seed.snapshot'],
  ['snapshot of F2 used with F1', 'other graph', 'other-graph', 'f2-s0.snapshot'],
  ['two param lines swapped with recomputed ids', 'layout', 'layout', 'swapped.snapshot'],
  ['F1 with one input-validated fact removed', 'changed facts', 'changed-facts', 'f1-fact-s0.snapshot'],
  ['--snapshot-id of another snapshot', 'pinned id', 'pinned-id', 'b5.snapshot'],
  ['unknown snapshot schema line', 'schema', 'schema', 'schema.snapshot'],
];
const REPLAYS = [
  ['verify --replay of the NGRAPH-CASE-10 snapshot', 'seed replay', 'replay-seed',
    ['b5.snapshot', 'b5.points', []]],
  ['verify --replay --from of the 10-step child', 'parent replay', 'replay-parent',
    ['b5-child.snapshot', 'b5.points', ['--from', 'b5.snapshot']]],
  ['replay with one data point changed', 'changed data', 'replay-changed-data',
    ['b5.snapshot', 'b5-changed.points', []]],
  ['replay with the wrong parent', 'wrong parent', 'replay-wrong-parent',
    ['b5-child.snapshot', 'b5.points', ['--from', 'b5-s0.snapshot']]],
  ['a library-forged file with altered params and recomputed snapshot and provenance digests', 'forged',
    'replay-forged', ['forged.snapshot', 'b5.points', []]],
];

// Case -> library tests that must each appear exactly once as `test NAME ... ok`
// in the gate `rust-tests` log (plan section 3 table, plus additions A1 and A3).
const LIBRARY_TESTS = {
  'NGRAPH-CASE-01': ['eligibility::core_15_resource_matrix', 'mutations::core_16_candidate_mutation_matrix'],
  'NGRAPH-CASE-03': ['check_reports_every_d7_field', 'extract_i1_to_i4_round_trip_through_check_and_eval'],
  'NGRAPH-CASE-04': ['check_refusal_classes', 'check_budgets_and_usage',
    'refusals_before_evaluation_evaluate_no_node'],
  'NGRAPH-CASE-05': ['extract_i1_to_i4_round_trip_through_check_and_eval', 'extract_refusals_publish_nothing',
    'extract_publication_never_replaces_existing_paths', 'every_source_control_is_refused_with_its_d6_code',
    'b2_vector_observations_and_b3_codes_match_the_record'],
  'NGRAPH-CASE-06': ['every_corpus_graph_round_trips_with_its_identity',
    'authored_name_sugar_comments_and_spacing_are_not_identity_bearing',
    'evidence_and_range_literals_are_symmetric',
    'comments_blank_lines_and_spacing_do_not_change_any_corpus_graph'],
  'NGRAPH-CASE-07': ['eval_probes_are_bitwise_in_both_evaluators',
    'extract_i1_to_i4_round_trip_through_check_and_eval'],
  'NGRAPH-CASE-08': ['eval_outcome_classes_and_first_failure_exit', 'eval_refuses_inputs_before_evaluation',
    'refusals_before_evaluation_evaluate_no_node'],
  'NGRAPH-CASE-09': ['f32_tokens_are_exact_encodings_or_exact_decimals',
    'vector_files_are_typed_and_refused_as_a_whole', 'data_files_split_inputs_and_targets_at_one_separator',
    'decimal_exactness_is_decided_inside_the_binary32_window', 'a_leading_plus_sign_is_not_a_number',
    'eval_refuses_inputs_before_evaluation'],
  'NGRAPH-CASE-10': ['b5_training_through_the_cli_reproduces_the_recorded_identities',
    'train_publishes_immutable_snapshots'],
  'NGRAPH-CASE-11': ['train_contract_refusals_publish_nothing'],
  'NGRAPH-CASE-12': ['snapshot_integrity_and_binding_refusals',
    'provenance_consistent_forgeries_are_refused_by_layout_or_replay',
    'refusals_before_evaluation_evaluate_no_node', 'wrong_graph_layout_and_stale_snapshots_are_refused'],
  'NGRAPH-CASE-13': ['verify_binds_and_replays_provenance',
    'provenance_consistent_forgeries_are_refused_by_layout_or_replay',
    'b5_training_through_the_cli_reproduces_the_recorded_identities'],
  'NGRAPH-CASE-14': ['graph_identities_and_fact_bindings_match_the_record',
    'b2_vector_observations_and_b3_codes_match_the_record',
    'b5_training_identities_match_the_record_and_the_receipt', 'seeded_campaigns_match_the_record',
    'special_values_match_std', 'powers_of_two_are_exact', 'seeded_bit_patterns_match_std'],
};
const CLI_TESTS = 'crates/noble-cli/tests/ngraph.rs';
const IDENTITY_TESTS = 'crates/noble-ngraph-experiment/tests/identity_agreement.rs';
const TEST_FILES = {
  'eligibility::core_15_resource_matrix': 'crates/noble-cli/tests/core/eligibility.rs',
  'mutations::core_16_candidate_mutation_matrix': 'crates/noble-cli/tests/core/mutations.rs',
  check_reports_every_d7_field: CLI_TESTS,
  extract_i1_to_i4_round_trip_through_check_and_eval: CLI_TESTS,
  check_refusal_classes: CLI_TESTS,
  check_budgets_and_usage: CLI_TESTS,
  refusals_before_evaluation_evaluate_no_node: 'crates/noble-ngraph/tests/ir_check.rs',
  extract_refusals_publish_nothing: CLI_TESTS,
  extract_publication_never_replaces_existing_paths: CLI_TESTS,
  every_source_control_is_refused_with_its_d6_code: 'crates/noble-ngraph-experiment/tests/extract_b3.rs',
  b2_vector_observations_and_b3_codes_match_the_record: IDENTITY_TESTS,
  every_corpus_graph_round_trips_with_its_identity: 'crates/noble-ngraph-experiment/tests/format_roundtrip.rs',
  authored_name_sugar_comments_and_spacing_are_not_identity_bearing: 'crates/noble-ngraph/tests/format_graph.rs',
  evidence_and_range_literals_are_symmetric: 'crates/noble-ngraph/tests/format_graph.rs',
  comments_blank_lines_and_spacing_do_not_change_any_corpus_graph:
    'crates/noble-ngraph-experiment/tests/format_roundtrip.rs',
  eval_probes_are_bitwise_in_both_evaluators: CLI_TESTS,
  eval_outcome_classes_and_first_failure_exit: CLI_TESTS,
  eval_refuses_inputs_before_evaluation: CLI_TESTS,
  f32_tokens_are_exact_encodings_or_exact_decimals: 'crates/noble-ngraph/tests/format_values.rs',
  vector_files_are_typed_and_refused_as_a_whole: 'crates/noble-ngraph/tests/format_values.rs',
  data_files_split_inputs_and_targets_at_one_separator: 'crates/noble-ngraph/tests/format_values.rs',
  decimal_exactness_is_decided_inside_the_binary32_window: 'crates/noble-ngraph/tests/format_values.rs',
  a_leading_plus_sign_is_not_a_number: 'crates/noble-ngraph/tests/format_values.rs',
  b5_training_through_the_cli_reproduces_the_recorded_identities: CLI_TESTS,
  train_publishes_immutable_snapshots: CLI_TESTS,
  train_contract_refusals_publish_nothing: CLI_TESTS,
  snapshot_integrity_and_binding_refusals: CLI_TESTS,
  provenance_consistent_forgeries_are_refused_by_layout_or_replay: CLI_TESTS,
  wrong_graph_layout_and_stale_snapshots_are_refused: 'crates/noble-ngraph-experiment/tests/b5_training.rs',
  verify_binds_and_replays_provenance: CLI_TESTS,
  graph_identities_and_fact_bindings_match_the_record: IDENTITY_TESTS,
  b5_training_identities_match_the_record_and_the_receipt: IDENTITY_TESTS,
  seeded_campaigns_match_the_record: IDENTITY_TESTS,
  special_values_match_std: 'crates/noble-ngraph/tests/soft_float.rs',
  powers_of_two_are_exact: 'crates/noble-ngraph/tests/soft_float.rs',
  seeded_bit_patterns_match_std: 'crates/noble-ngraph/tests/soft_float.rs',
  ten_million_bit_patterns_match_std: 'crates/noble-ngraph/tests/soft_float.rs',
};
const SOFT_FLOAT_TEST = 'ten_million_bit_patterns_match_std';
const NO_NODE = 'refusals_before_evaluation_evaluate_no_node';
// Keys a Phase 7 schema completion may add to every case (plan section 3).
const COMPLETION_KEYS = ['stage', 'outcome', 'guest_requests', 'protected_operations'];
const NO_GUEST = 'noble-ngraph-report/v1 lines carry no guest-request or protected-operation counter; '
  + 'noble ngraph starts no guest, Node, Wasm or subprocess (crates/noble-cli/src/ngraph.rs module contract)';
const REVIEW = 'claims review, recorded separately (verification/numeric-graph-v1/review.json); not produced by this runner';

const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const fileSha = file => sha(fs.readFileSync(file));
const stable = value => Array.isArray(value) ? value.map(stable)
  : value && typeof value === 'object' ? Object.fromEntries(Object.keys(value).sort()
    .map(key => [key, stable(value[key])])) : value;
const same = (left, right) => JSON.stringify(stable(left)) === JSON.stringify(stable(right));
const hex64 = text => typeof text === 'string' && /^[0-9a-f]{64}$/.test(text);
const display = file => file === root || file.startsWith(`${root}${path.sep}`)
  ? path.relative(root, file).split(path.sep).join('/') : file;
const errorText = error => String(error?.stack ?? error);

// ---------------------------------------------------------------------------
// BLAKE3 (one-shot, unkeyed) and the D6 snapshot identities, written out
// independently of the CLI so that forged files are internally consistent.
// ---------------------------------------------------------------------------

const B3_IV = [0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19];
const B3_PERMUTATION = [2, 6, 3, 10, 7, 0, 4, 13, 1, 11, 12, 5, 9, 14, 15, 8];
const rotr = (word, bits) => ((word >>> bits) | (word << (32 - bits))) >>> 0;

function b3compress(cv, block, counter, length, flags) {
  const s = new Uint32Array(16);
  s.set(cv);
  s.set(B3_IV.slice(0, 4), 8);
  s[12] = counter % 0x1_0000_0000;
  s[13] = Math.floor(counter / 0x1_0000_0000);
  s[14] = length;
  s[15] = flags;
  let m = Uint32Array.from(block);
  const g = (a, b, c, d, x, y) => {
    s[a] = s[a] + s[b] + x; s[d] = rotr(s[d] ^ s[a], 16);
    s[c] = s[c] + s[d]; s[b] = rotr(s[b] ^ s[c], 12);
    s[a] = s[a] + s[b] + y; s[d] = rotr(s[d] ^ s[a], 8);
    s[c] = s[c] + s[d]; s[b] = rotr(s[b] ^ s[c], 7);
  };
  for (let round = 0; round < 7; round += 1) {
    g(0, 4, 8, 12, m[0], m[1]); g(1, 5, 9, 13, m[2], m[3]);
    g(2, 6, 10, 14, m[4], m[5]); g(3, 7, 11, 15, m[6], m[7]);
    g(0, 5, 10, 15, m[8], m[9]); g(1, 6, 11, 12, m[10], m[11]);
    g(2, 7, 8, 13, m[12], m[13]); g(3, 4, 9, 14, m[14], m[15]);
    const previous = m;
    m = Uint32Array.from(B3_PERMUTATION, index => previous[index]);
  }
  for (let i = 0; i < 8; i += 1) {
    s[i] ^= s[i + 8];
    s[i + 8] ^= cv[i];
  }
  return s;
}

function b3words(bytes) {
  const words = new Uint32Array(16);
  for (let i = 0; i < bytes.length; i += 1) words[i >> 2] |= bytes[i] << (8 * (i & 3));
  return words;
}

export function blake3(input) {
  const bytes = Buffer.from(input);
  const chunks = Math.max(1, Math.ceil(bytes.length / 1024));
  const chunkOutput = index => {
    const chunk = bytes.subarray(index * 1024, Math.min(bytes.length, (index + 1) * 1024));
    const blocks = Math.max(1, Math.ceil(chunk.length / 64));
    let cv = Uint32Array.from(B3_IV);
    for (let block = 0; block < blocks - 1; block += 1) {
      cv = b3compress(cv, b3words(chunk.subarray(block * 64, (block + 1) * 64)), index, 64,
        block === 0 ? 1 : 0).slice(0, 8);
    }
    const last = chunk.subarray((blocks - 1) * 64);
    return { cv, block: b3words(last), counter: index, length: last.length, flags: (blocks === 1 ? 1 : 0) | 2 };
  };
  const chaining = output => b3compress(output.cv, output.block, output.counter, output.length,
    output.flags).slice(0, 8);
  const parent = (left, right) => {
    const block = new Uint32Array(16);
    block.set(left);
    block.set(right, 8);
    return { cv: Uint32Array.from(B3_IV), block, counter: 0, length: 64, flags: 4 };
  };
  const stack = [];
  for (let index = 0; index < chunks - 1; index += 1) {
    let cv = chaining(chunkOutput(index));
    for (let total = index + 1; (total & 1) === 0; total >>= 1) cv = chaining(parent(stack.pop(), cv));
    stack.push(cv);
  }
  let output = chunkOutput(chunks - 1);
  while (stack.length) output = parent(stack.pop(), chaining(output));
  const words = b3compress(output.cv, output.block, output.counter, output.length, output.flags | 8);
  const digest = Buffer.alloc(32);
  for (let i = 0; i < 8; i += 1) digest.writeUInt32LE(words[i], 4 * i);
  return digest.toString('hex');
}

// `H(domain ‖ 0x00 ‖ fields)` with `bytes(x) = u64le(len) ‖ x` and `str(s) = bytes(utf8 s)`.
class Identity {
  constructor(domain) { this.parts = [Buffer.from(domain, 'utf8'), Buffer.from([0])]; }
  raw(bytes) { this.parts.push(Buffer.from(bytes)); return this; }
  u32(value) { const b = Buffer.alloc(4); b.writeUInt32LE(value); return this.raw(b); }
  u64(value) { const b = Buffer.alloc(8); b.writeBigUInt64LE(BigInt(value)); return this.raw(b); }
  bytes(value) { return this.u64(value.length).raw(value); }
  str(text) { return this.bytes(Buffer.from(text, 'utf8')); }
  hex() { return blake3(Buffer.concat(this.parts)); }
}

// Rewrites a published snapshot's param records with `edit` and recomputes
// its layout key, ParamSnapshotId and provenance digest (design D6).
export function forgeSnapshot(text, edit) {
  const lines = text.split('\n');
  assert.equal(lines.pop(), '', 'a snapshot file ends with a newline');
  const tokens = key => {
    const found = lines.filter(line => line.split(' ')[0] === key);
    assert.equal(found.length, 1, `exactly one ${key} record`);
    return found[0].split(' ').slice(1);
  };
  const unhex = digits => {
    assert.match(digits, /^(?:[0-9a-f]{2})+$/, 'lowercase hex');
    return Buffer.from(digits, 'hex');
  };
  const params = edit(lines.filter(line => line.startsWith('param ')).map(line => {
    const match = /^param (\d+) 0x([0-9a-f]{8})$/.exec(line);
    assert.ok(match, `param record: ${line}`);
    return [Number(match[1]), Number.parseInt(match[2], 16)];
  }));
  const graph = unhex(tokens('graph')[0]);
  const layout = new Identity('noble.ngraph.param-layout.v0').u64(params.length);
  for (const [slot] of params) layout.u32(slot).str('f32');
  const layoutHex = layout.hex();
  const snapshot = new Identity('noble.ngraph.params.v0').bytes(graph).u64(params.length);
  for (const [slot] of params) snapshot.u32(slot);
  const values = Buffer.alloc(4 * params.length);
  params.forEach(([, bits], index) => values.writeUInt32LE(bits, 4 * index));
  const snapshotHex = snapshot.bytes(values).hex();
  const p = new Identity('noble.ngraph.provenance.v0').bytes(unhex(snapshotHex)).bytes(graph)
    .bytes(unhex(layoutHex)).u64(params.length);
  for (const [slot, bits] of params) p.u32(slot).u32(bits);
  p.bytes(unhex(tokens('optimizer-state')[0])).bytes(unhex(tokens('parent')[0]));
  const subject = tokens('subject');
  assert.equal(subject[2], 'closure', 'subject record');
  p.str(subject[0]).bytes(unhex(subject[1]));
  const closure = subject[3] === '-' ? [] : subject[3].split(',').map(id => BigInt(id));
  p.u64(closure.length);
  for (const id of closure) p.u64(id);
  const profile = tokens('profile');
  p.str(profile[0]).str(profile[1]);
  p.bytes(unhex(tokens('facts')[0])).bytes(unhex(tokens('loss')[0])).bytes(unhex(tokens('data')[0]));
  p.str(tokens('policy')[0]);
  const optIns = lines.filter(line => line.startsWith('opt-in ')).map(line => line.split(' ').slice(1));
  p.u64(optIns.length);
  for (const optIn of optIns) {
    p.u32(Number(optIn[0].replace(/^%/, ''))).str(optIn[1]).str(optIn[2]);
    if (optIn[3] !== undefined) p.u32(Number.parseInt(optIn[3].replace(/^0x/, ''), 16));
  }
  p.u64(BigInt(tokens('seed')[0])).str(tokens('prng')[0]);
  const optimizer = tokens('optimizer');
  p.str(optimizer[0]);
  for (const index of [2, 4, 6, 8]) p.u64(BigInt(optimizer[index]));
  p.u64(BigInt(tokens('steps')[0])).str(tokens('tools').join(' '));
  const provenance = p.hex();
  const out = [];
  let written = false;
  for (const line of lines) {
    const key = line.split(' ')[0];
    if (key === 'snapshot') out.push(`snapshot ${snapshotHex}`);
    else if (key === 'layout') out.push(`layout ${layoutHex}`);
    else if (key === 'provenance') out.push(`provenance ${provenance}`);
    else if (key !== 'param') out.push(line);
    else if (!written) {
      written = true;
      for (const [slot, bits] of params) out.push(`param ${slot} 0x${bits.toString(16).padStart(8, '0')}`);
    }
  }
  return `${out.join('\n')}\n`;
}

const EDITS = {
  'flip-first-param-bit': params => params.map(([slot, bits], index) =>
    [slot, index === 0 ? (bits ^ 1) >>> 0 : bits]),
  'swap-first-two-params': params => [params[1], params[0], ...params.slice(2)],
};

// ---------------------------------------------------------------------------
// Repository, case file and input review.
// ---------------------------------------------------------------------------

function git(args, { allow = [0] } = {}) {
  const result = spawnSync(GIT, ['-C', root, ...args], {
    encoding: 'utf8', maxBuffer: 64 * 1024 * 1024,
    env: { ...process.env, GIT_OPTIONAL_LOCKS: '0', GIT_CONFIG_NOSYSTEM: '1', GIT_PAGER: 'cat', LC_ALL: 'C' },
  });
  assert.equal(result.error, undefined, `git ${args[0]}: launch failure`);
  assert.ok(allow.includes(result.status), `git ${args.join(' ')}: ${result.stderr}`);
  return { status: result.status, stdout: result.stdout };
}

function selectedNode() {
  const selection = JSON.parse(fs.readFileSync(path.join(root, TOOL_SELECTION), 'utf8'));
  const node = path.join(selection.tool_paths.node.output, 'bin/node');
  const real = fs.realpathSync(node);
  assert.equal(fs.realpathSync(process.execPath), real, `run ${RUNNER} with the policy-selected Node ${node}`);
  assert.deepEqual(process.execArgv, [], 'unreviewed Node flags');
  return { path: node, realpath: real, sha256: fileSha(real), version: process.version };
}

function outside(directory) {
  const target = path.resolve(directory);
  assert.ok(target !== root && !target.startsWith(`${root}${path.sep}`),
    'the output directory must be outside the repository');
  assert.equal(fs.realpathSync(path.dirname(target)), path.dirname(target),
    'the output directory parent must not be a symlink');
  return target;
}

const byName = (left, right) => left.name < right.name ? -1 : left.name > right.name ? 1 : 0;

function walkFiles(directory, into) {
  for (const entry of fs.readdirSync(path.join(root, directory), { withFileTypes: true }).sort(byName)) {
    const name = `${directory}/${entry.name}`;
    if (entry.isDirectory()) walkFiles(name, into);
    else if (entry.isFile()) into.add(name);
    else throw Error(`unreviewed non-file source entry: ${name}`);
  }
  return into;
}

function projectionFiles(caseFile) {
  const files = new Set(SOURCE_FILES);
  if (caseFile !== null) files.add(caseFile);
  for (const tree of SOURCE_TREES) walkFiles(tree, files);
  return [...files].sort();
}

function resolveCaseFile(option) {
  if (option !== undefined) {
    const file = path.resolve(option);
    assert.ok(fs.statSync(file).isFile(), `--cases ${option}: not a file`);
    return file;
  }
  const found = CASE_FILES.filter(name => fs.existsSync(path.join(root, name)));
  assert.equal(found.length, 1, `exactly one numeric-graph case file expected, found ${JSON.stringify(found)}`);
  return path.join(root, found[0]);
}

// Asserts the case inputs this runner executes; returns the case rows by id.
function reviewCases(document) {
  assert.equal(typeof document.revision, 'string', 'case file revision');
  const ids = Array.from({ length: 15 }, (_, index) => `NGRAPH-CASE-${String(index + 1).padStart(2, '0')}`);
  assert.deepEqual(document.cases.map(row => row.id), ids, 'NGRAPH-CASE-01..15 in order');
  const cases = new Map(document.cases.map(row => [row.id, row]));
  for (const row of cases.values()) {
    assert.equal(row.profile, PROFILE, `${row.id}: profile`);
    assert.ok(row.input && row.expected && typeof row.expected === 'object', `${row.id}: input and expected`);
  }
  const input = id => cases.get(id).input;
  assert.deepEqual(input('NGRAPH-CASE-03').graphs.map(graph => graph.name),
    ['F1 printed by format::graph::print', 'I1 written by noble ngraph extract'], 'NGRAPH-CASE-03 graphs');
  assert.deepEqual(input('NGRAPH-CASE-04').variants, CHECK_REFUSALS.map(([variant]) => variant),
    'NGRAPH-CASE-04 variants');
  const five = input('NGRAPH-CASE-05');
  assert.deepEqual(Object.keys(five.sources), ['I1', 'I2', 'I3', 'I4'], 'NGRAPH-CASE-05 sources');
  assert.deepEqual(Object.keys(five.refusals), [...EXTRACT_REFUSALS.map(([key]) => key), 'existing out'],
    'NGRAPH-CASE-05 refusals');
  assert.equal(five.refusals['frontend refusal'], 'an unbound word');
  assert.equal(five.refusals['f32 profile'], '--profile ngraph-f32/0');
  assert.equal(five.refusals.expansion, 'I3 with --budget-expansion 2');
  assert.equal(five.refusals['existing out'], '--out naming an existing file');
  const seven = input('NGRAPH-CASE-07');
  assert.deepEqual(seven.evaluators, ['default (compiled)', '--reference'], 'NGRAPH-CASE-07 evaluators');
  assert.deepEqual(seven.vectors.map(vector => vector.graph), PROBES.map(([graph]) => graph),
    'NGRAPH-CASE-07 vectors');
  assert.deepEqual(seven.vectors[0].values, ['5', '4'], 'NGRAPH-CASE-07 I1 values');
  assert.deepEqual(input('NGRAPH-CASE-08').vectors, OUTCOMES.map(([variant]) => variant),
    'NGRAPH-CASE-08 vectors');
  const ten = input('NGRAPH-CASE-10');
  assert.deepEqual([ten.model, ten.loss, ten.policy, ten.optimizer, ten.steps],
    ['F1', 'MSE point loss', 'ngraph-deriv-real/0', 'adam/0 defaults', 500], 'NGRAPH-CASE-10 contract');
  assert.match(ten.seed, /^0x[0-9a-f]{1,16}$/, 'NGRAPH-CASE-10 seed');
  assert.match(ten.data, /data seed 0x1414da7a/, 'NGRAPH-CASE-10 data');
  assert.deepEqual(ten.follow_up, FOLLOW_UPS, 'NGRAPH-CASE-10 follow-ups');
  assert.deepEqual(input('NGRAPH-CASE-11').variants, TRAIN_REFUSALS.map(([variant]) => variant),
    'NGRAPH-CASE-11 variants');
  assert.deepEqual(input('NGRAPH-CASE-12').commands, ['eval', 'verify'], 'NGRAPH-CASE-12 commands');
  assert.deepEqual(input('NGRAPH-CASE-12').variants, BINDINGS.map(([variant]) => variant),
    'NGRAPH-CASE-12 variants');
  assert.deepEqual(input('NGRAPH-CASE-13').variants, REPLAYS.map(([variant]) => variant),
    'NGRAPH-CASE-13 variants');
  assert.ok(hex64(cases.get('NGRAPH-CASE-14').expected.b5_s0), 'NGRAPH-CASE-14 b5_s0');
  return cases;
}

const withInputs = text => {
  const match = /^(.*) with --inputs (\S+)$/.exec(text);
  assert.ok(match, `source and --inputs: ${text}`);
  return { source: `${match[1]}\n`, inputs: match[2] };
};

// The committed inputs: exact reviewed bytes, and the case inputs they spell.
function reviewFixtures(cases) {
  const directory = path.join(root, INPUTS);
  assert.deepEqual(fs.readdirSync(directory).sort(), Object.keys(FIXTURES).sort(),
    `${INPUTS}: exactly the reviewed fixtures`);
  const bytes = {};
  for (const [name, [digest]] of Object.entries(FIXTURES)) {
    const file = path.join(directory, name);
    assert.ok(fs.lstatSync(file).isFile(), `${name}: plain file`);
    bytes[name] = fs.readFileSync(file);
    assert.equal(sha(bytes[name]), digest, `${name}: reviewed bytes`);
  }
  const text = name => bytes[name].toString('utf8');
  for (const [id, row] of Object.entries(cases.get('NGRAPH-CASE-05').input.sources)) {
    assert.equal(text(`${id.toLowerCase()}.noble`), `${row.submissions.join('\n')}\n`, `${id}: case submissions`);
  }
  const refusals = cases.get('NGRAPH-CASE-05').input.refusals;
  for (const [key, , file] of EXTRACT_REFUSALS.slice(0, 3)) {
    const { source, inputs } = withInputs(refusals[key]);
    assert.equal(text(file), source, `${key}: case source`);
    assert.equal(inputs, 'i64', `${key}: case --inputs`);
  }
  const values = cases.get('NGRAPH-CASE-07').input.vectors[0].values;
  assert.equal(text('i1.vectors'), `noble-ngraph-vectors/0\ntypes i64 i64\nv ${values.join(' ')}\n`, 'I1 vector');
  const f1 = text('f1.ngraph').split('\n');
  assert.deepEqual([f1[0], f1.at(-2), f1.at(-1)], ['ngraph-ir/0 ngraph-f32/0', 'out %56', ''], 'F1 shape');
  const beforeOut = extra => [...f1.slice(0, -2), ...extra, 'out %56', ''].join('\n');
  const derived = {
    'f1-missing-schema.ngraph': f1.slice(1).join('\n'),
    'f1-unknown-op.ngraph': text('f1.ngraph').replace('%56 = f32.add %54 %55\n', '%56 = f32.div %54 %55\n'),
    'f1-missing-subject.ngraph': [f1[0], ...f1.slice(2)].join('\n'),
    'f1-odd-evidence.ngraph': beforeOut(['fact %56 finite evidence abc']),
    'f1-inexact-const.ngraph': [...f1.slice(0, -2), '%57 = const f32 0.1', '%58 = f32.add %56 %57', 'out %58', '']
      .join('\n'),
    'f1-asserted-range.ngraph': beforeOut(['fact %56 range 0xbf800000 0x3f800000 asserted']),
    'f1-fact.ngraph': beforeOut(['fact %0 range 0xbf800000 0x3f800000 input-validated']),
  };
  for (const [name, expected] of Object.entries(derived)) {
    assert.equal(text(name), expected, `${name}: the stated edit of f1.ngraph`);
  }
  const points = name => text(name).split('\n').filter(line => line.startsWith('point '));
  assert.equal(points('b5.points').length, 64, 'B5 has 64 points');
  assert.equal(points('b5-changed.points').length, 64, 'the changed set keeps 64 points');
  assert.equal(points('b5.points').filter((line, index) => line !== points('b5-changed.points')[index]).length, 1,
    'b5-changed.points changes exactly one point');
  return bytes;
}

// ---------------------------------------------------------------------------
// Planned scenarios: exact argv relative to a fresh directory per repetition.
// A step is a command (`argv`) or a file derivation (`derive`).
// ---------------------------------------------------------------------------

function buildScenarios(cases) {
  const ten = cases.get('NGRAPH-CASE-10').input;
  const contract = ['--policy', ten.policy, '--optimizer', 'adam/0', '--seed', ten.seed];
  const train = (model, loss, data, steps, out, extra = [], flags = contract) =>
    ['ngraph', 'train', model, '--loss', loss, '--data', data, ...flags, '--steps', String(steps),
      '--out', out, ...extra];
  const b5 = (steps, out, extra = [], model = 'f1.ngraph') =>
    train(model, 'mse.ngraph', 'b5.points', steps, out, extra);
  const command = (id, caseId, variant, argv) => ({ id, case_id: caseId, variant, argv });
  const extract = (id, caseId, variant, source, inputs, out, extra = []) =>
    command(id, caseId, variant, ['ngraph', 'extract', source, '--inputs', inputs, '--out', out, ...extra]);
  const sources = cases.get('NGRAPH-CASE-05').input.sources;
  const childSteps = Number(/ for (\d+) steps$/.exec(FOLLOW_UPS[1])[1]);
  const seed = BigInt(ten.seed);
  const seedRecord = value => `seed 0x${value.toString(16).padStart(16, '0')}`;
  const s0 = cases.get('NGRAPH-CASE-14').expected.b5_s0;
  const scenarios = [];

  scenarios.push({ id: 'check', steps: [
    command('check-f1', 'NGRAPH-CASE-03', 'F1 printed by format::graph::print', ['ngraph', 'check', 'f1.ngraph']),
    extract('extract-i1', 'NGRAPH-CASE-03', 'I1 written by noble ngraph extract', 'i1.noble',
      sources.I1.inputs, 'i1.ngraph'),
    command('check-i1', 'NGRAPH-CASE-03', 'I1 written by noble ngraph extract', ['ngraph', 'check', 'i1.ngraph']),
  ] });

  scenarios.push({ id: 'check-refusals', steps: [
    { id: 'oversize', derive: 'oversize', from: 'f1.ngraph', to: 'f1-oversized.ngraph', bytes: OVERSIZED_BYTES },
    ...CHECK_REFUSALS.map(([variant, id, args]) => command(id, 'NGRAPH-CASE-04', variant, ['ngraph', 'check', ...args])),
  ] });

  const refusalInputs = {
    'test.emit': ['i64', []], if: ['i64', []], 'program output': ['i64', []], 'frontend refusal': ['i64', []],
    'f32 profile': [sources.I1.inputs, ['--profile', 'ngraph-f32/0']],
    expansion: [sources.I3.inputs, ['--budget-expansion', '2']],
  };
  scenarios.push({ id: 'extract', steps: [
    { id: 'out-directories', derive: 'mkdir', directories: EXTRACT_REFUSALS.map(([, id]) => `out/${id}`) },
    { id: 'existing-file', derive: 'write', to: 'existing.ngraph', text: 'existing bytes\n' },
    ...Object.entries(sources).flatMap(([name, row]) => [
      extract(`extract-${name}`, 'NGRAPH-CASE-05', name, `${name.toLowerCase()}.noble`, row.inputs,
        `${name.toLowerCase()}.ngraph`),
      command(`check-${name}`, 'NGRAPH-CASE-05', name, ['ngraph', 'check', `${name.toLowerCase()}.ngraph`]),
    ]),
    ...EXTRACT_REFUSALS.map(([variant, id, source]) => extract(id, 'NGRAPH-CASE-05', variant, source,
      refusalInputs[variant][0], `out/${id}/refused.ngraph`, refusalInputs[variant][1])),
    extract('existing-out', 'NGRAPH-CASE-05', 'existing out', 'i1.noble', sources.I1.inputs, 'existing.ngraph'),
  ] });

  scenarios.push({ id: 'eval-probes', steps: [
    extract('extract-i1', 'NGRAPH-CASE-07', 'I1', 'i1.noble', sources.I1.inputs, 'i1.ngraph'),
    ...PROBES.flatMap(([variant, , id, graph, vectors]) => [
      command(`eval-${id}`, 'NGRAPH-CASE-07', variant, ['ngraph', 'eval', graph, vectors]),
      command(`eval-${id}-reference`, 'NGRAPH-CASE-07', variant, ['ngraph', 'eval', graph, vectors, '--reference']),
    ]),
  ] });

  scenarios.push({ id: 'eval-outcomes', steps: [
    command('train-f1-s0', 'NGRAPH-CASE-08', '--fuel 1 on F1', b5(0, 'f1-s0.snapshot')),
    ...OUTCOMES.flatMap(([variant, , id, args]) => [
      command(id, 'NGRAPH-CASE-08', variant, ['ngraph', 'eval', ...args]),
      command(`${id}-reference`, 'NGRAPH-CASE-08', variant, ['ngraph', 'eval', ...args, '--reference']),
    ]),
  ] });

  const replay = (snapshot, data, extra) => ['ngraph', 'verify', 'f1.ngraph', snapshot, '--replay', '--loss',
    'mse.ngraph', '--data', data, ...extra];
  const evalF1 = (snapshot, extra = []) => ['ngraph', 'eval', 'f1.ngraph', 'f1.vectors', '--snapshot', snapshot,
    ...extra];
  scenarios.push({ id: 'b5', steps: [
    command('train-b5', 'NGRAPH-CASE-10', 'B5 training of F1', b5(ten.steps, 'b5.snapshot')),
    command('eval-before', 'NGRAPH-CASE-10', FOLLOW_UPS[3], evalF1('b5.snapshot')),
    command('train-second', 'NGRAPH-CASE-10', FOLLOW_UPS[0], b5(ten.steps, 'b5-second.snapshot')),
    command('train-from', 'NGRAPH-CASE-10', FOLLOW_UPS[1], b5(childSteps, 'b5-child.snapshot', ['--from', 'b5.snapshot'])),
    command('train-existing', 'NGRAPH-CASE-10', FOLLOW_UPS[2], b5(ten.steps, 'b5.snapshot')),
    command('eval-after', 'NGRAPH-CASE-10', FOLLOW_UPS[3], evalF1('b5.snapshot')),
    ...REPLAYS.slice(0, 3).map(([variant, , id, args]) => command(id, 'NGRAPH-CASE-13', variant, replay(...args))),
    command('train-s0', 'NGRAPH-CASE-13', REPLAYS[3][0], b5(0, 'b5-s0.snapshot')),
    command(REPLAYS[3][2], 'NGRAPH-CASE-13', REPLAYS[3][0], replay(...REPLAYS[3][3])),
    { id: 'forge-params', derive: 'forge', from: 'b5.snapshot', to: 'forged.snapshot', edit: 'flip-first-param-bit' },
    command('bind-forged', 'NGRAPH-CASE-13', REPLAYS[4][0], ['ngraph', 'verify', 'f1.ngraph', 'forged.snapshot']),
    command(REPLAYS[4][2], 'NGRAPH-CASE-13', REPLAYS[4][0], replay(...REPLAYS[4][3])),
    { id: 'tamper-param', derive: 'flip-param', from: 'b5.snapshot', to: 'tampered-param.snapshot' },
    { id: 'tamper-seed', derive: 'replace', from: 'b5.snapshot', to: 'tampered-seed.snapshot',
      search: `\n${seedRecord(seed)}\n`, replacement: `\n${seedRecord(seed + 1n)}\n` },
    command('train-f2-s0', 'NGRAPH-CASE-12', BINDINGS[2][0],
      train('f2.ngraph', 'f2-loss.ngraph', 'f2.points', 0, 'f2-s0.snapshot')),
    { id: 'forge-layout', derive: 'forge', from: 'b5.snapshot', to: 'swapped.snapshot', edit: 'swap-first-two-params' },
    command('train-f1-fact-s0', 'NGRAPH-CASE-12', BINDINGS[4][0], b5(0, 'f1-fact-s0.snapshot', [], 'f1-fact.ngraph')),
    { id: 'schema-line', derive: 'replace', from: 'b5.snapshot', to: 'schema.snapshot',
      search: 'noble-ngraph-snapshot/0\n', replacement: 'noble-ngraph-snapshot/1\n' },
    ...BINDINGS.flatMap(([variant, , id, snapshot]) => {
      const pinned = id === 'pinned-id' ? ['--snapshot-id', s0] : [];
      return [
        command(`${id}-eval`, 'NGRAPH-CASE-12', variant, evalF1(snapshot, pinned)),
        command(`${id}-verify`, 'NGRAPH-CASE-12', variant, ['ngraph', 'verify', 'f1.ngraph', snapshot, ...pinned]),
      ];
    }),
  ] });

  const withoutPolicy = contract.slice(2);
  scenarios.push({ id: 'train-refusals', steps: [
    { id: 'out-directories', derive: 'mkdir',
      directories: TRAIN_REFUSALS.filter(([, key]) => key !== 'with opt-in').map(([, , id]) => `out/${id}`) },
    command('train-other-s0', 'NGRAPH-CASE-11', TRAIN_REFUSALS[5][0],
      train('other.ngraph', 'loss.ngraph', 'data.points', 0, 'other-s0.snapshot')),
    ...TRAIN_REFUSALS.map(([variant, key, id, [model, loss, data, steps, extra]]) => command(id, 'NGRAPH-CASE-11',
      variant, train(model, loss, data, steps, key === 'with opt-in' ? 'opt-in.snapshot' : `out/${id}/refused.snapshot`,
        extra, key === 'missing --policy' ? withoutPolicy : contract))),
  ] });

  for (const scenario of scenarios) {
    const ids = scenario.steps.map(item => item.id);
    assert.equal(new Set(ids).size, ids.length, `${scenario.id}: duplicate step`);
    scenario.cases = [...new Set(scenario.steps.filter(item => item.case_id).map(item => item.case_id))];
    const staged = new Set();
    for (const item of scenario.steps) {
      for (const token of [...(item.argv ?? []), item.from ?? '']) if (Object.hasOwn(FIXTURES, token)) staged.add(token);
    }
    scenario.stage = [...staged].sort();
  }
  return scenarios;
}

// The CORE-01..18 workload as `verification/m4/gate.mjs` drives it through
// `noble run`, `noble session` and `noble compile`, plus the calculator usage
// refusal (the calculator code that the ngraph dispatch change touched).
function coreWorkload(document) {
  const cases = new Map(document.cases.map(row => [row.id, row]));
  assert.deepEqual([...cases.keys()], Array.from({ length: 18 }, (_, index) =>
    `CORE-${String(index + 1).padStart(2, '0')}`), 'CORE-01..18');
  const rows = [];
  const hex = text => Buffer.from(text).toString('hex');
  const emit = optimization => ['--opt', optimization, '--emit', 'emit'];
  const run = (id, label, source, optimization, check) => rows.push({ id: label, case_id: id,
    source_hex: hex(source), argv: ['run', 'source.noble', ...emit(optimization)], check });
  const session = (id, label, submissions, optimization, check) => rows.push({ id: label, case_id: id,
    stdin_hex: Buffer.concat(submissions.map(text => Buffer.from(`${Buffer.byteLength(text)}\n${text}`)))
      .toString('hex'), argv: ['session', '--framed', ...emit(optimization)], check });
  const compile = (id, source, type) => rows.push({ id, case_id: id, source_hex: hex(source),
    argv: ['compile', 'source.noble', '--input-type', type], check: { kind: 'compile' } });
  const stack = id => cases.get(id).expected.stack;
  for (const id of ['CORE-01', 'CORE-02', 'CORE-06', 'CORE-11', 'CORE-12', 'CORE-14']) {
    for (const optimization of ['off', 'on']) {
      run(id, `${id}-${optimization}`, cases.get(id).input.source, optimization,
        { kind: 'normal', report: 0, stack: stack(id) });
    }
  }
  for (const id of ['CORE-07', 'CORE-10']) {
    run(id, id, cases.get(id).input.source, 'off',
      { kind: 'refusal', report: 0, stage: 'check', outcome: cases.get(id).expected.outcome });
  }
  for (const optimization of ['off', 'on']) {
    session('CORE-04', `CORE-04-${optimization}`, cases.get('CORE-04').input.submissions, optimization,
      { kind: 'normal', report: 1, stack: stack('CORE-04') });
  }
  session('CORE-08', 'CORE-08', ['7', 'def keep [ dup ]', cases.get('CORE-08').input.source, 'keep'], 'off',
    { kind: 'refusal', report: 2, stage: 'check', outcome: 'type-reject',
      then: { report: 3, stack: [{ type: 'I64', value: '7' }, { type: 'I64', value: '7' }] } });
  cases.get('CORE-13').input.sources.forEach((source, index) => run('CORE-13', `CORE-13-${index + 1}`, source, 'off',
    { kind: 'refusal', report: 0, stage: 'resolve', outcome: 'unbound-word' }));
  const eighteen = cases.get('CORE-18').input;
  [...eighteen.sources.map(text => Buffer.from(text)), ...eighteen.byte_sources_hex.map(text => Buffer.from(text, 'hex'))]
    .forEach((source, index) => rows.push({ id: `CORE-18-${index + 1}`, case_id: 'CORE-18',
      source_hex: source.toString('hex'), argv: ['run', 'source.noble', ...emit('off')],
      check: { kind: 'refusal', report: 0, stage: 'parse', outcome: 'reject' } }));
  for (const optimization of ['off', 'on']) {
    run('CORE-17', `CORE-17-${optimization}`, cases.get('CORE-17').input.source, optimization,
      { kind: 'trap', report: 0, request_trace: cases.get('CORE-17').expected.request_trace });
  }
  const three = cases.get('CORE-03').input;
  compile('CORE-03', `${three.invocation_input} swap ${three.builder} run`, 'I64');
  compile('CORE-05', '[ 20 nil ] dip [ [ 1 + ] ] [ [ 2 * ] ] if swap cons [ ] [ drop run ] list.case', 'Bool');
  const nine = cases.get('CORE-09').input;
  compile('CORE-09', `${nine.invocation_input} swap ${nine.builder} dup [ run ] dip dup reflect pair`, 'I64');
  rows.push({ id: 'calculator-usage', case_id: 'entry-point:calculator', argv: ['calculator', '--bogus'],
    check: { kind: 'usage', stderr: 'usage: noble calculator' } });
  return {
    rows,
    runtime_inputs_not_replayed: {
      'CORE-03': 'compiled Wasm text compared; the runtime capture inputs go through the in-repo Node host, not the binary',
      'CORE-05': 'compiled Wasm text compared; the program selection is a runtime input of the in-repo Node host',
      'CORE-09': 'compiled Wasm text compared; the runtime capture input goes through the in-repo Node host',
    },
    not_cli_driven: {
      'CORE-15': 'structured cargo-test control eligibility::core_15_resource_matrix, bound through the rust-tests log',
      'CORE-16': 'structured cargo-test control mutations::core_16_candidate_mutation_matrix, bound through the rust-tests log',
    },
  };
}

// ---------------------------------------------------------------------------
// plan
// ---------------------------------------------------------------------------

function plan(directory, options) {
  const node = selectedNode();
  const target = outside(directory);
  assert.ok(!fs.existsSync(target), 'a new external output directory is required');
  const caseFile = resolveCaseFile(options.cases);
  const caseBytes = fs.readFileSync(caseFile);
  const document = JSON.parse(caseBytes);
  const cases = reviewCases(document);
  const fixtures = reviewFixtures(cases);
  const coreBytes = fs.readFileSync(path.join(root, CORE_CASES));
  const core = coreWorkload(JSON.parse(coreBytes));
  const scenarios = buildScenarios(cases);
  const caseName = display(caseFile);
  const inRepository = CASE_FILES.includes(caseName);
  const sources = Object.fromEntries(projectionFiles(inRepository ? caseName : null)
    .map(name => [name, fileSha(path.join(root, name))]));
  const workload = document.cases.map(({ id, kind, requirements, input, expected }) =>
    ({ id, kind, requirements, input, expected }));
  fs.mkdirSync(target);
  fs.mkdirSync(path.join(target, 'inputs'));
  for (const name of Object.keys(FIXTURES)) {
    fs.writeFileSync(path.join(target, 'inputs', name), fixtures[name], { flag: 'wx', mode: 0o444 });
}
  const manifest = {
    schema: PLAN_SCHEMA, status: 'planned-not-executed', acceptance_claim: false, profile: PROFILE,
    source_root: root, output_directory: target,
    runner: { path: RUNNER, sha256: sources[RUNNER] },
    selected_node: node,
    cases: { file: caseName, in_repository: inRepository, sha256: sha(caseBytes), revision: document.revision,
      workload_sha256: sha(JSON.stringify(workload)), workload },
    core_cases: { file: CORE_CASES, sha256: sha(coreBytes) },
    fixtures: Object.fromEntries(Object.entries(FIXTURES).map(([name, [digest, origin]]) =>
      [name, { file: `inputs/${name}`, sha256: digest, bytes: fixtures[name].length, origin }])),
    sources,
    repetitions: REPETITIONS,
    executable: 'executables/noble (frozen copy of --binary); argv are relative to a fresh scenario directory',
    scenarios,
    core_workload: { executables: ['executables/noble-base', 'executables/noble'], ...core },
    library_tests: LIBRARY_TESTS,
    test_files: TEST_FILES,
    soft_float_test: SOFT_FLOAT_TEST,
  };
  const planFile = path.join(target, 'plan.json');
  fs.writeFileSync(planFile, `${JSON.stringify(manifest, null, 2)}\n`, { flag: 'wx', mode: 0o444 });
  assert.equal(fileSha(caseFile), sha(caseBytes), 'the case file changed during planning');
  for (const [name, digest] of Object.entries(sources)) {
    assert.equal(fileSha(path.join(root, name)), digest, `${name} changed during planning`);
}
  return { schema: PLAN_SCHEMA, status: manifest.status, plan: planFile, plan_sha256: fileSha(planFile),
    case_file: caseName, fixtures: Object.keys(FIXTURES).length,
    scenario_commands: scenarios.reduce((count, scenario) => count + scenario.steps.filter(item => item.argv).length, 0),
    repetitions: REPETITIONS, core_commands: core.rows.length };
}

// ---------------------------------------------------------------------------
// run: execution machinery. `R` is the run context: the external directory,
// the plan, the receipt under construction and the watched output files.
// ---------------------------------------------------------------------------

const relativeTo = (target, file) => path.relative(target, file).split(path.sep).join('/');

function hashCached(R, file) {
  const stat = fs.statSync(file, { bigint: true });
  const key = `${file}\0${stat.dev}:${stat.ino}:${stat.size}:${stat.mtimeNs}:${stat.ctimeNs}`;
  if (!R.hashes.has(key)) R.hashes.set(key, fileSha(file));
  return R.hashes.get(key);
}

// Every path under `directory` (directories end in `/`) with its sha256.
function tree(R, directory) {
  const entries = {};
  const walk = (folder, prefix) => {
    for (const entry of fs.readdirSync(folder, { withFileTypes: true }).sort(byName)) {
      const file = path.join(folder, entry.name);
      const name = `${prefix}${entry.name}`;
      if (entry.isDirectory()) {
        entries[`${name}/`] = 'directory';
        walk(file, `${name}/`);
      } else entries[name] = entry.isFile() ? hashCached(R, file) : 'special';
    }
  };
  walk(directory, '');
  return entries;
}

function retain(R, name, bytes) {
  const file = path.join(R.target, name);
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, bytes, { flag: 'wx', mode: 0o444 });
  const digest = sha(bytes);
  R.watched.push({ file, sha256: digest });
  return { file: name, sha256: digest, bytes: bytes.length };
}

function freeze(R, name, original, pinned) {
  const real = fs.realpathSync(path.resolve(original));
  assert.ok(fs.statSync(real).isFile(), `${original}: not a file`);
  const bytes = fs.readFileSync(real);
  const file = path.join(R.target, 'executables', name);
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, bytes, { flag: 'wx', mode: 0o555 });
  const digest = sha(bytes);
  R.watched.push({ file, sha256: digest }, { file: real, sha256: digest });
  if (pinned !== undefined && pinned !== digest) {
    R.receipt.failures.push({ case_id: 'tools', error: `${name}: sha256 ${digest} differs from the pinned ${pinned}` });
  }
  return { name: `executables/${name}`, file, original: path.resolve(original), realpath: real, sha256: digest,
    bytes: bytes.length, pinned_sha256: pinned ?? null };
}

function execute(R, { label, executable, argv, cwd, stdin = null, meta = {} }) {
  R.serial += 1;
  const id = R.serial;
  const stem = `commands/${String(id).padStart(4, '0')}-${label.replace(/[^A-Za-z0-9_.-]/g, '-')}`;
  const result = spawnSync(executable.file, argv, {
    cwd, input: stdin ?? Buffer.alloc(0), encoding: null, timeout: TIMEOUT_MS, killSignal: 'SIGKILL',
    maxBuffer: 64 * 1024 * 1024, env: { PATH: '/nonexistent', LANG: 'C', LC_ALL: 'C', TMPDIR: R.tmp },
  });
  const stdout = result.stdout ?? Buffer.alloc(0);
  const stderr = result.stderr ?? Buffer.alloc(0);
  const input = stdin === null ? null : retain(R, `${stem}.stdin`, stdin);
  const out = retain(R, `${stem}.stdout`, stdout);
  const err = retain(R, `${stem}.stderr`, stderr);
  R.receipt.commands.push({ id, label, ...meta, executable: executable.name, executable_sha256: executable.sha256,
    argv, cwd: relativeTo(R.target, cwd), stdin: input?.file ?? null, stdin_sha256: input?.sha256 ?? null,
    stdout: out.file, stdout_sha256: out.sha256, stdout_bytes: out.bytes,
    stderr: err.file, stderr_sha256: err.sha256, stderr_bytes: err.bytes,
    exit: result.status, signal: result.signal, error: result.error ? String(result.error) : null,
    timeout_ms: TIMEOUT_MS });
  return { id, exit: result.status, signal: result.signal, error: result.error ? String(result.error) : null,
    stdout, stderr };
}

// Report lines of one `noble ngraph` run (JSON lines, newline-terminated).
function reports(stdout, command) {
  const text = stdout.toString('utf8');
  if (text.length === 0) return { lines: [], error: null };
  if (!text.endsWith('\n')) return { lines: [], error: 'stdout does not end with a newline' };
  const lines = [];
  for (const raw of text.slice(0, -1).split('\n')) {
    let line;
    try { line = JSON.parse(raw); } catch (error) { return { lines: [], error: `not a JSON line: ${error}` }; }
    if (line?.schema !== REPORT || line.command !== command || typeof line.stage !== 'string'
      || !['normal', 'trap', 'reject', 'unsupported', 'exhausted'].includes(line.outcome)) {
      return { lines: [], error: `not a ${REPORT} line of ${command}: ${raw}` };
    }
    lines.push(line);
  }
  return { lines, error: null };
}

function derive(step, cwd) {
  const file = name => path.join(cwd, name);
  const text = () => fs.readFileSync(file(step.from), 'utf8');
  if (step.derive === 'mkdir') {
    for (const directory of step.directories) fs.mkdirSync(file(directory), { recursive: true });
    return {};
  }
  if (step.derive === 'write') {
    fs.writeFileSync(file(step.to), step.text, { flag: 'wx' });
    return {};
  }
  if (step.derive === 'oversize') {
    fs.writeFileSync(file(step.to), fs.readFileSync(file(step.from)), { flag: 'wx' });
    fs.truncateSync(file(step.to), step.bytes);
    return { bytes: fs.statSync(file(step.to)).size };
  }
  if (step.derive === 'forge') {
    const original = text();
    assert.equal(forgeSnapshot(original, params => params), original,
      `the independent D6 encoding must reproduce ${step.from} byte for byte`);
    fs.writeFileSync(file(step.to), forgeSnapshot(original, EDITS[step.edit]), { flag: 'wx' });
    return { identity_self_check: 'the unedited re-encoding reproduced every published identity', edit: step.edit };
  }
  if (step.derive === 'flip-param') {
    const original = text();
    const match = /\nparam (\d+) 0x([0-9a-f]{8})\n/.exec(original);
    assert.ok(match, `${step.from}: a param record`);
    const flipped = ((Number.parseInt(match[2], 16) ^ 1) >>> 0).toString(16).padStart(8, '0');
    fs.writeFileSync(file(step.to), original.replace(match[0], `\nparam ${match[1]} 0x${flipped}\n`), { flag: 'wx' });
    return { slot: Number(match[1]), from: `0x${match[2]}`, to: `0x${flipped}` };
  }
  if (step.derive === 'replace') {
    const original = text();
    assert.equal(original.split(step.search).length, 2, `${step.from}: exactly one ${JSON.stringify(step.search)}`);
    fs.writeFileSync(file(step.to), original.replace(step.search, step.replacement), { flag: 'wx' });
    return {};
  }
  throw Error(`unreviewed derivation ${step.derive}`);
}

function runScenario(R, scenario, repetition, binary) {
  const cwd = path.join(R.target, 'runs', `r${repetition}`, scenario.id);
  fs.mkdirSync(cwd, { recursive: true });
  for (const name of scenario.stage) {
    const bytes = fs.readFileSync(path.join(R.target, 'inputs', name));
    assert.equal(sha(bytes), R.planned.fixtures[name].sha256, `${name}: planned input bytes`);
    fs.writeFileSync(path.join(cwd, name), bytes, { flag: 'wx', mode: 0o644 });
  }
  const results = {};
  for (const step of scenario.steps) {
    const before = tree(R, cwd);
    try {
      if (step.argv) {
        const ran = execute(R, { label: `r${repetition}-${scenario.id}-${step.id}`, executable: binary,
          argv: step.argv, cwd, meta: { scenario: scenario.id, repetition, step: step.id, case_id: step.case_id,
            variant: step.variant } });
        const parsed = reports(ran.stdout, step.argv[1]);
        const after = tree(R, cwd);
        const published = {};
        for (const [name, digest] of Object.entries(after)) {
          if (digest !== 'directory' && before[name] !== digest
            && fs.statSync(path.join(cwd, name)).size <= 4 * 1024 * 1024) published[name] = fs.readFileSync(path.join(cwd, name));
        }
        results[step.id] = { ...ran, step: step.id, command: step.argv[1], lines: parsed.lines,
          report_error: parsed.error, before, after, published };
      } else {
        results[step.id] = { step: step.id, derived: step.derive, detail: derive(step, cwd), before, after: tree(R, cwd) };
      }
    } catch (error) {
      results[step.id] = { step: step.id, error: errorText(error), before, after: tree(R, cwd), lines: [],
        published: {}, stdout: Buffer.alloc(0), stderr: Buffer.alloc(0), exit: null };
    }
  }
  return { cwd, results, tree: tree(R, cwd) };
}

// ---------------------------------------------------------------------------
// Case evaluation: every leaf of `expected` is observed (one or more rows) or
// accounted for exactly once (library, review, assumption, classification).
// ---------------------------------------------------------------------------

const lookup = (object, keys) => keys.reduce((value, key) => value?.[key], object);
const leafPaths = (value, prefix = []) => value && typeof value === 'object' && !Array.isArray(value)
  && Object.keys(value).length ? Object.entries(value).flatMap(([key, child]) => leafPaths(child, [...prefix, key]))
  : [prefix];
const ABSENT = '<absent>';
const present = value => value === undefined ? ABSENT : value;
const vectorLines = result => (result.lines ?? []).filter(line => line.vector !== undefined && line.vector !== null);
const src = result => ({ step: result.step, command: result.id ?? null });

// The line that carries a command's outcome: the single line of check,
// extract, train and verify; for eval, the refusal line, or the vector lines
// when they all share one outcome and code.
function primary(result) {
  const lines = result.lines ?? [];
  if (result.command !== 'eval') return lines.length === 1 ? lines[0] : undefined;
  const vectors = vectorLines(result);
  const refusals = lines.filter(line => (line.vector === undefined || line.vector === null) && line.outcome !== 'normal');
  if (refusals.length === 1 && vectors.length === 0) return refusals[0];
  if (refusals.length === 0 && vectors.length > 0
    && vectors.every(line => line.outcome === vectors[0].outcome && line.code === vectors[0].code)) return vectors[0];
  return undefined;
}

function caseContext(row) {
  const context = {
    row, fields: [], accounted: [],
    expected: keys => lookup(row.expected, keys),
    observe(keys, observed, extra = {}) {
      const expected = lookup(row.expected, keys);
      context.fields.push({ path: keys, expected: present(expected), observed: present(observed),
        result: expected !== undefined && same(expected, observed) ? 'passed' : 'failed', ...extra });
    },
    account(keys, kind, detail) {
      context.accounted.push({ path: keys, kind, expected: present(lookup(row.expected, keys)), ...detail });
    },
  };
  return context;
}

// Observes the standard fields of one variant from one command.
function observeFields(c, keys, result) {
  const want = c.expected(keys);
  const line = primary(result);
  const extra = { ...src(result), stage: line?.stage ?? null };
  for (const field of Object.keys(want ?? {})) {
    if (field === 'code') c.observe([...keys, field], line === undefined ? ABSENT : line.code, extra);
    else if (field === 'exit') c.observe([...keys, field], result.exit, extra);
    else if (field === 'outcome') c.observe([...keys, field], line?.outcome, extra);
    else if (field === 'replayed') c.observe([...keys, field], line?.replayed, extra);
    else if (field === 'vector_lines') c.observe([...keys, field], vectorLines(result).length, extra);
    else if (field === 'usage_on_stderr') {
      c.observe([...keys, field], result.stdout.length === 0 && result.stderr.toString('utf8').includes('usage:'), extra);
    }
  }
}

function finishAccounting(c) {
  const key = keys => JSON.stringify(keys);
  const has = (rows, keys) => rows.some(item => key(item.path) === key(keys));
  for (const name of COMPLETION_KEYS) {
    if (!Object.hasOwn(c.row.expected, name) || has(c.fields, [name]) || has(c.accounted, [name])) continue;
    if (name === 'stage') {
      c.account([name], 'classification', { reason: 'case-level D8 stage; the stage of every command report is recorded with its field rows' });
    } else if (name === 'outcome') {
      c.account([name], 'classification', { reason: 'case-level outcome; each variant outcome, code and exit is compared individually' });
    } else c.account([name], 'not_observable_in_cli', { covered_by: [], reason: NO_GUEST });
  }
  const problems = [];
  const leaves = leafPaths(c.row.expected).map(key);
  for (const leaf of leaves) {
    const observed = c.fields.filter(item => key(item.path) === leaf).length;
    const accounted = c.accounted.filter(item => key(item.path) === leaf).length;
    if (observed === 0 && accounted === 0) problems.push(`unaccounted expected field ${leaf}`);
    if (observed > 0 && accounted > 0) problems.push(`expected field ${leaf} both observed and accounted`);
    if (accounted > 1) problems.push(`expected field ${leaf} accounted ${accounted} times`);
  }
  for (const item of [...c.fields, ...c.accounted]) {
    if (!leaves.includes(key(item.path))) problems.push(`row for ${key(item.path)}, which is not an expected field`);
  }
  return [...new Set(problems)];
}

const record = (bytes, name) => bytes?.toString('utf8').split('\n').find(line => line.startsWith(`${name} `))
  ?.slice(name.length + 1);

function evaluateCase(id, c, at, scenario, extra) {
  const library = (keys, tests, reason) => c.account(keys, 'library', { covered_by: tests, reason });
  const skip = new Set(['stage', 'guest_requests', 'protected_operations']);
  const remaining = handled => leafPaths(c.row.expected).filter(keys => !handled.includes(keys[0])
    && !(keys.length === 1 && skip.has(keys[0])));
  if (id === 'NGRAPH-CASE-01') {
    const core = extra.core;
    c.observe(['ordinary_reports_byte_identical'], core.identical, { evidence: core.identity_summary });
    c.observe(['outcome'], core.identical && core.ordinary_diff.empty ? 'ordinary-identities-unchanged'
      : 'ordinary-identities-changed', { evidence: { ordinary_diff_empty: core.ordinary_diff.empty } });
    c.observe(['kernel_f32_type_or_word'], !(core.ordinary_diff.empty && core.kernel_f32_tokens.length === 0),
      { evidence: { kernel_diff_empty: core.ordinary_diff.empty, kernel_f32_tokens: core.kernel_f32_tokens } });
    c.observe(['frontend_syntax_added'], !core.frontend.unchanged, { evidence: core.frontend });
    c.observe(['ngraph_reachable_only_through'], core.reachability.observed, { evidence: core.reachability });
    c.observe(['k_num_01_changed'], !core.k_num_01.unchanged, { evidence: core.k_num_01 });
  } else if (id === 'NGRAPH-CASE-02') {
    for (const keys of remaining([])) {
      c.account(keys, 'not_run', { reason: 'forbidden_capability_uses needs Octet/native-assurance capability facts for noble-ngraph; outside this runner' });
    }
  } else if (id === 'NGRAPH-CASE-03') {
    const [f1, extract, i1] = ['check-f1', 'extract-i1', 'check-i1'].map(step => at('check', step));
    for (const result of [f1, extract, i1]) {
      const line = primary(result);
      c.observe(['exit'], result.exit, src(result));
      c.observe(['outcome'], line?.outcome, src(result));
      c.observe(['schema'], line?.schema, src(result));
    }
    const [f1Line, extractLine, i1Line] = [f1, extract, i1].map(primary);
    c.observe(['F1', 'graph'], f1Line?.graph, src(f1));
    c.observe(['F1', 'params'], f1Line?.params, src(f1));
    const sameGraph = hex64(i1Line?.graph) && i1Line.graph === extractLine?.graph;
    c.observe(['I1', 'graph'], sameGraph ? c.expected(['I1', 'graph']) : 'differs from the extract report',
      { ...src(i1), evidence: { check: i1Line?.graph ?? null, extract: extractLine?.graph ?? null } });
    for (const field of ['inputs', 'outputs', 'live_nodes', 'layers']) c.observe(['I1', field], i1Line?.[field], src(i1));
  } else if (id === 'NGRAPH-CASE-04') {
    for (const [variant, step] of CHECK_REFUSALS) observeFields(c, [variant], at('check-refusals', step));
    c.account(['evaluated_nodes'], 'not_observable_in_cli', { covered_by: [NO_NODE],
      reason: 'check evaluates no node and its report carries no node counter' });
  } else if (id === 'NGRAPH-CASE-05') {
    for (const name of ['I1', 'I2', 'I3', 'I4']) {
      const extract = at('extract', `extract-${name}`), check = at('extract', `check-${name}`);
      const [extractLine, checkLine] = [extract, check].map(primary);
      c.observe(['I1..I4', 'outcome'], extractLine?.outcome, src(extract));
      c.observe(['I1..I4', 'exit'], extract.exit, src(extract));
      c.observe(['I1..I4', 'check_of_written_file_reports_same_graph'], check.exit === 0
        && checkLine?.outcome === 'normal' && hex64(checkLine?.graph) && checkLine.graph === extractLine?.graph,
      { ...src(check), evidence: { source: name, extract_graph: extractLine?.graph ?? null,
        check_graph: checkLine?.graph ?? null, published: Object.keys(extract.published).sort() } });
    }
    for (const [variant, step] of EXTRACT_REFUSALS) {
      const result = at('extract', step);
      observeFields(c, [variant], result);
      c.observe(['refusals_write_no_file'], same(result.before, result.after), src(result));
    }
    const existing = at('extract', 'existing-out');
    c.observe(['existing out', 'exit'], existing.exit, src(existing));
    c.observe(['existing out', 'existing_bytes_unchanged'], existing.before['existing.ngraph'] === sha('existing bytes\n')
      && existing.after['existing.ngraph'] === existing.before['existing.ngraph'], src(existing));
    c.observe(['refusals_write_no_file'], same(existing.before, existing.after), src(existing));
    c.account(['run of a Program input'], 'not_observable_in_cli', { covered_by: ['every_source_control_is_refused_with_its_d6_code',
      'b2_vector_observations_and_b3_codes_match_the_record'], reason: c.expected(['run of a Program input']) });
  } else if (id === 'NGRAPH-CASE-07') {
    let matches = true;
    for (const [, key, step] of PROBES) {
      const pair = [at('eval-probes', `eval-${step}`), at('eval-probes', `eval-${step}-reference`)];
      for (const result of pair) {
        const outputs = vectorLines(result).map(line => line.outputs);
        const observed = outputs.length === 1 ? outputs[0] : outputs;
        c.observe([key], observed, src(result));
        c.observe(['exit'], result.exit, src(result));
        matches &&= same(observed, c.expected([key])) && result.exit === 0;
      }
      c.observe(['default_and_reference_stdout_byte_identical'], pair[0].stdout.equals(pair[1].stdout),
        { step: pair[0].step, command: pair[0].id ?? null, evidence: { reference_command: pair[1].id ?? null } });
    }
    c.observe(['outcome'], matches ? 'each-vector-matches' : 'some-vector-differs');
  } else if (id === 'NGRAPH-CASE-08') {
    for (const [, key, step] of OUTCOMES) {
      for (const result of [at('eval-outcomes', step), at('eval-outcomes', `${step}-reference`)]) {
        if (key === 'mixed file') {
          c.observe([key, 'lines'], vectorLines(result).map(line => line.outcome), src(result));
          c.observe([key, 'exit'], result.exit, src(result));
        } else observeFields(c, [key], result);
      }
    }
    c.account(['refused_vectors_evaluated_nodes'], 'not_observable_in_cli', { covered_by: [NO_NODE],
      reason: 'eval reports carry no node counter; refused vectors print no outputs and input refusals print no vector line (recorded per variant)' });
  } else if (id === 'NGRAPH-CASE-10') {
    const train = at('b5', 'train-b5'), line = primary(train), published = train.published['b5.snapshot'];
    for (const field of ['outcome', 'snapshot', 'graph']) c.observe([field], line?.[field], src(train));
    c.observe(['exit'], train.exit, src(train));
    c.observe(['data_digest'], record(published, 'data'), src(train));
    const second = at('b5', 'train-second');
    c.observe(['second_run_file_byte_identical'], Boolean(published && second.published['b5-second.snapshot']?.equals(published)),
      { ...src(second), evidence: { exit: second.exit, stdout_identical_to_first: second.stdout.equals(train.stdout) } });
    const from = at('b5', 'train-from'), child = primary(from);
    const parent = published ? sha(published) : null;
    const evidence = { child_exit: from.exit, child_snapshot: child?.snapshot ?? null, child_parent: child?.parent ?? null,
      child_steps: child?.steps ?? null, child_published: Boolean(from.published['b5-child.snapshot']),
      parent_sha256_before: from.before['b5.snapshot'] ?? null, parent_sha256_after: from.after['b5.snapshot'] ?? null,
      parent_sha256_end_of_chain: scenario('b5').tree['b5.snapshot'] ?? null };
    c.observe(['parent_file_bytes_unchanged'], parent !== null && evidence.child_exit === 0
      && evidence.child_parent === line?.snapshot && evidence.child_published
      && [evidence.parent_sha256_before, evidence.parent_sha256_after, evidence.parent_sha256_end_of_chain]
        .every(digest => digest === parent), { ...src(from), evidence });
    const existing = at('b5', 'train-existing');
    c.observe(['existing_path', 'exit'], existing.exit, src(existing));
    c.observe(['existing_path', 'bytes_unchanged'], parent !== null && existing.before['b5.snapshot'] === parent
      && same(existing.before, existing.after), src(existing));
    const before = at('b5', 'eval-before'), after = at('b5', 'eval-after');
    c.observe(['first_snapshot_eval_unchanged'], before.exit === 0 && before.lines[0]?.snapshot === line?.snapshot
      && vectorLines(before).length > 0 && before.stdout.equals(after.stdout),
    { ...src(after), evidence: { before_command: before.id ?? null, before_exit: before.exit, after_exit: after.exit } });
    c.observe(['graph_file_bytes_unchanged'], scenario('b5').tree['f1.ngraph'] === FIXTURES['f1.ngraph'][0]);
  } else if (id === 'NGRAPH-CASE-11') {
    for (const [, key, step] of TRAIN_REFUSALS) {
      const result = at('train-refusals', step);
      if (key === 'with opt-in') {
        c.observe([key, 'exit'], result.exit, src(result));
        c.observe([key, 'opt_in_recorded_in_snapshot'], (result.published['opt-in.snapshot']?.toString('utf8') ?? '')
          .split('\n').includes(`opt-in ${STEP_OPT_IN.replace(':', ' ').replace('=', ' ')}`), src(result));
      } else {
        observeFields(c, [key], result);
        c.observe(['refusals_create_no_output_or_partial_file'], same(result.before, result.after), src(result));
      }
    }
  } else if (id === 'NGRAPH-CASE-12') {
    for (const [, key, step] of BINDINGS) {
      for (const command of ['eval', 'verify']) {
        const result = at('b5', `${step}-${command}`), line = primary(result);
        const evidence = { stage: line?.stage ?? null, lines: result.lines.length, vector_lines: vectorLines(result).length };
        c.observe([key], line === undefined ? ABSENT : line.code, { ...src(result), evidence });
        c.observe(['exit'], result.exit, src(result));
      }
    }
    c.account(['evaluated_nodes'], 'not_observable_in_cli', { covered_by: [NO_NODE, 'wrong_graph_layout_and_stale_snapshots_are_refused'],
      reason: 'eval and verify reports carry no node counter; each refused eval printed one refusal line and no vector line (recorded per variant)' });
  } else if (id === 'NGRAPH-CASE-13') {
    for (const [, key, step] of REPLAYS) observeFields(c, [key], at('b5', step));
    c.account(['platform_scope'], 'assumption', { reason: 'every replay ran on one pinned x86_64 Linux toolchain; no other platform was run' });
  } else if (id === 'NGRAPH-CASE-14') {
    const train = at('b5', 'train-b5'), s0 = at('b5', 'train-s0');
    c.observe(['b5_s0'], primary(train)?.parent, src(train));
    c.observe(['b5_s0'], primary(s0)?.snapshot, src(s0));
    c.observe(['b5_final'], primary(train)?.snapshot, src(train));
    const soft = ['soft_float_equals_std_bitwise', 'nan_results_compared_as_class'];
    const softTests = ['special_values_match_std', 'powers_of_two_are_exact', 'seeded_bit_patterns_match_std'];
    for (const keys of remaining(['b5_s0', 'b5_final'])) {
      library(keys, soft.includes(keys[0]) ? [...softTests, `${SOFT_FLOAT_TEST} (--soft-float-log)`]
        : LIBRARY_TESTS[id].filter(name => !softTests.includes(name)));
    }
  } else if (id === 'NGRAPH-CASE-06' || id === 'NGRAPH-CASE-09') {
    for (const keys of remaining([])) library(keys, LIBRARY_TESTS[id]);
  } else if (id === 'NGRAPH-CASE-15') {
    c.observe(['stdout_byte_identical_across_runs'], extra.determinism.stdout_identical,
      { evidence: { compared_commands: extra.determinism.compared_commands, repetitions: REPETITIONS } });
    c.observe(['help_lists'], extra.help.lists, { command: extra.help.command });
    for (const keys of remaining(['stdout_byte_identical_across_runs', 'help_lists'])) {
      c.account(keys, 'review', { reason: REVIEW });
    }
  }
  return finishAccounting(c);
}

// ---------------------------------------------------------------------------
// Library tests (gate rust-tests log), soft-float log and source binding.
// ---------------------------------------------------------------------------

// `test NAME ... RESULT` lines; the gate log prefixes them with `noble-rust-tests> `.
function testLines(text) {
  const counts = new Map();
  for (const raw of text.split('\n')) {
    const match = /^(?:noble-rust-tests> )?test (\S+) \.\.\. (.*)$/.exec(raw.replace(/\r$/, ''));
    if (!match) continue;
    const entry = counts.get(match[1]) ?? { ok: 0, other: [] };
    if (match[2] === 'ok') entry.ok += 1;
    else entry.other.push(match[2]);
    counts.set(match[1], entry);
  }
  return counts;
}

function readWatched(R, file) {
  const bytes = fs.readFileSync(file);
  R.watched.push({ file: path.resolve(file), sha256: sha(bytes) });
  return bytes;
}

function bindLibraryTests(R, gateFile, tested) {
  const gateBytes = readWatched(R, gateFile);
  const gate = JSON.parse(gateBytes);
  const binding = { gate_receipt: { file: display(path.resolve(gateFile)), sha256: sha(gateBytes) }, rust_tests: null,
    tests: [], result: 'failed' };
  const sections = [];
  const consider = (label, commit, checks) => {
    if (typeof commit === 'string' && Array.isArray(checks)) sections.push({ label, commit, checks });
  };
  consider('tested_commit', gate.tested_commit, gate.flake_checks);
  for (const [key, value] of Object.entries(gate)) {
    if (value && typeof value === 'object' && !Array.isArray(value)) {
      consider(key, value.commit ?? value.tested_commit, value.flake_checks ?? value.checks);
    }
  }
  const rows = sections.filter(section => section.commit === tested)
    .flatMap(section => section.checks.filter(row => row?.check === 'rust-tests').map(row => ({ section, row })));
  const failure = error => R.receipt.failures.push({ case_id: 'library-tests', error });
  if (rows.length !== 1) {
    failure(`the gate receipt has ${rows.length} rust-tests rows for ${tested}; exactly one is required`);
    return binding;
  }
  const { section, row } = rows[0];
  binding.rust_tests = { section: section.label, commit: section.commit, result: row.result ?? null,
    exit: row.exit ?? null, log: row.log ?? null, log_sha256: row.log_sha256 ?? null, observed_log_sha256: null };
  if (row.result !== 'pass' || (row.exit !== undefined && row.exit !== 0)) failure(`rust-tests row is ${row.result}/${row.exit}`);
  let counts = new Map();
  if (typeof row.log === 'string' && fs.existsSync(row.log)) {
    const log = readWatched(R, row.log);
    binding.rust_tests.observed_log_sha256 = sha(log);
    if (sha(log) !== row.log_sha256) failure(`rust-tests log sha256 ${sha(log)} differs from the gate receipt ${row.log_sha256}`);
    else counts = testLines(log.toString('utf8'));
  } else failure(`rust-tests log ${row.log} is not readable`);
  const definitions = new Map();
  const defined = (file, name) => {
    if (!definitions.has(file)) {
      const shown = git(['show', `${tested}:${file}`], { allow: [0, 128] });
      definitions.set(file, shown.status === 0 ? shown.stdout : '');
    }
    const base = name.split('::').at(-1);
    return (definitions.get(file).match(new RegExp(`\\bfn ${base}\\s*\\(`, 'g')) ?? []).length;
  };
  const names = [...new Set(Object.values(LIBRARY_TESTS).flat())].sort();
  for (const name of names) {
    const entry = counts.get(name) ?? { ok: 0, other: [] };
    const cases = Object.keys(LIBRARY_TESTS).filter(id => LIBRARY_TESTS[id].includes(name));
    const definitionsAtTested = defined(TEST_FILES[name], name);
    const passed = entry.ok === 1 && entry.other.length === 0 && definitionsAtTested === 1;
    binding.tests.push({ name, file: TEST_FILES[name], cases, ok_lines: entry.ok, other_lines: entry.other,
      definitions_at_tested_commit: definitionsAtTested, result: passed ? 'passed' : 'failed' });
    if (!passed) {
      R.receipt.failures.push({ case_id: 'library-tests', test: name, cases,
        error: `expected exactly one \`test ${name} ... ok\` line and one definition in ${TEST_FILES[name]} at ${tested}; `
          + `found ${entry.ok} ok line(s), other results ${JSON.stringify(entry.other)}, ${definitionsAtTested} definition(s)` });
    }
  }
  binding.result = R.receipt.failures.some(item => item.case_id === 'library-tests') ? 'failed' : 'passed';
  return binding;
}

function bindSoftFloat(R, file) {
  const bytes = readWatched(R, file);
  const entry = testLines(bytes.toString('utf8')).get(SOFT_FLOAT_TEST) ?? { ok: 0, other: [] };
  const passed = entry.ok === 1 && entry.other.length === 0;
  if (!passed) {
    R.receipt.failures.push({ case_id: 'NGRAPH-CASE-14', test: SOFT_FLOAT_TEST,
      error: `--soft-float-log has ${entry.ok} \`test ${SOFT_FLOAT_TEST} ... ok\` line(s), other ${JSON.stringify(entry.other)}` });
  }
  return { log: path.resolve(file), sha256: sha(bytes), bytes: bytes.length, test: SOFT_FLOAT_TEST,
    ok_lines: entry.ok, other_lines: entry.other, result: passed ? 'passed' : 'failed',
    scope: 'bound by sha256; the log does not name its commit (assumption: release run at --tested-commit)' };
}

function bindSource(R, tested) {
  const caseName = R.planned.cases.in_repository ? R.planned.cases.file : null;
  const files = projectionFiles(caseName);
  const current = Object.fromEntries(files.map(name => [name, fileSha(path.join(root, name))]));
  if (!same(Object.keys(current), Object.keys(R.planned.sources))) {
    R.receipt.integrity_failures.push({ file: 'source projection', error: 'the projected file set changed after planning' });
  }
  for (const [name, digest] of Object.entries(current)) {
    if (R.planned.sources[name] !== undefined && R.planned.sources[name] !== digest) {
      R.receipt.integrity_failures.push({ file: name, error: 'changed after planning' });
    }
  }
  let projection, frozen = null;
  try {
    projection = gitSourceProjection(root, files, true);
  } catch (error) {
    frozen = errorText(error).split('\n')[0];
    projection = gitSourceProjection(root, files, false);
    R.receipt.integrity_failures.push({ file: 'source projection', error: `not a frozen staged source: ${frozen}` });
  }
  const roots = ['Cargo.toml', 'Cargo.lock', 'crates/noble-cli/src/main.rs', 'crates/noble-cli/src/calculator.rs',
    'crates/noble-cli/src/ngraph.rs', 'crates/noble-cli/src/ngraph', 'crates/noble-ngraph',
    'crates/noble-ngraph-experiment'];
  const committed = Object.fromEntries(git(['ls-tree', '-r', tested, '--', ...roots]).stdout.split('\n')
    .filter(Boolean).map(line => { const [meta, name] = line.split('\t'); return [name, meta.split(' ')[2]]; }));
  const builtFiles = files.filter(built);
  const worktree = Object.fromEntries(git(['hash-object', '--no-filters', '--', ...builtFiles]).stdout.trim()
    .split('\n').map((oid, index) => [builtFiles[index], oid]));
  const differing = [...new Set([...Object.keys(committed), ...builtFiles])].sort()
    .filter(name => committed[name] !== worktree[name]);
  if (differing.length) {
    R.receipt.failures.push({ case_id: 'source', error: `projected crate sources differ from ${tested}: ${differing.join(', ')}` });
  }
  return { projection, current, section: { head: git(['rev-parse', 'HEAD']).stdout.trim(), files: current,
    built_sources: builtFiles.length, built_sources_equal_tested_commit: differing.length === 0,
    built_sources_differing: differing, frozen: frozen === null, frozen_error: frozen,
    projection: { schema: projection.schema, sha256: projection.sha256, unstaged: projection.unstaged,
      untracked: projection.untracked, status: projection.status } } };
}

// ---------------------------------------------------------------------------
// NGRAPH-CASE-01: CORE workload identity, dispatch and kernel review inputs.
// ---------------------------------------------------------------------------

function emitted(R, cwd) {
  const directory = path.join(cwd, 'emit');
  if (!fs.existsSync(directory)) return {};
  const marker = Buffer.from(cwd, 'utf8').toString('latin1');
  return Object.fromEntries(Object.entries(tree(R, directory)).map(([name, digest]) => [name,
    digest === 'directory' ? digest : sha(Buffer.from(fs.readFileSync(path.join(directory, name)).toString('latin1')
      .split(marker).join('<CWD>'), 'latin1'))]));
}

function coreCheck(check, ran) {
  const text = ran.stdout.toString('utf8');
  if (check.kind === 'compile') return ran.exit === 0 && text.trimStart().startsWith('(module');
  if (check.kind === 'usage') return ran.exit === 2 && text.length === 0 && ran.stderr.toString('utf8').includes(check.stderr);
  let lines;
  try { lines = text.split('\n').filter(Boolean).map(line => JSON.parse(line)); } catch { return false; }
  const stack = items => (items ?? []).map(item => ({ type: item.type, value: item.value }));
  const report = lines[check.report];
  if (check.kind === 'normal') return report?.stage === 'wasm' && report.outcome === 'normal' && same(stack(report.stack), check.stack);
  if (check.kind === 'trap') return report?.stage === 'wasm' && report.outcome === 'trap' && same(report.request_trace, check.request_trace);
  if (check.kind !== 'refusal' || report?.stage !== check.stage || report.outcome !== check.outcome) return false;
  const next = check.then ? lines[check.then.report] : null;
  return !check.then || (next?.stage === 'wasm' && next.outcome === 'normal' && same(stack(next.stack), check.then.stack));
}

const diffStat = (base, tested, paths) => git(['diff', '--no-ext-diff', '--no-color', '--stat', base, tested, '--', ...paths])
  .stdout.trim();
const diffLines = (base, tested, file) => {
  const lines = git(['diff', '--no-ext-diff', '--no-color', '-U0', base, tested, '--', file]).stdout.split('\n');
  return { added: lines.filter(line => line.startsWith('+') && !line.startsWith('+++')).map(line => line.slice(1)),
    removed: lines.filter(line => line.startsWith('-') && !line.startsWith('---')).map(line => line.slice(1)) };
};

function kNum(commit) {
  const shown = git(['show', `${commit}:.cairn/specs/language/spec.md`], { allow: [0, 128] });
  const match = /^### Requirement: K-NUM-01\n[\s\S]*?(?=^### |^## |(?![\s\S]))/m.exec(shown.status === 0 ? shown.stdout : '');
  return match ? match[0] : null;
}

function coreIdentity(R, base, tested, baseCommit, testedCommit) {
  const rows = [];
  for (const row of R.planned.core_workload.rows) {
    const sides = {};
    for (const [side, binary] of [['base', base], ['tested', tested]]) {
      const cwd = path.join(R.target, 'core', side, row.id);
      fs.mkdirSync(cwd, { recursive: true });
      if (row.source_hex !== undefined) fs.writeFileSync(path.join(cwd, 'source.noble'), Buffer.from(row.source_hex, 'hex'), { flag: 'wx' });
      const ran = execute(R, { label: `core-${side}-${row.id}`, executable: binary, argv: row.argv, cwd,
        stdin: row.stdin_hex === undefined ? null : Buffer.from(row.stdin_hex, 'hex'),
        meta: { scenario: 'core-identity', side, step: row.id, case_id: row.case_id } });
      sides[side] = { ran, emitted: emitted(R, cwd) };
    }
    const [b, t] = [sides.base, sides.tested];
    const identical = { stdout: b.ran.stdout.equals(t.ran.stdout), stderr: b.ran.stderr.equals(t.ran.stderr),
      exit: b.ran.exit === t.ran.exit, emitted: same(b.emitted, t.emitted) };
    const checked = coreCheck(row.check, t.ran);
    rows.push({ id: row.id, case_id: row.case_id, argv: row.argv, base_command: b.ran.id, tested_command: t.ran.id,
      exit: t.ran.exit, stdout_sha256: sha(t.ran.stdout), emitted_files: Object.keys(t.emitted).length, identical,
      tested_expectation: checked ? 'passed' : 'failed' });
    if (!Object.values(identical).every(Boolean)) {
      R.receipt.failures.push({ case_id: 'NGRAPH-CASE-01', step: row.id, error: `base and tested differ: ${JSON.stringify(identical)}` });
    }
    if (!checked) {
      R.receipt.failures.push({ case_id: 'NGRAPH-CASE-01', step: row.id,
        error: `the tested ${row.case_id} result does not match the CORE expectation (${row.check.kind})` });
    }
  }
  const ordinaryPaths = ['crates/noble-kernel', 'crates/noble-contracts', 'crates/noble-wasm'];
  const frontendPaths = ['crates/noble-contracts', 'crates/noble-cli/src/core'];
  const ordinaryStat = diffStat(baseCommit, testedCommit, ordinaryPaths);
  const frontendStat = diffStat(baseCommit, testedCommit, frontendPaths);
  const kernelTokens = git(['grep', '-n', '-i', '-w', '-e', 'f32', testedCommit, '--', 'crates/noble-kernel'], { allow: [0, 1] })
    .stdout.split('\n').filter(Boolean);
  const mainSource = git(['show', `${testedCommit}:crates/noble-cli/src/main.rs`]).stdout;
  const dispatches = (mainSource.match(/\bngraph::run\(/g) ?? []).length;
  const references = git(['grep', '-l', '-e', 'noble_ngraph', testedCommit, '--', 'crates/noble-cli/src'], { allow: [0, 1] })
    .stdout.split('\n').filter(Boolean).map(line => line.slice(testedCommit.length + 1));
  const strayReferences = references.filter(file => file !== 'crates/noble-cli/src/ngraph.rs'
    && !file.startsWith('crates/noble-cli/src/ngraph/'));
  const manifests = git(['grep', '-l', '-e', 'noble-ngraph', testedCommit, '--', 'Cargo.toml', ':(glob)crates/*/Cargo.toml'],
    { allow: [0, 1] }).stdout.split('\n').filter(Boolean).map(line => line.slice(testedCommit.length + 1));
  const allowedManifests = ['Cargo.toml', 'crates/noble-cli/Cargo.toml', 'crates/noble-ngraph/Cargo.toml',
    'crates/noble-ngraph-experiment/Cargo.toml'];
  const strayManifests = manifests.filter(file => !allowedManifests.includes(file));
  const dispatchOnly = dispatches === 1 && mainSource.includes('argument == "ngraph"');
  const observed = [...(dispatchOnly ? ['noble ngraph'] : [`noble-cli dispatch calls ngraph::run ${dispatches} time(s)`]),
    'noble-ngraph library', ...strayReferences.map(file => `noble-cli ${file}`), ...strayManifests.map(file => `dependent ${file}`)];
  const [baseBlock, testedBlock] = [kNum(baseCommit), kNum(testedCommit)];
  const identical = rows.every(row => Object.values(row.identical).every(Boolean));
  const checked = rows.every(row => row.tested_expectation === 'passed');
  return {
    base_commit: baseCommit, tested_commit: testedCommit,
    workload: { file: R.planned.core_cases.file, sha256: R.planned.core_cases.sha256,
      driver: 'verification/m4/gate.mjs command shapes (run/session --emit, compile --input-type)',
      runtime_inputs_not_replayed: R.planned.core_workload.runtime_inputs_not_replayed,
      not_cli_driven: R.planned.core_workload.not_cli_driven },
    rows, identical, tested_expectations: checked,
    identity_summary: { rows: rows.length, identical: rows.filter(row => Object.values(row.identical).every(Boolean)).length,
      compared: ['stdout', 'stderr', 'exit', 'emitted artifacts (emit directory path normalized)'] },
    ordinary_diff: { paths: ordinaryPaths, stat: ordinaryStat, empty: ordinaryStat === '' },
    frontend: { paths: frontendPaths, stat: frontendStat, unchanged: frontendStat === '' },
    kernel_f32_tokens: kernelTokens,
    cli_diff_stat: diffStat(baseCommit, testedCommit, ['crates/noble-cli/src']),
    main_rs: diffLines(baseCommit, testedCommit, 'crates/noble-cli/src/main.rs'),
    calculator_rs: diffLines(baseCommit, testedCommit, 'crates/noble-cli/src/calculator.rs'),
    reachability: { observed, ngraph_run_calls_in_main_rs: dispatches, noble_ngraph_references_in_noble_cli: references,
      manifests_naming_noble_ngraph: manifests },
    k_num_01: { unchanged: baseBlock !== null && baseBlock === testedBlock,
      base_sha256: baseBlock === null ? null : sha(baseBlock), tested_sha256: testedBlock === null ? null : sha(testedBlock) },
    result: identical && checked && ordinaryStat === '' ? 'passed' : 'failed',
  };
}

function helpListing(R, tested, base) {
  const run = (side, binary) => {
    const cwd = path.join(R.target, 'help', side);
    fs.mkdirSync(cwd, { recursive: true });
    return execute(R, { label: `help-${side}`, executable: binary, argv: ['--help'], cwd,
      meta: { scenario: 'help', side, step: 'help', case_id: 'NGRAPH-CASE-15' } });
  };
  const lists = text => ['ngraph', 'calculator'].filter(name =>
    new RegExp(`^\\s*(?:usage: )?noble ${name}\\b`, 'm').test(text));
  const [testedRun, baseRun] = [run('tested', tested), run('base', base)];
  const [testedText, baseText] = [testedRun, baseRun].map(item => item.stdout.toString('utf8'));
  return { command: testedRun.id, exit: testedRun.exit, lists: lists(testedText), base_command: baseRun.id,
    base_lists: lists(baseText), additive_to_base: testedText.startsWith(baseText.trimEnd()) };
}

// NGRAPH-CASE-15: every planned command twice, byte for byte.
function compareRepetitions(planned, runs) {
  const mismatches = [];
  let compared = 0;
  let stdoutIdentical = true;
  const changed = result => Object.fromEntries(Object.entries(result.after ?? {})
    .filter(([name, digest]) => (result.before ?? {})[name] !== digest));
  for (const scenario of planned.scenarios) {
    const [first, second] = [runs[1][scenario.id], runs[2][scenario.id]];
    for (const step of scenario.steps.filter(item => item.argv)) {
      compared += 1;
      const [x, y] = [first.results[step.id], second.results[step.id]];
      const differs = [];
      if (!x.stdout.equals(y.stdout)) {
        differs.push('stdout');
        stdoutIdentical = false;
      }
      if (!x.stderr.equals(y.stderr)) differs.push('stderr');
      if (x.exit !== y.exit) differs.push('exit');
      if (!same(changed(x), changed(y))) differs.push('published bytes');
      if (differs.length) mismatches.push({ scenario: scenario.id, step: step.id, differs, commands: [x.id ?? null, y.id ?? null] });
    }
    if (!same(first.tree, second.tree)) mismatches.push({ scenario: scenario.id, step: null, differs: ['final directory'] });
  }
  return { repetitions: REPETITIONS, compared_commands: compared, stdout_identical: stdoutIdentical,
    compared_fields: ['stdout', 'stderr', 'exit', 'bytes published by each command', 'final scenario directory'],
    mismatches, result: mismatches.length ? 'failed' : 'passed' };
}

// ---------------------------------------------------------------------------
// run
// ---------------------------------------------------------------------------

const SCOPES = {
  'NGRAPH-CASE-01': 'CORE-01..18 on the base and tested binaries, plus diff and dispatch observations; the case is a review: a recorded review is still required',
  'NGRAPH-CASE-02': 'not run by this runner; delegated to verification/ngraph-purity/gate.mjs, whose observation binds the implemented/passed state recorded in numeric-graph-cases.json',
  'NGRAPH-CASE-06': 'library tests bound through the gate rust-tests log',
  'NGRAPH-CASE-09': 'library tests bound through the gate rust-tests log',
  'NGRAPH-CASE-14': 'library tests, the soft-float log, and the B5 identities observed through the CLI',
  'NGRAPH-CASE-15': 'determinism of every planned command and --help only; the claims review is recorded separately',
};
// Steps that only prepare an observed variant: they must complete normally.
const SUPPORT = [['check', 'extract-i1'], ['eval-probes', 'extract-i1'], ['eval-outcomes', 'train-f1-s0'],
  ['b5', 'train-s0'], ['b5', 'bind-forged'], ['b5', 'train-f2-s0'], ['b5', 'train-f1-fact-s0'],
  ['train-refusals', 'train-other-s0']];
const ASSUMPTIONS = [
  'One pinned x86_64 Linux platform and toolchain; no cross-platform bit-reproducibility is claimed (NGRAPH-CASE-13 platform_scope).',
  'Evaluator agreement and replay are finite differential observations, not proofs; NG-PO-A..H stay open.',
  'The binaries are trusted to be release builds of --tested-commit and --base-commit: sha256 binds bytes, not build provenance. The projected crate sources are checked against the --tested-commit blobs.',
  'Library tests are bound through the gate rust-tests log by name only (the log does not name test binaries): each name must appear exactly once as ok and be defined once in its file at --tested-commit.',
  'The soft-float log is bound by sha256; the log itself does not record its commit.',
  'Forged snapshots come from this runner\'s independent D6/BLAKE3 encoding, which must first reproduce the unforged snapshot byte for byte.',
  'CORE-03, CORE-05 and CORE-09 compare compiled Wasm text (their runtime inputs go through the in-repo Node host); CORE-15 and CORE-16 are cargo-test controls bound through the log.',
  'Trusted tools: rustc and cargo (the builds), the selected Node (this runner) and git.',
];

function runAcceptance(directory, options) {
  for (const name of ['binary', 'base-binary', 'base-commit', 'gate-receipt', 'tested-commit', 'soft-float-log']) {
    assert.ok(options[name] !== undefined, `missing --${name}\n${USAGE}`);
  }
  for (const name of ['binary-sha256', 'base-binary-sha256']) {
    if (options[name] !== undefined) assert.ok(hex64(options[name]), `--${name} takes 64 lowercase hex digits`);
  }
  const node = selectedNode();
  const target = outside(directory);
  const planFile = path.join(target, 'plan.json');
  assert.ok(fs.existsSync(planFile), 'run needs a directory written by plan');
  for (const name of ['executables', 'runs', 'commands', 'core', 'help', 'tmp', 'acceptance.json', 'manifest.json']) {
    assert.ok(!fs.existsSync(path.join(target, name)), 'a planned directory is executed once');
  }
  const planBytes = fs.readFileSync(planFile);
  const planned = JSON.parse(planBytes);
  assert.equal(planned.schema, PLAN_SCHEMA, 'plan schema');
  assert.equal(planned.status, 'planned-not-executed', 'plan status');
  assert.equal(planned.output_directory, target, 'the plan was written for another directory');
  assert.equal(planned.source_root, root, 'the plan was written from another checkout');
  assert.equal(fileSha(path.join(root, RUNNER)), planned.runner.sha256, 'the runner changed after planning; plan again');
  const receiptFile = path.resolve(options.receipt ?? path.join(root, RECEIPT));
  assert.ok(!fs.existsSync(receiptFile), `${receiptFile} already exists`);
  // NGRAPH-CASE-01 compares against the pre-change baseline: an ancestor of
  // the tested commit that has no crates/noble-ngraph.
  const commit = text => git(['rev-parse', '--verify', '--quiet', `${text}^{commit}`]).stdout.trim();
  const testedCommit = commit(options['tested-commit']);
  const baseCommit = commit(options['base-commit']);
  assert.equal(git(['merge-base', '--is-ancestor', baseCommit, testedCommit], { allow: [0, 1] }).status, 0,
    `--base-commit ${baseCommit} is not an ancestor of --tested-commit ${testedCommit}`);
  assert.notEqual(git(['cat-file', '-e', `${baseCommit}:crates/noble-ngraph`], { allow: [0, 1, 128] }).status, 0,
    `--base-commit ${baseCommit} already contains crates/noble-ngraph; the baseline must predate the change`);
  const receipt = {
    schema: SCHEMA, profile: PROFILE, result: 'running', tested_commit: testedCommit, base_commit: baseCommit,
    source_revision: null, source: null,
    plan: { file: 'plan.json', sha256: sha(planBytes), case_file: planned.cases.file,
      case_file_sha256: planned.cases.sha256, case_revision: planned.cases.revision,
      workload_sha256: planned.cases.workload_sha256 },
    tools: { node, git: null, noble: null, noble_base: null },
    commands: [], cases: [], library_tests: null, soft_float: null, determinism: null, core_identity: null,
    help: null, derivations: [], failures: [], integrity_failures: [], external_raw_output: null,
    assumptions: ASSUMPTIONS, summary: null,
  };
  const R = { target, planned, receipt, serial: 0, watched: [], hashes: new Map(), tmp: path.join(target, 'tmp') };
  fs.mkdirSync(R.tmp);
  let projection = null;
  try {
    if (!planned.cases.in_repository) {
      receipt.failures.push({ case_id: 'cases', error: `${planned.cases.file} is not a repository case file (development control only)` });
    }
    R.watched.push({ file: planned.cases.in_repository ? path.join(root, planned.cases.file) : planned.cases.file,
      sha256: planned.cases.sha256 }, { file: path.join(root, CORE_CASES), sha256: planned.core_cases.sha256 },
    { file: path.join(root, RUNNER), sha256: planned.runner.sha256 });
    for (const [name, entry] of Object.entries(planned.fixtures)) {
      R.watched.push({ file: path.join(target, entry.file), sha256: entry.sha256 },
        { file: path.join(root, INPUTS, name), sha256: entry.sha256 });
    }
    for (const entry of R.watched) assert.equal(fileSha(entry.file), entry.sha256, `${entry.file} changed after planning`);
    const binary = freeze(R, 'noble', options.binary, options['binary-sha256']);
    const base = freeze(R, 'noble-base', options['base-binary'], options['base-binary-sha256']);
    const describe = item => ({ original: item.original, realpath: item.realpath, frozen: item.name, sha256: item.sha256,
      bytes: item.bytes, pinned_sha256: item.pinned_sha256 });
    receipt.tools.noble = { ...describe(binary), built_from: testedCommit };
    receipt.tools.noble_base = { ...describe(base), built_from: baseCommit };
    const bound = bindSource(R, testedCommit);
    projection = bound.projection;
    receipt.source = bound.section;
    receipt.source_revision = `sha256:${projection.sha256}`;
    receipt.tools.git = projection.git;
    receipt.library_tests = bindLibraryTests(R, options['gate-receipt'], testedCommit);
    receipt.soft_float = bindSoftFloat(R, options['soft-float-log']);

    const runs = {};
    for (let repetition = 1; repetition <= REPETITIONS; repetition += 1) {
      runs[repetition] = Object.fromEntries(planned.scenarios.map(scenario =>
        [scenario.id, runScenario(R, scenario, repetition, binary)]));
    }
    for (const [repetition, scenarios] of Object.entries(runs)) {
      for (const [id, run] of Object.entries(scenarios)) {
        for (const result of Object.values(run.results)) {
          for (const error of [result.error, result.report_error].filter(Boolean)) {
            receipt.failures.push({ case_id: 'scenario', scenario: id, repetition: Number(repetition), step: result.step, error });
          }
          if (Number(repetition) === 1 && result.derived) {
            receipt.derivations.push({ scenario: id, step: result.step, derive: result.derived, detail: result.detail });
          }
        }
      }
    }
    const scenario = id => runs[1][id];
    const at = (id, step) => scenario(id).results[step];
    const b5Snapshot = primary(at('b5', 'train-b5'))?.snapshot;
    const supporting = SUPPORT.map(([id, step]) => {
      const result = at(id, step), line = primary(result);
      const planStep = planned.scenarios.find(item => item.id === id).steps.find(item => item.id === step);
      const passed = result.exit === 0 && line?.outcome === 'normal'
        && (step !== 'bind-forged' || (line.bound === true && hex64(line.snapshot) && line.snapshot !== b5Snapshot));
      if (!passed) {
        receipt.failures.push({ case_id: planStep.case_id, scenario: id, step, error: `supporting step did not complete normally (exit ${result.exit})` });
      }
      return { case_id: planStep.case_id, scenario: id, step, command: result.id ?? null, exit: result.exit,
        outcome: line?.outcome ?? null, snapshot: line?.snapshot ?? null, graph: line?.graph ?? null,
        bound: line?.bound ?? null, result: passed ? 'passed' : 'failed' };
    });

    receipt.core_identity = coreIdentity(R, base, binary, baseCommit, testedCommit);
    receipt.help = helpListing(R, binary, base);
    receipt.determinism = compareRepetitions(planned, runs);
    for (const mismatch of receipt.determinism.mismatches) {
      receipt.failures.push({ case_id: 'NGRAPH-CASE-15', scenario: mismatch.scenario, step: mismatch.step,
        error: `repetitions differ in ${mismatch.differs.join(', ')}` });
    }
    const extra = { core: receipt.core_identity, determinism: receipt.determinism, help: receipt.help };
    const libraryResults = new Map(receipt.library_tests.tests.map(test => [test.name, test.result]));
    for (const row of planned.cases.workload) {
      const c = caseContext(row);
      const problems = evaluateCase(row.id, c, at, scenario, extra);
      const tests = LIBRARY_TESTS[row.id] ?? [];
      const support = supporting.filter(item => item.case_id === row.id);
      const ok = c.fields.every(field => field.result === 'passed') && problems.length === 0
        && tests.every(name => libraryResults.get(name) === 'passed') && support.every(item => item.result === 'passed')
        && (row.id !== 'NGRAPH-CASE-14' || receipt.soft_float.result === 'passed')
        && (row.id !== 'NGRAPH-CASE-15' || receipt.determinism.result === 'passed')
        && (row.id !== 'NGRAPH-CASE-01' || receipt.core_identity.result === 'passed');
      for (const field of c.fields.filter(item => item.result === 'failed')) {
        receipt.failures.push({ case_id: row.id, path: field.path, expected: field.expected, observed: field.observed,
          step: field.step ?? null, command: field.command ?? null });
      }
      for (const problem of problems) receipt.failures.push({ case_id: row.id, error: problem });
      receipt.cases.push({ case_id: row.id, kind: row.kind, scope: SCOPES[row.id] ?? 'exact case inputs through noble ngraph',
        result: row.id === 'NGRAPH-CASE-02' ? 'not-run' : ok ? 'passed' : 'failed',
        review_required: row.kind === 'review',
        library_tests: tests.map(name => ({ name, result: libraryResults.get(name) ?? 'failed' })),
        supporting: support, fields: c.fields, accounted: c.accounted, accounting_problems: problems,
        commands: receipt.commands.filter(command => command.case_id === row.id).map(command => command.id) });
    }
  } catch (error) {
    receipt.failures.push({ case_id: 'runner', error: errorText(error) });
  } finally {
    for (const entry of R.watched) {
      try {
        if (fileSha(entry.file) !== entry.sha256) receipt.integrity_failures.push({ file: display(entry.file), error: 'changed during the run' });
      } catch (error) {
        receipt.integrity_failures.push({ file: display(entry.file), error: errorText(error).split('\n')[0] });
      }
    }
    if (fileSha(planFile) !== sha(planBytes)) receipt.integrity_failures.push({ file: 'plan.json', error: 'changed during the run' });
    if (!same(fs.readdirSync(path.join(root, INPUTS)).sort(), Object.keys(planned.fixtures).sort())) {
      receipt.integrity_failures.push({ file: INPUTS, error: 'the committed inputs changed during the run' });
    }
    if (projection !== null) {
      try {
        const again = gitSourceProjection(root, projection.rows.map(row => row.path), receipt.source.frozen);
        if (!same(again, projection)) receipt.integrity_failures.push({ file: 'source projection', error: 'changed during the run' });
      } catch (error) {
        receipt.integrity_failures.push({ file: 'source projection', error: errorText(error).split('\n')[0] });
      }
    }
    const manifest = {};
    const walk = (folder, prefix) => {
      for (const entry of fs.readdirSync(folder, { withFileTypes: true }).sort(byName)) {
        const name = `${prefix}${entry.name}`;
        if (entry.isDirectory()) walk(path.join(folder, entry.name), `${name}/`);
        else if (!['acceptance.json', 'manifest.json'].includes(name)) {
          const bytes = fs.readFileSync(path.join(folder, entry.name));
          manifest[name] = { sha256: sha(bytes), bytes: bytes.length };
        }
      }
    };
    walk(target, '');
    const manifestBytes = Buffer.from(`${JSON.stringify(manifest, null, 2)}\n`);
    fs.writeFileSync(path.join(target, 'manifest.json'), manifestBytes, { flag: 'wx', mode: 0o444 });
    receipt.external_raw_output = { directory: target, manifest: 'manifest.json', manifest_sha256: sha(manifestBytes),
      files: Object.keys(manifest).length, bytes: Object.values(manifest).reduce((sum, item) => sum + item.bytes, 0),
      receipt_copy: 'acceptance.json' };
    const tests = receipt.library_tests?.tests ?? [];
    receipt.summary = {
      cases: Object.fromEntries(receipt.cases.map(row => [row.case_id, row.result])),
      commands: receipt.commands.length,
      scenario_commands_per_repetition: planned.scenarios.reduce((sum, item) => sum + item.steps.filter(step => step.argv).length, 0),
      core_rows: receipt.core_identity?.rows.length ?? 0,
      library_tests: { total: tests.length, passed: tests.filter(test => test.result === 'passed').length,
        failed: tests.filter(test => test.result !== 'passed').map(test => test.name) },
      failures: receipt.failures.length, integrity_failures: receipt.integrity_failures.length,
    };
    receipt.result = receipt.failures.length === 0 && receipt.integrity_failures.length === 0 ? 'passed' : 'failed';
    const bytes = Buffer.from(`${JSON.stringify(receipt, null, 2)}\n`);
    fs.writeFileSync(path.join(target, 'acceptance.json'), bytes, { flag: 'wx', mode: 0o444 });
    fs.mkdirSync(path.dirname(receiptFile), { recursive: true });
    fs.writeFileSync(receiptFile, bytes, { flag: 'wx' });
    console.log(JSON.stringify({ schema: SCHEMA, result: receipt.result, receipt: receiptFile, receipt_sha256: sha(bytes),
      external_raw_output: target, summary: receipt.summary }));
    process.exitCode = receipt.result === 'passed' ? 0 : 1;
  }
}

function main(argv) {
  const [mode, directory, ...rest] = argv;
  const string = { type: 'string' };
  if (mode === 'plan' && directory !== undefined) {
    const { values } = parseArgs({ args: rest, options: { cases: string }, strict: true, allowPositionals: false });
    console.log(JSON.stringify(plan(directory, values)));
  } else if (mode === 'run' && directory !== undefined) {
    const { values } = parseArgs({ args: rest, strict: true, allowPositionals: false, options: {
      binary: string, 'base-binary': string, 'gate-receipt': string, 'tested-commit': string, 'soft-float-log': string,
      'base-commit': string, 'binary-sha256': string, 'base-binary-sha256': string, receipt: string } });
    runAcceptance(directory, values);
  } else {
    console.error(USAGE);
    process.exitCode = 2;
  }
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main(process.argv.slice(2));
