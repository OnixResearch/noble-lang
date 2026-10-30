## Phase 1: Design (this change; document-only)

- [x] [serial] Audit the existing WIT worlds and the actual `bounded_region.rs` host behavior at `44329dc` (registration, borrowed read checks, release, destructor, post-return/abort cleanup, report schema, fuel/memory limits), recording findings F1–F11 with line citations. r[EPROTO-SCOPE-01] r[EPROTO-OBS-01]
- [x] [serial] Select `RegionProtocol-v1`; specify the transition table, result schema, resource identity, host assumptions A1–A6 and the five-way separation of permission, proof, applicability, ownership and cleanup authority. r[EPROTO-SCOPE-01] r[EPROTO-TRACE-01]
- [x] [serial] Specify the request/response/commit trace model, the normal-return postconditions for success and ordinary error, prefix safety P1–P6, and host-owned abnormal cleanup without a totality claim. r[EPROTO-TRACE-01]
- [x] [serial] Specify the order-preserving composition rule with a worked safe composition and rejected out-of-order, omitted/double release, wrong-identity and unmodeled-response cases. r[EPROTO-COMPOSE-01]
- [x] [serial] Write the EPROTO-CASE-01…11 unexecuted designs, the proof plan T1–T6, the monitor responsibilities and the first-profile recommendation; validate the change documents (not runtime or proof evidence). r[EPROTO-OBS-01]

## Future implementation and evidence gates (not undertaken by this change)

- [ ] [serial] Introduce a new bounded-region report schema with separate release kinds, distinct trap/quota/outside-model classes and an ordered event trace; keep `noble-bounded-region/v1` receipts byte-identical. r[EPROTO-OBS-01]
- [ ] [serial] Build the Lean `RegionProtocol` model, prove T2/T3 for the selected subject, and relate the `ownership::Table` transitions through the existing M5 lane (T4); keep T5/T6 open until separately discharged. r[EPROTO-TRACE-01] r[EPROTO-COMPOSE-01]
  Progress (2026-10-30): T1–T3 are proved in `proofs/foundation/NobleFoundation/RegionProtocol.lean`.
  - T1: the transition table as `step`, with host assumptions A1–A6 explicit.
  - T2: the subject's normal-return postconditions (`subject_normal_return`,
    `subject_rejects_exactly_out_of_bounds`), and prefix safety for every program and host. P1 is
    `exactly_one_release_commit`; P2–P5 are the monitor's rules with `run_is_accepted`; P6 is
    `stop_discards_the_rest`.
  - T3: `seq` over identical states, with `counters_do_not_commute` and `no_set_union_rule`.

  T4, relating `ownership::Table` through the M5 lane in `proofs/m5`, is not done, so the task stays
  open; T5/T6 stay open as stated.
- [ ] [serial] Implement a host interruption control before claiming any cancellation behavior; until then EPROTO-CASE-07 stays unsupported. r[EPROTO-OBS-01]
- [ ] [serial] Register and execute EPROTO-CASE-01…11 with independently constructed hostile components, retaining trace prefixes, reports and component bytes; record monitor verdicts as finite execution evidence only. r[EPROTO-SCOPE-01] r[EPROTO-OBS-01]
