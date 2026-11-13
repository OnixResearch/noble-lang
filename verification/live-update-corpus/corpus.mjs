/**
 * Shared declaration, derivation and checking code for the bounded live-update corpus (#119).
 *
 * Three layers stay strictly separate and every function here belongs to exactly one:
 *
 *   declaration   `SCENARIOS` and its `expectations` table. Frozen text/values chosen before any
 *                 measurement: an expectation is either quoted from canonical design text or
 *                 derived arithmetically from a fixture the exporter declares. No expected value
 *                 is ever read back from a lane report.
 *   derivation    `observe`/`effectRows`/`editSummary`: pure functions of the retained raw lane
 *                 bytes. They produce the normalized observation. Running them twice on the same
 *                 bytes must produce the same object; `verifyEvidence` re-runs them.
 *   classification  `classifyScenario` and `checkClaim`: map declared-versus-observed rows onto the
 *                 seven result classes. A mismatch is never silently absorbed.
 *
 * Nothing here promotes a conformance case, claims model-to-implementation correspondence or
 * treats a comparison-runtime outcome as a violation of Noble's profile or vice versa.
 */
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

export const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
export const CORPUS_DIRECTORY = 'verification/live-update-corpus';
export const PROFILE_PATH = `${CORPUS_DIRECTORY}/profiles/noble.json`;
export const EVIDENCE_DIRECTORY = `${CORPUS_DIRECTORY}/evidence`;
export const PROFILE_ID = 'noble-live-update-profile/noble-v1';
export const SCHEMA = 'noble-live-update-corpus/v1';
export const RECEIPT_SCHEMA = 'noble-live-update-corpus-receipt/v1';
export const CORPUS_REVISION = '1';
export const TESTS = 'crates/noble-cli/tests';
export const CANONICAL_CASES = 'specs/conformance/live-reference-cases.json';
export const SELECTION_COMMAND = 'noble live slot --engine v8 --authority <authority.json>';

export const RESULT_CLASSES = Object.freeze([
  { id: 'contract-violation',
    meaning: 'an observation contradicts a requirement of the runtime profile the corpus froze' },
  { id: 'intentional-semantic-difference',
    meaning: 'the outcome is admissible under that runtime\'s own profile and differs from the other runtime by design; it is a defect in neither runtime' },
  { id: 'supported-outcome',
    meaning: 'every declared expectation holds and the lane reproduced its observation' },
  { id: 'unexpected-refusal',
    meaning: 'a case the profile declares admissible was refused before or during execution' },
  { id: 'runtime-failure',
    meaning: 'a lane process failed, produced no report, or the selected CLI exited abnormally where the case expects a live session' },
  { id: 'unsupported',
    meaning: 'the case cannot be represented faithfully in this runtime or corpus revision; the reason stays visible' },
  { id: 'not-run',
    meaning: 'the case was not executed against that runtime; the reason stays visible' },
]);

export const RESULT_CLASS_IDS = RESULT_CLASSES.map(row => row.id);

export const OTHER_RUNTIME = Object.freeze({
  name: 'makepad-relax',
  reason: 'compiler-lab #188 (Relax host), #189 (temporal admission) and #190 (persistent sessions) are not available; no Relax profile is frozen, so the Relax column is not-run',
});

export const RUNTIME_SOURCES = Object.freeze([
  { path: 'crates/noble-cli/src/core/runtime/slot-registry.mjs',
    what: 'publication CAS, pins, retention and retirement' },
  { path: 'crates/noble-cli/src/core/runtime/slot-engine.mjs',
    what: 'root and frame pins, saved owners, module sweep' },
  { path: 'crates/noble-cli/src/core/runtime/slot-policy.mjs',
    what: 'bounded slot Wasm import/export policy' },
]);

export const LANES = Object.freeze({
  nested: { path: `${TESTS}/live_slot_nested_roots.mjs`, runner: `${TESTS}/live_slot.rs` },
  coherent: { path: `${TESTS}/live_slot_coherent_maps.mjs`,
    runner: `${TESTS}/live_slot_coherent_maps.rs` },
  relocation: { path: `${TESTS}/live_slot_relocation_witness.mjs`,
    runner: `${TESTS}/live_slot_relocation_witness.rs` },
  typed: { path: `${TESTS}/live_slot_typed_cases.mjs`, runner: `${TESTS}/live_slot_typed_cases.rs` },
  ownership: { path: `${TESTS}/live_slot_ownership_cases.mjs`,
    runner: `${TESTS}/live_slot_direct_quote.rs` },
  packageRetention: { path: `${TESTS}/live_slot_package_retention.mjs`,
    runner: `${TESTS}/live_slot_direct_quote.rs` },
  moduleRoot: { path: `${TESTS}/live_slot_module_root_retention.mjs`,
    runner: `${TESTS}/live_slot_direct_quote.rs` },
  capacity: { path: `${TESTS}/live_slot_program_capacity.mjs`, runner: `${TESTS}/live_slot.rs`,
    reportKind: 'text' },
  helper: { path: `${TESTS}/live_slot_resource_ref_cli.mjs`, runner: null },
});

export const sha256 = bytes => createHash('sha256').update(bytes).digest('hex');
export const fileSha256 = (root, relative) => sha256(fs.readFileSync(path.join(root, relative)));
export const readJson = file => JSON.parse(fs.readFileSync(file, 'utf8'));

/** Deterministic JSON: recursive key order, no whitespace. Used for all digests and compares. */
export function stableJson(value) {
  const order = input => {
    if (Array.isArray(input)) return input.map(order);
    if (input !== null && typeof input === 'object')
      return Object.fromEntries(Object.keys(input).sort().map(key => [key, order(input[key])]));
    return input;
  };
  return JSON.stringify(order(value));
}
export const digestValue = value => sha256(stableJson(value));

// ---------------------------------------------------------------------------------------------
// Raw lane reports
// ---------------------------------------------------------------------------------------------

/**
 * Lane stdout is one of three shapes: a single JSON object (coherent maps, ownership, relocation),
 * one JSON object per line plus a summary object (nested roots, package and module retention) or a
 * human sentence produced after the lane's own assertions (program capacity). All three are
 * reported as such; a lane is never assumed to print JSON.
 */
export function parseLaneStdout(text) {
  const trimmed = text.trim();
  if (trimmed === '') return { kind: 'empty', value: null };
  try {
    return { kind: 'json', value: JSON.parse(trimmed) };
  } catch { /* fall through to per-line JSON */ }
  const lines = trimmed.split('\n').filter(line => line.trim() !== '');
  const rows = [];
  for (const line of lines) {
    try {
      rows.push(JSON.parse(line));
    } catch {
      return { kind: 'text', value: trimmed };
    }
  }
  return { kind: 'jsonl', value: rows };
}

/** Parsed stdout of a lane that must produce JSON; a text or empty report is a runtime failure. */
export function structured(report, label) {
  assert.ok(report && (report.kind === 'json' || report.kind === 'jsonl'),
    `${label} did not produce a structured report`);
  return report.value;
}

const laneRun = (bundle, id) => {
  const run = bundle.lanes.get(id);
  assert.ok(run, `scenario lane ${id} is missing from the bundle`);
  return run;
};
export const hasExpectedReport = (id, kind) => LANES[id].reportKind === 'text'
  ? kind === 'text' : kind === 'json' || kind === 'jsonl';
const rowsOf = (map, variant) => map.get(variant) ?? [];
const operations = rows => rows.map(row => (row && typeof row.operation === 'string')
  ? row.operation : 'unknown');

const dispatchRows = trace => trace.filter(row => row.operation === 'dispatch');
const effectRows = trace => trace.filter(row => row.operation === 'effect');
/**
 * A *distinguished* effect row carries both its request and its response. A denied effect leaves a
 * row with a request and no response, so counting raw effect rows would silently count a denied
 * effect as a performed one.
 */
const distinguishedRows = trace => effectRows(trace).filter(row =>
  row.request !== undefined && row.request !== null
  && row.response !== undefined && row.response !== null);

/** One trace row reduced to the fields the corpus compares. */
const traceEntry = (row, ordinal) => ({
  ordinal,
  operation: row.operation ?? null,
  slot: row.policyInput?.slotId ?? row.slotId ?? null,
  generation: row.generation ?? null,
  incarnation: row.incarnation ?? null,
  definitionId: row.definitionId ?? null,
  rootEpoch: row.rootEpoch ?? row.policyInput?.rootEpoch ?? null,
  request: row.request ?? null,
  response: row.response ?? null,
});
const traceSummary = trace => trace.map(traceEntry);
const slotSequence = trace => dispatchRows(trace).map(row => row.policyInput?.slotId ?? null);
const generationSequence = trace => dispatchRows(trace).map(row => row.generation ?? null);
const epochSet = trace => [...new Set(dispatchRows(trace).map(row => row.rootEpoch))];

const lastExecuted = rows => rows.filter(row => row.outcome === 'executed'
  && Array.isArray(row.request_trace)).at(-1) ?? null;

/** Identity rows of the dispatch trace: selected version, generation, origin. */
const selectedIdentities = (subject, trace) => dispatchRows(trace).map((row, ordinal) => ({
  subject: `${subject}#${ordinal}`,
  slot: row.policyInput?.slotId ?? null,
  generation: row.generation ?? null,
  incarnation: row.incarnation ?? null,
  programValueId: row.programValueId ?? null,
  artifactSha256: row.artifactSha256 ?? null,
  rootEpoch: row.rootEpoch ?? null,
}));

const editSummary = (bundle, laneId) => {
  const run = laneRun(bundle, laneId);
  return [...run.requests.keys()].sort((a, b) => a.localeCompare(b, 'en')).map(variant => {
    const rows = run.requests.get(variant) ?? [];
    const bytes = run.requestBytes.get(variant) ?? Buffer.alloc(0);
    return {
      variant,
      count: rows.length,
      operations: operations(rows),
      requests_sha256: sha256(bytes),
    };
  });
};

// ---------------------------------------------------------------------------------------------
// Scenario declarations
// ---------------------------------------------------------------------------------------------

const canonical = (id, requirement, lines = null) => ({
  file: CANONICAL_CASES, id, ...(lines === null ? {} : { lines }), requirement,
});
const design = (id, lines, requirement) => ({
  file: '.cairn/changes/live-reference-lifetimes/design.md', id, lines, requirement,
});
const ownership = (id, lines, requirement) => ({
  file: '.cairn/changes/ownership-lifetime-contract/design.md', id, lines, requirement,
});

/** Marker used by expectations the profile cannot represent; the reason stays in the corpus. */
const unsupported = reason => ({ unsupported: reason });

// --- Scenario A: held root, nested replacement and a new root ---------------------------------

