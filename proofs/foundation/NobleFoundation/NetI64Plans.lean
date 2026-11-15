import NobleFoundation.NetI64

/-!
# Checked revision-3 plan semantics (issue #110)

This is an independent kernel model over the vendored inets hash-map/worklist
machine. It models the plan's captured neighbourhood and result/write image,
then stutters observably through the pre-commit phases and commits one inets
firing. It does not claim correspondence with the Rust byte compiler or the
Rust generation-tagged vector arena; finite Rust differential tests remain a
separate evidence class.
-/

namespace NobleFoundation.NetI64Plans

open INet
open NobleFoundation.NetI64

/-- The finite output image produced from one exact revision-3 interaction rule. -/
structure RulePlan where
  left : NetSym
  right : NetSym
  rhs : XRhs NetSym
  results : List (((Option Bool × Nat) × NetSym))
  commitPoint : Nat

/-- Compile the selected revision-3 rule table into its result-agent image. -/
def compileRule (left right : NetSym) : Option RulePlan :=
  (NetI64.xRule left right).map fun rhs =>
    { left := left
      right := right
      rhs := rhs
      results := rhs.slots.filterMap fun slot =>
        (rhs.lab slot.1 slot.2).map fun symbol => (slot, symbol)
      commitPoint := 6 }

/-- A rule-table entry is preserved exactly by the plan-rule compiler. -/
theorem compileRule_exact_rhs {left right : NetSym} {rhs : XRhs NetSym}
    (h : NetI64.xRule left right = some rhs) :
    (compileRule left right).map RulePlan.rhs = some rhs := by
  simp [compileRule, h]

/-- Every operation and payload admits the forward OP2 plan image. -/
theorem compileRule_op2_forward (op : Op) (first : BitVec 64) :
    (compileRule (.op2 op) (.num first)).isSome = true := by
  cases op <;> simp [compileRule, NetI64.xRule]

/-- Every operation and payload admits the mirrored OP2 plan image. -/
theorem compileRule_op2_reverse (op : Op) (first : BitVec 64) :
    (compileRule (.num first) (.op2 op)).isSome = true := by
  cases op <;> simp [compileRule, NetI64.xRule]

/-- Every operation and pair of values admits the forward OP1 plan image. -/
theorem compileRule_op1_forward (op : Op) (first second : BitVec 64) :
    (compileRule (.op1 op first) (.num second)).isSome = true := by
  cases op <;> simp [compileRule, NetI64.xRule]

/-- Every operation and pair of values admits the mirrored OP1 plan image. -/
theorem compileRule_op1_reverse (op : Op) (first second : BitVec 64) :
    (compileRule (.num second) (.op1 op first)).isSome = true := by
  cases op <;> simp [compileRule, NetI64.xRule]

/-- A DUP/number pair admits the forward duplication plan image. -/
theorem compileRule_dup_forward (value : BitVec 64) :
    (compileRule .dup (.num value)).isSome = true := by
  simp [compileRule, NetI64.xRule]

/-- A number/DUP pair admits the mirrored duplication plan image. -/
theorem compileRule_dup_reverse (value : BitVec 64) :
    (compileRule (.num value) .dup).isSome = true := by
  simp [compileRule, NetI64.xRule]

/-- An ERA/number pair admits the forward erasure plan image. -/
theorem compileRule_era_forward (value : BitVec 64) :
    (compileRule .era (.num value)).isSome = true := by
  simp [compileRule, NetI64.xRule]

/-- A number/ERA pair admits the mirrored erasure plan image. -/
theorem compileRule_era_reverse (value : BitVec 64) :
    (compileRule (.num value) .era).isSome = true := by
  simp [compileRule, NetI64.xRule]

/-- The finite executable result for a selected pair in the inets worklist model. -/
def localResult (H : HNet NetSym) (r : XRedex NetSym) : XNet NetSym :=
  let a := r.1
  let b := r.2.1
  let α := r.2.2.1
  let β := r.2.2.2.1
  let R := r.2.2.2.2
  (H.nbhd NetI64.arity a b α β).fire NetI64.arity a b α β R

/-- A checked plan image captured from the inets hash-map/worklist state. -/
structure Plan where
  redex : XRedex NetSym
  ruleValid : NetI64.xRule redex.2.2.1 redex.2.2.2.1 = some redex.2.2.2.2
  capturedBoundary : List (Port × Port)
  initializedAgents : List (Id × NetSym)
  wireWrites : List (Port × Port)
  newlyActive : List (Id × Id)
  arenaReservation : Nat
  scratchReservation : Nat
  worklistReservation : Nat
  commitPoint : Nat

/-- Capture the local neighbourhood, result agents, wire image, publications and bounds.
The plan phases refine the committed net; physical Rust slot reuse is intentionally
outside this model. -/
def Plan.compile (H : HNet NetSym) (r : XRedex NetSym)
    (hRule : NetI64.xRule r.2.2.1 r.2.2.2.1 = some r.2.2.2.2) : Plan :=
  let a := r.1
  let b := r.2.1
  let α := r.2.2.1
  let β := r.2.2.2.1
  let fired := localResult H r
  let boundary := H.nearWires NetI64.arity a b α β
  let published := principalPairs fired.wires
  { redex := r
    ruleValid := hRule
    capturedBoundary := boundary
    initializedAgents := fired.agents
    wireWrites := fired.wires
    newlyActive := published
    arenaReservation := r.2.2.2.2.slots.length
    scratchReservation := boundary.length + fired.wires.length
    worklistReservation := published.length
    commitPoint := 6 }

/-- Commit the image produced by a plan: erase both directions of its captured old
boundary, retire the consumed pair, install initialized agents and declared wires,
and publish its checked active-pair list. -/
def Plan.commit (H : HNet NetSym) (p : Plan) (rest : List (Id × Id)) : HNet NetSym :=
  let oldWires := p.capturedBoundary ++ p.capturedBoundary.map fun edge => (edge.2, edge.1)
  let retiredWires := oldWires.foldl (fun wires edge => wires.erase edge.1) H.wires
  let agents := p.initializedAgents.foldl (fun agents edge => agents.insert edge.1 edge.2)
    ((H.agents.erase p.redex.1).erase p.redex.2.1)
  let wires := p.wireWrites.foldl (fun wires edge => wires.insert edge.1 edge.2) retiredWires
  { agents := agents
    wires := wires
    free := H.free
    work := p.newlyActive ++ rest }

/-- A sequence of map inserts leaves keys outside the declared image unchanged. -/
private theorem insert_frame {K V : Type} [BEq K] [LawfulBEq K] [Hashable K] [DecidableEq K]
    (entries : List (K × V)) (base : Std.HashMap K V) (key : K)
    (outside : ∀ entry ∈ entries, entry.1 ≠ key) :
    (entries.foldl (fun m entry => m.insert entry.1 entry.2) base)[key]? = base[key]? := by
  induction entries generalizing base with
  | nil => rfl
  | cons entry rest ih =>
    have outsideRest : ∀ e ∈ rest, e.1 ≠ key := by
      intro e he
      exact outside e (List.mem_cons_of_mem _ he)
    simp only [List.foldl_cons]
    rw [ih (base := base.insert entry.1 entry.2) outsideRest, hget_insert]
    simp [outside entry (by simp)]

/-- A sequence of map erases leaves keys outside the captured boundary unchanged. -/
private theorem erase_frame {K V : Type} [BEq K] [LawfulBEq K] [Hashable K] [DecidableEq K]
    (entries : List (K × V)) (base : Std.HashMap K V) (key : K)
    (outside : ∀ entry ∈ entries, entry.1 ≠ key) :
    (entries.foldl (fun m entry => m.erase entry.1) base)[key]? = base[key]? := by
  induction entries generalizing base with
  | nil => rfl
  | cons entry rest ih =>
    have outsideRest : ∀ e ∈ rest, e.1 ≠ key := by
      intro e he
      exact outside e (List.mem_cons_of_mem _ he)
    simp only [List.foldl_cons]
    rw [ih (base := base.erase entry.1) outsideRest, hget_erase]
    simp [outside entry (by simp)]

/-- A plan leaves every agent outside the consumed pair and the result image untouched. -/
theorem Plan.commit_frame_agent (H : HNet NetSym) (p : Plan)
    (rest : List (Id × Id)) (id : Id)
    (left : id ≠ p.redex.1) (right : id ≠ p.redex.2.1)
    (outside : ∀ entry ∈ p.initializedAgents, entry.1 ≠ id) :
    (Plan.commit H p rest).agents[id]? = H.agents[id]? := by
  change (p.initializedAgents.foldl (fun m entry => m.insert entry.1 entry.2)
    ((H.agents.erase p.redex.1).erase p.redex.2.1))[id]? = H.agents[id]?
  rw [insert_frame _ _ _ outside, hget_erase, hget_erase]
  simp [Ne.symm left, Ne.symm right]

/-- A plan leaves every wire outside its captured boundary and declared writes untouched. -/
theorem Plan.commit_frame_wire (H : HNet NetSym) (p : Plan)
    (rest : List (Id × Id)) (port : Port)
    (oldOutside : ∀ entry ∈ p.capturedBoundary ++
      p.capturedBoundary.map (fun edge => (edge.2, edge.1)), entry.1 ≠ port)
    (newOutside : ∀ entry ∈ p.wireWrites, entry.1 ≠ port) :
    (Plan.commit H p rest).wires[port]? = H.wires[port]? := by
  change (p.wireWrites.foldl (fun m entry => m.insert entry.1 entry.2)
    ((p.capturedBoundary ++ p.capturedBoundary.map (fun edge => (edge.2, edge.1))).foldl
      (fun m entry => m.erase entry.1) H.wires))[port]? = H.wires[port]?
  rw [insert_frame _ _ _ newOutside, erase_frame _ _ _ oldOutside]

/-- Executing a compiled plan image is exactly the corresponding inets worklist firing. -/
theorem Plan.compile_commit (H : HNet NetSym) (r : XRedex NetSym)
    (hRule : NetI64.xRule r.2.2.1 r.2.2.2.1 = some r.2.2.2.2)
    (rest : List (Id × Id)) :
    Plan.commit H (Plan.compile H r hRule) rest =
      H.fire NetI64.xsys.sys.ar r.1 r.2.1 r.2.2.1 r.2.2.2.1 r.2.2.2.2 rest := by
  rfl

/-- The worklist published by the commit is exactly the captured local active-pair image. -/
theorem Plan.commit_worklist (H : HNet NetSym) (r : XRedex NetSym)
    (hRule : NetI64.xRule r.2.2.1 r.2.2.2.1 = some r.2.2.2.2)
    (rest : List (Id × Id)) :
    (Plan.commit H (Plan.compile H r hRule) rest).work =
      (Plan.compile H r hRule).newlyActive ++ rest := by
  rfl

/-- The actual vector queue appends newly discovered pairs after the pending
entries; inets' abstract worklist prepends them. They carry the same multiset,
not the same selection order. -/
theorem Plan.commit_worklist_permutation (H : HNet NetSym)
    (r : XRedex NetSym)
    (hRule : NetI64.xRule r.2.2.1 r.2.2.2.1 = some r.2.2.2.2)
    (rest : List (Id × Id)) :
    (rest ++ (Plan.compile H r hRule).newlyActive).Perm
      (Plan.commit H (Plan.compile H r hRule) rest).work := by
  rw [Plan.commit_worklist]
  exact List.perm_append_comm

/-- Before the explicit commit point, every plan phase leaves the abstract net unchanged. -/
inductive Phase where
  | captured
  | reserved
  | initialized
  | rewired
  | retired
  | published
  | committed
  deriving DecidableEq

def observe (H : HNet NetSym) (p : Plan) (rest : List (Id × Id)) : Phase → HNet NetSym
  | .captured => H
  | .reserved => H
  | .initialized => H
  | .rewired => H
  | .retired => H
  | .published => H
  | .committed => Plan.commit H p rest

/-- The six pre-commit microsteps are observational stutters. -/
theorem Plan.precommit_stutters (H : HNet NetSym) (p : Plan)
    (rest : List (Id × Id)) (phase : Phase) (h : phase ≠ .committed) :
    (observe H p rest phase).toNet = H.toNet := by
  cases phase with
  | captured => rfl
  | reserved => rfl
  | initialized => rfl
  | rewired => rfl
  | retired => rfl
  | published => rfl
  | committed => exact (h rfl).elim

/-- A checked plan commit is exactly one abstract revision-3 net reduction step. -/
theorem Plan.commit_refines {H : HNet NetSym} (r : XRedex NetSym)
    (hRule : NetI64.xRule r.2.2.1 r.2.2.2.1 = some r.2.2.2.2)
    (rest : List (Id × Id))
    (hm : H.toNet.Matching NetI64.arity)
    (hredex : Redex NetI64.system H.toNet
      r.1 r.2.1 r.2.2.1 r.2.2.2.1 r.2.2.2.2.toRhs) :
    CStep NetI64.system H.toNet
      (Plan.commit H (Plan.compile H r hRule) rest).toNet := by
  rw [Plan.compile_commit]
  refine ⟨H.toNet.fire r.1 r.2.1 r.2.2.2.2.toRhs, ?_, ?_⟩
  · exact ⟨r.1, r.2.1, r.2.2.1, r.2.2.2.1,
      r.2.2.2.2.toRhs, hredex, rfl⟩
  · simpa [NetI64.xsys] using
      (HNet.fire_toNet (xs := NetI64.xsys) hm hredex hRule rest).1

/-- The selected `ngraph-net-plan/1` on-wire image. Integers are little-endian;
the final byte is the commit marker, not another executable operation. -/
structure BytePlan where
  reuse : Bool
  opcode : UInt8
  pair : Nat × Nat × Nat × Nat
  action : UInt8 × UInt8
  boundaries : List ((Nat × Nat) × (Nat × Nat × Nat))
  results : List (Bool × Nat × NetSym × Nat × List (Option Nat))
  writes : List (Nat × Nat × Nat)
  externalWrites : List (Nat × Nat × Nat)
  arenaReservation : Nat
  scratchReservation : Nat
  worklistReservation : Nat
  publications : List (Nat × Nat × Nat × Nat)
  deriving DecidableEq

private def readByte : List UInt8 → Option (UInt8 × List UInt8)
  | [] => none
  | head :: tail => some (head, tail)

/-- Read an unsigned little-endian integer of exactly `size` bytes. -/
private def readWord : Nat → List UInt8 → Option (Nat × List UInt8)
  | 0, bytes => some (0, bytes)
  | size + 1, bytes => do
      let (digit, rest) ← readByte bytes
      let (high, tail) ← readWord size rest
      some (digit.toNat + 256 * high, tail)

/-- The same least-significant-byte-first word layout as the plan writer. -/
private def emitWord : Nat → Nat → List UInt8
  | 0, _ => []
  | size + 1, value =>
      UInt8.ofNat (value % 256) :: emitWord size (value / 256)

