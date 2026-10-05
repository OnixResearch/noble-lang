;; Opt-in Text.byte reads one byte without allocating or replacing its Text.
;; -1 is EOF, -2 is a negative offset, -3 is an offset past the end.
(func $op_text_byte
 (local $index i64) (local $text i32) (local $length i64)
 (local $base i64) (local $address i32)
 (if (i32.eqz (call $need (i32.const 2))) (then (return)))
 (if (i32.or
      (i32.ne (call $slot_kind (i32.sub (global.get $sp) (i32.const 1))) (i32.const 1))
      (i32.ne (call $slot_kind (i32.sub (global.get $sp) (i32.const 2))) (i32.const 11)))
  (then (call $fail (i32.const 4)) (return)))
 (local.set $index (call $take))
 (local.set $text (i32.wrap_i64 (call $slot_value (i32.sub (global.get $sp) (i32.const 1)))))
 (local.set $length (i64.extend_i32_u (call $y (local.get $text))))
 (local.set $base (i64.extend_i32_u (call $x (local.get $text))))
 ;; Both x and y are unsigned; check their sum in i64 before any memory load.
 (if (i64.gt_u (i64.add (local.get $base) (local.get $length))
                (i64.shl (i64.extend_i32_u (memory.size)) (i64.const 16)))
  (then (call $fail (i32.const 4)) (return)))
 (if (i64.lt_s (local.get $index) (i64.const 0))
  (then (call $push_i64 (i64.const -2)) (return)))
 (if (i64.gt_u (local.get $index) (local.get $length))
  (then (call $push_i64 (i64.const -3)) (return)))
 (if (i64.eq (local.get $index) (local.get $length))
  (then (call $push_i64 (i64.const -1)) (return)))
 ;; 0 <= index < length, so x + index is in bounds and fits memory32.
 (local.set $address (i32.wrap_i64 (i64.add (local.get $base) (local.get $index))))
 (call $push_i64 (i64.extend_i32_u (i32.load8_u (local.get $address)))))
