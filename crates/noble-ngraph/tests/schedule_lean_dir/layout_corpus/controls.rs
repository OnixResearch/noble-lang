pub(super) const LEAN_FRAGMENT: &str = r#"/-! These controls demonstrate why the input encoding and size claim
must retain every field. The metadata extension below is model-only: its extra layer is not
derived by Rust `CheckedGraph` and is deliberately outside the Rust correspondence boundary. -/

def nodesOnlyEncoding (input : Input) : List Bool :=
  encodeList encodeNode input.nodes

theorem nodesOnlyEncoding_is_not_injective :
    ∃ first second, first ≠ second ∧ nodesOnlyEncoding first = nodesOnlyEncoding second := by
  refine ⟨two_constants_real, two_constants_stale_revision, ?_, ?_⟩
  · intro same
    have revisionSame := congrArg (fun input : Input => input.tables.revisionOk) same
    simp [two_constants_real, two_constants_stale_revision] at revisionSame
  · rfl

def finiteFixtureLookup (input : Input) : Bool :=
  [interleaved_real.layerCount, two_constants_real.layerCount].contains input.layerCount

def two_constants_with_trusted_layer_extension : Input :=
  { two_constants_real with
    layerCount := 5,
    tables := { two_constants_real.tables with
      layers := two_constants_real.tables.layers ++ [(5, 5)],
      failSuffixMin := two_constants_real.tables.failSuffixMin ++ [maxRegister] } }

theorem finiteFixtureLookup_misses_model_accepted_extension :
    acceptedInput two_constants_with_trusted_layer_extension = true ∧
      finiteFixtureLookup two_constants_with_trusted_layer_extension = false := by
  decide

theorem node_count_does_not_measure_encoded_input :
    two_constants_real.nodes.length = two_constants_with_trusted_layer_extension.nodes.length ∧
      encodedSize two_constants_real <
        encodedSize two_constants_with_trusted_layer_extension := by
  constructor
  · rfl
  · have hsize :
        encodedSize two_constants_with_trusted_layer_extension =
          encodedSize two_constants_real + 79 := by
      simp [encodedSize, encodeInput, encodeTables, encodeNatPair,
        encodeConstant, encodeRegister, encodeWord, encodeNode, encodeBool, natWidth,
        maxRegister,
        two_constants_with_trusted_layer_extension, two_constants_real]
    omega

"#;
