#!/usr/bin/env node
// Issue #62 refusal-class controls for the independent-kernel pilot.
//
// Runs the pinned lean4export exporter and the pinned nanoda checker through one replay pipeline and
// classifies every run into a named class: accepted, export-failure, unsupported-format,
// unsupported-extension, forbidden-axiom, parse-refused, rejected, empty-check, timeout,
// resource-exhausted or refused-other. Only `accepted` is success, and acceptance is strict:
// - the exporter exited zero without a panic;
// - every requested root is declared in the export;
// - the checker exited zero and reported a positive number of checked declarations;
// - every axiom the export declares is one of Noble's three standard axioms, whatever the checker
//   was told to permit.
//
// Each control derives one input or configuration change from pinned inputs and states the class it
// must land in. The receipt holds only when every control lands in its expected class and no
// non-baseline control is accepted. This is pilot evidence, not a gate.
import { spawnSync } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const argv = process.argv.slice(2);
const value = (name) => {
  const index = argv.indexOf(name);
  if (index < 0 || index + 1 >= argv.length) throw new Error(`missing ${name}`);
  return argv[index + 1];
};
const nanoda = path.resolve(value('--nanoda'));
const exporter = path.resolve(value('--exporter'));
const leanBin = path.resolve(value('--lean-bin'));
const large = path.resolve(value('--large-export'));
const small = path.resolve(value('--small-export'));
const work = path.resolve(value('--work'));
const output = path.resolve(value('--output'));
const pins = {
  nanoda: value('--nanoda-sha256'),
  exporter: value('--exporter-sha256'),
  lean: value('--lean-sha256'),
  large: value('--large-sha256'),
  small: value('--small-sha256'),
};

const sha256 = (file) => crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
// Refuse before any process runs when a pinned input changed.
for (const [name, file] of [['nanoda', nanoda], ['exporter', exporter], ['lean', path.join(leanBin, 'lean')],
  ['large', large], ['small', small]]) {
  const actual = sha256(file);
  if (actual !== pins[name]) throw new Error(`${name} digest ${actual} differs from the pin ${pins[name]}`);
}
if (fs.existsSync(work)) throw new Error(`work directory exists: ${work}`);
fs.mkdirSync(work, { recursive: true });

const STANDARD_AXIOMS = ['propext', 'Classical.choice', 'Quot.sound'];
const SMALL_ROOT = 'NobleFoundation.LinkBoundary.vertical_simulation_compose';
const LARGE_ROOT = 'NobleFoundation.toInt_add';
const CUSTOM_AXIOM = 'NobleReplayControl.customAxiom';
const childEnv = {
  HOME: process.env.HOME ?? '/', LANG: 'C.UTF-8',
  PATH: [leanBin, '/run/current-system/sw/bin', '/usr/bin', '/bin'].join(':'),
};

/** Declared names, declared axioms and format version of an NDJSON export, from its own records. */
function inspectExport(file) {
  const names = new Map([[0, '']]);
  const declared = new Set();
  const axioms = [];
  let format = null;
  let records = 0;
  const nameOf = (index) => {
    if (!names.has(index)) throw new Error(`name index ${index} used before it is defined`);
    return names.get(index);
  };
  for (const line of fs.readFileSync(file, 'utf8').split('\n')) {
    if (line === '') continue;
    records++;
    const record = JSON.parse(line);
    if (record.meta) format = record.meta.format?.version ?? null;
    if (record.str && 'in' in record) {
      const prefix = nameOf(record.str.pre);
      names.set(record.in, prefix ? `${prefix}.${record.str.str}` : record.str.str);
    }
    if (record.num && 'in' in record) {
      const prefix = nameOf(record.num.pre);
      names.set(record.in, prefix ? `${prefix}.${record.num.i}` : String(record.num.i));
    }
    for (const kind of ['axiom', 'def', 'thm', 'opaque', 'quot']) {
      if (record[kind]) declared.add(nameOf(record[kind].name));
    }
    if (record.axiom) axioms.push(nameOf(record.axiom.name));
    if (record.inductive) {
      for (const group of ['types', 'ctors', 'recs']) {
        for (const entry of record.inductive[group] ?? []) declared.add(nameOf(entry.name));
      }
    }
  }
  return { records, format, declared, axioms };
}

