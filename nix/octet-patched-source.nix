# Exact, content-hashed correction to the already selected Octet source.
# Keep the original Crane dependency artifacts, vendor, runtime closure, and
# wrapper; this does not change Noble's source or authorize policy findings.
{
  octet,
  pkgs,
  system,
}:
let
  selected = octet.packages.${system};
  enginePatch = builtins.path {
    path = ./octet-path-engine-only.patch;
    name = "octet-path-engine-only.patch";
    sha256 = "sha256-np7GzQRCU6MZILedHAOD0QDHiNBoueSxzOw5T4TcZmQ=";
  };
  libcPatch = builtins.path {
    path = ./octet-path-libc-only.patch;
    name = "octet-path-libc-only.patch";
    sha256 = "sha256-TKxtYWG7AkVqcVCZF5exE7/em9sanW//gkI+y2+Sxi4=";
  };
  metadataLockedPatch = builtins.path {
    path = ./octet-path-metadata-locked.patch;
    name = "octet-path-metadata-locked.patch";
    sha256 = "sha256-m+1Bv2CQ2VTSdFzSoJjSmz7GS4f+ypqFa0NKWT67qwo=";
  };
  resolvedPlanPatch = builtins.path {
    path = ./octet-path-resolved-plan-only.patch;
    name = "octet-path-resolved-plan-only.patch";
    sha256 = "sha256-nOkA4KIPJ48fcccQye40qOanfixNO0eyu0u+gir9xlo=";
  };
  fixtureLockPatch = builtins.path {
    path = ./octet-path-test-lock-fixtures.patch;
    name = "octet-path-test-lock-fixtures.patch";
    sha256 = "sha256-cjLuj0f0CiTAZmuMMVzd1cJgXShwzoju59APa7+Eyqs=";
  };
  preFixtureSource = pkgs.applyPatches {
    name = "octet-pinned-235255bc-diagnostic-path-source";
    src = selected.octet.src;
    patches = [ enginePatch libcPatch metadataLockedPatch resolvedPlanPatch ];
  };
  patchedSource = pkgs.runCommand "octet-pinned-235255bc-locked-fixtures-source" {
    nativeBuildInputs = [ pkgs.git ];
  } ''
    mkdir -p "$out"
    cp -a ${preFixtureSource}/. "$out/"
    chmod u+w "$out/cargo-octet/src" "$out/cargo-octet/src/engine.rs"
    cd "$out"
    git apply --check ${fixtureLockPatch}
    git apply ${fixtureLockPatch}
  '';
  rustToolchain = pkgs.rust-bin.fromRustupToolchainFile "${octet.outPath}/rust-toolchain.toml";
  craneLib = (octet.inputs.crane.mkLib pkgs).overrideToolchain rustToolchain;
  patchedUnwrapped = craneLib.buildPackage {
    src = patchedSource;
    cargoArtifacts = selected.octet.cargoArtifacts;
    cargoVendorDir = selected.octet.cargoVendorDir;
    nativeBuildInputs = with pkgs; [ clang git mold pkg-config ];
    buildInputs = [ pkgs.openssl ];
    pname = "cargo-octet";
    version = "0.1.0";
    cargoExtraArgs = "--locked -p cargo-octet";
    doCheck = false;
  };
  patchedWrapped = selected.cargo-octet.overrideAttrs (_: {
    paths = [ patchedUnwrapped ];
  });
in
{
  inherit patchedSource patchedUnwrapped patchedWrapped;
}
