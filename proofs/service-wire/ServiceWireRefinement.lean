import NobleKernel
import NobleFoundation.ServiceWire

open Aeneas Aeneas.Std
open noble_kernel

set_option maxHeartbeats 2000000
set_option maxRecDepth 4096

/-!
# The extracted ServiceRef decoder refines the Lean model (issue #48)

`dataspace.wire.decode_service` is the checked-in Charon/Aeneas translation
(`proofs/m3/NobleKernel/Funs.lean`) of `decode_service` in `crates/noble-kernel/src/dataspace/wire.rs`.
`decode_service_refines` states its result for every byte slice and every configured depth: the
translation returns, and its result read through `observed` is the decision of the model decoder
`NobleFoundation.ServiceWire.decode`. The last section carries totality, exact acceptance, exact
refusal classes, decoding of every valid model encoding and canonical-only acceptance over to the
translation.

Trusted rather than proved here: the Charon/Aeneas translation itself; the Aeneas library models of
slices, arrays, range indexing, scalars, checked subtraction and loops; and the handwritten external
models the decoder reaches in `proofs/m3/NobleKernel/FunsExternal.lean` (`from_utf8`,
`is_ascii_alphanumeric`, and `str` length, emptiness and byte view). `from_utf8` is modelled as
returning its input on valid UTF-8. The encoder `encode_service` is not related to the model here.
-/

namespace ServiceWireRefinement

open NobleFoundation.ServiceWire

/-! ## Byte views -/

/-- The byte values of a slice. -/
def bytesOf (s : Slice U8) : List Nat := s.val.map (·.val)

theorem bytesOf_length (s : Slice U8) : (bytesOf s).length = s.length := by
  simp [bytesOf]

/-! ## The depth scan -/

/-- The decoder's depth scan, as a pure function of the remaining bytes: an opener fails as soon as
it would pass the limit, and a closer saturates at zero. -/
def scanFails (limit : Nat) : Nat → List Nat → Bool
  | _, [] => false
  | depth, byte :: rest =>
      if byte = 60 then (if limit < depth + 1 then true else scanFails limit (depth + 1) rest)
      else if byte = 62 then scanFails limit (depth - 1) rest
      else scanFails limit depth rest

theorem depthScanAux_ge (rest : List Nat) :
    ∀ current reached, reached ≤ depthScanAux current reached rest := by
  induction rest with
  | nil => intro current reached; simp [depthScanAux]
  | cons byte rest ih =>
      intro current reached
      unfold depthScanAux
      split
      · exact le_trans (le_max_left _ _) (ih _ _)
      · split
        · exact ih _ _
        · exact ih _ _

theorem scanFails_eq (limit : Nat) (rest : List Nat) :
    ∀ current reached, reached ≤ limit →
      scanFails limit current rest = decide (limit < depthScanAux current reached rest) := by
  induction rest with
  | nil =>
      intro current reached within
      simp [scanFails, depthScanAux]
      omega
  | cons byte rest ih =>
      intro current reached within
      unfold scanFails depthScanAux
      by_cases opener : byte = 60
      · rw [if_pos opener, if_pos opener]
        by_cases over : limit < current + 1
        · rw [if_pos over]
          have := depthScanAux_ge rest (current + 1) (max reached (current + 1))
          exact (decide_eq_true (by omega)).symm
        · rw [if_neg over]
          exact ih (current + 1) (max reached (current + 1)) (by omega)
      · rw [if_neg opener, if_neg opener]
        by_cases closer : byte = 62
        · rw [if_pos closer, if_pos closer]
          exact ih (current - 1) reached within
        · rw [if_neg closer, if_neg closer]
          exact ih current reached within

/-- **The model's depth decision is the decoder's scan.** -/
theorem depthExceeds_eq_scanFails (limit : Nat) (bytes : List Nat) :
    depthExceeds limit bytes = scanFails limit 0 bytes := by
  rw [scanFails_eq limit bytes 0 0 (Nat.zero_le _)]
  rfl

/-! ## ASCII bytes are valid UTF-8 -/

theorem utf8EncodeChar_ascii : ∀ n < 128, String.utf8EncodeChar (Char.ofNat n) = [UInt8.ofNat n] := by
  decide

theorem flatMap_ascii (bytes : List Nat) (ascii : ∀ n ∈ bytes, n < 128) :
    (bytes.map Char.ofNat).flatMap String.utf8EncodeChar = bytes.map UInt8.ofNat := by
  induction bytes with
  | nil => rfl
  | cons n rest ih =>
      rw [List.map_cons, List.flatMap_cons, utf8EncodeChar_ascii n (ascii n (by simp)),
        ih (fun m member => ascii m (by simp [member]))]
      rfl

theorem ascii_bytes_are_valid_utf8 (bytes : List Nat) (ascii : ∀ n ∈ bytes, n < 128) :
    (ByteArray.mk (bytes.map UInt8.ofNat).toArray).IsValidUTF8 := by
  refine ⟨bytes.map Char.ofNat, ?_⟩
  unfold List.utf8Encode
  rw [flatMap_ascii bytes ascii, ← List.data_toByteArray]

/-- **`from_utf8` accepts an ASCII slice unchanged.** -/
theorem from_utf8_ascii (value : Slice U8) (ascii : ∀ n ∈ bytesOf value, n < 128) :
    core.str.converts.from_utf8 value = Result.ok (.Ok value) := by
  have valid : (ByteArray.mk (value.val.map (fun byte => UInt8.ofNat byte.val)).toArray).IsValidUTF8 := by
    have h := ascii_bytes_are_valid_utf8 (bytesOf value) ascii
    simpa [bytesOf, List.map_map, Function.comp_def] using h
  unfold core.str.converts.from_utf8
  split
  · rfl
  · rename_i none
    simp only [String.fromUTF8?] at none
    split at none
    · cases none
    · rename_i invalid
      exact absurd valid invalid

/-! ## The depth-scan loop -/

