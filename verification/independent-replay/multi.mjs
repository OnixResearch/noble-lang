#!/usr/bin/env node
// Issue #62 N-version replay: every pinned external checker over the same bound closures.
//
// `binding.mjs` binds one checker (nanoda) behind statement and closure checks. This lane widens the
// replay to every checker bundled with the pinned Lean 4.35.0 release, keeping the same strict rules:
//
//   export stage   the export declares every requested root (from its own name records);
//   closure        every constant the roots reference, transitively, is declared;
//   statement      each root's statement, rendered from the export, equals the digest in
//                  statements.json, and that manifest equals the compiled environment;
//   format         the export's declared meta format version is the pinned 3.1.0, because checker
//                  binaries do not necessarily enforce the producer's declared format version;
//   axiom audit    every declared axiom is propext, Classical.choice or Quot.sound;
//   checker        exit 0 with a positive checked-declaration count and a rejection-shaped message
//                  absent. Exit 2, declared feature declines, parser diagnostics, kernel rejection,
//                  and resource exhaustion are separate refusal classes. Other nonzero exits and
//                  unparseable zero exits are `checker-error`; no refusal is a mathematical
//                  refutation.
//
// The quorum is never weaker than one checker: a subject is accepted only when every pinned checker
// accepts it under those rules. Disagreements are retained verbatim, not averaged.
//
// Every proof, closure, claim, format, axiom, parser, resource and capability control has a row for
// every checker. Controls without a checker-facing option are recorded as unsupported, never passed.
// Checker and lane verdicts are separate; executed checker calls retain per-run wall time and max RSS.
// Pilot evidence on one host, not a gate.
import { spawnSync } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  closureDigest, closureOf, dropDeclaration, exportStatement, parseExport, renameTheorem,
} from './binding-core.mjs';
import { foreignAxioms, inspectExport } from './ndjson.mjs';
import { pinGuard } from './pins.mjs';
import { leanStatements } from './statements-extract.mjs';

const argv = process.argv.slice(2);
const value = (name) => {
  const index = argv.indexOf(name);
  if (index < 0 || index + 1 >= argv.length) throw new Error(`missing ${name}`);
  return argv[index + 1];
};
const optional = (name, fallback = null) => (argv.includes(name) ? value(name) : fallback);
const sha256 = (data) => crypto.createHash('sha256').update(data).digest('hex');
const fileSha256 = (file) => sha256(fs.readFileSync(file));
const here = path.dirname(fileURLToPath(import.meta.url));

const leanBin = path.resolve(value('--lean-bin'));
const checkersPath = path.resolve(value('--checkers'));
const manifestPath = path.resolve(value('--manifest'));
const foundation = path.resolve(value('--foundation'));
const large = path.resolve(value('--large-export'));
const small = path.resolve(value('--small-export'));
const mutationSource = path.resolve(value('--mutation-export'));
const timeBin = path.resolve(optional('--time', '/run/current-system/sw/bin/time'));
const grepBin = path.resolve(optional('--grep', '/run/current-system/sw/bin/grep'));
const prlimitBin = path.resolve(optional('--prlimit', '/run/current-system/sw/bin/prlimit'));
const work = path.resolve(value('--work'));
const output = path.resolve(value('--output'));
const toolchain = optional('--toolchain');
const smallRetainedAt = optional('--small-retained-at');
const mutationRetainedAt = optional('--mutation-retained-at');
const pins = {
  lean: value('--lean-sha256'),
  large: value('--large-sha256'),
  small: value('--small-sha256'),
  mutation: value('--mutation-sha256'),
};
const checkersPin = JSON.parse(fs.readFileSync(checkersPath, 'utf8'));
const binaries = new Map(checkersPin.checkers.map((checker) => [
  checker.id,
  path.resolve(checker.binary_arg ? value(checker.binary_arg) : path.join(leanBin, checker.binary)),
]));

// The pin guard runs first: no checker binary, Lean binary or export is used when a digest moved.
pinGuard([
  { role: 'lean', file: path.join(leanBin, 'lean'), sha256: pins.lean, pinClass: 'tool-pin-mismatch' },
  { role: 'lake', file: path.join(leanBin, 'lake'), sha256: checkersPin.toolchain.lake_sha256, pinClass: 'tool-pin-mismatch' },
  { role: 'leanexport', file: path.join(leanBin, 'leanexport'),
    sha256: checkersPin.toolchain.leanexport_sha256, pinClass: 'tool-pin-mismatch' },
  ...checkersPin.checkers.map((checker) => ({
    role: `checker ${checker.id}`, file: binaries.get(checker.id), sha256: checker.sha256, pinClass: 'tool-pin-mismatch',
  })),
  { role: 'large export', file: large, sha256: pins.large, pinClass: 'export-pin-mismatch' },
  { role: 'small export', file: small, sha256: pins.small, pinClass: 'export-pin-mismatch' },
  { role: 'mutation export', file: mutationSource, sha256: pins.mutation, pinClass: 'export-pin-mismatch' },
]);

if (fs.existsSync(work)) throw new Error(`work directory exists: ${work}`);
fs.mkdirSync(work, { recursive: true });

