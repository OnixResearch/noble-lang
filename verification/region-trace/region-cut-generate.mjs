// Join verbatim pinned-Aeneas declarations to the historical M5 table proof.
// The six unchanged resources/ source files have the same bytes as that archive.
// Usage: node verification/region-trace/region-cut-generate.mjs GENERATED_DIR OUTPUT_FILE
import { readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

const [, , generated, output] = process.argv;
if (!generated || !output) throw Error('usage: region-cut-generate.mjs GENERATED_DIR OUTPUT_FILE');
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
function declaration(source, rust) {
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
const pieces = [
  ...['Refusal', 'Request', 'Range'].map(name =>
    declaration(types, `noble_kernel::region_read::${name}`)),
  declaration(funs, 'noble_kernel::region_read::MAX_REGION_BYTES'),
  declaration(funs, 'noble_kernel::region_read::preflight'),
  declaration(funs, 'noble_kernel::region_read::authorize_copy'),
];
const bridge = `-- The only new external model: Rust i64 -> usize refuses negative or out-of-range values.
-- The two other externals are the already audited M5 option equality and Result.is_err.
def Usize.Insts.CoreConvertTryFromI64TryFromIntError.try_from
    (value : Std.I64) :
    Result (core.result.Result Std.Usize core.num.error.TryFromIntError) :=
  if value.val < 0 then ok (.Err ())
  else match UScalar.tryMkOpt .Usize value.val.toNat with
    | some converted => ok (.Ok converted)
    | none => ok (.Err ())`;
const source = ['-- Aeneas-generated declarations, selected verbatim by region-cut-generate.mjs.',
  '-- The bridge below is the explicit signed-conversion external model, not a Rust translation.',
  'import M5Resources', '', 'open Aeneas Aeneas.Std Result ControlFlow Error',
  'namespace noble_kernel', '', ...pieces.slice(0, 4), bridge, ...pieces.slice(4),
  'end noble_kernel', ''].join('\n\n');
writeFileSync(output, source);
