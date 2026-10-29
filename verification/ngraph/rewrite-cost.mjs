// Issue #64 cost-lane receipt runner.  This runner deliberately stays outside the Rust
// measurement path: it binds the selected offline toolchain and source snapshot, executes the
// real rewrite_cost integration test, and retains the unmodified process streams beside the v2
// receipt.  A failed process is evidence, not a reason to retry with a different environment.
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { reviewedVendorRecipe, selectedVendor } from '../selected-vendor.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const scriptRelative = 'verification/ngraph/rewrite-cost.mjs';
const manifestRelative = 'verification/ngraph/rewrite-cost-v2.txt';
const corpusRelative = 'verification/ngraph/rewrite-quality-v1.txt';
const policyRelative = 'policy/tool-selection.json';
const commandArgs = Object.freeze([
  'test', '-p', 'noble-ngraph', '--test', 'rewrite_cost', '--offline', '--locked', '--', '--nocapture',
]);
const proposers = Object.freeze(['identity', 'first-eligible', 'single-pass', 'fixed-point']);
const dispositions = Object.freeze([
  'input-refused', 'certificate-refused', 'candidate-refused', 'layout-refused',
  'execution-refused', 'executed',
]);
const noCandidateExecution = new Set([
  'input-refused', 'certificate-refused', 'candidate-refused', 'layout-refused',
]);
const rawRowFields = Object.freeze([
  'fixture', 'proposer', 'steps', 'refused', 'residual', 'propose_us', 'propose_iqr_us',
  'check_us', 'check_iqr_us', 'check_per_step_us', 'in_memory_footprint_bytes',
  'check_threshold_met',
]);
const pipelineStages = Object.freeze([
  'input_admission', 'proposal_search', 'certificate_construction', 'certificate_checking',
  'candidate_admission', 'backend_preparation', 'schedule_validation', 'first_execution',
  'full_fresh_attempt', 'warm_execution',
]);
const unmeasuredPipelineStages = Object.freeze([
  'certificate_wire_bytes', 'decoding_replay', 'retained_memory',
]);

function requireThat(condition, message) {
  if (!condition) throw new Error(message);
}

function sha256(bytes) {
  return createHash('sha256').update(bytes).digest('hex');
}

function fileHash(file) {
  return sha256(fs.readFileSync(file));
}

function json(value) {
  return `${JSON.stringify(value, null, 2)}\n`;
}

function writeExclusive(file, bytes) {
  fs.writeFileSync(file, bytes, { flag: 'wx' });
}

function writeJsonExclusive(file, value) {
  writeExclusive(file, json(value));
}