function observeHeldRoot(bundle) {
  const nested = structured(laneRun(bundle, 'nested').report, 'nested-roots');
  const results = Array.isArray(nested) ? nested : nested.results;
  const named = name => {
    const found = results.find(row => row.name === name);
    assert.ok(found, `nested-roots report is missing ${name}`);
    return found;
  };
  const root = value => ({
    epoch: value.epoch,
    stack: value.stack,
    slots: slotSequence(value.request_trace),
    generations: generationSequence(value.request_trace),
    rootEpochs: epochSet(value.request_trace),
    dispatches: dispatchRows(value.request_trace).length,
    effects: effectRows(value.request_trace).length,
    trace: traceSummary(value.request_trace),
  });
  const base = named('base-parent-child');
  const transitive = named('transitive-A-B-C');
  const budget = named('later-member-budget-refusal');
  const coherent = structured(laneRun(bundle, 'coherent').report, 'coherent-maps');
  const coherentRows = rowsOf(laneRun(bundle, 'coherent').responses, 'coherent-caller-first')
    .filter(row => row.outcome === 'executed' && Array.isArray(row.request_trace));
  const heldTrace = coherentRows.at(1)?.request_trace ?? [];
  const freshTrace = coherentRows.at(2)?.request_trace ?? [];
  const pinnedEffect = effectRows(heldTrace).at(0) ?? null;
  const freshEffect = effectRows(freshTrace).at(0) ?? null;
  return {
    nested: {
      base: {
        name: base.name,
        status: base.status,
        initial_epoch: base.initial_epoch,
        committed_epoch: base.committed_epoch,
        installed_sha256: base.installed_sha256,
        checkpoint: base.checkpoint,
        ack: base.committed_ack,
        old_root: root(base.old_root),
        new_root: root(base.new_root),
        next_leaf: root(base.next_leaf),
        control_events: base.old_root.control_events,
      },
      transitive: {
        name: transitive.name,
        initial_epoch: transitive.initial_epoch,
        committed_epoch: transitive.committed_epoch,
        old_root: root(transitive.old_root),
        new_root: root(transitive.new_root),
        next_leaf: root(transitive.next_leaf),
      },
      budget_refusal: {
        outcome: budget.refusal.outcome,
        epoch: budget.refusal.epoch,
        guest_requests: budget.refusal.guest_requests,
        probes: budget.map_absence.map(probe => ({ slot: probe.slot, outcome: probe.outcome })),
        repeat_outcome: budget.repeat_refusal.outcome,
        trace_rows: Array.isArray(budget.trace.trace) ? budget.trace.trace.length : null,
      },
    },
    coherent: {
      leaf_first_counts: coherent.leafFirstCounts,
      caller_first_counts: coherent.callerFirstCounts,
      pinned_epoch: coherent.pinnedEpoch,
      published_epoch: coherent.publishedEpoch,
      publication_outcome: coherent.heldPublicationOutcome,
      held_root_counts: coherent.heldRootCounts,
      fresh_root_counts: coherent.freshRootCounts,
      held_definition: coherent.heldRootDefinition,
      fresh_definition: coherent.freshRootDefinition,
      held_root_epochs: epochSet(heldTrace),
      fresh_root_epochs: epochSet(freshTrace),
      held_effect_definition: pinnedEffect?.definitionId ?? null,
      fresh_effect_definition: freshEffect?.definitionId ?? null,
      without_compare_and_set: coherent.withoutCompareAndSetOutcome,
      half_published: {
        outcome: coherent.halfPublishedOutcome,
        rows: coherent.halfPublishedRows,
        session_state: coherent.halfPublishedSessionState,
      },
      omission_counts: coherent.omissionCounts,
    },
  };
}

const heldRootEffectRows = bundle => ({
  normal: [
    { subject: 'nested.base.old-root', purpose: 'held root selects the pinned version',
      rows: traceSummary(rowsOf(laneRun(bundle, 'nested').responses, 'LSLOT02-base-parent-child')
        .find(row => row.outcome === 'executed')?.request_trace ?? []) },
    { subject: 'nested.base.new-root', purpose: 'new root selects the replacement',
      rows: (() => {
        const executed = rowsOf(laneRun(bundle, 'nested').responses,
          'LSLOT02-base-parent-child').filter(row => row.outcome === 'executed');
        return traceSummary(executed.at(1)?.request_trace ?? []);
      })() },
    { subject: 'coherent.held-root', purpose: 'publication lands during a held root',
      rows: traceSummary(rowsOf(laneRun(bundle, 'coherent').responses, 'coherent-caller-first')
        .filter(row => row.outcome === 'executed').at(1)?.request_trace ?? []) },
  ].filter(entry => entry.rows.length > 0),
  abnormal: [
    { subject: 'nested.budget-refusal', reason: 'bootstrap refused before execution; no root ran',
      rows: [] },
    { subject: 'coherent.half-published', reason: 'session terminated after a guest failure',
      rows: [] },
    { subject: 'coherent.omission', reason: 'dispatch kept, no effect row',
      rows: traceSummary(rowsOf(laneRun(bundle, 'coherent').responses, 'coherent-omission')
        .filter(row => row.outcome === 'executed').at(-1)?.request_trace ?? []) },
  ],
});

function expectation(id, options) {
  return {
    expectation_id: id,
    origin: options.origin,
    claim: options.claim ?? 'equals',
    declared: options.declared,
    extract: options.extract,
    mismatch_class: options.mismatch_class ?? 'contract-violation',
    mapping: options.mapping,
  };
}
const fromCanonical = (ref, text) => ({ kind: 'canonical', ref, text });
const fromTable = (ref, text) => ({ kind: 'exporter-table', ref, text });
const fromOwnership = (ref, text) => ({
  kind: 'canonical',
  ref: `.cairn/changes/ownership-lifetime-contract/design.md#${ref}`,
  text,
});

// ---------------------------------------------------------------------------------------------
// The declared scenarios. Every declared number below is derived in the comment beside it from a
// fixture the lane declares; none was read back from a report.
// ---------------------------------------------------------------------------------------------

