# Evaluate only the reviewed Cargo vendor from a separately authenticated
# Nixpkgs source. The caller must bind the exact selected source NAR and
# absolute path, including when it lives in a private rootless store; this
# entrypoint does not discover a flake or choose an ambient Nixpkgs.
{ nixpkgs }:
let
  source = toString nixpkgs;
  pkgs = import nixpkgs { system = "x86_64-linux"; };
in
assert builtins.match "/.*" source != null;
assert builtins.pathExists (nixpkgs + "/default.nix");
import ./reviewed-vendor.nix { inherit pkgs; root = ../.; }
