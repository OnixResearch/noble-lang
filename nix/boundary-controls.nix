{
  pkgs,
  src,
  rust,
  nickel,
  octetGate,
  cargoOctet,
  selection,
}:
let
  policy = builtins.fromJSON (builtins.readFile ../policy/boundary-controls.json);
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
        export CARGO_BUILD_JOBS=1
        export NIX_CONFIG='experimental-features = nix-command'
        unset RUSTC_WRAPPER RUSTC_WORKSPACE_WRAPPER RUSTFLAGS CARGO_ENCODED_RUSTFLAGS DYLINT_RUSTFLAGS
        mkdir -p "$CARGO_HOME" "$out"
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
assert policy.timeout_seconds > 0 && policy.timeout_seconds <= 180;
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