function classifyChecker(result) {
  if (result.error?.code === 'ETIMEDOUT') return 'timeout';
  const stdout = result.stdout ?? '';
  const stderr = result.stderr ?? '';
  const checked = stdout.match(/Checked (\d+) declarations with no errors/u);
  if (result.status === 0 && checked) return Number(checked[1]) > 0 ? 'accepted' : 'empty-check';
  if (/export format version is (?:greater|less) than the (?:maximum|minimum) supported version/u.test(stderr)) {
    return 'unsupported-format';
  }
  if (/(?:Nat|String) lit extension disallowed by checker execution config/u.test(stderr)) return 'unsupported-extension';
  if (/declares unpermitted axiom/u.test(stderr)) return 'forbidden-axiom';
  if (result.signal === 'SIGABRT' || result.status === 134
      || /memory allocation of \d+ bytes failed|Cannot allocate memory|out of memory/iu.test(stderr)) {
    return 'resource-exhausted';
  }
  if (/EOF while parsing|expected value|missing field|unknown variant|invalid type/u.test(stderr)) return 'parse-refused';
  if (result.status === 101) return 'rejected';
  return 'refused-other';
}

function nanodaConfig(id, exportPath, overrides) {
  const file = path.join(work, `${id}.nanoda.json`);
  fs.writeFileSync(file, `${JSON.stringify({
    use_stdin: false,
    export_file_path: exportPath,
    permitted_axioms: STANDARD_AXIOMS,
    unpermitted_axiom_hard_error: true,
    num_threads: 1,
    nat_extension: true,
    string_extension: true,
    print_axioms: false,
    print_success_message: true,
    ...overrides,
  }, null, 2)}\n`);
  return file;
}

function runChecker(id, exportPath, overrides) {
  const config = nanodaConfig(id, exportPath, overrides);
  const started = process.hrtime.bigint();
  const result = spawnSync(nanoda, [config], {
    env: childEnv, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024, timeout: 1_800_000, killSignal: 'SIGKILL',
  });
  return {
    config_overrides: overrides,
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
      argv: args,
      exit_code: result.status,
      signal: result.signal,
      panic: /PANIC/u.test(result.stderr ?? ''),
      // A panic backtrace carries per-run load addresses; the panic message precedes it.
      stderr_excerpt: (result.stderr ?? '').split('\nbacktrace:')[0].trim().slice(0, 300),
    },
  };
}

/** Rewrites only the meta record's format version. */
function deriveFormat(id, source, version) {
  const text = fs.readFileSync(source, 'utf8');
  const newline = text.indexOf('\n');
  const meta = JSON.parse(text.slice(0, newline));
  if (!meta.meta?.format?.version) throw new Error(`${source}: first record is not the meta record`);
  meta.meta.format.version = version;
  const file = path.join(work, `${id}.ndjson`);
  fs.writeFileSync(file, JSON.stringify(meta) + text.slice(newline));
  return file;
}

/** Appends `axiom NobleReplayControl.customAxiom.{u} : Sort u`, built from the export's own `Sort u`
 *  expression, under fresh name indices. */
function deriveCustomAxiom(id, source) {
  const text = fs.readFileSync(source, 'utf8');
  const lines = text.split('\n').filter((line) => line !== '');
  const records = lines.map((line) => JSON.parse(line));
  const params = new Map(records.filter((record) => 'il' in record && 'param' in record)
    .map((record) => [record.il, record.param]));
  const sort = records.find((record) => 'ie' in record && 'sort' in record && params.has(record.sort));
  if (!sort) throw new Error(`${source}: no Sort u expression to type the custom axiom`);
  const fresh = Math.max(...records.filter((record) => 'in' in record).map((record) => record.in)) + 1;
  const [space, leaf] = CUSTOM_AXIOM.split('.');
  const appended = [
    { in: fresh, str: { pre: 0, str: space } },
    { in: fresh + 1, str: { pre: fresh, str: leaf } },
    { axiom: { name: fresh + 1, levelParams: [params.get(sort.sort)], type: sort.ie, isUnsafe: false } },
  ];
  const file = path.join(work, `${id}.ndjson`);
  fs.writeFileSync(file, `${[...lines, ...appended.map((record) => JSON.stringify(record))].join('\n')}\n`);
  return file;
}

