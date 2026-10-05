;; Only the opt-in Live-Slot-Design module includes these host-facing exports.
(func (export "begin_live")
 (param $allocation i32) (param $recipes i32) (param $depth i32)
 (param $operands i32) (param $continuations i32) (param $step_count i32)
 (result i32)
 (if (global.get $failure) (then (return (global.get $failure))))
 (if (i32.or (call $storage_busy) (i32.load (i32.const 1050624)))
  (then (call $fail (i32.const 4)) (return (global.get $failure))))
 ;; First root bindings may allocate host-issued resource cells before submit.
 ;; Initialize the private heap here, not after those cells are pushed.
 (if (i32.and (i32.eqz (global.get $generation))
              (i32.eqz (global.get $heap_cursor)))
  (then (call $storage_init)))
 (call $live_frame_reset)
 (call $begin_live_base
  (local.get $allocation) (local.get $recipes) (local.get $depth)
  (local.get $operands) (local.get $continuations) (local.get $step_count)))
(func (export "bind_live_ref") (param $position i32) (param $ordinal i32) (result i32)
 (if (global.get $failure) (then (return (global.get $failure))))
 (if (i32.or (global.get $cp) (global.get $phase))
  (then (call $fail (i32.const 4)) (return (global.get $failure))))
 ;; Validate root provenance, exact logical input type/position, and one-time
 ;; issue at the host boundary. Raw integer positions and ordinals are inert.
 (global.set $failure (i32.const 5))
 (if (i32.eq (call $host_live_bind_validate
                (local.get $position) (local.get $ordinal)) (i32.const 1))
  (then (global.set $failure (i32.const 0))
        (call $live_root_bind (local.get $position) (local.get $ordinal)))
  (else (global.set $failure (i32.const 4))))
 (global.get $failure))
(func (export "retain_target") (param $handle i32) (result i32)
 (if (global.get $cp) (then (return (i32.const 4))))
 (call $storage_retain_target (local.get $handle)))
(func (export "release_target") (param $handle i32) (result i32)
 (local $status i32)
 (if (global.get $cp) (then (return (i32.const 4))))
 (local.set $status (call $storage_retire_target (local.get $handle)))
 (if (local.get $status) (then (return (local.get $status))))
 (call $storage_collect))
(func (export "collect_unowned") (result i32)
 (call $storage_collect))
;; The host must use this instead of setting the imported stack height to
;; zero: each physical operand reference has to be released exactly once.
(func (export "clear_stack") (result i32)
 (if (i32.or (global.get $failure)
   (i32.or (call $storage_busy) (i32.load (i32.const 1050624))))
  (then (return (i32.const 4))))
 (block $done (loop $pop
  (br_if $done (i32.eqz (global.get $sp)))
  (drop (call $take))
  (br $pop)))
 (call $storage_collect))
;; A saved owner is created only by observing the actual backend stack slot.
;; Its token includes the physical cell's epoch, not a host-declared P value.
(func (export "save_stack_program") (param $index i32) (result i32)
 (call $storage_save_stack_program (local.get $index)))
(func (export "release_saved_program") (param $handle i32) (result i32)
 (call $storage_release_saved_program (local.get $handle)))
;; Return -1 for any busy/invalid query. Zero is the only no-owner result.
(func (export "code_owners") (param $start i32) (param $length i32) (result i32)
 (call $storage_code_owners (local.get $start) (local.get $length)))
