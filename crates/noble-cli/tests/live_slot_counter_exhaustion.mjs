import assert from 'node:assert/strict';
import { SlotRegistry } from '../src/core/runtime/slot-registry.mjs';

// LREF-CASE-02 (`.cairn/changes/live-reference-lifetimes`, r[LREF-CTR-01], integrator decision
// 47): live-slot counters never wrap. In process, three registries start from fixtures at a
// counter's maximum: the global epoch, slot S's generation and S's incarnation. Each would-be
// publication, deletion or recreation refuses with its own exhaustion diagnostic and leaves the
// registry unchanged; a root pinned before the refusal still dispatches to the version it pinned;
// no earlier counter value is reissued. The live-slot code-retention counter, the compiler's
// install generation, is exercised by noble-wasm's unit test at `u32::MAX`. No CLI input reaches
// these maxima, so this lane needs no CLI.
const U64_MAX = 0xffffffffffffffffn;
const digest = 'a'.repeat(64);
const descriptor = { input: ['I64'], output: ['I64'], effectCeiling: [] };
const checked = name => ({ programValueId: `P-${name}`, definitionId: `D-${name}`, captures: [],
  interface: descriptor, effects: [], semanticContext: 'C1', recipeSha256: digest, claim: 'Q1',
  assumptions: 'A1', artifactSha256: digest, resourceCatalog: [] });

function fixture(counterFixture) {
  const policy = new Set(['publish:S', 'dispatch:S', 'delete:S']);
  const registry = new SlotRegistry({ quota: 8, maxSlots: 128, knownEffects: [],
    authorize: request => policy.has(`${request.operation}:${request.slotId}`),
    performEffect: () => '', previewEffect: () => '', retireTarget: () => true,
    retainProgram: () => true, releaseProgramBackend: () => true, verifyEvidence: () => false,
    counterFixture });
  const module = registry.registerModule(digest, [{ site_id: 3, target_input: ['I64'],
    target_output: ['I64'], effect_ceiling: [], selected_ref_logical_position: 1,
    forwarded_source_positions: [], forwarded_target_positions: [] }]);
  const memory = new WebAssembly.Memory({ initial: 20, maximum: 20 });
  const view = new DataView(memory.buffer);
  // A root R pinned now; `select` enters its frame and dispatches S through its borrowed ref.
  const pin = () => {
    const root = registry.pinRoot(900, module);
    registry.bindRef(root, 1, 7, 'S', descriptor);
    assert.equal(registry.validateBorrowBinding(root, module, 1, 7), 1);
    return root;
  };
  const select = root => {
    view.setUint32(1050632, 1, true);
    for (const [offset, value] of [[0, 0], [4, 900], [8, 1], [12, 0], [16, 1], [20, 7]]) {
      view.setUint32(1077248 + offset, value, true);
    }
    registry.enterFrame(root, module, -1, 1077248, 900, memory);
    return registry.dispatch(root, 7, module, 3);
  };
  return { registry, pin, select,
    admit: (name, handle) => registry.admitCandidate(checked(name), handle, digest) };
}
const published = (row, epoch, incarnation, generation) => {
  assert.equal(row.outcome, 'published', JSON.stringify(row, (key, value) =>
    typeof value === 'bigint' ? value.toString() : value));
  assert.deepEqual([row.epoch, row.incarnation, row.generation], [epoch, incarnation, generation]);
};
const refused = (row, outcome, epoch) => {
  assert.equal(row.outcome, outcome);
  assert.equal(row.epoch, epoch, 'the registry is unchanged');
  return outcome;
};
const text = value => typeof value === 'bigint' ? value.toString() : value;

// Global epoch: v1 commits the last epoch; publication and deletion both refuse.
const epoch = (() => {
  const { registry, pin, select, admit } = fixture({ epoch: U64_MAX - 1n });
  published(registry.publish({ slotId: 'S', expectedEpoch: U64_MAX - 1n, interface: descriptor,
    version: admit('S1', 101) }), U64_MAX, 1n, 1n);
  const root = pin();
  const v2 = admit('S2', 102);
  const publication = refused(registry.publish({ slotId: 'S', expectedEpoch: U64_MAX,
    expectedIncarnation: 1n, expectedGeneration: 1n, version: v2 }), 'epoch-exhausted', U64_MAX);
  const deletion = refused(registry.delete({ slotId: 'S', expectedEpoch: U64_MAX,
    expectedIncarnation: 1n, expectedGeneration: 1n }), 'epoch-exhausted', U64_MAX);
  assert.equal(select(root).handle, 101, 'R still selects v1');
  return { publication, deletion, epoch_after: text(registry.epoch), pinned_root_selects: 101 };
})();

// Slot generation: S's first publication receives the last generation; its replacement refuses.
const generation = (() => {
  const { registry, pin, select, admit } = fixture({ firstGenerations: [['S', U64_MAX]] });
  published(registry.publish({ slotId: 'S', expectedEpoch: 0n, interface: descriptor,
    version: admit('S1', 101) }), 1n, 1n, U64_MAX);
  const root = pin();
  const publication = refused(registry.publish({ slotId: 'S', expectedEpoch: 1n,
    expectedIncarnation: 1n, expectedGeneration: U64_MAX, version: admit('S2', 102) }),
  'generation-exhausted', 1n);
  assert.equal(select(root).handle, 101, 'R still selects v1');
  return { publication, epoch_after: text(registry.epoch), pinned_root_selects: 101 };
})();

// Slot incarnation: S holds the last incarnation; after its deletion, recreation refuses.
const incarnation = (() => {
  const { registry, pin, select, admit } = fixture({ incarnations: [['S', U64_MAX - 1n]] });
  published(registry.publish({ slotId: 'S', expectedEpoch: 0n, interface: descriptor,
    version: admit('S1', 101) }), 1n, U64_MAX, 1n);
  const root = pin();
  const deleted = registry.delete({ slotId: 'S', expectedEpoch: 1n, expectedIncarnation: U64_MAX,
    expectedGeneration: 1n });
  assert.deepEqual([deleted.outcome, deleted.epoch], ['deleted', 2n]);
  const recreation = refused(registry.publish({ slotId: 'S', expectedEpoch: 2n,
    interface: descriptor, version: admit('S2', 102) }), 'incarnation-exhausted', 2n);
  assert.equal(select(root).handle, 101, 'R, pinned before the deletion, still selects v1');
  return { recreation, epoch_after: text(registry.epoch), pinned_root_selects: 101 };
})();

assert.equal(new Set([epoch.publication, generation.publication, incarnation.recreation]).size, 3,
  'each counter refuses with its own diagnostic');
console.log(JSON.stringify({ case: 'LREF-CASE-02', epoch, generation, incarnation,
  code_retention: 'compiler install generation at u32::MAX: noble-wasm unit test' }));
