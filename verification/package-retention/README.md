# Package retention across slot replacement (LREF-CASE-03)

`.cairn/changes/live-reference-lifetimes` designs LREF-CASE-03. Slot S's version v1 is module
M1's root, whose body returns `[ h ]`; the compiled body reads `def h [ 1 + ]` through an M1
global. A root R dispatches to v1 through `slot.invoke` and saves the returned Program O. Then v2
replaces v1, and the module sweep runs. The case expects `20 O run` to yield 21, and M1's span to
stay live while O lives. Once O is released, the span retires and the retained-version charge is
reclaimed. A stale-handle trap fails the case.

## What the runtime did

The OWN-CASE-06 fix already kept M1's module roots and code alive for O, so `20 O run` returned 21
after replacement, and M1 retired when O was released. The charge was the gap.
1. **The design.** LREF-PKG-01 makes the retained unit a version's whole package, with saved
   owners among its owners, and capacity admission counts packages rather than single cells.
2. **The defect.** The engine told the registry only about versions the saved graph reached *as
   cells*. O is `[ h ]`, a static Program of M1 but not v1's own cell, so v1 was never charged to
   O, even though O kept M1's code alive.
3. **The consequence.** With a retained-version quota of 1, v2 was admitted while O still held M1,
   and any number of superseded modules could stay resident behind saved results without being
   charged.

## The fix

When the engine saves a Program, it already computes the modules whose code the saved graph
enters; OWN-CASE-06 uses that set to keep their roots alive. The registry is now also charged, for
the owner's lifetime, for every known version whose cell lives in one of those modules, in addition
to the versions the graph reaches as cells. `releaseProgram` releases exactly the versions charged
at save, so the release path is unchanged.

The trade-off is stricter admission. A saved result can now refuse a publication that the
cell-only charge admitted, because it keeps the whole package of every version it enters.

## The lane

`crates/noble-cli/tests/live_slot_package_retention.mjs` runs two sessions. R completes before the
publication, so the publication's sweep finds O as the only owner of M1.

| Session | Quota | Expected |
|---|---|---|
| `replacement` | 2 | v2 is published while O lives without retiring M1; `20 O run` yields 21; releasing O retires M1's span; O is refused afterwards |
| `charge` | 1 | v2's publication is `retention-budget-refused` while O lives; `20 O run` yields 21; after O is released, v2 is published and M1's span retires |

The lane also runs as `cargo test -p noble-cli --test live_slot_direct_quote`, whose runner hosts
it as a third test so the compiler architecture keeps its reviewed unit count.
[`receipt.mjs`](receipt.mjs) runs it against a CLI built from a clean commit and against one built
from the commit before the fix.