/-- The byte reader recovers every representable word and leaves the following
bytes untouched; this holds for arbitrary 32-bit indices and 64-bit payloads. -/
theorem BytePlan.readWord_emitWord (size value : Nat) (rest : List UInt8)
    (bound : value < 256 ^ size) :
    readWord size (emitWord size value ++ rest) = some (value, rest) := by
  induction size generalizing value with
  | zero =>
    have hz : value = 0 := by simpa using bound
    subst value
    rfl
  | succ size ih =>
    have hdiv : value / 256 < 256 ^ size := by
      rw [Nat.pow_succ] at bound
      omega
    simp [emitWord, readWord, readByte, ih (value / 256) hdiv,
      UInt8.toNat_ofNat']
    omega

private def readMany {T : Type} (count : Nat)
    (reader : List UInt8 → Option (T × List UInt8)) :
    List UInt8 → Option (List T × List UInt8)
  | bytes => match count with
    | 0 => some ([], bytes)
    | count + 1 => do
        let (item, rest) ← reader bytes
        let (items, tail) ← readMany count reader rest
        some (item :: items, tail)

/-- An exact byte count consumes precisely that prefix and preserves its
following stream; this is used for fixed three-port result arrays. -/
theorem BytePlan.readMany_bytes (bytes rest : List UInt8) :
    readMany bytes.length readByte (bytes ++ rest) = some (bytes, rest) := by
  induction bytes with
  | nil => rfl
  | cons byte tail ih =>
    simp [readMany, readByte, ih]

private theorem readMany_emit {T : Type}
    (reader : List UInt8 → Option (T × List UInt8))
    (emit : T → List UInt8)
    (items : List T) (rest : List UInt8)
    (sound : ∀ item ∈ items, ∀ rest, reader (emit item ++ rest) = some (item, rest)) :
    readMany items.length reader (items.flatMap emit ++ rest) =
      some (items, rest) := by
  induction items with
  | nil => rfl
  | cons item tail ih =>
    have hhead := sound item (by simp) (tail.flatMap emit ++ rest)
    have htail : ∀ entry ∈ tail, ∀ suffix,
        reader (emit entry ++ suffix) = some (entry, suffix) := by
      intro entry he suffix
      exact sound entry (List.mem_cons_of_mem _ he) suffix
    simp [readMany, List.flatMap_cons, List.append_assoc, hhead, ih htail]

private def readCounted {T : Type} (limit : Nat)
    (reader : List UInt8 → Option (T × List UInt8))
    (bytes : List UInt8) : Option (List T × List UInt8) := do
  let (count, rest) ← readByte bytes
  if count.toNat ≤ limit then readMany count.toNat reader rest else none

private theorem readCounted_emit {T : Type}
    (limit : Nat) (reader : List UInt8 → Option (T × List UInt8))
    (emit : T → List UInt8)
    (items : List T) (rest : List UInt8)
    (sound : ∀ item ∈ items, ∀ suffix,
      reader (emit item ++ suffix) = some (item, suffix))
    (bound : items.length ≤ limit) (byteBound : items.length < UInt8.size) :
    readCounted limit reader
      (UInt8.ofNat items.length :: (items.flatMap emit ++ rest)) =
      some (items, rest) := by
  simp [readCounted, readByte, UInt8.toNat_ofNat_of_lt' byteBound,
    bound, readMany_emit reader emit items rest sound]

private def decodeSymbol (tag : UInt8) (payload : Nat) : Option NetSym :=
  match tag.toNat with
  | 0 => some (.num (BitVec.ofNat 64 payload))
  | 1 => if payload == 0 then some (.op2 .add) else none
  | 2 => if payload == 0 then some (.op2 .sub) else none
  | 3 => if payload == 0 then some (.op2 .mul) else none
  | 4 => if payload == 0 then some (.op2 .eq) else none
  | 5 => some (.op1 .add (BitVec.ofNat 64 payload))
  | 6 => some (.op1 .sub (BitVec.ofNat 64 payload))
  | 7 => some (.op1 .mul (BitVec.ofNat 64 payload))
  | 8 => some (.op1 .eq (BitVec.ofNat 64 payload))
  | 9 => if payload == 0 then some .dup else none
  | 10 => if payload == 0 then some .era else none
  | _ => none

private def opTag : Op → Nat
  | .add => 0
  | .sub => 1
  | .mul => 2
  | .eq => 3

private def emitSymbol : NetSym → UInt8 × Nat
  | .num value => (0, value.toNat)
  | .op2 op => (UInt8.ofNat (1 + opTag op), 0)
  | .op1 op first => (UInt8.ofNat (5 + opTag op), first.toNat)
  | .dup => (9, 0)
  | .era => (10, 0)

/-- Every selected revision-3 result symbol has the same tag and payload on
the emitted byte side and the independent parser side, for every I64 value. -/
theorem BytePlan.decodeSymbol_emitSymbol (symbol : NetSym) :
    decodeSymbol (emitSymbol symbol).1 (emitSymbol symbol).2 = some symbol := by
  cases symbol with
  | num value => simp [emitSymbol, decodeSymbol]
  | op2 op => cases op <;> rfl
  | op1 op first => cases op <;> simp [emitSymbol, decodeSymbol, opTag]
  | dup => rfl
  | era => rfl

/-- All emitted numeric symbols fit the exact eight-byte payload without
truncation, including arbitrary staged first operands. -/
theorem BytePlan.emitSymbol_payload_bounded (symbol : NetSym) :
    (emitSymbol symbol).2 < 256 ^ 8 := by
  have width : (256 : Nat) ^ 8 = 2 ^ 64 := by decide
  cases symbol with
  | num value => simpa [emitSymbol, width] using value.isLt
  | op2 op => simp [emitSymbol]
  | op1 op first => simpa [emitSymbol, width] using first.isLt
  | dup => simp [emitSymbol]
  | era => simp [emitSymbol]

private def readEndpoint (bytes : List UInt8) : Option ((Nat × Nat × Nat) × List UInt8) := do
  let (tag, rest) ← readByte bytes
  if tag == 0 then do
    let (slot, rest) ← readWord 4 rest
    let (port, rest) ← readByte rest
    some ((0, slot, port.toNat), rest)
  else if tag == 1 then do
    let (output, rest) ← readWord 4 rest
    some ((1, output, 0), rest)
  else none

private def emitEndpoint : Nat × Nat × Nat → List UInt8
  | (0, slot, port) => 0 :: (emitWord 4 slot ++ [UInt8.ofNat port])
  | (1, output, _) => 1 :: emitWord 4 output
  | _ => []

private def validEndpoint (endpoint : Nat × Nat × Nat) : Prop :=
  (endpoint.1 = 0 ∧ endpoint.2.1 < 256 ^ 4 ∧ endpoint.2.2 < UInt8.size) ∨
  (endpoint.1 = 1 ∧ endpoint.2.1 < 256 ^ 4 ∧ endpoint.2.2 = 0)

/-- The two admitted external endpoint tags (agent port or ordered output)
round-trip without losing the port byte or output index. -/
theorem BytePlan.readEndpoint_emitEndpoint (endpoint : Nat × Nat × Nat)
    (rest : List UInt8) (valid : validEndpoint endpoint) :
    readEndpoint (emitEndpoint endpoint ++ rest) = some (endpoint, rest) := by
  rcases endpoint with ⟨tag, slot, port⟩
  rcases valid with ⟨h, hs, hp⟩ | ⟨h, hs, hp⟩
  · change tag = 0 at h
    subst tag
    simp [emitEndpoint, readEndpoint, readByte, List.append_assoc,
      BytePlan.readWord_emitWord, hs, UInt8.toNat_ofNat_of_lt' hp]
  · change tag = 1 at h
    subst tag
    change port = 0 at hp
    subst port
    simp [emitEndpoint, readEndpoint, readByte,
      BytePlan.readWord_emitWord, hs]

private def readBoundary (bytes : List UInt8) :
    Option (((Nat × Nat) × (Nat × Nat × Nat)) × List UInt8) := do
  let (slot, rest) ← readWord 4 bytes
  let (port, rest) ← readByte rest
  let (endpoint, rest) ← readEndpoint rest
  some (((slot, port.toNat), endpoint), rest)

private def emitBoundary
    (boundary : (Nat × Nat) × (Nat × Nat × Nat)) : List UInt8 :=
  emitWord 4 boundary.1.1 ++
    [UInt8.ofNat boundary.1.2] ++ emitEndpoint boundary.2

/-- A captured external boundary tuple preserves its source port and exact
endpoint, for arbitrary physical indices within the encoded widths. -/
theorem BytePlan.readBoundary_emitBoundary
    (boundary : (Nat × Nat) × (Nat × Nat × Nat)) (rest : List UInt8)
    (hslot : boundary.1.1 < 256 ^ 4)
    (hport : boundary.1.2 < UInt8.size)
    (hendpoint : validEndpoint boundary.2) :
    readBoundary (emitBoundary boundary ++ rest) = some (boundary, rest) := by
  rcases boundary with ⟨⟨slot, port⟩, endpoint⟩
  simp [emitBoundary, readBoundary, readByte, List.append_assoc,
    BytePlan.readWord_emitWord, hslot, UInt8.toNat_ofNat_of_lt' hport,
    BytePlan.readEndpoint_emitEndpoint, hendpoint]

private def emitResultPort (port : Option Nat) : UInt8 :=
  UInt8.ofNat (port.getD 255)

private def decodeResultPort (port : UInt8) : Option Nat :=
  if port.toNat == 255 then none else some port.toNat

/-- A declared result-to-boundary index must be below the reserved missing-port
sentinel; every such index survives the exact one-byte encoding. -/
theorem BytePlan.resultPort_roundtrip (port : Option Nat)
    (valid : ∀ value, port = some value → value < 255) :
    decodeResultPort (emitResultPort port) = port := by
  cases port with
  | none => rfl
  | some value =>
    have h : value < 255 := valid value rfl
    have hv : value < UInt8.size := by
      have hsize : UInt8.size = 256 := by decide
      omega
    simp [emitResultPort, decodeResultPort, UInt8.toNat_ofNat_of_lt' hv,
      Nat.ne_of_lt h]

private theorem resultPorts_roundtrip (ports : List (Option Nat))
    (valid : ∀ port ∈ ports, ∀ value, port = some value → value < 255) :
    (ports.map emitResultPort).map decodeResultPort = ports := by
  induction ports with
  | nil => rfl
  | cons port rest ih =>
    have hp := BytePlan.resultPort_roundtrip port (valid port (by simp))
    have hr := ih (by
      intro other ho value hv
      exact valid other (List.mem_cons_of_mem _ ho) value hv)
    simp [hp, hr]

private def emitResult (result : Bool × Nat × NetSym × Nat × List (Option Nat)) :
    List UInt8 :=
  let (fresh, slot, symbol, donor, ports) := result
  let (tag, payload) := emitSymbol symbol
  [if fresh then 1 else 0] ++ emitWord 4 slot ++
    [tag] ++ emitWord 8 payload ++ emitWord 4 donor ++
    ports.map emitResultPort

private def readResult (bytes : List UInt8) :
    Option ((Bool × Nat × NetSym × Nat × List (Option Nat)) × List UInt8) := do
  let (fresh, rest) ← readByte bytes
  if fresh.toNat > 1 then none else do
    let (slot, rest) ← readWord 4 rest
    let (tag, rest) ← readByte rest
    let (payload, rest) ← readWord 8 rest
    let symbol ← decodeSymbol tag payload
    let (donor, rest) ← readWord 4 rest
    let (ports, rest) ← readMany 3 readByte rest
    let ports := ports.map decodeResultPort
    some ((fresh == 1, slot, symbol, donor, ports), rest)

/-- A checked emitted result has the same fresh flag, physical destination,
symbol and arbitrary I64 payload, donor and three boundary-index bytes when
decoded by the independent byte reader. -/
theorem BytePlan.readResult_emitResult (fresh : Bool) (slot : Nat)
    (symbol : NetSym) (donor : Nat) (ports : List (Option Nat))
    (rest : List UInt8)
    (hslot : slot < 256 ^ 4) (hdonor : donor < 256 ^ 4)
    (hlen : ports.length = 3)
    (hports : ∀ port ∈ ports, ∀ value, port = some value → value < 255) :
    readResult (emitResult (fresh, slot, symbol, donor, ports) ++ rest) =
      some ((fresh, slot, symbol, donor, ports), rest) := by
  have hword := BytePlan.emitSymbol_payload_bounded symbol
  have hread : readMany 3 readByte (ports.map emitResultPort ++ rest) =
      some (ports.map emitResultPort, rest) := by
    simpa [hlen] using BytePlan.readMany_bytes (ports.map emitResultPort) rest
  have hround := resultPorts_roundtrip ports hports
  cases fresh <;>
    simp [emitResult, readResult, readByte, List.append_assoc,
      BytePlan.readWord_emitWord, hslot, hdonor, hword,
      BytePlan.decodeSymbol_emitSymbol, hread, hround]

private def readWrite (bytes : List UInt8) :
    Option ((Nat × Nat × Nat) × List UInt8) := do
  let (result, rest) ← readByte bytes
  let (port, rest) ← readByte rest
  let (boundary, rest) ← readByte rest
  some ((result.toNat, port.toNat, boundary.toNat), rest)

private def emitWrite (write : Nat × Nat × Nat) : List UInt8 :=
  [UInt8.ofNat write.1, UInt8.ofNat write.2.1,
    UInt8.ofNat write.2.2]

/-- Each declared internal or reciprocal external connection write carries
three untruncated indices in the version-one byte format. -/
theorem BytePlan.readWrite_emitWrite (write : Nat × Nat × Nat)
    (rest : List UInt8)
    (result : write.1 < UInt8.size)
    (port : write.2.1 < UInt8.size)
    (boundary : write.2.2 < UInt8.size) :
    readWrite (emitWrite write ++ rest) = some (write, rest) := by
  rcases write with ⟨r, p, b⟩
  simp [emitWrite, readWrite, readByte,
    UInt8.toNat_ofNat_of_lt' result,
    UInt8.toNat_ofNat_of_lt' port,
    UInt8.toNat_ofNat_of_lt' boundary]

private def readPair (bytes : List UInt8) :
    Option ((Nat × Nat × Nat × Nat) × List UInt8) := do
  let (first, rest) ← readWord 4 bytes
  let (firstGeneration, rest) ← readWord 8 rest
  let (second, rest) ← readWord 4 rest
  let (secondGeneration, rest) ← readWord 8 rest
  some ((first, firstGeneration, second, secondGeneration), rest)

private def emitPair (pair : Nat × Nat × Nat × Nat) : List UInt8 :=
  emitWord 4 pair.1 ++ emitWord 8 pair.2.1 ++
  emitWord 4 pair.2.2.1 ++ emitWord 8 pair.2.2.2

/-- A physically queued pair's two 32-bit indices and two 64-bit generations
round-trip through exactly the on-wire tuple used by the plan compiler. -/
theorem BytePlan.readPair_emitPair (pair : Nat × Nat × Nat × Nat)
    (rest : List UInt8)
    (first : pair.1 < 256 ^ 4)
    (firstGeneration : pair.2.1 < 256 ^ 8)
    (second : pair.2.2.1 < 256 ^ 4)
    (secondGeneration : pair.2.2.2 < 256 ^ 8) :
    readPair (emitPair pair ++ rest) = some (pair, rest) := by
  rcases pair with ⟨slotA, genA, slotB, genB⟩
  simp [emitPair, readPair, List.append_assoc, BytePlan.readWord_emitWord,
    first, firstGeneration, second, secondGeneration]

private def readHeader (bytes : List UInt8) : Option (List UInt8) :=
  if bytes.take 5 == ([78, 80, 76, 78, 1] : List UInt8) &&
      bytes.length ≥ 5 then some (bytes.drop 5) else none

/-- Emit the same version-one field order as the selected Rust writer. No
truncation is claimed unless the stated admissibility bounds hold. -/
def BytePlan.emit (plan : BytePlan) : List UInt8 :=
  [78, 80, 76, 78, 1, if plan.reuse then 0 else 1, plan.opcode] ++
  emitPair plan.pair ++
  [plan.action.1, plan.action.2, UInt8.ofNat plan.boundaries.length] ++
  plan.boundaries.flatMap emitBoundary ++
  [UInt8.ofNat plan.results.length] ++
  plan.results.flatMap emitResult ++
  [UInt8.ofNat plan.writes.length] ++
  plan.writes.flatMap emitWrite ++
  [UInt8.ofNat plan.externalWrites.length] ++
  plan.externalWrites.flatMap emitWrite ++
  emitWord 4 plan.arenaReservation ++
  emitWord 4 plan.scratchReservation ++
  emitWord 4 plan.worklistReservation ++
  [UInt8.ofNat plan.publications.length] ++
  plan.publications.flatMap emitPair ++ [199]

private def validPair (pair : Nat × Nat × Nat × Nat) : Prop :=
  pair.1 < 256 ^ 4 ∧ pair.2.1 < 256 ^ 8 ∧
  pair.2.2.1 < 256 ^ 4 ∧ pair.2.2.2 < 256 ^ 8

private def validBoundary
    (boundary : (Nat × Nat) × (Nat × Nat × Nat)) : Prop :=
  boundary.1.1 < 256 ^ 4 ∧ boundary.1.2 < UInt8.size ∧
  validEndpoint boundary.2

private def validResult
    (result : Bool × Nat × NetSym × Nat × List (Option Nat)) : Prop :=
  result.2.1 < 256 ^ 4 ∧ result.2.2.2.1 < 256 ^ 4 ∧
  result.2.2.2.2.length = 3 ∧
  ∀ port ∈ result.2.2.2.2, ∀ value, port = some value → value < 255

private def validWrite (write : Nat × Nat × Nat) : Prop :=
  write.1 < UInt8.size ∧ write.2.1 < UInt8.size ∧
  write.2.2 < UInt8.size

/-- Exact finite bounds of an emitted plan, not an assertion about its rule
validity: an independently checked image must additionally match the live
selected pair, captured boundary, result symbols and write inventory. -/
structure BytePlan.Admissible (plan : BytePlan) : Prop where
  size : plan.emit.length ≤ 256
  actionFirst : plan.action.1.toNat ≤ 1
  actionSecond : plan.action.2.toNat ≤ 1
  pair : validPair plan.pair
  boundaries : plan.boundaries.length ≤ 2
  boundariesValid : ∀ boundary ∈ plan.boundaries, validBoundary boundary
  results : plan.results.length ≤ 2
  resultsValid : ∀ result ∈ plan.results, validResult result
  writes : plan.writes.length ≤ 2
  writesValid : ∀ write ∈ plan.writes, validWrite write
  externalWrites : plan.externalWrites.length ≤ 2
  externalWritesValid : ∀ write ∈ plan.externalWrites, validWrite write
  arena : plan.arenaReservation < 256 ^ 4
  scratch : plan.scratchReservation < 256 ^ 4
  worklist : plan.worklistReservation < 256 ^ 4
  publications : plan.publications.length ≤ 2
  publicationsValid : ∀ pair ∈ plan.publications, validPair pair

private theorem BytePlan.readHeader_emit (plan : BytePlan) :
    readHeader plan.emit = some (plan.emit.drop 5) := by
  simp [readHeader, BytePlan.emit]

private theorem count_fits_byte {count : Nat} (h : count ≤ 2) :
    count < UInt8.size := by
  have hsize : UInt8.size = 256 := by decide
  omega

/-- Decode the entire version-one byte grammar, rejecting malformed counts,
invalid symbols, noncanonical flags, missing commit markers and trailing bytes.
This parses bytes independently of the Rust compiler; field-vs-live-state
checking and execution correspondence are separate obligations. -/
def BytePlan.decode (bytes : List UInt8) : Option BytePlan := do
  if bytes.length > 256 then none else do
    let rest ← readHeader bytes
    let (mode, rest) ← readByte rest
    if mode.toNat > 1 then none else do
      let (opcode, rest) ← readByte rest
      let (pair, rest) ← readPair rest
      let (firstAction, rest) ← readByte rest
      let (secondAction, rest) ← readByte rest
      if firstAction.toNat > 1 || secondAction.toNat > 1 then none else do
        let (boundaries, rest) ← readCounted 2 readBoundary rest
        let (results, rest) ← readCounted 2 readResult rest
        let (writes, rest) ← readCounted 2 readWrite rest
        let (externalWrites, rest) ← readCounted 2 readWrite rest
        let (arenaReservation, rest) ← readWord 4 rest
        let (scratchReservation, rest) ← readWord 4 rest
        let (worklistReservation, rest) ← readWord 4 rest
        let (publications, rest) ← readCounted 2 readPair rest
        let (commit, remaining) ← readByte rest
        if commit == 199 && remaining.isEmpty then
          some {
            reuse := mode == 0
            opcode
            pair
            action := (firstAction, secondAction)
            boundaries
            results
            writes
            externalWrites
            arenaReservation
            scratchReservation
            worklistReservation
            publications
          }
        else none

/-- The independent parser decodes every bounded version-one field image
exactly, including arbitrary I64 staged operands, both endpoint tags, reused
or freshly allocated result slots, exact writes, reservations and queue
publications. This is byte-format agreement, not a physical commit proof. -/
theorem BytePlan.decode_emit (plan : BytePlan)
    (valid : plan.Admissible) :
    BytePlan.decode plan.emit = some plan := by
  have pairSound (suffix : List UInt8) :
      readPair (emitPair plan.pair ++ suffix) = some (plan.pair, suffix) :=
    BytePlan.readPair_emitPair plan.pair suffix
      valid.pair.1 valid.pair.2.1 valid.pair.2.2.1 valid.pair.2.2.2
  have boundarySound (suffix : List UInt8) :
      readCounted 2 readBoundary
        (UInt8.ofNat plan.boundaries.length ::
          (plan.boundaries.flatMap emitBoundary ++ suffix)) =
        some (plan.boundaries, suffix) :=
    readCounted_emit 2 readBoundary emitBoundary plan.boundaries suffix
      (by
        intro boundary hb tail
        rcases valid.boundariesValid boundary hb with ⟨hs, hp, he⟩
        exact BytePlan.readBoundary_emitBoundary boundary tail hs hp he)
      valid.boundaries (count_fits_byte valid.boundaries)
  have resultSound (suffix : List UInt8) :
      readCounted 2 readResult
        (UInt8.ofNat plan.results.length ::
          (plan.results.flatMap emitResult ++ suffix)) =
        some (plan.results, suffix) :=
    readCounted_emit 2 readResult emitResult plan.results suffix
      (by
        intro result hr tail
        rcases result with ⟨fresh, slot, symbol, donor, ports⟩
        rcases valid.resultsValid _ hr with ⟨hs, hd, hl, hp⟩
        exact BytePlan.readResult_emitResult fresh slot symbol donor ports
          tail hs hd hl hp)
      valid.results (count_fits_byte valid.results)
  have writeSound (suffix : List UInt8) :
      readCounted 2 readWrite
        (UInt8.ofNat plan.writes.length ::
          (plan.writes.flatMap emitWrite ++ suffix)) =
        some (plan.writes, suffix) :=
    readCounted_emit 2 readWrite emitWrite plan.writes suffix
      (by
        intro write hw tail
        rcases valid.writesValid write hw with ⟨hr, hp, hb⟩
        exact BytePlan.readWrite_emitWrite write tail hr hp hb)
      valid.writes (count_fits_byte valid.writes)
  have externalSound (suffix : List UInt8) :
      readCounted 2 readWrite
        (UInt8.ofNat plan.externalWrites.length ::
          (plan.externalWrites.flatMap emitWrite ++ suffix)) =
        some (plan.externalWrites, suffix) :=
    readCounted_emit 2 readWrite emitWrite plan.externalWrites suffix
      (by
        intro write hw tail
        rcases valid.externalWritesValid write hw with ⟨hr, hp, hb⟩
        exact BytePlan.readWrite_emitWrite write tail hr hp hb)
      valid.externalWrites (count_fits_byte valid.externalWrites)
  have publicationSound (suffix : List UInt8) :
      readCounted 2 readPair
        (UInt8.ofNat plan.publications.length ::
          (plan.publications.flatMap emitPair ++ suffix)) =
        some (plan.publications, suffix) :=
    readCounted_emit 2 readPair emitPair plan.publications suffix
      (by
        intro pair hp tail
        rcases valid.publicationsValid pair hp with ⟨ha, hga, hb, hgb⟩
        exact BytePlan.readPair_emitPair pair tail ha hga hb hgb)
      valid.publications (count_fits_byte valid.publications)
  have hsize : ¬ plan.emit.length > 256 := Nat.not_lt.mpr valid.size
  have hmode : ((if plan.reuse then (0 : UInt8) else 1) == 0) = plan.reuse := by
    cases plan.reuse <;> decide
  have hmodeRange : (if plan.reuse then (0 : UInt8) else 1).toNat ≤ 1 := by
    cases plan.reuse <;> decide
  unfold BytePlan.decode
  simp only [hsize, ↓reduceIte]
  simp [BytePlan.emit, readHeader, readByte,
    List.append_assoc, hmode, hmodeRange, valid.actionFirst, valid.actionSecond,
    pairSound, boundarySound, resultSound, writeSound, externalSound,
    publicationSound, BytePlan.readWord_emitWord, valid.arena,
    valid.scratch, valid.worklist]

/-- Every accepted byte plan has the exact version-one magic and version header. -/
theorem BytePlan.decode_header {bytes : List UInt8} {plan : BytePlan}
    (h : BytePlan.decode bytes = some plan) :
    bytes.take 5 = ([78, 80, 76, 78, 1] : List UInt8) := by
  unfold BytePlan.decode at h
  split at h
  · contradiction
  · cases hh : readHeader bytes with
    | none => simp [hh] at h
    | some rest =>
      have hheader : bytes.take 5 = ([78, 80, 76, 78, 1] : List UInt8) := by
        by_cases heq : bytes.take 5 = ([78, 80, 76, 78, 1] : List UInt8)
        · exact heq
        · have hnone : readHeader bytes = none := by simp [readHeader, heq]
          simp [hnone] at hh
      exact hheader

/-- OP2/number: reuse the consumed rule slot, initialize one staged I64
result, and retarget two ordered output boundaries. -/
def op2OutputFixture : BytePlan :=
  { reuse := true
    opcode := 1
    pair := (0, 0, 1, 0)
    action := (1, 0)
    boundaries := [((0, 1), (1, 0, 0)), ((0, 2), (1, 1, 0))]
    results := [(false, 0, .op1 .add (BitVec.ofNat 64 27), 0,
      [some 0, some 1, none])]
    writes := [(0, 0, 0), (0, 1, 1)]
    externalWrites := [(0, 0, 0), (0, 1, 1)]
    arenaReservation := 0
    scratchReservation := 256
    worklistReservation := 0
    publications := [] }

/-- The full OP2 physical image, including both reciprocal boundary writes,
decodes without a fresh slot allocation. This is a Lean-model byte fixture. -/
theorem BytePlan.op2_output_image :
    BytePlan.decode op2OutputFixture.emit = some op2OutputFixture := by
  apply BytePlan.decode_emit
  have hbyte : UInt8.size = 256 := by decide
  constructor
  · set_option maxRecDepth 3000 in decide
  all_goals simp [hbyte, op2OutputFixture, validPair, validBoundary,
    validEndpoint, validResult, validWrite]

/-- The OP2 fixture's result symbol is exactly the selected rule's staged
payload, independently of its physical reused-slot assignment. -/
theorem BytePlan.op2_rule_symbols :
    (compileRule (.op2 .add) (.num (BitVec.ofNat 64 27))).map
      (fun rule => rule.results.map Prod.snd) =
    some (op2OutputFixture.results.map fun result => result.2.2.1) := by
  decide

/-- One OP2 principal result wired to a live external number, hence a new
active pair, while the other result port remains connected to an output. -/
def op2PublishedFixture : BytePlan :=
  { op2OutputFixture with
    boundaries := [((0, 1), (0, 2, 0)), ((0, 2), (1, 0, 0))]
    worklistReservation := 1
    publications := [(0, 1, 2, 0)] }

theorem BytePlan.op2_published_image :
    BytePlan.decode op2PublishedFixture.emit = some op2PublishedFixture := by
  apply BytePlan.decode_emit
  have hbyte : UInt8.size = 256 := by decide
  constructor
  · set_option maxRecDepth 3000 in decide
  all_goals simp [hbyte, op2PublishedFixture, op2OutputFixture,
    validPair, validBoundary, validEndpoint, validResult, validWrite]

/-- OP1/subtract: stage the wrapped result into the reused consumed rule slot,
retargeting one external output. -/
def op1OutputFixture : BytePlan :=
  { reuse := true
    opcode := 2
    pair := (0, 0, 1, 0)
    action := (1, 0)
    boundaries := [((0, 1), (1, 0, 0))]
    results := [(false, 0, .num (BitVec.ofInt 64 (-6)), 0,
      [some 0, none, none])]
    writes := [(0, 0, 0)]
    externalWrites := [(0, 0, 0)]
    arenaReservation := 0
    scratchReservation := 256
    worklistReservation := 0
    publications := [] }

/-- The OP1 image retains the 64-bit wrapping subtraction payload when
decoded from its complete plan byte stream. -/
theorem BytePlan.op1_wrapping_output_image :
    BytePlan.decode op1OutputFixture.emit = some op1OutputFixture := by
  apply BytePlan.decode_emit
  have hbyte : UInt8.size = 256 := by decide
  constructor
  · decide
  all_goals simp [hbyte, op1OutputFixture, validPair, validBoundary,
    validEndpoint, validResult, validWrite]

/-- The OP1 fixture's wrapped subtraction symbol is the selected revision-3
rule result for seven minus thirteen. -/
theorem BytePlan.op1_wrapping_rule_symbols :
    (compileRule (.op1 .sub (BitVec.ofNat 64 7))
      (.num (BitVec.ofNat 64 13))).map
      (fun rule => rule.results.map Prod.snd) =
    some (op1OutputFixture.results.map fun result => result.2.2.1) := by
  decide

/-- DUP/number: reuse both consumed slots, retag both with the numeric
payload, and retarget the two external outputs separately. -/
def dupOutputFixture : BytePlan :=
  { reuse := true
    opcode := 3
    pair := (0, 0, 1, 0)
    action := (1, 1)
    boundaries := [((0, 1), (1, 0, 0)), ((0, 2), (1, 1, 0))]
    results := [(false, 0, .num (BitVec.ofNat 64 42), 0,
      [some 0, none, none]),
      (false, 1, .num (BitVec.ofNat 64 42), 1,
      [some 1, none, none])]
    writes := [(0, 0, 0), (1, 0, 1)]
    externalWrites := [(0, 0, 0), (1, 0, 1)]
    arenaReservation := 0
    scratchReservation := 256
    worklistReservation := 0
    publications := [] }

/-- Both DUP results and their independent physical output writes decode
without fresh slot allocation. -/
theorem BytePlan.dup_output_image :
    BytePlan.decode dupOutputFixture.emit = some dupOutputFixture := by
  apply BytePlan.decode_emit
  have hbyte : UInt8.size = 256 := by decide
  constructor
  · set_option maxRecDepth 2000 in decide
  all_goals simp [hbyte, dupOutputFixture, validPair, validBoundary,
    validEndpoint, validResult, validWrite]

/-- The two DUP result symbols agree in order with the selected right-hand
side; this does not prove reciprocal physical port writes. -/
theorem BytePlan.dup_rule_symbols :
    (compileRule .dup (.num (BitVec.ofNat 64 42))).map
      (fun rule => rule.results.map Prod.snd) =
    some (dupOutputFixture.results.map fun result => result.2.2.1) := by
  decide

/-- One erasure image with physical slots zero and one, both at generation zero.
The same 51-byte layout is used by the Rust writer for the no-result erasure
rule; its Rust identity remains a separate, finite differential obligation. -/
def eraPlanBytes : List UInt8 :=
  [78, 80, 76, 78, 1, 0, 4] ++
  [0, 0, 0, 0] ++ List.replicate 8 0 ++
  [1, 0, 0, 0] ++ List.replicate 8 0 ++
  [0, 0] ++ [0, 0, 0, 0] ++ List.replicate 4 0 ++
  [0, 1, 0, 0] ++ List.replicate 4 0 ++ [0, 199]

/-- The byte decoder reads every erasure field, including its zero fresh
reservation and the fixed 256-byte plan scratch reservation. -/
theorem BytePlan.era_image :
    BytePlan.decode eraPlanBytes = some {
      reuse := true
      opcode := 4
      pair := (0, 0, 1, 0)
      action := (0, 0)
      boundaries := []
      results := []
      writes := []
      externalWrites := []
      arenaReservation := 0
      scratchReservation := 256
      worklistReservation := 0
      publications := []
    } := by
  decide

/-- The no-result ERA plan image matches the selected rule's empty result
list for this representative numeric payload. -/
theorem BytePlan.era_rule_symbols :
    (compileRule .era (.num (BitVec.ofNat 64 42))).map
      (fun rule => rule.results.map Prod.snd) =
    some ([] : List NetSym) := by
  decide

/-- An erasure image with an appended operation is not a complete plan. -/
theorem BytePlan.era_trailing_bytes_refused :
    BytePlan.decode (eraPlanBytes ++ [0]) = none := by
  decide

/-- An erasure image carrying a different format version is refused. -/
theorem BytePlan.era_wrong_version_refused :
    BytePlan.decode ([78, 80, 76, 78, 2] ++ eraPlanBytes.drop 5) = none := by
  decide

/-- One physical slot in the sequential model. A logical identity is carried
separately from its reusable physical index and its bounded generation. -/
structure PhysicalSlot where
  agent : Option (Id × NetSym)
  generation : UInt64
  queuedGeneration : Option UInt64
  ports : List (Option (Nat × Nat))
  deriving DecidableEq

/-- A queued physical pair records both generations; stale entries are not
silently the same pair after either slot is reused. -/
structure PhysicalPair where
  first : Nat
  firstGeneration : UInt64
  second : Nat
  secondGeneration : UInt64
  deriving DecidableEq

/-- A stable abstract external port with its mutable physical arena target.
Rust stores the target under the output index; the stable `label` is the
logical free-port identity required by the inets representation relation. -/
structure PhysicalOutput where
  label : Port
  target : Nat × Nat
  deriving DecidableEq

/-- Concrete single-engine storage, including empty-slot inventory, stable
external output identities with mutable targets and the ordered worklist. -/
structure PhysicalState where
  slots : Array PhysicalSlot
  freeSlots : List Nat
  outputs : List PhysicalOutput
  work : List PhysicalPair
  deriving DecidableEq

/-- Retarget a Rust-indexed output while retaining the abstract free-port
label. Out-of-range indices refuse instead of creating a new output. -/
private def retargetOutputList : Nat → (Nat × Nat) → List PhysicalOutput →
    Option (List PhysicalOutput)
  | _, _, [] => none
  | 0, target, output :: rest =>
      some ({ output with target } :: rest)
  | index + 1, target, output :: rest =>
      (retargetOutputList index target rest).map (output :: ·)

def PhysicalState.retargetOutput (state : PhysicalState)
    (index : Nat) (target : Nat × Nat) : Option PhysicalState :=
  (retargetOutputList index target state.outputs).map fun outputs =>
    { state with outputs }

private theorem retargetOutputList_labels (outputs : List PhysicalOutput)
    (index : Nat) (target : Nat × Nat) (next : List PhysicalOutput)
    (h : retargetOutputList index target outputs = some next) :
    next.map PhysicalOutput.label = outputs.map PhysicalOutput.label := by
  induction index generalizing outputs next with
  | zero =>
    cases outputs with
    | nil => simp [retargetOutputList] at h
    | cons output rest =>
      simp [retargetOutputList] at h
      subst next
      rfl
  | succ index ih =>
    cases outputs with
    | nil => simp [retargetOutputList] at h
    | cons output rest =>
      cases hr : retargetOutputList index target rest with
      | none => simp [retargetOutputList, hr] at h
      | some updated =>
        simp [retargetOutputList, hr] at h
        subst next
        simp [ih rest updated hr]

/-- Retag one existing physical slot; an out-of-range index cannot silently
grow the bounded arena. -/
def PhysicalState.writeSlot (state : PhysicalState)
    (index : Nat) (replacement : PhysicalSlot) : PhysicalState :=
  { state with slots := state.slots.setIfInBounds index replacement }

/-- Updating a consumed physical slot leaves every other slot unchanged,
including its generation and queued-reference mark. -/
theorem PhysicalState.writeSlot_frame {state : PhysicalState}
    {index other : Nat} {replacement : PhysicalSlot} (h : index ≠ other) :
    (state.writeSlot index replacement).slots[other]? = state.slots[other]? := by
  simp [PhysicalState.writeSlot, Array.getElem?_setIfInBounds_ne h]

/-- A fresh physical slot is appended, without changing any prior slot. -/
def PhysicalState.appendSlot (state : PhysicalState)
    (fresh : PhysicalSlot) : PhysicalState :=
  { state with slots := state.slots.push fresh }

theorem PhysicalState.appendSlot_frame {state : PhysicalState}
    {fresh : PhysicalSlot} {index : Nat} (h : index < state.slots.size) :
    (state.appendSlot fresh).slots[index]? = state.slots[index]? := by
  simp [PhysicalState.appendSlot, Array.getElem?_push, Nat.ne_of_lt h]

def PhysicalState.logicalPort (state : PhysicalState) (port : Nat × Nat) : Option Port := do
  let slot ← state.slots[port.1]?
  let (logical, _) ← slot.agent
  some (logical, port.2)

/-- A physical write away from one boundary endpoint cannot retarget the
represented logical port at that endpoint. -/
theorem PhysicalState.logicalPort_writeSlot_away {state : PhysicalState}
    {index : Nat} {replacement : PhysicalSlot} {port : Nat × Nat}
    (h : index ≠ port.1) :
    (state.writeSlot index replacement).logicalPort port = state.logicalPort port := by
  simp [PhysicalState.logicalPort, PhysicalState.writeSlot,
    Array.getElem?_setIfInBounds_ne h]

private def PhysicalState.portWires (state : PhysicalState) (logical : Id) :
    Nat → List (Option (Nat × Nat)) → Std.HashMap Port Port → Std.HashMap Port Port
  | _, [], wires => wires
  | index, target :: rest, wires =>
      let wires := match target with
        | none => wires
        | some target =>
          match state.logicalPort target with
          | none => wires
          | some endpoint => wires.insert (logical, index) endpoint
      PhysicalState.portWires state logical (index + 1) rest wires

/-- Decode live physical slots into the inets agent map, using the logical
identity attached to each generation of a reusable physical slot. -/
def PhysicalState.agents (state : PhysicalState) : Std.HashMap Id NetSym :=
  state.slots.toList.foldl (fun agents slot =>
    match slot.agent with
    | none => agents
    | some (logical, symbol) => agents.insert logical symbol) {}

/-- Decode physical port references and both directions of an output link into
logical inets wires. Output labels remain fixed when their arena targets are
retargeted by an admitted plan. -/
def PhysicalState.wires (state : PhysicalState) : Std.HashMap Port Port :=
  let internal := state.slots.toList.foldl (fun wires slot =>
    match slot.agent with
    | none => wires
    | some (logical, _) => state.portWires logical 0 slot.ports wires) {}
  state.outputs.foldl (fun wires output =>
    match state.logicalPort output.target with
    | none => wires
    | some endpoint => (wires.insert output.label endpoint).insert endpoint output.label)
    internal

def PhysicalState.logicalPair (state : PhysicalState)
    (pair : PhysicalPair) : Option (Id × Id) := do
  let first ← state.slots[pair.first]?
  let second ← state.slots[pair.second]?
  if first.generation != pair.firstGeneration ||
      second.generation != pair.secondGeneration then none else do
    let (left, _) ← first.agent
    let (right, _) ← second.agent
    some (left, right)

/-- The vendored inets worklist records both directed principal wires,
whereas Rust queues each canonical physical pair only once. -/
private def PhysicalState.pairOrientations (state : PhysicalState)
    (pair : PhysicalPair) : List (Id × Id) :=
  match state.logicalPair pair with
  | none => []
  | some (left, right) => [(left, right), (right, left)]

/-- A queued pair naming an old first-slot generation never resolves to
the new agent occupying that same physical slot. -/
theorem PhysicalState.stale_first_refused {state : PhysicalState}
    {pair : PhysicalPair} {slot : PhysicalSlot}
    (hslot : state.slots[pair.first]? = some slot)
    (hstale : slot.generation ≠ pair.firstGeneration) :
    state.logicalPair pair = none := by
  simp [PhysicalState.logicalPair, hslot, hstale]

/-- Retagging an unrelated slot cannot change a queue entry's two decoded
logical identities or its generation validity. -/
theorem PhysicalState.logicalPair_writeSlot_away {state : PhysicalState}
    {index : Nat} {replacement : PhysicalSlot} {pair : PhysicalPair}
    (left : index ≠ pair.first) (right : index ≠ pair.second) :
    (state.writeSlot index replacement).logicalPair pair = state.logicalPair pair := by
  simp [PhysicalState.logicalPair, PhysicalState.writeSlot,
    Array.getElem?_setIfInBounds_ne left,
    Array.getElem?_setIfInBounds_ne right]

/-- Projection is computed from physical slots, ports, ordered outputs and
generation-checked queued pairs, not from a ghost abstract net. -/
def PhysicalState.project (state : PhysicalState) : HNet NetSym :=
  { agents := state.agents
    wires := state.wires
    free := state.outputs.map PhysicalOutput.label
    work := state.work.flatMap state.pairOrientations }

/-- A checked physical output write cannot rename, add, remove or reorder
external free ports even when the target physical slot is reused. -/
theorem PhysicalState.retargetOutput_preserves_free
    {state next : PhysicalState} {index : Nat} {target : Nat × Nat}
    (h : state.retargetOutput index target = some next) :
    next.project.free = state.project.free := by
  unfold PhysicalState.retargetOutput at h
  cases ho : retargetOutputList index target state.outputs with
  | none => simp [ho] at h
  | some outputs =>
    simp [ho] at h
    cases h
    exact retargetOutputList_labels state.outputs index target outputs ho

/-- Follow Rust's `ActiveWorklist.entries.push` after a checked commit: retain
the existing pending entries, then append the newly published generation-
tagged pairs, without changing slots, wires or outputs. -/
def PhysicalState.appendPublications (state : PhysicalState)
    (published : List PhysicalPair) : PhysicalState :=
  { state with work := state.work ++ published }

theorem PhysicalState.appendPublications_project_work
    (state : PhysicalState) (published : List PhysicalPair) :
    (state.appendPublications published).project.work =
      state.project.work ++ published.flatMap state.pairOrientations := by
  change (state.work ++ published).flatMap
      (state.appendPublications published).pairOrientations =
    state.work.flatMap state.pairOrientations ++
      published.flatMap state.pairOrientations
  rw [List.flatMap_append]
  rfl

/-- Once the decoded image's published physical pairs have the checked
principal-wire inventory, Rust's tail append and inets' head prepend have
identical directed-pair multiplicities despite their different order. The
premises are field-level queue inventories, not whole-net equality. -/
theorem PhysicalState.appendPublications_project_plan_work
    (state : PhysicalState) (published : List PhysicalPair)
    (before : HNet NetSym) (plan : Plan)
    (old : state.project.work = before.work)
    (new : published.flatMap state.pairOrientations = plan.newlyActive) :
    (state.appendPublications published).project.work.Perm
      (Plan.commit before plan before.work).work := by
  rw [PhysicalState.appendPublications_project_work, old, new]
  exact List.perm_append_comm

private theorem PhysicalState.portWires_appendPublications
    (state : PhysicalState) (published : List PhysicalPair)
    (logical : Id) (start : Nat) (ports : List (Option (Nat × Nat)))
    (wires : Std.HashMap Port Port) :
    (state.appendPublications published).portWires logical start ports wires =
      state.portWires logical start ports wires := by
  induction ports generalizing start wires with
  | nil => rfl
  | cons target rest ih =>
    have hport (port : Nat × Nat) :
        (state.appendPublications published).logicalPort port =
          state.logicalPort port := rfl
    simp [PhysicalState.portWires, hport, ih]

theorem PhysicalState.appendPublications_project_net
    (state : PhysicalState) (published : List PhysicalPair) :
    (state.appendPublications published).project.toNet = state.project.toNet := by
  have hwire : (state.appendPublications published).wires = state.wires := by
    have hlogical (port : Nat × Nat) :
        (state.appendPublications published).logicalPort port =
          state.logicalPort port := rfl
    change state.outputs.foldl (fun wires output =>
      match (state.appendPublications published).logicalPort output.target with
      | none => wires
      | some endpoint => (wires.insert output.label endpoint).insert endpoint output.label)
      (state.slots.toList.foldl (fun wires slot =>
      match slot.agent with
      | none => wires
      | some (logical, _) =>
        (state.appendPublications published).portWires logical 0 slot.ports wires) {}) =
      state.outputs.foldl (fun wires output =>
        match state.logicalPort output.target with
        | none => wires
        | some endpoint => (wires.insert output.label endpoint).insert endpoint output.label)
      (state.slots.toList.foldl (fun wires slot =>
        match slot.agent with
        | none => wires
        | some (logical, _) => state.portWires logical 0 slot.ports wires) {})
    simp only [PhysicalState.portWires_appendPublications, hlogical]
  apply Net.ext'
  · intro _ _
    rfl
  · intro _ _
    simp [HNet.toNet, PhysicalState.project, hwire]
  · intro _
    rfl

/-- Turn one result-port boundary reference into its physical peer address.
Output boundaries are represented by the separate mutable output table. -/
private def BytePlan.resultTarget (plan : BytePlan) :
    Option Nat → Option (Option (Nat × Nat))
  | none => some none
  | some index => do
      let boundary ← plan.boundaries[index]?
      match boundary.2 with
      | (0, slot, port) => some (some (slot, port))
      | (1, _, 0) => some none
      | _ => none

/-- Install one checked result into its declared physical slot. A freshly
allocated result must occupy the next slot; reuse must name a live slot with
room to increment its generation. The new logical identity is the inets
rule-slot identity, not the reused physical index. -/
private def PhysicalState.installResult (state : PhysicalState)
    (plan : BytePlan) (logical : Id)
    (result : Bool × Nat × NetSym × Nat × List (Option Nat)) :
    Option PhysicalState := do
  let donor ← state.slots[result.2.2.2.1]?
  if donor.agent.isNone then none else do
    let ports ← result.2.2.2.2.mapM plan.resultTarget
    let symbol := result.2.2.1
    if result.1 then
      if result.2.1 != state.slots.size then none else
        some (state.appendSlot {
          agent := some (logical, symbol)
          generation := 0
          queuedGeneration := none
          ports })
    else do
      let old ← state.slots[result.2.1]?
      if old.agent.isNone || old.generation == UInt64.ofNat (UInt64.size - 1)
      then none else
        some (state.writeSlot result.2.1 {
          agent := some (logical, symbol)
          generation := old.generation + 1
          queuedGeneration := none
          ports })

/-- Decode each selected rule slot's stable child identity and install its
physical result image in the same order as the checked Rust executor. -/
private def PhysicalState.installResults (plan : BytePlan)
    (left right : Id) (rhs : XRhs NetSym) :
    Nat → PhysicalState → List (Bool × Nat × NetSym × Nat × List (Option Nat)) →
      Option PhysicalState
  | _, state, [] => some state
  | index, state, result :: rest => do
      let slot ← rhs.slots[index]?
      if rhs.lab slot.1 slot.2 != some result.2.2.1 then none else do
        let installed ← state.installResult plan (child left right slot.1 slot.2) result
        PhysicalState.installResults plan left right rhs (index + 1) installed rest

/-- Retire a consumed slot not reused by a result, refusing stale or
overflowing generations rather than publishing an invalid free slot. -/
private def PhysicalState.retire (state : PhysicalState) (index : Nat)
    (reuse : Bool) : Option PhysicalState := do
  if reuse then some state else do
    let old ← state.slots[index]?
    if old.agent.isNone || old.generation == UInt64.ofNat (UInt64.size - 1)
    then none else
      let retired := state.writeSlot index {
        old with
        agent := none
        generation := old.generation + 1
        queuedGeneration := none }
      some { retired with freeSlots := index :: retired.freeSlots }

/-- Apply the reciprocal side of one checked boundary wire: either update an
external agent's exact prior physical endpoint or retarget a checked output.
The result's own port was installed by `installResult`. -/
private def PhysicalState.writeExternal (state : PhysicalState)
    (plan : BytePlan) (write : Nat × Nat × Nat) : Option PhysicalState := do
  let result ← plan.results[write.1]?
  let boundary ← plan.boundaries[write.2.2]?
  if result.2.2.2.2[write.2.1]? != some (some write.2.2) then none else do
    let target := (result.2.1, write.2.1)
    match boundary.2 with
    | (0, peer, port) => do
        if peer == plan.pair.1 || peer == plan.pair.2.2.1 then none else do
          let old ← state.slots[peer]?
          if old.agent.isNone || old.ports[port]? != some (some boundary.1)
          then none else
            some (state.writeSlot peer {
              old with
              ports := old.ports.set port (some target)
              queuedGeneration := if port == 0 then none else old.queuedGeneration })
    | (1, output, 0) => do
        let old ← state.outputs[output]?
        if old.target != boundary.1 then none else
          state.retargetOutput output target
    | _ => none

private def PhysicalState.writeExternals (state : PhysicalState)
    (plan : BytePlan) : List (Nat × Nat × Nat) → Option PhysicalState
  | [] => some state
  | write :: rest => do
      let next ← state.writeExternal plan write
      next.writeExternals plan rest

/-- Publish one reciprocal principal pair after the existing queue, marking
both current generations. Old pending entries and old-generation marks do not
block a new publication: Rust may leave those stale entries to be skipped
when popped. -/
private def PhysicalState.publish (state : PhysicalState)
    (pair : PhysicalPair) : Option PhysicalState := do
  if pair.first >= pair.second then none else do
    let first ← state.slots[pair.first]?
    let second ← state.slots[pair.second]?
    if (state.logicalPair pair).isNone ||
        first.ports[0]? != some (some (pair.second, 0)) ||
        second.ports[0]? != some (some (pair.first, 0)) ||
        first.queuedGeneration == some pair.firstGeneration ||
        second.queuedGeneration == some pair.secondGeneration ||
        state.work.length >= state.slots.size
    then none else
      let marked := (state.writeSlot pair.first
        { first with queuedGeneration := some pair.firstGeneration }).writeSlot
        pair.second { second with queuedGeneration := some pair.secondGeneration }
      some (marked.appendPublications [pair])

private def PhysicalState.publishAll (state : PhysicalState) :
    List PhysicalPair → Option PhysicalState
  | [] => some state
  | pair :: rest => do
      let next ← state.publish pair
      next.publishAll rest

/-- Execute the physical effects of a decoded checked image, retaining the
Rust queue order. This defines the missing concrete side of the refinement
problem: its observable equality with `Plan.compile` is proved only where
theorems below explicitly discharge the rule and boundary premises. The
independent checker remains responsible for ownership, exact-write inventory,
pair shape, output alias exclusion and publication validity. -/
def PhysicalState.executeCheckedImage (state : PhysicalState)
    (bytes : List UInt8) (redex : XRedex NetSym) :
    Option PhysicalState := do
  let plan ← BytePlan.decode bytes
  let pair : PhysicalPair :=
    { first := plan.pair.1
      firstGeneration := UInt64.ofNat plan.pair.2.1
      second := plan.pair.2.2.1
      secondGeneration := UInt64.ofNat plan.pair.2.2.2 }
  if state.logicalPair pair != some (redex.1, redex.2.1) then none else do
    let installed ← state.installResults plan redex.1 redex.2.1
      redex.2.2.2.2 0 plan.results
    let rewired ← installed.writeExternals plan plan.externalWrites
    let retiredFirst ← rewired.retire plan.pair.1 (plan.action.1 == 1)
    let retiredSecond ← retiredFirst.retire plan.pair.2.2.1 (plan.action.2 == 1)
    let publications := plan.publications.map fun (first, firstGeneration, second, secondGeneration) =>
      { first, firstGeneration := UInt64.ofNat firstGeneration,
        second, secondGeneration := UInt64.ofNat secondGeneration : PhysicalPair }
    retiredSecond.publishAll publications

/-- Installing any admitted result, whether recycled or freshly appended,
cannot alter the ordered external-output identities. -/
private theorem PhysicalState.installResult_output_labels
    {state next : PhysicalState} {plan : BytePlan} {logical : Id}
    {result : Bool × Nat × NetSym × Nat × List (Option Nat)}
    (h : state.installResult plan logical result = some next) :
    next.outputs.map PhysicalOutput.label =
      state.outputs.map PhysicalOutput.label := by
  unfold PhysicalState.installResult at h
  cases hd : state.slots[result.2.2.2.1]? with
  | none => simp [hd] at h
  | some donor =>
    cases ha : donor.agent with
    | none => simp [hd, ha] at h
    | some data =>
      cases hports : result.2.2.2.2.mapM plan.resultTarget with
      | none => simp [hd, hports] at h
      | some ports =>
        by_cases hfresh : result.1
        · simp [hd, ha, hports, hfresh, PhysicalState.appendSlot] at h
          rcases h with ⟨_, heq⟩
          cases heq
          rfl
        · cases ho : state.slots[result.2.1]? with
          | none => simp [hd, hports, hfresh, ho] at h
          | some old =>
            simp [hd, ha, hports, hfresh, ho,
              PhysicalState.writeSlot] at h
            rcases h with ⟨_, heq⟩
            cases heq
            rfl

/-- Every finite result installation preserves external-output identities;
the result count, slot choice and payload are arbitrary. -/
private theorem PhysicalState.installResults_output_labels
    {plan : BytePlan} {left right : Id} {rhs : XRhs NetSym}
    {index : Nat} {state next : PhysicalState}
    {results : List (Bool × Nat × NetSym × Nat × List (Option Nat))}
    (h : PhysicalState.installResults plan left right rhs index state results =
      some next) :
    next.outputs.map PhysicalOutput.label =
      state.outputs.map PhysicalOutput.label := by
  induction results generalizing index state next with
  | nil =>
    simp [PhysicalState.installResults] at h
    subst next
    rfl
  | cons result rest ih =>
    cases hs : rhs.slots[index]? with
    | none => simp [PhysicalState.installResults, hs] at h
    | some slot =>
      by_cases hlab : rhs.lab slot.1 slot.2 = some result.2.2.1
      · cases hi : state.installResult plan
          (child left right slot.1 slot.2) result with
        | none =>
          simp [PhysicalState.installResults, hs, hlab, hi] at h
        | some installed =>
          have hrest : PhysicalState.installResults plan left right rhs
              (index + 1) installed rest = some next := by
            simpa [PhysicalState.installResults, hs, hlab, hi] using h
          exact (ih hrest).trans (PhysicalState.installResult_output_labels hi)
      · simp [PhysicalState.installResults, hs, hlab] at h

/-- Retiring either consumed arena slot cannot rename or remove an output. -/
private theorem PhysicalState.retire_output_labels
    {state next : PhysicalState} {index : Nat} {reuse : Bool}
    (h : state.retire index reuse = some next) :
    next.outputs.map PhysicalOutput.label =
      state.outputs.map PhysicalOutput.label := by
  by_cases hr : reuse
  · simp [PhysicalState.retire, hr] at h
    cases h
    rfl
  · cases ho : state.slots[index]? with
    | none => simp [PhysicalState.retire, hr, ho] at h
    | some old =>
      simp [PhysicalState.retire, hr, ho, PhysicalState.writeSlot] at h
      rcases h with ⟨_, heq⟩
      cases heq
      rfl

/-- Marking and appending one canonical active pair never changes the
external-output inventory, regardless of whether the queue was already
nonempty. -/
private theorem PhysicalState.publish_output_labels
    {state next : PhysicalState} {pair : PhysicalPair}
    (h : state.publish pair = some next) :
    next.outputs.map PhysicalOutput.label =
      state.outputs.map PhysicalOutput.label := by
  by_cases hp : pair.first >= pair.second
  · simp [PhysicalState.publish, hp] at h
  · cases hf : state.slots[pair.first]? with
    | none => simp [PhysicalState.publish, hp, hf] at h
    | some first =>
      cases hs : state.slots[pair.second]? with
      | none => simp [PhysicalState.publish, hp, hf, hs] at h
      | some second =>
        simp [PhysicalState.publish, hp, hf, hs,
          PhysicalState.writeSlot, PhysicalState.appendPublications] at h
        rcases h with ⟨_, heq⟩
        cases heq
        rfl

/-- Any finite successful publication batch preserves ordered output
identities even though it may change queued-generation marks and the queue. -/
private theorem PhysicalState.publishAll_output_labels
    {state next : PhysicalState} {pairs : List PhysicalPair}
    (h : state.publishAll pairs = some next) :
    next.outputs.map PhysicalOutput.label =
      state.outputs.map PhysicalOutput.label := by
  induction pairs generalizing state next with
  | nil =>
    simp [PhysicalState.publishAll] at h
    cases h
    rfl
  | cons pair rest ih =>
    cases hp : state.publish pair with
    | none => simp [PhysicalState.publishAll, hp] at h
    | some published =>
      have hrest : published.publishAll rest = some next := by
        simpa [PhysicalState.publishAll, hp] using h
      exact (ih hrest).trans (PhysicalState.publish_output_labels hp)

/-- A checked reciprocal write may mutate an external slot or retarget an
output, but it cannot change the output's stable abstract identity. -/
private theorem PhysicalState.writeExternal_output_labels
    {state next : PhysicalState} {plan : BytePlan}
    {write : Nat × Nat × Nat}
    (h : state.writeExternal plan write = some next) :
    next.outputs.map PhysicalOutput.label =
      state.outputs.map PhysicalOutput.label := by
  unfold PhysicalState.writeExternal at h
  cases hr : plan.results[write.1]? with
  | none => simp [hr] at h
  | some result =>
    cases hb : plan.boundaries[write.2.2]? with
    | none => simp [hr, hb] at h
    | some boundary =>
      by_cases hp : result.2.2.2.2[write.2.1]? = some (some write.2.2)
      · cases boundary with
        | mk physical tag =>
          cases tag with
          | mk kind peerAndPort =>
            cases peerAndPort with
            | mk peer port =>
              cases kind with
              | zero =>
                cases hs : state.slots[peer]? with
                | none => simp [hr, hb, hs] at h
                | some old =>
                  simp [hr, hb, hp, hs, PhysicalState.writeSlot] at h
                  rcases h with ⟨_, _, heq⟩
                  cases heq
                  rfl
              | succ kind =>
                cases kind with
                | zero =>
                  cases port with
                  | zero =>
                    cases ho : state.outputs[peer]? with
                    | none => simp [hr, hb, ho] at h
                    | some old =>
                      simp [hr, hb, hp, ho] at h
                      rcases h with ⟨_, hretarget⟩
                      exact PhysicalState.retargetOutput_preserves_free hretarget
                  | succ port => simp [hr, hb] at h
                | succ kind => simp [hr, hb] at h
      · simp [hr, hb, hp] at h

/-- All checked reciprocal boundary writes retain the original free-port
identity list, including arbitrary mixtures of peer slots and outputs. -/
private theorem PhysicalState.writeExternals_output_labels
    {state next : PhysicalState} {plan : BytePlan}
    {writes : List (Nat × Nat × Nat)}
    (h : state.writeExternals plan writes = some next) :
    next.outputs.map PhysicalOutput.label =
      state.outputs.map PhysicalOutput.label := by
  induction writes generalizing state next with
  | nil =>
    simp [PhysicalState.writeExternals] at h
    cases h
    rfl
  | cons write rest ih =>
    cases hw : state.writeExternal plan write with
    | none => simp [PhysicalState.writeExternals, hw] at h
    | some rewired =>
      have hrest : rewired.writeExternals plan rest = some next := by
        simpa [PhysicalState.writeExternals, hw] using h
      exact (ih hrest).trans (PhysicalState.writeExternal_output_labels hw)

/-- For any successfully decoded rule image, including arbitrary slot
placement, external peers and existing pending pairs, the physical executor
preserves the abstract free-port labels through its entire transaction. -/
theorem PhysicalState.executeCheckedImage_preserves_free
    {state next : PhysicalState} {bytes : List UInt8}
    {redex : XRedex NetSym}
    (h : state.executeCheckedImage bytes redex = some next) :
    next.project.free = state.project.free := by
  change next.outputs.map PhysicalOutput.label =
    state.outputs.map PhysicalOutput.label
  unfold PhysicalState.executeCheckedImage at h
  cases hd : BytePlan.decode bytes with
  | none => simp [hd] at h
  | some plan =>
    let pair : PhysicalPair :=
      { first := plan.pair.1
        firstGeneration := UInt64.ofNat plan.pair.2.1
        second := plan.pair.2.2.1
        secondGeneration := UInt64.ofNat plan.pair.2.2.2 }
    by_cases hp : state.logicalPair pair = some (redex.1, redex.2.1)
    · simp [hd, pair, hp] at h
      cases hi : PhysicalState.installResults plan redex.1 redex.2.1
          redex.2.2.2.2 0 state plan.results with
      | none => simp [hi] at h
      | some installed =>
        simp [hi] at h
        cases hw : installed.writeExternals plan plan.externalWrites with
        | none => simp [hw] at h
        | some rewired =>
          simp [hw] at h
          cases hf : rewired.retire plan.pair.1 (plan.action.1 == 1) with
          | none => simp [hf] at h
          | some retiredFirst =>
            simp [hf] at h
            cases hs : retiredFirst.retire plan.pair.2.2.1
                (plan.action.2 == 1) with
            | none => simp [hs] at h
            | some retiredSecond =>
              simp [hs] at h
              calc
                next.outputs.map PhysicalOutput.label =
                    retiredSecond.outputs.map PhysicalOutput.label :=
                  PhysicalState.publishAll_output_labels h
                _ = retiredFirst.outputs.map PhysicalOutput.label :=
                  PhysicalState.retire_output_labels hs
                _ = rewired.outputs.map PhysicalOutput.label :=
                  PhysicalState.retire_output_labels hf
                _ = installed.outputs.map PhysicalOutput.label :=
                  PhysicalState.writeExternals_output_labels hw
                _ = state.outputs.map PhysicalOutput.label :=
                  PhysicalState.installResults_output_labels hi
    · simp [hd, pair, hp] at h

/-- Installing a result does not modify the pending physical queue, even
when the result reuses one of the consumed physical slots. -/
private theorem PhysicalState.installResult_work
    {state next : PhysicalState} {plan : BytePlan} {logical : Id}
    {result : Bool × Nat × NetSym × Nat × List (Option Nat)}
    (h : state.installResult plan logical result = some next) :
    next.work = state.work := by
  unfold PhysicalState.installResult at h
  cases hd : state.slots[result.2.2.2.1]? with
  | none => simp [hd] at h
  | some donor =>
    cases ha : donor.agent with
    | none => simp [hd, ha] at h
    | some data =>
      cases hports : result.2.2.2.2.mapM plan.resultTarget with
      | none => simp [hd, hports] at h
      | some ports =>
        by_cases hfresh : result.1
        · simp [hd, ha, hports, hfresh, PhysicalState.appendSlot] at h
          rcases h with ⟨_, heq⟩
          cases heq
          rfl
        · cases ho : state.slots[result.2.1]? with
          | none => simp [hd, hports, hfresh, ho] at h
          | some old =>
            simp [hd, ha, hports, hfresh, ho,
              PhysicalState.writeSlot] at h
            rcases h with ⟨_, heq⟩
            cases heq
            rfl

/-- Any finite sequence of checked result installations leaves the existing
physical worklist in place, independently of slot allocation strategy. -/
private theorem PhysicalState.installResults_work
    {plan : BytePlan} {left right : Id} {rhs : XRhs NetSym}
    {index : Nat} {state next : PhysicalState}
    {results : List (Bool × Nat × NetSym × Nat × List (Option Nat))}
    (h : PhysicalState.installResults plan left right rhs index state results =
      some next) :
    next.work = state.work := by
  induction results generalizing index state next with
  | nil =>
    simp [PhysicalState.installResults] at h
    subst next
    rfl
  | cons result rest ih =>
    cases hs : rhs.slots[index]? with
    | none => simp [PhysicalState.installResults, hs] at h
    | some slot =>
      by_cases hlab : rhs.lab slot.1 slot.2 = some result.2.2.1
      · cases hi : state.installResult plan
          (child left right slot.1 slot.2) result with
        | none =>
          simp [PhysicalState.installResults, hs, hlab, hi] at h
        | some installed =>
          have hrest : PhysicalState.installResults plan left right rhs
              (index + 1) installed rest = some next := by
            simpa [PhysicalState.installResults, hs, hlab, hi] using h
          exact (ih hrest).trans (PhysicalState.installResult_work hi)
      · simp [PhysicalState.installResults, hs, hlab] at h

/-- Neither a consumed-slot retirement nor its free-list update can remove
or reorder an existing pending pair. -/
private theorem PhysicalState.retire_work
    {state next : PhysicalState} {index : Nat} {reuse : Bool}
    (h : state.retire index reuse = some next) :
    next.work = state.work := by
  by_cases hr : reuse
  · simp [PhysicalState.retire, hr] at h
    cases h
    rfl
  · cases ho : state.slots[index]? with
    | none => simp [PhysicalState.retire, hr, ho] at h
    | some old =>
      simp [PhysicalState.retire, hr, ho, PhysicalState.writeSlot] at h
      rcases h with ⟨_, heq⟩
      cases heq
      rfl

/-- An output retarget preserves the pending physical queue. -/
private theorem PhysicalState.retargetOutput_work
    {state next : PhysicalState} {index : Nat} {target : Nat × Nat}
    (h : state.retargetOutput index target = some next) :
    next.work = state.work := by
  unfold PhysicalState.retargetOutput at h
  cases ho : retargetOutputList index target state.outputs with
  | none => simp [ho] at h
  | some outputs =>
    simp [ho] at h
    cases h
    rfl

/-- A reciprocal external peer write or output retarget has no physical
worklist effect; publication is a distinct, later step. -/
private theorem PhysicalState.writeExternal_work
    {state next : PhysicalState} {plan : BytePlan}
    {write : Nat × Nat × Nat}
    (h : state.writeExternal plan write = some next) :
    next.work = state.work := by
  unfold PhysicalState.writeExternal at h
  cases hr : plan.results[write.1]? with
  | none => simp [hr] at h
  | some result =>
    cases hb : plan.boundaries[write.2.2]? with
    | none => simp [hr, hb] at h
    | some boundary =>
      by_cases hp : result.2.2.2.2[write.2.1]? = some (some write.2.2)
      · cases boundary with
        | mk physical tag =>
          cases tag with
          | mk kind peerAndPort =>
            cases peerAndPort with
            | mk peer port =>
              cases kind with
              | zero =>
                cases hs : state.slots[peer]? with
                | none => simp [hr, hb, hs] at h
                | some old =>
                  simp [hr, hb, hp, hs, PhysicalState.writeSlot] at h
                  rcases h with ⟨_, _, heq⟩
                  cases heq
                  rfl
              | succ kind =>
                cases kind with
                | zero =>
                  cases port with
                  | zero =>
                    cases ho : state.outputs[peer]? with
                    | none => simp [hr, hb, ho] at h
                    | some old =>
                      simp [hr, hb, hp, ho] at h
                      rcases h with ⟨_, hretarget⟩
                      exact PhysicalState.retargetOutput_work hretarget
                  | succ port => simp [hr, hb] at h
                | succ kind => simp [hr, hb] at h
      · simp [hr, hb, hp] at h

/-- Every finite sequence of reciprocal writes keeps all old physical
worklist entries, including entries unrelated to the selected pair. -/
private theorem PhysicalState.writeExternals_work
    {state next : PhysicalState} {plan : BytePlan}
    {writes : List (Nat × Nat × Nat)}
    (h : state.writeExternals plan writes = some next) :
    next.work = state.work := by
  induction writes generalizing state next with
  | nil =>
    simp [PhysicalState.writeExternals] at h
    cases h
    rfl
  | cons write rest ih =>
    cases hw : state.writeExternal plan write with
    | none => simp [PhysicalState.writeExternals, hw] at h
    | some rewired =>
      have hrest : rewired.writeExternals plan rest = some next := by
        simpa [PhysicalState.writeExternals, hw] using h
      exact (ih hrest).trans (PhysicalState.writeExternal_work hw)

/-- Publishing one checked physical pair appends it after every old entry;
the queue is not reordered by generation-mark writes. -/
private theorem PhysicalState.publish_work
    {state next : PhysicalState} {pair : PhysicalPair}
    (h : state.publish pair = some next) :
    next.work = state.work ++ [pair] := by
  by_cases hp : pair.first >= pair.second
  · simp [PhysicalState.publish, hp] at h
  · cases hf : state.slots[pair.first]? with
    | none => simp [PhysicalState.publish, hp, hf] at h
    | some first =>
      cases hs : state.slots[pair.second]? with
      | none => simp [PhysicalState.publish, hp, hf, hs] at h
      | some second =>
        simp [PhysicalState.publish, hp, hf, hs,
          PhysicalState.writeSlot, PhysicalState.appendPublications] at h
        rcases h with ⟨_, heq⟩
        cases heq
        rfl

/-- All successful publications append in byte-plan order, even if the
pre-existing pending worklist is nonempty. -/
private theorem PhysicalState.publishAll_work
    {state next : PhysicalState} {pairs : List PhysicalPair}
    (h : state.publishAll pairs = some next) :
    next.work = state.work ++ pairs := by
  induction pairs generalizing state next with
  | nil =>
    simp [PhysicalState.publishAll] at h
    cases h
    rfl
  | cons pair rest ih =>
    cases hp : state.publish pair with
    | none => simp [PhysicalState.publishAll, hp] at h
    | some published =>
      have hrest : published.publishAll rest = some next := by
        simpa [PhysicalState.publishAll, hp] using h
      calc
        next.work = published.work ++ rest := ih hrest
        _ = (state.work ++ [pair]) ++ rest := by rw [PhysicalState.publish_work hp]
        _ = state.work ++ (pair :: rest) := by simp [List.append_assoc]

/-- A successful physical execution preserves the entire old physical
worklist as a prefix and appends precisely the decoded publication tuples.
This is independent of the number of results, reused/fresh slots, rule
orientation, peer/output boundaries and prior pending entries. It says
nothing about whether the bytes were emitted by Rust or checker-admitted. -/
theorem PhysicalState.executeCheckedImage_work_append
    {state next : PhysicalState} {bytes : List UInt8}
    {redex : XRedex NetSym} {plan : BytePlan}
    (hdecode : BytePlan.decode bytes = some plan)
    (h : state.executeCheckedImage bytes redex = some next) :
    next.work = state.work ++
      plan.publications.map fun (first, firstGeneration, second, secondGeneration) =>
        { first, firstGeneration := UInt64.ofNat firstGeneration,
          second, secondGeneration := UInt64.ofNat secondGeneration : PhysicalPair } := by
  unfold PhysicalState.executeCheckedImage at h
  let pair : PhysicalPair :=
    { first := plan.pair.1
      firstGeneration := UInt64.ofNat plan.pair.2.1
      second := plan.pair.2.2.1
      secondGeneration := UInt64.ofNat plan.pair.2.2.2 }
  by_cases hp : state.logicalPair pair = some (redex.1, redex.2.1)
  · simp [hdecode, pair, hp] at h
    cases hi : PhysicalState.installResults plan redex.1 redex.2.1
        redex.2.2.2.2 0 state plan.results with
    | none => simp [hi] at h
    | some installed =>
      simp [hi] at h
      cases hw : installed.writeExternals plan plan.externalWrites with
      | none => simp [hw] at h
      | some rewired =>
        simp [hw] at h
        cases hf : rewired.retire plan.pair.1 (plan.action.1 == 1) with
        | none => simp [hf] at h
        | some retiredFirst =>
          simp [hf] at h
          cases hs : retiredFirst.retire plan.pair.2.2.1
              (plan.action.2 == 1) with
          | none => simp [hs] at h
          | some retiredSecond =>
            simp [hs] at h
            calc
              next.work = retiredSecond.work ++
                plan.publications.map fun (first, firstGeneration, second, secondGeneration) =>
                  { first, firstGeneration := UInt64.ofNat firstGeneration,
                    second, secondGeneration := UInt64.ofNat secondGeneration : PhysicalPair } :=
                PhysicalState.publishAll_work h
              _ = state.work ++
                plan.publications.map fun (first, firstGeneration, second, secondGeneration) =>
                  { first, firstGeneration := UInt64.ofNat firstGeneration,
                    second, secondGeneration := UInt64.ofNat secondGeneration : PhysicalPair } := by
                rw [PhysicalState.retire_work hs, PhysicalState.retire_work hf,
                  PhysicalState.writeExternals_work hw, PhysicalState.installResults_work hi]
  · simp [hdecode, pair, hp] at h

/-- An output-connected OP2/number instance with a selected active pair at
generation zero. External identities remain stable when its two targets move. -/
def op2PhysicalStart : PhysicalState :=
  { slots := #[
      { agent := some ([1], .op2 .add)
        generation := 0
        queuedGeneration := none
        ports := [some (1, 0), none, none] },
      { agent := some ([2], .num (BitVec.ofNat 64 27))
        generation := 0
        queuedGeneration := none
        ports := [some (0, 0)] }]
    freeSlots := []
    outputs := [
      { label := ([100], 0), target := (0, 1) },
      { label := ([101], 0), target := (0, 2) }]
    work := [] }

def op2PhysicalRedex : XRedex NetSym :=
  ([1], [2], .op2 .add, .num (BitVec.ofNat 64 27),
    (NetI64.op2NumData .add (BitVec.ofNat 64 27)).toX)

private def op2InputWires : Std.HashMap Port Port :=
  let m : Std.HashMap Port Port := {}
  let m := m.insert ([1], 0) ([2], 0)
  let m := m.insert ([2], 0) ([1], 0)
  let m := m.insert ([100], 0) ([1], 1)
  let m := m.insert ([1], 1) ([100], 0)
  let m := m.insert ([101], 0) ([1], 2)
  m.insert ([1], 2) ([101], 0)

private theorem op2_start_wires :
    op2PhysicalStart.wires = op2InputWires := by
  rfl

private theorem op2_nearWires :
    HNet.nearWires NetI64.arity op2PhysicalStart.project [1] [2]
      (.op2 .add) (.num (BitVec.ofNat 64 27)) =
      [ (([1], 0), ([2], 0)), (([1], 1), ([100], 0)),
        (([1], 2), ([101], 0)), (([2], 0), ([1], 0)) ] := by
  simp [HNet.nearWires, portsOf, NetI64.arity, PhysicalState.project,
    op2_start_wires, op2InputWires, hget_insert]
  decide

private theorem op2_plan_boundary :
    (Plan.compile op2PhysicalStart.project op2PhysicalRedex (by rfl)).capturedBoundary =
      [ (([1], 0), ([2], 0)), (([1], 1), ([100], 0)),
        (([1], 2), ([101], 0)), (([2], 0), ([1], 0)) ] := by
  exact op2_nearWires

private theorem op2_nbhd :
    HNet.nbhd NetI64.arity op2PhysicalStart.project [1] [2]
      (.op2 .add) (.num (BitVec.ofNat 64 27)) =
      { agents := [([1], .op2 .add), ([2], .num (BitVec.ofNat 64 27))]
        free := [([100], 0), ([101], 0)]
        wires :=
          [ (([1], 0), ([2], 0)), (([1], 1), ([100], 0)),
            (([1], 2), ([101], 0)), (([2], 0), ([1], 0)),
            (([2], 0), ([1], 0)), (([100], 0), ([1], 1)),
            (([101], 0), ([1], 2)), (([1], 0), ([2], 0)) ] } := by
  simp [HNet.nbhd, op2_nearWires]

private theorem op2_plan_agents :
    (Plan.compile op2PhysicalStart.project op2PhysicalRedex (by rfl)).initializedAgents =
      [(child [1] [2] (some false) 0, .op1 .add (BitVec.ofNat 64 27))] := by
  decide

private theorem op2_plan_writes :
    (Plan.compile op2PhysicalStart.project op2PhysicalRedex (by rfl)).wireWrites =
      [ ((child [1] [2] (some false) 0, 0), ([100], 0)),
        ((child [1] [2] (some false) 0, 1), ([101], 0)),
        (([100], 0), (child [1] [2] (some false) 0, 0)),
        (([101], 0), (child [1] [2] (some false) 0, 1)) ] := by
  change
    ((HNet.nbhd NetI64.arity op2PhysicalStart.project [1] [2]
        (.op2 .add) (.num (BitVec.ofNat 64 27))).fire
        NetI64.arity [1] [2] (.op2 .add) (.num (BitVec.ofNat 64 27))
        (NetI64.op2NumData .add (BitVec.ofNat 64 27)).toX).wires = _
  rw [op2_nbhd]
  set_option maxRecDepth 3000 in decide

/-- Expected result of executing the decoded OP2 image, including the
generation increment, freed consumed slot and both exact output retargets. -/
def op2PhysicalAfter : PhysicalState :=
  { slots := #[
      { agent := some (child [1] [2] (some false) 0,
          .op1 .add (BitVec.ofNat 64 27))
        generation := 1
        queuedGeneration := none
        ports := [none, none, none] },
      { agent := none
        generation := 1
        queuedGeneration := none
        ports := [some (0, 0)] }]
    freeSlots := [1]
    outputs := [
      { label := ([100], 0), target := (0, 0) },
      { label := ([101], 0), target := (0, 1) }]
    work := [] }

/-- A decoded checked OP2 image really installs and retires its physical
slots and retargets both ordered outputs in the sequential arena model. -/
theorem PhysicalState.op2_output_execution_exact :
    op2PhysicalStart.executeCheckedImage op2OutputFixture.emit
      op2PhysicalRedex = some op2PhysicalAfter := by
  set_option maxRecDepth 2000 in decide

/-- The decoded OP2 image's physical transaction represents the selected
revision-3 net rewrite for this output-connected finite instance. -/
theorem PhysicalState.op2_output_refines :
    op2PhysicalAfter.project.toNet =
      (Plan.commit op2PhysicalStart.project
        (Plan.compile op2PhysicalStart.project op2PhysicalRedex (by rfl))
        []).toNet := by
  apply Net.ext'
  · intro i γ
    simp only [HNet.toNet, Plan.commit, op2_plan_agents]
    simp [Plan.compile, op2PhysicalRedex, PhysicalState.project,
      PhysicalState.agents, op2PhysicalStart, op2PhysicalAfter,
      hget_insert, hget_erase]
    by_cases hnew : i = child [1] [2] (some false) 0 <;>
      by_cases hleft : i = [1] <;>
      by_cases hright : i = [2] <;>
      simp_all [child]
  · intro p q
    simp only [HNet.toNet, Plan.commit, op2_plan_boundary, op2_plan_writes]
    simp [PhysicalState.project, PhysicalState.wires,
      PhysicalState.portWires, op2PhysicalStart, op2PhysicalAfter,
      PhysicalState.logicalPort, hget_insert, hget_erase]
    rw [show child [1] [2] (some false) 0 = [0, 1] from rfl]
    by_cases hnew0 : p = ([0, 1], 0)
    · subst p; simp
    by_cases hnew1 : p = ([0, 1], 1)
    · subst p; simp
    by_cases hout0 : p = ([100], 0)
    · subst p; simp
    by_cases hout1 : p = ([101], 0)
    · subst p; simp
    by_cases hleft0 : p = ([1], 0)
    · subst p; simp
    by_cases hleft1 : p = ([1], 1)
    · subst p; simp
    by_cases hleft2 : p = ([1], 2)
    · subst p; simp
    by_cases hright0 : p = ([2], 0)
    · subst p; simp
    have hnew0' : ([0, 1], 0) ≠ p := Ne.symm hnew0
    have hnew1' : ([0, 1], 1) ≠ p := Ne.symm hnew1
    have hout0' : ([100], 0) ≠ p := Ne.symm hout0
    have hout1' : ([101], 0) ≠ p := Ne.symm hout1
    have hleft0' : ([1], 0) ≠ p := Ne.symm hleft0
    have hleft1' : ([1], 1) ≠ p := Ne.symm hleft1
    have hleft2' : ([1], 2) ≠ p := Ne.symm hleft2
    have hright0' : ([2], 0) ≠ p := Ne.symm hright0
    simp [hnew0', hnew1', hout0', hout1', hleft0', hleft1',
      hleft2', hright0']
  · intro p
    rfl

/-- A nonempty publication: the resulting OP1's principal is connected to a
third live number, while its auxiliary still points at an ordered output. -/
def op2PublishedStart : PhysicalState :=
  { slots := #[
      { agent := some ([1], .op2 .add)
        generation := 0
        queuedGeneration := none
        ports := [some (1, 0), some (2, 0), none] },
      { agent := some ([2], .num (BitVec.ofNat 64 27))
        generation := 0
        queuedGeneration := none
        ports := [some (0, 0)] },
      { agent := some ([3], .num (BitVec.ofNat 64 9))
        generation := 0
        queuedGeneration := none
        ports := [some (0, 1)] }]
    freeSlots := []
    outputs := [{ label := ([100], 0), target := (0, 2) }]
    work := [] }

