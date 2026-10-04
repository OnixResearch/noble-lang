import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { sha256, fileHash } from './record.mjs';

const strictAxioms = new Set(['propext', 'Classical.choice', 'Quot.sound']);
const proofName = { 'CONTRACT-17': 'poly-refl', 'CONTRACT-18': 'increment-correct',
  'CONTRACT-19': 'poly-refl', 'CONTRACT-21': 'poly-refl', 'CONTRACT-25': 'poly-refl' };
const positiveImports = {
  'CONTRACT-17': {
    I64: ['type-I64', 'i64-refl'], Bool: ['type-Bool', 'bool-refl'],
    'Pair<I64,List<Bool>>': ['type-Pair-I64-List-Bool', 'pair-refl'],
  },
  'CONTRACT-25': { imported: ['accepted-imported-use', 'imported-list'] },
};
const rejectedTypeNames = {
  'Resource<test.counter>': 'Resource-test-counter',
  'Program<I64,I64,pure>': 'Program-I64-I64-pure',
  Syntax: 'Syntax', 'Pair<I64,Resource<test.counter>>': 'Pair-I64-Resource',
  Type1: 'Type1', 'forged-PureTyCode': 'forged-PureTyCode',
};
const closure = [
  ['NobleContracts', 'proofs/mc1/NobleContracts.lean'],
  ['NobleContracts.Model', 'proofs/mc1/NobleContracts/Model.lean'],
  ['NobleContracts.Rules', 'proofs/mc1/NobleContracts/Rules.lean'],
  ['NobleContracts.Expression', 'proofs/mc1/NobleContracts/Expression.lean'],
  ['NobleContracts.Obligation', 'proofs/mc1/NobleContracts/Obligation.lean'],
  ['NobleContracts.Examples', 'proofs/mc1/NobleContracts/Examples.lean'],
  ['NobleContracts.Composition', 'proofs/mc1/NobleContracts/Composition.lean'],
];
const typeWitness = 'proofs/mc1/IntrinsicTypeWitness.lean';
const contractSource = name => `crates/noble-contracts/src/${name}`;
const checkedSources = [
  ...[
    'intrinsic.rs', 'source.rs', 'source/lexer.rs', 'source/lexer/tokens.rs',
    'source/lexer/text.rs', 'source/parsing.rs', 'source/preparation.rs',
    'source/preflight.rs', 'source/preflight/paths.rs', 'source/resolution.rs',
    'source/resolution/comparison.rs', 'source/declared.rs',
    'source/declared/types.rs', 'source/declared/signatures.rs',
    'source/declared/links.rs', 'source/declared/parsing.rs',
    'source/declared/parsing/body.rs', 'source/declared/parsing/imports.rs',
    'source/declared/parsing/logic.rs', 'source/declared/parsing/members.rs',
    'source/declared/state.rs', 'source/declared/state/prepare.rs',
    'source/declared/state/register.rs', 'source/declared/state/register/adapter.rs',
    'source/declared/state/register/collection.rs',
    'source/declared/state/register/definitions.rs',
    'source/declared/state/register/definitions/graph.rs',
    'source/declared/state/register/definitions/scheduling.rs',
    'source/declared/state/register/exports.rs',
    'source/declared/state/register/logical.rs',
    'source/declared/state/register/publication.rs',
    'source/declared/state/register/schema.rs',
    'source/declared/state/register/schema/names.rs',
    'source/declared/state/register/schema/nominals.rs',
    'source/declared/state/register/schema/types.rs',
    'source/declared/state/namespace.rs',
    'source/declared/state/namespace/words.rs',
  ].map(contractSource),
  'crates/noble-cli/src/core/declared/mod.rs',
  'crates/noble-cli/src/core/entry/compilation.rs',
  typeWitness,
  'crates/noble-cli/src/workflow/intrinsic.rs',
  'crates/noble-cli/src/workflow/verification.rs',
  'crates/noble-cli/src/consumer/decoding.lean',
  'crates/noble-cli/src/consumer/replay.lean',
];
const sizeU64 = buffer => {
  const result = Buffer.alloc(8);
  result.writeBigUInt64LE(BigInt(buffer.length));
  return result;
};
function pinnedBytes(context, source) {
  const entry = context.plan.sources[source];
  assert.ok(entry, `missing checked source: ${source}`);
  const file = path.join(context.record.output, entry.snapshot);
  assert.equal(fileHash(file), entry.sha256, `${source}: modified checker source snapshot`);
  return fs.readFileSync(file);
}
function pinnedRevisions(context) {
  if (context.revisions) return context.revisions;
  const witness = pinnedBytes(context, typeWitness);
  const model = [
    Buffer.from('noble-intrinsic-import-closure/v2'),
    ...closure.flatMap(([name, source]) => {
      const moduleName = Buffer.from(name);
      const body = pinnedBytes(context, source);
      return [sizeU64(moduleName), moduleName, sizeU64(body), body];
    }),
    sizeU64(witness), witness,
  ];
  return context.revisions = {
    model: sha256(Buffer.concat(model)),
    checker: sha256(Buffer.concat(checkedSources.map(source => pinnedBytes(context, source)))),
  };
}

