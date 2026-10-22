;; Private application-supplied immutable adapter slot. The guest learns no
;; operation outcome: status 0 means the host recorded the request (whether it
;; denied, completed or lost track of it); any other status ends the invocation.
(func $op_send_bound (param $slot i32) (local $text i32)
 (local.set $text (i32.wrap_i64 (call $pop_kind (i32.const 11))))
 (if (global.get $failure) (then (return)))
 (global.set $failure (i32.const 5))
 (if (i32.eqz (call $host_send_bound
    (call $x (local.get $text)) (call $y (local.get $text)) (local.get $slot)))
  (then (global.set $failure (i32.const 0)))))
