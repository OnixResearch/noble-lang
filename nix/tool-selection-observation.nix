# Imperative build boundary: read selected sources and repository configuration.
{
  inputs,
  root,
  system,
}:
let
  inherit (builtins)
    fromJSON
    fromTOML
    readFile
    hashFile
    listToAttrs
    ;
  charon = inputs.aeneas.inputs.charon;
  pin = input: {
    rev = input.sourceInfo.rev or null;
    narHash = input.sourceInfo.narHash or null;
  };
  rust = fromTOML (readFile (root + "/rust-toolchain.toml"));
  cargo = fromTOML (readFile (root + "/Cargo.toml"));
  manifests = map (
    member: fromTOML (readFile (root + "/${member}/Cargo.toml"))
  ) cargo.workspace.members;
  fileNames = import ./tool-selection-files.nix;
in
{
  sources = builtins.mapAttrs (_: pin) {
    inherit (inputs)
      aeneas
      cairn
      octet
      nixpkgs
      ;
    inherit charon;
    rust_overlay = inputs.rust-overlay;
    aeneas_nixpkgs = inputs.aeneas.inputs.nixpkgs;
    extraction_nixpkgs = charon.inputs.nixpkgs;
  };
  file_sha256 = listToAttrs (
    map (name: {
      inherit name;
      value = hashFile "sha256" (root + "/${name}");
    }) fileNames
  );
  component_sync = fromJSON (readFile (root + "/verification/m5/pins.json"));
  inherit system;
  quality_rust = rust.toolchain.channel;
  extraction_rust = (fromTOML (readFile (charon + "/rust-toolchain"))).toolchain.channel;
  charon_pin_text = readFile (inputs.aeneas + "/charon-pin");
  lean_toolchain_text = readFile (root + "/proofs/m1/lean-toolchain");
  upstream_lean_toolchain_text = readFile (inputs.aeneas + "/backends/lean/lean-toolchain");
  lake_config = fromTOML (readFile (root + "/proofs/m1/lakefile.toml"));
  lake_manifest = fromJSON (readFile (root + "/proofs/m1/lake-manifest.json"));
  upstream_lake_manifest = fromJSON (readFile (inputs.aeneas + "/backends/lean/lake-manifest.json"));
  profiles = cargo.profile;
  features = builtins.attrNames (
    builtins.foldl' (features: manifest: features // (manifest.features or { })) { } manifests
  );
}
