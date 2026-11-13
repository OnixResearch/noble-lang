# Vendored interaction-net library (`InteractionNets`)

`InteractionNets.lean` is a byte-identical copy of
[OnixResearch/inets](https://github.com/OnixResearch/inets) at commit
`98bbb01070664e4462a35efa5d62de6f27143572` (provenance and SHA-256 in [`UPSTREAM.json`](UPSTREAM.json)).
It formalises Lafont's interaction nets in Lean 4.35 without Mathlib, for an arbitrary interaction
system: strong confluence, unique normal forms reached in the same number of steps by every
reduction, the reduction bound, observational equivalence and compaction, simultaneous firing of
independent pairs, renaming, and a shared-memory thread machine refined to the abstract reduction with
exact action, work and time bounds.

Noble's interaction-net work builds on it: the selected contract is
[`docs/i64-net-contract-v3.md`](../../docs/i64-net-contract-v3.md), whose I64 interaction system is an
instance of its `INet.System`.

**How it is checked.** `proofs/foundation` compiles this file as the library `InteractionNets`
(`lakefile.toml`, `srcDir = "../inets"`); nothing here is built separately.
`verification/foundation/check.mjs` refuses the vendored file unless its SHA-256 equals
`UPSTREAM.json`'s, scans it for the same forbidden constructs as the foundation sources, and binds its
digest in every foundation receipt. Every inventoried foundation theorem that depends on it has its
axioms audited transitively (`propext`, `Classical.choice`, `Quot.sound` only).

**Updating.** Replace the file with the upstream bytes of a newer commit, update `commit`, `sha256` and
`bytes` in `UPSTREAM.json`, rebuild `proofs/foundation`, and produce a new foundation receipt. Never edit
the vendored file in place: a local change belongs upstream.

Verify the copy against upstream:

```sh
git -C <clone of OnixResearch/inets> show 98bbb01070664e4462a35efa5d62de6f27143572:InteractionNets.lean | sha256sum
```
