// Production-side mutation lane for the ownership corpus (issue #59).
//
// The corpus lane's own six mutations corrupt a test-local shadow checker. That
// shows the corpus is sensitive to a changed rule; it does not show that the lane
// fails when the *production* rule breaks. This runner applies each declared
// mutant to a scratch copy of a clean committed tree, builds and tests that copy,
// and requires the ownership lane to fail. Everything runs outside the repository:
// the receipt is the only artefact written back.
//
// Mutants that survive are recorded honestly. A surviving mutant means the corpus
// has no executable production control for that rule, and the receipt says so
// instead of claiming coverage.
//
// Usage:
//   node verification/ownership/production-mutants.mjs \
//     --output FILE --scratch DIR --target DIR [--root .] \
//     [--cargo cargo] [--rustc rustc]
//
// Output, scratch and target paths are required absolute paths outside the
// repository. Choose Cargo and Rustc from a toolchain matching the repository's
// lockfile and build settings.
import { spawnSync } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';

const args = process.argv.slice(2);
const option = (name, fallback) => {
  const index = args.indexOf(`--${name}`);
  return index >= 0 ? args[index + 1] : fallback;
};
const root = path.resolve(option('root', '.'));
const cargo = option('cargo', 'cargo');
const rustc = option('rustc', 'rustc');
const outputArg = option('output');
const scratchArg = option('scratch');
const targetArg = option('target');
const externalPath = (name, value) => {
  if (typeof value !== 'string' || value.length === 0 || !path.isAbsolute(value)) {
    throw new Error(`--${name} must be an absolute path outside the repository`);
  }
  const resolved = path.resolve(value);
  const relative = path.relative(root, resolved);
  if (relative === ''
    || (relative !== '..' && !relative.startsWith(`..${path.sep}`) && !path.isAbsolute(relative))) {
    throw new Error(`--${name} must be outside the repository`);
  }
  return resolved;
};
const output = externalPath('output', outputArg);
const scratchBase = externalPath('scratch', scratchArg);
const target = externalPath('target', targetArg);

const lane = 'cargo test -p noble-kernel --test acceptance ownership --offline --locked -- --nocapture';

// Each mutant is one exact byte replacement in a tracked production file. The
// `from` text must occur exactly once, so a refactor that moves the rule is
// noticed instead of silently mutating nothing.
const MUTANTS = [
  {
    name: 'drop-unconstrained',
    file: 'crates/noble-kernel/src/acceptance/parts/instantiate.rs',
    from: `        Some(crate::contracts::Behavior::Dup)
        | Some(crate::contracts::Behavior::Drop)
        | Some(crate::contracts::Behavior::Quote) => Some(crate::words::Variable(1)),
        Some(crate::contracts::Behavior::Swap)`,
    to: `        Some(crate::contracts::Behavior::Dup)
        | Some(crate::contracts::Behavior::Quote) => Some(crate::words::Variable(1)),
        Some(crate::contracts::Behavior::Swap)
        | Some(crate::contracts::Behavior::Drop)`,
    expectation: 'killed',
    why: 'the data slot stops constraining `Drop`, so a must-use resource becomes droppable',
  },
  {
    name: 'dup-unconstrained',
    file: 'crates/noble-kernel/src/acceptance/parts/instantiate.rs',
    from: `        Some(crate::contracts::Behavior::Dup)
        | Some(crate::contracts::Behavior::Drop)
        | Some(crate::contracts::Behavior::Quote) => Some(crate::words::Variable(1)),
        Some(crate::contracts::Behavior::Swap)`,
    to: `        Some(crate::contracts::Behavior::Drop)
        | Some(crate::contracts::Behavior::Quote) => Some(crate::words::Variable(1)),
        Some(crate::contracts::Behavior::Swap)
        | Some(crate::contracts::Behavior::Dup)`,
    expectation: 'killed',
    why: 'the data slot stops constraining `Dup`, so a must-use resource becomes duplicable',
  },
  {
    name: 'quote-unconstrained',
    file: 'crates/noble-kernel/src/acceptance/parts/instantiate.rs',
    from: `        Some(crate::contracts::Behavior::Dup)
        | Some(crate::contracts::Behavior::Drop)
        | Some(crate::contracts::Behavior::Quote) => Some(crate::words::Variable(1)),
        Some(crate::contracts::Behavior::Swap)`,
    to: `        Some(crate::contracts::Behavior::Dup)
        | Some(crate::contracts::Behavior::Drop) => Some(crate::words::Variable(1)),
        Some(crate::contracts::Behavior::Swap)
        | Some(crate::contracts::Behavior::Quote) => None,`,
    expectation: 'survived',
    why: 'every `quote` case is model-only (the production quotation constructor produces a program value), so no executable control exists',
  },
  {
    name: 'swap-discards-operand',
    file: 'crates/noble-kernel/src/contracts/bootstrap.rs',
    from: `        alloc::vec![stack_var(0), value_var(1), value_var(2)],
        alloc::vec![stack_var(0), value_var(2), value_var(1)],`,
    to: `        alloc::vec![stack_var(0), value_var(1), value_var(2)],
        alloc::vec![stack_var(0), value_var(2)],`,
    expectation: 'killed',
    why: 'the swap scheme silently drops one operand instead of reordering two',
  },
  {
    name: 'swap-identity',
    file: 'crates/noble-kernel/src/contracts/bootstrap.rs',
    from: `        alloc::vec![stack_var(0), value_var(2), value_var(1)],`,
    to: `        alloc::vec![stack_var(0), value_var(1), value_var(2)],`,
    expectation: 'killed',
    why: 'the swap scheme stops reordering its operands',
  },
  {
    name: 'dup-duplicates-nothing',
    file: 'crates/noble-kernel/src/contracts/bootstrap.rs',
    from: `        alloc::vec![stack_var(0), value_var(1), value_var(1)],`,
    to: `        alloc::vec![stack_var(0), value_var(1)],`,
    expectation: 'killed',
    why: 'the dup scheme stops producing the second copy of its operand',
  },
];

