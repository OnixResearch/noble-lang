import NobleFoundation.Effects
import Mathlib.Data.List.Perm.Basic
import Mathlib.Data.List.Nodup

/-!
# Ownership model with aggregates, named definitions, case forms and a heap

This is a second small-step model for `ownership-lifetime-contract`. It does
not replace `NobleFoundation/Effects.lean`; it imports it for the labels,
responses, statuses and events, and extends the language that `Effects`
models:

* **aggregates**: `Pair<A,B>`, `Sum<A,B>` and `List<A>` construction and
  elimination (`pair`, `unpair`, `inl`, `inr`, `cases`, `nil`, `cons`,
  `listCase`). Aggregates may contain resources; capture eligibility is
  recursive, so a resource-bearing aggregate is not `Copy`.
* **named definitions**: a definition environment `D` of exact-interface
  bodies, applied with `call i`. A call's body is typed at the definition's
  exact input and output shapes under the definition's own effect bound.
  A cyclic definition has no finite typing derivation.
* **case forms**: `cases` runs one arm on the payload of a sum and
  `listCase` runs one arm on a list's head and tail or on `nil`. Both arms
  share one interface, so every incoming linear claim is accounted for by
  either arm (K-CONTROL-01).
* **a heap**: `alloc` moves a copyable value into a fresh cell and returns a
  cell reference; the cell is retained until the end of the modelled
  execution. Cells are immutable, shared and addressed by identity, and
  every reference to a cell must name an allocated cell.

The file also adds a **frame pin** to `run`: the running Program value is
held in a `pin` continuation frame until its body finishes, so the package's
storage claims are counted while the body runs (design row `run`). `dip`
keeps its `restore` frame.

Claims proved here, about this model only:

* `owners_step`: OWN-PO-1 over the extended language. Every typed step
  permutes the whole-configuration owner multiset (stack values, `restore`
  and `pin` frames, and heap cells) and keeps the live set and fresh-id
  counter, except a permitted `acquire` (adds the fresh id) and a permitted
  `release` (consumes the released id). Aggregates, both case forms, named
  calls, allocation, `run`'s pin and `dip`'s restore are all covered.
* `own_preserve`, `own_reach`: ownership is preserved through every finite
  prefix of a typed program.
