#!/usr/bin/env node
// Issue #62 protected-semantics lane (exported-closure fallback).
//
// `lake comparator` is not usable on this host because its required `bwrap` executable is absent
// from the execution PATH. Instead, the separate challenge and solution packages are built with the
// policy-pinned Lean 4.35.0 toolchain and exported by its bundled `leanexport`. This lane
// compares the exported theorem statements and the type/body of every semantic dependency reached
// from those statements. It excludes theorem proof bodies from the semantic digest and records a
// separate proof-closure digest. This is not a claim that the local comparison implements all of
// `lake comparator`; the only permitted definition holes are checked empty.
//
// The eight fixture cases are: a real proof, an alternate proof, a weakened predicate under the same
// theorem names/statements, changed imported semantics under the same theorem names/statements, a
// wrong-statement proof, a forbidden-axiom proof, an invalid proof rejected by Lean, and an
// export-level missing-definition control. They exercise five distinct refusal classes plus
// acceptance. Proofs are checked by each project build's Lean
// kernel; the exported-closure comparison is the protected-definition check. Opt-in evidence, not a gate.
import { spawnSync } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { closureDigest, closureOf, dropDeclaration, exportStatement, parseExport } from './binding-core.mjs';
import { foreignAxioms, inspectExport } from './ndjson.mjs';
import { pinGuard } from './pins.mjs';

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
const root = process.cwd();

const leanBin = path.resolve(value('--lean-bin'));
const work = path.resolve(value('--work'));
const output = path.resolve(value('--output'));
const toolchain = optional('--toolchain');
const pins = {
  lean: value('--lean-sha256'),
  lake: value('--lake-sha256'),
  leanexport: value('--leanexport-sha256'),
};
const checkersPath = path.join(here, 'checkers.json');
const checkersPin = JSON.parse(fs.readFileSync(checkersPath, 'utf8'));
const lake = path.join(leanBin, 'lake');
const leanexport = path.join(leanBin, 'leanexport');
const nanoda = path.join(leanBin, 'nanoda_bin');
const nanodaPin = checkersPin.checkers.find((checker) => checker.id === 'nanoda')?.sha256;
if (!nanodaPin) throw new Error('checkers.json has no nanoda binary pin');

pinGuard([
  { role: 'lean', file: path.join(leanBin, 'lean'), sha256: pins.lean, pinClass: 'tool-pin-mismatch' },
  { role: 'lake', file: lake, sha256: pins.lake, pinClass: 'tool-pin-mismatch' },
  { role: 'leanexport', file: leanexport, sha256: pins.leanexport, pinClass: 'tool-pin-mismatch' },
  { role: 'nanoda', file: nanoda, sha256: nanodaPin, pinClass: 'tool-pin-mismatch' },
]);
if (fs.existsSync(work)) throw new Error(`work directory exists: ${work}`);
if (fs.existsSync(output)) throw new Error(`output already exists: ${output}`);
fs.mkdirSync(work, { recursive: true });

