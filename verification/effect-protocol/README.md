# Effect-protocol scenario lane (EPROTO-CASE-01…11)

`execution.mjs` executes the `behavioral-effect-protocol` scenarios against the bounded-region adapter. It uses the CLI built from the checked-out commit and the `noble-bounded-region/v2` report. `monitor.mjs` replays each recorded trace against the RegionProtocol-v1 transition table.

Run it from a clean checkout with the selected Node and an empty environment:

```sh
env -i HOME="$HOME" PATH=/run/current-system/sw/bin GIT_TERMINAL_PROMPT=0 \
  "$(node -p 'require("./policy/tool-selection.json").tool_paths.node.output')/bin/node" \
  verification/effect-protocol/execution.mjs NEW_EXTERNAL_DIRECTORY
```

The lane does the following:

1. It builds `noble-cli` offline from the reviewed vendor.
2. It compiles the subject `region.read swap regions.release` (`verification/host-locality/fixtures/read-release.noble`).
3. It assembles the hostile components with the selected `wasm-tools`. Each is a core module written in `execution.mjs` against the world's synchronous memory32 Canonical ABI, so none derives from Noble compiler output.
4. It runs every scenario as one public `noble component read-region --report v2` invocation.
5. It checks each expected observation, then the monitor's verdicts.

Every executed scenario must end with no live owner, no native pin, no lost owner and no native memory access. Unless its trace is truncated, it must also hold exactly one release commit. `NEW_EXTERNAL_DIRECTORY/execution.json` retains, for every scenario:
- the full v2 report, which includes the trace prefix;
- the monitor result;
- the component bytes, in base64, with their hashes;
- the generated WAT;
- every command, with its output hashes.

The retained receipt `execution-3ef3f43a.json` is that file from the clean commit `3ef3f43a`.
The executed scenarios are registered as `specs/conformance/effect-protocol-cases.json`, bound to that receipt by SHA-256. The canonical EPROTO requirements, promoted when the `behavioral-effect-protocol` change was archived, link them. Two variants stay unregistered and are named in their cases' assumptions:
- case 03's `native_access` refusal, which is not exercised;
- case 11's host-produced unmodeled responses, which are covered only at the trace level.

## Scenarios

| Case | Component | Observation |
|---|---|---|
| 01 | compiled subject, `(1,2)` and `(3,0)` | `read` of `0102` and of nothing. The trace is a read, its commit, a value response and a guest release. No verdict. |
| 02 | compiled subject, `(3,1)`, `(2,2)`, `(-1,1)`, `(i64::MAX,1)` | `bounds-reject` with no commit and a `bounds` refusal, then the guest release. |
| 03 | hostile: 65 reads | 64 commits. The 65th read is refused as `scope-exhausted` and the guest sees `invalid-handle`. The trace is truncated at 64 events, so the monitor judges only the prefix. Forged and cross-host claims are direct `Host::read` fixtures, because a guest always presents the retained handle. |
| 04 | hostile: read, then `unreachable` | `trap` after the commit; the host retires the owner for reason `trap`. |
| 05 | hostile: `unreachable` first | `trap` with no read; the host retires the owner. |
| 06 | compiled subject, `--fuel` | A search finds the smallest fuel that reaches the read. At that fuel the quota is exhausted after the commit and before the release. One unit less never reaches the read. |
| 07 | hostile and compiled, `--cancel-after-reads 1` | `cancelled` after the first answered read. A second read never reaches the protocol, and a guest that only computes is interrupted. Without the control, the same guest exhausts its quota. |
| 08 | hostile: read, no release | `read` returns `0102`; the host retires the owner for reason `omitted-release`, and the monitor reports `omitted-release`. |
| 09 | hostile: release twice, or release then read | One release commit and no read after it. Wasmtime's handle table refuses the consumed handle (`unknown handle index`) before the call reaches the host, as an untyped error rather than a trap, so the adapter reports `adapter-invalid`. |
| 10 | Noble source out of order | `regions.release region.read` and `[ [ regions.release ] dip ] dip region.read` are refused at check time (`component-check`), and no component is produced. |
| 11 | hostile: `err("timeout")`, or the host's `ok` one byte short | `outside-model`, and the monitor reports `outside-model`. |

Ten controls mutate the real case 01 trace into traces the transition table forbids. The monitor must report each one:
- `omitted-release`;
- `double-release`;
- `post-release-use`;
- `unsolicited-response`;
- `outside-model`, twice: a host's `ok` one byte short of its request, and a host's `err("timeout")`;
- `uncommitted-read`;
- `unanswered-request`;
- `unsolicited-commit`;
- `release-count`.

## Limits

- A monitor verdict is evidence about one finite execution, not a protocol proof. A call the runtime refused before it reached the host leaves no event to judge.
- Case 03's `native-access` refusal is not exercised. `Host::read` passes `native_access` only the borrow `begin` has just opened, and T4 proves the table authorizes it (`open_borrow_returns`). No direct fixture reaches the refusal without corrupting table state.
- Case 11 executes unmodeled results returned by the guest. The audited host produces only `ok` bytes of the requested length, `bounds-reject` or `invalid-handle` (A3), and no modified host is built. What such a host would record is covered only at the trace level, by the two host-side monitor controls.
- `--cancel-after-reads` and `--fuel` are test knobs. Cancellation makes the host refuse every later call and set a Wasmtime epoch deadline, so a computing guest is interrupted at its next function entry or loop header. It is not an asynchronous cancellation API.
