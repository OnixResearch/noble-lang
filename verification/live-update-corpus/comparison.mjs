/**
 * Relax comparison for the bounded live-update corpus (#119).
 *
 * compiler-lab's `validate relax-live` lane runs Rust counterparts of the Noble scenarios under the
 * pinned Makepad Relax host against the exact bytes of a revision-1 Noble export, classifies them
 * under its own frozen Relax profile, and writes one `noble_columns` entry per Noble case. This
 * module turns that run into the revision-2 comparison corpus and verifies it. Three layers stay
 * separate:
 *
 *   retention     the compiler-lab report, scenario records, raw streams and replay report are kept
 *                 with local paths replaced by named placeholders. Each retained file names the
 *                 digest of its retained bytes and of the bytes compiler-lab wrote. Fixture sources,
 *                 reference executables and the profile text stay in the private repository and
 *                 are bound by digest only (`WITHHELD`).
 *   derivation    `deriveComparison` is a pure function of the export, the retained reports and the
 *                 retained-file list. `verifyComparison` re-runs it, so the corpus cannot state a
 *                 column that the retained compiler-lab material does not contain.
 *   binding       every column must agree with the export it was computed against, the frozen
 *                 Noble and Relax profiles, the pinned Relax host, its record files, its edit
 *                 digests, its timing and a reproduced replay.
 *
 * compiler-lab derives and verifies the Relax observations themselves (its controls and
 * `--replay`). This module checks that the retained evidence agrees with itself and with Noble's
 * export; it never reclassifies a Relax outcome as a Noble contract violation or the reverse.
 */
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {
  COMPARISON_RECEIPT_SCHEMA, COMPARISON_SCHEMA, MEASURED_CLASSES, OTHER_RUNTIME,
  PROFILE_PATH, RESULT_CLASS_IDS, digestValue, fileSha256, readJson, sha256, stableJson,
  validateCorpus, verifyEvidence,
} from './corpus.mjs';

/** The compiler-lab revision whose Relax lane this comparison accepts, frozen before the run. */
export const COMPILER_LAB = Object.freeze({
  repository: 'brittonr/compiler-lab',
  commit: '39fdff1592d0e1ae6c378958efa8a206764203be',
  visibility: 'private',
});

/** The frozen Relax profile at that revision; its text stays in compiler-lab. */
export const RELAX_PROFILE = Object.freeze({
  id: 'compiler-lab-relax-live-profile/relax-5441582-v1',
  path: 'src/validation/relax_live/profile.json',
  sha256: '7acb97fbec6ee1fe225f0ec1ae1b889cdcf0dd1cf45e1aadb71cb104a65cbb1a',
});

/** The pinned Relax host that `nix build .#relax-host` produces at that revision. */
export const RELAX_HOST = Object.freeze({
  source_repository: 'https://github.com/makepad/relax',
  source_revision: '544158264603dbf90994ac47fc97a2636437b656',
  executable_sha256: 'fed38792ec30d29829fae808398f39444008c03f6e90cf2eb4418a711738e260',
  manifest_sha256: 'aca8125ac31a1a839c1679c7256ecf8a339bb9758bee91d8b617c87cb4fce41d',
  reported_version: 'makepad run makepad d08642ed5eee jit 7c8930659157',
  std_tree_sha256: '73077222c2c40e616d2a58bb14921ebb9cd6f78cd1e7c96be4ddd30e64ba7c25',
});

/** The compiler-lab evidence controls that must have run and passed in the run and the replay. */
export const RELAX_CONTROLS = Object.freeze(['unchanged-evidence', 'wrong-version',
  'stale-evidence', 'premature-retirement', 'event-binding', 'profile']);

/** Placeholders that replace local paths in retained compiler-lab files. */
export const PLACEHOLDERS = Object.freeze({
  run: '<compiler-lab-run>',
  replay: '<compiler-lab-replay>',
  export: '<noble-export>',
  checkout: '<compiler-lab-checkout>',
  home: '<home>',
});

/**
 * Patterns that must not survive in a retained compiler-lab file: local home, cache, scratch and
 * temporary directories. The importer also refuses the producing host's name, which a later
 * verifier cannot know.
 */
export const PRIVATE_PATTERNS = Object.freeze([
  /\/home\//u, /\/root\//u, /\/dev\/shm\//u, /\/var\/tmp\//u, /(?:^|[\s"'=:(])\/tmp\//u,
  /\/\.cache\//u,
]);

/**
 * Rendered compiler diagnostics quote fixture source lines after a numbered gutter: a line number,
 * a space and `|`, then the source text. The fixtures are private compiler-lab sources, so the
 * quoted text after the gutter is replaced by this marker; the line number, location, message,
 * caret and label lines stay.
 */
export const SOURCE_WITHHELD = '<fixture source line withheld>';
const GUTTER_LINE = /^(\s*\d+ \|)(.*)$/u;

const redactLines = text => {
  let count = 0;
  const lines = text.split('\n').map(line => {
    const match = GUTTER_LINE.exec(line);
    if (match === null || match[2] === '' || match[2] === ` ${SOURCE_WITHHELD}`)
      return line;
    count += 1;
    return `${match[1]} ${SOURCE_WITHHELD}`;
  });
  return { text: lines.join('\n'), count };
};