const PROJECTS = {
  challenge: path.join(here, 'protected/challenge'),
  real: path.join(here, 'protected/solutions/real'),
  alternate: path.join(here, 'protected/solutions/alternate'),
  'weakened-predicate': path.join(here, 'protected/solutions/weakened'),
  'changed-semantics': path.join(here, 'protected/solutions/changed-semantics'),
  'wrong-statement': path.join(here, 'protected/solutions/wrong-statement'),
  'forbidden-assumption': path.join(here, 'protected/solutions/forbidden-assumption'),
  'kernel-rejection': path.join(here, 'protected/solutions/kernel-rejection'),
};
const configPath = path.join(here, 'protected/challenge.json');
const config = JSON.parse(fs.readFileSync(configPath, 'utf8'));
const THEOREMS = config.theorem_names;
const PERMITTED_AXIOMS = config.assumption_policy.permitted_axioms;
const EXPECTED_FORMAT = checkersPin.export_format.version;
const CONTROL_AXIOM = 'Protected.external_zero';
if (config.assumption_policy.definition_holes.length !== 0) {
  throw new Error('the exported-closure fallback does not support definition holes; challenge must list none');
}
if (!config.challenge_id || !config.model?.name || !config.model?.version
    || config.model.semantic_root !== 'Protected.threshold'
    || config.model.parameters?.threshold !== 32
    || !Array.isArray(config.external_contracts)
    || !config.external_contracts.some((contract) =>
      contract.name === 'Lean.Init/Nat' && contract.version === checkersPin.toolchain.lean)) {
  throw new Error('challenge must freeze its identity, model/version, and external contracts');
}
const childEnv = {
  HOME: process.env.HOME ?? '/', LANG: 'C.UTF-8',
  PATH: [leanBin, '/run/current-system/sw/bin', '/usr/bin', '/bin'].join(':'),
  TMPDIR: work,
};
const bwrapSearchPath = childEnv.PATH.split(path.delimiter);
const bwrapAvailable = bwrapSearchPath.some((directory) => {
  try {
    fs.accessSync(path.join(directory, 'bwrap'), fs.constants.X_OK);
    return true;
  } catch {
    return false;
  }
});

function run(argvRun, cwd) {
  const started = process.hrtime.bigint();
  const result = spawnSync(argvRun[0], argvRun.slice(1), {
    cwd, env: childEnv, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024,
    timeout: 1_800_000, killSignal: 'SIGKILL',
  });
  return {
    argv: argvRun.map((part) => part.replace(work, '$WORK').replace(root, '$ROOT')),
    project: path.relative(root, cwd),
    exit_code: result.status,
    signal: result.signal,
    elapsed_ms: Number((process.hrtime.bigint() - started) / 1_000_000n),
    stdout: (result.stdout ?? '').trim().slice(-1200),
    stderr: (result.stderr ?? '').trim().slice(-1200),
    timed_out: result.error?.code === 'ETIMEDOUT',
  };
}

/** Build one isolated fixture project and export its exact theorem roots. */
function buildAndExport(project, library, declarations,
  { allowForeignAxioms = false, expectKernelRejection = false } = {}) {
  const build = run([lake, 'build'], project);
  if (build.exit_code !== 0 || build.signal || build.timed_out) {
    const detail = `${build.stdout}\n${build.stderr}`.trim();
    const className = expectKernelRejection && /type mismatch/iu.test(detail)
      ? 'kernel-rejection' : 'build-failure';
    return { build, failure: { class: className, detail } };
  }
  const exportPath = path.join(work, `${library}-${path.basename(project)}.ndjson`);
  const descriptor = fs.openSync(exportPath, 'w');
  const exported = spawnSync(lake, ['env', leanexport, library, '--', ...declarations], {
    cwd: project, env: childEnv, encoding: 'utf8', stdio: ['ignore', descriptor, 'pipe'],
    maxBuffer: 64 * 1024 * 1024, timeout: 1_800_000, killSignal: 'SIGKILL',
  });
  fs.closeSync(descriptor);
  const stage = {
    argv: ['lake', 'env', 'leanexport', library, '--', ...declarations],
    exit_code: exported.status,
    signal: exported.signal,
    timed_out: exported.error?.code === 'ETIMEDOUT',
    panic: /PANIC/u.test(exported.stderr ?? ''),
    stderr_excerpt: (exported.stderr ?? '').split('\nbacktrace:')[0].trim().slice(0, 500),
  };
  if (stage.exit_code !== 0 || stage.signal || stage.timed_out || stage.panic) {
    return { build, stage, exportPath, failure: { class: 'export-failure', detail: stage.stderr_excerpt } };
  }
  let parsed;
  let inspection;
  try {
    parsed = parseExport(exportPath);
    inspection = inspectExport(exportPath);
  } catch (error) {
    return { build, stage, exportPath, failure: { class: 'export-failure', detail: String(error) } };
  }
  const absent = declarations.filter((name) => !parsed.decls.has(name));
  if (absent.length > 0) {
    return { build, stage, exportPath, parsed, inspection,
      failure: { class: 'export-failure', detail: `roots absent: ${absent.join(', ')}` } };
  }
  if (parsed.format !== EXPECTED_FORMAT) {
    return { build, stage, exportPath, parsed, inspection,
      failure: { class: 'unsupported-format', detail: `${parsed.format} != ${EXPECTED_FORMAT}` } };
  }
  const foreign = foreignAxioms(inspection).filter((name) => !PERMITTED_AXIOMS.includes(name));
  if (foreign.length > 0 && !allowForeignAxioms) {
    return { build, stage, exportPath, parsed, inspection,
      failure: { class: 'forbidden-axiom', detail: foreign.join(', ') } };
  }
  return {
    build, stage, exportPath, parsed, inspection,
    record: {
      sha256: fileSha256(exportPath), size_bytes: fs.statSync(exportPath).size,
      records: parsed.records, format: parsed.format, declarations: parsed.decls.size,
      axioms: parsed.axioms, foreign_axioms: foreign,
    },
  };
}

