import { createHash } from 'node:crypto';
import { readFileSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';

const root = process.cwd();
const manifest = 'verification/decoder-experiment/Cargo.toml';
const casePath = 'specs/conformance/adaptation-cases.json';
const selectionPath = 'policy/tool-selection.json';
const selection = JSON.parse(readFileSync(selectionPath));
const cases = JSON.parse(readFileSync(casePath)).cases;
const case04 = cases.find(item => item.id === 'ADAPT-04');
const case05 = cases.find(item => item.id === 'ADAPT-05');
const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const assert = (condition, message) => { if (!condition) throw Error(message); };
const files = [manifest, 'verification/decoder-experiment/src/lib.rs',
  'verification/decoder-experiment/src/main.rs',
  'verification/decoder-experiment/fixtures/escape.rs',
  'verification/decoder-experiment/gate.mjs', casePath, selectionPath,
  '.cairn/specs/safety/spec.md', '.cairn/specs/verification-toolchain/spec.md'];
const sources = Object.fromEntries(files.map(path => [path, digest(readFileSync(path))]));
const quality = selection.tool_paths.quality_rust.output;
const miri = selection.tool_paths.extraction_rust.output;
const scratch = mkdtempSync(join(process.env.HOME + '/.cache/noble-proof-tmp/', 'decoder-gate-'));
const baseEnv = {...process.env, TMPDIR: process.env.HOME + '/.cache/noble-proof-tmp', RUSTC_WRAPPER:'',
  CARGO_TARGET_DIR: join(scratch, 'target'),
  PATH:join(quality,'bin') + ':' + process.env.PATH};
const command = (executable, args, env = baseEnv) => {
  const run = spawnSync(executable, args, { cwd: root, env, encoding:'utf8', maxBuffer: 2*1024*1024 });
  if (run.error) throw run.error;
  return run;
};
const cargo = join(quality, 'bin/cargo');
const rustc = join(quality, 'bin/rustc');
const rustVersion = command(rustc, ['--version']);
const miriVersion = command(join(miri, 'bin/miri'), ['--version']);
assert(rustVersion.status === 0 && miriVersion.status === 0, 'selected compiler/Miri unavailable');
const build = command(cargo, ['build','-j','1','--offline','--manifest-path',manifest]);
assert(build.status === 0, `decoder build failed: ${build.stderr}`);
const binary = join(scratch, 'target/debug/noble-decoder-experiment');
const binarySha256 = digest(readFileSync(binary));
const cli = args => {
  const run = command(binary,args);
  assert(run.status === 0 && !run.stderr, `decoder process failed: ${run.stderr}`);
  return run.stdout.trim();
};
const refused = new Map([
  ['short-header','ShortHeader'], ['short-payload','ShortPayload'],
  ['unknown-version','Version'], ['unknown-tag','Tag'], ['reserved-bits','Reserved'],
  ['length-limit','LengthLimit'], ['trailing-byte','TrailingBytes'],
]);
const observations = [];
try {
  assert(case04?.profile === 'Decoder-Experiment' && case04.input.variants.length === 8 &&
    JSON.stringify(case04.input.start_offsets) === '[0,1]' && case04.input.max_payload_bytes === 16,
    'canonical ADAPT-04 workload changed');
  for (const row of case04.input.variants) {
    for (const offset of case04.input.start_offsets) {
      const owned = cli(['decode','owned',String(offset),row.hex]);
      const view = cli(['decode','view',String(offset),row.hex]);
      const expected = row.outcome === 'candidate' ? `candidate:${row.payload_hex}` : `reject:${refused.get(row.id)}`;
      assert(expected !== 'reject:undefined' && owned === expected && view === expected,
        `${row.id}@${offset}: owned ${owned}, view ${view}, expected ${expected}`);
      observations.push({variant:row.id, offset, owned, view});
    }
  }
  // Both boundaries are real decoder calls, not assertions about source text.
  const max16 = '0100000010000000' + 'ab'.repeat(16);
  const over17 = '0100000011000000' + 'ab'.repeat(17);
  for (const offset of [0,1]) {
    assert(cli(['decode','owned',String(offset),max16]) === 'candidate:' + 'ab'.repeat(16), '16-byte bound failed');
    assert(cli(['decode','view',String(offset),max16]) === 'candidate:' + 'ab'.repeat(16), '16-byte view failed');
    assert(cli(['decode','owned',String(offset),over17]) === 'reject:LengthLimit', '17-byte bound failed');
    assert(cli(['decode','view',String(offset),over17]) === 'reject:LengthLimit', '17-byte view failed');
  }
  assert(case05?.profile === 'Decoder-Experiment' && case05.input.variants.length === 4,
    'canonical ADAPT-05 workload changed');
  const lifetime = [];
  for (const row of case05.input.variants) {
    const outcome = cli(['lifetime',row.storage,row.event]);
    const expected = row.outcome === 'retain-616263' ? 'retain:616263' : row.outcome;
    assert(outcome === expected, `lifetime ${row.storage}/${row.event}: ${outcome}`);
    lifetime.push({storage:row.storage,event:row.event,outcome});
  }
  const compilerRefusal = command(rustc, ['--edition','2021',
    'verification/decoder-experiment/fixtures/escape.rs','--emit','metadata',
    '-o',join(scratch,'escape.rmeta')]);
  assert(compilerRefusal.status !== 0 && /error\[E0515\]/.test(compilerRefusal.stderr) &&
    /error\[E0502\]/.test(compilerRefusal.stderr), 'borrow-escape/mutation compile-fail controls not established');
  const miriRun = command(join(miri,'bin/cargo'), ['miri','test','--offline',
    '--target','x86_64-unknown-linux-gnu','--manifest-path',manifest],
    {...baseEnv, CARGO_TARGET_DIR:join(scratch,'miri-target'),
      PATH:join(miri,'bin') + ':' + process.env.PATH, MIRIFLAGS:'-Zmiri-strict-provenance'});
  assert(miriRun.status === 0 && miriRun.stdout.includes('3 passed; 0 failed'),
    `scoped strict Miri did not pass: ${miriRun.stdout} ${miriRun.stderr}`);
  const receipt = {
    schema:'noble-decoder-experiment/v1', result:'passed',
    cases:{'ADAPT-04':{result:'passed',matrix:observations, boundary_max16_and_refuse17_offsets:[0,1],
      optional_byte_view:'safe-Rust-borrowed-slice-selected', zerocopy_crate:'unselected-not-run-not-passed',
      accepted_program_created:false, unchecked_length_allocation:false},
      'ADAPT-05':{result:'passed',lifetime, immutable_marker_freezes_external_storage:false,
        use_after_invalidation:false}},
    tools:{quality_rust:rustVersion.stdout.trim(), miri:miriVersion.stdout.trim(),
      miri_target:'x86_64-unknown-linux-gnu', miri_flags:'-Zmiri-strict-provenance',
      miri_tests_passed:3, compile_fail_codes:['E0515','E0502']},
    source:{input_sha256:sources, binary_sha256:binarySha256},
    assumptions:['The safe std byte-slice view is not the unselected zerocopy crate; no crate feature, derive macro, native layout or unsafe cast enters this experiment.',
      'External storage with callback mutation or suspension is refused before a borrowed view escapes; safe Rust borrowing protects only Rust-owned immutable borrows, not arbitrary foreign memory.',
      'Decoded bytes remain untrusted candidate data. This experiment never constructs accepted Noble programs or grants effect, resource, proof or host authority; no canonical encoding or Wasm correspondence is claimed.'],
  };
  const text = JSON.stringify(receipt, null, 2) + '\n';
  if (process.argv.includes('--record')) {
    // The prepromotion receipt is created once, never overwritten by a later
    // run against postpromotion case/status documents.
    writeFileSync('verification/decoder-experiment/acceptance.json', text, {flag:'wx'});
  }
  process.stdout.write(text);
} finally {
  rmSync(scratch, {recursive:true,force:true});
}
