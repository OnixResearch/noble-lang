# Read-only check of a Lake working directory against the selected Git inputs.
{ pkgs, packages }:
pkgs.writeShellApplication {
  name = "noble-lean-dependencies-check";
  runtimeInputs = [
    pkgs.git
    pkgs.coreutils
  ];
  text = ''
    if [ "$#" -ne 1 ] || [ ! -d "$1" ]; then
      echo "lean-dependencies: expected one existing proof directory" >&2
      exit 2
    fi
    for path in "$1" "$1/.lake" "$1/.lake/packages"; do
      if [ -L "$path" ]; then
        echo "lean-dependencies: source directory symlink rejected" >&2
        exit 2
      fi
    done
    root=$(realpath -e -- "$1")
    ${pkgs.lib.concatMapStringsSep "\n" (package: ''
      path="$root/.lake/packages/"${pkgs.lib.escapeShellArg package.name}
      if [ -L "$path" ] || [ ! -d "$path" ]; then
        printf '%s\n' ${pkgs.lib.escapeShellArg "lean-dependencies: missing or symlink package: ${package.name}"} >&2
        exit 2
      fi
      top=$(git -C "$path" rev-parse --show-toplevel)
      if [ "$(realpath -e -- "$top")" != "$(realpath -e -- "$path")" ] \
        || [ "$(git -C "$path" rev-parse HEAD)" != ${pkgs.lib.escapeShellArg package.rev} ] \
        || [ -n "$(git -C "$path" status --porcelain --untracked-files=all)" ]; then
        printf '%s\n' ${pkgs.lib.escapeShellArg "lean-dependencies: changed Git source: ${package.name}"} >&2
        exit 2
      fi
      printf '%s\n' ${pkgs.lib.escapeShellArg "lean-dependencies: selected Git source: ${package.name} ${package.rev}"}
    '') packages}
    echo "lean-dependencies: source consistency only; compiled cache authenticity not established"
  '';
}
