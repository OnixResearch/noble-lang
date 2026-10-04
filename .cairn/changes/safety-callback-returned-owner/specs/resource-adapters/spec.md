# Host callback returned-owner custody

## MODIFIED Requirements

### Requirement: RA-OWN-01
r[RA-OWN-01]

Before committing an owned WIT transfer, the adapter MUST finish argument validation and establish a cleanup owner for each obligation. A failed preflight leaves ownership with the caller.

After commit, the sender cannot use the transferred handle. A domain error does not reverse the transfer unless the declared result explicitly returns ownership. The receiver or host retains cleanup responsibility after a trap.

This contract describes local handle ownership. It does not prove physical exclusivity of an external object or exactly-once remote cleanup.

For selected S-CASE-08 `noble-test:callback-owner/bounded@1.0.0`, the `tokens.issue() -> own<token>` callback result is an incoming **host-produced** owner claim, not an already trusted guest value. Only after complete recipe byte-match, `Host::new` MUST pre-register one pending authentic owner and at most one mode-selected fixture: none for authentic, a retired prior generation for stale, or one live wrong-kind or foreign-context owner. The `issue` callback selects a retained claim without creating a new owner. The production host MUST retain custody of the pending actual `Table::Owner` and validate the selected claim's table namespace, slot, generation, expected token kind, current invocation context, liveness and applicable rights before constructing Wasmtime `ResourceAny` and publishing exactly one `Val::Resource` as a guest WIT-owned value. Only a matching live claim permits that publication. The host retains its `Table::Owner` custody and deterministic cleanup accounting; this selected callback path does **not** invoke `Table::transfer` or claim to move the kernel owner into Wasmtime. A stale-generation, wrong-kind or foreign-context result MUST publish no guest owner while host custody of the pending owner remains for exactly-once retirement; matching numeric slot/generation in a distinct table/context is insufficient. The authentic guest `tokens.consume(own<token>)` request MUST consume the published WIT owner representation only once, account its one protected operation and let the host release the retained table owner once; an invalid result MUST perform no protected operation or create a native pin.

#### Scenario: Callback owner obligation before and after validated publication

- GIVEN a matched component with one pending authentic host-table owner and at most one mode-selective real fixture pre-registered before guest entry
- WHEN the host preflights the returned claim before Wasmtime owned-resource creation and the authentic guest subsequently calls `tokens.consume`
- THEN only the authentic result publishes one trusted WIT-owned guest value and reaches the second guest request, while the host retains its table owner until consume; invalid results leave the host responsible for retiring that pending owner once and settling the selected fixture separately without publishing a trusted value

### Requirement: RA-CLEAN-03
r[RA-CLEAN-03]

Retirement and callback handling MUST be idempotent with respect to local release. The invariant includes hidden adapter state and native pins, not only the guest stack. For selected S-CASE-08, an invalid callback-returned claim MUST NOT discard the separately retained live pending authentic owner, and repeated cleanup, callback failure or abnormal exit MUST NOT create a second release, resurrect the rejected claim or leave a hidden pin. The `noble-callback-owner/v1` report MUST account exactly one released pending owner, zero live owners/native pins, and `fixture_releases:0` in authentic mode or `fixture_releases:1` in each hostile mode. The hostile fixture release is the stale setup retirement or one wrong-kind/foreign-context live-owner cleanup; neither is a second release of the pending authentic owner. Each mode MUST allocate only its necessary fixture.

#### Scenario: Failed callback construction settles the retained owner

- GIVEN one pre-registered pending authentic owner in every mode, with no fixture for authentic, a prior retired generation for stale, or one live wrong-kind/foreign-context fixture
- WHEN a hostile returned claim refuses before resource construction, an authentic guest consumes its validated owner, or cleanup is repeated after either terminal outcome
- THEN the invocation reports one and only one pending-owner release, zero authentic or one hostile fixture release, no residual live owner or native pin, and no rejected claim becomes a guest-owned value