const STANDARD_AXIOMS = ['propext', 'Classical.choice', 'Quot.sound'];
const EXPECTED_FORMAT = checkersPin.export_format.version;
const LARGE_ROOT = 'NobleFoundation.toInt_add';
const SMALL_ROOT = 'NobleFoundation.LinkBoundary.vertical_simulation_compose';
const FRESH_ROOT = 'Nat.add_comm';
const DROPPED_THEOREM = '_private.NobleFoundation.LinkBoundary.0.NobleFoundation.LinkBoundary.forward_simulation_steps';
const DROPPED_DEFINITION = 'NobleFoundation.LinkBoundary.ForwardSimulation';
const CUSTOM_AXIOM = 'NobleReplayControl.customAxiom';
const manifest = JSON.parse(fs.readFileSync(manifestPath, 'utf8'));
const childEnv = {
  HOME: process.env.HOME ?? '/', LANG: 'C.UTF-8',
  PATH: [leanBin, '/run/current-system/sw/bin', '/usr/bin', '/bin'].join(':'), TMPDIR: work,
};

/** Declared implementation language, re-checked from the binary itself rather than asserted. */
function languageEvidence(checker) {
  const result = spawnSync(grepBin, ['-a', '-o', '-m', '1', '-E', checker.language_marker, binaries.get(checker.id)], {
    encoding: 'utf8', maxBuffer: 16 * 1024 * 1024, timeout: 300_000,
  });
  const matched = (result.stdout ?? '').trim();
  return {
    declared: checker.language,
    marker: checker.language_marker,
    matched: matched !== '',
    match: matched.slice(0, 120),
    exit_code: result.status,
  };
}

function nanodaConfig(id, exportPath, checker, overrides = {}) {
  const file = path.join(work, `${id}.${checker.id}.json`);
  fs.writeFileSync(file, `${JSON.stringify({
    use_stdin: false,
    export_file_path: exportPath,
    permitted_axioms: STANDARD_AXIOMS,
    unpermitted_axiom_hard_error: true,
    num_threads: checker.run.threads ?? 1,
    nat_extension: true,
    string_extension: true,
    print_axioms: false,
    print_success_message: true,
    ...overrides,
  }, null, 2)}\n`);
  return file;
}

function commandFor(checker, exportPath, id, configOverrides = {}) {
  if (checker.run.kind === 'nanoda-config') {
    return [binaries.get(checker.id), nanodaConfig(id, exportPath, checker, configOverrides)];
  }
  return [binaries.get(checker.id), ...checker.run.argv.map((part) => (part === 'FILE' ? exportPath : part))];
}

/** Classification from exit status, signal and the checker's own output; never from the lane's hopes. */
function classify(result, checker, timedOut) {
  const stdout = result.stdout ?? '';
  const stderr = result.stderr ?? '';
  const processError = result.error?.message ?? '';
  const text = `${stdout}\n${stderr}\n${processError}`;
  if (timedOut) return { classification: 'timeout', count: null };
  if (result.status === 2 || (checker.declined_exit ?? []).includes(result.status)) {
    return { classification: 'unsupported', count: null };
  }
  if (/\b(?:extension|literal)\b.{0,60}\b(?:disallow|unsupported|not supported|disabled|not enabled)\b|\b(?:nat|string) literal\b.{0,80}\b(?:disabled|unsupported|not supported|not enabled)\b|\bunsupported(?: feature| extension)?\b/iu.test(text)) {
    return { classification: 'unsupported', count: null };
  }
  if (result.signal === 'SIGKILL' || result.signal === 'SIGXCPU' || result.signal === 'SIGXFSZ'
      || result.status === 137 || /out of memory|cannot allocate(?: memory)?|allocation failed|memory allocation|too many open files|\bEMFILE\b|\bENOMEM\b|resource temporarily unavailable|failed to allocate|failed to map segment/iu.test(text)) {
    return { classification: 'resource-exhausted', count: null };
  }
  if (/parse.{0,60}(?:error|fail|invalid|unexpected|eof|end of input)|(?:json|ndjson).{0,60}(?:error|fail|invalid|unexpected|eof|end of input)|(?:unexpected|invalid|expected|eof).{0,60}(?:json|ndjson|token|input|key|value)|EOF while parsing|expected .{1,80} after|unexpected end of input/iu.test(text)) {
    return { classification: 'parse-failure', count: null };
  }
  if (result.status === 0) {
    const match = new RegExp(checker.accept_pattern, 'u').exec(stdout);
    if (!match) return { classification: 'checker-error', count: null, output_unparsed: true };
    const count = Number(match[1]);
    return count > 0
      ? { classification: 'accepted', count }
      : { classification: 'empty-check', count };
  }
  const rejectionShaped = /mismatch|invalid:|rejected|unknown constant|declaration not found|assertion failed|\(kernel\)/iu.test(text);
  return {
    classification: rejectionShaped ? 'kernel-rejection' : 'checker-error',
    count: null,
    rejection_shaped: rejectionShaped,
  };
}

