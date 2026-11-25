import NobleContracts.Rules
import NobleContracts.WasmLinearMemory

/-!
# Bounded Wasm Small-Step Semantics and MC1 `Exec` Simulation (`r[WREF-STEP-01]`)

First mechanization unit (extended with concrete `[0, 8192)` linear-memory helper
refinement) for `.cairn/changes/bounded-wasm-correspondence` Phase 2
(Owner Decision 56, 2026-11-23).

## Scope and independence

This module defines an **independent, Mathlib-free small-step operational semantics**
for the structured WebAssembly instruction subset reached by opt-off, non-calculator
linear-memory emission of a checked pure I64 arithmetic quotation (`I64.const`,
wrapping `+`, `-`, `*`):

- **Wasm core instructions** (`Instr`): `i32.const`, `i64.const`, `local.get`,
  `local.set`, `i64.add`, `i64.sub`, `i64.mul`, `call`, and `return` over explicit
  per-frame locals (`Frame.locals`), Wasm operand stacks (`Frame.vstack`), and a
  call-frame stack (`Config.frames`).
- **Reached arithmetic helpers** (`runtimeEnv`): exact structured Wasm bodies of
  `$push_i64` (`crates/noble-wasm/runtime/stack.wat:26`) and `$op_add`, `$op_sub`,
  `$op_mul` (`crates/noble-wasm/runtime/operations.wat:14-25`), including right-then-left
  popping into locals `$right` and `$left`, followed by `i64.{add,sub,mul} $left $right`.
- **Concrete `[0, 8192)` linear-memory helper refinement**: `NobleContracts.WasmLinearMemory`
  and Section 13 discharge `LinearMemorySlotPremise` by proving that byte-level
  `loadI32`/`loadI64`/`storeI32`/`storeI64` over `[0, 4096)` (operand slots) and
  `[4096, 8192)` (continuation slots) refine `storePushTag`, `storePopKind`,
  `storeEnqueue`, `$dispatch` dequeue, and `storeRuntimeTick` with region
  non-interference, realizability, and end-to-end lifting (`linearRunLowered_sound_exec`).
- **Continuation dispatch** (`DispatchStep`, `DispatchRun`): exact opt-off entry
  chaining matching `crates/noble-wasm/src/source/emit/functions.rs` (`calculator = false`)
  and the `$dispatch` loop in `crates/noble-wasm/runtime/stack.wat:78-88`.
- **Three-way outcome separation**: normal completion (`FailureStatus.ok`), profile
  quota exhaustion (`FailureStatus.quota phase reason` for operand, continuation, and
  step limits), and runtime faults (`FailureStatus.fault 4` for stack underflow,
  operand tag mismatch, or invalid dispatch entry).

The operational semantics is defined **without reference to** `NobleContracts.Exec`
or `NobleContracts.Step`, and is then related to `NobleContracts.Exec` by soundness,
completeness, frame/tail-preservation, quota/fault-separation, non-vacuity, and
rejection-sensitivity theorems.

## Remaining excluded boundaries (`r[WREF-CHAIN-01]`)

- **Excluded emitter/encoding/loading edges:** Extracted Rust emitter refinement
  (`r[WREF-EMIT-01]`), WAT/binary encoding, `table.init` / module loading, opt-on
  emission, calculator `$inline_step` fusion, WasmGC, named calls, nested `Program`
  quotations, branching, heap allocation, resources, live slots, and host effects are
  excluded from this theorem and remain open (`PO-17`, `PO-18`, `O1`, `O3`, `O7`).
-/

namespace NobleContracts.WasmStep

open NobleContracts
open NobleContracts.WasmLinearMemory

/-! ## 1. Runtime Tags, Quotas, and Failure Classification -/

/-- Runtime value tags matching `stack.wat`, `operations.wat`, and `data.wat`. -/
def tagI64 : BitVec 32 := 1
def tagBool : BitVec 32 := 2
def tagUnit : BitVec 32 := 3
def tagProgram : BitVec 32 := 4
def tagPair : BitVec 32 := 5
def tagCons : BitVec 32 := 6
def tagNil : BitVec 32 := 7
def tagSyntax : BitVec 32 := 10
def tagText : BitVec 32 := 11
def tagInl : BitVec 32 := 12
def tagInr : BitVec 32 := 13

/-- One 16-byte tagged operand slot (`tag : i32`, padding, `payload : i64`)
    abstracting `stack.wat:1-45`. -/
structure TaggedSlot where
  tag : BitVec 32
  payload : BitVec 64
deriving DecidableEq, Repr

/-- One 8-byte continuation entry (`fnIdx : i32`, `env : i32`)
    abstracting `stack.wat:61-88`. -/
structure ContEntry where
  fnIdx : BitVec 32
  env : BitVec 32
deriving DecidableEq, Repr

/-- Profile quota limits configured by `begin` (`common.wat:51-63`, `source.wat:11-37`). -/
structure ProfileLimits where
  operandLimit : Nat
  continuationLimit : Nat
  stepLimit : Nat
deriving DecidableEq, Repr

/-- Profile bounds enforced by `begin`, guaranteeing that slot/continuation byte sizes
    and step counts never wrap 32-bit unsigned arithmetic in `stack.wat`/`accounting.wat`. -/
def ProfileLimitsValid (l : ProfileLimits) : Prop :=
  l.operandLimit ≤ 2048 ∧ l.continuationLimit ≤ 4096 ∧ l.stepLimit ≤ 100000

/-- Quota exhaustion reasons matching `accounting.wat` and `stack.wat`. -/
inductive QuotaReason where
  | none
  | allocation
  | recipe
  | depth
  | operand
  | continuation
  | steps
deriving DecidableEq, Repr

def QuotaReason.code : QuotaReason → BitVec 32
  | .none => 0
  | .allocation => 1
  | .recipe => 2
  | .depth => 3
  | .operand => 4
  | .continuation => 5
  | .steps => 6

/-- Runtime failure state matching `accounting.wat:2-7`.
    `$quota` writes `$failure := 1 + $phase` and `$quota_reason := reason`;
    `$fail code` writes `$failure := code` and leaves `$quota_reason := 0`. -/
inductive FailureStatus where
  | ok
  | quota (phase : BitVec 32) (reason : QuotaReason)
  | fault (code : BitVec 32)
deriving DecidableEq, Repr

/-- Observable `$failure` global (`metric 0`). -/
def FailureStatus.failureGlobal : FailureStatus → BitVec 32
  | .ok => 0
  | .quota phase _ => 1 + phase
  | .fault code => code

/-- Observable `$quota_reason` global (`metric 1`). -/
def FailureStatus.quotaGlobal : FailureStatus → BitVec 32
  | .ok => 0
  | .quota _ reason => reason.code
  | .fault _ => 0

/-- Execution phase constant (`$phase = 1` during dispatch, `emit.rs:480`). -/
def execPhase : BitVec 32 := 1

/-- Runtime fault code used by `$need`, `$slot_kind`, and `$pop_kind` (`stack.wat:5,9,44`). -/
def faultCode : BitVec 32 := 4

/-- Normal, execution-phase quota exhaustion, and runtime fault (`code = 4`) have
    pairwise disjoint observable `$failure` globals. -/
theorem status_disjoint_exec_phase (r : QuotaReason) :
    FailureStatus.ok.failureGlobal ≠ (FailureStatus.quota execPhase r).failureGlobal ∧
    FailureStatus.ok.failureGlobal ≠ (FailureStatus.fault faultCode).failureGlobal ∧
    (FailureStatus.quota execPhase r).failureGlobal ≠
      (FailureStatus.fault faultCode).failureGlobal := by
  simp only [FailureStatus.failureGlobal, execPhase, faultCode]
  exact ⟨by decide, by decide, by decide⟩

/-- Every nonzero quota reason has a nonzero `$quota_reason` global, whereas faults
    leave `$quota_reason = 0`. -/
theorem quota_reason_nonzero_vs_fault (r : QuotaReason) (hr : r ≠ .none) :
    (FailureStatus.quota execPhase r).quotaGlobal ≠
      (FailureStatus.fault faultCode).quotaGlobal := by
  cases r with
  | none => exact absurd rfl hr
  | allocation => decide
  | recipe => decide
  | depth => decide
  | operand => decide
  | continuation => decide
  | steps => decide

/-! ## 2. Abstract Runtime Store (Scoped Stack-Access Contract) -/

/-- Wasm runtime store for the bounded arithmetic/dispatch slice.
    `slots` and `conts` are stored top-first so the top slot is at the head;
    bottom-first linear-memory order is `slots.reverse` with `sp = slots.length`. -/
structure RuntimeStore where
  slots : List TaggedSlot
  conts : List ContEntry
  limits : ProfileLimits
  phase : BitVec 32
  status : FailureStatus
  steps : Nat
  operandPeak : Nat
  continuationPeak : Nat
deriving DecidableEq, Repr

/-- Sticky `$fail`: records the first failure and leaves later errors ignored
    (`accounting.wat:2-3`). -/
def RuntimeStore.setFault (st : RuntimeStore) (code : BitVec 32) : RuntimeStore :=
  match st.status with
  | .ok => { st with status := .fault code }
  | _ => st

/-- Sticky `$quota`: records the first quota exhaustion with `$phase`
    (`accounting.wat:4-7`). -/
def RuntimeStore.setQuota (st : RuntimeStore) (reason : QuotaReason) : RuntimeStore :=
  match st.status with
  | .ok => { st with status := .quota st.phase reason }
  | _ => st

/-- Abstract `$push_tag (kind : i32) (value : i64)` (`stack.wat:15-25`). -/
def storePushTag (st : RuntimeStore) (kind : BitVec 32) (value : BitVec 64) : RuntimeStore :=
  match st.status with
  | .ok =>
      let bytes := (st.slots.length + 1) * 16
      if bytes > st.limits.operandLimit then
        st.setQuota .operand
      else
        { st with
          slots := ⟨kind, value⟩ :: st.slots
          operandPeak := Nat.max st.operandPeak bytes }
  | _ => st

/-- Abstract `$pop_kind (expected : i32) -> i64` (`stack.wat:41-45`). -/
def storePopKind (st : RuntimeStore) (expected : BitVec 32) : BitVec 64 × RuntimeStore :=
  match st.status with
  | .ok =>
      match st.slots with
      | [] => (0, st.setFault faultCode)
      | top :: rest =>
          if top.tag != expected then
            (0, st.setFault faultCode)
          else
            (top.payload, { st with slots := rest })
  | _ => (0, st)

/-- Abstract `$enqueue (fnIdx : i32) (env : i32)` (`stack.wat:62-72`). -/
def storeEnqueue (st : RuntimeStore) (fnIdx env : BitVec 32) : RuntimeStore :=
  match st.status with
  | .ok =>
      let bytes := (st.conts.length + 1) * 8
      if bytes > st.limits.continuationLimit then
        st.setQuota .continuation
      else
        { st with
          conts := ⟨fnIdx, env⟩ :: st.conts
          continuationPeak := Nat.max st.continuationPeak bytes }
  | _ => st

/-- Abstract `$runtime_tick () -> i32` (`accounting.wat:10-15`). -/
def storeRuntimeTick (st : RuntimeStore) : BitVec 32 × RuntimeStore :=
  match st.status with
  | .ok =>
      if st.steps ≥ st.limits.stepLimit then
        (0, st.setQuota .steps)
      else
        (1, { st with steps := st.steps + 1 })
  | _ => (0, st)

/-- Read one 16-byte tagged operand slot from `[0, 4096)` linear memory.
    Constrains ONLY bytes `+0..+4` (`tag`) and `+8..+16` (`payload`), leaving
    bytes `+4..+8` (padding, untouched by `$push_tag`) unconstrained. -/
def readSlot (mem : LinearMem) (idx : Nat) : TaggedSlot :=
  ⟨loadI32 mem (idx * 16), loadI64 mem (idx * 16 + 8)⟩

/-- Read one 8-byte continuation entry from `[4096, 8192)` linear memory. -/
def readCont (mem : LinearMem) (idx : Nat) : ContEntry :=
  ⟨loadI32 mem (4096 + idx * 8), loadI32 mem (4096 + idx * 8 + 4)⟩

/-- Top-first list of the `n` live operand slots stored in `[0, n * 16)`. -/
def readSlotsTopFirst (mem : LinearMem) : Nat → List TaggedSlot
  | 0 => []
  | n + 1 => readSlot mem n :: readSlotsTopFirst mem n

/-- Top-first list of the `n` live continuation entries stored in `[4096, 4096 + n * 8)`. -/
def readContsTopFirst (mem : LinearMem) : Nat → List ContEntry
  | 0 => []
  | n + 1 => readCont mem n :: readContsTopFirst mem n

/-- Exact status refinement relating concrete `($failure, $quota_reason)` globals
    to `FailureStatus`, enforcing non-zero failure codes and `$failure = 0 → $quota_reason = 0`. -/
def StatusRefines (failure quotaReason : BitVec 32) : FailureStatus → Prop
  | .ok => failure = 0 ∧ quotaReason = 0
  | .quota qPhase reason =>
      failure = 1 + qPhase ∧ 1 + qPhase ≠ 0 ∧
      quotaReason = reason.code ∧ reason ≠ .none
  | .fault code => failure = code ∧ code ≠ 0 ∧ quotaReason = 0

/-- Validity of `FailureStatus` globals ensuring failed states never collide with `failure = 0`. -/
def StatusGlobalValid : FailureStatus → Prop
  | .ok => True
  | .quota qPhase reason => 1 + qPhase ≠ 0 ∧ reason ≠ .none
  | .fault code => code ≠ 0

/-- Full refinement relation between concrete `[0, 8192)` `LinearRuntimeState`
    and abstract `RuntimeStore`. Equates `cst.operandPeak.toNat = st.operandPeak`
    directly without assuming `st.operandPeak ≥ st.slots.length * 16`. -/
structure StoreRefines (cst : LinearRuntimeState) (st : RuntimeStore) : Prop where
  limitsValid : ProfileLimitsValid st.limits
  limitsEq :
    cst.operandLimit.toNat = st.limits.operandLimit ∧
    cst.continuationLimit.toNat = st.limits.continuationLimit ∧
    cst.stepLimit.toNat = st.limits.stepLimit
  spBound : cst.sp.toNat ≤ 256
  cpBound : cst.cp.toNat ≤ 512
  stepsBound : cst.steps.toNat ≤ 100000
  phaseValid : 1 + cst.phase ≠ 0
  failureZeroQuotaZero : cst.failure = 0 → cst.quotaReason = 0
  slotsEq : readSlotsTopFirst cst.mem cst.sp.toNat = st.slots
  contsEq : readContsTopFirst cst.mem cst.cp.toNat = st.conts
  phaseEq : cst.phase = st.phase
  statusRef : StatusRefines cst.failure cst.quotaReason st.status
  stepsEq : cst.steps.toNat = st.steps
  operandPeakEq : cst.operandPeak.toNat = st.operandPeak
  continuationPeakEq : cst.continuationPeak.toNat = st.continuationPeak

/-- Computable decoder from `$quota_reason` global to `QuotaReason`. -/
def QuotaReason.ofCodeD (c : BitVec 32) : QuotaReason :=
  if c = 1 then .allocation
  else if c = 2 then .recipe
  else if c = 3 then .depth
  else if c = 4 then .operand
  else if c = 5 then .continuation
  else if c = 6 then .steps
  else .none

/-- Computable abstraction map from concrete `LinearRuntimeState` to `RuntimeStore`. -/
def absStore (cst : LinearRuntimeState) : RuntimeStore :=
  let status : FailureStatus :=
    if cst.failure = 0 then .ok
    else if cst.quotaReason ≠ 0 then .quota (cst.failure - 1) (QuotaReason.ofCodeD cst.quotaReason)
    else .fault cst.failure
  { slots := readSlotsTopFirst cst.mem cst.sp.toNat
    conts := readContsTopFirst cst.mem cst.cp.toNat
    limits :=
      { operandLimit := cst.operandLimit.toNat
        continuationLimit := cst.continuationLimit.toNat
        stepLimit := cst.stepLimit.toNat }
    phase := cst.phase
    status := status
    steps := cst.steps.toNat
    operandPeak := cst.operandPeak.toNat
    continuationPeak := cst.continuationPeak.toNat }

/-- Genuine concrete `[0, 8192)` linear-memory refinement, region non-interference,
    and realizability contract replacing the earlier abstract-slot assumption (`r[WREF-STEP-01]`). -/
structure LinearMemorySlotPremise : Prop where
  pushTagRefines :
    ∀ cst st kind value, StoreRefines cst st →
      StoreRefines (concretePushTag cst kind value) (storePushTag st kind value)
  popKindRefines :
    ∀ cst st expected, StoreRefines cst st →
      (concretePopKind cst expected).1 = (storePopKind st expected).1 ∧
      StoreRefines (concretePopKind cst expected).2 (storePopKind st expected).2
  enqueueRefines :
    ∀ cst st fnIdx env, StoreRefines cst st →
      StoreRefines (concreteEnqueue cst fnIdx env) (storeEnqueue st fnIdx env)
  dequeueRefines :
    ∀ cst st c rest, StoreRefines cst st → st.status = .ok → st.conts = c :: rest →
      (concreteDequeue cst).1 = (c.fnIdx, c.env) ∧
      StoreRefines (concreteDequeue cst).2 { st with conts := rest }
  runtimeTickRefines :
    ∀ cst st, StoreRefines cst st →
      (concreteRuntimeTick cst).1 = (storeRuntimeTick st).1 ∧
      StoreRefines (concreteRuntimeTick cst).2 (storeRuntimeTick st).2
  opWritesPreserveContRegion :
    ∀ cst, cst.sp.toNat ≤ 256 → cst.operandLimit.toNat ≤ 2048 →
      (∀ k v, MemEqOn 4096 8192 cst.mem (concretePushTag cst k v).mem ∧
              (concretePushTag cst k v).cp = cst.cp) ∧
      (∀ exp, MemEqOn 4096 8192 cst.mem (concretePopKind cst exp).2.mem ∧
              (concretePopKind cst exp).2.cp = cst.cp)
  contWritesPreserveOpRegion :
    ∀ cst, cst.cp.toNat ≤ 512 →
      (∀ fnIdx env, MemEqOn 0 4096 cst.mem (concreteEnqueue cst fnIdx env).mem ∧
                    (concreteEnqueue cst fnIdx env).sp = cst.sp) ∧
      (MemEqOn 0 4096 cst.mem (concreteDequeue cst).2.mem ∧
       (concreteDequeue cst).2.sp = cst.sp)
  tickPreservesBothRegions :
    ∀ cst, MemEqOn 0 8192 cst.mem (concreteRuntimeTick cst).2.mem ∧
           (concreteRuntimeTick cst).2.sp = cst.sp ∧
           (concreteRuntimeTick cst).2.cp = cst.cp
  realizable :
    ∀ st : RuntimeStore, ProfileLimitsValid st.limits →
      st.slots.length ≤ 256 → st.conts.length ≤ 512 → st.steps ≤ 100000 →
      1 + st.phase ≠ 0 → st.operandPeak < 2 ^ 32 → st.continuationPeak < 2 ^ 32 →
      StatusGlobalValid st.status → ∃ cst, StoreRefines cst st

theorem storePushTag_ok (st : RuntimeStore) (k : BitVec 32) (v : BitVec 64)
    (hok : st.status = .ok) (hlim : (st.slots.length + 1) * 16 ≤ st.limits.operandLimit) :
    storePushTag st k v =
      { st with
        slots := ⟨k, v⟩ :: st.slots
        operandPeak := Nat.max st.operandPeak ((st.slots.length + 1) * 16) } := by
  have hngt : ¬ ((st.slots.length + 1) * 16 > st.limits.operandLimit) := Nat.not_lt.mpr hlim
  simp [storePushTag, hok, hngt]

theorem storePushTag_quota (st : RuntimeStore) (k : BitVec 32) (v : BitVec 64)
    (hok : st.status = .ok) (hgt : (st.slots.length + 1) * 16 > st.limits.operandLimit) :
    storePushTag st k v = { st with status := .quota st.phase .operand } := by
  simp [storePushTag, RuntimeStore.setQuota, hok, hgt]

theorem storePushTag_failed (st : RuntimeStore) (k : BitVec 32) (v : BitVec 64)
    (hfail : st.status ≠ .ok) :
    storePushTag st k v = st := by
  cases h : st.status with
  | ok => exact absurd h hfail
  | quota _ _ => simp [storePushTag, h]
  | fault _ => simp [storePushTag, h]

theorem storePushTag_conts (st : RuntimeStore) (k : BitVec 32) (v : BitVec 64) :
    (storePushTag st k v).conts = st.conts := by
  cases h : st.status with
  | ok =>
      unfold storePushTag
      rw [h]
      dsimp only
      split
      · simp [RuntimeStore.setQuota, h]
      · rfl
  | quota _ _ => simp [storePushTag, h]
  | fault _ => simp [storePushTag, h]

theorem storePopKind_ok (st : RuntimeStore) (k : BitVec 32) (v : BitVec 64)
    (rest : List TaggedSlot) (hok : st.status = .ok) (hsl : st.slots = ⟨k, v⟩ :: rest) :
    storePopKind st k = (v, { st with slots := rest }) := by
  simp [storePopKind, hok, hsl]

theorem storePopKind_underflow (st : RuntimeStore) (k : BitVec 32)
    (hok : st.status = .ok) (hsl : st.slots = []) :
    storePopKind st k = (0, { st with status := .fault faultCode }) := by
  simp [storePopKind, RuntimeStore.setFault, hok, hsl]

theorem storePopKind_tag_mismatch (st : RuntimeStore) (k : BitVec 32)
    (top : TaggedSlot) (rest : List TaggedSlot)
    (hok : st.status = .ok) (hsl : st.slots = top :: rest) (hne : top.tag ≠ k) :
    storePopKind st k = (0, { st with status := .fault faultCode }) := by
  have hb : (top.tag != k) = true := bne_iff_ne.mpr hne
  simp [storePopKind, RuntimeStore.setFault, hok, hsl, hb]

theorem storePopKind_failed (st : RuntimeStore) (k : BitVec 32)
    (hfail : st.status ≠ .ok) :
    storePopKind st k = (0, st) := by
  cases h : st.status with
  | ok => exact absurd h hfail
  | quota _ _ => simp [storePopKind, h]
  | fault _ => simp [storePopKind, h]

theorem storePopKind_conts (st : RuntimeStore) (k : BitVec 32) :
    (storePopKind st k).2.conts = st.conts := by
  cases h : st.status with
  | ok =>
      cases hs : st.slots with
      | nil => simp [storePopKind, RuntimeStore.setFault, h, hs]
      | cons top rest =>
          unfold storePopKind
          rw [h, hs]
          dsimp only
          split
          · simp [RuntimeStore.setFault, h]
          · rfl
  | quota _ _ => simp [storePopKind, h]
  | fault _ => simp [storePopKind, h]

theorem storeEnqueue_ok (st : RuntimeStore) (fnIdx env : BitVec 32)
    (hok : st.status = .ok) (hlim : (st.conts.length + 1) * 8 ≤ st.limits.continuationLimit) :
    storeEnqueue st fnIdx env =
      { st with
        conts := ⟨fnIdx, env⟩ :: st.conts
        continuationPeak := Nat.max st.continuationPeak ((st.conts.length + 1) * 8) } := by
  have hngt : ¬ ((st.conts.length + 1) * 8 > st.limits.continuationLimit) := Nat.not_lt.mpr hlim
  simp [storeEnqueue, hok, hngt]

theorem storeEnqueue_quota (st : RuntimeStore) (fnIdx env : BitVec 32)
    (hok : st.status = .ok) (hgt : (st.conts.length + 1) * 8 > st.limits.continuationLimit) :
    storeEnqueue st fnIdx env = { st with status := .quota st.phase .continuation } := by
  simp [storeEnqueue, RuntimeStore.setQuota, hok, hgt]

theorem storeRuntimeTick_ok (st : RuntimeStore)
    (hok : st.status = .ok) (hlt : st.steps < st.limits.stepLimit) :
    storeRuntimeTick st = (1, { st with steps := st.steps + 1 }) := by
  have hnge : ¬ (st.steps ≥ st.limits.stepLimit) := Nat.not_le.mpr hlt
  simp [storeRuntimeTick, hok, hnge]

theorem storeRuntimeTick_quota (st : RuntimeStore)
    (hok : st.status = .ok) (hge : st.steps ≥ st.limits.stepLimit) :
    storeRuntimeTick st = (0, { st with status := .quota st.phase .steps }) := by
  simp [storeRuntimeTick, RuntimeStore.setQuota, hok, hge]

/-! ## 3. Independent Structured Wasm Small-Step Operational Semantics -/

/-- Wasm value stack entries for the reached subset (`i32`, `i64`). -/
inductive WVal where
  | i32 : BitVec 32 → WVal
  | i64 : BitVec 64 → WVal
deriving DecidableEq, Repr

/-- Function identifiers in the reached runtime helper closure. -/
inductive FuncId where
  | opAdd
  | opSub
  | opMul
  | pushI64
  | pushTag
  | popKind
  | enqueue
  | runtimeTick
deriving DecidableEq, Repr

/-- Structured Wasm instructions for the selected arithmetic/helper subset. -/
inductive Instr where
  | i32Const : BitVec 32 → Instr
  | i64Const : BitVec 64 → Instr
  | localGet : Nat → Instr
  | localSet : Nat → Instr
  | i64Add : Instr
  | i64Sub : Instr
  | i64Mul : Instr
  | call : FuncId → Instr
  | return_ : Instr
deriving DecidableEq, Repr

/-- Defined Wasm helper function signature and structured body. -/
structure FuncDef where
  paramCount : Nat
  localDefaults : List WVal
  body : List Instr
  retArity : Nat
deriving DecidableEq, Repr

/-- Function environment mapping defined Wasm helpers to their bodies;
    `none` delegates to the primitive runtime helper contract (`pushTag`, `popKind`,
    `enqueue`, `runtimeTick`). -/
abbrev FuncEnv := FuncId → Option FuncDef

/-- Exact structured Wasm body of `$push_i64` (`crates/noble-wasm/runtime/stack.wat:26`):
    `(func $push_i64 (param $value i64) (call $push_tag (i32.const 1) (local.get $value)))`. -/
def defPushI64 : FuncDef :=
  { paramCount := 1
    localDefaults := []
    body := [.i32Const tagI64, .localGet 0, .call .pushTag]
    retArity := 0 }

/-- Exact structured Wasm body of `$op_add` (`crates/noble-wasm/runtime/operations.wat:14-17`):
    pops `$right` (local 0), then `$left` (local 1), computes `i64.add $left $right`,
    and calls `$push_i64`. -/
def defOpAdd : FuncDef :=
  { paramCount := 0
    localDefaults := [.i64 0, .i64 0]
    body :=
      [ .i32Const tagI64, .call .popKind, .localSet 0
      , .i32Const tagI64, .call .popKind, .localSet 1
      , .localGet 1, .localGet 0, .i64Add, .call .pushI64 ]
    retArity := 0 }

/-- Exact structured Wasm body of `$op_sub` (`crates/noble-wasm/runtime/operations.wat:18-21`):
    pops `$right` (local 0), then `$left` (local 1), computes `i64.sub $left $right`,
    and calls `$push_i64`. -/
def defOpSub : FuncDef :=
  { paramCount := 0
    localDefaults := [.i64 0, .i64 0]
    body :=
      [ .i32Const tagI64, .call .popKind, .localSet 0
      , .i32Const tagI64, .call .popKind, .localSet 1
      , .localGet 1, .localGet 0, .i64Sub, .call .pushI64 ]
    retArity := 0 }

/-- Exact structured Wasm body of `$op_mul` (`crates/noble-wasm/runtime/operations.wat:22-25`):
    pops `$right` (local 0), then `$left` (local 1), computes `i64.mul $left $right`,
    and calls `$push_i64`. -/
def defOpMul : FuncDef :=
  { paramCount := 0
    localDefaults := [.i64 0, .i64 0]
    body :=
      [ .i32Const tagI64, .call .popKind, .localSet 0
      , .i32Const tagI64, .call .popKind, .localSet 1
      , .localGet 1, .localGet 0, .i64Mul, .call .pushI64 ]
    retArity := 0 }

/-- Production runtime helper environment for the bounded arithmetic closure. -/
def runtimeEnv : FuncEnv
  | .pushI64 => some defPushI64
  | .opAdd => some defOpAdd
  | .opSub => some defOpSub
  | .opMul => some defOpMul
  | .pushTag => none
  | .popKind => none
  | .enqueue => none
  | .runtimeTick => none

/-- Active or suspended Wasm activation frame. -/
structure Frame where
  locals : List WVal
  vstack : List WVal
  code : List Instr
  retArity : Nat
deriving DecidableEq, Repr

/-- Small-step Wasm machine configuration: call-frame stack + runtime store. -/
structure Config where
  frames : List Frame
  store : RuntimeStore
deriving DecidableEq, Repr

/-- Initial Wasm configuration running a top-level instruction sequence `code`
    with no locals and zero return arity. -/
def initConfig (code : List Instr) (st : RuntimeStore) : Config :=
  { frames := [⟨[], [], code, 0⟩], store := st }

/-- Terminal top-level configuration with zero return values. -/
def doneConfig (st : RuntimeStore) : Config :=
  { frames := [⟨[], [], [], 0⟩], store := st }

/-- Terminal top-level configuration returning one Wasm value `v`. -/
def doneValConfig (v : WVal) (st : RuntimeStore) : Config :=
  { frames := [⟨[], [v], [], 1⟩], store := st }

theorem doneConfig_inj {s1 s2 : RuntimeStore} (h : doneConfig s1 = doneConfig s2) : s1 = s2 := by
  cases h; rfl

theorem doneValConfig_inj {v1 v2 : WVal} {s1 s2 : RuntimeStore}
    (h : doneValConfig v1 s1 = doneValConfig v2 s2) : v1 = v2 ∧ s1 = s2 := by
  cases h; exact ⟨rfl, rfl⟩

/-- Independent small-step operational semantics for the structured Wasm subset.
    Defined purely in terms of Wasm values, locals, call frames, wrapping 64-bit
    arithmetic, and the abstract runtime store — never in terms of MC1 `Exec`. -/
