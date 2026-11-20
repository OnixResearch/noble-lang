#!/usr/bin/env bun
// ACORR-CASE-01..12: the executed positive and negative controls for the
// opt-in proof-bound artifact admission envelope (PAA-1, ACORR-ENV-01/
// CACHE-01/CTRL-01/TB-01).
//
// Every control runs the frozen CLI supplied here:
//   bun verification/acorr/gate.mjs CLI NEW_DIR [--barrier-cli PATH] [--allow-dirty]
// CLI is copied to NEW_DIR/bin/noble-executed and chmod 0500 before anything
// runs; its SHA-256, the source commit/tree and the clean worktree are
// retained. Negative candidates and altered pin documents are produced by
// this script's own preparation (WAT print/parse edits, byte-edit copies and
// JSON text edits), never through the admission code path, and every prepared
// input is hashed. The gate writes NEW_DIR/receipt.json and exits non-zero
// unless all twelve cases executed and every assertion held.
//
// --barrier-cli names a CLI built with the `paa1-test-barrier` feature. It is
// the only way ACORR-CASE-09 can replace files between the byte comparison and
// instantiation and observe that the executed bytes are still the compared
// buffer. Without it CASE-09 is recorded as not-executed, and the gate cannot
// report `passed`.
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { chmodSync, copyFileSync, existsSync, mkdirSync, readFileSync, readdirSync,
  realpathSync, statSync, writeFileSync } from 'node:fs';
import { homedir, userInfo } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { callSource, wrapperSource, watToWasm } from './prepare.mjs';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const policy = JSON.parse(readFileSync(join(root, 'policy/tool-selection.json')));
const node = join(policy.tool_paths.node.output, 'bin/node');
const wasmTools = join(policy.tool_paths.wasm_tools.output, 'bin/wasm-tools');
const git = realpathSync('/run/current-system/sw/bin/git');
const { homedir: home, username } = userInfo();
const fail = message => { throw Error(message); };
const requireThat = (condition, message) => { if (!condition) fail(message); };
const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const fileHash = path => digest(readFileSync(path));
const read = path => readFileSync(path);
const json = text => JSON.parse(text);
const wrap = value => BigInt.asIntN(64, value);
const I64_MAX = 9223372036854775807n;
const I64_MIN = -9223372036854775808n;

const args = process.argv.slice(2);
const option = name => { const index = args.indexOf(name); return index >= 0 ? args[index + 1] : undefined; };
const supplied = args[0];
const outputArg = args[1];
requireThat(supplied && outputArg,
  'usage: gate.mjs CLI NEW_DIR [--barrier-cli PATH] [--allow-dirty]');
const allowDirty = args.includes('--allow-dirty');
const barrierSupplied = option('--barrier-cli');
const out = resolve(outputArg);
mkdirSync(out);
mkdirSync(join(out, 'bin'));
mkdirSync(join(out, 'inputs'));
mkdirSync(join(out, 'candidates'));
mkdirSync(join(out, 'reports'));

// Reviewed Route B sandbox inputs (verification/mc2/gate.mjs `reviewed`).
const consumer = {
  NOBLE_LEAN: join(policy.tool_paths.lean.output, 'bin/lean'),
  NOBLE_BWRAP: '/nix/store/lqndphylsxqwbwm804n473pb4sqb98sh-bubblewrap-0.11.2/bin/bwrap',
  NOBLE_PRLIMIT: '/nix/store/mqvbf0flamqaq9c3ihb496aahag897n0-util-linux-2.42.3-bin/bin/prlimit',
  NOBLE_SYSTEMD_RUN: '/nix/store/baxgs06659w4i6ggsfr56srjrb52v1h8-systemd-261.2/bin/systemd-run',
  NOBLE_CONTRACT_LIBRARY: join(root, 'proofs/mc1'),
};
const childBase = Object.freeze({
  HOME: home, USER: username, LOGNAME: username, LANG: 'C.UTF-8', CI: '1',
  GIT_TERMINAL_PROMPT: '0', TMPDIR: join(out, 'tmp'),
  PATH: [dirname(node), dirname(git), '/run/current-system/sw/bin', '/usr/bin', '/bin'].join(':'),
});
mkdirSync(join(out, 'tmp'));
const environment = { ...childBase, ...consumer, XDG_RUNTIME_DIR: process.env.XDG_RUNTIME_DIR ?? '' };
requireThat(environment.XDG_RUNTIME_DIR, 'XDG_RUNTIME_DIR is required by the sandboxed Lean consumer');

const commands = [];
function run(executable, argv, options = {}) {
  const result = spawnSync(executable, argv, { encoding: 'utf8', maxBuffer: 128 * 1024 * 1024,
    timeout: 1800000, env: options.env ?? environment, cwd: options.cwd ?? root });
  commands.push({ executable, args: argv, exit: result.status, signal: result.signal });
  if (result.error || (result.status !== 0 && !options.allowFailure)) {
    fail(`${executable} ${argv.join(' ')} failed: ${result.error?.message ?? result.status}\n${result.stderr}\n${result.stdout}`);
  }
  return options.allowFailure ? result : result.stdout.trim();
}
function gitState() {
  return {
    head: run(git, ['rev-parse', 'HEAD']),
    tree: run(git, ['rev-parse', 'HEAD^{tree}']),
    status: run(git, ['status', '--porcelain=v1', '--untracked-files=all']),
  };
}
function freeze(source, name) {
  const executed = join(out, 'bin', name);
  copyFileSync(source, executed);
  chmodSync(executed, 0o500);
  return { supplied: resolve(source), executed, sha256: fileHash(executed) };
}
function writeInput(name, contents) {
  const path = join(out, 'inputs', name);
  writeFileSync(path, contents, { flag: 'wx' });
  return path;
}
function sourceText(name) {
  return read(join(root, name), 'utf8');
}

