import assert from 'node:assert/strict';
import { cli, expectOutcome, selectedAuthority } from './live_slot_resource_ref_cli.mjs';

// OWN-CASE-02 and OWN-CASE-03 (`.cairn/changes/ownership-lifetime-contract`) against the live-slot
// runtime. OWN-CASE-01 runs in `live_slot_direct_quote.mjs` and OWN-CASE-06 in
// `live_slot_module_root_retention.mjs`.
//   02: `[ 1 + ] dup` saves two owners of one Program; releasing them in either order leaves the
//       other runnable after the first release, and refuses both after the second.
//   03: an 8-leaf compose tree, each leaf and intermediate saved; after every owner except the
//       root's is released, the root still runs and gives the same result. The host's observation
//       of an anonymous saved Program is bounded (1000 bytes, 64 nodes), so a left-deep chain is
//       refused at its fifth compose: the refusal is `execution-failed` with that diagnostic, and
//       the session terminates, as every failed root does in this profile.
//   cross-module: a Program saved by one installed module is duplicated by a second one; both
//       owners must run the original Program and release independently (issue #106).
const binary = process.argv[2];
const P = 'Program<I64,I64,pure>';
const sources = { dupper: '[ 1 + ] dup', leaf: 'drop [ 1 + ]', composer: 'compose', runner: 'run',
  maker: '[ 1 + ]', passer: 'dup' };
const installs = [['dupper', []], ['leaf', ['I64']], ['composer', [P, P]], ['runner', ['I64', P]],
  ['maker', []], ['passer', [P]]];
const program = owner => ({ kind: 'program', owner });
const i64 = value => ({ kind: 'i64', value: String(value) });

async function session(variant, body) {
  const live = await cli(binary, variant, selectedAuthority({ sources, slots: [], effects: [] }));
  const issue = async (request, outcome) => expectOutcome(await live.issue(request), outcome);
  let complete = false;
  try {
    expectOutcome(await live.configured, 'configured');
    for (const [id, inputs] of installs) {
      await issue({ operation: 'install', id, source: sources[id], inputs }, 'installed');
    }
    const run = (owner, input = 0) => live.issue({ operation: 'invoke', id: 'runner',
      inputs: [i64(input), program(owner)], refs: [] });
    const result = await body({ live, issue, run });
    const exit = await live.close();
    complete = true;
    return { ...result, exit, selected_binary_sha256: live.selectedBinarySha256 };
  } finally { if (!complete) await live.abort(); }
}

const refused = row => row.outcome === 'refused';

async function sharedDuplicates(order) {
  return session(`OWN-CASE-02-${order}`, async ({ issue, run }) => {
    const made = await issue({ operation: 'invoke', id: 'dupper', inputs: [], refs: [] }, 'executed');
    assert.equal(made.stack.length, 2);
    const owners = made.stack.map(value => value.owner);
    assert.notEqual(owners[0], owners[1], 'each duplicate gets its own owner');
    const [first, second] = order === 'A-then-B' ? owners : [...owners].reverse();
    for (const owner of owners) assert.deepEqual(expectOutcome(await run(owner, 41), 'executed').stack, [{ kind: 1, value: '42' }]);
    await issue({ operation: 'release-program', owner: first }, 'program-released');
    assert.ok(refused(await run(first, 41)), 'a released owner is refused');
    const survivor = expectOutcome(await run(second, 41), 'executed');
    assert.deepEqual(survivor.stack, [{ kind: 1, value: '42' }], 'the other owner still runs');
    assert.ok(refused(await issue({ operation: 'release-program', owner: first }, 'refused')));
    await issue({ operation: 'release-program', owner: second }, 'program-released');
    assert.ok(refused(await run(second, 41)), 'both owners are refused after the second release');
    return { order, owners: [first, second], survivor: survivor.stack };
  });
}

