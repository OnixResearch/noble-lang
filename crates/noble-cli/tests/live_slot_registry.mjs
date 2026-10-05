import assert from 'node:assert/strict';
import { SlotRegistry } from '../src/core/runtime/slot-registry.mjs';

const digest = 'a'.repeat(64);
const descriptor = (input = ['I64'], output = ['I64'], effectCeiling = []) =>
  ({ input, output, effectCeiling });
const policy = new Map();
const performed = [];
const released = [];
const grants = (operation, slotId, allowed) => policy.set(`${operation}:${slotId}`, allowed);
const host = quota => new SlotRegistry({
  quota, knownEffects: ['test.emit'],
  authorize: request => policy.get(`${request.operation}:${request.slotId ?? request.nominalId}`) === true,
  performEffect: (effect, text, version) => {
    performed.push(`${version.programValueId}:${effect}:${text}`);
    return `ok-${text}`;
  },
  retireTarget: (handle, version) => { released.push(`${handle}:${version.programValueId}`); return true; },
  // Ledger unit test only: compiled Program retention is exercised by the
  // separate real-Wasm child, never inferred from these callbacks.
  retainProgram: () => true,
  releaseProgramBackend: () => true,
  verifyEvidence: record => record.accepted === true,
});
const checked = (name, effects = [], input = ['I64'], output = ['I64']) => ({
  programValueId: `P-${name}`, definitionId: `D-${name}`, captures: [`I64:${name}`],
  interface: descriptor(input, output), effects, semanticContext: 'C1',
  claim: 'Q1', assumptions: 'A1', artifactSha256: digest, resourceCatalog: [],
});
const admit = (registry, name, handle, effects, input, output) =>
  registry.admitCandidate(checked(name, effects, input, output), handle, digest);
const site = (input, output, effectCeiling, borrowedInputPosition = 1) =>
  ({ site_id: 3, target_input: input, target_output: output, effect_ceiling: effectCeiling,
    selected_ref_logical_position: borrowedInputPosition,
    forwarded_source_positions: [], forwarded_target_positions: [] });
const memory = new WebAssembly.Memory({ initial: 20, maximum: 20 });
const view = new DataView(memory.buffer);
const FRAME = 1052672;
const frame = (address, parent, handle, pairs) => {
  view.setUint32(address, parent, true);
  view.setUint32(address + 4, handle, true);
  view.setUint32(address + 8, pairs.length, true);
  view.setUint32(address + 12, 0, true);
  pairs.forEach(([position, ordinal], index) => {
    view.setUint32(address + 16 + index * 8, position, true);
    view.setUint32(address + 20 + index * 8, ordinal, true);
  });
};

