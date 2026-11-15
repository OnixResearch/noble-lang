import NobleFoundation.NetI64PlanExecute

/-!
# Composing the checked executor's phases (issue #110)

`PhysicalState.executeCheckedImage` runs result installation, the reciprocal
write list, two retirements and the publication list. Given the installation
post-state and admissible, pairwise distinct writes, retire flags matching the
consumed slots that no result reuses, publishable canonical pairs and enough
queue room, the whole image executes and its post-state has exactly the
`RewriteObservations` of the corresponding result and edge views. This is a
statement about the Lean model executor only.
-/

namespace NobleFoundation.NetI64Plans

open INet
open NobleFoundation.NetI64

/-- The decoded queue tuples as generation-tagged physical pairs. -/
def BytePlan.publishedPairs (plan : BytePlan) : List PhysicalPair :=
  plan.publications.map fun (first, firstGeneration, second, secondGeneration) =>
    { first, firstGeneration := UInt64.ofNat firstGeneration,
      second, secondGeneration := UInt64.ofNat secondGeneration : PhysicalPair }

/-- The executor is exactly its five phases once the image decodes and its
selected pair is current. -/
theorem PhysicalState.executeCheckedImage_phases
    {state : PhysicalState} {bytes : List UInt8} {redex : XRedex NetSym}
    {plan : BytePlan}
    {installed rewired retiredFirst retiredSecond next : PhysicalState}
    (hdecode : BytePlan.decode bytes = some plan)
    (hcurrent : state.logicalPair
      { first := plan.pair.1, firstGeneration := UInt64.ofNat plan.pair.2.1,
        second := plan.pair.2.2.1, secondGeneration := UInt64.ofNat plan.pair.2.2.2 } =
      some (redex.1, redex.2.1))
    (hinstall : PhysicalState.installResults plan redex.1 redex.2.1 redex.2.2.2.2 0
      state plan.results = some installed)
    (hwrite : installed.writeExternals plan plan.externalWrites = some rewired)
    (hfirst : rewired.retire plan.pair.1 (plan.action.1 == 1) = some retiredFirst)
    (hsecond : retiredFirst.retire plan.pair.2.2.1 (plan.action.2 == 1) = some retiredSecond)
    (hpublish : retiredSecond.publishAll plan.publishedPairs = some next) :
    state.executeCheckedImage bytes redex = some next := by
  simp only [PhysicalState.executeCheckedImage, hdecode, Option.bind_eq_bind,
    Option.bind_some, hcurrent, bne_self_eq_false, Bool.false_eq_true, ↓reduceIte,
    hinstall, hwrite, hfirst, hsecond]
  exact hpublish

/-- The consumed slots the two retire flags actually retire, in the order
`retire` pushes them onto the free list. -/
def BytePlan.retiredSlots (plan : BytePlan) : List Nat :=
  (if plan.action.2 == 1 then [] else [plan.pair.2.2.1]) ++
    (if plan.action.1 == 1 then [] else [plan.pair.1])

