#!/usr/bin/env node
// Opt-in, source-bound local equivalents of compiler-lab #217's selected batch-nine cases.
// The upstream issue has not published shared fixture bytes. Local success NEVER authorizes
// consumption of the upstream corpus. Keep all exports, logs, configs and receipts outside git.
import crypto from 'node:crypto';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { closureOf, exportStatement, parseExport } from './binding-core.mjs';
import { foreignAxioms, inspectExport, STANDARD_AXIOMS } from './ndjson.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, '../..');
const fixtureDir = path.join(here, 'compiler217');
const fixture = path.join(fixtureDir, 'Compiler217.lean');
const casesPath = path.join(fixtureDir, 'cases.json');
const checkersPath = path.join(here, 'checkers.json');
const criteriaPath = path.join(here, 'batch9-criteria.json');
const sha256 = (bytes) => crypto.createHash('sha256').update(bytes).digest('hex');
const fileSha256 = (file) => sha256(fs.readFileSync(file));
const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);
const argv = process.argv.slice(2);
const option = (name) => {
  const at = argv.indexOf(name);
  if (at < 0 || at + 1 >= argv.length) throw new AdmissionError('configuration-error', `expected ${name} VALUE`);
  return argv[at + 1];
};

class AdmissionError extends Error {
  constructor(classification, message) {
    super(message);
    this.classification = classification;
  }
}
const requireThat = (condition, classification, message) => {
  if (!condition) throw new AdmissionError(classification, message);
};

const work = path.resolve(option('--work'));
requireThat(argv.length === 2 && path.isAbsolute(option('--work')), 'configuration-error',
  'only --work <fresh absolute directory outside the repository> is accepted');
requireThat(work !== root && !work.startsWith(`${root}${path.sep}`), 'configuration-error',
  'the work directory must be outside the repository');
const resolvedWorkParent = fs.realpathSync(path.dirname(work));
requireThat(resolvedWorkParent !== root && !resolvedWorkParent.startsWith(`${root}${path.sep}`),
  'configuration-error', 'work parent resolves inside the repository');
const manifest = JSON.parse(fs.readFileSync(casesPath, 'utf8'));
const checkersPin = JSON.parse(fs.readFileSync(checkersPath, 'utf8'));
const criteria = JSON.parse(fs.readFileSync(criteriaPath, 'utf8'));
const leanBin = path.join(checkersPin.toolchain.output, 'bin');
const leanLib = path.join(checkersPin.toolchain.output, 'lib/lean');
const binaries = {
  lean: path.join(leanBin, 'lean'),
  exporter: path.join(leanBin, 'leanexport'),
  official: path.join(leanBin, 'leanchecker'),
  nanoda: path.join(leanBin, 'nanoda_bin'),
  time: '/run/current-system/sw/bin/time',
  prlimit: '/run/current-system/sw/bin/prlimit',
};
const nanodaPin = checkersPin.checkers.find((entry) => entry.id === 'nanoda');
const bounds = manifest.bounds;
const NATIVE = /\._native\.native_decide\.ax_[0-9]+_[0-9]+$/u;

function git(args) {
  const result = spawnSync('git', args, { cwd: root, encoding: 'utf8', timeout: 10_000 });
  requireThat(result.status === 0 && !result.error, 'source-unavailable',
    `git ${args.join(' ')} failed: ${result.stderr ?? result.error?.message ?? ''}`);
  return result.stdout.trim();
}

function sourceCommit() {
  requireThat(git(['status', '--porcelain']) === '', 'source-dirty',
    'commit all fixture, script, criteria and other worktree changes before running');
  return git(['rev-parse', 'HEAD']);
}

