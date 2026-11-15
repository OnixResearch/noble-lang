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

private def readMany {T : Type} (count : Nat)
    (reader : List UInt8 → Option (T × List UInt8)) :
    List UInt8 → Option (List T × List UInt8)
  | bytes => match count with
    | 0 => some ([], bytes)
    | count + 1 => do
        let (item, rest) ← reader bytes
        let (items, tail) ← readMany count reader rest
        some (item :: items, tail)

private def readCounted {T : Type} (limit : Nat)
    (reader : List UInt8 → Option (T × List UInt8))
    (bytes : List UInt8) : Option (List T × List UInt8) := do
  let (count, rest) ← readByte bytes
  if count.toNat ≤ limit then readMany count.toNat reader rest else none

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

private def readBoundary (bytes : List UInt8) :
    Option (((Nat × Nat) × (Nat × Nat × Nat)) × List UInt8) := do
  let (slot, rest) ← readWord 4 bytes
  let (port, rest) ← readByte rest
  let (endpoint, rest) ← readEndpoint rest
  some (((slot, port.toNat), endpoint), rest)

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
    let ports := ports.map fun value =>
      if value.toNat == 255 then none else some value.toNat
    some ((fresh == 1, slot, symbol, donor, ports), rest)

private def readWrite (bytes : List UInt8) :
    Option ((Nat × Nat × Nat) × List UInt8) := do
  let (result, rest) ← readByte bytes
  let (port, rest) ← readByte rest
  let (boundary, rest) ← readByte rest
  some ((result.toNat, port.toNat, boundary.toNat), rest)

private def readPair (bytes : List UInt8) :
    Option ((Nat × Nat × Nat × Nat) × List UInt8) := do
  let (first, rest) ← readWord 4 bytes
  let (firstGeneration, rest) ← readWord 8 rest
  let (second, rest) ← readWord 4 rest
  let (secondGeneration, rest) ← readWord 8 rest
  some ((first, firstGeneration, second, secondGeneration), rest)

private def readHeader (bytes : List UInt8) : Option (List UInt8) :=
  if bytes.take 5 == ([78, 80, 76, 78, 1] : List UInt8) &&
      bytes.length ≥ 5 then some (bytes.drop 5) else none

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

/-- One erasure image with physical slots zero and one, both at generation zero.
The same 51-byte layout is used by the Rust writer for the no-result erasure
rule; its Rust identity remains a separate, finite differential obligation. -/
private def eraPlanBytes : List UInt8 :=
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

/-- A queued physical pair records both generations; stale entries are not
silently the same pair after either slot is reused. -/
structure PhysicalPair where
  first : Nat
  firstGeneration : UInt64
  second : Nat
  secondGeneration : UInt64

/-- Concrete single-engine storage, including empty-slot inventory and ordered
boundary and worklist; the abstract net has no counterpart of the free slots. -/
structure PhysicalState where
  slots : Array PhysicalSlot
  freeSlots : List Nat
  outputs : List (Nat × Nat)
  work : List PhysicalPair

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

/-- Decode physical port references into logical inets wires. -/
def PhysicalState.wires (state : PhysicalState) : Std.HashMap Port Port :=
  state.slots.toList.foldl (fun wires slot =>
    match slot.agent with
    | none => wires
    | some (logical, _) => state.portWires logical 0 slot.ports wires) {}

def PhysicalState.logicalPair (state : PhysicalState)
    (pair : PhysicalPair) : Option (Id × Id) := do
  let first ← state.slots[pair.first]?
  let second ← state.slots[pair.second]?
  if first.generation != pair.firstGeneration ||
      second.generation != pair.secondGeneration then none else do
    let (left, _) ← first.agent
    let (right, _) ← second.agent
    some (left, right)

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
    free := state.outputs.filterMap state.logicalPort
    work := state.work.filterMap state.logicalPair }

/-- Ordered external boundary ports are unchanged by a physical slot write
whose index is absent from the boundary, not merely equal as an unordered set. -/
theorem PhysicalState.project_boundary_frame {state : PhysicalState}
    {index : Nat} {replacement : PhysicalSlot}
    (outside : ∀ port ∈ state.outputs, index ≠ port.1) :
    (state.writeSlot index replacement).project.free = state.project.free := by
  have preserve : ∀ ports : List (Nat × Nat),
      (∀ port ∈ ports, index ≠ port.1) →
      ports.filterMap (state.writeSlot index replacement).logicalPort =
        ports.filterMap state.logicalPort := by
    intro ports
    induction ports with
    | nil => intro _; rfl
    | cons port rest ih =>
      intro hports
      have hrest := ih (by
        intro entry he
        exact hports entry (List.mem_cons_of_mem _ he))
      simp only [List.filterMap_cons]
      rw [PhysicalState.logicalPort_writeSlot_away (hports port (by simp))]
      rw [hrest]
  exact preserve state.outputs outside

/-- The ordered active-pair worklist is unchanged by a write away from both
slots named by every queued pair. -/
theorem PhysicalState.project_work_frame {state : PhysicalState}
    {index : Nat} {replacement : PhysicalSlot}
    (outside : ∀ pair ∈ state.work,
      index ≠ pair.first ∧ index ≠ pair.second) :
    (state.writeSlot index replacement).project.work = state.project.work := by
  have preserve : ∀ pairs : List PhysicalPair,
      (∀ pair ∈ pairs, index ≠ pair.first ∧ index ≠ pair.second) →
      pairs.filterMap (state.writeSlot index replacement).logicalPair =
        pairs.filterMap state.logicalPair := by
    intro pairs
    induction pairs with
    | nil => intro _; rfl
    | cons pair rest ih =>
      intro hpairs
      have hpair := hpairs pair (by simp)
      have hrest := ih (by
        intro entry he
        exact hpairs entry (List.mem_cons_of_mem _ he))
      simp only [List.filterMap_cons]
      rw [PhysicalState.logicalPair_writeSlot_away hpair.1 hpair.2, hrest]
  exact preserve state.work outside

/-- A committed physical state must have no stale queued pair or incorrectly
classified free slot. These are obligations on the representation relation. -/
def PhysicalState.WellFormed (state : PhysicalState) : Prop :=
  (∀ pair ∈ state.work, (state.logicalPair pair).isSome) ∧
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

/-- The physical-to-abstract commit bridge exposes its missing premise rather
than claiming the Rust byte executor is proved: the decoded physical image
must be shown to project to the independently selected rule-plan commit. -/
theorem PhysicalTransaction.commit_refines_if_projected {transaction : PhysicalTransaction}
    {H : HNet NetSym} (r : XRedex NetSym)
    (hRule : NetI64.xRule r.2.2.1 r.2.2.2.1 = some r.2.2.2.2)
    (rest : List (Id × Id))
    (hm : H.toNet.Matching NetI64.arity)
    (hredex : Redex NetI64.system H.toNet
      r.1 r.2.1 r.2.2.1 r.2.2.2.1 r.2.2.2.2.toRhs)
    (himage : transaction.current.project =
      Plan.commit H (Plan.compile H r hRule) rest) :
    CStep NetI64.system H.toNet (transaction.commit).visible.project.toNet := by
  simpa [PhysicalTransaction.commit, PhysicalTransaction.visible, himage] using
    (Plan.commit_refines r hRule rest hm hredex)

end NobleFoundation.NetI64Plans
