/-
  A bounded model of static-input specialization for packed graphs (issue #32).

  The specialization keeps node indices fixed, substitutes static inputs, renumbers
  dynamic inputs in their original order, and folds only arithmetic nodes whose
  operands are statically known.  This is a graph model; it does not claim the
  Rust emitter, effect handling, budgets or any other language surface.
-/
import NobleFoundation.PackedSchedule
import NobleFoundation.GraphRewrite

namespace NobleFoundation.Specialize

open NobleFoundation.PackedSchedule

/-- Number of dynamic input slots strictly below `slot`. -/
def rank (σ : Nat → Option (BitVec 64)) : Nat → Nat
  | 0 => 0
  | slot + 1 => rank σ slot + if σ slot = none then 1 else 0

/-- Reconstruct the full input assignment from static values and dynamic inputs. -/
def merge (σ : Nat → Option (BitVec 64)) (d : Nat → BitVec 64) (slot : Nat) : BitVec 64 :=
  match σ slot with
  | some value => value
  | none => d (rank σ slot)

/-- The statically known I64 result of one original node, when available. -/
def staticStep (graph : Graph) (σ : Nat → Option (BitVec 64))
    (values : Nat → Option (BitVec 64)) (index : Nat) : Option (BitVec 64) :=
  match graph.node index with
  | .input slot => σ slot
  | .constant bits => some bits
  | .add first second =>
      match values first, values second with
      | some x, some y => some (x + y)
      | _, _ => none
  | .sub first second =>
      match values first, values second with
      | some x, some y => some (x - y)
      | _, _ => none
  | .mul first second =>
      match values first, values second with
      | some x, some y => some (x * y)
      | _, _ => none
  | .eq _ _ => none

/-- Static values of the first `count` nodes. -/
def staticValuesUpTo (graph : Graph) (σ : Nat → Option (BitVec 64)) :
    Nat → (Nat → Option (BitVec 64))
  | 0 => fun _ => none
  | count + 1 => fun index =>
      if index = count then
        staticStep graph σ (staticValuesUpTo graph σ count) count
      else staticValuesUpTo graph σ count index

/-- The statically known I64 result of one node. -/
def staticValue (graph : Graph) (σ : Nat → Option (BitVec 64)) (index : Nat) :
    Option (BitVec 64) :=
  staticValuesUpTo graph σ (index + 1) index

/-- A value in a strict prefix is the same as the value at its own prefix. -/
theorem staticValuesUpTo_eq_staticValue (graph : Graph) (σ : Nat → Option (BitVec 64)) :
    ∀ count index, index < count →
      staticValuesUpTo graph σ count index = staticValue graph σ index := by
  intro count
  induction count with
  | zero => intro index h; exact absurd h (Nat.not_lt_zero index)
  | succ count ih =>
      intro index h
      by_cases hlast : index = count
      · rw [hlast]
        simp [staticValue, staticValuesUpTo]
      · have hlt : index < count := by omega
        simp only [staticValuesUpTo, if_neg hlast]
        exact ih index hlt

/-- Rewrite one node without changing its index. -/
def specializeNode (graph : Graph) (σ : Nat → Option (BitVec 64)) (index : Nat) : Node :=
  match graph.node index with
  | .input slot =>
      match σ slot with
      | some value => .constant value
      | none => .input (rank σ slot)
  | .constant bits => .constant bits
  | .add first second =>
      match staticValue graph σ first, staticValue graph σ second with
      | some x, some y => .constant (x + y)
      | _, _ => .add first second
  | .sub first second =>
      match staticValue graph σ first, staticValue graph σ second with
      | some x, some y => .constant (x - y)
      | _, _ => .sub first second
  | .mul first second =>
      match staticValue graph σ first, staticValue graph σ second with
      | some x, some y => .constant (x * y)
      | _, _ => .mul first second
  | .eq first second => .eq first second

/-- Static-input specialization with node indices and the selected output fixed. -/
def specialize (graph : Graph) (σ : Nat → Option (BitVec 64)) : Graph :=
  { graph with node := specializeNode graph σ }

/-- A static result denotes the corresponding original graph value. -/
theorem staticValue_sound (graph : Graph) (σ : Nat → Option (BitVec 64))
    (hwf : WellFormed graph) (d : Nat → BitVec 64) :
    ∀ index bits, staticValue graph σ index = some bits →
      value graph (merge σ d) index = some (.i64 bits) := by
  intro index
  induction index using Nat.strongRecOn with
  | _ index ih =>
      intro bits hstatic
      cases hn : graph.node index with
      | input slot =>
          have hslot : σ slot = some bits := by
            simpa [staticValue, staticValuesUpTo, staticStep, hn] using hstatic
          simp [value, valuesUpTo, evalStep, hn, merge, hslot]
      | constant valueBits =>
          have hbits : valueBits = bits := by
            simpa [staticValue, staticValuesUpTo, staticStep, hn] using hstatic
          simp [value, valuesUpTo, evalStep, hn, hbits]
      | add first second =>
          have hfirst_lt : first < index :=
            hwf.2 index first (by simp [hn, Node.operands])
          have hsecond_lt : second < index :=
            hwf.2 index second (by simp [hn, Node.operands])
          have hfirst_prefix : staticValuesUpTo graph σ index first =
              staticValue graph σ first :=
            staticValuesUpTo_eq_staticValue graph σ index first hfirst_lt
          have hsecond_prefix : staticValuesUpTo graph σ index second =
              staticValue graph σ second :=
            staticValuesUpTo_eq_staticValue graph σ index second hsecond_lt
          have hstatic_step :
              staticStep graph σ (staticValuesUpTo graph σ index) index = some bits := by
            simpa [staticValue, staticValuesUpTo] using hstatic
          cases hfirst : staticValue graph σ first with
          | none =>
              simp [staticStep, hn, hfirst_prefix, hfirst, hsecond_prefix] at hstatic_step
          | some firstBits =>
              cases hsecond : staticValue graph σ second with
              | none =>
                  simp [staticStep, hn, hfirst_prefix, hfirst, hsecond_prefix, hsecond]
                    at hstatic_step
              | some secondBits =>
                  have hsum : firstBits + secondBits = bits := by
                    simpa [staticStep, hn, hfirst_prefix, hfirst, hsecond_prefix, hsecond]
                      using hstatic_step
                  have hfirst_value := ih first hfirst_lt firstBits hfirst
                  have hsecond_value := ih second hsecond_lt secondBits hsecond
                  have hfirst_up : valuesUpTo graph (merge σ d) index first =
                      some (.i64 firstBits) := by
                    rw [valuesUpTo_eq_value graph (merge σ d) index first hfirst_lt]
                    exact hfirst_value
                  have hsecond_up : valuesUpTo graph (merge σ d) index second =
                      some (.i64 secondBits) := by
                    rw [valuesUpTo_eq_value graph (merge σ d) index second hsecond_lt]
                    exact hsecond_value
                  have hvalue : value graph (merge σ d) index =
                      evalStep graph (valuesUpTo graph (merge σ d) index)
                        (merge σ d) index := by
                    simp [value, valuesUpTo]
                  rw [hvalue]
                  simp [evalStep, hn, hfirst_up, hsecond_up, hsum]
      | sub first second =>
          have hfirst_lt : first < index :=
            hwf.2 index first (by simp [hn, Node.operands])
          have hsecond_lt : second < index :=
            hwf.2 index second (by simp [hn, Node.operands])
          have hfirst_prefix : staticValuesUpTo graph σ index first =
              staticValue graph σ first :=
            staticValuesUpTo_eq_staticValue graph σ index first hfirst_lt
          have hsecond_prefix : staticValuesUpTo graph σ index second =
              staticValue graph σ second :=
            staticValuesUpTo_eq_staticValue graph σ index second hsecond_lt
          have hstatic_step :
              staticStep graph σ (staticValuesUpTo graph σ index) index = some bits := by
            simpa [staticValue, staticValuesUpTo] using hstatic
          cases hfirst : staticValue graph σ first with
          | none =>
              simp [staticStep, hn, hfirst_prefix, hfirst, hsecond_prefix] at hstatic_step
          | some firstBits =>
              cases hsecond : staticValue graph σ second with
              | none =>
                  simp [staticStep, hn, hfirst_prefix, hfirst, hsecond_prefix, hsecond]
                    at hstatic_step
              | some secondBits =>
                  have hdiff : firstBits - secondBits = bits := by
                    simpa [staticStep, hn, hfirst_prefix, hfirst, hsecond_prefix, hsecond]
                      using hstatic_step
                  have hfirst_value := ih first hfirst_lt firstBits hfirst
                  have hsecond_value := ih second hsecond_lt secondBits hsecond
                  have hfirst_up : valuesUpTo graph (merge σ d) index first =
                      some (.i64 firstBits) := by
                    rw [valuesUpTo_eq_value graph (merge σ d) index first hfirst_lt]
                    exact hfirst_value
                  have hsecond_up : valuesUpTo graph (merge σ d) index second =
                      some (.i64 secondBits) := by
                    rw [valuesUpTo_eq_value graph (merge σ d) index second hsecond_lt]
                    exact hsecond_value
                  have hvalue : value graph (merge σ d) index =
                      evalStep graph (valuesUpTo graph (merge σ d) index)
                        (merge σ d) index := by
                    simp [value, valuesUpTo]
                  rw [hvalue]
                  simp [evalStep, hn, hfirst_up, hsecond_up, hdiff]
      | mul first second =>
          have hfirst_lt : first < index :=
            hwf.2 index first (by simp [hn, Node.operands])
          have hsecond_lt : second < index :=
            hwf.2 index second (by simp [hn, Node.operands])
          have hfirst_prefix : staticValuesUpTo graph σ index first =
              staticValue graph σ first :=
            staticValuesUpTo_eq_staticValue graph σ index first hfirst_lt
          have hsecond_prefix : staticValuesUpTo graph σ index second =
              staticValue graph σ second :=
            staticValuesUpTo_eq_staticValue graph σ index second hsecond_lt
          have hstatic_step :
              staticStep graph σ (staticValuesUpTo graph σ index) index = some bits := by
            simpa [staticValue, staticValuesUpTo] using hstatic
          cases hfirst : staticValue graph σ first with
          | none =>
              simp [staticStep, hn, hfirst_prefix, hfirst, hsecond_prefix] at hstatic_step
          | some firstBits =>
              cases hsecond : staticValue graph σ second with
              | none =>
                  simp [staticStep, hn, hfirst_prefix, hfirst, hsecond_prefix, hsecond]
                    at hstatic_step
              | some secondBits =>
                  have hproduct : firstBits * secondBits = bits := by
                    simpa [staticStep, hn, hfirst_prefix, hfirst, hsecond_prefix, hsecond]
                      using hstatic_step
                  have hfirst_value := ih first hfirst_lt firstBits hfirst
                  have hsecond_value := ih second hsecond_lt secondBits hsecond
                  have hfirst_up : valuesUpTo graph (merge σ d) index first =
                      some (.i64 firstBits) := by
                    rw [valuesUpTo_eq_value graph (merge σ d) index first hfirst_lt]
                    exact hfirst_value
                  have hsecond_up : valuesUpTo graph (merge σ d) index second =
                      some (.i64 secondBits) := by
                    rw [valuesUpTo_eq_value graph (merge σ d) index second hsecond_lt]
                    exact hsecond_value
                  have hvalue : value graph (merge σ d) index =
                      evalStep graph (valuesUpTo graph (merge σ d) index)
                        (merge σ d) index := by
                    simp [value, valuesUpTo]
                  rw [hvalue]
                  simp [evalStep, hn, hfirst_up, hsecond_up, hproduct]
      | eq first second =>
          simp [staticValue, staticValuesUpTo, staticStep, hn] at hstatic

/-- Specialization preserves graph well-formedness. -/
theorem specialize_wellFormed (graph : Graph) (σ : Nat → Option (BitVec 64))
    (hwf : WellFormed graph) : WellFormed (specialize graph σ) := by
  have hsubset : ∀ index operand,
      operand ∈ (specializeNode graph σ index).operands →
        operand ∈ (graph.node index).operands := by
    intro index operand
    cases hn : graph.node index with
    | input slot =>
        cases hs : σ slot <;> simp [specializeNode, hn, hs, Node.operands]
    | constant bits => simp [specializeNode, hn, Node.operands]
    | add first second =>
        cases hfirst : staticValue graph σ first <;>
          cases hsecond : staticValue graph σ second <;>
          simp [specializeNode, hn, hfirst, hsecond, Node.operands]
    | sub first second =>
        cases hfirst : staticValue graph σ first <;>
          cases hsecond : staticValue graph σ second <;>
          simp [specializeNode, hn, hfirst, hsecond, Node.operands]
    | mul first second =>
        cases hfirst : staticValue graph σ first <;>
          cases hsecond : staticValue graph σ second <;>
          simp [specializeNode, hn, hfirst, hsecond, Node.operands]
    | eq first second => simp [specializeNode, hn, Node.operands]
  refine ⟨hwf.1, ?_⟩
  intro index operand hmem
  apply hwf.2 index operand
  apply hsubset index operand
  exact hmem

/-- Congruence when the two graphs also use different input assignments. -/
theorem congruenceWithInputs (graph₁ graph₂ : Graph)
    (inputs₁ inputs₂ : Nat → BitVec 64) (hwf₁ : WellFormed graph₁)
    (hwf₂ : WellFormed graph₂) (index : Nat)
    (hnode : graph₁.node index = graph₂.node index)
    (hinput : ∀ slot, graph₁.node index = .input slot → inputs₁ slot = inputs₂ slot)
    (hoperands : ∀ operand ∈ (graph₁.node index).operands,
      value graph₁ inputs₁ operand = value graph₂ inputs₂ operand) :
    value graph₁ inputs₁ index = value graph₂ inputs₂ index := by
  have hvalue₁ : value graph₁ inputs₁ index =
      evalStep graph₁ (valuesUpTo graph₁ inputs₁ index) inputs₁ index := by
    simp [value, valuesUpTo]
  have hvalue₂ : value graph₂ inputs₂ index =
      evalStep graph₂ (valuesUpTo graph₂ inputs₂ index) inputs₂ index := by
    simp [value, valuesUpTo]
  rw [hvalue₁, hvalue₂]
  have hnode₂ : graph₂.node index = graph₁.node index := hnode.symm
  cases hn : graph₁.node index with
  | input slot =>
      have hslot := hinput slot hn
      simp [evalStep, hn, hnode₂, hslot]
  | constant bits =>
      simp [evalStep, hn, hnode₂]
  | add first second =>
      have hfirst : first ∈ (graph₁.node index).operands := by simp [hn, Node.operands]
      have hsecond : second ∈ (graph₁.node index).operands := by simp [hn, Node.operands]
      have hfirst₁ : valuesUpTo graph₁ inputs₁ index first =
          value graph₁ inputs₁ first :=
        valuesUpTo_eq_value graph₁ inputs₁ index first (hwf₁.2 index first hfirst)
      have hsecond₁ : valuesUpTo graph₁ inputs₁ index second =
          value graph₁ inputs₁ second :=
        valuesUpTo_eq_value graph₁ inputs₁ index second (hwf₁.2 index second hsecond)
      have hfirst₂ : valuesUpTo graph₂ inputs₂ index first =
          value graph₂ inputs₂ first := by
        rw [valuesUpTo_eq_value graph₂ inputs₂ index first]
        apply hwf₂.2 index first
        simpa [hnode₂] using hfirst
      have hsecond₂ : valuesUpTo graph₂ inputs₂ index second =
          value graph₂ inputs₂ second := by
        rw [valuesUpTo_eq_value graph₂ inputs₂ index second]
        apply hwf₂.2 index second
        simpa [hnode₂] using hsecond
      simp only [evalStep, hn, hnode₂]
      rw [hfirst₁, hsecond₁, hfirst₂, hsecond₂,
        hoperands first hfirst, hoperands second hsecond]
  | sub first second =>
      have hfirst : first ∈ (graph₁.node index).operands := by simp [hn, Node.operands]
      have hsecond : second ∈ (graph₁.node index).operands := by simp [hn, Node.operands]
      have hfirst₁ : valuesUpTo graph₁ inputs₁ index first =
          value graph₁ inputs₁ first :=
        valuesUpTo_eq_value graph₁ inputs₁ index first (hwf₁.2 index first hfirst)
      have hsecond₁ : valuesUpTo graph₁ inputs₁ index second =
          value graph₁ inputs₁ second :=
        valuesUpTo_eq_value graph₁ inputs₁ index second (hwf₁.2 index second hsecond)
      have hfirst₂ : valuesUpTo graph₂ inputs₂ index first =
          value graph₂ inputs₂ first := by
        rw [valuesUpTo_eq_value graph₂ inputs₂ index first]
        apply hwf₂.2 index first
        simpa [hnode₂] using hfirst
      have hsecond₂ : valuesUpTo graph₂ inputs₂ index second =
          value graph₂ inputs₂ second := by
        rw [valuesUpTo_eq_value graph₂ inputs₂ index second]
        apply hwf₂.2 index second
        simpa [hnode₂] using hsecond
      simp only [evalStep, hn, hnode₂]
      rw [hfirst₁, hsecond₁, hfirst₂, hsecond₂,
        hoperands first hfirst, hoperands second hsecond]
  | mul first second =>
      have hfirst : first ∈ (graph₁.node index).operands := by simp [hn, Node.operands]
      have hsecond : second ∈ (graph₁.node index).operands := by simp [hn, Node.operands]
      have hfirst₁ : valuesUpTo graph₁ inputs₁ index first =
          value graph₁ inputs₁ first :=
        valuesUpTo_eq_value graph₁ inputs₁ index first (hwf₁.2 index first hfirst)
      have hsecond₁ : valuesUpTo graph₁ inputs₁ index second =
          value graph₁ inputs₁ second :=
        valuesUpTo_eq_value graph₁ inputs₁ index second (hwf₁.2 index second hsecond)
      have hfirst₂ : valuesUpTo graph₂ inputs₂ index first =
          value graph₂ inputs₂ first := by
        rw [valuesUpTo_eq_value graph₂ inputs₂ index first]
        apply hwf₂.2 index first
        simpa [hnode₂] using hfirst
      have hsecond₂ : valuesUpTo graph₂ inputs₂ index second =
          value graph₂ inputs₂ second := by
        rw [valuesUpTo_eq_value graph₂ inputs₂ index second]
        apply hwf₂.2 index second
        simpa [hnode₂] using hsecond
      simp only [evalStep, hn, hnode₂]
      rw [hfirst₁, hsecond₁, hfirst₂, hsecond₂,
        hoperands first hfirst, hoperands second hsecond]
  | eq first second =>
      have hfirst : first ∈ (graph₁.node index).operands := by simp [hn, Node.operands]
      have hsecond : second ∈ (graph₁.node index).operands := by simp [hn, Node.operands]
      have hfirst₁ : valuesUpTo graph₁ inputs₁ index first =
          value graph₁ inputs₁ first :=
        valuesUpTo_eq_value graph₁ inputs₁ index first (hwf₁.2 index first hfirst)
      have hsecond₁ : valuesUpTo graph₁ inputs₁ index second =
          value graph₁ inputs₁ second :=
        valuesUpTo_eq_value graph₁ inputs₁ index second (hwf₁.2 index second hsecond)
      have hfirst₂ : valuesUpTo graph₂ inputs₂ index first =
          value graph₂ inputs₂ first := by
        rw [valuesUpTo_eq_value graph₂ inputs₂ index first]
        apply hwf₂.2 index first
        simpa [hnode₂] using hfirst
      have hsecond₂ : valuesUpTo graph₂ inputs₂ index second =
          value graph₂ inputs₂ second := by
        rw [valuesUpTo_eq_value graph₂ inputs₂ index second]
        apply hwf₂.2 index second
        simpa [hnode₂] using hsecond
      simp only [evalStep, hn, hnode₂]
      rw [hfirst₁, hsecond₁, hfirst₂, hsecond₂,
        hoperands first hfirst, hoperands second hsecond]

/-- Residual nodes are congruent even though their input assignments differ. -/
theorem residual_specialize_sound (graph₁ graph₂ : Graph)
    (inputs₁ inputs₂ : Nat → BitVec 64) (hwf₁ : WellFormed graph₁)
    (hwf₂ : WellFormed graph₂) (index : Nat)
    (hnode : graph₁.node index = graph₂.node index)
    (hnotInput : ∀ slot, graph₁.node index ≠ .input slot)
    (hoperands : ∀ operand ∈ (graph₁.node index).operands,
      value graph₁ inputs₁ operand = value graph₂ inputs₂ operand) :
    value graph₁ inputs₁ index = value graph₂ inputs₂ index := by
  apply congruenceWithInputs graph₁ graph₂ inputs₁ inputs₂ hwf₁ hwf₂ index hnode
  · intro slot hinput
    exact (hnotInput slot hinput).elim
  · exact hoperands

/-- Values of the specialized graph agree with the original graph under merge. -/
theorem specialize_sound (graph : Graph) (σ : Nat → Option (BitVec 64))
    (hwf : WellFormed graph) (d : Nat → BitVec 64) :
    ∀ index, value (specialize graph σ) d index = value graph (merge σ d) index := by
  have hspecwf : WellFormed (specialize graph σ) :=
    specialize_wellFormed graph σ hwf
  intro index
  induction index using Nat.strongRecOn with
  | _ index ih =>
      cases hn : graph.node index with
      | input slot =>
          cases hslot : σ slot with
          | none =>
              simp [specialize, specializeNode, hn, hslot, value, valuesUpTo, evalStep,
                merge]
          | some bits =>
              simp [specialize, specializeNode, hn, hslot, value, valuesUpTo, evalStep,
                merge]
      | constant bits =>
          simp [specialize, specializeNode, hn, value, valuesUpTo, evalStep]
      | add first second =>
          have hfirst_lt : first < index :=
            hwf.2 index first (by simp [hn, Node.operands])
          have hsecond_lt : second < index :=
            hwf.2 index second (by simp [hn, Node.operands])
          cases hfirst : staticValue graph σ first with
          | none =>
              cases hsecond : staticValue graph σ second with
              | none =>
                  have hspecnode : (specialize graph σ).node index = .add first second := by
                    simp [specialize, specializeNode, hn, hfirst, hsecond]
                  have hnotInput : ∀ slot, (specialize graph σ).node index ≠ .input slot := by
                    intro slot hinput
                    rw [hspecnode] at hinput
                    cases hinput
                  apply residual_specialize_sound (specialize graph σ) graph d (merge σ d)
                    hspecwf hwf index (by rw [hspecnode, hn]) hnotInput
                  intro operand hmem
                  have hmem' : operand ∈ (graph.node index).operands := by
                    simpa [hn, hspecnode, Node.operands] using hmem
                  exact ih operand (hwf.2 index operand hmem')
              | some secondBits =>
                  have hspecnode : (specialize graph σ).node index = .add first second := by
                    simp [specialize, specializeNode, hn, hfirst, hsecond]
                  have hnotInput : ∀ slot, (specialize graph σ).node index ≠ .input slot := by
                    intro slot hinput
                    rw [hspecnode] at hinput
                    cases hinput
                  apply residual_specialize_sound (specialize graph σ) graph d (merge σ d)
                    hspecwf hwf index (by rw [hspecnode, hn]) hnotInput
                  intro operand hmem
                  have hmem' : operand ∈ (graph.node index).operands := by
                    simpa [hn, hspecnode, Node.operands] using hmem
                  exact ih operand (hwf.2 index operand hmem')
          | some firstBits =>
              cases hsecond : staticValue graph σ second with
              | none =>
                  have hspecnode : (specialize graph σ).node index = .add first second := by
                    simp [specialize, specializeNode, hn, hfirst, hsecond]
                  have hnotInput : ∀ slot, (specialize graph σ).node index ≠ .input slot := by
                    intro slot hinput
                    rw [hspecnode] at hinput
                    cases hinput
                  apply residual_specialize_sound (specialize graph σ) graph d (merge σ d)
                    hspecwf hwf index (by rw [hspecnode, hn]) hnotInput
                  intro operand hmem
                  have hmem' : operand ∈ (graph.node index).operands := by
                    simpa [hn, hspecnode, Node.operands] using hmem
                  exact ih operand (hwf.2 index operand hmem')
              | some secondBits =>
                  have hfirst_value := staticValue_sound graph σ hwf d first firstBits hfirst
                  have hsecond_value := staticValue_sound graph σ hwf d second secondBits hsecond
                  have hfirst_up : valuesUpTo graph (merge σ d) index first =
                      some (.i64 firstBits) := by
                    rw [valuesUpTo_eq_value graph (merge σ d) index first hfirst_lt]
                    exact hfirst_value
                  have hsecond_up : valuesUpTo graph (merge σ d) index second =
                      some (.i64 secondBits) := by
                    rw [valuesUpTo_eq_value graph (merge σ d) index second hsecond_lt]
                    exact hsecond_value
                  have hvalue₁ : value (specialize graph σ) d index =
                      some (.i64 (firstBits + secondBits)) := by
                    simp [specialize, specializeNode, hn, hfirst, hsecond,
                      value, valuesUpTo, evalStep]
                  have hvalue₂ : value graph (merge σ d) index =
                      some (.i64 (firstBits + secondBits)) := by
                    have hvalue : value graph (merge σ d) index =
                        evalStep graph (valuesUpTo graph (merge σ d) index)
                          (merge σ d) index := by
                      simp [value, valuesUpTo]
                    rw [hvalue]
                    simp [evalStep, hn, hfirst_up, hsecond_up]
                  exact hvalue₁.trans hvalue₂.symm
      | sub first second =>
          have hfirst_lt : first < index :=
            hwf.2 index first (by simp [hn, Node.operands])
          have hsecond_lt : second < index :=
            hwf.2 index second (by simp [hn, Node.operands])
          cases hfirst : staticValue graph σ first with
          | none =>
              cases hsecond : staticValue graph σ second with
              | none =>
                  have hspecnode : (specialize graph σ).node index = .sub first second := by
                    simp [specialize, specializeNode, hn, hfirst, hsecond]
                  have hnotInput : ∀ slot, (specialize graph σ).node index ≠ .input slot := by
                    intro slot hinput
                    rw [hspecnode] at hinput
                    cases hinput
                  apply residual_specialize_sound (specialize graph σ) graph d (merge σ d)
                    hspecwf hwf index (by rw [hspecnode, hn]) hnotInput
                  intro operand hmem
                  have hmem' : operand ∈ (graph.node index).operands := by
                    simpa [hn, hspecnode, Node.operands] using hmem
                  exact ih operand (hwf.2 index operand hmem')
              | some secondBits =>
                  have hspecnode : (specialize graph σ).node index = .sub first second := by
                    simp [specialize, specializeNode, hn, hfirst, hsecond]
                  have hnotInput : ∀ slot, (specialize graph σ).node index ≠ .input slot := by
                    intro slot hinput
                    rw [hspecnode] at hinput
                    cases hinput
                  apply residual_specialize_sound (specialize graph σ) graph d (merge σ d)
                    hspecwf hwf index (by rw [hspecnode, hn]) hnotInput
                  intro operand hmem
                  have hmem' : operand ∈ (graph.node index).operands := by
                    simpa [hn, hspecnode, Node.operands] using hmem
                  exact ih operand (hwf.2 index operand hmem')
          | some firstBits =>
              cases hsecond : staticValue graph σ second with
              | none =>
                  have hspecnode : (specialize graph σ).node index = .sub first second := by
                    simp [specialize, specializeNode, hn, hfirst, hsecond]
                  have hnotInput : ∀ slot, (specialize graph σ).node index ≠ .input slot := by
                    intro slot hinput
                    rw [hspecnode] at hinput
                    cases hinput
                  apply residual_specialize_sound (specialize graph σ) graph d (merge σ d)
                    hspecwf hwf index (by rw [hspecnode, hn]) hnotInput
                  intro operand hmem
                  have hmem' : operand ∈ (graph.node index).operands := by
                    simpa [hn, hspecnode, Node.operands] using hmem
                  exact ih operand (hwf.2 index operand hmem')
              | some secondBits =>
                  have hfirst_value := staticValue_sound graph σ hwf d first firstBits hfirst
                  have hsecond_value := staticValue_sound graph σ hwf d second secondBits hsecond
                  have hfirst_up : valuesUpTo graph (merge σ d) index first =
                      some (.i64 firstBits) := by
                    rw [valuesUpTo_eq_value graph (merge σ d) index first hfirst_lt]
                    exact hfirst_value
                  have hsecond_up : valuesUpTo graph (merge σ d) index second =
                      some (.i64 secondBits) := by
                    rw [valuesUpTo_eq_value graph (merge σ d) index second hsecond_lt]
                    exact hsecond_value
                  have hvalue₁ : value (specialize graph σ) d index =
                      some (.i64 (firstBits - secondBits)) := by
                    simp [specialize, specializeNode, hn, hfirst, hsecond,
                      value, valuesUpTo, evalStep]
                  have hvalue₂ : value graph (merge σ d) index =
                      some (.i64 (firstBits - secondBits)) := by
                    have hvalue : value graph (merge σ d) index =
                        evalStep graph (valuesUpTo graph (merge σ d) index)
                          (merge σ d) index := by
                      simp [value, valuesUpTo]
                    rw [hvalue]
                    simp [evalStep, hn, hfirst_up, hsecond_up]
                  exact hvalue₁.trans hvalue₂.symm
      | mul first second =>
          have hfirst_lt : first < index :=
            hwf.2 index first (by simp [hn, Node.operands])
          have hsecond_lt : second < index :=
            hwf.2 index second (by simp [hn, Node.operands])
          cases hfirst : staticValue graph σ first with
          | none =>
              cases hsecond : staticValue graph σ second with
              | none =>
                  have hspecnode : (specialize graph σ).node index = .mul first second := by
                    simp [specialize, specializeNode, hn, hfirst, hsecond]
                  have hnotInput : ∀ slot, (specialize graph σ).node index ≠ .input slot := by
                    intro slot hinput
                    rw [hspecnode] at hinput
                    cases hinput
                  apply residual_specialize_sound (specialize graph σ) graph d (merge σ d)
                    hspecwf hwf index (by rw [hspecnode, hn]) hnotInput
                  intro operand hmem
                  have hmem' : operand ∈ (graph.node index).operands := by
                    simpa [hn, hspecnode, Node.operands] using hmem
                  exact ih operand (hwf.2 index operand hmem')
              | some secondBits =>
                  have hspecnode : (specialize graph σ).node index = .mul first second := by
                    simp [specialize, specializeNode, hn, hfirst, hsecond]
                  have hnotInput : ∀ slot, (specialize graph σ).node index ≠ .input slot := by
                    intro slot hinput
                    rw [hspecnode] at hinput
                    cases hinput
                  apply residual_specialize_sound (specialize graph σ) graph d (merge σ d)
                    hspecwf hwf index (by rw [hspecnode, hn]) hnotInput
                  intro operand hmem
                  have hmem' : operand ∈ (graph.node index).operands := by
                    simpa [hn, hspecnode, Node.operands] using hmem
                  exact ih operand (hwf.2 index operand hmem')
          | some firstBits =>
              cases hsecond : staticValue graph σ second with
              | none =>
                  have hspecnode : (specialize graph σ).node index = .mul first second := by
                    simp [specialize, specializeNode, hn, hfirst, hsecond]
                  have hnotInput : ∀ slot, (specialize graph σ).node index ≠ .input slot := by
                    intro slot hinput
                    rw [hspecnode] at hinput
                    cases hinput
                  apply residual_specialize_sound (specialize graph σ) graph d (merge σ d)
                    hspecwf hwf index (by rw [hspecnode, hn]) hnotInput
                  intro operand hmem
                  have hmem' : operand ∈ (graph.node index).operands := by
                    simpa [hn, hspecnode, Node.operands] using hmem
                  exact ih operand (hwf.2 index operand hmem')
              | some secondBits =>
                  have hfirst_value := staticValue_sound graph σ hwf d first firstBits hfirst
                  have hsecond_value := staticValue_sound graph σ hwf d second secondBits hsecond
                  have hfirst_up : valuesUpTo graph (merge σ d) index first =
                      some (.i64 firstBits) := by
                    rw [valuesUpTo_eq_value graph (merge σ d) index first hfirst_lt]
                    exact hfirst_value
                  have hsecond_up : valuesUpTo graph (merge σ d) index second =
                      some (.i64 secondBits) := by
                    rw [valuesUpTo_eq_value graph (merge σ d) index second hsecond_lt]
                    exact hsecond_value
                  have hvalue₁ : value (specialize graph σ) d index =
                      some (.i64 (firstBits * secondBits)) := by
                    simp [specialize, specializeNode, hn, hfirst, hsecond,
                      value, valuesUpTo, evalStep]
                  have hvalue₂ : value graph (merge σ d) index =
                      some (.i64 (firstBits * secondBits)) := by
                    have hvalue : value graph (merge σ d) index =
                        evalStep graph (valuesUpTo graph (merge σ d) index)
                          (merge σ d) index := by
                      simp [value, valuesUpTo]
                    rw [hvalue]
                    simp [evalStep, hn, hfirst_up, hsecond_up]
                  exact hvalue₁.trans hvalue₂.symm
      | eq first second =>
          have hspecnode : (specialize graph σ).node index = .eq first second := by
            simp [specialize, specializeNode, hn]
          have hnotInput : ∀ slot, (specialize graph σ).node index ≠ .input slot := by
            intro slot hinput
            rw [hspecnode] at hinput
            cases hinput
          apply residual_specialize_sound (specialize graph σ) graph d (merge σ d)
            hspecwf hwf index (by rw [hspecnode, hn]) hnotInput
          intro operand hmem
          have hmem' : operand ∈ (graph.node index).operands := by
            simpa [hn, hspecnode, Node.operands] using hmem
          exact ih operand (hwf.2 index operand hmem')

/-! ### Fixtures and controls -/

def noStatic : Nat → Option (BitVec 64) := fun _ => none

def allStatic : Nat → Option (BitVec 64)
  | 0 => some 3
  | 1 => some 5
  | _ => none

def mixedStatic : Nat → Option (BitVec 64)
  | 0 => some 3
  | _ => none

def specializationGraph : Graph where
  arity := 5
  output := 4
  node := fun index =>
    if index = 0 then .input 0
    else if index = 1 then .input 1
    else if index = 2 then .add 0 1
    else if index = 3 then .sub 2 0
    else .mul 3 1

theorem specializationGraph_wellFormed : WellFormed specializationGraph := by
  refine ⟨by decide, ?_⟩
  intro index operand hmem
  by_cases h0 : index = 0
  · subst h0; simp [specializationGraph, Node.operands] at hmem
  by_cases h1 : index = 1
  · subst h1; simp [specializationGraph, Node.operands] at hmem
  by_cases h2 : index = 2
  · subst h2; simp [specializationGraph, Node.operands] at hmem; omega
  by_cases h3 : index = 3
  · subst h3; simp [specializationGraph, Node.operands] at hmem; omega
  · simp [specializationGraph, h0, h1, h2, h3, Node.operands] at hmem; omega

theorem no_static_fixture :
    specializeNode specializationGraph noStatic 0 = .input 0 ∧
      specializeNode specializationGraph noStatic 2 = .add 0 1 := by
  decide

theorem all_static_fixture :
    specializeNode specializationGraph allStatic 0 = .constant 3 ∧
      specializeNode specializationGraph allStatic 2 = .constant 8 ∧
      specializeNode specializationGraph allStatic 4 = .constant 25 := by
  decide

theorem mixed_static_fixture :
    specializeNode specializationGraph mixedStatic 0 = .constant 3 ∧
      specializeNode specializationGraph mixedStatic 1 = .input 0 ∧
      specializeNode specializationGraph mixedStatic 2 = .add 0 1 ∧
      specializeNode specializationGraph mixedStatic 3 = .sub 2 0 := by
  decide

def nonPrefixStatic : Nat → Option (BitVec 64)
  | 2 => some 100
  | _ => none

def nonPrefixGraph : Graph where
  arity := 5
  output := 4
  node := fun index =>
    if index = 0 then .input 0
    else if index = 1 then .input 1
    else if index = 2 then .input 2
    else if index = 3 then .add 0 1
    else .add 3 2

theorem nonPrefixGraph_wellFormed : WellFormed nonPrefixGraph := by
  refine ⟨by decide, ?_⟩
  intro index operand hmem
  by_cases h0 : index = 0
  · subst h0; simp [nonPrefixGraph, Node.operands] at hmem
  by_cases h1 : index = 1
  · subst h1; simp [nonPrefixGraph, Node.operands] at hmem
  by_cases h2 : index = 2
  · subst h2; simp [nonPrefixGraph, Node.operands] at hmem
  by_cases h3 : index = 3
  · subst h3; simp [nonPrefixGraph, Node.operands] at hmem; omega
  · simp [nonPrefixGraph, h0, h1, h2, h3, Node.operands] at hmem; omega

theorem non_prefix_static_fixture :
    rank nonPrefixStatic 0 = 0 ∧ rank nonPrefixStatic 1 = 1 ∧
      rank nonPrefixStatic 2 = 2 ∧
      specializeNode nonPrefixGraph nonPrefixStatic 0 = .input 0 ∧
      specializeNode nonPrefixGraph nonPrefixStatic 1 = .input 1 ∧
      specializeNode nonPrefixGraph nonPrefixStatic 2 = .constant 100 := by
  decide

def prefixMerge (staticCount : Nat) (σ : Nat → Option (BitVec 64))
    (d : Nat → BitVec 64) (slot : Nat) : BitVec 64 :=
  match σ slot with
  | some value => value
  | none => d (slot - staticCount)

def subtractionGraph : Graph where
  arity := 3
  output := 2
  node := fun index =>
    if index = 0 then .input 0
    else if index = 1 then .input 1
    else .sub 0 1

theorem deeper_minus_top_fixture :
    value (specialize subtractionGraph noStatic)
      (fun slot => if slot = 0 then 9 else 4) 2 = some (.i64 5) := by
  decide

def wrapGraph : Graph where
  arity := 3
  output := 2
  node := fun index =>
    if index = 0 then .constant (BitVec.ofNat 64 (2 ^ 64 - 1))
    else if index = 1 then .constant 1
    else .add 0 1

theorem wrapGraph_wellFormed : WellFormed wrapGraph := by
  refine ⟨by decide, ?_⟩
  intro index operand hmem
  by_cases h0 : index = 0
  · subst h0; simp [wrapGraph, Node.operands] at hmem
  by_cases h1 : index = 1
  · subst h1; simp [wrapGraph, Node.operands] at hmem
  · simp [wrapGraph, h0, h1, Node.operands] at hmem; omega

theorem wrap_boundary_fixture :
    specializeNode wrapGraph noStatic 2 =
        .constant (BitVec.ofNat 64 (2 ^ 64 - 1) + 1) ∧
      value (specialize wrapGraph noStatic) (fun _ => 0) 2 = some (.i64 0) := by
  decide

theorem specialize_wellFormed_fixture :
    WellFormed (specialize specializationGraph mixedStatic) :=
  specialize_wellFormed specializationGraph mixedStatic specializationGraph_wellFormed

theorem specialize_sound_fixture :
    value (specialize specializationGraph mixedStatic) (fun _ => 7) 4 =
      value specializationGraph (merge mixedStatic (fun _ => 7)) 4 := by
  exact specialize_sound specializationGraph mixedStatic specializationGraph_wellFormed
    (fun _ => 7) 4

theorem prefix_merge_control :
    value nonPrefixGraph (merge nonPrefixStatic (fun slot => if slot = 0 then 4 else 9)) 4 ≠
      value nonPrefixGraph
        (prefixMerge 1 nonPrefixStatic (fun slot => if slot = 0 then 4 else 9)) 4 := by
  decide

end NobleFoundation.Specialize