function bindInputs() {
  requireThat(manifest.schema === 'noble-compiler217-local-equivalents/v1'
    && manifest.provenance.kind === 'local-equivalents-not-upstream-corpus'
    && manifest.provenance.shared_corpus_consumption === 'NO-GO'
    && manifest.provenance.upstream_shared_corpus_revision === null
    && manifest.provenance.upstream_shared_fixture_sha256 === null, 'configuration-error',
  'local fixture provenance must not masquerade as an unpublished upstream shared corpus');
  requireThat(criteria.frozen_before_measurement === true
    && criteria.items.compiler217.reference === manifest.issue
    && criteria.selected_lean === manifest.lean
    && criteria.selected_export_format === manifest.export_format
    && same(criteria.allowed_axioms, STANDARD_AXIOMS)
    && same(manifest.allowed_axioms, STANDARD_AXIOMS), 'configuration-error',
  'frozen batch-nine profile, issue, or three-axiom policy differs from the manifest');
  requireThat(checkersPin.toolchain.lean === manifest.lean
    && checkersPin.export_format.version === manifest.export_format
    && nanodaPin?.run?.kind === 'nanoda-config', 'configuration-error',
  'pinned exporter/Nanoda profile differs from the manifest');
  requireThat(manifest.source === 'compiler217/Compiler217.lean'
    && fileSha256(fixture) === manifest.source_sha256, 'source-pin-mismatch',
  'local fixture bytes differ from the frozen manifest');
  requireThat(manifest.resource_bound_amendment?.prior_source_commit
      === 'e08ae2c7076513c52ece8a2038e30c98fbda2d5f'
    && manifest.resource_bound_amendment?.prior_receipt_sha256
      === 'eebc588438596c0de029c4cb49aa7bad186ee63482688ce9416936daa8be4986'
    && manifest.resource_bound_amendment?.prior_address_space_bytes === 2147483648
    && manifest.resource_bound_amendment?.new_address_space_bytes === bounds.address_space_bytes
    && bounds.address_space_bytes === 17179869184, 'configuration-error',
  'resource ceiling amendment must retain its exact pre-export refusal receipt');
  requireThat(manifest.warning_only_amendment?.prior_source_commit
      === 'e5ded0849a1c46cf46396061dbe4d718bed0a80f'
    && manifest.warning_only_amendment?.prior_receipt_sha256
      === 'b9604039262aeb5b4528b8f53a8507813cbd3d4c13c8263c950a5e5fbad7c8bb',
  'configuration-error', 'warning-only amendment must retain its exact pre-export refusal receipt');
  requireThat(manifest.encoding_amendment?.prior_source_commit
      === 'f45b7197166438c7699e4cf4baeca07ef5381722'
    && manifest.encoding_amendment?.prior_receipt_sha256
      === '1d5668a0de53990a6ee0bfe78d1182ae74b2184433cc91363adb2f38cae57a52',
  'configuration-error', 'NDJSON encoding amendment must retain its exact checker-decline receipt');
  requireThat(manifest.cases.length === 7
    && new Set(manifest.cases.map((entry) => entry.id)).size === 7
    && same(manifest.cases.map((entry) => `${entry.category}:${entry.expected.lane}`),
      ['function-eta:accepted', 'function-eta:kernel-rejection',
        'structure-eta:accepted', 'structure-eta:kernel-rejection',
        'native-axiom-control:accepted', 'native-axiom-control:forbidden-axiom',
        'native-axiom-control:forbidden-axiom']), 'configuration-error',
  'seven frozen case identities and expected classes are required');
  const expectedPins = {
    lean: checkersPin.toolchain.lean_sha256,
    exporter: checkersPin.toolchain.leanexport_sha256,
    official: manifest.official_checker_sha256,
    nanoda: nanodaPin.sha256,
  };
  for (const [name, digest] of Object.entries(expectedPins)) {
    requireThat(fs.existsSync(binaries[name]) && fileSha256(binaries[name]) === digest,
      'tool-pin-mismatch', `${name} executable is missing or does not match its pinned SHA-256`);
  }
  for (const name of ['time', 'prlimit']) {
    requireThat(fs.existsSync(binaries[name]), 'tool-unavailable', `${name} executable is missing`);
  }
  for (const [name, amount] of Object.entries(bounds)) {
    requireThat(Number.isSafeInteger(amount) && amount > 0, 'configuration-error',
      `invalid resource bound: ${name}`);
  }
  requireThat(fs.existsSync(leanLib), 'tool-unavailable', 'selected Lean library is unavailable');
  return {
    source_commit: sourceCommit(),
    files: Object.fromEntries([
      ['fixture', fixture], ['manifest', casesPath],
      ['runner', fileURLToPath(import.meta.url)],
      ['checkers', checkersPath], ['criteria', criteriaPath],
      ['export_reader', path.join(here, 'binding-core.mjs')],
      ['axiom_reader', path.join(here, 'ndjson.mjs')],
      ...Object.entries(binaries),
    ].map(([name, file]) => [name, { path: file, sha256: fileSha256(file), bytes: fs.statSync(file).size }])),
    node: { path: process.execPath, sha256: fileSha256(process.execPath) },
    selected_lean: manifest.lean,
    selected_lean_source_revision: checkersPin.toolchain.githash,
    selected_export_format: manifest.export_format,
  };
}

