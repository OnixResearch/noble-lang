;; Cells occupy [65536, 262144): 4096 immutable records of exactly 48 bytes.
;; Check the unsigned handle range before subtraction/multiplication can address memory.
(func $linear_address (param $h i32) (result i32)
  (if (i32.gt_u (i32.sub (local.get $h) (i32.const 1)) (i32.const 4095))
    (then unreachable))
  (i32.add
    (i32.const 65536)
    (i32.mul (i32.sub (local.get $h) (i32.const 1)) (i32.const 48))))

;; A current cell has both its expected id and a live kind, never a zero object.
(func $linear_live_address (param $h i32) (result i32)
  (local $p i32)
  (if (i32.gt_u (local.get $h) (global.get $heap_cursor))
    (then unreachable))
  (local.set $p (call $linear_address (local.get $h)))
  (if (i32.ne (i32.load offset=4 (local.get $p)) (local.get $h))
    (then unreachable))
  (if (i32.gt_u
        (i32.sub (i32.load (local.get $p)) (i32.const 1))
        (i32.const 9))
    (then unreachable))
  (local.get $p))

;; Nullable edges point only backward to existing cells, preserving acyclicity.
(func $linear_child (param $child i32) (param $parent i32) (result i32)
  (if (i32.eqz (local.get $child))
    (then (return (i32.const 0))))
  (if (i32.ge_u (local.get $child) (local.get $parent))
    (then unreachable))
  (drop (call $linear_live_address (local.get $child)))
  (local.get $child))

(func $storage_init
  ;; Leave scratch and common accounting untouched; initially this range is empty.
  (memory.fill
    (i32.const 65536)
    (i32.const 0)
    (i32.shl (i32.sub (memory.size) (i32.const 1)) (i32.const 16))))

(func $storage_ensure (param $h i32) (result i32)
  (local $needed i32)
  (local $current i32)
  (if (i32.gt_u (i32.sub (local.get $h) (i32.const 1)) (i32.const 4095))
    (then (return (i32.const 0))))
  ;; ceil((65536 + h * 48) / 65536) is at most four, without i32 overflow.
  (local.set $needed
    (i32.add
      (i32.const 1)
      (i32.shr_u
        (i32.add (i32.mul (local.get $h) (i32.const 48)) (i32.const 65535))
        (i32.const 16))))
  (local.set $current (memory.size))
  (if (i32.ge_u (local.get $current) (local.get $needed))
    (then (return (i32.const 1))))
  ;; reserve charges only after this succeeds; failed growth publishes no cell.
  (i32.ne
    (memory.grow (i32.sub (local.get $needed) (local.get $current)))
    (i32.const -1)))

(func $storage_discard (param $h i32)
  ;; Clear kind and id together. Pages remain allocated; handles are region-private.
  (i64.store (call $linear_live_address (local.get $h)) (i64.const 0)))

(func $new
  (param $kind i32) (param $payload i64)
  (param $a i32) (param $b i32) (param $c i32)
  (param $x i32) (param $y i32) (param $z i32)
  (param $w i32) (param $n i32)
  (result i32)
  (local $h i32)
  (local $p i32)
  (local.set $h (call $reserve))
  ;; In particular, failed reservations must not inspect child handles.
  (if (i32.eqz (local.get $h))
    (then (return (i32.const 0))))
  (local.set $p (call $linear_address (local.get $h)))
  (if (i32.gt_u (i32.sub (local.get $kind) (i32.const 1)) (i32.const 9))
    (then unreachable))
  (local.set $a (call $linear_child (local.get $a) (local.get $h)))
  (local.set $b (call $linear_child (local.get $b) (local.get $h)))
  (local.set $c (call $linear_child (local.get $c) (local.get $h)))
  (i64.store offset=8 (local.get $p) (local.get $payload))
  (i32.store offset=16 (local.get $p) (local.get $a))
  (i32.store offset=20 (local.get $p) (local.get $b))
  (i32.store offset=24 (local.get $p) (local.get $c))
  (i32.store offset=28 (local.get $p) (local.get $x))
  (i32.store offset=32 (local.get $p) (local.get $y))
  (i32.store offset=36 (local.get $p) (local.get $z))
  (i32.store offset=40 (local.get $p) (local.get $w))
  (i32.store offset=44 (local.get $p) (local.get $n))
  ;; Publish the live header only after every immutable field is initialized.
  (i32.store offset=4 (local.get $p) (local.get $h))
  (i32.store (local.get $p) (local.get $kind))
  (local.get $h))

(func $kind (param $h i32) (result i32)
  (i32.load (call $linear_live_address (local.get $h))))

(func $payload (param $h i32) (result i64)
  (i64.load offset=8 (call $linear_live_address (local.get $h))))

(func $a (param $h i32) (result i32)
  (call $linear_child
    (i32.load offset=16 (call $linear_live_address (local.get $h)))
    (local.get $h)))

(func $b (param $h i32) (result i32)
  (call $linear_child
    (i32.load offset=20 (call $linear_live_address (local.get $h)))
    (local.get $h)))

(func $c (param $h i32) (result i32)
  (call $linear_child
    (i32.load offset=24 (call $linear_live_address (local.get $h)))
    (local.get $h)))

(func $x (param $h i32) (result i32)
  (i32.load offset=28 (call $linear_live_address (local.get $h))))

(func $y (param $h i32) (result i32)
  (i32.load offset=32 (call $linear_live_address (local.get $h))))

(func $z (param $h i32) (result i32)
  (i32.load offset=36 (call $linear_live_address (local.get $h))))

(func $w (param $h i32) (result i32)
  (i32.load offset=40 (call $linear_live_address (local.get $h))))

(func $n (param $h i32) (result i32)
  (i32.load offset=44 (call $linear_live_address (local.get $h))))