// One pinned root keeps its old B target across publication, and the SAME
// borrowed ref selects B twice. The next root sees the newly published target.
{
  const registry = host(3);
  grants('publish', 'B', false); grants('dispatch', 'B', true); grants('effect', 'B', true);
  const contract = descriptor(['I64'], ['I64'], ['test.emit']);
  const b1 = admit(registry, 'B1', 101, ['test.emit']);
  const b2 = admit(registry, 'B2', 102, ['test.emit']);
  const module = registry.registerModule(digest,
    [site(contract.input, contract.output, contract.effectCeiling)]);
  const genericReflection = registry.reflectSite(module, 3);
  assert.deepEqual(genericReflection, { instruction: 'slot.invoke', siteId: 3,
    input: ['I64'], output: ['I64'], effects: ['test.emit', 'live.dispatch'] });
  assert.equal(registry.publish({ slotId: 'B', expectedEpoch: 0n, version: b1,
    interface: contract }).outcome, 'policy-denied');
  grants('publish', 'B', true);
  assert.deepEqual(registry.publish({ slotId: 'B', expectedEpoch: 0n, version: b1,
    interface: contract }), { outcome: 'published', epoch: 1n, incarnation: 1n, generation: 1n });
  assert.equal(released.includes('102:P-B2'), false, 'staged code has a separate physical owner');
  const oldRoot = registry.pinRoot(900, module);
  registry.bindRef(oldRoot, 1, 7, 'B', contract);
  assert.equal(registry.validateBorrowBinding(oldRoot, module, 0, 7), 0);
  view.setUint32(1050632, 1, true);
  frame(FRAME, 0, 900, [[1, 7]]);
  assert.throws(() => registry.enterFrame(oldRoot, module, -1, FRAME, 900, memory),
    /root input borrowing/, 'a rejected binding must not mutate the host frame ledger');
  assert.equal(registry.validateBorrowBinding(oldRoot, module, 1, 7), 1);
  assert.equal(registry.validateBorrowBinding(oldRoot, module, 1, 7), 0);
  frame(FRAME, 0, 900, [[1, 8]]);
  assert.throws(() => registry.enterFrame(oldRoot, module, -1, FRAME, 900, memory), /forged/);
  frame(FRAME, 0, 900, [[1, 7]]);
  const oldFrame = registry.enterFrame(oldRoot, module, -1, FRAME, 900, memory);
  assert.throws(() => registry.dispatch(oldRoot, 7, module, 99), /checked dispatch site/);
  assert.throws(() => registry.dispatch(oldRoot, 7,
    registry.registerModule(digest, [site(['Bool'], ['Bool'], [])]), 3), /checked dispatch site/);
  const first = registry.dispatch(oldRoot, 7, module, 3);
  assert.equal(first.handle, 101);
  frame(FRAME + 64, FRAME, 101, []);
  const firstFrame = registry.enterFrame(oldRoot, module, 3, FRAME + 64, 101, memory);
  assert.equal(registry.requestEffect(oldRoot, firstFrame, first.handle, 'test.emit', 'A').outcome, 'performed');
  registry.leaveFrame(oldRoot, firstFrame);
  assert.equal(registry.publish({ slotId: 'B', expectedEpoch: 1n,
    expectedIncarnation: 1n, expectedGeneration: 1n, version: b2 }).epoch, 2n);
  const second = registry.dispatch(oldRoot, 7, module, 3);
  assert.equal(second.handle, 101);
  frame(FRAME + 64, FRAME, 101, []);
  const secondFrame = registry.enterFrame(oldRoot, module, 3, FRAME + 64, 101, memory);
  registry.leaveFrame(oldRoot, secondFrame);
  assert.equal(registry.rootEpoch(oldRoot), 1n);
  const nextRoot = registry.pinRoot(901, module);
  registry.bindRef(nextRoot, 1, 7, 'B', contract);
  assert.equal(registry.validateBorrowBinding(nextRoot, module, 1, 7), 1);
  frame(FRAME + 128, 0, 901, [[1, 7]]);
  const nextFrame = registry.enterFrame(nextRoot, module, -1, FRAME + 128, 901, memory);
  assert.throws(() => registry.requestEffect(nextRoot, nextFrame, 101, 'test.emit', 'wrong-root'), /not selected/);
  assert.equal(registry.dispatch(nextRoot, 7, module, 3).handle, 102);
  frame(FRAME + 192, FRAME + 128, 102, []);
  const selectedNextFrame = registry.enterFrame(nextRoot, module, 3, FRAME + 192, 102, memory);
  assert.throws(() => registry.requestEffect(nextRoot, nextFrame, 102, 'test.emit', 'wrong-frame'), /not selected/);
  registry.leaveFrame(nextRoot, selectedNextFrame);
  assert.deepEqual(registry.reflectSite(module, 3), genericReflection);
  grants('dispatch', 'B', false);
  assert.equal(registry.dispatch(oldRoot, 7, module, 3).outcome, 'policy-denied');
  grants('dispatch', 'B', true); grants('effect', 'B', false);
  const oldAgain = registry.dispatch(oldRoot, 7, module, 3);
  frame(FRAME + 64, FRAME, 101, []);
  const deniedFrame = registry.enterFrame(oldRoot, module, 3, FRAME + 64, 101, memory);
  assert.deepEqual(registry.requestEffect(oldRoot, deniedFrame, oldAgain.handle, 'test.emit', 'denied'),
    { outcome: 'denied', protectedOperations: 1 });
  registry.leaveFrame(oldRoot, deniedFrame);
  assert.deepEqual(performed, ['P-B1:test.emit:A']);
  assert.equal(registry.trace(oldRoot).filter(row => row.operation === 'dispatch' && row.outcome === 'allowed').length, 3);
  assert.deepEqual(registry.trace(oldRoot).filter(row => row.operation === 'effect')
    .map(row => [row.outcome, row.response ?? null, row.protectedOperations]),
  [['performed', 'ok-A', 1], ['denied', null, 1]]);
  assert.throws(() => registry.bindRef(nextRoot, 1, 7, 'B', contract), /duplicate/);
  registry.leaveFrame(oldRoot, oldFrame);
  assert.throws(() => registry.enterFrame(oldRoot, module, -1, FRAME, 900, memory),
    /unselected or stale/);
  registry.finishRoot(oldRoot);
  assert.ok(released.includes('101:P-B1'));
  registry.leaveFrame(nextRoot, nextFrame);
  registry.finishRoot(nextRoot);
  assert.throws(() => registry.dispatch(oldRoot, 7, module, 3), /inactive/);
  const trapped = registry.pinRoot(902, module);
  registry.abortRoot(trapped);
  assert.throws(() => registry.rootEpoch(trapped), /inactive/);
}