function environment() {
  return {
    HOME: process.env.HOME ?? '/',
    LANG: 'C.UTF-8',
    PATH: [leanBin, '/run/current-system/sw/bin', '/usr/bin', '/bin'].join(':'),
    LEAN_PATH: [work, leanLib].join(path.delimiter),
    TMPDIR: work,
  };
}

/** A finite subprocess with a real address-space limit and observed GNU time wall/RSS. */
function execute(id, command, timeoutMs, cwd, { exportPath = null } = {}) {
  const timePath = path.join(work, 'logs', `${id}.time`);
  const args = [binaries.time, '-f', '%e %M', '-o', timePath, '--',
    binaries.prlimit, `--as=${bounds.address_space_bytes}`, '--', ...command];
  const outputDescriptor = exportPath === null ? null : fs.openSync(exportPath, 'w');
  const started = process.hrtime.bigint();
  let result;
  try {
    result = spawnSync(args[0], args.slice(1), {
      cwd, env: environment(), encoding: 'utf8',
      stdio: ['ignore', outputDescriptor ?? 'pipe', 'pipe'],
      timeout: timeoutMs, killSignal: 'SIGKILL', maxBuffer: bounds.max_captured_output_bytes,
    });
  } finally {
    if (outputDescriptor !== null) fs.closeSync(outputDescriptor);
  }
  const elapsedMs = Number((process.hrtime.bigint() - started) / 1_000_000n);
  const stdout = result.stdout ?? '';
  const stderr = result.stderr ?? '';
  const stdoutPath = path.join(work, 'logs', `${id}.stdout`);
  const stderrPath = path.join(work, 'logs', `${id}.stderr`);
  fs.writeFileSync(stdoutPath, stdout);
  fs.writeFileSync(stderrPath, stderr);
  let wallSeconds = null;
  let maxRssKb = null;
  if (fs.existsSync(timePath)) {
    const match = /^([0-9]+(?:\.[0-9]+)?) ([0-9]+)$/u.exec(
      fs.readFileSync(timePath, 'utf8').trim().split('\n').at(-1) ?? '');
    if (match) {
      wallSeconds = Number(match[1]);
      maxRssKb = Number(match[2]);
    }
  }
  return {
    argv: args, cwd, timeout_ms: timeoutMs, address_space_bytes: bounds.address_space_bytes,
    exit_code: result.status, signal: result.signal,
    process_error: result.error ? { code: result.error.code ?? null, message: result.error.message } : null,
    elapsed_ms: elapsedMs, observed_wall_seconds: wallSeconds, observed_max_rss_kb: maxRssKb,
    stdout: { path: stdoutPath, sha256: fileSha256(stdoutPath), bytes: fs.statSync(stdoutPath).size,
      excerpt: stdout.slice(-3000) },
    stderr: { path: stderrPath, sha256: fileSha256(stderrPath), bytes: fs.statSync(stderrPath).size,
      excerpt: stderr.slice(-3000) },
    // These are the full original texts only while classifying, never serialized into the receipt.
    text: `${stdout}\n${stderr}`,
  };
}

function publicRun(run) {
  const { text: _text, ...data } = run;
  return data;
}

function failureClass(run) {
  const text = run.text;
  if (run.process_error?.code === 'ETIMEDOUT') return 'timeout';
  if (run.process_error) return 'process-error';
  if (run.signal === 'SIGKILL' || run.signal === 'SIGXCPU' || run.exit_code === 137
    || (run.exit_code === 139 && run.text.trim() === '')
    || /out of memory|cannot allocate|allocation failed|resource temporarily unavailable|failed to map segment/iu.test(text)) {
    return 'resource-exhausted';
  }
  if (run.signal || run.exit_code === null) return 'process-error';
  if (run.observed_wall_seconds === null || run.observed_max_rss_kb === null) return 'timing-unavailable';
  return null;
}

