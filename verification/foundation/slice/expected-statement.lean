import NobleContracts
import NobleContracts.Expression

open NobleContracts
namespace MC1Obligation

def irRevision : Nat := 1
def semanticRevision : Nat := 0
def node_0 : Op := .word 8
def node_1 : Op := .lit (.i64 (BitVec.ofInt 64 (1)))
def node_2 : Op := .word 4
def node_3 : Op := .block [node_1, node_2]
def node_4 : Op := .word 9
def node_5 : Op := .word 10
def program : List Op := [node_0, node_3, node_4, node_5]

def inputTypes : List Ty := [.i64]
def outputTypes : List Ty := [.i64]
def paramTypes : List Ty := []
def expression_0 : Term := .bool true
def expression_1 : Term := .output 0
def expression_2 : Term := .input 0
def expression_3 : Term := .i64 (1)
def expression_4 : Term := .add expression_2 expression_3
def expression_5 : Term := .eq expression_1 expression_4
def precondition : Term := expression_0
def postcondition : Term := expression_5
def claim : Prop := exportedClaim program inputTypes outputTypes paramTypes
  (Holds precondition) (Holds postcondition)

end MC1Obligation