const initial = gitState();
requireThat(allowDirty || initial.status === '',
  `Noble source worktree is dirty:\n${initial.status}`);
const noble = freeze(supplied, 'noble-executed');
const barrier = barrierSupplied ? freeze(barrierSupplied, 'noble-barrier-executed') : null;

function nobleRun(argv, options = {}) {
  return run(noble.executed, argv, options);
}
function nobleBinary(executable, argv, options = {}) {
  return run(executable, argv, options);
}

// ---- preparation (independent of the admission code path) ------------------
const mc2 = 'verification/mc2/contracts';
const ownerLaw = 'verification/owner-law/laws/owner-law-v1.json';
const ownerLawV2 = 'verification/owner-law/laws/owner-law-v2.json';
const V1_SHA256 = '9a8eda12c74356ca0d312be840411d4445c6f377c16ee7b96a680581b2e91eac';
const V2_SHA256 = '51f2a3158155077dcb29c8f354d356d31df3282b3c76f60c2bc8534c7932ee80';
const Q_ONE = sourceText(`${mc2}/q-one.noble`);

const inputs = {};
const input = (name, contents) => {
  const path = writeInput(name, contents);
  inputs[path] = digest(contents);
  return path;
};
const instrument = path => { inputs[path] = fileHash(path); return path; };

function emit(sourcePath, mode, label) {
  const destination = join(out, 'candidates', `${label}-${mode}`);
  nobleRun(['run', sourcePath, '--opt', mode, '--emit', destination]);
  const module = join(destination, 'engine', mode === 'on' ? 'module-1-optimized.wasm' : 'module-1.wasm');
  requireThat(existsSync(module), `${label}/${mode}: no emitted module`);
  instrument(module);
  const wat = join(destination, 'engine', 'module-1.wat');
  instrument(wat);
  return { module, wat, directory: destination };
}
function pinsDocument(name, changes = {}) {
  const base = json(read(join(out, `pins-${name}.json`)));
  const merged = { ...base, ...changes };
  if (changes.helpers) merged.helpers = { ...base.helpers, ...changes.helpers };
  const path = input(`pins-${name}-prepared.json`, JSON.stringify(merged, null, 2) + '\n');
  return path;
}
function mutateWat(watPath, transform, label) {
  const text = read(watPath, 'utf8');
  const mutated = transform(text);
  requireThat(mutated !== text, `${label}: the WAT mutation changed nothing`);
  const wat = input(`${label}.wat`, mutated);
  const wasm = join(out, 'inputs', `${label}.wasm`);
  watToWasm(wasmTools, environment, wat, wasm);
  instrument(wasm);
  return wasm;
}
function textEdit(sourcePath, transform, label) {
  const text = read(sourcePath, 'utf8');
  const edited = transform(text);
  requireThat(edited !== text, `${label}: the text edit changed nothing`);
  return input(label, edited);
}

// Counterexample bodies prepared for substituted candidates.
const wrongBody = input('wrong-body.noble', '[ 2 + ]\n');
const definition = input('definition.noble', 'def twice [ dup compose ]\n');

// Pin documents: the release-owner preparation route observes the frozen CLI
// and writes the toolchain document every admission then verifies. The
// test-barrier build is a different executable, so it is pinned separately.
const pinsSha = {};
function emitPins(name, binary, mode) {
  const path = join(out, `pins-${name}.json`);
  nobleBinary(binary, ['admit-proof-artifact', '--emit-pins', path, '--opt', mode]);
  instrument(path);
  pinsSha[name] = fileHash(path);
  return path;
}
emitPins('off', noble.executed, 'off');
emitPins('on', noble.executed, 'on');
if (barrier !== null) emitPins('barrier-off', barrier.executed, 'off');

const increment = {
  law: ownerLaw, digest: V1_SHA256,
  contract: join(root, `${mc2}/increment.contract`),
  proof: join(root, `${mc2}/increment.proof.lean`),
  source: join(root, `${mc2}/q-one.noble`),
};
const guarded = {
  law: join(root, `${mc2}/guarded.contract`),
  digest: fileHash(join(root, `${mc2}/guarded.contract`)),
  contract: join(root, `${mc2}/guarded.contract`),
  proof: join(root, `${mc2}/guarded.proof.lean`),
  source: join(root, `${mc2}/q-one.noble`),
};
const eqFive = {
  law: join(root, 'verification/acorr/fixtures/eq-five.contract'),
  digest: fileHash(join(root, 'verification/acorr/fixtures/eq-five.contract')),
  contract: join(root, 'verification/acorr/fixtures/eq-five.contract'),
  proof: join(root, 'verification/acorr/fixtures/eq-five.proof.lean'),
  source: join(root, `${mc2}/q-one.noble`),
};
const addTwo = {
  law: ownerLawV2, digest: V2_SHA256,
  contract: join(root, 'verification/owner-law/fixtures/add-two.contract'),
  proof: join(root, 'verification/owner-law/fixtures/add-two.proof.lean'),
  source: join(root, 'verification/owner-law/fixtures/add-two.noble'),
};
for (const [name, law] of [['increment', increment], ['guarded', guarded],
  ['eq-five', eqFive], ['add-two', addTwo]]) {
  for (const path of [law.contract, law.proof, law.source]) instrument(path);
}
instrument(join(root, ownerLaw));
instrument(join(root, ownerLawV2));
instrument(join(root, 'policy/tool-selection.json'));
instrument(join(root, 'verification/acorr/gate.mjs'));

