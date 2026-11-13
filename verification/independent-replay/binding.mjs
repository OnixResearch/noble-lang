#!/usr/bin/env node
// Issue #62 statement and closure binding for the independent-kernel pilot.
//
// refusals.mjs binds requested roots by name. This lane also binds each root's exact statement and
// audits its dependency closure before a checker verdict can count:
// - statements.lean prints every root's canonical statement from Noble's compiled foundation
//   environment, through Lean's own Expr and never from a lean4export file. Those statements must
//   equal the digests committed in statements.json.
// - The export-side statement is rendered from the export's own records in the same canonical form
//   and must equal that digest, so a valid theorem checked under the root's name does not pass.
// - The lane walks every constant the roots reference, transitively, and refuses an export that
//   does not declare all of them before the checker runs. An accepted run records the closure's
//   Merkle digest over every reachable declaration's type, value and recursor rules.
// A run is accepted only when the export stage succeeds, every root is declared, the closure is
// complete, every statement is bound, the checker accepts a positive number of declarations, and
// every declared axiom is one of the three standard axioms. Pilot evidence, not a gate.
import { spawnSync } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  closureDigest, closureOf, dropDeclaration, exportStatement, parseExport, renameTheorem,
} from './binding-core.mjs';
import { pinGuard } from './pins.mjs';
import { extractorPath, leanStatements } from './statements-extract.mjs';

const argv = process.argv.slice(2);
const value = (name) => {
  const index = argv.indexOf(name);
  if (index < 0 || index + 1 >= argv.length) throw new Error(`missing ${name}`);
  return argv[index + 1];
};
const sha256 = (data) => crypto.createHash('sha256').update(data).digest('hex');
const fileSha256 = (file) => sha256(fs.readFileSync(file));
const here = path.dirname(fileURLToPath(import.meta.url));
const leanBin = path.resolve(value('--lean-bin'));
const foundation = path.resolve(value('--foundation'));
const childEnv = {
  HOME: process.env.HOME ?? '/', LANG: 'C.UTF-8',
  PATH: [leanBin, '/run/current-system/sw/bin', '/usr/bin', '/bin'].join(':'),
  TMPDIR: process.env.TMPDIR,
};
const extractStatements = (roots) => leanStatements({ leanBin, project: foundation, roots });

if (argv.includes('--print-statements')) {
  const roots = value('--roots').split(',');
  const { statements } = extractStatements(roots);
  const claims = roots.map((root) => ({
    root, statement_sha256: sha256(statements.get(root)), statement: statements.get(root),
  }));
  console.log(JSON.stringify(claims, null, 2));
  process.exit(0);
}

const nanoda = path.resolve(value('--nanoda'));
const exporter = path.resolve(value('--exporter'));
const large = path.resolve(value('--large-export'));
const small = path.resolve(value('--small-export'));
const smallRetainedAt = argv.includes('--small-retained-at') ? value('--small-retained-at') : null;
const manifestPath = path.resolve(value('--manifest'));
const work = path.resolve(value('--work'));
const output = path.resolve(value('--output'));
const pins = {
  nanoda: value('--nanoda-sha256'),
  exporter: value('--exporter-sha256'),
  lean: value('--lean-sha256'),
  large: value('--large-sha256'),
  small: value('--small-sha256'),
};
// Refuse before any process runs when a pinned input changed.
pinGuard([
  { role: 'checker', file: nanoda, sha256: pins.nanoda, pinClass: 'tool-pin-mismatch' },
  { role: 'exporter', file: exporter, sha256: pins.exporter, pinClass: 'tool-pin-mismatch' },
  { role: 'lean', file: path.join(leanBin, 'lean'), sha256: pins.lean, pinClass: 'tool-pin-mismatch' },
  { role: 'large export', file: large, sha256: pins.large, pinClass: 'export-pin-mismatch' },
  { role: 'small export', file: small, sha256: pins.small, pinClass: 'export-pin-mismatch' },
]);
if (fs.existsSync(work)) throw new Error(`work directory exists: ${work}`);
fs.mkdirSync(work, { recursive: true });

