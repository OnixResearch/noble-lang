;; Calculator-v1 only. The function table reserves slot 4 for the loop
;; continuation. LIFO draining runs a callback and all of its nested work
;; before this continuation consumes its returned Bool; parent work stays below.
(func $op_iterate (local $callback i32)
 (local.set $callback (i32.wrap_i64 (call $pop_kind (i32.const 4))))
 (if (global.get $failure) (then (return)))
 (call $enqueue (i32.const 4) (local.get $callback))
 (call $enqueue_program (local.get $callback)))
(func $iterate_resume (type $entry) (param $callback i32) (local $again i32)
 (local.set $again (i32.wrap_i64 (call $pop_kind (i32.const 2))))
 (if (global.get $failure) (then (return)))
 (if (local.get $again) (then
  (call $enqueue (i32.const 4) (local.get $callback))
  (call $enqueue_program (local.get $callback)))))

;; An adjacent, non-scheduling calculator operation can resume in its own
;; compiled entry. Preserve the dispatch tick before popping its already
;; enqueued continuation, including quota failure and peak accounting.
(func $inline_step (result i32) (local $address i32)
 (if (i32.eqz (call $runtime_tick)) (then (return (i32.const 0))))
 (global.set $cp (i32.sub (global.get $cp) (i32.const 1)))
 (local.set $address (i32.add (i32.const 4096) (i32.mul (global.get $cp) (i32.const 8))))
 (i64.store (local.get $address) (i64.const 0))
 (i32.const 1))

(func $op_calc_compare (local $right i64) (local $left i64)
 (local.set $right (call $pop_kind (i32.const 1)))
 (local.set $left (call $pop_kind (i32.const 1)))
 (if (global.get $failure) (then (return)))
 (call $push_i64
  (if (result i64) (i64.lt_s (local.get $left) (local.get $right))
   (then (i64.const -1))
   (else (if (result i64) (i64.gt_s (local.get $left) (local.get $right))
    (then (i64.const 1)) (else (i64.const 0)))))))
(func $op_calc_div_rem (local $divisor i64) (local $dividend i64)
 (local.set $divisor (call $pop_kind (i32.const 1)))
 (local.set $dividend (call $pop_kind (i32.const 1)))
 (if (global.get $failure) (then (return)))
 (if (i32.or (i64.lt_s (local.get $dividend) (i64.const 0))
              (i64.le_s (local.get $divisor) (i64.const 0)))
  (then (call $push_i64 (i64.const 0))
        (call $push_i64 (i64.const 0))
        (call $push_bool (i32.const 0))
        (return)))
 (call $push_i64 (i64.div_u (local.get $dividend) (local.get $divisor)))
 (call $push_i64 (i64.rem_u (local.get $dividend) (local.get $divisor)))
 (call $push_bool (i32.const 1)))

(func $op_text_length (local $text i32) (local $base i64) (local $length i64)
 (if (i32.eqz (call $need (i32.const 1))) (then (return)))
 (if (i32.ne (call $slot_kind (i32.sub (global.get $sp) (i32.const 1))) (i32.const 11))
  (then (call $fail (i32.const 4)) (return)))
 (local.set $text (i32.wrap_i64 (call $slot_value (i32.sub (global.get $sp) (i32.const 1)))))
 (local.set $base (i64.extend_i32_u (call $x (local.get $text))))
 (local.set $length (i64.extend_i32_u (call $y (local.get $text))))
 (if (i64.gt_u (i64.add (local.get $base) (local.get $length))
                (i64.shl (i64.extend_i32_u (memory.size)) (i64.const 16)))
  (then (call $fail (i32.const 4)) (return)))
 (call $push_i64 (local.get $length)))

