{
  description = "Pinned patched Owi package for the Noble #57 evaluation lane";

  inputs.nixpkgs.url = "https://releases.nixos.org/nixos/unstable/nixos-26.11pre1046609.643809054d65/nixexprs.tar.xz";

  outputs = { self, nixpkgs }:
    let
      system = "x86_64-linux";
      pkgs = import nixpkgs { inherit system; };
    in {
      packages.${system}.default = pkgs.owi.overrideAttrs (old: {
        patches = (old.patches or [ ]) ++ [
          ../../../nix/owi-memory-init-decode.patch
          ../../../nix/owi-register-modules.patch
        ];
      });
    };
}
