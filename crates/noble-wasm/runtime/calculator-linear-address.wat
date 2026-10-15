;; Calculator cells 1..4096 retain the ordinary region [65536, 262144).
;; Cells 4097..6826 use only the unused 131072-byte gap [393216, 524288).
;; Cells 6827..72362 use only the growable arena [1048576, 4194304).
;; Dynamic Text ends at 393216; static Text begins at 524288.
;; Host input scratch lives in [983040,1048576), never in this cell region.
(func $linear_address (param $h i32) (result i32)
  (if (i32.gt_u (i32.sub (local.get $h) (i32.const 1)) (i32.const 72361))
    (then unreachable))
  (if (result i32) (i32.le_u (local.get $h) (i32.const 4096))
    (then (i32.add
      (i32.const 65536)
      (i32.mul (i32.sub (local.get $h) (i32.const 1)) (i32.const 48))))
    (else
      (if (result i32) (i32.le_u (local.get $h) (i32.const 6826))
        (then (i32.add
          (i32.const 393216)
          (i32.mul (i32.sub (local.get $h) (i32.const 4097)) (i32.const 48))))
        (else (i32.add
          (i32.const 1048576)
          (i32.mul (i32.sub (local.get $h) (i32.const 6827)) (i32.const 48))))))))
