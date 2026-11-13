# Ownership accounting corpus (issue #59)

An independent check of the ordered stack/resource rules in
`crates/noble-kernel/src/acceptance`: a JavaScript mirror of the Lean model proposes
the expectations, the Lean kernel decides every row, and the production machine is
driven one word prefix at a time. The generator does not execute Lean to print rows;
each proposed `run`/`runTrace` result is instead checked by a generated `decide`
theorem. The design and its open edge are recorded in
`.cairn/changes/ownership-accounting-oracle/` and the model's section of
[`proofs/foundation/README.md`](../../proofs/foundation/README.md).

The lanes here are **finite differential evidence**. They are not a refinement
proof: nothing relates `crates/noble-kernel/src/acceptance` to the model by
extraction or by a proof, and the whole-corpus agreement is measured on one host
and target.

## The artefacts

| File | Role |
|---|---|
| `cases.mjs` | A handwritten mirror of the model used to propose rows. Writes `cases.txt` and `proofs/foundation/NobleFoundation/OwnershipCorpus.lean`; the latter kernel-checks each expected `run`/`runTrace` result. `--check` fails when committed files differ from the emission. |
| `cases.txt` | One row per case: `name ops= in= out= trace= decision= reason=`. The Rust lane parses it. |
| `../../proofs/foundation/NobleFoundation/OwnershipCorpus.lean` | The same rows as `decide`d equations of the model's `run` and per-word `runTrace`. A row whose expectation does not follow from the model fails `lake build`. |
| `../../crates/noble-kernel/tests/acceptance/ownership.rs` | The lane: drives production per word prefix, compares every corpus row with its kernel-checked twin, checks a test-local shadow checker and its declared mutations, and minimises a failing case into a reproducer. |
| `../../verification/foundation/receipts/2e932250/foundation-check.json` | Clean-source receipt for the imported model and all 116 corpus theorems (source commit `2e932250`). |
| `differential-<commit8>.json` | The retained comparison receipt: clean commit, toolchain, host, digests, coverage, model-only counts by reason, mutation line and the open edge. |
| `production-mutants-<commit8>.json` | The retained production-mutation receipt: which real rule changes the lane catches and which mutants survive. |
| `minimised.txt` | Written only when a comparison fails: the shortest failing word prefix and input with its removable trailing operands trimmed. |

## Coverage and classification

Every case is classified, and the classification is part of the corpus:

- `accepted` — driven through `noble_kernel::acceptance::check` one prefix at a
  time; after `k` words the machine's derived stack must equal the model trace's
  `k`-th entry.
- `rejected` — the machine must refuse at the word the model refuses, and the
  refusal class the machine itself reports is pinned in the lane, so a lost word
  rule turns into a different class instead of being hidden by an unchanged
  rejected total.
- `model-only` — no production analogue, with a stated reason:
  `branch-accounting` (the two-arm rule is not exposed as an operand-stack order),
  `error-return` (the ordinary error return), `quotation-program-value` (the
  production quotation constructor produces a program value instead of retaining
  the operand), `wrapped-payload` (no kernel binding type names a wrapped payload)
  and `operand-arity` (the model refuses for a missing operand; the machine refuses
  such a shape as a stack join, a different rule).

`swap` is driven through production: the machine instantiates one scheme per word,
so a swap prefix derives its reordered stack and the corpus compares it. What is
*not* observable through production is owner identity — every resource payload has
the same kernel type — so the equal-count/different-identity control stays
model-only.

## Commands

