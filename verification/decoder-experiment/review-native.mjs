import {createHash} from 'node:crypto';
import {readFileSync,writeFileSync} from 'node:fs';

const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const files = ['verification/decoder-experiment/review-native.mjs',
  'verification/decoder-experiment/acceptance.json',
  'verification/decoder-experiment/src/lib.rs',
  'verification/decoder-experiment/fixtures/escape.rs',
  '.cairn/specs/verification-toolchain/spec.md',
  'specs/conformance/adaptation-cases.json','policy/tool-selection.json'];
const source = Object.fromEntries(files.map(path => [path,hash(readFileSync(path))]));
const receipt = JSON.parse(readFileSync(files[1]));
const selection = JSON.parse(readFileSync('policy/tool-selection.json'));
const item = JSON.parse(readFileSync('specs/conformance/adaptation-cases.json')).cases.find(row => row.id === 'ADAPT-13');
if (item?.kind !== 'review' || item.input.variants.length !== 4 ||
    receipt.result !== 'passed' || receipt.cases['ADAPT-04'].result !== 'passed' ||
    receipt.cases['ADAPT-05'].result !== 'passed') {
  throw Error('scoped local decoder Miri source or ADAPT-13 review inputs are incomplete');
}
const local = {
  tests:receipt.tools.miri_tests_passed,
  target:receipt.tools.miri_target,
  flags:receipt.tools.miri_flags,
  compileFailures:receipt.tools.compile_fail_codes,
  inputHash:receipt.source.input_sha256['verification/decoder-experiment/src/lib.rs'],
  selectedTool:selection.tool_paths.extraction_rust.output,
};
const actualLocal = local.tests === 3 && local.target === 'x86_64-unknown-linux-gnu' &&
  local.flags === '-Zmiri-strict-provenance' &&
  local.compileFailures.includes('E0515') && local.compileFailures.includes('E0502') &&
  local.inputHash === hash(readFileSync('verification/decoder-experiment/src/lib.rs')) &&
  local.selectedTool.includes('nightly-2026-08-18');
if (!actualLocal) throw Error('pinned Noble-local scoped Miri control missing');

// Actual scoped receipt is used for the positive lane; the other canonical
// records are hostile policy cases. Neither an unsupported run nor upstream
// testing can be promoted by a mere claim that some Miri once passed.
function classify(row) {
  if (row.safety_argument !== 'cited-with-assumptions') return 'reject-claim';
  if (row.miri === 'required-but-unsupported') return 'required-lane-unmet';
  if (row.miri === 'upstream-only') return 'Noble-lane-unmet';
  if (row.miri === 'passed-scoped-pinned' && actualLocal) return 'scoped-evidence-only';
  throw Error('unknown native assurance record');
}
const observations = item.input.variants.map(row => {
  const observed = classify(row);
  if (observed !== row.outcome) throw Error(`native assurance variant mismatched: ${JSON.stringify(row)}`);
  return {safety_argument:row.safety_argument,miri:row.miri,observed};
});
const result = {schema:'noble-adapt13-assurance-review/v1',case:'ADAPT-13',kind:'review',result:'passed',
  claim:'All four native-boundary evidence-policy variants were classified. A cited safe-borrow/bounded-copy argument plus actual Noble-local pinned strict Miri and compile-fail controls yields scoped evidence only; missing argument, unsupported required Miri and upstream-only Miri cannot satisfy that lane.',
  observations,local_evidence:{...local,verified:true,
    receipt_sha256:source['verification/decoder-experiment/acceptance.json'],
    wasm_correspondence_proven:false,all_targets_covered:false,
    zerocopy_crate_selected:false},source:{input_sha256:source},
  assumptions:['The local safety argument covers safe Rust slices, bounded record reads and explicit external refusal, not arbitrary foreign-memory stability or every target.',
    'Three selected strict Miri tests and E0515/E0502 compile failures are scoped native observations. They do not prove Noble-to-Wasm correspondence, independent extraction/refinement, or zerocopy upstream behavior.']};
const text=JSON.stringify(result,null,2)+'\n';
if (process.argv.includes('--record')) writeFileSync('verification/decoder-experiment/adapt13-review.json',text,{flag:'wx'});
process.stdout.write(text);