function sourceEntry(context, id, name, group = 'variants') {
  const entry = group === 'baseline'
    ? context.plan.inputs[id]?.baseline
    : context.plan.inputs[id]?.[group]?.[name];
  assert.ok(entry, `${id}/${name}: missing frozen source`);
  return entry;
}

function sourcePath(context, id, name, group = 'variants') {
  return context.proof.asset(id, name, group);
}

function sourceIdentity(observed, entry, label, requireModule = true) {
  assert.equal(observed.report.source_sha256, entry.sha256,
    `${label}: proof report did not bind actual submitted source bytes`);
  if (requireModule)
    assert.ok(observed.report.module && typeof observed.report.module.name === 'string'
      && Number.isSafeInteger(observed.report.module.version),
    `${label}: no resolved immutable source module identity`);
  if (requireModule)
    assert.equal(observed.report.module.source_sha256, entry.sha256,
      `${label}: resolved module refers to other source bytes`);
}

function quietProof(report, label) {
  for (const key of ['guest_requests', 'host_requests', 'protected_operations',
    'candidate_executions', 'runtime_prover_calls'])
    assert.equal(report[key], 0, `${label}: proof check executed guest/host work (${key})`);
  assert.equal(report.proof_grants_host_authority, false,
    `${label}: source theorem granted host authority`);
}

export function accepted(observed, entry, expectedProof, label, context, expectedCount = 1) {
  assert.equal(observed.exit, 0, `${label}: proof source not independently accepted`);
  const report = observed.report;
  sourceIdentity(observed, entry, label);
  assert.equal(report.outcome, 'proved', `${label}: not a strict Lean proof`);
  assert.equal(report.stage, 'independent-recheck',
    `${label}: proof was accepted without independent claim checking`);
  assert.equal(report.independent_recheck, true, `${label}: producer-only acceptance`);
  assert.equal(report.proofs?.length, expectedCount, `${label}: accepted wrong proof count`);
  assert.equal(report.proofs.at(-1).name, expectedProof, `${label}: wrong proof identity`);
  assert.ok(report.proofs.every(item => item.claim && item.lowered_term),
    `${label}: no checked typed claim/lowering`);
  const pins = pinnedRevisions(context);
  for (const proof of report.proofs) {
    assert.equal(proof.model_revision, pins.model,
      `${label}: independently checked claim uses different model revision`);
    assert.equal(proof.checker_revision, pins.checker,
      `${label}: independently checked term uses different checker/consumer revision`);
    assert.ok(Array.isArray(proof.strict_lean_axioms)
      && proof.strict_lean_axioms.every(name => strictAxioms.has(name)),
    `${label}: unchecked Lean axiom used`);
    const selected = context.record.receipt.tools.lean.original;
    assert.equal(fs.realpathSync(proof.strict_lean_tools?.lean), selected,
      `${label}: proof was independently checked by another Lean tool`);
    for (const [field, tool] of [
      ['bubblewrap', 'noble_bwrap'],
      ['prlimit', 'noble_prlimit'],
      ['systemd_run', 'noble_systemd_run'],
    ]) assert.equal(fs.realpathSync(proof.strict_lean_tools?.[field]),
      context.record.receipt.tools[tool].original,
      `${label}: independent checker used unpinned ${field}`);
    assert.match(proof.strict_lean_tools?.lean_version ?? '', /Lean.*4\.31\.0/iu,
      `${label}: unselected Lean kernel version`);
    assert.match(proof.consumer_wire_sha256 ?? '', /^[0-9a-f]{64}$/u,
      `${label}: independent consumer wire identity missing`);
  }
  assert.ok(Number.isSafeInteger(report.work?.normalization)
    && Number.isSafeInteger(report.work?.substitution)
    && Number.isSafeInteger(report.work?.total),
  `${label}: bounded checker work trace absent`);
  quietProof(report, label);
  return { name: label, command: observed.command, outcome: report.outcome,
    source_sha256: entry.sha256, module: report.module,
    proof: report.proofs.at(-1).name, claim_sha256: sha256(Buffer.from(report.proofs.at(-1).claim)),
    axiom_count: report.proofs.reduce((count, item) => count + item.strict_lean_axioms.length, 0),
    model_revision: pins.model, checker_revision: pins.checker,
    work: report.work };
}

