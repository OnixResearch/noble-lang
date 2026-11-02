# Core replacement preview (issue #77)

`:preview <absolute selected path>` compares the installed Core definition with one explicitly
selected replacement file, before any installation request. It uses the existing preparation,
interface/effect compatibility and dependent-rebuild checks; it never stages a worker binary,
publishes, executes either body or consumes a self-edit grant.

[`example-report.json`](example-report.json) is the exact report the command printed for
`def addone [ 1 + ]` + `:preview <path>` with the built CLI; only the two path fields were
shortened.

## Operation

`noble live repl --source math.noble --engine v8`, then

| Line | Effect |
|---|---|
| `:preview <path>` | reads one bounded candidate snapshot, prepares it, runs the existing preparation and dependent-rebuild checks, retains one comparison reference |
| `:preview cancel` | discards the reference; no installed state changes |
| `:reload <path>` | the ordinary explicit installation, which must still match a retained reference |

## Report (`noble-live-preview/v1`)

| Field | Meaning |
|---|---|
| `base` | the session's namespace generation, submission count and installed definition **at preview time** |
| `base.installed_identity_scope` | an installed identity is unavailable when the session grant names no definition |
| `candidate` | the selected path, its exact SHA-256 and byte count, the definition name, and a **prospective** identity that is not installed and not authority |
| `comparison.equivalence` | always `not-established`: a passing structural check is not behavioural equivalence |
| `comparison.behaviour` | always `not-compared` |
| `comparison.interface` | `unavailable-not-exposed-by-this-preview`: replacement compatibility is checked but neither definition's ordered interface is printed |
| `dependents.rebuild` | current named dependents whose resolved calls reach the replaced definition |
| `checks.source_preparation`, `checks.dependent_rebuild` | actually performed by this preview |
| `checks.inert_submission` | the empty-body submission the preview prepared, with its scope stated: it is not either definition's interface |
| `checks.backend_admission`, `checks.publication` | `not-run`: a source-only preview makes no backend or publication claim |
| `checks.evidence` | `not-checked` with a null subject: no badge, theorem, receipt or conformance claim is created or copied from the installed definition |
| `token` | the candidate SHA-256 and base generation, with `authority: comparison-reference-not-publication-authority` |
| `installed_state` | `staged_binaries`, `grants_consumed`, `guest_requests`, `protected_operations` at zero and `namespace_changed: false` |

## Refusals keep their own reasons

`refusal.stage` names where the refusal happened and `refusal.reason` is the unchanged diagnostic:

- `preview-selection`: not absolute, differs from the selected source, changed the selected definition
  name, or not one checked definition;
- `source-snapshot`: the selected file could not be read as a bounded snapshot;
- `source-preparation`: the candidate is not a checked definition;
- `interface-or-effects`: the replacement changes the checked stack interface or increases effects;
- `dependency-rebind-or-limits`: the replacement depends on its previous identity, or the rebuild
  work limit is exhausted.

## Installation binding

The retained reference binds the **next** installation of that selected source:

- candidate bytes changed after the preview → `reload-refused` with
  "candidate bytes changed after the preview; preview again";
- base namespace generation changed after the preview → refused with
  "base namespace changed after the preview; preview again";
- a matching installation goes through the ordinary guarded path unchanged, reports
  `preview_token: consumed` and the exact committed `source_sha256` and generation, then clears the
  reference. A later installation with no retained reference claims no preview authority.

Discarding, cancelling or refusing a preview changes no acknowledgement, stack, generation, grant or
runtime ownership; the candidate is never executed or staged.

## Not established here

- Behaviour, termination and interface equality: never claimed.
- Backend admission and publication: `not-run`, with no worker binary and no self-edit grant
  consumed.
- Dependent display for deeper graphs is reported by the frontend and exercised at the contracts
  layer (`Session::current_dependent_names`); the single-definition live profile cannot itself hold
  dependents, so `dependents.rebuild` is empty there.