async function balancedTree() {
  return session('OWN-CASE-03-balanced', async ({ issue, run }) => {
    const leaves = [];
    for (let k = 0; k < 8; k += 1) {
      leaves.push((await issue({ operation: 'invoke', id: 'leaf', inputs: [i64(k)], refs: [] }, 'executed')).stack[0].owner);
    }
    const intermediates = [];
    let level = leaves;
    while (level.length > 1) {
      const next = [];
      for (let at = 0; at < level.length; at += 2) {
        const made = await issue({ operation: 'invoke', id: 'composer',
          inputs: [program(level[at]), program(level[at + 1])], refs: [] }, 'executed');
        next.push(made.stack[0].owner);
      }
      intermediates.push(...next);
      level = next;
    }
    const [root] = level;
    const before = expectOutcome(await run(root), 'executed');
    assert.deepEqual(before.stack, [{ kind: 1, value: '8' }]);
    const others = [...leaves, ...intermediates.filter(owner => owner !== root)];
    for (const owner of others) await issue({ operation: 'release-program', owner }, 'program-released');
    const after = expectOutcome(await run(root), 'executed');
    assert.deepEqual(after.stack, before.stack, 'the root runs alike after every other owner is released');
    for (const owner of others) assert.ok(refused(await run(owner)), `released ${owner} is refused`);
    await issue({ operation: 'release-program', owner: root }, 'program-released');
    assert.ok(refused(await run(root)), 'the root is refused after its release');
    return { leaves: leaves.length, intermediates: intermediates.length, released_before_root: others.length,
      before: before.stack, after: after.stack };
  });
}

async function deepChain() {
  return session('OWN-CASE-03-left-deep', async ({ live, issue, run }) => {
    const leaves = [];
    for (let k = 0; k < 8; k += 1) {
      leaves.push((await issue({ operation: 'invoke', id: 'leaf', inputs: [i64(k)], refs: [] }, 'executed')).stack[0].owner);
    }
    let chain = leaves[0];
    let composed = 0;
    for (const leaf of leaves.slice(1)) {
      const row = await live.issue({ operation: 'invoke', id: 'composer', inputs: [program(chain), program(leaf)], refs: [] });
      if (row.outcome !== 'executed') {
        assert.equal(row.outcome, 'execution-failed');
        assert.match(row.diagnostic, /exceed bounded exact trace representation/);
        assert.equal(row.session_state, 'terminated-after-guest-failure');
        assert.equal(row.guest_requests, 0);
        return { refused_at_compose: composed + 1, diagnostic: row.diagnostic, session_state: row.session_state };
      }
      composed += 1;
      chain = row.stack[0].owner;
      assert.deepEqual(expectOutcome(await run(chain), 'executed').stack, [{ kind: 1, value: String(composed + 1) }]);
    }
    assert.fail('the left-deep chain stayed within the observation bound');
    return null;
  });
}

async function crossModuleAlias() {
  return session('cross-module-saved-alias', async ({ issue, run }) => {
    const made = await issue({ operation: 'invoke', id: 'maker', inputs: [], refs: [] }, 'executed');
    assert.equal(made.stack.length, 1);
    const original = made.stack[0].owner;
    const passed = await issue({ operation: 'invoke', id: 'passer',
      inputs: [program(original)], refs: [] }, 'executed');
    assert.equal(passed.stack.length, 2);
    assert.notEqual(passed.stack[0].owner, passed.stack[1].owner, 'each duplicate gets its own owner');
    for (const row of passed.stack) {
      assert.deepEqual(expectOutcome(await run(row.owner, 41), 'executed').stack, [{ kind: 1, value: '42' }]);
    }
    await issue({ operation: 'release-program', owner: passed.stack[0].owner }, 'program-released');
    assert.deepEqual(expectOutcome(await run(passed.stack[1].owner, 41), 'executed').stack, [{ kind: 1, value: '42' }]);
    await issue({ operation: 'release-program', owner: passed.stack[1].owner }, 'program-released');
    await issue({ operation: 'release-program', owner: original }, 'program-released');
    assert.ok(refused(await run(passed.stack[1].owner, 41)), 'a released duplicate is refused');
    return { owners: [original, ...passed.stack.map(row => row.owner)] };
  });
}

const cases = {
  'OWN-CASE-02': [await sharedDuplicates('A-then-B'), await sharedDuplicates('B-then-A')],
  'cross-module-alias': await crossModuleAlias(),
  'OWN-CASE-03': { balanced: await balancedTree(), left_deep: await deepChain() },
};
for (const row of cases['OWN-CASE-02']) assert.equal(row.exit, 0);
assert.equal(cases['cross-module-alias'].exit, 0);
assert.equal(cases['OWN-CASE-03'].balanced.exit, 0);
assert.equal(cases['OWN-CASE-03'].left_deep.refused_at_compose, 5);
assert.equal(cases['OWN-CASE-03'].left_deep.exit, 2, 'a failed root terminates the session');
console.log(JSON.stringify({ cases }));