function checkerClass(run, name) {
  const failed = failureClass(run);
  if (failed) return { classification: failed, checked_declarations: null };
  const text = run.text;
  if (run.exit_code === 2 || /\bunsupported(?: format| feature| extension)?\b/iu.test(text)) {
    return { classification: 'unsupported', checked_declarations: null };
  }
  if (name === 'official-kernel') {
    if (run.exit_code === 0 && /Lean default kernel accepts the solution/u.test(text)
      && !/rejects the solution/u.test(text)) {
      return { classification: 'accepted', checked_declarations: null,
        count_evidence: 'leanchecker --from-export replays the full parsed constMap; its stock success marker is not a count' };
    }
    if (run.exit_code === 1 && /Lean default kernel rejects the solution:/u.test(text)) {
      return { classification: 'kernel-rejection', checked_declarations: null };
    }
  }
  if (/\b(?:declares )?unpermitted axiom\b|\bforbidden axiom\b/iu.test(text) && run.exit_code !== 0) {
    return { classification: 'forbidden-axiom', checked_declarations: null };
  }
  if (/\b(?:parse|json|ndjson|syntax)\b.{0,90}\b(?:error|invalid|fail|unexpected)\b|\bExpected JSON object\b|\b(?:Name|Level|Expr) (?:not found|invalid)\b|\b(?:Name|Level|Expr) index [0-9]+ bound twice\b|\bUnknown export object\b/iu.test(text)) {
    return { classification: 'parse-failure', checked_declarations: null };
  }
  if (name === 'nanoda') {
    const match = /Checked ([0-9]+) declarations with no errors/u.exec(run.stdout.excerpt);
    if (run.exit_code === 0 && match) {
      const count = Number(match[1]);
      return { classification: count > 0 ? 'accepted' : 'empty-check', checked_declarations: count };
    }
    // Nanoda's known type-check failure is an assertion in tc.rs; arbitrary panics do not pass.
    if (run.exit_code !== 0 && /\bsrc\/tc\.rs:\d+:\d+:/u.test(text)
      && /assertion failed: self\.def_eq\b|(?:application )?type mismatch/iu.test(text)) {
      return { classification: 'kernel-rejection', checked_declarations: null };
    }
  }
  return { classification: 'checker-error', checked_declarations: null };
}

function inspection(file, rootName) {
  const raw = fs.readFileSync(file);
  let text;
  try {
    text = new TextDecoder('utf-8', { fatal: true }).decode(raw);
  } catch (error) {
    throw new AdmissionError('parse-failure', `non-UTF8 export: ${error}`);
  }
  requireThat(text.endsWith('\n'), 'parse-failure', 'export must end in a newline');
  const lines = text.trimEnd().split('\n');
  let records;
  try {
    records = lines.map((line) => JSON.parse(line));
  } catch (error) {
    throw new AdmissionError('parse-failure', `invalid NDJSON: ${error}`);
  }
  requireThat(records.every((record) => record && typeof record === 'object' && !Array.isArray(record))
    && records.filter((record) => record.meta).length === 1 && records[0]?.meta,
  'parse-failure', 'an NDJSON export must begin with its single meta record');
  const meta = records[0].meta;
  requireThat(meta.format?.version === manifest.export_format
    && meta.lean?.version === manifest.lean
    && meta.lean?.githash === checkersPin.toolchain.githash, 'unsupported-format',
  'export metadata does not name the pinned 3.1.0/Lean 4.35.0 producer profile');
  let parsed;
  let listed;
  try {
    parsed = parseExport(file);
    listed = inspectExport(file);
  } catch (error) {
    throw new AdmissionError('parse-failure', `local NDJSON record reader: ${error}`);
  }
  requireThat(parsed.format === manifest.export_format && listed.format === parsed.format,
    'unsupported-format', 'local readers disagree about the exported profile');
  requireThat(parsed.decls.has(rootName), 'export-failure', `root missing from export: ${rootName}`);
  const rootNameIdx = [...parsed.names].find(([, name]) => name === rootName)?.[0];
  requireThat(rootNameIdx !== undefined
    && records.filter((record) => record.thm?.name === rootNameIdx).length === 1,
  'export-failure', `expected exactly one theorem record for ${rootName}`);
  let closure;
  try {
    closure = closureOf(parsed, [rootName]);
  } catch (error) {
    throw new AdmissionError('parse-failure', `incomplete expression record: ${error}`);
  }
  requireThat(closure.missing.size === 0, 'export-failure',
    `incomplete root closure: ${[...closure.missing].join(', ')}`);
  const foreign = foreignAxioms(listed);
  return {
    parsed, records, foreign, closure,
    record: {
      path: file, sha256: sha256(raw), bytes: raw.length, ndjson_records: lines.length,
      format: parsed.format, lean_githash: meta.lean.githash,
      root: rootName, root_statement_sha256: sha256(exportStatement(parsed, rootName)),
      declared_declarations: parsed.decls.size, reachable_declarations: closure.reachable.size,
      axioms: listed.axioms, foreign_axioms: foreign,
    },
  };
}

