;; Explicit disposal for a compile-only backend experiment after the host has
;; recorded the report, stack and request prefix. Preserve failure/phase and
;; never roll back host effects.
(func (export "cleanup") (result i32) (local $at i32)
 (call $cleanup_internal)
 (global.set $sp (i32.const 0)) (global.set $rp (i32.const 0))
 (global.set $observed_program (i32.const 0)) (global.set $recipe_count (i32.const 0))
 (loop $clear
  (i64.store (local.get $at) (i64.const 0))
  (local.set $at (i32.add (local.get $at) (i32.const 8)))
  (br_if $clear (i32.lt_u (local.get $at) (i32.const 61440))))
 (call $rollback (global.get $heap_baseline))
 (call $live_bytes))
