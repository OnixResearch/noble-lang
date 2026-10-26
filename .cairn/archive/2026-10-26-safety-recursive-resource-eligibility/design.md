## Context

S-CASE-04 names four internal Noble types, four generic operations and `selected_sum_branch:I64`. Its expected stage/outcome are canonical harness categories, not component CLI protocol fields. The WIT adapter admits `own<counter>` only as a bare import/export value; Pair/Sum/List containing its resource can be constructed by Noble source but are not WIT tuple/result/list<own> inputs. The existing `Ty::is_data` walk inspects both sides of Pair/Sum and List elements; the kernel applies it to `dup`, generic `drop`, and capture by `quote`. `reflect` itself has no Data check. r[S-RES-01] r[S-RES-04]

## Decisions

### Decision: Source-derived whole-type eligibility, not a substitute fixture

**Choice:** Parse a real `noble-test:recursive-resource/demo@1.0.0` world with `counters.open: [] -> own<counter>` and `counters.close: own<counter> -> s64`. Derive R from its public resource kind, not kernel fixture kind zero. Public `World::session()?.prepare(prefix,&[],limits)` must produce exact structural Ty for `counters.open`, `"tag" counters.open pair`, `false [ counters.open inl ] [ 7 inr ] if`, and `counters.open nil cons`. For each, `World::prepare_export("check",prefix+word,limits)` and the source session must return the typed `eligibility:Data` join naming the complete Ty and the exact `dup`, `drop`, or `quote` word. `generic-serialization` means the spec's `quote reflect`, rejected before `reflect`. Separately execute all sixteen production CLI `component compile` refusals with whole-type diagnostics and absent component output. r[S-RES-01] r[S-RES-04]

**Rationale:** A hand-built Ty predicate, kernel fixture kind, WIT aggregate-parse failure, frontend mismatch, or compiler-lowering failure cannot satisfy the exact scenario. The semantic source/checker path and CLI static publication boundary are independently observed; no guest or protected operation is run in this case.

### Decision: Honest positive controls and canonical classification

**Choice:** Compile typed resource open/close, source Pair/unpair/typed close, and copyable I64 positives with a real component. Use `false [ counters.open inl ] [ 7 inr ] if [ counters.close ] [ dup drop ] case` as a positive source-export preparation for the right/I64 payload, stopping before the known unsupported or exhausted component-lowering path. The nonempty List<Resource<R>> is positively typed by source preparation, not claimed as an external WIT list or compiled closed export. Distinct unsupported-WIT, unbound-word, and ill-typed source controls prove those failures cannot be mistaken for eligibility. The CLI literally reports `component-check/error`; only the gate's typed diagnostic, whole-value source type and non-emission justify the explicitly derived canonical `check/eligibility-reject`. r[S-RES-04]

**Rationale:** A current-variant-only eligibility predicate would wrongly accept the whole sum on the I64 branch. The branch-local `I64` may be manipulated only after `case` exposes it at its own type; that static fact is not a guest runtime observation. The selected component lowerer need not support sum/list/capture emission for their check-time rejections to be real.

### Decision: Freeze before promotion; keep earlier evidence immutable

**Choice:** A pinned selected-tool gate creates a fresh external directory, hashes exact source and historical receipts, runs the typed peer and CLI with raw command logs, rejects any incomplete matrix/control or source drift, and writes an immutable acceptance receipt. Only after terminal pass, sync the Cairn delta and promote S-CASE-04 status/evidence. A separate postpromotion check validates the exact prepromotion source projection, canonical field-only promotion and generated records; afterward coordinate a chronological latest-source replay. Do not close the unrelated active S-CASE-06 Cairn change or rewrite its accepted receipts. r[S-RES-01] r[S-RES-04]

**Rationale:** Prepromotion acceptance, generated views and final-source replay bind different source trees. Their distinct receipts prevent historical acceptance from being silently relabeled as latest-source proof.

## Risks / Trade-offs

- No executed Sum I64 branch or List resource consumer is claimed; no WIT aggregate ABI or generic serializer has been added. If selected lowering refuses a positive shape, retain that refusal rather than claiming a compiled control.
- The static checker and compiler toolchain are assumed; finite test observations do not establish universal resource safety, owner law, Octet acceptance, or Charon/Aeneas/Lean refinement.
