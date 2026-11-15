#!/usr/bin/env node
// Opt-in issue #62 comparison. Run from the repository root with a clean, built
// foundation and an independently pinned axiom-audit-pilot package. Output goes
// to a fresh caller-owned directory, never into the source tree.
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

const root = process.cwd();
const args = process.argv.slice(2);
const arg = key => { const i = args.indexOf(key); if (i < 0 || !args[i + 1]) throw Error(`missing ${key}`); return args[i + 1]; };
const out = path.resolve(arg('--out'));
const baseline = JSON.parse(fs.readFileSync(path.resolve(arg('--baseline')), 'utf8'));
const sourceCommit = arg('--source-commit');
const project = path.join(root, 'proofs/foundation');
const pilot = path.join(root, 'proofs/axiom-audit-pilot');
const leanBin = '/nix/store/kwvw84h9r7w0q40d3cm5xgfz0rpv7yzc-noble-lean-release-4.35.0/bin';
const auditor = path.join(pilot, '.lake/packages/axiomAudit/.lake/build/bin/axiom-audit');
const compare = path.join(pilot, '.lake/build/bin/noble-axiom-compare');
const timer = '/run/current-system/sw/bin/time';
const roots = ['NobleFoundation', 'NobleContracts', 'MathlibI64', 'InteractionNets'];
const allowed = new Set(['propext', 'Classical.choice', 'Quot.sound']);
const digest = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const fileDigest = file => digest(fs.readFileSync(file));
const sorted = xs => [...xs].sort();
const same = (a, b) => JSON.stringify(sorted(a)) === JSON.stringify(sorted(b));
const invariant = (condition, message) => { if (!condition) throw Error(message); };
const simpleEnv = { HOME: process.env.HOME, PATH: `${leanBin}:/run/current-system/sw/bin:/usr/bin:/bin`,
  LANG: 'C.UTF-8', TMPDIR: out };

