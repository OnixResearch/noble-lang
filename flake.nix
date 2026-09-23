{
  description = "Noble workspace tools, bounded Wasm experiments and incremental quality gates";

  inputs = {
    octet.url = "git+ssh://git@github.com/OnixResearch/octet?rev=235255bc4972ced9128fd5b4d1ec66ff7508ded4";
    aeneas.url = "github:AeneasVerif/aeneas/505b6ca35217e7be5c96c3e2f8045edfbdf47291";
    cairn.url = "git+ssh://git@github.com/OnixResearch/cairn?rev=15f00875562025e7ea7e0d1f4af24d1a2e2ac06f";
    nixpkgs.follows = "octet/nixpkgs";
    rust-overlay.follows = "octet/rust-overlay";
  };

  outputs =
    inputs@{
      self,
      nixpkgs,
      rust-overlay,
      octet,
      aeneas,
      cairn,
    }:
    let
      system = "x86_64-linux";
      pkgs = import nixpkgs {
        inherit system;
        overlays = [ (import rust-overlay) ];
      };
      rust = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
      nickel = octet.packages.${system}.nickel-1-17-0;
      standards = octet.packages.${system}.octet-standards;
      selectedAeneas = aeneas.packages.${system}.aeneas.overrideAttrs (old: {
        # Isolate this input from the flake's containing store path: tool pins
        # must not change recursively whenever the reviewed policy is renewed.
        patches = (old.patches or [ ]) ++ [
          (builtins.path {
            path = ./nix/aeneas-string-escaping.patch;
            name = "aeneas-string-escaping.patch";
          })
        ];
      });
      wasmVerificationTools = {
        node = pkgs.nodejs_24;
        wasm_tools = pkgs.wasm-tools;
        binaryen = pkgs.binaryen;
      };
      selectionPolicy = builtins.fromJSON (builtins.readFile ./policy/tool-selection.json);
      selectionObservation =
        (import ./nix/tool-selection-observation.nix {
          inherit inputs system;
          root = ./.;
        })
        // {
          verification_tool_versions = builtins.mapAttrs (_: tool: tool.version) wasmVerificationTools;
          tool_paths =
            builtins.mapAttrs
              (_: tool: {
                derivation = tool.drvPath;
                output = toString tool;
              })
              ({
                aeneas = selectedAeneas;
                charon = aeneas.packages.${system}.charon;
                inherit lean nickel;
                quality_rust = rust;
                extraction_rust = extractionRust;
                octet = octet.packages.${system}.cargo-octet;
                octet_standards = standards;
                cairn = cairn.packages.${system}.cairn;
                upstream_pin_check = aeneas.checks.${system}.check-charon-pin;
              } // wasmVerificationTools);
        };
      selection = import ./nix/tool-selection.nix {
        policy = selectionPolicy;
        observation = selectionObservation;
      };
      selectionTests = import ./nix/tool-selection-tests.nix {
        policy = selectionPolicy;
        observation = selectionObservation;
      };
      extractionRust = aeneas.inputs.charon.packages.${system}.rustToolchain;
      lean = import ./nix/lean-release.nix {
        inherit pkgs;
        selection = selectionPolicy;
      };
      toolchainCheck = import ./nix/tool-selection-app.nix {
        inherit
          pkgs
          rust
          extractionRust
          lean
          ;
        policy = selectionPolicy;
        charon = aeneas.packages.${system}.charon;
        aeneas = selectedAeneas;
        upstreamPinCheck = aeneas.checks.${system}.check-charon-pin;
        node = wasmVerificationTools.node;
        wasmTools = wasmVerificationTools.wasm_tools;
        binaryen = wasmVerificationTools.binaryen;
      };
      leanDependenciesCheck = import ./nix/lean-dependency-check.nix {
        inherit pkgs;
        packages = selectionObservation.lake_manifest.packages;
      };
      src = pkgs.lib.cleanSourceWith {
        src = ./.;
        filter =
          path: type:
          !(builtins.elem (baseNameOf path) [
            ".git"
            ".pi"
            ".lake"
            ".octet"
            "target"
            "node_modules"
          ])
          && !(builtins.elem (baseNameOf path) [
            "NobleKernel.lean"
            "translation.json"
          ]);
      };
      noble = (pkgs.makeRustPlatform {
        cargo = rust;
        rustc = rust;
      }).buildRustPackage {
        pname = "noble";
        version = (builtins.fromTOML (builtins.readFile ./Cargo.toml)).workspace.package.version;
        inherit src;
        cargoLock.lockFile = ./Cargo.lock;
        cargoBuildFlags = [ "--package" "noble-cli" "--bin" "noble" "--all-features" ];
        cargoTestFlags = [ "--workspace" "--all-targets" "--all-features" ];
        nativeCheckInputs = builtins.attrValues wasmVerificationTools;
        strictDeps = true;
      };
      m3Wasm = import ./nix/m3-wasm-app.nix {
        inherit pkgs src noble;
        node = wasmVerificationTools.node;
        wasmTools = wasmVerificationTools.wasm_tools;
        binaryen = wasmVerificationTools.binaryen;
      };
      boundaryControls = import ./nix/boundary-controls.nix {
        inherit
          pkgs
          src
          rust
          nickel
          octetGate
          ;
        cargoOctet = octet.packages.${system}.cargo-octet;
        selection = selectionPolicy;
      };
      sourceInventoryTests = import ./nix/source-inventory-tests.nix { };
      octetControlTests = import ./nix/octet-controls-tests.nix { };
      octetControls = import ./nix/octet-controls.nix {
        flakeNix = builtins.readFile ./flake.nix;
        flakeLock = builtins.readFile ./flake.lock;
        preCommit = builtins.readFile ./.pre-commit-config.yaml;
        selectionJson = builtins.fromJSON (builtins.readFile ./policy/tool-selection.json);
        cargoToml = builtins.readFile ./Cargo.toml;
        architecturePolicyJson = builtins.readFile ./policy/architecture.json;
      };
      # Compiler-derived inventory artifacts. Built in the sandbox by the published collector.
      inventoryArtifacts = pkgs.runCommand "noble-inventory-artifacts" {
        nativeBuildInputs = [
          rust
          pkgs.stdenv.cc
          pkgs.coreutils
          octet.packages.${system}.cargo-octet
        ];
      } ''
        export HOME="$TMPDIR/home"
        export CARGO_HOME="$HOME/cargo"
        export CARGO_TARGET_DIR="$TMPDIR/target"
        mkdir -p "$CARGO_HOME" "$out"
        cp -r ${src} source
        chmod -R u+w source
        cd source
        exec cargo-octet check --workspace --output-format json --artifact-dir "$out" \
          -- --all-targets --all-features
      '';
      inventory = import ./nix/source-inventory-derive.nix {
        ir = builtins.fromJSON (builtins.readFile "${inventoryArtifacts}/compiler-architecture-ir.json");
        coverage = builtins.fromJSON (
          builtins.readFile "${inventoryArtifacts}/compiler-architecture-coverage.json"
        );
        cargoGraph = builtins.fromJSON (
          builtins.readFile "${inventoryArtifacts}/project-architecture-cargo-graph.json"
        );
        selection = selectionPolicy;
      };
      inventoryResult = import ./nix/source-inventory.nix {
        inherit inventory;
        policy = builtins.fromJSON (builtins.readFile ./policy/source-inventory.json);
        architecturePolicy = builtins.fromJSON (builtins.readFile ./policy/architecture.json);
        selection = selectionPolicy;
      };
      nativeAssuranceTests = import ./nix/native-assurance-tests.nix { };
      nativeInventory = import ./nix/native-assurance-derive.nix {
        ir = builtins.fromJSON (builtins.readFile "${inventoryArtifacts}/compiler-architecture-ir.json");
        cargoGraph = builtins.fromJSON (
          builtins.readFile "${inventoryArtifacts}/project-architecture-cargo-graph.json"
        );
        workspaceToml = builtins.readFile ./Cargo.toml;
        fixtureLock = builtins.readFile ./verification/fixtures/randomness.Cargo.lock;
        selection = selectionPolicy;
      };
      nativeResult = import ./nix/native-assurance.nix {
        inventory = nativeInventory;
        policy = builtins.fromJSON (builtins.readFile ./policy/native-assurance.json);
      };
      # Use the published hook, not a copied lint list or a warning-only helper.
      sourceInventory = import ./nix/source-inventory-app.nix {
        inherit pkgs;
        cargoOctet = octet.packages.${system}.cargo-octet;
      };
      extractKernel = import ./nix/extract-kernel-app.nix {
        inherit pkgs;
        aeneas = selectedAeneas;
        leanTool = lean;
        extractionRust = extractionRust;
        leanDependenciesCheck = leanDependenciesCheck;
        selection = selectionPolicy;
      };
      nativeAssurance = import ./nix/native-assurance-app.nix {
        inherit pkgs;
        cargoOctet = octet.packages.${system}.cargo-octet;
      };
      octetGate = pkgs.writeShellApplication {
        name = "noble-octet-gate";
        runtimeInputs = [
          octet.packages.${system}.cargo-octet
          rust
        ];
        text = ''
          unset DYLINT_RUSTFLAGS RUSTC_WRAPPER RUSTC_WORKSPACE_WRAPPER
          unset RUSTFLAGS CARGO_ENCODED_RUSTFLAGS
          export OCTET_PRECOMMIT_USE_INSTALLED=true
          exec ${pkgs.runtimeShell} ${octet}/hooks/octet-deny-all.sh "$@"
        '';
      };
      mkCheck =
        name: inputs: command:
        pkgs.runCommand name
          {
            nativeBuildInputs = [
              rust
              pkgs.stdenv.cc
              pkgs.coreutils
            ]
            ++ inputs;
          }
          ''
            export HOME="$TMPDIR/home"
            export CARGO_HOME="$HOME/cargo"
            export CARGO_TARGET_DIR="$TMPDIR/target"
            mkdir -p "$CARGO_HOME" "$out"
            cp -r ${src} source
            chmod -R u+w source
            cd source
            ${command}
          '';
    in
    assert selection.enforce;
    {
      packages.${system} = {
        aeneas = selectedAeneas;
        charon = aeneas.packages.${system}.charon;
        extraction-rust = extractionRust;
        miri = extractionRust;
        toolchain-check = toolchainCheck;
        lean-dependencies-check = leanDependenciesCheck;
        inherit rust nickel lean noble;
        node = wasmVerificationTools.node;
        wasm-tools = wasmVerificationTools.wasm_tools;
        binaryen = wasmVerificationTools.binaryen;
        octet = octet.packages.${system}.cargo-octet;
        octet-standards = standards;
        octet-gate = octetGate;
        source-inventory = sourceInventory;
        extract-kernel = extractKernel;
        native-assurance = nativeAssurance;
        m3-wasm = m3Wasm;
        elan = pkgs.elan;
      };

      apps.${system} = {
        m3-wasm = {
          type = "app";
          program = "${m3Wasm}/bin/m3-wasm";
          meta.description = "Run both bounded Wasm representations with pinned tools and the built Noble CLI; not refinement";
        };
        lean-dependencies-check = {
          type = "app";
          program = "${leanDependenciesCheck}/bin/noble-lean-dependencies-check";
          meta.description = "Reject changed or symlinked Lake Git sources; not cache authentication";
        };
        toolchain-check = {
          type = "app";
          program = "${toolchainCheck}/bin/noble-toolchain-check";
          meta.description = "Check reviewed tool pins and reject tool overrides; not compatibility acceptance";
        };
        octet-gate = {
          type = "app";
          program = "${octetGate}/bin/noble-octet-gate";
          meta.description = "Run the pinned Octet deny-all and architecture gate";
        };
        source-inventory = {
          type = "app";
          program = "${sourceInventory}/bin/source-inventory";
          meta.description = "Collect the compiler-derived inventory and compare reviewed classification; not M1 acceptance";
        };
        extract-kernel = {
          type = "app";
          program = "${extractKernel}/bin/extract-kernel";
          meta.description = "Run the bounded Charon/Aeneas/Lean entry point for the actual kernel subject; not refinement";
        };
        native-assurance = {
          type = "app";
          program = "${nativeAssurance}/bin/native-assurance";
          meta.description = "Derive and check the native and dependency assurance inventory; not soundness";
        };
      };

      devShells.${system}.default = pkgs.mkShell {
        packages = [
          rust
          nickel
          standards
          octetGate
          octet.packages.${system}.cargo-octet
          pkgs.bun
          pkgs.git
          pkgs.pre-commit
          pkgs.b3sum
        ];
        shellHook = ''
          unset DYLINT_RUSTFLAGS RUSTC_WRAPPER RUSTC_WORKSPACE_WRAPPER
          unset RUSTFLAGS CARGO_ENCODED_RUSTFLAGS
          export OCTET_PRECOMMIT_USE_INSTALLED=true
        '';
      };

      # These are incremental gates, not the complete M1 acceptance matrix.
      checks.${system} = {
        boundary-controls = boundaryControls.check;
        tool-selection = mkCheck "noble-tool-selection" [ nickel pkgs.diffutils ] ''
          nickel export --format json policy/tool-selection.ncl > "$TMPDIR/tool-selection.json"
          cmp policy/tool-selection.json "$TMPDIR/tool-selection.json"
          nickel typecheck standards/policies/registry.ncl
          printf '%s\n' ${pkgs.lib.escapeShellArg (builtins.concatStringsSep "\n" selectionTests)} > "$out/controls.txt"
        '';
        tool-versions = mkCheck "noble-tool-versions" [ toolchainCheck ] ''
          noble-toolchain-check > "$out/versions.log"
        '';
        tool-overrides = mkCheck "noble-tool-overrides" [ toolchainCheck pkgs.gnugrep ] ''
          for name in CHARON_EXE AENEAS_EXE LEAN_PATH LEAN_SRC_PATH LEAN_SYSROOT \
            LAKE_HOME RUSTC RUSTC_WRAPPER RUSTC_WORKSPACE_WRAPPER RUSTFLAGS \
            CARGO_ENCODED_RUSTFLAGS RUSTUP_TOOLCHAIN CHARON_IS_SYMLINK \
            MIRI MIRI_SYSROOT MIRIFLAGS NOBLE_M3_CLI NOBLE_M3_NODE \
            NOBLE_M3_WASM_TOOLS NOBLE_M3_WASM_OPT NODE_OPTIONS; do
            set +e
            env "$name=/not-a-reviewed-tool" noble-toolchain-check > "$out/$name.log" 2>&1
            status=$?
            set -e
            test "$status" -eq 2
            grep -Fx "tool-selection: forbidden tool override: $name" "$out/$name.log"
          done
          set +e
          noble-toolchain-check --allow-pin-mismatch > "$out/manual-override.log" 2>&1
          status=$?
          set -e
          test "$status" -eq 2
          grep -Fx "tool-selection: no arguments or manual overrides are supported" "$out/manual-override.log"
          mkdir backend-fixture
          ln -s "$PWD" backend-fixture/.lake
          status=0
          ${leanDependenciesCheck}/bin/noble-lean-dependencies-check "$PWD/backend-fixture" > "$out/backend-symlink.log" 2>&1 || status=$?
          test "$status" -eq 2
          grep -Fx 'lean-dependencies: source directory symlink rejected' "$out/backend-symlink.log"
          echo '21 environment overrides, one manual override argument, and one backend directory symlink rejected' > "$out/controls.txt"
        '';
        format = mkCheck "noble-format" [ ] ''
          cargo fmt --all -- --check
        '';
        rust-tests = mkCheck "noble-rust-tests" (builtins.attrValues wasmVerificationTools) ''
          cargo test --workspace --all-targets --all-features --locked --offline
        '';
        clippy = mkCheck "noble-clippy" [ ] ''
          cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings
        '';
        documents = mkCheck "noble-documents" [ pkgs.bun ] ''
          bun tools/cairn-specs.mjs --self-test
          bun test tools/cairn-specs.test.mjs tools/check-specs.test.mjs
          bun tools/check-specs.mjs --self-test
        '';
        cairn = mkCheck "noble-cairn" [ cairn.packages.${system}.cairn ] ''
          cairn validate --root "$PWD" --policy ${cairn}/cairn-policy/generated/cairn-policy.json
        '';
        policy =
          assert
            octetControls.valid
            || throw (
              "octet pin/scope/lint controls failed: "
              + builtins.concatStringsSep ", " octetControls.diagnostics
            );
          mkCheck "noble-policy-freshness" [ nickel standards pkgs.diffutils ] ''
          nickel export --format json policy/architecture.ncl > "$TMPDIR/architecture.json"
          cmp policy/architecture.json "$TMPDIR/architecture.json"
          nickel export --format json policy/source-inventory.ncl > "$TMPDIR/source-inventory.json"
          cmp policy/source-inventory.json "$TMPDIR/source-inventory.json"
          nickel export --format json policy/native-assurance.ncl > "$TMPDIR/native-assurance.json"
          cmp policy/native-assurance.json "$TMPDIR/native-assurance.json"
          nickel export --format json policy/architecture-export-spec.ncl > "$TMPDIR/export-spec.json"
          octet-standards nickel-export-manifest --spec "$TMPDIR/export-spec.json" \
            --output policy/architecture.json --manifest-out "$TMPDIR/architecture.manifest.json" \
              --nickel-identity "$(nickel --version)"
          cmp policy/architecture.manifest.json "$TMPDIR/architecture.manifest.json"
          printf '%s\n' '${builtins.toJSON octetControls.diagnostics}' > "$out/octet-control-diagnostics.json"
          printf '%s\n' '${builtins.toJSON octetControls.lint_lints}' > "$out/octet-lint-table.json"
          printf '%s\n' ${pkgs.lib.escapeShellArg (builtins.concatStringsSep "\n" octetControlTests)} > "$out/octet-control-tests.txt"
        '';
        octet = mkCheck "noble-octet-deny-all" [ octetGate ] ''
          noble-octet-gate --workspace --artifact-dir "$out" -- --all-targets --all-features
        '';
        source-coverage =
          assert
            inventoryResult.valid
            || throw (
              "source coverage rejected: " + builtins.concatStringsSep ", " inventoryResult.diagnostics
            );
          pkgs.runCommand "noble-source-coverage" { } ''
            mkdir -p "$out"
            cp ${inventoryArtifacts}/compiler-architecture-ir.json "$out/"
            cp ${inventoryArtifacts}/compiler-architecture-coverage.json "$out/"
            printf '%s\n' ${pkgs.lib.escapeShellArg (builtins.toJSON inventoryResult)} > "$out/coverage.json"
            printf '%s\n' ${pkgs.lib.escapeShellArg (builtins.concatStringsSep "\n" sourceInventoryTests)} > "$out/self-tests.txt"
            printf '%s\n' 'source inventory: compiler-derived subjects matched the reviewed classification; not M1 acceptance' > "$out/validated.txt"
          '';
        native-assurance =
          assert
            nativeResult.valid
            || throw (
              "native assurance rejected: " + builtins.concatStringsSep ", " nativeResult.diagnostics
            );
          pkgs.runCommand "noble-native-assurance" { } ''
            mkdir -p "$out"
            printf '%s\n' ${pkgs.lib.escapeShellArg (builtins.toJSON nativeInventory)} > "$out/inventory.json"
            printf '%s\n' ${pkgs.lib.escapeShellArg (builtins.toJSON nativeResult)} > "$out/coverage.json"
            printf '%s\n' ${pkgs.lib.escapeShellArg (builtins.concatStringsSep "\n" nativeAssuranceTests)} > "$out/self-tests.txt"
            printf '%s\n' 'native assurance: scope and dependency records matched; not soundness and not M1 acceptance' > "$out/validated.txt"
          '';
      };
    };
}