private def op2PublishedPair : PhysicalPair :=
  { first := 0
    firstGeneration := 1
    second := 2
    secondGeneration := 0 }

def op2PublishedAfter : PhysicalState :=
  { slots := #[
      { agent := some (child [1] [2] (some false) 0,
          .op1 .add (BitVec.ofNat 64 27))
        generation := 1
        queuedGeneration := some 1
        ports := [some (2, 0), none, none] },
      { agent := none
        generation := 1
        queuedGeneration := none
        ports := [some (0, 0)] },
      { agent := some ([3], .num (BitVec.ofNat 64 9))
        generation := 0
        queuedGeneration := some 0
        ports := [some (0, 0)] }]
    freeSlots := [1]
    outputs := [{ label := ([100], 0), target := (0, 1) }]
    work := [op2PublishedPair] }

theorem PhysicalState.op2_published_execution_exact :
    op2PublishedStart.executeCheckedImage op2PublishedFixture.emit
      op2PhysicalRedex = some op2PublishedAfter := by
  set_option maxRecDepth 3000 in decide

theorem PhysicalState.op2_published_pair_orientations :
    op2PublishedAfter.project.work =
      [(child [1] [2] (some false) 0, [3]),
       ([3], child [1] [2] (some false) 0)] := by
  decide