/** Semantic closure from statement types through all reached definitions, but not theorem proofs. */
function semanticClosure(parsed, roots) {
  const targets = new Set(roots);
  const reachable = new Set();
  const missing = new Set();
  const queue = [];
  const edges = new Map();
  const dependencies = (indices) => {
    const found = new Set();
    const visited = new Set();
    const stack = indices.filter((index) => index !== undefined && index !== null);
    while (stack.length > 0) {
      const index = stack.pop();
      if (visited.has(index)) continue;
      visited.add(index);
      const expr = parsed.exprs.get(index);
      if (!expr) throw new Error(`expression ${index} is not defined`);
      if (expr.const) found.add(parsed.names.get(expr.const.name));
      else if (expr.app) stack.push(expr.app.fn, expr.app.arg);
      else if (expr.lam) stack.push(expr.lam.type, expr.lam.body);
      else if (expr.forallE) stack.push(expr.forallE.type, expr.forallE.body);
      else if (expr.letE) stack.push(expr.letE.type, expr.letE.value, expr.letE.body);
      else if (expr.mdata) stack.push(expr.mdata.expr);
      else if (expr.proj) {
        found.add(parsed.names.get(expr.proj.typeName));
        stack.push(expr.proj.struct);
      }
    }
    return [...found].sort();
  };
  queue.push(...roots);
  while (queue.length > 0) {
    const name = queue.pop();
    if (reachable.has(name) || missing.has(name)) continue;
    const decl = parsed.decls.get(name);
    if (!decl) {
      missing.add(name);
      continue;
    }
    reachable.add(name);
    const indices = decl.kind === 'inductive'
      ? decl.group.exprs
      : [decl.record.type,
        ...(!targets.has(name) && (decl.kind === 'def' || decl.kind === 'opaque')
          ? [decl.record.value]
          : [])];
    const names = dependencies(indices);
    edges.set(name, names);
    queue.push(...names.filter((dependency) => !reachable.has(dependency)));
  }
  const names = [...reachable].filter((name) => !targets.has(name)).sort();
  const entries = Object.fromEntries(names.map((name) => [name, closureDigest(parsed, new Set([name])).sha256]));
  const dependencyPath = (target) => {
    const starts = [...roots].sort();
    const pending = starts.map((name) => [name]);
    const visited = new Set();
    while (pending.length > 0) {
      const candidate = pending.shift();
      const current = candidate[candidate.length - 1];
      if (current === target) return candidate;
      if (visited.has(current)) continue;
      visited.add(current);
      for (const next of edges.get(current) ?? []) {
        if (!visited.has(next)) pending.push([...candidate, next]);
      }
    }
    return null;
  };
  const paths = Object.fromEntries([...new Set([...names, ...missing])].sort()
    .map((name) => [name, dependencyPath(name)]));
  return {
    declarations: names.length,
    names,
    missing: [...missing].sort(),
    digest: closureDigest(parsed, new Set(names)),
    declaration_digests: entries,
    dependency_paths: paths,
  };
}

