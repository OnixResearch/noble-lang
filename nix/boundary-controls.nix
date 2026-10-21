{
  pkgs,
  src,
  rust,
  nickel,
  octetGate,
  standards,
  cargoOctet,
  wasmtimeSource,
  reviewedVendor,
  reviewedCargoConfig,
  selection,
}:
let
  policy = builtins.fromJSON (builtins.readFile ../policy/boundary-controls.json);
  # The reviewed --all-features plan rows for the selected target; the
  # comparator accepts these or the default configuration, nothing else.
  declaredFeatureSets = map (row: row.features) (
    builtins.filter (row: row.triple == selection.configuration.target) (
      builtins.fromJSON (builtins.readFile ../policy/architecture.json)
    ).production.targets
  );
  files = builtins.listToAttrs (
    map
      (name: {
        inherit name;
        value = builtins.readFile (src + "/${name}");
      })
      [
        policy.kernel_source
        "crates/noble-kernel/Cargo.toml"
        "crates/noble-cli/Cargo.toml"
      ]
  );
  selfTests =
    (import ./boundary-observation-tests.nix { inherit policy selection; })
    ++ (import ./boundary-fixture-tests.nix {
      inherit files policy;
      inherit (pkgs) lib;
    });
  cases = builtins.listToAttrs (
    map (case: {
      name = case.id;
      value = case;
    }) policy.cases
  );
  randomnessLock = src + "/${policy.randomness_dependency.lockfile}";
  randomnessPackages = (builtins.fromTOML (builtins.readFile randomnessLock)).package;
  randomnessVendor = pkgs.rustPlatform.importCargoLock { lockFile = randomnessLock; };
  randomnessCargoConfig = pkgs.writeText "boundary-randomness-cargo-config" ''
    [source.crates-io]
    replace-with = "vendored-sources"
    [source.vendored-sources]
    directory = "${randomnessVendor}"
    [net]
    offline = true
  '';
  caseNames = [
    "workspace"
    "randomness-dependency"
    "acyclic-shell"
    "filesystem"
    "network"
    "process"
    "environment"
    "clock"
    "randomness"
    "global-state"
    "interior-state"
    "unsafe-block"
    "unsafe-function"
    "unsafe-foreign-declaration"
    "host-return"
    "test-source-import"
    "no-std-test-witness"
    "reverse-dependency"
  ];
  mkCase =
    case:
    let
      mutations = import ./boundary-fixture.nix {
        inherit files case policy;
        inherit (pkgs) lib;
      };
      positive = case.observation == "clean";
      compilerOnly = case.observation == "compiler_only";
      policyBlocked = case.observation == "policy";
      precollection = case.observation == "precollection";
      auxiliary =
        !positive
        && !compilerOnly
        && (!policyBlocked || case.deny_lint != "")
        && case.observation != "role_edge";
      usesRandomnessDependency = builtins.elem case.mutation [
        "randomness"
        "randomness_dependency"
      ];
      baseline = if case.base == "acyclic_shell" then outputs.acyclic-shell else outputs.workspace;
      writeMutations = pkgs.lib.concatStringsSep "\n" (
        pkgs.lib.mapAttrsToList (path: contents: ''
          mkdir -p "$(dirname ${pkgs.lib.escapeShellArg path})"
          cp ${pkgs.writeText "boundary-fixture-source" contents} ${pkgs.lib.escapeShellArg path}
        '') mutations
      );
    in
    pkgs.runCommand "noble-boundary-${case.id}"
      {
        buildInputs = [ wasmtimeSource reviewedVendor ];
        nativeBuildInputs = [
          rust
          pkgs.stdenv.cc
          pkgs.coreutils
          pkgs.nix
          pkgs.b3sum
          octetGate
          cargoOctet
        ];
      }
      ''
        ${pkgs.lib.optionalString (!positive) "test -f ${baseline}/validated.txt"}
        export HOME="$TMPDIR/home"
        export CARGO_HOME="$HOME/cargo"
        export CARGO_TARGET_DIR="$TMPDIR/deny-target"
        # Build parallelism changes scheduling only; every unit still yields its
        # own complete shard, and the comparator requires the exact roster.
        jobs="$NIX_BUILD_CORES"
        if [ -z "$jobs" ] || [ "$jobs" -le 0 ]; then jobs=$(nproc); fi
        export CARGO_BUILD_JOBS="$jobs"
        export NIX_CONFIG='experimental-features = nix-command'
        unset RUSTC_WRAPPER RUSTC_WORKSPACE_WRAPPER RUSTFLAGS CARGO_ENCODED_RUSTFLAGS DYLINT_RUSTFLAGS
        mkdir -p "$CARGO_HOME" "$out"
        ${pkgs.lib.optionalString (!usesRandomnessDependency) ''
          cp ${reviewedCargoConfig} "$CARGO_HOME/config.toml"
        ''}
        cp -r ${src} source
        chmod -R u+w source
        cd source
        ${writeMutations}
        ${pkgs.lib.optionalString usesRandomnessDependency ''
          cp ${randomnessCargoConfig} "$CARGO_HOME/config.toml"
          cp ${randomnessLock} Cargo.lock
          cp "$CARGO_HOME/config.toml" "$out/cargo-config.toml"
        ''}
        cargo generate-lockfile --offline > "$out/lock.log" 2>&1
        ${pkgs.lib.optionalString usesRandomnessDependency ''
          cmp Cargo.lock ${randomnessLock}
        ''}
        cp Cargo.lock "$out/fixture-Cargo.lock"
        # The policy and all targets stay present. No negative fixture is executed.
        cmp policy/architecture.json ${src}/policy/architecture.json
        cmp policy/architecture.ncl ${src}/policy/architecture.ncl
        cmp Cargo.toml ${src}/Cargo.toml
        b3sum Cargo.toml Cargo.lock crates/*/Cargo.toml crates/*/src/*.rs crates/*/tests/*.rs > "$out/source-blake3.txt"
        tar -cf "$out/source.tar" Cargo.toml Cargo.lock rust-toolchain.toml dylint.toml crates policy
        ${pkgs.lib.optionalString (case.base == "acyclic_shell") ''
          # Issue #27: the acyclic-shell base adds two lib units beyond the reviewed
          # workspace roster. This fixture alone re-exports the architecture policy
          # at the reviewed capacity plus two with the same deterministic exporter
          # the reviewed check uses, so the export manifest stays consistent. Every
          # other case keeps the reviewed capacity. The bump is derived from the
          # policy so a renewed roster cannot silently leave this fixture behind.
          cp policy/architecture.ncl "$out/architecture-acyclic.ncl"
          reviewed_units=$(grep -oE '^    max_units = [0-9]+,' policy/architecture.ncl | grep -oE '[0-9]+')
          reviewed_shards=$(grep -oE '^    max_shards = [0-9]+,' policy/architecture.ncl | grep -oE '[0-9]+')
          test -n "$reviewed_units" && test "$reviewed_units" = "$reviewed_shards"
          sed -i "s/^    max_units = $reviewed_units,/    max_units = $((reviewed_units + 2)),/; s/^    max_shards = $reviewed_shards,/    max_shards = $((reviewed_shards + 2)),/" \
            "$out/architecture-acyclic.ncl"
          grep -qE "^    max_units = $((reviewed_units + 2)),$" "$out/architecture-acyclic.ncl"
          mv "$out/architecture-acyclic.ncl" policy/architecture.ncl
          # The exporter reads its --output file, so export the policy first and
          # then derive the manifest from it, exactly as the reviewed check does.
          ${nickel}/bin/nickel export --format json policy/architecture.ncl \
            > "$out/architecture.json"
          ${nickel}/bin/nickel export --format json policy/architecture-export-spec.ncl \
            > "$out/export-spec.json"
          ${standards}/bin/octet-standards nickel-export-manifest \
            --spec "$out/export-spec.json" --output "$out/architecture.json" \
            --manifest-out "$out/architecture.manifest.json" \
            --nickel-identity "$(${nickel}/bin/nickel --version)"
          # Keep the exports in $out as case artifacts and use copies as the policy.
          cp "$out/architecture.json" policy/architecture.json
          cp "$out/architecture.manifest.json" policy/architecture.manifest.json
        ''}
        code=0
        timeout ${toString policy.timeout_seconds} noble-octet-gate --workspace --output-format json \
          --artifact-dir "$out/deny" -- --all-targets --all-features > "$out/deny.stdout" 2> "$out/deny.stderr" || code=$?
        printf '%s\n' "$code" > "$out/deny-exit.json"
        ${pkgs.lib.optionalString auxiliary ''
          # Separate fact probe, NOT a replacement for the required deny-all result.
          export CARGO_TARGET_DIR="$TMPDIR/observation-target"
          code=0
          timeout ${toString policy.timeout_seconds} cargo-octet check --workspace --output-format json \
            --artifact-dir "$out/observation" -- --all-targets --all-features > "$out/observation.stdout" 2> "$out/observation.stderr" || code=$?
          printf '%s\n' "$code" > "$out/observation-exit.json"
        ''}
        # Preserve raw context even when replay or comparison rejects the run.
        if [ -d .octet ]; then tar -cf "$out/raw-compiler-observations.tar" .octet; fi
        ${pkgs.lib.optionalString precollection ''
          # Pre-collection rejection: capture the diagnostic and confirm no bundle exists.
          cat "$out/deny.stderr" > "$out/precollection-stderr.txt"
          cat "$out/deny.stderr" > "$out/precollection-diagnostic.txt"
        ''}
        ${pkgs.lib.optionalString (!compilerOnly && !precollection) ''
          if ! cargo-octet artifact verify --artifact-dir "$out/${
            if auxiliary then "observation" else "deny"
          }" \
            --output-format json > "$out/replay.json"; then
            printf '%s\n' 'boundary control ${case.id}: artifact replay failed; not an accepted rejection' >&2
            exit 1
          fi
        ''}
        NIX_REMOTE=dummy:// nix-instantiate --eval --strict --expr '
          import ${./.}/boundary-read-observation.nix {
            directory = "'"$out"'";
            policy = builtins.fromJSON (builtins.readFile ${../policy/boundary-controls.json});
            selection = builtins.fromJSON (builtins.readFile ${../policy/tool-selection.json});
            case = builtins.fromJSON (builtins.readFile ${pkgs.writeText "boundary-case.json" (builtins.toJSON case)});
            declaredFeatureSets = builtins.fromJSON (builtins.readFile ${pkgs.writeText "boundary-declared-features.json" (builtins.toJSON declaredFeatureSets)});
          }
        ' > "$out/validated.txt"
      '';
  outputs = builtins.mapAttrs (_: mkCase) cases;
  shellLinks = pkgs.lib.concatStringsSep "\n" (
    map (id: ''
      cp -r ${outputs.${id}} "$out/${id}"
    '') caseNames
  );
in
assert builtins.deepSeq selfTests true;
assert policy.schema_version == "noble-boundary-controls/v1";
assert policy.timeout_seconds > 0 && policy.timeout_seconds <= 1800;
assert policy.target == selection.configuration.target;
assert policy.randomness_dependency.name == "getrandom";
assert policy.test_source.escaped == "crates/noble-kernel/src/" + policy.test_source.relative;
assert builtins.match ".*/tests/support/production_import.rs" policy.test_source.path != null;
assert
  builtins.length (
    builtins.filter (
      p:
      p.name == policy.randomness_dependency.name
      && p.version == policy.randomness_dependency.version
      && p.source == "registry+https://github.com/rust-lang/crates.io-index"
      && p ? checksum
    ) randomnessPackages
  ) == 1;
assert map (c: c.id) policy.cases == caseNames;
assert builtins.all (
  c:
  (builtins.elem c.id [
    "workspace"
    "acyclic-shell"
  ]) == (c.observation == "clean" && c.mutation == "none")
) policy.cases;
{
  inherit outputs;
  check =
    pkgs.runCommand "noble-boundary-controls"
      {
        nativeBuildInputs = [
          nickel
          pkgs.diffutils
        ];
      }
      ''
        nickel export --format json ${../policy/boundary-controls.ncl} > generated.json
        cmp generated.json ${../policy/boundary-controls.json}
        mkdir "$out"
        ${shellLinks}
        printf '%s\n' ${pkgs.lib.escapeShellArg (builtins.concatStringsSep "\n" selfTests)} > "$out/comparator-tests.txt"
        printf '%s\n' '2 positive baselines and 16 rejected boundary fixtures; not the complete M1 matrix' > "$out/controls.txt"
      '';
}