private theorem op2_published_nearWires :
    HNet.nearWires NetI64.arity op2PublishedStart.project [1] [2]
      (.op2 .add) (.num (BitVec.ofNat 64 27)) =
      [ (([1], 0), ([2], 0)), (([1], 1), ([3], 0)),
        (([1], 2), ([100], 0)), (([2], 0), ([1], 0)) ] := by
  simp [HNet.nearWires, portsOf, NetI64.arity, PhysicalState.project,
    PhysicalState.wires, PhysicalState.portWires, PhysicalState.logicalPort,
    op2PublishedStart, hget_insert]
  decide

private theorem op2_published_nbhd :
    HNet.nbhd NetI64.arity op2PublishedStart.project [1] [2]
      (.op2 .add) (.num (BitVec.ofNat 64 27)) =
      { agents := [([1], .op2 .add), ([2], .num (BitVec.ofNat 64 27))]
        free := [([3], 0), ([100], 0)]
        wires :=
          [ (([1], 0), ([2], 0)), (([1], 1), ([3], 0)),
            (([1], 2), ([100], 0)), (([2], 0), ([1], 0)),
            (([2], 0), ([1], 0)), (([3], 0), ([1], 1)),
            (([100], 0), ([1], 2)), (([1], 0), ([2], 0)) ] } := by
  simp [HNet.nbhd, op2_published_nearWires]