;; Scratch [983040,1048576) carries an untrusted host Text input or one
;; ephemeral observed signature; host Text is copied immediately before the
;; next observation can reuse it. Literal source segments occupy only
;; [524288,983040); expanded calculator cells use [393216,524288);
;; immutable dynamic Text occupies [262144,393216).
;; The cursor word at 65528 lives in imported memory and
;; survives instantiation of the next compiled module in the same session.
(func $calc_reserve_text (param $length i32) (result i32)
 (local $address i32)
 (if (global.get $failure) (then (return (i32.const 0))))
 (local.set $address (i32.load (i32.const 65528)))
 (if (i32.eqz (local.get $address)) (then (local.set $address (i32.const 262144))))
 (if (i32.or (i32.lt_u (local.get $address) (i32.const 262144))
             (i32.gt_u (local.get $address) (i32.const 393216)))
  (then (call $fail (i32.const 4)) (return (i32.const 0))))
 (if (i32.gt_u (local.get $length) (i32.sub (i32.const 393216) (local.get $address)))
  (then (call $quota (i32.const 1)) (return (i32.const 0))))
 (i32.store (i32.const 65528) (i32.add (local.get $address) (local.get $length)))
 (local.get $address))
(func (export "push_text") (param $address i32) (param $length i32) (result i32)
 (local $destination i32)
 (if (global.get $failure) (then (return (global.get $failure))))
 (if (i32.or (i32.ne (local.get $address) (i32.const 983040))
             (i32.gt_u (local.get $length) (i32.const 65536)))
  (then (call $fail (i32.const 4)) (return (global.get $failure))))
 (local.set $destination (call $calc_reserve_text (local.get $length)))
 (if (global.get $failure) (then (return (global.get $failure))))
 (memory.copy (local.get $destination) (local.get $address) (local.get $length))
 (call $push_ref (call $text (local.get $destination) (local.get $length)))
 (global.get $failure))

;; Traverse once to validate/count, once to write: allocate one Text cell,
;; never one cell or copied Text per byte. Input list order is output order.
(func $op_text_from_ascii
 (local $list i32) (local $next i32) (local $head i32) (local $count i32)
 (local $address i32) (local $index i32) (local $byte i64) (local $valid i32)
 (if (i32.eqz (call $need (i32.const 1))) (then (return)))
 (local.set $valid (i32.const 1))
 (local.set $list (i32.wrap_i64 (call $take)))
 (local.set $next (local.get $list))
 (block $validated (loop $check
  (br_if $validated (global.get $failure))
  (if (i32.eq (call $kind (local.get $next)) (i32.const 7)) (then (br $validated)))
  (if (i32.ne (call $kind (local.get $next)) (i32.const 6))
   (then (local.set $valid (i32.const 0)) (br $validated)))
  (if (i32.eqz (call $runtime_tick)) (then (br $validated)))
  (local.set $head (call $a (local.get $next)))
  (if (i32.ne (call $kind (local.get $head)) (i32.const 1))
   (then (local.set $valid (i32.const 0)) (br $validated)))
  (local.set $byte (call $payload (local.get $head)))
  (if (i64.gt_u (local.get $byte) (i64.const 127))
   (then (local.set $valid (i32.const 0)) (br $validated)))
  (local.set $count (i32.add (local.get $count) (i32.const 1)))
  (local.set $next (call $b (local.get $next)))
  (br $check)))
 (if (global.get $failure) (then (return)))
 (if (i32.eqz (local.get $valid))
  (then (call $push_ref (call $text (i32.const 262144) (i32.const 0)))
        (call $push_bool (i32.const 0)) (return)))
 (local.set $address (call $calc_reserve_text (local.get $count)))
 (if (global.get $failure) (then (return)))
 (local.set $next (local.get $list))
 (block $written (loop $write
  (br_if $written (global.get $failure))
  (br_if $written (i32.eq (call $kind (local.get $next)) (i32.const 7)))
  (if (i32.eqz (call $runtime_tick)) (then (br $written)))
  (i32.store8 (i32.add (local.get $address) (local.get $index))
   (i32.wrap_i64 (call $payload (call $a (local.get $next)))))
  (local.set $index (i32.add (local.get $index) (i32.const 1)))
  (local.set $next (call $b (local.get $next)))
  (br $write)))
 (if (global.get $failure) (then (return)))
 (call $push_ref (call $text (local.get $address) (local.get $count)))
 (call $push_bool (i32.const 1)))