export function refused(observed, entry, label, { outcome, code, reason } = {}) {
  assert.notEqual(observed.exit, 0, `${label}: hostile proof was accepted`);
  sourceIdentity(observed, entry, label, false);
  const report = observed.report;
  assert.notEqual(report.outcome, 'proved', `${label}: forged proof produced proved status`);
  assert.equal(report.independent_recheck, false, `${label}: refused proof was reclassified`);
  assert.ok(['parse', 'resolve', 'link', 'check', 'source-exhausted',
    'acceptance', 'proof-check', 'independent-recheck'].includes(report.stage),
    `${label}: refusal did not occur at an observed checker boundary`);
  assert.equal(report.outcome, outcome ?? 'error', `${label}: wrong refusal outcome`);
  assert.ok(typeof report.diagnostic?.code === 'string'
    && typeof report.diagnostic?.message === 'string' && report.diagnostic.message.length > 0,
  `${label}: no typed rejection diagnostic`);
  if (code) assert.equal(report.diagnostic.code, code, `${label}: wrong refusal class`);
  if (reason) assert.match(report.diagnostic.message, reason,
    `${label}: rejection did not reach the intended boundary`);
  quietProof(report, label);
  return { name: label, command: observed.command, outcome: report.outcome,
    code: report.diagnostic.code, stage: report.stage,
    source_sha256: entry.sha256 };
}

function verify(context, id, group, name, options = {}) {
  return context.proof.verify(`${id}-${name}`, sourcePath(context, id, name, group), options);
}

function caseRow(id, positive, hostiles, details = {}) {
  return { id, passed: true, positive, hostiles, details };
}

