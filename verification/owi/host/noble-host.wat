;; The `noble` host module a live Core-Bootstrap subject imports (issue #57).
;;
;; The committed subject `receipts/2026-10-26/subject-module-1.wasm` imports exactly one memory
;; (16 16), one table (16384 16384 funcref) and twenty-five mutable globals from module `noble`:
;; twenty-three i32 counters/pointers and two i64 byte totals. It imports no host function, so a
;; pure-data module satisfies every import and lets a symbolic engine instantiate the subject.
;; Quota limits start at the values in crates/noble-cli/src/core/runtime/abi.json. Counters and
;; pointers start at zero; host.mjs checks the six initial limits against the ABI.
(module
  (memory (export "memory") 16 16)
  (table (export "table") 16384 16384 funcref)
  (global (export "failure") (mut i32) (i32.const 0))
  (global (export "quota_reason") (mut i32) (i32.const 0))
  (global (export "phase") (mut i32) (i32.const 0))
  (global (export "heap_cursor") (mut i32) (i32.const 0))
  (global (export "heap_baseline") (mut i32) (i32.const 0))
  (global (export "allocation_limit") (mut i32) (i32.const 196608))
  (global (export "recipe_limit") (mut i32) (i32.const 256))
  (global (export "depth_limit") (mut i32) (i32.const 256))
  (global (export "operand_limit") (mut i32) (i32.const 2048))
  (global (export "continuation_limit") (mut i32) (i32.const 4096))
  (global (export "step_limit") (mut i32) (i32.const 100000))
  (global (export "allocation_peak") (mut i32) (i32.const 0))
  (global (export "operand_peak") (mut i32) (i32.const 0))
  (global (export "continuation_peak") (mut i32) (i32.const 0))
  (global (export "steps") (mut i32) (i32.const 0))
  (global (export "quote_invocations") (mut i32) (i32.const 0))
  (global (export "sp") (mut i32) (i32.const 0))
  (global (export "cp") (mut i32) (i32.const 0))
  (global (export "observed_program") (mut i32) (i32.const 0))
  (global (export "recipe_count") (mut i32) (i32.const 0))
  (global (export "reflection_steps") (mut i32) (i32.const 0))
  (global (export "rp") (mut i32) (i32.const 0))
  (global (export "generation") (mut i32) (i32.const 0))
  (global (export "allocated_total") (mut i64) (i64.const 0))
  (global (export "released_total") (mut i64) (i64.const 0))
)
