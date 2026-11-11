import NobleM2.SizeChangeMatrixPathProduct

/-!
# Weakening is monotone under composition (issue #101)

Long size-change composites collapse onto short ones because information only degrades along a path.
The order that expresses it: `r` is *at least as strong as* `s` when `r` implies `s` — `strict`
stronger than `nonIncrease`, `same` stronger than `nonIncrease`, both stronger than `unknown`, and
each relation equal to itself. `strict` and `same` are incomparable, so this is a partial order, not
a chain.

`weakerEq` decides the relation; `compose_weaker_left` and `compose_weaker_right` prove composition
monotone in each factor. That is the property a bound argument uses when it replaces one entry of a
path by a weaker one without changing what the path can do.

The counting bound itself is not proved here.
-/

namespace NobleM2.SizeChange

/-- Relation strength by implication: `strict` stronger than `nonIncrease`, `same` stronger than
`nonIncrease`, all stronger than `unknown`, and each relation at least as strong as itself. -/
def Rel4.weakerEq : Rel4 → Rel4 → Bool
  | .strict, .strict => true
  | .strict, .nonIncrease => true
  | .strict, .unknown => true
  | .nonIncrease, .nonIncrease => true
  | .nonIncrease, .unknown => true
  | .same, .same => true
  | .same, .nonIncrease => true
  | .same, .unknown => true
  | .unknown, .unknown => true
  | _, _ => false

/-- The order is reflexive. -/
theorem Rel4.weakerEq_refl (r : Rel4) : r.weakerEq r = true := by
  cases r <;> rfl

/-- Composing a weaker left factor weakens the product. -/
theorem compose_weaker_left {r r' s : Rel4} (h : r.weakerEq r' = true) :
    (r.compose s).weakerEq (r'.compose s) = true := by
  cases r <;> cases r' <;> cases s <;> simp_all [Rel4.weakerEq, Rel4.compose]

/-- Composing a weaker right factor weakens the product. -/
theorem compose_weaker_right {r s s' : Rel4} (h : s.weakerEq s' = true) :
    (r.compose s).weakerEq (r.compose s') = true := by
  cases r <;> cases s <;> cases s' <;> simp_all [Rel4.weakerEq, Rel4.compose]

end NobleM2.SizeChange
