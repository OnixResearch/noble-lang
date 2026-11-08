# Module-root retention for saved named callers (OWN-CASE-06)

`.cairn/changes/ownership-lifetime-contract` designs OWN-CASE-06. Module M defines
`def h [ 1 + ]` and `def g [ h ]`. A saved Program from M's compiled code calls `g`. M's
version is then retired, its module roots are released, and the saved owner is invoked. The
case passes if the invocation returns the checked result, or if retirement is refused while the
owner lives. A stale-handle trap is a failure.

## What the runtime did

The design inferred the failure from source, and running the case confirmed it.
1. **The defect.** The slot engine's `sweepRetirement` released a published module's roots as
   soon as its version, composite and replay owners were gone. It never asked whether a saved
   owner's Program still entered the module's code.
2. **Why the owner's edges don't help.** A saved Program's compiled body reads named definitions
   through module globals, and no cell edge records that read. Releasing the roots therefore
   freed `g`'s and `h`'s cells while the saved `[ g ]` lived.
3. **Why the code stayed callable.** The code span was not retired, because the saved cell still
   counted as a code owner.
4. **The result.** After M's published root was superseded, invoking the saved owner trapped:
   `execution-failed`, `RuntimeError: unreachable`, and the session terminated.

Two controls isolate that edge:

| Control | Outcome on the prior CLI |
|---|---|
| a saved static child that calls no named definition (`[ 1 + ]`) | survives supersession and returns 42 |
| the `[ g ]` child without supersession | returns 42 |

## The decision and the fix

The design leaves two ways to restore the lifetime: count code→global owner edges in the
backend, or extend module lifetime to saved callers. This change takes the second.

`sweepRetirement` now keeps a module's roots while any saved owner's graph enters its code. The
check is `savedOwnerReaches`, the same predicate `discard` and `retireCode` already apply, now
shared by all three. `releaseProgram` already re-sweeps the modules a released owner reached, so
the module retires right after its last saved caller is released.

The trade-off is retention. A module whose saved caller outlives its publication stays resident,
roots and code, until that caller is released. The alternative needs owner edges from compiled
code to the globals it reads, which the backend counters cannot express today.

## Evidence

[`receipt.mjs`](receipt.mjs) runs `crates/noble-cli/tests/live_slot_module_root_retention.mjs`
against two CLIs. The retained run is [`receipt.json`](receipt.json), with the lane output of each CLI in
[`fixed.stdout`](fixed.stdout) and [`prior.stdout`](prior.stdout); see its "source" field for
the exact commit.

| CLI | Observed |
|---|---|
| fixed, built from that clean commit | The lane passes: the saved `[ g ]` returns 42 after supersession. M's span is not retirable while the owner lives, becomes retirable when the owner is released, and the released owner is refused. |
| prior, built from the commit before the fix | The lane fails at the saved owner's invocation with `execution-failed` / `RuntimeError: unreachable`. |

The lane also runs as `cargo test -p noble-cli --test live_slot_direct_quote`, whose runner hosts it
as a second test so the compiler architecture keeps its reviewed unit count.

## Non-claims

The lane covers one module, one saved static Program and one supersession. Other retirement
routes are not exercised here: `discard` and `retireCode` already refused while a saved owner
reached the module. The prior binary is bound by its hash and its declared commit, which the
receipt does not rebuild. Nothing here promotes a conformance case.