function describe(file) {
  const inspection = inspectExport(file);
  return {
    inspection,
    record: {
      sha256: sha256(file), size_bytes: fs.statSync(file).size, records: inspection.records,
      format_version: inspection.format, declared: inspection.declared.size, axioms: inspection.axioms,
    },
  };
}

/** One replay through the pipeline: optional export, export checks, checker run, lane decision. */
function replay(control) {
  const entry = { id: control.id, changes: control.changes, expected: control.expected, roots: control.roots };
  let exportPath = control.file;
  let exportFailed = [];
  if (control.export) {
    const exported = runExporter(control.id, control.export);
    entry.export_stage = exported.record;
    exportPath = exported.exportPath;
    if (exported.record.exit_code !== 0 || exported.record.signal) exportFailed.push(`exporter exited ${exported.record.exit_code ?? exported.record.signal}`);
    if (exported.record.panic) exportFailed.push('exporter panicked');
  }
  const { inspection, record } = describe(exportPath);
  entry.export = record;
  const missing = control.roots.filter((root) => !inspection.declared.has(root));
  if (missing.length > 0) exportFailed.push(`roots absent from the export: ${missing.join(', ')}`);
  if (exportFailed.length > 0) {
    entry.verdict = 'export-failure';
    entry.reasons = exportFailed;
    // What a pipeline reading exit statuses alone would have seen: the checker on the same export.
    if (control.counterfactual) {
      const checker = runChecker(`${control.id}-status-only`, exportPath, {});
      entry.status_only_view = {
        exporter_exit: entry.export_stage?.exit_code ?? null, checker_exit: checker.exit_code,
        checker_stdout: checker.stdout, strict_classification: checker.classification,
      };
    }
    return entry;
  }
  entry.checker = runChecker(control.id, exportPath, control.config ?? {});
  const foreign = inspection.axioms.filter((axiom) => !STANDARD_AXIOMS.includes(axiom));
  if (entry.checker.classification === 'accepted' && foreign.length > 0) {
    entry.verdict = 'forbidden-axiom';
    entry.reasons = [`checker accepted, but the export declares non-standard axioms: ${foreign.join(', ')}`];
  } else {
    entry.verdict = entry.checker.classification;
  }
  return entry;
}

const smallAbove = deriveFormat('unsupported-format-above', small, '3.2.0');
const smallBelow = deriveFormat('unsupported-format-below', small, '3.0.0');
const smallCustom = deriveCustomAxiom('custom-axiom', small);
const controls = [
  { id: 'baseline-exported', changes: 'none: Init export of Nat.add_comm', baseline: true,
    export: ['Init', '--', 'Nat.add_comm'], roots: ['Nat.add_comm'], expected: 'accepted' },
  { id: 'baseline-retained', changes: 'none: retained export', baseline: true,
    file: small, roots: [SMALL_ROOT], expected: 'accepted' },
  { id: 'baseline-large', changes: 'none: pinned large export', baseline: true,
    file: large, roots: [LARGE_ROOT], expected: 'accepted' },
  { id: 'export-missing-module', changes: 'the exported module does not exist',
    export: ['NoSuchModule', '--', 'Nat.add_comm'], roots: ['Nat.add_comm'], expected: 'export-failure' },
  { id: 'export-missing-root', changes: 'the requested root does not exist', counterfactual: true,
    export: ['Init', '--', 'Nat.no_such_declaration'], roots: ['Nat.no_such_declaration'], expected: 'export-failure' },
  { id: 'export-partial-roots', changes: 'one of two requested roots does not exist', counterfactual: true,
    export: ['Init', '--', 'Nat.add_comm', 'Nat.no_such_declaration'],
    roots: ['Nat.add_comm', 'Nat.no_such_declaration'], expected: 'export-failure' },
  { id: 'unsupported-format-above', changes: 'meta format version 3.2.0, above the supported range',
    file: smallAbove, roots: [SMALL_ROOT], expected: 'unsupported-format' },
  { id: 'unsupported-format-below', changes: 'meta format version 3.0.0, below the supported range',
    file: smallBelow, roots: [SMALL_ROOT], expected: 'unsupported-format' },
  { id: 'unsupported-nat-literal', changes: 'checker run with the nat literal extension off',
    file: large, roots: [LARGE_ROOT], config: { nat_extension: false }, expected: 'unsupported-extension' },
  { id: 'unsupported-string-literal', changes: 'checker run with the string literal extension off',
    file: large, roots: [LARGE_ROOT], config: { string_extension: false }, expected: 'unsupported-extension' },
  { id: 'checker-forbidden-axiom', changes: 'checker permits only Classical.choice and Quot.sound',
    file: large, roots: [LARGE_ROOT], config: { permitted_axioms: ['Classical.choice', 'Quot.sound'] },
    expected: 'forbidden-axiom' },
  { id: 'custom-axiom-checker-refuses', changes: `export declares ${CUSTOM_AXIOM}; checker permits the standard axioms`,
    file: smallCustom, roots: [SMALL_ROOT], expected: 'forbidden-axiom' },
  { id: 'custom-axiom-checker-permits', changes: `export declares ${CUSTOM_AXIOM}; checker also permits it`,
    file: smallCustom, roots: [SMALL_ROOT], config: { permitted_axioms: [...STANDARD_AXIOMS, CUSTOM_AXIOM] },
    expected: 'forbidden-axiom' },
];