function run(command, argv, label, cwd, env, timeout = 1_800_000) {
  const timeFile = path.join(out, `${label}.time`);
  const result = spawnSync(timer, ['-f', 'elapsed_sec=%e rss_kib=%M', '-o', timeFile, command, ...argv],
    { cwd, env, encoding: 'utf8', maxBuffer: 128 * 1024 * 1024, timeout });
  fs.writeFileSync(path.join(out, `${label}.out`), result.stdout ?? '');
  fs.writeFileSync(path.join(out, `${label}.err`), result.stderr ?? '');
  const raw = fs.existsSync(timeFile) ? fs.readFileSync(timeFile, 'utf8') : '';
  const elapsed = raw.match(/elapsed_sec=([\d.]+)/)?.[1];
  const rss = raw.match(/rss_kib=(\d+)/)?.[1];
  return { status: result.status, signal: result.signal, error: result.error?.message ?? null,
    elapsed_sec: elapsed ? Number(elapsed) : null, rss_kib: rss ? Number(rss) : null,
    stdout_sha256: digest(result.stdout ?? ''), stderr_sha256: digest(result.stderr ?? '') };
}
function pathList(projectDir) {
  const entries = fs.readdirSync(projectDir).filter(name => name.endsWith('.lean')).sort();
  return entries.map(name => path.join(projectDir, name));
}
function sourceRoster() {
  const groups = [
    [path.join(root, 'proofs/foundation'), 'NobleFoundation'],
    [path.join(root, 'proofs/mc1'), 'NobleContracts'],
  ];
  const files = [];
  for (const [dir, name] of groups) {
    files.push(path.join(dir, `${name}.lean`));
    files.push(...pathList(path.join(dir, name)));
  }
  files.push(path.join(root, 'proofs/m3/MathlibI64.lean'));
  files.push(path.join(root, 'proofs/inets/InteractionNets.lean'));
  const modules = files.map(file => {
    const rel = path.relative(root, file);
    if (rel.startsWith('proofs/foundation/NobleFoundation/')) return `NobleFoundation.${path.basename(file, '.lean')}`;
    if (rel.startsWith('proofs/mc1/NobleContracts/')) return `NobleContracts.${path.basename(file, '.lean')}`;
    return path.basename(file, '.lean');
  });
  invariant(new Set(modules).size === modules.length, 'duplicate source module');
  for (const file of files) invariant(fs.existsSync(file), `source module absent: ${file}`);
  return { modules: sorted(modules), files: files.map(file => ({
    path: path.relative(root, file), sha256: fileDigest(file),
  })) };
}
function parseJson(label, runResult, allowError = false) {
  if (runResult.error || runResult.signal) throw Error(`${label} did not return: ${JSON.stringify(runResult)}`);
  const text = fs.readFileSync(path.join(out, `${label}.out`), 'utf8').trim();
  let data;
  try { data = JSON.parse(text); } catch { throw Error(`${label} did not emit JSON: ${text.slice(0, 1000)}`); }
  if (data.error && !allowError) throw Error(`${label}: ${data.error}`);
  return data;
}
function rowsOf(report) {
  const result = new Map();
  for (const entry of report.roots) {
    invariant(entry.count === entry.rows.length, `${entry.root}: truncated rows`);
    invariant(!result.has(entry.root), `${entry.root}: duplicate root`);
    result.set(entry.root, new Map(entry.rows.map(row => [row.name, row.axioms])));
    invariant(result.get(entry.root).size === entry.rows.length, `${entry.root}: duplicate declaration`);
  }
  return result;
}
function compareReports(candidate, stock, roster, cli) {
  const cand = rowsOf(candidate); const ref = rowsOf(stock);
  const imported = new Set(candidate.imported_modules);
  const moduleMissing = roster.modules.filter(name => !imported.has(name));
  const discrepancies = []; const rootCounts = {}; const union = new Set(); const policyViolations = [];
  for (const name of roots) {
    const c = cand.get(name), s = ref.get(name);
    invariant(c && s, `root absent from comparison: ${name}`);
    rootCounts[name] = { candidate: c.size, stock: s.size, cli: cli[name].audited };
    for (const [decl, axioms] of c) {
      const expected = s.get(decl);
      if (!expected || !same(axioms, expected)) discrepancies.push({ root: name, decl, candidate: axioms, stock: expected ?? null });
      for (const axiom of axioms) union.add(axiom);
      const bad = axioms.filter(axiom => !allowed.has(axiom));
      if (bad.length) policyViolations.push({ root: name, decl, axioms: bad });
    }
    for (const decl of s.keys()) if (!c.has(decl)) discrepancies.push({ root: name, decl, candidate: null, stock: s.get(decl) });
    if (cli[name].audited !== c.size || !same(cli[name].axiomsUsed, [...new Set([...c.values()].flat())]))
      discrepancies.push({ root: name, cli: 'count or union mismatch' });
    const cliViolations = cli[name].violations.map(row => `${row.decl}:${sorted(row.axioms).join(',')}`);
    const computed = policyViolations.filter(row => row.root === name)
      .map(row => `${row.decl}:${sorted(row.axioms).join(',')}`);
    if (!same(cliViolations, computed)) discrepancies.push({ root: name, cli: 'policy violations mismatch' });
  }
  const selected = baseline.theorems;
  const selectedMissing = [], selectedMismatches = [];
  for (const theorem of selected) {
    const actual = cand.get('NobleFoundation').get(theorem.name);
    if (!actual) selectedMissing.push(theorem.name);
    else if (!same(actual, theorem.axioms)) selectedMismatches.push({ name: theorem.name,
      candidate: actual, selected_audit: theorem.axioms });
  }
  const privateMissing = [], privateAmbiguous = [];
  for (const name of baseline.private_theorems) {
    const matches = [...cand.get('NobleFoundation').keys()]
      .filter(decl => decl === name || decl.endsWith(`.${name}`));
    if (!matches.length) privateMissing.push(name);
    if (matches.length > 1) privateAmbiguous.push({ name, matches });
  }
  return { rootCounts, source_modules: roster.modules.length, moduleMissing,
    required_roots: selected.length, selectedMissing, selectedMismatches,
    private_helpers: baseline.private_theorems.length, privateMissing, privateAmbiguous,
    candidate_stock_discrepancies: discrepancies, project_axioms_union: sorted(union),
    project_policy_violations: policyViolations };
}