// One admission through one frozen binary. Every report is retained by label.
let serial = 0;
function admit(label, specification) {
  const id = `${String(++serial).padStart(3, '0')}-${label}`;
  const argv = [
    'admit-proof-artifact', specification.subject,
    '--law', specification.law.law,
    '--law-digest', specification.law.digest,
    '--source', specification.law.source,
    '--contract', specification.law.contract,
    '--proof', specification.law.proof,
    '--pins', specification.pins,
    '--call', specification.call,
    '--argument', String(specification.argument),
    '--opt', specification.mode,
    '--timeout-ms', '600000',
  ];
  if (specification.manifest) argv.push('--manifest', specification.manifest);
  if (specification.policyRevision) argv.push('--policy-revision', String(specification.policyRevision));
  const extra = specification.env ?? {};
  const result = nobleBinary(specification.binary ?? noble.executed, argv,
    { allowFailure: true, env: { ...environment, ...extra } });
  const reportPath = join(out, 'reports', `${id}.json`);
  writeFileSync(reportPath, result.stdout);
  let report = null;
  try { report = json(result.stdout); } catch (error) { fail(`${id}: no JSON envelope: ${error.message}\n${result.stderr}`); }
  return {
    id, argv, exit: result.status, report,
    reportPath, reportSha256: digest(Buffer.from(result.stdout)),
    envelope: report.envelope ?? null,
  };
}
function stackOf(admission) {
  const raw = admission.envelope?.invocation?.stack;
  return typeof raw === 'string' ? json(raw) : null;
}
function stackPairs(admission) {
  return (stackOf(admission) ?? []).map(entry =>
    [entry.type, entry.value === undefined || entry.value === null ? null : String(entry.value)]);
}
function workerIds(admission) {
  return [admission.envelope?.load?.subject_worker, admission.envelope?.load?.call_worker];
}
function barrierRecord(admission, field) {
  const raw = admission.envelope?.load?.[field];
  return typeof raw === 'string' ? json(raw) : null;
}

const observed = [];
function observation(label, passed, details = {}) {
  observed.push({ label, passed, ...details });
  requireThat(passed, `${label} failed: ${JSON.stringify(details).slice(0, 800)}`);
}

const cases = [];
function case_(id, title, work) {
  const entry = { case: id, title, status: 'not-executed', variants: [] };
  cases.push(entry);
  try {
    work(entry);
    entry.status = entry.variants.every(variant => variant.status === 'passed') ? 'passed' : 'failed';
  } catch (error) {
    entry.status = 'failed';
    entry.error = error.message;
  }
  return entry;
}
function variant(entry, name, work) {
  const record = { name, status: 'passed' };
  entry.variants.push(record);
  try {
    const details = work();
    if (details) record.details = details;
  } catch (error) {
    record.status = 'failed';
    record.error = error.message;
    throw error;
  }
}
function refused(entry, name, admission, code) {
  variant(entry, name, () => {
    observation(`${name}: exit 1 or 4`, admission.exit !== 0, { exit: admission.exit });
    observation(`${name}: outcome ${code}`, admission.report.outcome === code,
      { outcome: admission.report.outcome, refusal: admission.report.refusal });
    observation(`${name}: zero guest requests`, admission.report.guest_requests === 0);
    observation(`${name}: zero protected operations`, admission.report.protected_operations === 0);
    return { code, report_sha256: admission.reportSha256 };
  });
}
function admitted(entry, name, admission, mode, pinsName = mode, expectedCli = noble.sha256) {
  variant(entry, name, () => {
    observation(`${name}: exit 0`, admission.exit === 0, { exit: admission.exit });
    observation(`${name}: admitted`, admission.report.admission === 'admitted',
      { refusal: admission.report.refusal });
    observation(`${name}: subject buffer is the compared buffer`,
      admission.envelope.load.subject_buffer_sha256 === admission.envelope.artifact.subject.wasm_sha256);
    observation(`${name}: call buffer is the compared buffer`,
      admission.envelope.load.call_buffer_sha256 === admission.envelope.artifact.certified_call.wasm_sha256);
    observation(`${name}: both buffers are hashed together`,
      admission.envelope.load.buffers_sha256 === digest(
        Buffer.from(`${admission.envelope.load.subject_buffer_sha256}:${admission.envelope.load.call_buffer_sha256}`)));
    observation(`${name}: optimizer mode is the consumer selection`,
      admission.envelope.build.opt === mode);
    observation(`${name}: fresh worker per artifact`,
      admission.envelope.load.subject_worker !== admission.envelope.load.call_worker);
    observation(`${name}: pinned CLI identity`,
      admission.envelope.toolchain.cli_sha256 === expectedCli,
      { observed: admission.envelope.toolchain.cli_sha256, expected: expectedCli });
    observation(`${name}: pin selection is the frozen document`,
      admission.envelope.pins.selection_sha256 === pinsSha[pinsName],
      { selection: admission.envelope.pins.selection_sha256, expected: pinsSha[pinsName] });
    return {
      opt: mode,
      subject_sha256: admission.envelope.artifact.subject.wasm_sha256,
      call_sha256: admission.envelope.artifact.certified_call.wasm_sha256,
      load_buffers_sha256: admission.envelope.load.buffers_sha256,
      workers: workerIds(admission),
      report_sha256: admission.reportSha256,
    };
  });
}

