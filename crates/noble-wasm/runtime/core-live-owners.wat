;; Core new_live only: existing 16-page heap, not the 20-page live-slot ABI.
;; Root bytes [57344,61440) are untouched by Core stack/recipe/reflection.
;; One shared cross-instance dispatch guard occupies [61440,61444).
;; Each physical handle has one module-global bit and seven independent host
;; retain counts. Mark bytes reuse the idle recipe scratch [8192,12288).
;; Core source cell edges are immutable and handles increase without physical
;; recycling. Retiring code tombstones unrooted cell headers; every child edge
;; points to a smaller handle. Code spans may be reused only after every
;; reachable Program cell disappears from that span.
(func $core_live_busy (result i32)
 (i32.or (i32.load (i32.const 61440))
  (i32.or (global.get $cp) (global.get $rp))))
(func $core_live_root_address (param $h i32) (result i32)
 (i32.add (i32.const 57344) (i32.sub (local.get $h) (i32.const 1))))
(func $core_live_mark_address (param $h i32) (result i32)
 (i32.add (i32.const 8192) (i32.sub (local.get $h) (i32.const 1))))
(func $core_live_module_root (param $h i32) (result i32)
 (local $address i32) (local $roots i32)
 (if (i32.eqz (call $storage_is_live (local.get $h))) (then (return (i32.const 4))))
 (if (i32.ne (call $kind (local.get $h)) (i32.const 4)) (then (return (i32.const 4))))
 (local.set $address (call $core_live_root_address (local.get $h)))
 (local.set $roots (i32.load8_u (local.get $address)))
 (if (i32.and (local.get $roots) (i32.const 128)) (then (return (i32.const 4))))
 (i32.store8 (local.get $address) (i32.or (local.get $roots) (i32.const 128)))
 (i32.const 0))
(func $core_live_module_rooted (param $h i32) (result i32)
 (if (i32.eqz (call $storage_is_live (local.get $h))) (then (return (i32.const 0))))
 (if (i32.ne (call $kind (local.get $h)) (i32.const 4)) (then (return (i32.const 0))))
 (i32.and (i32.load8_u (call $core_live_root_address (local.get $h))) (i32.const 128)))
(func $core_live_unroot_module (param $h i32)
 (local $address i32)
 (local.set $address (call $core_live_root_address (local.get $h)))
 (i32.store8 (local.get $address)
  (i32.and (i32.load8_u (local.get $address)) (i32.const 127))))
;; An installed adapter can call this module after its originating expression
;; has returned. The trusted host registers a lease before publishing the
;; immutable mapping, then releases it only after the mapping and callbacks
;; are gone. Module globals cannot be dropped while any such lease survives.
;; Both exports return 0 on success and 4 on refusal.
(func (export "retain_adapter_lease") (result i32)
 (if (i32.or (global.get $failure) (call $core_live_busy))
  (then (return (i32.const 4))))
 (if (i32.or (i32.eqz (global.get $core_live_module_installed))
             (i32.ge_u (global.get $core_live_adapter_leases) (i32.const 4096)))
  (then (return (i32.const 4))))
 (global.set $core_live_adapter_leases
  (i32.add (global.get $core_live_adapter_leases) (i32.const 1)))
 (i32.const 0))
(func (export "release_adapter_lease") (result i32)
 (if (i32.or (global.get $failure) (call $core_live_busy))
  (then (return (i32.const 4))))
 (if (i32.or (i32.eqz (global.get $core_live_module_installed))
             (i32.eqz (global.get $core_live_adapter_leases)))
  (then (return (i32.const 4))))
 (global.set $core_live_adapter_leases
  (i32.sub (global.get $core_live_adapter_leases) (i32.const 1)))
 (i32.const 0))
