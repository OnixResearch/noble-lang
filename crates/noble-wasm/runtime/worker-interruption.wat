;; Typed-worker interruption profile, revision worker-typed/step-4096/interrupt-8.
;; Every charged unit of execution or observation work passes through these
;; ticks, and every eighth one is an interruption point: the host sees the
;; total charged work and answers 0 to continue, 1 to cancel or 2 for a host
;; deadline. Any other answer is a host-selected guest fault.
(global $work_since_check (mut i32) (i32.const 0))
(func $interruption_point (result i32) (local $action i32)
 (global.set $work_since_check (i32.add (global.get $work_since_check) (i32.const 1)))
 (if (i32.lt_u (global.get $work_since_check) (i32.const 8)) (then (return (i32.const 1))))
 (global.set $work_since_check (i32.const 0))
 (local.set $action (call $host_interrupt_check
  (i32.add (global.get $steps) (global.get $reflection_steps))))
 (if (i32.eqz (local.get $action)) (then (return (i32.const 1))))
 (if (i32.eq (local.get $action) (i32.const 1))
  (then (call $fail (i32.const 7)) (return (i32.const 0))))
 (if (i32.eq (local.get $action) (i32.const 2))
  (then (call $fail (i32.const 8)) (return (i32.const 0))))
 unreachable)
(func $runtime_tick (result i32)
 (if (i32.eqz (call $runtime_charge)) (then (return (i32.const 0))))
 (call $interruption_point))
(func $observation_tick (result i32)
 (if (i32.eqz (call $observation_charge)) (then (return (i32.const 0))))
 (call $interruption_point))
