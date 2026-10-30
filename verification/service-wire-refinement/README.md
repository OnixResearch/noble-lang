# ServiceRef codec: refinement of the Lean model (issue #48)

This lane proves statements about two generated Lean functions:
`noble_kernel.dataspace.wire.decode_service` and `noble_kernel.dataspace.wire.encode_service`. Both
are checked-in Charon/Aeneas translations, in `proofs/m3/NobleKernel/Funs.lean`, of the codec in
[`crates/noble-kernel/src/dataspace/wire.rs`](../../crates/noble-kernel/src/dataspace/wire.rs). The
lane relates them to the model in
[`ServiceWire.lean`](../../proofs/foundation/NobleFoundation/ServiceWire.lean). The finite Rust
comparison in [`verification/service-ref`](../service-ref) stays separate evidence: 357 cases and
nine seeded decoder faults.

## Command

```sh
env -i HOME="$HOME" USER="$USER" LOGNAME="$USER" PATH=/run/current-system/sw/bin:/usr/bin:/bin \
  "$(selected node)" verification/service-wire-refinement/check.mjs --output /tmp/service-wire-check
```

The harness refuses three things:
- a caller environment carrying `LEAN*`, `LAKE*`, `NODE*`, `BASH*`, `GIT_*`, `NIX_*` or `ELAN*` variables;
- an output directory inside the source tree;
- a dirty worktree.

`--allow-dirty` records a dirty run instead of refusing it, but does not hide it.

## What one run checks

1. **The generated input.** The five `proofs/m3/NobleKernel` sources must match
   [the profile](model-inputs.json) and two independent records of one passed extraction:
   - `verification/declared-modules-v1/extraction.json`, for the generated outputs and the
     handwritten external models it read;
   - `verification/m4/extraction-lock.json`.

   The foundation model `ServiceWire.lean` must match its pinned hash. The
   [project](../../proofs/service-wire)'s `lean-toolchain` must equal M7's byte for byte, and its
   `lake-manifest.json` must equal M7's apart from the package name. Each run copies exactly these
   files into a fresh tree in the checkout's relative layout and compiles them; no retained `.olean`
   of the project is imported. The pinned Lean 4.31 executable and the Aeneas/Mathlib checkouts must
   match the manifest.
2. **Statements, subjects and axioms.** Every theorem in
   [`ServiceWireRefinement.lean`](../../proofs/service-wire/ServiceWireRefinement.lean) must appear
   in [the inventory](../../proofs/service-wire/inventory.json), and the reverse. The inventory pins
   three things, and each must equal what the compiled module produces:
   - each theorem's compiled statement;
   - its *subject functions*: the generated functions its statement names;
   - the printed form of every specification definition: `bytesOf`, `scanFails`, `recordMiddle`,
     `observed`, `packetBytes`, `prefixSlice` and `suffixArray`. `scanFails` is structurally
     recursive, so its printed form names its compiled body rather than spelling it out. The
     module's bytes are recorded in the receipt.

   Each transitive axiom set must lie within `propext`, `Classical.choice` and `Quot.sound`.
   Renewal is explicit: a failing run writes `observed-pins.json` for review, and nothing updates
   the inventory automatically.
3. **Source binding.** A Lean pass collects every `noble_kernel` constant reachable from the
   inventoried theorems, through types, proof terms and constructors. Each one maps to the generated
   declaration that owns it, and that declaration's `Source:` Rust file must hold exactly the bytes
   the extraction recorded (`exact-file`). There is no archived input for this extraction, so a
   reachable Rust file that has changed since then fails the run and needs a new extraction.
   Elaborator-shared matchers and side proofs are traversed but bind no Rust item. Reachable
   handwritten external models are listed as a trust boundary.
4. **Mutation controls.** Each structure-preserving edit changes one behaviour of the generated
   codec. Each mutated kernel must still compile, and elaborating the unchanged theorems against it
   must fail. The targeted theorems must then depend on `sorryAx`, while the other theorems keep
   strict axioms.

## What is proved