// A→B→C forwards independently issued root borrows through checked frame
// positions. Publishing C-v2 after A starts cannot change old-root C-v1.
{
  const registry = host(4);
  const ai = descriptor(['I64', 'LiveRef<B>', 'LiveRef<C>'], ['I64']);
  const bi = descriptor(['I64', 'LiveRef<C>'], ['I64']);
  const ci = descriptor();
  for (const slot of ['A', 'B', 'C']) {
    grants('publish', slot, true); grants('dispatch', slot, true);
  }
  const a = admit(registry, 'nested-A', 601, [], ai.input, ai.output);
  const b = admit(registry, 'nested-B', 602, [], bi.input, bi.output);
  const c1 = admit(registry, 'nested-C1', 603);
  const c2 = admit(registry, 'nested-C2', 604);
  assert.equal(registry.publish({ slotId: 'A', expectedEpoch: 0n, interface: ai, version: a }).epoch, 1n);
  assert.equal(registry.publish({ slotId: 'B', expectedEpoch: 1n, interface: bi, version: b }).epoch, 2n);
  assert.equal(registry.publish({ slotId: 'C', expectedEpoch: 2n, interface: ci, version: c1 }).epoch, 3n);
  const module = registry.registerModule(digest, [
    { site_id: 1, selected_ref_logical_position: 1, target_input: ai.input,
      target_output: ai.output, effect_ceiling: [],
      forwarded_source_positions: [2, 3], forwarded_target_positions: [1, 2] },
    { site_id: 2, selected_ref_logical_position: 1, target_input: bi.input,
      target_output: bi.output, effect_ceiling: [],
      forwarded_source_positions: [2], forwarded_target_positions: [1] },
    { site_id: 3, selected_ref_logical_position: 1, target_input: ci.input,
      target_output: ci.output, effect_ceiling: [],
      forwarded_source_positions: [], forwarded_target_positions: [] },
  ]);
  const root = registry.pinRoot(699, module);
  registry.bindRef(root, 1, 1, 'A', ai);
  registry.bindRef(root, 2, 2, 'B', bi);
  registry.bindRef(root, 3, 3, 'C', ci);
  for (const position of [1, 2, 3]) {
    assert.equal(registry.validateBorrowBinding(root, module, position, position), 1);
  }
  view.setUint32(1050632, 3, true);
  frame(FRAME + 512, 0, 699, [[1, 1], [2, 2], [3, 3]]);
  const rootFrame = registry.enterFrame(root, module, -1, FRAME + 512, 699, memory);
  assert.equal(registry.dispatch(root, 1, module, 1).handle, 601);
  frame(FRAME + 576, FRAME + 512, 601, [[1, 2], [2, 3]]);
  const aFrame = registry.enterFrame(root, module, 1, FRAME + 576, 601, memory);
  assert.equal(registry.dispatch(root, 2, module, 2).handle, 602);
  frame(FRAME + 640, FRAME + 576, 602, [[1, 2]]);
  assert.throws(() => registry.enterFrame(root, module, 2, FRAME + 640, 602, memory), /forged/);
  frame(FRAME + 640, FRAME + 576, 602, [[1, 3]]);
  const bFrame = registry.enterFrame(root, module, 2, FRAME + 640, 602, memory);
  assert.equal(registry.publish({ slotId: 'C', expectedEpoch: 3n,
    expectedIncarnation: 1n, expectedGeneration: 1n, version: c2 }).epoch, 4n);
  assert.equal(registry.dispatch(root, 3, module, 3).handle, 603);
  frame(FRAME + 704, FRAME + 640, 603, []);
  const cFrame = registry.enterFrame(root, module, 3, FRAME + 704, 603, memory);
  registry.leaveFrame(root, cFrame); registry.leaveFrame(root, bFrame);
  registry.leaveFrame(root, aFrame); registry.leaveFrame(root, rootFrame);
  assert.deepEqual(registry.trace(root).filter(row => row.operation === 'dispatch')
    .map(row => row.programValueId), ['P-nested-A', 'P-nested-B', 'P-nested-C1']);
  registry.finishRoot(root);
  assert.ok(released.includes('603:P-nested-C1'));
  const fresh = registry.pinRoot(700, module);
  registry.bindRef(fresh, 1, 3, 'C', ci);
  assert.equal(registry.validateBorrowBinding(fresh, module, 1, 3), 1);
  view.setUint32(1050632, 1, true);
  frame(FRAME + 768, 0, 700, [[1, 3]]);
  const freshFrame = registry.enterFrame(fresh, module, -1, FRAME + 768, 700, memory);
  assert.equal(registry.dispatch(fresh, 3, module, 3).handle, 604);
  frame(FRAME + 832, FRAME + 768, 604, []);
  const freshC = registry.enterFrame(fresh, module, 3, FRAME + 832, 604, memory);
  registry.leaveFrame(fresh, freshC);
  registry.leaveFrame(fresh, freshFrame);
  registry.finishRoot(fresh);
}

