# Thin execution and file adapter for the pure native-assurance derivation and comparator.
# Reserved interface: `native-assurance inventory` and `native-assurance check`.
{
  pkgs,
  cargoOctet,
}:
let
  derive = ./native-assurance-derive.nix;
  comparator = ./native-assurance.nix;
in
pkgs.writeShellApplication {
  name = "native-assurance";
  runtimeInputs = [
    pkgs.coreutils
    pkgs.gnugrep
    cargoOctet
  ];
  text = ''
    set -euo pipefail

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
      mkdir -p "$artifacts"

      local fixture_lock="null"
      if [ -f "$root/verification/fixtures/randomness.Cargo.lock" ]; then
        fixture_lock="builtins.readFile $root/verification/fixtures/randomness.Cargo.lock"
      fi

      case "$command" in
        inventory)
          mkdir -p "$artifacts/collector"
          (
            cd "$root"
            CARGO_TARGET_DIR="$artifacts/target" cargo-octet check --workspace --output-format json \
              --artifact-dir "$artifacts/collector" -- --all-targets --all-features \
              > "$artifacts/collector.stdout" 2> "$artifacts/collector.stderr"
          )
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