/**
 * Withhold quoted fixture source in a retained file. JSON and JSON Lines files are rewritten one
 * string literal at a time, so every literal without a quoted source line keeps its exact bytes;
 * other files are rewritten line by line.
 */
export function redactSourceExcerpts(relative, text) {
  if (!/\.jsonl?$/u.test(relative)) return redactLines(text);
  let count = 0;
  const result = text.replace(/"(?:[^"\\]|\\.)*"/gu, literal => {
    const redacted = redactLines(JSON.parse(literal));
    if (redacted.count === 0) return literal;
    count += redacted.count;
    return JSON.stringify(redacted.text);
  });
  return { text: result, count };
}

/** Whether a retained file still quotes a fixture source line. */
export const quotesSource = (relative, text) => redactSourceExcerpts(relative, text).count > 0;

/** compiler-lab material that is deliberately not retained, and why. */
export const WITHHELD = Object.freeze([
  { what: 'scenarios/*/*/source/*.rs',
    reason: 'the Rust fixture versions are private compiler-lab sources; each record binds every version by SHA-256 and the event binding digests their order' },
  { what: 'fixture source lines quoted by rendered compiler diagnostics',
    reason: `private compiler-lab source; the text after a numbered gutter is replaced by ${SOURCE_WITHHELD} and counted in the retained file's substitutions, while the diagnostic's code, message, location, carets and labels stay` },
  { what: 'scenarios/*/*/reference/*',
    reason: 'rustc-built reference executables; each record keeps the reference outcome, its values and the declared expectation' },
  { what: 'scenarios/*/*/work/*',
    reason: 'Relax driver scratch; the driver live log is retained as live-log.jsonl wherever the record binds one' },
  { what: 'profile.json',
    reason: 'the frozen Relax profile is a private compiler-lab source; the corpus binds its id, path, SHA-256 and commit, and every run checks its upstream citations against the pinned Relax source' },
  { what: 'commands/*',
    reason: 'per-process command records with local paths and environments; the records keep each command label, outcome and output values' },
  { what: 'noble/corpus.json',
    reason: 'byte copy of the consumed export; the importer checks that it equals export/corpus.json' },
  { what: 'toolchain.json and storage.json',
    reason: 'the reference toolchain identity is part of report.json; storage accounting is host-specific' },
  { what: 'the replay run scenario directories and replay/retained-report.json',
    reason: 'the replay report keeps every replayed record inline, and the importer checks that the retained-report copy equals the original report' },
]);

export const COMPARISON_NON_CLAIMS = Object.freeze([
  'this comparison is experimental evidence; it is not a proof, a conformance result or a model-to-implementation correspondence claim',
  'the authoritative Noble lanes keep their own claims; the export binds their bytes and reproduces their observations',
  'no LSLOT-01..09 canonical case state is promoted and no evidence is attached to a canonical case',
  'each runtime is classified under its own frozen profile; an intentional semantic difference is a defect of neither runtime, and nothing here requires Relax to implement Noble root-pinned selection',
  'a stored Rust closure or fn pointer is not equated with an immutable Noble Program beyond the faithful mapping each column states',
  'a failed Relax checked-before-execution policy is an observation of that Relax admission setting, not a Noble defect',
  'compiler-lab derives and replay-verifies the Relax observations; this corpus binds and cross-checks them and does not re-derive them from the raw streams',
  'timing and memory samples are bounded observations bound to their scenario, revision and admission outcome, not benchmarks',
]);

const RUN_FILES = Object.freeze([
  { path: 'report.json', role: 'report' },
  { path: 'request.json', role: 'request' },
  { path: 'controls.json', role: 'controls' },
  { path: 'identity.json', role: 'identity' },
  { path: 'pins.json', role: 'pins' },
]);
const REPLAY_FILES = Object.freeze([
  { path: 'report.json', role: 'report' },
  { path: 'request.json', role: 'request' },
  { path: 'controls.json', role: 'controls' },
]);
const STREAM_ROLES = Object.freeze(['live-stdout', 'live-stderr', 'live-log']);
const isObject = value => value !== null && typeof value === 'object' && !Array.isArray(value);
const pick = (value, keys) => Object.fromEntries(keys.map(key => [key, value?.[key] ?? null]));
const admissionLabel = admission => (admission === 'check-first' ? 'check-first' : 'default');

/** The digest compiler-lab gives an ordered edit list: canonical JSON of [version, sha256] pairs. */
export const eventBinding = edits => sha256(JSON.stringify((edits ?? [])
  .map(edit => [edit?.version, edit?.artifact?.sha256])));

/** Replace every literal with its placeholder; returns the retained bytes and per-placeholder counts. */
export function scrub(bytes, substitutions) {
  const text = bytes.toString('utf8');
  assert.ok(Buffer.from(text, 'utf8').equals(bytes), 'a retained compiler-lab file is not UTF-8');
  let result = text;
  const counts = {};
  for (const [literal, placeholder] of substitutions) {
    const parts = result.split(literal);
    counts[placeholder] = (counts[placeholder] ?? 0) + parts.length - 1;
    result = parts.join(placeholder);
  }
  return { bytes: Buffer.from(result, 'utf8'), counts };
}

export const privateMatches = text => PRIVATE_PATTERNS.filter(pattern => pattern.test(text))
  .map(pattern => pattern.source);