export const SCENARIOS = [
  {
    case_id: 'noble-live-update/held-root-and-new-root',
    title: 'Held root, nested replacement and a new root under one pinned map',
    bounded: {
      statement: 'Two sessions; at most three slots, two publications, four roots and one protected checkpoint hold each',
      limits: { sessions: 2, slots: 4, publications: 2, roots: 4, checkpoints: 1, invocations: 6 },
    },
    canonical_refs: [canonical('LSLOT-02', 'S-LSLOT-01'), canonical('LSLOT-04', 'H-LSLOT-01'),
      design('LREF-CASE-06', '129', 'LREF-CASE-06')],
    profile_refs: ['dimensions.version_selection_boundary', 'dimensions.active_frames',
      'dimensions.update_group_atomicity'],
    lanes: ['nested', 'coherent'],
    observe: observeHeldRoot,
    effectRows: heldRootEffectRows,
    mapping: 'root R pins one epoch and one map; every nested dispatch reports that epoch and the selected generation; a root started after the publication reports the new epoch only; a publication that lands during a held root is acknowledged and recorded as a control event without changing the held root\'s selections',
    unsupported: ['a single invocation routing to two emitting slots cannot be built from this authority shape', 'two host threads sharing one registry'],
    expectations: [
      expectation('held-root.old-root-selections', {
        origin: fromCanonical('LSLOT-02#expected.base_root_A_B_selections',
          'the held root selects the pinned version for every nested dispatch'),
        // Two dispatches to B from the A1 caller plus the root's own dispatch to A.
        declared: { slots: ['A', 'B', 'B'], rootEpochs: ['1'], generations: ['1', '1', '1'] },
        extract: observed => ({
          slots: observed.nested.base.old_root.slots,
          rootEpochs: observed.nested.base.old_root.rootEpochs,
          generations: observed.nested.base.old_root.generations,
        }),
        mapping: 'the held root\'s dispatch trace names A once and the pinned leaf B twice, all in the epoch pinned before the publication',
      }),
      expectation('held-root.old-root-result', {
        origin: fromCanonical('LSLOT-02#expected.base_root_A_result',
          'base root A result with B-v1 selected twice'),
        // 20 + 1 + 1: the caller forwards 20, A1 invokes B (1 +) twice.
        declared: [{ kind: 1, value: '22' }],
        extract: observed => observed.nested.base.old_root.stack,
        mismatch_class: 'unexpected-refusal',
        mapping: 'the held root must complete with the pinned version\'s arithmetic result',
      }),
      expectation('held-root.new-root-selections', {
        origin: fromCanonical('LSLOT-02#expected.base_next_root_B_selection',
          'a later root selects the published version'),
        declared: { slots: ['A', 'B', 'B'], rootEpochs: ['2'], generations: ['1', '2', '2'] },
        extract: observed => ({
          slots: observed.nested.base.new_root.slots,
          rootEpochs: observed.nested.base.new_root.rootEpochs,
          generations: observed.nested.base.new_root.generations,
        }),
        mapping: 'the root started after the publication keeps its own single new epoch and selects B generation 2',
      }),
      expectation('held-root.new-root-result', {
        origin: fromTable('exporter-table#base-new-root',
          '20 + 2 + 2: the replacement B2 body is 2 +'),
        declared: [{ kind: 1, value: '24' }],
        extract: observed => observed.nested.base.new_root.stack,
        mismatch_class: 'unexpected-refusal',
        mapping: "the new root's result follows from the replacement body, not from the held root",
      }),
      expectation('held-root.next-leaf-result', {
        origin: fromCanonical('LSLOT-02#expected.base_next_root_B_result',
          'a direct root after the publication selects the new version'),
        // 20 + 2: a direct leaf root on B2.
        declared: [{ kind: 1, value: '22' }],
        extract: observed => observed.nested.base.next_leaf.stack,
        mismatch_class: 'unexpected-refusal',
        mapping: 'a direct leaf root uses the same new version the next root selected',
      }),
      expectation('held-root.coherent-publication-during-hold', {
        origin: fromTable('exporter-table#held-publication',
          'a publication that lands during a held root does not change that root\'s map'),
        declared: { publication: 'published', held: { dispatches: 1, effects: 1 },
          fresh: { dispatches: 1, effects: 1 }, epochsDiffer: true },
        extract: observed => ({
          publication: observed.coherent.publication_outcome,
          held: observed.coherent.held_root_counts,
          fresh: observed.coherent.fresh_root_counts,
          epochsDiffer: observed.coherent.pinned_epoch !== observed.coherent.published_epoch,
        }),
        mapping: 'the held root dispatches once and emits once from the pinned map; the next root does the same from the new one',
      }),
      expectation('held-root.held-root-keeps-pinned-definition', {
        origin: fromCanonical('LSLOT-02#expected.mixed_registry_epochs_within_root',
          'no root mixes registry epochs; the effect row names the definition the dispatch selected'),
        declared: { heldEqualsPinned: true, freshDiffersFromHeld: true,
          heldEpochs: ['2'], freshEpochs: ['3'] },
        extract: observed => ({
          heldEqualsPinned: observed.coherent.held_effect_definition
            === observed.coherent.held_definition,
          freshDiffersFromHeld: observed.coherent.fresh_effect_definition
            !== observed.coherent.held_effect_definition,
          heldEpochs: observed.coherent.held_root_epochs,
          freshEpochs: observed.coherent.fresh_root_epochs,
        }),
        mapping: 'each root\'s dispatch and effect rows share one pinned epoch and one selected definition',
      }),
      expectation('held-root.stale-compare-and-set-control', {
        origin: fromCanonical('LSLOT-04#expected.stale_same_or_unrelated_writer_publishes',
          'a publication without the incarnation/generation compare-and-set fields stale-rejects'),
        declared: 'stale-reject',
        extract: observed => observed.coherent.without_compare_and_set,
        mapping: 'control: the refusal shape proves the compare-and-set fields are required whether or not a root is pinned',
      }),
      expectation('held-root.partial-publication-refusal', {
        origin: fromCanonical('LSLOT-02#expected.one-map-per-root-with-explicit-typed-ref-pass-through',
          'a root cannot mix maps: an unpublished target terminates the root'),
        declared: { outcome: 'execution-failed', rows: 0,
          session_state: 'terminated-after-guest-failure' },
        extract: observed => observed.coherent.half_published,
        mismatch_class: 'unexpected-refusal',
        mapping: 'a root attempted between two publications produces no trace rows and terminates the session',
      }),
      expectation('held-root.omission-control', {
        origin: fromTable('exporter-table#omission',
          'an inert target keeps the dispatch and performs no effect'),
        declared: { dispatches: 1, effects: 0 },
        extract: observed => observed.coherent.omission_counts,
        mapping: 'control separating omission from coherence: dispatch present, effect absent',
      }),
      expectation('held-root.retention-budget-refusal', {
        origin: fromTable('exporter-table#later-member-budget',
          'a two-member bootstrap under quota 1 is refused before publishing either member'),
        declared: { outcome: 'retention-budget-refused', epoch: '0', guest_requests: 0,
          probes: [{ slot: 'B', outcome: 'stale-reject' }, { slot: 'A', outcome: 'stale-reject' }],
          repeat_outcome: 'retention-budget-refused', trace_rows: 0 },
        extract: observed => observed.nested.budget_refusal,
        mapping: 'the quota is checked against the whole post-commit map, so neither member is published',
      }),
      expectation('held-root.transitive-selections', {
        origin: fromCanonical('LSLOT-02#expected.variant_old_root_A_B_C_selections',
          'a transitive victim is selected once, from the root\'s pinned map'),
        declared: { old: ['A', 'B', 'C'], oldGenerations: ['1', '1', '1'],
          new: ['A', 'B', 'C'], newGenerations: ['1', '1', '2'],
          oldEpochs: ['1'], newEpochs: ['2'] },
        extract: observed => ({
          old: observed.nested.transitive.old_root.slots,
          oldGenerations: observed.nested.transitive.old_root.generations,
          new: observed.nested.transitive.new_root.slots,
          newGenerations: observed.nested.transitive.new_root.generations,
          oldEpochs: observed.nested.transitive.old_root.rootEpochs,
          newEpochs: observed.nested.transitive.new_root.rootEpochs,
        }),
        mapping: 'A and B are unchanged between the two maps; only the transitive C selection moves from generation 1 to 2',
      }),
      expectation('held-root.transitive-results', {
        origin: fromCanonical('LSLOT-02#expected.variant_old_root_result',
          'the held transitive root completes on its pinned C while the new root uses C-v2'),
        // 20 + 1 = 21 with C1, then 20 + 2 = 22 with C2 for both the resumed caller and a direct leaf.
        declared: { old: [{ kind: 1, value: '21' }], new: [{ kind: 1, value: '22' }],
          leaf: [{ kind: 1, value: '22' }] },
        extract: observed => ({
          old: observed.nested.transitive.old_root.stack,
          new: observed.nested.transitive.new_root.stack,
          leaf: observed.nested.transitive.next_leaf.stack,
        }),
        mismatch_class: 'unexpected-refusal',
        mapping: 'the two roots\' results differ only through the version their own map selected',
      }),
    ],
    relax: {
      anticipated_differences: [
        { dimension: 'version-selection-boundary',
          noble: 'one immutable map pinned per root for all nested dispatch (S-LSLOT-01)',
          relax: 'reload is a source-edit transaction with explicit Refuse/Patch/Restart outcomes; no root-scoped map is implied',
          classification_rule: 'a Relax Refuse, Patch or Restart is admissible under the Relax profile and is an intentional semantic difference, never a Noble contract violation nor a Relax defect' },
        { dimension: 'held-root-mixing',
          noble: 'a root started before a publication keeps its pinned version',
          relax: 'a Patch may take effect in already-created state; whether an active call keeps the old code is a Relax-profile question that must be declared before measuring it',
          classification_rule: 'a difference here is recorded as intentional until the Relax profile is frozen and quoted' },
      ],
    },
  },
  {
    case_id: 'noble-live-update/saved-program-and-package-retention',
    title: 'Saved Program, package retention and the retained-version quota',
    bounded: {
      statement: 'Three sessions: two replacement sessions with quota 2 and 1, one module-root session, and one quota-saturated ledger session of 4096 owners',
      limits: { sessions: 4, slots: 2, publications: 3, saved_owners: 4096, quota: 4096 },
    },
    canonical_refs: [design('LREF-CASE-03', '126', 'LREF-PKG-01'),
      ownership('OWN-CASE-06', '193', 'OWN-PKG-01'),
      canonical('LSLOT-08', 'H-LSLOT-01')],
    profile_refs: ['dimensions.saved_callable_values', 'dimensions.reclamation',
      'dimensions.update_group_atomicity'],
    lanes: ['packageRetention', 'moduleRoot', 'capacity'],
    observe: observeRetention,
    effectRows: retentionEffectRows,
    mapping: 'saved Program O holds its own package: O runs its captured code after the slot is replaced, the superseding publication retires no span of M while O lives, releasing O retires the compiler span and reclaims the charge, and a quota-1 ledger refuses the publication while O holds the only retained unit',
    unsupported: ['cross-process persistence of the registry', 'a multi-slot atomic publication group'],
    expectations: [
      expectation('retention.replacement-runs-old-code', {
        origin: fromCanonical('LREF-CASE-03#expected.20-O-run-yields-21',
          'a saved exact Program runs its own captured code after the slot is replaced'),
        declared: [{ kind: 1, value: '21' }],
        extract: observed => observed.retention.replacement.run_after_replacement.stack,
        mismatch_class: 'unexpected-refusal',
        mapping: 'the saved Program is invoked with 20 through a separate runner module; LREF-CASE-03 expects 21',
      }),
      expectation('retention.no-retirement-while-owned', {
        origin: fromCanonical('LREF-CASE-03#expected.span-stays-live',
          'the retired span is not cleared while the saved owner reaches it'),
        declared: [],
        extract: observed => observed.retention.replacement.publication_while_owned.retire_spans,
        mapping: 'the publication that supersedes v1 must retire no span of M1 while O is a saved owner',
      }),
      expectation('retention.retirement-after-last-owner', {
        origin: fromCanonical('LREF-CASE-03#expected.span-retires',
          'releasing the last owner retires the span and reclaims the charge'),
        declared: { spans: 1, generation: 1, start: 4, length: 5 },
        extract: observed => {
          const spans = observed.retention.replacement.release.retire_spans;
          return { spans: spans.length, ...(spans[0] ?? {}) };
        },
        mapping: 'release-program returns the code span M1 occupied; the generation, start and length are the compiler\'s, recorded not derived',
      }),
      expectation('retention.released-program-refused', {
        origin: fromCanonical('LREF-CASE-03#expected.no-stale-handle-trap',
          'a released owner is refused rather than trapping'),
        declared: { executed: false, outcome: 'refused' },
        extract: observed => {
          const row = observed.retention.replacement.run_after_release;
          return { executed: row.outcome === 'executed', outcome: row.outcome };
        },
        mapping: 'after release the same owner must not execute; a stale-handle trap would fail the case',
      }),
      expectation('retention.charge-refuses-while-owned', {
        origin: fromCanonical('LREF-CASE-03#expected.charge-reclaimed',
          'a retained-version quota of 1 must not admit v2 while O holds v1\'s package'),
        declared: { outcome: 'retention-budget-refused', epoch: '1' },
        extract: observed => ({
          outcome: observed.retention.charge.publication_while_owned.outcome,
          epoch: observed.retention.charge.publication_while_owned.epoch,
        }),
        mapping: 'capacity admission counts post-commit reachability, so O\'s package occupies the only unit',
      }),
      expectation('retention.charge-admits-after-release', {
        origin: fromCanonical('LREF-CASE-03#expected.charge-reclaimed',
          'the publication after the release retires M1 and reclaims the retained-version charge'),
        declared: { outcome: 'published', spans: 1 },
        extract: observed => ({
          outcome: observed.retention.charge.publication_after_release.outcome,
          spans: observed.retention.charge.publication_after_release.retire_spans.length,
        }),
        mapping: 'the same publication succeeds once the saved owner is gone and retires the span',
      }),
      expectation('retention.module-root-survives-supersession', {
        origin: fromOwnership('OWN-CASE-06#expected-saved-named-caller',
          'either the invocation returns the correct value, or retirement is refused while the owner lives; a stale-handle trap is a failure'),
        declared: { outcome: 'executed', stack: [{ kind: 1, value: '42' }] },
        extract: observed => ({
          outcome: observed.module_root.after_supersession.outcome,
          stack: observed.module_root.after_supersession.stack,
        }),
        mismatch_class: 'unexpected-refusal',
        mapping: 'the saved Program calls g, whose body reads h through M\'s module global, after M\'s root was superseded',
      }),
      expectation('retention.module-root-retires-at-zero-owners', {
        origin: fromOwnership('OWN-CASE-06#expected-release-order',
          'M\'s code retires only after the last saved owner is released'),
        declared: { spans: 1, generation: 1, start: 4, length: 6 },
        extract: observed => {
          const spans = observed.module_root.after_release.retire_spans;
          return { spans: spans.length, ...(spans[0] ?? {}) };
        },
        mapping: 'the module-root session records one span of M, the compiler\'s own start and length',
      }),
      expectation('retention.module-root-last-run-refused', {
        origin: fromOwnership('OWN-CASE-06#expected-refusal-after-release',
          'a released owner is refused'),
        declared: 'refused',
        extract: observed => observed.module_root.final_run.outcome,
        mapping: 'the final invocation after release must be refused, not trapped',
      }),
      expectation('retention.saved-program-capacity-boundary', {
        origin: fromTable('exporter-table#saved-program-ledger',
          'the reviewed host ledger admits 4096 saved owners and refuses the 4097th root before execution'),
        // 4096 successful invokes + 1 refused + 1 admitted after a release = 4098 invoke requests.
        declared: { capacity: 4096, summary: 'saved Program capacity 4096 refused the over-budget root before execution',
          requests: { install: 1, publish: 1, invoke: 4098, 'release-program': 1 } },
        extract: observed => ({
          capacity: observed.capacity.request_counts.invoke - 2,
          summary: observed.capacity.summary_line,
          requests: observed.capacity.request_counts,
        }),
        mapping: 'the ledger bound is the lane\'s declared constant; the request sequence it produces is fixed by that constant',
      }),
    ],
    relax: {
      anticipated_differences: [
        { dimension: 'retention-unit',
          noble: 'the retained unit is the selected version\'s whole executable package, with a host-ledger quota in 1..16384',
          relax: 'retained code follows the reload transaction\'s own retained-code policy; no comparable quota is implied',
          classification_rule: 'a differing reclamation bound is an intentional semantic difference; only a stale handle that traps would be a failure under LREF-CASE-03' },
      ],
    },
  },
  {
    case_id: 'noble-live-update/relocation-and-effect-prefix',
    title: 'Event relocation under a pinned map and the abnormal effect prefix',
    bounded: {
      statement: 'Four sessions, one invocation each: old target, relocated target, inert target and a withdrawn effect grant',
      limits: { sessions: 4, slots: 4, publications: 2, invocations: 4 },
    },
    canonical_refs: [canonical('LSLOT-07', 'S-LSLOT-01'), canonical('LSLOT-08', 'DX-LSLOT-01'),
      design('LREF-CASE-01', '124', 'LREF-EXIT-01')],
    profile_refs: ['dimensions.active_frames', 'dimensions.version_selection_boundary'],
    lanes: ['relocation'],
    observe: observeRelocation,
    effectRows: relocationEffectRows,
    mapping: 'the request trace records the dispatch that selected the target and the effect performed for it; when the grant is withdrawn the dispatch and the effect request stay in the record and no effect response is produced',
    unsupported: ['a duplicate-vs-omission ratio for one invocation routing to two emitting slots'],
    expectations: [
      expectation('relocation.effect-moves-with-target', {
        origin: fromTable('exporter-table#relocation-witness',
          'the effect row names the checked target slot; republishing the forwarded reference moves it'),
        declared: { oldSlot: 'A', newSlot: 'E', definitionsDiffer: true },
        extract: observed => ({
          oldSlot: observed.relocation.old_slot,
          newSlot: observed.relocation.new_slot,
          definitionsDiffer: observed.relocation.old_definition !== observed.relocation.new_definition,
        }),
        mapping: 'the request and response are identical on both sides; only the selected target and its DefinitionId move',
      }),
      expectation('relocation.one-effect-per-root', {
        origin: fromCanonical('LSLOT-07#expected.already_completed_prior_effects_rolled_back',
          'a dispatch performs its effect once and no rollback erases it'),
        declared: { oldEffects: 1, newEffects: 1, oldRequest: 'relocated', newRequest: 'relocated',
          oldResponse: 'emitted:relocated', newResponse: 'emitted:relocated' },
        extract: observed => ({
          oldEffects: observed.relocation.old_effects,
          newEffects: observed.relocation.new_effects,
          oldRequest: observed.relocation.old_request,
          newRequest: observed.relocation.new_request,
          oldResponse: observed.relocation.old_response,
          newResponse: observed.relocation.new_response,
        }),
        mapping: 'exactly one distinguished effect row on each side, carrying the same request and response',
      }),
      expectation('relocation.dispatch-precedes-effect', {
        origin: fromCanonical('LSLOT-07#expected.revoked_effect_request_recorded',
          'a committed dispatch prefix is preserved rather than rolled back'),
        declared: { oldFirst: 'dispatch', newFirst: 'dispatch' },
        extract: observed => ({ oldFirst: observed.relocation.old_first, newFirst: observed.relocation.new_first }),
        mapping: 'the trace begins with the dispatch that precedes the effect; a denial before the emitting hop keeps the prefix',
      }),
      expectation('relocation.omission-control', {
        origin: fromTable('exporter-table#selection-omission-control',
          'an inert target keeps the dispatch and performs no effect'),
        declared: { dispatches: 1, effects: 0 },
        extract: observed => observed.relocation.omission,
        mapping: 'the omission control separates a coherent event from a missing one',
      }),
      expectation('relocation.denied-effect', {
        origin: fromCanonical('LSLOT-07#expected.revoked_effect_protected_operations',
          'with the effect grant withdrawn the same invocation records the dispatch but performs no effect'),
        declared: { executed: false, responses: 0, dispatchKept: true,
          effectRequest: 'relocated', outcome: 'execution-failed' },
        extract: observed => observed.relocation.denied,
        mismatch_class: 'unexpected-refusal',
        mapping: 'the guest must not perform the effect; the observed outcome is a guest failure, which the profile records as the abnormal exit',
      }),
      expectation('relocation.duplicate-not-constructible', {
        origin: fromTable('exporter-table#duplicate-control',
          'a single invocation cannot route to two emitting slots in this authority shape'),
        declared: unsupported('the lane cannot bind one invocation to two emitting targets: `slot.invoke` with two borrowed formals refuses `is_ambiguous is_compatible borrowed references`, and `slot.invoke.N` refuses `invalid borrowed input ordinal`'),
        extract: observed => observed.relocation.duplicate_count,
        mapping: 'recorded as unsupported rather than measured; the corpus will not report a duplicate/omission ratio it did not observe',
      }),
    ],
    relax: {
      anticipated_differences: [
        { dimension: 'effect-performed-for',
          noble: 'the host performs the effect for the checked target slot the frame selected',
          relax: 'an effect belongs to whichever reload transaction installed the running code',
          classification_rule: 'a differing effect owner is a mapping question; the corpus will not claim correspondence without a justified Relax mapping' },
      ],
    },
  },
  {
    case_id: 'noble-live-update/typed-cases-and-bounds',
    title: 'Typed generated cases, negative families and the declared host bounds',
    bounded: {
      statement: 'Five generated positive cases plus four negative witnesses, one retention saturation and one type-boundary pair',
      limits: { generated_cases: 5, negative_classes: 4, retained_roots: 128, type_stack: 128, nesting: 31 },
    },
    canonical_refs: [canonical('LSLOT-03', 'H-LSLOT-01'), canonical('LSLOT-05', 'VC-LSLOT-01')],
    profile_refs: ['dimensions.incompatible_layout_behavior', 'dimensions.reclamation'],
    lanes: ['typed'],
    observe: observeTyped,
    effectRows: typedEffectRows,
    mapping: 'each generated case\'s dispatch and effect counts follow from its declared shape, ceiling and repeat; the negative family is refused at the declared stage; the host bounds are the lane\'s declared constants',
    unsupported: ['structural coercion between differing slot interfaces'],
    expectations: [
      expectation('typed.direct-pure-repeat1', {
        origin: fromTable('exporter-table#typed-direct-pure',
          'one root of the direct case with the pure ceiling: one dispatch, no effect'),
        declared: { roots: 1, dispatches: [1], effects: [0] },
        extract: observed => observed.typed.cases.directPureRepeat1,
        mapping: 'the direct caller forwards exactly one reference; the inert leaf declares no effect',
      }),
      expectation('typed.direct-pure-repeat2', {
        origin: fromTable('exporter-table#typed-direct-pure',
          'two invocations of the direct pure case: one dispatch and no effect each'),
        declared: { roots: 2, dispatches: [1, 1], effects: [0, 0] },
        extract: observed => observed.typed.cases.directPureRepeat2,
        mapping: 'the repeat dimension invokes the same root twice; the counts must not accumulate across roots',
      }),
      expectation('typed.direct-emit-repeat1', {
        origin: fromTable('exporter-table#typed-direct-emit',
          'one root of the direct case with test.emit: one dispatch, one effect'),
        declared: { roots: 1, dispatches: [1], effects: [1] },
        extract: observed => observed.typed.cases.directEmitRepeat1,
        mapping: 'the emitting leaf declares test.emit and both slots carry it in their ceiling',
      }),
      expectation('typed.direct-emit-repeat2', {
        origin: fromTable('exporter-table#typed-direct-emit',
          'two invocations of the direct emitting case: one dispatch and one effect each'),
        declared: { roots: 2, dispatches: [1, 1], effects: [1, 1] },
        extract: observed => observed.typed.cases.directEmitRepeat2,
        mapping: 'the effect count is per root, not per session',
      }),
      expectation('typed.nested-chain', {
        origin: fromTable('exporter-table#typed-chain',
          'A invokes B which invokes C: three dispatches, no effect'),
        declared: { roots: 1, dispatches: [3], effects: [0] },
        extract: observed => observed.typed.cases.chain,
        mapping: 'the chain is the only nested positive shape; every hop is a dispatch row',
      }),
      expectation('typed.coverage-totals', {
        origin: fromTable('exporter-table#typed-coverage',
          'the declared case table has 4 direct roots and 1 chain of 3 hops, 3 effect rows in total'),
        // 1+1+1+1 dispatches for the direct repeats 1/2/1/2 -> 6, plus 3 chain hops = 9;
        // effects: one emitting root in each of the two emitting cases = 3.
        declared: { admitted: 5, total_dispatch_rows: 9, total_effect_rows: 3,
          by_shape: { direct: 4, chain: 1 }, by_nesting: { single: 4, nested: 1 },
          by_ceiling: { pure: 2, 'dispatch+emit': 2 },
          by_repeated_reborrow: { once: 2, twice: 2 } },
        extract: observed => observed.typed.coverage,
        mapping: 'counts recomputed from the five per-case declarations above, independent of the lane',
      }),
      expectation('typed.negative-families', {
        origin: fromCanonical('LSLOT-03#expected.only-exact-compatible-candidate-publishes',
          'a candidate or invocation whose shape is not exactly compatible is refused without executing'),
        declared: { refused: 4, classes: ['binding-foreign-root', 'binding-wrong-position',
          'effect-not-granted', 'stack-incompatible'],
        by_stage: { command: 3, slot: 1 }, guest_requests: 2,
        trace: [['dispatch', 'allowed'], ['effect', 'denied']] },
        extract: observed => observed.typed.negative,
        mismatch_class: 'unexpected-refusal',
        mapping: 'three refusals precede the guest; the withheld effect is refused at the slot stage with the guest\'s own denied row',
      }),
      expectation('typed.minimization-is-minimal', {
        origin: fromTable('exporter-table#typed-minimization',
          'each negative class has one minimal witness and the quota shrink cannot be accepted'),
        declared: { minimal_starting_witnesses: 4, accepted_shrinks: 0,
          rejected: [
            { class: 'binding-foreign-root', dimension: 'quota', observed: 'setup-refused' },
            { class: 'binding-wrong-position', dimension: 'quota', observed: 'setup-refused' },
            { class: 'stack-incompatible', dimension: 'quota', observed: 'setup-refused' },
            { class: 'effect-not-granted', dimension: 'quota', observed: 'setup-refused' },
          ] },
        extract: observed => observed.typed.minimization,
        mapping: 'the only declared shrink dimension is the quota; a two-slot map needs quota two, so the shrink is rejected during setup',
      }),
      expectation('typed.retention-saturation', {
        origin: fromTable('exporter-table#typed-retention',
          'the recorded-root retention bound is 128 and the 129th root is refused before execution'),
        declared: { bound: 128, retained: 128, refusal_stage: 'command' },
        extract: observed => ({
          bound: observed.typed.retention.bound,
          retained: observed.typed.retention.retained,
          refusal_stage: observed.typed.retention.refusal_stage,
        }),
        mapping: 'the lane saturates the recorded retention ledger and asserts the refusal stage',
      }),
      expectation('typed.type-boundaries', {
        origin: fromTable('exporter-table#typed-type-boundaries',
          'the declared authority bounds are a 128-type stack and 31 levels of constructor nesting'),
        declared: { stack: { admitted: 128, refused_first: 129 },
          nesting: { admitted: 31, refused_first: 32 }, refusal_stage: 'authority' },
        extract: observed => ({
          stack: observed.typed.type_boundaries.stack,
          nesting: observed.typed.type_boundaries.nesting,
          refusal_stage: observed.typed.type_boundaries.refusal_stage,
        }),
        mapping: 'both bounds are checked as a boundary pair: the last admitted and the first refused',
      }),
    ],
    relax: {
      anticipated_differences: [
        { dimension: 'typed-admission',
          noble: 'admission is an exact ordered interface plus nominal resource schema plus effect ceiling, refused before execution',
          relax: 'a reload transaction\'s compatibility rule is its own; no exact-interface rule is implied',
          classification_rule: 'an admitted Relax reload that Noble would refuse is a profile difference, not a defect, until a faithful mapping is declared' },
      ],
    },
  },
  {
    case_id: 'noble-live-update/ownership-duplicates-and-modules',
    title: 'Shared saved owners, composition ownership and a cross-module alias',
    bounded: {
      statement: 'Five sessions: two release orders for two owners, one balanced 8-leaf compose tree, one left-deep chain to its refusal, one cross-module duplicate',
      limits: { sessions: 5, leaves: 8, intermediates: 7, saved_owners: 18, compose_refusal_at: 5 },
    },
    canonical_refs: [ownership('OWN-CASE-02', '189', 'OWN-RECLAIM-01'),
      ownership('OWN-CASE-03', '190', 'OWN-RECLAIM-01')],
    profile_refs: ['dimensions.saved_callable_values', 'dimensions.reclamation'],
    lanes: ['ownership'],
    observe: observeOwnership,
    effectRows: ownershipEffectRows,
    mapping: 'each saved Program owns its package: a released owner is refused and the surviving owners keep running, in both release orders and across a module alias',
    unsupported: ['owner tokens surviving a host process restart'],
    expectations: [
      expectation('ownership.duplicates-released-in-both-orders', {
        origin: fromOwnership('OWN-CASE-02#expected-shared-duplicates',
          'the remaining owner runs after the first release; both are refused after the second'),
        declared: [
          { order: 'A-then-B', owners: 2, survivor: [{ kind: 1, value: '42' }], exit: 0,
            refusedInvocations: 2, refusedReleases: 1 },
          { order: 'B-then-A', owners: 2, survivor: [{ kind: 1, value: '42' }], exit: 0,
            refusedInvocations: 2, refusedReleases: 1 },
        ],
        extract: observed => observed.ownership.shared,
        mapping: 'run(41) through the survivor returns 42 in both release orders; the released owner and the double release are refused',
      }),
      expectation('ownership.balanced-tree-ownership', {
        origin: fromOwnership('OWN-CASE-03#expected-nested-composition',
          'results are equal and no leaf is freed while the root lives'),
        declared: { leaves: 8, intermediates: 7, released_before_root: 14,
          before: [{ kind: 1, value: '8' }], after: [{ kind: 1, value: '8' }], exit: 0 },
        extract: observed => observed.ownership.balanced,
        mapping: 'every non-root owner is released before the root is run again; the result must not change',
      }),
      expectation('ownership.deep-chain-bounded', {
        origin: fromTable('exporter-table#observation-bound',
          'the host observation of an anonymous saved Program is bounded; a left-deep fifth compose is refused as a guest failure'),
        declared: { refused_at_compose: 5, session_state: 'terminated-after-guest-failure', exit: 2 },
        extract: observed => observed.ownership.left_deep,
        mapping: 'the refusal is a declared bound, not a defect: the session terminates as every failed root does',
      }),
      expectation('ownership.cross-module-alias', {
        origin: fromOwnership('OWN-CASE-02#expected-distinct-owners',
          'a duplicate made by a second module gets its own owner and releases independently'),
        declared: { owners: 3, exit: 0 },
        extract: observed => observed.ownership.cross_module,
        mapping: 'the original and the two duplicates are distinct owners; each runs the original Program',
      }),
    ],
    relax: {
      anticipated_differences: [
        { dimension: 'owner-identity',
          noble: 'each saved Program owns its package through an explicit host owner token',
          relax: 'retained code is owned by the reload transaction, not by a saved value',
          classification_rule: 'owner-token differences are an intentional semantic difference; a double free or a stale handle is a failure in either runtime' },
      ],
    },
  },
];

