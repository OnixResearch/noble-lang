## Phase 1: Native contract and production boundary

- [x] [serial] Gate this pinned native proposal/design/tasks and retain unsynced safety, resource-adapter and evidence deltas until separate source-bound acceptance; structural gates alone do not execute S-CASE-06. r[S-EFFECT-03] r[RA-STATE-01] r[EV-CASE-03]
- [x] [serial] Compile the actual `"main.rs" fs.read` Noble source to the selected versioned WIT component and link its `read-file` export to production Wasmtime `read-fs`; independently prepare/compile the complete invoker-selected `WIT WORLD EXPORT=SOURCE ...` recipe and require candidate component-byte equality **before** file open or any grant. Only then preopen the host-vetted regular file at guest key `main.rs`, keep Directory owner/context host-only and live, and select READ or none separately; effects/preopen alone grant no READ. r[S-EFFECT-03] r[S-RES-03]
- [x] [serial] On import, account one actual callback request; use `Table::validate` on host-retained owner/context/kind/generation/liveness and independent READ, check exact mapped key, then perform at most one accounted protected operation with at most 4096 output bytes (up to one extra bounded sentinel byte to detect oversize). Ensure normal/error/trap cleanup retires the host owner once; reject stale, foreign, wrong-kind, wrong-generation, retired and path/grant variants before protected content access. r[RA-STATE-01] r[RA-CLEAN-01]

## Phase 2: Immutable prepromotion acceptance

- [x] [serial] Run canonical no-READ through the compiled guest and production host with a matching complete recipe: `authorization/denied`, `guest_requests:1`, `request_trace:["fs.read"]`, `protected_operations:0` and zero file-content reads; execute a fresh READ-granted run of those same compiled guest bytes returning exact bounded fixture bytes and releasing its owner, plus hostile path/owner/rights/size controls. Independently require a mismatched candidate/recipe to fail with status 2 **before** File open or Directory/READ creation and zero guest requests. r[S-EFFECT-03] r[EV-CASE-03]
- [x] [serial] Freeze a NEW source-bound acceptance/assurance receipt with exact prepromotion source, WIT, component/CLI, fixture, commands, raw observations and zero failures/integrity failures before syncing or changing the canonical case; keep historical runners/receipts byte-identical and owner-law/Octet/Aeneas proofs OPEN. r[EV-CASE-03]

## Phase 3: Conditional postpromotion documentation and closeout

- [x] [serial] **Only after immutable prepromotion PASS**, preview and execute Cairn sync; promote only the justified S-CASE-06 implementation/execution/trust/evidence fields, regenerate generated views/ledger and applicable status/roadmap, validate the native specifications without treating document validation as runtime/proof evidence. Coordinate owner of any shared canonical file. r[S-EFFECT-03] r[EV-CASE-03]
- [ ] [serial] Run separate staged postpromotion documents/Cairn checks against the accepted-source projection, then preview/execute archive only if its prerequisites pass; retain the original prepromotion receipt and every historical runner/receipt byte-for-byte. r[EV-CASE-03]

After the archived source and peer promotions settle, perform the separate chronological final-source replay; this is not the immutable prepromotion acceptance or the staged postpromotion document gate. Keep the following last checkbox unchecked until that release completes.

- [ ] Final source replay closeout [serial] r[EV-CASE-03]
