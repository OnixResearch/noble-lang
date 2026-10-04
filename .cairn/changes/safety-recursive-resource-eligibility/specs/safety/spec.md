# Recursive resource eligibility at the actual WIT boundary

## MODIFIED Requirements

### Requirement: S-RES-01
r[S-RES-01]

A live resource/capability has an explicit ownership obligation in Noble. Generic duplication, generic discard, capture by `quote`, persistence, and serialization MUST remain unavailable for resource-bearing values unless a separately specified operation has semantics that preserve the resource contract.

For the selected S-CASE-04 static profile, an admitted WIT `own<counter>` returned by an imported host operation MAY form a Noble resource value or be combined **inside Noble** into `Pair<Text,Resource<R>>`, `Sum<Resource<R>,I64>` or `List<Resource<R>>`. The production checker MUST refuse `dup`, generic `drop` and `quote` capture on each complete resource-bearing value before component emission. The selected route to generic serialization is `quote reflect`: its refusal MUST occur at `quote`, before serializable Syntax exists. This does not provide an independent serializer or add WIT aggregate inputs.

#### Scenario: Exact resource matrix refuses before component publication

- GIVEN the real admitted `own<counter>` kind and Noble-built `Resource<R>`, `Pair<Text,Resource<R>>`, `Sum<Resource<R>,I64>` and `List<Resource<R>>`
- WHEN each value is used with `dup`, generic `drop`, `quote` or the `quote reflect` route
- THEN the source/kernel eligibility constraint identifies the whole resource-bearing type and exact Data-requiring word, and every component compile refuses before publication without guest execution or protected operations; a WIT parse, frontend type or lowering error is not this result

### Requirement: S-RES-04
r[S-RES-04]

Resource-containing aggregates carry the ownership obligation recursively and MUST NOT satisfy `Data` or `Capture` merely because a particular runtime variant does not currently expose the resource.

For the selected S-CASE-04 sum, the static checker MUST inspect both alternatives of `Sum<Resource<R>,I64>` when Noble source selects the I64 alternative with a false conditional. The whole sum remains ineligible for the same generic operations; after checked `case` elimination the I64 payload MAY be used at its own Data type without making the containing sum Data. The canonical `check/eligibility-reject` result is a harness classification of an exact typed production refusal, not a claim that the literal component CLI reports those stage/outcome strings. Source construction and source-only branch controls do not imply support for WIT tuple, resource-bearing result/list ABI, component lowering of every positive aggregate, or runtime execution of the selected branch.

#### Scenario: Unselected resource alternative does not inherit Data from I64

- GIVEN the Noble source `false [ counters.open inl ] [ 7 inr ] if` with a real WIT-bound `counters.open` resource kind
- WHEN the complete sum is passed to the generic operations, and separately a checked `case` exposes the right-hand I64 payload
- THEN the whole sum refuses with its complete `Sum<Resource<R>,I64>` eligibility diagnostic while a source-checked I64 branch may use its own Data operations; the static observations alone do not establish a runtime branch trace