// --- Scenario B observation -------------------------------------------------------------------

function observeRetention(bundle) {
  const packageRows = structured(laneRun(bundle, 'packageRetention').report, 'package-retention');
  const moduleRows = structured(laneRun(bundle, 'moduleRoot').report, 'module-root-retention');
  const capacity = laneRun(bundle, 'capacity');
  const retentionSummary = packageRows.at(-1);
  assert.equal(retentionSummary.case, 'LREF-CASE-03');
  const row = (rows, variant, observation) => rows.find(entry =>
    entry.variant === variant && entry.observation === observation);
  const retention = variant => ({
    publication_while_owned: row(packageRows, variant, 'publication-while-owned'),
    run_after_replacement: row(packageRows, variant, 'run-after-replacement'),
    run_while_owned: row(packageRows, variant, 'run-while-owned'),
    release: row(packageRows, variant, 'release'),
    run_after_release: row(packageRows, variant, 'run-after-release'),
    publication_after_release: row(packageRows, variant, 'publication-after-release'),
  });
  const normalise = entry => entry === undefined ? null : {
    outcome: entry.outcome,
    stack: entry.stack ?? null,
    epoch: entry.epoch ?? null,
    diagnostic: entry.diagnostic ?? null,
    retire_spans: entry.retire_code_spans ?? null,
  };
  const replacement = retention('replacement');
  const charge = retention('charge');
  const moduleSummary = moduleRows.at(-1);
  assert.equal(moduleSummary.case, 'OWN-CASE-06');
  const moduleObservation = label => moduleRows.find(entry => entry.observation === label);
  const moduleResponses = rowsOf(laneRun(bundle, 'moduleRoot').responses,
    'OWN-CASE-06-module-root-retention');
  const finalRun = moduleResponses.at(-1) ?? null;
  const capacityRequests = rowsOf(capacity.requests, 'LSLOT-saved-program-capacity');
  const requestCounts = capacityRequests.reduce((counts, entry) => {
    const operation = entry.operation ?? 'unknown';
    counts[operation] = (counts[operation] ?? 0) + 1;
    return counts;
  }, {});
  const capacityText = capacity.report.kind === 'text' ? capacity.report.value : '';
  return {
    retention: {
      replacement: {
        publication_while_owned: normalise(replacement.publication_while_owned),
        run_after_replacement: normalise(replacement.run_after_replacement),
        release: normalise(replacement.release),
        run_after_release: normalise(replacement.run_after_release),
      },
      charge: {
        publication_while_owned: normalise(charge.publication_while_owned),
        run_while_owned: normalise(charge.run_while_owned),
        release: normalise(charge.release),
        publication_after_release: normalise(charge.publication_after_release),
      },
    },
    module_root: {
      before: moduleSummary.before,
      after_supersession: normalise(moduleObservation('after-supersession')),
      after_release: normalise(moduleObservation('after-release')),
      retired_span: moduleSummary.retired_span,
      final_run: finalRun === null ? null : { outcome: finalRun.outcome,
        diagnostic: finalRun.diagnostic ?? null },
    },
    capacity: {
      request_counts: requestCounts,
      summary_line: capacityText,
      lane_exit: capacity.exit,
    },
  };
}

