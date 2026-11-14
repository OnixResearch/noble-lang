import Lean
import Plausible.Sampleable

/- A separate finite test executable. No proof-source declarations depend on Plausible.
   The cell is a type index: generating or shrinking a PilotCase cannot change its
   selected interaction. Only the bounded, non-semantic variant is sampled. -/
open Lean Plausible

namespace PlausiblePilot

structure PilotCase (cell : Fin 4) where
  variant : Fin 4
  deriving Repr

instance (cell : Fin 4) : Arbitrary (PilotCase cell) where
  arbitrary := do
    let v ← Gen.choose Nat 0 3 (by decide)
    return ⟨⟨v.val, by omega⟩⟩

instance (cell : Fin 4) : Shrinkable (PilotCase cell) where
  shrink c := (List.range c.variant.val).map fun n => ⟨Fin.ofNat 4 n⟩

instance (cell : Fin 4) : SampleableExt (PilotCase cell) where
  proxy := PilotCase cell
  proxyRepr := inferInstance
  sample := inferInstance
  shrink := inferInstance
  interp := id

private def jstr (s : String) : Json := Json.str s
private def obj (fields : List (String × Json)) : Json := Json.mkObj fields
private def array (values : List Json) : Json := toJson values
private def push (v : String) : Json := obj [("push", jstr v)]
private def op (s : String) : Json := jstr s
private def one (ops : List Json) : Json := obj [("arith", array ops)]
private def addZero (v : Nat) : List Json := (List.range v).flatMap fun _ => [push "0", op "add"]
private def addZeroText (v : Nat) : List String := (List.range v).flatMap fun _ => ["0", "+"]
private def cellName : Fin 4 → String
  | ⟨0, _⟩ => "P1/plain/small/preserved_prefix_multiple"
  | ⟨1, _⟩ => "P1/plain/zero_negative_one/ordered_single"
  | ⟨2, _⟩ => "P1/quoted/min_max_wrap/ordered_single"
  | _ => "P1/composed/min_max_wrap/ordered_single"

private def interaction (cell : Fin 4) : Json :=
  let (shape, boundary, output, witness) := match cell.val with
    | 0 => ("plain", "small", "preserved_prefix_multiple", ["arithmetic"])
    | 1 => ("plain", "zero_negative_one", "ordered_single", ["arithmetic"])
    | 2 => ("quoted", "min_max_wrap", "ordered_single", ["quote", "run", "arithmetic"])
    | _ => ("composed", "min_max_wrap", "ordered_single", ["quote", "compose", "run", "arithmetic"])
  obj [("profile", jstr "P1-core-source-wasm"), ("cell", jstr (cellName cell)),
    ("quotation_composition", jstr shape), ("boundary_arithmetic", jstr boundary),
    ("output_shape", jstr output), ("source_operator_witness", toJson witness),
    ("witness_kind", jstr "successful-output-and-positive-execution-steps; no per-operator runtime trace")]

private def render (c : PilotCase cell) : Json × String := Id.run do
  let v := c.variant.val
  let pref := if cell.val == 0 then
    [["I64", toString (23 + v)], ["Bool", if v % 2 == 0 then "true" else "false"]] else []
  let p := array (pref.map fun row => array (row.map jstr))
  let interaction := interaction cell
  if cell.val == 0 then
    let ops := [push "2", push "3", op "mul"] ++ addZero v
    let text := String.intercalate " " ([toString (23 + v), if v % 2 == 0 then "true" else "false",
      "2", "3", "*"] ++ addZeroText v) ++ "\n"
    return (obj [("kind", jstr "straight"), ("prefix", p), ("ops", array ops),
      ("interaction", interaction)], text)
  if cell.val == 1 then
    let ops := [push "0", push "-1", op "sub"] ++ addZero v
    let text := String.intercalate " " (["0", "-1", "-"] ++ addZeroText v) ++ "\n"
    return (obj [("kind", jstr "straight"), ("prefix", p), ("ops", array ops),
      ("interaction", interaction)], text)
  if cell.val == 2 then
    let ops := [op "add"] ++ addZero v
    let text := String.intercalate " " (["9223372036854775807", "1", "[", "+"] ++
      addZeroText v ++ ["]", "run"]) ++ "\n"
    return (obj [("kind", jstr "program"), ("prefix", p),
      ("ins", array [push "9223372036854775807", push "1"]),
      ("program", one ops), ("interaction", interaction)], text)
  let second := [push "2", op "mul"] ++ addZero v
  let text := String.intercalate " " (["9223372036854775807", "[", "1", "+", "]",
    "["] ++ ["2", "*"] ++ addZeroText v ++ ["]", "compose", "run"]) ++ "\n"
  return (obj [("kind", jstr "program"), ("prefix", p),
    ("ins", array [push "9223372036854775807"]),
    ("program", obj [("compose", array [one [push "1", op "add"], one second])]),
    ("interaction", interaction)], text)

private def usage : String := "Driver.lean --seed 125 --count 16 --size 8"

def main (args : List String) : IO UInt32 := do
  unless args == ["--seed", "125", "--count", "16", "--size", "8"] do
    IO.eprintln usage
    return 2
  let seed := 125
  for i in [0:16] do
    let cell : Fin 4 := ⟨i % 4, by omega⟩
    -- A distinct explicit seed for every generated sample; the same program
    -- bytes are retained for replay, which never relies on this seeding rule.
    -- The additional stratum offset makes the four samples in one fixed cell
    -- use different low RNG bits (seed+i alone repeats a variant every four).
    let generated := (Plausible.runRandWith (seed + i + (i / 4) * 5)
      (SampleableExt.interpSample (PilotCase cell))).run ⟨8⟩
    let Except.ok sample := generated | throw (IO.userError s!"generator failed at case {i}")
    let (model, source) := render sample
    let positive := (Shrinkable.shrink sample).all fun candidate =>
      candidate.variant.val < sample.variant.val
    unless positive do
      IO.eprintln "cell-preserving shrink order control failed"
      return 1
    let row := obj [("index", toJson i), ("cell_index", toJson cell.val),
      ("variant", toJson sample.variant.val), ("source", jstr source),
      ("model", model), ("positive_control", toJson positive),
      ("false_property_refuted", toJson (decide (sample.variant.val < 0) == false))]
    IO.println row.compress
  return 0

end PlausiblePilot

def main (args : List String) : IO UInt32 := PlausiblePilot.main args
