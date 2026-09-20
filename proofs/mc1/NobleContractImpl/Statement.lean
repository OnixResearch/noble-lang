import NobleContractImpl.Funs

open Aeneas Aeneas.Std Result

set_option maxHeartbeats 1000000
set_option maxRecDepth 4096
set_option Aeneas.customDoElab false

namespace NobleContractImpl.Statement

/-- Observe successful preparation/export; failure and divergence cannot match
a retained statement. The caller supplies the actual extracted functions. -/
def statement
    (preparation : Result (core.result.Result noble_contracts.Prepared noble_contracts.Diagnostic))
    (exporter : noble_contracts.Prepared → Result String) : Option String :=
  match preparation.match with
  | .ok (.Ok prepared) =>
    match (exporter prepared).match with
    | .ok text => some text
    | _ => none
  | _ => none

/- These source-bound equations cover the named MC1 statement-export matrix.
They evaluate the actual extracted frontend and exporter, including the linked
inherited checker. They are not universal compiler-correctness theorems.
Their native-evaluation assumption is audited separately from the strict,
universal application/rule theorems; these modules never enter that library. -/

def incrementSource : String := "(contract 1 increment\n  (input (x I64))\n  (output (y I64))\n  (program [ 1 + ])\n  (requires true)\n  (ensures (eq (out y) (add (in x) 1))))\n"
def incrementStatement : String := "import NobleContracts\nimport NobleContracts.Expression\n\nopen NobleContracts\nnamespace MC1Obligation\n\ndef irRevision : Nat := 1\ndef semanticRevision : Nat := 0\ndef node_0 : Op := .lit (.i64 (BitVec.ofInt 64 (1)))\ndef node_1 : Op := .word 4\ndef program : List Op := [node_0, node_1]\n\ndef inputTypes : List Ty := [.i64]\ndef outputTypes : List Ty := [.i64]\ndef paramTypes : List Ty := []\ndef expression_0 : Term := .bool true\ndef expression_1 : Term := .output 0\ndef expression_2 : Term := .input 0\ndef expression_3 : Term := .i64 (1)\ndef expression_4 : Term := .add expression_2 expression_3\ndef expression_5 : Term := .eq expression_1 expression_4\ndef precondition : Term := expression_0\ndef postcondition : Term := expression_5\ndef claim : Prop := exportedClaim program inputTypes outputTypes paramTypes\n  (Holds precondition) (Holds postcondition)\n\nend MC1Obligation\n"

theorem increment_source_statement :
    statement (do
      let limits ← noble_contracts.Limits.Insts.CoreDefaultDefault.default
      noble_contracts.prepare (toStr incrementSource) limits)
      noble_contracts.rendering.export_lean = some incrementStatement := by
  native_decide

def composedSource : String := "(contract 1 composed-increments\n  (input (x I64))\n  (output (y I64))\n  (program [\n    (block (I64) (I64) [ 1 + ])\n    (block (I64) (I64) [ 1 + ])\n    compose run ])\n  (requires true)\n  (ensures (eq (out y) (add (add (in x) 1) 1))))\n"
def composedStatement : String := "import NobleContracts\nimport NobleContracts.Expression\n\nopen NobleContracts\nnamespace MC1Obligation\n\ndef irRevision : Nat := 1\ndef semanticRevision : Nat := 0\ndef node_0 : Op := .lit (.i64 (BitVec.ofInt 64 (1)))\ndef node_1 : Op := .word 4\ndef node_2 : Op := .block [node_0, node_1]\ndef node_3 : Op := .lit (.i64 (BitVec.ofInt 64 (1)))\ndef node_4 : Op := .word 4\ndef node_5 : Op := .block [node_3, node_4]\ndef node_6 : Op := .word 9\ndef node_7 : Op := .word 10\ndef program : List Op := [node_2, node_5, node_6, node_7]\n\ndef inputTypes : List Ty := [.i64]\ndef outputTypes : List Ty := [.i64]\ndef paramTypes : List Ty := []\ndef expression_0 : Term := .bool true\ndef expression_1 : Term := .output 0\ndef expression_2 : Term := .input 0\ndef expression_3 : Term := .i64 (1)\ndef expression_4 : Term := .add expression_2 expression_3\ndef expression_5 : Term := .i64 (1)\ndef expression_6 : Term := .add expression_4 expression_5\ndef expression_7 : Term := .eq expression_1 expression_6\ndef precondition : Term := expression_0\ndef postcondition : Term := expression_7\ndef claim : Prop := exportedClaim program inputTypes outputTypes paramTypes\n  (Holds precondition) (Holds postcondition)\n\nend MC1Obligation\n"

