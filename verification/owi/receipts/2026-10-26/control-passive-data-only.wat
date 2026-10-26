(module
  (import "noble" "memory" (memory 16 16))
  (data $d "abc")
  (func $g (export "g") (result i64)
    i64.const 7))