function runChecker(checker, exportPath, id, { configOverrides = {}, addressSpaceBytes = null } = {}) {
  const command = commandFor(checker, exportPath, id, configOverrides);
  const launched = addressSpaceBytes === null
    ? command
    : [prlimitBin, `--as=${addressSpaceBytes}`, '--', ...command];
  const timeFile = path.join(work, `${id}.${checker.id}.time`);
  const started = process.hrtime.bigint();
  const result = spawnSync(timeBin, ['-f', '%e %M', '-o', timeFile, '--', ...launched], {
    env: childEnv, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024, timeout: 900_000, killSignal: 'SIGKILL',
  });
  const elapsed = Number((process.hrtime.bigint() - started) / 1_000_000n);
  const timedOut = result.error?.code === 'ETIMEDOUT';
  let time = { seconds: null, max_rss_kb: null };
  if (fs.existsSync(timeFile)) {
    const timingLine = fs.readFileSync(timeFile, 'utf8').trim().split('\n').at(-1) ?? '';
    const [seconds, rss] = timingLine.trim().split(/\s+/u);
    const parsedSeconds = Number(seconds);
    const parsedRss = Number(rss);
    if (Number.isFinite(parsedSeconds) && Number.isFinite(parsedRss)) {
      time = { seconds: parsedSeconds, max_rss_kb: parsedRss };
    }
  }
  return {
    checker: checker.id,
    attempted: true,
    config_overrides: checker.run.kind === 'nanoda-config' ? configOverrides : {},
    resource_limit: addressSpaceBytes === null ? null : { address_space_bytes: addressSpaceBytes },
    argv: launched.map((part) => part.replace(work, '$WORK').replace(exportPath, path.basename(exportPath))),
    exit_code: result.status,
    signal: result.signal,
    process_error: result.error ? { code: result.error.code ?? null, message: result.error.message } : null,
    elapsed_ms: elapsed,
    wall_seconds: time.seconds,
    max_rss_kb: time.max_rss_kb,
    ...classify(result, checker, timedOut),
    stdout: result.stdout ?? '',
    stderr: result.stderr ?? '',
  };
}

/** The lane's own verdict for one export: the strict rules in front of the checker's verdict. */
function laneVerdict(parsed, roots, checkerResult) {
  const inspection = { declared: new Set(parsed.decls.keys()), axioms: parsed.axioms, format: parsed.format };
  const absent = roots.filter((root) => !inspection.declared.has(root));
  if (absent.length > 0) return { verdict: 'export-failure', reasons: [`roots absent from the export: ${absent.join(', ')}`] };
  const { reachable, missing } = closureOf(parsed, roots);
  const closure = {
    reachable: reachable.size, missing: [...missing].sort(),
  };
  if (missing.size > 0) {
    return { verdict: 'incomplete-closure', reasons: [`referenced but not declared: ${[...missing].sort().join(', ')}`], closure };
  }
  closure.digest = closureDigest(parsed, reachable);
  const statements = roots.map((root) => {
    const claim = manifest.claims.find((item) => item.root === root);
    return {
      root,
      export_statement_sha256: sha256(exportStatement(parsed, root)),
      pinned_sha256: claim?.statement_sha256 ?? null,
    };
  });
  for (const statement of statements) {
    if (statement.pinned_sha256 === null) {
      return { verdict: 'unbound-claim', reasons: [`${statement.root} has no pinned statement`], closure, statements };
    }
    if (statement.export_statement_sha256 !== statement.pinned_sha256) {
      return {
        verdict: 'claim-mismatch', closure, statements,
        reasons: [`${statement.root}: exported statement differs from the pinned statement`],
      };
    }
  }
  if (parsed.format !== EXPECTED_FORMAT) {
    return {
      verdict: 'unsupported-format', closure, statements,
      reasons: [`export format ${parsed.format} is not the pinned ${EXPECTED_FORMAT}`],
    };
  }
  const foreign = foreignAxioms(inspection);
  if (foreign.length > 0) {
    return { verdict: 'forbidden-axiom', closure, statements, reasons: [`declared axioms: ${foreign.join(', ')}`] };
  }
  return { verdict: checkerResult.classification, closure, statements };
}

function subjectEntry(control, exportPath, description) {
  const parsed = parseExport(exportPath);
  const runs = checkersPin.checkers.map((checker) => {
    const runOptions = {
      configOverrides: control.config ?? {},
      ...(control.runOptions?.(checker) ?? {}),
    };
    const result = runChecker(checker, exportPath, control.id, runOptions);
    return { ...result, lane: laneVerdict(parsed, control.roots, result) };
  });
  const accepted = runs.filter((run) => run.lane.verdict === 'accepted').map((run) => run.checker);
  return {
    id: control.id,
    kind: control.kind,
    roots: control.roots,
    changes: control.changes,
    expected: control.expected,
    export: { ...description, sha256: fileSha256(exportPath), records: parsed.records, format: parsed.format, declared: parsed.decls.size, axioms: parsed.axioms },
    runs,
    quorum: {
      accepted_by: accepted,
      refused_by: runs.filter((run) => run.lane.verdict !== 'accepted').map((run) => `${run.checker}:${run.lane.verdict}`),
      accepted: accepted.length === runs.length,
    },
  };
}

// The manifest must agree with the compiled foundation environment before any export can be bound.
const { statements: extracted, elapsed_ms: extractionMs } = leanStatements({
  leanBin, project: foundation, roots: manifest.claims.map((claim) => claim.root),
});
const manifestCheck = manifest.claims.map((claim) => ({
  root: claim.root,
  environment_sha256: sha256(extracted.get(claim.root)),
  pinned_sha256: claim.statement_sha256,
  matches: sha256(extracted.get(claim.root)) === claim.statement_sha256,
}));

// Baselines: the same three subjects limits.mjs and binding.mjs use.
const freshExport = path.join(work, 'fresh-Init.ndjson');
const exportDescriptor = fs.openSync(freshExport, 'w');
const freshRun = spawnSync(path.join(leanBin, 'leanexport'), ['Init', '--', FRESH_ROOT], {
  env: childEnv, encoding: 'utf8', stdio: ['ignore', exportDescriptor, 'pipe'],
  maxBuffer: 64 * 1024 * 1024, timeout: 900_000, killSignal: 'SIGKILL',
});
fs.closeSync(exportDescriptor);
const freshStage = {
  argv: ['leanexport', 'Init', '--', FRESH_ROOT],
  exit_code: freshRun.status,
  signal: freshRun.signal,
  panic: /PANIC/u.test(freshRun.stderr ?? ''),
  stderr_excerpt: (freshRun.stderr ?? '').split('\nbacktrace:')[0].trim().slice(0, 300),
};
const freshInspection = inspectExport(freshExport);
if (freshStage.exit_code !== 0 || freshStage.panic || !freshInspection.declared.has(FRESH_ROOT)) {
  throw new Error(`the fresh export stage failed: ${JSON.stringify(freshStage)}`);
}

