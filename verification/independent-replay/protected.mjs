#!/usr/bin/env node
// Issue #62 protected-semantics lane (exported-closure fallback plus kernel replay).
//
// `lake comparator` is not usable on this host because its required `bwrap` executable is absent
// from the execution PATH. Instead, the separate challenge and solution packages are built with the
// policy-pinned Lean 4.35.0 toolchain and exported by its bundled `leanexport`. This lane
// compares the exported theorem statements and the type/body of every semantic dependency reached
// from those statements. It excludes theorem proof bodies from the semantic digest and records a
// separate proof-closure digest. This is not a claim that the local comparison implements all of
// `lake comparator`; the only permitted definition holes are checked empty.
//
// The original eight fixture cases are: a real proof, an alternate proof, a weakened predicate under the same
// theorem names/statements, changed imported semantics under the same theorem names/statements, a
// wrong-statement proof, a forbidden-axiom proof, an invalid proof rejected by Lean, and an
// export-level missing-definition control. They exercise five distinct refusal classes plus
// acceptance. The two valid solutions and an independently selected amendment are also replayed
// from their actual exports by Nanoda and Lean's bundled official `leanchecker --from-export`.
// An amended challenge may only be approved in a separate committed manifest after its own
// export has been kernel-checked, before the amended solution is built. Opt-in evidence, not a gate.
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
const rawOutput = `${output}.raw`;
const freezeAmendment = argv.includes('--freeze-amendment');
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
const leanchecker = path.join(leanBin, 'leanchecker');
// Lean 4.35.0 bundled official kernel; its `LeanChecker.lean` dispatches --from-export
// to parseStream + Kernel.Environment.replay and checks quotient declarations.
const LEANCHECKER_SHA256 = 'ec7d3c6574b27f0db2319da5cb015d3ea7806b0302c3a70c7164c517487937e7';
const nanodaPin = checkersPin.checkers.find((checker) => checker.id === 'nanoda')?.sha256;
if (!nanodaPin) throw new Error('checkers.json has no nanoda binary pin');

