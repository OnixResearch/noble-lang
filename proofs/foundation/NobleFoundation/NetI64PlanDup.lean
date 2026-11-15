import NobleFoundation.NetI64PlanInstance

/-!
# Emitted DUP acceptance in both orientations (issue #110)

From a legal arena, a current selected pair, a matching projection and its
redex alone, an actually lowered DUP image that is not excluded by queue
capacity executes and passes the independent semantic checker at its exact
reservations. The image has two number results, one per captured auxiliary
endpoint of the duplicator: the first donated by the duplicator slot and the
second by the number slot, both rewritten in place under reuse or both
appended under the fresh policy. Captures, both results' publications, the
candidate shape and the abstract image are all derived here, never assumed.
These are statements about the Lean model emitter, executor and checker only;
they make no claim about the Rust implementation.
-/

namespace NobleFoundation.NetI64Plans

open INet
open NobleFoundation.NetI64

/-- Installing the two number results of a DUP image, donated by the rule
and value slots, either rewrites both donors at their next generations
(reuse) or appends both at generation zero (fresh). -/
private theorem PhysicalState.dup_install_results
    {state : PhysicalState} {plan : BytePlan} {left right : Id}
    {rhs : XRhs NetSym} {reuse : Bool} {ruleSlot valueSlot : Nat}
    {ruleOld valueOld : PhysicalSlot} {symbol : NetSym}
    {ports0 ports1 : List (Option Nat)}
    {resolved0 resolved1 : List (Option (Nat × Nat))}
    {rhsSlot0 rhsSlot1 : Option Bool × Nat}
    (hrule : state.slots[ruleSlot]? = some ruleOld)
    (hruleLive : ruleOld.agent.isSome = true)
    (hruleRoom : ruleOld.generation ≠ UInt64.ofNat (UInt64.size - 1))
    (hvalue : state.slots[valueSlot]? = some valueOld)
    (hvalueLive : valueOld.agent.isSome = true)
    (hvalueRoom : valueOld.generation ≠ UInt64.ofNat (UInt64.size - 1))
    (hdistinct : ruleSlot ≠ valueSlot)
    (hrhs0 : rhs.slots[0]? = some rhsSlot0)
    (hlab0 : rhs.lab rhsSlot0.1 rhsSlot0.2 = some symbol)
    (hrhs1 : rhs.slots[1]? = some rhsSlot1)
    (hlab1 : rhs.lab rhsSlot1.1 rhsSlot1.2 = some symbol)
    (hports0 : ports0.mapM plan.resultTarget = some resolved0)
    (hports1 : ports1.mapM plan.resultTarget = some resolved1) :
    ∃ installed, PhysicalState.installResults plan left right rhs 0 state
        [(!reuse, if reuse then ruleSlot else state.slots.size, symbol, ruleSlot, ports0),
          (!reuse, if reuse then valueSlot else state.slots.size + 1, symbol, valueSlot,
            ports1)] = some installed ∧
      state.InstallPost installed
        [{ slot := if reuse then ruleSlot else state.slots.size
           id := child left right rhsSlot0.1 rhsSlot0.2
           symbol
           generation := if reuse then ruleOld.generation + 1 else 0
           ports := resolved0 },
          { slot := if reuse then valueSlot else state.slots.size + 1
            id := child left right rhsSlot1.1 rhsSlot1.2
            symbol
            generation := if reuse then valueOld.generation + 1 else 0
            ports := resolved1 }]
        (if reuse then 0 else 2) := by
  have hruleBound : ruleSlot < state.slots.size := (getElem?_eq_some_iff.mp hrule).1
  have hvalueBound : valueSlot < state.slots.size := (getElem?_eq_some_iff.mp hvalue).1
  cases reuse with
  | true =>
    obtain ⟨installed, hinstall, hsize, houts, hwork, hfree, hframe, hslots⟩ :=
      PhysicalState.installResults_reuse (plan := plan) (left := left) (right := right)
        (rhs := rhs)
        [(false, ruleSlot, symbol, ruleSlot, ports0),
          (false, valueSlot, symbol, valueSlot, ports1)] 0 state (by simp [hdistinct])
        (by
          intro j result hj
          match j, hj with
          | 0, hj =>
            simp at hj
            subst result
            exact ⟨rfl, rfl, ⟨ruleOld, hrule, hruleLive, hruleRoom⟩, rhsSlot0, resolved0,
              hrhs0, hlab0, hports0⟩
          | 1, hj =>
            simp at hj
            subst result
            exact ⟨rfl, rfl, ⟨valueOld, hvalue, hvalueLive, hvalueRoom⟩, rhsSlot1, resolved1,
              hrhs1, hlab1, hports1⟩
          | _ + 2, hj => simp at hj)
    refine ⟨installed, by simpa using hinstall,
      ⟨by simpa using hsize, houts, hwork, hfree, ?_, ?_⟩⟩
    · intro index _ hnot
      exact hframe index (by simpa using hnot)
    · intro result hresult
      simp at hresult
      rcases hresult with rfl | rfl
      · simpa using hslots 0 (false, ruleSlot, symbol, ruleSlot, ports0) rhsSlot0 resolved0
          ruleOld (by simp) hrhs0 hports0 hrule
      · simpa using hslots 1 (false, valueSlot, symbol, valueSlot, ports1) rhsSlot1
          resolved1 valueOld (by simp) hrhs1 hports1 hvalue
  | false =>
    obtain ⟨installed, hinstall, hsize, houts, hwork, hfree, hframe, hslots⟩ :=
      PhysicalState.installResults_fresh (plan := plan) (left := left) (right := right)
        (rhs := rhs)
        [(true, state.slots.size, symbol, ruleSlot, ports0),
          (true, state.slots.size + 1, symbol, valueSlot, ports1)] 0 state
        (by
          intro j result hj
          match j, hj with
          | 0, hj =>
            simp at hj
            subst result
            exact ⟨rfl, rfl, hruleBound, ⟨ruleOld, hrule, hruleLive⟩, rhsSlot0, resolved0,
              hrhs0, hlab0, hports0⟩
          | 1, hj =>
            simp at hj
            subst result
            exact ⟨rfl, rfl, hvalueBound, ⟨valueOld, hvalue, hvalueLive⟩, rhsSlot1,
              resolved1, hrhs1, hlab1, hports1⟩
          | _ + 2, hj => simp at hj)
    refine ⟨installed, by simpa using hinstall,
      ⟨by simpa using hsize, houts, hwork, hfree, ?_, ?_⟩⟩
    · intro index hindex _
      exact hframe index hindex
    · intro result hresult
      simp at hresult
      rcases hresult with rfl | rfl
      · simpa using hslots 0 (true, state.slots.size, symbol, ruleSlot, ports0) rhsSlot0
          resolved0 (by simp) hrhs0 hports0
      · simpa using hslots 1 (true, state.slots.size + 1, symbol, valueSlot, ports1)
          rhsSlot1 resolved1 (by simp) hrhs1 hports1

