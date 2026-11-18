// Select the reviewed declarations of a fresh pinned Charon/Aeneas bounded extraction.
// The selection is never compiled: the proof project builds the checked-in whole-crate
// subject, and this text must equal the corresponding declarations of that subject byte
// for byte. The check reads both sides with this one selector, so neither is hand-edited.
// Usage: node verification/region-trace/region-cut-generate.mjs GENERATED_DIR OUTPUT_FILE
import { readFileSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

// The declarations this lane proves things about, in the order the receipt records them.
export const selectedSubjects = [
  { file: 'Types.lean', rust: 'noble_kernel::region_read::Refusal' },
  { file: 'Types.lean', rust: 'noble_kernel::region_read::Request' },
  { file: 'Types.lean', rust: 'noble_kernel::region_read::Range' },
  { file: 'Funs.lean', rust: 'noble_kernel::region_read::MAX_REGION_BYTES' },
  { file: 'Funs.lean', rust: 'noble_kernel::region_read::preflight' },
  { file: 'Funs.lean', rust: 'noble_kernel::region_read::authorize_copy' },
];

// The table functions this lane reuses; they are already checked-in M5 extracted bodies.
export const reusedTableSubjects = [
  'noble_kernel::resources::table::access::{noble_kernel::resources::table::Table}::begin',
  'noble_kernel::resources::table::access::{noble_kernel::resources::table::Table}::native_access',
  'noble_kernel::resources::table::access::{noble_kernel::resources::table::Table}::complete',
  'noble_kernel::resources::table::{noble_kernel::resources::table::Table}::commit',
];

export function declaration(source, rust) {
  const anchor = `/-- [${rust}]`;
  const first = source.indexOf(anchor);
  if (first < 0 || source.indexOf(anchor, first + 1) >= 0) {
    throw Error(`absent or duplicate generated declaration: ${rust}`);
  }
  const next = source.indexOf('\n/-- [', first + anchor.length);
  const end = next < 0 ? source.indexOf('\nend noble_kernel', first) : next;
  if (end < 0) throw Error(`unterminated declaration: ${rust}`);
  return source.slice(first, end).trimEnd();
}

export function selectedDeclarationText(sources) {
  const header = [
    '-- Aeneas-generated declarations selected verbatim from the pinned bounded region-read',
    '-- extraction. This file is not compiled: RegionCutProof proves the checked-in whole-crate',
    '-- subject, and this text must equal that subject byte for byte.',
  ].join('\n');
  const pieces = selectedSubjects.map(({ file, rust }) => declaration(sources[file], rust));
  return [header, ...pieces].join('\n\n') + '\n';
}

function main(argv) {
  const [generated, output] = argv;
  if (!generated || !output) {
    throw Error('usage: region-cut-generate.mjs GENERATED_DIR OUTPUT_FILE');
  }
  const types = readFileSync(join(generated, 'Types.lean'), 'utf8');
  const funs = readFileSync(join(generated, 'Funs.lean'), 'utf8');
  const metadata = JSON.parse(readFileSync(join(generated, 'translation.json'), 'utf8'));
  const roots = ['noble_kernel.region_read.preflight', 'noble_kernel.region_read.authorize_copy'];
  for (const root of roots) {
    const matches = metadata.functions.filter(row => row.lean_name === root && row.is_local &&
      !row.is_opaque && row.lean_file === 'Funs.lean' && row.source.file ===
        'crates/noble-kernel/src/region/read.rs');
    if (matches.length !== 1) throw Error(`missing actual translated body: ${root}`);
  }
  const external = metadata.functions.filter(row => !row.is_local).map(row => row.lean_name).sort();
  const expected = ['Usize.Insts.CoreConvertTryFromI64TryFromIntError.try_from',
    'core.option.Option.Insts.CoreCmpPartialEqOption.eq', 'core.result.Result.is_err'].sort();
  if (JSON.stringify(external) !== JSON.stringify(expected)) {
    throw Error(`unreviewed external dependencies: ${external}`);
  }
  writeFileSync(output, selectedDeclarationText({ 'Types.lean': types, 'Funs.lean': funs }));
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main(process.argv.slice(2));
}