// ---- ACORR-CASE-01: positive, opt off and on -------------------------------
case_('ACORR-CASE-01', 'positive `[ 1 + ]` increment law, no guard, off and on', entry => {
  for (const mode of ['off', 'on']) {
    const subject = emit(increment.source, mode, 'c1-subject');
    for (const argument of [41n, I64_MAX]) {
      const call = emit(input(`c1-call-${argument}.noble`, callSource(argument, Q_ONE, null)), mode,
        `c1-call-${argument}`);
      const admission = admit(`case01-${mode}-${argument}`, {
        subject: subject.module, call: call.module, law: increment, pins: join(out, `pins-${mode}.json`),
        argument, mode,
      });
      admitted(entry, `off/on ${mode}, x=${argument}`, admission, mode);
      variant(entry, `off/on ${mode}, x=${argument} execution`, () => {
        observation('executed I64(x+1) from the compared buffer',
          JSON.stringify(stackPairs(admission)) ===
            JSON.stringify([['I64', String(wrap(argument + 1n))]]),
          { stack: stackPairs(admission) });
        observation('no guard is bound', admission.envelope.guard === null);
        observation('zero guest requests', admission.report.guest_requests === 0);
        observation('zero protected operations', admission.report.protected_operations === 0);
        return { stack: stackOf(admission) };
      });
    }
  }
});

// ---- ACORR-CASE-02: positive guarded ---------------------------------------
case_('ACORR-CASE-02', 'guarded `[ 1 + ]` with the LtI64Max wrapper', entry => {
  const subject = emit(increment.source, 'off', 'c2-subject');
  const wrapper = wrapperSource('LtI64Max', I64_MAX);
  for (const [name, argument, expected] of [
    ['MAX-1 admits', I64_MAX - 1n, 'inr'],
    ['MAX refuses', I64_MAX, 'inl'],
  ]) {
    const call = emit(input(`c2-call-${argument}.noble`, callSource(argument, Q_ONE, wrapper)), 'off',
      `c2-call-${argument}`);
    const admission = admit(`case02-${argument}`, {
      subject: subject.module, call: call.module, law: guarded, pins: join(out, 'pins-off.json'),
      argument, mode: 'off',
    });
    admitted(entry, name, admission, 'off');
    variant(entry, `${name}: guard and result`, () => {
      observation('the recognized template is bound', admission.envelope.guard?.template === 'LtI64Max');
      observation('the wrapper source is pinned',
        admission.envelope.guard?.wrapper_source_sha256 === digest(Buffer.from(wrapper)));
      const stack = stackOf(admission);
      const tagged = stack?.[0];
      observation(`${name}: the result is a tagged Sum`, tagged?.type === 'Sum', { stack });
      if (expected === 'inl') {
        observation('the wrapper rejected before running the body',
          tagged?.variant === 'left' && tagged?.value?.type === 'I64' && tagged.value.value === '1',
          { stack });
      } else {
        observation('the accepted body ran on the argument',
          tagged?.variant === 'right' && tagged?.value?.type === 'I64' &&
            tagged.value.value === String(wrap(argument + 1n)),
          { stack });
      }
      observation('zero guest requests', admission.report.guest_requests === 0);
      return { stack };
    });
  }
});

// ---- ACORR-CASE-03: correct proof, wrong body ------------------------------
case_('ACORR-CASE-03', 'correct proof with a wrong body', entry => {
  const subject = emit(wrongBody, 'off', 'c3-wrong-subject');
  const call = emit(input('c3-call.noble', callSource(41n, sourceText(mc2 + '/q-one.noble'), null)),
    'off', 'c3-call');
  refused(entry, 'wrong body subject: subject binding', admit('case03-wrong-subject', {
    subject: subject.module, call: call.module, law: increment, pins: join(out, 'pins-off.json'),
    argument: 41n, mode: 'off',
  }), 'subject-mismatch');
  // Candidate `[ 2 + ]` bytes against the consumer-selected `[ 1 + ]` source.
  const validSubject = emit(increment.source, 'off', 'c3-valid-subject');
  const validCall = emit(input('c3-valid-call.noble', callSource(41n, Q_ONE, null)), 'off', 'c3-valid-call');
  const substituted = admit('case03-substituted-bytes', {
    subject: subject.module, call: validCall.module, law: increment, pins: join(out, 'pins-off.json'),
    argument: 41n, mode: 'off',
  });
  refused(entry, 'candidate `[ 2 + ]` bytes with SOURCE `[ 1 + ]`', substituted, 'correspondence-reject');
  // The valid pair is accepted, so the refusal above is not a general failure.
  admitted(entry, 'valid pair after the wrong-body refusals', admit('case03-control', {
    subject: validSubject.module, call: validCall.module, law: increment,
    pins: join(out, 'pins-off.json'), argument: 41n, mode: 'off',
  }), 'off');
});

// ---- ACORR-CASE-04: stale dependency/tool/helper ---------------------------
case_('ACORR-CASE-04', 'stale dependency, tool and helper versions', entry => {
  const subject = emit(increment.source, 'off', 'c4-subject');
  const call = emit(input('c4-call.noble', callSource(41n, Q_ONE, null)), 'off', 'c4-call');
  const outOfProfile = admit('case04-definition', {
    subject: subject.module, call: call.module, law: { ...increment, source: definition },
    pins: join(out, 'pins-off.json'), argument: 41n, mode: 'off',
  });
  refused(entry, 'SOURCE with a definition', outOfProfile, 'unsupported-profile');
  const altered = pinsDocument('off', {
    helpers: { count: 26, set_sha256: '0'.repeat(64) },
  });
  refused(entry, 'pin with an older helper set', admit('case04-old-helpers', {
    subject: subject.module, call: call.module, law: increment, pins: altered,
    argument: 41n, mode: 'off',
  }), 'tool-pin-reject');
  const olderCli = pinsDocument('off', { cli: { sha256: '0'.repeat(64) } });
  refused(entry, 'pin with an older CLI', admit('case04-old-cli', {
    subject: subject.module, call: call.module, law: increment, pins: olderCli,
    argument: 41n, mode: 'off',
  }), 'tool-pin-reject');
});

