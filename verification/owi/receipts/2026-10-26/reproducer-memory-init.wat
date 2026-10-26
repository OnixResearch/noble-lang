(module
  (import "noble" "memory" (memory 16 16))
  (data $d "abc")
  (func $g (export "g") (result i64)
    i32.const 0
    i32.const 0
    i32.const 3
    memory.init $d
    i64.const 7))