private theorem op2_published_plan_agents :
    (Plan.compile op2PublishedStart.project op2PhysicalRedex (by rfl)).initializedAgents =
      [(child [1] [2] (some false) 0,
        .op1 .add (BitVec.ofNat 64 27))] := by
  decide

private theorem op2_published_plan_boundary :
    (Plan.compile op2PublishedStart.project op2PhysicalRedex (by rfl)).capturedBoundary =
      [ (([1], 0), ([2], 0)), (([1], 1), ([3], 0)),
        (([1], 2), ([100], 0)), (([2], 0), ([1], 0)) ] :=
  op2_published_nearWires

private theorem op2_published_plan_writes :
    (Plan.compile op2PublishedStart.project op2PhysicalRedex (by rfl)).wireWrites =
      [ ((child [1] [2] (some false) 0, 0), ([3], 0)),
        ((child [1] [2] (some false) 0, 1), ([100], 0)),
        (([3], 0), (child [1] [2] (some false) 0, 0)),
        (([100], 0), (child [1] [2] (some false) 0, 1)) ] := by
  change
    ((HNet.nbhd NetI64.arity op2PublishedStart.project [1] [2]
      (.op2 .add) (.num (BitVec.ofNat 64 27))).fire
      NetI64.arity [1] [2] (.op2 .add) (.num (BitVec.ofNat 64 27))
      (NetI64.op2NumData .add (BitVec.ofNat 64 27)).toX).wires = _
  rw [op2_published_nbhd]
  set_option maxRecDepth 3000 in decide