function expressionIds(parsed, start) {
  const visited = new Set();
  const todo = [start];
  while (todo.length > 0) {
    const id = todo.pop();
    if (visited.has(id)) continue;
    visited.add(id);
    const expr = parsed.exprs.get(id);
    requireThat(expr, 'parse-failure', `expression ${id} missing from root type`);
    if (expr.app) todo.push(expr.app.fn, expr.app.arg);
    else if (expr.lam) todo.push(expr.lam.type, expr.lam.body);
    else if (expr.forallE) todo.push(expr.forallE.type, expr.forallE.body);
    else if (expr.letE) todo.push(expr.letE.type, expr.letE.value, expr.letE.body);
    else if (expr.mdata) todo.push(expr.mdata.expr);
    else if (expr.proj) todo.push(expr.proj.struct);
  }
  return visited;
}

/**
 * Copy the type path after the existing expression table. Back-references stay continuous and
 * earlier than each use; original expression/proof records remain untouched, and only the final
 * theorem type index changes. The mutation changes a single leaf on that copied type path.
 */
function replaceTypeLeaf(parsed, rootName, rootType, targetId, makeLeaf, lines) {
  const theoremName = [...parsed.names].find(([, name]) => name === rootName)?.[0];
  const theoremAt = lines.findIndex((line) => JSON.parse(line).thm?.name === theoremName);
  requireThat(theoremAt >= 0 && theoremAt === lines.length - 1, 'mutation-shape-error',
    'root theorem must be the last record of the export');
  let nextId = Math.max(...parsed.exprs.keys()) + 1;
  const appended = [];
  const emit = (record) => {
    const id = nextId++;
    appended.push(JSON.stringify({ ...record, ie: id }));
    return id;
  };
  const replacementId = makeLeaf(emit, parsed.exprs.get(targetId));
  const parents = new Map();
  function rewrite(id) {
    if (id === targetId) return replacementId;
    if (parents.has(id)) return parents.get(id);
    const old = parsed.exprs.get(id);
    requireThat(old, 'mutation-shape-error', `missing expression on theorem type path: ${id}`);
    const kind = ['app', 'lam', 'forallE', 'letE', 'mdata', 'proj']
      .find((key) => old[key] !== undefined);
    if (!kind) return id;
    const links = {
      app: ['fn', 'arg'], lam: ['type', 'body'], forallE: ['type', 'body'],
      letE: ['type', 'value', 'body'], mdata: ['expr'], proj: ['struct'],
    }[kind];
    const content = { ...old[kind] };
    let changed = false;
    for (const field of links) {
      const updated = rewrite(content[field]);
      if (updated !== content[field]) { content[field] = updated; changed = true; }
    }
    const result = changed ? emit({ ...old, [kind]: content }) : id;
    parents.set(id, result);
    return result;
  }
  const newType = rewrite(rootType);
  requireThat(newType !== rootType, 'mutation-shape-error',
    'chosen leaf is not reachable from the theorem type');
  const theorem = JSON.parse(lines[theoremAt]);
  theorem.thm.type = newType;
  lines.splice(theoremAt, 1, ...appended, JSON.stringify(theorem));
  return { original_type: rootType, amended_type: newType,
    copied_expression_records: appended.length };
}