function realConsumerFailure(context) {
  const id = 'CONTRACT-21', name = 'internal-checker-error';
  const entry = sourceEntry(context, id, name);
  const source = sourcePath(context, id, name);
  const node = path.join(context.record.output, context.plan.tools.node.file);
  const lean = context.record.receipt.tools.lean.original;
  const helper = path.join(context.record.receipt.source_root,
    'verification/intrinsic-proofs/consumer-fault.mjs');
  assert.equal(context.record.watch(helper),
    context.plan.sources['verification/intrinsic-proofs/consumer-fault.mjs'].sha256,
    'real process fault helper changed after the source freeze');
  const run = context.record.command('CONTRACT-21-real-Lean-consumer-SIGKILL', node,
    [helper, context.tools.cli, lean, source, '600000'],
    { env: context.environment, timeout: 180_000 });
  const fault = JSON.parse(run.bytes.toString('utf8'));
  assert.equal(run.status, 0, 'did not witness an exact pinned Lean consumer child');
  assert.equal(fault.schema, 'noble-real-lean-consumer-fault/v1');
  assert.equal(fault.result, 'consumer-process-killed',
    'actual pinned Lean consumer process did not receive SIGKILL');
  assert.equal(fault.selected_version?.status, 0, 'pinned Lean version was not checked');
  const version = Buffer.from(fault.selected_version.stdout_base64, 'base64');
  assert.match(version.toString('utf8'), /Lean \(version 4\.31\.0,.*68218e876d2a38b1985b8590fff244a83c321783/u);
  const target = fault.target;
  assert.ok(Number.isSafeInteger(target?.pid) && target.pid > 0
    && Number.isSafeInteger(target?.parent_pid) && target.parent_pid > 0,
  'target process PID and parent PID were not observed');
  assert.equal(target.exe, lean, 'killed process was not the selected real Lean');
  assert.ok(target.argv?.includes('/consumer/IntrinsicConsumer.lean'),
    'target was not the isolated typed proof consumer');
  assert.equal(target.ancestry?.[0], target.pid,
    'target ancestry did not begin at the observed pinned Lean PID');
  assert.equal(target.ancestry?.at(-1), fault.cli?.pid,
    'target ancestry did not reach the exact frozen source-proof CLI');
  assert.equal(target.signal, 'SIGKILL', 'did not terminate the exact checker process');
  assert.equal(fault.cli?.executable, fs.realpathSync(context.tools.cli),
    'fault control used another source-proof CLI');
  assert.equal(fault.cli?.sha256, fileHash(context.tools.cli),
    'fault control executed changed CLI bytes');
  assert.deepEqual(fault.cli?.argv,
    ['verify-module', source, '--timeout-ms', '600000'],
    'fault control did not check the frozen canonical source');
  assert.equal(fault.cli?.source_sha256, entry.sha256,
    'fault control substituted a different source module');
  assert.equal(fault.cli.status, 2, 'consumer process failure was not an error refusal');
  assert.equal(fault.cli.signal, null, 'CLI, rather than real consumer, was killed');
  const stdout = Buffer.from(fault.cli.stdout_base64, 'base64');
  const stderr = Buffer.from(fault.cli.stderr_base64, 'base64');
  const retained = Object.fromEntries([
    ['source-cli.stdout', stdout], ['source-cli.stderr', stderr],
    ['selected-lean-version.stdout', version],
    ['selected-lean-version.stderr',
      Buffer.from(fault.selected_version.stderr_base64, 'base64')],
  ].map(([file, bytes]) => {
    const frozen = context.record.retain(`controls/CONTRACT-21/${file}`, bytes);
    return [file, { file: context.record.relative(frozen), sha256: fileHash(frozen) }];
  }));
  const report = JSON.parse(stdout.toString('utf8').trim());
  const refusal = refused({ exit: fault.cli.status, command: run.id, report },
    entry, name, { code: 'intrinsic-checker-process-failure' });
  assert.equal(refusal.stage, 'independent-recheck',
    'fault did not occur after source typing and real consumer launch');
  assert.equal(report.diagnostic.details?.exit_status, 'exit status: 137',
    'real pinned consumer was not observed exiting after SIGKILL');
  assert.equal(report.proofs, undefined,
    'failed consumer process published a source proof despite the refusal');
  assert.ok(fault.monitor?.observed_ms < fault.monitor?.deadline_ms,
    'consumer fault witnessed only after expired observation deadline');
  return { ...refusal, target, cli_pid: fault.cli.pid,
    cli_status: fault.cli.status, selected_lean_version_status: fault.selected_version.status,
    consumer_exit_status: report.diagnostic.details.exit_status,
    cause: 'actual pinned Lean consumer process killed by bounded test control',
    retained };
}

export function polymorphicCase(context) {
  const id = 'CONTRACT-17', canonical = context.plan.canonical.cases[id];
  const base = verify(context, id, 'baseline', 'baseline');
  const positive = [accepted(base, sourceEntry(context, id, 'baseline', 'baseline'),
    proofName[id], 'polymorphic-reflexivity', context)];
  const claim = base.report.proofs[0].claim;
  assert.match(claim, /PureTyCode/u, 'Type0 binder lowered to unrestricted Lean Type');
  assert.match(claim, /El/u, 'decoded value binder absent from independent theorem');
  for (const ty of canonical.input.positive_type_instances) {
    const [fixture, declaration] = positiveImports[id][ty];
    const observed = verify(context, id, 'extras', fixture);
    positive.push(accepted(observed, sourceEntry(context, id, fixture, 'extras'),
      declaration, ty, context, 2));
  }
  const hostiles = [];
  for (const name of canonical.input.variants.slice(1)) {
    const observed = verify(context, id, 'variants', name);
    hostiles.push(refused(observed, sourceEntry(context, id, name), name,
      { reason: name === 'proof-lexer-escape-into-def' ? /delimiter|parenthesis|proof|parse/iu : undefined }));
  }
  for (const ty of canonical.input.negative_type_instances) {
    const name = rejectedTypeNames[ty], observed = verify(context, id, 'extras', name);
    const entry = sourceEntry(context, id, name, 'extras');
    hostiles.push(refused(observed, entry, ty,
      { reason: /Type0|code|Resource|Program|Syntax|logical|proof/iu }));
  }
  return caseRow(id, positive, hostiles, { lean_binder: claim,
    source_translation_correspondence_proved_by_test: false });
}

export function exactMc1Case(context) {
  const id = 'CONTRACT-18';
  const current = verify(context, id, 'baseline', 'baseline');
  const baseline = accepted(current, sourceEntry(context, id, 'baseline', 'baseline'),
    proofName[id], 'exact-source-proof', context);
  assert.equal(current.report.proofs[0].kind, 'contract', 'not a checked MC1 contract theorem');
  const mc1 = context.plan.sources['verification/mc1/increment.noble-contract'];
  const contractFile = path.join(context.record.output, mc1.snapshot);
  assert.equal(fileHash(contractFile), mc1.sha256);
  const regenerated = context.record.command('independent-mc1-expected-claim', context.tools.cli,
    ['explain-proof', contractFile], { env: context.environment, timeout: 180_000 });
  assert.equal(regenerated.status, 0, 'selected CLI failed to independently regenerate MC1 claim');
  const expected = JSON.parse(regenerated.bytes.toString('utf8'));
  assert.equal(expected.outcome, 'not-run', 'explain-proof unexpectedly executed a proof');
  assert.equal(expected.ordinary_typing?.outcome, 'accepted', 'MC1 analog source not typed');
  const subject = current.report.proofs[0].subject;
  assert.equal(subject?.generated_statement, expected.generated_statement,
    'module contract did not target exact independently regenerated MC1 statement');
  assert.equal(subject?.accepted_candidate, expected.subject?.accepted_candidate,
    'module proof was not bound to exact accepted `[ 1 + ]` subject');
  const hostiles = [];
  for (const name of context.plan.canonical.cases[id].input.variants.slice(1)) {
    const observed = verify(context, id, 'variants', name);
    hostiles.push(refused(observed, sourceEntry(context, id, name), name));
  }
  return caseRow(id, [baseline], hostiles, {
    regenerated_mc1_command: regenerated.id, generated_statement_sha256:
      sha256(Buffer.from(expected.generated_statement)), normal_return_only: true,
    signed_wrap_vectors: context.plan.canonical.cases[id].input.vectors });
}

export function forgedSourceCase(context) {
  const id = 'CONTRACT-19', base = verify(context, id, 'baseline', 'baseline');
  const positive = [accepted(base, sourceEntry(context, id, 'baseline', 'baseline'),
    proofName[id], 'independent-valid-polymorphic-proof-before-hostiles', context)];
  const hostiles = [];
  for (const name of context.plan.canonical.cases[id].input.variants) {
    const observed = verify(context, id, 'variants', name);
    const reason = ['axiom', 'admit', 'sorry', 'Lean-by-tactic',
      'unchecked-native-decision', 'unknown-rule'].includes(name) ? /proof|constructor|unknown|unsupported/iu : undefined;
    hostiles.push(refused(observed, sourceEntry(context, id, name), name, { reason }));
  }
  return caseRow(id, positive, hostiles, { unchecked_premises: 0 });
}

export function boundedCase(context, peer) {
  const id = 'CONTRACT-21', baseline = verify(context, id, 'baseline', 'baseline');
  const positive = [accepted(baseline, sourceEntry(context, id, 'baseline', 'baseline'),
    proofName[id], 'well-formed-small-polymorphic-proof', context)];
  const hostiles = [];
  let internalFaultGap = null;
  for (const name of context.plan.canonical.cases[id].input.variants.slice(1)) {
    if (name === 'internal-checker-error') {
      try { hostiles.push(realConsumerFailure(context)); }
      catch (error) {
        internalFaultGap = String(error.stack ?? error);
        hostiles.push({ name, outcome: 'blocked',
          source_sha256: sourceEntry(context, id, name).sha256,
          gap: internalFaultGap });
      }
      continue;
    }
    if (name === 'invalid-import-transitive-axiom') {
      assert.equal(peer?.transitive?.forged_nested_dependency_rejected, true,
        'production checker admitted a producer-forged nested dependency');
      const untrusted = verify(context, id, 'extras', 'untrusted-axiom-module');
      const refusal = refused(untrusted, sourceEntry(context, id, 'untrusted-axiom-module', 'extras'),
        'untrusted-exporter', { reason: /axiom|unknown|proof|unsupported/iu });
      hostiles.push({ name, command: peer.command, outcome: 'refused-transitive-axiom',
        untrusted_module_command: untrusted.command,
        untrusted_module_diagnostic: refusal.code,
        checker_diagnostic: peer.transitive.diagnostic });
      continue;
    }
    const options = {
      'normalization-work-exhaustion': { flags: ['--normalization-work', '1'] },
      'substitution-work-exhaustion': { flags: ['--substitution-work', '1'] },
      'proof-timeout': { timeoutMs: 1 },
    }[name] ?? {};
    const observed = verify(context, id, 'variants', name, options);
    const expected = {
      'normalization-work-exhaustion': { code: 'normalization-work-exhausted' },
      'substitution-work-exhaustion': { code: 'substitution-work-exhausted' },
      'proof-timeout': { outcome: 'timeout', code: 'timeout' },
      'partial-selector-used-in-Eq': {
        code: 'intrinsic-parse-invalid', reason: /logical delimiter or escape/iu },
    }[name] ?? {};
    hostiles.push(refused(observed, sourceEntry(context, id, name), name, expected));
  }
  const missingTool = verify(context, id, 'variants', 'internal-checker-error',
    { environment: { NOBLE_LEAN: '/nonexistent/noble-intrinsic-proof-lean' } });
  const operationalNegative = refused(missingTool,
    sourceEntry(context, id, 'internal-checker-error'), 'missing-tool-operational-negative',
    { outcome: 'unsupported', code: 'missing-tool' });
  const exhausted = hostiles.filter(row => ['normalization-work-exhaustion',
    'substitution-work-exhaustion', 'proof-timeout', 'internal-checker-error'].includes(row.name)
      && row.outcome !== 'blocked');
  assert.equal(new Set(exhausted.map(row => `${row.outcome}:${row.code}`)).size, exhausted.length,
    'checker collapsed work, timeout and real consumer-process failure into one class');
  assert.ok(!exhausted.some(row => row.code === operationalNegative.code),
    'unavailable executable is not a real internal checker failure');
  assert.notEqual(hostiles.find(row => row.name === 'proof-timeout')?.outcome, 'proved');
  return { ...caseRow(id, positive, hostiles, { unchecked_use_cycles: 0,
    operational_negative: operationalNegative,
    ...(internalFaultGap ? { internal_checker_failure_gap: internalFaultGap } : {}) }),
    passed: internalFaultGap === null };
}

export function dependentCase(context, peer) {
  const id = 'CONTRACT-25', base = verify(context, id, 'baseline', 'baseline');
  const positive = [accepted(base, sourceEntry(context, id, 'baseline', 'baseline'),
    proofName[id], 'polymorphic-reflexivity', context)];
  for (const [name, proofName] of [['apply-proof-to-valid-List-I64-code', 'list-refl'],
    ['subst-with-typed-motive-and-checked-premise', 'subst-ok']]) {
    const observed = verify(context, id, 'variants', name);
    positive.push(accepted(observed, sourceEntry(context, id, name), proofName, name,
      context, name.startsWith('apply') ? 2 : 1));
  }
  const exporter = verify(context, id, 'extras', 'accepted-exported-proof');
  accepted(exporter, sourceEntry(context, id, 'accepted-exported-proof', 'extras'),
    'poly-refl', 'exported-immutable-proof', context);
  const dependencies = ['accepted-exported-proof', 'import-version-1']
    .map(name => sourcePath(context, id, name, 'extras'));
  const imported = verify(context, id, 'extras', 'accepted-imported-use', { modules: dependencies });
  positive.push(accepted(imported, sourceEntry(context, id, 'accepted-imported-use', 'extras'),
    positiveImports[id].imported[1], 'imported-immutable-use', context));
  assert.equal(imported.report.dependencies?.[0]?.module, 'logic',
    'imported proof did not retain immutable owner');
  assert.equal(imported.report.dependencies?.[0]?.version, 1,
    'imported proof did not retain source version');
  const hostiles = [];
  for (const name of context.plan.canonical.cases[id].input.variants.slice(2)) {
    if (name === 'reuse-after-module-rebind') {
      assert.ok(peer?.stale?.old_dependency && peer.stale.stale_checker_callback_called === false,
        'real stale proof transaction peer was omitted');
      hostiles.push({ name, command: peer.command, outcome: 'refused-stale',
        stage: peer.stale.stale_stage, proof_owner: peer.stale.old_dependency });
      continue;
    }
    if (name === 'foreign-private-proof-import') {
      assert.equal(peer?.visibility?.outcome, 'refused-private-import',
        'real private import transaction peer was omitted');
      hostiles.push({ name, command: peer.command, outcome: 'refused-private-import',
        stage: peer.visibility.stage });
      continue;
    }
    const observed = verify(context, id, 'variants', name);
    hostiles.push(refused(observed, sourceEntry(context, id, name), name));
  }
  return caseRow(id, positive, hostiles, { transitive_dependencies_inspected: true,
    binder_capture_avoiding_claimed_by_finite_execution: false });
}