function proofClosure(parsed, roots) {
  const { reachable, missing } = closureOf(parsed, roots);
  return {
    declarations: reachable.size,
    missing: [...missing].sort(),
    digest: missing.size === 0 ? closureDigest(parsed, reachable) : null,
  };
}

function rootStatements(parsed) {
  return Object.fromEntries(THEOREMS.map((name) => {
    const declaration = parsed.decls.get(name);
    if (!declaration) return [name, null];
    return [name, sha256(exportStatement(parsed, name))];
  }));
}

const challenge = buildAndExport(PROJECTS.challenge, config.challenge_module, THEOREMS);
if (challenge.failure) throw new Error(`challenge ${challenge.failure.class}: ${challenge.failure.detail}`);
const challengeStatements = rootStatements(challenge.parsed);
const challengeSemantic = semanticClosure(challenge.parsed, THEOREMS);
if (challengeSemantic.missing.length > 0) {
  throw new Error(`challenge semantic closure is incomplete: ${challengeSemantic.missing.join(', ')}`);
}

const solutionBuilds = new Map();
for (const id of ['real', 'alternate', 'weakened-predicate', 'changed-semantics', 'wrong-statement',
  'forbidden-assumption', 'kernel-rejection']) {
  solutionBuilds.set(id, buildAndExport(PROJECTS[id], config.solution_module, THEOREMS, {
    allowForeignAxioms: id === 'forbidden-assumption',
    expectKernelRejection: id === 'kernel-rejection',
  }));
}

/** The export-side comparison retains distinct failure, statement, definition and missing classes. */
function compareSolution(id, candidate) {
  const entry = {
    id,
    project: PROJECTS[id] ? path.relative(root, PROJECTS[id]) : candidate.build?.project ?? null,
    build: candidate.build,
    export: candidate.record ?? null,
    export_stage: candidate.stage ?? null,
  };
  if (candidate.failure) {
    entry.verdict = candidate.failure;
    return entry;
  }
  const statements = rootStatements(candidate.parsed);
  entry.root_statement_sha256 = statements;
  const foreign = foreignAxioms(candidate.inspection).filter((name) => !PERMITTED_AXIOMS.includes(name));
  entry.foreign_axioms = foreign;
  if (foreign.length > 0) {
    entry.verdict = { class: 'forbidden-axiom', detail: foreign[0], forbidden: foreign };
    return entry;
  }
  const statementMismatch = THEOREMS.find((name) => statements[name] !== challengeStatements[name]);
  if (statementMismatch) {
    entry.verdict = { class: 'statement-mismatch', detail: statementMismatch, path: [statementMismatch] };
    return entry;
  }

  const semantic = semanticClosure(candidate.parsed, THEOREMS);
  entry.semantic_closure = semantic;
  const expectedNames = new Set(challengeSemantic.names);
  const actualNames = new Set(semantic.names);
  const missing = [...new Set([
    ...semantic.missing,
    ...challengeSemantic.names.filter((name) => !actualNames.has(name)),
  ])].sort();
  if (missing.length > 0) {
    entry.verdict = {
      class: 'const-missing-in-solution', detail: missing[0], missing,
      dependency_paths: Object.fromEntries(missing.map((name) => [name, semantic.dependency_paths[name] ?? null])),
    };
    return entry;
  }
  const changed = challengeSemantic.names.filter((name) =>
    semantic.declaration_digests[name] !== challengeSemantic.declaration_digests[name]);
  if (changed.length > 0) {
    entry.verdict = {
      class: 'definition-mismatch', detail: changed[0], changed,
      dependency_paths: Object.fromEntries(changed.map((name) => [name, semantic.dependency_paths[name] ?? null])),
    };
    return entry;
  }
  const newNames = semantic.names.filter((name) => !expectedNames.has(name));
  if (newNames.length > 0) {
    entry.verdict = {
      class: 'definition-mismatch', detail: newNames[0], changed: newNames,
      dependency_paths: Object.fromEntries(newNames.map((name) => [name, semantic.dependency_paths[name] ?? null])),
    };
    return entry;
  }
  entry.verdict = { class: 'accepted', detail: null };
  return entry;
}