/-- One DUP result's published pairs: none when its captured endpoint is an
output or a nonprincipal peer port, otherwise exactly the canonical pair of the
result slot at its published generation and that live peer. -/
private theorem PhysicalState.dup_publication_shape
    {state : PhysicalState} {pair : PhysicalPair}
    {boundaries : List ((Nat × Nat) × (Nat × Nat × Nat))}
    {index ruleSlot valueSlot sourcePort donor fresh : Nat} {endpoint : Port}
    {boundary : (Nat × Nat) × (Nat × Nat × Nat)} {reuse : Bool}
    {symbol : NetSym} {ports : List (Option Nat)} {old : PhysicalSlot}
    {published : List (Nat × Nat × Nat × Nat)} {view : BytePlan}
    (hview : view.publications = published)
    (hport : ports[0]? = some (some index))
    (hboundary : boundaries[index]? = some boundary)
    (hcapture : (∃ peer peerPort peerSlot peerId peerSymbol,
          boundary.2 = (0, peer, peerPort) ∧ peer ≠ ruleSlot ∧ peer ≠ valueSlot ∧
          state.slots[peer]? = some peerSlot ∧
          peerSlot.agent = some (peerId, peerSymbol) ∧
          peerSlot.ports[peerPort]? = some (some (ruleSlot, sourcePort)) ∧
          endpoint = (peerId, peerPort)) ∨
       (∃ output out, boundary.2 = (1, output, 0) ∧
          state.outputs[output]? = some out ∧
          out.target = (ruleSlot, sourcePort) ∧ endpoint = out.label))
    (hpair : (pair.first = ruleSlot ∧ pair.second = valueSlot) ∨
      (pair.first = valueSlot ∧ pair.second = ruleSlot))
    (hdonor : donor = ruleSlot ∨ donor = valueSlot)
    (hfresh : state.slots.size ≤ fresh)
    (hold : state.slots[donor]? = some old)
    (hroom : old.generation ≠ UInt64.ofNat (UInt64.size - 1))
    (hpub : state.publicationFor pair boundaries
      (!reuse, if reuse then donor else fresh, symbol, donor, ports) = some published) :
    (view.publishedPairs = [] ∧ ∀ peer, boundary.2 ≠ (0, peer, 0)) ∨
      ∃ peer peerSlot, boundary.2 = (0, peer, 0) ∧ state.slots[peer]? = some peerSlot ∧
        peer ≠ ruleSlot ∧ peer ≠ valueSlot ∧
        view.publishedPairs = [PhysicalState.canonicalPair
          (if reuse then donor else fresh) (if reuse then old.generation + 1 else 0)
          peer peerSlot.generation] := by
  rcases hcapture with ⟨peer, peerPort, peerSlot, peerId, peerSymbol, hk, h1, h2, hpeer,
      hpeerAgent, _, _⟩ | ⟨output, out, hk, _, _, _⟩
  · by_cases hz : peerPort = 0
    · subst hz
      right
      refine ⟨peer, peerSlot, hk, hpeer, h1, h2, ?_⟩
      have hf : peer ≠ pair.first := by
        rcases hpair with ⟨hf, _⟩ | ⟨hf, _⟩ <;> rw [hf] <;> assumption
      have hs : peer ≠ pair.second := by
        rcases hpair with ⟨_, hs⟩ | ⟨_, hs⟩ <;> rw [hs] <;> assumption
      have hpeerBound : peer < state.slots.size := (getElem?_eq_some_iff.mp hpeer).1
      have hpeerLive : peerSlot.agent.isSome = true := by simp [hpeerAgent]
      cases reuse with
      | true =>
        have hp := PhysicalState.publicationFor_reuse_active
          (boundaries := boundaries) (index := index) (pair := pair)
          (result := (!true, if true then donor else fresh, symbol, donor, ports))
          hport hboundary hk hf hs
          (by
            simp only [↓reduceIte]
            rcases hdonor with h | h <;> rw [h] <;> assumption)
          hpeer hpeerLive rfl hold hroom
        rw [hp] at hpub
        have hpubs := (Option.some.inj hpub).symm
        simp only [BytePlan.publishedPairs, hview, hpubs]
        by_cases hlt : donor < peer
        · simp [hlt, PhysicalState.canonicalPair, UInt64.ofNat_add, UInt64.ofNat_toNat]
        · simp [hlt, PhysicalState.canonicalPair, UInt64.ofNat_add, UInt64.ofNat_toNat]
      | false =>
        have hp := PhysicalState.publicationFor_fresh_active
          (boundaries := boundaries) (index := index) (pair := pair)
          (result := (!false, if false then donor else fresh, symbol, donor, ports))
          hport hboundary hk hf hs
          (by
            simp only [Bool.false_eq_true, ↓reduceIte]
            omega)
          hpeer hpeerLive rfl
        rw [hp] at hpub
        have hpubs := (Option.some.inj hpub).symm
        simp only [BytePlan.publishedPairs, hview, hpubs]
        by_cases hlt : fresh < peer
        · simp [hlt, PhysicalState.canonicalPair, UInt64.ofNat_toNat]
        · simp [hlt, PhysicalState.canonicalPair, UInt64.ofNat_toNat]
    · left
      have hp := PhysicalState.publicationFor_quiet (state := state)
        (boundaries := boundaries) (index := index) (pair := pair)
        (result := (!reuse, if reuse then donor else fresh, symbol, donor, ports))
        hport hboundary (.inr (by rw [hk]; exact hz))
      rw [hp] at hpub
      refine ⟨by simp [BytePlan.publishedPairs, hview, ← Option.some.inj hpub], ?_⟩
      intro peer' h
      rw [hk] at h
      simp at h
      exact hz h.2
  · left
    have hp := PhysicalState.publicationFor_quiet (state := state)
      (boundaries := boundaries) (index := index) (pair := pair)
      (result := (!reuse, if reuse then donor else fresh, symbol, donor, ports))
      hport hboundary (.inl (by simp [hk]))
    rw [hp] at hpub
    refine ⟨by simp [BytePlan.publishedPairs, hview, ← Option.some.inj hpub], ?_⟩
    intro peer' h
    rw [hk] at h
    simp at h

/-- Membership in one result's published pairs: exactly the canonical pair of
the result slot and a live peer at its captured principal endpoint. -/
private theorem PhysicalState.dup_published_mem
    {state : PhysicalState} {published : List PhysicalPair}
    {boundary : (Nat × Nat) × (Nat × Nat × Nat)} {ruleSlot valueSlot slot : Nat}
    {generation : UInt64}
    (hshape : (published = [] ∧ ∀ peer, boundary.2 ≠ (0, peer, 0)) ∨
      ∃ peer peerSlot, boundary.2 = (0, peer, 0) ∧ state.slots[peer]? = some peerSlot ∧
        peer ≠ ruleSlot ∧ peer ≠ valueSlot ∧
        published = [PhysicalState.canonicalPair slot generation peer peerSlot.generation])
    (queued : PhysicalPair) :
    queued ∈ published ↔ ∃ peer peerSlot, boundary.2 = (0, peer, 0) ∧
      state.slots[peer]? = some peerSlot ∧
      queued = PhysicalState.canonicalPair slot generation peer peerSlot.generation := by
  rcases hshape with ⟨hnone, hquiet⟩ | ⟨peer, peerSlot, hk, hpeer, _, _, hone⟩
  · rw [hnone]
    simp only [List.not_mem_nil, false_iff, not_exists, not_and]
    intro peer _ hk
    exact absurd hk (hquiet peer)
  · rw [hone, List.mem_singleton]
    constructor
    · intro hq
      exact ⟨peer, peerSlot, hk, hpeer, hq⟩
    · rintro ⟨peer', peerSlot', hk', hpeer', hq⟩
      have hpeerEq : peer' = peer := by
        have h := hk'.symm.trans hk
        simp at h
        exact h
      subst hpeerEq
      rw [hpeer] at hpeer'
      cases hpeer'
      exact hq

/-- The endpoints of a canonical pair are its slot and its peer, in index
order. -/
private theorem PhysicalState.canonicalPair_ends (slot : Nat) (generation : UInt64)
    (peer : Nat) (peerGeneration : UInt64) :
    ((PhysicalState.canonicalPair slot generation peer peerGeneration).first = slot ∧
      (PhysicalState.canonicalPair slot generation peer peerGeneration).second = peer) ∨
    ((PhysicalState.canonicalPair slot generation peer peerGeneration).first = peer ∧
      (PhysicalState.canonicalPair slot generation peer peerGeneration).second = slot) := by
  unfold PhysicalState.canonicalPair
  by_cases h : slot < peer <;> simp [h]

/-- A canonical pair of two different indices has two different endpoints. -/
private theorem PhysicalState.canonicalPair_ends_nodup {slot peer : Nat}
    {generation peerGeneration : UInt64} (hne : slot ≠ peer) :
    ([PhysicalState.canonicalPair slot generation peer peerGeneration].flatMap
      fun queued => [queued.first, queued.second]).Nodup := by
  have hends := PhysicalState.canonicalPair_ends slot generation peer peerGeneration
  simp
  rcases hends with ⟨h1, h2⟩ | ⟨h1, h2⟩ <;> omega

/-- Two canonical pairs over pairwise different slot and peer indices have
pairwise different endpoints. -/
private theorem PhysicalState.canonicalPairs_ends_nodup {slot peer slot' peer' : Nat}
    {generation peerGeneration generation' peerGeneration' : UInt64}
    (h1 : slot ≠ peer) (h2 : slot' ≠ peer') (h3 : slot ≠ slot') (h4 : slot ≠ peer')
    (h5 : peer ≠ slot') (h6 : peer ≠ peer') :
    ([PhysicalState.canonicalPair slot generation peer peerGeneration,
      PhysicalState.canonicalPair slot' generation' peer' peerGeneration'].flatMap
      fun queued => [queued.first, queued.second]).Nodup := by
  have hends := PhysicalState.canonicalPair_ends slot generation peer peerGeneration
  have hends' := PhysicalState.canonicalPair_ends slot' generation' peer' peerGeneration'
  simp
  rcases hends with ⟨e1, e2⟩ | ⟨e1, e2⟩ <;> rcases hends' with ⟨e3, e4⟩ | ⟨e3, e4⟩ <;> omega

/-- Two canonical pairs whose slots differ from each other and from the other
pair's peer are different pairs. -/
private theorem PhysicalState.canonicalPairs_nodup {slot peer slot' peer' : Nat}
    {generation peerGeneration generation' peerGeneration' : UInt64}
    (h3 : slot ≠ slot') (h4 : slot ≠ peer') :
    [PhysicalState.canonicalPair slot generation peer peerGeneration,
      PhysicalState.canonicalPair slot' generation' peer' peerGeneration'].Nodup := by
  have hends := PhysicalState.canonicalPair_ends slot generation peer peerGeneration
  have hends' := PhysicalState.canonicalPair_ends slot' generation' peer' peerGeneration'
  refine List.nodup_cons.mpr ⟨fun hmem => ?_, by simp⟩
  have heq := List.mem_singleton.mp hmem
  have hfirst := congrArg PhysicalPair.first heq
  have hsecond := congrArg PhysicalPair.second heq
  rcases hends with ⟨e1, e2⟩ | ⟨e1, e2⟩ <;> rcases hends' with ⟨e3, e4⟩ | ⟨e3, e4⟩ <;> omega