// ---- ACORR-CASE-05: weakened claim -----------------------------------------
case_('ACORR-CASE-05', 'weakened claim against the frozen increment law', entry => {
  const subject = emit(increment.source, 'off', 'c5-subject');
  const call = emit(input('c5-call.noble', callSource(41n, Q_ONE, null)), 'off', 'c5-call');
  for (const [name, fixture] of [
    ['weakened precondition', 'verification/owner-law/fixtures/weakened-precondition.contract'],
    ['different ensures', 'verification/owner-law/fixtures/identity-statement.contract'],
  ]) {
    refused(entry, name, admit(`case05-${name.replaceAll(' ', '-')}`, {
      subject: subject.module, call: call.module,
      law: { ...increment, contract: join(root, fixture) },
      pins: join(out, 'pins-off.json'), argument: 41n, mode: 'off',
    }), 'law-mismatch');
  }
});

// ---- ACORR-CASE-06: code/recipe divergence ---------------------------------
case_('ACORR-CASE-06', 'code and recipe divergence from the consumer rebuild', entry => {
  const subject = emit(increment.source, 'off', 'c6-subject');
  const call = emit(input('c6-call.noble', callSource(41n, Q_ONE, null)), 'off', 'c6-call');
  const code = mutateWat(subject.wat, text =>
    text.replace('(i64.add (local.get $left) (local.get $right))',
      '(i64.sub (local.get $left) (local.get $right))'), 'c6-code-mutation');
  refused(entry, 'targeted code mutation', admit('case06-code', {
    subject: code, call: call.module, law: increment, pins: join(out, 'pins-off.json'),
    argument: 41n, mode: 'off',
  }), 'correspondence-reject');
  const recipe = mutateWat(subject.wat, text => {
    const marker = '[I64]';
    requireThat(text.includes(marker), 'the emitted recipe data does not carry the subject interface');
    return text.replace(marker, '[I65]');
  }, 'c6-recipe-mutation');
  refused(entry, 'recipe-atom-only mutation', admit('case06-recipe', {
    subject: recipe, call: call.module, law: increment, pins: join(out, 'pins-off.json'),
    argument: 41n, mode: 'off',
  }), 'correspondence-reject');
});

// ---- ACORR-CASE-07: changed guard ------------------------------------------
case_('ACORR-CASE-07', 'changed, substituted and omitted guard wrappers', entry => {
  const subject = emit(increment.source, 'off', 'c7-subject');
  // EqI64Literal 5 contract; a wrapper for literal 6 is substituted.
  const wrongLiteral = emit(input('c7-call-six.noble',
    callSource(5n, Q_ONE, wrapperSource('EqI64Literal', 6n))), 'off', 'c7-call-six');
  refused(entry, 'EqI64Literal 6 wrapper for a k=5 contract', admit('case07-literal', {
    subject: subject.module, call: wrongLiteral.module, law: eqFive, pins: join(out, 'pins-off.json'),
    argument: 5n, mode: 'off',
  }), 'guard-mismatch');
  // NeI64Min substituted for the contract's LtI64Max template.
  const wrongTemplate = emit(input('c7-call-ne.noble',
    callSource(I64_MAX - 1n, Q_ONE, wrapperSource('NeI64Min', I64_MIN))), 'off', 'c7-call-ne');
  refused(entry, 'NeI64Min wrapper substituted for LtI64Max', admit('case07-template', {
    subject: subject.module, call: wrongTemplate.module, law: guarded,
    pins: join(out, 'pins-off.json'), argument: I64_MAX - 1n, mode: 'off',
  }), 'guard-mismatch');
  // A non-`true` precondition with no recognized template: the wrapper is
  // omitted and the consumer must refuse rather than run the subject unguarded.
  const nonnegative = {
    law: join(root, `${mc2}/nonnegative.contract`),
    digest: fileHash(join(root, `${mc2}/nonnegative.contract`)),
    contract: join(root, `${mc2}/nonnegative.contract`),
    proof: join(root, `${mc2}/nonnegative.proof.lean`),
    source: join(root, `${mc2}/q-one.noble`),
  };
  const unguardedCall = emit(input('c7-call-unguarded.noble', callSource(41n, Q_ONE, null)), 'off',
    'c7-call-unguarded');
  refused(entry, 'no recognized template for a non-`true` precondition', admit('case07-missing', {
    subject: subject.module, call: unguardedCall.module, law: nonnegative,
    pins: join(out, 'pins-off.json'), argument: 41n, mode: 'off',
  }), 'guard-mismatch');
  // The wrapper is bound but the candidate omits it.
  refused(entry, 'certified call without the bound wrapper', admit('case07-omitted', {
    subject: subject.module, call: subject.module, law: guarded,
    pins: join(out, 'pins-off.json'), argument: I64_MAX - 1n, mode: 'off',
  }), 'guard-mismatch');
});