const runs = [...solutionBuilds].map(([id, candidate]) => compareSolution(id, candidate));
const real = solutionBuilds.get('real');
const realParsed = real.parsed;
if (!realParsed) throw new Error(`real solution build failed: ${real.failure?.detail ?? 'no export'}`);
const missingDefinitionName = 'Protected.threshold';
const missingDefinition = (() => {
  const file = dropDeclaration('missing-definition', real.exportPath, missingDefinitionName, work);
  const parsed = parseExport(file);
  const inspection = inspectExport(file);
  return {
    file,
    parsed,
    inspection,
    record: {
      sha256: fileSha256(file), size_bytes: fs.statSync(file).size,
      records: parsed.records, format: parsed.format, declarations: parsed.decls.size,
      removed: missingDefinitionName, axioms: inspection.axioms,
    },
  };
})();
runs.push(compareSolution('missing-definition', {
  build: { project: path.relative(root, PROJECTS.real), from_export: real.record.sha256 },
  record: missingDefinition.record,
  parsed: missingDefinition.parsed,
  inspection: missingDefinition.inspection,
}));

const byId = (id) => runs.find((entry) => entry.id === id);
const verdictOf = (id) => byId(id)?.verdict.class;
const detailOf = (id) => byId(id)?.verdict.detail;
const expectedVerdicts = {
  real: ['accepted', null],
  alternate: ['accepted', null],
  'weakened-predicate': ['definition-mismatch', 'Protected.allowed'],
  'changed-semantics': ['definition-mismatch', 'Protected.threshold'],
  'wrong-statement': ['statement-mismatch', THEOREMS[0]],
  'forbidden-assumption': ['forbidden-axiom', CONTROL_AXIOM],
  'kernel-rejection': ['kernel-rejection', null],
  'missing-definition': ['const-missing-in-solution', missingDefinitionName],
};
const sourcePaths = Object.values(PROJECTS).flatMap((project) => {
  const files = [];
  const visit = (directory) => {
    for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
      if (entry.name === '.lake') continue;
      const full = path.join(directory, entry.name);
      if (entry.isDirectory()) visit(full);
      else if (entry.isFile() && (entry.name.endsWith('.lean') || entry.name === 'lakefile.toml'
          || entry.name === 'lean-toolchain' || entry.name === 'lake-manifest.json')) files.push(full);
    }
  };
  visit(project);
  return files;
}).sort();
const sourceEvidence = sourcePaths.map((file) => ({ path: path.relative(root, file), sha256: fileSha256(file) }));
const proofClosures = Object.fromEntries([
  ['challenge', challenge.parsed],
  ...[...solutionBuilds].filter(([, candidate]) => candidate.parsed).map(([id, candidate]) => [id, candidate.parsed]),
].map(([id, parsed]) => [id, proofClosure(parsed, THEOREMS)]));
const allExported = [challenge, ...solutionBuilds.values()];
const legitimateExports = allExported.filter((entry) =>
  entry !== solutionBuilds.get('forbidden-assumption')
    && entry !== solutionBuilds.get('kernel-rejection'));
