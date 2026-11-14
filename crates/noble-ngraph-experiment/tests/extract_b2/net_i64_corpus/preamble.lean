import InteractionNets
import NobleFoundation.NetI64
import NobleFoundation.PackedSchedule

namespace NobleFoundation.NetI64Corpus

open NobleFoundation.NetI64
open NobleFoundation.PackedSchedule

private def compareId : List Nat → List Nat → Ordering
  | [], [] => .eq
  | [], _ :: _ => .lt
  | _ :: _, [] => .gt
  | left :: leftTail, right :: rightTail =>
      match compare left right with
      | .eq => compareId leftTail rightTail
      | order => order

private def comparePort (left right : INet.Port) : Ordering :=
  match compareId left.1 right.1 with
  | .eq => compare left.2 right.2
  | order => order

private def compareWire (left right : INet.Port × INet.Port) : Ordering :=
  match comparePort left.1 right.1 with
  | .eq => comparePort left.2 right.2
  | order => order

private def insertSorted {α : Type} (less : α → α → Bool) (value : α) : List α → List α
  | [] => [value]
  | head :: rest =>
      if less value head then value :: head :: rest
      else head :: insertSorted less value rest

private def sortSorted {α : Type} (less : α → α → Bool) : List α → List α
  | [] => []
  | head :: rest => insertSorted less head (sortSorted less rest)

private def canonicalNet (net : INet.XNet NetSym) : INet.XNet NetSym :=
  { agents := sortSorted (fun left right => compareId left.1 right.1 == .lt) net.agents
    free := sortSorted (fun left right => comparePort left right == .lt) net.free
    wires := sortSorted (fun left right => compareWire left right == .lt) net.wires }

private def normalizeReadBack (program : Program) (inputs : List (BitVec 64)) :=
  (Program.normalize program inputs).map fun (net, steps) =>
    (Program.readBack program net, steps)

-- Rust-produced input nets, differential outputs and interaction counts follow.