// ---- ACORR-CASE-08: altered helper and import ------------------------------
case_('ACORR-CASE-08', 'altered helper implementation, extra import and helper-tree pin', entry => {
  const subject = emit(increment.source, 'off', 'c8-subject');
  const call = emit(input('c8-call.noble', callSource(41n, Q_ONE, null)), 'off', 'c8-call');
  const helper = mutateWat(subject.wat, text => {
    const first = text.indexOf('(i64.add');
    requireThat(first >= 0, 'the emitted subject carries no i64.add');
    return `${text.slice(0, first)}(i64.sub${text.slice(first + '(i64.add'.length)}`;
  }, 'c8-helper-mutation');
  refused(entry, 'altered add implementation', admit('case08-helper', {
    subject: helper, call: call.module, law: increment, pins: join(out, 'pins-off.json'),
    argument: 41n, mode: 'off',
  }), 'correspondence-reject');
  const imported = mutateWat(subject.wat, text => {
    const anchor = '(module';
    requireThat(text.startsWith(anchor), 'the emitted WAT does not start with a module form');
    return `${anchor}\n  (import "noble" "test_emit" (func $extra (param i32 i32 i32) (result i32)))${text.slice(anchor.length)}`;
  }, 'c8-extra-import');
  refused(entry, 'extra effect import', admit('case08-import', {
    subject: imported, call: call.module, law: increment, pins: join(out, 'pins-off.json'),
    argument: 41n, mode: 'off',
  }), 'correspondence-reject');
  refused(entry, 'pinned helper tree of another size', admit('case08-helper-tree', {
    subject: subject.module, call: call.module, law: increment,
    pins: pinsDocument('off', { helpers: { count: 26 } }), argument: 41n, mode: 'off',
  }), 'tool-pin-reject');
});

// ---- ACORR-CASE-09: post-check replacement --------------------------------
case_('ACORR-CASE-09', 'file replacement between comparison and instantiation', entry => {
  if (barrier === null) {
    entry.status = 'not-executed';
    entry.reason = 'no --barrier-cli built with the paa1-test-barrier feature';
    return;
  }
  const subject = emit(increment.source, 'off', 'c9-subject');
  const call = emit(input('c9-call.noble', callSource(41n, Q_ONE, null)), 'off', 'c9-call');
  const replacement = emit(wrongBody, 'off', 'c9-replacement');
  const replacementSha = fileHash(replacement.module);
  // The candidate files the admission reads, plus a build-temp module copy that
  // stands in for a path a re-reading implementation would consult.
  const candidate = join(out, 'candidates', 'c9-candidate.wasm');
  const callCandidate = join(out, 'candidates', 'c9-call-candidate.wasm');
  const tempModule = join(out, 'candidates', 'c9-build-temp-module-1.wasm');
  for (const path of [candidate, callCandidate, tempModule]) copyFileSync(subject.module, path);
  const plan = input('c9-plan.json', JSON.stringify({
    replacement: replacement.module,
    paths: [candidate, callCandidate, tempModule],
  }));
  const admission = admit('case09-after-compare', {
    subject: candidate, call: callCandidate, law: increment, pins: join(out, 'pins-barrier-off.json'),
    argument: 41n, mode: 'off', binary: barrier.executed,
    env: { NOBLE_PAA1_TEST_REPLACE_AFTER_COMPARE: plan },
  });
  admitted(entry, 'replacement after comparison keeps the compared buffer', admission, 'off',
    'barrier-off', barrier.sha256);
  variant(entry, 'post-check replacement observation', () => {
    const record = barrierRecord(admission, 'subject_test_barrier');
    observation('the barrier ran between comparison and instantiation', record !== null);
    observation('the barrier replaced every named file', Array.isArray(record.replaced) && record.replaced.length === 3,
      { replaced: record?.replaced });
    observation('the compared candidate is the admitted buffer',
      record.compared_candidate_sha256 === admission.envelope.artifact.subject.wasm_sha256);
    observation('the compared module is the admitted buffer',
      record.compared_module_sha256 === admission.envelope.artifact.subject.wasm_sha256);
    observation('the executed result is the compared buffer, not the replacement',
      JSON.stringify(stackPairs(admission)) === JSON.stringify([['I64', '42']]),
      { stack: stackPairs(admission), replacement_sha256: replacementSha });
    observation('the files on disk now hold the replacement',
      fileHash(candidate) === replacementSha && fileHash(tempModule) === replacementSha &&
        fileHash(callCandidate) === replacementSha);
    return { barrier: record, replacement_sha256: replacementSha };
  });
  // The same environment variable is inert without the feature build, and the
  // production CLI still executes the compared buffer.
  const fresh = join(out, 'candidates', 'c9-production-candidate.wasm');
  const freshCall = join(out, 'candidates', 'c9-production-call.wasm');
  copyFileSync(subject.module, fresh);
  copyFileSync(call.module, freshCall);
  const inertPlan = input('c9-inert-plan.json', JSON.stringify({
    replacement: replacement.module, paths: [fresh, freshCall],
  }));
  const production = admit('case09-production-inert', {
    subject: fresh, call: freshCall, law: increment, pins: join(out, 'pins-off.json'),
    argument: 41n, mode: 'off', env: { NOBLE_PAA1_TEST_REPLACE_AFTER_COMPARE: inertPlan },
  });
  admitted(entry, 'production build has no barrier', production, 'off');
  variant(entry, 'production environment is inert', () => {
    observation('no barrier record in the production build',
      barrierRecord(production, 'subject_test_barrier') === null);
    observation('the production files are unchanged', fileHash(fresh) === fileHash(subject.module));
    observation('the production execution is the compared subject',
      JSON.stringify(stackPairs(production)) === JSON.stringify([['I64', '42']]),
      { stack: stackPairs(production) });
    return {};
  });
  // Replacing before the comparison must refuse.
  const replacedEarlier = join(out, 'candidates', 'c9-replaced-earlier.wasm');
  copyFileSync(replacement.module, replacedEarlier);
  refused(entry, 'replacement before comparison refuses', admit('case09-before-compare', {
    subject: replacedEarlier, call: call.module, law: increment, pins: join(out, 'pins-off.json'),
    argument: 41n, mode: 'off',
  }), 'correspondence-reject');
});

