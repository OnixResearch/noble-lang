import Mathlib.Logic.Relation
import Mathlib.Data.List.Perm.Basic
import Mathlib.Data.List.Nodup

/-!
# Effect and resource layer (issue #15, Phase 2)

This is a small-step model **separate from MC1**. MC1 has no effects and no
resources. The model adds:

* a host word `emit` (a `test.emit`-like request carrying an I64 payload);
* move-only resource tokens obtained with `acquire` and consumed with
  `release`.

All three host requests receive a response from a host oracle
`H : Nat → Response`, indexed by request number, with response `permitted`,
`denied` or `failed`. Every request, including a denied or failed one, is
appended to an ordered trace. A non-permitted response halts the
configuration with status `denied` or `failed`, and the stack, code and
ownership are left unchanged. Cleanup after an abnormal stop is **not**
modelled.

Orientation: stacks and stack types are **top-first** in this file (the head
of the list is the stack top), as in MC1's internal `Run`/`Step`.

Program values carry a latent effect bound: `prog i o e`. `run` executes the
body by prepending it to the continuation `code`, so the continuation plays
the role of the frame stack.

Claims proved here, all about this model only:
* `preserve`: typing is preserved by every step, including halting steps.
* `progress`: a typed running configuration with nonempty code always
  steps, whatever the host answers. This does not claim termination.
* `effect_sound`: in every finite execution prefix, the trace only grows by
  appending, and every appended event's label is in the declared bound.
  Denied and failed requests are included.
* `bound_does_not_fix_order`: equal bounds do not make reordering valid.
* `own_preserve` and `live_change`: ownership is preserved, and the
  host-tracked live set changes only through recorded, permitted `acquire`
  or `release` events.
* `typed_rfree`: programs and continuations of typed configurations contain
  no resource anywhere, including nested programs.
* Negative cases: duplication, discard and capture of a resource are
  untypable, and the untyped executions that perform them break ownership.
-/

namespace NobleFoundation.Effects

inductive Label | emit | acquire | release
  deriving DecidableEq, Repr

inductive Response | permitted | denied | failed
  deriving DecidableEq, Repr

inductive Status | running | denied | failed
  deriving DecidableEq, Repr

def Response.halt : Response → Status
  | .permitted => .running
  | .denied => .denied
  | .failed => .failed

structure Event where
  label : Label
  arg : Nat
  response : Response
  deriving DecidableEq, Repr

inductive ETy where
  | i64
  | res
  | prog : List ETy → List ETy → List Label → ETy

/-- Copyable types may be duplicated, discarded and captured. Resources may not. -/
def ETy.Copy : ETy → Prop
  | .res => False
  | _ => True

mutual
  inductive EVal where
    | i64 : BitVec 64 → EVal
    | res : Nat → EVal
    | prog : List EOp → EVal

  inductive EOp where
    | push : EVal → EOp
    | add | dup | drop | swap | quote | compose | run
    | block : List EOp → EOp
    | emit | acquire | release
end

/-- A configuration: continuation (frames), operand stack, the host-tracked set
of live resources owned by the guest, the next fresh resource id, the ordered
request trace (oldest first) and the status. -/
structure Cfg where
  code : List EOp
  stack : List EVal
  live : List Nat
  next : Nat
  trace : List Event
  status : Status