/** The compiler-lab files a run directory contributes: fixed files plus every record's streams. */
export function retentionPlan(report) {
  const plan = RUN_FILES.map(row => ({ ...row, source: 'run' }));
  for (const record of report.scenarios ?? []) {
    const directory = `scenarios/${record.scenario}/${admissionLabel(record.admission)}`;
    plan.push({ path: `${directory}/record.json`, role: 'record', source: 'run' });
    for (const provenance of record.provenance ?? [])
      if (STREAM_ROLES.includes(provenance.role))
        plan.push({ path: provenance.artifact.path, role: provenance.role, source: 'run' });
  }
  plan.push(...REPLAY_FILES.map(row => ({ ...row, source: 'replay' })));
  return plan;
}

/** Read the inputs of a comparison from an evidence directory and its retained-file list. */
export function loadInputs(directory, retained) {
  const read = relative => fs.readFileSync(path.join(directory, relative));
  const json = relative => JSON.parse(read(relative).toString('utf8'));
  const exportCorpusBytes = read('export/corpus.json');
  const exportReceiptBytes = read('export/receipt.json');
  return {
    exportCorpus: JSON.parse(exportCorpusBytes.toString('utf8')),
    exportReceipt: JSON.parse(exportReceiptBytes.toString('utf8')),
    digests: {
      corpus: sha256(exportCorpusBytes),
      receipt: sha256(exportReceiptBytes),
      profile: sha256(read('export/profile.json')),
      report: sha256(read('relax/run/report.json')),
      replay: sha256(read('relax/replay/report.json')),
    },
    report: json('relax/run/report.json'),
    request: json('relax/run/request.json'),
    replay: json('relax/replay/report.json'),
    records: new Map((retained ?? []).filter(entry => entry.role === 'record')
      .map(entry => [entry.path, json(entry.path)])),
    retained,
  };
}

const relation = row => pick(row, ['dimension', 'noble', 'relax', 'classification_rule',
  'relation', 'observed', 'basis']);

function relaxSetting(setting, index) {
  const retainedPath = `relax/run/${setting?.record?.path}`;
  return {
    admission: setting?.admission ?? null,
    result_class: setting?.result_class ?? null,
    relax_contract: setting?.relax_contract ?? null,
    temporal_admission: setting?.temporal_admission ?? null,
    event_binding_sha256: setting?.event_binding_sha256 ?? null,
    record: {
      path: retainedPath,
      sha256: index.get(retainedPath)?.sha256 ?? null,
      original_path: setting?.record?.path ?? null,
      original_sha256: setting?.record?.sha256 ?? null,
    },
  };
}

/** One revision-2 column: a faithful transcription of the compiler-lab column for this case. */
function relaxColumn(scenario, column, profile, index) {
  const declared = scenario.other_runtime?.anticipated_differences ?? [];
  if (column === undefined)
    return { name: OTHER_RUNTIME.name, profile, result_class: 'not-run',
      reason: 'compiler-lab reported no makepad-relax column for this case',
      unsupported_dimensions: [], anticipated_differences: declared, admission_settings: [] };
  const measured = MEASURED_CLASSES.includes(column.result_class);
  return {
    name: column.name ?? null,
    ...(column.scenario === undefined ? {} : { scenario: column.scenario }),
    profile,
    result_class: column.result_class ?? null,
    reason: column.reason ?? null,
    ...(column.faithful_mapping === undefined ? {} : { faithful_mapping: column.faithful_mapping }),
    unsupported_dimensions: column.unsupported_dimensions ?? [],
    anticipated_differences: measured
      ? (column.anticipated_differences ?? []).map(relation) : declared,
    admission_settings: (column.admission_settings ?? []).map(setting => relaxSetting(setting, index)),
  };
}

/** The revision-2 corpus: the export with each makepad-relax column and the comparison binding. */
export function deriveComparison(inputs, generatedAt) {
  const { exportCorpus, exportReceipt, digests, report, request, replay, retained } = inputs;
  const index = new Map((retained ?? []).map(entry => [entry.path, entry]));
  const profile = { ...pick(report.profile, ['id', 'path', 'sha256']),
    repository: COMPILER_LAB.repository, commit: COMPILER_LAB.commit };
  const columns = new Map((report.noble_columns ?? []).map(column => [column.case_id, column]));
  const manifest = report.identity?.manifest;
  return {
    ...exportCorpus,
    schema: COMPARISON_SCHEMA,
    generated_at: generatedAt,
    scenarios: (exportCorpus.scenarios ?? []).map(scenario => ({
      ...scenario,
      other_runtime: relaxColumn(scenario, columns.get(scenario.case_id), profile, index),
    })),
    non_claims: [...COMPARISON_NON_CLAIMS],
    comparison: {
      export: {
        corpus: { path: 'export/corpus.json', sha256: digests.corpus },
        receipt: { path: 'export/receipt.json', sha256: digests.receipt,
          binding_sha256: exportReceipt.binding_sha256 ?? null },
        profile: { path: 'export/profile.json', sha256: digests.profile },
        source_commit: exportCorpus.source?.commit ?? null,
      },
      relax: {
        runtime: OTHER_RUNTIME.name,
        lane: 'validate relax-live',
        compiler_lab: { ...COMPILER_LAB },
        profile,
        host: {
          source_repository: manifest?.source_repository ?? null,
          source_revision: manifest?.source_revision ?? null,
          executable_sha256: report.identity?.executable_sha256 ?? null,
          manifest_sha256: report.identity?.manifest_sha256 ?? null,
          reported_version: report.identity?.reported_version ?? null,
          std_tree_sha256: report.identity?.std_tree_sha256 ?? null,
        },
        options: { admission: request.admission ?? null,
          toolchain: request.common?.toolchain ?? null, timeout_ms: request.common?.timeout_ms ?? null },
        report: { path: 'relax/run/report.json', outcome: report.outcome ?? null,
          identity_unchanged: report.identity_unchanged ?? null },
        replay: { path: 'relax/replay/report.json', outcome: replay.outcome ?? null,
          relations: (replay.replay ?? []).map(row => pick(row, ['scenario', 'admission', 'relation',
            'retained_event_binding_sha256', 'event_binding_sha256'])) },
        controls: (report.controls ?? []).map(row => pick(row, ['name', 'expected_reason',
          'observed_reason', 'passed'])),
        temporal_admission: (report.temporal_admission ?? []).map(row => pick(row, ['scenario',
          'admission', 'policy', 'outcome', 'detail'])),
        noble_columns_sha256: digestValue(report.noble_columns ?? null),
      },
      retained: [...(retained ?? [])].sort((left, right) => left.path.localeCompare(right.path, 'en')),
      withheld: WITHHELD.map(row => ({ ...row })),
    },
  };
}