// ---- ACORR-CASE-10: producer metadata forgery ------------------------------
case_('ACORR-CASE-10', 'producer metadata forgery is discarded', entry => {
  const subject = emit(increment.source, 'off', 'c10-subject');
  const call = emit(input('c10-call.noble', callSource(41n, Q_ONE, null)), 'off', 'c10-call');
  const forged = input('c10-forged.json', JSON.stringify({
    claimed_effects: [],
    trusted_correspondence: true,
    source: fileHash(subject.module),
    digest: 'f'.repeat(64),
    tool_sha256: 'e'.repeat(64),
    opt: 'on',
    allowed: true,
  }));
  const forgedAdmission = admit('case10-forged', {
    subject: subject.module, call: call.module, law: increment, pins: join(out, 'pins-off.json'),
    argument: 41n, mode: 'off', manifest: forged,
  });
  const plain = admit('case10-plain', {
    subject: subject.module, call: call.module, law: increment, pins: join(out, 'pins-off.json'),
    argument: 41n, mode: 'off',
  });
  admitted(entry, 'forged manifest still admits the valid pair', forgedAdmission, 'off');
  variant(entry, 'identical outcome and identities with and without metadata', () => {
    observation('forged manifest parsed and discarded',
      forgedAdmission.envelope.candidate.manifest.present === true &&
        JSON.stringify(forgedAdmission.envelope.candidate.manifest.discarded_fields.sort()) ===
          JSON.stringify(['allowed', 'claimed_effects', 'digest', 'opt', 'source', 'tool_sha256', 'trusted_correspondence']));
    observation('the producer opt claim did not select the mode',
      forgedAdmission.envelope.build.opt === 'off');
    observation('the same subject artifact',
      forgedAdmission.envelope.artifact.subject.wasm_sha256 === plain.envelope.artifact.subject.wasm_sha256);
    observation('the same certified call artifact',
      forgedAdmission.envelope.artifact.certified_call.wasm_sha256 === plain.envelope.artifact.certified_call.wasm_sha256);
    observation('the same instantiated buffers',
      forgedAdmission.envelope.load.buffers_sha256 === plain.envelope.load.buffers_sha256);
    observation('the same execution observations',
      JSON.stringify(stackOf(forgedAdmission)) === JSON.stringify(stackOf(plain)));
    return { forged_report_sha256: forgedAdmission.reportSha256, plain_report_sha256: plain.reportSha256 };
  });
});

// ---- ACORR-CASE-11: erasure ------------------------------------------------
case_('ACORR-CASE-11', 'proof-only edits change no executable byte, off and on', entry => {
  const proof = read(increment.proof, 'utf8');
  const editedProof = input('c11-proof-edited.lean', `-- a proof-only comment edit\n${proof}`);
  const results = {};
  for (const [name, proofPath] of [['original', increment.proof], ['proof-only edit', editedProof]]) {
    for (const mode of ['off', 'on']) {
      const subject = emit(increment.source, mode, `c11-subject-${mode}`);
      const call = emit(input(`c11-call-${mode}.noble`, callSource(41n, Q_ONE, null)), mode,
        `c11-call-${mode}`);
      const admission = admit(`case11-${name.replaceAll(' ', '-')}-${mode}`, {
        subject: subject.module, call: call.module,
        law: { ...increment, proof: proofPath }, pins: join(out, `pins-${mode}.json`),
        argument: 41n, mode,
      });
      admitted(entry, `${name}/${mode}`, admission, mode);
      results[`${name}/${mode}`] = {
        subject: admission.envelope.artifact.subject.wasm_sha256,
        call: admission.envelope.artifact.certified_call.wasm_sha256,
        proof: admission.envelope.proof.artifact_sha256,
      };
    }
  }
  variant(entry, 'the executable bytes are unchanged by the proof edit', () => {
    for (const mode of ['off', 'on']) {
      observation(`${mode}: subject bytes unchanged`,
        results[`original/${mode}`].subject === results[`proof-only edit/${mode}`].subject);
      observation(`${mode}: certified-call bytes unchanged`,
        results[`original/${mode}`].call === results[`proof-only edit/${mode}`].call);
      observation(`${mode}: the evidence bytes changed`,
        results[`original/${mode}`].proof !== results[`proof-only edit/${mode}`].proof);
    }
    observation('the optimization mode changes the subject bytes',
      results['original/off'].subject !== results['original/on'].subject);
    return results;
  });
});