* `heap_safe_step`, `heap_safe_reach`: every cell reference in code, frames,
  the stack or the heap names an allocated cell; allocation is fresh and
  allocation ids stay distinct; no step removes an allocated cell. Storage
  reclamation is deliberately a backend property here (design: "Storage
  reclamation timing is a backend property"), so the model proves the
  no-dangling half, which is OWN-PO-5's safety direction.
* `typed_rfree`, `typed_programs_rfree`: typed code and heap cells are
  resource-free at every nesting depth, and every program value anywhere on
  the stack has a resource-free body. `quote`/`alloc` only capture `Copy`
  values, and `Copy` recursively excludes resources (OWN-PO-3).
* `reachable_sound`: a typed program's every finite prefix is well typed,
  ownership-preserving, heap-safe and capture-free.

Not modelled, and explicitly outside this model's boundary:

* storage reclamation, reference counting and collection (OWN-PO-5 beyond
  the no-dangling invariant);
* heap reads: cells are write-only retention here, because reading a cell
  would put its value back on the operand stack and require a heap typing
  environment; value flow through the heap is not part of the ownership
  question this model covers;
* borrowing, the `borrow` word and scope tags (the owner selected no borrow
  profile);
* abnormal cleanup (OWN-PO-6), lowering/runtime/host-ledger correspondence
  (OWN-PO-4..7) and any checker refinement. Every theorem here is a model
  claim.
-/

namespace NobleFoundation.OwnershipModel

open NobleFoundation.Effects (Label Response Status Event)

set_option linter.defProp false

/-! ## Types and recursive eligibility -/

/-- The type language: I64, a move-only resource, unit, a heap cell
reference, aggregates and a Program with its input shape, output shape and
latent effect bound. -/
inductive Ty where
  | i64
  | res
  | unit
  | cell
  | pair : Ty → Ty → Ty
  | sum : Ty → Ty → Ty
  | list : Ty → Ty
  | prog : List Ty → List Ty → List Label → Ty

/-- Copyable types may be duplicated, discarded, captured and stored in a
heap cell. A resource is not copyable, and eligibility is recursive through
aggregates, so no wrapping depth makes a resource copyable. -/
def Ty.Copy : Ty → Prop
  | .i64 => True
  | .res => False
  | .unit => True
  | .cell => True
  | .pair a b => a.Copy ∧ b.Copy
  | .sum a b => a.Copy ∧ b.Copy
  | .list a => a.Copy
  | .prog _ _ _ => True

/-! ## Values and instructions -/

mutual
  /-- A value: I64, a resource token, unit, a heap cell reference, the
  aggregate constructors, and a Program value whose capture graph is its
  body (a `push` of the captured value). -/
  inductive Value where
    | i64 : BitVec 64 → Value
    | res : Nat → Value
    | unit : Value
    | cell : Nat → Value
    | pair : Value → Value → Value
    | inl : Value → Value
    | inr : Value → Value
    | nil : Value
    | cons : Value → Value → Value
    | prog : List Instr → Value

  /-- One instruction. `pin` and `restore` are run-time continuation
  entries; `CodeT` has no rule for either, so no source program can forge
  one. -/
  inductive Instr where
    | push : Value → Instr
    | add | dup | drop | swap | quote | compose | run
    | block : List Instr → Instr
    | emit | acquire | release
    | dip | ifte
    | pair | unpair
    | inl | inr
    | cases : List Instr → List Instr → Instr
    | nil | cons
    | listCase : List Instr → List Instr → Instr
    | alloc
    | call : Nat → Instr
    | restore : Value → Instr
    | pin : Value → Instr
end

/-! ## Named definitions -/

/-- One named definition: exact input and output shapes, its own effect
bound and its body. `call` requires the body to be typed at exactly these
shapes. -/
structure Def where
  input : List Ty
  output : List Ty
  bound : List Label
  body : List Instr

/-- Bounded definition lookup. Indexing is by position in the table. -/
def getDef : List Def → Nat → Option Def
  | [], _ => none
  | d :: _, 0 => some d
  | _ :: ds, n + 1 => getDef ds n

/-- A configuration. `cells` is the host-tracked set of allocated heap
cells; `heap` maps a cell id to its immutable content; `next` is the shared
fresh-id counter for resources and cells. -/
structure Config where
  code : List Instr
  stack : List Value
  live : List Nat
  cells : List Nat
  heap : List (Nat × Value)
  next : Nat
  trace : List Event
  status : Status

/-! ## Small-step semantics -/

/-- Small-step semantics under a fixed definition table and host oracle.
Only `running` configurations step. `run` pins the executed Program value in
a `pin` frame until its body reaches it; `alloc` moves a value into a fresh
cell. -/
inductive Step (D : List Def) (H : Nat → Response) : Config → Config → Prop where
  | push {v k s l cs hp n tr} :
      Step D H ⟨.push v :: k, s, l, cs, hp, n, tr, .running⟩
        ⟨k, v :: s, l, cs, hp, n, tr, .running⟩
  | add {a b k s l cs hp n tr} :
      Step D H ⟨.add :: k, .i64 b :: .i64 a :: s, l, cs, hp, n, tr, .running⟩
        ⟨k, .i64 (a + b) :: s, l, cs, hp, n, tr, .running⟩
  | dup {v k s l cs hp n tr} :
      Step D H ⟨.dup :: k, v :: s, l, cs, hp, n, tr, .running⟩
        ⟨k, v :: v :: s, l, cs, hp, n, tr, .running⟩
  | drop {v k s l cs hp n tr} :
      Step D H ⟨.drop :: k, v :: s, l, cs, hp, n, tr, .running⟩
        ⟨k, s, l, cs, hp, n, tr, .running⟩
  | swap {a b k s l cs hp n tr} :
      Step D H ⟨.swap :: k, b :: a :: s, l, cs, hp, n, tr, .running⟩
        ⟨k, a :: b :: s, l, cs, hp, n, tr, .running⟩
  | quote {v k s l cs hp n tr} :
      Step D H ⟨.quote :: k, v :: s, l, cs, hp, n, tr, .running⟩
        ⟨k, .prog [.push v] :: s, l, cs, hp, n, tr, .running⟩
  | block {p k s l cs hp n tr} :
      Step D H ⟨.block p :: k, s, l, cs, hp, n, tr, .running⟩
        ⟨k, .prog p :: s, l, cs, hp, n, tr, .running⟩
  | compose {p q k s l cs hp n tr} :
      Step D H ⟨.compose :: k, .prog q :: .prog p :: s, l, cs, hp, n, tr, .running⟩
        ⟨k, .prog (p ++ q) :: s, l, cs, hp, n, tr, .running⟩
  | run {p k s l cs hp n tr} :
      Step D H ⟨.run :: k, .prog p :: s, l, cs, hp, n, tr, .running⟩
        ⟨p ++ .pin (.prog p) :: k, s, l, cs, hp, n, tr, .running⟩
  | pin {v k s l cs hp n tr} :
      Step D H ⟨.pin v :: k, s, l, cs, hp, n, tr, .running⟩
        ⟨k, s, l, cs, hp, n, tr, .running⟩
  | dip {p v k s l cs hp n tr} :
      Step D H ⟨.dip :: k, .prog p :: v :: s, l, cs, hp, n, tr, .running⟩
        ⟨p ++ .restore v :: k, s, l, cs, hp, n, tr, .running⟩
  | restore {v k s l cs hp n tr} :
      Step D H ⟨.restore v :: k, s, l, cs, hp, n, tr, .running⟩
        ⟨k, v :: s, l, cs, hp, n, tr, .running⟩
  | ifTrue {p q x k s l cs hp n tr} : x ≠ 0 →
      Step D H ⟨.ifte :: k, .prog q :: .prog p :: .i64 x :: s, l, cs, hp, n, tr, .running⟩
        ⟨p ++ k, s, l, cs, hp, n, tr, .running⟩
  | ifFalse {p q x k s l cs hp n tr} : x = 0 →
      Step D H ⟨.ifte :: k, .prog q :: .prog p :: .i64 x :: s, l, cs, hp, n, tr, .running⟩
        ⟨q ++ k, s, l, cs, hp, n, tr, .running⟩
  | pair {a b k s l cs hp n tr} :
      Step D H ⟨.pair :: k, a :: b :: s, l, cs, hp, n, tr, .running⟩
        ⟨k, .pair a b :: s, l, cs, hp, n, tr, .running⟩
  | unpair {a b k s l cs hp n tr} :
      Step D H ⟨.unpair :: k, .pair a b :: s, l, cs, hp, n, tr, .running⟩
        ⟨k, a :: b :: s, l, cs, hp, n, tr, .running⟩
  | inl {v k s l cs hp n tr} :
      Step D H ⟨.inl :: k, v :: s, l, cs, hp, n, tr, .running⟩
        ⟨k, .inl v :: s, l, cs, hp, n, tr, .running⟩
  | inr {v k s l cs hp n tr} :
      Step D H ⟨.inr :: k, v :: s, l, cs, hp, n, tr, .running⟩
        ⟨k, .inr v :: s, l, cs, hp, n, tr, .running⟩
  | casesLeft {p q v k s l cs hp n tr} :
      Step D H ⟨.cases p q :: k, .inl v :: s, l, cs, hp, n, tr, .running⟩
        ⟨p ++ k, v :: s, l, cs, hp, n, tr, .running⟩
  | casesRight {p q v k s l cs hp n tr} :
      Step D H ⟨.cases p q :: k, .inr v :: s, l, cs, hp, n, tr, .running⟩
        ⟨q ++ k, v :: s, l, cs, hp, n, tr, .running⟩
  | nil {k s l cs hp n tr} :
      Step D H ⟨.nil :: k, s, l, cs, hp, n, tr, .running⟩
        ⟨k, .nil :: s, l, cs, hp, n, tr, .running⟩
  | cons {x xs k s l cs hp n tr} :
      Step D H ⟨.cons :: k, x :: xs :: s, l, cs, hp, n, tr, .running⟩
        ⟨k, .cons x xs :: s, l, cs, hp, n, tr, .running⟩
  | listCaseNil {p q k s l cs hp n tr} :
      Step D H ⟨.listCase p q :: k, .nil :: s, l, cs, hp, n, tr, .running⟩
        ⟨q ++ k, s, l, cs, hp, n, tr, .running⟩
  | listCaseCons {p q x xs k s l cs hp n tr} :
      Step D H ⟨.listCase p q :: k, .cons x xs :: s, l, cs, hp, n, tr, .running⟩
        ⟨p ++ k, x :: xs :: s, l, cs, hp, n, tr, .running⟩
  | alloc {v k s l cs hp n tr} :
      Step D H ⟨.alloc :: k, v :: s, l, cs, hp, n, tr, .running⟩
        ⟨k, .cell n :: s, l, n :: cs, (n, v) :: hp, n + 1, tr, .running⟩
  | call {i d k s l cs hp n tr} (hd : getDef D i = some d) :
      Step D H ⟨.call i :: k, s, l, cs, hp, n, tr, .running⟩
        ⟨d.body ++ k, s, l, cs, hp, n, tr, .running⟩
  | emitOk {x k s l cs hp n tr} : H tr.length = .permitted →
      Step D H ⟨.emit :: k, .i64 x :: s, l, cs, hp, n, tr, .running⟩
        ⟨k, s, l, cs, hp, n, tr ++ [⟨.emit, x.toNat, .permitted⟩], .running⟩
  | emitStop {x k s l cs hp n tr r} : H tr.length = r → r ≠ .permitted →
      Step D H ⟨.emit :: k, .i64 x :: s, l, cs, hp, n, tr, .running⟩
        ⟨.emit :: k, .i64 x :: s, l, cs, hp, n, tr ++ [⟨.emit, x.toNat, r⟩], r.halt⟩
  | acquireOk {k s l cs hp n tr} : H tr.length = .permitted →
      Step D H ⟨.acquire :: k, s, l, cs, hp, n, tr, .running⟩
        ⟨k, .res n :: s, n :: l, cs, hp, n + 1, tr ++ [⟨.acquire, n, .permitted⟩], .running⟩
  | acquireStop {k s l cs hp n tr r} : H tr.length = r → r ≠ .permitted →
      Step D H ⟨.acquire :: k, s, l, cs, hp, n, tr, .running⟩
        ⟨.acquire :: k, s, l, cs, hp, n, tr ++ [⟨.acquire, n, r⟩], r.halt⟩
  | releaseOk {m k s l cs hp n tr} : H tr.length = .permitted →
      Step D H ⟨.release :: k, .res m :: s, l, cs, hp, n, tr, .running⟩
        ⟨k, s, l.erase m, cs, hp, n, tr ++ [⟨.release, m, .permitted⟩], .running⟩
  | releaseStop {m k s l cs hp n tr r} : H tr.length = r → r ≠ .permitted →
      Step D H ⟨.release :: k, .res m :: s, l, cs, hp, n, tr, .running⟩
        ⟨.release :: k, .res m :: s, l, cs, hp, n, tr ++ [⟨.release, m, r⟩], r.halt⟩

/-- Finite execution prefixes. -/
abbrev Reach (D : List Def) (H : Nat → Response) := Relation.ReflTransGen (Step D H)

/-! ## Typing -/

mutual
  /-- Value typing. The cell reference type is opaque; allocation backs it
  at run time and `HeapSafe` is the invariant that every reference is
  backed. A Program's own latent bound is part of its type. -/
  inductive ValT (D : List Def) : Value → Ty → Prop where
    | i64 {x} : ValT D (.i64 x) .i64
    | res {n} : ValT D (.res n) .res
    | unit : ValT D .unit .unit
    | cell {n} : ValT D (.cell n) .cell
    | pair {a b x y} : ValT D a x → ValT D b y → ValT D (.pair a b) (.pair x y)
    | inl {v x y} : ValT D v x → ValT D (.inl v) (.sum x y)
    | inr {v x y} : ValT D v y → ValT D (.inr v) (.sum x y)
    | nil {x} : ValT D .nil (.list x)
    | cons {v vs x} : ValT D v x → ValT D vs (.list x) → ValT D (.cons v vs) (.list x)
    | prog {p i o e} : CodeT D e p i o → ValT D (.prog p) (.prog i o e)

  /-- Instruction typing under effect bound `E`, top-first. `pin` and
  `restore` have no rule here: they are continuation frames only. -/
  inductive InstrT (D : List Def) : List Label → Instr → List Ty → List Ty → Prop where
    | push {E v t s} : ValT D v t → t.Copy → InstrT D E (.push v) s (t :: s)
    | add {E s} : InstrT D E .add (.i64 :: .i64 :: s) (.i64 :: s)
    | dup {E t s} : t.Copy → InstrT D E .dup (t :: s) (t :: t :: s)
    | drop {E t s} : t.Copy → InstrT D E .drop (t :: s) s
    | swap {E a b s} : InstrT D E .swap (b :: a :: s) (a :: b :: s)
    | quote {E t s i} : t.Copy → InstrT D E .quote (t :: s) (.prog i (t :: i) [] :: s)
    | block {E p i o e s} : CodeT D e p i o → InstrT D E (.block p) s (.prog i o e :: s)
    | compose {E i m o e₁ e₂ s} :
        InstrT D E .compose (.prog m o e₂ :: .prog i m e₁ :: s) (.prog i o (e₁ ++ e₂) :: s)
    | run {E s t e} : e ⊆ E → InstrT D E .run (.prog s t e :: s) t
    | dip {E s u t e} : e ⊆ E → InstrT D E .dip (.prog s u e :: t :: s) (t :: u)
    | ifte {E s t e₁ e₂} : e₁ ⊆ E → e₂ ⊆ E →
        InstrT D E .ifte (.prog s t e₂ :: .prog s t e₁ :: .i64 :: s) t
    | emit {E s} : Label.emit ∈ E → InstrT D E .emit (.i64 :: s) s
    | acquire {E s} : Label.acquire ∈ E → InstrT D E .acquire s (.res :: s)
    | release {E s} : Label.release ∈ E → InstrT D E .release (.res :: s) s
    | pair {E a b s} : InstrT D E .pair (a :: b :: s) (.pair a b :: s)
    | unpair {E a b s} : InstrT D E .unpair (.pair a b :: s) (a :: b :: s)
    | inl {E a b s} : InstrT D E .inl (a :: s) (.sum a b :: s)
    | inr {E a b s} : InstrT D E .inr (b :: s) (.sum a b :: s)
    | cases {E p q a b s t e} : e ⊆ E → CodeT D e p (a :: s) t → CodeT D e q (b :: s) t →
        InstrT D E (.cases p q) (.sum a b :: s) t
    | nil {E a s} : InstrT D E .nil s (.list a :: s)
    | cons {E a s} : InstrT D E .cons (a :: .list a :: s) (.list a :: s)
    | listCase {E p q a s t e} : e ⊆ E → CodeT D e p (a :: .list a :: s) t →
        CodeT D e q s t → InstrT D E (.listCase p q) (.list a :: s) t
    | alloc {E t s} : t.Copy → InstrT D E .alloc (t :: s) (.cell :: s)
    | call {E i d} (hd : getDef D i = some d) (hb : d.bound ⊆ E) :
        CodeT D d.bound d.body d.input d.output → InstrT D E (.call i) d.input d.output

  /-- Code typing: a straight-line sequence with one interface. -/
  inductive CodeT (D : List Def) : List Label → List Instr → List Ty → List Ty → Prop where
    | nil {E s} : CodeT D E [] s s
    | cons {E op k s m t} : InstrT D E op s m → CodeT D E k m t → CodeT D E (op :: k) s t
end

inductive StackT (D : List Def) : List Value → List Ty → Prop where
  | nil : StackT D [] []
  | cons {v vs t ts} : ValT D v t → StackT D vs ts → StackT D (v :: vs) (t :: ts)

/-- Continuation typing: code plus the `restore` and `pin` frames that `dip`
and `run` push at run time. A `restore` frame may hold any typed value, a
resource or an aggregate containing one included. A `pin` frame holds only
a Program value, so it can never hide a resource. -/
inductive KontT (D : List Def) (E : List Label) : List Instr → List Ty → List Ty → Prop where
  | nil {s} : KontT D E [] s s
  | cons {op k s m t} : InstrT D E op s m → KontT D E k m t → KontT D E (op :: k) s t
  | restore {v k s t u} : ValT D v t → KontT D E k (t :: s) u → KontT D E (.restore v :: k) s u
  | pin {p i o e k s t} : ValT D (.prog p) (.prog i o e) → KontT D E k s t →
      KontT D E (.pin (.prog p) :: k) s t

/-- Heap typing: every retained cell holds a copyable (hence resource-free)
value. -/
def HeapT (D : List Def) (heap : List (Nat × Value)) : Prop :=
  ∀ entry ∈ heap, ∃ t, ValT D entry.2 t ∧ t.Copy

/-- A well-typed configuration under `D` and `E`: a typed stack, a typed
continuation (frames included) and a typed heap. -/
def WT (D : List Def) (E : List Label) (c : Config) : Prop :=
  ∃ ts out, StackT D c.stack ts ∧ KontT D E c.code ts out ∧ HeapT D c.heap

/-! ## Structural lemmas -/

theorem InstrT.weaken {D : List Def} {E E' : List Label} {op : Instr} {s t}
    (h : InstrT D E op s t)
    (hE : E ⊆ E') : InstrT D E' op s t := by
  cases h with
  | push hv hc => exact .push hv hc
  | add => exact .add
  | dup hc => exact .dup hc
  | drop hc => exact .drop hc
  | swap => exact .swap
  | quote hc => exact .quote hc
  | block hc => exact .block hc
  | compose => exact .compose
  | run he => exact .run (List.Subset.trans he hE)
  | dip he => exact .dip (List.Subset.trans he hE)
  | ifte he₁ he₂ => exact .ifte (List.Subset.trans he₁ hE) (List.Subset.trans he₂ hE)
  | emit h => exact .emit (hE h)
  | acquire h => exact .acquire (hE h)
  | release h => exact .release (hE h)
  | pair => exact .pair
  | unpair => exact .unpair
  | inl => exact .inl
  | inr => exact .inr
  | cases he hp hq => exact .cases (List.Subset.trans he hE) hp hq
  | nil => exact .nil
  | cons => exact .cons
  | listCase he hp hq => exact .listCase (List.Subset.trans he hE) hp hq
  | alloc hc => exact .alloc hc
  | call hd hb hc => exact .call hd (List.Subset.trans hb hE) hc

theorem CodeT.weaken {D : List Def} {E E' : List Label} {p : List Instr} {s t}
    (h : CodeT D E p s t) (hE : E ⊆ E') : CodeT D E' p s t := by
  induction p generalizing s with
  | nil => cases h; exact .nil
  | cons op k ih =>
      cases h with
      | cons hop hk => exact .cons (hop.weaken hE) (ih hk)

theorem CodeT.append {D : List Def} {E : List Label} {p q : List Instr} {s m t}
    (hp : CodeT D E p s m) (hq : CodeT D E q m t) : CodeT D E (p ++ q) s t := by
  induction p generalizing s with
  | nil => cases hp; exact hq
  | cons op k ih =>
      cases hp with
      | cons hop hk => exact .cons hop (ih hk)

/-- Source code is a continuation without frames. -/
theorem CodeT.kont {D : List Def} {E : List Label} {p : List Instr} {s t} (h : CodeT D E p s t) :
    KontT D E p s t := by
  induction p generalizing s with
  | nil => cases h; exact .nil
  | cons op k ih =>
      cases h with
      | cons hop hk => exact .cons hop (ih hk)

/-- Prepending a typed body to a typed continuation, as `run`, `dip`, both
case forms and both branches do. -/
theorem CodeT.appendKont {D : List Def} {E : List Label} {p k : List Instr} {s m t}
    (hp : CodeT D E p s m) (hk : KontT D E k m t) : KontT D E (p ++ k) s t := by
  induction p generalizing s with
  | nil => cases hp; exact hk
  | cons op rest ih =>
      cases hp with
      | cons hop hrest => exact .cons hop (ih hrest)

/-! ## Preservation and progress -/

/-- **Preservation** for the extended language: every step, including a
halting host step, keeps the configuration well typed under the same
definitions and effect bound. -/
theorem preserve {D H E c c'} (hw : WT D E c) (h : Step D H c c') : WT D E c' := by
  obtain ⟨ts, out, hs, hc, hh⟩ := hw
  cases h with
  | push =>
      cases hc with
      | cons hop hk => cases hop with
        | push hv _ => exact ⟨_, _, .cons hv hs, hk, hh⟩
  | add =>
      cases hc with
      | cons hop hk => cases hop with
        | add => cases hs with
          | cons _ hs1 => cases hs1 with
            | cons _ hs2 => exact ⟨_, _, .cons .i64 hs2, hk, hh⟩
  | dup =>
      cases hc with
      | cons hop hk => cases hop with
        | dup _ => cases hs with
          | cons hv hs1 => exact ⟨_, _, .cons hv (.cons hv hs1), hk, hh⟩
  | drop =>
      cases hc with
      | cons hop hk => cases hop with
        | drop _ => cases hs with
          | cons _ hs1 => exact ⟨_, _, hs1, hk, hh⟩
  | swap =>
      cases hc with
      | cons hop hk => cases hop with
        | swap => cases hs with
          | cons hb hs1 => cases hs1 with
            | cons ha hs2 => exact ⟨_, _, .cons ha (.cons hb hs2), hk, hh⟩
  | quote =>
      cases hc with
      | cons hop hk => cases hop with
        | quote hcopy => cases hs with
          | cons hv hs1 =>
              exact ⟨_, _, .cons (.prog (.cons (.push hv hcopy) .nil)) hs1, hk, hh⟩
  | block =>
      cases hc with
      | cons hop hk => cases hop with
        | block hp => exact ⟨_, _, .cons (.prog hp) hs, hk, hh⟩
  | compose =>
      cases hc with
      | cons hop hk => cases hop with
        | compose => cases hs with
          | cons hq hs1 => cases hs1 with
            | cons hp hs2 =>
                cases hq with
                | prog cq => cases hp with
                  | prog cp =>
                      refine ⟨_, _, .cons (.prog ?_) hs2, hk, hh⟩
                      exact (cp.weaken (List.subset_append_left _ _)).append
                        (cq.weaken (List.subset_append_right _ _))
  | run =>
      cases hc with
      | cons hop hk => cases hop with
        | run he => cases hs with
          | cons hv hs1 => cases hv with
            | prog cp =>
                exact ⟨_, _, hs1, (cp.weaken he).appendKont (.pin (.prog cp) hk), hh⟩
  | pin =>
      cases hc with
      | cons hop _ => cases hop
      | pin hv hk => exact ⟨_, _, hs, hk, hh⟩
  | dip =>
      cases hc with
      | cons hop hk => cases hop with
        | dip he => cases hs with
          | cons hp hs1 => cases hs1 with
            | cons hv hs2 => cases hp with
              | prog cp => exact ⟨_, _, hs2, (cp.weaken he).appendKont (.restore hv hk), hh⟩
  | restore =>
      cases hc with
      | cons hop _ => cases hop
      | restore hv hk => exact ⟨_, _, .cons hv hs, hk, hh⟩
  | ifTrue =>
      cases hc with
      | cons hop hk => cases hop with
        | ifte he₁ _ => cases hs with
          | cons _ hs1 => cases hs1 with
            | cons hp hs2 => cases hs2 with
              | cons _ hs3 => cases hp with
                | prog cp => exact ⟨_, _, hs3, (cp.weaken he₁).appendKont hk, hh⟩
  | ifFalse =>
      cases hc with
      | cons hop hk => cases hop with
        | ifte _ he₂ => cases hs with
          | cons hq hs1 => cases hs1 with
            | cons _ hs2 => cases hs2 with
              | cons _ hs3 => cases hq with
                | prog cq => exact ⟨_, _, hs3, (cq.weaken he₂).appendKont hk, hh⟩
  | pair =>
      cases hc with
      | cons hop hk => cases hop with
        | pair => cases hs with
          | cons hv hs1 => cases hs1 with
            | cons hw hs2 => exact ⟨_, _, .cons (.pair hv hw) hs2, hk, hh⟩
  | unpair =>
      cases hc with
      | cons hop hk => cases hop with
        | unpair => cases hs with
          | cons hv hs1 => cases hv with
            | pair ha hb => exact ⟨_, _, .cons ha (.cons hb hs1), hk, hh⟩
  | inl =>
      cases hc with
      | cons hop hk => cases hop with
        | inl => cases hs with
          | cons hv hs1 => exact ⟨_, _, .cons (.inl hv) hs1, hk, hh⟩
  | inr =>
      cases hc with
      | cons hop hk => cases hop with
        | inr => cases hs with
          | cons hv hs1 => exact ⟨_, _, .cons (.inr hv) hs1, hk, hh⟩
  | casesLeft =>
      cases hc with
      | cons hop hk => cases hop with
        | cases he hp _ => cases hs with
          | cons hv hs1 => cases hv with
            | inl hv' => exact ⟨_, _, .cons hv' hs1, (hp.weaken he).appendKont hk, hh⟩
  | casesRight =>
      cases hc with
      | cons hop hk => cases hop with
        | cases he _ hq => cases hs with
          | cons hv hs1 => cases hv with
            | inr hv' => exact ⟨_, _, .cons hv' hs1, (hq.weaken he).appendKont hk, hh⟩
  | nil =>
      cases hc with
      | cons hop hk => cases hop with
        | nil => exact ⟨_, _, .cons .nil hs, hk, hh⟩
  | cons =>
      cases hc with
      | cons hop hk => cases hop with
        | cons => cases hs with
          | cons hx hs1 => cases hs1 with
            | cons hl hs2 => exact ⟨_, _, .cons (.cons hx hl) hs2, hk, hh⟩
  | listCaseNil =>
      cases hc with
      | cons hop hk => cases hop with
        | listCase he _ hq => cases hs with
          | cons hv hs1 => cases hv with
            | nil => exact ⟨_, _, hs1, (hq.weaken he).appendKont hk, hh⟩
  | listCaseCons =>
      cases hc with
      | cons hop hk => cases hop with
        | listCase he hp _ => cases hs with
          | cons hv hs1 => cases hv with
            | cons hx hxs =>
                exact ⟨_, _, .cons hx (.cons hxs hs1), (hp.weaken he).appendKont hk, hh⟩
  | alloc =>
      cases hc with
      | cons hop hk => cases hop with
        | alloc hcopy =>
            cases hs with
            | cons hv hs1 =>
                exact ⟨_, _, .cons .cell hs1, hk, fun entry he => by
                  rcases List.mem_cons.mp he with rfl | he
                  · exact ⟨_, hv, hcopy⟩
                  · exact hh entry he⟩
  | call hd =>
      cases hc with
      | cons hop hk => cases hop with
        | call hd' hb hbody =>
            have hdd := Option.some.inj (hd.symm.trans hd')
            subst hdd
            exact ⟨_, _, hs, (hbody.weaken hb).appendKont hk, hh⟩
  | emitOk =>
      cases hc with
      | cons hop hk => cases hop with
        | emit _ => cases hs with
          | cons _ hs1 => exact ⟨_, _, hs1, hk, hh⟩
  | emitStop => exact ⟨ts, out, hs, hc, hh⟩
  | acquireOk =>
      cases hc with
      | cons hop hk => cases hop with
        | acquire _ => exact ⟨_, _, .cons .res hs, hk, hh⟩
  | acquireStop => exact ⟨ts, out, hs, hc, hh⟩
  | releaseOk =>
      cases hc with
      | cons hop hk => cases hop with
        | release _ => cases hs with
          | cons _ hs1 => exact ⟨_, _, hs1, hk, hh⟩
  | releaseStop => exact ⟨ts, out, hs, hc, hh⟩

theorem preserve_reach {D H E c c'} (hw : WT D E c) (h : Reach D H c c') : WT D E c' := by
  induction h with
  | refl => exact hw
  | tail _ hstep ih => exact preserve ih hstep

private theorem i64_inv {D} {v} (h : ValT D v .i64) : ∃ x, v = .i64 x := by
  cases h; exact ⟨_, rfl⟩
private theorem res_inv {D} {v} (h : ValT D v .res) : ∃ n, v = .res n := by
  cases h; exact ⟨_, rfl⟩
private theorem prog_inv {D} {v i o e} (h : ValT D v (.prog i o e)) : ∃ p, v = .prog p := by
  cases h; exact ⟨_, rfl⟩
private theorem pair_inv {D} {v a b} (h : ValT D v (.pair a b)) : ∃ x y, v = .pair x y := by
  cases h; exact ⟨_, _, rfl⟩
private theorem sum_inv {D} {v a b} (h : ValT D v (.sum a b)) :
    (∃ x, v = .inl x) ∨ (∃ y, v = .inr y) := by
  cases h with
  | inl _ => exact .inl ⟨_, rfl⟩
  | inr _ => exact .inr ⟨_, rfl⟩
private theorem list_inv {D} {v a} (h : ValT D v (.list a)) :
    v = .nil ∨ ∃ x xs, v = .cons x xs := by
  cases h with
  | nil => exact .inl rfl
  | cons _ _ => exact .inr ⟨_, _, rfl⟩

/-- **Progress relative to the host contract**: whatever the host answers, a
typed running configuration with remaining code has a next step. `pin`,
`restore`, named calls, both case forms and `alloc` included. No termination
claim. -/
theorem progress {D H E} {c : Config} (hw : WT D E c) (hr : c.status = .running)
    (hne : c.code ≠ []) : ∃ c', Step D H c c' := by
  obtain ⟨code, stack, l, cs, hp, n, tr, st⟩ := c
  simp only at hr hne
  subst hr
  obtain ⟨ts, out, hs, hc, _⟩ := hw
  simp only at hs hc
  cases hc with
  | nil => exact absurd rfl hne
  | cons hop hk =>
      cases hop with
      | push => exact ⟨_, .push⟩
      | add =>
          cases hs with
          | cons hb hs1 => cases hs1 with
            | cons ha _ =>
                obtain ⟨b, rfl⟩ := i64_inv hb
                obtain ⟨a, rfl⟩ := i64_inv ha
                exact ⟨_, .add⟩
      | dup => cases hs with | cons _ _ => exact ⟨_, .dup⟩
      | drop => cases hs with | cons _ _ => exact ⟨_, .drop⟩
      | swap => cases hs with
          | cons _ hs1 => cases hs1 with | cons _ _ => exact ⟨_, .swap⟩
      | quote => cases hs with | cons _ _ => exact ⟨_, .quote⟩
      | block => exact ⟨_, .block⟩
      | compose =>
          cases hs with
          | cons hq hs1 => cases hs1 with
            | cons hp' _ =>
                obtain ⟨q, rfl⟩ := prog_inv hq
                obtain ⟨p, rfl⟩ := prog_inv hp'
                exact ⟨_, .compose⟩
      | run =>
          cases hs with
          | cons hp' _ =>
              obtain ⟨p, rfl⟩ := prog_inv hp'
              exact ⟨_, .run⟩
      | dip =>
          cases hs with
          | cons hp' hs1 => cases hs1 with
            | cons _ _ =>
                obtain ⟨p, rfl⟩ := prog_inv hp'
                exact ⟨_, .dip⟩
      | ifte =>
          cases hs with
          | cons hq hs1 => cases hs1 with
            | cons hp' hs2 => cases hs2 with
              | cons hx _ =>
                  obtain ⟨q, rfl⟩ := prog_inv hq
                  obtain ⟨p, rfl⟩ := prog_inv hp'
                  obtain ⟨x, rfl⟩ := i64_inv hx
                  by_cases hx0 : x = 0
                  · exact ⟨_, .ifFalse hx0⟩
                  · exact ⟨_, .ifTrue hx0⟩
      | pair => cases hs with
          | cons _ hs1 => cases hs1 with | cons _ _ => exact ⟨_, .pair⟩
      | unpair => cases hs with
          | cons hv _ =>
              obtain ⟨a, b, rfl⟩ := pair_inv hv
              exact ⟨_, .unpair⟩
      | inl => cases hs with | cons _ _ => exact ⟨_, .inl⟩
      | inr => cases hs with | cons _ _ => exact ⟨_, .inr⟩
      | cases _ _ =>
          cases hs with
          | cons hv _ =>
              rcases sum_inv hv with ⟨v, rfl⟩ | ⟨v, rfl⟩
              · exact ⟨_, .casesLeft⟩
              · exact ⟨_, .casesRight⟩
      | nil => exact ⟨_, .nil⟩
      | cons => cases hs with
          | cons _ hs1 => cases hs1 with | cons _ _ => exact ⟨_, .cons⟩
      | listCase _ _ =>
          cases hs with
          | cons hv _ =>
              rcases list_inv hv with rfl | ⟨x, xs, rfl⟩
              · exact ⟨_, .listCaseNil⟩
              · exact ⟨_, .listCaseCons⟩
      | alloc => cases hs with | cons _ _ => exact ⟨_, .alloc⟩
      | call hd _ => exact ⟨_, .call hd⟩
      | emit =>
          cases hs with
          | cons hx _ =>
              obtain ⟨x, rfl⟩ := i64_inv hx
              by_cases hp' : H tr.length = .permitted
              · exact ⟨_, .emitOk hp'⟩
              · exact ⟨_, .emitStop rfl hp'⟩
      | acquire =>
          by_cases hp' : H tr.length = .permitted
          · exact ⟨_, .acquireOk hp'⟩
          · exact ⟨_, .acquireStop rfl hp'⟩
      | release =>
          cases hs with
          | cons hm _ =>
              obtain ⟨m, rfl⟩ := res_inv hm
              by_cases hp' : H tr.length = .permitted
              · exact ⟨_, .releaseOk hp'⟩
              · exact ⟨_, .releaseStop rfl hp'⟩
  | restore _ _ => exact ⟨_, .restore⟩
  | pin _ _ => exact ⟨_, .pin⟩

/-- Halted configurations (denied or failed) take no further step. -/
theorem halted_no_step {D H} {c c' : Config} (h : c.status ≠ .running) :
    ¬ Step D H c c' := by
  intro hs
  cases hs <;> exact h rfl

/-- The initial configuration of a program. -/
def init (p : List Instr) : Config := ⟨p, [], [], [], [], 0, [], .running⟩

theorem init_wt {D E p out} (hp : CodeT D E p [] out) : WT D E (init p) :=
  ⟨[], out, .nil, hp.kont, by simp [HeapT, init]⟩

/-! ## Owner multiset and OWN-PO-1 -/

/-- Resource ids occurring in a value. Programs contribute none: typed
program bodies are resource-free (`CodeT.rfree`), so a Program value cannot
hide a resource. Aggregates are traversed, so a resource-bearing Pair, Sum or
List is counted at every position. -/
def Value.resIds : Value → List Nat
  | .res n => [n]
  | .pair a b => a.resIds ++ b.resIds
  | .inl a => a.resIds
  | .inr a => a.resIds
  | .cons a b => a.resIds ++ b.resIds
  | _ => []

/-- Resource ids held by one continuation entry. Only the two run-time
frames can hold a value; typed source code cannot contain either. -/
def Instr.heldRes : Instr → List Nat
  | .restore v => v.resIds
  | .pin v => v.resIds
  | _ => []

/-- Resource ids held by the frames of a continuation. -/
def frameRes (k : List Instr) : List Nat := k.flatMap Instr.heldRes

/-- Resource ids on the operand stack, at every aggregate depth. -/
def stackRes (s : List Value) : List Nat := s.flatMap Value.resIds

/-- Resource ids retained by heap cells. Typed heap cells hold `Copy` values,
so this is `[]` in every well-typed configuration; it is included so that
`alloc` is a pure move of the owner multiset. -/
def heapRes (hp : List (Nat × Value)) : List Nat := hp.flatMap fun e => e.2.resIds

/-- The whole-configuration owner multiset: resources on the stack, in
`restore`/`pin` frames, and inside retained heap cells. -/
def resOwners (c : Config) : List Nat :=
  stackRes c.stack ++ frameRes c.code ++ heapRes c.heap

/-- Ownership invariant for typed configurations: the resource owners are
exactly the host-tracked live set, without duplicates, and every live id is
below the fresh-id counter. -/
structure Own (c : Config) : Prop where
  perm : (resOwners c).Perm c.live
  nodup : c.live.Nodup
  fresh : ∀ m ∈ c.live, m < c.next

private theorem perm_swap_left {a b c : List Nat} : (a ++ b ++ c).Perm (b ++ a ++ c) := by
  have h := List.perm_append_comm (l₁ := a) (l₂ := b)
  simpa [List.append_assoc] using h.append_right c

private theorem perm_rotate3 {a b c d : List Nat} :
    ((a ++ b) ++ c ++ d).Perm (b ++ c ++ (a ++ d)) := by
  have h := List.perm_append_comm (l₁ := a) (l₂ := b ++ c)
  simpa [List.append_assoc] using h.append_right d

mutual
  /-- Structural recursion behind `ValT.resIds_nil`. `Value` and `Instr` are
  mutually defined, so the value recursion is written with its companion. -/
  private def resIdsNilAux (D : List Def) :
      (v : Value) → (t : Ty) → ValT D v t → t.Copy → v.resIds = []
    | .i64 _, _, hv, _ => by cases hv; rfl
    | .res _, _, hv, hc => by cases hv; exact (hc : False).elim
    | .unit, _, hv, _ => by cases hv; rfl
    | .cell _, _, hv, _ => by cases hv; rfl
    | .pair a b, _, hv, hc => by
        cases hv with
        | pair hva hvb =>
            rw [Value.resIds, resIdsNilAux D a _ hva hc.1, resIdsNilAux D b _ hvb hc.2]
            rfl
    | .inl a, _, hv, hc => by
        cases hv with
        | inl hva => exact resIdsNilAux D a _ hva hc.1
    | .inr a, _, hv, hc => by
        cases hv with
        | inr hva => exact resIdsNilAux D a _ hva hc.2
    | .nil, _, hv, _ => by cases hv; rfl
    | .cons a b, _, hv, hc => by
        cases hv with
        | cons hva hvb =>
            rw [Value.resIds, resIdsNilAux D a _ hva hc, resIdsNilAux D b _ hvb hc]
            rfl
    | .prog _, _, _, _ => rfl

  /-- Companion of the mutual recursion: every typed instruction is an
  ordinary entry and holds no frame. -/
  private def heldResNilAux (D : List Def) (E : List Label) :
      (op : Instr) → (s t : List Ty) → InstrT D E op s t → op.heldRes = []
    | _, _, _, h => by cases h <;> rfl
end

/-- A typed copyable value holds no resource at any aggregate depth. -/
theorem ValT.resIds_nil {D} {v t} (hv : ValT D v t) (hc : t.Copy) : v.resIds = [] :=
  resIdsNilAux D v t hv hc

/-- Every typed operation is an ordinary entry: it holds no frame. -/
theorem InstrT.heldRes_nil {D E op s t} (h : InstrT D E op s t) : op.heldRes = [] := by
  cases h <;> rfl

/-- Typed code and program bodies hold no frame. Stated in the unfolded
`flatMap` form so it rewrites after `frameRes` unfolds. -/
theorem CodeT.frameRes_nil {D E p s t} (h : CodeT D E p s t) :
    p.flatMap Instr.heldRes = [] := by
  induction p generalizing s with
  | nil => rfl
  | cons op k ih =>
      cases h with
      | cons hop hk => simpa [List.flatMap_cons, hop.heldRes_nil] using ih hk

theorem frameRes_cons (op : Instr) (k : List Instr) :
    frameRes (op :: k) = op.heldRes ++ frameRes k := List.flatMap_cons

theorem frameRes_append (p k : List Instr) : frameRes (p ++ k) = frameRes p ++ frameRes k :=
  List.flatMap_append

theorem stackRes_cons (v : Value) (s : List Value) :
    stackRes (v :: s) = v.resIds ++ stackRes s := List.flatMap_cons

theorem heapRes_cons (n : Nat) (v : Value) (hp : List (Nat × Value)) :
    heapRes ((n, v) :: hp) = v.resIds ++ heapRes hp := List.flatMap_cons

/-- **OWN-PO-1 over the extended language**: every typed step preserves the
whole-configuration owner multiset (stack, both frame kinds and heap cells).

The first alternative is a pure permutation with the live set and fresh
counter unchanged: `dip`, `restore`, `pin`, both branch steps, both case
forms, `run`, `compose`, named `call` and every aggregate instruction.
The second is a permitted `acquire`, which adds exactly the fresh id. The
third is a permitted `release`, which consumes exactly the released id. The
fourth is `alloc`, which consumes a fresh id for the new cell but leaves the
resource owner multiset and the live set unchanged. -/
theorem owners_step {D H E c c'} (hw : WT D E c) (h : Step D H c c') :
    (c'.live = c.live ∧ c'.next = c.next ∧ (resOwners c').Perm (resOwners c)) ∨
    (c'.live = c.next :: c.live ∧ c'.next = c.next + 1 ∧
      (resOwners c').Perm (c.next :: resOwners c)) ∨
    (∃ m, c'.live = c.live.erase m ∧ c'.next = c.next ∧
      (m :: resOwners c').Perm (resOwners c)) ∨
    (c'.live = c.live ∧ c'.next = c.next + 1 ∧ (resOwners c').Perm (resOwners c)) := by
  obtain ⟨ts, out, hs, hc, hh⟩ := hw
  cases h with
  | push =>
      cases hc with
      | cons hop _ => cases hop with
        | push hv hcopy =>
            refine .inl ⟨rfl, rfl, ?_⟩
            simp [resOwners, stackRes, frameRes, heapRes, List.flatMap_cons, Instr.heldRes,
              hv.resIds_nil hcopy, List.nil_append, List.append_assoc]
  | add =>
      exact .inl ⟨rfl, rfl, by
        simp [resOwners, stackRes, frameRes, heapRes, List.flatMap_cons, Value.resIds,
          Instr.heldRes, List.nil_append, List.append_assoc]⟩
  | dup =>
      cases hc with
      | cons hop _ => cases hop with
        | dup hcopy => cases hs with
          | cons hv _ =>
              refine .inl ⟨rfl, rfl, ?_⟩
              simp [resOwners, stackRes, frameRes, heapRes, List.flatMap_cons, Instr.heldRes,
                hv.resIds_nil hcopy, List.nil_append, List.append_assoc]
  | drop =>
      cases hc with
      | cons hop _ => cases hop with
        | drop hcopy => cases hs with
          | cons hv _ =>
              refine .inl ⟨rfl, rfl, ?_⟩
              simp [resOwners, stackRes, frameRes, heapRes, List.flatMap_cons, Instr.heldRes,
                hv.resIds_nil hcopy, List.nil_append, List.append_assoc]
  | @swap a b k s l cs hp n tr =>
      refine .inl ⟨rfl, rfl, ?_⟩
      simp only [resOwners, stackRes, frameRes, heapRes, List.flatMap_cons, Instr.heldRes,
        List.append_assoc]
      have h := perm_swap_left (a := a.resIds) (b := b.resIds) (c := stackRes s)
      simpa [List.append_assoc, stackRes, frameRes, heapRes] using
        h.append_right (frameRes k ++ heapRes hp)
  | quote =>
      cases hc with
      | cons hop _ => cases hop with
        | quote hcopy => cases hs with
          | cons hv _ =>
              refine .inl ⟨rfl, rfl, ?_⟩
              simp [resOwners, stackRes, frameRes, heapRes, List.flatMap_cons, Instr.heldRes,
                Value.resIds, hv.resIds_nil hcopy, List.nil_append, List.append_assoc]
  | block =>
      exact .inl ⟨rfl, rfl, by
        simp [resOwners, stackRes, frameRes, heapRes, List.flatMap_cons, Instr.heldRes,
          Value.resIds, List.nil_append, List.append_assoc]⟩
  | compose =>
      exact .inl ⟨rfl, rfl, by
        simp [resOwners, stackRes, frameRes, heapRes, List.flatMap_cons, Instr.heldRes,
          Value.resIds, List.nil_append, List.append_assoc]⟩
  | run =>
      cases hc with
      | cons hop _ => cases hop with
        | run _ => cases hs with
          | cons hv _ => cases hv with
            | prog cp =>
                refine .inl ⟨rfl, rfl, ?_⟩
                simp [resOwners, stackRes, frameRes, heapRes, List.flatMap_cons, Instr.heldRes,
                  Value.resIds, List.flatMap_append, cp.frameRes_nil, List.nil_append,
                  List.append_assoc]
  | pin =>
      cases hc with
      | cons hop _ => cases hop
      | pin hv hk =>
          cases hv with
          | prog _ =>
              refine .inl ⟨rfl, rfl, ?_⟩
              simp [resOwners, stackRes, frameRes, heapRes, List.flatMap_cons, Instr.heldRes,
                Value.resIds, List.nil_append, List.append_assoc]
  | @dip p v k s l cs hp n tr =>
      cases hc with
      | cons hop _ => cases hop with
        | dip _ => cases hs with
          | cons hp hs1 => cases hs1 with
            | cons hv _ => cases hp with
              | prog cp =>
                  refine .inl ⟨rfl, rfl, ?_⟩
                  simp only [resOwners, stackRes, frameRes, heapRes, List.flatMap_cons,
                    Instr.heldRes, Value.resIds, List.flatMap_append, cp.frameRes_nil,
                    List.nil_append, List.append_assoc]
                  have h := (perm_swap_left (a := v.resIds) (b := stackRes s)
                    (c := frameRes k)).symm
                  simpa [List.append_assoc, stackRes, frameRes, heapRes] using
                    h.append_right (heapRes hp)
  | @restore v k s l cs hp n tr =>
      refine .inl ⟨rfl, rfl, ?_⟩
      simp only [resOwners, stackRes, frameRes, heapRes, List.flatMap_cons, Instr.heldRes,
        List.append_assoc]
      have h := perm_swap_left (a := v.resIds) (b := stackRes s) (c := frameRes k)
      simpa [List.append_assoc, stackRes, frameRes, heapRes] using
        h.append_right (heapRes hp)
  | ifTrue =>
      cases hc with
      | cons hop _ => cases hop with
        | ifte _ _ => cases hs with
          | cons _ hs1 => cases hs1 with
            | cons hp _ => cases hp with
              | prog cp =>
                  refine .inl ⟨rfl, rfl, ?_⟩
                  simp [resOwners, stackRes, frameRes, heapRes, List.flatMap_cons,
                    Instr.heldRes, Value.resIds, List.flatMap_append, cp.frameRes_nil,
                    List.nil_append, List.append_assoc]
  | ifFalse =>
      cases hc with
      | cons hop _ => cases hop with
        | ifte _ _ => cases hs with
          | cons hq hs1 => cases hs1 with
            | cons _ hs2 => cases hs2 with
              | cons _ _ => cases hq with
                | prog cq =>
                    exact .inl ⟨rfl, rfl, by
                      simp [resOwners, stackRes, frameRes, heapRes, List.flatMap_cons,
                        Instr.heldRes, Value.resIds, List.flatMap_append, cq.frameRes_nil,
                        List.nil_append, List.append_assoc]⟩
  | pair =>
      refine .inl ⟨rfl, rfl, ?_⟩
      simp [resOwners, stackRes, frameRes, heapRes, List.flatMap_cons, Instr.heldRes,
        Value.resIds, List.nil_append, List.append_assoc]
  | unpair =>
      refine .inl ⟨rfl, rfl, ?_⟩
      simp [resOwners, stackRes, frameRes, heapRes, List.flatMap_cons, Instr.heldRes,
        Value.resIds, List.nil_append, List.append_assoc]
  | inl =>
      refine .inl ⟨rfl, rfl, ?_⟩
      simp [resOwners, stackRes, frameRes, heapRes, List.flatMap_cons, Instr.heldRes,
        Value.resIds, List.nil_append, List.append_assoc]
  | inr =>
      refine .inl ⟨rfl, rfl, ?_⟩
      simp [resOwners, stackRes, frameRes, heapRes, List.flatMap_cons, Instr.heldRes,
        Value.resIds, List.nil_append, List.append_assoc]
  | casesLeft =>
      cases hc with
      | cons hop _ => cases hop with
        | cases _ hp _ => cases hs with
          | cons hv _ => cases hv with
            | inl _ =>
                refine .inl ⟨rfl, rfl, ?_⟩
                simp [resOwners, stackRes, frameRes, heapRes, List.flatMap_cons,
                  Instr.heldRes, Value.resIds, List.flatMap_append, hp.frameRes_nil,
                  List.nil_append, List.append_assoc]
  | casesRight =>
      cases hc with
      | cons hop _ => cases hop with
        | cases _ _ hq => cases hs with
          | cons hv _ => cases hv with
            | inr _ =>
                refine .inl ⟨rfl, rfl, ?_⟩
                simp [resOwners, stackRes, frameRes, heapRes, List.flatMap_cons,
                  Instr.heldRes, Value.resIds, List.flatMap_append, hq.frameRes_nil,
                  List.nil_append, List.append_assoc]
  | nil =>
      exact .inl ⟨rfl, rfl, by
        simp [resOwners, stackRes, frameRes, heapRes, List.flatMap_cons, Instr.heldRes,
          Value.resIds, List.nil_append, List.append_assoc]⟩
  | cons =>
      refine .inl ⟨rfl, rfl, ?_⟩
      simp [resOwners, stackRes, frameRes, heapRes, List.flatMap_cons, Instr.heldRes,
        Value.resIds, List.nil_append, List.append_assoc]
  | listCaseNil =>
      cases hc with
      | cons hop _ => cases hop with
        | listCase _ _ hq => cases hs with
          | cons hv _ => cases hv with
            | nil =>
                refine .inl ⟨rfl, rfl, ?_⟩
                simp [resOwners, stackRes, frameRes, heapRes, List.flatMap_cons,
                  Instr.heldRes, Value.resIds, List.flatMap_append, hq.frameRes_nil,
                  List.nil_append, List.append_assoc]
  | listCaseCons =>
      cases hc with
      | cons hop _ => cases hop with
        | listCase _ hp _ => cases hs with
          | cons hv _ => cases hv with
            | cons _ _ =>
                refine .inl ⟨rfl, rfl, ?_⟩
                simp [resOwners, stackRes, frameRes, heapRes, List.flatMap_cons,
                  Instr.heldRes, Value.resIds, List.flatMap_append, hp.frameRes_nil,
                  List.nil_append, List.append_assoc]
  | @alloc v k s l cs hp n tr =>
      refine .inr (.inr (.inr ⟨rfl, rfl, ?_⟩))
      simp only [resOwners, stackRes, frameRes, heapRes, List.flatMap_cons, Instr.heldRes,
        Value.resIds, List.nil_append, List.append_assoc]
      simpa [List.append_assoc, stackRes, frameRes, heapRes] using
        (perm_rotate3 (a := v.resIds) (b := stackRes s) (c := frameRes k)
          (d := heapRes hp)).symm
  | call hd =>
      cases hc with
      | cons hop _ => cases hop with
        | call hd' _ hbody =>
            have hdd := Option.some.inj (hd.symm.trans hd')
            subst hdd
            refine .inl ⟨rfl, rfl, ?_⟩
            simp [resOwners, stackRes, frameRes, heapRes, List.flatMap_cons, Instr.heldRes,
              List.flatMap_append, hbody.frameRes_nil, List.nil_append, List.append_assoc]
  | emitOk => exact .inl ⟨rfl, rfl, .refl _⟩
  | emitStop => exact .inl ⟨rfl, rfl, .refl _⟩
  | acquireOk =>
      exact .inr (.inl ⟨rfl, rfl, by
        simp [resOwners, stackRes, frameRes, heapRes, List.flatMap_cons, Instr.heldRes,
          Value.resIds, List.nil_append, List.append_assoc]⟩)
  | acquireStop => exact .inl ⟨rfl, rfl, .refl _⟩
  | releaseOk _ =>
      refine .inr (.inr (.inl ⟨_, rfl, rfl, ?_⟩))
      simp [resOwners, stackRes, frameRes, heapRes, List.flatMap_cons, Instr.heldRes,
        Value.resIds, List.nil_append, List.append_assoc]
  | releaseStop => exact .inl ⟨rfl, rfl, .refl _⟩

/-- **Ownership preservation**: on typed configurations every step keeps the
owners equal to the live set, without duplicates and fresh. -/
theorem own_preserve {D H E c c'} (hw : WT D E c) (ho : Own c) (h : Step D H c c') :
    Own c' := by
  obtain ⟨hp, hn, hf⟩ := ho
  rcases owners_step hw h with ⟨hl, hnx, ho⟩ | ⟨hl, hnx, ho⟩ | ⟨m, hl, hnx, ho⟩ |
    ⟨hl, hnx, ho⟩
  · refine ⟨?_, ?_, ?_⟩
    · rw [hl]; exact ho.trans hp
    · rw [hl]; exact hn
    · rw [hl, hnx]; exact hf
  · refine ⟨?_, ?_, ?_⟩
    · rw [hl]; exact ho.trans (hp.cons _)
    · rw [hl]; exact List.nodup_cons.mpr ⟨fun hm => Nat.lt_irrefl _ (hf _ hm), hn⟩
    · rw [hl, hnx]
      intro x hx
      rcases List.mem_cons.mp hx with rfl | hx
      · exact Nat.lt_succ_self _
      · exact Nat.lt_succ_of_lt (hf x hx)
  · have hmem : m ∈ c.live := (ho.trans hp).subset (List.mem_cons_self ..)
    refine ⟨?_, ?_, ?_⟩
    · rw [hl]; exact ((ho.trans hp).trans (List.perm_cons_erase hmem)).cons_inv
    · rw [hl]; exact hn.erase m
    · rw [hl, hnx]; exact fun x hx => hf x (List.mem_of_mem_erase hx)
  · refine ⟨?_, ?_, ?_⟩
    · rw [hl]; exact ho.trans hp
    · rw [hl]; exact hn
    · rw [hl, hnx]; exact fun x hx => Nat.lt_succ_of_lt (hf x hx)

/-- Ownership holds in every finite prefix of a typed program. -/
theorem own_reach {D H E c c'} (hw : WT D E c) (ho : Own c) (h : Reach D H c c') :
    Own c' := by
  induction h with
  | refl => exact ho
  | tail _ hstep ih => exact own_preserve (preserve_reach hw ‹_›) ih hstep

/-- The host-tracked live set changes only through a recorded, permitted
`acquire` (fresh id added) or `release` (id removed). Moves, aggregate
instructions, named calls and heap allocation leave it unchanged. -/
theorem live_change {D H c c'} (h : Step D H c c') :
    c'.live = c.live ∨ ∃ ev, c'.trace = c.trace ++ [ev] ∧ ev.response = .permitted ∧
      ((ev.label = .acquire ∧ ev.arg = c.next ∧ c'.live = ev.arg :: c.live) ∨
       (ev.label = .release ∧ c'.live = c.live.erase ev.arg)) := by
  cases h with
  | acquireOk => exact .inr ⟨_, rfl, rfl, .inl ⟨rfl, rfl, rfl⟩⟩
  | releaseOk => exact .inr ⟨_, rfl, rfl, .inr ⟨rfl, rfl⟩⟩
  | _ => exact .inl rfl

/-! ## Recursive capture eligibility (OWN-PO-3) -/

mutual
  /-- A value contains no resource at any aggregate or program depth. -/
  inductive RFreeV : Value → Prop where
    | i64 : RFreeV (.i64 x)
    | res : False → RFreeV (.res n)
    | unit : RFreeV .unit
    | cell : RFreeV (.cell n)
    | pair : RFreeV a → RFreeV b → RFreeV (.pair a b)
    | inl : RFreeV v → RFreeV (.inl v)
    | inr : RFreeV v → RFreeV (.inr v)
    | nil : RFreeV .nil
    | cons : RFreeV x → RFreeV xs → RFreeV (.cons x xs)
    | prog : RFreeC p → RFreeV (.prog p)

  /-- A source instruction contains no resource literal. `pin` has no rule:
  it is a run-time frame only. -/
  inductive RFreeO : Instr → Prop where
    | push : RFreeV v → RFreeO (.push v)
    | add : RFreeO .add
    | dup : RFreeO .dup
    | drop : RFreeO .drop
    | swap : RFreeO .swap
    | quote : RFreeO .quote
    | block : RFreeC p → RFreeO (.block p)
    | compose : RFreeO .compose
    | run : RFreeO .run
    | dip : RFreeO .dip
    | ifte : RFreeO .ifte
    | emit : RFreeO .emit
    | acquire : RFreeO .acquire
    | release : RFreeO .release
    | pair : RFreeO .pair
    | unpair : RFreeO .unpair
    | inl : RFreeO .inl
    | inr : RFreeO .inr
    | cases : RFreeC p → RFreeC q → RFreeO (.cases p q)
    | nil : RFreeO .nil
    | cons : RFreeO .cons
    | listCase : RFreeC p → RFreeC q → RFreeO (.listCase p q)
    | alloc : RFreeO .alloc
    | call : RFreeO (.call i)
    | restore : RFreeV v → RFreeO (.restore v)

  /-- Resource-free code. -/
  inductive RFreeC : List Instr → Prop where
    | nil : RFreeC []
    | cons : RFreeO op → RFreeC k → RFreeC (op :: k)
end

/-- Every program body reachable inside a value is resource-free. Stack
aggregates may contain resources, but a Program value may not. -/
def Value.programsRFree : Value → Prop
  | .prog p => RFreeC p
  | .pair a b => a.programsRFree ∧ b.programsRFree
  | .inl a => a.programsRFree
  | .inr a => a.programsRFree
  | .cons a b => a.programsRFree ∧ b.programsRFree
  | _ => True

mutual
  /-- Typed copyable values are resource-free at every depth. -/
  private def rfreeVAux (D : List Def) :
      (v : Value) → (t : Ty) → ValT D v t → t.Copy → RFreeV v
    | .i64 _, _, hv, _ => by cases hv; exact .i64
    | .res _, _, hv, hc => by cases hv; exact .res (hc : False)
    | .unit, _, hv, _ => by cases hv; exact .unit
    | .cell _, _, hv, _ => by cases hv; exact .cell
    | .pair a b, _, hv, hc => by
        cases hv with
        | pair hva hvb =>
            exact .pair (rfreeVAux D a _ hva hc.1) (rfreeVAux D b _ hvb hc.2)
    | .inl a, _, hv, hc => by
        cases hv with
        | inl hva => exact .inl (rfreeVAux D a _ hva hc.1)
    | .inr a, _, hv, hc => by
        cases hv with
        | inr hva => exact .inr (rfreeVAux D a _ hva hc.2)
    | .nil, _, hv, _ => by cases hv; exact .nil
    | .cons a b, _, hv, hc => by
        cases hv with
        | cons hva hvb =>
            exact .cons (rfreeVAux D a _ hva hc) (rfreeVAux D b _ hvb hc)
    | .prog p, _, hv, _ => by
        cases hv with
        | prog hc => exact .prog (rfreeCAux D _ p _ _ hc)

  /-- Program bodies reachable from typed values are resource-free. -/
  private def programsRFreeAux (D : List Def) :
      (v : Value) → (t : Ty) → ValT D v t → v.programsRFree
    | .i64 _, _, hv => by cases hv; trivial
    | .res _, _, hv => by cases hv; trivial
    | .unit, _, hv => by cases hv; trivial
    | .cell _, _, hv => by cases hv; trivial
    | .pair a b, _, hv => by
        cases hv with
        | pair hva hvb => exact ⟨programsRFreeAux D a _ hva, programsRFreeAux D b _ hvb⟩
    | .inl a, _, hv => by
        cases hv with
        | inl hva => exact programsRFreeAux D a _ hva
    | .inr a, _, hv => by
        cases hv with
        | inr hva => exact programsRFreeAux D a _ hva
    | .nil, _, hv => by cases hv; trivial
    | .cons a b, _, hv => by
        cases hv with
        | cons hva hvb => exact ⟨programsRFreeAux D a _ hva, programsRFreeAux D b _ hvb⟩
    | .prog p, _, hv => by
        cases hv with
        | prog hc => exact rfreeCAux D _ p _ _ hc

  /-- Typed instructions contain no resource literal at any depth. -/
  private def rfreeOAux (D : List Def) (E : List Label) :
      (op : Instr) → (s t : List Ty) → InstrT D E op s t → RFreeO op
    | .push v, _, _, h => by cases h with | push hv hc => exact .push (rfreeVAux D v _ hv hc)
    | .add, _, _, h => by cases h; exact .add
    | .dup, _, _, h => by cases h; exact .dup
    | .drop, _, _, h => by cases h; exact .drop
    | .swap, _, _, h => by cases h; exact .swap
    | .quote, _, _, h => by cases h; exact .quote
    | .block p, _, _, h => by cases h with | block hc => exact .block (rfreeCAux D _ p _ _ hc)
    | .compose, _, _, h => by cases h; exact .compose
    | .run, _, _, h => by cases h; exact .run
    | .dip, _, _, h => by cases h; exact .dip
    | .ifte, _, _, h => by cases h; exact .ifte
    | .emit, _, _, h => by cases h; exact .emit
    | .acquire, _, _, h => by cases h; exact .acquire
    | .release, _, _, h => by cases h; exact .release
    | .pair, _, _, h => by cases h; exact .pair
    | .unpair, _, _, h => by cases h; exact .unpair
    | .inl, _, _, h => by cases h; exact .inl
    | .inr, _, _, h => by cases h; exact .inr
    | .cases p q, _, _, h => by
        cases h with
        | cases _ hp hq =>
            exact .cases (rfreeCAux D _ p _ _ hp) (rfreeCAux D _ q _ _ hq)
    | .nil, _, _, h => by cases h; exact .nil
    | .cons, _, _, h => by cases h; exact .cons
    | .listCase p q, _, _, h => by
        cases h with
        | listCase _ hp hq =>
            exact .listCase (rfreeCAux D _ p _ _ hp) (rfreeCAux D _ q _ _ hq)
    | .alloc, _, _, h => by cases h; exact .alloc
    | .call _, _, _, h => by cases h; exact .call
    | .restore _, _, _, h => by cases h
    | .pin _, _, _, h => by cases h

  /-- Typed code is resource-free. -/
  private def rfreeCAux (D : List Def) (E : List Label) :
      (p : List Instr) → (s t : List Ty) → CodeT D E p s t → RFreeC p
    | [], _, _, h => by cases h; exact .nil
    | op :: k, _, _, h => by
        cases h with
        | cons hop hk =>
            exact .cons (rfreeOAux D E op _ _ hop) (rfreeCAux D E k _ _ hk)
end

/-- Typed copyable values contain no resource anywhere. -/
theorem ValT.rfree {D} {v t} (hv : ValT D v t) (hc : t.Copy) : RFreeV v :=
  rfreeVAux D v t hv hc

/-- Every Program body reachable inside a typed value is resource-free. -/
theorem ValT.programsRFree {D} {v t} (hv : ValT D v t) : v.programsRFree :=
  programsRFreeAux D v t hv

/-- Typed instructions contain no resource literal at any depth. -/
theorem InstrT.rfree {D E op s t} (h : InstrT D E op s t) : RFreeO op :=
  rfreeOAux D E op s t h

/-- Typed code is resource-free, including nested program bodies. -/
theorem CodeT.rfree {D E p s t} (h : CodeT D E p s t) : RFreeC p :=
  rfreeCAux D E p s t h

/-- A typed continuation is resource-free except that `restore` frames may
hold any value and `pin` frames hold a resource-free Program. -/
inductive RFreeK : List Instr → Prop where
  | nil : RFreeK []
  | op : RFreeO op → RFreeK k → RFreeK (op :: k)
  | restore : RFreeK k → RFreeK (.restore v :: k)
  | pin : RFreeC p → RFreeK k → RFreeK (.pin (.prog p) :: k)

theorem KontT.rfree {D E k s t} (h : KontT D E k s t) : RFreeK k := by
  induction h with
  | nil => exact .nil
  | cons hop _ ih => exact .op (InstrT.rfree hop) ih
  | restore _ _ ih => exact .restore ih
  | pin hv _ ih =>
      cases hv with
      | prog hc => exact .pin (CodeT.rfree hc) ih

theorem StackT.programsRFree {D s ts} (h : StackT D s ts) :
    ∀ v ∈ s, v.programsRFree := by
  induction h with
  | nil => simp
  | cons hv _ ih =>
      intro w hw
      rcases List.mem_cons.mp hw with rfl | hw
      · exact ValT.programsRFree hv
      · exact ih w hw

/-- Typed heap cells are resource-free. -/
theorem HeapT.rfree {D heap} (h : HeapT D heap) :
    ∀ entry ∈ heap, RFreeV entry.2 := by
  intro entry he
  obtain ⟨t, hv, hc⟩ := h entry he
  exact ValT.rfree hv hc

/-- **OWN-PO-3 for typed configurations**: the continuation is
resource-free except frames, every heap cell is resource-free, and every
Program value on the stack has a resource-free body. -/
theorem typed_rfree {D E c} (hw : WT D E c) :
    RFreeK c.code ∧ (∀ entry ∈ c.heap, RFreeV entry.2) ∧
      ∀ v ∈ c.stack, v.programsRFree := by
  obtain ⟨ts, out, hs, hc, hh⟩ := hw
  exact ⟨KontT.rfree hc, HeapT.rfree hh, StackT.programsRFree hs⟩

/-- `quote` captures only copyable values, hence no resource. -/
theorem quote_capture_rfree {D v t} (hv : ValT D v t) (hc : t.Copy) : RFreeV v :=
  ValT.rfree hv hc

/-- `alloc` stores only copyable values, hence no resource in the heap. -/
theorem alloc_store_rfree {D v t} (hv : ValT D v t) (hc : t.Copy) : RFreeV v :=
  ValT.rfree hv hc

/-! ## Heap reference safety -/

mutual
  /-- Cell ids occurring in a value, including inside Program bodies. -/
  def Value.cellIds : Value → List Nat
    | .cell n => [n]
    | .pair a b => a.cellIds ++ b.cellIds
    | .inl a => a.cellIds
    | .inr a => a.cellIds
    | .cons a b => a.cellIds ++ b.cellIds
    | .prog p => Instrs.cellIds p
    | _ => []

  /-- Cell ids occurring in an instruction sequence. -/
  def Instrs.cellIds : List Instr → List Nat
    | [] => []
    | op :: k => Instr.cellIds op ++ Instrs.cellIds k

  /-- Cell ids occurring in one instruction, including nested bodies and
  the values held by run-time frames. -/
  def Instr.cellIds : Instr → List Nat
    | .push v => v.cellIds
    | .block p => Instrs.cellIds p
    | .cases p q => Instrs.cellIds p ++ Instrs.cellIds q
    | .listCase p q => Instrs.cellIds p ++ Instrs.cellIds q
    | .restore v => v.cellIds
    | .pin v => v.cellIds
    | _ => []
end

/-- Every cell reference in code, stack or heap. -/
def cellRefs (c : Config) : List Nat :=
  Instrs.cellIds c.code ++ c.stack.flatMap Value.cellIds ++
    c.heap.flatMap (fun entry => entry.2.cellIds)

theorem Instrs.cellIds_append (p q : List Instr) :
    Instrs.cellIds (p ++ q) = Instrs.cellIds p ++ Instrs.cellIds q := by
  induction p with
  | nil => rfl
  | cons op k ih =>
      simp [Instrs.cellIds, ih, List.append_assoc]

/-- Heap-safety invariant: allocated cell ids are distinct and fresh, and
every cell reference in the configuration names an allocated cell. -/
structure HeapSafe (c : Config) : Prop where
  cellsNodup : c.cells.Nodup
  refsAllocated : ∀ n ∈ cellRefs c, n ∈ c.cells
  freshCells : ∀ n ∈ c.cells, n < c.next

/-- Every definition body is cell-free. A definition table is static, so this
is an initial condition rather than a step invariant. -/
def DefsCellFree (D : List Def) : Prop := ∀ d ∈ D, Instrs.cellIds d.body = []

private theorem getDef_mem {D : List Def} {i d} (h : getDef D i = some d) : d ∈ D := by
  induction D generalizing i with
  | nil => cases i <;> simp [getDef] at h
  | cons d0 ds ih =>
      cases i with
      | zero => simp [getDef] at h; subst h; exact List.mem_cons_self ..
      | succ i => simp [getDef] at h; exact List.mem_cons_of_mem _ (ih h)

/-- **Heap safety is preserved by every step**: a non-allocating step only
moves, duplicates or drops existing references; `alloc` adds the fresh id to
the allocation set and to the pushed reference. -/
theorem heap_safe_step {D H c c'} (hdefs : DefsCellFree D) (h : HeapSafe c)
    (hs : Step D H c c') : HeapSafe c' := by
  obtain ⟨hnodup, hrefs, hfresh⟩ := h
  cases hs with
  | @alloc v k s l cs hp n tr =>
      refine ⟨?_, ?_, ?_⟩
      · refine List.nodup_cons.mpr ⟨?_, hnodup⟩
        intro hmem
        exact Nat.lt_irrefl n (by simpa using hfresh n hmem)
      · intro m hm
        have hsub : cellRefs ⟨k, .cell n :: s, l, n :: cs, (n, v) :: hp, n + 1, tr, .running⟩ ⊆
            n :: cellRefs ⟨.alloc :: k, v :: s, l, cs, hp, n, tr, .running⟩ := by
          intro x hx
          simp only [cellRefs, Instrs.cellIds, Instr.cellIds, Value.cellIds,
            List.flatMap_cons, List.mem_append, List.mem_cons,
            List.not_mem_nil, or_false, false_or] at hx ⊢
          tauto
        rcases List.mem_cons.mp (hsub hm) with rfl | hm'
        · exact List.mem_cons_self ..
        · exact List.mem_cons_of_mem _ (hrefs m hm')
      · intro m hm
        rcases List.mem_cons.mp hm with heq | hm'
        · rw [heq]; exact Nat.lt_succ_self _
        · exact Nat.lt_succ_of_lt (hfresh m hm')
  | @acquireOk k s l cs hp n tr hm =>
      refine ⟨hnodup, ?_, ?_⟩
      · intro x hx
        refine hrefs x ?_
        simp only [cellRefs, Instrs.cellIds, Instr.cellIds, Value.cellIds,
          List.flatMap_cons, List.mem_append, List.not_mem_nil, false_or] at hx ⊢
        tauto
      · intro m hm
        exact Nat.lt_succ_of_lt (hfresh m hm)
  | @call i d k s l cs hp n tr hd =>
      have hbody : Instrs.cellIds d.body = [] := hdefs d (getDef_mem hd)
      refine ⟨hnodup, ?_, ?_⟩
      · intro x hx
        refine hrefs x ?_
        simp only [cellRefs, Instrs.cellIds, Instrs.cellIds_append, Instr.cellIds,
          List.mem_append, List.not_mem_nil, false_or, hbody] at hx ⊢
        tauto
      · intro m hm; exact hfresh m hm
  | _ =>
      refine ⟨?_, ?_, ?_⟩
      · exact hnodup
      · intro n hn
        refine hrefs n ?_
        simp only [cellRefs, Instrs.cellIds, Instr.cellIds, Value.cellIds,
          Instrs.cellIds_append, List.flatMap_cons, List.mem_append,
          List.not_mem_nil, or_false, false_or] at hn ⊢
        tauto
      · intro n hn; exact hfresh n hn

/-- Heap safety holds in every finite prefix. -/
theorem heap_safe_reach {D H c c'} (hdefs : DefsCellFree D) (h : Reach D H c c')
    (hsafe : HeapSafe c) : HeapSafe c' := by
  induction h with
  | refl => exact hsafe
  | tail _ hstep ih => exact heap_safe_step hdefs ih hstep

theorem init_own {D E p out} (hp : CodeT D E p [] out) : Own (init p) := by
  refine ⟨?_, by simp [init], by simp [init]⟩
  simp [resOwners, init, frameRes, stackRes, heapRes, hp.frameRes_nil]

theorem init_heap_safe {p : List Instr} (hp : Instrs.cellIds p = []) :
    HeapSafe (init p) := by
  refine ⟨by simp [init], ?_, by simp [init]⟩
  intro n hn
  simp [init, cellRefs, hp] at hn

/-- **Whole-execution theorem for the extended model.** In every finite
prefix of a typed program whose source is cell-free, with a cell-free
definition table: the configuration is well typed; OWN-PO-1 holds; heap
references are safe; and OWN-PO-3 holds (resource-free code and heap, and
resource-free Program bodies on the stack). -/
theorem reachable_sound {D H E p out c} (hdefs : DefsCellFree D)
    (hcell : Instrs.cellIds p = []) (hp : CodeT D E p [] out)
    (h : Reach D H (init p) c) :
    WT D E c ∧ Own c ∧ HeapSafe c ∧ RFreeK c.code ∧
      (∀ entry ∈ c.heap, RFreeV entry.2) ∧
      ∀ v ∈ c.stack, v.programsRFree := by
  have hw := preserve_reach (init_wt hp) h
  have ho := own_reach (init_wt hp) (init_own hp) h
  have hs := heap_safe_reach hdefs h (init_heap_safe hcell)
  obtain ⟨hrk, hheap, hstack⟩ := typed_rfree hw
  exact ⟨hw, ho, hs, hrk, hheap, hstack⟩

/-! ## Model controls -/

private def permit : Nat → Response := fun _ => .permitted
private def noDefs : List Def := []

/-- Pair construction followed by elimination and addition returns one I64. -/
theorem pair_unpair_round_trip :
    CodeT noDefs [] [.push (.i64 1), .push (.i64 2), .pair, .unpair, .add] [] [.i64] ∧
    Reach noDefs permit (init [.push (.i64 1), .push (.i64 2), .pair, .unpair, .add])
      ⟨[], [.i64 3], [], [], [], 0, [], .running⟩ := by
  refine ⟨?_, ?_⟩
  · exact .cons (.push .i64 trivial) (.cons (.push .i64 trivial)
      (.cons .pair (.cons .unpair (.cons .add .nil))))
  · exact ((((Relation.ReflTransGen.single .push).tail .push).tail .pair).tail .unpair).tail .add

/-- `cases` runs the left arm on an `inl` payload and the right arm on `inr`. -/
theorem sum_case_selects_arm :
    CodeT noDefs [] [.push (.i64 1), .inl,
        .cases [.drop, .push (.i64 10)] [.drop, .push (.i64 20)]] [] [.i64] ∧
    CodeT noDefs [] [.push (.i64 1), .inr,
        .cases [.drop, .push (.i64 10)] [.drop, .push (.i64 20)]] [] [.i64] ∧
    Reach noDefs permit (init [.push (.i64 1), .inl,
        .cases [.drop, .push (.i64 10)] [.drop, .push (.i64 20)]])
      ⟨[], [.i64 10], [], [], [], 0, [], .running⟩ ∧
    Reach noDefs permit (init [.push (.i64 1), .inr,
        .cases [.drop, .push (.i64 10)] [.drop, .push (.i64 20)]])
      ⟨[], [.i64 20], [], [], [], 0, [], .running⟩ := by
  refine ⟨?_, ?_, ?_, ?_⟩
  · have armL : CodeT noDefs [] [.drop, .push (.i64 10)] [.i64] [.i64] :=
      .cons (.drop (t := .i64) trivial) (.cons (.push .i64 trivial) .nil)
    have armR : CodeT noDefs [] [.drop, .push (.i64 20)] [.i64] [.i64] :=
      .cons (.drop (t := .i64) trivial) (.cons (.push .i64 trivial) .nil)
    refine .cons (.push .i64 trivial)
      (.cons (.inl (b := .i64) (s := [])) (.cons ?_ .nil))
    exact .cases (a := .i64) (b := .i64) (s := []) (t := [.i64]) (e := []) (by simp)
      armL armR
  · have armL : CodeT noDefs [] [.drop, .push (.i64 10)] [.i64] [.i64] :=
      .cons (.drop (t := .i64) trivial) (.cons (.push .i64 trivial) .nil)
    have armR : CodeT noDefs [] [.drop, .push (.i64 20)] [.i64] [.i64] :=
      .cons (.drop (t := .i64) trivial) (.cons (.push .i64 trivial) .nil)
    refine .cons (.push .i64 trivial)
      (.cons (.inr (a := .i64) (s := [])) (.cons ?_ .nil))
    exact .cases (a := .i64) (b := .i64) (s := []) (t := [.i64]) (e := []) (by simp)
      armL armR
  · exact ((((Relation.ReflTransGen.single .push).tail .inl).tail .casesLeft).tail
      .drop).tail .push
  · exact ((((Relation.ReflTransGen.single .push).tail .inr).tail .casesRight).tail
      .drop).tail .push

/-- `listCase` runs the nil arm on `nil` and the cons arm on a built list. -/
theorem list_case_selects_arm :
    Reach noDefs permit (init [.nil, .listCase
        [.drop, .drop, .push (.i64 7)] [.push (.i64 7)]])
      ⟨[], [.i64 7], [], [], [], 0, [], .running⟩ ∧
    Reach noDefs permit (init [.nil, .push (.i64 1), .cons, .listCase
        [.drop, .drop, .push (.i64 7)] [.push (.i64 7)]])
      ⟨[], [.i64 7], [], [], [], 0, [], .running⟩ := by
  refine ⟨?_, ?_⟩
  · exact ((Relation.ReflTransGen.single (Step.nil (D := noDefs) (H := permit))).tail
      .listCaseNil).tail .push
  · exact ((((((Relation.ReflTransGen.single (Step.nil (D := noDefs) (H := permit))).tail
      (Step.push (v := .i64 1))).tail (Step.cons)).tail (Step.listCaseCons)).tail
      (Step.drop)).tail (Step.drop)).tail (Step.push (v := .i64 7))

private def callDef : Def := ⟨[.i64, .i64], [.i64], [], [.add]⟩

/-- A named definition runs its exact-interface body. -/
theorem call_runs_named_body :
    CodeT [callDef] [] [.push (.i64 5), .push (.i64 7), .call 0] [] [.i64] ∧
    Reach [callDef] permit (init [.push (.i64 5), .push (.i64 7), .call 0])
      ⟨[], [.i64 12], [], [], [], 0, [], .running⟩ := by
  refine ⟨?_, ?_⟩
  · exact .cons (.push .i64 trivial) (.cons (.push .i64 trivial)
      (.cons (.call (i := 0) (d := callDef) rfl (by simp [callDef])
        (.cons .add .nil)) .nil))
  · exact (((Relation.ReflTransGen.single .push).tail .push).tail
      (.call (D := [callDef]) (H := permit) (i := 0) (d := callDef) rfl)).tail .add

private theorem call_input_eq {D E i s t} (h : InstrT D E (.call i) s t) :
    ∃ d, getDef D i = some d ∧ d.input = s ∧ d.output = t := by
  cases h with
  | call hd _ _ => exact ⟨_, hd, rfl, rfl⟩

/-- A named call uses the definition's exact input shape, not a polymorphic one. -/
theorem call_interface_exact :
    ¬ CodeT [callDef] [] [.call 0] [] [.i64] := by
  intro h
  cases h with
  | cons hop _ =>
      obtain ⟨d, hd, hin, _⟩ := call_input_eq hop
      have hd' : d = callDef := (Option.some.inj (by simpa [getDef] using hd)).symm
      subst hd'
      simp [callDef] at hin

/-- `alloc` moves a value into a fresh cell; dropping the last reference does
not reclaim it in this model (deferred storage reclamation). -/
theorem alloc_then_drop_keeps_cell :
    Reach noDefs permit (init [.push (.i64 7), .alloc, .drop])
      ⟨[], [], [], [0], [(0, .i64 7)], 1, [], .running⟩ := by
  exact ((Relation.ReflTransGen.single .push).tail (.alloc)).tail .drop

/-- `run` pins the executed Program value in a frame until its body reaches
the pin, then releases it. -/
theorem run_pins_program :
    Reach noDefs permit (init [.block [.push (.i64 4)], .run])
      ⟨[.push (.i64 4), .pin (.prog [.push (.i64 4)])], [], [], [], [], 0, [], .running⟩ ∧
    Reach noDefs permit (init [.block [.push (.i64 4)], .run])
      ⟨[], [.i64 4], [], [], [], 0, [], .running⟩ := by
  refine ⟨((Relation.ReflTransGen.single .block).tail .run), ?_⟩
  exact (((((Relation.ReflTransGen.single .block).tail .run).tail .push).tail
    (Step.pin (D := noDefs) (H := permit))))

/-- `dip` holds a Pair containing a live resource in its frame; ownership is
preserved, and the resource is restored once and released. -/
theorem dip_holds_aggregate_in_frame :
    Reach noDefs permit
      (init [.acquire, .push (.i64 1), .pair, .block [], .dip, .unpair, .drop, .release])
      ⟨[.restore (.pair (.i64 1) (.res 0)), .unpair, .drop, .release], [], [0], [], [], 1,
        [⟨.acquire, 0, .permitted⟩], .running⟩ ∧
    Own ⟨[.restore (.pair (.i64 1) (.res 0)), .unpair, .drop, .release], [], [0], [], [], 1,
        [⟨.acquire, 0, .permitted⟩], .running⟩ ∧
    Reach noDefs permit
      (init [.acquire, .push (.i64 1), .pair, .block [], .dip, .unpair, .drop, .release])
      ⟨[], [], [], [], [], 1,
        [⟨.acquire, 0, .permitted⟩, ⟨.release, 0, .permitted⟩], .running⟩ := by
  refine ⟨?_, ?_, ?_⟩
  · exact ((((Relation.ReflTransGen.single (.acquireOk rfl)).tail .push).tail .pair).tail
      (Step.block (p := []))).tail
      (Step.dip (p := []) (v := Value.pair (Value.i64 1) (Value.res 0)))
  · refine ⟨?_, by simp, by simp⟩
    simp [resOwners, stackRes, frameRes, heapRes, Instr.heldRes, Value.resIds]
  · exact Relation.ReflTransGen.tail
      (Relation.ReflTransGen.tail
        (Relation.ReflTransGen.tail
          (Relation.ReflTransGen.tail
            (Relation.ReflTransGen.tail
              (Relation.ReflTransGen.tail
                (Relation.ReflTransGen.tail
                  (Relation.ReflTransGen.tail
                    (Relation.ReflTransGen.single (Step.acquireOk rfl))
                    (Step.push (v := .i64 1)))
                  Step.pair)
                (Step.block (p := [])))
              (Step.dip (p := []) (v := Value.pair (Value.i64 1) (Value.res 0))))
            Step.restore)
          Step.unpair)
        Step.drop)
      (Step.releaseOk rfl)

/-! ## Negative controls -/

theorem dup_res_untypable {D E s out} : ¬ InstrT D E .dup (.res :: s) out := by
  intro h; cases h with | dup hc => exact hc

theorem drop_res_untypable {D E s out} : ¬ InstrT D E .drop (.res :: s) out := by
  intro h; cases h with | drop hc => exact hc

theorem quote_res_untypable {D E s out} : ¬ InstrT D E .quote (.res :: s) out := by
  intro h; cases h with | quote hc => exact hc

theorem alloc_res_untypable {D E s out} : ¬ InstrT D E .alloc (.res :: s) out := by
  intro h; cases h with | alloc hc => exact hc

theorem push_res_untypable {D E n s out} : ¬ InstrT D E (.push (.res n)) s out := by
  intro h; cases h with | push hv hc => cases hv; exact hc

theorem dup_pair_res_untypable {D E s out} :
    ¬ InstrT D E .dup (Ty.pair Ty.i64 Ty.res :: s) out := by
  intro h; cases h with | dup hc => exact hc.2

theorem drop_sum_res_untypable {D E s out} :
    ¬ InstrT D E .drop (Ty.sum Ty.res Ty.i64 :: s) out := by
  intro h; cases h with | drop hc => exact hc.1

theorem drop_list_res_untypable {D E s out} :
    ¬ InstrT D E .drop (Ty.list Ty.res :: s) out := by
  intro h; cases h with | drop hc => exact hc

theorem untyped_dup_breaks_ownership :
    Reach noDefs permit (init [.acquire, .dup])
      ⟨[], [.res 0, .res 0], [0], [], [], 1, [⟨.acquire, 0, .permitted⟩], .running⟩ ∧
    ¬ Own ⟨[], [.res 0, .res 0], [0], [], [], 1, [⟨.acquire, 0, .permitted⟩], .running⟩ := by
  refine ⟨(Relation.ReflTransGen.single (.acquireOk rfl)).tail .dup, ?_⟩
  intro h
  have := h.perm.length_eq
  simp [resOwners, stackRes, frameRes, heapRes, Value.resIds] at this

theorem untyped_drop_breaks_ownership :
    Reach noDefs permit (init [.acquire, .drop])
      ⟨[], [], [0], [], [], 1, [⟨.acquire, 0, .permitted⟩], .running⟩ ∧
    ¬ Own ⟨[], [], [0], [], [], 1, [⟨.acquire, 0, .permitted⟩], .running⟩ := by
  refine ⟨(Relation.ReflTransGen.single (.acquireOk rfl)).tail .drop, ?_⟩
  intro h
  have := h.perm.length_eq
  simp [resOwners, stackRes, frameRes, heapRes] at this

theorem untyped_alloc_hides_resource :
    Reach noDefs permit (init [.acquire, .alloc])
      ⟨[], [.cell 1], [0], [1], [(1, .res 0)], 2,
        [⟨.acquire, 0, .permitted⟩], .running⟩ ∧
    ¬ RFreeV (.res 0) := by
  refine ⟨(Relation.ReflTransGen.single (.acquireOk rfl)).tail
    (Step.alloc (D := noDefs) (H := permit)), ?_⟩
  intro h; cases h with | res h => exact h