// The global epoch rejects stale writers for the same and unrelated slots;
// delete/recreate cannot ABA even with an identical visible target ID.
{
  const registry = host(4);
  for (const slot of ['A', 'B']) { grants('publish', slot, true); grants('delete', slot, true); }
  const a1 = admit(registry, 'A1', 201), a2 = admit(registry, 'A2', 202);
  const b1 = admit(registry, 'BB1', 203), b2 = admit(registry, 'BB2', 204);
  const iface = descriptor();
  assert.equal(registry.publish({ slotId: 'A', expectedEpoch: 0n, interface: iface, version: a1 }).epoch, 1n);
  assert.equal(registry.publish({ slotId: 'B', expectedEpoch: 1n, interface: iface, version: b1 }).epoch, 2n);
  assert.equal(registry.publish({ slotId: 'A', expectedEpoch: 2n,
    expectedIncarnation: 1n, expectedGeneration: 1n, version: a2 }).epoch, 3n);
  assert.equal(registry.publish({ slotId: 'B', expectedEpoch: 2n,
    expectedIncarnation: 1n, expectedGeneration: 1n, version: b2 }).outcome, 'stale-reject');
  assert.equal(registry.publish({ slotId: 'A', expectedEpoch: 2n,
    expectedIncarnation: 1n, expectedGeneration: 1n, version: a1 }).outcome, 'stale-reject');
  assert.equal(registry.delete({ slotId: 'A', expectedEpoch: 3n,
    expectedIncarnation: 1n, expectedGeneration: 2n }).epoch, 4n);
  const a1Again = admit(registry, 'A1', 205);
  const recreated = registry.publish({ slotId: 'A', expectedEpoch: 4n, interface: iface, version: a1Again });
  assert.deepEqual([recreated.epoch, recreated.incarnation, recreated.generation], [5n, 2n, 1n]);
  assert.equal(registry.publish({ slotId: 'A', expectedEpoch: 2n,
    expectedIncarnation: 1n, expectedGeneration: 1n, version: a2 }).outcome, 'stale-reject');
  assert.equal(registry.publish({ slotId: 'B', expectedEpoch: 5n,
    expectedIncarnation: 1n, expectedGeneration: 1n, version: b2 }).epoch, 6n);
}