theorem bytesOf_drop (bytes : Slice U8) (index : Nat) (hlt : index < bytes.length) :
    (bytesOf bytes).drop index = (bytes.val[index]).val :: (bytesOf bytes).drop (index + 1) := by
  have hlt' : index < (bytesOf bytes).length := by simpa [bytesOf_length] using hlt
  rw [List.drop_eq_getElem_cons hlt']
  simp [bytesOf]

theorem bytesOf_drop_length (bytes : Slice U8) : (bytesOf bytes).drop bytes.length = [] := by
  simp [bytesOf]

/-- A closer saturates the depth at zero. -/
theorem saturating_sub_one_val (x : U32) :
    (core.num.U32.saturating_sub x 1#u32).val = x.val - 1 := by
  unfold core.num.U32.saturating_sub UScalar.saturating_sub
  show (BitVec.ofNat _ (max 0 (x.val - (1#u32).val))).toNat = x.val - 1
  rw [BitVec.toNat_ofNat]
  have h1 : (1#u32).val = 1 := rfl
  rw [h1]
  have hlt : x.val < 2 ^ UScalarTy.U32.numBits := x.bv.isLt
  rw [Nat.mod_eq_of_lt (by omega)]
  omega

/-- **The generated depth scan is the pure scan** for every slice and every limit up to the
allowed depth. -/
theorem check_depth_eq (bytes : Slice U8) (limit : U32) (bounded : limit.val ≤ 4) :
    dataspace.wire.check_depth bytes limit =
      Result.ok (if scanFails limit.val 0 (bytesOf bytes) then .Err .DepthLimit else .Ok ()) := by
  have spec : dataspace.wire.check_depth bytes limit ⦃ r =>
      r = (if scanFails limit.val 0 (bytesOf bytes) then .Err .DepthLimit else .Ok ()) ⦄ := by
    unfold dataspace.wire.check_depth dataspace.wire.check_depth_loop
    apply loop.spec_decr_nat (fun st : U32 × Usize => bytes.length - st.2.val)
      (fun st => st.1.val ≤ limit.val ∧ st.2.val ≤ bytes.length ∧
        scanFails limit.val st.1.val ((bytesOf bytes).drop st.2.val) =
          scanFails limit.val 0 (bytesOf bytes))
    · rintro ⟨depth, index⟩ ⟨hdepth, hindex, hsame⟩
      dsimp only at hdepth hindex hsame ⊢
      unfold dataspace.wire.check_depth_loop.body
      by_cases hlt : index.val < bytes.length
      · have hlt' : index < Slice.len bytes := by scalar_tac
        obtain ⟨byte, hget, hbyte⟩ :=
          WP.spec_imp_exists (Slice.index_usize_spec bytes index hlt)
        obtain ⟨index1, hadd, hval⟩ :=
          WP.spec_imp_exists (UScalar.add_spec (x := index) (y := 1#usize) (by scalar_tac))
        rw [bytesOf_drop bytes index.val hlt, ← hbyte] at hsame
        simp only [hlt', if_true, hget, bind_tc_ok]
        by_cases h60 : byte = 60#u8
        · have hv : byte.val = 60 := by rw [h60]; rfl
          simp only [scanFails, hv, if_true] at hsame
          obtain ⟨depth1, hdadd, hdval⟩ :=
            WP.spec_imp_exists (UScalar.add_spec (x := depth) (y := 1#u32) (by scalar_tac))
          simp only [h60, if_true, hdadd, bind_tc_ok]
          by_cases over : depth1 > limit
          · have over' : limit.val < depth.val + 1 := by scalar_tac
            rw [if_pos over'] at hsame
            simp only [over, if_true, WP.spec_ok]
            rw [← hsame]
            rfl
          · have over' : ¬ limit.val < depth.val + 1 := by scalar_tac
            rw [if_neg over'] at hsame
            simp only [over, if_false, hadd, bind_tc_ok, WP.spec_ok]
            refine ⟨⟨by scalar_tac, by scalar_tac, ?_⟩, by scalar_tac⟩
            rw [hdval, hval]
            exact hsame
        · have hv : byte.val ≠ 60 := fun h => h60 ((UScalar.eq_equiv _ _).mpr (by simpa using h))
          simp only [scanFails, hv, if_false] at hsame
          simp only [h60, if_false]
          by_cases h62 : byte = 62#u8
          · have hv62 : byte.val = 62 := by rw [h62]; rfl
            simp only [hv62, if_true] at hsame
            simp only [h62, if_true, bind_tc_ok, hadd, WP.spec_ok]
            refine ⟨⟨?_, by scalar_tac, ?_⟩, by scalar_tac⟩
            · rw [saturating_sub_one_val]
              omega
            · rw [hval]
              rw [saturating_sub_one_val]
              exact hsame
          · have hv62 : byte.val ≠ 62 := fun h => h62 ((UScalar.eq_equiv _ _).mpr (by simpa using h))
            simp only [hv62, if_false] at hsame
            simp only [h62, if_false, bind_tc_ok, hadd, WP.spec_ok]
            refine ⟨⟨hdepth, by scalar_tac, ?_⟩, by scalar_tac⟩
            rw [hval]
            exact hsame
      · have hge : ¬ index < Slice.len bytes := by scalar_tac
        have hend : index.val = bytes.length := by omega
        rw [hend, bytesOf_drop_length] at hsame
        simp only [hge, if_false, WP.spec_ok]
        rw [← hsame]
        rfl
    · exact ⟨by simp, by simp, by simp⟩
  obtain ⟨r, hr, hpost⟩ := WP.spec_imp_exists spec
  rw [hr, hpost]

/-! ## The prefix loop -/

/-- The generated prefix constant is the model's record prefix. -/
theorem prefix_bytes :
    (Array.make 10#usize [60#u8, 115#u8, 101#u8, 114#u8, 118#u8, 105#u8, 99#u8, 101#u8, 32#u8,
      34#u8]).val.map (·.val) = recordPrefix := by
  simp [Array.make_val]
  rfl

theorem take_succ_of_lt (l : List Nat) (index : Nat) (hlt : index < l.length) :
    l.take (index + 1) = l.take index ++ [l[index]] := by
  rw [List.take_add_one, List.getElem?_eq_getElem hlt]
  rfl

/-- **The generated prefix check is the model's prefix comparison** on every slice of at least ten
bytes. -/
theorem has_service_prefix_eq (bytes : Slice U8) (long : 10 ≤ bytes.length) :
    dataspace.wire.has_service_prefix bytes =
      Result.ok (decide ((bytesOf bytes).take 10 = recordPrefix)) := by
  have hpre := prefix_bytes
  have spec : dataspace.wire.has_service_prefix bytes ⦃ r =>
      r = decide ((bytesOf bytes).take 10 = recordPrefix) ⦄ := by
    unfold dataspace.wire.has_service_prefix dataspace.wire.has_service_prefix_loop
    apply loop.spec_decr_nat (fun index : Usize => 10 - index.val)
      (fun index => index.val ≤ 10 ∧
        (bytesOf bytes).take index.val = recordPrefix.take index.val)
    · rintro index ⟨hle, hsame⟩
      unfold dataspace.wire.has_service_prefix_loop.body
      simp only [lift, bind_tc_ok]
      by_cases hlt : index.val < 10
      · have hlt' : index < Slice.len (Array.to_slice (Array.make 10#usize [60#u8, 115#u8, 101#u8,
            114#u8, 118#u8, 105#u8, 99#u8, 101#u8, 32#u8, 34#u8])) := by
          simp [Array.to_slice]
          scalar_tac
        have hbytes : index.val < bytes.length := by omega
        obtain ⟨byte, hget, hbyte⟩ :=
          WP.spec_imp_exists (Slice.index_usize_spec bytes index hbytes)
        obtain ⟨expected, hexp, hexpected⟩ := WP.spec_imp_exists
          (Array.index_usize_spec (Array.make 10#usize [60#u8, 115#u8, 101#u8, 114#u8, 118#u8,
            105#u8, 99#u8, 101#u8, 32#u8, 34#u8]) index (by simpa using hlt))
        obtain ⟨index1, hadd, hval⟩ :=
          WP.spec_imp_exists (UScalar.add_spec (x := index) (y := 1#usize) (by scalar_tac))
        have hmodel : recordPrefix[index.val]'(by simp [recordPrefix_length]; omega) = expected.val := by
          rw [hexpected]
          simp only [← hpre, List.getElem_map]
        have hbytesval : (bytesOf bytes)[index.val]'(by simp [bytesOf_length]; omega) = byte.val := by
          rw [hbyte]
          simp [bytesOf]
        simp only [hlt', if_true, hget, hexp, bind_tc_ok]
        by_cases same : byte = expected
        · simp only [same, bne_self_eq_false, Bool.false_eq_true, if_false, hadd, bind_tc_ok,
            WP.spec_ok]
          refine ⟨⟨by scalar_tac, ?_⟩, by scalar_tac⟩
          rw [show index1.val = index.val + 1 by scalar_tac]
          rw [take_succ_of_lt _ _ (by simp [bytesOf_length]; omega),
            take_succ_of_lt _ _ (by simp [recordPrefix_length]; omega), hsame, hbytesval, hmodel,
            same]
        · have differ : (byte != expected) = true := by simpa using same
          simp only [differ, if_true, WP.spec_ok]
          symm
          rw [decide_eq_false_iff_not]
          intro whole
          have at10 := congrArg (fun l : List Nat => l[index.val]?) whole
          simp only [List.getElem?_take, if_pos hlt] at at10
          rw [List.getElem?_eq_getElem (by simp [bytesOf_length]; omega),
            List.getElem?_eq_getElem (by simp [recordPrefix_length]; omega), hbytesval, hmodel] at at10
          exact same ((UScalar.eq_equiv _ _).mpr (by simpa using at10))
      · have hend : index.val = 10 := by omega
        have hge : ¬ index < Slice.len (Array.to_slice (Array.make 10#usize [60#u8, 115#u8,
            101#u8, 114#u8, 118#u8, 105#u8, 99#u8, 101#u8, 32#u8, 34#u8])) := by
          simp [Array.to_slice]
          scalar_tac
        simp only [hge, if_false, WP.spec_ok]
        rw [hend] at hsame
        symm
        rw [decide_eq_true_iff]
        rw [hsame]
        rfl
    · exact ⟨by simp, by simp⟩
  obtain ⟨r, hr, hpost⟩ := WP.spec_imp_exists spec
  rw [hr, hpost]

/-! ## The name check -/

theorem take_all (l : List Nat) (n : Nat) (hn : l.length ≤ n) : l.take n = l :=
  List.take_of_length_le hn

/-- **The generated name check is the model's `validName`** on every string. -/
theorem valid_name_eq (label : Str) :
    dataspace.wire.valid_name label = Result.ok (validName (bytesOf label)) := by
  have loopSpec : dataspace.wire.valid_name_loop label 0#usize ⦃ r =>
      r = (bytesOf label).all nameByte ⦄ := by
    unfold dataspace.wire.valid_name_loop
    apply loop.spec_decr_nat (fun index : Usize => label.length - index.val)
      (fun index => index.val ≤ label.length ∧ ((bytesOf label).take index.val).all nameByte)
    · rintro index ⟨hle, hprefix⟩
      unfold dataspace.wire.valid_name_loop.body
      by_cases hlt : index.val < label.length
      · have hlt' : index < Slice.len label := by scalar_tac
        obtain ⟨byte, hget, hbyte⟩ := WP.spec_imp_exists (Slice.index_usize_spec label index hlt)
        obtain ⟨index1, hadd, hval⟩ :=
          WP.spec_imp_exists (UScalar.add_spec (x := index) (y := 1#usize) (by scalar_tac))
        have hbytesval : (bytesOf label)[index.val]'(by simp [bytesOf_length]; omega) = byte.val := by
          rw [hbyte]
          simp [bytesOf]
        have hnext : (bytesOf label).take (index.val + 1) =
            (bytesOf label).take index.val ++ [byte.val] := by
          rw [take_succ_of_lt _ _ (by simp [bytesOf_length]; omega), hbytesval]
        have hclass : nameByte byte.val = (decide ((48 ≤ byte.val ∧ byte.val ≤ 57) ∨
            (65 ≤ byte.val ∧ byte.val ≤ 90) ∨ (97 ≤ byte.val ∧ byte.val ≤ 122)) ||
              byte.val == 95 || byte.val == 45) := by
          unfold nameByte
          simp only [Bool.decide_or, Bool.decide_and, Bool.or_assoc]
        simp only [hlt', if_true, hget, bind_tc_ok, core.num.U8.is_ascii_alphanumeric]
        split
        · rename_i alnum
          simp only [hadd, bind_tc_ok, WP.spec_ok]
          refine ⟨⟨by scalar_tac, ?_⟩, by scalar_tac⟩
          rw [show index1.val = index.val + 1 by scalar_tac]
          rw [hnext, List.all_append, hprefix]
          simp [hclass, alnum]
        · rename_i notAlnum
          by_cases under : byte = 95#u8
          · have hv : byte.val = 95 := by rw [under]; rfl
            simp only [under, bne_self_eq_false, Bool.false_eq_true, if_false, hadd, bind_tc_ok,
              WP.spec_ok]
            refine ⟨⟨by scalar_tac, ?_⟩, by scalar_tac⟩
            rw [show index1.val = index.val + 1 by scalar_tac]
            rw [hnext, List.all_append, hprefix]
            simp [hv, nameByte]
          · have differ : (byte != 95#u8) = true := by simpa using under
            have hv95 : byte.val ≠ 95 := fun h => under ((UScalar.eq_equiv _ _).mpr (by simpa using h))
            simp only [differ, if_true]
            by_cases hyphen : byte = 45#u8
            · have hv : byte.val = 45 := by rw [hyphen]; rfl
              simp only [hyphen, bne_self_eq_false, Bool.false_eq_true, if_false, hadd, bind_tc_ok,
                WP.spec_ok]
              refine ⟨⟨by scalar_tac, ?_⟩, by scalar_tac⟩
              rw [show index1.val = index.val + 1 by scalar_tac]
              rw [hnext, List.all_append, hprefix]
              simp [hv, nameByte]
            · have differ45 : (byte != 45#u8) = true := by simpa using hyphen
              have hv45 : byte.val ≠ 45 := fun h => hyphen ((UScalar.eq_equiv _ _).mpr (by simpa using h))
              simp only [differ45, if_true, WP.spec_ok]
              symm
              rw [Bool.eq_false_iff]
              intro all
              have member : byte.val ∈ bytesOf label := by
                rw [← hbytesval]
                exact List.getElem_mem _
              have good := List.all_eq_true.mp all byte.val member
              rw [hclass] at good
              simp [hv95, hv45, notAlnum] at good
      · have hend : index.val = label.length := by omega
        have hge : ¬ index < Slice.len label := by scalar_tac
        simp only [hge, if_false, WP.spec_ok]
        rw [hend, take_all _ _ (by simp [bytesOf_length])] at hprefix
        exact hprefix.symm
    · exact ⟨by simp, by simp⟩
  obtain ⟨r, hr, hpost⟩ := WP.spec_imp_exists loopSpec
  unfold dataspace.wire.valid_name
  simp only [core.str.Str.is_empty, core.str.Str.len, core.str.Str.as_bytes, bind_tc_ok]
  unfold validName
  by_cases empty : label.val.isEmpty
  · have zero : (bytesOf label).length = 0 := by
      simp [bytesOf]
      simpa using empty
    simp [empty, zero]
  · have nonzero : 1 ≤ (bytesOf label).length := by
      simp [bytesOf]
      cases h : label.val with
      | nil => simp [h] at empty
      | cons _ _ => simp
    by_cases long : label.len > dataspace.MAX_NAME_BYTES
    · have toolong : ¬ (bytesOf label).length ≤ maxNameBytes := by
        simp only [dataspace.MAX_NAME_BYTES] at long
        simp only [bytesOf_length, maxNameBytes]
        scalar_tac
      simp [empty, long, toolong]
    · have fits : (bytesOf label).length ≤ maxNameBytes := by
        simp only [dataspace.MAX_NAME_BYTES] at long
        simp only [bytesOf_length, maxNameBytes]
        scalar_tac
      simp only [empty, Bool.false_eq_true, if_false, long, hr, hpost]
      simp [nonzero, fits]

/-! ## The decoder -/

/-- The name bytes of a record: what lies between the ten-byte prefix and the five-byte suffix. -/
def recordMiddle (bytes : List Nat) : List Nat := (bytes.drop 10).take (bytes.length - 15)

theorem encode_take (label : List Nat) (ready : Bool) :
    (encode label ready).take 10 = recordPrefix := by
  unfold encode
  rw [List.append_assoc, List.take_left' recordPrefix_length]

theorem encode_suffix (label : List Nat) (ready : Bool) :
    (encode label ready).drop ((encode label ready).length - 5) = recordSuffix ready := by
  rw [encode_length]
  unfold encode
  rw [show 15 + label.length - 5 = (recordPrefix ++ label).length by
    rw [List.length_append, recordPrefix_length]; omega, List.drop_left]

theorem encode_middle (label : List Nat) (ready : Bool) :
    recordMiddle (encode label ready) = label := by
  unfold recordMiddle
  rw [encode_length]
  unfold encode
  rw [List.append_assoc, List.drop_left' recordPrefix_length,
    show 15 + label.length - 15 = label.length by omega, List.take_left' rfl]

/-- A record of at least fifteen bytes is its prefix, its middle and its suffix. -/
theorem split_record (bytes : List Nat) (long : 15 ≤ bytes.length) :
    bytes = bytes.take 10 ++ recordMiddle bytes ++ bytes.drop (bytes.length - 5) := by
  unfold recordMiddle
  have middle : (bytes.drop 10).take (bytes.length - 15) ++ (bytes.drop 10).drop (bytes.length - 15) =
      bytes.drop 10 := List.take_append_drop _ _
  rw [List.drop_drop, show 10 + (bytes.length - 15) = bytes.length - 5 by omega] at middle
  rw [List.append_assoc, middle, List.take_append_drop]

/-- A record with the prefix and a suffix is the encoding of its middle. -/
theorem encode_of_shape (bytes : List Nat) (ready : Bool) (long : 15 ≤ bytes.length)
    (pre : bytes.take 10 = recordPrefix) (post : bytes.drop (bytes.length - 5) = recordSuffix ready) :
    encode (recordMiddle bytes) ready = bytes := by
  unfold encode
  rw [← pre, ← post]
  exact (split_record bytes long).symm

theorem suffix_of_encoding (bytes : List Nat) (label : List Nat) (ready : Bool)
    (enc : encode label ready = bytes) : bytes.drop (bytes.length - 5) = recordSuffix ready := by
  rw [← enc]
  exact encode_suffix label ready

/-- Every allowed name byte is ASCII. -/
theorem validName_ascii (label : List Nat) (valid : validName label = true) : ∀ n ∈ label, n < 128 := by
  unfold validName at valid
  simp only [Bool.and_eq_true, List.all_eq_true] at valid
  intro n mem
  have h := valid.2 n mem
  unfold nameByte at h
  simp at h
  omega

/-! ## Slices and the external `from_utf8` -/

theorem bytesOf_slice_drop (bytes : Slice U8) (start : Usize) :
    bytesOf (bytes.drop start) = (bytesOf bytes).drop start.val := by
  simp [bytesOf, List.map_drop]

theorem index_from (bytes : Slice U8) (start : Usize) (fits : start.val ≤ bytes.length) :
    core.slice.index.Slice.index (core.slice.index.SliceIndexRangeFromUsizeSlice U8) bytes
      { start := start } = Result.ok (bytes.drop start) := by
  simp only [core.slice.index.Slice.index, core.slice.index.SliceIndexRangeFromUsizeSlice.index,
    if_pos fits]

theorem index_range (bytes : Slice U8) (start stop : Usize) (ordered : start.val ≤ stop.val)
    (fits : stop.val ≤ bytes.length) :
    ∃ part : Slice U8, core.slice.index.Slice.index (core.slice.index.SliceIndexRangeUsizeSlice U8) bytes
        { start := start, «end» := stop } = Result.ok part ∧
      bytesOf part = ((bytesOf bytes).drop start.val).take (stop.val - start.val) := by
  simp only [core.slice.index.Slice.index, core.slice.index.SliceIndexRangeUsizeSlice.index]
  rw [if_pos ⟨by scalar_tac, by scalar_tac⟩]
  refine ⟨_, rfl, ?_⟩
  simp [bytesOf, Slice.from_val, List.slice, List.map_take, List.map_drop]

theorem index_usize_eq (v : Slice U8) (k : Usize) (i : Nat) (hi : k.val = i) (l : List U8)
    (hv : v.val = l) (hk : i < l.length) : v.index_usize k = Result.ok (l[i]'hk) := by
  subst hi hv
  obtain ⟨y, hy, hyv⟩ := WP.spec_imp_exists (Slice.index_usize_spec v k hk)
  rw [hy, hyv]

theorem five_cases (l : List U8) (h : l.length = 5) : ∃ a b c d e, l = [a, b, c, d, e] := by
  obtain _ | ⟨a, _ | ⟨b, _ | ⟨c, _ | ⟨d, _ | ⟨e, _ | ⟨f, rest⟩⟩⟩⟩⟩⟩ := l <;> simp at h
  exact ⟨a, b, c, d, e, rfl⟩

/-- The external `from_utf8` model either returns its input or reports invalid UTF-8. -/
theorem from_utf8_cases (value : Slice U8) :
    core.str.converts.from_utf8 value = Result.ok (.Ok value) ∨
      core.str.converts.from_utf8 value = Result.ok (.Err .Invalid) := by
  unfold core.str.converts.from_utf8
  split
  · exact Or.inl rfl
  · exact Or.inr rfl

theorem prefix_len : (Array.to_slice (Array.make 10#usize [60#u8, 115#u8, 101#u8, 114#u8, 118#u8,
    105#u8, 99#u8, 101#u8, 32#u8, 34#u8])).len = 10#usize := by
  apply (UScalar.eq_equiv _ _).mpr
  simp [Array.to_slice]

theorem suffix_len :
    (Array.to_slice (Array.make 5#usize [34#u8, 32#u8, 35#u8, 116#u8, 62#u8])).len = 5#usize := by
  apply (UScalar.eq_equiv _ _).mpr
  simp [Array.to_slice]

theorem ite_ok_decide (p q : Prop) [Decidable p] [Decidable q] :
    (if p then (Result.ok (decide q) : Result Bool) else Result.ok false) = Result.ok (decide (p ∧ q)) := by
  by_cases hp : p <;> simp [hp]

/-- A decoder result read as a model decision. Each error class the decoder may return has its model
class; any other error has none, so an equality with `some` rules those out too. -/
def observed : core.result.Result dataspace.wire.ServiceRef dataspace.Error → Option Decision
  | .Ok value => some (.accepted (bytesOf value.«name») value.ready)
  | .Err .ByteLimit => some .byteLimit
  | .Err .DepthLimit => some .depthLimit
  | .Err .Schema => some .schema
  | .Err _ => none

/-- Past the byte budget and the depth checks, the model decides schema unless it accepts. -/
theorem decode_after_checks (bytes : List Nat) (configured : Nat)
    (len : bytes.length ≤ maxWireBytes) (allowed : 1 ≤ configured ∧ configured ≤ maxWireDepth)
    (scan : depthExceeds configured bytes = false) :
    decode bytes configured = .schema ∨
      ∃ label ready, decode bytes configured = .accepted label ready := by
  unfold decode
  rw [if_neg (by omega), if_neg (by simpa using allowed), if_neg (by simp [scan])]
  split
  · exact Or.inl rfl
  · split
    · split
      · exact Or.inr ⟨_, _, rfl⟩
      · exact Or.inl rfl
    · exact Or.inl rfl

/-- Past the checks, the model decides schema unless the bytes encode a valid name. -/
theorem schema_of_refuted (bytes : List Nat) (configured : Nat)
    (len : bytes.length ≤ maxWireBytes) (allowed : 1 ≤ configured ∧ configured ≤ maxWireDepth)
    (scan : depthExceeds configured bytes = false)
    (refute : ∀ label ready, validName label = true → encode label ready = bytes → False) :
    decode bytes configured = .schema := by
  rcases decode_after_checks bytes configured len allowed scan with h | ⟨label, ready, h⟩
  · exact h
  · exact (refute label ready (accepted_name_is_valid _ _ _ _ h) (encode_of_accepted _ _ _ _ h)).elim

/-- A record with the prefix, a suffix and a valid middle is accepted with that middle. -/
theorem accepted_of_shape (bytes : List Nat) (configured : Nat) (ready : Bool)
    (allowed : 1 ≤ configured ∧ configured ≤ maxWireDepth) (long : 15 ≤ bytes.length)
    (pre : bytes.take 10 = recordPrefix) (post : bytes.drop (bytes.length - 5) = recordSuffix ready)
    (valid : validName (recordMiddle bytes) = true) :
    decode bytes configured = .accepted (recordMiddle bytes) ready := by
  have h := decode_encode (recordMiddle bytes) ready configured valid allowed
  rwa [encode_of_shape bytes ready long pre post] at h

theorem min_service_bytes : dataspace.wire.decode_service.MIN_SERVICE_BYTES.val = 15 := by
  unfold dataspace.wire.decode_service.MIN_SERVICE_BYTES
  simp [Array.to_slice]

theorem decode_service_refines (bytes : Slice U8) (configured : U32) :
    ∃ r, dataspace.wire.decode_service bytes configured = Result.ok r ∧
      observed r = some (decode (bytesOf bytes) configured.val) := by
  unfold dataspace.wire.decode_service
  by_cases over : Slice.len bytes > dataspace.MAX_WIRE_BYTES
  · have long : (bytesOf bytes).length > maxWireBytes := by
      simp only [dataspace.MAX_WIRE_BYTES] at over
      simp only [bytesOf_length, maxWireBytes]
      scalar_tac
    refine ⟨.Err .ByteLimit, by simp only [over, if_true], ?_⟩
    simp [observed, decode, long]
  have fits : (bytesOf bytes).length ≤ maxWireBytes := by
    simp only [dataspace.MAX_WIRE_BYTES] at over
    simp only [bytesOf_length, maxWireBytes]
    scalar_tac
  simp only [over, if_false]
  by_cases zero : configured = 0#u32
  · have disallowed : ¬ (1 ≤ configured.val ∧ configured.val ≤ maxWireDepth) := by
      rw [zero]
      simp
    refine ⟨.Err .DepthLimit, by simp only [zero, if_true], ?_⟩
    simp [observed, decode, disallowed, show ¬ (bytesOf bytes).length > maxWireBytes by omega]
  simp only [zero, if_false]
  by_cases deep : configured > dataspace.MAX_WIRE_DEPTH
  · have disallowed : ¬ (1 ≤ configured.val ∧ configured.val ≤ maxWireDepth) := by
      simp only [dataspace.MAX_WIRE_DEPTH] at deep
      simp only [maxWireDepth]
      scalar_tac
    refine ⟨.Err .DepthLimit, by simp only [deep, if_true], ?_⟩
    simp [observed, decode, disallowed, show ¬ (bytesOf bytes).length > maxWireBytes by omega]
  have allowed : 1 ≤ configured.val ∧ configured.val ≤ maxWireDepth := by
    simp only [dataspace.MAX_WIRE_DEPTH] at deep
    have : configured.val ≠ 0 := fun h => zero ((UScalar.eq_equiv _ _).mpr (by simpa using h))
    simp only [maxWireDepth]
    scalar_tac
  simp only [deep, if_false]
  rw [check_depth_eq bytes configured (by simpa [maxWireDepth] using allowed.2)]
  simp only [bind_tc_ok]
  by_cases fails : scanFails configured.val 0 (bytesOf bytes) = true
  · refine ⟨.Err .DepthLimit, by simp only [fails, if_true], ?_⟩
    have exceeds : depthExceeds configured.val (bytesOf bytes) = true := by
      rw [depthExceeds_eq_scanFails]
      exact fails
    simp [observed, decode, exceeds, allowed, show ¬ (bytesOf bytes).length > maxWireBytes by omega]
  have passes : depthExceeds configured.val (bytesOf bytes) = false := by
    rw [depthExceeds_eq_scanFails]
    simpa using fails
  simp only [fails, Bool.false_eq_true, if_false]
  simp only [lift, bind_tc_ok, prefix_len, suffix_len]
  have min15 := min_service_bytes
  by_cases short : bytes.len < dataspace.wire.decode_service.MIN_SERVICE_BYTES
  · have short' : (bytesOf bytes).length < 15 := by rw [bytesOf_length]; scalar_tac
    refine ⟨.Err .Schema, by rw [if_pos short], ?_⟩
    simp only [observed]
    rw [schema_of_refuted _ _ fits allowed passes]
    intro label ready _ enc
    have := encode_length label ready
    rw [enc] at this
    omega
  have long : 15 ≤ (bytesOf bytes).length := by rw [bytesOf_length]; scalar_tac
  rw [if_neg short, has_service_prefix_eq bytes (by rw [bytesOf_length] at long; omega)]
  simp only [bind_tc_ok]
  by_cases pre : (bytesOf bytes).take 10 = recordPrefix
  swap
  · refine ⟨.Err .Schema, by simp [pre], ?_⟩
    simp only [observed]
    rw [schema_of_refuted _ _ fits allowed passes]
    intro label ready _ enc
    rw [← enc] at pre
    exact pre (encode_take label ready)
  rw [if_pos (decide_eq_true pre)]
  obtain ⟨name_end, hsome, hval⟩ : ∃ n : Usize, bytes.len.checked_sub 5#usize = some n ∧
      n.val = (bytesOf bytes).length - 5 := by
    have hsub := Usize.checked_sub_bv_spec bytes.len 5#usize
    cases hc : bytes.len.checked_sub 5#usize with
    | none =>
        rw [hc] at hsub
        rw [bytesOf_length] at long
        scalar_tac
    | some z =>
        rw [hc] at hsub
        exact ⟨z, rfl, by rw [bytesOf_length]; scalar_tac⟩
  simp only [hsome]
  rw [if_neg (show ¬ name_end < 10#usize by scalar_tac),
    index_from bytes name_end (by rw [bytesOf_length] at hval; scalar_tac)]
  simp only [bind_tc_ok]
  obtain ⟨a, b, c, d, e, hsfx⟩ := five_cases (bytes.drop name_end).val
    (by rw [Slice.getElem!_val_drop, List.length_drop]; rw [bytesOf_length] at hval long; scalar_tac)
  have hT : (bytesOf bytes).drop ((bytesOf bytes).length - 5) = [a.val, b.val, c.val, d.val, e.val] := by
    have h1 : bytesOf (bytes.drop name_end) = [a.val, b.val, c.val, d.val, e.val] := by
      unfold bytesOf
      rw [hsfx]
      rfl
    rw [bytesOf_slice_drop, hval] at h1
    exact h1
  rw [index_usize_eq _ 0#usize 0 (by scalar_tac) _ hsfx (by simp),
    index_usize_eq _ 1#usize 1 (by scalar_tac) _ hsfx (by simp),
    index_usize_eq _ 2#usize 2 (by scalar_tac) _ hsfx (by simp),
    index_usize_eq _ 3#usize 3 (by scalar_tac) _ hsfx (by simp),
    index_usize_eq _ 4#usize 4 (by scalar_tac) _ hsfx (by simp)]
  simp only [bind_tc_ok, List.getElem_cons_zero, List.getElem_cons_succ]
  simp only [ite_ok_decide, bind_tc_ok, decide_eq_true_eq]
  have suffix_bytes : ∀ label ready, encode label ready = bytesOf bytes →
      a.val = 34 ∧ b.val = 32 ∧ c.val = 35 ∧ d.val = (if ready then 116 else 102) ∧ e.val = 62 := by
    intro label ready enc
    have h := suffix_of_encoding _ label ready enc
    rw [hT] at h
    unfold recordSuffix at h
    simp only [List.cons.injEq] at h
    exact ⟨h.1, h.2.1, h.2.2.1, h.2.2.2.1, h.2.2.2.2.1⟩
  by_cases quote : a = 34#u8 ∧ b = 32#u8
  swap
  · refine ⟨.Err .Schema, by rw [if_neg quote], ?_⟩
    simp only [observed]
    rw [schema_of_refuted _ _ fits allowed passes]
    intro label ready _ enc
    obtain ⟨ha, hb, -, -, -⟩ := suffix_bytes label ready enc
    exact quote ⟨(UScalar.eq_equiv _ _).mpr (by simpa using ha),
      (UScalar.eq_equiv _ _).mpr (by simpa using hb)⟩
  rw [if_pos quote]
  by_cases close : c = 35#u8 ∧ e = 62#u8
  swap
  · refine ⟨.Err .Schema, by rw [if_neg close], ?_⟩
    simp only [observed]
    rw [schema_of_refuted _ _ fits allowed passes]
    intro label ready _ enc
    obtain ⟨-, -, hc, -, he⟩ := suffix_bytes label ready enc
    exact close ⟨(UScalar.eq_equiv _ _).mpr (by simpa using hc),
      (UScalar.eq_equiv _ _).mpr (by simpa using he)⟩
  rw [if_pos close]
  have ha : a.val = 34 := by rw [quote.1]; rfl
  have hb : b.val = 32 := by rw [quote.2]; rfl
  have hc : c.val = 35 := by rw [close.1]; rfl
  have he : e.val = 62 := by rw [close.2]; rfl
  have post : ∀ ready, d.val = (if ready then 116 else 102) →
      (bytesOf bytes).drop ((bytesOf bytes).length - 5) = recordSuffix ready := by
    intro ready hd
    rw [hT, ha, hb, hc, he, hd]
    rfl
  obtain ⟨nm, hnm, hnmbytes⟩ := index_range bytes 10#usize name_end (by scalar_tac)
    (by rw [bytesOf_length] at hval long; scalar_tac)
  have middle : bytesOf nm = recordMiddle (bytesOf bytes) := by
    have ten : (10#usize).val = 10 := by scalar_tac
    rw [hnmbytes, ten, hval, show (bytesOf bytes).length - 5 - 10 = (bytesOf bytes).length - 15 by omega]
    rfl
  have middle_of : ∀ label ready, encode label ready = bytesOf bytes → label = bytesOf nm := by
    intro label ready enc
    rw [middle, ← enc, encode_middle]
  split
  · have post116 := post true rfl
    rw [hnm]
    simp only [bind_tc_ok]
    rcases from_utf8_cases nm with hu | hu
    · rw [hu]
      simp only [bind_tc_ok]
      rw [valid_name_eq]
      simp only [bind_tc_ok]
      by_cases hv : validName (bytesOf nm) = true
      · refine ⟨.Ok { «name» := nm, ready := true }, by rw [if_pos hv], ?_⟩
        simp only [observed]
        rw [middle] at hv ⊢
        rw [accepted_of_shape _ _ true allowed long pre post116 hv]
      · refine ⟨.Err .Schema, by rw [if_neg hv], ?_⟩
        simp only [observed]
        rw [schema_of_refuted _ _ fits allowed passes]
        intro label ready valid enc
        rw [middle_of label ready enc] at valid
        exact hv valid
    · rw [hu]
      simp only [bind_tc_ok]
      refine ⟨.Err .Schema, rfl, ?_⟩
      simp only [observed]
      rw [schema_of_refuted _ _ fits allowed passes]
      intro label ready valid enc
      rw [middle_of label ready enc] at valid
      rw [from_utf8_ascii nm (validName_ascii _ valid), Result.ok.injEq] at hu
      cases hu
  · have post102 := post false rfl
    rw [hnm]
    simp only [bind_tc_ok]
    rcases from_utf8_cases nm with hu | hu
    · rw [hu]
      simp only [bind_tc_ok]
      rw [valid_name_eq]
      simp only [bind_tc_ok]
      by_cases hv : validName (bytesOf nm) = true
      · refine ⟨.Ok { «name» := nm, ready := false }, by rw [if_pos hv], ?_⟩
        simp only [observed]
        rw [middle] at hv ⊢
        rw [accepted_of_shape _ _ false allowed long pre post102 hv]
      · refine ⟨.Err .Schema, by rw [if_neg hv], ?_⟩
        simp only [observed]
        rw [schema_of_refuted _ _ fits allowed passes]
        intro label ready valid enc
        rw [middle_of label ready enc] at valid
        exact hv valid
    · rw [hu]
      simp only [bind_tc_ok]
      refine ⟨.Err .Schema, rfl, ?_⟩
      simp only [observed]
      rw [schema_of_refuted _ _ fits allowed passes]
      intro label ready valid enc
      rw [middle_of label ready enc] at valid
      rw [from_utf8_ascii nm (validName_ascii _ valid), Result.ok.injEq] at hu
      cases hu
  · refine ⟨.Err .Schema, rfl, ?_⟩
    simp only [observed]
    rw [schema_of_refuted _ _ fits allowed passes]
    intro label ready _ enc
    obtain ⟨-, -, -, hd, -⟩ := suffix_bytes label ready enc
    cases ready
    · exact ‹d = 102#u8 → False› ((UScalar.eq_equiv _ _).mpr (by rw [hd]; rfl))
    · exact ‹d = 116#u8 → False› ((UScalar.eq_equiv _ _).mpr (by rw [hd]; rfl))

/-! ## Consequences for the extracted decoder -/

/-- **Totality.** The extracted decoder returns on every byte slice and every configured depth: no
panic, no failed bound check and no divergence. -/
theorem decode_service_total (bytes : Slice U8) (configured : U32) :
    ∃ r, dataspace.wire.decode_service bytes configured = Result.ok r := by
  obtain ⟨r, h, -⟩ := decode_service_refines bytes configured
  exact ⟨r, h⟩

/-- **Acceptance agrees exactly.** The extracted decoder returns a value with these name bytes and
this readiness tag if and only if the model accepts them. -/
theorem decode_service_accepts_iff (bytes : Slice U8) (configured : U32) (label : List Nat)
    (ready : Bool) :
    (∃ value : dataspace.wire.ServiceRef,
        dataspace.wire.decode_service bytes configured = Result.ok (.Ok value) ∧
          bytesOf value.«name» = label ∧ value.ready = ready) ↔
      decode (bytesOf bytes) configured.val = .accepted label ready := by
  obtain ⟨r, h, hobs⟩ := decode_service_refines bytes configured
  constructor
  · rintro ⟨value, hv, hlabel, hready⟩
    rw [hv, Result.ok.injEq] at h
    subst h
    simp only [observed, Option.some.injEq] at hobs
    rw [← hobs, hlabel, hready]
  · intro hmodel
    rw [hmodel] at hobs
    cases r with
    | Ok value =>
        simp only [observed, Option.some.injEq, Decision.accepted.injEq] at hobs
        exact ⟨value, h, hobs.1, hobs.2⟩
    | Err e => cases e <;> simp [observed] at hobs

/-- **Refusal classes agree exactly.** The extracted decoder returns each of its three refusals if
and only if the model decides that class; it returns no other error. -/
theorem decode_service_refuses_iff (bytes : Slice U8) (configured : U32) :
    (dataspace.wire.decode_service bytes configured = Result.ok (.Err .ByteLimit) ↔
        decode (bytesOf bytes) configured.val = .byteLimit) ∧
    (dataspace.wire.decode_service bytes configured = Result.ok (.Err .DepthLimit) ↔
        decode (bytesOf bytes) configured.val = .depthLimit) ∧
    (dataspace.wire.decode_service bytes configured = Result.ok (.Err .Schema) ↔
        decode (bytesOf bytes) configured.val = .schema) := by
  obtain ⟨r, h, hobs⟩ := decode_service_refines bytes configured
  rw [h]
  simp only [Result.ok.injEq]
  cases r with
  | Ok value =>
      simp only [observed, Option.some.injEq] at hobs
      simp [← hobs]
  | Err e => cases e <;> simp [observed] at hobs <;> simp [← hobs]

/-- **Model encodings decode.** Bytes that are the model encoding of a valid name decode, through the
extracted decoder, to that name and readiness tag under every allowed configured depth. -/
theorem decode_service_of_encode (bytes : Slice U8) (configured : U32) (label : List Nat)
    (ready : Bool) (valid : validName label = true)
    (depth : 1 ≤ configured.val ∧ configured.val ≤ maxWireDepth)
    (wire : bytesOf bytes = encode label ready) :
    ∃ value : dataspace.wire.ServiceRef,
      dataspace.wire.decode_service bytes configured = Result.ok (.Ok value) ∧
        bytesOf value.«name» = label ∧ value.ready = ready := by
  rw [decode_service_accepts_iff, wire]
  exact decode_encode label ready configured.val valid depth

/-- **Only canonical encodings are accepted.** Whatever the extracted decoder accepts is exactly the
model encoding of a valid name: no trailing byte, alternate spelling or escape is accepted. -/
theorem decode_service_accepted_is_canonical (bytes : Slice U8) (configured : U32)
    (value : dataspace.wire.ServiceRef)
    (h : dataspace.wire.decode_service bytes configured = Result.ok (.Ok value)) :
    validName (bytesOf value.«name») = true ∧
      encode (bytesOf value.«name») value.ready = bytesOf bytes := by
  have hmodel := (decode_service_accepts_iff bytes configured _ _).mp ⟨value, h, rfl, rfl⟩
  exact ⟨accepted_name_is_valid _ _ _ _ hmodel, encode_of_accepted _ _ _ _ hmodel⟩

end ServiceWireRefinement