/** Per-case summary of both columns, as the receipt records it. */
export function comparisonSummary(corpus) {
  return (corpus.scenarios ?? []).map(scenario => {
    const column = scenario.other_runtime ?? {};
    return {
      case_id: scenario.case_id,
      title: scenario.title,
      noble: { result_class: scenario.comparison?.result_class ?? null,
        unsupported_dimensions: scenario.comparison?.unsupported_dimensions ?? [] },
      relax: {
        result_class: column.result_class ?? null,
        scenario: column.scenario ?? null,
        reason: column.reason ?? null,
        faithful_mapping: column.faithful_mapping ?? null,
        unsupported_dimensions: column.unsupported_dimensions ?? [],
        relations: (column.anticipated_differences ?? []).map(row => ({
          dimension: row.dimension, relation: row.relation ?? 'not-measured' })),
        admission_settings: (column.admission_settings ?? []).map(setting => pick(setting,
          ['admission', 'result_class', 'relax_contract', 'temporal_admission'])),
      },
    };
  });
}

/** One Noble cell and one makepad-relax cell per case, plus one count per Relax admission run. */
export function comparisonCounts(corpus) {
  const zero = () => Object.fromEntries(RESULT_CLASS_IDS.map(id => [id, 0]));
  const counts = { noble: zero(), relax: zero(), relax_settings: zero() };
  for (const scenario of corpus.scenarios ?? []) {
    const noble = scenario.comparison?.result_class;
    const relax = scenario.other_runtime?.result_class;
    if (Object.hasOwn(counts.noble, noble)) counts.noble[noble] += 1;
    if (Object.hasOwn(counts.relax, relax)) counts.relax[relax] += 1;
    for (const setting of scenario.other_runtime?.admission_settings ?? [])
      if (Object.hasOwn(counts.relax_settings, setting.result_class))
        counts.relax_settings[setting.result_class] += 1;
  }
  return counts;
}

/** Relax timing samples, each bound to its case, setting, revision, profile, edits and outcome. */
export function relaxTimings(inputs) {
  const rows = [];
  for (const [retainedPath, record] of [...inputs.records.entries()]
    .sort(([left], [right]) => left.localeCompare(right, 'en')))
    rows.push({
      record: retainedPath,
      case_id: record.case_id ?? null,
      admission: record.admission ?? null,
      duration_ms: record.timing?.duration_ms ?? null,
      run_milliseconds: record.timing?.run_milliseconds ?? null,
      bound_to: record.timing?.bound_to ?? null,
    });
  return rows;
}

/** Issue item 10 for the comparison, derived from the classes and the Relax policy outcomes. */
export function issueTenObservations(corpus) {
  const nobleProblems = (corpus.scenarios ?? []).filter(scenario => ['contract-violation',
    'unexpected-refusal', 'runtime-failure'].includes(scenario.comparison?.result_class));
  const policy = corpus.comparison?.relax?.temporal_admission ?? [];
  return {
    noble_regressions: nobleProblems.map(scenario => ({ case_id: scenario.case_id,
      result_class: scenario.comparison.result_class, reasons: scenario.comparison.reasons })),
    relax_observations: policy.filter(row => row.outcome !== 'pass').map(row => ({
      subject: `${row.scenario} under Relax admission ${row.admission}`,
      policy: row.policy,
      outcome: row.outcome,
      detail: row.detail,
      noble_defect: false,
      reasoning: 'the checked-before-execution policy is evaluated against the Relax admission setting; the Noble column of the same case keeps its own classification',
    })),
    performance: [],
    conclusion: nobleProblems.length === 0
      ? 'No actionable Noble regression or performance defect: every Noble cell keeps the class its export recorded, and the Relax timings have no baseline, so they are bounds and not a performance claim.'
      : `Noble cells need attention: ${nobleProblems.map(scenario => scenario.case_id).join(', ')}.`,
  };
}

