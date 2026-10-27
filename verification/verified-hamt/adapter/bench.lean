import VerifiedHAMT.Map

/-!
# Bounded cost comparison for the VerifiedHAMT adapter (issue #60)

Build-inclusive and repeated-lookup costs of the adapter against the existing list representation, at
declared sizes and lookup distributions. The results are accumulated into a checked sum that is
printed, so the compiler cannot delete the timed work; numbers are for this host only.
-/

namespace NobleCostBench

structure Entry where
  slot : Nat
  body : Nat

def entries (size : Nat) : List Entry :=
  (List.range size).map (fun index => ⟨index * 7 + 1, index⟩)

def referenceLookup (entries : List Entry) (slot : Nat) : Option Entry :=
  entries.find? (fun entry => entry.slot == slot)

/-- The index, built once outside the repeated-lookup loop so that cost is not charged twice. -/
def buildIndex (entries : List Entry) : VerifiedHAMT.Map Nat Entry :=
  entries.foldr (fun entry map => map.insert entry.slot entry) ∅

def now : IO Nat := IO.monoNanosNow

def report (label : String) (nanos : Nat) (repeats : Nat) : IO Unit := do
  let micros := nanos / 1000
  let perLookup := if repeats = 0 then 0 else (nanos + repeats - 1) / repeats
  IO.println s!"{label}: {micros} us total, {perLookup} ns per operation"

/-- One measured size: build-inclusive cost, then hit-first, hit-last and absent lookups. -/
def benchmarkSize (sink : IO.Ref Nat) (size : Nat) : IO Unit := do
  let corpus := entries size
  let slots := corpus.map (fun entry => entry.slot)
  let first := match slots with
    | [] => 0
    | head :: _ => head
  let last := match slots.reverse with
    | [] => 0
    | head :: _ => head
  let absent := size * 1000 + 5
  let repeats := 2000
  let timed (label : String) (action : Nat → IO Nat) : IO Unit := do
    let start ← now
    let mut total := 0
    for index in [:repeats] do
      total := total + (← action index)
    let stop ← now
    sink.modify (fun seen => seen + total)
    report label (stop - start) repeats
  -- Build-inclusive: build the representation inside the timed region on every repeat.
  timed s!"size {size} build list" (fun _ => do
    let built := corpus.foldl (fun acc entry => entry :: acc) []
    return built.length)
  timed s!"size {size} build index" (fun _ => do
    let built : VerifiedHAMT.Map Nat Entry :=
      corpus.foldr (fun entry map => map.insert entry.slot entry) ∅
    return built.size)
  -- Repeated lookups over an already built representation: the index is built once, outside the
  -- timed loop, so this section is the repeated-lookup cost and not the build cost again.
  let indexed := buildIndex corpus
  for (name, slot) in [("hit-first", first), ("hit-last", last), ("absent", absent)] do
    timed s!"size {size} lookup list {name}" (fun _ => do
      let found := referenceLookup corpus slot
      return match found with
        | none => 0
        | some entry => entry.body + 1)
    timed s!"size {size} lookup index {name}" (fun _ => do
      let found : Option Entry := indexed.find? slot
      return match found with
        | none => 0
        | some entry => entry.body + 1)

def main : IO Unit := do
  IO.println "cold/build-inclusive and repeated-lookup costs, measured on this host"
  let sink ← IO.mkRef 0
  for size in [1, 8, 64, 512] do
    benchmarkSize sink size
  IO.println s!"checksum: {← sink.get}"

end NobleCostBench

/-- Entry point for `lean --run`. -/
def main : IO Unit := NobleCostBench.main
