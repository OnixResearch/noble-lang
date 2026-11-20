/-
Proof erasure for `Intrinsic-Proofs-Draft` (`intrinsic-noble-proofs`, task 3,
PO-20's erasure obligation at model level).

A module's members are `def`, `contract` and `proof`. Contracts and proofs are
compile-time-only declarations: erasing them must leave the executable
projection of the module unchanged, so that no DefinitionId, recipe input or
ordinary runtime observation moves.

What the compiler actually feeds into identity, read from the sources:

- `crates/noble-contracts/src/source/chunks.rs:138-147` — a `DefinitionRef`
  pins the referenced entry's lookup `identity`, `format`,
  `semantic_revision` and `source_revision`.
- `chunks.rs:257-273` — an `Envelope` is the admitted definition: format,
  semantic revision, profile, chunk id, source revision, interface, captures,
  dependencies, instantiations and the body `Recipe`.
- `chunks.rs:246-250` — a `Recipe` is a finite node arena plus its entry
  references.
- `chunks.rs:964` — `chunk_identity_of` derives the chunk id from the
  envelope, and `chunks.rs:2494` records `identity = envelope.chunk_id` on the
  admitted entry, so the identity is a function of the envelope.
- `crates/noble-contracts/src/source/emission/identity.rs:5-24` — the
  specialization identity extends that identity with the selected bound
  adapters, and its doc comment states that "the module's nominal id remains
  independent of this linked-program recipe".

The model below names exactly those inputs: `DefinitionRef` (identity, format,
semantic revision, source revision), `Definition` (identity, format, semantic
revision, source revision, interface and captures as opaque finite lists,
dependencies, the recipe's nodes and entry references, and the derived expanded
node count) and `specialize` (the specialization fold over selected adapter
slots). It proves that a module's definitions, its DefinitionIds, its recipe
inputs, its exports, its expanded node count and its specialization identities
are the same before and after erasure, and that a definition's observation —
its recipe's entry references — does not move.

It is a model claim, and a narrow one. It says nothing about the Rust path that
compiles a module: `noble-wasm`'s emission, the chunk envelope validation and
`recompute` in `noble-contracts` stay trusted, no Rust refinement is claimed,
and the `Envelope`'s hashing (protocol pins, digests, chunk-id collisions) is
represented by opaque `Nat` fields rather than modelled. What is proved is that
the erasure *function* has no access to any identity, recipe or observation
input: every such input is a function of the definition members alone, and
erasure does not touch them.
-/
import NobleFoundation.IntrinsicCalculus

namespace NobleFoundation.IntrinsicErasure

open NobleFoundation.IntrinsicCalculus (Proposition Proof)

/-! ## The compiler's identity and recipe inputs -/

/-- A reference to an admitted definition, exactly the fields
`chunks.rs:138-147` pins: lookup identity, envelope format, semantic revision
and emitted-source revision. -/
structure DefinitionRef where
  identity : Nat
  format : Nat
  semanticRevision : Nat
  sourceRevision : Nat
  deriving DecidableEq, Repr

/-- The admitted definition as the compiler feeds it into identity: the chunk
envelope's own fields (`format`, `semanticRevision`, `chunk_id` as `identity`,
`sourceRevision`), its interface and captures as opaque finite lists, its
dependency references, its body recipe (node arena and entry references,
`chunks.rs:246-250`) and the derived expanded node count
(`chunks.rs:280-286`). -/
structure Definition where
  identity : Nat
  format : Nat
  semanticRevision : Nat
  sourceRevision : Nat
  interface : List Nat
  captures : List Nat
  dependencies : List DefinitionRef
  nodes : List Nat
  entry : List Nat
  expandedNodes : Nat
  deriving DecidableEq, Repr

/-- The specialization identity of `emission/identity.rs:5-24`: the definition's
identity folded with the selected bound adapters' slots. Its inputs are the
identity and the selection, and the selection is the linker's environment, not
source text. -/
def specialize (identity : Nat) : List Nat → Nat
  | [] => identity
  | slot :: rest => specialize (identity * 31 + slot + 17) rest

/-- A module member. A `definition` carries its name, its admitted body and
whether the module exports it; `contract` and `proof` carry only compile-time
claims and a resolved proof term of the intrinsic calculus. -/
inductive Member where
  | definition (name : Nat) (body : Definition) (exported : Bool)
  | contract (claim : Proposition)
  | proof (statement : Proposition) (term : Proof)

/-- A module in the `Declared-Modules-v1` shape: a name, a version and its
members in source order. -/
structure Module where
  name : Nat
  version : Nat
  members : List Member

/-! ## The executable projection -/

/-- Whether a member is an executable definition. -/
def Member.isDefinition : Member → Bool
  | .definition _ _ _ => true
  | .contract _ => false
  | .proof _ _ => false

/-- Erasure: drop every contract and every proof, keeping the definitions in
source order. -/
def Module.erase (m : Module) : Module :=
  { m with members := m.members.filter Member.isDefinition }

/-- A module's definitions in source order, with the names they are exported
under. -/
def Module.definitions (m : Module) : List (Nat × Definition) :=
  m.members.filterMap fun
    | .definition name body _ => some (name, body)
    | .contract _ => none
    | .proof _ _ => none

/-- The DefinitionIds a module binds, in source order. -/
def Module.definitionIds (m : Module) : List Nat :=
  (m.definitions.map fun entry => entry.2.identity)