;; Calculator-v1 only: order-preserving sliding compaction of the immutable
;; cell arena, called by the engine before `begin`, while no Wasm frame or
;; continuation is live (cp = 0). The operand slots are the only roots.
;; Handle-bearing fields are a/b/c, which are strictly backward. One descending
;; pass therefore marks the complete reachable set. The marked set keeps its
;; relative order, so every edge stays backward after renaming. Forwarding
;; handles are kept in the id word until every edge and root is rewritten.
;; Then cells slide down, dynamic Text slides with them, and the vacated tail
;; is zeroed so that a stale handle fails liveness. Compiled bodies read their
;; quotation and callee Program handles from instance-private globals that
;; another instance cannot rewrite. If any reachable Program runs compiled code
;; (table entry >= 5, beyond the core entries), nothing moves. A malformed
;; arena fails closed with status 4, which terminates the session.
;; The same closure answers the read-only `compiled_programs_reachable` and
;; `programs_reachable` queries.
(func $collect_mark (param $h i32) (local $p i32)
 (local.set $p (call $linear_address (local.get $h)))
 (i32.store (local.get $p) (i32.or (i32.load (local.get $p)) (i32.const 256))))
(func $collect_child (param $child i32) (param $parent i32) (result i32)
 (if (i32.eqz (local.get $child)) (then (return (i32.const 1))))
 (if (i32.ge_u (local.get $child) (local.get $parent)) (then (return (i32.const 0))))
 (if (i32.ne (i32.load offset=4 (call $linear_address (local.get $child))) (local.get $child))
  (then (return (i32.const 0))))
 (call $collect_mark (local.get $child))
 (i32.const 1))
(func $collect_forward (param $child i32) (result i32)
 (if (result i32) (local.get $child)
  (then (i32.load offset=4 (call $linear_address (local.get $child))))
  (else (i32.const 0))))
(func $collect_unmark (param $limit i32) (local $h i32) (local $p i32)
 (local.set $h (i32.const 1))
 (block $done (loop $next
  (br_if $done (i32.gt_u (local.get $h) (local.get $limit)))
  (local.set $p (call $linear_address (local.get $h)))
  (i32.store (local.get $p) (i32.and (i32.load (local.get $p)) (i32.const 255)))
  (local.set $h (i32.add (local.get $h) (i32.const 1)))
  (br $next))))
;; The boundary's dynamic Text end, or 0 when a continuation is live or the
;; operand height, cell cursor or Text cursor is out of bounds.
(func $collect_text_end (result i32) (local $text_end i32)
 (if (i32.or (global.get $cp) (i32.gt_u (global.get $sp) (i32.const 128)))
  (then (return (i32.const 0))))
 (if (i32.gt_u (global.get $heap_cursor) (global.get $cell_limit))
  (then (return (i32.const 0))))
 (local.set $text_end (i32.load (i32.const 65528)))
 (if (i32.eqz (local.get $text_end)) (then (return (i32.const 262144))))
 (if (i32.or (i32.lt_u (local.get $text_end) (i32.const 262144))
             (i32.gt_u (local.get $text_end) (i32.const 393216)))
  (then (return (i32.const 0))))
 (local.get $text_end))