function retentionEffectRows(bundle) {
  return {
  normal: [
    { subject: 'retention.replacement.run-after-replacement', purpose: 'saved Program runs after the slot was replaced',
      rows: rowsOf(laneRun(bundle, 'packageRetention').responses, 'LREF-CASE-03-replacement')
        .filter(row => row.outcome === 'executed').map((row, ordinal) => ({
          ordinal, operation: 'invoke', slot: 'runner',
          request: row.stack?.[0]?.value ?? null, response: null })) },
    { subject: 'retention.module-root.after-supersession', purpose: 'module-root survivor still runs',
      rows: rowsOf(laneRun(bundle, 'moduleRoot').responses, 'OWN-CASE-06-module-root-retention')
        .filter(row => row.outcome === 'executed').map((row, ordinal) => ({
          ordinal, operation: 'invoke', slot: 'runner',
          request: row.stack?.[0]?.value ?? null, response: null })) },
  ].filter(entry => entry.rows.length > 0),
  abnormal: [
    { subject: 'retention.replacement.after-release', reason: 'released owner refused; no effect is erased',
      rows: [{ ordinal: 0, operation: 'invoke', slot: 'runner', request: null,
        response: 'refused' }] },
  ],
  };
}

// --- Scenario C observation -------------------------------------------------------------------

function observeRelocation(bundle) {
  const summary = structured(laneRun(bundle, 'relocation').report, 'relocation-witness');
  const responses = variant => rowsOf(laneRun(bundle, 'relocation').responses, variant);
  const traceOf = variant => {
    const executed = responses(variant).filter(row => row.outcome === 'executed'
      && Array.isArray(row.request_trace));
    return executed.at(-1)?.request_trace ?? [];
  };
  const DENIED = 'reloc-denied';
  const deniedRows = responses(DENIED);
  const denied = deniedRows.at(-1) ?? null;
  return {
    relocation: {
      old_slot: summary.oldSlot,
      new_slot: summary.newSlot,
      old_definition: summary.oldDefinitionId,
      new_definition: summary.newDefinitionId,
      duplicate_count: summary.duplicateCount,
      omission_count: summary.omissionCount,
      denied_outcome: summary.deniedOutcome,
      old_effects: distinguishedRows(traceOf('reloc-old')).length,
      new_effects: distinguishedRows(traceOf('reloc-new')).length,
      old_request: distinguishedRows(traceOf('reloc-old'))[0]?.request ?? null,
      old_response: distinguishedRows(traceOf('reloc-old'))[0]?.response ?? null,
      new_request: distinguishedRows(traceOf('reloc-new'))[0]?.request ?? null,
      new_response: distinguishedRows(traceOf('reloc-new'))[0]?.response ?? null,
      old_first: traceOf('reloc-old')[0]?.operation ?? null,
      new_first: traceOf('reloc-new')[0]?.operation ?? null,
      omission: {
        dispatches: dispatchRows(traceOf('reloc-omission')).length,
        effects: effectRows(traceOf('reloc-omission')).length,
      },
      denied: {
        executed: denied?.outcome === 'executed',
        responses: distinguishedRows(denied?.request_trace ?? []).length,
        dispatchKept: dispatchRows(denied?.request_trace ?? []).length > 0,
        effectRequest: effectRows(denied?.request_trace ?? [])[0]?.request ?? null,
        outcome: denied?.outcome ?? null,
      },
    },
  };
}

function relocationEffectRows(bundle) {
  const responses = variant => rowsOf(laneRun(bundle, 'relocation').responses, variant);
  const traceOf = variant => responses(variant)
    .filter(row => row.outcome === 'executed' && Array.isArray(row.request_trace))
    .at(-1)?.request_trace ?? [];
  const denied = rowsOf(laneRun(bundle, 'relocation').responses, 'reloc-denied').at(-1) ?? null;
  return {
    normal: [
      { subject: 'relocation.old-target', purpose: 'the effect is performed for slot A',
        rows: traceSummary(traceOf('reloc-old')) },
      { subject: 'relocation.relocated-target', purpose: 'the same effect is performed for slot E',
        rows: traceSummary(traceOf('reloc-new')) },
    ].filter(entry => entry.rows.length > 0),
    abnormal: [
      { subject: 'relocation.omission', reason: 'dispatch kept, no effect row',
        rows: traceSummary(traceOf('reloc-omission')) },
      { subject: 'relocation.denied', reason: 'effect grant withdrawn; the dispatch prefix stays in the record',
        rows: traceSummary(denied?.request_trace ?? []) },
    ],
  };
}

// --- Scenario D observation -------------------------------------------------------------------

function observeTyped(bundle) {
  const report = structured(laneRun(bundle, 'typed').report, 'typed-cases');
  const cases = Object.fromEntries(report.cases.map(entry => {
    const roots = Array.isArray(entry.expected) ? entry.expected : [];
    return [entry.variant, {
      roots: roots.length,
      dispatches: roots.map(root => root.dispatches),
      effects: roots.map(root => root.effects),
    }];
  }));
  return {
    typed: {
      seed: report.seed,
      case_digest: report.case_digest,
      cases: {
        directPureRepeat1: cases['typed-direct-pure-repeat1-0'],
        directPureRepeat2: cases['typed-direct-pure-repeat2-1'],
        directEmitRepeat1: cases['typed-direct-dispatch+emit-repeat1-2'],
        directEmitRepeat2: cases['typed-direct-dispatch+emit-repeat2-3'],
        chain: cases['typed-chain-nested-repeat1-4'],
      },
      coverage: report.coverage,
      negative: {
        refused: report.negative.refused,
        classes: [...report.negative.classes].sort(),
        by_stage: report.negative.by_stage,
        guest_requests: Math.max(0,
          ...report.negative.observations.map(row => row.observed.guest_requests ?? 0)),
        trace: report.negative.observations.flatMap(row => row.observed.trace),
      },
      minimization: {
        minimal_starting_witnesses: report.minimization.minimal_starting_witnesses,
        accepted_shrinks: report.minimization.accepted_shrinks,
        rejected: report.minimization.rejected_shrinks.map(row => ({
          class: row.class, dimension: row.dimension, observed: row.observed,
        })),
      },
      retention: report.retention,
      type_boundaries: report.typeBoundaries,
    },
  };
}

function typedEffectRows(bundle) {
  const typed = laneRun(bundle, 'typed');
  const emitted = [...typed.responses.keys()].sort((a, b) => a.localeCompare(b, 'en'))
    .map(variant => ({
      subject: `typed.${variant}`,
      purpose: 'effect rows of one generated case',
      rows: rowsOf(typed.responses, variant)
        .filter(row => row.outcome === 'executed' && Array.isArray(row.request_trace))
        .flatMap(row => traceSummary(row.request_trace).filter(entry => entry.operation === 'effect')),
    }))
    .filter(entry => entry.rows.length > 0);
  return { normal: emitted, abnormal: [] };
};

// --- Scenario E observation -------------------------------------------------------------------

function observeOwnership(bundle) {
  const report = structured(laneRun(bundle, 'ownership').report, 'ownership-cases');
  const cases = report.cases;
  const run = laneRun(bundle, 'ownership');
  // One issued request produces exactly one response, in order; pairing them recovers which
  // operation was refused without re-stating the lane's step list.
  const pairs = variant => {
    const requests = rowsOf(run.requests, variant);
    // Each session emits an initial `configured` response before its first request.
    const responses = rowsOf(run.responses, variant)
      .filter(response => response.outcome !== 'configured');
    return requests.map((request, ordinal) => ({ request, response: responses[ordinal] ?? null }));
  };
  const shared = cases['OWN-CASE-02'].map(entry => ({
    order: entry.order,
    owners: entry.owners.length,
    survivor: entry.survivor,
    exit: entry.exit,
    refusedInvocations: pairs(`OWN-CASE-02-${entry.order}`).filter(pair =>
      pair.request.operation === 'invoke' && pair.response?.outcome === 'refused').length,
    refusedReleases: pairs(`OWN-CASE-02-${entry.order}`).filter(pair =>
      pair.request.operation === 'release-program' && pair.response?.outcome === 'refused').length,
  }));
  const balanced = cases['OWN-CASE-03'].balanced;
  const leftDeep = cases['OWN-CASE-03'].left_deep;
  const crossModule = cases['cross-module-alias'];
  return {
    ownership: {
      shared,
      balanced: {
        leaves: balanced.leaves,
        intermediates: balanced.intermediates,
        released_before_root: balanced.released_before_root,
        before: balanced.before,
        after: balanced.after,
        exit: balanced.exit,
      },
      left_deep: {
        refused_at_compose: leftDeep.refused_at_compose,
        session_state: leftDeep.session_state,
        exit: leftDeep.exit,
      },
      cross_module: { owners: crossModule.owners.length, exit: crossModule.exit },
    },
  };
}