/-- **Emitted DUP acceptance**, for either orientation and storage policy: an
actually lowered DUP image of a legal, current, matched arena that is not
excluded by queue capacity executes and passes the independent semantic
checker at its exact reservations. -/
theorem PhysicalState.dup_accepted
    {state : PhysicalState} {pair : PhysicalPair} {redex : XRedex NetSym}
    (hRule : NetI64.xRule redex.2.2.1 redex.2.2.2.1 = some redex.2.2.2.2)
    {reuse : Bool} {plan : BytePlan}
    {ruleSlot valueSlot : Nat} {ruleId valueId : Id} {value : BitVec 64}
    {ruleOld valueOld : PhysicalSlot} {rhsSlot0 rhsSlot1 : Option Bool × Nat}
    {boundaryOne boundaryTwo : (Nat × Nat) × (Nat × Nat × Nat)} {p1 p2 : Port}
    {firstPublished secondPublished : List (Nat × Nat × Nat × Nat)}
    (hLegal : state.Legal)
    (hm : state.project.toNet.Matching NetI64.arity)
    (hcurrent : state.logicalPair pair = some (redex.1, redex.2.1))
    (horient : (pair.first = ruleSlot ∧ pair.second = valueSlot) ∨
      (pair.first = valueSlot ∧ pair.second = ruleSlot))
    (hids : (redex.1 = ruleId ∧ redex.2.1 = valueId) ∨
      (redex.1 = valueId ∧ redex.2.1 = ruleId))
    (hrule : state.slots[ruleSlot]? = some ruleOld)
    (hruleAgent : ruleOld.agent = some (ruleId, .dup))
    (hrulePort : ruleOld.ports[0]? = some (some (valueSlot, 0)))
    (hruleRoom : ruleOld.generation ≠ UInt64.ofNat (UInt64.size - 1))
    (hvalue : state.slots[valueSlot]? = some valueOld)
    (hvalueAgent : valueOld.agent = some (valueId, .num value))
    (hvaluePort : valueOld.ports[0]? = some (some (ruleSlot, 0)))
    (hvalueRoom : valueOld.generation ≠ UInt64.ofNat (UInt64.size - 1))
    (hdistinct : ruleSlot ≠ valueSlot)
    (hrhsSlot0 : redex.2.2.2.2.slots[0]? = some rhsSlot0)
    (hrhsLab0 : redex.2.2.2.2.lab rhsSlot0.1 rhsSlot0.2 = some (.num value))
    (hchild0 : child redex.1 redex.2.1 rhsSlot0.1 rhsSlot0.2 = 0 :: ruleId)
    (hrhsSlot1 : redex.2.2.2.2.slots[1]? = some rhsSlot1)
    (hrhsLab1 : redex.2.2.2.2.lab rhsSlot1.1 rhsSlot1.2 = some (.num value))
    (hchild1 : child redex.1 redex.2.1 rhsSlot1.1 rhsSlot1.2 = 2 :: ruleId)
    (hsource1 : boundaryOne.1 = (ruleSlot, 1))
    (hcapture1 : (∃ peer peerPort peerSlot peerId peerSymbol,
          boundaryOne.2 = (0, peer, peerPort) ∧ peer ≠ ruleSlot ∧ peer ≠ valueSlot ∧
          state.slots[peer]? = some peerSlot ∧
          peerSlot.agent = some (peerId, peerSymbol) ∧
          peerSlot.ports[peerPort]? = some (some (ruleSlot, 1)) ∧
          p1 = (peerId, peerPort)) ∨
       (∃ output out, boundaryOne.2 = (1, output, 0) ∧
          state.outputs[output]? = some out ∧
          out.target = (ruleSlot, 1) ∧ p1 = out.label))
    (hsource2 : boundaryTwo.1 = (ruleSlot, 2))
    (hcapture2 : (∃ peer peerPort peerSlot peerId peerSymbol,
          boundaryTwo.2 = (0, peer, peerPort) ∧ peer ≠ ruleSlot ∧ peer ≠ valueSlot ∧
          state.slots[peer]? = some peerSlot ∧
          peerSlot.agent = some (peerId, peerSymbol) ∧
          peerSlot.ports[peerPort]? = some (some (ruleSlot, 2)) ∧
          p2 = (peerId, peerPort)) ∨
       (∃ output out, boundaryTwo.2 = (1, output, 0) ∧
          state.outputs[output]? = some out ∧
          out.target = (ruleSlot, 2) ∧ p2 = out.label))
    (hLower : state.lowerPlan pair redex reuse = some plan)
    (hresults : plan.results =
      [(!reuse, if reuse then ruleSlot else state.slots.size, .num value, ruleSlot,
          [some 0, none, none]),
        (!reuse, if reuse then valueSlot else state.slots.size + 1, .num value, valueSlot,
          [some 1, none, none])])
    (hboundaries : plan.boundaries = [boundaryOne, boundaryTwo])
    (hwrites : plan.externalWrites = [(0, 0, 0), (1, 0, 1)])
    (hpub1 : state.publicationFor pair [boundaryOne, boundaryTwo]
      (!reuse, if reuse then ruleSlot else state.slots.size, .num value, ruleSlot,
        [some 0, none, none]) = some firstPublished)
    (hpub2 : state.publicationFor pair [boundaryOne, boundaryTwo]
      (!reuse, if reuse then valueSlot else state.slots.size + 1, .num value, valueSlot,
        [some 1, none, none]) = some secondPublished)
    (hpublications : plan.publications = firstPublished ++ secondPublished)
    (haction : plan.action = (if reuse then 1 else 0, if reuse then 1 else 0))
    (hpairPlan : plan.pair = (pair.first, pair.firstGeneration.toNat,
      pair.second, pair.secondGeneration.toNat))
    (hselAgents : (Plan.compile state.project redex hRule).initializedAgents =
      [(0 :: ruleId, .num value), (2 :: ruleId, .num value)])
    (hselWrites : (Plan.compile state.project redex hRule).wireWrites =
      [((0 :: ruleId, 0), p1), ((2 :: ruleId, 0), p2),
        (p1, (0 :: ruleId, 0)), (p2, (2 :: ruleId, 0))])
    (hselErased : ∀ port,
      (port ∈ (Plan.compile state.project redex hRule).capturedBoundary.map Prod.fst ∨
        port ∈ (Plan.compile state.project redex hRule).capturedBoundary.map Prod.snd) ↔
      (port = (ruleId, 0) ∨ port = (valueId, 0) ∨ port = (ruleId, 1) ∨
        port = (ruleId, 2) ∨ port = p1 ∨ port = p2))
    (hselBoundary : (Plan.compile state.project redex hRule).capturedBoundary.filter
      (fun edge => edge.1.2 != 0) = [((ruleId, 1), p1), ((ruleId, 2), p2)])
    (hcapacity : state.queueCapacityExcluded plan = false) :
    ∃ next, BytePlan.semanticCheck state plan.emit redex hRule plan.arenaReservation 256
      (state.slots.size + plan.arenaReservation - state.work.length) = some next := by
  have hdecode := PhysicalState.lowerPlan_decodes hLower
  have hc := PhysicalState.lowerPlan_has_candidate hLower
  obtain ⟨_, _, _, _, _, harena, _, hqueue⟩ := PhysicalState.planCandidate_fields hc
  have hruleBound : ruleSlot < state.slots.size := (getElem?_eq_some_iff.mp hrule).1
  have hvalueBound : valueSlot < state.slots.size := (getElem?_eq_some_iff.mp hvalue).1
  have hruleLive : ruleOld.agent.isSome = true := by simp [hruleAgent]
  have hvalueLive : valueOld.agent.isSome = true := by simp [hvalueAgent]
  -- the resolved result ports
  have hkind1 : (∃ peer port, boundaryOne.2 = (0, peer, port)) ∨
      (∃ output, boundaryOne.2 = (1, output, 0)) := by
    rcases hcapture1 with ⟨peer, port, _, _, _, hk, _⟩ | ⟨output, _, hk, _⟩
    · exact .inl ⟨peer, port, hk⟩
    · exact .inr ⟨output, hk⟩
  have hkind2 : (∃ peer port, boundaryTwo.2 = (0, peer, port)) ∨
      (∃ output, boundaryTwo.2 = (1, output, 0)) := by
    rcases hcapture2 with ⟨peer, port, _, _, _, hk, _⟩ | ⟨output, _, hk, _⟩
    · exact .inl ⟨peer, port, hk⟩
    · exact .inr ⟨output, hk⟩
  have htargetNone : plan.resultTarget none = some none := rfl
  have hports0 : ([some 0, none, none] : List (Option Nat)).mapM plan.resultTarget =
      some [PhysicalState.boundaryReference boundaryOne, none, none] := by
    have h0 := BytePlan.resultTarget_boundary (index := 0)
      (by rw [hboundaries]; rfl) hkind1
    simp [List.mapM_cons, h0, htargetNone]
  have hports1 : ([some 1, none, none] : List (Option Nat)).mapM plan.resultTarget =
      some [PhysicalState.boundaryReference boundaryTwo, none, none] := by
    have h1 := BytePlan.resultTarget_boundary (index := 1)
      (by rw [hboundaries]; rfl) hkind2
    simp [List.mapM_cons, h1, htargetNone]
  -- installation of both results
  obtain ⟨installed, hinstall, hpost⟩ := PhysicalState.dup_install_results
    (plan := plan) (left := redex.1) (right := redex.2.1) (rhs := redex.2.2.2.2)
    (reuse := reuse) hrule hruleLive hruleRoom hvalue hvalueLive hvalueRoom hdistinct
    hrhsSlot0 hrhsLab0 hrhsSlot1 hrhsLab1 hports0 hports1
  rw [← hresults] at hinstall
  rw [hchild0, hchild1] at hpost
  -- the decoded pair is the selected current pair
  have hpairEq : PhysicalPair.mk plan.pair.1 (UInt64.ofNat plan.pair.2.1)
      plan.pair.2.2.1 (UInt64.ofNat plan.pair.2.2.2) = pair := by
    rw [hpairPlan]
    cases pair
    simp [UInt64.ofNat_toNat]
  have hcurrentPlan := hpairEq ▸ hcurrent
  have hfirstPlan : plan.pair.1 = pair.first := by rw [hpairPlan]
  have hsecondPlan : plan.pair.2.2.1 = pair.second := by rw [hpairPlan]
  have hpeerAway : ∀ peer, peer ≠ ruleSlot → peer ≠ valueSlot →
      peer ≠ pair.first ∧ peer ≠ pair.second := by
    intro peer h1 h2
    rcases horient with ⟨hf, hs⟩ | ⟨hf, hs⟩ <;> rw [hf, hs] <;>
      exact ⟨by assumption, by assumption⟩
  have hpairSet : ∀ index, (index = pair.second ∨ index = pair.first) ↔
      (index = ruleSlot ∨ index = valueSlot) := by
    intro index
    rcases horient with ⟨hf, hs⟩ | ⟨hf, hs⟩ <;> rw [hf, hs]
    exact Or.comm
  have hpairNe : pair.first ≠ pair.second := by
    rcases horient with ⟨hf, hs⟩ | ⟨hf, hs⟩ <;> rw [hf, hs]
    · exact hdistinct
    · exact Ne.symm hdistinct
  -- the two installed results and their captured edges
  let slot0 : Nat := if reuse then ruleSlot else state.slots.size
  let slot1 : Nat := if reuse then valueSlot else state.slots.size + 1
  let view0 : ResultView :=
    { slot := slot0, id := 0 :: ruleId, symbol := .num value,
      generation := if reuse then ruleOld.generation + 1 else 0,
      ports := [PhysicalState.boundaryReference boundaryOne, none, none] }
  let view1 : ResultView :=
    { slot := slot1, id := 2 :: ruleId, symbol := .num value,
      generation := if reuse then valueOld.generation + 1 else 0,
      ports := [PhysicalState.boundaryReference boundaryTwo, none, none] }
  let edgeOne : EdgeView :=
    { sourcePort := 1, target := boundaryOne.2, endpoint := p1,
      result := (slot0, 0), resultId := 0 :: ruleId }
  let edgeTwo : EdgeView :=
    { sourcePort := 2, target := boundaryTwo.2, endpoint := p2,
      result := (slot1, 0), resultId := 2 :: ruleId }
  have hslotsNe : slot0 ≠ slot1 := by
    cases reuse <;> simp [slot0, slot1, hdistinct]
  have hpeerNot0 : ∀ peer, peer < state.slots.size → peer ≠ ruleSlot → peer ≠ valueSlot →
      peer ≠ slot0 := by
    intro peer hlt h1 _
    cases reuse <;> simp [slot0] <;> omega
  have hpeerNot1 : ∀ peer, peer < state.slots.size → peer ≠ ruleSlot → peer ≠ valueSlot →
      peer ≠ slot1 := by
    intro peer hlt _ h2
    cases reuse <;> simp [slot1] <;> omega
  have hrefPeer : ∀ (boundary : (Nat × Nat) × (Nat × Nat × Nat)) (peer peerPort : Nat),
      boundary.2 = (0, peer, peerPort) →
      PhysicalState.boundaryReference boundary = some (peer, peerPort) := by
    intro boundary peer peerPort hk
    simp [PhysicalState.boundaryReference, hk]
  -- admissible reciprocal writes
  have hresult0 : plan.results[0]? = some (!reuse,
      if reuse then ruleSlot else state.slots.size, .num value, ruleSlot,
      [some 0, none, none]) := by
    rw [hresults]; rfl
  have hresult1 : plan.results[1]? = some (!reuse,
      if reuse then valueSlot else state.slots.size + 1, .num value, valueSlot,
      [some 1, none, none]) := by
    rw [hresults]; rfl
  have hready1 : state.WriteReady plan (0, 0, 0) := by
    refine ⟨_, boundaryOne, hresult0, by rw [hboundaries]; rfl, rfl, ?_⟩
    rcases hcapture1 with ⟨peer, peerPort, peerSlot, peerId, peerSymbol, hk, h1, h2, hpeer,
        hpeerAgent, hpeerPort, _⟩ | ⟨output, out, hk, houtput, htarget, _⟩
    · obtain ⟨hf, hs⟩ := hpeerAway peer h1 h2
      exact .inl ⟨peer, peerPort, peerSlot, hk, hfirstPlan ▸ hf, hsecondPlan ▸ hs, hpeer,
        by simp [hpeerAgent], by rw [hsource1]; exact hpeerPort⟩
    · exact .inr ⟨output, out, hk, houtput, by rw [hsource1]; exact htarget⟩
  have hready2 : state.WriteReady plan (1, 0, 1) := by
    refine ⟨_, boundaryTwo, hresult1, by rw [hboundaries]; rfl, rfl, ?_⟩
    rcases hcapture2 with ⟨peer, peerPort, peerSlot, peerId, peerSymbol, hk, h1, h2, hpeer,
        hpeerAgent, hpeerPort, _⟩ | ⟨output, out, hk, houtput, htarget, _⟩
    · obtain ⟨hf, hs⟩ := hpeerAway peer h1 h2
      exact .inl ⟨peer, peerPort, peerSlot, hk, hfirstPlan ▸ hf, hsecondPlan ▸ hs, hpeer,
        by simp [hpeerAgent], by rw [hsource2]; exact hpeerPort⟩
    · exact .inr ⟨output, out, hk, houtput, by rw [hsource2]; exact htarget⟩
  have htarget1 : plan.writeTarget (0, 0, 0) = some boundaryOne.2 := by
    simp [BytePlan.writeTarget, hboundaries]
  have htarget2 : plan.writeTarget (1, 0, 1) = some boundaryTwo.2 := by
    simp [BytePlan.writeTarget, hboundaries]
  have hsource1' : plan.writeSource (0, 0, 0) = some edgeOne.result := by
    simp [BytePlan.writeSource, hresult0, edgeOne, slot0]
  have hsource2' : plan.writeSource (1, 0, 1) = some edgeTwo.result := by
    simp [BytePlan.writeSource, hresult1, edgeTwo, slot1]
  have hdistinctTargets : boundaryOne.2 ≠ boundaryTwo.2 :=
    PhysicalState.captured_targets_distinct (by decide) hcapture1 hcapture2
  -- publications: each result publishes at most its own canonical pair
  have hshape0 := PhysicalState.dup_publication_shape (state := state) (pair := pair)
    (boundaries := [boundaryOne, boundaryTwo]) (index := 0) (ruleSlot := ruleSlot)
    (valueSlot := valueSlot) (sourcePort := 1) (donor := ruleSlot)
    (fresh := state.slots.size) (endpoint := p1) (boundary := boundaryOne) (reuse := reuse)
    (symbol := .num value) (ports := [some 0, none, none]) (old := ruleOld)
    (published := firstPublished) (view := { plan with publications := firstPublished })
    rfl rfl rfl hcapture1 horient (.inl rfl) (Nat.le_refl _) hrule hruleRoom hpub1
  have hshape1 := PhysicalState.dup_publication_shape (state := state) (pair := pair)
    (boundaries := [boundaryOne, boundaryTwo]) (index := 1) (ruleSlot := ruleSlot)
    (valueSlot := valueSlot) (sourcePort := 2) (donor := valueSlot)
    (fresh := state.slots.size + 1) (endpoint := p2) (boundary := boundaryTwo)
    (reuse := reuse) (symbol := .num value) (ports := [some 1, none, none])
    (old := valueOld) (published := secondPublished)
    (view := { plan with publications := secondPublished })
    rfl rfl rfl hcapture2 horient (.inr rfl) (Nat.le_succ _) hvalue hvalueRoom hpub2
  have hsplit : plan.publishedPairs =
      BytePlan.publishedPairs { plan with publications := firstPublished } ++
        BytePlan.publishedPairs { plan with publications := secondPublished } := by
    simp only [BytePlan.publishedPairs, hpublications, List.map_append]
  generalize BytePlan.publishedPairs { plan with publications := firstPublished } = P0
    at hshape0 hsplit
  generalize BytePlan.publishedPairs { plan with publications := secondPublished } = P1
    at hshape1 hsplit
  have hpubExact : ∀ queued, queued ∈ plan.publishedPairs ↔ ∃ result ∈ [view0, view1],
      ∃ edge ∈ [edgeOne, edgeTwo], ∃ peer peerSlot, edge.result = (result.slot, 0) ∧
        edge.target = (0, peer, 0) ∧ state.slots[peer]? = some peerSlot ∧
        queued = PhysicalState.canonicalPair result.slot result.generation
          peer peerSlot.generation := by
    intro queued
    rw [hsplit, List.mem_append, PhysicalState.dup_published_mem hshape0 queued,
      PhysicalState.dup_published_mem hshape1 queued]
    constructor
    · rintro (⟨peer, peerSlot, hk, hpeer, hq⟩ | ⟨peer, peerSlot, hk, hpeer, hq⟩)
      · exact ⟨view0, by simp, edgeOne, by simp, peer, peerSlot, rfl, hk, hpeer, hq⟩
      · exact ⟨view1, by simp, edgeTwo, by simp, peer, peerSlot, rfl, hk, hpeer, hq⟩
    · rintro ⟨result, hr, edge, he, peer, peerSlot, hresult, htarget, hpeer, hq⟩
      simp only [List.mem_cons, List.not_mem_nil, or_false] at hr he
      rcases hr with rfl | rfl <;> rcases he with rfl | rfl
      · exact .inl ⟨peer, peerSlot, htarget, hpeer, hq⟩
      · exact absurd (Prod.mk.inj hresult).1 (Ne.symm hslotsNe)
      · exact absurd (Prod.mk.inj hresult).1 hslotsNe
      · exact .inr ⟨peer, peerSlot, htarget, hpeer, hq⟩
  have hpubDisjoint :
      (plan.publishedPairs.flatMap fun queued => [queued.first, queued.second]).Nodup := by
    rw [hsplit]
    rcases hshape0 with ⟨hnone0, _⟩ | ⟨peer0, peerSlot0, hk0, hpeer0, h01, h02, hone0⟩
    · rcases hshape1 with ⟨hnone1, _⟩ | ⟨peer1, peerSlot1, _, hpeer1, h11, h12, hone1⟩
      · rw [hnone0, hnone1]
        exact List.nodup_nil
      · rw [hnone0, hone1]
        exact PhysicalState.canonicalPair_ends_nodup
          (Ne.symm (hpeerNot1 peer1 (getElem?_eq_some_iff.mp hpeer1).1 h11 h12))
    · rcases hshape1 with ⟨hnone1, _⟩ | ⟨peer1, peerSlot1, hk1, hpeer1, h11, h12, hone1⟩
      · rw [hone0, hnone1]
        exact PhysicalState.canonicalPair_ends_nodup
          (Ne.symm (hpeerNot0 peer0 (getElem?_eq_some_iff.mp hpeer0).1 h01 h02))
      · rw [hone0, hone1]
        exact PhysicalState.canonicalPairs_ends_nodup
          (Ne.symm (hpeerNot0 peer0 (getElem?_eq_some_iff.mp hpeer0).1 h01 h02))
          (Ne.symm (hpeerNot1 peer1 (getElem?_eq_some_iff.mp hpeer1).1 h11 h12))
          hslotsNe
          (Ne.symm (hpeerNot0 peer1 (getElem?_eq_some_iff.mp hpeer1).1 h11 h12))
          (hpeerNot1 peer0 (getElem?_eq_some_iff.mp hpeer0).1 h01 h02)
          (fun h => hdistinctTargets (by rw [hk0, hk1, h]))
  have hpubNodup : plan.publishedPairs.Nodup := by
    rw [hsplit]
    rcases hshape0 with ⟨hnone0, _⟩ | ⟨peer0, peerSlot0, _, hpeer0, h01, h02, hone0⟩
    · rcases hshape1 with ⟨hnone1, _⟩ | ⟨peer1, peerSlot1, _, _, _, _, hone1⟩
      · rw [hnone0, hnone1]
        exact List.nodup_nil
      · rw [hnone0, hone1]
        simp
    · rcases hshape1 with ⟨hnone1, _⟩ | ⟨peer1, peerSlot1, _, hpeer1, h11, h12, hone1⟩
      · rw [hone0, hnone1]
        simp
      · rw [hone0, hone1]
        exact PhysicalState.canonicalPairs_nodup hslotsNe
          (Ne.symm (hpeerNot0 peer1 (getElem?_eq_some_iff.mp hpeer1).1 h11 h12))
  -- queue room for every publication
  have hcapacityPub : plan.publishedPairs = [] ∨
      state.work.length + plan.publishedPairs.length ≤
        state.slots.size + (if reuse then 0 else 2) := by
    have hlen : plan.publishedPairs.length = plan.worklistReservation := by
      rw [hqueue]
      simp [BytePlan.publishedPairs]
    have harena' : plan.arenaReservation = if reuse then 0 else 2 := by
      rw [harena, hresults]
      cases reuse <;> simp
    simp only [PhysicalState.queueCapacityExcluded, decide_eq_false_iff_not] at hcapacity
    rw [harena', ← hlen] at hcapacity
    generalize (if reuse then 0 else 2) = fresh at hcapacity ⊢
    by_cases hzero : plan.publishedPairs.length = 0
    · exact .inl (List.eq_nil_of_length_eq_zero hzero)
    · exact .inr (by omega)
  obtain ⟨next, hexec, hobs⟩ := PhysicalState.execute_observations
    (redex := redex) (ruleSlot := ruleSlot) (valueSlot := valueSlot)
    (results := [view0, view1]) (edges := [edgeOne, edgeTwo])
    (fresh := if reuse then 0 else 2)
    hdecode hcurrentPlan (by rw [hfirstPlan, hsecondPlan]; exact horient) hdistinct
    hinstall hpost
    (by
      intro result hr
      simp only [List.mem_cons, List.not_mem_nil, or_false] at hr
      rcases hr with rfl | rfl
      · cases reuse <;> simp [view0, slot0]
      · cases reuse <;> simp [view1, slot1])
    (by
      rw [hwrites]
      intro write hw
      simp only [List.mem_cons, List.not_mem_nil, or_false] at hw
      rcases hw with rfl | rfl
      · exact hready1
      · exact hready2)
    (by
      rw [hwrites]
      simp [htarget1, htarget2, hdistinctTargets])
    (by
      intro edge he
      simp only [List.mem_cons, List.not_mem_nil, or_false] at he
      rcases he with rfl | rfl
      · exact ⟨(0, 0, 0), by rw [hwrites]; simp, htarget1, hsource1'⟩
      · exact ⟨(1, 0, 1), by rw [hwrites]; simp, htarget2, hsource2'⟩)
    (by
      rw [hwrites]
      intro write hw
      simp only [List.mem_cons, List.not_mem_nil, or_false] at hw
      rcases hw with rfl | rfl
      · exact ⟨edgeOne, by simp, htarget1, hsource1'⟩
      · exact ⟨edgeTwo, by simp, htarget2, hsource2'⟩)
    (by
      rw [haction, hfirstPlan]
      cases reuse <;> simp [view0, view1, slot0, slot1] <;>
        rcases horient with ⟨hf, _⟩ | ⟨hf, _⟩ <;> simp [hf] <;> omega)
    (by
      rw [haction, hsecondPlan]
      cases reuse <;> simp [view0, view1, slot0, slot1] <;>
        rcases horient with ⟨_, hs⟩ | ⟨_, hs⟩ <;> simp [hs] <;> omega)
    (by
      rw [hfirstPlan]
      rcases horient with ⟨hf, _⟩ | ⟨hf, _⟩ <;> rw [hf]
      · exact ⟨ruleOld, hrule, hruleLive, hruleRoom⟩
      · exact ⟨valueOld, hvalue, hvalueLive, hvalueRoom⟩)
    (by
      rw [hsecondPlan]
      rcases horient with ⟨_, hs⟩ | ⟨_, hs⟩ <;> rw [hs]
      · exact ⟨valueOld, hvalue, hvalueLive, hvalueRoom⟩
      · exact ⟨ruleOld, hrule, hruleLive, hruleRoom⟩)
    (by
      intro result hr edge he port peer peerPort hresult htarget
      simp only [List.mem_cons, List.not_mem_nil, or_false] at hr he
      rcases hr with rfl | rfl <;> rcases he with rfl | rfl
      · obtain rfl : port = 0 := (Prod.mk.inj hresult).2.symm
        simp [view0, hrefPeer boundaryOne peer peerPort htarget]
      · exact absurd (Prod.mk.inj hresult).1 (Ne.symm hslotsNe)
      · exact absurd (Prod.mk.inj hresult).1 hslotsNe
      · obtain rfl : port = 0 := (Prod.mk.inj hresult).2.symm
        simp [view1, hrefPeer boundaryTwo peer peerPort htarget])
    (fun queued hq => (hpubExact queued).mp hq)
    hpubDisjoint hcapacityPub hLegal.2.2.2.2.1
  -- the abstract boundary agrees with the physical capture
  have hBoundary : plan.logicalBoundary state = some
      ((Plan.compile state.project redex hRule).capturedBoundary.filter
        fun edge => edge.1.2 != 0) := by
    rw [hselBoundary]
    have h1 := PhysicalState.logicalBoundary_entry hrule hruleAgent hsource1 hcapture1
    have h2 := PhysicalState.logicalBoundary_entry hrule hruleAgent hsource2 hcapture2
    rw [BytePlan.logicalBoundary_eq_mapM, hboundaries]
    simp [List.mapM_cons, h1, h2]
  -- slots outside the plan's touched set are outside every rewritten location
  have hTouched : ∀ index, index < state.slots.size →
      plan.touchedSlots.contains index = false →
      index ≠ ruleSlot ∧ index ≠ valueSlot ∧
      (∀ edge ∈ [edgeOne, edgeTwo], ∀ port, edge.target ≠ (0, index, port)) ∧
      (∀ queued ∈ plan.publishedPairs, queued.first ≠ index ∧ queued.second ≠ index) := by
    intro index _ hnot
    have hmem : index ∉ plan.touchedSlots := by
      intro h
      have := List.contains_iff_mem.mpr h
      rw [hnot] at this
      exact Bool.false_ne_true this
    simp only [BytePlan.touchedSlots, hboundaries, List.mem_append, List.mem_cons,
      List.not_mem_nil, or_false, not_or, List.mem_filterMap, List.mem_flatMap,
      not_exists, not_and] at hmem
    obtain ⟨⟨⟨hfirst, hsecond⟩, hpeers⟩, hpubs⟩ := hmem
    rw [hfirstPlan] at hfirst
    rw [hsecondPlan] at hsecond
    refine ⟨?_, ?_, ?_, ?_⟩
    · rcases horient with ⟨hf, hs⟩ | ⟨hf, hs⟩ <;> omega
    · rcases horient with ⟨hf, hs⟩ | ⟨hf, hs⟩ <;> omega
    · intro edge he port htarget
      simp at he
      rcases he with rfl | rfl
      · exact hpeers boundaryOne (.inl rfl) (by
          have htarget' : boundaryOne.2 = (0, index, port) := htarget
          rw [htarget']; simp)
      · exact hpeers boundaryTwo (.inr rfl) (by
          have htarget' : boundaryTwo.2 = (0, index, port) := htarget
          rw [htarget']; simp)
    · intro queued hq
      simp only [BytePlan.publishedPairs, List.mem_map] at hq
      obtain ⟨⟨f, fg, s, sg⟩, htuple, rfl⟩ := hq
      have hnotIn := hpubs (f, fg, s, sg) htuple
      exact ⟨fun h => hnotIn.1 h.symm, fun h => hnotIn.2 h.symm⟩
  -- the local-rewrite bundle
  have hRewrite : state.LocalRewrite next (Plan.compile state.project redex hRule)
      ruleSlot valueSlot ruleId valueId .dup (.num value)
      [view0, view1] [edgeOne, edgeTwo] plan.retiredSlots (if reuse then 0 else 2)
      plan.publishedPairs := by
    obtain ⟨hfreshId0, hfreshLabel0⟩ :=
      PhysicalState.extension_unused hLegal hm hrule hruleAgent 0
    obtain ⟨hfreshId2, hfreshLabel2⟩ :=
      PhysicalState.extension_unused hLegal hm hrule hruleAgent 2
    have hretired : plan.retiredSlots =
        if reuse then [] else [pair.second, pair.first] := by
      simp only [BytePlan.retiredSlots, haction, hfirstPlan, hsecondPlan]
      cases reuse <;> simp
    refine
      { legal := hLegal
        ruleOld := ⟨ruleOld, hrule, hruleAgent, hrulePort⟩
        valueOld := ⟨valueOld, hvalue, hvalueAgent, hvaluePort⟩
        valueArity := rfl
        distinctSlots := hdistinct
        outputsInArity := fun output hmem =>
          PhysicalState.output_target_in_arity hLegal hm hmem
        edgePorts := rfl
        edgeTargets := by simp [edgeOne, edgeTwo, hdistinctTargets]
        edgeCapture := ?_
        resultSlots := by simp [view0, view1, hslotsNe]
        resultIds := by simp [view0, view1]
        resultFresh := ?_
        resultPlace := ?_
        resultGeneration := ?_
        freshFilled := ?_
        resultArity := ?_
        resultPorts := ?_
        edgeResult := ?_
        edgeResults := by simp [edgeOne, edgeTwo, hslotsNe]
        retiredExact := ?_
        retiredNodup := ?_
        observed := hobs
        publishedExact := hpubExact
        publishedNodup := hpubNodup
        selectedPair := hids
        selectedAgents := by rw [hselAgents]; rfl
        selectedWrites := by rw [hselWrites]; rfl
        selectedErased := ?_
        selectedActive := rfl }
    · intro edge he
      simp only [List.mem_cons, List.not_mem_nil, or_false] at he
      rcases he with rfl | rfl
      · exact hcapture1
      · exact hcapture2
    · intro result hr
      simp only [List.mem_cons, List.not_mem_nil, or_false] at hr
      rcases hr with rfl | rfl
      · exact ⟨hfreshId0, hfreshLabel0⟩
      · exact ⟨hfreshId2, hfreshLabel2⟩
    · intro result hr
      simp only [List.mem_cons, List.not_mem_nil, or_false] at hr
      rcases hr with rfl | rfl
      · cases reuse <;> simp [view0, slot0]
      · cases reuse <;> simp [view1, slot1]
    · intro result hr old hold
      simp only [List.mem_cons, List.not_mem_nil, or_false] at hr
      rcases hr with rfl | rfl
      · cases reuse with
        | true =>
          simp only [view0, slot0, ↓reduceIte] at hold ⊢
          rw [hrule] at hold
          cases hold
          exact ⟨hruleRoom, rfl⟩
        | false =>
          simp only [view0, slot0, Bool.false_eq_true, ↓reduceIte] at hold
          have hlt := (getElem?_eq_some_iff.mp hold).1
          omega
      · cases reuse with
        | true =>
          simp only [view1, slot1, ↓reduceIte] at hold ⊢
          rw [hvalue] at hold
          cases hold
          exact ⟨hvalueRoom, rfl⟩
        | false =>
          simp only [view1, slot1, Bool.false_eq_true, ↓reduceIte] at hold
          have hlt := (getElem?_eq_some_iff.mp hold).1
          omega
    · intro index hlow hhigh
      cases reuse with
      | true => simp at hhigh; omega
      | false =>
        simp at hhigh
        by_cases h : index = state.slots.size
        · exact ⟨view0, by simp, by simp [view0, slot0, h]⟩
        · exact ⟨view1, by simp, by simp [view1, slot1]; omega⟩
    · intro result hr
      simp only [List.mem_cons, List.not_mem_nil, or_false] at hr
      rcases hr with rfl | rfl
      · simp [view0, NetI64.arity]
      · simp [view1, NetI64.arity]
    · intro result hr port target hport
      simp only [List.mem_cons, List.not_mem_nil, or_false] at hr
      rcases hr with rfl | rfl
      · match port, hport with
        | 0, hport =>
          left
          refine ⟨edgeOne, by simp, rfl, rfl, by simp [view0, NetI64.arity], ?_⟩
          simp [view0] at hport
          subst hport
          rcases hcapture1 with ⟨peer, peerPort, _, _, _, hk, _⟩ | ⟨output, _, hk, _⟩
          · exact .inl ⟨peer, peerPort, hk, by simp [PhysicalState.boundaryReference, hk]⟩
          · exact .inr ⟨output, hk, by simp [PhysicalState.boundaryReference, hk]⟩
        | 1, hport =>
          right
          simp [view0] at hport
          exact ⟨by simp [edgeOne, edgeTwo], by simp [view0, NetI64.arity], hport.symm⟩
        | 2, hport =>
          right
          simp [view0] at hport
          exact ⟨by simp [edgeOne, edgeTwo], by simp [view0, NetI64.arity], hport.symm⟩
        | _ + 3, hport =>
          simp [view0] at hport
      · match port, hport with
        | 0, hport =>
          left
          refine ⟨edgeTwo, by simp, rfl, rfl, by simp [view1, NetI64.arity], ?_⟩
          simp [view1] at hport
          subst hport
          rcases hcapture2 with ⟨peer, peerPort, _, _, _, hk, _⟩ | ⟨output, _, hk, _⟩
          · exact .inl ⟨peer, peerPort, hk, by simp [PhysicalState.boundaryReference, hk]⟩
          · exact .inr ⟨output, hk, by simp [PhysicalState.boundaryReference, hk]⟩
        | 1, hport =>
          right
          simp [view1] at hport
          exact ⟨by simp [edgeOne, edgeTwo], by simp [view1, NetI64.arity], hport.symm⟩
        | 2, hport =>
          right
          simp [view1] at hport
          exact ⟨by simp [edgeOne, edgeTwo], by simp [view1, NetI64.arity], hport.symm⟩
        | _ + 3, hport =>
          simp [view1] at hport
    · intro edge he
      simp only [List.mem_cons, List.not_mem_nil, or_false] at he
      rcases he with rfl | rfl
      · exact ⟨view0, by simp, rfl, rfl, by simp [view0, edgeOne]⟩
      · exact ⟨view1, by simp, rfl, rfl, by simp [view1, edgeTwo]⟩
    · intro index
      rw [hretired]
      cases reuse with
      | true =>
        simp only [↓reduceIte, List.not_mem_nil, false_iff, not_and]
        intro hindex hall
        rcases hindex with h | h
        · exact hall view0 (by simp) (by simp [view0, slot0, h])
        · exact hall view1 (by simp) (by simp [view1, slot1, h])
      | false =>
        simp only [Bool.false_eq_true, ↓reduceIte, List.mem_cons, List.not_mem_nil,
          or_false]
        rw [hpairSet]
        constructor
        · intro h
          refine ⟨h, ?_⟩
          rintro result (rfl | rfl)
          · simp only [view0, slot0, Bool.false_eq_true, ↓reduceIte]
            omega
          · simp only [view1, slot1, Bool.false_eq_true, ↓reduceIte]
            omega
        · exact fun h => h.1
    · rw [hretired]
      cases reuse with
      | true => exact List.nodup_nil
      | false => simp [Ne.symm hpairNe]
    · intro port
      rw [hselErased]
      simp only [edgeOne, edgeTwo, List.mem_cons, List.not_mem_nil, or_false,
        exists_eq_or_imp, exists_eq_left]
      constructor
      · rintro (h | h | h | h | h | h)
        · exact .inl h
        · exact .inr (.inl h)
        · exact .inr (.inr (.inl (.inl h)))
        · exact .inr (.inr (.inr (.inl h)))
        · exact .inr (.inr (.inl (.inr h)))
        · exact .inr (.inr (.inr (.inr h)))
      · rintro (h | h | (h | h) | (h | h))
        · exact .inl h
        · exact .inr (.inl h)
        · exact .inr (.inr (.inl h))
        · exact .inr (.inr (.inr (.inr (.inl h))))
        · exact .inr (.inr (.inr (.inl h)))
        · exact .inr (.inr (.inr (.inr (.inr h))))
  exact ⟨next, PhysicalState.semanticCheck_of_local_rewrite hRule hLower hBoundary hexec
    hRewrite hTouched hcapacity⟩

/-- **Forward DUP emitted acceptance.** -/
theorem PhysicalState.dup_forward_accepted
    {state : PhysicalState} {pair : PhysicalPair} {a b : Id}
    {value : BitVec 64} {reuse : Bool} {plan : BytePlan}
    (hLegal : state.Legal)
    (hm : state.project.toNet.Matching NetI64.arity)
    (hcurrent : state.logicalPair pair = some (a, b))
    (hredex : Redex NetI64.system state.project.toNet a b .dup (.num value)
      (dupNumData value).toX.toRhs)
    (hLower : state.lowerPlan pair (a, b, .dup, .num value,
      (dupNumData value).toX) reuse = some plan)
    (hcapacity : state.queueCapacityExcluded plan = false) :
    ∃ next, BytePlan.semanticCheck state plan.emit
      (a, b, .dup, .num value, (dupNumData value).toX) (by rfl)
      plan.arenaReservation 256
      (state.slots.size + plan.arenaReservation - state.work.length) = some next := by
  obtain ⟨first, second, hf, hs, hfa, hsa, _, _, hfp, hsp, hdistinct, hw0, hwb, _, _⟩ :=
    PhysicalState.redex_physical_pair hLegal hcurrent hm hredex
  obtain ⟨hroomF, hroomS⟩ := PhysicalState.lowerPlan_slot_headroom hLower hf hs
  have halias := PhysicalState.lowerPlan_no_alias hLower
  simp [PhysicalState.consumedBoundaryAlias, hf, NetI64.arity, List.range_succ] at halias
  have hnoAlias1 : ∀ peer peerPort, first.ports[1]? = some (some (peer, peerPort)) →
      peer ≠ pair.first ∧ peer ≠ pair.second := by
    intro peer peerPort hcontent
    have h := halias.1
    simp [hcontent] at h
    exact h
  have hnoAlias2 : ∀ peer peerPort, first.ports[2]? = some (some (peer, peerPort)) →
      peer ≠ pair.first ∧ peer ≠ pair.second := by
    intro peer peerPort hcontent
    have h := halias.2
    simp [hcontent] at h
    exact h
  obtain ⟨boundaryOne, p1, hcap1, hsrc1, hcapture1, hw1, _, _⟩ :=
    PhysicalState.capture_edge (valueSlot := pair.second) hLegal hf hfa
      (port := 1) (by simp [NetI64.arity]) hnoAlias1
  obtain ⟨boundaryTwo, p2, hcap2, hsrc2, hcapture2, hw2, _, _⟩ :=
    PhysicalState.capture_edge (valueSlot := pair.second) hLegal hf hfa
      (port := 2) (by simp [NetI64.arity]) hnoAlias2
  obtain ⟨published1, hpub1⟩ := PhysicalState.publicationFor_defined_outside (pair := pair)
    (boundaries := [boundaryOne, boundaryTwo])
    (result := (!reuse, if reuse then pair.first else state.slots.size, .num value,
      pair.first, [some 0, none, none]))
    (index := 0) (ruleSlot := pair.first) (valueSlot := pair.second)
    rfl rfl hcapture1 (.inl ⟨rfl, rfl⟩)
    (by
      intro peer hpeer h1 _
      have hlt : peer < state.slots.size := by
        cases h : state.slots[peer]? with
        | none => exact absurd h hpeer
        | some _ => exact (getElem?_eq_some_iff.mp h).1
      cases reuse <;> simp <;> omega)
    (by
      intro hr
      cases reuse with
      | true => exact ⟨first, by simpa using hf, hroomF⟩
      | false => simp at hr)
  obtain ⟨published2, hpub2⟩ := PhysicalState.publicationFor_defined_outside (pair := pair)
    (boundaries := [boundaryOne, boundaryTwo])
    (result := (!reuse, if reuse then pair.second else state.slots.size + 1, .num value,
      pair.second, [some 1, none, none]))
    (index := 1) (ruleSlot := pair.first) (valueSlot := pair.second)
    rfl rfl hcapture2 (.inl ⟨rfl, rfl⟩)
    (by
      intro peer hpeer _ h2
      have hlt : peer < state.slots.size := by
        cases h : state.slots[peer]? with
        | none => exact absurd h hpeer
        | some _ => exact (getElem?_eq_some_iff.mp h).1
      cases reuse <;> simp <;> omega)
    (by
      intro hr
      cases reuse with
      | true => exact ⟨second, by simpa using hs, hroomS⟩
      | false => simp at hr)
  have hc := PhysicalState.lowerPlan_has_candidate hLower
  obtain ⟨hresults, hboundaries, hwrites, hpublications, haction, hpairPlan⟩ :=
    PhysicalState.dup_forward_candidate_shape hcurrent hf hs (by simp [hfa])
      (by simp [hsa]) hfp hsp hcap1 hcap2 hpub1 hpub2 hc
  have haway1 := PhysicalState.captured_endpoint_away hLegal hf hfa hs hsa hcapture1
  have haway2 := PhysicalState.captured_endpoint_away hLegal hf hfa hs hsa hcapture2
  obtain ⟨hselBoundary, hselAgents, hselWrites⟩ :=
    Plan.dup_forward_fields state.project (value := value) hw0 hwb hw1 hw2 haway1 haway2
  refine PhysicalState.dup_accepted (by rfl) hLegal hm hcurrent (.inl ⟨rfl, rfl⟩)
    (.inl ⟨rfl, rfl⟩) hf hfa hfp hroomF hs hsa hsp hroomS hdistinct
    (rhsSlot0 := (some false, 0)) rfl
    (by simp [dupNumData, RuleData.toX, RuleData.ofWires, assoc]) rfl
    (rhsSlot1 := (some false, 1)) rfl
    (by simp [dupNumData, RuleData.toX, RuleData.ofWires, assoc]) rfl
    hsrc1 hcapture1 hsrc2 hcapture2 hLower hresults hboundaries hwrites hpub1 hpub2
    hpublications haction hpairPlan hselAgents hselWrites ?_ ?_ hcapacity
  · intro port
    rw [hselBoundary]
    simp only [List.map_cons, List.map_nil, List.mem_cons, List.not_mem_nil, or_false]
    constructor
    · rintro ((h | h | h | h) | (h | h | h | h))
      · exact .inl h
      · exact .inr (.inr (.inl h))
      · exact .inr (.inr (.inr (.inl h)))
      · exact .inr (.inl h)
      · exact .inr (.inl h)
      · exact .inr (.inr (.inr (.inr (.inl h))))
      · exact .inr (.inr (.inr (.inr (.inr h))))
      · exact .inl h
    · rintro (h | h | h | h | h | h)
      · exact .inl (.inl h)
      · exact .inl (.inr (.inr (.inr h)))
      · exact .inl (.inr (.inl h))
      · exact .inl (.inr (.inr (.inl h)))
      · exact .inr (.inr (.inl h))
      · exact .inr (.inr (.inr (.inl h)))
  · rw [hselBoundary]
    simp [List.filter_cons]

/-- **Reverse DUP emitted acceptance.** -/
theorem PhysicalState.dup_reverse_accepted
    {state : PhysicalState} {pair : PhysicalPair} {a b : Id}
    {value : BitVec 64} {reuse : Bool} {plan : BytePlan}
    (hLegal : state.Legal)
    (hm : state.project.toNet.Matching NetI64.arity)
    (hcurrent : state.logicalPair pair = some (a, b))
    (hredex : Redex NetI64.system state.project.toNet a b (.num value) .dup
      (dupNumData value).toX.swap.toRhs)
    (hLower : state.lowerPlan pair (a, b, .num value, .dup,
      (dupNumData value).toX.swap) reuse = some plan)
    (hcapacity : state.queueCapacityExcluded plan = false) :
    ∃ next, BytePlan.semanticCheck state plan.emit
      (a, b, .num value, .dup, (dupNumData value).toX.swap) (by rfl)
      plan.arenaReservation 256
      (state.slots.size + plan.arenaReservation - state.work.length) = some next := by
  obtain ⟨first, second, hf, hs, hfa, hsa, _, _, hfp, hsp, hdistinct, hw0, hwb, _, _⟩ :=
    PhysicalState.redex_physical_pair hLegal hcurrent hm hredex
  obtain ⟨hroomF, hroomS⟩ := PhysicalState.lowerPlan_slot_headroom hLower hf hs
  have halias := PhysicalState.lowerPlan_no_alias hLower
  simp [PhysicalState.consumedBoundaryAlias, hs, NetI64.arity, List.range_succ] at halias
  have hnoAlias1 : ∀ peer peerPort, second.ports[1]? = some (some (peer, peerPort)) →
      peer ≠ pair.second ∧ peer ≠ pair.first := by
    intro peer peerPort hcontent
    have h := halias.1
    simp [hcontent] at h
    exact ⟨h.2, h.1⟩
  have hnoAlias2 : ∀ peer peerPort, second.ports[2]? = some (some (peer, peerPort)) →
      peer ≠ pair.second ∧ peer ≠ pair.first := by
    intro peer peerPort hcontent
    have h := halias.2
    simp [hcontent] at h
    exact ⟨h.2, h.1⟩
  obtain ⟨boundaryOne, p1, hcap1, hsrc1, hcapture1, hw1, _, _⟩ :=
    PhysicalState.capture_edge (valueSlot := pair.first) hLegal hs hsa
      (port := 1) (by simp [NetI64.arity]) hnoAlias1
  obtain ⟨boundaryTwo, p2, hcap2, hsrc2, hcapture2, hw2, _, _⟩ :=
    PhysicalState.capture_edge (valueSlot := pair.first) hLegal hs hsa
      (port := 2) (by simp [NetI64.arity]) hnoAlias2
  obtain ⟨published1, hpub1⟩ := PhysicalState.publicationFor_defined_outside (pair := pair)
    (boundaries := [boundaryOne, boundaryTwo])
    (result := (!reuse, if reuse then pair.second else state.slots.size, .num value,
      pair.second, [some 0, none, none]))
    (index := 0) (ruleSlot := pair.second) (valueSlot := pair.first)
    rfl rfl hcapture1 (.inr ⟨rfl, rfl⟩)
    (by
      intro peer hpeer h1 _
      have hlt : peer < state.slots.size := by
        cases h : state.slots[peer]? with
        | none => exact absurd h hpeer
        | some _ => exact (getElem?_eq_some_iff.mp h).1
      cases reuse <;> simp <;> omega)
    (by
      intro hr
      cases reuse with
      | true => exact ⟨second, by simpa using hs, hroomS⟩
      | false => simp at hr)
  obtain ⟨published2, hpub2⟩ := PhysicalState.publicationFor_defined_outside (pair := pair)
    (boundaries := [boundaryOne, boundaryTwo])
    (result := (!reuse, if reuse then pair.first else state.slots.size + 1, .num value,
      pair.first, [some 1, none, none]))
    (index := 1) (ruleSlot := pair.second) (valueSlot := pair.first)
    rfl rfl hcapture2 (.inr ⟨rfl, rfl⟩)
    (by
      intro peer hpeer _ h2
      have hlt : peer < state.slots.size := by
        cases h : state.slots[peer]? with
        | none => exact absurd h hpeer
        | some _ => exact (getElem?_eq_some_iff.mp h).1
      cases reuse <;> simp <;> omega)
    (by
      intro hr
      cases reuse with
      | true => exact ⟨first, by simpa using hf, hroomF⟩
      | false => simp at hr)
  have hc := PhysicalState.lowerPlan_has_candidate hLower
  obtain ⟨hresults, hboundaries, hwrites, hpublications, haction, hpairPlan⟩ :=
    PhysicalState.dup_reverse_candidate_shape hcurrent hf hs (by simp [hfa])
      (by simp [hsa]) hfp hsp hcap1 hcap2 hpub1 hpub2 hc
  have haway1 := PhysicalState.captured_endpoint_away hLegal hs hsa hf hfa hcapture1
  have haway2 := PhysicalState.captured_endpoint_away hLegal hs hsa hf hfa hcapture2
  obtain ⟨hselBoundary, hselAgents, hselWrites⟩ :=
    Plan.dup_reverse_fields state.project (value := value) hw0 hwb hw1 hw2
      ⟨haway1.2, haway1.1⟩ ⟨haway2.2, haway2.1⟩
  refine PhysicalState.dup_accepted (by rfl) hLegal hm hcurrent (.inr ⟨rfl, rfl⟩)
    (.inr ⟨rfl, rfl⟩) hs hsa hsp hroomS hf hfa hfp hroomF (Ne.symm hdistinct)
    (rhsSlot0 := (some true, 0)) rfl
    (by simp [dupNumData, RuleData.toX, RuleData.ofWires, XRhs.swap, assoc]) rfl
    (rhsSlot1 := (some true, 1)) rfl
    (by simp [dupNumData, RuleData.toX, RuleData.ofWires, XRhs.swap, assoc]) rfl
    hsrc1 hcapture1 hsrc2 hcapture2 hLower hresults hboundaries hwrites hpub1 hpub2
    hpublications haction hpairPlan hselAgents hselWrites ?_ ?_ hcapacity
  · intro port
    rw [hselBoundary]
    simp only [List.map_cons, List.map_nil, List.mem_cons, List.not_mem_nil, or_false]
    constructor
    · rintro ((h | h | h | h) | (h | h | h | h))
      · exact .inr (.inl h)
      · exact .inl h
      · exact .inr (.inr (.inl h))
      · exact .inr (.inr (.inr (.inl h)))
      · exact .inl h
      · exact .inr (.inl h)
      · exact .inr (.inr (.inr (.inr (.inl h))))
      · exact .inr (.inr (.inr (.inr (.inr h))))
    · rintro (h | h | h | h | h | h)
      · exact .inl (.inr (.inl h))
      · exact .inl (.inl h)
      · exact .inl (.inr (.inr (.inl h)))
      · exact .inl (.inr (.inr (.inr h)))
      · exact .inr (.inr (.inr (.inl h)))
      · exact .inr (.inr (.inr (.inr h)))
  · rw [hselBoundary]
    simp [List.filter_cons]

end NobleFoundation.NetI64Plans