/** The receipt fields that follow mechanically from the evidence directory and its corpus. */
export function derivedReceiptFields(corpus, inputs) {
  return {
    export: corpus.comparison?.export ?? null,
    relax: {
      compiler_lab: corpus.comparison?.relax?.compiler_lab ?? null,
      profile: corpus.comparison?.relax?.profile ?? null,
      host: corpus.comparison?.relax?.host ?? null,
      report: { path: 'relax/run/report.json', sha256: inputs.digests.report },
      replay: { path: 'relax/replay/report.json', sha256: inputs.digests.replay },
    },
    scenarios: comparisonSummary(corpus),
    result_class_counts: comparisonCounts(corpus),
    counts_basis: 'one Noble cell and one makepad-relax cell per case; relax_settings counts each Relax admission setting that ran',
    observations_for_issue_10: issueTenObservations(corpus),
    relax_timings: relaxTimings(inputs),
    non_claims: [...COMPARISON_NON_CLAIMS],
  };
}

export function bindReceipt(receipt) {
  const body = { ...receipt };
  delete body.binding_sha256;
  return { ...body, binding_sha256: digestValue(body) };
}

/** What the import timing is bound to: the export revision and the exact compiler-lab reports. */
export function timingBinding(inputs) {
  return {
    source_commit: inputs.exportCorpus.source?.commit ?? null,
    export_corpus_sha256: inputs.digests.corpus,
    relax_report_sha256: inputs.digests.report,
    relax_replay_sha256: inputs.digests.replay,
    what: 'this import; each Relax timing in relax_timings is bound to its own case, admission setting, Relax revision, profile, edits and admission outcome',
  };
}

/**
 * Write the derived corpus and receipt for an evidence directory whose retained files are in place.
 * The importer calls it once; the controls call it to reseal a mutated copy, so that only the
 * semantic checks are left to notice the mutation.
 * `receiptBase.timing` supplies the wall times; its binding is derived here.
 */
export function sealComparison({ directory, retained, generatedAt, receiptBase }) {
  const refreshed = retained.map(entry => {
    const bytes = fs.readFileSync(path.join(directory, entry.path));
    return { ...entry, sha256: sha256(bytes), bytes: bytes.length };
  });
  const inputs = loadInputs(directory, refreshed);
  const corpus = deriveComparison(inputs, generatedAt);
  fs.writeFileSync(path.join(directory, 'corpus.json'), `${JSON.stringify(corpus, null, 2)}\n`);
  const corpusDigest = sha256(fs.readFileSync(path.join(directory, 'corpus.json')));
  const receipt = bindReceipt({
    ...receiptBase,
    ...derivedReceiptFields(corpus, inputs),
    timing: { ...pick(receiptBase.timing, ['started_at', 'ended_at', 'duration_ms']),
      bound_to: timingBinding(inputs) },
    corpus: { evidence_path: 'corpus.json', sha256: corpusDigest },
  });
  fs.writeFileSync(path.join(directory, 'receipt.json'), `${JSON.stringify(receipt, null, 2)}\n`);
  return { corpus, receipt };
}

/** Structural validation of a comparison receipt. */
export function validateComparisonReceipt(receipt) {
  const failures = [];
  const fail = (where, reason) => failures.push(`${where}: ${reason}`);
  if (!isObject(receipt)) return ['receipt: not an object'];
  if (receipt.schema !== COMPARISON_RECEIPT_SCHEMA) fail('schema', `expected ${COMPARISON_RECEIPT_SCHEMA}`);
  if (typeof receipt.claim !== 'string' || receipt.claim.length === 0) fail('claim', 'missing');
  if (!/^[0-9a-f]{40}$/u.test(receipt.source?.commit ?? '') || receipt.source?.clean !== true)
    fail('source', 'commit/clean binding missing');
  if (!Array.isArray(receipt.pins) || receipt.pins.length === 0) fail('pins', 'missing');
  if (!/^[0-9a-f]{64}$/u.test(receipt.corpus?.sha256 ?? '')) fail('corpus', 'digest missing');
  if (!Array.isArray(receipt.scenarios) || receipt.scenarios.length < 3)
    fail('scenarios', 'fewer than three');
  for (const runtime of ['noble', 'relax', 'relax_settings'])
    for (const id of RESULT_CLASS_IDS)
      if (!Number.isInteger(receipt.result_class_counts?.[runtime]?.[id]))
        fail(`result_class_counts.${runtime}.${id}`, 'missing count');
  if (!Number.isFinite(Date.parse(receipt.timing?.started_at ?? ''))
    || !Number.isFinite(Date.parse(receipt.timing?.ended_at ?? ''))
    || !Number.isFinite(receipt.timing?.duration_ms))
    fail('timing', 'missing');
  if (!Array.isArray(receipt.non_claims) || receipt.non_claims.length === 0)
    fail('non_claims', 'empty');
  if (receipt.controls !== undefined && (!isObject(receipt.controls)
    || !Array.isArray(receipt.controls.controls) || receipt.controls.controls.length === 0))
    fail('controls', 'malformed control results');
  if (!/^[0-9a-f]{64}$/u.test(receipt.binding_sha256 ?? '')) fail('binding_sha256', 'missing');
  return failures;
}