const baselines = [
  { id: 'baseline-large', kind: 'baseline', roots: [LARGE_ROOT], changes: 'none: the pinned toInt_add export',
    expected: 'accepted by every checker', file: large },
  { id: 'baseline-small', kind: 'baseline', roots: [SMALL_ROOT], changes: 'none: the retained vertical_simulation_compose export',
    expected: 'accepted by every checker', file: small },
  { id: 'baseline-fresh', kind: 'baseline', roots: [FRESH_ROOT], changes: 'none: a fresh Init export of Nat.add_comm',
    expected: 'accepted by every checker', file: freshExport },
].map((control) => subjectEntry(control, control.file, {
  retained: control.id !== 'baseline-fresh',
  path: control.id === 'baseline-small' ? smallRetainedAt : null,
  producer: control.id === 'baseline-fresh' ? 'leanexport 4.35.0 Init' : 'leanexport 4.35.0 NobleFoundation',
  stage: control.id === 'baseline-fresh' ? freshStage : null,
}));

// The proof-term mutation is derived from the currently pinned small export: one application
// argument in the root proof, `arg 396` -> `394`. The old 4.31.0 mutation receipt remains history.
function semanticMutation() {
  const lines = fs.readFileSync(mutationSource, 'utf8').split('\n');
  const parsed = parseExport(mutationSource);
  const root = parsed.decls.get(SMALL_ROOT);
  if (!root || root.kind !== 'thm') throw new Error(`${mutationSource}: missing theorem ${SMALL_ROOT}`);
  const reachable = new Set();
  const stack = [root.record.value];
  while (stack.length > 0) {
    const index = stack.pop();
    if (reachable.has(index)) continue;
    reachable.add(index);
    const expr = parsed.exprs.get(index);
    if (!expr) throw new Error(`${mutationSource}: proof expression ${index} is missing`);
    if (expr.app) stack.push(expr.app.fn, expr.app.arg);
    else if (expr.lam) stack.push(expr.lam.type, expr.lam.body);
    else if (expr.forallE) stack.push(expr.forallE.type, expr.forallE.body);
    else if (expr.letE) stack.push(expr.letE.type, expr.letE.value, expr.letE.body);
    else if (expr.mdata) stack.push(expr.mdata.expr);
    else if (expr.proj) stack.push(expr.proj.struct);
  }
  const targetExpression = 943;
  if (!reachable.has(targetExpression)) throw new Error(`${mutationSource}: mutation is outside ${SMALL_ROOT}'s proof`);
  const hits = lines.map((line, index) => ({ line, index }))
    .filter(({ line }) => line !== '')
    .filter(({ line }) => {
      const record = JSON.parse(line);
      return record.ie === targetExpression && record.app?.arg === 396 && record.app.fn === 507;
    });
  if (hits.length !== 1) throw new Error(`${mutationSource}: expected exactly one mutation target, found ${hits.length}`);
  const record = JSON.parse(hits[0].line);
  record.app.arg = 394;
  lines[hits[0].index] = JSON.stringify(record);
  const file = path.join(work, 'semantic-mutation.ndjson');
  fs.writeFileSync(file, lines.join('\n'));
  return {
    file,
    line: hits[0].index + 1,
    expression: targetExpression,
    before: '{"app":{"arg":396,"fn":507},"ie":943}',
    after: JSON.stringify(record),
    derived_digest: fileSha256(file),
  };
}

/** Adds one untrusted axiom from an export's own Sort expression and fresh name indices. */
function deriveCustomAxiom(sourcePath) {
  const lines = fs.readFileSync(sourcePath, 'utf8').split('\n').filter((line) => line !== '');
  const records = lines.map((line) => JSON.parse(line));
  const parameters = new Map(records.filter((record) => 'il' in record && 'param' in record)
    .map((record) => [record.il, record.param]));
  const sort = records.find((record) => 'ie' in record && 'sort' in record && parameters.has(record.sort));
  if (!sort) throw new Error(`${sourcePath}: no Sort u expression for the custom axiom control`);
  const fresh = Math.max(...records.filter((record) => 'in' in record).map((record) => record.in)) + 1;
  const [space, leaf] = CUSTOM_AXIOM.split('.');
  const appended = [
    { in: fresh, str: { pre: 0, str: space } },
    { in: fresh + 1, str: { pre: fresh, str: leaf } },
    { axiom: { name: fresh + 1, levelParams: [parameters.get(sort.sort)], type: sort.ie, isUnsafe: false } },
  ];
  const file = path.join(work, 'custom-axiom.ndjson');
  fs.writeFileSync(file, `${[...lines, ...appended.map((record) => JSON.stringify(record))].join('\n')}\n`);
  return { file, sha256: fileSha256(file), axiom: CUSTOM_AXIOM };
}
const semantic = semanticMutation();
const customAxiom = deriveCustomAxiom(small);

// The closure digest of the unmutated mutation source, so "the mutation changed the digest" is a
// comparison on one artifact.
const mutationBaseline = (() => {
  const parsed = parseExport(mutationSource);
  const { reachable, missing } = closureOf(parsed, [SMALL_ROOT]);
  return { reachable: reachable.size, missing: [...missing].sort(), digest: closureDigest(parsed, reachable) };
})();

