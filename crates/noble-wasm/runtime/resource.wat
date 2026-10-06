;; An authorized host root setup injects a nominal resource, never a raw
;; resource or a forgeable integer. The host binds the issued ordinal to the
;; exact logical input position and registered versioned schema before any
;; guest cell is allocated. Ordinary source code has no constructor for kind 17.
(func (export "push_nominal_resource")
 (param $logical_input_position i32) (param $issued_ordinal i32)
 (param $resource_kind i32) (param $module_lo i32) (param $module_hi i32)
 (param $nominal_ordinal i32) (result i32) (local $resource i32)
 (if (global.get $failure) (then (return (global.get $failure))))
 (if (i32.or (global.get $phase) (global.get $cp))
  (then (call $fail (i32.const 4)) (return (global.get $failure))))
 ;; An import throw leaves failure poisoned. A rejected binding is an invalid
 ;; root input, not permission to attempt another schema or logical position.
 (global.set $failure (i32.const 5))
 (if (i32.ne
   (call $host_resource_bind_validate
    (local.get $logical_input_position) (local.get $issued_ordinal)
    (local.get $resource_kind) (local.get $module_lo)
    (local.get $module_hi) (local.get $nominal_ordinal))
   (i32.const 1))
  (then (global.set $failure (i32.const 4)) (return (global.get $failure))))
 (global.set $failure (i32.const 0))
 (local.set $resource (call $new (i32.const 17)
  (i64.extend_i32_u (local.get $issued_ordinal))
  (i32.const 0) (i32.const 0) (i32.const 0)
  (local.get $resource_kind) (i32.const 0) (i32.const 0)
  (i32.const 0) (i32.const 0)))
 (if (global.get $failure) (then (return (global.get $failure))))
 (call $push_ref (call $new (i32.const 12) (i64.const 0)
  (local.get $resource) (i32.const 0) (i32.const 0)
  (local.get $module_lo) (local.get $module_hi)
  (local.get $nominal_ordinal) (i32.const 1) (i32.const 0)))
 (global.get $failure))