;; Mark the closure of the operand roots over handles 1..$old. Returns 0 with
;; that closure marked and no compiled code reachable; 1 when a reachable
;; Program runs compiled code (table entry >= 5); 4 on a malformed root, edge
;; or dynamic Text order. A nonzero result leaves no mark behind.
(func $collect_closure (param $old i32) (param $text_end i32) (result i32)
 (local $h i32) (local $p i32) (local $kind i32) (local $slot i32) (local $tag i32)
 (local $raw i64) (local $value i32) (local $upper i32) (local $x i32) (local $y i32)
 ;; Roots: every boxed operand slot names a live cell of exactly its tag.
 (block $roots (loop $root
  (br_if $roots (i32.ge_u (local.get $slot) (global.get $sp)))
  (local.set $tag (i32.load (i32.mul (local.get $slot) (i32.const 16))))
  (if (i32.ge_u (local.get $tag) (i32.const 4)) (then
   (local.set $raw (i64.load offset=8 (i32.mul (local.get $slot) (i32.const 16))))
   (local.set $value (i32.wrap_i64 (local.get $raw)))
   (if (i32.or (i64.gt_u (local.get $raw) (i64.const 4294967295))
        (i32.or (i32.eqz (local.get $value)) (i32.gt_u (local.get $value) (local.get $old))))
    (then (call $collect_unmark (local.get $old)) (return (i32.const 4))))
   (local.set $p (call $linear_address (local.get $value)))
   (if (i32.or (i32.ne (i32.load offset=4 (local.get $p)) (local.get $value))
        (i32.ne (i32.and (i32.load (local.get $p)) (i32.const 255)) (local.get $tag)))
    (then (call $collect_unmark (local.get $old)) (return (i32.const 4))))
   (call $collect_mark (local.get $value))))
  (local.set $slot (i32.add (local.get $slot) (i32.const 1)))
  (br $root)))
 ;; Descending closure. Dynamic Text extents must be disjoint and increase
 ;; with handle order, so they can slide without overtaking each other.
 (local.set $upper (local.get $text_end))
 (local.set $h (local.get $old))
 (block $marked (loop $mark
  (br_if $marked (i32.eqz (local.get $h)))
  (local.set $p (call $linear_address (local.get $h)))
  (local.set $kind (i32.load (local.get $p)))
  (if (i32.and (local.get $kind) (i32.const 256)) (then
   (local.set $kind (i32.and (local.get $kind) (i32.const 255)))
   (if (i32.or (i32.gt_u (i32.sub (local.get $kind) (i32.const 1)) (i32.const 15))
        (i32.ne (i32.load offset=4 (local.get $p)) (local.get $h)))
    (then (call $collect_unmark (local.get $old)) (return (i32.const 4))))
   (if (i32.and (i32.eq (local.get $kind) (i32.const 4))
        (i32.ge_u (i32.load offset=28 (local.get $p)) (i32.const 5)))
    (then (call $collect_unmark (local.get $old)) (return (i32.const 1))))
   (if (i32.eq (local.get $kind) (i32.const 11)) (then
    (local.set $x (i32.load offset=28 (local.get $p)))
    (local.set $y (i32.load offset=32 (local.get $p)))
    (if (i32.and (i32.ne (local.get $y) (i32.const 0))
         (i32.and (i32.ge_u (local.get $x) (i32.const 262144))
                  (i32.lt_u (local.get $x) (i32.const 393216)))) (then
     (if (i32.or (i32.gt_u (local.get $x) (local.get $upper))
          (i32.gt_u (local.get $y) (i32.sub (local.get $upper) (local.get $x))))
      (then (call $collect_unmark (local.get $old)) (return (i32.const 4))))
     (local.set $upper (local.get $x))))))
   (if (i32.eqz (i32.and (call $collect_child (i32.load offset=16 (local.get $p)) (local.get $h))
        (i32.and (call $collect_child (i32.load offset=20 (local.get $p)) (local.get $h))
         (call $collect_child (i32.load offset=24 (local.get $p)) (local.get $h)))))
    (then (call $collect_unmark (local.get $old)) (return (i32.const 4))))))
  (local.set $h (i32.sub (local.get $h) (i32.const 1)))
  (br $mark)))
 (i32.const 0))
;; Read-only boundary query, called by the engine after a normal submission.
;; 0 only when the boundary is well formed and no Program reachable from the
;; operand stack runs compiled code: no later instance can then reach a table
;; entry of an earlier module, so the compiler may restart the next module's
;; table initialization at the fixed base. Anything else answers 1, which
;; keeps every committed compiled-code slot reserved.
(func (export "compiled_programs_reachable") (result i32) (local $text_end i32)
 (if (global.get $failure) (then (return (i32.const 1))))
 (local.set $text_end (call $collect_text_end))
 (if (i32.eqz (local.get $text_end)) (then (return (i32.const 1))))
 (if (call $collect_closure (global.get $heap_cursor) (local.get $text_end))
  (then (return (i32.const 1))))
 (call $collect_unmark (global.get $heap_cursor))
 (i32.const 0))