const mutations = [
  { id: 'semantic-mutation', kind: 'mutation', roots: [SMALL_ROOT], file: semantic.file,
    changes: `one root-proof argument: expression ${semantic.expression}, record ${semantic.line}, arg 396 -> 394`,
    expected: 'every checker refuses; the lane is never accepted' },
  { id: 'dropped-theorem', kind: 'mutation', roots: [SMALL_ROOT],
    file: dropDeclaration('dropped-theorem', small, DROPPED_THEOREM, work),
    changes: `the retained export without the declaration of ${DROPPED_THEOREM}`,
    expected: 'incomplete-closure at the lane; the checkers refuse too' },
  { id: 'dropped-definition', kind: 'mutation', roots: [SMALL_ROOT],
    file: dropDeclaration('dropped-definition', small, DROPPED_DEFINITION, work),
    changes: `the retained export without the declaration of ${DROPPED_DEFINITION}`,
    expected: 'incomplete-closure at the lane; the checkers refuse too' },
  { id: 'substituted-valid-claim', kind: 'mutation', roots: [LARGE_ROOT],
    file: renameTheorem('substituted-valid-claim', freshExport, FRESH_ROOT, LARGE_ROOT, work),
    changes: `the fresh ${FRESH_ROOT} export with its theorem renamed ${LARGE_ROOT}`,
    expected: 'claim-mismatch at the lane; every checker accepts it' },
];

// A format-version mutation: the checkers do not enforce the lane's format pin, so this control
// demonstrates the lane-level audit rather than a checker verdict.
const formatMutation = (() => {
  const text = fs.readFileSync(small, 'utf8');
  const newline = text.indexOf('\n');
  const meta = JSON.parse(text.slice(0, newline));
  if (!meta.meta?.format?.version) throw new Error('the small export does not start with a meta record');
  meta.meta.format.version = '3.2.0';
  const file = path.join(work, 'format-3.2.0.ndjson');
  fs.writeFileSync(file, JSON.stringify(meta) + text.slice(newline));
  return { file, before: EXPECTED_FORMAT, after: '3.2.0' };
})();
mutations.push({
  id: 'format-version-mutation', kind: 'format-mutation', roots: [SMALL_ROOT], file: formatMutation.file,
  changes: `the export's meta format version: ${formatMutation.before} -> ${formatMutation.after}`,
  expected: 'unsupported-format at the lane regardless of the checker verdict',
});
mutations.push({
  id: 'custom-axiom-untrusted', kind: 'axiom-mutation', roots: [SMALL_ROOT], file: customAxiom.file,
  changes: `the retained export with untrusted axiom ${CUSTOM_AXIOM}; checkers use their normal policy`,
  expected: 'forbidden-axiom at the lane regardless of the checker verdict',
});
mutations.push({
  id: 'custom-axiom-permitted-to-checker', kind: 'axiom-mutation', roots: [SMALL_ROOT], file: customAxiom.file,
  config: { permitted_axioms: [...STANDARD_AXIOMS, CUSTOM_AXIOM] },
  changes: `the retained export with ${CUSTOM_AXIOM} explicitly permitted to configurable checkers`,
  expected: 'forbidden-axiom at the lane even if a checker accepts it',
});

const controls = mutations.map((control) => subjectEntry(control, control.file, { retained: false, producer: 'derived' }));

const RESOURCE_ADDRESS_SPACE_BYTES = 1024 * 1024;
const resourceControl = subjectEntry({
  id: 'resource-exhaustion', kind: 'resource-control', roots: [LARGE_ROOT],
  changes: `the large pinned export under RLIMIT_AS=${RESOURCE_ADDRESS_SPACE_BYTES} bytes`,
  expected: 'resource-exhausted',
  runOptions: () => ({ addressSpaceBytes: RESOURCE_ADDRESS_SPACE_BYTES }),
}, large, { retained: false, producer: 'pinned large export', address_space_limit_bytes: RESOURCE_ADDRESS_SPACE_BYTES });

/** Invalid NDJSON is parsed independently before each checker outcome is attributed. */
function parseFailureControl() {
  const file = path.join(work, 'parse-failure.ndjson');
  fs.writeFileSync(file, '{"meta":\n');
  let parserError = null;
  try {
    parseExport(file);
  } catch (error) {
    parserError = String(error);
  }
  if (parserError === null) throw new Error('the lane parser accepted the malformed NDJSON control');
  const runs = checkersPin.checkers.map((checker) => ({
    ...runChecker(checker, file, 'parse-failure'),
    lane: { verdict: 'parse-failure', reasons: [parserError] },
  }));
  return {
    id: 'parse-failure', kind: 'parse-control', expected: 'parse-failure', changes: 'malformed first NDJSON record',
    export: { sha256: fileSha256(file), size_bytes: fs.statSync(file).size, lane_parser_error: parserError },
    runs,
    quorum: {
      accepted_by: runs.filter((run) => run.classification === 'accepted').map((run) => run.checker),
      refused_by: runs.filter((run) => run.classification !== 'accepted').map((run) => `${run.checker}:${run.classification}`),
      accepted: false,
    },
  };
}