function plainObject(value) {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

function parseArgs(argv) {
  requireThat(argv.length === 2 && argv[0] === '--output' && argv[1].length > 0,
    'usage: node verification/ngraph/rewrite-cost.mjs --output <receipt-file> (evidence: <receipt-file>.evidence)');
  const output = path.resolve(argv[1]);
  const evidence = `${output}.evidence`;
  const outside = name => name !== root && !name.startsWith(`${root}${path.sep}`);
  requireThat(outside(output) && outside(evidence), 'output and evidence must be outside the source checkout');
  requireThat(!fs.existsSync(output), `refusing to overwrite output: ${output}`);
  requireThat(!fs.existsSync(evidence), `refusing to overwrite evidence directory: ${evidence}`);
  requireThat(fs.existsSync(path.dirname(output)), `output parent does not exist: ${path.dirname(output)}`);
  return { output, evidence };
}

function gitExecutable() {
  return fs.realpathSync('/run/current-system/sw/bin/git');
}

const git = gitExecutable();
const gitEnvironment = {
  PATH: '/run/current-system/sw/bin:/usr/bin:/bin',
  GIT_CONFIG_NOSYSTEM: '1',
  GIT_CONFIG_GLOBAL: '/dev/null',
  GIT_TERMINAL_PROMPT: '0',
};

function gitRun(...argv) {
  const result = spawnSync(git, argv, { cwd: root, env: gitEnvironment, encoding: 'utf8' });
  requireThat(!result.error && result.signal === null && result.status === 0,
    `git ${argv.join(' ')} failed: ${result.error?.message ?? result.status}\n${result.stderr ?? ''}`);
  return result.stdout.trim();
}

function parseManifest(text) {
  const lines = text.split('\n').map(line => line.trim()).filter(Boolean);
  const schema = lines.find(line => line.startsWith('schema '));
  requireThat(schema === 'schema noble-rewrite-cost/v2', 'rewrite cost manifest is not v2');
  const corpus = lines.find(line => line.startsWith('corpus '));
  requireThat(corpus === `corpus ${path.basename(corpusRelative)}`, 'rewrite cost corpus binding changed');
  const lens = [];
  const stages = [];
  let threshold = null;
  for (const line of lines) {
    if (line.startsWith('lens ')) {
      const lensPath = line.slice('lens '.length).trim();
      const match = lensPath.match(/^crates\/([A-Za-z0-9_-]+)\/src\/(.+)$/u);
      requireThat(match !== null, `malformed lens declaration: ${line}`);
      lens.push({ crate: match[1], module: match[2], lens: lensPath });
      continue;
    }
    const stage = line.match(/^stage ([^|]+) \| status (measured|unmeasured) \| reason (.+)$/u);
    if (stage) {
      stages.push({ name: stage[1].trim(), status: stage[2], reason: stage[3].trim() });
      continue;
    }
    const thresholdLine = line.match(
      /^threshold retention \| residual ([^|]+) \| check-per-step-us ([0-9]+)$/u,
    );
    if (thresholdLine) {
      threshold = { residual: thresholdLine[1], check_per_step_us: Number(thresholdLine[2]) };
    }
  }
  requireThat(lens.length > 0, 'rewrite cost manifest has no source lens');
  requireThat(stages.length > 0, 'rewrite cost manifest has no stage declarations');
  requireThat(new Set(stages.map(stage => stage.name)).size === stages.length,
    'rewrite cost manifest repeats a stage');
  requireThat(threshold !== null, 'rewrite cost manifest has no retention threshold');
  return { schema: 'noble-rewrite-cost/v2', corpus: path.basename(corpusRelative), lens, stages, threshold };
}

function parseCorpus(text) {
  const lines = text.split('\n').map(line => line.trim()).filter(Boolean);
  requireThat(lines.includes('schema noble-rewrite-quality/v1'), 'rewrite quality corpus schema changed');
  const fixtures = [];
  for (const line of lines) {
    const match = line.match(/^fixture ([^|]+) \| target ([^|]+) \| nodes (.+)$/u);
    if (!match) continue;
    fixtures.push({ name: match[1].trim(), target: match[2].trim() });
  }
  requireThat(fixtures.length === 7, `expected seven corpus fixtures, found ${fixtures.length}`);
  requireThat(new Set(fixtures.map(fixture => fixture.name)).size === fixtures.length,
    'rewrite quality corpus repeats a fixture');
  return fixtures;
}

function lensDigests(manifest) {
  const digests = [];
  let fileCount = 0;
  for (const declaration of manifest.lens) {
    requireThat(/^[A-Za-z0-9_-]+$/u.test(declaration.crate),
      `unsafe lens crate: ${declaration.crate}`);
    const sourceRoot = path.join(root, 'crates', declaration.crate, 'src');
    requireThat(fs.existsSync(sourceRoot) && fs.statSync(sourceRoot).isDirectory(),
      `lens crate has no source tree: ${declaration.crate}`);
    const files = [];
    const walk = (directory, depth) => {
      requireThat(depth <= 64, 'source lens directory depth exceeded');
      const entries = fs.readdirSync(directory, { withFileTypes: true })
        .sort((left, right) => left.name.localeCompare(right.name));
      for (const entry of entries) {
        const full = path.join(directory, entry.name);
        if (entry.isDirectory()) {
          walk(full, depth + 1);
        } else if (entry.isFile() && entry.name.endsWith('.rs')) {
          files.push(full);
          fileCount++;
          requireThat(fileCount <= 10_000, 'source lens file limit exceeded');
        }
      }
    };
    walk(sourceRoot, 0);
    files.sort();
    digests.push({
      lens: declaration.lens,
      crate: declaration.crate,
      files: files.map(file => ({
        path: path.relative(root, file),
        sha256: fileHash(file),
      })),
    });
  }
  return digests;
}

function readSourceSnapshot() {
  const manifestFile = path.join(root, manifestRelative);
  const corpusFile = path.join(root, corpusRelative);
  const policyFile = path.join(root, policyRelative);
  const scriptFile = path.join(root, scriptRelative);
  const manifestBytes = fs.readFileSync(manifestFile);
  const corpusBytes = fs.readFileSync(corpusFile);
  const manifest = parseManifest(manifestBytes.toString('utf8'));
  const corpus = parseCorpus(corpusBytes.toString('utf8'));
  const status = gitRun('status', '--porcelain=v1', '--untracked-files=all');
  const snapshot = {
    source_commit: gitRun('rev-parse', 'HEAD'),
    source_tree: gitRun('rev-parse', 'HEAD^{tree}'),
    status,
    clean_source: status === '',
    plan_commit: gitRun('log', '-1', '--format=%H', '--', manifestRelative),
    plan_sha256: sha256(manifestBytes),
    files: {
      [manifestRelative]: { sha256: sha256(manifestBytes) },
      [corpusRelative]: { sha256: sha256(corpusBytes) },
      [policyRelative]: { sha256: fileHash(policyFile) },
      [scriptRelative]: { sha256: fileHash(scriptFile) },
    },
    lens: lensDigests(manifest),
  };
  return { snapshot, manifest, corpus };
}

function executableIdentity(file) {
  const resolved = fs.realpathSync(file);
  const stat = fs.statSync(resolved);
  requireThat(stat.isFile(), `selected executable is not a regular file: ${file}`);
  return { path: resolved, sha256: fileHash(resolved), bytes: stat.size, mode: stat.mode & 0o777 };
}

function selectToolchain(selection, evidence) {
  requireThat(plainObject(selection.tool_paths?.quality_rust), 'quality Rust selection is missing');
  requireThat(typeof selection.tool_paths.quality_rust.output === 'string',
    'quality Rust output path is missing');
  requireThat(plainObject(selection.component_sync), 'component-sync policy is missing');
  const rustBin = path.join(selection.tool_paths.quality_rust.output, 'bin');
  const linkerBin = selection.component_sync.linker_bin;
  requireThat(typeof linkerBin === 'string', 'selected linker directory is missing');
  const cargo = path.join(rustBin, 'cargo');
  const rustc = path.join(rustBin, 'rustc');
  const rustdoc = path.join(rustBin, 'rustdoc');
  const linker = path.join(linkerBin, 'cc');
  const vendor = selectedVendor(selection.component_sync);
  const recipe = reviewedVendorRecipe(root, vendor);
  const policyFile = path.join(root, policyRelative);
  const cargoHome = path.join(evidence, 'cargo-home');
  const target = path.join(evidence, 'target');
  const temporary = path.join(evidence, 'tmp');
  const home = path.join(evidence, 'home');
  for (const directory of [cargoHome, target, temporary, home]) fs.mkdirSync(directory);
  const config = path.join(cargoHome, 'config.toml');
  writeExclusive(config,
    `[source.crates-io]\nreplace-with = "reviewed-vendor"\n` +
    `[source.reviewed-vendor]\ndirectory = ${JSON.stringify(vendor.directory)}\n` +
    '[net]\noffline = true\n');
  const environment = {
    HOME: home,
    USER: 'noble-rewrite-cost',
    LOGNAME: 'noble-rewrite-cost',
    LANG: 'C.UTF-8',
    LC_ALL: 'C.UTF-8',
    CI: '1',
    GIT_TERMINAL_PROMPT: '0',
    GIT_CONFIG_NOSYSTEM: '1',
    GIT_CONFIG_GLOBAL: '/dev/null',
    PATH: [rustBin, linkerBin, '/run/current-system/sw/bin', '/usr/bin', '/bin'].join(':'),
    TMPDIR: temporary,
    CARGO_HOME: cargoHome,
    CARGO_TARGET_DIR: target,
    CARGO_NET_OFFLINE: 'true',
    CARGO_BUILD_JOBS: '4',
    CARGO_INCREMENTAL: '0',
    RUSTC: rustc,
    RUSTDOC: rustdoc,
    RUSTC_WRAPPER: '',
    RUSTC_WORKSPACE_WRAPPER: '',
    CARGO_BUILD_RUSTC_WRAPPER: '',
    RUSTFLAGS: '',
    CARGO_ENCODED_RUSTFLAGS: '',
    CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS: '',
    CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER: linker,
    CARGO_TERM_COLOR: 'never',
  };
  return {
    rustBin,
    linkerBin,
    cargo,
    rustc,
    rustdoc,
    linker,
    vendor,
    recipe,
    config,
    environment,
    policy: {
      path: path.join(root, policyRelative),
      sha256: fileHash(policyFile),
      quality_rust: selection.tool_paths.quality_rust,
      linker_bin: linkerBin,
    },
  };
}

function processError(error) {
  return error ? { message: error.message, code: error.code ?? null } : null;
}

function runTest(toolchain, evidence, cargoIdentity) {
  const raw = path.join(evidence, 'raw');
  fs.mkdirSync(raw);
  let observed;
  try {
    observed = spawnSync(toolchain.cargo, commandArgs, {
      cwd: root,
      env: toolchain.environment,
      encoding: null,
      timeout: 1_800_000,
      maxBuffer: 64 * 1024 * 1024,
    });
  } catch (error) {
    observed = { error, signal: null, status: null, stdout: null, stderr: null };
  }
  const stdout = observed.stdout ?? Buffer.alloc(0);
  const stderr = observed.stderr ?? Buffer.alloc(0);
  writeExclusive(path.join(raw, 'stdout'), stdout);
  writeExclusive(path.join(raw, 'stderr'), stderr);
  const record = {
    executable: cargoIdentity,
    argv: [...commandArgs],
    full_argv: [cargoIdentity.path, ...commandArgs],
    cwd: root,
    environment: toolchain.environment,
    exit: observed.status ?? null,
    status: observed.status ?? null,
    signal: observed.signal ?? null,
    error: processError(observed.error),
    ok: !observed.error && observed.signal === null && observed.status === 0,
    stdout: { path: 'raw/stdout', bytes: stdout.length, sha256: sha256(stdout) },
    stderr: { path: 'raw/stderr', bytes: stderr.length, sha256: sha256(stderr) },
  };
  writeJsonExclusive(path.join(evidence, 'process.json'), record);
  return { record, stdout: stdout.toString('utf8'), stderr: stderr.toString('utf8') };
}

function extractReport(text) {
  let start = -1;
  let depth = 0;
  let quoted = false;
  let escaped = false;
  for (let index = 0; index < text.length; index++) {
    const character = text[index];
    if (start < 0) {
      if (character === '{') {
        start = index;
        depth = 1;
      }
      continue;
    }
    if (quoted) {
      if (escaped) escaped = false;
      else if (character === '\\') escaped = true;
      else if (character === '"') quoted = false;
      continue;
    }
    if (character === '"') quoted = true;
    else if (character === '{') depth++;
    else if (character === '}') {
      depth--;
      if (depth === 0) {
        const candidate = text.slice(start, index + 1);
        try {
          const parsed = JSON.parse(candidate);
          if (plainObject(parsed) && parsed.schema === 'noble-rewrite-cost-report/v2') return parsed;
        } catch {
          // Cargo diagnostics may contain balanced braces that are not JSON; continue scanning.
        }
        start = -1;
      }
    }
  }
  throw new Error('v2 report object is absent from both process streams');
}

function reportFromStreams(stdout, stderr) {
  for (const text of [stdout, stderr]) {
    try {
      return extractReport(text);
    } catch {
      // The test harness may place the report on either stream; retain both streams unchanged.
    }
  }
  throw new Error('v2 report object is absent from both process streams');
}

function finiteNumber(value, label) {
  requireThat(typeof value === 'number' && Number.isFinite(value), `${label} is not finite`);
}

function validateDuration(duration, label) {
  if (duration === null) return;
  requireThat(plainObject(duration), `${label} duration is not an object`);
  finiteNumber(duration.median_us, `${label}.median_us`);
  finiteNumber(duration.iqr_us, `${label}.iqr_us`);
  requireThat(Number.isInteger(duration.observations) && duration.observations >= 0,
    `${label}.observations is invalid`);
}

function validatePipelineStage(stage, label) {
  requireThat(plainObject(stage), `${label} stage is not an object`);
  requireThat(['measured', 'not_reached', 'not_applicable', 'shared'].includes(stage.status),
    `${label} stage status is invalid`);
  validateDuration(stage.duration_us, `${label}.duration_us`);
  if (stage.status === 'not_reached' || stage.status === 'not_applicable') {
    requireThat(stage.duration_us === null, `${label} refused stage has a duration`);
    requireThat(typeof stage.not_reached_reason === 'string' && stage.not_reached_reason.length > 0,
      `${label} not-reached stage has no reason`);
  } else {
    requireThat(plainObject(stage.duration_us), `${label} observed stage has no duration`);
  }
}

function validatePipeline(row, index) {
  const label = `rows[${index}].pipeline`;
  const pipeline = row.pipeline;
  requireThat(plainObject(pipeline), `${label} is not an object`);
  requireThat(typeof pipeline.schema === 'string' && pipeline.schema.length > 0,
    `${label}.schema is missing`);
  requireThat(dispositions.includes(pipeline.disposition), `${label}.disposition is invalid`);
  requireThat(pipeline.samples_per_row === 200 && pipeline.priming_pairs === 64,
    `${label} sampling contract changed`);
  for (const field of ['candidate_execution_calls', 'baseline_execution_calls']) {
    requireThat(Number.isInteger(pipeline[field]) && pipeline[field] >= 0,
      `${label}.${field} is invalid`);
  }
  requireThat(plainObject(pipeline.source), `${label}.source is missing`);
  requireThat(plainObject(pipeline.baseline), `${label}.baseline is missing`);
  requireThat(plainObject(pipeline.certified), `${label}.certified is missing`);
  requireThat(dispositions.includes(pipeline.baseline.disposition),
    `${label}.baseline.disposition is invalid`);
  requireThat(dispositions.includes(pipeline.certified.disposition),
    `${label}.certified.disposition is invalid`);
  if (pipeline.comparison !== undefined) {
    requireThat(plainObject(pipeline.comparison), `${label}.comparison is not an object`);
  }
  if (noCandidateExecution.has(pipeline.disposition)) {
    requireThat(pipeline.candidate_execution_calls === 0,
      `${label} records candidate execution for a refused pipeline`);
  }
  for (const [name, attempt] of [['baseline', pipeline.baseline], ['certified', pipeline.certified]]) {
    requireThat(plainObject(attempt.stages), `${label}.${name}.stages is not an object`);
    for (const stageName of pipelineStages) {
      requireThat(Object.hasOwn(attempt.stages, stageName),
        `${label}.${name}.stages lacks ${stageName}`);
      validatePipelineStage(attempt.stages[stageName], `${label}.${name}.stages.${stageName}`);
    }
  }
  const construction = pipeline.certified.stages.certificate_construction;
  if (construction.status === 'shared') {
    requireThat(construction.additive === false && construction.shared_with === 'proposal_search',
      `${label}.certified certificate construction is not a shared proposal stage`);
  } else {
    requireThat(construction.status === 'not_reached' && construction.duration_us === null &&
      typeof construction.not_reached_reason === 'string' && construction.not_reached_reason.length > 0,
    `${label}.certified certificate construction is neither shared nor explicitly unreached`);
  }
  requireThat(Array.isArray(pipeline.unmeasured_stages), `${label}.unmeasured_stages is missing`);
  const unmeasured = new Map(pipeline.unmeasured_stages.map(stage => [stage.name, stage]));
  requireThat(unmeasured.size === pipeline.unmeasured_stages.length &&
    unmeasured.size >= unmeasuredPipelineStages.length,
    `${label}.unmeasured_stages roster changed`);
  for (const stageName of unmeasuredPipelineStages) {
    const stage = unmeasured.get(stageName);
    requireThat(plainObject(stage) && stage.status === 'unmeasured' &&
      typeof stage.reason === 'string' && stage.reason.length > 0,
    `${label}.unmeasured_stages lacks ${stageName}`);
  }
}

function validateReport(report, manifest, corpus) {
  requireThat(report.schema === 'noble-rewrite-cost-report/v2', 'wrong rewrite cost report schema');
  requireThat(report.corpus === corpusRelative.split('/').pop(), 'report corpus changed');
  requireThat(report.samples_per_row === 200, 'report sample count is not 200');
  requireThat(report.identity_scheme === 'test-siphash13x4', 'report identity scheme changed');
  requireThat(plainObject(report.threshold), 'report threshold is missing');
  requireThat(report.threshold.residual === manifest.threshold.residual &&
    report.threshold.check_per_step_us === manifest.threshold.check_per_step_us,
  'report threshold differs from frozen v2 plan');
  requireThat(Array.isArray(report.stages) && report.stages.length === manifest.stages.length,
    'report stage roster differs from frozen v2 plan');
  for (const [index, expected] of manifest.stages.entries()) {
    const actual = report.stages[index];
    requireThat(actual.name === expected.name && actual.status === expected.status,
      `report stage ${expected.name} differs from frozen declaration`);
    requireThat(actual.reason === expected.reason,
      `report stage ${expected.name} observation reason differs from frozen declaration`);
  }
  requireThat(Array.isArray(report.rows) && report.rows.length === 28, 'report must retain 28 rows');
  const fixtureNames = new Set(corpus.map(fixture => fixture.name));
  const rowKeys = new Set();
  const dispositionCounts = Object.fromEntries(dispositions.map(disposition => [disposition, 0]));
  let rawRefused = 0;
  let rawTargetReached = 0;
  for (const [index, row] of report.rows.entries()) {
    requireThat(plainObject(row), `row ${index} is not an object`);
    for (const field of rawRowFields) requireThat(Object.hasOwn(row, field), `row ${index} lacks ${field}`);
    requireThat(fixtureNames.has(row.fixture), `row ${index} names an unknown fixture`);
    requireThat(proposers.includes(row.proposer), `row ${index} names an unknown proposer`);
    const key = `${row.fixture}\0${row.proposer}`;
    requireThat(!rowKeys.has(key), `duplicate row ${row.fixture}/${row.proposer}`);
    rowKeys.add(key);
    requireThat(Number.isInteger(row.steps) && row.steps >= 0, `row ${index}.steps is invalid`);
    requireThat(typeof row.refused === 'boolean', `row ${index}.refused is invalid`);
    requireThat(Number.isInteger(row.residual) && row.residual >= 0, `row ${index}.residual is invalid`);
    for (const field of ['propose_us', 'propose_iqr_us', 'check_us', 'check_iqr_us',
      'check_per_step_us']) {
      requireThat(row[field] === null || (typeof row[field] === 'number' && Number.isFinite(row[field])),
        `row ${index}.${field} is invalid`);
    }
    requireThat(Number.isInteger(row.in_memory_footprint_bytes) && row.in_memory_footprint_bytes >= 0,
      `row ${index}.in_memory_footprint_bytes is invalid`);
    requireThat(typeof row.check_threshold_met === 'boolean',
      `row ${index}.check_threshold_met is invalid`);
    validatePipeline(row, index);
    dispositionCounts[row.pipeline.disposition]++;
    if (row.refused) rawRefused++;
    if (!row.refused && row.residual === 0) rawTargetReached++;
  }
  requireThat(rowKeys.size === 28 && [...fixtureNames].every(fixture =>
    proposers.every(proposer => rowKeys.has(`${fixture}\0${proposer}`))),
  'report does not contain every fixture/proposer pair');
  const certificate = report.certificate_coverage;
  requireThat(plainObject(certificate), 'certificate_coverage is missing');
  for (const field of ['attempts', 'accepted', 'refused', 'target_reached']) {
    requireThat(Number.isInteger(certificate[field]) && certificate[field] >= 0,
      `certificate_coverage.${field} is invalid`);
  }
  requireThat(certificate.attempts === 28 && certificate.accepted + certificate.refused === 28,
    'certificate_coverage denominator is invalid');
  requireThat(certificate.refused === rawRefused && certificate.accepted === 28 - rawRefused,
    'certificate_coverage does not describe raw row refusals');
  requireThat(certificate.target_reached === rawTargetReached,
    'certificate_coverage target count is not derived from raw rows');
  const pipeline = report.pipeline_coverage;
  requireThat(plainObject(pipeline), 'pipeline_coverage is missing');
  requireThat(pipeline.attempts === 28, 'pipeline_coverage denominator is not 28');
  for (const disposition of dispositions) {
    requireThat(Number.isInteger(pipeline[disposition.replaceAll('-', '_')]) &&
      pipeline[disposition.replaceAll('-', '_')] >= 0,
    `pipeline_coverage.${disposition} is invalid`);
    requireThat(pipeline[disposition.replaceAll('-', '_')] === dispositionCounts[disposition],
      `pipeline_coverage.${disposition} disagrees with rows`);
  }
  const total = dispositions.reduce((sum, disposition) => sum + dispositionCounts[disposition], 0);
  requireThat(total === 28, 'pipeline disposition counts do not cover all rows');
}

function evidenceHashes(evidence) {
  const files = [
    'source-before.json', 'source-after.json', 'process.json', 'raw/stdout', 'raw/stderr',
    'cargo-home/config.toml',
  ];
  return Object.fromEntries(files.map(relative => {
    const file = path.join(evidence, relative);
    const bytes = fs.readFileSync(file);
    return [relative, { sha256: sha256(bytes), bytes: bytes.length }];
  }));
}

function failureRecord(evidence, phase, error, before, processRecord) {
  if (evidence === null) return;
  const file = path.join(evidence, 'failure.json');
  if (fs.existsSync(file)) return;
  try {
    writeJsonExclusive(file, {
      schema: 'noble-rewrite-cost-failure/v2',
      phase,
      error: { message: error.message, name: error.name },
      source_before: before,
      process: processRecord,
    });
  } catch (writeError) {
    console.error(`could not retain failure record: ${writeError.message}`);
  }
}

let options;
try {
  options = parseArgs(process.argv.slice(2));
} catch (error) {
  console.error(error.message);
  process.exitCode = 2;
}

if (options !== undefined) {
  let evidence = null;
  let phase = 'preflight';
  let before = null;
  let processRecord = null;
  try {
    evidence = options.evidence;
    fs.mkdirSync(evidence);
    phase = 'source-preflight';
    const initial = readSourceSnapshot();
    before = initial.snapshot;
    writeJsonExclusive(path.join(evidence, 'source-before.json'), before);
    requireThat(before.clean_source, `source worktree is dirty: ${before.status}`);

    phase = 'toolchain-selection';
    const selectionFile = path.join(root, policyRelative);
    const selection = JSON.parse(fs.readFileSync(selectionFile, 'utf8'));
    const toolchain = selectToolchain(selection, evidence);
    const cargo = executableIdentity(toolchain.cargo);
    const rustc = executableIdentity(toolchain.rustc);
    const rustdoc = executableIdentity(toolchain.rustdoc);
    const linker = executableIdentity(toolchain.linker);
    const test = {
      executable: cargo,
      argv: [...commandArgs],
      full_argv: [cargo.path, ...commandArgs],
      cwd: root,
      environment: toolchain.environment,
      text: `cargo ${commandArgs.join(' ')}`,
    };

    phase = 'test-execution';
    const run = runTest(toolchain, evidence, cargo);
    processRecord = run.record;

    phase = 'source-postflight';
    const final = readSourceSnapshot();
    writeJsonExclusive(path.join(evidence, 'source-after.json'), final.snapshot);
    requireThat(JSON.stringify(final.snapshot) === JSON.stringify(before),
      'source commit/tree/status or bound source hashes changed during execution');
    phase = 'process-result';
    requireThat(processRecord.ok,
      `rewrite_cost process failed: exit=${processRecord.exit} signal=${processRecord.signal}`);

    phase = 'test-artifact-binding';
    const executed = [...run.stderr.matchAll(/Running tests\/rewrite_cost\.rs \(([^\r\n]+)\)/gu)];
    requireThat(executed.length === 1, 'Cargo did not name exactly one rewrite_cost executable');
    const testExecutable = executableIdentity(executed[0][1]);
    requireThat(path.dirname(testExecutable.path) === path.join(toolchain.environment.CARGO_TARGET_DIR, 'debug/deps'),
      'test executable is outside the isolated Cargo target');

    phase = 'report-validation';
    const report = reportFromStreams(run.stdout, run.stderr);
    validateReport(report, initial.manifest, initial.corpus);
    const custodyCheck = readSourceSnapshot();
    requireThat(JSON.stringify(custodyCheck.snapshot) === JSON.stringify(before),
      'source commit/tree/status or bound source hashes changed before receipt write');
    const files = evidenceHashes(evidence);
    const receipt = {
      schema: 'noble-rewrite-cost-receipt/v2',
      issue: 64,
      source_commit: before.source_commit,
      source_tree: before.source_tree,
      clean_source: true,
      plan_commit: before.plan_commit,
      plan_sha256: before.plan_sha256,
      manifest: manifestRelative,
      manifest_sha256: before.files[manifestRelative].sha256,
      corpus: corpusRelative,
      corpus_sha256: before.files[corpusRelative].sha256,
      lens: before.lens,
      identity_scheme: report.identity_scheme,
      source: {
        before,
        after: final.snapshot,
        unchanged: true,
      },
      toolchain: {
        policy: toolchain.policy,
        node: { ...executableIdentity(process.execPath), version: process.versions.node },
        vendor: {
          directory: toolchain.vendor.directory,
          nar_hash: toolchain.vendor.narHash,
          recipe: toolchain.recipe,
        },
        quality_rust: toolchain.policy.quality_rust,
        cargo,
        rustc,
        rustdoc,
        linker,
      },
      command: test,
      command_text: test.text,
      test_executable: testExecutable,
      process: processRecord,
      host: {
        platform: `${os.type()} ${os.release()} ${os.arch()}`,
        shared: true,
        timing_scope: 'informational shared-host timings; not a retention gate',
      },
      scope: {
        identity: 'actual test-siphash13x4 library calls',
        fresh_extraction: false,
        deployment_timing: false,
        wire_decoder: false,
      },
      evidence: {
        directory: evidence,
        files,
      },
      report,
    };
    phase = 'receipt-write';
    writeExclusive(options.output, json(receipt));
    console.log(JSON.stringify({
      outcome: 'recorded',
      output: options.output,
      evidence: options.evidence,
      source_commit: receipt.source_commit,
      rows: report.rows.length,
      certificate_coverage: report.certificate_coverage,
      pipeline_coverage: report.pipeline_coverage,
    }));
  } catch (error) {
    failureRecord(evidence, phase, error, before, processRecord);
    console.error(`rewrite-cost v2 failed (${phase}): ${error.message}`);
    process.exitCode = 1;
  }
}