const checks = {
  challenge_configuration_is_explicit:
    config.challenge_id === 'noble-issue-62-protected-bound/v1'
      && config.model.name.length > 0 && config.model.version.length > 0
      && config.model.semantic_root === 'Protected.threshold'
      && config.model.parameters.threshold === 32
      && config.external_contracts.some((contract) =>
        contract.name === 'Lean.Init/Nat' && contract.version === checkersPin.toolchain.lean),
  challenge_declares_all_targets: THEOREMS.every((name) => challenge.parsed.decls.has(name)),
  challenge_semantic_closure_complete: challengeSemantic.missing.length === 0,
  every_valid_fixture_builds_and_exports: allExported
    .filter((entry) => entry !== solutionBuilds.get('kernel-rejection'))
    .every((entry) => entry.record !== undefined && entry.failure === undefined),
  invalid_proof_is_rejected_by_the_lean_kernel:
    verdictOf('kernel-rejection') === expectedVerdicts['kernel-rejection'][0]
      && byId('kernel-rejection')?.build?.exit_code !== 0
      && /type mismatch/iu.test(`${byId('kernel-rejection')?.build?.stdout ?? ''}\n${byId('kernel-rejection')?.build?.stderr ?? ''}`),
  every_legitimate_export_has_only_permitted_axioms: legitimateExports.every((entry) =>
    entry.inspection && foreignAxioms(entry.inspection).every((name) => PERMITTED_AXIOMS.includes(name))),
  real_solution_accepted: verdictOf('real') === expectedVerdicts.real[0],
  alternate_proof_accepted: verdictOf('alternate') === expectedVerdicts.alternate[0],
  weakened_predicate_refused_on_its_definition:
    verdictOf('weakened-predicate') === expectedVerdicts['weakened-predicate'][0]
      && detailOf('weakened-predicate') === expectedVerdicts['weakened-predicate'][1],
  changed_imported_semantics_refused_on_its_definition:
    verdictOf('changed-semantics') === expectedVerdicts['changed-semantics'][0]
      && detailOf('changed-semantics') === expectedVerdicts['changed-semantics'][1],
  wrong_statement_refused_as_statement_mismatch:
    verdictOf('wrong-statement') === expectedVerdicts['wrong-statement'][0]
      && detailOf('wrong-statement') === expectedVerdicts['wrong-statement'][1],
  forbidden_assumption_refused_as_axiom:
    verdictOf('forbidden-assumption') === expectedVerdicts['forbidden-assumption'][0]
      && detailOf('forbidden-assumption') === expectedVerdicts['forbidden-assumption'][1]
      && byId('forbidden-assumption')?.foreign_axioms?.includes(CONTROL_AXIOM),
  missing_definition_refused_by_name:
    verdictOf('missing-definition') === expectedVerdicts['missing-definition'][0]
      && detailOf('missing-definition') === expectedVerdicts['missing-definition'][1],
  every_statement_preserving_solution_matches_the_challenge: runs
    .filter((entry) => !['wrong-statement', 'kernel-rejection'].includes(entry.id))
    .every((entry) =>
    entry.root_statement_sha256 && THEOREMS.every((name) => entry.root_statement_sha256[name] === challengeStatements[name])),
  honest_solutions_reproduce_the_challenge_semantic_closure:
    ['real', 'alternate'].every((id) => byId(id)?.semantic_closure?.digest.sha256 === challengeSemantic.digest.sha256),
  mutated_semantics_do_not_reproduce_the_challenge_digest:
    ['weakened-predicate', 'changed-semantics'].every((id) =>
      byId(id)?.semantic_closure?.digest.sha256 !== challengeSemantic.digest.sha256),
  proof_and_semantic_closures_are_distinctly_recorded:
    challengeSemantic.digest.sha256 !== proofClosures.real?.digest?.sha256,
  proof_closure_distinguishes_all_three_proof_terms:
    new Set(['challenge', 'real', 'alternate'].map((id) => proofClosures[id]?.digest?.sha256)).size === 3,
  diagnostic_classes_remain_distinct:
    runs.length === 8
      && Object.keys(expectedVerdicts).every((id) => byId(id) !== undefined)
      && ['definition-mismatch', 'statement-mismatch', 'const-missing-in-solution',
        'forbidden-axiom', 'kernel-rejection'].every((className) =>
        runs.some((entry) => entry.verdict.class === className))
      && new Set(runs.map((entry) => entry.verdict.class)).size === 6,
  semantic_mismatch_reports_dependency_paths:
    ['weakened-predicate', 'changed-semantics'].every((id) => {
      const mismatch = byId(id)?.verdict;
      return Array.isArray(mismatch?.dependency_paths?.[mismatch.detail])
        && THEOREMS.includes(mismatch.dependency_paths[mismatch.detail][0])
        && mismatch.dependency_paths[mismatch.detail].at(-1) === mismatch.detail;
    }),
  missing_definition_reports_dependency_path:
    Array.isArray(byId('missing-definition')?.verdict.dependency_paths?.[missingDefinitionName])
      && byId('missing-definition').verdict.dependency_paths[missingDefinitionName].at(-1) === missingDefinitionName,
};
const holds = Object.values(checks).every(Boolean);
const host = spawnSync('uname', ['-srm'], { encoding: 'utf8' }).stdout.trim();
const source = spawnSync('git', ['rev-parse', 'HEAD'], { cwd: process.cwd(), encoding: 'utf8' });
if (source.status !== 0 || source.stdout.trim() === '') {
  throw new Error(`cannot identify receipt source commit: ${source.stderr.trim()}`);
}
const receipt = {
  schema: 'noble-independent-replay-protected/v2',
  measured_on: new Date().toISOString().slice(0, 10),
  host,
  source_commit: source.stdout.trim(),
  toolchain,
  assertion: 'separately built real and alternate proofs preserve all challenge statements and protected semantic definitions; a weakened predicate and changed imported threshold are refused by per-definition exported-closure comparison with dependency paths; wrong statements and a forbidden axiom have distinct refusal classes; a solution export missing a protected definition is refused by exact name',
  holds,
  checks,
  comparison: {
    method: 'exported-definition-closure/v1',
    comparator: 'lake comparator was not run: bwrap is absent from the execution PATH; this lane uses the exported-closure fallback instead',
    bwrap_on_execution_path: bwrapAvailable,
    execution_path: childEnv.PATH,
    definition_holes: config.assumption_policy.definition_holes,
    model: config.model,
    external_contracts: config.external_contracts,
  },
  harness: {
    protected: { path: 'verification/independent-replay/protected.mjs', sha256: fileSha256(fileURLToPath(import.meta.url)) },
    challenge_config: { path: 'verification/independent-replay/protected/challenge.json', sha256: fileSha256(configPath) },
    checker_manifest: { path: 'verification/independent-replay/checkers.json', sha256: fileSha256(checkersPath) },
    sources: sourceEvidence,
  },
  tools: {
    lean: { binary_sha256: pins.lean },
    lake: { binary_sha256: pins.lake },
    leanexport: { binary_sha256: pins.leanexport },
    release: checkersPin.toolchain.output,
    derivation: checkersPin.toolchain.derivation,
  },
  configuration: config,
  challenge: {
    build: challenge.build,
    export: challenge.stage,
    export_record: challenge.record,
    root_statement_sha256: challengeStatements,
    semantic_closure: challengeSemantic,
  },
  proof_closures: proofClosures,
  missing_definition_control: missingDefinition.record,
  runs,
  non_claims: [
    'This exported-closure fallback is not lake comparator and does not inherit all of its sandbox or comparison guarantees.',
    'Valid solutions are checked by Lean 4.35.0; the invalid-proof control demonstrates kernel rejection, but no independent proof checker result is claimed for these protected-semantic fixture packages.',
    'The challenge is authored in this lane and is not an independent specification of any Noble theorem.',
    'The missing-definition case removes one declaration from a valid export; a source package omitting a definition it uses would not typecheck.',
    'No CI, flake, policy or gate change; promotion is an owner decision.',
  ],
};
fs.mkdirSync(path.dirname(output), { recursive: true });
fs.writeFileSync(output, `${JSON.stringify(receipt, null, 2)}\n`);
console.log(JSON.stringify({
  holds,
  checks,
  verdicts: Object.fromEntries(runs.map((entry) => [entry.id, `${entry.verdict.class}:${entry.verdict.detail ?? '-'}`])),
}));
process.exitCode = holds ? 0 : 1;
