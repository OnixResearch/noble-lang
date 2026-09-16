# Test the mutation mechanism separately from compiler observations.
{
  lib,
  files,
  policy,
}:
let
  cases = builtins.listToAttrs (
    map (c: {
      name = c.id;
      value = c;
    }) policy.cases
  );
  render =
    case: source:
    import ./boundary-fixture.nix {
      inherit lib case policy;
      files = source;
    };
  rejects =
    name: case: source:
    assert !(builtins.tryEval (builtins.deepSeq (render case source) true)).success;
    name;
  fs = cases.filesystem;
  kernel = policy.kernel_source;
  tests = [
    (
      assert
        builtins.attrNames (render cases.randomness-dependency files)
        == [ "crates/noble-kernel/Cargo.toml" ];
      "dependency-only control leaves the production body unchanged"
    )
    (
      assert
        (render cases.randomness-dependency files)."crates/noble-kernel/Cargo.toml"
        == (render cases.randomness files)."crates/noble-kernel/Cargo.toml";
      "dependency-only and randomness controls use identical dependency configuration"
    )
    (rejects "randomness rejects an unrelated dependency configuration" (
      cases.randomness // { base = "acyclic_shell"; }
    ) files)
    (
      assert render cases.workspace files == { };
      "workspace baseline has no source mutation"
    )
    (
      assert builtins.attrNames (render fs files) == [ kernel ];
      "effect mutation retains kernel path"
    )
    (
      assert
        builtins.attrNames (render cases.randomness files) == [
          "crates/noble-kernel/Cargo.toml"
          kernel
        ];
      "randomness changes only kernel body and fixture dependency"
    )
    (
      assert lib.hasInfix cases.randomness.statement (render cases.randomness files).${kernel};
      assert lib.hasInfix ''getrandom = "=${policy.randomness_dependency.version}"''
        (render cases.randomness files)."crates/noble-kernel/Cargo.toml";
      "randomness uses a real version-pinned registry dependency"
    )
    (rejects "randomness missing anchor rejects" cases.randomness (
      files // { ${kernel} = "changed source"; }
    ))
    (rejects "missing anchor rejects" fs (files // { ${kernel} = "changed source"; }))
    (rejects "repeated anchor rejects" fs (
      files // { ${kernel} = files.${kernel} + policy.body_anchor; }
    ))
    (rejects "unknown mutation rejects" (fs // { mutation = "unreviewed"; }) files)
    (rejects "unknown baseline rejects" (fs // { base = "unreviewed"; }) files)
    (
      assert lib.hasInfix ''unsafe extern "C"''
        (render cases.unsafe-foreign-declaration files).${kernel};
      assert lib.hasInfix "pub fn boundary_foreign_symbol();"
        (render cases.unsafe-foreign-declaration files).${kernel};
      "foreign declaration control adds a public unsafe extern item"
    )
    (
      assert builtins.attrNames (render cases.unsafe-foreign-declaration files) == [ kernel ];
      "foreign declaration control changes only the kernel source"
    )
    (
      assert lib.hasInfix "#![cfg_attr(not(test), no_std)]"
        (render cases.no-std-test-witness files).${kernel};
      assert !lib.hasInfix "#![no_std]" (render cases.no-std-test-witness files).${kernel};
      "no-std witness control declares the attribute only outside the test configuration"
    )
    (rejects "no-std witness requires exactly one attribute" cases.no-std-test-witness (
      files // { ${kernel} = "// no attribute here\n"; }
    ))
    (
      assert builtins.attrNames (render cases.test-source-import files) == [
        kernel
        policy.test_source.path
      ];
      "test-source import control adds one test-labelled file and the kernel import"
    )
    (
      assert lib.hasInfix ''#[path = "${policy.test_source.relative}"]''
        (render cases.test-source-import files).${kernel};
      assert lib.hasInfix "pub mod ${policy.test_source.module};"
        (render cases.test-source-import files).${kernel};
      assert (render cases.test-source-import files).${policy.test_source.path}
        == "pub fn imported_value() -> u32 { 7 }\n";
      "test-source import uses the declared parent-relative path"
    )
  ];
in
builtins.deepSeq tests tests
