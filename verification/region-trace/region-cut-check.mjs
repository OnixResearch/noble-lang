// One real synchronous Host::read_region decision cut: fresh selected Charon/Aeneas
// translation, exact checked-in join, pinned Lean build and transitive theorem audit.
// Output is an immutable, commit-bound receipt in a new directory outside this tree.
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdirSync, readFileSync, realpathSync, writeFileSync } from 'node:fs';
import { userInfo } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(fileURLToPath(import.meta.url), '../../..');
const selection = JSON.parse(readFileSync(join(root, 'policy/tool-selection.json')));
const profile = JSON.parse(readFileSync(join(root, 'verification/host-locality/model-inputs.json')));
const rust = join(selection.tool_paths.extraction_rust.output, 'bin');
const lean = join(selection.tool_paths.lean.output, 'bin');
const node = join(selection.tool_paths.node.output, 'bin/node');
const charon = join(selection.tool_paths.charon.output, 'bin/charon');
const aeneas = join(selection.tool_paths.aeneas.output, 'bin/aeneas');
const git = realpathSync('/run/current-system/sw/bin/git');
const { homedir, username } = userInfo();
const argv = process.argv.slice(2);
const dirty = argv.includes('--allow-dirty');
if (dirty) argv.splice(argv.indexOf('--allow-dirty'), 1);
if (argv.length !== 2 || argv[0] !== '--output') throw Error('usage: region-cut-check.mjs [--allow-dirty] --output NEW_EXTERNAL_DIRECTORY');
const output = resolve(argv[1]);
const requireThat = (yes, description) => { if (!yes) throw Error(description); };
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const fileHash = path => hash(readFileSync(path));
const sourceHash = path => fileHash(join(root, path));
requireThat(output !== root && !output.startsWith(`${root}/`), 'output must be outside source');
requireThat(realpathSync(process.execPath) === realpathSync(node), 'selected Node required');
requireThat(process.env.CARGO_HOME && process.env.CARGO_TARGET_DIR, 'offline Cargo home and bounded target required');
const env = {
  HOME: homedir, USER: username, LOGNAME: username, LANG: 'C.UTF-8', CI: '1',
  CARGO_HOME: process.env.CARGO_HOME, CARGO_TARGET_DIR: process.env.CARGO_TARGET_DIR,
  TMPDIR: process.env.TMPDIR || dirname(output),
  CC: process.env.CC || 'cc',
  PATH: [rust, lean, dirname(node), dirname(git), '/run/current-system/sw/bin',
    '/usr/bin', '/bin'].join(':'),
};
mkdirSync(output);
const commands = [];
function run(executable, args, cwd = root) {
  const reply = spawnSync(executable, args, {
    cwd, env, encoding: 'utf8', timeout: 3_600_000, maxBuffer: 32 * 1024 * 1024,
  });
  commands.push({ executable, executable_sha256: fileHash(executable), args, cwd,
    exit: reply.status, stdout_sha256: hash(reply.stdout || ''), stderr_sha256: hash(reply.stderr || '') });
  requireThat(reply.status === 0 && !reply.signal && !reply.error,
    `${executable} ${args.join(' ')} failed: ${reply.status} ${reply.signal ?? ''}\n${reply.stderr ?? ''}\n${reply.stdout ?? ''}`);
  return reply.stdout || '';
}
function gitState() {
  return { commit: run(git, ['rev-parse', 'HEAD']).trim(),
    tree: run(git, ['rev-parse', 'HEAD^{tree}']).trim(),
    status: run(git, ['status', '--porcelain=v1', '--untracked-files=all']).trim() };
}
const initial = gitState();
requireThat(dirty || initial.status === '', `dirty source: ${initial.status}`);
const controls = JSON.parse(run(node, ['verification/region-trace/region-cut-controls.mjs']));
requireThat(controls.result === 'passed' && controls.controls.length === 5 &&
  controls.controls.every(row => row.observed === 'refused'), 'wrapper mutation controls failed');
