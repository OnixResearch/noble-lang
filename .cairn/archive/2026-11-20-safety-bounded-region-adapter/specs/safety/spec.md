# Three-byte host region safety

## MODIFIED Requirements

### Requirement: S-LANG-04
r[S-LANG-04]

A bounds violation, invalid variant/tag, malformed external value, stale handle, wrong handle kind/context, or equivalent representation error MUST be rejected before entering trusted Noble state or produce a specified failure. It MUST NOT authorize memory corruption or arbitrary execution. In the selected S-CASE-02 versioned bounded-region adapter, the host MUST validate the retained live owner, invocation/context/kind/generation/read right and the nonnegative signed offset and length, checked addition, region extent and output quota before any protected region byte read. Canonical `000102` at offset 3, length 1 MUST return `bounds-reject`, zero protected operations and zero region reads; offset 1, length 2 MUST return bytes `0102` through the same guest-callable operation. Offset 3, length 0 is an empty success; offset 4, length 0 and arithmetic overflow MUST reject. These finite tests do not establish universal native safety.

#### Scenario: S-CASE-02 checked refusal and actual guest control

- GIVEN the exact S-CASE-02 input and a separate host-selected three-byte resource passed as a typed owner to a compiled guest
- WHEN the guest invokes the versioned region adapter with offset 3 and length 1, and positive/boundary/overflow/invalid-owner controls invoke that same boundary
- THEN the canonical case matches every field of its expected record before any protected read, the valid control returns bytes `0102`, and failures neither read the region nor grant arbitrary native access

### Requirement: S-MEM-01
r[S-MEM-01]

A conforming host MUST NOT expose a guest operation that permits arbitrary reads or writes to host/native memory by numeric address. Guest-visible memory operations MUST be mediated by typed values, bounded byte regions, validated component memories, or opaque resources with documented contracts. The selected versioned bounded-region world MUST supply an immutable host-owned byte region through an opaque invocation-owned resource with an explicit read right. Host table and owner-context identities MUST remain distinct between simultaneously live invocations, with bounded nonwrapping allocation and refusal on exhaustion; an equal slot/generation from another host Store is not authority. Offset/length values are relative to that selected region only; they MUST NOT be interpreted as native addresses or offsets in the separate 16-page Noble Core Wasm memory. A valid read copies only a fully prevalidated bounded subrange, while an invalid read MUST NOT enter the protected region, start native work or consume its owner. A successful or refused borrowed read returns the identical owner for one explicit release or host cleanup on abnormal exit. `native_memory_access=false` for S-CASE-02 means no unchecked host/native-address dereference, not absence of ordinary safe host bookkeeping memory operations.

#### Scenario: S-CASE-02 host-selected three-byte extent

- GIVEN the exact three bytes `00 01 02` separately selected by the host and retained behind the invocation resource, not embedded into guest memory
- WHEN the compiled guest submits the canonical out-of-range read and a positive in-bounds read through the actual host method
- THEN `(3,1)` is `bounds-reject` with zero protected operations and no region bytes accessed, while `(1,2)` returns `01 02` and the owner is released once
