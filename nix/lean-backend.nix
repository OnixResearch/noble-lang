# Offline proof root for the pinned Lean backend.
# Assembles .lake/packages from reviewed fixed-output sources so lake can run
# without network access. Binding only; it makes no extraction or proof claim.
{
  pkgs,
  sources,
}:
let
  inherit (builtins)
    attrNames
    concatStringsSep
    map
    ;
  names = attrNames sources;
  expectedNames = [
    "Cli"
    "LeanSearchClient"
    "Qq"
    "aeneas"
    "aesop"
    "batteries"
    "importGraph"
    "mathlib"
    "plausible"
    "proofwidgets"
  ];
  copyPackages = concatStringsSep "\n" (
    map (name: ''
      cp -r ${sources.${name}} "$out/.lake/packages/${name}"
      chmod -R u+w "$out/.lake/packages/${name}"
      rm -rf "$out/.lake/packages/${name}/.lake"
      echo "${name}" >> "$out/packages.txt"
    '') names
  );
in
assert builtins.sort builtins.lessThan names == builtins.sort builtins.lessThan expectedNames;
pkgs.runCommand "noble-lean-proof-root" { } ''
  mkdir -p "$out/.lake/packages"
  ${copyPackages}
  cp ${../proofs/m1/lakefile.toml} "$out/lakefile.toml"
  cp ${../proofs/m1/lake-manifest.json} "$out/lake-manifest.json"
  cp ${../proofs/m1/lean-toolchain} "$out/lean-toolchain"
  printf '%s\n' 'offline proof root: ten pinned backend sources, no network inputs' > "$out/README.txt"
''