private theorem op2_published_plan_pair :
    (Plan.compile op2PublishedStart.project op2PhysicalRedex (by rfl)).newlyActive =
      [(child [1] [2] (some false) 0, [3]),
       ([3], child [1] [2] (some false) 0)] := by
  change principalPairs
    ((HNet.nbhd NetI64.arity op2PublishedStart.project [1] [2]
      (.op2 .add) (.num (BitVec.ofNat 64 27))).fire
      NetI64.arity [1] [2] (.op2 .add) (.num (BitVec.ofNat 64 27))
      (NetI64.op2NumData .add (BitVec.ofNat 64 27)).toX).wires = _
  rw [op2_published_nbhd]
  set_option maxRecDepth 3000 in decide

theorem PhysicalState.op2_published_refines :
    op2PublishedAfter.project.toNet =
      (Plan.commit op2PublishedStart.project
        (Plan.compile op2PublishedStart.project op2PhysicalRedex (by rfl))
        []).toNet ∧
    op2PublishedAfter.project.work.Perm
      (Plan.commit op2PublishedStart.project
        (Plan.compile op2PublishedStart.project op2PhysicalRedex (by rfl))
        []).work := by
  constructor
  · apply Net.ext'
    · intro i γ
      simp only [HNet.toNet, Plan.commit, op2_published_plan_agents]
      simp [Plan.compile, op2PhysicalRedex, PhysicalState.project,
        PhysicalState.agents, op2PublishedStart, op2PublishedAfter,
        hget_insert, hget_erase]
      rw [show child [1] [2] (some false) 0 = [0, 1] from rfl]
      by_cases hnew : i = [0, 1] <;>
        by_cases hleft : i = [1] <;>
        by_cases hright : i = [2] <;>
        by_cases hpeer : i = [3] <;>
        simp_all
      have hnew' : [0, 1] ≠ i := Ne.symm hnew
      have hleft' : [1] ≠ i := Ne.symm hleft
      have hright' : [2] ≠ i := Ne.symm hright
      have hpeer' : [3] ≠ i := Ne.symm hpeer
      simp [hnew', hleft', hright', hpeer']
    · intro p q
      simp only [HNet.toNet, Plan.commit,
        op2_published_plan_boundary, op2_published_plan_writes]
      simp [PhysicalState.project, PhysicalState.wires,
        PhysicalState.portWires, op2PublishedStart, op2PublishedAfter,
        PhysicalState.logicalPort, hget_insert, hget_erase]
      rw [show child [1] [2] (some false) 0 = [0, 1] from rfl]
      by_cases hnew0 : p = ([0, 1], 0)
      · subst p; simp
      by_cases hnew1 : p = ([0, 1], 1)
      · subst p; simp
      by_cases hpeer : p = ([3], 0)
      · subst p; simp
      by_cases hout : p = ([100], 0)
      · subst p; simp
      by_cases hleft0 : p = ([1], 0)
      · subst p; simp
      by_cases hleft1 : p = ([1], 1)
      · subst p; simp
      by_cases hleft2 : p = ([1], 2)
      · subst p; simp
      by_cases hright0 : p = ([2], 0)
      · subst p; simp
      have hnew0' : ([0, 1], 0) ≠ p := Ne.symm hnew0
      have hnew1' : ([0, 1], 1) ≠ p := Ne.symm hnew1
      have hpeer' : ([3], 0) ≠ p := Ne.symm hpeer
      have hout' : ([100], 0) ≠ p := Ne.symm hout
      have hleft0' : ([1], 0) ≠ p := Ne.symm hleft0
      have hleft1' : ([1], 1) ≠ p := Ne.symm hleft1
      have hleft2' : ([1], 2) ≠ p := Ne.symm hleft2
      have hright0' : ([2], 0) ≠ p := Ne.symm hright0
      simp [hnew0', hnew1', hpeer', hout', hleft0', hleft1',
        hleft2', hright0']
    · intro p
      rfl
  · rw [PhysicalState.op2_published_pair_orientations]
    simp only [Plan.commit, op2_published_plan_pair, List.append_nil]
    exact List.Perm.refl _

