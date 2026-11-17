// Pure accounting shared by the real gate and its adversarial controls.
// Charon is the body inventory; Aeneas is the translation inventory. Neither a
// handwritten Lean model nor a test function can satisfy a local-body entry.
import path from 'node:path';
import crypto from 'node:crypto';

export const schema = 'm4-extraction-lock/v1';
export const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
export const canonical = (value, normalize) => JSON.stringify(value, (_, item) => {
  if (!item || typeof item !== 'object' || Array.isArray(item)) return item;
  if (normalize) item = normalize(item);
  return Object.fromEntries(Object.keys(item).sort().map(key => [key, item[key]]));
});
export function fail(code, detail) { throw Error(`M4-${code}: ${detail}`); }
export function equal(actual, expected, code, detail) {
  if (canonical(actual) !== canonical(expected)) fail(code, detail);
}
export function exactBytes(actual, expected, detail) {
  if (!Buffer.from(actual).equals(Buffer.from(expected))) fail('GENERATED', detail);
}

// Independent discoveries of identical sources/tooling differed only in these
// seven macro-expansion span end columns: zero width versus the full macro token.
// Bind the exact source site and one occurrence per site; do not erase ordinary
// spans, the rest of these spans, any body, or any extraction option.
const syntheticExpansionSites = {
  noble_kernel: [
    { file: 'crates/noble-kernel/src/lib.rs', crate: 'noble_kernel', line: 36, end: 19,
      source: '        match $step {' },
    { file: '/rustc/library/core/src/macros/mod.rs', crate: 'core', line: 434, end: 25,
      source: null },
    { file: 'crates/noble-kernel/src/contracts/nominal/schemes.rs', crate: 'noble_kernel', line: 5, end: 24,
      source: '        match $candidate {' },
  ],
  noble_contracts: [
    { file: 'crates/noble-contracts/src/lib.rs', crate: 'noble_contracts', line: 23, end: 19,
      source: '        match $step {' },
    { file: '/rustc/library/core/src/macros/mod.rs', crate: 'core', line: 434, end: 25,
      source: null },
    { file: 'crates/noble-contracts/src/source/declared/state.rs', crate: 'noble_contracts', line: 9, end: 24,
      source: '        match $candidate {' },
  ],
  noble_wasm: [
    { file: 'crates/noble-wasm/src/lib.rs', crate: 'noble_wasm', line: 53, end: 19,
      source: '        match $step {' },
  ],
};

export function canonicalLlbcHash(llbc, normalizedSpans) {
  // Charon serializes this name map in hash-table order and records the physical
  // output directory. Preserve every other field, including all bodies/options.
  const translated = llbc.translated;
  const sites = syntheticExpansionSites[translated.crate_name];
  if (!sites) fail('LLBC-SPAN', `unreviewed crate ${translated.crate_name}`);
  const byFile = new Map();
  for (const site of sites) {
    const files = translated.files.filter(file => file?.name?.Local === site.file);
    if (files.length !== 1 || files[0].crate_name !== site.crate ||
        (site.source === null ? files[0].contents !== null :
          files[0].contents?.split('\n')[site.line - 1] !== site.source)) {
      fail('LLBC-SPAN', `unreviewed expansion source ${site.file}:${site.line}`);
    }
    byFile.set(files[0].id, { ...site, seen: 0 });
  }
  const normalizeSpan = item => {
    const value = item.Value;
    if (!Array.isArray(value) || value.length !== 2 || typeof value[0] !== 'number') return item;
    const span = value[1];
    const data = span?.data;
    const site = byFile.get(data?.file_id);
    if (!site || data.beg?.line !== site.line || data.beg?.col !== 8 ||
        data.end?.line !== site.line || span.generated_from_span !== null) return item;
    if (data.end.col !== 8 && data.end.col !== site.end) {
      fail('LLBC-SPAN', `unreviewed expansion end ${site.file}:${site.line}:${data.end.col}`);
    }
    if (++site.seen !== 1) fail('LLBC-SPAN', `duplicate expansion ${site.file}:${site.line}`);
    normalizedSpans?.push({ source: site.file, line: site.line, begin_col: 8,
      observed_end_col: data.end.col, canonical_end_col: site.end, field: 'span.Value[1].data.end.col' });
    return data.end.col === site.end ? item : { ...item, Value: [value[0], {
      ...span, data: { ...data, end: { ...data.end, col: site.end } },
    }] };
  };
  const names = translated.short_names.map(row => ({ key: canonical(row.key), row }));
  names.sort((a, b) => a.key < b.key ? -1 : a.key > b.key ? 1 : 0);
  for (let index = 1; index < names.length; index++) {
    if (names[index - 1].key === names[index].key) fail('LLBC-NAMES', 'duplicate short-name key');
  }
  const binding = canonical({ ...llbc, translated: { ...translated,
    options: { ...translated.options, dest_file: path.basename(translated.options.dest_file) },
    short_names: names.map(entry => entry.row) } }, normalizeSpan);
  for (const site of byFile.values()) {
    if (site.seen !== 1) fail('LLBC-SPAN', `missing expansion ${site.file}:${site.line}`);
  }
  return sha(binding);
}

