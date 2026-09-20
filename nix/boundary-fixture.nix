# Pure fixture transformation. The builder supplies source bytes and performs I/O.
{
  lib,
  files,
  case,
  policy,
}:
let
  replaceOnce =
    old: new: text:
    if builtins.length (lib.splitString old text) != 2 then
      throw "boundary fixture anchor missing or repeated"
    else
      builtins.replaceStrings [ old ] [ new ] text;
  kernel = policy.kernel_source;
  shell = "crates/noble-cli/src/main.rs";
  shellManifest = "crates/noble-cli/Cargo.toml";
  kernelManifest = "crates/noble-kernel/Cargo.toml";
  acyclic =
    if case.base == "acyclic_shell" then
      {
        # Isolate the reverse-edge control from every real CLI dependency.
        # Otherwise contracts or Wasm lowering retain an indirect kernel cycle.
        ${shellManifest} = ''
          [package]
          name = "noble-cli"
          version.workspace = true
          edition.workspace = true
          publish.workspace = true
          default-run = "noble"

          [[bin]]
          name = "noble"
          path = "src/main.rs"

          [lints]
          workspace = true
        '';
        ${shell} = ''
          //! Standalone acyclic shell fixture; compiled, never executed.
          fn main() {
              println!("acyclic-shell-fixture");
          }
        '';
        "crates/noble-cli/src/lib.rs" =
          "//! Empty shell library: acyclic dependency-control fixture only.\n";
      }
    else
      { };
  withStd =
    if case.needs_std then
      replaceOnce "/// Outcome of consuming" "extern crate std;\n\n/// Outcome of consuming"
        files.${kernel}
    else
      files.${kernel};
  dependency =
    if
      builtins.elem case.mutation [
        "randomness"
        "randomness_dependency"
      ]
    then
      {
        ${kernelManifest} = files.${kernelManifest} + ''

          [dependencies]
          ${policy.randomness_dependency.name} = "=${policy.randomness_dependency.version}"
        '';
      }
    else
      { };
  mutated =
    if
      builtins.elem case.mutation [
        "none"
        "randomness_dependency"
      ]
    then
      { }
    else if
      builtins.elem case.mutation [
        "body"
        "randomness"
      ]
    then
      {
        ${kernel} = replaceOnce policy.body_anchor (
          "    " + case.statement + "\n" + policy.body_anchor
        ) withStd;
      }
    else if case.mutation == "unsafe_function" then
      {
        ${kernel} = withStd + ''

          /// Isolated compiler-prohibition fixture. Never executed.
          pub unsafe fn boundary_unsafe(value: u32) -> u32 { value }
        '';
      }
    else if case.mutation == "unsafe_foreign_declaration" then
      {
        ${kernel} = withStd + ''

          unsafe extern "C" {
              /// Foreign-boundary fixture. Never executed.
              pub fn boundary_foreign_symbol();
          }
        '';
      }
    else if case.mutation == "no_std_test_witness" then
      {
        ${kernel} = replaceOnce "#![no_std]" "#![cfg_attr(not(test), no_std)]" files.${kernel};
      }
    else if case.mutation == "test_source_import" then
      {
        ${policy.test_source.path} = "pub fn imported_value() -> u32 { 7 }\n";
        ${kernel} =
          files.${kernel}
          + ''

            #[path = "${policy.test_source.relative}"]
            pub mod ${policy.test_source.module};
          '';
      }
    else if case.mutation == "host_return" then
      {
        ${kernel} = withStd + ''

          /// Isolated host-type fixture. Never executed.
          pub fn boundary_host_return(value: std::fs::File) -> std::fs::File { value }
        '';
      }
    else if case.mutation == "reverse_dependency" then
      {
        ${kernelManifest} = files.${kernelManifest} + ''

          [dependencies]
          noble-cli = { path = "../noble-cli" }
        '';
      }
    else
      throw "unknown boundary fixture mutation";
in
assert builtins.elem case.base [
  "workspace"
  "acyclic_shell"
];
assert
  !(builtins.elem case.mutation [
    "randomness"
    "randomness_dependency"
  ])
  || case.base == "workspace";
acyclic // dependency // mutated