/** Alter one leaf on a copied statement type path; never mutate an existing proof expression. */
function mutate(caseEntry, base, file) {
  const root = base.parsed.decls.get(caseEntry.root);
  requireThat(root?.kind === 'thm', 'mutation-shape-error', 'mutation root must be a theorem');
  const typeIds = expressionIds(base.parsed, root.record.type);
  const lines = fs.readFileSync(base.record.path, 'utf8').trimEnd().split('\n');
  let detail;
  if (caseEntry.mutation === 'function-eta-wrong-function') {
    const targets = [...typeIds].filter((id) => {
      const e = base.parsed.exprs.get(id);
      const body = base.parsed.exprs.get(e?.lam?.body);
      return body?.app
        && base.parsed.exprs.get(body.app.fn)?.bvar === 2
        && base.parsed.exprs.get(body.app.arg)?.bvar === 0;
    });
    requireThat(targets.length === 1, 'mutation-shape-error',
      `expected exactly one eta lambda, found ${targets.length}`);
    const bodyId = base.parsed.exprs.get(targets[0]).lam.body;
    const prior = base.parsed.exprs.get(bodyId);
    const copied = replaceTypeLeaf(base.parsed, caseEntry.root, root.record.type, bodyId,
      (emit, old) => {
        const g = emit({ bvar: 1 });
        return emit({ ...old, app: { ...old.app, fn: g } });
      }, lines);
    detail = {
      target_expression: bodyId, before: prior, ...copied,
      explanation: 'the valid lambda applies bound f (#2); the mutated lambda applies bound g (#1) to x (#0)',
    };
  } else if (caseEntry.mutation === 'structure-eta-wrong-field') {
    const targets = [...typeIds].filter((id) => {
      const e = base.parsed.exprs.get(id);
      return e?.const && base.parsed.names.get(e.const.name) === 'Prod.snd';
    });
    requireThat(targets.length === 1, 'mutation-shape-error',
      `expected exactly one Prod.snd constant in the root statement, found ${targets.length}`);
    const fst = [...base.parsed.names].find(([, name]) => name === 'Prod.fst')?.[0];
    requireThat(fst !== undefined, 'mutation-shape-error', 'Prod.fst name missing');
    const prior = base.parsed.exprs.get(targets[0]);
    const copied = replaceTypeLeaf(base.parsed, caseEntry.root, root.record.type, targets[0],
      (emit, old) => emit({ ...old, const: { ...old.const, name: fst } }), lines);
    detail = {
      target_expression: targets[0], before: prior, replacement_name: 'Prod.fst', ...copied,
      explanation: 'the valid Prod.snd p is replaced with same-typed Prod.fst p',
    };
  } else {
    throw new AdmissionError('configuration-error', `unknown frozen mutation: ${caseEntry.mutation}`);
  }
  fs.writeFileSync(file, `${lines.join('\n')}\n`);
  const changed = inspection(file, caseEntry.root);
  requireThat(changed.record.root_statement_sha256 !== base.record.root_statement_sha256
    && changed.record.reachable_declarations === base.record.reachable_declarations
    && same(changed.record.axioms, base.record.axioms), 'mutation-shape-error',
  'the mutated statement did not change uniquely while retaining its closure and axioms');
  return { detail, changed };
}

function declarationReferences(parsed, name) {
  const decl = parsed.decls.get(name);
  if (!decl || decl.kind === 'inductive') return [];
  const names = new Set();
  for (const start of decl.exprs) {
    for (const id of expressionIds(parsed, start)) {
      const expr = parsed.exprs.get(id);
      if (expr.const) names.add(parsed.names.get(expr.const.name));
    }
  }
  return [...names].sort();
}

function nativeEvidence(inspected, rootName) {
  const generated = inspected.foreign.filter((name) => NATIVE.test(name));
  requireThat(generated.length > 0 && generated.length === inspected.foreign.length
    && generated.every((name) => inspected.closure.reachable.has(name)),
  'native-axiom-missing',
  `${rootName} does not transitively reach only generated, forbidden native_decide axioms`);
  const direct = declarationReferences(inspected.parsed, rootName);
  if (rootName.endsWith('.nativeTransitive')) {
    const helper = direct.filter((name) => name.includes('nativeHelper'));
    requireThat(helper.length === 1
      && generated.some((name) => declarationReferences(inspected.parsed, helper[0]).includes(name)),
    'native-axiom-missing', 'transitive theorem must depend on private helper, then its generated axiom');
    return { generated, dependency_path: [rootName, helper[0], generated[0]] };
  }
  requireThat(generated.some((name) => direct.includes(name)), 'native-axiom-missing',
    'direct native_decide proof must reference its generated axiom');
  return { generated, dependency_path: [rootName, generated[0]] };
}