export const lanes = [
  { id: 'kernel', crate: 'noble_kernel', package: 'noble-kernel', basename: 'noble_kernel',
    module: 'NobleKernel', checked: 'proofs/m3/NobleKernel' },
  { id: 'contracts', crate: 'noble_contracts', package: 'noble-contracts', basename: 'noble_contract_impl',
    module: 'NobleContractImpl', checked: 'proofs/mc1/NobleContractImpl' },
  { id: 'wasm', crate: 'noble_wasm', package: 'noble-wasm', basename: 'noble_wasm_impl',
    module: 'NobleWasmImpl', checked: 'proofs/m3-wasm/NobleWasmImpl' },
];
export const inheritedOpaque = [
  'noble_kernel::shapes::impls::clone_parts',
  'noble_kernel::shapes::impls::debug_parts',
  'noble_kernel::shapes::impls::debug_slots',
  'noble_kernel::types::impls::clone_stack',
  'noble_kernel::types::impls::debug_stack',
];
// Reviewed renewal: the original complete-file hashes were
// types/impls.rs=f834b36f972feb518dfcdaf9c9f4f05c2ecc787a18f937a7f2e6d1a0898f0c3e
// shapes/impls.rs=c7aec68c0201b14d978d4d3b7b9906794176088e0b3b3ca8912090ad1a505203.
// Finite nominal Clone/Debug branches and one nominal equality arm were added.
// The latest renewal directly clones and formats ordered nominal payloads and
// enqueues their structural equality pairs rather than introducing mutual
// trait-implementation recursion through NominalShape's derived methods.
// Pattern::Nominal's new formatting arm writes the same default debug fields
// directly rather than creating a borrowed debug-tuple builder.
// All five inherited opaque helper implementations below retain their
// independently compared, separately pinned original body bytes.
// Cloning a finite stack still preserves order and invokes each element's
// structural Clone; nominal identity and shape are copied, not re-resolved.
// Formatting remains observational and propagates formatter failure; pretty
// layout and allocator success remain outside the extracted claim.
// Recent bounded structural-equality refactors changed adjacent non-opaque
// code in both impls.rs files. Re-pin the current file bytes only after the
// five separately hashed opaque helper bodies are verified unchanged.
// The #97 re-extraction renewal (prior types/impls.rs pin
// 636c0927667693891485be5bf1d57527975c8e7b13e217000f234c152e11a02f) only
// queues ty_eq's Program element pairs inside its walk instead of through a
// helper that threaded the borrowed work stack; clone_stack and debug_stack
// keep their pinned bodies.
export const inheritedOpaqueSources = {
  'crates/noble-kernel/src/types/impls.rs': '479761e9371a17ac9695e89a729a28dadfd20e2b106a985a1e405af7145a17df',
  'crates/noble-kernel/src/shapes/impls.rs': 'c5e01ec491af3ac46488e567ff2d3c0b9531a22e11cc842ecab7604544620413',
};
export const inheritedOpaqueBodies = {
  'crates/noble-kernel/src/types/impls.rs': {
    clone_stack: '5ae77f5a6d771ef94aec049717bb2f6a8d350dcb258ac22e28ec604d58d3f6e7',
    debug_stack: '631e7abdd5d4a1210a72d195f021f3de8f5b99fb41e5e4def25b29073124f04e',
  },
  'crates/noble-kernel/src/shapes/impls.rs': {
    clone_parts: '934493f41ceb76c80f78b1800bf9a38b3c619d0ba0217c54db9e7e19142beb26',
    debug_parts: 'c93c255598269fbc11e74206efca41ac2729955b8c948ec5ae7a481f2e942e9d',
    debug_slots: '8295e5745e323de390d59d074d1e348c2345111badc8cf82d1adcb73c8bb9527',
  },
};
// Reviewed #97 local opaque boundaries of the frontend and backend lanes. The
// frontend's host plug-in handles hold `dyn` trait objects or a host `fn`
// pointer, which the pinned Aeneas cannot represent: the four handle types are
// opaque and the methods that call through them are opaque. Three frontend
// Debug renderers and the backend's `debug_origin` are opaque because a
// transparent body needs a trait-instance cycle the pinned Aeneas refuses;
// formatting layout is already an abstracted, observation-only boundary. Each
// is a modelled boundary, never an extracted body, and its exact annotated item
// is pinned in `reviewedAnnotations` below.
export const reviewedOpaque = {
  contracts: [
    'noble_contracts::intrinsic::debug_form_kind',
    'noble_contracts::intrinsic::debug_proof_dependency',
    'noble_contracts::source::chunks::{noble_contracts::source::chunks::MonotonicClock}::now_ns',
    "noble_contracts::source::chunks::{noble_contracts::source::chunks::ObserverHandle<'_0>}::observe",
    "noble_contracts::source::chunks::{noble_contracts::source::chunks::RankerHandle<'_0>}::model_identity",
    "noble_contracts::source::chunks::{noble_contracts::source::chunks::RankerHandle<'_0>}::provenance_matches",
    "noble_contracts::source::chunks::{noble_contracts::source::chunks::RankerHandle<'_0>}::score",
    'noble_contracts::source::editor::debug_node',
    'noble_contracts::source::procedures::acceptance::{impl core::fmt::Debug for noble_contracts::source::procedures::acceptance::OwnerVerifier}::fmt',
    'noble_contracts::source::procedures::acceptance::{noble_contracts::source::procedures::acceptance::OwnerVerifier}::verify',
  ],
  wasm: ['noble_wasm::source::debug_origin'],
};
export const reviewedOpaqueTypes = {
  contracts: [
    'noble_contracts::source::chunks::MonotonicClock',
    'noble_contracts::source::chunks::ObserverHandle',
    'noble_contracts::source::chunks::RankerHandle',
    'noble_contracts::source::procedures::acceptance::OwnerVerifier',
  ],
  wasm: [],
};
// One out-of-line oracle belongs exclusively to a compiler-confirmed cfg(test)
// module. Its bytes remain source-bound, but it is not a production body. This
// is an exact path/parent/directive review, not a filename or directory waiver.
export const testOnlySourceModules = {
  'crates/noble-contracts/src/inference/effects/oracle.rs': {
    parent: 'crates/noble-contracts/src/inference/effects.rs',
    directive: '#[cfg(test)]\nmod oracle;',
  },
};
// Every Charon annotation outside the separately body-pinned inherited kernel
// helpers is bound to the exact item it decorates: first line, complete item
// SHA-256 and line count, in source order. Added/removed annotations or changed
// item extents/bodies refuse review, including rename and excluded adapters.
// - rename: extraction-only Lean names. A field named `source` would shadow the
//   `source.*` namespace of later field types, and three type names would
//   collide with other Lean declarations. Rust names and semantics are unchanged.
// - opaque: the `reviewedOpaque`/`reviewedOpaqueTypes` boundaries above.
// - exclude: public host adapters whose signatures name `dyn` trait objects,
//   nested borrows or host `fn` clocks. They are not part of the extraction: no
//   translated item can reference them (the pinned extraction then fails), and
//   the checked logic each one wraps, named in its doc comment, is extracted.
export const reviewedAnnotations = {
  "crates/noble-contracts/src/component/preparation.rs": [
    { annotation: "exclude", item: "pub fn check_import_effect(&self, word: &str, claimed: &[&str]) -> Result<(), super::Error> {",
      lines: 13, sha256: "5929e3108c71251070b90925398ee51d7b5030b131d261e770f2a444bf439cc6" },
  ],
  "crates/noble-contracts/src/inference/mod.rs": [
    { annotation: "variants_suffix(\"Term\")", item: "pub(crate) enum Term {",
      lines: 23, sha256: "a3e59dadb87a11de99f0285b5dab7fdf632b1327823f47b7abb831ccb9edf16f" },
  ],
  "crates/noble-contracts/src/intrinsic.rs": [
    { annotation: "opaque", item: "fn debug_form_kind(kind: &FormKind, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {",
      lines: 6, sha256: "dc77b2b97d5b71fb70c30969b6148ccfe70e07cc189ea759005afc135c852217" },
    { annotation: "opaque", item: "fn debug_proof_dependency(",
      lines: 16, sha256: "a7e38618f573f6f7d6995067782ab72dc66ba032f7a8144697b337b87a3cc2cc" },
    { annotation: "rename(\"IntrinsicMeter\")", item: "struct Meter<'a> {",
      lines: 7, sha256: "af149099953ff42060b75be1bc4416f25ba1fce095f1f9c9b97b038a565bef00" },
    { annotation: "variants_suffix(\"Code\")", item: "enum Code {",
      lines: 10, sha256: "7a87adf9158b5cb0bacf72b3d3ccde4a5d9172c704c01d11478422a24617eb92" },
  ],
  "crates/noble-contracts/src/lib.rs": [
    { annotation: "variants_suffix(\"Expr\")", item: "pub enum ExprKind {",
      lines: 34, sha256: "2fdd04e3c6b85d06d01153cb6d9aa71935d88584c0e71f45707a259ad0e32a79" },
  ],
  "crates/noble-contracts/src/source.rs": [
    { annotation: "rename(\"SourcePrepared\")", item: "pub struct Prepared {",
      lines: 20, sha256: "2eaef3abb8af04e8d7bcbac8e40870bc60e60ad1d40c0b6e18a5790b89a934cc" },
  ],
  "crates/noble-contracts/src/source/chunks.rs": [
    { annotation: "variants_suffix(\"Scalar\")", item: "pub enum Scalar {",
      lines: 6, sha256: "a33029a7eaf4a790fefcf5bce1eddcd8d44eacf3a4277f9f23e36a9b0a38c7c1" },
    { annotation: "variants_suffix(\"Node\")", item: "pub enum Node {",
      lines: 19, sha256: "6a56787ce015ee91bdf3d0818638d72d1c7b6177f21110e889fd3fc36feb13c1" },
    { annotation: "rename(\"source_bytes\")", item: "source: alloc::vec::Vec<u8>,",
      lines: 1, sha256: "58f4e27ef8d4038bd5ef2bef2d0d17196390eea4a6f465f4e78c32ab84c6a080" },
    { annotation: "variants_suffix(\"Element\")", item: "pub enum Element {",
      lines: 12, sha256: "2ac74f5af12fdcf57d6be1414704bd7bfbc73771e06a920dc315c3a9b28f4071" },
    { annotation: "opaque", item: "struct ObserverHandle<'a>(&'a mut CandidateOutcomeObserver<'a>);",
      lines: 1, sha256: "4cc011e932b011ac1af5c7e9037f166a743d1da94a71032309e3772bef30c42b" },
    { annotation: "opaque", item: "fn observe(&mut self, outcome: &CandidateOutcome) {",
      lines: 3, sha256: "7aadff513dde51bb4a90df5a6755302e58d85878b55f21e33e6f4eef558e58b5" },
    { annotation: "exclude", item: "pub const fn new(",
      lines: 9, sha256: "2c2f703fa6cdc6be4682e5def2d62cbf2a67a6c66c2372412386e03e20cbc4d6" },
    { annotation: "exclude", item: "fn reborrow(&mut self) -> RankerSession<'_> {",
      lines: 6, sha256: "f8ff46ac9551dddee04f598928f67aa8b368eed1b6c31b67debb5b0b22d24441" },
    { annotation: "opaque", item: "struct RankerHandle<'a>(&'a mut dyn CandidateRanker);",
      lines: 1, sha256: "573bbe018d7b7440275cd3627008a32c616ef26d27c0b548ab1083a8891bc12a" },
    { annotation: "opaque", item: "fn model_identity(&self) -> [u8; REVISION_BYTES] {",
      lines: 3, sha256: "6a35ead83ee60d3438aebe51563fe8c0912b260dfa3480ddb280d6a1fc70f84d" },
    { annotation: "opaque", item: "fn provenance_matches(&self, identity: &RankingIdentity) -> bool {",
      lines: 3, sha256: "39323e186c0fec386d46a3b6653c6a1dca125938bfabf9ce3542fd1225b2d824" },
    { annotation: "opaque", item: "fn score(",
      lines: 6, sha256: "d2674d4a72cf8b211bdb7118b8ede7df917db1e780cda57c09d8e8e63a594d49" },
    { annotation: "opaque", item: "struct MonotonicClock(fn() -> u64);",
      lines: 1, sha256: "dcfbb4beca5b3496b8c06a6e4803b3bdd9ef35c83dbd13bb6b1c33d54fde9201" },
    { annotation: "opaque", item: "fn now_ns(&mut self) -> u64 {",
      lines: 3, sha256: "5e88415743d4b753871353cff6582f73702c4237c3f565129e05b0cc02972f9a" },
    { annotation: "rename(\"source_bytes\")", item: "pub source: alloc::vec::Vec<u8>,",
      lines: 1, sha256: "1ac1b335e3b2bf7784824c1249855664e8db0b6f86a4f27d4d9ba8eed87b65a9" },
    { annotation: "variants_suffix(\"Value\")", item: "enum Value {",
      lines: 5, sha256: "d93e60d4c8a3ca9c6c6ca310f908f27a6bed4fdf97b78e4b671405feeb46aaa8" },
    { annotation: "exclude", item: "pub fn complete(",
      lines: 19, sha256: "91cd00a1ea5987521f14b96e6b13fd5419c9cade03c6a6bf24e2f1f1796f0d13" },
    { annotation: "exclude", item: "pub fn complete_observed(",
      lines: 20, sha256: "ab8d96a542fe2ebb8ad16f81533fd391d552e6b2ea8161191fb914c4b73338a3" },
    { annotation: "exclude", item: "pub fn complete_ranked(",
      lines: 20, sha256: "1dfcfe6034014b4048fe4b7cd8843ef6612c176871aa9dae5604e85e6405c40e" },
    { annotation: "exclude", item: "pub fn complete_ranked_observed(",
      lines: 21, sha256: "f494c036bff600b717cab47385f2249e7e601b546f3f459cce7e4906ed845d92" },
    { annotation: "exclude", item: "pub fn complete_replayed<'observer>(",
      lines: 25, sha256: "1b14c90b8a2b33aae8d914bbfc0b9d4b9f20c6c6c73825dab73eb4351fca608b" },
  ],
  "crates/noble-contracts/src/source/declared.rs": [
    { annotation: "rename(\"source_bytes\")", item: "source: alloc::vec::Vec<u8>,",
      lines: 1, sha256: "58f4e27ef8d4038bd5ef2bef2d0d17196390eea4a6f465f4e78c32ab84c6a080" },
  ],
  "crates/noble-contracts/src/source/declared/state/register/logical.rs": [
    { annotation: "rename(\"source_bytes\")", item: "source: &'a [u8],",
      lines: 1, sha256: "7050702421e87f3b3367a803261c9d474601086ec47bc543ea9dda2409d7528d" },
  ],
  "crates/noble-contracts/src/source/editor.rs": [
    { annotation: "opaque", item: "fn debug_node(node: &Node, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {",
      lines: 9, sha256: "551503703115a14a0ef93dee6c75b762e0761e799a3b3fe6e19c10444cee99de" },
  ],
  "crates/noble-contracts/src/source/procedures.rs": [
    { annotation: "rename(\"ProcedureDiagnostic\")", item: "struct Diagnostic<'a> {",
      lines: 4, sha256: "5afe2b662fafd8bd9965e29de3a023215430c42f26f5db16f260664f675d0ef5" },
    { annotation: "exclude", item: "pub fn with_verifier(",
      lines: 8, sha256: "a9c7332b152b17ea40b7ff096975cbb69218786eb1eebc9111728f901586d73e" },
  ],
  "crates/noble-contracts/src/source/procedures/acceptance.rs": [
    { annotation: "opaque", item: "pub(super) struct OwnerVerifier(pub(super) Box<dyn EvidenceVerifier>);",
      lines: 1, sha256: "a1645fac214375694cf7ea956059b01a6faee8ef7fc99cf101e1da03bf4c1563" },
    { annotation: "opaque", item: "pub(super) fn verify(",
      lines: 7, sha256: "17cf0e4ca63064ffe6d5d9485fa697120ab72f816ddd8c87bc6a8b9eb4c75d67" },
    { annotation: "opaque", item: "fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {",
      lines: 3, sha256: "931af0af1169532ea33a9852bc17dc2e47a8de3c5f0729a667281239b82fe5b7" },
  ],
  "crates/noble-contracts/src/source/procedures/composition.rs": [
    { annotation: "rename(\"source_bytes\")", item: "pub source: Vec<u8>,",
      lines: 1, sha256: "7c29010ca96d6406da01b4c2cdc4c3c1fc3e4b9660d556ec2a00e6570c6f56ae" },
  ],
  "crates/noble-contracts/src/source/procedures/persistence.rs": [
    { annotation: "exclude", item: "pub fn load_with_verifier(",
      lines: 11, sha256: "e22871f069591f46ea2943379becdaabfb95c1ab68544f0b2a82deb55e9a0c1b" },
  ],
  "crates/noble-contracts/src/source/proof.rs": [
    { annotation: "rename(\"source_bytes\")", item: "source: &'a [u8],",
      lines: 1, sha256: "7050702421e87f3b3367a803261c9d474601086ec47bc543ea9dda2409d7528d" },
  ],
  "crates/noble-contracts/src/source/sizechange.rs": [
    { annotation: "rename(\"source_measure\")", item: "pub source: MeasureIndex,",
      lines: 1, sha256: "72a7bb809b2fe238b1125f59e4f9b6258286ce18db6e0fab30f45a209737ebf9" },
  ],
  "crates/noble-kernel/src/contracts.rs": [
    { annotation: "variants_suffix(\"Behavior\")", item: "pub enum Behavior {",
      lines: 97, sha256: "db4039e28f2c3c31f922385970681af448ca4b40e22f46469ad13949638ba1d4" },
  ],
  "crates/noble-kernel/src/shapes.rs": [
    { annotation: "variants_suffix(\"Pattern\")", item: "pub enum Pattern {",
      lines: 54, sha256: "f7fc9508b3800a321d135d076731a5875dd87663bc2e3231afc8b0b32a3bbb96" },
  ],
  "crates/noble-kernel/src/types.rs": [
    { annotation: "variants_suffix(\"Type\")", item: "pub enum Ty {",
      lines: 53, sha256: "1b8e93751128a4cfb9fba5f2caf00d1303a26884236b8192c8b5563949898936" },
  ],
  "crates/noble-kernel/src/untrusted.rs": [
    { annotation: "variants_suffix(\"Lit\")", item: "pub enum Lit {",
      lines: 10, sha256: "edc9296e1952411253c49b15ec493c14c977fe5a8002cdb35a904b682a953072" },
  ],
  "crates/noble-wasm/src/lowering.rs": [
    { annotation: "variants_suffix(\"Operation\")", item: "pub(crate) enum Operation {",
      lines: 8, sha256: "2be3576075a157a74201477e92c74e51f232201791496bb4f1be844bee13d68a" },
  ],
  "crates/noble-wasm/src/source/mod.rs": [
    { annotation: "opaque", item: "fn debug_origin(",
      lines: 60, sha256: "a1af6f7d8ea9b8fa5872606c13875afe8882c682316962d90c09b27b60a4d573" },
  ],
  "crates/noble-wasm/src/source/plan.rs": [
    { annotation: "variants_suffix(\"Action\")", item: "pub(super) enum Action {",
      lines: 38, sha256: "5f2fac6a106b0f670c8bbd804f4adf0afabcf9d8704444ba010ee5f38a6b0694" },
  ],
};

