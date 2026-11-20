# Owner-law records for proof-required release (VC-OWNER-01, CONTRACT-16)

The two records in `laws/` are the frozen owner laws for CONTRACT-16.
The repository owner instructed on 2026-11-20 that they be frozen exactly as CONTRACT-16's
input specifies: `OwnerLaw.increment`, and an owner-approved v2 revision
(`OwnerLaw.addTwo`) for the `owner-approves-new-law-revision-and-fresh-exact-proof` variant.
They were committed before any consumer or candidate edit. Candidate authors never edit them
(`candidate_author_can_edit_owner_law: false`). A changed byte is a new law identity.

| Record | Revision | SHA-256 |
|---|---|---|
| `laws/owner-law-v1.json` | `owner-law-v1-immutable-before-candidate-edit` | `9a8eda12c74356ca0d312be840411d4445c6f377c16ee7b96a680581b2e91eac` |
| `laws/owner-law-v2.json` | `owner-law-v2-owner-approved-revision` | `51f2a3158155077dcb29c8f354d356d31df3282b3c76f60c2bc8534c7932ee80` |

A record states the subject interface, the effect-free fragment, the theorem declaration,
the expected statement in natural language and as an MC1 contract obligation (inputs,
outputs, `requires`, `ensures`), initial and final observations, normal-return scope,
preconditions, assumptions, the proof-library requirement and the consumer policy. It does
not know the candidate's body, recipe, proof or emitted artifact.