// Generated only in a caller-owned output directory. These deliberately bad
// sources must never be added to Noble's proof source or inventory.
function writeControls(dir) {
  fs.mkdirSync(dir);
  fs.writeFileSync(path.join(dir, 'NobleAuditImports.lean'), `import Lean\n\nnamespace NobleAuditImports\naxiom importedCustom : True\naxiom seed : Nat\nend NobleAuditImports\n`);
  fs.writeFileSync(path.join(dir, 'NobleAuditControl.lean'), `import NobleAuditImports\n\nnamespace NobleAuditControl\nprivate theorem hidden : True := NobleAuditImports.importedCustom\ntheorem viaPrivate : True := hidden\ntheorem sorrySample : True := by sorry\ntheorem nativeSample : (2 : Nat) + 2 = 4 := by native_decide\nmutual\ninductive Even : Nat → Prop\n  | zero : Even 0\n  | step (n : Nat) : Odd n → Even (n + 1)\n  | special : Even (NobleAuditImports.seed + 1)\ninductive Odd : Nat → Prop\n  | step (n : Nat) : Even n → Odd (n + 1)\nend\nend NobleAuditControl\n`);
}
function controlChecks(cli, candidate, stock, empty, missing) {
  const c = rowsOf(candidate).get('NobleAuditControl');
  const s = rowsOf(stock).get('NobleAuditControl');
  const equal = [...c].every(([name, axs]) => same(axs, s.get(name) ?? [])) && c.size === s.size;
  const via = c.get('NobleAuditControl.viaPrivate') ?? [];
  const hidden = [...c].filter(([name]) => name.endsWith('.NobleAuditControl.hidden'));
  const sorry = c.get('NobleAuditControl.sorrySample') ?? [];
  const native = c.get('NobleAuditControl.nativeSample') ?? [];
  const cycle = [...c].filter(([name]) => /NobleAuditControl\.(Even|Odd)/.test(name));
  const checks = {
    imported_custom_refused: cli.status === 1 && !cli.data.ok
      && cli.data.violations.some(v => v.decl === 'NobleAuditControl.viaPrivate'
        && v.axioms.includes('NobleAuditImports.importedCustom')),
    private_helper_covered: hidden.length === 1 && hidden[0][1].includes('NobleAuditImports.importedCustom'),
    sorry_detected: sorry.some(ax => ax.includes('sorryAx')),
    native_detected: native.includes('Lean.ofReduceBool'),
    native_generated_detected: native.some(ax => ax.includes('._native.native_decide.ax_')),
    cyclic_declarations_compared: cycle.length >= 4 && equal,
    empty_root_refused: empty.status === 1 && empty.data?.audited === 0,
    missing_module_refused: missing.status === 2 && missing.data?.audited === 0,
  };
  return { checks, candidate_count: c.size, stock_count: s.size,
    private_helper: hidden, cyclic_rows: cycle, cli_result: cli.data,
    empty_result: empty.data,
    missing_result: { audited: missing.data?.audited, ok: missing.data?.ok,
      unknown_module_refused: /unknown module prefix 'NobleAuditDoesNotExist'/.test(missing.data?.error ?? '') } };
}

