#!/usr/bin/env node
// WORKER-06 acceptance gate.
//
// Builds the production `noble` binary from a clean checkout and probes every
// declared admission limit category of the pure Worker Program v2 candidate
// subset at its boundary: an otherwise valid candidate exactly at the limit
// must be admitted and one unit over must be exhausted. Source candidates go
// through `noble worker program check` (admission only, never run); the
// memory category concerns package decoding and goes through
// `noble worker program import`. For each category the gate names the limit
// that binds a valid candidate and the unit by which "one over" is measured;
// where the limit is reached by a search, the search itself runs through the
// CLI so the boundary is the binary's, not an estimate.
//
// Usage (inside the pinned devShell):
//   node verification/worker06/gate.mjs --out NEW_DIRECTORY
import assert from 'node:assert/strict';
import { execFileSync, spawnSync } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = fs.realpathSync(path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..'));
const here = path.join(root, 'verification/worker06');
const SCOPE = ['crates', 'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml',
  'verification/worker06/gate.mjs', 'verification/worker06/fixtures'];
const CASE_FILE = 'specs/conformance/worker-cases.json';
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const hash = file => sha(fs.readFileSync(file));
const SOURCE_BYTES = 65_536;
const PACKAGE_MEMORY = 1_048_576;
const DIAGNOSTIC_BYTES = 512;

function argument(name) {
  const index = process.argv.indexOf(name);
  return index > 0 ? process.argv[index + 1] : undefined;
}

function git(...args) {
  return execFileSync('git', args, { cwd: root, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 });
}

function sourceSnapshot() {
  const dirty = git('status', '--porcelain', '--untracked-files=all', '--', ...SCOPE);
  assert.equal(dirty, '', `source scope is not clean:\n${dirty}`);
  const files = git('ls-files', '-z', '--', ...SCOPE).split('\0').filter(Boolean).sort();
  const snapshot = {};
  for (const file of files) snapshot[file] = { sha256: hash(path.join(root, file)) };
  return snapshot;
}

function caseProjection() {
  const cases = JSON.parse(fs.readFileSync(path.join(root, CASE_FILE), 'utf8')).cases;
  const selected = cases.filter(c => c.id === 'WORKER-06');
  assert.equal(selected.length, 1, 'expected exactly one WORKER-06 case');
  const { id, profile, kind, requirements, input, expected } = selected[0];
  return { id, profile, kind, requirements, input, expected };
}

function buildNoble() {
  const build = spawnSync('cargo', ['build', '--locked', '-p', 'noble-cli', '--bin', 'noble',
    '--message-format=json-render-diagnostics'], { cwd: root, encoding: 'utf8', maxBuffer: 256 * 1024 * 1024 });
  assert.equal(build.status, 0, `cargo build failed:\n${build.stderr}`);
  const artifacts = build.stdout.split('\n').filter(Boolean).map(line => JSON.parse(line))
    .filter(m => m.reason === 'compiler-artifact' && m.target?.name === 'noble' && m.executable);
  assert.equal(artifacts.length, 1, 'expected one noble executable artifact');
  return { executable: artifacts[0].executable, profile: artifacts[0].profile?.opt_level === '0' ? 'dev' : 'release' };
}

// --- candidate families -------------------------------------------------
const module = body => `module W@1 [ export f def f [ ${body} ] ]`;
const nested = depth => (depth === 0 ? '' : `[ ${nested(depth - 1)} ]`);
const families = {
  'decoded-nodes': k => module(`${'unit drop '.repeat(k)}1 +`),
  nesting: k => module(`${nested(k)} drop 1 +`),
  'dependency-traversal': k => {
    // A heap-shaped closure of k helpers under the root; leaves add one.
    const defs = [];
    for (let i = 0; i < k; i++) {
      const kids = [2 * i + 1, 2 * i + 2].filter(c => c < k).map(c => `d${c}`);
      defs.push(`def d${i} [ ${kids.length ? kids.join(' ') : '1 +'} ]`);
    }
    return `module W@1 [ ${defs.reverse().join(' ')} export f def f [ d0 ] ]`;
  },
};
// 255 constructors from seven `dup pair` steps; each `nil cons` adds one.
const typed = lists => module(`unit${' dup pair'.repeat(7)}${' nil cons'.repeat(lists)} drop 1 +`);
const branches = (units, ints, extra) => module(
  `dup 0 = [ ${'unit '.repeat(units)}${'1 '.repeat(ints)}] [ ${'unit '.repeat(units)}${'1 '.repeat(ints)}${extra} ] if drop 1 +`);

// Mirrors the importer's pre-decode scan: three units per byte, 128 per token.
function memoryEstimate(bytes) {
  let quoted = false, escaped = false, tokens = 0;
  for (const byte of bytes) {
    if (quoted) {
      if (escaped) escaped = false;
      else if (byte === 0x5c) escaped = true;
      else if (byte === 0x22) quoted = false;
      continue;
    }
    if (byte === 0x22) { quoted = true; tokens += 1; } else if (byte === 0x7b || byte === 0x5b || byte === 0x2c) tokens += 1;
  }
  return bytes.length * 3 + tokens * 128;
}

// --- runner ------------------------------------------------------------
function runner(noble, out) {
  let serial = 0;
  const runs = [];
  const invoke = (label, argv, input, { keep = true, limitMemory = false } = {}) => {
    const workdir = fs.mkdtempSync(path.join(os.tmpdir(), 'noble-worker06-'));
    const file = argv.find(a => a === '@INPUT') ? path.join(workdir, `input-${serial++}`) : null;
    if (file) fs.writeFileSync(file, input);
    const args = argv.map(a => (a === '@INPUT' ? file : a));
    const child = limitMemory
      ? spawnSync('bash', ['-c', 'ulimit -v 524288 && exec "$0" "$@"', noble, ...args], { cwd: workdir, encoding: 'buffer', timeout: 300_000 })
      : spawnSync(noble, args, { cwd: workdir, encoding: 'buffer', timeout: 300_000 });
    assert.equal(child.error, undefined, `${label}: ${child.error}`);
    if (file) fs.rmSync(file);
    const leftovers = fs.readdirSync(workdir).length;
    fs.rmSync(workdir, { recursive: true, force: true });
    const report = JSON.parse(child.stdout.toString('utf8'));
    const row = {
      label, exit: child.status, files_left_in_working_directory: leftovers,
      input_sha256: input === null ? null : sha(input), input_bytes: input === null ? null : input.length,
      stdout_sha256: sha(child.stdout),
      observed: {
        outcome: report.outcome, diagnostic: report.diagnostic ?? null,
        diagnostic_truncated: report.diagnostic_truncated ?? null, diagnostic_bytes: report.diagnostic_bytes ?? null,
        program_published: report.selected_graph_blake3 !== undefined,
        guest_requests: report.guest_requests, protected_operations: report.protected_operations,
        checking_work_shared: report.checking_work_shared ?? null, checking_work_left: report.checking_work_left ?? null,
        package_memory_estimate: report.package_memory_estimate ?? null,
      },
    };
    if (keep) {
      fs.writeFileSync(path.join(out, 'raw', `${label}.stdout.json`), child.stdout);
      if (input !== null) fs.writeFileSync(path.join(out, 'candidates', `${label}`), input);
      runs.push(row);
    }
    return row;
  };
  const check = (label, source, work, options) => invoke(label,
    ['worker', 'program', 'check', '@INPUT', '--name', 'f', '--effect-ceiling', 'pure', ...(work ? ['--work', String(work)] : [])],
    Buffer.from(source), options);
  const importPackage = (label, bytes, options) => invoke(label,
    ['worker', 'program', 'import', '@INPUT', '--effect-ceiling', 'pure'], bytes, options);
  return { runs, check, importPackage, invoke };
}

// Largest k in [lo, hi) a family admits, found through the CLI.
function largestAdmitted(check, family, lo, hi) {
  const admits = k => check('search', family(k), undefined, { keep: false }).observed.outcome === 'ready';
  assert.ok(admits(lo), 'family lower bound is not admitted');
  assert.ok(!admits(hi), 'family upper bound is admitted');
  while (hi - lo > 1) {
    const mid = (lo + hi) >> 1;
    if (admits(mid)) lo = mid; else hi = mid;
  }
  return lo;
}

function main() {
  const destination = argument('--out');
  assert.ok(destination, 'usage: node verification/worker06/gate.mjs --out NEW_DIRECTORY');
  const out = path.resolve(destination);
  assert.ok(!fs.existsSync(out), `output directory already exists: ${out}`);
  const sourceFiles = sourceSnapshot();
  const projection = caseProjection();
  assert.deepEqual(projection.input.cases_per_category, ['at-limit', 'one-unit-over-limit']);
  const build = buildNoble();
  fs.mkdirSync(path.join(out, 'raw'), { recursive: true });
  fs.mkdirSync(path.join(out, 'candidates'));
  const { runs, check, importPackage, invoke } = runner(build.executable, out);
  const categories = {};
  const pair = (category, limit, unit, atLimit, overLimit) => {
    categories[category] = { limit, unit, at_limit: atLimit.label, one_unit_over: overLimit.label };
  };

  // input-bytes: source file bytes, refused before the file is read.
  const base = module('1 +');
  const padded = bytes => `# ${'x'.repeat(bytes - base.length - 3)}\n${base}`;
  pair('input-bytes', `${SOURCE_BYTES} source bytes`, 'one byte',
    check('input-bytes-at-limit', padded(SOURCE_BYTES)), check('input-bytes-over', padded(SOURCE_BYTES + 1)));

  for (const [category, limit, unit, lo, hi] of [
    ['decoded-nodes', 'the definition\'s share of the 4096-node source budget', 'one `unit drop` pair', 1, 2000],
    ['nesting', 'quotation depth 32', 'one quotation level', 1, 64],
    ['dependency-traversal', 'the dependency closure each definition\'s share of the node budget can still check', 'one more helper definition in the closure', 1, 128],
  ]) {
    const k = largestAdmitted(check, families[category], lo, hi);
    pair(category, limit, unit, check(`${category}-at-limit`, families[category](k)),
      check(`${category}-over`, families[category](k + 1)));
    categories[category].at_limit_parameter = k;
  }

  pair('type-stack-size', '256 type constructors', 'one constructor',
    check('type-stack-size-at-limit', typed(1)), check('type-stack-size-over', typed(2)));

  // checking-work: the host-selected request work for one fixed candidate.
  const fixed = module('1 +');
  let [short, enough] = [1, 50_000_000];
  const admitsWork = work => check('search', fixed, work, { keep: false }).observed.outcome === 'ready';
  assert.ok(admitsWork(enough) && !admitsWork(short));
  while (enough - short > 1) {
    const mid = Math.floor((short + enough) / 2);
    if (admitsWork(mid)) enough = mid; else short = mid;
  }
  pair('checking-work', 'host-selected request work (--work)', 'one work unit',
    check('checking-work-at-limit', fixed, enough), check('checking-work-over', fixed, enough - 1));
  categories['checking-work'].at_limit_parameter = enough;

  // memory: the package decoding memory estimate, checked before decoding.
  const memoryPackage = fs.readFileSync(path.join(here, 'fixtures/memory-at-limit.json'));
  assert.equal(memoryEstimate(memoryPackage), PACKAGE_MEMORY, 'memory fixture is not exactly at the limit');
  const document = JSON.parse(memoryPackage.toString('utf8'));
  document.source_utf8 = document.source_utf8.replace('# ', '# x');
  const overPackage = Buffer.from(JSON.stringify(document));
  pair('memory', `${PACKAGE_MEMORY} estimated decoding units (3 per byte, 128 per token)`, 'one package byte (three estimate units)',
    importPackage('memory-at-limit', memoryPackage), importPackage('memory-over', overPackage));
  categories.memory.over_estimate = memoryEstimate(overPackage);

  // diagnostic-output: invalid candidates whose diagnostic is exactly the
  // 512-byte serialized bound and one byte over it.
  const diagnosticAt = check('diagnostic-at-limit', branches(11, 3, 'true'));
  const diagnosticOver = check('diagnostic-over', branches(12, 2, '1'));
  categories['diagnostic-output'] = { limit: `${DIAGNOSTIC_BYTES} serialized diagnostic bytes`, unit: 'one serialized byte',
    at_limit: diagnosticAt.label, one_unit_over: diagnosticOver.label };

  // Inputs far over the byte limits are refused under a 512 MiB address
  // space: neither file is read before its limit is checked.
  const sparse = path.join(os.tmpdir(), `noble-worker06-sparse-${process.pid}`);
  fs.closeSync(fs.openSync(sparse, 'w'));
  fs.truncateSync(sparse, 4 * 1024 * 1024 * 1024);
  const allocationControls = [
    invoke('sparse-source', ['worker', 'program', 'check', sparse, '--name', 'f', '--effect-ceiling', 'pure'], null, { limitMemory: true }),
    invoke('sparse-package', ['worker', 'program', 'import', sparse, '--effect-ceiling', 'pure'], null, { limitMemory: true }),
  ];
  fs.rmSync(sparse);

  // --- evaluation --------------------------------------------------------
  const byLabel = Object.fromEntries(runs.map(r => [r.label, r]));
  const failures = [];
  const admitted = r => r.exit === 0 && r.observed.outcome === 'ready' && r.observed.program_published;
  const exhausted = r => r.exit === 5 && r.observed.outcome === 'exhausted' && !r.observed.program_published;
  for (const [category, entry] of Object.entries(categories)) {
    if (category === 'diagnostic-output') continue;
    if (!admitted(byLabel[entry.at_limit])) failures.push(`${category}: at-limit candidate was not admitted`);
    if (!exhausted(byLabel[entry.one_unit_over])) failures.push(`${category}: one-over candidate was not exhausted`);
  }
  assert.deepEqual(Object.keys(categories).sort(), [...projection.input.limit_categories].sort(),
    'gate categories differ from the canonical WORKER-06 categories');
  const d = [diagnosticAt.observed, diagnosticOver.observed];
  if (!(diagnosticAt.exit === 2 && d[0].outcome === 'invalid-input' && d[0].diagnostic_bytes === DIAGNOSTIC_BYTES
    && d[0].diagnostic_truncated === false)) failures.push('diagnostic at the bound was not reported whole');
  if (!(diagnosticOver.exit === 2 && d[1].outcome === 'invalid-input' && d[1].diagnostic_bytes === DIAGNOSTIC_BYTES + 1
    && d[1].diagnostic_truncated === true && Buffer.byteLength(JSON.stringify(d[1].diagnostic)) <= DIAGNOSTIC_BYTES)) {
    failures.push('diagnostic one byte over the bound was not truncated with its outcome kept');
  }
  if (byLabel['memory-at-limit']?.observed.package_memory_estimate !== PACKAGE_MEMORY) failures.push('memory at-limit estimate was not reported exactly');
  const work = byLabel['checking-work-at-limit'].observed;
  if (!(work.checking_work_left !== null && work.checking_work_left <= work.checking_work_shared)) failures.push('shared checking work was not reported within its share');
  if (!allocationControls.every(r => r.exit === 5 && r.observed.outcome === 'exhausted')) failures.push('a far-over input was not refused before it was read');
  const everything = [...runs, ...allocationControls];
  const observed = {
    stage: 'bounded-admission',
    outcome: failures.length === 0 ? 'each-boundary-matches' : 'boundary-mismatch',
    at_limit_valid_candidate: Object.entries(categories).filter(([c]) => c !== 'diagnostic-output')
      .every(([, e]) => admitted(byLabel[e.at_limit])) ? 'accepted' : 'not-accepted',
    over_limit_valid_candidate: Object.entries(categories).filter(([c]) => c !== 'diagnostic-output')
      .every(([, e]) => exhausted(byLabel[e.one_unit_over])) ? 'exhausted' : 'not-exhausted',
    diagnostic_limit_on_invalid_candidate: d[1].diagnostic_truncated && d[1].outcome === 'invalid-input'
      ? 'invalid-with-truncated-diagnostic' : 'other',
    guest_requests: everything.reduce((sum, r) => sum + (r.observed.guest_requests ?? 0), 0),
    protected_operations: everything.reduce((sum, r) => sum + (r.observed.protected_operations ?? 0), 0),
    over_limit_partial_programs: everything.filter(r => r.observed.outcome !== 'ready'
      && (r.observed.program_published || r.files_left_in_working_directory > 0)).length,
    nested_budget_reset: !(admitted(byLabel['checking-work-at-limit']) && exhausted(byLabel['checking-work-over'])),
    allocation_before_limit_check: !allocationControls.every(r => r.exit === 5),
    exhaustion_proves_invalidity: !(exhausted(byLabel['checking-work-over']) && admitted(byLabel['checking-work-at-limit'])),
  };
  for (const [field, value] of Object.entries(projection.expected)) {
    if (observed[field] !== value) failures.push(`${field}: expected ${JSON.stringify(value)}, observed ${JSON.stringify(observed[field])}`);
  }

  const receipt = {
    schema: 'noble-worker06-acceptance/v1',
    case: 'WORKER-06',
    result: failures.length === 0 ? 'passed' : 'failed',
    failures,
    tested_commit: git('rev-parse', 'HEAD').trim(),
    source_scope: SCOPE,
    source_revision: `sha256:${sha(JSON.stringify(sourceFiles))}`,
    source_file_count: Object.keys(sourceFiles).length,
    case_projection_sha256: sha(JSON.stringify(projection)),
    case_projection: projection,
    binary: { name: 'noble', profile: build.profile, sha256: hash(build.executable),
      build: 'cargo build --locked -p noble-cli --bin noble' },
    fixtures: Object.fromEntries(fs.readdirSync(path.join(here, 'fixtures')).sort()
      .map(name => [name, hash(path.join(here, 'fixtures', name))])),
    candidate_subset: 'pure Worker Program v2 candidates: sources admitted by `noble worker program check`, packages by `noble worker program import`',
    categories,
    observed,
    observation_methods: {
      nested_budget_reset: 'The same candidate is admitted at the smallest request work and exhausted one unit below it; the module check, closure walk and invocation check draw every internal meter from one shared checking share, which the admitted run reports with what it left. rust-tests also runs shared_request_work_spans_nested_checking_stages.',
      allocation_before_limit_check: 'A 4 GiB sparse source and package are refused as exhausted while the process may map only 512 MiB.',
      exhaustion_proves_invalidity: 'The candidate exhausted one unit below the smallest request work is admitted at it.',
      over_limit_partial_programs: 'Refused runs that published a program graph or left a file in their fresh working directory.',
    },
    runs,
    allocation_controls: allocationControls,
    source_files: sourceFiles,
    non_claims: [
      'Each category is shown at the limit that binds a valid candidate on this path; nominal limits that no valid candidate can reach (package nodes and nesting, the 128-definition cap, the 256-value stack) are not exercised.',
      'Worker v1 (`noble worker typed`) is closed and is not this candidate subset.',
      'No proof obligation is discharged.',
    ],
  };
  fs.writeFileSync(path.join(out, 'acceptance.json'), `${JSON.stringify(receipt, null, 2)}\n`);
  console.log(JSON.stringify({ result: receipt.result, failures, observed,
    source_revision: receipt.source_revision, binary_sha256: receipt.binary.sha256 }));
  process.exitCode = failures.length === 0 ? 0 : 1;
}

main();
