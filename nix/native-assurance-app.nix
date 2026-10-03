# Thin execution and file adapter for the pure native-assurance derivation and comparator.
# Reserved interface: `native-assurance inventory` and `native-assurance check`.
{
  pkgs,
  cargoOctet,
  rust,
  wasmtimeSource,
  reviewedVendor,
  reviewedCargoConfig,
  selectionFile,
}:
let
  derive = pkgs.writeText "noble-native-assurance-derive.nix" (builtins.readFile ./native-assurance-derive.nix);
  comparator = pkgs.writeText "noble-native-assurance-comparator.nix" (builtins.readFile ./native-assurance.nix);
  packagedSelection = pkgs.writeText "noble-selected-tool-selection.json" (builtins.readFile selectionFile);
  selection = builtins.fromJSON (builtins.readFile selectionFile);
  runtimeTools = [
    pkgs.bash
    pkgs.coreutils
    pkgs.diffutils
    pkgs.findutils
    pkgs.gnugrep
    pkgs.gnumake
    pkgs.gnused
    pkgs.nix
    pkgs.stdenv.cc
    rust
    cargoOctet
  ];
in
assert selection.component_sync.wasmtime_source == toString wasmtimeSource;
assert selection.component_sync.vendor == toString reviewedVendor;
assert selection.tool_paths.quality_rust.output == toString rust;
assert selection.tool_paths.octet.output == toString cargoOctet;
pkgs.writeShellApplication {
  name = "native-assurance";
  runtimeInputs = runtimeTools ++ [ wasmtimeSource reviewedVendor ];
  text = ''
    set -euo pipefail

    export PATH=${pkgs.lib.escapeShellArg (pkgs.lib.makeBinPath runtimeTools)}
    derive='${derive}'
    comparator='${comparator}'

    command -v nix-instantiate >/dev/null || {
      echo "native-assurance: nix-instantiate is required on PATH" >&2
      exit 2
    }

    read_file() {
      printf 'builtins.fromJSON (builtins.readFile %s)' "$(printf '%q' "$1")"
    }

    usage() {
      echo "usage: native-assurance inventory --root DIR --selection FILE --artifact-dir DIR" >&2
      echo "       native-assurance check --root DIR --selection FILE --inventory FILE --policy FILE --artifact-dir DIR" >&2
      exit 2
    }

    reviewed_cargo_ancestors() {
      local ancestor="$1" cargo_config
      while :; do
        for cargo_config in "$ancestor/.cargo/config" "$ancestor/.cargo/config.toml"; do
          if [ -e "$cargo_config" ] || [ -L "$cargo_config" ]; then
            if [ "$cargo_config" != "/home/brittonr/.cargo/config.toml" ] ||
              [ ! -f "$cargo_config" ] ||
              [ "$(realpath -- "$cargo_config")" != "/nix/store/7y0cmn79ilsaq0g7lgpmpa7sqjrkii16-cargo-config.toml" ] ||
              [ "$(sha256sum "$cargo_config" | cut -d' ' -f1)" != "fd49ee6f0a53eb27d583fc54fdbf3c179e116e02942b7de4d52990896f961377" ]; then
              echo "native-assurance: workspace or ancestor Cargo config is not reviewed: $cargo_config" >&2
              return 2
            fi
          fi
        done
        [ "$ancestor" = / ] && break
        ancestor=$(dirname -- "$ancestor")
      done
    }

    main() {
      [ "$#" -ge 1 ] || usage
      local command="$1"
      shift
      local root="" selection_file="" inventory="" policy="" artifacts=""
      while [ "$#" -gt 0 ]; do
        case "$1" in
          --root) root="$2"; shift 2 ;;
          --selection) selection_file="$2"; shift 2 ;;
          --inventory) inventory="$2"; shift 2 ;;
          --policy) policy="$2"; shift 2 ;;
          --artifact-dir) artifacts="$2"; shift 2 ;;
          *) echo "native-assurance: unknown argument $1" >&2; usage ;;
        esac
      done
      [ -n "$root" ] || usage
      [ -n "$selection_file" ] || usage
      [ -n "$artifacts" ] || usage
      cmp -s "$selection_file" ${pkgs.lib.escapeShellArg (toString packagedSelection)} || {
        echo "native-assurance: selection differs from selected policy" >&2
        exit 2
      }
      root=$(realpath -- "$root")
      reviewed_cargo_ancestors "$root"
      [ -f "$root/crates/noble-cli/Cargo.toml" ] || {
        echo "native-assurance: root Wasmtime manifest is missing" >&2
        exit 2
      }
      local manifest_wasmtime
      manifest_wasmtime=$(NIX_REMOTE=dummy:// nix-instantiate --eval --strict --raw \
        --argstr manifest "$root/crates/noble-cli/Cargo.toml" \
        --expr '{ manifest }: (builtins.fromTOML (builtins.readFile manifest)).dependencies.wasmtime.path or ""')
      [ "$manifest_wasmtime" = ${pkgs.lib.escapeShellArg "${wasmtimeSource}/crates/wasmtime"} ] || {
        echo "native-assurance: root Wasmtime manifest does not use the selected absolute source" >&2
        exit 2
      }
      [ -d "${wasmtimeSource}/crates/wasmtime" ] && [ -d "${reviewedVendor}/source-registry-0" ] || {
        echo "native-assurance: selected Wasmtime source or reviewed vendor source is missing" >&2
        exit 2
      }
      [ "$(NIX_REMOTE=dummy:// nix hash path --sri --type sha256 ${pkgs.lib.escapeShellArg (toString wasmtimeSource)})" = ${pkgs.lib.escapeShellArg selection.component_sync.wasmtime_source_nar_hash} ] &&
        [ "$(NIX_REMOTE=dummy:// nix hash path --sri --type sha256 ${pkgs.lib.escapeShellArg (toString reviewedVendor)})" = ${pkgs.lib.escapeShellArg selection.component_sync.vendor_nar_hash} ] || {
        echo "native-assurance: selected Wasmtime source or reviewed vendor NAR differs from policy" >&2
        exit 2
      }
      mkdir -p "$artifacts"
      artifacts=$(realpath -- "$artifacts")

      local fixture_lock="null"
      if [ -f "$root/verification/fixtures/randomness.Cargo.lock" ]; then
        fixture_lock="builtins.readFile $root/verification/fixtures/randomness.Cargo.lock"
      fi

      case "$command" in
        inventory)
          [ ! -e "$artifacts/home" ] && [ ! -e "$artifacts/cargo-home" ] &&
            [ ! -e "$artifacts/target" ] && [ ! -e "$artifacts/collector" ] &&
            [ ! -L "$artifacts/home" ] && [ ! -L "$artifacts/cargo-home" ] &&
            [ ! -L "$artifacts/target" ] && [ ! -L "$artifacts/collector" ] || {
            echo "native-assurance: inventory requires fresh artifact-owned Cargo paths" >&2
            exit 2
          }
          export HOME="$artifacts/home"
          export CARGO_HOME="$artifacts/cargo-home"
          export CARGO_TARGET_DIR="$artifacts/target"
          mkdir -- "$HOME" "$CARGO_HOME" "$artifacts/collector"
          install -m 0644 ${pkgs.lib.escapeShellArg (toString reviewedCargoConfig)} "$CARGO_HOME/config.toml"
          local collector_status=0
          (
            # Cargo reads ancestor configuration. Only the separately reviewed
            # home config digest above may accompany the original source tree.
            cd "$root"
            env -i PATH="$PATH" HOME="$HOME" CARGO_HOME="$CARGO_HOME" \
              CARGO_TARGET_DIR="$CARGO_TARGET_DIR" CARGO_BUILD_TARGET_DIR="$CARGO_TARGET_DIR" \
              CARGO_BUILD_RUSTC_WRAPPER= CARGO_BUILD_JOBS=2 CARGO_NET_OFFLINE=true TMPDIR="$HOME" \
              cargo-octet check --workspace --output-format json \
              --artifact-dir "$artifacts/collector" -- --all-targets --all-features --offline
          ) > "$artifacts/collector.stdout" 2> "$artifacts/collector.stderr" || collector_status=$?
          reviewed_cargo_ancestors "$root"
          [ "$collector_status" -eq 0 ] || return "$collector_status"
          NIX_REMOTE=dummy:// nix-instantiate --eval --strict --json --expr '
            import '"$derive"' {
              ir = '"$(read_file "$artifacts/collector/compiler-architecture-ir.json")"';
              cargoGraph = '"$(read_file "$artifacts/collector/project-architecture-cargo-graph.json")"';
              workspaceToml = builtins.readFile '"$root/Cargo.toml"';
              fixtureLock = '"$fixture_lock"';
              selection = '"$(read_file "$selection_file")"';
            }' > "$artifacts/inventory.json"
          printf '%s\n' "native-assurance: inventory written"
          ;;
        check)
          [ -n "$inventory" ] || usage
          [ -n "$policy" ] || usage
          [ -f "$inventory" ] || { echo "native-assurance: inventory is missing" >&2; exit 2; }
          [ -f "$policy" ] || { echo "native-assurance: reviewed policy is missing" >&2; exit 2; }
          NIX_REMOTE=dummy:// nix-instantiate --eval --strict --json --expr '
            import '"$comparator"' {
              inventory = '"$(read_file "$inventory")"';
              policy = '"$(read_file "$policy")"';
            }' > "$artifacts/coverage.json"
          if grep -q '"valid":true' "$artifacts/coverage.json"; then
            echo "native-assurance: check passed (not M1 acceptance, not soundness)"
            exit 0
          fi
          echo "native-assurance: check rejected" >&2
          grep -o '"diagnostics":\[[^]]*\]' "$artifacts/coverage.json" >&2 || true
          exit 1
          ;;
        *) usage ;;
      esac
    }

    main "$@"
  '';
}
