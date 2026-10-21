(component
  (import "noble-test:worker-send/host@1.0.0" (instance $host
    (export "begin-send" (func (param "attempt" s64) (param "text" string)
      (result (future (result s64 (error string))))))
    (export "cancel-send" (func
      (param "value" (future (result s64 (error string))))))))
  (core module $memory
    (memory (export "memory") 1 1)
    (global $next (mut i32) (i32.const 1024))
    (func (export "realloc")
      (param $old i32) (param $old-len i32) (param $align i32) (param $new-len i32)
      (result i32) (local $ptr i32)
      (local.set $ptr (i32.and
        (i32.add (global.get $next) (i32.sub (local.get $align) (i32.const 1)))
        (i32.sub (i32.const 0) (local.get $align))))
      (if (i64.gt_u
        (i64.add (i64.extend_i32_u (local.get $ptr))
          (i64.extend_i32_u (local.get $new-len)))
        (i64.const 65536))
        (then unreachable))
      (if (i32.ne (local.get $old-len) (i32.const 0))
        (then (memory.copy (local.get $ptr) (local.get $old)
          (select (local.get $old-len) (local.get $new-len)
            (i32.lt_u (local.get $old-len) (local.get $new-len))))))
      (global.set $next (i32.add (local.get $ptr) (local.get $new-len)))
      (local.get $ptr))
    (data (i32.const 0) "ping"))
  (core instance $m (instantiate $memory))
  (alias export $host "begin-send" (func $begin-send))
  (alias core export $m "realloc" (core func $realloc))
  (core func $lowered (canon lower (func $begin-send)
    (memory $m "memory") (realloc $realloc)))
  (core module $guest
    (import "" "memory" (memory 1 1))
    (import "" "begin-send" (func $begin-send (param i64 i32 i32) (result i32)))
    (func $start
      (drop (call $begin-send (i64.const 1) (i32.const 0) (i32.const 4)))
      unreachable)
    (start $start)
    (func (export "dispatch") (param i64 i32 i32)))
  (core instance $g (instantiate $guest (with "" (instance
    (export "memory" (memory $m "memory"))
    (export "begin-send" (func $lowered))))))
  (func (export "dispatch") async (param "attempt" s64) (param "text" string)
    (canon lift (core func $g "dispatch") async
      (memory $m "memory") (realloc $realloc))))