inductive WasmStep (env : FuncEnv) : Config → Config → Prop where
  | i32Const {locals vstack rest r cs st n} :
      WasmStep env
        ⟨⟨locals, vstack, .i32Const n :: rest, r⟩ :: cs, st⟩
        ⟨⟨locals, .i32 n :: vstack, rest, r⟩ :: cs, st⟩
  | i64Const {locals vstack rest r cs st n} :
      WasmStep env
        ⟨⟨locals, vstack, .i64Const n :: rest, r⟩ :: cs, st⟩
        ⟨⟨locals, .i64 n :: vstack, rest, r⟩ :: cs, st⟩
  | localGet {locals vstack rest r cs st idx v} :
      locals[idx]? = some v →
      WasmStep env
        ⟨⟨locals, vstack, .localGet idx :: rest, r⟩ :: cs, st⟩
        ⟨⟨locals, v :: vstack, rest, r⟩ :: cs, st⟩
  | localSet {locals vstack rest r cs st idx v} :
      idx < locals.length →
      WasmStep env
        ⟨⟨locals, v :: vstack, .localSet idx :: rest, r⟩ :: cs, st⟩
        ⟨⟨locals.set idx v, vstack, rest, r⟩ :: cs, st⟩
  | i64Add {locals vstack rest r cs st lhs rhs} :
      WasmStep env
        ⟨⟨locals, .i64 rhs :: .i64 lhs :: vstack, .i64Add :: rest, r⟩ :: cs, st⟩
        ⟨⟨locals, .i64 (lhs + rhs) :: vstack, rest, r⟩ :: cs, st⟩
  | i64Sub {locals vstack rest r cs st lhs rhs} :
      WasmStep env
        ⟨⟨locals, .i64 rhs :: .i64 lhs :: vstack, .i64Sub :: rest, r⟩ :: cs, st⟩
        ⟨⟨locals, .i64 (lhs - rhs) :: vstack, rest, r⟩ :: cs, st⟩
  | i64Mul {locals vstack rest r cs st lhs rhs} :
      WasmStep env
        ⟨⟨locals, .i64 rhs :: .i64 lhs :: vstack, .i64Mul :: rest, r⟩ :: cs, st⟩
        ⟨⟨locals, .i64 (lhs * rhs) :: vstack, rest, r⟩ :: cs, st⟩
  | callDefined {locals vstack args vstack' rest r cs st fid fdef} :
      env fid = some fdef →
      vstack = args ++ vstack' →
      args.length = fdef.paramCount →
      WasmStep env
        ⟨⟨locals, vstack, .call fid :: rest, r⟩ :: cs, st⟩
        ⟨⟨args.reverse ++ fdef.localDefaults, [], fdef.body, fdef.retArity⟩ ::
          ⟨locals, vstack', rest, r⟩ :: cs, st⟩
  | returnEnd {locals vstack retVals vstackRem r cLocals cVstack cCode cR cs st} :
      vstack = retVals ++ vstackRem →
      retVals.length = r →
      WasmStep env
        ⟨⟨locals, vstack, [], r⟩ :: ⟨cLocals, cVstack, cCode, cR⟩ :: cs, st⟩
        ⟨⟨cLocals, retVals ++ cVstack, cCode, cR⟩ :: cs, st⟩
  | returnInstr {locals vstack retVals vstackRem rest r cLocals cVstack cCode cR cs st} :
      vstack = retVals ++ vstackRem →
      retVals.length = r →
      WasmStep env
        ⟨⟨locals, vstack, .return_ :: rest, r⟩ ::
          ⟨cLocals, cVstack, cCode, cR⟩ :: cs, st⟩
        ⟨⟨cLocals, retVals ++ cVstack, cCode, cR⟩ :: cs, st⟩
  | callPushTag {locals vstack rest r cs st k v} :
      env .pushTag = none →
      WasmStep env
        ⟨⟨locals, .i64 v :: .i32 k :: vstack, .call .pushTag :: rest, r⟩ :: cs, st⟩
        ⟨⟨locals, vstack, rest, r⟩ :: cs, storePushTag st k v⟩
  | callPopKind {locals vstack rest r cs st k} :
      env .popKind = none →
      WasmStep env
        ⟨⟨locals, .i32 k :: vstack, .call .popKind :: rest, r⟩ :: cs, st⟩
        ⟨⟨locals, .i64 (storePopKind st k).1 :: vstack, rest, r⟩ :: cs, (storePopKind st k).2⟩
  | callEnqueue {locals vstack rest r cs st fnIdx envId} :
      env .enqueue = none →
      WasmStep env
        ⟨⟨locals, .i32 envId :: .i32 fnIdx :: vstack, .call .enqueue :: rest, r⟩ :: cs, st⟩
        ⟨⟨locals, vstack, rest, r⟩ :: cs, storeEnqueue st fnIdx envId⟩
  | callRuntimeTick {locals vstack rest r cs st} :
      env .runtimeTick = none →
      WasmStep env
        ⟨⟨locals, vstack, .call .runtimeTick :: rest, r⟩ :: cs, st⟩
        ⟨⟨locals, .i32 (storeRuntimeTick st).1 :: vstack, rest, r⟩ :: cs,
          (storeRuntimeTick st).2⟩

/-- Reflexive-transitive closure of `WasmStep`. -/
inductive WasmSteps (env : FuncEnv) : Config → Config → Prop where
  | refl {c} : WasmSteps env c c
  | step {c1 c2 c3} : WasmStep env c1 c2 → WasmSteps env c2 c3 → WasmSteps env c1 c3

theorem WasmSteps.single {env c1 c2} (h : WasmStep env c1 c2) : WasmSteps env c1 c2 :=
  .step h .refl

theorem WasmSteps.trans {env c1 c2 c3} (h1 : WasmSteps env c1 c2) (h2 : WasmSteps env c2 c3) :
    WasmSteps env c1 c3 := by
  induction h1 with
  | refl => exact h2
  | step hs _ ih => exact .step hs (ih h2)

/-- Helper for list-prefix uniqueness when lengths match. -/
private theorem append_eq_of_length_eq {α : Type} {a b c d : List α}
    (h : a ++ b = c ++ d) (hlen : a.length = c.length) : a = c ∧ b = d := by
  induction a generalizing c with
  | nil =>
      cases c with
      | nil => exact ⟨rfl, h⟩
      | cons _ _ => cases hlen
  | cons x xs ih =>
      cases c with
      | nil => cases hlen
      | cons y ys =>
          simp only [List.cons_append, List.cons.injEq] at h
          rcases h with ⟨rfl, htail⟩
          simp only [List.length_cons, Nat.add_right_cancel_iff] at hlen
          obtain ⟨rfl, rfl⟩ := ih htail hlen
          exact ⟨rfl, rfl⟩

/-- Every single Wasm small step is deterministic. -/
theorem wasmStep_deterministic {env c c1 c2}
    (h1 : WasmStep env c c1) (h2 : WasmStep env c c2) : c1 = c2 := by
  cases h1 with
  | i32Const => cases h2; rfl
  | i64Const => cases h2; rfl
  | localGet hg1 =>
      cases h2 with
      | localGet hg2 =>
          rw [hg1] at hg2
          injection hg2 with hv
          subst hv
          rfl
  | localSet _ => cases h2; rfl
  | i64Add => cases h2; rfl
  | i64Sub => cases h2; rfl
  | i64Mul => cases h2; rfl
  | callDefined he1 hvs1 hlen1 =>
      cases h2 with
      | callDefined he2 hvs2 hlen2 =>
          rw [he1] at he2
          injection he2 with hf
          subst hf
          have heq := hvs1.symm.trans hvs2
          obtain ⟨rfl, rfl⟩ := append_eq_of_length_eq heq (hlen1.trans hlen2.symm)
          rfl
      | callPushTag hnone => rw [hnone] at he1; cases he1
      | callPopKind hnone => rw [hnone] at he1; cases he1
      | callEnqueue hnone => rw [hnone] at he1; cases he1
      | callRuntimeTick hnone => rw [hnone] at he1; cases he1
  | returnEnd hvs1 hlen1 =>
      cases h2 with
      | returnEnd hvs2 hlen2 =>
          have heq := hvs1.symm.trans hvs2
          obtain ⟨rfl, rfl⟩ := append_eq_of_length_eq heq (hlen1.trans hlen2.symm)
          rfl
  | returnInstr hvs1 hlen1 =>
      cases h2 with
      | returnInstr hvs2 hlen2 =>
          have heq := hvs1.symm.trans hvs2
          obtain ⟨rfl, rfl⟩ := append_eq_of_length_eq heq (hlen1.trans hlen2.symm)
          rfl
  | callPushTag hnone =>
      cases h2 with
      | callDefined he2 _ _ => rw [hnone] at he2; cases he2
      | callPushTag _ => rfl
  | callPopKind hnone =>
      cases h2 with
      | callDefined he2 _ _ => rw [hnone] at he2; cases he2
      | callPopKind _ => rfl
  | callEnqueue hnone =>
      cases h2 with
      | callDefined he2 _ _ => rw [hnone] at he2; cases he2
      | callEnqueue _ => rfl
  | callRuntimeTick hnone =>
      cases h2 with
      | callDefined he2 _ _ => rw [hnone] at he2; cases he2
      | callRuntimeTick _ => rfl

/-- No small step can exit a top-level terminal configuration (`frames = [⟨_, _, [], _⟩]`). -/
theorem wasmStep_terminal_absurd {env locals vstack r st c'}
    (h : WasmStep env ⟨[⟨locals, vstack, [], r⟩], st⟩ c') : False := by
  cases h

/-- Multi-step determinism for configurations reaching a top-level terminal state. -/
theorem wasmSteps_terminal_deterministic {env c l1 v1 r1 s1 l2 v2 r2 s2}
    (h1 : WasmSteps env c ⟨[⟨l1, v1, [], r1⟩], s1⟩)
    (h2 : WasmSteps env c ⟨[⟨l2, v2, [], r2⟩], s2⟩) :
    (⟨[⟨l1, v1, [], r1⟩], s1⟩ : Config) = ⟨[⟨l2, v2, [], r2⟩], s2⟩ := by
  generalize hc1 : (⟨[⟨l1, v1, [], r1⟩], s1⟩ : Config) = c1 at h1
  induction h1 with
  | refl =>
      subst hc1
      cases h2 with
      | refl => rfl
      | step hs _ => exact absurd hs wasmStep_terminal_absurd
  | step hs1 _ ih =>
      subst hc1
      cases h2 with
      | refl => exact absurd hs1 wasmStep_terminal_absurd
      | step hs2 rest2 =>
          have heq := wasmStep_deterministic hs1 hs2
          subst heq
          exact ih rest2 rfl

/-! ## 4. Small-Step Helper Derivations (`$push_i64`, `$op_add`, `$op_sub`, `$op_mul`) -/

/-- Exact small-step execution of `[.i64Const v, .call .pushI64]` in `runtimeEnv`
    from any store `st` (covering normal push, operand quota, and prior failure). -/
theorem pushI64_steps (v : BitVec 64) (st : RuntimeStore) :
    WasmSteps runtimeEnv
      (initConfig [.i64Const v, .call .pushI64] st)
      (doneConfig (storePushTag st tagI64 v)) := by
  unfold initConfig doneConfig
  refine .step .i64Const ?_
  have hcall :
      WasmStep runtimeEnv
        ⟨[⟨[], [.i64 v], [.call .pushI64], 0⟩], st⟩
        ⟨[⟨[.i64 v], [], defPushI64.body, 0⟩, ⟨[], [], [], 0⟩], st⟩ := by
    simpa [defPushI64] using
      (@WasmStep.callDefined runtimeEnv [] [.i64 v] [.i64 v] [] [] 0 [] st .pushI64 defPushI64
        rfl rfl rfl)
  refine .step hcall ?_
  refine .step .i32Const ?_
  refine .step (.localGet (by rfl)) ?_
  refine .step (.callPushTag rfl) ?_
  exact .single (@WasmStep.returnEnd runtimeEnv [.i64 v] [] [] [] 0 [] [] [] 0 [] _ rfl rfl)

/-- Normal execution of `i64.const v; call $push_i64` with sufficient operand quota. -/
theorem pushI64_steps_ok (v : BitVec 64) (st : RuntimeStore)
    (hok : st.status = .ok)
    (hlim : (st.slots.length + 1) * 16 ≤ st.limits.operandLimit) :
    WasmSteps runtimeEnv
      (initConfig [.i64Const v, .call .pushI64] st)
      (doneConfig
        { st with
          slots := ⟨tagI64, v⟩ :: st.slots
          operandPeak := Nat.max st.operandPeak ((st.slots.length + 1) * 16) }) := by
  simpa [storePushTag_ok st tagI64 v hok hlim] using pushI64_steps v st

/-- Operand-quota exhaustion on `i64.const v; call $push_i64` leaves `slots` unchanged
    and records `.quota st.phase .operand` (distinct from a fault). -/
theorem pushI64_steps_quota (v : BitVec 64) (st : RuntimeStore)
    (hok : st.status = .ok)
    (hgt : (st.slots.length + 1) * 16 > st.limits.operandLimit) :
    WasmSteps runtimeEnv
      (initConfig [.i64Const v, .call .pushI64] st)
      (doneConfig { st with status := .quota st.phase .operand }) := by
  simpa [storePushTag_quota st tagI64 v hok hgt] using pushI64_steps v st

/-- Helper: small-step execution of a binary arithmetic helper (`opAdd`, `opSub`, `opMul`)
    with explicit `$right` then `$left` pop order and wrapping 64-bit operation. -/
private theorem binop_body_steps
    (fid : FuncId) (instr : Instr) (op : BitVec 64 → BitVec 64 → BitVec 64)
    (hdef : runtimeEnv fid = some
      { paramCount := 0
        localDefaults := [.i64 0, .i64 0]
        body :=
          [ .i32Const tagI64, .call .popKind, .localSet 0
          , .i32Const tagI64, .call .popKind, .localSet 1
          , .localGet 1, .localGet 0, instr, .call .pushI64 ]
        retArity := 0 })
    (hinstr : ∀ (locals vstack : List WVal) (rest : List Instr) (r : Nat) (cs : List Frame)
        (st : RuntimeStore) (lhs rhs : BitVec 64),
      WasmStep runtimeEnv
        ⟨⟨locals, .i64 rhs :: .i64 lhs :: vstack, instr :: rest, r⟩ :: cs, st⟩
        ⟨⟨locals, .i64 (op lhs rhs) :: vstack, rest, r⟩ :: cs, st⟩)
    (st : RuntimeStore) :
    let p1 := storePopKind st tagI64
    let p2 := storePopKind p1.2 tagI64
    WasmSteps runtimeEnv
      (initConfig [.call fid] st)
      (doneConfig (storePushTag p2.2 tagI64 (op p2.1 p1.1))) := by
  intro p1 p2
  unfold initConfig doneConfig
  have hcall :
      WasmStep runtimeEnv
        ⟨[⟨[], [], [.call fid], 0⟩], st⟩
        ⟨[⟨[.i64 0, .i64 0], [],
          [ .i32Const tagI64, .call .popKind, .localSet 0
          , .i32Const tagI64, .call .popKind, .localSet 1
          , .localGet 1, .localGet 0, instr, .call .pushI64 ], 0⟩,
          ⟨[], [], [], 0⟩], st⟩ := by
    simpa using (@WasmStep.callDefined runtimeEnv [] [] [] [] [] 0 [] st fid _ hdef rfl rfl)
  refine .step hcall ?_
  -- Pop $right (local 0)
  refine .step .i32Const ?_
  refine .step (.callPopKind rfl) ?_
  refine .step (.localSet (by simp)) ?_
  -- Pop $left (local 1)
  refine .step .i32Const ?_
  refine .step (.callPopKind rfl) ?_
  refine .step (.localSet (by simp)) ?_
  -- Compute op $left $right (push $left, push $right, execute instr)
  refine .step (.localGet (by rfl)) ?_
  refine .step (.localGet (by rfl)) ?_
  refine .step (hinstr _ _ _ _ _ _ p2.1 p1.1) ?_
  -- Call $push_i64
  have hpushCall :
      WasmStep runtimeEnv
        ⟨⟨[.i64 p1.1, .i64 p2.1], [.i64 (op p2.1 p1.1)], [.call .pushI64], 0⟩ ::
          [⟨[], [], [], 0⟩], p2.2⟩
        ⟨⟨[.i64 (op p2.1 p1.1)], [], defPushI64.body, 0⟩ ::
          ⟨[.i64 p1.1, .i64 p2.1], [], [], 0⟩ :: [⟨[], [], [], 0⟩], p2.2⟩ := by
    simpa [defPushI64] using
      (@WasmStep.callDefined runtimeEnv [.i64 p1.1, .i64 p2.1]
        [.i64 (op p2.1 p1.1)] [.i64 (op p2.1 p1.1)] [] [] 0 [⟨[], [], [], 0⟩] p2.2
        .pushI64 defPushI64 rfl rfl rfl)
  refine .step hpushCall ?_
  refine .step .i32Const ?_
  refine .step (.localGet (by rfl)) ?_
  refine .step (.callPushTag rfl) ?_
  refine .step (@WasmStep.returnEnd runtimeEnv _ [] [] [] 0 _ _ _ _ _ _ rfl rfl) ?_
  exact .single (@WasmStep.returnEnd runtimeEnv _ [] [] [] 0 _ _ _ _ _ _ rfl rfl)

/-- Exact small-step execution of `[.call .opAdd]` for any runtime store. -/
theorem opAdd_steps (st : RuntimeStore) :
    let p1 := storePopKind st tagI64
    let p2 := storePopKind p1.2 tagI64
    WasmSteps runtimeEnv
      (initConfig [.call .opAdd] st)
      (doneConfig (storePushTag p2.2 tagI64 (p2.1 + p1.1))) :=
  binop_body_steps .opAdd .i64Add (· + ·) rfl (fun _ _ _ _ _ _ _ _ => .i64Add) st

/-- Exact small-step execution of `[.call .opSub]` for any runtime store. -/
theorem opSub_steps (st : RuntimeStore) :
    let p1 := storePopKind st tagI64
    let p2 := storePopKind p1.2 tagI64
    WasmSteps runtimeEnv
      (initConfig [.call .opSub] st)
      (doneConfig (storePushTag p2.2 tagI64 (p2.1 - p1.1))) :=
  binop_body_steps .opSub .i64Sub (· - ·) rfl (fun _ _ _ _ _ _ _ _ => .i64Sub) st

/-- Exact small-step execution of `[.call .opMul]` for any runtime store. -/
theorem opMul_steps (st : RuntimeStore) :
    let p1 := storePopKind st tagI64
    let p2 := storePopKind p1.2 tagI64
    WasmSteps runtimeEnv
      (initConfig [.call .opMul] st)
      (doneConfig (storePushTag p2.2 tagI64 (p2.1 * p1.1))) :=
  binop_body_steps .opMul .i64Mul (· * ·) rfl (fun _ _ _ _ _ _ _ _ => .i64Mul) st

/-- `$op_add` on top-of-stack `[.i64 right, .i64 left] ++ tail` computes `left + right`
    modulo `2^64` and leaves `tail` bit-for-bit unchanged. -/
theorem opAdd_steps_ok (st : RuntimeStore) (left right : BitVec 64) (tail : List TaggedSlot)
    (hok : st.status = .ok)
    (hsl : st.slots = ⟨tagI64, right⟩ :: ⟨tagI64, left⟩ :: tail)
    (hlim : (tail.length + 1) * 16 ≤ st.limits.operandLimit) :
    WasmSteps runtimeEnv
      (initConfig [.call .opAdd] st)
      (doneConfig
        { st with
          slots := ⟨tagI64, left + right⟩ :: tail
          operandPeak := Nat.max st.operandPeak ((tail.length + 1) * 16) }) := by
  have h := opAdd_steps st
  have hp1 : storePopKind st tagI64 = (right, { st with slots := ⟨tagI64, left⟩ :: tail }) :=
    storePopKind_ok st tagI64 right (⟨tagI64, left⟩ :: tail) hok hsl
  have hp2 : storePopKind { st with slots := ⟨tagI64, left⟩ :: tail } tagI64 =
      (left, { st with slots := tail }) :=
    storePopKind_ok { st with slots := ⟨tagI64, left⟩ :: tail } tagI64 left tail hok rfl
  have hpush : storePushTag { st with slots := tail } tagI64 (left + right) =
      { st with
        slots := ⟨tagI64, left + right⟩ :: tail
        operandPeak := Nat.max st.operandPeak ((tail.length + 1) * 16) } :=
    storePushTag_ok { st with slots := tail } tagI64 (left + right) hok hlim
  simpa [hp1, hp2, hpush] using h

/-- `$op_sub` on top-of-stack `[.i64 right, .i64 left] ++ tail` computes `left - right`
    modulo `2^64` (never `right - left`) and leaves `tail` bit-for-bit unchanged. -/
theorem opSub_steps_ok (st : RuntimeStore) (left right : BitVec 64) (tail : List TaggedSlot)
    (hok : st.status = .ok)
    (hsl : st.slots = ⟨tagI64, right⟩ :: ⟨tagI64, left⟩ :: tail)
    (hlim : (tail.length + 1) * 16 ≤ st.limits.operandLimit) :
    WasmSteps runtimeEnv
      (initConfig [.call .opSub] st)
      (doneConfig
        { st with
          slots := ⟨tagI64, left - right⟩ :: tail
          operandPeak := Nat.max st.operandPeak ((tail.length + 1) * 16) }) := by
  have h := opSub_steps st
  have hp1 : storePopKind st tagI64 = (right, { st with slots := ⟨tagI64, left⟩ :: tail }) :=
    storePopKind_ok st tagI64 right (⟨tagI64, left⟩ :: tail) hok hsl
  have hp2 : storePopKind { st with slots := ⟨tagI64, left⟩ :: tail } tagI64 =
      (left, { st with slots := tail }) :=
    storePopKind_ok { st with slots := ⟨tagI64, left⟩ :: tail } tagI64 left tail hok rfl
  have hpush : storePushTag { st with slots := tail } tagI64 (left - right) =
      { st with
        slots := ⟨tagI64, left - right⟩ :: tail
        operandPeak := Nat.max st.operandPeak ((tail.length + 1) * 16) } :=
    storePushTag_ok { st with slots := tail } tagI64 (left - right) hok hlim
  simpa [hp1, hp2, hpush] using h

/-- `$op_mul` on top-of-stack `[.i64 right, .i64 left] ++ tail` computes `left * right`
    modulo `2^64` and leaves `tail` bit-for-bit unchanged. -/
theorem opMul_steps_ok (st : RuntimeStore) (left right : BitVec 64) (tail : List TaggedSlot)
    (hok : st.status = .ok)
    (hsl : st.slots = ⟨tagI64, right⟩ :: ⟨tagI64, left⟩ :: tail)
    (hlim : (tail.length + 1) * 16 ≤ st.limits.operandLimit) :
    WasmSteps runtimeEnv
      (initConfig [.call .opMul] st)
      (doneConfig
        { st with
          slots := ⟨tagI64, left * right⟩ :: tail
          operandPeak := Nat.max st.operandPeak ((tail.length + 1) * 16) }) := by
  have h := opMul_steps st
  have hp1 : storePopKind st tagI64 = (right, { st with slots := ⟨tagI64, left⟩ :: tail }) :=
    storePopKind_ok st tagI64 right (⟨tagI64, left⟩ :: tail) hok hsl
  have hp2 : storePopKind { st with slots := ⟨tagI64, left⟩ :: tail } tagI64 =
      (left, { st with slots := tail }) :=
    storePopKind_ok { st with slots := ⟨tagI64, left⟩ :: tail } tagI64 left tail hok rfl
  have hpush : storePushTag { st with slots := tail } tagI64 (left * right) =
      { st with
        slots := ⟨tagI64, left * right⟩ :: tail
        operandPeak := Nat.max st.operandPeak ((tail.length + 1) * 16) } :=
    storePushTag_ok { st with slots := tail } tagI64 (left * right) hok hlim
  simpa [hp1, hp2, hpush] using h

/-- Stack underflow on `$op_sub` with a 1-element stack produces runtime fault `4`
    (never quota exhaustion). -/
theorem opSub_steps_underflow_one (st : RuntimeStore) (v : BitVec 64)
    (hok : st.status = .ok) (hsl : st.slots = [⟨tagI64, v⟩]) :
    WasmSteps runtimeEnv
      (initConfig [.call .opSub] st)
      (doneConfig { st with slots := [], status := .fault faultCode }) := by
  have h := opSub_steps st
  have hp1 : storePopKind st tagI64 = (v, { st with slots := [] }) :=
    storePopKind_ok st tagI64 v [] hok hsl
  have hp2 : storePopKind { st with slots := [] } tagI64 =
      (0, { st with slots := [], status := .fault faultCode }) :=
    storePopKind_underflow { st with slots := [] } tagI64 hok rfl
  dsimp only at h
  rw [hp1, hp2] at h
  rw [storePushTag_failed _ tagI64 _ (by simp)] at h
  exact h

/-- Operand tag mismatch on `$op_sub` (top slot has `tag ≠ tagI64`) produces runtime
    fault `4` and leaves `slots` unchanged (never quota exhaustion). -/
theorem opSub_steps_tag_mismatch (st : RuntimeStore) (top : TaggedSlot) (rest : List TaggedSlot)
    (hok : st.status = .ok) (hsl : st.slots = top :: rest) (hne : top.tag ≠ tagI64) :
    WasmSteps runtimeEnv
      (initConfig [.call .opSub] st)
      (doneConfig { st with status := .fault faultCode }) := by
  have h := opSub_steps st
  have hp1 : storePopKind st tagI64 = (0, { st with status := .fault faultCode }) :=
    storePopKind_tag_mismatch st tagI64 top rest hok hsl hne
  have hp2 : storePopKind { st with status := .fault faultCode } tagI64 =
      (0, { st with status := .fault faultCode }) :=
    storePopKind_failed _ tagI64 (by simp)
  dsimp only at h
  rw [hp1, hp2] at h
  rw [storePushTag_failed _ tagI64 _ (by simp)] at h
  exact h

/-! ## 5. Bounded Arithmetic Subset, Lowering, and Continuation Dispatch -/

/-- Bounded pure arithmetic operations of the Phase 2 first cut. -/
inductive ArithOp where
  | i64 : BitVec 64 → ArithOp
  | add : ArithOp
  | sub : ArithOp
  | mul : ArithOp
deriving DecidableEq, Repr

/-- Source-side MC1 operation corresponding to an `ArithOp`. -/
def ArithOp.toMc1 : ArithOp → Op
  | .i64 v => .lit (.i64 v)
  | .add => .word 4
  | .sub => .word 5
  | .mul => .word 6

/-- Source-side MC1 code (also the exact reflected recipe) of a bounded arithmetic body. -/
def toMc1Code (ops : List ArithOp) : List Op :=
  ops.map ArithOp.toMc1

/-- Structured Wasm instruction sequence lowered for one `ArithOp`, matching
    `crates/noble-wasm/src/source/emit/functions.rs:120,131,216-219,239-242`
    and `crates/noble-wasm/src/operations.rs:21-23`. -/
def lowerOp : ArithOp → List Instr
  | .i64 v => [.i64Const v, .call .pushI64]
  | .add => [.call .opAdd]
  | .sub => [.call .opSub]
  | .mul => [.call .opMul]

/-- One emitted opt-off continuation entry (`functions.rs:43-104`, `calculator = false`):
    if `next = some k`, it enqueues `(k, env)` before executing `body` when `failure == 0`. -/
structure ContBlock where
  next : Option Nat
  body : List Instr
deriving DecidableEq, Repr

/-- Table of lowered continuation blocks indexed by function-table index. -/
abbrev ProgramTable := Nat → Option ContBlock

/-- Lowering of a bounded arithmetic body `ops` starting at function-table slot `entry`.
    An empty body uses one empty continuation entry (matching `$empty_entry`);
    a nonempty body lowers `ops[i]` to entry `entry + i` chaining to `entry + i + 1`. -/
def lowerProgram (entry : Nat) (ops : List ArithOp) : ProgramTable :=
  fun idx =>
    match ops with
    | [] => if idx == entry then some ⟨none, []⟩ else none
    | _ :: _ =>
        if entry ≤ idx then
          let off := idx - entry
          match ops[off]? with
          | some op =>
              let next := if off + 1 < ops.length then some (idx + 1) else none
              some ⟨next, lowerOp op⟩
          | none => none
        else
          none

/-- Function-table index range validity (`entry + max 1 ops.length < 2^32`, implied by
    the 16,384-entry Wasm table in `imports.rs:60`). -/
def EntryRangeValid (entry : Nat) (ops : List ArithOp) : Prop :=
  entry + Nat.max 1 ops.length < 2 ^ 32

/-- Execution of one continuation block (`functions.rs:50-103`) through `WasmSteps`:
    1. If `failure ≠ 0`, return immediately.
    2. If `next = some k`, execute `i32.const k; i32.const env; call $enqueue`.
    3. If `failure = 0` after enqueue, execute `blk.body`. -/
inductive ExecContBlock (env : FuncEnv) (blk : ContBlock) (envId : BitVec 32) :
    RuntimeStore → RuntimeStore → Prop where
  | alreadyFailed {st} :
      st.status ≠ .ok →
      ExecContBlock env blk envId st st
  | noNext {st sFinal} :
      st.status = .ok →
      blk.next = none →
      WasmSteps env (initConfig blk.body st) (doneConfig sFinal) →
      ExecContBlock env blk envId st sFinal
  | withNextFailed {st sEnq nextIdx} :
      st.status = .ok →
      blk.next = some nextIdx →
      WasmSteps env
        (initConfig [.i32Const (BitVec.ofNat 32 nextIdx), .i32Const envId, .call .enqueue] st)
        (doneConfig sEnq) →
      sEnq.status ≠ .ok →
      ExecContBlock env blk envId st sEnq
  | withNextOk {st sEnq sFinal nextIdx} :
      st.status = .ok →
      blk.next = some nextIdx →
      WasmSteps env
        (initConfig [.i32Const (BitVec.ofNat 32 nextIdx), .i32Const envId, .call .enqueue] st)
        (doneConfig sEnq) →
      sEnq.status = .ok →
      WasmSteps env (initConfig blk.body sEnq) (doneConfig sFinal) →
      ExecContBlock env blk envId st sFinal

/-- One iteration of the `$dispatch` loop (`stack.wat:78-88`):
    checks `failure == 0 ∧ cp > 0`, runs `$runtime_tick`, pops the top continuation,
    and invokes its lowered block (faulting with `4` if the table index is unbound). -/
inductive DispatchStep (env : FuncEnv) (tbl : ProgramTable) :
    RuntimeStore → RuntimeStore → Prop where
  | tickQuota {st sTick c rest} :
      st.status = .ok →
      st.conts = c :: rest →
      WasmSteps env
        ⟨[⟨[], [], [.call .runtimeTick], 1⟩], st⟩
        (doneValConfig (.i32 0) sTick) →
      DispatchStep env tbl st sTick
  | invokeOk {st sTick sFinal c rest blk} :
      st.status = .ok →
      st.conts = c :: rest →
      WasmSteps env
        ⟨[⟨[], [], [.call .runtimeTick], 1⟩], st⟩
        (doneValConfig (.i32 1) sTick) →
      tbl c.fnIdx.toNat = some blk →
      ExecContBlock env blk c.env { sTick with conts := rest } sFinal →
      DispatchStep env tbl st sFinal
  | invokeMissing {st sTick c rest} :
      st.status = .ok →
      st.conts = c :: rest →
      WasmSteps env
        ⟨[⟨[], [], [.call .runtimeTick], 1⟩], st⟩
        (doneValConfig (.i32 1) sTick) →
      tbl c.fnIdx.toNat = none →
      DispatchStep env tbl st ({ sTick with conts := rest }.setFault faultCode)

/-- Full `$dispatch` execution to termination (`status ≠ .ok ∨ conts = []`). -/
inductive DispatchRun (env : FuncEnv) (tbl : ProgramTable) :
    RuntimeStore → RuntimeStore → Prop where
  | halted {st} :
      (st.status ≠ .ok ∨ st.conts = []) →
      DispatchRun env tbl st st
  | step {s0 s1 s2} :
      DispatchStep env tbl s0 s1 →
      DispatchRun env tbl s1 s2 →
      DispatchRun env tbl s0 s2

/-- Complete execution of a lowered program from entry `entry`:
    enqueues `(entry, 0)` (matching `submit`'s `(call $enqueue_program root)`)
    and runs `$dispatch` until halted. -/
def runLowered (env : FuncEnv) (entry : Nat) (ops : List ArithOp)
    (st final : RuntimeStore) : Prop :=
  ∃ sEnq,
    WasmSteps env
      (initConfig [.i32Const (BitVec.ofNat 32 entry), .i32Const 0, .call .enqueue] st)
      (doneConfig sEnq) ∧
    DispatchRun env (lowerProgram entry ops) sEnq final

/-- Exact small-step derivation for `i32.const fnIdx; i32.const envId; call $enqueue`. -/
theorem enqueue_steps (env : FuncEnv) (hnone : env .enqueue = none)
    (fnIdx envId : BitVec 32) (st : RuntimeStore) :
    WasmSteps env
      (initConfig [.i32Const fnIdx, .i32Const envId, .call .enqueue] st)
      (doneConfig (storeEnqueue st fnIdx envId)) := by
  unfold initConfig doneConfig
  refine .step .i32Const ?_
  refine .step .i32Const ?_
  exact .single (.callEnqueue hnone)

/-- Exact small-step derivation for `call $runtimeTick`. -/
theorem runtimeTick_steps (env : FuncEnv) (hnone : env .runtimeTick = none) (st : RuntimeStore) :
    WasmSteps env
      ⟨[⟨[], [], [.call .runtimeTick], 1⟩], st⟩
      (doneValConfig (.i32 (storeRuntimeTick st).1) (storeRuntimeTick st).2) :=
  .single (.callRuntimeTick hnone)

/-- Determinism of `ExecContBlock`. -/
theorem execContBlock_deterministic {env blk envId st s1 s2}
    (h1 : ExecContBlock env blk envId st s1)
    (h2 : ExecContBlock env blk envId st s2) : s1 = s2 := by
  cases h1 with
  | alreadyFailed hne =>
      cases h2 with
      | alreadyFailed _ => rfl
      | noNext hok _ _ => exact absurd hok hne
      | withNextFailed hok _ _ _ => exact absurd hok hne
      | withNextOk hok _ _ _ _ => exact absurd hok hne
  | noNext hok hnone hw1 =>
      cases h2 with
      | alreadyFailed hne => exact absurd hok hne
      | noNext _ _ hw2 =>
          exact doneConfig_inj (wasmSteps_terminal_deterministic hw1 hw2)
      | withNextFailed _ hsome _ _ => rw [hnone] at hsome; cases hsome
      | withNextOk _ hsome _ _ _ => rw [hnone] at hsome; cases hsome
  | withNextFailed hok hsome1 he1 hfail1 =>
      cases h2 with
      | alreadyFailed hne => exact absurd hok hne
      | noNext _ hnone _ => rw [hnone] at hsome1; cases hsome1
      | withNextFailed _ hsome2 he2 _ =>
          rw [hsome1] at hsome2; injection hsome2 with hn; subst hn
          exact doneConfig_inj (wasmSteps_terminal_deterministic he1 he2)
      | withNextOk _ hsome2 he2 hok2 _ =>
          rw [hsome1] at hsome2; injection hsome2 with hn; subst hn
          have heq := doneConfig_inj (wasmSteps_terminal_deterministic he1 he2)
          subst heq
          exact absurd hok2 hfail1
  | withNextOk hok hsome1 he1 hok1 hw1 =>
      cases h2 with
      | alreadyFailed hne => exact absurd hok hne
      | noNext _ hnone _ => rw [hnone] at hsome1; cases hsome1
      | withNextFailed _ hsome2 he2 hfail2 =>
          rw [hsome1] at hsome2; injection hsome2 with hn; subst hn
          have heq := doneConfig_inj (wasmSteps_terminal_deterministic he1 he2)
          subst heq
          exact absurd hok1 hfail2
      | withNextOk _ hsome2 he2 _ hw2 =>
          rw [hsome1] at hsome2; injection hsome2 with hn; subst hn
          have heq := doneConfig_inj (wasmSteps_terminal_deterministic he1 he2)
          subst heq
          exact doneConfig_inj (wasmSteps_terminal_deterministic hw1 hw2)

/-- Determinism of `DispatchStep`. -/
theorem dispatchStep_deterministic {env tbl s0 s1 s2}
    (h1 : DispatchStep env tbl s0 s1) (h2 : DispatchStep env tbl s0 s2) : s1 = s2 := by
  cases h1 with
  | tickQuota _ hc1 ht1 =>
      cases h2 with
      | tickQuota _ _ ht2 =>
          exact (doneValConfig_inj (wasmSteps_terminal_deterministic ht1 ht2)).2
      | invokeOk _ _ ht2 _ _ =>
          have hv := (doneValConfig_inj (wasmSteps_terminal_deterministic ht1 ht2)).1
          cases hv
      | invokeMissing _ _ ht2 _ =>
          have hv := (doneValConfig_inj (wasmSteps_terminal_deterministic ht1 ht2)).1
          cases hv
  | invokeOk _ hc1 ht1 htbl1 hex1 =>
      cases h2 with
      | tickQuota _ _ ht2 =>
          have hv := (doneValConfig_inj (wasmSteps_terminal_deterministic ht1 ht2)).1
          cases hv
      | invokeOk _ hc2 ht2 htbl2 hex2 =>
          rw [hc1] at hc2; injection hc2 with hce hcr; subst hce; subst hcr
          have hs := (doneValConfig_inj (wasmSteps_terminal_deterministic ht1 ht2)).2
          subst hs
          rw [htbl1] at htbl2; injection htbl2 with hblk; subst hblk
          exact execContBlock_deterministic hex1 hex2
      | invokeMissing _ hc2 _ htbl2 =>
          rw [hc1] at hc2; injection hc2 with hce _; subst hce
          rw [htbl1] at htbl2; cases htbl2
  | invokeMissing _ hc1 ht1 htbl1 =>
      cases h2 with
      | tickQuota _ _ ht2 =>
          have hv := (doneValConfig_inj (wasmSteps_terminal_deterministic ht1 ht2)).1
          cases hv
      | invokeOk _ hc2 _ htbl2 _ =>
          rw [hc1] at hc2; injection hc2 with hce _; subst hce
          rw [htbl1] at htbl2; cases htbl2
      | invokeMissing _ hc2 ht2 _ =>
          rw [hc1] at hc2; injection hc2 with hce hcr; subst hce; subst hcr
          have hs := (doneValConfig_inj (wasmSteps_terminal_deterministic ht1 ht2)).2
          subst hs; rfl

/-- No `DispatchStep` can leave a halted store (`status ≠ .ok ∨ conts = []`). -/
theorem dispatchStep_halted_absurd {env tbl s0 s1}
    (hhalt : s0.status ≠ .ok ∨ s0.conts = []) (hs : DispatchStep env tbl s0 s1) : False := by
  cases hs with
  | tickQuota hok hc _ =>
      rcases hhalt with hne | hnil
      · exact hne hok
      · rw [hnil] at hc; cases hc
  | invokeOk hok hc _ _ _ =>
      rcases hhalt with hne | hnil
      · exact hne hok
      · rw [hnil] at hc; cases hc
  | invokeMissing hok hc _ _ =>
      rcases hhalt with hne | hnil
      · exact hne hok
      · rw [hnil] at hc; cases hc

/-- Determinism of `DispatchRun`. -/
theorem dispatchRun_deterministic {env tbl s0 s1 s2}
    (h1 : DispatchRun env tbl s0 s1) (h2 : DispatchRun env tbl s0 s2) : s1 = s2 := by
  induction h1 with
  | halted hhalt =>
      cases h2 with
      | halted _ => rfl
      | step hs _ => exact absurd hs (dispatchStep_halted_absurd hhalt)
  | step hs1 _ ih =>
      cases h2 with
      | halted hhalt => exact absurd hs1 (dispatchStep_halted_absurd hhalt)
      | step hs2 hr2 =>
          have heq := dispatchStep_deterministic hs1 hs2
          subst heq
          exact ih hr2

/-- Determinism of `runLowered`. -/
theorem runLowered_deterministic {env entry ops st s1 s2}
    (h1 : runLowered env entry ops st s1) (h2 : runLowered env entry ops st s2) : s1 = s2 := by
  rcases h1 with ⟨se1, he1, hd1⟩
  rcases h2 with ⟨se2, he2, hd2⟩
  have heq := doneConfig_inj (wasmSteps_terminal_deterministic he1 he2)
  subst heq
  exact dispatchRun_deterministic hd1 hd2

/-! ## 6. Stack Refinement Relation (Independent of Lowering) -/

/-- Relation between one MC1 `Value` and one Wasm `TaggedSlot`.
    Scalars (`i64`, `bool`, `unit`) are bit-exact; heap-referenced values carry their
    exact runtime tag (`≠ tagI64`) while linear-memory cell layouts stay abstract. -/
def SlotRel : Value → TaggedSlot → Prop
  | .i64 n, slot => slot = ⟨tagI64, n⟩
  | .bool b, slot => slot = ⟨tagBool, if b then 1 else 0⟩
  | .unit, slot => slot = ⟨tagUnit, 0⟩
  | .program _, slot => slot.tag = tagProgram
  | .pair _ _, slot => slot.tag = tagPair
  | .list [], slot => slot.tag = tagNil
  | .list (_ :: _), slot => slot.tag = tagCons
  | .«syntax» _, slot => slot.tag = tagSyntax
  | .text _, slot => slot.tag = tagText
  | .inl _, slot => slot.tag = tagInl
  | .inr _, slot => slot.tag = tagInr

/-- A slot has `tag = tagI64` under `SlotRel` iff the MC1 value is `.i64 slot.payload`. -/
theorem slotRel_i64_iff {v : Value} {slot : TaggedSlot} (h : SlotRel v slot) :
    slot.tag = tagI64 ↔ v = .i64 slot.payload := by
  cases v with
  | i64 n =>
      simp only [SlotRel] at h
      subst h
      simp
  | bool b =>
      simp only [SlotRel] at h
      subst h
      constructor
      · intro htag
        simp only [tagBool, tagI64] at htag
        exact absurd htag (by decide)
      · intro hv; cases hv
  | unit =>
      simp only [SlotRel] at h
      subst h
      constructor
      · intro htag
        simp only [tagUnit, tagI64] at htag
        exact absurd htag (by decide)
      · intro hv; cases hv
  | text _ =>
      simp only [SlotRel] at h
      constructor
      · intro htag; rw [h] at htag; exact absurd htag (by decide)
      · intro hv; cases hv
  | pair _ _ =>
      simp only [SlotRel] at h
      constructor
      · intro htag; rw [h] at htag; exact absurd htag (by decide)
      · intro hv; cases hv
  | inl _ =>
      simp only [SlotRel] at h
      constructor
      · intro htag; rw [h] at htag; exact absurd htag (by decide)
      · intro hv; cases hv
  | inr _ =>
      simp only [SlotRel] at h
      constructor
      · intro htag; rw [h] at htag; exact absurd htag (by decide)
      · intro hv; cases hv
  | list vs =>
      cases vs with
      | nil =>
          simp only [SlotRel] at h
          constructor
          · intro htag; rw [h] at htag; exact absurd htag (by decide)
          · intro hv; cases hv
      | cons _ _ =>
          simp only [SlotRel] at h
          constructor
          · intro htag; rw [h] at htag; exact absurd htag (by decide)
          · intro hv; cases hv
  | program _ =>
      simp only [SlotRel] at h
      constructor
      · intro htag; rw [h] at htag; exact absurd htag (by decide)
      · intro hv; cases hv
  | «syntax» _ =>
      simp only [SlotRel] at h
      constructor
      · intro htag; rw [h] at htag; exact absurd htag (by decide)
      · intro hv; cases hv

/-- Top-first stack refinement relation. -/
inductive StackRelTop : Stack → List TaggedSlot → Prop where
  | nil : StackRelTop [] []
  | cons {v vs slot slots} :
      SlotRel v slot → StackRelTop vs slots → StackRelTop (v :: vs) (slot :: slots)

/-- Public stack refinement relation: relates a bottom-first MC1 `Stack` to the
    Wasm store's top-first slot stack. -/
def StackRel (s : Stack) (slots : List TaggedSlot) : Prop :=
  StackRelTop s.reverse slots

theorem stackRelTop_length {s : Stack} {slots : List TaggedSlot} (h : StackRelTop s slots) :
    s.length = slots.length := by
  induction h with
  | nil => rfl
  | cons _ _ ih => simp [ih]

theorem stackRel_length {s : Stack} {slots : List TaggedSlot} (h : StackRel s slots) :
    s.length = slots.length := by
  have := stackRelTop_length h
  simpa using this

theorem stackRelTop_append {s1 s2 : Stack} {t1 t2 : List TaggedSlot}
    (h1 : StackRelTop s1 t1) (h2 : StackRelTop s2 t2) :
    StackRelTop (s1 ++ s2) (t1 ++ t2) := by
  induction h1 with
  | nil => exact h2
  | cons hv _ ih => exact .cons hv ih

/-- Frame composition for bottom-first stacks: `tail ++ top` refines `topSlots ++ tailSlots`. -/
theorem stackRel_append {tail top : Stack} {tailSlots topSlots : List TaggedSlot}
    (htail : StackRel tail tailSlots) (htop : StackRel top topSlots) :
    StackRel (tail ++ top) (topSlots ++ tailSlots) := by
  unfold StackRel at *
  rw [List.reverse_append]
  exact stackRelTop_append htop htail

theorem stackRel_snoc_iff {s : Stack} {v : Value} {slot : TaggedSlot} {slots : List TaggedSlot} :
    StackRel (s ++ [v]) (slot :: slots) ↔ SlotRel v slot ∧ StackRel s slots := by
  unfold StackRel
  simp only [List.reverse_append, List.reverse_cons, List.reverse_nil, List.nil_append,
    List.cons_append]
  constructor
  · intro h; cases h with
    | cons hv hs => exact ⟨hv, hs⟩
  · rintro ⟨hv, hs⟩; exact .cons hv hs

/-- Inversion of `StackRel` on a singleton stack `[v]`. -/
theorem stackRel_singleton_iff {v : Value} {slots : List TaggedSlot} :
    StackRel [v] slots ↔ ∃ slot, slots = [slot] ∧ SlotRel v slot := by
  unfold StackRel
  simp only [List.reverse_cons, List.reverse_nil, List.nil_append]
  constructor
  · intro h
    cases h with
    | cons hv hnil =>
        cases hnil
        exact ⟨_, rfl, hv⟩
  · rintro ⟨slot, rfl, hv⟩
    exact .cons hv .nil

/-- Inversion of `StackRelTop` on two top elements. -/
theorem stackRelTop_cons2_inv {a b : Value} {s : Stack} {slots : List TaggedSlot}
    (h : StackRelTop (b :: a :: s) slots) :
    ∃ sb sa tail, slots = sb :: sa :: tail ∧ SlotRel b sb ∧ SlotRel a sa ∧ StackRelTop s tail := by
  cases h with
  | cons hvb hrest =>
      cases hrest with
      | cons hva htail =>
          exact ⟨_, _, _, rfl, hvb, hva, htail⟩

/-! ## 7. Per-Operation Simulation Against MC1 `Step` and `Exec` -/

/-- Exact operand-stack peak depth (in slots) reached by executing `ops` from an
    initial stack depth `d`. -/
def peakSlots : List ArithOp → Nat → Nat
  | [], d => d
  | .i64 _ :: rest, d => Nat.max (d + 1) (peakSlots rest (d + 1))
  | .add :: rest, d => Nat.max d (peakSlots rest (d - 1))
  | .sub :: rest, d => Nat.max d (peakSlots rest (d - 1))
  | .mul :: rest, d => Nat.max d (peakSlots rest (d - 1))

/-- Exact `$dispatch` step count required by `ops` (`1` for empty `$empty_entry`,
    `ops.length` for nonempty bodies). -/
def neededSteps (ops : List ArithOp) : Nat :=
  Nat.max 1 ops.length

theorem peakSlots_ge_init (ops : List ArithOp) (d : Nat) : d ≤ peakSlots ops d := by
  cases ops with
  | nil => exact Nat.le_refl d
  | cons op rest =>
      cases op with
      | i64 _ => exact Nat.le_trans (Nat.le_succ d) (Nat.le_max_left _ _)
      | add => exact Nat.le_max_left _ _
      | sub => exact Nat.le_max_left _ _
      | mul => exact Nat.le_max_left _ _

/-- Helper for binary arithmetic soundness: shared across `add`, `sub`, `mul`. -/
private theorem binop_sound_top
    (op : BitVec 64 → BitVec 64 → BitVec 64)
    (wordIdx : Nat)
    (hstepCtor : ∀ (a b : BitVec 64) (s : Stack),
      NobleContracts.Step (.word wordIdx) (.i64 b :: .i64 a :: s) (.i64 (op a b) :: s))
    (beforeRev : Stack) (st : RuntimeStore)
    (hrel : StackRelTop beforeRev st.slots)
    (hok : st.status = .ok)
    (hfinalOk :
      let p1 := storePopKind st tagI64
      let p2 := storePopKind p1.2 tagI64
      (storePushTag p2.2 tagI64 (op p2.1 p1.1)).status = .ok) :
    let p1 := storePopKind st tagI64
    let p2 := storePopKind p1.2 tagI64
    ∃ afterRev,
      StackRelTop afterRev (storePushTag p2.2 tagI64 (op p2.1 p1.1)).slots ∧
      NobleContracts.Step (.word wordIdx) beforeRev afterRev := by
  dsimp only at hfinalOk ⊢
  generalize hsl0 : st.slots = sl0 at hrel
  cases hrel with
  | nil =>
      have hp1 := storePopKind_underflow st tagI64 hok hsl0
      have hp2 := storePopKind_failed { st with status := .fault faultCode } tagI64 (by simp)
      rw [hp1, hp2, storePushTag_failed _ tagI64 _ (by simp)] at hfinalOk
      cases hfinalOk
  | cons hvr hrest =>
      rename_i vr vs1 sr slots1
      by_cases htr : sr.tag = tagI64
      · have hvrEq : vr = .i64 sr.payload := (slotRel_i64_iff hvr).mp htr
        subst hvrEq
        have hsr : sr = ⟨tagI64, sr.payload⟩ := hvr
        have hp1 : storePopKind st tagI64 = (sr.payload, { st with slots := slots1 }) := by
          rw [hsr] at hsl0
          exact storePopKind_ok st tagI64 sr.payload slots1 hok hsl0
        cases hrest with
        | nil =>
            have hp2 := storePopKind_underflow { st with slots := [] } tagI64 hok rfl
            rw [hp1, hp2, storePushTag_failed _ tagI64 _ (by simp)] at hfinalOk
            cases hfinalOk
        | cons hvl htail =>
            rename_i vl vs2 sl tailSlots
            by_cases htl : sl.tag = tagI64
            · have hvlEq : vl = .i64 sl.payload := (slotRel_i64_iff hvl).mp htl
              subst hvlEq
              have hsl : sl = ⟨tagI64, sl.payload⟩ := hvl
              have hp2 : storePopKind { st with slots := sl :: tailSlots } tagI64 =
                  (sl.payload, { st with slots := tailSlots }) := by
                exact storePopKind_ok { st with slots := sl :: tailSlots } tagI64 sl.payload
                  tailSlots hok (by rw [hsl])
              by_cases hgt : (tailSlots.length + 1) * 16 > st.limits.operandLimit
              · have hpush := storePushTag_quota { st with slots := tailSlots } tagI64
                  (op sl.payload sr.payload) hok hgt
                simp [hp1, hp2, hpush] at hfinalOk
              · have hlim : (tailSlots.length + 1) * 16 ≤ st.limits.operandLimit :=
                  Nat.not_lt.mp hgt
                have hpush := storePushTag_ok { st with slots := tailSlots } tagI64
                  (op sl.payload sr.payload) hok hlim
                simp only [hp1, hp2, hpush]
                exact ⟨.i64 (op sl.payload sr.payload) :: vs2, .cons rfl htail,
                  hstepCtor sl.payload sr.payload vs2⟩
            · have hp2 := storePopKind_tag_mismatch { st with slots := sl :: tailSlots }
                tagI64 sl tailSlots hok rfl htl
              rw [hp1, hp2, storePushTag_failed _ tagI64 _ (by simp)] at hfinalOk
              cases hfinalOk
      · have hp1 := storePopKind_tag_mismatch st tagI64 sr slots1 hok hsl0 htr
        have hp2 := storePopKind_failed { st with status := .fault faultCode } tagI64 (by simp)
        rw [hp1, hp2, storePushTag_failed _ tagI64 _ (by simp)] at hfinalOk
        cases hfinalOk

/-- **Per-operation Soundness (top-first)**: if `lowerOp op` runs cleanly in `runtimeEnv`
    from an `.ok` store refining `beforeRev` to an `.ok` final store, then MC1 `Step`
    transitions `beforeRev` to a matching `afterRev`. -/
theorem lowerOp_sound_top (op : ArithOp) (beforeRev : Stack) (st final : RuntimeStore)
    (hrel : StackRelTop beforeRev st.slots)
    (hok : st.status = .ok)
    (hw : WasmSteps runtimeEnv (initConfig (lowerOp op) st) (doneConfig final))
    (hfinalOk : final.status = .ok) :
    ∃ afterRev, StackRelTop afterRev final.slots ∧ NobleContracts.Step op.toMc1 beforeRev afterRev := by
  cases op with
  | i64 v =>
      have hdet := doneConfig_inj (wasmSteps_terminal_deterministic hw (pushI64_steps v st))
      subst hdet
      by_cases hgt : (st.slots.length + 1) * 16 > st.limits.operandLimit
      · rw [storePushTag_quota st tagI64 v hok hgt] at hfinalOk
        cases hfinalOk
      · have hlim : (st.slots.length + 1) * 16 ≤ st.limits.operandLimit := Nat.not_lt.mp hgt
        rw [storePushTag_ok st tagI64 v hok hlim]
        exact ⟨.i64 v :: beforeRev, .cons rfl hrel, .lit⟩
  | add =>
      have hdet := doneConfig_inj (wasmSteps_terminal_deterministic hw (opAdd_steps st))
      subst hdet
      exact binop_sound_top (· + ·) 4 (fun _ _ _ => .add) beforeRev st hrel hok hfinalOk
  | sub =>
      have hdet := doneConfig_inj (wasmSteps_terminal_deterministic hw (opSub_steps st))
      subst hdet
      exact binop_sound_top (· - ·) 5 (fun _ _ _ => .sub) beforeRev st hrel hok hfinalOk
  | mul =>
      have hdet := doneConfig_inj (wasmSteps_terminal_deterministic hw (opMul_steps st))
      subst hdet
      exact binop_sound_top (· * ·) 6 (fun _ _ _ => .mul) beforeRev st hrel hok hfinalOk

/-- **Per-operation Soundness (public bottom-first `Exec`)**. -/
theorem lowerOp_sound (op : ArithOp) (before : Stack) (st final : RuntimeStore)
    (hrel : StackRel before st.slots)
    (hok : st.status = .ok)
    (hw : WasmSteps runtimeEnv (initConfig (lowerOp op) st) (doneConfig final))
    (hfinalOk : final.status = .ok) :
    ∃ after, StackRel after final.slots ∧ NobleContracts.Exec [op.toMc1] before after := by
  obtain ⟨afterRev, hrel', hstep⟩ := lowerOp_sound_top op before.reverse st final hrel hok hw hfinalOk
  refine ⟨afterRev.reverse, ?_, ?_⟩
  · simpa [StackRel] using hrel'
  · simpa [Exec] using Run.cons hstep .nil

/-- **Per-operation Completeness (top-first)**: every MC1 `Step` of an `ArithOp`
    is simulated by `lowerOp op` in `runtimeEnv` whenever the resulting stack fits
    within `operandLimit`, preserving `conts`, `steps`, `limits`, and `phase`. -/
theorem lowerOp_complete_top (op : ArithOp) (beforeRev afterRev : Stack) (st : RuntimeStore)
    (hstep : NobleContracts.Step op.toMc1 beforeRev afterRev)
    (hrel : StackRelTop beforeRev st.slots)
    (hok : st.status = .ok)
    (hlim : afterRev.length * 16 ≤ st.limits.operandLimit) :
    ∃ final,
      WasmSteps runtimeEnv (initConfig (lowerOp op) st) (doneConfig final) ∧
      final.status = .ok ∧
      StackRelTop afterRev final.slots ∧
      final.conts = st.conts ∧
      final.steps = st.steps ∧
      final.limits = st.limits ∧
      final.phase = st.phase := by
  cases op with
  | i64 v =>
      cases hstep
      have hlen : (st.slots.length + 1) * 16 ≤ st.limits.operandLimit := by
        simpa [stackRelTop_length hrel] using hlim
      refine ⟨_, pushI64_steps_ok v st hok hlen, hok, .cons rfl hrel, rfl, rfl, rfl, rfl⟩
  | add =>
      cases hstep with
      | add =>
          obtain ⟨sb, sa, tail, hsl, hvr, hvl, htail⟩ := stackRelTop_cons2_inv hrel
          simp only [SlotRel] at hvr hvl
          have hlen : (tail.length + 1) * 16 ≤ st.limits.operandLimit := by
            simpa [stackRelTop_length htail] using hlim
          refine ⟨_, opAdd_steps_ok st _ _ _ hok (by rw [hsl, hvr, hvl]) hlen,
            hok, .cons rfl htail, rfl, rfl, rfl, rfl⟩
  | sub =>
      cases hstep with
      | sub =>
          obtain ⟨sb, sa, tail, hsl, hvr, hvl, htail⟩ := stackRelTop_cons2_inv hrel
          simp only [SlotRel] at hvr hvl
          have hlen : (tail.length + 1) * 16 ≤ st.limits.operandLimit := by
            simpa [stackRelTop_length htail] using hlim
          refine ⟨_, opSub_steps_ok st _ _ _ hok (by rw [hsl, hvr, hvl]) hlen,
            hok, .cons rfl htail, rfl, rfl, rfl, rfl⟩
  | mul =>
      cases hstep with
      | mul =>
          obtain ⟨sb, sa, tail, hsl, hvr, hvl, htail⟩ := stackRelTop_cons2_inv hrel
          simp only [SlotRel] at hvr hvl
          have hlen : (tail.length + 1) * 16 ≤ st.limits.operandLimit := by
            simpa [stackRelTop_length htail] using hlim
          refine ⟨_, opMul_steps_ok st _ _ _ hok (by rw [hsl, hvr, hvl]) hlen,
            hok, .cons rfl htail, rfl, rfl, rfl, rfl⟩

/-- **Per-operation Completeness (public bottom-first `Exec`)**. -/
theorem lowerOp_complete (op : ArithOp) (before after : Stack) (st : RuntimeStore)
    (hexec : NobleContracts.Exec [op.toMc1] before after)
    (hrel : StackRel before st.slots)
    (hok : st.status = .ok)
    (hlim : after.length * 16 ≤ st.limits.operandLimit) :
    ∃ final,
      WasmSteps runtimeEnv (initConfig (lowerOp op) st) (doneConfig final) ∧
      final.status = .ok ∧
      StackRel after final.slots := by
  cases hexec with
  | cons hstep hnil =>
      cases hnil
      have hlim' : after.reverse.length * 16 ≤ st.limits.operandLimit := by simpa using hlim
      obtain ⟨final, hw, hfok, hrel', _, _, _, _⟩ :=
        lowerOp_complete_top op before.reverse after.reverse st hstep hrel hok hlim'
      exact ⟨final, hw, hfok, hrel'⟩

/-! ## 8. Whole-Body Continuation Dispatch Simulation Against MC1 `Exec` -/

/-- `lowerOp` leaves the continuation stack `conts` untouched. -/
private theorem lowerOp_preserves_conts (op : ArithOp) (st final : RuntimeStore)
    (hw : WasmSteps runtimeEnv (initConfig (lowerOp op) st) (doneConfig final)) :
    final.conts = st.conts := by
  cases op with
  | i64 v =>
      have hdet := doneConfig_inj (wasmSteps_terminal_deterministic hw (pushI64_steps v st))
      subst hdet
      exact storePushTag_conts st tagI64 v
  | add =>
      have hdet := doneConfig_inj (wasmSteps_terminal_deterministic hw (opAdd_steps st))
      subst hdet
      rw [storePushTag_conts, storePopKind_conts, storePopKind_conts]
  | sub =>
      have hdet := doneConfig_inj (wasmSteps_terminal_deterministic hw (opSub_steps st))
      subst hdet
      rw [storePushTag_conts, storePopKind_conts, storePopKind_conts]
  | mul =>
      have hdet := doneConfig_inj (wasmSteps_terminal_deterministic hw (opMul_steps st))
      subst hdet
      rw [storePushTag_conts, storePopKind_conts, storePopKind_conts]

/-- Helper: `(BitVec.ofNat 32 k).toNat = k` when `k < 2^32`. -/
private theorem bv32_ofNat_toNat {k : Nat} (hk : k < 2 ^ 32) :
    (BitVec.ofNat 32 k).toNat = k := by
  simp [BitVec.toNat_ofNat, Nat.mod_eq_of_lt hk]

/-- Helper: extract the exact tick outcome from a `WasmSteps` derivation of `call $runtimeTick`. -/
private theorem runtimeTick_zero_implies_quota {st sTick : RuntimeStore}
    (hok : st.status = .ok)
    (htick : WasmSteps runtimeEnv
      ⟨[⟨[], [], [.call .runtimeTick], 1⟩], st⟩ (doneValConfig (.i32 0) sTick)) :
    sTick.status = .quota st.phase .steps := by
  obtain ⟨hv, hs⟩ := doneValConfig_inj
    (wasmSteps_terminal_deterministic htick (runtimeTick_steps runtimeEnv rfl st))
  by_cases hge : st.steps ≥ st.limits.stepLimit
  · rw [storeRuntimeTick_quota st hok hge] at hs
    subst hs; rfl
  · have hlt : st.steps < st.limits.stepLimit := Nat.not_le.mp hge
    rw [storeRuntimeTick_ok st hok hlt] at hv
    cases hv

private theorem runtimeTick_one_implies_ok {st sTick : RuntimeStore}
    (hok : st.status = .ok)
    (htick : WasmSteps runtimeEnv
      ⟨[⟨[], [], [.call .runtimeTick], 1⟩], st⟩ (doneValConfig (.i32 1) sTick)) :
    sTick = { st with steps := st.steps + 1 } := by
  obtain ⟨hv, hs⟩ := doneValConfig_inj
    (wasmSteps_terminal_deterministic htick (runtimeTick_steps runtimeEnv rfl st))
  by_cases hge : st.steps ≥ st.limits.stepLimit
  · rw [storeRuntimeTick_quota st hok hge] at hv
    cases hv
  · have hlt : st.steps < st.limits.stepLimit := Nat.not_le.mp hge
    rw [storeRuntimeTick_ok st hok hlt] at hs
    exact hs

/-- Suffix soundness for `DispatchRun` starting at continuation offset `off` inside
    a nonempty body `ops`. -/
private theorem dispatch_suffix_sound
    (entry : Nat) (ops : List ArithOp) (hne : ops ≠ [])
    (hrange : EntryRangeValid entry ops) :
    ∀ (rem : Nat) (off : Nat) (beforeRev : Stack) (st final : RuntimeStore) (envId : BitVec 32),
      rem = ops.length - off →
      off < ops.length →
      st.status = .ok →
      st.conts = [⟨BitVec.ofNat 32 (entry + off), envId⟩] →
      StackRelTop beforeRev st.slots →
      DispatchRun runtimeEnv (lowerProgram entry ops) st final →
      final.status = .ok →
      ∃ afterRev,
        StackRelTop afterRev final.slots ∧
        final.conts = [] ∧
        NobleContracts.Run (toMc1Code (ops.drop off)) beforeRev afterRev := by
  intro rem
  induction rem with
  | zero =>
      intro off _ _ _ _ hrem hoff
      omega
  | succ rem ih =>
      intro off beforeRev st final envId hrem hoff hok hconts hrel hrun hfinalOk
      cases hrun with
      | halted hhalt =>
          rcases hhalt with hbad | hnil
          · exact absurd hok hbad
          · rw [hconts] at hnil; cases hnil
      | @step _ s1 _ hstep hrest =>
          have hmax : ops.length ≤ Nat.max 1 ops.length := Nat.le_max_right 1 ops.length
          have hlt32 : entry + off < 2 ^ 32 := by
            unfold EntryRangeValid at hrange; omega
          have hidxNat : (BitVec.ofNat 32 (entry + off)).toNat = entry + off :=
            bv32_ofNat_toNat hlt32
          have hop : ops[off]? = some ops[off] := List.getElem?_eq_getElem hoff
          have hdrop : ops.drop off = ops[off] :: ops.drop (off + 1) :=
            List.drop_eq_getElem_cons hoff
          have htbl :
              lowerProgram entry ops (entry + off) =
                some ⟨if off + 1 < ops.length then some (entry + off + 1) else none,
                      lowerOp ops[off]⟩ := by
            cases ops with
            | nil => exact absurd rfl hne
            | cons _ _ =>
                have hle : entry ≤ entry + off := Nat.le_add_right entry off
                have hsub : entry + off - entry = off := Nat.add_sub_cancel_left entry off
                simp [lowerProgram, hle, hsub, hop]
          cases hstep with
          | tickQuota _ _ htick =>
              have hq := runtimeTick_zero_implies_quota hok htick
              have hneOk : s1.status ≠ FailureStatus.ok := by rw [hq]; simp
              cases hrest with
              | halted _ => exact absurd hfinalOk hneOk
              | step hs2 _ => exact absurd hs2 (dispatchStep_halted_absurd (Or.inl hneOk))
          | invokeMissing _ hc _ hnone =>
              rw [hconts] at hc; injection hc with hce _; subst hce
              rw [hidxNat, htbl] at hnone; cases hnone
          | invokeOk _ hc htick htbl' hex =>
              rw [hconts] at hc; injection hc with hce hcr; subst hce; subst hcr
              rw [hidxNat, htbl] at htbl'; injection htbl' with hblk; subst hblk
              have htkEq := runtimeTick_one_implies_ok hok htick
              subst htkEq
              by_cases hnext : off + 1 < ops.length
              · cases hex with
                | alreadyFailed hbad => exact absurd hok hbad
                | noNext _ hnone _ => simp [hnext] at hnone
                | withNextFailed _ _ _ hfail =>
                    cases hrest with
                    | halted _ => exact absurd hfinalOk hfail
                    | step hs2 _ => exact absurd hs2 (dispatchStep_halted_absurd (Or.inl hfail))
                | withNextOk _ hsome henq henqOk hbody =>
                    simp only [hnext, ↓reduceIte, Option.some.injEq] at hsome
                    subst hsome
                    have hdetEnq := doneConfig_inj
                      (wasmSteps_terminal_deterministic henq (enqueue_steps runtimeEnv rfl _ _ _))
                    subst hdetEnq
                    have hcontLim : (0 + 1) * 8 ≤ st.limits.continuationLimit := by
                      by_cases hgt : (0 + 1) * 8 > st.limits.continuationLimit
                      · rw [storeEnqueue_quota { st with conts := [], steps := st.steps + 1 }
                          _ _ hok hgt] at henqOk
                        cases henqOk
                      · exact Nat.not_lt.mp hgt
                    have hEnqStore :
                        storeEnqueue { st with conts := [], steps := st.steps + 1 }
                          (BitVec.ofNat 32 (entry + off + 1)) envId =
                        { st with
                          conts := [⟨BitVec.ofNat 32 (entry + off + 1), envId⟩]
                          steps := st.steps + 1
                          continuationPeak := Nat.max st.continuationPeak 8 } :=
                      storeEnqueue_ok { st with conts := [], steps := st.steps + 1 } _ _ hok hcontLim
                    rw [hEnqStore] at hbody
                    by_cases hs1Ok : s1.status = .ok
                    · have hconts1 : s1.conts = [⟨BitVec.ofNat 32 (entry + (off + 1)), envId⟩] := by
                        have hc' := lowerOp_preserves_conts ops[off] _ s1 hbody
                        simpa [Nat.add_assoc] using hc'
                      obtain ⟨midRev, hrelMid, hstepMc1⟩ :=
                        lowerOp_sound_top ops[off] beforeRev
                          { st with
                            conts := [⟨BitVec.ofNat 32 (entry + off + 1), envId⟩]
                            steps := st.steps + 1
                            continuationPeak := Nat.max st.continuationPeak 8 }
                          s1 hrel hok hbody hs1Ok
                      obtain ⟨afterRev, hrelAfter, hcontsFinal, hrunRest⟩ :=
                        ih (off + 1) midRev s1 final envId (by omega) hnext hs1Ok hconts1
                          hrelMid hrest hfinalOk
                      refine ⟨afterRev, hrelAfter, hcontsFinal, ?_⟩
                      rw [hdrop]
                      exact .cons hstepMc1 hrunRest
                    · cases hrest with
                      | halted _ => exact absurd hfinalOk hs1Ok
                      | step hs2 _ => exact absurd hs2 (dispatchStep_halted_absurd (Or.inl hs1Ok))
              · have hdropTail : ops.drop (off + 1) = [] := List.drop_eq_nil_of_le (by omega)
                cases hex with
                | alreadyFailed hbad => exact absurd hok hbad
                | withNextFailed _ hsome _ _ => simp [hnext] at hsome
                | withNextOk _ hsome _ _ _ => simp [hnext] at hsome
                | noNext _ _ hbody =>
                    have hconts1 : s1.conts = [] := lowerOp_preserves_conts ops[off] _ s1 hbody
                    cases hrest with
                    | halted _ =>
                        obtain ⟨afterRev, hrelAfter, hstepMc1⟩ :=
                          lowerOp_sound_top ops[off] beforeRev
                            { st with conts := [], steps := st.steps + 1 }
                            final hrel hok hbody hfinalOk
                        refine ⟨afterRev, hrelAfter, hconts1, ?_⟩
                        rw [hdrop, hdropTail]
                        exact .cons hstepMc1 .nil
                    | step hs2 _ =>
                        exact absurd hs2 (dispatchStep_halted_absurd (Or.inr hconts1))

/-- **Whole-Body Soundness (`r[WREF-STEP-01]`)**:
    If the lowered structured Wasm program for `ops` executes cleanly (`final.status = .ok`)
    from an `.ok`, empty-continuation store whose tagged operand slots refine `before`
    (including any arbitrary nonempty untouched tail), then `final.slots` refines an MC1
    stack `after` satisfying `NobleContracts.Exec (toMc1Code ops) before after`. -/
theorem runLowered_sound (entry : Nat) (ops : List ArithOp)
    (before : Stack) (st final : RuntimeStore)
    (hrange : EntryRangeValid entry ops)
    (hok : st.status = .ok)
    (hconts : st.conts = [])
    (hrel : StackRel before st.slots)
    (hrun : runLowered runtimeEnv entry ops st final)
    (hfinalOk : final.status = .ok) :
    ∃ after,
      StackRel after final.slots ∧
      final.conts = [] ∧
      NobleContracts.Exec (toMc1Code ops) before after := by
  rcases hrun with ⟨sEnq, henq, hdisp⟩
  have hdetEnq := doneConfig_inj
    (wasmSteps_terminal_deterministic henq (enqueue_steps runtimeEnv rfl _ _ st))
  subst hdetEnq
  by_cases hgt : (st.conts.length + 1) * 8 > st.limits.continuationLimit
  · rw [storeEnqueue_quota st _ _ hok hgt] at hdisp
    cases hdisp with
    | halted _ =>
        simp at hfinalOk
    | step hs _ =>
        exact absurd hs (dispatchStep_halted_absurd (Or.inl (by simp)))
  · have hclim : (st.conts.length + 1) * 8 ≤ st.limits.continuationLimit := Nat.not_lt.mp hgt
    have hEnqEq : storeEnqueue st (BitVec.ofNat 32 entry) 0 =
        { st with
          conts := [⟨BitVec.ofNat 32 entry, 0⟩]
          continuationPeak := Nat.max st.continuationPeak 8 } := by
      simpa [hconts] using storeEnqueue_ok st (BitVec.ofNat 32 entry) 0 hok hclim
    rw [hEnqEq] at hdisp
    let sEnqStore : RuntimeStore :=
      { st with
        conts := [⟨BitVec.ofNat 32 entry, 0⟩]
        continuationPeak := Nat.max st.continuationPeak 8 }
    have hsEnqOk : sEnqStore.status = .ok := hok
    cases ops with
    | nil =>
        have hmax : 1 ≤ Nat.max 1 0 := Nat.le_max_left 1 0
        have hlt32 : entry < 2 ^ 32 := by
          unfold EntryRangeValid at hrange; simp only [List.length_nil] at hrange; omega
        have hidxNat : (BitVec.ofNat 32 entry).toNat = entry := bv32_ofNat_toNat hlt32
        have htbl : lowerProgram entry [] entry = some ⟨none, []⟩ := by simp [lowerProgram]
        change DispatchRun runtimeEnv (lowerProgram entry []) sEnqStore final at hdisp
        generalize hs0 : sEnqStore = s0 at hdisp
        cases hdisp with
        | halted hhalt =>
            subst hs0
            rcases hhalt with hbad | hnil
            · exact absurd hsEnqOk hbad
            · cases hnil
        | @step _ s1 _ hstep hrest =>
            subst hs0
            cases hstep with
            | tickQuota _ _ htick =>
                have hq := runtimeTick_zero_implies_quota hsEnqOk htick
                have hneOk : s1.status ≠ FailureStatus.ok := by rw [hq]; simp
                cases hrest with
                | halted _ => exact absurd hfinalOk hneOk
                | step hs2 _ => exact absurd hs2 (dispatchStep_halted_absurd (Or.inl hneOk))
            | invokeMissing _ hc _ hnone =>
                injection hc with hce _; subst hce
                rw [hidxNat, htbl] at hnone; cases hnone
            | invokeOk _ hc htick htbl' hex =>
                injection hc with hce hcr; subst hce; subst hcr
                rw [hidxNat, htbl] at htbl'; injection htbl' with hblk; subst hblk
                have htkEq := runtimeTick_one_implies_ok hsEnqOk htick
                subst htkEq
                cases hex with
                | alreadyFailed hbad => exact absurd hsEnqOk hbad
                | withNextFailed _ hsome _ _ => cases hsome
                | withNextOk _ hsome _ _ _ => cases hsome
                | noNext _ _ hbody =>
                    have hbodyDet := doneConfig_inj (wasmSteps_terminal_deterministic hbody .refl)
                    subst hbodyDet
                    cases hrest with
                    | halted _ =>
                        exact ⟨before, hrel, rfl, by simp [toMc1Code]⟩
                    | step hs2 _ =>
                        exact absurd hs2 (dispatchStep_halted_absurd (Or.inr rfl))
    | cons op rest =>
        change DispatchRun runtimeEnv (lowerProgram entry (op :: rest)) sEnqStore final at hdisp
        have hcontsEnq : sEnqStore.conts = [⟨BitVec.ofNat 32 (entry + 0), 0⟩] := by
          simp [sEnqStore]
        obtain ⟨afterRev, hrelAfter, hcontsFinal, hrunMc1⟩ :=
          dispatch_suffix_sound entry (op :: rest) (List.cons_ne_nil op rest) hrange
            (op :: rest).length 0 before.reverse sEnqStore final 0
            (by simp) (by simp) hsEnqOk hcontsEnq hrel hdisp hfinalOk
        refine ⟨afterRev.reverse, ?_, hcontsFinal, ?_⟩
        · simpa [StackRel] using hrelAfter
        · simpa [Exec] using hrunMc1

/-- Helper: relate operand-stack peak budget across one MC1 `Step` of `op`. -/
private theorem peakSlots_step_bounds (op : ArithOp) (restOps : List ArithOp)
    (beforeRev midRev : Stack) (extraDepth limit : Nat)
    (hstep : NobleContracts.Step op.toMc1 beforeRev midRev)
    (hlim : peakSlots (op :: restOps) (beforeRev.length + extraDepth) * 16 ≤ limit) :
    (midRev.length + extraDepth) * 16 ≤ limit ∧
    peakSlots restOps (midRev.length + extraDepth) * 16 ≤ limit := by
  cases op with
  | i64 v =>
      cases hstep
      simp only [peakSlots, List.length_cons] at hlim ⊢
      have heq : beforeRev.length + 1 + extraDepth = beforeRev.length + extraDepth + 1 := by omega
      rw [heq]
      exact ⟨Nat.le_trans (Nat.mul_le_mul_right 16 (Nat.le_max_left _ _)) hlim,
        Nat.le_trans (Nat.mul_le_mul_right 16 (Nat.le_max_right _ _)) hlim⟩
  | add =>
      cases hstep
      rename_i a b s
      simp only [peakSlots, List.length_cons] at hlim ⊢
      have heq : s.length + 1 + extraDepth = (s.length + 1 + 1 + extraDepth) - 1 := by omega
      have hle : s.length + 1 + extraDepth ≤ s.length + 1 + 1 + extraDepth := by omega
      exact ⟨Nat.le_trans (Nat.mul_le_mul_right 16 hle)
          (Nat.le_trans (Nat.mul_le_mul_right 16 (Nat.le_max_left _ _)) hlim),
        by rw [heq]; exact Nat.le_trans (Nat.mul_le_mul_right 16 (Nat.le_max_right _ _)) hlim⟩
  | sub =>
      cases hstep
      rename_i a b s
      simp only [peakSlots, List.length_cons] at hlim ⊢
      have heq : s.length + 1 + extraDepth = (s.length + 1 + 1 + extraDepth) - 1 := by omega
      have hle : s.length + 1 + extraDepth ≤ s.length + 1 + 1 + extraDepth := by omega
      exact ⟨Nat.le_trans (Nat.mul_le_mul_right 16 hle)
          (Nat.le_trans (Nat.mul_le_mul_right 16 (Nat.le_max_left _ _)) hlim),
        by rw [heq]; exact Nat.le_trans (Nat.mul_le_mul_right 16 (Nat.le_max_right _ _)) hlim⟩
  | mul =>
      cases hstep
      rename_i a b s
      simp only [peakSlots, List.length_cons] at hlim ⊢
      have heq : s.length + 1 + extraDepth = (s.length + 1 + 1 + extraDepth) - 1 := by omega
      have hle : s.length + 1 + extraDepth ≤ s.length + 1 + 1 + extraDepth := by omega
      exact ⟨Nat.le_trans (Nat.mul_le_mul_right 16 hle)
          (Nat.le_trans (Nat.mul_le_mul_right 16 (Nat.le_max_left _ _)) hlim),
        by rw [heq]; exact Nat.le_trans (Nat.mul_le_mul_right 16 (Nat.le_max_right _ _)) hlim⟩

/-- Suffix completeness for `DispatchRun` starting at continuation offset `off` inside
    a nonempty body `ops`. -/
private theorem dispatch_suffix_complete
    (entry : Nat) (ops : List ArithOp) (hne : ops ≠ [])
    (hrange : EntryRangeValid entry ops) :
    ∀ (rem : Nat) (off : Nat) (beforeRev afterRev : Stack) (st : RuntimeStore) (envId : BitVec 32),
      rem = ops.length - off →
      off < ops.length →
      NobleContracts.Run (toMc1Code (ops.drop off)) beforeRev afterRev →
      st.status = .ok →
      st.conts = [⟨BitVec.ofNat 32 (entry + off), envId⟩] →
      StackRelTop beforeRev st.slots →
      peakSlots (ops.drop off) beforeRev.length * 16 ≤ st.limits.operandLimit →
      8 ≤ st.limits.continuationLimit →
      st.steps + (ops.length - off) ≤ st.limits.stepLimit →
      ∃ final,
        DispatchRun runtimeEnv (lowerProgram entry ops) st final ∧
        final.status = .ok ∧
        final.conts = [] ∧
        final.steps = st.steps + (ops.length - off) ∧
        StackRelTop afterRev final.slots := by
  intro rem
  induction rem with
  | zero =>
      intro off _ _ _ _ hrem hoff
      omega
  | succ rem ih =>
      intro off beforeRev afterRev st envId hrem hoff hrun hok hconts hrel hopLim hcontLim hstepLim
      have hmax : ops.length ≤ Nat.max 1 ops.length := Nat.le_max_right 1 ops.length
      have hlt32 : entry + off < 2 ^ 32 := by
        unfold EntryRangeValid at hrange; omega
      have hidxNat : (BitVec.ofNat 32 (entry + off)).toNat = entry + off :=
        bv32_ofNat_toNat hlt32
      have hop : ops[off]? = some ops[off] := List.getElem?_eq_getElem hoff
      have hdrop : ops.drop off = ops[off] :: ops.drop (off + 1) :=
        List.drop_eq_getElem_cons hoff
      have htbl :
          lowerProgram entry ops (entry + off) =
            some ⟨if off + 1 < ops.length then some (entry + off + 1) else none,
                  lowerOp ops[off]⟩ := by
        cases ops with
        | nil => exact absurd rfl hne
        | cons _ _ =>
            have hle : entry ≤ entry + off := Nat.le_add_right entry off
            have hsub : entry + off - entry = off := Nat.add_sub_cancel_left entry off
            simp [lowerProgram, hle, hsub, hop]
      rw [hdrop] at hrun hopLim
      cases hrun with
      | cons hstepMc1 hrunRest =>
          rename_i midRev
          have hmidLen := peakSlots_step_bounds ops[off] (ops.drop (off + 1)) beforeRev midRev 0
            st.limits.operandLimit hstepMc1 (by simpa using hopLim)
          have hmidLen1 : midRev.length * 16 ≤ st.limits.operandLimit := by simpa using hmidLen.1
          have hmidLen2 : peakSlots (ops.drop (off + 1)) midRev.length * 16 ≤
              st.limits.operandLimit := by simpa using hmidLen.2
          have hstepLt : st.steps < st.limits.stepLimit := by omega
          have htick :
              WasmSteps runtimeEnv
                ⟨[⟨[], [], [.call .runtimeTick], 1⟩], st⟩
                (doneValConfig (.i32 1) { st with steps := st.steps + 1 }) := by
            simpa [storeRuntimeTick_ok st hok hstepLt] using
              runtimeTick_steps runtimeEnv rfl st
          let sPop : RuntimeStore := { st with conts := [], steps := st.steps + 1 }
          by_cases hnext : off + 1 < ops.length
          · have henq :
                WasmSteps runtimeEnv
                  (initConfig
                    [.i32Const (BitVec.ofNat 32 (entry + off + 1)), .i32Const envId, .call .enqueue]
                    sPop)
                  (doneConfig
                    { sPop with
                      conts := [⟨BitVec.ofNat 32 (entry + off + 1), envId⟩]
                      continuationPeak := Nat.max sPop.continuationPeak 8 }) := by
              have hcl : (sPop.conts.length + 1) * 8 ≤ sPop.limits.continuationLimit := by
                simpa [sPop] using hcontLim
              simpa [storeEnqueue_ok sPop _ _ hok hcl, sPop] using
                enqueue_steps runtimeEnv rfl (BitVec.ofNat 32 (entry + off + 1)) envId sPop
            let sEnq : RuntimeStore :=
              { sPop with
                conts := [⟨BitVec.ofNat 32 (entry + off + 1), envId⟩]
                continuationPeak := Nat.max sPop.continuationPeak 8 }
            obtain ⟨s1, hbody, hs1Ok, hrelMid, hconts1, hsteps1, hlim1, _⟩ :=
              lowerOp_complete_top ops[off] beforeRev midRev sEnq hstepMc1 hrel hok hmidLen1
            have hex :
                ExecContBlock runtimeEnv
                  ⟨if off + 1 < ops.length then some (entry + off + 1) else none, lowerOp ops[off]⟩
                  envId sPop s1 :=
              .withNextOk hok (by simp [hnext]) henq hok hbody
            have hdispStep : DispatchStep runtimeEnv (lowerProgram entry ops) st s1 :=
              .invokeOk hok hconts htick (by rw [hidxNat]; exact htbl) hex
            have hconts1' : s1.conts = [⟨BitVec.ofNat 32 (entry + (off + 1)), envId⟩] := by
              simpa [sEnq, Nat.add_assoc] using hconts1
            have hsteps1' : s1.steps = st.steps + 1 := by simpa [sEnq, sPop] using hsteps1
            have hlim1' : s1.limits = st.limits := by simpa [sEnq, sPop] using hlim1
            obtain ⟨final, hdispRest, hfinalOk, hfinalConts, hfinalSteps, hrelFinal⟩ :=
              ih (off + 1) midRev afterRev s1 envId (by omega) hnext hrunRest hs1Ok hconts1'
                hrelMid (by rw [hlim1']; exact hmidLen2) (by rw [hlim1']; exact hcontLim)
                (by rw [hlim1', hsteps1']; omega)
            refine ⟨final, .step hdispStep hdispRest, hfinalOk, hfinalConts, ?_, hrelFinal⟩
            rw [hfinalSteps, hsteps1']; omega
          · have hdropTail : ops.drop (off + 1) = [] := List.drop_eq_nil_of_le (by omega)
            rw [hdropTail] at hrunRest
            cases hrunRest
            obtain ⟨s1, hbody, hs1Ok, hrelMid, hconts1, hsteps1, _, _⟩ :=
              lowerOp_complete_top ops[off] beforeRev afterRev sPop hstepMc1 hrel hok hmidLen1
            have hex :
                ExecContBlock runtimeEnv
                  ⟨if off + 1 < ops.length then some (entry + off + 1) else none, lowerOp ops[off]⟩
                  envId sPop s1 :=
              .noNext hok (by simp [hnext]) hbody
            have hdispStep : DispatchStep runtimeEnv (lowerProgram entry ops) st s1 :=
              .invokeOk hok hconts htick (by rw [hidxNat]; exact htbl) hex
            have hconts1' : s1.conts = [] := by simpa [sPop] using hconts1
            have hsteps1' : s1.steps = st.steps + (ops.length - off) := by
              have : ops.length - off = 1 := by omega
              simpa [sPop, this] using hsteps1
            exact ⟨s1, .step hdispStep (.halted (Or.inr hconts1')),
              hs1Ok, hconts1', hsteps1', hrelMid⟩

/-- **Whole-Body Completeness / Existence (`r[WREF-STEP-01]`)**:
    For every MC1 execution `Exec (toMc1Code ops) before after` and every `.ok` store
    with empty continuation queue, matching tagged slots, and sufficient operand,
    continuation, and step quotas, `runLowered` terminates cleanly in `final` with
    `StackRel after final.slots` and exact step count `st.steps + neededSteps ops`. -/
theorem runLowered_complete (entry : Nat) (ops : List ArithOp)
    (before after : Stack) (st : RuntimeStore)
    (hrange : EntryRangeValid entry ops)
    (hexec : NobleContracts.Exec (toMc1Code ops) before after)
    (hok : st.status = .ok)
    (hconts : st.conts = [])
    (hrel : StackRel before st.slots)
    (hopLim : peakSlots ops before.length * 16 ≤ st.limits.operandLimit)
    (hcontLim : 8 ≤ st.limits.continuationLimit)
    (hstepLim : st.steps + neededSteps ops ≤ st.limits.stepLimit) :
    ∃ final,
      runLowered runtimeEnv entry ops st final ∧
      final.status = .ok ∧
      final.conts = [] ∧
      final.steps = st.steps + neededSteps ops ∧
      StackRel after final.slots := by
  have hcl : (st.conts.length + 1) * 8 ≤ st.limits.continuationLimit := by
    simpa [hconts] using hcontLim
  have henqEq : storeEnqueue st (BitVec.ofNat 32 entry) 0 =
      { st with
        conts := [⟨BitVec.ofNat 32 entry, 0⟩]
        continuationPeak := Nat.max st.continuationPeak 8 } := by
    simpa [hconts] using storeEnqueue_ok st (BitVec.ofNat 32 entry) 0 hok hcl
  have henq :
      WasmSteps runtimeEnv
        (initConfig [.i32Const (BitVec.ofNat 32 entry), .i32Const 0, .call .enqueue] st)
        (doneConfig
          { st with
            conts := [⟨BitVec.ofNat 32 entry, 0⟩]
            continuationPeak := Nat.max st.continuationPeak 8 }) := by
    rw [← henqEq]
    exact enqueue_steps runtimeEnv rfl (BitVec.ofNat 32 entry) 0 st
  let sEnq : RuntimeStore :=
    { st with
      conts := [⟨BitVec.ofNat 32 entry, 0⟩]
      continuationPeak := Nat.max st.continuationPeak 8 }
  cases ops with
  | nil =>
      simp only [toMc1Code, List.map_nil, exec_nil_iff] at hexec
      subst hexec
      have hmax : 1 ≤ Nat.max 1 0 := Nat.le_max_left 1 0
      have hlt32 : entry < 2 ^ 32 := by
        unfold EntryRangeValid at hrange; simp only [List.length_nil] at hrange; omega
      have hidxNat : (BitVec.ofNat 32 entry).toNat = entry := bv32_ofNat_toNat hlt32
      have htbl : lowerProgram entry [] entry = some ⟨none, []⟩ := by simp [lowerProgram]
      have hneed : neededSteps [] = 1 := rfl
      have hstepLt : sEnq.steps < sEnq.limits.stepLimit := by
        rw [hneed] at hstepLim
        simpa [sEnq] using (by omega : st.steps < st.limits.stepLimit)
      have htick :
          WasmSteps runtimeEnv
            ⟨[⟨[], [], [.call .runtimeTick], 1⟩], sEnq⟩
            (doneValConfig (.i32 1) { sEnq with steps := sEnq.steps + 1 }) := by
        simpa [storeRuntimeTick_ok sEnq hok hstepLt] using
          runtimeTick_steps runtimeEnv rfl sEnq
      let sFinal : RuntimeStore := { sEnq with conts := [], steps := sEnq.steps + 1 }
      have hex : ExecContBlock runtimeEnv ⟨none, []⟩ 0 sFinal sFinal :=
        .noNext hok rfl .refl
      have hdispStep : DispatchStep runtimeEnv (lowerProgram entry []) sEnq sFinal :=
        .invokeOk hok rfl htick (by rw [hidxNat]; exact htbl) hex
      refine ⟨sFinal, ⟨sEnq, henq, .step hdispStep (.halted (Or.inr rfl))⟩,
        hok, rfl, ?_, hrel⟩
      simp [sFinal, sEnq, hneed]
  | cons op rest =>
      have hcontsEnq : sEnq.conts = [⟨BitVec.ofNat 32 (entry + 0), 0⟩] := by simp [sEnq]
      have hopLim' : peakSlots ((op :: rest).drop 0) before.reverse.length * 16 ≤
          sEnq.limits.operandLimit := by
        simpa [sEnq] using hopLim
      have hneed : neededSteps (op :: rest) = rest.length + 1 := by
        unfold neededSteps
        simp only [List.length_cons]
        exact Nat.max_eq_right (Nat.succ_le_succ (Nat.zero_le _))
      have hstepLim' : sEnq.steps + ((op :: rest).length - 0) ≤ sEnq.limits.stepLimit := by
        rw [hneed] at hstepLim
        simpa [sEnq] using hstepLim
      obtain ⟨final, hdisp, hfok, hfconts, hfsteps, hfrel⟩ :=
        dispatch_suffix_complete entry (op :: rest) (List.cons_ne_nil op rest) hrange
          (op :: rest).length 0 before.reverse after.reverse sEnq 0
          (by simp) (by simp) hexec hok hcontsEnq hrel hopLim' hcontLim hstepLim'
      refine ⟨final, ⟨sEnq, henq, hdisp⟩, hfok, hfconts, ?_, hfrel⟩
      rw [hneed]
      simpa [sEnq] using hfsteps

/-! ## 9. Exact Tail Preservation Under Untouched Incoming Suffixes -/

/-- Frame lemma for `ArithOp` single-step execution: executing `lowerOp op` on
    `topSlots ++ tailSlots` leaves `tailSlots` bit-for-bit identical. -/
theorem lowerOp_frame_complete (op : ArithOp) (inpRev outRev : Stack)
    (topSlots tailSlots : List TaggedSlot) (st : RuntimeStore)
    (hstep : NobleContracts.Step op.toMc1 inpRev outRev)
    (htop : StackRelTop inpRev topSlots)
    (hsl : st.slots = topSlots ++ tailSlots)
    (hok : st.status = .ok)
    (hlim : (outRev.length + tailSlots.length) * 16 ≤ st.limits.operandLimit) :
    ∃ outSlots final,
      WasmSteps runtimeEnv (initConfig (lowerOp op) st) (doneConfig final) ∧
      final.status = .ok ∧
      StackRelTop outRev outSlots ∧
      final.slots = outSlots ++ tailSlots ∧
      final.conts = st.conts ∧
      final.steps = st.steps ∧
      final.limits = st.limits ∧
      final.phase = st.phase := by
  cases op with
  | i64 v =>
      cases hstep
      have hlen : (st.slots.length + 1) * 16 ≤ st.limits.operandLimit := by
        rw [hsl, List.length_append, ← stackRelTop_length htop]
        simpa [Nat.add_assoc, Nat.add_comm, Nat.add_left_comm] using hlim
      refine ⟨⟨tagI64, v⟩ :: topSlots, _, pushI64_steps_ok v st hok hlen,
        hok, .cons rfl htop, by simp [hsl], rfl, rfl, rfl, rfl⟩
  | add =>
      cases hstep with
      | add =>
          obtain ⟨sb, sa, remSlots, htopEq, hvr, hvl, hrem⟩ := stackRelTop_cons2_inv htop
          rename_i a b s
          simp only [SlotRel] at hvr hvl
          have hlen : ((remSlots ++ tailSlots).length + 1) * 16 ≤ st.limits.operandLimit := by
            rw [List.length_append, ← stackRelTop_length hrem]
            simpa [Nat.add_assoc, Nat.add_comm, Nat.add_left_comm] using hlim
          have hsl' : st.slots = ⟨tagI64, b⟩ :: ⟨tagI64, a⟩ :: (remSlots ++ tailSlots) := by
            simp [hsl, htopEq, hvr, hvl]
          refine ⟨⟨tagI64, a + b⟩ :: remSlots, _,
            opAdd_steps_ok st a b (remSlots ++ tailSlots) hok hsl' hlen,
            hok, .cons rfl hrem, rfl, rfl, rfl, rfl, rfl⟩
  | sub =>
      cases hstep with
      | sub =>
          obtain ⟨sb, sa, remSlots, htopEq, hvr, hvl, hrem⟩ := stackRelTop_cons2_inv htop
          rename_i a b s
          simp only [SlotRel] at hvr hvl
          have hlen : ((remSlots ++ tailSlots).length + 1) * 16 ≤ st.limits.operandLimit := by
            rw [List.length_append, ← stackRelTop_length hrem]
            simpa [Nat.add_assoc, Nat.add_comm, Nat.add_left_comm] using hlim
          have hsl' : st.slots = ⟨tagI64, b⟩ :: ⟨tagI64, a⟩ :: (remSlots ++ tailSlots) := by
            simp [hsl, htopEq, hvr, hvl]
          refine ⟨⟨tagI64, a - b⟩ :: remSlots, _,
            opSub_steps_ok st a b (remSlots ++ tailSlots) hok hsl' hlen,
            hok, .cons rfl hrem, rfl, rfl, rfl, rfl, rfl⟩
  | mul =>
      cases hstep with
      | mul =>
          obtain ⟨sb, sa, remSlots, htopEq, hvr, hvl, hrem⟩ := stackRelTop_cons2_inv htop
          rename_i a b s
          simp only [SlotRel] at hvr hvl
          have hlen : ((remSlots ++ tailSlots).length + 1) * 16 ≤ st.limits.operandLimit := by
            rw [List.length_append, ← stackRelTop_length hrem]
            simpa [Nat.add_assoc, Nat.add_comm, Nat.add_left_comm] using hlim
          have hsl' : st.slots = ⟨tagI64, b⟩ :: ⟨tagI64, a⟩ :: (remSlots ++ tailSlots) := by
            simp [hsl, htopEq, hvr, hvl]
          refine ⟨⟨tagI64, a * b⟩ :: remSlots, _,
            opMul_steps_ok st a b (remSlots ++ tailSlots) hok hsl' hlen,
            hok, .cons rfl hrem, rfl, rfl, rfl, rfl, rfl⟩

/-- Suffix frame completeness: running a lowered body on `topSlots ++ tailSlots`
    leaves `tailSlots` literally identical (even when `tailSlots` holds arbitrary
    non-I64 values or opaque tags). -/
private theorem dispatch_suffix_frame_complete
    (entry : Nat) (ops : List ArithOp) (hne : ops ≠ [])
    (hrange : EntryRangeValid entry ops) (tailSlots : List TaggedSlot) :
    ∀ (rem : Nat) (off : Nat) (inpRev outRev : Stack) (topSlots : List TaggedSlot)
      (st : RuntimeStore) (envId : BitVec 32),
      rem = ops.length - off →
      off < ops.length →
      NobleContracts.Run (toMc1Code (ops.drop off)) inpRev outRev →
      st.status = .ok →
      st.conts = [⟨BitVec.ofNat 32 (entry + off), envId⟩] →
      StackRelTop inpRev topSlots →
      st.slots = topSlots ++ tailSlots →
      peakSlots (ops.drop off) (inpRev.length + tailSlots.length) * 16 ≤ st.limits.operandLimit →
      8 ≤ st.limits.continuationLimit →
      st.steps + (ops.length - off) ≤ st.limits.stepLimit →
      ∃ outSlots final,
        DispatchRun runtimeEnv (lowerProgram entry ops) st final ∧
        final.status = .ok ∧
        final.conts = [] ∧
        final.steps = st.steps + (ops.length - off) ∧
        StackRelTop outRev outSlots ∧
        final.slots = outSlots ++ tailSlots := by
  intro rem
  induction rem with
  | zero =>
      intro off _ _ _ _ _ hrem hoff
      omega
  | succ rem ih =>
      intro off inpRev outRev topSlots st envId hrem hoff hrun hok hconts htop hsl
        hopLim hcontLim hstepLim
      have hmax : ops.length ≤ Nat.max 1 ops.length := Nat.le_max_right 1 ops.length
      have hlt32 : entry + off < 2 ^ 32 := by
        unfold EntryRangeValid at hrange; omega
      have hidxNat : (BitVec.ofNat 32 (entry + off)).toNat = entry + off :=
        bv32_ofNat_toNat hlt32
      have hop : ops[off]? = some ops[off] := List.getElem?_eq_getElem hoff
      have hdrop : ops.drop off = ops[off] :: ops.drop (off + 1) :=
        List.drop_eq_getElem_cons hoff
      have htbl :
          lowerProgram entry ops (entry + off) =
            some ⟨if off + 1 < ops.length then some (entry + off + 1) else none,
                  lowerOp ops[off]⟩ := by
        cases ops with
        | nil => exact absurd rfl hne
        | cons _ _ =>
            have hle : entry ≤ entry + off := Nat.le_add_right entry off
            have hsub : entry + off - entry = off := Nat.add_sub_cancel_left entry off
            simp [lowerProgram, hle, hsub, hop]
      rw [hdrop] at hrun hopLim
      cases hrun with
      | cons hstepMc1 hrunRest =>
          rename_i midRev
          have hmidLen := peakSlots_step_bounds ops[off] (ops.drop (off + 1)) inpRev midRev
            tailSlots.length st.limits.operandLimit hstepMc1 hopLim
          have hstepLt : st.steps < st.limits.stepLimit := by omega
          have htick :
              WasmSteps runtimeEnv
                ⟨[⟨[], [], [.call .runtimeTick], 1⟩], st⟩
                (doneValConfig (.i32 1) { st with steps := st.steps + 1 }) := by
            simpa [storeRuntimeTick_ok st hok hstepLt] using
              runtimeTick_steps runtimeEnv rfl st
          let sPop : RuntimeStore := { st with conts := [], steps := st.steps + 1 }
          by_cases hnext : off + 1 < ops.length
          · have henq :
                WasmSteps runtimeEnv
                  (initConfig
                    [.i32Const (BitVec.ofNat 32 (entry + off + 1)), .i32Const envId, .call .enqueue]
                    sPop)
                  (doneConfig
                    { sPop with
                      conts := [⟨BitVec.ofNat 32 (entry + off + 1), envId⟩]
                      continuationPeak := Nat.max sPop.continuationPeak 8 }) := by
              have hcl : (sPop.conts.length + 1) * 8 ≤ sPop.limits.continuationLimit := by
                simpa [sPop] using hcontLim
              simpa [storeEnqueue_ok sPop _ _ hok hcl, sPop] using
                enqueue_steps runtimeEnv rfl (BitVec.ofNat 32 (entry + off + 1)) envId sPop
            let sEnq : RuntimeStore :=
              { sPop with
                conts := [⟨BitVec.ofNat 32 (entry + off + 1), envId⟩]
                continuationPeak := Nat.max sPop.continuationPeak 8 }
            obtain ⟨midSlots, s1, hbody, hs1Ok, hrelMid, hsl1, hconts1, hsteps1, hlim1, _⟩ :=
              lowerOp_frame_complete ops[off] inpRev midRev topSlots tailSlots sEnq
                hstepMc1 htop hsl hok hmidLen.1
            have hex :
                ExecContBlock runtimeEnv
                  ⟨if off + 1 < ops.length then some (entry + off + 1) else none, lowerOp ops[off]⟩
                  envId sPop s1 :=
              .withNextOk hok (by simp [hnext]) henq hok hbody
            have hdispStep : DispatchStep runtimeEnv (lowerProgram entry ops) st s1 :=
              .invokeOk hok hconts htick (by rw [hidxNat]; exact htbl) hex
            have hconts1' : s1.conts = [⟨BitVec.ofNat 32 (entry + (off + 1)), envId⟩] := by
              simpa [sEnq, Nat.add_assoc] using hconts1
            have hsteps1' : s1.steps = st.steps + 1 := by simpa [sEnq, sPop] using hsteps1
            have hlim1' : s1.limits = st.limits := by simpa [sEnq, sPop] using hlim1
            obtain ⟨outSlots, final, hdispRest, hfinalOk, hfinalConts, hfinalSteps,
              hrelFinal, hslFinal⟩ :=
              ih (off + 1) midRev outRev midSlots s1 envId (by omega) hnext hrunRest
                hs1Ok hconts1' hrelMid hsl1 (by rw [hlim1']; exact hmidLen.2)
                (by rw [hlim1']; exact hcontLim) (by rw [hlim1', hsteps1']; omega)
            refine ⟨outSlots, final, .step hdispStep hdispRest, hfinalOk, hfinalConts, ?_,
              hrelFinal, hslFinal⟩
            rw [hfinalSteps, hsteps1']; omega
          · have hdropTail : ops.drop (off + 1) = [] := List.drop_eq_nil_of_le (by omega)
            rw [hdropTail] at hrunRest
            cases hrunRest
            obtain ⟨midSlots, s1, hbody, hs1Ok, hrelMid, hsl1, hconts1, hsteps1, _, _⟩ :=
              lowerOp_frame_complete ops[off] inpRev outRev topSlots tailSlots sPop
                hstepMc1 htop hsl hok hmidLen.1
            have hex :
                ExecContBlock runtimeEnv
                  ⟨if off + 1 < ops.length then some (entry + off + 1) else none, lowerOp ops[off]⟩
                  envId sPop s1 :=
              .noNext hok (by simp [hnext]) hbody
            have hdispStep : DispatchStep runtimeEnv (lowerProgram entry ops) st s1 :=
              .invokeOk hok hconts htick (by rw [hidxNat]; exact htbl) hex
            have hconts1' : s1.conts = [] := by simpa [sPop] using hconts1
            have hsteps1' : s1.steps = st.steps + (ops.length - off) := by
              have : ops.length - off = 1 := by omega
              simpa [sPop, this] using hsteps1
            exact ⟨midSlots, s1, .step hdispStep (.halted (Or.inr hconts1')),
              hs1Ok, hconts1', hsteps1', hrelMid, hsl1⟩

/-- **Preserved-Tail Frame Simulation (`r[WREF-STEP-01]`)**:
    Given `Exec (toMc1Code ops) inp out` on the quotation's own inputs and ANY
    untouched incoming tail `tailSlots` below `inpSlots`, the lowered Wasm body
    produces `outSlots ++ tailSlots` with `tailSlots` bit-for-bit unchanged. -/
theorem runLowered_frame_complete (entry : Nat) (ops : List ArithOp)
    (inp out : Stack) (inpSlots tailSlots : List TaggedSlot) (st : RuntimeStore)
    (hrange : EntryRangeValid entry ops)
    (hexec : NobleContracts.Exec (toMc1Code ops) inp out)
    (hok : st.status = .ok)
    (hconts : st.conts = [])
    (hinp : StackRel inp inpSlots)
    (hsl : st.slots = inpSlots ++ tailSlots)
    (hopLim : peakSlots ops (inp.length + tailSlots.length) * 16 ≤ st.limits.operandLimit)
    (hcontLim : 8 ≤ st.limits.continuationLimit)
    (hstepLim : st.steps + neededSteps ops ≤ st.limits.stepLimit) :
    ∃ outSlots final,
      runLowered runtimeEnv entry ops st final ∧
      final.status = .ok ∧
      final.conts = [] ∧
      StackRel out outSlots ∧
      final.slots = outSlots ++ tailSlots := by
  have hcl : (st.conts.length + 1) * 8 ≤ st.limits.continuationLimit := by
    simpa [hconts] using hcontLim
  have henqEq : storeEnqueue st (BitVec.ofNat 32 entry) 0 =
      { st with
        conts := [⟨BitVec.ofNat 32 entry, 0⟩]
        continuationPeak := Nat.max st.continuationPeak 8 } := by
    simpa [hconts] using storeEnqueue_ok st (BitVec.ofNat 32 entry) 0 hok hcl
  have henq :
      WasmSteps runtimeEnv
        (initConfig [.i32Const (BitVec.ofNat 32 entry), .i32Const 0, .call .enqueue] st)
        (doneConfig
          { st with
            conts := [⟨BitVec.ofNat 32 entry, 0⟩]
            continuationPeak := Nat.max st.continuationPeak 8 }) := by
    rw [← henqEq]
    exact enqueue_steps runtimeEnv rfl (BitVec.ofNat 32 entry) 0 st
  let sEnq : RuntimeStore :=
    { st with
      conts := [⟨BitVec.ofNat 32 entry, 0⟩]
      continuationPeak := Nat.max st.continuationPeak 8 }
  cases ops with
  | nil =>
      simp only [toMc1Code, List.map_nil, exec_nil_iff] at hexec
      subst hexec
      have hmax : 1 ≤ Nat.max 1 0 := Nat.le_max_left 1 0
      have hlt32 : entry < 2 ^ 32 := by
        unfold EntryRangeValid at hrange; simp only [List.length_nil] at hrange; omega
      have hidxNat : (BitVec.ofNat 32 entry).toNat = entry := bv32_ofNat_toNat hlt32
      have htbl : lowerProgram entry [] entry = some ⟨none, []⟩ := by simp [lowerProgram]
      have hneed : neededSteps [] = 1 := rfl
      have hstepLt : sEnq.steps < sEnq.limits.stepLimit := by
        rw [hneed] at hstepLim
        simpa [sEnq] using (by omega : st.steps < st.limits.stepLimit)
      have htick :
          WasmSteps runtimeEnv
            ⟨[⟨[], [], [.call .runtimeTick], 1⟩], sEnq⟩
            (doneValConfig (.i32 1) { sEnq with steps := sEnq.steps + 1 }) := by
        simpa [storeRuntimeTick_ok sEnq hok hstepLt] using
          runtimeTick_steps runtimeEnv rfl sEnq
      let sFinal : RuntimeStore := { sEnq with conts := [], steps := sEnq.steps + 1 }
      have hex : ExecContBlock runtimeEnv ⟨none, []⟩ 0 sFinal sFinal :=
        .noNext hok rfl .refl
      have hdispStep : DispatchStep runtimeEnv (lowerProgram entry []) sEnq sFinal :=
        .invokeOk hok rfl htick (by rw [hidxNat]; exact htbl) hex
      exact ⟨inpSlots, sFinal, ⟨sEnq, henq, .step hdispStep (.halted (Or.inr rfl))⟩,
        hok, rfl, hinp, hsl⟩
  | cons op rest =>
      have hcontsEnq : sEnq.conts = [⟨BitVec.ofNat 32 (entry + 0), 0⟩] := by simp [sEnq]
      have hopLim' : peakSlots ((op :: rest).drop 0) (inp.reverse.length + tailSlots.length) * 16 ≤
          sEnq.limits.operandLimit := by
        simpa [sEnq] using hopLim
      have hneed : neededSteps (op :: rest) = rest.length + 1 := by
        unfold neededSteps
        simp only [List.length_cons]
        exact Nat.max_eq_right (Nat.succ_le_succ (Nat.zero_le _))
      have hstepLim' : sEnq.steps + ((op :: rest).length - 0) ≤ sEnq.limits.stepLimit := by
        rw [hneed] at hstepLim
        simpa [sEnq] using hstepLim
      obtain ⟨outSlots, final, hdisp, hfok, hfconts, _, hfrel, hfslots⟩ :=
        dispatch_suffix_frame_complete entry (op :: rest) (List.cons_ne_nil op rest) hrange
          tailSlots (op :: rest).length 0 inp.reverse out.reverse inpSlots sEnq 0
          (by simp) (by simp) hexec hok hcontsEnq hinp hsl hopLim' hcontLim hstepLim'
      exact ⟨outSlots, final, ⟨sEnq, henq, hdisp⟩, hfok, hfconts, hfrel, hfslots⟩

/-! ## 10. Quota Exhaustion vs Runtime Fault vs Source Semantics Separation -/

/-- Default valid execution-phase runtime store helper for witnesses and controls. -/
def mkStore (slots : List TaggedSlot) (limits : ProfileLimits) : RuntimeStore :=
  { slots := slots
    conts := []
    limits := limits
    phase := execPhase
    status := .ok
    steps := 0
    operandPeak := slots.length * 16
    continuationPeak := 0 }

/-- Standard valid profile limits matching `common.wat:51-63`. -/
def defaultLimits : ProfileLimits :=
  { operandLimit := 2048, continuationLimit := 4096, stepLimit := 100000 }

theorem defaultLimits_valid : ProfileLimitsValid defaultLimits :=
  ⟨by decide, by decide, by decide⟩

/-- **Step-quota refusal does not imply source divergence**:
    With `stepLimit = 0`, `runLowered` halts with `.quota execPhase .steps`
    (`$failure = 2`, `$quota_reason = 6`) on ANY program, even when the exact same
    source program satisfies `NobleContracts.Exec (toMc1Code ops) before after`. -/
theorem step_quota_not_divergence (entry : Nat) (ops : List ArithOp)
    (before after : Stack) (slots : List TaggedSlot)
    (hexec : NobleContracts.Exec (toMc1Code ops) before after) :
    let tight : ProfileLimits := { operandLimit := 2048, continuationLimit := 4096, stepLimit := 0 }
    ∃ finalTight,
      runLowered runtimeEnv entry ops (mkStore slots tight) finalTight ∧
      finalTight.status = .quota execPhase .steps ∧
      finalTight.status.failureGlobal = 2 ∧
      finalTight.status.quotaGlobal = 6 ∧
      NobleContracts.Exec (toMc1Code ops) before after := by
  intro tight
  let st := mkStore slots tight
  have henqEq : storeEnqueue st (BitVec.ofNat 32 entry) 0 =
      { st with conts := [⟨BitVec.ofNat 32 entry, 0⟩], continuationPeak := 8 } := by
    have hcl : (st.conts.length + 1) * 8 ≤ st.limits.continuationLimit := by
      simp [st, mkStore, tight]
    simpa [st, mkStore] using storeEnqueue_ok st (BitVec.ofNat 32 entry) 0 rfl hcl
  have henq :
      WasmSteps runtimeEnv
        (initConfig [.i32Const (BitVec.ofNat 32 entry), .i32Const 0, .call .enqueue] st)
        (doneConfig { st with conts := [⟨BitVec.ofNat 32 entry, 0⟩], continuationPeak := 8 }) := by
    rw [← henqEq]
    exact enqueue_steps runtimeEnv rfl (BitVec.ofNat 32 entry) 0 st
  let sEnq : RuntimeStore :=
    { st with conts := [⟨BitVec.ofNat 32 entry, 0⟩], continuationPeak := 8 }
  have htickEq : storeRuntimeTick sEnq = (0, { sEnq with status := .quota execPhase .steps }) := by
    have hge : sEnq.steps ≥ sEnq.limits.stepLimit := by simp [sEnq, st, mkStore, tight]
    simpa [sEnq, st, mkStore] using storeRuntimeTick_quota sEnq rfl hge
  have htick :
      WasmSteps runtimeEnv
        ⟨[⟨[], [], [.call .runtimeTick], 1⟩], sEnq⟩
        (doneValConfig (.i32 0) { sEnq with status := .quota execPhase .steps }) := by
    have h := runtimeTick_steps runtimeEnv rfl sEnq
    rw [htickEq] at h
    exact h
  let sQuota : RuntimeStore := { sEnq with status := .quota execPhase .steps }
  have hdisp : DispatchRun runtimeEnv (lowerProgram entry ops) sEnq sQuota :=
    .step (.tickQuota rfl rfl htick) (.halted (Or.inl (by simp)))
  exact ⟨sQuota, ⟨sEnq, henq, hdisp⟩, rfl, rfl, rfl, hexec⟩

/-- **Continuation-quota refusal does not imply source divergence**:
    With `continuationLimit = 0`, enqueueing the root entry immediately halts with
    `.quota execPhase .continuation` (`$failure = 2`, `$quota_reason = 5`), distinct
    from both a runtime fault (`$failure = 4`) and source divergence. -/
theorem continuation_quota_not_divergence (entry : Nat) (ops : List ArithOp)
    (before after : Stack) (slots : List TaggedSlot)
    (hexec : NobleContracts.Exec (toMc1Code ops) before after) :
    let tight : ProfileLimits := { operandLimit := 2048, continuationLimit := 0, stepLimit := 100000 }
    ∃ finalTight,
      runLowered runtimeEnv entry ops (mkStore slots tight) finalTight ∧
      finalTight.status = .quota execPhase .continuation ∧
      finalTight.status.failureGlobal = 2 ∧
      finalTight.status.quotaGlobal = 5 ∧
      NobleContracts.Exec (toMc1Code ops) before after := by
  intro tight
  let st := mkStore slots tight
  have hgt : (st.conts.length + 1) * 8 > st.limits.continuationLimit := by
    simp [st, mkStore, tight]
  have henqEq : storeEnqueue st (BitVec.ofNat 32 entry) 0 =
      { st with status := .quota execPhase .continuation } := by
    simpa [st, mkStore] using storeEnqueue_quota st (BitVec.ofNat 32 entry) 0 rfl hgt
  have henq :
      WasmSteps runtimeEnv
        (initConfig [.i32Const (BitVec.ofNat 32 entry), .i32Const 0, .call .enqueue] st)
        (doneConfig { st with status := .quota execPhase .continuation }) := by
    rw [← henqEq]
    exact enqueue_steps runtimeEnv rfl (BitVec.ofNat 32 entry) 0 st
  let sQuota : RuntimeStore := { st with status := .quota execPhase .continuation }
  exact ⟨sQuota, ⟨sQuota, henq, .halted (Or.inl (by simp))⟩, rfl, rfl, rfl, hexec⟩

/-- **Operand-quota refusal on literal push is distinct from fault and source divergence**:
    `[ 7 ]` terminates in MC1 (`Exec [.lit (.i64 7)] [] [.i64 7]`), yet with
    `operandLimit = 0` it halts with `.quota execPhase .operand` (`$failure = 2`, `$quota_reason = 4`). -/
theorem operand_quota_not_divergence :
    let tight : ProfileLimits := { operandLimit := 0, continuationLimit := 4096, stepLimit := 100000 }
    ∃ finalTight,
      runLowered runtimeEnv 0 [.i64 7] (mkStore [] tight) finalTight ∧
      finalTight.status = .quota execPhase .operand ∧
      finalTight.status.failureGlobal = 2 ∧
      finalTight.status.quotaGlobal = 4 ∧
      NobleContracts.Exec (toMc1Code [.i64 7]) [] [.i64 7] := by
  intro tight
  let st := mkStore [] tight
  have henq :
      WasmSteps runtimeEnv
        (initConfig [.i32Const 0, .i32Const 0, .call .enqueue] st)
        (doneConfig { st with conts := [⟨0, 0⟩], continuationPeak := 8 }) := by
    simpa [st, mkStore, tight, storeEnqueue_ok] using
      enqueue_steps runtimeEnv rfl 0 0 st
  let sEnq : RuntimeStore := { st with conts := [⟨0, 0⟩], continuationPeak := 8 }
  have htick :
      WasmSteps runtimeEnv
        ⟨[⟨[], [], [.call .runtimeTick], 1⟩], sEnq⟩
        (doneValConfig (.i32 1) { sEnq with steps := 1 }) := by
    simpa [sEnq, st, mkStore, tight, storeRuntimeTick_ok] using
      runtimeTick_steps runtimeEnv rfl sEnq
  let sPop : RuntimeStore := { sEnq with conts := [], steps := 1 }
  have hpush :
      WasmSteps runtimeEnv
        (initConfig [.i64Const 7, .call .pushI64] sPop)
        (doneConfig { sPop with status := .quota execPhase .operand }) :=
    pushI64_steps_quota 7 sPop rfl (by decide)
  let sQuota : RuntimeStore := { sPop with status := .quota execPhase .operand }
  have htbl : lowerProgram 0 [.i64 7] 0 = some ⟨none, lowerOp (.i64 7)⟩ := by
    simp [lowerProgram]
  have hdisp : DispatchRun runtimeEnv (lowerProgram 0 [.i64 7]) sEnq sQuota :=
    .step (.invokeOk rfl rfl htick htbl (.noNext rfl rfl hpush))
      (.halted (Or.inl (by decide)))
  exact ⟨sQuota, ⟨sEnq, henq, hdisp⟩, rfl, by decide, by decide, by simp [toMc1Code, ArithOp.toMc1]⟩

/-- **Runtime fault (underflow) vs quota**:
    Running `[ - ]` on a 1-element stack `[.i64 5]` with generous quotas faults with
    `.fault 4` (`$failure = 4`, `$quota_reason = 0`), and MC1 has no transition. -/
theorem underflow_fault_distinct_from_quota :
    ∃ final,
      runLowered runtimeEnv 0 [.sub] (mkStore [⟨tagI64, 5⟩] defaultLimits) final ∧
      final.status = .fault faultCode ∧
      final.status.failureGlobal = 4 ∧
      final.status.quotaGlobal = 0 ∧
      (∀ t, ¬ NobleContracts.Exec (toMc1Code [.sub]) [.i64 5] t) := by
  let st := mkStore [⟨tagI64, 5⟩] defaultLimits
  have henq :
      WasmSteps runtimeEnv
        (initConfig [.i32Const 0, .i32Const 0, .call .enqueue] st)
        (doneConfig { st with conts := [⟨0, 0⟩], continuationPeak := 8 }) := by
    simpa [st, mkStore, defaultLimits, storeEnqueue_ok] using
      enqueue_steps runtimeEnv rfl 0 0 st
  let sEnq : RuntimeStore := { st with conts := [⟨0, 0⟩], continuationPeak := 8 }
  have htick :
      WasmSteps runtimeEnv
        ⟨[⟨[], [], [.call .runtimeTick], 1⟩], sEnq⟩
        (doneValConfig (.i32 1) { sEnq with steps := 1 }) := by
    simpa [sEnq, st, mkStore, defaultLimits, storeRuntimeTick_ok] using
      runtimeTick_steps runtimeEnv rfl sEnq
  let sPop : RuntimeStore := { sEnq with conts := [], steps := 1 }
  have hsub :
      WasmSteps runtimeEnv
        (initConfig [.call .opSub] sPop)
        (doneConfig { sPop with slots := [], status := .fault faultCode }) :=
    opSub_steps_underflow_one sPop 5 rfl rfl
  let sFault : RuntimeStore := { sPop with slots := [], status := .fault faultCode }
  have htbl : lowerProgram 0 [.sub] 0 = some ⟨none, lowerOp .sub⟩ := by
    simp [lowerProgram]
  have hdisp : DispatchRun runtimeEnv (lowerProgram 0 [.sub]) sEnq sFault :=
    .step (.invokeOk rfl rfl htick htbl (.noNext rfl rfl hsub))
      (.halted (Or.inl (by decide)))
  refine ⟨sFault, ⟨sEnq, henq, hdisp⟩, rfl, by decide, by decide, ?_⟩
  intro t hex
  cases hex with
  | cons hs _ => cases hs

/-- **Runtime fault (operand tag mismatch) vs quota**:
    Running `[ - ]` on `[.i64 5, .bool true]` faults with `.fault 4` (`$failure = 4`,
    `$quota_reason = 0`), and MC1 has no transition. -/
theorem tag_mismatch_fault_distinct_from_quota :
    ∃ final,
      runLowered runtimeEnv 0 [.sub]
        (mkStore [⟨tagBool, 1⟩, ⟨tagI64, 5⟩] defaultLimits) final ∧
      final.status = .fault faultCode ∧
      final.status.failureGlobal = 4 ∧
      final.status.quotaGlobal = 0 ∧
      (∀ t, ¬ NobleContracts.Exec (toMc1Code [.sub]) [.i64 5, .bool true] t) := by
  let st := mkStore [⟨tagBool, 1⟩, ⟨tagI64, 5⟩] defaultLimits
  have henq :
      WasmSteps runtimeEnv
        (initConfig [.i32Const 0, .i32Const 0, .call .enqueue] st)
        (doneConfig { st with conts := [⟨0, 0⟩], continuationPeak := 8 }) := by
    simpa [st, mkStore, defaultLimits, storeEnqueue_ok] using
      enqueue_steps runtimeEnv rfl 0 0 st
  let sEnq : RuntimeStore := { st with conts := [⟨0, 0⟩], continuationPeak := 8 }
  have htick :
      WasmSteps runtimeEnv
        ⟨[⟨[], [], [.call .runtimeTick], 1⟩], sEnq⟩
        (doneValConfig (.i32 1) { sEnq with steps := 1 }) := by
    simpa [sEnq, st, mkStore, defaultLimits, storeRuntimeTick_ok] using
      runtimeTick_steps runtimeEnv rfl sEnq
  let sPop : RuntimeStore := { sEnq with conts := [], steps := 1 }
  have hsub :
      WasmSteps runtimeEnv
        (initConfig [.call .opSub] sPop)
        (doneConfig { sPop with status := .fault faultCode }) :=
    opSub_steps_tag_mismatch sPop ⟨tagBool, 1⟩ [⟨tagI64, 5⟩] rfl rfl (by decide)
  let sFault : RuntimeStore := { sPop with status := .fault faultCode }
  have htbl : lowerProgram 0 [.sub] 0 = some ⟨none, lowerOp .sub⟩ := by
    simp [lowerProgram]
  have hdisp : DispatchRun runtimeEnv (lowerProgram 0 [.sub]) sEnq sFault :=
    .step (.invokeOk rfl rfl htick htbl (.noNext rfl rfl hsub))
      (.halted (Or.inl (by decide)))
  refine ⟨sFault, ⟨sEnq, henq, hdisp⟩, rfl, by decide, by decide, ?_⟩
  intro t hex
  cases hex with
  | cons hs _ => cases hs

/-! ## 11. Non-Vacuity Witnesses: Ordered Arithmetic, Preserved Tail, and 64-Bit Wraparound -/

/-- **Non-vacuity witness (Scenario `r[WREF-STEP-01]`)**:
    Ordered subtraction `[ - ]` on inputs `[.i64 10, .i64 3]` above a nonempty mixed
    tail `[.bool true, .i64 99]` yields `10 - 3 = 7` with the tail untouched in both
    the Wasm small-step semantics and MC1 `Exec`. -/
theorem ordered_sub_with_tail_witness :
    let tail : Stack := [.bool true, .i64 99]
    let tailSlots : List TaggedSlot := [⟨tagI64, 99⟩, ⟨tagBool, 1⟩]
    let inpSlots : List TaggedSlot := [⟨tagI64, 3⟩, ⟨tagI64, 10⟩]
    ∃ final,
      runLowered runtimeEnv 0 [.sub] (mkStore (inpSlots ++ tailSlots) defaultLimits) final ∧
      final.status = .ok ∧
      final.slots = ⟨tagI64, 7⟩ :: tailSlots ∧
      NobleContracts.Exec (toMc1Code [.sub]) (tail ++ [.i64 10, .i64 3]) (tail ++ [.i64 7]) := by
  intro tail tailSlots inpSlots
  have hsubEq : (10 : BitVec 64) - 3 = 7 := by decide
  have hexec : NobleContracts.Exec (toMc1Code [.sub]) [.i64 10, .i64 3] [.i64 7] := by
    change NobleContracts.Exec [.word 5] ([] ++ [.i64 10, .i64 3]) ([] ++ [.i64 7])
    rw [exec_sub_iff, hsubEq]
  have hrelInp : StackRel [.i64 10, .i64 3] inpSlots :=
    .cons rfl (.cons rfl .nil)
  obtain ⟨outSlots, final, hrun, hok, _, hrelOut, hslots⟩ :=
    runLowered_frame_complete 0 [.sub] [.i64 10, .i64 3] [.i64 7] inpSlots tailSlots
      (mkStore (inpSlots ++ tailSlots) defaultLimits)
      (by unfold EntryRangeValid; decide) hexec rfl rfl hrelInp rfl
      (by decide) (by decide) (by decide)
  obtain ⟨slot, rfl, hv⟩ := stackRel_singleton_iff.mp hrelOut
  simp only [SlotRel] at hv
  subst hv
  have hexecTail :
      NobleContracts.Exec (toMc1Code [.sub]) (tail ++ [.i64 10, .i64 3]) (tail ++ [.i64 7]) := by
    change NobleContracts.Exec [.word 5] (tail ++ [.i64 10, .i64 3]) (tail ++ [.i64 7])
    rw [exec_sub_iff, hsubEq]
  exact ⟨final, hrun, hok, hslots, hexecTail⟩

/-- **64-bit wraparound witness (`sub`)**:
    `0 - 1 = 2^64 - 1` (`BitVec.ofInt 64 (-1)`) matches between `runLowered` and MC1 `Exec`. -/
theorem wraparound_sub_witness :
    let zero : BitVec 64 := 0
    let one : BitVec 64 := 1
    let allOnes : BitVec 64 := BitVec.ofInt 64 (-1)
    ∃ final,
      runLowered runtimeEnv 0 [.i64 zero, .i64 one, .sub] (mkStore [] defaultLimits) final ∧
      final.status = .ok ∧
      final.slots = [⟨tagI64, allOnes⟩] ∧
      NobleContracts.Exec (toMc1Code [.i64 zero, .i64 one, .sub]) [] [.i64 allOnes] := by
  intro zero one allOnes
  have hsubEq : zero - one = allOnes := by decide
  have hexec : NobleContracts.Exec (toMc1Code [.i64 zero, .i64 one, .sub]) [] [.i64 allOnes] := by
    simp [toMc1Code, ArithOp.toMc1, Exec, hsubEq]
  obtain ⟨final, hrun, hok, _, _, hrel⟩ :=
    runLowered_complete 0 [.i64 zero, .i64 one, .sub] [] [.i64 allOnes]
      (mkStore [] defaultLimits) (by unfold EntryRangeValid; decide) hexec rfl rfl .nil
      (by decide) (by decide) (by decide)
  obtain ⟨slot, hslots, hv⟩ := stackRel_singleton_iff.mp hrel
  simp only [SlotRel] at hv
  subst hv
  exact ⟨final, hrun, hok, hslots, hexec⟩

/-- **64-bit wraparound witness (`add`, `mul`)**:
    `(2^63 - 1) + 1 = 2^63` and `2^63 * 2 = 0` match between `runLowered` and MC1 `Exec`. -/
theorem wraparound_add_mul_witness :
    let i64Max : BitVec 64 := BitVec.ofNat 64 (2 ^ 63 - 1)
    let prog : List ArithOp := [.i64 i64Max, .i64 1, .add, .i64 2, .mul]
    ∃ final,
      runLowered runtimeEnv 0 prog (mkStore [] defaultLimits) final ∧
      final.status = .ok ∧
      final.slots = [⟨tagI64, 0⟩] ∧
      NobleContracts.Exec (toMc1Code prog) [] [.i64 0] := by
  intro i64Max prog
  have hexec : NobleContracts.Exec (toMc1Code prog) [] [.i64 0] := by
    simp [prog, toMc1Code, ArithOp.toMc1, Exec]
    decide
  obtain ⟨final, hrun, hok, _, _, hrel⟩ :=
    runLowered_complete 0 prog [] [.i64 0]
      (mkStore [] defaultLimits) (by unfold EntryRangeValid prog; decide) hexec rfl rfl .nil
      (by decide) (by decide) (by decide)
  obtain ⟨slot, hslots, hv⟩ := stackRel_singleton_iff.mp hrel
  simp only [SlotRel] at hv
  subst hv
  exact ⟨final, hrun, hok, hslots, hexec⟩

/-! ## 12. Rejection-Sensitivity Controls (`r[WREF-STEP-01]`, `r[WREF-EMIT-01]`) -/

/-- General soundness property parameterized by a helper environment `env` and
    a lowered program `ops`: every clean Wasm run must simulate `Exec (toMc1Code ops)`. -/
def SoundFor (env : FuncEnv) (entry : Nat) (ops : List ArithOp) : Prop :=
  ∀ (before : Stack) (st final : RuntimeStore),
    st.status = .ok →
    st.conts = [] →
    StackRel before st.slots →
    runLowered env entry ops st final →
    final.status = .ok →
    ∃ after, StackRel after final.slots ∧ NobleContracts.Exec (toMc1Code ops) before after

/-- Production `runtimeEnv` satisfies `SoundFor` on every valid entry range. -/
theorem runtimeEnv_soundFor (entry : Nat) (ops : List ArithOp)
    (hrange : EntryRangeValid entry ops) :
    SoundFor runtimeEnv entry ops := by
  intro before st final hok hconts hrel hrun hfok
  obtain ⟨after, hrel', _, hexec⟩ :=
    runLowered_sound entry ops before st final hrange hok hconts hrel hrun hfok
  exact ⟨after, hrel', hexec⟩

/-- **Mutated helper (swapped subtraction operand order)**:
    `defOpSubSwapped` pops `$right` (local 0) and `$left` (local 1) in the same order
    but computes `i64.sub $right $left` instead of `i64.sub $left $right`. -/
def defOpSubSwapped : FuncDef :=
  { paramCount := 0
    localDefaults := [.i64 0, .i64 0]
    body :=
      [ .i32Const tagI64, .call .popKind, .localSet 0
      , .i32Const tagI64, .call .popKind, .localSet 1
      , .localGet 0, .localGet 1, .i64Sub, .call .pushI64 ]
    retArity := 0 }

def runtimeEnvSwappedSub : FuncEnv
  | .opSub => some defOpSubSwapped
  | f => runtimeEnv f

/-- Exact small-step execution of the mutated `$op_sub` helper computes `right - left`. -/
theorem opSubSwapped_steps_ok (st : RuntimeStore) (left right : BitVec 64)
    (tail : List TaggedSlot) (hok : st.status = .ok)
    (hsl : st.slots = ⟨tagI64, right⟩ :: ⟨tagI64, left⟩ :: tail)
    (hlim : (tail.length + 1) * 16 ≤ st.limits.operandLimit) :
    WasmSteps runtimeEnvSwappedSub
      (initConfig [.call .opSub] st)
      (doneConfig
        { st with
          slots := ⟨tagI64, right - left⟩ :: tail
          operandPeak := Nat.max st.operandPeak ((tail.length + 1) * 16) }) := by
  unfold initConfig doneConfig
  have hcall :
      WasmStep runtimeEnvSwappedSub
        ⟨[⟨[], [], [.call .opSub], 0⟩], st⟩
        ⟨[⟨[.i64 0, .i64 0], [], defOpSubSwapped.body, 0⟩, ⟨[], [], [], 0⟩], st⟩ := by
    simpa [defOpSubSwapped] using
      (@WasmStep.callDefined runtimeEnvSwappedSub [] [] [] [] [] 0 [] st .opSub defOpSubSwapped
        rfl rfl rfl)
  refine .step hcall ?_
  have hp1 : storePopKind st tagI64 = (right, { st with slots := ⟨tagI64, left⟩ :: tail }) :=
    storePopKind_ok st tagI64 right (⟨tagI64, left⟩ :: tail) hok hsl
  have hp2 : storePopKind { st with slots := ⟨tagI64, left⟩ :: tail } tagI64 =
      (left, { st with slots := tail }) :=
    storePopKind_ok { st with slots := ⟨tagI64, left⟩ :: tail } tagI64 left tail hok rfl
  have hpush : storePushTag { st with slots := tail } tagI64 (right - left) =
      { st with
        slots := ⟨tagI64, right - left⟩ :: tail
        operandPeak := Nat.max st.operandPeak ((tail.length + 1) * 16) } :=
    storePushTag_ok { st with slots := tail } tagI64 (right - left) hok hlim
  refine .step .i32Const ?_
  refine .step (by simpa [hp1] using
    (@WasmStep.callPopKind runtimeEnvSwappedSub _ _ _ _ _ st tagI64 rfl)) ?_
  refine .step (.localSet (by simp)) ?_
  refine .step .i32Const ?_
  refine .step (by simpa [hp2] using
    (@WasmStep.callPopKind runtimeEnvSwappedSub _ _ _ _ _ { st with slots := ⟨tagI64, left⟩ :: tail }
      tagI64 rfl)) ?_
  refine .step (.localSet (by simp)) ?_
  refine .step (.localGet (by rfl)) ?_
  refine .step (.localGet (by rfl)) ?_
  refine .step .i64Sub ?_
  have hpushCall :
      WasmStep runtimeEnvSwappedSub
        ⟨⟨[.i64 right, .i64 left], [.i64 (right - left)], [.call .pushI64], 0⟩ ::
          [⟨[], [], [], 0⟩], { st with slots := tail }⟩
        ⟨⟨[.i64 (right - left)], [], defPushI64.body, 0⟩ ::
          ⟨[.i64 right, .i64 left], [], [], 0⟩ :: [⟨[], [], [], 0⟩],
          { st with slots := tail }⟩ := by
    simpa [defPushI64] using
      (@WasmStep.callDefined runtimeEnvSwappedSub [.i64 right, .i64 left]
        [.i64 (right - left)] [.i64 (right - left)] [] [] 0 [⟨[], [], [], 0⟩]
        { st with slots := tail } .pushI64 defPushI64 rfl rfl rfl)
  refine .step hpushCall ?_
  refine .step .i32Const ?_
  refine .step (.localGet (by rfl)) ?_
  refine .step (by simpa [hpush] using
    (@WasmStep.callPushTag runtimeEnvSwappedSub _ _ _ _ _ { st with slots := tail }
      tagI64 (right - left) rfl)) ?_
  refine .step (@WasmStep.returnEnd runtimeEnvSwappedSub _ [] [] [] 0 _ _ _ _ _ _ rfl rfl) ?_
  exact .single (@WasmStep.returnEnd runtimeEnvSwappedSub _ [] [] [] 0 _ _ _ _ _ _ rfl rfl)

/-- **Rejection Sensitivity #1 — Swapped subtraction order fails simulation**:
    `runtimeEnvSwappedSub` produces `3 - 10` on `[.i64 10, .i64 3]`, contradicting
    MC1 `Exec [.word 5]` (`10 - 3 = 7`), so `¬ SoundFor runtimeEnvSwappedSub 0 [.sub]`. -/
theorem swapped_opSub_fails_simulation :
    ¬ SoundFor runtimeEnvSwappedSub 0 [.sub] := by
  intro hsound
  let st := mkStore [⟨tagI64, 3⟩, ⟨tagI64, 10⟩] defaultLimits
  have henq :
      WasmSteps runtimeEnvSwappedSub
        (initConfig [.i32Const 0, .i32Const 0, .call .enqueue] st)
        (doneConfig { st with conts := [⟨0, 0⟩], continuationPeak := 8 }) := by
    simpa [st, mkStore, defaultLimits, storeEnqueue_ok] using
      enqueue_steps runtimeEnvSwappedSub rfl 0 0 st
  let sEnq : RuntimeStore := { st with conts := [⟨0, 0⟩], continuationPeak := 8 }
  have htick :
      WasmSteps runtimeEnvSwappedSub
        ⟨[⟨[], [], [.call .runtimeTick], 1⟩], sEnq⟩
        (doneValConfig (.i32 1) { sEnq with steps := 1 }) := by
    simpa [sEnq, st, mkStore, defaultLimits, storeRuntimeTick_ok] using
      runtimeTick_steps runtimeEnvSwappedSub rfl sEnq
  let sPop : RuntimeStore := { sEnq with conts := [], steps := 1 }
  have hsub := opSubSwapped_steps_ok sPop 10 3 [] rfl rfl (by decide)
  let sFinal : RuntimeStore :=
    { sPop with slots := [⟨tagI64, (3 : BitVec 64) - 10⟩],
                operandPeak := Nat.max sPop.operandPeak 16 }
  have htbl : lowerProgram 0 [.sub] 0 = some ⟨none, lowerOp .sub⟩ := by
    simp [lowerProgram]
  have hrun : runLowered runtimeEnvSwappedSub 0 [.sub] st sFinal :=
    ⟨sEnq, henq, .step (.invokeOk rfl rfl htick htbl (.noNext rfl rfl hsub))
      (.halted (Or.inr rfl))⟩
  have hrel : StackRel [.i64 10, .i64 3] st.slots := .cons rfl (.cons rfl .nil)
  obtain ⟨after, hrelAfter, hexec⟩ := hsound [.i64 10, .i64 3] st sFinal rfl rfl hrel hrun rfl
  have ha : after = [.i64 ((10 : BitVec 64) - 3)] := by
    have h' : NobleContracts.Exec [.word 5] ([] ++ [.i64 10, .i64 3]) after := hexec
    simpa using (exec_sub_iff.mp h')
  subst ha
  obtain ⟨slot, hslots, hv⟩ := stackRel_singleton_iff.mp hrelAfter
  simp only [sFinal, List.cons.injEq, and_true] at hslots
  subst hslots
  simp only [SlotRel] at hv
  have hval : (3 : BitVec 64) - 10 = (10 : BitVec 64) - 3 := by
    injection hv
  exact absurd hval (by decide)

/-- **Rejection Sensitivity #2 — Substituted body below the same interface**:
    Running the lowered `[ + ]` body does not satisfy the correspondence claim for
    `[ * ]` on `[.i64 2, .i64 3]` (`2 + 3 = 5 ≠ 6 = 2 * 3`). -/
theorem substituted_body_fails_correspondence :
    ¬ (∀ (before : Stack) (st final : RuntimeStore),
        st.status = .ok → st.conts = [] → StackRel before st.slots →
        runLowered runtimeEnv 0 [.add] st final → final.status = .ok →
        ∃ after, StackRel after final.slots ∧
          NobleContracts.Exec (toMc1Code [.mul]) before after) := by
  intro hclaim
  let st := mkStore [⟨tagI64, 3⟩, ⟨tagI64, 2⟩] defaultLimits
  have haddEq : (2 : BitVec 64) + 3 = 5 := by decide
  have hexecAdd : NobleContracts.Exec (toMc1Code [.add]) [.i64 2, .i64 3] [.i64 5] := by
    change NobleContracts.Exec [.word 4] ([] ++ [.i64 2, .i64 3]) ([] ++ [.i64 5])
    rw [exec_add_iff, haddEq]
  have hrel : StackRel [.i64 2, .i64 3] st.slots := .cons rfl (.cons rfl .nil)
  obtain ⟨final, hrun, hfok, _, _, hrelAdd⟩ :=
    runLowered_complete 0 [.add] [.i64 2, .i64 3] [.i64 5] st
      (by unfold EntryRangeValid; decide) hexecAdd rfl rfl hrel
      (by decide) (by decide) (by decide)
  obtain ⟨after, hrelMul, hexecMul⟩ := hclaim [.i64 2, .i64 3] st final rfl rfl hrel hrun hfok
  have ha : after = [.i64 6] := by
    have h' : NobleContracts.Exec [.word 6] ([] ++ [.i64 2, .i64 3]) after := hexecMul
    simpa [show (2 : BitVec 64) * 3 = 6 by decide] using (exec_mul_iff.mp h')
  subst ha
  obtain ⟨s5, hsl5, hv5⟩ := stackRel_singleton_iff.mp hrelAdd
  obtain ⟨s6, hsl6, hv6⟩ := stackRel_singleton_iff.mp hrelMul
  rw [hsl5] at hsl6
  injection hsl6 with hsEq
  subst hsEq
  simp only [SlotRel] at hv5 hv6
  rw [hv5] at hv6
  injection hv6 with _ h56
  exact absurd h56 (by decide)

/-- Subject-bound correspondence premise tying one exact source body, its reflected
    MC1 recipe, its lowered continuation table, and soundness against `Exec`. -/
structure SubjectCorrespondence
    (env : FuncEnv) (entry : Nat) (body : List ArithOp)
    (recipe : List Op) (tbl : ProgramTable) : Prop where
  recipeExact : recipe = toMc1Code body
  tableExact : tbl = lowerProgram entry body
  sound : SoundFor env entry body

/-- Two extensionally indistinguishable quotations: `[ 1 1 + ]` and `[ 2 ]`. -/
def progOnePlusOne : List ArithOp := [.i64 1, .i64 1, .add]
def progTwo : List ArithOp := [.i64 2]

/-- **Rejection Sensitivity #3 — Indistinguishable ordinary value with different ordered
    body and reflected recipe (`r[WREF-EMIT-01]`)**:
    1. `[ 1 1 + ]` and `[ 2 ]` produce the exact same ordinary I64 stack value `[.i64 2]`
       under both `runLowered` and MC1 `Exec`.
    2. Their reflected MC1 recipes are distinct (`Exec [.block r1, .word 11]` cannot equal
       `Exec [.block r2, .word 11]`), and their step budgets differ (`3` vs `1`).
    3. Therefore, substituting `progTwo`'s recipe or lowered table beneath `progOnePlusOne`
       violates `SubjectCorrespondence`. -/
theorem same_value_distinct_recipe_sensitivity :
    (∃ f1 f2,
      runLowered runtimeEnv 0 progOnePlusOne (mkStore [] defaultLimits) f1 ∧
      runLowered runtimeEnv 0 progTwo (mkStore [] defaultLimits) f2 ∧
      f1.status = .ok ∧ f2.status = .ok ∧
      f1.slots = [⟨tagI64, 2⟩] ∧ f2.slots = [⟨tagI64, 2⟩] ∧
      NobleContracts.Exec (toMc1Code progOnePlusOne) [] [.i64 2] ∧
      NobleContracts.Exec (toMc1Code progTwo) [] [.i64 2]) ∧
    (toMc1Code progOnePlusOne ≠ toMc1Code progTwo) ∧
    (∀ s t,
      NobleContracts.Exec [.block (toMc1Code progOnePlusOne), .word 11] s t →
      ¬ NobleContracts.Exec [.block (toMc1Code progTwo), .word 11] s t) ∧
    (neededSteps progOnePlusOne = 3 ∧ neededSteps progTwo = 1) ∧
    (¬ SubjectCorrespondence runtimeEnv 0 progOnePlusOne
        (toMc1Code progTwo) (lowerProgram 0 progOnePlusOne)) ∧
    (¬ SubjectCorrespondence runtimeEnv 0 progOnePlusOne
        (toMc1Code progOnePlusOne) (lowerProgram 0 progTwo)) := by
  have hex1 : NobleContracts.Exec (toMc1Code progOnePlusOne) [] [.i64 2] := by
    simp [progOnePlusOne, toMc1Code, ArithOp.toMc1, Exec]
  have hex2 : NobleContracts.Exec (toMc1Code progTwo) [] [.i64 2] := by
    simp [progTwo, toMc1Code, ArithOp.toMc1, Exec]
  obtain ⟨f1, hrun1, hok1, _, _, hrel1⟩ :=
    runLowered_complete 0 progOnePlusOne [] [.i64 2] (mkStore [] defaultLimits)
      (by unfold EntryRangeValid progOnePlusOne; decide) hex1 rfl rfl .nil
      (by decide) (by decide) (by decide)
  obtain ⟨f2, hrun2, hok2, _, _, hrel2⟩ :=
    runLowered_complete 0 progTwo [] [.i64 2] (mkStore [] defaultLimits)
      (by unfold EntryRangeValid progTwo; decide) hex2 rfl rfl .nil
      (by decide) (by decide) (by decide)
  obtain ⟨s1, hf1Slots, hv1⟩ := stackRel_singleton_iff.mp hrel1
  simp only [SlotRel] at hv1; subst hv1
  obtain ⟨s2, hf2Slots, hv2⟩ := stackRel_singleton_iff.mp hrel2
  simp only [SlotRel] at hv2; subst hv2
  refine ⟨⟨f1, f2, hrun1, hrun2, hok1, hok2, hf1Slots, hf2Slots, hex1, hex2⟩, ?_, ?_, ?_, ?_, ?_⟩
  · intro h; simp [progOnePlusOne, progTwo, toMc1Code, ArithOp.toMc1] at h
  · intro s t h1 h2
    simp only [Exec, Run.cons_iff, Run.nil_iff, Step.block_iff, Step.reflect_iff,
      exists_eq_left] at h1 h2
    rw [h1] at h2
    simp [progOnePlusOne, progTwo, toMc1Code, ArithOp.toMc1] at h2
  · exact ⟨by decide, by decide⟩
  · intro hcorr
    have h := hcorr.recipeExact
    simp [progOnePlusOne, progTwo, toMc1Code, ArithOp.toMc1] at h
  · intro hcorr
    have h := congrFun hcorr.tableExact 0
    simp [lowerProgram, progOnePlusOne, progTwo, lowerOp] at h

/-! ## 13. Discharging `LinearMemorySlotPremise` and End-to-End `[0, 8192)` Lifting -/

theorem readSlotsTopFirst_length (mem : LinearMem) (n : Nat) :
    (readSlotsTopFirst mem n).length = n := by
  induction n with
  | zero => rfl
  | succ k ih => simp [readSlotsTopFirst, ih]

theorem readContsTopFirst_length (mem : LinearMem) (n : Nat) :
    (readContsTopFirst mem n).length = n := by
  induction n with
  | zero => rfl
  | succ k ih => simp [readContsTopFirst, ih]

theorem readSlotsTopFirst_memEqOn {m1 m2 : LinearMem} {lo hi n : Nat}
    (h : MemEqOn lo hi m1 m2) (hlo : lo = 0) (hhi : n * 16 ≤ hi) :
    readSlotsTopFirst m1 n = readSlotsTopFirst m2 n := by
  subst hlo
  induction n with
  | zero => rfl
  | succ k ih =>
      simp only [readSlotsTopFirst, List.cons.injEq]
      refine ⟨?_, ih (by omega)⟩
      unfold readSlot
      have h1 : loadI32 m1 (k * 16) = loadI32 m2 (k * 16) :=
        loadI32_memEqOn h (by omega) (by omega)
      have h2 : loadI64 m1 (k * 16 + 8) = loadI64 m2 (k * 16 + 8) :=
        loadI64_memEqOn h (by omega) (by omega)
      rw [h1, h2]

theorem readContsTopFirst_memEqOn {m1 m2 : LinearMem} {lo hi n : Nat}
    (h : MemEqOn lo hi m1 m2) (hlo : lo ≤ 4096) (hhi : 4096 + n * 8 ≤ hi) :
    readContsTopFirst m1 n = readContsTopFirst m2 n := by
  induction n with
  | zero => rfl
  | succ k ih =>
      simp only [readContsTopFirst, List.cons.injEq]
      refine ⟨?_, ih (by omega)⟩
      unfold readCont
      have h1 : loadI32 m1 (4096 + k * 8) = loadI32 m2 (4096 + k * 8) :=
        loadI32_memEqOn h (by omega) (by omega)
      have h2 : loadI32 m1 (4096 + k * 8 + 4) = loadI32 m2 (4096 + k * 8 + 4) :=
        loadI32_memEqOn h (by omega) (by omega)
      rw [h1, h2]

private theorem quotaReason_ofCodeD_code (r : QuotaReason) :
    QuotaReason.ofCodeD r.code = r := by
  cases r <;> decide

private theorem quotaReason_code_ne_zero {r : QuotaReason} (hr : r ≠ .none) :
    r.code ≠ 0 := by
  cases r with
  | none => exact absurd rfl hr
  | allocation => decide
  | recipe => decide
  | depth => decide
  | operand => decide
  | continuation => decide
  | steps => decide

theorem absStore_of_storeRefines {cst : LinearRuntimeState} {st : RuntimeStore}
    (h : StoreRefines cst st) : absStore cst = st := by
  rcases st with ⟨slots, conts, ⟨ol, cl, sl⟩, phase, status, steps, opPeak, contPeak⟩
  have hlim := h.limitsEq
  dsimp only at hlim
  rcases hlim with ⟨hol, hcl, hsl⟩
  have hst : (if cst.failure = 0 then FailureStatus.ok
      else if cst.quotaReason ≠ 0 then FailureStatus.quota (cst.failure - 1) (QuotaReason.ofCodeD cst.quotaReason)
      else FailureStatus.fault cst.failure) = status := by
    have hsr := h.statusRef
    cases status with
    | ok =>
        rcases hsr with ⟨hf, _⟩
        simp [hf]
    | quota qph r =>
        rcases hsr with ⟨hf, hnz, hqr, hr⟩
        have hfnz : cst.failure ≠ 0 := by rw [hf]; exact hnz
        have hqrnz : cst.quotaReason ≠ 0 := by
          rw [hqr]; exact quotaReason_code_ne_zero hr
        have hsub : cst.failure - 1 = qph := by
          apply BitVec.eq_of_toNat_eq
          rw [hf]
          have h1 : (1 : BitVec 32).toNat = 1 := rfl
          have hq : qph.toNat < 2 ^ 32 := qph.isLt
          have hnz' : (1 + qph).toNat ≠ 0 := fun hz =>
            hnz (BitVec.eq_of_toNat_eq (by simpa using hz))
          rw [BitVec.toNat_add, h1] at hnz'
          have hlt : 1 + qph.toNat < 2 ^ 32 := by omega
          have hle : (1 : BitVec 32) ≤ 1 + qph := by
            rw [BitVec.le_def, h1, BitVec.toNat_add, h1]
            omega
          rw [BitVec.toNat_sub_of_le hle, BitVec.toNat_add, h1]
          omega
        have hrc : r.code ≠ 0 := quotaReason_code_ne_zero hr
        simp only [hfnz, ↓reduceIte, hqr, hsub, quotaReason_ofCodeD_code]
        split
        · rfl
        · contradiction
    | fault c =>
        rcases hsr with ⟨hf, hcnz, hqr⟩
        simp only [hf, hcnz, ↓reduceIte, hqr, ne_eq, not_true_eq_false]
  unfold absStore
  dsimp only
  rw [h.slotsEq, h.contsEq, hol, hcl, hsl, h.phaseEq, hst, h.stepsEq, h.operandPeakEq,
    h.continuationPeakEq]

private theorem storeRefines_failure_eq_zero_iff {cst : LinearRuntimeState} {st : RuntimeStore}
    (h : StoreRefines cst st) : cst.failure = 0 ↔ st.status = .ok := by
  have hsr := h.statusRef
  cases hs : st.status with
  | ok =>
      rw [hs] at hsr
      exact ⟨fun _ => rfl, fun _ => hsr.1⟩
  | quota qph r =>
      rw [hs] at hsr
      constructor
      · intro hf; exact absurd (hf ▸ hsr.1.symm) hsr.2.1
      · intro hok; cases hok
  | fault c =>
      rw [hs] at hsr
      constructor
      · intro hf; exact absurd (hf ▸ hsr.1.symm) hsr.2.1
      · intro hok; cases hok

theorem linear_fail_refines {cst : LinearRuntimeState} {st : RuntimeStore} (code : BitVec 32)
    (h : StoreRefines cst st) (hcode : code ≠ 0) :
    StoreRefines (concreteFail cst code) (st.setFault code) := by
  by_cases hf : cst.failure = 0
  · have hok : st.status = .ok := (storeRefines_failure_eq_zero_iff h).mp hf
    have hqz : cst.quotaReason = 0 := h.failureZeroQuotaZero hf
    rw [concreteFail_ok cst code hf]
    have hsf : st.setFault code = { st with status := .fault code } := by
      simp [RuntimeStore.setFault, hok]
    rw [hsf]
    refine
      { limitsValid := h.limitsValid
        limitsEq := h.limitsEq
        spBound := h.spBound
        cpBound := h.cpBound
        stepsBound := h.stepsBound
        phaseValid := h.phaseValid
        failureZeroQuotaZero := fun hz => absurd hz hcode
        slotsEq := h.slotsEq
        contsEq := h.contsEq
        phaseEq := h.phaseEq
        statusRef := ⟨rfl, hcode, hqz⟩
        stepsEq := h.stepsEq
        operandPeakEq := h.operandPeakEq
        continuationPeakEq := h.continuationPeakEq }
  · have hne : st.status ≠ .ok := fun hok =>
      hf ((storeRefines_failure_eq_zero_iff h).mpr hok)
    rw [concreteFail_failed cst code hf]
    have hsf : st.setFault code = st := by
      cases hs : st.status with
      | ok => exact absurd hs hne
      | quota _ _ => simp [RuntimeStore.setFault, hs]
      | fault _ => simp [RuntimeStore.setFault, hs]
    rw [hsf]
    exact h

theorem linear_quota_refines {cst : LinearRuntimeState} {st : RuntimeStore} (reason : QuotaReason)
    (h : StoreRefines cst st) (hr : reason ≠ .none) :
    StoreRefines (concreteQuota cst reason.code) (st.setQuota reason) := by
  by_cases hf : cst.failure = 0
  · have hok : st.status = .ok := (storeRefines_failure_eq_zero_iff h).mp hf
    rw [concreteQuota_ok cst reason.code hf]
    have hsq : st.setQuota reason = { st with status := .quota st.phase reason } := by
      simp [RuntimeStore.setQuota, hok]
    rw [hsq]
    refine
      { limitsValid := h.limitsValid
        limitsEq := h.limitsEq
        spBound := h.spBound
        cpBound := h.cpBound
        stepsBound := h.stepsBound
        phaseValid := h.phaseValid
        failureZeroQuotaZero := fun hz => absurd hz h.phaseValid
        slotsEq := h.slotsEq
        contsEq := h.contsEq
        phaseEq := h.phaseEq
        statusRef := ⟨by rw [h.phaseEq], by rw [← h.phaseEq]; exact h.phaseValid, rfl, hr⟩
        stepsEq := h.stepsEq
        operandPeakEq := h.operandPeakEq
        continuationPeakEq := h.continuationPeakEq }
  · have hne : st.status ≠ .ok := fun hok =>
      hf ((storeRefines_failure_eq_zero_iff h).mpr hok)
    rw [concreteQuota_failed cst reason.code hf]
    have hsq : st.setQuota reason = st := by
      cases hs : st.status with
      | ok => exact absurd hs hne
      | quota _ _ => simp [RuntimeStore.setQuota, hs]
      | fault _ => simp [RuntimeStore.setQuota, hs]
    rw [hsq]
    exact h

private theorem bv32_one_toNat_ws : (1 : BitVec 32).toNat = 1 := rfl

theorem linear_push_tag_refines (cst : LinearRuntimeState) (st : RuntimeStore)
    (kind : BitVec 32) (value : BitVec 64) (h : StoreRefines cst st) :
    StoreRefines (concretePushTag cst kind value) (storePushTag st kind value) := by
  by_cases hf : cst.failure = 0
  · have hok : st.status = .ok := (storeRefines_failure_eq_zero_iff h).mp hf
    have hlen : cst.sp.toNat = st.slots.length := by
      rw [← h.slotsEq, readSlotsTopFirst_length]
    have hbytes : ((cst.sp + 1) * 16).toNat = (st.slots.length + 1) * 16 := by
      rw [bv32_sp_bytes_toNat cst.sp h.spBound, hlen]
    have hol : cst.operandLimit.toNat = st.limits.operandLimit := h.limitsEq.1
    by_cases hgt : cst.operandLimit < (cst.sp + 1) * 16
    · have hgtNat : (st.slots.length + 1) * 16 > st.limits.operandLimit := by
        rw [BitVec.lt_def, hbytes, hol] at hgt; exact hgt
      rw [concretePushTag_quota cst kind value hf hgt]
      rw [storePushTag_quota st kind value hok hgtNat]
      have hq := linear_quota_refines .operand h (by decide)
      rw [concreteQuota_ok cst _ hf] at hq
      simpa [QuotaReason.code, RuntimeStore.setQuota, hok] using hq
    · have hle : (cst.sp + 1) * 16 ≤ cst.operandLimit := by
        rw [BitVec.le_def]; rw [BitVec.lt_def] at hgt; omega
      have hleNat : (st.slots.length + 1) * 16 ≤ st.limits.operandLimit := by
        rw [BitVec.le_def, hbytes, hol] at hle; exact hle
      have hlimOp : st.limits.operandLimit ≤ 2048 := h.limitsValid.1
      have hsp' : (cst.sp + 1).toNat = cst.sp.toNat + 1 := by
        rw [BitVec.toNat_add, bv32_one_toNat_ws]
        have := h.spBound; omega
      have hspLe : (cst.sp + 1).toNat ≤ 256 := by omega
      have haddr : (cst.sp * 16).toNat = cst.sp.toNat * 16 :=
        bv32_sp_addr_toNat cst.sp h.spBound
      let bytes := (cst.sp + 1) * 16
      let mem1 := storeI32 cst.mem (cst.sp * 16).toNat kind
      let mem2 := storeI64 mem1 ((cst.sp * 16).toNat + 8) value
      have hslots2 : readSlotsTopFirst mem2 (cst.sp + 1).toNat = ⟨kind, value⟩ :: st.slots := by
        rw [hsp']
        simp only [readSlotsTopFirst, List.cons.injEq]
        refine ⟨?_, ?_⟩
        · unfold readSlot
          have hk : loadI32 mem2 (cst.sp.toNat * 16) = kind := by
            have heq : loadI32 mem2 (cst.sp.toNat * 16) = loadI32 mem1 (cst.sp.toNat * 16) := by
              have hpres0 : MemEqOn (cst.sp.toNat * 16) (cst.sp.toNat * 16 + 4) mem1 mem2 := by
                refine storeI64_preserves_outside value (Or.inr ?_)
                rw [haddr]; omega
              exact loadI32_memEqOn hpres0.symm (Nat.le_refl _) (Nat.le_refl _)
            rw [heq]
            dsimp only [mem1]
            rw [haddr]
            exact loadI32_storeI32 cst.mem (cst.sp.toNat * 16) kind
          have hv : loadI64 mem2 (cst.sp.toNat * 16 + 8) = value := by
            dsimp only [mem2]
            rw [haddr]
            exact loadI64_storeI64 mem1 (cst.sp.toNat * 16 + 8) value
          rw [hk, hv]
        · have hpres : MemEqOn 0 (cst.sp.toNat * 16) cst.mem mem2 := by
            have hm1 : MemEqOn 0 (cst.sp.toNat * 16) cst.mem mem1 := by
              refine storeI32_preserves_outside kind (Or.inr ?_)
              rw [haddr]; omega
            have hm2 : MemEqOn 0 (cst.sp.toNat * 16) mem1 mem2 := by
              refine storeI64_preserves_outside value (Or.inr ?_)
              rw [haddr]; omega
            exact hm1.trans hm2
          rw [readSlotsTopFirst_memEqOn hpres.symm rfl (Nat.le_refl _)]
          exact h.slotsEq
      have hconts2 : readContsTopFirst mem2 cst.cp.toNat = st.conts := by
        have hc := concretePushTag_preserves_contRegion cst kind value h.spBound (by omega)
        rw [concretePushTag_ok cst kind value hf hle] at hc
        have hmem := hc.1
        have hcp := h.cpBound
        rw [readContsTopFirst_memEqOn hmem.symm (Nat.le_refl 4096) (by omega)]
        exact h.contsEq
      have hpeak : (if cst.operandPeak < bytes then bytes else cst.operandPeak).toNat =
          Nat.max st.operandPeak ((st.slots.length + 1) * 16) := by
        by_cases hp : cst.operandPeak < bytes
        · simp only [hp, ↓reduceIte]
          rw [BitVec.lt_def] at hp
          have h1 : st.operandPeak < (st.slots.length + 1) * 16 := by
            rw [← h.operandPeakEq, ← hbytes]; exact hp
          rw [hbytes]
          simp only [Nat.max_def]
          split <;> omega
        · simp only [hp, ↓reduceIte]
          rw [BitVec.lt_def] at hp
          have h1 : (st.slots.length + 1) * 16 ≤ st.operandPeak := by
            rw [← h.operandPeakEq, ← hbytes]; exact Nat.le_of_not_lt hp
          rw [h.operandPeakEq]
          simp only [Nat.max_def]
          split <;> omega
      rw [concretePushTag_ok cst kind value hf hle]
      rw [storePushTag_ok st kind value hok hleNat]
      exact
        { limitsValid := h.limitsValid
          limitsEq := h.limitsEq
          spBound := hspLe
          cpBound := h.cpBound
          stepsBound := h.stepsBound
          phaseValid := h.phaseValid
          failureZeroQuotaZero := h.failureZeroQuotaZero
          slotsEq := hslots2
          contsEq := hconts2
          phaseEq := h.phaseEq
          statusRef := h.statusRef
          stepsEq := h.stepsEq
          operandPeakEq := hpeak
          continuationPeakEq := h.continuationPeakEq }
  · have hne : st.status ≠ .ok := fun hok =>
      hf ((storeRefines_failure_eq_zero_iff h).mpr hok)
    rw [concretePushTag_failed cst kind value hf, storePushTag_failed st kind value hne]
    exact h

theorem linear_pop_kind_refines (cst : LinearRuntimeState) (st : RuntimeStore)
    (expected : BitVec 32) (h : StoreRefines cst st) :
    (concretePopKind cst expected).1 = (storePopKind st expected).1 ∧
    StoreRefines (concretePopKind cst expected).2 (storePopKind st expected).2 := by
  by_cases hf : cst.failure = 0
  · have hok : st.status = .ok := (storeRefines_failure_eq_zero_iff h).mp hf
    cases hs : st.slots with
    | nil =>
        have hsp0 : cst.sp.toNat = 0 := by
          rw [← readSlotsTopFirst_length cst.mem cst.sp.toNat, h.slotsEq, hs]; rfl
        have hlt : cst.sp < 1 := by
          rw [BitVec.lt_def, hsp0, bv32_one_toNat_ws]; omega
        rw [concretePopKind_underflow cst expected hf hlt]
        rw [storePopKind_underflow st expected hok hs]
        refine ⟨rfl, ?_⟩
        have hfr := linear_fail_refines faultCode h (by decide)
        rw [concreteFail_ok cst faultCode hf] at hfr
        simpa [RuntimeStore.setFault, hok, faultCode] using hfr
    | cons top rest =>
        have hlen : cst.sp.toNat = rest.length + 1 := by
          rw [← readSlotsTopFirst_length cst.mem cst.sp.toNat, h.slotsEq, hs]; rfl
        have hle1 : (1 : BitVec 32) ≤ cst.sp := by
          rw [BitVec.le_def, bv32_one_toNat_ws, hlen]; omega
        have hsp' : (cst.sp - 1).toNat = rest.length := by
          rw [BitVec.toNat_sub_of_le hle1, hlen, bv32_one_toNat_ws]; omega
        have hspSubBound : (cst.sp - 1).toNat ≤ 256 := by
          have := h.spBound; rw [hsp']; omega
        have haddr : ((cst.sp - 1) * 16).toNat = rest.length * 16 := by
          rw [bv32_sp_addr_toNat (cst.sp - 1) hspSubBound, hsp']
        have hslEq := h.slotsEq
        rw [hlen, hs] at hslEq
        simp only [readSlotsTopFirst, List.cons.injEq] at hslEq
        rcases hslEq with ⟨htop, hrest⟩
        have htag : loadI32 cst.mem ((cst.sp - 1) * 16).toNat = top.tag := by
          rw [haddr, ← htop]; rfl
        have hpay : loadI64 cst.mem (((cst.sp - 1) * 16).toNat + 8) = top.payload := by
          rw [haddr, ← htop]; rfl
        by_cases htagEq : top.tag = expected
        · have heq : loadI32 cst.mem ((cst.sp - 1) * 16).toNat = expected := by
            rw [htag, htagEq]
          rw [concretePopKind_ok cst expected hf hle1 heq]
          rw [concreteTake_ok cst hf hle1]
          have hs' : st.slots = ⟨expected, top.payload⟩ :: rest := by
            rw [hs, ← htagEq]
          rw [storePopKind_ok st expected top.payload rest hok hs']
          dsimp only
          refine ⟨hpay, ?_⟩
          have hslots2 : readSlotsTopFirst
              (storeI64 (storeI64 cst.mem ((cst.sp - 1) * 16).toNat 0)
                (((cst.sp - 1) * 16).toNat + 8) 0) (cst.sp - 1).toNat = rest := by
            have htake := concreteTake_preserves_below cst h.spBound
            rw [concreteTake_ok cst hf hle1] at htake
            dsimp only at htake
            have hbound : (cst.sp.toNat - 1) * 16 = (cst.sp - 1).toNat * 16 := by
              rw [hsp', hlen]; simp
            rw [hbound] at htake
            rw [readSlotsTopFirst_memEqOn htake.symm rfl (Nat.le_refl _), hsp']
            exact hrest
          have hconts2 : readContsTopFirst
              (storeI64 (storeI64 cst.mem ((cst.sp - 1) * 16).toNat 0)
                (((cst.sp - 1) * 16).toNat + 8) 0) cst.cp.toNat = st.conts := by
            have htake := concreteTake_preserves_contRegion cst h.spBound
            rw [concreteTake_ok cst hf hle1] at htake
            dsimp only at htake
            have hcp := h.cpBound
            rw [readContsTopFirst_memEqOn htake.1.symm (Nat.le_refl 4096) (by omega)]
            exact h.contsEq
          exact
            { limitsValid := h.limitsValid
              limitsEq := h.limitsEq
              spBound := hspSubBound
              cpBound := h.cpBound
              stepsBound := h.stepsBound
              phaseValid := h.phaseValid
              failureZeroQuotaZero := h.failureZeroQuotaZero
              slotsEq := hslots2
              contsEq := hconts2
              phaseEq := h.phaseEq
              statusRef := h.statusRef
              stepsEq := h.stepsEq
              operandPeakEq := h.operandPeakEq
              continuationPeakEq := h.continuationPeakEq }
        · have hne : loadI32 cst.mem ((cst.sp - 1) * 16).toNat ≠ expected := by
            rw [htag]; exact htagEq
          rw [concretePopKind_mismatch cst expected hf hle1 hne]
          rw [storePopKind_tag_mismatch st expected top rest hok hs htagEq]
          refine ⟨rfl, ?_⟩
          have hfr := linear_fail_refines faultCode h (by decide)
          rw [concreteFail_ok cst faultCode hf] at hfr
          simpa [RuntimeStore.setFault, hok, faultCode] using hfr
  · have hne : st.status ≠ .ok := fun hok =>
      hf ((storeRefines_failure_eq_zero_iff h).mpr hok)
    rw [concretePopKind_failed cst expected hf, storePopKind_failed st expected hne]
    exact ⟨rfl, h⟩

theorem linear_enqueue_refines (cst : LinearRuntimeState) (st : RuntimeStore)
    (fnIdx env : BitVec 32) (h : StoreRefines cst st) :
    StoreRefines (concreteEnqueue cst fnIdx env) (storeEnqueue st fnIdx env) := by
  by_cases hf : cst.failure = 0
  · have hok : st.status = .ok := (storeRefines_failure_eq_zero_iff h).mp hf
    have hlen : cst.cp.toNat = st.conts.length := by
      rw [← h.contsEq, readContsTopFirst_length]
    have hbytes : ((cst.cp + 1) * 8).toNat = (st.conts.length + 1) * 8 := by
      rw [bv32_cp_bytes_toNat cst.cp h.cpBound, hlen]
    have hcl : cst.continuationLimit.toNat = st.limits.continuationLimit := h.limitsEq.2.1
    by_cases hgt : cst.continuationLimit < (cst.cp + 1) * 8
    · have hgtNat : (st.conts.length + 1) * 8 > st.limits.continuationLimit := by
        rw [BitVec.lt_def, hbytes, hcl] at hgt; exact hgt
      rw [concreteEnqueue_quota cst fnIdx env hf hgt]
      rw [storeEnqueue_quota st fnIdx env hok hgtNat]
      have hq := linear_quota_refines .continuation h (by decide)
      rw [concreteQuota_ok cst _ hf] at hq
      simpa [QuotaReason.code, RuntimeStore.setQuota, hok] using hq
    · have hle : (cst.cp + 1) * 8 ≤ cst.continuationLimit := by
        rw [BitVec.le_def]; rw [BitVec.lt_def] at hgt; omega
      have hleNat : (st.conts.length + 1) * 8 ≤ st.limits.continuationLimit := by
        rw [BitVec.le_def, hbytes, hcl] at hle; exact hle
      have hlimCont : st.limits.continuationLimit ≤ 4096 := h.limitsValid.2.1
      have hcp' : (cst.cp + 1).toNat = cst.cp.toNat + 1 := by
        rw [BitVec.toNat_add, bv32_one_toNat_ws]
        have := h.cpBound; omega
      have hcpLe : (cst.cp + 1).toNat ≤ 512 := by omega
      have haddr : ((4096 : BitVec 32) + cst.cp * 8).toNat = 4096 + cst.cp.toNat * 8 :=
        bv32_cp_addr_toNat cst.cp h.cpBound
      let bytes := (cst.cp + 1) * 8
      let mem1 := storeI32 cst.mem ((4096 : BitVec 32) + cst.cp * 8).toNat fnIdx
      let mem2 := storeI32 mem1 (((4096 : BitVec 32) + cst.cp * 8).toNat + 4) env
      have hconts2 : readContsTopFirst mem2 (cst.cp + 1).toNat = ⟨fnIdx, env⟩ :: st.conts := by
        rw [hcp']
        simp only [readContsTopFirst, List.cons.injEq]
        refine ⟨?_, ?_⟩
        · unfold readCont
          have h1 : loadI32 mem2 (4096 + cst.cp.toNat * 8) = fnIdx := by
            have heq : loadI32 mem2 (4096 + cst.cp.toNat * 8) =
                loadI32 mem1 (4096 + cst.cp.toNat * 8) := by
              have hpres0 : MemEqOn (4096 + cst.cp.toNat * 8) (4096 + cst.cp.toNat * 8 + 4) mem1 mem2 := by
                dsimp only [mem2]
                refine storeI32_preserves_outside env (Or.inr ?_)
                rw [haddr]; omega
              exact loadI32_memEqOn hpres0.symm (Nat.le_refl _) (Nat.le_refl _)
            rw [heq]
            dsimp only [mem1]
            rw [haddr]
            exact loadI32_storeI32 cst.mem (4096 + cst.cp.toNat * 8) fnIdx
          have h2 : loadI32 mem2 (4096 + cst.cp.toNat * 8 + 4) = env := by
            dsimp only [mem2]
            rw [haddr]
            exact loadI32_storeI32 mem1 (4096 + cst.cp.toNat * 8 + 4) env
          rw [h1, h2]
        · have hpres : MemEqOn 4096 (4096 + cst.cp.toNat * 8) cst.mem mem2 := by
            have hm1 : MemEqOn 4096 (4096 + cst.cp.toNat * 8) cst.mem mem1 := by
              dsimp only [mem1]
              refine storeI32_preserves_outside fnIdx (Or.inr ?_)
              rw [haddr]; omega
            have hm2 : MemEqOn 4096 (4096 + cst.cp.toNat * 8) mem1 mem2 := by
              dsimp only [mem2]
              refine storeI32_preserves_outside env (Or.inr ?_)
              rw [haddr]; omega
            exact hm1.trans hm2
          rw [readContsTopFirst_memEqOn hpres.symm (Nat.le_refl 4096) (Nat.le_refl _)]
          exact h.contsEq
      have hslots2 : readSlotsTopFirst mem2 cst.sp.toNat = st.slots := by
        have he := concreteEnqueue_preserves_opRegion cst fnIdx env h.cpBound
        rw [concreteEnqueue_ok cst fnIdx env hf hle] at he
        have hsp := h.spBound
        rw [readSlotsTopFirst_memEqOn he.1.symm rfl (by omega)]
        exact h.slotsEq
      have hpeak : (if cst.continuationPeak < bytes then bytes else cst.continuationPeak).toNat =
          Nat.max st.continuationPeak ((st.conts.length + 1) * 8) := by
        by_cases hp : cst.continuationPeak < bytes
        · simp only [hp, ↓reduceIte]
          rw [BitVec.lt_def] at hp
          have h1 : st.continuationPeak < (st.conts.length + 1) * 8 := by
            rw [← h.continuationPeakEq, ← hbytes]; exact hp
          rw [hbytes]
          simp only [Nat.max_def]
          split <;> omega
        · simp only [hp, ↓reduceIte]
          rw [BitVec.lt_def] at hp
          have h1 : (st.conts.length + 1) * 8 ≤ st.continuationPeak := by
            rw [← h.continuationPeakEq, ← hbytes]; exact Nat.le_of_not_lt hp
          rw [h.continuationPeakEq]
          simp only [Nat.max_def]
          split <;> omega
      rw [concreteEnqueue_ok cst fnIdx env hf hle]
      have hstEq : storeEnqueue st fnIdx env =
          { st with
            conts := ⟨fnIdx, env⟩ :: st.conts
            continuationPeak := Nat.max st.continuationPeak ((st.conts.length + 1) * 8) } :=
        storeEnqueue_ok st fnIdx env hok hleNat
      rw [hstEq]
      exact
        { limitsValid := h.limitsValid
          limitsEq := h.limitsEq
          spBound := h.spBound
          cpBound := hcpLe
          stepsBound := h.stepsBound
          phaseValid := h.phaseValid
          failureZeroQuotaZero := h.failureZeroQuotaZero
          slotsEq := hslots2
          contsEq := hconts2
          phaseEq := h.phaseEq
          statusRef := h.statusRef
          stepsEq := h.stepsEq
          operandPeakEq := h.operandPeakEq
          continuationPeakEq := hpeak }
  · have hne : st.status ≠ .ok := fun hok =>
      hf ((storeRefines_failure_eq_zero_iff h).mpr hok)
    have hstEq : storeEnqueue st fnIdx env = st := by
      cases hs : st.status with
      | ok => exact absurd hs hne
      | quota _ _ => simp [storeEnqueue, hs]
      | fault _ => simp [storeEnqueue, hs]
    rw [concreteEnqueue_failed cst fnIdx env hf, hstEq]
    exact h

theorem linear_dequeue_refines (cst : LinearRuntimeState) (st : RuntimeStore)
    (c : ContEntry) (rest : List ContEntry)
    (h : StoreRefines cst st) (hok : st.status = .ok) (hc : st.conts = c :: rest) :
    (concreteDequeue cst).1 = (c.fnIdx, c.env) ∧
    StoreRefines (concreteDequeue cst).2 { st with conts := rest } := by
  have hf : cst.failure = 0 := (storeRefines_failure_eq_zero_iff h).mpr hok
  have hlen : cst.cp.toNat = rest.length + 1 := by
    rw [← readContsTopFirst_length cst.mem cst.cp.toNat, h.contsEq, hc]; rfl
  have hne0 : cst.cp ≠ 0 := by
    intro hz; simp [hz] at hlen
  have hge1 : (1 : BitVec 32) ≤ cst.cp := by
    rw [BitVec.le_def, bv32_one_toNat_ws]; omega
  have hcp' : (cst.cp - 1).toNat = rest.length := by
    rw [BitVec.toNat_sub_of_le hge1, hlen, bv32_one_toNat_ws]; omega
  have hcpSubBound : (cst.cp - 1).toNat ≤ 512 := by
    have := h.cpBound; rw [hcp']; omega
  have haddr : ((4096 : BitVec 32) + (cst.cp - 1) * 8).toNat = 4096 + rest.length * 8 := by
    rw [bv32_cp_addr_toNat (cst.cp - 1) hcpSubBound, hcp']
  have hceq := h.contsEq
  rw [hlen, hc] at hceq
  simp only [readContsTopFirst, List.cons.injEq] at hceq
  rcases hceq with ⟨hhead, hrest⟩
  have hfn : loadI32 cst.mem ((4096 : BitVec 32) + (cst.cp - 1) * 8).toNat = c.fnIdx := by
    rw [haddr, ← hhead]; rfl
  have henv : loadI32 cst.mem (((4096 : BitVec 32) + (cst.cp - 1) * 8).toNat + 4) = c.env := by
    rw [haddr, ← hhead]; rfl
  rw [concreteDequeue_ok cst hf hne0]
  dsimp only
  refine ⟨by rw [hfn, henv], ?_⟩
  have hslots2 : readSlotsTopFirst
      (storeI64 cst.mem ((4096 : BitVec 32) + (cst.cp - 1) * 8).toNat 0) cst.sp.toNat = st.slots := by
    have hd := concreteDequeue_preserves_opRegion cst h.cpBound
    rw [concreteDequeue_ok cst hf hne0] at hd
    dsimp only at hd
    have hsp := h.spBound
    rw [readSlotsTopFirst_memEqOn hd.1.symm rfl (by omega)]
    exact h.slotsEq
  have hconts2 : readContsTopFirst
      (storeI64 cst.mem ((4096 : BitVec 32) + (cst.cp - 1) * 8).toNat 0) (cst.cp - 1).toNat = rest := by
    have hd := concreteDequeue_preserves_below cst h.cpBound
    rw [concreteDequeue_ok cst hf hne0] at hd
    dsimp only at hd
    have hbound : 4096 + (cst.cp.toNat - 1) * 8 = 4096 + (cst.cp - 1).toNat * 8 := by
      rw [hcp', hlen]; simp
    rw [hbound] at hd
    rw [readContsTopFirst_memEqOn hd.symm (Nat.le_refl 4096) (Nat.le_refl _), hcp']
    exact hrest
  exact
    { limitsValid := h.limitsValid
      limitsEq := h.limitsEq
      spBound := h.spBound
      cpBound := hcpSubBound
      stepsBound := h.stepsBound
      phaseValid := h.phaseValid
      failureZeroQuotaZero := h.failureZeroQuotaZero
      slotsEq := hslots2
      contsEq := hconts2
      phaseEq := h.phaseEq
      statusRef := h.statusRef
      stepsEq := h.stepsEq
      operandPeakEq := h.operandPeakEq
      continuationPeakEq := h.continuationPeakEq }

theorem linear_runtime_tick_refines (cst : LinearRuntimeState) (st : RuntimeStore)
    (h : StoreRefines cst st) :
    (concreteRuntimeTick cst).1 = (storeRuntimeTick st).1 ∧
    StoreRefines (concreteRuntimeTick cst).2 (storeRuntimeTick st).2 := by
  by_cases hf : cst.failure = 0
  · have hok : st.status = .ok := (storeRefines_failure_eq_zero_iff h).mp hf
    have hsl : cst.stepLimit.toNat = st.limits.stepLimit := h.limitsEq.2.2
    have hst : cst.steps.toNat = st.steps := h.stepsEq
    by_cases hge : cst.stepLimit ≤ cst.steps
    · have hgeNat : st.steps ≥ st.limits.stepLimit := by
        rw [BitVec.le_def, hsl, hst] at hge; exact hge
      rw [concreteRuntimeTick_quota cst hf hge]
      rw [storeRuntimeTick_quota st hok hgeNat]
      refine ⟨rfl, ?_⟩
      have hq := linear_quota_refines .steps h (by decide)
      rw [concreteQuota_ok cst _ hf] at hq
      simpa [QuotaReason.code, RuntimeStore.setQuota, hok] using hq
    · have hlt : cst.steps < cst.stepLimit := by
        rw [BitVec.lt_def]; rw [BitVec.le_def] at hge; omega
      have hltNat : st.steps < st.limits.stepLimit := by
        rw [BitVec.lt_def, hsl, hst] at hlt; exact hlt
      have hlimStep : st.limits.stepLimit ≤ 100000 := h.limitsValid.2.2
      have hsteps' : (cst.steps + 1).toNat = cst.steps.toNat + 1 := by
        rw [BitVec.toNat_add, bv32_one_toNat_ws]
        omega
      rw [concreteRuntimeTick_ok cst hf hlt]
      rw [storeRuntimeTick_ok st hok hltNat]
      refine ⟨rfl, ?_⟩
      exact
        { limitsValid := h.limitsValid
          limitsEq := h.limitsEq
          spBound := h.spBound
          cpBound := h.cpBound
          stepsBound := by dsimp only; rw [hsteps']; omega
          phaseValid := h.phaseValid
          failureZeroQuotaZero := h.failureZeroQuotaZero
          slotsEq := h.slotsEq
          contsEq := h.contsEq
          phaseEq := h.phaseEq
          statusRef := h.statusRef
          stepsEq := by dsimp only; rw [hsteps', hst]
          operandPeakEq := h.operandPeakEq
          continuationPeakEq := h.continuationPeakEq }
  · have hne : st.status ≠ .ok := fun hok =>
      hf ((storeRefines_failure_eq_zero_iff h).mpr hok)
    have hstEq : storeRuntimeTick st = (0, st) := by
      cases hs : st.status with
      | ok => exact absurd hs hne
      | quota _ _ => simp [storeRuntimeTick, hs]
      | fault _ => simp [storeRuntimeTick, hs]
    rw [concreteRuntimeTick_failed cst hf, hstEq]
    exact ⟨rfl, h⟩

/-- Structural writer populating top-first operand slots in `[0, slots.length * 16)`. -/
def writeSlotsTopFirst (mem : LinearMem) : List TaggedSlot → LinearMem
  | [] => mem
  | s :: rest =>
      let m := writeSlotsTopFirst mem rest
      let addr := rest.length * 16
      storeI64 (storeI32 m addr s.tag) (addr + 8) s.payload

/-- Structural writer populating top-first continuation entries in `[4096, 4096 + conts.length * 8)`. -/
def writeContsTopFirst (mem : LinearMem) : List ContEntry → LinearMem
  | [] => mem
  | c :: rest =>
      let m := writeContsTopFirst mem rest
      let addr := 4096 + rest.length * 8
      storeI32 (storeI32 m addr c.fnIdx) (addr + 4) c.env

private theorem readSlotsTopFirst_writeSlotsTopFirst (mem : LinearMem) (slots : List TaggedSlot) :
    readSlotsTopFirst (writeSlotsTopFirst mem slots) slots.length = slots := by
  induction slots with
  | nil => rfl
  | cons s rest ih =>
      simp only [List.length_cons, readSlotsTopFirst, List.cons.injEq]
      let m := writeSlotsTopFirst mem rest
      let addr := rest.length * 16
      let m' := storeI64 (storeI32 m addr s.tag) (addr + 8) s.payload
      refine ⟨?_, ?_⟩
      · unfold readSlot
        have hk : loadI32 m' addr = s.tag := by
          have heq : loadI32 m' addr = loadI32 (storeI32 m addr s.tag) addr := by
            have hpres0 : MemEqOn addr (addr + 4) (storeI32 m addr s.tag) m' :=
              storeI64_preserves_outside s.payload (Or.inr (by show addr + 4 ≤ addr + 8; omega))
            exact loadI32_memEqOn hpres0.symm (Nat.le_refl _) (Nat.le_refl _)
          rw [heq]
          exact loadI32_storeI32 m addr s.tag
        have hv : loadI64 m' (addr + 8) = s.payload :=
          loadI64_storeI64 (storeI32 m addr s.tag) (addr + 8) s.payload
        change ⟨loadI32 m' addr, loadI64 m' (addr + 8)⟩ = s
        rw [hk, hv]
      · have hpres : MemEqOn 0 (rest.length * 16) m m' := by
          have hm1 : MemEqOn 0 (rest.length * 16) m (storeI32 m addr s.tag) :=
            storeI32_preserves_outside s.tag (Or.inr (by omega))
          have hm2 : MemEqOn 0 (rest.length * 16) (storeI32 m addr s.tag) m' := by
            dsimp only [m', addr]
            exact storeI64_preserves_outside s.payload (Or.inr (by omega))
          exact hm1.trans hm2
        change readSlotsTopFirst m' rest.length = rest
        rw [readSlotsTopFirst_memEqOn hpres.symm rfl (Nat.le_refl _)]
        exact ih

private theorem writeContsTopFirst_preserves_opRegion (mem : LinearMem) (conts : List ContEntry) :
    MemEqOn 0 4096 mem (writeContsTopFirst mem conts) := by
  induction conts with
  | nil => exact MemEqOn.refl 0 4096 mem
  | cons c rest ih =>
      let m := writeContsTopFirst mem rest
      let addr := 4096 + rest.length * 8
      have hm1 : MemEqOn 0 4096 m (storeI32 m addr c.fnIdx) :=
        storeI32_preserves_outside c.fnIdx (Or.inr (by omega))
      have hm2 : MemEqOn 0 4096 (storeI32 m addr c.fnIdx)
          (storeI32 (storeI32 m addr c.fnIdx) (addr + 4) c.env) :=
        storeI32_preserves_outside c.env (Or.inr (by omega))
      exact ih.trans (hm1.trans hm2)

private theorem readContsTopFirst_writeContsTopFirst (mem : LinearMem) (conts : List ContEntry) :
    readContsTopFirst (writeContsTopFirst mem conts) conts.length = conts := by
  induction conts with
  | nil => rfl
  | cons c rest ih =>
      simp only [List.length_cons, readContsTopFirst, List.cons.injEq]
      let m := writeContsTopFirst mem rest
      let addr := 4096 + rest.length * 8
      let m' := storeI32 (storeI32 m addr c.fnIdx) (addr + 4) c.env
      refine ⟨?_, ?_⟩
      · unfold readCont
        have h1 : loadI32 m' addr = c.fnIdx := by
          have heq : loadI32 m' addr = loadI32 (storeI32 m addr c.fnIdx) addr := by
            have hpres0 : MemEqOn addr (addr + 4) (storeI32 m addr c.fnIdx) m' :=
              storeI32_preserves_outside c.env (Or.inr (by show addr + 4 ≤ addr + 4; omega))
            exact loadI32_memEqOn hpres0.symm (Nat.le_refl _) (Nat.le_refl _)
          rw [heq]
          exact loadI32_storeI32 m addr c.fnIdx
        have h2 : loadI32 m' (addr + 4) = c.env :=
          loadI32_storeI32 (storeI32 m addr c.fnIdx) (addr + 4) c.env
        change ⟨loadI32 m' addr, loadI32 m' (addr + 4)⟩ = c
        rw [h1, h2]
      · have hpres : MemEqOn 4096 (4096 + rest.length * 8) m m' := by
          have hm1 : MemEqOn 4096 (4096 + rest.length * 8) m (storeI32 m addr c.fnIdx) :=
            storeI32_preserves_outside c.fnIdx (Or.inr (by omega))
          have hm2 : MemEqOn 4096 (4096 + rest.length * 8) (storeI32 m addr c.fnIdx) m' := by
            dsimp only [m', addr]
            exact storeI32_preserves_outside c.env (Or.inr (by omega))
          exact hm1.trans hm2
        change readContsTopFirst m' rest.length = rest
        rw [readContsTopFirst_memEqOn hpres.symm (Nat.le_refl 4096) (Nat.le_refl _)]
        exact ih

/-- Realizability: every bounded `RuntimeStore` with valid status globals has a
    concrete `[0, 8192)` `LinearRuntimeState` realizing it. -/
theorem storeRefines_realizable (st : RuntimeStore)
    (hlim : ProfileLimitsValid st.limits)
    (hsp : st.slots.length ≤ 256)
    (hcp : st.conts.length ≤ 512)
    (hsteps : st.steps ≤ 100000)
    (hphase : 1 + st.phase ≠ 0)
    (hopPeak : st.operandPeak < 2 ^ 32)
    (hcontPeak : st.continuationPeak < 2 ^ 32)
    (hstat : StatusGlobalValid st.status) :
    ∃ cst, StoreRefines cst st := by
  let mem := writeContsTopFirst (writeSlotsTopFirst zeroMem st.slots) st.conts
  let cst : LinearRuntimeState :=
    { mem := mem
      sp := BitVec.ofNat 32 st.slots.length
      cp := BitVec.ofNat 32 st.conts.length
      failure := st.status.failureGlobal
      quotaReason := st.status.quotaGlobal
      phase := st.phase
      steps := BitVec.ofNat 32 st.steps
      operandLimit := BitVec.ofNat 32 st.limits.operandLimit
      continuationLimit := BitVec.ofNat 32 st.limits.continuationLimit
      stepLimit := BitVec.ofNat 32 st.limits.stepLimit
      operandPeak := BitVec.ofNat 32 st.operandPeak
      continuationPeak := BitVec.ofNat 32 st.continuationPeak }
  have hspNat : cst.sp.toNat = st.slots.length := bv32_ofNat_toNat (by omega)
  have hcpNat : cst.cp.toNat = st.conts.length := bv32_ofNat_toNat (by omega)
  have hstepsNat : cst.steps.toNat = st.steps := bv32_ofNat_toNat (by omega)
  have holNat : cst.operandLimit.toNat = st.limits.operandLimit :=
    bv32_ofNat_toNat (by have := hlim.1; omega)
  have hclNat : cst.continuationLimit.toNat = st.limits.continuationLimit :=
    bv32_ofNat_toNat (by have := hlim.2.1; omega)
  have hslNat : cst.stepLimit.toNat = st.limits.stepLimit :=
    bv32_ofNat_toNat (by have := hlim.2.2; omega)
  have hopNat : cst.operandPeak.toNat = st.operandPeak := bv32_ofNat_toNat hopPeak
  have hcontNat : cst.continuationPeak.toNat = st.continuationPeak := bv32_ofNat_toNat hcontPeak
  have hslotsEq : readSlotsTopFirst cst.mem cst.sp.toNat = st.slots := by
    rw [hspNat]
    have hpres := writeContsTopFirst_preserves_opRegion (writeSlotsTopFirst zeroMem st.slots) st.conts
    rw [readSlotsTopFirst_memEqOn hpres.symm rfl (by omega)]
    exact readSlotsTopFirst_writeSlotsTopFirst zeroMem st.slots
  have hcontsEq : readContsTopFirst cst.mem cst.cp.toNat = st.conts := by
    rw [hcpNat]
    exact readContsTopFirst_writeContsTopFirst (writeSlotsTopFirst zeroMem st.slots) st.conts
  have hsr : StatusRefines cst.failure cst.quotaReason st.status := by
    cases hs : st.status with
    | ok =>
        dsimp only [cst]
        rw [hs]
        exact ⟨rfl, rfl⟩
    | quota qph r =>
        rw [hs] at hstat
        dsimp only [cst]
        rw [hs]
        exact ⟨rfl, hstat.1, rfl, hstat.2⟩
    | fault c =>
        rw [hs] at hstat
        dsimp only [cst]
        rw [hs]
        exact ⟨rfl, hstat, rfl⟩
  have hfz : cst.failure = 0 → cst.quotaReason = 0 := by
    intro hf
    cases hs : st.status with
    | ok => simp [cst, hs, FailureStatus.quotaGlobal]
    | quota qph r =>
        rw [hs] at hstat
        simp only [cst, hs, FailureStatus.failureGlobal] at hf
        exact absurd hf hstat.1
    | fault c => simp [cst, hs, FailureStatus.quotaGlobal]
  exact ⟨cst,
    { limitsValid := hlim
      limitsEq := ⟨holNat, hclNat, hslNat⟩
      spBound := by omega
      cpBound := by omega
      stepsBound := by omega
      phaseValid := hphase
      failureZeroQuotaZero := hfz
      slotsEq := hslotsEq
      contsEq := hcontsEq
      phaseEq := rfl
      statusRef := hsr
      stepsEq := hstepsNat
      operandPeakEq := hopNat
      continuationPeakEq := hcontNat }⟩

/-- Complete constructive proof discharging `LinearMemorySlotPremise` over `[0, 8192)`. -/
theorem linearMemorySlotPremise_holds : LinearMemorySlotPremise where
  pushTagRefines := linear_push_tag_refines
  popKindRefines := linear_pop_kind_refines
  enqueueRefines := linear_enqueue_refines
  dequeueRefines := linear_dequeue_refines
  runtimeTickRefines := linear_runtime_tick_refines
  opWritesPreserveContRegion := by
    intro cst hsp hlim
    refine ⟨?_, ?_⟩
    · intro k v
      have h := concretePushTag_preserves_contRegion cst k v hsp hlim
      exact ⟨h.1, h.2.1⟩
    · intro exp
      have h := concretePopKind_preserves_contRegion cst exp hsp
      exact ⟨h.1, h.2.1⟩
  contWritesPreserveOpRegion := by
    intro cst hcp
    refine ⟨?_, ?_⟩
    · intro fnIdx env
      have h := concreteEnqueue_preserves_opRegion cst fnIdx env hcp
      exact ⟨h.1, h.2.1⟩
    · have h := concreteDequeue_preserves_opRegion cst hcp
      exact ⟨h.1, h.2.1⟩
  tickPreservesBothRegions := by
    intro cst
    have h := concreteRuntimeTick_preserves_mem cst
    refine ⟨?_, h.2.1, h.2.2.1⟩
    rw [h.1]
    exact MemEqOn.refl 0 8192 cst.mem
  realizable := storeRefines_realizable

/-- Concrete `[0, 8192)` linear-memory Wasm configuration. -/
structure LinearConfig where
  frames : List Frame
  store : LinearRuntimeState

def initLinearConfig (code : List Instr) (cst : LinearRuntimeState) : LinearConfig :=
  { frames := [⟨[], [], code, 0⟩], store := cst }

def doneLinearConfig (cst : LinearRuntimeState) : LinearConfig :=
  { frames := [⟨[], [], [], 0⟩], store := cst }

def doneValLinearConfig (v : WVal) (cst : LinearRuntimeState) : LinearConfig :=
  { frames := [⟨[], [v], [], 1⟩], store := cst }

/-- Concrete `[0, 8192)` small-step operational semantics replacing the abstract store
    operations with `concretePushTag`, `concretePopKind`, `concreteEnqueue`, and
    `concreteRuntimeTick`. -/
inductive LinearWasmStep (env : FuncEnv) : LinearConfig → LinearConfig → Prop where
  | i32Const {locals vstack rest r cs cst n} :
      LinearWasmStep env
        ⟨⟨locals, vstack, .i32Const n :: rest, r⟩ :: cs, cst⟩
        ⟨⟨locals, .i32 n :: vstack, rest, r⟩ :: cs, cst⟩
  | i64Const {locals vstack rest r cs cst n} :
      LinearWasmStep env
        ⟨⟨locals, vstack, .i64Const n :: rest, r⟩ :: cs, cst⟩
        ⟨⟨locals, .i64 n :: vstack, rest, r⟩ :: cs, cst⟩
  | localGet {locals vstack rest r cs cst idx v} :
      locals[idx]? = some v →
      LinearWasmStep env
        ⟨⟨locals, vstack, .localGet idx :: rest, r⟩ :: cs, cst⟩
        ⟨⟨locals, v :: vstack, rest, r⟩ :: cs, cst⟩
  | localSet {locals vstack rest r cs cst idx v} :
      idx < locals.length →
      LinearWasmStep env
        ⟨⟨locals, v :: vstack, .localSet idx :: rest, r⟩ :: cs, cst⟩
        ⟨⟨locals.set idx v, vstack, rest, r⟩ :: cs, cst⟩
  | i64Add {locals vstack rest r cs cst lhs rhs} :
      LinearWasmStep env
        ⟨⟨locals, .i64 rhs :: .i64 lhs :: vstack, .i64Add :: rest, r⟩ :: cs, cst⟩
        ⟨⟨locals, .i64 (lhs + rhs) :: vstack, rest, r⟩ :: cs, cst⟩
  | i64Sub {locals vstack rest r cs cst lhs rhs} :
      LinearWasmStep env
        ⟨⟨locals, .i64 rhs :: .i64 lhs :: vstack, .i64Sub :: rest, r⟩ :: cs, cst⟩
        ⟨⟨locals, .i64 (lhs - rhs) :: vstack, rest, r⟩ :: cs, cst⟩
  | i64Mul {locals vstack rest r cs cst lhs rhs} :
      LinearWasmStep env
        ⟨⟨locals, .i64 rhs :: .i64 lhs :: vstack, .i64Mul :: rest, r⟩ :: cs, cst⟩
        ⟨⟨locals, .i64 (lhs * rhs) :: vstack, rest, r⟩ :: cs, cst⟩
  | callDefined {locals vstack args vstack' rest r cs cst fid fdef} :
      env fid = some fdef →
      vstack = args ++ vstack' →
      args.length = fdef.paramCount →
      LinearWasmStep env
        ⟨⟨locals, vstack, .call fid :: rest, r⟩ :: cs, cst⟩
        ⟨⟨args.reverse ++ fdef.localDefaults, [], fdef.body, fdef.retArity⟩ ::
          ⟨locals, vstack', rest, r⟩ :: cs, cst⟩
  | returnEnd {locals vstack retVals vstackRem r cLocals cVstack cCode cR cs cst} :
      vstack = retVals ++ vstackRem →
      retVals.length = r →
      LinearWasmStep env
        ⟨⟨locals, vstack, [], r⟩ :: ⟨cLocals, cVstack, cCode, cR⟩ :: cs, cst⟩
        ⟨⟨cLocals, retVals ++ cVstack, cCode, cR⟩ :: cs, cst⟩
  | returnInstr {locals vstack retVals vstackRem rest r cLocals cVstack cCode cR cs cst} :
      vstack = retVals ++ vstackRem →
      retVals.length = r →
      LinearWasmStep env
        ⟨⟨locals, vstack, .return_ :: rest, r⟩ ::
          ⟨cLocals, cVstack, cCode, cR⟩ :: cs, cst⟩
        ⟨⟨cLocals, retVals ++ cVstack, cCode, cR⟩ :: cs, cst⟩
  | callPushTag {locals vstack rest r cs cst k v} :
      env .pushTag = none →
      LinearWasmStep env
        ⟨⟨locals, .i64 v :: .i32 k :: vstack, .call .pushTag :: rest, r⟩ :: cs, cst⟩
        ⟨⟨locals, vstack, rest, r⟩ :: cs, concretePushTag cst k v⟩
  | callPopKind {locals vstack rest r cs cst k} :
      env .popKind = none →
      LinearWasmStep env
        ⟨⟨locals, .i32 k :: vstack, .call .popKind :: rest, r⟩ :: cs, cst⟩
        ⟨⟨locals, .i64 (concretePopKind cst k).1 :: vstack, rest, r⟩ :: cs,
          (concretePopKind cst k).2⟩
  | callEnqueue {locals vstack rest r cs cst fnIdx envId} :
      env .enqueue = none →
      LinearWasmStep env
        ⟨⟨locals, .i32 envId :: .i32 fnIdx :: vstack, .call .enqueue :: rest, r⟩ :: cs, cst⟩
        ⟨⟨locals, vstack, rest, r⟩ :: cs, concreteEnqueue cst fnIdx envId⟩
  | callRuntimeTick {locals vstack rest r cs cst} :
      env .runtimeTick = none →
      LinearWasmStep env
        ⟨⟨locals, vstack, .call .runtimeTick :: rest, r⟩ :: cs, cst⟩
        ⟨⟨locals, .i32 (concreteRuntimeTick cst).1 :: vstack, rest, r⟩ :: cs,
          (concreteRuntimeTick cst).2⟩

/-- Reflexive-transitive closure of `LinearWasmStep`. -/
inductive LinearWasmSteps (env : FuncEnv) : LinearConfig → LinearConfig → Prop where
  | refl {c} : LinearWasmSteps env c c
  | step {c1 c2 c3} : LinearWasmStep env c1 c2 → LinearWasmSteps env c2 c3 → LinearWasmSteps env c1 c3

theorem LinearWasmSteps.single {env c1 c2} (h : LinearWasmStep env c1 c2) :
    LinearWasmSteps env c1 c2 :=
  .step h .refl

/-- Single concrete Wasm step refines `WasmStep`. -/
theorem linearWasmStep_refines {env frames frames' cst cst' st}
    (hstep : LinearWasmStep env ⟨frames, cst⟩ ⟨frames', cst'⟩)
    (href : StoreRefines cst st) :
    ∃ st', WasmStep env ⟨frames, st⟩ ⟨frames', st'⟩ ∧ StoreRefines cst' st' := by
  cases hstep with
  | i32Const => exact ⟨st, .i32Const, href⟩
  | i64Const => exact ⟨st, .i64Const, href⟩
  | localGet hg => exact ⟨st, .localGet hg, href⟩
  | localSet hl => exact ⟨st, .localSet hl, href⟩
  | i64Add => exact ⟨st, .i64Add, href⟩
  | i64Sub => exact ⟨st, .i64Sub, href⟩
  | i64Mul => exact ⟨st, .i64Mul, href⟩
  | callDefined he hvs hlen => exact ⟨st, .callDefined he hvs hlen, href⟩
  | returnEnd hvs hlen => exact ⟨st, .returnEnd hvs hlen, href⟩
  | returnInstr hvs hlen => exact ⟨st, .returnInstr hvs hlen, href⟩
  | callPushTag hnone =>
      exact ⟨storePushTag st _ _, .callPushTag hnone, linear_push_tag_refines cst st _ _ href⟩
  | @callPopKind _ _ _ _ _ _ k hnone =>
      have hp := linear_pop_kind_refines cst st k href
      refine ⟨(storePopKind st k).2, ?_, hp.2⟩
      rw [hp.1]
      exact .callPopKind hnone
  | callEnqueue hnone =>
      exact ⟨storeEnqueue st _ _, .callEnqueue hnone, linear_enqueue_refines cst st _ _ href⟩
  | callRuntimeTick hnone =>
      have ht := linear_runtime_tick_refines cst st href
      refine ⟨(storeRuntimeTick st).2, ?_, ht.2⟩
      rw [ht.1]
      exact .callRuntimeTick hnone

/-- Multi-step concrete Wasm execution refines `WasmSteps`. -/
theorem linearWasmSteps_refines {env frames frames' cst cst' st}
    (hsteps : LinearWasmSteps env ⟨frames, cst⟩ ⟨frames', cst'⟩)
    (href : StoreRefines cst st) :
    ∃ st', WasmSteps env ⟨frames, st⟩ ⟨frames', st'⟩ ∧ StoreRefines cst' st' := by
  generalize hc1 : (⟨frames, cst⟩ : LinearConfig) = c1 at hsteps
  generalize hc2 : (⟨frames', cst'⟩ : LinearConfig) = c2 at hsteps
  induction hsteps generalizing frames cst st with
  | refl =>
      subst hc1
      injection hc2 with hf hs
      subst hf; subst hs
      exact ⟨st, .refl, href⟩
  | @step c1 cm c2 hs _ ih =>
      subst hc1 hc2
      rcases cm with ⟨fMid, cstMid⟩
      obtain ⟨stMid, hws, hrefMid⟩ := linearWasmStep_refines hs href
      obtain ⟨st', hwss, href'⟩ := ih hrefMid rfl rfl
      exact ⟨st', .step hws hwss, href'⟩

/-- Concrete execution of one continuation block (`functions.rs:50-103`). -/
inductive LinearExecContBlock (env : FuncEnv) (blk : ContBlock) (envId : BitVec 32) :
    LinearRuntimeState → LinearRuntimeState → Prop where
  | alreadyFailed {cst} :
      cst.failure ≠ 0 →
      LinearExecContBlock env blk envId cst cst
  | noNext {cst cstFinal} :
      cst.failure = 0 →
      blk.next = none →
      LinearWasmSteps env (initLinearConfig blk.body cst) (doneLinearConfig cstFinal) →
      LinearExecContBlock env blk envId cst cstFinal
  | withNextFailed {cst cstEnq nextIdx} :
      cst.failure = 0 →
      blk.next = some nextIdx →
      LinearWasmSteps env
        (initLinearConfig [.i32Const (BitVec.ofNat 32 nextIdx), .i32Const envId, .call .enqueue] cst)
        (doneLinearConfig cstEnq) →
      cstEnq.failure ≠ 0 →
      LinearExecContBlock env blk envId cst cstEnq
  | withNextOk {cst cstEnq cstFinal nextIdx} :
      cst.failure = 0 →
      blk.next = some nextIdx →
      LinearWasmSteps env
        (initLinearConfig [.i32Const (BitVec.ofNat 32 nextIdx), .i32Const envId, .call .enqueue] cst)
        (doneLinearConfig cstEnq) →
      cstEnq.failure = 0 →
      LinearWasmSteps env (initLinearConfig blk.body cstEnq) (doneLinearConfig cstFinal) →
      LinearExecContBlock env blk envId cst cstFinal

/-- One concrete iteration of `$dispatch` (`stack.wat:78-88`) using `concreteDequeue`. -/
inductive LinearDispatchStep (env : FuncEnv) (tbl : ProgramTable) :
    LinearRuntimeState → LinearRuntimeState → Prop where
  | tickQuota {cst cstTick} :
      cst.failure = 0 →
      cst.cp ≠ 0 →
      LinearWasmSteps env
        ⟨[⟨[], [], [.call .runtimeTick], 1⟩], cst⟩
        (doneValLinearConfig (.i32 0) cstTick) →
      LinearDispatchStep env tbl cst cstTick
  | invokeOk {cst cstTick cstFinal blk} :
      cst.failure = 0 →
      cst.cp ≠ 0 →
      LinearWasmSteps env
        ⟨[⟨[], [], [.call .runtimeTick], 1⟩], cst⟩
        (doneValLinearConfig (.i32 1) cstTick) →
      tbl (concreteDequeue cstTick).1.1.toNat = some blk →
      LinearExecContBlock env blk (concreteDequeue cstTick).1.2 (concreteDequeue cstTick).2 cstFinal →
      LinearDispatchStep env tbl cst cstFinal
  | invokeMissing {cst cstTick} :
      cst.failure = 0 →
      cst.cp ≠ 0 →
      LinearWasmSteps env
        ⟨[⟨[], [], [.call .runtimeTick], 1⟩], cst⟩
        (doneValLinearConfig (.i32 1) cstTick) →
      tbl (concreteDequeue cstTick).1.1.toNat = none →
      LinearDispatchStep env tbl cst (concreteFail (concreteDequeue cstTick).2 faultCode)

/-- Concrete `$dispatch` run to termination (`failure ≠ 0 ∨ cp = 0`). -/
inductive LinearDispatchRun (env : FuncEnv) (tbl : ProgramTable) :
    LinearRuntimeState → LinearRuntimeState → Prop where
  | halted {cst} :
      (cst.failure ≠ 0 ∨ cst.cp = 0) →
      LinearDispatchRun env tbl cst cst
  | step {c0 c1 c2} :
      LinearDispatchStep env tbl c0 c1 →
      LinearDispatchRun env tbl c1 c2 →
      LinearDispatchRun env tbl c0 c2

/-- Complete concrete `[0, 8192)` execution of a lowered program from `entry`. -/
def linearRunLowered (env : FuncEnv) (entry : Nat) (ops : List ArithOp)
    (cst final : LinearRuntimeState) : Prop :=
  ∃ cstEnq,
    LinearWasmSteps env
      (initLinearConfig [.i32Const (BitVec.ofNat 32 entry), .i32Const 0, .call .enqueue] cst)
      (doneLinearConfig cstEnq) ∧
    LinearDispatchRun env (lowerProgram entry ops) cstEnq final

theorem linearExecContBlock_refines {env blk envId cst cstFinal st}
    (hex : LinearExecContBlock env blk envId cst cstFinal)
    (href : StoreRefines cst st) :
    ∃ stFinal, ExecContBlock env blk envId st stFinal ∧ StoreRefines cstFinal stFinal := by
  cases hex with
  | alreadyFailed hne =>
      have hne' : st.status ≠ .ok := fun hok =>
        hne ((storeRefines_failure_eq_zero_iff href).mpr hok)
      exact ⟨st, .alreadyFailed hne', href⟩
  | noNext hf hnone hsteps =>
      have hok : st.status = .ok := (storeRefines_failure_eq_zero_iff href).mp hf
      obtain ⟨stFinal, hw, hrefFinal⟩ := linearWasmSteps_refines hsteps href
      exact ⟨stFinal, .noNext hok hnone hw, hrefFinal⟩
  | withNextFailed hf hsome hsteps hfail =>
      have hok : st.status = .ok := (storeRefines_failure_eq_zero_iff href).mp hf
      obtain ⟨stEnq, hw, hrefEnq⟩ := linearWasmSteps_refines hsteps href
      have hne' : stEnq.status ≠ .ok := fun hok' =>
        hfail ((storeRefines_failure_eq_zero_iff hrefEnq).mpr hok')
      exact ⟨stEnq, .withNextFailed hok hsome hw hne', hrefEnq⟩
  | withNextOk hf hsome hsteps1 hf1 hsteps2 =>
      have hok : st.status = .ok := (storeRefines_failure_eq_zero_iff href).mp hf
      obtain ⟨stEnq, hw1, hrefEnq⟩ := linearWasmSteps_refines hsteps1 href
      have hok1 : stEnq.status = .ok := (storeRefines_failure_eq_zero_iff hrefEnq).mp hf1
      obtain ⟨stFinal, hw2, hrefFinal⟩ := linearWasmSteps_refines hsteps2 hrefEnq
      exact ⟨stFinal, .withNextOk hok hsome hw1 hok1 hw2, hrefFinal⟩

private theorem wasmSteps_runtimeTick_one_ok {env st stTick}
    (hnone : env .runtimeTick = none) (hok : st.status = .ok)
    (hwtick : WasmSteps env ⟨[⟨[], [], [.call .runtimeTick], 1⟩], st⟩ (doneValConfig (.i32 1) stTick)) :
    stTick = { st with steps := st.steps + 1 } := by
  generalize hcfg : doneValConfig (.i32 1) stTick = c2 at hwtick
  cases hwtick with
  | refl => cases hcfg
  | step hs1 hs2 =>
      cases hs1 with
      | callDefined he => rw [hnone] at he; cases he
      | callRuntimeTick =>
          cases hs2 with
          | refl =>
              by_cases hlt : st.steps < st.limits.stepLimit
              · have hokTick := storeRuntimeTick_ok st hok hlt
                rw [hokTick] at hcfg
                exact (doneValConfig_inj hcfg).2
              · have hge := Nat.not_lt.mp hlt
                have hqTick := storeRuntimeTick_quota st hok hge
                rw [hqTick] at hcfg
                have hv := (doneValConfig_inj hcfg).1
                cases hv
          | step hs3 _ => cases hs3

theorem linearDispatchStep_refines {env tbl cst cst' st}
    (hnone : env .runtimeTick = none)
    (hstep : LinearDispatchStep env tbl cst cst')
    (href : StoreRefines cst st) :
    ∃ st', DispatchStep env tbl st st' ∧ StoreRefines cst' st' := by
  cases hstep with
  | tickQuota hf hcp htick =>
      have hok : st.status = .ok := (storeRefines_failure_eq_zero_iff href).mp hf
      have hc : ∃ c rest, st.conts = c :: rest := by
        cases hcs : st.conts with
        | nil =>
            have hcp0 : cst.cp.toNat = 0 := by
              rw [← readContsTopFirst_length cst.mem cst.cp.toNat, href.contsEq, hcs]; rfl
            exact absurd (BitVec.eq_of_toNat_eq (by simpa using hcp0)) hcp
        | cons c rest => exact ⟨c, rest, rfl⟩
      rcases hc with ⟨c, rest, hc⟩
      obtain ⟨stTick, hwtick, hrefTick⟩ := linearWasmSteps_refines htick href
      exact ⟨stTick, .tickQuota hok hc hwtick, hrefTick⟩
  | invokeOk hf hcp htick htbl hex =>
      have hok : st.status = .ok := (storeRefines_failure_eq_zero_iff href).mp hf
      have hc : ∃ c rest, st.conts = c :: rest := by
        cases hcs : st.conts with
        | nil =>
            have hcp0 : cst.cp.toNat = 0 := by
              rw [← readContsTopFirst_length cst.mem cst.cp.toNat, href.contsEq, hcs]; rfl
            exact absurd (BitVec.eq_of_toNat_eq (by simpa using hcp0)) hcp
        | cons c rest => exact ⟨c, rest, rfl⟩
      rcases hc with ⟨c, rest, hc⟩
      obtain ⟨stTick, hwtick, hrefTick⟩ := linearWasmSteps_refines htick href
      have htickEq := wasmSteps_runtimeTick_one_ok hnone hok hwtick
      have htickOk : stTick.status = .ok := by rw [htickEq]; exact hok
      have htickConts : stTick.conts = c :: rest := by rw [htickEq]; exact hc
      obtain ⟨hdeqVal, hdeqRef⟩ := linear_dequeue_refines _ stTick c rest hrefTick htickOk htickConts
      have hfn : (concreteDequeue _).1.1 = c.fnIdx := congrArg Prod.fst hdeqVal
      have henv : (concreteDequeue _).1.2 = c.env := congrArg Prod.snd hdeqVal
      rw [hfn] at htbl
      rw [henv] at hex
      obtain ⟨stFinal, hexAbs, hrefFinal⟩ := linearExecContBlock_refines hex hdeqRef
      exact ⟨stFinal, .invokeOk hok hc hwtick htbl hexAbs, hrefFinal⟩
  | invokeMissing hf hcp htick htbl =>
      have hok : st.status = .ok := (storeRefines_failure_eq_zero_iff href).mp hf
      have hc : ∃ c rest, st.conts = c :: rest := by
        cases hcs : st.conts with
        | nil =>
            have hcp0 : cst.cp.toNat = 0 := by
              rw [← readContsTopFirst_length cst.mem cst.cp.toNat, href.contsEq, hcs]; rfl
            exact absurd (BitVec.eq_of_toNat_eq (by simpa using hcp0)) hcp
        | cons c rest => exact ⟨c, rest, rfl⟩
      rcases hc with ⟨c, rest, hc⟩
      obtain ⟨stTick, hwtick, hrefTick⟩ := linearWasmSteps_refines htick href
      have htickEq := wasmSteps_runtimeTick_one_ok hnone hok hwtick
      have htickOk : stTick.status = .ok := by rw [htickEq]; exact hok
      have htickConts : stTick.conts = c :: rest := by rw [htickEq]; exact hc
      obtain ⟨hdeqVal, hdeqRef⟩ := linear_dequeue_refines _ stTick c rest hrefTick htickOk htickConts
      have hfn : (concreteDequeue _).1.1 = c.fnIdx := congrArg Prod.fst hdeqVal
      rw [hfn] at htbl
      exact ⟨({ stTick with conts := rest }.setFault faultCode),
        .invokeMissing hok hc hwtick htbl,
        linear_fail_refines faultCode hdeqRef (by decide)⟩

theorem linearDispatchRun_refines {env tbl cst cstFinal st}
    (hnone : env .runtimeTick = none)
    (hrun : LinearDispatchRun env tbl cst cstFinal)
    (href : StoreRefines cst st) :
    ∃ stFinal, DispatchRun env tbl st stFinal ∧ StoreRefines cstFinal stFinal := by
  induction hrun generalizing st with
  | halted hhalt =>
      have hhalt' : st.status ≠ .ok ∨ st.conts = [] := by
        rcases hhalt with hf | hcp
        · exact Or.inl (fun hok => hf ((storeRefines_failure_eq_zero_iff href).mpr hok))
        · right
          rw [← href.contsEq, hcp]
          rfl
      exact ⟨st, .halted hhalt', href⟩
  | step hs _ ih =>
      obtain ⟨stMid, hds, hrefMid⟩ := linearDispatchStep_refines hnone hs href
      obtain ⟨stFinal, hdr, hrefFinal⟩ := ih hrefMid
      exact ⟨stFinal, .step hds hdr, hrefFinal⟩

/-- Soundness of concrete `[0, 8192)` `linearRunLowered` relative to abstract `runLowered`. -/
theorem linearRunLowered_refines_runLowered (entry : Nat) (ops : List ArithOp)
    (cst cstFinal : LinearRuntimeState) (st : RuntimeStore)
    (hrun : linearRunLowered runtimeEnv entry ops cst cstFinal)
    (href : StoreRefines cst st) :
    ∃ stFinal, runLowered runtimeEnv entry ops st stFinal ∧ StoreRefines cstFinal stFinal := by
  rcases hrun with ⟨cstEnq, henq, hdisp⟩
  obtain ⟨stEnq, hwenq, hrefEnq⟩ := linearWasmSteps_refines henq href
  obtain ⟨stFinal, hwdisp, hrefFinal⟩ := linearDispatchRun_refines rfl hdisp hrefEnq
  exact ⟨stFinal, ⟨stEnq, hwenq, hwdisp⟩, hrefFinal⟩

private theorem linearWasmSteps_preserves_failure_ne {env f cst f' cst'}
    (h : LinearWasmSteps env ⟨f, cst⟩ ⟨f', cst'⟩) (hf : cst.failure ≠ 0) :
    cst'.failure ≠ 0 := by
  generalize hc1 : (⟨f, cst⟩ : LinearConfig) = c1 at h
  generalize hc2 : (⟨f', cst'⟩ : LinearConfig) = c2 at h
  induction h generalizing f cst with
  | refl =>
      subst hc1
      injection hc2 with _ hs
      subst hs
      exact hf
  | @step c1 cm c2 hs _ ih =>
      subst hc1 hc2
      rcases cm with ⟨fm, cstm⟩
      have hm : cstm.failure ≠ 0 := by
        cases hs with
        | i32Const | i64Const | localGet _ | localSet _ | i64Add | i64Sub | i64Mul
        | callDefined _ _ _ | returnEnd _ _ | returnInstr _ _ => exact hf
        | callPushTag _ => rw [concretePushTag_failed _ _ _ hf]; exact hf
        | callPopKind _ => rw [concretePopKind_failed _ _ hf]; exact hf
        | callEnqueue _ => rw [concreteEnqueue_failed _ _ _ hf]; exact hf
        | callRuntimeTick _ => rw [concreteRuntimeTick_failed _ hf]; exact hf
      exact ih hm rfl rfl

private theorem linearDispatchRun_preserves_failure_ne {env tbl cst cst'}
    (h : LinearDispatchRun env tbl cst cst') (hf : cst.failure ≠ 0) :
    cst'.failure ≠ 0 := by
  cases h with
  | halted _ => exact hf
  | step hs _ =>
      cases hs with
      | tickQuota h0 _ _ => exact absurd h0 hf
      | invokeOk h0 _ _ _ _ => exact absurd h0 hf
      | invokeMissing h0 _ _ _ => exact absurd h0 hf

/-- End-to-end soundness: any clean concrete `[0, 8192)` linear-memory run of a lowered
    program simulates `NobleContracts.Exec (toMc1Code ops) before after`. -/
theorem linearRunLowered_sound_exec (entry : Nat) (ops : List ArithOp)
    (before : Stack) (cst cstFinal : LinearRuntimeState) (st : RuntimeStore)
    (hrange : EntryRangeValid entry ops)
    (href : StoreRefines cst st)
    (hconts : cst.cp = 0)
    (hrel : StackRel before st.slots)
    (hrun : linearRunLowered runtimeEnv entry ops cst cstFinal)
    (hfok : cstFinal.failure = 0) :
    ∃ after stFinal,
      StoreRefines cstFinal stFinal ∧
      StackRel after stFinal.slots ∧
      NobleContracts.Exec (toMc1Code ops) before after := by
  have hcstOk : cst.failure = 0 := by
    refine Classical.byContradiction fun hfne => ?_
    rcases hrun with ⟨cstEnq, henq, hdisp⟩
    have h1 := linearWasmSteps_preserves_failure_ne henq hfne
    have h2 := linearDispatchRun_preserves_failure_ne hdisp h1
    exact h2 hfok
  have hstOk : st.status = .ok := (storeRefines_failure_eq_zero_iff href).mp hcstOk
  have hstConts : st.conts = [] := by
    rw [← href.contsEq, hconts]
    rfl
  obtain ⟨stFinal, hrunAbs, hrefFinal⟩ :=
    linearRunLowered_refines_runLowered entry ops cst cstFinal st hrun href
  have hfinalOk : stFinal.status = .ok := (storeRefines_failure_eq_zero_iff hrefFinal).mp hfok
  obtain ⟨after, hrelAfter, _, hexec⟩ :=
    runLowered_sound entry ops before st stFinal hrange hstOk hstConts hrel hrunAbs hfinalOk
  exact ⟨after, stFinal, hrefFinal, hrelAfter, hexec⟩

private theorem linear_opSub_steps_ok
    (cst3 cst4 cst5 cstFinal : LinearRuntimeState) (left right : BitVec 64)
    (hpop1 : concretePopKind cst3 tagI64 = (right, cst4))
    (hpop2 : concretePopKind cst4 tagI64 = (left, cst5))
    (hpush : concretePushTag cst5 tagI64 (left - right) = cstFinal) :
    LinearWasmSteps runtimeEnv
      (initLinearConfig [.call .opSub] cst3)
      (doneLinearConfig cstFinal) := by
  unfold initLinearConfig doneLinearConfig
  have hcall : LinearWasmStep runtimeEnv
      ⟨[⟨[], [], [.call .opSub], 0⟩], cst3⟩
      ⟨[⟨[.i64 0, .i64 0], [], defOpSub.body, 0⟩, ⟨[], [], [], 0⟩], cst3⟩ :=
    @LinearWasmStep.callDefined runtimeEnv [] [] [] [] [] 0 [] cst3 .opSub defOpSub rfl rfl rfl
  refine .step hcall ?_
  refine .step .i32Const ?_
  refine .step (by simpa [hpop1] using
    @LinearWasmStep.callPopKind runtimeEnv _ _ _ _ _ cst3 tagI64 rfl) ?_
  refine .step (.localSet (by simp)) ?_
  refine .step .i32Const ?_
  refine .step (by simpa [hpop2] using
    @LinearWasmStep.callPopKind runtimeEnv _ _ _ _ _ cst4 tagI64 rfl) ?_
  refine .step (.localSet (by simp)) ?_
  refine .step (.localGet (by rfl)) ?_
  refine .step (.localGet (by rfl)) ?_
  refine .step .i64Sub ?_
  have hpushCall : LinearWasmStep runtimeEnv
      ⟨⟨[.i64 right, .i64 left], [.i64 (left - right)], [.call .pushI64], 0⟩ ::
        [⟨[], [], [], 0⟩], cst5⟩
      ⟨⟨[.i64 (left - right)], [], defPushI64.body, 0⟩ ::
        ⟨[.i64 right, .i64 left], [], [], 0⟩ :: [⟨[], [], [], 0⟩], cst5⟩ :=
    @LinearWasmStep.callDefined runtimeEnv [.i64 right, .i64 left]
      [.i64 (left - right)] [.i64 (left - right)] [] [] 0 [⟨[], [], [], 0⟩]
      cst5 .pushI64 defPushI64 rfl rfl rfl
  refine .step hpushCall ?_
  refine .step .i32Const ?_
  refine .step (.localGet (by rfl)) ?_
  refine .step (by simpa [hpush] using
    @LinearWasmStep.callPushTag runtimeEnv _ _ _ _ _ cst5 tagI64 (left - right) rfl) ?_
  refine .step (@LinearWasmStep.returnEnd runtimeEnv _ [] [] [] 0 _ _ _ _ _ _ rfl rfl) ?_
  exact .single (@LinearWasmStep.returnEnd runtimeEnv _ [] [] [] 0 _ _ _ _ _ _ rfl rfl)

private theorem linear_ordered_sub_with_tail_aux
    (tailSlots : List TaggedSlot) (cst0 : LinearRuntimeState)
    (hf0 : cst0.failure = 0) (hsp0 : cst0.sp = 4) (hcp0 : cst0.cp = 0)
    (hsteps0 : cst0.steps = 0) (hol0 : cst0.operandLimit = 2048)
    (hcl0 : cst0.continuationLimit = 4096) (hsl0 : cst0.stepLimit = 100000)
    (hslots0 : readSlotsTopFirst cst0.mem 4 = [⟨tagI64, 3⟩, ⟨tagI64, 10⟩] ++ tailSlots) :
    ∃ cstFinal,
      linearRunLowered runtimeEnv 0 [.sub] cst0 cstFinal ∧
      cstFinal.failure = 0 ∧
      cstFinal.cp = 0 ∧
      cstFinal.sp = 3 ∧
      readSlotsTopFirst cstFinal.mem 3 = ⟨tagI64, 7⟩ :: tailSlots ∧
      MemEqOn 0 32 cst0.mem cstFinal.mem := by
  obtain ⟨cst1, hcst1Def⟩ : ∃ x, x = concreteEnqueue cst0 0 0 := ⟨_, rfl⟩
  have henq : LinearWasmSteps runtimeEnv
      (initLinearConfig [.i32Const (BitVec.ofNat 32 0), .i32Const 0, .call .enqueue] cst0)
      (doneLinearConfig cst1) :=
    .step .i32Const (.step .i32Const (.single (by rw [hcst1Def]; exact .callEnqueue rfl)))
  have hcst1Lim : (cst0.cp + 1) * 8 ≤ cst0.continuationLimit := by
    rw [hcp0, hcl0]; decide
  have hcst1Eq : cst1 =
      { cst0 with
        mem := storeI32 (storeI32 cst0.mem 4096 0) 4100 0
        cp := cst0.cp + 1
        continuationPeak := if cst0.continuationPeak < (cst0.cp + 1) * 8 then (cst0.cp + 1) * 8 else cst0.continuationPeak } := by
    have h := concreteEnqueue_ok cst0 0 0 hf0 hcst1Lim
    have haddr : ((4096 : BitVec 32) + cst0.cp * 8).toNat = 4096 := by rw [hcp0]; decide
    dsimp only at h
    rw [haddr] at h
    rw [hcst1Def]
    exact h
  have hcst1f : cst1.failure = 0 := by rw [hcst1Eq]; exact hf0
  have hcst1cp : cst1.cp = 1 := by rw [hcst1Eq]; dsimp only; rw [hcp0]; decide
  have hcst1sp : cst1.sp = 4 := by rw [hcst1Eq]; exact hsp0
  have hcst1ol : cst1.operandLimit = 2048 := by rw [hcst1Eq]; exact hol0
  have hcst1steps : cst1.steps = 0 := by rw [hcst1Eq]; exact hsteps0
  have hcst1sl : cst1.stepLimit = 100000 := by rw [hcst1Eq]; exact hsl0
  have hmem01 : MemEqOn 0 4096 cst0.mem cst1.mem := by
    rw [hcst1Def]
    exact (concreteEnqueue_preserves_opRegion cst0 0 0 (by rw [hcp0]; decide)).1
  obtain ⟨cst2, hcst2Def⟩ : ∃ x, x = (concreteRuntimeTick cst1).2 := ⟨_, rfl⟩
  have htickVal : concreteRuntimeTick cst1 = (1, { cst1 with steps := cst1.steps + 1 }) :=
    concreteRuntimeTick_ok cst1 hcst1f (by rw [hcst1steps, hcst1sl]; decide)
  have hcst2Eq : cst2 = { cst1 with steps := 1 } := by
    rw [hcst2Def, htickVal, hcst1steps]
    rfl
  have htick : LinearWasmSteps runtimeEnv
      ⟨[⟨[], [], [.call .runtimeTick], 1⟩], cst1⟩
      (doneValLinearConfig (.i32 1) cst2) := by
    have ht : LinearWasmStep runtimeEnv
        ⟨[⟨[], [], [.call .runtimeTick], 1⟩], cst1⟩
        (doneValLinearConfig (.i32 (concreteRuntimeTick cst1).1) (concreteRuntimeTick cst1).2) :=
      .callRuntimeTick rfl
    rw [← hcst2Def] at ht
    have h1 : (concreteRuntimeTick cst1).1 = 1 := by rw [htickVal]
    rw [h1] at ht
    exact .single ht
  have hcst2f : cst2.failure = 0 := by rw [hcst2Eq]; exact hcst1f
  have hcst2cp : cst2.cp = 1 := by rw [hcst2Eq]; exact hcst1cp
  have hcst2sp : cst2.sp = 4 := by rw [hcst2Eq]; exact hcst1sp
  have hcst2ol : cst2.operandLimit = 2048 := by rw [hcst2Eq]; exact hcst1ol
  have hmem12 : MemEqOn 0 4096 cst1.mem cst2.mem := by
    rw [hcst2Eq]; exact MemEqOn.refl 0 4096 cst1.mem
  have hdeq : concreteDequeue cst2 =
      ((loadI32 cst2.mem 4096, loadI32 cst2.mem 4100),
       { cst2 with mem := storeI64 cst2.mem 4096 0, cp := 0 }) := by
    have hcpNe : cst2.cp ≠ 0 := by rw [hcst2cp]; decide
    have h := concreteDequeue_ok cst2 hcst2f hcpNe
    have hcpSub : cst2.cp - 1 = 0 := by rw [hcst2cp]; decide
    have haddr : ((4096 : BitVec 32) + (cst2.cp - 1) * 8).toNat = 4096 := by
      rw [hcpSub]; decide
    dsimp only at h
    rw [haddr, hcpSub] at h
    exact h
  obtain ⟨cst3, hcst3Def⟩ : ∃ x, x = (concreteDequeue cst2).2 := ⟨_, rfl⟩
  have hcst3Eq : cst3 = { cst2 with mem := storeI64 cst2.mem 4096 0, cp := 0 } := by
    rw [hcst3Def, hdeq]
  have hcst3f : cst3.failure = 0 := by rw [hcst3Eq]; exact hcst2f
  have hcst3cp : cst3.cp = 0 := by rw [hcst3Eq]
  have hcst3sp : cst3.sp = 4 := by rw [hcst3Eq]; exact hcst2sp
  have hcst3ol : cst3.operandLimit = 2048 := by rw [hcst3Eq]; exact hcst2ol
  have hmem23 : MemEqOn 0 4096 cst2.mem cst3.mem := by
    rw [hcst3Def]
    exact (concreteDequeue_preserves_opRegion cst2 (by rw [hcst2cp]; decide)).1
  have hmem03 : MemEqOn 0 4096 cst0.mem cst3.mem := (hmem01.trans hmem12).trans hmem23
  have hsl03 : readSlotsTopFirst cst3.mem 4 = [⟨tagI64, 3⟩, ⟨tagI64, 10⟩] ++ tailSlots := by
    rw [readSlotsTopFirst_memEqOn hmem03.symm rfl (by omega)]
    exact hslots0
  have hsl03' : readSlot cst3.mem 3 :: readSlotsTopFirst cst3.mem 3 =
      ⟨tagI64, 3⟩ :: (⟨tagI64, 10⟩ :: tailSlots) := hsl03
  injection hsl03' with hread3 hsl3_tail
  have hle3 : (1 : BitVec 32) ≤ cst3.sp := by rw [hcst3sp]; decide
  have haddr3 : ((cst3.sp - 1) * 16).toNat = 3 * 16 := by rw [hcst3sp]; decide
  have htag3 : loadI32 cst3.mem ((cst3.sp - 1) * 16).toNat = tagI64 := by
    rw [haddr3]
    exact congrArg TaggedSlot.tag hread3
  have hpay3 : loadI64 cst3.mem (((cst3.sp - 1) * 16).toNat + 8) = 3 := by
    rw [haddr3]
    exact congrArg TaggedSlot.payload hread3
  obtain ⟨cst4, hcst4Def⟩ : ∃ x, x = (concretePopKind cst3 tagI64).2 := ⟨_, rfl⟩
  have hpop1 : concretePopKind cst3 tagI64 = (3, cst4) := by
    rw [hcst4Def]
    rw [concretePopKind_ok cst3 tagI64 hcst3f hle3 htag3, concreteTake_ok cst3 hcst3f hle3]
    simp only [hpay3]
  have hcst4Eq : cst4 =
      { cst3 with
        mem := storeI64 (storeI64 cst3.mem 48 0) 56 0
        sp := cst3.sp - 1 } := by
    rw [hcst4Def, concretePopKind_ok cst3 tagI64 hcst3f hle3 htag3, concreteTake_ok cst3 hcst3f hle3]
    dsimp only
    rw [haddr3]
  have hcst4sp : cst4.sp = 3 := by rw [hcst4Eq]; dsimp only; rw [hcst3sp]; decide
  have hcst4f : cst4.failure = 0 := by rw [hcst4Eq]; exact hcst3f
  have hcst4cp : cst4.cp = 0 := by rw [hcst4Eq]; exact hcst3cp
  have hcst4ol : cst4.operandLimit = 2048 := by rw [hcst4Eq]; exact hcst3ol
  have hmem34 : MemEqOn 0 48 cst3.mem cst4.mem := by
    have h := concretePopKind_preserves_below cst3 tagI64 (by rw [hcst3sp]; decide)
    have hsp3 : (cst3.sp.toNat - 1) * 16 = 48 := by rw [hcst3sp]; decide
    rw [hsp3, ← hcst4Def] at h
    exact h
  have hsl04 : readSlotsTopFirst cst4.mem 3 = ⟨tagI64, 10⟩ :: tailSlots := by
    rw [readSlotsTopFirst_memEqOn hmem34.symm rfl (by omega)]
    exact hsl3_tail
  have hsl04' : readSlot cst4.mem 2 :: readSlotsTopFirst cst4.mem 2 =
      ⟨tagI64, 10⟩ :: tailSlots := hsl04
  injection hsl04' with hread4 hsl4_tail
  have hle4 : (1 : BitVec 32) ≤ cst4.sp := by rw [hcst4sp]; decide
  have haddr4 : ((cst4.sp - 1) * 16).toNat = 2 * 16 := by rw [hcst4sp]; decide
  have htag4 : loadI32 cst4.mem ((cst4.sp - 1) * 16).toNat = tagI64 := by
    rw [haddr4]
    exact congrArg TaggedSlot.tag hread4
  have hpay4 : loadI64 cst4.mem (((cst4.sp - 1) * 16).toNat + 8) = 10 := by
    rw [haddr4]
    exact congrArg TaggedSlot.payload hread4
  obtain ⟨cst5, hcst5Def⟩ : ∃ x, x = (concretePopKind cst4 tagI64).2 := ⟨_, rfl⟩
  have hpop2 : concretePopKind cst4 tagI64 = (10, cst5) := by
    rw [hcst5Def]
    rw [concretePopKind_ok cst4 tagI64 hcst4f hle4 htag4, concreteTake_ok cst4 hcst4f hle4]
    simp only [hpay4]
  have hcst5Eq : cst5 =
      { cst4 with
        mem := storeI64 (storeI64 cst4.mem 32 0) 40 0
        sp := cst4.sp - 1 } := by
    rw [hcst5Def, concretePopKind_ok cst4 tagI64 hcst4f hle4 htag4, concreteTake_ok cst4 hcst4f hle4]
    dsimp only
    rw [haddr4]
  have hcst5sp : cst5.sp = 2 := by rw [hcst5Eq]; dsimp only; rw [hcst4sp]; decide
  have hcst5f : cst5.failure = 0 := by rw [hcst5Eq]; exact hcst4f
  have hcst5cp : cst5.cp = 0 := by rw [hcst5Eq]; exact hcst4cp
  have hcst5ol : cst5.operandLimit = 2048 := by rw [hcst5Eq]; exact hcst4ol
  have hmem45 : MemEqOn 0 32 cst4.mem cst5.mem := by
    have h := concretePopKind_preserves_below cst4 tagI64 (by rw [hcst4sp]; decide)
    have hsp4 : (cst4.sp.toNat - 1) * 16 = 32 := by rw [hcst4sp]; decide
    rw [hsp4, ← hcst5Def] at h
    exact h
  obtain ⟨cstFinal, hcstFinalDef⟩ : ∃ x, x = concretePushTag cst5 tagI64 ((10 : BitVec 64) - 3) := ⟨_, rfl⟩
  have hpushLim : (cst5.sp + 1) * 16 ≤ cst5.operandLimit := by
    rw [hcst5sp, hcst5ol]; decide
  have hcstFinalEq : cstFinal =
      { cst5 with
        mem := storeI64 (storeI32 cst5.mem (cst5.sp * 16).toNat tagI64)
                 ((cst5.sp * 16).toNat + 8) ((10 : BitVec 64) - 3)
        sp := cst5.sp + 1
        operandPeak := if cst5.operandPeak < (cst5.sp + 1) * 16 then (cst5.sp + 1) * 16 else cst5.operandPeak } := by
    rw [hcstFinalDef]
    exact concretePushTag_ok cst5 tagI64 ((10 : BitVec 64) - 3) hcst5f hpushLim
  have hmem5F : MemEqOn 0 32 cst5.mem cstFinal.mem := by
    have h := concretePushTag_preserves_below cst5 tagI64 ((10 : BitVec 64) - 3) (by rw [hcst5sp]; decide)
    have hsp5 : cst5.sp.toNat * 16 = 32 := by rw [hcst5sp]; decide
    rw [hsp5, ← hcstFinalDef] at h
    exact h
  have hmem0F : MemEqOn 0 32 cst0.mem cstFinal.mem :=
    (((hmem03.mono (by omega) (by omega)).trans (hmem34.mono (by omega) (by omega))).trans hmem45).trans hmem5F
  have hopSub : LinearWasmSteps runtimeEnv
      (initLinearConfig [.call .opSub] cst3)
      (doneLinearConfig cstFinal) :=
    linear_opSub_steps_ok cst3 cst4 cst5 cstFinal 10 3 hpop1 hpop2 hcstFinalDef.symm
  have hfnIdx : (concreteDequeue cst2).1.1 = 0 := by
    rw [hdeq]
    dsimp only
    have h4096 : MemEqOn 4096 4100 cst1.mem cst2.mem := by
      rw [hcst2Eq]; exact MemEqOn.refl _ _ _
    rw [← @loadI32_memEqOn _ _ 4096 4100 4096 h4096 (by omega) (by omega), hcst1Eq]
    dsimp only
    have hpres0 : MemEqOn 4096 4100 (storeI32 cst0.mem 4096 0)
        (storeI32 (storeI32 cst0.mem 4096 0) 4100 0) :=
      storeI32_preserves_outside (0 : BitVec 32) (Or.inr (by omega))
    rw [loadI32_memEqOn hpres0.symm (Nat.le_refl _) (Nat.le_refl _)]
    exact loadI32_storeI32 cst0.mem 4096 0
  have htbl : lowerProgram 0 [.sub] (concreteDequeue cst2).1.1.toNat = some ⟨none, lowerOp .sub⟩ := by
    rw [hfnIdx]; simp [lowerProgram]
  have hblock : LinearExecContBlock runtimeEnv ⟨none, lowerOp .sub⟩
      (concreteDequeue cst2).1.2 (concreteDequeue cst2).2 cstFinal :=
    .noNext (by rw [← hcst3Def]; exact hcst3f) rfl (by rw [← hcst3Def]; exact hopSub)
  have hdisp : LinearDispatchRun runtimeEnv (lowerProgram 0 [.sub]) cst1 cstFinal :=
    .step (.invokeOk hcst1f (by rw [hcst1cp]; decide) htick htbl hblock)
      (.halted (Or.inr (by rw [hcstFinalEq]; exact hcst5cp)))
  have hslFinal : readSlotsTopFirst cstFinal.mem 3 = ⟨tagI64, 7⟩ :: tailSlots := by
    change readSlot cstFinal.mem 2 :: readSlotsTopFirst cstFinal.mem 2 = ⟨tagI64, 7⟩ :: tailSlots
    rw [List.cons.injEq]
    refine ⟨?_, ?_⟩
    · unfold readSlot
      have hsubEq : (10 : BitVec 64) - 3 = 7 := by decide
      have haddr5 : (cst5.sp * 16).toNat = 32 := by rw [hcst5sp]; decide
      rw [hcstFinalEq, haddr5, hsubEq]
      have hk : loadI32 (storeI64 (storeI32 cst5.mem 32 tagI64) 40 7) 32 = tagI64 := by
        have hpres0 : MemEqOn 32 36 (storeI32 cst5.mem 32 tagI64)
            (storeI64 (storeI32 cst5.mem 32 tagI64) 40 7) :=
          storeI64_preserves_outside (7 : BitVec 64) (Or.inr (by omega))
        rw [loadI32_memEqOn hpres0.symm (Nat.le_refl _) (Nat.le_refl _)]
        exact loadI32_storeI32 cst5.mem 32 tagI64
      have hv : loadI64 (storeI64 (storeI32 cst5.mem 32 tagI64) 40 7) 40 = 7 :=
        loadI64_storeI64 (storeI32 cst5.mem 32 tagI64) 40 7
      dsimp only
      rw [hk, hv]
    · rw [readSlotsTopFirst_memEqOn (hmem45.trans hmem5F).symm rfl (by omega)]
      exact hsl4_tail
  refine ⟨cstFinal, ⟨cst1, henq, hdisp⟩, ?_, ?_, ?_, hslFinal, hmem0F⟩
  · rw [hcstFinalEq]; exact hcst5f
  · rw [hcstFinalEq]; exact hcst5cp
  · rw [hcstFinalEq]; dsimp only; rw [hcst5sp]; decide

/-- Concrete non-vacuity witness on `[0, 8192)` linear memory: ordered subtraction
    `[ - ]` on `[.i64 10, .i64 3]` above nonempty mixed tail `[.bool true, .i64 99]`
    preserves `[0, 32)` tail bytes and computes `10 - 3 = 7`. -/
theorem linear_ordered_sub_with_tail_witness :
    let tail : Stack := [.bool true, .i64 99]
    let tailSlots : List TaggedSlot := [⟨tagI64, 99⟩, ⟨tagBool, 1⟩]
    let inpSlots : List TaggedSlot := [⟨tagI64, 3⟩, ⟨tagI64, 10⟩]
    let initMem := writeSlotsTopFirst zeroMem (inpSlots ++ tailSlots)
    let cst0 : LinearRuntimeState :=
      { mem := initMem
        sp := 4
        cp := 0
        failure := 0
        quotaReason := 0
        phase := execPhase
        steps := 0
        operandLimit := 2048
        continuationLimit := 4096
        stepLimit := 100000
        operandPeak := 64
        continuationPeak := 0 }
    ∃ cstFinal,
      linearRunLowered runtimeEnv 0 [.sub] cst0 cstFinal ∧
      cstFinal.failure = 0 ∧
      cstFinal.cp = 0 ∧
      cstFinal.sp = 3 ∧
      readSlotsTopFirst cstFinal.mem 3 = ⟨tagI64, 7⟩ :: tailSlots ∧
      MemEqOn 0 32 cst0.mem cstFinal.mem ∧
      NobleContracts.Exec (toMc1Code [.sub]) (tail ++ [.i64 10, .i64 3]) (tail ++ [.i64 7]) := by
  intro tail tailSlots inpSlots initMem cst0
  have hsl0 : readSlotsTopFirst cst0.mem 4 = [⟨tagI64, 3⟩, ⟨tagI64, 10⟩] ++ tailSlots :=
    readSlotsTopFirst_writeSlotsTopFirst zeroMem (inpSlots ++ tailSlots)
  obtain ⟨cstFinal, hrun, hf, hcp, hsp, hsl, hmem⟩ :=
    linear_ordered_sub_with_tail_aux tailSlots cst0 rfl rfl rfl rfl rfl rfl rfl hsl0
  have hexecTail :
      NobleContracts.Exec (toMc1Code [.sub]) (tail ++ [.i64 10, .i64 3]) (tail ++ [.i64 7]) := by
    change NobleContracts.Exec [.word 5] (tail ++ [.i64 10, .i64 3]) (tail ++ [.i64 7])
    rw [exec_sub_iff, (by decide : (10 : BitVec 64) - 3 = 7)]
  exact ⟨cstFinal, hrun, hf, hcp, hsp, hsl, hmem, hexecTail⟩

end NobleContracts.WasmStep