theorem composed_source_statement :
    statement (do
      let limits ← noble_contracts.Limits.Insts.CoreDefaultDefault.default
      noble_contracts.prepare (toStr composedSource) limits)
      noble_contracts.rendering.export_lean = some composedStatement := by
  native_decide

def familySource : String := "(contract 1 addition-builder-family\n  (input (n I64))\n  (output (p (Program (I64) (I64))))\n  (params (x I64))\n  (program [ quote (block (I64 I64) (I64) [ + ]) compose ])\n  (requires true)\n  (ensures (maps (out p) (param x) (add (param x) (in n)))))\n"
def familyStatement : String := "import NobleContracts\nimport NobleContracts.Expression\n\nopen NobleContracts\nnamespace MC1Obligation\n\ndef irRevision : Nat := 1\ndef semanticRevision : Nat := 0\ndef node_0 : Op := .word 8\ndef node_1 : Op := .word 4\ndef node_2 : Op := .block [node_1]\ndef node_3 : Op := .word 9\ndef program : List Op := [node_0, node_2, node_3]\n\ndef inputTypes : List Ty := [.i64]\ndef outputTypes : List Ty := [(.program [.i64] [.i64])]\ndef paramTypes : List Ty := [.i64]\ndef expression_0 : Term := .bool true\ndef expression_1 : Term := .output 0\ndef expression_2 : Term := .param 0\ndef expression_3 : Term := .param 0\ndef expression_4 : Term := .input 0\ndef expression_5 : Term := .add expression_3 expression_4\ndef expression_6 : Term := .maps expression_1 expression_2 expression_5\ndef precondition : Term := expression_0\ndef postcondition : Term := expression_6\ndef claim : Prop := exportedClaim program inputTypes outputTypes paramTypes\n  (Holds precondition) (Holds postcondition)\n\nend MC1Obligation\n"

theorem family_source_statement :
    statement (do
      let limits ← noble_contracts.Limits.Insts.CoreDefaultDefault.default
      noble_contracts.prepare (toStr familySource) limits)
      noble_contracts.rendering.export_lean = some familyStatement := by
  native_decide

def structuralSource : String := "(contract 1 structural-roundtrip\n  (input (xs (List I64)) (a I64))\n  (output (p (Pair (List I64) I64)))\n  (define identity Bool true)\n  (program [ pair ])\n  (requires (def identity))\n  (ensures (and (eq (first (out p)) (in xs))\n                (eq (second (out p)) (in a)))))\n"
def structuralStatement : String := "import NobleContracts\nimport NobleContracts.Expression\n\nopen NobleContracts\nnamespace MC1Obligation\n\ndef irRevision : Nat := 1\ndef semanticRevision : Nat := 0\ndef node_0 : Op := .word 13\ndef program : List Op := [node_0]\n\ndef inputTypes : List Ty := [(.list .i64), .i64]\ndef outputTypes : List Ty := [(.pair (.list .i64) .i64)]\ndef paramTypes : List Ty := []\ndef expression_0 : Term := .bool true\ndef expression_1 : Term := .definition 0 expression_0\ndef expression_2 : Term := .output 0\ndef expression_3 : Term := .first expression_2\ndef expression_4 : Term := .input 0\ndef expression_5 : Term := .eq expression_3 expression_4\ndef expression_6 : Term := .output 0\ndef expression_7 : Term := .second expression_6\ndef expression_8 : Term := .input 1\ndef expression_9 : Term := .eq expression_7 expression_8\ndef expression_10 : Term := .and expression_5 expression_9\ndef precondition : Term := expression_1\ndef postcondition : Term := expression_10\ndef claim : Prop := exportedClaim program inputTypes outputTypes paramTypes\n  (Holds precondition) (Holds postcondition)\n\nend MC1Obligation\n"