// ---- ACORR-CASE-12: cache and revalidation --------------------------------
case_('ACORR-CASE-12', 'mode, policy, law and pin changes force full revalidation', entry => {
  const subject = emit(increment.source, 'off', 'c12-subject');
  const call = emit(input('c12-call.noble', callSource(41n, Q_ONE, null)), 'off', 'c12-call');
  const off = admit('case12-off', {
    subject: subject.module, call: call.module, law: increment, pins: join(out, 'pins-off.json'),
    argument: 41n, mode: 'off',
  });
  admitted(entry, 'first off admission', off, 'off');
  // The on-mode admission compiles new subject and call candidates.
  const onSubject = emit(increment.source, 'on', 'c12-subject-on');
  const onCall = emit(input('c12-call-on.noble', callSource(41n, Q_ONE, null)), 'on', 'c12-call-on');
  const on = admit('case12-on', {
    subject: onSubject.module, call: onCall.module, law: increment, pins: join(out, 'pins-on.json'),
    argument: 41n, mode: 'on',
  });
  admitted(entry, 'mode change revalidates and uses a new selection', on, 'on');
  variant(entry, 'mode change observations', () => {
    observation('the two admissions bind different pin selections',
      off.envelope.pins.selection_sha256 !== on.envelope.pins.selection_sha256);
    const workers = new Set([...workerIds(off), ...workerIds(on)]);
    observation('every admission used fresh workers', workers.size === 4, { workers: [...workers] });
    return { off_workers: workerIds(off), on_workers: workerIds(on) };
  });
  // Only the pin document's opt_mode changes: full revalidation refuses it.
  refused(entry, 'pin mode change forces revalidation', admit('case12-pin-mode', {
    subject: subject.module, call: call.module, law: increment,
    pins: pinsDocument('off', { opt_mode: 'on' }), argument: 41n, mode: 'off',
  }), 'tool-pin-reject');
  // Only the policy revision changes: a new envelope and fresh workers.
  const revised = admit('case12-policy-revision', {
    subject: subject.module, call: call.module, law: increment, pins: join(out, 'pins-off.json'),
    argument: 41n, mode: 'off', policyRevision: 2,
  });
  admitted(entry, 'policy revision change revalidates', revised, 'off');
  variant(entry, 'policy revision observations', () => {
    observation('the envelope binds the selected policy revision',
      revised.envelope.proof.policy_revision === 2);
    observation('the first admission bound the pinned revision',
      off.envelope.proof.policy_revision === 1);
    observation('the policy change used fresh workers',
      workerIds(revised).every(worker => !workerIds(off).includes(worker)),
      { workers: workerIds(revised) });
    return { workers: workerIds(revised) };
  });
  // Only the law revision changes: the v2 record admits its own subject and
  // refuses the v1 contract.
  const addTwoSubject = emit(addTwo.source, 'off', 'c12-add-two-subject');
  const addTwoCall = emit(input('c12-add-two-call.noble', callSource(41n, sourceText('verification/owner-law/fixtures/add-two.noble'), null)),
    'off', 'c12-add-two-call');
  const revision = admit('case12-law-revision', {
    subject: addTwoSubject.module, call: addTwoCall.module, law: addTwo,
    pins: join(out, 'pins-off.json'), argument: 41n, mode: 'off',
  });
  admitted(entry, 'law revision change revalidates', revision, 'off');
  variant(entry, 'law revision observations', () => {
    observation('the v2 record is a different law identity',
      revision.envelope.owner_law.revision !== off.envelope.owner_law.revision);
    observation('the v2 law admitted its own statement',
      revision.envelope.owner_law.digest.matched === true &&
        revision.envelope.owner_law.digest.computed === V2_SHA256);
    return { revision: revision.envelope.owner_law.revision };
  });
  refused(entry, 'the v1 contract under the v2 law refuses', admit('case12-law-substitution', {
    subject: subject.module, call: call.module,
    law: { ...addTwo, contract: join(root, `${mc2}/increment.contract`), source: increment.source },
    pins: join(out, 'pins-off.json'), argument: 41n, mode: 'off',
  }), 'law-mismatch');
});

// ---- receipt ---------------------------------------------------------------
const final = gitState();
requireThat(final.head === initial.head && final.tree === initial.tree &&
  final.status === initial.status, 'the Noble source worktree changed during the gate');
const executed = cases.filter(entry => entry.status === 'passed');
const failed = cases.filter(entry => entry.status === 'failed');
const notExecuted = cases.filter(entry => entry.status === 'not-executed');
const receipt = {
  schema: 'noble-acorr-gate/v1',
  case_ids: ['ACORR-CASE-01', 'ACORR-CASE-02', 'ACORR-CASE-03', 'ACORR-CASE-04',
    'ACORR-CASE-05', 'ACORR-CASE-06', 'ACORR-CASE-07', 'ACORR-CASE-08',
    'ACORR-CASE-09', 'ACORR-CASE-10', 'ACORR-CASE-11', 'ACORR-CASE-12'],
  source: {
    commit: initial.head, tree: initial.tree, clean: initial.status === '',
    dirty_allowed: allowDirty,
  },
  frozen_cli: noble,
  barrier_cli: barrier,
  environment,
  pins: Object.fromEntries(Object.entries(pinsSha).map(([name, sha256]) => [name, {
    path: join(out, `pins-${name}.json`), sha256,
    document: json(read(join(out, `pins-${name}.json`))),
  }])),
  inputs,
  gate_sha256: instrument(join(root, 'verification/acorr/gate.mjs')),
  prepare_sha256: instrument(join(root, 'verification/acorr/prepare.mjs')),
  commands: commands.length,
  cases,
  result: failed.length === 0 && notExecuted.length === 0 ? 'passed'
    : failed.length > 0 ? 'failed' : 'incomplete',
};
receipt.receipt_sha256 = digest(Buffer.from(JSON.stringify(receipt, null, 2) + '\n'));
writeFileSync(join(out, 'receipt.json'), JSON.stringify(receipt, null, 2) + '\n');
console.log(JSON.stringify({
  result: receipt.result,
  receipt: join(out, 'receipt.json'),
  source_commit: receipt.source.commit,
  frozen_cli_sha256: noble.sha256,
  cases: cases.map(entry => `${entry.case}:${entry.status}`),
  commands: commands.length,
}, null, 2));
if (receipt.result !== 'passed') process.exitCode = 1;
