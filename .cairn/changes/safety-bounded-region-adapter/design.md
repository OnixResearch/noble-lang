## Context

The canonical S-CASE-02 record in `specs/conformance/safety-cases.json` remains `Resources-Draft/adapter`, `000102`, offset 3, length 1; expected `bounds-reject`, protected operations zero, native memory access false. Existing S-CASE-03 proves only absent resource slot 42 with one host registration; S-CASE-08 callback matrix remains unexecuted. A separate S-CASE-16 final-source receipt is historical for its selected source.

## Decisions

### Decision: A separate versioned component boundary

**Choice:** `noble-test:bounded-region/bounded@1.0.0` imports `regions.region.read(s64,s64)->result<list<u8>,string>` and `regions.release(own<region>)`; its `read-region` export consumes one host-injected owner and returns one result. Actual compiled Noble body `region.read swap regions.release` threads the borrow, preserves the result and releases the owner on either normal branch. The host's `component read-region COMPONENT HOST_BUFFER_HEX OFFSET LENGTH` route selects actual bytes before creating a fresh Wasmtime Store, registers a resource with a private production Table and passes a typed own resource, not a raw pointer. No bootstrap WIT, `values.mjs` or general guest memory semantics change. r[S-MEM-01]

**Rationale:** The existing parser and compiler support signed s64, result bytes/string, resource methods and own/borrow threading, but not u64 or multi-result tuple. An explicit owner input gives the invoker rather than guest the 3-byte object. Effect names remain the exact versioned read and release imports; the authorization right is a separate host grant, not an effect manifest.

### Decision: Pre-read refusal and bounded ownership

**Choice:** Retain immutable host bytes at most 4096 and a private owner bound to one table/context/kind/generation/read-right. Assign TableId and Context from an atomic monotonic process-local counter shared across Host instances; never wrap or recycle while any Store may retain an owner, and refuse exhaustion. The host does not import serialized handles from a terminated process. On method call, check Wasmtime's resource representation and borrowed receiver, then retained Table validation with host-selected requirement; check signed nonnegative offset/length, checked addition, extent and maximum output before taking a native pin or reading the region. Only a valid request may `begin`, `native_access`, copy the checked byte range, `complete` and return the owner, then guest explicitly releases it. `(3,0)` succeeds with empty bytes; `(3,1)`, `(2,2)`, `(4,0)`, negative values and i64 overflow reject. Refusals do not read the protected region or commit a protected read; `native_memory_access=false` means no arbitrary host/native-address access, not no normal Rust housekeeping memory loads. r[S-LANG-04] r[S-MEM-01]

**Rationale:** Checking guest Wasm memory of 16 pages would admit `(3,1)` and silently substitute another object. `Table::validate` alone is read-only preflight, not authorization to start native work; positive path uses `begin/native_access/complete`. Separate counters witness that rejection did not reach a protected read and the positive path did.

### Decision: Source-bound evidence without rewriting history

**Choice:** The original source-bound gate binds canonical input/expected and complete production source/WIT, compiled binary, host command and raw external outputs; positive bytes and owner lifecycle, boundary and arithmetic refusals, and forged/wrong-context/wrong-kind/retired resource claims are distinct controls. Its receipt/evidence[0] remains byte-exact historical evidence for that pre-identity-fix source. A new corrected gate must independently bind the atomic uniqueness fix, exercise two simultaneously live hosts with equal slot/generation but distinct TableId/Context and reject the copied cross-host claim before byte access, then replay the canonical compiled guest/host negative and positive. Append evidence[1] only after this new gate succeeds; preserve both receipts/runners and require a fresh final source/replay release after all changes. Never claim generic native safety, backend refinement, host authenticity or owner-law proof.

**Rationale:** Document validation, unit slice tests and the historical current-source replay cannot establish this new adapter's execution or updated source revision.

## Risks / Trade-offs

- Selected Wasmtime 40.0.2, Canonical ABI linking, invoker-supplied immutable region lifetime and Rust host implementation remain trusted. The CLI accepts a component path; finite receipt must bind the exact compiled guest bytes, not assume any component's result came from the region.
- Guest code that traps before explicit release requires host cleanup; a normal borrowed read must return the same owner before release. Native bytes are a host-owned copy, not guest Wasm pages or an arbitrary numeric-address capability.