/-- One retire step with either flag: a set flag leaves the arena unchanged;
a clear flag retires a live slot with headroom and changes nothing else. -/
theorem PhysicalState.retire_flag
    {state : PhysicalState} {index : Nat} (flag : Bool)
    (hlive : flag = false → ∃ old, state.slots[index]? = some old ∧
      old.agent.isSome = true ∧ old.generation ≠ UInt64.ofNat (UInt64.size - 1)) :
    ∃ next, state.retire index flag = some next ∧
      next.slots.size = state.slots.size ∧ next.outputs = state.outputs ∧
      next.work = state.work ∧
      next.freeSlots = (if flag then [] else [index]) ++ state.freeSlots ∧
      (∀ other, other ≠ index ∨ flag = true → next.slots[other]? = state.slots[other]?) ∧
      (flag = false → ∃ old, state.slots[index]? = some old ∧
        next.slots[index]? = some (PhysicalState.retiredSlot old) ∧
        old.generation ≠ UInt64.ofNat (UInt64.size - 1)) ∧
      (state.queuedGenerationsBound = true → next.queuedGenerationsBound = true) := by
  cases flag with
  | true =>
    refine ⟨state, PhysicalState.retire_reused state index, rfl, rfl, rfl, by simp,
      fun _ _ => rfl, fun h => by simp at h, fun h => h⟩
  | false =>
    obtain ⟨old, hold, holdLive, hroom⟩ := hlive rfl
    have hstep := PhysicalState.retire_eq hold holdLive hroom
    refine ⟨_, hstep, by simp [PhysicalState.writeSlot], rfl, rfl, by simp, ?_, ?_, ?_⟩
    · intro other hother
      have hne : index ≠ other := by
        rcases hother with h | h
        · exact Ne.symm h
        · simp at h
      exact PhysicalState.writeSlot_frame hne
    · intro _
      exact ⟨old, hold, PhysicalState.writeSlot_self hold, hroom⟩
    · intro hbound
      exact PhysicalState.retire_preserves_queued_generations_bound hbound hstep

/-- A write admissible in the prestate stays admissible after installation
when its peer is an old slot that no result occupies. -/
theorem PhysicalState.writeReady_installed
    {state installed : PhysicalState} {plan : BytePlan}
    {results : List ResultView} {fresh : Nat} {write : Nat × Nat × Nat}
    (hpost : state.InstallPost installed results fresh)
    (hready : state.WriteReady plan write)
    (hpeers : ∀ peer port, plan.writeTarget write = some (0, peer, port) →
      ∀ result ∈ results, result.slot ≠ peer) :
    installed.WriteReady plan write := by
  obtain ⟨result, boundary, hresult, hboundary, hport, hkind⟩ := hready
  refine ⟨result, boundary, hresult, hboundary, hport, ?_⟩
  rcases hkind with ⟨peer, port, old, hk, hleft, hright, hpeer, hlive, htarget⟩ |
    ⟨output, old, hk, houtput, htarget⟩
  · have hbound : peer < state.slots.size := (getElem?_eq_some_iff.mp hpeer).1
    have hnot := hpeers peer port (by simp [BytePlan.writeTarget, hboundary, hk])
    exact .inl ⟨peer, port, old, hk, hleft, hright,
      (hpost.frame peer hbound hnot).trans hpeer, hlive, htarget⟩
  · exact .inr ⟨output, old, hk, by rw [hpost.outputs]; exact houtput, htarget⟩

/-- Every successful publication list preserves the queued-generation bound. -/
theorem PhysicalState.publishAll_preserves_queued_generations_bound
    {state next : PhysicalState} {pairs : List PhysicalPair}
    (hbound : state.queuedGenerationsBound = true)
    (h : state.publishAll pairs = some next) :
    next.queuedGenerationsBound = true := by
  induction pairs generalizing state next with
  | nil =>
    simp [PhysicalState.publishAll] at h
    cases h
    exact hbound
  | cons pair rest ih =>
    simp only [PhysicalState.publishAll] at h
    obtain ⟨middle, hmiddle, hrest⟩ := Option.bind_eq_some_iff.mp h
    exact ih (PhysicalState.publish_preserves_queued_generations_bound hbound hmiddle) hrest

