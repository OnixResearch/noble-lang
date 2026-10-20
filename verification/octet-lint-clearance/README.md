# Octet deny-all lint clearance (`cargo octet check`)

Decision record for the workspace-wide `octet` deny-all lint phase, closing the
remaining findings after the `noble-ngraph` sweep (`25b24c9`). It clears the
lint phase to zero; it does not decide or clear the architecture
`required-compiler-unknown` phase tracked by issue #25.

## Measured result

Command (pinned collector, every lint reported instead of aborting):

```sh
HOOK=/nix/store/*octet*/hooks/octet-deny-all.sh
WARNS=$(sed -n 's/^-D /-W /p' "$HOOK" | tr '\n' ' ')
DYLINT_RUSTFLAGS="$WARNS" cargo octet check --workspace \
  --output-format json --artifact-dir "$OUT" -- --all-targets --all-features
```

| | Findings | Files |
|---|---:|---:|
| Before (`main` @ `25b24c9`) | 1890 | 72 |
| After | **0** | 0 |

`octet summary`: `Status: lint-failure` (exit 1 from the still-blocked
architecture phase), **`Lint phase: clean (static_lint)`**, `Findings: 0`.
`Architecture findings: 1181`, receipt
`6b1a0114b006e214ce41ec04e08e839ff45a09c76213eb20e0d1a0ed6a533a09`, measured
with the selected Octet `b20681f87046fb83fb72b18908564c6674402977`. The
compiler-desugaring class is zero: `?`, `for` and range expressions are
compiler-owned expansions in Octet, and Noble no longer carries a local patch
for them. The remaining findings are 1113 `AmbiguousTraitDispatch`, 60
`UnresolvedDefinition` and 8 `DynamicDispatch`. They belong to issue #25's open
disposition, not to the lint phase.

Per-finding baseline inventory and disposition:
[`dispositions.tsv`](dispositions.tsv) (327 rows: one row per
`file × lint`, with its baseline count and disposition).

## Dispositions

### 1. Fixed in code (131 source-tree findings)

- `noble-wasm` sentinels: `Pool::intern_prefix` and `preflight::concrete::sized`
  now convert with an explicit refusal instead of `unwrap_or(u32::MAX)`;
  `source::admission::check` compares the live-ref ordinal through
  `u32::try_from(...).is_ok_and(...)`.
- `noble-wasm` growth: `value_inputs` and `borrowed_input_positions` reserve the
  admitted interface length, `OriginProgram::returned_outputs` and
  `forwarded_target_positions` start with the checked capacity, and
  `signatures::Pool::expanded_keys` fixtures are test-only.
- `noble-wasm` names: `OriginSource` → `OriginClass`, `emit_effects` →
  `emit_bounded` (both path-segment repetitions), `byte_cursor` →
  `is_byte_cursor`, `cache` → `is_cached`, `valid` → `is_valid`,
  `no_partial` → `!has_partial` in the phase-B receipt harness.
- `noble-wasm` constants: the passive-data address comes from
  `CALCULATOR_TEXT_END`; the two entry-length checks use `u32::from(u16::MAX) + 1`
  instead of the bare `65_536` literal.
- `noble-wasm::Compiler::builtin_count`: `else if` chain rewritten as ordered
  early returns (same table, no nesting).
- `noble-ngraph-experiment` harness: `J` alias replaced by
  `serde_json::Value`, node-index conversion is `filter_map(u32::try_from(..).ok())`,
  and the phase-C training-step count uses `checked_div`.
- Every stale `#[expect]` reported by `unfulfilled_lint_expectations` was
  removed or, where the reviewed rationale still applies, converted to an
  `#[allow]` with the same owner/reason text.

### 2. Reviewed `#[expect]` records in `noble-wasm` (69 findings)

Crate-level, because the lint is a whole-file/whole-crate pass:

- `excessive_file_length` — each source unit keeps one lowering stage, its byte
  layout and its refusal ordering in one reviewable file.
- `module_file_count` — the source stage is already split by lowering phase at
  the policy's nine-file capacity.
- `platform_dependent_cast` — supported targets have `usize >= 32` bits and
  every flagged cast converts a validated non-negative count or index.
- `nested_conditionals` — the ordered emitter dispatch and its early-return
  macro keep the outer constructor match adjacent to its shape arms; flattening
  would duplicate the constructor guards.

Item-level: `function_length`, `too_many_parameters`, `no_recursion`,
`mutating_input_in_pure`, `borrowed_argument_types`, `unchecked_division`,
`unchecked_narrowing`, `unsafe-API` and `retained_bytes` records each name the
function and the invariant that makes the shape deliberate. Each is a
code-level record, not an architecture-policy waiver.

### 3. Reviewed file-level harness policy (1690 findings, 50 files: 46 test files and 4 receipt binaries)

`noble-ngraph`, `noble-wasm`, `noble-cli` and `noble-contracts` integration
tests and the four `noble-ngraph-experiment` receipt binaries carry one
enumerated `#![allow(...)]` header each, listing only the lints observed in
that file, with the owner and a reason naming the harness role:

- evidence receipt binaries measure wall-clock and environment effects and
  write their JSON receipt directly, so terse names, unwraps and long campaign
  legs are the intended form;
- integration harnesses construct boundary scenarios and assert their
  outcomes, so terse fixture names, unwraps and long setup legs are the
  readable form.

No blanket waiver exists: the headers are per file, the lint list is the exact
measured set, and `tests/**` module files inherit tool registration from their
crate root. Test roots gained `#![feature(register_tool)]` /
`#![register_tool(tigerstyle)]` where they already carried no lint attributes.

## Verification

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo octet check --workspace -- --all-targets --all-features   # lint phase clean
```

## Non-claims

- The architecture phase is still blocked with 3795 `required-compiler-unknown`
  findings; `source-coverage`, `native-assurance` and the flake `octet` check
  still fail on that phase. Deciding between reviewed policy waivers and an
  Octet compiler-owned-expansion change is issue #25's open question.
- The harness headers are policy waivers for host-only test/evidence code.
  They are not evidence of production-code exceptions, and they do not apply
  to `src` library targets.
- Test-only lint waivers do not change test assertions, coverage or the
  boundary scenarios they construct.