| Group | Theorems | Claim |
|---|---|---|
| Decoder refinement | `decode_service_refines` | For every byte slice and every `u32` configured depth, the generated decoder returns `ok r`. Read through `observed`, `r` is the model decision on the slice's bytes: an accepted value is `accepted` with its name bytes and readiness tag; `ByteLimit`, `DepthLimit` and `Schema` are the model's three refusal classes; no other error occurs. |
| Encoder refinement | `encode_service_refines` | For every string and readiness tag, the generated encoder returns. For a name the model's `validName` accepts, it returns a packet of `15 + n` bytes whose first `len` bytes are exactly the model encoding; for any other name it returns `InvalidName`. It never returns `ByteLimit`. |
| Decoder consequences | `decode_service_total`, `decode_service_accepts_iff`, `decode_service_refuses_iff`, `decode_service_of_encode`, `decode_service_accepted_is_canonical` | The generated decoder never panics, fails a bound check or diverges. It accepts given name bytes and a readiness tag exactly when the model does, and returns each refusal class exactly when the model decides it. Every model encoding of a valid name decodes to that name under every configured depth `1..=4`. Everything it accepts has a valid name, and its input is exactly that value's model encoding. |
| Rust round trips | `decode_service_encode_service`, `encode_service_decode_service` | Through `Packet.as_bytes`, a valid name's packet decodes back to that name and tag at every configured depth `1..=4`. Re-encoding any value the decoder accepts gives back exactly the input bytes. |
| Generated steps | `check_depth_eq`, `has_service_prefix_eq`, `valid_name_eq`, `as_bytes_eq` | For configured depths up to four, `check_depth` returns `DepthLimit` exactly when the model's balanced scan exceeds the depth. On every input of at least ten bytes, `has_service_prefix` returns whether the first ten bytes are the model prefix. On every string, `valid_name` returns the model's `validName`. A packet within the wire budget exposes exactly its first `len` bytes through `as_bytes`. |
| UTF-8 | `from_utf8_ascii`, `from_utf8_cases`, `ascii_bytes_are_valid_utf8` and two encoding lemmas | The external `from_utf8` model returns its input or reports invalid UTF-8, and it accepts an all-ASCII slice unchanged. With `validName_ascii` this means the decoder's UTF-8 step never refuses a valid name. |
| Model shape | `decode_after_checks`, `schema_of_refuted`, `accepted_of_shape`, `fill_record`, `validName_ascii`, `validName_length` and six record-layout lemmas | Past the byte budget, the configured depth and the depth scan, the model decides `schema` unless the bytes are the encoding of a valid name. A record with the prefix, a readiness suffix and a valid middle is accepted with that middle. Writing a ten-byte prefix, a name and a five-byte suffix in turn leaves exactly that record in front. |

The remaining 30 theorems are lemmas these rest on:
- the three model depth-scan lemmas behind the generated loop's early exit (`scanFails_eq`,
  `depthExceeds_eq_scanFails`, `depthScanAux_ge`);
- nine byte-view and slice-write lemmas;
- eighteen generated-step lemmas: range and element indexing, slice lengths, the generated prefix
  and suffix constants, the fifteen-byte minimum, saturating subtraction, slice copies and a guarded
  boolean step.

## Retained runs

### `48965967`: [receipt-48965967.json](receipt-48965967.json)

This is a clean run at source `48965967`, and all 60 theorems compile:

| Axioms | Theorems |
|---|---|
| `propext, Classical.choice, Quot.sound` | 36 |
| `propext, Quot.sound` | 13 |
| `propext` | 10 |
| none | 1 |

Every statement, every subject set and the seven specification definitions match their inventory
pins. The subjects are `decode_service`, `encode_service`, `Packet.as_bytes`, `check_depth`,
`has_service_prefix`, `valid_name`, `decode_service.MIN_SERVICE_BYTES` and the external `from_utf8`.

The reachability pass found 86 project constants, including 19 generated declarations and 44 shared
auxiliaries. Two files are bound at `exact-file`:
- `dataspace/wire.rs`: fifteen declarations, the decoder with its depth, prefix and name checks,
  the encoder, `ServiceRef`, `Packet` and `Packet::as_bytes`;
- `dataspace/mod.rs`: four declarations, `Error` and the three wire limits.

