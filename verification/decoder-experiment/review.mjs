import { createHash } from 'node:crypto';
import { readFileSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';

const sha256 = bytes => createHash('sha256').update(bytes).digest('hex');
const cases = JSON.parse(readFileSync('specs/conformance/adaptation-cases.json')).cases;
const item = cases.find(row => row.id === 'ADAPT-07');
if (item?.kind !== 'review' || item.profile !== 'Implementation-Policy' ||
    item.input.canonical_schema_selected !== false || item.expected.dependency_selected !== false) {
  throw Error('ADAPT-07 proposal changed; independent review required');
}
const files = ['verification/decoder-experiment/native-layout-review.rs',
  'verification/decoder-experiment/review.mjs','specs/conformance/adaptation-cases.json',
  '.cairn/specs/safety/spec.md', 'crates/noble-contracts/src/companion/subject.rs',
  'Cargo.lock','policy/tool-selection.json'];
const inputSha256 = Object.fromEntries(files.map(path => [path, sha256(readFileSync(path))]));
const pin = JSON.parse(readFileSync('policy/tool-selection.json')).tool_paths.quality_rust.output;
const root = mkdtempSync(join(process.env.HOME + '/.cache/noble-proof-tmp/', 'native-layout-review-'));
try {
  const output = join(root, 'layout');
  const compiler = spawnSync(join(pin,'bin/rustc'), ['--edition','2021',
    'verification/decoder-experiment/native-layout-review.rs','-o',output],
    {encoding:'utf8'});
  if (compiler.status !== 0) throw Error('layout probe compilation failed: ' + compiler.stderr);
  const probe = spawnSync(output,[],{encoding:'utf8'});
  if (probe.status !== 0 || !probe.stdout.includes('native-layout-cannot-be-canonical-id')) {
    throw Error('native layout order probe failed');
  }
  const first = probe.stdout.match(/tag-then-length:\((\d+), (\d+), (\d+), (\d+)\)/);
  const second = probe.stdout.match(/length-then-tag:\((\d+), (\d+), (\d+), (\d+)\)/);
  if (!first || !second || first[3] === second[3] || first[4] === second[4]) {
    throw Error('same semantic fields unexpectedly had identical native offsets');
  }
  // The portable experiment decoder uses explicit version/tag/reserved/LE
  // length and exact bounds; native field offsets cannot replace its schema.
  // Existing companion identities additionally fold semantic recipe events,
  // signatures/captures and a domain revision; no IntoBytes output is its API.
  const receipt = {schema:'noble-adapt07-review/v1',case:'ADAPT-07',result:'passed',kind:'review',
    decision:'reject-native-IntoBytes-as-ProgramValueId',
    claim:'The proposed native-layout IntoBytes-to-program-identity shortcut is rejected. An executed safe Rust native ABI probe showed different field offsets for the same logical fields, while the declared 8-byte decoder explicitly fixes LE/order/version/limits and existing companion subject identity folds semantic signatures, recipe observations and captures. This does not establish canonical Noble encoding or select zerocopy.',
    observation:{layout_first:first.slice(1).map(Number),layout_second:second.slice(1).map(Number),
      native_layout_not_canonical:true,canonical_schema_selected:false,
      dependency_selected:false,accepted_program_created:false},
    source:{input_sha256:inputSha256,binary_sha256:sha256(readFileSync(output)),
      toolchain:spawnSync(join(pin,'bin/rustc'),['--version'],{encoding:'utf8'}).stdout.trim()},
    assumptions:['The probe demonstrates native Rust layout variability under declared repr(C) field order; it does not exercise a zerocopy IntoBytes derive or prove every target ABI.',
      'The rejection is a design-policy review under S-DECODE-03, not a Noble runtime/decoder proof or an adopted canonical program format.']};
  const text = JSON.stringify(receipt,null,2) + '\n';
  if (process.argv.includes('--record')) {
    writeFileSync('verification/decoder-experiment/adapt07-review.json',text,{flag:'wx'});
  }
  process.stdout.write(text);
} finally { rmSync(root,{recursive:true,force:true}); }