function run(argv, options = {}) {
  const result = spawnSync(argv[0], argv.slice(1), {
    encoding: 'utf8',
    maxBuffer: 256 * 1024 * 1024,
    timeout: 3_600_000,
    ...options,
  });
  if (result.error) throw new Error(`${argv.join(' ')}: ${result.error.message}`);
  return { status: result.status, stdout: result.stdout ?? '', stderr: result.stderr ?? '' };
}

const must = (argv, options) => {
  const result = run(argv, options);
  if (result.status !== 0) {
    throw new Error(`${argv.join(' ')} failed (${result.status}): ${result.stderr.slice(-2000)}`);
  }
  return result.stdout;
};

const sha256 = file => crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');

const toolchain = {
  cargo: (() => {
    const result = run([cargo, '-V'], {
      cwd: root,
      env: {
        ...process.env,
        NOBLE_ROOT: root,
        NOBLE_TARGET: path.join(target, 'baseline'),
        CARGO_TARGET_DIR: path.join(target, 'baseline'),
      },
    });
    return result.status === 0 ? result.stdout.trim() : null;
  })(),
  rustc: (() => {
    const result = run([rustc, '-vV']);
    return result.status === 0 ? result.stdout.trim() : null;
  })(),
  host: must(['uname', '-srm']).trim().split('\n')[0],
};

// A clean committed tree: `git archive HEAD` is the mutant material, and the
// receipt must describe what that commit contains.
const head = must(['git', '-C', root, 'rev-parse', 'HEAD']).trim();
const tree = must(['git', '-C', root, 'rev-parse', 'HEAD^{tree}']).trim();
const dirt = must(['git', '-C', root, 'status', '--porcelain'])
  .split('\n')
  .filter(line => line.trim() !== '');
if (dirt.length > 0) {
  throw new Error(`production-mutants requires a clean tracked tree: ${dirt.join('; ')}`);
}
fs.mkdirSync(scratchBase, { recursive: true });
const scratch = fs.mkdtempSync(path.join(scratchBase, 'run-'));
const archive = path.join(scratch, 'head.tar');
must(['git', '-C', root, 'archive', '--format=tar', `--output=${archive}`, 'HEAD']);

const extract = (destination) => {
  fs.mkdirSync(destination, { recursive: true });
  must(['tar', '-xf', archive, '-C', destination]);
  return destination;
};

const environmentOf = copy => ({
  ...process.env,
  NOBLE_ROOT: copy,
  NOBLE_TARGET: path.join(target, path.basename(copy)),
  CARGO_TARGET_DIR: path.join(target, path.basename(copy)),
});

function corpusHasMinimisedCase(copy, fixture) {
  const parsed = /^minimised-[^\s]+ ops=(\S+) in=(\S+) divergence=/u.exec(fixture);
  if (!parsed) return false;
  const caseFile = path.join(copy, 'verification/ownership/cases.txt');
  return fs.readFileSync(caseFile, 'utf8').split('\n').some(row => {
    if (!row || row.startsWith('#')) return false;
    const fields = new Set(row.split(/\s+/u));
    return fields.has(`ops=${parsed[1]}`)
      && fields.has(`in=${parsed[2]}`)
      && (fields.has('decision=accepted') || fields.has('decision=rejected'));
  });
}

// The lane's own outcome, so a compile error is never mistaken for the corpus
// catching a semantic mutant.
function laneOutcome(copy) {
  const result = run([cargo, 'test', '-p', 'noble-kernel', '--test', 'acceptance', 'ownership',
    '--offline', '--locked', '--', '--nocapture'], { cwd: copy, env: environmentOf(copy) });
  const text = `${result.stdout}\n${result.stderr}`;
  const compileError = /could not compile|error\[E\d+\]/u.test(text);
  const testFailed = /test result: FAILED/u.test(text);
  const failedTest = /^failures:$/mu.test(text);
  const firstFailure = text
    .split('\n')
    .find(line => line.startsWith('Error: ') || line.includes('panicked at') || line.startsWith('error: test failed')) ?? null;
  const summary = text.split('\n').find(line => line.startsWith('ownership-differential ')) ?? null;
  const mutations = text.split('\n').find(line => line.startsWith('ownership-mutations ')) ?? null;
  const minimisedPath = path.join(copy, 'verification/ownership/minimised.txt');
  const minimisedFixture = fs.existsSync(minimisedPath)
    ? fs.readFileSync(minimisedPath, 'utf8').split('\n').find(line => line.startsWith('minimised-')) ?? null
    : null;
  return {
    exit: result.status,
    outcome: compileError ? 'compile-error' : testFailed && failedTest ? 'test-failed' : result.status === 0 ? 'passed' : 'other-failure',
    first_failure: firstFailure,
    summary,
    mutations,
    minimised_fixture: minimisedFixture,
    tail: text.split('\n').slice(-40).join('\n'),
  };
}