function ownershipEffectRows(bundle) {
  return {
  normal: [
    { subject: 'ownership.shared-owners', purpose: 'each duplicate runs the same Program',
      rows: rowsOf(laneRun(bundle, 'ownership').responses, 'OWN-CASE-02-A-then-B')
        .filter(row => row.outcome === 'executed').map((row, ordinal) => ({
          ordinal, operation: 'invoke', slot: 'runner',
          request: row.stack?.[0]?.value ?? null, response: null })) },
  ].filter(entry => entry.rows.length > 0),
  abnormal: [
    { subject: 'ownership.released-owner', reason: 'a released owner is refused, not trapped',
      rows: [{ ordinal: 0, operation: 'invoke', slot: 'runner', request: null,
        response: 'refused' }] },
  ],
  };
}

// ---------------------------------------------------------------------------------------------
// Classification
// ---------------------------------------------------------------------------------------------

export function checkClaim(claim, declared, observed) {
  if (claim === 'equals') return stableJson(declared) === stableJson(observed);
  if (claim === 'not-equals') return stableJson(declared) !== stableJson(observed);
  if (claim === 'matches')
    return typeof observed === 'string' && new RegExp(declared).test(observed);
  throw new Error(`unknown expectation claim ${claim}`);
}

/**
 * Stable digest of the frozen expectation declarations. The receipt records it, so editing a
 * declared expectation invalidates the evidence produced under the old table.
 */
export function expectationTableDigest() {
  return digestValue(SCENARIOS.flatMap(scenario => scenario.expectations.map(row => ({
    case_id: scenario.case_id, expectation_id: row.expectation_id, claim: row.claim,
    declared: row.declared, origin: row.origin, mismatch_class: row.mismatch_class,
  }))));
}

/**
 * Precedence: a failed lane dominates (runtime-failure), then a contract violation, then an
 * unexpected refusal, then a declared intentional difference. `unsupported` only remains when every
 * modeled expectation holds and the only open rows are declared unrepresentable.
 */
export function classifyScenario(expectations, laneRuns = []) {
  const failed = laneRuns.filter(run => run.exit !== 0 || run.report_valid !== true);
  if (failed.length > 0)
    return { result_class: 'runtime-failure',
      reasons: failed.map(run => `${run.id} exited ${run.exit} with ${run.report_valid ? 'its expected report' : 'an unexpected report shape'}`) };
  const mismatches = expectations.filter(row => row.status === 'mismatch');
  for (const candidate of ['contract-violation', 'unexpected-refusal',
    'intentional-semantic-difference']) {
    const selected = mismatches.filter(row => row.mismatch_class === candidate);
    if (selected.length > 0)
      return { result_class: candidate,
        reasons: selected.map(row => `${row.expectation_id}: declared ${stableJson(row.declared)} observed ${stableJson(row.observed)}`) };
  }
  if (expectations.some(row => row.status === 'unsupported'))
    return { result_class: 'unsupported',
      reasons: expectations.filter(row => row.status === 'unsupported')
        .map(row => `${row.expectation_id}: ${row.declared?.unsupported ?? 'not representable'}`) };
  return { result_class: 'supported-outcome', reasons: [] };
}

/**
 * Per-runtime classification for an outcome observed in a *foreign* runtime whose own profile is
 * frozen separately. An outcome the owner profile accepts is an intentional semantic difference
 * from the Noble profile, never a Noble contract violation and never a defect of the owner.
 */
export function classifyForeignOutcome(outcome, ownerProfile) {
  const accepted = ownerProfile?.accepted_outcomes ?? [];
  if (accepted.includes(outcome)) return 'intentional-semantic-difference';
  if (outcome === undefined || outcome === null) return 'not-run';
  return 'unsupported';
}

// ---------------------------------------------------------------------------------------------
// Scenario records
// ---------------------------------------------------------------------------------------------

export function evaluateExpectations(scenario, observations) {
  return scenario.expectations.map(declaredExpectation => {
    const { extract, ...declaration } = declaredExpectation;
    const unsupportedRow = declaration.declared !== null
      && typeof declaration.declared === 'object'
      && Object.keys(declaration.declared).length === 1
      && typeof declaration.declared.unsupported === 'string';
    const observed = extract(observations);
    const status = unsupportedRow ? 'unsupported'
      : (checkClaim(declaration.claim, declaration.declared, observed) ? 'holds' : 'mismatch');
    return { ...declaration, observed, status };
  });
}

export function scenarioRecord(scenario, bundle, context) {
  const observations = scenario.observe(bundle);
  const expectations = evaluateExpectations(scenario, observations);
  const lanes = context.lanes;
  const comparison = classifyScenario(expectations, lanes);
  const effect_prefixes = scenario.effectRows(bundle);
  const edits = { variants: [] };
  const ordering = { checkpoints: [], publications: [], roots: [] };
  const selected = [];
  for (const laneId of scenario.lanes) {
    edits.variants.push(...editSummary(bundle, laneId));
    for (const variant of [...laneRun(bundle, laneId).responses.keys()].sort((a, b) => a.localeCompare(b, 'en'))) {
      const rows = laneRun(bundle, laneId).responses.get(variant) ?? [];
      for (const row of rows) {
        if (row.stage === 'operator-control' && row.outcome === 'checkpoint-entered')
          ordering.checkpoints.push({
            lane: laneId, variant, imported: row.import ?? null, ordinal: row.ordinal ?? null,
            root: row.root ?? null, checkpoint: row.checkpoint ?? null, outcome: row.outcome,
          });
        if (row.stage === 'operator-control' && row.outcome === 'published')
          ordering.publications.push({
            lane: laneId, variant, operation: 'publish', epoch: row.epoch ?? null,
            root: row.root ?? null, checkpoint: row.checkpoint ?? null, outcome: row.outcome,
          });
        if (row.outcome === 'executed' && typeof row.epoch === 'string')
          ordering.roots.push({ lane: laneId, variant, epoch: row.epoch });
        if (Array.isArray(row.request_trace))
          selected.push(...selectedIdentities(`${laneId}.${variant}`, row.request_trace));
      }
    }
  }
  // Publication rows from the nested-roots report carry their own acknowledgement shape.
  for (const laneId of scenario.lanes) {
    for (const variant of [...laneRun(bundle, laneId).responses.keys()]) {
      for (const row of laneRun(bundle, laneId).responses.get(variant) ?? []) {
        if (row.outcome === 'published' && row.stage === undefined)
          ordering.publications.push({ lane: laneId, variant, operation: 'publish',
            epoch: row.epoch ?? null, root: row.root ?? null, checkpoint: row.checkpoint ?? null,
            outcome: row.outcome });
      }
    }
  }
  // The timing record is bound to this case, the measured source revision and the case's admission
  // outcome. The per-step admission and refusal observations stay in `observations` and
  // `expectations`; the aggregate outcome recorded here is the class this case was judged with.
  const timing = {
    started_at: context.startedAt,
    ended_at: context.endedAt,
    duration_ms: Math.max(0, Date.parse(context.endedAt) - Date.parse(context.startedAt)),
    bound_to: {
      case_id: scenario.case_id,
      source_commit: context.sourceCommit,
      result_class: comparison.result_class,
      admission_outcome: expectations.map(row => ({
        expectation_id: row.expectation_id,
        status: row.status,
        observed: row.observed,
      })),
    },
  };
  const eventFiles = [];
  for (const laneId of scenario.lanes) {
    for (const file of laneRun(bundle, laneId).eventEvidence)
      eventFiles.push({ path: file.path, sha256: file.sha256 });
  }
  eventFiles.sort((a, b) => a.path.localeCompare(b.path, 'en'));
  return {
    case_id: scenario.case_id,
    title: scenario.title,
    bounded: scenario.bounded,
    canonical_refs: scenario.canonical_refs,
    profile_refs: scenario.profile_refs,
    lanes,
    provenance: context.provenance,
    runtime_identities: {
      cli_sha256: context.cliSha256,
      engine: 'v8',
      installed_sha256: collectInstalled(observations),
    },
    edits,
    ordering,
    selected_identities: selected,
    effect_prefixes,
    event_binding: {
      files: eventFiles,
      sha256: sha256(eventFiles.map(file => file.sha256).join('\n')),
    },
    expectations,
    observations,
    comparison: {
      runtime: 'noble',
      result_class: comparison.result_class,
      reasons: comparison.reasons,
      faithful_mapping: scenario.mapping,
      unsupported_dimensions: scenario.unsupported ?? [],
      intentional_differences: [],
    },
    other_runtime: {
      name: OTHER_RUNTIME.name,
      profile: null,
      result_class: 'not-run',
      reason: OTHER_RUNTIME.reason,
      anticipated_differences: scenario.relax.anticipated_differences,
    },
    timing,
  };
}

function collectInstalled(observations) {
  const base = observations?.nested?.base?.installed_sha256;
  return base ?? {};
}

// ---------------------------------------------------------------------------------------------
// Evidence loading and re-derivation
// ---------------------------------------------------------------------------------------------

/**
 * Rebuild the lane bundle from retained bytes only. `verifyEvidence` runs this and re-derives every
 * scenario, so the corpus cannot state an observation its own raw reports do not contain.
 */
export function bundleFromFiles(directory, laneIds) {
  const lanes = new Map();
  for (const laneId of laneIds) {
    const definition = LANES[laneId];
    assert.ok(definition, `unknown lane ${laneId}`);
    const wanted = path.basename(definition.path, '.mjs');
    const rawRoot = path.join(directory, 'raw');
    assert.ok(fs.existsSync(rawRoot), 'the evidence directory has no raw reports');
    const entries = fs.readdirSync(rawRoot, { withFileTypes: true })
      .filter(entry => entry.isDirectory() && entry.name === wanted)
      .map(entry => entry.name);
    assert.equal(entries.length, 1, `retained raw directory for ${laneId} is missing or ambiguous`);
    const root = path.join(rawRoot, entries[0]);
    const files = fs.readdirSync(root).sort();
    const stdoutFile = files.find(name => name === 'stdout.txt');
    assert.ok(stdoutFile, `retained stdout for ${laneId} is missing`);
    const stdout = fs.readFileSync(path.join(root, stdoutFile), 'utf8');
    const report = parseLaneStdout(stdout);
    const requests = new Map();
    const responses = new Map();
    const requestBytes = new Map();
    const eventEvidence = [];
    for (const name of files) {
      const bytes = fs.readFileSync(path.join(root, name));
      const variant = name.replace(/\.(requests|responses|session|stderr)\.(jsonl|txt)$/u, '');
      if (name.endsWith('.requests.jsonl')) {
        requests.set(variant, bytes.toString('utf8').trim().split('\n')
          .filter(line => line !== '').map(line => JSON.parse(line)));
        requestBytes.set(variant, bytes);
        eventEvidence.push({ path: `raw/${entries[0]}/${name}`, sha256: sha256(bytes) });
      } else if (name.endsWith('.responses.jsonl')) {
        responses.set(variant, bytes.toString('utf8').trim().split('\n')
          .filter(line => line !== '').map(line => JSON.parse(line)));
        eventEvidence.push({ path: `raw/${entries[0]}/${name}`, sha256: sha256(bytes) });
      }
    }
    lanes.set(laneId, { id: laneId, path: definition.path, exit: 0, stdout, report,
      requests, responses, requestBytes, eventEvidence });
  }
  return { lanes };
}

// ---------------------------------------------------------------------------------------------
// Validation and evidence verification
// ---------------------------------------------------------------------------------------------

const HEX = /^[0-9a-f]{64}$/;
const isObject = value => value !== null && typeof value === 'object' && !Array.isArray(value);
const isString = value => typeof value === 'string' && value.length > 0;