function checkers(id, exportPath) {
  const configPath = path.join(work, 'configs', `${id}.nanoda.json`);
  fs.writeFileSync(configPath, `${JSON.stringify({
    use_stdin: false, export_file_path: exportPath,
    permitted_axioms: STANDARD_AXIOMS, unpermitted_axiom_hard_error: true,
    num_threads: nanodaPin.run.threads, nat_extension: true, string_extension: true,
    print_axioms: false, print_success_message: true,
  }, null, 2)}\n`);
  const commands = {
    'official-kernel': [binaries.official, '--from-export', exportPath],
    nanoda: [binaries.nanoda, configPath],
  };
  const runs = {};
  for (const [name, command] of Object.entries(commands)) {
    const execution = execute(`${id}.${name}`, command, bounds.checker_timeout_ms, work);
    runs[name] = {
      ...publicRun(execution), ...checkerClass(execution, name),
      config: name === 'nanoda' ? {
        path: configPath, sha256: fileSha256(configPath), bytes: fs.statSync(configPath).size,
      } : null,
    };
  }
  return runs;
}

function evaluate(entry, inspected, runs) {
  requireThat(inspected.record.declared_declarations > 0, 'empty-check',
    'the official kernel cannot accept an export without declared constants');
  if (runs['official-kernel'].classification === 'accepted') {
    // LeanChecker.checkExport replays its parsed constMap, after dropping generated Quot.mk/lift/ind.
    // It prints only an acceptance marker; this is an eligible INPUT count, not a printed checker count.
    runs['official-kernel'].eligible_input_declarations = inspected.record.declared_declarations
      - ['Quot.mk', 'Quot.lift', 'Quot.ind'].filter((name) => inspected.parsed.decls.has(name)).length;
    runs['official-kernel'].eligible_count_origin = 'parsed-export-minus-quotient-postchecks-not-checker-output';
    requireThat(runs['official-kernel'].eligible_input_declarations > 0, 'empty-check',
      'the official kernel has no eligible declarations to replay');
  }
  const native = entry.expected.lane === 'forbidden-axiom';
  let nativeControl = null;
  if (native) nativeControl = nativeEvidence(inspected, entry.root);
  else requireThat(inspected.foreign.length === 0, 'forbidden-axiom',
    `unexpected axiom in ${entry.id}: ${inspected.foreign.join(', ')}`);
  const actual = native ? 'forbidden-axiom'
    : runs['official-kernel'].classification === 'kernel-rejection'
        && runs.nanoda.classification === 'kernel-rejection' ? 'kernel-rejection'
      : runs['official-kernel'].classification === 'accepted'
        && runs.nanoda.classification === 'accepted' ? 'accepted'
        : 'checker-disagreement';
  return {
    native_control: nativeControl,
    lane_class: actual,
    matches_frozen_expectation: actual === entry.expected.lane
      && Object.entries(entry.expected).every(([name, expected]) =>
        name === 'lane' || runs[name]?.classification === expected),
  };
}