theorem structural_source_statement :
    statement (do
      let limits ← noble_contracts.Limits.Insts.CoreDefaultDefault.default
      noble_contracts.prepare (toStr structuralSource) limits)
      noble_contracts.rendering.export_lean = some structuralStatement := by
  native_decide

def syntaxSource : String := "(contract 1 reflected-syntax (input (p (Program (I64) (I64)))) (output (s Syntax)) (program [ reflect ]) (requires true) (ensures true))\n"
def syntaxStatement : String := "import NobleContracts\nimport NobleContracts.Expression\n\nopen NobleContracts\nnamespace MC1Obligation\n\ndef irRevision : Nat := 1\ndef semanticRevision : Nat := 0\ndef node_0 : Op := .word 11\ndef program : List Op := [node_0]\n\ndef inputTypes : List Ty := [(.program [.i64] [.i64])]\ndef outputTypes : List Ty := [.«syntax»]\ndef paramTypes : List Ty := []\ndef expression_0 : Term := .bool true\ndef expression_1 : Term := .bool true\ndef precondition : Term := expression_0\ndef postcondition : Term := expression_1\ndef claim : Prop := exportedClaim program inputTypes outputTypes paramTypes\n  (Holds precondition) (Holds postcondition)\n\nend MC1Obligation\n"

theorem syntax_source_statement :
    statement (do
      let limits ← noble_contracts.Limits.Insts.CoreDefaultDefault.default
      noble_contracts.prepare (toStr syntaxSource) limits)
      noble_contracts.rendering.export_lean = some syntaxStatement := by
  native_decide

def wrapSource : String := "(contract 1 signed-wrap (input) (output (y I64)) (program [ 9223372036854775807 1 + ]) (requires true) (ensures (eq (out y) -9223372036854775808)))\n"
def wrapStatement : String := "import NobleContracts\nimport NobleContracts.Expression\n\nopen NobleContracts\nnamespace MC1Obligation\n\ndef irRevision : Nat := 1\ndef semanticRevision : Nat := 0\ndef node_0 : Op := .lit (.i64 (BitVec.ofInt 64 (9223372036854775807)))\ndef node_1 : Op := .lit (.i64 (BitVec.ofInt 64 (1)))\ndef node_2 : Op := .word 4\ndef program : List Op := [node_0, node_1, node_2]\n\ndef inputTypes : List Ty := []\ndef outputTypes : List Ty := [.i64]\ndef paramTypes : List Ty := []\ndef expression_0 : Term := .bool true\ndef expression_1 : Term := .output 0\ndef expression_2 : Term := .i64 (-9223372036854775808)\ndef expression_3 : Term := .eq expression_1 expression_2\ndef precondition : Term := expression_0\ndef postcondition : Term := expression_3\ndef claim : Prop := exportedClaim program inputTypes outputTypes paramTypes\n  (Holds precondition) (Holds postcondition)\n\nend MC1Obligation\n"

theorem wrap_source_statement :
    statement (do
      let limits ← noble_contracts.Limits.Insts.CoreDefaultDefault.default
      noble_contracts.prepare (toStr wrapSource) limits)
      noble_contracts.rendering.export_lean = some wrapStatement := by
  native_decide

end NobleContractImpl.Statement