;; Explicit host-owned roots must be retained before the host removes a
;; Program/aggregate from the backend stack or releases its module global.
;; Status 0 succeeds; 4 rejects stale, scalar, excess or unheld roots.
(func $core_live_retain_root (export "retain_code_root") (param $h i32) (result i32)
 (local $address i32) (local $roots i32)
 ;; `live_propose` provisionally sets failure=5 during its trusted host
 ;; callback. Pin the popped Program only during that exact callback;
 ;; a denied proposal is terminal and cannot reopen root mutation.
 (if (i32.and (global.get $failure)
   (i32.eqz (i32.and (i32.eq (global.get $failure) (i32.const 5))
                    (global.get $core_live_proposal_callback_active))))
  (then (return (i32.const 4))))
 (if (i32.eqz (call $storage_is_live (local.get $h))) (then (return (i32.const 4))))
 (if (i32.lt_u (call $kind (local.get $h)) (i32.const 4)) (then (return (i32.const 4))))
 (local.set $address (call $core_live_root_address (local.get $h)))
 (local.set $roots (i32.load8_u (local.get $address)))
 (if (i32.eq (i32.and (local.get $roots) (i32.const 127)) (i32.const 127))
  (then (return (i32.const 4))))
 (i32.store8 (local.get $address) (i32.add (local.get $roots) (i32.const 1)))
 (i32.const 0))
(func (export "release_code_root") (param $h i32) (result i32)
 (local $address i32) (local $roots i32)
 ;; Permit the active proposal callback to undo its provisional root; a
 ;; previously denied proposal cannot release unrelated saved Programs.
 (if (i32.and (global.get $failure)
   (i32.eqz (i32.and (i32.eq (global.get $failure) (i32.const 5))
                    (global.get $core_live_proposal_callback_active))))
  (then (return (i32.const 4))))
 (if (i32.eqz (call $storage_is_live (local.get $h))) (then (return (i32.const 4))))
 (if (i32.lt_u (call $kind (local.get $h)) (i32.const 4)) (then (return (i32.const 4))))
 (local.set $address (call $core_live_root_address (local.get $h)))
 (local.set $roots (i32.load8_u (local.get $address)))
 (if (i32.eqz (i32.and (local.get $roots) (i32.const 127)))
  (then (return (i32.const 4))))
 (i32.store8 (local.get $address) (i32.sub (local.get $roots) (i32.const 1)))
 (i32.const 0))
;; Observe a real typed stack reference and retain it in one host call.
;; Zero refuses; a positive result is the exact backend handle retained.
(func (export "save_stack_code_root") (param $index i32) (result i32)
 (local $tag i32) (local $value i64) (local $h i32)
 (if (i32.or (global.get $failure) (i32.ge_u (local.get $index) (global.get $sp)))
  (then (return (i32.const 0))))
 (local.set $tag (i32.load (i32.mul (local.get $index) (i32.const 16))))
 (if (i32.or (i32.lt_u (local.get $tag) (i32.const 4))
             (i32.gt_u (local.get $tag) (i32.const 16)))
  (then (return (i32.const 0))))
 (local.set $value (i64.load offset=8 (i32.mul (local.get $index) (i32.const 16))))
 (if (i64.ne (i64.shr_u (local.get $value) (i64.const 32)) (i64.const 0))
  (then (return (i32.const 0))))
 (local.set $h (i32.wrap_i64 (local.get $value)))
 (if (i32.eqz (call $storage_is_live (local.get $h))) (then (return (i32.const 0))))
 (if (i32.ne (call $kind (local.get $h)) (local.get $tag)) (then (return (i32.const 0))))
 (if (call $core_live_retain_root (local.get $h)) (then (return (i32.const 0))))
 (local.get $h))
;; The reflection observation global is a real root until explicitly cleared.
(func (export "clear_code_observation") (result i32)
 (if (i32.or (global.get $failure) (call $core_live_busy))
  (then (return (i32.const 4))))
 (global.set $observed_program (i32.const 0))
 (i32.const 0))