;; Signature IDs live only in Program cells (stack interface), Syntax cells and
;; recipe nodes (literal schemas and invocation witnesses), and recipe nodes are
;; reachable only through a Program or Syntax cell. Returns 1 when the marked
;; closure over handles 1..$old holds any of them.
(func $collect_signature_holders (param $old i32) (result i32) (local $h i32) (local $kind i32)
 (local.set $h (i32.const 1))
 (block $done (loop $next
  (br_if $done (i32.gt_u (local.get $h) (local.get $old)))
  (local.set $kind (i32.load (call $linear_address (local.get $h))))
  (if (i32.and (local.get $kind) (i32.const 256)) (then
   (local.set $kind (i32.and (local.get $kind) (i32.const 255)))
   (if (i32.or (i32.or (i32.eq (local.get $kind) (i32.const 4)) (i32.eq (local.get $kind) (i32.const 10)))
               (i32.or (i32.eq (local.get $kind) (i32.const 8)) (i32.eq (local.get $kind) (i32.const 9))))
    (then (return (i32.const 1))))))
  (local.set $h (i32.add (local.get $h) (i32.const 1)))
  (br $next)))
 (i32.const 0))
;; Read-only boundary query, called by the engine after a normal submission
;; whose compiled_programs_reachable answer was 0. 0 only when the boundary is
;; well formed and no Program, Syntax or recipe cell is reachable from the
;; operand stack: no later module can then observe or run a signature ID of an
;; earlier one, so the compiler may restart its signature pool and published
;; descriptors from ID 0. Anything else answers 1.
(func (export "programs_reachable") (result i32) (local $text_end i32) (local $held i32)
 (if (global.get $failure) (then (return (i32.const 1))))
 (local.set $text_end (call $collect_text_end))
 (if (i32.eqz (local.get $text_end)) (then (return (i32.const 1))))
 (if (call $collect_closure (global.get $heap_cursor) (local.get $text_end))
  (then (return (i32.const 1))))
 (local.set $held (call $collect_signature_holders (global.get $heap_cursor)))
 (call $collect_unmark (global.get $heap_cursor))
 (local.get $held))
