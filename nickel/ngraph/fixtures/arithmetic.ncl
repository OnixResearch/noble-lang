let graph = import "../library.ncl" in
let arithmetic = fun bias scale => graph.template "arithmetic/1"
  [graph.parameter "bias" bias, graph.parameter "scale" scale]
  ["x"]
  [
    graph.input "x",
    graph.constant "bias" bias,
    graph.add "shared" "x" "bias",
    graph.constant "scale" scale,
    graph.mul "scaled" "shared" "scale",
    graph.sub "difference" "shared" "x",
  ]
  ["scaled", "difference", "shared"] in
let first = arithmetic "9223372036854775807" "9007199254740993" in
let second = arithmetic "-9223372036854775808" "-2" in
graph.bundle
  [
    graph.parameter "first.bias" "9223372036854775807",
    graph.parameter "first.scale" "9007199254740993",
    graph.parameter "second.bias" "-9223372036854775808",
    graph.parameter "second.scale" "-2",
  ]
  [graph.instantiate "first" first, graph.instantiate "second" second]
  [graph.wire (graph.handle "first" "shared") (graph.handle "second" "x")]
  [graph.handle "first" "x"]
  [
    graph.handle "first" "scaled",
    graph.handle "first" "difference",
    graph.handle "second" "scaled",
    graph.handle "second" "difference",
    graph.handle "first" "shared",
    graph.handle "second" "shared",
    graph.handle "first" "scaled",
  ]