requireThat(profile.schema === 'noble-host-locality-model-inputs/v1', 'M5 resource profile changed');
for (const [path, expected] of Object.entries(profile.historical_resource_source_sha256)) {
  requireThat(sourceHash(path) === expected, `M5 extracted table bytes changed: ${path}`);
}
requireThat(sourceHash(profile.source) === profile.archive_sha256, 'historical generated archive changed');
const generated = join(output, 'generated');
mkdirSync(generated);
const llbc = join(output, 'region-cut.llbc');
const roots = ['noble_kernel::region_read::preflight',
  'noble_kernel::region_read::authorize_copy', 'noble_kernel::resources::table::access'];
run(charon, ['cargo', '--preset=aeneas', '--error-on-warnings', '--inline-anon-consts',
  ...roots.flatMap(name => ['--start-from', name]), '--dest-file', llbc, '--',
  '--manifest-path', 'crates/noble-kernel/Cargo.toml', '--lib', '--all-features',
  '--target', 'x86_64-unknown-linux-gnu', '--locked', '--offline']);
run(aeneas, ['-backend', 'lean', '-abort-on-error', '-warnings-as-errors', '-no-progress-bar',
  '-emit-json', '-split-files', '-gen-lib-entry', '-all-computable', '-dest', generated, llbc]);
const joined = join(output, 'RegionCutExtracted.lean');
run(node, ['verification/region-trace/region-cut-generate.mjs', generated, joined]);
const source = 'proofs/m5/RegionCutExtracted.lean';
requireThat(readFileSync(joined).equals(readFileSync(join(root, source))),
  'checked-in extracted cut is not verbatim fresh pinned Aeneas output');
const freshFuns = readFileSync(join(generated, 'Funs.lean'), 'utf8');
const archivedFuns = readFileSync(join(root, 'proofs/m3/NobleKernel/Funs.lean'), 'utf8');
function generatedBody(text, name) {
  const anchor = `/-- [${name}]`;
  const start = text.indexOf(anchor);
  requireThat(start >= 0 && text.indexOf(anchor, start + 1) < 0,
    `missing or duplicate generated table function: ${name}`);
  const end = text.indexOf('\n/-- [', start + anchor.length);
  requireThat(end >= 0, `unterminated generated function: ${name}`);
  return text.slice(start, end).trimEnd();
}
const reusedTableBodies = [
  'noble_kernel::resources::table::access::{noble_kernel::resources::table::Table}::begin',
  'noble_kernel::resources::table::access::{noble_kernel::resources::table::Table}::native_access',
  'noble_kernel::resources::table::access::{noble_kernel::resources::table::Table}::complete',
  'noble_kernel::resources::table::{noble_kernel::resources::table::Table}::commit',
];
for (const subject of reusedTableBodies) {
  requireThat(generatedBody(freshFuns, subject) === generatedBody(archivedFuns, subject),
    `the compiled historical M5 table body differs from fresh selected Rust extraction: ${subject}`);
}
const proof = 'proofs/m5/RegionCutProof.lean';
const inventory = JSON.parse(readFileSync(join(root, 'proofs/m5/region-cut-inventory.json')));
const names = inventory.theorems.map(row => row.name);
requireThat(names.length > 0 && new Set(names).size === names.length &&
  names.every(name => name.startsWith('RegionCutProof.')), 'invalid theorem inventory');