/-- The zero-result erasure image has no boundaries and retires both
generation-zero slots without leaving a live agent or active pair. -/
def eraPhysicalStart : PhysicalState :=
  { slots := #[
      { agent := some ([1], .era)
        generation := 0
        queuedGeneration := none
        ports := [some (1, 0)] },
      { agent := some ([2], .num (BitVec.ofNat 64 42))
        generation := 0
        queuedGeneration := none
        ports := [some (0, 0)] }]
    freeSlots := []
    outputs := []
    work := [] }

def eraPhysicalRedex : XRedex NetSym :=
  ([1], [2], .era, .num (BitVec.ofNat 64 42),
    NetI64.eraNumData.toX)

def eraPhysicalAfter : PhysicalState :=
  { slots := #[
      { agent := none
        generation := 1
        queuedGeneration := none
        ports := [some (1, 0)] },
      { agent := none
        generation := 1
        queuedGeneration := none
        ports := [some (0, 0)] }]
    freeSlots := [1, 0]
    outputs := []
    work := [] }

private theorem era_nearWires :
    HNet.nearWires NetI64.arity eraPhysicalStart.project [1] [2]
      .era (.num (BitVec.ofNat 64 42)) =
      [ (([1], 0), ([2], 0)), (([2], 0), ([1], 0)) ] := by
  simp [HNet.nearWires, portsOf, NetI64.arity,
    PhysicalState.project, PhysicalState.wires, PhysicalState.portWires,
    PhysicalState.logicalPort, eraPhysicalStart, hget_insert]

private theorem era_nbhd :
    HNet.nbhd NetI64.arity eraPhysicalStart.project [1] [2]
      .era (.num (BitVec.ofNat 64 42)) =
      { agents := [([1], .era), ([2], .num (BitVec.ofNat 64 42))]
        free := []
        wires := [ (([1], 0), ([2], 0)), (([2], 0), ([1], 0)),
          (([2], 0), ([1], 0)), (([1], 0), ([2], 0)) ] } := by
  simp [HNet.nbhd, era_nearWires]

private theorem era_plan_agents :
    (Plan.compile eraPhysicalStart.project eraPhysicalRedex (by rfl)).initializedAgents =
      [] := by
  change
    ((HNet.nbhd NetI64.arity eraPhysicalStart.project [1] [2]
      .era (.num (BitVec.ofNat 64 42))).fire
      NetI64.arity [1] [2] .era (.num (BitVec.ofNat 64 42))
      NetI64.eraNumData.toX).agents = []
  rw [era_nbhd]
  decide

private theorem era_plan_boundary :
    (Plan.compile eraPhysicalStart.project eraPhysicalRedex (by rfl)).capturedBoundary =
      [ (([1], 0), ([2], 0)), (([2], 0), ([1], 0)) ] :=
  era_nearWires

private theorem era_plan_writes :
    (Plan.compile eraPhysicalStart.project eraPhysicalRedex (by rfl)).wireWrites =
      [] := by
  change
    ((HNet.nbhd NetI64.arity eraPhysicalStart.project [1] [2]
      .era (.num (BitVec.ofNat 64 42))).fire
      NetI64.arity [1] [2] .era (.num (BitVec.ofNat 64 42))
      NetI64.eraNumData.toX).wires = []
  rw [era_nbhd]
  decide

theorem PhysicalState.era_execution_exact :
    eraPhysicalStart.executeCheckedImage eraPlanBytes eraPhysicalRedex =
      some eraPhysicalAfter := by
  set_option maxRecDepth 2000 in decide

/-- Erasure's physical image represents the empty abstract result, with
no residual principal wire or stale free-slot alias. -/
theorem PhysicalState.era_refines :
    eraPhysicalAfter.project.toNet =
      (Plan.commit eraPhysicalStart.project
        (Plan.compile eraPhysicalStart.project eraPhysicalRedex (by rfl))
        []).toNet := by
  apply Net.ext'
  · intro i γ
    simp only [HNet.toNet, Plan.commit, era_plan_agents]
    simp [Plan.compile, eraPhysicalRedex,
      PhysicalState.project, PhysicalState.agents, eraPhysicalStart,
      eraPhysicalAfter, hget_erase]
    intro hright hleft
    simp [hright, hleft]
  · intro p q
    simp only [HNet.toNet, Plan.commit, era_plan_boundary, era_plan_writes]
    simp [PhysicalState.project,
      PhysicalState.wires, PhysicalState.portWires,
      PhysicalState.logicalPort, eraPhysicalStart, eraPhysicalAfter,
      hget_insert, hget_erase]
    intro hleft hright hleft'
    simp [hleft, hright]
  · intro p
    rfl

/-- A checked OP1 subtraction output image replaces the consumed operation
with the wrapped I64 result and frees the second slot. -/
def op1PhysicalStart : PhysicalState :=
  { slots := #[
      { agent := some ([1], .op1 .sub (BitVec.ofNat 64 7))
        generation := 0
        queuedGeneration := none
        ports := [some (1, 0), none] },
      { agent := some ([2], .num (BitVec.ofNat 64 13))
        generation := 0
        queuedGeneration := none
        ports := [some (0, 0)] }]
    freeSlots := []
    outputs := [{ label := ([100], 0), target := (0, 1) }]
    work := [] }

def op1PhysicalRedex : XRedex NetSym :=
  ([1], [2], .op1 .sub (BitVec.ofNat 64 7),
    .num (BitVec.ofNat 64 13),
    (NetI64.op1NumData .sub (BitVec.ofNat 64 7)
      (BitVec.ofNat 64 13)).toX)

def op1PhysicalAfter : PhysicalState :=
  { slots := #[
      { agent := some (child [1] [2] (some false) 0,
          .num (BitVec.ofInt 64 (-6)))
        generation := 1
        queuedGeneration := none
        ports := [none, none, none] },
      { agent := none
        generation := 1
        queuedGeneration := none
        ports := [some (0, 0)] }]
    freeSlots := [1]
    outputs := [{ label := ([100], 0), target := (0, 0) }]
    work := [] }

theorem PhysicalState.op1_wrapping_execution_exact :
    op1PhysicalStart.executeCheckedImage op1OutputFixture.emit
      op1PhysicalRedex = some op1PhysicalAfter := by
  set_option maxRecDepth 2000 in decide

private theorem op1_nearWires :
    HNet.nearWires NetI64.arity op1PhysicalStart.project [1] [2]
      (.op1 .sub (BitVec.ofNat 64 7))
      (.num (BitVec.ofNat 64 13)) =
      [ (([1], 0), ([2], 0)), (([1], 1), ([100], 0)),
        (([2], 0), ([1], 0)) ] := by
  simp [HNet.nearWires, portsOf, NetI64.arity, PhysicalState.project,
    PhysicalState.wires, PhysicalState.portWires, PhysicalState.logicalPort,
    op1PhysicalStart, hget_insert]
  decide

private theorem op1_nbhd :
    HNet.nbhd NetI64.arity op1PhysicalStart.project [1] [2]
      (.op1 .sub (BitVec.ofNat 64 7))
      (.num (BitVec.ofNat 64 13)) =
      { agents := [([1], .op1 .sub (BitVec.ofNat 64 7)),
          ([2], .num (BitVec.ofNat 64 13))]
        free := [([100], 0)]
        wires := [ (([1], 0), ([2], 0)), (([1], 1), ([100], 0)),
          (([2], 0), ([1], 0)), (([2], 0), ([1], 0)),
          (([100], 0), ([1], 1)), (([1], 0), ([2], 0)) ] } := by
  simp [HNet.nbhd, op1_nearWires]

private theorem op1_plan_agents :
    (Plan.compile op1PhysicalStart.project op1PhysicalRedex (by rfl)).initializedAgents =
      [(child [1] [2] (some false) 0, .num (BitVec.ofInt 64 (-6)))] := by
  decide

private theorem op1_plan_boundary :
    (Plan.compile op1PhysicalStart.project op1PhysicalRedex (by rfl)).capturedBoundary =
      [ (([1], 0), ([2], 0)), (([1], 1), ([100], 0)),
        (([2], 0), ([1], 0)) ] :=
  op1_nearWires

private theorem op1_plan_writes :
    (Plan.compile op1PhysicalStart.project op1PhysicalRedex (by rfl)).wireWrites =
      [ ((child [1] [2] (some false) 0, 0), ([100], 0)),
        (([100], 0), (child [1] [2] (some false) 0, 0)) ] := by
  change
    ((HNet.nbhd NetI64.arity op1PhysicalStart.project [1] [2]
      (.op1 .sub (BitVec.ofNat 64 7)) (.num (BitVec.ofNat 64 13))).fire
      NetI64.arity [1] [2] (.op1 .sub (BitVec.ofNat 64 7))
      (.num (BitVec.ofNat 64 13))
      (NetI64.op1NumData .sub (BitVec.ofNat 64 7)
        (BitVec.ofNat 64 13)).toX).wires = _
  rw [op1_nbhd]
  set_option maxRecDepth 3000 in decide

theorem PhysicalState.op1_wrapping_refines :
    op1PhysicalAfter.project.toNet =
      (Plan.commit op1PhysicalStart.project
        (Plan.compile op1PhysicalStart.project op1PhysicalRedex (by rfl))
        []).toNet := by
  apply Net.ext'
  · intro i γ
    simp only [HNet.toNet, Plan.commit, op1_plan_agents]
    simp [Plan.compile, op1PhysicalRedex, PhysicalState.project,
      PhysicalState.agents, op1PhysicalStart, op1PhysicalAfter,
      hget_insert, hget_erase]
    by_cases hnew : i = child [1] [2] (some false) 0 <;>
      by_cases hleft : i = [1] <;>
      by_cases hright : i = [2] <;>
      simp_all [child]
  · intro p q
    simp only [HNet.toNet, Plan.commit, op1_plan_boundary, op1_plan_writes]
    simp [PhysicalState.project, PhysicalState.wires,
      PhysicalState.portWires, op1PhysicalStart, op1PhysicalAfter,
      PhysicalState.logicalPort, hget_insert, hget_erase]
    rw [show child [1] [2] (some false) 0 = [0, 1] from rfl]
    by_cases hnew : p = ([0, 1], 0)
    · subst p; simp
    by_cases hout : p = ([100], 0)
    · subst p; simp
    by_cases hleft0 : p = ([1], 0)
    · subst p; simp
    by_cases hleft1 : p = ([1], 1)
    · subst p; simp
    by_cases hright0 : p = ([2], 0)
    · subst p; simp
    have hnew' : ([0, 1], 0) ≠ p := Ne.symm hnew
    have hout' : ([100], 0) ≠ p := Ne.symm hout
    have hleft0' : ([1], 0) ≠ p := Ne.symm hleft0
    have hleft1' : ([1], 1) ≠ p := Ne.symm hleft1
    have hright0' : ([2], 0) ≠ p := Ne.symm hright0
    simp [hnew', hout', hleft0', hleft1', hright0']
  · intro p
    rfl

/-- Two-result DUP stages both replacement numbers in the original physical
slots and preserves each output's stable logical label separately. -/
def dupPhysicalStart : PhysicalState :=
  { slots := #[
      { agent := some ([1], .dup)
        generation := 0
        queuedGeneration := none
        ports := [some (1, 0), none, none] },
      { agent := some ([2], .num (BitVec.ofNat 64 42))
        generation := 0
        queuedGeneration := none
        ports := [some (0, 0)] }]
    freeSlots := []
    outputs := [
      { label := ([100], 0), target := (0, 1) },
      { label := ([101], 0), target := (0, 2) }]
    work := [] }

def dupPhysicalRedex : XRedex NetSym :=
  ([1], [2], .dup, .num (BitVec.ofNat 64 42),
    (NetI64.dupNumData (BitVec.ofNat 64 42)).toX)

def dupPhysicalAfter : PhysicalState :=
  { slots := #[
      { agent := some (child [1] [2] (some false) 0,
          .num (BitVec.ofNat 64 42))
        generation := 1
        queuedGeneration := none
        ports := [none, none, none] },
      { agent := some (child [1] [2] (some false) 1,
          .num (BitVec.ofNat 64 42))
        generation := 1
        queuedGeneration := none
        ports := [none, none, none] }]
    freeSlots := []
    outputs := [
      { label := ([100], 0), target := (0, 0) },
      { label := ([101], 0), target := (1, 0) }]
    work := [] }

theorem PhysicalState.dup_execution_exact :
    dupPhysicalStart.executeCheckedImage dupOutputFixture.emit
      dupPhysicalRedex = some dupPhysicalAfter := by
  set_option maxRecDepth 3000 in decide

private theorem dup_nearWires :
    HNet.nearWires NetI64.arity dupPhysicalStart.project [1] [2]
      .dup (.num (BitVec.ofNat 64 42)) =
      [ (([1], 0), ([2], 0)), (([1], 1), ([100], 0)),
        (([1], 2), ([101], 0)), (([2], 0), ([1], 0)) ] := by
  simp [HNet.nearWires, portsOf, NetI64.arity, PhysicalState.project,
    PhysicalState.wires, PhysicalState.portWires, PhysicalState.logicalPort,
    dupPhysicalStart, hget_insert]
  decide

private theorem dup_nbhd :
    HNet.nbhd NetI64.arity dupPhysicalStart.project [1] [2]
      .dup (.num (BitVec.ofNat 64 42)) =
      { agents := [([1], .dup), ([2], .num (BitVec.ofNat 64 42))]
        free := [([100], 0), ([101], 0)]
        wires :=
          [ (([1], 0), ([2], 0)), (([1], 1), ([100], 0)),
            (([1], 2), ([101], 0)), (([2], 0), ([1], 0)),
            (([2], 0), ([1], 0)), (([100], 0), ([1], 1)),
            (([101], 0), ([1], 2)), (([1], 0), ([2], 0)) ] } := by
  simp [HNet.nbhd, dup_nearWires]

private theorem dup_plan_agents :
    (Plan.compile dupPhysicalStart.project dupPhysicalRedex (by rfl)).initializedAgents =
      [(child [1] [2] (some false) 0, .num (BitVec.ofNat 64 42)),
       (child [1] [2] (some false) 1, .num (BitVec.ofNat 64 42))] := by
  decide

private theorem dup_plan_boundary :
    (Plan.compile dupPhysicalStart.project dupPhysicalRedex (by rfl)).capturedBoundary =
      [ (([1], 0), ([2], 0)), (([1], 1), ([100], 0)),
        (([1], 2), ([101], 0)), (([2], 0), ([1], 0)) ] :=
  dup_nearWires

private theorem dup_plan_writes :
    (Plan.compile dupPhysicalStart.project dupPhysicalRedex (by rfl)).wireWrites =
      [ ((child [1] [2] (some false) 0, 0), ([100], 0)),
        ((child [1] [2] (some false) 1, 0), ([101], 0)),
        (([100], 0), (child [1] [2] (some false) 0, 0)),
        (([101], 0), (child [1] [2] (some false) 1, 0)) ] := by
  change
    ((HNet.nbhd NetI64.arity dupPhysicalStart.project [1] [2]
      .dup (.num (BitVec.ofNat 64 42))).fire
      NetI64.arity [1] [2] .dup (.num (BitVec.ofNat 64 42))
      (NetI64.dupNumData (BitVec.ofNat 64 42)).toX).wires = _
  rw [dup_nbhd]
  set_option maxRecDepth 3000 in decide