/** Exposes an unsupported policy control as unsupported rather than a successful skip. */
function extensionControl(id, extension) {
  const option = `${extension}_extension`;
  const runs = checkersPin.checkers.map((checker) => {
    if (checker.run.kind !== 'nanoda-config') {
      return {
        checker: checker.id, attempted: false, exit_code: null, signal: null,
        classification: 'unsupported', unsupported_reason: `no ${option} configuration interface`,
        lane: { verdict: 'unsupported', reasons: [`checker cannot express ${option}=false`] },
      };
    }
    const result = runChecker(checker, large, id, { configOverrides: { [option]: false } });
    return { ...result, lane: { verdict: result.classification } };
  });
  return {
    id, kind: 'capability-control', expected: 'unsupported',
    changes: `${option}=false; checkers without this configuration are recorded unsupported`,
    input: { root: LARGE_ROOT, sha256: pins.large },
    runs,
    quorum: {
      accepted_by: runs.filter((run) => run.classification === 'accepted').map((run) => run.checker),
      refused_by: runs.filter((run) => run.classification !== 'accepted').map((run) => `${run.checker}:${run.classification}`),
      accepted: false,
    },
  };
}

const operationalControls = [
  resourceControl,
  parseFailureControl(),
  extensionControl('unsupported-nat-literal', 'nat'),
  extensionControl('unsupported-string-literal', 'string'),
];
// The same refusal-class mapping is checked for every configured checker; real and synthetic
// unsupported outcomes remain separately identified in the receipt.
const classifierCases = [
  { id: 'declined-exit', input: { status: 2, stdout: '', stderr: 'declined' }, expected: 'unsupported' },
  { id: 'unsupported-feature', input: { status: 1, stdout: '', stderr: 'Nat literal extension disallowed' }, expected: 'unsupported' },
  { id: 'kernel-rejection', input: { status: 1, stdout: '', stderr: 'rejected: application type mismatch' }, expected: 'kernel-rejection' },
  { id: 'parse-failure', input: { status: 3, stdout: '', stderr: "parse error: expected ':' after a key" }, expected: 'parse-failure' },
  { id: 'resource-exhaustion', input: { status: 137, stdout: '', stderr: '' }, expected: 'resource-exhausted' },
  { id: 'unparseable-zero-exit', input: { status: 0, stdout: '', stderr: '' }, expected: 'checker-error' },
  { id: 'empty-check', input: { status: 0, stdout: 'Checked 0 declarations with no errors', stderr: '' }, expected: 'empty-check' },
];
const classifierSelfTest = checkersPin.checkers.flatMap((checker) => classifierCases.map((row) => {
  const classified = classify({ ...row.input, signal: null }, checker, false);
  return { checker: checker.id, ...row, classified: classified.classification };
}));

const baselineRuns = baselines.flatMap((subject) => subject.runs.map((run) => ({ subject: subject.id, ...run })));
const mutationRuns = controls.flatMap((subject) => subject.runs.map((run) => ({ subject: subject.id, ...run })));
const operationalRuns = operationalControls.flatMap((subject) => subject.runs.map((run) => ({ subject: subject.id, ...run })));
const allExecutedRuns = [...baselineRuns, ...mutationRuns, ...operationalRuns].filter((run) => run.attempted !== false);
const proofMutations = controls.filter((control) => control.id === 'semantic-mutation');
const proofMutationRuns = proofMutations.flatMap((control) => control.runs.map((run) => ({ subject: control.id, ...run })));
const checkerIds = checkersPin.checkers.map((checker) => checker.id);
const languageEvidenceRows = new Map(checkersPin.checkers.map((checker) => [checker.id, languageEvidence(checker)]));
const independentDesigns = checkersPin.checkers.filter((checker) => checker.independent_checker_design)
  .map((checker) => checker.id).sort();
const subjectAndOperationalControls = [...controls, ...operationalControls];
const independentDesignLanguages = independentDesigns.map((id) => checkersPin.checkers.find((checker) => checker.id === id).language);