```sh
NODE=node
RUSTC=rustc
CARGO=cargo
OUT='<new absolute directory outside the repository>'
SCRATCH='<private temporary directory>'
TARGET='<new absolute directory outside the repository>'
umask 077
mkdir -p "$OUT" "$SCRATCH" "$TARGET"

# Regenerate the corpus and its kernel-check module (fails --check on drift).
"$NODE" verification/ownership/cases.mjs
"$NODE" verification/ownership/cases.mjs --check

# Run with the repository's pinned Rust toolchain and offline dependencies.
CARGO_TARGET_DIR="$TARGET/acceptance" "$CARGO" \
  test -p noble-kernel --test acceptance ownership --offline --locked -- --nocapture

# After committing the source changes, retain a clean-source foundation receipt.
verification/ci/lean-proofs.sh "$SCRATCH/lean-proofs"
SOURCE_COMMIT=$(git rev-parse --short=8 HEAD)
mkdir -p "verification/foundation/receipts/$SOURCE_COMMIT"
cp "$SCRATCH/lean-proofs/foundation/foundation-check.json" \
  "verification/foundation/receipts/$SOURCE_COMMIT/foundation-check.json"
# Commit this evidence-only receipt, then continue from the resulting clean HEAD.

# Run both at the same clean commit after retaining the matching foundation receipt.
SOURCE_COMMIT=$(git rev-parse --short=8 HEAD)
CARGO_TARGET_DIR="$TARGET/differential" "$NODE" verification/ownership/differential.mjs \
    --root "$PWD" --output "$OUT/differential.json" \
    --cargo "$CARGO" --rustc "$RUSTC"

# Retain a production-mutation receipt at the current clean commit.
CARGO_TARGET_DIR="$TARGET/production-mutants" "$NODE" verification/ownership/production-mutants.mjs \
  --root "$PWD" --output "$OUT/production-mutants.json" \
  --scratch "$SCRATCH/mutant-copies" \
  --target "$TARGET/production-mutants" \
  --cargo "$CARGO" --rustc "$RUSTC"

# Retain receipts under the commit they name; commit them as an evidence-only change.
cp "$OUT/differential.json" "verification/ownership/differential-$SOURCE_COMMIT.json"
cp "$OUT/production-mutants.json" \
  "verification/ownership/production-mutants-$SOURCE_COMMIT.json"
```

The Lean gate and both comparison producers require a clean source commit. Commit
the foundation receipt before running the differential producer: its model binding
requires a clean-source receipt that matches `Ownership.lean`, `OwnershipTrace.lean`
and `OwnershipCorpus.lean`. The production-mutation runner does not need that receipt,
but runs at the same clean HEAD so the two comparison receipts share one source identity.
Both comparison producers refuse to record with any
untracked or modified files. The differential also refuses when `cases.mjs --check`
reports drift, and the Rust lane compares every corpus row with its kernel-checked
twin, so a receipt cannot describe a corpus that the kernel did not decide.
Producer outputs and mutation copies stay outside the repository until copied into
their retained paths after a successful clean-source run. `production-mutants.mjs`
allocates a separate Cargo target directory for the baseline and every mutant under
the specified target base. These commands assume the repository's pinned Rust
toolchain and offline dependency configuration are active.

The pre-existing undated `differential.json` is retained byte-for-byte as historical
evidence for source `419c0144`; its model binding is stale. Use only the matching
dated differential receipt as current evidence.

## The mutation lanes

Two kinds of mutant, kept separate because they answer different questions:

- **Corpus mutations** (`ownership_corpus_detects_each_declared_checker_mutation`,
  a test-local copy of the model with one declared variation): shows the corpus is
  sensitive to a changed rule, and that the unmutated copy agrees with every row.
- **Production mutants** (`production-mutants.mjs`): one exact byte replacement in
  a production file, applied to a scratch copy of the clean commit, with the lane
  required to fail. Each caught mutant must also write a minimized fixture in its
  scratch copy, and the receipt retains that row; the runner refuses unless its
  operation/operand pair is already a non-model-only row in the committed corpus.
  This is the control the corpus mutations cannot give.

A surviving production mutant is recorded, not suppressed. `quote-unconstrained`
survives by construction: every `quote` case is model-only, so the corpus has no
executable control for the quotation eligibility rule. The receipt states that
instead of counting it as covered.

The minimiser's synthetic probe pins prefix and trailing-operand reduction.
`ownership_minimiser_preserves_model_acceptance` prevents a failing accepted
control from shrinking into an unrelated model-only operand-arity refusal.

## Correspondence route and non-claims

The route is handwritten: `cases.mjs` mirrors selected model definitions and emits
rows checked by the Lean kernel; the model's own bytes are bound by a retained
clean-source foundation receipt whose
`sources_sha256` matches (`verification/foundation/model-receipts.mjs`). The one
trusted translation is reading a corpus row into the model's terms.
The differential receipt lists the inventoried ownership and corpus theorem
counts separately and refuses to record unless the corpus theorem count equals
the number of rows.

Open, and deliberately not closed here:

- no extraction or refinement proof relates the acceptance machine to the model;
- owner identity is not observable through the machine's type-only derivation, so
  the identity controls stay model-only;
- the production quotation constructor and the two-arm branch have no operand-stack
  analogue, so their rules stay model-only;
- no claim about release of a native object, and no whole-compiler safety claim.
