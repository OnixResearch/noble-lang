# Roundoff certificate corpus lane (issue #44)

This lane exercises the total certificate checker in
[`proofs/foundation/NobleFoundation/Binary32Checker.lean`](../../proofs/foundation/NobleFoundation/Binary32Checker.lean)
on a pinned corpus. The kernel check is the proof. The sampling here is secondary evidence about
bound quality.

[`lane.mjs`](lane.mjs) does four things:

1. **Corpus.** It generates 24 add/multiply graphs over binary32 inputs and constants from SplitMix64
   seed `44`. Input domains are `[1, 2]`, `[-3, 5]`, `[-1, 1]`, `[2^-130, 2^-120]` (subnormal),
   `[2^60, 2^62]` and `[0.1f, 0.3f]`. Graphs whose reach would hit the `2^127` overflow test are
   discarded (2 of 26).
2. **Untrusted analyzer.** In exact BigInt rationals, it proposes for every node the tightest
   certificate the checker's rules admit: the interval enclosure and the error bound.
3. **Kernel check.** [`corpus.lean`](corpus.lean) is generated, not hand-written. In it, the Lean kernel
   accepts every certificate (`certifiedN`) and refuses a weakened copy of each, whose output error
   bound is halved (`weakenedN`, `unknown` at the output). Both use `decide +kernel`. The run is timed
   against an import-only baseline.
4. **Independent expected values.** On 2,000 sampled inputs per graph, with the first two samples at
   the domain endpoints, it evaluates every node twice:
   - with the host's binary32 arithmetic: binary64, then `Math.fround`. Products of binary32 values
     are exact in binary64, and sums double-round innocuously because `53 ≥ 2·24 + 1`.
   - with exact dyadic arithmetic over values decoded from their **binary64** bits.

   That second path shares neither the analyzer's float32 decoder nor its rational interval
   arithmetic, and it converts to rationals only to compare with a certified bound.

Run it from the repository root:

```sh
node verification/numerical-certificate/lane.mjs --lean-bin <pinned Lean bin> \
  --foundation proofs/foundation --corpus verification/numerical-certificate/corpus.lean \
  --output verification/numerical-certificate/receipts/<date>/lane.json --graphs 24 --samples 2000 --seed 44
```

## Measured (2026-10-30)

The receipt is [`receipts/2026-10-30/lane.json`](receipts/2026-10-30/lane.json), with `holds: true`.

| | |
|---|---|
| corpus | 24 graphs, 202 nodes, 48 theorems; `corpus.lean` sha256 `9ec794e5…` |
| kernel | all 24 certificates accepted, all 24 weakened copies refused |
| checker cost | about **100 ms per kernel check**: corpus 7,197 ms against an import-only 2,378 ms (medians of three runs after a warm-up), divided over 48 checks. This includes elaborating the generated definitions |
| largest certificate rational | 775 bits (budget 832) |
| node checks sampled | 404,000; **0 violations** of enclosure, finiteness or error bound |
| certified output bound / worst observed output error | min **1.20**, quartiles 2.57 / 4.01 / 7.40, over the 23 graphs with a nonzero observed error |

- **Tightness.** The tightest graph (`14`, input in `[2^-130, 2^-120]`) observes 4.59e-44 against a
  bound of 5.52e-44. The absolute term `η` is nearly attained in the subnormal range.
- **Loose bounds** come from mixing magnitudes. Graphs combining the subnormal and `[2^60, 2^62]`
  domains propagate interval reach over the whole domain, so bounds can exceed the worst observed
  error by many orders of magnitude (ratios up to about `10^135`).
  - In two graphs the output bound is at most `2η` per node: their outputs lie far below the
    subnormal range, where only the absolute term remains.
  - Graph `11` produced an exact output in all 2,000 samples.
- **The two expected-value paths agree.** An earlier run evaluated expected values with the analyzer's
  float32 decoder and rationals. On the same corpus, it gave identical worst errors and exact-output
  counts for all 24 graphs.

## External analyzer comparison (Gappa)

[`gappa.mjs`](gappa.mjs) compares the certified output bounds with Gappa, an external analyzer that is
untrusted here, on the same corpus. [`corpus.mjs`](corpus.mjs) holds the corpus generator and analyzer
that both lanes share. The comparison lane regenerates the 24 graphs and requires them to rebuild
`corpus.lean` byte for byte.