/-- The recipe inputs of every definition, in source order: the node arena and
the entry references. -/
def Module.recipeInputs (m : Module) : List (List Nat × List Nat) :=
  m.definitions.map fun entry => (entry.2.nodes, entry.2.entry)

/-- The dependency references a module's definitions carry. -/
def Module.dependencies (m : Module) : List (List DefinitionRef) :=
  m.definitions.map fun entry => entry.2.dependencies

/-- The exported definitions' names. -/
def Module.exportedNames (m : Module) : List Nat :=
  m.members.filterMap fun
    | .definition name _ true => some name
    | .definition _ _ false => none
    | .contract _ => none
    | .proof _ _ => none

/-- The expanded node count of every definition. -/
def Module.expandedNodes (m : Module) : List Nat :=
  m.definitions.map fun entry => entry.2.expandedNodes

/-- The specialization identity of every definition under a linker selection. -/
def Module.specializations (m : Module) (selection : List Nat) : List Nat :=
  m.definitions.map fun entry => specialize entry.2.identity selection

/-- The ordinary runtime observation of one definition: the entry references of
its body recipe, which are the slots an ordinary run reaches first. -/
def recipeObservation (d : Definition) : List Nat := d.entry

/-- The module's ordinary runtime observations, in source order. -/
def Module.observed (m : Module) : List (List Nat) :=
  m.definitions.map fun entry => recipeObservation entry.2

/-! ## Erasure leaves every identity, recipe and observation input unchanged -/

/-- The claim every identity, recipe and observation input rests on: the
definition list a module projects is the same before and after erasure. Proved
member by member, because a member's contribution is exactly its own definition
and a contract or a proof contributes nothing. -/
theorem Module.definitions_erase (m : Module) : m.erase.definitions = m.definitions := by
  obtain ⟨name, version, members⟩ := m
  induction members with
  | nil => rfl
  | cons head tail ih =>
      cases head with
      | definition n b e => exact congrArg (List.cons (n, b)) ih
      | contract c => exact ih
      | proof st tm => exact ih

/-- The exported names are the same before and after erasure: only an exported
definition names one. -/
theorem Module.exportedNames_erase (m : Module) :
    m.erase.exportedNames = m.exportedNames := by
  obtain ⟨name, version, members⟩ := m
  induction members with
  | nil => rfl
  | cons head tail ih =>
      cases head with
      | definition n b e =>
          cases e with
          | true => exact congrArg (List.cons n) ih
          | false => exact ih
      | contract c => exact ih
      | proof st tm => exact ih

/-- Erasure is idempotent. -/
theorem Module.erase_idempotent (m : Module) : m.erase.erase = m.erase := by
  simp [Module.erase]

/-- Erasure keeps the module's name. -/
theorem Module.erase_name (m : Module) : m.erase.name = m.name := rfl

/-- Erasure keeps the module's version. -/
theorem Module.erase_version (m : Module) : m.erase.version = m.version := rfl

/-- The DefinitionIds do not move. -/
theorem Module.definitionIds_erase (m : Module) : m.erase.definitionIds = m.definitionIds := by
  simp only [Module.definitionIds, Module.definitions_erase]

/-- The recipe inputs do not move. -/
theorem Module.recipeInputs_erase (m : Module) :
    m.erase.recipeInputs = m.recipeInputs := by
  simp only [Module.recipeInputs, Module.definitions_erase]

/-- The dependency references do not move. -/
theorem Module.dependencies_erase (m : Module) :
    m.erase.dependencies = m.dependencies := by
  simp only [Module.dependencies, Module.definitions_erase]

/-- The expanded node counts do not move, so erasure adds no emitted node. -/
theorem Module.expandedNodes_erase (m : Module) :
    m.erase.expandedNodes = m.expandedNodes := by
  simp only [Module.expandedNodes, Module.definitions_erase]

/-- The specialization identities do not move: they are the definitions'
identities folded with the linker's selection, and erasure touches neither. -/
theorem Module.specializations_erase (m : Module) (selection : List Nat) :
    m.erase.specializations selection = m.specializations selection := by
  simp only [Module.specializations, Module.definitions_erase]

/-- The ordinary runtime observations do not move. -/
theorem Module.observed_erase (m : Module) : m.erase.observed = m.observed := by
  simp only [Module.observed, Module.definitions_erase]

/-- A contract or proof member can be dropped one at a time with the same
result as erasing the whole module, so no future erasure step can differ. -/
theorem Module.erase_cons_contract (name version : Nat) (members : List Member)
    (claim : Proposition) :
    (Module.mk name version (Member.contract claim :: members)).erase =
      (Module.mk name version members).erase := by
  simp [Module.erase, Member.isDefinition]

theorem Module.erase_cons_proof (name version : Nat) (members : List Member)
    (statement : Proposition) (term : Proof) :
    (Module.mk name version (Member.proof statement term :: members)).erase =
      (Module.mk name version members).erase := by
  simp [Module.erase, Member.isDefinition]

/-- A definition member is kept by erasure: the definitions of the erased module
start with it and continue with the erased tail. -/
theorem Module.erase_cons_definition (name version : Nat) (members : List Member)
    (dname : Nat) (body : Definition) (exported : Bool) :
    (Module.mk name version (Member.definition dname body exported :: members)).erase.members =
      Member.definition dname body exported ::
        (Module.mk name version members).erase.members := by
  simp [Module.erase, Member.isDefinition, List.filter_cons]

end NobleFoundation.IntrinsicErasure