pinGuard([
  { role: 'lean', file: path.join(leanBin, 'lean'), sha256: pins.lean, pinClass: 'tool-pin-mismatch' },
  { role: 'lake', file: lake, sha256: pins.lake, pinClass: 'tool-pin-mismatch' },
  { role: 'leanexport', file: leanexport, sha256: pins.leanexport, pinClass: 'tool-pin-mismatch' },
  { role: 'nanoda', file: nanoda, sha256: nanodaPin, pinClass: 'tool-pin-mismatch' },
  { role: 'leanchecker', file: leanchecker, sha256: LEANCHECKER_SHA256, pinClass: 'tool-pin-mismatch' },
]);
if (fs.existsSync(work)) throw new Error(`work directory exists: ${work}`);
if (fs.existsSync(output)) throw new Error(`output already exists: ${output}`);
if (fs.existsSync(rawOutput)) throw new Error(`raw output already exists: ${rawOutput}`);
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
  'amended-challenge': path.join(here, 'protected/amendment/challenge'),
  'amended-solution': path.join(here, 'protected/solutions/amended'),
};
const configPath = path.join(here, 'protected/challenge.json');
const config = JSON.parse(fs.readFileSync(configPath, 'utf8'));
const amendedConfigPath = path.join(here, 'protected/amendment/challenge.json');
const amendedConfig = JSON.parse(fs.readFileSync(amendedConfigPath, 'utf8'));
const amendedApprovalPath = path.join(here, 'protected/amendment/approval.json');
const THEOREMS = config.theorem_names;
const PERMITTED_AXIOMS = config.assumption_policy.permitted_axioms;
const EXPECTED_FORMAT = checkersPin.export_format.version;
const CONTROL_AXIOM = 'Protected.external_zero';
const OLD_APPROVAL = {
  challenge_id: 'noble-issue-62-protected-bound/v1',
  config_sha256: '01611d9fd87eb8bd63f5d4ec38463b27270bb6ac6358c21c50fdb94a4a09258c',
  semantic_closure_sha256: '7cc25479566cb12a3192812f4cd789da4e9f6ed4d8030b854fb9b228fb8ae605',
  challenge_source_sha256: {
    'Challenge.lean': 'ec19efc7538e20381bc4db5d473a5ca94d5b1982a3226e2ac7277dbd0c0c1ef3',
    'Protected/Semantics.lean': '5641e7e90be1205f5b3101a07ce09931c1ee27f1a67e8c9ed7d9a8bb0866c45f',
  },
};
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
if (fileSha256(configPath) !== OLD_APPROVAL.config_sha256
    || Object.entries(OLD_APPROVAL.challenge_source_sha256).some(([relative, hash]) =>
      fileSha256(path.join(PROJECTS.challenge, relative)) !== hash)) {
  throw new Error('original challenge source or frozen approval identity changed');
}
if (amendedConfig.challenge_id !== 'noble-issue-62-protected-bound/v2-64'
    || amendedConfig.challenge_id === config.challenge_id
    || amendedConfig.model?.name !== config.model.name
    || amendedConfig.model?.version !== '2'
    || amendedConfig.model?.semantic_root !== config.model.semantic_root
    || amendedConfig.model?.predicate !== config.model.predicate
    || amendedConfig.model?.parameters?.threshold !== 64
    || JSON.stringify(amendedConfig.theorem_names) !== JSON.stringify(THEOREMS)
    || JSON.stringify(amendedConfig.assumption_policy) !== JSON.stringify(config.assumption_policy)
    || amendedConfig.challenge_module !== 'Challenge' || amendedConfig.solution_module !== 'Solution'
    || JSON.stringify(amendedConfig.external_contracts) !== JSON.stringify(config.external_contracts)) {
  throw new Error('unsupported amended challenge contract or changed axiom policy');
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
    argv: argvRun, cwd,
    exit_code: result.status,
    signal: result.signal,
    process_error: result.error ? { code: result.error.code ?? null, message: result.error.message } : null,
    elapsed_ms: Number((process.hrtime.bigint() - started) / 1_000_000n),
    stdout: result.stdout ?? '',
    stderr: result.stderr ?? '',
    timed_out: result.error?.code === 'ETIMEDOUT',
  };
}