For each graph it writes a Gappa script with binary32 round-to-nearest-even arithmetic
(`float<ieee_32, ne>`). Each input is `rnd(X)` with `X` in the domain, so it ranges over exactly the
binary32 values the certificate admits. The machine chain rounds every operation; the real chain
computes exactly from the same inputs. Gappa then encloses the output's machine-minus-real error,
with no hints. Gappa comes from the nixpkgs the repository flake locks (`nixpkgs` follows
`octet/nixpkgs`):

```sh
GAPPA="$(nix build --no-link --print-out-paths --impure --expr \
  '(builtins.getFlake "git+file://'"$PWD"'?rev='"$(git rev-parse HEAD)"'").inputs.nixpkgs.legacyPackages.x86_64-linux.gappa')/bin/gappa"
node verification/numerical-certificate/gappa.mjs --gappa "$GAPPA" --output NEW_EXTERNAL_FILE
```

The receipt is [`receipts/2026-11-08/gappa.json`](receipts/2026-11-08/gappa.json), from the clean commit
`91303c14`, with `holds: true` (Gappa 1.6.1).

| | |
|---|---|
| graphs Gappa bounds | all 24; every Gappa bound covers the worst output error the corpus lane observed |
| certified bound over Gappa's bound | min **1.16**, quartiles 1.36 / **1.66** / 2.91 over 23 graphs, max about `5.6·10^135` |
| exact output | graph `11`: Gappa proves the output exact (`[0, 0]`), where the certificate allows `7.2·10^16` |
| cost | Gappa median **16 ms** per graph, including process start-up, against about 100 ms per kernel check |

- **Most graphs.** In 17 of the 23, the certificates are within 1.16 to 2.91 times Gappa's enclosures.
  Graph `0` is at 7.4.
- **Mixed magnitudes.** The five large ratios (graphs `1`, `6`, `17`, `22`, `23`) all combine the
  subnormal domain with ordinary or large magnitudes. There the checker propagates interval reach over
  the whole domain. In four of them (`1`, `6`, `22`, `23`), Gappa's bound matches the worst observed
  error to six significant digits. Other graphs that mix the subnormal domain with ordinary magnitudes
  stay within 2.1.
- **Different guarantees.** Gappa's bounds are untrusted and not kernel-checked here. The certificates
  are the proof, so tighter Gappa bounds suggest where the checker's rules lose precision, not that
  a certificate is wrong.

## Non-claims

- **Binding is identity equality, not derivation.** [`Binary32Binding.lean`](../../proofs/foundation/NobleFoundation/Binary32Binding.lean)
  (issue #44, NGRAPH-ERR-04) gates the checker on six identities — snapshot, graph revision,
  domain-set digest, executed-order digest, FMA-contraction flag and analyzer model revision — and
  refuses with its own outcome, `notApplicable`, when the declared and executing contexts differ.
  Theorems: `checkBound_only_certifies_matching` (acceptance implies equal contexts and an accepted
  unbounded check), `checkBound_sound` (the bounded statement is the unbounded one under that
  premise), and six kernel-decided controls (`changed_snapshot_is_not_applicable`,
  `changed_revision_is_not_applicable`, `changed_domain_is_not_applicable`,
  `reordered_execution_is_not_applicable`, `fma_contraction_is_not_applicable`,
  `stale_model_is_not_applicable`) plus `matching_context_certifies` (the same certificate is still
  accepted under its own context, so the gate refuses nothing vacuously) and
  `mismatch_is_not_an_unknown_outcome` (a mismatch is never reported as a failed check). How the
  identities are computed, and whether they name the graph they claim to, is **outside** this model:
  a digest computed wrongly still compares equal to itself.
- Sampling is secondary evidence: soundness is the kernel-checked certificate, not the absence of
  sampled violations.
- The sampled machine is the host's binary32 arithmetic, not the Lean model; their agreement is not
  claimed here.
- The corpus is Lean model graphs. No encoded `noble-ngraph` graph, Rust or Wasm evaluator is
  exercised.
- The external-analyzer comparison uses Gappa only; FPTaylor and Daisy are not packaged in the pinned
  nixpkgs. Gappa's enclosures are not kernel-checked here.
