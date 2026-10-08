;; Selected pure declared-text I64 ordering; Core and standalone cursor do not emit it.
(func $op_i64_less (local $right i64) (local $left i64)
 (local.set $right (call $pop_kind (i32.const 1)))
 (local.set $left (call $pop_kind (i32.const 1)))
 (if (global.get $failure) (then (return)))
 (call $push_bool (i64.lt_s (local.get $left) (local.get $right))))
