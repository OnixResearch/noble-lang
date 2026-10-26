import VerifiedHAMT
open VerifiedHAMT
open scoped VerifiedHAMT.Map

-- Duplicate-slot semantics of the bundled constructor.
def dup : Map Nat String := Map.ofList [(7, "old"), (3, "three"), (7, "new")]

#eval dup.find? 7
#eval dup.size
#eval (Map.empty.insert 7 "old").insert 7 "new" |>.find? 7
#eval (Map.empty.insert 7 "old").insert 7 "new" |>.size