// Rollback is a fresh authorized publication at a later generation, not a
// mutation of the epoch-2 root or a restoration of effects already performed.
{
  const registry = host(2);
  grants('publish', 'rollback', true); grants('dispatch', 'rollback', true);
  const v1 = admit(registry, 'rollback-v1', 251);
  const v2 = admit(registry, 'rollback-v2', 252);
  const iface = descriptor();
  const saved = registry.saveProgram(v1);
  assert.equal(registry.publish({ slotId: 'rollback', expectedEpoch: 0n,
    interface: iface, version: v1 }).generation, 1n);
  assert.equal(registry.publish({ slotId: 'rollback', expectedEpoch: 1n,
    expectedIncarnation: 1n, expectedGeneration: 1n, version: v2 }).epoch, 2n);
  const module = registry.registerModule(digest, [site(iface.input, iface.output, [])]);
  const oldRoot = registry.pinRoot(299, module);
  registry.bindRef(oldRoot, 1, 9, 'rollback', iface);
  assert.equal(registry.validateBorrowBinding(oldRoot, module, 1, 9), 1);
  view.setUint32(1050632, 1, true);
  frame(FRAME + 896, 0, 299, [[1, 9]]);
  const oldFrame = registry.enterFrame(oldRoot, module, -1, FRAME + 896, 299, memory);
  grants('publish', 'rollback', false);
  assert.equal(registry.publish({ slotId: 'rollback', expectedEpoch: 2n,
    expectedIncarnation: 1n, expectedGeneration: 2n, version: v1 }).outcome, 'policy-denied');
  grants('publish', 'rollback', true);
  assert.deepEqual(registry.publish({ slotId: 'rollback', expectedEpoch: 2n,
    expectedIncarnation: 1n, expectedGeneration: 2n, version: v1 }),
  { outcome: 'published', epoch: 3n, incarnation: 1n, generation: 3n });
  assert.equal(registry.dispatch(oldRoot, 9, module, 3).handle, 252);
  frame(FRAME + 960, FRAME + 896, 252, []);
  const selectedOld = registry.enterFrame(oldRoot, module, 3, FRAME + 960, 252, memory);
  registry.leaveFrame(oldRoot, selectedOld);
  registry.leaveFrame(oldRoot, oldFrame);
  registry.finishRoot(oldRoot);
  registry.releaseProgram(saved);
  assert.ok(released.includes('252:P-rollback-v2'));
  assert.equal(registry.epoch, 3n);
}