/** Build one isolated fixture project and export its exact theorem roots. */
function buildAndExport(id, project, library, declarations,
  { allowForeignAxioms = false, expectKernelRejection = false } = {}) {
  const build = run([lake, 'build'], project);
  if (build.exit_code !== 0 || build.signal || build.process_error) {
    const detail = `${build.stdout}\n${build.stderr}`.trim();
    const className = build.timed_out ? 'timeout'
      : build.signal === 'SIGKILL' || /\b(?:out of memory|cannot allocate|ENOMEM)\b/iu.test(detail)
        ? 'resource-exhausted'
        : expectKernelRejection && /type mismatch/iu.test(detail)
          ? 'kernel-rejection' : 'build-failure';
    return { build, failure: { class: className, detail } };
  }
  const exportPath = path.join(work, `${id}.ndjson`);
  const descriptor = fs.openSync(exportPath, 'w');
  const started = process.hrtime.bigint();
  const exportArgv = [lake, 'env', leanexport, library, '--', ...declarations];
  const exported = spawnSync(lake, ['env', leanexport, library, '--', ...declarations], {
    cwd: project, env: childEnv, encoding: 'utf8', stdio: ['ignore', descriptor, 'pipe'],
    maxBuffer: 64 * 1024 * 1024, timeout: 1_800_000, killSignal: 'SIGKILL',
  });
  fs.closeSync(descriptor);
  const stage = {
    argv: exportArgv,
    cwd: project,
    exit_code: exported.status,
    signal: exported.signal,
    process_error: exported.error ? { code: exported.error.code ?? null, message: exported.error.message } : null,
    elapsed_ms: Number((process.hrtime.bigint() - started) / 1_000_000n),
    timed_out: exported.error?.code === 'ETIMEDOUT',
    panic: /PANIC/u.test(exported.stderr ?? ''),
    stderr: exported.stderr ?? '',
    stdout_file: exportPath,
    stdout_sha256: fileSha256(exportPath),
    stdout_size_bytes: fs.statSync(exportPath).size,
  };
  if (stage.exit_code !== 0 || stage.signal || stage.process_error || stage.panic) {
    const className = stage.timed_out ? 'timeout'
      : stage.signal === 'SIGKILL' || /\b(?:out of memory|cannot allocate|ENOMEM)\b/iu.test(stage.stderr)
        ? 'resource-exhausted' : 'export-failure';
    return { build, stage, exportPath, failure: { class: className, detail: stage.stderr } };
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

/** The official kernel skips unsafe and partial declarations; do not count an incomplete replay. */
function skippedByKernel(parsed) {
  const skipped = [];
  for (const [name, decl] of parsed.decls) {
    const record = decl.kind === 'inductive'
      ? [...(decl.group.record.types ?? []), ...(decl.group.record.ctors ?? []),
        ...(decl.group.record.recs ?? [])].find((member) => parsed.names.get(member.name) === name)
      : decl.record;
    if (decl.kind === 'def' && record.safety !== 'safe'
      || decl.kind !== 'def' && record.isUnsafe === true) skipped.push(name);
  }
  return skipped.sort();
}

/** One physical NDJSON is passed to both separately pinned kernels. */
function replayExport(candidate, id) {
  if (candidate.failure || !candidate.parsed || !candidate.record) {
    return { classification: candidate.failure?.class ?? 'export-failure', runs: [] };
  }
  const skipped = skippedByKernel(candidate.parsed);
  const foreign = foreignAxioms(candidate.inspection).filter((name) => !PERMITTED_AXIOMS.includes(name));
  // `LeanChecker.checkExport` removes these three generated quotient declarations from
  // the kernel input and checks them against the regenerated kernel values afterward.
  const postcheckedQuotients = ['Quot.mk', 'Quot.lift', 'Quot.ind']
    .filter((name) => candidate.parsed.decls.has(name));
  const replayable = candidate.parsed.decls.size - skipped.length - postcheckedQuotients.length;
  if (candidate.parsed.format !== EXPECTED_FORMAT || skipped.length || foreign.length || replayable <= 0) {
    return {
      classification: candidate.parsed.format !== EXPECTED_FORMAT ? 'unsupported-format'
        : skipped.length ? 'unsupported' : foreign.length ? 'forbidden-axiom' : 'empty-check',
      skipped_declarations: skipped, foreign_axioms: foreign, eligible_declarations: replayable,
      original_parsed_count: candidate.parsed.decls.size,
      quotient_exclusions: postcheckedQuotients, runs: [],
    };
  }
  const nanodaConfigPath = path.join(work, `${id}.nanoda.json`);
  fs.writeFileSync(nanodaConfigPath, `${JSON.stringify({
    use_stdin: false,
    export_file_path: candidate.exportPath,
    permitted_axioms: PERMITTED_AXIOMS,
    unpermitted_axiom_hard_error: true,
    num_threads: 1,
    nat_extension: true,
    string_extension: true,
    print_axioms: false,
    print_success_message: true,
  }, null, 2)}\n`);
  const commands = [
    { checker: 'nanoda', argv: [nanoda, nanodaConfigPath] },
    { checker: 'lean-official', argv: [leanchecker, '--from-export', candidate.exportPath] },
  ];
  const runs = commands.map(({ checker, argv: command }) => {
    const result = run(command, work);
    const text = `${result.stdout}\n${result.stderr}\n${result.process_error?.message ?? ''}`;
    let classification;
    let checkedDeclarations = null;
    if (result.timed_out) classification = 'timeout';
    else if (result.signal === 'SIGKILL' || result.signal === 'SIGXCPU' || result.exit_code === 137
      || /out of memory|cannot allocate(?: memory)?|allocation failed|memory allocation|failed to allocate|\bENOMEM\b/iu.test(text)) {
      classification = 'resource-exhausted';
    } else if (result.exit_code === 2
      || /\bunsupported(?: format| feature| extension)?\b|export format version is (?:greater|less) than/iu.test(text)) {
      classification = 'unsupported';
    } else if (/\bparse\b.{0,60}(?:error|fail|invalid|unexpected|eof)|\binvalid\s+(?:json|ndjson|token)\b|\bEOF while parsing\b/iu.test(text)) {
      classification = 'parse-failure';
    } else if (/\bunpermitted axiom\b/iu.test(text)) {
      classification = 'forbidden-axiom';
    } else if (result.exit_code === 0 && !result.signal && !result.process_error) {
      if (checker === 'nanoda') {
        const match = /^Checked (\d+) declarations with no errors\s*$/u.exec(result.stdout);
        checkedDeclarations = match ? Number(match[1]) : null;
      } else if (result.stdout === 'Lean default kernel accepts the solution\n'
        && result.stderr === '') {
        // LeanChecker.lean's checkExport replays every safe declaration from this exact
        // export and checks the quotient postconditions; it prints no numeric count.
        checkedDeclarations = replayable;
      }
      classification = checkedDeclarations === null ? 'checker-error'
        : checkedDeclarations > 0 ? 'accepted' : 'empty-check';
    } else {
      classification = /\b(?:mismatch|invalid|rejects?|unknown constant|assertion failed|kernel)\b/iu.test(text)
        ? 'kernel-rejection' : 'checker-error';
    }
    return { checker, ...result, classification, checked_declarations: checkedDeclarations,
      count_origin: checker === 'nanoda' ? 'checker-stdout' : 'parsed-export-nonquotient',
      official_count_source: checker === 'lean-official' ? 'parsed_export_nonquotient' : null,
      original_parsed_count: candidate.parsed.decls.size,
      quotient_exclusions: postcheckedQuotients,
      export_sha256: candidate.record.sha256,
      ...(checker === 'nanoda' ? {
        config_file: nanodaConfigPath, config_sha256: fileSha256(nanodaConfigPath),
      } : {}) };
  });
  return {
    classification: runs.every((item) => item.classification === 'accepted') ? 'accepted' : 'replay-refused',
    eligible_declarations: replayable,
    original_parsed_count: candidate.parsed.decls.size,
    quotient_exclusions: postcheckedQuotients,
    skipped_declarations: skipped,
    foreign_axioms: foreign,
    runs,
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

function git(args) {
  const command = spawnSync('git', args, { cwd: root, encoding: 'utf8', maxBuffer: 4 * 1024 * 1024 });
  if (command.status !== 0 || command.error) {
    throw new Error(`git ${args.join(' ')}: ${command.stderr ?? command.error?.message ?? 'failed'}`);
  }
  return command.stdout.trim();
}

function committedSource(file, revision = 'HEAD') {
  const relative = path.relative(root, file);
  if (relative.startsWith('..') || path.isAbsolute(relative)) {
    throw new Error(`not a source within repository: ${file}`);
  }
  const committed = spawnSync('git', ['show', `${revision}:${relative}`], {
    cwd: root, encoding: 'buffer', maxBuffer: 4 * 1024 * 1024,
  });
  if (committed.status !== 0 || committed.error || !committed.stdout.equals(fs.readFileSync(file))) {
    throw new Error(`uncommitted or changed source at ${revision}: ${relative}`);
  }
  return { path: relative, sha256: fileSha256(file) };
}

if (!freezeAmendment && !fs.existsSync(amendedApprovalPath)) {
  throw new Error('approval-missing: amended challenge needs a separately committed approval manifest');
}
const presentedApproval = freezeAmendment ? null
  : JSON.parse(fs.readFileSync(amendedApprovalPath, 'utf8'));
const proposalSourceCommit = freezeAmendment ? git(['rev-parse', 'HEAD'])
  : presentedApproval.proposal_source_commit;
if (!/^[0-9a-f]{40}$/u.test(proposalSourceCommit)) {
  throw new Error('amended approval source commit must be a full SHA-1 revision');
}
const amendedSourceFiles = [
  amendedConfigPath,
  ...['Challenge.lean', 'Protected/Semantics.lean',
    'lake-manifest.json', 'lakefile.toml', 'lean-toolchain']
    .map((relative) => path.join(PROJECTS['amended-challenge'], relative)),
];
const amendedSource = amendedSourceFiles.map((file) => {
  committedSource(file, 'HEAD');
  return committedSource(file, proposalSourceCommit);
});
const amendedChallenge = buildAndExport(
  'amended-challenge', PROJECTS['amended-challenge'], amendedConfig.challenge_module, THEOREMS);
if (amendedChallenge.failure) {
  throw new Error(`amended challenge ${amendedChallenge.failure.class}: ${amendedChallenge.failure.detail}`);
}
const amendedStatements = rootStatements(amendedChallenge.parsed);
const amendedSemantic = semanticClosure(amendedChallenge.parsed, THEOREMS);
if (amendedSemantic.missing.length) {
  throw new Error(`amended challenge semantic closure incomplete: ${amendedSemantic.missing.join(', ')}`);
}
const amendedChallengeReplay = replayExport(amendedChallenge, 'amended-challenge');
const proposal = {
  schema: 'noble-independent-replay-protected-approval/v1',
  challenge_id: amendedConfig.challenge_id,
  proposal_source_commit: proposalSourceCommit,
  model: amendedConfig.model,
  source_sha256: Object.fromEntries(amendedSource.map((entry) => [entry.path, entry.sha256])),
  root_statement_sha256: amendedStatements,
  semantic_closure_sha256: amendedSemantic.digest.sha256,
  permitted_axioms: PERMITTED_AXIOMS,
  toolchain: checkersPin.toolchain.output,
  export_format: EXPECTED_FORMAT,
};

function retainRaw(files) {
  fs.mkdirSync(path.dirname(output), { recursive: true });
  fs.mkdirSync(rawOutput, { recursive: false });
  return files.filter((file) => file && fs.existsSync(file)).map((file) => {
    const retained = path.join(rawOutput, path.basename(file));
    if (fs.existsSync(retained)) throw new Error(`duplicate raw artifact: ${retained}`);
    fs.copyFileSync(file, retained, fs.constants.COPYFILE_EXCL);
    return { path: path.relative(path.dirname(output), retained),
      sha256: fileSha256(retained), size_bytes: fs.statSync(retained).size };
  });
}

if (freezeAmendment) {
  const holds = amendedChallengeReplay.classification === 'accepted'
    && THEOREMS.every((name) => amendedChallenge.parsed.decls.has(name))
    && Object.values(amendedStatements).every((hash) => typeof hash === 'string')
    && foreignAxioms(amendedChallenge.inspection).every((name) => PERMITTED_AXIOMS.includes(name));
  const raw_artifacts = retainRaw([
    amendedChallenge.exportPath,
    ...amendedChallengeReplay.runs.map((entry) => entry.config_file),
  ]);
  const frozen = {
    schema: 'noble-independent-replay-protected-amendment-proposal/v1',
    source_commit: proposal.proposal_source_commit,
    holds,
    challenge: { build: amendedChallenge.build, export: amendedChallenge.stage,
      export_record: amendedChallenge.record, root_statement_sha256: amendedStatements,
      semantic_closure: amendedSemantic, proof_closure: proofClosure(amendedChallenge.parsed, THEOREMS),
      replay: amendedChallengeReplay },
    approval_proposal: holds ? proposal : null,
    raw_artifacts,
    status: 'observed challenge only; not owner approval; amended solution has not been built',
  };
  fs.mkdirSync(path.dirname(output), { recursive: true });
  fs.writeFileSync(output, `${JSON.stringify(frozen, null, 2)}\n`);
  console.log(JSON.stringify({ holds, proposal: holds ? proposal : null }));
  process.exit(holds ? 0 : 1);
}

// Approval is a distinct tracked commit, not a value inferred from a solution export.
const committedApproval = committedSource(amendedApprovalPath);
const approval = presentedApproval;
const approvalCommit = git(['log', '-1', '--format=%H', '--', committedApproval.path]);
if (approval.schema !== proposal.schema
    || approval.challenge_id !== amendedConfig.challenge_id
    || approval.challenge_id === OLD_APPROVAL.challenge_id
    || approval.proposal_source_commit === approvalCommit
    || git(['merge-base', proposal.proposal_source_commit, approvalCommit]) !== proposal.proposal_source_commit
    || JSON.stringify(approval) !== JSON.stringify(proposal)) {
  throw new Error('approval-mismatch: independent committed owner approval does not match the checked challenge');
}

const challenge = buildAndExport('challenge', PROJECTS.challenge, config.challenge_module, THEOREMS);
if (challenge.failure) throw new Error(`challenge ${challenge.failure.class}: ${challenge.failure.detail}`);
const challengeStatements = rootStatements(challenge.parsed);
const challengeSemantic = semanticClosure(challenge.parsed, THEOREMS);
if (challengeSemantic.missing.length > 0) {
  throw new Error(`challenge semantic closure is incomplete: ${challengeSemantic.missing.join(', ')}`);
}
if (challengeSemantic.digest.sha256 !== OLD_APPROVAL.semantic_closure_sha256) {
  throw new Error('original challenge semantic closure differs from its immutable approval');
}

const solutionBuilds = new Map();
for (const id of ['real', 'alternate', 'weakened-predicate', 'changed-semantics', 'wrong-statement',
  'forbidden-assumption', 'kernel-rejection']) {
  solutionBuilds.set(id, buildAndExport(id, PROJECTS[id], config.solution_module, THEOREMS, {
    allowForeignAxioms: id === 'forbidden-assumption',
    expectKernelRejection: id === 'kernel-rejection',
  }));
}
const amendedSolution = buildAndExport(
  'amended-solution', PROJECTS['amended-solution'], amendedConfig.solution_module, THEOREMS);

/** Compare to a separately selected, approval-bound challenge; never to the candidate itself. */
function compareSolution(id, candidate, approved = {
  id: OLD_APPROVAL.challenge_id, statements: challengeStatements, semantic: challengeSemantic,
}) {
  const entry = {
    id,
    project: PROJECTS[id] ? path.relative(root, PROJECTS[id])
      : candidate.build?.cwd ? path.relative(root, candidate.build.cwd) : candidate.build?.project ?? null,
    approval_id: approved.id,
    approved_semantic_closure_sha256: approved.semantic.digest.sha256,
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
  const statementMismatch = THEOREMS.find((name) => statements[name] !== approved.statements[name]);
  if (statementMismatch) {
    entry.verdict = { class: 'statement-mismatch', detail: statementMismatch, path: [statementMismatch] };
    return entry;
  }

  const semantic = semanticClosure(candidate.parsed, THEOREMS);
  entry.semantic_closure = semantic;
  const expectedNames = new Set(approved.semantic.names);
  const actualNames = new Set(semantic.names);
  const missing = [...new Set([
    ...semantic.missing,
    ...approved.semantic.names.filter((name) => !actualNames.has(name)),
  ])].sort();
  if (missing.length > 0) {
    entry.verdict = {
      class: 'const-missing-in-solution', detail: missing[0], missing,
      dependency_paths: Object.fromEntries(missing.map((name) => [name, semantic.dependency_paths[name] ?? null])),
    };
    return entry;
  }
  const changed = approved.semantic.names.filter((name) =>
    semantic.declaration_digests[name] !== approved.semantic.declaration_digests[name]);
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
const amendedReference = {
  id: approval.challenge_id, statements: amendedStatements, semantic: amendedSemantic,
};
const amendmentRuns = [
  compareSolution('amended-solution', amendedSolution, amendedReference),
  compareSolution('old-solution-under-new-approval', solutionBuilds.get('real'), amendedReference),
  compareSolution('amended-solution-under-old-approval', amendedSolution),
  compareSolution('old-challenge-under-new-approval', challenge, amendedReference),
  compareSolution('amended-challenge-under-old-approval', amendedChallenge),
];
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
const amendedById = (id) => amendmentRuns.find((entry) => entry.id === id);
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
  ['amended-challenge', amendedChallenge.parsed],
  ...(amendedSolution.parsed ? [['amended-solution', amendedSolution.parsed]] : []),
  ...[...solutionBuilds].filter(([, candidate]) => candidate.parsed).map(([id, candidate]) => [id, candidate.parsed]),
].map(([id, parsed]) => [id, proofClosure(parsed, THEOREMS)]));
const allExported = [challenge, amendedChallenge, amendedSolution, ...solutionBuilds.values()];
const legitimateExports = allExported.filter((entry) =>
  entry !== solutionBuilds.get('forbidden-assumption')
    && entry !== solutionBuilds.get('kernel-rejection'));
const replay = Object.fromEntries([
  ['challenge', challenge],
  ['real', real],
  ['alternate', solutionBuilds.get('alternate')],
  ['amended-challenge', amendedChallenge],
  ['amended-solution', amendedSolution],
].map(([id, candidate]) => [id, id === 'amended-challenge'
  ? amendedChallengeReplay : replayExport(candidate, id)]));
const successfulReplay = (id) => replay[id]?.classification === 'accepted'
  && replay[id].runs.length === 2
  && replay[id].runs.every((entry) => entry.classification === 'accepted'
    && entry.checked_declarations > 0 && entry.export_sha256
    === (id === 'challenge' ? challenge : id === 'amended-challenge' ? amendedChallenge
      : id === 'amended-solution' ? amendedSolution : solutionBuilds.get(id)).record.sha256);
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
  real_and_alternate_replay_on_their_actual_exports:
    ['challenge', 'real', 'alternate'].every(successfulReplay),
  amended_challenge_and_new_solution_replay_on_their_actual_exports:
    ['amended-challenge', 'amended-solution'].every(successfulReplay),
  amended_challenge_bound_to_separate_approval:
    approval.challenge_id !== OLD_APPROVAL.challenge_id
      && approval.semantic_closure_sha256 === amendedSemantic.digest.sha256
      && amendedSemantic.digest.sha256 !== OLD_APPROVAL.semantic_closure_sha256
      && committedApproval.sha256 === fileSha256(amendedApprovalPath)
      && amendedById('amended-solution')?.verdict.class === 'accepted'
      && amendedById('amended-solution')?.semantic_closure?.digest.sha256 === approval.semantic_closure_sha256
      && THEOREMS.every((name) =>
        amendedById('amended-solution')?.root_statement_sha256?.[name] === approval.root_statement_sha256[name]),
  old_approval_refuses_amended_semantics:
    ['amended-solution-under-old-approval', 'amended-challenge-under-old-approval']
      .every((id) => amendedById(id)?.verdict.class === 'definition-mismatch'
        && amendedById(id)?.verdict.detail === 'Protected.threshold'),
  amended_approval_refuses_old_semantics:
    ['old-solution-under-new-approval', 'old-challenge-under-new-approval']
      .every((id) => amendedById(id)?.verdict.class === 'definition-mismatch'
        && amendedById(id)?.verdict.detail === 'Protected.threshold'),
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
    challengeSemantic.digest.sha256 !== proofClosures.real?.digest?.sha256
      && amendedSemantic.digest.sha256 !== proofClosures['amended-solution']?.digest?.sha256
      && proofClosures['amended-solution']?.digest?.sha256 !== proofClosures.real?.digest?.sha256,
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
const raw_artifacts = retainRaw([
  ...allExported.map((candidate) => candidate.exportPath),
  missingDefinition.file,
  ...Object.values(replay).flatMap((result) => result.runs.map((entry) => entry.config_file)),
]);
const receipt = {
  schema: 'noble-independent-replay-protected/v3',
  measured_on: new Date().toISOString().slice(0, 10),
  host,
  source_commit: source.stdout.trim(),
  toolchain,
  assertion: 'independently selected old and amended challenges bind separate approval identities and exact semantic closures; real, alternate and amended proofs are kernel-replayed on their retained exports by pinned Nanoda and Lean; the two approval identities reject each other’s semantics; all eight historical control verdicts remain enforced',
  holds,
  checks,
  comparison: {
    method: 'exported-definition-closure/v1',
    comparator: 'lake comparator was not run: bwrap is absent from the execution PATH; this lane uses the exported-closure fallback instead',
    bwrap_on_execution_path: bwrapAvailable,
    execution_path: childEnv.PATH,
    definition_holes: config.assumption_policy.definition_holes,
    model: config.model,
    amended_model: amendedConfig.model,
    external_contracts: config.external_contracts,
  },
  harness: {
    protected: { path: 'verification/independent-replay/protected.mjs', sha256: fileSha256(fileURLToPath(import.meta.url)) },
    challenge_config: { path: 'verification/independent-replay/protected/challenge.json', sha256: fileSha256(configPath) },
    amended_challenge_config: { path: 'verification/independent-replay/protected/amendment/challenge.json',
      sha256: fileSha256(amendedConfigPath) },
    amended_approval: committedApproval,
    checker_manifest: { path: 'verification/independent-replay/checkers.json', sha256: fileSha256(checkersPath) },
    sources: sourceEvidence,
  },
  tools: {
    lean: { binary_sha256: pins.lean },
    lake: { binary_sha256: pins.lake },
    leanexport: { binary_sha256: pins.leanexport },
    nanoda: { binary_sha256: nanodaPin },
    leanchecker: { binary_sha256: LEANCHECKER_SHA256, interface: '--from-export <exact retained NDJSON>' },
    release: checkersPin.toolchain.output,
    derivation: checkersPin.toolchain.derivation,
  },
  configuration: config,
  approvals: {
    old: OLD_APPROVAL,
    amended: { ...approval, approval_manifest_sha256: committedApproval.sha256, approval_commit: approvalCommit },
  },
  challenge: {
    build: challenge.build,
    export: challenge.stage,
    export_record: challenge.record,
    root_statement_sha256: challengeStatements,
    semantic_closure: challengeSemantic,
  },
  amended_challenge: {
    build: amendedChallenge.build,
    export: amendedChallenge.stage,
    export_record: amendedChallenge.record,
    root_statement_sha256: amendedStatements,
    semantic_closure: amendedSemantic,
  },
  replay,
  proof_closures: proofClosures,
  missing_definition_control: missingDefinition.record,
  runs,
  amendment_runs: amendmentRuns,
  raw_artifacts,
  non_claims: [
    'This exported-closure fallback is not lake comparator and does not inherit all of its sandbox or comparison guarantees.',
    'The official Lean checker replays the retained export through Lean’s own C++ kernel; it is not an independent checker implementation. Nanoda is separately implemented but neither checker proves the correctness of the other.',
    'The approved amendment is source-and-export-bound in a separate committed manifest; a solution may not mint its own approval. The issuer of the approval remains an owner decision.',
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
  amended_verdicts: Object.fromEntries(amendmentRuns.map((entry) =>
    [entry.id, `${entry.verdict.class}:${entry.verdict.detail ?? '-'}`])),
}));
process.exitCode = holds ? 0 : 1;
