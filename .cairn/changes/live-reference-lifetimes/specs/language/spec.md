# Live-reference counters and escaped results

## ADDED Requirements

### Requirement: LREF-CTR-01
r[LREF-CTR-01]

In `Live-Slot-Design`, the following counters MUST NOT wrap or be reissued:

- the global registry epoch;
- slot generation;
- slot incarnation;
- the code-retention revision;
- guest handle epochs.

When a publication, bootstrap, slot recreation or code retirement would exceed a counter's maximum, that operation MUST refuse explicitly with a distinct exhaustion diagnostic. The published registry MUST stay unchanged, no candidate body executes, and existing root pins and saved Programs MUST remain valid.

#### Scenario: LREF-CASE-02 exhausted counters refuse without wrapping

- GIVEN registry fixtures at the maximum global epoch, slot generation, slot incarnation and code-retention revision, as in LREF-CASE-02
- WHEN publication, recreation and code retirement are attempted
- THEN each refuses with an exhaustion diagnostic, the registry is unchanged, and held roots and saved Programs still run

### Requirement: LREF-ESCAPE-01
r[LREF-ESCAPE-01]

A `Program` value returned or saved from a root and produced by a slot-selected target MUST be an ordinary exact immutable Program. Its identity MUST be its own recipe and capture lineage, not the slot identity or the generic caller's identity. It MUST contain or reach no `LiveRef`, MUST own its executable package, and MUST NOT follow any later slot publication. Its reflection, evidence applicability and replay MUST be those of exact code.

Within one root, slot-selected versions from the pinned map MAY coexist with exact saved Programs supplied as ordinary inputs. The root MUST NOT select two different versions of the same slot.

A saved generic caller containing `slot.invoke` MUST require a new root, a freshly pinned map and freshly host-issued refs for each invocation. A ref retained from another root MUST be refused before dispatch.

#### Scenario: LREF-CASE-04 escaped result is not live

- GIVEN a Program saved from target v1 before v2 publishes, and a saved generic caller invoked in two roots with fresh and retained refs, as in LREF-CASE-04 and LREF-CASE-05
- WHEN the saved Program and the generic caller are invoked after publication
- THEN the saved Program keeps v1 behavior and lineage identity, fresh roots select v2, and the retained ref is refused