theorem PhysicalState.dup_refines :
    dupPhysicalAfter.project.toNet =
      (Plan.commit dupPhysicalStart.project
        (Plan.compile dupPhysicalStart.project dupPhysicalRedex (by rfl))
        []).toNet := by
  apply Net.ext'
  · intro i γ
    simp only [HNet.toNet, Plan.commit, dup_plan_agents]
    simp [Plan.compile, dupPhysicalRedex, PhysicalState.project,
      PhysicalState.agents, dupPhysicalStart, dupPhysicalAfter,
      hget_insert, hget_erase]
    rw [show child [1] [2] (some false) 0 = [0, 1] from rfl,
        show child [1] [2] (some false) 1 = [2, 1] from rfl]
    by_cases hnew0 : i = [0, 1] <;>
      by_cases hnew1 : i = [2, 1] <;>
      by_cases hleft : i = [1] <;>
      by_cases hright : i = [2] <;>
      simp_all
  · intro p q
    simp only [HNet.toNet, Plan.commit, dup_plan_boundary, dup_plan_writes]
    simp [PhysicalState.project, PhysicalState.wires,
      PhysicalState.portWires, dupPhysicalStart, dupPhysicalAfter,
      PhysicalState.logicalPort, hget_insert, hget_erase]
    rw [show child [1] [2] (some false) 0 = [0, 1] from rfl,
        show child [1] [2] (some false) 1 = [2, 1] from rfl]
    by_cases hnew0 : p = ([0, 1], 0)
    · subst p; simp
    by_cases hnew1 : p = ([2, 1], 0)
    · subst p; simp
    by_cases hout0 : p = ([100], 0)
    · subst p; simp
    by_cases hout1 : p = ([101], 0)
    · subst p; simp
    by_cases hleft0 : p = ([1], 0)
    · subst p; simp
    by_cases hleft1 : p = ([1], 1)
    · subst p; simp
    by_cases hleft2 : p = ([1], 2)
    · subst p; simp
    by_cases hright0 : p = ([2], 0)
    · subst p; simp
    have hnew0' : ([0, 1], 0) ≠ p := Ne.symm hnew0
    have hnew1' : ([2, 1], 0) ≠ p := Ne.symm hnew1
    have hout0' : ([100], 0) ≠ p := Ne.symm hout0
    have hout1' : ([101], 0) ≠ p := Ne.symm hout1
    have hleft0' : ([1], 0) ≠ p := Ne.symm hleft0
    have hleft1' : ([1], 1) ≠ p := Ne.symm hleft1
    have hleft2' : ([1], 2) ≠ p := Ne.symm hleft2
    have hright0' : ([2], 0) ≠ p := Ne.symm hright0
    simp [hnew0', hnew1', hout0', hout1', hleft0', hleft1',
      hleft2', hright0']
  · intro p
    rfl

/-- Ordered external free-port identities survive a physical slot write,
including one that retags a slot formerly targeted by an output. Only the
physical output target may change, never the external free-port identity. -/
theorem PhysicalState.project_boundary_frame {state : PhysicalState}
    {index : Nat} {replacement : PhysicalSlot} :
    (state.writeSlot index replacement).project.free = state.project.free := by
  rfl

/-- The ordered active-pair worklist is unchanged by a write away from both
slots named by every queued pair. -/
theorem PhysicalState.project_work_frame {state : PhysicalState}
    {index : Nat} {replacement : PhysicalSlot}
    (outside : ∀ pair ∈ state.work,
      index ≠ pair.first ∧ index ≠ pair.second) :
    (state.writeSlot index replacement).project.work = state.project.work := by
  have preserve : ∀ pairs : List PhysicalPair,
      (∀ pair ∈ pairs, index ≠ pair.first ∧ index ≠ pair.second) →
      pairs.flatMap (state.writeSlot index replacement).pairOrientations =
        pairs.flatMap state.pairOrientations := by
    intro pairs
    induction pairs with
    | nil => intro _; rfl
    | cons pair rest ih =>
      intro hpairs
      have hpair := hpairs pair (by simp)
      have hrest := ih (by
        intro entry he
        exact hpairs entry (List.mem_cons_of_mem _ he))
      simp only [List.flatMap_cons]
      rw [show (state.writeSlot index replacement).pairOrientations pair =
        state.pairOrientations pair by
          simp [PhysicalState.pairOrientations,
            PhysicalState.logicalPair_writeSlot_away hpair.1 hpair.2], hrest]
  exact preserve state.work outside

/-- The only fields of a physical slot used to decode a pending active pair.
Port retargets and queued-generation marks are deliberately absent. -/
def PhysicalState.pairSlotView (state : PhysicalState)
    (index : Nat) : Option (UInt64 × Option Id) :=
  (state.slots[index]?).map fun slot =>
    (slot.generation, slot.agent.map Prod.fst)

/-- Pending-pair representation is stable when its two slot generations and
agent identities are unchanged; other physical fields may differ. -/
def PhysicalState.SamePairSlots (before after : PhysicalState)
    (pair : PhysicalPair) : Prop :=
  before.pairSlotView pair.first = after.pairSlotView pair.first ∧
    before.pairSlotView pair.second = after.pairSlotView pair.second

/-- A write outside a queued pair's endpoint leaves the endpoint's physical
generation and logical agent identity unchanged. -/
theorem PhysicalState.pairSlotView_writeSlot_away
    {state : PhysicalState} {index other : Nat}
    {replacement : PhysicalSlot} (h : index ≠ other) :
    (state.writeSlot index replacement).pairSlotView other =
      state.pairSlotView other := by
  unfold PhysicalState.pairSlotView
  rw [PhysicalState.writeSlot_frame h]

/-- Fresh allocation cannot change an existing queued slot's identity. -/
theorem PhysicalState.pairSlotView_appendSlot_existing
    {state : PhysicalState} {fresh : PhysicalSlot} {index : Nat}
    (h : index < state.slots.size) :
    (state.appendSlot fresh).pairSlotView index =
      state.pairSlotView index := by
  unfold PhysicalState.pairSlotView
  rw [PhysicalState.appendSlot_frame h]

/-- Disjoint physical writes preserve both endpoints of one queued pair. -/
theorem PhysicalState.samePairSlots_writeSlot_away
    {state : PhysicalState} {index : Nat} {replacement : PhysicalSlot}
    {pair : PhysicalPair} (first : index ≠ pair.first)
    (second : index ≠ pair.second) :
    state.SamePairSlots (state.writeSlot index replacement) pair :=
  ⟨(PhysicalState.pairSlotView_writeSlot_away first).symm,
   (PhysicalState.pairSlotView_writeSlot_away second).symm⟩

/-- Any in-bounds old pair survives a fresh slot append with both slot
generation and logical identity unchanged. -/
theorem PhysicalState.samePairSlots_appendSlot
    {state : PhysicalState} {fresh : PhysicalSlot} {pair : PhysicalPair}
    (first : pair.first < state.slots.size)
    (second : pair.second < state.slots.size) :
    state.SamePairSlots (state.appendSlot fresh) pair :=
  ⟨(PhysicalState.pairSlotView_appendSlot_existing first).symm,
   (PhysicalState.pairSlotView_appendSlot_existing second).symm⟩

/-- The generation-checked pair decoder reads exactly `pairSlotView`, not
ports, queue markers, free-list bookkeeping or output targets. -/
private theorem PhysicalState.logicalPair_pairSlotView
    (state : PhysicalState) (pair : PhysicalPair) :
    state.logicalPair pair = do
      let (firstGeneration, firstAgent) ← state.pairSlotView pair.first
      let (secondGeneration, secondAgent) ← state.pairSlotView pair.second
      if firstGeneration != pair.firstGeneration ||
          secondGeneration != pair.secondGeneration then none else do
        let left ← firstAgent
        let right ← secondAgent
        some (left, right) := by
  cases hf : state.slots[pair.first]? with
  | none => simp [PhysicalState.logicalPair, PhysicalState.pairSlotView, hf]
  | some first =>
    cases hs : state.slots[pair.second]? with
    | none => simp [PhysicalState.logicalPair, PhysicalState.pairSlotView, hf, hs]
    | some second =>
      cases ha : first.agent with
      | none =>
        simp [PhysicalState.logicalPair, PhysicalState.pairSlotView, hf, hs, ha]
      | some left =>
        cases hb : second.agent with
        | none =>
          simp [PhysicalState.logicalPair, PhysicalState.pairSlotView, hf, hs, ha, hb]
        | some right =>
          simp [PhysicalState.logicalPair, PhysicalState.pairSlotView, hf, hs, ha, hb]

/-- Field-level equality of a queued pair's two physical slot identities
preserves its decoded logical endpoints, including its stale/valid status. -/
theorem PhysicalState.logicalPair_of_same_pair_slots
    {before after : PhysicalState} {pair : PhysicalPair}
    (h : before.SamePairSlots after pair) :
    before.logicalPair pair = after.logicalPair pair := by
  simp only [PhysicalState.logicalPair_pairSlotView, h.1, h.2]

/-- Each Rust canonical pair has two orientations in the inets worklist;
unchanged slot tags preserve both, including for an unrelated old pair. -/
theorem PhysicalState.pairOrientations_of_same_pair_slots
    {before after : PhysicalState} {pair : PhysicalPair}
    (h : before.SamePairSlots after pair) :
    before.pairOrientations pair = after.pairOrientations pair := by
  simp [PhysicalState.pairOrientations,
    PhysicalState.logicalPair_of_same_pair_slots h]

/-- A successful plan execution leaves every existing physical queue entry
in place. If its two physical slot identities remain stable, their projected
two orientations stay ahead of the newly published orientations, even when
the pending queue was already nonempty. The tag premise is field-level and
does not assume equality of either complete represented net. -/
theorem PhysicalState.executeCheckedImage_project_work_split
    {state next : PhysicalState} {bytes : List UInt8}
    {redex : XRedex NetSym} {plan : BytePlan}
    (hdecode : BytePlan.decode bytes = some plan)
    (h : state.executeCheckedImage bytes redex = some next)
    (hpending : ∀ pair ∈ state.work, state.SamePairSlots next pair) :
    next.project.work = state.project.work ++
      (plan.publications.map fun (first, firstGeneration, second, secondGeneration) =>
        { first, firstGeneration := UInt64.ofNat firstGeneration,
          second, secondGeneration := UInt64.ofNat secondGeneration : PhysicalPair }).flatMap
        next.pairOrientations := by
  have hpreserve (pairs : List PhysicalPair)
      (hids : ∀ pair ∈ pairs, state.SamePairSlots next pair) :
      pairs.flatMap next.pairOrientations =
        pairs.flatMap state.pairOrientations := by
    induction pairs with
    | nil => rfl
    | cons pair rest ih =>
      have hrest : ∀ other ∈ rest, state.SamePairSlots next other := by
        intro other ho
        exact hids other (List.mem_cons_of_mem _ ho)
      simp only [List.flatMap_cons]
      rw [← PhysicalState.pairOrientations_of_same_pair_slots
        (hids pair (by simp)), ih hrest]
  change next.work.flatMap next.pairOrientations =
    state.work.flatMap state.pairOrientations ++
      (plan.publications.map fun (first, firstGeneration, second, secondGeneration) =>
        { first, firstGeneration := UInt64.ofNat firstGeneration,
          second, secondGeneration := UInt64.ofNat secondGeneration : PhysicalPair }).flatMap
        next.pairOrientations
  rw [PhysicalState.executeCheckedImage_work_append hdecode h, List.flatMap_append,
    hpreserve state.work hpending]

/-- Pending pairs retain canonical slot order even if their generations have
gone stale. The ghost free-slot inventory exactly classifies retired slots. -/
def PhysicalState.WellFormed (state : PhysicalState) : Prop :=
  (∀ pair ∈ state.work, pair.first < pair.second) ∧
  (∀ index, index ∈ state.freeSlots ↔
    ∃ slot, state.slots[index]? = some slot ∧ slot.agent = none)

/-- The journal is the concrete, saved physical view seen by an observer until
commit; intermediate writes to `current` are not admitted as committed nets. -/
structure PhysicalTransaction where
  current : PhysicalState
  journal : Option PhysicalState

def PhysicalTransaction.visible (transaction : PhysicalTransaction) : PhysicalState :=
  transaction.journal.getD transaction.current

def PhysicalTransaction.Related (transaction : PhysicalTransaction)
    (net : HNet NetSym) : Prop :=
  transaction.visible.WellFormed ∧ transaction.visible.project = net

/-- Begin an in-flight transaction only after all three plan reservations fit.
A refused reservation leaves no transaction or modified arena to publish. -/
def PhysicalState.reserve (state : PhysicalState) (plan : BytePlan)
    (arenaAvailable scratchAvailable worklistAvailable : Nat) :
    Option PhysicalTransaction :=
  if plan.arenaReservation ≤ arenaAvailable &&
      plan.scratchReservation ≤ scratchAvailable &&
      plan.worklistReservation ≤ worklistAvailable then
    some { current := state, journal := some state }
  else none

/-- A successful reservation returns the exact original arena and its saved
snapshot; no slot, port or queued pair is changed before the first microstep. -/
theorem PhysicalState.reserve_snapshot {state : PhysicalState}
    {plan : BytePlan} {arena scratch queue : Nat} {transaction : PhysicalTransaction}
    (h : state.reserve plan arena scratch queue = some transaction) :
    transaction.current = state ∧ transaction.journal = some state := by
  unfold PhysicalState.reserve at h
  split at h
  · cases Option.some.inj h
    exact ⟨rfl, rfl⟩
  · simp at h

/-- All three resource obligations are discharged before the first physical
write is permitted; an unchecked plan cannot acquire an in-flight journal. -/
theorem PhysicalState.reserve_within_limits {state : PhysicalState}
    {plan : BytePlan} {arena scratch queue : Nat} {transaction : PhysicalTransaction}
    (h : state.reserve plan arena scratch queue = some transaction) :
    plan.arenaReservation ≤ arena ∧
      plan.scratchReservation ≤ scratch ∧
      plan.worklistReservation ≤ queue := by
  unfold PhysicalState.reserve at h
  split at h
  · rename_i hlimits
    simp at hlimits
    rcases hlimits with ⟨⟨ha, hs⟩, hq⟩
    exact ⟨ha, hs, hq⟩
  · simp at h

/-- One internal microstep can change physical storage only after a journal is
present; the observer continues to see the saved committed state. -/
def PhysicalTransaction.stage (transaction : PhysicalTransaction)
    (change : PhysicalState → PhysicalState) : Option PhysicalTransaction :=
  transaction.journal.map fun saved =>
    { current := change transaction.current, journal := some saved }

def PhysicalTransaction.rollback (transaction : PhysicalTransaction) : PhysicalTransaction :=
  { current := transaction.visible, journal := none }

def PhysicalTransaction.commit (transaction : PhysicalTransaction) : PhysicalTransaction :=
  { current := transaction.current, journal := none }

/-- A staged microstep preserves the complete represented net and slot/queue
validity of the saved state; the physical writes remain unobservable. -/
theorem PhysicalTransaction.stage_stutters {transaction next : PhysicalTransaction}
    {net : HNet NetSym} {change : PhysicalState → PhysicalState}
    (related : transaction.Related net)
    (step : transaction.stage change = some next) :
    next.Related net := by
  cases hj : transaction.journal with
  | none => simp [PhysicalTransaction.stage, hj] at step
  | some saved =>
    simp [PhysicalTransaction.stage, hj] at step
    cases step
    simpa [PhysicalTransaction.Related, PhysicalTransaction.visible, hj] using related

/-- A rollback restores the identical physical snapshot, not only the same
abstract observation, after any sequence of in-flight writes. -/
theorem PhysicalTransaction.rollback_restores {transaction : PhysicalTransaction}
    {net : HNet NetSym} (related : transaction.Related net) :
    (transaction.rollback).Related net ∧
      (transaction.rollback).current = transaction.visible := by
  constructor
  · simpa [PhysicalTransaction.rollback, PhysicalTransaction.Related,
      PhysicalTransaction.visible] using related
  · rfl

/-- The physical-to-abstract commit bridge exposes its missing premises rather
than claiming the Rust byte executor is proved: the decoded physical image
must project to the selected rule's observable net and its Rust-ordered queue
must contain the abstract pending-pair multiset. The two worklists are not
generally equal as ordered lists. -/
theorem PhysicalTransaction.commit_refines_if_projected {transaction : PhysicalTransaction}
    {H : HNet NetSym} (r : XRedex NetSym)
    (hRule : NetI64.xRule r.2.2.1 r.2.2.2.1 = some r.2.2.2.2)
    (rest : List (Id × Id))
    (hm : H.toNet.Matching NetI64.arity)
    (hredex : Redex NetI64.system H.toNet
      r.1 r.2.1 r.2.2.1 r.2.2.2.1 r.2.2.2.2.toRhs)
    (himage : transaction.current.project.toNet =
      (Plan.commit H (Plan.compile H r hRule) rest).toNet)
    (hqueue : transaction.current.project.work.Perm
      (Plan.commit H (Plan.compile H r hRule) rest).work) :
    CStep NetI64.system H.toNet (transaction.commit).visible.project.toNet ∧
      (transaction.commit).visible.project.work.Perm
        (Plan.commit H (Plan.compile H r hRule) rest).work := by
  constructor
  · simpa [PhysicalTransaction.commit, PhysicalTransaction.visible, himage] using
      (Plan.commit_refines r hRule rest hm hredex)
  · simpa [PhysicalTransaction.commit, PhysicalTransaction.visible] using hqueue

end NobleFoundation.NetI64Plans
