let graph = import "../library.ncl" in
let identity_template = graph.template "identity/1" [] ["x"]
  [graph.input "x"] ["x"] in
let identity_bundle = graph.bundle []
  [graph.instantiate "identity" identity_template] []
  [graph.handle "identity" "x"] [graph.handle "identity" "x"] in
let copy_bundle = graph.bundle []
  [graph.instantiate "copy" identity_template] []
  [graph.handle "copy" "x"]
  [graph.handle "copy" "x", graph.handle "copy" "x"] in
let add_template = graph.template "add-5/1"
  [graph.parameter "offset" "5"] ["x"]
  [
    graph.input "x",
    graph.constant "offset" "5",
    graph.add "result" "x" "offset",
  ] ["result"] in
let subtract_template = graph.template "subtract-2/1"
  [graph.parameter "offset" "2"] ["x"]
  [
    graph.input "x",
    graph.constant "offset" "2",
    graph.sub "result" "x" "offset",
  ] ["result"] in
let product_bundle = graph.bundle
  [graph.parameter "left.offset" "5", graph.parameter "right.offset" "2"]
  [graph.instantiate "left" add_template, graph.instantiate "right" subtract_template]
  []
  [graph.handle "left" "x", graph.handle "right" "x"]
  [graph.handle "left" "result", graph.handle "right" "result"] in
let permutation_template = graph.template "permutation/1" [] ["x", "y"]
  [graph.input "x", graph.input "y"] ["y", "x"] in
let permutation_bundle = graph.bundle []
  [graph.instantiate "permutation" permutation_template] []
  [graph.handle "permutation" "x", graph.handle "permutation" "y"]
  [graph.handle "permutation" "y", graph.handle "permutation" "x"] in
let add_stage = graph.template "add/1" [graph.parameter "offset" "5"] ["x"]
  [
    graph.input "x",
    graph.constant "offset" "5",
    graph.add "result" "x" "offset",
  ] ["result"] in
let multiply_stage = graph.template "multiply/1" [graph.parameter "factor" "2"] ["x"]
  [
    graph.input "x",
    graph.constant "factor" "2",
    graph.mul "result" "x" "factor",
  ] ["result"] in
let subtract_stage = graph.template "subtract/1" [graph.parameter "offset" "3"] ["x"]
  [
    graph.input "x",
    graph.constant "offset" "3",
    graph.sub "result" "x" "offset",
  ] ["result"] in
let left_associated = graph.bundle
  [
    graph.parameter "add.offset" "5",
    graph.parameter "multiply.factor" "2",
    graph.parameter "subtract.offset" "3",
  ]
  [
    graph.instantiate "root.left.left" add_stage,
    graph.instantiate "root.left.right" multiply_stage,
    graph.instantiate "root.right" subtract_stage,
  ]
  [
    graph.wire (graph.handle "root.left.left" "result") (graph.handle "root.left.right" "x"),
    graph.wire (graph.handle "root.left.right" "result") (graph.handle "root.right" "x"),
  ]
  [graph.handle "root.left.left" "x"]
  [graph.handle "root.right" "result"] in
let right_associated = graph.bundle
  [
    graph.parameter "add.offset" "5",
    graph.parameter "multiply.factor" "2",
    graph.parameter "subtract.offset" "3",
  ]
  [
    graph.instantiate "root.left" add_stage,
    graph.instantiate "root.right.left" multiply_stage,
    graph.instantiate "root.right.right" subtract_stage,
  ]
  [
    graph.wire (graph.handle "root.left" "result") (graph.handle "root.right.left" "x"),
    graph.wire (graph.handle "root.right.left" "result") (graph.handle "root.right.right" "x"),
  ]
  [graph.handle "root.left" "x"]
  [graph.handle "root.right.right" "result"] in
{
  identity = identity_bundle,
  scalar_copy = copy_bundle,
  product = product_bundle,
  permutation = permutation_bundle,
  reassociation_left = left_associated,
  reassociation_right = right_associated,
}