/-- Small-step semantics. Only `running` configurations step. -/
inductive Step (H : Nat → Response) : Cfg → Cfg → Prop where
  | push {v k s l n tr} :
      Step H ⟨.push v :: k, s, l, n, tr, .running⟩ ⟨k, v :: s, l, n, tr, .running⟩
  | add {a b k s l n tr} :
      Step H ⟨.add :: k, .i64 b :: .i64 a :: s, l, n, tr, .running⟩
        ⟨k, .i64 (a + b) :: s, l, n, tr, .running⟩
  | dup {v k s l n tr} :
      Step H ⟨.dup :: k, v :: s, l, n, tr, .running⟩ ⟨k, v :: v :: s, l, n, tr, .running⟩
  | drop {v k s l n tr} :
      Step H ⟨.drop :: k, v :: s, l, n, tr, .running⟩ ⟨k, s, l, n, tr, .running⟩
  | swap {a b k s l n tr} :
      Step H ⟨.swap :: k, b :: a :: s, l, n, tr, .running⟩ ⟨k, a :: b :: s, l, n, tr, .running⟩
  | quote {v k s l n tr} :
      Step H ⟨.quote :: k, v :: s, l, n, tr, .running⟩
        ⟨k, .prog [.push v] :: s, l, n, tr, .running⟩
  | block {p k s l n tr} :
      Step H ⟨.block p :: k, s, l, n, tr, .running⟩ ⟨k, .prog p :: s, l, n, tr, .running⟩
  | compose {p q k s l n tr} :
      Step H ⟨.compose :: k, .prog q :: .prog p :: s, l, n, tr, .running⟩
        ⟨k, .prog (p ++ q) :: s, l, n, tr, .running⟩
  | run {p k s l n tr} :
      Step H ⟨.run :: k, .prog p :: s, l, n, tr, .running⟩ ⟨p ++ k, s, l, n, tr, .running⟩
  | emitOk {x k s l n tr} : H tr.length = .permitted →
      Step H ⟨.emit :: k, .i64 x :: s, l, n, tr, .running⟩
        ⟨k, s, l, n, tr ++ [⟨.emit, x.toNat, .permitted⟩], .running⟩
  | emitStop {x k s l n tr r} : H tr.length = r → r ≠ .permitted →
      Step H ⟨.emit :: k, .i64 x :: s, l, n, tr, .running⟩
        ⟨.emit :: k, .i64 x :: s, l, n, tr ++ [⟨.emit, x.toNat, r⟩], r.halt⟩
  | acquireOk {k s l n tr} : H tr.length = .permitted →
      Step H ⟨.acquire :: k, s, l, n, tr, .running⟩
        ⟨k, .res n :: s, n :: l, n + 1, tr ++ [⟨.acquire, n, .permitted⟩], .running⟩
  | acquireStop {k s l n tr r} : H tr.length = r → r ≠ .permitted →
      Step H ⟨.acquire :: k, s, l, n, tr, .running⟩
        ⟨.acquire :: k, s, l, n, tr ++ [⟨.acquire, n, r⟩], r.halt⟩
  | releaseOk {m k s l n tr} : H tr.length = .permitted →
      Step H ⟨.release :: k, .res m :: s, l, n, tr, .running⟩
        ⟨k, s, l.erase m, n, tr ++ [⟨.release, m, .permitted⟩], .running⟩
  | releaseStop {m k s l n tr r} : H tr.length = r → r ≠ .permitted →
      Step H ⟨.release :: k, .res m :: s, l, n, tr, .running⟩
        ⟨.release :: k, .res m :: s, l, n, tr ++ [⟨.release, m, r⟩], r.halt⟩

/-- Finite execution prefixes. -/
abbrev Reach (H : Nat → Response) := Relation.ReflTransGen (Step H)

mutual
  inductive ValT : EVal → ETy → Prop where
    | i64 {x} : ValT (.i64 x) .i64
    | res {n} : ValT (.res n) .res
    | prog {p i o e} : CodeT e p i o → ValT (.prog p) (.prog i o e)

  /-- Operation typing under effect bound `E`, top-first. -/
  inductive OpT : List Label → EOp → List ETy → List ETy → Prop where
    | push {E v t s} : ValT v t → t.Copy → OpT E (.push v) s (t :: s)
    | add {E s} : OpT E .add (.i64 :: .i64 :: s) (.i64 :: s)
    | dup {E t s} : t.Copy → OpT E .dup (t :: s) (t :: t :: s)
    | drop {E t s} : t.Copy → OpT E .drop (t :: s) s
    | swap {E a b s} : OpT E .swap (b :: a :: s) (a :: b :: s)
    | quote {E t s i} : t.Copy → OpT E .quote (t :: s) (.prog i (t :: i) [] :: s)
    | block {E p i o e s} : CodeT e p i o → OpT E (.block p) s (.prog i o e :: s)
    | compose {E i m o e1 e2 s} :
        OpT E .compose (.prog m o e2 :: .prog i m e1 :: s) (.prog i o (e1 ++ e2) :: s)
    | run {E s t e} : e ⊆ E → OpT E .run (.prog s t e :: s) t
    | emit {E s} : Label.emit ∈ E → OpT E .emit (.i64 :: s) s
    | acquire {E s} : Label.acquire ∈ E → OpT E .acquire s (.res :: s)
    | release {E s} : Label.release ∈ E → OpT E .release (.res :: s) s

  inductive CodeT : List Label → List EOp → List ETy → List ETy → Prop where
    | nil {E s} : CodeT E [] s s
    | cons {E op k s m t} : OpT E op s m → CodeT E k m t → CodeT E (op :: k) s t