const baseline = laneOutcome(extract(path.join(scratch, 'baseline')));
if (baseline.outcome !== 'passed') {
  throw new Error(`the unmutated copy does not pass the ownership lane (${baseline.outcome}); fix the lane before measuring mutants\n${baseline.tail}`);
}

const mutants = [];
for (const mutant of MUTANTS) {
  const copy = extract(path.join(scratch, mutant.name));
  const target_ = path.join(copy, mutant.file);
  const before = fs.readFileSync(target_, 'utf8');
  const occurrences = before.split(mutant.from).length - 1;
  if (occurrences !== 1) {
    throw new Error(`mutant ${mutant.name}: the declared text occurs ${occurrences} times in ${mutant.file}`);
  }
  fs.writeFileSync(target_, before.replace(mutant.from, mutant.to));
  const laneResult = laneOutcome(copy);
  const killed = laneResult.outcome === 'test-failed';
  if (killed && laneResult.minimised_fixture === null) {
    throw new Error(`mutant ${mutant.name} failed the lane without writing a minimised fixture\n${laneResult.tail}`);
  }
  if (killed && !corpusHasMinimisedCase(copy, laneResult.minimised_fixture)) {
    throw new Error(`mutant ${mutant.name} minimized to a program/operand pair not retained as a production case: ${laneResult.minimised_fixture}`);
  }
  const verdict = killed
    ? 'killed'
    : laneResult.outcome === 'passed'
      ? 'survived'
      : laneResult.outcome;
  if (verdict !== mutant.expectation) {
    throw new Error(
      `mutant ${mutant.name} produced ${verdict}, which contradicts its declared expectation ${mutant.expectation}`,
    );
  }
  mutants.push({
    name: mutant.name,
    file: mutant.file,
    mutation: mutant.why,
    expectation: mutant.expectation,
    verdict,
    exit: laneResult.exit,
    first_failure: laneResult.first_failure,
    lane_summary: laneResult.summary,
    lane_mutations: laneResult.mutations,
    minimised_fixture: laneResult.minimised_fixture,
    file_sha256_before: crypto.createHash('sha256').update(before).digest('hex'),
    file_sha256_after: sha256(target_),
  });
}

const killed = mutants.filter(mutant => mutant.verdict === 'killed');
const survivors = mutants.filter(mutant => mutant.verdict === 'survived');
if (killed.length !== 5 || survivors.length !== 1 || survivors[0]?.name !== 'quote-unconstrained') {
  throw new Error(
    `production mutation coverage changed: killed=${killed.length} survivors=${survivors.map(mutant => mutant.name).join(',')}`,
  );
}

const receipt = {
  schema: 'noble-ownership-production-mutants/v1',
  assertion: 'each declared mutant of the production ownership rules makes the ownership corpus lane fail, except the mutants listed as survivors, which the corpus has no executable control for; the unmutated copy passes in the same runner. Finite evidence on one host and target, not a refinement proof.',
  source_commit: head,
  source_tree: tree,
  clean_source: true,
  toolchain,
  cargo,
  rustc,
  host: toolchain.host,
  lane,
  cargo_target_base: target,
  materialisation: 'git archive HEAD unpacked into a scratch directory per mutant; the mutant is one exact byte replacement in a tracked production file',
  baseline: {
    outcome: baseline.outcome,
    summary: baseline.summary,
    mutations: baseline.mutations,
  },
  mutants,
  totals: { mutants: mutants.length, killed: killed.length, survived: survivors.length },
  survivors,
  survivor_accounting: survivors.map(mutant => ({
    name: mutant.name,
    why: mutant.expectation === 'survived'
      ? MUTANTS.find(entry => entry.name === mutant.name).why
      : 'unexpected survivor',
  })),
  claims: [
    'The ownership lane fails on a real production rule change, not only on a test-local shadow checker change.',
    '`quote` has no executable production control: every quote case is model-only because the production quotation constructor produces a program value instead of retaining the operand. That mutant therefore survives and is recorded, not hidden.',
    'The mutants are applied to scratch copies of one commit; no production source file is changed by this lane.',
  ],
};

fs.mkdirSync(path.dirname(output), { recursive: true });
fs.writeFileSync(output, `${JSON.stringify(receipt, null, 2)}\n`);
console.log(JSON.stringify({ outcome: 'recorded', output, totals: receipt.totals, survivors: survivors.map(mutant => mutant.name) }));