const STANDARD_AXIOMS = ['propext', 'Classical.choice', 'Quot.sound'];
const SMALL_ROOT = 'NobleFoundation.LinkBoundary.vertical_simulation_compose';
const LARGE_ROOT = 'NobleFoundation.toInt_add';
const FRESH_ROOT = 'Nat.add_comm';
const DROPPED_THEOREM = '_private.NobleFoundation.LinkBoundary.0.NobleFoundation.LinkBoundary.forward_simulation_steps';
const DROPPED_DEFINITION = 'NobleFoundation.LinkBoundary.ForwardSimulation';
const manifest = JSON.parse(fs.readFileSync(manifestPath, 'utf8'));

function classifyChecker(result) {
  if (result.error?.code === 'ETIMEDOUT') return 'timeout';
  const stdout = result.stdout ?? '';
  const stderr = result.stderr ?? '';
  const checked = stdout.match(/Checked (\d+) declarations with no errors/u);
  if (result.status === 0 && checked) return Number(checked[1]) > 0 ? 'accepted' : 'empty-check';
  if (/declares unpermitted axiom/u.test(stderr)) return 'forbidden-axiom';
  if (result.signal === 'SIGABRT' || result.status === 134) return 'resource-exhausted';
  if (/EOF while parsing|expected value|missing field|unknown variant|invalid type/u.test(stderr)) return 'parse-refused';
  if (result.status === 101) return 'rejected';
  return 'refused-other';
}

function runChecker(id, exportPath) {
  const config = path.join(work, `${id}.nanoda.json`);
  fs.writeFileSync(config, `${JSON.stringify({
    use_stdin: false, export_file_path: exportPath, permitted_axioms: STANDARD_AXIOMS,
    unpermitted_axiom_hard_error: true, num_threads: 1, nat_extension: true, string_extension: true,
    print_axioms: false, print_success_message: true,
  }, null, 2)}\n`);
  const started = process.hrtime.bigint();
  const result = spawnSync(nanoda, [config], {
    env: childEnv, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024, timeout: 1_800_000, killSignal: 'SIGKILL',
  });
  return {
    exit_code: result.status,
    signal: result.signal,
    elapsed_ms: Number((process.hrtime.bigint() - started) / 1_000_000n),
    classification: classifyChecker(result),
    stdout: (result.stdout ?? '').trim().slice(0, 300),
    stderr_excerpt: (result.stderr ?? '').trim().slice(0, 300),
  };
}

function runExporter(id, args) {
  const exportPath = path.join(work, `${id}.ndjson`);
  const descriptor = fs.openSync(exportPath, 'w');
  const result = spawnSync(exporter, args, {
    env: childEnv, encoding: 'utf8', stdio: ['ignore', descriptor, 'pipe'],
    maxBuffer: 64 * 1024 * 1024, timeout: 1_800_000, killSignal: 'SIGKILL',
  });
  fs.closeSync(descriptor);
  return {
    exportPath,
    record: {
      argv: args, exit_code: result.status, signal: result.signal, panic: /PANIC/u.test(result.stderr ?? ''),
      stderr_excerpt: (result.stderr ?? '').split('\nbacktrace:')[0].trim().slice(0, 300),
    },
  };
}

