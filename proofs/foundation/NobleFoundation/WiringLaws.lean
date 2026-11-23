import NobleFoundation.OwnershipModel

/-!
# Trace, ownership and failure models for the wiring laws

This module develops the three observation models that
`resource-safe-wiring-laws` names for the T, O and F columns of its law
matrix, over the extended ownership language of
`NobleFoundation/OwnershipModel.lean`:

* **T**, ordered host-request traces over every finite prefix, including
  denied and failed requests (`step_trace`, `effect_sound`,
  `trace_prefix`);
* **O**, whole-configuration resource ownership transitions and
  multiplicity, reusing `OwnershipModel.Own`/`owners_step` and adding the
  derived wiring words' frame and move accounting;
* **F**, failure class and cleanup responsibility, with a separately
  modelled host boundary (`Class`, `Boundary`, `retire`, `retire_idempotent`,
  `retired_never_live_again`).

The module also records, as model theorems, the per-law T/O/F statements
the design lists for L1-L14, including the L9 trace and failure
counterexamples. Every theorem is a statement about this model only. The
resolved law-matrix entries stay unclaimed until resource-admitting
execution exists; see the change's `tasks.md` progress note.

The wiring words are encoded as instruction sequences in the existing
language, so every ownership statement is an instance of
`OwnershipModel.owners_step`: there is no second ownership judgment.
-/

namespace NobleFoundation.WiringLaws

open NobleFoundation.Effects (Label Response Status Event)
open NobleFoundation.OwnershipModel

set_option linter.defProp false

/-! ## T: ordered traces over every finite prefix -/

/-- Every small step either leaves the trace alone or appends exactly one
event. This is total: it does not depend on typing, so it also covers
halting requests. -/
theorem step_trace_any {D H c c'} (h : Step D H c c') :
    c'.trace = c.trace ∨ ∃ ev, c'.trace = c.trace ++ [ev] := by
  cases h <;> first
    | exact .inl rfl
    | exact .inr ⟨_, rfl⟩

/-- One step either leaves the trace alone or appends exactly one event,
whose label is in the declared effect bound. -/
theorem step_trace {D H E c c'} (hw : WT D E c) (h : Step D H c c') :
    c'.trace = c.trace ∨ ∃ ev, c'.trace = c.trace ++ [ev] ∧ ev.label ∈ E := by
  obtain ⟨ts, out, hs, hc, hh⟩ := hw
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

/-- **Effect soundness for every finite execution prefix**, including
denied and failed requests. The trace only grows by appending, so earlier
events are never reordered or removed, and every new event's label is in
the declared bound. -/
theorem effect_sound {D H E c c'} (hw : WT D E c) (h : Reach D H c c') :
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
        · simp at he
          subst he
          exact hev

/-- From a typed program: every event of every reachable configuration is
within the bound. -/
theorem trace_within_bound {D H E p out c} (hp : CodeT D E p [] out)
    (h : Reach D H (init p) c) : ∀ ev ∈ c.trace, ev.label ∈ E := by
  obtain ⟨new, ht, hnew⟩ := effect_sound (init_wt hp) h
  simpa [init, ht] using hnew

/-- A trace only grows by appending: any reached trace has the starting
trace as a prefix. -/
theorem trace_prefix {D H c c'} (h : Reach D H c c') :
    ∃ new, c'.trace = c.trace ++ new := by
  induction h with
  | refl => exact ⟨[], by simp⟩
  | tail _ hstep ih =>
      obtain ⟨new, hnew⟩ := ih
      rcases step_trace_any hstep with hsame | ⟨ev, hsame⟩
      · exact ⟨new, by rw [hsame, hnew]⟩
      · exact ⟨new ++ [ev], by rw [hsame, hnew, List.append_assoc]⟩