The reachable external models in `FunsExternal.lean` and `TypesExternal.lean` are `from_utf8` with
`Utf8Error`, `u8::is_ascii_alphanumeric`, and `str::len`, `str::is_empty` and `str::as_bytes`. The
build reported 12 lint warnings, all in the checked-in generated and external sources. None came
from the module or the model.

Each mutated kernel compiled, and elaborating the unchanged theorems against it failed:

| Control | Behaviour changed | Failing declaration | Theorems on `sorryAx` | Theorems kept strict |
|---|---|---|---|---|
| `byte-budget-boundary` | a 64-byte input is over budget | `decode_service_refines` | 8 | 52 |
| `zero-depth-class` | depth 0 is refused as `Schema` | `decode_service_refines` | 8 | 52 |
| `depth-scan-closer` | `>` no longer lowers the scanned depth | `check_depth_eq` | 9 | 51 |
| `prefix-byte` | the decoder's prefix expects `<Service "` | `has_service_prefix_eq` | 9 | 51 |
| `ready-tag-swap` | `#t` decodes as not ready | `decode_service_refines` | 8 | 52 |
| `name-alphabet-underscore` | a backquote replaces `_` in the shared name check | `valid_name_eq` | 10 | 50 |
| `suffix-quote` | the decoded name must close with `'` | `decode_service_refines` | 8 | 52 |
| `encoder-ready-tag` | a ready value is encoded with `#f` | `encode_service_refines` | 3 | 57 |
| `encoder-invalid-name-class` | an invalid name is refused as `ByteLimit` | `encode_service_refines` | 3 | 57 |
| `encoder-skips-prefix` | the name is written into the buffer without the prefix | `encode_service_refines` | 3 | 57 |
| `encoder-budget` | records over 32 bytes are refused | `encode_service_refines` | 3 | 57 |

**Targeted versus dependency losses.** Each control's targeted theorems are statements the mutated
code makes false. For example, under `depth-scan-closer` the input `<><>` at depth 1 returns
`DepthLimit` where the model scan passes, and under `encoder-budget` a valid 20-byte name is refused.
Other theorems land on `sorryAx` only through their proofs, and their statements stay true:
- `decode_service_total`, under every decoder control: every mutated decoder is still total.
- `decode_service_accepts_iff`, `decode_service_of_encode`, `decode_service_accepted_is_canonical`
  and both round trips, under `byte-budget-boundary`, `zero-depth-class` and `depth-scan-closer`.
  Those controls leave acceptance unchanged: canonical records have at most 47 bytes and one
  bracket pair, and the round trips use depths `1..=4`.
- `decode_service_refuses_iff`, under `ready-tag-swap`: the refusals are unchanged.
- `encode_service_decode_service`, under `name-alphabet-underscore`: the encoder shares the mutated
  name check.
- The round trips, under `encoder-invalid-name-class`: they only involve valid names.

### `a3223b03`: [receipt.json](receipt.json)

The first run covered the decoder only: 43 theorems and seven mutation controls. Its receipt is
unchanged.

A pin negative control at that source made three changes:
- `decode_service_total` restated as `True` in the module;
- the subject set of `decode_service_accepts_iff` emptied in the inventory;
- the printed pin of `observed` weakened to `True`.

The checker refused it and named exactly `statement` and `subjects` of `decode_service_total`,
`subjects` of `decode_service_accepts_iff`, and `specification` of `observed`. An earlier draft of
that control left the restated theorem's binders unused, and the build-diagnostic guard refused it
before the pins were read.

## Boundaries

- The Charon/Aeneas translation of the bound sources is trusted; this lane does not re-run it.
- The Aeneas library models are trusted: slices, arrays, range indexing and range writes, slice
  copies, scalars, checked arithmetic and loops.
- The listed external models are trusted. In particular, `from_utf8` is modelled as returning its
  input on valid UTF-8 and `Utf8Error::Invalid` otherwise.
- The decoded name is related by its byte values; that it borrows the input slice is not stated.
- A packet is related by its first `len` bytes; the rest of its 64-byte buffer is not stated.
- Callers of the codec are out of scope.
- No Rust compiler, Charon or Aeneas correctness claim.
- The phase 3 task of `service-ref-codec-reference` ("Relate `wire.rs` to the model through an
  extraction or refinement route") stays unticked: promotion is not selected.
