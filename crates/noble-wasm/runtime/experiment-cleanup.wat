;; Explicit disposal for a compile-only backend experiment after the host has
;; recorded the report, stack and request prefix. Preserve failure/phase and
;; never roll back host effects.
(func (export "cleanup") (result i32)
 (call $cleanup_internal)
 (i32.store (i32.const 57352) (i32.const 0))
 (global.set $sp (i32.const 0)) (global.set $rp (i32.const 0))
 (global.set $observed_program (i32.const 0)) (global.set $recipe_count (i32.const 0))
 (call $rollback (global.get $heap_baseline))
 (call $live_bytes))
