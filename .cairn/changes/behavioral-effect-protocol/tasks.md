## Phase 1: Design (this change; document-only)

- [x] [serial] Audit the existing WIT worlds and the actual `bounded_region.rs` host behavior at `44329dc` (registration, borrowed read checks, release, destructor, post-return/abort cleanup, report schema, fuel/memory limits), recording findings F1–F11 with line citations. r[EPROTO-SCOPE-01] r[EPROTO-OBS-01]
- [x] [serial] Select `RegionProtocol-v1`; specify the transition table, result schema, resource identity, host assumptions A1–A6 and the five-way separation of permission, proof, applicability, ownership and cleanup authority. r[EPROTO-SCOPE-01] r[EPROTO-TRACE-01]
- [x] [serial] Specify the request/response/commit trace model, the normal-return postconditions for success and ordinary error, prefix safety P1–P6, and host-owned abnormal cleanup without a totality claim. r[EPROTO-TRACE-01]
- [x] [serial] Specify the order-preserving composition rule with a worked safe composition and rejected out-of-order, omitted/double release, wrong-identity and unmodeled-response cases. r[EPROTO-COMPOSE-01]
- [x] [serial] Write the EPROTO-CASE-01…11 unexecuted designs, the proof plan T1–T6, the monitor responsibilities and the first-profile recommendation; validate the change documents (not runtime or proof evidence). r[EPROTO-OBS-01]

## Future implementation and evidence gates (not undertaken by this change)

- [ ] [serial] Introduce a new bounded-region report schema with separate release kinds, distinct trap/quota/outside-model classes and an ordered event trace; keep `noble-bounded-region/v1` receipts byte-identical. r[EPROTO-OBS-01]
- [ ] [serial] Build the Lean `RegionProtocol` model, prove T2/T3 for the selected subject, and relate the `ownership::Table` transitions through the existing M5 lane (T4); keep T5/T6 open until separately discharged. r[EPROTO-TRACE-01] r[EPROTO-COMPOSE-01]
- [ ] [serial] Implement a host interruption control before claiming any cancellation behavior; until then EPROTO-CASE-07 stays unsupported. r[EPROTO-OBS-01]
- [ ] [serial] Register and execute EPROTO-CASE-01…11 with independently constructed hostile components, retaining trace prefixes, reports and component bytes; record monitor verdicts as finite execution evidence only. r[EPROTO-SCOPE-01] r[EPROTO-OBS-01]