const entries = controls.map((control) => ({ ...replay(control), baseline: Boolean(control.baseline) }));
const byId = (id) => entries.find((entry) => entry.id === id);
const checks = {
  every_control_in_its_expected_class: entries.every((entry) => entry.verdict === entry.expected),
  baselines_accepted: entries.filter((entry) => entry.baseline).every((entry) => entry.verdict === 'accepted'),
  no_other_control_accepted: entries.filter((entry) => !entry.baseline).every((entry) => entry.verdict !== 'accepted'),
  checker_accepts_permitted_custom_axiom: byId('custom-axiom-checker-permits')?.checker?.classification === 'accepted',
};
const holds = Object.values(checks).every(Boolean);
const observations = {
  missing_root_exporter_exit: byId('export-missing-root')?.export_stage?.exit_code ?? null,
  missing_root_status_only_view: byId('export-missing-root')?.status_only_view ?? null,
  partial_roots_status_only_view: byId('export-partial-roots')?.status_only_view ?? null,
};

const host = spawnSync('uname', ['-srm'], { encoding: 'utf8' }).stdout.trim();
const receipt = {
  schema: 'noble-independent-replay-refusals/v1',
  measured_on: new Date().toISOString().slice(0, 10),
  host,
  assertion: 'every control lands in its stated refusal class and no non-baseline control is accepted: an unsupported export format, a disabled literal extension, an exporter failure (including a missing root the exporter reports only by a panic), a checker-refused axiom and a custom axiom the checker was told to permit',
  holds,
  checks,
  harness: {
    path: 'verification/independent-replay/refusals.mjs',
    sha256: sha256(fileURLToPath(import.meta.url)),
  },
  acceptance_rule: [
    'exporter exit status zero and no exporter panic',
    'every requested root declared in the export',
    'checker exit status zero with a positive number of checked declarations',
    `every declared axiom in ${STANDARD_AXIOMS.join(', ')}, whatever the checker permits`,
  ],
  tools: {
    nanoda: { binary_sha256: pins.nanoda, num_threads: 1 },
    lean4export: { binary_sha256: pins.exporter },
    lean: { binary_sha256: pins.lean },
  },
  inputs: {
    large: { root: LARGE_ROOT, sha256: pins.large, retained: false },
    small: {
      root: SMALL_ROOT, sha256: pins.small, retained: true,
      path: 'verification/independent-replay/receipts/2026-10-29/vertical_simulation_compose.ndjson',
    },
  },
  controls: entries,
  observations,
  non_claims: [
    'Pilot evidence on one host with locally built pinned tools; no CI, flake, policy or gate change.',
    'A refusal class names why a run did not succeed; it is never a mathematical refutation of the subject.',
    'The custom-axiom export is a constructed control, not a Noble declaration.',
    'No claim about nanoda or lean4export correctness, completeness or equivalence to the Lean kernel.',
    'Root presence binds names, not statements: exact claim binding beyond declared names stays open.',
  ],
};
fs.mkdirSync(path.dirname(output), { recursive: true });
fs.writeFileSync(output, `${JSON.stringify(receipt, null, 2)}\n`);
console.log(JSON.stringify({ holds, checks, verdicts: Object.fromEntries(entries.map((entry) => [entry.id, entry.verdict])) }));
process.exitCode = holds ? 0 : 1;