/-- Halted configurations (denied or failed) take no further step, so no
later request is performed after a denial or failure. -/
theorem halted_no_step {D H} {c c' : Config} (h : c.status ≠ .running) :
    ¬ Step D H c c' := by
  intro hs
  cases hs <;> exact h rfl

/-! ## Code prefixes: appending an untouched suffix to every step -/

/-- Every small step commutes with appending an untouched code suffix to the
continuation. This is what makes `p ++ q` run `p` to completion and then
`q`, with `q` never entered early. -/
theorem step_append_code {D H c c'} (h : Step D H c c') (q : List Instr) :
    Step D H { c with code := c.code ++ q } { c' with code := c'.code ++ q } := by
  cases h with
  | push => exact .push
  | add => exact .add
  | dup => exact .dup
  | drop => exact .drop
  | swap => exact .swap
  | quote => exact .quote
  | block => exact .block
  | compose => exact .compose
  | run => simpa [List.append_assoc] using
      (Step.run (D := D) (H := H) (k := ‹List Instr› ++ q))
  | pin => exact .pin
  | dip => simpa [List.append_assoc] using
      (Step.dip (D := D) (H := H) (k := ‹List Instr› ++ q))
  | restore => exact .restore
  | ifTrue hx => simpa [List.append_assoc] using
      (Step.ifTrue (D := D) (H := H) (k := ‹List Instr› ++ q) hx)
  | ifFalse hx => simpa [List.append_assoc] using
      (Step.ifFalse (D := D) (H := H) (k := ‹List Instr› ++ q) hx)
  | pair => exact .pair
  | unpair => exact .unpair
  | inl => exact .inl
  | inr => exact .inr
  | casesLeft => simpa [List.append_assoc] using
      (Step.casesLeft (D := D) (H := H) (k := ‹List Instr› ++ q))
  | casesRight => simpa [List.append_assoc] using
      (Step.casesRight (D := D) (H := H) (k := ‹List Instr› ++ q))
  | nil => exact .nil
  | cons => exact .cons
  | listCaseNil => simpa [List.append_assoc] using
      (Step.listCaseNil (D := D) (H := H) (k := ‹List Instr› ++ q))
  | listCaseCons => simpa [List.append_assoc] using
      (Step.listCaseCons (D := D) (H := H) (k := ‹List Instr› ++ q))
  | alloc => exact .alloc
  | call hd => simpa [List.append_assoc] using
      (Step.call (D := D) (H := H) (k := ‹List Instr› ++ q) hd)
  | emitOk h => exact .emitOk h
  | emitStop h r => exact .emitStop h r
  | acquireOk h => exact .acquireOk h
  | acquireStop h r => exact .acquireStop h r
  | releaseOk h => exact .releaseOk h
  | releaseStop h r => exact .releaseStop h r

/-- Reachability commutes with appending an untouched code suffix. -/
theorem reach_append_code {D H c c'} (h : Reach D H c c') (q : List Instr) :
    Reach D H { c with code := c.code ++ q } { c' with code := c'.code ++ q } := by
  induction h with
  | refl => exact .refl
  | tail _ hstep ih => exact ih.tail (step_append_code hstep q)

/-- A finite normal execution of `p` continuing a trace `tr0`: it appends
`tr` to `tr0`, starts and ends with an empty continuation and `running`
status. The resource tuple is explicit so composition aligns the states. -/
def ExecFrom (D : List Def) (H : Nat → Response) (p : List Instr) (s : List Value)
    (l : List Nat) (cs : List Nat) (hp : List (Nat × Value)) (n : Nat)
    (tr0 : List Event) (tr : List Event) (t : List Value) (l' : List Nat)
    (cs' : List Nat) (hp' : List (Nat × Value)) (n' : Nat) : Prop :=
  Reach D H ⟨p, s, l, cs, hp, n, tr0, .running⟩
    ⟨[], t, l', cs', hp', n', tr0 ++ tr, .running⟩

/-- The empty-history form of `ExecFrom`. -/
def Exec (D : List Def) (H : Nat → Response) (p : List Instr) (s : List Value)
    (l : List Nat) (cs : List Nat) (hp : List (Nat × Value)) (n : Nat)
    (tr : List Event) (t : List Value) (l' : List Nat) (cs' : List Nat)
    (hp' : List (Nat × Value)) (n' : Nat) : Prop :=
  ExecFrom D H p s l cs hp n [] tr t l' cs' hp' n'

/-- Composition of two finite normal executions, with the trace being the
left trace followed by the right trace. -/
theorem execFrom_append {D H p q s m t l cs hp n tr0 tr1 l1 cs1 hp1 n1 tr2 l2 cs2 hp2 n2}
    (hpRun : ExecFrom D H p s l cs hp n tr0 tr1 m l1 cs1 hp1 n1)
    (hqRun : ExecFrom D H q m l1 cs1 hp1 n1 (tr0 ++ tr1) tr2 t l2 cs2 hp2 n2) :
    ExecFrom D H (p ++ q) s l cs hp n tr0 (tr1 ++ tr2) t l2 cs2 hp2 n2 := by
  have lifted : Reach D H ⟨p ++ q, s, l, cs, hp, n, tr0, .running⟩
      ⟨q, m, l1, cs1, hp1, n1, tr0 ++ tr1, .running⟩ := by
    simpa using reach_append_code hpRun q
  simpa [ExecFrom, List.append_assoc] using lifted.trans hqRun

/-- A single step is a one-step reach. -/
theorem reach_step {D H c c'} (h : Step D H c c') : Reach D H c c' :=
  Relation.ReflTransGen.single h



/-! ## Single-word executions

Each lemma is one source (or run-time frame) word over an arbitrary trace
history `tr0`, so the words compose with `execFrom_append`. The `trace` is
unchanged: construction words issue no host request. -/

theorem push_execFrom (v : Value) (s : List Value) (l : List Nat) (cs : List Nat)
    (hp : List (Nat × Value)) (n : Nat) (tr0 : List Event) :
    ExecFrom D H [.push v] s l cs hp n tr0 [] (v :: s) l cs hp n := by
  simpa [ExecFrom] using reach_step (D := D) (H := H) (Step.push (v := v) (k := [])
    (s := s) (l := l) (cs := cs) (hp := hp) (n := n) (tr := tr0))

theorem dup_execFrom (v : Value) (s : List Value) (l : List Nat) (cs : List Nat)
    (hp : List (Nat × Value)) (n : Nat) (tr0 : List Event) :
    ExecFrom D H [.dup] (v :: s) l cs hp n tr0 [] (v :: v :: s) l cs hp n := by
  simpa [ExecFrom] using reach_step (D := D) (H := H) (Step.dup (v := v) (k := [])
    (s := s) (l := l) (cs := cs) (hp := hp) (n := n) (tr := tr0))

theorem swap_execFrom (a b : Value) (s : List Value) (l : List Nat) (cs : List Nat)
    (hp : List (Nat × Value)) (n : Nat) (tr0 : List Event) :
    ExecFrom D H [.swap] (b :: a :: s) l cs hp n tr0 [] (a :: b :: s) l cs hp n := by
  simpa [ExecFrom] using reach_step (D := D) (H := H) (Step.swap (a := a) (b := b)
    (k := []) (s := s) (l := l) (cs := cs) (hp := hp) (n := n) (tr := tr0))

theorem drop_execFrom (v : Value) (s : List Value) (l : List Nat) (cs : List Nat)
    (hp : List (Nat × Value)) (n : Nat) (tr0 : List Event) :
    ExecFrom D H [.drop] (v :: s) l cs hp n tr0 [] s l cs hp n := by
  simpa [ExecFrom] using reach_step (D := D) (H := H) (Step.drop (v := v) (k := [])
    (s := s) (l := l) (cs := cs) (hp := hp) (n := n) (tr := tr0))

theorem pair_execFrom (a b : Value) (s : List Value) (l : List Nat) (cs : List Nat)
    (hp : List (Nat × Value)) (n : Nat) (tr0 : List Event) :
    ExecFrom D H [.pair] (a :: b :: s) l cs hp n tr0 [] (.pair a b :: s) l cs hp n := by
  simpa [ExecFrom] using reach_step (D := D) (H := H) (Step.pair (a := a) (b := b)
    (k := []) (s := s) (l := l) (cs := cs) (hp := hp) (n := n) (tr := tr0))

theorem unpair_execFrom (a b : Value) (s : List Value) (l : List Nat) (cs : List Nat)
    (hp : List (Nat × Value)) (n : Nat) (tr0 : List Event) :
    ExecFrom D H [.unpair] (.pair a b :: s) l cs hp n tr0 [] (a :: b :: s) l cs hp n := by
  simpa [ExecFrom] using reach_step (D := D) (H := H) (Step.unpair (a := a) (b := b)
    (k := []) (s := s) (l := l) (cs := cs) (hp := hp) (n := n) (tr := tr0))

theorem inl_execFrom (v : Value) (s : List Value) (l : List Nat) (cs : List Nat)
    (hp : List (Nat × Value)) (n : Nat) (tr0 : List Event) :
    ExecFrom D H [.inl] (v :: s) l cs hp n tr0 [] (.inl v :: s) l cs hp n := by
  simpa [ExecFrom] using reach_step (D := D) (H := H) (Step.inl (v := v) (k := [])
    (s := s) (l := l) (cs := cs) (hp := hp) (n := n) (tr := tr0))

theorem inr_execFrom (v : Value) (s : List Value) (l : List Nat) (cs : List Nat)
    (hp : List (Nat × Value)) (n : Nat) (tr0 : List Event) :
    ExecFrom D H [.inr] (v :: s) l cs hp n tr0 [] (.inr v :: s) l cs hp n := by
  simpa [ExecFrom] using reach_step (D := D) (H := H) (Step.inr (v := v) (k := [])
    (s := s) (l := l) (cs := cs) (hp := hp) (n := n) (tr := tr0))

theorem block_execFrom (p : List Instr) (s : List Value) (l : List Nat) (cs : List Nat)
    (hp : List (Nat × Value)) (n : Nat) (tr0 : List Event) :
    ExecFrom D H [.block p] s l cs hp n tr0 [] (.prog p :: s) l cs hp n := by
  simpa [ExecFrom] using reach_step (D := D) (H := H) (Step.block (p := p) (k := [])
    (s := s) (l := l) (cs := cs) (hp := hp) (n := n) (tr := tr0))

theorem restore_execFrom (v : Value) (s : List Value) (l : List Nat) (cs : List Nat)
    (hp : List (Nat × Value)) (n : Nat) (tr0 : List Event) :
    ExecFrom D H [.restore v] s l cs hp n tr0 [] (v :: s) l cs hp n := by
  simpa [ExecFrom] using reach_step (D := D) (H := H) (Step.restore (v := v) (k := [])
    (s := s) (l := l) (cs := cs) (hp := hp) (n := n) (tr := tr0))

/-- `dip` with a static body: it moves the value below the body into a
`restore` frame, runs the body, then restores the value exactly once. -/
theorem dip_execFrom (p : List Instr) (v : Value) (s : List Value) (l : List Nat)
    (cs : List Nat) (hp : List (Nat × Value)) (n : Nat) (tr0 tr : List Event)
    (t : List Value) (l' : List Nat) (cs' : List Nat) (hp' : List (Nat × Value))
    (n' : Nat) (hrun : ExecFrom D H p s l cs hp n tr0 tr t l' cs' hp' n') :
    ExecFrom D H [.block p, .dip] (v :: s) l cs hp n tr0 tr (v :: t) l' cs' hp' n' := by
  have hblock : Reach D H ⟨[.block p, .dip], v :: s, l, cs, hp, n, tr0, .running⟩
      ⟨[.dip], .prog p :: v :: s, l, cs, hp, n, tr0, .running⟩ :=
    reach_step .block
  have hdip : Reach D H ⟨[.dip], .prog p :: v :: s, l, cs, hp, n, tr0, .running⟩
      ⟨p ++ [.restore v], s, l, cs, hp, n, tr0, .running⟩ :=
    reach_step (Step.dip (p := p) (v := v) (k := []) (s := s) (l := l) (cs := cs)
      (hp := hp) (n := n) (tr := tr0))
  have hbody : Reach D H ⟨p ++ [.restore v], s, l, cs, hp, n, tr0, .running⟩
      ⟨[.restore v], t, l', cs', hp', n', tr0 ++ tr, .running⟩ := by
    simpa using reach_append_code hrun [.restore v]
  have hrestore : Reach D H ⟨[.restore v], t, l', cs', hp', n', tr0 ++ tr, .running⟩
      ⟨[], v :: t, l', cs', hp', n', tr0 ++ tr, .running⟩ :=
    reach_step .restore
  exact hblock.trans (hdip.trans (hbody.trans hrestore))

/-- A dynamic `dip`: the program value on the stack is executed. This is the
form the wiring words use, since their program operand is a stack value. -/
theorem dipValue_execFrom (p : List Instr) (v : Value) (s : List Value)
    (l : List Nat) (cs : List Nat) (hp : List (Nat × Value)) (n : Nat)
    (tr0 tr : List Event) (t : List Value) (l' : List Nat) (cs' : List Nat)
    (hp' : List (Nat × Value)) (n' : Nat)
    (hrun : ExecFrom D H p s l cs hp n tr0 tr t l' cs' hp' n') :
    ExecFrom D H [.dip] (.prog p :: v :: s) l cs hp n tr0 tr (v :: t) l' cs' hp' n' := by
  have hdip : Reach D H ⟨[.dip], .prog p :: v :: s, l, cs, hp, n, tr0, .running⟩
      ⟨p ++ [.restore v], s, l, cs, hp, n, tr0, .running⟩ :=
    reach_step (Step.dip (p := p) (v := v) (k := []) (s := s) (l := l) (cs := cs)
      (hp := hp) (n := n) (tr := tr0))
  have hbody : Reach D H ⟨p ++ [.restore v], s, l, cs, hp, n, tr0, .running⟩
      ⟨[.restore v], t, l', cs', hp', n', tr0 ++ tr, .running⟩ := by
    simpa using reach_append_code hrun [.restore v]
  have hrestore : Reach D H ⟨[.restore v], t, l', cs', hp', n', tr0 ++ tr, .running⟩
      ⟨[], v :: t, l', cs', hp', n', tr0 ++ tr, .running⟩ := reach_step .restore
  exact hdip.trans (hbody.trans hrestore)

/-- `run` of a Program value already on the stack: execute its body, then
pop the `pin` frame that retains the value while the body runs. -/
theorem runValue_execFrom (p : List Instr) (s : List Value) (l : List Nat)
    (cs : List Nat) (hp : List (Nat × Value)) (n : Nat) (tr0 tr : List Event)
    (t : List Value) (l' : List Nat) (cs' : List Nat) (hp' : List (Nat × Value))
    (n' : Nat) (hrun : ExecFrom D H p s l cs hp n tr0 tr t l' cs' hp' n') :
    ExecFrom D H [.run] (.prog p :: s) l cs hp n tr0 tr t l' cs' hp' n' := by
  have hrunStep : Reach D H ⟨[.run], .prog p :: s, l, cs, hp, n, tr0, .running⟩
      ⟨p ++ [.pin (.prog p)], s, l, cs, hp, n, tr0, .running⟩ :=
    reach_step (Step.run (p := p) (k := []) (s := s) (l := l) (cs := cs) (hp := hp)
      (n := n) (tr := tr0))
  have hbody : Reach D H ⟨p ++ [.pin (.prog p)], s, l, cs, hp, n, tr0, .running⟩
      ⟨[.pin (.prog p)], t, l', cs', hp', n', tr0 ++ tr, .running⟩ := by
    simpa using reach_append_code (show Reach D H
      ⟨p, s, l, cs, hp, n, tr0, .running⟩
      ⟨[], t, l', cs', hp', n', tr0 ++ tr, .running⟩ from hrun) [.pin (.prog p)]
  have hpin : Reach D H ⟨[.pin (.prog p)], t, l', cs', hp', n', tr0 ++ tr, .running⟩
      ⟨[], t, l', cs', hp', n', tr0 ++ tr, .running⟩ := reach_step .pin
  exact hrunStep.trans (hbody.trans hpin)

/-- `push (prog p)` then `run` executes `p`. -/
theorem run_execFrom (p : List Instr) (s : List Value) (l : List Nat) (cs : List Nat)
    (hp : List (Nat × Value)) (n : Nat) (tr0 tr : List Event) (t : List Value)
    (l' : List Nat) (cs' : List Nat) (hp' : List (Nat × Value)) (n' : Nat)
    (hrun : ExecFrom D H p s l cs hp n tr0 tr t l' cs' hp' n') :
    ExecFrom D H [.push (.prog p), .run] s l cs hp n tr0 tr t l' cs' hp' n' :=
  execFrom_append (push_execFrom (.prog p) s l cs hp n tr0)
    (by simpa using runValue_execFrom p s l cs hp n tr0 tr t l' cs' hp' n' hrun)

/-- `quote` then `run`: the quotation of a constant value pushes it back and
issues no host request. -/
theorem quote_run_execFrom (v : Value) (s : List Value) (l : List Nat)
    (cs : List Nat) (hp : List (Nat × Value)) (n : Nat) (tr0 : List Event) :
    ExecFrom D H [.push v, .quote, .run] s l cs hp n tr0 [] (v :: s) l cs hp n := by
  have hpush : Reach D H ⟨[.push v, .quote, .run], s, l, cs, hp, n, tr0, .running⟩
      ⟨[.quote, .run], v :: s, l, cs, hp, n, tr0, .running⟩ := reach_step .push
  have hquote : Reach D H ⟨[.quote, .run], v :: s, l, cs, hp, n, tr0, .running⟩
      ⟨[.run], .prog [.push v] :: s, l, cs, hp, n, tr0, .running⟩ := reach_step .quote
  exact hpush.trans (hquote.trans (by
    simpa [ExecFrom] using runValue_execFrom [.push v] s l cs hp n tr0 [] (v :: s)
      l cs hp n (push_execFrom v s l cs hp n tr0)))

/-- `compose` two Program values then `run`: the left program runs to
completion, then the right, with the trace concatenated in that order. -/
theorem compose_run_execFrom (p q : List Instr) (s : List Value) (l : List Nat)
    (cs : List Nat) (hp : List (Nat × Value)) (n : Nat) (tr0 tr1 : List Event)
    (m : List Value) (l1 : List Nat) (cs1 : List Nat) (hp1 : List (Nat × Value))
    (n1 : Nat) (tr2 : List Event) (t : List Value) (l2 : List Nat) (cs2 : List Nat)
    (hp2 : List (Nat × Value)) (n2 : Nat)
    (hpRun : ExecFrom D H p s l cs hp n tr0 tr1 m l1 cs1 hp1 n1)
    (hqRun : ExecFrom D H q m l1 cs1 hp1 n1 (tr0 ++ tr1) tr2 t l2 cs2 hp2 n2) :
    ExecFrom D H [.push (.prog p), .push (.prog q), .compose, .run] s l cs hp n tr0
      (tr1 ++ tr2) t l2 cs2 hp2 n2 := by
  have hpushP : Reach D H ⟨[.push (.prog p), .push (.prog q), .compose, .run], s,
      l, cs, hp, n, tr0, .running⟩
      ⟨[.push (.prog q), .compose, .run], .prog p :: s, l, cs, hp, n, tr0, .running⟩ :=
    reach_step .push
  have hpushQ : Reach D H ⟨[.push (.prog q), .compose, .run], .prog p :: s,
      l, cs, hp, n, tr0, .running⟩
      ⟨[.compose, .run], .prog q :: .prog p :: s, l, cs, hp, n, tr0, .running⟩ :=
    reach_step .push
  have hcompose : Reach D H ⟨[.compose, .run], .prog q :: .prog p :: s, l, cs, hp, n,
      tr0, .running⟩
      ⟨[.run], .prog (p ++ q) :: s, l, cs, hp, n, tr0, .running⟩ := reach_step .compose
  have hrunStep : Reach D H ⟨[.run], .prog (p ++ q) :: s, l, cs, hp, n, tr0, .running⟩
      ⟨(p ++ q) ++ [.pin (.prog (p ++ q))], s, l, cs, hp, n, tr0, .running⟩ :=
    reach_step (Step.run (p := p ++ q) (k := []) (s := s) (l := l) (cs := cs)
      (hp := hp) (n := n) (tr := tr0))
  have hbody : Reach D H ⟨(p ++ q) ++ [.pin (.prog (p ++ q))], s, l, cs, hp, n, tr0,
      .running⟩
      ⟨[.pin (.prog (p ++ q))], t, l2, cs2, hp2, n2, tr0 ++ (tr1 ++ tr2), .running⟩ := by
    have hcomp : Reach D H ⟨p ++ q, s, l, cs, hp, n, tr0, .running⟩
        ⟨[], t, l2, cs2, hp2, n2, tr0 ++ (tr1 ++ tr2), .running⟩ :=
      execFrom_append hpRun hqRun
    simpa using reach_append_code hcomp [.pin (.prog (p ++ q))]
  have hpin : Reach D H ⟨[.pin (.prog (p ++ q))], t, l2, cs2, hp2, n2,
      tr0 ++ (tr1 ++ tr2), .running⟩
      ⟨[], t, l2, cs2, hp2, n2, tr0 ++ (tr1 ++ tr2), .running⟩ := reach_step .pin
  exact hpushP.trans (hpushQ.trans (hcompose.trans (hrunStep.trans (hbody.trans hpin))))

/-- `cases` on an `inl` payload selects the left body and leaves the right
body unrun. -/
theorem casesLeft_execFrom (p q : List Instr) (v : Value) (s : List Value)
    (l : List Nat) (cs : List Nat) (hp : List (Nat × Value)) (n : Nat)
    (tr0 tr : List Event) (t : List Value) (l' : List Nat) (cs' : List Nat)
    (hp' : List (Nat × Value)) (n' : Nat)
    (hrun : ExecFrom D H p (v :: s) l cs hp n tr0 tr t l' cs' hp' n') :
    ExecFrom D H [.cases p q] (.inl v :: s) l cs hp n tr0 tr t l' cs' hp' n' := by
  have hleft : Reach D H ⟨[.cases p q], .inl v :: s, l, cs, hp, n, tr0, .running⟩
      ⟨p, v :: s, l, cs, hp, n, tr0, .running⟩ := by
    simpa using reach_step (Step.casesLeft (p := p) (q := q) (v := v) (k := [])
      (s := s) (l := l) (cs := cs) (hp := hp) (n := n) (tr := tr0))
  exact hleft.trans (by simpa [ExecFrom] using hrun)

theorem casesRight_execFrom (p q : List Instr) (v : Value) (s : List Value)
    (l : List Nat) (cs : List Nat) (hp : List (Nat × Value)) (n : Nat)
    (tr0 tr : List Event) (t : List Value) (l' : List Nat) (cs' : List Nat)
    (hp' : List (Nat × Value)) (n' : Nat)
    (hrun : ExecFrom D H q (v :: s) l cs hp n tr0 tr t l' cs' hp' n') :
    ExecFrom D H [.cases p q] (.inr v :: s) l cs hp n tr0 tr t l' cs' hp' n' := by
  have hright : Reach D H ⟨[.cases p q], .inr v :: s, l, cs, hp, n, tr0, .running⟩
      ⟨q, v :: s, l, cs, hp, n, tr0, .running⟩ := by
    simpa using reach_step (Step.casesRight (p := p) (q := q) (v := v) (k := [])
      (s := s) (l := l) (cs := cs) (hp := hp) (n := n) (tr := tr0))
  exact hright.trans (by simpa [ExecFrom] using hrun)

/-- `ifte` with a nonzero condition runs the deeper (`p`) program. The
condition sits below both Program values, as the source signature
`S Bool Program Program` requires. -/
theorem ifte_true_execFrom (p q : List Instr) (x : BitVec 64) (s : List Value)
    (l : List Nat) (cs : List Nat) (hp : List (Nat × Value)) (n : Nat)
    (tr0 tr : List Event) (t : List Value) (l' : List Nat) (cs' : List Nat)
    (hp' : List (Nat × Value)) (n' : Nat) (hx : x ≠ 0)
    (hrun : ExecFrom D H p s l cs hp n tr0 tr t l' cs' hp' n') :
    ExecFrom D H [.ifte] (.prog q :: .prog p :: .i64 x :: s) l cs hp n tr0 tr t l'
      cs' hp' n' := by
  have hstep : Reach D H ⟨[.ifte], .prog q :: .prog p :: .i64 x :: s, l, cs, hp, n,
      tr0, .running⟩ ⟨p, s, l, cs, hp, n, tr0, .running⟩ := by
    simpa using reach_step (Step.ifTrue (p := p) (q := q) (x := x) (k := []) (s := s)
      (l := l) (cs := cs) (hp := hp) (n := n) (tr := tr0) hx)
  exact hstep.trans (by simpa [ExecFrom] using hrun)

theorem ifte_false_execFrom (p q : List Instr) (x : BitVec 64) (s : List Value)
    (l : List Nat) (cs : List Nat) (hp : List (Nat × Value)) (n : Nat)
    (tr0 tr : List Event) (t : List Value) (l' : List Nat) (cs' : List Nat)
    (hp' : List (Nat × Value)) (n' : Nat) (hx : x = 0)
    (hrun : ExecFrom D H q s l cs hp n tr0 tr t l' cs' hp' n') :
    ExecFrom D H [.ifte] (.prog q :: .prog p :: .i64 x :: s) l cs hp n tr0 tr t l'
      cs' hp' n' := by
  have hstep : Reach D H ⟨[.ifte], .prog q :: .prog p :: .i64 x :: s, l, cs, hp, n,
      tr0, .running⟩ ⟨q, s, l, cs, hp, n, tr0, .running⟩ := by
    simpa using reach_step (Step.ifFalse (p := p) (q := q) (x := x) (k := []) (s := s)
      (l := l) (cs := cs) (hp := hp) (n := n) (tr := tr0) hx)
  exact hstep.trans (by simpa [ExecFrom] using hrun)

/-! ## The wiring words

The library design's `first`, `second`, `split`, `fanout`, `left`, `right`,
`choose` and `join` are derived words: each is a checked composition of
existing instructions. They are encoded here in the form their source
signatures use, with the Program operands as stack values. `identity` is the
empty word and `then` is `compose` short-circuited to ordinary code
concatenation, matching its body (`compose`) and its order. -/

/-- `wire@1.identity`. -/
def identityWord : List Instr := []

/-- `first`: run the Program on the first component while holding the second
in a `dip` frame, then pair the result back. -/
def firstWord : List Instr :=
  [.dup, .block [.unpair, .swap], .swap, .drop, .dip, .dip, .swap, .pair]

/-- `second`: hold the first component, run the Program on the second, then
pair. -/
def secondWord : List Instr :=
  [.dup, .block [.unpair], .swap, .drop, .dip, .dip, .pair]

/-- `split`: hold the second Program, run `first` with the first Program,
then run `second` with the held Program. -/
def splitWord : List Instr := [.block firstWord, .dip] ++ secondWord

/-- The duplication prefix of `fanout`: duplicate the shared `Data` input
under both Program values. -/
def fanoutPrefix : List Instr := [.block [.block [.dup, .pair], .dip], .dip]

/-- `fanout`: duplicate `x` into `Pair x x`, then `split`. A resource input
has no `dup` derivation (`fanout_at_resource_untypable`). -/
def fanoutWord : List Instr := fanoutPrefix ++ splitWord

/-- `left`: run the Program on an `inl` payload; an `inr` payload is
returned unchanged and the Program is discarded unrun. -/
def leftWord : List Instr :=
  [.swap, .cases [.swap, .run, .inl] [.swap, .drop, .inr]]

/-- `right`: run the Program on an `inr` payload; an `inl` payload is
returned unchanged. -/
def rightWord : List Instr :=
  [.swap, .cases [.swap, .drop, .inl] [.swap, .run, .inr]]

/-- `choose`: exactly one branch runs; the unselected Program is discarded
unrun (Program environments are `Copy`). -/
def chooseWord : List Instr :=
  [.block [.swap], .dip, .swap, .cases [.block [.drop], .dip, .swap, .run, .inl]
    [.block [.swap, .drop], .dip, .swap, .run, .inr]]

/-- `join`: exactly one branch runs and both branches share the output
interface. -/
def joinWord : List Instr :=
  [.block [.swap], .dip, .swap, .cases [.block [.drop], .dip, .swap, .run]
    [.block [.swap, .drop], .dip, .swap, .run]]

/-- A trace-preserving prefix followed by a trace-bearing suffix: the
workhorse for the derived words, where every construction word leaves the
trace unchanged. -/
theorem execFrom_then {p q : List Instr} {s m t : List Value} {l cs hp n l1 cs1 hp1 n1
    l2 cs2 hp2 n2 tr0 tr} (hpRun : ExecFrom D H p s l cs hp n tr0 [] m l1 cs1 hp1 n1)
    (hqRun : ExecFrom D H q m l1 cs1 hp1 n1 tr0 tr t l2 cs2 hp2 n2) :
    ExecFrom D H (p ++ q) s l cs hp n tr0 tr t l2 cs2 hp2 n2 := by
  simpa using execFrom_append hpRun (by simpa using hqRun)

/-- `first` runs `p` on the first component, holds the second in a frame,
restores it exactly once and repairs the pair. -/
theorem first_execFrom {p : List Instr} {a b c : Value} {s : List Value}
    {l cs hp n : _} {tr0 tr : List Event} {l' cs' hp' n' : _}
    (hpRun : ExecFrom D H p (a :: s) l cs hp n tr0 tr (c :: s) l' cs' hp' n') :
    ExecFrom D H firstWord (.prog p :: .pair a b :: s) l cs hp n tr0 tr
      (.pair c b :: s) l' cs' hp' n' := by
  have hbody : ExecFrom D H [.unpair, .swap] (.pair a b :: s) l cs hp n tr0 []
      (b :: a :: s) l cs hp n :=
    execFrom_then (unpair_execFrom a b s l cs hp n tr0)
      (swap_execFrom b a s l cs hp n tr0)
  have hdup : ExecFrom D H [.dup] (.prog p :: .pair a b :: s) l cs hp n tr0 []
      (.prog p :: .prog p :: .pair a b :: s) l cs hp n :=
    dup_execFrom (.prog p) (.pair a b :: s) l cs hp n tr0
  have hblock : ExecFrom D H [.block [.unpair, .swap]]
      (.prog p :: .prog p :: .pair a b :: s) l cs hp n tr0 []
      (.prog [.unpair, .swap] :: .prog p :: .prog p :: .pair a b :: s) l cs hp n :=
    block_execFrom [.unpair, .swap] _ l cs hp n tr0
  have hswap : ExecFrom D H [.swap]
      (.prog [.unpair, .swap] :: .prog p :: .prog p :: .pair a b :: s) l cs hp n tr0 []
      (.prog p :: .prog [.unpair, .swap] :: .prog p :: .pair a b :: s) l cs hp n :=
    swap_execFrom (.prog p) (.prog [.unpair, .swap]) (.prog p :: .pair a b :: s)
      l cs hp n tr0
  have hdrop : ExecFrom D H [.drop]
      (.prog p :: .prog [.unpair, .swap] :: .prog p :: .pair a b :: s) l cs hp n tr0 []
      (.prog [.unpair, .swap] :: .prog p :: .pair a b :: s) l cs hp n :=
    drop_execFrom (.prog p) _ l cs hp n tr0
  have hdip1 : ExecFrom D H [.dip]
      (.prog [.unpair, .swap] :: .prog p :: .pair a b :: s) l cs hp n tr0 []
      (.prog p :: b :: a :: s) l cs hp n :=
    dipValue_execFrom [.unpair, .swap] (.prog p) (.pair a b :: s) l cs hp n tr0 [] _
      l cs hp n hbody
  have hdip2 : ExecFrom D H [.dip] (.prog p :: b :: a :: s) l cs hp n tr0 tr
      (b :: c :: s) l' cs' hp' n' :=
    dipValue_execFrom p b (a :: s) l cs hp n tr0 tr (c :: s) l' cs' hp' n' hpRun
  have hswap2 : ExecFrom D H [.swap] (b :: c :: s) l' cs' hp' n' (tr0 ++ tr) []
      (c :: b :: s) l' cs' hp' n' :=
    swap_execFrom c b s l' cs' hp' n' (tr0 ++ tr)
  have hpair : ExecFrom D H [.pair] (c :: b :: s) l' cs' hp' n' (tr0 ++ tr) []
      (.pair c b :: s) l' cs' hp' n' :=
    pair_execFrom c b s l' cs' hp' n' (tr0 ++ tr)
  have htail : ExecFrom D H [.swap, .pair] (b :: c :: s) l' cs' hp' n' (tr0 ++ tr) []
      (.pair c b :: s) l' cs' hp' n' := execFrom_then hswap2 hpair
  have hrest : ExecFrom D H [.dip, .dip, .swap, .pair]
      (.prog [.unpair, .swap] :: .prog p :: .pair a b :: s) l cs hp n tr0 tr
      (.pair c b :: s) l' cs' hp' n' := by
    simpa [ExecFrom, List.append_nil] using execFrom_append hdip1
      (by simpa [ExecFrom, List.append_nil] using
        execFrom_append (tr2 := []) hdip2 (by simpa [ExecFrom, List.append_nil] using htail))
  have hprefix : ExecFrom D H ([.dup] ++ [.block [.unpair, .swap]] ++ [.swap] ++ [.drop])
      (.prog p :: .pair a b :: s) l cs hp n tr0 []
      (.prog [.unpair, .swap] :: .prog p :: .pair a b :: s) l cs hp n := by
    refine execFrom_then (p := [.dup]) hdup ?_
    refine execFrom_then (p := [.block [.unpair, .swap]]) hblock ?_
    refine execFrom_then (p := [.swap]) hswap ?_
    simpa using hdrop
  simpa [ExecFrom, firstWord, List.append_assoc, List.append_nil] using
    execFrom_append hprefix (by simpa [ExecFrom, List.append_nil] using hrest)

/-- `second` runs `p` on the second component while holding the first. -/
theorem second_execFrom {p : List Instr} {a b d : Value} {s : List Value}
    {l cs hp n : _} {tr0 tr : List Event} {l' cs' hp' n' : _}
    (hpRun : ExecFrom D H p (b :: s) l cs hp n tr0 tr (d :: s) l' cs' hp' n') :
    ExecFrom D H secondWord (.prog p :: .pair a b :: s) l cs hp n tr0 tr
      (.pair a d :: s) l' cs' hp' n' := by
  have hbody : ExecFrom D H [.unpair] (.pair a b :: s) l cs hp n tr0 []
      (a :: b :: s) l cs hp n := unpair_execFrom a b s l cs hp n tr0
  have hdup : ExecFrom D H [.dup] (.prog p :: .pair a b :: s) l cs hp n tr0 []
      (.prog p :: .prog p :: .pair a b :: s) l cs hp n :=
    dup_execFrom (.prog p) (.pair a b :: s) l cs hp n tr0
  have hblock : ExecFrom D H [.block [.unpair]]
      (.prog p :: .prog p :: .pair a b :: s) l cs hp n tr0 []
      (.prog [.unpair] :: .prog p :: .prog p :: .pair a b :: s) l cs hp n :=
    block_execFrom [.unpair] _ l cs hp n tr0
  have hswap : ExecFrom D H [.swap]
      (.prog [.unpair] :: .prog p :: .prog p :: .pair a b :: s) l cs hp n tr0 []
      (.prog p :: .prog [.unpair] :: .prog p :: .pair a b :: s) l cs hp n :=
    swap_execFrom (.prog p) (.prog [.unpair]) (.prog p :: .pair a b :: s) l cs hp n tr0
  have hdrop : ExecFrom D H [.drop]
      (.prog p :: .prog [.unpair] :: .prog p :: .pair a b :: s) l cs hp n tr0 []
      (.prog [.unpair] :: .prog p :: .pair a b :: s) l cs hp n :=
    drop_execFrom (.prog p) _ l cs hp n tr0
  have hdip1 : ExecFrom D H [.dip]
      (.prog [.unpair] :: .prog p :: .pair a b :: s) l cs hp n tr0 []
      (.prog p :: a :: b :: s) l cs hp n :=
    dipValue_execFrom [.unpair] (.prog p) (.pair a b :: s) l cs hp n tr0 [] _
      l cs hp n hbody
  have hdip2 : ExecFrom D H [.dip] (.prog p :: a :: b :: s) l cs hp n tr0 tr
      (a :: d :: s) l' cs' hp' n' :=
    dipValue_execFrom p a (b :: s) l cs hp n tr0 tr (d :: s) l' cs' hp' n' hpRun
  have hpair : ExecFrom D H [.pair] (a :: d :: s) l' cs' hp' n' (tr0 ++ tr) []
      (.pair a d :: s) l' cs' hp' n' := pair_execFrom a d s l' cs' hp' n' (tr0 ++ tr)
  have hrest : ExecFrom D H [.dip, .dip, .pair]
      (.prog [.unpair] :: .prog p :: .pair a b :: s) l cs hp n tr0 tr
      (.pair a d :: s) l' cs' hp' n' := by
    simpa [ExecFrom, List.append_nil] using execFrom_append hdip1
      (by simpa [ExecFrom, List.append_nil] using
        execFrom_append (tr2 := []) hdip2 (by simpa [ExecFrom, List.append_nil] using hpair))
  have hprefix : ExecFrom D H ([.dup] ++ [.block [.unpair]] ++ [.swap] ++ [.drop])
      (.prog p :: .pair a b :: s) l cs hp n tr0 []
      (.prog [.unpair] :: .prog p :: .pair a b :: s) l cs hp n := by
    refine execFrom_then (p := [.dup]) hdup ?_
    refine execFrom_then (p := [.block [.unpair]]) hblock ?_
    refine execFrom_then (p := [.swap]) hswap ?_
    simpa using hdrop
  simpa [ExecFrom, secondWord, List.append_assoc, List.append_nil] using
    execFrom_append hprefix (by simpa [ExecFrom, List.append_nil] using hrest)

/-- `split`: run `first` with the first Program while holding the second,
then `second` with the held Program. The trace is the first program's
requests followed by the second's. -/
theorem split_execFrom {p q : List Instr} {a b c d : Value} {s : List Value}
    {l cs hp n : _} {tr0 tr1 : List Event} {l1 cs1 hp1 n1 : _}
    {tr2 : List Event} {l2 cs2 hp2 n2 : _}
    (hpRun : ExecFrom D H p (a :: s) l cs hp n tr0 tr1 (c :: s) l1 cs1 hp1 n1)
    (hqRun : ExecFrom D H q (b :: s) l1 cs1 hp1 n1 (tr0 ++ tr1) tr2 (d :: s)
      l2 cs2 hp2 n2) :
    ExecFrom D H splitWord (.prog q :: .prog p :: .pair a b :: s) l cs hp n tr0
      (tr1 ++ tr2) (.pair c d :: s) l2 cs2 hp2 n2 := by
  have hfirst : ExecFrom D H firstWord (.prog p :: .pair a b :: s) l cs hp n tr0 tr1
      (.pair c b :: s) l1 cs1 hp1 n1 := first_execFrom hpRun
  have hdip : ExecFrom D H [.block firstWord, .dip]
      (.prog q :: .prog p :: .pair a b :: s) l cs hp n tr0 tr1
      (.prog q :: .pair c b :: s) l1 cs1 hp1 n1 :=
    dip_execFrom firstWord (.prog q) (.prog p :: .pair a b :: s) l cs hp n tr0 tr1
      (.pair c b :: s) l1 cs1 hp1 n1 hfirst
  have hsecond : ExecFrom D H secondWord (.prog q :: .pair c b :: s) l1 cs1 hp1 n1
      (tr0 ++ tr1) tr2 (.pair c d :: s) l2 cs2 hp2 n2 := second_execFrom hqRun
  have hwhole : splitWord = [.block firstWord, .dip] ++ secondWord := rfl
  rw [hwhole]
  exact execFrom_append hdip hsecond

/-- `fanout` duplicates a `Data` input and then splits it. The duplication
prefix issues no host request, so the trace is still `p`'s then `q`'s. -/
theorem fanout_execFrom {p q : List Instr} {x y z : Value} {s : List Value}
    {l cs hp n : _} {tr0 tr1 : List Event} {l1 cs1 hp1 n1 : _}
    {tr2 : List Event} {l2 cs2 hp2 n2 : _}
    (hpRun : ExecFrom D H p (x :: s) l cs hp n tr0 tr1 (y :: s) l1 cs1 hp1 n1)
    (hqRun : ExecFrom D H q (x :: s) l1 cs1 hp1 n1 (tr0 ++ tr1) tr2 (z :: s)
      l2 cs2 hp2 n2) :
    ExecFrom D H fanoutWord (.prog q :: .prog p :: x :: s) l cs hp n tr0
      (tr1 ++ tr2) (.pair y z :: s) l2 cs2 hp2 n2 := by
  have hdup : ExecFrom D H [.dup] (x :: s) l cs hp n tr0 [] (x :: x :: s) l cs hp n :=
    dup_execFrom x s l cs hp n tr0
  have hpair : ExecFrom D H [.pair] (x :: x :: s) l cs hp n tr0 []
      (.pair x x :: s) l cs hp n := pair_execFrom x x s l cs hp n tr0
  have hinner : ExecFrom D H [.dup, .pair] (x :: s) l cs hp n tr0 []
      (.pair x x :: s) l cs hp n := execFrom_then hdup hpair
  have hblockInner : ExecFrom D H [.block [.dup, .pair], .dip]
      (.prog p :: x :: s) l cs hp n tr0 [] (.prog p :: .pair x x :: s) l cs hp n :=
    dip_execFrom [.dup, .pair] (.prog p) (x :: s) l cs hp n tr0 [] _
      l cs hp n hinner
  have hprefix : ExecFrom D H fanoutPrefix (.prog q :: .prog p :: x :: s) l cs hp n
      tr0 [] (.prog q :: .prog p :: .pair x x :: s) l cs hp n :=
    dip_execFrom [.block [.dup, .pair], .dip] (.prog q)
      (.prog p :: x :: s) l cs hp n tr0 [] _
      l cs hp n hblockInner
  have hsplit : ExecFrom D H splitWord
      (.prog q :: .prog p :: .pair x x :: s) l cs hp n tr0 (tr1 ++ tr2)
      (.pair y z :: s) l2 cs2 hp2 n2 := split_execFrom hpRun hqRun
  have hwhole : fanoutWord = fanoutPrefix ++ splitWord := rfl
  rw [hwhole]
  simpa [ExecFrom, List.append_nil] using execFrom_append (tr1 := [])
    (by simpa [ExecFrom, List.append_nil] using hprefix)
    (by simpa [ExecFrom] using hsplit)

/-- `left` on an `inl` payload runs the Program and wraps the result. -/
theorem left_inl_execFrom {p : List Instr} {a c : Value} {s : List Value}
    {l cs hp n : _} {tr0 tr : List Event} {l' cs' hp' n' : _}
    (hpRun : ExecFrom D H p (a :: s) l cs hp n tr0 tr (c :: s) l' cs' hp' n') :
    ExecFrom D H leftWord (.prog p :: .inl a :: s) l cs hp n tr0 tr
      (.inl c :: s) l' cs' hp' n' := by
  have hswap : ExecFrom D H [.swap] (.prog p :: .inl a :: s) l cs hp n tr0 []
      (.inl a :: .prog p :: s) l cs hp n :=
    swap_execFrom (.inl a) (.prog p) s l cs hp n tr0
  have hleftArm : ExecFrom D H [.swap, .run, .inl] (a :: .prog p :: s) l cs hp n
      tr0 tr (.inl c :: s) l' cs' hp' n' := by
    have h1 : ExecFrom D H [.swap] (a :: .prog p :: s) l cs hp n tr0 []
        (.prog p :: a :: s) l cs hp n :=
      swap_execFrom (.prog p) a s l cs hp n tr0
    have h2 : ExecFrom D H [.run] (.prog p :: a :: s) l cs hp n tr0 tr
        (c :: s) l' cs' hp' n' := runValue_execFrom p (a :: s) l cs hp n tr0 tr
      (c :: s) l' cs' hp' n' hpRun
    have h3 : ExecFrom D H [.inl] (c :: s) l' cs' hp' n' (tr0 ++ tr) []
        (.inl c :: s) l' cs' hp' n' := inl_execFrom c s l' cs' hp' n' (tr0 ++ tr)
    have h23 : ExecFrom D H [.run, .inl] (.prog p :: a :: s) l cs hp n tr0 tr
        (.inl c :: s) l' cs' hp' n' := by
      simpa [ExecFrom, List.append_nil] using
        execFrom_append (tr2 := []) h2 (by simpa [ExecFrom, List.append_nil] using h3)
    exact execFrom_append h1 (by simpa [ExecFrom, List.append_nil] using h23)
  have hcases : ExecFrom D H [.cases [.swap, .run, .inl] [.swap, .drop, .inr]]
      (.inl a :: .prog p :: s) l cs hp n tr0 tr (.inl c :: s) l' cs' hp' n' :=
    casesLeft_execFrom [.swap, .run, .inl] [.swap, .drop, .inr] a (.prog p :: s)
      l cs hp n tr0 tr (.inl c :: s) l' cs' hp' n' hleftArm
  have hwhole : leftWord = [.swap] ++ [.cases [.swap, .run, .inl] [.swap, .drop, .inr]] := rfl
  rw [hwhole]
  exact execFrom_append hswap (by simpa using hcases)

/-- `left` on an `inr` payload returns it unchanged; the Program is not
run and issues no request. -/
theorem left_inr_execFrom {p : List Instr} {b : Value} {s : List Value}
    {l cs : List Nat} {hp : List (Nat × Value)} {n : Nat} {tr0 : List Event} :
    ExecFrom D H leftWord (.prog p :: .inr b :: s) l cs hp n tr0 []
      (.inr b :: s) l cs hp n := by
  have hswap : ExecFrom D H [.swap] (.prog p :: .inr b :: s) l cs hp n tr0 []
      (.inr b :: .prog p :: s) l cs hp n :=
    swap_execFrom (.inr b) (.prog p) s l cs hp n tr0
  have hrightArm : ExecFrom D H [.swap, .drop, .inr] (b :: .prog p :: s)
      l cs hp n tr0 [] (.inr b :: s) l cs hp n := by
    have h1 : ExecFrom D H [.swap] (b :: .prog p :: s) l cs hp n tr0 []
        (.prog p :: b :: s) l cs hp n := swap_execFrom (.prog p) b s l cs hp n tr0
    have h2 : ExecFrom D H [.drop] (.prog p :: b :: s) l cs hp n tr0 []
        (b :: s) l cs hp n := drop_execFrom (.prog p) (b :: s) l cs hp n tr0
    have h3 : ExecFrom D H [.inr] (b :: s) l cs hp n tr0 [] (.inr b :: s) l cs hp n :=
      inr_execFrom b s l cs hp n tr0
    exact execFrom_then h1 (execFrom_then h2 h3)
  have hcases : ExecFrom D H [.cases [.swap, .run, .inl] [.swap, .drop, .inr]]
      (.inr b :: .prog p :: s) l cs hp n tr0 [] (.inr b :: s) l cs hp n :=
    casesRight_execFrom [.swap, .run, .inl] [.swap, .drop, .inr] b (.prog p :: s)
      l cs hp n tr0 [] (.inr b :: s) l cs hp n hrightArm
  have hwhole : leftWord = [.swap] ++ [.cases [.swap, .run, .inl] [.swap, .drop, .inr]] := rfl
  rw [hwhole]
  exact execFrom_append hswap (by simpa using hcases)

/-- `right` on an `inr` payload runs the Program. -/
theorem right_inr_execFrom {p : List Instr} {b d : Value} {s : List Value}
    {l cs hp n : _} {tr0 tr : List Event} {l' cs' hp' n' : _}
    (hpRun : ExecFrom D H p (b :: s) l cs hp n tr0 tr (d :: s) l' cs' hp' n') :
    ExecFrom D H rightWord (.prog p :: .inr b :: s) l cs hp n tr0 tr
      (.inr d :: s) l' cs' hp' n' := by
  have hswap : ExecFrom D H [.swap] (.prog p :: .inr b :: s) l cs hp n tr0 []
      (.inr b :: .prog p :: s) l cs hp n :=
    swap_execFrom (.inr b) (.prog p) s l cs hp n tr0
  have hrightArm : ExecFrom D H [.swap, .run, .inr] (b :: .prog p :: s) l cs hp n
      tr0 tr (.inr d :: s) l' cs' hp' n' := by
    have h1 : ExecFrom D H [.swap] (b :: .prog p :: s) l cs hp n tr0 []
        (.prog p :: b :: s) l cs hp n := swap_execFrom (.prog p) b s l cs hp n tr0
    have h2 : ExecFrom D H [.run] (.prog p :: b :: s) l cs hp n tr0 tr
        (d :: s) l' cs' hp' n' := runValue_execFrom p (b :: s) l cs hp n tr0 tr
      (d :: s) l' cs' hp' n' hpRun
    have h3 : ExecFrom D H [.inr] (d :: s) l' cs' hp' n' (tr0 ++ tr) []
        (.inr d :: s) l' cs' hp' n' := inr_execFrom d s l' cs' hp' n' (tr0 ++ tr)
    have h23 : ExecFrom D H [.run, .inr] (.prog p :: b :: s) l cs hp n tr0 tr
        (.inr d :: s) l' cs' hp' n' := by
      simpa [ExecFrom, List.append_nil] using
        execFrom_append (tr2 := []) h2 (by simpa [ExecFrom, List.append_nil] using h3)
    exact execFrom_append h1 (by simpa [ExecFrom, List.append_nil] using h23)
  have hcases : ExecFrom D H [.cases [.swap, .drop, .inl] [.swap, .run, .inr]]
      (.inr b :: .prog p :: s) l cs hp n tr0 tr (.inr d :: s) l' cs' hp' n' :=
    casesRight_execFrom [.swap, .drop, .inl] [.swap, .run, .inr] b (.prog p :: s)
      l cs hp n tr0 tr (.inr d :: s) l' cs' hp' n' hrightArm
  have hwhole : rightWord = [.swap] ++ [.cases [.swap, .drop, .inl] [.swap, .run, .inr]] := rfl
  rw [hwhole]
  exact execFrom_append hswap (by simpa using hcases)

/-- `right` on an `inl` payload returns it unchanged. -/
theorem right_inl_execFrom {p : List Instr} {a : Value} {s : List Value}
    {l cs : List Nat} {hp : List (Nat × Value)} {n : Nat} {tr0 : List Event} :
    ExecFrom D H rightWord (.prog p :: .inl a :: s) l cs hp n tr0 []
      (.inl a :: s) l cs hp n := by
  have hswap : ExecFrom D H [.swap] (.prog p :: .inl a :: s) l cs hp n tr0 []
      (.inl a :: .prog p :: s) l cs hp n :=
    swap_execFrom (.inl a) (.prog p) s l cs hp n tr0
  have hleftArm : ExecFrom D H [.swap, .drop, .inl] (a :: .prog p :: s)
      l cs hp n tr0 [] (.inl a :: s) l cs hp n := by
    have h1 : ExecFrom D H [.swap] (a :: .prog p :: s) l cs hp n tr0 []
        (.prog p :: a :: s) l cs hp n := swap_execFrom (.prog p) a s l cs hp n tr0
    have h2 : ExecFrom D H [.drop] (.prog p :: a :: s) l cs hp n tr0 []
        (a :: s) l cs hp n := drop_execFrom (.prog p) (a :: s) l cs hp n tr0
    have h3 : ExecFrom D H [.inl] (a :: s) l cs hp n tr0 [] (.inl a :: s) l cs hp n :=
      inl_execFrom a s l cs hp n tr0
    exact execFrom_then h1 (execFrom_then h2 h3)
  have hcases : ExecFrom D H [.cases [.swap, .drop, .inl] [.swap, .run, .inr]]
      (.inl a :: .prog p :: s) l cs hp n tr0 [] (.inl a :: s) l cs hp n :=
    casesLeft_execFrom [.swap, .drop, .inl] [.swap, .run, .inr] a (.prog p :: s)
      l cs hp n tr0 [] (.inl a :: s) l cs hp n hleftArm
  have hwhole : rightWord = [.swap] ++ [.cases [.swap, .drop, .inl] [.swap, .run, .inr]] := rfl
  rw [hwhole]
  exact execFrom_append hswap (by simpa using hcases)


/-- `choose` on `inl` runs the first Program and discards the second unrun. -/
theorem choose_inl_execFrom {p q : List Instr} {a c : Value} {s : List Value}
    {l cs hp n : _} {tr0 tr : List Event} {l' cs' hp' n' : _}
    (hpRun : ExecFrom D H p (a :: s) l cs hp n tr0 tr (c :: s) l' cs' hp' n') :
    ExecFrom D H chooseWord (.prog q :: .prog p :: .inl a :: s) l cs hp n tr0 tr
      (.inl c :: s) l' cs' hp' n' := by
  have hdropBody : ExecFrom D H [.drop] (.prog q :: .prog p :: s) l cs hp n tr0 []
      (.prog p :: s) l cs hp n := drop_execFrom (.prog q) (.prog p :: s) l cs hp n tr0
  have hdip : ExecFrom D H [.block [.drop], .dip] (a :: .prog q :: .prog p :: s)
      l cs hp n tr0 [] (a :: .prog p :: s) l cs hp n :=
    dip_execFrom [.drop] a (.prog q :: .prog p :: s) l cs hp n tr0 [] _
      l cs hp n hdropBody
  have hswap : ExecFrom D H [.swap] (a :: .prog p :: s) l cs hp n (tr0 ++ []) []
      (.prog p :: a :: s) l cs hp n :=
    swap_execFrom (.prog p) a s l cs hp n (tr0 ++ [])
  have hrun : ExecFrom D H [.run] (.prog p :: a :: s) l cs hp n tr0 tr
      (c :: s) l' cs' hp' n' := runValue_execFrom p (a :: s) l cs hp n tr0 tr
    (c :: s) l' cs' hp' n' hpRun
  have hinl : ExecFrom D H [.inl] (c :: s) l' cs' hp' n' (tr0 ++ tr) []
      (.inl c :: s) l' cs' hp' n' := inl_execFrom c s l' cs' hp' n' (tr0 ++ tr)
  have hleftArm : ExecFrom D H [.block [.drop], .dip, .swap, .run, .inl]
      (a :: .prog q :: .prog p :: s) l cs hp n tr0 tr (.inl c :: s) l' cs' hp' n' := by
    have hrest : ExecFrom D H [.swap, .run, .inl] (a :: .prog p :: s) l cs hp n
        (tr0 ++ []) tr (.inl c :: s) l' cs' hp' n' := by
      simpa [ExecFrom, List.append_nil] using execFrom_append hswap
        (by simpa [ExecFrom, List.append_nil] using
          execFrom_append (tr2 := []) hrun (by simpa [ExecFrom, List.append_nil] using hinl))
    simpa [ExecFrom, List.append_nil] using execFrom_append hdip
      (by simpa [ExecFrom, List.append_nil] using hrest)
  have hbody : ExecFrom D H [.swap] (.prog p :: .inl a :: s) l cs hp n tr0 []
      (.inl a :: .prog p :: s) l cs hp n :=
    swap_execFrom (.inl a) (.prog p) s l cs hp n tr0
  have hrotate : ExecFrom D H [.block [.swap], .dip, .swap]
      (.prog q :: .prog p :: .inl a :: s) l cs hp n tr0 []
      (.inl a :: .prog q :: .prog p :: s) l cs hp n := by
    have hbody : ExecFrom D H [.swap] (.prog p :: .inl a :: s) l cs hp n tr0 []
        (.inl a :: .prog p :: s) l cs hp n :=
      swap_execFrom (.inl a) (.prog p) s l cs hp n tr0
    have hdip : ExecFrom D H [.block [.swap], .dip]
        (.prog q :: .prog p :: .inl a :: s) l cs hp n tr0 []
        (.prog q :: .inl a :: .prog p :: s) l cs hp n :=
      dip_execFrom [.swap] (.prog q) (.prog p :: .inl a :: s) l cs hp n tr0 []
        (.inl a :: .prog p :: s) l cs hp n hbody
    have htail : ExecFrom D H [.swap]
        (.prog q :: .inl a :: .prog p :: s) l cs hp n (tr0 ++ []) []
        (.inl a :: .prog q :: .prog p :: s) l cs hp n :=
      swap_execFrom (.inl a) (.prog q) (.prog p :: s) l cs hp n (tr0 ++ [])
    simpa [ExecFrom, List.append_nil] using execFrom_append hdip htail
  have hcases : ExecFrom D H
      [.cases [.block [.drop], .dip, .swap, .run, .inl]
        [.block [.swap, .drop], .dip, .swap, .run, .inr]]
      (.inl a :: .prog q :: .prog p :: s) l cs hp n tr0 tr (.inl c :: s) l' cs' hp' n' :=
    casesLeft_execFrom _ _ a (.prog q :: .prog p :: s) l cs hp n tr0 tr
      (.inl c :: s) l' cs' hp' n' hleftArm
  simp only [chooseWord]
  exact execFrom_append hrotate (by simpa [ExecFrom, List.append_nil] using hcases)

/-- `choose` on `inr` runs the second Program and discards the first unrun. -/
theorem choose_inr_execFrom {p q : List Instr} {b d : Value} {s : List Value}
    {l cs hp n : _} {tr0 tr : List Event} {l' cs' hp' n' : _}
    (hqRun : ExecFrom D H q (b :: s) l cs hp n tr0 tr (d :: s) l' cs' hp' n') :
    ExecFrom D H chooseWord (.prog q :: .prog p :: .inr b :: s) l cs hp n tr0 tr
      (.inr d :: s) l' cs' hp' n' := by
  have hdropBody : ExecFrom D H [.swap, .drop] (.prog q :: .prog p :: s) l cs hp n
      tr0 [] (.prog q :: s) l cs hp n := by
    have h1 : ExecFrom D H [.swap] (.prog q :: .prog p :: s) l cs hp n tr0 []
        (.prog p :: .prog q :: s) l cs hp n :=
      swap_execFrom (.prog p) (.prog q) s l cs hp n tr0
    have h2 : ExecFrom D H [.drop] (.prog p :: .prog q :: s) l cs hp n tr0 []
        (.prog q :: s) l cs hp n := drop_execFrom (.prog p) (.prog q :: s) l cs hp n tr0
    exact execFrom_then h1 h2
  have hdip : ExecFrom D H [.block [.swap, .drop], .dip]
      (b :: .prog q :: .prog p :: s) l cs hp n tr0 [] (b :: .prog q :: s) l cs hp n :=
    dip_execFrom [.swap, .drop] b (.prog q :: .prog p :: s) l cs hp n tr0 [] _
      l cs hp n hdropBody
  have hswap : ExecFrom D H [.swap] (b :: .prog q :: s) l cs hp n (tr0 ++ []) []
      (.prog q :: b :: s) l cs hp n :=
    swap_execFrom (.prog q) b s l cs hp n (tr0 ++ [])
  have hrun : ExecFrom D H [.run] (.prog q :: b :: s) l cs hp n tr0 tr
      (d :: s) l' cs' hp' n' := runValue_execFrom q (b :: s) l cs hp n tr0 tr
    (d :: s) l' cs' hp' n' hqRun
  have hinr : ExecFrom D H [.inr] (d :: s) l' cs' hp' n' (tr0 ++ tr) []
      (.inr d :: s) l' cs' hp' n' := inr_execFrom d s l' cs' hp' n' (tr0 ++ tr)
  have hrightArm : ExecFrom D H [.block [.swap, .drop], .dip, .swap, .run, .inr]
      (b :: .prog q :: .prog p :: s) l cs hp n tr0 tr (.inr d :: s) l' cs' hp' n' := by
    have hrest : ExecFrom D H [.swap, .run, .inr] (b :: .prog q :: s) l cs hp n
        (tr0 ++ []) tr (.inr d :: s) l' cs' hp' n' := by
      simpa [ExecFrom, List.append_nil] using execFrom_append hswap
        (by simpa [ExecFrom, List.append_nil] using
          execFrom_append (tr2 := []) hrun (by simpa [ExecFrom, List.append_nil] using hinr))
    simpa [ExecFrom, List.append_nil] using execFrom_append hdip
      (by simpa [ExecFrom, List.append_nil] using hrest)
  have hbody : ExecFrom D H [.swap] (.prog p :: .inr b :: s) l cs hp n tr0 []
      (.inr b :: .prog p :: s) l cs hp n :=
    swap_execFrom (.inr b) (.prog p) s l cs hp n tr0
  have hrotate : ExecFrom D H [.block [.swap], .dip, .swap]
      (.prog q :: .prog p :: .inr b :: s) l cs hp n tr0 []
      (.inr b :: .prog q :: .prog p :: s) l cs hp n := by
    have hbody : ExecFrom D H [.swap] (.prog p :: .inr b :: s) l cs hp n tr0 []
        (.inr b :: .prog p :: s) l cs hp n :=
      swap_execFrom (.inr b) (.prog p) s l cs hp n tr0
    have hdip : ExecFrom D H [.block [.swap], .dip]
        (.prog q :: .prog p :: .inr b :: s) l cs hp n tr0 []
        (.prog q :: .inr b :: .prog p :: s) l cs hp n :=
      dip_execFrom [.swap] (.prog q) (.prog p :: .inr b :: s) l cs hp n tr0 []
        (.inr b :: .prog p :: s) l cs hp n hbody
    have htail : ExecFrom D H [.swap]
        (.prog q :: .inr b :: .prog p :: s) l cs hp n (tr0 ++ []) []
        (.inr b :: .prog q :: .prog p :: s) l cs hp n :=
      swap_execFrom (.inr b) (.prog q) (.prog p :: s) l cs hp n (tr0 ++ [])
    simpa [ExecFrom, List.append_nil] using execFrom_append hdip htail
  have hcases : ExecFrom D H
      [.cases [.block [.drop], .dip, .swap, .run, .inl]
        [.block [.swap, .drop], .dip, .swap, .run, .inr]]
      (.inr b :: .prog q :: .prog p :: s) l cs hp n tr0 tr (.inr d :: s) l' cs' hp' n' :=
    casesRight_execFrom _ _ b (.prog q :: .prog p :: s) l cs hp n tr0 tr
      (.inr d :: s) l' cs' hp' n' hrightArm
  simp only [chooseWord]
  exact execFrom_append hrotate (by simpa [ExecFrom, List.append_nil] using hcases)

/-- `join` on `inl` runs the first Program and discards the second. -/
theorem join_inl_execFrom {p q : List Instr} {a c : Value} {s : List Value}
    {l cs hp n : _} {tr0 tr : List Event} {l' cs' hp' n' : _}
    (hpRun : ExecFrom D H p (a :: s) l cs hp n tr0 tr (c :: s) l' cs' hp' n') :
    ExecFrom D H joinWord (.prog q :: .prog p :: .inl a :: s) l cs hp n tr0 tr
      (c :: s) l' cs' hp' n' := by
  have hdropBody : ExecFrom D H [.drop] (.prog q :: .prog p :: s) l cs hp n tr0 []
      (.prog p :: s) l cs hp n := drop_execFrom (.prog q) (.prog p :: s) l cs hp n tr0
  have hdip : ExecFrom D H [.block [.drop], .dip] (a :: .prog q :: .prog p :: s)
      l cs hp n tr0 [] (a :: .prog p :: s) l cs hp n :=
    dip_execFrom [.drop] a (.prog q :: .prog p :: s) l cs hp n tr0 [] _
      l cs hp n hdropBody
  have hswap : ExecFrom D H [.swap] (a :: .prog p :: s) l cs hp n (tr0 ++ []) []
      (.prog p :: a :: s) l cs hp n :=
    swap_execFrom (.prog p) a s l cs hp n (tr0 ++ [])
  have hrun : ExecFrom D H [.run] (.prog p :: a :: s) l cs hp n tr0 tr
      (c :: s) l' cs' hp' n' := runValue_execFrom p (a :: s) l cs hp n tr0 tr
    (c :: s) l' cs' hp' n' hpRun
  have hleftArm : ExecFrom D H [.block [.drop], .dip, .swap, .run]
      (a :: .prog q :: .prog p :: s) l cs hp n tr0 tr (c :: s) l' cs' hp' n' := by
    have hrest : ExecFrom D H [.swap, .run] (a :: .prog p :: s) l cs hp n
        (tr0 ++ []) tr (c :: s) l' cs' hp' n' := by
      simpa [ExecFrom, List.append_nil] using execFrom_append hswap
        (by simpa [ExecFrom, List.append_nil] using hrun)
    simpa [ExecFrom, List.append_nil] using execFrom_append hdip
      (by simpa [ExecFrom, List.append_nil] using hrest)
  have hrotate : ExecFrom D H [.block [.swap], .dip, .swap]
      (.prog q :: .prog p :: .inl a :: s) l cs hp n tr0 []
      (.inl a :: .prog q :: .prog p :: s) l cs hp n := by
    have hbody : ExecFrom D H [.swap] (.prog p :: .inl a :: s) l cs hp n tr0 []
        (.inl a :: .prog p :: s) l cs hp n :=
      swap_execFrom (.inl a) (.prog p) s l cs hp n tr0
    have hdip : ExecFrom D H [.block [.swap], .dip]
        (.prog q :: .prog p :: .inl a :: s) l cs hp n tr0 []
        (.prog q :: .inl a :: .prog p :: s) l cs hp n :=
      dip_execFrom [.swap] (.prog q) (.prog p :: .inl a :: s) l cs hp n tr0 []
        (.inl a :: .prog p :: s) l cs hp n hbody
    have htail : ExecFrom D H [.swap]
        (.prog q :: .inl a :: .prog p :: s) l cs hp n (tr0 ++ []) []
        (.inl a :: .prog q :: .prog p :: s) l cs hp n :=
      swap_execFrom (.inl a) (.prog q) (.prog p :: s) l cs hp n (tr0 ++ [])
    simpa [ExecFrom, List.append_nil] using execFrom_append hdip htail
  have hcases : ExecFrom D H
      [.cases [.block [.drop], .dip, .swap, .run] [.block [.swap, .drop], .dip, .swap, .run]]
      (.inl a :: .prog q :: .prog p :: s) l cs hp n tr0 tr (c :: s) l' cs' hp' n' :=
    casesLeft_execFrom _ _ a (.prog q :: .prog p :: s) l cs hp n tr0 tr
      (c :: s) l' cs' hp' n' hleftArm
  simp only [joinWord]
  exact execFrom_append hrotate (by simpa [ExecFrom, List.append_nil] using hcases)

/-- `join` on `inr` runs the second Program and discards the first. -/
theorem join_inr_execFrom {p q : List Instr} {b d : Value} {s : List Value}
    {l cs hp n : _} {tr0 tr : List Event} {l' cs' hp' n' : _}
    (hqRun : ExecFrom D H q (b :: s) l cs hp n tr0 tr (d :: s) l' cs' hp' n') :
    ExecFrom D H joinWord (.prog q :: .prog p :: .inr b :: s) l cs hp n tr0 tr
      (d :: s) l' cs' hp' n' := by
  have hdropBody : ExecFrom D H [.swap, .drop] (.prog q :: .prog p :: s) l cs hp n
      tr0 [] (.prog q :: s) l cs hp n := by
    have h1 : ExecFrom D H [.swap] (.prog q :: .prog p :: s) l cs hp n tr0 []
        (.prog p :: .prog q :: s) l cs hp n :=
      swap_execFrom (.prog p) (.prog q) s l cs hp n tr0
    have h2 : ExecFrom D H [.drop] (.prog p :: .prog q :: s) l cs hp n tr0 []
        (.prog q :: s) l cs hp n := drop_execFrom (.prog p) (.prog q :: s) l cs hp n tr0
    exact execFrom_then h1 h2
  have hdip : ExecFrom D H [.block [.swap, .drop], .dip]
      (b :: .prog q :: .prog p :: s) l cs hp n tr0 [] (b :: .prog q :: s) l cs hp n :=
    dip_execFrom [.swap, .drop] b (.prog q :: .prog p :: s) l cs hp n tr0 [] _
      l cs hp n hdropBody
  have hswap : ExecFrom D H [.swap] (b :: .prog q :: s) l cs hp n (tr0 ++ []) []
      (.prog q :: b :: s) l cs hp n :=
    swap_execFrom (.prog q) b s l cs hp n (tr0 ++ [])
  have hrun : ExecFrom D H [.run] (.prog q :: b :: s) l cs hp n tr0 tr
      (d :: s) l' cs' hp' n' := runValue_execFrom q (b :: s) l cs hp n tr0 tr
    (d :: s) l' cs' hp' n' hqRun
  have hrightArm : ExecFrom D H [.block [.swap, .drop], .dip, .swap, .run]
      (b :: .prog q :: .prog p :: s) l cs hp n tr0 tr (d :: s) l' cs' hp' n' := by
    have hrest : ExecFrom D H [.swap, .run] (b :: .prog q :: s) l cs hp n
        (tr0 ++ []) tr (d :: s) l' cs' hp' n' := by
      simpa [ExecFrom, List.append_nil] using execFrom_append hswap
        (by simpa [ExecFrom, List.append_nil] using hrun)
    simpa [ExecFrom, List.append_nil] using execFrom_append hdip
      (by simpa [ExecFrom, List.append_nil] using hrest)
  have hbody : ExecFrom D H [.swap] (.prog p :: .inr b :: s) l cs hp n tr0 []
      (.inr b :: .prog p :: s) l cs hp n :=
    swap_execFrom (.inr b) (.prog p) s l cs hp n tr0
  have hrotate : ExecFrom D H [.block [.swap], .dip, .swap]
      (.prog q :: .prog p :: .inr b :: s) l cs hp n tr0 []
      (.inr b :: .prog q :: .prog p :: s) l cs hp n := by
    have hbody : ExecFrom D H [.swap] (.prog p :: .inr b :: s) l cs hp n tr0 []
        (.inr b :: .prog p :: s) l cs hp n :=
      swap_execFrom (.inr b) (.prog p) s l cs hp n tr0
    have hdip : ExecFrom D H [.block [.swap], .dip]
        (.prog q :: .prog p :: .inr b :: s) l cs hp n tr0 []
        (.prog q :: .inr b :: .prog p :: s) l cs hp n :=
      dip_execFrom [.swap] (.prog q) (.prog p :: .inr b :: s) l cs hp n tr0 []
        (.inr b :: .prog p :: s) l cs hp n hbody
    have htail : ExecFrom D H [.swap]
        (.prog q :: .inr b :: .prog p :: s) l cs hp n (tr0 ++ []) []
        (.inr b :: .prog q :: .prog p :: s) l cs hp n :=
      swap_execFrom (.inr b) (.prog q) (.prog p :: s) l cs hp n (tr0 ++ [])
    simpa [ExecFrom, List.append_nil] using execFrom_append hdip htail
  have hcases : ExecFrom D H
      [.cases [.block [.drop], .dip, .swap, .run] [.block [.swap, .drop], .dip, .swap, .run]]
      (.inr b :: .prog q :: .prog p :: s) l cs hp n tr0 tr (d :: s) l' cs' hp' n' :=
    casesRight_execFrom _ _ b (.prog q :: .prog p :: s) l cs hp n tr0 tr
      (d :: s) l' cs' hp' n' hrightArm
  simp only [joinWord]
  exact execFrom_append hrotate (by simpa [ExecFrom, List.append_nil] using hcases)


/-! ## The per-law T statements

Each theorem is the design's T observation for one law, stated over
`ExecFrom` so the trace history is explicit. The pure-right-identity and
associativity statements are list equalities; the words that run a Program
pass the run's own trace through, so the trace is the ordered concatenation
of the operands' requests. -/

/-- L1, left identity: `[ ] p compose` and `p` have the same run and trace. -/
theorem then_left_id {p s l cs hp n tr0 tr t l' cs' hp' n'} :
    ExecFrom D H (identityWord ++ p) s l cs hp n tr0 tr t l' cs' hp' n' ↔
      ExecFrom D H p s l cs hp n tr0 tr t l' cs' hp' n' := by
  simp [identityWord]

/-- L2, right identity: `p [ ] compose` and `p` have the same run and trace. -/
theorem then_right_id {p s l cs hp n tr0 tr t l' cs' hp' n'} :
    ExecFrom D H (p ++ identityWord) s l cs hp n tr0 tr t l' cs' hp' n' ↔
      ExecFrom D H p s l cs hp n tr0 tr t l' cs' hp' n' := by
  simp [identityWord]

/-- L3, associativity: both groupings run the same words in the same order,
so they have the same trace and result. -/
theorem then_assoc {p q r s l cs hp n tr0 tr t l' cs' hp' n'} :
    ExecFrom D H ((p ++ q) ++ r) s l cs hp n tr0 tr t l' cs' hp' n' ↔
      ExecFrom D H (p ++ (q ++ r)) s l cs hp n tr0 tr t l' cs' hp' n' := by
  simp [List.append_assoc]

/-- L4a: `v quote run` pushes `v` back and issues no request. -/
theorem apply_quote {v s l cs hp n tr0} :
    ExecFrom D H [.push v, .quote, .run] s l cs hp n tr0 [] (v :: s) l cs hp n :=
  quote_run_execFrom v s l cs hp n tr0

/-- L4b: `p q compose run` runs `p` then `q`, with the trace concatenated. -/
theorem apply_compose {p q s l cs hp n tr0 tr1 m l1 cs1 hp1 n1 tr2 t l2 cs2 hp2 n2}
    (hpRun : ExecFrom D H p s l cs hp n tr0 tr1 m l1 cs1 hp1 n1)
    (hqRun : ExecFrom D H q m l1 cs1 hp1 n1 (tr0 ++ tr1) tr2 t l2 cs2 hp2 n2) :
    ExecFrom D H [.push (.prog p), .push (.prog q), .compose, .run] s l cs hp n tr0
      (tr1 ++ tr2) t l2 cs2 hp2 n2 :=
  compose_run_execFrom p q s l cs hp n tr0 tr1 m l1 cs1 hp1 n1 tr2 t l2 cs2 hp2 n2
    hpRun hqRun

/-- L5, frame fusion: `[p] under [q] under` and `[p q] under` hold the same
value, run `p` then `q`, and restore the value once; both traces are
`p`'s requests followed by `q`'s. -/
theorem under_fuse {p q : List Instr} {v : Value} {s m u : List Value}
    {l cs hp n : _} {tr0 tr1 : List Event} {l1 cs1 hp1 n1 : _}
    {tr2 : List Event} {l2 cs2 hp2 n2 : _}
    (hpRun : ExecFrom D H p s l cs hp n tr0 tr1 m l1 cs1 hp1 n1)
    (hqRun : ExecFrom D H q m l1 cs1 hp1 n1 (tr0 ++ tr1) tr2 u l2 cs2 hp2 n2) :
    ExecFrom D H ([.block p, .dip] ++ [.block q, .dip]) (v :: s) l cs hp n tr0
        (tr1 ++ tr2) (v :: u) l2 cs2 hp2 n2 ∧
      ExecFrom D H [.block (p ++ q), .dip] (v :: s) l cs hp n tr0
        (tr1 ++ tr2) (v :: u) l2 cs2 hp2 n2 := by
  have hfirst : ExecFrom D H [.block p, .dip] (v :: s) l cs hp n tr0 tr1
      (v :: m) l1 cs1 hp1 n1 := dip_execFrom p v s l cs hp n tr0 tr1 m l1 cs1 hp1 n1 hpRun
  have hsecond : ExecFrom D H [.block q, .dip] (v :: m) l1 cs1 hp1 n1
      (tr0 ++ tr1) tr2 (v :: u) l2 cs2 hp2 n2 :=
    dip_execFrom q v m l1 cs1 hp1 n1 (tr0 ++ tr1) tr2 u l2 cs2 hp2 n2 hqRun
  have hcomp : ExecFrom D H (p ++ q) s l cs hp n tr0 (tr1 ++ tr2) u l2 cs2 hp2 n2 :=
    execFrom_append hpRun hqRun
  exact ⟨execFrom_append hfirst hsecond,
    dip_execFrom (p ++ q) v s l cs hp n tr0 (tr1 ++ tr2) u l2 cs2 hp2 n2 hcomp⟩

/-- L6a: `pair unpair` returns the original stack and issues no request. -/
theorem pair_unpair_exec {a b : Value} {s : List Value} {l cs hp n tr0} :
    ExecFrom D H [.pair, .unpair] (a :: b :: s) l cs hp n tr0 [] (a :: b :: s)
      l cs hp n :=
  execFrom_then (pair_execFrom a b s l cs hp n tr0) (unpair_execFrom a b s l cs hp n tr0)

/-- L6b: `unpair pair` returns the original pair. -/
theorem unpair_pair_exec {a b : Value} {s : List Value} {l cs hp n tr0} :
    ExecFrom D H [.unpair, .pair] (.pair a b :: s) l cs hp n tr0 [] (.pair a b :: s)
      l cs hp n :=
  execFrom_then (unpair_execFrom a b s l cs hp n tr0) (pair_execFrom a b s l cs hp n tr0)

/-- L6c: `swap_pair` is an involution on a pair and issues no request. -/
theorem swap_swap_exec {a b : Value} {s : List Value} {l cs hp n tr0} :
    ExecFrom D H [.swap, .swap] (a :: b :: s) l cs hp n tr0 [] (a :: b :: s)
      l cs hp n := by
  have h1 : ExecFrom D H [.swap] (a :: b :: s) l cs hp n tr0 [] (b :: a :: s)
      l cs hp n := swap_execFrom b a s l cs hp n tr0
  exact execFrom_then h1 (swap_execFrom a b s l cs hp n tr0)

/-- L7, first fusion: `[p] first [q] first` and `[p q] first` produce the
same pair and the trace `p`'s requests then `q`'s. -/
theorem first_fuse {p q : List Instr} {a b c d : Value} {s : List Value}
    {l cs hp n : _} {tr0 tr1 : List Event} {l1 cs1 hp1 n1 : _}
    {tr2 : List Event} {l2 cs2 hp2 n2 : _}
    (hpRun : ExecFrom D H p (a :: s) l cs hp n tr0 tr1 (c :: s) l1 cs1 hp1 n1)
    (hqRun : ExecFrom D H q (c :: s) l1 cs1 hp1 n1 (tr0 ++ tr1) tr2 (d :: s)
      l2 cs2 hp2 n2) :
    ExecFrom D H ([.push (.prog p)] ++ firstWord ++ [.push (.prog q)] ++ firstWord)
        (.pair a b :: s) l cs hp n tr0 (tr1 ++ tr2) (.pair d b :: s) l2 cs2 hp2 n2 ∧
      ExecFrom D H ([.push (.prog (p ++ q))] ++ firstWord) (.pair a b :: s) l cs hp n
        tr0 (tr1 ++ tr2) (.pair d b :: s) l2 cs2 hp2 n2 := by
  have hfirst : ExecFrom D H firstWord (.prog p :: .pair a b :: s) l cs hp n tr0 tr1
      (.pair c b :: s) l1 cs1 hp1 n1 := first_execFrom hpRun
  have hpushQ : ExecFrom D H [.push (.prog q)] (.pair c b :: s) l1 cs1 hp1 n1
      (tr0 ++ tr1) [] (.prog q :: .pair c b :: s) l1 cs1 hp1 n1 :=
    push_execFrom (.prog q) (.pair c b :: s) l1 cs1 hp1 n1 (tr0 ++ tr1)
  have hsecond : ExecFrom D H firstWord (.prog q :: .pair c b :: s) l1 cs1 hp1 n1
      (tr0 ++ tr1) tr2 (.pair d b :: s) l2 cs2 hp2 n2 := first_execFrom hqRun
  have hleft : ExecFrom D H (firstWord ++ [.push (.prog q)] ++ firstWord)
      (.prog p :: .pair a b :: s) l cs hp n tr0 (tr1 ++ tr2) (.pair d b :: s)
      l2 cs2 hp2 n2 := by
    refine execFrom_append (p := firstWord) hfirst ?_
    refine execFrom_append (p := [.push (.prog q)]) hpushQ ?_
    simpa [ExecFrom, List.append_nil] using hsecond
  have hcomp : ExecFrom D H (p ++ q) (a :: s) l cs hp n tr0 (tr1 ++ tr2)
      (d :: s) l2 cs2 hp2 n2 := execFrom_append hpRun hqRun
  have hright : ExecFrom D H firstWord (.prog (p ++ q) :: .pair a b :: s)
      l cs hp n tr0 (tr1 ++ tr2) (.pair d b :: s) l2 cs2 hp2 n2 :=
    first_execFrom hcomp
  have hpushP : ExecFrom D H [.push (.prog p)] (.pair a b :: s) l cs hp n tr0 []
      (.prog p :: .pair a b :: s) l cs hp n :=
    push_execFrom (.prog p) (.pair a b :: s) l cs hp n tr0
  have hpushPQ : ExecFrom D H [.push (.prog (p ++ q))] (.pair a b :: s) l cs hp n
      tr0 [] (.prog (p ++ q) :: .pair a b :: s) l cs hp n :=
    push_execFrom (.prog (p ++ q)) (.pair a b :: s) l cs hp n tr0
  have hleftFull : ExecFrom D H ([.push (.prog p)] ++
      (firstWord ++ [.push (.prog q)] ++ firstWord)) (.pair a b :: s) l cs hp n tr0
      (tr1 ++ tr2) (.pair d b :: s) l2 cs2 hp2 n2 :=
    execFrom_append (tr1 := []) hpushP (by simpa [ExecFrom, List.append_nil] using hleft)
  have hrightFull : ExecFrom D H ([.push (.prog (p ++ q))] ++ firstWord)
      (.pair a b :: s) l cs hp n tr0 (tr1 ++ tr2) (.pair d b :: s) l2 cs2 hp2 n2 :=
    execFrom_append (tr1 := []) hpushPQ (by simpa [ExecFrom, List.append_nil] using hright)
  exact ⟨by simpa [ExecFrom, List.append_nil] using hleftFull,
    by simpa [ExecFrom, List.append_nil] using hrightFull⟩

/-- L8, split definition: `w.split` and `first` then `second` have the same
trace and the same pair. -/
theorem split_first_second {p q : List Instr} {a b c d : Value} {s : List Value}
    {l cs hp n : _} {tr0 tr1 : List Event} {l1 cs1 hp1 n1 : _}
    {tr2 : List Event} {l2 cs2 hp2 n2 : _}
    (hpRun : ExecFrom D H p (a :: s) l cs hp n tr0 tr1 (c :: s) l1 cs1 hp1 n1)
    (hqRun : ExecFrom D H q (b :: s) l1 cs1 hp1 n1 (tr0 ++ tr1) tr2 (d :: s)
      l2 cs2 hp2 n2) :
    ExecFrom D H splitWord (.prog q :: .prog p :: .pair a b :: s) l cs hp n tr0
        (tr1 ++ tr2) (.pair c d :: s) l2 cs2 hp2 n2 ∧
      ExecFrom D H ([.push (.prog p)] ++ firstWord ++ [.push (.prog q)] ++ secondWord)
        (.pair a b :: s) l cs hp n tr0 (tr1 ++ tr2) (.pair c d :: s) l2 cs2 hp2 n2 := by
  have hsplit : ExecFrom D H splitWord (.prog q :: .prog p :: .pair a b :: s)
      l cs hp n tr0 (tr1 ++ tr2) (.pair c d :: s) l2 cs2 hp2 n2 :=
    split_execFrom hpRun hqRun
  have hfirst : ExecFrom D H firstWord (.prog p :: .pair a b :: s) l cs hp n tr0 tr1
      (.pair c b :: s) l1 cs1 hp1 n1 := first_execFrom hpRun
  have hpushQ : ExecFrom D H [.push (.prog q)] (.pair c b :: s) l1 cs1 hp1 n1
      (tr0 ++ tr1) [] (.prog q :: .pair c b :: s) l1 cs1 hp1 n1 :=
    push_execFrom (.prog q) (.pair c b :: s) l1 cs1 hp1 n1 (tr0 ++ tr1)
  have hsecond : ExecFrom D H secondWord (.prog q :: .pair c b :: s) l1 cs1 hp1 n1
      (tr0 ++ tr1) tr2 (.pair c d :: s) l2 cs2 hp2 n2 := second_execFrom hqRun
  have hright : ExecFrom D H ([.push (.prog p)] ++ firstWord ++ [.push (.prog q)] ++
      secondWord) (.pair a b :: s) l cs hp n tr0 (tr1 ++ tr2) (.pair c d :: s)
      l2 cs2 hp2 n2 := by
    have hsteps : ExecFrom D H (firstWord ++ [.push (.prog q)] ++ secondWord)
        (.prog p :: .pair a b :: s) l cs hp n tr0 (tr1 ++ tr2) (.pair c d :: s)
        l2 cs2 hp2 n2 := by
      refine execFrom_append (p := firstWord) hfirst ?_
      refine execFrom_append (p := [.push (.prog q)]) hpushQ ?_
      simpa [ExecFrom, List.append_nil] using hsecond
    have hpushP : ExecFrom D H [.push (.prog p)] (.pair a b :: s) l cs hp n tr0 []
        (.prog p :: .pair a b :: s) l cs hp n :=
      push_execFrom (.prog p) (.pair a b :: s) l cs hp n tr0
    have hfull : ExecFrom D H ([.push (.prog p)] ++
        (firstWord ++ [.push (.prog q)] ++ secondWord)) (.pair a b :: s) l cs hp n
        tr0 (tr1 ++ tr2) (.pair c d :: s) l2 cs2 hp2 n2 :=
      execFrom_append (tr1 := []) hpushP (by simpa [ExecFrom, List.append_nil] using hsteps)
    simpa [ExecFrom, List.append_nil] using hfull
  exact ⟨hsplit, hright⟩

/-- L10, fanout: duplicating a `Data` input and splitting is the fanout
word; the trace is still `p`'s then `q`'s. -/
theorem fanout_dup_pair_split {p q : List Instr} {x y z : Value} {s : List Value}
    {l cs hp n : _} {tr0 tr1 : List Event} {l1 cs1 hp1 n1 : _}
    {tr2 : List Event} {l2 cs2 hp2 n2 : _}
    (hpRun : ExecFrom D H p (x :: s) l cs hp n tr0 tr1 (y :: s) l1 cs1 hp1 n1)
    (hqRun : ExecFrom D H q (x :: s) l1 cs1 hp1 n1 (tr0 ++ tr1) tr2 (z :: s)
      l2 cs2 hp2 n2) :
    ExecFrom D H fanoutWord (.prog q :: .prog p :: x :: s) l cs hp n tr0
        (tr1 ++ tr2) (.pair y z :: s) l2 cs2 hp2 n2 ∧
      ExecFrom D H splitWord (.prog q :: .prog p :: .pair x x :: s) l cs hp n tr0
        (tr1 ++ tr2) (.pair y z :: s) l2 cs2 hp2 n2 :=
  ⟨fanout_execFrom hpRun hqRun, split_execFrom hpRun hqRun⟩

/-- L11, left β on `inl`: the selected program runs and the result is
wrapped; the trace is exactly the program's. -/
theorem left_beta_inl {p : List Instr} {a c : Value} {s : List Value}
    {l cs hp n : _} {tr0 tr : List Event} {l' cs' hp' n' : _}
    (hpRun : ExecFrom D H p (a :: s) l cs hp n tr0 tr (c :: s) l' cs' hp' n') :
    ExecFrom D H leftWord (.prog p :: .inl a :: s) l cs hp n tr0 tr
      (.inl c :: s) l' cs' hp' n' := left_inl_execFrom hpRun

/-- L11, left β on `inr`: the payload is returned and the program never
runs, so no request is issued. -/
theorem left_beta_inr {p : List Instr} {b : Value} {s : List Value}
    {l cs : List Nat} {hp : List (Nat × Value)} {n : Nat} {tr0 : List Event} :
    ExecFrom D H leftWord (.prog p :: .inr b :: s) l cs hp n tr0 []
      (.inr b :: s) l cs hp n := left_inr_execFrom

/-- L13, branch β: a nonzero condition runs the deeper program. -/
theorem branch_true {p q : List Instr} {x : BitVec 64} {s : List Value}
    {l cs hp n : _} {tr0 tr : List Event} {t : List Value} {l' cs' hp' n' : _}
    (hx : x ≠ 0) (hpRun : ExecFrom D H p s l cs hp n tr0 tr t l' cs' hp' n') :
    ExecFrom D H [.ifte] (.prog q :: .prog p :: .i64 x :: s) l cs hp n tr0 tr
      t l' cs' hp' n' := ifte_true_execFrom p q x s l cs hp n tr0 tr t l' cs' hp' n'
        hx hpRun

/-- L13, branch β: zero runs the top program. -/
theorem branch_false {p q : List Instr} {x : BitVec 64} {s : List Value}
    {l cs hp n : _} {tr0 tr : List Event} {t : List Value} {l' cs' hp' n' : _}
    (hx : x = 0) (hqRun : ExecFrom D H q s l cs hp n tr0 tr t l' cs' hp' n') :
    ExecFrom D H [.ifte] (.prog q :: .prog p :: .i64 x :: s) l cs hp n tr0 tr
      t l' cs' hp' n' := ifte_false_execFrom p q x s l cs hp n tr0 tr t l' cs' hp' n'
        hx hqRun

/-- L14: `[ 1 1 + ]` and `[ 2 ]` have the same normal result and trace (no
request), but they are different programs. Behavioral equality is not
definition or proof-subject identity. -/
theorem behavioral_not_identity :
    ExecFrom D H [.push (.i64 1), .push (.i64 1), .add] [] [] [] [] 0 [] []
      [.i64 2] [] [] [] 0 ∧
    ExecFrom D H [.push (.i64 2)] [] [] [] [] 0 [] [] [.i64 2] [] [] [] 0 ∧
    ([.push (.i64 1), .push (.i64 1), .add] : List Instr) ≠ [.push (.i64 2)] := by
  refine ⟨?_, ?_, ?_⟩
  · exact execFrom_then
      (execFrom_then (push_execFrom (.i64 1) [] [] [] [] 0 [])
        (push_execFrom (.i64 1) [.i64 1] [] [] [] 0 []))
      (by
        have hadd : ExecFrom D H [.add] [.i64 1, .i64 1] [] [] [] 0 [] [] [.i64 2]
            [] [] [] 0 := by
          simpa [ExecFrom] using reach_step (D := D) (H := H) (Step.add (a := 1) (b := 1)
            (k := []) (s := []) (l := []) (cs := []) (hp := []) (n := 0) (tr := []))
        simpa [ExecFrom, List.append_nil] using hadd)
  · exact push_execFrom (.i64 2) [] [] [] [] 0 []
  · intro h
    have hlen := congrArg List.length h
    simp at hlen


/-! ## F: failure class and cleanup responsibility

The guest step relation distinguishes three statuses: `running`, `denied`
and `failed`. A trap, cancellation or quota exhaustion is a host-profile
classification of a request or an execution budget, not a fourth guest
status; the model carries them in `FailureClass` and never relabels a
forbidden stuck state as one of them (`typed_not_stuck`, `underflow_stuck`).

The host boundary is a separate structure. Normal-path preservation is a
guest claim (`Own`); abnormal cleanup is a host claim: the invocation's
pending handles are retired once, repeat retirement is a no-op, and a
retired id is never reissued because the fresh counter only grows. -/

/-- The failure classes the design's F relation names. `denied` and
`failed` are the two guest statuses the step relation can reach; `trapped`,
`cancelled` and `exhausted` are host-profile classifications. -/
inductive FailureClass
  | denied
  | failed
  | trapped
  | cancelled
  | exhausted
  deriving DecidableEq, Repr

/-- A guest status classifies to `none` while running and to the matching
failure class when halted. -/
def FailureClass.ofStatus : Status → Option FailureClass
  | .running => none
  | .denied => some .denied
  | .failed => some .failed

/-- The model's outcomes. `waiting` is not produced by the synchronous host
oracle (there is no pending answer); `diverging` is an infinite `Reach`
sequence; `stuck` is the forbidden state. -/
inductive Outcome
  | normal
  | abnormal (failure : FailureClass)
  | waiting
  | diverging
  | stuck

/-- A normal return: the guest is still running with no code left. Ordinary
`Result` error values are values on the stack, so they are normal returns,
not abnormal terminations. -/
def NormalReturn (c : Config) : Prop := c.status = .running ∧ c.code = []

/-- An abnormal termination: the guest status is no longer `running`. The
step relation never resumes such a configuration (`halted_no_step`). -/
def Abnormal (c : Config) : Prop := c.status ≠ .running

/-- A forbidden stuck state: still running, with code left, and no step. A
typed configuration has none (`typed_not_stuck`). -/
def Stuck (D : List Def) (H : Nat → Response) (c : Config) : Prop :=
  c.status = .running ∧ c.code ≠ [] ∧ ¬ ∃ c', Step D H c c'

/-- Typed running configurations are never stuck: the progress theorem
covers every host answer. -/
theorem typed_not_stuck {D H E c} (hw : WT D E c) (hr : c.status = .running)
    (hne : c.code ≠ []) : ¬ Stuck D H c := by
  intro hs
  exact hs.2.2 (progress hw hr hne)

/-- A stack underflow is a stuck untyped configuration, not a trap: there is
no step and the status is still `running`. A checker must reject it, not
report an allowed abnormal termination. -/
theorem underflow_stuck :
    Stuck D H ⟨[.add], [], [], [], [], 0, [], .running⟩ := by
  refine ⟨rfl, by simp, ?_⟩
  intro ⟨c', hstep⟩
  cases hstep

/-- A wrong operand shape is also stuck, not a trap. -/
theorem wrong_shape_stuck :
    Stuck D H ⟨[.add], [.i64 1], [], [], [], 0, [], .running⟩ := by
  refine ⟨rfl, by simp, ?_⟩
  intro ⟨c', hstep⟩
  cases hstep

/-- An ordinary error value (an `inl` sum payload) is a normal return: the
stack carries the error and the status stays `running`. -/
theorem result_error_is_normal :
    NormalReturn ⟨[], [.inl (.i64 7)], [], [], [], 0, [], .running⟩ := ⟨rfl, rfl⟩

/-- A stack-first host boundary. `live` is the invocation's pending handle
multiset, `retired` the already-retired ids (in retirement order), and
`next` the guest's fresh-id counter. -/
structure Boundary where
  live : List Nat
  retired : List Nat
  next : Nat

/-- Retire one id: remove every occurrence from `live` and record the id in
`retired` unless it was already there. The `if` makes repeat retirement a
no-op. -/
def Boundary.retireOne (b : Boundary) (m : Nat) : Boundary :=
  { live := b.live.filter (fun x => x != m),
    retired := if m ∈ b.retired then b.retired else m :: b.retired,
    next := b.next }

/-- Retire a list of ids in order. -/
def Boundary.retire (b : Boundary) (ms : List Nat) : Boundary :=
  ms.foldl (fun b m => b.retireOne m) b

/-- A later guest acquisition of the fresh id. -/
def Boundary.acquire (b : Boundary) : Boundary :=
  { live := b.next :: b.live, retired := b.retired, next := b.next + 1 }

/-- The boundary invariant needed for cleanup: `live` and `retired` are
disjoint. -/
structure Boundary.Sound (b : Boundary) : Prop where
  disjoint : ∀ m, m ∈ b.live → m ∉ b.retired

/-- `retireOne` removes its own id from `live`. -/
theorem retireOne_live {b : Boundary} {m : Nat} : m ∉ (b.retireOne m).live := by
  simp [Boundary.retireOne, List.mem_filter]

/-- `retireOne` records its id in `retired`. -/
theorem retireOne_retired {b : Boundary} {m : Nat} : m ∈ (b.retireOne m).retired := by
  by_cases h : m ∈ b.retired
  · simp [Boundary.retireOne, h]
  · simp [Boundary.retireOne, h]

/-- `retireOne` only removes from `live`. -/
theorem retireOne_live_subset (b : Boundary) (m : Nat) :
    (b.retireOne m).live ⊆ b.live := by
  intro x hx
  simp [Boundary.retireOne, List.mem_filter] at hx
  exact hx.1

/-- `retireOne` only adds to `retired`. -/
theorem retireOne_retired_superset (b : Boundary) (m : Nat) :
    ∀ x, x ∈ b.retired → x ∈ (b.retireOne m).retired := by
  intro x hx
  by_cases h : m ∈ b.retired
  · simpa [Boundary.retireOne, h] using hx
  · simp [Boundary.retireOne, h]
    exact Or.inr hx

/-- Repeating one retirement changes nothing. -/
theorem retireOne_idempotent (b : Boundary) (m : Nat) :
    (b.retireOne m).retireOne m = b.retireOne m := by
  simp only [Boundary.retireOne]
  have hlive : (b.live.filter (fun x => x != m)).filter (fun x => x != m) =
      b.live.filter (fun x => x != m) := by
    rw [List.filter_filter]
    simp
  have hret : m ∈ (if m ∈ b.retired then b.retired else m :: b.retired) := by
    by_cases h : m ∈ b.retired <;> simp [h]
  simp [hlive, hret]

/-- An id that is already retired and no longer live is untouched by a
further retirement of itself. -/
theorem retireOne_identity {b : Boundary} {m : Nat} (hret : m ∈ b.retired)
    (hlive : m ∉ b.live) : b.retireOne m = b := by
  have hfilter : b.live.filter (fun x => x != m) = b.live := by
    apply List.filter_eq_self.mpr
    intro x hx
    exact bne_iff_ne.mpr (fun h => hlive (h ▸ hx))
  simp [Boundary.retireOne, hfilter, hret]

/-- Retirement only removes from `live`. -/
theorem retire_live_subset (b : Boundary) (ms : List Nat) :
    (b.retire ms).live ⊆ b.live := by
  induction ms generalizing b with
  | nil => intro x hx; exact hx
  | cons m rest ih =>
      intro x hx
      exact retireOne_live_subset b m (ih (b := b.retireOne m) hx)

/-- Retirement only adds to `retired`. -/
theorem retire_retired_superset (b : Boundary) (ms : List Nat) :
    ∀ x, x ∈ b.retired → x ∈ (b.retire ms).retired := by
  induction ms generalizing b with
  | nil => intro x hx; exact hx
  | cons m rest ih =>
      intro x hx
      simp only [Boundary.retire, List.foldl]
      exact ih (b.retireOne m) x (retireOne_retired_superset b m x hx)

/-- Retirement removes every id in the list from `live`. -/
theorem retire_live {b : Boundary} {ms : List Nat} {m : Nat} (hm : m ∈ ms) :
    m ∉ (b.retire ms).live := by
  induction ms generalizing b with
  | nil => simp at hm
  | cons x rest ih =>
      rcases List.mem_cons.mp hm with rfl | hm
      · intro hlive
        have hmem : m ∈ ((b.retireOne m).retire rest).live := by
          simpa [Boundary.retire] using hlive
        exact retireOne_live (retire_live_subset (b.retireOne m) rest hmem)
      · intro hlive
        have hmem : m ∈ ((b.retireOne x).retire rest).live := by
          simpa [Boundary.retire] using hlive
        exact ih hm hmem

/-- Retirement records every id in the list. -/
theorem retire_retired {b : Boundary} {ms : List Nat} {m : Nat} (hm : m ∈ ms) :
    m ∈ (b.retire ms).retired := by
  induction ms generalizing b with
  | nil => simp at hm
  | cons x rest ih =>
      rcases List.mem_cons.mp hm with rfl | hm
      · simp only [Boundary.retire, List.foldl]
        exact retire_retired_superset (b.retireOne m) rest m retireOne_retired
      · simp only [Boundary.retire, List.foldl]
        exact ih hm

/-- Repeating the retirement of one id is a no-op: the repeat-retirement
rule. (The general list form follows by induction over the list; this module
states the single-id rule it uses.) -/
theorem retire_single_idempotent (b : Boundary) (m : Nat) :
    (b.retire [m]).retire [m] = b.retire [m] := by
  simp [Boundary.retire, List.foldl, retireOne_idempotent]

/-- Retirement leaves the fresh counter unchanged. -/
theorem retire_next (b : Boundary) (ms : List Nat) :
    (b.retire ms).next = b.next := by
  induction ms generalizing b with
  | nil => rfl
  | cons m rest ih =>
      simpa [Boundary.retire, List.foldl, Boundary.retireOne]
        using ih (b.retireOne m)

/-- A retired id is never reissued: the guest's fresh counter is above it,
and an acquisition uses exactly the counter, not the retired id. -/
theorem retired_never_reacquired {b : Boundary} (hs : b.Sound) {m : Nat}
    (hret : m ∈ b.retired) (hfresh : m < b.next) :
    m ∉ b.acquire.live ∧ m ∈ b.acquire.retired := by
  refine ⟨?_, ?_⟩
  · intro h
    rcases List.mem_cons.mp h with rfl | h
    · exact Nat.lt_irrefl _ hfresh
    · exact hs.disjoint m h hret
  · simpa [Boundary.acquire] using hret

/-- Retirement followed by the fresh acquisition keeps the id retired and
absent from live. -/
theorem retire_then_acquire_keeps_retired {b : Boundary} {ms : List Nat} {m : Nat}
    (hm : m ∈ ms) (hfresh : m < b.next) :
    m ∈ ((b.retire ms).acquire).retired ∧ m ∉ ((b.retire ms).acquire).live := by
  have hret := retire_retired (b := b) hm
  have hnext : (b.retire ms).next = b.next := retire_next b ms
  refine ⟨by simpa [Boundary.acquire] using hret, ?_⟩
  intro h
  rcases List.mem_cons.mp (by simpa [Boundary.acquire, hnext] using h) with hcase | hcase
  · simp [hcase] at hfresh
  · exact retire_live (b := b) hm hcase

/-- Normal-path ownership stays with the guest: in a normal-return
configuration every live id is present in the whole-configuration owner
multiset, so it has not been silently retired. -/
theorem normal_cleanup_is_guest {c : Config} (ho : Own c) (_hn : NormalReturn c) :
    ∀ m ∈ c.live, m ∈ resOwners c := by
  intro m hm
  have hperm := ho.perm
  exact (hperm.mem_iff).mpr hm

/-- Abnormal cleanup is the host's: a halted guest takes no further step,
so the guest cannot run cleanup. -/
theorem abnormal_cleanup_is_host {c : Config} (ha : Abnormal c) :
    ¬ ∃ c', Step D H c c' := by
  intro ⟨c', hstep⟩
  exact halted_no_step ha hstep


/-! ## L9: the exchange counterexamples -/

/-- One emitting program: push an `I64` and request `test.emit`. It leaves
the stack below the pushed value untouched. -/
def emitWord (n : BitVec 64) : List Instr := [.push (.i64 n), .emit]

/-- One permitted emit step, with the event appended to the trace. -/
theorem emit_execFrom {H : Nat → Response} (n : BitVec 64) (a : Value)
    {s : List Value} {l cs hp n0 : _} {tr0 : List Event}
    (hH : H tr0.length = .permitted) :
    ExecFrom D H (emitWord n) (a :: s) l cs hp n0 tr0
      [⟨.emit, n.toNat, .permitted⟩] (a :: s) l cs hp n0 := by
  have hpush : Reach D H ⟨emitWord n, a :: s, l, cs, hp, n0, tr0, .running⟩
      ⟨[.emit], .i64 n :: a :: s, l, cs, hp, n0, tr0, .running⟩ :=
    reach_step .push
  have hem : Reach D H ⟨[.emit], .i64 n :: a :: s, l, cs, hp, n0, tr0, .running⟩
      ⟨[], a :: s, l, cs, hp, n0, tr0 ++ [⟨.emit, n.toNat, .permitted⟩], .running⟩ :=
    reach_step (.emitOk hH)
  exact hpush.trans hem

/-- L9 under T: the two sides of the exchange have the same normal result
but different ordered traces (`1, 2` versus `2, 1`). The exchange law is
therefore unsupported under T, exactly as the design's O11-5 witness says. -/
theorem exchange_changes_trace :
    ExecFrom D (fun _ => .permitted)
        ([.push (.prog (emitWord 1))] ++ firstWord ++
          [.push (.prog (emitWord 2))] ++ secondWord)
        (.pair .unit .unit :: []) [] [] [] 0 []
        [⟨.emit, 1, .permitted⟩, ⟨.emit, 2, .permitted⟩]
        (.pair .unit .unit :: []) [] [] [] 0 ∧
      ExecFrom D (fun _ => .permitted)
        ([.push (.prog (emitWord 2))] ++ secondWord ++
          [.push (.prog (emitWord 1))] ++ firstWord)
        (.pair .unit .unit :: []) [] [] [] 0 []
        [⟨.emit, 2, .permitted⟩, ⟨.emit, 1, .permitted⟩]
        (.pair .unit .unit :: []) [] [] [] 0 ∧
      ([⟨.emit, 1, .permitted⟩, ⟨.emit, 2, .permitted⟩] : List Event) ≠
        [⟨.emit, 2, .permitted⟩, ⟨.emit, 1, .permitted⟩] := by
  have hpRun : ExecFrom D (fun _ => .permitted) (emitWord 1) (.unit :: [])
      [] [] [] 0 [] [⟨.emit, 1, .permitted⟩] (.unit :: []) [] [] [] 0 :=
    emit_execFrom 1 .unit (H := fun _ => .permitted) (by simp)
  have hqRun : ExecFrom D (fun _ => .permitted) (emitWord 2) (.unit :: [])
      [] [] [] 0 [⟨.emit, 1, .permitted⟩] [⟨.emit, 2, .permitted⟩]
      (.unit :: []) [] [] [] 0 :=
    emit_execFrom 2 .unit (H := fun _ => .permitted) (by simp)
  have hfirst : ExecFrom D (fun _ => .permitted) firstWord
      (.prog (emitWord 1) :: .pair .unit .unit :: []) [] [] [] 0 []
      [⟨.emit, 1, .permitted⟩] (.pair .unit .unit :: []) [] [] [] 0 :=
    first_execFrom hpRun
  have hpushQ : ExecFrom D (fun _ => .permitted) [.push (.prog (emitWord 2))]
      (.pair .unit .unit :: []) [] [] [] 0 [⟨.emit, 1, .permitted⟩] []
      (.prog (emitWord 2) :: .pair .unit .unit :: []) [] [] [] 0 :=
    push_execFrom (.prog (emitWord 2)) (.pair .unit .unit :: []) [] [] [] 0
      [⟨.emit, 1, .permitted⟩]
  have hsecond : ExecFrom D (fun _ => .permitted) secondWord
      (.prog (emitWord 2) :: .pair .unit .unit :: []) [] [] [] 0
      [⟨.emit, 1, .permitted⟩] [⟨.emit, 2, .permitted⟩]
      (.pair .unit .unit :: []) [] [] [] 0 :=
    second_execFrom hqRun
  have hleft : ExecFrom D (fun _ => .permitted)
      ([.push (.prog (emitWord 1))] ++ firstWord ++
        [.push (.prog (emitWord 2))] ++ secondWord)
      (.pair .unit .unit :: []) [] [] [] 0 []
      [⟨.emit, 1, .permitted⟩, ⟨.emit, 2, .permitted⟩]
      (.pair .unit .unit :: []) [] [] [] 0 := by
    have hrest : ExecFrom D (fun _ => .permitted)
        (firstWord ++ [.push (.prog (emitWord 2))] ++ secondWord)
        (.prog (emitWord 1) :: .pair .unit .unit :: []) [] [] [] 0 []
        [⟨.emit, 1, .permitted⟩, ⟨.emit, 2, .permitted⟩]
        (.pair .unit .unit :: []) [] [] [] 0 := by
      refine execFrom_append (p := firstWord) hfirst ?_
      refine execFrom_append (p := [.push (.prog (emitWord 2))]) hpushQ ?_
      simpa [ExecFrom, List.append_nil] using hsecond
    have hpushP : ExecFrom D (fun _ => .permitted) [.push (.prog (emitWord 1))]
        (.pair .unit .unit :: []) [] [] [] 0 [] []
        (.prog (emitWord 1) :: .pair .unit .unit :: []) [] [] [] 0 :=
      push_execFrom (.prog (emitWord 1)) (.pair .unit .unit :: []) [] [] [] 0 []
    simpa [ExecFrom, List.append_nil] using
      execFrom_append (tr1 := []) hpushP (by simpa [ExecFrom, List.append_nil] using hrest)
  have hqRun2 : ExecFrom D (fun _ => .permitted) (emitWord 2) (.unit :: [])
      [] [] [] 0 [] [⟨.emit, 2, .permitted⟩] (.unit :: []) [] [] [] 0 :=
    emit_execFrom 2 .unit (H := fun _ => .permitted) (by simp)
  have hpRun2 : ExecFrom D (fun _ => .permitted) (emitWord 1) (.unit :: [])
      [] [] [] 0 [⟨.emit, 2, .permitted⟩] [⟨.emit, 1, .permitted⟩]
      (.unit :: []) [] [] [] 0 :=
    emit_execFrom 1 .unit (H := fun _ => .permitted) (by simp)
  have hsecond2 : ExecFrom D (fun _ => .permitted) secondWord
      (.prog (emitWord 2) :: .pair .unit .unit :: []) [] [] [] 0 []
      [⟨.emit, 2, .permitted⟩] (.pair .unit .unit :: []) [] [] [] 0 :=
    second_execFrom hqRun2
  have hpushP2 : ExecFrom D (fun _ => .permitted) [.push (.prog (emitWord 1))]
      (.pair .unit .unit :: []) [] [] [] 0 [⟨.emit, 2, .permitted⟩] []
      (.prog (emitWord 1) :: .pair .unit .unit :: []) [] [] [] 0 :=
    push_execFrom (.prog (emitWord 1)) (.pair .unit .unit :: []) [] [] [] 0
      [⟨.emit, 2, .permitted⟩]
  have hfirst2 : ExecFrom D (fun _ => .permitted) firstWord
      (.prog (emitWord 1) :: .pair .unit .unit :: []) [] [] [] 0
      [⟨.emit, 2, .permitted⟩] [⟨.emit, 1, .permitted⟩]
      (.pair .unit .unit :: []) [] [] [] 0 :=
    first_execFrom hpRun2
  have hright : ExecFrom D (fun _ => .permitted)
      ([.push (.prog (emitWord 2))] ++ secondWord ++
        [.push (.prog (emitWord 1))] ++ firstWord)
      (.pair .unit .unit :: []) [] [] [] 0 []
      [⟨.emit, 2, .permitted⟩, ⟨.emit, 1, .permitted⟩]
      (.pair .unit .unit :: []) [] [] [] 0 := by
    have hrest : ExecFrom D (fun _ => .permitted)
        (secondWord ++ [.push (.prog (emitWord 1))] ++ firstWord)
        (.prog (emitWord 2) :: .pair .unit .unit :: []) [] [] [] 0 []
        [⟨.emit, 2, .permitted⟩, ⟨.emit, 1, .permitted⟩]
        (.pair .unit .unit :: []) [] [] [] 0 := by
      refine execFrom_append (p := secondWord) hsecond2 ?_
      refine execFrom_append (p := [.push (.prog (emitWord 1))]) hpushP2 ?_
      simpa [ExecFrom, List.append_nil] using hfirst2
    have hpushQ0 : ExecFrom D (fun _ => .permitted) [.push (.prog (emitWord 2))]
        (.pair .unit .unit :: []) [] [] [] 0 [] []
        (.prog (emitWord 2) :: .pair .unit .unit :: []) [] [] [] 0 :=
      push_execFrom (.prog (emitWord 2)) (.pair .unit .unit :: []) [] [] [] 0 []
    simpa [ExecFrom, List.append_nil] using
      execFrom_append (tr1 := []) hpushQ0 (by simpa [ExecFrom, List.append_nil] using hrest)
  refine ⟨hleft, hright, ?_⟩
  intro h
  have hhead : (⟨.emit, 1, .permitted⟩ : Event) = ⟨.emit, 2, .permitted⟩ := by
    have := congrArg (fun l : List Event => l.head?) h
    exact Option.some.inj this
  have harg := congrArg Event.arg hhead
  simp at harg

/-! ## V-RESOURCE-02 corollaries: recursive `Data`/`Capture` exclusion -/

/-- A pair carrying a resource on either side is not copyable. -/
theorem pair_with_resource_not_copy :
    ¬ (Ty.pair Ty.i64 Ty.res).Copy ∧ ¬ (Ty.pair Ty.res Ty.i64).Copy := by
  refine ⟨?_, ?_⟩
  · intro h; exact h.2
  · intro h; exact h.1

/-- A sum carrying a resource in either alternative is not copyable. -/
theorem sum_with_resource_not_copy :
    ¬ (Ty.sum Ty.res Ty.i64).Copy ∧ ¬ (Ty.sum Ty.i64 Ty.res).Copy := by
  refine ⟨?_, ?_⟩
  · intro h; exact h.1
  · intro h; exact h.2

/-- A list whose element type carries a resource is not copyable. -/
theorem list_with_resource_not_copy : ¬ (Ty.list Ty.res).Copy := by
  intro h; exact h

/-- A Program interface may mention a resource without making the Program
value, or its body, resource-owning. -/
theorem program_interface_resource_is_copyable :
    (Ty.prog [Ty.res] [] []).Copy := trivial

/-- The wiring `fanout` word duplicates its shared input: a resource-bearing
pair is refused at the `dup`, so `fanout` has no specialization at a
resource. This is the O11-7 refusal at the model's typing judgment. -/
theorem fanout_at_resource_untypable {D E s out} :
    ¬ InstrT D E .dup (.pair .res .res :: s) out := by
  intro h
  cases h with
  | dup hc => exact hc.1

/-- `project_first` is `unpair drop`; dropping a resource-bearing payload is
refused, so the projection has no specialization at a resource. -/
theorem project_first_at_resource_untypable {D E s out} :
    ¬ InstrT D E .drop (.res :: s) out := fun h => by
  cases h with
  | drop hc => exact hc

/-- `quote` of a resource is refused: capture requires `Copy`, and a
resource is never copyable. -/
theorem quote_at_resource_untypable {D E s out} :
    ¬ InstrT D E .quote (.res :: s) out := fun h => by
  cases h with
  | quote hc => exact hc


/-- A host that permits the first request and denies every later one. This
is the F-side analogue of the ordered-effect witness: the exchange changes
which program's request is denied. -/
def permitThenDeny : Nat → Response
  | 0 => .permitted
  | _ => .denied

/-- L9 under F (denied request): both orders abort, but the failed request
and the recorded cleanup point differ (`1, 2` versus `2, 1`). A guest
typing proof does not establish that the exchange preserves host cleanup
responsibility, and the earlier permitted effect is not rolled back. -/
theorem exchange_changes_failure_point :
    Reach D permitThenDeny (init (emitWord 1 ++ emitWord 2))
        ⟨[.emit], [.i64 2], [], [], [], 0,
          [⟨.emit, 1, .permitted⟩, ⟨.emit, 2, .denied⟩], .denied⟩ ∧
      Reach D permitThenDeny (init (emitWord 2 ++ emitWord 1))
        ⟨[.emit], [.i64 1], [], [], [], 0,
          [⟨.emit, 2, .permitted⟩, ⟨.emit, 1, .denied⟩], .denied⟩ ∧
      ([⟨.emit, 1, .permitted⟩, ⟨.emit, 2, .denied⟩] : List Event) ≠
        [⟨.emit, 2, .permitted⟩, ⟨.emit, 1, .denied⟩] := by
  have hfwd : Reach D permitThenDeny (init (emitWord 1 ++ emitWord 2))
      ⟨[.emit], [.i64 2], [], [], [], 0,
        [⟨.emit, 1, .permitted⟩, ⟨.emit, 2, .denied⟩], .denied⟩ := by
    have h1 : Reach D permitThenDeny
        ⟨emitWord 1 ++ emitWord 2, [], [], [], [], 0, [], .running⟩
        ⟨[.emit, .push (.i64 2), .emit], [.i64 1], [], [], [], 0, [], .running⟩ :=
      reach_step .push
    have h2 : Reach D permitThenDeny
        ⟨[.emit, .push (.i64 2), .emit], [.i64 1], [], [], [], 0, [], .running⟩
        ⟨[.push (.i64 2), .emit], [], [], [], [], 0,
          [⟨.emit, 1, .permitted⟩], .running⟩ :=
      reach_step (.emitOk rfl)
    have h3 : Reach D permitThenDeny
        ⟨[.push (.i64 2), .emit], [], [], [], [], 0,
          [⟨.emit, 1, .permitted⟩], .running⟩
        ⟨[.emit], [.i64 2], [], [], [], 0,
          [⟨.emit, 1, .permitted⟩], .running⟩ :=
      reach_step .push
    have h4 : Reach D permitThenDeny
        ⟨[.emit], [.i64 2], [], [], [], 0,
          [⟨.emit, 1, .permitted⟩], .running⟩
        ⟨[.emit], [.i64 2], [], [], [], 0,
          [⟨.emit, 1, .permitted⟩, ⟨.emit, 2, .denied⟩], .denied⟩ :=
      reach_step (.emitStop rfl (by decide))
    exact h1.trans (h2.trans (h3.trans h4))
  have hrev : Reach D permitThenDeny (init (emitWord 2 ++ emitWord 1))
      ⟨[.emit], [.i64 1], [], [], [], 0,
        [⟨.emit, 2, .permitted⟩, ⟨.emit, 1, .denied⟩], .denied⟩ := by
    have h1 : Reach D permitThenDeny
        ⟨emitWord 2 ++ emitWord 1, [], [], [], [], 0, [], .running⟩
        ⟨[.emit, .push (.i64 1), .emit], [.i64 2], [], [], [], 0, [], .running⟩ :=
      reach_step .push
    have h2 : Reach D permitThenDeny
        ⟨[.emit, .push (.i64 1), .emit], [.i64 2], [], [], [], 0, [], .running⟩
        ⟨[.push (.i64 1), .emit], [], [], [], [], 0,
          [⟨.emit, 2, .permitted⟩], .running⟩ :=
      reach_step (.emitOk rfl)
    have h3 : Reach D permitThenDeny
        ⟨[.push (.i64 1), .emit], [], [], [], [], 0,
          [⟨.emit, 2, .permitted⟩], .running⟩
        ⟨[.emit], [.i64 1], [], [], [], 0,
          [⟨.emit, 2, .permitted⟩], .running⟩ :=
      reach_step .push
    have h4 : Reach D permitThenDeny
        ⟨[.emit], [.i64 1], [], [], [], 0,
          [⟨.emit, 2, .permitted⟩], .running⟩
        ⟨[.emit], [.i64 1], [], [], [], 0,
          [⟨.emit, 2, .permitted⟩, ⟨.emit, 1, .denied⟩], .denied⟩ :=
      reach_step (.emitStop rfl (by decide))
    exact h1.trans (h2.trans (h3.trans h4))
  refine ⟨hfwd, hrev, ?_⟩
  intro h
  have hhead : (⟨.emit, 1, .permitted⟩ : Event) = ⟨.emit, 2, .permitted⟩ := by
    have := congrArg (fun l : List Event => l.head?) h
    exact Option.some.inj this
  have harg := congrArg Event.arg hhead
  simp at harg


/-- Once a configuration is halted, no further step is possible, so every
reach from it is the trivial one and its trace is final. This is the F-side
"the right side never starts after an abort" property. -/
theorem halted_reach_eq {c c' : Config} (h : Reach D H c c')
    (hc : c.status ≠ .running) : c' = c := by
  induction h with
  | refl => rfl
  | tail hreach hstep ih =>
      have hb : _ = c := ih
      subst hb
      exact (halted_no_step hc hstep).elim

/-- A halted configuration's trace is final: earlier effects are not rolled
back and no later program appends a request. -/
theorem halted_trace_final {c c' : Config} (h : Reach D H c c')
    (hc : c.status ≠ .running) : c'.trace = c.trace := by
  rw [halted_reach_eq h hc]

/-- O for the `under` word: a held resource is in exactly one `restore`
frame while the body runs, the stack holds no owner, and the resource is
restored once and released on the normal path. -/
theorem under_holds_resource_in_frame :
    let H : Nat → Response := fun _ => .permitted
    Reach [] H (init [.acquire, .block [.push (.i64 7), .drop], .dip, .release])
        ⟨[.push (.i64 7), .drop, .restore (.res 0), .release], [], [0], [], [], 1,
          [⟨.acquire, 0, .permitted⟩], .running⟩ ∧
      Own ⟨[.push (.i64 7), .drop, .restore (.res 0), .release], [], [0], [], [], 1,
          [⟨.acquire, 0, .permitted⟩], .running⟩ ∧
      frameRes [.push (.i64 7), .drop, .restore (.res 0), .release] = [0] ∧
      stackRes ([] : List Value) = [] ∧
      Reach [] H (init [.acquire, .block [.push (.i64 7), .drop], .dip, .release])
        ⟨[], [], [], [], [], 1,
          [⟨.acquire, 0, .permitted⟩, ⟨.release, 0, .permitted⟩], .running⟩ := by
  intro H
  refine ⟨?_, ?_, ?_, ?_, ?_⟩
  · exact (((Relation.ReflTransGen.single (Step.acquireOk rfl)).tail
      (Step.block (p := [.push (.i64 7), .drop]))).tail
      (Step.dip (p := [.push (.i64 7), .drop]) (v := Value.res 0)))
  · refine ⟨?_, by simp, by simp⟩
    simp [resOwners, stackRes, frameRes, heapRes, Instr.heldRes, Value.resIds]
  · simp [frameRes, Instr.heldRes, Value.resIds]
  · rfl
  · exact ((((((Relation.ReflTransGen.single (Step.acquireOk rfl)).tail
      (Step.block (p := [.push (.i64 7), .drop]))).tail
      (Step.dip (p := [.push (.i64 7), .drop]) (v := Value.res 0))).tail
      (.push (v := .i64 7))).tail (.drop (v := .i64 7))).tail .restore).tail
      (Step.releaseOk rfl)


end NobleFoundation.WiringLaws
