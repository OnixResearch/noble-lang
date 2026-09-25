import NobleKernel

open Aeneas Aeneas.Std
open noble_kernel

set_option Aeneas.customDoElab false
set_option maxHeartbeats 1000000

namespace M7Syndicate

/- These pure references are independent of the extracted Rust functions. The
   outer Result is Aeneas' checked-computation envelope; it does not attest to
   host admission, a component engine, network delivery, or native cleanup. -/
def decide : Option Bool → Bool → dataspace.Publication
  | none, _ => .Added
  | some previous, requested =>
      if previous = requested then .Unchanged else .Replaced

def allowed (rights : dataspace.Rights) : dataspace.Operation → Bool
  | .Publish => rights.publish
  | .Observe => rights.observe

def admit (requested : Bool) : core.result.Result Unit dataspace.Error :=
  if requested then .Err .Denied else .Ok ()

/-- Every actual Rust publication decision equals the independent finite
    reference, including replacement and unchanged duplicate publication. -/
theorem publication_refines (current : Option Bool) (requested : Bool) :
    dataspace.decide_publication current requested = .ok (decide current requested) := by
  cases current with
  | none => rfl
  | some previous =>
      cases previous <;> cases requested <;>
        simp [dataspace.decide_publication, decide]

/-- The operation's WIT name is not a right: each extracted policy result is
    exactly the host-issued bit selected by the independent reference. -/
theorem permission_refines (rights : dataspace.Rights) (operation : dataspace.Operation) :
    dataspace.permits rights operation = .ok (allowed rights operation) := by
  cases operation <;> rfl

/-- The actual admission guard refuses a requested shared mutable guest memory
    before the host can issue a facet; this is only the pure guard, not engine
    evidence that two guest stores physically cannot share memory. -/
theorem shared_memory_refines (requested : Bool) :
    dataspace.admit_shared_memory requested = .ok (admit requested) := by
  cases requested <;> rfl

/-- An explicit shared-memory request is a denial, never successful admission. -/
theorem shared_memory_refused :
    dataspace.admit_shared_memory true = .ok (.Err dataspace.Error.Denied) := by
  rw [shared_memory_refines]
  rfl

/-- The observe right cannot grant publication. -/
theorem publish_right_required (rights : dataspace.Rights) (denied : rights.publish = false) :
    dataspace.permits rights .Publish = .ok false := by
  rw [permission_refines]
  simp [allowed, denied]

/-- The publish right cannot grant observation. -/
theorem observe_right_required (rights : dataspace.Rights) (denied : rights.observe = false) :
    dataspace.permits rights .Observe = .ok false := by
  rw [permission_refines]
  simp [allowed, denied]

/-- Repeating an owned ready value does not request an additional publication. -/
theorem duplicate_publication_unchanged (ready : Bool) :
    dataspace.decide_publication (some ready) ready = .ok .Unchanged := by
  rw [publication_refines]
  simp [decide]

/-- A different ready value is a replacement, not an unchanged duplicate. -/
theorem changed_publication_replaces (previous requested : Bool)
    (different : previous ≠ requested) :
    dataspace.decide_publication (some previous) requested = .ok .Replaced := by
  rw [publication_refines]
  simp [decide, different]

/-- The first assertion is an addition, independent of its ready bit. -/
theorem first_publication_adds (ready : Bool) :
    dataspace.decide_publication none ready = .ok .Added := by
  rw [publication_refines]
  rfl

end M7Syndicate