;; -1 is refusal, never a zero-owner answer. Stack, registered external roots,
;; per-module Program globals, captured aggregate edges and the observation
;; are all retained; an active frame or malformed/unknown root fails closed.
(func $core_live_code_owners (export "code_owners")
 (param $start i32) (param $length i32) (result i32)
 (local $at i32) (local $h i32) (local $root i32) (local $tag i32)
 (local $value i64) (local $p i32) (local $child i32)
 (local $edge i32) (local $entry i32) (local $count i32)
 (if (i32.or (global.get $failure) (call $core_live_busy))
  (then (return (i32.const -1))))
 (if (i32.or (i32.eqz (local.get $length))
             (i32.gt_u (local.get $length) (i32.const 16384)))
  (then (return (i32.const -1))))
 (if (i32.or (i32.lt_u (local.get $start) (i32.const 4))
             (i32.gt_u (local.get $start)
              (i32.sub (i32.const 16384) (local.get $length))))
  (then (return (i32.const -1))))
 (if (i32.or (i32.gt_u (global.get $heap_cursor) (i32.const 4096))
             (i32.gt_u (global.get $sp) (i32.const 128)))
  (then (return (i32.const -1))))
 (memory.fill (i32.const 8192) (i32.const 0) (i32.const 4096))
 ;; Check the whole root registry, including bytes beyond the current heap.
 (block $roots_done (loop $roots
  (br_if $roots_done (i32.ge_u (local.get $at) (i32.const 4096)))
  (local.set $root (i32.load8_u (i32.add (i32.const 57344) (local.get $at))))
  (if (local.get $root) (then
   (local.set $h (i32.add (local.get $at) (i32.const 1)))
   (if (i32.eqz (call $storage_is_live (local.get $h)))
    (then (return (i32.const -1))))
   (local.set $tag (call $kind (local.get $h)))
   (if (i32.or (i32.lt_u (local.get $tag) (i32.const 4))
      (i32.and (i32.and (local.get $root) (i32.const 128))
               (i32.ne (local.get $tag) (i32.const 4))))
    (then (return (i32.const -1))))
   (i32.store8 (call $core_live_mark_address (local.get $h)) (i32.const 1))))
  (local.set $at (i32.add (local.get $at) (i32.const 1)))
  (br $roots)))
 (local.set $at (i32.const 0))
 (block $stack_done (loop $stack
  (br_if $stack_done (i32.ge_u (local.get $at) (global.get $sp)))
  (local.set $tag (i32.load (i32.mul (local.get $at) (i32.const 16))))
  (if (i32.or (i32.lt_u (local.get $tag) (i32.const 1))
              (i32.gt_u (local.get $tag) (i32.const 16)))
   (then (return (i32.const -1))))
  (if (i32.ge_u (local.get $tag) (i32.const 4)) (then
   (local.set $value (i64.load offset=8
    (i32.mul (local.get $at) (i32.const 16))))
   (if (i64.ne (i64.shr_u (local.get $value) (i64.const 32)) (i64.const 0))
    (then (return (i32.const -1))))
   (local.set $h (i32.wrap_i64 (local.get $value)))
   (if (i32.eqz (call $storage_is_live (local.get $h)))
    (then (return (i32.const -1))))
   (if (i32.ne (call $kind (local.get $h)) (local.get $tag))
    (then (return (i32.const -1))))
   (i32.store8 (call $core_live_mark_address (local.get $h)) (i32.const 1))))
  (local.set $at (i32.add (local.get $at) (i32.const 1)))
  (br $stack)))
 (local.set $h (global.get $observed_program))
 (if (local.get $h) (then
  (if (i32.eqz (call $storage_is_live (local.get $h)))
   (then (return (i32.const -1))))
  (local.set $tag (call $kind (local.get $h)))
  (if (i32.eqz (i32.or (i32.eq (local.get $tag) (i32.const 4))
    (i32.or (i32.eq (local.get $tag) (i32.const 10))
     (i32.or (i32.eq (local.get $tag) (i32.const 14))
      (i32.or (i32.eq (local.get $tag) (i32.const 15))
       (i32.eq (local.get $tag) (i32.const 16)))))))
   (then (return (i32.const -1))))
  (i32.store8 (call $core_live_mark_address (local.get $h)) (i32.const 1))))
 ;; Immutable child handles always decrease, so a reverse pass computes the
 ;; complete transitive closure without allocating a queue or modifying cells.
 (local.set $at (global.get $heap_cursor))
 (block $walk_done (loop $walk
  (br_if $walk_done (i32.eqz (local.get $at)))
  (local.set $h (local.get $at))
  (local.set $at (i32.sub (local.get $at) (i32.const 1)))
  (if (i32.load8_u (call $core_live_mark_address (local.get $h))) (then
   (if (i32.eqz (call $storage_is_live (local.get $h)))
    (then (return (i32.const -1))))
   (local.set $p (call $linear_live_address (local.get $h)))
   (if (i32.eq (call $kind (local.get $h)) (i32.const 4)) (then
    (local.set $entry (i32.load offset=28 (local.get $p)))
    (if (i32.ge_u (local.get $entry) (i32.const 16384))
     (then (return (i32.const -1))))
    (if (i32.and (i32.ge_u (local.get $entry) (local.get $start))
       (i32.lt_u (i32.sub (local.get $entry) (local.get $start))
        (local.get $length)))
     (then (local.set $count (i32.add (local.get $count) (i32.const 1)))))))
   (local.set $edge (i32.const 16))
   (block $edges_done (loop $edges
    (br_if $edges_done (i32.gt_u (local.get $edge) (i32.const 24)))
    (local.set $child (i32.load (i32.add (local.get $p) (local.get $edge))))
    (if (local.get $child) (then
     (if (i32.or (i32.ge_u (local.get $child) (local.get $h))
                 (i32.eqz (call $storage_is_live (local.get $child))))
      (then (return (i32.const -1))))
     (i32.store8 (call $core_live_mark_address (local.get $child)) (i32.const 1))))
    (local.set $edge (i32.add (local.get $edge) (i32.const 4)))
    (br $edges)))))
  (br $walk)))
 (local.get $count))
