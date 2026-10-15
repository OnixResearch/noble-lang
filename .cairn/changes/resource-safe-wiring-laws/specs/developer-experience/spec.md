# Resource-safe wiring library vocabulary

## ADDED Requirements

### Requirement: WIRE-API-01
r[WIRE-API-01]

Each combinator in an optional wiring library MUST be an ordinary checked definition derived from existing words. Each combinator MUST state:

- its complete ordered stack signature, including the preserved tail;
- its latent effect bound;
- its evaluation order;
- its eligibility: move-only, `Data`, or `Capture`.

The library MUST NOT provide an unrestricted lifting operation from host functions. Programs enter the library only as checked quotations, named definitions, `quote` of `Capture` values, or results of `compose`, `case` or `if`.

Routing independently owned inputs MUST be distinct from fanout of one shared input.

- Fanout MUST require `Data` for every duplicated value.
- Projection or discard MUST require `Data` for every discarded value.
- Constant lifting MUST require `Capture`.

The kernel MUST enforce these premises at every specialization, even where a library signature cannot state them. Moving or framing a resource MUST preserve its identity and single owner. It MUST NOT implicitly copy, discard, release or capture the resource.

A combinator that takes two Programs MUST NOT claim a narrower combined effect bound than the bounds of its operands. Where the signature syntax can name only one effect slot, both operands MUST be adapted to that bound explicitly.
