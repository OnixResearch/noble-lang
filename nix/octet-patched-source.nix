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
  typedCallPatch = builtins.path {
    path = ./octet-typed-call-collector.patch;
    name = "octet-typed-call-collector.patch";
    sha256 = "sha256-Xnv4404uFoMUX4XfONrdGwxBZP37bRoVc7tcZLp95Tk=";
  };
  traitBodyPatch = builtins.path {
    path = ./octet-trait-body-resolver.patch;
    name = "octet-trait-body-resolver.patch";
    sha256 = "sha256-O425gW3T8/7ABG0LVZSyzaSYHdBYAUxVPcezVp+eWi4=";
  };
  dynamicTraitUfcsPatch = builtins.path {
    path = ./octet-dynamic-trait-ufcs.patch;
    name = "octet-dynamic-trait-ufcs.patch";
    sha256 = "sha256-1uR6T1SHOQFtF27XZXYGSjmbTkGcwsuqR3vefqYposY=";
  };
  preFixtureSource = pkgs.applyPatches {
    name = "octet-pinned-235255bc-diagnostic-path-source";
    src = selected.octet.src;
    patches = [ enginePatch libcPatch metadataLockedPatch resolvedPlanPatch ];
  };
  fixtureLockedSource = pkgs.runCommand "octet-pinned-235255bc-locked-fixtures-source" {
    nativeBuildInputs = [ pkgs.git ];
  } ''
    mkdir -p "$out"
    cp -a ${preFixtureSource}/. "$out/"
    chmod u+w "$out/cargo-octet/src" "$out/cargo-octet/src/engine.rs"
    cd "$out"
    git apply --check ${fixtureLockPatch}
    git apply ${fixtureLockPatch}
  '';
  patchedSource = pkgs.applyPatches {
    name = "octet-pinned-235255bc-typed-strict-trait-source";
    src = fixtureLockedSource;
    patches = [ typedCallPatch traitBodyPatch dynamicTraitUfcsPatch ];
  };
  rustToolchain = pkgs.rust-bin.fromRustupToolchainFile "${octet.outPath}/rust-toolchain.toml";
  craneLib = (octet.inputs.crane.mkLib pkgs).overrideToolchain rustToolchain;
  hostTriple = pkgs.stdenv.hostPlatform.rust.rustcTarget;
  toolchainChannel = (builtins.fromTOML (builtins.readFile "${octet.outPath}/rust-toolchain.toml")).toolchain.channel;
  rustupToolchain = if pkgs.lib.hasSuffix hostTriple toolchainChannel then toolchainChannel else "${toolchainChannel}-${hostTriple}";
  sharedLibraryPrefix = if pkgs.stdenv.hostPlatform.isWindows then "" else "lib";
  sharedLibrarySuffix = if pkgs.stdenv.hostPlatform.isDarwin then ".dylib" else if pkgs.stdenv.hostPlatform.isWindows then ".dll" else ".so";
  octetLibraryName = "${sharedLibraryPrefix}octet${sharedLibrarySuffix}";
  octetUiLibraryName = "${sharedLibraryPrefix}octet@${rustupToolchain}${sharedLibrarySuffix}";
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
  patchedOctet = selected.octet.overrideAttrs (_: { src = patchedSource; });
  octetUiLibrary = pkgs.runCommand "octet-ui-library-selected-source" { } ''
    mkdir -p "$out/lib"
    ln -s "${patchedOctet}/lib/${octetLibraryName}" "$out/lib/${octetUiLibraryName}"
  '';
  oldWrapper = "${selected.cargo-octet}/bin/cargo-octet";
  oldWrapperText = builtins.readFile oldWrapper;
  oldLintLibraryLines = builtins.filter
    (line: pkgs.lib.hasPrefix "export TIGERSTYLE_LINT_LIB='" line)
    (pkgs.lib.splitString "\n" oldWrapperText);
  oldLintLibraryLine = assert builtins.length oldLintLibraryLines == 1; builtins.head oldLintLibraryLines;
  oldLintLibrary = pkgs.lib.removeSuffix "'" (pkgs.lib.removePrefix "export TIGERSTYLE_LINT_LIB='" oldLintLibraryLine);
  newLintLibrary = "${octetUiLibrary}/lib/${octetUiLibraryName}";
  patchedWrapped = pkgs.runCommand "cargo-octet-0.1.0-typed-strict-trait" {
    nativeBuildInputs = [ pkgs.gnugrep ];
  } ''
    mkdir -p "$out/bin"
    for binary in cargo-octet cargo-slotcar cargo-tigerstyle; do
      test -x "${patchedUnwrapped}/bin/$binary"
      cp "${selected.cargo-octet}/bin/$binary" "$out/bin/$binary"
      chmod u+w "$out/bin/$binary"
      substituteInPlace "$out/bin/$binary" \
        --replace-fail ${pkgs.lib.escapeShellArg oldLintLibrary} ${pkgs.lib.escapeShellArg newLintLibrary} \
        --replace-fail "${selected.cargo-octet}/bin/.$binary-wrapped" "${patchedUnwrapped}/bin/$binary"
      if grep -F ${pkgs.lib.escapeShellArg oldLintLibrary} "$out/bin/$binary"; then exit 1; fi
      grep -F ${pkgs.lib.escapeShellArg newLintLibrary} "$out/bin/$binary" >/dev/null
      grep -F "${patchedUnwrapped}/bin/$binary" "$out/bin/$binary" >/dev/null
    done
  '';
in
{
  inherit patchedSource patchedUnwrapped patchedOctet patchedWrapped;
}