// A capacity-one replacement refuses while either independent old owner
// remains; after the last owner releases, fresh CAS can retire v1 and commit.
{
  const registry = host(1);
  grants('publish', 'Q', true); grants('dispatch', 'Q', true);
  const v1 = admit(registry, 'Q1', 301), v2 = admit(registry, 'Q2', 302);
  const iface = descriptor();
  assert.equal(registry.publish({ slotId: 'Q', expectedEpoch: 0n, interface: iface, version: v1 }).epoch, 1n);
  const module = registry.registerModule(digest, []);
  const root = registry.pinRoot(1001, module);
  const saved = registry.saveProgram(v1);
  const attempt = () => registry.publish({ slotId: 'Q', expectedEpoch: 1n,
    expectedIncarnation: 1n, expectedGeneration: 1n, version: v2 });
  assert.equal(attempt().outcome, 'retention-budget-refused');
  assert.equal(registry.epoch, 1n);
  registry.finishRoot(root);
  assert.equal(attempt().outcome, 'retention-budget-refused');
  assert.equal(released.includes('301:P-Q1'), false);
  registry.releaseProgram(saved);
  assert.throws(() => registry.releaseProgram(saved), /invalid/);
  assert.equal(attempt().outcome, 'published');
  assert.ok(released.includes('301:P-Q1'));
  assert.equal(registry.retainedVersions, 1);
  assert.equal(registry.publish({ slotId: 'Q', expectedEpoch: 2n,
    expectedIncarnation: 1n, expectedGeneration: 2n, version: v1 }).outcome, 'unadmitted-target');
}