(func (export "collect") (result i32)
 (local $old i32) (local $h i32) (local $p i32) (local $kind i32) (local $slot i32)
 (local $count i32) (local $status i32) (local $text_end i32) (local $text i32)
 (local $x i32) (local $y i32) (local $dst i32)
 (if (global.get $failure) (then (return (global.get $failure))))
 (local.set $text_end (call $collect_text_end))
 (if (i32.eqz (local.get $text_end))
      (then (call $fail (i32.const 4)) (return (global.get $failure))))
 (local.set $old (global.get $heap_cursor))
 (local.set $status (call $collect_closure (local.get $old) (local.get $text_end)))
 (if (i32.eq (local.get $status) (i32.const 1)) (then (return (i32.const 0))))
 (if (local.get $status)
      (then (call $fail (i32.const 4)) (return (global.get $failure))))
 ;; Assign dense forwarding handles in the original order.
 (local.set $h (i32.const 1))
 (block $numbered (loop $number
  (br_if $numbered (i32.gt_u (local.get $h) (local.get $old)))
  (local.set $p (call $linear_address (local.get $h)))
  (if (i32.and (i32.load (local.get $p)) (i32.const 256)) (then
   (local.set $count (i32.add (local.get $count) (i32.const 1)))
   (i32.store offset=4 (local.get $p) (local.get $count))))
  (local.set $h (i32.add (local.get $h) (i32.const 1)))
  (br $number)))
 ;; Rename every edge and root before anything moves.
 (local.set $h (i32.const 1))
 (block $renamed (loop $rename
  (br_if $renamed (i32.gt_u (local.get $h) (local.get $old)))
  (local.set $p (call $linear_address (local.get $h)))
  (if (i32.and (i32.load (local.get $p)) (i32.const 256)) (then
   (i32.store offset=16 (local.get $p) (call $collect_forward (i32.load offset=16 (local.get $p))))
   (i32.store offset=20 (local.get $p) (call $collect_forward (i32.load offset=20 (local.get $p))))
   (i32.store offset=24 (local.get $p) (call $collect_forward (i32.load offset=24 (local.get $p))))))
  (local.set $h (i32.add (local.get $h) (i32.const 1)))
  (br $rename)))
 (local.set $slot (i32.const 0))
 (block $rooted (loop $reroot
  (br_if $rooted (i32.ge_u (local.get $slot) (global.get $sp)))
  (if (i32.ge_u (i32.load (i32.mul (local.get $slot) (i32.const 16))) (i32.const 4))
   (then (i64.store offset=8 (i32.mul (local.get $slot) (i32.const 16))
    (i64.extend_i32_u (call $collect_forward
     (i32.wrap_i64 (i64.load offset=8 (i32.mul (local.get $slot) (i32.const 16)))))))))
  (local.set $slot (i32.add (local.get $slot) (i32.const 1)))
  (br $reroot)))
 ;; Slide cells (and their dynamic Text) downward in ascending order.
 (local.set $text (i32.const 262144))
 (local.set $h (i32.const 1))
 (block $moved (loop $move
  (br_if $moved (i32.gt_u (local.get $h) (local.get $old)))
  (local.set $p (call $linear_address (local.get $h)))
  (local.set $kind (i32.load (local.get $p)))
  (if (i32.and (local.get $kind) (i32.const 256)) (then
   (local.set $kind (i32.and (local.get $kind) (i32.const 255)))
   (local.set $dst (call $linear_address (i32.load offset=4 (local.get $p))))
   (if (i32.ne (local.get $dst) (local.get $p))
    (then (memory.copy (local.get $dst) (local.get $p) (i32.const 48))))
   (i32.store (local.get $dst) (local.get $kind))
   (if (i32.eq (local.get $kind) (i32.const 11)) (then
    (local.set $x (i32.load offset=28 (local.get $dst)))
    (local.set $y (i32.load offset=32 (local.get $dst)))
    (if (i32.and (i32.ne (local.get $y) (i32.const 0))
         (i32.and (i32.ge_u (local.get $x) (i32.const 262144))
                  (i32.lt_u (local.get $x) (i32.const 393216)))) (then
     (if (i32.ne (local.get $text) (local.get $x))
      (then (memory.copy (local.get $text) (local.get $x) (local.get $y))))
     (i32.store offset=28 (local.get $dst) (local.get $text))
     (local.set $text (i32.add (local.get $text) (local.get $y)))))))))
  (local.set $h (i32.add (local.get $h) (i32.const 1)))
  (br $move)))
 (local.set $h (i32.add (local.get $count) (i32.const 1)))
 (block $cleared (loop $clear
  (br_if $cleared (i32.gt_u (local.get $h) (local.get $old)))
  (memory.fill (call $linear_address (local.get $h)) (i32.const 0) (i32.const 48))
  (local.set $h (i32.add (local.get $h) (i32.const 1)))
  (br $clear)))
 (memory.fill (local.get $text) (i32.const 0) (i32.sub (local.get $text_end) (local.get $text)))
 (i32.store (i32.const 65528) (local.get $text))
 (global.set $heap_cursor (local.get $count))
 (global.set $heap_baseline (local.get $count))
 ;; No certificate or observation may name a pre-compaction handle.
 (memory.fill (i32.const 57344) (i32.const 0) (i32.const 4096))
 (global.set $observed_program (i32.const 0))
 (global.set $recipe_count (i32.const 0))
 (global.set $rp (i32.const 0))
 (i32.const 0))