function replay(control) {
  const entry = { id: control.id, changes: control.changes, expected: control.expected, roots: control.roots };
  let exportPath = control.file;
  const refusals = [];
  if (control.export) {
    const exported = runExporter(control.id, control.export);
    entry.export_stage = exported.record;
    exportPath = exported.exportPath;
    if (exported.record.exit_code !== 0 || exported.record.signal) refusals.push(['export-failure', `exporter exited ${exported.record.exit_code ?? exported.record.signal}`]);
    if (exported.record.panic) refusals.push(['export-failure', 'exporter panicked']);
  }
  const parsed = parseExport(exportPath);
  entry.path = exportPath;
  entry.export = {
    sha256: fileSha256(exportPath), size_bytes: fs.statSync(exportPath).size, records: parsed.records,
    format_version: parsed.format, declared: parsed.decls.size, axioms: parsed.axioms,
  };
  const absent = control.roots.filter((root) => !parsed.decls.has(root));
  if (absent.length > 0) refusals.push(['export-failure', `roots absent from the export: ${absent.join(', ')}`]);
  if (refusals.length === 0) {
    const { reachable, missing } = closureOf(parsed, control.roots);
    entry.closure = {
      reachable: reachable.size, missing: [...missing].sort(),
      unreferenced_declared: [...parsed.decls.keys()].filter((name) => !reachable.has(name)).sort(),
    };
    if (missing.size > 0) refusals.push(['incomplete-closure', `referenced but not declared: ${[...missing].sort().join(', ')}`]);
    else entry.closure.digest = closureDigest(parsed, reachable);
    entry.statements = control.roots.map((root) => {
      const text = exportStatement(parsed, root);
      const claim = manifest.claims.find((item) => item.root === root);
      return { root, export_statement_sha256: sha256(text), pinned_sha256: claim?.statement_sha256 ?? null };
    });
    for (const statement of entry.statements) {
      if (statement.pinned_sha256 === null) refusals.push(['unbound-claim', `${statement.root} has no pinned statement`]);
      else if (statement.export_statement_sha256 !== statement.pinned_sha256) {
        refusals.push(['claim-mismatch', `${statement.root}: exported statement differs from the pinned statement`]);
      }
    }
  }
  if (refusals.length > 0) {
    entry.verdict = refusals[0][0];
    entry.reasons = refusals.map(([, reason]) => reason);
    // What a pipeline that trusts root names and checker status would have seen on the same export.
    if (control.counterfactual) entry.checker_only_view = runChecker(`${control.id}-checker-only`, exportPath);
    return entry;
  }
  entry.checker = runChecker(control.id, exportPath);
  const foreign = parsed.axioms.filter((axiom) => !STANDARD_AXIOMS.includes(axiom));
  entry.verdict = entry.checker.classification === 'accepted' && foreign.length > 0 ? 'forbidden-axiom' : entry.checker.classification;
  return entry;
}

// The manifest must agree with the compiled environment, whatever the exports say.
const claimRoots = manifest.claims.map((claim) => claim.root);
const extraction = extractStatements(claimRoots);
const compare = (claims) => claims.map((claim) => ({
  root: claim.root,
  environment_sha256: sha256(extraction.statements.get(claim.root)),
  pinned_sha256: claim.statement_sha256,
  matches: sha256(extraction.statements.get(claim.root)) === claim.statement_sha256,
}));
const manifestCheck = compare(manifest.claims);
const swapped = manifest.claims.map((claim, index, all) => ({
  ...claim, statement_sha256: all[(index + 1) % all.length].statement_sha256,
}));
const staleManifest = compare(swapped);

const controls = [
  { id: 'bound-large', changes: 'none: the pinned toInt_add export', baseline: true,
    file: large, roots: [LARGE_ROOT], expected: 'accepted' },
  { id: 'bound-small', changes: 'none: the retained export', baseline: true,
    file: small, roots: [SMALL_ROOT], expected: 'accepted' },
  { id: 'bound-exported', changes: 'none: a fresh Init export of Nat.add_comm', baseline: true,
    export: ['Init', '--', FRESH_ROOT], roots: [FRESH_ROOT], expected: 'accepted' },
  { id: 'dropped-theorem', changes: `the retained export without the declaration of ${DROPPED_THEOREM}`, counterfactual: true,
    file: dropDeclaration('dropped-theorem', small, DROPPED_THEOREM, work), roots: [SMALL_ROOT], expected: 'incomplete-closure' },
  { id: 'dropped-definition', changes: `the retained export without the declaration of ${DROPPED_DEFINITION}`, counterfactual: true,
    file: dropDeclaration('dropped-definition', small, DROPPED_DEFINITION, work), roots: [SMALL_ROOT], expected: 'incomplete-closure' },
];
const entries = controls.map((control) => ({ ...replay(control), baseline: Boolean(control.baseline) }));
const byId = (id) => entries.find((entry) => entry.id === id);
const substituted = {
  id: 'substituted-valid-claim',
  changes: `the fresh ${FRESH_ROOT} export with its theorem renamed ${LARGE_ROOT}`, counterfactual: true,
  file: renameTheorem('substituted-valid-claim', byId('bound-exported').path, FRESH_ROOT, LARGE_ROOT, work),
  roots: [LARGE_ROOT], expected: 'claim-mismatch',
};
entries.push({ ...replay(substituted), baseline: false });