/** Structural validation of a corpus envelope. Returns an array of failure strings. */
export function validateCorpus(document) {
  const failures = [];
  const fail = (where, reason) => failures.push(`${where}: ${reason}`);
  if (!isObject(document)) return ['corpus: not an object'];
  if (document.schema !== SCHEMA) fail('schema', `expected ${SCHEMA}`);
  if (!isString(document.corpus_revision)) fail('corpus_revision', 'missing');
  if (!isString(document.generated_at) || Number.isNaN(Date.parse(document.generated_at)))
    fail('generated_at', 'not a date-time');
  if (!isObject(document.profile) || document.profile.path !== PROFILE_PATH
    || document.profile.id !== PROFILE_ID || !HEX.test(document.profile.sha256 ?? ''))
    fail('profile', 'path/id/sha256 mismatch');
  if (!isObject(document.runtime) || document.runtime.name !== 'noble'
    || document.runtime.engine !== 'v8' || !HEX.test(document.runtime.cli?.sha256 ?? ''))
    fail('runtime', 'name/engine/cli digest mismatch');
  if (!Array.isArray(document.runtime?.sources) || document.runtime.sources.length < 3)
    fail('runtime.sources', 'expected the three slot runtime sources');
  if (!isObject(document.source) || !/^[0-9a-f]{40}$/u.test(document.source.commit ?? '')
    || document.source.clean !== true)
    fail('source', 'commit/clean binding missing');
  if (!Array.isArray(document.pins) || document.pins.length === 0)
    fail('pins', 'empty');
  else for (const pin of document.pins)
    if (!isString(pin.path) || !HEX.test(pin.sha256 ?? '')) fail('pins', 'malformed binding');
  if (!Array.isArray(document.result_classes)
    || document.result_classes.map(row => row.id).join(',') !== RESULT_CLASS_IDS.join(','))
    fail('result_classes', 'the seven classes are not in declaration order');
  const states = document.canonical_cases?.states ?? {};
  if (document.canonical_cases?.path !== CANONICAL_CASES) fail('canonical_cases.path', 'mismatch');
  const required = { implementation: 'absent', execution: 'not-run', proof: 'open',
    trust: 'unassessed' };
  for (const [id, state] of Object.entries(states)) {
    for (const [key, value] of Object.entries(required))
      if (state[key] !== value) fail(`canonical_cases.states.${id}.${key}`, `expected ${value}`);
    if (!Array.isArray(state.evidence) || state.evidence.length !== 0)
      fail(`canonical_cases.states.${id}.evidence`, 'must stay empty');
  }
  if (Object.keys(states).length !== 9) fail('canonical_cases.states', 'expected nine cases');
  if (!Array.isArray(document.scenarios) || document.scenarios.length < 3)
    return [...failures, 'scenarios: fewer than three bounded scenarios'];
  const seen = new Set();
  for (const scenario of document.scenarios) {
    const where = `scenarios.${scenario.case_id}`;
    if (!/^noble-live-update\/[a-z0-9-]+$/u.test(scenario.case_id ?? ''))
      fail(where, 'malformed case_id');
    if (seen.has(scenario.case_id)) fail(where, 'duplicate case_id');
    seen.add(scenario.case_id);
    if (!isString(scenario.title)) fail(where, 'missing title');
    if (!isObject(scenario.bounded) || !isObject(scenario.bounded.limits)
      || Object.keys(scenario.bounded.limits).length === 0)
      fail(where, 'missing bounded limits');
    if (!Array.isArray(scenario.canonical_refs) || scenario.canonical_refs.length === 0)
      fail(where, 'no canonical reference');
    if (!Array.isArray(scenario.profile_refs) || scenario.profile_refs.length === 0)
      fail(where, 'no profile dimension referenced');
    if (!Array.isArray(scenario.lanes) || scenario.lanes.length === 0)
      fail(where, 'no lane');
    else for (const lane of scenario.lanes) {
      if (!isString(lane.path) || !HEX.test(lane.sha256 ?? ''))
        fail(where, `lane ${lane.id} lacks a digest`);
      if (!Array.isArray(lane.variants) || lane.variants.length === 0)
        fail(where, `lane ${lane.id} lacks variants`);
      if (!Number.isInteger(lane.exit)) fail(where, `lane ${lane.id} lacks an exit status`);
      if (!['json', 'jsonl', 'text', 'empty'].includes(lane.report_kind)
        || typeof lane.report_valid !== 'boolean'
        || Number.isFinite(lane.duration_ms) !== true || lane.duration_ms < 0)
        fail(where, `lane ${lane.id} has malformed report or timing metadata`);
      else if (Object.hasOwn(LANES, lane.id)
        && lane.report_valid !== hasExpectedReport(lane.id, lane.report_kind))
        fail(where, `lane ${lane.id} report validity disagrees with its declared output shape`);
    }
    if (!Array.isArray(scenario.provenance) || scenario.provenance.length === 0)
      fail(where, 'no raw provenance');
    else for (const entry of scenario.provenance) {
      if (!isString(entry.path) || !HEX.test(entry.sha256 ?? ''))
        fail(where, 'provenance entry without a digest');
    }
    if (!isObject(scenario.edits) || !Array.isArray(scenario.edits.variants)
      || scenario.edits.variants.length === 0) fail(where, 'no exact edits recorded');
    else for (const variant of scenario.edits.variants)
      if (!isString(variant.variant) || !Number.isInteger(variant.count)
        || !Array.isArray(variant.operations)) fail(where, 'malformed edit record');
    if (!isObject(scenario.ordering)) fail(where, 'no ordering record');
    if (!Array.isArray(scenario.selected_identities)) fail(where, 'no selected identities');
    if (!isObject(scenario.effect_prefixes) || !Array.isArray(scenario.effect_prefixes.normal)
      || !Array.isArray(scenario.effect_prefixes.abnormal))
      fail(where, 'effect prefixes are not split into normal/abnormal');
    if (!isObject(scenario.event_binding) || !HEX.test(scenario.event_binding.sha256 ?? ''))
      fail(where, 'event binding missing');
    if (!Array.isArray(scenario.expectations) || scenario.expectations.length === 0)
      fail(where, 'no expectation');
    else for (const row of scenario.expectations) {
      if (!isString(row.expectation_id)) fail(where, 'expectation without id');
      if (!['canonical', 'exporter-table'].includes(row.origin?.kind))
        fail(where, `expectation ${row.expectation_id} lacks a declared origin`);
      if (!['equals', 'not-equals', 'matches'].includes(row.claim))
        fail(where, `expectation ${row.expectation_id} has an unknown claim`);
      if (!['holds', 'mismatch', 'unsupported'].includes(row.status))
        fail(where, `expectation ${row.expectation_id} has an unknown status`);
      if (!RESULT_CLASS_IDS.includes(row.mismatch_class))
        fail(where, `expectation ${row.expectation_id} has an unknown mismatch class`);
    }
    if (!isObject(scenario.comparison) || scenario.comparison.runtime !== 'noble'
      || !RESULT_CLASS_IDS.includes(scenario.comparison.result_class)
      || scenario.comparison.result_class === 'not-run')
      fail(where, 'noble comparison missing or misclassified');
    if (!isString(scenario.comparison?.faithful_mapping))
      fail(where, 'no faithful mapping statement');
    if (scenario.other_runtime?.name !== OTHER_RUNTIME.name
      || scenario.other_runtime?.result_class !== 'not-run'
      || !isString(scenario.other_runtime?.reason)
      || scenario.other_runtime.profile !== null)
      fail(where, 'the foreign runtime column must be not-run with a reason and no profile');
    if (!Array.isArray(scenario.other_runtime?.anticipated_differences)
      || scenario.other_runtime.anticipated_differences.length === 0)
      fail(where, 'no anticipated foreign-runtime differences');
    if (!isObject(scenario.timing) || scenario.timing.bound_to?.case_id !== scenario.case_id
      || scenario.timing.bound_to?.source_commit !== document.source?.commit
      || scenario.timing.bound_to?.result_class !== scenario.comparison?.result_class
      || !Array.isArray(scenario.timing.bound_to?.admission_outcome)
      || scenario.timing.bound_to.admission_outcome.length === 0
      || !Number.isFinite(scenario.timing.duration_ms))
      fail(where, 'timing is not bound to this case, revision and outcome');
  }
  if (!Array.isArray(document.non_claims) || document.non_claims.length === 0)
    fail('non_claims', 'empty');
  return failures;
}

/** Structural validation of a receipt. */
export function validateReceipt(receipt) {
  const failures = [];
  const fail = (where, reason) => failures.push(`${where}: ${reason}`);
  if (receipt.schema !== RECEIPT_SCHEMA) fail('schema', `expected ${RECEIPT_SCHEMA}`);
  if (!isString(receipt.claim)) fail('claim', 'missing');
  if (!/^[0-9a-f]{40}$/u.test(receipt.source?.commit ?? '') || receipt.source?.clean !== true)
    fail('source', 'commit/clean binding missing');
  if (receipt.profile?.path !== PROFILE_PATH || !HEX.test(receipt.profile?.sha256 ?? ''))
    fail('profile', 'path/sha256 missing');
  if (!HEX.test(receipt.corpus?.sha256 ?? '')) fail('corpus', 'digest missing');
  if (!HEX.test(receipt.expectations_sha256 ?? '')) fail('expectations_sha256', 'missing');
  if (!Array.isArray(receipt.pins) || receipt.pins.length === 0)
    fail('pins', 'source bindings are missing');
  else for (const pin of receipt.pins)
    if (!isString(pin.path) || !HEX.test(pin.sha256 ?? '')) fail('pins', 'malformed source binding');
  if (!Array.isArray(receipt.scenarios) || receipt.scenarios.length < 3)
    fail('scenarios', 'fewer than three');
  if (!isObject(receipt.result_class_counts)) fail('result_class_counts', 'missing');
  else for (const id of RESULT_CLASS_IDS)
    if (!Number.isInteger(receipt.result_class_counts[id]))
      fail(`result_class_counts.${id}`, 'missing count');
  if (receipt.relax?.result_class !== 'not-run' || !isString(receipt.relax?.reason))
    fail('relax', 'must be not-run with a reason');
  if (!Array.isArray(receipt.non_claims) || receipt.non_claims.length === 0)
    fail('non_claims', 'empty');
  // The control section is optional here so a producer can verify its evidence before the suite
  // has run; `controls.test.mjs` requires it on the committed receipt.
  if (receipt.controls !== undefined
    && (!isObject(receipt.controls) || !Array.isArray(receipt.controls.controls)
      || receipt.controls.controls.length === 0))
    fail('controls', 'malformed control results');
  if (!HEX.test(receipt.binding_sha256 ?? '')) fail('binding_sha256', 'missing');
  return failures;
}

export function canonicalSnapshot(root) {
  const file = path.join(root, CANONICAL_CASES);
  const bytes = fs.readFileSync(file);
  const document = JSON.parse(bytes.toString('utf8'));
  return {
    path: CANONICAL_CASES,
    sha256: sha256(bytes),
    states: Object.fromEntries(document.cases.map(row => [row.id, {
      implementation: row.state?.implementation ?? null,
      execution: row.state?.execution ?? null,
      proof: row.state?.proof ?? null,
      trust: row.state?.trust ?? null,
      evidence: row.evidence ?? [],
    }])),
  };
}

/** A promotion guard: every canonical case must stay absent/not-run/open/unassessed. */
export function canonicalPromotions(states) {
  const required = { implementation: 'absent', execution: 'not-run', proof: 'open',
    trust: 'unassessed' };
  const failures = [];
  for (const [id, state] of Object.entries(states)) {
    for (const [key, value] of Object.entries(required))
      if (state[key] !== value)
        failures.push({ code: 'canonical-case-promoted', target: `${id}.${key}`,
          detail: `expected ${value}, recorded ${stableJson(state[key])}` });
    if (!Array.isArray(state.evidence) || state.evidence.length !== 0)
      failures.push({ code: 'canonical-case-promoted', target: `${id}.evidence`,
        detail: 'canonical case gained evidence' });
  }
  return failures;
}

/**
 * Verify retained evidence against itself, the profile, and (when a repository root is supplied)
 * the live profile and the canonical case states.
 *
 * `receipt`/`corpus` may be supplied in memory when a producer verifies its own output before the
 * receipt is written.
 */
