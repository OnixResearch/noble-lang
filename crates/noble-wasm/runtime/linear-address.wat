;; Ordinary cells occupy [65536, 262144): 4096 immutable 48-byte records.
;; Check the unsigned handle range before subtraction/multiplication.
(func $linear_address (param $h i32) (result i32)
  (if (i32.gt_u (i32.sub (local.get $h) (i32.const 1)) (i32.const 4095))
    (then unreachable))
  (i32.add
    (i32.const 65536)
    (i32.mul (i32.sub (local.get $h) (i32.const 1)) (i32.const 48))))
