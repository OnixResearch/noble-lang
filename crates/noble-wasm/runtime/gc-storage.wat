;; Fields are immutable; child references preserve the actual GC object graph.
(rec
  (type $gc_cell (struct
    (field $kind i32)
    (field $id i32)
    (field $payload i64)
    (field $a (ref null $gc_cell))
    (field $b (ref null $gc_cell))
    (field $c (ref null $gc_cell))
    (field $x i32)
    (field $y i32)
    (field $z i32)
    (field $w i32)
    (field $n i32))))
(type $gc_roots (array (mut (ref null $gc_cell))))
(global $gc_root_table (mut (ref null $gc_roots)) (ref.null $gc_roots))

;; Handle h occupies slot h-1; zero is a nullable child, never a root handle.
(func $gc_cell_at (param $h i32) (result (ref $gc_cell))
  (ref.as_non_null
    (array.get $gc_roots
      (ref.as_non_null (global.get $gc_root_table))
      (i32.sub (local.get $h) (i32.const 1)))))

(func $gc_child_at (param $h i32) (result (ref null $gc_cell))
  (if (result (ref null $gc_cell)) (i32.eqz (local.get $h))
    (then (ref.null $gc_cell))
    (else (call $gc_cell_at (local.get $h)))))

(func $gc_handle (param $cell (ref null $gc_cell)) (result i32)
  (if (result i32) (ref.is_null (local.get $cell))
    (then (i32.const 0))
    (else (struct.get $gc_cell $id (ref.as_non_null (local.get $cell))))))

(func $storage_init
  (if (i32.eqz (ref.is_null (global.get $gc_root_table)))
    (then unreachable))
  (global.set $gc_root_table
    (array.new $gc_roots (ref.null $gc_cell) (i32.const 4096))))

(func $storage_ensure (param $h i32) (result i32)
  (i32.lt_u
    (i32.sub (local.get $h) (i32.const 1))
    (array.len (ref.as_non_null (global.get $gc_root_table)))))

;; Dropping a root does not promise immediate collection or physical reclamation.
(func $storage_discard (param $h i32)
  (drop (call $gc_cell_at (local.get $h)))
  (array.set $gc_roots
    (ref.as_non_null (global.get $gc_root_table))
    (i32.sub (local.get $h) (i32.const 1))
    (ref.null $gc_cell)))

;; Common reserve charges logical 48-byte cells, not engine-defined GC bytes.
;; Root-array/struct allocation can fail at the engine level; ensure is not an OOM probe.
(func $new
  (param $kind i32) (param $payload i64)
  (param $a i32) (param $b i32) (param $c i32)
  (param $x i32) (param $y i32) (param $z i32)
  (param $w i32) (param $n i32)
  (result i32)
  (local $h i32)
  (local.set $h (call $reserve))
  ;; A failed reservation must not dereference even a malformed child handle.
  (if (i32.eqz (local.get $h)) (then (return (i32.const 0))))
  (array.set $gc_roots
    (ref.as_non_null (global.get $gc_root_table))
    (i32.sub (local.get $h) (i32.const 1))
    (struct.new $gc_cell
      (local.get $kind)
      (local.get $h)
      (local.get $payload)
      (call $gc_child_at (local.get $a))
      (call $gc_child_at (local.get $b))
      (call $gc_child_at (local.get $c))
      (local.get $x)
      (local.get $y)
      (local.get $z)
      (local.get $w)
      (local.get $n)))
  (local.get $h))

;; Bounds checks and ref.as_non_null trap on absent or discarded private cells.
(func $kind (param $h i32) (result i32)
  (struct.get $gc_cell $kind (call $gc_cell_at (local.get $h))))

(func $payload (param $h i32) (result i64)
  (struct.get $gc_cell $payload (call $gc_cell_at (local.get $h))))

(func $a (param $h i32) (result i32)
  (call $gc_handle (struct.get $gc_cell $a (call $gc_cell_at (local.get $h)))))

(func $b (param $h i32) (result i32)
  (call $gc_handle (struct.get $gc_cell $b (call $gc_cell_at (local.get $h)))))

(func $c (param $h i32) (result i32)
  (call $gc_handle (struct.get $gc_cell $c (call $gc_cell_at (local.get $h)))))

(func $x (param $h i32) (result i32)
  (struct.get $gc_cell $x (call $gc_cell_at (local.get $h))))

(func $y (param $h i32) (result i32)
  (struct.get $gc_cell $y (call $gc_cell_at (local.get $h))))

(func $z (param $h i32) (result i32)
  (struct.get $gc_cell $z (call $gc_cell_at (local.get $h))))

(func $w (param $h i32) (result i32)
  (struct.get $gc_cell $w (call $gc_cell_at (local.get $h))))

(func $n (param $h i32) (result i32)
  (struct.get $gc_cell $n (call $gc_cell_at (local.get $h))))