;; Host calls this only in the trusted retire transaction after checking the
;; exact installed CodeSpan, zero owners and its current shared-table entries,
;; but before clearing them. Invalidate every now-unowned Program cell in the
;; retired span and every unrooted ancestor referring to such a dead cell.
;; An old raw Program or aggregate handle must never be resurrected after
;; that span is reoccupied by a later generation.
;; The complete owner query precedes any mutation, so refusal is atomic.
(func (export "invalidate_code_span")
 (param $start i32) (param $length i32) (result i32)
 (local $at i32) (local $p i32) (local $entry i32)
 (local $child i32) (local $edge i32) (local $dead i32)
 (if (i32.ne (call $core_live_code_owners (local.get $start) (local.get $length))
             (i32.const 0))
  (then (return (i32.const 4))))
 (block $done (loop $scan
  (br_if $done (i32.ge_u (local.get $at) (global.get $heap_cursor)))
  (local.set $p (i32.add (i32.const 65536)
   (i32.mul (local.get $at) (i32.const 48))))
  (if (call $storage_is_live (i32.add (local.get $at) (i32.const 1))) (then
   (local.set $dead (i32.const 0))
   (if (i32.eq (i32.load (local.get $p)) (i32.const 4)) (then
    (local.set $entry (i32.load offset=28 (local.get $p)))
    (if (i32.and (i32.ge_u (local.get $entry) (local.get $start))
        (i32.lt_u (i32.sub (local.get $entry) (local.get $start))
                   (local.get $length)))
     (then (local.set $dead (i32.const 1))))))
   ;; Child handles decrease, so the same ascending pass propagates death
   ;; through every unrooted wrapper without a second mark/scratch region.
   (local.set $edge (i32.const 16))
   (block $edges_done (loop $edges
    (br_if $edges_done (i32.or (local.get $dead)
                     (i32.gt_u (local.get $edge) (i32.const 24))))
    (local.set $child (i32.load (i32.add (local.get $p) (local.get $edge))))
    (if (local.get $child) (then
     (if (i32.or
       (i32.ge_u (local.get $child) (i32.add (local.get $at) (i32.const 1)))
       (i32.eqz (call $storage_is_live (local.get $child))))
      (then (local.set $dead (i32.const 1))))))
    (local.set $edge (i32.add (local.get $edge) (i32.const 4)))
    (br $edges)))
   (if (local.get $dead) (then (i64.store (local.get $p) (i64.const 0))))))
  (local.set $at (i32.add (local.get $at) (i32.const 1)))
  (br $scan)))
 (i32.const 0))