invariant(!fs.existsSync(out), 'output directory already exists');
fs.mkdirSync(out, { recursive: true });
try {
  invariant(baseline.clean_source && baseline.theorem_count === baseline.theorems.length,
    'selected foundation receipt is not a clean complete audit');
  const roster = sourceRoster();
  const sourceMismatches = Object.entries(baseline.sources_sha256)
    .filter(([name]) => name.startsWith('proofs/'))
    .filter(([name, sha]) => !fs.existsSync(path.join(root, name)) || fileDigest(path.join(root, name)) !== sha)
    .map(([name]) => name);
  invariant(sourceMismatches.length === 0, `selected proof-source bytes differ: ${sourceMismatches.join(', ')}`);
  const file = path.join(out, 'roster.txt');
  fs.writeFileSync(file, `${roster.modules.join(',')}\n${roots.join(',')}\n`);
  fs.writeFileSync(path.join(out, 'source-roster.json'), JSON.stringify(roster, null, 2));
  const lake = path.join(leanBin, 'lake');
  const built = run(lake, ['build', ...roster.modules], 'build-roster', project, simpleEnv);
  invariant(built.status === 0, `not every source module built: ${fs.readFileSync(path.join(out, 'build-roster.err'), 'utf8').slice(-2000)}`);
  const fp = spawnSync(lake, ['env', 'printenv', 'LEAN_PATH'], { cwd: project, env: simpleEnv, encoding: 'utf8' });
  const pp = spawnSync(lake, ['env', 'printenv', 'LEAN_PATH'], { cwd: pilot, env: simpleEnv, encoding: 'utf8' });
  invariant(fp.status === 0 && pp.status === 0, 'cannot resolve both pinned Lake search paths');
  const env = { ...simpleEnv, LEAN_PATH: `${pp.stdout.trim()}:${fp.stdout.trim()}` };
  const runList = {};
  for (const name of roots) {
    const label = `library-${name}`;
    const stats = run(auditor, ['--root', name, '--modules', roster.modules.join(','), '--json'], label, project, env);
    const data = parseJson(label, stats);
    runList[label] = { ...stats, audited: data.audited, ok: data.ok,
      axiomsUsed: data.axiomsUsed, violations: data.violations.length };
    runList[`data-${name}`] = data;
  }
  for (const mode of ['candidate', 'stock']) {
    const label = `per-declaration-${mode}`;
    runList[label] = run(compare, [file, mode], label, project, env);
    invariant(runList[label].status === 0, `${mode} per-declaration run failed`);
  }
  const candidate = parseJson('per-declaration-candidate', runList['per-declaration-candidate']);
  const stock = parseJson('per-declaration-stock', runList['per-declaration-stock']);
  const cli = Object.fromEntries(roots.map(name => [name, runList[`data-${name}`]]));
  const comparison = compareReports(candidate, stock, roster, cli);
  const controlsDir = path.join(out, 'controls');
  writeControls(controlsDir);
  const controlEnv = { ...env, LEAN_PATH: `${controlsDir}:${env.LEAN_PATH}` };
  for (const name of ['NobleAuditImports', 'NobleAuditControl']) {
    const label = `build-${name}`;
    runList[label] = run(path.join(leanBin, 'lean'), [path.join(controlsDir, `${name}.lean`),
      '-o', path.join(controlsDir, `${name}.olean`)], label, controlsDir, controlEnv);
    invariant(runList[label].status === 0, `control ${name} did not compile`);
  }
  const fixtureRoster = path.join(out, 'controls-roster.txt');
  fs.writeFileSync(fixtureRoster, 'NobleAuditControl,NobleAuditImports\nNobleAuditControl\n');
  const controlCli = run(auditor, ['--root', 'NobleAuditControl', '--modules', 'NobleAuditControl', '--json'],
    'control-cli', project, controlEnv);
  const controlData = parseJson('control-cli', controlCli);
  const empty = run(auditor, ['--root', 'NobleAuditAbsent', '--modules', 'NobleAuditControl', '--json'],
    'control-empty', project, controlEnv);
  const missing = run(auditor, ['--root', 'NobleAuditControl', '--modules', 'NobleAuditDoesNotExist', '--json'],
    'control-missing', project, controlEnv);
  const controlModes = {};
  for (const mode of ['candidate', 'stock']) {
    const label = `control-${mode}`;
    controlModes[mode] = run(compare, [fixtureRoster, mode], label, project, controlEnv);
    invariant(controlModes[mode].status === 0, `control ${mode} failed`);
  }
  const controls = controlChecks({ ...controlCli, data: controlData },
    parseJson('control-candidate', controlModes.candidate), parseJson('control-stock', controlModes.stock),
    { ...empty, data: parseJson('control-empty', empty) },
    { ...missing, data: parseJson('control-missing', missing, true) });
  const complete = comparison.moduleMissing.length === 0 && comparison.selectedMissing.length === 0
    && comparison.selectedMismatches.length === 0 && comparison.privateMissing.length === 0
    && comparison.privateAmbiguous.length === 0 && comparison.candidate_stock_discrepancies.length === 0
    && Object.values(comparison.rootCounts).every(row => row.candidate > 0 && row.candidate === row.stock && row.stock === row.cli)
    && Object.values(controls.checks).every(Boolean)
    && roots.every(name => cli[name].ok && runList[`library-${name}`].status === 0);
  const result = { schema: 'noble-issue62-axiom-audit-measurement/v1', source_commit: sourceCommit,
    selected_baseline_commit: baseline.source_commit, clean_selected_baseline: baseline.clean_source,
    candidate_source_revision: '0b885d88a69982cd76990b502742690b80b1a685',
    candidate_executable_sha256: fileDigest(auditor), comparator_executable_sha256: fileDigest(compare),
    lean_executable_sha256: fileDigest(path.join(leanBin, 'lean')),
    module_roster_sha256: fileDigest(path.join(out, 'source-roster.json')),
    candidate_project_axioms: cli.NobleFoundation.axiomsUsed,
    comparison, controls, performance: Object.fromEntries(Object.entries(runList)
      .filter(([key]) => !key.startsWith('data-'))),
    go: complete, decision: complete ? 'GO: experimental coverage/performance cross-check only'
      : 'NO-GO: retain the selected inventory-bound audit as deciding reference' };
  fs.writeFileSync(path.join(out, 'summary.json'), JSON.stringify(result, null, 2));
  console.log(JSON.stringify({ go: result.go, source_commit: result.source_commit,
    rootCounts: comparison.rootCounts, moduleMissing: comparison.moduleMissing.length,
    rootMismatches: comparison.selectedMismatches.length,
    privateMissing: comparison.privateMissing.length,
    candidateStockMismatches: comparison.candidate_stock_discrepancies.length,
    policyViolations: comparison.project_policy_violations.length, controls: controls.checks }));
} catch (error) {
  fs.writeFileSync(path.join(out, 'failure.json'), JSON.stringify({ error: String(error) }, null, 2));
  console.error(String(error));
  process.exitCode = 1;
}