end

inductive StackT : List EVal → List ETy → Prop where
  | nil : StackT [] []
  | cons {v vs t ts} : ValT v t → StackT vs ts → StackT (v :: vs) (t :: ts)

/-- A well-typed configuration under effect bound `E`. -/
def WT (E : List Label) (c : Cfg) : Prop :=
  ∃ ts out, StackT c.stack ts ∧ CodeT E c.code ts out

/-! ## Structural lemmas -/

theorem OpT.weaken {E E' : List Label} {op s t} (h : OpT E op s t) (hE : E ⊆ E') :
    OpT E' op s t := by
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
  | emit h => exact .emit (hE h)
  | acquire h => exact .acquire (hE h)
  | release h => exact .release (hE h)

theorem CodeT.weaken {E E' : List Label} {p : List EOp} {s t} (h : CodeT E p s t)
    (hE : E ⊆ E') : CodeT E' p s t := by
  induction p generalizing s with
  | nil => cases h; exact .nil
  | cons op k ih =>
      cases h with
      | cons hop hk => exact .cons (hop.weaken hE) (ih hk)

theorem CodeT.append {E : List Label} {p q : List EOp} {s m t} (hp : CodeT E p s m)
    (hq : CodeT E q m t) : CodeT E (p ++ q) s t := by
  induction p generalizing s with
  | nil => cases hp; exact hq
  | cons op k ih =>
      cases hp with
      | cons hop hk => exact .cons hop (ih hk)

/-! ## Preservation and progress -/

/-- **Preservation**: every step, including a halting host step, keeps the
configuration well typed under the same bound. -/
theorem preserve {H E c c'} (hw : WT E c) (h : Step H c c') : WT E c' := by
  obtain ⟨ts, out, hs, hc⟩ := hw
  cases h with
  | push =>
      cases hc with
      | cons hop hk => cases hop with
        | push hv _ => exact ⟨_, _, .cons hv hs, hk⟩
  | add =>
      cases hc with
      | cons hop hk => cases hop with
        | add => cases hs with
          | cons _ hs1 => cases hs1 with
            | cons _ hs2 => exact ⟨_, _, .cons .i64 hs2, hk⟩
  | dup =>
      cases hc with
      | cons hop hk => cases hop with
        | dup _ => cases hs with
          | cons hv hs1 => exact ⟨_, _, .cons hv (.cons hv hs1), hk⟩
  | drop =>
      cases hc with
      | cons hop hk => cases hop with
        | drop _ => cases hs with
          | cons _ hs1 => exact ⟨_, _, hs1, hk⟩
  | swap =>
      cases hc with
      | cons hop hk => cases hop with
        | swap => cases hs with
          | cons hb hs1 => cases hs1 with
            | cons ha hs2 => exact ⟨_, _, .cons ha (.cons hb hs2), hk⟩
  | quote =>
      cases hc with
      | cons hop hk => cases hop with
        | quote hcopy => cases hs with
          | cons hv hs1 =>
              exact ⟨_, _, .cons (.prog (.cons (.push hv hcopy) .nil)) hs1, hk⟩
  | block =>
      cases hc with
      | cons hop hk => cases hop with
        | block hp => exact ⟨_, _, .cons (.prog hp) hs, hk⟩
  | compose =>
      cases hc with
      | cons hop hk => cases hop with
        | compose => cases hs with
          | cons hq hs1 => cases hs1 with
            | cons hp hs2 =>
                cases hq with
                | prog cq => cases hp with
                  | prog cp =>
                      refine ⟨_, _, .cons (.prog ?_) hs2, hk⟩
                      exact (cp.weaken (List.subset_append_left _ _)).append
                        (cq.weaken (List.subset_append_right _ _))
  | run =>
      cases hc with
      | cons hop hk => cases hop with
        | run he => cases hs with
          | cons hv hs1 => cases hv with
            | prog cp => exact ⟨_, _, hs1, (cp.weaken he).append hk⟩
  | emitOk =>
      cases hc with
      | cons hop hk => cases hop with
        | emit _ => cases hs with
          | cons _ hs1 => exact ⟨_, _, hs1, hk⟩
  | emitStop => exact ⟨ts, out, hs, hc⟩
  | acquireOk =>
      cases hc with
      | cons hop hk => cases hop with
        | acquire _ => exact ⟨_, _, .cons .res hs, hk⟩
  | acquireStop => exact ⟨ts, out, hs, hc⟩
  | releaseOk =>
      cases hc with
      | cons hop hk => cases hop with
        | release _ => cases hs with
          | cons _ hs1 => exact ⟨_, _, hs1, hk⟩
  | releaseStop => exact ⟨ts, out, hs, hc⟩

theorem preserve_reach {H E c c'} (hw : WT E c) (h : Reach H c c') : WT E c' := by
  induction h with
  | refl => exact hw
  | tail _ hstep ih => exact preserve ih hstep

private theorem i64_inv {v} (h : ValT v .i64) : ∃ x, v = .i64 x := by
  cases h; exact ⟨_, rfl⟩
private theorem res_inv {v} (h : ValT v .res) : ∃ n, v = .res n := by
  cases h; exact ⟨_, rfl⟩
private theorem prog_inv {v i o e} (h : ValT v (.prog i o e)) : ∃ p, v = .prog p := by
  cases h; exact ⟨_, rfl⟩

/-- **Progress relative to the host contract**: whatever the host answers, a
typed running configuration with remaining code has a next step. There is no
forbidden stuck state. Termination is not claimed. -/
theorem progress {H E} {c : Cfg} (hw : WT E c) (hr : c.status = .running)
    (hne : c.code ≠ []) : ∃ c', Step H c c' := by
  obtain ⟨code, stack, l, n, tr, st⟩ := c
  simp only at hr hne
  subst hr
  obtain ⟨ts, out, hs, hc⟩ := hw
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
            | cons hp _ =>
                obtain ⟨q, rfl⟩ := prog_inv hq
                obtain ⟨p, rfl⟩ := prog_inv hp
                exact ⟨_, .compose⟩
      | run =>
          cases hs with
          | cons hp _ =>
              obtain ⟨p, rfl⟩ := prog_inv hp
              exact ⟨_, .run⟩
      | emit =>
          cases hs with
          | cons hx _ =>
              obtain ⟨x, rfl⟩ := i64_inv hx
              by_cases hp : H tr.length = .permitted
              · exact ⟨_, .emitOk hp⟩
              · exact ⟨_, .emitStop rfl hp⟩
      | acquire =>
          by_cases hp : H tr.length = .permitted
          · exact ⟨_, .acquireOk hp⟩
          · exact ⟨_, .acquireStop rfl hp⟩
      | release =>
          cases hs with
          | cons hm _ =>
              obtain ⟨m, rfl⟩ := res_inv hm
              by_cases hp : H tr.length = .permitted
              · exact ⟨_, .releaseOk hp⟩
              · exact ⟨_, .releaseStop rfl hp⟩

/-- Halted configurations (denied or failed) take no further step, so no
later request is performed after a denial or failure. -/
theorem halted_no_step {H} {c c' : Cfg} (h : c.status ≠ .running) : ¬ Step H c c' := by
  intro hs
  cases hs <;> exact h rfl

/-! ## Effect soundness for every finite prefix -/

/-- One step appends at most one event, and its label is in the bound. -/
theorem step_trace {H E c c'} (hw : WT E c) (h : Step H c c') :
    c'.trace = c.trace ∨ ∃ ev, c'.trace = c.trace ++ [ev] ∧ ev.label ∈ E := by
  obtain ⟨ts, out, hs, hc⟩ := hw
  cases h with
  | emitOk | emitStop =>
      cases hc with
      | cons hop _ => cases hop with
        | emit he => exact .inr ⟨_, rfl, he⟩
  | acquireOk | acquireStop =>
      cases hc with
      | cons hop _ => cases hop with
        | acquire he => exact .inr ⟨_, rfl, he⟩
  | releaseOk | releaseStop =>
      cases hc with
      | cons hop _ => cases hop with
        | release he => exact .inr ⟨_, rfl, he⟩
  | _ => exact .inl rfl

/-- **Effect soundness for every finite execution prefix**, including denied
and failed requests. The trace only grows by appending, so earlier events
are never reordered or removed, and every new event's label is in the
declared bound `E`. -/
theorem effect_sound {H E c c'} (hw : WT E c) (h : Reach H c c') :
    ∃ new, c'.trace = c.trace ++ new ∧ ∀ ev ∈ new, ev.label ∈ E := by
  induction h with
  | refl => exact ⟨[], by simp, by simp⟩
  | @tail b d hreach hstep ih =>
      obtain ⟨new, hb, hnew⟩ := ih
      rcases step_trace (preserve_reach hw hreach) hstep with hd | ⟨ev, hd, hev⟩
      · exact ⟨new, by rw [hd, hb], hnew⟩
      · refine ⟨new ++ [ev], by rw [hd, hb, List.append_assoc], ?_⟩
        intro e he
        rcases List.mem_append.mp he with he | he
        · exact hnew e he
        · simp at he; subst he; exact hev

/-- The initial configuration of a program. -/
def init (p : List EOp) : Cfg := ⟨p, [], [], 0, [], .running⟩

theorem init_wt {E p out} (hp : CodeT E p [] out) : WT E (init p) := ⟨[], out, .nil, hp⟩

/-- From a typed program: every event of every reachable configuration is
within the bound. -/
theorem trace_within_bound {H E p out c} (hp : CodeT E p [] out) (h : Reach H (init p) c) :
    ∀ ev ∈ c.trace, ev.label ∈ E := by
  obtain ⟨new, ht, hnew⟩ := effect_sound (init_wt hp) h
  simpa [init, ht] using hnew

/-- A bound is a set, not an order: two programs typed under the same bound
`[emit]` produce different ordered traces under the same host. A matching
effect bound therefore does not justify reordering. -/
theorem bound_does_not_fix_order :
    let p₁ := [EOp.push (.i64 1), .emit, .push (.i64 2), .emit]
    let p₂ := [EOp.push (.i64 2), .emit, .push (.i64 1), .emit]
    CodeT [.emit] p₁ [] [] ∧ CodeT [.emit] p₂ [] [] ∧
    Reach (fun _ => .permitted) (init p₁)
      ⟨[], [], [], 0, [⟨.emit, 1, .permitted⟩, ⟨.emit, 2, .permitted⟩], .running⟩ ∧
    Reach (fun _ => .permitted) (init p₂)
      ⟨[], [], [], 0, [⟨.emit, 2, .permitted⟩, ⟨.emit, 1, .permitted⟩], .running⟩ ∧
    ([⟨.emit, 1, .permitted⟩, ⟨.emit, 2, .permitted⟩] : List Event) ≠
      [⟨.emit, 2, .permitted⟩, ⟨.emit, 1, .permitted⟩] := by
  have ty : ∀ a b : BitVec 64, CodeT [.emit] [EOp.push (.i64 a), .emit, .push (.i64 b), .emit] [] [] :=
    fun _ _ => .cons (.push .i64 trivial) (.cons (.emit (by simp))
      (.cons (.push .i64 trivial) (.cons (.emit (by simp)) .nil)))
  have run : ∀ a b : BitVec 64, Reach (fun _ => .permitted)
      (init [EOp.push (.i64 a), .emit, .push (.i64 b), .emit])
      ⟨[], [], [], 0, [⟨.emit, a.toNat, .permitted⟩, ⟨.emit, b.toNat, .permitted⟩], .running⟩ :=
    fun a b => (((Relation.ReflTransGen.single .push).tail (.emitOk rfl)).tail .push).tail
      (.emitOk rfl)
  exact ⟨ty 1 2, ty 2 1, run 1 2, run 2 1, by decide⟩

/-- A denied request is recorded in the trace and halts execution. The second
request is never performed. -/
theorem denied_request_recorded_and_halts :
    Reach (fun _ => .denied) (init [.push (.i64 7), .emit, .push (.i64 8), .emit])
      ⟨[.emit, .push (.i64 8), .emit], [.i64 7], [], 0, [⟨.emit, 7, .denied⟩], .denied⟩ ∧
    ∀ c', ¬ Step (fun _ => .denied)
      ⟨[.emit, .push (.i64 8), .emit], [.i64 7], [], 0, [⟨.emit, 7, .denied⟩], .denied⟩ c' := by
  refine ⟨(Relation.ReflTransGen.single .push).tail (.emitStop rfl (by decide)), ?_⟩
  intro c' h
  exact halted_no_step (by decide) h

/-! ## Ownership -/

/-- Resource ids held at the top level of the stack. -/
def topIds : List EVal → List Nat
  | [] => []
  | .res n :: vs => n :: topIds vs
  | .i64 _ :: vs => topIds vs
  | .prog _ :: vs => topIds vs

/-- Ownership invariant. The resources on the stack are exactly the live set,
with no duplicates, and every live id is below the fresh-id counter. -/
structure Own (c : Cfg) : Prop where
  perm : (topIds c.stack).Perm c.live
  nodup : c.live.Nodup
  fresh : ∀ m ∈ c.live, m < c.next

theorem topIds_copy {v t} (hv : ValT v t) (hc : t.Copy) (s : List EVal) :
    topIds (v :: s) = topIds s := by
  cases hv with
  | i64 => rfl
  | res => exact (hc : False).elim
  | prog _ => rfl

/-- **Ownership preservation** modulo the declared transitions: on typed
configurations, every step keeps the stack's resources equal to the live set
with no duplicate. -/
theorem own_preserve {H E c c'} (hw : WT E c) (ho : Own c) (h : Step H c c') : Own c' := by
  obtain ⟨ts, out, hs, hc⟩ := hw
  obtain ⟨hp, hn, hf⟩ := ho
  cases h with
  | push =>
      cases hc with
      | cons hop _ => cases hop with
        | push hv hcopy => exact ⟨by rw [topIds_copy hv hcopy]; exact hp, hn, hf⟩
  | add => exact ⟨by simpa [topIds] using hp, hn, hf⟩
  | dup =>
      cases hc with
      | cons hop _ => cases hop with
        | dup hcopy => cases hs with
          | cons hv _ =>
              refine ⟨?_, hn, hf⟩
              rw [topIds_copy hv hcopy, topIds_copy hv hcopy]
              rw [topIds_copy hv hcopy] at hp; exact hp
  | drop =>
      cases hc with
      | cons hop _ => cases hop with
        | drop hcopy => cases hs with
          | cons hv _ => exact ⟨by rw [topIds_copy hv hcopy] at hp; exact hp, hn, hf⟩
  | @swap a b k s l n tr =>
      refine ⟨?_, hn, hf⟩
      refine List.Perm.trans ?_ hp
      cases a <;> cases b <;> simp [topIds, List.Perm.swap]
  | quote =>
      cases hc with
      | cons hop _ => cases hop with
        | quote hcopy => cases hs with
          | cons hv _ =>
              exact ⟨by rw [topIds_copy hv hcopy] at hp; simpa [topIds] using hp, hn, hf⟩
  | block => exact ⟨by simpa [topIds] using hp, hn, hf⟩
  | compose => exact ⟨by simpa [topIds] using hp, hn, hf⟩
  | run => exact ⟨by simpa [topIds] using hp, hn, hf⟩
  | emitOk => exact ⟨by simpa [topIds] using hp, hn, hf⟩
  | emitStop => exact ⟨hp, hn, hf⟩
  | @acquireOk k s l n tr _ =>
      refine ⟨by simpa [topIds] using hp, ?_, ?_⟩
      · exact List.nodup_cons.mpr ⟨fun hm => Nat.lt_irrefl _ (hf n hm), hn⟩
      · intro m hm
        rcases List.mem_cons.mp hm with rfl | hm
        · exact Nat.lt_succ_self _
        · exact Nat.lt_succ_of_lt (hf m hm)
  | acquireStop => exact ⟨hp, hn, hf⟩
  | @releaseOk m k s l n tr _ =>
      have hp' : (m :: topIds s).Perm l := by simpa [topIds] using hp
      have hmem : m ∈ l := hp'.subset (List.mem_cons_self ..)
      refine ⟨?_, hn.erase m, fun x hx => hf x (List.mem_of_mem_erase hx)⟩
      exact (hp'.trans (List.perm_cons_erase hmem)).cons_inv
  | releaseStop => exact ⟨hp, hn, hf⟩

/-- The host-tracked live set changes only through a recorded, permitted
`acquire` (fresh id added) or `release` (id removed). Moves are pure stack
permutations and leave it unchanged. -/
theorem live_change {H c c'} (h : Step H c c') :
    c'.live = c.live ∨ ∃ ev, c'.trace = c.trace ++ [ev] ∧ ev.response = .permitted ∧
      ((ev.label = .acquire ∧ ev.arg = c.next ∧ c'.live = ev.arg :: c.live) ∨
       (ev.label = .release ∧ c'.live = c.live.erase ev.arg)) := by
  cases h with
  | acquireOk => exact .inr ⟨_, rfl, rfl, .inl ⟨rfl, rfl, rfl⟩⟩
  | releaseOk => exact .inr ⟨_, rfl, rfl, .inr ⟨rfl, rfl⟩⟩
  | _ => exact .inl rfl

/-! ## No resource inside programs or continuations -/

mutual
  inductive RFreeV : EVal → Prop where
    | i64 {x} : RFreeV (.i64 x)
    | prog {p} : RFreeC p → RFreeV (.prog p)

  inductive RFreeO : EOp → Prop where
    | push {v} : RFreeV v → RFreeO (.push v)
    | block {p} : RFreeC p → RFreeO (.block p)
    | add : RFreeO .add
    | dup : RFreeO .dup
    | drop : RFreeO .drop
    | swap : RFreeO .swap
    | quote : RFreeO .quote
    | compose : RFreeO .compose
    | run : RFreeO .run
    | emit : RFreeO .emit
    | acquire : RFreeO .acquire
    | release : RFreeO .release

  inductive RFreeC : List EOp → Prop where
    | nil : RFreeC []
    | cons {op k} : RFreeO op → RFreeC k → RFreeC (op :: k)
end

mutual
  theorem ValT.rfree : {v : EVal} → {t : ETy} → ValT v t → t.Copy → RFreeV v
    | _, _, .i64, _ => .i64
    | _, _, .res, h => (h : False).elim
    | _, _, .prog hc, _ => .prog (CodeT.rfree hc)

  theorem OpT.rfree : {E : List Label} → {op : EOp} → {s t : List ETy} →
      OpT E op s t → RFreeO op
    | _, _, _, _, .push hv hc => .push (ValT.rfree hv hc)
    | _, _, _, _, .block hc => .block (CodeT.rfree hc)
    | _, _, _, _, .add => .add
    | _, _, _, _, .dup _ => .dup
    | _, _, _, _, .drop _ => .drop
    | _, _, _, _, .swap => .swap
    | _, _, _, _, .quote _ => .quote
    | _, _, _, _, .compose => .compose
    | _, _, _, _, .run _ => .run
    | _, _, _, _, .emit _ => .emit
    | _, _, _, _, .acquire _ => .acquire
    | _, _, _, _, .release _ => .release

  theorem CodeT.rfree : {E : List Label} → {p : List EOp} → {s t : List ETy} →
      CodeT E p s t → RFreeC p
    | _, _, _, _, .nil => .nil
    | _, _, _, _, .cons hop hk => .cons (OpT.rfree hop) (CodeT.rfree hk)
end

theorem StackT.rfree {vs ts} (h : StackT vs ts) : ∀ v ∈ vs, (∃ n, v = .res n) ∨ RFreeV v := by
  induction h with
  | nil => simp
  | @cons v vs t ts hv _ ih =>
      intro w hw
      rcases List.mem_cons.mp hw with rfl | hw
      · cases hv with
        | i64 => exact .inr .i64
        | res => exact .inl ⟨_, rfl⟩
        | prog hc => exact .inr (.prog (CodeT.rfree hc))
      · exact ih w hw

/-- **Recursive capture eligibility** for typed configurations. The
continuation (frames) and every program value on the stack, including nested
programs, contain no resource. Resources occur only as top-level stack
values. -/
theorem typed_rfree {E c} (hw : WT E c) :
    RFreeC c.code ∧ ∀ v ∈ c.stack, (∃ n, v = .res n) ∨ RFreeV v := by
  obtain ⟨_, _, hs, hc⟩ := hw
  exact ⟨CodeT.rfree hc, StackT.rfree hs⟩

/-- **Whole-execution theorem** for a typed program. In every finite prefix
(including halting at a denied or failed request):
- every event is within the bound;
- the live resources are exactly the stack's resources, without duplicates;
- no program value or continuation contains a resource;
- the configuration is still well typed. -/
theorem reachable_sound {H E p out c} (hp : CodeT E p [] out) (h : Reach H (init p) c) :
    WT E c ∧ (∀ ev ∈ c.trace, ev.label ∈ E) ∧ Own c ∧ RFreeC c.code ∧
      ∀ v ∈ c.stack, (∃ n, v = .res n) ∨ RFreeV v := by
  have hown : Own c := by
    induction h with
    | refl => exact ⟨by simp [init, topIds], by simp [init], by simp [init]⟩
    | tail hr hs ih => exact own_preserve (preserve_reach (init_wt hp) hr) ih hs
  have hw := preserve_reach (init_wt hp) h
  exact ⟨hw, trace_within_bound hp h, hown, typed_rfree hw⟩

/-! ## Resource negative cases: typing rejects, and untyped runs break ownership -/

theorem dup_res_untypable {E s out} : ¬ OpT E .dup (.res :: s) out := by
  intro h; cases h with | dup hc => exact hc

theorem drop_res_untypable {E s out} : ¬ OpT E .drop (.res :: s) out := by
  intro h; cases h with | drop hc => exact hc

theorem quote_res_untypable {E s out} : ¬ OpT E .quote (.res :: s) out := by
  intro h; cases h with | quote hc => exact hc

theorem push_res_untypable {E n s out} : ¬ OpT E (.push (.res n)) s out := by
  intro h; cases h with | push hv hc => cases hv; exact hc

private def perm : Nat → Response := fun _ => .permitted

/-- Untyped duplication: `acquire dup` holds one live resource twice. -/
theorem untyped_dup_breaks_ownership :
    Reach perm (init [.acquire, .dup])
      ⟨[], [.res 0, .res 0], [0], 1, [⟨.acquire, 0, .permitted⟩], .running⟩ ∧
    ¬ Own ⟨[], [.res 0, .res 0], [0], 1, [⟨.acquire, 0, .permitted⟩], .running⟩ := by
  refine ⟨(Relation.ReflTransGen.single (.acquireOk rfl)).tail .dup, ?_⟩
  intro h; have := h.perm.length_eq; simp [topIds] at this

/-- Untyped discard: `acquire drop` leaks a live resource that no stack value
holds. -/
theorem untyped_drop_breaks_ownership :
    Reach perm (init [.acquire, .drop])
      ⟨[], [], [0], 1, [⟨.acquire, 0, .permitted⟩], .running⟩ ∧
    ¬ Own ⟨[], [], [0], 1, [⟨.acquire, 0, .permitted⟩], .running⟩ := by
  refine ⟨(Relation.ReflTransGen.single (.acquireOk rfl)).tail .drop, ?_⟩
  intro h; have := h.perm.length_eq; simp [topIds] at this

/-- Untyped capture: `acquire quote` stores a live resource inside a program
value. -/
theorem untyped_capture_breaks_ownership :
    Reach perm (init [.acquire, .quote])
      ⟨[], [.prog [.push (.res 0)]], [0], 1, [⟨.acquire, 0, .permitted⟩], .running⟩ ∧
    ¬ Own ⟨[], [.prog [.push (.res 0)]], [0], 1, [⟨.acquire, 0, .permitted⟩], .running⟩ ∧
    ¬ RFreeV (.prog [.push (.res 0)]) := by
  refine ⟨(Relation.ReflTransGen.single (.acquireOk rfl)).tail .quote, ?_, ?_⟩
  · intro h; have := h.perm.length_eq; simp [topIds] at this
  · intro h
    cases h with
    | prog hc => cases hc with
      | cons ho _ => cases ho with
        | push hv => cases hv

/-- Positive control: moving a resource (`swap`) and releasing it is typable
and ends with no live resource. -/
theorem move_then_release :
    CodeT [.acquire, .release] [.acquire, .push (.i64 5), .swap, .release, .drop] [] [] ∧
    Reach perm (init [.acquire, .push (.i64 5), .swap, .release, .drop])
      ⟨[], [], [], 1, [⟨.acquire, 0, .permitted⟩, ⟨.release, 0, .permitted⟩], .running⟩ := by
  refine ⟨.cons (.acquire (by simp)) (.cons (.push .i64 trivial) (.cons .swap
    (.cons (.release (by simp)) (.cons (.drop trivial) .nil)))), ?_⟩
  exact ((((Relation.ReflTransGen.single (.acquireOk rfl)).tail .push).tail .swap).tail
    (.releaseOk rfl)).tail .drop

/-- Programs are duplicable data even when their interface expects a
resource: `prog [res] [] [release]` is copyable. -/
theorem program_expecting_resource_is_copyable :
    (ETy.prog [.res] [] [.release]).Copy ∧
    OpT [] .dup (.prog [.res] [] [.release] :: []) (.prog [.res] [] [.release] ::
      .prog [.res] [] [.release] :: []) :=
  ⟨trivial, .dup trivial⟩

end NobleFoundation.Effects