const checks = {
  manifest_matches_the_compiled_environment: manifestCheck.every((row) => row.matches),
  stale_manifest_is_detected: staleManifest.every((row) => !row.matches),
  every_control_in_its_expected_class: entries.every((entry) => entry.verdict === entry.expected),
  baselines_accepted: entries.filter((entry) => entry.baseline).every((entry) => entry.verdict === 'accepted'),
  no_other_control_accepted: entries.filter((entry) => !entry.baseline).every((entry) => entry.verdict !== 'accepted'),
  substituted_claim_passes_names_and_checker: byId('substituted-valid-claim')?.checker_only_view?.classification === 'accepted'
    && byId('substituted-valid-claim')?.closure?.missing.length === 0,
  dropped_declarations_are_exactly_the_missing_names:
    JSON.stringify(byId('dropped-theorem')?.closure?.missing) === JSON.stringify([DROPPED_THEOREM])
    && JSON.stringify(byId('dropped-definition')?.closure?.missing) === JSON.stringify([DROPPED_DEFINITION]),
};
const holds = Object.values(checks).every(Boolean);
for (const entry of entries) delete entry.path;

const host = spawnSync('uname', ['-srm'], { encoding: 'utf8' }).stdout.trim();
const source = spawnSync('git', ['rev-parse', 'HEAD'], { cwd: process.cwd(), encoding: 'utf8' });
if (source.status !== 0 || source.stdout.trim() === '') {
  throw new Error(`cannot identify receipt source commit: ${source.stderr.trim()}`);
}
const receipt = {
  schema: 'noble-independent-replay-binding/v1',
  measured_on: new Date().toISOString().slice(0, 10),
  host,
  source_commit: source.stdout.trim(),
  assertion: 'every root is bound to the statement the compiled foundation environment gives it, and to a complete dependency closure, before the checker verdict counts: a valid theorem checked under a root name and an export missing a referenced declaration are both refused, and a manifest that disagrees with the environment is detected',
  holds,
  checks,
  harness: {
    binding: { path: 'verification/independent-replay/binding.mjs', sha256: fileSha256(fileURLToPath(import.meta.url)) },
    extractor: { path: 'verification/independent-replay/statements.lean', sha256: fileSha256(extractorPath) },
    manifest: { path: 'verification/independent-replay/statements.json', sha256: fileSha256(manifestPath) },
  },
  acceptance_rule: [
    'exporter exit status zero and no exporter panic',
    'every requested root declared in the export',
    'every constant the roots reference, transitively, declared in the export',
    'every root statement rendered from the export equal to its pinned statement digest',
    'checker exit status zero with a positive number of checked declarations',
    `every declared axiom in ${STANDARD_AXIOMS.join(', ')}`,
  ],
  statement_source: {
    extraction_elapsed_ms: extraction.elapsed_ms,
    manifest_check: manifestCheck,
    stale_manifest_control: staleManifest,
  },
  tools: {
    nanoda: { binary_sha256: pins.nanoda, num_threads: 1 },
    lean4export: { binary_sha256: pins.exporter },
    lean: { binary_sha256: pins.lean },
  },
  inputs: {
    large: { root: LARGE_ROOT, sha256: pins.large, retained: false },
    small: {
      root: SMALL_ROOT, sha256: pins.small, retained: smallRetainedAt !== null,
      path: smallRetainedAt,
    },
  },
  controls: entries,
  non_claims: [
    'Pilot evidence on one host with locally built pinned tools; no CI, flake, policy or gate change.',
    'The expected statements come from Lean reading the compiled foundation environment: they bind the export to what Lean elaborated, not to the source text or its meaning.',
    'The closure audit follows explicit constant references; declarations the kernel needs only through literal extensions are not required by it.',
    'No claim about nanoda or lean4export correctness, completeness or equivalence to the Lean kernel.',
    'A refusal class names why a run did not succeed; it is never a mathematical refutation of the subject.',
  ],
};
fs.mkdirSync(path.dirname(output), { recursive: true });
fs.writeFileSync(output, `${JSON.stringify(receipt, null, 2)}\n`);
console.log(JSON.stringify({ holds, checks, verdicts: Object.fromEntries(entries.map((entry) => [entry.id, entry.verdict])) }));
process.exitCode = holds ? 0 : 1;
