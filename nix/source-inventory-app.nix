# Thin execution and file adapter for the pure inventory derivation and comparator.
# Reserved interface: `source-inventory collect` and `source-inventory check`.
{
  pkgs,
  cargoOctet,
}:
let
  derive = ./source-inventory-derive.nix;
  comparator = ./source-inventory.nix;
in
pkgs.writeShellApplication {
  name = "source-inventory";
  runtimeInputs = [
    pkgs.coreutils
    cargoOctet
  ];
  text = ''
    set -euo pipefail

    derive='${derive}'
    comparator='${comparator}'

    # The caller supplies the Nix CLI, as the runbook invocation already does.
    command -v nix-instantiate >/dev/null || {
      echo "source-inventory: nix-instantiate is required on PATH" >&2
      exit 2
    }

    read_file() {
      # Emit a Nix expression that reads one JSON file.
      printf 'builtins.fromJSON (builtins.readFile %s)' "$(printf '%q' "$1")"
    }

    command -v cargo-octet >/dev/null || {
      echo "source-inventory: cargo-octet is not available" >&2
      exit 2
    }

    usage() {
      echo "usage: source-inventory collect --root DIR --selection FILE --artifact-dir DIR" >&2
      echo "       source-inventory check --root DIR --selection FILE --inventory FILE --policy FILE --artifact-dir DIR" >&2
      exit 2
    }

    main() {
      [ "$#" -ge 1 ] || usage
      local command="$1"
      shift
      local root="" selection="" inventory="" policy="" artifacts=""
      while [ "$#" -gt 0 ]; do
        case "$1" in
          --root) root="$2"; shift 2 ;;
          --selection) selection="$2"; shift 2 ;;
          --inventory) inventory="$2"; shift 2 ;;
          --policy) policy="$2"; shift 2 ;;
          --artifact-dir) artifacts="$2"; shift 2 ;;
          *) echo "source-inventory: unknown argument $1" >&2; usage ;;
        esac
      done
      [ -n "$root" ] || usage
      [ -n "$selection" ] || usage
      [ -n "$artifacts" ] || usage
      [ -f "$root/policy/architecture.json" ] || {
        echo "source-inventory: architecture policy export is missing" >&2
        exit 2
      }
      mkdir -p "$artifacts"

      case "$command" in
        collect)
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
              coverage = '"$(read_file "$artifacts/collector/compiler-architecture-coverage.json")"';
              cargoGraph = '"$(read_file "$artifacts/collector/project-architecture-cargo-graph.json")"';
              selection = '"$(read_file "$selection")"';
            }' > "$artifacts/inventory.json"
          printf '%s\n' "source-inventory: collected $(grep -o '"production_subjects":[0-9]*' "$artifacts/inventory.json" | head -1)"
          ;;
        check)
          [ -n "$inventory" ] || usage
          [ -n "$policy" ] || usage
          [ -f "$inventory" ] || { echo "source-inventory: inventory is missing: $inventory" >&2; exit 2; }
          [ -f "$policy" ] || { echo "source-inventory: reviewed policy is missing: $policy" >&2; exit 2; }
          NIX_REMOTE=dummy:// nix-instantiate --eval --strict --json --expr '
            import '"$comparator"' {
              inventory = '"$(read_file "$inventory")"';
              policy = '"$(read_file "$policy")"';
              architecturePolicy = '"$(read_file "$root/policy/architecture.json")"';
              selection = '"$(read_file "$selection")"';
            }' > "$artifacts/coverage.json"
          if grep -q '"valid":true' "$artifacts/coverage.json"; then
            echo "source-inventory: coverage passed (not M1 acceptance)"
            exit 0
          fi
          echo "source-inventory: coverage rejected" >&2
          grep -o '"diagnostics":\[[^]]*\]' "$artifacts/coverage.json" >&2 || true
          exit 1
          ;;
        *) usage ;;
      esac
    }

    main "$@"
  '';
}