function run() {
  const source = bindInputs();
  requireThat(!fs.existsSync(work), 'configuration-error', 'work directory already exists');
  fs.mkdirSync(work, { recursive: false, mode: 0o700 });
  for (const dir of ['logs', 'exports', 'configs']) fs.mkdirSync(path.join(work, dir));
  const receipt = {
    schema: 'noble-compiler217-local-replay/v1',
    source, manifest_schema: manifest.schema,
    fixture_provenance: manifest.provenance,
    supported_profile: { lean: manifest.lean, export_format: manifest.export_format,
      allowed_axioms: STANDARD_AXIOMS, checkers: ['official-kernel', 'nanoda'] },
    resource_bounds: bounds,
    resource_bound_amendment: manifest.resource_bound_amendment,
    warning_only_amendment: manifest.warning_only_amendment,
    encoding_amendment: manifest.encoding_amendment,
    compile: null, exports: [], cases: [],
    shared_corpus_consumption: {
      decision: 'NO-GO',
      reason: manifest.provenance.reason,
      upstream_fixture_source_revision: null, upstream_fixture_sha256: null,
    },
    local_replay_holds: false,
    non_claims: [
      'No upstream compiler-lab #217 fixture bytes have been consumed or executed.',
      'Neither checker proves its own correctness or its equivalence to the other checker.',
      'This local conformance corpus does not establish source authenticity or compiled Rust/Wasm correspondence.',
    ],
  };
  const receiptPath = path.join(work, 'receipt.json');
  try {
    const outputOlean = path.join(work, 'Compiler217.olean');
    const compilation = execute('compile',
      [binaries.lean, '-o', outputOlean, fixture], bounds.compile_timeout_ms, fixtureDir);
    receipt.compile = { ...publicRun(compilation),
      classification: failureClass(compilation) ?? (compilation.exit_code === 0 ? 'accepted' : 'compile-error'),
      olean: fs.existsSync(outputOlean)
        ? { path: outputOlean, sha256: fileSha256(outputOlean), bytes: fs.statSync(outputOlean).size }
        : null };
    requireThat(receipt.compile.classification === 'accepted' && receipt.compile.olean
      && !/\bwarning:/iu.test(compilation.text), receipt.compile.classification === 'accepted'
        ? 'compile-error' : receipt.compile.classification, 'fixture did not compile cleanly');
    const exports = new Map();
    for (const rootName of new Set(manifest.cases.map((entry) => entry.root))) {
      const id = rootName.split('.').at(-1);
      const exportPath = path.join(work, 'exports', `${id}.ndjson`);
      const exported = execute(`export-${id}`,
        [binaries.exporter, manifest.module, '--', rootName],
        bounds.export_timeout_ms, fixtureDir, { exportPath });
      const stage = { root: rootName, ...publicRun(exported),
        classification: failureClass(exported) ?? (exported.exit_code === 0
          && !/\bPANIC\b/u.test(exported.text) ? 'accepted' : 'export-failure'),
        raw_export: { path: exportPath, sha256: fileSha256(exportPath),
          bytes: fs.statSync(exportPath).size } };
      receipt.exports.push(stage);
      if (stage.classification !== 'accepted') continue;
      try {
        const parsed = inspection(exportPath, rootName);
        stage.export = parsed.record;
        exports.set(rootName, parsed);
      } catch (error) {
        stage.classification = error.classification ?? 'export-failure';
        stage.error = String(error);
      }
    }
    for (const entry of manifest.cases) {
      const row = {
        id: entry.id, category: entry.category, root: entry.root,
        provenance: 'local-equivalent-not-upstream-fixture',
        rationale: entry.rationale, expected: entry.expected,
      };
      receipt.cases.push(row);
      try {
        const base = exports.get(entry.root);
        requireThat(base, 'export-failure', `base export for ${entry.root} did not pass admission`);
        let inspected = base;
        if (entry.mutation) {
          const file = path.join(work, 'exports', `${entry.id}.ndjson`);
          const mutation = mutate(entry, base, file);
          inspected = mutation.changed;
          row.mutation = { ...mutation.detail, base_export_sha256: base.record.sha256,
            base_statement_sha256: base.record.root_statement_sha256 };
        }
        row.export = inspected.record;
        row.checkers = checkers(entry.id, inspected.record.path);
        Object.assign(row, evaluate(entry, inspected, row.checkers));
      } catch (error) {
        row.lane_class = error.classification ?? 'process-error';
        row.error = String(error);
        row.matches_frozen_expectation = false;
      }
    }
    const allExpected = receipt.cases.length === 7
      && receipt.cases.every((entry) => entry.matches_frozen_expectation === true);
    receipt.local_replay_holds = allExpected
      && receipt.exports.length === 5
      && receipt.exports.every((entry) => entry.classification === 'accepted');
  } catch (error) {
    receipt.failure = { classification: error.classification ?? 'process-error', detail: String(error) };
  } finally {
    try {
      requireThat(sourceCommit() === source.source_commit, 'source-dirty',
        'source changed during replay');
      for (const [name, entry] of Object.entries(source.files)) {
        requireThat(fileSha256(entry.path) === entry.sha256, 'source-pin-mismatch',
          `${name} bytes changed during replay`);
      }
    } catch (error) {
      receipt.failure = { classification: error.classification ?? 'process-error', detail: String(error) };
    }
    if (receipt.failure) receipt.local_replay_holds = false;
    fs.writeFileSync(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`);
  }
  console.log(`compiler217: local=${receipt.local_replay_holds ? 'PASS' : 'NO-GO'} `
    + `shared-corpus=NO-GO receipt=${receiptPath}`);
  if (!receipt.local_replay_holds) process.exitCode = 1;
}

try {
  run();
} catch (error) {
  console.error(`compiler217: class=${error.classification ?? 'process-error'} ${error}`);
  process.exitCode = 1;
}