const checks = {
  manifest_matches_the_compiled_environment: manifestCheck.every((row) => row.matches),
  every_checker_language_marker_matches: checkersPin.checkers
    .every((checker) => languageEvidenceRows.get(checker.id).matched),
  independent_designs_include_three_distinct_languages:
    independentDesigns.join(',') === 'con-leche,nanoda,ocaml4lean'
    && new Set(independentDesignLanguages).size === independentDesignLanguages.length,
  con_ron_is_distinct_but_related: checkersPin.checkers.some((checker) =>
    checker.id === 'con-ron' && checker.independent_implementation && !checker.independent_checker_design),
  lean4lean_is_kernel_derived: checkersPin.checkers.some((checker) =>
    checker.id === 'lean4lean' && !checker.independent_of_lean_kernel && !checker.independent_checker_design),
  every_control_has_a_row_for_every_checker: subjectAndOperationalControls.every((control) =>
    control.runs.length === checkerIds.length
      && new Set(control.runs.map((run) => run.checker)).size === checkerIds.length
      && checkerIds.every((checker) => control.runs.some((run) => run.checker === checker))),
  every_baseline_subject_accepted_by_every_checker:
    baselines.every((subject) => subject.quorum.accepted),
  every_baseline_lane_verdict_accepted:
    baselineRuns.every((run) => run.lane.verdict === 'accepted'),
  every_accepted_run_has_a_positive_declaration_count:
    baselineRuns.filter((run) => run.classification === 'accepted').every((run) => run.count > 0)
    && baselineRuns.filter((run) => run.classification === 'accepted').length > 0,
  every_checker_run_has_wall_time_and_max_rss:
    allExecutedRuns.every((run) =>
      Number.isFinite(run.wall_seconds) && Number.isFinite(run.max_rss_kb) && run.max_rss_kb > 0),
  no_checker_accepted_a_proof_mutation:
    proofMutationRuns.every((run) => run.classification !== 'accepted'),
  semantic_mutation_refused_by_every_checker:
    controls.find((control) => control.id === 'semantic-mutation').runs.every((run) => run.classification === 'kernel-rejection'),
  dropped_dependencies_refused_by_the_lane:
    controls.filter((control) => control.id.startsWith('dropped-'))
      .every((control) => control.runs.every((run) =>
        run.lane.verdict === 'incomplete-closure' && run.classification !== 'accepted')),
  substituted_claim_accepted_by_every_checker:
    controls.find((control) => control.id === 'substituted-valid-claim')
      .runs.every((run) => run.classification === 'accepted'),
  substituted_claim_refused_by_statement_binding:
    controls.find((control) => control.id === 'substituted-valid-claim')
      .runs.every((run) => run.lane.verdict === 'claim-mismatch'),
  semantic_mutation_changed_the_closure_digest:
    controls.find((control) => control.id === 'semantic-mutation').runs[0].lane.closure.digest
      !== mutationBaseline.digest,
  format_mutation_refused_by_the_lane:
    controls.find((control) => control.id === 'format-version-mutation')
      .runs.every((run) => run.lane.verdict === 'unsupported-format'),
  format_mutation_shows_a_checker_divergence:
    new Set(controls.find((control) => control.id === 'format-version-mutation').runs.map((run) => run.classification)).size > 1,
  custom_axiom_control_refused_by_lane:
    controls.filter((control) => control.id.startsWith('custom-axiom-'))
      .every((control) => control.runs.every((run) => run.lane.verdict === 'forbidden-axiom')),
  configurable_checker_accepts_permitted_custom_axiom:
    controls.find((control) => control.id === 'custom-axiom-permitted-to-checker')
      .runs.some((run) => run.checker === 'nanoda' && run.classification === 'accepted'
        && run.config_overrides.permitted_axioms.includes(CUSTOM_AXIOM)),
  parse_failure_control_is_refused_by_every_checker:
    operationalControls.find((control) => control.id === 'parse-failure')
      .runs.every((run) => run.classification === 'parse-failure' && run.lane.verdict === 'parse-failure'),
  resource_limit_is_reported_as_resource_exhaustion_for_every_checker:
    resourceControl.runs.every((run) => run.classification === 'resource-exhausted' && run.lane.verdict === 'resource-exhausted'),
  unsupported_controls_are_explicit:
    operationalControls.filter((control) => control.id.startsWith('unsupported-'))
      .every((control) => control.runs.every((run) => run.classification === 'unsupported'
        && (run.attempted === true || (run.attempted === false && run.unsupported_reason !== undefined)))),
  classifier_preserves_all_refusal_classes_for_every_checker:
    classifierSelfTest.every((row) => row.classified === row.expected)
      && checkersPin.checkers.every((checker) => classifierSelfTest.some((row) => row.checker === checker.id && row.expected === 'kernel-rejection')
        && classifierSelfTest.some((row) => row.checker === checker.id && row.expected === 'parse-failure')
        && classifierSelfTest.some((row) => row.checker === checker.id && row.expected === 'unsupported')
        && classifierSelfTest.some((row) => row.checker === checker.id && row.expected === 'resource-exhausted')),
  no_mutation_lane_verdict_accepted: subjectAndOperationalControls.every((control) => control.quorum.accepted === false),
};
const holds = Object.values(checks).every(Boolean);

const declinedExitObserved = allExecutedRuns.some((run) => run.exit_code === 2);
const host = spawnSync('uname', ['-srm'], { encoding: 'utf8' }).stdout.trim();
const source = spawnSync('git', ['rev-parse', 'HEAD'], { cwd: process.cwd(), encoding: 'utf8' });
if (source.status !== 0 || source.stdout.trim() === '') {
  throw new Error(`cannot identify receipt source commit: ${source.stderr.trim()}`);
}
const summaries = checkersPin.checkers.map((checker) => {
  const runs = [...baselineRuns, ...mutationRuns, ...operationalRuns].filter((run) => run.checker === checker.id);
  return {
    checker: checker.id,
    language: checker.language,
    implementation_family: checker.implementation_family,
    independent_of_lean_kernel: checker.independent_of_lean_kernel,
    independent_implementation: checker.independent_implementation,
    independent_checker_design: checker.independent_checker_design,
    language_evidence: languageEvidenceRows.get(checker.id),
    source_evidence: checker.source_evidence,
    binary_source_revision: checker.binary_source_revision,
    accepted_runs: runs.filter((run) => run.classification === 'accepted').length,
    run_records: runs.length,
    attempted_runs: runs.filter((run) => run.attempted !== false).length,
    wall_seconds_total: Number(runs.reduce((sum, run) => sum + (run.wall_seconds ?? 0), 0).toFixed(2)),
    max_rss_kb_peak: runs.reduce((peak, run) => Math.max(peak, run.max_rss_kb ?? 0), 0),
    classifications: runs.reduce((counts, run) => {
      counts[run.classification] = (counts[run.classification] ?? 0) + 1;
      return counts;
    }, {}),
  };
});

