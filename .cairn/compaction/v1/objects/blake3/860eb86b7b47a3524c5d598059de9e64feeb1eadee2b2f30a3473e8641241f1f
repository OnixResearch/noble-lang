# Published body-owner repair adoption

Noble remains 5/21 complete. Task 2.2 and independent M1 acceptance remain open.

## Published prerequisite

Octet published `e3e705a7c04a30b814000b251d245cf083894e4a` before Noble changed either pin.
Its archived change preserves compiler-owned constant callers without invalid signature or visibility queries.
No schema, role, lint level, or production obligation was relaxed.
Provider evidence is retained under the primary repository's `.pi/m1-inline-owner/`.

The provider passed 879 scoped tests. Eleven explicit controls include ten actual compiler runs with ten units each and one precollection rejection.
Four UI harness tests and thirty overlapping compiler-target tests also passed.
Tested, final-built, and published collector bytes match: `95762941b0902d497fcce5173c626ff5f26c7094a5cdc538552fa6810f7c2b0b`.
Both retained Noble candidate bundles replay with the published runner.

## Reviewed adoption

Starting Noble staged tree: `4dea0a1db3c1ae0207cfac8f67eaab9783c27190`.
Nix and pre-commit now use the same published revision.
Nix regenerated only the Octet lock node (task 2476 in the adoption record).
The new NAR hash is `sha256-z1HwSDUpiU5XYIoffw/QEFolojKd1Mp31uC5NpZ2KRA=`.

The reviewed Nickel selection changed only the Octet source identity, two affected tool recipe/output pairs, and three affected file hashes.
Those file bindings are `flake.nix`, `flake.lock`, and `.pre-commit-config.yaml`.
The JSON export was regenerated explicitly before checking. Checks do not update their own expectations.
Other source pins, tool recipes, execution configuration, and backend Git revisions remain selected unchanged.

## Observed checks

Task 2477 passed all eleven flake checks after the pin and selection update.
The unchanged boundary harness now matches two positive baselines and eleven negative fixtures.
The thread-local negative requires its intended deny-all diagnostic, both kernel production observations, and valid complete replay.
The historical compiler crash is retained; it is not counted as a successful rejection.

Five workspace Rust tests and 100 Bun regressions pass, with 26 conversion and 226 document self-tests.
No production Rust source, Cargo manifest, architecture policy, fixture, target, role, or extraction input changed.
The CLI remains an internal smoke shell. No fixture effect operation is executed.

Final document validation, direct hook checks, retained bundle replay, and source/command snapshots are recorded in `.pi/m1-inline-adoption/`.
These are incremental checks. Randomness, complete source/native accounting, proof obligations, integrated controls, CI, and independent acceptance remain open.
No Noble lifecycle or publication follows solely from this adoption.