function opaqueBody(source, name) {
  const declaration = new RegExp(`(?:^|\\n)fn ${name}\\(`, 'g');
  const matches = [...source.matchAll(declaration)];
  if (matches.length !== 1) fail('OPAQUE-MODEL', `${name}: missing or duplicate helper`);
  const start = matches[0].index + (matches[0][0].startsWith('\n') ? 1 : 0);
  const opening = source.indexOf('{', start);
  if (opening < 0) fail('OPAQUE-MODEL', `${name}: missing function body`);
  let depth = 0;
  for (let at = opening; at < source.length; at++) {
    if (source[at] === '{') depth++;
    else if (source[at] === '}' && --depth === 0) return source.slice(start, at + 1);
  }
  fail('OPAQUE-MODEL', `${name}: unterminated function body`);
}
// Preserve offsets/newlines while removing Rust comments and literal contents.
// Delimiters inside strings, raw strings, characters or nested block comments
// cannot terminate a reviewed item or introduce an extraction annotation.
function rustCode(text, file) {
  const code = text.split('');
  const blank = (start, end) => {
    for (let index = start; index < end; index++) if (code[index] !== '\n') code[index] = ' ';
  };
  for (let at = 0; at < text.length;) {
    const start = at;
    if (text.startsWith('//', at)) {
      at = text.indexOf('\n', at);
      if (at < 0) at = text.length;
    } else if (text.startsWith('/*', at)) {
      at += 2;
      let depth = 1;
      while (at < text.length && depth) {
        if (text.startsWith('/*', at)) { depth++; at += 2; }
        else if (text.startsWith('*/', at)) { depth--; at += 2; }
        else at++;
      }
      if (depth) fail('SOURCE-FILTER', `${file}: unterminated Rust comment`);
    } else {
      const raw = ['r', 'b', 'c'].includes(text[at]) &&
        text.slice(at).match(/^(?:b|c)?r(#{0,255})"/);
      if (raw) {
        const closing = text.indexOf(`"${raw[1]}`, at + raw[0].length);
        if (closing < 0) fail('SOURCE-FILTER', `${file}: unterminated Rust raw string`);
        at = closing + 1 + raw[1].length;
      } else if (text[at] === '"') {
        at++;
        while (at < text.length && text[at] !== '"') at += text[at] === '\\' ? 2 : 1;
        if (at >= text.length) fail('SOURCE-FILTER', `${file}: unterminated Rust string`);
        at++;
      } else if (text[at] === "'") {
        const character = text.slice(at).match(/^'(?:\\(?:u\{[\da-fA-F_]+\}|x[\da-fA-F]{2}|[^\n])|[^'\\\n])'/u);
        if (character) at += character[0].length;
        else { at++; continue; } // A lifetime, not a character literal.
      } else { at++; continue; }
    }
    blank(start, at);
  }
  return code.join('');
}
// The annotation (with its argument) at `start`, the first line of the item it
// decorates (after further attributes, comments and blank lines), and the
// complete item text from that line through the end of its body.
export function annotatedItem(text, start, file, code = rustCode(text, file)) {
  const matched = text.slice(start).match(/^charon\s*::\s*([A-Za-z_][A-Za-z_0-9]*)(?:\s*\(\s*("[^"\n]*")\s*\))?\s*\]/);
  if (!matched) fail('SOURCE-FILTER', `${file}: unsupported Charon annotation`);
  // Normalize syntax around the argument, never bytes inside its quotes.
  const annotation = matched[1] + (matched[2] === undefined ? '' : `(${matched[2]})`);
  let at = start + matched[0].length;
  while (at < code.length) {
    if (/\s/.test(code[at])) { at++; continue; }
    if (!code.startsWith('#[', at)) break;
    at += 2;
    let depth = 1;
    while (at < code.length && depth) {
      if (code[at] === '[') depth++;
      if (code[at] === ']') depth--;
      at++;
    }
    if (depth) fail('SOURCE-FILTER', `${file}: unterminated Rust attribute`);
  }
  if (at === code.length) fail('SOURCE-FILTER', `${file}: annotation without an item`);
  const lineEnd = text.indexOf('\n', at);
  const item = text.slice(at, lineEnd < 0 ? text.length : lineEnd).trim();
  // A function ends at the brace that closes its body. Another item ends at
  // `;` or `,` outside parentheses, brackets and generic arguments (a tuple
  // struct or field), or at the brace that closes its body. The arrow in an
  // Fn bound is not a generic closing bracket.
  let depth = 0;
  let braces = 0;
  for (let index = at; index < code.length; index++) {
    const character = code[index];
    if (braces > 0) {
      if (character === '{') braces++;
      else if (character === '}' && --braces === 0) return { annotation, item, text: text.slice(at, index + 1) };
    } else if (character === '(' || character === '[' || character === '<') depth++;
    else if (character === ')' || character === ']' || character === '>' && code[index - 1] !== '-') depth--;
    else if (depth === 0 && character === '{') braces = 1;
    else if (depth === 0 && (character === ';' || character === ',')) {
      return { annotation, item, text: text.slice(at, index + 1) };
    }
  }
  fail('SOURCE-FILTER', `${file}: unterminated annotated item ${item}`);
}
const kinds = { functions: 'fun_decls', types: 'type_decls', globals: 'global_decls',
  trait_decls: 'trait_decls', trait_impls: 'trait_impls' };

export function sourcePolicy(sourceFiles, readSource) {
  for (const [file, expected] of Object.entries(inheritedOpaqueSources)) {
    equal(sourceFiles[file], expected, 'OPAQUE-SOURCE', file);
    const source = readSource(file);
    for (const [name, reviewed] of Object.entries(inheritedOpaqueBodies[file])) {
      equal(sha(opaqueBody(source, name)), reviewed, 'OPAQUE-MODEL', `${file}::${name}`);
    }
  }
  for (const file of Object.keys(sourceFiles).filter(file =>
    lanes.some(lane => file.startsWith(`crates/${lane.package}/src/`)) && file.endsWith('.rs'))) {
    const text = readSource(file);
    const code = rustCode(text, file);
    const annotations = [];
    const annotationControls = new Set();
    for (const match of code.matchAll(/#\[\s*(charon\s*::\s*([A-Za-z_][A-Za-z_0-9]*))/g)) {
      const start = match.index + match[0].indexOf('charon');
      annotationControls.add(start);
      if (match[2] === 'opaque' && inheritedOpaqueSources[file]) continue;
      if (!['rename', 'opaque', 'exclude', 'variants_suffix'].includes(match[2])) {
        fail('SOURCE-FILTER', `${file}: unreviewed Charon annotation ${match[1]}`);
      }
      annotations.push(annotatedItem(text, start, file, code));
    }
    // Every live control path must own one recognized attribute occurrence.
    // Macro-fed paths and alternative attribute spellings are not absences.
    for (const match of code.matchAll(/\bcharon\s*::/g)) {
      if (!annotationControls.delete(match.index)) {
        fail('SOURCE-FILTER', `${file}: unreviewed Charon control token`);
      }
    }
    const reviewed = reviewedAnnotations[file] ?? [];
    equal(annotations.map(({ annotation, item }) => ({ annotation, item })),
      reviewed.map(({ annotation, item }) => ({ annotation, item })),
      'SOURCE-FILTER', `${file}: unreviewed Charon annotation set`);
    for (const [index, row] of reviewed.entries()) {
      equal(annotations[index].text.split('\n').length, row.lines,
        'SOURCE-FILTER', `${file}::${row.item}: item extent`);
      equal(sha(annotations[index].text), row.sha256,
        row.annotation === 'opaque' ? 'OPAQUE-MODEL' : 'SOURCE-FILTER', `${file}::${row.item}`);
    }
    // The extraction targets the non-test library, so exact cfg(test) items
    // cannot select production bodies, and test-only derives add no production
    // impl. Every other cfg branch needs review.
    const debugDerives = [];
    const conditionalControls = new Set();
    for (const match of code.matchAll(/#\[\s*(cfg(?:_attr)?)\s*\(([^\n]*)/g)) {
      conditionalControls.add(match.index + match[0].indexOf(match[1]));
      const exact = text.slice(match.index, match.index + match[0].length);
      const following = code.slice(match.index + match[0].length).split('\n').slice(1)
        .map(line => line.trim()).find(line => line && !line.startsWith('#['));
      const allowed = match[1] === 'cfg' && match[2] === 'test)]' ||
        match[1] === 'cfg_attr' && ['crates/noble-kernel/src/words.rs',
          'crates/noble-kernel/src/untrusted.rs'].includes(file) && match[2] === 'test, derive(PartialEq, Eq))]' ||
        match[1] === 'cfg_attr' && file === 'crates/noble-kernel/src/types.rs' &&
          match[2] === 'test, derive(Eq))]' &&
          text.slice(match.index + match[0].length).startsWith('\npub enum NominalShape {') ||
        // #97: Debug for the proof-term trees is derived only for test diagnostics.
        match[1] === 'cfg_attr' && file === 'crates/noble-contracts/src/intrinsic.rs' &&
          match[2] === 'test, derive(Debug))]' &&
          ['enum Code {', 'enum Sort {', 'enum Proposition {'].includes(following);
      if (!allowed) fail('SOURCE-FILTER', `${file}: unreviewed conditional source selection`);
      if (file === 'crates/noble-contracts/src/intrinsic.rs' && exact === '#[cfg_attr(test, derive(Debug))]') {
        debugDerives.push(following);
      }
    }
    // Conditional metadata must have the same exact attribution as Charon
    // controls. Runtime cfg!(...) macros are not item-selection metadata.
    for (const match of code.matchAll(/\bcfg(?:_attr)?\s*\(/g)) {
      if (!conditionalControls.delete(match.index)) {
        fail('SOURCE-FILTER', `${file}: unreviewed conditional source-selection token`);
      }
    }
    if (file === 'crates/noble-contracts/src/intrinsic.rs') {
      equal(debugDerives, ['enum Code {', 'enum Sort {', 'enum Proposition {'],
        'SOURCE-FILTER', `${file}: test Debug derive set`);
    }
  }
}

function sourceName(file) {
  if (!file || typeof file.name?.Local !== 'string') fail('LLBC-SOURCE', 'local source name missing');
  const name = path.posix.normalize(file.name.Local);
  if (path.posix.isAbsolute(name) || name.startsWith('../')) fail('LLBC-SOURCE', name);
  return name;
}
function uniqueNames(rows, label) {
  const names = rows.map(row => row.lean_name);
  if (names.some(name => typeof name !== 'string' || !name) || new Set(names).size !== names.length) {
    fail('INVENTORY', `missing or duplicate Lean name: ${label}`);
  }
}
function isDropGlue(decl, crate) {
  const implementation = decl.src?.TraitImpl?.impl_ref?.id;
  return Number.isInteger(implementation) && crate.trait_impls[implementation]?.src === 'Destruct' &&
    decl.item_meta.name.some(part => part.Builtin?.[0] === 'DropGlue');
}
function isVTable(decl) {
  return decl?.src?.VTableInstance && decl.item_meta.name.some(part => part.Builtin?.[0] === 'VTable');
}
function directRustName(decl) {
  return decl.item_meta.name.every(part => part.Ident)
    ? decl.item_meta.name.map(part => part.Ident[0]).join('::') : null;
}

function compilerSpans(crate) {
  const spans = new Map();
  const pending = [crate];
  while (pending.length) {
    const value = pending.pop();
    if (!value || typeof value !== 'object') continue;
    if (Array.isArray(value.Value) && Number.isInteger(value.Value[0]) &&
        Number.isInteger(value.Value[1]?.data?.file_id)) {
      spans.set(value.Value[0], value.Value[1]);
    }
    pending.push(...Object.values(value));
  }
  return spans;
}

function loopOrigin(item, declaration, translation, crate, spans) {
  const parent = translation.functions.find(row => row.def_id === item.def_id &&
    row.is_local && !row.loop && row.lean_name === item.parent_lean_name);
  if (!parent) fail('OMITTED-BODY', `loop helper has no translated parent: ${item.lean_name}`);
  if (parent.rust_name !== item.rust_name || item.is_opaque ||
      !Number.isInteger(item.loop.id) || item.loop.id < 0 ||
      typeof item.loop.is_body !== 'boolean' || !Array.isArray(item.loop.pos) ||
      item.loop.pos.some(position => !Number.isInteger(position) || position < 0)) {
    fail('LOCAL-OWNER', `invalid generated loop identity: ${item.lean_name}`);
  }
  // A helper may point into matches!/other standard macros. Its compiler ID
  // still owns a real local body, and the reported span must occur in that
  // body's compiler tree; an arbitrary external source origin is not accepted.
  const pending = [declaration.body];
  while (pending.length) {
    const value = pending.pop();
    if (!value || typeof value !== 'object') continue;
    const span = value.span;
    const data = (span?.Untagged ?? span?.Value?.[1] ?? spans.get(span?.Deduplicated))?.data;
    if (data && crate.files[data.file_id]?.name?.Local === item.source.file &&
        data.beg.line === item.source.begin_line && data.end.line === item.source.end_line) return;
    pending.push(...Object.values(value));
  }
  fail('LOCAL-OWNER', `loop origin is absent from its compiler body: ${item.lean_name}`);
}

export function account(lane, llbc, translation, sources) {
  if (llbc.has_errors !== false || llbc.charon_version !== '0.1.291') fail('LLBC', `${lane.id}: failed/unreviewed Charon output`);
  const crate = llbc.translated;
  if (crate?.crate_name !== lane.crate || translation.crate !== lane.crate ||
      translation.aeneas_version !== '1238879' || translation.charon_version !== '0.1.291') {
    fail('TRANSLATOR', lane.id);
  }
  for (const option of ['start_from', 'start_from_if_exists', 'start_from_attribute', 'include', 'exclude', 'opaque', 'rustc_args', 'targets']) {
    equal(crate.options?.[option], [], 'EXTRACTION-SCOPE', `${lane.id}: ${option}`);
  }
  for (const option of ['start_from_pub', 'monomorphize', 'extract_opaque_bodies', 'skip_borrowck', 'no_typecheck', 'no_serialize']) {
    equal(crate.options?.[option], false, 'EXTRACTION-SCOPE', `${lane.id}: ${option}`);
  }
  if (crate.options?.preset !== 'Aeneas' || crate.options?.error_on_warnings !== true) fail('EXTRACTION-SCOPE', lane.id);
  // The reviewed `'static` promoted-constant caveat covers exactly this mode.
  if (crate.options?.inline_anon_consts !== true) fail('EXTRACTION-SCOPE', `${lane.id}: inline_anon_consts`);
  const targets = crate.target_information;
  if (targets?.length !== 1 || targets[0].key !== 'x86_64-unknown-linux-gnu' ||
      targets[0].value?.target_pointer_size !== 8 || targets[0].value?.is_little_endian !== true) fail('TARGET', lane.id);
  if (!Array.isArray(crate.files)) fail('LLBC-SOURCE', `${lane.id}: no source inventory`);
  const collectedFiles = {};
  for (const file of crate.files.filter(file => file?.crate_name === lane.crate)) {
    const name = sourceName(file);
    if (typeof file.contents !== 'string') fail('LLBC-SOURCE', `${lane.id}: ${name}`);
    const digest = sha(file.contents);
    // Distinct include_str! paths may normalize to one file. Bind every
    // occurrence to the same frozen bytes rather than skipping duplicates.
    equal(digest, sources[name], 'LLBC-SOURCE', `${lane.id}: source contents differ: ${name}`);
    collectedFiles[name] = digest;
  }
  const testOnlySources = {};
  for (const file of Object.keys(sources).filter(file => file.startsWith(`crates/${lane.package}/src/`) && file.endsWith('.rs'))) {
    if (collectedFiles[file]) continue;
    const review = testOnlySourceModules[file];
    const parent = review && crate.files.find(item => item?.name?.Local === review.parent && item.crate_name === lane.crate);
    if (!parent || !collectedFiles[review.parent] ||
        rustCode(parent.contents, review.parent).split(review.directive).length !== 2) {
      fail('SOURCE-COVERAGE', `${lane.id}: uncollected production source ${file}`);
    }
    testOnlySources[file] = { source_sha256: sources[file], parent: review.parent,
      parent_sha256: collectedFiles[review.parent], directive: review.directive,
      disposition: 'compiler-uncollected-exact-test-only-module' };
  }
  const inventories = {};
  const collector = {};
  const spans = compilerSpans(crate);
  const opaqueFunctions = lane.id === 'kernel' ? inheritedOpaque : reviewedOpaque[lane.id];
  const opaqueTypes = lane.id === 'kernel' ? [] : reviewedOpaqueTypes[lane.id];
  for (const [kind, field] of Object.entries(kinds)) {
    if (!Array.isArray(translation[kind]) || !Array.isArray(crate[field])) fail('INVENTORY', `${lane.id}: ${kind}`);
    uniqueNames(translation[kind], `${lane.id}/${kind}`);
    const local = crate[field].filter(item => item?.item_meta?.is_local);
    const rows = translation[kind].map(item => {
      if (!Number.isInteger(item.def_id) || typeof item.is_local !== 'boolean' || !item.source?.file ||
          typeof item.rust_name !== 'string' || typeof item.lean_file !== 'string') fail('INVENTORY', `${lane.id}: malformed ${kind}`);
      const declaration = crate[field][item.def_id];
      if (!declaration || declaration.def_id !== item.def_id || declaration.item_meta.is_local !== item.is_local) {
        fail('INVENTORY', `${lane.id}: forged ${kind} ID ${item.def_id}`);
      }
      const source = crate.files[declaration.item_meta.span?.Untagged?.data?.file_id];
      if (!source) fail('INVENTORY', `${lane.id}: missing span for ${item.rust_name}`);
      if (item.is_local) {
        const name = sourceName(source);
        if (!name.startsWith(`crates/${lane.package}/src/`)) {
          fail('LOCAL-OWNER', `${lane.id}: ${item.rust_name} is not a production body`);
        }
        if (kind === 'functions' && item.loop) loopOrigin(item, declaration, translation, crate, spans);
        else if (item.source.file !== name) fail('LOCAL-OWNER', `${lane.id}: substituted source for ${item.rust_name}`);
        if (kind === 'functions') {
          if (typeof item.is_opaque !== 'boolean') fail('OPACITY', item.rust_name);
          if (item.is_opaque !== (declaration.body === 'Opaque')) fail('OPACITY', item.rust_name);
          if (declaration.signature?.is_unsafe !== false) fail('UNSAFE', item.rust_name);
        }
        const expectedFile = kind === 'types' && opaqueTypes.includes(item.rust_name) ? 'TypesExternal_Template.lean'
          : kind === 'types' || kind === 'trait_decls' ? 'Types.lean' : 'Funs.lean';
        if (!(kind === 'functions' && item.is_opaque) && item.lean_file !== expectedFile) {
          fail('LOCAL-OWNER', `${item.rust_name}: ${item.lean_file}`);
        }
      }
      return { ...item, source: { ...item.source, file: path.posix.normalize(item.source.file) } };
    }).sort((a, b) => a.lean_name.localeCompare(b.lean_name, 'en'));
    collector[kind] = local.map(decl => {
      let translated = rows.filter(row => row.is_local && row.def_id === decl.def_id);
      let disposition = 'translated';
      if (kind === 'functions' && decl.src?.GlobalInitializer) {
        const global = crate.global_decls[decl.src.GlobalInitializer.id];
        translated = translation.globals.filter(row => row.is_local && row.def_id === decl.src.GlobalInitializer.id);
        disposition = isVTable(global) && decl.item_meta.name.some(part => part.Builtin?.[0] === 'VTable')
          ? 'compiler-generated-vtable-not-translated' : 'global-initializer-translated-with-global';
      } else if (kind === 'functions' && isDropGlue(decl, crate) || kind === 'trait_impls' && decl.src === 'Destruct') {
        // Compiler-generated destructor scaffolding is absent from the selected
        // no-drop semantics. It is accounted explicitly, never an authored-body exemption.
        disposition = 'compiler-generated-destruct-not-translated';
      } else if (kind === 'globals' && isVTable(decl)) {
        disposition = 'compiler-generated-vtable-not-translated';
      } else if (kind === 'types' && decl.kind?.Alias && !translated.length) {
        // Aliases have no independent body; the translator expands their type.
        disposition = 'type-alias-expanded';
      }
      if (!translated.length && !['compiler-generated-destruct-not-translated',
        'compiler-generated-vtable-not-translated', 'type-alias-expanded'].includes(disposition)) {
        fail('OMITTED-BODY', `${lane.id}: ${kind} ${decl.def_id} ${directRustName(decl) ?? ''}`);
      }
      // An ADT constructor has a real compiler body and a real translated
      // `.Variant.constructor` definition. It is not an omitted-body waiver.
      const constructor = kind === 'functions' && decl.src === 'AdtConstructor';
      if (kind === 'functions' && disposition === 'translated' && !translated.some(row =>
        !row.loop && row.lean_name.endsWith(
          `.${row.rust_name.split('::').at(-1)}${constructor ? '.constructor' : ''}`))) {
        fail('OMITTED-BODY', `${lane.id}: only loop helpers remain for function ${decl.def_id}`);
      }
      if (kind === 'functions' && decl.body === 'Opaque' &&
          !['compiler-generated-vtable-not-translated', 'compiler-generated-destruct-not-translated'].includes(disposition)) {
        const names = translated.map(row => row.rust_name);
        if (!names.length || names.some(name => !opaqueFunctions.includes(name))) fail('LOCAL-OPAQUE', names.join(','));
        disposition = lane.id === 'kernel' ? 'inherited-opaque-model' : 'reviewed-opaque-model';
      }
      if (kind === 'types' && decl.kind === 'Opaque') {
        const names = translated.map(row => row.rust_name);
        if (!names.length || names.some(name => !opaqueTypes.includes(name))) fail('LOCAL-OPAQUE', names.join(','));
        disposition = 'reviewed-opaque-type-model';
      }
      return { def_id: decl.def_id, source_name: decl.item_meta.name, source_span: decl.item_meta.span,
        disposition, expanded_alias: disposition === 'type-alias-expanded' ? decl.kind.Alias : null,
        lean_names: translated.map(row => row.lean_name).sort() };
    }).sort((a, b) => a.def_id - b.def_id);
    inventories[kind] = rows;
  }
  equal(inventories.functions.filter(item => item.is_local && item.is_opaque).map(item => item.rust_name).sort(),
    [...opaqueFunctions].sort(), 'LOCAL-OPAQUE', lane.id);
  equal(inventories.types.filter(item => item.is_local && item.lean_file !== 'Types.lean').map(item => item.rust_name).sort(),
    [...opaqueTypes].sort(), 'LOCAL-OPAQUE', `${lane.id} types`);
  for (const kind of ['trait_decls', 'trait_impls']) {
    if (inventories[kind].some(item => item.lean_file.includes('External'))) fail('EXTERNAL-KIND', `${lane.id}/${kind}`);
  }
  if (!collector.functions.length || !inventories.functions.some(item => item.is_local && !item.is_opaque)) fail('INVENTORY', `${lane.id}: empty`);
  return { crate: lane.crate, collected_files: collectedFiles, test_only_sources: testOnlySources,
    collector, translation: inventories };
}

const sourceRoot = (lane, type, method) => ({ lane, kind: 'functions', type, method, source: true });
export const requiredRoots = [
  { lane: 'kernel', kind: 'functions', rust: 'noble_kernel::acceptance::check' },
  ...['TextLiteral', 'Body', 'Definition', 'Submission'].map(type =>
    ({ lane: 'kernel', kind: 'types', rust: `noble_kernel::execution::${type}` })),
  { lane: 'contracts', kind: 'functions', rust: 'noble_contracts::prepare' },
  ...['new', 'prepare', 'commit'].map(method => sourceRoot('contracts', 'Session', method)),
  ...['submission', 'output', 'is_definition'].map(method => sourceRoot('contracts', 'Prepared', method)),
  ...['stage', 'diagnostic'].map(method => sourceRoot('contracts', 'Error', method)),
  { lane: 'wasm', kind: 'functions', rust: 'noble_wasm::compile' },
  ...['new', 'prepare', 'commit'].map(method => sourceRoot('wasm', 'Compiler', method)),
  sourceRoot('wasm', 'Prepared', 'wat'),
];
export function rootsFor(accounts) {
  return requiredRoots.map(required => {
    const lane = lanes.find(lane => lane.id === required.lane);
    const matches = accounts[lane.id].translation[required.kind].filter(item => item.is_local &&
      (!item.is_opaque) && (required.rust ? item.rust_name === required.rust :
        item.source.file.startsWith(`crates/${lane.package}/src/source`) &&
        item.rust_name.endsWith(`::${required.type}}::${required.method}`)) &&
      (required.kind !== 'functions' || item.lean_name.endsWith(`.${required.method ?? required.rust.split('::').at(-1)}`)));
    if (matches.length !== 1) fail('REQUIRED-ROOT', `${JSON.stringify(required)}: found ${matches.length}`);
    return { ...required, declaration: matches[0].lean_name,
      owner: `${lane.module}.${required.kind === 'types' ? 'Types' : 'Funs'}`,
      requires_acceptance: required.method === 'prepare' || required.rust === 'noble_wasm::compile' };
  });
}

export function auditPacket(accounts, retainedModels) {
  const roots = rootsFor(accounts);
  if (!Array.isArray(retainedModels) || !retainedModels.length) fail('BASELINE', 'missing historical boundary inventory');
  const inheritedNames = new Set(retainedModels.map(item => item.name));
  const declarations = [];
  const boundaries = [];
  const bridges = [];
  for (const lane of lanes) {
    const inventory = accounts[lane.id].translation;
    for (const [kind, rows] of Object.entries(inventory)) {
      for (const item of rows) {
        if (item.lean_file === 'Funs.lean' || item.lean_file === 'Types.lean') {
          const name = item.lean_name.startsWith(`${lane.crate}.`) ? item.lean_name : `${lane.crate}.${item.lean_name}`;
          declarations.push({ name, owner: `${lane.module}.${item.lean_file.slice(0, -5)}`,
            transparent: item.lean_file === 'Funs.lean', kind, local: item.is_local });
        } else if (item.lean_file.includes('External')) {
          const name = `${lane.crate}.${item.lean_name}`;
          boundaries.push({ name, lane: lane.id, kind, rust: item.rust_name, inherited_opaque: item.is_local,
            classification: inheritedNames.has(name) ? 'inherited-model' : 'newly-required-model' });
          const dependency = lanes.find(other => other.crate !== lane.crate && item.rust_name.startsWith(`${other.crate}::`));
          if (kind === 'globals' && !dependency) fail('EXTERNAL-KIND', `${lane.id}: unbound global ${item.rust_name}`);
          if (dependency && (kind === 'functions' || kind === 'globals')) {
            // An opaque foreign type hides projection-name collisions: its
            // accessor may be `index` while actual extraction emits `impl.index`.
            // The complete Rust identity, not that local Lean spelling, joins
            // exactly one non-opaque entrypoint, not its generated loop helpers,
            // in the actual dependency extraction.
            const actual = accounts[dependency.id].translation[kind].filter(other => other.is_local && !other.is_opaque && !other.loop &&
              other.lean_file === 'Funs.lean' &&
              other.rust_name === item.rust_name);
            if (actual.length !== 1) fail('BRIDGE-TARGET', `${name}: no unique actual extracted dependency`);
            bridges.push({ model: name, actual: actual[0].lean_name });
          }
        } else fail('INVENTORY', `${lane.id}: unsupported generated file ${item.lean_file}`);
      }
    }
  }
  const requested = new Set(boundaries.map(item => item.name));
  return { schema: 'm4-extraction-audit/v1', declarations, boundaries, bridges, roots,
    retained_models: retainedModels,
    newly_required_models: boundaries.filter(item => !inheritedNames.has(item.name)).map(item => item.name).sort(),
    retained_unrequested_models: retainedModels.filter(item => !requested.has(item.name)).map(item => item.name).sort() };
}

// The backend now depends on the frontend's checked WIT exports. Its external
// type module imports frontend types before elaborating backend types, so the
// pinned discriminant elaborator allocates fresh names in dependency order.
// Audit the whole transitive lane set without editing generated bytes.
export function auditPackets(packet) {
  const selectors = {
    declarations: row => lanes.find(lane =>
      [`${lane.module}.Types`, `${lane.module}.Funs`].includes(row.owner))?.id,
    boundaries: row => row.lane,
    bridges: row => lanes.find(lane => row.model.startsWith(`${lane.crate}.`))?.id,
    roots: row => row.lane,
    retained_models: row => row.lane,
  };
  const packets = Object.fromEntries(['contracts', 'wasm'].map(lane => [lane, {
    schema: packet.schema, lane,
    ...Object.fromEntries(Object.keys(selectors).map(field => [field, []])),
  }]));
  for (const [field, select] of Object.entries(selectors)) {
    for (const row of packet[field]) {
      const lane = select(row);
      if (!lanes.some(known => known.id === lane)) fail('PACKET', `unassigned ${field}: ${JSON.stringify(row)}`);
      for (const target of lane === 'kernel' || lane === 'contracts' ? ['contracts', 'wasm'] : [lane]) {
        packets[target][field].push(row);
      }
    }
  }
  return packets;
}