const proofCode = readFileSync(join(root, proof), 'utf8').replace(/\/-[\s\S]*?-\//g, '').replace(/--.*$/gm, '');
requireThat(!/\b(?:sorry|admit|native_decide|implemented_by|unsafe|opaque|partial|run_cmd|run_elab)\b|^\s*axiom\s/m.test(proofCode),
  'proof escape hatch refused');
const declared = [...proofCode.matchAll(/^theorem\s+(\w+)/gm)].map(match => `RegionCutProof.${match[1]}`);
requireThat(declared.length === names.length && names.every(name => declared.includes(name)),
  `cut theorem inventory differs from declarations: ${declared}`);
const project = join(root, 'proofs/m5');
run(join(lean, 'lake'), ['build', 'RegionCutProof'], project);
const auditFile = join(output, 'Audit.lean');
const lines = ['import RegionCutProof', ''];
const positions = {};
for (const name of names) {
  positions[lines.length + 1] = { name, field: 'statement' }; lines.push(`#check @${name}`);
  positions[lines.length + 1] = { name, field: 'axioms' }; lines.push(`#print axioms ${name}`);
}
writeFileSync(auditFile, lines.join('\n') + '\n');
const printed = run(join(lean, 'lake'), ['env', 'lean', '--json', auditFile], project);
const records = {};
for (const line of printed.split('\n').filter(Boolean)) {
  const msg = JSON.parse(line);
  requireThat(msg.severity === 'information' && positions[msg.pos.line], `Lean diagnostic: ${line}`);
  const { name, field } = positions[msg.pos.line];
  (records[name] ??= {})[field] = msg.data.trim();
}
const standardAxioms = ['propext', 'Classical.choice', 'Quot.sound'];
const theorems = inventory.theorems.map(row => {
  const printed = records[row.name];
  requireThat(printed?.statement && printed?.axioms, `missing audit for ${row.name}`);
  const match = printed.axioms.match(/depends on axioms: \[([\s\S]*)\]/);
  const axioms = /does not depend on any axioms/.test(printed.axioms) ? [] :
    (requireThat(match, `unparsed axioms for ${row.name}`), match[1].split(',').map(x => x.trim()).filter(Boolean));
  requireThat(JSON.stringify(row.allowed_standard_axioms) === JSON.stringify(standardAxioms) &&
    axioms.every(name => standardAxioms.includes(name)), `unreviewed axiom for ${row.name}: ${axioms}`);
  return { ...row, statement: printed.statement.replace(/\s+/g, ' '), axioms, status: 'proved' };
});
const final = gitState();
requireThat(final.commit === initial.commit && final.tree === initial.tree && final.status === initial.status,
  'source changed during extraction or proof');
const paths = [source, proof, 'proofs/m5/HostLocality.lean', 'proofs/m5/M5Resources.lean',
  'proofs/m3/NobleKernel/Funs.lean',
  'proofs/m5/region-cut-inventory.json', 'proofs/m5/lakefile.toml',
  'crates/noble-kernel/src/region_read.rs', 'crates/noble-cli/src/component/bounded_region.rs',
  'verification/host-locality/model-inputs.json',
  'verification/region-trace/region-cut-generate.mjs', 'verification/region-trace/region-cut-check.mjs',
  'verification/region-trace/region-cut-controls.mjs'];
const receipt = { schema: 'noble-region-cut-extraction/v1', result: 'passed',
  source_commit: initial.commit, source_tree: initial.tree, clean_source: initial.status === '',
  selected_roots: roots, generated_sha256: Object.fromEntries(
    ['Funs.lean', 'Types.lean', 'translation.json'].map(name => [name, fileHash(join(generated, name))])),
  llbc_sha256: fileHash(llbc), source_sha256: Object.fromEntries(paths.map(path => [path, sourceHash(path)])),
  reused_table_functions_checked_byte_for_byte: reusedTableBodies,
  inherited_m5_resource_source_sha256: profile.historical_resource_source_sha256,
  external_models: {
    'core.result.Result.is_err': 'existing pinned Aeneas M5 external',
    'core.option.Option.Insts.CoreCmpPartialEqOption.eq': 'existing pinned Aeneas M5 external',
    'Usize.Insts.CoreConvertTryFromI64TryFromIntError.try_from':
      'explicit width-sensitive total model in RegionCutExtracted.lean; conversion theorems audited below',
  }, theorems, controls, commands,
  scope: 'Two extracted Rust decision functions and the existing M5 table functions, not the imperative wrapper, allocator, emitted component, Canonical ABI or Wasmtime semantics.' };
writeFileSync(join(output, 'receipt.json'), JSON.stringify(receipt, null, 2) + '\n');
process.stdout.write(`REGION-CUT: ${theorems.length} implementation theorems; pinned extraction and strict axioms accepted\n`);