// Nominal resource identity, ordered inputs, ceiling, and exact selected
// evidence refuse independently before publication or candidate execution.
{
  const registry = host(2);
  grants('publish', 'account', true);
  const resourceRows = [
    { module: '7', ordinal: 1, kind: 77, shapeFingerprint: 'b'.repeat(64) },
    { module: '7', ordinal: 2, kind: 78, shapeFingerprint: 'c'.repeat(64) },
  ];
  const sourceSchemas = resourceRows.map(({ module, ordinal }) =>
    ({ module, ordinal, schemaFingerprint: 'd'.repeat(64) }));
  for (const [index, resource] of resourceRows.entries()) {
    grants('register-resource', `${resource.module}:${resource.ordinal}`, true);
    registry.registerNominal({ ...resource, schemaFingerprint: 'd'.repeat(64),
      owner: 'host-account-owner', sourceArtifactSha256: digest,
      interfaceDescriptor: `Account@${index + 1}` });
  }
  assert.throws(() => registry.registerNominal({ ...resourceRows[0],
    shapeFingerprint: 'e'.repeat(64), schemaFingerprint: 'd'.repeat(64),
    owner: 'host-account-owner', sourceArtifactSha256: digest,
    interfaceDescriptor: 'Account@1' }), /duplicate/);
  assert.throws(() => registry.admitResourceCatalog('forged-owner', digest, resourceRows, sourceSchemas), /unregistered/);
  assert.throws(() => registry.admitResourceCatalog('host-account-owner', digest,
    [{ ...resourceRows[0], shapeFingerprint: 'f'.repeat(64) }], [sourceSchemas[0]]), /unregistered/);
  assert.throws(() => registry.admitResourceCatalog('host-account-owner', digest, resourceRows,
    [{ ...sourceSchemas[0], schemaFingerprint: 'e'.repeat(64) }, sourceSchemas[1]]), /unregistered/);
  const catalog = registry.admitResourceCatalog('host-account-owner', digest, resourceRows, sourceSchemas);
  const resourceCandidate = (name, handle, effects, input, output) => {
    const metadata = checked(name, effects, input, output);
    metadata.resourceCatalog = resourceRows;
    metadata.sourceArtifactSha256 = digest;
    return registry.admitCandidate(metadata, handle, digest, catalog);
  };
  const iface = descriptor(['Account@1', 'I64'], ['Account@1', 'I64'], ['test.emit']);
  const unregistered = checked('unregistered', [], iface.input, iface.output);
  unregistered.resourceCatalog = resourceRows;
  unregistered.sourceArtifactSha256 = digest;
  assert.throws(() => registry.admitCandidate(unregistered, 400, digest), /nominal catalog/);
  const good = resourceCandidate('account', 401, ['test.emit'], iface.input, iface.output);
  const reversed = resourceCandidate('reversed', 402, [], ['I64', 'Account@1'], iface.output);
  const wrongSchema = resourceCandidate('schema', 403, [], ['Account@2', 'I64'], iface.output);
  const widened = checked('widened', ['test.emit', 'unknown.effect'], iface.input, iface.output);
  widened.resourceCatalog = resourceRows;
  widened.sourceArtifactSha256 = digest;
  assert.throws(() => registry.admitCandidate(widened, 404, digest, catalog), /unavailable/);
  for (const version of [reversed, wrongSchema]) {
    assert.equal(registry.publish({ slotId: 'account', expectedEpoch: 0n,
      interface: iface, version }).outcome, 'ordered-interface-mismatch');
  }
  const requiredProof = { ...iface, proofRequired: true };
  assert.equal(registry.publish({ slotId: 'account', expectedEpoch: 0n,
    interface: requiredProof, version: good }).outcome, 'selected-target-evidence-refused');
  const exact = { ...good, accepted: true };
  for (const evidence of [{ ...exact, definitionId: 'D-old' }, { ...exact, captures: ['I64:old'] },
    { ...exact, artifactSha256: 'b'.repeat(64) }, { ...exact, semanticContext: 'C-old' },
    { ...exact, assumptions: 'A-old' }, { ...exact, claim: 'Q-old' }]) {
    assert.equal(registry.publish({ slotId: 'account', expectedEpoch: 0n,
      interface: requiredProof, version: good, evidence }).outcome, 'selected-target-evidence-refused');
  }
  assert.equal(registry.publish({ slotId: 'account', expectedEpoch: 0n,
    interface: requiredProof, version: good, evidence: exact }).outcome, 'published');
  assert.equal(registry.epoch, 1n);
  grants('issue-resource', '7:1', true);
  const module = registry.registerModule(digest, [], ['Account@1', 'I64'], catalog, digest);
  const root = registry.pinRoot(450, module);
  assert.throws(() => registry.issueResource(root, 0, 5, '7', 1, 77, 'source-forged-owner'), /unregistered/);
  registry.issueResource(root, 0, 5, '7', 1, 77, 'host-account-owner');
  assert.equal(registry.validateResourceBinding(root, module, 1, 5, 77, 7, 0, 1), 0);
  assert.equal(registry.validateResourceBinding(root, module, 0, 5, 78, 7, 0, 1), 0);
  assert.equal(registry.validateResourceBinding(root, module, 0, 5, 77, 7, 0, 1), 1);
  assert.equal(registry.validateResourceBinding(root, module, 0, 5, 77, 7, 0, 1), 0);
  assert.equal(registry.validateResource(root, 5, 77), 1);
  assert.equal(registry.validateResource(root, 5, 78), 0);
  view.setUint32(1050632, 0, true);
  frame(FRAME + 1024, 0, 450, []);
  const resourceFrame = registry.enterFrame(root, module, -1, FRAME + 1024, 450, memory);
  registry.leaveFrame(root, resourceFrame);
  registry.finishRoot(root);
  assert.equal(registry.validateResource(root, 5, 77), 0);
  registry.discardCandidate(reversed);
  registry.discardCandidate(wrongSchema);
}

// A backend release failure happens AFTER map/epoch publication. It must be
// terminal and explicitly report committed state, never a stale/refused CAS.
{
  const registry = new SlotRegistry({ quota: 1, knownEffects: ['test.emit'],
    authorize: () => true, performEffect: () => 'ok', retireTarget: () => false });
  const v1 = admit(registry, 'fatal-1', 501);
  const v2 = admit(registry, 'fatal-2', 502);
  assert.equal(registry.publish({ slotId: 'fatal', expectedEpoch: 0n,
    interface: descriptor(), version: v1 }).outcome, 'published');
  assert.throws(() => registry.publish({ slotId: 'fatal', expectedEpoch: 1n,
    expectedIncarnation: 1n, expectedGeneration: 1n, version: v2 }), error =>
    error.committed === true && error.epoch === 2n && error.transition === 'publication');
  assert.equal(registry.epoch, 2n);
  assert.throws(() => registry.pinRoot(999), /backend retirement failed/);
}

console.log('live-slot host registry: pin/CAS/authority/quota/nominal/evidence transitions passed');
