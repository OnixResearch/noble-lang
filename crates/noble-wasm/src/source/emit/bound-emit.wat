;; Private application-supplied immutable adapter slot. Unlike Core test.emit this
;; exact bound operation consumes Text without pushing historical Unit.
(func $op_emit_bound (param $slot i32) (local $text i32)
 (local.set $text (i32.wrap_i64 (call $pop_kind (i32.const 11))))
 (if (global.get $failure) (then (return)))
 (global.set $failure (i32.const 5))
 (if (i32.eqz (call $host_emit_bound
    (call $x (local.get $text)) (call $y (local.get $text)) (local.get $slot)))
  (then (global.set $failure (i32.const 0)))))