export function verifyEvidence({ directory, repositoryRoot = null, receipt = null,
  corpus = null, profileBytes = null }) {
  const failures = [];
  const fail = (code, target, detail) => failures.push({ code, target, detail });
  const read = name => fs.readFileSync(path.join(directory, name));
  const receiptPath = path.join(directory, 'receipt.json');
  const corpusPath = path.join(directory, 'corpus.json');
  const profilePath = path.join(directory, 'profile.json');
  if (receipt === null) {
    if (!fs.existsSync(receiptPath)) return { failures: [{ code: 'missing-evidence',
      target: 'receipt.json', detail: 'no receipt in the evidence directory' }] };
    receipt = JSON.parse(read('receipt.json').toString('utf8'));
  }
  if (corpus === null) {
    if (!fs.existsSync(corpusPath)) return { failures: [{ code: 'missing-evidence',
      target: 'corpus.json', detail: 'no corpus in the evidence directory' }] };
    corpus = JSON.parse(read('corpus.json').toString('utf8'));
  }
  if (profileBytes === null) {
    if (!fs.existsSync(profilePath)) return { failures: [{ code: 'missing-evidence',
      target: 'profile.json', detail: 'no retained profile' }] };
    profileBytes = read('profile.json');
  }
  const profileDigest = sha256(profileBytes);
  if (profileDigest !== receipt.profile?.sha256)
    fail('profile-changed', 'profile.json',
      `retained profile digest ${profileDigest} differs from the receipt's ${receipt.profile?.sha256}`);
  if (repositoryRoot !== null) {
    const liveProfile = path.join(repositoryRoot, PROFILE_PATH);
    if (!fs.existsSync(liveProfile))
      fail('profile-changed', PROFILE_PATH, 'the live profile is missing');
    else if (fileSha256(repositoryRoot, PROFILE_PATH) !== receipt.profile?.sha256)
      fail('profile-changed', PROFILE_PATH,
        'the live profile differs from the profile this evidence was produced under; prior comparison evidence is invalid');
  }
  if (fs.existsSync(corpusPath) && sha256(read('corpus.json')) !== receipt.corpus?.sha256)
    fail('stale-evidence', 'corpus.json', 'corpus bytes differ from the receipt binding');
  const schemaFailures = validateCorpus(corpus);
  for (const row of schemaFailures) fail('schema-invalid', 'corpus.json', row);
  const pinFailures = validateReceipt(receipt);
  for (const row of pinFailures) fail('schema-invalid', 'receipt.json', row);
  const boundPins = (receipt.pins ?? []).map(({ path: sourcePath, sha256: digest }) => ({
    path: sourcePath, sha256: digest,
  }));
  if (stableJson(receipt.pins) !== stableJson(corpus.pins))
    fail('source-pin-mismatch', 'pins', 'the receipt and corpus source bindings differ');
  if (stableJson(receipt.source?.bound) !== stableJson(boundPins))
    fail('source-pin-mismatch', 'receipt.source.bound', 'the receipt source binding list is inconsistent');
  if (receipt.source?.commit !== corpus.source?.commit
    || receipt.source?.tree !== corpus.source?.tree)
    fail('source-pin-mismatch', 'source', 'receipt and corpus source revisions differ');
  if (stableJson(receipt.runtime) !== stableJson(corpus.runtime))
    fail('source-pin-mismatch', 'runtime', 'receipt and corpus runtime identities differ');
  if (repositoryRoot !== null) {
    const repository = path.resolve(repositoryRoot);
    for (const pin of receipt.pins ?? []) {
      const sourceFile = path.resolve(repository, pin.path);
      if (sourceFile === repository || !sourceFile.startsWith(`${repository}${path.sep}`)) {
        fail('source-pin-mismatch', pin.path, 'source binding escapes the repository');
      } else if (!fs.existsSync(sourceFile)) {
        fail('source-pin-mismatch', pin.path, 'bound source file is missing');
      } else if (fileSha256(repository, pin.path) !== pin.sha256) {
        fail('source-pin-mismatch', pin.path, 'bound source bytes changed after export');
      }
    }
  }

  const retained = new Map();
  for (const scenario of corpus.scenarios ?? []) {
    for (const entry of scenario.provenance ?? []) {
      const file = path.join(directory, entry.path);
      if (!fs.existsSync(file)) {
        if (entry.retained !== false)
          fail('missing-evidence', entry.path, 'a retained raw report is missing');
        continue;
      }
      const bytes = fs.readFileSync(file);
      retained.set(entry.path, bytes);
      if (sha256(bytes) !== entry.sha256)
        fail('stale-evidence', entry.path,
          'raw bytes differ from the recorded digest; the observation is stale');
      if (bytes.length !== entry.bytes)
        fail('stale-evidence', entry.path, 'raw byte length differs from the recorded length');
    }
  }

  // Expectations exactly as declared by the frozen table, independent of the retained corpus.
  if (expectationTableDigest() !== receipt.expectations_sha256)
    fail('expectation-table-mismatch', 'expectations',
      'the retained expectations digest differs from the declared table');

  const stateFailures = canonicalPromotions(corpus.canonical_cases?.states ?? {});
  for (const row of stateFailures) fail(row.code, row.target, row.detail);
  if (repositoryRoot !== null) {
    const live = canonicalSnapshot(repositoryRoot);
    if (live.sha256 !== corpus.canonical_cases?.sha256)
      fail('canonical-state-changed', CANONICAL_CASES,
        'the canonical case file differs from the snapshot this corpus recorded; re-export before comparing');
  }

  // Re-derive every scenario from the retained bytes and compare with what the corpus states.
  for (const scenario of corpus.scenarios ?? []) {
    const declaration = SCENARIOS.find(row => row.case_id === scenario.case_id);
    if (!declaration) {
      fail('derivation-drift', scenario.case_id, 'no declared scenario matches this case id');
      continue;
    }
    let bundle;
    try {
      bundle = bundleFromFiles(directory, declaration.lanes);
    } catch (error) {
      fail('derivation-drift', scenario.case_id, `retained raw reports cannot be reloaded: ${error.message}`);
      continue;
    }
    if (scenario.lanes?.length !== declaration.lanes.length)
      fail('derivation-drift', scenario.case_id, 'the retained lane roster differs from the declaration');
    for (const laneId of declaration.lanes) {
      const recorded = scenario.lanes?.find(lane => lane.id === laneId);
      const run = bundle.lanes.get(laneId);
      if (recorded === undefined) {
        fail('derivation-drift', `${scenario.case_id}.${laneId}`, 'the retained lane record is missing');
        continue;
      }
      if (recorded.path !== LANES[laneId].path || recorded.report_kind !== run.report.kind
        || recorded.report_valid !== hasExpectedReport(laneId, run.report.kind))
        fail('derivation-drift', `${scenario.case_id}.${laneId}`,
          'the retained report identity or expected shape differs from the raw report');
      const laneDirectory = path.join(directory, 'raw', path.basename(LANES[laneId].path, '.mjs'));
      for (const [name, digestField, bytesField] of [
        ['stdout.txt', 'stdout_sha256', 'stdout_bytes'],
        ['stderr.txt', 'stderr_sha256', 'stderr_bytes'],
      ]) {
        const bytes = fs.readFileSync(path.join(laneDirectory, name));
        if (sha256(bytes) !== recorded[digestField] || bytes.length !== recorded[bytesField])
          fail('stale-evidence', `${scenario.case_id}.${laneId}.${name}`,
            'the raw lane report differs from its recorded digest or length');
      }
      const variants = [...new Set([
        ...run.requests.keys(), ...run.responses.keys(),
      ])].sort((left, right) => left.localeCompare(right, 'en'));
      if (stableJson(variants) !== stableJson(recorded.variants))
        fail('derivation-drift', `${scenario.case_id}.${laneId}.variants`,
          'the retained variant identities differ from the raw request/response reports');
    }
    for (const entry of scenario.event_binding?.files ?? []) {
      const file = path.join(directory, entry.path);
      if (!fs.existsSync(file)) {
        fail('missing-evidence', entry.path, 'a bound event file is missing');
        continue;
      }
      if (sha256(fs.readFileSync(file)) !== entry.sha256)
        fail('event-binding-mismatch', entry.path,
          'an edit or effect binding changed: the comparison evidence is invalid');
    }
    const reconstructed = scenario.event_binding?.files ?? [];
    const recomputedBinding = sha256(reconstructed.map(entry => entry.sha256).join('\n'));
    if (recomputedBinding !== scenario.event_binding?.sha256)
      fail('event-binding-mismatch', scenario.case_id,
        'the recorded event binding does not cover its own listed files');
    for (const variant of scenario.edits?.variants ?? []) {
      const candidates = [...(scenario.event_binding?.files ?? [])]
        .filter(entry => entry.path.endsWith(`${variant.variant}.requests.jsonl`));
      if (candidates.length !== 1) continue;
      const file = path.join(directory, candidates[0].path);
      if (!fs.existsSync(file)) continue;
      if (sha256(fs.readFileSync(file)) !== variant.requests_sha256)
        fail('edit-binding-mismatch', candidates[0].path,
          'the retained edits differ from the recorded edit digest');
    }
    try {
      const observations = declaration.observe(bundle);
      const expectations = evaluateExpectations(declaration, observations);
      const expectedPrefixes = declaration.effectRows(bundle);
      if (stableJson(observations) !== stableJson(scenario.observations))
        fail('derivation-drift', scenario.case_id,
          'the retained reports no longer derive the recorded observation');
      if (stableJson(expectedPrefixes) !== stableJson(scenario.effect_prefixes))
        fail('derivation-drift', `${scenario.case_id}.effect_prefixes`,
          'the retained reports no longer derive the recorded effect prefixes');
      for (const row of expectations) {
        const recorded = (scenario.expectations ?? [])
          .find(entry => entry.expectation_id === row.expectation_id);
        if (!recorded) {
          fail('expectation-drift', row.expectation_id, 'a declared expectation is missing');
          continue;
        }
        if (row.status !== recorded.status)
          fail('expectation-drift', row.expectation_id,
            `recorded ${recorded.status}, recomputed ${row.status}`);
        if (stableJson(row.declared) !== stableJson(recorded.declared))
          fail('expectation-drift', row.expectation_id,
            'the expected value differs from the frozen declaration');
        if (stableJson(row.observed) !== stableJson(recorded.observed))
          fail('derivation-drift', row.expectation_id,
            'the observed value no longer derives from the retained reports');
      }
      const admissionOutcome = expectations.map(row => ({
        expectation_id: row.expectation_id,
        status: row.status,
        observed: row.observed,
      }));
      if (stableJson(scenario.timing?.bound_to?.admission_outcome)
        !== stableJson(admissionOutcome))
        fail('timing-unbound', `${scenario.case_id}.timing.bound_to.admission_outcome`,
          'timing does not bind the recomputed admission and expectation outcomes');
      const comparison = classifyScenario(expectations,
        declaration.lanes.map(id => ({ id, exit: scenario.lanes
          .find(lane => lane.id === id)?.exit ?? 0,
        report_valid: hasExpectedReport(id, bundle.lanes.get(id)?.report?.kind) })));
      if (comparison.result_class !== scenario.comparison?.result_class)
        fail('comparison-drift', scenario.case_id,
          `recorded ${scenario.comparison?.result_class}, recomputed ${comparison.result_class}`);
    } catch (error) {
      fail('derivation-drift', scenario.case_id, `re-derivation failed: ${error.message}`);
    }
  }

  // The receipt binds itself over everything except its own binding field.
  if (receipt.binding_sha256 !== undefined) {
    const copy = { ...receipt };
    delete copy.binding_sha256;
    if (digestValue(copy) !== receipt.binding_sha256)
      fail('receipt-binding-mismatch', 'receipt.json',
        'the receipt body does not match its own binding digest');
  }
  if (!Number.isFinite(Date.parse(receipt.timing?.started_at ?? ''))
    || !Number.isFinite(Date.parse(receipt.timing?.ended_at ?? ''))
    || !Number.isFinite(receipt.timing?.duration_ms)
    || receipt.timing?.bound_to?.source_commit !== receipt.source?.commit
    || !Array.isArray(receipt.timing?.bound_to?.scenarios)
    || receipt.timing.bound_to.scenarios.length === 0)
    fail('timing-unbound', 'receipt.timing',
      'the receipt timing is not bound to the measured revision and its scenarios');
  const admissionOutcomes = Object.fromEntries((corpus.scenarios ?? []).map(scenario => [
    scenario.case_id, scenario.timing?.bound_to?.admission_outcome,
  ]));
  if (stableJson(receipt.timing?.bound_to?.admission_outcomes) !== stableJson(admissionOutcomes))
    fail('timing-unbound', 'receipt.timing.bound_to.admission_outcomes',
      'receipt timing does not bind every scenario admission outcome');
  return { failures };
}
