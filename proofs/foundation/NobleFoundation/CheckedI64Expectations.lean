import NobleFoundation.CheckedI64

namespace NobleFoundation.CheckedI64Expectations

open NobleFoundation.CheckedI64

def renderScalar : Except ArithError ℤ → String
  | .ok value => "ok:" ++ toString value
  | .error .overflow => "error:1"
  | .error .divisionByZero => "error:2"

def renderDivRem : Except ArithError (ℤ × ℤ) → String
  | .ok (quotient, remainder) =>
      "pair:" ++ toString quotient ++ ":" ++ toString remainder
  | .error .overflow => "error:1"
  | .error .divisionByZero => "error:2"

def evaluate (operation : String) (left right : ℤ) : String :=
  match operation with
  | "add" => renderScalar (checkedAdd left right)
  | "sub" => renderScalar (checkedSub left right)
  | "mul" => renderScalar (checkedMul left right)
  | "neg" => renderScalar (checkedNeg left)
  | "abs" => renderScalar (checkedAbs left)
  | "lt" =>
      if left < right then
        "bool:true"
      else
        "bool:false"
  | "divrem" => renderDivRem (checkedDivRem left right)
  | _ => "invalid"

/-- External arithmetic proposals are retained verbatim and checked against the model. -/
def retained : List (String × ℤ × ℤ × String) := [
  ("add", 9223372036854775806, 1, "ok:9223372036854775807"),
  ("add", 9223372036854775807, 1, "error:1"),
  ("add", -9223372036854775807, -1, "ok:-9223372036854775808"),
  ("add", -9223372036854775808, -1, "error:1"),
  ("add", 0, -1, "ok:-1"),
  ("add", -1, 0, "ok:-1"),
  ("add", 0, 0, "ok:0"),
  ("add", 1, 2, "ok:3"),
  ("add", 1, -2, "ok:-1"),
  ("add", -1, 2, "ok:1"),
  ("add", -1, -2, "ok:-3"),
  ("sub", -9223372036854775807, 1, "ok:-9223372036854775808"),
  ("sub", -9223372036854775808, 1, "error:1"),
  ("sub", 9223372036854775807, -1, "error:1"),
  ("sub", 9223372036854775807, 1, "ok:9223372036854775806"),
  ("sub", 0, -1, "ok:1"),
  ("sub", -1, 0, "ok:-1"),
  ("sub", 1, 2, "ok:-1"),
  ("sub", 2, 1, "ok:1"),
  ("sub", -1, -2, "ok:1"),
  ("mul", 9223372036854775807, 1, "ok:9223372036854775807"),
  ("mul", -9223372036854775808, 1, "ok:-9223372036854775808"),
  ("mul", -9223372036854775808, -1, "error:1"),
  ("mul", 9223372036854775807, 2, "error:1"),
  ("mul", -9223372036854775808, 0, "ok:0"),
  ("mul", -1, 3, "ok:-3"),
  ("mul", 3, -2, "ok:-6"),
  ("mul", -2, -3, "ok:6"),
  ("neg", 9223372036854775807, 0, "ok:-9223372036854775807"),
  ("neg", -9223372036854775807, 0, "ok:9223372036854775807"),
  ("neg", -9223372036854775808, 0, "error:1"),
  ("neg", 0, 0, "ok:0"),
  ("neg", -1, 0, "ok:1"),
  ("abs", -9223372036854775808, 0, "error:1"),
  ("abs", -9223372036854775807, 0, "ok:9223372036854775807"),
  ("abs", -1, 0, "ok:1"),
  ("abs", 0, 0, "ok:0"),
  ("abs", 9223372036854775807, 0, "ok:9223372036854775807"),
  ("lt", -1, 0, "bool:true"),
  ("lt", 0, -1, "bool:false"),
  ("lt", -9223372036854775808, 9223372036854775807, "bool:true"),
  ("lt", 9223372036854775807, -9223372036854775808, "bool:false"),
  ("lt", -1, -1, "bool:false"),
  ("lt", 1, 2, "bool:true"),
  ("lt", 1, -2, "bool:false"),
  ("lt", -2, -1, "bool:true"),
  ("divrem", -7, 2, "pair:-3:-1"),
  ("divrem", 7, -2, "pair:-3:1"),
  ("divrem", 7, 2, "pair:3:1"),
  ("divrem", -7, -2, "pair:3:-1"),
  ("divrem", 2, 7, "pair:0:2"),
  ("divrem", -9223372036854775808, 1, "pair:-9223372036854775808:0"),
  ("divrem", -9223372036854775807, 1, "pair:-9223372036854775807:0"),
  ("divrem", 9223372036854775807, -1, "pair:-9223372036854775807:0"),
  ("divrem", -9223372036854775808, -1, "error:1"),
  ("divrem", 7, 0, "error:2"),
  ("divrem", 0, -1, "pair:0:0")
]

theorem retained_expectations_checked :
    retained.all (fun (operation, left, right, expected) =>
      evaluate operation left right == expected) = true := by
  decide

end NobleFoundation.CheckedI64Expectations
