# Core replacement preview (issue #77)

`:preview <absolute selected path>` compares the installed Core definition with one explicitly
selected replacement file, before any installation request. It uses the existing preparation,
interface/effect compatibility and dependent-rebuild checks; it never stages a worker binary,
publishes, executes either body or consumes a self-edit grant.

[`example-report.json`](example-report.json) is the exact report the command printed for the
invocation below with the built CLI; only the two path fields were shortened.

```
printf 'def addone [ 1 + ]' > math.noble
printf ':preview %s\n' "$PWD/math.noble" \
  | noble live repl --source "$PWD/math.noble" --engine v8 \
  | tail -n 1
```

The first JSON line is the initial `reload-committed` report; `tail -n 1` keeps the preview report
this lane documents. `installed_state.live_code_entries` is 1 on both sides because the initial
reload staged one binary before the preview; the preview itself adds none.

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
| `base.installed_identity_scope` | the installed definition's session-local identity at the base generation; it is not portable or authority |
| `candidate` | the selected path, its exact SHA-256 and byte count, the definition name, and a **prospective** identity that is not installed and not authority |
| `comparison.installed` | the installed definition's ordered inferred interface under `identity_scope: installed-at-base-generation`, with the identity it was read from |
| `comparison.candidate` | the candidate's ordered inferred interface under `identity_scope: prospective-not-installed` and its prospective identity |
| `comparison.interface_equal` | whether the two ordered descriptions are equal, and `null` when the installed side could not be inferred |
| `comparison.equality_scope` | equal descriptions cover the ordered inferred input/output types and admitted effects only: not behaviour and not acceptance |
| `comparison.equivalence` | always `not-established`: a passing structural check is not behavioural equivalence |
| `comparison.behaviour` | always `not-compared` |
| `dependents.rebuild` | current named dependents whose resolved calls reach the replaced definition, in declaration order |
| `checks.source_preparation`, `checks.dependent_rebuild` | actually performed by this preview |
| `checks.inert_submission` | the empty-body submission the preview prepared, with its scope stated: it is not either definition's interface |
| `checks.backend_admission`, `checks.publication` | `not-run`: a source-only preview makes no backend or publication claim |
| `checks.evidence` | `not-checked` with a null subject: no badge, theorem, receipt or conformance claim is created or copied from the installed definition |
| `token` | the candidate SHA-256 and base generation, with `authority: comparison-reference-not-publication-authority` |
| `installed_state` | measured before and after the operation (see below) |

### Ordered interfaces

Both sides use the stack-field and effect-list shape from the inspection format
(`noble-live-inspection/v1`): `{"stack_in": {"values": ["I64"], "depth": 1},
"stack_out": {"values": ["I64"], "depth": 1}, "effects": []}`. `values` are the ordered concrete
stack the source checker inferred, bottom first; `depth` is their count. The preview additionally
reports `stack_in_tail` and `stack_out_tail` to state whether each side still ends in an
unconstrained stack variable (`unconstrained`) or in a concrete stack (`closed`). An open side is
reported as open rather than padded with invented entries.

`effects` lists the finite host-effect identities the definition's latent bound admits, spelled by
their reserved names where one exists (`test.emit`, `test.abort`, `test.clock`, `live.propose`,
`live.observe-generation`, `live.dispatch`, `test.code-export`, `test.send`) and `effect:<id>`
otherwise; `effects_unresolved` says the set was not finite and is therefore not fully reported.
These are static inference results over retained source, not runtime observations.

### Measured installed state

`installed_state` is derived from one measured point before and one after the operation, never from
constants:

| Field | Meaning |
|---|---|
| `generation`, `submissions`, `live_code_entries` | `{before, after}` from the session's own namespace generation, submission count and live-code registry |
| `grant` | `{before, after}` active `:grant-self-edit` selection |
| `namespace_changed` | `generation.before != generation.after` |
| `staged_binaries` | the live-code registry's growth (`after - before`, saturating) |
| `grants_consumed` | 1 exactly when an active grant was present before and absent after |
| `submissions_charged` | the submission count's growth |
| `selected_source_acknowledgement` | `unchanged` or `changed`, from the selected-file digest the session holds |
| `guest_requests`, `protected_operations` | `0`, and `unreachable.fields` says why: this path holds no worker and issues no guest request |

## Refusals keep their own classes

`refusal.kind` names the contract that refused, `refusal.reason` is the unchanged diagnostic,
`refusal.stage` is the source stage the interrupted check stopped in (provenance beside the class,
never the class) and `refusal.category` is the diagnostic's own machine-facing category.

| `refusal.kind` | Meaning |
|---|---|
| `preview-selection` | not absolute, differs from the selected source, or changed the selected definition name |
| `source-snapshot` | the selected file could not be read as a bounded snapshot |
| `source-preparation` | the candidate could not be prepared as one checked definition |
| `unsupported-profile` | a declared-modules session, another namespace snapshot, or a candidate that is not one replaceable checked definition |
| `interface-change` | the replacement's ordered stack interface differs |
| `effect-growth` | the replacement admits an effect the installed definition does not (a drop is allowed) |
| `self-rebind` | the replacement transitively depends on its own previous identity |
| `dependent-rebuild` | a retained dependent no longer rebuilds to its own name, effects or resolved call targets |
| `work-exhausted` | a dependency walk, comparison or dependent replay exhausted its work or source budget |
| `retained-source` | a retained dependent's own retained source is missing or malformed |
| `comparison-failed` | a check returned a diagnostic the preview cannot classify; the diagnostic is preserved |

## Installation binding

The retained reference binds the **next** installation of that selected source:

- candidate bytes changed after the preview → `reload-refused` with
  "candidate bytes changed after the preview; preview again";
- base namespace generation changed after the preview → refused with
  "base namespace changed after the preview; preview again";
- a matching installation goes through the ordinary guarded path unchanged, reports
  `preview_token: consumed` and the exact committed `source_sha256` and generation, then clears the
  reference. A later installation with no retained reference claims no preview authority.

A refused installation does **not** discard the reference: it is kept as a stale marker, and every
later plain reload is refused with the recorded reason (`preview_state:
stale-retained-reference`, `preview_repeated` true from the second refusal on) until a new
`:preview` replaces it or `:preview cancel` discards it explicitly. Restoring the exact previewed
bytes in the same base therefore still needs a new preview.

Discarding, cancelling or refusing a preview changes no acknowledgement, stack, generation, grant or
runtime ownership; the candidate is never executed or staged.

## Not established here

- Behaviour, termination and interface equality as behaviour: never claimed. `interface_equal`
  compares the two ordered inferred descriptions and nothing more.
- Backend admission and publication: `not-run`, with no worker binary and no self-edit grant
  consumed.
- A declared-modules session: refused as `unsupported-profile`; its own `:reload` path is the review
  surface for immutable module roots.