const receipt = {
  schema: 'noble-independent-replay-multi/v2',
  measured_on: new Date().toISOString().slice(0, 10),
  host,
  source_commit: source.stdout.trim(),
  toolchain,
  execution_tools: {
    time: { path: timeBin, sha256: fileSha256(timeBin) },
    prlimit: { path: prlimitBin, sha256: fileSha256(prlimitBin) },
  },
  assertion: 'every pinned checker accepted both named closures and the fresh Init closure under the strict rule; every checker row records semantic-mutation and dropped-dependency outcomes, parse and resource failures are distinguished from kernel rejection and unsupported controls, the substituted valid claim was refused by statement binding, custom axioms were refused by the lane, and every executed run records wall time and max RSS',
  holds,
  checks,
  pin_guard_passed: true,
  harness: {
    multi: { path: 'verification/independent-replay/multi.mjs', sha256: fileSha256(fileURLToPath(import.meta.url)) },
    checkers: { path: 'verification/independent-replay/checkers.json', sha256: fileSha256(checkersPath) },
    ocaml_derivation: { path: 'verification/independent-replay/ocaml4lean.nix', sha256: fileSha256(path.join(here, 'ocaml4lean.nix')) },
    binding_core: { path: 'verification/independent-replay/binding-core.mjs', sha256: fileSha256(path.join(here, 'binding-core.mjs')) },
    ndjson: { path: 'verification/independent-replay/ndjson.mjs', sha256: fileSha256(path.join(here, 'ndjson.mjs')) },
    pins: { path: 'verification/independent-replay/pins.mjs', sha256: fileSha256(path.join(here, 'pins.mjs')) },
    statements: { path: 'verification/independent-replay/statements.lean', sha256: fileSha256(path.join(here, 'statements.lean')) },
    manifest: { path: 'verification/independent-replay/statements.json', sha256: fileSha256(manifestPath) },
  },
  acceptance_rule: [
    'exporter exit status zero and no exporter panic (for the fresh export)',
    'every requested root declared in the export',
    'every constant the roots reference, transitively, declared in the export',
    'every root statement rendered from the export equal to its pinned statement digest',
    `the export's declared meta format version equal to ${EXPECTED_FORMAT}`,
    `every declared axiom in ${STANDARD_AXIOMS.join(', ')}`,
    'the checker exits zero with a positive checked-declaration count',
  ],
  toolchain_pins: {
    lean: { binary_sha256: pins.lean },
    lake: { binary_sha256: checkersPin.toolchain.lake_sha256 },
    leanexport: { binary_sha256: checkersPin.toolchain.leanexport_sha256 },
    release: checkersPin.toolchain.output,
    derivation: checkersPin.toolchain.derivation,
    policy: checkersPin.toolchain.policy,
    checkers: checkersPin.checkers.map((checker) => ({
      id: checker.id, binary: checker.binary, binary_sha256: checker.sha256,
      binary_source_revision: checker.binary_source_revision,
      binary_arg: checker.binary_arg ?? null,
      language: checker.language,
      independent_of_lean_kernel: checker.independent_of_lean_kernel,
      independent_implementation: checker.independent_implementation,
      independent_checker_design: checker.independent_checker_design,
      independence: checker.independence,
    })),
  },
  statement_source: {
    extraction_elapsed_ms: extractionMs,
    manifest_check: manifestCheck,
  },
  inputs: {
    large: { root: LARGE_ROOT, sha256: pins.large, retained: false },
    small: { root: SMALL_ROOT, sha256: pins.small, retained: true, path: smallRetainedAt },
    mutation_source: { sha256: pins.mutation, retained: true, path: mutationRetainedAt },
    fresh: { root: FRESH_ROOT, sha256: fileSha256(freshExport), retained: false, stage: freshStage },
  },
  subjects: baselines,
  controls,
  operational_controls: operationalControls,
  semantic_mutation: semantic,
  semantic_mutation_baseline_closure: mutationBaseline,
  classifier_self_test: classifierSelfTest,
  declined_exit_observed: declinedExitObserved,
  unsupported_classification_observed: allExecutedRuns.some((run) => run.classification === 'unsupported'),
  checker_summaries: summaries,
  observations: {
    declaration_count_spread_on_large: Object.fromEntries(
      baselines.find((subject) => subject.id === 'baseline-large').runs
        .map((run) => [run.checker, run.count]),
    ),
    independence: checkersPin.checkers.map((checker) => ({
      checker: checker.id,
      independent_of_lean_kernel: checker.independent_of_lean_kernel,
      independent_implementation: checker.independent_implementation,
      independent_checker_design: checker.independent_checker_design,
      implementation_family: checker.implementation_family,
      note: checker.independence,
    })),
    divergences: subjectAndOperationalControls
      .filter((control) => new Set(control.runs.map((run) => run.classification)).size > 1)
      .map((control) => ({
        control: control.id,
        by_checker: Object.fromEntries(control.runs.map((run) => [run.checker, run.classification])),
      })),
  },
  non_claims: [
    'Acceptance by several checkers says nothing about any checker\'s correctness, completeness or equivalence to the Lean kernel.',
    'con-ron is con-leche\'s Rust twin: agreement between them is not independent corroboration.',
    'lean4lean is derived from the C++ kernel and is counted as lower diversity, never as an independent implementation.',
    'OCaml4Lean is built from its pinned upstream source revision with the source-pinned Nix derivation; the other checkers remain release-bundled.',
    'Only the three bound subjects were replayed; this is not a full NobleFoundation replay.',
    'No checker emitted exit 2 in this run; every checker maps that exit to unsupported in the classifier self-test.',
    'No CI, flake, policy or gate change; promotion is an owner decision.',
  ],
};
fs.mkdirSync(path.dirname(output), { recursive: true });
fs.writeFileSync(output, `${JSON.stringify(receipt, null, 2)}\n`);
console.log(JSON.stringify({
  holds,
  checks,
  quorum: Object.fromEntries([...baselines, ...controls].map((subject) => [subject.id, subject.quorum.accepted])),
  declined_exit_observed: declinedExitObserved,
}));
process.exitCode = holds ? 0 : 1;