/-- **Generic emitted-image execution.** Installation into reserved result
slots, admissible pairwise-distinct reciprocal writes to captured endpoints,
retire flags matching the consumed slots no result reuses, publishable
canonical pairs with disjoint endpoints, and queue room for all of them make
the whole decoded image execute; the post-state has exactly the observations
of the given result and edge views. -/
theorem PhysicalState.execute_observations
    {state installed : PhysicalState} {plan : BytePlan} {redex : XRedex NetSym}
    {ruleSlot valueSlot : Nat} {results : List ResultView} {edges : List EdgeView}
    {fresh : Nat}
    (hdecode : BytePlan.decode plan.emit = some plan)
    (hcurrent : state.logicalPair
      { first := plan.pair.1, firstGeneration := UInt64.ofNat plan.pair.2.1,
        second := plan.pair.2.2.1, secondGeneration := UInt64.ofNat plan.pair.2.2.2 } =
      some (redex.1, redex.2.1))
    (hpairSlots : (plan.pair.1 = ruleSlot ∧ plan.pair.2.2.1 = valueSlot) ∨
      (plan.pair.1 = valueSlot ∧ plan.pair.2.2.1 = ruleSlot))
    (hdistinct : ruleSlot ≠ valueSlot)
    (hinstall : PhysicalState.installResults plan redex.1 redex.2.1 redex.2.2.2.2 0
      state plan.results = some installed)
    (hpost : state.InstallPost installed results fresh)
    (hplace : ∀ result ∈ results, (result.slot = ruleSlot ∨ result.slot = valueSlot) ∨
      state.slots.size ≤ result.slot)
    (hready : ∀ write ∈ plan.externalWrites, state.WriteReady plan write)
    (hnodup : (plan.externalWrites.map plan.writeTarget).Nodup)
    (hedgeWrites : ∀ edge ∈ edges, ∃ write ∈ plan.externalWrites,
      plan.writeTarget write = some edge.target ∧ plan.writeSource write = some edge.result)
    (hwriteEdges : ∀ write ∈ plan.externalWrites, ∃ edge ∈ edges,
      plan.writeTarget write = some edge.target ∧ plan.writeSource write = some edge.result)
    (hflagFirst : (plan.action.1 == 1) = true ↔
      ∃ result ∈ results, result.slot = plan.pair.1)
    (hflagSecond : (plan.action.2 == 1) = true ↔
      ∃ result ∈ results, result.slot = plan.pair.2.2.1)
    (hfirstLive : ∃ old, state.slots[plan.pair.1]? = some old ∧ old.agent.isSome = true ∧
      old.generation ≠ UInt64.ofNat (UInt64.size - 1))
    (hsecondLive : ∃ old, state.slots[plan.pair.2.2.1]? = some old ∧
      old.agent.isSome = true ∧ old.generation ≠ UInt64.ofNat (UInt64.size - 1))
    (hresultPeerPorts : ∀ result ∈ results, ∀ edge ∈ edges, ∀ port peer peerPort,
      edge.result = (result.slot, port) → edge.target = (0, peer, peerPort) →
      result.ports[port]? = some (some (peer, peerPort)))
    (hpubs : ∀ pair ∈ plan.publishedPairs, ∃ result ∈ results, ∃ edge ∈ edges,
      ∃ peer peerSlot, edge.result = (result.slot, 0) ∧ edge.target = (0, peer, 0) ∧
      state.slots[peer]? = some peerSlot ∧
      pair = PhysicalState.canonicalPair result.slot result.generation
        peer peerSlot.generation)
    (hpubDisjoint : (plan.publishedPairs.flatMap fun pair => [pair.first, pair.second]).Nodup)
    (hcapacity : state.work.length + plan.publishedPairs.length ≤ state.slots.size + fresh)
    (hbound : state.queuedGenerationsBound = true) :
    ∃ next, state.executeCheckedImage plan.emit redex = some next ∧
      state.RewriteObservations next ruleSlot valueSlot results edges
        plan.retiredSlots fresh plan.publishedPairs := by
  have hfirstConsumed : plan.pair.1 = ruleSlot ∨ plan.pair.1 = valueSlot := by
    rcases hpairSlots with ⟨h1, _⟩ | ⟨h1, _⟩
    · exact .inl h1
    · exact .inr h1
  have hsecondConsumed : plan.pair.2.2.1 = ruleSlot ∨ plan.pair.2.2.1 = valueSlot := by
    rcases hpairSlots with ⟨_, h2⟩ | ⟨_, h2⟩
    · exact .inr h2
    · exact .inl h2
  have hpairDistinct : plan.pair.1 ≠ plan.pair.2.2.1 := by
    rcases hpairSlots with ⟨h1, h2⟩ | ⟨h1, h2⟩
    · rw [h1, h2]; exact hdistinct
    · rw [h1, h2]; exact Ne.symm hdistinct
  -- peers of reciprocal writes are old slots outside the consumed pair
  have hpeerFacts : ∀ write ∈ plan.externalWrites, ∀ peer port,
      plan.writeTarget write = some (0, peer, port) →
      peer < state.slots.size ∧ peer ≠ plan.pair.1 ∧ peer ≠ plan.pair.2.2.1 ∧
      ∃ old, state.slots[peer]? = some old ∧ old.agent.isSome = true := by
    intro write hw peer port htarget
    obtain ⟨result, boundary, _, hboundary, _, hkind⟩ := hready write hw
    have hk : boundary.2 = (0, peer, port) := by
      simpa [BytePlan.writeTarget, hboundary] using htarget
    rcases hkind with ⟨peer', port', old, hk', hleft, hright, hpeer, hlive, _⟩ |
      ⟨output, old, hk', _, _⟩
    · rw [hk] at hk'
      simp only [Prod.mk.injEq, true_and] at hk'
      obtain ⟨rfl, rfl⟩ := hk'
      exact ⟨(getElem?_eq_some_iff.mp hpeer).1, hleft, hright, old, hpeer, hlive⟩
    · rw [hk] at hk'
      simp at hk'
  have hpeerNotResult : ∀ write ∈ plan.externalWrites, ∀ peer port,
      plan.writeTarget write = some (0, peer, port) →
      ∀ result ∈ results, result.slot ≠ peer := by
    intro write hw peer port htarget result hr he
    obtain ⟨hbound', hleft, hright, _⟩ := hpeerFacts write hw peer port htarget
    rcases hplace result hr with (h | h) | h
    · rcases hfirstConsumed with h1 | h1 <;> rcases hsecondConsumed with h2 | h2
      all_goals omega
    · rcases hfirstConsumed with h1 | h1 <;> rcases hsecondConsumed with h2 | h2
      all_goals omega
    · omega
  -- phase 2: reciprocal writes
  have hreadyInstalled : ∀ write ∈ plan.externalWrites, installed.WriteReady plan write :=
    fun write hw => PhysicalState.writeReady_installed hpost (hready write hw)
      (hpeerNotResult write hw)
  obtain ⟨rewired, hrewired, hwrites⟩ :=
    PhysicalState.writeExternals_success plan.externalWrites installed hreadyInstalled hnodup
  -- slots no write targets are unchanged by the write phase
  have hnotTarget : ∀ index, (index = plan.pair.1 ∨ index = plan.pair.2.2.1 ∨
      state.slots.size ≤ index) → ∀ write ∈ plan.externalWrites, ∀ port,
      plan.writeTarget write ≠ some (0, index, port) := by
    intro index hindex write hw port htarget
    obtain ⟨hbound', hleft, hright, _⟩ := hpeerFacts write hw index port htarget
    rcases hindex with h | h | h <;> omega
  -- phase 3: the two retirements
  have hfirstFlag : (plan.action.1 == 1) = false → ∃ old,
      rewired.slots[plan.pair.1]? = some old ∧ old.agent.isSome = true ∧
        old.generation ≠ UInt64.ofNat (UInt64.size - 1) := by
    intro hflag
    obtain ⟨old, hold, hlive, hroom⟩ := hfirstLive
    have hnone : ∀ result ∈ results, result.slot ≠ plan.pair.1 := by
      intro result hr he
      have := hflagFirst.mpr ⟨result, hr, he⟩
      simp [hflag] at this
    refine ⟨old, ?_, hlive, hroom⟩
    rw [hwrites.slotFrame _ (hnotTarget _ (.inl rfl)),
      hpost.frame _ (getElem?_eq_some_iff.mp hold).1 hnone]
    exact hold
  obtain ⟨retiredFirst, hretireFirst, hsize₁, houts₁, hwork₁, hfree₁, hframe₁,
    hretired₁, hbound₁⟩ := PhysicalState.retire_flag (state := rewired)
      (index := plan.pair.1) (plan.action.1 == 1) hfirstFlag
  have hsecondFlag : (plan.action.2 == 1) = false → ∃ old,
      retiredFirst.slots[plan.pair.2.2.1]? = some old ∧ old.agent.isSome = true ∧
        old.generation ≠ UInt64.ofNat (UInt64.size - 1) := by
    intro hflag
    obtain ⟨old, hold, hlive, hroom⟩ := hsecondLive
    have hnone : ∀ result ∈ results, result.slot ≠ plan.pair.2.2.1 := by
      intro result hr he
      have := hflagSecond.mpr ⟨result, hr, he⟩
      simp [hflag] at this
    refine ⟨old, ?_, hlive, hroom⟩
    rw [hframe₁ _ (.inl (Ne.symm hpairDistinct)),
      hwrites.slotFrame _ (hnotTarget _ (.inr (.inl rfl))),
      hpost.frame _ (getElem?_eq_some_iff.mp hold).1 hnone]
    exact hold
  obtain ⟨retiredSecond, hretireSecond, hsize₂, houts₂, hwork₂, hfree₂, hframe₂,
    hretired₂, hbound₂⟩ := PhysicalState.retire_flag (state := retiredFirst)
      (index := plan.pair.2.2.1) (plan.action.2 == 1) hsecondFlag
  -- the result slots survive writes and retirements unchanged
  have hresultKept : ∀ result ∈ results, retiredSecond.slots[result.slot]? =
      some (PhysicalState.resultSlot result.id result.symbol result.generation
        result.ports) := by
    intro result hr
    have hnotWritten : ∀ write ∈ plan.externalWrites, ∀ port,
        plan.writeTarget write ≠ some (0, result.slot, port) := by
      intro write hw port htarget
      exact hpeerNotResult write hw result.slot port htarget result hr rfl
    have hkeep₁ : result.slot ≠ plan.pair.1 ∨ (plan.action.1 == 1) = true := by
      by_cases he : result.slot = plan.pair.1
      · exact .inr (hflagFirst.mpr ⟨result, hr, he⟩)
      · exact .inl he
    have hkeep₂ : result.slot ≠ plan.pair.2.2.1 ∨ (plan.action.2 == 1) = true := by
      by_cases he : result.slot = plan.pair.2.2.1
      · exact .inr (hflagSecond.mpr ⟨result, hr, he⟩)
      · exact .inl he
    rw [hframe₂ _ hkeep₂, hframe₁ _ hkeep₁, hwrites.slotFrame _ hnotWritten]
    exact hpost.result result hr
  -- phase 4: publication
  have hsizeRetired : retiredSecond.slots.size = state.slots.size + fresh := by
    rw [hsize₂, hsize₁, hwrites.size, hpost.size]
  have hworkRetired : retiredSecond.work = state.work := by
    rw [hwork₂, hwork₁, hwrites.work, hpost.work]
  have hpubReady : ∀ pair ∈ plan.publishedPairs, retiredSecond.PublishReady pair := by
    intro pair hp
    obtain ⟨result, hr, edge, he, peer, peerSlot, hresult, htarget, hpeer, rfl⟩ :=
      hpubs pair hp
    obtain ⟨write, hw, hwt, hws⟩ := hedgeWrites edge he
    obtain ⟨hpeerBound, hpeerFirst, hpeerSecond, old, hold, hlive⟩ :=
      hpeerFacts write hw peer 0 (by rw [hwt, htarget])
    have hsamePeer : peerSlot = old := Option.some.inj (hpeer.symm.trans hold)
    subst hsamePeer
    have hne : result.slot ≠ peer :=
      hpeerNotResult write hw peer 0 (by rw [hwt, htarget]) result hr
    obtain ⟨written, hwritten, hwrittenPort, hwrittenMark⟩ :=
      hwrites.written write hw peer 0 edge.result (by rw [hwt, htarget]) hws
    obtain ⟨peerRewired, hpeerRewired, hpeerAgent, hpeerGen, _, _, _⟩ :=
      hwrites.peerSlot peer peerSlot (by
        rw [hpost.frame peer hpeerBound (hpeerNotResult write hw peer 0
          (by rw [hwt, htarget]))]
        exact hpeer)
    have hsameWritten : written = peerRewired :=
      Option.some.inj (hwritten.symm.trans hpeerRewired)
    subst hsameWritten
    have hpeerAfter : retiredSecond.slots[peer]? = some written := by
      rw [hframe₂ _ (.inl hpeerSecond), hframe₁ _ (.inl hpeerFirst)]
      exact hwritten
    have hresultAfter := hresultKept result hr
    have hresultPort : result.ports[0]? = some (some (peer, 0)) :=
      hresultPeerPorts result hr edge he 0 peer 0 hresult htarget
    have hpeerPort : written.ports[0]? = some (some (result.slot, 0)) := by
      rw [hwrittenPort, hresult]
    have hpeerMark : written.queuedGeneration = none := hwrittenMark rfl
    obtain ⟨owner, howner'⟩ := Option.isSome_iff_exists.mp hlive
    have howner : written.agent = some owner := hpeerAgent.trans howner'
    unfold PhysicalState.canonicalPair
    by_cases hlt : result.slot < peer
    · simp only [hlt, ↓reduceIte]
      refine ⟨hlt, _, written, (result.id, owner.1), hresultAfter, hpeerAfter, ?_,
        by simpa [PhysicalState.resultSlot] using hresultPort, hpeerPort,
        by simp [PhysicalState.resultSlot], by simp [hpeerMark]⟩
      simp [PhysicalState.logicalPair, hresultAfter, hpeerAfter, PhysicalState.resultSlot,
        hpeerGen, howner]
    · have hgt : peer < result.slot := by omega
      simp only [hlt, ↓reduceIte]
      refine ⟨hgt, written, _, (owner.1, result.id), hpeerAfter, hresultAfter, ?_,
        hpeerPort, by simpa [PhysicalState.resultSlot] using hresultPort,
        by simp [hpeerMark], by simp [PhysicalState.resultSlot]⟩
      simp [PhysicalState.logicalPair, hresultAfter, hpeerAfter, PhysicalState.resultSlot,
        hpeerGen, howner]
  obtain ⟨next, hpublish, hworkNext, houtsNext, hfreeNext, hsizeNext, hkeepNext,
    hframeNext⟩ := PhysicalState.publishAll_success plan.publishedPairs retiredSecond
      hpubReady hpubDisjoint (by rw [hworkRetired, hsizeRetired]; exact hcapacity)
  refine ⟨next, PhysicalState.executeCheckedImage_phases hdecode hcurrent hinstall
    hrewired hretireFirst hretireSecond hpublish, ?_⟩
  -- helper: an old slot that is not consumed reaches `retiredSecond` through
  -- installation and both retirements unchanged except for its peer writes
  have hconsumedFirst : ∀ index, index ≠ ruleSlot → index ≠ valueSlot →
      index ≠ plan.pair.1 := by
    intro index h1 h2 he
    rcases hfirstConsumed with h | h <;> omega
  have hconsumedSecond : ∀ index, index ≠ ruleSlot → index ≠ valueSlot →
      index ≠ plan.pair.2.2.1 := by
    intro index h1 h2 he
    rcases hsecondConsumed with h | h <;> omega
  have hnotResult : ∀ index, index < state.slots.size → index ≠ ruleSlot →
      index ≠ valueSlot → ∀ result ∈ results, result.slot ≠ index := by
    intro index hbound' h1 h2 result hr he
    rcases hplace result hr with (h | h) | h <;> omega
  have hwriteFromEdges : ∀ index port, (∀ edge ∈ edges, edge.target ≠ (0, index, port)) →
      ∀ write ∈ plan.externalWrites, plan.writeTarget write ≠ some (0, index, port) := by
    intro index port hnone write hw htarget
    obtain ⟨edge, he, hwt, _⟩ := hwriteEdges write hw
    rw [hwt] at htarget
    exact hnone edge he (Option.some.inj htarget)
  have hbound₀ : installed.queuedGenerationsBound = true :=
    PhysicalState.installResults_preserves_queued_generations_bound hbound hinstall
  have hboundRewired : rewired.queuedGenerationsBound = true :=
    PhysicalState.writeExternals_preserves_queued_generations_bound hbound₀ hrewired
  have hboundRetired : retiredSecond.queuedGenerationsBound = true :=
    hbound₂ (hbound₁ hboundRewired)
  refine ⟨?_, ?_, ?_, ?_, ?_, ?_, ?_, ?_, ?_⟩
  · rw [hsizeNext, hsizeRetired]
  · intro result hr
    obtain ⟨slot, hslot, hagent, hgen, hports⟩ :=
      hkeepNext result.slot _ (hresultKept result hr)
    exact ⟨slot, hslot, by simpa [PhysicalState.resultSlot] using hagent,
      by simpa [PhysicalState.resultSlot] using hgen,
      by simpa [PhysicalState.resultSlot] using hports⟩
  · intro index hindex
    simp only [BytePlan.retiredSlots, List.mem_append] at hindex
    rcases hindex with hindex | hindex
    · cases hflag : (plan.action.2 == 1) with
      | true => simp [hflag] at hindex
      | false =>
        simp [hflag] at hindex
        subst hindex
        obtain ⟨old, hold, hlive, hroom⟩ := hsecondLive
        obtain ⟨old', hold', hretiredSlot, _⟩ := hretired₂ hflag
        have hnone : ∀ result ∈ results, result.slot ≠ plan.pair.2.2.1 := by
          intro result hr he
          have := hflagSecond.mpr ⟨result, hr, he⟩
          simp [hflag] at this
        have hsame : old' = old := by
          rw [hframe₁ _ (.inl (Ne.symm hpairDistinct)),
            hwrites.slotFrame _ (hnotTarget _ (.inr (.inl rfl))),
            hpost.frame _ (getElem?_eq_some_iff.mp hold).1 hnone, hold] at hold'
          exact (Option.some.inj hold').symm
        subst hsame
        obtain ⟨slot, hslot, hagent, hgen, _⟩ := hkeepNext _ _ hretiredSlot
        refine ⟨old', slot, hold, hslot, ?_, ?_, hroom⟩
        · simpa [PhysicalState.retiredSlot] using hagent
        · simpa [PhysicalState.retiredSlot] using hgen
    · cases hflag : (plan.action.1 == 1) with
      | true => simp [hflag] at hindex
      | false =>
        simp [hflag] at hindex
        subst hindex
        obtain ⟨old, hold, hlive, hroom⟩ := hfirstLive
        obtain ⟨old', hold', hretiredSlot, _⟩ := hretired₁ hflag
        have hnone : ∀ result ∈ results, result.slot ≠ plan.pair.1 := by
          intro result hr he
          have := hflagFirst.mpr ⟨result, hr, he⟩
          simp [hflag] at this
        have hsame : old' = old := by
          rw [hwrites.slotFrame _ (hnotTarget _ (.inl rfl)),
            hpost.frame _ (getElem?_eq_some_iff.mp hold).1 hnone, hold] at hold'
          exact (Option.some.inj hold').symm
        subst hsame
        have hsecondFrame : retiredSecond.slots[plan.pair.1]? =
            retiredFirst.slots[plan.pair.1]? :=
          hframe₂ _ (.inl hpairDistinct)
        obtain ⟨slot, hslot, hagent, hgen, _⟩ :=
          hkeepNext _ _ (hsecondFrame.trans hretiredSlot)
        refine ⟨old', slot, hold, hslot, ?_, ?_, hroom⟩
        · simpa [PhysicalState.retiredSlot] using hagent
        · simpa [PhysicalState.retiredSlot] using hgen
  · intro index old hold h1 h2
    have hbound' : index < state.slots.size := (getElem?_eq_some_iff.mp hold).1
    have hinstalled : installed.slots[index]? = some old :=
      (hpost.frame index hbound' (hnotResult index hbound' h1 h2)).trans hold
    obtain ⟨slot, hslot, hagent, hgen, hlen, _, hports⟩ :=
      hwrites.peerSlot index old hinstalled
    have hretiredSlot : retiredSecond.slots[index]? = some slot := by
      rw [hframe₂ _ (.inl (hconsumedSecond index h1 h2)),
        hframe₁ _ (.inl (hconsumedFirst index h1 h2))]
      exact hslot
    obtain ⟨final, hfinal, hagent', hgen', hports'⟩ := hkeepNext index slot hretiredSlot
    refine ⟨final, hfinal, hagent'.trans hagent, hgen'.trans hgen,
      by rw [hports']; exact hlen, ?_, ?_⟩
    · intro edge he port htarget
      obtain ⟨write, hw, hwt, hws⟩ := hedgeWrites edge he
      obtain ⟨written, hwritten, hwrittenPort, _⟩ :=
        hwrites.written write hw index port edge.result (by rw [hwt, htarget]) hws
      rw [hslot] at hwritten
      cases hwritten
      rw [hports']
      exact hwrittenPort
    · intro port hnone
      rw [hports']
      exact hports port (hwriteFromEdges index port hnone)
  · intro index hbound' h1 h2 hnoEdge hnoPub
    rw [hframeNext index hnoPub, hframe₂ _ (.inl (hconsumedSecond index h1 h2)),
      hframe₁ _ (.inl (hconsumedFirst index h1 h2)),
      hwrites.slotFrame index (fun write hw port =>
        hwriteFromEdges index port (fun edge he => hnoEdge edge he port) write hw),
      hpost.frame index hbound' (hnotResult index hbound' h1 h2)]
  · refine ⟨by rw [houtsNext, houts₂, houts₁, hwrites.outputLength, hpost.outputs], ?_⟩
    intro output old hold
    have hinstalled : installed.outputs[output]? = some old := by
      rw [hpost.outputs]
      exact hold
    obtain ⟨out, hout, hlabel, hkeep⟩ := hwrites.outputFrame output old hinstalled
    refine ⟨out, by rw [houtsNext, houts₂, houts₁]; exact hout, hlabel, ?_, ?_⟩
    · intro edge he htarget
      obtain ⟨write, hw, hwt, hws⟩ := hedgeWrites edge he
      obtain ⟨out', hout', htarget'⟩ :=
        hwrites.outputWritten write hw output edge.result (by rw [hwt, htarget]) hws
      rw [hout] at hout'
      cases hout'
      exact htarget'
    · intro hnone
      have hunwritten : ∀ write ∈ plan.externalWrites,
          plan.writeTarget write ≠ some (1, output, 0) := by
        intro write hw htarget
        obtain ⟨edge, he, hwt, _⟩ := hwriteEdges write hw
        rw [hwt] at htarget
        exact hnone edge he (Option.some.inj htarget)
      rw [hkeep hunwritten]
  · rw [hworkNext, hworkRetired]
  · rw [hfreeNext, hfree₂, hfree₁, hwrites.free, hpost.free]
    simp [BytePlan.retiredSlots]
  · exact PhysicalState.publishAll_preserves_queued_generations_bound hboundRetired hpublish

end NobleFoundation.NetI64Plans
