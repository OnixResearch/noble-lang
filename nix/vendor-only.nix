# Evaluate only the reviewed Cargo vendor from a separately authenticated
# Nixpkgs source. The caller must bind the exact selected source NAR and path;
# this entrypoint does not discover a flake or choose an ambient Nixpkgs.
{ nixpkgs }:
let
  source = toString nixpkgs;
  pkgs = import nixpkgs { system = "x86_64-linux"; };
in
assert builtins.match "/nix/store/[0-9a-z]{32}-source" source != null;
import ./reviewed-vendor.nix { inherit pkgs; root = ../.; }