const firstDifference = (left, right) => {
  if (!isObject(left) || !isObject(right)) return '';
  const keys = [...new Set([...Object.keys(left), ...Object.keys(right)])].sort();
  return keys.filter(key => stableJson(left[key]) !== stableJson(right[key])).join(', ');
};

/**
 * Verify a comparison directory: the export, every retained byte, the derivation, and each binding
 * of the Relax column. With a repository root, also the live profile and the source pins.
 * `receipt`/`corpus` may be supplied in memory by a producer that has not written them yet.
 */
export function verifyComparison({ directory, repositoryRoot = null, receipt = null,
  corpus = null }) {
  const failures = [];
  const fail = (code, target, detail) => failures.push({ code, target, detail });
  const exists = relative => fs.existsSync(path.join(directory, relative));
  for (const name of ['corpus.json', 'receipt.json'])
    if ((name === 'corpus.json' ? corpus : receipt) === null && !exists(name))
      return { failures: [{ code: 'missing-evidence', target: name, detail: 'not in the comparison' }] };
  receipt ??= readJson(path.join(directory, 'receipt.json'));
  corpus ??= readJson(path.join(directory, 'corpus.json'));

  // The Noble export, exactly as compiler-lab consumed it, verifies on its own.
  const exported = verifyEvidence({ directory: path.join(directory, 'export'), repositoryRoot });
  for (const row of exported.failures) fail(row.code, `export/${row.target}`, row.detail);

  if (exists('corpus.json') && sha256(fs.readFileSync(path.join(directory, 'corpus.json')))
    !== receipt.corpus?.sha256)
    fail('stale-evidence', 'corpus.json', 'corpus bytes differ from the receipt binding');
  for (const row of validateCorpus(corpus)) fail('schema-invalid', 'corpus.json', row);
  if (corpus.schema !== COMPARISON_SCHEMA)
    fail('schema-invalid', 'corpus.json', `a comparison corpus uses ${COMPARISON_SCHEMA}`);
  for (const row of validateComparisonReceipt(receipt)) fail('schema-invalid', 'receipt.json', row);

  // Every retained compiler-lab byte matches its digest, stays inside relax/, carries no local
  // path, and nothing unlisted sits beside it.
  const retained = Array.isArray(corpus.comparison?.retained) ? corpus.comparison.retained : [];
  const listed = new Set();
  for (const entry of retained) {
    const relative = entry?.path ?? '';
    if (!/^relax\/(run|replay)\//u.test(relative) || relative.split('/').includes('..')) {
      fail('schema-invalid', relative, 'a retained file escapes relax/');
      continue;
    }
    listed.add(relative);
    if (!exists(relative)) {
      fail('missing-evidence', relative, 'a retained compiler-lab file is missing');
      continue;
    }
    const bytes = fs.readFileSync(path.join(directory, relative));
    if (sha256(bytes) !== entry.sha256 || bytes.length !== entry.bytes)
      fail('stale-evidence', relative, 'retained bytes differ from the recorded digest or length');
    const leaks = privateMatches(bytes.toString('utf8'));
    if (leaks.length > 0)
      fail('private-path', relative, `retained bytes match ${leaks.join(', ')}`);
    if (quotesSource(relative, bytes.toString('utf8')))
      fail('private-source', relative, 'retained bytes quote a private fixture source line');
  }
  const walk = relative => fs.readdirSync(path.join(directory, relative), { withFileTypes: true })
    .flatMap(entry => entry.isDirectory() ? walk(`${relative}/${entry.name}`)
      : [`${relative}/${entry.name}`]);
  if (exists('relax'))
    for (const relative of walk('relax'))
      if (!listed.has(relative))
        fail('unlisted-evidence', relative, 'a file under relax/ is not in the retained list');

  let inputs;
  try {
    inputs = loadInputs(directory, retained);
  } catch (error) {
    fail('missing-evidence', 'relax', `the retained reports cannot be loaded: ${error.message}`);
    return { failures };
  }
  const { report, replay, request, exportCorpus, digests } = inputs;

  // The column follows from the retained material and nothing else.
  try {
    const derived = deriveComparison(inputs, corpus.generated_at);
    if (stableJson(derived) !== stableJson(corpus))
      fail('comparison-drift', 'corpus.json',
        `the retained compiler-lab material derives another corpus (differs in: ${firstDifference(derived, corpus)})`);
    for (const scenario of derived.scenarios) {
      const recorded = corpus.scenarios?.find(row => row.case_id === scenario.case_id);
      if (recorded && stableJson(recorded.other_runtime) !== stableJson(scenario.other_runtime))
        fail('comparison-drift', scenario.case_id,
          `the makepad-relax column differs in: ${firstDifference(scenario.other_runtime, recorded.other_runtime)}`);
    }
  } catch (error) {
    fail('comparison-drift', 'corpus.json', `derivation failed: ${error.message}`);
  }

  // The run, its records and its replay were computed against this export and these profiles.
  const exportCommit = exportCorpus.source?.commit;
  const corpusBindings = [['relax/run/report.json', report.corpus],
    ['relax/replay/report.json', replay.corpus],
    ...[...inputs.records.entries()].map(([relative, record]) => [relative, record.corpus])];
  for (const [target, binding] of corpusBindings) {
    if (binding?.sha256 !== digests.corpus || binding?.source_commit !== exportCommit
      || binding?.corpus_revision !== exportCorpus.corpus_revision)
      fail('corpus-binding-mismatch', target,
        'compiler-lab ran against another Noble export than export/corpus.json');
    if (binding?.noble_profile_sha256 !== digests.profile
      || binding?.noble_profile_id !== exportCorpus.profile?.id
      || binding?.noble_profile_path !== PROFILE_PATH)
      fail('profile-changed', target,
        'compiler-lab compared against another Noble profile than the export carries');
  }
  if (repositoryRoot !== null && fs.existsSync(path.join(repositoryRoot, PROFILE_PATH))
    && fileSha256(repositoryRoot, PROFILE_PATH) !== report.corpus?.noble_profile_sha256)
    fail('profile-changed', PROFILE_PATH,
      'the live Noble profile differs from the one this comparison was computed against; prior comparison evidence is invalid');
  const profiles = [['relax/run/report.json', report.profile],
    ['relax/replay/report.json', replay.profile],
    ...[...inputs.records.entries()].map(([relative, record]) => [relative, record.profile]),
    ...(report.noble_columns ?? []).map(column => [`noble_columns.${column.case_id}`, column.profile])];
  for (const [target, profile] of profiles)
    if (stableJson(pick(profile, ['id', 'path', 'sha256'])) !== stableJson(RELAX_PROFILE))
      fail('relax-profile-changed', target,
        'the Relax profile differs from the frozen profile this comparison accepts');
  if (stableJson(corpus.comparison?.relax?.compiler_lab) !== stableJson(COMPILER_LAB))
    fail('relax-profile-changed', 'comparison.relax.compiler_lab',
      'another compiler-lab revision than the one this comparison accepts');

  // The pinned Relax host ran every scenario and did not change during the run or the replay.
  for (const [target, document] of [['relax/run/report.json', report],
    ['relax/replay/report.json', replay]]) {
    const identity = document.identity;
    const observed = {
      source_repository: identity?.manifest?.source_repository,
      source_revision: identity?.manifest?.source_revision,
      executable_sha256: identity?.executable_sha256,
      manifest_sha256: identity?.manifest_sha256,
      reported_version: identity?.reported_version,
      std_tree_sha256: identity?.std_tree_sha256,
    };
    if (stableJson(observed) !== stableJson(RELAX_HOST) || document.identity_unchanged !== true)
      fail('relax-identity-changed', target, 'the Relax host is not the pinned host, or it changed');
  }
  for (const [relative, record] of inputs.records)
    if (record.host?.executable_sha256 !== RELAX_HOST.executable_sha256
      || record.host?.reported_version !== RELAX_HOST.reported_version
      || record.host?.std_tree_sha256 !== RELAX_HOST.std_tree_sha256)
      fail('relax-identity-changed', relative, 'a record names another Relax host');

  // compiler-lab's own evidence controls ran and passed, in the run and in the replay.
  for (const [target, document] of [['relax/run/report.json', report],
    ['relax/replay/report.json', replay]]) {
    const names = (document.controls ?? []).map(row => row.name);
    if (stableJson(names) !== stableJson(RELAX_CONTROLS)
      || !(document.controls ?? []).every(row => row.passed === true))
      fail('relax-controls-failed', target,
        'the compiler-lab controls did not all run and pass for their intended reasons');
  }
  if (stableJson(report.result_classes) !== stableJson(RESULT_CLASS_IDS))
    fail('schema-invalid', 'relax/run/report.json', 'compiler-lab names other result classes');
  if (stableJson(request.admission) !== stableJson(['default', 'check-first']))
    fail('schema-invalid', 'relax/run/request.json', 'the run did not cover both Relax admission settings');

  // Each measured column setting is the record it names, and that record is bound to its edits,
  // its Noble case and its timing.
  const exportScenarios = new Map((exportCorpus.scenarios ?? []).map(row => [row.case_id, row]));
  const columns = report.noble_columns ?? [];
  const recordsByPath = inputs.records;
  const columnRecords = new Set();
  for (const column of columns) {
    const exportScenario = exportScenarios.get(column.case_id);
    if (exportScenario === undefined) {
      fail('corpus-binding-mismatch', `noble_columns.${column.case_id}`, 'no such Noble case');
      continue;
    }
    if (MEASURED_CLASSES.includes(column.result_class)) {
      const declared = (exportScenario.other_runtime?.anticipated_differences ?? [])
        .map(row => pick(row, ['dimension', 'noble', 'relax', 'classification_rule']));
      const related = (column.anticipated_differences ?? [])
        .map(row => pick(row, ['dimension', 'noble', 'relax', 'classification_rule']));
      if (stableJson(declared) !== stableJson(related))
        fail('noble-anchor-mismatch', `noble_columns.${column.case_id}`,
          'the related dimensions are not the differences the Noble export anticipated');
    }
    for (const setting of column.admission_settings ?? []) {
      const retainedPath = `relax/run/${setting.record?.path}`;
      const target = `${column.case_id}/${setting.admission}`;
      columnRecords.add(retainedPath);
      const entry = retained.find(row => row.path === retainedPath);
      const record = recordsByPath.get(retainedPath);
      if (entry === undefined || record === undefined) {
        fail('record-binding-mismatch', target, 'the setting names a record that is not retained');
        continue;
      }
      if (entry.original_sha256 !== setting.record?.sha256
        || entry.original_path !== `run/${setting.record?.path}`)
        fail('record-binding-mismatch', target,
          'the column names other record bytes than the importer retained');
      if (record.case_id !== column.case_id || record.scenario !== column.scenario
        || record.admission !== setting.admission || record.result_class !== setting.result_class
        || record.relax_contract !== setting.relax_contract
        || stableJson(setting.temporal_admission ?? null) !== stableJson(
          record.temporal_admission === null || record.temporal_admission === undefined ? null
            : pick(record.temporal_admission, ['policy', 'outcome'])))
        fail('record-binding-mismatch', target, 'the column and its record disagree');
      if (eventBinding(record.edits) !== record.event_binding_sha256
        || record.event_binding_sha256 !== setting.event_binding_sha256
        || record.timing?.bound_to?.event_binding_sha256 !== setting.event_binding_sha256)
        fail('event-binding-mismatch', target,
          'the edit digests no longer give the bound event binding: the comparison evidence is invalid');
      const noble = record.noble ?? {};
      const exportExpectations = (exportScenario.expectations ?? [])
        .map(row => ({ expectation_id: row.expectation_id, status: row.status }));
      if (noble.case_id !== column.case_id || noble.result_class !== exportScenario.comparison?.result_class
        || stableJson(noble.expectations) !== stableJson(exportExpectations)
        || noble.sha256 !== sha256(stableJson(exportScenario)))
        fail('noble-anchor-mismatch', target,
          'the record anchors another Noble scenario, class or expectation status than the export');
      const bound = record.timing?.bound_to ?? {};
      const outcome = (record.run?.edits ?? []).map(edit => ({ edit: edit.index, rung: edit.rung,
        error_codes: edit.error_codes }));
      if (bound.case_id !== column.case_id || bound.scenario !== column.scenario
        || bound.admission !== setting.admission || bound.result_class !== setting.result_class
        || bound.relax_revision !== RELAX_HOST.source_revision
        || bound.host_sha256 !== RELAX_HOST.executable_sha256
        || bound.profile_sha256 !== RELAX_PROFILE.sha256
        || stableJson(bound.admission_outcome) !== stableJson(outcome)
        || !Number.isFinite(record.timing?.duration_ms))
        fail('timing-unbound', target,
          'the Relax timing is not bound to this case, revision, profile, edits and admission outcome');
      const replayed = (replay.replay ?? []).filter(row => row.scenario === column.scenario
        && row.admission === setting.admission);
      if (replayed.length !== 1 || replayed[0].relation !== 'reproduced'
        || replayed[0].retained_event_binding_sha256 !== setting.event_binding_sha256
        || replayed[0].event_binding_sha256 !== setting.event_binding_sha256)
        fail('replay-not-reproduced', target,
          'the replay did not reproduce this setting from its retained edits');
    }
  }
  for (const relative of recordsByPath.keys())
    if (!columnRecords.has(relative))
      fail('record-binding-mismatch', relative, 'a retained record is named by no column');
  if ((report.scenarios ?? []).length !== recordsByPath.size
    || (replay.replay ?? []).length !== recordsByPath.size)
    fail('replay-not-reproduced', 'relax/replay/report.json',
      'the run, its retained records and the replay relations do not cover the same settings');
  if (replay.replay_of === null || replay.replay_of === undefined)
    fail('replay-not-reproduced', 'relax/replay/report.json', 'the replay names no retained run');

  // The receipt binds this comparison: the export, the derived summaries and its own body.
  const expectedFields = derivedReceiptFields(corpus, inputs);
  for (const [key, value] of Object.entries(expectedFields))
    if (stableJson(receipt[key]) !== stableJson(value))
      fail('comparison-drift', `receipt.${key}`, 'the receipt differs from the retained evidence');
  const exportReceipt = inputs.exportReceipt;
  if (receipt.source?.commit !== exportCommit || exportReceipt.source?.commit !== exportCommit)
    fail('source-pin-mismatch', 'receipt.source',
      'the comparison and its export come from different source revisions');
  if (stableJson(receipt.timing?.bound_to) !== stableJson(timingBinding(inputs)))
    fail('timing-unbound', 'receipt.timing',
      'the import timing is not bound to this export and these compiler-lab reports');
  if (stableJson(receipt.pins) !== stableJson(exportCorpus.pins))
    fail('source-pin-mismatch', 'receipt.pins', 'the comparison binds other sources than its export');
  if (receipt.binding_sha256 !== undefined) {
    const copy = { ...receipt };
    delete copy.binding_sha256;
    if (digestValue(copy) !== receipt.binding_sha256)
      fail('receipt-binding-mismatch', 'receipt.json',
        'the receipt body does not match its own binding digest');
  }
  return { failures };
}

/**
 * Copy an evidence directory into a private temporary directory for one control.
 * `os.tmpdir()` follows the caller's TMPDIR.
 */
export function copyComparison(directory) {
  const target = fs.mkdtempSync(path.join(os.tmpdir(), 'noble-live-update-comparison-control-'));
  fs.cpSync(directory, target, { recursive: true, dereference: false });
  return target;
}
